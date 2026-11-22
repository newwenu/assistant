//! 中间件链（I36 spec Step 4）。
//!
//! P4 只注册 1、3、4、5、6、7、8 七项；② `SummarizeMiddleware` 随续跑式压缩同批
//! 落地（I38b D13），此处不注册、不预留空位。
//!
//! 顺序是语义：③ 必须在 ② 之后（P4 无 ②），④ 必须在 ③ 之后（否则还原的是空内容），
//! `after_tool` 内部顺序固定为 D26（豁免）→ D25（分类预算）→ D19（通用 2/3 阶梯）。

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};

use assistant_providers::types::ChatMessage;
use async_trait::async_trait;

use crate::agent::ladder::{self, ClearedBody};
use crate::agent::node_event::{MiddlewareAction, MiddlewareVerdict};
use crate::agent::round_executor::{ToolErrorClass, ToolExecResult, ToolStage};
use crate::multimodal::ProjectionPurpose;
use crate::request_projection::offload_if_over_budget;

/// P4 固定注册顺序（I36 spec §4.2 的第 1、3、4、5、6、7、8 项）。
pub const P4_MIDDLEWARE_ORDER: [&str; 7] = [
    "projection",
    "microcompact",
    "wrap-up-restore",
    "hard-trim",
    "tool-output-cap",
    "tool-outcome-capture",
    "repeated-failure",
];

pub const REPEATED_FAILURE_NUDGE_AT: u32 = 2;
pub const REPEATED_FAILURE_HALT_AT: u32 = 3;
/// 运行时兜底（I38a P2-C6 的 `per_tool_limit_chars`）。
pub const TOOL_OUTPUT_CAP_CHARS: usize = 50_000;
/// 豁免「同参重复」判定的工具：轮询类工具的同参重复是合法进展（D26）。
pub const EXEMPT_TOOLS: &[&str] = &[];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MiddlewareOutcome {
    Continue,
    /// `middleware` 只用于日志与遥测；`reason` 是用户可见正文，不得混入内部标识（I38b §5.1 / A21b）。
    StopTurn {
        middleware: &'static str,
        reason: String,
    },
}

/// 迭代级只读上下文；`before_model` / `after_tool` / `after_iteration` 三阶段共用。
pub struct TurnCtx<'a> {
    pub turn_id: &'a str,
    pub conversation_id: &'a str,
    pub iteration: u32,
    pub max_iterations: u32,
    pub model: &'a str,
}

/// `before_model` 的可变视图：阶梯与投影只改「本次请求」的消息副本，不写回历史。
pub struct ModelRequestView<'a> {
    pub messages: &'a mut Vec<ChatMessage>,
    pub context_window_tokens: Option<u32>,
    pub max_tokens: u32,
    pub is_wrap_up_call: bool,
}

/// `after_tool` 的可变视图：参数与指纹由 executor 现算（端口结果不携带参数，D25）。
pub struct ToolResultView<'a> {
    pub result: &'a mut ToolExecResult,
    pub arguments: &'a str,
    pub argument_fingerprint: &'a str,
    pub scope: &'a str,
}

#[async_trait]
pub trait TurnMiddleware: Send + Sync {
    fn name(&self) -> &'static str;

    async fn before_model(
        &self,
        req: &mut ModelRequestView<'_>,
        ctx: &TurnCtx<'_>,
    ) -> MiddlewareOutcome {
        let _ = (req, ctx);
        MiddlewareOutcome::Continue
    }

    async fn after_tool(
        &self,
        res: &mut ToolResultView<'_>,
        ctx: &TurnCtx<'_>,
    ) -> MiddlewareOutcome {
        let _ = (res, ctx);
        MiddlewareOutcome::Continue
    }

    async fn after_iteration(&self, ctx: &TurnCtx<'_>) -> MiddlewareOutcome {
        let _ = ctx;
        MiddlewareOutcome::Continue
    }
}

/// ⑧ 捕获的工具结果摘要，供 `RoundOutcome` / `TurnOutcome` 汇总。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedToolOutcome {
    pub call_id: String,
    pub tool_name: String,
    pub success: bool,
    pub arguments: String,
}

/// D25 的失败分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FailureClass {
    Validation,
    Permission,
    Transient,
    UncertainSideEffect,
}

impl FailureClass {
    /// 允许重试次数（nudge 之后）。
    pub fn retry_budget(self) -> u32 {
        match self {
            Self::Validation => 1,
            Self::Permission => 0,
            Self::Transient => 2,
            Self::UncertainSideEffect => 1,
        }
    }
}

/// 分类只看 `(stage, error_kind)`，不扫错误文本（D25）。
///
/// `Cancelled` 不是失败；`Persist` 及其余组合不分类，落 D19 通用阶梯。
pub fn classify_failure(stage: ToolStage, kind: Option<ToolErrorClass>) -> Option<FailureClass> {
    match (stage, kind) {
        (_, Some(ToolErrorClass::Cancelled)) => None,
        (ToolStage::Find, Some(ToolErrorClass::Internal)) => Some(FailureClass::Validation),
        (ToolStage::Validate, Some(ToolErrorClass::Business)) => Some(FailureClass::Validation),
        (ToolStage::Gateway, Some(ToolErrorClass::Denied)) => Some(FailureClass::Permission),
        (ToolStage::Gateway, Some(ToolErrorClass::Internal)) => Some(FailureClass::Transient),
        (ToolStage::Execute, Some(ToolErrorClass::Timeout)) => {
            Some(FailureClass::UncertainSideEffect)
        }
        (ToolStage::Execute, Some(ToolErrorClass::Business)) => Some(FailureClass::Validation),
        (ToolStage::Execute, Some(ToolErrorClass::Internal)) => Some(FailureClass::Transient),
        _ => None,
    }
}

fn nudge_text(stage: ToolStage, class: FailureClass, tool: &str) -> Option<String> {
    let text = match (class, stage) {
        (FailureClass::Validation, _) => format!(
            "工具 {} 上次调用未通过校验/不存在，请修正参数后重试一次。",
            tool
        ),
        (FailureClass::UncertainSideEffect, _) => format!(
            "工具 {} 超时可能已部分生效，请先核对结果再以更小粒度重试。",
            tool
        ),
        (FailureClass::Transient, ToolStage::Gateway) => {
            format!("工具 {} 网关内部故障，可重试。", tool)
        }
        _ => return None,
    };
    Some(text)
}

fn halt_summary(tool: &str, attempts: u32, last_error: &str) -> String {
    let reason = last_error.trim();
    let reason: String = reason.chars().take(200).collect();
    format!(
        "工具 {} 连续失败 {} 次，已停止本轮：{}",
        tool, attempts, reason
    )
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Default)]
struct FailureLedger {
    /// 必须带工具名：参数文本跨工具不唯一，只按指纹会让两个不同工具的同一段
    /// 参数（含解析失败的 `{}`）共用计数，误判成「重复了同一调用」。
    generic: HashMap<(String, String), u32>,
    classified: HashMap<(FailureClass, String, String), u32>,
    generic_counted_this_iteration: HashSet<(String, String)>,
    /// 本迭代已计过数的分类键：同一批次内同键多次失败只算 1 次。
    ///
    /// 否则并行批次里的同键失败会让 `Validation`（预算 1）在同一次迭代内耗尽预算，
    /// nudge 来不及被消费就被 halt（连续 = 跨请求）。
    classified_counted_this_iteration: HashSet<(FailureClass, String, String)>,
    nudges: Vec<String>,
    halt: Option<String>,
}

/// 一次 turn 内中间件共享的可变状态；链构建时创建，turn 结束即丢弃。
#[derive(Default)]
pub struct SharedTurnState {
    ladder: Mutex<Vec<ClearedBody>>,
    captured: Mutex<Vec<CapturedToolOutcome>>,
    failures: Mutex<FailureLedger>,
}

/// ① 阶梯：沿用既有 offload 预算口径，不重写阈值。
struct ProjectionMiddleware;

#[async_trait]
impl TurnMiddleware for ProjectionMiddleware {
    fn name(&self) -> &'static str {
        "projection"
    }

    async fn before_model(
        &self,
        req: &mut ModelRequestView<'_>,
        _ctx: &TurnCtx<'_>,
    ) -> MiddlewareOutcome {
        // ① 沿用既有 offload 预算口径，不叠加安全裕度（裕度只属于 ③④⑤，I36 spec §4.3）。
        let budget =
            crate::kernel::request::offload_budget_of(req.context_window_tokens, req.max_tokens);
        offload_if_over_budget(req.messages, budget, ProjectionPurpose::Send);
        MiddlewareOutcome::Continue
    }
}

/// ③ 微压缩：清空较旧工具结果体，记录供 ④ 还原。
struct MicrocompactMiddleware {
    keep_recent: usize,
    shared: Arc<SharedTurnState>,
}

#[async_trait]
impl TurnMiddleware for MicrocompactMiddleware {
    fn name(&self) -> &'static str {
        "microcompact"
    }

    async fn before_model(
        &self,
        req: &mut ModelRequestView<'_>,
        _ctx: &TurnCtx<'_>,
    ) -> MiddlewareOutcome {
        let budget = ladder::budget_of(req.context_window_tokens, req.max_tokens);
        let cleared = ladder::microcompact(req.messages, budget, self.keep_recent);
        // 累加而非覆盖：收尾调用时 ③ 通常已无内容可清，覆盖会把 ④ 要还原的记录抹掉（D23）。
        let mut ledger = lock(&self.shared.ladder);
        for entry in cleared {
            match ledger.iter_mut().find(|old| old.index == entry.index) {
                Some(old) => *old = entry,
                None => ledger.push(entry),
            }
        }
        MiddlewareOutcome::Continue
    }
}

/// ④ 收尾还原：只限 capped turn 的结论调用，额度从 ① 里拆。
struct WrapUpRestoreMiddleware {
    shared: Arc<SharedTurnState>,
}

#[async_trait]
impl TurnMiddleware for WrapUpRestoreMiddleware {
    fn name(&self) -> &'static str {
        "wrap-up-restore"
    }

    async fn before_model(
        &self,
        req: &mut ModelRequestView<'_>,
        _ctx: &TurnCtx<'_>,
    ) -> MiddlewareOutcome {
        if !req.is_wrap_up_call {
            return MiddlewareOutcome::Continue;
        }
        let Some(budget) = ladder::budget_of(req.context_window_tokens, req.max_tokens) else {
            return MiddlewareOutcome::Continue;
        };
        let estimated = crate::request_projection::estimate_total_tokens(req.messages);
        let (_, restore_allowance) = ladder::split_input_allowance(budget, estimated);
        if restore_allowance == 0 {
            return MiddlewareOutcome::Continue;
        }
        let cleared = lock(&self.shared.ladder).clone();
        ladder::restore_tool_bodies(req.messages, &cleared, restore_allowance);
        MiddlewareOutcome::Continue
    }
}

/// ⑤ 硬裁剪：整条淘汰最旧消息，system 与最后一条用户消息永不淘汰。
struct HardTrimMiddleware;

#[async_trait]
impl TurnMiddleware for HardTrimMiddleware {
    fn name(&self) -> &'static str {
        "hard-trim"
    }

    async fn before_model(
        &self,
        req: &mut ModelRequestView<'_>,
        _ctx: &TurnCtx<'_>,
    ) -> MiddlewareOutcome {
        if let Some(budget) = ladder::budget_of(req.context_window_tokens, req.max_tokens) {
            ladder::hard_trim(req.messages, budget);
        }
        MiddlewareOutcome::Continue
    }
}

/// ⑥ 单条工具结果字符上限：进入 transcript 之前的兜底截断。
struct ToolOutputCapMiddleware;

#[async_trait]
impl TurnMiddleware for ToolOutputCapMiddleware {
    fn name(&self) -> &'static str {
        "tool-output-cap"
    }

    async fn after_tool(
        &self,
        res: &mut ToolResultView<'_>,
        _ctx: &TurnCtx<'_>,
    ) -> MiddlewareOutcome {
        if let Some(capped) = cap_chars(&res.result.content, TOOL_OUTPUT_CAP_CHARS) {
            res.result.content = capped;
        }
        MiddlewareOutcome::Continue
    }
}

fn cap_chars(text: &str, cap: usize) -> Option<String> {
    let total = text.chars().count();
    if total <= cap {
        return None;
    }
    let head = cap * 2 / 3;
    let tail = cap - head;
    let head_text: String = text.chars().take(head).collect();
    let tail_text: String = text.chars().skip(total - tail).collect();
    Some(format!("{}\n...\n{}", head_text, tail_text))
}

/// ⑦ 捕获结果摘要（success / call_id / 原始参数）。
struct ToolOutcomeCaptureMiddleware {
    shared: Arc<SharedTurnState>,
}

#[async_trait]
impl TurnMiddleware for ToolOutcomeCaptureMiddleware {
    fn name(&self) -> &'static str {
        "tool-outcome-capture"
    }

    async fn after_tool(
        &self,
        res: &mut ToolResultView<'_>,
        _ctx: &TurnCtx<'_>,
    ) -> MiddlewareOutcome {
        lock(&self.shared.captured).push(CapturedToolOutcome {
            call_id: res.result.call_id.clone(),
            tool_name: res.result.tool_name.clone(),
            success: !res.result.is_error,
            arguments: res.arguments.to_string(),
        });
        MiddlewareOutcome::Continue
    }
}

/// ⑧ 失败阶梯：豁免（D26）→ 分类预算（D25）→ 通用 2/3 阶梯（D19）。
struct RepeatedFailureMiddleware {
    shared: Arc<SharedTurnState>,
}

#[async_trait]
impl TurnMiddleware for RepeatedFailureMiddleware {
    fn name(&self) -> &'static str {
        "repeated-failure"
    }

    async fn after_tool(
        &self,
        res: &mut ToolResultView<'_>,
        _ctx: &TurnCtx<'_>,
    ) -> MiddlewareOutcome {
        let result = &res.result;
        let mut ledger = lock(&self.shared.failures);

        // 成功一次即重置：同工具同参计数清空，该 scope 的分类计数也清空（D25）。
        if !result.is_error {
            let fingerprint = res.argument_fingerprint.to_string();
            let tool = result.tool_name.clone();
            ledger.generic.remove(&(tool.clone(), fingerprint.clone()));
            ledger
                .generic_counted_this_iteration
                .retain(|(name, fp)| !(name == &tool && fp == &fingerprint));
            ledger
                .classified
                .retain(|(_, name, scope), _| !(name == &tool && scope == res.scope));
            ledger
                .classified_counted_this_iteration
                .retain(|(_, name, scope)| !(name == &tool && scope == res.scope));
            return MiddlewareOutcome::Continue;
        }

        // 取消不是失败（D20）；豁免工具即使同参失败也不进任何阶梯（D26）。
        if matches!(result.error_kind, Some(ToolErrorClass::Cancelled)) {
            return MiddlewareOutcome::Continue;
        }
        if EXEMPT_TOOLS.contains(&result.tool_name.as_str()) {
            return MiddlewareOutcome::Continue;
        }

        match classify_failure(result.stage, result.error_kind) {
            Some(class) => {
                let key = (class, result.tool_name.clone(), res.scope.to_string());
                if !ledger.classified_counted_this_iteration.insert(key.clone()) {
                    return MiddlewareOutcome::Continue;
                }
                let count = ledger.classified.entry(key).or_insert(0);
                *count += 1;
                let attempts = *count;
                if attempts > class.retry_budget() {
                    ledger.halt = Some(halt_summary(&result.tool_name, attempts, &result.content));
                } else if attempts == 1 {
                    if let Some(text) = nudge_text(result.stage, class, &result.tool_name) {
                        ledger.nudges.push(text);
                    }
                }
            }
            None => {
                let key = (
                    result.tool_name.clone(),
                    res.argument_fingerprint.to_string(),
                );
                if !ledger.generic_counted_this_iteration.insert(key.clone()) {
                    return MiddlewareOutcome::Continue;
                }
                let count = ledger.generic.entry(key).or_insert(0);
                *count += 1;
                let attempts = *count;
                if attempts >= REPEATED_FAILURE_HALT_AT {
                    ledger.halt = Some(halt_summary(&result.tool_name, attempts, &result.content));
                } else if attempts == REPEATED_FAILURE_NUDGE_AT {
                    ledger.nudges.push(format!(
                        "你刚才重复了同一调用（{}），请换一种方式或修正参数。",
                        result.tool_name
                    ));
                }
            }
        }
        MiddlewareOutcome::Continue
    }

    async fn after_iteration(&self, _ctx: &TurnCtx<'_>) -> MiddlewareOutcome {
        let mut ledger = lock(&self.shared.failures);
        // 迭代边界：下一批次的同键失败重新获得计数机会（连续 = 跨请求）。
        ledger.generic_counted_this_iteration.clear();
        ledger.classified_counted_this_iteration.clear();
        match ledger.halt.take() {
            Some(reason) => MiddlewareOutcome::StopTurn {
                middleware: self.name(),
                reason,
            },
            None => MiddlewareOutcome::Continue,
        }
    }
}

/// `before_model` 的改写快照：只比较这三项（I55 Q2），不比较消息正文（逐条比文本成本高）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RequestSnapshot {
    messages: usize,
    context_window_tokens: Option<u32>,
    max_tokens: u32,
}

impl RequestSnapshot {
    fn of(req: &ModelRequestView<'_>) -> Self {
        Self {
            messages: req.messages.len(),
            context_window_tokens: req.context_window_tokens,
            max_tokens: req.max_tokens,
        }
    }
}

/// `after_tool` 的改写快照：长度与错误标记即可覆盖截断、分类改写。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResultSnapshot {
    content_len: usize,
    is_error: bool,
}

impl ResultSnapshot {
    fn of(result: &ToolExecResult) -> Self {
        Self {
            content_len: result.content.len(),
            is_error: result.is_error,
        }
    }
}

/// 固定顺序中间件链：不做运行时排序。
pub struct MiddlewareChain {
    order: Vec<Arc<dyn TurnMiddleware>>,
    shared: Arc<SharedTurnState>,
}

impl MiddlewareChain {
    /// P4 实际链：1、3、4、5、6、7、8 共七项，② 不注册；常量与实例顺序不允许分叉。
    pub fn p4(microcompact_keep_recent: usize) -> Self {
        let shared = Arc::new(SharedTurnState::default());
        let order: Vec<Arc<dyn TurnMiddleware>> = P4_MIDDLEWARE_ORDER
            .iter()
            .map(|name| match *name {
                "projection" => Arc::new(ProjectionMiddleware) as Arc<dyn TurnMiddleware>,
                "microcompact" => Arc::new(MicrocompactMiddleware {
                    keep_recent: microcompact_keep_recent,
                    shared: shared.clone(),
                }),
                "wrap-up-restore" => Arc::new(WrapUpRestoreMiddleware {
                    shared: shared.clone(),
                }),
                "hard-trim" => Arc::new(HardTrimMiddleware),
                "tool-output-cap" => Arc::new(ToolOutputCapMiddleware),
                "tool-outcome-capture" => Arc::new(ToolOutcomeCaptureMiddleware {
                    shared: shared.clone(),
                }),
                "repeated-failure" => Arc::new(RepeatedFailureMiddleware {
                    shared: shared.clone(),
                }),
                other => unreachable!("P4_MIDDLEWARE_ORDER 含未实现的中间件: {other}"),
            })
            .collect();
        Self { order, shared }
    }

    /// 仅 ① 投影（`spec.ladder = ProjectionOnly`）：③④⑤ 不注册，executor 不做运行时分支。
    pub fn projection_only() -> Self {
        Self {
            order: vec![Arc::new(ProjectionMiddleware) as Arc<dyn TurnMiddleware>],
            shared: Arc::new(SharedTurnState::default()),
        }
    }

    pub fn names(&self) -> Vec<&str> {
        self.order.iter().map(|mw| mw.name()).collect()
    }

    /// 返回 `(结果, 实际改写过请求的中间件名)`。
    ///
    /// `applied` 只记真正改动过请求的项（I55 Q2）：执行前后快照三项任一变化才记入。
    pub async fn before_model(
        &self,
        req: &mut ModelRequestView<'_>,
        ctx: &TurnCtx<'_>,
    ) -> (MiddlewareOutcome, Vec<&'static str>) {
        let mut applied: Vec<&'static str> = Vec::new();
        for mw in &self.order {
            let before = RequestSnapshot::of(req);
            let outcome = mw.before_model(req, ctx).await;
            if RequestSnapshot::of(req) != before {
                applied.push(mw.name());
            }
            if let MiddlewareOutcome::StopTurn { middleware, reason } = outcome {
                log::warn!(
                    target: "agent::middleware",
                    "中间件 {} 在 before_model 终止本轮：{}",
                    middleware,
                    reason
                );
                return (MiddlewareOutcome::StopTurn { middleware, reason }, applied);
            }
        }
        (MiddlewareOutcome::Continue, applied)
    }

    /// 返回 `(结果, 本结果上各中间件的判定)`。
    ///
    /// 判定只记 `stop` 与真正改动过结果的 `continue`；恒 `continue` 且无改写的不入列，
    /// 否则每个结果会固定塞入 7 条空判定，把 `after-tool` 的 detail 变成噪声。
    pub async fn after_tool(
        &self,
        res: &mut ToolResultView<'_>,
        ctx: &TurnCtx<'_>,
    ) -> (MiddlewareOutcome, Vec<MiddlewareVerdict>) {
        let mut verdicts: Vec<MiddlewareVerdict> = Vec::new();
        for mw in &self.order {
            let before = ResultSnapshot::of(res.result);
            let outcome = mw.after_tool(res, ctx).await;
            match outcome {
                MiddlewareOutcome::Continue => {
                    if ResultSnapshot::of(res.result) != before {
                        verdicts.push(MiddlewareVerdict {
                            middleware: mw.name().to_string(),
                            action: MiddlewareAction::Continue,
                            reason: None,
                        });
                    }
                }
                MiddlewareOutcome::StopTurn { middleware, reason } => {
                    verdicts.push(MiddlewareVerdict {
                        middleware: middleware.to_string(),
                        action: MiddlewareAction::Stop,
                        reason: Some(reason.clone()),
                    });
                    log::warn!(
                        target: "agent::middleware",
                        "中间件 {} 在 after_tool 终止本轮：{}",
                        middleware,
                        reason
                    );
                    return (MiddlewareOutcome::StopTurn { middleware, reason }, verdicts);
                }
            }
        }
        (MiddlewareOutcome::Continue, verdicts)
    }

    pub async fn after_iteration(&self, ctx: &TurnCtx<'_>) -> MiddlewareOutcome {
        for mw in &self.order {
            if let MiddlewareOutcome::StopTurn { middleware, reason } =
                mw.after_iteration(ctx).await
            {
                log::warn!(
                    target: "agent::middleware",
                    "中间件 {} 在 after_iteration 终止本轮：{}",
                    middleware,
                    reason
                );
                return MiddlewareOutcome::StopTurn { middleware, reason };
            }
        }
        MiddlewareOutcome::Continue
    }

    /// 取出本迭代累积的 nudge；只进下一次请求，绝不写回 `messages` 表。
    pub fn take_pending_nudges(&self) -> Vec<String> {
        std::mem::take(&mut lock(&self.shared.failures).nudges)
    }

    pub fn captured_outcomes(&self) -> Vec<CapturedToolOutcome> {
        lock(&self.shared.captured).clone()
    }
}

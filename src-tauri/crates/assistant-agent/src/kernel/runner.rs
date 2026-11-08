use std::sync::Arc;

use assistant_providers::base::{ProviderError, ProviderErrorPayload};

use crate::compaction::{self, CompactDecision};

use crate::agent::round_executor::{
    RoundExecutor, ToolExecutionPort, TurnBudget, TurnObserver, TurnRunCtx,
};
use crate::agent::tool_event::ToolEvent;

use super::emit::TurnEmitter;
use super::ids::TurnContext;
use super::outcome::{TurnError, TurnOutcome, TurnStatus};
use super::persist::{cache_capable_of, PersistCtx, TurnPersister};
use super::phase::TurnPhase;
use super::ports::{
    ChatError, EventSink, MessageStore, ProviderDelta, TurnGeneration, TurnInputSource,
};
use super::state::SessionStates;
use super::tasks::TurnTasks;
use super::{EventRoute, TurnSpec};

/// 代际守卫：`TurnGeneration` 的生产实现，判定与 `TurnPersister::is_current` 同源。
struct TurnGenerationGuard {
    tasks: Arc<TurnTasks>,
    conversation_id: String,
    turn_id: super::ids::TurnId,
}

impl TurnGeneration for TurnGenerationGuard {
    fn is_current(&self) -> bool {
        self.tasks.is_current(&self.conversation_id, &self.turn_id)
    }
}

/// 终止式压缩的触发配置；`None` 表示该链路不做压缩判断。
#[derive(Clone)]
pub struct CompactionConfig {
    pub sink: Arc<dyn super::ports::CompactionSink>,
    pub auto_compact: bool,
    pub threshold_percent: u8,
}

/// Turn 运行时。`chat` 与 `agent` 只差 `TurnSpec` 参数，内核内不出现链路分支。
pub struct TurnRunner {
    sink: Arc<dyn EventSink>,
    store: Arc<dyn MessageStore>,
    input: Arc<dyn TurnInputSource>,
    states: Arc<SessionStates>,
    tasks: Arc<TurnTasks>,
    clock: Arc<dyn super::Clock>,
    round_executor: Arc<dyn RoundExecutor>,
    tool_exec: Option<Arc<dyn ToolExecutionPort>>,
    compaction: Option<CompactionConfig>,
}

impl TurnRunner {
    pub fn new(
        sink: Arc<dyn EventSink>,
        store: Arc<dyn MessageStore>,
        input: Arc<dyn TurnInputSource>,
        states: Arc<SessionStates>,
        tasks: Arc<TurnTasks>,
        clock: Arc<dyn super::Clock>,
        compaction: Option<CompactionConfig>,
    ) -> Self {
        Self {
            sink,
            store,
            input,
            states,
            tasks,
            clock,
            round_executor: Arc::new(crate::agent::loop_executor::LoopRoundExecutor),
            tool_exec: None,
            compaction,
        }
    }

    /// 工具执行端口由宿主装配（`ToolPipeline` 适配器）；`None` 表示该链路无工具执行实现。
    pub fn with_tool_exec(mut self, port: Arc<dyn ToolExecutionPort>) -> Self {
        self.tool_exec = Some(port);
        self
    }

    /// 唯一入口：跑完一个 turn 并返回终态。宿主只负责 begin / spawn / 映射返回值。
    pub async fn run(&self, ctx: &TurnContext, spec: &TurnSpec) -> TurnOutcome {
        let conversation_id = ctx.conversation_id.as_str();
        // 墙钟起点唯一来源：注入 `Clock`，不读真实 `Instant::now()`（H2）。
        let started_at = self.clock.now();
        let emitter = TurnEmitter::new(
            self.sink.clone(),
            self.tasks.clone(),
            conversation_id,
            ctx.turn_id.clone(),
        );
        let mut persister = TurnPersister::new(
            self.store.clone(),
            self.tasks.clone(),
            PersistCtx {
                conversation_id: conversation_id.to_string(),
                turn_id: ctx.turn_id.clone(),
                message_id: ctx.message_id.clone(),
                branch_leaf_id: None,
                kind: spec.kind.to_string(),
                provider_id: None,
                model_id: None,
                cache_capable: false,
                created_at_ms: self.clock.now_millis(),
            },
        );

        let mut terminal_error: Option<ChatError> = None;
        let mut compaction_decision: Option<CompactDecision> = None;
        let mut outcome = self
            .drive(
                ctx,
                spec,
                started_at,
                &emitter,
                &mut persister,
                &mut terminal_error,
                &mut compaction_decision,
            )
            .await;

        if let Err(err) = persister.persist_terminal(&outcome).await {
            log::error!("terminal persist failed: {err}");
            outcome.status = TurnStatus::Failed;
            outcome
                .error
                .get_or_insert(TurnError::Other(err.to_string()));
        }

        // 先落库、后改状态（C5.19）。
        let still_current = self.tasks.is_current(conversation_id, &ctx.turn_id);
        self.log_turn_summary(ctx, &outcome, started_at);
        self.states
            .finish_turn(conversation_id, &ctx.turn_id, &outcome);
        emitter.terminated(outcome.status, terminal_error);
        self.tasks.finish(conversation_id, &ctx.turn_id);

        // 压缩必须在本 turn 完全收尾后触发：压缩自身的 `begin(Replace)` 会顶替当前代际，
        // 提前触发会让本轮的终态落库与终态事件被静默丢弃。
        if still_current {
            if let (Some(config), Some(CompactDecision::Terminate)) =
                (&self.compaction, compaction_decision)
            {
                config.sink.terminate(conversation_id).await;
            }
        }
        outcome
    }

    #[allow(clippy::too_many_arguments)]
    async fn drive(
        &self,
        ctx: &TurnContext,
        spec: &TurnSpec,
        started_at: std::time::Instant,
        emitter: &TurnEmitter,
        persister: &mut TurnPersister,
        terminal_error: &mut Option<ChatError>,
        compaction_decision: &mut Option<CompactDecision>,
    ) -> TurnOutcome {
        let conversation_id = ctx.conversation_id.as_str();
        let token = self.tasks.cancel_token(conversation_id).unwrap_or_else(|| {
            log::warn!(
                "turn {} has no cancel token, running uncancellable",
                ctx.turn_id
            );
            tokio_util::sync::CancellationToken::new()
        });
        let mut outcome = TurnOutcome::new();

        if !self.tasks.is_current(conversation_id, &ctx.turn_id) {
            log::warn!("turn {} is stale, skipping", ctx.turn_id);
            outcome.status = TurnStatus::Interrupted;
            outcome.error = Some(TurnError::Other("stale turn".to_string()));
            return outcome;
        }

        if let Err(err) = self
            .states
            .begin_turn_with(conversation_id, &ctx.turn_id, spec.on_busy)
        {
            outcome.status = TurnStatus::Failed;
            outcome.error = Some(TurnError::Other(format!("{err:?}")));
            *terminal_error = Some(ProviderErrorPayload::from(&ProviderError::RequestRejected(
                "conversation is busy".to_string(),
            )));
            return outcome;
        }

        emitter.stream_start(TurnPhase::Starting);
        self.set_phase(
            emitter,
            spec,
            conversation_id,
            &ctx.turn_id,
            TurnPhase::Thinking,
        );

        let input = match cancellable(&token, self.input.conversation_detail(conversation_id)).await
        {
            None => {
                outcome.status = TurnStatus::Cancelled;
                *terminal_error = Some(ProviderErrorPayload::from(&ProviderError::Cancelled));
                return outcome;
            }
            Some(Err(err)) => {
                outcome.status = TurnStatus::Failed;
                outcome.error = Some(TurnError::Other(err.to_string()));
                *terminal_error = Some(ProviderErrorPayload::from(
                    &ProviderError::RequestRejected(err.to_string()),
                ));
                return outcome;
            }
            Some(Ok(input)) => input,
        };

        persister.set_source(
            &input.provider.row,
            input.detail.conversation.active_branch_id.clone(),
            Some(input.detail.conversation.model_id.clone()),
        );

        let generation = TurnGenerationGuard {
            tasks: self.tasks.clone(),
            conversation_id: conversation_id.to_string(),
            turn_id: ctx.turn_id.clone(),
        };
        let observer = RunnerObserver {
            states: self.states.as_ref(),
            emitter,
            spec,
            conversation_id,
            turn_id: &ctx.turn_id,
        };
        let round_ctx = TurnRunCtx {
            turn_id: &ctx.turn_id,
            conversation_id,
            initial_message_id: &ctx.message_id,
            start_iteration: 1,
            remaining_turn_budget: TurnBudget {
                remaining_tool_calls: spec.limits.max_tool_calls,
                turn_deadline: spec.limits.turn_wall_clock.map(|d| started_at + d),
            },
            turn_started_at: started_at,
            child_token: &token,
            tools: spec.tools.as_deref(),
            tool_exec: self.tool_exec.as_deref(),
            observer: &observer,
            store: self.store.as_ref(),
            input: &input,
            clock: self.clock.as_ref(),
            generation: &generation,
            limits: &spec.limits,
            spec,
            provider_id: Some(input.provider.row.id.as_str()),
            model_id: Some(input.detail.conversation.model_id.as_str()),
            model: input.detail.conversation.model_id.as_str(),
            branch_leaf_id: input.detail.conversation.active_branch_id.as_deref(),
            cache_capable: cache_capable_of(&input.provider.row),
            agent_id: "root",
            parent_turn_id: None,
            delegation_depth: 0,
        };

        let round = self.round_executor.run_iterations(round_ctx).await;

        *compaction_decision = self.decide_compaction_from_outcome(&round, conversation_id);

        outcome.status = round.status;
        outcome.text = round.text;
        outcome.reasoning = round.reasoning;
        outcome.iterations = round.iterations;
        outcome.model_calls = round.model_calls;
        outcome.tool_calls = round.tool_calls;
        outcome.prompt_tokens = round.prompt_tokens;
        outcome.completion_tokens = round.completion_tokens;
        outcome.cache_read_tokens = round.cache_read_tokens;
        outcome.total_tokens = round.total_tokens;
        outcome.attempt_usages = round.attempt_usages;
        outcome.ttft_ms = round.ttft_ms;
        outcome.hit_cap = round.hit_cap;
        outcome.wrap_up_injected = round.wrap_up_injected;
        outcome.final_message_id = round.final_message_id;
        outcome.error = round.error;

        // 执行器把 provider 的**原始**错误载荷一并带出（`code` / `retryable` /
        // `retry_after_ms` 等），在此还原为 `TurnEvent::Terminated.error`。
        // `TurnError::Provider` 只有文案，无法承载这些字段（I38b V8）。
        if terminal_error.is_none() {
            *terminal_error = round.provider_error;
        }

        outcome
    }

    /// 每轮一条固定字段汇总日志（design §9.1）；TTFT 无 delta 时记 `-`。
    fn log_turn_summary(
        &self,
        ctx: &TurnContext,
        outcome: &TurnOutcome,
        started_at: std::time::Instant,
    ) {
        let elapsed_ms = self.clock.now().duration_since(started_at).as_millis() as u64;
        log::info!(
            "[turn] conv={} turn={} status={} iterations={} model_calls={} tool_calls={} \
             prompt={} completion={} cache_read={} ttft_ms={} elapsed_ms={} hit_cap={}",
            ctx.conversation_id,
            ctx.turn_id,
            outcome.status.as_str(),
            outcome.iterations,
            outcome.model_calls,
            outcome.tool_calls,
            outcome.prompt_tokens.unwrap_or(0),
            outcome.completion_tokens.unwrap_or(0),
            outcome.cache_read_tokens.unwrap_or(0),
            outcome
                .ttft_ms
                .map(|ms| ms.to_string())
                .unwrap_or_else(|| "-".to_string()),
            elapsed_ms,
            outcome.hit_cap,
        );
    }

    fn set_phase(
        &self,
        emitter: &TurnEmitter,
        spec: &TurnSpec,
        conversation_id: &str,
        turn_id: &str,
        phase: TurnPhase,
    ) {
        self.states
            .set_phase(conversation_id, &turn_id.to_string(), phase);
        // 状态快照事件只对广播路由发；orb 链路保持原有事件序列不变。
        if spec.route == EventRoute::Broadcast {
            emitter.phase_changed(phase);
        }
    }

    /// 只做判定，不在 turn 内部触发；触发点由 `run` 在 turn 收尾后统一执行。
    fn decide_compaction_from_outcome(
        &self,
        round: &crate::agent::round_executor::RoundOutcome,
        conversation_id: &str,
    ) -> Option<CompactDecision> {
        let config = self.compaction.as_ref()?;
        let prompt_tokens = round.prompt_tokens?;
        let decision = compaction::evaluate_compaction(
            prompt_tokens,
            round.context_window_tokens,
            config.auto_compact,
            config.threshold_percent,
            round.reserved_output_tokens,
        );
        if decision == CompactDecision::Undetermined {
            log::warn!(
                "[compact-decision] conversation={} 窗口未知，无法判定是否超限；不触发自动终止（P={}）",
                conversation_id,
                prompt_tokens
            );
        }
        Some(decision)
    }
}

/// 交给 RoundExecutor 的唯一 phase / 工具事件入口（I38b C3.18）。
/// 执行器不得自持 `EventSink` 绕开这里，`SessionStates` 仍是唯一写方。
struct RunnerObserver<'a> {
    states: &'a SessionStates,
    emitter: &'a TurnEmitter,
    spec: &'a TurnSpec,
    conversation_id: &'a str,
    turn_id: &'a str,
}

impl TurnObserver for RunnerObserver<'_> {
    fn set_phase(&self, phase: TurnPhase) {
        self.states
            .set_phase(self.conversation_id, &self.turn_id.to_string(), phase);
        if self.spec.route == EventRoute::Broadcast {
            self.emitter.phase_changed(phase);
        }
    }

    fn set_iteration(&self, iteration: u32) {
        self.emitter.set_iteration(iteration);
    }

    fn emit_tool_event(&self, event: ToolEvent) {
        self.emitter.tool_event(event);
    }

    fn delta(&self, delta: &ProviderDelta) -> u32 {
        self.emitter.delta(delta)
    }
}

/// 长 await 一律走这里：`biased` 优先响应取消，禁止 `JoinHandle::abort()`（C4.9）。
async fn cancellable<F, T>(token: &tokio_util::sync::CancellationToken, fut: F) -> Option<T>
where
    F: std::future::Future<Output = T>,
{
    tokio::select! {
        biased;
        _ = token.cancelled() => None,
        value = fut => Some(value),
    }
}

use std::sync::Arc;
use std::time::Duration;

use assistant_providers::base::{ProviderError, ProviderErrorPayload};

use crate::compaction::{self, CompactDecision};

use crate::agent::round_executor::{RoundExecutor, TurnBudget, TurnRunCtx};

use super::emit::TurnEmitter;
use super::ids::TurnContext;
use super::outcome::{TurnError, TurnOutcome, TurnStatus};
use super::persist::{PersistCtx, TurnPersister};
use super::phase::TurnPhase;
use super::ports::{ChatError, EventSink, MessageStore, ProviderDelta, TurnInputSource};
use super::request::{self, ChatRequest};
use super::state::SessionStates;
use super::stream::{self, PartialOutcome, StreamError, StreamOutcome};
use super::tasks::TurnTasks;
use super::{EventRoute, TurnSpec};

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
    round_executor: Option<Arc<dyn RoundExecutor>>,
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
            round_executor: None,
            compaction,
        }
    }

    /// P3 恒为 `None`；P4 注入真实实现后才会有工具轮。
    pub fn with_round_executor(mut self, executor: Arc<dyn RoundExecutor>) -> Self {
        self.round_executor = Some(executor);
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

        let mut iteration: u32 = 0;
        let mut empty_retries: u32 = 0;
        let mut retrying_empty = false;

        loop {
            // 1. 刹车
            if token.is_cancelled() {
                outcome.status = TurnStatus::Cancelled;
                *terminal_error = Some(ProviderErrorPayload::from(&ProviderError::Cancelled));
                break;
            }
            if self.wall_clock_exceeded(started_at, &spec.limits) {
                outcome.status = TurnStatus::Failed;
                outcome.error = Some(TurnError::Other("turn wall clock exceeded".to_string()));
                *terminal_error = Some(ProviderErrorPayload::from(&ProviderError::Timeout(
                    "Turn wall clock exceeded".to_string(),
                )));
                break;
            }

            if !retrying_empty {
                if iteration >= spec.max_iterations {
                    outcome.hit_cap = true;
                    outcome.status = TurnStatus::Completed;
                    break;
                }
                iteration += 1;
                outcome.iterations = iteration;
                emitter.set_iteration(iteration);
                self.set_phase(
                    emitter,
                    spec,
                    conversation_id,
                    &ctx.turn_id,
                    TurnPhase::Thinking,
                );
            }
            retrying_empty = false;

            // 2. 阶梯：P3 只有投影 + 终止式压缩，构建即完成
            // 3. 单次调用
            let request_started = self.clock.now();
            let request = match request::build(&input, iteration, spec.tools.as_ref()).await {
                Ok(request) => request,
                Err(err) => {
                    outcome.status = TurnStatus::Failed;
                    outcome.error = Some(TurnError::Other(err.clone()));
                    *terminal_error = Some(ProviderErrorPayload::from(
                        &ProviderError::RequestRejected(err),
                    ));
                    break;
                }
            };

            let child = TurnTasks::child_token(&token);
            let stream = match request.stream(child.clone()) {
                Ok(stream) => stream,
                Err(err) => {
                    outcome.status = TurnStatus::Failed;
                    *terminal_error = Some(ProviderErrorPayload::from(&err));
                    outcome.error = Some(TurnError::Other(err.to_string()));
                    break;
                }
            };

            self.set_phase(
                emitter,
                spec,
                conversation_id,
                &ctx.turn_id,
                TurnPhase::Streaming,
            );
            outcome.model_calls += 1;

            let timeout = self.effective_model_call_timeout(started_at, &spec.limits);
            let mut tool_acc = crate::agent::tool_call::ToolCallAccumulator::new();
            let mut tool_calls: Vec<crate::agent::tool_call::FinalizedToolCall> = Vec::new();
            let result = stream::consume(
                stream,
                child.clone(),
                timeout,
                self.clock.as_ref(),
                request_started,
                |_seq, delta| {
                    if let Some(previous) = tool_acc.apply_chat_delta(delta).finalized_previous {
                        tool_calls.push(previous);
                    }
                    emit_delta(emitter, delta)
                },
            )
            .await;

            match result {
                Ok(stream_outcome) => {
                    tool_calls.extend(
                        tool_acc.finalize_all(crate::agent::tool_call::ToolCallBoundary::StreamEnd),
                    );
                    apply_usage(&mut outcome, &stream_outcome);
                    outcome.text = stream_outcome.full_content.clone();
                    outcome.reasoning = stream_outcome.full_reasoning.clone();
                    outcome.attempt_usages = stream_outcome.attempt_usages.clone();

                    // 4. 空响应重试：不递增 `iteration`
                    if !stream_outcome.has_effective_output
                        && empty_retries < spec.limits.empty_response_retries
                    {
                        empty_retries += 1;
                        retrying_empty = true;
                        continue;
                    }
                    if !stream_outcome.has_effective_output {
                        outcome.status = TurnStatus::Failed;
                        outcome.error = Some(TurnError::Other("empty response".to_string()));
                        *terminal_error = Some(ProviderErrorPayload::from(
                            &ProviderError::ParseError("empty response".to_string()),
                        ));
                        break;
                    }

                    // 5. 无工具结束
                    if tool_calls.is_empty() || spec.tools.is_none() {
                        outcome.status = TurnStatus::Completed;
                        *compaction_decision =
                            self.decide_compaction(&request, &outcome, conversation_id);
                        break;
                    }

                    // 7. 工具执行：P3 无执行器，直接按触顶收尾
                    match &self.round_executor {
                        Some(executor) => {
                            self.set_phase(
                                emitter,
                                spec,
                                conversation_id,
                                &ctx.turn_id,
                                TurnPhase::ToolCalling,
                            );
                            let round_ctx = TurnRunCtx {
                                turn_id: &ctx.turn_id,
                                conversation_id,
                                iteration,
                                remaining_turn_budget: TurnBudget {
                                    remaining_tool_calls: spec.limits.max_tool_calls,
                                    turn_deadline: None,
                                },
                                child_token: &child,
                                tools: spec.tools.as_deref(),
                                tool_exec: None,
                                sink: self.sink.as_ref(),
                                store: self.store.as_ref(),
                                input: self.input.as_ref(),
                                clock: self.clock.as_ref(),
                                limits: &spec.limits,
                                spec,
                                agent_id: "root",
                                parent_turn_id: None,
                                delegation_depth: 0,
                            };
                            let round =
                                cancellable(&child, executor.run_iterations(round_ctx)).await;
                            let Some(round) = round else {
                                outcome.status = TurnStatus::Cancelled;
                                *terminal_error =
                                    Some(ProviderErrorPayload::from(&ProviderError::Cancelled));
                                break;
                            };
                            outcome.tool_calls += round.tool_calls;
                            if !round.hit_cap {
                                continue;
                            }
                            outcome.hit_cap = true;
                            outcome.status = TurnStatus::Completed;
                            break;
                        }
                        None => {
                            log::warn!(
                                "turn {} requested {} tool calls but no executor is wired",
                                ctx.turn_id,
                                tool_calls.len()
                            );
                            outcome.hit_cap = true;
                            outcome.status = TurnStatus::Completed;
                            break;
                        }
                    }
                }
                Err(StreamError::Cancelled(partial)) => {
                    self.save_partial(persister, &partial).await;
                    outcome.text = partial.full_content.clone();
                    apply_partial_usage(&mut outcome, &partial);
                    outcome.status = TurnStatus::Cancelled;
                    *terminal_error = Some(ProviderErrorPayload::from(&ProviderError::Cancelled));
                    break;
                }
                Err(StreamError::Timeout(partial)) => {
                    self.save_partial(persister, &partial).await;
                    outcome.text = partial.full_content.clone();
                    apply_partial_usage(&mut outcome, &partial);
                    outcome.status = TurnStatus::Failed;
                    outcome.error = Some(TurnError::Other("stream timed out".to_string()));
                    *terminal_error = Some(ProviderErrorPayload::from(&ProviderError::Timeout(
                        "Stream timed out".to_string(),
                    )));
                    break;
                }
                Err(StreamError::Provider(err, partial)) => {
                    self.save_partial(persister, &partial).await;
                    outcome.text = partial.full_content.clone();
                    apply_partial_usage(&mut outcome, &partial);
                    outcome.status = TurnStatus::Failed;
                    outcome.error = Some(TurnError::Other(err.to_string()));
                    *terminal_error = Some(ProviderErrorPayload::from(&err));
                    break;
                }
            }
        }

        outcome
    }

    async fn save_partial(&self, persister: &TurnPersister, partial: &PartialOutcome) {
        if let Err(err) = persister.persist_partial(partial).await {
            log::error!("partial persist failed: {err}");
        }
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

    fn wall_clock_exceeded(
        &self,
        started_at: std::time::Instant,
        limits: &super::limits::TurnLimits,
    ) -> bool {
        match limits.turn_wall_clock {
            Some(limit) if !limit.is_zero() => self.clock.now().duration_since(started_at) >= limit,
            _ => false,
        }
    }

    /// 单次超时取 `min(model_call, turn 剩余)`；都关闭时为 `None`。
    fn effective_model_call_timeout(
        &self,
        started_at: std::time::Instant,
        limits: &super::limits::TurnLimits,
    ) -> Option<Duration> {
        let model_call = limits.model_call.filter(|d| !d.is_zero());
        let remaining = limits
            .turn_wall_clock
            .filter(|d| !d.is_zero())
            .map(|limit| limit.saturating_sub(self.clock.now().duration_since(started_at)));

        match (model_call, remaining) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }

    /// 只做判定，不在 turn 内部触发；触发点由 `run` 在 turn 收尾后统一执行。
    fn decide_compaction(
        &self,
        request: &ChatRequest,
        outcome: &TurnOutcome,
        conversation_id: &str,
    ) -> Option<CompactDecision> {
        let config = self.compaction.as_ref()?;
        let prompt_tokens = outcome.prompt_tokens?;
        let decision = compaction::evaluate_compaction(
            prompt_tokens,
            request.context_window_tokens,
            config.auto_compact,
            config.threshold_percent,
            request.options.max_tokens,
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

fn emit_delta(emitter: &TurnEmitter, delta: &ProviderDelta) {
    emitter.delta(delta);
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

fn apply_usage(outcome: &mut TurnOutcome, stream_outcome: &StreamOutcome) {
    outcome.prompt_tokens = stream_outcome.prompt_tokens;
    outcome.completion_tokens = stream_outcome.completion_tokens;
    outcome.cache_read_tokens = stream_outcome.cache_read_tokens;
    outcome.total_tokens = stream_outcome.total_tokens;
    if outcome.ttft_ms.is_none() {
        outcome.ttft_ms = stream_outcome.ttft_ms;
    }
}

fn apply_partial_usage(outcome: &mut TurnOutcome, partial: &PartialOutcome) {
    let usage = partial.last_usage();
    outcome.prompt_tokens = usage.prompt_tokens;
    outcome.completion_tokens = usage.completion_tokens;
    outcome.cache_read_tokens = usage.cache_read_tokens;
    outcome.total_tokens = usage.total_tokens;
    outcome.reasoning = partial.full_reasoning.clone();
    outcome.attempt_usages = partial.attempt_usages.clone();
    if outcome.ttft_ms.is_none() {
        outcome.ttft_ms = partial.ttft_ms;
    }
}

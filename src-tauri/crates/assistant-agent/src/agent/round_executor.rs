use std::time::Instant;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::kernel::{
    Clock, EventSink, MessageStore, ToolCatalog, TurnError, TurnInputSource, TurnLimits, TurnPhase,
    TurnSpec, TurnStatus,
};

use super::tool_call::FinalizedToolCall;
use super::tool_event::ToolEvent;

#[async_trait]
pub trait RoundExecutor: Send + Sync {
    async fn run_iterations(&self, ctx: TurnRunCtx<'_>) -> RoundOutcome;
}

#[derive(Debug, Clone)]
pub struct RoundOutcome {
    pub status: TurnStatus,
    pub text: String,
    pub iterations: u32,
    pub model_calls: u32,
    pub tool_calls: u32,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub cache_read_tokens: Option<u32>,
    pub confirmation_wait_ms: u64,
    pub hit_cap: bool,
    pub wrap_up_injected: bool,
    pub error: Option<TurnError>,
}

impl Default for RoundOutcome {
    fn default() -> Self {
        Self {
            status: TurnStatus::Completed,
            text: String::new(),
            iterations: 0,
            model_calls: 0,
            tool_calls: 0,
            prompt_tokens: None,
            completion_tokens: None,
            cache_read_tokens: None,
            confirmation_wait_ms: 0,
            hit_cap: false,
            wrap_up_injected: false,
            error: None,
        }
    }
}

pub trait TurnObserver: Send + Sync {
    fn set_phase(&self, phase: TurnPhase);

    fn emit_tool_event(&self, event: ToolEvent);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnBudget {
    pub remaining_tool_calls: u32,
    pub turn_deadline: Option<Instant>,
}

pub struct TurnRunCtx<'a> {
    pub turn_id: &'a str,
    pub conversation_id: &'a str,
    pub iteration: u32,
    pub remaining_turn_budget: TurnBudget,
    pub child_token: &'a CancellationToken,
    pub tools: Option<&'a dyn ToolCatalog>,
    pub tool_exec: Option<&'a dyn ToolExecutionPort>,
    pub sink: &'a dyn EventSink,
    pub store: &'a dyn MessageStore,
    pub input: &'a dyn TurnInputSource,
    pub clock: &'a dyn Clock,
    pub limits: &'a TurnLimits,
    pub spec: &'a TurnSpec,
    pub agent_id: &'a str,
    pub parent_turn_id: Option<&'a str>,
    pub delegation_depth: u32,
}

#[async_trait]
pub trait ToolExecutionPort: Send + Sync {
    async fn execute(
        &self,
        calls: Vec<FinalizedToolCall>,
        ctx: &ToolExecCtx<'_>,
    ) -> Vec<ToolExecResult>;
}

pub struct ToolExecCtx<'a> {
    pub turn_id: &'a str,
    pub conversation_id: &'a str,
    pub iteration: u32,
    pub child_token: &'a CancellationToken,
    pub tools: Option<&'a dyn ToolCatalog>,
    pub clock: &'a dyn Clock,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolExecResult {
    pub call_id: String,
    pub tool_name: String,
    pub content: String,
    pub is_error: bool,
    pub stage: ToolStage,
    pub error_kind: Option<ToolErrorClass>,
    pub confirmation_wait_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolStage {
    Find,
    Validate,
    Gateway,
    Execute,
    Persist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolErrorClass {
    Business,
    Timeout,
    Cancelled,
    Internal,
    Denied,
}

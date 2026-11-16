use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;

use super::context::ApprovalRequest;
use super::decision::{ApprovalDecision, ApprovalPrompter};

/// 脚本化审批替身：按脚本依次返回决策，脚本耗尽后回落到 `Deny`（fail-closed）。
pub struct ScriptedApprovalPrompter {
    decisions: Mutex<VecDeque<ApprovalDecision>>,
    requests: Mutex<Vec<ApprovalRequest>>,
    fallback: ApprovalDecision,
    delay: Option<Duration>,
}

impl ScriptedApprovalPrompter {
    pub fn new(decisions: Vec<ApprovalDecision>) -> Self {
        Self {
            decisions: Mutex::new(decisions.into()),
            requests: Mutex::new(Vec::new()),
            fallback: ApprovalDecision::Deny {
                reason: Some("script exhausted".to_string()),
            },
            delay: None,
        }
    }

    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    pub fn request_count(&self) -> usize {
        self.requests.lock().map(|r| r.len()).unwrap_or(0)
    }

    pub fn requests(&self) -> Vec<ApprovalRequest> {
        self.requests.lock().map(|r| r.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl ApprovalPrompter for ScriptedApprovalPrompter {
    async fn request(&self, request: ApprovalRequest) -> ApprovalDecision {
        if let Ok(mut requests) = self.requests.lock() {
            requests.push(request);
        }
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
        self.decisions
            .lock()
            .ok()
            .and_then(|mut decisions| decisions.pop_front())
            .unwrap_or_else(|| self.fallback.clone())
    }
}

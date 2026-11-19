use std::collections::HashSet;
use std::sync::Arc;

use assistant_agent::agent::round_executor::EscalationSink;
use assistant_providers::types::ToolSpec;
use assistant_tools::{
    EscalationCandidate, EscalationOutcome, EscalationRejectReason, EscalationRejection,
    RegisteredTool, ToolEscalationPort, ToolRegistry, ToolTier,
};
use async_trait::async_trait;

/// 本轮工具层级上限的来源（I58 §3.8 ③）：与 `WorkspaceRootSource` 同形态，
/// 按 `conversation_id` 每次求值。层级是会话级配置（模式 `maxToolTier`），
/// 不塞进 `TurnRunCtx`——循环内核不需要知道配置域（R21）。
pub type MaxToolTierSource = Arc<dyn Fn(&str) -> ToolTier + Send + Sync>;

/// 升级端口的生产实现：注册表 + 本轮层级 + 本轮收集器三样，不读其它状态。
///
/// 只做校验与登记（I58 §3.8 ①）：不改注册表、不改 provider 请求；
/// 通过项在此解析成完整 `ToolSpec` 并交给循环侧收集器，由循环尾部追加（§3.8 ④）。
pub struct RegistryEscalationPort {
    registry: Arc<ToolRegistry>,
    max_tier: ToolTier,
    sink: Arc<dyn EscalationSink>,
}

impl RegistryEscalationPort {
    pub fn new(
        registry: Arc<ToolRegistry>,
        max_tier: ToolTier,
        sink: Arc<dyn EscalationSink>,
    ) -> Self {
        Self {
            registry,
            max_tier,
            sink,
        }
    }

    /// 申请按 `id` 或 `name` 均可：先查模型可见名，再回落到内部 id。
    fn resolve(&self, key: &str) -> Option<RegisteredTool> {
        self.registry
            .find_by_name(key)
            .or_else(|| self.registry.find(key))
    }

    /// `brief` 档：工具未覆写 `brief()` 时回落到 `description()` 的首句（I58 §3.6）。
    async fn brief_of(&self, registered: &RegisteredTool) -> String {
        if let Some(brief) = registered.tool.brief() {
            return brief.to_string();
        }
        match registered.tool.description().await {
            Ok(description) => first_sentence(&description),
            Err(error) => {
                log::warn!(
                    "tool '{}' description unavailable for brief fallback: {}",
                    registered.id,
                    error
                );
                String::new()
            }
        }
    }

    fn spec_of(&self, registered: &RegisteredTool, description: String) -> ToolSpec {
        ToolSpec {
            name: registered.name.clone(),
            description,
            input_schema: registered.tool.input_schema(),
        }
    }
}

#[async_trait]
impl ToolEscalationPort for RegistryEscalationPort {
    async fn candidates(&self, available: &[String]) -> Vec<EscalationCandidate> {
        let available: HashSet<&str> = available.iter().map(String::as_str).collect();
        let mut ids = self.registry.tool_ids();
        ids.sort();

        let mut candidates = Vec::new();
        for id in ids {
            let Some(registered) = self.registry.find(&id) else {
                continue;
            };
            if !registered.tool.audience().agent
                || registered.tool.tier() > self.max_tier
                || available.contains(registered.name.as_str())
                || available.contains(registered.id.as_str())
            {
                continue;
            }
            let Some(base) = registered.tool.companion_of() else {
                continue;
            };
            if !available.contains(base) {
                continue;
            }
            candidates.push(EscalationCandidate {
                id: registered.id.clone(),
                name: registered.name.clone(),
                brief: self.brief_of(&registered).await,
            });
        }
        candidates
    }

    async fn request(&self, requested: &[String], available: &[String]) -> EscalationOutcome {
        let available: HashSet<&str> = available.iter().map(String::as_str).collect();
        let mut outcome = EscalationOutcome::default();

        for key in requested {
            let Some(registered) = self.resolve(key) else {
                outcome.rejected.push(EscalationRejection {
                    id: key.clone(),
                    reason: EscalationRejectReason::NotInCatalog,
                });
                continue;
            };

            let reject = if registered.tool.tier() > self.max_tier {
                Some(EscalationRejectReason::TierExceeded)
            } else {
                match registered.tool.companion_of() {
                    None => Some(EscalationRejectReason::NotCompanion),
                    Some(base) if !available.contains(base) => {
                        Some(EscalationRejectReason::BaseNotAvailable)
                    }
                    Some(_) => None,
                }
            };
            if let Some(reason) = reject {
                outcome.rejected.push(EscalationRejection {
                    id: registered.id.clone(),
                    reason,
                });
                continue;
            }

            let description = match registered.tool.description().await {
                Ok(description) => description,
                Err(error) => {
                    log::warn!(
                        "tool '{}' description unavailable, escalating with empty: {}",
                        registered.id,
                        error
                    );
                    String::new()
                }
            };
            self.sink.push(self.spec_of(&registered, description));
            outcome.enabled.push(registered.name.clone());
        }

        outcome
    }
}

/// `brief` 回落用的首句：到第一个句号 / 问号 / 感叹号 / 换行止，含标点。
fn first_sentence(description: &str) -> String {
    let trimmed = description.trim();
    let mut end = trimmed.len();
    for (index, ch) in trimmed.char_indices() {
        if matches!(ch, '.' | '?' | '!' | '\n' | '。' | '？' | '！') {
            end = index + ch.len_utf8();
            break;
        }
    }
    trimmed[..end].trim().to_string()
}

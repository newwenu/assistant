use async_trait::async_trait;
use serde_json::Value;

use crate::context::ToolUseContext;
use crate::escalation::{EscalationCandidate, EscalationOutcome, EscalationRejection};
use crate::os::footer;
use crate::r#trait::{BehaviorHints, Tool, ToolAudience, ToolTier, ValidationResult};
use crate::result::{ToolErrorKind, ToolResult};

/// 内部唯一标识（注册表键、白名单匹配、审计）。模型可见名默认与它同名。
const TOOL_ID: &str = "request_tool";

/// 单个工具手册的字符上限（I58 §3.1 G5）。
pub const MANUAL_MAX_CHARS: usize = 2000;
/// 一次返回的手册总字符上限（I58 §3.1 G5）。
pub const MANUAL_TOTAL_CHARS: usize = 8000;

/// 单份手册超出 [`MANUAL_MAX_CHARS`] 时的尾标记。
const MANUAL_TRUNCATED_MARKER: &str = "\n(manual truncated)";

/// 能力发现工具：返回本轮可用集清单与按需手册（I58 §3.1）。
///
/// 只读、无审批、**不改变本轮工具集**：它不触碰注册表、不触碰 provider 请求，
/// 只是把 `ToolUseContext` 里已有的两份事实（`available_tool_names` 与 `tool_catalog`）
/// 投影成模型可读的文本。
pub struct AgentRequestTool;

impl AgentRequestTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AgentRequestTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for AgentRequestTool {
    fn id(&self) -> &str {
        TOOL_ID
    }

    async fn description(&self) -> Result<String, String> {
        Ok(
            "List the tools available in this turn, fetch their manuals on demand, and enable \
             companion tools for the next iteration. `detail = \"list\"` (default) returns only \
             the tool names; `detail = \"manual\"` additionally returns the manual of every tool \
             whose id or name matches `query` (case-insensitive; omit `query` to get all \
             manuals). `detail = \"enable\"` registers `tools` for this turn: it does not change \
             this iteration's tool set, the tools become callable from the next tool call, and \
             only companion tools whose base tool is already available in this turn can be \
             enabled. Listing and manuals never add tools to the turn. When the turn's tool set \
             is unavailable the list falls back to the registry, and the reply says so \
             explicitly because the fallback may include tools that are not enabled in this \
             turn. Manuals are capped at 2000 characters per tool and 8000 characters in total."
                .to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "detail": {
                    "type": "string",
                    "enum": ["list", "manual", "enable"],
                    "description": "list = names only (default); manual = names + manuals; \
                                    enable = register `tools` for the next iteration"
                },
                "query": {
                    "type": "string",
                    "description": "Case-insensitive substring matched against tool id or name; \
                                    only used when detail = manual. Omit to return every manual"
                },
                "tools": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tool ids or names to enable for this turn; only used when \
                                    detail = enable. Requires a base tool that is already \
                                    available in this turn"
                }
            }
        })
    }

    fn tier(&self) -> ToolTier {
        ToolTier::Os
    }

    fn audience(&self) -> ToolAudience {
        ToolAudience::BOTH
    }

    fn behavior_hints(&self) -> BehaviorHints {
        BehaviorHints {
            read_only: true,
            destructive: false,
            open_world: false,
        }
    }

    fn validate_input(&self, input: &Value, _context: &ToolUseContext) -> ValidationResult {
        match Request::parse(input) {
            Ok(_) => ValidationResult::ok(),
            Err(error) => ValidationResult::invalid(error),
        }
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String> {
        let request = match Request::parse(input) {
            Ok(request) => request,
            Err(error) => return Ok(ToolResult::err(error)),
        };

        let Some(catalog) = context.tool_catalog.as_deref() else {
            return Ok(ToolResult::err(
                "tool catalog is unavailable: request_tool only works inside an agent turn",
            ));
        };
        let entries = catalog.entries().await;

        // 可用集：优先本轮真实工具集（G2），缺失时回退注册表并标注（G3）。
        let (listed, source) = match context.available_tool_names.as_deref() {
            Some(names) => (entries_for(names, &entries), Source::Turn),
            None => (
                entries
                    .iter()
                    .map(|entry| Listed {
                        id: entry.id.clone(),
                        name: entry.name.clone(),
                    })
                    .collect(),
                Source::Registry,
            ),
        };

        let mut text = header(&listed, source);
        for listed_tool in &listed {
            text.push_str(&format!("\n- {}", listed_tool.name));
        }

        // 可升级段（I58 §3.8）：只有「本轮工具集事实 + 升级端口」两者都在才算。
        // 端口缺失时不猜、不用 `ToolCatalog` 代算——否则会出现「清单里有、申请不了」的
        // 第二种事实来源（§3.8 ⑥）。
        let available = context.available_tool_names.as_deref();
        let escalatable: Vec<EscalationCandidate> = match (available, context.escalation.as_deref())
        {
            (Some(names), Some(port)) => port.candidates(names).await,
            _ => Vec::new(),
        };

        let manuals = match request.detail {
            Detail::List | Detail::Enable => 0,
            Detail::Manual => {
                let matched: Vec<&Listed> = listed
                    .iter()
                    .filter(|listed_tool| request.matches(listed_tool))
                    .collect();
                let mut shown = 0usize;
                let mut used = 0usize;
                for listed_tool in &matched {
                    let body = match entries.iter().find(|entry| entry.name == listed_tool.name) {
                        Some(entry) => clip_manual(&entry.manual),
                        None => String::new(),
                    };
                    let block = format!("\n\n## {}\n{}", listed_tool.name, body);
                    let block_chars = block.chars().count();
                    if used + block_chars > MANUAL_TOTAL_CHARS {
                        break;
                    }
                    used += block_chars;
                    text.push_str(&block);
                    shown += 1;
                }
                if shown < matched.len() {
                    text.push_str(&format!(
                        "\n\n{}",
                        footer::render(
                            footer::CATALOG,
                            format!("showing {} of {}", shown, matched.len())
                        )
                    ));
                }
                shown
            }
        };

        let mut data = serde_json::Map::new();
        data.insert("available".into(), Value::from(listed.len()));
        data.insert(
            "listed".into(),
            Value::from(
                listed
                    .iter()
                    .map(|listed_tool| listed_tool.name.clone())
                    .collect::<Vec<_>>(),
            ),
        );
        data.insert("manuals".into(), Value::from(manuals));
        data.insert("source".into(), Value::from(source.as_str()));

        if request.detail == Detail::Enable {
            return enable(
                &request,
                context.escalation.as_deref(),
                available,
                &escalatable,
                data,
                text,
            )
            .await;
        }

        insert_escalatable(&mut data, source, &escalatable);
        text.push_str(&escalation_section(&escalatable));
        Ok(ToolResult::ok_with_assistant(Value::Object(data), text))
    }
}

/// `detail = "enable"` 分支：端口只登记，工具集由循环下一迭代追加（I58 §3.8 ⑤）。
async fn enable(
    request: &Request,
    port: Option<&dyn crate::escalation::ToolEscalationPort>,
    available: Option<&[String]>,
    escalatable: &[EscalationCandidate],
    mut data: serde_json::Map<String, Value>,
    mut text: String,
) -> Result<ToolResult, String> {
    if request.tools.is_empty() {
        data.insert("enabled".into(), Value::from(Vec::<String>::new()));
        data.insert("rejected".into(), Value::from(Vec::<Value>::new()));
        // F13：仍要附可升级段，否则模型拿到错误却不知道该申请什么。
        insert_escalatable(&mut data, Source::Turn, escalatable);
        text.push_str(&escalation_section(escalatable));
        return Ok(ToolResult::err_with_data(
            ToolErrorKind::Business,
            "'detail = \"enable\"' requires a non-empty 'tools' list",
            Value::Object(data),
            text,
        ));
    }

    let Some(port) = port else {
        data.insert("enabled".into(), Value::from(Vec::<String>::new()));
        data.insert("rejected".into(), Value::from(Vec::<Value>::new()));
        // 端口缺失时可升级段恒为空（候选本身由端口算出），因此不插入。
        return Ok(ToolResult::err_with_data(
            ToolErrorKind::Business,
            "tool escalation is unavailable this turn: request_tool cannot enable tools here",
            Value::Object(data),
            text,
        ));
    };

    let outcome = port.request(&request.tools, available.unwrap_or(&[])).await;

    // 已生效的项不再属于可升级段：可升级段的判据是「不在本轮可用名」，
    // 而本轮可用名要等循环追加后才更新，故这里按名字做差。
    let remaining: Vec<EscalationCandidate> = escalatable
        .iter()
        .filter(|candidate| !outcome.enabled.contains(&candidate.name))
        .cloned()
        .collect();

    data.insert("enabled".into(), Value::from(outcome.enabled.clone()));
    data.insert("rejected".into(), Value::from(rejections_json(&outcome)));
    insert_escalatable(&mut data, Source::Turn, &remaining);

    text.push_str(&enabled_section(&outcome));
    text.push_str(&escalation_section(&remaining));

    if outcome.enabled.is_empty() {
        return Ok(ToolResult::err_with_data(
            ToolErrorKind::Business,
            "none of the requested tools could be enabled",
            Value::Object(data),
            text,
        ));
    }

    Ok(ToolResult::ok_with_assistant(Value::Object(data), text))
}

fn insert_escalatable(
    data: &mut serde_json::Map<String, Value>,
    source: Source,
    escalatable: &[EscalationCandidate],
) {
    if source == Source::Turn && !escalatable.is_empty() {
        data.insert(
            "escalatable".into(),
            Value::from(
                escalatable
                    .iter()
                    .map(|candidate| candidate.name.clone())
                    .collect::<Vec<_>>(),
            ),
        );
    }
}

fn escalation_section(escalatable: &[EscalationCandidate]) -> String {
    if escalatable.is_empty() {
        return String::new();
    }
    let mut section = String::from(
        "\n\nThese tools are not enabled this turn. You may call request_tool with \
         detail = \"enable\" and tools = [...] to enable them for the next iteration; \
         their base tool is already available.",
    );
    for candidate in escalatable {
        section.push_str(&format!("\n- {}: {}", candidate.name, candidate.brief));
    }
    section
}

fn enabled_section(outcome: &EscalationOutcome) -> String {
    let mut section = String::new();
    if !outcome.enabled.is_empty() {
        section.push_str(
            "\n\nEnabled for this turn (effective from the next tool call; the current \
             iteration's tool set is unchanged):",
        );
        for name in &outcome.enabled {
            section.push_str(&format!("\n- {}", name));
        }
    }
    if !outcome.rejected.is_empty() {
        section.push_str("\n\nNot enabled:");
        for rejection in &outcome.rejected {
            section.push_str(&format!(
                "\n- {} ({}): {}",
                rejection.id,
                rejection.reason.as_str(),
                rejection.reason.message()
            ));
        }
    }
    section
}

fn rejections_json(outcome: &EscalationOutcome) -> Vec<Value> {
    outcome.rejected.iter().map(rejections_json_one).collect()
}

fn rejections_json_one(rejection: &EscalationRejection) -> Value {
    serde_json::json!({
        "id": rejection.id,
        "reason": rejection.reason.as_str(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    /// 本轮真实工具集（`TurnRunCtx.tools` 透传）。
    Turn,
    /// 注册表回退：可能含本轮未启用的工具。
    Registry,
}

impl Source {
    fn as_str(self) -> &'static str {
        match self {
            Self::Turn => "turn",
            Self::Registry => "registry",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Listed {
    id: String,
    name: String,
}

/// 把本轮可用名映射回目录条目，并按 `id` 排序（与 `tool_definitions_for_tier` 同序）。
///
/// 名字不在目录里时仍保留条目：白名单与注册表短暂不一致（MCP 工具刚注册、
/// 工具被注销）时，模型至少能看到「这个名字本轮可用」，而不是被静默吞掉。
fn entries_for(names: &[String], entries: &[crate::catalog::CatalogEntry]) -> Vec<Listed> {
    let mut listed: Vec<Listed> = names
        .iter()
        .map(
            |name| match entries.iter().find(|entry| &entry.name == name) {
                Some(entry) => Listed {
                    id: entry.id.clone(),
                    name: entry.name.clone(),
                },
                None => Listed {
                    id: name.clone(),
                    name: name.clone(),
                },
            },
        )
        .collect();
    listed.sort_by(|left, right| left.id.cmp(&right.id));
    listed.dedup_by(|left, right| left.id == right.id && left.name == right.name);
    listed
}

fn header(listed: &[Listed], source: Source) -> String {
    let mut header = format!(
        "Available tools ({}, source: {})",
        listed.len(),
        source.as_str()
    );
    if source == Source::Registry {
        header.push_str(
            "\n(the turn's tool set is unavailable, so this list falls back to the registry; \
             it may include tools that are not enabled in this turn)",
        );
    }
    header
}

/// 单份手册裁到 [`MANUAL_MAX_CHARS`]（含尾标记），保证上限是硬约束。
fn clip_manual(text: &str) -> String {
    let chars = text.chars().count();
    if chars <= MANUAL_MAX_CHARS {
        return text.to_string();
    }
    let keep = MANUAL_MAX_CHARS - MANUAL_TRUNCATED_MARKER.chars().count();
    let clipped: String = text.chars().take(keep).collect();
    format!("{}{}", clipped, MANUAL_TRUNCATED_MARKER)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Detail {
    List,
    Manual,
    Enable,
}

#[derive(Debug)]
struct Request {
    detail: Detail,
    query: Option<String>,
    tools: Vec<String>,
}

impl Request {
    fn parse(input: &Value) -> Result<Self, String> {
        let object = input
            .as_object()
            .ok_or_else(|| "input must be a JSON object".to_string())?;

        let detail = match object.get("detail") {
            None | Some(Value::Null) => Detail::List,
            Some(Value::String(value)) => match value.as_str() {
                "list" => Detail::List,
                "manual" => Detail::Manual,
                "enable" => Detail::Enable,
                other => {
                    return Err(format!(
                        "'detail' must be \"list\", \"manual\" or \"enable\", got \"{}\"",
                        other
                    ))
                }
            },
            Some(_) => return Err("'detail' must be a string".to_string()),
        };

        let query = match object.get("query") {
            None | Some(Value::Null) => None,
            Some(Value::String(value)) => {
                let trimmed = value.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_string())
            }
            Some(_) => return Err("'query' must be a string".to_string()),
        };

        // 缺失 / 空数组不是解析错误：`enable` 的业务错误（F13）必须带上可升级段，
        // 而 `validate_input` 的失败没有 `data` 通道（I58 §3.8 ⑤）。
        let tools = match object.get("tools") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(items)) => {
                let mut tools = Vec::with_capacity(items.len());
                for item in items {
                    let Value::String(value) = item else {
                        return Err("'tools' must be an array of strings".to_string());
                    };
                    let trimmed = value.trim();
                    if !trimmed.is_empty() {
                        tools.push(trimmed.to_string());
                    }
                }
                tools
            }
            Some(_) => return Err("'tools' must be an array of strings".to_string()),
        };

        Ok(Self {
            detail,
            query,
            tools,
        })
    }

    /// `query` 对 `id` / `name` 做大小写不敏感的子串匹配；缺省即全匹配。
    fn matches(&self, listed: &Listed) -> bool {
        let Some(query) = self.query.as_deref() else {
            return true;
        };
        let query = query.to_lowercase();
        listed.id.to_lowercase().contains(&query) || listed.name.to_lowercase().contains(&query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn context_with(
        names: Option<Vec<&str>>,
        catalog: Option<std::sync::Arc<dyn crate::ToolCatalog>>,
    ) -> ToolUseContext {
        ToolUseContext {
            available_tool_names: names
                .map(|names| names.into_iter().map(str::to_string).collect()),
            tool_catalog: catalog,
            ..Default::default()
        }
    }

    #[test]
    fn parse_defaults_to_list() {
        let request = Request::parse(&json!({})).expect("empty input is valid");
        assert_eq!(request.detail, Detail::List);
        assert!(request.query.is_none());
    }

    #[test]
    fn parse_rejects_unknown_detail() {
        let error = Request::parse(&json!({ "detail": "all" })).expect_err("unknown detail");
        assert!(
            error.contains("list") && error.contains("manual"),
            "{error}"
        );
    }

    #[test]
    fn parse_rejects_non_string_query() {
        let error = Request::parse(&json!({ "query": 1 })).expect_err("non-string query");
        assert!(error.contains("query"), "{error}");
    }

    #[test]
    fn clip_manual_keeps_the_hard_cap() {
        let long: String = "x".repeat(MANUAL_MAX_CHARS + 100);
        let clipped = clip_manual(&long);
        assert_eq!(clipped.chars().count(), MANUAL_MAX_CHARS);
        assert!(clipped.ends_with(MANUAL_TRUNCATED_MARKER));
    }

    #[test]
    fn short_manual_is_returned_verbatim() {
        assert_eq!(clip_manual("short"), "short");
    }

    #[test]
    fn query_matches_id_or_name_case_insensitively() {
        let request = Request {
            detail: Detail::Manual,
            query: Some("READ".to_string()),
            tools: Vec::new(),
        };
        let listed = Listed {
            id: "read".to_string(),
            name: "read".to_string(),
        };
        assert!(request.matches(&listed));
    }

    #[test]
    fn missing_catalog_is_a_business_error() {
        let tool = AgentRequestTool::new();
        let context = context_with(Some(vec!["read"]), None);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");
        let result = runtime
            .block_on(tool.execute(&json!({}), &context))
            .expect("execute must not fail internally");
        assert!(!result.ok);
        assert!(result
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("catalog is unavailable"));
    }
}

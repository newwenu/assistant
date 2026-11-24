use chrono::Utc;
use tauri::{command, AppHandle, Manager};
use uuid::Uuid;

use crate::protocol::{AgentModeDto, AgentModeInputDto, ConversationToolConfigDto};
use assistant_storage::agent_modes::{self, AgentModeRow};

fn row_to_dto(row: &AgentModeRow) -> AgentModeDto {
    AgentModeDto::from(row)
}

/// 头像三态（I43 §3.5）：缺省 / `null` 沿用现值；`""` 清除为 `NULL`；非空覆盖。
fn resolve_avatar(input: Option<String>, existing: Option<&AgentModeRow>) -> Option<String> {
    match input {
        Some(raw) if raw.is_empty() => None,
        Some(raw) => Some(raw),
        None if existing.is_none() => None,
        None => existing.and_then(|m| m.avatar.clone()),
    }
}

#[command]
pub fn list_agent_modes(app: AppHandle) -> Result<Vec<AgentModeDto>, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let modes = agent_modes::get_modes(&conn)?;
    Ok(modes.iter().map(row_to_dto).collect())
}

#[command]
pub fn save_agent_mode(app: AppHandle, mode: AgentModeInputDto) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    let is_new = mode.id.is_empty();
    let now = Utc::now().timestamp_millis();

    // `kind` 决定已有会话的发送路径与布局，允许编辑会让同一会话静默换链路（I38d §4.8）。
    // 与既有行不一致即拒绝；换类型必须新建模式。
    let existing = if is_new {
        None
    } else {
        agent_modes::get_by_id(&conn, &mode.id)?
    };
    if let Some(row) = existing.as_ref() {
        if row.kind != mode.kind.as_str() {
            return Err(assistant_i18n::t("error.agent_mode_kind_locked"));
        }
    }

    let tool_config = match mode.tool_config {
        Some(cfg) => cfg,
        None if is_new => ConversationToolConfigDto::default(),
        None => existing
            .as_ref()
            .map(|m| AgentModeDto::from(m).tool_config)
            .unwrap_or_default(),
    };
    // 规则在落库前校验（I70 §6.2 / A11）：失败即拒绝保存，不留半份写入。
    crate::adapters::validate_tool_config_rules(&tool_config)?;
    let tool_config_json = Some(serde_json::to_string(&tool_config).map_err(|e| e.to_string())?);

    // 头像三态（I43 §3.5）：缺省 / `null` 沿用现值；`""` 清除为 `NULL`；非空覆盖。
    let avatar = resolve_avatar(mode.avatar, existing.as_ref());

    let row = AgentModeRow {
        id: if is_new {
            Uuid::new_v4().to_string()
        } else {
            mode.id.clone()
        },
        name: mode.name,
        description: mode.description,
        kind: mode.kind.as_str().to_string(),
        prompt: mode.prompt,
        model: mode.model,
        temperature: mode.temperature,
        max_tokens: mode.max_tokens,
        reasoning_enabled: mode.reasoning_enabled,
        reasoning_effort: mode.reasoning_effort,
        presentation_rank: mode.presentation_rank,
        is_builtin: mode.is_builtin,
        created_at: mode.created_at,
        updated_at: now,
        tool_config_json,
        avatar,
    };

    if is_new {
        agent_modes::insert_mode(&conn, &row)?;
    } else {
        agent_modes::update_mode(&conn, &row)?;
    }
    Ok(())
}

#[command]
pub fn delete_agent_mode(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    agent_modes::delete_mode(&conn, &id).map_err(super::translate_storage_error)
}

#[command]
pub fn reset_builtin_mode(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    agent_modes::reset_builtin_mode(&conn, &id, &|key| assistant_i18n::t(key))
        .map_err(super::translate_storage_error)
}

#[cfg(test)]
mod tests {
    use super::resolve_avatar;
    use assistant_storage::agent_modes::AgentModeRow;

    fn existing(avatar: Option<&str>) -> AgentModeRow {
        AgentModeRow {
            avatar: avatar.map(|v| v.to_string()),
            ..AgentModeRow::default()
        }
    }

    #[test]
    fn new_mode_without_avatar_stays_null() {
        assert_eq!(resolve_avatar(None, None), None);
    }

    #[test]
    fn new_mode_with_avatar_persists_value() {
        assert_eq!(
            resolve_avatar(Some("builtin:agent".to_string()), None),
            Some("builtin:agent".to_string())
        );
    }

    #[test]
    fn missing_avatar_keeps_existing_value() {
        let row = existing(Some("builtin:translate"));
        assert_eq!(
            resolve_avatar(None, Some(&row)),
            Some("builtin:translate".to_string())
        );
    }

    #[test]
    fn empty_avatar_clears_existing_value() {
        let row = existing(Some("builtin:translate"));
        assert_eq!(resolve_avatar(Some(String::new()), Some(&row)), None);
    }

    #[test]
    fn non_empty_avatar_overrides_existing_value() {
        let row = existing(Some("builtin:chat"));
        assert_eq!(
            resolve_avatar(Some("builtin:agent".to_string()), Some(&row)),
            Some("builtin:agent".to_string())
        );
    }

    #[test]
    fn a11_over_limit_or_over_length_rules_are_rejected_before_persisting() {
        fn config(
            rules: Vec<crate::protocol::ApprovalRuleDto>,
        ) -> crate::protocol::ConversationToolConfigDto {
            crate::protocol::ConversationToolConfigDto {
                max_tool_tier: crate::protocol::ToolTierDto::Sandbox,
                approval_level: crate::protocol::ApprovalLevelDto::None,
                enabled_tools: Vec::new(),
                disabled_tools: Vec::new(),
                approval_rules: rules,
            }
        }

        fn rule(
            action: String,
            resource: String,
            effect: crate::protocol::ApprovalRuleEffectDto,
        ) -> crate::protocol::ApprovalRuleDto {
            crate::protocol::ApprovalRuleDto {
                action,
                resource,
                effect,
            }
        }

        // `save_agent_mode` 在 `serde_json::to_string` 与 `insert/update` 之前调用本校验：
        // 返回 Err 时命令立即退出，不存在部分写入（A11）。
        let over_limit = config(vec![
            rule(
                "read".to_string(),
                "*".to_string(),
                crate::protocol::ApprovalRuleEffectDto::Allow,
            );
            65
        ]);
        assert!(crate::adapters::validate_tool_config_rules(&over_limit).is_err());

        let long_action = config(vec![rule(
            "a".repeat(33),
            "*".to_string(),
            crate::protocol::ApprovalRuleEffectDto::Ask,
        )]);
        assert!(crate::adapters::validate_tool_config_rules(&long_action).is_err());

        let long_resource = config(vec![rule(
            "read".to_string(),
            "r".repeat(257),
            crate::protocol::ApprovalRuleEffectDto::Ask,
        )]);
        assert!(crate::adapters::validate_tool_config_rules(&long_resource).is_err());

        // 边界值通过：64 条、action 32 字符、resource 256 字符。
        let boundary = config(vec![
            rule(
                "read".to_string(),
                "*".to_string(),
                crate::protocol::ApprovalRuleEffectDto::Allow,
            );
            64
        ]);
        assert!(crate::adapters::validate_tool_config_rules(&boundary).is_ok());
        let max_lengths = config(vec![rule(
            "a".repeat(32),
            "r".repeat(256),
            crate::protocol::ApprovalRuleEffectDto::Ask,
        )]);
        assert!(crate::adapters::validate_tool_config_rules(&max_lengths).is_ok());
    }
}

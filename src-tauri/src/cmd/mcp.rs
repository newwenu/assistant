use std::sync::Arc;

use chrono::Utc;
use tauri::{command, AppHandle, Manager};

use assistant_storage::mcp_servers::{self, McpLaunchConfig, McpServerRow};
use assistant_tools::mcp::{McpServerManager, McpServerSnapshot, McpServerStatus};

use crate::mcp::config::{
    derive_server_id, is_valid_server_id, open_launch_config, runtime_config, seal_launch_config,
    validate_launch_config,
};
use crate::protocol::{McpLaunchInputDto, McpServerCreateDto, McpServerDto, McpServerUpdateDto};

fn launch_config(dto: &McpLaunchInputDto) -> McpLaunchConfig {
    McpLaunchConfig {
        command: dto.command.trim().to_string(),
        args: dto.args.clone(),
        cwd: dto
            .cwd
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        env: dto.env.clone(),
        call_timeout_ms: dto.call_timeout_ms,
    }
}

fn manager(app: &AppHandle) -> Result<Arc<McpServerManager>, String> {
    crate::boot::mcp_manager(app).map_err(|e| e.to_string())
}

/// 行 → DTO。解密失败不隐藏该行：`config_error` 非空即说明「必须重填配置才能启动」。
fn server_dto(row: &McpServerRow, snapshot: Option<McpServerSnapshot>) -> McpServerDto {
    let (launch, config_error) = match open_launch_config(row) {
        Ok(launch) => (Some(launch), None),
        Err(e) => (None, Some(e)),
    };
    let status = snapshot
        .as_ref()
        .map(|s| s.status)
        .unwrap_or(McpServerStatus::Stopped);
    McpServerDto {
        id: row.id.clone(),
        name: row.name.clone(),
        transport: row.transport.clone(),
        enabled: row.enabled,
        status: status.into(),
        last_error: snapshot
            .and_then(|s| s.last_error)
            .or_else(|| config_error.clone()),
        command: launch
            .as_ref()
            .map(|l| l.command.clone())
            .unwrap_or_default(),
        args: launch.as_ref().map(|l| l.args.clone()).unwrap_or_default(),
        cwd: launch.as_ref().and_then(|l| l.cwd.clone()),
        env_keys: launch
            .as_ref()
            .map(|l| l.env.keys().cloned().collect())
            .unwrap_or_default(),
        call_timeout_ms: launch.as_ref().and_then(|l| l.call_timeout_ms),
        config_error,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn load_rows(app: &AppHandle) -> Result<Vec<McpServerRow>, String> {
    let state = app.state::<crate::db_state::DbState>();
    let conn = super::get_db_conn(&state)?;
    mcp_servers::list_servers(&conn)
}

#[command]
pub async fn list_mcp_servers(app: AppHandle) -> Result<Vec<McpServerDto>, String> {
    let rows = load_rows(&app)?;
    let snapshots = manager(&app)?.snapshots().await;
    Ok(rows
        .iter()
        .map(|row| {
            let snapshot = snapshots.iter().find(|s| s.id == row.id).cloned();
            server_dto(row, snapshot)
        })
        .collect())
}

/// 新建：`id` 由展示名派生且创建后只读；默认 `enabled = 0`（启用是显式动作）。
#[command]
pub async fn create_mcp_server(
    app: AppHandle,
    input: McpServerCreateDto,
) -> Result<McpServerDto, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("name is required".to_string());
    }
    let launch = launch_config(&input.launch);
    validate_launch_config(&launch)?;
    let (config_encrypted, config_hash) = seal_launch_config(&launch)?;

    let now = Utc::now().timestamp_millis();
    let row = {
        let state = app.state::<crate::db_state::DbState>();
        let conn = super::get_db_conn(&state)?;
        let mut id = derive_server_id(&name);
        if !is_valid_server_id(&id) {
            id = format!("server-{}", crate::mcp::config::short_suffix());
        }
        // 重名只影响 id（工具名与授权键都用它），因此追加短后缀而不是拒绝创建。
        while mcp_servers::get_server(&conn, &id)?.is_some() {
            let suffix = crate::mcp::config::short_suffix();
            let keep = crate::mcp::config::SERVER_ID_MAX_LEN - suffix.len() - 1;
            let base: String = id.chars().take(keep).collect();
            id = format!("{base}-{suffix}");
        }
        let row = McpServerRow {
            id,
            name,
            transport: "stdio".to_string(),
            config_encrypted,
            config_hash,
            enabled: false,
            created_at: now,
            updated_at: now,
        };
        mcp_servers::insert_server(&conn, &row)?;
        row
    };
    Ok(server_dto(&row, manager(&app)?.snapshot(&row.id).await))
}

/// 更新：`id` 只读；`launch` 缺省表示只改展示名（不重建配置、不换授权键）。
///
/// 配置变更走「停旧 → 注销工具 → 按新配置重启」，因此只影响下一 turn 的工具清单
/// （规格 §6.3 第 4 条）。
#[command]
pub async fn update_mcp_server(
    app: AppHandle,
    input: McpServerUpdateDto,
) -> Result<McpServerDto, String> {
    let now = Utc::now().timestamp_millis();
    let (row, launch_changed) = {
        let state = app.state::<crate::db_state::DbState>();
        let conn = super::get_db_conn(&state)?;
        let mut row = mcp_servers::get_server(&conn, &input.id)?
            .ok_or_else(|| format!("mcp server '{}' not found", input.id))?;

        if let Some(name) = input.name.as_deref() {
            let name = name.trim();
            if name.is_empty() {
                return Err("name is required".to_string());
            }
            row.name = name.to_string();
        }
        if let Some(launch) = input.launch.as_ref() {
            let launch = launch_config(launch);
            validate_launch_config(&launch)?;
            let (config_encrypted, config_hash) = seal_launch_config(&launch)?;
            row.config_encrypted = config_encrypted;
            row.config_hash = config_hash;
        }
        row.updated_at = now;
        mcp_servers::update_server(&conn, &row)?;
        (row, input.launch.is_some())
    };

    let manager = manager(&app)?;
    if launch_changed && row.enabled {
        match runtime_config(&row) {
            Ok(config) => {
                let status = manager.enable(config).await;
                log::info!("mcp server '{}' restarted: {status}", row.id);
            }
            Err(e) => {
                log::warn!("mcp server '{}' config unusable: {e}", row.id);
                manager.disable(&row.id).await;
                manager.mark_config_error(&row.id, &row.name, e).await;
            }
        }
    }
    Ok(server_dto(&row, manager.snapshot(&row.id).await))
}

/// 删除：先禁用（注销工具 + 杀进程）再删行，避免残留工具指向已消失的 server。
#[command]
pub async fn delete_mcp_server(app: AppHandle, id: String) -> Result<(), String> {
    let manager = manager(&app)?;
    manager.disable(&id).await;
    {
        let state = app.state::<crate::db_state::DbState>();
        let conn = super::get_db_conn(&state)?;
        mcp_servers::delete_server(&conn, &id)?;
    }
    // 运行时条目也要清掉，否则列表里会留下无对应行的 Stopped 条目。
    manager.forget(&id).await;
    Ok(())
}

/// 启停：`enabled` 持久化，随后立即执行启停；失败只记 `last_error`，不回滚开关。
#[command]
pub async fn set_mcp_server_enabled(
    app: AppHandle,
    id: String,
    enabled: bool,
) -> Result<McpServerDto, String> {
    let now = Utc::now().timestamp_millis();
    let row = {
        let state = app.state::<crate::db_state::DbState>();
        let conn = super::get_db_conn(&state)?;
        if mcp_servers::get_server(&conn, &id)?.is_none() {
            return Err(format!("mcp server '{id}' not found"));
        }
        mcp_servers::set_server_enabled(&conn, &id, enabled, now)?;
        mcp_servers::get_server(&conn, &id)?
            .ok_or_else(|| format!("mcp server '{id}' not found"))?
    };

    let manager = manager(&app)?;
    if enabled {
        match runtime_config(&row) {
            Ok(config) => {
                let status = manager.enable(config).await;
                log::info!("mcp server '{}' enabled: {status}", row.id);
            }
            Err(e) => {
                log::warn!("mcp server '{}' config unusable: {e}", row.id);
                manager.mark_config_error(&row.id, &row.name, e).await;
            }
        }
    } else {
        manager.disable(&id).await;
    }
    Ok(server_dto(&row, manager.snapshot(&row.id).await))
}

//! 配置行与运行时投影（P7-C4）：加解密、合法性校验与启动序列。
//!
//! 明文只在这一层出现：加密/解密只在宿主侧完成，`assistant-tools` 不感知。
//! 日志只写 server id 与失败原因，绝不写密文、明文 `args` / `env` 值。

use std::time::Duration;

use assistant_crypto::api_key as crypto;
use assistant_storage::mcp_servers::{self, McpLaunchConfig, McpServerRow};
use assistant_tools::mcp::McpServerConfig;
use tauri::{AppHandle, Manager};

/// `server_id` 同时进入模型工具名与授权键，因此只接受 slug（I38e §3.1 / §3.5）。
pub const SERVER_ID_MAX_LEN: usize = 32;

pub fn is_valid_server_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= SERVER_ID_MAX_LEN
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// 新建 server 的 id 由展示名派生：非字母数字转 `_`，全空或冲突时追加短随机后缀。
pub fn derive_server_id(name: &str) -> String {
    let normalized: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = normalized.trim_matches('_');
    if trimmed.is_empty() {
        format!("server-{}", short_suffix())
    } else {
        trimmed.chars().take(SERVER_ID_MAX_LEN).collect()
    }
}

pub fn short_suffix() -> String {
    uuid::Uuid::new_v4().to_string().chars().take(8).collect()
}

/// 启动配置合法性：只挡「一定跑不起来」的输入，不猜测 server 自己的参数语义。
pub fn validate_launch_config(launch: &McpLaunchConfig) -> Result<(), String> {
    if launch.command.trim().is_empty() {
        return Err("command is required".to_string());
    }
    if launch.command.contains('\0') || launch.command.contains('\n') {
        return Err("command contains illegal characters".to_string());
    }
    for arg in &launch.args {
        if arg.contains('\0') {
            return Err("args contain illegal characters".to_string());
        }
    }
    if let Some(cwd) = launch.cwd.as_deref() {
        if !cwd.is_empty() && !std::path::Path::new(cwd).is_absolute() {
            return Err("cwd must be an absolute path".to_string());
        }
    }
    for (key, value) in &launch.env {
        if key.is_empty() || key.contains('=') || key.contains('\0') {
            return Err("env keys must be non-empty and must not contain '='".to_string());
        }
        if value.contains('\0') {
            return Err(format!("env value for '{key}' contains illegal characters"));
        }
    }
    if let Some(ms) = launch.call_timeout_ms {
        if ms == 0 {
            return Err("callTimeoutMs must be positive".to_string());
        }
    }
    Ok(())
}

/// 明文规范 JSON 的密文（base64）与哈希：两者必须由同一次明文计算得出。
pub fn seal_launch_config(launch: &McpLaunchConfig) -> Result<(String, String), String> {
    let hash = launch.config_hash()?;
    let machine_id = crypto::get_machine_id()?;
    let encrypted = crypto::encrypt_api_key_b64(&launch.canonical_json()?, &machine_id)?;
    Ok((encrypted, hash))
}

pub fn open_launch_config(row: &McpServerRow) -> Result<McpLaunchConfig, String> {
    let machine_id = crypto::get_machine_id()?;
    let plain = crypto::decrypt_api_key_b64(&row.config_encrypted, &machine_id)?;
    serde_json::from_str(&plain).map_err(|e| format!("invalid launch config: {e}"))
}

/// 行 → 管理器入参。哈希按当前明文重算：DB 中的 `config_hash` 只是索引值，
/// 不一致说明行被外部改过，以明文为准并留 warn（授权键随之失效）。
pub fn runtime_config(row: &McpServerRow) -> Result<McpServerConfig, String> {
    let launch = open_launch_config(row)?;
    let hash = launch.config_hash()?;
    if hash != row.config_hash {
        log::warn!(
            "mcp server '{}': stored config hash mismatch, using recomputed hash",
            row.id
        );
    }
    Ok(McpServerConfig {
        id: row.id.clone(),
        name: row.name.clone(),
        command: launch.command,
        args: launch.args,
        cwd: launch.cwd,
        env: launch.env,
        config_hash: hash,
        call_timeout: launch.call_timeout_ms.map(Duration::from_millis),
    })
}

/// 启动序列（规格 §4.2 第 1 条）：只拉起 `enabled = 1` 的 server。
///
/// 每个 server 独立 task，失败只记日志、不阻塞应用启动；解密失败同样只记日志。
pub fn start_enabled_servers(app: &AppHandle) {
    // 能力门禁：`tool_runtime` 未装配时不隐式装配，拉起留给下一次 `ensure`。
    let manager = match crate::boot::mcp_manager(app) {
        Ok(manager) => manager,
        Err(e) => {
            log::warn!("mcp startup skipped: {e}");
            return;
        }
    };
    let rows = {
        let state = app.state::<crate::db_state::DbState>();
        let loaded = match state.0.lock() {
            Ok(conn) => mcp_servers::list_enabled_servers(&conn),
            Err(e) => {
                log::warn!("failed to lock db for mcp startup: {e}");
                return;
            }
        };
        match loaded {
            Ok(rows) => rows,
            Err(e) => {
                log::warn!("failed to load mcp servers: {e}");
                return;
            }
        }
    };

    for row in rows {
        let manager = manager.clone();
        tauri::async_runtime::spawn(async move {
            match runtime_config(&row) {
                Ok(config) => {
                    let status = manager.enable(config).await;
                    log::info!("mcp server '{}' startup: {status}", row.id);
                }
                Err(e) => {
                    log::warn!("mcp server '{}' not started: {e}", row.id);
                    manager.mark_config_error(&row.id, &row.name, e).await;
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn launch(command: &str) -> McpLaunchConfig {
        McpLaunchConfig {
            command: command.to_string(),
            args: vec!["--stdio".to_string()],
            cwd: None,
            env: BTreeMap::new(),
            call_timeout_ms: Some(30_000),
        }
    }

    fn row(launch: &McpLaunchConfig) -> McpServerRow {
        let (config_encrypted, config_hash) = seal_launch_config(launch).expect("seal");
        McpServerRow {
            id: "demo".to_string(),
            name: "Demo".to_string(),
            transport: "stdio".to_string(),
            config_encrypted,
            config_hash,
            enabled: true,
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn derive_id_is_slug_and_bounded() {
        assert_eq!(derive_server_id("Filesystem Server"), "filesystem_server");
        assert_eq!(derive_server_id("  @@@ ").len(), "server-".len() + 8);
        let long = derive_server_id(&"a".repeat(64));
        assert_eq!(long.len(), SERVER_ID_MAX_LEN);
        assert!(is_valid_server_id(&long));
        assert!(!is_valid_server_id("has space"));
    }

    #[test]
    fn validate_rejects_inputs_that_cannot_spawn() {
        assert!(validate_launch_config(&launch("npx")).is_ok());
        assert!(validate_launch_config(&launch("  ")).is_err());

        let mut relative = launch("npx");
        relative.cwd = Some("sub/dir".to_string());
        assert!(validate_launch_config(&relative).is_err());

        let mut bad_env = launch("npx");
        bad_env.env.insert("A=B".to_string(), "v".to_string());
        assert!(validate_launch_config(&bad_env).is_err());

        let mut zero_timeout = launch("npx");
        zero_timeout.call_timeout_ms = Some(0);
        assert!(validate_launch_config(&zero_timeout).is_err());
    }

    #[test]
    fn seal_hides_plaintext_and_roundtrips() {
        let mut launch = launch("npx");
        launch
            .env
            .insert("TOKEN".to_string(), "s3cr3t-value".to_string());
        let row = row(&launch);
        assert!(!row.config_encrypted.contains("s3cr3t-value"));
        assert!(!row.config_encrypted.contains("npx"));

        let opened = open_launch_config(&row).expect("open");
        assert_eq!(opened, launch);

        let runtime = runtime_config(&row).expect("runtime");
        assert_eq!(runtime.command, "npx");
        assert_eq!(
            runtime.env.get("TOKEN").map(String::as_str),
            Some("s3cr3t-value")
        );
        assert_eq!(runtime.call_timeout, Some(Duration::from_millis(30_000)));
        assert_eq!(runtime.config_hash, row.config_hash);
    }

    #[test]
    fn corrupted_ciphertext_is_reported_as_config_error() {
        let mut row = row(&launch("npx"));
        row.config_encrypted.push_str("tampered");
        assert!(open_launch_config(&row).is_err());
    }
}

// 参考: feature-mount-traversal-spec.md（已归档） §3 — vault 走插件边界，可选性只体现一次

use tauri::plugin::TauriPlugin;

#[cfg(feature = "vault")]
mod mounted {
    use super::TauriPlugin;

    use tauri::Manager;

    use crate::app_dirs::registry;
    use crate::app_dirs::AppDirs;
    use crate::cmd::storage::probe::{
        BackupPruneReport, ProbeError, ProbeRegistry, ProbeStats, StorageOwnerProbe,
        StorageOwnerProbeHandle,
    };
    use crate::cmd::vault;
    use assistant_vault::security_db::{SecurityDbState, MAX_BACKUPS};
    use assistant_vault::storage_stats::DirStats;

    /// `E7`–`E11` 的 vault 侧 provider：只调 owner 的公开观测入口，不拼路径、不开库。
    ///
    /// 持 `AppHandle` 而非 `SecurityDbState`：后者不实现 `Clone`，且在 `spawn_blocking`
    /// 里需要 `'static` 句柄；`try_state` 取不到即降级，绝不让管理器 panic。
    struct VaultStorageProbe {
        app: tauri::AppHandle,
    }

    fn stats_of(stats: DirStats) -> ProbeStats {
        ProbeStats {
            bytes: Some(stats.bytes),
            count: Some(stats.count),
            oldest_at: stats.oldest_at,
            newest_at: stats.newest_at,
        }
    }

    /// 单文件 `metadata` 失败归 `Unavailable`（文件正在替换 / 迁移，§7.1 特例 2）；
    /// 文件缺失已由 owner 映射成全零，不会走到这里。
    fn map_file_io(_error: std::io::Error) -> ProbeError {
        ProbeError::Unavailable
    }

    /// 目录遍历失败归 `Unreadable`（权限 / IO 失败）。
    fn map_io(_error: std::io::Error) -> ProbeError {
        ProbeError::Unreadable
    }

    impl VaultStorageProbe {
        /// `SecurityDbState` 在 `vault_setup` 内 `manage`；取不到说明装配异常，
        /// 返回 `Err` 让管理器按 `unreadable` 降级，绝不 panic（§3.10 / §7.1）。
        fn db_state(&self) -> Result<tauri::State<'_, SecurityDbState>, ProbeError> {
            self.app
                .try_state::<SecurityDbState>()
                .ok_or(ProbeError::Unreadable)
        }

        fn dirs(&self) -> Result<AppDirs, ProbeError> {
            AppDirs::resolve().map_err(|_| ProbeError::Unavailable)
        }
    }

    impl StorageOwnerProbe for VaultStorageProbe {
        fn capability(&self) -> &'static str {
            registry::SECURITY_OBSERVABILITY
        }

        /// 按登记表 `key` 分派（`E7`–`E11`）；其余 key 一律 `None`（不归本 provider）。
        ///
        /// 这里的 key 字面量必须与 `registry::MANAGED_ROOTS` 一致，V22 单测逐行校验
        /// 「`Feature` 行的 `stats(key)` 必须 `Some`」，改登记表 key 会直接判红。
        fn stats(&self, key: &'static str) -> Option<Result<ProbeStats, ProbeError>> {
            match key {
                "security/security.db" => Some(
                    self.db_state()
                        .and_then(|state| state.db_file_stats().map_err(map_file_io))
                        .map(stats_of),
                ),
                "security/backups" => Some(
                    self.db_state()
                        .and_then(|state| state.backup_stats().map_err(map_io))
                        .map(stats_of),
                ),
                "security/vault" => Some(
                    self.db_state()
                        .and_then(|state| {
                            // 锁定态下仓库内容不可信：先判锁，再观测（§3.10 的 `locked` 语义）。
                            if state.is_locked() {
                                return Err(ProbeError::Locked);
                            }
                            vault::repo_stats(&self.dirs()?).map_err(map_io)
                        })
                        .map(stats_of),
                ),
                "security/recovery" => Some(
                    self.dirs()
                        .and_then(|dirs| vault::recovery_stats(&dirs).map_err(map_io))
                        .map(stats_of),
                ),
                "security/audit" => Some(
                    self.db_state()
                        .and_then(|state| state.audit_stats().map_err(map_io))
                        .map(stats_of),
                ),
                _ => None,
            }
        }

        fn audit_policy(&self) -> Option<(u32, i64)> {
            self.app
                .try_state::<SecurityDbState>()
                .and_then(|state| state.audit_policy())
        }

        /// `E8` 的保留份数：直接自述 owner 常量，管理器不持有任何字面量（G3）。
        ///
        /// 取不到 `SecurityDbState` 不影响本值——它是常量，不是运行时状态。
        fn max_backups(&self) -> Option<u32> {
            u32::try_from(MAX_BACKUPS).ok()
        }

        /// `E8`：重新计数与裁剪都在 owner 侧一次完成（管理器只传 `keep`），
        /// 备份目录不在 `SecurityDbState` 之外拼接，解锁状态不影响本入口。
        fn prune_backups(&self, keep: u32) -> Option<Result<BackupPruneReport, ProbeError>> {
            let state = match self.db_state() {
                Ok(state) => state,
                Err(error) => return Some(Err(error)),
            };
            let db_path = std::path::Path::new(state.db_path()).to_path_buf();
            Some(
                SecurityDbState::prune_backups_to(&db_path, keep as usize)
                    .map(|report| BackupPruneReport {
                        removed: report.removed,
                        removed_bytes: report.removed_bytes,
                        failed: report.failed,
                    })
                    .map_err(|_| ProbeError::Unreadable),
            )
        }
    }

    pub(crate) fn init() -> TauriPlugin<tauri::Wry> {
        crate::cmd::register_conf_applier(
            "vault",
            crate::cmd::vault::apply_conf as crate::cmd::ConfApplier,
        );
        tauri::plugin::Builder::<tauri::Wry>::new("vault")
            .setup(|app, _api| {
                #[cfg(target_os = "windows")]
                if let Err(error) = assistant_vault::biometric::warm_up_keyring_backend() {
                    log::warn!("Windows Credential Manager backend unavailable: {}", error);
                }
                crate::cmd::vault::vault_setup(app)?;
                // 注册到通用槽（G18(b)）：槽由 `plugins::attach` 在 `Builder` 阶段建好。
                // 取不到槽 = 装配异常：记错误并继续，不让插件 setup 失败（不 panic、不覆盖）。
                let probe: StorageOwnerProbeHandle = std::sync::Arc::new(VaultStorageProbe {
                    app: app.app_handle().clone(),
                });
                match app.try_state::<std::sync::Arc<ProbeRegistry>>() {
                    Some(registry) => {
                        if let Err(error) = registry.register(probe) {
                            log::error!("storage probe 注册失败（E7–E11 将降级）: {}", error);
                        }
                    }
                    None => log::error!(
                        "ProbeRegistry 槽不存在（E7–E11 将永久降级）: {}",
                        registry::SECURITY_OBSERVABILITY
                    ),
                }
                Ok(())
            })
            .invoke_handler(tauri::generate_handler![
                crate::cmd::vault::vault_init,
                crate::cmd::vault::vault_unlock,
                crate::cmd::vault::vault_is_locked,
                crate::cmd::vault::vault_is_initialized,
                crate::cmd::vault::vault_lock,
                crate::cmd::vault::vault_list_entries,
                crate::cmd::vault::vault_count_entries,
                crate::cmd::vault::vault_get_entry,
                crate::cmd::vault::vault_copy_password,
                crate::cmd::vault::vault_copy_field,
                crate::cmd::vault::vault_reveal_password,
                crate::cmd::vault::vault_reveal_notes,
                crate::cmd::vault::vault_create_entry,
                crate::cmd::vault::vault_update_entry,
                crate::cmd::vault::vault_set_favorite,
                crate::cmd::vault::vault_delete_entry,
                crate::cmd::vault::vault_list_deleted_entries,
                crate::cmd::vault::vault_restore_entry,
                crate::cmd::vault::vault_purge_deleted,
                crate::cmd::vault::vault_change_master_password,
                crate::cmd::vault::vault_rotate_dek,
                crate::cmd::vault::vault_rebuild_local_db,
                crate::cmd::vault::vault_reset_repository,
                crate::cmd::vault::vault_export_to_file,
                crate::cmd::vault::vault_import,
                crate::cmd::vault::vault_totp_generate,
                crate::cmd::vault::vault_copy_totp,
                crate::cmd::vault::vault_totp_parse_uri,
                crate::cmd::vault::vault_query_audit_log,
                crate::cmd::vault::vault_hibp_check,
                crate::cmd::vault::vault_hibp_audit,
                crate::cmd::vault::vault_hibp_audit_cancel,
                crate::cmd::vault::vault_check_master_password_strength,
                crate::cmd::vault::vault_show_unlock_window,
                crate::cmd::vault::vault_is_biometric_available,
                crate::cmd::vault::vault_is_biometric_enrolled,
                crate::cmd::vault::vault_unlock_biometric,
                crate::cmd::vault::vault_enroll_biometric,
                crate::cmd::vault::vault_disenroll_biometric,
                crate::cmd::vault::vault_sync,
                crate::cmd::vault::vault_sync_last_time,
                crate::cmd::vault::vault_get_sync_config,
                crate::cmd::vault::vault_set_sync_config,
                crate::cmd::vault::vault_resolve_conflict,
                crate::cmd::vault::vault_sync_abort,
                crate::cmd::vault::vault_sync_discard,
                crate::cmd::vault::vault_git_init,
                crate::cmd::vault::vault_git_remove_remote,
                crate::cmd::vault::vault_git_test_connection,
                crate::cmd::vault::vault_git_remote,
                crate::cmd::vault::vault_sync_readyness,
                crate::cmd::vault::vault_sync_remote_status,
                crate::cmd::vault::vault_sync_conflicts,
                crate::cmd::vault::vault_sync_pending_removals,
                crate::cmd::vault::vault_sync_confirm_removals,
                crate::cmd::vault::vault_history_list,
                crate::cmd::vault::vault_history_preview,
                crate::cmd::vault::vault_history_restore,
                crate::cmd::vault::vault_conflict_preview,
                crate::cmd::vault::vault_conflict_reveal,
                crate::cmd::vault::vault_ssh_add_to_agent,
                crate::cmd::vault::vault_ssh_remove_from_agent,
                crate::cmd::vault::vault_ssh_list_agent,
                crate::cmd::vault::vault_ssh_auto_add_all,
                crate::cmd::vault::vault_ssh_is_agent_running,
                crate::cmd::vault::vault_ssh_parse_key,
                crate::cmd::vault::vault_ssh_key_meta,
                crate::cmd::vault::vault_get_conf,
                crate::cmd::vault::vault_export_recovery_key,
                crate::cmd::vault::vault_recover_from_paper,
                crate::cmd::vault::vault_locate_paper_error,
                crate::cmd::vault::vault_has_recovery_backup,
            ])
            .build()
    }

    pub(crate) fn on_setup(_app: &tauri::AppHandle) -> Result<(), String> {
        match assistant_vault::conf::VaultConf::load() {
            Ok(_) => Ok(()),
            Err(e) => {
                // 配置损坏或非法时不覆盖用户文件、也不阻塞应用启动；
                // 具体错误由 vault_get_conf 以 Validation 返回给前端。
                log::error!("Failed to init vault config: {}", e);
                Ok(())
            }
        }
    }

    pub(crate) fn on_exit(app: &tauri::AppHandle) {
        crate::cmd::vault::vault_cleanup_on_exit(app);
    }
}

#[cfg(not(feature = "vault"))]
mod mounted {
    use super::TauriPlugin;

    pub(crate) fn init() -> TauriPlugin<tauri::Wry> {
        tauri::plugin::Builder::<tauri::Wry>::new("vault").build()
    }

    pub(crate) fn on_setup(_app: &tauri::AppHandle) -> Result<(), String> {
        Ok(())
    }

    pub(crate) fn on_exit(_app: &tauri::AppHandle) {}
}

pub(crate) use mounted::{init, on_exit, on_setup};

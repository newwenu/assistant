// 参考: feature-mount-traversal-spec.md（已归档） §3 — vault 走插件边界，可选性只体现一次

use tauri::plugin::TauriPlugin;

#[cfg(feature = "vault")]
mod mounted {
    use super::TauriPlugin;

    use tauri::Manager;

    use crate::app_dirs::registry::{RootCoverage, RootScope, StatsMode};
    use crate::app_dirs::AppDirs;
    use crate::cmd::storage::section::{
        EntryDecl, SectionEntry, SectionError, SectionReclaimReport, SectionRegistry,
        SectionSample, SectionStats, StorageSection,
    };
    use crate::cmd::vault;
    use crate::protocol::dto::{
        ReclaimPolicyDto, StorageEligibilityDto, StoragePolicySourceDto, StorageReclaimActionDto,
        StorageReclaimReasonDto,
    };
    use assistant_config::ConfGroup;
    use assistant_vault::conf::VaultConf;
    use assistant_vault::security_db::SecurityDbState;
    use assistant_vault::storage_section::{
        VaultAction, VaultCoverage, VaultEligibility, VaultPolicy, VaultReason, VaultRisk,
        VaultScope, VaultSectionError, VaultStats, VaultStatsMode, VaultStorageEntry,
        VaultStorageSection,
    };

    /// 安全域分区的 core 侧适配器：只做**类型映射与转发**，不认识任何条目 `key`
    /// 或 `security` 字面量（G17(b) / V21 / V24）。
    ///
    /// 持 `SecurityDbState`（`Clone`，内部状态 Arc 共享）与两个由本侧现算的目录，
    /// 不再持 `AppHandle`；取不到状态 / 目录时不注册分区（§7.1「未挂载 = 行不存在」）。
    struct SecuritySection {
        section: VaultStorageSection,
        entries: &'static [SectionEntry],
        actions: &'static [StorageReclaimActionDto],
    }

    fn map_scope(scope: VaultScope) -> RootScope {
        match scope {
            VaultScope::Data => RootScope::Data,
            VaultScope::Local => RootScope::Local,
        }
    }

    fn map_coverage(coverage: VaultCoverage) -> RootCoverage {
        match coverage {
            VaultCoverage::SingleFile => RootCoverage::SingleFile,
            VaultCoverage::SelfFilesOnly => RootCoverage::SelfFilesOnly,
            VaultCoverage::RecursiveTree => RootCoverage::RecursiveTree,
        }
    }

    /// 采样档原样转发分区自述值（I79 §3.3）：core 不按 `key` 或 `coverage` 反推档位，
    /// 因为「递归但结构性有界」与「递归且无上界」在 `coverage` 上同形。
    fn map_stats_mode(mode: VaultStatsMode) -> StatsMode {
        match mode {
            VaultStatsMode::Inline => StatsMode::Inline,
            VaultStatsMode::Deferred => StatsMode::Deferred,
        }
    }

    fn map_action(action: VaultAction) -> StorageReclaimActionDto {
        match action {
            VaultAction::None => StorageReclaimActionDto::None,
            VaultAction::PruneToLimit => StorageReclaimActionDto::PruneToLimit,
            VaultAction::SweepExpired => StorageReclaimActionDto::SweepExpired,
        }
    }

    fn unmap_action(action: StorageReclaimActionDto) -> Option<VaultAction> {
        match action {
            StorageReclaimActionDto::None => Some(VaultAction::None),
            StorageReclaimActionDto::PruneToLimit => Some(VaultAction::PruneToLimit),
            StorageReclaimActionDto::SweepExpired => Some(VaultAction::SweepExpired),
            _ => None,
        }
    }

    fn map_eligibility(eligibility: VaultEligibility) -> StorageEligibilityDto {
        match eligibility {
            VaultEligibility::Eligible => StorageEligibilityDto::Eligible,
            VaultEligibility::NotEligible => StorageEligibilityDto::NotEligible,
            VaultEligibility::Locked => StorageEligibilityDto::Locked,
            VaultEligibility::OwnerManaged => StorageEligibilityDto::OwnerManaged,
        }
    }

    fn map_reason(reason: VaultReason) -> StorageReclaimReasonDto {
        match reason {
            VaultReason::Expired => StorageReclaimReasonDto::Expired,
            VaultReason::OverLimit => StorageReclaimReasonDto::OverLimit,
            VaultReason::Transient => StorageReclaimReasonDto::Transient,
            VaultReason::UserData => StorageReclaimReasonDto::UserData,
            VaultReason::BusinessData => StorageReclaimReasonDto::BusinessData,
            VaultReason::Locked => StorageReclaimReasonDto::Locked,
            VaultReason::Unknown => StorageReclaimReasonDto::Unknown,
        }
    }

    fn map_risk(risk: VaultRisk) -> crate::cmd::storage::section::RiskTier {
        match risk {
            VaultRisk::Low => crate::cmd::storage::section::RiskTier::Low,
            VaultRisk::Medium => crate::cmd::storage::section::RiskTier::Medium,
            VaultRisk::High => crate::cmd::storage::section::RiskTier::High,
            VaultRisk::Forbidden => crate::cmd::storage::section::RiskTier::Forbidden,
        }
    }

    fn map_policy(policy: VaultPolicy) -> ReclaimPolicyDto {
        match policy {
            VaultPolicy::None => ReclaimPolicyDto {
                source: StoragePolicySourceDto::None,
                ..ReclaimPolicyDto::default()
            },
            VaultPolicy::Constant { keep_count } => ReclaimPolicyDto {
                source: StoragePolicySourceDto::Constant,
                keep_count: keep_count.and_then(|count| u32::try_from(count).ok()),
                ..ReclaimPolicyDto::default()
            },
            VaultPolicy::Config {
                retention_days,
                effective_since,
            } => ReclaimPolicyDto {
                source: StoragePolicySourceDto::Config,
                retention_days,
                effective_since,
                mutable: true,
                ..ReclaimPolicyDto::default()
            },
        }
    }

    fn unmap_policy(policy: Option<ReclaimPolicyDto>) -> Option<VaultPolicy> {
        let policy = policy?;
        match policy.source {
            StoragePolicySourceDto::None => Some(VaultPolicy::None),
            StoragePolicySourceDto::Constant => Some(VaultPolicy::Constant {
                keep_count: policy.keep_count.map(|count| count as usize),
            }),
            StoragePolicySourceDto::Config => Some(VaultPolicy::Config {
                retention_days: policy.retention_days,
                effective_since: policy.effective_since,
            }),
            StoragePolicySourceDto::Owner => None,
        }
    }

    fn map_stats(stats: VaultStats) -> SectionStats {
        SectionStats {
            bytes: stats.bytes,
            count: stats.count,
            oldest_at: stats.oldest_at,
            newest_at: stats.newest_at,
        }
    }

    fn map_error(error: VaultSectionError) -> SectionError {
        match error {
            VaultSectionError::Locked => SectionError::Locked,
            VaultSectionError::Unavailable => SectionError::Unavailable,
            VaultSectionError::Unreadable => SectionError::Unreadable,
        }
    }

    fn map_entry(entry: &VaultStorageEntry) -> SectionEntry {
        SectionEntry {
            entry: entry.entry,
            key: entry.key,
            segments: entry.segments,
            scope: map_scope(entry.scope),
            coverage: map_coverage(entry.coverage),
            owner: entry.owner,
            inventory: entry.inventory,
            // 档位由分区自述（I79 §3.3）：锁态只影响资格文案，不影响观测，因此不构成
            // 「整条必须延后算」的理由；core 不按 key 反推档位。
            stats_mode: map_stats_mode(entry.stats_mode),
            decl: EntryDecl {
                action: map_action(entry.decl.action),
                eligibility: map_eligibility(entry.decl.eligibility),
                reason: map_reason(entry.decl.reason),
                policy: map_policy(entry.decl.policy),
                auto_active: entry.decl.auto_active,
                counts_self_logs: entry.decl.counts_self_logs,
                risk: map_risk(entry.decl.risk),
                note: entry.decl.note,
            },
        }
    }

    fn unmap_stats(stats: SectionStats) -> VaultStats {
        VaultStats {
            bytes: stats.bytes,
            count: stats.count,
            oldest_at: stats.oldest_at,
            newest_at: stats.newest_at,
        }
    }

    impl SecuritySection {
        fn new(
            state: SecurityDbState,
            repo_dir: std::path::PathBuf,
            recovery_dir: std::path::PathBuf,
            conf_path: std::path::PathBuf,
        ) -> Self {
            let section = VaultStorageSection::new(state, repo_dir, recovery_dir, conf_path);
            // 条目清单在注册时一次性映射并固化为 `'static`：注册只发生一次，
            // 采样 / 对账期间不再重建。
            let entries: &'static [SectionEntry] = Box::leak(
                section
                    .entries()
                    .iter()
                    .map(map_entry)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            );
            Self {
                section,
                entries,
                actions: &[StorageReclaimActionDto::PruneToLimit],
            }
        }
    }

    impl StorageSection for SecuritySection {
        fn id(&self) -> &'static str {
            "security-db"
        }

        fn entries(&self) -> &'static [SectionEntry] {
            self.entries
        }

        /// 只转发：`None` 原样返回（不归本分区），**不**按条目 `key` 字面量分派（G17(b)）。
        fn stats(&self, key: &'static str) -> Option<Result<SectionStats, SectionError>> {
            self.section
                .stats(key)
                .map(|result| result.map(map_stats).map_err(map_error))
        }

        fn eligibility(
            &self,
            key: &'static str,
            sample: &SectionSample,
        ) -> Option<(StorageEligibilityDto, StorageReclaimReasonDto)> {
            let observed = sample
                .observed
                .as_ref()
                .ok()
                .map(|stats| unmap_stats(*stats));
            self.section
                .eligibility(key, observed.as_ref())
                .map(|(eligibility, reason)| (map_eligibility(eligibility), map_reason(reason)))
        }

        fn policy(&self, key: &'static str) -> Option<ReclaimPolicyDto> {
            self.section.policy(key).map(map_policy)
        }

        fn reclaim_actions(&self) -> &'static [StorageReclaimActionDto] {
            self.actions
        }

        fn reclaim(
            &self,
            key: &'static str,
            action: StorageReclaimActionDto,
            policy: Option<ReclaimPolicyDto>,
        ) -> Option<Result<SectionReclaimReport, SectionError>> {
            let action = unmap_action(action)?;
            self.section
                .reclaim(key, action, unmap_policy(policy))
                .map(|result| {
                    result
                        .map(|report| SectionReclaimReport {
                            removed: report.removed,
                            removed_bytes: report.removed_bytes,
                            failed: report.failed,
                        })
                        .map_err(map_error)
                })
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
                // 安全域分区在 `on_setup` 自注册：那时 `SecurityDbState` 已由
                // `vault_setup` 放进 state，能取到真实状态而非构造期快照（§3.10）。
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

    pub(crate) fn on_setup(ctx: &crate::boot::StartupCtx<'_>) -> Result<(), String> {
        let app = ctx.app;
        match assistant_vault::conf::VaultConf::load() {
            Ok(_) => {}
            Err(e) => {
                // 配置损坏或非法时不覆盖用户文件、也不阻塞应用启动；
                // 具体错误由 vault_get_conf 以 Validation 返回给前端。
                log::error!("Failed to init vault config: {}", e);
            }
        }
        let dirs = match ctx.dirs() {
            Ok(dirs) => dirs,
            Err(error) => {
                log::error!("dirs 未就绪，安全域分区未注册: {}", error);
                return Ok(());
            }
        };
        register_storage_section(app, &dirs);
        Ok(())
    }

    /// 分区自注册（§3.10）：任一前置取不到都只记日志、不注册分区，
    /// 该分区条目随之整体消失（§7.1「未挂载 = 行不存在」），绝不 panic。
    fn register_storage_section(app: &tauri::AppHandle, dirs: &AppDirs) {
        let Some(registry) = app
            .try_state::<std::sync::Arc<SectionRegistry>>()
            .map(|state| state.inner().clone())
        else {
            log::error!("SectionRegistry 槽不存在，安全域分区未注册");
            return;
        };
        let Some(state) = app
            .try_state::<SecurityDbState>()
            .map(|state| state.inner().clone())
        else {
            log::error!("SecurityDbState 不存在，安全域分区未注册");
            return;
        };
        let section = std::sync::Arc::new(SecuritySection::new(
            state,
            vault::repo_dir(dirs),
            vault::recovery_dir(dirs),
            vault::security_dir(dirs).join(<VaultConf as ConfGroup>::FILE_NAME),
        ));
        if let Err(error) = registry.register(section) {
            log::error!("安全域分区注册失败: {}", error);
        }
    }

    pub(crate) fn on_exit(app: &tauri::AppHandle) {
        crate::cmd::vault::vault_cleanup_on_exit(app);
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// V21：观测四元组两向无损——`oldest_at` / `newest_at` 漏搬会让页面时间窗恒空。
        #[test]
        fn stats_mapping_keeps_time_bounds_both_ways() {
            let source = VaultStats {
                bytes: 4_096,
                count: 7,
                oldest_at: Some(100),
                newest_at: Some(900),
            };
            let mapped = map_stats(source);
            assert_eq!((mapped.bytes, mapped.count), (4_096, 7));
            assert_eq!(mapped.oldest_at, Some(100), "oldest_at 漏搬");
            assert_eq!(mapped.newest_at, Some(900), "newest_at 漏搬");
            assert_eq!(unmap_stats(mapped), source, "反向映射同样不得漏字段");
        }

        /// V21 / V22③：逐条映射不丢声明字段，owner 命名域三条成立。
        #[test]
        fn entry_mapping_keeps_declaration_and_owner_domain() {
            let dir = tempfile::tempdir().expect("temp dir");
            let db = dir.path().join("security").join("security.db");
            let state = SecurityDbState::new(&db.to_string_lossy());
            let section = SecuritySection::new(
                state,
                dir.path().join("repo"),
                dir.path().join("recovery"),
                dir.path()
                    .join("security")
                    .join(<VaultConf as ConfGroup>::FILE_NAME),
            );
            let mapped = section.entries();
            let source = section.section.entries();
            assert_eq!(mapped.len(), source.len(), "映射不得增减条目");
            for (mapped, source) in mapped.iter().zip(source.iter()) {
                assert_eq!(mapped.entry, source.entry);
                assert_eq!(mapped.key, source.key);
                assert_eq!(mapped.segments, source.segments);
                assert_eq!(mapped.decl.auto_active, source.decl.auto_active);
                assert_eq!(mapped.decl.counts_self_logs, source.decl.counts_self_logs);
                assert_eq!(mapped.decl.note, source.decl.note);
                assert!(
                    !mapped.owner.contains("::"),
                    "{} owner 不得含路径分隔",
                    mapped.entry
                );
                assert_ne!(
                    mapped.owner, "vault",
                    "{} owner 不得等于 feature 名",
                    mapped.entry
                );
                assert!(
                    !mapped.owner.ends_with("State"),
                    "{} owner 不得是可选 crate 类型名",
                    mapped.entry
                );
            }
        }

        /// G17(b) / V24：适配器源码不出现可选条目路径与条目 id 字面量。
        #[test]
        fn adapter_source_has_no_entry_literals() {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/feature_plugins/vault_plugin.rs");
            let source = std::fs::read_to_string(&path).expect("读适配器源码");
            let optional_path = ["security", "/"].concat();
            assert!(
                !source.contains(&optional_path),
                "适配器不得出现可选条目路径字面量"
            );
            for number in 7..=11 {
                let id = format!("E{number}");
                assert!(!source.contains(&id), "适配器不得出现条目 id `{id}`");
            }
        }
    }
}

#[cfg(not(feature = "vault"))]
mod mounted {
    use super::TauriPlugin;

    pub(crate) fn init() -> TauriPlugin<tauri::Wry> {
        tauri::plugin::Builder::<tauri::Wry>::new("vault").build()
    }

    pub(crate) fn on_setup(_ctx: &crate::boot::StartupCtx<'_>) -> Result<(), String> {
        Ok(())
    }

    pub(crate) fn on_exit(_app: &tauri::AppHandle) {}
}

pub(crate) use mounted::{init, on_exit, on_setup};

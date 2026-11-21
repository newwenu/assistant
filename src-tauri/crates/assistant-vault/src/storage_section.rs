//! 安全域分区自述（I61 §3.10「可选功能分区自注册」/ §3.1.1）：`E7`–`E11` 的条目清单、
//! 策略、资格判据与回收入口。
//!
//! 只用本 crate 的自有普通类型：不依赖 app crate，也不依赖 `assistant-protocol`
//! （本 crate 的依赖里没有它），映射成 core DTO 的责任在 app 侧适配器（§3.1.1）。

use std::path::{Path, PathBuf};

use crate::security_db::{SecurityDbState, MAX_BACKUPS};
use crate::storage_stats::{self, DirStats};

/// 可选分区首段：core 只保留这一个首段事实（`OPTIONAL_SEGMENTS`），条目细节全在本文件。
const SEGMENT: &str = "security";

const KEY_DB: &str = "security/security.db";
const KEY_BACKUPS: &str = "security/backups";
const KEY_REPO: &str = "security/vault";
const KEY_RECOVERY: &str = "security/recovery";
const KEY_AUDIT: &str = "security/audit";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultScope {
    Data,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultCoverage {
    SingleFile,
    SelfFilesOnly,
    RecursiveTree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultAction {
    None,
    PruneToLimit,
    SweepExpired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultEligibility {
    Eligible,
    NotEligible,
    Locked,
    OwnerManaged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultReason {
    Expired,
    OverLimit,
    Transient,
    UserData,
    BusinessData,
    Locked,
    Unknown,
}

/// 风险档：core 只据此派生 `confirmation`，不进 wire（与 `action` 无关）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultRisk {
    Low,
    Medium,
    High,
    Forbidden,
}

/// 策略自述：core 的 `ReclaimPolicyDto` 由适配器逐字段映射，本 crate 不持有 DTO。
///
/// `keep_count` 用 `usize`（与 owner 的 `MAX_BACKUPS` 同型，避免在功能侧做窄化转换）；
/// 映射成 wire 的 `u32` 是适配器的事（`R-06`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultPolicy {
    None,
    Constant {
        keep_count: Option<usize>,
    },
    Config {
        retention_days: Option<u32>,
        effective_since: Option<i64>,
    },
}

impl VaultPolicy {
    pub const fn keep_count(self) -> Option<usize> {
        match self {
            Self::Constant { keep_count } => keep_count,
            Self::Config { .. } | Self::None => None,
        }
    }
}

/// 观测四元组：无文件时 `oldest_at` / `newest_at` 为 `None`，不是 `0`（§3.9）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VaultStats {
    pub bytes: u64,
    pub count: u64,
    pub oldest_at: Option<i64>,
    pub newest_at: Option<i64>,
}

impl From<DirStats> for VaultStats {
    fn from(stats: DirStats) -> Self {
        Self {
            bytes: stats.bytes,
            count: stats.count,
            oldest_at: stats.oldest_at,
            newest_at: stats.newest_at,
        }
    }
}

/// 取数失败的封闭类别：core 映射成 `locked` / `unavailable` / `unreadable`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultSectionError {
    Locked,
    Unavailable,
    Unreadable,
}

/// 一次回收的回报：字节由 owner 现算，core 不预估（§3.6 硬约束 3）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VaultReclaimReport {
    pub removed: u64,
    pub removed_bytes: u64,
    pub failed: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VaultEntryDecl {
    pub action: VaultAction,
    pub eligibility: VaultEligibility,
    pub reason: VaultReason,
    pub policy: VaultPolicy,
    pub auto_active: bool,
    pub counts_self_logs: bool,
    pub risk: VaultRisk,
    pub note: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VaultStorageEntry {
    pub entry: &'static str,
    pub key: &'static str,
    pub segments: &'static [&'static str],
    pub scope: VaultScope,
    pub coverage: VaultCoverage,
    /// 只做展示，不参与路由（G17 / V22）。
    pub owner: &'static str,
    pub inventory: bool,
    pub decl: VaultEntryDecl,
}

const ENTRIES: &[VaultStorageEntry] = &[
    VaultStorageEntry {
        entry: "E7",
        key: KEY_DB,
        segments: &[SEGMENT, "security.db"],
        scope: VaultScope::Data,
        coverage: VaultCoverage::SingleFile,
        owner: "SecurityDb",
        inventory: true,
        decl: VaultEntryDecl {
            action: VaultAction::None,
            eligibility: VaultEligibility::NotEligible,
            reason: VaultReason::BusinessData,
            policy: VaultPolicy::None,
            auto_active: false,
            counts_self_logs: false,
            risk: VaultRisk::Forbidden,
            note: None,
        },
    },
    VaultStorageEntry {
        entry: "E8",
        key: KEY_BACKUPS,
        segments: &[SEGMENT, "backups"],
        scope: VaultScope::Data,
        coverage: VaultCoverage::RecursiveTree,
        owner: "SecurityDb",
        inventory: true,
        decl: VaultEntryDecl {
            action: VaultAction::PruneToLimit,
            eligibility: VaultEligibility::Eligible,
            reason: VaultReason::OverLimit,
            policy: VaultPolicy::Constant {
                keep_count: Some(MAX_BACKUPS),
            },
            auto_active: true,
            counts_self_logs: false,
            risk: VaultRisk::Medium,
            note: None,
        },
    },
    VaultStorageEntry {
        entry: "E9",
        key: KEY_REPO,
        segments: &[SEGMENT, "vault"],
        scope: VaultScope::Data,
        coverage: VaultCoverage::RecursiveTree,
        owner: "SecurityRepo",
        inventory: true,
        decl: VaultEntryDecl {
            action: VaultAction::None,
            eligibility: VaultEligibility::NotEligible,
            reason: VaultReason::UserData,
            policy: VaultPolicy::None,
            auto_active: false,
            counts_self_logs: false,
            risk: VaultRisk::Forbidden,
            note: None,
        },
    },
    VaultStorageEntry {
        entry: "E10",
        key: KEY_RECOVERY,
        segments: &[SEGMENT, "recovery"],
        scope: VaultScope::Data,
        coverage: VaultCoverage::RecursiveTree,
        owner: "SecurityRecovery",
        inventory: true,
        decl: VaultEntryDecl {
            action: VaultAction::SweepExpired,
            eligibility: VaultEligibility::OwnerManaged,
            reason: VaultReason::Transient,
            policy: VaultPolicy::Constant { keep_count: None },
            auto_active: true,
            counts_self_logs: false,
            risk: VaultRisk::Low,
            note: None,
        },
    },
    VaultStorageEntry {
        entry: "E11",
        key: KEY_AUDIT,
        segments: &[SEGMENT, "audit"],
        scope: VaultScope::Data,
        coverage: VaultCoverage::RecursiveTree,
        owner: "SecurityDb",
        inventory: true,
        decl: VaultEntryDecl {
            action: VaultAction::SweepExpired,
            eligibility: VaultEligibility::OwnerManaged,
            reason: VaultReason::Expired,
            policy: VaultPolicy::Config {
                retention_days: None,
                effective_since: None,
            },
            auto_active: true,
            counts_self_logs: false,
            risk: VaultRisk::Medium,
            note: None,
        },
    },
];

/// 安全域分区：持 owner 句柄与两个由适配器现算的目录，只转发 owner 的公开观测入口。
///
/// `Clone` 与 `SecurityDbState` 一致：克隆出的是同一份会话 / 策略状态（Arc 共享），
/// 因此解锁后克隆体也能观测到新会话。
#[derive(Clone)]
pub struct VaultStorageSection {
    state: SecurityDbState,
    repo_dir: PathBuf,
    recovery_dir: PathBuf,
}

impl VaultStorageSection {
    /// 两个目录由适配器用 app 侧既有的路径知识现算后传入：本 crate 不拼 `security/` 前缀。
    pub fn new(state: SecurityDbState, repo_dir: PathBuf, recovery_dir: PathBuf) -> Self {
        Self {
            state,
            repo_dir,
            recovery_dir,
        }
    }

    pub fn entries(&self) -> &'static [VaultStorageEntry] {
        ENTRIES
    }

    /// 按 `key` 取观测快照；`None` = 不归本分区（未登记或属于别的分区）。
    pub fn stats(&self, key: &str) -> Option<Result<VaultStats, VaultSectionError>> {
        let result = match key {
            // `E7` 只读 `metadata`：vault 锁定不影响它，因此失败只可能是文件正在替换（§7.1）。
            KEY_DB => self
                .state
                .db_file_stats()
                .map(VaultStats::from)
                .map_err(|_| VaultSectionError::Unavailable),
            KEY_BACKUPS => self
                .state
                .backup_stats()
                .map(VaultStats::from)
                .map_err(|_| VaultSectionError::Unreadable),
            KEY_REPO => self.repo_stats(),
            KEY_RECOVERY => storage_stats::tree_stats(&self.recovery_dir)
                .map(VaultStats::from)
                .map_err(|_| VaultSectionError::Unreadable),
            KEY_AUDIT => self
                .state
                .audit_stats()
                .map(VaultStats::from)
                .map_err(|_| VaultSectionError::Unreadable),
            _ => return None,
        };
        Some(result)
    }

    /// `E9`：锁定态下仓库内容不可信，先判锁再观测（`locked` 语义，§3.10）。
    fn repo_stats(&self) -> Result<VaultStats, VaultSectionError> {
        if self.state.is_locked() {
            return Err(VaultSectionError::Locked);
        }
        storage_stats::tree_stats(&self.repo_dir)
            .map(VaultStats::from)
            .map_err(|_| VaultSectionError::Unreadable)
    }

    /// 随观测态变化的资格判据；`sample = None` 表示取数失败（判据不可算 → `unknown`）。
    pub fn eligibility(
        &self,
        key: &str,
        sample: Option<&VaultStats>,
    ) -> Option<(VaultEligibility, VaultReason)> {
        let judged = match key {
            // `E7` 观测失败**不得**判 `locked`（§3.4 Q3 / §7.1 特例 2）。
            KEY_DB => match sample {
                Some(_) => (VaultEligibility::NotEligible, VaultReason::BusinessData),
                None => (VaultEligibility::NotEligible, VaultReason::Unknown),
            },
            KEY_BACKUPS => match sample {
                Some(stats) if stats.count > MAX_BACKUPS as u64 => {
                    (VaultEligibility::Eligible, VaultReason::OverLimit)
                }
                Some(_) => (VaultEligibility::NotEligible, VaultReason::OverLimit),
                None => (VaultEligibility::NotEligible, VaultReason::Unknown),
            },
            KEY_REPO => {
                if self.state.is_locked() {
                    (VaultEligibility::Locked, VaultReason::Locked)
                } else {
                    (VaultEligibility::NotEligible, VaultReason::UserData)
                }
            }
            // `E10` / `E11` 本期静态自述：清扫由 owner 自动路径负责，不给手动入口。
            KEY_RECOVERY => (VaultEligibility::OwnerManaged, VaultReason::Transient),
            KEY_AUDIT => (VaultEligibility::OwnerManaged, VaultReason::Expired),
            _ => return None,
        };
        Some(judged)
    }

    /// 运行期策略：`E8` 自述 owner 常量份数，`E11` 自述当前生效的保留天数与生效时刻。
    pub fn policy(&self, key: &str) -> Option<VaultPolicy> {
        match key {
            KEY_BACKUPS => Some(VaultPolicy::Constant {
                keep_count: Some(MAX_BACKUPS),
            }),
            KEY_AUDIT => self
                .state
                .audit_policy()
                .map(|(days, since)| VaultPolicy::Config {
                    retention_days: Some(days),
                    effective_since: Some(since),
                }),
            _ => None,
        }
    }

    /// 手动入口只含 `E8` 的 `PruneToLimit`：`E10` / `E11` 的 `SweepExpired` 是 owner 自动路径。
    pub fn reclaim_actions(&self) -> &'static [VaultAction] {
        &[VaultAction::PruneToLimit]
    }

    /// `E8`：重新计数与裁剪都在 owner 侧一次完成，`keep` 来自策略快照（缺省用 owner 常量）。
    pub fn reclaim(
        &self,
        key: &str,
        action: VaultAction,
        policy: Option<VaultPolicy>,
    ) -> Option<Result<VaultReclaimReport, VaultSectionError>> {
        if key != KEY_BACKUPS || action != VaultAction::PruneToLimit {
            return None;
        }
        let keep = policy
            .and_then(VaultPolicy::keep_count)
            .unwrap_or(MAX_BACKUPS);
        let report = SecurityDbState::prune_backups_to(Path::new(self.state.db_path()), keep)
            .map(|report| VaultReclaimReport {
                removed: report.removed,
                removed_bytes: report.removed_bytes,
                failed: report.failed,
            })
            .map_err(|_| VaultSectionError::Unreadable);
        Some(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section() -> (VaultStorageSection, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let db = dir.path().join("security").join("security.db");
        let state = SecurityDbState::new(&db.to_string_lossy());
        let section =
            VaultStorageSection::new(state, dir.path().join("repo"), dir.path().join("recovery"));
        (section, dir)
    }

    #[test]
    fn entries_are_well_formed() {
        assert_eq!(ENTRIES.len(), 5);
        for entry in ENTRIES {
            assert_eq!(entry.segments.join("/"), entry.key, "{}", entry.entry);
            assert_eq!(entry.segments[0], SEGMENT, "{}", entry.entry);
            assert!(entry.inventory, "{}", entry.entry);
            assert!(entry.decl.note.is_none(), "{}", entry.entry);
        }
    }

    #[test]
    fn unknown_key_is_not_claimed() {
        let (section, _dir) = section();
        assert!(section.stats("security/elsewhere").is_none());
        assert!(section.eligibility("security/elsewhere", None).is_none());
        assert!(section.policy("security/elsewhere").is_none());
        assert!(section
            .reclaim("security/elsewhere", VaultAction::PruneToLimit, None)
            .is_none());
    }

    #[test]
    fn clone_shares_policy_state() {
        let (section, _dir) = section();
        let clone = section.clone();
        clone.state.set_audit_retention_days(7);
        assert_eq!(section.state.audit_retention_days(), 7);
        let cloned = clone.policy(KEY_AUDIT).expect("E11 自述策略");
        assert_eq!(cloned, section.policy(KEY_AUDIT).expect("E11 自述策略"));
        match cloned {
            VaultPolicy::Config {
                retention_days,
                effective_since,
            } => {
                assert_eq!(retention_days, Some(7));
                assert!(effective_since.is_some());
            }
            other => panic!("unexpected policy: {other:?}"),
        }
    }

    #[test]
    fn missing_entries_are_zero_stats_and_locked_repo_is_locked() {
        let (section, _dir) = section();
        assert_eq!(section.stats(KEY_RECOVERY), Some(Ok(VaultStats::default())));
        assert_eq!(section.stats(KEY_AUDIT), Some(Ok(VaultStats::default())));
        assert_eq!(section.stats(KEY_DB), Some(Ok(VaultStats::default())));
        assert_eq!(
            section.stats(KEY_REPO),
            Some(Err(VaultSectionError::Locked))
        );
        assert_eq!(
            section.eligibility(KEY_REPO, None),
            Some((VaultEligibility::Locked, VaultReason::Locked))
        );
    }

    #[test]
    fn backups_over_limit_are_eligible() {
        let (section, _dir) = section();
        let over = VaultStats {
            count: MAX_BACKUPS as u64 + 1,
            ..VaultStats::default()
        };
        assert_eq!(
            section.eligibility(KEY_BACKUPS, Some(&over)),
            Some((VaultEligibility::Eligible, VaultReason::OverLimit))
        );
        let at_limit = VaultStats {
            count: MAX_BACKUPS as u64,
            ..VaultStats::default()
        };
        assert_eq!(
            section.eligibility(KEY_BACKUPS, Some(&at_limit)),
            Some((VaultEligibility::NotEligible, VaultReason::OverLimit))
        );
    }

    #[test]
    fn only_backups_offer_a_manual_action() {
        let (section, _dir) = section();
        assert_eq!(section.reclaim_actions().len(), 1);
        assert_eq!(section.reclaim_actions()[0], VaultAction::PruneToLimit);
        assert!(section
            .reclaim(KEY_DB, VaultAction::PruneToLimit, None)
            .is_none());
        assert!(section
            .reclaim(KEY_BACKUPS, VaultAction::SweepExpired, None)
            .is_none());
        let report = section
            .reclaim(KEY_BACKUPS, VaultAction::PruneToLimit, None)
            .expect("E8 提供手动入口");
        assert_eq!(report, Ok(VaultReclaimReport::default()));
    }
}

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttachmentSummary {
    pub attachment_id: String,
    pub media_type: String,
    pub bytes: u64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationAttachmentSummary {
    pub conversation_id: String,
    pub title: String,
    pub attachments: Vec<AttachmentSummary>,
    pub total_bytes: u64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageBreakdownDto {
    /// 目录内附件总字节数。未找到目录配额常量，无法论证更窄上限，故保留 `u64`。
    pub total_bytes: u64,
    pub total_count: u32,
    pub orphans: Vec<AttachmentSummary>,
    pub conversations: Vec<ConversationAttachmentSummary>,
    /// 一次快照一个时刻（秒）：遍历非原子，必须随附。
    #[serde(default)]
    pub sampled_at: i64,
    /// 字节口径，本期恒 `fileLength`；口径进 DTO，不只进文档。
    #[serde(default)]
    pub byte_basis: StorageByteBasisDto,
    /// 被 ≥2 个会话引用的附件字节之和（同一附件只计一次）。
    #[serde(default)]
    pub shared_bytes: u64,
    /// 仅被 1 个会话引用的附件字节之和。
    #[serde(default)]
    pub referenced_once_bytes: u64,
    /// 13 条条目，顺序固定（`E1`→`E13`）；不可用条目也必须出现。
    #[serde(default)]
    pub entries: Vec<StorageEntryDto>,
    /// 对账余量快照；恒存在（未扫过时 `status = neverRun`）。
    #[serde(default)]
    pub inventory: StorageInventoryDto,
    /// 除 `E12` 外的 12 条全部 `availability ∈ {available, missing}` 时为 `true`。
    #[serde(default)]
    pub complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttachmentImportDto {
    pub attachment_id: String,
    pub media_type: String,
    pub bytes: u64,
    pub file_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetDirsDto {
    pub attachments_dir: String,
    pub mascot_assets_dir: String,
}

/// 附件限额的只读投影，数值唯一来源是 `assistant_agent::multimodal::ATTACHMENT_LIMITS`。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttachmentLimitsDto {
    pub max_images: u32,
    pub max_files: u32,
    pub max_image_bytes: u64,
    pub max_file_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttachmentDeleteResultDto {
    pub removed_count: u32,
    /// 本次删除释放的字节数。未找到目录配额常量，无法论证更窄上限，故保留 `u64`。
    pub removed_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum StorageLayerDto {
    Data,
    Local,
}

/// 字节口径；本期唯一值 `fileLength`，保留值位以便将来扩展。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum StorageByteBasisDto {
    #[default]
    FileLength,
}

/// 条目可用性；封闭词表跨 tab 共用（I54 §2.4），存储条目只用前六个值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum StorageAvailabilityDto {
    Available,
    Missing,
    Locked,
    Unreadable,
    Unavailable,
    NotImplemented,
    NoReportYet,
    StaleReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum StorageReclaimActionDto {
    None,
    DeleteOrphans,
    DeleteSelected,
    SweepExpired,
    PruneToLimit,
    RebuildCache,
    Rotate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum StorageEligibilityDto {
    Eligible,
    NotEligible,
    Locked,
    OwnerManaged,
    NotImplemented,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum StorageReclaimReasonDto {
    Orphan,
    Expired,
    OverLimit,
    DerivedCache,
    Transient,
    Referenced,
    UserData,
    BusinessData,
    ReplayRisk,
    Locked,
    NoStats,
    NoReclaimApi,
    ExternalOwner,
    PolicyDisabled,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum StoragePolicySourceDto {
    Config,
    Constant,
    Owner,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum StorageConfirmLevelDto {
    None,
    Confirm,
    StrongConfirm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum StorageReclaimStatusDto {
    Done,
    Partial,
    Failed,
    Skipped,
    NotEligible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum StorageReclaimErrorCodeDto {
    PermissionDenied,
    Io,
    OwnerUnavailable,
    PolicyDisabled,
    Locked,
    Unknown,
}

/// 回收策略快照；输出侧 `Option` 键恒在、值 `null`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReclaimPolicyDto {
    pub source: StoragePolicySourceDto,
    pub retention_days: Option<u32>,
    pub keep_count: Option<u32>,
    pub max_bytes: Option<u64>,
    pub effective_since: Option<i64>,
    pub mutable: bool,
    pub fail_closed: bool,
}

impl Default for ReclaimPolicyDto {
    fn default() -> Self {
        Self {
            source: StoragePolicySourceDto::None,
            retention_days: None,
            keep_count: None,
            max_bytes: None,
            effective_since: None,
            mutable: false,
            fail_closed: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReclaimViewDto {
    pub action: StorageReclaimActionDto,
    pub eligibility: StorageEligibilityDto,
    pub reason: Option<StorageReclaimReasonDto>,
    pub policy: ReclaimPolicyDto,
    pub reclaimable_bytes: Option<u64>,
    pub reclaimable_count: Option<u64>,
    pub confirmation: StorageConfirmLevelDto,
    pub auto_active: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageEntryDto {
    pub key: String,
    pub layer: StorageLayerDto,
    pub owner: String,
    pub bytes: Option<u64>,
    pub count: Option<u64>,
    pub oldest_at: Option<i64>,
    pub newest_at: Option<i64>,
    pub availability: StorageAvailabilityDto,
    pub availability_reason: Option<String>,
    pub includes_derived: bool,
    pub counts_self_logs: bool,
    pub reclaim: ReclaimViewDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum StorageInventoryStatusDto {
    NeverRun,
    Running,
    Fresh,
    Stale,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum StorageInventoryClassDto {
    Declared,
    OptionalNotMounted,
    Companion,
    Ignored,
    Unregistered,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageInventoryItemDto {
    pub name: String,
    pub bytes: Option<u64>,
    pub count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageInventoryNodeDto {
    pub name: String,
    pub class: StorageInventoryClassDto,
    pub scope: StorageLayerDto,
    pub entry: Option<String>,
    pub scan_bytes: Option<u64>,
    pub scan_count: Option<u64>,
    pub declared_bytes: Option<u64>,
    pub declared_count: Option<u64>,
    pub residual_bytes: Option<u64>,
    pub residual_count: Option<u64>,
    pub over_registered: bool,
    pub children: Vec<StorageInventoryItemDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageInventoryDto {
    pub status: StorageInventoryStatusDto,
    pub scanned_at: Option<i64>,
    pub failed_at: Option<i64>,
    pub nodes: Vec<StorageInventoryNodeDto>,
    pub unregistered_bytes: Option<u64>,
    pub unregistered_count: Option<u64>,
    pub partial: bool,
    pub truncated_nodes: Vec<String>,
}

impl Default for StorageInventoryDto {
    fn default() -> Self {
        Self {
            status: StorageInventoryStatusDto::NeverRun,
            scanned_at: None,
            failed_at: None,
            nodes: Vec::new(),
            unregistered_bytes: None,
            unregistered_count: None,
            partial: false,
            truncated_nodes: Vec::new(),
        }
    }
}

/// `refresh_storage_inventory` 的返回（I61 §6.2 的 `StorageInventoryStatusDto`）。
///
/// 与 `StorageInventoryDto` 的同名部分同义、同构造点填充；单列是为了让「刷新」
/// 命令在类型层面就无法回数字。枚举 `StorageInventoryStatusDto` 与本文档同名，
/// 故此处以 `Snapshot` 区分。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageInventorySnapshotStatusDto {
    pub status: StorageInventoryStatusDto,
    pub scanned_at: Option<i64>,
    pub failed_at: Option<i64>,
}

/// `get_storage_reclaim_plan` 的输入：空 `entryKeys` = 全部 13 条。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageReclaimPlanRequestDto {
    pub entry_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageReclaimPlanEntryDto {
    pub key: String,
    pub action: StorageReclaimActionDto,
    pub eligibility: StorageEligibilityDto,
    pub reason: Option<StorageReclaimReasonDto>,
    /// 不可回收为 `null`，不得填 0 冒充。
    pub expected_count: Option<u64>,
    pub expected_bytes: Option<u64>,
    pub policy: ReclaimPolicyDto,
    /// 非 `eligible` 恒 `none`；`eligible` 按条目风险档定档，与 `expectedBytes` 无关。
    pub confirmation: StorageConfirmLevelDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageReclaimPlanDto {
    /// 不透明，前端只回传不解析。
    pub plan_id: String,
    pub sampled_at: i64,
    pub entries: Vec<StorageReclaimPlanEntryDto>,
    /// 任一 `eligible` 条目算不出则 `null`。
    pub total_reclaimable_bytes: Option<u64>,
    pub complete: bool,
}

/// 单项回收请求：`action` 必须与计划一致，防止「预览 30 天、执行孤儿」式偷换。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageReclaimEntryRequestDto {
    pub key: String,
    pub action: StorageReclaimActionDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageReclaimRequestDto {
    /// 必须命中未过期计划。
    pub plan_id: String,
    /// 非空；每项必须在该计划的条目集合内。
    pub entries: Vec<StorageReclaimEntryRequestDto>,
    /// 必须 `true`。
    pub confirmed: bool,
}

/// 失败原因：`message` 人类可读，不含绝对路径与文件名。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageReclaimErrorDto {
    pub code: StorageReclaimErrorCodeDto,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageReclaimEntryResultDto {
    pub key: String,
    pub action: StorageReclaimActionDto,
    pub status: StorageReclaimStatusDto,
    pub removed_count: u64,
    pub removed_bytes: u64,
    pub failed_count: u64,
    /// 执行时被判据排除（如刚好被引用），不计失败。
    pub skipped_count: u64,
    /// 只含失败原因，不含路径。
    pub errors: Vec<StorageReclaimErrorDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageReclaimResultDto {
    pub plan_id: String,
    pub executed_at: i64,
    /// 与请求顺序一致，不因单条失败中断。
    pub entries: Vec<StorageReclaimEntryResultDto>,
}

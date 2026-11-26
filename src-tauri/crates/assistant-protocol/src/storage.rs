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
    /// core 条目在前、已挂载分区自述条目紧随其后；未挂载分区**不出行**（不是占位行）。
    #[serde(default)]
    pub entries: Vec<StorageEntryDto>,
    /// 对账余量快照；恒存在（未扫过时 `status = neverRun`）。
    #[serde(default)]
    pub inventory: StorageInventoryDto,
    /// 除 `E12` 外全部条目 `availability ∈ {available, missing}` 时为 `true`。
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

/// 条目可用性；封闭词表跨 tab 共用（I54 §2.4），八个取值全部在用（I79 §7.3）。
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
    /// 整根清空：删基根下全部子项、保留基根本身（I77 §4.1）。
    ///
    /// 与 `DeleteOrphans` 的区别在于判据来源：不做磁盘侧差集、不读会话表，
    /// 删除权完全交给用户显式意图（含活跃会话段与 `shared`）。
    ClearAll,
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
    /// 已登记、本次不删、下次启动执行（`E14` WebView2 profile 独占句柄，§13.3.5）。
    ///
    /// 与 `Skipped` 的区别：`Skipped` 是「没做也不需要做」，`Scheduled` 是
    /// 「已落标记、有明确后续执行点」。合并会让用户以为请求被丢弃。
    Scheduled,
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
    pub includes_derived: bool,
    pub counts_self_logs: bool,
    /// 该条数字的观测时刻（秒，I79 §7.1）：`Inline` = 本次响应的 `sampledAt`；
    /// `Deferred` 首次尚未算完时为 `null`。键恒在、值可为 `null`。
    pub observed_at: Option<i64>,
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

/// 单个 scope 的对账合计（I67 §5.1）：`scanBytes` 是该 scope 的分母。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageInventoryScopeTotalsDto {
    pub scan_bytes: Option<u64>,
    pub scan_count: Option<u64>,
    pub declared_bytes: Option<u64>,
    pub declared_count: Option<u64>,
    pub residual_bytes: Option<u64>,
    pub residual_count: Option<u64>,
    pub unregistered_bytes: Option<u64>,
    pub unregistered_count: Option<u64>,
    /// `inventory = false` 首段（`E12`）的扫描值：计入分母、不参与减法（I67 §3.5）。
    pub unreconciled_bytes: Option<u64>,
    pub unreconciled_count: Option<u64>,
    /// 首层不可命名条目的字节 / 条数（I67 §3.4）。
    pub unnameable_bytes: Option<u64>,
    pub unnameable_count: Option<u64>,
}

/// 全类余量合计（I67 §5.1）：`partial` 为 `true` 时数字是下界。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageInventoryTotalsDto {
    pub scan_bytes: Option<u64>,
    pub scan_count: Option<u64>,
    pub residual_bytes: Option<u64>,
    pub residual_count: Option<u64>,
    /// 任一贡献项为 `null`：本结构数字是下界（I67 §3.6）。
    pub partial: bool,
    pub data: StorageInventoryScopeTotalsDto,
    pub local: StorageInventoryScopeTotalsDto,
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
    /// 恒存在（键恒在）；`status = neverRun` 时内部数字全 `null`、`partial = false`。
    pub totals: StorageInventoryTotalsDto,
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
            totals: StorageInventoryTotalsDto::default(),
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

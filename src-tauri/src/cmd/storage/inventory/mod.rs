//! 对账余量扫描与快照状态机（I61 §3.11）。
//!
//! 三条硬约束（G2 例外边界）：①**只读**——不删、不移、不写（连应用日志都不写，
//! 避免污染 `E12` 的自指字节）；②产出**绝不回流** `entries`（G10）；③有界
//! （深度 / 条目数 / 耗时）。扫描结果只作为「被减数」，减数来自条目本身。
//!
//! 减法放在**读路径**而不是扫描任务里：登记值必须与同一响应里的 `entries` 同源，
//! 若在后台任务里先减一遍，前端看到的 `entries` 与 `inventory` 会来自两个时刻。
//!
//! 目录枚举（`std::fs` 遍历）全仓库唯一落点是子模块 [`walk`]；[`classify`] / [`subtract`] 是纯函数，
//! [`cache`] 只持有快照与状态机。

mod cache;
mod classify;
mod subtract;
mod walk;

use std::time::{SystemTime, UNIX_EPOCH};

use crate::app_dirs::registry::RootScope;
use crate::protocol::dto::{StorageInventoryClassDto, StorageInventoryItemDto};

pub use cache::{trigger_scan, InventoryCache};
pub use classify::classify;
pub use subtract::subtract_declared;
pub use walk::scan;

/// 快照 TTL（§3.11.4）：扫描比观测贵得多，余量变化频率低。
pub const SCAN_TTL_SECONDS: i64 = 300;

pub(super) fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| i64::try_from(since.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// 单个首段的扫描结果；数字为 `None` 表示「知道有、不知多大」（触界 / 读不到）。
#[derive(Debug, Clone, PartialEq)]
pub struct RawNode {
    pub name: String,
    pub scope: RootScope,
    pub class: StorageInventoryClassDto,
    /// `declared` / `optionalNotMounted` 给出对应 `E` 编号，其余 `None`。
    pub entry: Option<&'static str>,
    pub scan_bytes: Option<u64>,
    pub scan_count: Option<u64>,
    pub children: Vec<StorageInventoryItemDto>,
    pub truncated: bool,
}

/// 单个 scope 的分母与不可命名部分（T1）：均在扫描时一次遍历产出，不重扫。
///
/// `scan_bytes` / `scan_count` 是该 scope 全部首层节点（含 `inventory = false` 的 `E12`）
/// 与不可命名条目的字节 / 条数之和；`None` 表示「有触界 / 读不到的部分」（下界，§3.6）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScopeScanTotals {
    pub scan_bytes: Option<u64>,
    pub scan_count: Option<u64>,
    pub unnameable_bytes: Option<u64>,
    pub unnameable_count: Option<u64>,
}

/// 两个基根各自的分母（T1）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScanTotals {
    pub data: ScopeScanTotals,
    pub local: ScopeScanTotals,
}

/// 一次扫描的产物：节点 + 全局触界标记 + 分母。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScanResult {
    pub nodes: Vec<RawNode>,
    pub partial: bool,
    pub truncated: Vec<String>,
    pub totals: ScanTotals,
    /// 任一贡献项（节点扫描值 / 不可命名部分）为 `null`：`totals` 为下界（§3.6）。
    pub totals_partial: bool,
}

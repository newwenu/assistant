//! 条目并行采样装配（I61 §3.10 + I79 §5.1）：按采样档分流。
//!
//! 三条纪律：①`E1` 与顶层字段同源，由调用方传入同一份 `list_all` 结果，这里不重扫；
//! ②`Inline` 条目并行现场采样，单条超时只降级该条（`unavailable` + 「统计超时」），不拖整页；
//! ③超时只是放弃等待，任务线程仍会跑完——因此 owner stats 必须只读、幂等、不持锁。
//!
//! 分档（I79 §4.1）：`Inline` 在响应内算，`SAMPLE_TIMEOUT` 是「声明说谎探针」；
//! `Deferred` 页面档只读缓存（缺失 / 过期由 [`cache::trigger_stats`] 后台补算），
//! 计划档现场采样（预算 [`PLAN_SAMPLE_TIMEOUT`]）且不写缓存，保住计划新鲜度（INV3）。
//!
//! 本模块不读配置：策略一律由 owner 自述（`result_retention` / 分区 `policy()`），
//! 管理器不得成为第二个配置读取入口（§3.5 硬约束 1）。
//!
//! 条目清单不再固定条数：core 条目恒在，可选分区条目由已注册分区自述，
//! 因此采样产物 [`Sampled`] 同时带出「视图清单 + 对齐的样本」，下游一律按清单寻址（X15）。
//!
//! 子模块按职责划分：`collect` 采样编排、`cache` 统计缓存与后台补算、`rows` 各条目取数、
//! `stats` 观测结构适配、`policy` 策略快照、`timeout` 超时包装、`sections` 可选分区取数。

mod cache;
mod collect;
mod policy;
mod rows;
mod sections;
mod stats;
mod timeout;

use std::collections::HashSet;
use std::sync::Arc;

use assistant_storage::attachment_store::{AttachmentEntry, AttachmentStore};
use assistant_tools::{ToolResultStorage, ToolScratchStore};

use crate::app_dirs::AppDirs;
use crate::cmd::storage::entries::{EntrySample, EntryView};
use crate::cmd::storage::section::StorageSectionHandle;
use crate::db_state::DbState;

pub use cache::{trigger_stats, EntryStatsCache};
pub use collect::{collect, collect_with};
pub use stats::attachment_reason;

/// 单条 `Inline` 观测上限（§3.10）：超过即降级，绝不阻塞整页。
///
/// I79 起语义收窄为「声明说谎探针」：该条声明 `Inline` 却超过本预算，说明 `stats_mode`
/// 填错或 owner 实现退化，应改档或修 owner，而不是调大本常量（§4.4）。
pub const SAMPLE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);

/// 计划模式下 `Deferred` 条目的现场采样预算（I79 §8.2）：比页面档宽，仍要保证计划新鲜。
pub const PLAN_SAMPLE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(2000);

/// 缓存新鲜期（I79 §5.3）：到期即读路径触发补算；`staleReport` 仍展示旧数字。
pub const ENTRY_STATS_TTL_SECONDS: i64 = 60;

/// 失败退避（I79 §5.3）：避免每次重拉都重试。
pub const ENTRY_STATS_RETRY_SECONDS: i64 = 30;

/// 单条后台任务的 **UI 占位** 时限（I79 §5.3）：只写 `unavailable`，不终止任务。
pub const ENTRY_STATS_JOB_TIMEOUT_MS: u64 = 5000;

/// 后台并发上限（I79 §5.3）：避免多个整树遍历同时抢磁盘。
pub const ENTRY_STATS_MAX_INFLIGHT: usize = 2;

/// `running` 租约（I79 §5.3）：owner 真卡死时到期允许重试，避免永久占满 permit。
pub const ENTRY_STATS_LEASE_SECONDS: i64 = 120;

/// 采样模式（I79 §5.1）：页面快照与回收计划对 `Deferred` 条目走不同路径。
#[derive(Debug, Clone, Copy)]
pub enum SampleMode<'a> {
    /// 页面快照：`Inline` 现场采样，`Deferred` 读缓存。
    Page,
    /// 回收计划：`Deferred` 也现场采样；`force = None` 表示清单内全部条目。
    Plan { force: Option<&'a [&'static str]> },
}

impl SampleMode<'_> {
    /// 该条目在本次采样里是否必须现场算（`Inline` 恒是；`Deferred` 只在计划档强制）。
    fn forces(&self, key: &str) -> bool {
        match self {
            SampleMode::Page => false,
            SampleMode::Plan { force } => {
                force.is_none_or(|keys| keys.iter().copied().any(|forced| forced == key))
            }
        }
    }
}

/// 采样输入：句柄一律由调用方给，采样层不碰 `AppHandle`（计划命令复用同一实现）。
///
/// `sections` / `tool_results` 用 `Arc` 句柄而非引用：阻塞型 owner 要搬进
/// `spawn_blocking`，闭包必须 `'static`；`db` 用可克隆的 `DbState`
/// （内含 `Arc<Mutex<Connection>>` + 文件路径，只读 `metadata`）。
pub struct SampleInputs<'a> {
    pub dirs: &'a AppDirs,
    pub store: &'a AttachmentStore,
    pub entries: &'a [AttachmentEntry],
    /// `E1` 的孤儿判据：调用方已加载的引用集合（与顶层字段同一份，不重查 DB）。
    pub referenced: Option<&'a HashSet<String>>,
    /// 已注册的可选分区快照：空 = 关闭特性构建，条目清单里只剩 core 条目。
    pub sections: Vec<StorageSectionHandle>,
    pub tool_results: Option<Arc<dyn ToolResultStorage>>,
    /// `E19` 的观测句柄（`boot::tool_exec(app).scratch()`，与 `E3` 同款双重暴露）。
    pub scratch: Option<Arc<dyn ToolScratchStore>>,
    pub db: Option<DbState>,
    /// `Deferred` 条目的缓存：页面档必给；计划档恒 `None`（计划不读也不写缓存，INV3）。
    pub stats: Option<EntryStatsCache>,
    /// 本次响应的统一时刻：`Deferred` 读映射与 TTL 判据都按它比较。
    pub now: i64,
}

/// 采样产物：视图清单与样本严格一一对齐（同一次 `entries::views()` 产出）。
pub struct Sampled {
    pub views: Vec<EntryView>,
    pub samples: Vec<EntrySample>,
    /// 需要后台补算的 `Deferred` 条目（`triggers` 只由页面档产生，计划档恒空）。
    ///
    /// 采样层不碰 `AppHandle`：由命令层拿本列表调 [`trigger_stats`]（幂等，I79 §5.1 第 4 步）。
    pub triggers: Vec<&'static str>,
}

//! 条目并行采样装配（I61 §3.10）：每条一个独立任务 + 单条 500 ms 超时。
//!
//! 三条纪律：①`E1` 与顶层字段同源，由调用方传入同一份 `list_all` 结果，这里不重扫；
//! ②除 `E1` 外全部并行，单条超时只降级该条（`unavailable` + 「统计超时」），不拖整页；
//! ③超时只是放弃等待，任务线程仍会跑完——因此 owner stats 必须只读、幂等、不持锁。
//!
//! 本模块不读配置：策略一律由 owner 自述（`result_retention` / 分区 `policy()`），
//! 管理器不得成为第二个配置读取入口（§3.5 硬约束 1）。
//!
//! 条目清单不再固定条数：core 条目恒在，可选分区条目由已注册分区自述，
//! 因此采样产物 [`Sampled`] 同时带出「视图清单 + 对齐的样本」，下游一律按清单寻址（X15）。
//!
//! 子模块按职责划分：`collect` 采样编排、`rows` 各条目取数、`stats` 观测结构适配、
//! `policy` 策略快照、`timeout` 超时包装、`sections` 可选分区取数。

mod collect;
mod policy;
mod rows;
mod sections;
mod stats;
mod timeout;

use std::collections::HashSet;
use std::sync::Arc;

use assistant_storage::attachment_store::{AttachmentEntry, AttachmentStore};
use assistant_tools::ToolResultStorage;

use crate::app_dirs::AppDirs;
use crate::cmd::storage::entries::{EntrySample, EntryView};
use crate::cmd::storage::section::StorageSectionHandle;
use crate::db_state::DbState;

pub use collect::{collect, collect_with};
pub use stats::attachment_reason;

/// 单条观测上限（§3.10）：超过即降级，绝不阻塞整页。
pub const SAMPLE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);

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
    pub db: Option<DbState>,
}

/// 采样产物：视图清单与样本严格一一对齐（同一次 `entries::views()` 产出）。
pub struct Sampled {
    pub views: Vec<EntryView>,
    pub samples: Vec<EntrySample>,
}

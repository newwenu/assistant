//! 计划 / 执行骨架（I61 §5.2、§6.1、§6.2）：纯函数，句柄一律由调用方给。
//!
//! 三条纪律：①计划**无副作用**，自己重新采样，不复用整页快照的数字（否则「预览」
//! 与「执行」会各说一套）；②执行必须校验 `planId` 未过期、条目在计划内、`action`
//! 与计划一致、且落在 §3.3 白名单，任一不符**整体拒绝**；③单条失败不中断其余条目。
//!
//! 子模块按职责划分：[`store`] 只持有计划与串行闸门，[`plan`] 只读预览，
//! [`execute`] 校验 + 路由，[`executors`] 各条目执行器，[`report`] 结果装配。

mod execute;
mod executors;
mod plan;
mod report;
mod store;

use std::sync::Arc;

use assistant_storage::attachment_store::AttachmentStore;
use assistant_tools::ToolResultStorage;

use crate::app_dirs::AppDirs;
use crate::cmd::storage::section::StorageSectionHandle;
use crate::setup::DbState;

pub use execute::execute_storage_reclaim;
pub use plan::plan_storage_reclaim;
pub use store::{CachedPlan, CachedPlanEntry, ExecutionGuard, PlanLookupError, ReclaimPlanStore};

/// 计划有效期（§5.2.1）：`planId` 只在 10 分钟内可执行，过期零副作用。
pub const PLAN_TTL_SECONDS: i64 = 600;

/// 执行侧依赖：全部由命令层从 `AppHandle` 取好后传入，本模块不碰 `AppHandle`。
///
/// `dirs` / `attachments` 恒装配（`E1` / `E2` 是 Always 挂载）；`sections` / `tool_results`
/// / `db` 属可选能力，缺装配时对应条目整体不存在（未挂载 = 行不存在），不影响计划出数。
pub struct ReclaimPorts<'a> {
    pub plans: &'a ReclaimPlanStore,
    pub dirs: &'a AppDirs,
    pub attachments: &'a AttachmentStore,
    /// 已注册的可选分区快照：空 = 关闭特性构建，清单里只剩 core 8 条。
    pub sections: Vec<StorageSectionHandle>,
    pub tool_results: Option<Arc<dyn ToolResultStorage>>,
    pub db: Option<DbState>,
    /// 本次请求的统一时间戳：计划 `sampledAt` 与执行「判据排除」都按它比较。
    pub now: i64,
}

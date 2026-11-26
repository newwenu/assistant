// 参考: docs/store/I75-startup-layering-spec.md §4.1 / §4.3 / §9 — 资源表与失败语义
//
// 本文件只放声明，不放逻辑：执行顺序 = 本表声明顺序，`run_boot` 不写顺序。

use std::any::Any;
use std::sync::Arc;

use crate::boot::ctx::StartupCtx;

/// 泳道。分派由 `run_boot` / `run_idle` 按本列读取，不存在第二份预热清单。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    Boot,
    Idle,
    OnDemand,
}

impl Lane {
    pub fn label(self) -> &'static str {
        match self {
            Lane::Boot => "boot",
            Lane::Idle => "idle",
            Lane::OnDemand => "on-demand",
        }
    }

    pub fn dto(self) -> assistant_protocol::startup::LaneDto {
        use assistant_protocol::startup::LaneDto;
        match self {
            Lane::Boot => LaneDto::Boot,
            Lane::Idle => LaneDto::Idle,
            Lane::OnDemand => LaneDto::OnDemand,
        }
    }
}

/// 装配产物。类型擦除只为放进同一个槽，访问侧一律经 `access::*` 还原。
pub type CapValue = Arc<dyn Any + Send + Sync>;

/// 能力错误。`NotReady` 不是错误状态，只表示「还没装配」。
#[derive(Debug, Clone)]
pub enum CapErr {
    NotReady {
        id: &'static str,
    },
    Build {
        id: &'static str,
        message: String,
        retryable: bool,
    },
}

impl CapErr {
    pub fn not_ready(id: &'static str) -> Self {
        Self::NotReady { id }
    }

    pub fn build(id: &'static str, message: impl Into<String>) -> Self {
        Self::Build {
            id,
            message: message.into(),
            retryable: true,
        }
    }

    pub fn id(&self) -> &'static str {
        match self {
            Self::NotReady { id } | Self::Build { id, .. } => *id,
        }
    }

    pub fn retryable(&self) -> bool {
        match self {
            Self::NotReady { .. } => true,
            Self::Build { retryable, .. } => *retryable,
        }
    }
}

impl std::error::Error for CapErr {}

impl std::fmt::Display for CapErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotReady { id } => write!(f, "capability {id} not ready"),
            Self::Build { id, message, .. } => write!(f, "capability {id} build failed: {message}"),
        }
    }
}

/// 幂等装配：不读配置、只从 `ctx` 取前序产物；`manage` 之外的全局副作用由 registry 统一收敛。
pub type BuildFn = fn(&StartupCtx<'_>) -> Result<CapValue, CapErr>;

pub struct Res {
    pub id: &'static str,
    pub lane: Lane,
    pub needs: &'static [&'static str],
    /// 0 = 未测量；非零项由 G8 断言 p95。
    pub budget_ms: u32,
    pub build: BuildFn,
}

/// 表序即启动顺序（§9 接入矩阵现状 1:1 复刻）。
pub static RESOURCES: &[Res] = &[
    Res {
        id: "dirs",
        lane: Lane::Boot,
        needs: &[],
        budget_ms: 0,
        build: crate::boot::build::dirs::build,
    },
    Res {
        id: "boot_conf",
        lane: Lane::Boot,
        needs: &["dirs"],
        budget_ms: 0,
        build: crate::boot::build::conf::build,
    },
    Res {
        id: "i18n",
        lane: Lane::Boot,
        needs: &["boot_conf"],
        budget_ms: 0,
        build: crate::boot::build::state::build_i18n,
    },
    Res {
        id: "feature_on_setup",
        lane: Lane::Boot,
        needs: &["dirs", "boot_conf"],
        budget_ms: 0,
        build: crate::boot::build::state::build_feature_on_setup,
    },
    Res {
        id: "storage",
        lane: Lane::Boot,
        needs: &["dirs", "boot_conf"],
        budget_ms: 0,
        build: crate::boot::build::storage::build,
    },
    Res {
        id: "attachment_store",
        lane: Lane::Boot,
        needs: &["dirs"],
        budget_ms: 0,
        build: crate::boot::build::attachments::build,
    },
    Res {
        id: "runtime_state",
        lane: Lane::Boot,
        needs: &["storage"],
        budget_ms: 0,
        build: crate::boot::build::state::build_runtime_state,
    },
    Res {
        id: "tool_runtime",
        lane: Lane::Idle,
        needs: &["dirs", "boot_conf"],
        budget_ms: 0,
        build: crate::boot::build::tools::build,
    },
    Res {
        id: "tray",
        lane: Lane::Boot,
        needs: &["boot_conf", "i18n"],
        budget_ms: 0,
        build: crate::boot::build::shell::build_tray,
    },
    Res {
        id: "orb_shell",
        lane: Lane::Boot,
        needs: &["dirs", "boot_conf"],
        budget_ms: 0,
        build: crate::boot::build::windows::build_orb_shell,
    },
    Res {
        id: "screen_inspection",
        lane: Lane::Boot,
        needs: &[],
        budget_ms: 0,
        build: crate::boot::build::shell::build_screen_inspection,
    },
    Res {
        id: "shortcuts",
        lane: Lane::Boot,
        needs: &["boot_conf"],
        budget_ms: 0,
        build: crate::boot::build::shell::build_shortcuts,
    },
    Res {
        id: "window_main",
        lane: Lane::Idle,
        needs: &["dirs", "boot_conf"],
        budget_ms: 0,
        build: crate::boot::build::windows::build_window_main,
    },
    Res {
        id: "storage_maintenance",
        lane: Lane::Idle,
        needs: &["dirs", "storage", "attachment_store", "boot_conf"],
        budget_ms: 0,
        build: crate::boot::build::maintenance::build,
    },
    Res {
        id: "mcp_start",
        lane: Lane::Idle,
        needs: &["tool_runtime"],
        budget_ms: 0,
        build: crate::boot::build::mcp::build,
    },
];

pub fn find(id: &str) -> Option<&'static Res> {
    RESOURCES.iter().find(|res| res.id == id)
}

/// 唯一主入口：Orb 是启动期唯一可见窗口，T_visible 的终点（§3.5）。
pub const PRIMARY_ENTRY: &[&str] = &["orb_shell"];
/// 失败即终止启动：继续跑会写错数据，或失去唯一入口。
pub const BOOT_REQUIRED: &[&str] = &["dirs", "storage", "runtime_state", "i18n"];
/// 失败可降级：功能缺失但应用可用，可观测且不级联。
pub const OBSERVABLE: &[&str] = &[
    "boot_conf",
    "attachment_store",
    "feature_on_setup",
    "tool_runtime",
    "tray",
    "window_main",
    "storage_maintenance",
];
/// 尽力而为：只 warn。
pub const BEST_EFFORT: &[&str] = &["screen_inspection", "shortcuts", "mcp_start"];
/// 降级对用户可操作，需要前端提示（P3 起为 `tool_runtime`）。
pub const ACTIONABLE: &[&str] = &["tool_runtime"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Fatal,
    Degraded,
    Ignore,
}

/// 失败语义只由集合成员派生：表项不存 severity 字段。
pub fn severity(id: &str) -> Severity {
    if PRIMARY_ENTRY.contains(&id) || BOOT_REQUIRED.contains(&id) {
        Severity::Fatal
    } else if OBSERVABLE.contains(&id) {
        Severity::Degraded
    } else {
        Severity::Ignore
    }
}

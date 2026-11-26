// 参考: docs/store/I75-startup-layering-spec.md §5 — 对外只做导出，无业务分支

mod access;
mod build;
mod conf;
mod ctx;
mod registry;
mod resources;
mod runner;

pub use access::{
    boot_conf, dirs, mcp_manager, pending_approval, pending_question, permission_audit, tool_exec,
    tool_scratch, tools,
};
pub use conf::BootConf;
pub(crate) use ctx::epoch_ms;
pub use ctx::{BootClock, Outcome, StartupCtx, StartupReport};
pub use registry::{ensure, run_idle, Registry};
pub use resources::{
    find, severity, CapErr, CapValue, Lane, Res, Severity, ACTIONABLE, BEST_EFFORT, BOOT_REQUIRED,
    OBSERVABLE, PRIMARY_ENTRY, RESOURCES,
};
pub use runner::run_boot;

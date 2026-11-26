// 参考: docs/store/I75-startup-layering-spec.md §5 — 窗口模块对外导出

pub(crate) mod create;
pub(crate) mod geometry;
pub mod mode;
pub mod orb_panel;
pub(crate) mod placement;
pub(crate) mod spec;

pub use create::{
    apply_orb_window_attrs, destroy_all, get_or_error, show_main_window, show_or_create,
    show_or_create_main, toggle,
};
pub use geometry::persist_main_geometry;
pub use mode::{effective_on_top, on_window_event};

// 参考: I78-serial-debug-spec.md §3.3 / C10 — 事件名与发射点都在特性目录内，core 事件表零改动

mod ports;
pub(crate) mod reader;
mod session;

pub use ports::*;
pub use session::*;

/// 接收批事件：定向主窗口，不广播
pub const SERIAL_DEBUG_RX: &str = "serial_debug:rx";

/// 会话状态事件：广播（打开成功 / 关闭 / 异常转 closed）
pub const SERIAL_DEBUG_STATUS: &str = "serial_debug:status";

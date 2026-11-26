pub mod backend;
pub mod error;
pub mod event;
mod reader;
pub mod session;
pub mod state;

pub use backend::{PortBackend, PortHandle, SerialPortBackend};
pub use error::SerialError;
pub use event::{EventSink, SerialEvent};
pub use session::SerialRuntime;
pub use state::SerialConfig;

/// 端口读超时：必须小于批边界，否则读超时成为实际节拍（空闲时最坏等一整个读超时才发批）
pub const READ_TIMEOUT_MS: u64 = 8;

/// 批边界之一：距批首字节的时长上限
pub const BATCH_INTERVAL_MS: u64 = 16;

/// 批边界之一：单批字节上限
pub const BATCH_BYTES: usize = 4096;

/// 波特率校验上界（自定义波特率是调试刚需，不做常用值白名单）
pub const BAUD_RATE_MAX: u32 = 4_000_000;

pub mod branch;
pub mod chat;
pub mod clock;
pub mod config;
pub mod orb;
pub mod os_tool;
pub mod settings;
pub mod storage;

/// Unix **毫秒**时间戳。wire 上为 JSON number，最大可精确表示值为 2^53-1。
pub type Millis = i64;

pub use branch::*;
pub use chat::*;
pub use clock::*;
pub use config::*;
pub use orb::*;
pub use os_tool::*;
pub use settings::*;
pub use storage::*;

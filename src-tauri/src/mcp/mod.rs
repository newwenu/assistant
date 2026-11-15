//! MCP 宿主侧装配（P7-C4）：进程传输、配置加解密与启动序列。
//!
//! 纯协议与状态机在 `assistant-tools::mcp`；本模块只负责「接上真实世界」：
//! 子进程、磁盘行、加密与日志。

pub mod config;
pub mod transport;

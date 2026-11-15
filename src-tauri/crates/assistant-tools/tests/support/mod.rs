//! 集成测试共用的夹具。
//!
//! 只放「同一语义、多平台各写一份后已经漂移」的 helper；平台语义本身不同的断言
//! 仍留在各自用例里用 `cfg` 门控。`tests/support/` 不会被 cargo 当作独立 test target。

#![allow(dead_code)]

pub mod link;

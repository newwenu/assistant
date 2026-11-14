//! 测试共用件：跨多个测试目标重复出现的消息构造、fixture 与流替身。
//!
//! 只在声明了 `mod common;` 的测试目标里编译；子模块自带 `#![allow(dead_code)]`，
//! 因为每个目标只会用到其中一部分。
//!
//! 按依赖分层，便于不带 `test-doubles` feature 的目标也能复用：
//! - `chat` / `modes` / `rows` / `harness` / `scan` 只依赖常规 API，任何目标可用；
//! - `fixtures` / `stream` / `runner` 依赖 `ports::test_doubles`，随 feature 门控。

pub mod chat;
pub mod harness;
pub mod modes;
pub mod rows;
pub mod scan;

#[cfg(feature = "test-doubles")]
pub mod fixtures;

#[cfg(feature = "test-doubles")]
pub mod runner;

#[cfg(feature = "test-doubles")]
pub mod stream;

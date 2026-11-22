//! 测试夹具：创建链接，只在测试构建时存在。
//!
//! 直接复用集成测试那份实现：`#[cfg(test)]` 项对集成测试不可见，两处各写一份必然
//! 再次漂移（历史上已漂出「只调 `symlink_dir` 导致用例静默跳过」）。用 `#[path]`
//! 指向同一文件，平台分派只写一次。
//!
//! 由 `os::test_support` 以 `#[path]` 引入，门控为 `cfg(test)` 或 `test-support`
//! feature——后者供其它 package 的集成测试通过 `assistant_tools::os::test_support`
//! 使用，避免跨 package 的 `#[path]`（rust-analyzer 不认，会误报 E0583）。

#[path = "../../../tests/support/link.rs"]
pub mod link;

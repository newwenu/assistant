//! 内联测试共用的夹具，只在 `cargo test` 编译 lib 时存在。
//!
//! 直接复用集成测试那份实现：`#[cfg(test)]` 项对集成测试不可见，两处各写一份必然
//! 再次漂移（历史上已漂出「只调 `symlink_dir` 导致用例静默跳过」）。用 `#[path]`
//! 指向同一文件，平台分派只写一次。

#[path = "../../tests/support/link.rs"]
pub(crate) mod link;

// 参考: docs/store/feature-optional-binary-spec.md §4.2 R8 — 插件边界集中在 feature_plugins/，
// 声明收敛于此，`lib.rs` 只留一行 `mod feature_plugins;`（装配层不再随特性数增长）

pub(crate) mod serial_debug_plugin;
pub(crate) mod timekeeper_plugin;
pub(crate) mod vault_plugin;

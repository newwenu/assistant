# I7 — cmd.rs 模块拆分 任务清单

> 规格：[I7-cmd-split-spec.md](./I7-cmd-split-spec.md)
> 前置：I6 ✅

---

## 阶段 A — 创建子模块文件

- [x] A1. 创建 `cmd/mod.rs`：声明 8 个子模块 + `pub use` re-export 所有 command + `perform_exit` + `get_db_conn`
- [x] A2. 创建 `cmd/config.rs`：迁移 `get_app_conf` / `set_app_conf` / `get_orb_conf` / `set_orb_conf` / `set_theme` / `emit_theme_changed` + `apply_autostart` + `apply_orb_click_through`
- [x] A3. 创建 `cmd/orb.rs`：迁移 `get_mouse_position` / `toggle_orb_visibility` / `set_orb_click_through` / `get_screen_size` / `snap_orb_to_screen` / `snap_orb_to_edge` / `save_orb_position` + `save_orb_position_sync` + `ORB_CLICK_THROUGH` + `ORB_SNAP_CANCEL`
- [x] A4. 创建 `cmd/provider.rs`：迁移 `list_providers` / `save_provider` / `delete_provider` / `list_models_cmd` / `preview_models_cmd` + `mask_api_key_encrypted` + `build_provider_config`
- [x] A5. 创建 `cmd/conversation.rs`：迁移 18 个对话/消息命令
- [x] A6. 创建 `cmd/chat.rs`：迁移 `send_chat_message` / `cancel_chat`
- [x] A7. 创建 `cmd/tool.rs`：迁移 `record_tool_usage` / `get_recent_tools` / `get_favorite_tools` / `toggle_tool_favorite` / `set_favorite_tools_order`
- [x] A8. 创建 `cmd/agent_mode.rs`：迁移 `list_agent_modes` / `save_agent_mode` / `delete_agent_mode` / `reset_builtin_mode`
- [x] A9. 创建 `cmd/app.rs`：迁移 `app_quit` / `perform_exit` / `reload_shortcuts` / `get_app_version` / `get_rustc_version` / `set_launch_at_login`

## 阶段 B — 跨域引用处理

- [x] B1. `config.rs` 中 `apply_orb_click_through` 改为调用 `super::orb::apply_orb_click_through`（读写 orb 模块的 static）
- [x] B2. `chat.rs` 中 `build_provider_config` 改为调用 `super::provider::build_provider_config`
- [x] B3. 所有子模块中 `get_db_conn` 改为调用 `super::get_db_conn`

## 阶段 C — 清理旧文件

- [x] C1. 删除 `cmd.rs`（根目录旧单文件）
- [x] C2. 确认 `lib.rs` 中 `mod cmd;` 无需改动（Rust 自动识别 `cmd/mod.rs`）

## 阶段 D — 编译验证

- [x] D1. `cargo check` 通过
- [x] D2. `cargo clippy` 无新增 warning
- [x] D3. 确认 `lib.rs` invoke_handler 中所有 `cmd::xxx` 路径不变
- [x] D4. 确认 `tray.rs` 中 `crate::cmd::perform_exit` 路径不变

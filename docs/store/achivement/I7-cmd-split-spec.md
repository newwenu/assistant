# I7 — cmd.rs 模块拆分 规格说明

## 1. 交付目标

将 `src-tauri/src/cmd.rs`（1211 行、40+ command）按业务域拆分为 `cmd/` 目录下 8 个子模块，提升可维护性和增量编译效率。

> **前置依赖**：I6 ✅
> **核心变更**：仅文件结构重组，零功能变更、零 API 变更。

## 2. 当前问题

| 问题 | 说明 |
|------|------|
| 单文件过大 | 1211 行，`send_chat_message` 一个函数 250 行与 `toggle_tool_favorite` 5 行混在一起 |
| 6 域混杂 | config / orb / provider / conversation / chat / tool / agent_mode / app 无边界 |
| 增量编译无效 | 改任何命令全量重编 cmd 单元 |

## 3. 目标结构

```
src-tauri/src/
  cmd.rs          → 删除
  cmd/
    mod.rs          ← 子模块声明 + re-export + get_db_conn
    config.rs       ← AppConf/OrbConf/Theme 6 命令 + apply_autostart + apply_orb_click_through
    orb.rs          ← 鼠标/屏幕/吸附/穿透/位置 7 命令 + save_orb_position_sync + 2 AtomicBool
    provider.rs     ← Provider CRUD + 模型列表 5 命令 + mask_api_key_encrypted + build_provider_config
    conversation.rs ← 对话/消息 CRUD 18 命令
    chat.rs         ← 流式发送 + 取消 2 命令
    tool.rs         ← 工具记录/收藏 5 命令
    agent_mode.rs   ← Agent 模式管理 4 命令
    app.rs          ← 退出/版本/快捷键/自启 5 命令 + perform_exit
```

## 4. 模块内容映射

### 4.1 `mod.rs`

- 声明 8 个子模块
- `pub use` re-export 所有 command 函数 + `perform_exit`
- `pub(crate) fn get_db_conn` — 唯一跨域共享基础设施函数

### 4.2 `config.rs`（6 命令）

| 函数 | 可见性 |
|------|--------|
| `get_app_conf` | pub |
| `set_app_conf` | pub |
| `get_orb_conf` | pub |
| `set_orb_conf` | pub |
| `set_theme` | pub |
| `emit_theme_changed` | pub |
| `apply_autostart` | 私有 |
| `apply_orb_click_through` | 私有（调用 `super::orb::ORB_CLICK_THROUGH`） |

依赖：`AppConf`, `OrbConf`, `WINDOW_MAIN`, `WINDOW_ORB`, `super::orb`

### 4.3 `orb.rs`（7 命令）

| 函数 | 可见性 |
|------|--------|
| `get_mouse_position` | pub |
| `toggle_orb_visibility` | pub |
| `set_orb_click_through` | pub |
| `get_screen_size` | pub |
| `snap_orb_to_screen` | pub |
| `snap_orb_to_edge` | pub |
| `save_orb_position` | pub |
| `save_orb_position_sync` | 私有 |
| `ORB_CLICK_THROUGH` | pub(crate) static |
| `ORB_SNAP_CANCEL` | 私有 static |

依赖：`placement`, `OrbConf`, `WINDOW_ORB`, `crate::window`

### 4.4 `provider.rs`（5 命令）

| 函数 | 可见性 |
|------|--------|
| `list_providers` | pub |
| `save_provider` | pub |
| `delete_provider` | pub |
| `list_models_cmd` | pub |
| `preview_models_cmd` | pub |
| `mask_api_key_encrypted` | 私有 |
| `build_provider_config` | pub(crate)（chat.rs 需要） |

### 4.5 `conversation.rs`（18 命令）

`create_conversation`, `list_conversations`, `get_conversation`, `delete_conversation`, `rename_conversation`, `update_conversation_model`, `update_conversation_provider`, `get_conversation_paginated`, `delete_message_persist`, `search_conversations`, `export_conversation`, `toggle_conversation_pin`, `duplicate_conversation`, `batch_delete_conversations`, `purge_conversations`, `edit_message`, `regenerate_message`, `get_conversation_token_usage`

### 4.6 `chat.rs`（2 命令）

`send_chat_message`, `cancel_chat`

依赖：`ChatTasks`, `super::provider::build_provider_config`

### 4.7 `tool.rs`（5 命令）

`record_tool_usage`, `get_recent_tools`, `get_favorite_tools`, `toggle_tool_favorite`, `set_favorite_tools_order`

### 4.8 `agent_mode.rs`（4 命令）

`list_agent_modes`, `save_agent_mode`, `delete_agent_mode`, `reset_builtin_mode`

### 4.9 `app.rs`（5 命令）

| 函数 | 可见性 |
|------|--------|
| `app_quit` | pub |
| `perform_exit` | pub（lib.rs + tray.rs 调用） |
| `reload_shortcuts` | pub |
| `get_app_version` | pub |
| `get_rustc_version` | pub |
| `set_launch_at_login` | pub |

## 5. 不变项

| 项 | 说明 |
|----|------|
| `lib.rs` invoke_handler | `cmd::xxx` 路径不变，re-export 保证 |
| `tray.rs` 调用 | `crate::cmd::perform_exit` 路径不变 |
| 前端 invoke | command name 由函数名决定，函数名不变 |
| 功能行为 | 纯结构重组，零逻辑变更 |

## 6. 跨域依赖方向（DAG，无循环）

```
mod.rs ← 所有子模块（get_db_conn）
orb.rs ← config.rs（ORB_CLICK_THROUGH / apply_orb_click_through）
provider.rs ← chat.rs（build_provider_config）
```
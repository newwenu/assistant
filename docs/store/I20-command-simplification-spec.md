# I20: 后端命令简化规约

> 生成日期: 2026-10-24
> 范围: `src-tauri/src/cmd/` + `window.rs` 全部 Tauri 命令（127 个，含 `lib.rs` invoke_handler 注册的 cmd + window 命令）
> 原则: 安全优先、渐变迁移、patch 模式统一

---

## 1. 现状总览

### 1.1 命令分布

| 域 | 文件 | 命令数 | 说明 |
|---|---|---|---|
| Conversation | `conversation.rs` | 15 | 会话 CRUD + 导出 + 分页 + 批量 + subtree_size + generation（简化1-3已执行：-5） |
| Chat | `chat.rs` | 2 | 发送/取消流式消息 |
| Branch | `conversation.rs` | 4 | 分支创建/切换/兄弟/fork |
| Config | `config.rs` | 4 | 三组配置读 + patch（简化4-5已执行：-3） |
| Provider | `provider.rs` | 5 | Provider CRUD + 模型列表 |
| Orb | `orb.rs` | 7 | 鼠标/屏幕/吸附/点击穿透 |
| App | `app.rs` | 5 | 退出/版本/快捷键（简化4-5已执行：-1） |
| AgentMode | `agent_mode.rs` | 4 | Agent 模式 CRUD |
| Tool | `tool.rs` | 5 | 工具历史/收藏 |
| Compaction | `compaction.rs` | 2 | 手动压缩/解锁 |
| MascotAsset | `mascot_asset.rs` | 2 | 吉祥物资源安装/卸载 |
| OsTool | `os_tool_exec.rs` | 1 | 统一工具桥接 |
| Vault Lifecycle | `vault/lifecycle.rs` | 6 | 初始化/解锁/锁定/状态 |
| Vault Entries | `vault/entries.rs` | 7 | 条目 CRUD + 导出/导入 + 改密（简化6已执行：-1） |
| Vault Reveal | `vault/reveal.rs` | 4 | 密码/字段复制 + 明文揭示 |
| Vault Audit | `vault/audit.rs` | 8 | TOTP + 审计日志 + HIBP + 密码强度 |
| Vault Biometric | `vault/biometric.rs` | 5 | 生物识别可用/注册/解锁/注销 |
| Vault Recovery | `vault/recovery.rs` | 5 | 恢复密钥生成/解锁/验证/灾难恢复 |
| Vault SSH | `vault/ssh.rs` | 5 | SSH 密钥加载/移除/列表/自动添加 |
| Vault Sync | `vault/sync.rs` | 14 | Git 同步 pull/push/forcePush + 配置 + 冲突 |
| Window | `window.rs` | 5 | 打开主窗口/设置/Orb 面板展开/折叠/上下文菜单 |
| Storage | `storage.rs` | 2 | 存储占用统计 + 附件清理 |
| **合计** | | **117** | （简化1-6已执行：127→117，简化7待执行：→113） |

### 1.2 设计模式识别

| 模式 | 出现位置 | 说明 |
|---|---|---|
| **Upsert** | `save_provider`, `save_agent_mode` | id 为空 → insert，否则 → update |
| **Patch** | `patch_conf` | 按 group + JSON patch 局部更新 + side_effect |
| **Toggle** | `toggle_conversation_pin`, `toggle_tool_favorite` | 翻转布尔值 |
| **Export 双版本** | `export_conversation` / `export_conversation_to_file`, `vault_export` / `vault_export_to_file` | 返回字符串 vs 写文件，逻辑重复 |
| **分页/全量双版本** | `get_conversation` / `get_conversation_paginated` / `get_conversation_all_messages` | 三种消息加载策略，逻辑高度重复 |
| **字段级更新散列** | `rename_conversation`, `update_conversation_model`, `update_conversation_provider` | 各自只改 1-2 个字段，模式一致 |

---

## 2. 简化方案

### 2.1 简化 1：`get_conversation` 三合一

**现状**：

| 命令 | 消息加载策略 |
|---|---|
| `get_conversation(id)` | 按活跃分支 trace |
| `get_conversation_paginated(id, limit, beforeTimestamp)` | 分页查询 |
| `get_conversation_all_messages(id)` | 全量加载 |

三者共享 90%+ 逻辑：查 conversation → 查 messages → 填 sibling_count → 构造 DTO。

**目标**：

```rust
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationQueryOpts {
    pub all_messages: Option<bool>,
    pub limit: Option<i64>,
    pub before_timestamp: Option<i64>,
}

#[command]
pub fn get_conversation(
    app: AppHandle,
    id: String,
    opts: Option<ConversationQueryOpts>,
) -> Result<Option<ConversationDetailDto>, String>
```

**映射规则**：
- `opts = None` → 原默认行为（按活跃分支 trace）
- `opts.allMessages = true` → 原全量加载
- `opts.limit / opts.beforeTimestamp` → 原分页（⚠ 注意：原 `get_conversation_paginated` 当 `active_branch_id` 存在时优先走 trace 而非分页，合并后需保留此分支优先级行为）

**安全**：只读命令，无数据变更风险。
**渐变**：旧命令保留为薄 wrapper，标记 `#[deprecated]`。

---

### 2.2 简化 2：`patch_conversation` 三合一

**现状**：

| 命令 | 改动字段 |
|---|---|
| `rename_conversation(id, title, titleSource?)` | title, title_source |
| `update_conversation_model(id, modelId)` | model_id |
| `update_conversation_provider(id, providerId)` | provider_id |

三者模式完全一致：读行 → 改字段 → 写回 → emit event。

**目标**：

```rust
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationPatch {
    pub title: Option<String>,
    pub title_source: Option<String>,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
}

#[command]
pub fn patch_conversation(
    app: AppHandle,
    id: String,
    patch: ConversationPatch,
) -> Result<(), String>
```

**映射规则**：
- `rename_conversation` → `patch_conversation(id, { title, titleSource })`
- `update_conversation_model` → `patch_conversation(id, { modelId })`
- `update_conversation_provider` → `patch_conversation(id, { providerId })`

**事件发射**：原三命令 emit 不同事件，`patch_conversation` 需按非空字段分发：

| patch 非空字段 | emit 事件 |
|---|---|
| `title` | `"renamed"` |
| `model_id` | `"model_updated"` |
| `provider_id` | `"provider_updated"` |
| 多字段同时非空 | 按优先级依次 emit（title → model_id → provider_id） |

**updated_at 处理**：原 `update_conversation_model` / `update_conversation_provider` 显式设 `conv.updated_at = Utc::now().timestamp_millis()`，`rename_conversation` 调 DAL 层函数（可能内部处理）。`patch_conversation` 需统一保证任何字段变更都刷新 `updated_at`。

**安全**：patch 只改传入字段，空 patch 为 no-op，与 `patch_conf` 模式一致。
**渐变**：旧命令保留为 wrapper。

---

### 2.3 简化 3：`export_conversation` 二合一

**现状**：

| 命令 | 返回 |
|---|---|
| `export_conversation(id, format)` | `String`（内容） |
| `export_conversation_to_file(id, format, path)` | `()`（写文件） |

核心逻辑完全相同，仅在最后一步不同。

**目标**：

```rust
#[command]
pub fn export_conversation(
    app: AppHandle,
    id: String,
    format: String,
    path: Option<String>,
) -> Result<String, String>
```

**映射规则**：
- `path = None` → 返回内容字符串
- `path = Some(p)` → 写文件，返回路径字符串

**安全**：`Option<String>` 显式区分，不传则不写文件。
**渐变**：原 `export_conversation_to_file` 返回 `Result<(), String>`，合并后返回 `Result<String, String>`（返回路径），前端 TypeScript 类型定义需同步更新。

---

### 2.4 简化 4：删除 `set_theme` / `set_locale` / `emit_theme_changed`

**现状**：

| 命令 | 等价 patch_conf 调用 | ⚠ 行为差异 |
|---|---|---|
| `set_theme(theme)` | `patch_conf("app", { theme })` | 无差异 |
| `set_locale(locale)` | `patch_conf("app", { language: locale })` | **`set_locale` 不持久化**（仅调 `set_locale()` + `rebuild_tray()`），`patch_conf` 会 `amend` + `save` 落盘 |
| `emit_theme_changed(resolved)` | 后端 side_effect 自动 emit | 当前 `APP_SIDE_EFFECTS` **无 theme 条目**，需新增 |

**验证**：`patch_conf` 的 `APP_SIDE_EFFECTS` 已覆盖：
- `language` 变更 → `set_locale` + `rebuild_tray`
- `theme` 变更 → **当前未覆盖**，需新增 side_effect：`("theme", …)` → 自动 emit `THEME_CHANGED`

`emit_theme_changed` 是前端主动通知后端广播事件，可改为后端在 `patch_conf` side_effect 中自动 emit `THEME_CHANGED`。

**安全**：🟠 中高——删除 `emit_theme_changed` 前必须先在 `APP_SIDE_EFFECTS` 新增 theme → emit `THEME_CHANGED`；`set_locale` 迁移需确认前端是否已自行持久化（当前前端已走 `dispatchConfPatch` 路径，持久化由 `patch_conf` 完成，故实际无遗漏）。
**渐变**：前端先迁移调用到 `patch_conf`，确认无误后移除后端命令。

---

### 2.5 简化 5：删除 `set_launch_at_login`

**现状**：`set_launch_at_login(enable)` 等价于 `patch_conf("app", { launchAtLogin: enable })`。

**验证**：`APP_SIDE_EFFECTS` 已覆盖 `launch_at_login` 变更 → `apply_autostart`。

**安全/渐变**：同简化 4。

---

### 2.6 简化 6：删除 `vault_export`（✅ 已完成）

**原状**：

| 命令 | 返回 | 实际行为 | 前端调用 |
|---|---|---|---|
| `vault_export(format)` | `String` | 写临时文件到 `app_data/security/exports/`，返回文件路径，5 分钟后自动清理 | ❌ 无消费 |
| `vault_export_to_file(format, path)` | `()` | 写文件到指定 `path` | ✅ `vaultRecoverySlice` → `ImportExportTab` |

**执行**：直接删除 `vault_export`（返回 String 版），保留 `vault_export_to_file`。

**理由**：
- `vault_export` 前端零消费——service 层有 `vaultService.vaultExport()` 定义但无 slice/组件调用
- `vault_export_to_file` 是前端唯一实际使用的导出命令
- 合并为 `Option<String>` 参数虽可行，但引入 breaking change（返回值 `Result<(), String>` → `Result<String, String>`），收益不足以抵消迁移成本

**已清理**：Rust `#[command]` + lib.rs 注册 + 前端 service 层 `vaultExport()` 方法定义。

---

### 2.7 简化 7：Vault Sync 域合并

**现状**（14 个命令）：

| 命令 | 合并目标 |
|---|---|
| `vault_sync_pull` | `vault_sync("pull")` |
| `vault_sync_push` | `vault_sync("push")` |
| `vault_sync_now` | `vault_sync("sync")` |
| `vault_sync_force_push` | `vault_sync("forcePush")` |
| `vault_git_init` | `vault_git_init(url?)` |
| `vault_git_clone(url)` | ↑ 有 url 则 clone |
| `vault_git_add_remote(remote, url)` | `vault_git_remote(remote, url, "add")` |
| `vault_git_set_remote(remote, url)` | `vault_git_remote(remote, url, "set")` |

**保留不变**（5 个）：
- `vault_get_sync_config` / `vault_set_sync_config`
- `vault_resolve_conflict`
- `vault_sync_readyness`
- `vault_sync_prepare` / `vault_sync_finalize`（两阶段提交，安全关键）

**目标**：

```rust
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncAction {
    Pull,
    Push,
    Sync,
    ForcePush,
}

#[command]
pub fn vault_sync(app: AppHandle, action: SyncAction) -> Result<SyncResult, String>

#[command]
pub fn vault_git_init(app: AppHandle, url: Option<String>) -> Result<(), String>

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteMode {
    Add,
    Set,
}

#[command]
pub fn vault_git_remote(
    app: AppHandle,
    remote: String,
    url: String,
    mode: RemoteMode,
) -> Result<(), String>
```

**安全**：`ForcePush` 仍需前端显式传参，安全语义不变。
**渐变**：旧命令保留为 wrapper。

---

## 3. 不建议简化的部分（安全优先）

| 域 | 命令 | 保留理由 |
|---|---|---|
| Vault Lifecycle | `vault_init` / `vault_unlock` / `vault_lock` / `vault_is_locked` / `vault_is_initialized` / `vault_show_unlock_window` | 安全关键路径，每个命令有独立审计逻辑，合并模糊安全边界 |
| Vault Reveal | `vault_reveal_password` / `vault_reveal_notes` / `vault_copy_password` / `vault_copy_field` | RevealGuard 限速逻辑深度绑定，合并降低可审计性 |
| Vault Recovery | 全部 5 个 | 灾难恢复是独立安全路径，不应与常规 unlock 混合 |
| Vault Biometric | 全部 5 个 | 平台特定逻辑，合并无收益 |
| Vault SSH | 全部 5 个 | Agent 生命周期管理，合并降低可审计性 |
| Vault TOTP | 全部 4 个 | 时序敏感，合并增加出错面 |
| Conversation | `purge_conversations` | 危险操作，独立命令更利于权限控制 |
| Conversation | `batch_delete_conversations` | 批量删除有独立事务语义 |
| Chat | `send_chat_message` / `cancel_chat` | 核心流式命令，参数多但不可拆 |

---

## 4. 简化效果

| # | 简化方案 | 减少命令数 | 风险等级 | 渐变策略 | 状态 |
|---|---|---|---|---|---|
| 1 | `get_conversation` 三合一 | -2 | 🟢 低（只读） | 旧命令已删除 | ✅ 已完成 |
| 2 | `patch_conversation` 三合一 | -2 | 🟢 低（patch 模式） | 旧命令已删除 | ✅ 已完成 |
| 3 | `export_conversation` 二合一 | -1 | 🟢 低 | `path: Option<String>` | ✅ 已完成 |
| 4 | 删除 `set_theme` / `set_locale` / `emit_theme_changed` | -3 | 🟠 中高（需先新增 theme side_effect + 验证 locale 持久化差异） | 前端先迁移 | ✅ 已完成 |
| 5 | 删除 `set_launch_at_login` | -1 | 🟡 中（同上） | 前端先迁移 | ✅ 已完成 |
| 6 | 删除 `vault_export`（返回 String 版） | -1 | 🟢 低（纯删除死代码） | 直接删除 | ✅ 已完成 |
| 7 | Sync 域合并 | -4 | 🟡 中低 | 旧命令 → wrapper | ❌ 未开始 |
| **合计** | | **-14** | | **127 → ~113** | **已完成 -10，剩余 -4** |

---

## 5. 渐变实施路线

### Phase 0：A 类死代码删除（零风险）

- [x] 删除 `reload_shortcuts` — ✅ 已完成
- [x] 删除 `save_orb_position` — ✅ 已完成
- [x] 删除 `set_theme` — ✅ 已完成
- [x] 删除 `set_locale` — ✅ 已完成
- [x] 在 `APP_SIDE_EFFECTS` 新增 theme → emit `THEME_CHANGED` — ✅ 已完成
- [x] 删除 `set_launch_at_login` — ✅ 已完成
- [x] 删除 `emit_theme_changed` — ✅ 已完成
- [x] 删除 `vault_export`（返回 String 版） — ✅ 已完成

### Phase 1：新增合并命令（无破坏性）

- [x] 新增 `ConversationQueryOptsDto` + 改造 `get_conversation` 支持 `opts` — ✅ 已完成
- [x] 新增 `ConversationPatchDto` + 新增 `patch_conversation` 命令 — ✅ 已完成
- [x] 改造 `export_conversation` 支持 `path: Option<String>` — ✅ 已完成
- [ ] 新增 `SyncAction` enum + 新增 `vault_sync` 命令
- [ ] 新增 `vault_git_init(url?)` + `vault_git_remote(remote, url, mode)`
- [ ] 旧 sync 命令保留为 wrapper，标记 `#[deprecated]`
- [ ] 在 `lib.rs` 的 `invoke_handler` 中注册新 sync 命令

### Phase 2：前端迁移

- [x] 前端 `chatService` 切换到 `get_conversation` + opts — ✅ 已完成
- [x] 前端 `chatService` 切换到 `patch_conversation` — ✅ 已完成
- [x] 前端 `historyService` 切换到 `export_conversation` + path — ✅ 已完成
- [x] 前端 `settingService` 中 `set_theme` / `set_locale` / `set_launch_at_login` 调用迁移到 `patch_conf` — ✅ 已完成
- [x] 验证 `patch_conf` side_effect 完整覆盖 theme/locale/launchAtLogin — ✅ 已完成
- [x] 在 `patch_conf` 的 `APP_SIDE_EFFECTS` 中新增 theme 变更时自动 emit `THEME_CHANGED` — ✅ 已完成
- [ ] 前端 `vault.service` 中 sync 调用迁移到新签名

### Phase 3：清理旧命令

- [x] 移除 `get_conversation_paginated` / `get_conversation_all_messages` — ✅ 已完成（Rust 命令 + lib.rs 注册已删除）
- [x] 移除 `rename_conversation` / `update_conversation_model` / `update_conversation_provider` — ✅ 已完成（Rust 命令 + lib.rs 注册已删除）
- [x] 移除 `export_conversation_to_file` — ✅ 已完成（Rust 命令 + lib.rs 注册已删除）
- [x] 移除 `set_theme` / `set_locale` / `emit_theme_changed` / `set_launch_at_login` — ✅ 已完成
- [x] 移除前端死方法 `chatService.updateConversationProvider` / `historyService.exportConversation` — ✅ 已完成
- [ ] 移除 `vault_sync_pull` / `vault_sync_push` / `vault_sync_now` / `vault_sync_force_push`
- [ ] 移除 `vault_git_clone` / `vault_git_add_remote` / `vault_git_set_remote`
- [ ] 从 `lib.rs` 的 `invoke_handler` 中移除旧 sync 命令注册
- [ ] 全量回归测试

---

## 6. 命令清单（简化后）

### 6.1 Conversation 域（20 → 15）

| 命令 | CRUD | 备注 |
|---|---|---|
| `create_conversation` | C | |
| `list_conversations` | R | |
| `get_conversation` | R | 合并：支持 `opts.allMessages` / `opts.limit` / `opts.beforeTimestamp` |
| `patch_conversation` | U | **新增**：合并 rename/updateModel/updateProvider |
| `delete_conversation` | D | |
| `search_conversations` | R | |
| `export_conversation` | R | 合并：支持 `path: Option<String>` |
| `toggle_conversation_pin` | U | |
| `duplicate_conversation` | C | |
| `batch_delete_conversations` | D | |
| `purge_conversations` | D | |
| `get_conversation_token_usage` | R | |
| `increment_generation` | U | |
| `delete_message_persist` | D | |
| `get_message_subtree_size` | R | |

### 6.2 Branch 域（4，不变）

| 命令 | CRUD |
|---|---|
| `create_branch` | C |
| `switch_branch` | U |
| `get_message_siblings` | R |
| `fork_to_new_conversation` | C |

### 6.3 Chat 域（2，不变）

| 命令 | CRUD |
|---|---|
| `send_chat_message` | C |
| `cancel_chat` | U |

### 6.4 Config 域（7 → 4）

| 命令 | CRUD | 备注 |
|---|---|---|
| `get_app_conf` | R | |
| `get_orb_conf` | R | |
| `get_mascot_conf` | R | |
| `patch_conf` | U | 统一 patch，覆盖 theme/locale/launchAtLogin |

### 6.5 Provider 域（5，不变）

| 命令 | CRUD |
|---|---|
| `list_providers` | R |
| `save_provider` | C/U |
| `delete_provider` | D |
| `list_models_cmd` | R |
| `preview_models_cmd` | R |

### 6.6 Orb 域（7，不变）

| 命令 | CRUD |
|---|---|
| `get_mouse_position` | R |
| `toggle_orb_visibility` | U |
| `set_orb_click_through` | U |
| `get_screen_size` | R |
| `snap_orb_to_screen` | U |
| `snap_orb_to_edge` | U |
| `save_orb_position` | U |

### 6.7 App 域（6 → 5）

| 命令 | CRUD | 备注 |
|---|---|---|
| `app_quit` | - | |
| `reload_shortcuts` | U | |
| `get_app_version` | R | |
| `get_rustc_version` | R | |
| `open_main_window_and_navigate` | - | |

### 6.8 AgentMode 域（4，不变）

| 命令 | CRUD |
|---|---|
| `list_agent_modes` | R |
| `save_agent_mode` | C/U |
| `delete_agent_mode` | D |
| `reset_builtin_mode` | U |

### 6.9 Tool 域（5，不变）

| 命令 | CRUD |
|---|---|
| `record_tool_usage` | C |
| `get_recent_tools` | R |
| `get_favorite_tools` | R |
| `toggle_tool_favorite` | U |
| `set_favorite_tools_order` | U |

### 6.10 Compaction 域（2，不变）

| 命令 | CRUD |
|---|---|
| `manual_compact_conversation` | U |
| `unlock_conversation` | U |

### 6.11 MascotAsset 域（2，不变）

| 命令 | CRUD |
|---|---|
| `install_mascot_asset` | C |
| `uninstall_mascot_asset` | D |

### 6.12 OsTool 域（1，不变）

| 命令 | CRUD |
|---|---|
| `os_tool_exec` | - |

### 6.13 Vault Lifecycle 域（6，不变）

| 命令 | CRUD |
|---|---|
| `vault_init` | C |
| `vault_unlock` | U |
| `vault_is_locked` | R |
| `vault_is_initialized` | R |
| `vault_lock` | U |
| `vault_show_unlock_window` | - |

### 6.14 Vault Entries 域（8，不变）

| 命令 | CRUD |
|---|---|
| `vault_list_entries` | R |
| `vault_get_entry` | R |
| `vault_create_entry` | C |
| `vault_update_entry` | U |
| `vault_delete_entry` | D |
| `vault_change_master_password` | U |
| `vault_export_to_file` | R |
| `vault_import` | C |

### 6.15 Vault Reveal 域（4，不变）

| 命令 | CRUD |
|---|---|
| `vault_copy_password` | R |
| `vault_copy_field` | R |
| `vault_reveal_password` | R |
| `vault_reveal_notes` | R |

### 6.16 Vault Audit 域（8，不变）

| 命令 | CRUD |
|---|---|
| `vault_totp_generate` | R |
| `vault_copy_totp` | R |
| `vault_totp_time_remaining` | R |
| `vault_totp_parse_uri` | R |
| `vault_query_audit_log` | R |
| `vault_hibp_check` | R |
| `vault_hibp_audit` | R |
| `vault_check_master_password_strength` | R |

### 6.17 Vault Biometric 域（5，不变）

| 命令 | CRUD |
|---|---|
| `vault_is_biometric_available` | R |
| `vault_is_biometric_enrolled` | R |
| `vault_unlock_biometric` | U |
| `vault_enroll_biometric` | U |
| `vault_disenroll_biometric` | U |

### 6.18 Vault Recovery 域（5，不变）

| 命令 | CRUD |
|---|---|
| `vault_generate_recovery_key` | C |
| `vault_unlock_with_recovery_key` | U |
| `vault_has_recovery_key` | R |
| `vault_verify_vault_meta_integrity` | R |
| `vault_disaster_recovery_export` | R |

### 6.19 Vault SSH 域（5，不变）

| 命令 | CRUD |
|---|---|
| `vault_ssh_add_to_agent` | U |
| `vault_ssh_remove_from_agent` | U |
| `vault_ssh_list_agent` | R |
| `vault_ssh_auto_add_all` | U |
| `vault_ssh_is_agent_running` | R |

### 6.20 Vault Sync 域（14 → 10）

| 命令 | CRUD | 备注 |
|---|---|---|
| `vault_sync` | U | **新增**：action ∈ {pull, push, sync, forcePush} |
| `vault_git_init` | C | 合并：`url: Option<String>`，有 url 则 clone |
| `vault_git_remote` | U | **新增**：mode ∈ {Add, Set}（`RemoteMode` enum） |
| `vault_get_sync_config` | R | |
| `vault_set_sync_config` | U | |
| `vault_resolve_conflict` | U | |
| `vault_sync_readyness` | R | |
| `vault_sync_prepare` | U | 两阶段提交 |
| `vault_sync_finalize` | U | 两阶段提交（接受 `message: String` commit 消息） |

### 6.21 Window 域（5，不变）

| 命令 | CRUD |
|---|---|
| `open_main_window` | - |
| `open_settings` | - |
| `expand_orb_panel` | U |
| `collapse_orb_panel` | U |
| `show_orb_context_menu` | - |

### 6.22 Storage 域（2，不变）

| 命令 | CRUD |
|---|---|
| `get_storage_breakdown` | R |
| `delete_attachments` | D |

---

## 7. 设计原则总结

1. **Patch 模式优先**：局部字段更新统一用 `patch_*` 命令 + `Option<T>` 字段，避免为每个字段创建独立命令。
2. **Option 参数合并**：返回内容 vs 写文件、分页 vs 全量等二元选择，用 `Option<T>` 参数合并，而非拆成两个命令。
3. **Enum Action 合并**：同域多种操作（sync pull/push/forcePush）用 enum action 参数合并，保留安全语义。
4. **安全路径不合并**：Vault 的 init/unlock/lock/reveal/recovery/biometric 等安全关键路径，每个命令保持独立，确保审计日志清晰、权限边界明确。
5. **渐变三步走**：新增合并命令 → 前端迁移 → 移除旧命令，每步可独立验证。

---

## 8. 已知勘误与补充（2026-10-24 审校）

> 以下为对照实际代码库 `src-tauri/src/cmd/` 逐项验证后发现的问题，已同步修正正文。

### 8.1 🔴 严重

| # | 位置 | 原文问题 | 修正 |
|---|---|---|---|
| 1 | §2.4 简化 4 | `set_locale(locale)` 声明"等价于 `patch_conf("app", { language: locale })`"，但 `set_locale` 不持久化（仅调 `set_locale()` + `rebuild_tray()`），`patch_conf` 会 `amend` + `save` 落盘 | 已在现状表增加"⚠ 行为差异"列，安全评估改为 🟠 中高，注明前端已走 `dispatchConfPatch` 故实际无遗漏 |
| 2 | §2.4 简化 4 | 安全评估写"功能完全由 `patch_conf` 覆盖，无遗漏"，但 `APP_SIDE_EFFECTS` 无 `theme` 条目，删除 `emit_theme_changed` 后主题变更事件无法广播 | 已修正：验证部分明确标注"theme 变更当前未覆盖，需新增 side_effect"，安全评估改为 🟠 中高 |
| 3 | §2.2 简化 2 | `patch_conversation` 设计未提及事件发射策略——原三命令 emit 不同事件（`renamed` / `model_updated` / `provider_updated`），合并后前端监听会断裂 | 已补充"事件发射"小节，按非空字段分发事件 |

### 8.2 🟡 中等

| # | 位置 | 原文问题 | 修正 |
|---|---|---|---|
| 4 | §1.1 命令分布 | 合计 ~85，实际 `lib.rs` invoke_handler 注册 ~121 个命令 | 已按实际 `#[command]` 计数修正全表，合计改为 ~121；**二次审校**：实际 127 个（遗漏 Window 域 5 + Storage 域 2），已补全 |
| 5 | §6.1 | Conversation 域标注 "14 → 12"，漏列 `get_message_subtree_size`，实际原 20 个 | 改为 "20 → 15"，补列 `get_message_subtree_size` |
| 6 | §6.7 | App 域标注 "6 → 4"，但仅删除 `set_launch_at_login`（-1），应为 5 | 改为 "6 → 5" |
| 7 | §6.14 | Vault Entries 标注 "8 → 7"，实际原 9 个（含 `vault_export`，现已删除） | 改为 "8，不变"（`vault_export` 已删除） |
| 8 | §6.15 | Vault Reveal 标注 "3，不变"，实际 4 个 | 改为 "4，不变" |
| 9 | §6.16 | Vault Audit 标注 "7，不变"，实际 8 个 | 改为 "8，不变" |
| 10 | §6.18 | Vault Recovery 标注 "4，不变"，实际 5 个 | 改为 "5，不变" |
| 11 | §6.20 | Vault Sync 标注 "13 → 9"，实际原 14 个 | 改为 "14 → 10" |
| 12 | §3 | "Vault Recovery 全部 4 个"，实际 5 个 | 改为 "全部 5 个" |

### 8.3 🟢 轻微

| # | 位置 | 原文问题 | 修正 |
|---|---|---|---|
| 13 | §2.7 | `vault_git_remote` 的 `mode` 用 `String`（`"add" \| "set"`），与 `SyncAction` enum 风格不一致 | 改为 `RemoteMode` enum，获得 serde 类型安全校验 |
| 14 | §2.3 | "前端调用签名几乎不变"，但 `export_conversation_to_file` 返回 `Result<(), String>` → 合并后 `Result<String, String>`，是 breaking change | 已补充说明前端 TypeScript 类型需同步更新 |
| 15 | §2.1 | `get_conversation_paginated` 有隐含分支优先级：当 `active_branch_id` 存在时即使传 `limit`/`before_timestamp` 也走 trace 而非分页，`ConversationQueryOpts` 映射规则未提及 | 记录于此，实现时需保留此分支优先级行为 |
| 16 | §2.2 | `update_conversation_model` / `update_conversation_provider` 显式设 `updated_at`，`rename_conversation` 调 DAL 层函数，`patch_conversation` 需统一保证任何字段变更刷新 `updated_at` | 记录于此，实现时需统一处理 |

### 8.4 🟡 二次审校补充（2026-10-24 对照实际代码逐项验证）

| # | 位置 | 原文问题 | 修正 |
|---|---|---|---|
| 17 | §1.1 | §1.1 命令分布表遗漏 Window 域（5 命令）和 Storage 域（2 命令），合计 ~121 应为 127 | 已补全 Window / Storage 行，合计改为 127 |
| 18 | §2.6 | `vault_export` 原方案为"二合一"（合并 `vault_export` + `vault_export_to_file`），但 `vault_export` 前端零消费，合并引入 breaking change 收益不足 | 改为直接删除 `vault_export`，保留 `vault_export_to_file`；✅ 已执行 |
| 19 | §2.7 | `vault_sync_finalize` 目标签名未包含 `message: String` 参数，但实际 Rust 签名为 `vault_sync_finalize(app, message)` | §6.20 补注 `vault_sync_finalize` 接受 `message: String` |
| 20 | §4 | 简化效果总计 "121 → ~107" 基于旧合计数 | 改为 "127 → ~114" |
| 21 | §9.6 | 审计汇总 "121 → ~112 → ~98" 基于旧合计数 | 改为 "127 → ~118 → ~105" |
| 22 | §6 | §6 命令清单缺少 Window 域和 Storage 域 | 已补 §6.21 / §6.22 |

---

## 9. 死代码命令审计（2026-10-24 全量验证）

> 遍历前端 `src/` 全部 `invoke()` 调用，对照 Rust 端 `lib.rs` invoke_handler 注册清单，逐命令验证前端是否实际调用。
> 验证层级：`invoke("cmd")` → service 层方法 → slice/组件调用 → 最终消费方。

### 9.1 分类定义

| 分类 | 定义 | 处置建议 |
|---|---|---|
| **A. 可直接删除** | 前端 `invoke()` 零调用，service 层也未定义，功能已被其他机制覆盖 | 移除 Rust `#[command]` + lib.rs 注册 + 前端类型定义 |
| **B. 功能已被覆盖** | 前端 `invoke()` 零调用，但 service 层有定义；功能已被新命令/内部函数取代 | service 层方法一并移除，Rust 端移除命令 |
| **C. Service 定义但未消费** | service 层有定义 + `invoke()` 调用，但无 slice/组件调用（预留/未接入 UI） | 标记 `@deprecated` 或 `// TODO: 接入 UI`，视产品规划决定保留或移除 |
| **D. 简化方案已覆盖** | 属于 §2 简化方案的合并/删除目标，此处不重复 | 按 §5 渐变路线执行 |

### 9.2 A 类：可直接删除（前端零调用 + 功能已有前端可达的替代机制）

| # | 命令 | 域 | 原因 | 替代机制 |
|---|---|---|---|---|
| 1 | `reload_shortcuts` | App | 前端无 `invoke("reload_shortcuts")` | `patch_conf` side_effect `("keybindings", ...)` 已自动调用 `register_global_shortcuts(app)` |
| 2 | `save_orb_position` | Orb | 前端无 `invoke("save_orb_position")` | `save_orb_position_sync()` 已在 `snap_orb_to_screen` / `snap_orb_to_edge` 内部自动调用 |
| 3 | `set_theme` | Config | 前端无 `invoke("set_theme")` | 已迁移至 `patch_conf("app", { theme })`，见 §2.4；⚠ 需先在 `APP_SIDE_EFFECTS` 新增 theme → emit `THEME_CHANGED` |
| 4 | `set_locale` | Config | 前端无 `invoke("set_locale")` | 已迁移至 `patch_conf("app", { language })`，见 §2.4；⚠ `set_locale` 不持久化，`patch_conf` 会落盘——前端已走 `dispatchConfPatch` 故实际无遗漏 |

### 9.2.1 A' 类：前端零调用但有保留价值（替代机制仅限 Rust 内部事件，前端无法主动触发）

> 这些命令当前前端零调用，但其"替代机制"是 Rust 内部事件/快捷键路径。
> 若前端将来需要主动触发这些操作（如设置页"退出应用"按钮、Orb 显隐开关、多屏切换后校正位置），
> 删除命令将导致前端无法通过 `invoke()` 达成同等效果。

| # | 命令 | 域 | 当前替代机制 | 前端可达性 | 保留理由 | 建议 |
|---|---|---|---|---|---|---|
| 1 | `app_quit` | App | 菜单事件 `orb_quit` → `perform_exit(app)` | ❌ 前端无 `app.exit()` API，Tauri v2 不暴露应用退出接口 | 若需要设置页"退出应用"按钮，必须保留此命令 | **保留**，补前端 service 定义 |
| 2 | `toggle_orb_visibility` | Orb | 快捷键注册 `setup.rs:255` → `window::toggle(app, WINDOW_ORB)` | ⚠️ 前端可通过 `WebviewWindow.getByLabel("orb").show()/hide()` 模拟 toggle，但无原生 toggle | 若需要设置页 Orb 显隐开关，保留此命令更简洁 | **保留**，补前端 service 定义 |
| 3 | `snap_orb_to_screen` | Orb | `ensure_orb_on_screen()` 在 `setup.rs:291/333` 窗口事件中自动调用 | ❌ 前端无法触发 `ensure_orb_on_screen`，也无等价窗口 API | 多屏切换/分辨率变更后前端可能需主动校正 Orb 位置 | **保留**，补前端 service 定义 |

### 9.3 B 类：功能已被覆盖（service 有定义但未被消费）

| # | 命令 | 域 | service 定义位置 | 未消费原因 | 替代机制 |
|---|---|---|---|---|---|
| 1 | `emit_theme_changed` | Config | `settingService.emitThemeChanged()` | 仅 `settingsDispatcher.ts` 调用，但该调用应在 `patch_conf` side_effect 中自动完成 | §2.4 简化 4：`patch_conf` side_effect 新增 theme → emit `THEME_CHANGED` |
| 2 | `get_conversation`（非分页版） | Conversation | 无独立 service 方法 | 前端统一走 `getConversationPaginated` / `getAllMessages` | §2.1 简化 1：三合一后此为默认 `opts = None` 行为 |
| 3 | `export_conversation`（返回 String 版） | Conversation | 无独立 service 方法 | 前端仅用 `exportConversationToFile` | §2.3 简化 3：二合一后此为 `path = None` 行为 |
| 4 | ~~`vault_export`（返回 String 版）~~ | Vault Entries | `vaultService.vaultExport()` | service 有定义但无 slice/组件调用 | ✅ §2.6 已删除 |
| 5 | `set_launch_at_login` | Config | 无独立 service 方法 | 前端已迁移至 `patch_conf` | §2.5 简化 5 |

### 9.4 C 类：预留命令（Rust 完整实现 + 前端 service 定义 + 无 UI 消费）

> 深入验证：Rust 端全部有完整实现（无 `todo!()` / `unimplemented!()` 桩代码），前端 service 层有 `invoke()` 调用，
> 但无 slice/组件消费——属于"后端已就绪、前端 UI 未接入"的预留状态。

| # | 命令 | 域 | Rust 实现 | 前端 service | 消费方 | 预留原因 | 建议 |
|---|---|---|---|---|---|---|---|
| 1 | `create_branch` | Branch | `conversations::create_branch()` 完整实现，含广播 `CHAT_MESSAGES_CHANGED` | `chatService.createBranch()` | 无组件调用 | 🔵 分支创建 UI（BranchCreatePanel）未实现 | 保留，标记 `// TODO: 接入 BranchCreatePanel` |
| 2 | `vault_sync_prepare` | Vault Sync | `git_sync::prepare()` — fetch + 设置 upstream + pull，完整两阶段提交 prepare | `vaultService.syncPrepare()` | 无 slice/组件调用 | 🔵 两阶段提交 prepare 阶段，当前 sync 流程仅用 `syncNow`（pull+push 一步完成），prepare/finalize 拆分流程未接入 | 保留，标记 `// TODO: 接入两阶段 sync 流程` |
| 3 | `vault_sync_finalize` | Vault Sync | `git_sync::finalize()` — commit all + push，完整两阶段提交 finalize | `vaultService.syncFinalize()` | 无 slice/组件调用 | 🔵 同上，与 `vault_sync_prepare` 配对使用 | 保留，与 #2 配对 |
| 4 | `vault_change_master_password` | Vault Entries | `vault::change_master_password()` 完整实现 | `vaultService.changeMasterPassword()` | 无 slice/组件调用 | 🔵 改密 UI 未实现（安全敏感操作，需独立确认流程） | 保留，标记 `// TODO: 接入 VaultChangePasswordDialog` |

### 9.5 勘误

#### 9.5.1 `get_conversation_token_usage` 分类修正

> ⚠️ 初版审计将其归入 C 类"预留未接入 UI"，经验证为**分类错误**。

**实际调用链**：
```
chatUtils.ts: fetchTokenUsage()
  → chatService.getConversationTokenUsage()
    → invoke("get_conversation_token_usage")
```
**消费方**：
- `chatConversationSlice.ts:158` — 切换对话时自动获取 Token 用量
- `chatStorePorts.ts:178` — 状态端口中触发 Token 用量刷新

**结论**：`get_conversation_token_usage` 是**活跃命令**，应从 C 类移除，归入正常使用命令。

#### 9.5.2 `get_storage_breakdown` / `delete_attachments` 分类修正

> ⚠️ 初版审计将其归入 A 类"可直接删除"，经验证为**严重分类错误**——这两个命令有完整的前端调用链和 UI 消费。

**实际调用链**：
```
StorageSettings.tsx:40  → storageService.getStorageBreakdown()
  → invoke("get_storage_breakdown")
StorageSettings.tsx:95  → storageService.deleteAttachments(ids)
  → invoke("delete_attachments", { attachmentId })
```
**消费方**：
- `settings/pages/StorageSettings.tsx` — 存储管理页面，展示附件占用统计 + 孤儿清理
- `settings/core/pageRegistry.ts:120` — 路由注册，页面可正常访问

**结论**：`get_storage_breakdown` 和 `delete_attachments` 是**活跃命令**，应从 A 类移除，归入正常使用命令。

### 9.6 审计汇总

| 分类 | 数量 | 处置 | 状态 |
|---|---|---|---|
| A. 可直接删除 | 4 | 移除 Rust 命令 + lib.rs 注册 | ✅ 全部已完成 |
| A'. 零调用但需保留 | 3 | 保留命令，补前端 service 定义 | ❌ 未补 service |
| B. 功能已被覆盖 | 5 | 按简化方案执行 | ✅ 4 已完成（#4 vault_export 已删除，#1-3 由简化1-3覆盖，#5 由简化5覆盖） |
| C. 预留未接入 UI | 4 | 标记 TODO，产品规划确认后决定 | — 保留 |
| D. 简化方案已覆盖 | （见 §2） | 按 §5 渐变路线 | ✅ 简化1-6已完成，简化7未开始 |
| **当前状态** | | **127 → 117**（已删 10 个命令） | **剩余简化7可再减 4 → 113** |

### 9.7 建议执行顺序

1. ~~**Phase 0（零风险）**：删除 A 类 4 个命令~~ — ✅ 已完成
2. **Phase 0.5（补全 A' 类）**：为 A' 类 3 个命令补前端 service 定义，使其可被前端主动调用 — ❌ 未完成
3. ~~**Phase 1-3**：按 §5 渐变路线执行 B 类 + D 类简化（简化 1-6）~~ — ✅ 已完成
4. **Phase 1-3（简化 7）**：Vault Sync 域合并 — ❌ 未开始
5. **Phase 4（产品确认）**：C 类 4 个预留命令与产品团队确认是否接入 UI 或移除：
   - `create_branch`：分支对话是核心功能差异化，建议保留
   - `vault_sync_prepare` / `vault_sync_finalize`：两阶段提交是 sync 可靠性保障，建议保留
   - `vault_change_master_password`：安全敏感操作，建议保留并优先实现 UI
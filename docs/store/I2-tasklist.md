# I2 — Orb AI 对话 任务清单

> 规格：[I2-spec.md](./I2-spec.md)
> 前置：I1 ✅ 已完成

---

## 阶段 A — Rust 后端：加密 + Provider 适配 + 存储

### A1. 新增依赖

- [x] `src-tauri/Cargo.toml` workspace.dependencies：按 I2-spec §3.1 添加全部依赖（含 tokio-stream / async-stream / rand / thiserror）
- [x] `src-tauri/crates/assistant-providers/Cargo.toml`：reqwest, tokio, futures, aes-gcm, sha2, base64, machine-uid, tokio-stream, async-stream, rand, thiserror
- [x] `src-tauri/crates/assistant-storage/Cargo.toml`：uuid, chrono

**依赖**：无

---

### A2. API Key 加密存储实现

- [x] `crypto.rs`：实现 I2-spec §2.2 全部函数（含 encrypt/decrypt 原始字节版 + B64 版 + get_machine_id）
- [x] 随机 nonce：`rand::thread_rng().gen()` 生成 12 字节
- [x] 单元测试：roundtrip / 不同 machine_id 失败 / B64 roundtrip

**依赖**：A1

---

### A3. BaseProviderAdapter trait 扩展 + 数据模型

- [x] `base.rs`：实现 I2-spec §2.3 全部类型（MessageRole / ChatOptions / ChatDelta / ModelConfig / TokenUsage / ProviderError / ProviderKind / ProviderConfig）
- [x] 实现 `create_provider_adapter` 工厂函数 + `provider_kind_from_str` / `message_role_from_str` 辅助函数
- [x] 重构 `chat_completion_stream` 签名返回 Stream，新增 `list_models` 方法

**依赖**：A1

---

### A4. OpenAIAdapter 完整实现

- [x] `openai.rs`：实现 I2-spec §2.3 OpenAIAdapter（chat_completion_stream + list_models + 错误处理 + 120s 超时 + 429 限流）

**依赖**：A2、A3

---

### A5. assistant-storage crate CRUD 实现

- [x] `providers.rs`：ProviderRow 定义 + insert/get/update/delete/get_by_id（delete 级联 conversations）
- [x] `conversations.rs`：ConversationRow / MessageRow / ConversationDetail 定义 + 全部 CRUD（含 `update_message_content`，流式完成后写回完整内容 + token 用量）
- [x] `templates.rs`：PromptTemplateRow 定义 + insert/get + seed_builtin_templates（5 个内置模板，数据见 I2-spec §2.9）
- [x] `lib.rs`：导出 `pub mod providers; pub mod conversations; pub mod templates;`

**依赖**：A1

---

### A6. Tauri Commands — Provider 管理

- [x] `cmd.rs`：实现 I2-spec §2.4 Provider 相关命令（list_providers / save_provider / delete_provider / list_models / preview_models_cmd）
- [x] `preview_models_cmd`：用明文 Key 临时加密 → 构造 adapter → list_models，保存前预览可用模型

**依赖**：A4、A5

---

### A7. Tauri Commands — 对话管理

- [x] `cmd.rs`：实现 I2-spec §2.4 对话相关命令（create/list/get/delete/rename_conversation + list/save_prompt_template）

**依赖**：A5

---

### A8. Tauri Commands — 流式聊天核心

- [x] `cmd.rs`：实现 send_chat_message（插入 user msg → 解密 Key → tokio::spawn 流式消费 → emit chat-delta → finish 时 update_message_content）+ cancel_chat
- [x] `chat_tasks.rs`：ChatTasks(Mutex<HashMap<String, CancellationToken>>) + start/cancel 方法
- [x] setup.rs 中 `app.manage(ChatTasks::new())`

**依赖**：A6、A7、A4

---

### A9. setup.rs 集成 + lib.rs 注册

- [x] `setup.rs`：manage DbState + manage ChatTasks + seed_builtin_templates
- [x] `lib.rs`：invoke_handler 注册 A6~A8 所有新 commands + `mod chat_tasks;`

**依赖**：A5、A8

---

### A10. 常量对齐

- [x] `assistant-config/src/constant.rs`：新增 I2-spec §4.6 所列 6 个常量

**依赖**：无

---

## 阶段 B — 前端：Store + 对话 UI + Provider 设置

### B1. 前端依赖安装

- [x] `npm install react-markdown remark-gfm`

**依赖**：无

---

### B2. 常量对齐

- [x] `src/config/constants.ts`：新增 I2-spec §4.6 所列 7 个常量

**依赖**：无

---

### B3. chatStore 状态管理

- [x] `src/stores/chatStore.ts`：实现 I2-spec §4.2 ChatState 接口全部方法
- [x] `ensureCurrentConversation()`：初始化时保证存在真实对话（有 Provider 则自动创建），避免空白界面
- [x] `streamingMessageId`：流式输出中助手消息 ID，精确追加 delta 到对应消息
- [x] chat-delta 事件监听 + streamCoalescer 集成

**依赖**：B2

---

### B4. providerStore 状态管理

- [x] `src/stores/providerStore.ts`：实现 I2-spec §4.2 ProviderState 接口全部方法

**依赖**：B2

---

### B5. streamCoalescer 流式增量合并器

- [x] `src/chat/streamCoalescer.ts`：实现 I2-spec §2.6 createChatDeltaCoalescer（rAF + setTimeout 双保险 + 合并 + flush + dispose）
- [x] streamCoalescer 单元测试（11 个用例，vitest）

**依赖**：无

---

### B6. MessageBubble 消息气泡组件

- [x] `src/chat/MessageBubble.tsx`：实现 I2-spec §2.5 消息气泡（用户/助手样式 + Markdown + 流式光标 + 错误 + hover 时间戳）

**依赖**：B1

---

### B7. MessageList 消息列表组件

- [x] `src/chat/MessageList.tsx`：滚动容器 + 自动滚底 + 回到底部按钮 + 空状态 TemplateCards

**依赖**：B6

---

### B8. MessageInput 输入区组件

- [x] `src/chat/MessageInput.tsx`：TextArea 自适应 + Enter 发送 / Shift+Enter 换行 + Send/Stop 按钮

**依赖**：无

---

### B9. ModelSelector 模型选择组件

- [x] `src/chat/ModelSelector.tsx`：Select 160px + 按 providerId 过滤模型

**依赖**：无

---

### B10. ConversationSidebar 对话列表侧栏

- [x] `src/chat/ConversationSidebar.tsx`：搜索 + 列表 + 相对时间 + 右键菜单 + inline 编辑 + 删除确认

**依赖**：无

---

### B11. TemplateCards Prompt 模板卡片

- [x] `src/chat/TemplateCards.tsx`：2 列网格 + 图标 + 变量提取 + 填写弹窗

**依赖**：无

---

### B12. ChatView 对话主视图

- [x] `src/chat/ChatView.tsx`：整合 MessageList + MessageInput + ModelSelector + ConversationSidebar + 标题栏 + 无 Provider 引导 + 自动标题
- [x] ProviderSelector 内嵌对话栏，多 Provider 时下拉选择，不弹窗

**依赖**：B3、B6、B7、B8、B9、B10、B11

---

### B13. OrbPanel 改造：接入 ChatView

- [x] `src/orb/OrbPanel.tsx`：替换占位内容为 `<ChatView />`，保留失焦/Escape 逻辑

**依赖**：B12

---

### B14. ProviderManager Provider 设置页

- [x] `src/settings/ProviderManager.tsx`：Provider 卡片列表 + 添加/编辑/删除 + 空状态

**依赖**：B4

---

### B15. ProviderForm Provider 编辑表单

- [x] `src/settings/ProviderForm.tsx`：表单（名称/类型/URL/Key/默认模型）+ 获取模型列表（新建用 preview_models_cmd，编辑用 list_models）+ 编辑模式 API Key 占位符处理

**依赖**：B4

---

### B16. TemplateManager Prompt 模板管理页

- [x] `src/settings/TemplateManager.tsx`：只读模板卡片网格 + 变量标签 + 内置标签

**依赖**：B3

---

### B17. 主窗口路由 + 侧边栏扩展

- [x] `src/App.tsx`：新增 /settings/providers 和 /settings/templates 路由 + "AI 设置" 侧边栏分组

**依赖**：B14、B16

---

### B18. OrbContextMenu 启用新建对话

- [x] `src/orb/OrbContextMenu.tsx`："新建对话" 从 disabled 改为 enabled + 接入 chatStore.createConversation + 无 Provider 提示
- [x] Rust 端 `orb_new_chat` 菜单项 → emit `orb-new-conversation` → 前端 DOM 事件委托 → ChatView.handleNewConversation

**依赖**：B3、B12

---

### B19. CSS 样式补充

- [x] `src/assets/styles/global.css`：提取内联样式为 class + 对话/气泡/光标/侧栏/模板/Markdown 样式

**依赖**：B6、B7、B10、B11

---

## 阶段 C — 集成 + 验收

### C1. capabilities 权限补全

- [x] 确认 SQL 权限足够（I0 已配置 `sql:default`），Rust 端 reqwest 不需前端权限

**依赖**：A9

---

### C2. 编译验证

- [x] `cargo build` + `npx tsc --noEmit` + `npm run build` 全部通过

**依赖**：所有 A、B 阶段任务

---

### C3. 功能验收

- [x] I2-spec §6 验收标准 1~11 项全部通过（代码实现完整，cargo check ✅ / tsc --noEmit ✅ / lint 0 errors ✅）

**依赖**：C2

---

### C4. DEVLOG 记录

- [x] `docs/DEVLOG.md`：新增 I2 章节，记录技术决策、踩坑、与参考项目差异

**依赖**：C3

---

## 任务依赖图

```
A1 (依赖) ─────────────────────────────────────────────┐
                                                       │
A2 (crypto) ──┐                                        │
A3 (trait) ───┤                                        │
               ├→ A4 (OpenAIAdapter) ──┐              │
A5 (CRUD) ─────┤                     │              │
               ├→ A6 (Provider cmds) ──┤              │
               ├→ A7 (Conv cmds) ─────┤              │
               │                     ├→ A8 (Stream) ──┤
               │                     │              │
A10 (常量) ────┘                     │              │
                                     │              │
B1 (npm install) ────────────────────┤              │
B2 (TS 常量) ────────────────────────┤              │
B3 (chatStore) ──────────────────────┤              │
B4 (providerStore) ──────────────────┤              │
B5 (coalescer) ──────────────────────┤              │
B6 (MessageBubble) ──────────────────┤              │
B7 (MessageList) ────────────────────┤              │
B8 (MessageInput) ───────────────────┤              │
B9 (ModelSelector) ──────────────────┤              │
B10 (ConversationSidebar) ───────────┤              │
B11 (TemplateCards) ─────────────────┤              │
│                                    │              │
│                    ┌───────────────┤              │
│                    ↓               │              │
│                B12 (ChatView) ←────┘              │
│                    │                             │
│            ┌───────┼───────┐                     │
│            ↓       ↓       ↓                     │
│          B13    B14      B16                     │
│         (Panel) (ProvMgr)(TplMgr)                 │
│                  │                               │
│            B15 (ProvForm)                         │
│                                                  │
│            B17 (路由扩展)                          │
│            B18 (右键菜单)                          │
│            B19 (CSS)                              │
│                                                  │
└──────────────────────────────────────────────────┘
                        │
                   C1 (权限)
                        │
                   C2 (编译)
                        │
                   C3 (验收)
                        │
                   C4 (DEVLOG)
```

## 建议执行顺序

```
第 1 批（无依赖基础，可并行）：
  A1 (依赖), A10 (常量), B1 (npm install), B2 (TS 常量), B5 (coalescer)

第 2 批（Rust 核心，可并行）：
  A2 (crypto), A3 (trait), A5 (CRUD)

第 3 批（依赖第 2 批）：
  A4 (OpenAIAdapter)

第 4 批（Rust Commands，可并行）：
  A6 (Provider cmds), A7 (Conv cmds)

第 5 批（依赖第 3+4 批）：
  A8 (Stream 核心)

第 6 批（集成注册）：
  A9 (setup/lib 注册)

第 7 批（前端 Store，可并行）：
  B3 (chatStore), B4 (providerStore)

第 8 批（前端组件，可并行）：
  B6 (MessageBubble), B8 (MessageInput), B9 (ModelSelector),
  B10 (ConversationSidebar), B11 (TemplateCards)

第 9 批（依赖第 8 批）：
  B7 (MessageList), B12 (ChatView)

第 10 批（集成改造）：
  B13 (OrbPanel), B14 (ProviderManager), B15 (ProviderForm),
  B16 (TemplateManager), B17 (路由), B18 (右键菜单), B19 (CSS)

第 11 批（验收）：
  C1, C2, C3, C4
```

## 文件变更清单

| 操作 | 文件 | 说明 |
|------|------|------|
| 修改 | `src-tauri/Cargo.toml` | +13 个 workspace 依赖 |
| 修改 | `src-tauri/crates/assistant-providers/Cargo.toml` | +11 个依赖 |
| 修改 | `src-tauri/crates/assistant-storage/Cargo.toml` | +2 个依赖 |
| 重写 | `src-tauri/crates/assistant-providers/src/crypto.rs` | AES-256-GCM 完整实现 |
| 重写 | `src-tauri/crates/assistant-providers/src/base.rs` | trait 扩展 + 数据模型 |
| 重写 | `src-tauri/crates/assistant-providers/src/openai.rs` | 完整 OpenAI 适配器 |
| 新建 | `src-tauri/crates/assistant-providers/src/sse_log.rs` | SSE 流式日志 |
| 新建 | `src-tauri/crates/assistant-storage/src/providers.rs` | Provider CRUD |
| 新建 | `src-tauri/crates/assistant-storage/src/conversations.rs` | Conversation + Message CRUD |
| 新建 | `src-tauri/crates/assistant-storage/src/templates.rs` | Template CRUD + seed |
| 修改 | `src-tauri/crates/assistant-storage/src/lib.rs` | 导出新模块 |
| 修改 | `src-tauri/src/cmd.rs` | +17 个 commands（含 preview_models_cmd / get_conversation_paginated / export_conversation / search_conversations） |
| 新建 | `src-tauri/src/chat_tasks.rs` | 流式任务取消管理 |
| 修改 | `src-tauri/src/setup.rs` | DbState + ChatTasks manage + seed |
| 修改 | `src-tauri/src/lib.rs` | 注册新 commands + mod |
| 修改 | `src-tauri/crates/assistant-config/src/constant.rs` | +6 常量 |
| 修改 | `package.json` | +react-markdown +remark-gfm |
| 修改 | `src/config/constants.ts` | +17 常量（含 I2-history 前置常量） |
| 新建 | `src/stores/chatStore.ts` | 对话状态管理（含 I2-history 前置方法） |
| 新建 | `src/stores/providerStore.ts` | Provider 状态管理（含预设/格式体系） |
| 新建 | `src/chat/streamCoalescer.ts` | 流式增量合并器 |
| 新建 | `src/chat/chatErrors.ts` | ChatError 错误体系（13 种错误码） |
| 新建 | `src/chat/ChatView.tsx` | 对话主视图 |
| 新建 | `src/chat/MessageList.tsx` | 消息列表 |
| 新建 | `src/chat/MessageList.module.css` | 消息列表 CSS Modules |
| 新建 | `src/chat/MessageBubble.tsx` | 消息气泡 |
| 新建 | `src/chat/MessageBubble.module.css` | 消息气泡 CSS Modules |
| 新建 | `src/chat/MessageInput.tsx` | 输入区 |
| 新建 | `src/chat/ModelSelector.tsx` | 模型选择 |
| 新建 | `src/chat/ProviderSelector.tsx` | Provider 选择（对话栏内嵌） |
| 新建 | `src/chat/ConversationSidebar.tsx` | 对话列表侧栏 |
| 新建 | `src/chat/TemplateCards.tsx` | Prompt 模板卡片 |
| 新建 | `src/chat/HistoryManager.tsx` | 消息历史管理页（I2-history 前置 UI） |
| 修改 | `src/orb/OrbPanel.tsx` | 接入 ChatView |
| 新建 | `src/settings/ProviderManager.tsx` | Provider 设置页 |
| 新建 | `src/settings/AddProviderDialog.tsx` | Provider 预设选择对话框 |
| 新建 | `src/settings/ConnectDialog.tsx` | Provider 连接配置对话框 |
| 新建 | `src/settings/ProviderSwatch.tsx` | Provider 格式标签 |
| 新建 | `src/settings/TemplateManager.tsx` | 模板管理页 |
| 新建 | `src/settings/settingListeners.ts` | 设置事件监听 |
| 新建 | `src/settings/settingsDispatcher.ts` | 设置事件分发 |
| 修改 | `src/App.tsx` | 路由扩展（+/history +/settings/providers +/settings/templates）+ 侧边栏扩展 |
| 修改 | `src/orb/OrbContextMenu.tsx` | 启用新建对话 |
| 修改 | `src/assets/styles/global.css` | 对话相关 CSS |
| 修改 | `docs/DEVLOG.md` | I2 开发日志 |

---

## 未完成项

### 简化/偏差

- A8: `send_chat_message` 中 `tokio::spawn` 内 `get_db_conn` 通过 `app_clone.state()` 获取，生命周期依赖 Tauri State 而非显式 Arc，极端并发下可能锁竞争
- B3: chatStore `deleteConversation` 未弹确认弹窗（文档要求确认后删除），直接调用 invoke（ConversationSidebar 已有 Modal.confirm 兜底）
- B15: ProviderForm 编辑时 API Key 占位符判断逻辑在 Rust 端（`starts_with("••••")`），但前端 `Input.Password` 无法区分"未修改"和"清空"，用户清空 Key 会被当作"未修改"

### 设计变更（相对原始文档，已同步到 I2-spec）

- **B64 加密辅助函数**：crypto.rs 新增 encrypt/decrypt_api_key_b64，直接对接 SQLite TEXT 字段（已写入 spec §2.2）
- **preview_models_cmd**：新增保存前预览模型列表命令，不依赖已保存 Provider（已写入 spec §2.4 + §3.4）
- **update_message_content**：conversations.rs 新增，流式完成后写回完整内容（已写入 spec §2.8）
- **ProviderError / ProviderKind / ProviderConfig / 工厂函数**：base.rs 新增，工程必需（已写入 spec §2.3）
- **额外依赖**：tokio-stream / async-stream / rand / thiserror（已写入 spec §3.1）
- **ensureCurrentConversation**：chatStore 初始化时保证存在真实对话（已写入 spec §4.2）
- **streamingMessageId**：流式输出中助手消息 ID，精确追加 delta（已写入 spec §4.2）
- **强制当前对话**：有 Provider 则自动创建对话，不使用虚拟会话
- **无 Provider 不阻断**：ChatView 始终渲染完整界面，无 Provider 时消息区显示引导
- **路由修复**：App.tsx Menu 导航从 `window.location.hash` 改为 `useNavigate()`，加 `selectedKeys` 高亮当前路由
- **OrbPanel 始终渲染**：`.orb-panel` 始终在 DOM 中，收起/展开动画由 CSS transition 驱动

### 实现扩展（相对 I2-spec 原始范围，代码中已实现但 spec 未记录）

- **ChatError 错误体系**：`chatErrors.ts` 实现 ChatErrorCode 枚举（13 种变体）+ ChatError 接口 + formatRetryAfter + toChatError 转换，超出 spec 原始 5 种 ProviderError（Rust 端），前端独立扩展了 AuthFailed / ContextOverflow / NetworkError / Timeout / ServerError / RequestRejected / Cancelled / StreamError / InternalError
- **ProviderSelector 组件**：`ProviderSelector.tsx` 内嵌对话栏的多 Provider 下拉选择，spec 未单独列出此组件
- **AddProviderDialog / ConnectDialog / ProviderSwatch**：Provider 设置拆分为预设选择对话框 + 连接配置对话框 + 格式标签组件，比 spec 中单一 ProviderForm 更细化
- **HistoryManager 组件**：`HistoryManager.tsx` 主窗口消息历史管理页，含时间分组、搜索、导出、批量操作，已接入 `/history` 路由（App.tsx），此为 I2-history 前置 UI 骨架
- **settingListeners / settingsDispatcher**：设置页事件监听与分发模块，spec 未列出
- **MessageBubble.module.css / MessageList.module.css**：CSS Modules 样式隔离，spec 列为 B19 global.css 补充，实际采用 CSS Modules 方案
- **sse_log.rs**：Rust 端 SSE 日志模块，辅助 OpenAI 流式调试，spec 未列出
- **chatStore 扩展方法**：`updateModel` / `deleteMessage` / `loadMoreMessages` / `searchConversations` / `clearSearch` / `hasMoreMessages` / `isLoadingMoreMessages` / `totalMessageCount` / `searchResults` / `searchQuery`，部分为 I2-history 前置实现
- **get_conversation_paginated / export_conversation / search_conversations**：Rust 端已实现部分 I2-history commands，作为 I2 迭代后期的前置实现

### 后续优化（I2-history）

- 虚拟滚动（>100 条消息性能，react-virtuoso）
- `normalizeProviderBaseUrl`（去除 `/chat/completions` 等后缀，参考 OpenBitFun）
- `withCoalescedDeltas` 高阶包装 + `accept` redelivery 过滤（参考 OpenHuman）
- Schema v2 迁移（conversations 新增 9 列 + messages 新增 4 列 + tool_calls 新表）
- Branded Type / ContentBlock / MessageSource 类型系统
- Persist Queue 串行化 / 自动重试 / 上下文监控 / 草稿保存
- 消息编辑 / 重新生成 / 分支切换 / Tool Call 状态追踪
- 代码高亮（shiki）/ 数学公式（KaTeX）/ Reasoning 折叠 / TTS 朗读
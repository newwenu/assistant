# I2-History — 聊天消息历史管理 任务清单

> 规格：[I2-history-spec.md](./I2-history-spec.md)
> 前置：I2 ✅ 已完成

---

## 阶段 A — Rust 后端：存储 CRUD 扩展

### ~~A1. Schema：编写迁移函数，1→0 版本迁移~~ ❌已取消

> **⚠️ 可靠程度有问题，迁移路径已取消**（详见 I2-history-spec §3.5）
>
> 取消理由：`ALTER TABLE ADD COLUMN` 无法幂等执行，迁移函数可靠性无法保证；项目尚在内测阶段无外部用户，旧 DB（v0）几乎不存在；`CREATE TABLE IF NOT EXISTS` 兜底已足够。

- [x] `db.rs`：`init_db` 中 `CREATE TABLE IF NOT EXISTS` 包含完整表结构（含新字段），新 DB 直接写入 `schema_version = 1` ✅
- [x] `migrations.rs`：保留空壳函数，不执行迁移逻辑 ✅
- [x] `lib.rs`：导出 `pub mod migrations;` ✅
- ~~迁移 v1→v0（降级兼容）~~ ❌已取消
- ~~验证：旧 DB（v0）迁移后表结构正确~~ ❌已取消

**依赖**：无

---

### A2. ConversationRow / MessageRow 结构体扩展

- [x] `conversations.rs`：ConversationRow 新增字段（is_pinned/generation/sort_order/title_source）✅
- [x] `conversations.rs`：MessageRow 新增字段（parent_id/is_pinned）✅
- [x] `conversations.rs`：get_conversations 查询新增字段 ✅
- [x] `conversations.rs`：insert_conversation / update_conversation 新增字段（含 title_source 默认 `'placeholder'`）✅
- [x] `conversations.rs`：get_conversation_detail 新增字段 ✅

**依赖**：A1

---

### A3. ToolCallRow 新模块 ⏸️暂缓

> **暂缓理由**：总架构声明 Function Calling 暂不实现（ARCHITECTURE.md I6），无 agent loop 则 tool_calls 表的 result/status/finished_at 永远无值可写，建表和 CRUD 无意义。

- [ ] `tool_calls.rs`：ToolCallRow 结构体（id/message_id/name/arguments/result/status/started_at/finished_at/is_error）
- [ ] `tool_calls.rs`：insert_tool_call / update_tool_call_status / get_tool_calls
- [ ] `lib.rs`：导出 `pub mod tool_calls;`

**依赖**：A1

---

### A4. conversations.rs 新增 CRUD 函数

- [x] `get_messages_paginated(conn, conversation_id, limit, before_timestamp)` — 分页查询 ✅
- [x] `toggle_pin(conn, id)` — 切换置顶（is_pinned 取反，置顶时 sort_order = MAX+1）✅
- [x] `duplicate_conversation(conn, id)` — 复制对话（新 ID + 标题加" (副本)" + 复制全部消息）✅
- [x] `search_conversations(conn, query)` — 消息内容搜索（同时搜索标题 `c.title LIKE` 和消息内容 `m.content LIKE`）✅
- [x] `rename_conversation(conn, id, title, title_source)` — 重命名对话（含 `title_source` 更新）✅
- [ ] `generate_conversation_title(conn, id)` — AI 标题生成 | ⏸️暂缓
- [x] `batch_delete(conn, ids)` — 批量删除 ✅
- [x] `purge_all(conn)` — 清空所有对话和消息 ✅
- [x] `increment_generation(conn, id)` — 递增 generation ✅
- [ ] `update_conversation_stats(conn, id)` — 更新 message_count 冗余缓存 | ⏸️暂缓：message_count 暂缓

**依赖**：A2

---

### A5. messages.rs 新增 CRUD 函数

- [x] `delete_message(conn, id)` — 删除单条消息 ✅
- [x] `edit_message(conn, id, new_content)` — 编辑消息内容 + 更新 timestamp ✅
- [x] `export_conversation_json(conn, id)` — 导出 JSON ✅
- [x] `export_conversation_markdown(conn, id)` — 导出 Markdown ✅

**依赖**：A2

---

### A6. Tauri Commands — 对话高级操作

- [x] `get_conversation_paginated` — 分页获取对话 ✅
- [x] `toggle_conversation_pin` — 切换置顶 ✅
- [x] `duplicate_conversation` — 复制对话 ✅
- [x] `export_conversation` — 导出对话（format: json/markdown/text）✅
- [x] `batch_delete_conversations` — 批量删除 ✅
- [x] `purge_conversations` — 清空所有 ✅
- [x] `search_conversations` — 消息内容搜索（标题搜索由前端本地过滤）✅
- [x] `rename_conversation` — 重命名对话（含 `title_source` 参数）✅
- [ ] `generate_conversation_title` — AI 标题生成 | ⏸️暂缓

**依赖**：A4

---

### A7. Tauri Commands — 消息操作

- [x] `edit_message` — 编辑消息 ✅
- [x] `regenerate_message` — 重新生成（删除该消息 + 重新发送）✅
- [x] `delete_message_persist` — 持久化删除消息 ✅

**依赖**：A5

---

### A8. Tauri Commands — 流式事件扩展 ⏸️暂缓

> **暂缓理由**：同 A3，总架构声明 Function Calling 暂不实现，send_chat_message 无 agent loop，无法 emit tool_call_status/result/finished_at 事件。

- [ ] `send_chat_message` 扩展：流式消费中 emit tool_call_status / tool_call_result / tool_call_started_at / tool_call_finished_at
- [ ] `chat_tasks.rs`：流式消费中插入 tool_calls 表记录

**依赖**：A3, A6

---

### A9. lib.rs 注册 + setup 集成

- [x] `lib.rs` invoke_handler 注册 A6~A8 所有新 commands ✅
- [x] `setup.rs`：确认新表结构生效 ✅（DbState + ChatTasks managed state）

**依赖**：A6, A7, A8

---

### A⏸️. 暂缓 — Rust 后端

- [ ] `save_draft(conn, id, content)` — 保存草稿（输入框"串台"问题使用频率低）
- [ ] `set_color_mark(conn, id, color)` — 设置颜色标记
- [ ] `set_labels(conn, id, labels)` — 设置标签
- [ ] `update_feedback(conn, id, feedback)` — 更新消息反馈
- [ ] `get_children(conn, parent_id)` — 获取分支子消息
- [ ] `import_conversation_json(conn, data)` — 导入 JSON
- [ ] `save_conversation_draft` — Tauri Command 保存草稿
- [ ] `import_conversation` — Tauri Command 导入对话
- [ ] `set_conversation_color_mark` — Tauri Command 设置颜色标记
- [ ] `set_conversation_labels` — Tauri Command 设置标签
- [ ] `update_message_feedback` — Tauri Command 更新反馈
- [ ] ConversationRow 新增 color_mark / labels / total_tokens / message_count / draft_content / draft_updated_at 字段
- [ ] MessageRow 新增 source / feedback 字段（同 §2.3 / §4.10 暂缓）

---

## 阶段 B — 前端：类型系统 + Store 扩展 + 核心功能

### B1. 前端依赖安装

- [x] `npm install remark-math rehype-katex katex` ✅
- [ ] `npm install shiki` | ⏸️暂缓：shiki 含全部 TextMate 语法+主题约 5-10MB，当前阶段核心是存储 CRUD+分页+对话操作，代码高亮非核心，延后到渲染增强阶段再装

**依赖**：无

---

### B3. ContentBlock 类型定义 ⏸️暂缓

> **暂缓理由**：同 §2.2，当前 content:string + toolCalls + reasoning 够用，改为 ContentBlock[] 是巨大重构且总架构声明 Function Calling 暂不实现。

- [ ] `src/types/content.ts`：ContentBlock 联合类型（text/image/tool_call/tool_result/thinking）
- [ ] `src/types/message.ts`：MessageSource 联合类型
- [ ] `chatStore.ts`：Message.content 类型从 `string` 改为 `ContentBlock[]`
- [ ] 向后兼容：读取纯字符串时自动包装为 `[{ type: 'text', text: content }]`

**依赖**：无

---

### B4. 常量新增

- [x] `src/config/constants.ts`：新增 MESSAGES_PAGE_SIZE / CONTEXT_WARN_THRESHOLD / CONTEXT_DANGER_THRESHOLD / CONVERSATION_SUMMARY_LENGTH / SCROLL_LOAD_THRESHOLD_PX ✅（含 VIRTUAL_SCROLL_THRESHOLD / VIRTUAL_SCROLL_OVERSCAN）

**依赖**：无

---

### B5. chatStore 扩展 — Persist Queue

- [x] `src/stores/chatStore.ts`：实现 `enqueuePersist` 串行化队列 ✅（PersistQueue class）
- [x] 所有写操作（renameConversation / deleteConversation / sendMessage 等）改用 `enqueuePersist` ✅
- [ ] 验证：快速连续操作无数据丢失 ❓需手动验收

**依赖**：无

---

### B6. chatStore 扩展 — 分页加载

- [x] ChatState 新增：hasMoreMessages / isLoadingMoreMessages ✅
- [x] `loadMoreMessages()`：调用 `get_conversation_paginated`，before_timestamp 取 messages[0].timestamp ✅
- [x] `switchConversation()`：改为调用 `get_conversation_paginated`（limit = MESSAGES_PAGE_SIZE），不再调用 `get_conversation` ✅

**依赖**：B4, B5

---

### B7. chatStore 扩展 — 对话高级操作

- [x] `togglePin(id)` / `duplicateConversation(id)` ✅
- [x] `exportConversation(id, format)` — 调用 Rust 端 → 触发浏览器下载 ✅
- [x] `searchConversations(query)` — 默认前端本地过滤标题；`searchFullText=true` 时调用 `search_conversations` 搜索消息内容 ✅
- [x] `searchFullText: boolean` — 全文搜索开关（默认 false）✅
- [x] `toggleSearchFullText()` — 切换全文搜索开关 ✅
- [x] `renameConversation(id, title, titleSource?)` — 重命名对话（含 `title_source` 更新，默认 `'user'`）✅
- [ ] `generateConversationTitle(id)` — AI 标题生成 | ⏸️暂缓
- [x] `filterProviderId` — 纯前端筛选状态 ✅
- [x] `batchDelete()` — 调用 `batch_delete_conversations` ✅
- [x] `purgeAll()` — 调用 `purge_conversations` ✅

**依赖**：B5

---

### B8. chatStore 扩展 — 消息编辑与重新生成

- [x] `editMessage(messageId, newContent)` — 调用 `edit_message` + 删除后续消息 + 重新发送 ✅
- [x] `regenerateMessage(messageId)` — 调用 `regenerate_message` ✅
- [x] `deleteMessage(id)` — 改为调用 `delete_message_persist` 持久化删除 ✅

**依赖**：B5

---

### B8.1 chatStore 扩展 — 对话标题生成

- [x] `fallbackTitleFromUserMessage(userMessage, maxLength=30)` — 从用户消息智能截取标题（首行非空文本 + 句末标点优先断开 + 省略号）✅
- [x] `sendMessage` 中：用户消息追加后，若 `title_source === 'placeholder'`，fire-and-forget 调用 `fallbackTitleFromUserMessage` + `renameConversation(id, title, 'fallback')` ✅
- [x] `renameConversation(id, title, titleSource='user')` — 用户手动重命名时 `title_source = 'user'`，后续自动标题不再覆盖 ✅
- [x] 移除 `finishStream` 中旧的 `lastAssistant.content.slice(0, 20)` 标题逻辑 ✅
- [ ] `generateConversationTitle(id)` — AI 标题生成 | ⏸️暂缓

**依赖**：B5

---

### B9. chatStore 扩展 — Tool Call 状态追踪 ⏸️暂缓

> **暂缓理由**：同 §2.6，总架构声明 Function Calling 暂不实现，无 agent loop 则 tool_call_status/result/finished_at 无法产生。

- [ ] ChatDeltaPayload 新增字段：tool_call_status / tool_call_result / tool_call_started_at / tool_call_finished_at
- [ ] `initChatDeltaListener` 扩展：处理 tool_call_status 更新 → 更新 messages 中对应 ToolCallEntry
- [ ] Message.toolCalls 类型从 `ToolCall[]` 改为 `ToolCallEntry[]`（含 status/result/started_at/finished_at）

**依赖**：B3

---

### B10. chatStore 扩展 — 上下文监控 + Generation Counter

- [x] ChatState 新增：contextUsage（used_tokens / max_tokens / percentage）✅
- [x] `finishStream` 更新 contextUsage（从 usage 计算）✅
- [x] `sendMessage` 时重置 contextUsage ✅
- [x] `finishStream` 递增 generation ✅（fire-and-forget 调用 `invoke("increment_generation")`）

**依赖**：B4

---

### B⏸️. 暂缓 — 前端 Store

- [ ] B2. Branded Type 定义：MessageId / ConversationId / ToolCallId / ProviderId
- [ ] B3. ContentBlock 类型定义（同 §2.2 暂缓）
- [ ] B9. Tool Call 状态追踪（同 §2.6 暂缓）
- [ ] B11. 自动重试：retryWithBackoff 指数退避
- [ ] B12. 草稿保存：switchConversation 中 saveDraft / draft_content 恢复（输入框"串台"问题使用频率低）
- [ ] MessageSource 类型定义（同 §2.3 暂缓）
- [ ] `saveDraft(content)` / `updateFeedback(messageId, feedback)` / `switchBranch(parentId, index)` / `activeBranchMap`
- [ ] `multiSelectMode` / `selectedConversationIds` — 多选状态
- [ ] VIRTUAL_SCROLL_THRESHOLD / VIRTUAL_SCROLL_OVERSCAN / RETRY_MAX_ATTEMPTS / RETRY_BASE_DELAY_MS / RETRY_MAX_DELAY_MS 常量

---

## 阶段 C — 前端：UI 组件

### C1. ConversationSidebar 重构

- [x] 时间分组：`groupConversations()` 函数 → 置顶 / 今天 / 昨天 / 近 7 天 / 近 30 天 / 更早 ✅
- [x] Provider 过滤标签栏（Tabs 组件）✅
- [x] 对话摘要预览（标题 + 相对时间）✅
- [x] 置顶📌 标识 ✅
- [x] 右键菜单扩展：置顶/复制/导出/删除 ✅

**依赖**：B7

---

### C2. ConversationGroup 组件

- [x] 分组标题 + 可折叠 + 对话列表 ✅（内联在 ConversationSidebar 中，未独立组件但功能完整）

**依赖**：C1

---

### C3. MessageList 重构 — 分页加载

- [x] `startReached` 触发 `loadMoreMessages` ✅
- [x] 顶部加载骨架屏 ✅（skeletonLine CSS + LoadingOutlined）
- [x] 新消息自动滚底 ✅
- [x] "回到底部"按钮 ✅

**依赖**：B6

---

### C4. MessageActionBar 组件

- [x] `src/chat/MessageActions.tsx`：hover 显示操作栏 ✅
- [x] 📋 复制 / ✏️ 编辑（用户消息）/ 🔄 重新生成（助手消息）/ 🗑️ 删除 ✅
- [x] 操作栏定位：消息区域 ✅

**依赖**：B8

---

### C5. MessageBubble 重构

- [x] 集成 MessageActions ✅
- [x] Reasoning 折叠：默认折叠，显示"💭 展开思考"，点击展开 ✅
- [ ] 代码块语法高亮：集成 `shiki` + 语言标签 + 复制按钮 | ⏸️暂缓：shiki 约 5-10MB，延后安装
- [x] 数学公式：集成 `KaTeX`（remark-math + rehype-katex）✅
- [ ] Tool Call 状态渲染：委托 ToolCallBubble | ⏸️暂缓：同 §2.6

**依赖**：C4, B1

---

### C6. ToolCallBubble 组件 ⏸️暂缓

> **暂缓理由**：同 §2.6，总架构声明 Function Calling 暂不实现，无 agent loop 则只有 running 态，三态渲染无意义。

- [ ] `src/chat/ToolCallBubble.tsx`：三态渲染（running/success/error）
- [ ] running：旋转图标 + 工具名称
- [ ] success：✓ + 名称 + 耗时 + 可折叠结果
- [ ] error：✗ + 错误信息

**依赖**：B9

---

### C7. ToolTimeline 组件 ⏸️暂缓

> **暂缓理由**：同 §2.6，时间线需要 started_at→finished_at 的完整生命周期，无 agent loop 则无法产生。

- [ ] `src/chat/ToolTimeline.tsx`：工具调用时间线视图
- [ ] 按执行顺序展示每个 ToolCallEntry
- [ ] 可折叠/展开

**依赖**：C6

---

### C9. ContextBar 组件

- [x] `src/chat/ContextBar`（内联 ChatView.tsx）：上下文窗口使用量进度条 ✅
- [x] 格式：Progress 组件 ✅
- [x] >80% 黄色警告，>95% 红色警告 ✅
- [x] 位于 ChatView 输入区上方 ✅

**依赖**：B10

---

### C10. SearchBar 组件

- [x] 对话搜索（内嵌 ConversationSidebar）✅
- [x] 输入防抖 300ms → `chatStore.searchConversations` ✅
- [x] 默认模式：前端本地过滤 `conversations` by `title`（即时响应）✅
- [x] 全文搜索开关：`Switch` / `Tooltip` 组件，切换 `chatStore.searchFullText` ✅
- [x] 全文搜索模式：调用后端 `search_conversations` 搜索消息内容 ✅
- [x] 搜索结果替换侧栏对话列表 ✅
- [x] 清空搜索恢复原列表 ✅

**依赖**：B7

---

### C11. ChatView 修改

- [x] 集成 ContextBar（输入区上方）✅
- [x] Token 计数器：ContextBar Tooltip 显示 token 数 ✅

**依赖**：C9

---

### C13. CSS 样式补充

- [x] 对话分组标题样式 ✅（global.css: chat-sidebar-group / chat-sidebar-group-header 等）
- [x] 消息操作栏样式 ✅（MessageActions.module.css）
- [ ] Tool Call 气泡样式（三态颜色） | ⏸️暂缓：同 §2.6
- [x] 上下文进度条样式 ✅
- [x] 骨架屏样式 ✅（MessageList.module.css: skeletonLine + skeleton-pulse）

**依赖**：C1~C11

---

### C⏸️. 暂缓 — 前端 UI

- [ ] C6. ToolCallBubble 组件（同 §2.6 暂缓）
- [ ] C7. ToolTimeline 组件（同 §2.6 暂缓）
- [ ] C8. BranchSwitcher 组件：分支切换器 ◀ 1/N ▶
- [ ] C12. TTS 语音朗读：Web Speech API
- [ ] 虚拟滚动：react-virtuoso 集成
- [ ] 多选模式：☑ 按钮 + Checkbox + 底部操作栏
- [ ] 键盘导航：↑↓ / Enter / Delete / Escape
- [ ] 颜色圆点标识 / 颜色标记右键菜单
- [ ] 消息反馈 UI：点赞/点踩图标
- [ ] 流式速率指示：tokens/s
- [ ] awaiting_user / cancelled 态渲染

---

## 阶段 D — 验证

### D1. 编译验证

- [ ] `cargo build` 通过
- [ ] `npm run typecheck` 通过
- [ ] `npm run lint` 通过

**依赖**：A9, C13

---

### D2. 功能验收

- [ ] I2-history-spec §7 验收标准（非暂缓项）全部通过

**依赖**：D1

---

### D3. DEVLOG 记录

- [x] `docs/DEVLOG.md`：新增 I2-History 章节 ✅（2026-10-06，含迁移取消决策/Rust新增/Store扩展/UI组件/功能缺失修复/行距修复/内联组件评估）

**依赖**：D2

---

## 任务依赖图

> 注：A3/A8/B3/B9/C6/C7 已标 ⏸️暂缓（依赖总架构 I6 Agent Loop），依赖图中以虚线表示。

```
A1 → A2 → A4 → A6 ─┐
   → A3 ────────→ A8 ┤  ⏸️暂缓（同 §2.6）
   → A5 ────→ A7 ────┤
                       ├→ A9 → D1 → D2 → D3
B1 ──────────────────┐│
B3 ──────────────────┤│  ⏸️暂缓（同 §2.2）
B4 → B6 → C3 ───────┤│
B5 → B7 → C1 → C2   ││
   → B8 → C4 → C5   ││
   → B10 → C9       ││
B1 → C5             ││
B9 ──────────────────┤│  ⏸️暂缓（同 §2.6）
C6 → C7             ││  ⏸️暂缓（同 §2.6）
C10                 ││
C13 ────────────────┘│
                     ┘
```
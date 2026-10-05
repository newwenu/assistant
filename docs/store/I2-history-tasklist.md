# I2-History — 聊天消息历史管理 任务清单

> 规格：[I2-history-spec.md](./I2-history-spec.md)
> 前置：I2 ✅ 已完成

---

## 阶段 A — Rust 后端：Schema 迁移 + 存储 CRUD 扩展

### A1. Schema v2 迁移

- [ ] `db.rs` `run_migrations`：增加 v2 迁移（conversations 新增 9 列 + messages 新增 4 列 + tool_calls 新表 + 2 个索引）
- [ ] 迁移后 `schema_version` 写入 `2`
- [ ] 验证：`cargo build` 通过 + 新建 DB 后 conversations 表含 is_pinned/is_starred/color_mark/labels/draft_content/draft_updated_at/message_count/total_tokens/generation/sort_order 列

**依赖**：无

---

### A2. ConversationRow / MessageRow 结构体扩展

- [ ] `conversations.rs`：ConversationRow 新增 9 个字段（is_pinned/is_starred/color_mark/labels/draft_content/draft_updated_at/message_count/total_tokens/generation/sort_order）
- [ ] `conversations.rs`：MessageRow 新增 4 个字段（parent_id/source/feedback/is_pinned）
- [ ] `conversations.rs`：get_conversations 查询新增字段
- [ ] `conversations.rs`：insert_conversation / update_conversation 新增字段
- [ ] `conversations.rs`：get_conversation_detail 新增字段

**依赖**：A1

---

### A3. ToolCallRow 新模块

- [ ] `tool_calls.rs`：ToolCallRow 结构体（id/message_id/name/arguments/result/status/started_at/finished_at/is_error）
- [ ] `tool_calls.rs`：insert_tool_call / update_tool_call_status / get_tool_calls
- [ ] `lib.rs`：导出 `pub mod tool_calls;`

**依赖**：A1

---

### A4. conversations.rs 新增 CRUD 函数

- [ ] `get_messages_paginated(conn, conversation_id, limit, before_timestamp)` — 分页查询
- [ ] `toggle_pin(conn, id)` — 切换置顶（is_pinned 取反，置顶时 sort_order = MAX+1）
- [ ] `toggle_star(conn, id)` — 切换收藏
- [ ] `save_draft(conn, id, content)` — 保存草稿
- [ ] `duplicate_conversation(conn, id)` — 复制对话（新 ID + 标题加" (副本)" + 复制全部消息）
- [ ] `search_conversations(conn, query)` — 全文搜索（JOIN messages + LIKE）
- [ ] `batch_delete(conn, ids)` — 批量删除
- [ ] `purge_all(conn)` — 清空所有对话和消息
- [ ] `increment_generation(conn, id)` — 递增 generation
- [ ] `update_conversation_stats(conn, id)` — 更新 message_count/total_tokens 冗余缓存
- [ ] `set_color_mark(conn, id, color)` — 设置颜色标记
- [ ] `set_labels(conn, id, labels)` — 设置标签

**依赖**：A2

---

### A5. messages.rs 新增 CRUD 函数

- [ ] `delete_message(conn, id)` — 删除单条消息（含级联 tool_calls）
- [ ] `edit_message(conn, id, new_content)` — 编辑消息内容 + 更新 updated_at
- [ ] `update_feedback(conn, id, feedback)` — 更新消息反馈
- [ ] `get_children(conn, parent_id)` — 获取分支子消息
- [ ] `export_conversation_json(conn, id)` — 导出 JSON
- [ ] `export_conversation_markdown(conn, id)` — 导出 Markdown
- [ ] `import_conversation_json(conn, data)` — 导入 JSON

**依赖**：A2

---

### A6. Tauri Commands — 对话高级操作

- [ ] `get_conversation_paginated` — 分页获取对话
- [ ] `toggle_conversation_pin` — 切换置顶
- [ ] `toggle_conversation_star` — 切换收藏
- [ ] `save_conversation_draft` — 保存草稿
- [ ] `duplicate_conversation` — 复制对话
- [ ] `export_conversation` — 导出对话（format: json/markdown/text）
- [ ] `import_conversation` — 导入对话
- [ ] `batch_delete_conversations` — 批量删除
- [ ] `purge_conversations` — 清空所有
- [ ] `search_conversations` — 全文搜索
- [ ] `set_conversation_color_mark` — 设置颜色标记
- [ ] `set_conversation_labels` — 设置标签

**依赖**：A4

---

### A7. Tauri Commands — 消息操作

- [ ] `edit_message` — 编辑消息
- [ ] `regenerate_message` — 重新生成（删除该消息 + 重新发送）
- [ ] `delete_message_persist` — 持久化删除消息
- [ ] `update_message_feedback` — 更新反馈

**依赖**：A5

---

### A8. Tauri Commands — 流式事件扩展

- [ ] `send_chat_message` 扩展：流式消费中 emit tool_call_status / tool_call_result / tool_call_started_at / tool_call_finished_at
- [ ] `chat_tasks.rs`：流式消费中插入 tool_calls 表记录

**依赖**：A3, A6

---

### A9. lib.rs 注册 + setup 集成

- [ ] `lib.rs` invoke_handler 注册 A6~A8 所有新 commands
- [ ] `setup.rs`：确认 migration v2 自动执行

**依赖**：A6, A7, A8

---

## 阶段 B — 前端：类型系统 + Store 扩展 + 核心功能

### B1. 前端依赖安装

- [ ] `npm install react-virtuoso shiki katex remark-math rehype-katex`

**依赖**：无

---

### B2. Branded Type 定义

- [ ] `src/types/branded.ts`：MessageId / ConversationId / ToolCallId / ProviderId + as* 转换函数
- [ ] `chatStore.ts`：所有 id 字段改用 branded type
- [ ] 验证：`npm run typecheck` 通过

**依赖**：无

---

### B3. ContentBlock 类型定义

- [ ] `src/types/content.ts`：ContentBlock 联合类型（text/image/tool_call/tool_result/thinking）
- [ ] `src/types/message.ts`：MessageSource 联合类型
- [ ] `chatStore.ts`：Message.content 类型从 `string` 改为 `ContentBlock[]`
- [ ] 向后兼容：读取纯字符串时自动包装为 `[{ type: 'text', text: content }]`

**依赖**：B2

---

### B4. 常量新增

- [ ] `src/config/constants.ts`：新增 MESSAGES_PAGE_SIZE / VIRTUAL_SCROLL_THRESHOLD / VIRTUAL_SCROLL_OVERSCAN / CONTEXT_WARN_THRESHOLD / CONTEXT_DANGER_THRESHOLD / RETRY_MAX_ATTEMPTS / RETRY_BASE_DELAY_MS / RETRY_MAX_DELAY_MS / CONVERSATION_SUMMARY_LENGTH / SCROLL_LOAD_THRESHOLD_PX

**依赖**：无

---

### B5. chatStore 扩展 — Persist Queue

- [ ] `src/stores/chatStore.ts`：实现 `enqueuePersist` 串行化队列
- [ ] 所有写操作（renameConversation / deleteConversation / sendMessage 等）改用 `enqueuePersist`
- [ ] 验证：快速连续操作无数据丢失

**依赖**：无

---

### B6. chatStore 扩展 — 分页加载

- [ ] ChatState 新增：hasMoreMessages / isLoadingMoreMessages
- [ ] `loadMoreMessages()`：调用 `get_conversation_paginated`，before_timestamp 取 messages[0].timestamp
- [ ] `switchConversation()`：改为调用 `get_conversation_paginated`（limit = MESSAGES_PAGE_SIZE），不再调用 `get_conversation`

**依赖**：B4, B5

---

### B7. chatStore 扩展 — 对话高级操作

- [ ] `togglePin(id)` / `toggleStar(id)` / `saveDraft(content)` / `duplicateConversation(id)`
- [ ] `exportConversation(id, format)` — 调用 Rust 端 → 触发浏览器下载
- [ ] `searchConversations(query)` — 调用 `search_conversations` → 更新 searchResults
- [ ] `filterProviderId` / `filterStarred` — 纯前端筛选状态
- [ ] `multiSelectMode` / `selectedConversationIds` — 多选状态
- [ ] `batchDelete()` — 调用 `batch_delete_conversations`
- [ ] `purgeAll()` — 调用 `purge_conversations`

**依赖**：B5

---

### B8. chatStore 扩展 — 消息编辑与重新生成

- [ ] `editMessage(messageId, newContent)` — 调用 `edit_message` + 删除后续消息 + 重新发送
- [ ] `regenerateMessage(messageId)` — 调用 `regenerate_message`
- [ ] `deleteMessage(id)` — 改为调用 `delete_message_persist` 持久化删除
- [ ] `updateFeedback(messageId, feedback)` — 调用 `update_message_feedback`
- [ ] `switchBranch(parentId, index)` — 切换分支
- [ ] `activeBranchMap` — 分支活跃索引映射

**依赖**：B5

---

### B9. chatStore 扩展 — Tool Call 状态追踪

- [ ] ChatDeltaPayload 新增字段：tool_call_status / tool_call_result / tool_call_started_at / tool_call_finished_at
- [ ] `initChatDeltaListener` 扩展：处理 tool_call_status 更新 → 更新 messages 中对应 ToolCallEntry
- [ ] Message.toolCalls 类型从 `ToolCall[]` 改为 `ToolCallEntry[]`（含 status/result/started_at/finished_at）

**依赖**：B3

---

### B10. chatStore 扩展 — 上下文监控 + Generation Counter

- [ ] ChatState 新增：contextUsage（used_tokens / max_tokens / percentage）/ generation
- [ ] `finishStream` 更新 contextUsage（从 usage 计算）+ 递增 generation
- [ ] `sendMessage` 时重置 contextUsage

**依赖**：B4

---

### B11. chatStore 扩展 — 自动重试

- [ ] `src/chat/retryWithBackoff.ts`：实现指数退避重试（maxAttempts / baseDelayMs / maxDelayMs / jitter）
- [ ] 可重试错误判断：NetworkError / Timeout / RateLimited / ServerError / StreamError
- [ ] `sendMessage` 集成：失败时自动重试，UI 显示重试状态

**依赖**：B4

---

### B12. chatStore 扩展 — 草稿保存

- [ ] `switchConversation` 中：切出前调用 `saveDraft(inputContent)`，切入后从 `conversation.draft_content` 恢复
- [ ] ChatView 输入框内容与 draft 同步

**依赖**：B7

---

## 阶段 C — 前端：UI 组件

### C1. ConversationSidebar 重构

- [ ] 时间分组：`groupConversations()` 函数 → 置顶 / 今天 / 昨天 / 近 7 天 / 近 30 天 / 更早
- [ ] Provider 过滤标签栏（Tabs 组件）
- [ ] 收藏筛选按钮（⭐）
- [ ] 对话摘要预览（标题 + 最后消息摘要 30 字符 + 相对时间 + Provider 图标）
- [ ] 置顶📌 / 收藏⭐ / 颜色圆点标识
- [ ] 右键菜单扩展：置顶/收藏/颜色标记/复制/导出/删除
- [ ] 多选模式：☑ 按钮 + Checkbox + 底部操作栏
- [ ] 键盘导航：↑↓ / Enter / Delete / Escape

**依赖**：B7

---

### C2. ConversationGroup 组件

- [ ] `src/chat/ConversationGroup.tsx`：分组标题 + 可折叠 + 对话列表

**依赖**：C1

---

### C3. MessageList 重构 — 虚拟滚动 + 分页

- [ ] 集成 `react-virtuoso`：消息数 > VIRTUAL_SCROLL_THRESHOLD 时启用
- [ ] `startReached` 触发 `loadMoreMessages`
- [ ] 顶部加载骨架屏（3 行）
- [ ] `followOutput="smooth"` 新消息自动滚底
- [ ] "回到底部"按钮

**依赖**：B6

---

### C4. MessageActionBar 组件

- [ ] `src/chat/MessageActionBar.tsx`：hover 显示操作栏
- [ ] 📋 复制（纯文本 / Markdown） / ✏️ 编辑（用户消息）/ 🔄 重新生成（助手消息）/ 👍👎 反馈 / 📌 固定 / 🗑️ 删除
- [ ] 操作栏定位：消息右上角，半透明背景

**依赖**：B8

---

### C5. MessageBubble 重构

- [ ] 集成 MessageActionBar
- [ ] Reasoning 折叠：默认折叠，显示"💭 思考了 X 秒"，点击展开
- [ ] 代码块语法高亮：集成 `shiki` + 语言标签 + 复制按钮
- [ ] 数学公式：集成 `KaTeX`（remark-math + rehype-katex）
- [ ] 消息反馈 UI：点赞/点踩图标
- [ ] Tool Call 状态渲染：委托 ToolCallBubble

**依赖**：C4, B1

---

### C6. ToolCallBubble 组件

- [ ] `src/chat/ToolCallBubble.tsx`：五态渲染
- [ ] running：旋转图标 + 工具名称
- [ ] success：✓ + 名称 + 耗时 + 可折叠结果
- [ ] error：✗ + 错误信息
- [ ] awaiting_user：⚠ + 审批按钮
- [ ] cancelled：⊘ 灰色

**依赖**：B9

---

### C7. ToolTimeline 组件

- [ ] `src/chat/ToolTimeline.tsx`：工具调用时间线视图
- [ ] 按执行顺序展示每个 ToolCallEntry
- [ ] 可折叠/展开

**依赖**：C6

---

### C8. BranchSwitcher 组件

- [ ] `src/chat/BranchSwitcher.tsx`：分支切换器
- [ ] 显示 `◀ 1/N ▶`，◀▶ 切换分支
- [ ] 切换时调用 `chatStore.switchBranch`

**依赖**：B8

---

### C9. ContextBar 组件

- [ ] `src/chat/ContextBar.tsx`：上下文窗口使用量进度条
- [ ] 格式：`████░░ 8192/128000 (6.4%)`
- [ ] >80% 黄色警告，>95% 红色警告
- [ ] 位于 ChatView 输入区上方

**依赖**：B10

---

### C10. SearchBar 组件

- [ ] `src/chat/SearchBar.tsx`：全文搜索
- [ ] 输入防抖 300ms → `chatStore.searchConversations`
- [ ] 搜索结果替换侧栏对话列表
- [ ] 清空搜索恢复原列表

**依赖**：B7

---

### C11. ChatView 修改

- [ ] 集成 ContextBar（输入区上方）
- [ ] 草稿保存：切换对话前保存输入框内容
- [ ] 流式速率指示：流式中显示 tokens/s
- [ ] Token 计数器：输入框右下角显示预估 token 数

**依赖**：C9, B12

---

### C12. TTS 语音朗读

- [ ] MessageActionBar 增加 🔊 朗读按钮
- [ ] 使用 Web Speech API `speechSynthesis.speak()`
- [ ] 朗读中按钮变为 ⏹ 停止

**依赖**：C4

---

### C13. CSS 样式补充

- [ ] 对话分组标题样式
- [ ] 消息操作栏样式（半透明 + 定位）
- [ ] Tool Call 气泡样式（五态颜色）
- [ ] 分支切换器样式
- [ ] 上下文进度条样式
- [ ] 骨架屏样式
- [ ] 多选模式样式

**依赖**：C1~C12

---

## 阶段 D — 验证

### D1. 编译验证

- [ ] `cargo build` 通过
- [ ] `npm run typecheck` 通过
- [ ] `npm run lint` 通过

**依赖**：A9, C13

---

### D2. 功能验收

- [ ] I2-history-spec §7 验收标准 1~20 全部通过

**依赖**：D1

---

### D3. DEVLOG 记录

- [ ] `docs/DEVLOG.md`：新增 I2-History 章节，记录技术决策与踩坑

**依赖**：D2

---

## 任务依赖图

```
A1 → A2 → A4 → A6 ─┐
   → A3 ────────→ A8 ┤
   → A5 ────→ A7 ────┤
                       ├→ A9 → D1 → D2 → D3
B1 ──────────────────┐│
B2 → B3 → B9 ───────┤│
B4 → B6 → C3 ───────┤│
B5 → B7 → C1 → C2   ││
   → B8 → C4 → C5   ││
         → C8        ││
   → B10 → C9       ││
   → B11            ││
   → B12 → C11      ││
B1 → C5             ││
C6 → C7             ││
C10                 ││
C12                 ││
C13 ────────────────┘│
                     ┘
```
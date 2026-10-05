# I2-History — 聊天消息历史管理系统 规格说明

## 1. 交付目标

在 I2 基础对话能力之上，构建完整的聊天消息历史管理系统：消息分页加载、对话分组与高级操作、消息编辑与重新生成、Tool Call 状态追踪、上下文窗口监控、数据持久化安全、消息渲染增强。

> 前置：I2 ✅ 已完成
> 参考：openhuman（Turn State / Tool Timeline）、airi（Persist Queue / Session Index / 云端同步）、deepseek（Branded Type / ContentBlock / MessageSource）、onepoint（Preset 分离历史）

## 2. 数据模型扩展

### 2.1 Branded Type ID

当前 `Message.id` / `Conversation.id` / `ToolCall.id` 均为 `string`，编译期无法区分，易混淆传参。

```typescript
type MessageId = string & { readonly __brand: 'MessageId' }
type ConversationId = string & { readonly __brand: 'ConversationId' }
type ToolCallId = string & { readonly __brand: 'ToolCallId' }
type ProviderId = string & { readonly __brand: 'ProviderId' }

function asMessageId(id: string): MessageId { return id as MessageId }
function asConversationId(id: string): ConversationId { return id as ConversationId }
```

**Rust 端**：无需 branded type（Rust 的 newtype 模式更重，且 Tauri command 序列化边界已做校验），仅前端 TypeScript 使用。

### 2.2 ContentBlock 多模态内容

消息 `content` 从纯 `string` 改为结构化 `ContentBlock[]`，支持文本/图片/工具调用/思考内嵌。

```typescript
type ContentBlock =
  | { type: 'text'; text: string }
  | { type: 'image'; url: string; alt?: string; mime_type?: string }
  | { type: 'tool_call'; id: ToolCallId; name: string; arguments: string }
  | { type: 'tool_result'; id: ToolCallId; content: string; is_error?: boolean }
  | { type: 'thinking'; text: string }

interface Message {
  id: MessageId
  conversation_id: ConversationId
  role: 'system' | 'user' | 'assistant' | 'tool'
  content: ContentBlock[]
  source?: MessageSource
  tokens: number | null
  timestamp: number
  error?: ChatError | null
  parent_id?: MessageId | null
  children_ids?: MessageId[]
  feedback?: 'positive' | 'negative' | null
  is_pinned?: boolean
}
```

**向后兼容**：Rust 端存储仍为 `content TEXT`（JSON 序列化 `ContentBlock[]`），读取时若为纯字符串则自动包装为 `[{ type: 'text', text: content }]`。

### 2.3 MessageSource 来源追踪

每条消息携带来源信息，支持 Provider 切换后重放和 UI 来源标识。

```typescript
type MessageSource =
  | { kind: 'user' }
  | { kind: 'model'; provider: string; model: string; replay_state?: unknown }
  | { kind: 'tool'; call_id: ToolCallId }
  | { kind: 'system-prompt' }
```

### 2.4 消息分支（Branching）

编辑用户消息后产生分支，同一用户消息下可有多个助手回复。

```typescript
interface Message {
  parent_id?: MessageId | null
  children_ids?: MessageId[]
}
```

- 首条消息 `parent_id = null`
- 同一 `parent_id` 下的多个 `children` 为分支
- UI 显示分支切换器（"1 / 3 ◀ ▶"），当前活跃分支高亮
- `chatStore` 增加 `activeBranchMap: Record<MessageId, number>` 记录每个分支节点的当前活跃子节点索引

### 2.5 Conversation 扩展字段

```typescript
interface Conversation {
  id: ConversationId
  title: string
  provider_id: ProviderId | null
  model_id: string
  system_prompt: string | null
  is_pinned: boolean
  is_starred: boolean
  color_mark: string | null
  labels: string[]
  draft_content: string | null
  draft_updated_at: number | null
  message_count: number
  total_tokens: number
  generation: number
  created_at: number
  updated_at: number
}
```

| 新增字段 | 类型 | 说明 | 参考 |
|---------|------|------|------|
| `is_pinned` | boolean | 置顶，排序优先 | 通用 |
| `is_starred` | boolean | 收藏，快速筛选 | 通用 |
| `color_mark` | string \| null | 颜色标记（如 `"red"` / `"blue"`） | 通用 |
| `labels` | string[] | 标签/分类 | openhuman `Thread.labels` |
| `draft_content` | string \| null | 输入框草稿 | 通用 |
| `draft_updated_at` | number \| null | 草稿更新时间 | 通用 |
| `message_count` | number | 消息数（冗余缓存，避免 COUNT 查询） | openhuman |
| `total_tokens` | number | 总 token 数（冗余缓存） | openhuman |
| `generation` | number | 渲染优化版本号 | airi `sessionGenerations` |

### 2.6 Tool Call 状态模型

当前 `ToolCall` 仅有 `id/name/arguments`，无状态追踪。扩展为五态模型：

```typescript
type ToolCallStatus = 'running' | 'success' | 'error' | 'awaiting_user' | 'cancelled'

interface ToolCallEntry {
  id: ToolCallId
  name: string
  arguments: string
  result?: string
  status: ToolCallStatus
  started_at: number
  finished_at: number | null
  is_error?: boolean
  subagent?: SubagentActivity
}

interface SubagentActivity {
  task_id: string
  agent_id: string
  status: 'running' | 'awaiting_user' | 'completed' | 'failed'
  display_name?: string
  child_iteration?: number
  child_max_iterations?: number
  iterations?: number
  elapsed_ms?: number
  output?: string
  tool_calls: ToolCallEntry[]
}
```

### 2.7 TokenUsage 扩展

```typescript
interface TokenUsage {
  prompt_tokens: number
  completion_tokens: number
  total_tokens: number
  cache_read_tokens?: number
  cache_write_tokens?: number
  cost_usd?: number
  context_window_tokens?: number
}
```

### 2.8 ChatError 扩展

```typescript
enum ChatErrorCode {
  ApiKeyMissing = "ApiKeyMissing",
  AuthFailed = "AuthFailed",
  RateLimited = "RateLimited",
  ModelNotFound = "ModelNotFound",
  ContextOverflow = "ContextOverflow",
  NetworkError = "NetworkError",
  Timeout = "Timeout",
  ServerError = "ServerError",
  RequestRejected = "RequestRejected",
  ParseError = "ParseError",
  Cancelled = "Cancelled",
  StreamError = "StreamError",
  InternalError = "InternalError",
  GuardrailBlocked = "GuardrailBlocked",
  BudgetExhausted = "BudgetExhausted",
  ToolTimeout = "ToolTimeout",
  SubagentFailed = "SubagentFailed",
}
```

## 3. SQLite Schema 变更

### 3.1 conversations 表扩展

```sql
ALTER TABLE conversations ADD COLUMN is_pinned INTEGER NOT NULL DEFAULT 0;
ALTER TABLE conversations ADD COLUMN is_starred INTEGER NOT NULL DEFAULT 0;
ALTER TABLE conversations ADD COLUMN color_mark TEXT;
ALTER TABLE conversations ADD COLUMN labels TEXT NOT NULL DEFAULT '[]';
ALTER TABLE conversations ADD COLUMN draft_content TEXT;
ALTER TABLE conversations ADD COLUMN draft_updated_at INTEGER;
ALTER TABLE conversations ADD COLUMN message_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE conversations ADD COLUMN total_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE conversations ADD COLUMN generation INTEGER NOT NULL DEFAULT 0;
ALTER TABLE conversations ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;
```

### 3.2 messages 表扩展

```sql
ALTER TABLE messages ADD COLUMN parent_id TEXT;
ALTER TABLE messages ADD COLUMN source TEXT;
ALTER TABLE messages ADD COLUMN feedback TEXT;
ALTER TABLE messages ADD COLUMN is_pinned INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_messages_conv_ts
  ON messages(conversation_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_messages_parent
  ON messages(parent_id);
```

### 3.3 tool_calls 新表

```sql
CREATE TABLE IF NOT EXISTS tool_calls (
  id TEXT PRIMARY KEY,
  message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  arguments TEXT NOT NULL,
  result TEXT,
  status TEXT NOT NULL DEFAULT 'running',
  started_at INTEGER NOT NULL,
  finished_at INTEGER,
  is_error INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_tool_calls_message
  ON tool_calls(message_id);
```

### 3.4 conversation_labels 关联表（可选，标签多对多）

```sql
CREATE TABLE IF NOT EXISTS conversation_labels (
  conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  label TEXT NOT NULL,
  PRIMARY KEY (conversation_id, label)
);
```

### 3.5 Schema Migration

`db.rs` 的 `run_migrations` 增加 v2 迁移：

```rust
if current_version < 2 {
    info!("Running migration v2: I3 chat history management");
    conn.execute_batch(
        "ALTER TABLE conversations ADD COLUMN is_pinned INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE conversations ADD COLUMN is_starred INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE conversations ADD COLUMN color_mark TEXT;
         ALTER TABLE conversations ADD COLUMN labels TEXT NOT NULL DEFAULT '[]';
         ALTER TABLE conversations ADD COLUMN draft_content TEXT;
         ALTER TABLE conversations ADD COLUMN draft_updated_at INTEGER;
         ALTER TABLE conversations ADD COLUMN message_count INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE conversations ADD COLUMN total_tokens INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE conversations ADD COLUMN generation INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE conversations ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE messages ADD COLUMN parent_id TEXT;
         ALTER TABLE messages ADD COLUMN source TEXT;
         ALTER TABLE messages ADD COLUMN feedback TEXT;
         ALTER TABLE messages ADD COLUMN is_pinned INTEGER NOT NULL DEFAULT 0;
         CREATE INDEX IF NOT EXISTS idx_messages_conv_ts ON messages(conversation_id, timestamp);
         CREATE INDEX IF NOT EXISTS idx_messages_parent ON messages(parent_id);
         CREATE TABLE IF NOT EXISTS tool_calls (
           id TEXT PRIMARY KEY,
           message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
           name TEXT NOT NULL,
           arguments TEXT NOT NULL,
           result TEXT,
           status TEXT NOT NULL DEFAULT 'running',
           started_at INTEGER NOT NULL,
           finished_at INTEGER,
           is_error INTEGER NOT NULL DEFAULT 0
         );
         CREATE INDEX IF NOT EXISTS idx_tool_calls_message ON tool_calls(message_id);"
    ).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('schema_version', '2')",
        [],
    ).map_err(|e| e.to_string())?;
}
```

## 4. 功能规格

### 4.1 消息分页加载

**问题**：`get_conversation` 一次返回全部消息，长对话（>500 条）导致前端卡顿和大量 DOM 节点。

**方案**：

| 参数 | 值 | 说明 |
|------|-----|------|
| `MESSAGES_PAGE_SIZE` | 50 | 每页消息数 |
| 加载方向 | 向上（更早消息） | 首次加载最近 N 条，向上滚动触发加载 |
| 加载触发 | 距顶部 < 100px | `onScroll` 检测 |
| 加载指示 | 顶部骨架屏 | 3 行灰色条 |

**Rust 端**：

```rust
pub fn get_messages_paginated(
    conn: &Connection,
    conversation_id: &str,
    limit: i64,
    before_timestamp: Option<i64>,
) -> Result<Vec<MessageRow>, String>
```

- `before_timestamp = None` → 最近 `limit` 条（`ORDER BY timestamp DESC LIMIT limit`，结果反转为 ASC）
- `before_timestamp = Some(ts)` → `timestamp < ts` 的最近 `limit` 条

**Tauri Command**：

```rust
#[command]
pub fn get_conversation_paginated(
    app: AppHandle,
    id: String,
    limit: Option<i64>,
    before_timestamp: Option<i64>,
) -> Result<serde_json::Value, String>
```

**前端 chatStore 扩展**：

```typescript
interface ChatState {
  hasMoreMessages: boolean
  isLoadingMoreMessages: boolean
  loadMoreMessages: () => Promise<void>
}
```

### 4.2 对话侧栏分组

**时间分组**：

```typescript
type ConversationGroup = {
  key: string
  label: string
  conversations: Conversation[]
}

function groupConversations(convs: Conversation[]): ConversationGroup[] {
  const now = Date.now()
  const groups: ConversationGroup[] = []

  const pinned = convs.filter(c => c.is_pinned)
  if (pinned.length > 0) groups.push({ key: 'pinned', label: '置顶', conversations: pinned })

  const unpinned = convs.filter(c => !c.is_pinned)
  const today = unpinned.filter(c => now - c.updated_at < 86400000)
  const yesterday = unpinned.filter(c => now - c.updated_at >= 86400000 && now - c.updated_at < 172800000)
  const week = unpinned.filter(c => now - c.updated_at >= 172800000 && now - c.updated_at < 604800000)
  const month = unpinned.filter(c => now - c.updated_at >= 604800000 && now - c.updated_at < 2592000000)
  const older = unpinned.filter(c => now - c.updated_at >= 2592000000)

  if (today.length) groups.push({ key: 'today', label: '今天', conversations: today })
  if (yesterday.length) groups.push({ key: 'yesterday', label: '昨天', conversations: yesterday })
  if (week.length) groups.push({ key: 'week', label: '近 7 天', conversations: week })
  if (month.length) groups.push({ key: 'month', label: '近 30 天', conversations: month })
  if (older.length) groups.push({ key: 'older', label: '更早', conversations: older })

  return groups
}
```

**Provider 过滤**：侧栏顶部增加 Provider 筛选标签栏（`Tabs` 组件），"全部" + 各 Provider 名称。

**收藏筛选**：侧栏顶部增加 ⭐ 图标按钮，切换仅显示 `is_starred = true` 的对话。

### 4.3 对话高级操作

#### 置顶/取消置顶

```rust
#[command]
pub fn toggle_conversation_pin(app: AppHandle, id: String) -> Result<(), String>
```

- `is_pinned` 取反
- 置顶时 `sort_order = MAX(sort_order) + 1`（置顶组内排序）
- 取消置顶时 `sort_order = 0`

#### 收藏/取消收藏

```rust
#[command]
pub fn toggle_conversation_star(app: AppHandle, id: String) -> Result<(), String>
```

#### 草稿保存/恢复

```rust
#[command]
pub fn save_conversation_draft(app: AppHandle, id: String, content: String) -> Result<(), String>
```

- 切换对话前调用 `save_conversation_draft` 保存当前输入框内容
- 切换到新对话后从 `draft_content` 恢复输入框

#### 对话复制

```rust
#[command]
pub fn duplicate_conversation(app: AppHandle, id: String) -> Result<String, String>
```

- 复制对话元数据 + 全部消息，新 ID，标题加 " (副本)"

#### 对话导出

```rust
#[command]
pub fn export_conversation(app: AppHandle, id: String, format: String) -> Result<String, String>
```

- `format`：`"json"` / `"markdown"` / `"text"`
- 返回导出内容字符串，前端触发下载

#### 对话导入

```rust
#[command]
pub fn import_conversation(app: AppHandle, data: String, format: String) -> Result<String, String>
```

#### 批量删除

```rust
#[command]
pub fn batch_delete_conversations(app: AppHandle, ids: Vec<String>) -> Result<(), String>
```

#### 清空所有对话

```rust
#[command]
pub fn purge_conversations(app: AppHandle) -> Result<(), String>
```

#### 全文搜索

```rust
#[command]
pub fn search_conversations(app: AppHandle, query: String) -> Result<serde_json::Value, String>
```

- SQL：`SELECT DISTINCT c.* FROM conversations c JOIN messages m ON m.conversation_id = c.id WHERE c.title LIKE ?1 OR m.content LIKE ?1 ORDER BY c.updated_at DESC`
- 参数：`%query%`

### 4.4 消息编辑与重新生成

#### 消息编辑

- 用户消息：点击编辑图标 → 内容变为可编辑 `TextArea` → 修改后按 Enter 提交
- 提交后：删除该消息之后的所有消息 → 重新发送编辑后的内容
- 若存在分支：编辑后创建新分支（保留原分支）

#### 消息重新生成

- 助手消息：hover 显示"重新生成"图标 → 点击后删除该助手消息 → 以相同上下文重新请求
- 重新生成产生新分支（原助手消息保留为分支 1，新回复为分支 2）

#### 分支切换

- 存在分支的消息下方显示分支指示器：`◀ 1/3 ▶`
- ◀ / ▶ 切换分支，切换时替换当前 `children_ids[idx]` 对应的消息子树

**Tauri Commands**：

```rust
#[command]
pub fn edit_message(app: AppHandle, message_id: String, new_content: String) -> Result<(), String>

#[command]
pub fn regenerate_message(app: AppHandle, conversation_id: String, message_id: String) -> Result<(), String>

#[command]
pub fn delete_message_persist(app: AppHandle, message_id: String) -> Result<(), String>
```

### 4.5 消息删除持久化

当前 `deleteMessage` 仅前端 `filter`，未调用 Rust 后端。修复：

```typescript
deleteMessage: async (id: string) => {
  const state = get()
  set({ messages: state.messages.filter(m => m.id !== id) })
  try {
    await invoke("delete_message_persist", { messageId: id })
  } catch (e) {
    console.error("Failed to delete message:", e)
  }
}
```

### 4.6 Tool Call 状态追踪

**流式事件扩展**：

```typescript
interface ChatDeltaPayload {
  conversation_id: string
  message_id: string
  delta_text?: string
  delta_reasoning?: string
  delta_tool_call_id?: string
  delta_tool_call_name?: string
  delta_tool_call_arguments?: string
  tool_call_status?: ToolCallStatus
  tool_call_result?: string
  tool_call_started_at?: number
  tool_call_finished_at?: number
  finish_reason?: string
  usage?: TokenUsage
  error?: string
}
```

**chatStore 处理**：

- `delta_tool_call_id` + `tool_call_status = 'running'` → 创建新 `ToolCallEntry`
- `tool_call_status` 更新 → 更新对应 entry 的 status/result/finished_at
- `tool_call_result` → 更新对应 entry 的 result

**UI 渲染**：

- `running`：旋转加载图标 + 工具名称
- `success`：✓ + 工具名称 + 耗时，结果可折叠
- `error`：✗ + 错误信息
- `awaiting_user`：⚠ + 审批按钮
- `cancelled`：⊘ 灰色

### 4.7 上下文窗口监控

**chatStore 扩展**：

```typescript
interface ChatState {
  contextUsage: {
    used_tokens: number
    max_tokens: number
    percentage: number
  } | null
}
```

- 每次流式完成（`finishStream`）后，从 `usage` 更新 `contextUsage`
- UI 在标题栏或输入区上方显示进度条：`████████░░ 8192/128000 (6.4%)`
- `percentage > 80%` 时黄色警告，`> 95%` 时红色警告

### 4.8 Persist Queue 串行化

当前所有 `invoke()` 无串行化，并发写入可能导致 SQLite 数据竞争。

**实现**（参考 airi `enqueuePersist`）：

```typescript
let persistQueue: Promise<void> = Promise.resolve()

function enqueuePersist<T>(task: () => Promise<T>): Promise<T> {
  const next = persistQueue.then(task)
  persistQueue = next.then(
    () => undefined,
    (err) => console.warn('[persist] task failed:', err)
  )
  return next
}
```

**chatStore 中所有写操作改用 `enqueuePersist`**：

```typescript
renameConversation: async (id, title) => {
  await enqueuePersist(() => invoke("rename_conversation", { id, title }))
  await get().loadConversations()
}
```

### 4.9 自动重试

```typescript
interface RetryConfig {
  maxAttempts: number
  baseDelayMs: number
  maxDelayMs: number
}

const DEFAULT_RETRY_CONFIG: RetryConfig = {
  maxAttempts: 3,
  baseDelayMs: 1000,
  maxDelayMs: 30000,
}

async function retryWithBackoff(
  fn: () => Promise<void>,
  config: RetryConfig,
  isRetryable: (error: ChatError) => boolean
): Promise<void>
```

- 可重试错误：`NetworkError` / `Timeout` / `RateLimited` / `ServerError` / `StreamError`
- 不可重试错误：`ApiKeyMissing` / `AuthFailed` / `ModelNotFound` / `ContextOverflow` / `Cancelled`
- 退避策略：`delay = min(baseDelayMs * 2^attempt + jitter, maxDelayMs)`
- UI 显示重试状态："重试中 (2/3)..."

### 4.10 消息渲染增强

#### 代码块语法高亮

- 集成 `shiki`（与 airi 一致），支持 100+ 语言
- 代码块右上角：语言标签 + 复制按钮 + 行号切换

#### 数学公式

- 集成 `KaTeX`（比 MathJax 轻量 10x+）
- `remark-math` + `rehype-katex` 插件
- 行内 `$...$`，块级 `$$...$$`

#### Thinking/Reasoning 折叠

- reasoning 内容默认折叠，显示"💭 思考了 X 秒"
- 点击展开，展开时平滑动画
- 折叠状态持久化到 `localStorage`

#### 消息操作栏

- hover 时消息右上角显示操作图标栏：
  - 📋 复制（纯文本 / Markdown 源码）
  - ✏️ 编辑（仅用户消息）
  - 🔄 重新生成（仅助手消息）
  - 👍/👎 反馈（仅助手消息）
  - 📌 固定到上下文
  - 🗑️ 删除

#### 消息反馈

- 点赞/点踩存入 `messages.feedback`
- 用于后续模型评估和 RLHF 数据收集

### 4.11 虚拟滚动

当消息数 > `VIRTUAL_SCROLL_THRESHOLD`（默认 100）时启用虚拟列表。

| 参数 | 值 | 说明 |
|------|-----|------|
| 库 | `react-virtuoso` | 成熟的 React 虚拟列表，支持分组/跟随/反向无限加载 |
| `VIRTUAL_SCROLL_THRESHOLD` | 100 | 消息数超过此值启用虚拟滚动 |
| `overscan` | 5 | 可视区域外额外渲染的行数 |

**react-virtuoso 集成**：

```tsx
<Virtuoso
  followOutput="smooth"
  firstItemIndex={firstItemIndex}
  initialTopMostItemIndex={messages.length - 1}
  itemContent={(index) => <MessageBubble message={messages[index]} />}
  startReached={loadMoreMessages}
  isScrolling={(isScrolling) => { if (!isScrolling) checkAutoScroll() }}
/>
```

### 4.12 对话侧栏增强

#### 右键菜单扩展

```
┌─ 置顶/取消置顶 ─────┐
│ 收藏/取消收藏        │
│ 颜色标记 → 子菜单    │
│ 标签管理 → 子菜单    │
│─────────────────────│
│ 重命名              │
│ 复制对话            │
│ 导出 → JSON/MD/TXT  │
│─────────────────────│
│ 删除                │
└─────────────────────┘
```

#### 多选模式

- 侧栏顶部增加 ☑ 按钮，点击进入多选模式
- 多选模式：每项前显示 Checkbox，底部显示操作栏（批量删除/批量导出/批量置顶/批量收藏）
- Escape 退出多选模式

#### 对话摘要预览

- 每项显示：标题 + 最后一条消息摘要（30 字符截断）+ 相对时间 + Provider 图标
- `is_pinned` 项显示 📌 图标
- `is_starred` 项显示 ⭐ 图标
- `color_mark` 项显示对应颜色圆点

#### 键盘导航

| 快捷键 | 操作 |
|--------|------|
| ↑ / ↓ | 上下移动选中 |
| Enter | 打开选中对话 |
| Delete | 删除选中对话 |
| Escape | 退出搜索/多选 |
| Ctrl+F | 聚焦搜索框 |

### 4.13 Generation Counter 渲染优化

```typescript
interface ChatState {
  generation: number
}

finishStream: () => {
  set(state => ({
    isStreaming: false,
    streamingContent: "",
    streamingMessageId: null,
    generation: state.generation + 1,
  }))
}
```

- `MessageList` 的 `useMemo` 依赖 `generation` 而非 `messages.length`
- `ConversationSidebar` 的 `useMemo` 依赖 `conversations` 的 `generation` 字段

### 4.14 辅助功能

#### TTS 语音朗读

- 助手消息操作栏增加 🔊 朗读按钮
- 使用 Web Speech API `speechSynthesis.speak()`
- 朗读中按钮变为 ⏹ 停止
- 支持 `speechSynthesis.getVoices()` 选择语音

#### Token 计数器

- 输入框右下角显示预估 token 数
- 使用 `tiktoken` 的 WASM 版本（`js-tiktoken`）或简单估算（字符数 / 4）
- 格式：`~128 tokens`

#### 流式速率指示

- 流式中在助手消息底部显示 `XX tokens/s`
- 计算：`completion_tokens / (now - streamStartTime) * 1000`

## 5. Rust 后端规格

### 5.1 新增依赖

**assistant-storage/Cargo.toml**：
- 无新增（`serde_json` 已有，用于 labels/content_block JSON 序列化）

**前端**：
- `npm install react-virtuoso shiki katex remark-math rehype-katex`

### 5.2 assistant-storage crate 扩展

**conversations.rs 新增函数**：

| 函数 | 签名 | 说明 |
|------|------|------|
| `get_messages_paginated` | `(conn, conversation_id, limit, before_timestamp) → Result<Vec<MessageRow>>` | 分页查询消息 |
| `toggle_pin` | `(conn, id) → Result<()>` | 切换置顶 |
| `toggle_star` | `(conn, id) → Result<()>` | 切换收藏 |
| `save_draft` | `(conn, id, content) → Result<()>` | 保存草稿 |
| `duplicate_conversation` | `(conn, id) → Result<String>` | 复制对话 |
| `search_conversations` | `(conn, query) → Result<Vec<ConversationRow>>` | 全文搜索 |
| `batch_delete` | `(conn, ids) → Result<()>` | 批量删除 |
| `purge_all` | `(conn) → Result<()>` | 清空所有对话 |
| `increment_generation` | `(conn, id) → Result<()>` | 递增 generation |
| `update_conversation_stats` | `(conn, id) → Result<()>` | 更新 message_count/total_tokens 冗余缓存 |

**messages.rs 新增函数**：

| 函数 | 签名 | 说明 |
|------|------|------|
| `delete_message` | `(conn, id) → Result<()>` | 删除单条消息（含级联 tool_calls） |
| `edit_message` | `(conn, id, new_content) → Result<()>` | 编辑消息内容 |
| `update_feedback` | `(conn, id, feedback) → Result<()>` | 更新消息反馈 |
| `get_children` | `(conn, parent_id) → Result<Vec<MessageRow>>` | 获取分支子消息 |
| `export_conversation` | `(conn, id, format) → Result<String>` | 导出对话 |
| `import_conversation` | `(conn, data, format) → Result<String>` | 导入对话 |

**tool_calls.rs 新增模块**：

| 函数 | 签名 | 说明 |
|------|------|------|
| `insert_tool_call` | `(conn, entry) → Result<()>` | 插入工具调用 |
| `update_tool_call_status` | `(conn, id, status, result, finished_at) → Result<()>` | 更新状态 |
| `get_tool_calls` | `(conn, message_id) → Result<Vec<ToolCallRow>>` | 获取消息的工具调用 |

### 5.3 Tauri Commands 新增

| command | 签名 | 说明 |
|---------|------|------|
| `get_conversation_paginated` | `(id, limit?, before_timestamp?) → Result<ConversationDetail>` | 分页获取对话 |
| `toggle_conversation_pin` | `(id) → Result<()>` | 切换置顶 |
| `toggle_conversation_star` | `(id) → Result<()>` | 切换收藏 |
| `save_conversation_draft` | `(id, content) → Result<()>` | 保存草稿 |
| `duplicate_conversation` | `(id) → Result<String>` | 复制对话 |
| `export_conversation` | `(id, format) → Result<String>` | 导出对话 |
| `import_conversation` | `(data, format) → Result<String>` | 导入对话 |
| `batch_delete_conversations` | `(ids) → Result<()>` | 批量删除 |
| `purge_conversations` | `() → Result<()>` | 清空所有 |
| `search_conversations` | `(query) → Result<Vec<ConversationRow>>` | 全文搜索 |
| `edit_message` | `(message_id, new_content) → Result<()>` | 编辑消息 |
| `regenerate_message` | `(conversation_id, message_id) → Result<()>` | 重新生成 |
| `delete_message_persist` | `(message_id) → Result<()>` | 持久化删除消息 |
| `update_message_feedback` | `(message_id, feedback) → Result<()>` | 更新反馈 |
| `set_conversation_color_mark` | `(id, color) → Result<()>` | 设置颜色标记 |
| `set_conversation_labels` | `(id, labels) → Result<()>` | 设置标签 |

## 6. 前端规格

### 6.1 chatStore 扩展

```typescript
interface ChatState {
  conversations: Conversation[]
  currentConversationId: ConversationId | null
  messages: Message[]
  isStreaming: boolean
  streamingContent: string
  streamingMessageId: MessageId | null
  sidebarOpen: boolean
  hasMoreMessages: boolean
  isLoadingMoreMessages: boolean
  contextUsage: { used_tokens: number; max_tokens: number; percentage: number } | null
  generation: number
  activeBranchMap: Record<string, number>
  searchQuery: string
  searchResults: Conversation[] | null
  filterProviderId: ProviderId | null
  filterStarred: boolean
  multiSelectMode: boolean
  selectedConversationIds: Set<ConversationId>

  loadConversations: () => Promise<void>
  createConversation: (providerId: string, modelId: string) => Promise<string>
  ensureCurrentConversation: () => Promise<void>
  switchConversation: (id: string) => Promise<void>
  deleteConversation: (id: string) => Promise<void>
  renameConversation: (id: string, title: string) => Promise<void>
  updateModel: (id: string, modelId: string) => Promise<void>
  togglePin: (id: string) => Promise<void>
  toggleStar: (id: string) => Promise<void>
  saveDraft: (content: string) => Promise<void>
  duplicateConversation: (id: string) => Promise<void>
  exportConversation: (id: string, format: string) => Promise<void>
  searchConversations: (query: string) => Promise<void>

  sendMessage: (content: string) => Promise<void>
  cancelStream: () => Promise<void>
  editMessage: (messageId: string, newContent: string) => Promise<void>
  regenerateMessage: (messageId: string) => Promise<void>
  deleteMessage: (id: string) => Promise<void>
  updateFeedback: (messageId: string, feedback: 'positive' | 'negative' | null) => Promise<void>
  switchBranch: (parentId: string, index: number) => void

  loadMoreMessages: () => Promise<void>
  appendDelta: (delta: string) => void
  finishStream: (usage?: TokenUsage) => void
  errorStream: (error: ChatError) => void
  toggleSidebar: () => void
  setFilterProvider: (providerId: string | null) => void
  setFilterStarred: (starred: boolean) => void
  toggleMultiSelect: () => void
  toggleSelectConversation: (id: string) => void
}
```

### 6.2 新增/修改组件

| 组件 | 路径 | 变更 | 职责 |
|------|------|------|------|
| ConversationSidebar | `src/chat/ConversationSidebar.tsx` | 重构 | 时间分组 + Provider 过滤 + 收藏筛选 + 多选 + 右键菜单扩展 |
| MessageBubble | `src/chat/MessageBubble.tsx` | 重构 | 操作栏 + 分支切换 + 反馈 + 折叠 reasoning + Tool Call 状态 |
| MessageList | `src/chat/MessageList.tsx` | 重构 | 虚拟滚动 + 分页加载 + 骨架屏 |
| ChatView | `src/chat/ChatView.tsx` | 修改 | 上下文进度条 + 草稿保存 |
| ContextBar | `src/chat/ContextBar.tsx` | 新增 | 上下文窗口使用量进度条 |
| BranchSwitcher | `src/chat/BranchSwitcher.tsx` | 新增 | 消息分支切换器 |
| ToolCallBubble | `src/chat/ToolCallBubble.tsx` | 新增 | Tool Call 状态渲染（五态） |
| ToolTimeline | `src/chat/ToolTimeline.tsx` | 新增 | 工具调用时间线视图 |
| ConversationGroup | `src/chat/ConversationGroup.tsx` | 新增 | 对话分组标题 + 可折叠 |
| MessageActionBar | `src/chat/MessageActionBar.tsx` | 新增 | 消息操作栏（复制/编辑/重生成/反馈/固定/删除） |
| SearchBar | `src/chat/SearchBar.tsx` | 新增 | 全文搜索（标题 + 消息内容） |

### 6.3 常量新增

```typescript
export const MESSAGES_PAGE_SIZE = 50
export const VIRTUAL_SCROLL_THRESHOLD = 100
export const VIRTUAL_SCROLL_OVERSCAN = 5
export const CONTEXT_WARN_THRESHOLD = 0.8
export const CONTEXT_DANGER_THRESHOLD = 0.95
export const RETRY_MAX_ATTEMPTS = 3
export const RETRY_BASE_DELAY_MS = 1000
export const RETRY_MAX_DELAY_MS = 30000
export const CONVERSATION_SUMMARY_LENGTH = 30
export const SCROLL_LOAD_THRESHOLD_PX = 100
```

## 7. 验收标准

| # | 标准 | 验证方法 |
|---|------|---------|
| 1 | 消息分页加载：500 条消息对话，首次加载 50 条，向上滚动加载更多，无卡顿 | 手动测试 |
| 2 | 虚拟滚动：200 条消息对话，DOM 节点数 < 60 | Chrome DevTools Elements 计数 |
| 3 | 时间分组：侧栏按"今天/昨天/近 7 天/近 30 天/更早"分组显示 | 手动测试 |
| 4 | 置顶/收藏：置顶对话在侧栏最顶部，收藏筛选仅显示收藏对话 | 手动测试 |
| 5 | 草稿保存：输入框输入内容 → 切换对话 → 切回 → 内容恢复 | 手动测试 |
| 6 | 消息编辑：编辑用户消息 → 后续消息删除 → 重新发送 | 手动测试 |
| 7 | 消息重新生成：点击重新生成 → 新回复产生 → 分支指示器显示 | 手动测试 |
| 8 | 分支切换：编辑产生分支 → ◀▶ 切换 → 消息子树替换 | 手动测试 |
| 9 | 消息删除持久化：删除消息 → 刷新页面 → 消息不再出现 | 手动测试 |
| 10 | Tool Call 五态：工具调用显示 running/success/error/awaiting_user/cancelled | Deep 模式测试 |
| 11 | 上下文监控：进度条显示使用量，>80% 黄色警告 | 长对话测试 |
| 12 | Persist Queue：快速连续操作（重命名+删除+发送）无数据丢失 | 压力测试 |
| 13 | 自动重试：模拟网络断开 → 恢复 → 消息自动重试成功 | DevTools Network Offline |
| 14 | 全文搜索：搜索消息内容 → 匹配对话显示 | 手动测试 |
| 15 | 对话导出/导入：导出 JSON → 删除对话 → 导入 → 对话恢复 | 手动测试 |
| 16 | 代码高亮：代码块语法高亮 + 复制按钮 | 手动测试 |
| 17 | 数学公式：`$$E=mc^2$$` 正确渲染 | 手动测试 |
| 18 | Reasoning 折叠：思考内容默认折叠，点击展开 | 手动测试 |
| 19 | 消息反馈：点赞/点踩存入 DB | 手动测试 + DB 查询 |
| 20 | Branded Type：`MessageId` / `ConversationId` 类型不匹配时编译报错 | TypeScript 编译 |
# I2 — Orb AI 对话 规格说明

## 1. 交付目标

Orb 面板内可用的多模型 AI 对话，流式输出，历史可查看，对话持久化。

## 2. 功能规格

### 2.1 Provider 配置 UI

在主窗口设置页内管理 AI Provider（Provider = 一个 API 服务商 + 凭证 + 模型列表）。

| 字段 | 类型 | 说明 |
|------|------|------|
| id | string (UUID) | 唯一标识 |
| name | string | 显示名称（如 "OpenAI"、"DeepSeek"） |
| type | ProviderKind | 枚举：OpenAI / Anthropic / Gemini / Custom |
| base_url | string | API 端点（OpenAI 默认 `https://api.openai.com/v1`） |
| api_key | string | 明文 API Key（仅 UI 输入时可见，存储时加密） |
| models | ModelConfig[] | 可用模型列表 |
| default_model | string | 默认模型 ID |

**UI 规格**：
- 路由：`/settings/providers`
- 列表页：Provider 卡片列表 + 添加按钮
- 编辑页：表单（名称 + 类型 + Base URL + API Key + 模型列表 + 默认模型）
- API Key 输入：`Input.Password`，blur 后显示 `••••••••`
- 模型列表：点击"获取模型列表"按钮 → Rust 端调用 `GET /models` → 填充下拉
- 保存时：API Key 明文 → Rust 端 `encrypt_api_key` → 存入 SQLite `providers.api_key_encrypted`
- 删除：确认弹窗 → 删除 Provider + 级联删除其下所有 conversations

### 2.2 API Key 加密存储

| 参数 | 值 | 说明 |
|------|-----|------|
| 算法 | AES-256-GCM | 对称加密，AEAD 保证密文完整性 |
| 密钥派生 | SHA-256(machine_id) | machine_id 来自系统唯一标识 |
| machine_id 来源 | `machine-uid` crate | 跨平台机器唯一 ID |
| 存储格式 | Base64(IV ‖ ciphertext ‖ tag) | SQLite TEXT 字段 |
| 解密时机 | 发起 API 请求时 | 前端永远不接触明文 Key |

**Rust 端实现**：
- `encrypt_api_key(key: &str, machine_id: &[u8]) → Result<Vec<u8>, String>`
- `decrypt_api_key(encrypted: &[u8], machine_id: &[u8]) → Result<String, String>`
- `get_machine_id() → Result<Vec<u8>, String>` — SHA-256(machine_uid)
- `encrypt_api_key_b64(key: &str, machine_id: &[u8]) → Result<String, String>` — 加密后 Base64 编码，直接存入 SQLite TEXT 字段
- `decrypt_api_key_b64(encrypted_b64: &str, machine_id: &[u8]) → Result<String, String>` — Base64 解码后解密，从 SQLite TEXT 字段读取
- 依赖：`aes-gcm` crate + `machine-uid` crate + `sha2` crate + `base64` crate + `rand` crate（随机 nonce 生成）

### 2.3 Rust 端 Provider 适配器

**BaseProviderAdapter trait**（已有骨架，需补全）：

```rust
pub trait BaseProviderAdapter: Send + Sync {
    fn kind(&self) -> &ProviderKind;
    fn config(&self) -> &ProviderConfig;
    fn chat_completion_stream(
        &self,
        messages: &[ChatMessage],
        model: &str,
        options: &ChatOptions,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatDelta, ProviderError>> + Send>>, ProviderError>;
    fn list_models(&self) -> Result<Vec<ModelConfig>, ProviderError>;
}
```

**ChatMessage 扩展**：

```rust
pub struct ChatMessage {
    pub role: MessageRole,  // System / User / Assistant
    pub content: String,
}

pub struct ChatOptions {
    pub temperature: f32,
    pub max_tokens: u32,
    pub stream: bool,
}

pub struct ChatDelta {
    pub delta_text: Option<String>,
    pub finish_reason: Option<String>,
    pub usage: Option<TokenUsage>,
}

pub struct ModelConfig {
    pub id: String,
    pub name: String,
    pub owned_by: String,
}

pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}
```

**OpenAIAdapter 实现**（兼容 DeepSeek / Moonshot / 等所有 OpenAI 格式 API）：
- `chat_completion_stream`：POST `{base_url}/chat/completions`，`stream: true`
- 响应解析：SSE `data: {json}\n\n`，每行 `choices[0].delta.content` → `ChatDelta`
- `list_models`：GET `{base_url}/models`，解析 `data[].id`
- HTTP 客户端：`reqwest` crate（stream + JSON）
- SSE 解析：`eventsource-stream` crate 或手动按行解析

**ProviderError 错误类型**（`thiserror` crate 派生）：5 种变体 — ApiKeyMissing（Provider 未配置 API Key）、RequestFailed（网络/HTTP 错误）、ParseError（JSON/SSE 解析错误）、RateLimited（429 状态码）、ModelNotFound（模型不存在）。

**前端 ChatError 错误体系**（`src/chat/chatErrors.ts`）：扩展 Rust 端 ProviderError 为 13 种前端错误码，覆盖更多用户可感知的错误场景：

| ChatErrorCode | 说明 | retryable |
|---------------|------|-----------|
| ApiKeyMissing | API Key 未配置 | ❌ |
| AuthFailed | 认证失败 | ❌ |
| RateLimited | 请求限流 | ✅ |
| ModelNotFound | 模型不可用 | ❌ |
| ContextOverflow | 上下文过长 | ❌ |
| NetworkError | 网络错误 | ✅ |
| Timeout | 请求超时 | ✅ |
| ServerError | 服务端错误 | ✅ |
| RequestRejected | 请求被拒绝 | ❌ |
| ParseError | 解析错误 | ❌ |
| Cancelled | 已取消 | ❌ |
| StreamError | 流式错误 | ✅ |
| InternalError | 内部错误 | ❌ |

**ProviderKind 枚举**：OpenAI / Anthropic / Gemini / Custom{base_url}，Custom 变体携带自定义 base_url。

**ProviderConfig 结构体**：kind + api_key_encrypted + base_url + default_model + max_tokens + temperature，封装适配器所需的全部配置。

**create_provider_adapter 工厂函数**：接收 ProviderConfig，根据 ProviderKind 创建对应适配器实例（I2 仅实现 OpenAI，后续扩展 Anthropic/Gemini）。

**依赖**：`reqwest`（+ stream feature）、`tokio`、`futures`、`tokio-stream`、`async-stream`、`thiserror`

### 2.4 前端 Provider 适配层

前端不直接发 HTTP 请求，所有 API 通信通过 Rust 端 Tauri command 中转。

**Tauri Commands**：

| command | 签名 | 说明 |
|---------|------|------|
| `list_providers` | `() → Result<Vec<ProviderRow>>` | 列出所有 Provider（api_key 返回脱敏 `sk-***...`） |
| `save_provider` | `(provider: ProviderInput) → Result<()>` | 新增/更新 Provider（api_key 明文传入，Rust 端加密） |
| `delete_provider` | `(id: String) → Result<()>` | 删除 Provider + 级联删除 conversations |
| `list_models` | `(provider_id: String) → Result<Vec<ModelConfig>>` | 调用 Provider API 获取模型列表（需已保存的 Provider） |
| `preview_models` | `(provider_type: String, base_url: String, api_key: String) → Result<Vec<ModelConfig>>` | 用明文 Key 预览模型列表（保存前预览，不依赖已保存 Provider） |
| `send_chat_message` | `(conversation_id: String, content: String) → Result<()>` | 发送用户消息，开始流式响应 |
| `cancel_chat` | `(conversation_id: String) → Result<()>` | 取消当前流式响应 |
| `list_conversations` | `(provider_id: Option<String>) → Result<Vec<ConversationRow>>` | 列出对话（可按 provider 过滤） |
| `get_conversation` | `(id: String) → Result<ConversationDetail>` | 获取对话详情（含所有 messages） |
| `create_conversation` | `(provider_id: String, model_id: String) → Result<String>` | 新建对话，返回 id |
| `delete_conversation` | `(id: String) → Result<()>` | 删除对话 |
| `rename_conversation` | `(id: String, title: String, title_source?: String) → Result<()>` | 重命名对话（含标题来源追踪） |
| `list_prompt_templates` | `() → Result<Vec<PromptTemplate>>` | 列出 Prompt 模板 |
| `save_prompt_template` | `(template: PromptTemplateInput) → Result<()>` | 新增/更新模板 |

> **I2 迭代后期前置实现**（属 I2-history 范围，已提前实现）：

| command | 签名 | 说明 |
|---------|------|------|
| `get_conversation_paginated` | `(id, limit?, before_timestamp?) → Result<ConversationDetail>` | 分页获取对话 |
| `export_conversation` | `(id, format) → Result<String>` | 导出对话（format: json/markdown/text） |
| `search_conversations` | `(query) → Result<Vec<ConversationRow>>` | 消息内容搜索（标题搜索由前端本地过滤） |

**流式事件**：Rust 端通过 `app.emit("chat-delta", payload)` 推送增量到前端：

```typescript
interface ChatDeltaPayload {
  conversation_id: string;
  message_id: string;
  delta_text?: string;
  finish_reason?: string;
  usage?: TokenUsage;
  error?: string;
}
```

### 2.5 Orb 面板对话 UI

OrbPanel 从 I1 占位替换为完整对话界面。

**布局**（444×624 逻辑像素内，减去 chrome inset 12×2 = 420×600 可用区域）：

```
┌─────────────────────────────┐
│ 标题栏 (40px)               │  对话标题 + 模型选择 + 新建对话按钮
├─────────────────────────────┤
│                             │
│ 消息列表 (flex: 1)          │  滚动区域，消息气泡
│                             │
├─────────────────────────────┤
│ 输入区 (auto)               │  文本框 + 发送按钮 + 模型切换
└─────────────────────────────┘
```

**标题栏**：
- 左：对话标题（可点击编辑）+ 返回按钮（显示对话列表侧栏）
- 右：模型选择下拉（`Select` 组件）+ 新建对话按钮

**消息列表**：
- 用户消息：右对齐，蓝色气泡
- 助手消息：左对齐，灰色气泡，支持 Markdown 渲染
- 流式输出中：助手消息末尾显示闪烁光标 `▍`
- 空状态：空白（内置模板快捷入口已移除，见 DEVLOG 2026-10-05）
- 自动滚动：新消息时滚到底部，用户上滚时停止自动滚动（"回到底部"按钮出现）
- 消息渲染：`react-markdown` + `remark-gfm`（GFM 表格/删除线/任务列表）

**输入区**：
- `TextArea` 自适应高度（1~4 行）
- Enter 发送，Shift+Enter 换行
- 发送按钮（Enter 键图标，流式中变为 Stop 按钮）
- 无 Provider 配置时：显示"请先配置 AI Provider"提示 + 跳转设置按钮

**对话列表侧栏**（点击标题栏返回按钮展开）：
- 左侧 200px 宽度滑出
- 对话列表（按 updated_at 降序）
- 每项：标题 + 时间 + 右键菜单（重命名/删除）
- 顶部：搜索框
- 底部：新建对话按钮

### 2.6 流式输出实时渲染

**chatDeltaCoalescer**（前端，参考 OpenHuman）：

| 参数 | 值 | 说明 |
|------|-----|------|
| 调度策略 | `requestAnimationFrame` + `setTimeout(50ms)` 双保险 | rAF 在窗口可见时用，隐藏窗口 rAF 暂停但 timer 兜底 |
| 合并条件 | 相邻且 `(conversation_id, message_id)` 相同 | 仅合并同一条助手消息的连续 delta |
| flush 触发 | 非 delta 事件（如 finish/error）前必须 flush | 保证文本在 finish 之前完整渲染 |

**实现**：
- `createChatDeltaCoalescer(deliver, schedule)` → `{ push, flush, dispose }`
- `push(channel, event)`：与队列末尾同 key → 合并 delta；否则追加 → 调度 flush
- `flush()`：清空队列，逐条 `deliver(channel, event)`
- `dispose()`：立即 flush + 取消调度

**chatStore 集成**：
- 监听 `listen("chat-delta")` → coalescer.push
- coalescer deliver → 追加到当前助手消息的 `streamingContent`
- `chat-delta` 中 `finish_reason` 非 null → flush + 标记消息完成 + 更新 usage
- `chat-delta` 中 `error` 非 null → flush + 标记消息错误

### 2.7 对话管理

| 操作 | 触发 | 行为 |
|------|------|------|
| 新建对话 | 右键菜单"新建对话" / 标题栏"+"按钮 / 快捷键 | 选择 Provider + Model → 创建空对话 → 切换到新对话 |
| 列表 | 标题栏返回按钮 | 展开对话列表侧栏 |
| 删除 | 对话列表右键菜单 | 确认弹窗 → 删除对话 + 消息 → 切换到下一个对话 |
| 重命名 | 对话列表右键菜单 / 标题栏点击 | Inline 编辑标题 |
| 切换对话 | 对话列表点击 | 加载对话消息 → 滚到底部 |
| 自动标题 | 首条助手消息完成后 | 取前 20 字符作为标题（若用户未手动设置） |

### 2.8 对话持久化

**SQLite CRUD**（assistant-storage crate 扩展）：

| 函数 | 签名 | 说明 |
|------|------|------|
| `insert_provider` | `(ProviderRow) → Result<()>` | 插入 Provider |
| `get_providers` | `() → Result<Vec<ProviderRow>>` | 查询所有 Provider |
| `update_provider` | `(ProviderRow) → Result<()>` | 更新 Provider |
| `delete_provider` | `(id: &str) → Result<()>` | 删除 Provider + 级联 |
| `insert_conversation` | `(ConversationRow) → Result<()>` | 插入对话 |
| `get_conversations` | `(provider_id: Option<&str>) → Result<Vec<ConversationRow>>` | 查询对话列表 |
| `get_conversation_detail` | `(id: &str) → Result<ConversationDetail>` | 查询对话 + 消息 |
| `update_conversation` | `(ConversationRow) → Result<()>` | 更新对话（标题/时间） |
| `delete_conversation` | `(id: &str) → Result<()>` | 删除对话 + 级联消息 |
| `insert_message` | `(MessageRow) → Result<()>` | 插入消息 |
| `get_messages` | `(conversation_id: &str) → Result<Vec<MessageRow>>` | 查询对话消息 |
| `update_message_content` | `(id: &str, content: &str, tokens: Option<i64>) → Result<()>` | 更新消息内容（流式完成后写回完整内容 + token 用量） |
| `insert_prompt_template` | `(PromptTemplateRow) → Result<()>` | 插入模板 |
| `get_prompt_templates` | `() → Result<Vec<PromptTemplateRow>>` | 查询模板 |
| `seed_builtin_templates` | `() → Result<()>` | 插入 5 个内置模板 |

**Connection 管理**：
- 使用 `tauri-plugin-sql` 管理的连接（I0 已集成），通过 `app.state::<SqlitePool>` 获取
- 或使用 `r2d2` 连接池（`Mutex<Connection>` 单连接即可，桌面应用并发低）
- 所有写操作在 Tauri command 中同步执行，读操作亦然

### 2.9 Prompt 模板库

5 个内置模板（`is_builtin = 1`，不可删除）：

| 模板 | category | content |
|------|----------|---------|
| 翻译助手 | text | `请将以下内容翻译为{{target_language}}：\n\n{{content}}` |
| 代码解释 | dev | `请解释以下代码的功能：\n\n\`\`\`{{language}}\n{{code}}\n\`\`\`` |
| 文本摘要 | text | `请用{{length}}字以内总结以下内容：\n\n{{content}}` |
| 续写 | text | `请根据以下内容继续写作：\n\n{{content}}` |
| 头脑风暴 | daily | `请针对以下主题给出{{count}}个创意想法：\n\n{{topic}}` |

**UI**：
- ~~空对话时显示模板卡片（图标 + 名称 + 点击填充输入框）~~ — **已移除**（见 DEVLOG 2026-10-05）
- ~~变量占位符 `{{variable}}` → 点击模板后弹出变量填写表单~~ — **已移除**
- 管理页：`/settings/templates`（I2 仅展示内置模板，自定义模板 I3 扩展）

## 3. Rust 后端规格

### 3.1 新增依赖

**assistant-providers/Cargo.toml**：
- `reqwest = { version = "0.12", features = ["stream", "json"] }`
- `tokio = { version = "1", features = ["full"] }`
- `futures = "0.3"`
- `aes-gcm = "0.10"`
- `sha2 = "0.10"`
- `base64 = "0.22"`
- `machine-uid = "0.2"`
- `tokio-stream = "0.1"` — Stream trait 支持（SSE 流式消费）
- `async-stream = "0.3"` — `stream!` 宏优雅实现 SSE 流
- `rand = "0.8"` — 随机 nonce 生成（AES-GCM）
- `thiserror = "2"` — ProviderError 错误类型派生

**assistant-storage/Cargo.toml**：
- `uuid = { version = "1", features = ["v4"] }`
- `chrono = "0.4"`

**根 Cargo.toml [workspace.dependencies]**：
- 同步添加上述依赖

### 3.2 assistant-providers crate 扩展

**crypto.rs**（完整实现）：
- `get_machine_id()` → `machine_uid::get()` → SHA-256 → 32 字节密钥
- `encrypt_api_key(key, machine_id)` → AES-256-GCM 加密 → `IV ‖ ciphertext ‖ tag`（原始字节）
- `decrypt_api_key(encrypted, machine_id)` → AES-256-GCM 解密 → 明文
- `encrypt_api_key_b64(key, machine_id)` → 加密后 Base64 编码 → `String`（直接存 SQLite TEXT）
- `decrypt_api_key_b64(encrypted_b64, machine_id)` → Base64 解码后解密 → 明文
- 随机 nonce：`rand::thread_rng().gen()` 生成 12 字节 nonce

**openai.rs**（完整实现）：
- `chat_completion_stream(messages, model, options)` → reqwest POST → SSE 解析 → `impl Stream<Item = Result<ChatDelta>>`
- `list_models()` → reqwest GET `/models` → JSON 解析 → `Vec<ModelConfig>`
- 错误处理：HTTP 状态码 / 超时 / JSON 解析 / 连接失败 → `ProviderError`

**base.rs**（扩展）：
- `ChatMessage.role` 改为 `MessageRole` 枚举
- 新增 `ChatOptions`、`ChatDelta`、`ModelConfig`、`TokenUsage` 结构体
- `BaseProviderAdapter::chat_completion_stream` 返回 `Stream` 而非 `Result<()>`
- `BaseProviderAdapter::list_models` 新增方法
- 新增 `ProviderError` 枚举（5 种变体：ApiKeyMissing / RequestFailed / ParseError / RateLimited / ModelNotFound）
- 新增 `ProviderKind` 枚举（OpenAI / Anthropic / Gemini / Custom{base_url}）
- 新增 `ProviderConfig` 结构体（kind + api_key_encrypted + base_url + default_model + max_tokens + temperature）
- 新增 `create_provider_adapter(config) → Box<dyn BaseProviderAdapter>` 工厂函数
- 新增 `provider_kind_from_str()` / `message_role_from_str()` 辅助转换函数

### 3.3 assistant-storage crate 扩展

**db.rs** → 拆分为多模块：
- `mod providers` — Provider CRUD
- `mod conversations` — Conversation + Message CRUD
- `mod templates` — PromptTemplate CRUD + seed

**Connection 管理**：
- `init_db` 返回 `Connection`（已有）
- 新增 `DbState(Mutex<Connection>)` 作为 Tauri managed state
- `setup.rs` 中 `app.manage(DbState(conn))`

### 3.4 Tauri Commands 新增

**cmd.rs** 新增 14 个 command（见 2.4 节表），核心逻辑：

- `preview_models`：
  - 接收明文 provider_type + base_url + api_key
  - 临时加密 api_key → 构造 ProviderConfig → 创建 adapter → `list_models()` → 返回
  - 用途：ProviderForm 保存前预览可用模型列表，无需先保存 Provider

- `send_chat_message`：
  1. 插入 user message 到 SQLite
  2. 构建历史消息列表（从 SQLite 读取）
  3. 获取 Provider → 解密 API Key → 构造 adapter
  4. `tokio::spawn` 异步调用 `chat_completion_stream`
  5. 流式消费：每个 delta → `app.emit("chat-delta", payload)`
  6. finish → 插入 assistant message 到 SQLite → emit finish delta

- `cancel_chat`：
  - 使用 `CancellationToken`（`tokio_util` crate）取消流式任务
  - `Arc<CancellationToken>` 存入 `app.state::<ChatTasks>`（`HashMap<String, CancellationToken>`）

### 3.5 流式任务管理

```rust
pub struct ChatTasks(Mutex<HashMap<String, CancellationToken>>);

impl ChatTasks {
    pub fn start(&self, conversation_id: String) -> CancellationToken { ... }
    pub fn cancel(&self, conversation_id: &str) -> Option<CancellationToken> { ... }
}
```

## 4. 前端规格

### 4.1 新增组件

| 组件 | 路径 | 职责 |
|------|------|------|
| ChatView | `src/chat/ChatView.tsx` | 对话主视图（消息列表 + 输入区 + 标题栏） |
| MessageList | `src/chat/MessageList.tsx` | 消息列表（滚动 + 自动滚底 + 回到底部按钮） |
| MessageBubble | `src/chat/MessageBubble.tsx` | 单条消息气泡（Markdown 渲染 + 流式光标） |
| MessageInput | `src/chat/MessageInput.tsx` | 输入区（TextArea + 发送/停止按钮） |
| ModelSelector | `src/chat/ModelSelector.tsx` | 模型选择下拉 |
| ProviderSelector | `src/chat/ProviderSelector.tsx` | Provider 选择（对话栏内嵌下拉） |
| ConversationSidebar | `src/chat/ConversationSidebar.tsx` | 对话列表侧栏 |
| ~~TemplateCards~~ | ~~`src/chat/TemplateCards.tsx`~~ | ~~Prompt 模板快捷入口卡片~~ **已移除** |
| HistoryManager | `src/chat/HistoryManager.tsx` | 消息历史管理页（I2-history 前置 UI 骨架） |
| ProviderManager | `src/settings/ProviderManager.tsx` | Provider 配置管理页 |
| AddProviderDialog | `src/settings/AddProviderDialog.tsx` | Provider 预设选择对话框 |
| ConnectDialog | `src/settings/ConnectDialog.tsx` | Provider 连接配置对话框 |
| ProviderSwatch | `src/settings/ProviderSwatch.tsx` | Provider 格式标签 |
| TemplateManager | `src/settings/TemplateManager.tsx` | Prompt 模板管理页 |

> **实现说明**：spec 原始设计为单一 `ProviderForm` 组件，实际拆分为 `AddProviderDialog`（预设选择）+ `ConnectDialog`（连接配置）+ `ProviderSwatch`（格式标签），交互更清晰。`ProviderSelector` 为对话栏内嵌的 Provider 切换组件，spec 未单独列出。`HistoryManager` 为 I2-history 前置 UI 骨架，已接入 `/history` 路由。

### 4.2 新增 Store

**chatStore** — `src/stores/chatStore.ts`：

```typescript
interface ChatState {
  conversations: Conversation[];
  currentConversationId: string | null;
  messages: Message[];
  isStreaming: boolean;
  streamingContent: string;
  streamingMessageId: string | null;
  sidebarOpen: boolean;

  loadConversations: () => Promise<void>;
  ensureCurrentConversation: () => Promise<void>;
  createConversation: (providerId: string, modelId: string) => Promise<string>;
  switchConversation: (id: string) => Promise<void>;
  deleteConversation: (id: string) => Promise<void>;
  renameConversation: (id: string, title: string) => Promise<void>;
  sendMessage: (content: string) => Promise<void>;
  cancelStream: () => Promise<void>;
  appendDelta: (delta: string) => void;
  finishStream: (usage?: TokenUsage) => void;
  errorStream: (error: string) => void;
  toggleSidebar: () => void;
}
```

- `streamingMessageId`：当前流式输出中助手消息的 ID，用于将 delta 精确追加到对应消息，避免多对话并发时错位。
- `ensureCurrentConversation()`：初始化时保证存在真实对话 — 若 currentConversationId 为空且有对话列表则切换到第一个；若列表也为空但有 Provider 则自动创建新对话并切换；若无 Provider 则保持空状态等待引导。避免用户首次打开面对空白界面。

**providerStore** — `src/stores/providerStore.ts`：

```typescript
interface ProviderState {
  providers: Provider[];
  loading: boolean;

  loadProviders: () => Promise<void>;
  saveProvider: (provider: ProviderInput) => Promise<void>;
  deleteProvider: (id: string) => Promise<void>;
  fetchModels: (providerId: string) => Promise<ModelConfig[]>;
}
```

### 4.3 OrbPanel 改造

OrbPanel 从 I1 占位替换为 ChatView：

```tsx
const OrbPanel: React.FC = () => {
  const { collapse } = useOrbExpand();
  // ...失焦/Escape 逻辑保持不变...

  return (
    <div className="orb-panel" style={...}>
      <ChatView />
    </div>
  );
};
```

### 4.4 主窗口路由扩展

App.tsx `MainLayout` 路由新增：
- `/history` → `HistoryManager`（I2-history 前置 UI 骨架）
- `/settings/providers` → `ProviderManager`
- `/settings/templates` → `TemplateManager`

侧边栏新增：
- "消息历史" 菜单项
- "AI 设置" 分组：Provider 管理 + Prompt 模板

### 4.5 右键菜单启用

OrbContextMenu "新建对话" 从 disabled 改为 enabled：
- 点击 → `chatStore.createConversation()` → 展开 Orb 面板

### 4.6 常量新增

| 常量 | 值 | 说明 |
|------|-----|------|
| `CHAT_PANEL_HEADER_HEIGHT` | 40 | 标题栏高度 |
| `CHAT_INPUT_MIN_ROWS` | 1 | 输入框最小行数 |
| `CHAT_INPUT_MAX_ROWS` | 4 | 输入框最大行数 |
| `CHAT_AUTO_TITLE_LENGTH` | 20 | 自动标题截取字符数 |
| `CHAT_SIDEBAR_WIDTH` | 200 | 对话列表侧栏宽度 |
| `STREAM_COALESCE_TIMER_MS` | 50 | 隐藏窗口兜底 timer 间隔 |
| `BUILTIN_TEMPLATE_COUNT` | 5 | 内置模板数量 |

> **I2-history 前置常量**（已提前实现）：

| 常量 | 值 | 说明 |
|------|-----|------|
| `MESSAGES_PAGE_SIZE` | 50 | 消息分页大小 |
| `VIRTUAL_SCROLL_THRESHOLD` | 100 | 虚拟滚动启用阈值 |
| `VIRTUAL_SCROLL_OVERSCAN` | 5 | 虚拟滚动额外渲染行数 |
| `SCROLL_LOAD_THRESHOLD_PX` | 100 | 滚动加载触发距离 |
| `CONTEXT_WARN_THRESHOLD` | 0.8 | 上下文警告阈值 |
| `CONTEXT_DANGER_THRESHOLD` | 0.95 | 上下文危险阈值 |
| `RETRY_MAX_ATTEMPTS` | 3 | 重试最大次数 |
| `RETRY_BASE_DELAY_MS` | 1000 | 重试基础延迟 |
| `RETRY_MAX_DELAY_MS` | 30000 | 重试最大延迟 |
| `CONVERSATION_SUMMARY_LENGTH` | 30 | 对话摘要截取长度 |

### 4.7 新增前端依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| `react-markdown` | ^9 | Markdown 渲染 |
| `remark-gfm` | ^4 | GFM 扩展（表格/任务列表/删除线） |

## 5. 不做的事（I2 范围外）

- ❌ Agent 模式切换（I6）
- ❌ Mascot 形象联动（I5）
- ❌ 工具调用 / MCP（I4/I6）
- ❌ Anthropic / Gemini 原生适配器（I2 仅实现 OpenAI 兼容格式，Anthropic/Gemini 可通过 Custom + 兼容端点使用）
- ❌ 自定义 Prompt 模板（I2 仅内置模板，自定义 I3）
- ❌ Context Compaction（I6）
- ❌ 多轮 token 计费展示（I3 增强）
- ❌ 流式 thinking/reasoning 展示（I3 增强）

> **实现前置说明**：对话搜索（`search_conversations`）和对话导出（`export_conversation`）的 Rust 端 commands 及前端 HistoryManager 组件已在 I2 迭代后期提前实现，作为 I2-history 的前置骨架。完整功能（分页/分支/高级搜索/导入等）在 I2-history 中补全。

## 6. 验收标准

1. 主窗口设置页可添加/编辑/删除 Provider，API Key 加密存储
2. 点击"获取模型列表"可从 API 拉取可用模型
3. Orb 面板内可选择 Provider + Model，发送消息
4. 流式输出逐字渲染，无卡顿/丢字/乱序
5. 流式输出中可点击 Stop 按钮取消
6. 对话历史持久化，关闭重开后可查看
7. 对话列表可新建/删除/重命名/切换
8. ~~空对话时显示 Prompt 模板快捷入口~~ — **已移除**（见 DEVLOG 2026-10-05）
9. 无 Provider 配置时显示引导提示
10. 窗口隐藏时流式输出不卡死（timer 兜底）
11. API Key 在 SQLite 中为加密密文，前端永远不接触明文
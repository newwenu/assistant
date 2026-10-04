# I2 — Orb AI 对话 任务清单

> 规格：[I2-spec.md](./I2-spec.md)
> 前置：I1 ✅ 已完成

---

## 阶段 A — Rust 后端：加密 + Provider 适配 + 存储

### A1. 新增依赖

**文件**：`src-tauri/Cargo.toml`（workspace.dependencies）

- [ ] 添加 `reqwest = { version = "0.12", features = ["stream", "json"] }`
- [ ] 添加 `tokio = { version = "1", features = ["full"] }`
- [ ] 添加 `futures = "0.3"`
- [ ] 添加 `aes-gcm = "0.10"`
- [ ] 添加 `sha2 = "0.10"`
- [ ] 添加 `base64 = "0.22"`
- [ ] 添加 `machine-uid = "0.2"`
- [ ] 添加 `uuid = { version = "1", features = ["v4"] }`
- [ ] 添加 `chrono = { version = "0.4", features = ["serde"] }`
- [ ] 添加 `tokio-util = { version = "0.7", features = ["rt"] }`

**文件**：`src-tauri/crates/assistant-providers/Cargo.toml`

- [ ] 添加 `reqwest`, `tokio`, `futures`, `aes-gcm`, `sha2`, `base64`, `machine-uid`

**文件**：`src-tauri/crates/assistant-storage/Cargo.toml`

- [ ] 添加 `uuid`, `chrono`

**依赖**：无

---

### A2. API Key 加密存储实现

**文件**：`src-tauri/crates/assistant-providers/src/crypto.rs`（重写）

- [ ] 实现 `get_machine_id() → Result<Vec<u8>, String>`
  - 调用 `machine_uid::get()` 获取机器唯一 ID
  - SHA-256 哈希 → 返回 32 字节作为 AES 密钥
  - 参考：Tauri 安全最佳实践 — 密钥派生自设备标识
- [ ] 实现 `encrypt_api_key(key: &str, machine_id: &[u8]) → Result<Vec<u8>, String>`
  - 使用 `Aes256Gcm::new_from_slice(machine_id)` 创建 cipher
  - 随机生成 12 字节 nonce (IV)
  - 加密明文 key → ciphertext + tag
  - 返回 `IV ‖ ciphertext ‖ tag`（拼接）
- [ ] 实现 `decrypt_api_key(encrypted: &[u8], machine_id: &[u8]) → Result<String, String>`
  - 从 encrypted 前 12 字节提取 IV，剩余为 ciphertext + tag
  - 解密 → 返回明文 String
- [ ] 单元测试：
  - `test_encrypt_decrypt_roundtrip` — 加密后解密应得到原文
  - `test_different_machine_ids_fail` — 不同 machine_id 解密应失败

**依赖**：A1

---

### A3. BaseProviderAdapter trait 扩展 + 数据模型

**文件**：`src-tauri/crates/assistant-providers/src/base.rs`（重写）

- [ ] 新增 `MessageRole` 枚举：`System / User / Assistant`
- [ ] `ChatMessage.role` 改为 `MessageRole`
- [ ] 新增 `ChatOptions { temperature: f32, max_tokens: u32, stream: bool }`
- [ ] 新增 `ChatDelta { delta_text: Option<String>, finish_reason: Option<String>, usage: Option<TokenUsage> }`
- [ ] 新增 `ModelConfig { id, name, owned_by }`
- [ ] 新增 `TokenUsage { prompt_tokens, completion_tokens, total_tokens }`
- [ ] 重构 `BaseProviderAdapter::chat_completion_stream` 签名：
  ```rust
  fn chat_completion_stream(
      &self,
      messages: &[ChatMessage],
      model: &str,
      options: &ChatOptions,
  ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatDelta, ProviderError>> + Send>>, ProviderError>
  ```
- [ ] 新增 `fn list_models(&self) -> Result<Vec<ModelConfig>, ProviderError>`

**依赖**：A1

---

### A4. OpenAIAdapter 完整实现

**文件**：`src-tauri/crates/assistant-providers/src/openai.rs`（重写）

- [ ] `chat_completion_stream` 方法：
  - 构建 HTTP POST 请求到 `{base_url}/chat/completions`
  - Body：`{ model, messages, stream: true, temperature, max_tokens }`
  - Header：`Authorization: Bearer {decrypted_api_key}`
  - 使用 `reqwest::Client::new().post(url).body(json).send()`
  - 解析 SSE 响应流：按行分割 → 过滤空行和 `[DONE]` → JSON parse → 提取 `choices[0].delta.content`
  - 返回 `impl Stream<Item = Result<ChatDelta>>`
  - 错误处理：HTTP 状态码映射到 `ProviderError` 变体
- [ ] `list_models` 方法：
  - GET `{base_url}/models`
  - 解析 `data[].id` → `Vec<ModelConfig>`
- [ ] 错误处理完善：
  - `RequestFailed(String)` — 网络/HTTP 错误
  - `ParseError(String)` — JSON/SSE 解析错误
  - `RateLimited` — 429 状态码
  - `ModelNotFound(String)` — 模型不存在
  - 超时：`reqwest::ClientBuilder::timeout(Duration::from_secs(120))`

**依赖**：A2、A3

---

### A5. assistant-storage crate CRUD 实现

**文件**：`src-tauri/crates/assistant-storage/src/providers.rs`（新建）

- [ ] 定义 `ProviderRow { id, name, type_, base_url, api_key_encrypted, models_json, default_model, updated_at }`
- [ ] `insert_provider(conn: &Connection, row: &ProviderRow) → Result<()>`
- [ ] `get_providers(conn: &Connection) → Result<Vec<ProviderRow>>`
- [ ] `update_provider(conn: &Connection, row: &ProviderRow) → Result<()>`
- [ ] `delete_provider(conn: &Connection, id: &str) → Result<()>`（级联删除 conversations）
- [ ] `get_provider_by_id(conn: &Connection, id: &str) → Result<Option<ProviderRow>>`

**文件**：`src-tauri/crates/assistant-storage/src/conversations.rs`（新建）

- [ ] 定义 `ConversationRow { id, title, provider_id, model_id, system_prompt, created_at, updated_at }`
- [ ] 定义 `MessageRow { id, conversation_id, role, content, tokens, timestamp }`
- [ ] 定义 `ConversationDetail { conversation, messages }`
- [ ] `insert_conversation(conn: &Connection, row: &ConversationRow) → Result<()>`
- [ ] `get_conversations(conn: &Connection, provider_id: Option<&str>) → Result<Vec<ConversationRow>>`（按 updated_at DESC 排序）
- [ ] `get_conversation_detail(conn: &Connection, id: &str) → Result<ConversationDetail>`（含 messages 按 timestamp ASC）
- [ ] `update_conversation(conn: &Connection, row: &ConversationRow) → Result<()>`
- [ ] `delete_conversation(conn: &Connection, id: &str) → Result<()>`（级联删除 messages）
- [ ] `insert_message(conn: &Connection, row: &MessageRow) → Result<()>`
- [ ] `get_messages(conn: &Connection, conversation_id: &str) → Result<Vec<MessageRow>>`

**文件**：`src-tauri/crates/assistant-storage/src/templates.rs`（新建）

- [ ] 定义 `PromptTemplateRow { id, name, category, content, variables_json, is_builtin }`
- [ ] `insert_template(conn: &Connection, row: &PromptTemplateRow) → Result<()>`
- [ ] `get_templates(conn: &Connection) → Result<Vec<PromptTemplateRow>>`
- [ ] `seed_builtin_templates(conn: &Connection) → Result<()>`（插入 5 个内置模板）
- [ ] 内置模板数据（见 I2-spec 2.9 节表）

**文件**：`src-tauri/crates/assistant-storage/src/lib.rs`（修改）

- [ ] 导出 `pub mod providers; pub mod conversations; pub mod templates;`

**依赖**：A1

---

### A6. Tauri Commands — Provider 管理

**文件**：`src-tauri/src/cmd.rs`（扩展）

- [ ] `list_providers(app: AppHandle) → Result<Value>`
  - 从 DbState 获取连接 → `get_providers()` → 序列化返回
  - API Key 返回脱敏格式：`sk-***{last4chars}` 或仅显示 `••••••••`
- [ ] `save_provider(app: AppHandle, provider: Value) → Result<()>`
  - 解析输入 → 若有 api_key 明文 → `encrypt_api_key()` → 存入 `api_key_encrypted`
  - models 序列化为 JSON 字符串
  - insert 或 update（根据 id 是否存在）
- [ ] `delete_provider(app: AppHandle, id: String) → Result<()>`
  - 删除 Provider + 级联删除其下所有 conversations
- [ ] `list_models(app: AppHandle, provider_id: String) → Result<Value>`
  - 从 DB 获取 Provider 配置 → 解密 API Key → 构造 OpenAIAdapter → `list_models()` → 返回

**依赖**：A4、A5

---

### A7. Tauri Commands — 对话管理

**文件**：`src-tauri/src/cmd.rs`（继续扩展）

- [ ] `create_conversation(app: AppHandle, provider_id: String, model_id: String) → Result<String>`
  - 生成 UUID → 插入 conversations 表 → 返回 id
- [ ] `list_conversations(app: AppHandle, provider_id: Option<String>) → Result<Value>`
  - `get_conversations()` → 序列化返回
- [ ] `get_conversation(app: AppHandle, id: String) → Result<Value>`
  - `get_conversation_detail()` → 含 messages → 序列化返回
- [ ] `delete_conversation(app: AppHandle, id: String) → Result<()>`
  - 删除对话 + 级联消息
- [ ] `rename_conversation(app: AppHandle, id: String, title: String) → Result<()>`
  - 更新 title + updated_at
- [ ] `list_prompt_templates(app: AppHandle) → Result<Value>`
  - `get_templates()` → 序列化返回
- [ ] `save_prompt_template(app: AppHandle, template: Value) → Result<()>`
  - insert 或 update

**依赖**：A5

---

### A8. Tauri Commands — 流式聊天核心

**文件**：`src-tauri/src/cmd.rs`（继续扩展）

- [ ] `send_chat_message(app: AppHandle, conversation_id: String, content: String) → Result<()>`
  1. 插入 user message 到 SQLite（role=user, content=content）
  2. 从 DB 获取 conversation 详情（provider_id, model_id）+ 历史 messages
  3. 从 DB 获取 Provider → 解密 API Key → 构造 OpenAIAdapter
  4. 创建 assistant message 占位记录（id=UUID, role=assistant, content=""）
  5. `tokio::spawn` 异步任务：
     a. 调用 `adapter.chat_completion_stream(messages, model, options)`
     b. 流式消费 Stream：
        - 每个 delta 有 `delta_text` → `app.emit("chat-delta", { conversation_id, message_id, delta_text })`
        - delta 有 `finish_reason` → 更新 assistant message content → emit finish delta
        - delta 有 `usage` → 更新消息 tokens
     c. 错误处理：emit error delta
  6. 返回 Ok(())（不阻塞前端）
- [ ] `cancel_chat(app: AppHandle, conversation_id: String) → Result<()>`
  - 从 ChatTasks 取出 CancellationToken → cancel()
  - emit chat-delta with error="cancelled"
- [ ] **ChatTasks 状态管理**：
  - 新建 `src-tauri/src/chat_tasks.rs`：`ChatTasks(Mutex<HashMap<String, CancellationToken>>)`
  - `start(conversation_id)` → 创建 CancellationToken → 存入 map → 返回 clone
  - `cancel(conversation_id)` → 取消并移除
  - 在 `setup.rs` 中 `app.manage(ChatTasks::new())`

**依赖**：A6、A7、A4

---

### A9. setup.rs 集成 + lib.rs 注册

**文件**：`src-tauri/src/setup.rs`（修改）

- [ ] 在 `init` 中创建 DbState 并 manage：`app.manage(DbState(Mutex::new(conn)))`
- [ ] 在 `init` 中创建 ChatTasks 并 manage：`app.manage(ChatTasks::new())`
- [ ] 调用 `seed_builtin_templates()`（若模板表为空）

**文件**：`src-tauri/src/lib.rs`（修改）

- [ ] 在 `invoke_handler` 中注册 A6~A8 所有新 commands
- [ ] 新增 `mod chat_tasks;`

**依赖**：A5、A8

---

### A10. 常量对齐

**文件**：`src-tauri/crates/assistant-config/src/constant.rs`

- [ ] 新增：
  ```rust
  pub const CHAT_PANEL_HEADER_HEIGHT: u32 = 40;
  pub const CHAT_INPUT_MIN_ROWS: u32 = 1;
  pub const CHAT_INPUT_MAX_ROWS: u32 = 4;
  pub const CHAT_AUTO_TITLE_LENGTH: usize = 20;
  pub const CHAT_SIDEBAR_WIDTH: u32 = 200;
  pub const STREAM_COALESCE_TIMER_MS: u64 = 50;
  ```

**依赖**：无

---

## 阶段 B — 前端：Store + 对话 UI + Provider 设置

### B1. 前端依赖安装

- [ ] `npm install react-markdown remark-gfm`

**依赖**：无

---

### B2. 常量对齐

**文件**：`src/config/constants.ts`

- [ ] 新增与 Rust constant.rs 对齐的常量：
  ```typescript
  export const CHAT_PANEL_HEADER_HEIGHT = 40;
  export const CHAT_INPUT_MIN_ROWS = 1;
  export const CHAT_INPUT_MAX_ROWS = 4;
  export const CHAT_AUTO_TITLE_LENGTH = 20;
  export const CHAT_SIDEBAR_WIDTH = 200;
  export const STREAM_COALESCE_TIMER_MS = 50;
  export const BUILTIN_TEMPLATE_COUNT = 5;
  ```

**依赖**：无

---

### B3. chatStore 状态管理

**文件**：`src/stores/chatStore.ts`（新建）

- [ ] Zustand store 定义（见 I2-spec 4.2 节接口）
- [ ] `loadConversations()`：`invoke("list_conversations")` → set conversations
- [ ] `createConversation(providerId, modelId)`：`invoke("create_conversation")` → switchConversation
- [ ] `switchConversation(id)`：`invoke("get_conversation")` → set currentConversationId + messages
- [ ] `deleteConversation(id)`：确认后 `invoke("delete_conversation")` → reload
- [ ] `renameConversation(id, title)`：`invoke("rename_conversation")` → reload
- [ ] `sendMessage(content)`：
  1. 插入 user message 到本地 messages
  2. 创建空的 assistant message（streamingContent=""）
  3. set isStreaming=true
  4. `invoke("send_chat_message", { conversationId, content })`
- [ ] `appendDelta(delta)`：追加到 streamingContent
- [ ] `finishStream(usage?)`：将 streamingContent 写入 assistant message content → isStreaming=false
- [ ] `errorStream(error)`：标记消息错误 → isStreaming=false
- [ ] `cancelStream()`：`invoke("cancel_chat")` → isStreaming=false
- [ ] `toggleSidebar()`：切换 sidebarOpen
- [ ] **初始化**：`useEffect` 中调用 `loadConversations()`
- [ ] **chat-delta 事件监听**：
  - `listen("chat-delta", (event) => { ... })`
  - event.payload.delta_text → appendDelta
  - event.payload.finish_reason → finishStream
  - event.payload.error → errorStream
  - event.payload.usage → 记录 usage

**依赖**：B2

---

### B4. providerStore 状态管理

**文件**：`src/stores/providerStore.ts`（新建）

- [ ] Zustand store 定义（见 I2-spec 4.2 节接口）
- [ ] `loadProviders()`：`invoke("list_providers")` → set providers
- [ ] `saveProvider(input)`：`invoke("save_provider", input)` → reload
- [ ] `deleteProvider(id)`：确认后 `invoke("delete_provider", id)` → reload
- [ ] `fetchModels(providerId)`：`invoke("list_models", providerId)` → 返回 ModelConfig[]
- [ ] **初始化**：`useEffect` 中调用 `loadProviders()`

**依赖**：B2

---

### B5. streamCoalescer 流式增量合并器

**文件**：`src/chat/streamCoalescer.ts`（新建）

- [ ] 移植 OpenHuman `createChatDeltaCoalescer` 核心逻辑（简化版，无需 thread/request_id/round 分层）
- [ ] `frameScheduler`：`requestAnimationFrame` + `setTimeout(STREAM_COALESCE_TIMER_MS)` 双保险
- [ ] `CoalescibleDelta`：`{ conversation_id: string; message_id: string; delta: string }`
- [ ] `createChatDeltaCoalescer(deliver, schedule?)` → `{ push, flush, dispose }`
- [ ] 合并条件：相邻且 `(conversation_id, message_id)` 相同 → 拼接 delta
- [ ] flush：清空队列 → 逐条 deliver
- [ ] dispose：flush + 取消调度
- [ ] 单元测试（可选）：验证合并 + flush + dispose 行为
- [ ] 参考：[openhuman/app/src/services/chatDeltaCoalescer.ts](../../references/openhuman/app/src/services/chatDeltaCoalescer.ts)

**依赖**：无

---

### B6. MessageBubble 消息气泡组件

**文件**：`src/chat/MessageBubble.tsx`（新建）

- [ ] Props：`message: Message; isStreaming?: boolean`
- [ ] 用户消息：右对齐，蓝色背景（`var(--color-primary-bg)`），白色文字
- [ ] 助手消息：左对齐，灰色背景（`var(--color-fill-secondary)`），深色文字
- [ ] Markdown 渲染：使用 `ReactMarkdown` + `remarkGfm` 插件
- [ ] 流式光标：isStreaming=true 时在末尾显示闪烁光标 `▍`（CSS animation blink）
- [ ] 错误状态：红色边框 + 错误提示文字
- [ ] 时间戳显示（hover 时显示）
- [ ] 样式：圆角 12px，max-width 85%，padding 10px 14px，margin 4px 0

**依赖**：B1（react-markdown）

---

### B7. MessageList 消息列表组件

**文件**：`src/chat/MessageList.tsx`（新建）

- [ ] Props：`messages: Message[]; isStreaming: boolean`
- [ ] 滚动容器：`overflow-y: auto; flex: 1`
- [ ] 自动滚底逻辑：
  - 新消息时 → 若用户未手动上滚 → `scrollTo({ top: scrollHeight, behavior: 'smooth' })`
  - 用户上滚（距底部 > 100px）→ 显示"回到底部"浮动按钮
  - 点击"回到底部"按钮 → scrollTo bottom + 隐藏按钮
  - isStreaming 且在底部 → 实时跟随滚动
- [ ] 空状态：显示 `<TemplateCards />`
- [ ] 消息列表渲染：`messages.map(msg => <MessageBubble key={msg.id} message={msg} isStreaming={...} />)`
- [ ] 性能优化：长列表考虑虚拟滚动（I2 可先不做，>100 条消息再优化）

**依赖**：B6

---

### B8. MessageInput 输入区组件

**文件**：`src/chat/MessageInput.tsx`（新建）

- [ ] Props：`onSend: (content: string) => void; onCancel: () => void; isStreaming: boolean; disabled: boolean`
- [ ] `TextArea` 组件（Ant Design）：
  - autoSize: { minRows: CHAT_INPUT_MIN_ROWS, maxRows: CHAT_INPUT_MAX_ROWS }
  - placeholder: "输入消息..." / "请先配置 AI Provider"（disabled 时）
  - disabled: 无 Provider 或 isStreaming
- [ ] Enter 发送，Shift+Enter 换行（`onKeyDown` 处理）
- [ ] 发送按钮：
  - isStreaming=false → Send 图标（蓝色）
  - isStreaming=true → Stop 图标（红色），点击调用 onCancel
- [ ] 发送条件：content.trim().length > 0 && !disabled
- [ ] 发送后清空输入框
- [ ] 样式：底部固定，padding 8px 12px，border-top

**依赖**：无

---

### B9. ModelSelector 模型选择组件

**文件**：`src/chat/ModelSelector.tsx`（新建）

- [ ] Props：`providerId: string | null; modelId: string; onModelChange: (modelId: string) => void; providers: Provider[]`
- [ ] Ant Design `Select` 组件
- [ ] 根据 providerId 过滤当前 Provider 的模型列表
- [ ] 下拉选项：每个模型显示 `name`（或 `id`），value 为 `id`
- [ ] 无 Provider 时：显示 "—"（禁用态）
- [ ] 切换模型：调用 onModelChange → 更新 chatStore 当前模型
- [ ] 样式：紧凑型，宽度 160px

**依赖**：无

---

### B10. ConversationSidebar 对话列表侧栏

**文件**：`src/chat/ConversationSidebar.tsx`（新建）

- [ ] Props：`conversations: Conversation[]; currentId: string | null; onSelect: (id: string) => void; onDelete: (id: string) => void; onRename: (id: string, title: string) => void; onCreate: () => void; visible: boolean`
- [ ] 侧栏布局：宽度 CHAT_SIDEBAR_WIDTH，左侧滑入动画（CSS transform + transition）
- [ ] 顶部搜索框：`Input.Search`，过滤 conversations by title
- [ ] 对话列表项：
  - 标题（截断单行）
  - 时间（相对时间："刚刚"/"5分钟前"/"昨天"/日期）
  - 选中高亮（`var(--color-primary-bg-light)`）
  - 右键菜单：重命名 / 删除
- [ ] 底部：新建对话按钮
- [ ] 重命名：Inline 编辑（点击标题 → Input → blur 保存）
- [ ] 删除确认：`Modal.confirm`
- [ ] 空列表提示："暂无对话"

**依赖**：无

---

### B11. TemplateCards Prompt 模板卡片

**文件**：`src/chat/TemplateCards.tsx`（新建）

- [ ] Props：`templates: PromptTemplate[]; onSelect: (template: PromptTemplate) => void`
- [ ] 网格布局：2 列，卡片式展示
- [ ] 每张卡片：
  - 图标（根据 category 选择：翻译=🌐 / 代码=💻 / 文本=📝 / 续写=✏️ / 头脑风暴=💡）
  - 名称
  - category 标签
  - 点击触发 onSelect
- [ ] 变量填写弹窗（可选 I2 简化版）：
  - 解析 template.content 中的 `{{variable}}` 占位符
  - 弹出 Modal 表单，每个变量一个 Input
  - 填充后替换占位符 → 填入 MessageInput

**依赖**：无

---

### B12. ChatView 对话主视图

**文件**：`src/chat/ChatView.tsx`（新建）

- [ ] 整合 MessageList + MessageInput + ModelSelector + ConversationSidebar
- [ ] 标题栏（高度 CHAT_PANEL_HEADER_HEIGHT）：
  - 左：返回按钮（sidebarOpen 控制）+ 对话标题（可编辑，点击进入 inline edit）
  - 右：<ModelSelector /> + 新建对话按钮（PlusOutlined）
- [ ] 布局：flex column，header 固定 + MessageList flex:1 + input 固定
- [ ] 无 Provider 引导态：
  - 当 providers 为空时，全屏显示引导卡片
  - "配置 AI Provider 开始对话" + 按钮 → 打开主窗口 `/settings/providers`
- [ ] 发送流程：
  - MessageInput onSend → chatStore.sendMessage(content)
  - sendMessage 内部：invoke send_chat_message → Rust 端流式响应 → chat-delta 事件 → coalescer → chatStore.appendDelta → UI 更新
- [ ] 取消流程：
  - MessageInput onCancel → chatStore.cancelStream()
- [ ] 新建对话流程：
  - 若有多个 Provider → 弹窗选择 Provider + Model
  - 若只有一个 Provider → 直接用默认 Model
  - chatStore.createConversation → 切换到新对话
- [ ] 自动标题：首条助手消息完成后，取前 CHAT_AUTO_TITLE_LENGTH 字符 → invoke rename_conversation

**依赖**：B3、B6、B7、B8、B9、B10、B11

---

### B13. OrbPanel 改造：接入 ChatView

**文件**：`src/orb/OrbPanel.tsx`（修改）

- [ ] import ChatView 替换占位内容
- [ ] 移除 "I2 阶段实现 AI 对话" 和 "I2 阶段启用输入" 占位文字
- [ ] 渲染 `<ChatView />` 替代中间和底部 div
- [ ] 保留失焦收起 + Escape 收起逻辑不变
- [ ] 保留外层样式（position/inset/background/borderRadius/boxShadow/flex/column/overflow/zIndex）

**依赖**：B12

---

### B14. ProviderManager Provider 设置页

**文件**：`src/settings/ProviderManager.tsx`（新建）

- [ ] Provider 列表页：
  - 页面标题 "AI Provider 设置"
  - Provider 卡片列表（名称 + 类型标签 + base_url + 默认模型 + 最后更新时间）
  - 每张卡片：编辑按钮 + 删除按钮
  - "添加 Provider" 按钮（顶部右侧）
- [ ] 编辑/新增模式：
  - 点击添加/编辑 → 进入 ProviderForm
  - 保存成功后返回列表
- [ ] 删除确认：`Modal.confirm` → 确认后 `providerStore.deleteProvider(id)`
- [ ] 空状态："尚未配置 Provider" + 添加按钮

**依赖**：B4

---

### B15. ProviderForm Provider 编辑表单

**文件**：`src/settings/ProviderForm.tsx`（新建）

- [ ] Props：`provider?: Provider | null; onSave: (input: ProviderInput) => void; onCancel: () => void`
- [ ] 表单字段（Ant Design Form）：
  - 名称：`Input`，required
  - 类型：`Select`（OpenAI / Anthropic / Gemini / Custom），required
  - Base URL：`Input`，Custom 类型时必填，其他类型有默认值
  - API Key：`Input.Password`，required，placeholder "sk-..."
  - 默认模型：`Select`（从已获取的模型列表中选择，可为空）
  - 模型列表获取：`Button` "获取模型列表" → `providerStore.fetchModels(providerId)` → 填充模型下拉
- [ ] 表单校验：必填字段 + URL 格式校验
- [ ] 保存逻辑：收集表单值 → `providerStore.saveProvider(input)` → 成功后回调
- [ ] 编辑模式预填：provider 存在时填充表单（API Key 显示 `••••••••`，不回填明文；若用户修改则用新值，未修改则保留原加密值——需特殊处理：Rust 端 save_provider 判断 api_key 是否为空/占位符来决定是否重新加密）

**依赖**：B4

---

### B16. TemplateManager Prompt 模板管理页

**文件**：`src/settings/TemplateManager.tsx`（新建）

- [ ] 页面标题 "Prompt 模板"
- [ ] 模板卡片网格（只读展示，I2 不支持自定义）
- [ ] 每张卡片：
  - 名称 + 分类标签
  - 内容预览（截断 3 行）
  - 变量列表（解析 `{{variable}}` 显示）
  - "内置" 标签（is_builtin=1）
- [ ] I2 范围：仅展示，不支持编辑/删除/新增自定义模板

**依赖**：B3

---

### B17. 主窗口路由 + 侧边栏扩展

**文件**：`src/App.tsx`（修改）

- [ ] MainLayout 路由新增：
  ```tsx
  <Route path="/settings/providers" element={<ProviderManager />} />
  <Route path="/settings/templates" element={<TemplateManager />} />
  ```
- [ ] 侧边栏新增 "AI 设置" 分组：
  - "Provider 管理" → 导航到 `/settings/providers`
  - "Prompt 模板" → 导航到 `/settings/templates`
- [ ] SettingsPage 可改为链接到子路由（或保持独立页面）

**依赖**：B14、B16

---

### B18. OrbContextMenu 启用新建对话

**文件**：`src/orb/OrbContextMenu.tsx`（修改）

- [ ] "新建对话" 菜单项从 `disabled` 改为正常
- [ ] onClick：调用 `chatStore.createConversation()`（需选择 Provider/Model 的逻辑与 ChatView 一致）
- [ ] 若无 Provider 配置：点击后提示 "请先在设置中配置 AI Provider"

**依赖**：B3、B12

---

### B19. CSS 样式补充

**文件**：`src/assets/styles/global.css`（修改）

- [ ] 对话面板内联样式提取为 class（OrbPanel 内联 style → CSS class）
- [ ] 新增 `.chat-view` / `.chat-header` / `.chat-messages` / `.chat-input` 等 class
- [ ] 消息气泡样式：`.msg-bubble` / `.msg-bubble-user` / `.msg-bubble-assistant` / `.msg-bubble-error`
- [ ] 流式光标动画：`@keyframes blink-cursor { 0%, 100% { opacity: 1; } 50% { opacity: 0; } }`
- [ ] 侧栏滑入动画：`.chat-sidebar { transition: transform 0.25s ease; }`
- [ ] 回到底部按钮：`.scroll-to-bottom` 浮动按钮
- [ ] 模板卡片样式：`.template-card` 网格布局
- [ ] Markdown 渲染样式覆盖（code block / table / list 等）

**依赖**：B6、B7、B10、B11

---

## 阶段 C — 集成 + 验收

### C1. capabilities 权限补全

**文件**：`src-tauri/capabilities/default.json`

- [ ] 确认 SQL 权限足够（I0 已配置 `sql:default`）
- [ ] 如需 HTTP 权限（reqwest 在 Rust 端发起，不需要前端权限）

**依赖**：A9

---

### C2. 编译验证

- [ ] `cargo build` 通过（Rust 后端，含新增 crate 依赖编译）
- [ ] `npx tsc --noEmit` 通过（TypeScript 类型检查）
- [ ] `npx eslint src` 通过（ESLint 检查）
- [ ] `npm run build` 通过（Vite 构建）

**依赖**：所有 A、B 阶段任务

---

### C3. 功能验收

- [ ] 1. 主窗口设置页可添加/编辑/删除 Provider
- [ ] 2. API Key 在 SQLite 中为加密密文（可用 DB 工具验证）
- [ ] 3. 点击"获取模型列表"可拉取模型
- [ ] 4. Orb 面板可选择 Provider + Model
- [ ] 5. 发送消息后流式输出逐字渲染
- [ ] 6. 流式中 Stop 按钮可取消
- [ ] 7. 关闭重开后对话历史存在
- [ ] 8. 对话列表可新建/删除/重命名/切换
- [ ] 9. 空对话显示 Prompt 模板快捷入口
- [ ] 10. 无 Provider 时显示引导
- [ ] 11. 窗口隐藏时流式输出不卡死（最小化窗口测试）
- [ ] 12. Markdown 渲染正确（代码块/表格/列表/加粗/斜体）

**依赖**：C2

---

### C4. DEVLOG 记录

**文件**：`docs/DEVLOG.md`

- [ ] 新增 `## I2 — Orb AI 对话` 章节
- [ ] 记录技术决策、踩坑、与参考项目的差异

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
| 修改 | `src-tauri/Cargo.toml` | +9 个 workspace 依赖 |
| 修改 | `src-tauri/crates/assistant-providers/Cargo.toml` | +7 个依赖 |
| 修改 | `src-tauri/crates/assistant-storage/Cargo.toml` | +2 个依赖 |
| 重写 | `src-tauri/crates/assistant-providers/src/crypto.rs` | AES-256-GCM 完整实现 |
| 重写 | `src-tauri/crates/assistant-providers/src/base.rs` | trait 扩展 + 数据模型 |
| 重写 | `src-tauri/crates/assistant-providers/src/openai.rs` | 完整 OpenAI 适配器 |
| 新建 | `src-tauri/crates/assistant-storage/src/providers.rs` | Provider CRUD |
| 新建 | `src-tauri/crates/assistant-storage/src/conversations.rs` | Conversation + Message CRUD |
| 新建 | `src-tauri/crates/assistant-storage/src/templates.rs` | Template CRUD + seed |
| 修改 | `src-tauri/crates/assistant-storage/src/lib.rs` | 导出新模块 |
| 修改 | `src-tauri/src/cmd.rs` | +13 个 commands |
| 新建 | `src-tauri/src/chat_tasks.rs` | 流式任务取消管理 |
| 修改 | `src-tauri/src/setup.rs` | DbState + ChatTasks manage + seed |
| 修改 | `src-tauri/src/lib.rs` | 注册新 commands + mod |
| 修改 | `src-tauri/crates/assistant-config/src/constant.rs` | +6 常量 |
| 修改 | `package.json` | +react-markdown +remark-gfm |
| 修改 | `src/config/constants.ts` | +7 常量 |
| 新建 | `src/stores/chatStore.ts` | 对话状态管理 |
| 新建 | `src/stores/providerStore.ts` | Provider 状态管理 |
| 新建 | `src/chat/streamCoalescer.ts` | 流式增量合并器 |
| 新建 | `src/chat/ChatView.tsx` | 对话主视图 |
| 新建 | `src/chat/MessageList.tsx` | 消息列表 |
| 新建 | `src/chat/MessageBubble.tsx` | 消息气泡 |
| 新建 | `src/chat/MessageInput.tsx` | 输入区 |
| 新建 | `src/chat/ModelSelector.tsx` | 模型选择 |
| 新建 | `src/chat/ConversationSidebar.tsx` | 对话列表侧栏 |
| 新建 | `src/chat/TemplateCards.tsx` | Prompt 模板卡片 |
| 修改 | `src/orb/OrbPanel.tsx` | 接入 ChatView |
| 新建 | `src/settings/ProviderManager.tsx` | Provider 设置页 |
| 新建 | `src/settings/ProviderForm.tsx` | Provider 编辑表单 |
| 新建 | `src/settings/TemplateManager.tsx` | 模板管理页 |
| 修改 | `src/App.tsx` | 路由扩展 + 侧边栏扩展 |
| 修改 | `src/orb/OrbContextMenu.tsx` | 启用新建对话 |
| 修改 | `src/assets/styles/global.css` | 对话相关 CSS |
| 修改 | `docs/DEVLOG.md` | I2 开发日志 |
# I7 前置 — 上下文窗口获取改造方案

> 参考：deepseek-harness-orb `discovery.ts` 多字段兼容发现
> 目标：修复当前上下文窗口无法正确获取的问题，为 I7 Context Compaction 的 `should_terminate` 提供准确的 `context_window_tokens`

---

## 1. 问题诊断

### 1.1 当前架构全景

```
┌─────────────────────────────────────────────────────────────────────┐
│                        模型发现阶段                                  │
│  list_models_cmd / preview_models_cmd                               │
│    → adapter.list_models()                                          │
│      → GET /models → parse response                                 │
│      → get_context_window_tokens(model_id)  ← 硬编码映射表           │
│      → ModelConfig { id, name, owned_by, context_window }           │
│    → 存入 provider.models_json (SQLite)                              │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      发送消息阶段                                    │
│  send_chat_message                                                  │
│    → ctx = get_context_window_tokens(&model)  ← 硬编码映射表         │
│    → if models_json[model].context_window > 0: ctx = 覆盖           │
│    → ChatOptions.context_window_tokens = ctx                        │
│    → 流式 delta 携带 usage.context_window_tokens                    │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      前端消费阶段                                    │
│  chatDeltaEvents.ts:                                                │
│    → payload.usage.context_window_tokens → contextUsage.max_tokens  │
│  chatStore.switchConversation:                                      │
│    → invoke("get_conversation_token_usage", { conversationId })      │
│      ❌ 未传 context_window_tokens → Rust 默认 128_000              │
│  chatStore.finishStream (无 usage 时):                              │
│    → invoke("get_conversation_token_usage", { conversationId })      │
│      ❌ 未传 context_window_tokens → Rust 默认 128_000              │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 问题清单

| # | 问题 | 严重度 | 影响 |
|---|------|--------|------|
| P1 | `get_conversation_token_usage` 前端调用未传 `context_window_tokens`，Rust 默认 `128_000` | **高** | 切换对话/流结束无 usage 时，Claude 200k/Gemini 1M 等模型显示为 128k |
| P2 | API `/models` 响应中的上下文字段被丢弃，仅靠硬编码映射 | **中** | 新模型/自定义模型无法获取正确上下文窗口 |
| P3 | OpenAI `ModelItem` 未解析 `context_window`/`context_length` 等字段 | **中** | OpenRouter/Groq 等网关返回的上下文信息丢失 |
| P4 | Gemini `GeminiModelEntry` 未解析 `input_token_limit` 字段 | **中** | Google 官方 API 返回的上下文窗口被丢弃 |
| P5 | Anthropic `AnthropicModelEntry` 未解析上下文字段 | **低** | Anthropic 官方 API 目前不返回上下文字段，但网关可能返回 |
| P6 | 前端 `ModelConfig.context_window` 为 `optional`，但 Rust 端 `context_window: u32` 默认 `0`，序列化后前端收到 `0` 而非 `undefined` | **低** | 前端 `context_window?: number` 语义不一致 |

### 1.3 deepseek-harness-orb 的设计要点

```
┌──────────────────────────────────────────────────────────────┐
│  discovery.ts 的三层优先级                                     │
│                                                                │
│  1. Catalog 优先                                               │
│     catalogModels(provider) → 内置模型注册表                   │
│     携带 contextWindow + maxTokens + inputModalities           │
│     无网络调用，最权威                                          │
│                                                                │
│  2. Endpoint 发现                                              │
│     GET /models → readListing(body)                            │
│     多字段兼容解析：                                            │
│       contextWindow   ← OpenAI camelCase                      │
│       context_window  ← snake_case                            │
│       context_length  ← Ollama / LM Studio                    │
│       max_input_tokens ← 某些 gateway                         │
│       limit.context   ← 嵌套结构 (Anthropic)                  │
│                                                                │
│  3. capacity() 函数                                            │
│     按优先级依次尝试，取第一个正整数值                            │
│     capacity(entry.contextWindow, entry.context_window, ...)   │
│                                                                │
│  关键：contextWindow 和 maxTokens 分离                         │
│  上下文窗口 ≠ 最大输出 tokens                                   │
└──────────────────────────────────────────────────────────────┘
```

---

## 2. 改造方案

### 2.0 设计原则

1. **API 响应优先**：从 `/models` 响应中解析上下文字段，作为第一优先级
2. **硬编码兜底**：`get_context_window_tokens()` 硬编码映射表作为第二优先级（API 不返回时兜底）
3. **多字段兼容**：参考 deepseek-harness-orb 的 `capacity()` 模式，兼容多种字段命名
4. **前端传参修复**：`get_conversation_token_usage` 调用时传入正确的 `context_window_tokens`
5. **最小改动**：不改变现有数据结构，只增强解析能力和修复传参

### 2.1 优先级链

```
resolve_context_window(model_id, api_entry):
  1. api_entry 中的上下文字段（多字段兼容）  ← 新增
  2. get_context_window_tokens(model_id)      ← 现有硬编码映射
  3. DEFAULT_CONTEXT_WINDOW = 128_000          ← 最终兜底
```

### 2.2 Rust 端改造

#### 2.2.1 新增 `resolve_context_window` 函数

**文件**：`assistant-providers/src/types/config.rs`

```rust
/// 多字段兼容上下文窗口解析，参考 deepseek-harness-orb discovery.ts capacity()
///
/// 优先级：
///   1. API 响应中的上下文字段（按常见程度排序）
///   2. get_context_window_tokens() 硬编码映射表
///   3. DEFAULT_CONTEXT_WINDOW (128_000)
pub fn resolve_context_window(model_id: &str, api_context: Option<u32>) -> u32 {
    if let Some(ctx) = api_context {
        if ctx > 0 {
            return ctx;
        }
    }
    get_context_window_tokens(model_id)
}
```

#### 2.2.2 OpenAI types.rs — ModelItem 扩展

**文件**：`assistant-providers/src/openai/types.rs`

```rust
#[derive(Deserialize, Debug)]
pub(crate) struct ModelItem {
    pub id: String,
    #[serde(default)]
    pub owned_by: Option<String>,
    // --- 新增：多字段兼容上下文窗口 ---
    #[serde(default)]
    pub context_window: Option<u32>,
    #[serde(default)]
    pub context_length: Option<u32>,
    #[serde(default)]
    pub max_input_tokens: Option<u32>,
}
```

**新增辅助函数**：

```rust
impl ModelItem {
    pub(crate) fn resolved_context_window(&self) -> Option<u32> {
        capacity(self.context_window, self.context_length, self.max_input_tokens)
    }
}

fn capacity(candidates: Option<u32>...) -> Option<u32> {
    // 取第一个 Some(正整数)
}
```

#### 2.2.3 OpenAI discovery.rs — 使用 API 上下文

**文件**：`assistant-providers/src/openai/discovery.rs`

```rust
// 改造前
let ctx = get_context_window_tokens(&m.id);

// 改造后
let ctx = resolve_context_window(&m.id, m.resolved_context_window());
```

#### 2.2.4 Gemini types.rs — GeminiModelEntry 扩展

**文件**：`assistant-providers/src/gemini/types.rs`

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeminiModelEntry {
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub supported_generation_methods: Vec<String>,
    // --- 新增 ---
    #[serde(default)]
    pub input_token_limit: Option<u32>,
}
```

#### 2.2.5 Gemini discovery.rs — 使用 API 上下文

```rust
// 改造前
let ctx = get_context_window_tokens(&id);

// 改造后
let ctx = resolve_context_window(&id, model.input_token_limit);
```

#### 2.2.6 Anthropic types.rs — AnthropicModelEntry 扩展

**文件**：`assistant-providers/src/anthropic/types.rs`

```rust
#[derive(Deserialize, Debug)]
pub(crate) struct AnthropicModelEntry {
    pub id: String,
    #[serde(default)]
    pub display_name: Option<String>,
    // --- 新增：网关可能返回 ---
    #[serde(default)]
    pub context_window: Option<u32>,
    #[serde(default)]
    pub max_input_tokens: Option<u32>,
}
```

#### 2.2.7 `get_conversation_token_usage` — Rust 端自动解析

**文件**：`src-tauri/src/cmd/conversation.rs`

当 `context_window_tokens` 为 `None` 时，从对话的 provider + model 自动解析，而非硬编码 `128_000`：

```rust
#[command]
pub fn get_conversation_token_usage(
    app: AppHandle,
    conversation_id: String,
    context_window_tokens: Option<u32>,
) -> Result<serde_json::Value, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    let ctx = if let Some(c) = context_window_tokens {
        c
    } else {
        // 自动解析：conversation → provider → models_json → context_window
        resolve_context_window_for_conversation(&conn, &conversation_id)
    };

    let summary = agent_modes::get_conversation_token_usage(&conn, &conversation_id, ctx)?;
    serde_json::to_value(&summary).map_err(|e| e.to_string())
}

fn resolve_context_window_for_conversation(conn: &Connection, conv_id: &str) -> u32 {
    // 1. 获取对话的 model_id + provider_id
    // 2. 获取 provider 的 models_json
    // 3. 查找 model 的 context_window（与 send_chat_message 相同逻辑）
    // 4. 失败则 fallback 到 get_context_window_tokens(model_id)
}
```

### 2.3 前端改造

#### 2.3.1 chatStore — `switchConversation` 传参修复

**文件**：`src/stores/chatStore.ts`

```ts
// 改造前
invoke("get_conversation_token_usage", { conversationId: id })

// 改造后：从 provider models 中查找 context_window_tokens
const ctxTokens = resolveContextWindowTokens(
  detail.conversation.model_id,
  detail.conversation.provider_id,
);
invoke("get_conversation_token_usage", {
  conversationId: id,
  contextWindowTokens: ctxTokens,
})
```

#### 2.3.2 新增 `resolveContextWindowTokens` 工具函数

**文件**：`src/stores/chatStore.ts`（或提取到 `src/utils/contextWindow.ts`）

```ts
/**
 * 从 provider models_json 中解析当前模型的 context_window_tokens
 * 与 Rust 端 send_chat_message 的两层回退逻辑保持一致
 */
function resolveContextWindowTokens(modelId: string, providerId: string | null): number | undefined {
  if (!providerId) return undefined;
  const provider = useProviderStore.getState().providers.find(p => p.id === providerId);
  if (!provider) return undefined;
  try {
    const models: ModelConfig[] = JSON.parse(provider.models);
    const model = models.find(m => m.id === modelId);
    if (model && model.context_window && model.context_window > 0) {
      return model.context_window;
    }
  } catch {}
  return undefined;
}
```

#### 2.3.3 chatStore — `finishStream` 传参修复

```ts
// 改造前
invoke("get_conversation_token_usage", { conversationId: convId })

// 改造后
const ctxTokens = resolveContextWindowTokens(
  conv?.model_id ?? "",
  conv?.provider_id ?? null,
);
invoke("get_conversation_token_usage", {
  conversationId: convId,
  contextWindowTokens: ctxTokens,
})
```

#### 2.3.4 chatDeltaEvents.ts — 流式 usage 中的 context_window_tokens 已经正确

当前逻辑已正确：Rust 端 `send_chat_message` 计算的 `context_window_tokens` 通过流式 delta 传递到前端。**无需修改**。

---

## 3. 改造任务清单

### 阶段 1 — Rust 端多字段兼容解析

| 任务 | 文件 | 说明 |
|------|------|------|
| 1.1 | `types/config.rs` | 新增 `resolve_context_window()` 函数 |
| 1.2 | `openai/types.rs` | `ModelItem` 新增 `context_window` / `context_length` / `max_input_tokens` 字段 + `resolved_context_window()` 方法 |
| 1.3 | `openai/discovery.rs` | 使用 `resolve_context_window` 替换 `get_context_window_tokens` |
| 1.4 | `gemini/types.rs` | `GeminiModelEntry` 新增 `input_token_limit` 字段 |
| 1.5 | `gemini/discovery.rs` | 使用 `resolve_context_window` 替换 `get_context_window_tokens` |
| 1.6 | `anthropic/types.rs` | `AnthropicModelEntry` 新增 `context_window` / `max_input_tokens` 字段 |
| 1.7 | `anthropic/discovery.rs` | 使用 `resolve_context_window` 替换 `get_context_window_tokens` |

### 阶段 2 — Rust 端 `get_conversation_token_usage` 自动解析

| 任务 | 文件 | 说明 |
|------|------|------|
| 2.1 | `cmd/conversation.rs` | 新增 `resolve_context_window_for_conversation()` 函数 |
| 2.2 | `cmd/conversation.rs` | `get_conversation_token_usage` 中 `None` 时自动解析而非默认 `128_000` |

### 阶段 3 — 前端传参修复

| 任务 | 文件 | 说明 |
|------|------|------|
| 3.1 | `stores/chatStore.ts` | 新增 `resolveContextWindowTokens()` 工具函数 |
| 3.2 | `stores/chatStore.ts` | `switchConversation` 调用 `get_conversation_token_usage` 时传入 `contextWindowTokens` |
| 3.3 | `stores/chatStore.ts` | `finishStream` 调用 `get_conversation_token_usage` 时传入 `contextWindowTokens` |

### 阶段 4 — 验证

| 任务 | 说明 |
|------|------|
| 4.1 | `cargo clippy` + `cargo test` 通过 |
| 4.2 | `npm run typecheck` + `npm run lint` 通过 |
| 4.3 | 手动验证：OpenAI/Claude/Gemini/DeepSeek 模型列表中 `context_window` 正确 |
| 4.4 | 手动验证：切换对话时 ContextBar 显示正确的 max_tokens |
| 4.5 | 手动验证：流式对话结束后 ContextBar 显示正确的 max_tokens |

---

## 4. 数据流改造前后对比

### 改造前

```
GET /models → ModelItem { id, owned_by }
           → context_window = get_context_window_tokens(id)  ← 仅硬编码
           → 存入 models_json

send_chat_message:
  ctx = get_context_window_tokens(model)                     ← 仅硬编码
  if models_json[model].context_window > 0: ctx = 覆盖       ← 覆盖的也是硬编码值
  ChatOptions.context_window_tokens = ctx

get_conversation_token_usage (前端未传参):
  ctx = 128_000                                              ← 永远 128k
```

### 改造后

```
GET /models → ModelItem { id, owned_by, context_window, context_length, max_input_tokens }
           → api_ctx = capacity(context_window, context_length, max_input_tokens)
           → context_window = resolve_context_window(id, api_ctx)  ← API 优先，硬编码兜底
           → 存入 models_json

send_chat_message:
  ctx = get_context_window_tokens(model)                     ← 硬编码
  if models_json[model].context_window > 0: ctx = 覆盖       ← 覆盖的是 API 返回值或硬编码值
  ChatOptions.context_window_tokens = ctx

get_conversation_token_usage (前端传参 or Rust 自动解析):
  前端传: ctx = resolveContextWindowTokens(model, provider)   ← 从 models_json 查找
  前端未传: ctx = resolve_context_window_for_conversation()   ← Rust 自动从 DB 解析
```

---

## 5. API 响应上下文字段参考

| Provider/Gateway | 字段名 | 示例值 | 说明 |
|------------------|--------|--------|------|
| OpenAI 官方 | _(不返回)_ | — | 需硬编码兜底 |
| OpenRouter | `context_length` | 128000 | snake_case |
| Groq | `context_window` | 32768 | snake_case |
| DeepSeek | _(不返回)_ | — | 需硬编码兜底 |
| Anthropic 官方 | _(不返回)_ | — | 需硬编码兜底 |
| Anthropic 网关 | `max_input_tokens` | 200000 | 部分网关返回 |
| Gemini 官方 | `inputTokenLimit` | 1000000 | camelCase，在 model entry 中 |
| Ollama | `context_length` | 4096 | 本地模型 |
| LM Studio | `context_length` | 8192 | 本地模型 |
| 智谱 GLM | _(不返回)_ | — | 需硬编码兜底 |
| 通义千问 | _(不返回)_ | — | 需硬编码兜底 |

---

## 6. 与 I7 Context Compaction 的关系

I7 的 `should_terminate` 依赖准确的 `context_window_tokens`：

```rust
should_terminate(prompt_tokens, context_window):
  prompt_tokens >= context_window * COMPACTION_THRESHOLD_PERCENT / 100
```

如果 `context_window` 不准确（如 Claude 200k 被当成 128k），则：
- **偏小**：过早触发终止（80% of 128k = 102k，而实际 80% of 200k = 160k）
- **偏大**：过晚触发终止，可能已超出 API 限制导致报错

本改造确保 `context_window_tokens` 在所有路径（流式 delta / 切换对话 / 流结束回查）上都是一致的、准确的值，为 I7 提供可靠基础。
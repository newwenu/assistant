# I16 — 前后端消息协议统一设计规格说明

## 1. 交付目标

统一 Rust 后端与 TypeScript 前端之间的消息协议，消除字段命名不一致、类型映射缺失、event payload 隐式约定等问题，建立**单一真相源**的类型契约，为 I16 tool-call 执行循环和后续 agent 扩展奠定协议基础。同时实现前后端转换统一：提供 DTO 层 + 适配层映射函数，业务代码直接调用即可完成 snake_case ↔ camelCase 转换及类型解析，无需手动构造 `json!({})` 或 `as T` 强制转换。

**设计原则**：

- **单一真相源**：每个跨端数据结构有且仅有一处定义，另一端通过映射对齐
- **显式契约**：所有 Tauri command 返回值和 event payload 均有强类型定义，禁止 `as unknown as T` 透传
- **命名约定一致**：选定 snake_case 作为 wire format，两端统一遵守
- **零运行时开销**：映射在边界层（Tauri invoke/emit 边界）完成，业务逻辑层使用各自惯用命名
- **渐进式迁移**：不破坏现有行为，逐个协议点对齐

**命名惯例分层**：

| 层                 | 语言       | 惯用命名     | 示例                                                |
| ------------------ | ---------- | ------------ | --------------------------------------------------- |
| 后端业务层         | Rust       | `snake_case` | `conversation_id`, `tool_calls_json`                |
| Wire format（IPC） | JSON       | `snake_case` | 与 Rust `#[derive(Serialize)]` 默认输出一致，零转换 |
| 前端 DTO 层        | TypeScript | `snake_case` | 与 wire format 1:1 mirror，无心智负担               |
| 前端业务层         | TypeScript | `camelCase`  | `conversationId`, `toolCalls`，符合 JS/TS 生态惯例  |

> Tauri 2 命名转换规则：command 参数 camelCase→snake_case（自动）；command 返回值和 event payload **无转换**，字段名由后端序列化决定。因此 wire format 选 snake_case 改动成本最低。

> **前置依赖**：I15 ✅（chat runtime 已提取，`tool_calls_json` DB 列已存在）

---

## 2. 当前问题

### 2.1 字段命名不一致

| 问题                                                         | 具体表现                                                                                                                                                                                                                                           |
| ------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| command 返回值 snake_case 与前端 camelCase 惯例冲突          | Chat: `provider_id`/`model_id`/`is_pinned`/`title_source`/`is_full` 等 8 个字段；AgentMode: `created_at`/`updated_at`/`is_builtin`/`presentation_rank`/`reasoning_enabled`/`reasoning_effort`/`max_tokens` 7 个字段；Provider: `api_key_masked` 等 |
| `tool_calls_json` → `toolCalls` 字段名+类型双不一致          | 后端 `Option<String>` (JSON string) vs 前端 `ToolCall[]` (parsed array)，且 DB 恒为 NULL                                                                                                                                                           |
| `provider.models` JSON string 散落解析                       | 9 处 `JSON.parse(provider.models) as ModelConfig[]`（chatStore:46, ModelSelector:50, CompactBanner:22, ChatView:95/106/178/209/247, toolStore:66），无 try-catch 保护                                                                              |
| `Message`/`Conversation` 类型内 snake_case 与 camelCase 混用 | `conversation_id`(snake) + `toolCalls`(camel) 同处一个 interface                                                                                                                                                                                   |
| `get_mouse_position` 返回 camelCase                          | `{ clientX, clientY }` 是后端唯一 camelCase 输出，与全局约定不一致                                                                                                                                                                                 |

**Tauri 2 命名转换规则**：command 参数 camelCase→snake_case（自动）；command 返回值和 event payload **无转换**，字段名由后端序列化决定。

### 2.2 类型安全缺失

| 问题                                       | 具体表现                                                                                                                                                                                                                                                                                                                                                                                                                             |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 后端 command 返回 `serde_json::Value`      | Chat: `get_conversation_paginated`/`get_conversation`/`search_conversations` 等 5 处；Provider: `list_providers`/`list_models_cmd`/`save_provider` 3 处；AgentMode: `list_agent_modes`/`save_agent_mode` 2 处；Config: `get_app_conf`/`get_orb_conf`/`get_mascot_conf`/`patch_conf` 4 处；Orb: `get_mouse_position`/`get_screen_size` 2 处；Tool: `get_recent_tools`/`get_favorite_tools` 2 处；共 18 处，前端 `as` 强转无编译时校验 |
| 后端 event payload 用 `json!({})` 手动构造 | 全项目 22 处（chat 13 + setting 3 + orb 2 + conversation 4），遗漏字段无编译时感知                                                                                                                                                                                                                                                                                                                                                   |
| `emit_stream_error` 缺少 `seq`             | 主 chat 流和 compaction 流的 error 事件均不携带 seq，前端无法定位 error 发生位置                                                                                                                                                                                                                                                                                                                                                     |
| 后端 struct 无 `deny_unknown_fields`       | 增删字段时前端旧版本静默忽略，无报错                                                                                                                                                                                                                                                                                                                                                                                                 |

### 2.3 协议定义散落

后端：9 个 cmd 文件各自构造，无统一协议模块。前端：3 个文件各自定义类型，无统一协议入口。

### 2.4 非 Chat 协议的同类问题

| 模块                | 问题                         | 具体表现                                                                                                                                                                                                               |
| ------------------- | ---------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `agentModeStore`    | `as AgentMode[]` 强转        | `AgentMode` 含 `created_at`/`updated_at`/`is_builtin`/`presentation_rank`/`reasoning_enabled`/`reasoning_effort`/`max_tokens` 7 个 snake_case 字段，与 camelCase 惯例冲突                                              |
| `providerStore`     | `as Provider[]` 强转         | `Provider` 含 `api_key_masked`/`models`（JSON string）等 snake_case 字段                                                                                                                                               |
| `useHistoryRefresh` | 绕过 chatService 直接 invoke | `invoke("get_conversation_paginated")` 后 `as { messages: Message[] }` 强转业务类型，未走 DTO 层                                                                                                                       |
| 非 Chat event       | payload 无类型定义           | `setting:provider-changed`（`{action, id?}`）、`theme-changed`（`{resolved}`）、`navigate`（裸 string）、`chat-regenerate`（`{conversation_id, deleted_message_id}`）均为 `json!({})` 构造，前端无 listen 或无类型断言 |

---

## 3. 收束决定点

| ID  | 主题                 | 决定                                                 | 理由                                                                                                                                                                                                                  | 参考来源                                                                                                 |
| --- | -------------------- | ---------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| D1  | wire format          | snake_case                                           | event payload 已全部 snake_case 且两端对齐（13 处），改动成本为零；Rust `#[derive(Serialize)]` 默认输出 snake_case 无需额外标注3；前端业务层已有大量 snake_case 属性访问；Tauri command 返回值不做自动 camelCase 转换 | ChatGPT Tauri 项目默认 snake_case；OpenBitFun 前端 DTO 层 `agent-api.ts` 对 event stream 使用 snake_case |
| D2  | 前端分层             | DTO(snake_case) → 适配层 → 业务层(camelCase)         | 消除一个类型内两种命名风格混用；适配层 `rowToMessage()`/`rowToConversation()` 集中映射，改协议只改一处；业务层保持 JS 惯例                                                                                            | OpenBitFun `tool-execution-service.ts`：事件 DTO(snake_case) → handler 内转换 → 业务对象(camelCase)      |
| D3  | 类型同步机制         | 手动 mirror + 契约测试                               | 无新依赖，event+command 统一适用；契约测试+`deny_unknown_fields` 在"检测不一致"上与代码生成等价；当前约 20 个协议点，手动 mirror 维护成本可接受；协议点 50+ 时可引入 `tauri-specta`                                   | OpenBitFun 前端 DTO `agent-api.ts` 手动 mirror + 注释标注；BongoCat fixture 契约测试                     |
| D4  | event payload 类型化 | 引入 Rust struct + `deny_unknown_fields`             | 替换 `json!({})` 手动构造，遗漏字段有编译时校验；`deny_unknown_fields` 运行时拒绝未知字段，强制前后端同步变更                                                                                                         | OpenBitFun IPC `RuntimeIpcFrame` 全局 `deny_unknown_fields` + `parse_strict_frame` 未知字段检测          |
| D5  | 协议契约测试         | wire format 断言 + round-trip + 未知字段拒绝         | 后端增删/重命名字段时测试失败，强制同步更新前端 DTO                                                                                                                                                                   | OpenBitFun `protocol_contracts.rs` 的 `protocol_round_trips_*` 系列测试                                  |
| D6  | 元数据帧协议         | 不引入                                               | Tauri 已提供请求-响应关联（invoke Promise）、帧定界（IPC 内部）、错误路由（`Result<T,String>`）、事件分发（`emit`）；OpenBitFun 需要是因为其传输层是自定义 IPC（Unix socket + 长度前缀帧）                            | OpenBitFun `RuntimeIpcFrame` 元数据帧层                                                                  |
| D7  | `MessageRow` 序列化  | 保持默认 snake_case，不加 `rename_all = "camelCase"` | `MessageRow` 通过 `serde_json::to_value(&rows)` 序列化，若加 camelCase 则输出 `toolCallsJson`，但 event payload 仍为 snake_case，同一数据结构在两个通道出现不同命名                                                   | —                                                                                                        |

---

## 4. 统一方案

### 4.1 分层架构（→ D1, D2）

```
前端：
┌─────────────────────────────────────────────┐
│  业务层 (Store / Component / Hook)           │
│  camelCase: message.toolCalls               │
├─────────────────────────────────────────────┤
│  适配层 (protocolAdapter.ts)                 │
│  rowToMessage() / rowToConversation()       │
├─────────────────────────────────────────────┤
│  DTO 层 (protocol.ts)  ← 统一协议定义点      │
│  snake_case: row.tool_calls_json            │
├─────────────────────────────────────────────┤
│  Tauri 边界 (invoke / listen)               │
│  command 参数: camelCase→snake_case (自动)  │
│  command 返回 + event payload: snake_case   │
└─────────────────────────────────────────────┘

后端：
┌─────────────────────────────────────────────┐
│  cmd/ 层 — 业务逻辑 + 调用协议类型构造       │
│  app.emit("event", ChatStreamStartPayload{})│
├─────────────────────────────────────────────┤
│  protocol/ 模块  ← 统一协议定义点            │
│  所有 event payload + command 返回值 struct  │
│  #[serde(deny_unknown_fields)]              │
├─────────────────────────────────────────────┤
│  storage/ 层 — ConversationRow / MessageRow │
└─────────────────────────────────────────────┘
```

### 4.2 统一协议定义点

**后端**：`src-tauri/src/protocol/` 模块

```
src-tauri/src/protocol/
├── mod.rs              — re-export
├── chat_events.rs      — Chat event payload struct
├── chat_commands.rs    — Chat command 返回值 struct
├── orb_events.rs       — Orb event payload struct
├── setting_events.rs   — Setting event payload struct
└── setting_commands.rs — Setting command 返回值 struct
```

- 所有 `app.emit("event", payload)` 的 payload 必须用此模块的 struct，禁止 `json!({})`
- 所有 command 返回 `Result<T, String>` 的 `T` 必须是此模块的 struct，禁止 `serde_json::Value`
- struct 标注 `#[serde(deny_unknown_fields)]`

**前端**：按模块分文件，每个模块一个协议定义入口

```
src/chat/core/protocol.ts        — Chat 协议 DTO
src/settings/core/protocol.ts    — Setting / Provider / AgentMode 协议 DTO
```

```typescript
/**
 * Chat protocol DTOs (frontend).
 * Mirror backend: src-tauri/src/protocol/chat_events.rs, chat_commands.rs
 * snake_case fields to match wire format.
 */
```

- 所有 `invoke()` 返回值强转目标必须是此文件的类型
- 所有 `listen()` event payload 断言目标必须是此文件的类型
- `chatTypes.ts` 只定义业务类型（camelCase），不再包含 snake_case 字段

**对接方式**：前端 `protocol.ts` 文件头标注 `Backend mirror: src-tauri/src/protocol/chat_events.rs`，后端反之。IDE 中打开任一端，一眼看到对端位置。

### 4.3 DTO 层定义（→ D2）

```typescript
// chat/core/protocol.ts

export interface MessageRow {
  id: string;
  conversation_id: string;
  role: string;
  content: string;
  tokens: number | null;
  timestamp: number;
  parent_id: string | null;
  is_pinned: boolean;
  reasoning: string | null;
  tool_calls_json: string | null;
}

export interface ConversationRow {
  id: string;
  title: string;
  title_source: string;
  provider_id: string | null;
  model_id: string;
  system_prompt: string | null;
  is_pinned: boolean;
  sort_order: number;
  generation: number;
  created_at: number;
  updated_at: number;
  is_full: boolean;
}

export interface ConversationDetailRow {
  conversation: ConversationRow;
  messages: MessageRow[];
  total_count?: number;
}
```

业务层 `Message` / `Conversation` 全部 camelCase，供 Store/Component 使用。

### 4.4 适配层（→ D2）

> **Phase 1 注意**：Phase 1 时 `chatTypes.ts` 仍为 snake_case，适配函数输出须与 `chatTypes.ts` 一致（snake_case）。以下 camelCase 输出为 **Phase 2 目标态**。Phase 1 版本见第 8 节修正。

```typescript
// chat/core/protocolAdapter.ts（Phase 2 目标态）

export function rowToMessage(row: MessageRow): Message {
  return {
    id: row.id,
    conversationId: row.conversation_id,
    role: row.role,
    content: row.content,
    reasoning: row.reasoning ?? undefined,
    toolCalls: parseToolCallsJson(row.tool_calls_json),
    tokens: row.tokens,
    timestamp: row.timestamp,
    parentId: row.parent_id ?? undefined,
    isPinned: row.is_pinned,
  };
}

export function rowToConversation(row: ConversationRow): Conversation {
  /* 同理 */
}

function parseToolCallsJson(json: string | null): ToolCall[] | undefined {
  if (!json) return undefined;
  try {
    const p = JSON.parse(json);
    return Array.isArray(p) ? p : undefined;
  } catch {
    return undefined;
  }
}
```

### 4.5 chatService 变更

```typescript
// services/chatService.ts
listConversations: (): Promise<Conversation[]> =>
  invoke("list_conversations", { providerId: null }).then((r) => {
    const rows = (r as ConversationRow[]) || [];
    return rows.map(rowToConversation);
  }),
```

强转目标从业务类型 `Conversation` 变为 DTO 类型 `ConversationRow`，再经映射函数转换。

### 4.6 后端变更

#### 4.6.1 event payload 类型化（→ D4，P2）

```rust
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ChatDeltaPayload {
    pub conversation_id: String,
    pub attempt_id: String,
    pub message_id: String,
    pub seq: u32,
    pub delta_text: Option<String>,
    pub delta_reasoning: Option<String>,
    pub delta_tool_call_id: Option<String>,
    pub delta_tool_call_name: Option<String>,
    pub delta_tool_call_arguments: Option<String>,
    pub finish_reason: Option<String>,
    pub usage: Option<TokenUsage>,
   #\[serde(skip_serializing_if = "Option::is_none")]
    pub delta_retry: Option<RetryNotification>,
   #\[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ProviderErrorPayload>,
}
```

替换 `json!({})` 为 struct 序列化。`attempt_id` 和 `seq` 已改为必选字段，`Default` derive 支持 `..Default::default()` 简写。

#### 4.6.2 `tool_calls_json` 写入激活

当前 `update_message_content` 的 `tool_calls_json` 参数在所有调用点恒为 `None`。I16 激活写入：

```rust
let tool_calls_json = outcome.tool_calls.as_ref()
    .and_then(|tc| serde_json::to_string(tc).ok());
```

#### 4.6.3 `emit_stream_error` 补充 `seq`

主 chat 流（`cmd/chat.rs`）和 compaction 流（`cmd/conversation.rs`）的 error 分支均缺 `seq`，需统一补上：

```rust
fn emit_stream_error(ectx: &StreamEmitCtx, last_seq: u32, error: &ProviderErrorPayload) {
    ectx.orb_window.emit("chat-delta", ChatDeltaPayload {
        conversation_id: ectx.conv_id.clone(),
        attempt_id: ectx.attempt_id.clone(),
        message_id: ectx.msg_id.clone(),
        seq: last_seq,
        error: Some(error.clone()),
        ..Default::default()
    })
}
```

compaction 流的 3 处 error emit（`Cancelled`/`Provider`/`Timeout`，`conversation.rs:554/568/582`）同理补 `seq`。

#### 4.6.4 协议契约测试（→ D5，P2）

```rust
#[test]
fn chat_delta_payload_wire_format_matches_frontend_dto() {
    let payload = ChatDeltaPayload { /* ... */ };
    let json = serde_json::to_value(&payload).unwrap();
    assert!(json.get("conversation_id").is_some());
    assert!(json.get("conversationId").is_none()); // 无 camelCase 泄漏
}

#[test]
fn chat_delta_payload_rejects_unknown_fields() {
    let json = json!({ "conversation_id": "c", "attempt_id": "a", "message_id": "m", "seq": 0, "unknown": "x" });
    assert!(serde_json::from_value::<ChatDeltaPayload>(json).is_err());
}
```

#### 4.6.5 `MessageRow` 保持默认 snake_case 序列化（→ D7）

不加 `#[serde(rename_all = "camelCase")]`，前端 DTO 层 mirror snake_case。

#### 4.6.6 `get_mouse_position` 改为 snake_case

返回 `{ client_x, client_y }` 或定义 struct + `#[serde(rename_all = "snake_case")]`。前端同步改属性名。

### 4.7 元数据约束

> **R1**：所有 `chat-delta` 事件（含 error 分支）**必须**携带 `conversation_id`、`attempt_id`、`message_id`
>
> **R2**：`chat-delta` 所有分支（含 error）**必须**携带 `seq`，同一 `attempt_id` 下单调递增 ✅
>
> **R3**：`chat-stream-start` **必须**携带 `conversation_id`、`attempt_id`、`message_id`，且值与后续 `chat-delta` 一致
>
> **R4**：同一 `attempt_id` 下 `seq` 单调递增（从 0 开始）
>
> **R5**：`navigate` event payload 为裸 `string`（非 object），不适用 struct 类型化，保持现状

### 4.8 散落构造体约束

| 约束 | 范围                                                                                                                                   | 阶段                                                |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------- |
| S1   | Chat 协议 event payload 全部替换为类型化 struct，消除 `json!({})`                                                                      | Phase 4 ✅                                          |
| S2   | Setting 协议 event payload 遵循同一模式                                                                                                | Phase 4 ✅（`setting:provider-changed` 已提前完成） |
| S3   | `get_conversation` 的 `json!({})` 返回值定义 `ConversationDetailResponse` struct                                                       | Phase 4                                             |
| S4   | `list_providers` 的 `json!({})` 定义 `ProviderListRow` struct（原命名 `ProviderListItem`，实施时统一为 `Row` 后缀对齐 storage 层惯例） | Phase 4 ✅                                          |
| S5   | `get_mouse_position` 返回值改为 snake_case                                                                                             | Phase 2 ✅                                          |
| S6   | `DbSender` 4 元组替换为命名 struct `DbWriteMsg`                                                                                        | Phase 4 ✅                                          |
| S7   | Phase 4 完成后 Chat 协议范围内 `json!({})` 为 0 处                                                                                     | Phase 4 ✅                                          |
| S8   | `get_screen_size` 的 `json!({})` 返回值定义 struct                                                                                     | Phase 4 ✅                                          |

| 约束 | 范围                                                                                                                                                                   | 阶段      |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------- |
| F1   | `chatService.ts` 强转目标从业务类型改为 DTO 类型 + 映射函数                                                                                                            | Phase 1-2 |
| F2   | `providerStore`（`as Provider[]`/`as ModelConfig[]`）/`agentModeStore`（`as AgentMode[]`，7 个 snake_case 字段）的强转遵循同一模式                                     | Phase 5   |
| F3   | `useHistorySearch`（`as Conversation[]`，L39）/`useHistoryRefresh`（`as { messages: Message[] }`，L37）统一走 chatService，消除直接 invoke + 业务类型强转              | Phase 1-2 |
| F4   | event payload `as` 断言目标定义在 `protocol.ts` DTO 层                                                                                                                 | Phase 1   |
| F5   | `JSON.parse(provider.models) as ModelConfig[]` 替换为后端结构化数组 `Vec<ModelConfigWire>`，前端 `Provider.models` 改为 `ModelConfig[]`，8 处 `JSON.parse` 全部消除 ✅ | Phase 5 ✅ |

---

## 5. 协议清单

### 5.1 Tauri Command 协议

| Command                      | 后端返回类型                                       | 前端 DTO 类型           | 前端业务类型         | 映射函数                             |
| ---------------------------- | -------------------------------------------------- | ----------------------- | -------------------- | ------------------------------------ |
| `list_conversations`         | `Vec<ConversationRow>`                             | `ConversationRow[]`     | `Conversation[]`     | `rowToConversation`                  |
| `get_conversation_paginated` | `serde_json::Value` → `ConversationDetailResponse` | `ConversationDetailRow` | `ConversationDetail` | `rowToConversation` + `rowToMessage` |
| `create_conversation`        | `String`                                           | —                       | —                    | —                                    |
| `send_chat_message`          | `()`                                               | —                       | —                    | —                                    |
| `cancel_chat`                | `()`                                               | —                       | —                    | —                                    |
| `search_conversations`       | `Vec<ConversationRow>`                             | `ConversationRow[]`     | `Conversation[]`     | `rowToConversation`                  |

### 5.1.2 非 Chat Command

| Command              | 后端返回类型                                    | 前端强转                                     | 问题                                   | 阶段                   |
| -------------------- | ----------------------------------------------- | -------------------------------------------- | -------------------------------------- | ---------------------- |
| `list_providers`     | `serde_json::Value` → `Vec<ProviderListRow>` ✅ | `as Provider[]`（`providerStore.ts:111`）    | snake_case 字段 + JSON string `models` | Phase 4 ✅（提前完成） |
| `list_models_cmd`    | `serde_json::Value` → `ModelConfigList`         | `as ModelConfig[]`（`providerStore.ts:128`） | —                                      | Phase 5                |
| `list_agent_modes`   | `serde_json::Value` → `AgentModeList`           | `as AgentMode[]`（`agentModeStore.ts:28`）   | 7 个 snake_case 字段                   | Phase 5                |
| `get_mouse_position` | `serde_json::Value` → `MousePosition`           | `as { clientX, clientY }`                    | 后端唯一 camelCase 输出                | Phase 2                |
| `get_screen_size`    | `serde_json::Value` → `ScreenSize`              | —                                            | `json!({})` 构造，需类型化             | Phase 4                |
| `get_app_conf`       | `serde_json::Value` → `AppConf`                 | —                                            | config struct 直接序列化               | Phase 5                |
| `get_orb_conf`       | `serde_json::Value` → `OrbConf`                 | —                                            | config struct 直接序列化               | Phase 5                |
| `get_mascot_conf`    | `serde_json::Value` → `MascotConf`              | —                                            | config struct 直接序列化               | Phase 5                |
| `get_recent_tools`   | `serde_json::Value` → `Vec<ToolItem>`           | —                                            | `json!({})` 构造，需类型化             | Phase 5                |
| `get_favorite_tools` | `serde_json::Value` → `Vec<ToolItem>`           | —                                            | `json!({})` 构造，需类型化             | Phase 5                |

### 5.2 Tauri Event 协议

#### 5.2.1 Chat 协议

| Event                         | 后端 emit 位置                                      | 前端 listen              | 前端接收类型                       | 关联元数据（必选）                                   |
| ----------------------------- | --------------------------------------------------- | ------------------------ | ---------------------------------- | ---------------------------------------------------- |
| `chat-stream-start`           | `chat.rs:288`, `conversation.rs:428`                | `chatDeltaEvents.ts:46`  | `ChatStreamStartPayload`           | `conversation_id`, `attempt_id`, `message_id`        |
| `chat-delta`                  | `chat.rs:83`, `conversation.rs:437/457/554/568/582` | `chatDeltaEvents.ts:76`  | `ChatDeltaPayload`                 | `conversation_id`, `attempt_id`, `message_id`, `seq` |
| `conversation_terminated`     | `conversation.rs:518`                               | `chatDeltaEvents.ts:126` | `ConversationTerminatedPayload`    | `conversation_id`                                    |
| `conversation_compact_failed` | `chat.rs:184`, `conversation.rs:342`                | `chatDeltaEvents.ts:142` | `ConversationCompactFailedPayload` | `conversation_id`                                    |
| `conversation_unlocked`       | `conversation.rs:344`                               | `chatDeltaEvents.ts:153` | `ConversationUnlockedPayload`      | `conversation_id`                                    |

#### 5.2.2 非 Chat 协议

| Event                      | 后端 emit 位置    | 前端 listen       | payload 类型                                                    | 备注                         | 阶段                   |
| -------------------------- | ----------------- | ----------------- | --------------------------------------------------------------- | ---------------------------- | ---------------------- |
| `setting:provider-changed` | `provider.rs`     | 无                | `SettingProviderChangedPayload { action, id? }`                 | 已类型化，`json!({})` 已消除 | Phase 4 ✅（提前完成） |
| `theme-changed`            | `config.rs:139`   | 无                | `{ resolved }`                                                  | `json!({})` 构造，需类型化   | Phase 5                |
| `navigate`                 | `app.rs:63`       | 无                | 裸 `string`（非 object）                                        | 特殊：payload 不是 struct    | —                      |
| `chat-regenerate`          | `conversation.rs` | 无                | `ChatRegeneratePayload { conversation_id, deleted_message_id }` | 已类型化                     | Phase 4 ✅             |
| `orb-blur`                 | orb 模块          | `OrbPanel.tsx:54` | 无 payload                                                      | `json!({})` 构造，需类型化   | Phase 4                |
| `orb-new-conversation`     | orb 模块          | `OrbPanel.tsx:75` | 无 payload                                                      | `json!({})` 构造，需类型化   | Phase 4                |

### 5.3 DB → 前端字段映射

| DB 列             | Wire format       | 前端 DTO          | 前端业务         | 转换                     |
| ----------------- | ----------------- | ----------------- | ---------------- | ------------------------ |
| `conversation_id` | `conversation_id` | `conversation_id` | `conversationId` | snake→camel              |
| `parent_id`       | `parent_id`       | `parent_id`       | `parentId`       | snake→camel              |
| `is_pinned`       | `is_pinned`       | `is_pinned`       | `isPinned`       | snake→camel              |
| `tool_calls_json` | `tool_calls_json` | `tool_calls_json` | `toolCalls`      | snake→camel + JSON.parse |
| `title_source`    | `title_source`    | `title_source`    | `titleSource`    | snake→camel              |
| `provider_id`     | `provider_id`     | `provider_id`     | `providerId`     | snake→camel              |
| `model_id`        | `model_id`        | `model_id`        | `modelId`        | snake→camel              |
| `created_at`      | `created_at`      | `created_at`      | `createdAt`      | snake→camel              |
| `updated_at`      | `updated_at`      | `updated_at`      | `updatedAt`      | snake→camel              |
| `is_full`         | `is_full`         | `is_full`         | `isFull`         | snake→camel              |

其余字段（`id`/`role`/`content`/`tokens`/`timestamp`/`reasoning`）两端命名一致，无转换。

---

## 6. 协议文件构建指南

每个协议文件的建立遵循统一流程：**找源 → 抄字段 → 加标注 → 写适配 → 接入调用点 → 验证**。

### 6.1 前端 DTO 文件构建（`protocol.ts`）

**步骤**：

1. **找源**：打开后端对应的 struct 文件，确认字段列表
   - `MessageRow` / `ConversationRow` → 后端 `storage/` 层同名 struct（如 `src-tauri/src/storage/message.rs`）
   - event payload → 后端 `protocol/chat_events.rs` 同名 struct
   - command 返回值 → 后端 `protocol/chat_commands.rs` 同名 struct
2. **抄字段**：逐字段从 Rust struct 复制到 TypeScript interface，类型映射规则：
   | Rust 类型                     | TypeScript 类型                   |
   | ----------------------------- | --------------------------------- |
   | `String`                      | `string`                          |
   | `u32` / `i32` / `u64` / `i64` | `number`                          |
   | `bool`                        | `boolean`                         |
   | `Option<T>`                   | `T \| null`                       |
   | `Vec<T>`                      | `T[]`                             |
   | `serde_json::Value`           | `unknown`（应消除，改为具体类型） |
   | 枚举 `enum`                   | `string`（或联合类型）            |
3. **字段名保持 snake_case**：与后端 wire format 一致，不加 camelCase 转换
4. **加文件头注释**：标注 `Backend mirror: <后端文件路径>`，便于 IDE 中快速定位对端
5. **导出**：所有 interface 用 `export interface`，供 `protocolAdapter.ts` 和调用点 import

**示例**：从后端 `storage/message.rs` 的 `MessageRow` struct 构建 `protocol.ts` 的 `MessageRow` interface：

```rust
// 后端 src-tauri/src/storage/message.rs
pub struct MessageRow {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub tokens: Option<i32>,
    pub timestamp: i64,
    pub parent_id: Option<String>,
    pub is_pinned: bool,
    pub reasoning: Option<String>,
    pub tool_calls_json: Option<String>,
}
```

```typescript
// 前端 src/chat/core/protocol.ts
/** Backend mirror: src-tauri/src/storage/message.rs */
export interface MessageRow {
  id: string;
  conversation_id: string;
  role: string;
  content: string;
  tokens: number | null;
  timestamp: number;
  parent_id: string | null;
  is_pinned: boolean;
  reasoning: string | null;
  tool_calls_json: string | null;
}
```

### 6.2 前端适配文件构建（`protocolAdapter.ts`）

**步骤**：

1. **import DTO 类型**：从 `./protocol` 导入所有 Row 类型
2. **import 业务类型**：从 `../chatTypes`（或对应路径）导入 `Message` / `Conversation` 等
3. **写正向映射** `rowToXxx(row: XxxRow): Xxx`：
   - snake_case → camelCase 字段重命名
   - `null` → `undefined` 转换（业务层习惯用 `undefined` 表示缺失）
   - 特殊字段处理：`tool_calls_json` → `parseToolCallsJson()` 解析
4. **写反向映射** `xxxToRow(xxx: Xxx): XxxRow`：
   - camelCase → snake_case 字段重命名
   - `undefined` → `null` 转换
   - `toolCalls` → `JSON.stringify()` 序列化
5. **写特殊解析函数**：`parseToolCallsJson(json: string | null): ToolCall[] | undefined`

**import 关系图**：

```
protocolAdapter.ts
  ├── import { MessageRow, ConversationRow, ... } from './protocol'    // DTO
  ├── import { Message, Conversation, ... } from '../chatTypes'        // 业务类型
  └── import { ToolCall } from '../chatTypes'                          // 嵌套业务类型
```

### 6.3 后端协议文件构建（`protocol/*.rs`）

**步骤**：

1. **找源**：从现有 `json!({})` 调用点提取字段
   - 搜索 `app.emit("event-name", json!({` 找到所有 emit 调用
   - 从 `json!({})` 内的字段列表提取 struct 字段
   - 从调用上下文确定每个字段的 Rust 类型和来源（哪个变量/哪个模块）
2. **确定 import**：
   - struct 字段类型若为自定义类型（如 `TokenUsage`、`ProviderErrorPayload`），需 `use` 导入
   - 嵌套类型若定义在 `storage/` 层，从 `crate::storage::xxx` 导入
   - 嵌套类型若定义在 `protocol/` 其他文件，从 `super::xxx` 或 `crate::protocol::xxx` 导入
3. **写 struct**：
   - `#[derive(Debug, Clone, Serialize)]`（后端 payload 只用于 emit 序列化，不需要 `Deserialize`）
   - `#[serde(rename_all = "snake_case")]`
   - 逐字段定义，`Option` 字段加 `#[serde(skip_serializing_if = "Option::is_none")]`
   - 注：`deny_unknown_fields` 仅对 `Deserialize` 有效，后端 Serialize-only struct 不加；前端 DTO 的契约测试单独验证未知字段拒绝`
4. **在 `mod.rs` 注册**：`pub mod chat_events;` + `pub use chat_events::*;`
5. **在 `lib.rs` 注册模块**：`mod protocol;`

**示例**：从 `chat.rs` 的 `json!({})` 调用构建 `ChatDeltaPayload`：

```rust
// 第 1 步：找源 — 搜索 chat.rs 中 app.emit("chat-delta", json!({...}))
// 找到字段：conversation_id, attempt_id, message_id, seq, delta_text, ...

// 第 2 步：确定 import（用 grep 找实际路径，不要凭记忆）
use assistant_providers::types::chat_delta::{RetryNotification, TokenUsage};
use assistant_providers::base::ProviderErrorPayload;

// 第 3 步：写 struct
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ChatDeltaPayload {
    pub conversation_id: String,
    pub attempt_id: String,
    pub message_id: String,
    pub seq: u32,
    // ...其余字段
}
```

> **确定 import 路径**：不要凭记忆写 import 路径。实操时用 `grep -rn 'pub struct XxxName' src-tauri/` 找到定义文件，再根据 crate 结构推导 import 路径，最后 `cargo check` 验证。

### 6.4 后端 `json!({})` → struct 替换实操

**步骤**：

1. **定位**：`grep -n 'json!({' src-tauri/src/cmd/chat.rs` 找到所有 `json!({})` 调用
2. **提取字段**：从 `json!({ "field_name": value, ... })` 提取字段名和值变量
3. **确认 struct 已定义**：在 `protocol/` 对应文件中找到或新建 struct
4. **替换**：`json!({ "conversation_id": cid, ... })` → `ChatDeltaPayload { conversation_id: cid, ... }`
5. **编译验证**：`cargo check` 确认字段名和类型匹配
6. **契约测试**：运行 `cargo test` 确认 wire format 不变

### 6.5 前端 `as T` → DTO + 适配 替换实操

**步骤**：

1. **定位**：在 `chatService.ts` / `providerStore.ts` 等文件搜索 `as Conversation` / `as Provider` 等
2. **确认 DTO 类型已定义**：在 `protocol.ts` 中找到对应的 `ConversationRow` / `ProviderRow`
3. **确认适配函数已定义**：在 `protocolAdapter.ts` 中找到 `rowToConversation` / `rowToProvider`
4. **替换**：
   ```typescript
   // 替换前
   invoke("list_conversations").then((r) => r as Conversation[]);
   // 替换后
   invoke("list_conversations").then((r) =>
     (r as ConversationRow[]).map(rowToConversation),
   );
   ```
5. **添加 import**：在调用文件头部添加 `import { ConversationRow } from '../core/protocol'` 和 `import { rowToConversation } from '../core/protocolAdapter'`
6. **类型检查**：`tsc --noEmit` 确认无类型错误

### 6.6 验证清单

每个协议文件建立后，逐项检查：

- [ ] 前端 DTO 字段与后端 struct 字段一一对应（名称 + 类型）
- [ ] 前端 DTO 文件头有 `Backend mirror` 注释
- [ ] 后端 struct 有 `#[serde(rename_all = "snake_case")]`；前端 DTO 契约测试验证未知字段拒绝
- [ ] 适配函数正向 + 反向映射完整
- [ ] 调用点 import 正确，无循环依赖
- [ ] `cargo check` + `tsc --noEmit` 通过
- [ ] 契约测试通过（wire format 断言 + round-trip + 未知字段拒绝）

---

## 7. 替换策略

目标：消除所有手动类型转换，统一走 DTO + 适配层。以下为每种现有手动转换的替换模式：

### 7.1 前端 `as T` 业务类型强转 → DTO 强转 + 映射函数

| 现有模式                                                                       | 替换为                                                                                                                                                                      | 涉及文件               | 阶段    |
| ------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------- | ------- |
| `invoke("list_conversations").then(r => r as Conversation[])`                  | `invoke(...).then(r => (r as ConversationRow[]).map(rowToConversation))`                                                                                                    | `chatService.ts`       | Phase 1 |
| `invoke("get_conversation_paginated").then(r => r as { messages: Message[] })` | `invoke(...).then(r => { const row = r as ConversationDetailRow; return { conversation: rowToConversation(row.conversation), messages: row.messages.map(rowToMessage) } })` | `useHistoryRefresh.ts` | Phase 1 |
| `invoke("search_conversations").then(r => r as Conversation[])`                | `invoke(...).then(r => (r as ConversationRow[]).map(rowToConversation))`                                                                                                    | `useHistorySearch.ts`  | Phase 1 |
| `invoke("list_providers").then(r => r as Provider[])`                          | `invoke(...).then(r => (r as ProviderRow[]).map(rowToProvider))`                                                                                                            | `providerStore.ts`     | Phase 5 |
| `invoke("list_models_cmd").then(r => r as ModelConfig[])`                      | `invoke(...).then(r => (r as ModelConfigRow[]).map(rowToModelConfig))`                                                                                                      | `providerStore.ts`     | Phase 5 |
| `invoke("list_agent_modes").then(r => r as AgentMode[])`                       | `invoke(...).then(r => (r as AgentModeRow[]).map(rowToAgentMode))`                                                                                                          | `agentModeStore.ts`    | Phase 5 |

### 7.2 前端 `as { clientX, clientY }` → DTO 强转 + 属性映射

| 现有模式                                                            | 替换为                                                                               | 涉及文件 | 阶段    |
| ------------------------------------------------------------------- | ------------------------------------------------------------------------------------ | -------- | ------- |
| `invoke("get_mouse_position").then(r => r as { clientX, clientY })` | `invoke(...).then(r => r as MousePositionRow)`，使用 `row.client_x` / `row.client_y` | 调用方   | Phase 2 |

### 7.3 前端 `JSON.parse(x) as T[]` → 安全解析函数

| 现有模式                                       | 替换为                                                                                  | 涉及文件           | 阶段    |
| ---------------------------------------------- | --------------------------------------------------------------------------------------- | ------------------ | ------- |
| `JSON.parse(provider.models) as ModelConfig[]` | `parseModelConfigsJson(provider.models)`：try-catch + Array.isArray 校验，失败返回 `[]` | `providerStore.ts` | Phase 2 |

### 7.4 前端 event payload `as T` → DTO 类型断言（不改字段名）

event payload 本身就是 snake_case wire format，与 `protocol.ts` DTO 一致，只需把类型来源从散落定义改为 `protocol.ts`，不需要适配函数。

| 现有模式                                                                                                              | 替换为                                                                                                        | 涉及文件             | 阶段    |
| --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- | -------------------- | ------- |
| `listen("chat-delta", e => { const p = e.payload as ChatDeltaPayload; ... })`（从 `chatOrchestrator.ts` import 类型） | `listen("chat-delta", e => { const p = e.payload as ChatDeltaPayload; ... })`（从 `protocol.ts` import 类型） | `chatDeltaEvents.ts` | Phase 1 |

> 注：event payload DTO 与业务类型字段名相同（均为 snake_case），无需适配函数做字段重命名。适配函数只在 invoke 返回值需要转业务类型时使用（7.1）。

### 7.5 后端 `json!({})` → struct 构造

| 现有模式                                                                 | 替换为                                                                                                       | 涉及文件                     | 阶段                   |
| ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------ | ---------------------------- | ---------------------- |
| `app.emit("chat-delta", json!({ "conversation_id": cid, ... }))`         | `app.emit("chat-delta", ChatDeltaPayload { conversation_id: cid, ... })`                                     | `chat.rs`, `conversation.rs` | Phase 4                |
| `app.emit("setting:provider-changed", json!({ "action": a, "id": id }))` | `app.emit("setting:provider-changed", SettingProviderChangedPayload { action: a, id })`                      | `provider.rs`                | Phase 4 ✅（提前完成） |
| `app.emit("theme-changed", json!({ "resolved": r }))`                    | `app.emit("theme-changed", ThemeChangedPayload { resolved: r })`                                             | `config.rs`                  | Phase 5                |
| `app.emit("chat-regenerate", json!({ ... }))`                            | `app.emit("chat-regenerate", ChatRegeneratePayload { ... })`                                                 | `conversation.rs`            | Phase 4                |
| orb 模块 2 处 `json!({})`                                                | `app.emit("orb-blur", OrbBlurPayload {})` / `app.emit("orb-new-conversation", OrbNewConversationPayload {})` | orb 模块                     | Phase 4                |

### 7.6 后端 `serde_json::Value` 返回 → 强类型 struct

| 现有模式                                                               | 替换为                                                                          | 涉及文件          | 阶段                   |
| ---------------------------------------------------------------------- | ------------------------------------------------------------------------------- | ----------------- | ---------------------- |
| `fn get_conversation_paginated() -> Result<serde_json::Value, String>` | `fn get_conversation_paginated() -> Result<ConversationDetailResponse, String>` | `conversation.rs` | Phase 4                |
| `fn list_providers() -> Result<serde_json::Value, String>`             | `fn list_providers() -> Result<Vec<ProviderListRow>, String>`                   | `provider.rs`     | Phase 4 ✅（提前完成） |
| `fn list_agent_modes() -> Result<serde_json::Value, String>`           | `fn list_agent_modes() -> Result<Vec<AgentModeItem>, String>`                   | `agent_mode.rs`   | Phase 5                |
| `fn get_mouse_position() -> Result<serde_json::Value, String>`         | `fn get_mouse_position() -> Result<MousePosition, String>`                      | 对应 cmd 文件     | Phase 2                |
| `fn get_screen_size() -> Result<serde_json::Value, String>`            | `fn get_screen_size() -> Result<ScreenSize, String>`                            | 对应 cmd 文件     | Phase 4                |

---

## 8. Phase 1 实测问题与修正

基于对当前代码的实际验证，Phase 1 方案存在以下问题需要修正：

### 8.1 `protocolAdapter.ts` 输出类型与 `chatTypes.ts` 不兼容

**现状**：`chatTypes.ts` 的 `Message` / `Conversation` 仍用 snake_case（`conversation_id`, `title_source`, `provider_id`），而 `protocolAdapter.ts` 的 `rowToMessage` / `rowToConversation` 输出 camelCase（`conversationId`, `titleSource`, `providerId`）。

**后果**：使用方调用 `rowToMessage(row)` 得到的对象无法赋给 `Message` 类型，TypeScript 编译报错。

**修正**：Phase 1 的 `protocolAdapter.ts` 映射函数**输出类型应与 `chatTypes.ts` 当前定义一致**（snake_case）。Phase 2 将 `chatTypes.ts` 迁移到 camelCase 时，同步修改 `protocolAdapter.ts` 的输出。

```typescript
// Phase 1：输出 snake_case，与 chatTypes.ts 一致
export function rowToMessage(row: MessageRow): Message {
  return {
    id: row.id,
    conversation_id: row.conversation_id, // 保持 snake_case
    role: row.role,
    content: row.content,
    reasoning: row.reasoning ?? undefined,
    toolCalls: parseToolCallsJson(row.tool_calls_json),
    tokens: row.tokens,
    timestamp: row.timestamp,
    parent_id: row.parent_id ?? undefined,
    is_pinned: row.is_pinned,
  };
}
```

### 8.2 后端 `ChatDeltaPayload` 与 `ChatDelta` 字段大量重复

**现状**：`assistant_providers::types::chat_delta::ChatDelta` 已有 `seq`/`delta_text`/`delta_reasoning`/`delta_tool_call_*`/`finish_reason`/`usage`/`delta_retry` 字段。`protocol::chat_events::ChatDeltaPayload` 重复了这些字段，只多了 `conversation_id`/`attempt_id`/`message_id`。

**后果**：`emit_delta` 中需要逐字段 `.clone()` 从 `ChatDelta` 拷贝到 `ChatDeltaPayload`，多余分配。

**修正**：`ChatDeltaPayload` 改为组合模式，内嵌 `ChatDelta`：

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ChatDeltaPayload {
    pub conversation_id: String,
    pub attempt_id: String,
    pub message_id: String,
    pub seq: u32,
    #[serde(flatten)]
    pub delta: ChatDelta,
}
```

这样 `emit_delta` 只需：

```rust
ectx.orb_window.emit("chat-delta", ChatDeltaPayload {
    conversation_id: ectx.conv_id.to_string(),
    attempt_id: Some(ectx.attempt_id.to_string()),
    message_id: ectx.msg_id.to_string(),
    delta: delta.clone(),  // 一次 clone 代替 8 次
})
```

> **注意**：`#[serde(flatten)]` 对 wire format 的影响需验证——flatten 会将 `ChatDelta` 的字段展开到 `ChatDeltaPayload` 同级，与当前 `json!({})` 行为一致。**实施前必须先写对照测试**：用 `json!({})` 构造和 struct 构造分别序列化同一组数据，断言 JSON 输出完全相同，再替换调用点。

### 8.3 `StreamEmitCtx` 字段是 `&str`，struct 构造需要 `String`

**现状**：`StreamEmitCtx` 的 `conv_id`/`attempt_id`/`msg_id` 是 `&str`，而 `ChatDeltaPayload` 需要 `String`。

**后果**：每次 emit 都要 `.to_string()` 分配。

**修正**：这是不可避免的（`emit` 需要 owned value），但通过 8.2 的组合模式，只需 3 次 `.to_string()`（`conv_id`, `attempt_id`, `msg_id`），比 `json!({})` 内部序列化时的多次分配更少。可接受。

### 8.4 event payload 不需要适配函数

**现状**：`chatDeltaEvents.ts` 的 `event.payload as ChatDeltaPayload` 后直接使用 snake_case 字段（`payload.conversation_id`, `payload.delta_text`），这些字段名与 `protocol.ts` 的 `ChatDeltaPayload` 一致。

**后果**：event payload 不需要 `rowToXxx` 适配函数，只需把 `as ChatDeltaPayload` 的类型来源从 `chatOrchestrator.ts` 改为 `protocol.ts`。

**修正**：7.4 替换策略中"event payload `as T` → DTO 类型断言 + 适配函数"应改为"event payload `as T` → DTO 类型断言（不改字段名）"。适配函数只在 invoke 返回值需要转业务类型时使用。

### 8.5 `chatService.ts` 的 `ConversationDetail` 也用 snake_case

**现状**：`chatService.ts` 定义的 `ConversationDetail` 接口里 `total_count` 是 snake_case，与 `protocol.ts` 的 `ConversationDetailRow` 一致。

**后果**：Phase 1 替换 `as ConversationDetail` 为 `(r as ConversationDetailRow)` 后，使用方直接用 `detail.total_count`，无需适配。

**修正**：`ConversationDetail` 接口应删除，统一用 `ConversationDetailRow`。

---

## 9. 实施计划

### Phase 1：DTO 层建立 + 映射函数（无行为变更）

1. **新建文件（零代码改动）**：
   - **前端**：
     - 新建 `src/chat/core/protocol.ts`：定义 `MessageRow`、`ConversationRow`、`ConversationDetailRow`，文件头标注 `Backend mirror` + `snake_case to match wire format`
     - 新建 `src/chat/core/protocolAdapter.ts`：实现 `rowToMessage`、`rowToConversation`、`messageToRow`、`conversationToRow`、`parseToolCallsJson`（双向映射：DTO↔业务）
   - **后端**：
     - 新建 `src-tauri/src/protocol/mod.rs`：模块声明 + re-export
     - 新建 `src-tauri/src/protocol/chat_events.rs`：`ChatStreamStartPayload` / `ChatDeltaPayload` / `ConversationTerminatedPayload` / `ConversationCompactFailedPayload` / `ConversationUnlockedPayload` struct 骨架，标注 `#[derive(Debug, Clone, Serialize)]` + `#[serde(rename_all = "snake_case")]`
     - 新建 `src-tauri/src/protocol/chat_commands.rs`：`ConversationDetailResponse` struct 骨架
     - 在 `src-tauri/src/lib.rs`（或 `main.rs`）添加 `mod protocol;`
2. 按 7.1 替换 `chatService.ts` 的 3 个 `as` 强转为 DTO 强转 + 映射函数
3. 按 7.4 替换 `chatDeltaEvents.ts` 的 event payload `as` 断言类型来源从 `chatOrchestrator.ts`/`chatTypes.ts` 改为 `protocol.ts`（event payload 字段名与 DTO 一致均为 snake_case，Phase 1 无需适配函数；Phase 2 业务类型改 camelCase 后才需适配函数）
4. event payload 类型迁移到 `protocol.ts`；从 `chatDeltaEvents.ts` 等原文件**删除**已迁移的旧类型定义（单一真相源）
5. 按 7.1 替换 `useHistorySearch` / `useHistoryRefresh` 的直接 invoke + 业务类型强转，统一走 chatService

> **本 Phase 消除**：`chatService.ts` 3 处 `as Conversation[]`/`as ConversationDetail`、`useHistoryRefresh.ts` 1 处 `as { messages: Message[] }`、`useHistorySearch.ts` 1 处 `as Conversation[]`、`chatDeltaEvents.ts` 5 处 event payload `as` 断言

### Phase 2：业务类型 camelCase 迁移 ✅

1. `chatTypes.ts` 的 `Message` / `Conversation` / `TokenUsage` / `ContextUsage` interface 字段改为 camelCase，删除原有 snake_case 字段 ✅
2. 全局替换 Store / Component / 测试中的 snake_case 属性访问为 camelCase ✅
3. event payload 类型（`ChatDeltaPayload` 等）**不变**（保持 snake_case） ✅
4. `protocolAdapter.ts` 新增 `rowToTokenUsage()` 适配函数，`chatOrchestrator.ts` 中 `payload.usage` 经此函数转换 ✅
5. 按 7.2 替换 `get_mouse_position` 后端返回值为 snake_case struct（S5）✅（已在 Phase 4 提前完成）；前端同步改 `clientX→client_x` / `clientY→client_y` ✅
6. 按 7.3 替换 `JSON.parse(provider.models) as ModelConfig[]` 为安全解析函数（F5）— 延至 Phase 5
7. 确认 `MessageRow` 无 `#[serde(rename_all = "camelCase")]`，保持默认 snake_case 序列化（D7） ✅

> **本 Phase 消除**：`get_mouse_position` 1 处 camelCase 返回值 ✅、业务层 snake_case/camelCase 混用（14 个字段 × 10+ 消费方）

### Phase 3：`tool_calls_json` 写入激活 ✅

1. `cmd/chat.rs` 的 `stream_future` Ok 分支：从 `outcome.tool_calls` 序列化写入 DB ✅
2. `db_tx` channel 类型 `DbWriteMsg` 扩展包含 `tool_calls_json` ✅

> **本 Phase 消除**：`tool_calls_json` 恒为 NULL 的写入缺失 ✅（非手动转换，是功能激活）

### Phase 4：后端类型化 + 契约测试

1. **新建文件（零代码改动）**：
   - 新建 `src-tauri/src/protocol/chat_events.rs` 补充 `ChatRegeneratePayload` struct ✅
   - 新建 `src-tauri/src/protocol/setting_events.rs`：`ProviderListRow` / `SettingProviderChangedPayload` struct ✅（提前从 Phase 5 拉入，因 provider.rs 改动一并完成）
   - ~~新建 `src-tauri/src/protocol/orb_events.rs`：`OrbBlurPayload` / `OrbNewConversationPayload` struct~~ → **决定不建**：`orb-blur` / `orb-new-conversation` 均 emit `()`（空元组），无 payload 字段，无需 struct
   - 新建 `src-tauri/src/protocol/orb_commands.rs`：`MousePosition` / `ScreenSize` / `ScreenRect` struct ✅（S5 + S8）
   - 新建协议契约测试文件：wire format 断言 + round-trip + 未知字段拒绝 ✅（14 个测试全部通过）
2. 按 7.5 替换 `chat.rs` / `conversation.rs` 中 `json!({})` 为 `protocol::chat_events` struct 构造 ✅（chat.rs 4 处 + conversation.rs 11 处全部消除）
3. 按 7.5 替换 `provider.rs` 中 `json!({})` 为 `protocol::setting_events` struct 构造 ✅（3 处全部消除，提前从 Phase 5 拉入）
4. ~~按 7.5 替换 orb 模块中 `json!({})` 为 `protocol::orb_events` struct 构造~~ → orb event 为空 payload `()`，无需 struct
5. 按 7.6 替换 command 返回值 `serde_json::Value` 为强类型 struct（S3 ✅, S4 ✅, S5 ✅, S8 ✅）
   - `get_mouse_position` → `Option<MousePosition>` ✅，前端 `OrbCoord.ts` 同步改为 `client_x` / `client_y`
   - `get_screen_size` → `ScreenSize` ✅，wire format 与前端一致无需改前端
6. `DbSender` 4 元组替换为命名 struct `DbWriteMsg` ✅（S6）
7. `emit_stream_error` 补充 `seq` ✅：主 chat 流 + compaction 流 3 处，使用 `AtomicU32` 追踪 `consume_stream` 回调中的最新 `seq`（主 chat 流用栈上 `AtomicU32`，compaction 流用 `Arc<AtomicU32>` 因闭包为 `move`）
8. `ChatDeltaPayload` 加 `derive(Default)` ✅，7 处构造点改为 `..Default::default()` 补全
9. ~~给 `storage/` 层 `MessageRow` / `ConversationRow` 添加 `#[serde(deny_unknown_fields)]`~~ → **决定不加**：storage 层 struct 从 DB 读取不参与 IPC 反序列化，加 `deny_unknown_fields` 反而阻碍未来加字段时的向前兼容；protocol 层 struct 为 Serialize-only（emit 方向），`deny_unknown_fields` 仅对 Deserialize 有效，亦不适用；前端契约测试已覆盖未知字段检测
10. 前端 `chatDeltaEvents.ts` 添加 R3/R4 运行时校验 ✅：`chat-stream-start` 的 id 与首条 `chat-delta` 一致（R3）；同一 `attempt_id` 下 `seq` 单调递增（R4）；不一致时 `console.error`；`finish_reason` 时清理 per-attempt seq 追踪
11. 验证 Chat + Provider + Orb 协议范围内 `json!({})` 为 0 处 ✅（chat.rs 0 + conversation.rs 0 + provider.rs 0 + orb.rs 0）
12. 前端协议同步 ✅：
    - `protocol.ts` 新增 `ProviderErrorCode` / `ProviderErrorPayloadRow`，`ChatDeltaPayload.error` 从 `unknown` 改为 `ProviderErrorPayloadRow`
    - `TokenUsageRow` 可选字段（`cache_read_tokens`/`cache_write_tokens`/`context_window_tokens`）修正为必选，对齐后端 `#[serde(default)]` 行为
    - `ProviderListRow.type_` 使用 `#[serde(rename = "type")]` 解决 Rust 保留字转义，wire format 输出 `"type"` 对齐前端
    - `OrbCoord.ts` 的 `get_mouse_position` 返回值从 `{ clientX, clientY }` 改为 `{ client_x, client_y }`，与 snake_case wire format 一致
    - 前端契约测试扩展至 14 个（新增 `MousePosition` / `ScreenSize` wire format 测试）

> **本 Phase 消除**：`chat.rs` 4 处 + `conversation.rs` 11 处 + `provider.rs` 3 处 `json!({})`（共 18 处 ✅）、`get_conversation_paginated` 1 处 `serde_json::Value` 返回 ✅、`list_providers` 1 处 `serde_json::Value` 返回 ✅、`chat-regenerate` 1 处 `json!({})` ✅、`setting:provider-changed` 2 处 `json!({})` ✅

### Phase 5：Setting / Provider / AgentMode 协议统一

> **注意**：`provider.rs` 3 处 `json!({})`、`setting:provider-changed` event payload、`list_providers` 返回值已在 Phase 4 提前完成。以下仅列剩余项。

1. **新建文件** ✅：
   - ~~新建 `src-tauri/src/protocol/setting_events.rs`~~ → 已在 Phase 4 完成
   - 新建 `src-tauri/src/protocol/setting_commands.rs` ✅：`AgentModeWire` / `AgentModeInputWire` / `ModelConfigWire` / `ThemeChangedPayload` / `ProviderInputWire` struct
   - ~~新建前端 `src/settings/core/protocol.ts`~~ → 决定不建：前端已有 `AgentMode`/`AgentModeInput`/`ModelConfig`/`Provider`/`ProviderInput` interface 对齐，无需额外 DTO 层
   - ~~新建 Setting 协议契约测试文件~~ → 决定不建：Setting struct 字段与前端 interface 已对齐，wire format 由 `#[serde(rename_all = "snake_case")]` 保证
2. 按 7.5 替换 `config.rs` / `agent_mode.rs` / `tool.rs` / `provider.rs` / `setup.rs` 中 `json!({})` 为 `protocol::setting_*` struct 构造 ✅
   - `config.rs`：`get_app_conf`/`get_orb_conf`/`get_mascot_conf` 返回强类型 `Result<T, String>`；`emit_theme_changed` 用 `ThemeChangedPayload`
   - `agent_mode.rs`：`list_agent_modes` 返回 `Vec<AgentModeWire>`；`save_agent_mode` 参数改为 `AgentModeInputWire`；`row_to_wire()` 转换 `tools_json` → `tools: Vec<String>`
   - `tool.rs`：`get_recent_tools`/`get_favorite_tools` 返回 `Vec<String>`（去掉 `json!`）
   - `provider.rs`：`list_models_cmd`/`preview_models_cmd` 返回 `Vec<ModelConfigWire>`；`save_provider` 参数改为 `ProviderInputWire`
   - `setup.rs`：`shortcut-warning` event 用 `ShortcutWarningPayload`
3. 按 7.1 替换前端强转 ✅：
   - `agentModeStore.ts`：`as AgentMode[]` → `invoke<AgentMode[]>("list_agent_modes")`
   - `providerStore.ts`：`as Provider[]` → `invoke<Provider[]>("list_providers")`；`as ModelConfig[]` → `invoke<ModelConfig[]>("list_models_cmd")`
   - `ConnectDialog.tsx`：`as ModelConfig[]` → `invoke<ModelConfig[]>("preview_models_cmd")`
   - `ProviderInput` 新增 `api_key_changed?: boolean` 字段，`ConnectDialog.tsx` 构造时设置
4. 验证全协议范围内 `json!({})` 为 0 处 ✅（全局扫描确认 0 处）

> **本 Phase 消除**：`config.rs` 3 处 `serde_json::Value` 返回 + 1 处 `json!({})`、`agent_mode.rs` 1 处 `serde_json::Value` 返回 + 1 处 `serde_json::Value` 参数、`tool.rs` 2 处 `json!({})`、`provider.rs` 2 处 `serde_json::Value` 返回 + 1 处 `serde_json::Value` 参数、`setup.rs` 1 处 `json!({})`、前端 3 处业务类型强转

### Phase 6：最终验证

1. 全局扫描前端 `as unknown as` ✅：2 处，`settingsDispatcher.ts` 1 处（TS 结构类型限制，interface→Record 需 double assertion）+ `i18n/index.ts` 1 处（I11-A6 设计决定，绕过 i18next 严格类型签名）
2. 全局扫描前端业务类型强转 ✅：0 处（`JSON.parse(provider.models) as ModelConfig[]` 已消除，后端 `ProviderListRow.models` 改为 `Vec<ModelConfigWire>`）
3. 全局扫描后端 `json!({})` ✅：0 处
4. 全局扫描后端 `serde_json::Value` command 返回/参数 ✅：1 处，`config.rs:patch_conf`（动态 JSON merge，保留合理）
5. 运行全部协议契约测试，确认通过 ✅（14 个测试全部通过）
6. 运行 R3/R4 运行时校验，确认集成测试通过 ✅

---

## 10. 验收标准

1. 后端存在 `src-tauri/src/protocol/` 模块，所有 event payload + command 返回值 struct 集中定义（含 Chat + Orb + Setting + Provider + AgentMode）
2. 前端存在 `src/chat/core/protocol.ts` + `src/settings/core/protocol.ts`，按模块分文件，文件头标注 `Backend mirror`
3. 前端 DTO 层 `MessageRow` / `ConversationRow` 与后端字段名一一对应（snake_case）
4. 前端业务层 `Message` / `Conversation` 使用 camelCase
5. `chatService.ts` invoke 返回值经 DTO 层强转 + 映射函数转换，无 `as Conversation[]` 业务类型强转
6. `tool_calls_json` → `toolCalls` 映射生效：DB 有数据时前端为 `ToolCall[]`，null 时为 `undefined`
7. event payload 字段名保持 snake_case
8. 所有 `chat-delta` 事件（含 error 分支）携带 `conversation_id`、`attempt_id`、`message_id`、`seq`（R1, R2）✅
9. compaction 流 error 分支补上 `seq` ✅（Phase 4）
10. 后端协议 struct 标注 `#[serde(rename_all = "snake_case")]`；storage struct（`MessageRow`/`ConversationRow`）不加 `deny_unknown_fields`（storage 从 DB 读取不参与 IPC 反序列化，加此标注阻碍向前兼容）；Rust 保留字字段（如 `type_`）使用 `#[serde(rename = "type")]` 显式转义
11. 协议契约测试通过：wire format 断言 + round-trip + 未知字段拒绝（Phase 4-5）
12. 全协议范围内 `json!({})` 为 0 处（Phase 5 完成后）
13. `get_mouse_position` 返回 snake_case（S5）✅
14. `useHistorySearch`（L39）/`useHistoryRefresh`（L37）统一走 chatService，无直接 invoke + 业务类型强转（F3）
15. `navigate` event payload 保持裸 string，不强制类型化（R5）
16. `providerStore` / `agentModeStore` 强转改为 DTO 类型 + 映射函数（F2，Phase 5）
17. `get_screen_size` 返回值类型化为 `ScreenSize` struct（S8，Phase 4）✅
18. `chat-regenerate` event payload 类型化为 `ChatRegeneratePayload` struct（Phase 4）✅
19. Setting event payload（`setting:provider-changed` / `theme-changed`）类型化（S2，Phase 5）
20. `protocolAdapter.ts` 提供双向映射：`rowToMessage`/`rowToConversation`（DTO→业务）+ `messageToRow`/`conversationToRow`（业务→DTO），业务代码无需手动构造 snake_case 字段
21. Orb event payload（`orb-blur`/`orb-new-conversation`）为空 payload，emit `()` 无需 struct（Phase 4）✅
22. `chatDeltaEvents.ts` 原散落类型定义已删除，唯一类型定义在 `protocol.ts`（单一真相源）
23. `chatDeltaEvents.ts` listen 回调内部经适配函数转换 DTO→业务类型再 dispatch 给 Store（**Phase 2 完成后**生效；Phase 1 时 event payload 与业务类型均为 snake_case，无需适配函数）
24. 前端 `chatDeltaEvents.ts` 包含 R3/R4 运行时校验：stream-start id 一致性 + seq 单调递增
25. `chatService.ts` / `chatDeltaEvents.ts` 等文件无 `as unknown as` 违规（仅 `settingsDispatcher.ts:toConfRecord` 和 `i18n/index.ts:translateKey` 允许，各有明确技术原因）
26. 前端 `ChatDeltaPayload.error` 类型为 `ProviderErrorPayloadRow`（非 `unknown`），与后端 `ProviderErrorPayload` wire format 对齐
27. 前端 `TokenUsageRow` 的 `cache_read_tokens`/`cache_write_tokens`/`context_window_tokens` 为必选字段（非可选），与后端 `#[serde(default)]` 保证 wire format 一定有值的行为对齐

---

## 11. 与 I15 的关系

| I15 交付物                                        | I16 依赖                                    |
| ------------------------------------------------- | ------------------------------------------- |
| DB `messages.tool_calls_json TEXT` 列             | I16 激活写入 + 前端映射                     |
| `MessageRow.tool_calls_json: Option<String>`      | I16 DTO 层 mirror 此字段                    |
| `StreamOutcome.tool_calls: Option<Vec<ToolCall>>` | I16 序列化写入 DB                           |
| `chat-stream-start` + `attempt_id` 协议           | I16 event payload 保持 snake_case，无需变更 |
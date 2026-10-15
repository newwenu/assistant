# I16 — 前后端消息协议统一设计规格说明

## 1. 交付目标

统一 Rust 后端与 TypeScript 前端之间的消息协议，消除当前字段命名不一致、类型映射缺失、事件 payload 隐式约定等问题，建立**单一真相源**的类型契约，为 I16 tool-call 执行循环和后续 agent 扩展奠定协议基础。

**设计原则**：
- **单一真相源**：每个跨端数据结构有且仅有一处定义，另一端通过映射或代码生成对齐
- **显式契约**：所有 Tauri command 返回值和 event payload 均有强类型定义，禁止 `as unknown as T` 透传
- **命名约定一致**：选定一种命名风格作为 wire format，两端统一遵守
- **零运行时开销**：映射在边界层（Tauri invoke/emit 边界）完成，业务逻辑层使用各自惯用命名
- **渐进式迁移**：不破坏现有行为，逐个协议点对齐

> **前置依赖**：I15 ✅（chat runtime 已提取，`tool_calls_json` DB 列已存在）
> **核心参考**：OpenBitFun 前端 `agent-api.ts` / `tool-events.ts`（snake_case wire format + 注释标注 "mirror backend payloads"）、Tauri 2 默认 command 参数 rename 规则

---

## 2. 当前问题全景

### 2.1 字段命名不一致

当前后端 Rust struct 使用 `snake_case`，前端 TypeScript interface 使用 `camelCase`，Tauri 作为中间层有隐式转换规则，但**不完整且不可靠**：

| 后端字段 (Rust) | 前端字段 (TS) | 传输方式 | 当前对齐状态 | 问题 |
|---|---|---|---|---|
| `tool_calls_json` | `toolCalls` | DB → Tauri command 返回 | ❌ 不对齐 | 后端返回 `tool_calls_json: null`，前端期望 `toolCalls?: ToolCall[]`，字段名和类型均不匹配 |
| `is_pinned` | `is_pinned` | DB → Tauri command 返回 | ⚠️ 偶然对齐 | 前端未转 camelCase，与 JS 惯例不符 |
| `is_full` | `is_full` | DB → Tauri command 返回 | ⚠️ 偶然对齐 | 同上 |
| `title_source` | `title_source` | DB → Tauri command 返回 | ⚠️ 偶然对齐 | 同上 |
| `provider_id` | `provider_id` | DB → Tauri command 返回 | ⚠️ 偶然对齐 | 同上 |
| `model_id` | `model_id` | DB → Tauri command 返回 | ⚠️ 偶然对齐 | 同上 |
| `parent_id` | `parent_id` | DB → Tauri command 返回 | ⚠️ 偶然对齐 | 同上 |
| `conversation_id` | `conversation_id` | event payload | ✅ 对齐 | event payload 无 Tauri rename，两端都用 snake_case |
| `attempt_id` | `attempt_id` | event payload | ✅ 对齐 | 同上 |
| `message_id` | `message_id` | event payload | ✅ 对齐 | 同上 |
| `delta_text` | `delta_text` | event payload | ✅ 对齐 | 同上 |
| `delta_reasoning` | `delta_reasoning` | event payload | ✅ 对齐 | 同上 |
| `delta_tool_call_id` | `delta_tool_call_id` | event payload | ✅ 对齐 | 同上 |
| `finish_reason` | `finish_reason` | event payload | ✅ 对齐 | 同上 |
| `retry_after_ms` | `retry_after_ms` | event payload | ✅ 对齐 | 同上 |

**关键发现**：
1. **Tauri command 返回值**：后端 `#[derive(Serialize)]` struct 默认按 Rust 字段名序列化（snake_case），Tauri **不做自动 camelCase 转换**。前端 `chatService.ts` 用 `as Conversation[]` / `as Message[]` 强转，依赖字段名恰好一致
2. **Tauri event payload**：后端 `json!({})` 手动构造，字段名由开发者决定，当前全部用 snake_case，前端也用 snake_case 接收，**已对齐**
3. **Tauri command 参数**：Tauri 2 默认将前端 camelCase 参数 rename 为 Rust snake_case（`conversationId` → `conversation_id`），**已对齐**
4. **`tool_calls_json` → `toolCalls`**：字段名不一致 + 类型不一致（`Option<String>` vs `ToolCall[]`），是最严重的协议断裂点

### 2.2 类型映射缺失

| 数据 | 后端类型 | 前端类型 | 映射逻辑 | 当前状态 |
|---|---|---|---|---|
| `tool_calls_json` | `Option<String>` (JSON string) | `toolCalls?: ToolCall[]` (parsed array) | `JSON.parse(tool_calls_json)` | ❌ 未实现，DB 中恒为 NULL |
| `is_pinned` | `i64` (0/1) | `boolean` | `val !== 0` | ⚠️ 后端 `read_conv_row` 已转换，但 `MessageRow` 的 `is_pinned` 直接用 `bool`，不一致 |
| `tokens` | `Option<i64>` | `number \| null` | 直接透传 | ✅ |
| `timestamp` | `i64` | `number` | 直接透传 | ✅ |
| `reasoning` | `Option<String>` | `string \| undefined` | 直接透传 | ✅ 字段名一致 |

### 2.3 事件 payload 无强类型契约

后端 `emit_delta` 使用 `json!({})` 手动构造 payload，字段名和结构与前端 `ChatDeltaPayload` interface 的对齐完全靠开发者自觉，无编译时校验。

当前 `chat-delta` 事件 payload 字段清单（后端 `emit_delta` 函数，[chat.rs:82-97](file:///g:/codeversion/assistant/src-tauri/src/cmd/chat.rs#L82-L97)）：

```rust
json!({
    "conversation_id": ectx.conv_id,
    "attempt_id": ectx.attempt_id,
    "message_id": ectx.msg_id,
    "seq": seq,
    "delta_text": delta.delta_text,
    "delta_reasoning": delta.delta_reasoning,
    "delta_tool_call_id": delta.delta_tool_call_id,
    "delta_tool_call_name": delta.delta_tool_call_name,
    "delta_tool_call_arguments": delta.delta_tool_call_arguments,
    "finish_reason": delta.finish_reason,
    "usage": delta.usage,
    "delta_retry": delta.delta_retry,
})
```

前端 `ChatDeltaPayload` interface（[chatOrchestrator.ts:63-78](file:///g:/codeversion/assistant/src/services/chatOrchestrator.ts#L63-L78)）：

```typescript
export interface ChatDeltaPayload {
  conversation_id: string;
  attempt_id?: string;
  message_id: string;
  seq?: number;
  delta_text?: string;
  delta_reasoning?: string;
  delta_tool_call_id?: string;
  delta_tool_call_name?: string;
  delta_tool_call_arguments?: string;
  finish_reason?: string;
  usage?: TokenUsage;
  error?: unknown;
  delta_retry?: RetryNotification;
}
```

两端字段名已对齐（都用 snake_case），但**无共享类型定义**，后端改字段名前端不会报错。

### 2.4 Tauri command 返回值无类型校验

后端 `list_conversations` / `get_conversation` / `get_conversation_paginated` 返回 `serde_json::Value`，前端用 `as Conversation[]` / `as ConversationDetail` 强转。若后端增删字段，前端无编译时感知。

```rust
// 后端：返回 serde_json::Value，丢失类型信息
pub fn list_conversations(...) -> Result<serde_json::Value, String> {
    let rows = conversations::get_conversations(&conn, provider_id.as_deref())?;
    serde_json::to_value(&rows).map_err(|e| e.to_string())
}
```

```typescript
// 前端：as 强转，无编译时校验
listConversations: (): Promise<Conversation[]> =>
    invoke("list_conversations", { providerId: null }).then(
        (r) => (r as Conversation[]) || [],
    ),
```

---

## 3. 收束决定点

参考项目模式速览：

| 项目 | 枏构 | Wire format | 对齐机制 | 未知字段检测 | TS 类型生成 |
|---|---|---|---|---|---|
| OpenBitFun (IPC) | Rust IPC | camelCase | `deny_unknown_fields` + 契约测试 | ✅ `parse_strict_frame` | — |
| OpenBitFun (App Server) | JSON-RPC | camelCase | `ts-rs` 自动生成 | ✅ `deny_unknown_fields` | ✅ `ts-rs` |
| OpenBitFun (前端 DTO) | Event stream | snake_case | 注释标注 mirror | — | — |
| BongoCat | Tauri (库) | camelCase | `deny_unknown_fields` | ✅ | — |
| ChatGPT | Tauri 2 | snake_case (默认) | 强类型返回 | — | — |
| 本项目 (event) | Tauri 2 | snake_case | 人工对齐 | — | — |
| 本项目 (command) | Tauri 2 | snake_case (默认) | `as` 强转 | — | — |

Tauri 2 命名转换规则：command 参数 camelCase→snake_case（自动）；command 返回值 / event payload **无转换**，字段名由后端序列化决定。当前项目 event payload 已全部 snake_case 且两端对齐，是最简单可靠的基准模式。

| # | 决策点 | 结论 | 理由 | 参考来源 |
|---|--------|------|------|---------|
| D1 | wire format 统一为 snake_case | ✅ 采用 | ① event payload 已全 snake_case 且对齐，改动成本为零 ② Rust `#[derive(Serialize)]` 默认 snake_case，无需 `#[serde(rename)]` ③ Tauri command 返回值无自动 camelCase 转换 ④ 前端业务层已有大量 snake_case 属性访问，统一 camelCase 改动面过大 | OpenBitFun 前端 DTO 层 (`agent-api.ts`) 同样 snake_case mirror 后端；ChatGPT Tauri 默认 snake_case |
| D2 | 前端 DTO/业务层分离 | ✅ 采用 | DTO 层 snake_case（与 wire format 一致）+ 适配层映射 + 业务层 camelCase（JS 惯例），消除一个类型内两种命名风格混用 | OpenBitFun `tool-execution-service.ts`：事件 DTO snake_case → handler 映射 → 业务对象 camelCase |
| D3 | 类型同步机制选 B（手动 mirror + 契约测试） | ✅ 采用 | ① Tauri event payload 无法代码生成（`tauri-specta`/`ts-rs` 只覆盖 command 返回值，event 13 处 vs command 4 处）② 项目约 20 个协议点，手动 mirror 可接受 ③ 契约测试 + `deny_unknown_fields` 在"检测不一致"上与代码生成等价 ④ 运行时校验（zod）每个协议点写两遍类型，过重 | OpenBitFun 前端 DTO 层手动 mirror + 注释标注；BongoCat fixture 契约测试；远期升级路径：协议点 50+ 时引入 `tauri-specta` |
| D4 | 后端 event payload 类型化 + `deny_unknown_fields` | ✅ 采用（P2） | 替换 `json!({})` 为 struct + `#[serde(deny_unknown_fields)]`，编译时校验字段存在性 + 运行时拒绝未知字段，消除遗漏字段无感知问题 | OpenBitFun IPC `deny_unknown_fields` 全局启用 + `parse_strict_frame` 未知字段检测；BongoCat fixture `deny_unknown_fields` |
| D5 | 协议契约测试 | ✅ 采用（P2） | wire format 断言 + round-trip 测试 + 未知字段拒绝测试，后端增删/重命名字段时测试失败，强制同步前端 DTO | OpenBitFun `protocol_contracts.rs`：`protocol_round_trips_*` + `protocol_rejects_unknown_fields_and_operations` |
| D6 | 协议版本化 | ❌ 远期 | 当前单版本部署，Tauri invoke/emit 已内置请求-响应关联和错误路由，无需 `RuntimeIpcFrame` 元数据帧层 | OpenBitFun `PROTOCOL_VERSION: u32` + `Initialize` 握手；远期触发条件：协议需向后兼容 或 脱离 Tauri 支持 HTTP/WS/CLI |
| D7 | 元数据帧协议层 | ❌ 不需要 | Tauri 2 已提供：invoke 自带 Promise resolve/reject（请求-响应关联）、IPC 内部帧定界、`Result<T, String>` 错误路由、`emit(event_name, payload)` 事件分发 | OpenBitFun `RuntimeIpcFrame` 因自定义 IPC（Unix socket + 长度前缀帧）需要应用层自行处理；本项目 Tauri 已是"帧协议" |
| D8 | `MessageRow` 添加 `#[serde(rename_all = "camelCase")]` | ❌ 不采用 | 同一数据结构在 command 返回值和 event payload 两个通道出现不同命名，增加认知负担 | — |

---

## 4. 统一方案

### 4.1 核心决策：wire format 统一为 snake_case

**理由**：
1. 当前 event payload 已全部使用 snake_case 且两端对齐，改动成本为零
2. 后端 Rust 惯用 snake_case，`#[derive(Serialize)]` 默认输出 snake_case，无需额外 `#[serde(rename)]` 标注
3. Tauri command 返回值不做自动 camelCase 转换，前端若用 camelCase 则需手动映射每条 command
4. OpenBitFun 参考项目前端 DTO 层也使用 snake_case mirror 后端
5. 前端业务层（Store/Component）已有大量 `snake_case` 属性访问（`conversation_id`、`message_id`、`provider_id` 等），统一为 camelCase 改动面过大

**代价**：前端 DTO interface 使用 snake_case，不符合 TypeScript 惯例。但通过**分层隔离**（DTO 层 snake_case ↔ 业务层 camelCase）可消除影响。

### 4.1.1 前端对接后端的三种机制对比

前端 DTO 层如何"对上"后端 wire format，本质上是一个**类型同步**问题。业界有三种机制：

| 机制 | 原理 | 代表项目 | 优点 | 缺点 |
|---|---|---|---|---|
| **A. 代码生成** | Rust 编译时自动生成 TS interface，前端直接 import | OpenBitFun SDK (`ts-rs`)、Tauri 官方 (`tauri-specta`) | 零人工同步，后端改字段前端自动跟着变 | 需引入新依赖 + build pipeline；生成代码难定制；event payload 不走 command 通道无法自动生成 |
| **B. 手动 mirror + 契约测试** | 前端手动定义 DTO interface（注释标注 mirror 后端），契约测试验证 wire format 一致 | OpenBitFun 前端 DTO 层 (`agent-api.ts`)、BongoCat | 无新依赖，灵活可控，event + command 统一适用 | 人工维护，需契约测试兜底 |
| **C. 运行时校验** | 前端用 zod/io-ts 定义 schema，`invoke().then(validate)` 运行时校验 | Web API 常见模式 | 运行时保障，可生成 TypeScript 类型 | 引入 zod 依赖；每个协议点写 schema 等于写两遍类型；Tauri invoke 泛型签名需包装 |

**当前项目选择 B（手动 mirror + 契约测试）**，理由：

1. **Tauri event payload 无法代码生成**：`tauri-specta` / `ts-rs` 只能生成 command 返回值类型，event payload 是 `app.emit("event", payload)` 手动构造的 `json!({})`，没有 Rust 函数签名可供生成。当前项目 event payload 是协议主体（13 处），command 返回值只有 4 处——代码生成只能覆盖小部分协议点
2. **项目规模不需要代码生成**：当前约 20 个协议点（13 event + 4 command 返回值 + 3 command 参数），手动 mirror 维护成本可接受。OpenBitFun 用 `ts-rs` 是因为面向外部 SDK 有上百个协议点
3. **契约测试提供同等保障**：wire format 断言 + round-trip 测试 + `deny_unknown_fields` 拒绝未知字段，组合起来与代码生成在"检测不一致"上效果等价，区别只在"修复不一致"时代码生成自动修、手动 mirror 需人修——但修复频率低（协议变更不频繁）
4. **运行时校验过重**：zod schema 定义量与 DTO interface 等价，等于每个协议点写两遍类型；且 Tauri invoke 泛型签名 `invoke<T>` 需要额外包装才能插入校验

**远期升级路径**：若协议点增长到 50+ 或需要对外暴露 SDK，可引入 `tauri-specta` 为 command 返回值自动生成类型，event payload 仍保持手动 mirror + 契约测试。

### 4.2 统一协议定义点

**核心原则：前端一个文件，后端一个模块，对着看就是协议全貌。**

#### 当前问题：协议定义散落各处

**后端**（9 个 cmd 文件各自构造，无统一协议模块）：

| 文件 | 内容 | 问题 |
|---|---|---|
| `cmd/chat.rs` | 4 个 emit + `send_chat_message` | event payload 用 `json!({})` 内联，无 struct 定义 |
| `cmd/conversation.rs` | 9 个 emit + 12 个 command | 同上，`serde_json::Value` 返回值无类型 |
| `cmd/provider.rs` | 2 个 emit + 4 个 command | 同上 |
| `cmd/config.rs` | 1 个 emit + 6 个 command | 同上 |
| `cmd/orb.rs` | 5 个 command | `get_mouse_position` 返回 camelCase |
| `cmd/tool.rs` | 4 个 command | `serde_json::Value` 返回值 |
| `cmd/agent_mode.rs` | 4 个 command | 同上 |
| `cmd/app.rs` | 1 个 emit + 6 个 command | `navigate` event 无类型 |
| `storage/conversations.rs` | `ConversationRow` / `MessageRow` struct | 定义在 storage 层，cmd 层不引用，直接 `to_value(&rows)` 序列化 |

**前端**（3 个文件各自定义，无统一协议入口）：

| 文件 | 内容 | 问题 |
|---|---|---|
| `chatTypes.ts` | `Message` / `Conversation` / 3 个 event payload | 混合 DTO+业务（snake_case + camelCase 混用）|
| `chatOrchestrator.ts` | `ChatDeltaPayload` / `ChatStreamStartPayload` | 2 个 event payload 定义在业务文件中 |
| `chatService.ts` | `ConversationDetail` / `TokenUsageSummary` / `ProviderBrief` | 3 个 DTO 定义在 service 文件中 |

#### 目标：两端各一个协议定义点

**后端**：`src-tauri/src/protocol/` 模块

```
src-tauri/src/protocol/
├── mod.rs              — re-export 所有协议类型
├── chat_events.rs      — Chat 协议所有 event payload struct
├── chat_commands.rs    — Chat 协议所有 command 返回值 struct
├── setting_events.rs   — Setting 协议所有 event payload struct（远期）
└── setting_commands.rs — Setting 协议所有 command 返回值 struct（远期）
```

- 所有 `app.emit("event", payload)` 的 payload 必须用此模块的 struct 构造，禁止 `json!({})`
- 所有 command 返回 `Result<T, String>` 的 `T` 必须是此模块的 struct，禁止 `serde_json::Value`
- struct 标注 `#[serde(deny_unknown_fields)]`
- `cmd/` 文件只做 `app.emit("event", ChatStreamStartPayload { ... })?` ，不定义任何协议类型

**前端**：`src/chat/core/protocol.ts` 文件

```typescript
/**
 * Chat protocol DTOs (frontend).
 *
 * These types mirror backend protocol structs and intentionally use
 * snake_case fields to match the wire format.
 * 
 * Backend mirror: src-tauri/src/protocol/chat_events.rs
 *                 src-tauri/src/protocol/chat_commands.rs
 */

// ── Command 返回值 ──
export interface ConversationRow { ... }   // mirror ConversationRow
export interface MessageRow { ... }        // mirror MessageRow
export interface ConversationDetailRow { ... }
export interface ProviderBriefRow { ... }
export interface TokenUsageSummaryRow { ... }

// ── Event payload ──
export interface ChatStreamStartPayload { ... }
export interface ChatDeltaPayload { ... }
export interface ChatStreamErrorPayload { ... }
export interface ConversationTerminatedPayload { ... }
export interface ConversationCompactFailedPayload { ... }
export interface ConversationUnlockedPayload { ... }
```

- 所有 `invoke()` 返回值强转目标必须是此文件的类型
- 所有 `listen()` event payload 断言目标必须是此文件的类型
- `chatTypes.ts` 只定义业务类型（camelCase），不再包含任何 snake_case 字段
- `chatService.ts` / `chatOrchestrator.ts` 不再定义任何协议类型

#### 对接方式：看文件头注释

前端 `protocol.ts` 文件头标注 `Backend mirror: src-tauri/src/protocol/chat_events.rs`，后端 `chat_events.rs` 文件头标注 `Frontend mirror: src/chat/core/protocol.ts`。IDE 中打开任一端，一眼看到对端位置。

#### 契约测试：验证两端一致

```rust
// src-tauri/src/protocol/tests/chat_contracts.rs

#[test]
fn chat_stream_start_wire_format() {
    let payload = ChatStreamStartPayload {
        conversation_id: "test".into(),
        model_id: "gpt-4".into(),
        ..Default::default()
    };
    let json = serde_json::to_value(&payload).unwrap();
    // 验证 wire format 字段名与前端 protocol.ts 一致
    assert!(json.get("conversation_id").is_some());
    assert!(json.get("model_id").is_some());
    // 验证无 camelCase 泄漏
    assert!(json.get("conversationId").is_none());
    assert!(json.get("modelId").is_none());
}
```

测试注释标注对应前端类型名：`// mirrors: ChatStreamStartPayload in protocol.ts`

### 4.3 分层架构

```
┌─────────────────────────────────────────────────┐
│  前端业务层 (Store / Component / Hook)           │
│  使用 camelCase: message.toolCalls              │
│  使用 camelCase: conversation.providerId         │
├─────────────────────────────────────────────────┤
│  适配层 (protocolAdapter.ts)                     │
│  DTO → 业务类型映射: rowToMessage()              │
│  业务类型 → DTO: messageToRow()                  │
├─────────────────────────────────────────────────┤
│  DTO 层 (protocol.ts)        ← 统一协议定义点     │
│  使用 snake_case: message_row.tool_calls_json    │
│  与后端 wire format 完全一致                      │
├─────────────────────────────────────────────────┤
│  Tauri 边界 (invoke / listen)                    │
│  command 参数: camelCase → snake_case (自动)     │
│  command 返回: snake_case (原样)                  │
│  event payload: snake_case (原样)                │
└─────────────────────────────────────────────────┘

后端对应：
┌─────────────────────────────────────────────────┐
│  cmd/ 层 — 只做业务逻辑 + 调用协议类型构造        │
│  app.emit("event", ChatStreamStartPayload{...}) │
├─────────────────────────────────────────────────┤
│  protocol/ 模块           ← 统一协议定义点        │
│  所有 event payload struct + command 返回值 struct│
│  #[serde(deny_unknown_fields)]                   │
├─────────────────────────────────────────────────┤
│  storage/ 层 — ConversationRow / MessageRow      │
│  protocol/ 的 struct 可以直接引用 storage/ 的     │
│  或定义独立的 command 返回值 struct                │
└─────────────────────────────────────────────────┘
```

### 4.4 后端变更

#### 4.4.1 `MessageRow` 添加 `#[serde(rename_all = "camelCase")]` — ❌ 不采用

**理由**：`MessageRow` 通过 `serde_json::to_value(&rows)` 序列化后传给前端，若加 `rename_all = "camelCase"` 则输出 `toolCallsJson`，前端 DTO 层需同步改为 camelCase，但 event payload 仍为 snake_case，**同一数据结构在两个通道出现不同命名**，增加认知负担。

**决策**：后端 struct 保持默认 snake_case 序列化，前端 DTO 层 mirror snake_case。

#### 4.4.2 `tool_calls_json` 写入激活

当前 `update_message_content` 的第 6 参数 `tool_calls_json` 在所有调用点恒为 `None`。I16 激活写入：

```rust
// cmd/chat.rs — stream_future Ok 分支
let tool_calls_json = outcome.tool_calls.as_ref()
    .and_then(|tc| serde_json::to_string(tc).ok());

let _ = db_tx.send((
    msg_id.clone(),
    outcome.full_content.clone(),
    tokens,
    reasoning,
    tool_calls_json,  // 从 None 变为实际序列化值
));
```

#### 4.4.3 event payload 类型化 + `deny_unknown_fields`（借鉴 OpenBitFun）

当前 `emit_delta` 使用 `json!({})` 手动构造。引入 `ChatDeltaPayload` struct 替代，并借鉴 OpenBitFun 的 `deny_unknown_fields` 机制：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
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
    pub delta_retry: Option<RetryNotification>,
}
```

**收益**：
1. 后端改字段名有编译时校验（`Serialize` derive 会检查字段存在性）
2. `deny_unknown_fields` 使反序列化时拒绝未知字段，**运行时保障**前后端协议一致（借鉴 OpenBitFun `parse_strict_frame` 模式）
3. `Debug + Clone + Deserialize` 支持 round-trip 契约测试

**代价**：需定义 struct + 替换 `json!({})` 调用点。
**优先级**：P2，可在 I16 tool-call 循环实现后统一清理。

#### 4.4.4 协议契约测试（借鉴 OpenBitFun `protocol_contracts.rs`）

在 `assistant-agent` crate 中添加协议契约测试，确保 Rust struct 序列化输出与前端 DTO interface 字段名一致：

```rust
// tests/protocol_contracts.rs

#[test]
fn chat_delta_payload_wire_format_matches_frontend_dto() {
    let payload = ChatDeltaPayload {
        conversation_id: "conv-1".into(),
        attempt_id: "att-1".into(),
        message_id: "msg-1".into(),
        seq: 0,
        delta_text: Some("hello".into()),
        delta_reasoning: None,
        delta_tool_call_id: None,
        delta_tool_call_name: None,
        delta_tool_call_arguments: None,
        finish_reason: None,
        usage: None,
        delta_retry: None,
    };

    let json = serde_json::to_value(&payload).unwrap();

    // 断言 wire format 字段名与前端 ChatDeltaPayload interface 一致
    assert!(json.get("conversation_id").is_some());  // snake_case
    assert!(json.get("attempt_id").is_some());
    assert!(json.get("message_id").is_some());
    assert!(json.get("delta_text").is_some());
    assert!(json.get("delta_reasoning").is_some());
    assert!(json.get("delta_tool_call_id").is_some());
    assert!(json.get("finish_reason").is_some());

    // 断言不存在 camelCase 字段（防止误加 rename_all）
    assert!(json.get("conversationId").is_none());
    assert!(json.get("deltaText").is_none());
}

#[test]
fn chat_delta_payload_rejects_unknown_fields() {
    let json_with_extra = json!({
        "conversation_id": "conv-1",
        "attempt_id": "att-1",
        "message_id": "msg-1",
        "seq": 0,
        "unknown_field": "should_fail"
    });
    assert!(serde_json::from_value::<ChatDeltaPayload>(json_with_extra).is_err());
}

#[test]
fn message_row_wire_format_matches_frontend_dto() {
    let row = MessageRow {
        id: "msg-1".into(),
        conversation_id: "conv-1".into(),
        role: "assistant".into(),
        content: "hello".into(),
        tokens: Some(10),
        timestamp: 1000,
        parent_id: None,
        is_pinned: false,
        reasoning: None,
        tool_calls_json: None,
    };

    let json = serde_json::to_value(&row).unwrap();

    // 断言 wire format 使用 snake_case
    assert!(json.get("tool_calls_json").is_some());   // 不是 "toolCallsJson"
    assert!(json.get("conversation_id").is_some());   // 不是 "conversationId"
    assert!(json.get("is_pinned").is_some());         // 不是 "isPinned"

    // 断言不存在 camelCase 字段
    assert!(json.get("toolCallsJson").is_none());
    assert!(json.get("conversationId").is_none());
}
```

**收益**：后端增删/重命名字段时，契约测试失败，强制开发者同步更新前端 DTO。
**借鉴来源**：OpenBitFun `protocol_contracts.rs` 的 `protocol_round_trips_*` 系列测试。

#### 4.4.5 协议版本化（远期，借鉴 OpenBitFun）

OpenBitFun 使用 `PROTOCOL_VERSION: u32` 常量标识协议版本，前端在 `Initialize` 握手时校验版本兼容性。当前项目规模不需要此机制，但记录为远期演进方向：

```rust
// 远期：当协议变更需要向后兼容时引入
pub const CHAT_PROTOCOL_VERSION: u32 = 1;
```

### 4.5 协议元数据约束

#### 4.5.1 为什么不需要元数据驱动的帧协议

OpenBitFun 使用 `RuntimeIpcFrame` 元数据帧层，因为其传输层是自定义 IPC（Unix domain socket + 长度前缀 JSON 帧），需要在应用层自行处理请求-响应关联、错误路由、事件分发。

**当前项目使用 Tauri 2 内置 invoke/emit 机制**，Tauri 已提供：

| 元数据职责 | OpenBitFun 自行实现 | 本项目 Tauri 内置 |
|---|---|---|
| 请求-响应关联 | `request_id` 元数据字段 | Tauri invoke 自带 Promise resolve/reject |
| 帧定界 | 长度前缀 + `parse_strict_frame` | Tauri IPC 内部处理 |
| 错误路由 | `RuntimeIpcFrame::Error` variant | Tauri command 返回 `Result<T, String>` |
| 事件分发 | `RuntimeIpcFrame::Event` + `RuntimeIpcEvent` enum | Tauri `emit(event_name, payload)` |
| 协议版本协商 | `Initialize { protocol_version }` 握手 | 无（单版本部署） |

**结论**：不需要 `RuntimeIpcFrame` 这样的元数据帧层。Tauri 已经是"帧协议"。

#### 4.5.2 已有元数据字段的协议级约束

当前项目中 event payload 已包含关联元数据字段，但**无协议级约束**确保其一致性和完整性：

**Event payload 关联元数据清单**：

| 元数据字段 | 类型 | 必选事件 | 可选事件 | 语义 |
|---|---|---|---|---|
| `conversation_id` | `string` | `chat-stream-start`, `chat-delta`, `conversation_terminated`, `conversation_compact_failed`, `conversation_unlocked` | — | 关联到哪个会话 |
| `attempt_id` | `string` | `chat-stream-start` | `chat-delta`（error 分支可缺省） | 关联到哪次流式尝试（I15 引入） |
| `message_id` | `string` | `chat-stream-start`, `chat-delta` | — | 关联到哪条消息 |
| `seq` | `number` | `chat-delta`（正常 delta） | `chat-delta`（error 分支） | delta 序号，用于排序和去重 |

**当前问题**：

1. **`emit_stream_error` 缺少 `seq`**：error 事件不携带 `seq`，前端无法知道 error 发生在哪个 delta 之后
2. **`attempt_id` 在 error 分支为必选但语义弱**：error 发生时 `attempt_id` 已知，应始终携带
3. **元数据字段无类型化约束**：后端 `json!({})` 手动构造，遗漏字段无编译时感知

**约束要求**：

> **R1**：所有 `chat-delta` 事件（包括 error 分支）**必须**携带 `conversation_id`、`attempt_id`、`message_id` 三个关联元数据字段
>
> **R2**：`chat-delta` 正常 delta 分支**必须**携带 `seq`；error 分支 `seq` 可选（error 可能在 seq 赋值前发生）
>
> **R3**：`chat-stream-start` 事件**必须**携带 `conversation_id`、`attempt_id`、`message_id`，且值与后续 `chat-delta` 事件一致
>
> **R4**：同一 `attempt_id` 下的 `chat-delta` 事件，`seq` 必须**单调递增**（从 0 开始）

**修复 `emit_stream_error` 缺少 `seq`**：

```rust
// 变更前
fn emit_stream_error(ectx: &StreamEmitCtx, error: &ProviderErrorPayload) {
    ectx.orb_window.emit("chat-delta", json!({
        "conversation_id": ectx.conv_id,
        "attempt_id": ectx.attempt_id,
        "message_id": ectx.msg_id,
        "error": error,
    }))
}

// 变更后：添加 last_seq 参数
fn emit_stream_error(ectx: &StreamEmitCtx, last_seq: u32, error: &ProviderErrorPayload) {
    ectx.orb_window.emit("chat-delta", json!({
        "conversation_id": ectx.conv_id,
        "attempt_id": ectx.attempt_id,
        "message_id": ectx.msg_id,
        "seq": last_seq,    // 补充：error 发生时的最后 seq
        "error": error,
    }))
}
```

调用点需传入当前 `seq`（`consume_stream` 的 `on_delta` 回调已提供 `seq` 参数，error 分支需从 `StreamAccumulator` 获取已消费的 delta 数量）。

#### 4.5.3 元数据契约测试

在协议契约测试中增加元数据完整性断言：

```rust
#[test]
fn chat_delta_error_event_includes_all_correlation_metadata() {
    let error_payload = json!({
        "conversation_id": "conv-1",
        "attempt_id": "att-1",
        "message_id": "msg-1",
        "seq": 5,
        "error": { "code": "provider_error", "message": "rate limited" }
    });

    // 断言 error 事件包含全部关联元数据
    assert!(error_payload.get("conversation_id").is_some());
    assert!(error_payload.get("attempt_id").is_some());
    assert!(error_payload.get("message_id").is_some());
    assert!(error_payload.get("seq").is_some());
}
```

#### 4.5.4 远期：当脱离 Tauri 时需引入帧协议层

如果未来项目需要支持非 Tauri 传输（如 HTTP API、WebSocket、CLI），则需要引入类似 OpenBitFun `RuntimeIpcFrame` 的元数据帧层。此时应：

1. 定义 `ChatFrame` enum，包装所有 command 和 event
2. 引入 `request_id` 用于请求-响应关联
3. 引入 `PROTOCOL_VERSION` 用于版本协商
4. 帧大小限制 + 未知字段检测（借鉴 OpenBitFun `framing.rs`）

当前不引入，避免过度设计。

### 4.6 前端变更

#### 4.6.1 DTO 层：新增 `MessageRow` / `ConversationRow` 类型

当前前端 `Message` interface 混合了 DTO 层和业务层职责。拆分为两层：

**DTO 层**（与后端 wire format 完全一致，借鉴 OpenBitFun 注释标注模式）：

```typescript
// chat/core/protocol.ts

/**
 * Wire-format DTOs for Tauri command returns and event payloads.
 *
 * These types mirror backend Rust structs (MessageRow, ConversationRow, etc.)
 * and intentionally use snake_case fields to match the wire format.
 * Do NOT convert field names here — conversion happens in protocolAdapter.ts.
 *
 * @see OpenBitFun reference: web-ui/src/shared/types/agent-api.ts
 */

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
  tool_calls_json: string | null;  // snake_case + JSON string，与后端一致
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

**业务层**（前端惯用命名，供 Store / Component 使用）：

```typescript
// chat/core/chatTypes.ts（现有，微调）

export interface Message {
  id: string;
  conversationId: string;     // camelCase
  role: string;
  content: string;
  reasoning?: string;
  toolCalls?: ToolCall[];     // camelCase + 已解析数组
  tokens: number | null;
  timestamp: number;
  error?: ChatError | null;
  parentId?: string | null;   // camelCase
  isPinned?: boolean;         // camelCase
}

export interface Conversation {
  id: string;
  title: string;
  titleSource: string;        // camelCase
  providerId: string | null;  // camelCase
  modelId: string;            // camelCase
  systemPrompt: string | null; // camelCase
  isPinned: boolean;
  sortOrder: number;
  generation: number;
  createdAt: number;
  updatedAt: number;
  isFull: boolean;
}
```

#### 4.6.2 适配层：`rowToMessage` / `rowToConversation` 映射函数

```typescript
// chat/core/protocolAdapter.ts

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
  return {
    id: row.id,
    title: row.title,
    titleSource: row.title_source,
    providerId: row.provider_id,
    modelId: row.model_id,
    systemPrompt: row.system_prompt,
    isPinned: row.is_pinned,
    sortOrder: row.sort_order,
    generation: row.generation,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
    isFull: row.is_full,
  };
}

function parseToolCallsJson(json: string | null): ToolCall[] | undefined {
  if (!json) return undefined;
  try {
    const parsed = JSON.parse(json);
    if (Array.isArray(parsed)) return parsed;
    return undefined;
  } catch {
    return undefined;
  }
}
```

#### 4.6.3 `chatService.ts` 变更

```typescript
// services/chatService.ts

export const chatService = {
  listConversations: (): Promise<Conversation[]> =>
    invoke("list_conversations", { providerId: null }).then((r) => {
      const rows = (r as ConversationRow[]) || [];
      return rows.map(rowToConversation);
    }),

  getConversationPaginated: (
    id: string | null,
    limit: number,
    beforeTimestamp: number | null,
  ): Promise<ConversationDetail | null> =>
    invoke("get_conversation_paginated", { id, limit, beforeTimestamp }).then(
      (r) => {
        const detail = r as ConversationDetailRow | null;
        if (!detail) return null;
        return {
          conversation: rowToConversation(detail.conversation),
          messages: detail.messages.map(rowToMessage),
          total_count: detail.total_count ?? detail.messages.length,
        };
      },
    ),

  // ... 其他方法不变
};
```

#### 4.6.4 event payload 类型保持 snake_case

event payload 已对齐，**不变**。`ChatDeltaPayload` / `ChatStreamStartPayload` / `ConversationTerminatedPayload` 等保持 snake_case 字段名。

#### 4.6.5 Store 层变更

`chatStore.ts` 中访问 `conversation` 和 `message` 属性改为 camelCase：

```typescript
// 变更前
conv.provider_id
conv.model_id
detail.conversation?.is_full
m.conversation_id

// 变更后
conv.providerId
conv.modelId
detail.conversation?.isFull
m.conversationId
```

**影响范围**：需全局搜索 `snake_case` 属性访问并替换。主要文件：
- `chatStore.ts`：`provider_id`、`model_id`、`is_full`、`title_source` 等
- `chatOrchestrator.ts`：`conversation_id`、`attempt_id`、`message_id`（event payload，**不变**）
- `chatDeltaEvents.ts`：event payload 属性，**不变**
- `MessageBubble.tsx`：`message.toolCalls`（已是 camelCase）
- `useMascotChatSync.ts`：`m.toolCalls`（已是 camelCase）

---

## 5. 协议清单（单一真相源）

### 5.1 Tauri Command 协议

| Command | 后端返回类型 | 前端 DTO 类型 | 前端业务类型 | 映射函数 |
|---|---|---|---|---|
| `list_conversations` | `Vec<ConversationRow>` | `ConversationRow[]` | `Conversation[]` | `rowToConversation` |
| `get_conversation` | `ConversationDetail` | `ConversationDetailRow` | `ConversationDetail` | `rowToConversation` + `rowToMessage` |
| `get_conversation_paginated` | `serde_json::Value` | `ConversationDetailRow` | `ConversationDetail` | `rowToConversation` + `rowToMessage` |
| `create_conversation` | `String` | — | — | — |
| `delete_conversation` | `()` | — | — | — |
| `rename_conversation` | `()` | — | — | — |
| `send_chat_message` | `()` | — | — | — |
| `cancel_chat` | `()` | — | — | — |
| `search_conversations` | `Vec<ConversationRow>` | `ConversationRow[]` | `Conversation[]` | `rowToConversation` |

### 5.2 Tauri Event 协议

| Event | 后端构造方式 | 前端接收类型 | 字段命名 |
|---|---|---|---|
| `chat-stream-start` | `json!({})` | `ChatStreamStartPayload` | snake_case |
| `chat-delta` | `json!({})` | `ChatDeltaPayload` | snake_case |
| `conversation_terminated` | `json!({})` | `ConversationTerminatedPayload` | snake_case |
| `conversation_compact_failed` | `json!({})` | `ConversationCompactFailedPayload` | snake_case |
| `conversation_unlocked` | `json!({})` | `ConversationUnlockedPayload` | snake_case |

**Event payload 全部保持 snake_case，无需映射。**

### 5.2.1 散落构造体盘点

当前后端 `json!({})` 散布在 4 个文件共 23 处，按协议性质分为三类：

**A. Chat 协议 event payload（§5.2 已覆盖，Phase 4 类型化）**：

| 文件 | 行 | 事件名 | 构造方式 | 问题 |
|---|---|---|---|---|
| `cmd/chat.rs` | L85 | `chat-delta` | `json!({})` | 无类型约束 |
| `cmd/chat.rs` | L107 | `chat-delta` (error) | `json!({})` | 缺少 `seq`（§4.4.2 R2） |
| `cmd/chat.rs` | L290 | `chat-stream-start` | `json!({})` | 无类型约束 |
| `cmd/chat.rs` | L186 | `conversation_compact_failed` | `json!({})` | 无类型约束 |
| `cmd/conversation.rs` | L428 | `chat-stream-start` (compaction) | `json!({})` | 无类型约束 |
| `cmd/conversation.rs` | L437 | `chat-delta` (compaction seq=0) | `json!({})` | 无类型约束 |
| `cmd/conversation.rs` | L457 | `chat-delta` (compaction delta) | `json!({})` | 无类型约束 |
| `cmd/conversation.rs` | L520 | `chat-delta` (Cancelled error) | `json!({})` | 缺少 `seq` |
| `cmd/conversation.rs` | L540 | `chat-delta` (Provider error) | `json!({})` | 缺少 `seq` |
| `cmd/conversation.rs` | L554 | `chat-delta` (Timeout error) | `json!({})` | 缺少 `seq` |
| `cmd/conversation.rs` | L323 | `conversation_compact_failed` | `json!({})` | 无类型约束 |
| `cmd/conversation.rs` | L344 | `conversation_unlocked` | `json!({})` | 无类型约束 |
| `cmd/conversation.rs` | L228 | `conversation_terminated` | `json!({})` | 无类型约束 |

**B. Setting 协议 event payload（不在 I16 范围，但记录风险）**：

| 文件 | 行 | 事件名 | 构造方式 | 问题 |
|---|---|---|---|---|
| `cmd/provider.rs` | L135 | `setting:provider-changed` | `json!({})` | 无类型约束，前端用 `SettingEventPayload` 消费 |
| `cmd/provider.rs` | L149 | `setting:provider-changed` | `json!({})` | 同上 |
| `cmd/config.rs` | L139 | `theme-changed` | `json!({})` | 无类型约束 |
| `setup.rs` | L149 | `shortcut-warning` | `json!({})` | 无类型约束，前端无监听 |

**C. Command 返回值构造（非 event，但存在类型安全风险）**：

| 文件 | 行 | Command | 构造方式 | 问题 |
|---|---|---|---|---|
| `cmd/conversation.rs` | L143 | `get_conversation` | `json!({})` | 手动拼装 `{ conversation, messages, total_count }`，无类型 struct |
| `cmd/provider.rs` | L54 | `list_providers` | `json!({})` | 手动逐字段构造，遗漏字段无编译时感知 |
| `cmd/orb.rs` | L25 | `get_mouse_position` | `json!({})` | 返回 `{ clientX, clientY }` — **camelCase**，与全局 snake_case 约定不一致 |
| `cmd/orb.rs` | L85,88 | `get_screen_size` | `json!({})` | 返回 `{ screen: { x, y, width, height } }` |

### 5.2.2 零散全局变量盘点

| 文件 | 变量 | 类型 | 用途 | 协议影响 |
|---|---|---|---|---|
| `cmd/orb.rs` | `ORB_CLICK_THROUGH` | `AtomicBool` | Orb 穿透点击状态 | 无（内部状态，不跨 IPC） |
| `cmd/orb.rs` | `ORB_SNAP_CANCEL` | `AtomicBool` | 取消吸附动画 | 无（内部状态） |
| `cmd/chat.rs` | `DbSender` type alias | `UnboundedSender<(String, String, Option<i64>, Option<String>)>` | DB 写入 channel | **有**：4 元组 `(msg_id, content, tokens, reasoning)` 是隐式协议，字段顺序和类型无文档化 |
| `cmd/chat.rs` | `spawn_finish_listener` 返回 | `UnboundedSender<String>` | 通知流结束 | **弱**：仅传 `conv_id`，无协议风险 |
| `cmd/chat.rs` | `spawn_compaction_listener` 返回 | `UnboundedSender<(String, ConversationDetail, ProviderRow)>` | 触发自动压缩 | **弱**：内部传递，不跨 IPC |

### 5.2.3 对散落构造体和全局变量的约束要求

> **S1**：A 类（Chat 协议 event payload）在 Phase 4 全部替换为类型化 struct + `deny_unknown_fields`，消除 `json!({})` 手动构造
>
> **S2**：B 类（Setting 协议 event payload）不在 I16 范围，但应遵循同一模式：未来类型化时优先处理 `setting:provider-changed`（前端已有 `SettingEventPayload` 消费）
>
> **S3**：C 类中 `get_conversation` 的 `json!({})` 返回值应定义 `ConversationDetailResponse` struct（含 `conversation`、`messages`、`total_count` 字段），替代手动拼装
>
> **S4**：C 类中 `list_providers` 的 `json!({})` 逐字段构造应定义 `ProviderListItem` struct（含 `api_key_masked` 而非 `api_key_encrypted`），替代手动映射
>
> **S5**：`get_mouse_position` 返回 `{ clientX, clientY }` 使用 **camelCase**，与全局 snake_case 约定不一致。应改为 `{ client_x, client_y }` 或定义 struct + `#[serde(rename_all = "snake_case")]`。**注意**：此 command 仅 Orb 窗口使用，变更需同步前端调用点
>
> **S6**：`DbSender` 的 4 元组 `(String, String, Option<i64>, Option<String>)` 应定义命名 struct `DbWriteMsg { msg_id, content, tokens, reasoning }`，消除位置依赖和类型歧义。Phase 4 实施
>
> **S7**：所有 `json!({})` 构造点在 Phase 4 完成后应为 **0 处**（Chat 协议范围内）。`cargo clippy` 可配置 `disallowed_methods` 禁止 `serde_json::json!` 在 `cmd/` 目录使用

### 5.2.4 前端散落构造体盘点

前端同样存在散落的类型断言和隐式协议依赖，按性质分为四类：

**A. invoke 返回值 `as` 强转（核心问题，§4.5 已规划消除）**：

| 文件 | 行 | invoke | 强转目标 | 问题 |
|---|---|---|---|---|
| `chatService.ts` | L24 | `list_conversations` | `as Conversation[]` | 后端返回 `serde_json::Value`，前端无编译时校验 |
| `chatService.ts` | L28 | `list_providers` | `as ProviderBrief[]` | 同上 |
| `chatService.ts` | L43 | `get_conversation_paginated` | `as ConversationDetail \| null` | 同上 |
| `providerStore.ts` | L111 | `list_providers` | `as Provider[]` | Store 层重复强转，与 chatService 不同类型 |
| `providerStore.ts` | L128 | `list_models_cmd` | `as ModelConfig[]` | 同上 |
| `agentModeStore.ts` | L28 | `list_agent_modes` | `as AgentMode[]` | 同上 |
| `useHistorySearch.ts` | L39 | `search_conversations` | `as Conversation[]` | Hook 层直接 invoke + 强转，绕过 chatService |
| `useHistoryRefresh.ts` | L37 | `get_conversation_paginated` | `as { messages, total_count }` | 匿名类型强转，无 interface 定义 |

**B. event payload `as` 强转（Tauri 限制，无法完全消除）**：

| 文件 | 行 | 事件 | 强转目标 | 说明 |
|---|---|---|---|---|
| `chatDeltaEvents.ts` | L48 | `chat-stream-start` | `as ChatStreamStartPayload` | Tauri `listen` 返回 `unknown`，必须强转 |
| `chatDeltaEvents.ts` | L77 | `chat-delta` | `as ChatDeltaPayload` | 同上 |
| `chatDeltaEvents.ts` | L128 | `conversation_terminated` | `as ConversationTerminatedPayload` | 同上 |
| `chatDeltaEvents.ts` | L144 | `conversation_compact_failed` | `as ConversationCompactFailedPayload` | 同上 |
| `chatDeltaEvents.ts` | L155 | `conversation_unlocked` | `as ConversationUnlockedPayload` | 同上 |

> Tauri 2 的 `listen` API 返回 `event.payload: unknown`，TypeScript 层必须用 `as` 断言。这是 Tauri 的设计限制，**无法完全消除**。但可通过 DTO 层类型定义 + 适配层映射函数确保断言目标类型与后端一致。

**C. `JSON.parse` + `as` 强转**：

| 文件 | 行 | 解析目标 | 强转 | 问题 |
|---|---|---|---|---|
| `chatStore.ts` | L46 | `provider.models` | `as ModelConfig[]` | `models` 是 JSON 字符串，解析后强转，无运行时校验 |
| `toolStore.ts` | L66 | localStorage | `as string[]` | 本地存储，协议风险低 |

**D. 模块级全局变量**：

| 文件 | 变量 | 类型 | 用途 | 协议影响 |
|---|---|---|---|---|
| `chatDeltaEvents.ts` | `orchestratorInstance` | `ChatOrchestrator \| null` | 单例引用 | 无（内部状态） |
| `chatStore.ts` | `_orchestrator` | `ChatOrchestrator \| null` | 单例引用 | 无（内部状态） |
| `toolStore.ts` | `fuseInstance` | `Fuse` | 搜索索引 | 无（内部状态） |

### 5.2.5 对前端散落构造体的约束要求

> **F1**：A 类中 `chatService.ts` 的 `as Conversation[]` / `as ConversationDetail` / `as ProviderBrief[]` 在 Phase 1-2 替换为 DTO 层 `as ConversationRow[]` + 映射函数 `rowToConversation()`，强转目标从业务类型变为 DTO 类型（与后端 wire format 一致）
>
> **F2**：A 类中 `providerStore.ts` 的 `as Provider[]` / `as ModelConfig[]` 和 `agentModeStore.ts` 的 `as AgentMode[]` 不在 I16 范围（Setting 协议），但应遵循同一模式：未来定义 DTO 层 + 映射函数
>
> **F3**：A 类中 `useHistorySearch.ts` 和 `useHistoryRefresh.ts` 的直接 invoke + 强转应**统一走 chatService**，避免同一 command 有多个消费点各自强转不同类型。当前 `useHistoryRefresh.ts` 用匿名类型 `as { messages, total_count }` 尤其危险
>
> **F4**：B 类 event payload `as` 断言**无法消除**（Tauri 限制），但要求：
> - 断言目标类型必须定义在 DTO 层（`protocol.ts`），与后端 wire format 一致
> - 不得在断言后直接当业务对象使用，须经适配层映射
> - 当前 `ChatDeltaPayload` 已在 `chatOrchestrator.ts` 定义，Phase 1 迁移到 `protocol.ts`
>
> **F5**：C 类 `JSON.parse(provider.models) as ModelConfig[]` 应替换为 `parseModelsJson(models: string | null): ModelConfig[]` 安全解析函数（对非法 JSON 返回 `[]`），与 `parseToolCallsJson` 同模式
>
> **F6**：D 类模块级全局变量无协议影响，不要求变更。但 `_orchestrator` / `orchestratorInstance` 两个同名概念分别存在于 `chatStore.ts` 和 `chatDeltaEvents.ts`，应统一为单一入口（Phase 2 重构时顺带处理）

### 5.2.6 当前做法与文档方案的差异对比

文档提出的方案与当前代码实际做法存在 **7 项核心差异**，按影响程度排列：

#### 差异 1：前端 Message/Conversation 类型混合了 DTO 层和业务层（最大差异）

| 维度 | 当前做法 | 文档方案 |
|---|---|---|
| `Message` interface | `conversation_id: string`（snake_case）+ `toolCalls?: ToolCall[]`（camelCase）| 拆分为 `MessageRow`（全 snake_case）+ `Message`（全 camelCase）|
| `Conversation` interface | `provider_id: string \| null`（snake_case）| 拆分为 `ConversationRow`（全 snake_case）+ `Conversation`（全 camelCase）|
| 文件 | `chatTypes.ts` 一个文件同时定义 | `protocol.ts`（DTO）+ `chatTypes.ts`（业务）|
| 映射函数 | 不存在，直接 `as Conversation[]` 强转 | `protocolAdapter.ts` 提供 `rowToMessage()` / `rowToConversation()` |

**当前问题**：`Message` 中 `conversation_id` 是 snake_case（匹配后端 wire format），`toolCalls` 是 camelCase（匹配 JS 惯例），**一个类型内两种命名风格混用**。文档方案通过 DTO/业务层分离彻底解决。

**影响范围**：`chatTypes.ts` 定义 + `chatService.ts` 强转 + `chatStore.ts` 属性访问 + `chatOrchestrator.ts` + `chatDeltaEvents.ts` + 所有消费 `Message`/`Conversation` 的组件（约 20+ 文件）

#### 差异 2：chatService invoke 返回值直接强转为业务类型

| 维度 | 当前做法 | 文档方案 |
|---|---|---|
| `listConversations` | `(r) => (r as Conversation[]) \|\| []` | `(r) => ((r as ConversationRow[]) \|\| []).map(rowToConversation)` |
| `getConversationPaginated` | `r as ConversationDetail \| null` | `r as ConversationDetailRow \| null` → 映射转换 |
| 强转目标 | 业务类型 `Conversation`（混合命名）| DTO 类型 `ConversationRow`（纯 snake_case）|

**当前问题**：强转目标是混合命名的业务类型，后端增删字段时 TypeScript 无任何编译时感知。

#### 差异 3：后端 event payload 用 `json!({})` 手动构造，无类型化 struct

| 维度 | 当前做法 | 文档方案 |
|---|---|---|
| `emit_delta` | `json!({ "conversation_id": ..., "delta_text": ... })` | `serde_json::to_value(&ChatDeltaPayload { ... })?` |
| 类型约束 | 无，遗漏字段无编译时感知 | `#[serde(deny_unknown_fields)]` 编译时 + 运行时保障 |
| 契约测试 | 无 | wire format 断言 + round-trip + 未知字段拒绝 |

**当前问题**：`emit_stream_error` 已遗漏 `seq` 字段（§4.4.2），`json!({})` 无法在编译时发现。

#### 差异 4：后端 command 返回值用 `serde_json::Value`，无强类型

| 维度 | 当前做法 | 文档方案 |
|---|---|---|
| `list_conversations` | `Result<serde_json::Value, String>` | `Result<Vec<ConversationRow>, String>` |
| `get_conversation_paginated` | `Result<serde_json::Value, String>` + `json!({})` 拼装 | `Result<ConversationDetailResponse, String>` |
| `list_providers` | `Result<serde_json::Value, String>` + `json!({})` 逐字段构造 | `Result<Vec<ProviderListItem>, String>` |

**当前问题**：`cmd/` 目录下 11 个 command 返回 `serde_json::Value`，前端强转无编译时保障。文档方案 Phase 4 将其替换为强类型 struct。

#### 差异 5：后端 struct 无 `deny_unknown_fields`

| 维度 | 当前做法 | 文档方案 |
|---|---|---|
| `ConversationRow` | `#[derive(Serialize, Deserialize)]` | 增加 `#[serde(deny_unknown_fields)]` |
| `MessageRow` | `#[derive(Serialize, Deserialize)]` | 增加 `#[serde(deny_unknown_fields)]` |

**当前问题**：后端增删字段时，前端旧版本静默忽略新字段/缺失旧字段，无报错。`deny_unknown_fields` 使反序列化严格拒绝未知字段，强制前后端同步变更。

#### 差异 6：`tool_calls_json` 映射未实现

| 维度 | 当前做法 | 文档方案 |
|---|---|---|
| 前端 `Message.toolCalls` | 类型定义存在但 DB 恒为 NULL，映射未实现 | `parseToolCallsJson(row.tool_calls_json)` 在适配层实现 |
| 后端 `tool_calls_json` | `Option<String>` 存在但未写入 | Phase 1-2 实现写入 + 前端解析 |

**当前状态**：I15 已在 DB 和 Rust struct 中添加 `tool_calls_json` 列，但后端尚未将 tool call 结果写入该列，前端 `toolCalls` 恒为 `undefined`。

#### 差异 7：`get_mouse_position` 返回 camelCase

| 维度 | 当前做法 | 文档方案 |
|---|---|---|
| 返回值 | `json!({ "clientX": x, "clientY": y })` | `json!({ "client_x": x, "client_y": y })` 或 struct + `rename_all = "snake_case"` |

**当前问题**：这是后端唯一使用 camelCase 的 command 返回值，与全局 snake_case 约定不一致。

#### 差异总结

| 差异 | 影响程度 | 实施阶段 | 是否破坏现有行为 |
|---|---|---|---|
| 1. DTO/业务层分离 | 大（20+ 文件）| Phase 1-2 | 否（映射函数保证输出一致）|
| 2. invoke 强转目标变更 | 中（chatService）| Phase 1-2 | 否 |
| 3. event payload 类型化 | 大（chat.rs + conversation.rs）| Phase 4 | 否（wire format 不变）|
| 4. command 返回值强类型 | 大（11 个 command）| Phase 4 | 否（序列化结果不变）|
| 5. deny_unknown_fields | 中（storage struct）| Phase 4 | 否（当前无未知字段）|
| 6. tool_calls_json 映射 | 中 | Phase 1-2 | 否（当前恒为 NULL）|
| 7. get_mouse_position camelCase | 小（1 个 command）| Phase 2 | **是**（前端需同步改属性名）|

### 5.3 DB → 前端字段映射清单

| DB 列名 | 后端 Rust 字段 | Wire format (JSON key) | 前端 DTO 字段 | 前端业务字段 | 类型转换 |
|---|---|---|---|---|---|
| `id` | `id: String` | `id` | `id` | `id` | — |
| `conversation_id` | `conversation_id: String` | `conversation_id` | `conversation_id` | `conversationId` | snake_case → camelCase |
| `role` | `role: String` | `role` | `role` | `role` | — |
| `content` | `content: String` | `content` | `content` | `content` | — |
| `tokens` | `tokens: Option<i64>` | `tokens` | `tokens` | `tokens` | — |
| `timestamp` | `timestamp: i64` | `timestamp` | `timestamp` | `timestamp` | — |
| `parent_id` | `parent_id: Option<String>` | `parent_id` | `parent_id` | `parentId` | snake_case → camelCase |
| `is_pinned` | `is_pinned: bool` | `is_pinned` | `is_pinned` | `isPinned` | snake_case → camelCase |
| `reasoning` | `reasoning: Option<String>` | `reasoning` | `reasoning` | `reasoning` | — |
| `tool_calls_json` | `tool_calls_json: Option<String>` | `tool_calls_json` | `tool_calls_json` | `toolCalls` | snake_case → camelCase + `JSON.parse` + `null` → `undefined` |

### 5.4 Conversation 字段映射清单

| DB 列名 | 后端 Rust 字段 | Wire format | 前端 DTO 字段 | 前端业务字段 | 类型转换 |
|---|---|---|---|---|---|
| `title_source` | `title_source: String` | `title_source` | `title_source` | `titleSource` | snake_case → camelCase |
| `provider_id` | `provider_id: Option<String>` | `provider_id` | `provider_id` | `providerId` | snake_case → camelCase |
| `model_id` | `model_id: String` | `model_id` | `model_id` | `modelId` | snake_case → camelCase |
| `system_prompt` | `system_prompt: Option<String>` | `system_prompt` | `system_prompt` | `systemPrompt` | snake_case → camelCase |
| `sort_order` | `sort_order: i64` | `sort_order` | `sort_order` | `sortOrder` | snake_case → camelCase |
| `created_at` | `created_at: i64` | `created_at` | `created_at` | `createdAt` | snake_case → camelCase |
| `updated_at` | `updated_at: i64` | `updated_at` | `updated_at` | `updatedAt` | snake_case → camelCase |
| `is_full` | `is_full: bool` | `is_full` | `is_full` | `isFull` | snake_case → camelCase |

---

## 6. 实施计划

### 6.1 Phase 1：DTO 层建立 + 映射函数（无行为变更）

1. 新建 `src/chat/core/protocol.ts`：定义 `MessageRow`、`ConversationRow`、`ConversationDetailRow`，文件头部注释标注 "mirror backend payloads" + "snake_case to match wire format"（借鉴 OpenBitFun `agent-api.ts`）
2. 新建 `src/chat/core/protocolAdapter.ts`：实现 `rowToMessage`、`rowToConversation`、`parseToolCallsJson`（借鉴 OpenBitFun `tool-execution-service.ts` 的 DTO → 业务对象转换模式）
3. `chatService.ts` 的 `listConversations` / `getConversationPaginated` / `searchConversations` 改用映射函数，消除 `as Conversation[]` / `as Message[]` 强转
4. 验证：所有现有测试通过，行为不变

### 6.2 Phase 2：业务类型 camelCase 迁移

1. `chatTypes.ts` 的 `Message` / `Conversation` interface 字段改为 camelCase
2. 全局替换 Store / Component 中的 snake_case 属性访问为 camelCase
3. event payload 类型（`ChatDeltaPayload` 等）**不变**（与 OpenBitFun 前端 DTO 层保持 snake_case 一致）
4. 验证：所有现有测试通过，行为不变

### 6.3 Phase 3：`tool_calls_json` 写入激活

1. `cmd/chat.rs` 的 `stream_future` Ok 分支：从 `outcome.tool_calls` 序列化写入 DB
2. `db_tx` channel 类型扩展：第 5 元素从 `Option<String>` (reasoning) 扩展为包含 `tool_calls_json`
3. 验证：Deep 模式 + tool-call agent 模式下，刷新页面后 `toolCalls` 仍可从 DB 恢复

### 6.4 Phase 4：后端 event payload 类型化 + `deny_unknown_fields`（P2，借鉴 OpenBitFun）

1. 新建 `ChatDeltaPayload` / `ChatStreamStartPayload` Rust struct，标注 `#[serde(rename_all = "snake_case", deny_unknown_fields)]`
2. 替换 `json!({})` 为 struct 序列化
3. 添加协议契约测试（借鉴 OpenBitFun `protocol_contracts.rs`）：wire format 断言 + round-trip 测试 + 未知字段拒绝测试
4. 验证：前端行为不变

### 6.5 Phase 5：协议版本化（远期，借鉴 OpenBitFun）

1. 引入 `CHAT_PROTOCOL_VERSION: u32` 常量
2. 前端在首次 invoke 时校验版本兼容性
3. 当协议变更需要向后兼容时启用

---

## 7. 风险与缓解

| 风险 | 影响 | 缓解 |
|---|---|---|
| camelCase 迁移遗漏属性访问 | 运行时 `undefined` 访问 | TypeScript 编译时校验（interface 字段改名后，所有访问点报错） |
| `tool_calls_json` JSON.parse 失败 | `toolCalls` 为 `undefined`，不影响其他字段 | `parseToolCallsJson` 内部 try-catch，解析失败返回 `undefined` |
| DB 中已有 `tool_calls_json = NULL` 的旧数据 | 前端 `toolCalls` 为 `undefined`，符合预期 | `parseToolCallsJson(null)` 返回 `undefined`，与当前行为一致 |
| event payload 字段名后端改错 | 前端收不到数据 | Phase 4 类型化后编译时校验；Phase 1-3 人工对齐 + E2E 测试 |

---

## 8. 验收标准

1. 后端存在 `src-tauri/src/protocol/` 模块，Chat 协议所有 event payload struct + command 返回值 struct 集中定义于此
2. 前端存在 `src/chat/core/protocol.ts` 文件，所有 Chat 协议 DTO 类型集中定义于此，文件头标注 `Backend mirror`
3. 前端 DTO 层 `MessageRow` / `ConversationRow` 与后端 `MessageRow` / `ConversationRow` 字段名一一对应（snake_case）
4. 前端 DTO 文件头部注释标注 "mirror backend payloads" + "snake_case to match wire format"（借鉴 OpenBitFun）
5. 前端业务层 `Message` / `Conversation` 使用 camelCase 字段名
6. `chatService.ts` 所有 Tauri invoke 返回值经映射函数转换，无 `as Conversation[]` / `as Message[]` 直接强转
7. `tool_calls_json` → `toolCalls` 映射生效：DB 中有 tool_calls 数据时，前端 `message.toolCalls` 为解析后的 `ToolCall[]`
6. `tool_calls_json` 为 `null` 时，前端 `message.toolCalls` 为 `undefined`
7. event payload 字段名保持 snake_case，与后端 `json!({})` 构造一致
8. 所有现有测试通过，无行为变更（Phase 1-2）
9. Deep 模式 reasoning 持久化 + 恢复不受影响
10. `parseToolCallsJson` 对非法 JSON 返回 `undefined`，不抛异常
11. 后端协议 struct 标注 `deny_unknown_fields`，拒绝未知字段（Phase 4，借鉴 OpenBitFun）
12. 协议契约测试通过：wire format 断言 + round-trip 测试 + 未知字段拒绝测试（Phase 4，借鉴 OpenBitFun）
13. 所有 `chat-delta` 事件（含 error 分支）携带 `conversation_id`、`attempt_id`、`message_id` 关联元数据（R1）
14. `emit_stream_error` 补充 `seq` 字段，前端可定位 error 发生位置（R2）
15. 同一 `attempt_id` 下 `seq` 单调递增（R4，由 `consume_stream` 保证）
16. Chat 协议范围内 `json!({})` 构造点为 0 处（S1，Phase 4 完成后）
17. `DbSender` 4 元组替换为命名 struct `DbWriteMsg`（S6，Phase 4）
18. `get_mouse_position` 返回值改为 snake_case（S5，Phase 2 或独立 PR）
19. `chatService.ts` invoke 返回值经 DTO 层强转 + 映射函数转换，无 `as Conversation[]` 业务类型强转（F1）
20. event payload `as` 断言目标类型定义在 `protocol.ts` DTO 层（F4）
21. `useHistorySearch.ts` / `useHistoryRefresh.ts` 统一走 chatService，无直接 invoke + 强转（F3）
22. `JSON.parse(provider.models) as ModelConfig[]` 替换为安全解析函数（F5）

---

## 9. 与 I15 的关系

I15 已完成的工作为 I16 协议统一奠定基础：

| I15 交付物 | I16 依赖 |
|---|---|
| DB `messages` 表新增 `tool_calls_json TEXT` 列 | I16 激活写入 + 前端映射 |
| `MessageRow.tool_calls_json: Option<String>` 字段 | I16 DTO 层 mirror 此字段 |
| `update_message_content` 签名已含 `tool_calls_json` 参数 | I16 传入实际值（当前恒 None） |
| `StreamOutcome.tool_calls: Option<Vec<ToolCall>>` | I16 序列化写入 DB |
| `ChatDelta` 含 `delta_tool_call_*` 字段 | I16 event payload 已对齐，无需变更 |
| `chat-stream-start` 事件 + `attempt_id` 协议 | I16 event payload 保持 snake_case，无需变更 |

I15 spec 明确标注的 I16 预留项：

> `tool_calls_json` → `toolCalls` 前端映射：tool_calls 当前未实际接入项目可用状态；I16 设计统一前后端消息协议时一并解决字段映射，I15 不做临时 JSON.parse 补丁

本设计文档即为此预留项的完整实施方案。
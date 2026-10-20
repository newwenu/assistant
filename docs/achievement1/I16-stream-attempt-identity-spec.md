# I16 — 流尝试身份对齐规格说明

## 1. 交付目标

引入独立的**流尝试身份标识**（`attempt_id`），替代当前用 `message_id` 兼做流归属判据的错误做法，使前端能正确区分新旧请求的事件，解决 editMessage 重发和 regenerate 时的 Cancelled 误判及流不结束问题。

**设计原则**：
- **身份分离**：`message_id` 只做消息持久化身份，`attempt_id` 只做流事件归属标识，两者语义不混淆
- **后端生成**：`attempt_id` 由后端在 `send_chat_message` 入口生成，随每个 `chat-delta` 事件传递，前端从事件中获取而非自造
- **全帧过滤**：`handleDeltaEvent` 中所有事件类型（error / finish_reason / delta_reasoning / delta_tool_call / usage / delta_retry）统一按 `attempt_id` 过滤
- **start 帧协议**：后端在流开始前先发 `chat-stream-start` 事件（含 `attempt_id`），前端收到后记录为活跃 attempt，后续事件按此过滤
- **保持行为兼容**：`ChatDeltaPayload` 新增 `attempt_id` 字段（optional），旧事件无该字段时退化为不过滤，确保过渡期不破坏

> **前置依赖**：I15 ✅（chat runtime 已提取）
> **核心参考**：deepseek-harness-orb `AssistantStreamAttempt`（`LlmAttemptId` + `start`/`chunk`/`end` 帧协议）、OpenBitFun `round_executor`（`${round_id}:attempt:${attempt_number}` + 每事件携带 `attempt_id`）

---

## 2. 当前问题全景

### 2.1 ID 不对齐（根因）

| ID | 生成位置 | 格式 | 用途 |
|---|---|---|---|
| `streamingMessageId` | 前端 `chatOrchestrator.ts:186` | `temp-assistant-${Date.now()}` | 前端流状态标识 |
| `payload.message_id` | 后端 `chat.rs:69` | `Uuid::new_v4()` | 事件携带的消息持久化 ID |

两者**永远不匹配** → `payload.message_id === activeMsgId` 永远为 `false`。

### 2.2 所有不对齐点与潜在 Bug

下表逐点列出当前代码中每个涉及流事件归属判定的位置，标记其对齐状态和潜在 bug：

| # | 文件:行 | 当前逻辑 | 对齐参考项目？ | 潜在 Bug | 严重度 |
|---|---|---|---|---|---|
| **A1** | `chatOrchestrator.ts:186` | 前端生成 `temp-assistant-${Date.now()}` 作为 `assistantMsgId` | ❌ 参考项目前端不生成流身份 | 前端临时 ID 与后端 UUID 永远不匹配，过滤失效 | 🔴 致命 |
| **A2** | `chatOrchestrator.ts:233` | `const activeMsgId = deps.env.getStreamingMessageId()` 取前端临时 ID | ❌ 参考项目从 `start` 帧获得后端生成的 `attemptId` | 过滤条件基于错误 ID | 🔴 致命 |
| **A3** | `chatOrchestrator.ts:234-239` | `if (activeMsgId && payload.message_id === activeMsgId)` 过滤 error | ❌ 参考项目用 `attemptId` 过滤 | 所有 error 被静默丢弃（条件永远 false），包括新请求的真实 error | 🔴 致命 |
| **A4** | `chatOrchestrator.ts:283-287` | `finish_reason` 无 `message_id` 过滤，直接调 `finishStream()` | ❌ 参考项目 `end` 帧也按 `attemptId` 过滤 | 旧请求的 stale `finish_reason` 也会触发 `finishStream()`，提前结束新流 | 🔴 致命 |
| **A5** | `chatOrchestrator.ts:241-244` | `delta_retry` 无过滤 | ❌ 参考项目每帧按 `attemptId` 过滤 | 旧请求的 retry 通知可能覆盖新请求的状态 | 🟡 中 |
| **A6** | `chatOrchestrator.ts:246-254` | `delta_reasoning` 无过滤，仅检查 `streamingMessageId` 非 null | ❌ 参考项目按 `attemptId` 过滤 | 旧请求的 reasoning delta 可能追加到新请求的消息上 | 🟡 中 |
| **A7** | `chatOrchestrator.ts:256-268` | `delta_tool_call_*` 无过滤，仅检查 `streamingMessageId` 非 null | ❌ 参考项目按 `attemptId` 过滤 | 旧请求的 tool call delta 可能追加到新请求的消息上 | 🟡 中 |
| **A8** | `chatOrchestrator.ts:270-280` | `usage` 无过滤 | ❌ 参考项目按 `attemptId` 过滤 | 旧请求的 usage 可能覆盖新请求的 context usage 显示 | 🟡 中 |
| **A9** | `chatDeltaEvents.ts:56-58` | coalescer `push` 用 `message_id` 做合并 key | ⚠️ 参考项目用 `attemptId` 做 key | 旧请求和新请求的 text delta 可能因 `message_id` 不同而不合并（功能正确但效率低）；若 `message_id` 碰撞则错误合并 | 🟡 中 |
| **A10** | `streamCoalescer.ts:97-98` | 合并条件 `conversation_id + message_id` 匹配 | ⚠️ 应改为 `conversation_id + attempt_id` | 同 A9 | 🟡 中 |
| **A11** | `streamCoalescer.ts:120` | seqDedup key 用 `conversation_id:message_id` | ⚠️ 应改为 `conversation_id:attempt_id` | 旧请求和新请求的 seq 空间不同但 key 碰撞（同 conv 同 msg），可能误拒新请求的合法 delta | 🟡 中 |
| **A12** | `chatStore.ts:517-522` | `editMessage` 先 `cancelChat` + 清 `streamingMessageId=null`，再 `sendMessage` | ❌ 参考项目 cancel 和新 attempt 是原子切换 | cancel 和 resend 之间有时序窗口，`streamingMessageId` 为 null，所有到达的事件被丢弃 | 🔴 致命 |
| **A13** | `chatStore.ts:548-557` | `regenerateMessage` 同理：先删消息再 `sendMessage` | ❌ 同 A12 | 同 A12 | 🔴 致命 |
| **A14** | `chat_tasks.rs:33-36` | `start()` 隐式取消同 `conversation_id` 的旧 token | ⚠️ 参考项目用独立 `attempt_id`，旧 attempt 的 cancel 不影响新 attempt | 隐式取消导致旧流发 Cancelled error 到前端，若无正确过滤则误判 | 🟡 中（A14 本身合理，但需 A1-A8 的过滤修复配合） |
| **A15** | `chat.rs:69` | 后端生成 `assistant_msg_id = Uuid::new_v4()` 但不通知前端 | ❌ 参考项目后端通过 `start` 帧通知前端 | 前端永远不知道后端分配的真实消息 ID | 🔴 致命 |
| **A16** | `ChatDeltaPayload` 接口 | 无 `attempt_id` 字段 | ❌ 参考项目每帧携带 `attemptId` | 无法按 attempt 过滤 | 🔴 致命 |
| **A17** | `ChatEnvPort` 接口 | `getStreamingMessageId()` 返回前端临时 ID | ❌ 参考项目有 `getActiveAttemptId()` 返回后端分配的 ID | 接口语义错误 | 🔴 致命 |
| **A18** | `chatOrchestrator.ts:155-168` | `send()` 中 `beginSend` 传入前端临时 `assistantMsgId` | ❌ 参考项目 `beginSend` 不需要前端预造 ID | 临时 ID 被写入 `streamingMessageId`，后续过滤全失效 | 🔴 致命 |

### 2.3 Bug 因果链

```
A1(前端造临时ID) + A15(后端不通知前端真实ID)
  → A2(过滤基于错误ID) + A16(无attempt_id字段) + A17(接口语义错误)
    → A3(error过滤失效) + A4(finish无过滤) + A5-A8(其他事件无过滤)
      → Bug1: 旧请求 Cancelled 误判为新请求失败
      → Bug2: 加message_id过滤后正常流也无法结束(ID不对齐)

A12(editMessage时序窗口) + A13(regenerateMessage时序窗口)
  → cancel和resend之间streamingMessageId=null
    → 所有事件(包括新请求的)被丢弃

A14(隐式取消) 需要A3-A8的过滤修复配合才能正确工作
```

---

## 3. 参考项目做法

### 3.1 deepseek-harness-orb

**两个独立 branded type**，各司其职：

| 类型 | 语义 | 出现位置 |
|---|---|---|
| `MessageId` | 消息持久化身份 | `updateQueue(itemId)`, CRUD 操作 |
| `LlmAttemptId` | 流事件归属标识 | `start`/`chunk`/`end` 帧的 `attemptId` |

**帧协议**（3 种帧类型）：

```typescript
type SessionAssistantStreamFrame =
  | { type: 'start', attemptId: LlmAttemptId, revision, turn, step }
  | { type: 'chunk', attemptId: LlmAttemptId, revision, index, time, chunk }
  | { type: 'end',   attemptId: LlmAttemptId, revision, index, outcome }
```

**前端过滤**（`ClientAssistantStream.acceptFrame`）：

```typescript
case 'chunk': {
  if (attempt === undefined || attempt.attemptId !== frame.attemptId) return undefined
}
case 'end': {
  if (attempt === undefined || attempt.attemptId !== frame.attemptId) return undefined
}
```

**`LlmAttemptId` 生成**：后端 `AssistantStreamAttempt` 构造时 `LlmAttemptId(\`${sessionId}:${attempt}\`)`。

### 3.2 OpenBitFun

**事件体系中无 `message_id`**，每事件用三级定位 + `attempt_id`：

```rust
TextChunk {
    session_id: String,
    turn_id: String,
    round_id: String,
    attempt_id: Option<String>,  // 流归属标识
    attempt_index: Option<u32>,
    text: String,
}
```

**`attempt_id` 生成**：后端 `round_executor` 中 `format!("{round_id}:attempt:{attempt_number}")`。

**前端过滤**：`EventHandlerModule` 将 `attemptId` 路由到对应 `ModelRoundAttempt`，Store 按 `item.attemptId === attempt.id` 匹配。

### 3.3 共同模式

| 设计要素 | deepseek-harness-orb | OpenBitFun |
|---|---|---|
| 流身份标识 | `LlmAttemptId`（独立 branded type） | `attempt_id`（独立字段） |
| 标识生成方 | 后端 | 后端 |
| 标识传递方式 | 每帧携带 | 每事件携带 |
| start 帧协议 | ✅ 显式 `start` 帧 | ✅ `ModelRoundStarted` 事件 |
| 前端过滤 | `frame.attemptId === activeAttempt.attemptId` | `item.attemptId === attempt.id` |
| `message_id` 用于流过滤？ | ❌ 不用 | ❌ 不用 |

---

## 4. 修复方案

### 4.1 协议变更

#### 4.1.1 新增 `chat-stream-start` 事件

后端在 `send_chat_message` 中，**流开始前**发射：

```json
{
  "event": "chat-stream-start",
  "payload": {
    "conversation_id": "conv-xxx",
    "attempt_id": "conv-xxx:1",
    "message_id": "uuid-of-assistant-msg"
  }
}
```

- `attempt_id`：格式 `${conversation_id}:${attempt_counter}`，对齐 deepseek-harness-orb 的 `${sessionId}:${attempt}` 模式
- `attempt_counter`：由 `ChatTasks` 在 `start()` 时分配并返回（当前 `start()` 只返回 `CancellationToken`，需扩展）
- `message_id`：后端生成的 assistant 消息 UUID，前端用于将临时 ID 替换为真实 ID

#### 4.1.2 `chat-delta` 事件新增 `attempt_id` 字段

```json
{
  "event": "chat-delta",
  "payload": {
    "conversation_id": "conv-xxx",
    "attempt_id": "conv-xxx:1",
    "message_id": "uuid-of-assistant-msg",
    "delta_text": "...",
    "...": "..."
  }
}
```

`attempt_id` 为必填字段，所有 `chat-delta` 事件均携带。

### 4.2 后端变更

#### 4.2.1 `ChatTasks` 扩展

```rust
pub struct ChatTasks(pub Mutex<HashMap<String, ChatTaskEntry>>);

pub struct ChatTaskEntry {
    pub cancel_token: CancellationToken,
    pub attempt_counter: u32,  // 新增：单调递增的 attempt 计数器
}

impl ChatTasks {
    pub fn start(&self, conversation_id: String) -> (CancellationToken, String) {
        let token = CancellationToken::new();
        let mut map = self.lock_map();
        let (counter, attempt_id) = if let Some(old) = map.get_mut(&conversation_id) {
            old.cancel_token.cancel();
            old.attempt_counter += 1;
            old.cancel_token = token.clone();
            (old.attempt_counter, format!("{}:{}", conversation_id, old.attempt_counter))
        } else {
            let entry = ChatTaskEntry { cancel_token: token.clone(), attempt_counter: 1 };
            map.insert(conversation_id.clone(), entry);
            (1, format!("{}:1", conversation_id))
        };
        (token, attempt_id)
    }
}
```

#### 4.2.2 `send_chat_message` 发射 `chat-stream-start`

在 `chat_tasks.start()` 之后、流循环之前：

```rust
let (cancel_token, attempt_id) = chat_tasks.start(conversation_id.clone());

// 发射 start 事件
orb_window.emit("chat-stream-start", json!({
    "conversation_id": conversation_id,
    "attempt_id": attempt_id,
    "message_id": assistant_msg_id,
}))?;
```

#### 4.2.3 所有 `chat-delta` emit 添加 `attempt_id`

流循环中所有 `orb_window.emit("chat-delta", ...)` 的 payload 添加 `"attempt_id": attempt_id.clone()`。

### 4.3 前端变更

#### 4.3.1 `ChatDeltaPayload` 新增 `attempt_id`

```typescript
export interface ChatDeltaPayload {
  conversation_id: string;
  attempt_id: string;      // 新增：流尝试身份标识
  message_id: string;       // 保留：消息持久化身份
  seq?: number;
  delta_text?: string;
  // ...其余不变
}
```

#### 4.3.2 `ChatStreamStartPayload` 新增接口

```typescript
export interface ChatStreamStartPayload {
  conversation_id: string;
  attempt_id: string;
  message_id: string;
}
```

#### 4.3.3 `ChatEnvPort` 替换 `getStreamingMessageId`

```typescript
export interface ChatEnvPort {
  getActiveConversationId: () => string | null;
  getIsStreaming: () => boolean;
  getStreamingMessageId: () => string | null;   // 保留：用于 reasoning/toolCall 追加
  getActiveAttemptId: () => string | null;       // 新增：流事件归属过滤
  getMessages: () => Message[];
  getConversations: () => Conversation[];
  getSelectedModeId: () => string | null;
}
```

#### 4.3.4 `chatStore` 新增 `activeAttemptId` 状态

```typescript
interface ChatState {
  // ...现有字段
  activeAttemptId: string | null;  // 新增
}
```

`statePort.beginSend` 签名不变（仍接受 `assistantMsgId`），但 `activeAttemptId` 初始为 null，等 `chat-stream-start` 事件到达时设置。

新增 `statePort.setActiveAttemptId(attemptId: string, realMessageId: string)`：
- 设置 `activeAttemptId = attemptId`
- 将 `streamingMessageId` 从 `temp-assistant-xxx` 替换为 `realMessageId`（后端 UUID）
- 同时更新 `messages` 数组中对应消息的 `id`

#### 4.3.5 `chatDeltaEvents.ts` 监听 `chat-stream-start`

```typescript
track(
  listen("chat-stream-start", (event) => {
    if (disposed) return;
    const payload = event.payload as ChatStreamStartPayload;
    const store = useChatStore.getState();
    if (store.currentConversationId === payload.conversation_id) {
      store.setActiveAttemptId(payload.attempt_id, payload.message_id);
    }
  }),
);
```

#### 4.3.6 `handleDeltaEvent` 统一 `attempt_id` 过滤

```typescript
function handleDeltaEvent(payload: ChatDeltaPayload) {
  const activeAttemptId = deps.env.getActiveAttemptId();

  // 所有事件类型统一过滤：attempt_id 不匹配则忽略
  if (activeAttemptId && payload.attempt_id !== activeAttemptId) {
    return;
  }

  // 以下逻辑不变，但不再需要 message_id 过滤
  if (payload.error) {
    setPhase("error");
    deps.state.errorStream(toChatError(payload.error));
    return;
  }

  if (payload.delta_retry) {
    deps.state.setRetryStatus(payload.delta_retry);
    return;
  }

  // ... reasoning, tool_call, usage, finish_reason 处理不变
  // 但不再需要单独的 message_id 检查
}
```

#### 4.3.7 `CoalescibleDelta` 和 `streamCoalescer` 更新

`CoalescibleDelta` 新增 `attempt_id`，合并 key 和 seqDedup key 从 `conversation_id:message_id` 改为 `conversation_id:attempt_id`。

#### 4.3.8 `editMessage` / `regenerateMessage` 时序修复

`cancelChat` 后不再清 `streamingMessageId` 和 `activeAttemptId`（保留旧值直到 `chat-stream-start` 到达时替换），消除时序窗口。

---

## 5. 变更影响矩阵

| 变更 | 影响文件 | 影响范围 | 风险 |
|---|---|---|---|
| `ChatTasks` 扩展 | `chat_tasks.rs` | 内部结构变更，`start()` 返回值变更 | 低：仅 `chat.rs` 消费 |
| `chat-stream-start` 事件 | `chat.rs`, `chatDeltaEvents.ts` | 新增事件，不破坏现有 | 低：增量添加 |
| `chat-delta` 新增 `attempt_id` | `chat.rs`, `ChatDeltaPayload` | payload 新增字段 | 低：optional 兼容 |
| `activeAttemptId` 状态 | `chatStore.ts`, `ChatEnvPort` | 新增状态和接口方法 | 低：增量添加 |
| `handleDeltaEvent` 过滤逻辑 | `chatOrchestrator.ts` | 核心逻辑变更 | 中：需充分测试 |
| `streamingMessageId` 替换为真实 ID | `chatStore.ts` | `beginSend` / `setActiveAttemptId` | 中：需确保 UI 消息 ID 一致性 |
| coalescer key 变更 | `streamCoalescer.ts`, `chatDeltaEvents.ts` | 合并/去重逻辑 | 低：key 语义更正确 |
| `editMessage` 时序修复 | `chatStore.ts` | cancel 后不清 ID | 中：需验证无副作用 |

---

## 6. 测试要点

### 6.1 单元测试（`chatOrchestrator.test.ts`）

| 测试 | 验证 |
|---|---|
| `handleDeltaEvent` 按 `attempt_id` 过滤 error | stale attempt_id 的 error 被忽略，active attempt_id 的 error 被处理 |
| `handleDeltaEvent` 按 `attempt_id` 过滤 finish_reason | stale attempt_id 的 finish 被忽略，active 的 finish 调用 `finishStream` |
| `handleDeltaEvent` 按 `attempt_id` 过滤 reasoning/toolCall/usage/retry | stale 事件被忽略 |
| `activeAttemptId` 为 null 时不过滤 | 向后兼容：无 start 帧时所有事件正常处理 |
| `send()` 不再依赖前端临时 ID 做过滤 | `streamingMessageId` 仅用于 UI 渲染，不参与事件归属判定 |

### 6.2 集成测试（手动）

| 场景 | 验证 |
|---|---|
| 正常发送 → 流式接收 → finish | 流正常结束，`finishStream` 被调用 |
| editMessage 重发 | 旧请求 Cancelled 被忽略，新请求正常流式接收和结束 |
| regenerateMessage | 同上 |
| 流式过程中切换对话 | 旧对话事件被忽略 |
| 网络错误 | error 事件被正确处理 |
| 快速连续 editMessage | 每次 cancel → resend 的事件归属正确 |

---

## 7. 不在范围内

- **tool-call 执行循环**：属于 I16 后续迭代，本次只修复流事件归属
- **`LlmAttemptId` branded type**：TypeScript 端用普通 `string` 即可，无需 branded type（参考项目用 branded 是因为跨包边界严格性要求，我们项目规模不需要）
- **durable/transient 帧分离**：deepseek-harness-orb 的完整帧协议（rebaseline / settlement / abandonment）超出本次范围，我们只需 `start` + `attempt_id` 过滤
- **多轮 attempt 重试**：OpenBitFun 的 `attempt_number` 自动递增和 `ModelRoundAttemptSuperseded` 事件超出本次范围
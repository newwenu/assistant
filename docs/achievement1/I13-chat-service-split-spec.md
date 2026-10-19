# I13 — Chat 服务层分离 规格说明

## 1. 交付目标

将 `chatStore.ts` 中混杂的 **IPC 调用 + 类型断言 + 业务编排** 三种职责分离为三层，使每一层可独立演进，为将来 Tool Call Agent Loop 扩展打下基础。

> **前置依赖**：无（纯前端重构，不涉及 Rust 端变更）

## 2. 当前问题

| 类别 | 问题 | 影响 |
|------|------|------|
| 职责混合 | `chatStore.ts` 701 行，同时承担 IPC 调用（21 处 `invoke`）、返回值类型断言（`as Conversation[]` 等 6 处）、业务编排（`sendMessage` 80 行）、纯状态更新、跨 Store 读取 | 修改任一职责需通读全文件，改 IPC 协议易遗漏 |
| 事件处理绕过 Store | `chatDeltaEvents.ts` 直接 `store.setState()` 修改 `isReasoningStreaming`、`messages`（tool call 拼接），绕过 store action | 状态变更路径不透明，调试困难 |
| 类型不安全 | IPC 返回值全靠 `as` 断言，无编译期校验 | Rust 端改签名，前端不报错，运行时才崩 |
| Tool Call 死路 | `sendMessage` 发出 `invoke("send_chat_message")` 后流直接结束；`finishStream` 不区分 `finish_reason`；前端无工具执行循环 | 无法支持 Agent 模式 |
| `tauriInvoke.ts` 空壳 | 仅 13 行，加了个 console.error，无任何 chat 专用封装 | 所有 store 都裸调 `invoke` |

## 3. 架构设计

### 3.1 三层职责划分

```
┌─────────────────────────────────────────────────────┐
│  Layer 3: chatStore.ts  — 纯状态容器                 │
│  职责：state shape + 简单 set/get action             │
│  不含：invoke、业务编排、跨 Store 读取                │
├─────────────────────────────────────────────────────┤
│  Layer 2: chatOrchestrator.ts  — 对话生命周期编排    │
│  职责：sendMessage 编排、流式状态机、事件处理分发     │
│  不含：invoke、直接 set store（通过回调通知 store）   │
├─────────────────────────────────────────────────────┤
│  Layer 1: chatService.ts  — IPC 协议层              │
│  职责：类型安全的 invoke 封装 + 返回值转换            │
│  不含：业务逻辑、store 读取                           │
└─────────────────────────────────────────────────────┘
```

### 3.2 层间依赖（无循环）

```
chatDeltaEvents.ts ──→ chatOrchestrator.ts ──→ chatService.ts ──→ invoke
                         ↑                         ↑
chatStore.ts ────────────┘                         |
  (持有 orchestrator 引用)                          |
  (orchestrator 通过 onXxx 回调写 store)            |
useAgentModeStore ─────────────────────────────────┘
  (orchestrator 读 modeId)
```

### 3.3 各层变更预测

| 将来变化 | chatService | chatOrchestrator | chatStore |
|---------|-------------|------------------|-----------|
| 新增 IPC 命令 | **加方法** | 调用新方法 | 不变 |
| Tool call 执行循环 | 加 `executeTool` | **加状态机 phase + 编排逻辑** | 加 `toolCallStates` 字段 |
| 新增 delta 事件类型 | 不变 | **加 handler 分支** | 加对应状态字段 |
| 修改流式合并策略 | 不变 | **改 delta handler** | 不变 |
| 修改 UI 状态展示 | 不变 | 不变 | **改 state shape** |

## 4. 实施步骤

### Step 1：创建 `chatService.ts` — IPC 协议层 ✅ 已完成

**目标**：将全部裸 `invoke` + `as` 类型断言集中到一个文件，提供类型安全的调用接口。

**产出文件**：
- `src/services/chatService.ts` — 18 个方法，覆盖 chatStore 全部 26 处 invoke 调用点
- `src/services/__tests__/chatService.test.ts` — 28 个契约测试用例

**chatStore.ts 替换结果**：
- `import { invoke }` 已移除，改为 `import { chatService }`
- 全部 26 处 `invoke(...)` 替换为 `chatService.xxx(...)` 调用
- 零 invoke 残留，TypeScript 编译零错误

**类型修复记录**（原始 invoke → chatService 签名对齐过程）：

| 问题 | 原始 invoke 传参类型 | chatService 初始签名 | 修复后签名 |
|------|---------------------|---------------------|-----------|
| `modeId` null/undefined 不兼容 | `selectedModeId: string \| null`（agentModeStore） | `modeId?: string`（= `string \| undefined`） | `modeId?: string \| null` |
| `getConversationPaginated` id 类型 | `state.currentConversationId: string \| null` | `id: string \| null` ✅ | 无需修复 |
| `getConversationTokenUsage` contextWindowTokens | `resolveContextWindowTokens() → number \| undefined` | `contextWindowTokens: number \| undefined` ✅ | 无需修复 |

**chatService 方法与原始 invoke 契约对照表**：

| # | chatService 方法 | invoke 命令名 | 参数 | 返回值变换 |
|---|-----------------|-------------|------|-----------|
| 1 | `listConversations()` | `list_conversations` | `{ providerId: null }` | `(r as Conversation[]) \|\| []` |
| 2 | `listProviders()` | `list_providers` | 无 | `(r as ProviderBrief[]) \|\| []` |
| 3 | `createConversation(p, m, mode?)` | `create_conversation` | `{ providerId, modelId, modeId }` | 泛型 `invoke<string>` |
| 4 | `getConversationPaginated(id, limit, ts)` | `get_conversation_paginated` | `{ id, limit, beforeTimestamp }` | `r as ConversationDetail \| null` |
| 5 | `getConversationTokenUsage(id, tokens)` | `get_conversation_token_usage` | `{ conversationId, contextWindowTokens }` | 泛型 `invoke<TokenUsageSummary>` |
| 6 | `deleteConversation(id)` | `delete_conversation` | `{ id }` | void |
| 7 | `renameConversation(id, title, src)` | `rename_conversation` | `{ id, title, titleSource }` | void |
| 8 | `updateConversationModel(id, modelId)` | `update_conversation_model` | `{ id, modelId }` | void |
| 9 | `sendChatMessage(convId, content, mode?)` | `send_chat_message` | `{ conversationId, content, modeId }` | void |
| 10 | `cancelChat(conversationId)` | `cancel_chat` | `{ conversationId }` | void |
| 11 | `incrementGeneration(id)` | `increment_generation` | `{ id }` | void |
| 12 | `deleteMessagePersist(messageId)` | `delete_message_persist` | `{ messageId }` | void |
| 13 | `editMessage(messageId, newContent)` | `edit_message` | `{ messageId, newContent }` | void |
| 14 | `regenerateMessage(convId, msgId)` | `regenerate_message` | `{ conversationId, messageId }` | void |
| 15 | `toggleConversationPin(id)` | `toggle_conversation_pin` | `{ id }` | void |
| 16 | `duplicateConversation(id)` | `duplicate_conversation` | `{ id }` | void |
| 17 | `batchDeleteConversations(ids)` | `batch_delete_conversations` | `{ ids }` | void |
| 18 | `purgeConversations()` | `purge_conversations` | 无 | void |

**chatStore 中 invoke → chatService 替换对照**（26 处）：

| chatStore 方法 | 原始 invoke | 替换为 chatService 调用 |
|---------------|------------|----------------------|
| `loadConversations` | `invoke("list_conversations", { providerId: null })` + `as` 断言 | `chatService.listConversations()` |
| `ensureCurrentConversation` | `invoke("list_providers")` + `as` 断言 | `chatService.listProviders()` |
| `ensureCurrentConversation` | `invoke<string>("create_conversation", { p.id, p.default_model })` | `chatService.createConversation(p.id, p.default_model \|\| "")` |
| `createConversation` | `invoke<string>("create_conversation", { providerId, modelId })` | `chatService.createConversation(providerId, modelId)` |
| `switchConversation` | `invoke("cancel_chat", { conversationId })` | `chatService.cancelChat(prevConvId)` |
| `switchConversation` | `invoke("get_conversation_paginated", ...)` + `as` 断言 | `chatService.getConversationPaginated(id, limit, null)` |
| `switchConversation` | `invoke<...>("get_conversation_token_usage", ...)` | `chatService.getConversationTokenUsage(id, resolveContextWindowTokens(...))` |
| `deleteConversation` | `invoke("delete_conversation", { id })` | `chatService.deleteConversation(id)` |
| `renameConversation` | `invoke("rename_conversation", { id, title, titleSource })` | `chatService.renameConversation(id, title, titleSource \|\| "user")` |
| `updateModel` | `invoke("update_conversation_model", { id, modelId })` | `chatService.updateConversationModel(id, modelId)` |
| `sendMessage` (cancel) | `invoke("cancel_chat", { conversationId })` | `chatService.cancelChat(state.currentConversationId)` |
| `sendMessage` (list) | `invoke("list_providers")` + `as` 断言 | `chatService.listProviders()` |
| `sendMessage` (create) | `invoke<string>("create_conversation", { p.id, p.default_model, modeId })` | `chatService.createConversation(p.id, p.default_model \|\| "", modeId)` |
| `sendMessage` (send) | `invoke("send_chat_message", { conversationId, content, modeId })` | `chatService.sendChatMessage(convId, content, modeId)` |
| `cancelStream` | `invoke("cancel_chat", { conversationId })` | `chatService.cancelChat(state.currentConversationId)` |
| `finishStream` | `invoke("increment_generation", { id })` | `chatService.incrementGeneration(convId)` |
| `finishStream` | `invoke<...>("get_conversation_token_usage", ...)` | `chatService.getConversationTokenUsage(convId, resolveContextWindowTokens(...))` |
| `deleteMessage` | `invoke("delete_message_persist", { messageId })` | `chatService.deleteMessagePersist(id)` |
| `editMessage` (cancel) | `invoke("cancel_chat", { conversationId })` | `chatService.cancelChat(state.currentConversationId)` |
| `editMessage` (edit) | `invoke("edit_message", { messageId, newContent })` | `chatService.editMessage(messageId, newContent)` |
| `regenerateMessage` | `invoke("regenerate_message", { conversationId, messageId })` | `chatService.regenerateMessage(convId, messageId)` |
| `loadMoreMessages` | `invoke("get_conversation_paginated", ...)` + `as` 断言 | `chatService.getConversationPaginated(currentConversationId, limit, ts)` |
| `togglePin` | `invoke("toggle_conversation_pin", { id })` | `chatService.toggleConversationPin(id)` |
| `duplicateConversation` | `invoke("duplicate_conversation", { id })` | `chatService.duplicateConversation(id)` |
| `batchDelete` | `invoke("batch_delete_conversations", { ids })` | `chatService.batchDeleteConversations(ids)` |
| `purgeAll` | `invoke("purge_conversations")` | `chatService.purgeConversations()` |

### Step 2：创建 `chatOrchestrator.ts` — 对话生命周期编排 ✅ 已完成

**目标**：将 `sendMessage` 的 80 行编排逻辑 + `chatDeltaEvents.ts` 的事件处理逻辑提取到独立模块，通过 Port 接口通知 store。

#### 2.1 参考项目模式对比

| 参考项目 | 模式 | 核心设计 | 与本项目适配度 |
|---------|------|---------|-------------|
| **airi** `chat-orchestrator-runtime.ts` | **Port 接口 + 工厂函数** | `createChatOrchestratorRuntime(deps)` 接收 `ChatOrchestratorRuntimeDeps`（含 `session`/`foregroundStream`/`llm` 端口），返回 `ChatOrchestratorRuntime`（含 `ingest`/`cancelPendingSends`/`hooks`）。orchestrator 不 import 任何 store，所有状态写入通过 `deps.foregroundStream.patch()` / `deps.session.appendSessionMessage()` 等端口方法。 | ⭐⭐⭐ 最适配——本项目也是 Tauri + Zustand，Port 接口让 orchestrator 可独立测试 |
| **deepseek-harness-orb** `ConversationController` | **Service 类 + 作用域寻址** | `ConversationController extends Service`，通过 `this.scopedSession('send')` 获取当前会话的 SessionFace，调 `session.prompt()` / `session.cancel()`。依赖注入通过 cordis Context 的 Service tracker。 | ⭐⭐ 思路一致但 cordis 框架太重，本项目不使用 |
| **openhuman** `orchestrator_tools.rs` | **声明式子代理编排** | orchestrator 不直接编排对话流，而是为每个子代理合成一个命名 tool（`manage_tasks`/`create_image`），LLM 通过 function calling 选择委托目标。 | ⭐ Rust 端设计，前端不涉及，但 tool call 合成思路可参考 |

**结论**：采用 **airi 的 Port 接口 + 工厂函数** 模式，原因：
1. orchestrator 零 store 依赖 → 可独立单元测试（airi 测试中 mock 全部端口，零 Pinia/Vue 依赖）
2. Port 接口天然适配将来 tool call 循环——新增 `toolExecPort` 即可，不改 store
3. 工厂函数比类更轻量，与本项目 Zustand 风格一致

#### 2.2 核心设计 — Port 接口

```typescript
// src/services/chatOrchestrator.ts

// ---- orchestrator 写状态的端口（store 实现此接口）----
export interface ChatStatePort {
  setStreaming: (streaming: boolean, messageId: string | null) => void;
  appendDelta: (delta: string) => void;
  setReasoningStreaming: (streaming: boolean) => void;
  appendReasoningDelta: (messageId: string, delta: string) => void;
  appendToolCallDelta: (messageId: string, delta: ToolCallDelta) => void;
  updateContextUsage: (usage: ContextUsage) => void;
  finishStream: (usage?: TokenUsage) => void;
  errorStream: (error: ChatError) => void;
}

// ---- orchestrator 读外部状态的端口 ----
export interface ChatEnvPort {
  getActiveConversationId: () => string | null;
  getIsStreaming: () => boolean;
  getMessages: () => Message[];
  getConversations: () => Conversation[];
  getSelectedModeId: () => string | null;
  resolveContextWindowTokens: (modelId: string, providerId: string | null) => number | undefined;
}

// ---- orchestrator 依赖注入 ----
export interface ChatOrchestratorDeps {
  state: ChatStatePort;
  env: ChatEnvPort;
  // store 的编排 action（orchestrator 触发后 store 执行副作用）
  onConversationCreated?: (id: string) => Promise<void>;
  onConversationsChanged?: () => Promise<void>;
}

// ---- orchestrator 公开 API ----
export interface ChatOrchestrator {
  send: (content: string) => Promise<void>;
  cancel: () => Promise<void>;
  handleDeltaEvent: (payload: ChatDeltaPayload) => void;
  getSendInProgress: () => boolean;
}
```

#### 2.3 核心设计 — ChatPhase 状态机

```typescript
type ChatPhase =
  | "idle"
  | "sending"        // 用户点击发送，IPC 已调
  | "streaming"      // 流式输出中
  | "tool_executing" // [将来] 工具执行中
  | "error";
```

状态机转换：
- `idle` → `sending`：用户点击发送
- `sending` → `streaming`：收到第一个 delta
- `streaming` → `idle`：收到 finish
- `streaming` → `error`：收到 error
- `*` → `idle`：用户取消
- `streaming` → `tool_executing`：[将来] 收到 tool_call finish
- `tool_executing` → `sending`：[将来] 工具结果回传，发起下一轮

#### 2.4 工厂函数

```typescript
export function createChatOrchestrator(deps: ChatOrchestratorDeps): ChatOrchestrator {
  let sendInProgress = false;
  let phase: ChatPhase = "idle";

  async function send(content: string) { /* 编排逻辑 */ }
  async function cancel() { /* 取消逻辑 */ }
  function handleDeltaEvent(payload: ChatDeltaPayload) { /* 事件分发 */ }

  return { send, cancel, handleDeltaEvent, getSendInProgress: () => sendInProgress };
}
```

#### 2.5 从 chatStore 移出的逻辑

| 来源 | 逻辑 | 移到 orchestrator |
|------|------|-----------------|
| `chatStore.sendMessage` | cancel 前流 | `send()` 开头 |
| `chatStore.sendMessage` | 确保对话存在（listProviders + createConversation） | `send()` 中 |
| `chatStore.sendMessage` | 创建临时 user/assistant 消息 + 设流式状态 | `send()` 中，通过 `state.setStreaming()` |
| `chatStore.sendMessage` | 自动标题 fallbackTitleFromUserMessage | `send()` 中，通过 `onConversationCreated` |
| `chatStore.sendMessage` | 调 `chatService.sendChatMessage` | `send()` 中 |
| `chatStore.cancelStream` | cancel 逻辑 | `cancel()` |
| `chatDeltaEvents.ts` | reasoning 拼接（`store.setState`） | `handleDeltaEvent()` → `state.appendReasoningDelta()` |
| `chatDeltaEvents.ts` | tool call 拼接（`store.setState`） | `handleDeltaEvent()` → `state.appendToolCallDelta()` |
| `chatDeltaEvents.ts` | usage 更新（`store.setState`） | `handleDeltaEvent()` → `state.updateContextUsage()` |
| `chatDeltaEvents.ts` | finish 处理 | `handleDeltaEvent()` → `state.finishStream()` |
| `chatDeltaEvents.ts` | error 处理 | `handleDeltaEvent()` → `state.errorStream()` |
| 模块级 `let sendInProgress` | 发送互斥 | orchestrator 内部状态 |

#### 2.6 chatDeltaEvents.ts 改动

**改动前**（4 处 `store.setState` 绕过 action）：
```typescript
store.setState({ isReasoningStreaming: false });
store.setState({ isReasoningStreaming: true, messages: ... });
store.setState({ messages: ... });  // tool call
store.setState({ contextUsage: ... });  // usage
```

**改动后**（解析 payload → 传给 orchestrator）：
```typescript
// chatDeltaEvents.ts 只做事件订阅 + payload 解析
orchestrator.handleDeltaEvent(parsedPayload);
// orchestrator 内部通过 state port 写 store
```

#### 2.7 chatStore 改动

store 实现 `ChatStatePort` + `ChatEnvPort`，创建 orchestrator 实例：

```typescript
// chatStore.ts
const orchestrator = createChatOrchestrator({
  state: {
    setStreaming: (streaming, messageId) => set({ isStreaming: streaming, streamingMessageId: messageId, ... }),
    appendDelta: (delta) => set({ streamingContent: get().streamingContent + delta }),
    // ... 其余端口方法
  },
  env: {
    getActiveConversationId: () => get().currentConversationId,
    getIsStreaming: () => get().isStreaming,
    // ...
  },
});

// store action 变为简单委托
sendMessage: (content) => orchestrator.send(content),
cancelStream: () => orchestrator.cancel(),
```

#### 2.8 测试策略（参照 airi）

airi 的 `chat-orchestrator-runtime.test.ts` 创建 harness 时 mock 全部端口：
```typescript
const runtime = createChatOrchestratorRuntime({
  session: { ensureSession: vi.fn(), getSessionMessages: () => [], appendSessionMessage: vi.fn(), ... },
  foregroundStream: { patch: vi.fn(), reset: vi.fn() },
  llm: { stream: vi.fn() },
  // ...
});
```

本项目同理：mock `ChatStatePort` + `ChatEnvPort`，验证 orchestrator 在各 phase 下调了正确的端口方法。

### Step 3：瘦身 `chatStore.ts` — 流式状态分离 + 编排逻辑上移 ⬜ 未开始

**目标**：将流式状态拆为独立 store，将 `finishStream` 中的编排副作用上移到 orchestrator，从公开 API 中移除仅被 orchestrator 内部使用的 Port 方法，使 chatStore 回归"持久化对话状态 + 副作用 action"的单一职责。

**当前行数**：635 行 → **目标行数**：chatStore ~400 行 + chatStreamStore ~80 行。

**当前文件行数现状**：

| 文件 | 行数 | 职责 |
|------|------|------|
| `chatStore.ts` | 635 | 状态容器 + Port 实现 + 副作用 action |
| `chatOrchestrator.ts` | 289 | 对话生命周期编排 |
| `chatDeltaEvents.ts` | 108 | 事件订阅 + payload 解析 |
| `chatService.ts` | 102 | IPC 协议层 |

#### 3.0 参考项目模式对比

| 参考项目 | 模式 | 核心设计 | 与本项目适配度 |
|---------|------|---------|-------------|
| **airi** `stores/chat/stream-store.ts` | **流式状态独立 store** | `useChatStreamStore` 持有 `streamingMessage` ref，暴露 `beginStream`/`appendStreamLiteral`/`finalizeStream`/`resetStream`。orchestrator runtime 通过 `deps.foregroundStream.patch()` 写流式状态，通过 `deps.session.appendSessionMessage()` 写持久化消息。chat 主 store（`chat.ts`）仅做 orchestrator 实例化 + Port 连接 + 公开 API 暴露，不含任何编排逻辑。 | ⭐⭐⭐ 最适配——本项目 Zustand 等价于 Pinia setup store，拆分模式可直接移植 |
| **airi** `stores/chat/session-store.ts` | **会话持久化独立 store** | `useChatSessionStore` 持有 `sessions` map + `activeSessionId`，暴露 `getSessionMessages`/`appendSessionMessage`/`ensureSession`。主 store 通过 `chatSession.xxx()` 读写。 | ⭐⭐ 思路一致，但本项目对话列表 + 消息列表目前耦合在同一个 ChatState，拆分会涉及较多消费者改动，Step 3 暂不拆 |

**结论**：Step 3 采用 airi 的 **流式状态独立 store** 模式，原因：
1. 流式状态（`isStreaming`/`streamingContent`/`streamingMessageId`/`isReasoningStreaming`）与持久化对话状态（`conversations`/`messages`/`currentConversationId`）生命周期完全不同——流式状态在 send→finish 间瞬态，持久化状态跨会话存活
2. 拆分后 chatStore 的 `ChatState` 接口不再包含 `appendDelta`/`finishStream`/`errorStream`，公开 API 面积最小化
3. chatStreamStore 可被 orchestrator 直接引用为 Port 实现，无需通过 chatStore 闭包间接访问

#### 3.0.1 已发现 Bug：流式状态重置遗漏

当前 `chatStore.ts` 中有 3 处 `set()` 重置流式状态时**遗漏 `streamingMessageId` 和 `isReasoningStreaming`**，与 `cancelStreaming`/`finishStream`/`errorStream` 的完整 4 字段重置模式不一致：

| 方法 | 行号 | 重置的字段 | 遗漏的字段 | 后续是否覆盖 | 实际影响 |
|------|------|-----------|-----------|------------|---------|
| `deleteConversation` | L413-414 | `isStreaming` + `streamingContent` | `streamingMessageId` + `isReasoningStreaming` | ✅ 后续 `switchConversation` L365-368 完整重置 | ❌ 无可见影响（短暂窗口内 messages 为空） |
| `batchDelete` | L602-603 | 同上 | 同上 | ✅ 同上 | ❌ 同上 |
| `purgeAll` | L617-618 | 同上 | 同上 | ❌ **永不覆盖**（purgeAll 后不调 loadConversations/switchConversation） | ⚠️ **isReasoningStreaming 残留 true → `deriveMascotFaceFromChat` 返回 "thinking" → mascot 卡死在思考表情** |

**Bug 触发条件**：用户在 reasoning streaming 进行中点击"清空全部对话" → `purgeAll` 只重置 `isStreaming: false` + `streamingContent: ""`，`isReasoningStreaming` 残留 `true` → mascot 表情永远显示"思考中"。

**修复方案**：Step 3 批项 A 实施时统一修复，将 3 处不完整重置改为调用 `useChatStreamStore.getState().cancelStreaming()`（完整重置 4 字段）。同时在 Step 3 前可单独修复此 bug：将 3 处 `set({ isStreaming: false, streamingContent: "" })` 补全为 `set({ isStreaming: false, streamingContent: "", streamingMessageId: null, isReasoningStreaming: false })`。

#### 3.0.2 实施顺序决策

**建议顺序**：**B → A → C → D**

| 顺序 | 批项 | 理由 |
|------|------|------|
| 1 | B（finishStream 编排上移） | 先让 finishStream 变成纯 set，拆到 chatStreamStore 时逻辑更干净，无需跨 store 编排 |
| 2 | A（chatStreamStore 拆分） | finishStream 已是纯 set，拆分不涉及跨 store 编排副作用 |
| 3 | C（API 最小化 + envPort 内联） | A 完成后 appendDelta/finishStream/errorStream 已在 chatStreamStore，从 ChatState 移除顺理成章 |
| 4 | D（withLoading helper） | 纯重构，风险最低，放最后 |

#### 3.0.3 关键设计决策

| # | 决策点 | 结论 | 理由 |
|---|--------|------|------|
| 1 | `finishStream` 中"合并 streamingContent 到 messages"的逻辑放哪？ | **留在 chatStreamStore，通过回调通知 chatStore 写 messages** | finishStream 需要读 `streamingMessageId` + `streamingContent`（chatStreamStore）并写 `messages`（chatStore）。方案：chatStreamStore 的 `finishStream` 执行纯流式状态重置 + 返回合并数据，通过 `onFinishMerge` 回调让 chatStore 更新 messages |
| 2 | `useMascotChatSync` 如何同时监听两个 store？ | **两个独立 subscribe** | `useChatStore.subscribe` 监听持久化状态（messages），`useChatStreamStore.subscribe` 监听流式状态（isStreaming/isReasoningStreaming），各自触发回调逻辑 |
| 3 | `withLoading` helper 是否值得提取？ | **不提取** | 实际只有 `loadConversations`（isLoadingConversations）和 `loadMoreMessages`（isLoadingMoreMessages）2 处使用，且 `loadConversations` 有 `silent` 参数无法直接套用，提取收益不足以抵消复杂度 |
| 4 | `appendDelta`/`finishStream`/`errorStream` 外部消费者确认 | **零外部消费者**，可安全从 ChatState 移除 | grep 确认：仅在 chatStore.ts 的 `return {}` 中暴露为公开 action、chatOrchestrator.ts 通过 `deps.state.xxx()` 调用、chatOrchestrator.test.ts 中 mock。无任何 UI 组件直接调用 |

#### 3.1 批项 A：创建 `chatStreamStore.ts` — 流式状态独立 store（新增 ~80 行，chatStore -~120 行）

**问题**：当前 `isStreaming`/`streamingContent`/`streamingMessageId`/`isReasoningStreaming` 四个流式状态字段混在 `ChatState` 中，与持久化对话状态无逻辑关联。`appendDelta`/`finishStream`/`errorStream` 三个方法仅在 `statePort` 中被 orchestrator 调用，却被暴露为 store 公开 action——UI 消费者从不调用它们。

此外，chatStore 中有 6 处非 statePort 的直接 `set()` 重置流式状态（见 3.0.1 Bug 记录 + 以下补充），拆分后需统一改为调用 `useChatStreamStore.getState().cancelStreaming()`：

| 位置 | 方法 | 行号 | 场景 | 当前重置方式 |
|------|------|------|------|------------|
| L344-352 | `switchConversation` | 344-352 | 切换对话时取消前流 | `set({ isStreaming: false, streamingContent: "", streamingMessageId: null, isReasoningStreaming: false })` ✅ 完整 |
| L365-368 | `switchConversation` | 365-368 | 加载新对话详情后重置 | 同上 ✅ 完整 |
| L413-414 | `deleteConversation` | 413-414 | 删除当前对话后重置 | `set({ isStreaming: false, streamingContent: "" })` ❌ 遗漏 2 字段 |
| L480-488 | `editMessage` | 480-488 | 编辑消息时取消流 | `set({ isStreaming: false, streamingContent: "", streamingMessageId: null, isReasoningStreaming: false })` ✅ 完整 |
| L602-603 | `batchDelete` | 602-603 | 批量删除含当前对话时重置 | `set({ isStreaming: false, streamingContent: "" })` ❌ 遗漏 2 字段 |
| L617-618 | `purgeAll` | 617-618 | 清空全部时重置 | `set({ isStreaming: false, streamingContent: "" })` ❌ 遗漏 2 字段（**Bug**） |

**改造**：

```typescript
// src/stores/chatStreamStore.ts（新增）
import { create } from "zustand";
import type { Message, TokenUsage, ContextUsage } from "../chat/chatTypes";
import type { ChatError } from "../chat/chatErrors";
import { formatRetryAfter } from "../chat/chatErrors";

interface ChatStreamState {
  isStreaming: boolean;
  streamingContent: string;
  streamingMessageId: string | null;
  isReasoningStreaming: boolean;

  // Port 方法（仅 orchestrator 调用，不暴露给 UI）
  beginStream: (messages: Message[], assistantMsgId: string) => void;
  cancelStreaming: () => void;
  appendDelta: (delta: string) => void;
  setReasoningStreaming: (streaming: boolean) => void;
  appendReasoningDelta: (messageId: string, delta: string) => void;
  appendToolCallDelta: (...) => void;
  updateContextUsage: (usage: ContextUsage) => void;
  finishStream: (usage?: TokenUsage) => void;
  errorStream: (error: ChatError) => void;
}

export const useChatStreamStore = create<ChatStreamState>((set, get) => {
  // ... 从 chatStore statePort 移入的全部实现
});
```

**chatStore 改动**：
- `ChatState` 接口移除 `isStreaming`/`streamingContent`/`streamingMessageId`/`isReasoningStreaming`/`appendDelta`/`finishStream`/`errorStream` 七个成员
- `statePort` 改为委托 `useChatStreamStore.getState()`：
  ```typescript
  const statePort: ChatStatePort = {
    beginSend: (...args) => useChatStreamStore.getState().beginStream(...args),
    appendDelta: (delta) => useChatStreamStore.getState().appendDelta(delta),
    // ... 其余同理
  };
  ```
- `return {}` 中移除 `appendDelta: statePort.appendDelta`、`finishStream: statePort.finishStream`、`errorStream: statePort.errorStream` 三行
- UI 消费流式状态改为 `useChatStreamStore((s) => s.isStreaming)` 等

**UI 消费者改动清单**（grep 确认，完整覆盖）：

| 消费者 | 读取的流式状态 | 改动方式 |
|--------|---------------|---------|
| `ChatView.tsx` L322-325 | `chat.isStreaming`/`chat.isReasoningStreaming`/`chat.streamingMessageId`/`chat.streamingContent` | 改为从 `useChatStreamStore` 读取：`const stream = useChatStreamStore();` 然后传 `stream.isStreaming` 等 |
| `ChatView.tsx` L346 | `chat.isStreaming`（MessageInput） | 同上 |
| `useMascotChatSync.ts` L37 | `state.isStreaming`/`state.isReasoningStreaming`（subscribe 回调） | 改为两个独立 subscribe：`useChatStreamStore.subscribe` 监听 isStreaming/isReasoningStreaming，`useChatStore.subscribe` 监听 messages |
| `MessageList.tsx` L12-15 | props 传入（`isStreaming`/`isReasoningStreaming`/`streamingMessageId`/`streamingContent`） | **无需改动**（props 由 ChatView 传入） |
| `MessageInput.tsx` L20 | props 传入（`isStreaming`） | **无需改动** |
| `mascotStore.ts` L84-93 | 参数传入（`isStreaming`/`isReasoningStreaming`） | **无需改动** |
| `HistorySidebar.tsx` | 无流式状态消费 | **无需改动** |
| `HistoryManager.tsx` | 无流式状态消费 | **无需改动** |

#### 3.2 批项 B：`finishStream` 编排逻辑上移到 orchestrator（chatStore -~30 行，chatOrchestrator +~25 行）

**问题**：当前 `statePort.finishStream` 内含两个编排副作用：
1. `chatService.incrementGeneration(convId)` — 通知 Rust 端递增 generation
2. `chatService.getConversationTokenUsage(convId, ...)` — 异步刷新 token 用量

这两个调用属于"流结束后的业务编排"，不是"纯状态 set"。它们留在 Port 实现中违反了 Port 只做状态写入的设计契约。

**finishStream 跨 store 读写分析**（批项 A 拆分后）：

当前 `finishStream` 实现（chatStore.ts L168-233）需要：
- **读** `state.isStreaming`（判断是否在流中）→ 拆分后在 chatStreamStore
- **读** `state.streamingMessageId`（合并 streamingContent 到 messages）→ 拆分后在 chatStreamStore
- **读** `state.streamingContent`（合并到 messages）→ 拆分后在 chatStreamStore
- **读** `state.conversations`（找 model_id/provider_id 算 token）→ 拆分后在 chatStore
- **写** `messages`（合并 streamingContent 到对应 message）→ 拆分后在 chatStore
- **写** `isStreaming`/`streamingContent`/`streamingMessageId`/`isReasoningStreaming`/`contextUsage` → 拆分后前 4 个在 chatStreamStore，contextUsage 在 chatStore

**拆分方案**：finishStream 拆成两步：
1. **chatStreamStore.finishStream(usage)**：纯流式状态重置（`isStreaming: false` 等）+ 返回合并数据 `{ streamingMessageId, streamingContent }`
2. **onFinishMerge 回调**（chatStore 注册）：接收合并数据，执行 `messages.map(m => ...)` 写入 chatStore.messages

这样 chatStreamStore 不直接写 chatStore，chatStore 不直接读 chatStreamStore，通过回调解耦。

**改造**：

1. `ChatStatePort.finishStream` 签名不变，但实现只做纯状态 set（移除 incrementGeneration 和 getConversationTokenUsage 调用）
2. orchestrator 的 `handleDeltaEvent` 中 `finish_reason` 分支增加编排逻辑：
   ```typescript
   if (payload.finish_reason) {
     setPhase("idle");
     deps.state.finishStream(payload.usage);  // 纯状态 set
     // 编排副作用
     const convId = deps.env.getActiveConversationId();
     if (convId) {
       chatService.incrementGeneration(convId).catch(() => {});
       // 异步刷新 token 用量
       const conv = deps.env.getConversations().find(c => c.id === convId);
       chatService.getConversationTokenUsage(convId, deps.env.resolveContextWindowTokens?.(conv?.model_id ?? "", conv?.provider_id ?? null))
         .then(summary => deps.state.updateContextUsage({ used_tokens: summary.total_tokens, max_tokens: summary.context_window_tokens, percentage: summary.percentage }))
         .catch(() => {});
     }
   }
   ```
3. `ChatEnvPort` 新增 `resolveContextWindowTokens` 方法（从 chatStore 的模块级函数移入 envPort）

**参考**：airi 的 `chat-orchestrator-runtime.ts` 在流结束后通过 `deps.session.appendSessionMessage()` 和 `deps.onLlmGeneration()` 回调执行编排副作用，Port 方法 `foregroundStream.patch()` 只做纯状态写入。

#### 3.3 批项 C：公开 API 最小化 + envPort 内联（chatStore -~50 行）

**问题**：
1. `ChatState` 接口仍包含 `appendDelta`/`finishStream`/`errorStream`，但 UI 消费者从不调用——公开 API 面积过大
2. `envPort` 的每个方法单独命名再引用，增加了不必要的间接层

**改造**：

1. `ChatState` 接口移除 `appendDelta`/`finishStream`/`errorStream`（批项 A 已移到 chatStreamStore）
2. `sendMessage`/`cancelStream` 保留在 `ChatState` 中，但实现仅为委托：
   ```typescript
   sendMessage: (content: string) => orchestrator.send(content),
   cancelStream: () => orchestrator.cancel(),
   ```
3. envPort 内联为闭包（不再单独声明 `envPort` 变量）：
   ```typescript
   const orchestrator = createChatOrchestrator({
     state: statePort,
     env: {
       getActiveConversationId: () => get().currentConversationId,
       getIsStreaming: () => useChatStreamStore.getState().isStreaming,
       getStreamingMessageId: () => useChatStreamStore.getState().streamingMessageId,
       getMessages: () => get().messages,
       getConversations: () => get().conversations,
       getSelectedModeId: () => useAgentModeStore.getState().selectedModeId,
       resolveContextWindowTokens,
     },
     // ...
   });
   ```

#### 3.4 批项 D：副作用 action 精简（chatStore -~20 行）

**问题**：`loadConversations`/`loadMoreMessages` 中有重复的 try/catch + `set({ isLoadingXxx: true/false })` 样板代码。

**决策**：**不提取 `withLoading` helper**。原因：
1. 实际只有 2 处使用（`loadConversations` 用 `isLoadingConversations`，`loadMoreMessages` 用 `isLoadingMoreMessages`）
2. `loadConversations` 有 `silent` 参数，无法直接套用 `withLoading`
3. 提取后类型签名需要 `keyof ChatState` 的 `as Partial<ChatState>` 断言，增加类型不安全
4. 收益不足以抵消复杂度

**替代方案**：仅做最小改动——在拆分流式状态后，这些方法中 `set({ isStreaming: false, streamingContent: "" })` 等流式重置已移到 chatStreamStore，自然减少行数。无需额外重构。
```

#### 3.5 改造前后对比

| 维度 | 改造前 | 改造后 |
|------|--------|--------|
| chatStore 行数 | 635 | ~400 |
| chatStreamStore 行数 | — | ~80 |
| ChatState 公开 action 数 | 17（含 appendDelta/finishStream/errorStream） | 14（仅 UI 真正需要的） |
| finishStream 职责 | 纯 set + incrementGeneration + getConversationTokenUsage | 纯 set（编排逻辑在 orchestrator） |
| 流式状态归属 | 混在 ChatState 中 | 独立 chatStreamStore |
| envPort 声明方式 | 单独变量 + 逐方法命名 | 内联闭包 |
| 流式状态重置遗漏 bug | 3 处（deleteConversation/batchDelete/purgeAll） | 0（统一走 chatStreamStore.cancelStreaming） |
| withLoading helper | — | 不提取（2 处使用，收益不足） |
| 参考模式 | 无 | airi stream-store 拆分 |

#### 3.6 保留在 chatStore 中的

- state shape 定义（`ChatState` interface，不含流式字段）
- 持久化状态：`conversations`/`messages`/`currentConversationId`/`contextUsage`/`hasMoreMessages`/`isLoadingMoreMessages`/`isLoadingConversations`/`totalMessageCount`
- 副作用 action：`loadConversations`/`switchConversation`/`deleteConversation`/`renameConversation`/`updateModel`/`createConversation`/`ensureCurrentConversation`/`deleteMessage`/`editMessage`/`regenerateMessage`/`loadMoreMessages`/`togglePin`/`duplicateConversation`/`batchDelete`/`purgeAll`
- 委托 action：`sendMessage`（→ orchestrator.send）/`cancelStream`（→ orchestrator.cancel）
- orchestrator 实例化 + Port 连接
- `resolveContextWindowTokens`（读 providerStore，envPort 引用）
- `getChatOrchestrator()` 导出

#### 3.7 移到 chatStreamStore 的

- `isStreaming`/`streamingContent`/`streamingMessageId`/`isReasoningStreaming` 四个状态字段
- `statePort` 全部实现：`beginSend`/`cancelStreaming`/`appendDelta`/`setReasoningStreaming`/`appendReasoningDelta`/`appendToolCallDelta`/`updateContextUsage`/`finishStream`/`errorStream`

#### 3.8 移到 orchestrator 的

- `finishStream` 中的 `chatService.incrementGeneration()` 调用
- `finishStream` 中的 `chatService.getConversationTokenUsage()` 调用
- `resolveContextWindowTokens` 引用（通过 envPort 暴露给 orchestrator）

## 5. 行为等价性验证策略

### 核心原理

替换 `invoke("xxx", args)` 为 `chatService.xxx(args)` 行为一致的条件：
1. **命令名一致**：chatService 调的 invoke 命令名与原来完全相同
2. **参数一致**：传给 invoke 的参数对象与原来完全相同
3. **返回值变换一致**：`as` 类型断言 + `|| []` fallback 逻辑与原来完全相同

只要 chatService 满足以上 3 条，store 中替换后行为**数学上保证**一致。

### 契约测试

对 `chatService.ts` 写契约测试（`src/services/__tests__/chatService.test.ts`），mock `invoke`，验证每个方法：

- 调了正确的命令名（`expect(invoke).toHaveBeenCalledWith("list_conversations", ...)`）
- 传了正确的参数（参数对象逐字段对比）
- 返回值变换与原来一致（null fallback、类型断言等）

**已实现**：28 个测试全部通过，覆盖全部 18 个 chatService 方法。

### 替换后验证

Step 1 完成后（chatStore 中所有 invoke 替换为 chatService 调用）：
- TypeScript 编译通过 → 参数类型匹配
- 契约测试通过 → 命令名、参数、返回值变换一致
- 因此行为等价

## 6. 风险与回退

| 风险 | 缓解 | 状态 |
|------|------|------|
| Step 1 替换 invoke 时遗漏或参数不匹配 | 每个 invoke 逐个替换后立即 TypeScript 编译检查 | ✅ 已验证 |
| Step 2 orchestrator 回调时序与原 store.setState 不一致 | 先写单元测试覆盖 sendMessage 全流程，重构后跑测试 | ✅ 21 契约测试通过 |
| Step 2 chatDeltaEvents 改调 orchestrator 后流式行为变化 | 对比重构前后 delta 日志（DEV 模式已有 console.debug） | ✅ 事件路径统一验证 |
| Step 2 TDZ 初始化错误 | `_orchestrator` 重复声明导致暂时性死区 | ✅ 已修复（删除重复声明） |
| Step 3 chatStreamStore 拆分后 UI 消费者遗漏更新 | 逐个 grep `isStreaming`/`streamingContent`/`streamingMessageId`/`isReasoningStreaming` 消费点，全部改为 useChatStreamStore | ⬜ Step 3 实施时验证 |
| Step 3 finishStream 编排逻辑上移后时序变化 | incrementGeneration/getConversationTokenUsage 改为 orchestrator 在 finish_reason 分支调用，时序与原 finishStream 尾部调用一致 | ⬜ Step 3 实施时验证 |
| Step 3 ChatState 接口移除 appendDelta/finishStream/errorStream 后编译报错 | 这三个方法无外部消费者（仅 orchestrator 通过 Port 调用），移除后零编译错误 | ⬜ Step 3 实施时验证 |
| Step 3 流式状态重置遗漏 bug（purgeAll 中 isReasoningStreaming 残留） | 批项 A 统一改为 chatStreamStore.cancelStreaming()，完整重置 4 字段 | ⬜ Step 3 批项 A 修复 |
| Step 3 finishStream 跨 store 读写（读 chatStreamStore + 写 chatStore.messages） | 拆成两步：chatStreamStore 纯重置 + onFinishMerge 回调写 messages | ⬜ Step 3 批项 A 实施时验证 |
| Step 3 useMascotChatSync 需同时监听两个 store | 两个独立 subscribe：useChatStreamStore.subscribe + useChatStore.subscribe | ⬜ Step 3 批项 A 实施时验证 |

## 7. 验收标准

- [x] `chatService.ts` 契约测试 28/28 通过
- [x] `chatStore.ts` 中零 `invoke` 调用
- [x] `chatStore.ts` 中零 `as` 类型断言（IPC 返回值相关）— 仅剩 1 处 `JSON.parse as ModelConfig[]`（非 IPC 返回值，可接受）
- [x] `chatDeltaEvents.ts` 中零 `store.setState()` 直接调用 — Step 2 已彻底消除，零 store 依赖
- [x] `chatService.ts` 覆盖全部 26 个原 invoke 调用点（18 个方法）
- [x] `chatOrchestrator.ts` 包含 ChatPhase 状态机
- [x] `chatOrchestrator.ts` 契约测试 21/21 通过
- [x] orchestrator 零 store 依赖（只通过 Port 接口）
- [x] 功能不变：发送消息、流式输出、取消、错误处理、对话切换、消息编辑/删除/重新生成
- [x] TypeScript 编译通过
- [ ] `chatStreamStore.ts` 存在，持有 `isStreaming`/`streamingContent`/`streamingMessageId`/`isReasoningStreaming` + 全部 Port 方法实现
- [ ] `chatStore.ts` 中 `ChatState` 接口不含 `appendDelta`/`finishStream`/`errorStream`/`isStreaming`/`streamingContent`/`streamingMessageId`/`isReasoningStreaming`
- [ ] `chatStore.ts` 中 `statePort.finishStream` 不含 `chatService.incrementGeneration` / `chatService.getConversationTokenUsage` 调用
- [ ] `chatOrchestrator.ts` 中 `handleDeltaEvent` 的 `finish_reason` 分支包含 incrementGeneration + getConversationTokenUsage 编排逻辑
- [ ] `ChatEnvPort` 接口包含 `resolveContextWindowTokens` 方法
- [ ] UI 消费者（ChatView/useMascotChatSync）流式状态从 `useChatStreamStore` 读取
- [ ] chatStore 行数 ≤ 450
- [ ] 流式状态重置遗漏 bug 已修复：`deleteConversation`/`batchDelete`/`purgeAll` 中 `isReasoningStreaming` + `streamingMessageId` 完整重置
- [ ] `finishStream` 跨 store 读写通过 `onFinishMerge` 回调解耦
- [ ] `useMascotChatSync` 使用两个独立 subscribe 监听 chatStore + chatStreamStore
- [ ] `withLoading` helper 不提取（决策记录在 3.0.3）

## 8. Step 2 实施记录

### 8.1 依赖关系优化成果

改造后的依赖关系（单向，无循环）：

```
                        ┌─────────────────────────────────────────┐
                        │           OrbPanel.tsx (UI)             │
                        └──────┬────────────────────┬────────────┘
                               │                    │
                    useChatStore()         getChatOrchestrator()
                    useChatStreamStore()
                               │                    │
                               ▼                    ▼
┌──────────────────────────────────────────────────────────────────┐
│                        chatStore.ts                              │
│  实现 ChatEnvPort                                                │
│  statePort 委托 chatStreamStore                                  │
│  创建 orchestrator = createChatOrchestrator({state, env})        │
│  _orchestrator = orchestrator                                    │
│  导出: useChatStore, getChatOrchestrator()                       │
└────────┬─────────────────────────────────────────────┬───────────┘
         │ import                                       │ 返回引用
         ▼                                              ▼
┌─────────────────────────┐     ┌──────────────────────────────────┐
│    chatService.ts        │     │   chatDeltaEvents.ts             │
│  (纯 Tauri invoke 封装)  │     │   零 store 依赖 ✅               │
└─────────────────────────┘     │   零 store.setState ✅            │
                                 │   通过 setChatOrchestrator()     │
                                 │   注入 orchestrator 实例         │
                                 └────────┬─────────────────────────┘
                                          │ 调用
                                          ▼
                           ┌──────────────────────────────────┐
                           │   chatOrchestrator.ts             │
                           │   零 store 依赖 ✅                │
                           │   仅通过 Port 接口写状态 ✅        │
                           │   finish 后编排副作用 ✅           │
                           └──────────┬───────────────────────┘
                                      │ import
                                      ▼
                           ┌──────────────────────────┐
                           │    chatService.ts         │
                           │    (Tauri invoke 封装)    │
                           └──────────────────────────┘

┌──────────────────────────────────┐
│   chatStreamStore.ts (Step 3)    │
│   流式状态 + ChatStatePort 实现  │
│   被 chatStore statePort 委托   │
│   被 UI 直接消费流式状态        │
└──────────────────────────────────┘
```

### 8.2 事件流路径

```
Tauri event → chatDeltaEvents 监听
                │
                ├─ delta_text → coalescer (渲染优化) → orchestrator.onTextDelta(delta)
                │                                           │
                │                                           ├─ statePort.appendDelta(delta)
                │                                           └─ phase: sending → streaming
                │
                └─ 其余事件 → orchestrator.handleDeltaEvent(payload)
                                    │
                                    ├─ delta_reasoning → statePort.appendReasoningDelta()
                                    ├─ tool_call      → statePort.appendToolCallDelta()
                                    ├─ usage          → statePort.updateContextUsage()
                                    ├─ finish_reason  → statePort.finishStream() (纯 set)
                                    │                  → chatService.incrementGeneration() (编排)
                                    │                  → chatService.getConversationTokenUsage() (编排)
                                    └─ error          → statePort.errorStream()
```

### 8.3 与改造前对比

| 维度 | 改造前 | 改造后 |
|------|--------|--------|
| chatDeltaEvents → chatStore | **直接 import useChatStore** + **直接 store.setState** | **零依赖**，orchestrator 运行时注入 |
| chatStore 编排逻辑 | sendMessage 内含全部编排 + 事件处理 | 委托 orchestrator.send()，store 仅实现 Port |
| 循环依赖风险 | chatDeltaEvents ↔ chatStore 双向 | **单向**：chatStore → chatOrchestrator，chatDeltaEvents ← chatOrchestrator(type) |
| 可测试性 | 需 mock 整个 store | mock Port 接口即可，21 个契约测试覆盖 |
| 扩展性 | 新增事件需改 store | 新增事件只需改 orchestrator.handleDeltaEvent |
| 流式状态归属 (Step 3) | 混在 ChatState 中 | 独立 chatStreamStore，与持久化状态分离 |
| finishStream 职责 (Step 3) | 纯 set + incrementGeneration + getConversationTokenUsage | 纯 set（编排在 orchestrator finish_reason 分支） |
| 公开 API 面积 (Step 3) | 17 action（含 3 个仅 Port 使用的） | 14 action（仅 UI 真正需要的） |
| 流式状态重置遗漏 bug (Step 3) | 3 处（deleteConversation/batchDelete/purgeAll） | 0（统一走 chatStreamStore.cancelStreaming） |
| withLoading helper (Step 3) | — | 不提取（2 处使用，收益不足） |

### 8.4 修复记录

| 问题 | 原因 | 修复 |
|------|------|------|
| TDZ: `Cannot access '_orchestrator' before initialization` | chatStore.ts 中存在两个 `let _orchestrator` 声明（第 95 行 + 第 633 行），第二个声明使变量进入暂时性死区，`create()` 回调同步执行赋值时触发 | 删除第 633 行重复声明，`getChatOrchestrator()` 引用第 95 行变量 |
| 未使用导入 | TypeScript 编译报错 3 处 | 逐一删除 |
| onTextDelta 无 delta 参数 | coalescer 回调只做 phase 转换，delta 内容未追加到 streamingContent | onTextDelta 改为接收 delta 参数，内部调 `deps.state.appendDelta(delta)` |
| handleDeltaEvent 重复处理 delta_text | delta_text 同时走 coalescer 和 handleDeltaEvent 导致重复 appendDelta | 从 handleDeltaEvent 移除 delta_text 分支，统一由 coalescer → onTextDelta 处理 |

## 9. 远期扩展预留

### 9.1 Tool Call 循环

当前 ChatPhase 状态机已预留 `tool_executing`，将来落地时：

- chatService 新增 `executeToolCall(conversationId, toolCallId, result)` 方法
- orchestrator 的 `handleDeltaEvent` 中 `finish_reason === "tool_calls"` 分支 → 调 `chatService.executeToolCall` → phase 转回 `sending`
- 零 store 改动，零 chatStreamStore 改动，仅 orchestrator + chatService 扩展
- Step 3 拆分后优势更明显：流式状态在 chatStreamStore 中独立管理，tool_executing phase 只需在 chatStreamStore 加 `toolCallStates` 字段，不影响 chatStore 的持久化状态

### 9.2 事件路由表化

当前 `handleDeltaEvent` 用 if/else 分支路由事件类型。当事件类型增多（tool_result、tool_progress 等），建议改为路由表：

```typescript
const deltaHandlers: Record<string, (payload: ChatDeltaPayload) => void> = {
  delta_reasoning: (p) => state.appendReasoningDelta(...),
  tool_call:       (p) => state.appendToolCallDelta(...),
  usage:           (p) => state.updateContextUsage(...),
  // 将来新增只需加一行
};
```
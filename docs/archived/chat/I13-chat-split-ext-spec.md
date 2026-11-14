# I13-Ext — Chat Store 职责细粒度拆分 规格说明

> 基于 I13 已完成的 Step 1（chatService）+ Step 2（chatOrchestrator），对 Step 3 重新设计
> 前置：I13 Step 1 ✅ + Step 2 ✅
> 参考：[I13-chat-service-split-spec.md](./I13-chat-service-split-spec.md)

## 1. 交付目标

将 `chatStore.ts`（825 行）中混杂的 **5 个职责域** 按实际代码边界拆分，使每一层可独立演进、独立测试，消除跨 store 读写耦合。

## 2. 当前代码职责审计

### 2.1 五个职责域

| # | 职责域 | 状态字段 | action/Port | 行数占比 |
|---|--------|---------|-------------|---------|
| 1 | **持久层** | `conversations` / `currentConversationId` / `messages` / `hasMoreMessages` / `isLoadingMoreMessages` / `isLoadingConversations` / `totalMessageCount` / `contextUsage` | 16 个 CRUD action | ~55% |
| 2 | **流式标量层** | `isStreaming` / `streamingContent` / `streamingMessageId` / `activeAttemptId` / `isReasoningStreaming` / `retryStatus` | 6 个纯标量 Port | ~15% |
| 3 | **流式 messages 写入层** | （无独立字段，操作持久层 messages） | 6 个写 messages 的 Port：`appendReasoningDelta` / `appendToolCallDelta` / `finishStream`（合并） / `errorStream`（合并） / `cancelStreaming`（合并） / `setActiveAttemptId`（ID 替换） | ~12% |
| 4 | **Compact 层** | `isFull` / `compactSummary` / `compactError` | `manualCompact` / `unlockConversation` / `clearCompactError` | ~8% |
| 5 | **Branch 层** | `activeBranchId` / `branchOperationSeq` / `pendingBranchPoint` | `switchBranch` + `resendFrom` 中 pendingBranchPoint 逻辑 | ~5% |

### 2.2 跨职责域依赖（当前 chatStore 内部）

| 依赖 | 方向 | 说明 |
|------|------|------|
| 持久层 action → 流式标量层 | 写 | `switchConversation`/`deleteConversation`/`batchDelete`/`purgeAll` 重置流式标量 |
| 流式 messages 写入层 → 持久层 messages | 写 | `appendReasoningDelta`/`appendToolCallDelta`/`finishStream`/`errorStream`/`cancelStreaming` 写 `messages` |
| 流式 messages 写入层 → 流式标量层 | 读+写 | `finishStream`/`errorStream`/`cancelStreaming` 读 `streamingMessageId`/`streamingContent`，写 `isStreaming` 等 |
| Compact 层 → 流式标量层 | 写 | `manualCompact` 设 `isStreaming: true` / `streamingMessageId` |
| Branch 层 → 持久层 | 读 | `switchBranch` 读 `currentConversationId` |
| Branch 层 → 流式标量层 | 读 | `resendFrom` 通过 orchestrator 间接读 `pendingBranchPoint` |

### 2.3 chatDeltaEvents 职责审计

当前 237 行，6 个事件监听分属 2 个职责域：

| 事件 | 职责域 | 写 store 方式 | 行数 |
|------|--------|-------------|------|
| `chat-stream-start` | 流式 | `store.setActiveAttemptId()` | ~10 |
| `chat-delta` | 流式 | 走 orchestrator ✅ | ~60 |
| `conversation_terminated` | 生命周期 | `useChatStore.setState({ isFull, compactSummary })` | ~10 |
| `conversation_compact_failed` | 生命周期 | `useChatStore.setState({ compactError })` | ~5 |
| `conversation_unlocked` | 生命周期 | `useChatStore.setState({ isFull, ... })` | ~10 |
| `chat-messages-changed` | 生命周期 | `useChatStore.setState({ messages, activeBranchId })` | ~15 |

### 2.4 已发现 Bug

| Bug | 位置 | 触发条件 | 影响 |
|-----|------|---------|------|
| `manualCompact` 错误处理遗漏 `isReasoningStreaming: false` | chatStore.ts L772 | compact 失败时 `isReasoningStreaming` 残留 `true` | mascot 表情卡死在"思考中" |

## 3. 架构设计

### 3.1 拆分决策

| 拆分项 | 决策 | 理由 |
|--------|------|------|
| chatStreamStore（流式标量） | **拆** | 6 个字段生命周期独立（send→finish 瞬态），与持久化状态无逻辑关联；拆分后重置逻辑统一，消除 10 处散落的 `set({ isStreaming: false, ... })` |
| 流式 messages 写入 Port | **不拆出 chatStore** | 6 个 Port 全部写 `messages`（持久层），拆到 chatStreamStore 必然跨 store 读写 messages，引入 6 处回调/间接访问；留在 chatStore 作为 statePort 实现，零跨 store 读写 |
| Compact 层 | **不拆** | 3 个字段 + 3 个 action 体量小，且 `manualCompact` 需写流式标量状态，拆分后跨 store 依赖不减反增 |
| Branch 层 | **不拆** | 3 个字段 + 1 个 action 体量极小，`pendingBranchPoint` 横跨 resendFrom→orchestrator，拆分后 envPort 需跨 store 读取 |
| chatDeltaEvents → streamEvents + lifecycleEvents | **拆** | 流式事件零 store 依赖（走 orchestrator），生命周期事件写持久化状态（直接写 chatStore），职责清晰分离 |

### 3.2 拆分后架构

```
┌───────────────────────────────────────────────────────────────┐
│  chatStreamStore.ts  (~60 行) — 纯流式标量状态                 │
│  字段：isStreaming / streamingContent / streamingMessageId     │
│        activeAttemptId / isReasoningStreaming / retryStatus    │
│  方法：beginStream / cancelStreaming / appendDelta             │
│        setReasoningStreaming / setRetryStatus / resetStreaming │
│  特征：零跨 store 读写，纯标量 set/get                         │
│  消费者：ChatView / MessageBubble / useMascotChatSync          │
├───────────────────────────────────────────────────────────────┤
│  chatStorePorts.ts  (~285 行) — Port 工厂 + 辅助函数          │
│  工厂：createStatePort(set, get, deps) / createEnvPort(...)   │
│        / createOrchestrator(...)                              │
│  辅助：resolveContextWindowTokens / fetchTokenUsage            │
│        / mergeStreamingContent                                │
│  特征：零 chatStore import（依赖通过参数注入，无循环依赖）     │
│  FetchTokenUsageDeps：isActiveConv / setContextUsage           │
├───────────────────────────────────────────────────────────────┤
│  chatStore.ts  (~540 行) — 持久化状态 + CRUD action            │
│  持久字段：conversations / currentConversationId / messages     │
│            contextUsage / hasMoreMessages / isLoading...       │
│            totalMessageCount                                   │
│  Compact 字段：isFull / compactSummary / compactError          │
│  Branch 字段：activeBranchId / branchOperationSeq              │
│               pendingBranchPoint                               │
│  CRUD action：16 个                                            │
│  Compact action：manualCompact / unlockConversation            │
│                  / clearCompactError                            │
│  Branch action：switchBranch                                   │
│  编排委托：sendMessage / cancelStream / resendFrom              │
│  Port 组装：createStatePort / createEnvPort / createOrchestrator│
│  特征：statePort/envPort 实现委托 chatStorePorts 工厂函数       │
│        流式标量委托 chatStreamStore（零跨 store 读写 messages） │
├───────────────────────────────────────────────────────────────┤
│  chatStreamEvents.ts  (~80 行) — 流式事件订阅                  │
│  监听：chat-stream-start + chat-delta                           │
│  特征：零 store 依赖，全部走 orchestrator                       │
│  新增：orchestrator.onStreamStart(attemptId, messageId)        │
├───────────────────────────────────────────────────────────────┤
│  chatLifecycleEvents.ts  (~50 行) — 生命周期事件订阅           │
│  监听：conversation_terminated / compact_failed / unlocked     │
│        / chat-messages-changed                                 │
│  特征：直接写 chatStore（持久化状态），合理依赖                 │
├───────────────────────────────────────────────────────────────┤
│  chatOrchestrator.ts  (~420 行) — 不变                         │
│  chatService.ts  (~142 行) — 不变                              │
└───────────────────────────────────────────────────────────────┘
```

### 3.3 层间依赖（无循环）

```
chatStreamEvents.ts ──→ chatOrchestrator.ts ──→ chatService.ts
                         ↑          ↑
chatStorePorts.ts ───────┘          |
  (createStatePort / createEnvPort / createOrchestrator)         |
  (→ chatStreamStore, chatOrchestrator, chatService)             |
  (零 chatStore import ✅)                                       |
chatStore.ts ──→ chatStorePorts.ts                               |
  (Port 工厂调用 + FetchTokenUsageDeps 注入)                     |
  (→ chatStreamStore)                                            |
chatLifecycleEvents.ts ──→ chatStore.ts
  (写持久化状态)
useAgentModeStore ─────────────────┘
  (envPort 读 selectedModeId)
```

### 3.4 chatStreamStore 接口设计

```typescript
// src/stores/chatStreamStore.ts
import { create } from "zustand";

interface ChatStreamState {
  isStreaming: boolean;
  streamingContent: string;
  streamingMessageId: string | null;
  activeAttemptId: string | null;
  isReasoningStreaming: boolean;
  retryStatus: RetryNotification | null;

  beginStream: (assistantMsgId: string) => void;
  cancelStreaming: () => void;
  appendDelta: (delta: string) => void;
  setReasoningStreaming: (streaming: boolean) => void;
  setActiveAttemptId: (attemptId: string, realMessageId: string) => void;
  clearActiveAttemptId: () => void;
  setRetryStatus: (retry: RetryNotification | null) => void;
  resetStreaming: () => void;
}

export const useChatStreamStore = create<ChatStreamState>()((set, get) => ({
  isStreaming: false,
  streamingContent: "",
  streamingMessageId: null,
  activeAttemptId: null,
  isReasoningStreaming: false,
  retryStatus: null,

  beginStream: (assistantMsgId: string) => {
    set({
      isStreaming: true,
      streamingContent: "",
      streamingMessageId: assistantMsgId,
      activeAttemptId: null,
      isReasoningStreaming: false,
      retryStatus: null,
    });
  },

  cancelStreaming: () => {
    set({
      isStreaming: false,
      streamingContent: "",
      streamingMessageId: null,
      activeAttemptId: null,
      isReasoningStreaming: false,
      retryStatus: null,
    });
  },

  appendDelta: (delta: string) => {
    set({ streamingContent: get().streamingContent + delta });
  },

  setReasoningStreaming: (streaming: boolean) => {
    set({ isReasoningStreaming: streaming });
  },

  setActiveAttemptId: (attemptId: string, realMessageId: string) => {
    set({ activeAttemptId: attemptId, streamingMessageId: realMessageId });
  },

  clearActiveAttemptId: () => {
    set({ activeAttemptId: null });
  },

  setRetryStatus: (retry: RetryNotification | null) => {
    set({ retryStatus: retry });
  },

  resetStreaming: () => {
    // 与 cancelStreaming 标量重置逻辑相同，保留独立方法名以区分语义
    set({
      isStreaming: false,
      streamingContent: "",
      streamingMessageId: null,
      activeAttemptId: null,
      isReasoningStreaming: false,
      retryStatus: null,
    });
  },
}));
```

**关键设计决策**：

| # | 决策点 | 结论 | 理由 |
|---|--------|------|------|
| 1 | `cancelStreaming` 是否合并 streamingContent 到 messages？ | **否，chatStreamStore 只做标量重置** | 合并 messages 是持久层操作，留在 chatStore 的 statePort.cancelStreaming 中 |
| 2 | `setActiveAttemptId` 是否替换 messages 中的 temp ID？ | **否，只更新标量** | ID 替换是 messages 操作，留在 chatStore 的 statePort.setActiveAttemptId 中 |
| 3 | `beginStream` 是否设置 messages？ | **否，只设标量** | messages 的 optimistic update 留在 chatStore 的 statePort.beginSend 中 |
| 4 | `resetStreaming` vs `cancelStreaming` | **保留两个方法名，共享内部实现** | 语义不同：`cancelStreaming` 用于用户取消/流结束（chatStore statePort 需合并 streamingContent→messages），`resetStreaming` 用于对话切换/删除等场景（无需合并 messages）。chatStreamStore 层面两者标量重置逻辑相同，但保留双名避免调用方语义混淆 |

### 3.5 chatStore statePort 改造

statePort/envPort 实现从 chatStore 内联提取到 `chatStorePorts.ts` 工厂函数，chatStore 仅做组装调用：

```typescript
// chatStore.ts
const fetchTokenUsageDeps: FetchTokenUsageDeps = {
  isActiveConv: (convId: string) => get().currentConversationId === convId,
  setContextUsage: (usage: ContextUsage) => set({ contextUsage: usage }),
};
const statePort = createStatePort(set, get, fetchTokenUsageDeps);
const envPort = createEnvPort(get, set);
const orchestrator = createOrchestrator(statePort, envPort, { ... });
```

`chatStorePorts.ts` 中 `createStatePort` 的实现逻辑：**标量部分委托 chatStreamStore + messages 操作通过 set/get 参数**：

```typescript
// chatStorePorts.ts
export function createStatePort(
  set: (partial: Partial<StatePortWritable>) => void,
  get: () => StatePortReadable,
  fetchTokenUsageDeps: FetchTokenUsageDeps,
): ChatStatePort {
  return {
    beginSend: (messages, assistantMsgId) => {
      useChatStreamStore.getState().beginStream(assistantMsgId);
      set({ messages, contextUsage: { usedTokens: 0, maxTokens: 128_000, percentage: 0 } });
    },
    cancelStreaming: () => {
      const stream = useChatStreamStore.getState();
      useChatStreamStore.getState().cancelStreaming();
      set({ messages: mergeStreamingContent(get().messages, stream.streamingMessageId, stream.streamingContent) });
    },
    // ... 其余 Port 方法同理
  };
}
```

**关键设计决策**：

| # | 决策点 | 结论 | 理由 |
|---|--------|------|------|
| 1 | `fetchTokenUsage` 是否直接 `import useChatStore` | **否，通过 FetchTokenUsageDeps 参数注入** | 消除 chatStorePorts ↔ chatStore 循环依赖 |
| 2 | `set`/`get` 类型 | **最小接口（StatePortWritable/StatePortReadable）** | 比 `Partial<ChatState>` 更精确，且不引入 ChatState 类型依赖 |
| 3 | Port 实现放在哪 | **chatStorePorts.ts 工厂函数** | chatStore 主文件只做组装，Port 实现细节独立可测 |

### 3.6 chatStore 流式重置统一化

当前 10 处散落的流式标量重置，拆分后统一改为 `useChatStreamStore.getState().cancelStreaming()`：

| 位置 | 方法 | 当前代码 | 改为 |
|------|------|---------|------|
| L417 | `switchConversation`（cancel 前流） | `set({ isStreaming: false, streamingContent: "", ... })` | `useChatStreamStore.getState().cancelStreaming()` |
| L435 | `switchConversation`（加载后重置） | 同上 | 同上 |
| L488 | `deleteConversation` | 同上 | 同上 |
| L717 | `batchDelete` | 同上 | 同上 |
| L735 | `purgeAll` | 同上 | 同上 |
| L772 | `manualCompact` 错误处理 | `set({ isStreaming: false, ..., activeAttemptId: null })` 遗漏 `isReasoningStreaming` | 同上（**同时修复 Bug**） |

### 3.7 chatOrchestrator Port 接口变更

#### ChatStatePort 变更

```typescript
export interface ChatStatePort {
  beginSend: (messages: Message[], assistantMsgId: string) => void;
  cancelStreaming: () => void;
  setActiveAttemptId: (attemptId: string, realMessageId: string) => void;
  clearActiveAttemptId: () => void;
  appendDelta: (delta: string) => void;
  setReasoningStreaming: (streaming: boolean) => void;
  appendReasoningDelta: (messageId: string, delta: string) => void;
  appendToolCallDelta: (...) => void;
  updateContextUsage: (usage: ContextUsage) => void;
  finishStream: (usage?: TokenUsage) => void;
  errorStream: (error: ChatError) => void;
  setRetryStatus: (retry: RetryNotification | null) => void;
}
```

**不变**——Port 接口不变，只有实现从"直接 set chatStore"改为"标量委托 chatStreamStore + messages 留 chatStore"。

#### ChatEnvPort 变更

```typescript
export interface ChatEnvPort {
  getActiveConversationId: () => string | null;
  getIsStreaming: () => boolean;           // 改为读 chatStreamStore
  getStreamingMessageId: () => string | null;  // 改为读 chatStreamStore
  getActiveAttemptId: () => string | null;     // 改为读 chatStreamStore
  getMessages: () => Message[];
  getConversations: () => Conversation[];
  getSelectedModeId: () => string | null;
  getPendingBranchPoint: () => string | null;
  clearPendingBranchPoint: () => void;
}
```

**变更**：`getIsStreaming` / `getStreamingMessageId` / `getActiveAttemptId` 实现从 `get().xxx` 改为 `useChatStreamStore.getState().xxx`。

#### ChatOrchestrator 接口变更

```typescript
export interface ChatOrchestrator {
  send: (content: string) => Promise<void>;
  cancel: () => Promise<void>;
  handleDeltaEvent: (payload: ChatDeltaPayload) => void;
  onTextDelta: (delta: string) => void;
  onStreamStart: (attemptId: string, messageId: string) => void;  // 新增
  getActiveConversationId: () => string | null;                   // 新增
  ensureIdle: () => Promise<void>;
  getSendInProgress: () => boolean;
  getPhase: () => ChatPhase;
}
```

**新增 `onStreamStart`**：将 `chat-stream-start` 事件的处理从 chatDeltaEvents 直接调 `store.setActiveAttemptId` 改为走 orchestrator Port。

**新增 `getActiveConversationId`**：供 `chatStreamEvents.ts` 判断 `chat-stream-start` 事件是否属于当前对话（避免非当前对话的 stream-start 干扰当前流式状态）。

### 3.8 chatDeltaEvents 拆分

#### chatStreamEvents.ts

```typescript
// src/services/chatStreamEvents.ts
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ChatOrchestrator } from "./chatOrchestrator";
import type { ChatDeltaPayload, ChatStreamStartPayload } from "../chat/core/chatTypes";
import { createChatDeltaCoalescer, createSeqDedupFilter, textChunkScheduler, type CoalescibleDelta } from "../chat/core/streamCoalescer";
import { createLogger } from "./logger";

const log = createLogger("chatStreamEvents");

let orchestratorInstance: ChatOrchestrator | null = null;

export function setChatOrchestrator(orchestrator: ChatOrchestrator): void {
  orchestratorInstance = orchestrator;
}

export function subscribeChatStreamEvents(): () => void {
  // ... chat-stream-start + chat-delta 监听
  // chat-stream-start → orchestrator.onStreamStart(attemptId, messageId)
  // chat-delta → coalescer → orchestrator.onTextDelta / orchestrator.handleDeltaEvent
  // 零 store 依赖 ✅
}
```

#### chatLifecycleEvents.ts

```typescript
// src/services/chatLifecycleEvents.ts
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useChatStore } from "../stores/chatStore";
import type { ConversationTerminatedPayload, ConversationCompactFailedPayload, ConversationUnlockedPayload, ChatMessagesChangedPayload } from "../chat/core/chatTypes";
import { toChatError } from "../chat/core/chatErrors";
import { createLogger } from "./logger";

const log = createLogger("chatLifecycleEvents");

export function subscribeChatLifecycleEvents(): () => void {
  // ... conversation_terminated / compact_failed / unlocked / messages-changed
  // 直接写 useChatStore.setState（持久化状态）
}
```

### 3.9 UI 消费者改动

| 消费者 | 当前 | 改为 |
|--------|------|------|
| `ChatView.tsx` L76 | `const chat = useChatStore()` | `const chat = useChatStore()` + `const stream = useChatStreamStore()` |
| `ChatView.tsx` L385-388 | `chat.isStreaming` / `chat.streamingContent` 等 | `stream.isStreaming` / `stream.streamingContent` 等 |
| `ChatView.tsx` L417 | `chat.isStreaming`（MessageInput disabled） | `stream.isStreaming` |
| `ChatView.tsx` L425 | `chat.isStreaming`（MessageInput prop） | `stream.isStreaming` |
| `MessageBubble.tsx` | props 传入 | **无需改动**（props 由 ChatView 传入） |
| `useMascotChatSync.ts` L36 | 单个 `useChatStore.subscribe` | 两个独立 subscribe：`useChatStreamStore.subscribe`（isStreaming/isReasoningStreaming）+ `useChatStore.subscribe`（messages） |
| `OrbPanel.tsx` | 无流式状态消费 | **无需改动** |
| `HistoryManager.tsx` | 无流式状态消费 | **无需改动** |
| `CompactBanner.tsx` | 无流式状态消费 | **无需改动** |
| `CompactErrorBanner.tsx` | 无流式状态消费 | **无需改动** |

### 3.10 useMascotChatSync 双 subscribe 设计

```typescript
export function useMascotChatSync(policy: ExpressionPolicy = DEFAULT_EXPRESSION_POLICY): void {
  const prevStreamingRef = useRef(false);
  const prevReasoningRef = useRef(false);
  const prevErrorRef = useRef(false);

  useEffect(() => {
    const unsubStream = useChatStreamStore.subscribe((streamState) => {
      const { isStreaming, isReasoningStreaming } = streamState;
      const messages = useChatStore.getState().messages;

      const justStartedStreaming = isStreaming && !prevStreamingRef.current;
      const justStoppedStreaming = !isStreaming && prevStreamingRef.current;
      const justStoppedReasoning = !isReasoningStreaming && prevReasoningRef.current;

      // ... 表情切换逻辑（与原来相同，只是数据源从单 store 变为两 store）
      // 流式状态变化时读 chatStore.messages 用于 hadToolCall 判断

      prevStreamingRef.current = isStreaming;
      prevReasoningRef.current = isReasoningStreaming;
    });

    const unsubChat = useChatStore.subscribe((chatState) => {
      const streamState = useChatStreamStore.getState();
      // messages 变化时检查 error 状态
      const hasError = chatState.messages.some(
        (m) => m.error != null && m.role === "assistant",
      );
      if (hasError && !prevErrorRef.current) {
        useMascotStore.getState().holdThenIdle(policy.chatFace.onError);
      }
      prevErrorRef.current = hasError;
    });

    const initial = useChatStreamStore.getState();
    prevStreamingRef.current = initial.isStreaming;
    prevReasoningRef.current = initial.isReasoningStreaming;

    return () => { unsubStream(); unsubChat(); };
  }, [policy]);
}
```

**时序分析**：`finishStream` 执行时，chatStreamStore 先重置（触发 stream subscribe），然后 chatStore.messages 更新（触发 chat subscribe）。stream subscribe 回调中通过 `useChatStore.getState().messages` 读取 messages 时，messages 已同步更新完毕——因为 Zustand 的 `set()` 是同步写入内存的，`getState()` 直接读内存引用，不经过 React 调度。因此 `justStoppedStreaming` 分支读到的 `lastAssistantMsg` 内容正确。注意：React 组件的 re-render 可能被批量延迟，但这不影响 subscribe 回调中的 `getState()` 读取，两者走不同路径。

## 4. 行为等价性验证策略

### 核心原理

拆分 chatStreamStore 后，流式标量的 set/get 路径从 `chatStore.set({ isStreaming: ... })` 变为 `chatStreamStore.set({ isStreaming: ... })`。行为一致的条件：

1. **字段初始值一致**：chatStreamStore 的初始值与 chatStore 原初始值相同
2. **set 触发时机一致**：statePort 中调 chatStreamStore.set 的时机与原来调 chatStore.set 的时机相同
3. **get 返回值一致**：envPort/UI 消费者从 chatStreamStore 读到的值与原来从 chatStore 读到的值相同

### 验证方法

| 验证项 | 方法 |
|--------|------|
| 旧行为锁定 | **characterization test**：先对 chatStore 当前流式标量字段写测试，覆盖初始值 / beginSend / appendDelta / setActiveAttemptId / cancelStreaming / resetStreaming / setReasoningStreaming / setRetryStatus / finishStream / errorStream，锁定旧行为契约 |
| 新 store 行为复现 | chatStreamStore 单元测试：用与 characterization test 相同的用例结构验证 chatStreamStore，确认复现了旧 chatStore 的流式标量行为 |
| 集成后行为等价 | 集成完成后，characterization test 改为读 chatStreamStore，仍全部通过——行为等价性闭环 |
| statePort 行为等价 | chatOrchestrator 契约测试（mock statePort，验证调了正确方法） |
| UI 渲染等价 | 手动验证：发送消息 → 流式输出 → 取消 → 错误 → mascot 表情 |
| 事件监听等价 | 手动验证：conversation_terminated/compact_failed/unlocked/messages-changed 事件正常处理 |

## 5. 改造前后对比

| 维度 | 改造前 | 改造后 |
|------|--------|--------|
| chatStore 行数 | 825 | ~540 |
| chatStorePorts 行数 | — | ~285 |
| chatStreamStore 行数 | — | ~60 |
| chatStreamEvents 行数 | — | ~170 |
| chatLifecycleEvents 行数 | — | ~100 |
| chatDeltaEvents 行数 | 237 | 0（删除） |
| ChatState 公开 action 数 | 22（含 appendDelta/finishStream/errorStream） | 19（移除 3 个仅 Port 使用的） |
| ChatState 流式字段 | 6 个混在 ChatState 中 | 0（独立 chatStreamStore） |
| 流式标量重置方式 | 10 处散落 `set({ isStreaming: false, ... })` | 统一 `chatStreamStore.cancelStreaming()` |
| 跨 store 读写 | 0（全在 chatStore 内） | 0（标量委托 + messages 留 chatStore） |
| chatDeltaEvents store 依赖 | `import { useChatStore }` + 11 处调用 | chatStreamEvents 零依赖 + chatLifecycleEvents 合理依赖 |
| manualCompact Bug | 遗漏 `isReasoningStreaming: false` | 已修复 |
| chatService 契约测试 | 4/24 | 4/4 有业务逻辑的方法全覆盖（20 个 passthrough 无需测试） |
| chatOrchestrator 契约测试 | 12/~18 | ~18/~18 |

## 6. 风险与缓解

| 风险 | 缓解 | 严重度 |
|------|------|--------|
| chatStorePorts ↔ chatStore 循环依赖 | `fetchTokenUsage` 通过 `FetchTokenUsageDeps` 参数注入，chatStorePorts 零 chatStore import ✅ | 已消除 |
| useMascotChatSync 双 subscribe 时序 | stream subscribe 中同步读 `useChatStore.getState().messages`，Zustand set 同步完成，无时序问题 | 低 |
| ChatState 移除流式字段后 UI 编译报错 | 逐个 grep 消费点，全部改为 useChatStreamStore | 中 |
| chatDeltaEvents 拆分后事件丢失 | 拆分后立即手动验证全部 6 个事件 | 中 |
| chatStreamStore 忘记设 activeAttemptId 导致 orchestrator 忽略事件 | chatStreamStore 单元测试覆盖 setActiveAttemptId | 低 |
| finishStream 中先 cancelStreaming 再 set messages 的短暂窗口 | cancelStreaming 只重置标量不写 messages，messages 的合并和标量重置在同一调用栈中完成 | 低 |
| chatStreamStore 独立后被非预期模块直接依赖 | 文件头注释标注消费者白名单（ChatView / useMascotChatSync / chatStore statePort），code review 时拦截 | 低 |
| 拆分步骤引入行为偏差且发现较晚 | 每步验证门（`tsc --noEmit`）即时捕获编译偏差；行为偏差由最终冒烟测试覆盖，失败则在当前步内修复而非回滚 | 中 |

## 7. 验收标准

- [ ] `chatStreamStore.ts` 存在，持有 6 个流式标量字段 + 8 个方法
- [ ] `chatStorePorts.ts` 存在，含 `createStatePort` / `createEnvPort` / `createOrchestrator` 工厂函数
- [ ] `chatStorePorts.ts` 零 `chatStore` import（无循环依赖）
- [ ] `fetchTokenUsage` 通过 `FetchTokenUsageDeps` 参数注入，不直接操作 store
- [ ] `chatStore.ts` 中 `ChatState` 接口不含 `isStreaming`/`streamingContent`/`streamingMessageId`/`activeAttemptId`/`isReasoningStreaming`/`retryStatus`
- [ ] `chatStore.ts` 中 `ChatState` 接口不含 `appendDelta`/`finishStream`/`errorStream`
- [ ] `chatStore.ts` 中 statePort 标量部分全部委托 `useChatStreamStore.getState()`
- [ ] `chatStore.ts` 中 statePort messages 写作部分留在 chatStore
- [ ] `chatStore.ts` 中流式重置统一走 `useChatStreamStore.getState().cancelStreaming()`
- [ ] `manualCompact` 错误处理遗漏 `isReasoningStreaming` Bug 已修复
- [ ] `chatStreamEvents.ts` 存在，零 store 依赖
- [ ] `chatLifecycleEvents.ts` 存在，合理依赖 useChatStore
- [ ] `chatDeltaEvents.ts` 已删除
- [ ] `ChatOrchestrator` 接口包含 `onStreamStart` + `getActiveConversationId`
- [ ] `chat-stream-start` 事件走 `orchestrator.onStreamStart()`
- [ ] UI 消费者（ChatView/useMascotChatSync）流式状态从 `useChatStreamStore` 读取
- [ ] `useMascotChatSync` 使用两个独立 subscribe
- [ ] chatStore 行数 ≤ 550
- [ ] chatService 契约测试覆盖 4 个有业务逻辑的方法（20 个 passthrough 无需测试）
- [ ] chatOrchestrator 契约测试覆盖 send/cancel/ensureIdle/handleDeltaEvent/onTextDelta/onStreamStart/phase
- [ ] chatStreamStore 单元测试覆盖全部 8 个方法
- [ ] TypeScript 编译通过
- [ ] 功能不变：发送消息、流式输出、取消、错误处理、对话切换、消息编辑/删除/重新生成、mascot 表情切换
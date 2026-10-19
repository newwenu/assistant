# I13-Ext — Chat Store 职责细粒度拆分 任务清单

> 规格：[I13-chat-split-ext-spec.md](./I13-chat-split-ext-spec.md)
> 前置：I13 Step 1 ✅ + Step 2 ✅

---

## 验证原则

**每一步拆分紧跟一步行为等价性验证。** 核心不变式：

> 从 orchestrator 和 UI 的视角，拆分前后读到的流式标量值和 messages 内容完全一致。

验证方法：TypeScript 编译通过 + 手动冒烟测试（发送→流式→取消→错误→mascot 表情）。

---

## Step 1 — 创建 chatStreamStore.ts + chatStore statePort 委托改造（spec §3.4 + §3.5）

- [x] 1.1 新建 `src/stores/__tests__/chatStreamCharacterization.test.ts`：对 chatStore 当前 6 个流式标量字段 + 8 个相关 action 写 characterization test，覆盖初始值 / `beginSend→isStreaming=true` / `appendDelta→streamingContent累加` / `setActiveAttemptId→ID替换` / `cancelStreaming→全部重置` / `resetStreaming→全部重置` / `setReasoningStreaming` / `setRetryStatus` / `finishStream→重置+合并messages` / `errorStream→重置+写error`，**锁定旧行为契约**
- [x] **1.2 ✅ 验证**：`vitest run src/stores/__tests__/chatStreamCharacterization.test.ts` 全部通过（对旧 chatStore）
- [x] 1.3 新建 `src/stores/chatStreamStore.ts`：定义 `ChatStreamState` 接口（6 字段 + 8 方法），实现 store，初始值与 chatStore 原初始值一致
- [x] 1.4 实现 8 个方法：`beginStream`（设 isStreaming+streamingMessageId+重置其余）/ `cancelStreaming`（重置全部 6 字段）/ `appendDelta`（累加 streamingContent）/ `setReasoningStreaming` / `setActiveAttemptId`（设 activeAttemptId+更新 streamingMessageId）/ `clearActiveAttemptId` / `setRetryStatus` / `resetStreaming`（与 cancelStreaming 标量重置逻辑相同，保留独立方法名以区分语义：cancel 用于流结束/用户取消，reset 用于对话切换/删除等）
- [x] 1.5 新建 `src/stores/__tests__/chatStreamStore.test.ts`，用与 characterization test 相同的用例结构验证 chatStreamStore，确认新 store 复现了旧 chatStore 的流式标量行为
- [x] **1.6 ✅ 验证**：`vitest run src/stores/__tests__/chatStreamStore.test.ts` 全部通过，且用例与 characterization test 行为一致
- [x] 1.7 `chatStore.ts` import `useChatStreamStore`，`ChatState` 接口移除 6 个流式标量字段 + 3 个仅 Port 使用的 action（`appendDelta`/`finishStream`/`errorStream`），初始值同步移除
- [x] 1.8 statePort 标量部分委托 chatStreamStore：`beginSend` → `beginStream` + `set({ messages, contextUsage })`；`cancelStreaming` → 读流式数据 → `cancelStreaming()` → `set({ messages: 合并后 })`；`appendDelta` / `setReasoningStreaming` / `clearActiveAttemptId` / `setRetryStatus` → 纯委托；`setActiveAttemptId` → 读旧 streamingMessageId → 委托 → `set({ messages: ID 替换 })`
- [x] 1.9 statePort messages 写入部分留在 chatStore：`appendReasoningDelta` / `appendToolCallDelta` / `updateContextUsage` 不变；`finishStream` → 读流式数据 → `cancelStreaming()` → `set({ contextUsage, messages: 合并后 })` → 编排副作用；`errorStream` → 读流式数据 → `cancelStreaming()` → `set({ messages: 合并后 })`
- [x] **1.10 ✅ 验证**：`tsc --noEmit` 通过 + characterization test 改为读 chatStreamStore 后仍全部通过（行为等价性闭环）

---

## Step 2 — chatStore 流式重置统一化 + envPort 改造 + Bug 修复（spec §3.6 + §3.7）

- [x] 2.1 6 处散落的流式标量重置统一改为 `useChatStreamStore.getState().cancelStreaming()`：`switchConversation`（2 处）/ `deleteConversation` / `batchDelete` / `purgeAll` / `manualCompact` 错误处理（**同时修复遗漏 `isReasoningStreaming: false` Bug**）
- [x] 2.2 envPort 流式读取改为 chatStreamStore：`getIsStreaming()` / `getStreamingMessageId()` / `getActiveAttemptId()` 改为 `useChatStreamStore.getState().xxx`
- [x] **2.3 ✅ 验证**：`tsc --noEmit` 通过 + 手动验证 `manualCompact` 失败后 mascot 表情不再卡死

---

## Step 3 — chatOrchestrator 新增 onStreamStart + chatDeltaEvents 拆分（spec §3.7 + §3.8）

- [x] 3.1 `ChatOrchestrator` 接口新增 `onStreamStart: (attemptId: string, messageId: string) => void`，实现调 `deps.state.setActiveAttemptId(attemptId, messageId)`
- [x] 3.2 新建 `src/services/chatStreamEvents.ts`：实现 `setChatOrchestrator` + `subscribeChatStreamEvents`，`chat-stream-start` → `orchestrator.onStreamStart()`，`chat-delta` → 搬入 coalescer + seq dedup + textChunkScheduler 逻辑走 orchestrator，**零 useChatStore import**
- [x] 3.3 新建 `src/services/chatLifecycleEvents.ts`：实现 `subscribeChatLifecycleEvents`，搬入 4 个生命周期事件监听（`conversation_terminated` / `compact_failed` / `unlocked` / `messages-changed`），直接写 `useChatStore.setState`
- [x] 3.4 更新所有引用 chatDeltaEvents 的文件改为引用 chatStreamEvents + chatLifecycleEvents，删除 `src/services/chatDeltaEvents.ts`
- [x] **3.5 ✅ 验证**：`tsc --noEmit` 通过 + 手动验证 6 个事件全部正常处理

---

## Step 4 — UI 消费者迁移（spec §3.9 + §3.10）

- [x] 4.1 `ChatView.tsx`：新增 `const stream = useChatStreamStore()`，5 个流式字段迁移（`isStreaming` / `streamingContent` / `streamingMessageId` / `isReasoningStreaming` / `retryStatus`），移除从 `useChatStore` 解构的流式字段
- [x] 4.2 `useMascotChatSync.ts`：改为两个独立 subscribe（`useChatStreamStore.subscribe` 监听 isStreaming/isReasoningStreaming + `useChatStore.subscribe` 监听 messages/error），stream subscribe 中同步读 `useChatStore.getState().messages`，初始值从 `useChatStreamStore.getState()` 读取
- [x] 4.3 grep 确认其他 UI 文件无遗漏的流式字段直接引用
- [x] **4.4 ✅ 验证**：`tsc --noEmit` 通过 + 手动验证 mascot 表情切换（idle→thinking→speaking→happy→idle）

---

## Step 5 — 补齐测试（spec §7 验收标准）

### 5A — chatOrchestrator 契约测试

- [x] 5B.1 补齐 `cancel` 测试（idle 无副作用 / streaming → cancelChat + cancelStreaming）+ `ensureIdle` 测试 + `handleDeltaEvent` finish_reason/error 测试 + `onTextDelta` + `onStreamStart` 测试 + ChatPhase 转换测试（idle→sending→streaming→idle / tool_executing / error）
- [x] **5B.2 ✅ 验证**：`vitest run src/services/__tests__/chatOrchestrator.test.ts` 全部通过（14/14）

### 5C — chatService 契约测试

- [x] 5C.1 补齐 4 个有业务逻辑的方法测试（null fallback / DTO 转换）：`listConversations` / `getConversationPaginated` / `getConversationAllMessages` / `getConversationTokenUsage`。其余 20 个纯 passthrough 方法（`invoke("cmd", args)` 零变换）无需单独测试
- [x] **5C.2 ✅ 验证**：`vitest run src/services/__tests__/chatService.test.ts` 全部通过（6/6）

---

## Step 6 — chatStorePorts 提取 + 行数达标

- [x] 6.1 提取 statePort/envPort/orchestrator 创建到 `chatStorePorts.ts`，chatStore 行数降至 ≤ 550
- [x] 6.2 提取 `resolveContextWindowTokens`/`fetchTokenUsage`/`mergeStreamingContent` 辅助函数到 chatStorePorts
- [x] 6.3 chatStreamStore 精简至 ≤ 70 行（resetState 共享对象）
- [x] 6.4 消除 chatStorePorts ↔ chatStore 循环依赖：`fetchTokenUsage` 改为 `FetchTokenUsageDeps` 参数注入（`isActiveConv` + `setContextUsage`），chatStorePorts 移除 `import { useChatStore }`
- [x] 6.5 简化 `useMascotChatSync` 双 subscribe：`unsubscribeChat` 仅保留 error 处理，流结束表情统一由 `unsubscribeStream` 处理
- [x] 6.6 删除 `chatStore.ts` 中无用 `export { resolveContextWindowTokens } from "./chatStorePorts"` re-export
- [x] **6.7 ✅ 验证**：chatStore 533 行 / chatStreamStore 63 行 / chatStorePorts 零 chatStore import / tsc + eslint 通过

---

## Step 7 — 测试环境修复

- [x] 7.1 新建 `src/testSetup.ts`，提供 `window` + `import.meta.env` 全局 mock
- [x] 7.2 `vitest.config.ts` 添加 `setupFiles`
- [x] 7.3 chatOrchestrator.test.ts 从 `window is not defined` 恢复为 14/14 通过
- [x] **7.4 ✅ 验证**：`vitest run` 131/131 全部通过

---

## 最终验收

- [x] F1. `tsc --noEmit` 通过
- [x] F2. `eslint` 0 errors
- [x] F3. `vitest run` 全部通过（131/131）
- [x] F4. chatStore 行数 ≤ 550（533）
- [x] F5. chatStreamStore 行数 ≤ 70（63）
- [x] F6. chatStreamEvents 零 `useChatStore` import
- [x] F7. chatDeltaEvents.ts 已删除
- [x] F9. chatStorePorts.ts 存在，零 `chatStore` import（无循环依赖）
- [x] F10. `fetchTokenUsage` 通过 `FetchTokenUsageDeps` 参数注入
- [x] F11. `ChatOrchestrator` 接口包含 `getActiveConversationId`
- [x] F12. chatService 契约测试覆盖 4 个有业务逻辑的方法（20 个 passthrough 无需测试）
- [ ] F8. 手动冒烟测试通过：
  - 发送消息 → 流式输出 → 完成
  - 发送消息 → 取消
  - 发送消息 → Provider 错误
  - 对话切换 → 流式状态正确重置
  - mascot 表情：idle → thinking → speaking → happy → idle
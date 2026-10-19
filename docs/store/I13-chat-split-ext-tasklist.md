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

- [ ] 1.1 新建 `src/stores/chatStreamStore.ts`：定义 `ChatStreamState` 接口（6 字段 + 8 方法），实现 store，初始值与 chatStore 原初始值一致
- [ ] 1.2 实现 8 个方法：`beginStream`（设 isStreaming+streamingMessageId+重置其余）/ `cancelStreaming`（重置全部 6 字段）/ `appendDelta`（累加 streamingContent）/ `setReasoningStreaming` / `setActiveAttemptId`（设 activeAttemptId+更新 streamingMessageId）/ `clearActiveAttemptId` / `setRetryStatus` / `resetStreaming`（与 cancelStreaming 相同实现）
- [ ] 1.3 `chatStore.ts` import `useChatStreamStore`，`ChatState` 接口移除 6 个流式标量字段 + 3 个仅 Port 使用的 action（`appendDelta`/`finishStream`/`errorStream`），初始值同步移除
- [ ] 1.4 statePort 标量部分委托 chatStreamStore：`beginSend` → `beginStream` + `set({ messages, contextUsage })`；`cancelStreaming` → 读流式数据 → `cancelStreaming()` → `set({ messages: 合并后 })`；`appendDelta` / `setReasoningStreaming` / `clearActiveAttemptId` / `setRetryStatus` → 纯委托；`setActiveAttemptId` → 读旧 streamingMessageId → 委托 → `set({ messages: ID 替换 })`
- [ ] 1.5 statePort messages 写入部分留在 chatStore：`appendReasoningDelta` / `appendToolCallDelta` / `updateContextUsage` 不变；`finishStream` → 读流式数据 → `cancelStreaming()` → `set({ contextUsage, messages: 合并后 })` → 编排副作用；`errorStream` → 读流式数据 → `cancelStreaming()` → `set({ messages: 合并后 })`
- [ ] **1.6 ✅ 验证**：`tsc --noEmit` 通过

---

## Step 2 — chatStore 流式重置统一化 + envPort 改造 + Bug 修复（spec §3.6 + §3.7）

- [ ] 2.1 6 处散落的流式标量重置统一改为 `useChatStreamStore.getState().cancelStreaming()`：`switchConversation`（2 处）/ `deleteConversation` / `batchDelete` / `purgeAll` / `manualCompact` 错误处理（**同时修复遗漏 `isReasoningStreaming: false` Bug**）
- [ ] 2.2 envPort 流式读取改为 chatStreamStore：`getIsStreaming()` / `getStreamingMessageId()` / `getActiveAttemptId()` 改为 `useChatStreamStore.getState().xxx`
- [ ] **2.3 ✅ 验证**：`tsc --noEmit` 通过 + 手动验证 `manualCompact` 失败后 mascot 表情不再卡死

---

## Step 3 — chatOrchestrator 新增 onStreamStart + chatDeltaEvents 拆分（spec §3.7 + §3.8）

- [ ] 3.1 `ChatOrchestrator` 接口新增 `onStreamStart: (attemptId: string, messageId: string) => void`，实现调 `deps.state.setActiveAttemptId(attemptId, messageId)`
- [ ] 3.2 新建 `src/services/chatStreamEvents.ts`：实现 `setChatOrchestrator` + `subscribeChatStreamEvents`，`chat-stream-start` → `orchestrator.onStreamStart()`，`chat-delta` → 搬入 coalescer + seq dedup + textChunkScheduler 逻辑走 orchestrator，**零 useChatStore import**
- [ ] 3.3 新建 `src/services/chatLifecycleEvents.ts`：实现 `subscribeChatLifecycleEvents`，搬入 4 个生命周期事件监听（`conversation_terminated` / `compact_failed` / `unlocked` / `messages-changed`），直接写 `useChatStore.setState`
- [ ] 3.4 更新所有引用 chatDeltaEvents 的文件改为引用 chatStreamEvents + chatLifecycleEvents，删除 `src/services/chatDeltaEvents.ts`
- [ ] **3.5 ✅ 验证**：`tsc --noEmit` 通过 + 手动验证 6 个事件全部正常处理

---

## Step 4 — UI 消费者迁移（spec §3.9 + §3.10）

- [ ] 4.1 `ChatView.tsx`：新增 `const stream = useChatStreamStore()`，5 个流式字段迁移（`isStreaming` / `streamingContent` / `streamingMessageId` / `isReasoningStreaming` / `retryStatus`），移除从 `useChatStore` 解构的流式字段
- [ ] 4.2 `useMascotChatSync.ts`：改为两个独立 subscribe（`useChatStreamStore.subscribe` 监听 isStreaming/isReasoningStreaming + `useChatStore.subscribe` 监听 messages/error），stream subscribe 中同步读 `useChatStore.getState().messages`，初始值从 `useChatStreamStore.getState()` 读取
- [ ] 4.3 grep 确认其他 UI 文件无遗漏的流式字段直接引用
- [ ] **4.4 ✅ 验证**：`tsc --noEmit` 通过 + 手动验证 mascot 表情切换（idle→thinking→speaking→happy→idle）

---

## Step 5 — 补齐测试（spec §7 验收标准）

### 5A — chatStreamStore 单元测试

- [ ] 5A.1 新建 `src/stores/__tests__/chatStreamStore.test.ts`，覆盖全部 8 个方法 + `beginStream → appendDelta → cancelStreaming` 完整流程
- [ ] **5A.2 ✅ 验证**：`vitest run src/stores/__tests__/chatStreamStore.test.ts` 全部通过

### 5B — chatOrchestrator 契约测试

- [ ] 5B.1 补齐 `cancel` 测试（idle 无副作用 / streaming → cancelChat + cancelStreaming）+ `ensureIdle` 测试 + `handleDeltaEvent` finish_reason/error 测试 + `onTextDelta` + `onStreamStart` 测试 + ChatPhase 转换测试（idle→sending→streaming→idle / tool_executing / error）
- [ ] **5B.2 ✅ 验证**：`vitest run src/services/__tests__/chatOrchestrator.test.ts` 全部通过

### 5C — chatService 契约测试

- [ ] 5C.1 补齐全部 24 个方法测试（当前仅 4 个），含 null 返回值 fallback 测试
- [ ] **5C.2 ✅ 验证**：`vitest run src/services/__tests__/chatService.test.ts` 全部通过

---

## 最终验收

- [ ] F1. `tsc --noEmit` 通过
- [ ] F2. `eslint` 0 errors
- [ ] F3. `vitest run` 全部通过
- [ ] F4. chatStore 行数 ≤ 550
- [ ] F5. chatStreamStore 行数 ≤ 70
- [ ] F6. chatStreamEvents 零 `useChatStore` import
- [ ] F7. chatDeltaEvents.ts 已删除
- [ ] F8. 手动冒烟测试通过：
  - 发送消息 → 流式输出 → 完成
  - 发送消息 → 取消
  - 发送消息 → Provider 错误
  - 对话切换 → 流式状态正确重置
  - mascot 表情：idle → thinking → speaking → happy → idle
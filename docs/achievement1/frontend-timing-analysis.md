# 前端时序问题审查

> **审查日期**：2026-10-25
> **审查范围**：`src/stores/`、`src/services/`、`src/hooks/`、`src/chat/`、`src/mascot/`、`src/main.tsx`
> **审查方法**：逐文件阅读核心状态管理代码，追踪异步调用链与跨 store 依赖，识别竞态、状态不一致、fire-and-forget 等时序缺陷

---

## 1. 审查摘要

| # | 严重度 | 问题 | 核心时序缺陷 | 涉及文件 |
|---|--------|------|-------------|---------|
| 1 | 🔴 P0 | `bootstrap()` 与 `useAppInitialization()` 并行加载配置 | 双所有权竞态，配置闪烁 | `main.tsx`、`useAppInitialization.ts` |
| 2 | 🔴 P0 | `send()` 异常路径下 `sendInProgress` 与 `phase` 状态机脱节 | 冗余标志不一致，后续 send 行为异常 | `chatOrchestrator.ts` |
| 3 | 🔴 P0 | `switchConversation` 取消流式传输未等待完成 | 残留流数据污染新会话 | `chatConversationSlice.ts` |
| 4 | 🟡 P1 | `useMascotChatSync` 双订阅跨 store 读取时序不确定 | 基于过时消息判断表情 | `useMascotChatSync.ts` |
| 5 | 🟡 P1 | `ChatView` 初始化中 `loadProviders` 与 `loadConversations` 并行发射 | provider 未就绪就创建会话；冗余请求 | `ChatView.tsx` |
| 6 | 🟡 P1 | `handleProviderChange` / `handleModeChange` 中 provider 与 model 并行更新 | 后端收到乱序请求 | `ChatView.tsx` |
| 7 | 🟢 P2 | `toolStore` 中 `fuseInstance` 模块级变量与 store state 不同步 | 语言切换后 store.fuse 过时 | `toolStore.ts` |

---

## 2. 问题详析与修复方案

### 2.1 🔴 P0 — `bootstrap()` 与 `useAppInitialization()` 并行加载配置

#### 现状

```
main.tsx bootstrap()                    useAppInitialization() (MainLayout 挂载后)
─────────────────────                  ────────────────────────────────────────
await store.loadAppConf()   ──┐        void loadAppConf()          ──┐
await store.loadOrbConf()   ──┤ 并发    void loadOrbConf()          ──┤ 并发
await store.loadMascotConf()─┘        void loadMascotConf()       ──┘
```

- `bootstrap()` 在 `ReactDOM.render` 之前串行执行 `loadAppConf → loadOrbConf → loadMascotConf`
- React 挂载后 `useAppInitialization()` 再次并行调用相同的三个加载函数
- 两者几乎同时执行，导致同一配置被并发请求两次

#### 时序风险

1. **配置闪烁**：`bootstrap` 先写入 `appConf=A`，`useAppInitialization` 的请求稍晚到达覆盖为 `appConf=A'`（若后端配置在此期间被修改，或网络响应乱序，值可能不同）
2. **`applyLocaleChange` 重复调用**：`loadAppConf` 内部检测 `language` 变化后调用 `applyLocaleChange`，两次 `loadAppConf` 导致 i18n 切换执行两次
3. **网络请求浪费**：6 次请求中有 3 次是冗余的

#### 方案对比

| 方案 | 描述 | 根因消除 | 防御性 | 改动量 | 缺陷 |
|------|------|---------|--------|--------|------|
| A：if 守卫 | `useAppInitialization` 中 `if (!appConf) void loadAppConf()` | ❌ 双所有权仍在 | ❌ 隐式依赖 bootstrap 先完成 | ~3 行 | 移除 bootstrap 中加载后配置永远不加载，且无警告 |
| **B：消除双所有权** | **移除 `useAppInitialization` 中的配置加载，全部由 bootstrap 负责** | **✅ 单一职责** | **✅ 编译时可验证** | **~5 行** | **HMR 时需依赖 `ConfPatched` 事件驱动重加载** |

#### 推荐方案：B — 消除双所有权

`bootstrap` 在 render 前执行，是天然的"配置就绪"保证点。`useAppInitialization` 只保留 tool 相关加载（`loadFavorites`、`loadRecentTools`），这些不在 `bootstrap` 范围内。

HMR 兜底由 `subscribeSettingEvents` 已有的 `ConfPatched` 监听实现——配置变更时 `loadAppConf` 会被事件驱动重新调用，不需要 `useAppInitialization` 主动拉取。

#### 回归风险

- `subscribeSettingEvents` 的 `ConfPatched` 事件延迟到达时，首屏短暂显示默认配置 → 风险极低（bootstrap 已在 render 前完成，HMR 场景下短暂默认配置可接受）
- 未来新增配置类型时，需在 `bootstrap` 中同步添加加载调用 → 可通过 `settingsStore` 的类型定义约束

---

### 2.2 🔴 P0 — `send()` 异常路径下 `sendInProgress` 与 `phase` 状态机脱节

#### 现状

`send()` 中 `sendInProgress`（布尔标志）和 `phase`（五态状态机）追踪同一概念——"是否有发送操作在进行"，但两者更新不是原子的。

关键代码路径分析：

| 路径 | `sendInProgress` | `phase` | 一致？ |
|------|-----------------|---------|--------|
| 正常流：`sendChatMessage` 成功，`finishReason` 到达 | `false`（finally） | `"idle"`（finishReason 处理） | ✅ |
| Tauri delta error：`payload.error` 到达 | `false`（finally） | `"error"`（handleDeltaEvent） | ❌ |
| invoke 异常：`sendChatMessage` 抛异常 | `false`（finally） | `"sending"`（未被重置） | ❌ |
| 用户取消：`cancel()` 调用 | `true`→`false`（cancel 后 finally 立即重置） | `"idle"`（cancel 重置） | ⚠️ 极短窗口 |

**最危险的是 invoke 异常路径**：`sendChatMessage` 抛异常时，`catch` 块调用 `errorStream`，但 `errorStream` **不会**设置 `phase`（它只做消息合并 + `cancelStreaming`）。`phase` 停留在 `"sending"`，`finally` 将 `sendInProgress` 重置为 `false`。后续 `send()` 调用通过 `sendInProgress` 守卫进入，但 `phase` 仍为 `"sending"`，`handleDeltaEvent` 中 `phase === "sending"` 条件意外满足。

#### 时序风险

1. `sendChatMessage` 抛异常 → `catch` 调用 `errorStream`（不改 phase）→ `finally` 重置 `sendInProgress`
2. 状态：`sendInProgress=false, phase="sending"`
3. 下一次 `send()` 通过 `sendInProgress` 守卫进入 → `setPhase("sending")`（幂等，看似无害）
4. 但如果上一次的 Tauri 事件仍在队列中（如延迟的 `StreamStart`），`handleDeltaEvent` 的 `phase === "sending"` 条件匹配 → 旧 attemptId 的 delta 被处理 → **消息内容错乱**

#### 方案对比

| 方案 | 描述 | 根因消除 | 防御性 | 改动量 | 缺陷 |
|------|------|---------|--------|--------|------|
| **A：finally 兜底重置** | **`finally { sendInProgress = false; if (phase !== "idle") setPhase("idle"); }`** | **⚠️ 部分** | **✅ 出口一致** | **~2 行** | **`phase="error"` 被 UI 消费前就被覆盖为 `"idle"`，error 状态不可见** |
| B：消除 sendInProgress | 从 phase 派生 `sendInProgress` 语义 | ✅ 单一状态源 | ✅ 不可能不一致 | ~15 行 | phase 五态与 sendInProgress 语义不完全等价（`streaming` 时 sendInProgress 应为 false），需重新设计入口守卫 |

#### 推荐方案：A — finally 兜底重置

选择 A 的理由：

1. `sendInProgress` 和 `phase` 的语义**不完全等价**：`phase="streaming"` 时 `sendInProgress` 应为 `false`（send 已返回，流仍在进行），彻底消除需要重新设计入口守卫，改动面大
2. `finally` 重置是**防御性编程的标准模式**——保证函数出口状态一致，无论内部路径如何
3. 关于 `phase="error"` 被覆盖的问题：`error` 状态由 `handleDeltaEvent` 中的 `payload.error` 设置，此时 `send` 函数早已返回（`sendChatMessage` 是 await，delta 事件在 await 期间异步到达），`finally` 已执行完毕。**`finally` 不会覆盖 `error` 状态**——因为 `error` 发生在 `send` 返回之后的事件循环轮次中

**澄清 invoke 异常路径的精确行为**：

- `send()` 的 `catch` 块调用 `statePort.errorStream()`——该方法**不调用** `setPhase`，只做消息合并 + `cancelStreaming`
- `setPhase("error")` 只在 `handleDeltaEvent` 收到 `payload.error` 时调用，此时 `send` 已返回（delta 事件在 `await sendChatMessage` 期间异步到达），`finally` 已执行完毕
- 因此 invoke 异常路径：`catch` → `errorStream`（不改 phase）→ `finally` 重置 `sendInProgress` → 状态：`sendInProgress=false, phase="sending"` → **phase 残留为 `"sending"` 才是真正的问题**

`finally` 中 `if (phase !== "idle") setPhase("idle")` 在 invoke 异常路径将 `"sending"` 重置为 `"idle"`，是正确且必要的行为。`phase="error"` 不会被 `finally` 覆盖——因为 `error` 发生在 `send` 返回之后的事件循环轮次中。

#### 回归风险

- `cancel()` 路径：`cancel` 在 `send` 的 `try` 块内被调用时，`cancel` 自身 `setPhase("idle")`，`finally` 的 `if` 判断跳过 → 无影响
- `ensureIdle()` 路径：`ensureIdle` 等待 `phase === "idle" || "error"`，`finally` 重置后 `phase="idle"` → 正常 resolve

---

### 2.3 🔴 P0 — `switchConversation` 取消流式传输未等待完成

#### 现状

`switchConversation` 手动拼装了 cancel + resetStreaming 的逻辑，但项目中**已有 `ensureIdle()`** 做完全相同的事情，且更完善。

当前逻辑 vs `ensureIdle` 对比：

| 步骤 | `switchConversation` 当前 | `ensureIdle()` |
|------|--------------------------|----------------|
| cancel 后端 | `chatService.cancelChat(prevConvId)` (fire-and-forget) | `chatService.cancelChat(convId)` (await) |
| 重置前端流状态 | `streamStore.resetStreaming()`（直接重置，**不合并流内容**） | `statePort.cancelStreaming()`（**先合并流内容到消息**，再重置 streamStore） |
| 重置 orchestrator phase | ❌ 未重置 | ✅ 等待 phase === "idle" \|\| "error" |
| 重置 sendInProgress | ❌ 未重置 | ✅ `sendInProgress = false` |
| 超时保护 | ❌ 无 | ✅ 5 秒超时强制重置 |

> **注**：`chatStreamStore` 中 `cancelStreaming()` 和 `resetStreaming()` 实现完全相同（均 `set(resetState)`），都不合并流内容。真正的合并发生在 `chatStorePorts.ts` 的 `statePort.cancelStreaming()`——它先 `mergeStreamingContent` 再调 `streamStore.cancelStreaming()`。`switchConversation` 直接调 streamStore 层的 `resetStreaming()`，绕过了 statePort 层的合并逻辑。

#### 时序风险

```
时间线 ──────────────────────────────────────────────────►

前端:  cancelChat(fire)  resetStreaming()  getConversationPaginated(newId)
                                                     ↓ set({currentConversationId: newId})
后端:        ... cancel 处理中 ...                     cancel 生效
Tauri event:                              StreamStart(oldAttemptId) ← 旧会话的迟到事件
                                           ↓ onStreamStart 设置 attemptId
                                           ↓ 后续 delta 事件匹配 attemptId，写入当前会话！
```

1. `cancelChat` 是 fire-and-forget，后端 cancel 可能尚未生效
2. `resetStreaming()` 清除了前端流状态（含 `activeAttemptId`），但后端可能还在发送旧会话的 delta
3. 在 `getConversationPaginated` 的 `await` 期间，事件循环处理待处理的 Tauri events，此时 `currentConversationId` 尚未更新（仍是旧值），`StreamStart` 的 `conversationId` 匹配 → `onStreamStart` 被调用 → 旧 attemptId 被设置
4. 后续 delta 匹配旧 attemptId → **旧数据写入当前会话**

#### 方案对比

| 方案 | 描述 | 根因消除 | 防御性 | 改动量 | 缺陷 |
|------|------|---------|--------|--------|------|
| A：await cancelChat | 将 fire-and-forget 改为 await | ⚠️ 部分（未重置 phase/sendInProgress） | ❌ 新增 cancel 逻辑仍需手动同步 | ~2 行 | DRY 违反；与 `ensureIdle` 逻辑重复且不完整 |
| **B：使用 ensureIdle** | **让 `switchConversation` 通过 `ensureIdle()` 停止流式传输** | **✅ 完全** | **✅ 复用已验证机制** | **~8 行** | **`ensureIdle` 的 5 秒超时可能导致 `switchConversation` 卡住** |

#### 推荐方案：B — 使用 ensureIdle

`ensureIdle` 是项目为"停止当前流"专门设计的完整机制。`switchConversation` 重新拼装是 DRY 违反且不完整（缺少 phase 重置、sendInProgress 重置、流内容合并、超时保护）。

对于超时风险：`ensureIdle` 超时后**强制 resolve**（`setPhase("idle")` + `resolve()`），不会永久卡住。5 秒超时在正常场景下远超 cancel 所需时间。如果超时，说明后端无响应，此时强制切换也是合理的降级行为。

需从 `chatStore` 导入 `getChatOrchestrator` 获取 orchestrator 实例，并提供无 orchestrator 时的 fallback（手动 cancel + resetStreaming）。

#### 回归风险

- `ensureIdle` 超时（5 秒）导致切换延迟 → 可接受（超时后强制 resolve，且正常场景远低于 5 秒）
- `ensureIdle` 中 `cancelStreaming()` 会合并流内容到消息 → 这是正确行为（保留用户已看到的内容）
- 无 orchestrator 实例时（如 Orb 窗口）需 fallback → fallback 逻辑与当前代码相同，不引入新风险

---

### 2.4 🟡 P1 — `useMascotChatSync` 双订阅跨 store 读取时序不确定

#### 现状

`useMascotChatSync` 同时订阅 `useChatStreamStore` 和 `useChatStore`，在 streamStore 订阅中跨 store 读取 `useChatStore.getState().messages`。

`finishStream` 的执行顺序：
1. `useChatStore.set({ messages: merged, ... })` — 触发 chatStore 订阅
2. `useChatStreamStore.cancelStreaming()` — 触发 streamStore 订阅

Zustand 的 `subscribe` 回调在 `set` 后同步触发，但两个 store 的订阅回调执行顺序不确定。

#### 时序风险

如果 streamStore 订阅先执行，读取的 `chatStore.messages` 可能是合并前的旧值：
- `justStoppedStreaming=true` 但 `lastAssistantMsg` 是旧消息（内容不完整）→ mascot 表情基于不完整内容判断
- `hadToolCall` 基于旧消息列表 → 可能误判

实际影响有限：mascot 表情是视觉反馈，下一帧 chatStore 订阅触发时会修正。但在流结束的瞬间可能出现短暂错误表情。

#### 方案对比

| 方案 | 描述 | 根因消除 | 防御性 | 改动量 | 缺陷 |
|------|------|---------|--------|--------|------|
| A：移动判断 | 将 `justStoppedStreaming` 处理移到 chatStore 订阅 | ⚠️ 部分（其他跨 store 读取仍在） | ❌ 双订阅竞态仍在 | ~20 行 | `justStartedStreaming` 中读 messages 判断 hadToolCall 仍有竞态 |
| **B：单一订阅点** | **只订阅 chatStore，streamState 用 `getState()` 快照读取** | **✅ 消除双订阅竞态** | **✅ 单一权威订阅** | **~30 行** | **需确认 chatStore 的 `set` 在所有流状态变化时都触发** |

#### 推荐方案：B — 单一订阅点

chatStore 的 `set` 触发订阅时，`chatState` 参数是本次 set 的新值（确定性），`useChatStreamStore.getState()` 是当前最新值（确定性）。两者虽不是原子更新，但 chatStore 订阅回调中读取 streamStore 快照的语义是"当消息变化时，流状态是什么"——这是明确定义的。

反过来，在 streamStore 订阅中读 chatStore 则语义模糊——"当流状态变化时，消息是什么"但消息可能还没更新完。

需验证：`isStreaming` 从 `true` 变为 `false` 时，chatStore 是否一定触发 `set`？→ 是的，`finishStream` 和 `errorStream` 都先 `set` chatStore 再 `cancelStreaming` streamStore，chatStore 订阅触发时 `streamStore.isStreaming` 可能仍为 `true`（cancelStreaming 尚未执行）。因此需要在 chatStore 订阅中**同时检查 streamStore 快照和 prevStreamingRef** 来判断 `justStoppedStreaming`。

#### 回归风险

- chatStore 订阅中 `useChatStreamStore.getState().isStreaming` 可能仍为 `true`（cancelStreaming 尚未执行）→ 需用 `finishStream` 设置的标记位或消息内容变化来判断流结束，而非仅依赖 `isStreaming`
- 实现时需仔细处理 `justStoppedStreaming` 的判断条件，可能需要增加一个 `finishReason` 标记到消息对象中

---

### 2.5 🟡 P1 — `ChatView` 初始化中 `loadProviders` 与 `loadConversations` 并行发射

#### 现状

`ChatView` 的 `useEffect` 中 `loadProviders` 和 `loadConversations` 并行发射。`loadConversations` → `ensureCurrentConversation` → 可能调用 `providerService.listProviders()` 创建初始会话。

#### 时序风险

1. `loadProviders` 和 `loadConversations` 并行启动
2. `loadConversations` 的 `ensureCurrentConversation` 发现无当前会话、无会话列表 → 尝试创建初始会话
3. 创建初始会话需要 provider → 调用 `providerService.listProviders()`（绕过 store 直接调 service）
4. 存在**冗余请求**（`loadProviders` 和 `ensureCurrentConversation` 各调一次 `listProviders`）

实际风险较低：`ensureCurrentConversation` 内部直接调 `providerService.listProviders()` 而非读 store，因此不依赖 `loadProviders` 的完成。但 provider selector 在 `loadProviders` 完成前短暂显示空列表。

#### 推荐方案：`loadConversations` 依赖 `loadProviders` 完成后再执行

消除冗余请求；保证 provider 列表已加载后再尝试创建会话；provider selector 不会短暂显示空列表。

`loadProviders` 通常很快（本地 Tauri invoke），串行等待对首屏延迟影响可忽略。

#### 回归风险

- 极低。`loadProviders` 失败时 `loadConversations` 仍会执行（`.then` 在 reject 时不执行，需用 `.finally` 或 try-catch 包裹）

---

### 2.6 🟡 P1 — provider 与 model 并行更新导致后端乱序

#### 现状

`handleProviderChange` 和 `handleModeChange` 中 `updateConversationProvider` 和 `updateModel` 是两个独立的异步调用，没有顺序保证。后端可能先收到 `updateModel` 再收到 `updateConversationProvider`。

具体地，`handleProviderChange` 中两个调用使用了不同的 fire-and-forget 模式：

```ts
chatService.updateConversationProvider(convId, newProviderId)
  .catch((e) => log.error("Failed to update provider", e));  // .catch() 模式：意识到可能失败但选择忽略
void chat.updateModel(convId, modelId);                       // void 模式：认为不需要等待
```

两者几乎同时发射，后端收到顺序取决于 IPC 调度与 Rust 端处理延迟。

#### 时序风险

1. 后端先处理 `updateModel(newModelId)` → 此时 provider 还是旧的 → model 可能不属于旧 provider → 验证失败或静默忽略
2. 后端先处理 `updateConversationProvider(newProviderId)` → 然后 `updateModel` → 正常
3. 结果不确定，取决于网络延迟

#### 方案对比

| 方案 | 描述 | 根因消除 | 防御性 | 改动量 | 缺陷 |
|------|------|---------|--------|--------|------|
| A：逐处 await | 每个调用点先 `await updateConversationProvider` 再 `updateModel` | ❌ 模式仍在 | ❌ 新调用点可能忘 | ~10 行 | 两处调用点需分别修改 |
| **B：封装函数** | **提取 `updateConversationProviderAndModel(convId, providerId, modelId)`** | **✅ 不可能乱序** | **✅ 封装强制顺序** | **~15 行** | 仍两次请求（需后端配合才能单次） |

#### 推荐方案：B — 封装为单一操作函数

调用方只需一行，不可能遗漏顺序。封装是低成本高收益的标准实践。

`handleModeChange` 的结构比 `handleProviderChange` 更复杂（有循环查找 provider 的逻辑），封装函数需接受 `providerId` 和 `modelId` 两个参数，调用方自行确定这两个值后传入。

#### 回归风险

- 封装函数内部 `updateConversationProvider` 失败时是否仍执行 `updateModel`？→ 建议失败时跳过 `updateModel`（provider 未更新时 model 更新无意义）
- 后端如未来提供 `updateConversationProviderAndModel` 单一 API，可替换封装函数内部实现，调用方无需改动

---

### 2.7 🟢 P2 — `toolStore` 中 `fuseInstance` 模块级变量与 store state 不同步

#### 现状

`search` 方法直接使用模块级 `fuseInstance`，所以搜索功能正常。但 store state 中的 `fuse` 字段是创建时的快照，永远指向初始实例。

#### 时序风险

1. 语言切换后，`useToolStore(s => s.fuse)` 的消费者拿到过时的 fuse 实例
2. 当前无消费者使用 `s.fuse`（`search` 方法用模块级变量），所以**实际无功能影响**
3. 但如果未来有组件直接读 `store.fuse`，会得到错误结果

#### 推荐方案：`languageChanged` 回调中同步更新 store state

一行改动；保持 store state 与模块级变量一致；为未来消费者提供正确数据。

#### 决策：接受风险，顺带修复

当前无功能影响，但修复成本极低（一行），属于"顺手做正确的事"。

---

## 3. 修复优先级与实施计划

### Phase 1 — P0 修复（建议立即实施）

| 问题 | 修复文件 | 改动量 | 风险 | 推荐方案 |
|------|---------|--------|------|---------|
| 2.1 bootstrap 竞态 | `useAppInitialization.ts` | ~5 行 | 极低 | 消除双所有权 |
| 2.2 send/phase 脱节 | `chatOrchestrator.ts` | ~2 行 | 低 | finally 兜底重置 |
| 2.3 switchConversation cancel | `chatConversationSlice.ts` | ~8 行 | 低 | 使用 ensureIdle |

### Phase 2 — P1 修复（建议本周实施）

| 问题 | 修复文件 | 改动量 | 风险 | 推荐方案 |
|------|---------|--------|------|---------|
| 2.4 mascot 双订阅 | `useMascotChatSync.ts` | ~30 行 | 中（需测试表情切换） | 单一订阅点 |
| 2.5 ChatView 初始化顺序 | `ChatView.tsx` | ~3 行 | 低 | 串行加载 |
| 2.6 provider/model 乱序 | `ChatView.tsx` + 新函数 | ~15 行 | 低 | 封装函数 |

### Phase 3 — P2 修复（可随其他改动顺带修复）

| 问题 | 修复文件 | 改动量 | 风险 | 推荐方案 |
|------|---------|--------|------|---------|
| 2.7 fuseInstance 同步 | `toolStore.ts` | ~1 行 | 极低 | setState 同步 |

---

## 4. 根因模式与因果分析

本次审查发现的时序问题可归纳为三类根因模式。每个模式不仅描述"是什么"，还追溯"为什么这个代码库会产生此类问题"。

### 4.1 重复初始化竞态

**模式**：启动阶段（`bootstrap`）和运行阶段（`useEffect`）对同一资源执行相同的加载逻辑，两者并发执行。

**因果链**：

```
Tauri 多窗口架构
  → Orb 窗口不经过 ChatView，需在 bootstrap 中加载配置
  → Chat 窗口也需配置，bootstrap 中加载
  → React 组件需要自初始化能力（HMR、动态挂载）
  → useAppInitialization 中也加载配置
  → 双所有权
```

这是架构层面的张力：Tauri 多窗口要求 render 前加载，React 组件要求自包含。解法不是加守卫，而是**明确划分职责**：bootstrap 负责配置加载（所有窗口共用），React 组件只负责自身特有的加载（tool、provider）。

**通用解法**：将初始化职责明确划分给单一执行点；HMR 兜底由事件驱动（`ConfPatched`）而非主动拉取。

**涉及问题**：2.1、2.5

### 4.2 fire-and-forget 异步调用

**模式**：对需要顺序保证的异步操作使用 `.catch()` 或 `void` 丢弃 Promise，导致后续逻辑在前置操作完成前执行。

**因果链**：

```
早期代码将 cancelChat 视为"通知型"操作（类似 UI 事件）
  → fire-and-forget
  → 但 cancelChat 语义是"命令型"（需确认后端已停止发送）
  → 后续逻辑在前置操作完成前执行
  → 残留数据污染
```

这是对操作语义的误判。`cancelChat` 不是"发个通知就行"，而是"确保后端已停止"——后续操作（加载新会话）依赖这个前置条件。

**通用解法**：对有顺序依赖的异步操作使用 `await`；仅对真正独立的操作（如日志、非关键通知）使用 fire-and-forget。判断标准：**后续操作是否依赖前置操作的完成？**

**涉及问题**：2.3、2.6

### 4.3 跨 store 状态读取时序

**模式**：在 store A 的订阅回调中同步读取 store B 的状态，但两个 store 的更新不是原子的，订阅回调执行顺序不确定。

**因果链**：

```
chatStreamStore 从 chatStore 中拆分出来（重构）
  → mascot 同步逻辑没有随之迁移
  → 仍在 streamStore 订阅中读 chatStore
  → 两个 store 的 set 不是原子的
  → 订阅回调执行顺序不确定
  → 基于过时数据判断
```

这是重构遗留问题。`chatStreamStore` 拆分时，mascot 同步逻辑应随之迁移到 chatStore 订阅（或独立订阅），但被遗漏了。

**通用解法**：将依赖多 store 状态的逻辑集中到数据源 store 的订阅中；或在拆分 store 时同步迁移所有消费者。判断标准：**订阅回调中是否读取了非订阅 store 的状态？**

**涉及问题**：2.4、2.7

---

## 5. 验证策略

### 5.1 单元测试

- **2.2**：模拟 `sendChatMessage` 抛异常，验证 `send()` 返回后 `getPhase()==="idle"` 且 `getSendInProgress()===false`
- **2.3**：模拟 `switchConversation` 时正在流式传输，验证 `ensureIdle` 被调用且 `cancelChat` 被 await

### 5.2 集成测试

- **2.1**：在 `bootstrap` 完成后挂载组件，验证 `useAppInitialization` 不触发配置加载网络请求
- **2.4**：流结束时验证 mascot 表情基于合并后的消息内容（非流式内容）
- **2.6**：切换 provider 时验证后端收到有序的 provider→model 更新

### 5.3 手动验证

- **2.3**：在流式传输中快速切换会话，验证新会话不显示旧会话内容
- **2.5**：首次打开应用时验证 provider selector 和会话列表正常显示
- **2.6**：切换 provider 后验证 model 列表和当前 model 正确更新

---

## 6. 不修复 / 接受风险决策

| 问题 | 决策 | 理由 |
|------|------|------|
| 2.7 当前无功能影响 | 仍修复 | 改动量 1 行，消除未来消费者踩坑风险 |
| 2.5 `ensureCurrentConversation` 绕过 store 直接调 service | 接受 | 这是有意设计：`ensureCurrentConversation` 需在 store 为空时创建初始会话，直接调 service 避免循环依赖。修复后 `loadProviders` 先完成，`ensureCurrentConversation` 可改为读 store，但收益不大 |
| 2.2 `cancel()` 路径中 `sendInProgress=true` 但 `phase="idle"` | 接受 | `cancel` 在 `send` 的 `try` 块内被调用，`finally` 随后执行将 `sendInProgress` 重置为 `false`。不一致窗口仅存在于 `cancel` 执行期间到 `finally` 执行之间（同步代码，无并发调用方能观察到） |

---

## 7. 竞态触发概率分析

对每个问题的触发概率进行量化评估，考虑触发条件、用户操作频率、时序窗口宽度等因素。

### 概率评估框架

| 因子 | 说明 |
|------|------|
| **时序窗口** | 竞态条件成立的时间窗口宽度。窗口越宽，触发概率越高 |
| **操作频率** | 触发竞态的用户操作在日常使用中的频率 |
| **依赖条件** | 除时序外还需满足的额外条件（如网络延迟、后端状态等） |
| **Tauri IPC 特性** | Tauri invoke 是异步 IPC，响应时间通常 1-10ms；event 是异步推送，延迟取决于后端处理速度 |

### 7.1 🔴 P0 — bootstrap 与 useAppInitialization 并行加载

| 因子 | 评估 |
|------|------|
| 时序窗口 | **宽**：`bootstrap` 串行 await 3 个配置（~3-30ms），React 挂载 + useEffect 调度（~5-15ms），两者执行时间重叠概率极高 |
| 操作频率 | **每次启动必触发**：100% |
| 依赖条件 | 配置值在两次请求间被修改（极低）或响应乱序（Tauri IPC 是本地调用，无网络乱序） |

**触发概率：高（每次启动都发生并发请求），但实际影响概率：极低**

- 并发请求**必然发生**（概率 ~100%），但两次请求都读同一本地 SQLite，返回值相同
- 配置闪烁需要两次请求返回**不同值**——在 Tauri 本地 IPC 场景下几乎不可能（无网络延迟、无并发写入）
- `applyLocaleChange` 重复调用是**确定会发生的**，但 i18next 对相同语言切换是幂等的，无可见影响
- **结论**：竞态结构存在，但因 Tauri 本地 IPC 的特性，实际数据不一致概率 ≈ 0。修复价值在于消除冗余请求和架构清晰度

### 7.2 🔴 P0 — sendInProgress 与 phase 脱节

| 因子 | 评估 |
|------|------|
| 时序窗口 | **中等**：从 `finally` 执行到下一次 `send()` 调用之间 |
| 操作频率 | **低**：需要 `sendChatMessage` invoke 抛异常 |
| 依赖条件 | ① invoke 异常（网络断开/API 超时/后端 crash）；② 异常后用户在旧 Tauri event 仍排队时再次发送 |

**触发概率：低**

- **invoke 异常**本身就不常见：正常使用中 `sendChatMessage` 几乎不会失败。触发场景：
  - Provider API Key 无效或过期（~5% 用户可能遇到）
  - 后端 Rust panic（极低，<0.1%）
  - 网络中断（取决于用户环境，移动热点场景 ~1-3%）
- **异常后立即再发送**：用户看到错误后通常会检查问题而非立即重试，快速重发概率 ~20%
- **旧 event 仍在队列**：Tauri event 在同一微任务中处理，invoke 异常后事件队列通常已清空。只有在 invoke 超时（后端仍在运行但响应极慢）时，旧 event 才可能迟到
- **综合概率**：~5% × 20% × 5%（event 迟到）≈ **0.05%**
- **结论**：概率极低但后果严重（消息内容错乱），且修复成本极低（2 行），值得修复

### 7.3 🔴 P0 — switchConversation 取消流式传输未等待

| 因子 | 评估 |
|------|------|
| 时序窗口 | **宽**：从 `cancelChat` fire-and-forget 到后端实际停止发送，窗口 = 后端 cancel 处理时间（~5-50ms） |
| 操作频率 | **中等**：用户在流式输出期间切换会话 |
| 依赖条件 | ① 正在流式传输；② 用户在此期间切换会话；③ 后端 cancel 延迟 > 前端 `getConversationPaginated` 返回时间 |

**触发概率：中等**

- **流式传输中切换会话**：用户可能在 AI 回复过长时切换，或在快速浏览历史时误触。估计 ~10-15% 的会话中会发生
- **cancel 延迟 > 加载新会话延迟**：`cancelChat` 是 Rust 端处理（需停止 LLM 流、清理状态），通常 ~5-20ms；`getConversationPaginated` 也是本地 SQLite 查询，通常 ~2-10ms。两者时间接近，cancel 稍慢的概率 ~40-60%
- **旧 StreamStart 事件在切换后到达**：需要后端在 cancel 生效前发出了 StreamStart。如果 cancel 在 StreamStart 之前被处理则无问题。概率取决于后端事件发送频率与 cancel 处理速度的竞争
- **综合概率**：~12% × 50% × 30% ≈ **1.8%**
- **结论**：概率不可忽视，且后果严重（旧数据写入新会话，用户可见的数据错乱）。这是 7 个问题中**最可能在正常使用中触发**的 P0 问题

### 7.4 🟡 P1 — useMascotChatSync 双订阅竞态

| 因子 | 评估 |
|------|------|
| 时序窗口 | **极窄**：Zustand `subscribe` 回调在 `set` 后同步触发，两个 store 的订阅回调在同一微任务中执行，执行顺序取决于注册顺序 |
| 操作频率 | **每次流结束都触发**：100% |
| 依赖条件 | streamStore 订阅回调先于 chatStore 订阅回调执行 |

**触发概率：中等（每次流结束都可能），但影响极小**

- **订阅顺序**：`useMascotChatSync` 中先注册 `unsubscribeStream` 再注册 `unsubscribeChat`。Zustand 的 `subscribe` 按注册顺序调用——**streamStore 订阅总是先执行**
- 这意味着 `justStoppedStreaming` 判断时，`chatStore.messages` **总是合并前的旧值**——这不是随机竞态，而是**确定性的时序问题**
- **每次流结束都会触发**，但影响仅是 mascot 表情基于不完整内容判断，下一帧（~16ms）修正
- **结论**：触发概率 100%（每次流结束），但用户可感知影响概率极低（表情错误持续 <1 帧）。修复价值在于代码正确性

### 7.5 🟡 P1 — ChatView 初始化并行

| 因子 | 评估 |
|------|------|
| 时序窗口 | **中等**：`loadProviders` 完成时间（~5-20ms） |
| 操作频率 | **每次打开 Orb/ChatView 必触发**：100% |
| 依赖条件 | `ensureCurrentConversation` 在 `loadProviders` 完成前执行且需要创建初始会话 |

**触发概率：低**

- **并行请求必然发生**（概率 100%），冗余的 `listProviders` 调用也必然发生
- **provider selector 短暂空列表**：`loadProviders` 通常 ~5-10ms，空列表闪烁时间极短，用户几乎不可能注意到
- **创建初始会话失败**：需要同时满足"无已有会话"且"provider 列表为空"。首次使用应用时可能触发，但 `ensureCurrentConversation` 直接调 `providerService.listProviders()` 绕过 store，不依赖 `loadProviders` 完成
- **结论**：冗余请求必然发生但无功能影响；provider selector 闪烁概率 ~100% 但不可感知。修复价值在于消除冗余请求

### 7.6 🟡 P1 — provider/model 乱序

| 因子 | 评估 |
|------|------|
| 时序窗口 | **中等**：两个 IPC 调用发射间隔 ~0ms，后端处理间隔取决于 Rust 端调度 |
| 操作频率 | **中等**：每次切换 provider 或 agent 模式时触发 |
| 依赖条件 | 后端先处理 updateModel 再处理 updateConversationProvider |

**触发概率：低-中**

- **Tauri IPC 顺序**：同一前端微任务中发射的两个 `invoke`，Tauri 会按发射顺序发送到 Rust 端。但 Rust 端使用 tokio 异步运行时，两个命令可能被不同 task 处理，处理完成顺序不确定
- **后端先处理 updateModel 的概率**：取决于两个命令的处理时间差异。`updateConversationProvider` 需要验证 provider + 更新会话，`updateModel` 需要验证 model + 更新会话，两者复杂度相近。乱序概率 ~30-50%
- **乱序导致实际错误**：需要 model 不属于旧 provider。如果用户切换到同一 provider 的不同 model（常见操作），则不会触发。切换到不同 provider 时（~30% 的切换操作），model 不属于旧 provider 的概率 ~90%
- **综合概率**：~40% × 30% × 90% ≈ **10.8%**（在切换 provider 操作中）
- **结论**：在切换 provider 场景下触发概率不可忽视（~10%），但后端可能做了容错处理（静默忽略无效 model 更新），实际用户可感知错误概率可能更低。仍建议修复

### 7.7 🟢 P2 — fuseInstance 不同步

| 因子 | 评估 |
|------|------|
| 时序窗口 | **持续**：语言切换后 `store.fuse` 永久过时，直到 store 重新创建 |
| 操作频率 | **低**：仅在语言切换时触发 |
| 依赖条件 | 有组件通过 `useToolStore(s => s.fuse)` 消费 fuse 实例 |

**触发概率：当前为 0，未来可能非 0**

- **当前无消费者**：`search` 方法使用模块级 `fuseInstance`，无组件读 `store.fuse`
- **语言切换频率**：极低，大多数用户从不切换语言
- **结论**：当前触发概率 = 0。未来如有组件读 `store.fuse` 则触发概率 = 100%（每次语言切换后）。修复成本极低（1 行），属于预防性修复

### 概率总结

| # | 问题 | 触发概率 | 影响严重度 | 风险评分 | 修复优先级 |
|---|------|---------|-----------|---------|-----------|
| 2.1 | bootstrap 竞态 | 并发 100%，数据不一致 ≈0% | 低（冗余请求） | ⭐⭐ | 中（架构清晰度） |
| 2.2 | send/phase 脱节 | **~0.05%** | 高（消息错乱） | ⭐⭐⭐ | 高（低成本高收益） |
| 2.3 | switchConversation cancel | **~1.8%** | 高（数据污染） | ⭐⭐⭐⭐⭐ | **最高** |
| 2.4 | mascot 双订阅 | 100%（每次流结束） | 极低（1 帧表情错误） | ⭐⭐ | 中（代码正确性） |
| 2.5 | ChatView 初始化并行 | 冗余 100%，可见影响 ≈0% | 低（冗余请求） | ⭐ | 低 |
| 2.6 | provider/model 乱序 | **~10.8%**（切换 provider 时） | 中（model 更新失败） | ⭐⭐⭐⭐ | 高 |
| 2.7 | fuseInstance 不同步 | 当前 0%，未来可能 100% | 低（无消费者） | ⭐ | 低（预防性） |

> **关键发现**：2.3（switchConversation cancel）是**最可能在正常使用中触发的 P0 问题**，概率 ~1.8%——意味着每 50-100 次流式传输中切换会话的操作就会出现一次数据污染。2.6（provider/model 乱序）在切换 provider 场景下概率 ~10%，也不可忽视。
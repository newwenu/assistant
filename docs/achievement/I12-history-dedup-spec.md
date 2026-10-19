# I12 — HistoryManager 与 chatStore 代码优化 规格说明

## 1. 交付目标

优化 `HistoryManager.tsx` 与 `chatStore.ts` 之间的代码重复和状态同步问题，分四层递进：

1. **提取独立模块**：类型定义、PersistQueue 工具、纯计算函数从大文件中拆出，降低阅读负担，提升复用性
2. **目录归位**：HistoryManager 是主窗口路由页面（与 ToolBox/ToolPage/SettingsContent 平级），从 `chat/` 迁移到独立的 `history/` 目录，与 `chat/`（Orb 对话域）分离
3. **conversations 单源共享**：会话列表收归 chatStore 单源，HistoryManager 读 chatStore 而非自行 invoke，消除双源获取
4. **PersistQueue 补齐 + 单向同步**：HistoryManager 的写操作补上 PersistQueue 串行化，操作完成后刷新 chatStore 数据源

> **前置依赖**：I2 ✅（聊天消息历史管理系统已完成）
> **性质**：代码健康度优化，不新增功能，不改变外部行为
> **核心约束**：HistoryManager 的 `selectedId` / `messages` / 导出 等独立语义不合并到 chatStore，避免行为变更

## 2. 问题诊断

### 2.1 核心问题：双源获取 + PersistQueue 缺失

`HistoryManager.tsx`（1035行）自行维护了完整的会话/消息状态，与 `chatStore.ts`（839行）形成双源：

| 操作 | chatStore | HistoryManager | 差异 |
|------|-----------|----------------|------|
| 加载会话列表 | `loadConversations` → invoke → set | `loadConversations` → invoke → set | **完全重复** |
| 删除会话 | PersistQueue → invoke → load | invoke → load | **未走 PersistQueue** |
| 重命名 | PersistQueue → invoke → load | invoke → load | **未走 PersistQueue** |
| 置顶 | PersistQueue → invoke → load | invoke → load | **未走 PersistQueue** |
| 复制 | PersistQueue → invoke → load | invoke → load | **未走 PersistQueue** |
| 删除消息 | PersistQueue → invoke | invoke → 本地 filter | **未走 PersistQueue**，不刷新会话列表 |
| 导出 | invoke → Blob（文件名 `conversation.ext`） | invoke → Blob（文件名 `conversation-id.ext`） | 文件名/MIME/通知 不同 |
| 加载消息 | `switchConversation` → invoke paginated | `selectConversation` → invoke paginated | **语义不同**：Orb 活跃对话 vs 历史查看 |
| 加载更多 | `loadMoreMessages` → 分页 invoke | `loadMoreMessages` → 分页 invoke | **服务不同面板** |
| 搜索 | `searchConversations` → invoke/filter | `handleSearch` → invoke/filter | 逻辑相同，debounce 位置不同 |

### 2.2 目录错位：HistoryManager 放在 `chat/` 下

`chat/` 目录的组件按消费者分属两类不同窗口：

| 模块 | 消费者 | 所属窗口 |
|------|--------|---------|
| ChatView / message / blocks / ModeSelector / ModelSelector / ProviderSelector | OrbPanel | **Orb 窗口** |
| HistoryManager | App.tsx (`/history` 路由) | **主窗口** |
| chatErrors / chatTypes | 两者 | 共享 |

HistoryManager 是主窗口路由页面，与 `tools/ToolBox`、`settings/SettingsContent` 平级，但放在了 `chat/` 下——而 `chat/` 其他组件全部服务 Orb 窗口。这导致：
- 开发者误以为 HistoryManager 是 Orb 对话的一部分，可能做出错误的依赖决策
- 目录语义不清，`chat/` 混装了两个不同窗口域的组件

**解决方案**：新建 `src/history/` 目录，将 HistoryManager / ConversationListItem / conversationGrouping 移入。`chatTypes.ts` 留在 `chat/`（类型语义根源是聊天域，跨域共享通过 `import type` 解决）。

### 2.3 实际后果

1. **PersistQueue 缺失**：HistoryManager 写操作绕过串行化队列，快速连续操作（连点删除、连点置顶）可能竞态，导致数据丢失或状态不一致
2. **conversations 双源**：HistoryManager 删除会话后 chatStore 不感知；chatStore 操作后 HistoryManager 也不感知
3. **维护成本**：会话列表加载逻辑写两遍，变更需同步两处
4. **类型耦合**：`Message`/`Conversation` 等类型定义在 `chatStore.ts` 中，外部 `from "../../stores/chatStore"` 导入类型暗示了对 store 实现的依赖

### 2.4 语义差异确认（不可合并）

HistoryManager 与 chatStore 存在**有意的语义差异**，不可强行合并：

| 状态 | chatStore 语义 | HistoryManager 语义 | 合并风险 |
|------|---------------|-------------------|---------|
| `currentConversationId` | Orb 面板**正在聊天**的会话 | — | — |
| `selectedId` | — | 历史面板**正在查看**的会话 | 合并后：历史面板点击查看 → Orb 活跃对话被切换 |
| `messages` | Orb 面板对话消息（含流式） | 历史面板查看的消息（静态） | 合并后：历史面板加载消息 → Orb 流式状态被重置 |
| `exportConversation` | 静默，文件名 `conversation.ext` | 带通知，文件名 `conversation-id.ext` | 合并后：需改 chatStore 行为或丢失差异 |

### 2.5 非问题确认

- **chatStore.ts 839 行**：中心 store，action 间强依赖（sendMessage→loadConversations→switchConversation），拆 slice 引入跨 slice 协调，得不偿失
- **AgentModeSettings.tsx / ConnectDialog.tsx / SettingField.tsx / App.tsx**：各组件行数可接受，拆出破坏行为局部性

## 3. 改动范围

### 3.1 改动清单

| # | 改动 | 类型 | 风险 | 收益 |
|---|------|------|------|------|
| 1 | 提取类型定义到 `chat/chatTypes.ts` | 纯移动 | 极低 | 消除类型对 store 的耦合 |
| 2 | 提取 PersistQueue 到 `utils/persistQueue.ts` | 纯移动 | 极低 | 通用工具可复用 |
| 3 | 提取 conversationGrouping 纯函数到 `history/conversationGrouping.ts` | 纯移动 | 极低 | 可独立测试和复用 |
| 4 | 提取 ConversationListItem 子组件到 `history/ConversationListItem.tsx` | 结构调整 | 低 | HistoryManager 更聚焦 |
| 5 | 目录迁移：`chat/` → `history/` | 纯移动 | 极低 | 目录语义清晰，与 Orb 域分离 |
| 6 | chatStore 新增 `isLoadingConversations` | 小幅增强 | 极低 | 支持共享加载状态 |
| 7 | HistoryManager conversations 单源 + PersistQueue 补齐 | 逻辑重构 | 中 | 消除双源 + 修复竞态 |

### 3.2 不做的事

| 方案 | 不做原因 |
|------|---------|
| HistoryManager 全量委托 chatStore | selectedId/messages/导出语义不同，合并会改变外部行为 |
| 拆 chatStore 为 Zustand slice | action 间强依赖，slice 边界难划定，项目无 slice 先例 |
| 合并 HistoryManager 的 selectedId 到 chatStore.currentConversationId | 历史面板查看 ≠ Orb 活跃对话，合并后点击历史会切换 Orb 对话 |
| 统一导出实现到 chatStore | 文件名/通知差异是有意设计，统一需改 chatStore 行为 |

## 4. 各改动规格

### 4.1 改动 1：提取类型定义到 `chat/chatTypes.ts`

**动机**：`Message`、`Conversation`、`ToolCall`、`TokenUsage`、`ContextUsage` 被以下文件导入，但只需类型：

| 文件 | 导入的类型 |
|------|-----------|
| `chat/HistoryManager.tsx` | `Conversation`, `Message` |
| `chat/message/MessageList.tsx` | `Message` |
| `chat/message/MessageBubble.tsx` | `Message`, `ToolCall` |
| `chat/blocks/ToolCallBlock.tsx` | `ToolCall` |
| `services/chatDeltaEvents.ts` | `TokenUsage` |

导入路径 `from "../../stores/chatStore"` 暗示对 store 实现的依赖，实际只需类型。参照 `mascot/core/types.ts` 模式。

**操作**：

- 新建 `src/chat/chatTypes.ts`，移入 5 个 interface/type
- `chatStore.ts`：`import type { ... } from '../chat/chatTypes'`，并 `export type { ... } from '../chat/chatTypes'` 保持外部导入兼容
- 各消费文件可逐步改为从 `chat/chatTypes` 直接导入（可选，chatStore re-export 已保证兼容）

**验证**：`tsc --noEmit` 通过

### 4.2 改动 2：提取 PersistQueue 到 `utils/persistQueue.ts`

**动机**：PersistQueue 是通用异步串行化队列，与聊天业务无关。放在 chatStore 里限制复用，且与 store 状态逻辑混在一起增加阅读负担。

**操作**：

- 新建 `src/utils/persistQueue.ts`，将 `PersistQueue` class + `PersistEntry` interface 原样搬出
- `chatStore.ts`：`import { PersistQueue } from '../utils/persistQueue'`
- chatStore 内部 `const persistQueue = new PersistQueue()` 不变

**验证**：`tsc --noEmit` 通过

### 4.3 改动 3：提取 conversationGrouping 纯函数到 `history/conversationGrouping.ts`

**动机**：`groupConversations` 函数和 `GROUP_LABELS` 常量是纯计算逻辑，无 React 依赖。提取后可独立单元测试，且 HistoryManager 更聚焦。

**操作**：

- 新建 `src/history/conversationGrouping.ts`
- 移出 `ConversationGroup` 类型、`GROUP_LABELS` 常量、`groupConversations` 函数
- `groupConversations` 签名改为接收 `translateFn: (key: string) => string` 参数，不再内部依赖 `translateKey`，使其成为纯函数
- HistoryManager 调用时传入 `translateKey` 作为翻译函数

**验证**：分组行为不变

### 4.4 改动 4：提取 ConversationListItem 子组件到 `history/ConversationListItem.tsx`

**动机**：`ConversationListItem`（约 120 行）定义在 HistoryManager 底部，包含右键菜单和内联重命名。提取后 HistoryManager 主组件更聚焦于面板布局和数据编排。

**操作**：

- 新建 `src/history/ConversationListItem.tsx`
- Props：`conversation` / `isSelected` / `onSelect` / `onDelete` / `onRename` / `onTogglePin` / `onDuplicate` / `onExport`
- 所有回调由 HistoryManager 传入

**验证**：视觉和交互行为不变

### 4.5 改动 5：目录迁移 `chat/` → `history/`

**动机**：HistoryManager 是主窗口 `/history` 路由页面，与 `tools/ToolBox`、`settings/SettingsContent` 平级。放在 `chat/` 下导致目录语义混乱——`chat/` 其他组件全部服务 Orb 窗口。

**操作**：

- 新建 `src/history/` 目录
- 将 `HistoryManager.tsx`、`ConversationListItem.tsx`、`conversationGrouping.ts` 移入 `src/history/`
- 删除 `src/chat/` 下的旧文件
- `App.tsx` 导入路径：`@/chat/HistoryManager` → `@/history/HistoryManager`
- `history/` 内文件导入 `chatTypes` 路径：`./chatTypes` → `../chat/chatTypes`
- `chatTypes.ts` 留在 `src/chat/`（类型语义根源是聊天域，跨域共享通过 `import type` 解决）

**验证**：`tsc --noEmit` 通过

### 4.6 改动 6：chatStore 新增 `isLoadingConversations`

**动机**：HistoryManager 当前有本地 `loadingConvList` 状态。conversations 收归 chatStore 单源后，HistoryManager 需从 chatStore 读取加载状态。

**操作**：

- `ChatState` 新增 `isLoadingConversations: boolean`，初始 `false`
- `loadConversations` 开始时 `set({ isLoadingConversations: true })`，finally 中 `set({ isLoadingConversations: false })`

**验证**：`tsc --noEmit` 通过

### 4.7 改动 7：HistoryManager conversations 单源 + PersistQueue 补齐 — **核心改动**

**动机**：消除 conversations 列表的双源获取，补齐写操作的 PersistQueue 串行化，写操作后单向同步刷新 chatStore 数据源。

**设计原则**：

1. **conversations 单源**：HistoryManager 不再自行 invoke 加载会话列表，改为 `useChatStore(s => s.conversations)` 读取
2. **PersistQueue 补齐**：HistoryManager 写操作通过独立 PersistQueue 实例串行化
3. **单向同步**：写操作完成后调用 `chatStore.loadConversations()` 刷新共享数据源，确保 Orb 面板等消费者感知变更
4. **独立语义保留**：selectedId / messages / 导出 保持本地实现，不合并到 chatStore
5. **不改变外部行为**

#### 数据源改造

删除的本地状态（改为从 chatStore 读取）：

| 删除 | 替换为 |
|------|--------|
| `conversations` + `setConversations` | `useChatStore(s => s.conversations)` |
| `loadConversations()` | `chatStore.loadConversations()` |
| `refreshConversations()` | `chatStore.loadConversations()` |
| `loadingConvList` | `useChatStore(s => s.isLoadingConversations)` |

保留的本地状态（语义独立，不可合并）：

| 状态 | 原因 |
|------|------|
| `selectedId` | 历史面板查看的会话 ID，≠ chatStore.currentConversationId（Orb 活跃对话） |
| `messages` + `setMessages` | 历史面板查看的消息列表，≠ chatStore.messages（Orb 对话消息，含流式） |
| `totalCount` / `hasMore` / `loadingMore` | 消息分页状态，服务于历史面板 |
| `loadingMessages` / `refreshing` | UI 加载状态 |
| `searchQuery` / `searchResults` / `searchFullText` | 搜索 UI 状态 |
| `collapsedGroups` / `userScrolledUp` | 交互 UI 状态 |

#### PersistQueue 补齐改造

HistoryManager 新增独立的 PersistQueue 实例（从 `utils/persistQueue.ts` 导入）：

```typescript
import { PersistQueue } from "../utils/persistQueue";
const historyPersistQueue = new PersistQueue();
```

写操作改造（invoke 包入 PersistQueue，完成后刷新 chatStore）：

| 操作 | 改造前 | 改造后 |
|------|--------|--------|
| `deleteConversation` | `await invoke(...)` → `await loadConversations()` | `await historyPersistQueue.enqueue(() => invoke(...))` → `await chatStore.loadConversations()` |
| `renameConversation` | `await invoke(...)` → `await loadConversations()` | `await historyPersistQueue.enqueue(...)` → `await chatStore.loadConversations()` |
| `togglePin` | `await invoke(...)` → `await loadConversations()` | `await historyPersistQueue.enqueue(...)` → `await chatStore.loadConversations()` |
| `duplicateConversation` | `await invoke(...)` → `await loadConversations()` | `await historyPersistQueue.enqueue(...)` → `await chatStore.loadConversations()` |
| `deleteMessage` | `await invoke(...)` → 本地 filter | `await historyPersistQueue.enqueue(...)` → 本地 filter + `await chatStore.loadConversations()` |

用户通知（`message.success` / `message.error`）保留在 HistoryManager 本地——通知是 UI 层关注点。

#### 保持独立实现的操作

以下操作因语义差异，保持 HistoryManager 本地实现：

- **`selectConversation(id)`**：加载历史消息供查看，不改变 chatStore 的 `currentConversationId`。委托 `chatStore.switchConversation(id)` 会导致 Orb 面板对话被意外切换
- **`loadMoreMessages()`**：历史面板消息分页，独立于 chatStore 的 `loadMoreMessages`（后者服务 Orb 面板）
- **`exportConversation(id, format)`**：文件名带 ID（`conversation-${id}.${ext}`），MIME 带 `;charset=utf-8`，带用户通知——与 chatStore 的导出实现有意的差异
- **搜索逻辑**：debounce 是 UI 交互关注点，保留本地。标题搜索本地过滤 `chatStore.conversations`，全文搜索 `invoke("search_conversations")`
- **`refreshMessages()`**：仅刷新历史面板当前查看的消息，不干扰 Orb 流式输出
- **定时刷新**：保留 15 秒 useEffect，conversations 刷新改为 `chatStore.loadConversations()`，messages 刷新保留本地 `refreshMessages()`

#### 预估效果

- **行数**：从 1035 行降至约 800 行（conversations 相关 state/load/refresh 删除约 30 行，PersistQueue 包装增加约 25 行，净减约 5 行；加上改动 3/4 提取减约 210 行）
- **PersistQueue**：写操作全部串行化，消除竞态风险
- **conversations 一致性**：单源（chatStore），写操作后单向同步刷新
- **行为不变**：selectedId / messages / 搜索 / 导出 均保持独立语义

## 5. 改动后文件结构

```
src/
  chat/                            # Orb 对话域（仅 Orb 窗口消费）
    chatTypes.ts                    # 共享类型：Message, Conversation, ToolCall, TokenUsage, ContextUsage
    ChatView.tsx                    # 不变
    message/                        # 不变
    blocks/                         # 不变
    ModeSelector.tsx                # 不变
    ModelSelector.tsx               # 不变
    ProviderSelector.tsx            # 不变
    chatErrors.ts                   # 不变
    streamCoalescer.ts              # 不变
    useTypewriter.ts                # 不变
  history/                          # 主窗口历史页面（仅主窗口消费）
    HistoryManager.tsx              # 精简：~800行，conversations 读 chatStore + 本地 PersistQueue + 独立语义
    ConversationListItem.tsx        # 单条会话项组件
    conversationGrouping.ts         # groupConversations + GROUP_LABELS 纯函数
  utils/
    persistQueue.ts                 # PersistQueue 通用串行化队列
  stores/
    chatStore.ts                    # 精简：类型和 PersistQueue 移出，新增 isLoadingConversations
    ...
```

## 6. 实施顺序与验证

| 步骤 | 改动 | 风险 | 验证方式 |
|------|------|------|---------|
| 1 | 提取 chatTypes.ts | 极低 | `tsc --noEmit` |
| 2 | 提取 persistQueue.ts | 极低 | `tsc --noEmit` |
| 3 | 提取 conversationGrouping.ts | 极低 | `tsc --noEmit` |
| 4 | 提取 ConversationListItem.tsx | 低 | 手动测试：会话列表渲染 + 右键菜单 + 内联重命名 |
| 5 | 目录迁移 `chat/` → `history/` | 极低 | `tsc --noEmit` |
| 6 | chatStore 新增 isLoadingConversations | 极低 | `tsc --noEmit` |
| 7 | HistoryManager conversations 单源 + PersistQueue 补齐 | 中 | 手动测试：见 §7 |
| 8 | lint + typecheck | — | `npm run lint && npm run typecheck` |

步骤 1-3 为纯移动，可并行完成。步骤 4 为结构调整。步骤 5 为目录迁移。步骤 6 为 chatStore 小幅增强。步骤 7 为核心改动，需仔细测试。

## 7. 验收标准

### 7.1 编译

| # | 标准 |
|---|------|
| 1 | `npm run typecheck` 通过 |
| 2 | `npm run lint` 通过 |

### 7.2 功能（步骤 6 后手动测试）

| # | 标准 | 验证方法 |
|---|------|---------|
| 3 | 会话列表正常加载和显示 | 打开历史面板 |
| 4 | 时间分组正确（置顶/今天/昨天/近7天/近30天/更早） | 查看分组标题 |
| 5 | 点击会话查看消息，不影响 Orb 活跃对话 | 点击历史会话，检查 Orb 面板对话未变 |
| 6 | 向上滚动加载更多消息 | 长对话（>50条消息） |
| 7 | 删除会话：列表更新 + chatStore 同步更新 | 删除后切换到 Orb 面板验证 |
| 8 | 重命名会话：标题更新 + title_source='user' | 右键重命名 |
| 9 | 置顶/取消置顶：排序变化 + 列表刷新 | 右键置顶 |
| 10 | 复制会话：新会话出现 | 右键复制 |
| 11 | 导出会话：文件名带 ID，下载成功 | 右键导出 JSON/Markdown |
| 12 | 删除消息：消息消失 + 会话列表刷新 | 消息操作栏删除 |
| 13 | 标题搜索：输入即过滤 | 搜索框输入 |
| 14 | 全文搜索：切换后搜索消息内容 | 切换为全文搜索 |
| 15 | PersistQueue 生效：快速连续操作无数据丢失 | 连续快速点击删除/置顶 |
| 16 | 定时刷新：停留 15 秒后列表自动刷新 | 等待观察 |
| 17 | 操作通知：删除/重命名/复制/导出/删除消息均有 message 提示 | 执行各操作观察通知 |

### 7.3 代码健康度

| # | 标准 |
|---|------|
| 18 | `src/history/HistoryManager.tsx` 存在，`src/chat/HistoryManager.tsx` 不存在 |
| 19 | `src/history/ConversationListItem.tsx` 存在，`src/chat/ConversationListItem.tsx` 不存在 |
| 20 | `src/history/conversationGrouping.ts` 存在，`src/chat/conversationGrouping.ts` 不存在 |
| 21 | HistoryManager.tsx ≤ 850 行 |
| 22 | HistoryManager 中无 conversations 相关的本地 state 和 load/refresh 函数 |
| 23 | HistoryManager 写操作均走 PersistQueue（deleteConversation / renameConversation / togglePin / duplicateConversation / deleteMessage） |
| 24 | HistoryManager 写操作完成后均调用 `chatStore.loadConversations()` 单向同步 |
| 25 | chatTypes.ts 存在于 `src/chat/` 且包含 Message / Conversation / ToolCall / TokenUsage / ContextUsage |
| 26 | persistQueue.ts 存在于 `src/utils/` 且 PersistQueue class 可独立导入 |
| 27 | chatStore 包含 `isLoadingConversations: boolean` 状态 |
| 28 | App.tsx 导入路径为 `@/history/HistoryManager` |
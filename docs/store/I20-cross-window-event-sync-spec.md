# I20 — 跨窗口事件同步改进方案

## 核心主线

**从"轮询+双写"到"事件驱动"的数据流改造。**

```
改造前                                    改造后
─────────────────────────────────        ─────────────────────────────────
操作方 invoke → 后端 → DB               操作方 invoke → 后端 → DB
                ↓                                    ↓
         (可能 emit 给 Orb)                    app.emit 广播
                ↓                                    ↓
操作方手动 loadConversations ← 冗余       事件监听器 → loadConversations
                                         （操作方自己也被事件刷新，删掉手动调用）

主窗口 ← 15s 轮询 → 始终滞后             主窗口 ← 事件驱动（60s 兜底轮询）
```

三步走：

| 步骤 | 做什么 | 为什么 |
|------|--------|--------|
| **① 打通管道** | 后端所有数据变更操作都 `app.emit` 广播 | 当前元数据操作不发事件、部分事件用 `orb_window.emit` 主窗口收不到 |
| **② 接通主窗口** | 新增 `historyStore` + `subscribeHistoryEvents()` 驱动主窗口数据刷新 | 当前主窗口零监听，只靠 15s 轮询；且主窗口状态管理方式与 Orb 有本质差异，不能直接复用 chatStore |
| **③ 清理双写** | 操作方删掉手动 `loadConversations()` | 加了事件监听后，操作方 invoke → 后端 emit → 自己的监听器也会刷新，手动调用变成冗余 |

---

## 0. 参考项目验证

"业务事件一律 `app.emit` 广播"是 Tauri 2 多窗口应用的行业共识：

| 项目 | `app.emit` 占比 | `window.emit` 用途 |
|------|----------------|-------------------|
| OpenBitFun | 93%+ (30/32) | 仅窗口关闭拦截 |
| openhuman | 96%+ (25/26) | 仅 PTT 覆盖层 UI 控制 |
| ChatGPT | 0% (1/1) | 仅 titlebar 导航 |

`window.emit` 仅用于两种场景：**窗口自身 UI 控制**（关闭拦截、焦点控制）和**窗口独占高频流**（PTT 覆盖层激活信号）。

本项目当前 `orb_window.emit` 用于 `chat-messages-changed`、`create_branch` 等业务事件，属于反模式。

---

## 1. 问题诊断

### 1.1 主窗口零监听（P0）

`HistoryManager` 仅靠 15s 轮询，`src/history/` 目录没有任何 `listen()` 调用。Orb 的变更最多延迟 15 秒才能在主窗口体现。

### 1.2 元数据操作不发事件（P0）

9 个修改对话列表结构的后端命令，执行后不 emit 任何事件：

| 操作 | Rust 函数 |
|------|-----------|
| 删除对话 | `delete_conversation` |
| 重命名 | `rename_conversation` |
| 置顶/取消 | `toggle_conversation_pin` |
| 复制对话 | `duplicate_conversation` |
| 切换模型 | `update_conversation_model` |
| 切换 Provider | `update_conversation_provider` |
| 批量删除 | `batch_delete_conversations` |
| 全部清空 | `purge_conversations` |
| 新建对话 | `create_conversation` |

### 1.3 emit 目标不一致（P1）

同一个 `chat-messages-changed` 事件，5 处 emit 中仅 1 处用 `app.emit`（广播，`message_deleted`）、4 处用 `orb_window.emit`（定向，`branch_created`/`branch_switched`/`branch_forked`/`stream_finished`）。主窗口即使加了监听也收不到后 4 种。

### 1.3.1 `MessageChangeKind` 类型与实际 emit 脱节（P1）

TS 的 `MessageChangeKind` 枚举包含 `message_edited` / `messages_regenerated` 但后端从未 emit 这两个值（编辑和重新生成都走"创建分支+重发"路径，通知走 `stream_finished`）；反之，后端实际 emit 的 `stream_finished` 不在枚举中。详见 §7.3。

### 1.4 问题关联图

```
Orb 窗口操作 ──→ 后端 ──→ DB 写入
                      │
                      ├─ emit chat-messages-changed (orb_window.emit)  ──→ 仅 Orb 收到
                      ├─ emit chat-delta (orb_window.emit)             ──→ 仅 Orb 收到
                      └─ 元数据操作: 无 emit                            ──→ 无人收到

主窗口 ──→ 15s 轮询 list_conversations ──→ 始终滞后
       ──→ 无任何 listen() 调用
```

---

## 2. 设计目标

1. **实时性**：任何窗口的对话变更，另一窗口在 <1s 内感知到
2. **一致性**：同一类事件使用统一的 emit 方式
3. **最小改动**：复用现有事件基础设施，不引入新的消息总线
4. **向后兼容**：Orb 窗口现有监听逻辑不受影响

---

## 3. 事件职能分类

事件按职能分三类，决定 emit 方式：

| 职能 | emit 方式 | 判断依据 |
|------|----------|---------|
| **① 数据变更通知** | `app.emit` 广播 | "数据发生了什么变化"，关心该数据的窗口都应收到 |
| **② 窗口 UI 控制** | `window.emit` 定向 | "某个窗口应该做什么 UI 动作"，只有目标窗口响应 |
| **③ 高频独占流** | `window.emit` 定向 | 某窗口独占的渲染流，其他窗口不需要逐帧接收 |

本项目事件分类：

| 事件 | 职能 | emit 方式 | 理由 |
|------|------|----------|------|
| `chat-messages-changed` | ① 数据变更 | `app.emit` | 消息列表变更，两窗口都可能需要刷新 |
| `conversation-list-changed`（新增） | ① 数据变更 | `app.emit` | 对话列表变更，两窗口都需要感知 |
| `conversation_terminated` | ① 数据变更 | `app.emit` | 已是广播，无需改动 |
| `conversation_unlocked` | ① 数据变更 | `app.emit` | 已是广播，无需改动 |
| `conversation_compact_failed` | ① 数据变更 | `app.emit` | 已是广播，无需改动 |
| `chat-delta` | ③ 高频独占流 | `emit_orb_only` | 每秒数十次，主窗口不参与流式渲染 |
| `chat-stream-start` | ③ 高频独占流 | `emit_orb_only` | 流开始信号仅 Orb 需要 |
| `orb-blur` | ② UI 控制 | `emit_orb_only` | Orb 失焦收起；主窗口不需要监听 |
| `orb-new-conversation` | ② UI 控制 | `app.emit` | 已是广播；Orb 响应展开+新建，主窗口不需要响应 |
| `navigate` | ② UI 控制 | `app.emit` | 已是广播；主窗口做路由跳转，Orb 不需要 |

---

## 4. ① 打通管道 — 后端所有数据变更都 emit 广播

### 4.1 统一 `chat-messages-changed` 的 emit 方式

4 处 `orb_window.emit` 改为 `app.emit`：

| 文件 | 行号 | 当前 | 改为 |
|------|------|------|------|
| conversation.rs | 765 | `orb_window.emit`（`branch_created`） | `emit_broadcast(&app, ...)` |
| conversation.rs | 803 | `orb_window.emit`（`branch_switched`） | `emit_broadcast(&app, ...)` |
| conversation.rs | 866 | `orb_window.emit`（`branch_forked`） | `emit_broadcast(&app, ...)` |
| chat.rs | 208 | `orb_window.emit`（`stream_finished`） | `emit_broadcast(&app, ...)` |

改动细节：
- `create_branch` / `switch_branch` / `fork_to_new_conversation`：移除 `get_webview_window(WINDOW_ORB)` 调用（仅用于 emit，改为 `emit_broadcast` 后不再需要）
- `stream_finished`（chat.rs）：去掉 `finish_app.get_webview_window(WINDOW_ORB)` 包裹，直接 `emit_broadcast(&finish_app, ...)`

### 4.2 新增 `conversation-list-changed` 事件

元数据操作改变的是对话列表结构（而非某条对话内的消息），需要一种轻量事件通知"对话列表已变更"。

**Payload**：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationListChangedPayloadDto {
    pub change_kind: String,
    pub conversation_id: Option<String>,
    /// 批量删除时携带被删除的 ID 列表，供前端精确判断当前选中是否受影响
    pub affected_ids: Option<Vec<String>>,
}
```

**change_kind 枚举**：

| change_kind | 含义 | conversation_id | affected_ids |
|-------------|------|-----------------|--------------|
| `renamed` | 对话重命名 | Some(id) | None |
| `deleted` | 对话删除 | Some(id) | None |
| `pin_toggled` | 置顶状态变更 | Some(id) | None |
| `duplicated` | 对话复制 | Some(新对话id) | None |
| `model_updated` | 模型切换 | Some(id) | None |
| `provider_updated` | Provider 切换 | Some(id) | None |
| `batch_deleted` | 批量删除 | None | Some(被删ID列表) |
| `purged` | 全部清空 | None | None |
| `created` | 新建对话 | Some(id) | None |

> **设计决策**：`batch_deleted` 必须携带 `affected_ids`。如果只传 `None`，前端无法判断当前选中的对话是否在被批量删除的集合中——要么保守地无条件清空选中状态（误伤），要么不做清理（遗漏）。携带 ID 列表可精确匹配。

**后端改动**：在以下函数末尾添加 `emit_broadcast`：

| 函数 | change_kind | affected_ids |
|------|-------------|-------------|
| `delete_conversation` | `"deleted"` | None |
| `rename_conversation` | `"renamed"` | None |
| `toggle_conversation_pin` | `"pin_toggled"` | None |
| `duplicate_conversation` | `"duplicated"` | None |
| `update_conversation_model` | `"model_updated"` | None |
| `update_conversation_provider` | `"provider_updated"` | None |
| `batch_delete_conversations` | `"batch_deleted"` | Some(被删ID列表) |
| `purge_conversations` | `"purged"` | None |
| `create_conversation` | `"created"` | None |

**示例**：

```rust
#[command]
pub fn rename_conversation(
    app: AppHandle,
    id: String,
    title: String,
    title_source: Option<String>,
) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let source = title_source.unwrap_or_else(|| "user".to_string());
    conversations::rename_conversation(&conn, &id, &title, &source)?;

    emit_broadcast(
        &app,
        event_name::CONVERSATION_LIST_CHANGED,
        ConversationListChangedPayloadDto {
            change_kind: "renamed".to_string(),
            conversation_id: Some(id),
            affected_ids: None,
        },
    );

    Ok(())
}
```

**额外补充**：`fork_to_new_conversation` 当前仅 emit `chat-messages-changed`（源对话的消息变更），但 fork 创建了新对话，主窗口侧边栏不会出现新对话。需在该函数末尾额外 emit `conversation-list-changed`（`changeKind = "created"`，`conversationId = 新对话ID`）。

---

## 5. ② 接通主窗口 — historyStore + subscribeHistoryEvents

### 5.1 为什么不能直接复用 chatStore（关键分析）

初步直觉是让主窗口也用 `chatStore.currentConversationId` 和 `chatStore.messages`，与 Orb 共享同一个 store。但深入分析代码后发现**不可行**，原因有二：

#### 5.1.1 分支展示模式本质不同

| 维度 | Orb（ChatView） | 主窗口（HistoryManager） |
|------|----------------|------------------------|
| **消息加载策略** | 分页加载（`getConversationPaginated`），默认加载最新 N 条 | 全量加载（`getConversationAllMessages`），一次性拿完所有消息 |
| **分支可见性** | 只显示当前活跃分支（由后端 `activeBranchId` 决定） | 所有分支**同时平铺展示**（`buildBranchView()` 按深度分组，每个深度层级作为 BranchSlot 并列显示） |
| **分支切换方式** | 调后端 `switch_branch` 改变 `activeBranchId`，重新拉取消息 | 纯前端本地 state 切换（`localIndex`），不调后端，不改 `activeBranchId` |
| **交互能力** | 编辑/重试/重新生成/fork/发送消息 | 只读查看 + 删除消息 |

主窗口的 `buildBranchView()` 函数需要拿到**所有消息（含所有分支）**来构建平铺视图。而后端的 `getConversationPaginated` + `activeBranchId` 只返回当前活跃分支的消息。因此 `chatStore.messages` 对主窗口**不可用**——它永远只有活跃分支的数据。

#### 5.1.2 `ensureCurrentConversation` 对主窗口有害

`chatStore.loadConversations()` 内部会调用 `ensureCurrentConversation()`，检查 `currentConversationId` 是否有效。若为 `null` 则自动选中第一个对话甚至创建新对话。

主窗口的选中状态可以是 `null`（用户未选中任何对话或当前选中被删除后）。如果直接调 `chatStore.loadConversations()`，会触发自动选中行为——用户看到未点击却突然选中了一个对话，体验异常。

> 这两个约束使得"统一到 chatStore"的方案 A 不可行。必须为主窗口建立独立的状态管理。

### 5.2 方案选择：historyStore + subscribeHistoryEvents（沿用项目既有模式）

项目已有两套完全相同的事件驱动模式：

| 层 | Orb 窗口 | 设置页 |
|----|---------|-------|
| **Store** | `useChatStore` | `useProviderStore` / `useThemeStore` / `useSettingsStore` |
| **Events** | `subscribeChatDeltaEvents()` | `subscribeSettingEvents()` |
| **事件→Store** | `useChatStore.getState().xxx()` | `useXxxStore.getState().xxx()` / `useXxxStore.setState(...)` |

本项目共 8 个 Zustand store + 2 个 events 模块，全部遵循"store + subscribeEvents"模式。新方案延续这一模式：

```
┌─────────────────────────────────────────────────┐
│                   chatStore                      │
│  conversations ◄────── 两窗口共享 ──────► conversations │
│  currentConversationId  (Orb 专用)               │
│  messages / isStreaming   (Orb 专用)              │
└─────────────────────────────────────────────────┘
         ▲                                    ▲
         │ loadConversations                  │ loadConversations
         │ (Orb 通过 store 方法)              │ (主窗口通过 service 直接 setState)
         │                                    │
┌────────┴────────┐                  ┌────────┴────────┐
│  subscribeChat   │                  │ subscribeHistory │
│  DeltaEvents()   │                  │ Events()         │
│  (Orb 窗口)      │                  │ (主窗口)          │
└─────────────────┘                  └────────┬────────┘
                                              │
                                              ▼
                                    ┌─────────────────────┐
                                    │   useHistoryStore    │
                                    │  selectedId          │
                                    │  messages (全量)     │
                                    │  totalCount / hasMore│
                                    │  collapsedGroups     │
                                    └─────────────────────┘
```

**核心优势**：
- **零新模式**：store + subscribeEvents 是项目既定模式，新开发者立刻能理解
- **不改 chatStore**：避免对 Orb 侧的任何影响评估
- **绕过 ensureCurrentConversation**：主窗口的事件监听器直接调 `chatService.listConversations()` + `useChatStore.setState({ conversations })`，不走 `loadConversations()`
- **保留全量加载 + 分支平铺**：`historyStore.refreshMessages()` 调 `getConversationAllMessages`，不受 `activeBranchId` 过滤

### 5.3 新增 `useHistoryStore`

**文件**：`src/stores/historyStore.ts`

**职责**：管理主窗口 HistoryManager 的全部视图状态，将当前散落在组件内的 7 个 `useState` 提升为全局 store。

**包含的状态**（从 HistoryManager 原样搬出）：

| 状态 | 类型 | 说明 |
|------|------|------|
| `selectedId` | `string \| null` | 当前选中的对话 ID |
| `messages` | `Message[]` | 当前选中对话的全量消息（所有分支） |
| `totalCount` | `number` | 消息总数 |
| `hasMore` | `boolean` | 是否有更多消息可加载（主窗口通常为 false，因全量加载） |
| `loadingMore` | `boolean` | 加载更多中 |
| `loadingMessages` | `boolean` | 消息加载中 |
| `collapsedGroups` | `Set<string>` | 侧边栏分组折叠状态 |

**包含的方法**（从 HistoryManager 原样搬出）：

| 方法 | 说明 |
|------|------|
| `selectConversation(id)` | 选中对话，调用 `getConversationAllMessages` 全量加载消息 |
| `clearSelection()` | 清空选中（设 selectedId 为 null，messages 为空数组） |
| `refreshMessages()` | 重新加载当前选中对话的消息（事件驱动刷新入口） |
| `loadMoreMessages()` | 分页加载更早的消息（保留现有分页逻辑） |
| `toggleGroup(key)` | 切换侧边栏分组折叠状态 |

**设计要点**：
- 这些方法是从 HistoryManager **原样搬出**的，不是新写的——只是从 `useState` + `useCallback` 变成了 Zustand store 的 `set` / `get`
- `selectConversation` 和 `refreshMessages` 都走 `getConversationAllMessages`（全量），保证分支平铺功能不受影响
- `collapsedGroups` 也放入 store 是因为它是跨组件共享状态（Sidebar 和 Manager 都需要）

### 5.4 新增 `subscribeHistoryEvents`

**文件**：`src/services/historyEvents.ts`

**职责**：在主窗口挂载期间监听后端事件，驱动 `historyStore` 和 `chatStore`（仅 conversations 字段）的数据刷新。

**沿用的模式**：与 [settingEvents.ts](file:///g:/codeversion/assistant1/src/services/settingEvents.ts) 和 [chatDeltaEvents.ts](file:///g:/codeversion/assistant1/src/services/chatDeltaEvents.ts) 完全一致：
- 返回 cleanup 函数（`() => void`）
- 内部用 `disposed` 标志防止异步回调泄漏
- 用 `track()` 辅助函数统一处理 listener 注册和错误处理
- 事件处理函数内直接调 `store.getState()` 或 `store.setState()`，无回调桥接

**监听的事件及处理逻辑**：

| 事件 | 处理逻辑 |
|------|---------|
| `conversation-list-changed` | ① 判断当前选中是否被删除（见下文详细说明）；② 若被删除则调 `historyStore.clearSelection()`；③ 防抖 300ms 后刷新对话列表 |
| `chat-messages-changed` | 若 `payload.conversationId === historyStore.selectedId`，则调 `historyStore.refreshMessages()`（全量重载，不用 payload 的 messages） |
| `conversation_terminated` | 防抖 300ms 后刷新对话列表（isFull 状态变了） |
| `conversation_unlocked` | 防抖 300ms 后刷新对话列表 |
| `conversation_compact_failed` | **不监听**——compact 错误只在 Orb 的对话面板中显示，主窗口的历史详情页不展示 compact 状态 |

**对话列表刷新的实现细节**：

```typescript
// 不调 useChatStore.getState().loadConversations()
// （因为 loadConversations 内部会调 ensureCurrentConversation，对主窗口有害）
// 改为直接调 service + setState：
const convs = await chatService.listConversations();
useChatStore.setState({ conversations: convs });
```

这是方案 C 的关键设计决策：**绕过 chatStore 的 loadConversations 方法，直接操作其 conversations 字段**。这样既利用了 chatStore 作为 conversations 的单一数据源（两窗口共享），又避免了 ensureCurrentConversation 的副作用。

**选中状态清理的详细逻辑**（`conversation-list-changed` 监听器内）：

```typescript
const selectedId = useHistoryStore.getState().selectedId;
const { changeKind, conversationId, affectedIds } = event.payload;

const isSelectedDeleted =
  // 单个删除：conversationId 匹配
  (changeKind === "deleted" && conversationId === selectedId) ||
  // 批量删除：affectedIds 包含 selectedId
  (changeKind === "batch_deleted" &&
    selectedId != null &&
    affectedIds?.includes(selectedId)) ||
  // 全部清空：只要当前有选中就一定被删
  (changeKind === "purged" && selectedId !== null);

if (isSelectedDeleted) {
  useHistoryStore.getState().clearSelection();
}
```

> **为什么 `purged` 时 conversationId 为 null 但仍要清理？**
> `purged` 表示所有对话被清空，此时前端无法知道具体哪些 ID 被删（或者可以说所有 ID 都被删）。保守策略是：只要有当前选中就清空。这与 `batch_deleted` 不同——后者有 `affectedIds` 可以精确判断。

**`chat-messages-changed` 为什么不用 payload 的 messages？**

Orb 的 `chat-messages-changed` 监听器直接 `useChatStore.setState({ messages: payload.messages })`，因为 Orb 只看活跃分支，payload 里的 messages 就是它需要的。

但主窗口需要**全量消息（所有分支）**来做平铺展示，而 `chat-messages-changed` 的 payload 只包含**活跃分支的消息**（后端返回的是按 `activeBranchId` 过滤的结果）。所以主窗口收到此事件后必须重新调 `getConversationAllMessages` 全量拉取，不能用 payload。

> 性能影响：广播到主窗口的 `chat-messages-changed` 携带完整消息列表（可能几十 KB），但主窗口只用 `conversationId` 做判断，payload 的 messages 浪费了。短期接受（Tauri 2 事件系统不支持选择性反序列化），长期可考虑拆分为轻量通知事件 + 按需拉取。

### 5.5 HistoryManager 简化为薄组件

集成 `historyStore` + `subscribeHistoryEvents` 后，HistoryManager 从"状态管理+事件处理+UI 渲染"的厚组件简化为"纯 UI 渲染"薄组件：

**移除的内容**：
- 7 个 `useState`（selectedId, messages, totalCount, hasMore, loadingMore, loadingMessages, collapsedGroups）
- 5 个 `useCallback`（selectConversation, reloadMessages, loadMoreMessages, deleteMessage 中的 reloadMessages 部分, toggleGroup）
- 所有操作回调中的 `chatStoreLoadConversations(true)` 调用（5 处）

**保留的内容**：
- 从 `useHistoryStore` 读取状态的 selector hooks
- 从 `useChatStore` 读取 `conversations` 的 selector hook（两窗口共享）
- 操作回调中的 `invoke` 调用（rename/delete/togglePin/duplicate/export）
- 操作回调中的 toast 通知（message.success/error）
- `handleSearch` 等 search 相关逻辑
- JSX 渲染（HistorySidebar + HistoryDetail）

**新增的内容**：
- `useEffect` 中调用 `subscribeHistoryEvents()` 并在卸载时 cleanup

**示例（renameConversation 改动前后对比）**：

```
改动前：
  const renameConversation = useCallback(async (id, title) => {
    await historyPersistQueue.enqueue(() => historyService.renameConversation(id, title));
    await chatStoreLoadConversations(true);  ← 手动刷新（冗余）
    message.success(t("..."));
  }, [chatStoreLoadConversations, t]);

改动后：
  const renameConversation = useCallback(async (id, title) => {
    await historyPersistQueue.enqueue(() => historyService.renameConversation(id, title));
    // 不再手动 loadConversations —— 事件监听器会触发
    message.success(t("..."));
  }, [t]);
```

### 5.6 Orb 窗口补全 `conversation-list-changed` 监听

Orb 窗口当前通过 `chatDeltaEvents.ts` 监听 `conversation_terminated` 和 `conversation_unlocked` 来刷新对话列表，但元数据操作（重命名/置顶/复制等）在 Orb 侧也没有事件驱动刷新。

**改动位置**：`src/services/chatDeltaEvents.ts` 的 `subscribeChatDeltaEvents()` 中新增 `conversation-list-changed` 监听。

**处理逻辑**：
- 判断当前 `currentConversationId` 是否被删除（deleted/batch_deleted/purged），若是则清空
- 调 `useChatStore.getState().loadConversations(true)` 刷新列表（Orb 可以安全调这个方法，ensureCurrentConversation 对 Orb 是正确行为）

### 5.7 轮询降级为兜底

事件驱动为主路径，轮询仅防极端丢失。间隔从 15s 延长到 60s：

```
改动前：REFRESH_INTERVAL_MS = 15000
改动后：REFRESH_INTERVAL_MS = 60000
```

轮询同时负责刷新对话列表和消息（`refreshConversations` + `refreshMessages`）。事件驱动覆盖了大部分场景，60s 兜底足以应对极端事件丢失。

---

## 6. ③ 清理双写 — 操作方删掉手动刷新

### 6.1 问题

当前前端每个元数据操作都遵循 `invoke() → loadConversations()` 模式。加了事件监听后变成双刷新：

```
用户在 Orb 点重命名
  → chatStore.renameConversation()
    → invoke("rename_conversation")           ← 后端执行 + emit 事件
    → await loadConversations()               ← ① 手动刷新（现有代码，冗余）
  → 事件监听器收到 conversation-list-changed
    → loadConversations(true)                 ← ② 事件驱动刷新（操作方自己的监听器）
  → 主窗口事件监听器收到
    → loadConversations(true)                 ← ③ 主窗口刷新（跨窗口同步，核心价值）
```

① 和 ② 是同一次操作的重复刷新，② 和 ③ 才是跨窗口同步的核心价值。

**原则：谁触发操作，谁就不手动刷新列表——交给事件监听器统一处理。**

### 6.2 chatStore.ts 改动（Orb 侧）

| 操作 | 当前 | 改后 | 保留的本地状态逻辑 |
|------|------|------|-------------------|
| `renameConversation` | `invoke` + `loadConversations()` | 只 `invoke` | 无 |
| `updateModel` | `invoke` + `loadConversations()` | 只 `invoke` | 无 |
| `togglePin` | `invoke` + `loadConversations()` | 只 `invoke` | 无 |
| `duplicateConversation` | `invoke` + `loadConversations()` | 只 `invoke` | 无 |
| `deleteConversation` | `invoke` + 本地清理 + `loadConversations()` | `invoke` + 本地清理 | 清理 `currentConversationId`/`messages` |
| `batchDelete` | `invoke` + 本地清理 + `loadConversations()` | `invoke` + 本地清理 | 清理 `currentConversationId`/`messages` |
| `purgeAll` | `invoke` + 直接 set 空数组 | **保留不变** | 前端直接置空，无需 IPC |
| `createConversation` | `invoke` + `loadConversations()` + `switchConversation` | **保留不变** | 需要返回值做后续切换 |

> `createConversation` 保留手动 `loadConversations` 是因为 `switchConversation(id)` 依赖列表已加载。事件驱动的 `loadConversations(true)` 对操作方窗口是冗余但无害的，对其他窗口是同步。

### 6.3 HistoryManager.tsx 改动（主窗口侧）

删除各操作回调中的 `chatStoreLoadConversations(true)`：

```
改动前：
  const renameConversation = useCallback(async (id, title) => {
    await historyPersistQueue.enqueue(() => historyService.renameConversation(id, title));
    await chatStoreLoadConversations(true);  ← 删掉这行
    message.success(t("..."));
  }, [chatStoreLoadConversations, t]);

改动后：
  const renameConversation = useCallback(async (id, title) => {
    await historyPersistQueue.enqueue(() => historyService.renameConversation(id, title));
    message.success(t("..."));
  }, [t]);
```

同样适用于 `deleteConversation`、`togglePin`、`duplicateConversation`。

**注意**：`deleteConversation` 中的本地状态清理（`setSelectedId(null)` + `setMessages([])`）也需要删除——因为 `subscribeHistoryEvents` 的 `conversation-list-changed` 监听器已经会调 `historyStore.clearSelection()`。但保留也无害（幂等操作），可根据团队偏好决定是否删除。

### 6.4 deleteMessage 的特殊处理

`deleteMessage` 回调中同时有 `reloadMessages()` + `chatStoreLoadConversations(true)`：
- `chatStoreLoadConversations(true)` → **删除**（消息删除不改变列表结构）
- `reloadMessages()` → **暂时保留**，等确认 `subscribeHistoryEvents` 的 `chat-messages-changed` 监听稳定后再删

> 暂时保留的原因：`reloadMessages` 走 `getConversationAllMessages` 全量加载，而事件监听器的 `refreshMessages` 也走同样的路径。两者并发执行结果相同，但双重 IPC 浪费。待事件驱动路径验证稳定后再删 `reloadMessages`。

### 6.5 updateConversationProvider 的特殊处理

[ChatView.tsx](file:///g:/codeversion/assistant1/src/chat/ChatView.tsx) 中 `handleProviderChange` 直接调用 `chatService.updateConversationProvider()` 绕过 store。后端加了 `conversation-list-changed` 事件后，前端无需手动 `loadConversations`，事件监听器会处理。

---

## 7. 事件模块化治理（基础设施）

### 7.1 后端：事件名常量 + emit 辅助函数

在 `src-tauri/src/protocol/` 下新增 `events.rs`：

```rust
pub mod event_name {
    pub const CHAT_MESSAGES_CHANGED: &str = "chat-messages-changed";
    pub const CHAT_DELTA: &str = "chat-delta";
    pub const CHAT_STREAM_START: &str = "chat-stream-start";
    pub const CONVERSATION_LIST_CHANGED: &str = "conversation-list-changed";
    pub const CONVERSATION_TERMINATED: &str = "conversation_terminated";
    pub const CONVERSATION_UNLOCKED: &str = "conversation_unlocked";
    pub const CONVERSATION_COMPACT_FAILED: &str = "conversation_compact_failed";
    pub const ORB_NEW_CONVERSATION: &str = "orb-new-conversation";
    pub const ORB_BLUR: &str = "orb-blur";
    pub const NAVIGATE: &str = "navigate";
    pub const THEME_CHANGED: &str = "theme-changed";
    pub const SETTING_PROVIDER_CHANGED: &str = "setting:provider-changed";
    pub const SETTING_THEME_CHANGED: &str = "setting:theme-changed";
    pub const SETTING_CONF_PATCHED: &str = "setting:conf-patched";
    pub const SHORTCUT_WARNING: &str = "shortcut-warning";
}

pub fn emit_broadcast(
    app: &tauri::AppHandle,
    event: &str,
    payload: impl serde::Serialize,
) {
    let _ = app.emit(event, payload);
}

pub fn emit_orb_only(
    app: &tauri::AppHandle,
    event: &str,
    payload: impl serde::Serialize,
) {
    if let Some(win) = app.get_webview_window(crate::setup::WINDOW_ORB) {
        let _ = win.emit(event, payload);
    }
}
```

**使用方式**：

```rust
use crate::protocol::events::{event_name, emit_broadcast, emit_orb_only};

emit_broadcast(&app, event_name::CHAT_MESSAGES_CHANGED, payload);
emit_orb_only(&app, event_name::CHAT_DELTA, payload);
```

**收益**：
- 事件名拼写错误 → 编译失败
- `emit_broadcast` vs `emit_orb_only` → emit 策略在调用点显式声明
- 后续新增事件只需在 `event_name` 加一行常量

### 7.2 前端：事件名 enum

扩展 `src/types/events.ts` 为全事件注册表：

```typescript
export enum AppEvent {
  ChatMessagesChanged = "chat-messages-changed",
  ChatDelta = "chat-delta",
  ChatStreamStart = "chat-stream-start",
  ConversationListChanged = "conversation-list-changed",
  ConversationTerminated = "conversation_terminated",
  ConversationUnlocked = "conversation_unlocked",
  ConversationCompactFailed = "conversation_compact_failed",
  OrbNewConversation = "orb-new-conversation",
  OrbBlur = "orb-blur",
  Navigate = "navigate",
  ThemeChanged = "theme-changed",
  SettingProviderChanged = "setting:provider-changed",
  SettingThemeChanged = "setting:theme-changed",
  SettingConfPatched = "setting:conf-patched",
}
```

所有 `listen("chat-messages-changed", ...)` 改为 `listen(AppEvent.ChatMessagesChanged, ...)`。

### 7.3 前端类型补充

在 `src/chat/core/chatTypes.ts` 中新增：

```typescript
export type ConversationListChangeKind =
  | "renamed"
  | "deleted"
  | "pin_toggled"
  | "duplicated"
  | "model_updated"
  | "provider_updated"
  | "batch_deleted"
  | "purged"
  | "created";

export interface ConversationListChangedPayload {
  changeKind: ConversationListChangeKind;
  conversationId: string | null;
  affectedIds?: string[];  // 仅 batch_deleted 时有值
}
```

修正 `MessageChangeKind`：补入后端实际 emit 但类型遗漏的值，删除有害预留值：

```typescript
export type MessageChangeKind =
  | "branch_created"
  | "branch_switched"
  | "branch_forked"
  | "message_deleted"
  | "stream_finished";
```

**删除 `message_edited` / `messages_regenerated` 的理由**：

这两个值在 TS 枚举中定义，但后端从未 emit 过。原因是当前架构中"编辑"和"重新生成"都走 `resendFrom()` → `sendMessage()` → 流式输出 → `stream_finished` 路径，本质是"创建分支 + 重发"，不存在"原地修改/原地重新生成"的操作。保留这两个值是有害的：

- **误导**：暗示存在不存在的事件路径，新开发者可能写永远不会触发的处理逻辑
- **掩盖真问题**：注意力被"预留值"吸引，反而忽略 `stream_finished` 缺失的真 bug
- **违反 YAGNI**：加一个枚举值是一行代码的事，提前预留的收益几乎为零，成本是认知负担

如果未来实现"原地编辑消息内容（不创建分支）"，届时加回 `message_edited` 即可。

### 7.4 改动量评估

| 改动 | 文件数 | 行数 |
|------|--------|------|
| 新增 `protocol/events.rs` | 1 | ~40 |
| Rust 端替换字符串为常量 | 8 | ~45 处替换 |
| TS 端扩展 `events.ts` enum | 1 | ~15 |
| TS 端替换字符串为 enum | 7 | ~12 处替换 |

总计 ~60 处机械替换，无逻辑变更，风险极低。

---

## 8. 改动总览

### 8.1 后端（Rust）

| 文件 | 改动 |
|------|------|
| `protocol/events.rs` | **新建**：事件名常量 + `emit_broadcast` / `emit_orb_only` |
| `protocol/mod.rs` | 导出 events 模块 |
| `protocol/dto.rs` | 新增 `ConversationListChangedPayloadDto`（含 `affected_ids` 字段） |
| `cmd/conversation.rs` | ① 9 个元数据命令末尾加 `emit_broadcast`；② 3 处 `orb_window.emit` → `emit_broadcast`；③ `fork_to_new_conversation` 额外加 `conversation-list-changed` emit；④ 字符串事件名 → 常量 |
| `cmd/chat.rs` | ① `stream_finished` 的 `orb_window.emit` → `emit_broadcast`；② 字符串事件名 → 常量 |
| `cmd/config.rs` / `cmd/provider.rs` / `tray.rs` / `setup.rs` / `lib.rs` / `cmd/app.rs` | 字符串事件名 → 常量 |

### 8.2 前端

| 文件 | 改动 |
|------|------|
| `types/events.ts` | 扩展 `AppEvent` enum 覆盖全部事件名 |
| `chat/core/chatTypes.ts` | 新增 `ConversationListChangeKind`、`ConversationListChangedPayload`（含 `affectedIds`）；扩展 `MessageChangeKind` |
| `stores/historyStore.ts` | **新建**：主窗口视图状态 store（从 HistoryManager 搬出 7 个 state + 5 个方法） |
| `services/historyEvents.ts` | **新建**：主窗口事件监听模块（沿用 settingEvents/chatDeltaEvents 模式） |
| `history/HistoryManager.tsx` | ① 集成 `subscribeHistoryEvents()`；② 7 个 useState 替换为 useHistoryStore selector；③ 删除 5 处 `chatStoreLoadConversations(true)`；④ 5 个 useCallback 简化或删除 |
| `history/core/useHistoryRefresh.ts` | 轮询间隔 15s → 60s |
| `services/chatDeltaEvents.ts` | ① 新增 `conversation-list-changed` 监听（含选中状态清理）；② 字符串事件名 → `AppEvent.*` |
| `stores/chatStore.ts` | 删除 6 处 `loadConversations()` 调用，保留本地状态清理逻辑 |
| 其余含 `listen()` 的文件 | 字符串事件名 → `AppEvent.*` |

---

## 9. 不改动的部分

| 项目 | 原因 |
|------|------|
| `chat-delta` / `chat-stream-start` 仍用 `emit_orb_only` | 高频流式数据，主窗口不参与渲染，广播会增加无谓开销 |
| `orb-blur` 仍用 `emit_orb_only` | Orb 窗口自身 UI 控制 |
| `chatStore.createConversation()` | 保留手动 `loadConversations` + `switchConversation`，需要返回值 |
| `chatStore.purgeAll()` | 前端直接置空，无需 IPC |
| Orb 窗口现有 `chat-messages-changed` 监听 | `app.emit` 广播后 Orb 仍能收到，行为不变 |
| `chatStore.loadConversations()` 的 `ensureCurrentConversation` | 不修改该方法签名；主窗口通过 `subscribeHistoryEvents` 绕过它（直接调 service + setState） |
| `chatStore.switchConversation()` 的分页加载逻辑 | 不修改；主窗口通过 `historyStore.selectConversation` 走全量加载路径 |

---

## 10. 实施步骤

| Phase | 目标 | 改动 |
|-------|------|------|
| **1. 后端管道** | 所有数据变更都 `app.emit` 广播 | 新建 `protocol/events.rs`（常量+辅助函数）；新增 `ConversationListChangedPayloadDto`（含 `affected_ids`）；9 个元数据命令末尾加 `emit_broadcast`；4 处 `orb_window.emit` → `emit_broadcast`；`fork_to_new_conversation` 额外加 `conversation-list-changed` emit；全局字符串事件名 → 常量 |
| **2. historyStore** | 主窗口状态从组件提升到 store | 新建 `stores/historyStore.ts`，将 HistoryManager 的 7 个 useState + 5 个 useCallback 原样搬入；HistoryManager 改为 useSelector 读取 |
| **3. historyEvents** | 主窗口接通事件驱动 | 新建 `services/historyEvents.ts`，实现 4 个监听器（conversation-list-changed / chat-messages-changed / conversation_terminated / conversation_unlocked）；HistoryManager `useEffect` 中调 `subscribeHistoryEvents()`；删 5 处手动 `chatStoreLoadConversations(true)` |
| **4. Orb 补全** | Orb 也监听 conversation-list-changed | `chatDeltaEvents.ts` 新增监听；chatStore 删 6 处手动 `loadConversations()` |
| **5. 收尾** | 轮询降频 + 类型完善 | `useHistoryRefresh` 15s → 60s；前端事件名 enum + 类型补充；全局字符串 → enum 替换 |

---

## 11. 风险与缓解

### 11.1 已识别风险

| # | 风险 | 严重度 | 缓解措施 |
|---|------|--------|---------|
| 1 | `batch_deleted` 缺少 `affected_ids` 导致无法精确判断选中是否受影响 | 🔴 P0 | Payload 设计已包含此字段 |
| 2 | 操作方 invoke 返回 → 事件到达间隙（~300ms），列表短暂显示旧数据 | 🟡 P1 | 可接受的用户体验；如需优化可在操作方做乐观更新（写 chatStore.conversations 数组中单项） |
| 3 | `chat-messages-changed` 广播重 payload 到主窗口但主窗口不用 messages | 🟢 P2 | 短期接受；长期拆分为轻量通知 + 按需拉取 |
| 4 | `createConversation` 双重 loadConversations（手动 + 事件驱动） | 🟢 P2 | 事件驱动的对操作方是冗余但无害；可接受 |
| 5 | `deleteMessage` 的 `reloadMessages` 与事件驱动的 `refreshMessages` 双重执行 | 🟢 P2 | 暂时保留 `reloadMessages`，Phase 3 验证稳定后再删 |
| 6 | `historyStore` 是新 store，团队成员需理解其与 `chatStore` 的职责边界 | 🟡 P1 | 文档明确标注：historyStore 管理主窗口视图状态，chatStore 管理 Orb 对话状态 + 两窗口共享的 conversations |
| 7 | 主窗口隐藏（非销毁）期间事件丢失 | 🟢 P2 | 重新显示时初始 `useEffect` 会触发 `chatStoreLoadConversations()` 加载最新数据；60s 兜底轮询也覆盖 |

### 11.2 架构决策记录

| 决策 | 选择 | 否决的替代方案 | 原因 |
|------|------|---------------|------|
| 主窗口状态管理 | 新建 `historyStore` | 复用 `chatStore`（方案 A） | 分支平铺展示需要全量消息，chatStore.messages 只有活跃分支；ensureCurrentConversation 对主窗口有害 |
| 事件→状态传递方式 | `store.getState()` 直接调用（方案 C） | 回调桥接（方案 B） | 零新模式（沿用 settingEvents/chatDeltaEvents）；无回调稳定性问题（ref 桥接样板代码） |
| 对话列表刷新方式 | 直接调 `chatService.listConversations()` + `setState` | 改 `loadConversations` 加 `skipEnsure` 参数 | 不改 chatStore 签名，零对 Orb 影响 |
| `batch_deleted` payload | 携带 `affected_ids` | 保守清空选中 / 不清理 | 精确判断 vs 误伤 vs 遗漏，携带 ID 列表是最优解 |
| `chat-messages-changed` 在主窗口的处理 | 重新全量加载 | 直接用 payload | payload 只有活跃分支消息，主窗口需要全量（分支平铺） |

---

## 12. 验证方案

### 12.1 功能验证

| 场景 | 操作 | 预期 |
|------|------|------|
| Orb 重命名 → 主窗口 | Orb 中重命名对话 | 主窗口侧边栏 <1s 内更新标题 |
| 主窗口删除 → Orb | 主窗口删除对话 | Orb 对话列表 <1s 内移除该项；若删除的是 Orb 当前选中对话，Orb 清空选中 |
| Orb 置顶 → 主窗口 | Orb 中置顶对话 | 主窗口侧边栏 <1s 内排序更新 |
| Orb 发消息 → 主窗口 | Orb 中发送消息 | 主窗口选中同一对话时消息列表 <1s 内更新（全量重载，含所有分支） |
| Orb 分支操作 → 主窗口 | Orb 中创建/切换分支 | 主窗口选中同一对话时消息列表 <1s 内更新（全量重载，分支平铺视图反映最新） |
| 主窗口重命名 → Orb | 主窗口重命名对话 | Orb 对话列表 <1s 内更新标题 |
| 批量删除 → 两个窗口 | 主窗口批量删除 3 个对话 | 两窗口 <1s 内移除这 3 项；若 Orb/主窗口当前选中在其中，自动清空选中 |
| 全部清空 → 两个窗口 | 主窗口点击"清空所有" | 两窗口 <1s 内清空列表并清空选中 |
| Fork → 主窗口 | Orb 中 fork 到新对话 | 主窗口侧边栏 <1s 内出现新对话（依赖 fork_to_new_conversation 额外 emit） |

### 12.2 性能验证

| 指标 | 基线 | 目标 |
|------|------|------|
| 主窗口 `list_conversations` 调用频率 | 每 15s 1 次 | 事件驱动 + 每 60s 兜底 |
| `chat-messages-changed` 广播开销 | 仅 Orb 接收 | 两窗口接收，主窗口仅在选中同一对话时处理（且走全量重载而非用 payload） |
| `conversation-list-changed` 开销 | 无 | 每次元数据操作多 1 次 emit，payload ~50-200 bytes（batch_deleted 带 ID 列表时稍大） |

### 12.3 边界场景

| 场景 | 处理 |
|------|------|
| 主窗口未打开（仅 Orb） | `app.emit` 广播，主窗口无 listener，事件被丢弃，无副作用 |
| 事件丢失（极端情况） | 60s 兜底轮询覆盖 |
| 快速连续操作（如连续重命名 5 个对话） | 事件监听器内置 300ms 防抖，合并短时间内的多次对话列表刷新 |
| 操作方 invoke 后事件尚未到达 | 同进程 IPC，延迟 <1ms；极端丢失由 60s 兜底覆盖；用户可能看到 ~300ms 旧数据（防抖窗口），可接受 |
| 主窗口选中对话 A，Orb 删除对话 A | 主窗口 `conversation-list-changed` 监听器检测到 `isSelectedDeleted` → `historyStore.clearSelection()` → 右侧详情面板清空 |
| 主窗口选中对话 A，Orb 批量删除 [A, B, C] | `batch_deleted` 的 `affectedIds` 包含 A → `isSelectedDeleted` 为 true → 清空选中 |
| 主窗口选中对话 D，Orb 批量删除 [A, B, C] | `affectedIds` 不包含 D → `isSelectedDeleted` 为 false → 选中保持不变 ✅ |

---

## 13. 可选优化（P2，不在本次范围）

### 13.1 增量更新

`loadConversations` 每次全量拉取。可改为事件携带变更的 `ConversationDto`，前端直接 patch store。但当前列表通常 <100 条，全量拉取 <10ms，收益不大。

### 13.2 chat-messages-changed 拆分

当前 `chat-messages-changed` 对主窗口来说 payload 浪费（只读 conversationId）。可拆分为：
- `chat-messages-changed-light`：只传 `{ conversationId }`，主窗口用它触发全量重载
- `chat-messages-changed`（保持不变）：传完整 messages，Orb 直接用

但这增加了事件种类和维护成本，当前不必要。

### 13.3 完整事件抽象层

当项目规模增长到以下任一条件时，升级为 OpenBitFun 式的完整抽象层（`EventEmitter` trait + `TauriTransportAdapter` + `frontend_projection`）：

- 事件名 > 20 个
- 需要跨平台（Tauri 桌面 + Web）
- 需要事件过滤/投影
- 需要事件相关单测（`NullEmitter` mock）

当前不满足任何条件，最小化治理足够。
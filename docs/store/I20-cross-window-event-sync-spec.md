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
| **② 接通主窗口** | 主窗口添加事件监听 hook | 当前主窗口零监听，只靠 15s 轮询 |
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

同一个 `chat-messages-changed` 事件，5 处 emit 中 3 处用 `app.emit`（广播）、4 处用 `orb_window.emit`（定向）。主窗口即使加了监听也收不到后 4 种（`create_branch`、`switch_branch`、`fork_to_new_conversation`、`stream_finished`）。

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
| conversation.rs | 765 | `orb_window.emit("chat-messages-changed", ...)` | `emit_broadcast(&app, ...)` |
| conversation.rs | 803 | `orb_window.emit("chat-messages-changed", ...)` | `emit_broadcast(&app, ...)` |
| conversation.rs | 859 | `orb_window.emit("chat-messages-changed", ...)` | `emit_broadcast(&app, ...)` |
| chat.rs | 208 | `orb_window.emit("chat-messages-changed", ...)` | `emit_broadcast(&app, ...)` |

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
}
```

**change_kind 枚举**：

| change_kind | 含义 | conversation_id |
|-------------|------|-----------------|
| `renamed` | 对话重命名 | Some(id) |
| `deleted` | 对话删除 | Some(id) |
| `pin_toggled` | 置顶状态变更 | Some(id) |
| `duplicated` | 对话复制 | Some(新对话id) |
| `model_updated` | 模型切换 | Some(id) |
| `provider_updated` | Provider 切换 | Some(id) |
| `batch_deleted` | 批量删除 | None |
| `purged` | 全部清空 | None |
| `created` | 新建对话 | Some(id) |

**后端改动**：在以下函数末尾添加 `emit_broadcast`：

| 函数 | change_kind |
|------|-------------|
| `delete_conversation` | `"deleted"` |
| `rename_conversation` | `"renamed"` |
| `toggle_conversation_pin` | `"pin_toggled"` |
| `duplicate_conversation` | `"duplicated"` |
| `update_conversation_model` | `"model_updated"` |
| `update_conversation_provider` | `"provider_updated"` |
| `batch_delete_conversations` | `"batch_deleted"` |
| `purge_conversations` | `"purged"` |
| `create_conversation` | `"created"` |

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
        },
    );

    Ok(())
}
```

---

## 5. ② 接通主窗口 — 添加事件监听

### 5.1 新增 `useHistoryEvents` hook

**文件**：`src/history/core/useHistoryEvents.ts`

职责：在 HistoryManager 挂载期间监听后端事件，驱动数据刷新。

```typescript
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";
import type { ChatMessagesChangedPayload } from "../../chat/core/chatTypes";
import { useChatStore } from "../../stores/chatStore";

interface ConversationListChangedPayload {
  changeKind: string;
  conversationId: string | null;
}

interface UseHistoryEventsOptions {
  selectedId: string | null;
  onConversationsChanged: () => Promise<void>;  // 返回值被忽略，标记 Promise 仅表示异步副作用
}

export function useHistoryEvents({
  selectedId,
  onConversationsChanged,
}: UseHistoryEventsOptions): void {
  const disposedRef = useRef(false);
  const unlistenersRef = useRef<UnlistenFn[]>([]);
  const selectedIdRef = useRef(selectedId);
  selectedIdRef.current = selectedId;

  useEffect(() => {
    disposedRef.current = false;
    const unlisteners: UnlistenFn[] = [];

    const track = (promise: Promise<UnlistenFn>) => {
      promise
        .then((unlisten) => {
          if (disposedRef.current) unlisten();
          else unlisteners.push(unlisten);
        })
        .catch((err) => {
          console.debug("[history-events] listener unavailable:", err);
        });
    };

    let convTimer: ReturnType<typeof setTimeout> | null = null;
    const debouncedConvRefresh = () => {
      if (disposedRef.current) return;
      if (convTimer) clearTimeout(convTimer);
      convTimer = setTimeout(() => {
        convTimer = null;
        void onConversationsChanged();
      }, 300);
    };

    track(
      listen<ConversationListChangedPayload>(
        "conversation-list-changed",
        (event) => {
          if (disposedRef.current) return;
          const { changeKind, conversationId } = event.payload;

          if (
            (changeKind === "deleted" ||
              changeKind === "batch_deleted" ||
              changeKind === "purged") &&
            conversationId &&
            useChatStore.getState().currentConversationId === conversationId
          ) {
            useChatStore.setState({ currentConversationId: null, messages: [] });
          }

          debouncedConvRefresh();
        },
      ),
    );

    track(
      listen<ChatMessagesChangedPayload>(
        "chat-messages-changed",
        (event) => {
          if (disposedRef.current) return;
          const { conversationId, messages, activeBranchId } = event.payload;
          if (selectedIdRef.current === conversationId) {
            useChatStore.setState({ messages, activeBranchId });
          }
        },
      ),
    );

    track(
      listen("conversation_terminated", debouncedConvRefresh),
    );

    track(
      listen("conversation_unlocked", debouncedConvRefresh),
    );

    track(
      listen<ConversationCompactFailedPayload>(
        "conversation_compact_failed",
        (event) => {
          if (disposedRef.current) return;
          const { conversationId, error } = event.payload;
          if (selectedIdRef.current === conversationId) {
            useChatStore.setState({ compactError: error });
          }
        },
      ),
    );

    unlistenersRef.current = unlisteners;

    return () => {
      disposedRef.current = true;
      if (convTimer) clearTimeout(convTimer);
      for (const unlisten of unlisteners.splice(0)) unlisten();
    };
  }, [onConversationsChanged]);
}
```

**设计要点**：

1. **`selectedIdRef`**：用 ref 捕获 `selectedId` 最新值，避免作为 `useEffect` 依赖导致 listener 频繁重注册
2. **`chat-messages-changed` 直接使用 payload**：与 Orb 窗口行为一致，直接 `useChatStore.setState({ messages, activeBranchId })`，无需再 IPC 拉取
3. **`conversation_compact_failed` 监听**：主窗口选中正在 compact 的对话时也能显示错误
4. **跨窗口选中状态清理**：`conversation-list-changed` 中 `deleted`/`batch_deleted`/`purged` 时，若当前对话被删则清空 `currentConversationId`
5. **`onConversationsChanged` 必须是稳定引用**：该回调作为 `useEffect` 依赖，若每次渲染产生新引用会导致 listener 频繁重注册。Zustand selector 返回的 store 函数天然稳定；若用 `useCallback` 包裹也需确保依赖为空或稳定

### 5.2 HistoryManager 集成

```diff
  const { refreshing, handleRefresh } = useHistoryRefresh(
    chatStoreLoadConversations,
    selectedId,
    handleMessagesRefreshed,
  );

+ useHistoryEvents({
+   selectedId,
+   onConversationsChanged: () => chatStoreLoadConversations(true),
+ });
```

> 传 `true` 表示静默刷新（不显示 loading），事件驱动刷新不应弹出 loading 状态。

### 5.3 Orb 窗口补全 `conversation-list-changed` 监听

Orb 窗口当前通过 `chatDeltaEvents.ts` 监听 `conversation_terminated` 和 `conversation_unlocked`，但元数据操作在 Orb 侧也没有事件驱动刷新。

**改动位置**：`src/services/chatDeltaEvents.ts` 的 `subscribeChatDeltaEvents()` 中新增：

```typescript
track(
  listen<ConversationListChangedPayload>(
    "conversation-list-changed",
    (event) => {
      if (disposed) return;
      const { changeKind, conversationId } = event.payload;

      if (
        (changeKind === "deleted" ||
          changeKind === "batch_deleted" ||
          changeKind === "purged") &&
        conversationId &&
        useChatStore.getState().currentConversationId === conversationId
      ) {
        useChatStore.setState({ currentConversationId: null, messages: [] });
      }

      void useChatStore.getState().loadConversations(true);
    },
  ),
);
```

### 5.4 轮询降级为兜底

事件驱动为主路径，轮询仅防极端丢失。间隔从 15s 延长到 60s：

```diff
- const REFRESH_INTERVAL_MS = 15000;
+ const REFRESH_INTERVAL_MS = 60000;
```

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

### 6.2 chatStore.ts 改动

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

### 6.3 HistoryManager.tsx 改动

删除各操作回调中的 `chatStoreLoadConversations(true)`：

```diff
  const renameConversation = useCallback(
    async (id: string, title: string) => {
      try {
        await historyPersistQueue.enqueue(async () => {
          await historyService.renameConversation(id, title);
        });
-       await chatStoreLoadConversations(true);
        message.success(t("chat.history.renamed"));
      } catch (e) { ... }
    },
-   [chatStoreLoadConversations, t],
+   [t],
  );
```

同样适用于 `deleteConversation`、`togglePin`、`duplicateConversation`。

### 6.4 deleteMessage 的特殊处理

`deleteMessage` 回调中同时有 `reloadMessages()` + `chatStoreLoadConversations(true)`：
- `chatStoreLoadConversations(true)` → **删除**（消息删除不改变列表结构）
- `reloadMessages()` → **暂时保留**，等主窗口 `useHistoryEvents` 集成 `chat-messages-changed` 监听后再删

### 6.5 updateConversationProvider 的特殊处理

[ChatView.tsx:170](file:///g:/codeversion/assistant/src/chat/ChatView.tsx#L170) 中 `handleProviderChange` 直接调用 `chatService.updateConversationProvider()` 绕过 store。后端加了 `conversation-list-changed` 事件后，前端无需手动 `loadConversations`，事件监听器会处理。

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
}
```

扩展 `MessageChangeKind` 覆盖后端实际 emit 的全部 `changeKind`：

```typescript
export type MessageChangeKind =
  | "message_edited"
  | "message_deleted"
  | "messages_regenerated"
  | "branch_created"
  | "branch_switched"
  | "branch_forked"
  | "stream_finished";
```

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
| `protocol/dto.rs` | 新增 `ConversationListChangedPayloadDto` |
| `cmd/conversation.rs` | ① 9 个元数据命令末尾加 `emit_broadcast`；② 3 处 `orb_window.emit` → `emit_broadcast`；③ 字符串事件名 → 常量 |
| `cmd/chat.rs` | ① `stream_finished` 的 `orb_window.emit` → `emit_broadcast`；② 字符串事件名 → 常量 |
| `cmd/config.rs` / `cmd/provider.rs` / `tray.rs` / `setup.rs` / `lib.rs` / `cmd/app.rs` | 字符串事件名 → 常量 |

### 8.2 前端

| 文件 | 改动 |
|------|------|
| `types/events.ts` | 扩展 `AppEvent` enum 覆盖全部事件名 |
| `chat/core/chatTypes.ts` | 新增 `ConversationListChangeKind`、`ConversationListChangedPayload`；扩展 `MessageChangeKind` |
| `history/core/useHistoryEvents.ts` | **新建**：主窗口事件监听 hook |
| `history/HistoryManager.tsx` | ① 集成 `useHistoryEvents`；② 删除操作回调中的 `chatStoreLoadConversations(true)` |
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

---

## 10. 审查遗漏（已修正）

| # | 问题 | 修正 |
|---|------|------|
| 1 | `updateConversationProvider` 绕过 store 直接调用 service | 后端加 emit，前端无需手动 loadConversations |
| 2 | `deleteMessage` 中 `chatStoreLoadConversations(true)` 应删除 | 消息删除不改变列表结构 |
| 3 | `selectedId` 作为 useEffect 依赖导致 listener 频繁重注册 | 改用 `selectedIdRef` |
| 4 | 主窗口 `chat-messages-changed` 应直接用 payload 而非重新 IPC 拉取 | 与 Orb 行为一致，直接 `setState` |
| 5 | `orb-new-conversation` 主窗口不需要监听 | 事件分类表已标注 |
| 6 | `emit_orb_only` 在 Orb 窗口不存在时应静默跳过 | 返回 `()`，内部 `if let Some` |
| 7 | `conversation_compact_failed` 主窗口需要监听 | `useHistoryEvents` 已添加 |
| 8 | 删除对话后非操作方窗口 `currentConversationId` 未清理 | 监听器中对 `deleted`/`batch_deleted`/`purged` 做特殊处理 |
| 9 | `MessageChangeKind` TS 类型不完整 | 扩展为 7 种联合类型 |
| 10 | `ConversationListChangedPayload.changeKind` 类型为 `string` | 新增 `ConversationListChangeKind` 字面量联合类型 |

---

## 11. 验证方案

### 11.1 功能验证

| 场景 | 操作 | 预期 |
|------|------|------|
| Orb 重命名 → 主窗口 | Orb 中重命名对话 | 主窗口侧边栏 <1s 内更新标题 |
| 主窗口删除 → Orb | 主窗口删除对话 | Orb 对话列表 <1s 内移除该项 |
| Orb 置顶 → 主窗口 | Orb 中置顶对话 | 主窗口侧边栏 <1s 内排序更新 |
| Orb 发消息 → 主窗口 | Orb 中发送消息 | 主窗口选中同一对话时消息列表实时更新 |
| Orb 分支操作 → 主窗口 | Orb 中创建/切换分支 | 主窗口选中同一对话时消息列表实时更新 |
| 主窗口重命名 → Orb | 主窗口重命名对话 | Orb 对话列表 <1s 内更新标题 |

### 11.2 性能验证

| 指标 | 基线 | 目标 |
|------|------|------|
| 主窗口 `list_conversations` 调用频率 | 每 15s 1 次 | 事件驱动 + 每 60s 兜底 |
| `chat-messages-changed` 广播开销 | 仅 Orb 接收 | 两窗口接收，主窗口仅在选中同一对话时处理 |
| `conversation-list-changed` 开销 | 无 | 每次元数据操作多 1 次 emit，payload ~50 bytes |

### 11.3 边界场景

| 场景 | 处理 |
|------|------|
| 主窗口未打开（仅 Orb） | `app.emit` 广播，主窗口无 listener，事件被丢弃，无副作用 |
| 事件丢失（极端情况） | 60s 兜底轮询覆盖 |
| 快速连续操作 | 事件监听器内置 300ms 防抖，合并短时间内的多次 `loadConversations` |
| 操作方 invoke 后事件尚未到达 | 同进程 IPC，延迟 <1ms；极端丢失由 60s 兜底覆盖 |

---

## 12. 可选优化（P2，不在本次范围）

### 12.1 增量更新

`loadConversations` 每次全量拉取。可改为事件携带变更的 `ConversationDto`，前端直接 patch store。但当前列表通常 <100 条，全量拉取 <10ms，收益不大。

### 12.2 完整事件抽象层

当项目规模增长到以下任一条件时，升级为 OpenBitFun 式的完整抽象层（`EventEmitter` trait + `TauriTransportAdapter` + `frontend_projection`）：

- 事件名 > 20 个
- 需要跨平台（Tauri 桌面 + Web）
- 需要事件过滤/投影
- 需要事件相关单测（`NullEmitter` mock）

当前不满足任何条件，最小化治理足够。
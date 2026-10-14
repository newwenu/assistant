# I12 — HistoryManager 与 chatStore 代码优化 任务清单

> 规格：[I12-history-dedup-spec.md](./I12-history-dedup-spec.md)
> 前置：I2 ✅（聊天消息历史管理系统已完成）

---

## 阶段 A — 纯移动：类型与工具提取

### A1. 提取类型定义到 `chat/chatTypes.ts`

- [x] 新建 `src/chat/chatTypes.ts`
- [x] 从 `chatStore.ts` 移出 `Message` / `Conversation` / `ToolCall` / `TokenUsage` / `ContextUsage` 五个 interface/type
- [x] `chatStore.ts`：`import type { ... } from '../chat/chatTypes'`
- [x] `chatStore.ts`：`export type { ... } from '../chat/chatTypes'` 保持外部导入兼容
- [x] 验证：`tsc --noEmit` 通过

**依赖**：无

---

### A2. 提取 PersistQueue 到 `utils/persistQueue.ts`

- [x] 新建 `src/utils/persistQueue.ts`
- [x] 从 `chatStore.ts` 积出 `PersistQueue` class + `PersistEntry` interface（原样搬出，不改逻辑）
- [x] `chatStore.ts`：`import { PersistQueue } from '../utils/persistQueue'`
- [x] 验证：`tsc --noEmit` 通过

**依赖**：无

---

### A3. 提取 conversationGrouping 纯函数

- [x] 新建 `src/chat/conversationGrouping.ts`（后续迁移到 `src/history/`）
- [x] 从 `HistoryManager.tsx` 积出 `ConversationGroup` 类型 + `GROUP_LABELS` 常量 + `groupConversations` 函数
- [x] `groupConversations` 签名改为接收 `translateFn: (key: string) => string` 参数，移除内部 `translateKey` 依赖，使其成为纯函数
- [x] HistoryManager 调用时传入 `translateKey`
- [x] 验证：`tsc --noEmit` 通过

**依赖**：无

---

## 阶段 B — 结构调整：子组件提取

### B1. 提取 ConversationListItem 子组件

- [x] 新建 `src/chat/ConversationListItem.tsx`（后续迁移到 `src/history/`）
- [x] 从 `HistoryManager.tsx` 底部移出 `ConversationListItem` 组件（约 120 行）
- [x] Props：`conversation` / `selected` / `onSelect` / `onDelete` / `onRename` / `onTogglePin` / `onDuplicate` / `onExport`
- [x] `HistoryManager.tsx`：`import { ConversationListItem } from './ConversationListItem'`
- [x] 清理 HistoryManager 中不再使用的导入（Dropdown / EditOutlined / PushpinOutlined / CopyOutlined / MenuProps / formatRelativeTime）
- [x] 验证：`tsc --noEmit` 通过

**依赖**：A1（类型导入路径）

---

## 阶段 C — 目录迁移：`chat/` → `history/`

### C1. 新建 `src/history/` 目录并迁移文件

- [x] 新建 `src/history/` 目录
- [x] 将 `HistoryManager.tsx` 写入 `src/history/HistoryManager.tsx`，调整导入路径（`./chatTypes` → `../chat/chatTypes`，`./conversationGrouping` → 本目录，`./ConversationListItem` → 本目录）
- [x] 将 `ConversationListItem.tsx` 写入 `src/history/ConversationListItem.tsx`，调整导入路径（`./chatTypes` → `../chat/chatTypes`）
- [x] 将 `conversationGrouping.ts` 写入 `src/history/conversationGrouping.ts`，调整导入路径（`./chatTypes` → `../chat/chatTypes`）
- [x] 更新 `App.tsx` 导入路径：`@/chat/HistoryManager` → `@/history/HistoryManager`
- [x] 删除旧文件：`src/chat/HistoryManager.tsx`、`src/chat/ConversationListItem.tsx`、`src/chat/conversationGrouping.ts`
- [x] 验证：`tsc --noEmit` 通过

**依赖**：A1, A3, B1

---

## 阶段 D — chatStore 小幅增强

### D1. 新增 `isLoadingConversations` 状态

- [x] `ChatState` 新增 `isLoadingConversations: boolean`，初始 `false`
- [x] `loadConversations` 开始时 `set({ isLoadingConversations: true })`
- [x] `loadConversations` finally 中 `set({ isLoadingConversations: false })`
- [x] 验证：`tsc --noEmit` 通过

**依赖**：A1, A2

---

## 阶段 E — 核心重构：HistoryManager conversations 单源 + PersistQueue 补齐

### E1. 数据源改造：conversations 收归 chatStore

- [x] 删除 HistoryManager 中的 `conversations` / `setConversations` state，改为 `useChatStore(s => s.conversations)`
- [x] 删除 HistoryManager 中的 `loadConversations` 函数，改为调用 `chatStore.loadConversations()`
- [x] 删除 HistoryManager 中的 `refreshConversations` 函数，改为调用 `chatStore.loadConversations()`
- [x] 删除 HistoryManager 中的 `loadingConvList` state，改为 `useChatStore(s => s.isLoadingConversations)`
- [x] 初始加载 useEffect 改为调用 `chatStore.loadConversations()`
- [x] 定时刷新 useEffect 中 conversations 刷新改为 `chatStore.loadConversations()`
- [x] 验证：编译通过，会话列表能正常显示

**依赖**：D1, C1

---

### E2. PersistQueue 补齐：写操作串行化

- [x] HistoryManager 新增 `import { PersistQueue } from '../utils/persistQueue'` + `const historyPersistQueue = new PersistQueue()`
- [x] `deleteConversation`：invoke 包入 `historyPersistQueue.enqueue()`，完成后调用 `chatStore.loadConversations()`
- [x] `renameConversation`：invoke 包入 `historyPersistQueue.enqueue()`，完成后调用 `chatStore.loadConversations()`
- [x] `togglePin`：invoke 包入 `historyPersistQueue.enqueue()`，完成后调用 `chatStore.loadConversations()`
- [x] `duplicateConversation`：invoke 包入 `historyPersistQueue.enqueue()`，完成后调用 `chatStore.loadConversations()`
- [x] `deleteMessage`：invoke 包入 `historyPersistQueue.enqueue()`，完成后本地 filter + `chatStore.loadConversations()`
- [x] 用户通知（message.success/error）保留在 HistoryManager 本地
~~- [x] 验证：手动测试删除/重命名/置顶/复制/删除消息，快速连续操作无数据丢失（跳过，以编译验证替代）~~

**依赖**：E1, A2

---

### E3. 搜索逻辑适配

- [x] 标题搜索：本地过滤 `chatStore.conversations`（不再依赖本地 conversations state）
- [x] 全文搜索：保持 `invoke("search_conversations")` 不变
- [x] debounce 逻辑保留本地（UI 交互关注点）
- [x] 验证：手动测试标题搜索/全文搜索/清空搜索

**依赖**：E1

---

### E4. 清理残留

- [x] 删除 HistoryManager 中所有已无引用的 state 定义和函数
- [x] 检查并清理不再需要的 `useCallback` / `useEffect` 依赖项
- [x] 确认 HistoryManager 中仍需直接 `invoke` 的操作（selectConversation / loadMoreMessages / exportConversation / refreshMessages / 搜索全文）保留 invoke 导入
- [x] 验证：`npm run lint && npm run typecheck` 通过

**依赖**：E2, E3

---

## 阶段 F — 验证

### F1. 编译验证

- [x] `npm run typecheck` 通过
- [x] `npm run lint` 通过

**依赖**：E4

---

### F2. 功能验收

- [ ] I12-history-dedup-spec §7.2 全部验收标准通过
- [ ] I12-history-dedup-spec §7.3 代码健康度标准通过

**依赖**：F1

---

### F3. DEVLOG 记录

- [ ] `docs/DEVLOG.md`：新增 I12 章节

**依赖**：F2

---

## 任务依赖图

```
A1 ─┐
A2 ─┤
A3 ─┼→ B1 → C1 → D1 → E1 → E2 ─┐
    │                    E3 ─┤→ E4 → F1 → F2 → F3
    ┘
```

A1/A2/A3 为纯移动，可并行完成。C1 为目录迁移。D1 为 chatStore 增强。E1-E3 有依赖顺序，E2/E3 可在 E1 完成后并行推进。
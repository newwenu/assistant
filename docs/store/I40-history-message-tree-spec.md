# I40 — 主窗口历史消息树渲染规格

| 项 | 值 |
|---|---|
| 状态 | 现行（独立规格，不隶属 I38 任一阶段） |
| 日期 | 2026-11-08 |
| 关联 | [ARCHITECTURE §I3](../ARCHITECTURE.md)（历史管理交付项）、[I38d §3.4](./I38d-agent-frontend-modes-spec.md)（历史 `tool_calls_json` 展示口径）、[代码拆分原则](../principle/CODE-SPLIT-PRINCIPLE.md) |
| 影响面 | `src/history/detail/*`、`src/stores/historyStore.ts`、`src/services/historyService.ts`、`src/i18n/locales/*`、`src-tauri/crates/assistant-storage/src/conversations/msg_ops.rs` |

> **实现状态（2026-11-08）**：C1–C7 已完成并附单测；本批未实现 §5.3 的可选写库「设为当前分支」（P2）、§5.6 的深度徽标与焦点行高亮；焦点 / 展开 / 原始行状态由 `HistoryDetail` 本地状态承载（未进 `historyStore`，§7 相应调整）；`scroll_load_more` 已随分页移除。

---

## 0 定位

本文只回答一个问题：**主窗口 `/history` 的详情区，如何把「一次拉取到的全量消息（含所有分支）」渲染成可读、可切换分支的树**。

两套渲染目标不同，实现必须分开，不追求管线复用：

| 视图 | 数据范围 | 目标 |
|---|---|---|
| Orb / chat 消息区（`MessageList`） | **仅活动分支**（后端 `trace_active_branch`） | 正确渲染当前分支，线性流式阅读 |
| 主窗口历史详情区 | **全量消息**（后端 `mode: "all"`，含所有分支、工具行、压缩摘要行） | 让用户看清整棵分支树、查看任意分支、切换焦点分支 |

因此本文**只复用叶子渲染组件**（`MessageBubble` / `MessageContent` / `ReasoningBlock` / `ToolCallBubble`），**不复用** `MessageList` 的滚动与过滤管线，也不修改 `MessageList` 的任何行为；`MessageBubble` 仅新增一个通用显示开关 `showBranchPicker`（默认 `true`，历史页置 `false`）。组件内部实现不在本文复述，只定义调用契约（§5.4）。

---

## 1 目标与非目标

### 1.1 目标

| # | 目标 | 判据 |
|---|---|---|
| G1 | 全量可达 | 任意一条 `messages` 行（user / assistant / tool / compact_summary / system / 孤儿行）都能在详情区被看到（默认折叠者可按需展开） |
| G2 | 关系可见 | 父子层级、兄弟分支、分支点、当前焦点路径均有视觉表达（缩进 + 轨道 + 分支槽 + 焦点高亮） |
| G3 | 分支可切换 | 在详情区内切换焦点分支，切换后主干与旁支同步重算，且滚动定位到被切换节点 |
| G4 | 富内容正确渲染 | Markdown / 公式 / 多模态附件 / 推理内容 / 工具调用卡片 / 压缩摘要样式全部复用既有组件 |
| G5 | 不重复渲染 | 工具结果行不与其父 assistant 气泡里的工具卡片重复出现 |
| G6 | 大数据不卡 | 千行级会话滚动流畅、无空白与错位；刷新后不跳动 |
| G7 | 状态可持久 | 焦点与展开状态按会话记忆，消息刷新后按 id 校验，失效自动回退 |

### 1.2 非目标

- 不修改 `MessageList`、`MessageBubble`、`MessageContent`、`ToolCallBubble` 的行为或 props 语义。
- 不改 `mode: "all"` 的后端语义（仍返回全量行）；不新增 DTO 字段。
- 不在历史页提供编辑 / 重新生成 / 重试 / 分叉（写操作只保留「删除消息」与「删除会话」）。
- 不引入第三方虚拟化库（`package.json` 无 `@tanstack/react-virtual` / `react-window`，本次不新增依赖）。
- 不做跨会话消息树（一次只展示一个会话的树）。

### 1.3 与 chat 链路的边界

| 关注点 | chat（`src/chat/**`） | history（`src/history/**`） |
|---|---|---|
| 建树 | `MessageTree.fromActiveBranch`（单链） | `buildHistoryTree`（全量多叉树，独立实现） |
| 分支切换 | `chatService.switchBranch` 写库 + 事件刷新 | 默认**只读**焦点切换（§5.3） |
| 渲染入口 | `MessageList` → `MessageBubble` | `HistoryBranchView` → `HistoryBranchRow` → `MessageBubble` |
| 过滤 `tool` 行 | `MessageList` 直接 `filter` 掉 | 树中保留为**结构行**，渲染时透传（§5.2.3） |

> 建树逻辑不复用 `MessageTree`：`MessageTree` 的节点缓存与 `getTurnRootId` 语义服务于「重新生成分支点」，与「全量多叉树 + 焦点路径」职责不同，强行合并会让两侧都承担两类变化原因（违反[拆分原则](../principle/CODE-SPLIT-PRINCIPLE.md) §1）。

---

## 2 现状盘点

### 2.1 现有链路

```
MainLayout "/history"
  → HistoryManager（事件订阅 / 删除 / 导出）
  → HistoryDetail（header + 滚动容器 + buildBranchView + 虚拟滚动）
  → HistoryMessageItem（纯文本气泡 + 本地 activeIndex + BranchPicker）
```

数据来源：`historyService.getConversationAllMessages` → `chatService.getConversationAllMessages` → `get_conversation { mode: "all" }` → `get_messages`（`ORDER BY timestamp ASC`，全表、无分支过滤）。

### 2.2 缺陷与归属

| # | 缺陷 | 根因 | 修复归属 |
|---|---|---|---|
| D1 | 分支关系错误 | `buildBranchView` 按 **depth** 分组，把不同父节点、同深度的消息并成一个「兄弟组」 | §5.1 改为严格按 `parentId` 建树 |
| D2 | 分支切换是假切换 | `HistoryMessageItem` 用本地 `useState(activeIndex)`，只改显示、不改焦点、刷新后不同步 | §5.3 焦点上移到 store，按 id 记忆 |
| D3 | 工具行被当普通气泡 | 全量模式下 `role: "tool"` 未过滤，工具结果原文铺进列表 | §5.2.3 结构行透传 |
| D4 | 内容纯文本 | `HistoryMessageItem` 直接渲染 `msg.content` | §5.4 复用 `MessageBubble` |
| D5 | 推理 / 工具卡片 / 压缩摘要缺失 | 同上 | §5.4 / §5.5 |
| D6 | 虚拟滚动冻结 | `visibleSlots` 的 `useMemo` 依赖不含 `scrollTop`，滚动不触发重算；固定 80px 估值与真实高度不符 | §6 测量式虚拟列表 |
| D7 | 分页是死代码 | `mode:"all"` 后 `hasMore` 恒为 `false`，`loadMoreMessages` 永不进入；且按 `timestamp` 分页会切断父子链 | §4.3 明确删除 |
| D8 | key 可能越界 | `slot.siblings[slot.activeIndex].id` 在数据刷新后可能 undefined | §5.2 行 key 由节点 id 生成 |

---

## 3 概念模型

| 术语 | 定义 |
|---|---|
| **节点** | 一条消息在树中的位置，一个 `HistoryNode` 对应一条 `messages` 行 |
| **兄弟组** | 同一 `parentId` 下的所有子节点，按 `siblingIndex` 排序 |
| **主干（spine）** | 从某个根到**焦点叶**的路径，是默认完整展开的那条链 |
| **旁支（side branch）** | 兄弟组中不在主干上的成员及其子树 |
| **焦点叶（focus leaf）** | 用户当前关注的节点；主干 = 根到该节点的路径 |
| **行（row）** | 展平后的渲染单元，只有两种：`message`（一条气泡）与 `stub`（一组折叠旁支） |

示意（`U1`/`A1` 为第一轮，`A1` 有两个版本；`A1b` 是旁支）：

```
U1
 └ A1            ← 主干（焦点叶 = A2）
    ├ A1b        ← 旁支：折叠为 stub 行（可展开）
    └ T1         ← 工具行：结构行，渲染透传
       └ A2      ← 主干
```

展平后：

```
row[0] message U1
row[1] stub  "其他分支（1）"（A1b）
row[2] message A1
row[3] message A2      ← 工具行 T1 未产生 row，A2 与 A1 同深度
```

---

## 4 数据契约

### 4.1 前端可用的既有字段（无需改动）

| 字段 | 用途 |
|---|---|
| `Message.id` / `parentId` | 建树的父子边；`parentId` 缺失或指向不存在的 id → 孤儿 |
| `Message.siblingIndex` / `siblingCount` | 兄弟组内排序与分支数；`siblingCount` 由后端 `get_sibling_counts_batch` 按 `parent_id` 全表计数，与返回集合无关，全量模式下准确 |
| `Message.role` | 过滤与样式分支 |
| `Message.timestamp` | 叶子平局排序、时间显示 |
| `Message.toolCalls` | 历史工具卡片；由 `toChatMessage` 把 DTO 合并结果转为 `ToolCallView` |
| `Message.reasoning` | 推理块 |
| `ConversationDetail.conversation.activeBranchId` | 默认焦点的首选叶 |
| `ConversationDetail.totalCount` | header 计数 |

### 4.2 后端改动（唯一一处，低风险）

`src-tauri/crates/assistant-storage/src/conversations/msg_ops.rs` 的 `get_messages`：

- 现状：`ORDER BY timestamp ASC`。
- 改为：`ORDER BY timestamp ASC, rowid ASC`。
- 理由：同一毫秒写入的兄弟行（如并行写入的 tool 行）顺序不确定，会让前端 DFS 展平顺序在多次刷新间抖动；`rowid` 是 SQLite 隐式单调键，与 `siblingIndex` 一致，作为稳定兜底。
- 不改返回结构、不改 DTO、不改 `mode` 语义。

### 4.3 明确不改 / 明确删除

| 项 | 处置 |
|---|---|
| `mode: "all"` 全量返回 | **保留**。树的分支分组需要全量；分页会切断父子链与兄弟计数 |
| `historyStore.hasMore` / `loadingMore` / `loadMoreMessages` | **删除**（D7 死代码），同步清理 `HistoryDetail` 的 `onLoadMore` / `hasMore` / `loadingMore` props 与「向上滚动加载更多」提示 |
| `historyService.getConversationPaginated` | 保留（chat 链路使用），历史页不再调用 |
| `MessageDto` 字段 | **不新增**。`tool` 行无法定位所属 `callId`，原始行模式按通用只读卡片渲染（§5.5） |

---

## 5 视图规格

### 5.1 建树 `buildHistoryTree`

纯函数，无 React 依赖，放在 `src/history/detail/historyTree.ts`。

```ts
export interface HistoryNode {
  message: Message;
  children: HistoryNode[];
  depth: number;              // 根为 0
  subtreeSize: number;        // 含自身
  deepestLeafId: string;      // 子树最深叶（平局取 timestamp 大者）
  orphan: boolean;            // parentId 缺失 / 指向集合外
}

export interface HistoryTree {
  roots: HistoryNode[];
  nodeById: Map<string, HistoryNode>;
  orphanRoots: HistoryNode[]; // 与 roots 分离，便于单独成组展示
  orphanGroups: Map<string, HistoryNode[]>; // key = 缺失的父 id，孤儿按此分组
}
```

规则：

1. 以 `parentId` 建边；`parentId == null` 或 `!nodeById.has(parentId)` → 挂到 `orphanRoots`，`orphan = true`。
2. 兄弟组排序：`siblingIndex ASC` → `timestamp ASC` → `id` 字典序（保证确定性）。
3. `roots` 与 `orphanRoots` 各自按上述规则排序。
4. 孤儿按**缺失的父 id**分组写入 `orphanGroups`，而不是统一归为一组：`siblingIndex` / `siblingCount` 是后端按父 id 算出来的，同父孤儿之间才是真兄弟，跨父孤儿不是。分组后每个缺失父 id 各自形成一个 `stub` 组，互不干扰。
5. 环检测：DFS 计算 `depth` 时维护 `visited`，遇环把该节点降级为孤儿并 `log.warn`，不抛错、不死循环。
6. `subtreeSize` / `deepestLeafId` 自底向上一次算完，避免渲染期重复遍历。
7. **不做角色过滤**：`tool` 行必须留在树里，否则其子节点（下一轮 assistant）会因父节点缺失变成孤儿（见 §5.2.3）。

### 5.2 展平 `flattenHistoryRows`

纯函数，签名：

```ts
export type HistoryRow =
  | {
      kind: "message";
      key: string;              // `message:${id}`
      node: HistoryNode;
      indent: number;           // 视觉缩进层级（工具行透传时不递增）
      lane: number;             // 分支色轨道编号
      dimmed: boolean;          // 不在焦点路径上
      onFocusPath: boolean;
    }
  | {
      kind: "stub";
      key: string;              // `stub:${parentId}`
      parentId: string;         // 根组用 "__root__"
      indent: number;
      lane: number;
      siblings: HistoryNode[];
      expanded: boolean;
      orphan: boolean;          // 孤儿组渲染告警图标
    };

export function flattenHistoryRows(
  tree: HistoryTree,
  focusLeafId: string | null,
  expandedKeys: ReadonlySet<string>,
  opts: { rawRows: boolean },
): HistoryRow[];
```

#### 5.2.1 焦点路径

```ts
export function computeFocusPath(tree: HistoryTree, focusLeafId: string | null): Set<string>
```

- 从 `focusLeafId` 沿 `parentId` 上溯，返回 id 集合。
- `focusLeafId` 为 `null` 或不在 `nodeById` 中 → 返回空集，展平期改用「最深子树优先」兜底。
- `resolveDefaultFocusLeaf(tree, activeBranchId)`：`activeBranchId` 在树中则用它；否则取最后一个根的最深叶（平局取 `timestamp` 大者）。

#### 5.2.2 展平算法

```
emitGroup(rows, parentId, children, childDepth, parentLane, parentDimmed):
  if children 为空: return
  preferred = children 中在焦点路径上的那个
             ?? children 中 deepestLeafId 深度最大的那个（平局取 timestamp 大者）
  others = children - preferred

  if others 非空:
    key = `stub:${parentId}`
    rows.push({ kind:"stub", key, parentId, depth: childDepth, lane: parentLane,
                siblings: others, expanded: expandedKeys.has(key) })
    if expandedKeys.has(key):
      for other in others:                       // 每个旁支独立一条轨道，统一淡化
        emitNode(rows, other, childDepth, laneOf(other, parentLane), true)

  if preferred 存在:
    emitNode(rows, preferred, childDepth, laneOf(preferred, parentLane),
             parentDimmed || !focusPath.has(preferred.message.id))

emitNode(rows, node, depth, lane, dimmed):
  if node.message.role == "tool" && !opts.rawRows:
    // 结构行透传：不产生 row，子节点继承当前 depth / lane
    emitGroup(rows, node.message.id, node.children, depth, lane, dimmed)
    return
  rows.push({ kind:"message", key:`message:${node.message.id}`, node, depth, lane,
              dimmed, onFocusPath: focusPath.has(node.message.id) })
  emitGroup(rows, node.message.id, node.children, depth + 1, lane, dimmed)

laneOf(node, parentLane) =
  node.message.siblingCount > 1 ? node.message.siblingIndex : parentLane
```

入口：先 `emitGroup(tree.roots, "__root__", 0, 0, false)`；若 `tree.orphanRoots` 非空，再以 `parentId = "__orphan__"` 追加一组（渲染时标题为「未挂载消息」）。

#### 5.2.3 为什么 `tool` 行必须保留在树里

Agent 链是 `assistant → tool → assistant` 的**链式**存储：第二轮的 `assistant.parentId` 指向工具行 id。若建树时按角色过滤掉工具行，第二轮 assistant 就会因父节点缺失被判为孤儿，分支关系整体断裂。

因此规则是：**建树保留、渲染透传**。工具行的内容已通过 `tool_results` 合并进父 assistant 的 `toolCalls`（后端 `message_tool_calls`），由 `ToolCallBubble` 呈现，不需要也不应该再单独出一个气泡。

#### 5.2.4 行 key 与稳定性

- `message` 行 key 只用消息 id，`stub` 行 key 只用 `parentId`。
- 展开状态按 key 记忆；消息刷新后 key 不变 → 展开状态不丢。
- 不使用「数组下标」「sibling 在组内位置」作为 key（避免 D8 越界与刷新后错位）。

### 5.3 焦点与切换

历史页**只有一个切换入口**：`stub` 行里的旁支预览列表。`MessageBubble` 自带的 `BranchPicker`（`1/2 ◀ ▶`）在历史页关闭（`showBranchPicker={false}`），原因：

- 语义不同：`BranchPicker` 在 chat 里对应 `switchBranch` 写库，在历史里只能改视图焦点，同一个图标两种效果会误导用户。
- 功能重复：任何有兄弟的节点上方必有一行 `stub`，已按角色 + 内容预览列出全部旁支，信息比 `1/2` 计数更全。

| 交互 | 行为 |
|---|---|
| 首次打开会话 | `resolveDefaultFocusLeaf(tree, conversation.activeBranchId)` |
| 点 `stub` 行内的某个旁支预览项 | 焦点切到该旁支节点的**最深叶**；该 `stub` 自动展开，被选项脱离 stub 成为主干 |
| 点任意 `message` 行气泡 | 焦点切到该节点的最深叶（点旁支气泡时同时把焦点路径移到该旁支） |
| 消息刷新 / 删除后 | 焦点 id 失效 → 回退到 `activeBranchId`；仍失效 → `resolveDefaultFocusLeaf` |
| 「设为当前分支」（可选，P2） | 显式按钮 → `chatService.switchBranch(conversationId, focusLeafId)`，写库并广播；**默认不写库** |

只读语义的理由：历史页是查看工具；若切换即写库，Orb 里正在使用的对话分支会被静默改变，违背用户预期。默认只切视图焦点，需要真正切换时由用户点显式按钮。

焦点切换后的滚动（已实现）：`HistoryVirtualList` 接收 `revealKey` + `revealNonce`，`onFocusNode` 每次把 nonce 自增触发一次定位；`revealKey` 是**被点中的那一行**（`message:${focusLeafId}`），不是焦点叶，避免「点中间某行却跳到最新消息」。定位时先查 DOM：目标行已完整可见则**完全不滚动**；不可见才居中，虚拟模式下先用 `offsets` 反算 `top` 再下一帧按 `data-row-key` 校正。若该节点位于折叠 `stub` 内或为未渲染的 `tool` 行，则不做定位。

### 5.4 叶子渲染复用契约

`HistoryBranchRow` 对 `message` 行渲染：

```tsx
<MessageBubble
  message={row.node.message}
  isStreaming={false}
  compact={false}
  showBranchPicker={false}
  onDelete={onDeleteMessage}
/>
```

| `MessageBubble` prop | 历史页取值 | 说明 |
|---|---|---|
| `message` | 树节点消息 | `toolCalls` 已是 `ToolCallView` |
| `isStreaming` | `false` | 历史只读 |
| `onDelete` | 传入 | 走现有子树删除确认流程 |
| `showBranchPicker` | `false` | 历史页分支切换统一走 `stub` 行（§5.3） |
| `onSwitchBranch` | **不传** | `showBranchPicker=false` 后无入口，避免空实现 |
| `onEdit` / `onRegenerate` / `onRetry` / `onFork` / `onErrorAction` | **不传** | 对应按钮/区块自动不渲染 |
| `retryStatus` | 不传 | 历史无重试态 |
| `compact` | `false` | 历史页不启用紧凑密度 |

由此自动获得：Markdown、KaTeX、图片/文件附件、`ReasoningBlock`、`ToolCallBubble`、`compact_summary` 摘要样式、复制/删除动作、时间戳。

历史特有的视觉元素（缩进、轨道线、分支色、stub 行、孤儿标记、焦点徽标）全部在 `HistoryBranchRow` / `HistoryStubRow` 外层实现，不侵入 `MessageBubble`。

### 5.5 角色过滤与原始行模式

| `role` | 默认渲染 | 原始行模式 |
|---|---|---|
| `user` | `MessageBubble`（含多模态附件） | 同左 |
| `assistant` | `MessageBubble` + 推理 + 工具卡片 | 同左 |
| `compact_summary` | `MessageBubble`（压缩摘要样式） | 同左 |
| `system` | `MessageBubble`（system 样式） | 同左 |
| `tool` | **透传**（不产生气泡，内容已在父气泡工具卡片中） | 只读卡片：等宽字体 + 角色标签 + 完整 `content` |
| 未知 `role` | 按 assistant 样式兜底 | 同左 |

原始行模式是 header 上的开关（`history.tree.raw_rows`），默认关闭，用于排查「工具结果没有对应卡片」等数据异常。

### 5.6 视觉规格

| 元素 | 规格 |
|---|---|
| 缩进 | `paddingLeft = min(depth, 6) * 16px`；`depth > 6` 不再增加缩进（深度徽标留待 P2） |
| 轨道线 | 行左侧 3px 竖线，颜色按 `lane` 取调色板（复用现有 `BRANCH_COLORS` 八色，迁入 `historyTree.ts` 导出） |
| 旁支淡化 | `dimmed = true` 的行整体 `opacity: 0.55`，hover 恢复 `1`；焦点路径 `opacity: 1` |
| stub 行 | 虚线边框卡片：`其他分支（N）` + 每个旁支一行预览（角色标签 + 内容首行截断 60 字 + 时间）；整行可点击展开/收起 |
| 孤儿组 | 组标题「未挂载消息」+ 警示图标，样式与普通分支区分 |
| 焦点定位 | 切换后滚动到被点行（已实现）；行高亮样式预留未启用 |

### 5.7 删除与刷新

- 删除沿用 `HistoryManager.deleteMessage`（先查子树大小 → 二次确认 → `deleteMessagePersist` → `refreshMessages`）。
- 刷新后：按 id 校验 `focusLeafId` 与 `expandedKeys`；失效项剔除，不整树重置。
- `chat-messages-changed` 触发的自动刷新：保留当前 `scrollTop`（虚拟列表内做 clamp 恢复），避免阅读位置跳变。

---

## 6 虚拟滚动

### 6.1 现状缺陷（D6）

`HistoryDetail` 的 `visibleSlots` 在 `useMemo` 内读取 `messageListRef.current.scrollTop`，依赖数组只有 `[branchSlots, useVirtualScroll]`；`handleScroll` 只改 `userScrolledUp`，**不会触发重算**。超过阈值后窗口冻结。同时 `ESTIMATED_ITEM_HEIGHT = 80` 与 Markdown 气泡真实高度差距过大。

### 6.2 测量式虚拟列表

新增 `src/history/detail/HistoryVirtualList.tsx`（通用、无业务知识）：

```ts
interface HistoryVirtualListProps {
  rows: HistoryRow[];
  renderRow: (row: HistoryRow) => React.ReactNode;
  rowKey: (row: HistoryRow) => string;
  scrollTop: number;            // 容器测得后传入
  viewportHeight: number;       // 容器测得后传入
  scrollContainerRef?: React.RefObject<HTMLDivElement | null>;
  revealKey?: string | null;    // 切换焦点后需要定位的行 key
  revealNonce?: number;         // 每次切换自增，触发一次定位
  overscan?: number;            // 默认 VIRTUAL_SCROLL_OVERSCAN
  threshold?: number;           // 默认 VIRTUAL_SCROLL_THRESHOLD
}
```

规则：

1. `rows.length <= threshold` → 直接全量渲染（不测量、不加 spacer）。
2. 超过阈值 → 维护 `heights: Map<key, number>` 与 `offsets: number[]`（前缀和）。
3. 滚动位置与视口高度由外层容器（`HistoryDetail`）测量后传入，本组件不直接订阅滚动事件；据此二分求可见区间 `[start, end)`，上下各留 `overscan` 行。
4. 测量：每个渲染行挂 `ResizeObserver`，回调写入 `heights`；高度变化超过 1px 才更新，避免抖动。
5. 未测量行使用 `ESTIMATED_ROW_HEIGHT = 96` 估算；总高度 = `sum(heights)`。
6. 上下 spacer 用 `height: offsets[start]` / `offsets[len] - offsets[end]`。
7. `rows` 变化时：只失效被移除 key 的高度，保留其余测量值；`scrollTop` 按 `clamp(0, scrollHeight - clientHeight)` 恢复。
8. 容器滚动条与 `scrollToBottom` 按钮沿用现有样式。

### 6.3 参数

| 常量 | 值 | 位置 |
|---|---|---|
| `VIRTUAL_SCROLL_THRESHOLD` | 100（沿用） | `src/config/constants.ts` |
| `VIRTUAL_SCROLL_OVERSCAN` | 5（沿用） | 同上 |
| `ESTIMATED_ROW_HEIGHT` | 96（新增） | `HistoryVirtualList.tsx` 内部常量 |

> 删除 `SCROLL_LOAD_THRESHOLD_PX` 在历史页的使用（分页移除后无消费方；若其它模块仍用则保留常量）。

---

## 7 状态管理

本批焦点 / 展开 / 原始行模式由 `HistoryDetail` 组件本地状态承载，未进 `historyStore`：

| 状态 | 位置 | 语义与清理 |
|---|---|---|
| `focusLeafId` | `HistoryDetail` `useState` | 切会话时置 `null`，交给视图按 `activeBranchId` 解析；消息刷新后失效则由 `resolveDefaultFocusLeaf` 兜底 |
| `expandedKeys` | `HistoryDetail` `useState` | 切会话清空；消息刷新不清空，失效 key 无害 |
| `rawRows` | `HistoryDetail` `useState` | 切会话重置为 `false` |

`historyStore` 仅删除分页死代码：`hasMore` / `loadingMore` / `loadMoreMessages`。需要跨组件共享焦点或多窗口同步时再迁移到 store（P2）。

---

## 8 文件职责划分

| 文件 | 动作 | 职责 |
|---|---|---|
| `src/history/detail/historyTree.ts` | 新增 | 纯函数：建树、焦点路径、默认焦点、展平、调色板导出。无 React |
| `src/history/detail/HistoryBranchView.tsx` | 新增 | 树渲染容器：解析焦点、调展平、组合虚拟列表、处理切换与展开 |
| `src/history/detail/HistoryBranchRow.tsx` | 新增 | 单条 `message` 行：缩进 + 轨道 + `MessageBubble` |
| `src/history/detail/HistoryStubRow.tsx` | 新增 | `stub` 行：旁支预览列表 + 展开/收起 |
| `src/history/detail/HistoryVirtualList.tsx` | 新增 | 测量式虚拟列表（通用） |
| `src/history/detail/HistoryDetail.tsx` | 改写 | 仅保留 header、空态、容器；消息区交给 `HistoryBranchView` |
| `src/history/detail/HistoryMessageItem.tsx` / `.module.css` | 删除 | 被 `HistoryBranchRow` + `MessageBubble` 取代 |
| `src/history/detail/HistoryDetail.module.css` | 改写 | 移除 slot/虚拟滚动样式，保留 header / 容器 / 空态 |
| `src/stores/historyStore.ts` | 改写 | 删除分页死代码；焦点 / 展开 / 原始行状态留在 `HistoryDetail` 本地（§7） |
| `src-tauri/crates/assistant-storage/src/conversations/msg_ops.rs` | 改写 | `get_messages` 加 `rowid ASC` 次级排序 |
| `src/i18n/locales/zh-CN.json` / `en-US.json` | 改写 | §9 新增键 |

---

## 9 i18n 键

新增于 `history` 命名空间：

| key | zh-CN | en-US |
|---|---|---|
| `history.tree.other_branches` | `其他分支（{{count}} 条）` | `Other branches ({{count}})` |
| `history.tree.expand` | `展开` | `Expand` |
| `history.tree.collapse` | `收起` | `Collapse` |
| `history.tree.orphan_group` | `未挂载消息（{{count}} 条）` | `Detached messages ({{count}})` |
| `history.tree.raw_rows` | `原始行` | `Raw rows` |
| `history.tree.collapse_all` | `收起分支` | `Collapse branches` |
| `history.tree.focus_hint` | `当前查看分支` | `Viewing branch` |

删除：`history.scroll_load_more`（分页移除）。`set_active_branch` / `depth_badge` / `raw_rows_hint` 留待对应功能落地时再补（P2）。

---

## 10 验收标准

### 10.1 单元测试（`src/history/detail/__tests__/historyTree.test.ts`）

| # | 场景 | 断言 |
|---|---|---|
| T1 | 单链会话（user → assistant → user → assistant） | 4 行、无 stub、depth 递增 |
| T2 | Agent 多迭代链（A1 → T1 → A2 → T2 → A3） | 工具行不产生 row；A1/A2/A3 同深度且顺序正确 |
| T3 | 一个分支点含 2 个兄弟 | 非焦点兄弟生成 1 个 stub 行；焦点兄弟正常展开 |
| T4 | stub 展开 | 旁支节点及其子树全部出现在 rows 中，`dimmed = true` |
| T5 | 焦点叶为空 / 不存在 | 回退到最深叶；不抛错 |
| T6 | 孤儿（`parentId` 指向不存在 id） | 进入 `orphanRoots`，单独成组 |
| T7 | 环（A.parent=B, B.parent=A） | 降级为孤儿 + 告警，不死循环 |
| T8 | 原始行模式 | 工具行产生 `message` 行 |
| T9 | 兄弟排序 | `siblingIndex` 升序；同值按 `timestamp` / id 稳定 |
| T10 | `compact_summary` | 作为普通节点渲染，不被过滤 |

### 10.2 手工验收

1. 打开一个含 3 个分支、含 Agent 工具迭代的会话：每个迭代一个气泡、工具卡片在发起它的气泡内、无重复工具原文。
2. 点 stub 行里的某个旁支预览项：主干与旁支同步重算，焦点行居中滚动。
3. 展开旁支 stub：该旁支完整子树出现，缩进与轨道正确，切回焦点后状态保留。
4. 刷新（切换会话再切回 / 等待 `chat-messages-changed`）：焦点与展开状态保持，滚动位置不跳。
5. 1000+ 行会话：滚动无空白、无错位；`scrollToBottom` 可用。
6. 删除一条消息：子树删除确认 → 刷新 → 焦点自动回退，不出现空树或崩溃。
7. 原始行模式开关：工具行以只读卡片出现；关闭后消失。

### 10.3 回归项

- chat 侧 `MessageList` / `MessageBubble` / `MessageTree` 行为与测试**零改动**。
- `npm run lint`、`npm run typecheck`、`npm run test` 通过。
- 后端改动后 `cargo test`（cwd = `src-tauri`）通过。

---

## 11 实施顺序

| 步 | 内容 | 依赖 |
|---|---|---|
| C1 | `historyTree.ts` 纯函数 + T1–T10 单测 | 无 |
| C2 | 后端 `get_messages` 次级排序 | 无 |
| C3 | `HistoryVirtualList` 测量式虚拟列表 | 无 |
| C4 | `HistoryBranchRow` / `HistoryStubRow`（复用 `MessageBubble`） | C1 |
| C5 | `HistoryBranchView` 组合焦点、展平、切换、展开 | C1/C3/C4 |
| C6 | `HistoryDetail` 改写 + `historyStore` 新字段 + 删除分页死代码 | C5 |
| C7 | i18n 键补齐 + 删除 `HistoryMessageItem` | C6 |
| C8 | 手工验收 + lint/typecheck/test | 全部 |

---

## 12 风险与明确不做

| # | 风险 | 处理 |
|---|---|---|
| R1 | 全量树 DOM 高度测量开销 | 虚拟列表按 key 缓存高度；旁支默认折叠不构建子树 DOM |
| R2 | 只读切换让用户误以为改了库 | stub 行与 picker 使用「查看分支」措辞；真正的写库切换只走显式按钮 |
| R3 | `MessageBubble` 80% 最大宽度在缩进后过窄 | 在 `HistoryBranchRow` 容器上覆盖 `max-width` 变量，不修改 `MessageBubble` 样式 |
| R4 | 孤儿 / 环数据导致树异常 | 建树期兜底 + 告警日志；原始行模式用于排查 |
| R5 | 刷新与滚动恢复冲突 | 虚拟列表统一负责 clamp 恢复，业务层不直接操作 `scrollTop` |

明确不做：

- 不在历史页实现编辑 / 重新生成 / 重试 / 分叉。
- 不把 `tool` 行按 `callId` 归属到具体工具卡片（DTO 无 `tool_call_id`，需要 `contract:gen` 连锁改动，收益不足）。
- 不为历史页新增后端专用查询命令；`mode: "all"` 已满足。
- 不与 chat 的 `MessageList` 共用滚动、过滤、虚拟化逻辑。
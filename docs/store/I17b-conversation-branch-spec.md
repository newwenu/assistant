# I17b — 对话分支（Conversation Branch）规格说明

> **前置依赖**：I17（Chat 重发/重试恢复体系重构）必须完成。I17b 在 I17 的 `resendFrom` 原语 + `TruncationStrategy` 策略接口基础上扩展。

## 1. 目标

将编辑/重试/重新生成操作从**破坏性截断**改为**非破坏性分支**，使用户可以：
- 编辑 user 消息后，**原始 assistant 回复保留**，可随时切换回来
- 重新生成 assistant 消息后，**原始回复保留**，形成多个候选回复
- 在同一对话中探索不同方向，而不丢失任何已有内容

## 2. 现状分析

### 2.1 当前行为：截断式（破坏性）

所有重发操作（`retryFromError` / `editMessage` / `regenerateMessage`）都走 `resendFrom` 原语的 `{ kind: "truncate" }` 策略，**永久删除**分支点之后的所有消息：

```
编辑前：
  [U1] → [A1] → [U2] → [A2]

编辑 U2 后：
  [U1] → [A1] → [U2'] → [A2']    ← A2 永久丢失
```

### 2.2 参考项目对比

| 项目 | 分支模型 | 实现状态 | 分支上限 | 事件通知 |
|------|---------|---------|---------|---------|
| **ChatGPT** | Tree Branch + `< 1/3 >` 选择器 | ✅ 完整实现 | 20/parent，达到后禁止新建+提示Fork | 后端返回完整 snapshot |
| **OpenHuman (assistant-ui)** | Tree Branch（`BranchPickerPrimitive` 框架） | 🟡 框架就位，`setMessages` 是空操作 | — | — |
| **OpenBitFun** | Session Fork（复制到新会话） | ✅ 已实现 `forkChatSession` | 无（Fork 本身就是降级） | 后端返回新 session ID |
| **我们** | 无 | ❌ `parentId` 字段存在但未使用 | — | — |

> 详细分析见 [2.5 参考项目分支实现详析](#25-参考项目分支实现详析)

### 2.3 我们已有的基础设施

| 基础设施 | 位置 | 状态 |
|---------|------|------|
| `parent_id` DB 列 | `db.rs` messages 表 | ✅ 存在，`TEXT` 类型，可为 NULL |
| `parentId` Rust 字段 | `MessageRow.parent_id: Option<String>` | ✅ 存在 |
| `parentId` TS 字段 | `Message.parentId?: string \| null` | ✅ 存在 |
| `parentId` 协议契约 | DTO + 前端类型 | ✅ 存在 |
| `parentId` 业务逻辑使用 | — | ❌ **从未使用** |

**缺失**：
- `sibling_index`：同一 `parentId` 下的兄弟排序
- `active_branch_id`：对话当前活跃分支指针
- `BranchPicker` UI 组件
- 消息树渲染逻辑（`MessageList` 当前假设线性数组）

## 2.5 参考项目分支实现详析

### 2.5.1 ChatGPT（Tree Branch + `< 1/3 >` 选择器）

**模型**：原地消息树分支，每条消息可有多个兄弟（sibling），用户通过 `< 1/3 >` 导航器在兄弟间切换。

**关键设计**：
- 消息表存储 `parent_message_id`，同一 parent 下的兄弟按创建时间排序
- `conversation` 表存储 `current_leaf_message_id` 指向活跃分支末端
- 编辑/重新生成时**不删除**原消息，而是插入新兄弟 + 更新 leaf 指针
- `GET /conversation/{id}` 只返回活跃分支的线性消息序列（从 root 到 leaf 回溯）
- 前端 `BranchPicker` 组件：`<ChevronLeft /> 1/3 <ChevronRight />`，单分支时隐藏
- **分支上限**：同一 parent 下最多 20 个兄弟，达到后禁止新建分支，提示 Fork 到新对话

### 2.5.2 OpenHuman / assistant-ui（BranchPickerPrimitive 框架）

**模型**：assistant-ui 框架提供 `BranchPickerPrimitive` 组件原语，但实际分支逻辑由宿主应用实现。

**关键代码**（[thread.tsx:L1839-1872](file:///g:/codeversion/assistant/references/openhuman/app/src/components/assistant-ui/thread.tsx#L1839-L1872)）：
```tsx
const BranchPicker: FC<BranchPickerPrimitive.Root.Props> = ({ className, ...rest }) => {
  const { canSwitchToBranch } = useAuiEditCapabilities();
  if (!canSwitchToBranch) return null;  // 无分支能力时隐藏

  return (
    <BranchPickerPrimitive.Root hideWhenSingleBranch className={...} {...rest}>
      <BranchPickerPrimitive.Previous asChild>
        <TooltipIconButton tooltip="Previous"><ChevronLeftIcon /></TooltipIconButton>
      </BranchPickerPrimitive.Previous>
      <span className="aui-branch-picker-state font-medium">
        <BranchPickerPrimitive.Number /> / <BranchPickerPrimitive.Count />
      </span>
      <BranchPickerPrimitive.Next asChild>
        <TooltipIconButton tooltip="Next"><ChevronRightIcon /></TooltipIconButton>
      </BranchPickerPrimitive.Next>
    </BranchPickerPrimitive.Root>
  );
};
```

**当前状态**：`setMessages` 是空操作（no-op），`canSwitchToBranch` 始终为 false，`BranchPicker` 不渲染。框架就位但分支逻辑未实现。

**对我们的启示**：`@assistant-ui/react` 的 `hideWhenSingleBranch` 模式值得借鉴——单分支时 DOM 中不存在选择器，避免无意义渲染。

### 2.5.3 OpenBitFun（Session Fork 模型）

**模型**：不在原地分支，而是将对话**复制到新会话**（fork）。用户在某个 turn 点 fork 后，新会话从该 turn 开始，原会话不受影响。

**关键代码**（[SessionModule.ts:L774-823](file:///g:/codeversion/assistant/references/OpenBitFun/src/web-ui/src/flow_chat/services/flow-chat-manager/SessionModule.ts#L774-L823)）：
```typescript
export async function forkChatSession(
  context: FlowChatContext,
  sourceSessionId: string,
  sourceTurnId: string
): Promise<string> {
  // 后端 fork_session：复制 sourceTurnId 及之前的消息到新会话
  const response = await sessionAPI.forkSession(
    sourceSessionId, sourceTurnId, workspaceId);
  // 前端创建新 session 并切换
  context.flowChatStore.createSession(response.sessionId, { ... });
  context.flowChatStore.switchSession(response.sessionId);
}
```

**对我们的启示**：Fork 模型适合"分支过多时降级"场景——当同一 parent 下兄弟数接近 `MAX_SIBLINGS` 时，可以建议用户 fork 到新对话，避免单对话消息树过于庞大。

### 2.5.4 参考项目对比总结

| 项目 | 分支模型 | 事件通知 | 分支上限 | 降级策略 |
|------|---------|---------|---------|---------|
| **ChatGPT** | 原地 Tree Branch | 后端返回完整 snapshot | 20/parent | 提示开启新对话 |
| **OpenHuman** | 框架就位，未实现 | — | — | — |
| **OpenBitFun** | Session Fork（复制到新会话） | 后端返回新 session ID | 无 | Fork 本身就是降级 |
| **我们（I17b）** | 原地 Tree Branch | `chat-messages-changed` 事件（见 3.7） | 20/parent（硬上限） | 达到上限禁止新建，提示 Fork 到新对话（见 3.8） |

## 3. 设计方案

### 3.1 数据模型：消息树

#### 3.1.1 DB Schema 变更

```sql
-- 新增列（ALTER TABLE，向后兼容）
ALTER TABLE messages ADD COLUMN sibling_index INTEGER NOT NULL DEFAULT 0;
ALTER TABLE conversations ADD COLUMN active_branch_id TEXT;

-- 索引：按 parentId 查找兄弟消息
CREATE INDEX IF NOT EXISTS idx_messages_parent_id
  ON messages(conversation_id, parent_id, sibling_index);
```

**`sibling_index`**：同一 `parentId` 下的兄弟序号。第一个分支为 0，后续递增。

**`active_branch_id`**：对话当前活跃分支的 leaf 消息 ID。用于 `get_conversation_detail` 时确定返回哪条分支链。

#### 3.1.2 Rust Struct 变更

```rust
// conversations.rs
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MessageRow {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub tokens: Option<i64>,
    pub timestamp: i64,
    pub parent_id: Option<String>,
    pub sibling_index: i64,         // ← 新增
    pub is_pinned: bool,
    pub reasoning: Option<String>,
    pub tool_calls_json: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConversationRow {
    // ... 现有字段 ...
    pub active_branch_id: Option<String>,  // ← 新增
}
```

#### 3.1.3 TypeScript Interface 变更

```typescript
// chatTypes.ts
export interface Message {
  id: string;
  conversationId: string;
  role: string;
  content: string;
  reasoning?: string;
  toolCalls?: ToolCall[];
  tokens: number | null;
  timestamp: number;
  error?: ChatError | null;
  parentId?: string | null;
  siblingIndex: number;           // ← 新增
  siblingCount?: number;          // ← 新增（仅 UI 用，不持久化）
  isPinned?: boolean;
}

export interface Conversation {
  // ... 现有字段 ...
  activeBranchId?: string | null; // ← 新增
}
```

### 3.2 消息树结构

#### 3.2.1 树的构建规则

```
对话消息树示例：

root (虚拟根节点)
├── [U1] parentId=null, siblingIndex=0
│   └── [A1] parentId=U1.id, siblingIndex=0
│       └── [U2] parentId=A1.id, siblingIndex=0
│           ├── [A2] parentId=U2.id, siblingIndex=0    ← 原始回复
│           ├── [A2'] parentId=U2.id, siblingIndex=1   ← 重新生成
│           └── [A2''] parentId=U2.id, siblingIndex=2  ← 再次重新生成
│               └── [U3] parentId=A2''.id, siblingIndex=0
│                   └── [A3] parentId=U3.id, siblingIndex=0
└── [U1'] parentId=null, siblingIndex=1                ← 编辑 U1
    └── [A1'] parentId=U1'.id, siblingIndex=0
```

#### 3.2.2 活跃分支

对话的**活跃分支**是从 root 到 `activeBranchId` 指定的 leaf 消息的唯一路径。`get_conversation_detail` 返回的 `messages` 数组是活跃分支上的消息序列（线性），保持向后兼容。

**`activeBranchId` 更新时机**：
- 新消息追加到对话末尾 → 指向新消息
- 用户切换分支 → 指向目标分支的 leaf 消息
- 编辑/重试创建新分支 → 指向新分支的 leaf 消息

### 3.3 后端 API 变更

#### 3.3.1 `get_conversation_detail` — 返回活跃分支

```rust
// 改前：返回所有消息（线性）
// 改后：返回活跃分支上的消息序列

pub fn get_conversation_detail(conn: &Connection, id: &str) -> Result<ConversationDetail> {
    let conv = get_conversation(conn, id)?;

    // 从 active_branch_id 回溯到 root，构建活跃分支
    let active_messages = if let Some(leaf_id) = conv.active_branch_id {
        trace_active_branch(conn, id, &leaf_id)?
    } else {
        // 无 active_branch_id（旧数据），按 timestamp 排序返回全部
        get_all_messages(conn, id)?
    };

    Ok(ConversationDetail {
        conversation: conv,
        messages: active_messages,
    })
}
```

**`trace_active_branch`**：从 leaf 消息沿 `parentId` 回溯到 root，返回有序消息序列。

#### 3.3.2 `get_message_siblings` — 新增

```rust
pub fn get_message_siblings(
    conn: &Connection,
    message_id: &str,
) -> Result<Vec<SiblingInfo>> {
    // 查找同 parentId 的所有兄弟消息
    let parent_id: Option<String> = conn.query_row(
        "SELECT parent_id FROM messages WHERE id = ?1",
        params![message_id],
        |row| row.get(0),
    ).optional()?.flatten();

    let siblings = if let Some(pid) = parent_id {
        query_as::<SiblingInfo>(
            conn,
            "SELECT id, sibling_index, content_preview FROM messages
             WHERE conversation_id = (SELECT conversation_id FROM messages WHERE id = ?1)
               AND parent_id = ?2
             ORDER BY sibling_index",
            params![message_id, pid],
        )?
    } else {
        // parentId = null 的顶层消息
        query_as::<SiblingInfo>(
            conn,
            "SELECT id, sibling_index, content_preview FROM messages
             WHERE conversation_id = (SELECT conversation_id FROM messages WHERE id = ?1)
               AND parent_id IS NULL
             ORDER BY sibling_index",
            params![message_id],
        )?
    };

    Ok(siblings)
}

pub struct SiblingInfo {
    pub id: String,
    pub sibling_index: i64,
    pub content_preview: String,  // 前 100 字符，用于 UI tooltip
}
```

#### 3.3.3 `switch_branch` — 新增

```rust
pub fn switch_branch(
    conn: &Connection,
    conversation_id: &str,
    target_message_id: &str,
) -> Result<()> {
    // 验证 target_message_id 属于该对话
    let exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE id = ?1 AND conversation_id = ?2",
        params![target_message_id, conversation_id],
        |row| row.get::<_, i64>(0).map(|c| c > 0),
    )?;

    if !exists {
        return Err(Error::MessageNotFound(target_message_id.to_string()));
    }

    // 更新 active_branch_id
    conn.execute(
        "UPDATE conversations SET active_branch_id = ?1 WHERE id = ?2",
        params![target_message_id, conversation_id],
    )?;

    Ok(())
}
```

#### 3.3.4 `create_branch` — 新增（替代 `editMessage` / `regenerateMessage` 的截断逻辑）

```rust
pub fn create_branch(
    conn: &Connection,
    conversation_id: &str,
    branch_point_message_id: &str,
    new_role: &str,
    new_content: &str,
) -> Result<MessageRow> {
    // 事务包裹：防止并发 create_branch 导致 sibling_index 重复
    let tx = conn.unchecked_transaction()?;

    // 1. 找到分支点消息
    let branch_point = get_message(&tx, branch_point_message_id)?;

    // 1.5 硬上限检查：达到 MAX_SIBLINGS 后拒绝新建分支（见 3.8.1）
    let sibling_count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM messages
         WHERE conversation_id = ?1 AND parent_id = ?2",
        params![conversation_id, branch_point.parent_id],
        |row| row.get(0),
    )?;
    if sibling_count >= MAX_SIBLINGS {
        tx.rollback()?;
        return Err(Error::MaxSiblingsExceeded(...));
    }

    // 2. 计算新消息的 siblingIndex（事务内 MAX+1，原子性保证）
    let max_sibling_index: i64 = tx.query_row(
        "SELECT COALESCE(MAX(sibling_index), -1) FROM messages
         WHERE conversation_id = ?1 AND parent_id = ?2",
        params![conversation_id, branch_point.parent_id],
        |row| row.get(0),
    ).unwrap_or(-1);

    // 3. 插入新消息
    let new_id = uuid();
    let now = now_epoch_ms();
    tx.execute(
        "INSERT INTO messages (id, conversation_id, role, content, timestamp, parent_id, sibling_index)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            new_id,
            conversation_id,
            new_role,
            new_content,
            now,
            branch_point.parent_id,  // 同一 parent
            max_sibling_index + 1,
        ],
    )?;

    // 4. 更新 active_branch_id 指向新消息
    tx.execute(
        "UPDATE conversations SET active_branch_id = ?1 WHERE id = ?2",
        params![new_id, conversation_id],
    )?;

    tx.commit()?;
    get_message(conn, &new_id)
}
```

**关键语义**：`create_branch` **不删除任何消息**，只是在同一 `parentId` 下追加新兄弟，并将活跃分支切换到新兄弟。**事务包裹**保证 `sibling_index` 的 `MAX()+1` 计算与 INSERT 原子执行，防止并发分支创建导致重复序号。**硬上限** `MAX_SIBLINGS = 20` 保证分支数不会无限增长，达到上限后返回 `MaxSiblingsExceeded` 错误。

### 3.4 前端变更

#### 3.4.1 `TruncationStrategy.branch` 实现

I17 预留的 `TruncationStrategy` 扩展点：

```typescript
// chatStore.ts — resendFrom 的 branch 策略实现
async function resendFrom(
  pivotTimestamp: number,
  contentToSend: string,
  strategy: TruncationStrategy = { kind: "truncate" },
): Promise<void> {
  await orchestrator.ensureIdle();

  if (strategy.kind === "truncate") {
    set(state => ({
      messages: state.messages.filter(m => m.timestamp < pivotTimestamp),
    }));
  } else if (strategy.kind === "branch") {
    // I17b 实现：不删除消息，而是切换到新分支
    // 新分支的消息会在 sendMessage 的流式回调中追加
    // 此处只需标记"下一个 sendMessage 应创建分支"
    set({ pendingBranchPoint: strategy.branchPointMessageId });
  }

  void get().sendMessage(contentToSend);
}
```

#### 3.4.2 `sendMessage` 适配分支

```typescript
// chatStore.ts — sendMessage 追加消息时设置 parentId
sendMessage: async (content: string) => {
  const state = get();
  const pendingBranchPoint = state.pendingBranchPoint;

  // 确定新 user 消息的 parentId
  let parentId: string | null = null;
  let siblingIndex = 0;

  if (pendingBranchPoint) {
    // 分支模式：新 user 消息的 parentId = 分支点消息的 parentId
    const branchPoint = state.messages.find(m => m.id === pendingBranchPoint);
    parentId = branchPoint?.parentId ?? null;
    // siblingIndex 由后端 create_branch 计算
    set({ pendingBranchPoint: null });
  } else {
    // 正常模式：新 user 消息的 parentId = 最后一条消息的 id
    const lastMsg = state.messages[state.messages.length - 1];
    parentId = lastMsg?.id ?? null;
  }

  // ... 调用后端 send_chat_message（传入 parentId）...
},
```

#### 3.4.3 `retryFromError` 改用分支策略

```typescript
retryFromError: async (messageId: string) => {
  const state = get();
  const msg = state.messages.find((m) => m.id === messageId);
  if (!msg || msg.role !== "assistant") return;

  const userMsg = [...state.messages]
    .filter((m) => m.role === "user" && m.timestamp < msg.timestamp)
    .pop();
  if (!userMsg) return;

  // 后端创建分支（不删除）
  await chatService.createBranch(
    state.currentConversationId!,
    messageId,    // branchPoint：失败的 assistant 消息
    "assistant",  // 新消息角色（将由流式填充）
    "",           // 内容由流式填充
  );

  // 基于原语重发，使用分支策略
  await resendFrom(msg.timestamp, userMsg.content, {
    kind: "branch",
    branchPointMessageId: messageId,
  });
},
```

#### 3.4.4 `editMessage` 改用分支策略

```typescript
editMessage: async (messageId: string, newContent: string) => {
  const state = get();
  const msg = state.messages.find((m) => m.id === messageId);
  if (!msg) return;

  // 后端创建分支（不删除，不截断）
  await chatService.createBranch(
    state.currentConversationId!,
    messageId,    // branchPoint：被编辑的消息
    msg.role,
    newContent,
  );

  // 前端不修改原消息，而是等待新分支消息通过流式到达

  if (msg.role === "user") {
    await resendFrom(msg.timestamp, newContent, {
      kind: "branch",
      branchPointMessageId: messageId,
    });
  }
},
```

#### 3.4.5 `switchBranch` — 新增

```typescript
switchBranch: async (messageId: string) => {
  const state = get();
  if (!state.currentConversationId) return;

  // 后端切换活跃分支
  await chatService.switchBranch(
    state.currentConversationId,
    messageId,
  );

  // 重新加载对话详情（返回新活跃分支的消息序列）
  const detail = await chatService.getConversationDetail(
    state.currentConversationId,
  );
  set({
    messages: detail.messages,
    activeBranchId: messageId,
  });
},
```

### 3.5 UI 组件

#### 3.5.1 `BranchPicker` 组件

参考 OpenHuman 项目的 `@assistant-ui/react` 库中 `BranchPickerPrimitive` 的 API 设计（`hideWhenSingleBranch`、`Number`/`Count` 槽位等），但自研实现——我们未引入该第三方库：

```tsx
interface BranchPickerProps {
  currentSiblingIndex: number;
  siblingCount: number;
  maxSiblings: number;           // MAX_SIBLINGS = 20
  onSwitch: (direction: "prev" | "next") => void;
  onFork?: () => void;           // Fork 到新对话回调（达到上限时显示）
}

const BRANCH_WARNING_THRESHOLD = 15;

const BranchPicker: FC<BranchPickerProps> = ({
  currentSiblingIndex,
  siblingCount,
  maxSiblings,
  onSwitch,
  onFork,
}) => {
  if (siblingCount <= 1) return null;

  const isNearLimit = siblingCount >= BRANCH_WARNING_THRESHOLD;
  const isAtLimit = siblingCount >= maxSiblings;

  return (
    <div className={cx(css.root, isNearLimit && css.warning)}>
      <button
        disabled={currentSiblingIndex === 0}
        onClick={() => onSwitch("prev")}
        aria-label={t("chat.branch.previous")}
      >
        <ChevronLeftIcon />
      </button>
      <span className={css.label}>
        {currentSiblingIndex + 1} / {siblingCount}
      </span>
      <button
        disabled={currentSiblingIndex === siblingCount - 1}
        onClick={() => onSwitch("next")}
        aria-label={t("chat.branch.next")}
      >
        <ChevronRightIcon />
      </button>
      {/* 接近上限（15-19）：始终可见的文字提醒 */}
      {isNearLimit && !isAtLimit && (
        <span className={css.warningText}>
          {t("chat.branch.nearLimitHint", { max: maxSiblings })}
        </span>
      )}
      {/* 达到上限（20）：Fork 按钮 */}
      {isAtLimit && onFork && (
        <button
          className={css.forkBtn}
          onClick={onFork}
          aria-label={t("chat.branch.forkAction")}
        >
          <ForkIcon />
        </button>
      )}
    </div>
  );
};
// nearLimitHint: "分支较多，可 Fork 到新对话"
```

**视觉表现**：

| siblingCount | BranchPicker 外观 | MessageActions |
|---|---|---|
| 1 | 不渲染 | Fork 按钮可用 |
| 2-14 | `◁ 1/3 ▷` 正常样式 | Fork 按钮可用 |
| 15-19 | `◁ 1/15 ▷` **warning 样式**（橙色边框）+ **始终可见文字**"分支较多，可 Fork 到新对话" | Fork 按钮可用 |
| 20 | `◁ 1/20 ▷` **warning 样式** + **Fork 按钮**（点击弹出 `ForkDialog`） | 编辑/重新生成 **disabled**；Fork 按钮可用 |

#### 3.5.2 `MessageBubble` 集成

```tsx
// MessageBubble.tsx — 在消息底部添加 BranchPicker
const MAX_SIBLINGS = 20;

<div className={css.footer}>
  <BranchPicker
    currentSiblingIndex={message.siblingIndex}
    siblingCount={message.siblingCount ?? 1}
    maxSiblings={MAX_SIBLINGS}
    onSwitch={(direction) => {
      const siblings = getSiblings(message.id);
      const target = direction === "next"
        ? siblings[message.siblingIndex + 1]
        : siblings[message.siblingIndex - 1];
      if (target) onSwitchBranch(target.id);
    }}
    onFork={() => setShowForkDialog(true)}
  />
  {/* MessageActions — isBranchAtLimit 传入见 3.8.5 */}
</div>
```

#### 3.5.3 `MessageList` 适配

`MessageList` 当前渲染 `messages: Message[]`（线性数组）。分支模式下，`messages` 仍然是活跃分支的线性序列（由 `get_conversation_detail` 返回），**无需改变渲染逻辑**。

`siblingCount` 信息需要额外获取。两种方案：

| 方案 | 优点 | 缺点 |
|------|------|------|
| **A：`get_conversation_detail` 附带** | 一次请求获取全部 | 响应体增大，每条消息多一个字段 |
| **B：按需 `get_message_siblings`** | 按需加载 | 切换分支时额外请求 |

**选择方案 A**：`get_conversation_detail` 返回的每条消息附带 `siblingCount`，因为分支数通常很少（1-5），额外数据量可忽略。

```rust
// ConversationDetail 返回时计算 siblingCount
pub struct MessageWithSiblings {
    pub message: MessageRow,
    pub sibling_count: i64,
}
```

### 3.6 `resendFrom` 原语完整签名（I17 + I17b）

I17 中 `resendFrom` 签名为 `(pivotTimestamp, contentToSend)`，I17b 扩展为接受 `TruncationStrategy` 参数：

```typescript
type TruncationStrategy =
  | { kind: "truncate" }                                    // I17：删除后续消息
  | { kind: "branch"; branchPointMessageId: string };       // I17b：创建新分支

async function resendFrom(
  pivotTimestamp: number,
  contentToSend: string,
  strategy: TruncationStrategy = { kind: "truncate" },
): Promise<void> {
  await orchestrator.ensureIdle();

  if (strategy.kind === "truncate") {
    const snapshot = get().messages;
    set(state => ({
      messages: state.messages.filter(m => m.timestamp < pivotTimestamp),
    }));
    try {
      void get().sendMessage(contentToSend);
    } catch (e) {
      set({ messages: snapshot });  // 回滚
    }
  } else {
    set({ pendingBranchPoint: strategy.branchPointMessageId });
    void get().sendMessage(contentToSend);
  }
}
```

### 3.7 通用消息变更事件系统（`chat-messages-changed`）

#### 3.7.1 问题：当前"invoke + 乐观更新"模式在分支下不可行

I17 阶段的 `editMessage`（截断模式）前端可以乐观推算——删后续消息 + 改 content，逻辑简单。但 I17b 的分支操作完全不同：

| 操作 | 后端效果 | 前端能否乐观推算？ |
|------|---------|------------------|
| `create_branch` | 插入新消息（后端生成 id、siblingIndex）+ 更新 active_branch_id | ❌ 无法推算新消息 id 和 siblingIndex（事务内 MAX+1） |
| `switch_branch` | 更新 active_branch_id + 返回新分支消息序列 | ❌ 无法推算新分支的消息序列（需 DB 查询） |
| `edit_message`（分支模式） | 创建分支 + 新消息 | ❌ 同 create_branch |
| `retryFromError`（分支模式） | 创建分支 + 新消息 | ❌ 同 create_branch |

此外，I17 spec 2.6 已指出 `chat-regenerate` 事件前端未监听是风险。I17b 如果继续这种模式，会有更多"发了事件但没人听"或"需要事件但没有发"的情况。

#### 3.7.2 设计：统一消息变更事件

不按操作类型发不同事件，而是**按变更语义发统一事件**，事件携带完整 snapshot，前端监听后直接替换本地 state：

```typescript
// 前端类型
interface ChatMessagesChangedPayload {
  conversationId: string;
  changeKind: MessageChangeKind;
  messages: Message[];              // 活跃分支消息序列（完整 snapshot）
  activeBranchId: string | null;
}

type MessageChangeKind =
  | "branch_created"       // create_branch：新分支已创建
  | "branch_switched"      // switch_branch：活跃分支已切换
  | "branch_forked"        // fork_to_new_conversation：分支已 Fork 到新对话
  | "message_edited"       // edit_message：消息内容已修改（分支模式下可能同时创建分支）
  | "message_deleted"      // delete_message：消息已删除
  | "messages_regenerated"; // regenerate：消息已重新生成（截断模式）
```

#### 3.7.3 后端实现

```rust
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessagesChangedPayloadDto {
    pub conversation_id: String,
    pub change_kind: String,
    pub messages: Vec<MessageWithSiblingsDto>,
    pub active_branch_id: Option<String>,
}

// 在每个变更操作末尾 emit
pub fn create_branch(...) -> Result<MessageRow> {
    // ... 事务逻辑 ...
    tx.commit()?;

    let snapshot = get_active_branch_snapshot(conn, conversation_id)?;
    let _ = app.emit("chat-messages-changed", ChatMessagesChangedPayloadDto {
        conversation_id: conversation_id.to_string(),
        change_kind: "branch_created".to_string(),
        messages: snapshot.messages,
        active_branch_id: snapshot.active_branch_id,
    });

    get_message(conn, &new_id)
}
```

**emit 时机**：`create_branch` / `switch_branch` / `fork_to_new_conversation` / `edit_message`（分支模式）/ `regenerate_message` / `delete_message` 的**事务 commit 之后**。

#### 3.7.4 前端监听

```typescript
// chatDeltaEvents.ts 新增
track(
  listen<ChatMessagesChangedPayload>("chat-messages-changed", (event) => {
    if (disposed) return;
    const payload = event.payload;
    const store = useChatStore.getState();
    if (store.currentConversationId === payload.conversationId) {
      useChatStore.setState({
        messages: payload.messages,
        activeBranchId: payload.activeBranchId,
      });
    }
  }),
);
```

#### 3.7.5 双模式更新策略（Optimistic + Event Driven）

不是所有操作都需要等事件。截断模式（I17 行为）可保留乐观更新以获得即时反馈；分支模式必须等事件：

```typescript
type UpdateStrategy =
  | { kind: "optimistic"; rollback: () => void }   // 截断模式：乐观更新 + 事件确认/回滚
  | { kind: "event_driven" };                        // 分支模式：纯事件驱动

// resendFrom 内部选择策略
async function resendFrom(pivotTimestamp, contentToSend, strategy) {
  await orchestrator.ensureIdle();

  if (strategy.kind === "truncate") {
    // 截断模式：乐观更新（I17 行为不变）
    const snapshot = get().messages;
    set(state => ({ messages: state.messages.filter(m => m.timestamp < pivotTimestamp) }));
    try {
      void get().sendMessage(contentToSend);
    } catch (e) {
      set({ messages: snapshot });  // 回滚
    }
  } else {
    // 分支模式：invoke create_branch → 等待 chat-messages-changed 事件 → 事件到达后 set snapshot
    // 前端不做乐观更新，避免与后端不一致
    await chatService.createBranch(...);
    // 事件监听器自动更新 state
  }
}
```

#### 3.7.6 对现有操作的影响

| 操作 | 改前（乐观更新） | 改后（事件驱动） |
|------|----------------|----------------|
| `editMessage`（截断） | invoke + 自己 set content + resendFrom | **不变**（保留乐观更新） |
| `editMessage`（分支） | invoke + 自己 set content + resendFrom | invoke create_branch → 等事件 → set snapshot |
| `retryFromError`（分支） | invoke regenerate + resendFrom | invoke create_branch → 等事件 → set snapshot |
| `regenerateMessage`（分支） | invoke + 自己 filter + sendMessage | invoke create_branch → 等事件 → set snapshot |
| `switchBranch` | invoke + getConversationDetail | invoke switch_branch → 等事件 → set snapshot（**无需二次请求**） |
| `deleteMessage` | invoke + 自己 filter | invoke delete → 等事件 → set snapshot |

**关键收益**：
1. `switchBranch` 不再需要二次 `getConversationDetail` 请求，事件自带 snapshot
2. 解决 `chat-regenerate` 未监听的历史问题（统一为 `chat-messages-changed`）
3. 分支操作数据一致性由后端事件保证，前端无法推算出错误状态

### 3.8 分支上限 `MAX_SIBLINGS` 与 Fork 到新对话

#### 3.8.1 `MAX_SIBLINGS = 20`（硬上限）

同一 `parentId` 下最多 **20 个兄弟消息**，这是硬上限。达到后**禁止新建分支**，不执行任何清理操作。

```rust
const MAX_SIBLINGS: i64 = 20;

pub fn create_branch(...) -> Result<MessageRow> {
    let tx = conn.unchecked_transaction()?;

    // 硬上限检查：达到 MAX_SIBLINGS 后直接拒绝
    let sibling_count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM messages
         WHERE conversation_id = ?1 AND parent_id = ?2",
        params![conversation_id, branch_point.parent_id],
        |row| row.get(0),
    )?;

    if sibling_count >= MAX_SIBLINGS {
        tx.rollback()?;
        return Err(Error::MaxSiblingsExceeded(MaxSiblingsExceededError {
            conversation_id: conversation_id.to_string(),
            parent_id: branch_point.parent_id.clone(),
            current_count: sibling_count,
            max_siblings: MAX_SIBLINGS,
        }));
    }

    // ... 正常 create_branch 逻辑 ...
}
```

**设计原则**：不做自动清理（不删除用户数据），达到上限直接拒绝。用户可选择 Fork 到新对话来继续探索。

#### 3.8.2 分支数接近上限时的提醒

当同一 `parentId` 下兄弟数达到 **15** 时，前端在 `BranchPicker` 旁显示 inline 提醒"分支较多，可 Fork 到新对话"；达到 **20** 时，`create_branch` 返回 `MaxSiblingsExceeded` 错误，前端弹出 Fork 对话框。

```typescript
const BRANCH_WARNING_THRESHOLD = 15;
const MAX_SIBLINGS = 20;

// chatStore.ts — createBranch 处理
async function createBranch(...) {
  const result = await chatService.createBranch(...);

  if (result.warning === "approaching_max_siblings") {
    set({ branchWarning: {
      messageId: result.branchPointMessageId,
      siblingCount: result.siblingCount,
      maxSiblings: MAX_SIBLINGS,
    }});
  }

  if (result.error?.kind === "max_siblings_exceeded") {
    set({ showForkDialog: {
      conversationId: ...,
      branchPointMessageId: ...,
      siblingCount: result.error.currentCount,
    }});
  }
}
```

#### 3.8.3 Fork 到新对话（`fork_to_new_conversation`）

参考 OpenBitFun 的 `forkChatSession` 模型，将当前活跃分支**复制**到新对话。原对话不受影响，分支数据完整保留。

```rust
pub fn fork_to_new_conversation(
    conn: &Connection,
    source_conversation_id: &str,
    branch_leaf_message_id: &str,
) -> Result<ConversationRow> {
    // 1. 创建新对话
    let new_conv_id = uuid();
    conn.execute(
        "INSERT INTO conversations (id, title, created_at, updated_at)
         SELECT ?1, title || ' (分支)', datetime('now'), datetime('now')
         FROM conversations WHERE id = ?2",
        params![new_conv_id, source_conversation_id],
    )?;

    // 2. 从 branch_leaf_message_id 回溯到 root，复制消息到新对话
    let branch_messages = trace_active_branch(conn, source_conversation_id, branch_leaf_message_id)?;

    // parentId 映射：旧 ID → 新 ID
    let mut id_map: HashMap<String, String> = HashMap::new();
    for msg in &branch_messages {
        let new_id = uuid();
        id_map.insert(msg.id.clone(), new_id.clone());
        let new_parent_id = msg.parent_id.as_ref().and_then(|pid| id_map.get(pid)).cloned();
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, timestamp, parent_id, sibling_index)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
            params![new_id, new_conv_id, msg.role, msg.content, msg.timestamp, new_parent_id],
        )?;
    }

    // 3. 新对话的 active_branch_id 指向最后一条消息
    let last_new_id = branch_messages.last().and_then(|m| id_map.get(&m.id)).cloned();
    conn.execute(
        "UPDATE conversations SET active_branch_id = ?1 WHERE id = ?2",
        params![last_new_id, new_conv_id],
    )?;

    // 注意：不删除原对话的任何消息，Fork 是纯复制操作
    get_conversation(conn, &new_conv_id)
}
```

**设计要点**：Fork 是**纯复制**，原对话完全不受影响。用户可以在新对话中继续探索，原对话的分支数据完整保留。`parentId` 通过 `id_map` 重新映射，确保新对话的消息树结构自洽。

#### 3.8.4 前端 Fork 对话框 UI

```tsx
interface ForkDialogProps {
  open: boolean;
  conversationId: string;
  branchPointMessageId: string;
  siblingCount: number;
  onFork: () => void;
  onCancel: () => void;
}

const ForkDialog: FC<ForkDialogProps> = ({ open, siblingCount, onFork, onCancel }) => (
  <Dialog open={open}>
    <DialogTitle>{t("chat.branch.forkTitle")}</DialogTitle>
    <DialogContent>
      <p>{t("chat.branch.forkMessage", { count: siblingCount, max: MAX_SIBLINGS })}</p>
      <p className={css.hint}>{t("chat.branch.forkHint")}</p>
    </DialogContent>
    <DialogActions>
      <Button onClick={onCancel}>{t("common.cancel")}</Button>
      <Button variant="primary" onClick={onFork}>{t("chat.branch.forkAction")}</Button>
    </DialogActions>
  </Dialog>
);
// forkTitle: "分支已达上限"
// forkMessage: "当前分支点已有 {count} 个分支（上限 {max}），无法再创建新分支。"
// forkHint: "您可以将当前分支 Fork 到新对话中继续探索，原对话不受影响。"
// forkAction: "Fork 到新对话"
```

#### 3.8.5 达到上限时禁用编辑/重新生成按钮

当某条消息的 `siblingCount >= MAX_SIBLINGS` 时，该消息的**编辑**（user 消息）和**重新生成**（assistant 消息）按钮应显示为 disabled 状态，hover 时提示原因。

当前 `MessageActions` 组件通过是否传入回调控制按钮显示/隐藏（`onEdit &&`、`onRegenerate &&`），没有 disabled 状态。I17b 需扩展接口：

```tsx
// MessageActions.tsx — 扩展 props
interface MessageActionsProps {
  role: string;
  isStreaming?: boolean;
  isBranchAtLimit?: boolean;     // ← 新增：该消息分支已达 MAX_SIBLINGS
  onCopy?: () => void;
  onDelete?: () => void;
  onEdit?: () => void;
  onRegenerate?: () => void;
  onFork?: () => void;           // ← 新增：Fork 到新对话
}

// 按钮顺序：复制 | 编辑(user) | 重新生成(assistant) | Fork | 删除

// 编辑按钮：isBranchAtLimit 时 disabled + tooltip
{isUser && onEdit && !isStreaming && (
  <button
    className={isBranchAtLimit ? css.btnDisabled : css.btn}
    onClick={isBranchAtLimit ? undefined : onEdit}
    disabled={isBranchAtLimit}
    title={isBranchAtLimit
      ? t("chat.branch.atLimitDisableEdit")
      : t("chat.message.edit")}
    type="button"
  >
    <EditOutlined style={{ fontSize: 12 }} />
  </button>
)}

// 重新生成按钮：同理
{isAssistant && onRegenerate && !isStreaming && (
  <button
    className={isBranchAtLimit ? css.btnDisabled : css.btn}
    onClick={isBranchAtLimit ? undefined : onRegenerate}
    disabled={isBranchAtLimit}
    title={isBranchAtLimit
      ? t("chat.branch.atLimitDisableRegenerate")
      : t("chat.message.regenerate")}
    type="button"
  >
    <RedoOutlined style={{ fontSize: 12 }} />
  </button>
)}

// Fork 按钮：常规操作，任何消息都可 Fork（不限上限状态）
{onFork && !isStreaming && (
  <button
    className={css.btn}
    onClick={onFork}
    title={t("chat.message.fork")}
    type="button"
  >
    <ForkOutlined style={{ fontSize: 12 }} />
  </button>
)}

// atLimitDisableEdit: "分支已达上限（20），无法编辑。可 Fork 到新对话继续探索。"
// atLimitDisableRegenerate: "分支已达上限（20），无法重新生成。可 Fork 到新对话继续探索。"
// fork: "Fork 到新对话"
```

**数据流**：`Message.siblingCount` → `MessageBubble` → `MessageActions.isBranchAtLimit`（`siblingCount >= MAX_SIBLINGS`）。无需 store 额外状态，每条消息自带分支数信息。

**为什么用 disabled 而非隐藏**：达到上限时用户需要明确知道"编辑/重新生成功能存在但受限"，而非突然找不到按钮而困惑。disabled + tooltip 是最清晰的表达。

**Fork 按钮的可见性**：Fork 是常规操作，任何消息（user/assistant）都可 Fork，不限于上限状态。达到上限时 BranchPicker 内也有 Fork 按钮（见 3.5.1），两处入口都触发同一个 `ForkDialog`。

### 3.9 `MessageTree` 辅助类

I17b 后消息从线性数组变为树结构，但 `get_conversation_detail` 仍返回线性序列（活跃分支）。前端需要能从线性序列 + `parentId`/`siblingIndex`/`siblingCount` 信息构建轻量树视图，用于 `BranchPicker` 的 sibling 导航。

将树操作逻辑从 `chatStore` 和 `MessageBubble` 中解耦，单一职责，可独立测试：

```typescript
// chat/core/messageTree.ts
export class MessageTree {
  private nodes: Map<string, MessageTreeNode>;

  static fromActiveBranch(messages: Message[]): MessageTree {
    const tree = new MessageTree();
    for (const msg of messages) {
      tree.nodes.set(msg.id, {
        message: msg,
        children: [],
        siblingIndex: msg.siblingIndex,
        siblingCount: msg.siblingCount ?? 1,
      });
    }
    // 构建 parent → children 映射
    for (const msg of messages) {
      if (msg.parentId && tree.nodes.has(msg.parentId)) {
        tree.nodes.get(msg.parentId)!.children.push(msg.id);
      }
    }
    return tree;
  }

  getSiblings(messageId: string): SiblingInfo[] | null {
    const node = this.nodes.get(messageId);
    if (!node || node.siblingCount <= 1) return null;
    return Array.from({ length: node.siblingCount }, (_, i) => ({
      index: i,
      isCurrent: i === node.siblingIndex,
    }));
  }

  getPathToRoot(messageId: string): Message[] {
    const path: Message[] = [];
    let current = this.nodes.get(messageId);
    while (current) {
      path.unshift(current.message);
      current = current.message.parentId
        ? this.nodes.get(current.message.parentId)
        : undefined;
    }
    return path;
  }

  getActiveLeafId(): string | null {
    for (const [id, node] of this.nodes) {
      if (node.children.length === 0) return id;
    }
    return null;
  }
}

interface MessageTreeNode {
  message: Message;
  children: string[];
  siblingIndex: number;
  siblingCount: number;
}

interface SiblingInfo {
  index: number;
  isCurrent: boolean;
}
```

**使用方式**：
```typescript
// chatStore.ts
const messageTree = useMemo(() => MessageTree.fromActiveBranch(messages), [messages]);

// MessageBubble.tsx
const siblings = messageTree.getSiblings(message.id);
```

### 3.10 操作序列号验证（防乱序）

前端可能在后端事务提交前就发起下一个操作。引入前端序列号检测乱序：

```typescript
// chatStore.ts
let operationSeq = 0;

async function createBranch(...) {
  const seq = ++operationSeq;
  await chatService.createBranch({ ..., _seq: seq });
  // chat-messages-changed 事件到达时验证 seq
}

// chatDeltaEvents.ts — 事件监听中验证
listen("chat-messages-changed", (event) => {
  const payload = event.payload;
  if (payload._seq !== undefined && payload._seq !== expectedSeq) {
    // 乱序：刷新整个对话
    chatStore.loadConversation(payload.conversationId);
    return;
  }
  // 正常更新
  useChatStore.setState({ messages: payload.messages, ... });
});
```

后端在 `chat-messages-changed` 事件中回传 `_seq`（可选字段，`#[serde(skip_serializing_if = "Option::is_none")]`），前端验证顺序。如果乱序，刷新整个对话。

## 4. 向后兼容

### 4.1 数据迁移

| 场景 | 处理 |
|------|------|
| 旧数据：`sibling_index` 不存在 | `ALTER TABLE ADD COLUMN sibling_index INTEGER NOT NULL DEFAULT 0`，所有旧消息为 0 |
| 旧数据：`active_branch_id` 不存在 | `ALTER TABLE ADD COLUMN active_branch_id TEXT`，默认 NULL → `get_conversation_detail` 按 timestamp 排序返回全部（旧行为） |
| 旧数据：`parent_id` 全为 NULL | 所有消息视为单分支线性序列，`siblingCount = 1`，`BranchPicker` 不显示 |
| 旧版前端 + 新版后端 | `siblingIndex` / `siblingCount` 被忽略，前端仍按线性渲染 |
| 新版前端 + 旧版后端 | `siblingIndex` 缺失时默认 0，`siblingCount` 缺失时默认 1，`BranchPicker` 不显示 |

### 4.2 功能降级

```typescript
// chatTypes.ts — 新字段有默认值
export interface Message {
  // ... 现有字段 ...
  siblingIndex: number;      // 默认 0
  siblingCount?: number;     // 默认 1（缺失时）
}

// MessageBubble.tsx — 安全降级
const effectiveSiblingCount = message.siblingCount ?? 1;
if (effectiveSiblingCount <= 1) return null;  // 不显示 BranchPicker
```

## 5. 实施步骤

### Step 1：后端 Schema 迁移

**目标**：`messages` 表新增 `sibling_index`，`conversations` 表新增 `active_branch_id`。

**变更**：
- `db.rs`：`CREATE TABLE` 语句新增列 + `ALTER TABLE` 迁移
- `conversations.rs`：`MessageRow` 新增 `sibling_index`，`ConversationRow` 新增 `active_branch_id`
- `dto.rs`：`MessageDto` 新增 `sibling_index`，`ConversationDto` 新增 `active_branch_id`，`#[serde(default)]`

**验证**：旧 DB 升级后，所有消息 `sibling_index = 0`，`active_branch_id = NULL`，行为不变。

### Step 2：后端 `trace_active_branch` + `get_conversation_detail` 适配

**目标**：`get_conversation_detail` 返回活跃分支消息序列 + 每条消息的 `siblingCount`。

**变更**：
- `conversations.rs`：新增 `trace_active_branch()` 函数
- `conversations.rs`：`get_conversation_detail` 优先按 `active_branch_id` 回溯，否则按 timestamp 排序
- `conversations.rs`：每条消息附带 `siblingCount`（`SELECT COUNT(*) FROM messages WHERE parent_id = ?`）

**验证**：`active_branch_id = NULL` 时行为与 I17 完全相同。

### Step 3：后端 `create_branch` + `switch_branch` + `get_message_siblings`

**目标**：新增 3 个 API。

**变更**：
- `conversations.rs`：新增 `create_branch` / `switch_branch` / `get_message_siblings`
- `conversation.rs`（cmd）：新增 3 个 Tauri command
- `dto.rs`：新增 `SiblingInfoDto`

**验证**：`create_branch` 不删除任何消息；`switch_branch` 切换后 `get_conversation_detail` 返回新分支。

### Step 4：前端类型 + `chatService` 适配

**目标**：`Message` 接口新增 `siblingIndex` / `siblingCount`；`chatService` 新增 3 个方法。

**变更**：
- `chatTypes.ts`：`Message` 新增字段
- `chatService.ts`：新增 `createBranch` / `switchBranch` / `getMessageSiblings`

**验证**：TypeScript 编译通过；旧数据缺失字段时默认值正确。

### Step 5：前端 `resendFrom` 分支策略实现

**目标**：`TruncationStrategy.branch` 策略生效。

**变更**：
- `chatStore.ts`：`resendFrom` 的 `strategy.kind === "branch"` 分支实现
- `chatStore.ts`：新增 `pendingBranchPoint` 状态
- `chatStore.ts`：`sendMessage` 适配 `pendingBranchPoint`

**验证**：`retryFromError` / `editMessage` 传入 `{ kind: "branch" }` 后，不删除消息，新回复作为兄弟追加。

### Step 6：前端 `retryFromError` + `editMessage` 改用分支策略

**目标**：默认使用分支策略而非截断。

**变更**：
- `chatStore.ts`：`retryFromError` 传入 `{ kind: "branch", branchPointMessageId: messageId }`
- `chatStore.ts`：`editMessage` 传入 `{ kind: "branch", branchPointMessageId: messageId }`

**验证**：编辑/重试后原始回复保留，`BranchPicker` 显示 `1/2`。

### Step 7：前端 `BranchPicker` 组件 + `MessageBubble` 集成

**目标**：消息底部显示分支选择器。

**变更**：
- 新增 `BranchPicker.tsx`
- `MessageBubble.tsx`：集成 `BranchPicker`
- `MessageList.tsx`：传递 `siblingCount` + `onSwitchBranch`

**验证**：多分支消息显示 `< 1/3 >`，点击箭头切换分支。

### Step 8：前端 `switchBranch` + 活跃分支管理

**目标**：切换分支后重新加载消息序列。

**变更**：
- `chatStore.ts`：新增 `switchBranch` action
- `ChatView.tsx`：传递 `onSwitchBranch` 回调

**验证**：切换分支后，消息列表更新为新分支的消息序列。

### Step 9：后端 `chat-messages-changed` 事件系统

**目标**：所有消息变更操作在事务 commit 后 emit `chat-messages-changed` 事件，携带完整 snapshot。

**变更**：
- `dto.rs`：新增 `ChatMessagesChangedPayloadDto`
- `conversations.rs`：`create_branch` / `switch_branch` / `edit_message` / `regenerate_message` / `delete_message` 末尾 emit 事件
- `chatDeltaEvents.ts`：新增 `chat-messages-changed` 监听器

**验证**：`create_branch` 后前端收到事件，`messages` 与 `get_conversation_detail` 返回值一致。

### Step 10：前端双模式更新策略 + `switchBranch` 去掉二次请求

**目标**：截断模式保留乐观更新，分支模式改为事件驱动；`switchBranch` 不再调用 `getConversationDetail`。

**变更**：
- `chatStore.ts`：`resendFrom` 按 `strategy.kind` 选择乐观/事件驱动
- `chatStore.ts`：`switchBranch` 改为 invoke 后等事件（去掉 `getConversationDetail` 调用）
- `chatStore.ts`：`editMessage`（分支模式）去掉乐观 set content

**验证**：截断模式行为不变（即时反馈）；分支模式等事件后 state 更新。

### Step 11：后端 `create_branch` 硬上限检查 + `fork_to_new_conversation` + 前端 Fork 对话框

**目标**：`MAX_SIBLINGS = 20` 硬上限生效，达到后禁止新建分支；接近上限时提醒；Fork 到新对话可用。

**变更**：
- `conversations.rs`：`create_branch` 新增 `MAX_SIBLINGS` 硬上限检查，达到后返回 `MaxSiblingsExceeded` 错误
- `conversations.rs`：新增 `fork_to_new_conversation`（纯复制，不删除原对话数据）
- `conversation.rs`（cmd）：新增 `fork_to_new_conversation` Tauri command
- `chatStore.ts`：`createBranch` 处理 `approaching_max_siblings` warning 和 `max_siblings_exceeded` error
- 新增 `ForkDialog.tsx`

**验证**：同一 parent 下达到 20 个兄弟时，`create_branch` 返回 `MaxSiblingsExceeded` 错误；Fork 后新对话包含活跃分支消息，原对话不受影响。

### Step 12：前端 `MessageTree` 辅助类 + 操作序列号验证

**目标**：树操作逻辑解耦；防乱序。

**变更**：
- 新增 `chat/core/messageTree.ts`
- `chatStore.ts`：`messageTree` computed property
- `MessageBubble.tsx`：使用 `messageTree.getSiblings()` 替代内联逻辑
- `chatStore.ts`：`operationSeq` 递增 + 事件中验证

**验证**：`MessageTree` 单元测试通过；乱序事件触发 `loadConversation` 刷新。

## 6. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 消息树导致 DB 查询变慢 | `trace_active_branch` 最多遍历对话长度（通常 < 100 条），加 `parent_id` 索引后 O(depth) |
| 分支数无限增长 | `MAX_SIBLINGS = 20` 硬上限，达到后禁止新建分支；用户可 Fork 到新对话继续探索 |
| `active_branch_id` 指向的消息被删除 | `switch_branch` 前验证消息存在；如果 `active_branch_id` 无效，降级为 timestamp 排序 |
| `active_branch_id` 指向非 leaf 消息 | `switch_branch` 接受任意消息 ID，`trace_active_branch` 从该消息回溯到 root；该消息**之后的子分支不显示**（仅显示到该节点）。如需延伸到子分支，自动选择 `siblingIndex = 0` 的子节点作为 leaf |
| 分支切换时流式正在进行 | `switchBranch` 调用 `ensureIdle()` 先取消流式 |
| 旧版前端无法渲染 `siblingIndex` | `#[serde(default)]` + 前端默认值，旧版前端忽略新字段 |
| `create_branch` 并发 | `create_branch` 使用 `unchecked_transaction()` 事务包裹，`MAX()+1` 与 INSERT 在同一事务内原子执行 |
| 分支模式下 `sendMessage` 的上下文构建 | 后端 `send_chat_message` 按 `active_branch_id` 回溯构建上下文，而非按 timestamp 排序 |
| `chat-messages-changed` 事件丢失 | 事件在事务 commit 后 emit；前端有 5s 超时兜底，超时后 `loadConversation` 刷新 |
| `chat-messages-changed` 事件乱序 | 操作序列号 `_seq` 验证，乱序时刷新整个对话 |
| 事件 snapshot 响应体过大 | 活跃分支通常 < 100 条消息，snapshot 大小与 `get_conversation_detail` 一致，可接受 |
| Fork 到新对话时消息 ID 变更 | Fork 后新对话的消息使用新 ID，`parentId` 通过 `id_map` 重新映射；原对话完全不受影响 |
| 截断模式与分支模式切换不一致 | `resendFrom` 内部按 `strategy.kind` 严格分支，不存在混合路径；截断模式保留 I17 行为不变 |

## 7. 验收标准

- [ ] **非破坏性编辑**：编辑 user 消息后，原始 assistant 回复保留，`BranchPicker` 显示 `2/2`
- [ ] **非破坏性重试**：重试失败消息后，原始错误消息保留，新回复作为兄弟追加
- [ ] **非破坏性重新生成**：重新生成后，原始回复保留，形成多个候选
- [ ] **分支切换**：点击 `BranchPicker` 箭头，消息列表更新为目标分支
- [ ] **活跃分支持久化**：切换分支后 reload 对话，仍显示上次选择的分支
- [ ] **单分支不显示选择器**：`siblingCount = 1` 时 `BranchPicker` 不渲染
- [ ] **向后兼容**：旧数据（`parent_id = NULL`, `sibling_index = 0`）行为与 I17 完全相同
- [ ] **截断策略仍可用**：`resendFrom` 传入 `{ kind: "truncate" }` 仍执行截断（I17 行为）
- [ ] **上下文正确**：分支模式下，`send_chat_message` 的上下文仅包含活跃分支的消息
- [ ] **并发安全**：同时编辑两条消息，`sibling_index` 不重复（事务保证）
- [ ] **非 leaf 切换**：`switch_branch` 到中间节点时，显示该节点到 root 的路径；自动延伸到 `siblingIndex = 0` 的子节点
- [ ] **`chat-messages-changed` 事件正确**：`create_branch` / `switch_branch` / `edit_message`（分支模式）后，前端收到事件且 `messages` snapshot 与 `get_conversation_detail` 一致
- [ ] **事件驱动更新**：分支操作后前端 state 由事件更新，非乐观推算；`switchBranch` 不调用 `getConversationDetail`
- [ ] **截断模式即时反馈**：`resendFrom` 传入 `{ kind: "truncate" }` 时，前端仍做乐观更新（I17 行为不变）
- [ ] **`MAX_SIBLINGS = 20` 硬上限**：同一 `parentId` 下达到 20 个兄弟时，`create_branch` 返回 `MaxSiblingsExceeded` 错误，禁止新建分支
- [ ] **接近上限提醒**：兄弟数达到 15 时，`BranchPicker` 显示 warning 样式 + 始终可见文字"分支较多，可 Fork 到新对话"
- [ ] **达到上限 BranchPicker 表现**：兄弟数达到 20 时，`BranchPicker` 显示 Fork 图标按钮，点击弹出 `ForkDialog`
- [ ] **达到上限禁用操作按钮**：`siblingCount >= MAX_SIBLINGS` 时，编辑/重新生成按钮 disabled，hover 提示"分支已达上限，可 Fork 到新对话"
- [ ] **Fork 到新对话**：`ForkDialog` 确认后，活跃分支消息复制到新对话，原对话不受影响
- [ ] **Fork 为常规操作**：`MessageActions` 中 Fork 按钮始终可用（不限上限状态），任何消息都可 Fork
- [ ] **`MessageTree` 辅助类**：`getSiblings` / `getPathToRoot` / `getActiveLeafId` 方法正确，可独立测试
- [ ] **操作序列号验证**：乱序 `chat-messages-changed` 事件触发 `loadConversation` 刷新

## 8. I17 ↔ I17b 边界总结

| 维度 | I17 | I17b |
|------|-----|------|
| 核心目标 | 修复重发语义错误 + 错误类别感知 | 非破坏性重发 + 分支切换 |
| `resendFrom` 签名 | `(pivotTimestamp, contentToSend)` | `(pivotTimestamp, contentToSend, strategy?)` |
| `TruncationStrategy` | 不存在（I17 无 dead code） | `{ kind: "truncate" } \| { kind: "branch" }` |
| 消息数据模型 | 线性数组（不变） | 树结构（`parentId` + `siblingIndex`） |
| 后端 API | 不改消息表结构 | 新增 `sibling_index` + `active_branch_id` + 4 个 API（含 `fork_to_new_conversation`） |
| 前端更新模式 | 乐观更新 | 截断=乐观更新，分支=事件驱动（`chat-messages-changed`） |
| 事件系统 | `chat-delta` / `chat-regenerate`（未监听） | 新增 `chat-messages-changed` 统一事件，`chat-regenerate` 废弃 |
| UI | 无 `BranchPicker` | `BranchPicker` + `ForkDialog` 组件 |
| `editMessage` 行为 | 截断后续 + 重发 | 创建分支 + 重发 |
| `retryFromError` 行为 | 删除失败消息 + 重发 | 保留失败消息 + 创建兄弟 + 重发 |
| 分支上限 | 无 | `MAX_SIBLINGS = 20` 硬上限，达到后禁止新建，提示 Fork 到新对话 |
| 辅助工具 | 无 | `MessageTree` 辅助类 + 操作序列号验证 |
| 依赖关系 | 无 | 依赖 I17 完成 |
| 可独立部署 | ✅ | ❌（需 I17 先完成） |
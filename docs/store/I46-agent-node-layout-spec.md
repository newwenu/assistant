# I46 — Agent 节点布局规格（坐标 / 容器 / 分组 / 密度 / 挂载）

| 项 | 值 |
|---|---|
| 状态 | Draft（实施规格，不含进度记录） |
| 日期 | 2026-11-13 |
| 决策源 | [I46 Agent 节点布局设计（ADR）](./I46-agent-node-layout-design.md)（决策 D1–D13、事实 F1–F15、缺口 G1–G9、门禁 1–20 的唯一出处，本文不复述理由） |
| 关联 | [I46 任务清单](./I46-agent-node-layout-tasklist.md)（任务级进度唯一来源，S1–S6 勾选）；[I38 阶段总纲](./I38-agent-roadmap.md)（阶段与门禁唯一入口）、[I36a 内核能力基线](./I36a-agent-kernel-baseline.md)（turn 事件不变式）、[I36b 内核一致性规格](./I36b-agent-kernel-conformance-spec.md)、[I46 紧凑消息格式设计](./I46-agent-compact-message-format-design.md)（参考记录，`compact` 信封定义仍在用） |
| 影响范围 | 存储两列 + 会话计数器一列；`MessageDto` 两字段；两个既有事件载荷各加坐标；7 条写路径进事务；读取排序键与索引；前端渲染拆 chrome、新增分组投影与 turn 尾；分页游标由时间戳改 turn。**不改**分支链拓扑、工具结果信封、事件名与事件语义 |

> 本文回答「怎么做、改哪些签名、怎么验收」。**为什么这样设计**只在 ADR 里，本文不重述；两文冲突时以 ADR 为准，本文只允许在「实现细节」上细化，不允许推翻决策。
>
> 术语沿用 ADR §1.2：节点 = 行、turn = 容器、group = 展示层投影。本文新增的所有类型都带「展示层 / 持久层」归属标注。

---

## 1 适用范围与开工门

### 1.1 本文覆盖

| 层 | 覆盖内容 | 对应 ADR |
|---|---|---|
| 持久层 | `messages.turn_index` / `messages.iteration` / `conversations.next_turn_index`、取号规则、7 条写路径、读取排序与索引 | D2、§3.1 |
| 契约层 | `MessageDto` 两字段、两个事件载荷、`contract:gen` 同步 | D2 约束 7、§6 |
| 前端数据层 | `Message` 类型继承、乐观消息坐标、事件回填、段表降级为缓存 | D2 约束 7、F3 |
| 前端渲染层 | `groupNodeRows` 纯函数、`TurnTail`、chrome 拆挂、三档密度、安全阀、`unresolved` 平铺 | D3–D13、§3.2–§3.5 |
| 分页 | 游标与页单位改为 turn | D10、P3 |

### 1.2 开工门（三条前置依赖）

| 编号 | 前置依赖 | 现状 | 对本文的影响 | 处置 |
|---|---|---|---|---|
| P1 | 工具结果展示投影 `compact`（`head` / `truncated` / `persisted` / `errorKind` / `anchor`） | **已落地（门禁关闭）**：`ToolResultCompactDto` 与 `compact` 信封可用（I46 compact 任务清单 C1–C10 完成） | 单行形态直接取 `compact.head`，锚点分支（§6.3）可达 | 无需降级；降级分支仅作 `compact == null`（旧数据）的兜底 |
| P2 | 写路径事务化 | `cmd/chat/insert.rs` 与 `cmd/compaction.rs` 的写入**不在事务内**；`adapters/message_store.rs` 两处已在事务内 | 计数器递增与消息写入必须同事务（门禁 19） | S2 内先把 insert 与 compaction 包进 `unchecked_transaction` |
| P3 | 分页按 turn | `get_messages_paginated` 按 `timestamp < ?` 取页 | 跨页切开 turn（G6） | S6 独立切片；S1–S5 期间接受现状，`unresolved` 组不折叠已兜住 |

三条前置之外的阻塞项：无。P1 已关闭；降级分支仅作旧数据（`compact == null`）的兜底，不作为验收缺陷。

### 1.3 不做的事

- 不回填存量行坐标（ADR OQ1 默认值：旧行 `turn_index = NULL`，永久平铺）。
- 不新增事件名、不改既有事件语义、不新增节点 DTO 流。
- 不按 `modeId` / `link` / `TurnKind` 在节点层分叉（门禁 16）。
- 不改 `MessageList` 的滚动与未读逻辑，只在其数据入口前插入投影。

---

## 2 持久层规格

### 2.1 表结构

新库走 `CREATE TABLE`，存量库走 pragma 守卫 `ALTER TABLE`（沿用 `src-tauri/crates/assistant-storage/src/db.rs` 既有模式）。守卫落点有两处，与既有列守卫的分布一致：

- `messages` 的两列守卫落在 `is_new_db == false` 分支内、紧接 `has_mode_id` 守卫之后（该分支是存量库唯一的加列入口）。
- `conversations.next_turn_index` 守卫落在该分支**之前**、紧接 `has_conv_workspace_root` 守卫之后（`conversations` 的列守卫都在这个位置，`messages` 的才在分支内）。

```sql
-- messages（两列）
turn_index INTEGER,                          -- NULL = 未知（存量行 / 压缩摘要）
iteration  INTEGER NOT NULL DEFAULT 0        -- 0 = 非迭代行；>= 1 = 所属迭代

-- conversations（一列）
next_turn_index INTEGER NOT NULL DEFAULT 1   -- 下一个可分配的 turn 序号
```

三处同步：

1. `CREATE TABLE IF NOT EXISTS messages` 末尾追加两列；`CREATE TABLE IF NOT EXISTS conversations` 末尾追加 `next_turn_index`。
2. `MSG_SELECT` 常量追加 `turn_index, iteration`；**注意 `insert_message` 用同一常量拼 `INSERT` 列清单**，参数序号由 17 个变 19 个，必须同步。
3. `MessageRow` 加 `turn_index: Option<i64>`、`iteration: i64`（storage 层沿用既有 `i64` 口径）；`read_msg_row` 追加两行 `row.get("turn_index")?` / `row.get::<_, i64>("iteration")?`。

索引：

```sql
CREATE INDEX IF NOT EXISTS idx_messages_conv_coord
  ON messages (conversation_id, turn_index, iteration);
```

索引**不能**写进 `execute_batch` 的建表批里，也不能写进 `is_new_db == false` 分支内：必须落在两处守卫之后、`init` 收尾前。前者在存量库上会因缺列失败（建表批先于加列执行），后者对新库不生效（新库走 `CREATE TABLE`，不进该分支）。

旧索引 `idx_messages_conv_ts` 保留（导出 / 搜索仍按时间排序）。

`MessageRow` 是 struct literal 构造，加字段后编译器会强制全部构造点补齐——这是「写路径无遗漏」的第一道机械保障，不需要额外巡检。构造点共 7 处：`cmd/chat/insert.rs`（2）、`adapters/message_store.rs`（2）、`cmd/compaction.rs`（1）、`conversations/ops.rs` 分支复制（2）；测试替身（`tests/common/mod.rs`、`protocol_contract.rs`、`protocol_dto_roundtrip.rs`、`assistant-agent/tests/*`）一并补齐。

### 2.2 取号规则

两个取号函数落在 `assistant-storage` 的 `conversations::msg_ops`（与 `insert_message` 同模块，便于在同一事务内调用）：

```rust
/// 跨 turn 取号：读会话计数器旧值并 +1。必须在调用方事务内执行。
pub fn allocate_turn_index(conn: &Connection, conversation_id: &str) -> Result<u32, String>;

/// 同 turn 内取迭代号：`COALESCE(MAX(iteration), 0) + 1`，仅限该 turn 内。
pub fn allocate_iteration(conn: &Connection, conversation_id: &str, turn_index: u32) -> Result<u32, String>;
```

SQL：

```sql
-- allocate_turn_index：先读后写，两条语句在同一事务
SELECT next_turn_index FROM conversations WHERE id = ?1;
UPDATE conversations SET next_turn_index = next_turn_index + 1 WHERE id = ?1;

-- allocate_iteration
SELECT COALESCE(MAX(iteration), 0) + 1 FROM messages
 WHERE conversation_id = ?1 AND turn_index = ?2;
```

四条硬规则（对应 ADR D2 约束 2 与门禁 19）：

1. `allocate_turn_index` 的 `UPDATE` 影响行数为 0 时报「会话不存在」，不得返回默认值。
2. **禁止** `SELECT MAX(turn_index) + 1`；`allocate_iteration` 是唯一允许的算术，且带 `turn_index` 等值条件。
3. 两个函数都不自行开事务、不 `commit`：事务边界归调用方（S2 各路径），避免嵌套事务。
4. 计数器初值恒为 `1`；存量库 ALTER 时不回填 `MAX(turn_index)+1`（ADR §3.5）。

### 2.3 契约字段

`src-tauri/src/protocol/dto/chat.rs`：

```rust
pub struct MessageDto {
    // ...既有字段
    /// 会话内单调 turn 序号（从 1 起）。`null` = 未知（存量行 / 未确认的乐观行）。
    pub turn_index: Option<u32>,
    /// turn 内迭代序号。`0` = 非迭代行（user / compact_summary）。
    pub iteration: u32,
}
```

- `turn_index` 是输出 DTO 的 `Option`，**不加** `skip_serializing_if`：键恒在、值 `null`；生成类型为 `turnIndex: number | null`（必填键）。
- `iteration` 非 `Option`，生成类型 `iteration: number`。
- `From<MessageRow> for MessageDto` 内做显式转换：`r.turn_index.and_then(|v| u32::try_from(v).ok())`、`u32::try_from(r.iteration).unwrap_or(0)`。越界不 panic、按 `null` / `0` 降级（这两种值都落到 `unresolved`，与未知同语义）。
- 生成后必须提交 `src/types/protocol.contract.ts`；禁止手改（门禁 13）。

前端 `Message` 类型（`src/chat/core/chatTypes.ts`）由 `Omit<MessageDto, ...>` 派生，**无需手改**；坐标字段自动继承。

### 2.4 七条写路径

所有路径的共同要求：**取号、写行、递增计数器在同一事务内**；任一语句失败即整体回滚（门禁 19）。

| # | 位置 | 现状 | 规格 |
|---|---|---|---|
| 1 | `src-tauri/src/cmd/chat/insert.rs` `insert_normal_append` | user + assistant 两次 `insert_message`，无事务 | 由 `insert_chat_messages` 开事务；先 `allocate_turn_index` 得 `t`；user 行 `(t, 0)`；assistant 行 `(t, 1)` |
| 2 | 同上 `insert_on_branch`（分支点 `role == "user"`） | 同上，无事务 | 同上：新开一轮，`(t, 0)` + `(t, 1)` |
| 3 | 同上 `insert_on_branch`（分支点 `role == "assistant"`） | 只插 assistant 行，无事务 | 读分支点 `turn_index`；若为 `NULL` 则视为无坐标（`NULL`, `0`）；否则 `t = 分支点.turn_index`、`iteration = allocate_iteration(t)`。**不递增会话计数器**（ADR D2 约束 4 例外） |
| 4 | `src-tauri/src/adapters/message_store.rs` `create_assistant_message` | 已有事务 | `CreateAssistantReq` 加 `turn_index: u32`、`iteration: u32`，直接写入行 |
| 5 | 同上 `append_tool_message` | 已有事务 | `AppendToolMessageReq` 加 `turn_index: u32`、`iteration: u32`，直接写入行 |
| 6 | `src-tauri/src/cmd/compaction.rs` 摘要行 | 单次写入，无事务 | 包事务；写行 `(NULL, 0)`（ADR OQ3 默认值：不并入被压缩 turn） |
| 7 | `assistant-storage` `conversations::ops::duplicate_conversation` | 整表复制 | 原样复制 `turn_index` / `iteration`；新会话 `next_turn_index` 取 `源会话.next_turn_index`（不重排、不重算） |

`insert_chat_messages` 的返回值扩展，供事件下发使用：

```rust
pub(crate) struct InsertedMessages {
    pub assistant_msg_id: String,
    pub user_msg_id: Option<String>,
    pub turn_index: u32,          // 新增：本轮 turn 序号
}
```

分支点 assistant 路径的 `turn_index` 可能为 `NULL`（分支点在存量行上），此时 `InsertedMessages.turn_index` 用 `Option<u32>` 表达；两条分支合并返回：

```rust
pub(crate) struct InsertedMessages {
    pub assistant_msg_id: String,
    pub user_msg_id: Option<String>,
    pub turn_index: Option<u32>,
}
```

`None` 的后果是事件载荷不带坐标、前端该轮落 `unresolved`——与「存量分支点」的真实语义一致，不报错（门禁 14）。

### 2.5 内核侧坐标透传

agent 链路的迭代行与 tool 行由内核写，坐标必须随上下文传入：

| 项 | 位置 | 规格 |
|---|---|---|
| `TurnContext` | `src-tauri/crates/assistant-agent/src/kernel/ids.rs` | 新增 `turn_index: Option<u32>` 字段；`TurnContext::new` 加参数（调用点仅 `src-tauri/src/cmd/agent/mod.rs`） |
| `ensure_row` | `src-tauri/crates/assistant-agent/src/agent/loop_executor/persist.rs` | `CreateAssistantReq { turn_index: ctx.turn_index, iteration: state.iteration }`；`turn_index` 为 `None` 时该列写 `NULL`，`iteration` 照写 |
| `append_tool_results` | 同文件 | `AppendToolMessageReq { turn_index: ctx.turn_index, iteration: 发起该调用的迭代 }`，取 `RoundState.iteration`（`FinalizedToolCall` 无迭代字段，不得新加） |
| `TurnRunCtx` | `src-tauri/crates/assistant-agent/src/agent/round_executor.rs` | 透传 `turn_index`；只做字段搬运，不加判定 |

`CreateAssistantReq` / `AppendToolMessageReq` 的坐标字段用 `Option<u32>`（`None` → 写 `NULL`），避免给内核强加「必然有坐标」的假设：内核不认识持久坐标，只搬运。

`ids.rs` 现有注释「`iteration` 与 `provider_attempt` 不落库、不进 `MessageDto`」必须改写为「`iteration` 落库并进 `MessageDto`；`provider_attempt` 仍只在事件与日志」，否则注释与实现直接矛盾。

### 2.6 实时坐标下发

两个**既有**载荷各加字段（不新增事件、不改事件名）：

```rust
pub struct ChatUserMessageConfirmedPayloadDto {
    // ...既有
    pub turn_index: Option<u32>,
}

pub struct ChatStreamStartPayloadDto {
    // ...既有
    pub turn_index: Option<u32>,
    pub iteration: Option<u32>,   // assistant 占位行的实际迭代号
}
```

`iteration` **不是恒 `1`**：分支点是 assistant 时属于同轮重生成，`iteration = allocate_iteration(t) ≥ 2`（§2.4 路径 3）。写死 `1` 会让前端占位消息与落库行坐标不一致，因此由 `InsertedMessages.iteration` 如实下发。

`Option` 的理由：`insert_on_branch` 分支点在存量行上时无坐标（§2.4 路径 3），非 `Option` 会逼出假值。`None` 时前端按 `unresolved` 处理。

装配点：

| 载荷 | 装配位置 | 来源 |
|---|---|---|
| `ChatUserMessageConfirmedPayloadDto` | `src-tauri/src/adapters/event_sink.rs` | `TauriEventSink` 持有 `turn_index: Option<u32>` 与 `iteration: Option<u32>`（由 `with_turn_coords` 注入） |
| `ChatStreamStartPayloadDto` | 同上 | 同一份 `turn_index` / `iteration`，两载荷共用，不各自推导 |

`src-tauri/src/cmd/agent/mod.rs` 的 `TauriEventSink::new(...)` 调用后追加 `.with_turn_coords(inserted.turn_index, Some(inserted.iteration))`；chat 链路走同一个 sink 构造点，不另写一份。`InsertedMessages` 因此同时携带 `turn_index` 与 `iteration`。

turn 内后续迭代**不新增事件**：前端用「`chat-stream-start` 的 `turnIndex` + `chat-delta` 自带的 `iteration`」给新段赋坐标，两分量均为 wire 事实（ADR D2 约束 7）。

### 2.7 读取与排序

排序键（`unresolved` 行整体垫底，旧数据不会被排到最前）：

```sql
ORDER BY (turn_index IS NULL) ASC,
         turn_index ASC,
         iteration ASC,
         timestamp ASC,
         rowid ASC
```

落点：

| 位置 | 现状 | 规格 |
|---|---|---|
| `msg_ops::get_messages` | `ORDER BY timestamp ASC, rowid ASC` | 改为上表键 |
| `branch::trace_active_branch` | 按父链逐级 `SELECT`，结果顺序由链决定 | **不改**：链顺序是 turn 顺序的唯一权威，坐标只做校验不做拓扑（ADR §3.1） |
| `msg_ops::get_messages_paginated` | `timestamp < ?2` 游标 | 见 §7 |

---

## 3 前端数据层规格

### 3.1 乐观消息

`src/chat/core/optimisticMessage.ts` 是唯一构造点，扩展入参与输出：

```ts
export interface OptimisticMessageFields {
  // ...既有
  turnIndex?: number | null;
  iteration?: number;
}

export function createOptimisticMessage(fields: OptimisticMessageFields): Message {
  return {
    // ...既有
    turnIndex: fields.turnIndex ?? null,
    iteration: fields.iteration ?? 0,
  };
}
```

输出字段必填（`turnIndex: number | null`、`iteration: number`），入参可选，缺省即 `null` / `0`——与「未确认的乐观行」语义一致。

调用点（共 8 处）按可用信息传值，**不得**为了凑字段去推算相邻关系（ADR D2 约束 3）：

| 调用点 | `turnIndex` | `iteration` |
|---|---|---|
| `src/stores/agent/agentSegments.ts` `makePlaceholder` | 已知则传（见 §3.4），否则 `null` | 传入的 `iteration` |
| `src/services/stream/turnOrchestrator.ts` user 占位（发送时） | `null`（后端尚未分配） | `0` |
| 同上 assistant 占位（`chat-stream-start` 后） | `chat-stream-start` 的 `turnIndex` | `chat-stream-start` 的 `iteration`（分支重生成时 `> 1`） |
| `src/stores/chat/chatCompactSlice.ts` 等其余点 | 能取到则传，取不到 `null` | 同上口径 |

### 3.2 事件回填

`src/services/stream/transport.ts` 的 handler 签名扩展（传输层仍只透传、不解释）：

```ts
export interface StreamTransportHandlers {
  // ...既有
  onStreamStart: (
    conversationId: string,
    attemptId: string,
    messageId: string,
    turnIndex: number | null,
    iteration: number | null,
  ) => void;
  onUserMessageConfirmed: (
    userMessageId: string,
    userMessageContent: string,
    turnIndex: number | null,
  ) => void;
}
```

`src/services/stream/turnOrchestrator.ts`：

1. `onStreamStart` 把 `turnIndex` 写入 store（新增 `currentTurnIndex: number | null` 与 `setCurrentTurnIndex`），并通过 `setMessageCoords(messageId, turnIndex, iteration)` 把当前 assistant 占位行的坐标回填为事件给出的两分量（两分量缺一不回填）。
2. `onUserMessageConfirmed` 把 `turnIndex` 传给 `confirmUserMessage`（端口签名由 `(id, content)` 扩为 `(id, content, turnIndex)`，同步 `src/stores/chatStorePorts.ts`、`src/services/stream/turnStrategy.ts` 与测试替身）。
3. `onTextDelta` 收到 `iteration >= 2` 时开新段，新段坐标 = `(store.currentTurnIndex, chunk.iteration)`。

门禁 17 的判定依据即此三项：流式期当前 turn 的节点必须已有 `turnIndex`，不得等终态快照。

### 3.3 段表降级为缓存

`src/stores/agent/agentSegments.ts` 的 `segmentMessageIdByIteration` 由「唯一真源」降级为「加速缓存」：

```ts
/** 从已加载消息重建映射：坐标是事实源，段表只是缓存。 */
rebuildFromMessages: (messages: Message[], turnIndex: number | null) => void;
```

- 扫描 `role === "assistant"` 且 `turnIndex === turnIndex` 且 `iteration >= 1` 的行，按 `iteration` 建映射；
- 与既有映射冲突时**以消息行为准**（坐标是持久事实）；
- `resolveSegment(iteration)` 在缓存未命中时先查已加载消息（同坐标行），仍无则 `openSegment`；
- `markUnknownSegment`（切回运行中会话 / 刷新恢复）先调用 `rebuildFromMessages`：已落库的迭代直接对上历史行，仍在飞的迭代（库中尚无行）由首个增量开新段；
- `adoptSegment` 保留但退化为可选优化，不再是刷新后恢复的唯一手段（F3 的失忆场景由 `rebuildFromMessages` 覆盖）。

### 3.4 store 侧新增字段

`src/stores/chatStreamStore.ts` 加两个会话无关的瞬时字段（不进 wire、不进落库）：

```ts
currentTurnIndex: number | null;
setCurrentTurnIndex: (turnIndex: number | null) => void;
```

切会话时随 `beginSend` / 重置逻辑一并清空（避免把上一轮的坐标带到新会话）。

---

## 4 容器层规格

### 4.1 chrome 归属

| 元素 | 归属 | 实现落点 |
|---|---|---|
| 身份行（头像 / 模式 / 模型） | turn 内首个 `role === "assistant"` 节点 | `showIdentity` 判据改为坐标（§4.3） |
| 时间戳 / `MessageActions` / `BranchPicker`（assistant 侧） | turn 尾 `TurnTail` | 新增 `src/chat/message/TurnTail.tsx` |
| 时间戳 / 编辑 / 删除（user 侧） | user 节点自身 | `MessageBubble` 不动该分支 |
| 失败重试入口 | 失败节点就地 | 现状保留 |
| 折叠控件 | 组头 | §5 |
| `compact_summary` | 自身渲染 | 不吃身份行、不吃 `TurnTail` |

### 4.2 `TurnTail` 契约

```tsx
interface TurnTailProps {
  /** 锚点行（assistant 侧）；调用方保证非空。 */
  anchor: Message;
  isStreaming?: boolean;
  isBranchAtLimit?: boolean;
  onDelete?: (id: string) => void;
  onRegenerate?: (id: string) => void;
  onSwitchBranch?: (id: string, direction: "prev" | "next") => void;
  onFork?: (id: string) => void;
  onCopy?: (text: string) => void;
}
```

三条实现约束：

1. 只读 `anchor.id` / `anchor.timestamp` / `anchor.status` / `anchor.siblingIndex` / `anchor.siblingCount`；**不新增消息行**、不改分支链。
2. 锚点选择（ADR D3 三级优先级）在 `groupNodeRows` 内完成，`TurnTail` 不自行选锚点：

```ts
/** 优先级 1：最后一条有正文且 settled 的 assistant 回复行；
    2：该 turn 内最后一个 assistant 节点；3：无 assistant 节点 -> null（不渲染 TurnTail）。 */
function pickTailAnchor(messages: Message[]): Message | null;
```

3. `compact_summary` 行与 `role === "tool"` 行不参与锚点选择（前者是边界行，后者不在可见列表）。

### 4.3 `showIdentity` 判据

```
showIdentity = msg.role === "assistant"
             && msg.turnIndex !== null
             && 该组内这是第一个 assistant 节点
```

`turnIndex === null` 的分组回落现状判据（`index === 0 || 上一条不是 assistant`），因为 `unresolved` 组没有轮边界可依（ADR D2 约束 3 的降级口径）。

### 4.4 `MessageBubble` 改造

新增两个可选 prop：

```ts
interface MessageBubbleProps {
  // ...既有
  /** true 时 assistant 侧 chrome 由 TurnTail 承接，本气泡不渲染。 */
  suppressAssistantChrome?: boolean;
  /** 本行是所在 turn 的尾锚点：由它挂载唯一的 TurnTail。 */
  isTurnTail?: boolean;
}
```

- `suppressAssistantChrome === true` 时跳过 `actionsWrapper`（时间戳 + `BranchPicker` + `MessageActions`）；`error` / `retry` / 编辑块仍在本气泡渲染（失败与重试必须就地）。
- user 行的 chrome 恒由自身渲染，不受该 prop 影响。
- `isTurnTail` 由 `groupNodeRows` 的 `tailAnchor` 派生（`tailAnchor.id === message.id`），组件不自行选锚点。
- 规格曾规划的 `dense` prop 随 `compact` 密度一并落地，本批次不加（避免无用接口）。
- `TurnTail` 复用 `MessageBubble.module.css` 的 `actionsWrapper` / `timestamp` 样式，挂载位置与该气泡现状一致；chat 轮（1 user + 1 assistant）因此**零视觉变化**，门禁 15 的判据是逐项一致，不是「大致相同」。
- `React.memo` 比较器追加两个新 prop 与坐标字段（`turnIndex` / `iteration`），否则坐标变化不触发重渲染。

---

## 5 分组投影规格

### 5.1 纯函数签名

新增 `src/chat/message/groupNodeRows.ts`（纯函数，无 I/O、无时间、无随机；不 import 任何 store）：

```ts
export interface GroupNodeRowsInput {
  messages: Message[];                 // 已过滤 role === "tool"
  link: "chat" | "agent";              // 只读快照，禁止参与分支（门禁 16）
  phase: GroupPhase;                   // §5.3
  currentTurnIndex: number | null;     // `chat-stream-start` 载荷（§3.2）
  /** 流式占位行 id：`currentTurnIndex` 未下发时兜住 `collecting`（§5.3）；不参与分组。 */
  streamingMessageId?: string | null;
}

export function groupNodeRows(input: GroupNodeRowsInput): NodeGroupView[];
```

`link` 的唯一用途是写入 `NodeGroupView.link` 供断言与调试；函数体内**不得**出现任何 `if (link === ...)`。

### 5.2 视图类型

```ts
export type GroupPhase = "collecting" | "settled";

/** 一个连续可折叠步段（run）的统计；`id` 由首步 `callId` 生成，跨渲染稳定。 */
export interface NodeRunStats {
  id: string;
  steps: number;
  failed: number;
  durationMs: number | null;
}

export type NodeView =
  | {
      kind: "message";
      message: Message;
      showIdentity: boolean;
      ownsAssistantChrome: boolean;
      /** true：气泡无正文，只承载身份行 / 尾锚点 / 错误，不画空气泡壳。 */
      chromeOnly: boolean;
    }
  | {
      kind: "tool";
      callId: string;
      view: ToolCallView;
      /** 所属折叠 run；null 表示 run 外的步（待处理 / 无摘要 / 流式中），恒以卡片显示。 */
      run: NodeRunStats | null;
    };

export interface NodeGroupView {
  turnIndex: number | null;
  nodes: NodeView[];                   // 已按 §2.7 键序
  phase: GroupPhase;
  isLastGroupInTurn: boolean;
  /** 组级汇总，仅供断言与调试；折叠判定只看 run（§5.4）。 */
  needsAttention: boolean;
  link: "chat" | "agent";              // 只读，禁止参与决策
  stats: { steps: number; failed: number; durationMs: number | null };
  /** 本 turn 内是否存在可折叠 run；`false` 时全部步为 run 外卡片节点。 */
  collapsible: boolean;
  /** turn 尾锚点；null 时本组不渲染 TurnTail。 */
  tailAnchor: Message | null;
}
```

- 组内**不含**「是否展开」字段：展开态是 UI 本地状态（ADR D6），**且按 run 各自持有**（§6.1）。
- 密度不是投影字段：`run === null` 的步恒为卡片，`run !== null` 的步是否单行由该 run 的展开态决定（§6.1）；投影不预判 UI 状态。
- `stats` 只由 `view.status` / `view.startedAt` / `view.finishedAt` 与节点数算出，**不读正文**（门禁 8）。
- `isLastGroupInTurn` 供未来「末组不折叠」类规则使用；本文的实现不使用它做折叠判定（折叠只看 §5.4 判据）。

### 5.3 相位

```ts
carriesStreamingRow = streamingMessageId != null
                   && group.messages.some(m => m.id === streamingMessageId)

phase = (isStreaming && group.turnIndex !== null
         && (group.turnIndex === currentTurnIndex || carriesStreamingRow))
      ? "collecting"
      : "settled";
```

- `isStreaming` 唯一来源是 `session-state-changed`（`Processing` → `true`，`Idle` / `Error` → `false`）；
- `currentTurnIndex` 唯一来源是 `chat-stream-start`（§3.2）；
- `carriesStreamingRow` 是 `currentTurnIndex` 未下发时（刷新 / 恢复 / 事件乱序）的兜底：只要该组含流式占位行，就仍是 `collecting`。**没有它，运行中的 turn 会被判成 `settled` 而提前收起，组头步数随每次增量跳动**（门禁 6 的反例）；
- **禁止**静默计时器、`useEffect` 后置标志、渲染完成信号（ADR D7、门禁 6）；
- 终态事件顺序「先 `chat-messages-changed`、后 `session-state-changed`」保证 `Idle` 到达时快照已并入，故两态足够。

`unresolved` 组（`turnIndex === null`）恒为 `settled` 且 `collapsible === false`。

### 5.4 折叠判据

折叠的**最小单位是 run（连续可折叠步段），不是 turn**。一个 turn 内可以出现多个 run，各自独立计数、独立判定阈值、独立展开（ADR D8 的本意是「需要用户动作的节点不折叠」，不是「整个 turn 一票否决」）。

**步是否可折叠（`isFoldableStep`）**：

```ts
isFoldableStep(view) = (view.status === "success" || view.status === "error")
                     && hasStepSummary(view)
// hasStepSummary：compact.head 非空串，或 summarizeArguments(arguments) 非空串
```

`running` / `awaiting_user` / `missing` / `cancelled` 一律**不可折叠**，且**切断**当前 run。

失败步（`status === "error"`）**允许折叠**（2026-11-14 口径调整）：失败原因已由 `compact.head`（`err <kind> <tool> · <subject> · <reason>`）与 `compact.errorKind` 承载，单行同时显示失败图标、原因与状态文案，组头显示本段失败数；点开该行即还原完整卡片（`ToolCallBubble`）。于是「一个失败就把整轮铺开」不再发生，原因也不会被藏掉——它在单行里。

**切段规则（`planRuns`）**：按消息顺序遍历，遇下列任一情况 `flush()` 当前 run 并置 `cutNext`：

- 不可折叠的步（见上）；
- 消息级 `error != null` 的 assistant 行；
- `compact_summary` 边界行；
- **任何带正文（`content` 非空）的行**。

正文行必须切断 run：它在节点序列里会插入一个气泡节点，若不切断，同一 run 的步会被气泡打断，组头在该段前重复渲染，且**每处都显示整段总步数**——组头计数与实际行数不符（「所有节点共用一个步数」「12 步但下面没那么多行」）。切段后每个 run 在节点序列里严格连续，组头出现且仅出现一次，`run.steps` 恒等于该 run 的步行行数。

**思考行（2026-11-14 口径调整）与工具步同权**：`reasoning` 不再折算进气泡，而是投影为独立的 `kind === "reasoning"` 节点，与同段工具步共享同一 run、同一组头、同一展开开关。因此思考行**不切断** run（否则「思考 + 本轮工具」会被拆成两段、组头出现两次）。仅当 turn 内**存在工具步**时才上移思考节点；零工具 turn（chat 轮、agent 纯问答轮）思考仍留在气泡内，外观与现状一致。思考节点先于同消息的正文气泡、工具步之前输出（思考发生在该轮动作之前）。

同一 turn 内重复下发的 `callId` 只计一次、只渲染一次（`seenCallIds` 与 `emittedCallIds` 同一口径）：否则组头步数虚高，且重复的 React key 会让实际渲染行数少于计数。

**run 成立**：

```ts
const PROCESS_COLLAPSE_THRESHOLD = 1;   // src/config/constants.ts

run = 连续可折叠步（含思考行），且 pending.length >= PROCESS_COLLAPSE_THRESHOLD
     且 pending 内至少 1 个工具步
```

阈值取 **1**（2026-11-14 口径调整，ADR OQ9 定案）：1～2 步也成组。单步 run 渲染「组头 + 1 行」≈ 52 px，仍远低于一张卡片（≈ 160–400 px），所以每个可折叠步都有压行收益，不再出现「两三个工具永远铺成卡片」。阈值只决定「是否成组」，不决定组头形态：`0` 步的 turn 仍落单条档。

run 必须含至少 1 个工具步：纯思考段若也成组，会渲染出「执行过程 · 1 思考」这种只有一行、且与思考行自身文案重复的组头。纯思考段（如「思考 → 中间正文 → 工具」里的前置思考）的 `run === null`，思考节点走 `density="block"`，由 `ReasoningBlock` 自带开关折叠，仍是独立节点而非气泡内容。

**组级 `collapsible`**：

```ts
collapsible = foldAllowed && runs.length > 0

foldAllowed = phase === "settled"
           && turnIndex !== null
           && !hasBoundaryNode(turn)      // compact_summary
```

- `foldAllowed` 只拦「整个 turn 都不能折叠」的情形（流式中 / 无坐标 / 含边界节点）；单步失败**不再**参与组级判定；
- `processCount` 按 run 计（`NodeRunStats.steps`），不计回复行（ADR D5）。思考行单独计入 `NodeRunStats.reasoning`：它与工具步共用展开开关，但组头文案分开显示（`N 步` / `M 思考`），不把思考折算成工具步；
- 回复行永不进折叠组：任何 `kind === "message"` 且 `message.content` 非空（或 `role === "assistant"` 且非空正文）的节点都留在组外；
- **气泡只承载正文与 chrome，不再承载任何 `toolCalls` / `reasoning`**（2026-11-14 口径调整）：可折叠步上移为单行，run 外的步（`awaiting_user` / 无摘要 / 流式中）同样上移为**独立卡片节点**（`density="card"`）。这样「步 → 等待卡 → 步」的时间顺序不会被气泡挤到最前面，`ask_user` 也不再被顶到工具步之前；
- 纯工具行若既无正文、也不承载身份行 / 尾锚点 / 错误，则**不产生气泡节点**；需要承载时只留一个 `chromeOnly` 气泡（不画空气泡壳），保证身份行与 `TurnTail` 恒有挂载点；
- `needsAttention` 来自 `view.status === "error"` / `"awaiting_user"` 与消息级 `error`，**只作组级汇总与断言**，不参与 `collapsible` 计算（D8 安全阀已下沉到步级切段）；
- 组头显示本 run 的失败数（`NodeRunStats.failed`，即该 run 内 `error` / `cancelled` 工具数）：失败步可折叠，但失败数必须外显；`awaiting_user` 与无摘要的步仍在气泡内常驻。

### 5.5 挂载顺序

一个 turn 的渲染序列固定为：

```
[身份行] + [回复行 / 过程组（组头 + 步行）] + [TurnTail]
```

一个 turn 内可以有**多个过程组**（每个 run 一组），组头在 run 的首步之前渲染；节点严格按消息顺序输出，气泡不再抢占步的位置：

```
[身份行] + [run₁ 组头 + run₁ 步行] + [run 外的步（独立卡片节点）] + [run₂ 组头 + run₂ 步行] + [回复行] + [TurnTail]
```

投影层的输出顺序是「每条消息先气泡（仅当需要）、再该消息的全部步」，因此 `[c1(success), ask_user(awaiting_user)]` 渲染为 `tool(c1) → 组头? → tool(ask_user, card)`，`ask_user` 不会被顶到 `c1` 之前；纯工具 turn 仍由 `chromeOnly` 气泡挂身份行与 `TurnTail`。

- 组内顺序沿用消息顺序，不重排（`reasoning < reply < process < boundary` 的类顺序只用于同迭代内的稳定排序，实现在投影内，不改消息数组）；
- 单条消息的节点输出序固定为「思考节点 → 正文气泡（仅当需要）→ 工具步」：思考发生在该轮动作之前，正文是该轮小结；同一 run 内的思考行与工具步在节点序列里连续，组头只在 run 首个节点前出现一次；
- 正文行仍切断 run（§5.4）：保证「组头 + 步」连续，组头计数等于其下实际行数（`run.steps` + `run.reasoning`）。
- `TurnTail` 挂在尾锚点行（§4.2 三级优先级）；锚点通常是该组最后一条回复行，故渲染位置即组末。若该 turn 在回复行之后还有无正文的 assistant 节点（如后续迭代的纯工具行），尾锚点仍是回复行，`TurnTail` 随该行渲染；
- `unresolved` 组不产生组头、不产生 `TurnTail`（chrome 留在各气泡，形态与现状一致）。

---

## 6 节点行形态规格

### 6.1 三档密度

密度按 **run** 判定，不按 turn：

| 档 | 条件 | 渲染 |
|---|---|---|
| 单条 | 该 run 未展开 | 组头 + 每步一行；回复行与 run 外的步（独立卡片节点）在组外 |
| 展开 | 用户点击该 run 的组头 | 该 run 的步逐条还原为卡片（复用现有卡片），其他 run 不受影响 |
| 不折叠 | `foldAllowed === false`（流式中 / 无坐标 / 含边界节点），或该 run 内无可折叠步（阈值 1 时等价于 0 步） | 现状逐节点卡片，无组头 |

展开态由 `TurnGroup` 的本地 state 持有，**键为 run id（`run:<首步 callId>`）**；`turnIndex` 变化时随父级 key 重建，切会话时重置。**禁止**把展开态写进 `NodeGroupView`、store 持久层或 `messages`。一个 run 的展开/收起**不得**影响同 turn 内其他 run 的密度。

### 6.2 单行取值（正式分支 `compact`，降级仅兜底旧数据）

正式分支（`view.compact != null`，P1 已关闭）：

| 元素 | 取值 | 缺失时 |
|---|---|---|
| 状态图标 + 成败 | `view.status` / `view.compact.errorKind` | `errorKind === null` 时不显示分类 |
| 工具名 | `view.name` | — |
| 对象 / 规模 / 命中 | `view.compact.head`（后端已拼好的单行，≤ 160 字符） | `head` 为空串时回退降级分支 |
| 耗时 | `view.startedAt` / `view.finishedAt` | 任一为 `null` 时省略 |
| 文档位置引用 | `view.compact.anchor` | 为 `null` 时不显示跳转 |

降级分支（仅 `view.compact == null`，即旧数据）：

| 元素 | 取值 | 缺失时 |
|---|---|---|
| 状态图标 + 成败 | `view.status` / `view.errorKind` | `errorKind === null` 时不显示分类 |
| 工具名 | `view.name` | — |
| 对象 | `summarizeArguments(view.arguments)`（既有实现） | 返回 `null` 时只显示工具名 |
| 规模（行数 / 字节 / 命中） | **整列省略** | 不得从 `resultText` 现算，不得截断正文代替（门禁 8、9） |
| 耗时 | `view.startedAt` / `view.finishedAt` | 任一为 `null` 时省略 |
| 文档位置引用 | **不存在** | 见 §6.3 |

失败步同样走单行档：状态图标为失败、状态文案取 `view.status`，原因已包含在 `compact.head` 的 `err <kind> <tool> · <subject> · <reason>` 中，因此失败步不再享有「整卡常驻」特权；需要完整输出时点开该行即还原卡片（ADR D8，2026-11-14 口径调整）。`awaiting_user` 与无摘要的步仍整卡常驻，但它们是**独立的卡片节点**（`run === null`），不再塞回气泡（§5.5）。

两分支的**形态完全一致**（同一行、同一列序），只是取值来源不同；渲染层不得为「让单行好看」自行拼 head 或现算规模。

### 6.3 选路单点

新增 `src/chat/tool/ToolCallRenderer.tsx`，作为工具渲染的唯一入口：

```tsx
interface ToolCallRendererProps {
  view: ToolCallView;
  density: "single" | "card";
}
```

- `onOpenDocument` 属于 `compact.anchor` 分支的接口，随 `compact` 落地一并加入（P1 已关闭，接口已可用）；
- 选路判据只有 `compact.anchor` 是否存在与文档面板是否打开（ADR D12）；
- **禁止**读 `role` / `modeId` / `TurnKind` / 工具名（门禁 11）；
- 旧数据 `compact == null` 时 `anchor` 恒不存在 → 走 `density` 分支；
- `MessageBubble` 内 `toolCalls.map(...)` 改为 `ToolCallRenderer`；步行在 `TurnGroup` 内按 run 展开态给出 `density`，投影不产出该字段（§6.1）。

### 6.4 思考行形态

思考节点（`ReasoningNode`，`src/chat/message/ReasoningNode.tsx`）与工具步同权，三档密度同样按 **run** 判定：

| 档 | 条件 | 渲染 |
|---|---|---|
| `single` | `run !== null` 且该 run 未展开 | 单行：`💭 思考` + 首个非空行（单行省略号截断）+ 流式标记 |
| `expanded` | `run !== null` 且该 run 已展开 | `💭 思考` 标签行 + 全文 `pre`（不二次折叠，避免「组头展开后还要再点一次」） |
| `block` | `run === null`（流式中 / 纯思考段 / 边界行） | 复用 `ReasoningBlock`：自带开关的单行 + 展开后全文 |

- 单行档只取**首个非空行**（与门禁 8 同口径：不解析、不截断正文代替结构信息），全文只在展开态出现；
- 思考行**不显示**状态图标、耗时、工具名等工具列（思考没有这些字段）；
- 组头计数：`run.steps` 显示「N 步」，`run.reasoning` 显示「M 思考」，两者独立；`run.reasoning` 为 0 时不显示该文案；
- 思考节点不修改 wire、不落库、不新增字段：投影键由 `reasoningKeyOf(message)` 派生稳定键 `reasoning:<messageId>`（一条消息至多一段思考），仅用于 `runByKey` 映射与 React key；
- 上移思考节点时，气泡消息体的 `reasoning` 置为 `null`（投影副本，不改原 `messages`），保证「思考只出现一次」。

---

## 7 分页与挂载规格（P3）

### 7.1 后端

`msg_ops::get_messages_paginated` 游标由时间戳改 turn：

```rust
pub fn get_messages_paginated(
    conn: &Connection,
    conversation_id: &str,
    limit: i64,
    before_turn_index: Option<u32>,
) -> Result<Vec<MessageRow>, String>;
```

两步查询（保证页边界不切开 turn）：

```sql
-- 1) 取上一页的 turn 集合（最近 N 个有坐标的 turn）
SELECT DISTINCT turn_index FROM messages
 WHERE conversation_id = ?1 AND turn_index IS NOT NULL AND turn_index < ?2
 ORDER BY turn_index DESC LIMIT ?3;

-- 2) 取这些 turn 的全部消息（加上 unresolved 段：turn_index IS NULL 单独成页）
SELECT {MSG_SELECT} FROM messages
 WHERE conversation_id = ?1
   AND (turn_index IN (...第 1 步结果...) OR (?2 IS NULL AND turn_index IS NULL))
 ORDER BY (turn_index IS NULL) ASC, turn_index ASC, iteration ASC, timestamp ASC, rowid ASC;
```

- `limit` 保留为「本页最多消息数」的护栏，新增常量 `TURNS_PER_PAGE = 12` 控制第 1 步的 turn 数；
- `before_turn_index === null` 表示取最新一页；`unresolved` 段在 `turn_index` 游标耗尽后单独成页；
- command 层参数由 `beforeTimestamp` 改 `beforeTurnIndex: Option<u32>`，前端 `chatService` 与调用点同步；这是 command 参数变更，不属于 DTO 契约，但需同步测试。

### 7.2 前端

- `MESSAGES_PAGE_SIZE` 语义改为「消息护栏」，原值保留；`TURNS_PER_PAGE = 12` 是**后端常量**（`assistant-storage` 导出），前端不重复定义——页容量由后端切片，前端只传护栏与游标，避免两处常量漂移；
- `MessageList` 的 `onLoadMore` 不改语义，只把游标换成 `beforeTurnIndex`；
- 跨页只合并「持久化相邻」的同 turn 节点；做不到时该组标 `unresolved` 并禁用折叠（门禁 10）。

---

## 8 实施切片

每个切片独立可回滚；切片内完成自验（§9）。

| 切片 | 内容 | 涉及文件 | 完成判据 |
|---|---|---|---|
| S1 | 存储与契约：两列 + 计数器 + 索引 + `MSG_SELECT` / `MessageRow` + DTO 两字段 + 排序键 | `db.rs`、`conversations/mod.rs`、`msg_ops.rs`、`protocol/dto/chat.rs` | `cargo check` 通过；`contract:gen` 生成物已提交；旧数据查询仍可用 |
| S2 | 写路径：7 处 + 取号函数 + `insert_chat_messages` 事务化 + `InsertedMessages.turn_index` + 内核 `TurnContext` 透传 | `cmd/chat/insert.rs`、`adapters/message_store.rs`、`cmd/compaction.rs`、`ops.rs`、`kernel/ids.rs`、`persist.rs`、`round_executor.rs`、`cmd/agent/mod.rs` | 新建会话一轮后，库中无 `turn_index IS NULL` 的新行（门禁 18 的 chat 段） |
| S3 | 事件与前端数据层：两载荷 + `event_sink` 注入 + handler 签名 + 乐观消息坐标 + store `currentTurnIndex` + 段表 `rebuildFromMessages` | `event_sink.rs`、`transport.ts`、`turnOrchestrator.ts`、`optimisticMessage.ts`、`chatStreamStore.ts`、`chatStorePorts.ts`、`agentSegments.ts` | 流式期当前 turn 节点已有坐标（门禁 17）；刷新后段表可重建（门禁 2 的前端侧） |
| S4 | 容器层：`showIdentity` 改坐标 + `MessageBubble` 两 prop + `TurnTail` | `MessageList.tsx`、`MessageBubble.tsx`、`TurnTail.tsx` | chat 与 agent 单轮零工具渲染与现状逐项一致（门禁 15） |
| S5 | 分组与密度：`groupNodeRows` + 切段（`planRuns`）+ `GroupHeader` + 三档 + 步级安全阀 + `ToolCallRenderer` 单行取值（正式分支 `compact.head`，旧数据降级） | `groupNodeRows.ts`、`TurnGroup.tsx`、`GroupHeader.tsx`、`ToolCallRenderer.tsx`、`ToolCallBubble.tsx` | 8 迭代 × 2 调用场景可折叠；1～2 步 run 与失败步同样成组；`awaiting_user` / 无摘要的步只切断当前段；回复行恒在组外（门禁 4、5、7） |
| S6 | 分页按 turn | `msg_ops.rs`、command 层、`chatService.ts`、`constants.ts` | 一次 turn 不被切成两页（门禁 10） |

依赖顺序：S1 → S2 → S3 → S4 → S5 → S6。S4 可在 S2 完成后先做（`turnIndex === null` 全量兜底时形态与现状一致），S5 必须等 S3（否则折叠判据拿不到 `phase`）。

---

## 9 验收与用例

### 9.1 门禁映射

| 门禁（ADR §4） | 用例落点 | 断言要点 |
|---|---|---|
| 1 坐标一致 | `src-tauri/tests/conversations.rs` 新增用例 + `tests/protocol_dto_roundtrip.rs` | assistant 行 `iteration` 等于 `chat-delta` / `chat-stream-start` 的 `iteration`；tool 行等于 `agent-tool-call` 的 `iteration`；tool 行不写 `0` |
| 2 重启等价 | `src-tauri/tests/conversations.rs` | 同一库重新打开后，按新排序键得到的 `(turn_index, iteration)` 序列与写入时一致 |
| 3 禁用启发式 | `src/chat/message/__tests__/groupNodeRows.test.ts` | 两个相邻 assistant 行、坐标不同时不被合并；坐标缺失时落 `unresolved` |
| 4 回复行不折 | 同上 | 「结论后继续迭代一次」的序列中，含正文节点全部在组外；run 外的步为独立卡片节点，气泡不承载 `toolCalls` |
| 5 折叠纯投影 | 同上 | 投影前后 `messages` 引用/内容不变；展开后节点顺序与 `id` 一致；同一 turn 的多个 run 各自计数 |
| 6 相位不靠定时器 | 同上 | 注入长静默不改变 `phase`；`isStreaming=false` 且非当前 turn 时恒 `settled`；流式增量期间不产生 run |
| 7 安全阀 | 同上 | `awaiting_user` / 无摘要的步**切断 run** 且渲染为独立卡片节点（不进 run、不被折叠吞掉）；失败步（`error`）可折叠且单行显示原因、组头显示失败数；1 步 run 也成组（阈值 1）；`running` 或 `compact_summary` 存在时该段不折叠 |
| 8 收起态不读正文 | 同上（spy） | `parsePersistedOutput` / `resultStats` / markdown 解析零调用 |
| 9 单行可读 | 同上 | `arguments` 可解析时单行非空；不可解析时仍有工具名 + 状态 |
| 10 跨页不重建 | `src/chat/message/__tests__/groupNodeRows.test.ts` + 后端分页用例 | 页边界处不合并非相邻节点；`unresolved` 组不折叠 |
| 11 选路不看身份 | `src/chat/tool/__tests__/ToolCallRenderer.test.tsx` | 同 turn 内一个有锚点一个没有，选路只看锚点与面板态 |
| 12 无新增 wire 负担 | `tests/protocol_dto_roundtrip.rs` | 新增字段仅 `turn_index` / `iteration` / 两个载荷坐标；无其它字段 |
| 13 契约同步 | CI | `contract:gen` 后无 diff |
| 14 旧数据可用 | `src-tauri/tests/conversations.rs` | 全部 `turn_index IS NULL` 的库可正常读出，排序稳定 |
| 15 退化等价 | `src/chat/message/__tests__/MessageList.snapshot.test.tsx` | chat 轮与 agent 单轮零工具的渲染树逐项一致（时间戳位置、`MessageActions`、`BranchPicker`、身份行、`compact_summary`） |
| 16 节点层零读链路 | 同上 + 源码断言用例 | `groupNodeRows` 内无 `modeId` / `link` / `TurnKind` 分支；`NodeGroupView.link` 只出现在断言 |
| 17 流式期坐标不跳变 | `src/services/__tests__/turnOrchestrator.test.ts` | `chat-stream-start` 后当前 turn 节点已有 `turnIndex`；终态快照坐标与流式期逐条相等 |
| 18 写路径无遗漏 | `src-tauri/tests/conversations.rs` 端到端用例 | 「新建会话 → chat 一轮 → agent 多迭代含工具一轮 → 分支重生成 → 压缩 → 复制会话」后不存在新的 `turn_index IS NULL` 行 |
| 19 事务原子性 | 同上（注入失败） | 写入失败时计数器不递增；计数器更新失败时行不落库 |
| 20 分支重生成不新开轮 | 同上 | 分支点为 assistant 时，新行 `turn_index` 等于分支点 turn，`iteration` 在该 turn 内递增 |

### 9.2 命令

在 `src-tauri` 目录下执行 Rust 侧（禁止 `--manifest-path`）：

```
cargo test --test conversations
cargo test --test protocol_dto_roundtrip
cargo test -p assistant-storage -p assistant-agent
cargo clippy --workspace --all-targets -- -D warnings
```

前端：

```
npm run contract:gen
npm run typecheck
npm run lint
npm test
```

### 9.3 验收口径的两条显式声明

1. **存量会话的新形态收益为零**：所有历史轮无坐标 → 永久平铺、永不折叠。验收记录必须写成「新形态只对上线后新增的轮生效」，不得把「历史会话仍逐气泡」记为缺陷（ADR §3.5）。
2. **含压缩摘要的段落平铺**：`compact_summary` 的 `turn_index = NULL`（OQ3 默认值），其所在段不折叠。这是显式取舍，不是缺陷。

---

## 10 已知限制

| # | 限制 | 来源 | 处置 |
|---|---|---|---|
| L1 | 分页查询不按 `active_branch_id` 过滤（既有偏差） | ADR §3.1 注 | 不在本文处理；规格中记为已知限制，避免被误认为新形态引入 |
| L2 | 旧数据（`compact == null`）单行缺「规模 / 锚点」两列 | P1（已关闭） | 降级分支（§6.2）兜底，不得现场统计字节；新数据走正式分支 |
| L3 | 分支点位于存量行（`turn_index IS NULL`）时，重生成行无坐标 | §2.4 路径 3 | 落 `unresolved`，chrome 逐气泡，不报错 |
| L4 | `duplicate_conversation` 复制后 `next_turn_index` 沿用源值 | §2.4 路径 7 | 复制品的新轮序号与源会话同区间，但两者是独立会话，序号无全局唯一要求 |
| L5 | 子 agent 独立会话坐标空间未接入 | ADR 关联 I38f | 暂缓，不影响本文六切片 |

---

## 11 未决项

以下沿用 ADR §7 的编号，**均不阻塞** S1–S6；定稿后若与本文冲突，以定稿结论回写本文。

| 编号 | 问题 | 对本文的影响 |
|---|---|---|
| OQ1 | 存量行是否离线回填坐标 | 只影响「历史是否享受折叠」，S1–S6 按不回填实现 |
| OQ3 | `compact_summary` 的坐标归属 | 默认 `NULL`；若改为「归属被压缩的 turn」，只改 §2.4 路径 6 与 §5.4 判据 |
| OQ11 | `settled` 后是否自动收起 | 若启用，需第三态 `settling` 且判据必须是可观测用户信号（ADR D7 已限定） |
| OQ12 | 虚拟化单位 | 本文只落分页（S6），虚拟化不实现 |
| OQ14 | 计数器复位迁移 | 本文按「新库初值 1、存量库 ALTER 默认 1」实现 |
| OQ15 | chat 是否共用 `groupNodeRows` | 本文按共用实现（D13）；若拆开则门禁 15 失效 |

---

## 12 与 ADR 的差异登记

本文在实现细节上对 ADR 做了两处收窄，均不推翻决策：

| # | ADR 写法 | 本文写法 | 理由 |
|---|---|---|---|
| 1 | 事件载荷坐标写成 `turnIndex: u32`、`iteration: u32` | 落为 `Option<u32>` | 分支点位于存量行时无坐标（§2.4 路径 3），非 `Option` 会逼出假值；`None` 与 `unresolved` 同语义 |
| 2 | `CreateAssistantReq` / `AppendToolMessageReq` 未定义坐标类型 | 落为 `Option<u32>` | 内核只搬运、不判定；`None` 时写 `NULL`，与「内核不认识持久坐标」一致 |

其余字段、判据、门禁与 ADR 逐条对齐；本文不新增门禁，只给门禁落用例。
# I46 — Agent 节点布局设计（ADR）

| 项 | 值 |
|---|---|
| 状态 | Draft（设计文档，不记录实施进度） |
| 日期 | 2026-11-13 |
| 取代 | [I46 紧凑消息格式设计](./I46-agent-compact-message-format-design.md)（保留为参考记录，不再作为权威源；其信封 / 省略 / 落盘 / 预算结论仍然有效，见 §9） |
| 关联 | [I46 节点布局规格](./I46-agent-node-layout-spec.md)（实施落地：字段、SQL、写路径、切片、门禁用例）、[I18 工具架构 ADR](./I18-agent-tool-architecture-design.md)、[I38 阶段总纲](./I38-agent-roadmap.md)（阶段与门禁唯一入口）、[I36a 内核能力基线](./I36a-agent-kernel-baseline.md)（turn 事件不变式）、[I36b 内核一致性规格](./I36b-agent-kernel-conformance-spec.md)、[I38f 子 Agent 规格](./I38f-agent-subagent-spec.md)（独立会话坐标空间） |
| 影响范围 | 设计层：定义 agent 输出的**节点形态**（节点 = 行、turn = 容器）、**坐标持久化**、**容器归属**、**分组投影**、**折叠密度**、**挂载单位**。新增 `MessageDto` 两个坐标字段与两列存储；给两个**既有**事件载荷各加一个只读 `turnIndex`（不新增事件、不改事件语义）；不改分支链拓扑、不改工具结果信封。前置依赖见 §1.4 |

> 本文只回答六件事：一个 agent 节点**凭什么知道自己属于哪一轮**（坐标）；**谁拥有 chrome 与折叠**（容器）；节点**如何被聚合与收起**（分组投影）；一个多迭代 turn **占多少高度**（密度）；节点**按什么单位挂载与分页**（窗口化）；**两类 mode（有工具 / 普通 chat）如何共用同一套形态**（链路只在容器层判定）。
>
> 本文的主轴是**布局形态**。工具结果的信封语法、省略三态、落盘生命周期、预算归属不在本文重述，仍以参考记录为准（§9 给出处置表）。实施批次与进度归 [I38 §2](./I38-agent-roadmap.md) 与对应 tasklist。

---

## 1 问题

### 1.1 现状事实基线

| # | 事实 | 代码位置 |
|---|---|---|
| F1 | 坐标只存在于**事件层**：`AgentToolCallDto` 带 `turn_id` / `iteration` / `call_id`，但 `turn_id` 是进程内代际 id（`{conversationId}:{counter}`，counter 只存内存，进程重启回到 1），**不可持久** | `src-tauri/src/protocol/dto/agent.rs`、`src-tauri/crates/assistant-agent/src/kernel/tasks.rs` |
| F2 | 坐标在**落库时被丢弃**：`MessageRow` 与 `MessageDto` 都没有 `turn_id` / `iteration`；`MessageToolCallDto` 只有 `call_id` / `name` / `arguments` / `status` / `result`，也没有 `iteration` | `src-tauri/crates/assistant-storage/src/conversations/mod.rs`、`src-tauri/src/protocol/dto/chat.rs` |
| F3 | 前端坐标**只在内存**：`agentSegments` 用 `segmentMessageIdByIteration: Map<number, string>` 维护「迭代 → 消息 id」，刷新页面或切回会话即失忆，只能靠 `adoptSegment` 在补拉快照时人工绑定 | `src/stores/agent/agentSegments.ts` |
| F4 | **容器是消息**：`openSegment` 每次迭代追加一条 assistant 占位行，于是「一迭代 = 一消息 = 一气泡」；气泡边界由数据行决定，而不是由 turn 决定 | 同 F3 |
| F5 | **chrome 逐气泡**：`MessageBubble` 内 `timestamp` / `BranchPicker` / `MessageActions` 每个气泡各渲染一份；`showIdentity` 只省掉「头像 + 模式 + 模型」身份行，其余控件不省 | `src/chat/message/MessageBubble.tsx`、`src/chat/message/MessageList.tsx` |
| F6 | **节点的组织轴是「调用」**：`MessageBubble` 内 `toolCalls.map(view => <ToolCallBubble/>)`，一个调用一张卡片；调用之间没有行概念 | `src/chat/message/MessageBubble.tsx` |
| F7 | **卡片是固定高度的多段结构**：状态头 + 参数行 + 结果预览 + 折叠按钮 + 规模统计；`resultStats` / `parsePersistedOutput` 必须读正文，因此现状**无法只凭头部压成一行** | `src/chat/tool/ToolCallBubble.tsx`、`src/chat/tool/toolCallDisplay.ts` |
| F8 | tool 行**被前端整体过滤**：`messages.filter(msg => msg.role !== "tool")`，它只承担分支链拓扑，不参与渲染 | `src/chat/message/MessageList.tsx` |
| F9 | **分页单位是消息**：`MESSAGES_PAGE_SIZE = 50`，后端按 `timestamp < cursor` 取上一页；一次 turn 可能被切成两页，跨页时同一轮的过程会被拆开 | `src/config/constants.ts`、`src-tauri/crates/assistant-storage/src/conversations/msg_ops.rs` |
| F10 | **没有 turn 级 chrome、没有 turn 级折叠**：全仓唯一与「轮」有关的判据是 `showIdentity`（`index === 0 \|\| 上一条不是 assistant`），且只用于身份行 | `src/chat/message/MessageList.tsx` |
| F11 | **同一份结果在 wire 上出现两遍**：assistant 行 `toolCalls[].result` 由 tool 行回填，tool 行自身也在列表里下发，而前端丢弃后者 | `src-tauri/src/cmd/conversation_utils.rs`、F8 |
| F12 | **前端反解模型文本**：`parsePersistedOutput` 用正则锚定 `<persisted-output ...>`，`resultStats` 自行统计行数与字节 | `src/chat/tool/toolCallDisplay.ts` |
| F13 | **渲染无虚拟化**：`MessageList` 对可见消息直接 `map` 全量渲染，高度由内容决定 | `src/chat/message/MessageList.tsx` |
| F14 | **链路判定已是单点**：`linkFromModeKind` 只认 `kind === "agent"`，其余（`null` / 未知 / 旧值 / mode 行缺失）全部落 chat；`ConversationView` 用 `route.link` 决定挂哪个 layout | `src/chat/core/conversationLink.ts`、`src/chat/ConversationView.tsx` |
| F15 | **chat 是单缓冲**：`StreamStatePort.appendDelta` 注释即「chat 语义下即整轮唯一气泡」，chat 恒一轮一条 assistant 行；agent 才按 `iteration` 分段 | `src/services/stream/turnStrategy.ts` |

### 1.2 缺口

- **G1 坐标不可持久**：F1+F2+F3 让「节点属于哪一轮、第几次迭代」只能靠「连续 assistant 行」这条启发式在渲染层猜；猜错不会报错，只会把两轮并成一组或把一轮拆成两组，且无法被测试证伪。
- **G2 容器错位**：F4+F5 把 chrome 与折叠挂在**消息**上，于是 N 次迭代 = N 份时间戳 / 操作 / 分支控件。用户看到的不是「一轮的结论」，而是「一堆各自独立的对话片段」。
- **G3 节点形态单一**：F6+F7 只有「卡片」一种形态，且卡片必须读正文才能算出规模，因此**无法压成一行**；过程与结论在视觉上等重。
- **G4 高度乘法**：G2 × G3 相乘——N 个气泡 × 每气泡 M 张卡片，高度随迭代线性增长，且每份 chrome 都不承载信息。
- **G5 无分组投影层**：没有「仅展示层」的 group 概念，任何收起都只能改动数据（段表 / 消息列表），展开无法逐项还原。
- **G6 挂载单位与容器单位不一致**：F9 按消息分页、F13 按内容全量渲染，容器是 turn 但挂载单位是消息，分组天然会被页边界切开。
- **G7 结果重复传输**：F11 的同一正文传两遍，其中一遍必然被丢弃。
- **G8 前端是文本解析器**：F12 的格式漂移只会静默降级。
- **G9 两类 mode 无形态等价证明**：F14 已把链路判定收敛到单点，但没有任何测试能证明「chat 与 agent 单轮零工具渲染一致」。一旦节点布局按 `modeId` 分叉或只给 agent 写坐标（F15 的单缓冲事实说明 chat 也该有坐标），失效方式是**静默的**——形态没变、不报错，只是 chrome 回落成逐气泡（D13 / 门禁 15）。

### 1.3 目标与非目标

目标：agent 的渲染单元从「消息」改为「节点」；每个节点有**可持久、可回放**的坐标；chrome 与折叠**按 turn 挂载一次**；过程节点可压成单行且**不读正文**；折叠是**纯派生投影**、展开逐项还原；分页与虚拟化的单位与容器单位一致；结论行**永不因折叠而消失**；**有工具与普通 chat 两类 mode 共用同一套形态**，chat 靠数据天然退化、并以快照门禁证明与现状逐项等价。

非目标：不改 turn 事件语义（`turn_id` / `seq` 单调 / 终态唯一，见 I36a）；不改分支链拓扑与压缩投影；不改工具结果信封、省略口径、落盘生命周期（§9）；不为「省 token」引入摘要；不做「会话即文档」的整体重排；不在本文定义虚拟化实现与分页命令（§2.10 只给约束）；**不按 `modeId` / `TurnKind` 在渲染层分叉**（D13）。

### 1.4 前置依赖（本文生效条件）

| # | 依赖 | 现状 | 缺失后果 | 处置 |
|---|---|---|---|---|
| P1 | 参考记录的 `compact` 展示投影（`head` / `truncated` / `persisted` / `errorKind` / `anchor`） | **已落地**（2026-11-14，P13 关闭）：`ToolResultCompactDto` 已进 `MessageToolCallDto`，前端 `ToolCallView.compact` 可用 | 单行可直接拿「对象 + 规模 + 锚点」，D4 表格按正式分支实现 | 正式分支取 `compact.head` / `compact.anchor`；`compact == null`（旧数据）才走降级分支（工具名 + `summarizeArguments` + 状态 + 耗时），规模与锚点两列**省略而非伪造**（D4 / 门禁 9） |
| P2 | 消息写入事务化 | **未落地**：`insert_chat_messages` 与压缩摘要写入各自多次 `insert_message` + `update_*`，无外层事务 | `next_turn_index` 与消息行可能不同步（计数器递增成功、消息写入失败 → 序号空洞；反序 → 重复序号） | 由 D2 约束 6 要求：所有写路径必须在同一事务内完成「取号 → 写行 → 递增」 |
| P3 | 分页按 turn 聚合 | **未落地**：`get_messages_paginated` 按 `timestamp < cursor` 取消息页 | 跨页切开 turn（G6）；只能靠 §2.10 约束 1 的兜底 | 允许先按 §2.10 约束 1 兜底（切开的边界组落 `unresolved` 并禁用折叠），turn 游标命令归独立议题（OQ12） |

> P1 决定**单行的信息量**，P2 决定**坐标的正确性**，P3 决定**跨页形态**。P1 已于 2026-11-14 关闭；P2 / P3 仍不阻塞「坐标落库 + 容器归属」这批主干的实施顺序，但必须在规格文档里写成显式降级分支，不能当成「已经有了」。

---

## 2 决策

### 2.1 D1 术语：节点是行，turn 是容器，group 是投影

| 概念 | 定义 | 是否持久化 | 是否进 wire |
|---|---|---|---|
| **节点（node）** | 一个渲染行：一次工具调用、一段回复、一段 reasoning、一个边界提示 | 否（由数据派生） | 否 |
| **turn** | 一次用户输入触发的完整处理，是**容器**；拥有 chrome 与折叠控件 | 是（坐标） | 是（坐标） |
| **step / iteration** | turn 内的一次模型调用，是节点的**坐标分量** | 是（坐标） | 是（坐标） |
| **group** | 展示层的节点聚合（过程组 / 运行组） | **否** | **否** |

节点分三类，混用会让折叠判据失效：

| 类 | 内容 | 折叠行为 |
|---|---|---|
| **回复行（reply）** | 有正文的 assistant 文本 | **永不折叠**（D5） |
| **过程行（process）** | reasoning、工具调用、模型重试 | 可折叠（D4） |
| **边界行（boundary）** | turn 错误、压缩提示、token 上限、审批/提问、完成页脚 | 挂 turn 位置，**不折叠**（D8） |

理由：参考项目的共同形态正是这个三分——过程可收、回复必显、边界挂 turn（§8.1）。

### 2.2 D2 坐标必须持久化，禁止渲染层重建

`MessageDto` 新增两个字段（输出 DTO 的 `Option` 键恒在、值 `null`）：

```rust
pub struct MessageDto {
    // ...既有字段
    /// 会话内单调 turn 序号（从 1 起）。`null` = 未知（旧数据 / 未确认的乐观行 / 子 agent 未对齐）。
    pub turn_index: Option<u32>,
    /// turn 内迭代序号（从 1 起）。`0` = 非迭代行（user / 压缩摘要）。
    pub iteration: u32,
}
```

`iteration` 的口径按「行是否属于某次模型调用」划分，不按 `role` 划分：

| 行 | `iteration` | 理由 |
|---|---|---|
| `user` | `0` | 不发起模型调用 |
| `assistant`（某次迭代的正文 / 推理） | `≥ 1`，等于该次迭代 | 一迭代一行（F4） |
| `tool` | `≥ 1`，等于**发起该调用的那次迭代** | 与 `AgentToolCallDto.iteration` 逐条相等（门禁 1），否则事件与落库无法比对 |
| `compact_summary` | `0` | 不属任何迭代（坐标见 OQ3 的默认值） |

**tool 行不写 `0`**：它的坐标是「所属迭代」，不是「非迭代行」。它仍不参与渲染（F8 过滤），但坐标必须与事件一致——这是门禁 1 唯一可验证的形式。

七条硬约束：

1. **不复用 kernel `turn_id`**。它是进程内代际 id（F1），重启后与旧值重复，拿它当持久坐标会让「上一轮」与「这一轮」同 id。
2. **`turnIndex` 是逻辑序号，不是存储槽位**。只用于排序与分组，**禁止**用 `turnIndex + 1` 推算「下一轮」——删除、压缩、分支切换都会让它稀疏。
3. **坐标是事实，不是推断**。渲染层不得从 `role` / `parentId` / `timestamp` / 消息相邻关系推导 turn；缺坐标一律落 `unresolved` 组，该组不折叠（宁可平铺，不可错并）。
4. **user 行也带坐标**：它开启一轮，`turnIndex = 新值`、`iteration = 0`。这样「一轮从哪开始」不依赖 role 扫描。**唯一的例外是「分支点重生成」**：`insert_on_branch` 在 `branch_point.role == "assistant"` 时只插一条 assistant 行、不插 user 行（现状），此时新行**继承分支点所在 turn 的 `turnIndex`**，并在该 turn 内分配新的 `iteration`（不新开一轮、不落 `unresolved`）。若不写这条规则，重生成会凭空多出一轮或整段平铺。
5. **坐标不区分链路**。chat 链路同样写 `turnIndex`（每轮一个新值），`iteration` 恒 `1`（chat 单缓冲即整轮一条，见 I38d §3.7），落在「`iteration >= 1` = 属于某次迭代的行」口径内。**只给 agent 写坐标是错误实现**：chat 行会全部落 `unresolved`，按约束 3 一律平铺不折叠，`TurnTail` 定位不到轮，chrome 静默回落成逐气泡——形态没变、也不报错，属于最难发现的失效。
6. **写路径必须全部覆盖，且在事务内**。生产写入点共 5 处 + 1 处整表复制，缺一处就会产生无坐标行：

| 写路径 | 现状 | 要求 |
|---|---|---|
| `cmd/chat/insert.rs` `insert_normal_append`（user + assistant） | 两次 `insert_message`，无事务 | 同一事务内：取号 → 写 user（`iteration = 0`）→ 写 assistant（`iteration = 1`）→ 递增计数器 |
| `cmd/chat/insert.rs` `insert_on_branch`（分支点 user） | 两次 `insert_message`，无事务 | 同上（新开一轮） |
| `cmd/chat/insert.rs` `insert_on_branch`（分支点 assistant） | 一次 `insert_message`，无事务 | 同一事务内：读分支点 `turn_index` → 该 turn 内 `iteration = max(iteration)+1`（仅在该 turn 内允许，见约束 2 的例外说明） |
| `adapters/message_store.rs` `create_assistant_message`（agent 迭代行） | 单次写入 | 同一事务内：读当前 turn 的 `turn_index` → `iteration = 传入值` → 写行 |
| `adapters/message_store.rs` `append_tool_message`（tool 行） | 单次写入 | 同一事务内：写行，`turn_index` / `iteration` 取发起调用的迭代 |
| `cmd/compaction.rs` 摘要行 | 单次写入 | 同一事务内：写行，坐标取 OQ3 的默认值 |
| `conversations::duplicate_conversation` | 整表复制 | 原样复制 `turn_index` / `iteration`，并复制 `next_turn_index`（不重排） |

约束 2 的例外：**`turnIndex` 的唯一合法算术是「同一 turn 内 `iteration` 递增」**，跨 turn 一律取会话计数器新值，禁止 `max(turn_index) + 1`。

7. **实时坐标必须随事件下发，不能等终态快照**。坐标在落库时生成，而 `chat-stream-start` 每 turn 只发一次、`chat-delta` 只带 `iteration`、`chat-user-message-confirmed` 只带 id/content——若不加字段，流式期间当前 turn 恒 `unresolved`，chrome 会先逐气泡、turn 结束再一次性跳到 `TurnTail`（可见跳变）。因此给两个**既有**载荷各加一个只读字段：

| 载荷 | 新增字段 | 来源 |
|---|---|---|
| `ChatUserMessageConfirmedPayloadDto` | `turnIndex: u32` | user 行落库时分配的值 |
| `ChatStreamStartPayloadDto` | `turnIndex: u32`、`iteration: u32`（= 1） | 该 turn 的首个 assistant 行 |

turn 内后续迭代**不新增事件**：前端用「本 turn 已知的 `turnIndex`（来自 `chat-stream-start`）+ 事件自带的 `iteration`」给新开的段赋坐标——两个分量都是 wire 事实，不是相邻关系推断（不违反约束 3）。turn 结束后以 `chat-messages-changed` 的落库坐标为准，两者必须逐条相等（门禁 17）。

收益：F3 的内存 Map 从「唯一真源」降级为「加速缓存」——刷新后可由历史重建，`adoptSegment` 退化为可选优化。

**计数器落库位置**：`conversations` 表新增 `next_turn_index`（`INTEGER NOT NULL DEFAULT 1`），与消息写入同事务递增。**不得**实现成 `SELECT max(turn_index) + 1`——删消息或删 turn 会让计数器回退并产生重复序号（§7 OQ14）。

### 2.3 D3 chrome 按 turn 挂载，不按消息挂载

| 元素 | 现状（F5） | 目标 |
|---|---|---|
| 身份行（头像 / 模式 / 模型） | 连续 assistant 的首个气泡 | **turn 内首个 `role === "assistant"` 节点**（判据从「上一条是否 assistant」改为「是否本 turn 首个 assistant 节点」） |
| 时间戳（assistant 侧） | 每个气泡 | **turn 尾**（`TurnTail`） |
| `MessageActions`（复制 / 删除 / 重新生成） | 每个气泡 | **turn 尾**（仅 assistant 侧） |
| `BranchPicker` | 每个气泡 | **turn 尾**（仅 assistant 侧） |
| 失败重试入口 | 每个气泡 | **失败节点**（就地） |
| 折叠控件 | 无 | **turn**（组头） |

三条边界：

- **身份行判据不是「turn 首节点」**：turn 首节点是 user 行，而身份行只对 assistant 生效（现状 `isAssistant && showIdentity`）。判据必须是「本 turn 内首个 assistant 节点」；`compact_summary`（边界行）既不吃身份行，也不吃 `TurnTail`，保留自身渲染。
- **`TurnTail` 只承接 assistant 侧收尾**：user 行的时间戳与编辑 / 删除留在 user 节点上，不随 `TurnTail` 搬走。`MessageActions` 是 user / assistant 共用组件，**按侧分治**，不是整组上移。
- **chrome 是视图，不是数据**。`TurnTail` 只读**锚点行**的 id 与 status，不新增消息行，不改变分支链。锚点定义必须精确：

| 优先级 | 锚点 | 说明 |
|---|---|---|
| 1 | turn 内**最后一条有正文的 `settled` assistant 回复行** | 与参考项目的 `turn-tail` 口径一致；复制 / 删除 / 重新生成都以「结论」为目标 |
| 2 | 该 turn 内最后一个 assistant 节点（无回复行时，如纯工具失败轮 / 被中断的空正文轮） | 保证中断与失败轮仍有操作入口 |
| 3 | 无 assistant 节点 → 不渲染 `TurnTail` | 守卫分支；user-only 的轮（若存在）不产生收尾 |

**禁止**用「turn 末节点」当锚点：末节点可能是 tool 行（被 `role !== "tool"` 过滤掉、不在可见列表里）、过程行或边界行，按位置取会把操作挂到看不见的行上，或挂到 `compact_summary` 上。时间戳取锚点行的 `timestamp`（与现状「显示该气泡时间」语义一致）。

另：**不删除任何操作能力**，只改挂载位置。行内的编辑 / 删除仍可在 hover 时出现，但**不常驻**。

理由：chrome 属于「轮」而不属于「段」。放在段上，N 次迭代就会产生 N 份相同语义的控件（G2），而每份都占用固定高度。

### 2.4 D4 节点行三档形态

| 档 | 条件 | 渲染 |
|---|---|---|
| **单行** | 过程行，收起态 | 一行：状态图标 + 工具名 + `compact.head` + 耗时；**不读正文** |
| **展开** | 用户点开该行 | 完整卡片（复用现有 `ToolCallBubble`） |
| **整组** | 至少 1 个可折叠过程行（阈值 1）且 turn 已结束 | 组头一行 + 每步一行（回复行仍在组外，按序显示） |

单行的字段映射（全部来自结构化字段，无一读正文）：

| 元素 | 来源（`compact` 可用，新数据） | 来源（`compact == null`，旧数据兜底） | 缺失时 |
|---|---|---|---|
| 状态图标 + 成败 | `view.status` / `compact.errorKind` | `view.status` / `view.errorKind`（两者均已存在） | 图标取 `status`；`errorKind` 为 `null` 时不显示分类 |
| 工具名 | `view.name` | `view.name` | — |
| 对象 | `compact.head` | `summarizeArguments(view.arguments)`（现有实现） | `summarizeArguments` 返回 `null` → 只显示工具名 |
| 规模（行数 / 字节 / 命中数） | `compact.head` | **省略** | 不得从 `resultText` 现算（门禁 8），也不得猜 |
| 耗时 | `view.startedAt` / `finishedAt` | 同左 | 任一为 `null` → 省略 |
| 文档位置引用 | `compact.anchor` | **不存在** → 省略 | 见 D12 |

理由：F7 证明现状卡片无法压成一行；`compact.head` 是为「对象 + 规模」存在的结构化字段（§9）。**`compact == null`（旧数据）时单行信息量下降，但形态不变**——单行退化为「工具名 + 参数摘要 + 状态 + 耗时」，规模一列留白。这一列不允许用「截断正文前 N 字」或「现场统计字节」填补：前者不准且不稳定，后者违反门禁 8。

### 2.5 D5 折叠只折过程，任何回复行永不折叠

折叠判据是**内容**，不是**位置**：

- 只要节点有正文（`reply`），它就不进折叠组，无论它属于第几步；
- 折叠组只收 `reasoning` / 工具调用 / 重试这类过程行；
- 短回复（「接下来我会……」）也不折——多一行可接受，结论消失不可接受。

这条**取代**参考记录里的「末段永不折叠」。后者是位置判据，已被参考项目证伪（§8.2）：一轮在给出结论后仍可能继续迭代（子 agent 完成通知唤醒下一步、补充记录、再调一次工具），此时「最后一段」是一句补充说明甚至不存在纯文本段，真实结论会被折进过程。

### 2.6 D6 折叠是纯派生投影，永不序列化

- group 只存在于渲染层：**不进 wire、不进落库、不进 `agentSegments` 段表**；
- 展开即逐项还原：节点 id、顺序、chrome 全部由原数据重建；
- 投影函数必须是纯函数：输入节点列表，输出渲染项列表，无 I/O、无时间、无随机。

理由：一旦折叠写进数据，展开就无法还原（历史里只有「折叠后」的形态），且与分支 / fork 产生不一致状态。

### 2.7 D7 相位由 transcript 边界驱动，不用定时器

group 带 `phase: collecting | settled`：

| 相位 | 触发（唯一来源） | 渲染 |
|---|---|---|
| `collecting` | turn 正在运行（`session-state-changed` 的 `Processing`） | 不折叠，逐行展开 |
| `settled` | **终态事件已应用**：收到 `session-state-changed` 的 `Idle` / `Error`，且该 turn 的终态 `chat-messages-changed` 已并入 store | 允许折叠（仍需用户显式操作） |

**只有两个相位**：参考项目里的第三态（`settling`，即「终态已到、渲染未完成」）在本仓**不成立也不需要**——本仓的终态事件顺序是「先 `chat-messages-changed`、后 `session-state-changed`」，看到 `Idle` 时快照必然已并入；而且折叠**默认不自动触发**（OQ11），不存在「内容在用户眼前塌缩」这一风险，所以不需要一个「等渲染完成」的中间态。

两条禁令：

1. **不得**用「流静默 N 秒」或定时器判相位：静默可能只是工具执行中，定时器还会引入生命周期与重叠控制。
2. **不得**用「渲染完成」（`useEffect` 之后 / 下一帧）作为判据：它不可观测、不可测试，且与数据无关。

若将来启用「settled 后自动收起」（OQ11 的可选设置），届时才引入第三态 `settling`，且其判据必须是**可观测的用户信号**（该组已在视口内且用户未与其交互），仍不得用定时器。

### 2.8 D8 安全阀：需要用户动作的节点不折叠

以下四类**一律完整显示**，且组头显式提示待处理数量：

| 类 | 理由 |
|---|---|
| `running` 的 turn | 折叠会让用户看不见进展，组高在增量中反复跳动 |
| `awaiting_user`（审批 / 提问） | 把需要点确认的东西藏进折叠组是事故 |
| `boundary` 节点（压缩提示 / token 上限 / turn 错误） | 属于 turn 级事实，本就不在过程组内 |
| 无摘要的步（`compact.head` 与参数摘要皆空） | 压行只会得到空行，宁可高不可空 |

失败步（`error`）**允许折叠**（2026-11-14 口径调整）：失败原因已由 `compact.head` 的 `err <kind> <tool> · <subject> · <reason>` 与 `compact.errorKind` 承载，单行同时显示失败图标、原因与状态文案，组头显示本段失败数，点开该行即还原完整卡片。因此「失败原因是结论的一部分」这条旧理由被「单行即可读到原因」取代，不再构成安全阀。

**安全阀是节点级，不是 turn 级**：待处理 / 无摘要的步切断它所在的折叠段（run）并整卡显示，但**不得**因此否决同一 turn 内其他步的折叠——否则一个节点出错就让整轮全部展开，既违背「每个收起的 node 各自处理」，也让流式期与终态的组头计数互相污染。2026-11-14 口径调整后，这些步以**独立卡片节点**渲染（`run === null`），不再塞回气泡：气泡只承载正文与 chrome，这样「步 → 等待卡 → 步」的原始时间顺序不会被气泡抢占。`running` 与 `boundary` 仍是 turn 级：前者整轮处于 `collecting`、不产生任何 run；后者使该 turn 不可折叠。

### 2.9 D9 折叠不改数据，且必须可逆

门禁要求「折叠前 → 展开后」逐项一致：消息数组、节点顺序、`id`、chrome 数量、分支控件全部相同。任何需要改动段表或落库才能实现的折叠方案都不被采纳（§5）。

### 2.10 D10 挂载以 turn 为单位（边界 + 约束）

现状 F9 按消息分页、F13 全量渲染，容器是 turn 而挂载单位是消息，两者不一致（G6）。目标是把**分页游标与虚拟化单位都改成 turn**。

本文只给约束，不给实现（实现属独立议题，见 §7 OQ12）：

1. **页边界不得切开 turn**：服务端按 `turnIndex` 取页，或客户端补齐同 turn 剩余消息；做不到时该边界组标 `unresolved` 并**禁用折叠**。
2. **跨页只允许合并「持久化相邻」的同 turn 成员**：不得把两页里「看起来相邻」的节点合并成组（参考项目的修复教训，§8.2）。
3. **高度估算按行类型**（单行 / 回复行 / 展开卡片 / 组头），不按消息平均高度。
4. **`unresolved` 组不折叠、不做高度缓存**：坐标未知时任何聚合都可能错。

### 2.11 D11 `compact` 是节点行的唯一数据源（正交收益）

节点行不读正文这条要求（D4）之所以可行，是因为工具结果已有展示投影 `compact`（`head` / `truncated` / `persisted` / `errorKind` / `anchor`）。它的信封语法、省略三态、落盘定位符、预算归属全部沿用参考记录，本文**不重复定义**，只加一条额外要求：

> **`head` 必须能独立表达「对象 + 规模 + 成败」**，否则单行退化回「截断正文前 N 字」，既不准也不稳定。

这是布局对信封的唯一反向约束；信封其余部分与布局正交。

**当前状态：该投影已落地**（2026-11-14，见 §1.4 P1）。D4 的单行走正式分支取 `compact.head` / `compact.anchor`；仅 `compact == null`（旧数据）回落到降级分支（工具名 + `summarizeArguments` + 状态 + 耗时），两分支形态一致。**不得**为了「让单行好看」在渲染层自行拼 head。

### 2.12 D12 文档锚点是节点的一种行形态

当 `compact.anchor != null` 且文档面板正打开该 `path` 时，该节点行渲染为「文档第 N 行」引用（可点击跳转），而不是卡片：

| 条件 | 形态 |
|---|---|
| `anchor != null` 且面板打开该 `path` | 单行引用（`head` + 「第 N 行」） |
| `anchor != null` 但面板未打开 | 卡片 + 「打开文件」入口（不静默切面板） |
| `anchor == null` | 卡片 / 单行（按 D4 密度档） |

三条沿用结论（参考记录的 D10/D11 未被推翻）：

1. 锚点只来自**结果时刻的实际落点**，不得由 `args` 估算；
2. 锚点是位置不是内容，diff 行内容仍由 `result` 文本承载，不新增第二份 diff 字段；
3. 选路只在**单点渲染器**内进行，判据只有 `anchor` 与面板状态，**不看** `role` / `modeId` / `TurnKind` / 工具名。

### 2.13 D13 链路判定只发生在容器层，节点层禁止读 `modeId`

两类 mode（`kind === "agent"` 与其余全部归 chat）在形态上**同构**，差异只在能力，不在渲染：

| 场景 | 消息行 | 工具节点 | 节点布局判定 | 与现状关系 |
|---|---|---|---|---|
| chat turn | 1 | 0 | 单条档 | 必须逐项一致（门禁 15） |
| agent turn，直接回答 | 1 | 0 | 单条档 | 必须逐项一致（门禁 15） |
| agent turn，多迭代 | N | ≥ N | 折叠档 | 新增形态 |

第二行与第一行**渲染结果必须完全相同**——agent 模式下不调工具直接回答，就该长得和 chat 一样。所以形态判据只能是**节点数 + 终态**，不能是 modeId：若按 modeId 分叉，第二行会被套上 agent 外壳（工具槽、迭代段、组头），这正是「按 modeId 判渲染」的直接故障。

两条约束：

1. **上游只判存在性**：`resolveConversationRoute(...).link` 只决定「挂哪个 layout」（即有没有工具槽 / 有没有迭代段），判定点唯一，保持现状不改。
2. **节点层零读取**：进入节点布局后，禁止用 `modeId` / `link` / `TurnKind` 做任何形态或折叠决策。`NodeGroupView` 上带一个**只读** `link` 字段，仅用于断言与调试，不参与分支。

判据本质：不是「现在有没有工具」，而是「这一轮允不允许有多个 iteration」——这是**容器属性**，不是**渲染判据**。

---

## 3 规范

### 3.1 坐标模型与 wire 增量

```rust
pub struct MessageDto {
    // ...既有字段
    pub turn_index: Option<u32>,   // 会话内单调，从 1 起；null = unresolved
    pub iteration: u32,            // turn 内迭代，从 1 起；0 = 非迭代行（user / 压缩摘要）
}
```

存储：`messages` 表新增两列，沿用本仓既有的 pragma 守卫 `ALTER TABLE` 模式：

| 列 | 类型 | 默认 | 说明 |
|---|---|---|---|
| `turn_index` | `INTEGER` | `NULL` | 旧行为 `NULL`，不猜测回填（§7 OQ1） |
| `iteration` | `INTEGER NOT NULL` | `0` | user / `compact_summary` 为 `0`；assistant / tool 行为所属迭代（`>= 1`，见 D2 口径表） |

节点坐标 = `(turnIndex, iteration, callId?)`；`callId` 仅过程行需要，回复行取迭代级。排序键 = `(turnIndex, iteration, 节点类顺序, callId 首次出现顺序)`，其中节点类顺序固定为 `reasoning < reply < process < boundary`。

**不得**用 `turnIndex` 做算术推算（D2 约束 2）；稀疏是合法状态。

**排序的 SQL 落点**（现状所有查询都按 `timestamp ASC, rowid ASC`，新键必须给出 `NULL` 的落位，否则旧数据会被排到最前或最后）：

```sql
ORDER BY (turn_index IS NULL) ASC,   -- 有坐标的行在前，unresolved 行整体垫底
         turn_index ASC,
         iteration ASC,
         timestamp ASC,
         rowid ASC                    -- 同迭代同时间戳的稳定兜底
```

| 位置 | 现状 | 要求 |
|---|---|---|
| `msg_ops::get_messages` | `ORDER BY timestamp ASC, rowid ASC` | 改为上表排序键 |
| `msg_ops::get_messages_paginated` | `WHERE timestamp < ?2 ORDER BY timestamp DESC LIMIT ?3` | 游标改为 `(turn_index, iteration, timestamp, rowid)` 四元组；`turn_index IS NULL` 段单独成页（P3 / OQ12） |
| `branch::trace_active_branch` | 按父链逐级 `SELECT`，结果顺序由链决定 | 链顺序即 turn 顺序的**唯一权威**；坐标只做校验，不做拓扑 |
| 索引 | `idx_messages_conv_ts (conversation_id, timestamp)` | 新增 `idx_messages_conv_coord (conversation_id, turn_index, iteration)`；旧索引保留（按时间排序的导出 / 搜索仍在用） |

> 分页查询现状**不按 `active_branch_id` 过滤**（分支树由 `trace_active_branch` 另行求），改 turn 游标不会修正这一点；它是既有偏差，不在本文处理，但规格里必须写成已知限制，避免被误认为「新形态引入」。

### 3.2 容器归属表

| 元素 | 挂载单位 | 唯一性 |
|---|---|---|
| 身份行 | turn 内首个 `role === "assistant"` 节点 | 每 turn 至多一份 |
| 时间戳 / `MessageActions` / `BranchPicker`（assistant 侧） | turn 尾（`TurnTail`） | 每 turn 至多一份 |
| 时间戳 / 编辑 / 删除（user 侧） | user 节点自身 | 每 user 节点一份（不上移） |
| `compact_summary` | 边界节点自身渲染 | 不参与身份行与 `TurnTail` |
| 折叠组头 | turn（过程组） | 每 turn 至多一份 |
| 审批卡 / 提问卡 | 对应过程节点 | 每节点一份，不折叠 |
| 工具卡片 | 过程节点 | 每节点一份 |
| 分支拓扑（tool 行） | 数据层，不渲染 | 不变（F8） |

**同一套挂载规则对 chat 与 agent 生效**（D13）：chat 轮天然是「1 个 user 节点 + 1 个 assistant 节点」，因此 `TurnTail` 必然只有一份、组头必然不出现——退化到与现状逐项一致，不需要额外分支。

### 3.3 分组投影结构

```ts
/** 仅展示层归属；永不写入会话或传输载荷。 */
interface NodeGroupView {
  turnIndex: number | null;
  nodes: NodeView[];
  phase: "collecting" | "settled";
  isLastGroupInTurn: boolean;
  needsAttention: boolean;
  /** 只读容器属性，仅供断言与调试；禁止参与形态 / 折叠决策（D13）。 */
  link: "chat" | "agent";
  stats: { steps: number; failed: number; durationMs: number | null };
}
```

- `stats` 只由 `view` 元数据与节点数算出，**不读正文**；
- `needsAttention` 来自 `awaiting_user` / `error`，驱动 D8 安全阀；
- `phase` 由 D7 的边界事件写入，不由计时器写入；
- `link` 是容器属性快照，**不参与任何分支**（D13 约束 2）；
- 结构里没有「是否展开」字段——展开态是 UI 本地状态，不进投影对象（避免投影被 UI 状态污染）。

### 3.4 折叠判据与三档密度

```
可折叠 = 过程行数 >= 阈值（默认 1）
      && phase === "settled"
      && 无 awaiting_user / 无摘要节点（它们只切断所在 run，不否决整组）
      && turnIndex !== null（unresolved 不折叠）
展开态 = 用户手动展开 || phase !== "settled"
```

| 档 | 条件 | 渲染 |
|---|---|---|
| 单条 | 无可折叠过程行（0 步），或 `turnIndex === null` | 现状：回复行 + 逐节点卡片 |
| 折叠 | 过程行 ≥ 阈值（1）且 `settled` | 组头 + 每步一行；**回复行全部留在组外** |
| 展开 | 手动展开或非 `settled` | 逐节点还原 |

高度账（单步行 24 px、组头 28 px、回复行约 22 px/行）：

| 场景 | 现状 | 折叠后 |
|---|---|---|
| 1 迭代 × 2 调用 | ≈ 1 气泡 chrome + 2 卡片 ≈ 160–400 px | ≈ 组头 28 + 2 步 × 24 ≈ 76 px |
| 8 迭代 × 2 调用 | ≈ 8 份气泡 chrome + 16 张卡片 ≈ 1300–3000 px | ≈ 组头 28 + 16 步 × 24 + 回复行 ≈ 600 px |

注意最后一行：因为回复行不折叠（D5），折叠后高度不会压到「只有组头」。这是**刻意的代价**——把结论留在视口里，比把高度压到最小更重要。

**chat 与「agent 直接回答」都落在单条档**（D13）：过程行 0 < 阈值，形态与现状一致。三档密度是**数据驱动**的，`modeId` 不参与判定。

### 3.5 运行组结构

```
┌ 12 步 · 8.4s · 1 失败                          [展开]   ← 组头，一行
├ ✓ read  src/a.ts · 120 行 · 40ms                        ← 步行，一行
├ ✓ grep  "compact" · 12 命中 · 90ms
├ ✓ edit  src/b.ts · 第 40-52 行 · 13 行
└ ✗ bash  npm test · 失败                                 ← 失败步同样压行，原因取自 compact.head，点开即还原整卡
  （组外）回答正文……                                      ← 回复行，始终可见
```

- 组只包**过程行**；回复行按原序穿插显示，不进入组内；
- 步行点击 → 就地展开该步卡片，不展开整组；
- 组头字段全部可算：步数 = 过程行数、耗时 = 首末节点时间差、失败数 = `status` 统计。
- 上图中的「120 行」「12 命中」「第 40-52 行」来自 `compact.head` / `compact.anchor`；**旧数据（`compact == null`）时这三列整列省略**，步行退化为「✓ read src/a.ts · 40ms」（D4），不得用正文截断代替。

### 3.6 落地位置

| 位置 | 改动 | 不改 |
|---|---|---|
| `MessageList` | 新增 `groupNodeRows(messages, link)` 纯函数 + `TurnTail`；**props 增加 `link: "chat" \| "agent"`**（由 `ConversationFrame` / `ConversationView` 从 `route.link` 透传） | `role !== "tool"` 过滤、流式滚动逻辑 |
| `MessageBubble` | `showIdentity` 判据改为「本 turn 内首个 assistant 节点」；`toolCalls.map` 换成 `NodeRow` 渲染；本 turn 有工具步时 `reasoning` 上移为独立 `ReasoningNode`（气泡不再承载） | 正文 markdown、零工具 turn 的气泡内 reasoning、error / retry 行为、`compact_summary` 渲染 |
| `TurnTail` | 新增：只承接 assistant 侧时间戳 / 操作 / 分支，锚点按 D3 三级优先级 | user 侧 chrome 不动 |
| `ToolCallRenderer` | 单点选路（卡片 / 单行 / 文档引用） | 不读 `role` / `modeId` / `TurnKind` / 工具名 |
| `agentSegments` | 保留（仍是写入侧的段表）；新增「本 turn `turnIndex`」的会话级缓存，供流式期赋坐标 | **不因折叠而改**；坐标持久化后它降级为缓存 |
| `protocol/dto/chat.rs` | `MessageDto` 加 `turn_index` / `iteration`；`ChatUserMessageConfirmedPayloadDto` 加 `turnIndex`；`ChatStreamStartPayloadDto` 加 `turnIndex` / `iteration` | 既有字段语义、事件名、`seq` 语义 |
| `protocol/dto/branch.rs` | `ChatMessagesChangedPayloadDto` 不变（坐标随 `MessageDto` 走） | — |
| 后端·分配 | 新增 `conversations.next_turn_index` 与「取号 → 写行 → 递增」的事务化辅助函数（唯一入口） | 事件语义、分支拓扑、结果信封、`resolveConversationRoute` 判定口径 |
| 后端·写路径 | D2 约束 6 的 7 条路径全部改为走该入口；`conversation_utils` 映射坐标 | 压缩投影、`is_full` 判定 |
| 后端·读取 | `msg_ops` 排序键与索引（§3.1） | `trace_active_branch` 的拓扑求法 |

`groupNodeRows` 的签名必须带 `link`：`NodeGroupView.link` 是容器属性快照，投影函数若只收 `messages` 就拿不到它，会退化成「渲染层自己去读 store」，等于把链路判定重新散开（违反 D13 单点原则）。`link` 只写入投影对象、**不参与任何分支**。

### 3.7 迁移与兼容

- **旧数据不回填坐标**（倾向，见 OQ1）：`turnIndex = null` → `unresolved` 组 → 平铺渲染、不折叠、不报错。理由是「回填值」与「真实值」不可区分，会让错误的 turn 边界变成不可证伪的事实。
- **存量会话的新形态收益为零，且这是显式取舍**：所有历史轮都无坐标，因此永远平铺、永不折叠。验收口径必须写成「新形态只对**上线后新增的轮**生效」，不得把「历史会话仍逐气泡」记为缺陷。若产品要求历史也享受折叠，唯一合法路径是「一次性离线回填 + 标记来源」（OQ1 的另一分支），不得在渲染层猜。
- **旧前端兼容**：新字段为可选，旧前端忽略后行为与今天一致（逐气泡）。
- **压缩摘要行的坐标是已知未定项**（OQ3）：默认 `turnIndex = null` 会让「含压缩摘要的那一段」落 `unresolved`，从而**平铺不折叠**——即被压缩过的会话会有一段不享受折叠。这是显式取舍（宁可平铺，不可把摘要误并进某个 turn）；若 OQ3 改为「归属被压缩的 turn」，则须同时保证该 turn 内 `iteration` 不冲突。
- **契约**：DTO 改动后必须 `npm run contract:gen` 并提交生成物（`src/types/protocol.contract.ts`）。
- **DB**：pragma 守卫的 `ALTER TABLE` 自动补齐存量库，不阻塞启动；`next_turn_index` 的初始值必须是 `1`，**不得**初始化为 `max(turn_index)+1`（旧库没有该列时无法算出，也不需要——旧行不参与新分配）。

---

## 4 门禁

1. **坐标一致**：落库 `turn_index` / `iteration` 与事件逐条对应——`assistant` 行对 `chat-delta` / `chat-stream-start` 的 `iteration`，`tool` 行对 `agent-tool-call` 的 `iteration`（tool 行**不写 `0`**）；`turn_id` 只作进程内代际校验，不参与持久化。
2. **重启等价**：同一会话重启前后，节点分组、顺序、chrome 数量逐项相等（覆盖 F3 的失忆场景）。
3. **禁用启发式**：渲染层不得从 `role` / `parentId` / `timestamp` 推导 turn；测试须覆盖「两个相邻 turn 都是 assistant 行且无坐标差异」时**不被合并**。
4. **回复行不折**：任何含正文的节点永不进入折叠组；测试须覆盖「结论给出后仍继续迭代一次」，结论仍在视口内（§8.2 的真实故障形态）。
5. **折叠纯投影**：折叠前后 `messages` 数组、wire、落库逐项不变；展开后节点顺序与 `id` 与折叠前一致。
6. **相位不靠定时器**：`phase` 只有两个取值，`settled` 只在「终态 `chat-messages-changed` 已并入 store 且 `session-state-changed` 为 `Idle` / `Error`」时置位；测试注入长时间静默、以及「终态事件已到但快照未并入」都不改变 `phase`。
7. **安全阀**：`awaiting_user` / 无摘要的步切断其所在折叠段并渲染为独立卡片节点（`run === null`），不阻止同 turn 其他步折叠；失败步（`error`）允许折叠，其单行必须同时显示失败状态与 `compact.head` 中的原因，组头须显示该段失败数；`running` 期间整轮不产生 run；含 `boundary` 的 turn 不折叠。
8. **收起态不读正文**：收起态不得调用 `parsePersistedOutput` / `resultStats` / markdown 解析（用 spy 断言零调用）。
9. **单行可读**：`compact == null`（旧数据）时单行退化为「工具名 + `summarizeArguments` + 状态 + 耗时」，不得渲染为空行，也不得为凑信息量去解析 `resultText`（规模列留白，见 D4）。
10. **跨页不重建**：分页边界不得合并「非持久化相邻」的同 turn 节点；`unresolved` 组不折叠。
11. **选路不看身份**：`ToolCallRenderer` 不读 `role` / `modeId` / `TurnKind` / 工具名；测试覆盖「同一 turn 内一个节点有锚点、一个没有」。
12. **无新增 wire 负担**：除坐标两字段外，分组与折叠**不产生任何 wire 字段**。
13. **契约同步**：DTO 改动后 `npm run contract:gen` 并提交生成物；禁止手改生成类型。
14. **旧数据可用**：无坐标历史正常渲染（单轮平铺、不折叠、不报错）。
15. **退化等价**：chat 链路、以及 agent 链路中「单轮、零工具」的 turn，在节点布局下的渲染必须与现状**逐项一致**（快照比对：时间戳位置、`MessageActions`、`BranchPicker`、身份行显隐、`compact_summary` 渲染）。这条锁死「节点布局不得顺手改 chat 外观」，也是 D13 唯一可验证的形式。
16. **节点层零读链路**：`groupNodeRows` 及其下游不得读 `modeId` / `link` / `TurnKind` 做形态或折叠决策（用 spy 或源码断言）；`NodeGroupView.link` 只许出现在断言里。
17. **流式期坐标不跳变**：流式进行中，当前 turn 的节点必须**已有** `turnIndex`（来自 `chat-stream-start` / `chat-user-message-confirmed`）与 `iteration`（来自 `chat-delta`），不得出现「流式期平铺 → 终态瞬间成组」；turn 结束后落库坐标与流式期坐标逐条相等。
18. **写路径无遗漏**：D2 约束 6 表中 7 条路径全部产出坐标行——用「新建会话 → chat 一轮 → agent 多迭代含工具一轮 → 分支重生成 → 压缩 → 复制会话」一条端到端用例断言「不存在 `turn_index IS NULL` 的新行」。
19. **事务原子性**：注入「消息写入失败」时，`next_turn_index` 不得递增（无序号空洞）；注入「计数器更新失败」时消息不得落库。测试用回滚断言，不允许出现「行已写入但计数器未变」的中间态。
20. **分支重生成不新开轮**：`branch_point.role == "assistant"` 的重生成，新行 `turn_index` 等于分支点所在 turn（门禁 2 的分组不变），`iteration` 在该 turn 内递增（D2 约束 4 的例外）。

---

## 5 备选方案（不采纳理由）

| 方案 | 不采纳理由 |
|---|---|
| 保留消息做容器，只把卡片压小 | 治不了 G4 的乘法：N 气泡 × M 卡片的固定 chrome 仍在；且每份 chrome 不承载信息 |
| 用 `parentId` 链推导 turn | 编辑 / 分支切换会改变父链，tool 行也参与拓扑；推导结果不可证伪，与 G1 同病 |
| 复用 kernel `turn_id` 作持久坐标 | 它是进程内代际 id（F1），重启后重复；持久坐标必须是新分配的逻辑序号 |
| 用时间戳间隔猜 turn 边界 | 长任务、并发、崩溃恢复都会打破间隔假设；且同一毫秒内的多轮无法区分 |
| 按 `role` 分叉渲染形态 | `role` 是传输概念，不携带「属于哪一轮」；探索型与创作型调用共用 `assistant`，会一并改掉 |
| 按 `modeId` / `TurnKind` 分叉折叠 | `modeId` 是会话级身份且已被策略选路独占；agent 链路也会一轮答完，按模式折叠会给最常见短对话加壳 |
| 整体重写消息渲染为文档流 | 需重做用户气泡、reasoning、error / retry、分支控件、消息操作；收益仅是换排版，且失去消息级操作 |
| 折叠所有迭代（含回复），只留结论 | 参考项目已证伪：位置判据会把真实结论折进过程（§8.2）；用户多点一次才能看到答案 |
| 「末段永不折叠」（参考记录原方案） | 位置判据；回答后继续迭代时「末段」可能是补充说明甚至不存在，结论仍会消失 |
| 在 `agentSegments` 段表里实现折叠 | 渲染态写进数据态，展开无法还原，且与分支 / fork 不一致（D6） |
| 持久化「哪个组被展开过」 | UI 本地偏好不是会话事实；进历史会与分支切换产生矛盾状态 |
| 只做前端虚拟化、不改坐标 | 分组仍靠启发式，跨页仍会切开分组（G6）；虚拟化只是把错误分组渲染得更快 |
| 给 tool 行单独存一份 turn 坐标 | 重复事实：tool 行只承担拓扑，坐标随 assistant 行即可定位其归属 |
| 定时器判相位 / 自动收起 | 参考项目明确禁止由定时器推导相位；自动收起会让内容在用户眼前塌缩 |
| 为节点布局新开一套 wire（节点 DTO 流） | 会变成第二个结果通道；坐标两字段 + `compact` 已足够，节点是**派生**而非**存储** |
| chat 保留独立渲染路径、只给 agent 上节点布局 | 两套渲染必然漂移（同一份时间戳 / 操作控件要维护两遍）；且「agent 直接回答」与 chat 本应同形，分开后无法用一条门禁证明等价（D13 / 门禁 15） |
| 只给 agent 写坐标、chat 留 `null` | chat 全部落 `unresolved` → 平铺不折叠 → `TurnTail` 定位不到轮 → chrome 静默回落逐气泡：形态没变也不报错，是最难发现的失效（D2 约束 5） |

---

## 6 影响

- **契约**：`MessageDto` 新增 `turnIndex`（`Option<u32>`）与 `iteration`（`u32`）；`ChatUserMessageConfirmedPayloadDto` 新增 `turnIndex`、`ChatStreamStartPayloadDto` 新增 `turnIndex` / `iteration`（D2 约束 7，均为只读新增、无删除、无事件名变化）；旧前端忽略即退化为现状。运行 `npm run contract:gen` 后提交 `src/types/protocol.contract.ts`。
- **存储**：`messages` 表加两列（pragma 守卫），新增 `idx_messages_conv_coord`（§3.1），`conversations` 表加 `next_turn_index` 计数器（初值 `1`、同事务递增），`msg_ops` 读写与排序同步。
- **后端·写**：D2 约束 6 的 7 条写路径（chat 普通追加 / 分支 user / 分支 assistant / agent 迭代行 / tool 行 / 压缩摘要 / 会话复制）全部改为走「取号 → 写行 → 递增」的**单一事务化入口**；`conversation_utils` 映射坐标；`MessageToolCallDto` 不新增 `iteration`（其归属由所在行的 `iteration` 决定）；**chat 与 agent 两条链路都写坐标**（D2 约束 5），不做链路分支。
- **后端·读**：`get_messages` / `get_messages_paginated` 换排序键与游标；`trace_active_branch` 仍按父链求拓扑（坐标只做校验）。
- **前端·形态**：`MessageList` 新增 `groupNodeRows(messages, link)` 纯函数与 `TurnTail`，props 透传 `link`；`MessageBubble` 的 chrome 迁出、`showIdentity` 改读坐标；`ToolCallRenderer` 单点选路。**两条链路共用同一套渲染，chat 靠数据天然退化**（D13），不新增链路分支。
- **前端·坐标**：新增「本 turn `turnIndex`」的会话级缓存（供流式期赋坐标），`agentSegments` 保留为写入侧段表并降级为缓存。
- **前端·密度**：组头 + 步行 + 组外回复行；收起态只读 `compact.head` 与 `view` 元数据；旧数据（`compact == null`）按 D4 降级（规模列留白）。
- **测试**：门禁 1–20 各一组；其中新增的三类硬用例是「流式期坐标不跳变（17）」「7 条写路径无 `turn_index IS NULL` 新行（18）」「写失败时计数器不递增（19）」。
- **前置依赖**：P1（`compact` 投影）已关闭；P2（写事务化）、P3（turn 分页）按 §1.4 显式降级，**不得**当成已具备能力实现。
- **不做**：turn 事件语义、压缩投影、审批链、权限网关、分支拓扑、结果信封（§9）、分页命令与虚拟化实现（§7 OQ12）。

---

## 7 未决问题

1. 旧数据是否回填 `turn_index`：当前倾向**不回填**（回填值与真实值不可区分，会让错误边界成为事实）；若回填，需接受「一次性启发式固化」并标记来源。
2. `turn_index` 取值域：会话内单调序号（倾向）还是全局唯一 id？前者排序即顺序，后者便于跨会话引用。
3. 压缩摘要行的坐标：`turnIndex = null` + `iteration = 0`，还是归属被压缩的 turn（会影响「一轮是否连续」的判断）。
4. 分支 / fork 后 `turnIndex` 是否重排：倾向「fork 复制保持原值、不重排」，需确认分支树排序键是否只用 `turnIndex`。
5. 交错输入（一轮进行中用户又发消息）时 turn 边界如何定义：新消息开新 turn，还是排队进同一 turn？
6. 子 agent 会话是否共用坐标空间（I38f）：倾向独立会话、独立 `turnIndex`，父 turn 只引用其 id。
7. `TurnTail` 是否收纳 token 用量与模型名（当前挂在消息上）：收纳后「一轮一份」更一致，但会与身份行重复。
8. 单行节点是否允许两行（如失败原因需要一行摘要 + 一行原因）：会破坏「每步一行」的高度账。
9. 折叠阈值：**已定案（2026-11-14）为 1**——1～2 步也成组，「读一个文件再改一处」（2 步）折叠为两行；单步 run 渲染「组头 + 1 行」（≈ 52 px）仍低于一张卡片，故不再保留「≥ 3」口径。
10. 是否需要「整组一次性展开」快捷操作，及其与消息级操作的冲突边界。
11. `settled` 之后是否自动收起（参考项目有 `collapseTiming` 设置项）：倾向不自动，改为可选设置。
12. 分页改 turn 游标：需要后端新命令，还是前端聚合现有 50 条消息页（后者无法保证不切 turn）。
13. 虚拟化高度估算口径：按行类型常量，还是按内容测量（后者会触发正文读取，违反门禁 8）。
14. `next_turn_index` 计数器的复位与迁移：会话删除 / fork / 分支切换后是否重算；倾向「fork 复制当前值、删除随会话一并删除、永不回退」。计数器回退会产生重复 `turnIndex`，必须显式禁止 `max()+1` 实现。
15. chat 是否与 agent 共用同一 `groupNodeRows`：倾向**共用**（差异靠数据退化，门禁 15 保证等价），代价是 chat 也走一遍投影；替代方案是保留 chat 原路径，但那会引入两套渲染，后续必然漂移。
16. 分支切换后「同一 turn 内的 `iteration` 是否重排」：D2 约束 4 的例外允许「分支点重生成」在同一 turn 内递增 `iteration`，但若用户在同一 turn 内多次切换分支，旧分支行仍占 `iteration` 值，导致同一 turn 内出现「同 `iteration` 不同分支」。当前倾向**不重排**（`iteration` 在 turn 内单调、不保证连续、不保证唯一），`callId` 级顺序仍由链决定；需确认是否有消费者假设「同 turn 内 `iteration` 唯一」。
17. `compact.head` 的生成归属（P1 落地时）：由内核在生成结果时写入，还是由落库层对结果信封做一次投影？倾向**内核写入**（信封的省略口径在 I46-compact 里已定义，落库层再算会产生第二真源）；本文只依赖「它存在且不读正文」这一条。
18. `chat-stream-start` 是否已有 `iteration`：D2 约束 7 假设它可携带首个 assistant 行的 `iteration`（= 1）。若实现上该事件在落库前发出、拿不到坐标，则退化为「`chat-stream-start` 只带 `turnIndex`，`iteration` 由首个 `chat-delta` 提供」——两种方案对投影等价，需在规格里定一种，不得两种并存。

---

## 8 参考项目研究结论

### 8.1 三个项目是同一形态

| 项目 | 容器 | 行 | 坐标来源 | per-turn chrome |
|---|---|---|---|---|
| DeepSeek Harness | turn | 业务节点（`order` 扁平键序列） | `Location` 一等事实，四形状 `session / turn / step / unresolved` | turn-tail（footer / branch / copy 挂在末条 assistant 回复） |
| Codex（TUI） | turn | `HistoryCell` 异构行 | `Turn` 数组 + 持久化成员相邻 | 无逐行 chrome；活动组按 turn 聚合 |
| OpenBitFun | `DialogTurn` | `FlowItem`（`ModelRound.items`） | `DialogTurnIdentity { ordinal, storageTurnIndex, state }`，品牌类型区分「逻辑序号 / 存储槽位」 | 每 turn 一份 |

三者的共同点，恰好是本文的四根支柱：坐标持久、容器是 turn、分组是展示投影、挂载以 turn 为单位。

### 8.2 采纳

| 经验 | 出处 | 落点 |
|---|---|---|
| 坐标是一等事实，且 `unresolved` 是合法状态（缺前序边界 ≠ 会话级） | DSH `Location` | D2 约束 3 / §3.1 |
| 逻辑序号与存储槽位必须分开，序号可稀疏 | OpenBitFun `DialogTurnIdentity` | D2 约束 2 |
| 分组是展示层归属，**永不序列化** | OpenBitFun `grouping/types.ts` 首行注释 | D6 / §3.3 |
| 相位由 transcript 边界推导，**永不来自静默流或定时器** | 同上 `phase` 注释 | D7 / 门禁 6 |
| 组带 `isLastGroupInTurn` / `wasCutByCritical`，即「组内最后」与「被关键边界切断」是显式字段 | 同上 | §3.3 / D8 |
| 折叠只折过程，**每条回复行留在折叠之外**；位置不是内容 | DSH `keep-assistant-replies`（#6132 / #5429） | D5 / 门禁 4 |
| per-turn chrome 挂在 turn 末条回复（`turn-tail`） | DSH 同上 | D3 |
| 跨页修复只合并「同 turn 的持久化相邻成员」 | Codex `join_older_activity_group` | D10 约束 2 / 门禁 10 |
| 组头统计只由元数据算出（步数 / 失败数 / 耗时） | Codex `ActivityGroup` | §3.3 `stats` |

### 8.3 拒绝

| 参考项目做法 | 拒绝理由 |
|---|---|
| DSH 的 Assembler / Definition / Context / Location data store 全套装配引擎 | 那是为「多业务共享事件流 + 双向分页重放」设计的通用引擎；我们的业务只有 chat / agent 两条链路，引入等于自建一套框架，收益不抵复杂度 |
| DSH 的「节点即业务对象、每业务一个渲染器」注册表 | 我们只有一个渲染点（`ToolCallRenderer`），没有 per-tool 视图注册表这笔债（参考记录 §7.3 已记） |
| OpenBitFun 的四层 `DialogTurn > ModelRound > ModelRoundAttempt > FlowItem` 全量落库 | 我们已有「一迭代一行」的段表（F4），只需补坐标，不需要第四层对象；多一层会让历史与现有 DTO 双轨 |
| 定时器 / 静默判相位 | 见 D7 |
| 把展开态写进持久化 | 见 §5 |

### 8.4 本项目自研

1. **持久坐标从内核代际 id 迁到 `turnIndex`**：三个参考项目都有持久 turn 序号，我们的内核 `turn_id` 是进程内计数器（F1），必须新增列而不是复用。
2. **`agentSegments` 内存表降级为缓存**：参考项目没有「迭代 → 消息 id」的进程内映射；我们保留它作写入侧段表，但读侧以坐标为准（D2 收益）。
3. **节点行与 `compact` 信封的衔接**：参考项目用结构化 content block 直接给渲染器；我们的结果是文本信封 + 展示投影，因此「单行可读」必须由 `head` 契约保证（D11）。
4. **`unresolved` 组的平铺策略**：参考项目在分页缺口上做重放修正；我们选择「不折叠、不聚合、不报错」的保守策略（门禁 14）。
5. **两类 mode 同构渲染**：参考项目按「业务 / 会话类型」分渲染注册表，我们反过来——chat 与 agent **共用同一套节点渲染**，差异由数据退化（chat 恒一轮一 assistant 节点），并以快照门禁证明等价（D13 / 门禁 15）。理由是本仓的链路判定已集中在 `resolveConversationRoute` 单点，渲染层再分叉一次即出现第二个真源。

---

## 9 与参考记录的关系

[I46 紧凑消息格式设计](./I46-agent-compact-message-format-design.md)保留为参考记录，不再作为权威源。处置如下：

| 参考记录内容 | 本文处置 |
|---|---|
| 三段信封（head / body / tail）、省略三态、口径分离、保留策略分裂、落盘生命周期、预算归属、`compact` wire 字段 | **保留有效**，降为 D11 的「正交收益」，本文不复述；**`compact` 展示投影已于 2026-11-14 落地**（`ToolCallView.compact` 可用，后端 `ToolResultCompactDto` 已进契约），按 §1.4 P1 走正式分支，旧数据按降级分支兜底 |
| 门禁 1–12（字节相等、head 单行、三态一致、round-trip、一次传输、事件、口径分离、定位符非解析、分类可回放、清理安全、锚点结果时刻、形态不越界） | **保留有效**，仍约束信封与结果层 |
| D10 文档形态渲染 | 降为 D12：节点行的一种形态，不再是主轴 |
| D11 切换点（单点替换渲染函数、拒绝按 role / modeId / 整体重写） | **保留有效**，并入 D12 约束 3 与门禁 11；其中「拒绝按 `modeId` 分叉」升级为独立决策 D13（明确 chat / agent 同构、坐标不区分链路、门禁 15 退化等价） |
| D12 turn 级密度与「末段永不折叠」 | **被 D5 取代**：改为「所有回复行永不折叠」 |
| 门禁 13–15（选路不看身份、折叠不改数据、安全阀） | 重写为本文门禁 11 / 5 / 7 |
| 事实基线 F1–F19、缺口 G1–G10 | 本文按布局视角重新编号；信封相关事实仍以参考记录为准 |

**主轴差异一句话**：参考记录回答「一次工具结果长什么样」，本文回答「一堆节点如何组成一轮、一轮如何收成一段」。
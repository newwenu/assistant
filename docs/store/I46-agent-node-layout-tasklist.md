# I46 — Agent 节点布局 任务清单

> 规格：[I46-agent-node-layout-spec.md](./I46-agent-node-layout-spec.md)
> 决策源：[I46-agent-node-layout-design.md](./I46-agent-node-layout-design.md)
> 前置：规格 §1.2 三条开工门中 **P1 `compact` 已关闭**（2026-11-14，P13 落地）；P2 事务 / P3 分页仍按降级分支处理，**不阻塞**本清单。

六个批次逐条对应规格 §8：S1 存储与契约 → S2 写路径 → S3 事件与前端数据层 → S4 容器层 → S5 分组与密度 → S6 分页。依赖顺序 S1 → S2 → S3 → S4 → S5 → S6；S4 可在 S2 后提前，S5 必须等 S3。

> 本清单不管理提交。提交粒度按代码依赖与可编译性现场决定；这里只回答「代码改没改、验收跑没跑」。

---

## 阶段 S1 — 存储与契约（已完成 2026-11-13）

### S1.1 表结构与读写口径

对应规格 §2.1 / §2.7 / 门禁 2、14。

- [x] `assistant-storage/src/db.rs` `CREATE TABLE conversations` 追加 `next_turn_index INTEGER NOT NULL DEFAULT 1`
- [x] `db.rs` `CREATE TABLE messages` 追加 `turn_index INTEGER` 与 `iteration INTEGER NOT NULL DEFAULT 0`
- [x] `conversations.next_turn_index` 存量守卫落在 `has_conv_workspace_root` 之后（分支外）
- [x] `messages` 两列存量守卫落在 `is_new_db == false` 分支内、`has_mode_id` 之后，只加列不回填
- [x] 新建 `idx_messages_conv_coord (conversation_id, turn_index, iteration)`，落点在两处守卫之后、`init` 收尾前（不能进建表批，也不能进存量分支）
- [x] 保留旧索引 `idx_messages_conv_ts`，导出 / 搜索仍按时间排序
- [x] `conversations/mod.rs` `MSG_SELECT` 由 17 列扩到 19 列（`turn_index, iteration` 追加在 `mode_id` 之后）
- [x] `msg_ops::insert_message` 占位符由 `?1..?17` 改 `?1..?19`，`params!` 追加 `row.turn_index` / `row.iteration`
- [x] `MessageRow` 加 `turn_index: Option<i64>` / `iteration: i64`；`read_msg_row` 追加两行 `row.get(...)?`
- [x] `msg_ops::get_messages` 排序键改为 `(turn_index IS NULL) ASC, turn_index ASC, iteration ASC, timestamp ASC, rowid ASC`（`turn_index` 为 `NULL` 的存量行整体垫底）
- [x] `branch::trace_active_branch` 保持按父链取行、顺序不改（规格 §2.7 明确不动）

### S1.2 契约字段

对应规格 §2.3 / 门禁 12、13。

- [x] `protocol/dto/chat.rs` `MessageDto` 加 `turn_index: Option<u32>`（输出 `Option` 不加 `skip_serializing_if`，键恒在、值 `null`）与 `iteration: u32`
- [x] `From<MessageRow> for MessageDto` 显式降级转换：`and_then(|v| u32::try_from(v).ok())` / `unwrap_or(0)`，越界不 panic
- [x] `npm run contract:gen` 生成 `src/types/protocol.contract.ts`：`turnIndex: number | null` 必填、`iteration: number`
- [x] 前端 `Message` 由 `Omit<MessageDto, ...>` 派生，未手改类型文件

### S1.3 构造点与自验

对应规格 §2.1「机械保障」/ §9.2。

- [x] 7 处 `MessageRow` 构造点补坐标：`cmd/chat/insert.rs`（2 处，暂 `None` / `0`）、`adapters/message_store.rs`（2 处，暂 `None` / `0`）、`cmd/compaction.rs`（1 处，`None` / `0`）、`conversations/ops.rs` 分支复制（2 处，**原样复制**源行坐标）
- [x] 测试替身补齐：`assistant-storage/tests/common/mod.rs`、`src-tauri/tests/protocol_contract.rs`、`assistant-agent/tests/{prompt_options_spec,loop_executor,kernel_runner,kernel_emit,attachment_budget}.rs`
- [x] `tests/protocol_dto_roundtrip.rs` 断言 `turnIndex` / `iteration` 的 camelCase 与取值
- [x] `cargo check --all-targets` 通过
- [x] `cargo test -p assistant-storage` 全绿
- [x] `npm run typecheck` 通过

**S1 遗留（按规格分给后续批次，不是缺陷）**：

- 写入侧坐标仍恒 `None` / `0`——取号函数与 7 条写路径属 S2；此时新会话新行是 `turn_index IS NULL`，门禁 18 尚未成立（S2 已完成，遗留关闭）。
- 分页游标仍按 `timestamp < ?` 取页——属 S6（P3），S1–S5 期间靠 `unresolved` 组不折叠兜住（S6 已完成，遗留关闭）。

---

## 阶段 S2 — 写路径（已完成 2026-11-13）

### S2.1 取号函数

对应规格 §2.2 / 门禁 19。

- [x] `msg_ops::allocate_turn_index(conn, conversation_id) -> Result<u32, String>`：先读 `next_turn_index` 再 `UPDATE ... + 1`；`UPDATE` 影响行数为 0 报「会话不存在」，不返回默认值
- [x] `msg_ops::allocate_iteration(conn, conversation_id, turn_index) -> Result<u32, String>`：`COALESCE(MAX(iteration), 0) + 1`，带 `turn_index` 等值条件
- [x] 两个函数都不开事务、不 `commit`（事务边界归调用方）；禁止 `SELECT MAX(turn_index) + 1`

### S2.2 七条写路径事务化

对应规格 §2.4 / 门禁 18、19、20。

- [x] 路径 1 `insert_normal_append`：`insert_chat_messages` 开 `unchecked_transaction`，`allocate_turn_index` 得 `t`，user 行 `(t, 0)`、assistant 行 `(t, 1)`
- [x] 路径 2 `insert_on_branch`（分支点 user）：同上，新开一轮
- [x] 路径 3 `insert_on_branch`（分支点 assistant）：读分支点 `turn_index`；`NULL` 则 `(None, 0)`；否则 `t` 取分支点、`iteration = allocate_iteration(t)`；**不递增会话计数器**
- [x] 路径 4 `create_assistant_message`：`CreateAssistantReq` 加 `turn_index` / `iteration`，写入行（该路径已有事务）
- [x] 路径 5 `append_tool_message`：`AppendToolMessageReq` 加同两字段（已有事务）
- [x] 路径 6 `cmd/compaction.rs` 摘要行：包事务，写 `(None, 0)`（OQ3 默认值）
- [x] 路径 7 `duplicate_conversation`：原样复制坐标（S1 已随构造点完成）；新会话 `next_turn_index` 沿用源值（L4）
- [x] `InsertedMessages` 扩展 `turn_index: Option<u32>` 与 `iteration: u32` 并在两分支合并返回

### S2.3 内核坐标透传

对应规格 §2.5 / 门禁 1。

- [x] `kernel/ids.rs` `TurnContext` 加 `turn_index: Option<u32>`，`TurnContext::new` 加参数（调用点仅 `cmd/agent/mod.rs`）
- [x] `ids.rs` 注释改写：「`iteration` 落库并进 `MessageDto`；`provider_attempt` 仍只在事件与日志」
- [x] `loop_executor/persist.rs` `ensure_row`：`CreateAssistantReq { turn_index: ctx.turn_index, iteration: state.iteration }`
- [x] `loop_executor/persist.rs` `append_tool_results`：`iteration` 取发起该调用的 `RoundState.iteration`，不给 `FinalizedToolCall` 加字段
- [x] `round_executor.rs` `TurnRunCtx` 透传 `turn_index`，只搬运不判定

### S2.4 自验

- [x] 门禁 18 chat 段：新建会话一轮后库中无 `turn_index IS NULL` 的新行
- [x] 门禁 19：注入写入失败时计数器不递增、计数器更新失败时行不落库（`insert::write_failure_rolls_back_counter_and_rows`）
- [x] 门禁 20：分支点为 assistant 时新行 `turn_index` 等于分支点 turn、`iteration` 在该 turn 内递增
- [x] `cargo test --test conversations`、`cargo test -p assistant-storage -p assistant-agent` 通过

---

## 阶段 S3 — 事件与前端数据层（已完成 2026-11-13）

### S3.1 乐观消息坐标（已提前完成）

对应规格 §3.1。

- [x] `src/chat/core/optimisticMessage.ts` 入参加可选 `turnIndex?: number | null` / `iteration?: number`，输出必填 `turnIndex: number | null` / `iteration: number`，缺省 `null` / `0`
- [x] 测试替身补齐坐标字段：`src/test-utils/messageFixtures.ts`、`src/orb/__tests__/orbBusy.test.ts`、`src/orb/__tests__/orbAutoCollapse.test.ts`
- [x] 8 处调用点按可用信息传值（`agentSegments.ts` `makePlaceholder` 取 `currentTurnIndex` + 段迭代号、`turnOrchestrator.ts` user / assistant 占位先 `null` 后由 `chat-stream-start` 回填、`chatCompactSlice.ts` 摘要行走缺省 `null` / `0`），不得推算相邻关系

### S3.2 事件载荷

对应规格 §2.6 / 门禁 12、17。

- [x] `ChatUserMessageConfirmedPayloadDto` 加 `turn_index: Option<u32>`
- [x] `ChatStreamStartPayloadDto` 加 `turn_index: Option<u32>` 与 `iteration: Option<u32>`；`iteration` 取 assistant 占位行**实际**迭代号（分支重生成时 `> 1`），不写死 `1`
- [x] `adapters/event_sink.rs` `TauriEventSink` 加 `turn_index` / `iteration` 字段与 `with_turn_coords`，两载荷共用同一份
- [x] `cmd/chat/insert.rs` `InsertedMessages` 加 `iteration: u32`，两条写路径返回真实值
- [x] `cmd/agent/mod.rs` 的 `TauriEventSink::new(...)` 后追加 `.with_turn_coords(inserted.turn_index, Some(inserted.iteration))`；chat 链路走同一构造点
- [x] `npm run contract:gen` 并提交生成物

### S3.3 传输层与 store

对应规格 §3.2 / 门禁 17。

- [x] `services/stream/transport.ts` `onStreamStart` 扩为 5 参、`onUserMessageConfirmed` 扩为 3 参（传输层只透传）
- [x] `services/stream/turnOrchestrator.ts`：`onStreamStart` 写 `currentTurnIndex`，两分量非空时 `setMessageCoords(messageId, turnIndex, iteration)` 回填 assistant 占位
- [x] `confirmUserMessage` 端口签名扩为 `(id, content, turnIndex)`，同步 `chatStorePorts.ts`、`turnStrategy.ts` 与测试替身
- [x] `onTextDelta` 在 `iteration >= 2` 时开新段，新段坐标取 `(currentTurnIndex, chunk.iteration)`
- [x] `stores/chatStreamStore.ts` 加 `currentTurnIndex: number | null` 与 `setCurrentTurnIndex`，`resetState` 归零；不进持久化、不进 wire

### S3.4 段表降级为缓存

对应规格 §3.3 / 门禁 2（前端侧）。

- [x] `stores/agent/agentSegments.ts` 加 `rebuildFromMessages(messages, turnIndex)`（实现为 `applyRebuild`）：扫 `role === "assistant" && turnIndex 相等 && iteration >= 1` 的行重建映射；`turnIndex` 为 `null` 时以已加载消息的最大 turn 为目标（`lastTurnIndex` 兜底）
- [x] 与既有映射冲突时以消息行为准；`adoptSegment` 保留为写入侧缓存更新
- [x] `markUnknownSegment` 先重建，再让仍在飞的迭代由首个增量开新段
- [x] `resolveSegment` 缓存未命中时先按坐标查已加载消息，仍无才 `openSegment`
- [x] 刷新后（重启 / 重开会话）段表可由消息重建

---

## 阶段 S4 — 容器层（已完成 2026-11-14）

### S4.1 身份行改坐标

对应规格 §4.1 / 门禁 3、15。

- [x] `MessageList.tsx` `showIdentity` 由「`index === 0` 或上一条不是 assistant」的相邻启发式改为坐标判定（`turnIndex` 与上一条不同 → 显示；`turnIndex === null` → 走现状兜底）
- [x] 不改 `role !== "tool"` 的过滤与滚动 / 未读逻辑（规格 §1.3）

### S4.2 chrome 迁到 turn 尾

对应规格 §4.2 / 门禁 15。

- [x] `MessageBubble.tsx` **仅 assistant 侧**跳过气泡内的 `actionsWrapper`（时间戳 + `BranchPicker` + `MessageActions`），由新 prop `suppressAssistantChrome` 控制（规格 §4.4）；user 行的 chrome 仍由自身渲染，**不随 `TurnTail` 上移**
- [x] 新建 `src/chat/message/TurnTail.tsx`：assistant 侧时间戳、`BranchPicker`、`MessageActions` 只在 turn 尾锚点挂载一次；复用 `MessageBubble.module.css` 的 `actionsWrapper` / `timestamp` 样式，保证与现状同位置、同外观
- [x] 尾锚点按 ADR D3 三级优先级在 `MessageList` 内计算（最后一条有正文且 settled 的 assistant 行 → 该 turn 最后一个 assistant 行 → 无 assistant 则不挂尾），`TurnTail` 只读锚点字段
- [x] `MessageBubble` memo 比较器追加 `suppressAssistantChrome` / `isTurnTail` 与坐标字段（`turnIndex` / `iteration` / `status`）
- [x] chat 气泡视觉零变化：chat 轮恒为「1 user + 1 assistant」，`TurnTail` 挂在该 assistant 行、组头不出现、user 行 chrome 不动，渲染结果与现状逐项一致（门禁 15）；这是回归项，不是重设计

---

## 阶段 S5 — 分组与密度（已完成 2026-11-14）

### S5.1 分组投影

对应规格 §5 / 门禁 3–7、10、16。

- [x] 新建 `src/chat/message/groupNodeRows.ts`：纯函数，输入节点数组输出「轮 + 组 + 平铺节点」视图；不读 `modeId` / `link` / `TurnKind`
- [x] 折叠判据只由坐标与 transcript 边界驱动，禁定时器；`isStreaming === false` 且非当前 turn 恒 `settled`；`currentTurnIndex` 未下发时由 `streamingMessageId`（流式占位行）兜住 `collecting`，避免运行中的 turn 提前收起、组头步数随增量跳动
- [x] 折叠单位为**连续可折叠步段（run）**而非 turn：`planRuns` 按「`success` 且有摘要」切段，每段独立计数（`NodeRunStats.steps`）、独立判定阈值、独立展开
- [x] run 在节点序列里严格连续：正文 / 推理行、失败 / 待处理 / 无摘要的步、`compact_summary` 边界行一律切断 run；组头每段只出现一次，`run.steps` 恒等于其下实际步行行数（禁止同一 run 被气泡打断后重复渲染组头、每处都显示整段总步数）
- [x] 同一 turn 内重复 `callId` 只计一次、只渲染一次（`planRuns` 与节点构建同一口径），组头步数不虚高、无重复 React key
- [x] 安全阀下沉到步级：`error` / `awaiting_user` / 无摘要的步**切断当前 run** 并留在气泡内完整显示，不影响其他成功步折叠；`compact_summary` 命中 `hasBoundaryNode` 时整 turn 不折叠；`needsAttention` 只作组级汇总与断言
- [x] 回复行恒在组外（含正文节点不折叠）；`unresolved` 组不折叠（跨页兜底），且 `phase` 恒 `settled`、`tailAnchor` 恒 `null`
- [x] 投影前后 `messages` 引用 / 内容不变，展开后节点顺序与 `id` 一致（折叠态只产出浅拷贝，`toolCalls` 仅保留 run 外的步，不改原数组）
- [x] 新增 `TurnGroup.tsx` 承接一组渲染：`[身份行] + [过程组（组头 + 步行），每 run 一组] + [回复行] + [TurnTail]`；展开态是组件本地 `useState`，**按 run id 各自持有**，一个 run 的展开不影响其他 run；`key` 取 `turnIndex`（`unresolved` 组取首节点 id）保证切会话重建
- [x] `MessageList.tsx` 改为按组渲染，`RoleContext` 由 `prevRoleById` 映射逐行传入，组间间距沿用 `MessageBubble.module.css` 的相邻规则

### S5.2 三档密度与组件

对应规格 §6 / 门禁 8、9、11。

- [x] 新建 `src/chat/message/GroupHeader.tsx`：组头计数 + 展开 / 收起，无正文读取
- [x] 新建 `src/chat/tool/ToolCallRenderer.tsx`：`single` 单行档 / `card` 卡片档；单行档取 `view.compact.head`，`compact == null`（旧数据）才走降级分支（工具名 + 状态 + 参数摘要 + 耗时），不现场统计字节（P1 已于 2026-11-14 关闭，正式分支已接通）
- [x] `MessageBubble.tsx` 内 `toolCalls.map(...)` 改为 `ToolCallRenderer`；卡片档直通 `ToolCallBubble`，单行档不调用 `parsePersistedOutput` / `resultStats` / markdown 解析（门禁 9）
- [x] 选路只看 `density` 与 `compact.anchor` 是否命中当前文档，不看身份（门禁 11）；`onOpenDocument` 随 `compact` 落地已加入接口
- [x] `constants.ts` 加 `PROCESS_COLLAPSE_THRESHOLD = 3`
- [x] 组头文案进 `zh-CN` / `en-US` 的 `chat.message.group.*`（`process` / `steps` / `failed`）

### S5.3 自验

- [x] `src/chat/message/__tests__/groupNodeRows.test.ts`：15 例覆盖分组、相位、折叠判据、节点投影、尾锚点（门禁 3、4、5、6、7、8、10、16）
- [x] `src/chat/message/__tests__/nodeLayoutGates.test.ts`：源码级断言门禁 6、8、9、11、16 与 `TURNS_PER_PAGE` 单一来源（仓库无 React 渲染测试环境，组件级断言改为静态扫描）
- [x] 8 迭代 × 2 调用场景可折叠，回复行恒在组外

---

## 阶段 S6 — 分页按 turn（已完成 2026-11-13）

对应规格 §7 / 门禁 10。

- [x] `msg_ops::get_messages_paginated` 游标由 `timestamp < ?` 改为 turn 边界（两步查询：先取 turn 集合，再取整轮消息；`limit` 降级为消息护栏，截断只在 turn 边界发生，单个 turn 超护栏时整轮返回）
- [x] command 层参数 `before_timestamp` 改 `before_turn_index: Option<u32>`；`chatService.ts` 与两处调用点（`chatMessageSlice` / `chatConversationSlice`）同步，游标取已加载消息的最小 turn
- [x] `TURNS_PER_PAGE = 12` 定为**后端常量**（`assistant-storage`），前端不重复定义；`MESSAGES_PAGE_SIZE = 50` 保留并注释为「消息护栏」
- [x] 页边界处不合并非相邻节点；`unresolved` 组不折叠（`turn_index IS NULL` 在 turn 游标耗尽后单独成页、恒保留）
- [x] 后端分页用例 + 前端投影用例通过

---

## 验收口径（规格 §9.3，验收时逐条对照）

1. 存量会话（全 `turn_index IS NULL`）永久平铺、永不折叠，记为「新形态只对上线后新增的轮生效」，**不算缺陷**。
2. 含 `compact_summary` 的段落平铺，**不算缺陷**。
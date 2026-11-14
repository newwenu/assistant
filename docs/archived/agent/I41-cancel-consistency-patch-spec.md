# I41 — 用户取消的跨链路一致性补丁规格

| 项 | 值 |
|---|---|
| 状态 | 已归档（2026-11-09）；P1 / P1b / P2 / P4 已落地，P3 未落地（实现落点移交 [I38e](../../store/I38e-mcp-tools-spec.md) §4.3，随 P7 落地）。实施复核与已知缺口见 §9 |
| 归属 | P3 / P6 收口补丁；P7（[I38e](../../store/I38e-mcp-tools-spec.md)）前置落点 |
| 关联 | [I36 spec](./I36-agent-runtime-spec.md) Step 2 / Step 7、[I39](./I39-compaction-chat-merge-spec.md) §3.4 / §4.1 / §8、[I38e](../../store/I38e-mcp-tools-spec.md) §4.3、[I38a](./I38a-tool-control-layer-spec.md) A5、[I38c](./I38c-os-tools-spec.md) §9 / §13、[I38d](./I38d-agent-frontend-modes-spec.md) §3.5 / §4.8 |
| 影响面 | `src-tauri/crates/assistant-agent/src/kernel/{tasks,runner}.rs` 与 `tests/*`；`src-tauri/crates/assistant-storage/src/conversations/{mod,msg_ops,ops}.rs`；`src-tauri/src/protocol/dto/chat.rs`；`src-tauri/src/cmd/{compaction,conversation}.rs`、`src-tauri/src/cmd/chat/insert.rs`；`src-tauri/src/adapters/{tool_exec,message_store}.rs`；`src-tauri/tests/{protocol_bindings,protocol_dto_roundtrip}.rs`；`src-tauri/crates/assistant-tools/src/{context,exec}.rs` 与 `mcp/`；`src/services/{chatService.ts,stream/turnOrchestrator.ts,stream/turnStrategy.ts}`；`src/stores/{chatCompactSlice.ts,chatStorePorts.ts}`；`src/chat/{core/chatTypes.ts,message/MessageBubble.tsx,message/MessageBubble.module.css}`；`src/i18n/locales/*.json`；`src/types/protocol.contract.ts`（生成物） |

> 本文已归档并冻结，只作历史参照：不再修改、不再作为权威源。P1 / P1b / P2 / P4 的现行行为以代码为准；P3 的现行契约在 [I38e](../../store/I38e-mcp-tools-spec.md) §4.3，本文 §3 只作设计依据。

## 0 定位

本文**不改**取消主机制：协作式置位（`TurnTasks::cancel` 只 `token.cancel()`，不 `abort`）、代际守卫（`is_current` / `finish` / `TurnEmitter::dispatch` 三处写侧校验）、子令牌层级（I36 Step 2.1）全部保持不变。

另有一处**表示层**调整：用户取消不再以 `ChatError::Cancelled` 临时挂在气泡上，改为落为消息持久状态 `status: "interrupted"`（§1.4）。这不改取消的触发与传播，只改「取消结果怎么表示、存哪」。本补丁只闭合「用户取消」在既有规格中的未闭合点。四条缺口均已确认**不阻塞 P7 开工**，但必须在 P7 首个 MCP 工具落地前收口，否则实现会出现歧义。

| 编号 | 缺口 | 类型 | 权威依据 |
|---|---|---|---|
| P1 | 压缩取消时前端报失败并删临时气泡 | 实现偏离规格 | I39 §8 验收 4 |
| P1b | 取消用临时前端错误表示，刷新即丢且与失败共用渲染通道 | 表示层缺口 | I39 §4.2 / §8 验收 4 |
| P2 | 取消后到 `finish` 之间 `begin(Reject)` 仍返回 `Busy` | 规格留白 | I36 Step 7 验收 7 |
| P3 | `ToolUseContext` 无取消令牌，MCP 无请求级取消落点 | 实现落点缺失 | I38e §4.3 / I38a A5 |
| P4 | `TurnStatus::Cancelled` 仍触发终止式压缩 | 规格留白 | I39 §3.4 |

---

## 1 P1 — 压缩取消的前端收口

### 1.1 补丁前现状（偏离）

| 环节 | 当前行为 |
|---|---|
| 后端 | 取消分支落 `TurnStatus::Cancelled` → `save_compaction_partial` → `Err("Compaction cancelled")`（`src-tauri/src/cmd/compaction.rs`，终态分支），符合 I39 |
| 命令边界 | `manual_compact_conversation` 校验准入后 `spawn` 并立即返回 `Ok(())`；运行期取消/失败只落在 spawn 任务的日志里，**不会**进入前端 `catch` |
| 编排器 | `cancel()` 只要 `streamingMessageId` 有值就把 `ChatError::Cancelled` 写到该气泡（`src/services/stream/turnOrchestrator.ts` 的 `cancel`）；压缩流的 `streamingMessageId` 是**临时摘要气泡** id（`chatCompactSlice` 先 `beginStream(tempId)`） |
| 压缩 slice | `manualCompact` 的 `catch` 把所有异常当作准入失败：`notifyError(...)` + 过滤掉临时摘要气泡（`src/stores/chatCompactSlice.ts` 的 `manualCompact`） |

结果：取消时前端会在临时摘要气泡上写出 `Cancelled` 错误（违反 I39 §8 验收 4「无失败横幅」），而 `catch` 缺少取消判定——一旦命令边界改为同步返回错误（或运行期错误回传），取消就会被当成失败提示。

### 1.2 目标行为

`catch` 必须按「准入失败 / 运行期取消 / 运行期失败」三分，不再合并：

| 场景 | 临时气泡 | 提示 | 真实摘要行 |
|---|---|---|---|
| 准入失败（`conversation_full` / `conversation_busy`，turn 未开始） | 移除 | `notifyError`（现状保留，I39 §4.1） | 不存在 |
| 运行期取消 | 移除 | **不通知** | 后端已落库 `interrupted`，由 `chat-messages-changed` 刷新呈现 |
| 运行期失败（超时 / provider 报错 / 提交失败） | 移除 | 现状保留 `notifyError` | 后端 `terminated(Failed, error)` 已带错误；失败呈现通道的统一属 I39 §4.2，不在本补丁范围 |

> 可达性（2026-11-09 补齐 G1 后）：`manual_compact_conversation` 已改为 `async` 直接 `await execute_compaction`，运行期取消 / 失败经 invoke 的 Promise 进入前端 `catch`，三分全部可达；此前 `spawn` 包装下只有「准入失败」一行生效，见 §9 G1。

两处配套约束：

1. **取消不写临时气泡、也不写 `Cancelled` 错误**：`turnOrchestrator.cancel()` 不再写 `ChatError::Cancelled`，改为把**当前 turn 的气泡**落为 `status: "interrupted"`（§1.4）。压缩不是 turn（`TurnSpec` 不参与，I39 §1.2），其临时摘要气泡既不落状态、也不写错误，直接移除；会话级取消语义由后端 `terminated(Cancelled)` + 摘要行 `interrupted` 承担。
2. **取消判定集中一处**：在 `src/services/chatService.ts` 导出 `isCompactCancelled(error): boolean`，`chatCompactSlice` 只调用它，禁止在 slice 内散落 `"Compaction cancelled"` 字面量比较（后端文案变更不得影响前端分支）。

### 1.3 验收

- **A1**：超阈值会话手动压缩 → 点停止 → 无 toast/横幅；临时气泡消失；真实摘要行以 `interrupted` 出现且 `is_full` 仍为 `false`；输入框立即可用。
- **A2**：`conversation_busy` 场景（turn 运行中触发压缩）仍弹准入失败提示并回滚临时气泡（不回归）。

### 1.4 取消的持久表示：消息状态化（P1b）

补丁前取消用 `ChatErrorCode::Cancelled` 临时挂在气泡上，是**前端本地状态**：刷新 / 重启即丢，且与真实失败共用同一条错误渲染通道（I39 §4.2 的失败呈现因此被迫区分取消）。本补丁改为**消息持久状态**承载取消，三链路（chat / agent / compaction）统一。

| 层 | 契约 |
|---|---|
| 存储 | `messages.status` 取值 `streaming` / `completed` / `interrupted` / `failed`；取消终态为 `interrupted`（`assistant-agent` 既有映射 `TurnStatus::Cancelled \| Interrupted → MessageStatus::Interrupted` 不变） |
| 协议 | `MessageDto` 增 `status: MessageStatusDto`（封闭枚举，wire snake_case：`"streaming" \| "completed" \| "interrupted" \| "failed"`）；`From<MessageRow>` 用 `MessageStatusDto::from_db` 映射，未知取值兜底 `completed`（兼容存量库） |
| 前端类型 | `Message["status"]` 为可选（乐观占位消息尚未落库）；`MessageDto` 本身必填，`toChatMessage` 原样透传 |
| 前端收口 | `turnOrchestrator.cancel()` 调 `markMessageInterrupted`，**不**写 `Cancelled` 错误；气泡在 `!isStreaming && status === "interrupted"` 时渲染「已中断」次级文案 |
| 复制 / 分叉 | 承接时 `streaming` 降为 `completed`（运行期瞬时态不可复制），其余终态原样承接（`conversations::ops` 的 `carried_status`） |

约束：

1. **取消不是失败**：`Cancelled` 错误码仍保留在 provider 错误集合（上游主动取消时使用），但**用户取消**不再产生任何 `ChatError`；`error` 只承载真实失败。
2. **单一真相源**：前端 `markMessageInterrupted` 是乐观收口，后端落库 + `chat-messages-changed` 刷新为幂等二次收口；两处写同一状态，不新增事件。
3. **wire 不承载本地 UI 状态**：`status` 是可持久终态，不是 `error` 那类本地扩展；`Message` 用交叉类型保留本地字段的既有做法不变。
4. **压缩临时气泡例外**：临时摘要气泡不是落库行，不落 `status`，移除即可；真实摘要行由后端写入 `interrupted`。

验收：

- **A9**：chat / agent 链路取消 → 气泡显示「已中断」且**无** `Cancelled` 错误块；重启应用后仍显示「已中断」。
- **A10**：契约生成物包含 `MessageStatusDto` 封闭枚举；存量库中 `status` 异常值读出为 `completed`（不 panic、不误标中断）。
- **A11**：复制 / 分叉含 `streaming` 消息的会话 → 新会话中该消息为 `completed`，`interrupted` / `failed` 原样承接。

---

## 2 P2 — 取消后的互斥窗口

### 2.1 补丁前现状（规格留白）

| 环节 | 当前行为 |
|---|---|
| 前端 `cancel()` | 先 `phase = "idle"` 并删除本地代际记录，不等待后端 `terminated`（乐观收口） |
| `TurnTasks::begin(Reject)` | 只判「当前代际是否存在」，存在即返回 `Busy`（`src-tauri/crates/assistant-agent/src/kernel/tasks.rs`） |
| `patch_conversation` | 以 `tasks.current(conv).is_some()` 判忙 → 返回 `conversation_busy`（`src-tauri/src/cmd/conversation.rs`） |
| `manual_compact_conversation` | 同样以 `current()` 判忙 → `Err("Cannot compact while stream is running")`（`src-tauri/src/cmd/compaction.rs`） |

取消到 `tasks.finish` 之间（取消点落库、收尾、`terminated`、压缩触发判定）存在毫秒级窗口：此窗口内用户已看到 idle，但后端仍视作忙。表现为「取消后立刻发送 → `conversation_busy`」「取消后立刻切模式 → 被拒」。

### 2.2 目标契约

**判定口径改为「忙 = 存在当前代际 且 其令牌未取消」**。取消意图一旦表达，互斥即视为解除，无需等待收尾完成。

| 落点 | 变更 |
|---|---|
| `TurnTasks` | 新增 `is_busy(conversation_id) -> bool`：`current()` 存在且 `cancel_token` 未 `is_cancelled()` |
| `TurnTasks::begin(Reject)` | 旧代际令牌**已取消**时按 `Replace` 处理（幂等 `cancel` → 代际递增 → 装新），否则维持返回 `Busy` |
| `patch_conversation` / `manual_compact_conversation` | 忙判定改用 `is_busy` |
| `is_current` / `finish` / `cancel_token` / `TurnEmitter::dispatch` | **不变**，仍严格按代际 |

顶替安全性论证（不新增守卫）：

- 旧代际的三处写侧（事件发射、终态落库、状态迁移）已由代际守卫拦截，迟到收尾不会覆盖新 turn；chat 链路的 `Replace` 顶替即同一先例（I39 §8 验收 5）。
- 取消轮的部分内容在取消点已落库（I36 Step 2 第 3 条），顶替不丢内容。
- 旧代际的 `finish()` 因 `turn_id` 不匹配而不生效，新代际的令牌独立，互不影响。

**已知边界（不因本补丁改变）**：旧轮工具体的阻塞段（如已进入的系统调用、子进程）仍按 I36 Step 2.1 第 2 条（取消只置位、禁止 `abort`）执行到自然结束，可能与新轮并行。这是协作式取消的既有取舍，见 §5。

### 2.3 验收

- **A3**：agent 链路取消后立即发送 → 不返回 `conversation_busy`，新 turn 正常跑完并落库。
- **A4**：取消后立即 `patch_conversation { modeId }` 与 `manual_compact_conversation` → 均不被忙判定拒绝。
- **A5**：旧轮迟到完成（复用 I36 Step 7 验收 9 的构造）→ 不写库、不改状态、不发终止事件。

---

## 3 P3 — MCP 请求级取消

### 3.1 补丁前现状（实现落点缺失）

| 环节 | 当前行为 |
|---|---|
| I38e §4.3 | 已规定「中止本次请求、返回 `Cancelled`、不杀 server 进程」 |
| I38a A5 | 曾把 `ToolUseContext` 的取消字段顺延到 P4 |
| P4 实际落地 | 只把 `child_token` 放进 `ToolExecCtx`（`src-tauri/crates/assistant-agent/src/agent/round_executor.rs`），未进入工具层 |
| `ToolUseContext` | 仅 `workspace_root` / `resolve_base` / `session_id` / `turn_id` / `iteration` / `tool_call_id`，无取消令牌（`src-tauri/crates/assistant-tools/src/context.rs`） |
| 适配器 | 构造 `ToolUseContext` 时只填会话/turn/迭代（`src-tauri/src/adapters/tool_exec.rs`） |
| `exec::invoke` | 仅套 `tokio::time::timeout`；`manages_own_execution_timeout = true` 时直接透传，不接收取消信号（`src-tauri/crates/assistant-tools/src/exec.rs`） |

即：规格要求的能力没有实现载体——工具拿不到令牌，就无法在 MCP 请求级别中止。

### 3.2 目标契约

1. **上下文携带令牌**：`ToolUseContext` 增 `cancel: Option<CancellationToken>`，标 `#[serde(skip)]`。ToolBox 反序列化路径恒 `None`；wire 形态与 DTO 不变。
2. **适配器填充**：`src-tauri/src/adapters/tool_exec.rs` 用 `ctx.child_token.clone()` 填入，与 I36 Step 2.1 的工具子令牌同源（`TurnTasks::child_token`）。
3. **MCP 客户端消费**：`call_tool` 内部 `tokio::select! { biased; _ = token.cancelled() => ..., r = client.call_tool(...) => r }`；取消时
   - 尽力发送 `notifications/cancelled { requestId, reason }`（失败仅 `log::debug!`，不影响返回）；
   - 返回 `ToolErrorKind::Cancelled`（回填模型时按 I36 Step 2.1 第 4 条写「已取消」）；
   - 不杀 server 进程；在途响应按 `requestId` 丢弃，同一 server 的后续调用复用进程。
4. **内置工具零变化**：内置工具不读该字段，取消仍由 `exec::invoke` 外层的 `cancellable` 包装响应（I38c §9「绕过统一超时不等于不可中断」）。

**为什么不在 `exec::invoke` 统一 select**：`invoke` 是整批工具的执行边界，在此处响应取消会把「turn 取消」变成「逐个工具中断」，改变 I38b §4.5 第 6 条的整批 `Cancelled` 语义。本补丁只把令牌交给需要「请求级中止」的 External 工具，范围最小。

### 3.3 验收

- **A6a**：MCP 工具执行中取消 turn → 该次调用返回 `Cancelled`，MCP server 进程存活（I38e A6 原文）。
- **A6b**：mock server 收到 `notifications/cancelled`，且 `requestId` 与在途请求一致。
- **A6c**：取消后对同一 server 的下一次调用成功（进程复用，未重启）。

---

## 4 P4 — 取消的 turn 不触发终止式压缩

### 4.1 补丁前现状（规格留白）

`run` 收尾的自动压缩触发条件只判 `still_current` + `CompactDecision::Terminate`（`src-tauri/crates/assistant-agent/src/kernel/runner.rs`）；`decide_compaction_from_outcome` 只看 `prompt_tokens`，不看 `outcome.status`。因此**取消后仍会触发终止式压缩**。

### 4.2 目标契约

| `TurnStatus` | 终止式压缩 |
|---|---|
| `Completed` / `Failed` | 维持现状（按阈值触发） |
| `Cancelled` / `Interrupted` | **不触发** |

理由：取消是用户的明确意图，随即自动发起一次模型摘要调用与之相反；且取消高发于超长上下文，阈值判定必然命中，缺口会被稳定触发。

落点：`run` 的触发分支增加状态判定（`matches!(outcome.status, Completed | Failed)`），不改 `decide_compaction_from_outcome` 的阈值逻辑。

### 4.3 验收

- **A7**：构造超阈值会话 → 发送 → 取消 → 5s 内不出现 `CompactionStarted` / 压缩流 / 摘要行；同会话下一次正常完成仍按阈值触发。

---

## 5 明确不做

| 项 | 理由 |
|---|---|
| 给内置工具加执行侧中断（工具内部主动监听令牌、强制 kill） | 违反 I36 Step 2.1 第 2 条「取消只置位、禁止 `abort`」，改动面与风险不成比例 |
| 新增 `Cancelled` phase 或改动前端五态 | I38d §3.5 的状态机不因取消补丁扩容 |
| 改 `cancel` 命令签名 / 返回类型 / DTO | 沿用「按链路分派 + 同键令牌」，前端 `cancel()` 乐观收口不变 |
| 新增 `cancel_compaction` 独立命令 | 压缩与 turn 共用 `TurnTasks`，令牌按会话键即可命中 |
| 取消后自动重试或自动回滚用户消息 | 取消的语义是「停下」，不是「撤销」 |

---

## 6 实施顺序

| 顺序 | 补丁 | 依赖 | 状态 | 备注 |
|---|---|---|---|---|
| 1 | P1 | 无 | 已落地 | 纯前端 + 一处判定，成本最低，先消除与 I39 的冲突 |
| 2 | P1b | 无 | 已落地 | 存储 → 协议 → 前端三处贯通，需 `npm run contract:gen` 与复制/分叉承接（A9–A11） |
| 3 | P4 | 无 | 已落地 | 内核一处条件，与 P1 独立 |
| 4 | P2 | 无 | 已落地 | 内核 + 两个命令守卫，需补并发测试（A3–A5） |
| 5 | P3 | P7 开工 | 未落地 | 随首个 MCP 工具同批落地，不阻塞 P1/P2/P4 |

P3 的实现落点已在 [I38e](../../store/I38e-mcp-tools-spec.md) §4.3 登记；P7 落地时只需按 A6 验收，不再改动规格。

---

## 7 验收汇总

| 编号 | 断言 | 归属 | 状态（归档复核 2026-11-09） |
|---|---|---|---|
| A1 | 压缩取消无失败提示，摘要行 `interrupted`，`is_full` 不变 | P1 | 已实现；后端 `execute_compaction` 运行期结果同步回传；前端 `chatStreamCharacterization::manualCompact — 运行期取消 / 失败分支（I41 §1.2）` 断言取消不提示失败、移除临时气泡 |
| A2 | 压缩准入失败仍提示并回滚临时气泡 | P1 | 已实现；同组用例的失败分支断言移除临时气泡并提示一次失败 |
| A3 | 取消后立即发送不被 `conversation_busy` 拒绝 | P2 | 已实现；`kernel_tasks::reject_after_cancel_replaces_stale_generation` |
| A4 | 取消后立即切模式 / 手动压缩不被忙判定拒绝 | P2 | 已实现；`kernel_tasks::is_busy_ignores_cancelled_generation` |
| A5 | 旧轮迟到完成不覆盖新轮 | P2 | 已实现；`kernel_tasks::finish_with_stale_turn_id_keeps_new_generation`、`kernel_runner::stale_turn_writes_nothing_and_emits_nothing` |
| A6 | MCP 取消返回 `Cancelled`、进程存活、收到 `notifications/cancelled`、后续调用成功 | P3 | 未落地（随 P7） |
| A7 | 取消轮不触发终止式压缩 | P4 | 已实现；`kernel_runner::completed_turn_triggers_terminal_compaction`、`kernel_runner::cancelled_turn_never_triggers_compaction` |
| A8 | 全链路回归：`cargo test --test assembly_contract`（cwd = `src-tauri`）+ 前端 `npm run typecheck` 通过 | 全部 | 手工执行，见下 |
| A9 | chat / agent 取消显示「已中断」、无 `Cancelled` 错误块，重启后仍为「已中断」 | P1b | 后端 `kernel_runner::cancelled_turn_marks_message_interrupted`；前端 `turnOrchestrator::cancel — 压缩占位气泡（I41 §1）` 断言 `markMessageInterrupted` 的调用与跳过 |
| A10 | `MessageStatusDto` 进入契约生成物；存量库异常值兜底 `completed` | P1b | 已实现；`protocol_dto_roundtrip::message_status_dto_maps_db_values_and_falls_back_to_completed` 覆盖四个枚举值与未知值兜底 |
| A11 | 复制 / 分叉承接状态：`streaming` 降为 `completed`，其余终态原样 | P1b | 已实现（`conversations::ops::carried_status`）；`conversations::duplicate_and_fork_do_not_carry_streaming_status` 覆盖降级路径 |

回归结果（2026-11-09）：`cargo test --all-targets --features test-doubles`（cwd = `src-tauri`）全绿；`npm run test` 38 文件 / 470 项通过；`npm run typecheck`、`npm run lint` 通过；`npm run contract:gen` 与生成物一致（`protocol.contract.ts` 仅含 `MessageStatusDto` 变更）。

> 复核实测（2026-11-09，归档时重跑）：`kernel_tasks` 12 项、`kernel_values` 11 项、`kernel_runner` 19 项（**必须带 `--features test-doubles`**，否则显示 `running 0 tests`，会掩盖 P2 / P4 相关用例）、`assistant-storage` conversations 36 项、`protocol_bindings` 1 项、`protocol_dto_roundtrip` 26 项全部通过；`npm run typecheck` 通过。

---

## 8 决策记录

| 编号 | 决策 | 取舍 |
|---|---|---|
| D1 | 用户取消不产生任何 `ChatError`，改落消息状态 `interrupted`（§1.4） | 取消是持久终态而非临时错误；刷新 / 重启不丢，且与真实失败解耦 |
| D2 | 「忙」= 当前代际存在且令牌未取消 | 取消即解除互斥；代际守卫已覆盖顶替安全，无需新机制 |
| D3 | MCP 取消在客户端 `call_tool` 内 select，不下沉到 `exec::invoke` | 保留整批 `Cancelled` 语义，改动面最小 |
| D4 | `Cancelled` / `Interrupted` 不触发终止式压缩 | 尊重用户意图，避免取消后自动烧一次摘要调用 |
| D5 | 取消状态走 `MessageDto.status` 封闭枚举，未知库值兜底 `completed` | 封闭集合用 `enum`；兜底保证存量数据与手改库不炸渲染 |

---

## 9 实施复核与已知缺口（归档时补充）

### 9.1 落地矩阵

| 补丁 | 落点 | 判定 |
|---|---|---|
| P1 | `chatService::isCompactCancelled`；`chatCompactSlice` 三分 catch；`turnOrchestrator.cancel` 跳过临时摘要气泡；`cmd/compaction.rs` 运行期结果同步回传 | 已落地 |
| P1b | `messages.status` + `MessageStatusDto` + `from_db` 兜底；`carried_status`；`markMessageInterrupted`；「已中断」渲染与 i18n | 已落地 |
| P2 | `TurnTasks::is_busy`；`OnBusy::Reject` 对已取消代际按 `Replace`；`conversation.rs` / `compaction.rs` 改用 `is_busy` | 已落地 |
| P4 | `kernel/runner.rs` 收尾按 `Completed \| Failed` 判定触发 | 已落地 |
| P3 | `ToolUseContext.cancel`、MCP 客户端 `call_tool` select | 未落地，落点移交 [I38e](../../store/I38e-mcp-tools-spec.md) §4.3 |

### 9.2 已知缺口

| 编号 | 缺口 | 处置（2026-11-09 已补齐） |
|---|---|---|
| G1 | 压缩 `catch` 的运行期取消 / 失败分支不可达：`manual_compact_conversation` 仍是 `spawn` 后立即 `Ok(())`，运行期错误只落日志，不进入前端 `catch` | 采用同步回传：命令改为 `async` 直接 `await execute_compaction`，DB 读取在 await 前释放锁；§1.2 三分全部可达，前端 `manualCompact` 取消 / 失败分支有自动化断言 |
| G2 | A7 / A9（前端）/ A10 / A11 有实现、无自动化断言（见 §7 状态列） | 已补：`kernel_runner::completed_turn_triggers_terminal_compaction` / `cancelled_turn_never_triggers_compaction`（compaction sink 断言）、`turnOrchestrator::cancel — 压缩占位气泡`（`markMessageInterrupted` 调用断言）、`protocol_dto_roundtrip::message_status_dto_maps_db_values_and_falls_back_to_completed`（`from_db` 兜底）、`conversations::duplicate_and_fork_do_not_carry_streaming_status`（duplicate / fork 带 `streaming` 降级） |
| G3 | 原回归口径缺 `--features test-doubles`，会以 `running 0 tests` 掩盖内核用例 | 已在 §7 补正；后续回归统一带该 feature |

### 9.3 后续

- G1 / G2 已补齐，不再遗留；G3 属记录口径，已修正。
- 自动压缩走宿主侧 `adapters/compaction_sink.rs`，无前端调用方，其 `spawn` 包装只影响日志、不影响分支可达性，故不改。
- P3 的现行契约由 [I38e](../../store/I38e-mcp-tools-spec.md) §4.3 承载，本文 §3 冻结为设计依据，P7 落地时不再回改本文。
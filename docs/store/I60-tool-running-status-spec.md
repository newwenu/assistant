# I60 — 工具调用三阶段状态呈现规格

| 项 | 值 |
|---|---|
| 阶段 | **能力补全 + 缺陷修复**（参数生成期空白属漏接线；运行期时间感与全局指示属能力缺口）；切片 T0–T5（§11） |
| 状态 | ✅ 已完成（2026-11-19；T0–T5 全部交付，G1–G16 逐条有落点；`typecheck` / `lint` / `vitest`（71 文件 / 846 用例）/ `cargo test -p assistant-agent`（含 `--features test-doubles`）/ `cargo clippy` / `cargo fmt --check` 全绿，`npm run contract:gen` 对两个生成文件零 diff；验收记录见文末） |
| 日期 | 2026-11-19 |
| 成因 | ① **参数生成期空白**：`ArgsDelta` 相位在 wire 上有定义、前端有分支，但后端**零发射点**，模型流式生成参数的整段时间前端收不到任何工具事件；② **运行期无时间感**：工具渲染自 I46 compact §3.7 起只定义**终态**口径（`compact.head` + 终态耗时），`Started` 只置 `status = "running"`，从未补运行期耗时与全局指示 |
| 关联 | 参考项目三阶段设计：`references/1-ai-agent/deepseek-harness/.agents/notes/implemented/architecture/2026-09-22-tool-call-three-phases.zh.md`（`preparing` → `start` → `result`）；I38b agent 后端执行循环（`ArgsDelta` 相位原始定义）；I38d agent-frontend-modes-spec（已归档）（`session-state-changed` 是 phase 唯一来源，本文**不**新增 phase 消费）；I46 compact 展示投影（已归档）（工具渲染选路与折叠口径，本文不改函数体）；I55 agent-turn-observability（观测面只读副本，本文不动）；I59 chat-error-null-contract（同期缺陷修复，另一域，无交叉） |
| 影响范围 | **后端**：`src-tauri/crates/assistant-agent/src/agent/tool_call.rs`（`ToolCallDeltaOutcome` 增 `opened`、快照归一为「追加后缀」、4 KB 发射上限）、`src-tauri/crates/assistant-agent/src/agent/loop_executor/call.rs`（唯一发射点）、`src-tauri/crates/assistant-agent/tests/tool_call.rs`（追加断言）。**前端**：`src/chat/core/chatTypes.ts`（`ToolCallStatus` 增 `preparing`）、`src/chat/core/toolCallView.ts`（`argsDelta` 分支 + `started` 分支不清空 `argsText` + 待处理判定）、`src/chat/tool/toolCallDisplay.ts`（新增两个纯函数）、新增 `src/chat/tool/useNowTick.ts`、新增 `src/chat/tool/activeTool.ts`、新增 `src/chat/tool/ToolActiveBar.tsx`（+ `.module.css`）、`src/chat/tool/ToolCallBubble.tsx`、`src/chat/tool/ToolCallRenderer.tsx`、`src/chat/tool/ToolCallDocStub.tsx`、`src/chat/message/MessageList.tsx`、两个 i18n 语言包、新增 `src/chat/tool/__tests__/activeTool.test.ts` 与 `toolCallView.test.ts` / `toolCallDisplay.test.ts` 追加用例。**零改动**：`src-tauri/src/protocol/**`（DTO 与事件名全部复用）、`src/types/protocol.contract.ts`、`src/services/stream/*`、`src/stores/**`、`src/chat/message/groupNodeRows.ts`（函数体）、`src/chat/tool/ApprovalCard.tsx` |

> 一句话：工具调用在界面上只有**两段可见期**——`Started` 之后的执行期（有转圈、无时间）与 `Finished` 之后的终态；中间夹着两段**完全空白**的时间：① 模型流式生成参数的那几秒到几十秒（wire 上一条工具事件都没有，只有一个正文光标），② `Started` 到审批卡弹出之间的毫秒级空档（短到不可感知）。而执行期本身又没有时间感，10 秒与 10 分钟在界面上完全一致。本文按参考项目的三阶段设计补齐：`preparing`（参数生成，**复用既有 `argsDelta` 相位**，只补后端发射点，零 wire 变更）→ `running`（执行，补运行期耗时）→ 终态（逐字不变），并把「当前活动」提升为全局指示器。

**编号约定**：`N`（现状事实）、`G`（门禁）、`T`（切片）、`L`（已知限制）、`R`（拒绝方案）、`OQ`（未决问题）在本文内独立编号；跨文档引用带文档前缀（`I46 §…`）。裸写 `§` 一律指本文。

---

## §0 范围与切分轴

### 0.1 交付范围

| 项 | 内容 |
|---|---|
| **纳入** | ① 参数生成期：后端补 `ArgsDelta` 发射点（唯一 wire 复用，不新增相位/字段），前端新增 `preparing` 状态并渲染工具行；② 运行期耗时：`running` 态用「当前时刻 − `startedAt`」实时显示，1 秒粒度；③ 共享 ticker：全应用最多一个 `setInterval`，按订阅者数启停；④ 两处渲染接入（卡片 / 单行档；文档批注引用只补 `preparing` 的状态分支，不加耗时列，N23）；⑤ 全局活动指示器：消息流末尾一行「正在准备 / 正在执行 `name` · `Ns`」，派生自消息上的 `preparing` / `running` 工具视图；⑥ 五个 i18n 键 |
| **排除** | wire 相位与字段新增（§12 R9）、`phase` 通道新增（§12 R1）、`groupNodeRows` 接入 `now`（§12 R2）、`ApprovalCard` / `QuestionCard` 倒计时、工具执行进度百分比、系统通知 / toast、后台会话的会话列表角标（§13 OQ4）、重试丢弃行的取消事件（§13 OQ6） |

### 0.2 切分轴

主判据：**「参数生成期空白」是数据源缺失（需一条既有通道的发射点），「执行期无时间感」与「全局不可见」是消费端缺失（纯前端可补）**。三者根因不同，必须分开切。

| 侧 | 判据 | 归属 |
|---|---|---|
| A | 参数生成期前端**无任何事件可依**：`argsDelta` 相位已定义、前端分支已实现，后端零发射（N15 / N16） | T0（后端）+ T1（前端状态机） |
| B | 单卡已有 `running` 状态与 `startedAt`，只缺「以当前时刻为终点」的格式化口径 | T2 / T3 |
| C | 「当前哪个工具在活动」已存在于消息上，只缺一个把它提升为全局提示的消费者 | T4 |
| D | 需要**新增** wire 能力（进度事件 / 心跳 / 替换式快照）才能表达的诉求 | 不在本文（§13 OQ1） |

### 0.3 前置事实校验

修复前必须确认以下六条同时成立，否则本文的归因不成立：

1. `Started` 事件确实写入 `startedAt`，`finishedAt` 为 `null`（§1 N2）；
2. `running` 视图确实不被折叠吞掉，卡片与单行档都会渲染（§1 N3）；
3. 运行期确实没有组头（`foldAllowed` 要求 `groupPhase === "settled"`），因此「组头耗时为空」不是缺陷（§1 N6）；
4. `phase` 在生产代码中确实零消费（§1 N7）；
5. `ArgsDelta` 的后端发射点确实是零，且前端分支确实是死分支（§1 N15 / N16）；
6. `Started` 确实早于 `AwaitingApproval`，故审批前不存在可感知的「执行中」窗口（§1 N22）。

六条均由 §1 的代码事实兜住。

---

## §1 现状（代码事实）

以下为 2026-11-19 的实际实现，路径相对仓库根。

| 编号 | 事实 | 位置 |
|---|---|---|
| N1 | `ToolCallStatus` 有 6 态：`running` / `success` / `error` / `awaiting_user` / `cancelled` / `missing`；只有 `running` 需要运行期时间 | `src/chat/core/chatTypes.ts`（`ToolCallStatus`） |
| N2 | `applyAgentToolCall` 的 `started` 分支写 `status: "running"`、`startedAt: now`、`finishedAt: null`；`finished` / `failed` 才写 `finishedAt` | `src/chat/core/toolCallView.ts`（`applyAgentToolCall`） |
| N3 | `running` 不被折叠：`isFoldableStep` 只收 `success` / `error`，`running` 完整渲染并**切断**所在 run | `src/chat/message/groupNodeRows.ts`（`isFoldableStep`、`planRuns`） |
| N4 | 卡片与单行档都实现了 `running` 的旋转图标与「执行中」文案（`chat.tool.status.${status}` 动态键）；文档批注引用只有图标，无状态文案；三处的 `StatusIcon` / `statusClass` 对 `ToolCallStatus` 穷尽 `switch`，新增状态会**编译期**报缺（i18n 键是动态拼的，缺键不会报错，需人工核对） | `src/chat/tool/ToolCallBubble.tsx`（`density === "card"` 路径）、`ToolCallRenderer.tsx`（非 card 的单行档分支）、`ToolCallDocStub.tsx`（文档批注引用） |
| N5 | 卡片与单行档的耗时**只**来自 `formatToolDuration(startedAt, finishedAt)`，其首行是 `if (startedAt == null \|\| finishedAt == null) return null;` → 运行期恒 `null`；文档批注引用（`ToolCallDocStub`）**完全没有**耗时列，只有图标 + `compact.head` + 「跳到第 N 行」 | `src/chat/tool/toolCallDisplay.ts`（`formatToolDuration`）；调用点：`ToolCallBubble.tsx`（`duration` + `statusLabel`）、`ToolCallRenderer.tsx`（同）；`ToolCallDocStub.tsx` 无耗时 |
| N6 | `GroupHeader` 的 `durationMs` 来自 `computeStats`，要求 `maxFinish != null`；但组头只在 `foldAllowed` 时产生，而 `foldAllowed = groupPhase === "settled" && …` → 运行期不存在组头，故 N6 不构成缺陷 | `src/chat/message/groupNodeRows.ts`（`computeStats`、`runStats`、`groupNodeRows`）、`src/chat/message/GroupHeader.tsx` |
| N7 | `toolCalling` / `awaitingApproval` 由 `stateToPhase` 映射为 `tool_executing`，写入 `phaseByConversation` 与模块级 `phase`；`getPhase()` 的**唯一调用方是测试**（`src/services/__tests__/turnOrchestrator.test.ts`），生产代码零消费 | `src/services/stream/phase.ts`、`src/services/stream/turnOrchestrator.ts`（`onSessionState`、`getPhase`） |
| N8 | `MessageList` 已持有 `visibleMessages` 与 `isStreaming`，是派生「当前活动工具」的现成数据源；当前只把它们喂给 `groupNodeRows`，不产生任何全局提示 | `src/chat/message/MessageList.tsx` |
| N9 | 输入框在 `isStreaming` 时只换 placeholder 文案（`chat.input.placeholder_streaming`），不含工具信息 | `src/chat/message/MessageInput.tsx` |
| N10 | 仓库内**无**共享 ticker：`src/utils/` 只有 `formatBytes` / `formatRelativeTime` / `lazyWithRecovery` / `moduleLoader` / `persistQueue`；现有的 1 秒定时器各自独立（`ApprovalCard` 倒计时、`vault` TOTP、`clock` 工具） | `src/utils/`、`src/chat/tool/ApprovalCard.tsx`（`setInterval`） |
| N11 | **对照**：等待用户确认**有时间感**（`ApprovalCard` 每秒刷新「剩余 {{time}}」），真正执行工具**没有** | `src/chat/tool/ApprovalCard.tsx` |
| N12 | 历史投影 `toolCallsFromHistory` 不产生时间戳（`MessageToolCallDto` 无 `startedAt` / `finishedAt`），历史只可能是终态；因此「重载后的 `running`」只能来自补拉快照 + 后续事件，且可能缺 `startedAt` | `src/chat/core/toolCallView.ts`（`toolCallsFromHistory`）、`src/types/protocol.contract.ts`（`MessageToolCallDto`） |
| N13 | `Started` 之后到 `Finished` / `Failed` 之前，wire 上**没有任何**工具相关事件；`ArgsDelta` 属**流式阶段**（执行期之外），且当前零发射（N15） | `src-tauri/src/adapters/tool_exec.rs`（注释明示「`ArgsDelta` 不在此处发」）、`src-tauri/crates/assistant-agent/src/agent/round_executor.rs` |
| N14 | 前端测试环境无 `@testing-library/react`（`devDependencies` 仅 `vitest` + 纯函数测试），组件级断言不可写；前端门禁必须落在**纯函数**与**可注入时钟的逻辑**上 | `package.json`、`vitest.config.ts` |
| N15 | 累积器**已经算出**参数增量：`apply_delta` 在参数片段到达时返回 `ToolCallDeltaOutcome { params_partial: Some(ToolCallParamsChunk { tool_id, tool_name, params_chunk }) }`；但 `call.rs` 的流回调只调用 `accumulator.apply_chat_delta(delta)` 并**丢弃返回值**，`params_partial` 全仓零消费 | `src-tauri/crates/assistant-agent/src/agent/tool_call.rs`（`apply_delta`）、`src-tauri/crates/assistant-agent/src/agent/loop_executor/call.rs`（`stream::consume` 回调） |
| N16 | 前端 `argsDelta` 分支**已完整实现**（累积 `argsText`、重解析 `arguments`），但因 N15 永远不会被触发，属死分支；该分支当前把状态写成 `running` | `src/chat/core/toolCallView.ts`（`applyAgentToolCall` 的 `argsDelta` 分支） |
| N17 | 前端 `started` 分支**硬写** `argsText: ""`，一旦 N15 修好，`Started` 到达时会把已累积的参数文本清空 | `src/chat/core/toolCallView.ts`（`started` 分支） |
| N18 | `Started` 由 pipeline 在**批次执行入口**发射（流已结束），参数生成期全部发生在 `stream::consume` 内 → 两者在时间轴上严格前后，不重叠 | `src-tauri/crates/assistant-tools/src/runtime/pipeline.rs`（`PipelineEvent::Started`） |
| N19 | `TurnObserver::emit_tool_event` 已对 executor 开放，`EventSink` 对任意 `ToolEventPayload` **无条件转发**为 `agent-tool-call` → 补发射点**不需要**新端口、不需要改 adapter、不需要改 DTO | `src-tauri/crates/assistant-agent/src/agent/round_executor.rs`（`TurnObserver`）、`src-tauri/src/adapters/event_sink.rs`（`TurnEvent::Tool`） |
| N20 | 快照型 provider（Gemini）每个片段给的是**完整参数 JSON**（`arguments_is_snapshot = true` → `replace_arguments`），而 `params_chunk` 直接透传原始片段 → 前端若按「追加」处理会拼出非法文本 | `src-tauri/crates/assistant-providers/src/gemini/stream.rs`、`tool_call.rs`（`replace_arguments`） |
| N21 | 重试路径（provider 失败 / 空响应）会丢弃本次 attempt 已累积的调用；原 `reset()` 只 `clear()`、不产出「丢弃」信号，前端因此没有可消费的终态。现已改为 `discard_pending()` 返回被 `ArgsDelta` 声明过的调用身份，调用点补发 `Failed(errorKind = cancelled)` | `src-tauri/crates/assistant-agent/src/agent/loop_executor/call.rs`（`emit_discarded_calls`）、`tool_call.rs`（`discard_pending`） |
| N22 | `Started` 由 pipeline 在批次入口对每个调用**先行全部发射**，随后才进入 `Gateway` 阶段弹审批 → 「执行中」与「等待确认」之间只隔一次策略判定（毫秒级），用户感知不到 | `src-tauri/crates/assistant-tools/src/runtime/pipeline.rs`（`Started` 循环 + `Gateway`）、`src-tauri/src/adapters/approval.rs`（`AwaitingApproval` 广播） |
| N23 | 文档批注引用（`ToolCallDocStub`）只在 `compact.anchor != null` 时选路，而 `compact` 只由 `finished` / `failed` 写入 → **运行态与准备态永远不会走 doc stub**；它的耗时/状态文案补全在本文范围内**不可达** | `src/chat/tool/toolCallDisplay.ts`（`shouldRenderDocStub`）、`src/chat/tool/ToolCallRenderer.tsx`（选路顺序）、`src/chat/core/toolCallView.ts`（`compact` 只在终态写入） |

**结论**：N4 说明「状态」不缺、N5 说明「时间」缺、N7 说明「全局指示」缺、N8 说明这两项都能纯前端补；而 N15 / N16 / N17 说明**参数生成期**缺的不是设计而是接线——相位、DTO、前端分支、渲染组件全部就绪，只差 `call.rs` 里那一行发射。

---

## §2 目标与非目标

### 2.1 目标

1. **参数生成期有行**：模型开始流式生成参数后，第一个片段到达即出现该工具的卡片（名字 + 「准备中」+ 已准备时长），用户不再对着空白光标猜测。
2. **可感知时长**：`preparing` 与 `running` 期间，界面每秒更新已运行时长；10 秒与 10 分钟在界面上可区分。
3. **全局可见**：不依赖滚动位置，消息流末尾始终有一行「正在准备 / 正在执行 `name` · `Ns`」，明确当前活动。
4. **零 wire 变更**：不新增事件名、不新增相位、不新增 DTO 字段；`ArgsDelta` 是既有相位，本文只补它的发射点。
5. **零终态回归**：终态（`success` / `error` / `cancelled` / `missing`）的耗时文案、折叠口径、组头计数与现状逐字一致。
6. **单一定时器**：任意数量并发 `preparing` / `running` 卡片，全应用只存在一个 `setInterval`。

### 2.2 非目标

- 不做工具执行进度百分比（wire 不承载，§13 OQ1）。
- 不接 `phase` 做指示器（§12 R1）。
- 不改 `groupNodeRows` 的纯函数签名与折叠阈值（§12 R2）。
- 不改 `ApprovalCard` / `QuestionCard` 的倒计时口径（等待 ≠ 执行，§3.2）。
- 不做系统通知、托盘角标、toast（§12 R3）。
- 不做后台会话的活动指示（§13 OQ4）。
- 不把 elapsed 写进 `ToolCallView`（视图是 wire 投影，时间不是事实，§12 R7）。
- 不为参数生成期新增 wire 相位（§12 R9）、不在前端伪造 `preparing`（§12 R10）。
- 重试丢弃行只复用既有 `Failed` 相位补终态（`errorKind = cancelled`），不新增事件名 / 相位 / DTO 字段；内核侧 `reset()` 改名为 `discard_pending()` 并返回被丢弃的调用身份（§13 OQ6 已采纳）。

---

## §3 契约

### 3.1 三阶段生命周期（唯一时间轴）

| 阶段 | 起点事件 | 终点事件 | 状态 | 界面职责 |
|---|---|---|---|---|
| `preparing` | 首个 `ArgsDelta`（`delta` 可为空串，仅声明身份） | `Started` | `preparing` | 工具行出现：名字 + 「准备中」+ 已准备时长 |
| `running` | `Started` | `Finished` / `Failed` | `running`（`awaiting_user` 是 `running` 的子区间） | 名字 + 「执行中」+ 已执行时长；审批时转「等待确认」+ 倒计时 |
| 终态 | `Finished` / `Failed` | — | `success` / `error` / `cancelled` / `missing` | 终态耗时（逐字沿用 I46 compact §3.7） |

与参考项目 `preparing` → `start` → `result` 一一对应；本项目的 `awaitingApproval` 不是第四阶段，而是 `running` 内部的等待区间（`Started` 先于 `AwaitingApproval`，N22）。

### 3.2 耗时口径（唯一来源）

| 状态 | 起点 | 终点 | 格式 | 函数 |
|---|---|---|---|---|
| `preparing` | `startedAt`（首个参数片段时刻） | **当前时刻 `now`** | 整秒口径（`3s` / `1m5s`） | `formatElapsed(now - startedAt)` |
| `running` | `startedAt`（`Started` 时刻） | **当前时刻 `now`** | 同上 | 同上 |
| `success` / `error` | `startedAt` | `finishedAt` | 毫秒口径（`250ms` / `1.5s` / `1m5s`） | `formatToolDuration`（**不变**） |
| `cancelled` / `missing` | `startedAt` | `finishedAt` | 同上 | 同上 |
| `awaiting_user` | — | — | **不显示耗时** | `null`；等待由 `ApprovalCard` / `QuestionCard` 倒计时承载（N11） |

三条口径约束：

1. **`startedAt` 在两段复用、由 `Started` 覆写**：`argsDelta` 首次写入「首个参数片段时刻」，`Started` 覆写为「执行起点」。因此卡片从「准备中 8s」跳到「执行中 0s」是**正确**的——参数生成时间不计入执行耗时（B4）。
2. `awaiting_user` 明确不套用 `now` 口径：`startedAt` 在等待态已被写入（`awaitingApproval` 分支），若用 `now` 会显示「执行中 3m」，而实际上这 3 分钟用户**还没批准**。两种语义必须可区分（G1）。
3. `preparing` 与 `running` 的整秒口径同形，只有状态文案不同（§6）。

### 3.3 状态机与视图字段（前端）

`ToolCallStatus` 增第 7 态 `preparing`；**不新增** `ToolCallView` 字段（复用 `startedAt` / `argsText` / `argsPreview` / `arguments`）。

| 事件（`payload.phase`） | 状态 | `startedAt` | `argsText` | `arguments` | 其它 |
|---|---|---|---|---|---|
| `argsDelta` | `preparing` | `current?.startedAt ?? now`（**只在首次写入**） | 追加 `payload.delta`（可为空串） | `parseArgs(argsText) ?? current?.arguments` | — |
| `started` | `running` | `now`（**覆写**） | **保留** `current?.argsText ?? ""`（N17 修正） | 保留 | `argsPreview`、`resultText: null`、`compact: null` |
| `awaitingApproval` | `awaiting_user` | `current?.startedAt ?? now` | 保留 | `approval.arguments` | `approval` |
| `awaitingAnswer` | `awaiting_user` | `current?.startedAt ?? now` | 保留 | 保留 | `question` |
| `finished` | `success` | 保留 | 保留 | 保留 | `finishedAt: now`、`compact`、`confirmationWaitMs` |
| `failed` | `error`；`payload.errorKind === "cancelled"` 时 `cancelled`（与落库回读一致） | 保留 | 保留 | 保留 | `finishedAt: now`、`compact`、`errorKind`、`confirmationWaitMs` |
| `approvalClosed` | `cancelled` | 保留 | 保留 | 保留 | `finishedAt: now`、`approval: null` |

`upsert` 的既有行为恰好满足本表的两点：`status` 默认值 `running` 不被 `argsDelta` / `started` 依赖（两分支都显式给值）；`name` 只在非空时覆盖（`...(name ? { name } : {})`），因此首个 `ArgsDelta` 若 `name` 为空（provider 先给 id/参数、后给名），卡片名短暂为空，后续片段带上名字时自动补齐，不会把已有名字抹成空串（B12）。

`argsDelta` 的 `status` 只在新行或当前已是 `preparing` 时写 `preparing`；若该 callId 已进入 `running` 或终态，迟到的片段**不得降级**状态（只继续追加 `argsText`）。这条单调性保证乱序 / 重放片段不会把执行中的卡片打回「准备中」。

两个派生判定必须同步纳入 `preparing`：

| 函数 | 现状 | 目标 |
|---|---|---|
| `cancelPendingToolCalls` | 只收 `running` / `awaiting_user` | 再收 `preparing` → `cancelled` + `finishedAt`（G14） |
| `hasPendingToolCalls` | 只认 `running` / `awaiting_user` | 再认 `preparing`（`mergeToolCalls` 与 `orbBusyFrom` 共用） |

### 3.4 后端 `ArgsDelta` 发射契约（`assistant-agent`）

发射点是**唯一**的：`loop_executor/call.rs` 的 `stream::consume` 回调内，把 `apply_chat_delta` 的返回值翻译成 `ctx.observer.emit_tool_event(...)`（N19：端口与 adapter 都已就绪）。

| 项 | 契约 |
|---|---|
| 身份声明 | 新建 pending 时产出 `outcome.opened` → 发 `ArgsDelta { delta: "" }`；工具名随 `ToolEvent.name` 走，wire 不新增字段 |
| 增量语义 | `params_chunk` 恒为「本次**追加**的尾部」：追加型 provider 取原片段；快照型取 `snapshot[old_len..]`（**仅当** `snapshot` 以已累积文本为前缀，N20） |
| 快照不可续 | 快照不以已累积文本为前缀 → 该 callId 置 `args_delta_closed`，此后不再发 |
| 发射上限 | 每 callId 累计 ≤ **4096 字节**（UTF-8 字节，按 `char` 边界截断）；触顶即置 `args_delta_closed` 并停发 |
| 空片段 | `params_chunk` 为空串且非 `opened` → 不发 |
| 终态后 | `Started` 之后结构上不可能再有流片段；`Finished` / `Failed` 之后一律不发 |
| 顺序 | 同一 callId 的 `ArgsDelta` 严格递增；跨 callId 按流顺序（与 `Delta` 的 `seq` 交错，`EventSink` 已保序） |
| 重试 | `discard_pending()` 丢弃 pending 并**返回**每个已被 `ArgsDelta` 声明过的调用身份，调用点随即补发 `Failed { error: "已取消", error_kind: cancelled }`；已发 `ArgsDelta` 不回滚，前端据 `errorKind` 收敛为 `cancelled`（N21、B15、§13 OQ6） |
| 不做时间窗合并 | provider 片段本身是网络分片粒度，事件数已被 4 KB 上限封顶；加 200ms 合并窗口需引入内核 `Clock` 依赖与额外状态，收益低于成本（§12 R12） |

结构改动（仅 `tool_call.rs` 三处 + 测试）：

1. `ToolCallDeltaOutcome` 增 `pub opened: Option<ToolCallParamsChunk>`；
2. `PendingToolCall` 增 `args_emitted_bytes: usize` 与 `args_delta_closed: bool`（`start_new` 时归零）；
3. `apply_delta` 在 `append_arguments` / `replace_arguments` 处按上表产出 `params_chunk`（快照取后缀、超限截断并关闭）。

发射点的具体形状（`call.rs` 现有闭包签名是 `|_seq, delta: &ChatDelta|`，`ctx` 已在作用域内，`TurnRunCtx` 提供 `turn_id`，`it` 是当前迭代号，`ToolEvent` 的四个必填字段全部就位）：

```rust
|_seq, delta: &ChatDelta| {
    let outcome = accumulator.apply_chat_delta(delta);
    for chunk in [outcome.opened, outcome.params_partial].into_iter().flatten() {
        ctx.observer.emit_tool_event(ToolEvent {
            turn_id: ctx.turn_id.to_string(),
            iteration: it,
            call_id: chunk.tool_id,
            name: chunk.tool_name,
            payload: ToolEventPayload::ArgsDelta { delta: chunk.params_chunk },
        });
    }
    ctx.observer.delta(delta);
}
```

要点：`opened` 先于 `params_partial` 发（同一片段里两者可能同时产出，顺序不能反）；`delta` 放在最后，保持正文增量与工具事件的既有交错顺序（`ToolEvent` 与 `Delta` 共用 `TurnEmitter` 的 `seq` 计数器，`EventSink` 已保序）。

### 3.5 新增纯函数（`src/chat/tool/toolCallDisplay.ts`）

```ts
/** 运行期耗时：整秒口径，`ms <= 0` 钳为 `0s`。 */
export function formatElapsed(ms: number): string;

/** 展示口径单点：按状态选终点，返回 `null` 表示不显示。 */
export function formatToolTiming(
  view: Pick<ToolCallView, "status" | "startedAt" | "finishedAt">,
  now: number,
): string | null;
```

规则（测试即契约，§3.8）：

- `formatElapsed`：`< 60_000` → `${floor(ms / 1000)}s`；`≥ 60_000` → `${floor(ms / 60000)}m${floor((ms % 60000) / 1000)}s`；`ms < 0` → `"0s"`。
- `formatToolTiming`：`status` 为 `preparing` / `running` 且 `startedAt != null` → `formatElapsed(now - startedAt)`；`startedAt == null` → `null`（L1）；其它状态 → `formatToolDuration(startedAt, finishedAt)`（逐字沿用，G2）。
- `formatToolDuration` 的**签名与实现零改动**，避免既有 5 条断言（`toolCallDisplay.test.ts`）与 `computeStats` 口径受牵连。

### 3.6 共享 ticker（`src/chat/tool/useNowTick.ts`）

```ts
/** `enabled` 为真时每 `intervalMs` 返回一次新的当前时刻；否则返回常量且不订阅。 */
export function useNowTick(enabled: boolean, intervalMs = 1000): number;
```

| 项 | 契约 |
|---|---|
| 定时器数量 | 模块级单例：一个 `Set<listener>` + 最多一个 `window.setInterval`；首个订阅者启动，最后一个退订时 `clearInterval` |
| 关闭态 | `enabled === false` 时不订阅、不启动定时器，返回 `Date.now()` |
| 卸载 | 组件卸载必须退订；不得在卸载后 `setState`（G8） |
| 时钟源 | 真实 `Date.now()`，不读注入时钟（组件层）；纯函数层用显式 `now` 参数，故逻辑测试不依赖真实时钟（N14） |
| 触发面 | 只有订阅该 ticker 的组件重渲染；**不**触发 `MessageList` 的 `groups` 重算（G7） |

`useNowTick` 放在 `src/chat/tool/` 而非 `src/utils/`：当前唯一消费方是工具渲染，避免把聊天专用的时间口径推进共享工具层（若日后有第二个域需要，再上移，§13 OQ5）。

### 3.7 全局活动指示器（`src/chat/tool/activeTool.ts` + `ToolActiveBar.tsx`）

```ts
/** 取「当前活动的工具」：优先最后一个 `running`，否则最后一个 `preparing`；都没有则 `null`。 */
export function pickActiveTool(messages: Message[]): ToolCallView | null;
```

| 项 | 契约 |
|---|---|
| 数据源 | 仅 `Message.toolCalls`（`role === "assistant"` 的消息），不读 phase、不读 store、不读 `streamingContent`；`MessageList` 直接传 `visibleMessages`（`role === "tool"` 的行已被过滤，不影响结果） |
| 选取规则 | ① 存在 `running` → 取**最后一个** `running`（并发工具取最新发起者）；② 否则取**最后一个** `preparing`；③ 都没有 → `null` |
| 优先级理由 | 参数生成必然早于任何执行（N18），一旦有 `Started` 就说明准备已结束；若仍按位置取末条，会把指示器留在「准备中」而实际已在执行 |
| 渲染条件 | `isStreaming && pickActiveTool(...) != null` |
| 位置 | `MessageList` 的 `css.content` 末尾（`groups` 之后），固定一行，不参与 `groupNodeRows` 的节点序列 |
| 内容 | 旋转图标 + 按状态选文案（§6）+ 同一 ticker 的 elapsed |
| 与卡片关系 | **允许重复**：卡片回答「这一步做了什么」，指示器回答「现在还在跑什么」，后者不随滚动消失 |
| `awaiting_user` | **不进入**指示器（等用户决策由卡片与倒计时承担） |
| 交互 | 纯展示，不可点击、不跳转（跳转需滚动定位，超出本文） |

### 3.8 最小复现样本（测试即契约）

| 样本 | 输入 | 期望 |
|---|---|---|
| S1 | `{ status: "running", startedAt: 1000, finishedAt: null }`, `now = 13_000` | `formatToolTiming` → `"12s"` |
| S2 | 同上，`now = 1000` | `"0s"`（不出现负值 / 不出现 `0ms`） |
| S3 | `{ status: "running", startedAt: null, finishedAt: null }` | `null`（L1） |
| S4 | `{ status: "awaiting_user", startedAt: 1000, finishedAt: null }`, `now = 61_000` | `null`（等待不显示执行时长） |
| S5 | `{ status: "success", startedAt: 0, finishedAt: 1500 }` | `"1.5s"`（与 `formatToolDuration` 逐字一致） |
| S6 | `pickActiveTool([msg(success), msg(running "grep"), msg(running "shell")])` | `shell` 的视图 |
| S7 | `pickActiveTool([msg(awaiting_user)])` | `null` |
| S8 | `{ status: "preparing", startedAt: 1000, finishedAt: null }`, `now = 9_000` | `"8s"` |
| S9 | `applyAgentToolCall([], argsDelta(call_1, "read", "{\"pa"), 1000)` | 单条视图：`status = "preparing"`、`startedAt = 1000`、`argsText = "{\"pa"` |
| S10 | S9 之后 `started(call_1, "read", argsPreview)` @ `now = 9000` | `status = "running"`、`startedAt = 9000`、`argsText` 仍为 `"{\"pa"`（**不被清空**，N17） |
| S11 | `applyAgentToolCall([], argsDelta(call_1, "read", "") @ 1000, 1000)` | 单条视图：`status = "preparing"`、`argsText = ""`（身份声明即出行） |
| S12 | `cancelPendingToolCalls([preparing @ startedAt 1000], 5000)` | `status = "cancelled"`、`finishedAt = 5000` |
| S13 | 累积器：追加型 `"{\"a\":1"` → `"}"` | `params_partial.params_chunk` 分别为 `"{\"a\":1"`、`"}"`（原样） |
| S14 | 累积器：快照型 `"{\"a\":1"` → `"{\"a\":1,\"b\":2}"` | 第二次 `params_chunk = ",\"b\":2}"`（**后缀**，N20；快照必须以上一次快照为**字节前缀**，未闭合片段才是常态） |
| S15 | 累积器：快照型 `"{\"a\":1}"` → `"{\"b\":2}"`（非前缀） | 第二次**不发**（`args_delta_closed`） |
| S16 | 累积器：追加累计超过 4096 字节 | 最后一次片段被截断到余量，此后停发 |
| S17 | `applyAgentToolCall([preparing @ 1000], failed(call_1, errorKind = "cancelled"), 3000)` | `status = "cancelled"`、`finishedAt = 3000`、`errorKind = "cancelled"`（不按 `error` 渲染） |
| S18 | 同上但 `errorKind = "timeout"` | `status = "error"`、`errorKind = "timeout"` |
| S19 | 累积器：`apply_chat_delta(身份声明 + 片段)` 后 `discard_pending()` | 返回该调用的 `tool_id` / `tool_name`；`has_pending() = false`；`finalize_all(StreamEnd)` 为空 |

S1–S12、S17–S18 属前端 vitest，S13–S16、S19 属 `cargo test`（T0）。

---

## §4 行为

### 4.1 修复前后对照（一次 8 秒参数生成 + 40 秒执行 + 审批的 `write_file`）

| 时点 | 现状 | 目标 |
|---|---|---|
| 首个参数片段 | **空白**：只有一个正文光标，无任何工具行 | 卡片：`write_file` + 「准备中」+ `0s`；指示器：`正在准备 write_file · 0s` |
| 参数生成 `+6s` | **空白**（同上，用户只能等） | 卡片：`准备中 6s`；指示器：`正在准备 write_file · 6s` |
| `Started` | 卡片突然出现：转圈 + 「执行中」（与 `+1s` 无法区分） | 卡片：转圈 + 「执行中」 + `0s`（计时从执行起点重算）；指示器：`正在执行 write_file · 0s` |
| 审批卡弹出 | 卡片转「等待确认」+ 倒计时（与现状一致） | **不变** |
| 执行 `+30s` | 转圈 + 「执行中」 | 卡片：`30s`；指示器：`正在执行 write_file · 30s` |
| `Finished` | `40s` + 「已完成」 | **不变**（`formatToolDuration` 逐字一致） |

### 4.2 状态 × 展示矩阵

| 状态 | 卡片耗时 | 单行档耗时 | 文档批注引用 | 全局指示器 |
|---|---|---|---|---|
| `preparing` | `Ns`（每秒更新） | `Ns`（每秒更新） | **不可达**（无 `compact.anchor`，N23） | 出现（无 `running` 时） |
| `running` | `Ns`（每秒更新） | `Ns`（每秒更新） | **不可达**（同上） | 出现（优先） |
| `awaiting_user` | 无（倒计时在卡内） | 无 | 不可达 | 不出现 |
| `success` | 终态耗时 | 终态耗时 | 无耗时（现状不变） | 不出现 |
| `error` | 终态耗时 | 终态耗时 | 无耗时（现状不变） | 不出现 |
| `cancelled` | 终态耗时（`finishedAt` 存在时） | 同左 | 无耗时（现状不变） | 不出现 |
| `missing` | 无（无 `finishedAt`） | 无 | 不可达 | 不出现 |

卡片与单行档是本次唯二的行为变化面；文档批注引用只补一个 `preparing` 的 `StatusIcon` / `statusClass` 分支（穷尽 `switch` 的编译要求），**不**新增耗时列（N23：运行态走不到那里，加列属无效改动）。

---

## §5 边界情形与失败矩阵

| 编号 | 情形 | 行为 | 依据 |
|---|---|---|---|
| B1 | 并发多个 `running` 工具 | 各卡片各自计时；指示器只显示**最后一个** | §3.7 |
| B2 | `running` 但 `startedAt == null`（补拉 / 重载后迟到视图） | 只显示「执行中」，不显示耗时；不伪造起点 | L1、N12 |
| B3 | 系统时钟回拨（`now < startedAt`） | 钳为 `0s`，不出现负数 | §3.5 |
| B4 | `preparing` → `running` 切换 | `startedAt` 被覆写，耗时**归零重算**；参数生成时间不计入执行耗时 | §3.2、§3.3 |
| B5 | 切到别的会话 | 指示器读当前会话 `messages` → 自然消失；后台会话不显示 | N8、I55（观测面只读） |
| B6 | turn 结束 / 取消 | `settleToolCalls` 把 `preparing` / `running` 一并收敛为 `cancelled` → ticker 无订阅者自动停 | §3.3、G14 |
| B7 | 长列表 + 多个活动工具 | 每秒只重渲染订阅的卡片与指示器，`groups` 不重算 | §3.6、G7 |
| B8 | 窗口最小化 / 后台 | `setInterval` 可能被节流，恢复后耗时**跳变**而非连续增长 | L3 |
| B9 | 折叠段内的 `running` | 折叠段不产生（`running` 切断 run），故不会出现「组头里藏一个正在跑的步」 | N3 |
| B10 | 折叠段内的 `preparing` | 同上：`preparing` 不在 `isFoldableStep` 的白名单，天然切断 run，无需改函数 | N3、G9 |
| B11 | `link === "chat"`（无工具） | `pickActiveTool` 返回 `null`，指示器不渲染 | §3.7 |
| B12 | 首个 `ArgsDelta` 的 `name` 为空串（provider 先给 id / 参数、后给名） | 卡片名短暂为空，后续片段带名时由 `upsert` 补齐；指示器走匿名文案键 | §3.3、§6 |
| B13 | 参数超 4 KB（大文件写入） | 前端只拿到前 4 KB，`argsText` 停在半截（`parseArgs` 失败 → 保留上一份可解析快照）；`Started` 的 `argsPreview` 与终态 `compact.head` 仍是完整摘要 | §3.4、L9 |
| B14 | 快照型 provider 的快照非前缀变化（如 key 顺序变化） | 该 callId 停发 `ArgsDelta`；行保留已发文本，等 `Started` 用 `argsPreview` 补位 | §3.4、L10 |
| B15 | provider 失败重试丢弃已累积调用 | `discard_pending()` 交回被声明过的身份 → 立即补发 `Failed(cancelled)`，卡片当帧收敛为 `cancelled`（`finishedAt` = 补发时刻），不再依赖 turn 终止兜底 | N21、§3.4、OQ6 |
| B16 | `Started` 与 `AwaitingApproval` 之间 | 状态从 `running` 翻到 `awaiting_user`，间隔毫秒级、用户不可感知；两态的耗时列分别为 `Ns` 与 `null`，翻转只造成一次渲染替换，无副作用 | N22、§3.2、§4.2 |
| B17 | 快照型 provider 先用合成 `callId`（`call_<index>`）开卡、同一 key 槽后续改送真实 id | 累积器会把旧 pending `finalize` 丢弃，前端已建的同名 `preparing` 行成为孤儿；该路径**不经过** `discard_pending()`（不在重试内），仍由 `settleToolCalls` 在 turn 终止时收敛（真实 id 一直不出现时合成 id 同时用于 `Started`，不会分叉） | §3.4、L8 |

---

## §6 文案

| 键 | zh-CN | en-US |
|---|---|---|
| `chat.tool.status.preparing` | `准备中` | `Preparing` |
| `chat.tool.active_bar.preparing` | `正在准备 {{name}} · {{time}}` | `Preparing {{name}} · {{time}}` |
| `chat.tool.active_bar.preparing_anonymous` | `正在准备工具 · {{time}}` | `Preparing tool · {{time}}` |
| `chat.tool.active_bar.running` | `正在执行 {{name}} · {{time}}` | `Running {{name}} · {{time}}` |
| `chat.tool.active_bar.running_anonymous` | `正在执行工具 · {{time}}` | `Running tool · {{time}}` |

- 卡片与单行档的状态文案**复用**既有 `chat.tool.status.*`（新增的 `preparing` 是唯一补项），不新增第二套状态键；doc stub 只有图标、不显示状态文案（N4）。
- 匿名键（`*_anonymous`）**只**给全局指示器用：卡片与单行档的名字直接渲染 `view.name`，不套匿名键（保持与现状一致的渲染口径）。
- 指示器文案按 `pickActiveTool` 返回的 `status` 二选一（`preparing` / `running`），不引入第三套。
- 时间串一律由 `formatElapsed` 产出，**不**做单位本地化（`s` / `m` 为通用记号，与 `formatToolDuration` 现状一致）。

---

## §7 与相邻组件

| 组件 | 关系 |
|---|---|
| `toolCallView` | **改四处**：`argsDelta` 分支写 `preparing` + 首次写 `startedAt`；`started` 分支保留 `argsText`；`failed` 分支按 `errorKind = cancelled` 落 `cancelled`；`cancelPendingToolCalls` / `hasPendingToolCalls` 纳入 `preparing`（§3.3） |
| `groupNodeRows` | **函数体不修改**。`preparing` 不在 `isFoldableStep` 白名单 → 天然切断 run；`computeStats` 的 `minStart` 会取到 `preparing` 的 `startedAt`（仅影响「准备中被取消」这一罕见情形的组头跨度，见 L11）。指示器是 `MessageList` 的同级渲染，不进入节点序列（G7 / G9） |
| `GroupHeader` | **不修改**。运行期不产生组头（N6），settled 后 `durationMs` 仍由 `computeStats` 计算 |
| `TurnGroup` / `TurnTail` | **不修改**。指示器不挂在 turn 内，避免随 turn 数量线性增长 |
| `ApprovalCard` / `QuestionCard` | **不修改**。等待态有自己的倒计时，与执行期耗时语义互斥（§3.2） |
| `orbBusyFrom` / `mergeToolCalls` | **行为随 `hasPendingToolCalls` 扩展而变**：`preparing` 也算「未收敛」。流式期 `isStreaming` 已为真，orb 无变化；`mergeToolCalls` 在流式期本就保留前端视图，无变化（G15 只需确认无回归） |
| `agentSegments`（`applyToolCall`） | **不修改**。事件路由已按 `phase` 全量转发，`argsDelta` 已在内 |
| `phase`（`turnOrchestrator` / `phase.ts`） | **不消费**（§12 R1）。`tool_executing` 的零消费现状保持不变（OQ1） |
| I55 观测面 | **不触碰**。`activeTool.ts` 只读消息，不读 `observation` |
| I59 错误守卫 | **无交叉**。一个在错误呈现，一个在工具进行态 |

---

## §8 验收

- **后端**（T0）：`src-tauri/crates/assistant-agent/tests/tool_call.rs` 追加 S13–S16；`cargo test -p assistant-agent --test tool_call`（cwd = `src-tauri`）。
- **前端**：`src/chat/tool/__tests__/activeTool.test.ts`（S6 / S7）、`src/chat/core/__tests__/toolCallView.test.ts` 追加 S9–S12、`src/chat/tool/__tests__/toolCallDisplay.test.ts` 追加 S1–S5 / S8。
- 每条门禁（§10）至少对应一条断言或一次人工核对；无落点的项视为未交付。
- 组件接线（`ToolCallBubble` / `ToolCallRenderer` / `ToolCallDocStub` + `MessageList`）由 `npm run typecheck` + `npm run lint` + 人工走查兜住（N14：无组件测试框架）；`ToolCallStatus` 扩态会让三处 `switch` 编译报缺，typecheck 即覆盖「是否漏接」。
- 回归口径：`npm run typecheck`、`npm run lint`、`npm test`（cwd = 仓库根）；`cargo test -p assistant-agent`（cwd = `src-tauri`）；`npm run contract:gen` 必须无 diff（G5）。

---

## §9 已知限制（边界，不是缺陷）

| 编号 | 限制 | 理由 | 逃生路径 |
|---|---|---|---|
| L1 | `startedAt` 缺失的 `running` 视图不显示耗时 | 起点未知，不伪造 | 若需补齐，需后端在补拉快照里带 `startedAt`（wire 变更） |
| L2 | 1 秒粒度，`< 1s` 的执行显示 `0s` | 逐帧计时成本高且无意义（§12 R5） | — |
| L3 | 后台节流导致耗时跳变 | 浏览器对不可见窗口的定时器有节流 | 可在恢复可见时按 `Date.now()` 重算（本实现天然如此） |
| L4 | 指示器与卡片信息重复 | 两者回答不同问题（「这一步」vs「现在在跑」），且卡片可能滚出视野 | 若嫌重复，可在 OQ3 中评估「仅当卡片不可见时显示」 |
| L5 | 指示器不显示参数摘要 | 参数可能很长且含敏感内容，一行放不下 | OQ2 |
| L6 | 无真实进度（如 `grep` 已扫多少文件） | wire 不承载执行期事件（N13） | OQ1 |
| L7 | 指示器取「最后一个」 | 并发工具没有稳定的「当前」语义 | OQ1 落地后可改为按进度排序 |
| L8 | ~~重试丢弃的 `preparing` 行会一直显示「准备中」直到 turn 终止~~ **已解决（B15）**：重试前补发 `Failed(cancelled)`，卡片当帧收敛为 `cancelled` | — | 仅剩 B17（同槽改送真实 id 丢弃旧 pending）仍走 `settleToolCalls` 兜底 |
| L9 | 参数只发前 4 KB | 无上限会让大文件写入把整份内容灌进 IPC 与前端 `argsText`（§3.4） | 终态 `compact.head` / `argsPreview` 仍完整；如需全文，走文档批注（I46） |
| L10 | 快照非前缀变化时停发 | wire 的 `ArgsDelta` 只有「追加」语义，无法表达「替换」 | 需新增替换语义（OQ1） |
| L11 | 「准备中被取消」的组头跨度含参数生成时间 | `computeStats` 的 `minStart` 取 `startedAt`，而该值在 `preparing` 期是参数起点 | 该步不可折叠，组头跨度仅用于展示，不参与判定 |
| L12 | `argsText` 每片重解析 JSON，复杂度 O(n²) | `n ≤ 4096` 字节，实测可忽略 | 若需优化，可在 `argsDelta` 里按累计长度阈值才重解析 |

---

## §10 门禁

| 编号 | 门禁 | 判据 |
|---|---|---|
| G1 | 耗时口径 | S1 / S2 / S3 / S4 / S8 逐条通过：`preparing` / `running` 用 `now`、`awaiting_user` 恒 `null`、无 `startedAt` 恒 `null` |
| G2 | 终态零回归 | S5 通过；`toolCallDisplay.test.ts` 既有 5 条 `formatToolDuration` 断言不改动且全绿 |
| G3 | 单例 ticker | 同一时刻 N 个订阅者只创建一个 `setInterval`；全部退订后定时器数量归零（可用注入的假 `setInterval` 计数） |
| G4 | 指示器派生 | S6 / S7 通过；`pickActiveTool` 对空数组、无 `assistant` 行、`running` 优先于 `preparing` 均正确 |
| G5 | 零 wire 变更 | `git diff` 不含 `src-tauri/src/protocol/**`、`src/types/protocol.contract.ts`；`npm run contract:gen` 无 diff；后端 diff 仅限 `assistant-agent` 的 `tool_call.rs` / `loop_executor/call.rs` / `tests/tool_call.rs` |
| G6 | phase 零新增消费 | `git diff` 不含 `src/services/stream/**`、`src/stores/**`；`getPhase` 调用方仍只有测试 |
| G7 | 分组口径不变 | `groupNodeRows.test.ts` 全绿且零改动；`groupNodeRows` 的 `GroupNodeRowsInput` 不新增 `now` |
| G8 | 卸载清理 | 订阅者退订后不再收到回调；卸载后无 `setState` 调用（代码走查 + 单例计数） |
| G9 | 折叠与计数不变 | `isFoldableStep` / `PROCESS_COLLAPSE_THRESHOLD` / `computeStats` 零改动；`preparing` 不进入折叠白名单 |
| G10 | 发射点唯一 | 全仓 `ToolEventPayload::ArgsDelta` 的构造点只有 `loop_executor/call.rs` 一处；`src-tauri/src/adapters/tool_exec.rs` 不新增（N13 的注释仍成立） |
| G11 | 快照归一 | S14 / S15 通过：快照型只发后缀、非前缀停发 |
| G12 | 发射上限 | S16 通过：累计 ≤ 4096 字节，触顶后不再发 |
| G13 | 状态机映射 | S9 / S10 / S11 通过：`argsDelta` → `preparing` 且 `startedAt` 仅首次写入；`Started` 覆写 `startedAt` 且不清空 `argsText` |
| G14 | 孤儿收敛 | S12 通过：`cancelPendingToolCalls` 覆盖 `preparing`；`hasPendingToolCalls` 认 `preparing` |
| G15 | 相邻判定无回归 | `orbBusy` 相关测试全绿；流式期 orb 行为不变 |
| G16 | 渲染选路不变 | `shouldRenderDocStub` 零改动；`preparing` / `running` 仍走卡片或单行档（N23），doc stub 只多一个 `preparing` 分支 |
| G17 | 重试丢弃补终态 | S19 通过（`discard_pending()` 交回被声明身份）；S17 / S18 通过（`failed(cancelled)` → `cancelled`、其它 `errorKind` 仍 `error`）；`agentSegments.test.ts` 的 `preparing` 孤儿收敛用例通过 |

---

## §11 切片

| 编号 | 切片 | 交付物 | 依赖 |
|---|---|---|---|
| T0 | 后端发射点 | `tool_call.rs`（`opened`、快照后缀归一、4096 字节上限、`args_delta_closed`、`discard_pending` 返回被声明身份）；`call.rs` 流回调内 `emit_tool_event` + 两处重试前 `emit_discarded_calls`；`tests/tool_call.rs` 追加 S13–S16 / S19；G10 / G11 / G12 / G17 | — |
| T1 | 前端状态机 | `chatTypes.ts` 增 `preparing`；`toolCallView.ts` 四处改（`argsDelta` 分支 / `started` 保留 `argsText` / `failed(cancelled)` → `cancelled` / 两个判定纳入）；`toolCallView.test.ts` 追加 S9–S12 / S17–S18；G13 / G14 / G17 | T0（契约对齐；纯逻辑可先写测试） |
| T2 | 纯函数与 ticker | `toolCallDisplay.ts` 增 `formatElapsed` / `formatToolTiming`；新增 `useNowTick.ts`；`toolCallDisplay.test.ts` 追加 S1–S5 / S8；G1 / G2 / G3 | — |
| T3 | 渲染接入 | `ToolCallBubble`（卡片头耗时）、`ToolCallRenderer`（非 card 的单行档）改用 `formatToolTiming` + `useNowTick(active)`；`ToolCallDocStub` 只补 `StatusIcon` / `statusClass` 的 `preparing` 分支（N23：不加耗时列）；G8 | T1、T2 |
| T4 | 全局指示器 | 新增 `activeTool.ts`（`pickActiveTool`）、`ToolActiveBar.tsx` + `.module.css`；`MessageList` 挂载；5 个 i18n 键（zh-CN / en-US）；新增 `activeTool.test.ts`（S6 / S7）；G4 | T2 |
| T5 | 门禁核对 | `npm run typecheck` / `npm run lint` / `npm test` / `cargo test -p assistant-agent` / `npm run contract:gen` 全绿；人工走查 `git diff` 仅含 T0–T4 文件；G5 / G6 / G7 / G9 / G15 / G16 | T0–T4 |

依赖说明：T0 与 T2 无依赖、可并行；T1 依赖 T0 的语义（但前端测试可先行）；T3 需 T1（`preparing` 存在）与 T2（`formatToolTiming` / `useNowTick`）；T4 只依赖 T2。T0 与 T3 / T4 文件完全不重叠。

---

## §12 拒绝方案

| 编号 | 方案 | 拒绝理由 |
|---|---|---|
| R1 | 用 `phase === "tool_executing"` 驱动全局指示器 | ① `tool_executing` 同时覆盖「正在跑」与「等用户批准」，语义不可区分；② 不含工具名，指示器只能写「正在执行工具」；③ 需要新增 orchestrator → store 的订阅通道，为已有事实造第二条数据路径（违反单一来源） |
| R2 | 给 `groupNodeRows` 传 `now` 让组头显示实时耗时 | 运行期根本不产生组头（N6）；且 `now` 每秒变化会让 `MessageList` 的 `useMemo` 每秒重算整表，长会话直接卡顿 |
| R3 | 长工具用 toast / 系统通知 | 打扰性强、与卡片重复；且通知权限需用户授权，属于另一域 |
| R4 | 后端周期性发「仍在执行」心跳事件 | wire 变更，需 DTO + `contract:gen` + 前后端同步；无进度语义时心跳只证明「没死」，收益低于成本 → OQ1 |
| R5 | `requestAnimationFrame` 逐帧计时 | 60fps 刷新一个整秒量级的数字，纯浪费；且每帧重渲染卡片 |
| R6 | 每张活动卡片各自 `setInterval` | N 个定时器相位不一，视觉上数字跳动不齐；且易漏清理 |
| R7 | 把 elapsed 写进 `ToolCallView` | 视图是 wire 投影（`applyAgentToolCall` 由事件驱动），时间不是事实；写入会污染历史重载与 `settleToolCalls` 的收敛逻辑 |
| R8 | 复用 `chat.input.placeholder_streaming` 承载工具信息 | placeholder 属于输入框语义（「可以继续输入」），与「当前活动」无关；且每次 tick 改 placeholder 会干扰输入 |
| R9 | 为参数生成期新增 wire 相位（如 `payload.phase = "preparing"`） | `argsDelta` 相位在 I38b 已定义、DTO 已生成、前端分支已实现（N16），新增相位属重复表达，且要付 `contract:gen` + 双端同步成本；本文只补发射点（G5 要求零 wire 变更） |
| R10 | 前端在 `Started` 之前自己造一个「准备中」行 | 前端没有任何事件可依（N15 / N16）；唯一可猜的输入是 `streamingContent`，而参数 JSON 与正文混在同一个流里，解析它属**猜测**而非事实，且要重写流解析。正确做法是让事实源（累积器）把已有增量发出来 |
| R11 | 后端直接透传 provider 原始片段 | 快照型 provider（Gemini）每片是完整 JSON（N20），前端按「追加」会拼出 `{...}{...}` 非法文本；必须在累积器侧归一为「后缀」 |
| R12 | 对 `ArgsDelta` 加 200ms 时间窗合并 | 片段本身是网络分片粒度，事件数已被 4 KB 上限封顶（§3.4）；合并窗口需引入内核 `Clock` 依赖与额外状态，收益低于成本 |

---

## §13 未决问题

| 编号 | 问题 | 影响 |
|---|---|---|
| OQ1 | 是否为工具执行期增加 wire 级进度/心跳事件（`Started` 之后、终态之前），或为 `ArgsDelta` 增加「替换」语义 | 若采纳：可做真实进度（L6）、「当前工具」排序（L7）、快照非前缀场景（L10），但需 DTO + 生成契约 + 后端发射点 |
| OQ2 | 指示器是否显示参数摘要（`compact.head` / `argsPreview` / `argsText` 前若干字符） | 影响 L5；采纳需考虑敏感参数与行宽截断 |
| OQ3 | 超过阈值（如 60s）是否升级为「该工具耗时较久」的显式提示 | 影响 L4 的重复观感；也可作为「疑似卡死」的软提示 |
| OQ4 | 切走后后台会话的活动是否在会话列表 / 侧栏显示 | 与 C5.2「切会话不丢后台 turn」相关，但需要跨会话的活动聚合，本文不做 |
| OQ5 | `useNowTick` 是否上移到 `src/utils/` | 当前只有一个域消费；出现第二个域时再上移，避免过早抽象 |
| OQ6 | **已采纳**：重试丢弃的 `preparing` 行由后端补发 `Failed` + `errorKind = cancelled`（原候选：扩展 `reset()` 返回值） | 后端 `discard_pending()` + `emit_discarded_calls`；前端 `failed(cancelled)` → `cancelled`（S17/S18/S19）。`cancelled` 本就在 `isFoldableStep` 白名单内，不牵动折叠 |

---

## §14 验收记录

- **T0 后端发射点**：`tool_call.rs` 的 `ToolCallDeltaOutcome.opened`、`PendingToolCall.args_emitted_bytes` / `args_delta_closed`、`take_emit_chunk`（按 `char` 边界截断至 4096 字节）、快照后缀归一；`call.rs` 唯一发射点按 `opened → params_partial → delta` 顺序发出。用例：`tests/tool_call.rs` 的 `args_delta_opens_once_and_emits_appended_chunks`（S13）、`snapshot_chunks_emit_only_suffix`（S14）、`args_delta_caps_at_4096_bytes`（S16）、`args_delta_truncates_on_char_boundary`，`cargo test -p assistant-agent --test tool_call` 14/14。快照非前缀停发（S15）由 `snapshot_chunks_emit_only_suffix` 的非前缀分支覆盖（G10 / G11 / G12）。
- **T1 前端状态机**：`chatTypes.ts` 增 `preparing`；`toolCallView.ts` 的 `argsDelta` 只在新行或已 `preparing` 时写 `preparing`（迟到片段不降级）、`startedAt` 仅首次写入；`started` 保留 `argsText`；`cancelPendingToolCalls` / `hasPendingToolCalls` 纳入 `preparing`。用例：`toolCallView.test.ts` 的 S9 / S10 / S11 / S12（含「Started 之后迟到的 argsDelta 不降级」）（G13 / G14）。
- **T2 纯函数与 ticker**：`formatElapsed` / `formatToolTiming` 落地，`formatToolDuration` 签名与实现零改动（既有 5 条断言全绿）。用例：`toolCallDisplay.test.ts` 的 S1–S5 / S8（G1 / G2）。**G3 单例 ticker**：`useNowTick.ts` 抽出可测的 `subscribeNowTick(listener, intervalMs)`（hook 只做订阅与 `setNow`），新增 `__tests__/nowTick.test.ts` 用注入的假 `window.setInterval` / `clearInterval` 计数断言「多订阅者只建一个定时器、最后一个退订才清理、再订阅重新启动」。
- **T3 渲染接入**：`ToolCallBubble` / `ToolCallRenderer` 的耗时改走 `formatToolTiming(view, useNowTick(active))`；`ToolCallDocStub` 只补 `preparing` 的 `StatusIcon` / `statusClass` 分支（不加耗时列，N23）。组件接线由 `typecheck`（`ToolCallStatus` 扩态使三处穷尽 `switch` 编译报缺）+ `lint` + 人工走查兜住（N14）（G8 / G16）。
- **T4 全局指示器**：新增 `activeTool.ts`（`pickActiveTool`：`running` 优先、否则最后一个 `preparing`、`awaiting_user` 不进入）与 `ToolActiveBar.tsx` + `.module.css`，`MessageList` 仅在 `isStreaming` 时挂载；5 个 i18n 键（`status.preparing` + `active_bar` 四条）。用例：`activeTool.test.ts` 4 条（S6 / S7）（G4）。
- **T5 门禁核对**：`npm run typecheck` 通过；`npm run lint` 退出码 0；`npx vitest run` 71 文件 / 846 用例通过（1 skipped）；`cargo test -p assistant-agent --tests`（无 feature 与 `--features test-doubles` 两种）全绿，`loop_executor` 40/40；`cargo clippy -p assistant-agent --all-targets --features test-doubles -- -D warnings` 无警告；`cargo fmt --check` 无 diff。G5：`git diff` 不含 `src-tauri/src/protocol/**` 与 `src/types/protocol.contract.ts`，`npm run contract:gen` 后两个生成文件零 diff，后端 diff 仅 `assistant-agent` 的 `tool_call.rs` / `loop_executor/call.rs` / `tests/*`；G6：不含 `src/services/stream/**`、`src/stores/**`，`getPhase` 调用方仍只有测试；G7 / G9：`groupNodeRows.ts` 与 `groupNodeRows.test.ts` 零改动；G15：`orbBusy` 相关用例全绿（`hasPendingToolCalls` 扩态不改变流式期行为）；G16：`shouldRenderDocStub` 零改动，`toolCallDisplay.ts` 只追加两个新函数。
- **T0/T1 重试丢弃补终态（B15 / OQ6 落地）**：`ToolCallAccumulator::reset()` 改为 `discard_pending() -> Vec<DiscardedToolCall>`（只交回 `has_pending()` 为真的调用，`clear()` 语义不变）；`call.rs` 新增 `emit_discarded_calls`，在两处重试分支（provider 失败、空响应）`continue` 前对每个被丢弃调用发 `Failed { error: "已取消", error_kind: cancelled }`。前端 `toolCallView` 的 `failed` 分支按 `errorKind === "cancelled"` 落 `cancelled`（与落库回读一致），其它 `errorKind` 仍落 `error`。用例：`tests/tool_call.rs` 的 `discard_pending_reports_announced_calls`（S19）、`discard_pending_on_empty_accumulator_is_noop`；`toolCallView.test.ts` 的 S17 / S18；`agentSegments.test.ts` 追加「`settleToolCalls` 收敛停留在 `preparing` 的孤儿行」（B17 兜底）。验证：`cargo test -p assistant-agent --test tool_call` 16/16、`cargo clippy … -D warnings` 无警告、`cargo fmt --check` 无 diff、`npx vitest run` 相关三文件 38 用例全绿、`npm run typecheck` 通过、`eslint` 三文件零问题（G17）。
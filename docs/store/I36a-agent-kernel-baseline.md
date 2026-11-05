# I36a — Agent 内核能力基线（参考实现对照）

| 项 | 值 |
|---|---|
| 状态 | 现行（能力基线，只读对照） |
| 日期 | 2026-11-05 |
| 关联 | [I38 阶段总纲](./I38-agent-roadmap.md)、[I36 运行时决策](./I36-agent-runtime-design.md)、[I36 运行时规格](./I36-agent-runtime-spec.md)、[I36 任务清单](./I36-agent-runtime-tasklist.md)、[I36b 一致性验证](./I36b-agent-kernel-conformance-spec.md)、[I38b Agent Loop](./I38b-agent-loop-spec.md)、[I38a 工具控制层](./I38a-tool-control-layer-spec.md) |
| 影响范围 | 仅文档；不改变任何代码契约 |

> 本文是 agent 文档的**内核篇**：回答「一个够用且通用的内核必须具备哪些能力、本项目内核处在什么位置、每个缺口的归属阶段与扩展点」。
> 本文**不**回答「怎么做」——实现步骤见 I36 spec / I38b；工具实现见 I38c；前端见 I38d。
> **单一权威源不变**：类型与契约仍归各 spec，本文只做横向对照与基线判定，不复制类型定义。

---

## 1 判定口径

### 1.1 什么算「内核」

内核是**跨迭代决策的唯一持有者**：它决定一轮从开始到结束怎么走、什么时候停、状态怎么迁移、事件与事实怎么落。它不决定单个工具做什么，也不决定界面怎么画。

| 层 | 职责 | 权威文档 | 判据 |
|---|---|---|---|
| turn 编排（kernel） | turn 生命周期、状态机、预算、取消、阶梯、落库、事件 | I36 spec | 跨迭代 |
| 迭代循环（RoundExecutor） | 模型调用 → 工具执行 → 回填 → 下一迭代 | I38b | 跨迭代，但只跨「一次 turn 内」 |
| 工具控制（Policy/Gateway/Pipeline） | 权限、审批、校验、并发分批、结果持久化 | I38a | 单次工具调用前后 |
| 工具实现（fs / shell / MCP） | 具体能力与参数语义 | I38c / I38e | 单次调用内部 |

### 1.2 能力入阶段的判据

一个能力进入 P0–P7，必须同时满足：

1. **单 turn 闭环需要**：缺它，模型 → 工具 → 回填 → 再生成这条链跑不通或不受控。
2. **单会话自足**：不引入跨会话、跨进程、跨重启的状态。
3. **有强制点**：能落到某个不可绕过的执行点（端口、Pipeline、状态机迁移），而不是「约定大家遵守」。

不满足的进 P8+ 或明确不做。这条判据用来回答「够不够通用」：**通用不等于全做**，而是**做过的部分不留分支、没做的部分留好扩展点**。

### 1.3 七条内核不变式

| 编号 | 不变式 | 强制点 |
|---|---|---|
| K1 | 一轮只有一个入口（`TurnRunner::run`），宿主只做 begin / spawn / 映射 | `kernel/runner.rs` |
| K2 | 内核零 tauri，外部世界只经四端口 | `kernel/ports.rs` |
| K3 | 工具执行唯一入口是 `ToolPipeline`，不得裸调 `Tool::execute` | Pipeline 契约；`Tool::execute` 仍是 `pub`，裸调只靠调用点约束（G5 待补） |
| K4 | 事实以落库为准，内存状态只是投影 | `kernel/state.rs` + `kernel/persist.rs` |
| K5 | 取消只置位令牌，不做 `abort`；旧代际的迟到写入一律丢弃 | `kernel/tasks.rs` |
| K6 | 事件恒带 `turn_id`，同 turn `seq` 严格单调；终态只有一个出口 | `kernel/emit.rs` + `runner.rs` |
| K7 | 链路差异只经 `TurnSpec` 参数表达，内核内不写 `if is_agent` | `kernel/mod.rs` |

本表只登记**不变式与强制点**。每条不变式当前是否成立、用什么用例判定，见 [I36b](./I36b-agent-kernel-conformance-spec.md) §3；是否已落地见 [I38 roadmap](./I38-agent-roadmap.md) §2 与各阶段 tasklist。本文不维护「现状」。

### 1.4 溯源：哪些是转述，哪些是新增

本文不是新设计，也不是纯摘抄。按来源分三类：

| 类别 | 内容 | 占比 | 是否会改动现有文档 |
|---|---|---|---|
| **转述** | §2 能力矩阵的「能力」列与「阶段」列（对应 I36 spec Step 0–8、I38b D1–D26、I38a、roadmap §2.1）；§3 的机制描述 | 约 6 成 | 不改。已有决策已定稿，本文只做横向索引 |
| **核对** | §2 的「现状」列（逐模块比对代码与任务清单勾选）；§4.2 收口清单 | 约 2 成 | **会**：状态口径与代码不符的文档需修正（已修 5 处，见 §4.2 末注） |
| **新增** | §1.2 入阶段三判据；§1.3 不变式的强制点；§4.3 接口冲突清单；§5 扩展点表；§6 门禁 G5/G7/G8；§7 优先级 | 约 2 成 | 部分会：G7/G8 需并入 I38b 验收，G5 需可见性收口 |

判定方法：凡是能在既有 spec 里找到原文的，归入「转述」；凡是从参考项目**机制**推出、但本项目文档未写的，归入「新增」。**新增项一律不直接改设计**，只登记为门禁或验收候选，由对应阶段开工时决定是否采纳。

---

## 2 内核能力矩阵

状态列取值：**已落地** / **骨架** / **未实现** / **不做**。参考列只列首个可对照的实现，细节见 §3。

**本表不记录现状**：每行只登记「能力 → 归属 → 参考 → 阶段」。实现进度见 [I38 roadmap](./I38-agent-roadmap.md) §2 与各阶段 tasklist；判定方法见 [I36b](./I36b-agent-kernel-conformance-spec.md) §3。

### 2.1 turn 生命周期

| 能力 | 归属 | 参考 | 决策 / 阶段 |
|---|---|---|---|
| turn 身份（宿主按 `conv:counter` 生成，内核只消费） | kernel | OpenBitFun `turn_cancellation.rs` 按 turn id 建令牌表 | P3 |
| 七态状态机（`Thinking/Streaming/ToolCalling/AwaitingApproval/...`） | kernel | OpenHuman `session_state`、OpenBitFun `SessionState::Processing` | P3 |
| 取消令牌表 + 忙时策略（`Replace` / `Reject`） | kernel | Codex 中断只作用于被 spawn 的 agent，根 agent 不可中断 | P3 |
| 代际守卫（迟到完成不写库 / 不改状态 / 不发事件） | kernel | Codex `interrupt` 快照语义 | P3 |
| 双层墙钟（turn 总时长 + 单次模型调用） | kernel | OpenHuman `turn_policy.rs` 同时设 turn 与单次调用上限 | P3 |
| 流式消费与部分结果 | kernel | OpenHuman `turn_runner.rs` 经 harness 流 | P3 C5 |
| turn 级请求构建 | kernel | OpenBitFun `prompt.rs` / `prompt_cache.rs` | P3 C5 |
| 终态落库（`status` 四值） | kernel | Codex rollout、OpenBitFun `session_event_journal.rs` | P3 C7 |
| 启动清扫（`streaming` → `interrupted`） | 宿主 | DSH 崩溃恢复以 turn 为边界（§3.3） | P3 C7 |
| 停止钩子（两次调用之间刹车） | kernel（trait）+ agent | Goose `StopHook` + `deny_limit`（§3.1） | P4 |

### 2.2 循环与工具执行

| 能力 | 归属 | 现状 | 参考 | 决策 / 阶段 |
|---|---|---|---|---|
| 迭代循环（模型 → 工具 → 回填） | RoundExecutor | 骨架（占位 trait） | OpenHuman `turn_runner`、Goose `StateMachine` | P4 |
| tool call 累积器（跨 chunk、快照模式、截断修复） | agent | 未实现 | OpenBitFun / Codex 三家方言差异 | P4（I38b D1–D9） |
| `ChatOptions.tools` 三 adapter 注入 | providers | 未实现 | Goose / Codex 统一 `Tool` 形状 | P4（D10） |
| 工具结果回填与落库（`role=tool` + `tool_call_id` / `is_error`） | 宿主 + storage | 未实现 | 三家均落库且均不建 tool 表 | P4（D11） |
| 并行批次（`is_concurrency_safe`） | tools | 已落地（P2） | Codex `tools/parallel.rs` 读写闸门 | P4 接线（D20） |
| 结果体积预算与落盘回填 | tools | 已落地（P2） | DSH retention library（§3.3） | P4 接线（D21） |
| 重复失败阶梯（Continue / Nudge / Halt） | agent | 未实现 | OpenHuman `RepeatedToolFailureMiddleware` | P4（D19 / D25） |
| 重复调用豁免（轮询类工具） | agent | 未实现 | OpenHuman `is_repeat_call_exempt`（仅 `wait_subagent`） | P4 空集（D26） |
| 未处理工具调用兜底（fail-closed 回灌错误） | agent | 未实现 | Goose `UnhandledToolCallsOperation`（§3.1） | P4 |
| 审批等待与工具耗时分账 | agent | 未实现 | OpenBitFun `confirmation_wait_ms` 独立字段 | P4（D12） |

### 2.3 上下文与预算

| 能力 | 归属 | 现状 | 参考 | 决策 / 阶段 |
|---|---|---|---|---|
| 请求投影（图片 → 工具 → 历史文本逐级卸载） | kernel | 已落地 | 三家同构 | P3 |
| 终止式压缩（压缩即结束本轮） | kernel | 已落地 | Codex `compact.rs` | P3 |
| 阶梯 ② 摘要压缩（续跑式） | agent | 未实现 | OpenHuman `ContextCompressionMiddleware` | P4 之后单独提交（D13） |
| 阶梯 ③ 微压缩（清空旧工具结果正文） | agent | 未实现 | OpenHuman `MicrocompactMiddleware` + token 预算门 | P4 |
| 阶梯 ④ 收尾调用还原（受 allowance 约束） | agent | 未实现 | OpenHuman `FinalCallWrapUpMiddleware` | P4（D23） |
| 阶梯 ⑤ 确定性裁剪（按比例预算、系统消息不丢） | agent | 未实现 | OpenHuman `ImageAwareMessageTrimMiddleware` | P4 |
| 三维预算（迭代 / 工具次数 / 墙钟） | kernel | 已落地 | OpenHuman 同时设三类上限 | P3（D18） |
| 成本（USD）预算 | 宿主端口 | 未实现 | OpenHuman `BudgetGate` 端口 + `CostBudgetMiddleware` | P8+ |

### 2.4 观测与恢复

| 能力 | 归属 | 现状 | 参考 | 决策 / 阶段 |
|---|---|---|---|---|
| 轮级用量汇总（迭代 / 调用 / token / 工具数） | kernel | 已落地（未接线；工具数为 0 待 P4 填） | OpenBitFun `TurnStats` / `TurnTokenUsage` | P3 C8 |
| TTFT | kernel | 未实现 | OpenBitFun 首 token 计时 | P3 C8 |
| 事件 seq 与终态去重 | kernel | 已落地（未接线） | OpenBitFun `event_router.rs` / `event_queue.rs` | P3 C6 |
| 崩溃后「停在哪个 turn」可判定 | storage | 未实现 | DSH turn enclosure（§3.3） | P3 C7 |
| 重启续跑 / 后台常驻 | — | 不做 | DSH goal / schedule；OpenHuman `reaper.rs` | P9，单独 ADR |

### 2.5 演进能力（P8+）

| 能力 | 归属 | 现状 | 参考 | 决策 / 阶段 |
|---|---|---|---|---|
| 子代理（一次性委派、深度上限、只读工具集） | agent | 未实现 | Goose `SubagentOperation`、Codex control plane | P8.1 |
| 可继续子会话 / 任务板 / steer | 新层 | 未实现 | DSH continuable subagent、OpenBitFun `subagentProjection` | P8.2 |
| 会话之上 Router | 新层 | 未实现 | DSH Super-agent Router / Worker | P8.3 |
| 排队输入 / 中途插话 | kernel | 不做 | DSH `inject` + steering FIFO | P8+，需先解决「注入消息是否落库」 |
| 长期记忆 / RAG | 新层 | 不做 | OpenHuman `experience` / `learning` | 无规格，暂不立项 |

> 委派预留（`agent_id` / `parent_turn_id` / `delegation_depth` + 取消树）已在 I36 D16–D18 定稿，P4 只留字段不实现；禁止在 RoundExecutor 内写 `if is_subagent`（roadmap R10）。

---

## 3 参考实现对照

五个项目在同一组问题上的解法差异，正好覆盖内核的每个关注点。下文只提取**机制**，不评价产品功能。

### 3.1 Goose：Operation + StateMachine（事件溯源式循环）

`references/1-ai-agent/goose/crates/goose-agent/src/` 把整个 turn 表达为**一组可组合的 Operation**，`StateMachine` 逐个尝试，命中即停：

- `operation.rs`：`Operation::run` 返回 `NotApplicable` / `Applied(StepResult)`；`Applied` 携带**效果列表**（`AppendMessage` / `ReplaceConversation` / `PatchToolRequestMeta` / `SetMessageVisibility`），由 `EffectHandler` 统一施加。内核不改状态，只产出「状态变更意图」。
- `machine.rs`：每轮循环**重新加载会话**（`runtime.load(session_id)`）再跑一步，因此**落库会话就是唯一真源**，进程内没有第二份可变状态。
- **幂等重放**：`Operation::set_message_meta` 把「这个操作做过什么」写进消息元数据（如 `stop_hook/denied`、`foreground_subagent/delivered`），从持久化会话重建管线时结论一致。这是崩溃恢复能对齐的关键。
- `stop_hook.rs`：`StopHookDecision::{Allow, Deny}`；`Deny` 回灌一条用户可见性为 false 的纠正消息，并有 `deny_limit`（默认 8）防死循环。
- `unhandled_tool_calls.rs`：本轮结束时若仍有未响应的工具调用，除 `AwaitingUser` 外一律回灌错误响应，避免留下悬空调用。
- `subagent.rs`：子代理也是 Operation，通过元数据里的 `delivered` / `cancelled` 标记判断哪些已经跑过，重放不会重复启动。
- `tool_call_state.rs`：工具调用状态由**会话内容推导**（`Responded / Unchecked / Executable / Blocked / AwaitingUser / UserAnswered`），不是内存里另存一份。

对本项目的意义：我们的 `TurnOutcome` 是「结果」，Goose 的 effect 列表是「变更意图」。后者更适合跨宿主复用与重放，但改动面更大；P3/P4 保持结果式，**只把「重放一致性」作为验收要求**（同一 turn 重跑不产生重复副作用）。

### 3.2 OpenHuman：harness + middleware 栈（顺序即语义）

`references/1-ai-agent/openhuman/crates/openhuman-core/src/agent/tinyagents/` 的循环体交给通用 harness，产品语义全部落在中间件：

- `harness_assembly.rs`：`push_middleware` 的**注册顺序就是 `before_model` / `after_tool` 的执行顺序**，每条注释都写明「为什么必须在这个位置」。
- `harness_context_ladder.rs`：阶梯顺序固定为 **压缩 → 微压缩 → 收尾还原 → 确定性裁剪**。注释记录了一次真实事故：早期顺序是「微压缩 → 压缩」，摘要器拿到的正文已被清空，摘要里永远缺关键结果。这条顺序教训直接对应我们 I38b D23 / D24。
- `middleware/loop_guards.rs`：区分**确定性失败**与**可恢复失败**（超时、限流、5xx），后者给更大重试余量（同参 8 次 / 连续 12 次），并对轮询类工具（`wait_subagent`）豁免「同参重复」判定。
- `middleware/repeated_failure.rs`：`Continue / Nudge / Halt` 三级，nudge 只作用于**下一次请求**，不进持久历史（否则重放时变成陈旧指令）；halt 用根因摘要覆盖最终文本。
- `middleware/cost_budget.rs` + `host/budget_gate.rs`：**预算能力是宿主端口**，不是运行时内部字段。注释明确写了「runtime 被做成 host 无关之后，不能再直接访问 `cron::scheduler_gate` / `cost`」，于是抽 `BudgetGate`（准入、定价提示、压缩建议三件事）。
- `middleware/tool_output.rs` / `tool_result_artifacts/`：工具结果在**进入 transcript 之前**做字节上限、语义摘要与落盘，而不是等上下文超限再补救。

对本项目的意义：阶梯顺序与额度**已在 I36 spec Step 4.2 / §4.3 写明**（「顺序是语义」、门控公式），OpenHuman 的增量只有两条：顺序错了会**静默降质**（摘要缺关键结果，测试很难发现），所以顺序需要**锁定它的测试**而非只写文档；`after_tool` 的「豁免 → 分类预算 → 通用阶梯」同理（I38b D26 → D25 → D19，已有文字，无测试）。

### 3.3 DeepSeek Harness：能力接缝 + 不变量

`references/1-ai-agent/deepseek-harness-orb/.agents/notes/archived/architecture/` 的设计笔记，几乎每条都对应我们内核的一条不变式：

- `2026-06-17-filesystem-capability-seam.md`：能力按**三包**拆——接口包（`dsh-fs`）、实现包（`dsh-fs-local`）、消费包（`dsh-tool-fs`）；**消费包只依赖接口包，永不依赖实现包**。文件访问策略、路径解析、原子写属于实现；工具 schema 与结果格式属于消费。并明确写明：**接缝本身不蕴含权限与沙箱**，containment 由独立策略包在 `tools/execute` 前否决。这条直接印证我们「Policy 不在内核、在 Pipeline」的取舍，也提示需要一条「不得绕过 Pipeline」的不变式。
- `2026-06-15-turn-enclosure-invariant.md`：**每个会话事件都必须落在一个 turn 内**（`turn/start` 与 `turn/end` 之间），因为 turn 是崩溃恢复边界。排队消息改为在 `turn/start` 之后写入；空闲期注入包成一次性 turn。规则由**生产方**强制并加 dev 断言，而不是让读取方容忍。
- `2026-07-06-tool-result-retention-library.md`：把「保留了什么、省略了什么」抽成纯库（`ItemRetainer` / `TextRetainer`），省略数量必须**精确**（`none` / `exact` / `unknown`），且不掺入工具语义与提示词文案。工具各自保留分组、行号、退出码、溢出文件。
- `2026-06-18-agent-lifecycle-and-ownership-contracts.md`：`AgentHandle = { agent, dispose }`，**销毁是消费方能力**；`cancel()` 是唯一公开停止原语；后台任务带 `owner` 令牌，归属存在执行器里，HMR 重载不丢。

对本项目的意义：这三条（接缝不蕴含权限、事件封轮、保留量可精确说明）都应写进内核门禁，而不是仅靠 review。

### 3.4 Codex：控制面与执行面分离

`references/1-ai-agent/codex/codex-rs/core/src/agent/control/` 把「多 agent 的生命周期」抽成控制面：

- `api.rs`：`AgentControl` 暴露 `resolve / spawn / send / interrupt / resume / ...`；`spawn` 的输入必须是「启动子 turn」的消息模式，不允许静默排队。
- `interrupt.rs`：**根 agent 不可被中断，agent 不可中断自己**；未加载或已死的运行时视为中断成功，且**绝不为了中断而重新加载**。
- `budget.rs`：预算记账与「提醒是否已投递」分离，提醒只有在进入历史之后才确认投递——避免提醒在重放时重复出现。
- `residency.rs` / `mailbox.rs` / `spawn_guard.rs`：常驻、信箱、并发派生护栏各自独立成模块。

对本项目的意义：P8.1 的 `spawn_agent` 应照此把**控制面**与**执行面**分开——`spawn` 只是「发起一个子 turn」，不直接操作内核状态；`interrupt` 的边界（不能中断根、不能中断自己）应写进规格。

### 3.5 OpenBitFun：turn 统计与事件路由

`references/1-ai-agent/OpenBitFun/src/crates/execution/agent-runtime/src/` 把 turn 作为一等事实：

- `dialog_turn.rs`：`TurnStats`（轮数 / 工具数 / token / 耗时）与 `TurnTokenUsage`（按 `turn_id` 聚合，忽略其它 turn 的用量事件）都是**provider 中立**的值类型，不绑定会话存储。
- `turn_cancellation.rs`：`DialogTurnCancellationTokenStore` 按 turn id 复用同一令牌（`get_or_insert_new`），取消即 `cancel()`，不 `abort`。
- `session_event_journal.rs` / `checkpoint.rs` / `event_router.rs` / `event_queue.rs`：事件有日志、检查点与路由，UI 只是订阅方之一。
- `permission.rs` / `native_hooks/` / `skills/`：权限、钩子、技能各自独立，不侵入循环体。

对本项目的意义：我们的 `TurnOutcome` 十一字段已覆盖 `TurnStats` 的主体，但**按 turn 聚合用量并忽略跨 turn 事件**这一点应成为事件端口的显式契约（当前 `seq` 已保证，但缺少「跨 turn 归零」的测试用例，见 I36 C6.9）。

---

## 4 与本项目内核的差异

### 4.1 已对齐

| 项 | 证据 |
|---|---|
| 参数化而非分叉 | `TurnSpec::chat()` / `TurnSpec::agent()` 只差字段值；`kernel/` 内无 `is_agent` 扫描用例 |
| 端口化 | `kernel/ports.rs` 四端口；`kernel/` 无 tauri 依赖 |
| 取消语义 | 只置位令牌；代际守卫覆盖事件 / 落库 / 状态迁移三写侧 |
| 预算维度 | 迭代 / 工具次数 / 墙钟三维独立 |
| 工具执行语义 | `partition_tool_batches` 并行分批、未注册 fail-closed、结果按索引回填 |
| 权限取最严 | `most_restrictive` + 审批键规范化 + 授权两表 |

### 4.2 P3 收口（代码已在，接线未完成）

| 缺口 | 现象 | 收口项 |
|---|---|---|
| chat 未迁移 | `src-tauri/src/cmd/chat` 仍走 `chat_runtime.rs`；`src-tauri/src` 内无 `TurnSpec::` 引用 | C5.25–C5.35 |
| 宿主命令缺失 | 无 `src-tauri/src/cmd/agent/`、无 `src-tauri/src/adapters/` | C5.21–C5.24 |
| 事件 DTO 未加 | `phase` / `session-state-changed` 未进 wire | C6 |
| 终态列缺失 | `messages` 无 `status`；启动清扫未接 | C7 |
| 观测缺失 | 无 `ttft_ms`、无轮级汇总日志 | C8 |

**本次核对已修正的文档事实**（均在 `docs/store`，不改设计）：

| 文档 | 原状态 | 修正为 |
|---|---|---|
| I36 spec 实施状态总览 | 端口 / `TurnSpec` / `tasks` / `stream` / `request` / `persist` / `emit` / `state` / `limits` / `runner` 全标「未实现」；`limits` 标「双层墙钟尚未接线」 | 均标「已实现」；双层墙钟已在 `runner.rs` / `stream.rs` 接线 |
| I36 spec §0B.2 | `TurnSpec` 七字段，无偏差记录（tasklist C5.13 引用的「§0B.2 偏差记录」不存在） | 补 `kind` / `limits` 两字段，新增 §0B.2.1 偏差记录 |
| I36 spec §3.1 | `TurnRunner` 列 10 字段（含 `limits` / `middlewares` / `stop_hooks`） | 按实现列 8 字段，说明 `limits` 已进 `TurnSpec`、其余随 `agent/` 落地 |
| I36 tasklist C5.14 | 落点写 `agent/mod.rs` | 实际落点 `kernel/mod.rs`；P4 再迁 `agent/round_executor.rs` |
| I38c §1.2 | 「RoundExecutor（P4 已完成）」 | 「P4，未开始，规格见 I38b」 |

### 4.3 P4 开工前必须统一的接口

P3 用了占位 trait，I38b 另起了一套签名；两边不一致会导致 P4 返工。

| 符号 | 代码现状 | I38b 要求 | 建议落点 |
|---|---|---|---|
| `RoundExecutor` | `kernel/mod.rs`，`run_iterations(&self, ctx, iteration)` | `agent/round_executor.rs`，携带迭代状态与工具批次 | `agent/`，内核只留 trait 引用 |
| `RoundOutcome` | 两字段（`tool_calls` / `executed`） | 多字段（含 `confirmation_wait_ms`） | 与 I38b §2.3 对齐后一次定型 |
| `ToolCatalog` | 空同步 trait | `async` + `agent_tools()` | `kernel/ports.rs`，改 async（D16） |
| `StopHook` | 只有 `name()` | 有 `check`，同步且禁止 IO | `agent/`，语义归 executor |

文档口径已在 §4.2 末表统一：C5.14 的落点改为 `kernel/mod.rs`（并注明 P4 迁至 `agent/round_executor.rs`），I38c §1.2 改为「P4 未开始」。四个符号的签名差异**仍待 P4 开工时一次定型**，属实现决策，本次不动。

### 4.4 推迟与不做

| 能力 | 结论 | 理由 |
|---|---|---|
| 子代理 / 多 agent | P8.1 起 | 单 turn 闭环不需要；预留字段已够 |
| 沙箱隔离 | P8+ | `ToolTier::Sandbox` 已定义但零实现；P5 的 `shell_exec` 靠审批兜底 |
| 成本预算 | P8+ | 需要定价目录与账本，属宿主能力 |
| 排队输入 / steering | P8+ | 先要定「注入消息是否落库、落哪个 turn」 |
| 重启续跑 / 后台常驻 | P9 | 跨重启状态，单独 ADR |

---

## 5 扩展点：新增能力怎么不改内核

| 想加的能力 | 走哪个扩展点 | 不需要改 |
|---|---|---|
| 新的上下文降级步骤 | 阶梯插入（`TurnSpec.ladder` + agent 中间件） | 循环体、事件格式 |
| 新的刹车条件 | `StopHook`（同步、无 IO） | 状态机、落库 |
| 新的预算维度 | `TurnLimits` 加字段 + `TurnSpec` 传值 | `TurnRunner` 主流程 |
| 新的工具 | `ToolCatalog` + Pipeline 注册 | 内核、事件 |
| 新的外部工具来源（MCP） | `ToolCatalog` 实现 + 安全标记推断 | 内核、Policy 逻辑 |
| 新的宿主（CLI / headless） | 实现四端口 | 内核全部代码 |
| 子代理 | 作为工具接入 Pipeline + 取消树 | `TurnRunner` 接口、事件格式 |

反过来，以下改动**必须**动内核，遇到时应先改本文与 I38 roadmap 的阶段表，而不是直接改代码：

- 增加新的 turn 终态（`TurnStatus`）。
- 增加新的 phase。
- 让事件携带新的必填字段。
- 改变「工具执行唯一入口」这条不变式。

---

## 6 内核门禁（可机检）

| 编号 | 检查 | 方式 |
|---|---|---|
| G1 | `kernel/` 无 tauri / `AppHandle` / `Window` / `tauri::State`，不 `use` `src-tauri` 类型 | `cargo tree -p assistant-agent` + grep |
| G2 | `kernel/` 无 `is_agent` / `is_chat` 分支 | 文件扫描用例（已存在） |
| G3 | `TurnSpec::chat()` / `agent()` 逐字段断言 | 单测（已存在） |
| G4 | 仅用端口替身可跑完整轮 | 集成测试 |
| G5 | 工具执行只经 Pipeline（无裸 `Tool::execute` 调用点） | grep 调用点；当前无可见性强制，属 review 门禁，P4 可改为 `pub(crate)` 收口 |
| G6 | 每条 turn 事件带 `turn_id`，同 turn `seq` 单调、跨 turn 归零 | 集成测试 |
| G7 | 同一 turn 重跑不产生重复副作用（幂等） | 重放用例（P4 起） |
| G8 | 工具结果被裁剪 / 省略时，省略量可精确说明 | 结果契约测试（P4 起） |

G7 / G8 取自 Goose 的 operation notes 与 DSH 的 retention library；它们不是当前实现的一部分，但**接口形状必须现在就容得下**（例如结果里保留 `omitted` 语义位），否则以后加要动 DTO 与落库格式。

---

## 7 借鉴优先级

| 优先级 | 借鉴项 | 来源 | 落地时机 |
|---|---|---|---|
| P0 | 事件封轮（事件不得落在 turn 之外） | DSH | P3 C6/C7 |
| P0 | 未处理工具调用 fail-closed 回灌 | Goose | P4 |
| P0 | 单一公开停止原语 + 不可中断根 | DSH / Codex | P3 已对齐，P8 复用 |
| P1 | 阶梯顺序不变式（压缩先于微压缩） | OpenHuman | P4 |
| P1 | 可恢复失败单独余量 + 轮询工具豁免 | OpenHuman | P4 |
| P1 | 结果保留量精确说明 | DSH | P4 契约预留 |
| P1 | 预算能力做成宿主端口 | OpenHuman `BudgetGate` | P8 |
| P2 | 子代理表达为 Operation / 控制面 | Goose / Codex | P8.1 |
| P2 | 能力三包接缝（接口 / 实现 / 消费） | DSH | P8，若引入沙箱后端 |

---

## 8 维护规则

- 能力增删只改 §2 矩阵与 §4.4，不改 §3 对照（对照是历史事实）。
- 新增阶段或改变门禁，先改 [I38 roadmap](./I38-agent-roadmap.md) §2 / §6，再改本文。
- 本文出现的所有仓库路径一律相对路径；不写本机绝对路径。

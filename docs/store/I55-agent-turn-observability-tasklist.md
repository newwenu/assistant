# I55 — Agent Turn 观测面 任务清单

> 规格：[I55-agent-turn-observability-spec.md](./I55-agent-turn-observability-spec.md)（节点表 §3.1 / 事件词汇 §4 / 状态机 §5 / 投影 §6 / 门禁 §9 / 批次 §10 / 编排器前瞻 §11）
> 事实基线：I55 §1.1 的 N1–N18（已按代码核对，见下方「前置核对」）
> 前置：P4（`TurnObserver` / `TurnEmitter` / `RecordingSink` 已就绪）、P6（`transport` / `turnOrchestrator` 已接线）均已完成；P15 当前为 **I38 §2.1 未立项候选**，本清单的 V0 是唯一的开工闸门
> 阶段台账：[I38-agent-roadmap.md](./I38-agent-roadmap.md) §2 阶段表（唯一状态来源）

批次对应规格章节：**V0 立项前置**（I38 §7 第 5 条）→ **V1 出声**（I55 §4 / §5 / §9 G1·G3·G5·G9·G12写侧 / §10）→ **V2 投影与渲染**（I55 §6 / §6.7 / §9 G2·G4·G6·G7·G8·G10·G12读侧·G13·G14）→ **V3 回放**（I55 §7，受闸）→ **V4 多实例与并发**（I55 §11.4 第 4 条，不在本清单）。

依赖：V1 不依赖任何在途阶段（只加方法与事件，不改既有事件语义，I55 O8）；V2 依赖 V1 产出的 wire 契约（`npm run contract:gen` 之后才有前端类型）；V3 依赖 V0 立项 + V1/V2 全部完成 + 保留策略有真实事件量数据；V4 依赖编排层阶段（未立项），本清单不预留也不阻止。

> 本清单不管理提交。提交粒度按代码依赖与可编译性现场决定；这里只回答「代码改没改、验收跑没跑」。
> 勾选表示代码已改且对应验收已跑。

---

## 前置核对（2026-11-18，按代码）

| 事实 | 结论 | 落点 |
|---|---|---|
| `TurnObserver` 现有 4 方法（`set_phase` / `set_iteration` / `emit_tool_event` / `delta`） | 只加 1 个方法，既有实现零改动 | `src-tauri/crates/assistant-agent/src/agent/round_executor.rs` |
| `TurnEvent` 现有 8 变体，`#[serde(tag = "kind")]` | 加 `Node` 变体不改变既有 tag 形状 | `src-tauri/crates/assistant-agent/src/kernel/ports/mod.rs` |
| `TurnEmitter::new` 有 3 处调用点 | 加两个形参需同批改 3 处 | `kernel/turn/runner.rs`、`src-tauri/src/cmd/compaction.rs`、`tests/kernel_emit.rs` |
| `RecordingSink` 已有 `failing_after(n)` | G5 无需新造替身 | `src-tauri/crates/assistant-agent/src/kernel/ports/test_doubles.rs` |
| `ScriptedToolExec` 已存在 | G1 的带工具 turn 可直接装配 | 同上 |
| `PipelineEventSink` 端口与适配器已存在，只转发三态 | 加 `StageEntered` 后适配器需分流到节点事件 | `src-tauri/crates/assistant-tools/src/runtime/pipeline.rs`、`src-tauri/src/adapters/tool_exec.rs` |
| `transport.ts` 对 `agent-tool-call` 按活跃会话过滤 | V2 唯一需要**修改既有行为**的一处（G6） | `src/services/stream/transport.ts` |
| `TurnRunCtx` 已带 `agent_id` / `parent_turn_id` / `delegation_depth` | Q6 的「V1 就带键」零成本 | `agent/round_executor.rs`、`kernel/turn/runner.rs` |

---

## V0 — 立项前置（不改代码）

唯一闸门。I38 §7 第 5 条：新增阶段必须先改 §2/§3，再写 spec；本文与 I55 spec 已留档，但 P15 仍在候选表内，**开工前必须完成本节**。

- [x] `docs/store/I38-agent-roadmap.md` §2：把 P15 行从 §2.1 候选表移入 §2 阶段表，状态列写「阶段名 + 一句话结论 + 日期」（§7 第 1 条），不写任务明细
- [x] `docs/store/I38-agent-roadmap.md` §2.1：首段「P15 规格已留档但**未立项**」改为「P15 已立项」；候选表删除 P15 行；其余候选与五轴不互相前置的表述保持不变
- [x] `docs/store/I38-agent-roadmap.md` §3 依赖行（`P4 + P6 → P15`）与 §4 文档地图行已存在，**只核对不改**
- [x] 冻结命名：本阶段两个模板名固定为「turn 模板」（常量，12 节点）与「工作流模板」（数据，独立立项，I55 Q7）；后续任务一律不得混用（落点：I55 §3.3 / §11.4 第 3 条 / Q7）
- [x] 冻结 node id 拼写：以 I55 §3.1 表为准，wire 序列化为 `kebab-case`（`persist-assistant` / `budget-split` / `after-tool` / `after-iteration`），前端模板常量同形（G2 的相等断言以此为准；落点：I55 §3.1 / §4.2）

> V0 未完成时 V1 不得开工；V0 不改任何代码，也不改 I55 spec 正文。

---

## V1 — 出声（后端，只加不改）

V1 的退出条件：G1 / G3 / G5 / G9 / G12（写侧）通过，`cargo test -p assistant-agent` 全绿，clippy / fmt 通过，`contract:gen` 幂等。此时前端可以不渲染（I55 §10 末条）。

### V1-a 词汇与端口

- [x] 新增 `src-tauri/crates/assistant-agent/src/agent/node_event.rs`：`AgentNode`（12 变体，`#[serde(rename_all = "kebab-case")]`）、`NodeState`（7 值闭集，`lowercase`）、`AgentNodeEvent`（I55 §4.2 全形）、`AgentNodeDetail`（9 变体，`#[serde(tag = "kind")]`）及 detail 结构（`BudgetRemaining` / `MiddlewareVerdict`）
- [x] `BudgetRemaining` 全字段 `Option`，`None` = 「未配置」而非 0（I55 §4.4 末段；`token-budget` 未配置时钩子不安装）
- [x] `PipelineStage` 沿用既有类型：`assistant-tools::PipelineStage`（5 值）与 `agent::round_executor::ToolStage` 已由 `src-tauri/src/adapters/tool_exec.rs::into_stage` 一一镜像；`agent/node_event.rs` 的 `PipelineDetail.stage` 直接引用 `ToolStage`，**不新造枚举、不新造第二份镜像**（I55 §4.4）
- [x] `AgentNodeEvent.transition: bool` 为**必填非 `Option`**：`true` 只用于 §5.1 迁移表的合法迁移，`false` 只用于 `StageEntered` 的过程事实（I55 §4.2 / §4.6 第 5 条）；禁止让 `state` 兼职表达进度（那会把 `pipeline` 的 `running` 打成 5N 条并重置 `sinceMs`）
- [x] `src-tauri/crates/assistant-agent/src/agent/mod.rs`：注册 `pub mod node_event;`
- [x] `src-tauri/crates/assistant-agent/src/kernel/ports/mod.rs`：`TurnEvent` 增 `Node(AgentNodeEvent)` 变体；保持既有 `#[serde(rename_all = "camelCase", tag = "kind")]`，不新增 tag 层
- [x] `src-tauri/crates/assistant-agent/src/agent/round_executor.rs`：`TurnObserver` 增 `fn node(&self, node: AgentNode, iteration: u32, state: NodeState, transition: bool, detail: Option<AgentNodeDetail>)`；方法体不得有返回值（G11 的同口径）；`transition` 显式传参而非由实现方推断，保证写侧语义只有一处来源；给默认空实现（测试替身与非广播链路不为「不观测」强加噪声）
- [x] `src-tauri/crates/assistant-agent/src/kernel/turn/emit.rs`：`TurnEmitter::new` 增 `agent_id` / `parent_turn_id` 两形参（当前恒 `"root"` / `None`）；新增 `node()` 走既有 `dispatch`（复用 `seq` 计数器与代际校验，I55 O1/O3）；`iteration` 由调用方显式传入，不复用 `current_iteration()`（迭代外节点恒 0）
- [x] `src-tauri/src/cmd/compaction.rs`、`src-tauri/crates/assistant-agent/tests/kernel_emit.rs`：`TurnEmitter::new` 调用点补 `"root"` / `None`（压缩 turn 无节点事件，行为不变）
- [x] `src-tauri/crates/assistant-agent/src/kernel/turn/runner.rs`：`RunnerObserver` 增 `clock: &'a dyn crate::kernel::Clock` / `turn_started_at: std::time::Instant` 字段（取自 `run()` 作用域已有的 `self.clock` 与 `started_at`，不新增端口）；`agent_id` / `parent_turn_id` **只在 `TurnEmitter` 保存一份**（观测器只负责路由，不复制写侧身份，避免两处真源）；`node()` 实现：`at_ms` 由 `(clock.now() - turn_started_at)` 计算（禁止 `SystemTime::now()`，I55 §4.2），且只在 `EventRoute::Broadcast` 时发射（与 `set_phase` 同口径，orb 链路事件序列不变）

### V1-b 12 处出声点

每个节点都遵守：`running` 每条每迭代一条；`finished` / `error` 每条每迭代**恰好一条**（O2 写侧）；`pending` 与 `skipped` **不由后端发**（I55 §4.6 第 1–2 条）。

- [x] ① `admit`（`runner.rs` `run()` 内，`TurnEmitter` 创建之后）：`running` + `finished` 补记，`iteration = 0`、`at_ms = 0`。理由：`TurnTasks::begin` 在宿主侧先于 sink 可见（`begin` 无 sink 参数），此处是唯一可发点；准入失败（`Busy`）**不发声**（I55 Q1），走既有命令错误通道
- [x] ② `prepare`（`agent/loop_executor/mod.rs` `run()` 的 `request::build` 处）：`running` 前置，`Ok` → `finished`、`Err` → `error`，`iteration = 0`、`detail = None`
- [x] ③ `brake`（`mod.rs` 的 `state.brake(...)` 前后）：`running` → `finished`（`verdict: stop` 仍是 `finished`，不是 `error`，I55 §5.1 末条）；`detail` 取 `BrakeDetail`，`hook` 为命中钩子名（`None` 表示继续），`budget` 由 `ctx.limits` / `state.tool_calls` / `ctx.remaining_turn_budget` 组装
- [x] ④ `ladder`（`mod.rs` 的 `chain.before_model(...)` 前后）：`running` → `finished` | `error`；`applied` 用**前后快照比较**得出（`messages.len()` / `context_window_tokens` / `max_tokens` 三项任一变化即记入），不新增清单常量、不改 `TurnMiddleware` trait（I55 Q2：只报实际改写请求的项）
- [x] ⑤ `model`（`agent/loop_executor/call.rs` 起点发 `running`；重试分支发 `waiting`（带 `attempt` / `retryReason`）后回到 `running`；`mod.rs` 在 `accumulator.finalize_all` 之后发 `finished`）：`attempt = retries_used + 1`，`finishReason` 取 `outcome.finish_reason`，`toolCallCount` 取 `calls.len()`；`error` / `cancelled` 由 `stream_failure` / `cancelled_before_call` 分支发
- [x] ⑥ `persist-assistant`（发在 `persist.rs` 的两个写入点 `persist_row` / `persist_row_as_new` 内，**不**在 9 处 `persist_final_text` / `persist_row` 调用点逐个包裹）：`RoundState` 增 `persist_running_emitted: bool`（随 `reset_iteration_flags` 复位），首次写入前发 `running`（每次迭代仅一条）；每次写入成功发 `finished`、失败发 `error`，`rows = 1`、`messageIds = [msg_id]`。**同一迭代可有多条 `finished`**（行已 `finalize` 后只能新建行：收尾 / 取消 / 预算中止路径都会二次落库），这是 I55 O2 的唯一例外，见 spec §4.6 第 4 条
- [x] ⑦ `branch`（`mod.rs` 的 `finalize_all` 之后）：`running` → `finished`，`detail` 为 `BranchDetail`（`hasToolCalls` / `wrapUp` / `toolCallCount`）；无工具时 `finished` 之后立即出口 `completed`，前端据此把 ⑧–⑫ 批量标 `skipped`
- [x] ⑧ `budget-split`（`execute.rs` `execute_tools` 内的 `split_by_budget(...)` 调用前后；函数体在 `budget.rs`）：`running` → `finished`，`detail` 为 `BudgetSplitDetail`（`requested` / `executed` / `deferred`，取自 `calls.len()` / `to_run.len()` / `skipped.len()`）
- [x] ⑨ `pipeline`（`execute.rs` `execute_tools`）：`running` 在进入执行前发（`transition = true`、`detail = None`，每迭代**仅一条**）、`finished` 在结果收齐后发；执行失败走 `error`、取消走 `cancelled`（`terminal.rs` 的 `cancelled_tool_exec` 一处）；三个终态的 `detail = None`（`stage` / `index` / `total` 只随 V1-c 的 `StageEntered` 以 `transition = false` 出现，终态不携带子阶段信息，I55 §4.4）
- [x] ⑩ `after-tool`（`execute.rs` 逐结果的 `chain.after_tool(...)` 前后）：首个结果前 `running`、全部结果后 `finished`，`detail` 的 `verdicts` 按结果聚合
- [x] ⑪ `persist-tools`（`agent/loop_executor/persist.rs` `append_tool_results` 改为返回 `Result<Vec<String>, StoreError>`，携带落库行 id）：调用点 3 处同步（`mod.rs` 一处、`terminal.rs` 两处：`cancelled_tool_exec` / `wrap_up_on_last_iteration`），`running` → `finished` | `error`，`rows = results.len()`、`messageIds` 取返回的 id 集；空结果批次（`wrap_up` 路径）仍发 `running` + `finished`（`rows = 0`），避免留下悬空格子
- [x] ⑫ `after-iteration`（`mod.rs` 的 nudge 收集 + `chain.after_iteration(...)` 之后）：`running` → `finished`，`detail` 为 `AfterIterationDetail`（`nudges` / `nextIteration = it + 1`）
- [x] `ladder` / `after-tool` 的 verdict 采集：`MiddlewareChain` 增返回值形态（`before_model` 返回 `(MiddlewareOutcome, Vec<&str>)`、`after_tool` 返回 `(MiddlewareOutcome, Vec<MiddlewareVerdict>)`），**不改** `TurnMiddleware` trait 与 `P4_MIDDLEWARE_ORDER`
- [x] 终态路径不遗漏：`stopped` / `halted_by_budget` / `wrap_up` / `cancelled` / `failed` 五个出口都要有对应节点已发终态（`wrap_up` 路径的节点顺序是 ⑪→⑤→⑥，与模板顺序不同，图投影按事件顺序着色，见 V2-b）

### V1-c 工具流水线的阶段进入事件

- [x] `src-tauri/crates/assistant-tools/src/runtime/pipeline.rs`：`PipelineEvent` 增 `StageEntered { tool_call_id, tool_name, stage: PipelineStage, index: u32, total: u32 }`；既有三态变体字段不变（O8）
- [x] `run_call` 增 `sink: &dyn PipelineEventSink` 与 `(index, total)` 形参，在 4 个阶段边界（`Find` / `Validate` / `Gateway` / `Execute`）各发一条 `StageEntered`
- [x] `execute` 的落盘循环在 `self.persist(...)` 前发 `Persist` 阶段的 `StageEntered`；`index` 用**全局序号**（按发起顺序），`total` 为本次 `tool_calls.len()`（I55 Q3）
- [x] `InMemoryEventSink` 无需改动（`events()` 天然录制新变体）
- [x] `src-tauri/src/adapters/tool_exec.rs`：`ObserverPipelineSink::emit` 按变体分流——三态仍转 `ToolEvent`（既有路径不变），`StageEntered` 调 `observer.node(pipeline, running, transition = false, PipelineDetail 过程形态)`，**不**造 `ToolEvent`（I55 §4.7：`agent-tool-call` 不扩展）；**不得**用 `transition = true`（G12 写侧，否则 `pipeline` 格子会被 5N 条 `running` 反复重置 `sinceMs`）

### V1-d wire 与契约

- [x] `src-tauri/src/protocol/events.rs`：`event_name::AGENT_NODE_EVENT = "agent-node-event"`（事件名表 23 → 24，只加一族，I55 §4.7）
- [x] `src-tauri/src/protocol/dto/agent.rs`：新增 `AgentNodeEventDto`（I55 §4.2 全形，`camelCase`）、`AgentNodeDto`（`kebab-case`）、`NodeStateDto`、`AgentNodeDetailDto`（`#[serde(tag = "kind", rename_all = "camelCase")]`）、`PipelineStageDto`（5 值）、`BudgetRemainingDto`、`MiddlewareVerdictDto` 及各 detail DTO；`parentTurnId` **不加** `skip_serializing_if`（键恒在、值 `null`，前端声明 `string | null`）；`transition` 为必填 `boolean`（不包 `Option`、不加 `skip_serializing_if`，键恒在）
- [x] 输出 DTO 一律不加 `deny_unknown_fields`；`AgentNodeEventDto` 只做 wire 投影，内部类型（`AgentNode` / `NodeState`）不外泄到契约
- [x] `src-tauri/src/adapters/event_sink.rs`：`TurnEvent::Node(event)` 分支 → `emit_wire(event_name::AGENT_NODE_EVENT, AgentNodeEventDto)`；只记 `debug` 日志（节点事件密度高，禁止 `info` 刷屏）
- [x] `src-tauri/tests/protocol_bindings.rs`：`declarations` 数组登记全部新 DTO（漏登记测试直接失败，I38 §7 第 9 条）
- [x] `npm run contract:gen` 生成 `src/types/protocol.contract.ts`（生成物禁止手改）；`npm run contract:check` 通过

### V1-e 验收（G1 / G3 / G5 / G9 / G12 写侧）

- [x] 新增 `src-tauri/crates/assistant-agent/tests/node_events.rs`：G1 —— 跑一轮带工具的 turn（`ScriptedToolExec` + `RecordingSink`），断言 12 个 node id 各至少一条 `finished`；无工具路径断言 ⑦ 之后 ⑧–⑫ 无事件（`skipped` 由前端推导）
- [x] 同文件：G3 —— 混排 `Delta` 与 `Node`，断言 `seq` 严格递增、无重复、无回退
- [x] 同文件：G5 —— `RecordingSink::failing_after(1)` 下 turn 仍 `completed`（既有用例口径复用）
- [x] 同文件：G12（写侧）—— `StageEntered` 产出的节点事件 `transition == false`；`transition == true` 的 `pipeline` `running` 每个 `(node, iteration)` 恰好一条。并发批次下必须显式验（`pipeline.rs` 的 `join_all` 会并发跑多个调用，5N 条子阶段事实不得混入状态迁移）
- [x] 同文件：逐节点终态唯一性（O2 写侧）—— 除 `persist-assistant` 外，每个 `(node, iteration)` 的 `finished`/`error`/`cancelled` 合计恰好 1 条；`persist-assistant` 允许同迭代多条 `finished`（`rows` 累加等于该迭代实际落库行数），但不得出现 `finished` 之后再回 `running`
- [x] 同文件：收尾 / 取消 / 预算中止三条路径各跑一次（`wrap_up` 由 `TurnSpec` 开启、`cancelled_tool_exec` 由取消令牌触发、`halted_by_budget` 由 tool-call 上限触发），断言 ⑤ / ⑥ / ⑨ / ⑪ 的终态都在且无悬空 `running`
- [x] 既有测试零快照变更（`kernel_emit` / `loop_executor` / `kernel_runner` 全绿，G10 的后端半边）
- [x] 命令（cwd = `src-tauri`）：`cargo test -p assistant-agent`、`cargo test -p assistant-tools`、`cargo test -p assistant-app --test protocol_bindings`、`cargo clippy --workspace --all-targets`、`cargo fmt --check`

---

## V2 — 投影与渲染（前端）

V2 的退出条件：G2 / G4 / G6 / G7 / G8 / G10 / G12（读侧）/ G13 / G14 通过，`typecheck` / `lint` / `vitest` 全绿。

### V2-a 观测存储与 reducer

- [x] 新增 `src/stores/agent/observation/`：`store.ts`（`Map<observationKey, TurnObservation>`，键 = `${conversationId}:${turnId}`，I55 §6.1）、`types.ts`（`TurnObservation` / `NodeCell` / `NodeFact`）
- [x] `types.ts`：`NodeCell` 增 `source: "observed" | "derived" | "assumed"`（本地类型，**不进 wire**，I55 §6.6）；`NodeFact` 增 `transition: boolean`（透传自事件，供 inspector 区分「状态迁移」与「过程事实」）
- [x] `store.ts` 容量上限 32 个 `observationKey`，按「最近有事件」LRU 淘汰；淘汰只丢 `cells` / `timeline`，保留 `status` 摘要（I55 §6.3 第 3 条）
- [x] `store.ts`：单实例 `timeline` 上限 2000 条，超出按固定顺序丢弃（先最早 `transition: false` 的过程事实，再最早的非首条 `running`，**状态迁移终态永不丢**），并在时间线留一条「已省略 N 条」的合成事实（`source = derived`），禁止静默截断（I55 §6.7 第 5 条）
- [x] `reducer.ts`：`applyNodeEvent(obs, event)` —— `transition === false` 时**只写 `timeline`**，不改格子状态、不重置 `sinceMs`（G12 读侧，I55 §4.6 第 5 条）；`transition === true` 时才进入状态机
- [x] `reducer.ts`：终态不可覆盖（O2 / G4，状态变更尝试只记 `debug` 日志）；**唯一例外**：`persist-assistant` 的重复 `finished` 只累加 `rows` / `messageIds`，状态值不动（I55 O2 例外条）；未知 `node` / `state` 写入 `timeline` 但**不建格子**（G8，I55 §4.8 第 3 条）
- [x] `reducer.ts`：**无变化必返原引用**（O11 / G14）——终态保护丢弃的事件、未知取值、`persist-assistant` 重复 `finished` 中不改变 `rows` 的部分，一律 `return obs` 而非 `return { ...obs }`；判据是 `cells` / `timeline` 是否真的变了，不是「是否进入了分支」（I55 §6.7 第 2 条）
- [x] `reducer.ts`：`source` 标注——真实事件写入即置 `observed`；模板推导 `pending`、⑦ `BranchDetail` 推 `skipped`、`max(iteration)` 推圈数置 `derived`；恢复点亮置 `assumed`；**只升不降**，一旦 `observed` 不再回退（G13，I55 §6.6 第 2 条）
- [x] `reducer.ts`：`applyDeltaSeq(obs, seq, iteration)` —— 只记 `lastSeq` 与「模型出字」时间轴，不触碰任何格子状态（I55 §4.7）
- [x] `reducer.ts`：`applyToolCall(obs, call)` —— 由 `agent-tool-call` 的 `AwaitingApproval` / `AwaitingAnswer` 把 `pipeline` 置 `waiting`，`ApprovalClosed` 回 `running`（I55 §5.2 第 1–2 条；**禁止**新增事件，O5）；这类 `waiting` 属 `observed`（有真实载体），不是 `derived`
- [x] `template.ts`：`AGENT_NODE_TEMPLATE` 12 节点常量（id / 行 / 列 / `inIteration`），与 I55 §3.1 表 1:1；模板**不进 wire**、无运行时注册表（O4）
- [x] `template.ts` 单测：模板 id 集合与契约生成类型的 key 集合相等（G2）
- [x] `nodeFromPhase.ts`：`TurnPhase` → node 的恢复映射，**只允许这一个函数**承载，且注释写「仅恢复用，正常渲染以节点事件为准」（I55 §6.4）；点亮的格子 `source = assumed`
- [x] `restore.ts`：窗口重载 / 切会话补拉——复用 `onPendingToolCalls` 的结论（有挂起调用即 `pipeline` = `waiting`）+ `session-state-changed` 的 `phase` 点灯；**不得**新增「历史节点事件查询」命令（I55 §6.4 第 3 条）；补拉产生的所有格子 `source = assumed`，收到真实事件后升 `observed`（I55 §6.6 第 2 条）
- [x] 实例收尾规则：turn 终态（`session-state-changed` → `idle` / `error`）后实例 `status` 置终态；仍处 `running` / `waiting` 的格子**保持原状态值**，由图渲染器显示「未完成」徽标，**不臆造** `skipped` / `cancelled`（O2 / O5 的直接后果）；收尾不改 `source`（证据等级不因 turn 结束而变化）

### V2-b 传输与接线

- [x] `src/chat/core/chatTypes.ts`：`ChatEvent` 枚举增 `AgentNodeEvent = "agent-node-event"`（字面量必须与后端 `event_name::AGENT_NODE_EVENT` 一致）
- [x] `src/services/stream/transport.ts`：新增 `AGENT_NODE_EVENT` 监听 → `handlers.onNodeEvent(payload)`；**不做**任何活跃会话过滤（I55 §6.3 第 1 条）
- [x] `src/services/stream/transport.ts`：移除 `agent-tool-call` 的活跃会话过滤（N10 / G6），**保留** `chat-user-message-confirmed` 的过滤（它只影响当前会话占位消息归属）
- [x] `src/services/stream/transport.ts`：`StreamTransportHandlers` 增 `onNodeEvent`；禁止在传输层新增 `route.link` 判断（既有纪律）
- [x] `src/services/chat/chatStreamEvents.ts`：接线 `onNodeEvent` → `orchestrator.onAgentNodeEvent`
- [x] `src/services/stream/turnOrchestrator.ts`：增 `onAgentNodeEvent`，只写观测存储、不做活跃会话判断；`onAgentToolCall` 改为「总是写观测存储，仅活跃会话再进策略」（把可见性判定上移到策略层，I55 §6.3 第 2 条）
- [x] `turnOrchestrator.ts` 的 `handleDeltaEvent`：把 `seq` / `iteration` 记入观测实例（只为时间线对齐）
- [x] `src/services/stream/turnStrategy.ts` / `strategies/agentTurn.ts`：不改签名（可见性判定留在 `turnOrchestrator` 既有 `strategyFor` 处）

### V2-c 三个投影（纯函数）

- [x] `src/stores/agent/observation/projections/graph.ts`：`projectGraph(TurnObservation) → GraphView`（12 格状态 + 圈数 + 当前活动边 + 每格 `source`；圈数 = 最大 `iteration`，不由后端下发，I55 §5.4 第 3 条）
- [x] `src/stores/agent/observation/projections/timeline.ts`：`projectTimeline(TurnObservation) → NodeFact[]`（按 `seq` 排序，保留 `transition` 供 inspector 区分迁移与过程事实）
- [x] `src/stores/agent/observation/projections/summary.ts`：`projectTurnSummary(TurnObservation) → 耗时分解 / 预算余量 / 迭代数`；耗时分解**必须跳过 `source === "assumed"` 的格子**（起点是猜的，I55 §6.6 第 4 条 / §5.3 第三个前提）
- [x] 三者互不引用、不读 store（输入只有 `TurnObservation`，I55 §6.2）；`wrap_up` 路径的乱序节点按事件顺序着色，不按模板顺序强推
- [x] 新增 `src/stores/agent/observation/schedule.ts`：**唯一**投影调度层（I55 §6.7 第 1 / 3 / 4 条）——折叠（`applyNodeEvent` / `applyDeltaSeq` / `applyToolCall`）**逐事件同步**、不排队不丢弃；三投影重算按帧合并（同一帧多次变更只重算一次，用 `requestAnimationFrame` 或等价单次合并）；`GraphCanvas` / `InspectorPanel` **不各自** `listen(agent-node-event)`，只订阅本层暴露的「投影已更新」
- [x] 投影不自带缓存（I55 §6.7 末段）：缓存放在调用方并以 `Object.is` 作失效判据，投影保持纯函数（否则 V3 的回放等价性测试失去意义）
- [x] V1 **不画百分比**：进行中只用「`running` + `atMs` 差」，不引入 `value` / `max`（I55 §5.3）；`assumed` 格子不显示已持续时长（同上）

### V2-d 渲染

- [x] 新增 `src/features/observation/module.ts`（`FeatureModule`，`routes` 注册 `/agent-observation`）+ `ObservationPage.tsx`
- [x] `src/features/registry.ts`：`loadFeatures` 注册该 module（与 `vaultModule` 并列，不经 `__VAULT_ENABLED__`）
- [x] `src/features/__tests__/featureModuleRefs.test.ts`：把新 module 加入 `modules` 常量
- [x] `src/features/observation/GraphCanvas.tsx` + `NodeCell.tsx`：7 值状态色 + 圈数徽标 + 出口汇点徽标（`completed` / `stopped` / `halted_by_budget` / `wrap_up` / `cancelled` / `failed`，I55 §3.2）
- [x] `NodeCell.tsx`：`source` 徽标——`observed` 无标记、`derived` / `assumed` 各一种可见形态（虚线边 / 问号角标），**禁止**三种 `source` 同形渲染（G13，I55 §6.6 第 3 条）
- [x] `src/features/observation/InspectorPanel.tsx`：时间线 + D 级字段（I55 §4.5），D 级字段一律 `Option` 且不参与判定；每条事实标注 `transition` 与 `source`，`derived` 值标「推导值」（I55 §6.6 第 5 条）
- [x] 空态与后台态：无观测实例时显示空态；会话切走后实例继续累积（后台实例，I55 §6.3）
- [x] `i18n/locales/{zh-CN,en-US}.json`：节点中文名（取 I55 §3.1 表）+ 状态名 + `source` 三值名 + 空态文案

### V2-e 验收（G2 / G4 / G6 / G7 / G8 / G10 / G12 读侧 / G13 / G14）

- [x] `src/stores/agent/observation/__tests__/reducer.test.ts`：G4（终态后事件不改状态）、G8（未知 `node` / `state` 进时间线、无格子）、`waiting` 三来源（审批 / 作答 / 重试）
- [x] 同文件：G12（读侧）—— `transition === false` 的事件只增 `timeline`，格子 `state` 与 `sinceMs` 逐字段不变（用并发批次的 5N 条 `StageEntered` 事件序列构造）
- [x] 同文件：G13 —— `source` 升级路径（`derived` → `observed`、`assumed` → `observed`，且不回退）；`projectTurnSummary` 跳过 `assumed`
- [x] 同文件：G14 —— 无变化的事件返回**原引用**（`expect(next).toBe(prev)`，覆盖终态保护丢弃 / 未知取值 / 重复 `finished` 三种构造）；同一帧内 N 条事件只触发**一次**投影重算（spy 计数），且 `timeline` 事实数等于事件数（不丢事件，I55 §6.7 第 1–4 条）
- [x] 同文件：`timeline` 有界 —— 超 2000 条后按固定顺序丢弃并留「已省略 N 条」合成事实；状态迁移终态事实不被丢弃（I55 §6.7 第 5 条）
- [x] 同目录 `template.test.ts`：G2（模板集合与契约 key 相等）
- [x] 新增 `src/services/stream/__tests__/transport.test.ts`（`vi.mock("@tauri-apps/api/event")` 提供可控 `listen`，仓内尚无传输层单测）：G6（非活跃会话的 `agent-tool-call` 与 `agent-node-event` 仍写入观测存储；`chat-user-message-confirmed` 仍按活跃会话过滤）
- [x] G7 依赖方向检查：新增单测静态断言观测模块不被 `turnOrchestrator` 的 `send` / `cancel` 路径 import（观测是只读副本，I55 §6.5）—— `__tests__/dependencyDirection.test.ts`
- [x] G10：既有 `turnOrchestrator` / `agentSegments` / `chatStreamStore` / `chatStreamEvents` 相关测试全绿，无快照变更
- [x] 命令：`npm run typecheck`、`npm run lint`、`npm run test`、`npm run contract:check`

---

## V3 — 回放（受闸，不随 V1/V2 开工）

前置：V0 已完成（P15 已在 I38 §2 阶段表）；V1 / V2 全部勾选；且必须有真实事件量数据支撑保留策略（I55 §7.1 第 2 条）。未满足前本节一律不动。

- [ ] 迁移：`agent_node_events` 表（I55 §7.2 形态），主键 `(turn_id, seq)`，只追加永不更新
- [ ] 写侧：落库走既有 `TurnEmitter` 出口，**不新建发射器**（O1：每个 turn 恰好一个发射者）
- [ ] 保留策略：单 turn 上限 4000 行；超限丢弃顺序确定且可解释（先 `running` 非首条，再 D 级 `detail`），禁止随机丢
- [ ] 读侧：回放 = 按 `seq` 顺序喂给**同一个** reducer（V2-a），不写第二个解码器（I55 §7.2 第 2 条）；`transition` 原样保留（过程事实照样只进时间线），`source` 全部为 `observed`（回放没有恢复映射，不产生 `derived` / `assumed`）
- [ ] 与既有表的关系不变：`detail.messageIds` 是引用，不复制正文；禁止用节点事件重算 token 用量（I55 §7.3）
- [ ] 验收：回放结果与运行期投影逐格一致（同一 reducer 的等价性测试）；`turn_id` / `seq` 唯一约束

---

## V4 — 多实例与并发（不在本清单）

后台会话列表、按会话切换图实例、`TurnTasks` 键策略（并行节点）均依赖编排层阶段（I55 §11.4 第 4 条）。本清单不列条目、不预留字段，也不阻止该阶段。

---

## 门禁 → 任务 → 验证

| 门禁 | 归属批次 | 覆盖任务 | 验证 |
|---|---|---|---|
| **G1** 12 节点各一条 `finished` | V1 | V1-b 全部 12 条 | `tests/node_events.rs` |
| **G2** 模板与 node id 集合相等 | V2 | `template.ts` + `template.test.ts` | vitest |
| **G3** `seq` 与 `Delta` 共用单调序列 | V1 | `TurnEmitter::node` 走 `dispatch` | `tests/node_events.rs` |
| **G4** 终态唯一且不可覆盖 | V1 写侧 / V2 读侧 | V1-b「终态唯一性」+ `reducer.ts` | 双侧单测 |
| **G5** sink 失败不影响 turn | V1 | `RunnerObserver::node` 无返回值 | `RecordingSink::failing_after` |
| **G6** 后台会话事件不被丢弃 | V2 | transport 去掉过滤 | `transport.test.ts` |
| **G7** 观测不参与控制 | V2 | 依赖方向断言 | 静态 import 断言 |
| **G8** 未知 `node` / `state` 不渲染不吞 | V2 | `reducer.ts` | `reducer.test.ts` |
| **G9** 契约生成与登记 | V1 | V1-d 全部条目 | `contract:gen` / `contract:check` / `protocol_bindings` |
| **G10** 既有对话链路不变 | V1 + V2 | 无既有语义改动 | 既有测试集合全绿 |
| **G11** sink 恒无返回值 | V1 | `TurnObserver::node` / `EventSink::emit` 签名 | 签名断言 + 审查 |
| **G12** 过程事实与状态迁移分离（`transition`） | V1 写侧 / V2 读侧 | V1-a 字段 + V1-c `StageEntered` 发 `false` + V2-a reducer 分流 | 双侧单测 |
| **G13** 忠实度可审计（`source` 三级、只升不降、不同形渲染） | V2 | V2-a 标注 + V2-c summary 跳过 `assumed` + V2-d 徽标 | vitest |
| **G14** 变更抑制与投影调度（原引用 + 同步折叠 + 按帧合并 + `timeline` 有界） | V2 | V2-a 原引用契约 + V2-a `timeline` 有界 + V2-c 统一节流 | reducer / 投影单测（`toBe` + spy 计数） |

---

## 未决问题的落地口径

| # | 处置 | 落地位置 |
|---|---|---|
| Q1 | `admit` 失败不发声，观测只覆盖已准入的 turn | V1-b ① |
| Q2 | `ladder.applied` 只报实际改写请求的项（前后快照比较得出） | V1-b ④ |
| Q3 | `pipeline.index` 用全局序号，批次信息留 D 级 | V1-c |
| Q4 | 子 turn 是独立实例（独立 `observationKey`），父图只显示 `pipeline` 的 `waiting` + 子实例徽标 | V2-a（键策略）+ V4 前不实现徽标 |
| Q5 | 压缩 turn 复用同一事件族，模板允许只点亮部分节点（`pending` 语义天然支持） | V1-b / V2-a |
| Q6 | `agentId` / `parentTurnId` **V1 就带**（键恒在、值可为 `null`） | V1-a / V1-d |
| Q7 | 「turn 模板」（常量）与「工作流模板」（数据）分开命名，后者独立立项 | V0 第 4 条 |
| Q8 | 节点跳过仍由内核中间件裁决、经同一 emitter 发声；编排器只表达策略 | V1-b（`MiddlewareOutcome` 既有入口） |
| Q9 | 观测面是节点级采样、非全保真：`source` 三级标注证据等级，四类固定缺口不补全 | V2-a（`source` 标注）+ V2-d（徽标） |
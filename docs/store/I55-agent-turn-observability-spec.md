# I55 — Agent Turn 图观测规格（节点级事件与三投影）

| 项 | 值 |
|---|---|
| 状态 | 草稿（观测面规格；不含图编排器、不含画布交互） |
| 日期 | 2026-11-18 |
| 关联 | [I36a Agent 内核基线](./I36a-agent-kernel-baseline.md)（内核分层与「必须动内核」清单）、[I38 Agent 阶段总纲](./I38-agent-roadmap.md)（阶段门禁唯一台账）、[I38f 子代理规格](./I38f-agent-subagent-spec.md)（`agent_id` / `parent_turn_id` / `delegation_depth` 的语义来源）、[I54 资源监视器规格](./I54-resource-monitor-spec.md)（「不得用 0 冒充读不到」的显示口径）、[CODE-SPLIT-PRINCIPLE](../principle/CODE-SPLIT-PRINCIPLE.md)（拆分原则唯一来源，本文只引用不复述） |
| 影响范围 | 后端：`src-tauri/crates/assistant-agent/src/agent/round_executor.rs`（`TurnObserver` 加一方法）、`src-tauri/crates/assistant-agent/src/kernel/ports/mod.rs`（`TurnEvent` 加一变体）、`src-tauri/crates/assistant-agent/src/kernel/turn/emit.rs`（`TurnEmitter` 加一入口）、新增 `src-tauri/crates/assistant-agent/src/agent/node_event.rs`、`src-tauri/crates/assistant-agent/src/agent/loop_executor/{mod,call,execute,persist}.rs`（12 处出声点）、`src-tauri/crates/assistant-agent/src/agent/middleware.rs`（阶梯与 `after_tool` 裁决）、`src-tauri/crates/assistant-agent/src/agent/stop.rs`（刹车裁决）、`src-tauri/crates/assistant-tools/src/runtime/pipeline.rs`（`PipelineEvent` 加 `StageEntered`）、`src-tauri/src/protocol/dto/agent.rs`、`src-tauri/src/protocol/events.rs`、`src-tauri/src/adapters/event_sink.rs`。前端：`src/types/protocol.contract.ts`（生成，禁止手改）、`src/services/stream/transport.ts`（去掉活跃会话过滤）、新增 `src/stores/agent/observation/`、新增 `src/services/stream/observation.ts`。文档：本文 + [I38](./I38-agent-roadmap.md) §2.1 候选行 / §3 依赖行 / §4 文档地图（已登记为**未立项候选 P15**）。 |
| 参考实现 | deepseek-harness（微内核事件分类体系 / GUI 分层与 RPC 四象限 / 子代理生命周期事件）、ComfyUI（`NodeState` + `value/max` 进度）、n8n（`ExecutionStatus` 八值 + 按 `executionId` 键控的执行数据 store 与逐节点投影）、langgraph（检查点回放）。以上均为稀疏检出的参考项目，不在仓内；本文标注「参考实现」的形态判断分别出自上述条目，不外推到未检出的部分。 |

> 一句话：本规格**不新建编排器**，只把「一轮 agent turn 已经发生的事」补成可被图渲染消费的完整观测面——内核在 12 个既有节点上出声，wire 只增加一族事件，前端只增加一层按 `(conversationId, turnId)` 键控的投影存储；三者都不改变现有对话链路的行为，也**不阻挡将来的编排器**（编排器是同一份事件流的只读订阅者 + 独立控制端口，见 §11）。

本文中的「必须 / 禁止」为可验收约束；「应当」为默认做法，偏离需在代码注释里写明理由。

**编号约定**：`N1–N18` 是现状事实；`O1–O10` 是不变式（观测面自身的约束）；`G1–G11` 是门禁；`V1–V4` 是交付批次。节点编号沿用现行口头口径 ①–⑫，与 [I38](./I38-agent-roadmap.md) 的 `RoundExecutor` 术语一致。

---

## 1 目标与非目标

### 1.1 现状（以代码为准）

| # | 事实 | 落点 |
|---|---|---|
| N1 | 内核事件枚举只有 8 个变体：`StreamStart` / `Delta` / `PhaseChanged` / `Tool` / `CompactionStarted` / `CompactionFinished` / `Terminated` / `UserMessageConfirmed`，全部是 **turn 级**粒度，无任何节点级事件 | `src-tauri/crates/assistant-agent/src/kernel/ports/mod.rs` |
| N2 | 事件有唯一入口 `TurnEmitter`：分配单调 `seq`、发射前做代际校验、`Delta` 自动携带 `iteration`；`EventSink::emit` 同步无返回值，**发射失败只记日志、不得让 turn 失败** | `src-tauri/crates/assistant-agent/src/kernel/turn/emit.rs`、`kernel/ports/mod.rs` |
| N3 | `TurnObserver` 是执行器面向外界的唯一观察端口，已有 4 个方法：`set_phase` / `set_iteration` / `emit_tool_event` / `delta`；executor 被禁止直接持有 `EventSink` | `src-tauri/crates/assistant-agent/src/agent/round_executor.rs` |
| N4 | `TurnPhase` 只有 7 个值：`Starting` / `Compacting` / `Thinking` / `Streaming` / `ToolCalling` / `AwaitingApproval` / `Finalizing`，是**粗粒度**状态，无法区分「阶梯压缩」与「预算裁剪」等同处 `Thinking` 的节点 | `src-tauri/crates/assistant-agent/src/kernel/turn/phase.rs` |
| N5 | `ToolEventPayload` 有 4 个变体：`Started` / `ArgsDelta` / `Finished` / `Failed`；DTO 层另有 `AwaitingApproval` / `ApprovalClosed` / `AwaitingAnswer` 三个「等待」相位 | `src-tauri/crates/assistant-agent/src/agent/tool_event.rs`、`src-tauri/src/protocol/dto/agent.rs` |
| N6 | 工具流水线只有 **终态阶段** 记录：`ToolOutcome.stage`（`Find` / `Validate` / `Gateway` / `Execute` / `Persist`），成功路径不上报「进入过哪些阶段」；`PipelineEvent` 只有 `Started` / `Finished` / `Failed` | `src-tauri/crates/assistant-tools/src/runtime/pipeline.rs` |
| N7 | `PipelineEventSink` 端口已存在（`async fn emit`），且有 `InMemoryEventSink` 测试实现；适配器侧只转发 P2 的三个事件 | 同上、`src-tauri/src/adapters/event_sink.rs` |
| N8 | wire 事件名表共 23 个常量，与 agent 观测相关的是 `chat-stream-start` / `chat-delta` / `session-state-changed` / `agent-tool-call` / `chat-messages-changed` | `src-tauri/src/protocol/events.rs` |
| N9 | 事件路由分 `EventRoute::Orb`（只发 orb 窗口）与 `EventRoute::Broadcast`（全窗口）；agent 链路为 `Broadcast`，即主窗口**已经在接收 orb agent 的全量事件** | `src-tauri/src/adapters/event_sink.rs`、`kernel/turn/runner.rs` |
| N10 | 前端 `transport.ts` 对 `agent-tool-call` 按 **活跃会话** 过滤：`handlers.getActiveConversationId() === payload.conversationId` 不成立即丢弃 | `src/services/stream/transport.ts` |
| N11 | `chatStreamStore` 是全局单缓冲：`streamingContent` / `streamingMessageId` / `streamingIteration` 各只有一份，无按 turn 的实例表 | `src/stores/chat/chatStreamStore.ts` |
| N12 | `turnOrchestrator` 已有按会话的两张 Map：`phaseByConversation` / `turnIdByConversation`，注释明确「切会话不取消后台 turn」 | `src/services/stream/turnOrchestrator.ts` |
| N13 | 停止钩子共 4 个：`iteration-limit` / `tool-call-limit` / `turn-wall-clock` / `token-budget`（最后一个未配置时不安装，**无默认值**） | `src-tauri/crates/assistant-agent/src/agent/stop.rs` |
| N14 | `before_model` 中间件固定 7 项顺序：`projection` / `microcompact` / `wrap-up-restore` / `hard-trim` / `tool-output-cap` / `tool-outcome-capture` / `repeated-failure`；`after_tool` 内部顺序固定 D26 → D25 → D19 | `src-tauri/crates/assistant-agent/src/agent/middleware.rs` |
| N15 | 三维预算：迭代上限、工具调用上限、turn 墙钟；预算余量只在内部 `TurnBudget` / `TurnLimits` 里，**从不下发** | `src-tauri/crates/assistant-agent/src/kernel/limits.rs`、`agent/round_executor.rs` |
| N16 | 工具调用历史有落库：assistant 行 `messages.tool_calls_json` + 配对结果行（含 `tool_call_id` / `is_error` / `status` / `turn_index` / `iteration`），但**无节点级运行记录表** | `src-tauri/crates/assistant-storage/src/db.rs`、`conversations/msg_ops.rs` |
| N17 | `TurnRunCtx` 已预留 `agent_id` / `parent_turn_id` / `delegation_depth`，当前唯一调用点写死 `"root"` / `None` / `0` | `src-tauri/crates/assistant-agent/src/agent/round_executor.rs`、`kernel/turn/runner.rs` |
| N18 | 全仓无任何 `run` / `node` / `span` 级持久化或采样，`agent-tool-call` 是唯一的节点级事实来源 | 全局检索结论 |

**由此得到的观测缺口**：12 个节点中只有 5 个（准入、投影、模型调用、工具流水线、终态）有可用事件，7 个完全静默；且工具流水线内部的 5 个阶段在 wire 上塌缩成 1 个点。

### 1.2 目标

1. 内核在 **12 个既有节点** 上出声，使「一轮 turn 走到哪、为什么走、还剩多少预算」全部可观测；
2. 节点状态用**一套闭集**表达，图渲染器不需要为每个节点写分支；
3. 观测面**按 `(conversationId, turnId)` 可分组**，使前台 orb 与后台会话能画在同一张模板上；
4. 同一事件流可被**三种投影**消费（对话 / 图 / 时间线），投影之间不得互相污染；
5. 历史回放与实时渲染共用**同一个 reducer**，回放不是第二套解码器；
6. 观测面**不阻挡将来的编排器**：事件是它的只读输入，控制走独立端口，两者正交（§11）。

### 1.3 非目标

- **不做图编排器**：节点是静态模板，不由用户连线；本规格只交付「看到」。**注意**：这不是禁止将来做编排器——将来的编排器是观测面的**订阅者 + 自有控制端口**，本文已按此预留（§11）；
- **不做画布交互**（拖拽 / 缩放 / 编辑）与节点参数表单：这些属于后续编排层阶段；
- **不新增执行语义**：不加并行节点、不加条件路由、不改迭代循环结构；
- **不改既有事件语义**：`chat-stream-start` / `chat-delta` / `agent-tool-call` 的字段与消费方行为保持不变（O8）；
- **不做性能采样**：CPU / 内存 / 磁盘属于 [I54](./I54-resource-monitor-spec.md)，本文只观测 turn 自身；
- **不做跨 turn 汇总统计**：token 计量归既有 usage 记录，不在此重复造账；
- **不落正文**：节点事件只带事实与标识，不复制模型输出正文（正文的唯一来源仍是消息行）。

---

## 2 术语与分层

### 2.1 三个身份

| 身份 | 职责 | 谁来做 | 可否越界 |
|---|---|---|---|
| **事实生产者** | 在节点进入 / 离开时产生一条不可变事实 | 当前是内核（`TurnObserver::node`）；编排器接入后可上移（§11.2） | **不能**决定 UI 形态；只报「发生了什么」；**每个 turn 恰好一个发射者** |
| **承载者** | 分配 `seq`、做代际校验、序列化到 wire | `TurnEmitter` + `TauriEventSink` | 不能改写事实；不能合并两条事实；不能给事实加回执或背压 |
| **只读订阅者** | 把事实折叠成可渲染状态 / 供上层决策参考 | 前端投影存储；将来的编排器（§11） | **不能**经观测 sink 发起控制（O6 / O10）；控制走独立端口 |

这条分工沿用 [I36a](./I36a-agent-kernel-baseline.md) 的端口隔离：内核零 tauri 依赖，外部世界只经端口交互。观测面的新增同样走端口，不走直接调用。

**订阅者 ≠ 视图**：本规格的读者容易被「投影」二字误导，以为观测面的消费者只能是 UI。实际上「谁订阅事件」与「谁能控制执行」是两个正交的问题——将来的编排器会是同一份事件流的**只读订阅者**，而它的控制能力来自**另一个端口**（§11.2）。这条区分是本文档能否被编排层复用的关键，故在 §2.1 就写明。

### 2.2 行与列：节点 ≠ 状态

这是本规格最容易混淆的一处：

- **节点**是**行**，是图上的格子，共 12 个，**静态且数量固定**；
- **状态**是**列**，是某一时刻某格子的取值，取值域是 7 个值的闭集（§4.3）；
- **迭代**是**第三维**，同一个节点在不同迭代里有独立的格子实例（参考 ComfyUI 的 `node_id` + `prompt_id` 双键，本项目用 `(node, iteration)`）。

禁止把「节点」和「状态」合成一个枚举：那会让图渲染器无法区分「阶梯没走到」与「阶梯走了但被跳过」。

### 2.3 三种投影

| 投影 | 输入 | 输出 | 消费方 |
|---|---|---|---|
| **对话投影** | 既有 `chat-delta` / `agent-tool-call` / `chat-messages-changed` | 气泡、工具卡、段表 | orb 面板、对话视图（**行为不变**） |
| **图投影** | 节点事件 + 既有 turn 级事件 | 节点状态图、圈数、进度、预算余量 | 主窗口画布（后续阶段） |
| **时间线投影** | 节点事件的 `seq` + `atMs` | 有序事实流、耗时分解 | 节点 inspector、问题定位 |

三者读**同一份事件流**，各自维护自己的折叠结果。参考实现（n8n）的做法是：按 `executionId` 建一份执行数据 store，画布只消费其中的逐节点投影（`executionStatusByNodeId` / `runDataByNodeId` 等），不为每个视图各存一份原始数据。本项目照此：**原始事件只存一份**，投影是纯函数。

**投影不是唯一的消费形态**：编排器接入后，它会直接消费**原始事件**（而非任何投影），因为投影是为「人看」而裁剪过的，丢失了编排需要的标识与因果。所以原始事件必须自带完整标识（§11.4 第 1 条），不能指望从投影反推。

### 2.4 不变式

| # | 不变式 | 违反的后果 | 守住它的地方 |
|---|---|---|---|
| **O1** | **单一事实源**：节点状态只能由 `agent-node-event` 产生；禁止从既有 turn 级事件反推节点状态 | 两套推导必然漂移，图与实际执行不一致且无法定位谁的错 | §4.6（`pending` 不发事件、`skipped` 由 `branch` 批量标记）；唯一例外是 §6.4 的恢复映射，且只允许出现在一个函数里 |
| **O2** | **终态不可覆盖**：每个 `(node, iteration)` 恰好一条终态事件，写入后不得修改 | 图会在完成后「倒退」，用户无法信任 | 写侧：§4.6 第 4 条；读侧：reducer 丢弃终态后的事件并记日志（G4） |
| **O3** | **序列唯一**：`seq` 与 `Delta` 共用同一单调计数器，无重复、无回退 | 时间线投影无法把增量与节点事实对齐 | `TurnEmitter` 是唯一分配点（N2）；executor 不持 `EventSink`（N3） |
| **O4** | **模板静态**：节点表 / 边表 / 出口表是编译期常量，不进 wire、不由后端下发 | 首帧无法布局；模板漂移静默改变 UI | §3.3；G2 用单测锁死集合相等 |
| **O5** | **不造第二载体**：`waiting`、工具相位、工具阶段分别复用既有载体（N5 / N6），禁止为同一事实并行新增事件 | 双源必然不同步，前端要写两套合并逻辑 | §5.2、§4.7；新增的 `StageEntered` 是「进入阶段」这一**新事实**，不是既有终态阶段的重复 |
| **O6** | **观测不驱动当前 turn**：`EventSink` 恒为无返回值、无回执、无背压；任何「读事件 → 决定本 turn 下一步」的链路不得建立在它之上；同一 turn 内的控制只走既有中间件链（`&mut ModelRequestView` / `MiddlewareOutcome::StopTurn`）与 stop hooks | 内核会在发射点等待外部决策，观测失败即拖垮 turn（违 O9） | N2 的同步签名、N14 的中间件链；G7 / G11 |
| **O7** | **观测不写业务状态**：不写消息行、不进模型上下文、不重算 token 账 | 观测面成为事实的第二个来源，账目与上下文污染 | §7.3；`detail.messageIds` 只做引用 |
| **O8** | **新增不改旧语义**：既有事件字段与消费方行为保持不变 | 对话链路回归，且回归点分散在 5 个事件 | §4.7；G10 |
| **O9** | **失败隔离**：观测面的任何失败（sink 失败、前端解码失败、存储淘汰）不得影响 turn 完成与对话渲染 | 观测拖垮主链路——这是观测面最不可接受的失效模式 | 写侧 N2；读侧 §4.8；G5 / G8 |
| **O10** | **控制与观测分离**：跨 turn / 跨节点的控制决策（调度、取消、重试策略）必须经**独立控制端口**；`agent-node-event` 及其 sink 永远只是只读输入，不得被赋予改写、回执或调度能力 | 观测面被改造成控制通道后，既失去失败隔离，也无法同时服务「视图」与「调度」两类需求不同的消费者 | §11.2；G11 |

---

## 3 图模型（静态模板）

### 3.1 节点表

| # | node id | 中文 | 是否在迭代内 | 代码落点 |
|---|---|---|---|---|
| ① | `admit` | 准入 | 否（`iteration = 0`） | `kernel/tasks.rs` `TurnTasks::begin` |
| ② | `prepare` | 快照 + 投影 | 否（`iteration = 0`） | `agent/loop_executor/mod.rs` 请求构造 |
| ③ | `brake` | 刹车（4 个 stop hook） | 是 | `agent/stop.rs` |
| ④ | `ladder` | 上下文阶梯（7 项中间件） | 是 | `agent/middleware.rs` |
| ⑤ | `model` | 模型调用（含内层重试） | 是 | `agent/loop_executor/call.rs` |
| ⑥ | `persist-assistant` | assistant 行落库 | 是 | `agent/loop_executor/persist.rs` |
| ⑦ | `branch` | 判定（有无工具调用） | 是 | `agent/loop_executor/mod.rs` |
| ⑧ | `budget-split` | 预算切分 | 是 | `agent/loop_executor/budget.rs` |
| ⑨ | `pipeline` | 工具流水线（5 子阶段） | 是 | `assistant-tools/src/runtime/pipeline.rs` |
| ⑩ | `after-tool` | 工具后中间件 | 是 | `agent/middleware.rs` |
| ⑪ | `persist-tools` | 工具结果落库 | 是 | `agent/loop_executor/persist.rs` |
| ⑫ | `after-iteration` | 迭代尾（nudge 收集） | 是 | `agent/loop_executor/mod.rs` |

```
        ┌──────────────── 回边 A：iteration += 1（1..=max）─────────────────┐
        │                                                                  │
        ▼                                                                  │
 ①admit → ②prepare → ③brake → ④ladder → ⑤model → ⑥persist-assistant → ⑦branch
                        │(Stop)                │(重试⟲)                     │
                        ▼                      ▼                          ├─无工具─► 出口 completed
                     出口 stopped          出口 failed/cancelled            │
                                                                            └─有工具─► ⑧budget-split
                                                                                        → ⑨pipeline
                                                                                        → ⑩after-tool
                                                                                        → ⑪persist-tools
                                                                                        → ⑫after-iteration ──┘
```

### 3.2 出口（不是节点）

出口是 `TurnStatus` 的取值，渲染为汇点徽标，**不参与节点状态机**：

| 出口 | 由哪个节点决定 | 现状来源 |
|---|---|---|
| `completed` | ⑦ `branch` 无工具调用 | `Terminated` |
| `stopped` | ③ / ⑩ / ⑫ 的中间件返回 `StopTurn` | `Terminated` |
| `halted_by_budget` | ③ 刹车 | `Terminated` |
| `wrap_up` | ⑦ 达迭代上限且有工具 | `Terminated` |
| `cancelled` | ⑤ / ⑨ 子令牌取消 | `Terminated` |
| `failed` | ⑤ / ⑥ | `Terminated` |

### 3.3 模板不是数据（O4）

节点表、边表、出口表是**编译期常量**（`const` 数组），不进 wire、不由后端下发。原因有三：

1. 图模板若走 wire，每次渲染都要等一次往返，首帧无法确定布局；
2. 模板变化会静默改变前端布局，而节点集合实际由代码结构决定，跟着版本走即可；
3. 后续编排层需要「用户自定义图」时，那是**另一张图**（工作流模板），与本模板不是同一概念，不应共用同一个下发通道（工作流模板是**数据**，经命令 / 存储读取；本模板是**常量**，永不上 wire）。两者在代码里也必须分开命名，禁止用同一个类型名承载。

约束：模板常量必须与 §3.1 的 12 个 node id **一一对应**，新增/删除节点时两侧同批改，并用单测锁死集合相等（G2）。

---

## 4 事件词汇

### 4.1 为什么是「单一事件名 + `node` 判别」

三种候选：

| 方案 | 优点 | 缺点 | 结论 |
|---|---|---|---|
| 每节点一个事件名（`agent-node-brake` / `agent-node-ladder` …） | 前端可按需订阅 | 事件名表 +12；每加节点都要改适配层与前端订阅；与 N8 的常量表风格冲突 | 否决 |
| 复用既有 turn 级事件并加字段 | 不加事件名 | 会把节点密度（12 × iteration）灌进 `chat-delta` 这类高频事件，既有消费方语义漂移（违 O8） | 否决 |
| **单一 `agent-node-event` + `node` + `state` 双判别** | 前端一个 reducer 入口；图数据驱动；事件名表只 +1；与 `agent-tool-call` 的 `phase` 判别式同构（N5 已有先例） | payload 需要二级判别，schema 略复杂（由 ts-rs 生成 union 消解） | **采用** |

### 4.2 全形

```rust
pub struct AgentNodeEvent {
    pub turn_id: String,
    pub conversation_id: String,        // 前端 observationKey 的一半（§6.1）；缺它则无法按会话分组
    pub seq: u32,                       // 与 turn 内 Delta 共用同一单调序列（N2）
    pub node: AgentNode,
    pub iteration: u32,                 // 迭代内节点从 1 起；迭代外节点为 0
    pub state: NodeState,
    pub at_ms: u64,                     // 相对 turn 起点的毫秒，取自 Clock 端口
    pub agent_id: String,               // 预留：当前恒 "root"（N17），子代理/编排器接入后非 root
    pub parent_turn_id: Option<String>, // 预留：当前恒 None；`Option` 不加 `skip_serializing_if`（键恒在、值 null）
    pub detail: Option<AgentNodeDetail>,
}
```

wire DTO 同形（`AgentNodeEventDto`），字段 `camelCase`，`detail` 用 `kind` 标签的闭集 union。

**`conversation_id` 与 `agent_id` / `parent_turn_id` 是「路由键」，不是展示字段**：前端用前两个键定位实例，将来的编排器用后两个键把事件归到自己的调度单元（§11.4 第 1 条）。三个键都必须在 V1 就存在——补键是 wire 破坏性变更，补值不是。

**`seq` 必须与 `Delta` 共用同一个计数器**：两者混排才有意义——「第 37 条增量落在 `model` 节点的 `running` 期内」是时间线投影的核心判据。若各用一套计数器，两个序列无法对齐。

**`at_ms` 必须取自 `Clock` 端口**，禁止在内核里用 `SystemTime::now()`：内核的可测试性依赖时间可注入（[I36a](./I36a-agent-kernel-baseline.md) 的端口纪律）。

### 4.3 `NodeState`（7 值闭集）

参考实现：ComfyUI 的 `NodeState` 只有 4 值（`pending` / `running` / `finished` / `error`）+ `NodeProgressState` 的 `value` / `max`；n8n 的 `ExecutionStatus` 有 8 值（`canceled` / `crashed` / `error` / `new` / `running` / `success` / `unknown` / `waiting`）。本项目取两者的并集裁剪：

| 值 | 含义 | 何时产生 |
|---|---|---|
| `pending` | 已确定要执行、尚未进入 | 迭代开始时对**可能走到**的节点批量标记（不逐个发事件，见 §4.6） |
| `running` | 正在执行 | 进入节点 |
| `waiting` | 在执行中，但阻塞在外部 | 审批 / 作答 / 子代理等待 |
| `finished` | 正常完成 | 离开节点 |
| `error` | 失败 | 离开节点且失败 |
| `skipped` | 本轮不会走到 | ⑦ 判定后对未走的分支标记 |
| `cancelled` | 因取消而中止 | 子令牌取消 |

**不引入 `unknown`**：n8n 的 `unknown` 用于「从持久化恢复后无法判定」的场景；本项目在恢复场景用 `pending` + 补拉快照（§6.4）替代，避免出现一个语义空洞的值。

**终态只有 4 个**：`finished` / `error` / `skipped` / `cancelled`。`pending` / `running` / `waiting` 是可覆盖的中间态。这条必须由 reducer 强制（O2），不允许后端「回头改状态」。

### 4.4 `AgentNodeDetail`（按 node 分派的闭集）

只列 **M 级**（必需，驱动图渲染）；**D 级**（诊断，驱动 inspector）在 §4.5 单列，V1 不交付。

| node | detail | 字段 |
|---|---|---|
| `brake` | `BrakeDetail` | `verdict: "continue" \| "stop"`、`hook: Option<String>`、`budget: BudgetRemaining` |
| `ladder` | `LadderDetail` | `applied: Vec<String>`（实际生效的中间件名）、`droppedMessages: Option<u32>`、`trimmedChars: Option<u64>` |
| `model` | `ModelDetail` | `attempt: u32`、`retryReason: Option<String>`、`promptTokens: Option<u32>`、`completionTokens: Option<u32>`、`finishReason: Option<String>`、`toolCallCount: u32` |
| `pipeline` | `PipelineDetail` | `stage: PipelineStage`、`index: u32`、`total: u32`、`toolName: Option<String>`、`toolCallId: Option<String>` |
| `budget-split` | `BudgetSplitDetail` | `requested: u32`、`executed: u32`、`deferred: u32` |
| `branch` | `BranchDetail` | `hasToolCalls: bool`、`wrapUp: bool`、`toolCallCount: u32` |
| `persist-assistant` / `persist-tools` | `PersistDetail` | `rows: u32`、`messageIds: Vec<String>` |
| `after-tool` | `AfterToolDetail` | `verdicts: Vec<MiddlewareVerdict>`（`middleware` / `action` / `reason`） |
| `after-iteration` | `AfterIterationDetail` | `nudges: Vec<String>`、`nextIteration: u32` |
| `admit` / `prepare` | `None` | — |

**`BudgetRemaining` 的每个字段都是 `Option`，且 `None` 表示「未配置」而非 0**：`token-budget` 钩子未配置时不安装（N13），此时余量是「不设限」，显示为 0 会被读成「已用尽」。这是 [I54](./I54-resource-monitor-spec.md) §1.2 第 6 条同一口径的复用。

**`PipelineStage` 复用 `ToolStage` 的 5 值**（N6 已有 `Find` / `Validate` / `Gateway` / `Execute` / `Persist`），不新造枚举。语义差别只在「终态阶段」与「进入阶段」：前者已存在，后者由本规格新增的 `PipelineEvent::StageEntered` 提供（§4.7）。

**`LadderDetail.applied` 的事实来源已有**：`MiddlewareChain::names()` 返回「已注册的中间件名，顺序即执行顺序」，因此 `applied` 不需要新造清单，只需在链执行时记录「实际改写过请求」的那些项（见 Q2）。

### 4.5 M 级与 D 级的划分

| 级别 | 判据 | 交付批次 |
|---|---|---|
| **M**（must） | 缺了它，图上的格子无法着色或无法显示「还剩多少」 | V1 |
| **D**（diagnostic） | 缺了它，图仍然正确，只是 inspector 少一行解释 | V2 |

D 级字段示例：`ladder` 的 `estTokensAfter`、`model` 的 `ttftMs`、`pipeline` 的 `concurrencyBatch`、`after-tool` 的指纹摘要。**D 级字段一律 `Option`**，且不参与任何判定（O6）。

### 4.6 事件密度与 `pending` 的批量规则

一次 10 迭代的 turn，若每个节点每次迭代都发 `pending` + `running` + `finished`，事件数 ≈ 12 × 3 × 10 = 360。这对本地 IPC 无压力，但会淹没时间线。规则：

1. **`pending` 不发事件**：图投影在收到某个节点的第一条 `running` 时，用**静态模板**推导「同迭代中排在它后面的节点仍是 pending」，不依赖后端补发（O4 的直接收益）；
2. **`skipped` 由 ⑦ `branch` 一次性批量标记**：`branch` 的 `finished` 事件携带 `BranchDetail`，前端据此把 ⑧–⑫ 全部标为 `skipped`；
3. **`running` 每条节点每次迭代只发一次**；重试不重发 `running`，而是更新 `ModelDetail.attempt`（`model` 节点用 `waiting` 表示内层重试等待）；
4. **`finished` / `error` 每条节点每次迭代恰好一条**（O2 的写侧保证）。

### 4.7 与既有事件的关系

| 既有事件 | 是否复用 | 说明 |
|---|---|---|
| `chat-stream-start` | 复用 | 它已带 `turnIndex` / `iteration` / `phase`，是 turn 的起点坐标，不重复造 |
| `chat-delta` | 复用 | 正文增量归对话投影；图投影只用它的 `seq` 与 `iteration` 做「模型正在出字」的时间轴 |
| `session-state-changed` | 复用 | turn 级 phase，图投影用它做**粗校验**：节点状态与 phase 矛盾时以节点状态为准并记日志 |
| `agent-tool-call` | 复用，**不扩展** | 工具调用的 4 个 phase 与 3 个等待相位已足够（N5）；`pipeline` 节点的 `stage` 走新事件，两者是「节点」与「节点内的单个调用」的关系，不是重复 |
| `chat-messages-changed` | 复用 | `persist-*` 节点的 `finished` 之后由它刷消息；图投影不依赖它 |
| `CompactionStarted` / `CompactionFinished` | 复用 | 压缩是 turn 外的独立 turn，其观测按同一套节点事件覆盖，不另立 |

**新增事件只有一族**：`agent-node-event`。这条是 O8 的具体化——新增不改变既有语义。

### 4.8 丢弃与降级

1. 代际不符（`TurnEmitter::is_current` 为假）时**静默丢弃**，与既有行为一致（N2）；
2. `EventSink::emit` 失败只记日志，**不得**让 turn 失败；
3. 前端收到未知 `node` 或未知 `state` 时，**必须**保留该事件到时间线，但**不得**渲染未知格子——未知取值走「不显示」而不是「显示为默认态」。这是为将来节点集合扩张留的向前兼容口。

---

## 5 节点状态机

### 5.1 迁移表

| 节点 | 允许的迁移 | 终态 |
|---|---|---|
| `admit` | `running` → `finished` | `finished` |
| `prepare` | `running` → `finished` \| `error` | 同左 |
| `brake` | `running` → `finished`（含 `verdict: stop`） | `finished` |
| `ladder` | `running` → `finished` \| `error` | 同左 |
| `model` | `running` ⇄ `waiting` → `finished` \| `error` \| `cancelled` | `finished` / `error` / `cancelled` |
| `persist-assistant` | `running` → `finished` \| `error` | 同左 |
| `branch` | `running` → `finished` | `finished` |
| `budget-split` | `running` → `finished` | `finished` |
| `pipeline` | `running` ⇄ `waiting` → `finished` \| `error` \| `cancelled` | 同左 |
| `after-tool` | `running` → `finished` | `finished` |
| `persist-tools` | `running` → `finished` \| `error` | 同左 |
| `after-iteration` | `running` → `finished` | `finished` |

**`brake` 的 `stop` 不是节点失败**：刹车正常工作是 `finished` + `verdict: stop`，出口由 turn 终态表达。把「预算用尽」画成红色错误是错的——它是一条正常的控制流。

### 5.2 `waiting` 的三个来源

| 来源 | 现状载体 | 图投影如何得知 |
|---|---|---|
| 审批等待 | `agent-tool-call` 的 `AwaitingApproval` / `ApprovalClosed`（N5） | `pipeline` 节点由 `running` → `waiting`，`ApprovalClosed` 后回 `running` |
| 用户作答等待 | `agent-tool-call` 的 `AwaitingAnswer` | 同上 |
| 模型内层重试等待 | `chat-delta` 的 `deltaRetry` | `model` 节点 `waiting`，`retryReason` 取自 `ModelDetail` |

**禁止为 `waiting` 新增事件**：三个来源都已有载体，新增会造成双源（O5）。

### 5.3 进度（`value` / `max`）

参考实现 ComfyUI 的 `NodeProgressState` 是 `state` + `value` + `max` 三元组，且 `max` 可由节点自报。本项目 V1 **不引入进度值**，理由：

1. 12 个节点里只有 `model`（token 数）与 `pipeline`（调用序号）有天然分母；
2. `pipeline` 的分母已经在 `PipelineDetail` 的 `index` / `total` 里，不需要第二个字段；
3. `model` 的 token 进度需要 provider 流式上报累计值，现有 `chat-delta` 只带增量与最终 usage，硬算会得到不可信的估计值。

**V1 的替代**：图投影用「节点是否 running + 已持续时长（`atMs` 差）」表达进行中，**不画百分比**。V2 若引入进度，必须同时给出分母来源与「未知分母」的显示形态，禁止用 `0/0`。

### 5.4 圈数

`iteration` 是图上的「第几圈」徽标。规则：

1. 迭代外节点（`admit` / `prepare`）的 `iteration` 恒为 0，渲染时不带徽标；
2. 迭代内节点的 `iteration` 从 1 起，**与 `chat-stream-start` 的 `iteration` 同源**，不得各自推算；
3. 回边 A 的圈数 = 当前最大 `iteration`，由图投影从事件里取最大值得到，不由后端下发。

---

## 6 前端投影层

### 6.1 存储键

```
observationKey = `${conversationId}:${turnId}`
```

一张 Map：`Map<observationKey, TurnObservation>`。`TurnObservation` 内部再按 `node` + `iteration` 分格：

```ts
interface TurnObservation {
  conversationId: string;
  turnId: string;
  turnIndex: number | null;
  lastSeq: number;
  startedAtMs: number;
  status: "running" | "completed" | "stopped" | "halted_by_budget" | "wrap_up" | "cancelled" | "failed";
  cells: Map<string, NodeCell>;   // key = `${node}:${iteration}`
  timeline: NodeFact[];
}

interface NodeCell {
  node: AgentNodeId;
  iteration: number;
  state: NodeState;
  sinceMs: number;
  detail: AgentNodeDetail | null;
}
```

**为什么不是 `(turnId)` 单键**：`turnId` 全局唯一已经够用，但加上 `conversationId` 的价值在于**按会话列举实例**——画布要显示「这个会话的后台运行」，这是唯一自然的索引（参考实现 n8n 用 `executionId` 单键 + 反向映射；本项目会话是更强的一级概念，故双键）。

### 6.2 三个投影函数

| 投影 | 签名 | 用途 |
|---|---|---|
| `projectGraph` | `TurnObservation → GraphView` | 12 格状态 + 圈数 + 当前活动边 |
| `projectTimeline` | `TurnObservation → NodeFact[]` | inspector 的按时间排序事实流 |
| `projectTurnSummary` | `TurnObservation → { 耗时分解、预算余量、迭代数 }` | 画布顶部条 |

三者**都是纯函数**，输入同一份 `TurnObservation`，输出互不引用。这条保证「加一个视图」不需要改存储。

### 6.3 后台实例

**现状缺陷（N10）**：`transport.ts` 对 `agent-tool-call` 与 `chat-user-message-confirmed` 都做了活跃会话过滤（`handlers.getActiveConversationId() === payload.conversationId` 不成立即丢弃），导致切走后台上限内的工具事件被丢弃。修法：

1. 传输层**不再按活跃会话丢弃**，改为**总是**把事件写入观测存储；`chat-user-message-confirmed` 保留过滤（它只影响当前会话的占位消息归属，与图无关）；
2. 「是否影响当前可见对话」的判定上移到策略层（`strategyFor` 已经是唯一判据点，N12 的纪律不变）；
3. 保留一个**容量上限**：观测存储最多保留 N 个 `observationKey`（建议 32），按「最近有事件」LRU 淘汰，淘汰时只丢 `cells` 与 `timeline`，保留 `status` 摘要。

**禁止**在传输层做任何 `route.link` 判断（既有纪律）；会话级过滤属于编排层。

### 6.4 窗口重载与切会话

窗口重载后内存中的观测表为空，只能靠**快照补拉**：

1. 已有 `onPendingToolCalls`（`turnOrchestrator`）是挂起工具调用的唯一兜底来源（N12），图投影必须复用它的结论：有挂起调用即把 `pipeline` 置为 `waiting`；
2. `session-state-changed` 的 `Processing { phase }` 用于把对应节点置为 `running`（`phase` → node 的映射见下表），其余节点保持 `pending`；
3. **不得**为了补拉而新增一个「历史节点事件查询」命令：V1 不做持久化（§7），补拉只能来自快照 + 现有事件。

`TurnPhase` → node 的映射（**粗粒度，只用于恢复，不用于正常渲染**）：

| phase | 恢复时点亮 |
|---|---|
| `Starting` | `admit` |
| `Thinking` | `ladder` |
| `Streaming` | `model` |
| `ToolCalling` | `pipeline` |
| `AwaitingApproval` | `pipeline`（`waiting`） |
| `Compacting` | `ladder`（压缩视图单独标注） |
| `Finalizing` | `persist-assistant` |

这张映射**只允许出现在一个函数里**（`nodeFromPhase`），且必须注释「仅恢复用」。正常渲染一律以节点事件为准。

### 6.5 不得反向驱动（O6）

观测存储是**只读副本**。禁止出现：

- 由观测状态决定是否发送 / 取消 turn；
- 由观测状态改变气泡渲染（对话投影的唯一输入仍是既有事件）；
- 把观测状态写入消息行或模型上下文（O7）。

理由：观测面一旦参与控制，它就从「事实镜像」变成「第二套状态机」，与内核状态机必然漂移。

本节约束的是**前端观测存储**；将来的编排器同样不得读取它来做控制决策（O10）——编排器若需要事实，应订阅 wire 原始事件，而不是读前端折叠后的投影（§11.4 第 5 条）。

---

## 7 持久化与回放

### 7.1 是否落库

V1 **不落库**。理由：

1. 节点事件的完整价值在**运行期**（用户看的是正在跑的那一轮）；历史轮次已有消息行 + 工具行可看；
2. 落库需要新表 + 迁移 + 保留策略，而保留策略必须先有真实事件量数据才能定；
3. [I38](./I38-agent-roadmap.md) §7 第 5 条规定：任何候选开工前必须先改阶段表。落库属于新阶段，不在本文 V1 范围。

### 7.2 回放路径（V3）

若后续需要回放，形态已定：

```sql
CREATE TABLE agent_node_events (
  turn_id         TEXT NOT NULL,
  seq             INTEGER NOT NULL,
  conversation_id TEXT NOT NULL,
  node            TEXT NOT NULL,
  iteration       INTEGER NOT NULL,
  state           TEXT NOT NULL,
  at_ms           INTEGER NOT NULL,
  detail_json     TEXT,
  PRIMARY KEY (turn_id, seq)
);
```

约束：

1. **只追加**，永不更新（与 `seq` 单调一致）；
2. **回放 = 把事件按 `seq` 顺序喂给同一个 reducer**，不写第二个解码器；这是 langgraph 检查点回放形态的最小化借用——本项目不需要检查点快照，因为状态可由事件重算；
3. 保留上限：单 turn 最多 4000 行；超限的丢弃顺序**必须确定且可解释**：先丢 `pending`（本就不发，故实际不适用）→ 再丢 `running` 的非首条 → 最后丢 `detail` 的 D 级字段。禁止随机丢；
4. 与消息行的关系：`detail.messageIds` 是**引用**，不复制正文（§1.3 末条）。

### 7.3 与现有表的关系

| 现有表/列 | 关系 |
|---|---|
| `messages.tool_calls_json` | 工具调用的**参数与声明**来源；节点事件不重复存参数 |
| 工具结果行（`tool_call_id` / `is_error` / `status`） | 工具调用的**终态**来源；节点事件不重复存结果正文 |
| usage 记录 | token 计量的唯一来源；`ModelDetail` 的 token 字段是**运行期快照**，不是账本 |

**禁止**用节点事件重算 token 用量：那会与 usage 账本产生第二个口径。

---

## 8 参考项目对照

| 形态判断 | 参考来源 | 本项目的取舍 |
|---|---|---|
| 事件是带类型的扩展点，按分发模式分类（`waterfall` / `serial` / `parallel` / `emit`） | deepseek-harness 微内核事件分类体系 | **本项目两种模式都已存在，不能笼统说「只有单向 sink」**：同一 turn 内的控制链是 `waterfall`（`src-tauri/crates/assistant-agent/src/agent/middleware.rs` 的 `before_model(&mut ModelRequestView)` 可改写请求、`MiddlewareOutcome::StopTurn` 可中止）；观测面是 `emit`（fire-and-forget、失败隔离，N2）。本文只新增 `emit` 侧，**不改既有 `waterfall` 链**；将来编排器要做执行中介入，扩展点是**加中间件**（既有能力），不是改造观测 sink（§11.1） |
| 轮次与步骤边界由持久会话事件拥有，不由 UI 事件拥有 | 同上 | 采纳：`chat-stream-start` / `Terminated` 仍是 turn 边界的唯一来源，节点事件不得充当边界 |
| 注册表成员变化以类型化事件广播，消费方镜像而非假设顺序 | deepseek-harness 子代理生命周期事件 | 采纳其**精神**：节点集合变化靠静态模板（O4）+ 单测锁死（G2），不引入运行时注册表广播 |
| 四象限消息模型（谁发起 × request/response），wire 可校验可对账 | deepseek-harness GUI 分层与 RPC 协议 | 部分采纳：本项目观测事件是**纯推送**（server → client 单向），四象限中的 ③ 单向推送格；不引入 rpcId/应答语义 |
| 节点状态四值 `pending` / `running` / `finished` / `error` + `value`/`max` 进度 | ComfyUI `comfy_execution/progress.py` | 采纳状态四值作为子集，扩展 `waiting` / `skipped` / `cancelled`；**不采纳**进度值（§5.3 给出理由） |
| 执行状态八值，且分「可崩溃覆盖的中间态」与「终态」 | n8n `execution-status.ts` | 采纳「中间态 vs 终态」的二分（§4.3 末条），不采纳 `unknown` / `crashed`（本项目无跨进程恢复语义） |
| 按 `executionId` 键控的执行数据 store，画布只消费逐节点派生投影 | n8n `workflowExecutionState.store.ts` | 采纳：观测存储单例 + 纯函数投影（§6.1 / §6.2） |
| 检查点回放 | langgraph `checkpoint` | 采纳其「回放走同一解码路径」的原则；不引入检查点快照（§7.2 第 2 条） |

---

## 9 门禁与验收

| # | 门禁 | 验证方式 |
|---|---|---|
| **G1** | 12 个节点在成功路径上**各至少一条** `finished` 事件；`skipped` 分支有批量标记 | `assistant-agent` 组件测试：跑一轮带工具的 turn，断言事件序列覆盖全部 12 个 node id |
| **G2** | 前端模板常量与后端 node id 集合**完全相等** | 前端单测：把 `AGENT_NODE_TEMPLATE` 的 id 列表与契约生成类型的 key 集合做相等断言 |
| **G3** | `seq` 与 `Delta` 共用同一单调序列，且严格递增 | 组件测试：混排 delta 与 node 事件，断言 `seq` 无重复无回退 |
| **G4** | 每个 `(node, iteration)` 恰好一条终态；终态不可被覆盖 | reducer 单测：构造终态后的事件，断言状态不变并记日志 |
| **G5** | `EventSink` 抛错 / 不可用时 turn 仍正常完成 | 端口替身测试：注入永远失败的 sink，断言 turn 状态为 `completed` |
| **G6** | 后台会话的工具事件**不被丢弃** | 前端单测：非活跃会话的 `agent-tool-call` 仍写入观测存储 |
| **G7** | 观测面不参与控制：无任何发送/取消路径读取观测状态 | 代码审查 + 依赖方向检查（观测模块不得被 `turnOrchestrator` 的 send/cancel 路径 import） |
| **G8** | 未知 `node` / `state` 不渲染、不崩溃、不吞事件 | 前端单测：喂入未知取值，断言时间线有记录、图无该格 |
| **G9** | `contract:gen` 幂等，`protocol.contract.ts` 为生成产物；新增 DTO 已登记 `src-tauri/tests/protocol_bindings.rs` 的 `declarations` 数组 | `npm run contract:gen` + `npm run contract:check`；`protocol_bindings` 测试 |
| **G10** | 既有对话链路行为不变 | 现有 `turnOrchestrator` / `transport` / `agentSegments` 测试全绿，无快照变更 |
| **G11** | 观测 sink 签名恒无返回值（`EventSink::emit(&self, TurnEvent)` / `PipelineEventSink::emit` 均返回 `()`），且任何控制决策路径不依赖观测 sink | 签名断言 + 代码审查；编排器（若存在）的控制路径不得 import 观测 sink 类型（§11.2） |

**回归口径**：本规格的门禁只覆盖**新增观测面**；既有链路的等价性由现有测试集合保证，不新增「观测等价」类测试（那会变成第二套事实）。

---

## 10 交付批次

| 批次 | 内容 | 退出条件 |
|---|---|---|
| **V1 出声** | `TurnObserver::node` + `TurnEvent::Node` + `TurnEmitter::node` + `agent/node_event.rs` + 12 处出声点 + `PipelineEvent::StageEntered` + wire DTO/事件名/适配层 + `contract:gen` | G1 / G3 / G5 / G9 通过；`cargo test -p assistant-agent` 全绿；clippy / fmt 通过 |
| **V2 投影与渲染** | 观测存储 + 三投影 + 传输层去掉活跃会话过滤 + 图渲染器（主窗口 feature module）+ 节点 inspector（D 级字段） | G2 / G4 / G6 / G8 / G10 通过；`typecheck` / `lint` / `vitest` 全绿 |
| **V3 回放** | `agent_node_events` 表 + 迁移 + 保留策略 + 回放 reducer | 需先按 [I38](./I38-agent-roadmap.md) §7 第 5 条把 P15 从 §2.1 候选移入 §2 阶段表并立项；本文不自行改变阶段门禁 |
| **V4 多实例与并发** | 后台会话列表、按会话切换图实例、`TurnTasks` 键策略（若需要并行节点） | 依赖编排层阶段（§11.4 第 4 条），不在本文范围 |

**V1 与 V2 的边界**：V1 只保证「事件完整且可校验」，此时前端可以不渲染；V2 才出现画布。这条边界的作用是让 V1 的验收不依赖任何 UI 决策。

---

## 11 演化：编排器接入（前瞻约束）

> 本节回答一个问题：**将来的编排器会不会被本文档挡住**。结论是不会——但前提是把「观测」与「控制」始终当成两个正交维度，而不是把观测 sink 改造成决策通道。

### 11.1 编排器的两种未来需求

| 需求 | 形态 | 现在是否已有承载 |
|---|---|---|
| **A 调度**（turn 之间 / 节点之间决定下一步做什么） | 编排器读事件 → 决定下一个 turn 或下一批工具 → 经命令入口发起 | **有**：观测 sink 作只读输入；发起走既有命令入口（`src-tauri/src/cmd/agent`）。本文不需要为它加任何东西 |
| **B 介入**（执行中改写请求、跳过节点、换重试策略） | 需要在执行路径上拿到可变视图 | **有**：`TurnMiddleware::before_model(&mut ModelRequestView)` / `after_tool(&mut ToolResultView)` / `after_iteration` + `MiddlewareOutcome::{Continue, StopTurn}` 已是可改写、可中止的 `waterfall` 链（N14） |

两条需求都不需要把观测 sink 变成有返回值的接口。反过来，若为了编排器把 `EventSink::emit` 改成 `async fn emit(...) -> Decision`，会同时破坏三件事：N2 的「发射失败只记日志」、O9 的失败隔离、以及「一个事件流服务多个消费者」的能力（视图不想等调度决策，调度也不该被视图的渲染速度拖慢）。

### 11.2 编排器接在哪一层

```
  ┌──────────── 编排层（未来，独立 crate / feature）────────────┐
  │  NodeScheduler（控制端口，自有决策状态机）                  │
  └──────┬───────────────────────────────────────▲─────────────┘
   控制（发起 turn / 取消 / 下一批）         观测（只读订阅，无回执）
         ▼                                        │
  ┌──────────── 内核层（现状）────────────────────┴─────────────┐
  │ TurnRunner ──► RoundExecutor ──► ToolPipeline               │
  │      │              │                                       │
  │ TurnEmitter（seq / 代际）──► EventSink ──► wire ──► 订阅者    │
  └─────────────────────────────────────────────────────────────┘
```

四条约束：

1. **编排器拥有自己的控制端口**（例如 `NodeScheduler`），不进内核；内核不认识「编排」这个概念。
2. **编排器只经既有命令入口发起 turn**（当前是 `src-tauri/src/cmd/agent`），不直接构造 `TurnSpec` 绕过命令层。
3. **观测 sink 恒为只读输入**：编排器可以订阅，但订阅不得被赋予回执、背压或「事件 → 决策」的同步往返（O10）。
4. **编排器的失败不得回灌内核**：与 O9 同口径——编排层崩了，正在跑的 turn 仍应正常完成。

**编排器放在前端还是后端，约束相同**：若将来把编排器做成 webview 内的模块（与画布同进程），它读的是同一份 wire 事件，仍然不得把前端观测存储当控制源（§6.5 末段），也不得要求投影携带编排字段（§11.4 第 5 条）。本节不预设编排器的位置。

### 11.3 哪些措辞是持久的，哪些会随编排器改变

| 本文措辞 | 持久性 | 编排器接入后的样子 |
|---|---|---|
| O1 单一事实源 | **持久** | 发射者可从内核上移为编排器，但**每个 turn 仍恰好一个发射者**（这才是 O1 的真正内容，而非「内核必须是发射者」） |
| O2 终态不可覆盖 | **持久** | 不变 |
| O3 `seq` 与 `Delta` 共用单调序列 | **持久** | 不变；编排器发起的新 turn 用新计数器 |
| O4 模板静态 | **限定于「本模板」** | 本模板（turn 内部 12 节点）仍是常量；用户自定义工作流图是**另一张图**，不得与本模板合并（§3.3） |
| O5 不造第二载体 | **持久** | 不变 |
| O6 观测不驱动当前 turn | **持久** | 不变；这是内核侧边界 |
| O7 观测不写业务状态 | **持久** | 不变 |
| O8 新增不改旧语义 | **持久** | 不变 |
| O9 失败隔离 | **持久** | 扩展到编排层：编排器订阅失败也不得回灌 |
| O10 控制与观测分离 | **持久** | 正是为编排器而写的边界：它不禁止编排器，只规定控制走哪个端口 |
| §3.1 的 12 节点表 | **会变**（节点数随内核结构变） | 新增节点（如 `spawn_agent`、后台任务）时两侧同批改并更新 G2 断言 |
| §6.1 的 `observationKey` 双键 | **持久** | 编排器可按同一键索引实例 |
| §7.1「V1 不落库」 | **会变**（V3 起落库） | 形态已由 §7.2 给出 |

一句话判据：**凡关于「谁产生事实、事实长什么样、谁可以读」的措辞都持久；凡关于「有几个节点、存在哪里」的措辞都会随实现演进。**

### 11.4 现在就要预留的（否则将来返工）

1. **事件自带完整可路由标识**：`conversationId` / `turnId` / `node` / `iteration` / `seq` + **`agentId` / `parentTurnId`**（已在 §4.2 定型）。后两个字段现在就能从 `TurnRunCtx` 直接取（N17 已预留、当前恒为 `"root"` / `None`），零成本；V1 若不带上，将来子代理或编排器接入时要同时改 wire、适配层与 reducer。
2. **`seq` 单调 + 代际校验**已具备（N2），编排器可直接做因果排序，无需另建序号。
3. **两层图模板现在就分开命名**：本模板叫「turn 模板」（12 节点，常量）；将来用户可编辑的图叫「工作流模板」（数据）。命名混用是将来最大的返工来源。
4. **`TurnTasks` 的键策略不动**：当前按 `conversation_id` 单 turn 互斥（`OnBusy::Reject`）。编排器若要并行节点，需要新的键策略——那是**编排层的事**，不在观测面，本文既不预留也不阻止。
5. **不要在投影里塞编排需要的字段**：投影为人服务，编排消费原始事件（§2.3）。

### 11.5 结论

本文档**不构成编排器的障碍**。真正会挡住编排器的三件事，本文都已明确禁止：把 sink 改成有返回值、把节点状态写进消息行、把节点模板做成 wire 数据。除此之外，编排器只需新增自己的控制端口与订阅逻辑，观测面一行不用改。

---

## 12 未决问题

| # | 问题 | 影响 | 建议 |
|---|---|---|---|
| Q1 | `admit` 的失败（会话忙碌拒绝）是否应产生节点事件 | 该 turn 从未 `begin`，`turn_id` 可能尚未分配，事件无坐标 | 建议**不产生**，走既有命令错误通道；观测面只覆盖已准入的 turn，图中不显示这一格 |
| Q2 | `ladder` 的 `applied` 是否包含未生效的中间件名 | 中间件可能因前置条件跳过 | 建议只报**实际改写过请求**的项；跳过项不报，避免图上一堆「动了但没动」 |
| Q3 | `pipeline` 的 `index` / `total` 在分批并发下的语义 | 一批 N 个调用并行，`index` 是全局序号还是批内序号 | 建议**全局序号**（按发起顺序），批次信息走 D 级 `concurrencyBatch` |
| Q4 | 子代理（[I38f](./I38f-agent-subagent-spec.md)）接入后，子 turn 的节点事件如何归并 | 子 turn 有自己的 `turnId`，父图要不要显示子图 | 建议：子 turn 是**独立实例**（独立 `observationKey`），父图只显示 `pipeline` 节点的 `waiting` + 一个「有子实例」徽标；不内联子图 |
| Q5 | 压缩 turn（`CompactionStarted`）是否复用同一套节点事件 | 压缩是 turn 外的独立流程，可能没有 12 节点 | 建议复用事件族，但节点子集不同；模板需允许「本轮只点亮部分节点」，这一点已由 `pending` 语义天然支持 |
| Q6 | 节点事件是否在 V1 就带 `agentId` / `parentTurnId` | 不带则将来子代理 / 编排器接入要同时改 wire、适配层与 reducer（§11.4 第 1 条） | 建议**V1 就带**：两者都可从 `TurnRunCtx` 直接取（N17），零成本；`Option` 键恒在 |
| Q7 | 「turn 模板」（常量）与「工作流模板」（数据）的命名与归属 | 命名混用会导致将来把静态模板改成 wire 数据，破坏 O4 与首帧布局 | 建议现在就固定命名，且工作流模板**独立立项**，不并入 P15 |
| Q8 | 编排器若需在节点粒度「跳过」某个节点，跳过事实由谁产生 | 若编排器直接往 wire 发节点事件，就出现两个发射者（违 O1） | 建议：跳过仍由内核侧中间件裁决并以同一 emitter 发声（`MiddlewareOutcome` 已是现成入口）；编排器只表达「策略」，不直接发声 |
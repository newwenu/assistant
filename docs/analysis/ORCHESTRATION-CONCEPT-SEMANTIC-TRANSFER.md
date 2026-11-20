# 编排分析概念清单与语义转移对照

> 日期：2026-11-20｜性质：**概念对账**（只回答「分析借了哪些概念、落到本项目变成什么、哪些是范畴错配」；**不含取舍决议**——哪些概念采用、缺口怎么排优先级、怎么实施，属规格职责）
> 对账对象：`AGENT-WORKFLOW-ORCHESTRATION-ANALYSIS.md`、`MODEL-LEVEL-ORCHESTRATION-ANALYSIS.md`、`ORCHESTRATION-CODE-SHAPE-REFERENCE-ANALYSIS.md`
> 判据来源：`docs/principle/CODE-REVIEW-PRINCIPLE.md`、`docs/principle/CODE-SPLIT-PRINCIPLE.md`
> 代码事实快照：仓库代码的 2026-11-20 时点（**时点记录**，不随代码演进追改，需要最新事实时以代码为准）；代码事实沿用对账对象的 N / M / R / S 编号，不另起口径
> 时效与终止：本文是**一次性决策输入**。编排层规格定案后整体归档冻结、不再修改；有效内容由规格承接，事实条目作为历史快照保留。
> 引用已归档规格时只写「文档名 +（已归档）」，不给路径。

---

## 0 命题核对

| # | 命题 | 核对 | 依据 |
|---|---|---|---|
| 1 | 三份分析里的「节点」是同一个概念 | **不成立** | 参考项目的节点是执行 / 编辑 / 观测三合一；本项目已分成三个角色（§1） |
| 2 | §12 的「4 缺 + 3 半缺 / 8 项全缺」可整体照收 | **不成立** | 其中 7 条是范畴错配（§4），不是待补能力 |
| 3 | 观测的 12 节点必须作为编排输入保留 | **不成立** | L2 编排只需 turn 终态 + 工具事件 + usage（§4） |
| 4 | 「被编排性」对照表的判据可直接使用 | **部分成立** | 须先按角色归类；对 WorkflowNode / ExecutionUnit 成立，对 ObservationNode 是范畴错配（§3） |
| 5 | 概念可以原义移植 | **部分成立** | 六种转移类型里只有「直接沿用」一类是真移植（§2、§3） |
| 6 | 本对账改变 wire / 契约 / 表结构 | **不成立** | 只重排概念口径；不改事件名、不改 schema（§5） |
| 7 | 「编排层是内核的客户端」是新设计 | **不成立** | 它是 §1.2 执行者身份、§4.5 只读订阅、§5 四条约束、O1 / O5 的收束命名，已写回 `AGENT-WORKFLOW-ORCHESTRATION-ANALYSIS.md` §1.2.1 |

一句话：**三份分析在命题层已经把 L1 观测与 L2 编排切开，但证据层仍用参考项目「三合一节点」的词汇计数，于是把观测侧的现象当成编排侧的缺口。本文给每个概念标出角色归属与语义转移类型，把范畴错配条目从缺口计数里分离出来，并指出缺口实际落在哪三个结构位置（类型化交接、模板版本、run 状态）。**

---

## 1 三个角色：对账的坐标系

| 角色 | 定义 | 现有承载 | 谁拥有 | 用户可编辑 | 可被调度 | 生命周期 |
|---|---|---|---|---|---|---|
| **ObservationNode** | turn 内部执行过程的事实投影 | I55 的 12 节点静态模板 + 7 值状态 + `agent-node-event` | 内核（编译期常量） | **否** | **否** | 一个 turn 的一次迭代 |
| **WorkflowNode** | 工作流模板里的一个节点，是对执行单元的**引用** + 参数 + 类型化交接契约 | 尚不存在（模板 schema 未定义） | 用户（数据资产） | **是** | 是（作为调度目标） | 跨 run 存在，版本不可变 |
| **ExecutionUnit** | 真正产生事实的执行体，有生命周期 / 取消 / 预算 / 观测 / 落库 | turn / 工具调用 / 会话；四种执行体 Unit / Companion / Job / Hook | 内核 | 否 | 否（只被请求） | turn ≤ t < run |

三条不变式：

1. **ObservationNode 不是积木**：它是只读报告，不参与调度、不被连线、不作为 run 状态来源。
2. **WorkflowNode 不是执行体**：它只持有「发起哪个 turn、用什么参数、消费什么输入」，执行仍经既有命令入口。
3. **ExecutionUnit 不是模板**：`TurnSpec` 是编译期构造器，工作流模板是用户数据；两者以 O4 的双类型双表分离。

---

## 2 概念清单

### 2.1 ComfyUI

| 概念 | 原义 | 本项目对应物 | 归属角色 | 转移类型 |
|---|---|---|---|---|
| 节点（类属性契约） | 执行 / 编辑 / 观测三合一 | 无单一对应物 | — | **拆分** |
| `RETURN_TYPES` | 类型化输出声明 | 尚无输出类别契约 | WorkflowNode | **替换**（类型化交接，非变量池） |
| `NODE_CLASS_MAPPINGS` / `CATEGORY` | 运行时注册表 | `ToolCatalog` 只注册工具；`TurnSpec` 是构造器 | WorkflowNode / ExecutionUnit | **替换**（注册表 + bootstrap） |
| `IS_CHANGED` | 变更签名，判定是否跳过 | 模板版本号 / 内容哈希（尚不存在） | WorkflowNode | **替换**（模板版本，非节点跳过） |
| `expand` / `add_ephemeral_node` | 运行时生长子图 | 子代理 P8.1（未实现） | WorkflowNode | **不引入**（静态模板 + 版本化） |
| `ExecutionBlocker` | 调度层阻断传播 | 观测面 `skipped` | ObservationNode | **范畴错配**（报告 ≠ 调度） |
| `INPUT_IS_LIST` | 同一单元对列表逐项跑 | `is_concurrency_safe`（能否并行） | ExecutionUnit | **范畴错配** |
| `VALIDATE_INPUTS` | 图级类型对接校验 | `coerce_to_schema`（单次调用内） | WorkflowNode | **替换**（保存期校验） |
| `add_external_block` | 等外部值 / 人在环 | `ask_user` 工具 | ObservationNode / ExecutionUnit | **范畴错配**（工具 ≠ 节点） |
| `lazy` / `check_lazy_status` | 运行中决定依赖 | `route`（turn 结束后路由） | ExecutionUnit | **范畴错配** |
| `TopologicalSort` + ready set | 就绪判定 | 调度器尚不存在 | WorkflowNode | **直接沿用** |
| `CacheKeySetInputSignature` | 节点输出缓存 | 默认不缓存 + `replayable` 显式标记 | ExecutionUnit | **替换**（可判定幂等） |
| `PromptQueue` | 排队执行 | `TurnTasks` 是互斥表，不是队列 | ExecutionUnit | **不引入**（不可当队列用） |
| `execute_outputs` | 部分执行 | 尚不存在 | WorkflowNode | **直接沿用** |
| `ExecutionResult` 三值 + 分支错误 | 分支级错误处理 | turn 有终态，无分支级 | WorkflowNode / ExecutionUnit | **直接沿用**（run 级语义属另一层） |
| `DynamicPrompt` 覆盖 / 注入 | 运行时改图 | 静态模板（O4） | WorkflowNode | **不引入** |
| `normalize_outputs` | 媒体输出归一 | 产物链路（模型级分析的 G5） | ExecutionUnit | **直接沿用** |
| `validate_loops` | 环校验 | D2：图保持 DAG | WorkflowNode | **直接沿用** |
| `NodeState` 4 值 / `value,max` | 节点进度 | I55 已裁剪为 7 值闭集 | ObservationNode | **直接沿用**（仅限观测） |

### 2.2 n8n / langgraph / Dify

| 概念 | 来源 | 原义 | 本项目对应物 | 归属角色 | 转移类型 |
|---|---|---|---|---|---|
| `executionId` 键控 | n8n | 执行实例标识 | `run_id` | WorkflowNode 实例 | **直接沿用** |
| `waitingExecution` | n8n | 等待回填后恢复 | 图级 HITL 节点（复用 `ask_user` DTO） | WorkflowNode | **直接沿用** |
| `ExecutionStatus` 8 值 | n8n | 执行状态闭集 | I55 已定 7 值 | ObservationNode | **不引入**（已裁剪） |
| IF 节点 / 多分支 | n8n | 条件路由 | 边上的类型化交接判定 | WorkflowNode | **直接沿用** |
| superstep | langgraph | 批量推进 | turn 内部迭代循环 | ExecutionUnit | **替换**（机制不同） |
| checkpoint | langgraph | 状态快照 | run / node_run 台账 | WorkflowNode 实例 | **直接沿用**（自带 schema） |
| 子图隔离 | langgraph | 嵌套执行 | 子 run / 委派节点 | WorkflowNode | **直接沿用** |
| `ChannelRead/Write` | langgraph | 状态访问动词 | 不借用对象代数，只借「唯一入口」 | — | **不引入** |
| 引擎包 `graphon` | Dify | 引擎与产品分包 | 无独立引擎包；对应位置是 workspace 内 crate 边界 | — | **收窄**（包边界 → crate 边界） |
| `agent_v2` 节点 | Dify | agent 作为工作流节点 | turn 节点 | WorkflowNode → ExecutionUnit | **直接沿用** |
| `human_input` 节点 | Dify | HITL 作为节点 | 图级 HITL 节点 | WorkflowNode | **直接沿用** |
| 变量池 | Dify | 全局共享变量 | 无；改用类型化交接 | WorkflowNode | **替换** |
| `graph_topology` | Dify | 保存期拓扑校验 | O13 | WorkflowNode | **直接沿用** |
| `NodeFactory` / bootstrap | Dify | 注册在引擎、bootstrap 在产品 | 注册表尚不存在 | WorkflowNode | **直接沿用** |

### 2.3 本项目既有（被重新解释的概念）

| 概念 | 原义 | 被分析如何使用 | 正确归属 | 问题 |
|---|---|---|---|---|
| `TurnSpec` | 编译期链路构造器 | 节点参数来源 | ExecutionUnit 配置面 | 只消费，不新增配置面 |
| `TurnRunCtx` | turn 归属上下文 | run 归属载体（已预留三键） | ExecutionUnit | 无 |
| `TurnTasks` | 会话内 turn 互斥 | 被当作「队列」 | ExecutionUnit | **互斥 ≠ 调度**（§12.3 第 3 项） |
| `TurnEvent::Node(AgentNodeEvent)` | turn 内节点事件 | 编排的输入 | ObservationNode | 词汇被单一链路占据，缝的前提问题 |
| `NodeState` 7 值 | turn 内节点状态 | 被当成「无需再设计」 | ObservationNode | **≠ run 级节点状态** |
| `ask_user` | 工具 | 被当成图节点 | ExecutionUnit | **工具 ≠ 节点** |
| `spawn_agent` | 工具（有深度限制） | 委派节点 | ExecutionUnit | 图内委派与工具调用不共享深度与取消树 |
| `skipped` | 观测状态 | 被当成调度阻断 | ObservationNode | **报告 ≠ 调度** |
| `route` | turn 结束后选路 | 被当成 lazy 依赖 | ExecutionUnit | 时点不同 |
| `BudgetRemaining` | turn 预算 | 被当成 run 预算 | ExecutionUnit | run 预算属另一层 |
| 观测 12 节点图 | 静态只读投影 | 被当成「本项目的 L1 图」 | ObservationNode | **不是可编辑图的前身** |

### 2.4 三份分析新造的概念

| 概念 | 来源文档 | 含义 | 性质 |
|---|---|---|---|
| L1 / L2 / L3 三层粒度 | AGENT-WORKFLOW §1.1 | turn 内 / turn 间 / 会话上 | 沿用；L1 与编排无关 |
| 三个身份（图作者 / 调度者 / 执行者） | AGENT-WORKFLOW §1.2 | 权利拆分 | 沿用 |
| 价值门槛（并行 / 条件 / HITL） | AGENT-WORKFLOW §4.12 | 能力门槛判断 | 沿用；两处口径不一致（§4.12 与 §7） |
| 被编排性 / 统一编排 | AGENT-WORKFLOW §12 | 两侧能力对照 | 沿用判据；条目需按角色归类 |
| 四种执行体 Unit / Companion / Job / Hook | MODEL-LEVEL §1.3 | 闭集执行体 | 沿用 |
| 缝 / 反向依赖边 = 0 | CODE-SHAPE §1、§5 | 可机检的清晰指标 | 沿用 |
| run / node_run / edge 台账 | AGENT-WORKFLOW §4.7 | 业务事实落库 | 沿用 |
| 图级授权（配置面） | AGENT-WORKFLOW §4.8 | 配置面，不可跳过 Gateway | 沿用 |
| 类型化交接 | 本对账 | 节点间按输出类别传递，非变量池 | 新增概念（对应变量池的位置） |
| 模板版本 / 冻结快照 | 本对账 | `IS_CHANGED` 的对应位置 | 新增概念 |

---

## 3 语义转移的六种类型

| 类型 | 含义 | 典型条目 | 对应关系 |
|---|---|---|---|
| **直接沿用** | 机制与语义均成立，只换载体 | 就绪集合、executionId 键控、waiting–resume、checkpoint、agent/HITL 节点、保存期校验、分支错误、环校验 | 一对一映射 |
| **收窄** | 名字保留，语义缩小 | 节点 → turn/工具调用；缓存 → 仅 `replayable`；环 → DAG + turn 内收敛；授权 → 配置面 | 一对多映射，带限定条件 |
| **拆分** | 一概念裂成多角色 | ComfyUI「节点」→ 三角色；`ExecutionStatus` → run 状态 + 7 值 + NodeState；「图」→ turn 模板 / 工作流模板 / 观测图 | 需先定角色再计数 |
| **替换** | 换机制但解决同一问题 | 纯函数缓存 → 可判定幂等；变量池 → 类型化交接；superstep → turn 迭代；动态注入 → 静态模板 + 版本 | 位置相同、机制不同 |
| **不引入** | 参考项目有此机制，本项目无对应位置 | 8 值状态、全量缓存、`DynamicPrompt` 注入、`TurnTasks` 当队列、langgraph 对象代数 | 无对应物 |
| **范畴错配** | 概念属于不同角色，不构成缺口 | `skipped` 阻断、`ask_user` 节点、`is_concurrency_safe` 扇出、`route` lazy、NodeState 复用 | 归属角色不同（§4） |

---

## 4 范畴错配清单

> 这些条目在三份分析里被记为「半缺 / 全缺」，但按 §1 的三角色划分，它们属于 ObservationNode 或 ExecutionUnit，不是 WorkflowNode 的待补能力。

| # | 原条目 | 为什么错配 | 正确归属 | 核对 |
|---|---|---|---|---|
| 1 | §12.2 #5 `ExecutionBlocker` ↔ `skipped` | `skipped` 是**报告**节点未执行，不是**调度**阻断下游 | ObservationNode | 不构成「被编排性」缺口；run 级阻断是另一语义 |
| 2 | §12.2 #8 `add_external_block` ↔ `ask_user` | `ask_user` 是**工具**，不是图节点；图级 HITL 是另一形态 | ExecutionUnit → WorkflowNode | 图级 HITL 与 `ask_user` 不是同一位置，不计入「已有 half」 |
| 3 | §12.2 #6 `INPUT_IS_LIST` ↔ `is_concurrency_safe` | 「能否并行」≠「对列表扇出」 | ExecutionUnit | 不构成缺口；扇出属另一语义 |
| 4 | §12.2 #9 `lazy` ↔ `route` | `route` 在 turn **结束后**选路，不是运行中决定依赖 | ExecutionUnit | 不构成缺口 |
| 5 | §12.3 #2 缓存「全缺」 | 节点非纯函数，问题不是「缺缓存」而是「不能缓存」 | ExecutionUnit | 不构成「统一编排缺口」 |
| 6 | R2「NodeState 已被 I55 采纳，无需重复设计」 | 7 值是 **turn 内**状态；run 级节点状态是另一层 | ObservationNode | 该判断只覆盖 ObservationNode；run 状态是另一层 |
| 7 | §12.4「三项我们领先」 | 观测 / 取消 / 审批属不同角色，与 ComfyUI 的能力对比不同维度 | 多角色 | 事实成立，但不构成「编排能力领先」的对照 |

**分离出范畴错配后，落在 WorkflowNode / ExecutionUnit 的缺口**：类型化输出契约、节点注册表 + bootstrap、模板版本（变更签名）、部分执行、分支级错误策略、环校验、run 状态闭集。

---

## 5 结构核对

**分析借来的不是「一套节点」，而是「三种被焊死的职责」。** 参考项目里执行、编辑、观测是同一个类；本项目里它们已经是三个东西。本对账的作用不是补缺口，而是把观测侧的现象与编排侧的缺口分开计数。

分离出范畴错配后，落在 WorkflowNode 与其 run 实例的缺口收敛为三处结构位置：**类型化交接**（变量池所在的位置）、**模板版本**（变更签名所在的位置）、**run 级状态闭集**（7 值 NodeState 之外的另一层）。这三处与 V0 的观测节点无关。

本文不改任何事件名、wire 字段或表结构；它只给三份分析提供一把统一的尺子。

---

## 参考

- 对账对象：`AGENT-WORKFLOW-ORCHESTRATION-ANALYSIS.md`、`MODEL-LEVEL-ORCHESTRATION-ANALYSIS.md`、`ORCHESTRATION-CODE-SHAPE-REFERENCE-ANALYSIS.md`
- 关联：`AGENT-CANVAS-RENDERING-ANALYSIS.md`（双图分离、格子/盒子分型）
- 观测面规格：I55 观测规格（已归档）
- 拆分原则：`docs/principle/CODE-SPLIT-PRINCIPLE.md`、`docs/principle/CODE-REVIEW-PRINCIPLE.md`
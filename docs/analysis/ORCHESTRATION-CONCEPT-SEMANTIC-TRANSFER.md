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

### 2.5 边与卡片词表（唯一出处）

**为什么需要这一节**：三份分析里「节点」跨四层、「边」跨三个语义、「接线」只以工程动词出现，导致同一句话在不同章节指向不同东西；而规格的图例与文案必须让用户知道「他正在操作的这个东西叫什么」。本节给每层一个专名，供规格与 i18n 引用，**本文不复述第二次**。

**性质标注**：`沿用` = 有原文依据；`命名` = 本文提出、待规格采纳；`未定案` = 依赖未决取舍项（D11 / D5），规格不得预设。

#### 2.5.1 同一个「节点」在四层各叫什么

| 层 | 观测侧（L1） | 编排侧（L2 / L3） |
|---|---|---|
| **语义** | ObservationNode：turn 内执行过程的事实投影 | WorkflowNode：对执行单元的引用 + 参数 + 类型化交接契约 |
| **数据形状** | 格子（cell）：`row` / `column` / `order`，无尺寸、无端口 | 盒子（box）：`x` / `y` / `w` / `h` + 端口 + `z-order` |
| **渲染形态** | 单元格（cell）：名称 + 状态 + 序号 + 证据徽标；主体**恒为运行摘要** | 卡片（card）：头部 / 主体 / 端口区 / 脚部 / 选择态 / 拖拽柄；主体随「模式 × 缩放」四态（见 2.5.2「卡片模式」） |
| **事实来源** | 只读副本（O6 / §6.5 / C14） | 模板 = 命令产物；run 进度 = 只读投影 |

「格子」与「卡片」**不是简配与高配**：格子没有参数（ObservationNode 不是积木）、没有端口（12 节点共享同一 `TurnRunCtx`）、没有动作（只读）。两套视图模型不得合并（画布分析命题 5 / C16）。

#### 2.5.2 编排侧（L2 / L3）术语

| 术语 | 英文 | 定义 | 性质 |
|---|---|---|---|
| **画布** | canvas | 编排面的容器；有视口、缩放、平移、可编辑 | 沿用 |
| **卡片** | card | 节点在编排画布上的渲染形态，六件构成（见 2.5.1） | 命名 |
| **执行卡片** | execution card | 引用一个执行单元的卡片；`WorkflowNode` 的主类别 | 命名 |
| **容器卡片** | container card | 承载子图的卡片（会话级 / 迭代容器），先占名、未立项 | 命名 |
| **卡片类别** | card kind | 执行卡片按被引用执行单元分型；候选 turn / 工具调用 / 会话 / HITL，闭集由工作流模板 schema 定（D5） | 未定案 |
| **卡片模式** | card mode | 决定卡片**主体**显示什么：**编辑模式**显示参数表单、**运行模式**显示运行摘要；与缩放级别（详情 / 概览）正交，四态见下 | 命名 |
| **参数** | parameters | 用户为节点填的**常量配置值**，存进工作流模板；`WorkflowNode` 三项构成之一（§1）。n8n 同词 | 沿用 |
| **参数 schema** | parameter schema | 参数的定义（字段 / 类型 / 默认值 / 必填）；`ToolSpec.input_schema` 已有同构物，`WorkflowNode` 级**缺**（编排分析 §12.2 #2） | 命名 |
| **参数表单** | parameter form | 卡片主体在编辑模式下的一组可填控件；即文档原词「节点参数表单」（编排分析 §1.4） | 命名 |
| **参数项** | parameter item | 参数表单里的一个字段 = 一个控件（下拉 / 数字 / 文本）；ComfyUI 的 `widget` 即此物 | 命名 |
| **参数摘要** | parameter summary | 参数表单的只读折叠形态：只显示已填值，不可改 | 命名 |
| **端口** | port | 卡片上可连接的位置，同时是**视觉锚点 + 命中区 + 类型声明**三合一 | 沿用 |
| **输出槽 / 输入槽** | output slot / input slot | 端口的两个方向；`sourceHandle` / `targetHandle` 即槽 id | 命名 |
| **交接端点** | handoff endpoint | **抽象层**：D11 选端口级时取值 = 槽；选节点级时取值 = 节点整体（句柄恒 `null`） | 命名 |
| **接线** | wiring | 两个交接端点之间的**类型化交接实例**；即 `(源端点, 目标端点, 交接类别)` | 命名 |
| **交接类别** | handoff type | 接线上传递的输出类别；闭集未定（编排分析 §12.2 #1 标「缺，最关键」） | 未定案 |
| **连线** | connector | 接线的**渲染形态**：路径 + 命中区 + 标签 + 状态（活动 / 选中 / 悬空） | 命名 |
| **连接句柄** | handle | 连线两端锚定的槽 id，沿用 xyflow 的 `sourceHandle` / `targetHandle` | 沿用 |
| **悬空接线** | dangling wiring | 一端或两端未接的接线；保存期校验对象（O13） | 命名 |
| **就绪集合** | ready set | 拓扑顺序语义：入边全部完成的节点集合 | 沿用 |
| **激活** | activation | 条件语义：由边决定哪条下游进入（不引入条件指令，编排分析 §1.3） | 沿用 |
| **冲突域** | conflict domain | 并行节点声明「不得同时写同一资源」的域（O8）；落点未定 | 未定案 |
| **插入点** | insertion point | 连线上可插入新节点的位置（边即入口，R10） | 命名 |
| **连线中** | connecting | 手势状态：拖拽已开始、尚未落下 | 沿用 |

**三层不要混**：`接线` 是语义（类型匹配实例）、`边` 是数据形状（`source` / `target` / 句柄）、`连线` 是渲染（像素与命中区）。同一句「这条边是红的」在三层各有落点，规格必须指定层。

**卡片主体的四态**（`卡片模式` × 缩放级别）：

| 模式 \ 缩放 | 详情（可读表单） | 概览（只读摘要） |
|---|---|---|
| **编辑模式** | 参数表单（可填） | 参数摘要 |
| **运行模式** | 运行详情（输出预览 / 错误） | 运行摘要（状态 + 进度） |

四态是**同一张卡片主体的内容切换**，不是四种卡片。观测格子没有这一维：格子主体恒为运行摘要（它只有运行模式）。

**「填」与「接」是两个动作，不是两种外观**：

| 动作 | 落在哪 | 生效时机 | 数据去哪 |
|---|---|---|---|
| **填**（fill） | 卡片主体的参数表单 | 编辑期 | 固化进工作流模板 |
| **接**（connect） | 端口 → 接线 | 运行期 | 上游输出在运行时流入 |

同一字段理论上既可填也可接（ComfyUI 的 widget→input 转换即此），但**本项目 Q1 未定案前不开放转换**：`WorkflowNode` 的参数只接受常量，不接受引用。

**不存在「可填的卡片」这个子类**。`WorkflowNode` = 引用 + **参数** + 交接契约（§1），参数是执行卡片的**定义性构成**，不是可选装饰：

| 形态 | 能填吗 | 原因 |
|---|---|---|
| 观测格子 | **不能** | 不是积木（ObservationNode 无参数），只读副本 |
| 执行卡片 | **必能** | 参数是 `WorkflowNode` 三项构成之一 |
| 容器卡片 | 能 | 容器自身的参数（迭代次数 / 会话键） |

所以「可以填东西的卡片」**不需要新名字——它就是执行卡片**。需要命名的是它**填东西的那块区域**：参数表单。

#### 2.5.3 观测侧（L1）术语

| 术语 | 英文 | 定义 | 性质 |
|---|---|---|---|
| **观测栅格** | observation grid | L1 的容器；固定拓扑、只读、无视口无缩放无拖拽 | 命名 |
| **格子** | cell | 节点在栅格里的渲染单元；见 2.5.1 | 沿用（代码 `NodeCell`） |
| **锚点** | anchor | 格子边缘的**纯几何**锚定位置；不承载类型、不可连接 | 命名 |
| **模板边** | template edge | 12 节点常量拓扑：10 横向前向 + 1 纵向前向（⑦→⑧）+ 1 回边 | 沿用（C15） |
| **前向边 / 回边** | forward edge / loop edge | 模板边的两个类别 | 沿用 |
| **出口边** | exit edge | 指向**出口徽标**（汇点）而非节点的边；必须与节点边分成两类数据 | 沿用（画布分析 §4.6） |
| **出口徽标** | exit badge | 汇点的渲染形态；不是第 13 个节点（O4） | 命名 |
| **活动边** | active edge | 投影派生量（`activeEdgeOf`），不得由画布自行推断 | 沿用 |
| **相位** | phase | 边随两端节点状态取值（未开始 / 进行中 / 已完成 / 跳过…） | 沿用 |
| **证据等级** | evidence grade | 实测 `observed` / 推导 `derived` / 补拉 `assumed` 三值 | 沿用 |
| **圈** | iteration | turn 内的第几圈；画布只渲染当前圈 | 沿用 |

#### 2.5.4 易混词与禁用叫法

| 不要这样说 | 应这样说 | 理由 |
|---|---|---|
| 观测画布的「端口」 | **锚点** | L1 无边可接，叫 port 会被塞进类型校验 |
| 观测格子叫「卡片」 | **单元格** | 格子无参数、无端口、无动作 |
| 「画布」不加限定 | **观测栅格** / **编排画布** | 两者不是同一类 UI，唯一共同点是都画节点和边 |
| 「节点模板」不加限定 | **turn 模板**（L1 常量）/ **工作流模板**（L2 数据） | I55 §11.4 第 3 条：命名混用是最大返工来源 |
| `skipped` 一套含义 | L1 = **报告**未执行；L2 = **调度阻断** | 两张图上长得像、语义相反 |
| 「接线」与「连线」互换 | 语义用**接线**，渲染用**连线** | 否则类型校验与绘制会混在同一层讨论 |
| 边 = 接线 | 边是**数据形状**，接线是**语义实例** | 一条接线在数据里就是一条边 |
| 「可填的卡片」当作卡片子类 | 能填是**执行卡片的定义属性**；要指区域就说**参数表单** | `WorkflowNode` 三项构成之一就是参数 |
| 「参数」与「输入」互换 | 编辑期填的常量叫**参数**；运行期流进来的叫**输入** | 前者固化进模板，后者只在运行时存在 |
| 「参数表单」与「参数 schema」互换 | 表单是**控件集合**（UI），schema 是**字段定义**（数据） | 前者渲染，后者校验 |

#### 2.5.5 图例映射（供规格引用）

图例条目必须是**术语**而不是描述句；右侧键为现有 i18n 键（`src/i18n/locales/*.json`），缺键的标「待补」。

| 图例条目 | 术语 | 现有键 |
|---|---|---|
| 状态（7 值） | state | `observation.state.*` |
| 证据等级（3 值） | evidence grade | `observation.source.*`（含 `*_hint`） |
| 边类别 | forward edge / loop edge | `observation.graph.forward` / `observation.graph.loop` |
| 活动边 | active edge | `observation.graph.active_edge` |
| 出口 | exit badge | `observation.graph.exit`（**键已存在、全仓零引用**） |
| 圈 | iteration | `observation.graph.iteration` |
| 工具分支 | tool lane | `observation.graph.tool_lane` |
| 图例组标题 / 展开 | legend | **待补** |

### 2.6 开发原语词表（非用户可见）

**为什么需要这一节**：2.5 给的是**用户能看见的东西的名字**（格子、卡片、连线、锚点）。但开发中大量讨论发生在**用户看不见的中间层**：一次事件怎么折叠、投影算了几次、失效在哪一点合并、锚点从哪个数据推出来。这些层目前只有代码符号，没有中文名，导致评审时只能说「那个 projections 里的那个函数」。

本节只登记**代码里已存在的符号**或**W1 已规划符号**。原语**不进 wire、不进 DTO、不进 i18n**（O4 / 画布命题 12）。

**性质三档**：`已存在`（给出代码符号）/ `待建`（W1 计划，规格需标注）/ `不造`（明确拒绝，见 2.6.6）。

#### 2.6.1 数据原语（wire → 存储 → 视图模型）

| 原语 | 代码符号 | 层 | 说明 | 性质 |
|---|---|---|---|---|
| **节点事件** | `AgentNodeEventDto` | wire | 契约生成，唯一事实输入 | 已存在 |
| **观测实例** | `TurnObservation` | 存储 | 一份 turn 的全部事实：`cells` + `timeline` + `status` | 已存在 |
| **格子** | `NodeCell` | 存储 | `(node, iteration)` 的当前状态，含 `sinceMs` / `detail` / `source` | 已存在 |
| **事实** | `NodeFact` | 存储 | 时间线一条；`node` / `state` 为 `string`，未知值保留 | 已存在 |
| **格子键** | `cellKey(node, iteration)` | 存储 | `${node}:${iteration}`，格子唯一寻址 | 已存在 |
| **实例键** | `observationKey(conv, turn)` | 存储 | `${conversationId}:${turnId}` | 已存在 |
| **证据等级** | `FactSource` / `FACT_SOURCE_RANK` | 存储 | observed > assumed > derived，只升不降 | 已存在 |
| **实例终态** | `TurnObservationStatus` | 存储 | 7 值；出口不占节点 | 已存在 |
| **格子视图** | `GraphNodeView` | 视图模型 | 一格的可视投影（含 `unfinished`） | 已存在 |
| **边视图** | `GraphEdgeView` | 视图模型 | 仅 `from` / `to` / `kind`，**无 id、无句柄、无标签** | 已存在 |
| **图视图** | `GraphView` | 视图模型 | 12 格 + `activeEdge` + `nowMs` + `iteration` | 已存在 |
| **摘要视图** | `TurnSummaryView` | 视图模型 | inspector 汇总 | 已存在 |
| **投影包** | `ObservationProjections` | 视图模型 | `{ graph, timeline, summary }`，三件同源 | 已存在 |
| **实例列表项** | `ObservationInstanceView` | 视图模型 | 列表只取摘要字段，不交出整份实例 | 已存在 |

**一条关键读法**：`GraphEdgeView` 没有 `id`、没有句柄、没有标签——这不是缺字段，而是 L1 的边是**常量拓扑 + 派生相位**，不是可编辑数据。任何给 `GraphEdgeView` 加 `id` / `handle` / `label` 的提案，都是在把 L2 的形状塞进 L1（命题 5）。

#### 2.6.2 派生原语（纯函数，单向数据流）

| 原语 | 代码符号 | 签名要点 | 性质 |
|---|---|---|---|
| **折叠** | `reduceNodeEvent` / `reduceDeltaSeq` / `reduceToolCall` / `reduceSessionState` | `(obs, event) => obs`，逐事件、不合并、不丢弃 | 已存在 |
| **建实例** | `createObservation` | 首次事件建实例 | 已存在 |
| **图投影** | `projectGraph(obs) => GraphView` | 12 格恒定输出，缺失补 `pending` + `derived` | 已存在 |
| **时间线投影** | `projectTimeline(obs) => NodeFact[]` | 有界丢弃 + `synthetic` 省略事实 | 已存在 |
| **汇总投影** | `projectTurnSummary(obs) => TurnSummaryView` | — | 已存在 |
| **三投影一次算齐** | `projectObservation(obs)` | 三者互不引用，输入只有同一份 `obs` | 已存在 |
| **圈数** | `currentIterationOf(obs)` | 所有格子 `iteration` 的最大值 | 已存在 |
| **活动边** | `activeEdgeOf(obs, iteration)` | 全仓唯一算法；画布不得自推 | 已存在 |
| **模板前驱** | `templatePredecessor(node)` | 由 `order - 1` 反查，**隐式邻接** | 已存在（W1a 换显式边集） |
| **观测时钟** | `observedNowMs(obs)` | `max(atMs)`，**不是** `Date.now()` | 已存在 |
| **未完成判定** | `isUnfinishedState(state)` | `running` / `waiting` | 已存在 |
| **无缓存读** | `readProjections(conv, turn)` | 不缓存；调用方自己按 `Object.is` 判失效 | 已存在 |

#### 2.6.3 调度原语（唯一失效点）

| 原语 | 代码符号 | 说明 | 性质 |
|---|---|---|---|
| **订阅** | `subscribeProjections` | 组件唯一入口；禁止各自 `listen(agent-node-event)` | 已存在 |
| **待刷键集** | `pendingKeys` | 本帧需重算的实例键集合 | 已存在 |
| **帧合并** | `scheduleFlush` / `flushQueued` | `requestAnimationFrame`；无 rAF 退 `setTimeout(0)` | 已存在 |
| **刷新** | `flushPending` | 只对 `observation` 引用变了的订阅者重算 | 已存在 |
| **订阅者** | `Subscriber` | 持有上次 `observation` 与 `projections` 两个引用 | 已存在 |
| **写侧抑制** | `commit` | `next === prev` 直接 return（O11） | 已存在 |
| **淘汰** | `touch` / `recency` / `ensureCapacity` / `summarize` / `OBSERVATION_LIMIT` | 32 上限；淘汰只留 `status` 摘要 | 已存在 |
| **列表选择器** | `useObservationInstances` | 只取 Map 引用，不构造新数组进选择器 | 已存在 |
| **投影订阅 hook** | `useProjections` | 组件侧唯一订阅方式 | 已存在 |

**两条不变量（规格必须引用）**：

1. **抑制点只有一个**：`commit` 里的引用相等。往下到 DOM 没有任何第二道抑制；
2. **调度点只有一个**：`subscribeProjections`。任何组件自建监听都会绕开帧合并，让事件数 = 渲染数。

#### 2.6.4 模板原语（编译期常量）

| 原语 | 代码符号 | 说明 | 性质 |
|---|---|---|---|
| **模板常量** | `AGENT_NODE_TEMPLATE` / `TEMPLATE_BY_ID` | 12 条，`id/order/row/column/inIteration` | 已存在 |
| **编译期锁** | `satisfies Record<AgentNodeId, …>` | 缺一个或多一个节点即编译失败 | 已存在 |
| **节点序** | `AGENT_NODE_IDS` / `templateOrderOf` | `order` 0–11 | 已存在 |
| **工具分支集** | `TOOL_BRANCH_NODE_IDS` | ⑧–⑫；⑦ 判定后批量标 `skipped` | 已存在 |
| **状态闭集** | `NODE_STATES` / `TERMINAL_NODE_STATES` | 7 值 / 4 终态 | 已存在 |
| **守卫** | `isAgentNodeId` / `isNodeState` | 未知值不建格子，只进时间线（§4.8） | 已存在 |
| **边集常量** | `templateEdges`（待建） | 12 条：10 横向前向 + 1 纵向前向 + 1 回边 | 待建（W1a） |
| **边索引** | `templateEdgeIndex`（待建） | `(from, to) => kind`，O(1) 查相位 | 待建（W1a） |

**为什么必须建 `templateEdges`**：现在邻接由 `templatePredecessor` 隐式推导，而活动边由 `activeEdgeOf` 独立判定。两条规则各写一份，就会出现「高亮的边不在画出来的边集里」——这种不一致没有任何测试能拦住（画布分析 §4.6 / K14）。

#### 2.6.5 画布待建原语（L3 几何 / L5 交互）

| 原语 | 建议符号 | 说明 | 性质 |
|---|---|---|---|
| **锚点** | `anchorOf(node, side)` | 由 `row` / `column` 推导，**不读 DOM** | 待建（W1b） |
| **锚点坐标** | `AnchorPoint { xPct, yPct }` | 百分比，随格子尺寸自适应 | 待建（W1b） |
| **出口锚** | `exitAnchorOf(node)` | 右边缘 75% 高度，与 50% 端口桩错开 | 待建（W1b） |
| **边相位** | `edgePhaseOf(edge, graph)` | 四态；按 `seq` 取该格最后一条 `finished` | 待建（W1b） |
| **选择状态** | `selectedNodeId` | 单值；v1 不做多选 | 待建（W1c） |
| **选择动作** | `selectNode` / `clearSelection` | 纯 reducer，无副作用 | 待建（W1c） |
| **时间线过滤** | `filterTimelineByNode` | **投影之后**过滤；不得进投影签名 | 待建（W1c） |
| **图例组** | `LegendGroup` / `LegendItem` | 图例的结构原语，文案走 i18n | 待建（W1d） |

#### 2.6.6 反原语（明确不造）

| 不造的原语 | 理由 | 依据 |
|---|---|---|
| 全局坐标系 / `viewport store` | L1 是固定拓扑栅格，无视口无缩放 | 命题 5 |
| 时钟源 / `tickNow()` | 投影是纯派生，`nowMs` 只取已观测 `atMs` | I55 §5.3 |
| `ResizeObserver` 写进 store | 测量不进 store | P4 / K5 |
| 画布自算活动边 | 唯一算法在 `activeEdgeOf` | 命题 6 |
| 组件内 `listen(agent-node-event)` | 唯一调度点在 `subscribeProjections` | C5 |
| `aria-live` 区域 | 高频更新会逐帧播报 | D15 / WCAG 2.2 |
| `React.memo` 边界（v1） | 60–70 节点，收益低于「引用保持」风险 | 2.6.3 |
| 格子内的端口类型声明 | L1 无边可接，端口是 L2 概念 | N3 / 2.5.4 |
| L2 参数表单原语（`ParameterForm` / `ParameterItem` / `parameterSchemaOf`） | **L2 未立项**；符号先行会变成空头承诺，等模板 schema 定（D5）后再给符号 | 2.6.7 第 3 条 |

#### 2.6.7 原语的命名纪律

1. **原语不进 wire、不进 DTO、不进 i18n**：`FactSource` 是本地的（O4），`GraphView` 不出前端；
2. **同一层同一物只有一个符号**；跨层同名必须加限定——`NodeCell`（存储类型）/ `NodeCell.tsx`（渲染组件）/ 格子（用户词）三者不得混称；
3. **待建原语在规格里必须标「待建」并给符号**，落地后改标「已存在」；否则规格与代码各说各话；
4. **用户词与原语词成对给出**：图例写「活动边」，代码写 `activeEdge`，规格必须同时给出两列，否则用户词与实现会漂移。

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
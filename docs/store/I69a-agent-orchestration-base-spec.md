# I69a — 编排层规格（基础 / 数据 / 调度）

| 项 | 值 |
|---|---|
| 状态 | 定稿（可实施） |
| 日期 | 2026-11-23 |
| 阶段 | P18（编排轴 L2；见 [I38 §2](./I38-agent-roadmap.md) / §3） |
| 覆盖波次 | **P18-R0 – P18-R4** 的语义与结构约束（R4 的对外契约与 R5 的前端接缝见 [I69b](./I69b-agent-orchestration-contract-spec.md)） |
| 波次与交付顺序 | [I69](./I69-agent-orchestration-roadmap.md)（**唯一来源**：交付物、前置、退出条件、回退点；本规格不列交付物、不排波次） |
| 事实来源 | `docs/analysis/AGENT-WORKFLOW-ORCHESTRATION-ANALYSIS.md`（2026-11-20 快照：N1–N18、G1–G11、D1–D10、O1–O16；**时点记录，不随后续代码追改**） |
| 术语唯一出处 | `docs/analysis/ORCHESTRATION-CONCEPT-SEMANTIC-TRANSFER.md` §2.5（用户可见词）· §2.6（开发原语）；本文只引用，不复述定义 |
| 关联 | I55（已归档，观测面）· I38f（`spawn_agent`，暂缓）· [I52](./I52-shell-sandbox-spec.md)（沙箱不替代审批）· I66 / I68（已归档，画布 W1 / W2） |
| 影响范围 | 后端：新建 `src-tauri/crates/assistant-orchestrator/`；`assistant-storage/src/orchestration.rs`；`src-tauri/src/cmd/agent/mod.rs`（`start_turn` 回传 turn 标识、sink 组合 tap）。前端与 wire 见 [I69b](./I69b-agent-orchestration-contract-spec.md) |
| 契约影响 | **wire 冻结前零变更**（无 DTO、无命令、无事件）；冻结清单见 [I69b §2](./I69b-agent-orchestration-contract-spec.md) |
| 下游承接清单 | 见 §16.1（**逐条 ID，不用区间**）；上游新增或修改冻结项时，同批次同步本行所指两表与下游附录 |

> **本文管理**：定位与边界（§0–§1）、术语纪律（§2）、不变量（§3）、决议（§4）、结构与依赖（§5）、模板 schema 与校验（§6）、控制端口（§7）、台账与持久化（§8）、调度语义（§9）、失败矩阵（§10）、缺口落点（§11）、阶段关系（§12）、验收 A1–A24 / A26 / A28–A32（§13）、拒绝方案（§14）、未决问题（§15）、下游承接清单与冻结常量（§16）。
> **本文不管理**：波次顺序、交付物、退出条件、回退点（[I69](./I69-agent-orchestration-roadmap.md)）；事件、wire 契约与前端接缝（[I69b](./I69b-agent-orchestration-contract-spec.md)）；画布编辑器与运行态叠加的形态（[I69c](./I69c-agent-orchestration-canvas-spec.md)）。
> **两卷关系**：I69a 是语义与结构的主体，I69b 只定义「怎么暴露、前端怎么读」，**单向依赖 I69a**，不改本文任何约束。

> 本文中的「必须 / 禁止」为可验收约束；「应当」为默认做法，偏离须在代码注释写明理由。判据编号 **A1–A32 跨两卷统一定义、不重编**：A1–A24、A26、A28–A32 在本文 §13，A25 / A27 在 [I69b §4](./I69b-agent-orchestration-contract-spec.md)。「哪一波交付、按什么顺序、何时算过关」由 [I69](./I69-agent-orchestration-roadmap.md) 定义。

---

## 0 定位：编排层是内核的客户端

一句话：**编排层只请求执行、只读事实、自己记账、失败不牵连内核**。

| 客户端行为 | 编排层做什么 | 编排层不做什么 |
|---|---|---|
| **请求执行** | 经宿主控制端口发起一次 turn；turn 内部怎么跑由内核决定 | 不构造 `TurnSpec`、不直连 provider、不自己跑工具流水线 |
| **读事实** | 订阅 turn 终态（tap）与节点级事件（只读） | 不改 `EventSink` 签名、不加回执、不做背压、不读前端投影 |
| **自己记账** | 模板 / run / node_run 落库，run 状态只从台账读 | 不把 run 状态塞进消息行，不从前端缓存反推状态 |
| **失败不牵连** | 崩溃 / 丢事件 / 被杀时只降级自己的视图 | 不让 turn 失败、不重放事件、不改内核终态 |

**为什么是「客户端」而不是「内核的一部分」**：内核的 12 节点模板是编译期常量、共享一个 `TurnRunCtx`、只有一个 `TurnObserver`（分析 N3、N5）。把编排塞进内核，等于让常量模板变成可编辑数据、让唯一观测端口分叉——这两件事都是不可逆的结构破坏。所以编排必须站在内核外面。

---

## 1 范围与边界

### 1.1 本文的职责

本文只定义「做出来的东西必须满足什么」：不变量、决议、结构、端口、模板、台账、调度语义、失败矩阵与判据（A1–A24 / A26）。

**交付什么、按什么顺序交付、每波怎么验收、哪一波可回退，全部见 [I69](./I69-agent-orchestration-roadmap.md)**。需要知道某条约束落在哪一波，查 I69 §4 的交付物表。

约束的作用域到「编排层 + 它的前端接缝」为止。模板编辑器与运行态叠加的**交互形态**不受本规格约束（形态见 [I69c](./I69c-agent-orchestration-canvas-spec.md)），但**保存期校验语义（§6.3）**、**台账与调度事实（§8–§9）**与 [I69b §3.2](./I69b-agent-orchestration-contract-spec.md) 的读取纪律**仍然适用**——编辑器不得另行判定校验、不得自行修补 run 状态。

### 1.2 边界：不做的语义

| # | 不交付 | 原因 |
|---|---|---|
| 1 | L3（会话之上、跨会话 Router） | D1、D16；与 P8.3 边界未定 |
| 2 | 节点结果缓存与 `replayable` 标记 | D3；agent 节点非纯函数（分析 §4.2） |
| 3 | 图级环 | D2；循环收敛到 turn 内部迭代 |
| 4 | 图级授权（整链一次批准） | D7；会形成绕过 `Gateway` 的第二条路径 |
| 5 | 图内委派节点（`spawn_agent`） | D10；P8.1 暂缓，第二套深度规则禁止 |
| 6 | 工具调用节点 | D17；工具调用在 turn 内，无独立生命周期 |
| 7 | 参数引用上游输出 / 变量池 | D11；参数只接受常量 |
| 8 | typed handoff（交接类别闭集） | 分析 §12.2 #1 是最关键缺口，但闭集需单独裁定；v1 的边只有激活语义（§2.2 第 3 条） |
| 9 | 队列（pending / running / history） | 分析 §12.3 第 3 项；`TurnTasks` 是互斥表 |
| 10 | 运行时注入节点（运行时生长子图） | INV7；静态模板 + 版本化，运行时增删节点会让校验无法静态化 |
| 11 | 节点级续跑 | D15 |
| 12 | 画布基础设施改造（补边 / 视口 / 预设 / 布局算法） | W1 / W2 已交付（I66 / I68，已归档）：视口、几何（`anchorOf`）、卡片、边（`CanvasEdge`）、预设、放置栏、网格摆放（`layoutTemplate`，**行列号网格**，供观测栅格与预设摆放，不是任意图的自动布局）与选择原语（`selectedId` / `onSelect`）均已落地（`src/features/canvas/`）。**未交付且不立项**：出口锚点 `exitAnchorOf`——W3 / W4 不需要它（出口角标用 `CanvasCardView.exit` 现有呈现），故不登记为待建项；见 [I69c §2.1](./I69c-agent-orchestration-canvas-spec.md)。P18 只在其上叠加，不改设施 |
| 13 | 后台任务（应用关闭后继续跑） | P9 / I52-Ext |
| 14 | 原始事件落库 | D5；事件是广播，不入台账 |
| 15 | 运行时节点类型注册表（`NODE_CLASS_MAPPINGS` 类） | 节点类别是编译期闭集枚举（§6.1），不做运行时注册；分析 §12.2 #2 的缺口以闭集收口 |
| 16 | 模板删除命令 | `CONST-CMD-10` 已冻结、无删除命令；删除会让引用它的 run 台账失去模板定位（§8.1 的 `template_version`），需单独裁定。v1 不提供删除界面，也不以 `save` 变相清空（清空只产生「空模板」，不产生删除语义）；重命名不另立命令，走 `save` 改 `name` |

### 1.3 与观测面的分界

| 维度 | 观测面（L1，P15 已交付） | 编排层（L2，本文） |
|---|---|---|
| 单位 | 格子 `(node, iteration)`，12 个常量节点 | 节点 `WorkflowNode`，用户定义的 `turn` / `hitl` |
| 状态闭集 | `NodeState` 7 值（I55 定型） | `NodeRunState` 7 值（§8.2） |
| `skipped` 含义 | **报告**未执行 | **调度阻断**（入边未激活） |
| 拓扑来源 | 编译期常量 `TEMPLATE_EDGES` | 落库的模板数据 |
| 可编辑 | 否 | 是（W3 起） |
| 数据流向 | 只读投影，人看 | 只读输入，调度用 |

**两套闭集不得互借类型**：`NodeState` 与 `NodeRunState` 必须各自定义、各自有转换表。把 L1 的 `NodeState` 复用到 run 台账，会让「报告」与「阻断」两个相反语义共用一个词（词表 §2.5.4 已列为禁用）。

### 1.4 与既有命令入口的关系

| 能力 | 既有入口 | 编排层如何使用 |
|---|---|---|
| 发起 turn | `src-tauri/src/cmd/agent/mod.rs` 的 `start_turn`（`send_agent_message` 的唯一实现） | 经 `RunHost::launch_turn`，由宿主调用 `start_turn`。**不新增第二个发射点** |
| 取消 | `cancel_agent`（按会话）→ `TurnTasks::cancel` | 经 `RunHost::cancel_conversation`；run 级取消由编排层对在跑节点的会话逐个调用（§9.4） |
| 工具执行 | `ToolPipeline`（`assistant-tools`） | **不接触**。编排层不依赖 `assistant-tools`（§5.2） |
| 审批 / 提问回填 | `resolve_agent_tool_call` / `resolve_agent_question` | **不接触**。HITL gate 是编排层自己的通道（§9.5） |

---

## 2 术语引用与用词纪律

### 2.1 引用表

| 术语 | 唯一出处 | 本文用法 |
|---|---|---|
| 执行单元 / `WorkflowNode` / 卡片 / 端口 / 接线 / 连线 / 就绪集合 / 激活 / 冲突域 | 词表 §2.5.2 | 直接引用，不再定义 |
| 观测节点 / 格子 / 锚点 / 相位 / 证据等级 | 词表 §2.5.3 | L1 语境一律用这组词 |
| 折叠 / 投影 / 抑制点 / 调度点 | 词表 §2.6.2、§2.6.3 | 前端侧只引用，不改 |
| run / run 台账 / node_run / gate | §8 | 本文定义，属 L2 专属 |
| `TurnSpec` / `TurnRunCtx` / `TurnTasks` / `EventSink` | 分析 N3 / N7 / N8 | 只描述既有事实，不重命名 |

### 2.2 用词纪律

| # | 禁止 | 必须 |
|---|---|---|
| 1 | 把「节点」理解成 L1 的观测节点 | 本文的「节点」一律指 `WorkflowNode`；L1 一律写「观测节点」或「格子」 |
| 2 | 把激活边叫「接线」 | v1 的边**没有交接类别**，只有激活语义 → 叫**激活边**（activation edge）。接线（typed handoff）不在 v1（§1.2 第 8 项） |
| 3 | `skipped` 一套含义 | L1 = 报告；L2 = 调度阻断。两处都必须带限定语 |
| 4 | `run` 与 `turn` 互换 | run 是编排实例（含多节点）；turn 是执行单元（一次内核轮次） |
| 5 | 「工作流模板」与「turn 模板」混用 | 工作流模板 = 数据（落库、可编辑）；turn 模板 = 编译期常量（12 节点）。类型名、表名、文件必须分开 |
| 6 | 把「就绪集合」当存储状态 | 就绪集合是纯派生量（§9.1）；台账只存 `pending` / `running` / `waiting` 等显式状态 |

---

## 3 不变量

| # | 不变量 | 落点 | 检查方式 |
|---|---|---|---|
| **INV1** | **唯一发射者**：turn 只能经 `RunHost::launch_turn` 发起，宿主内部调 `start_turn`；编排层不得依赖 `tauri` / `assistant-agent` / `assistant-tools` | §5.2 依赖方向 | `cargo tree -p assistant-orchestrator` 无上述三项；全仓 `TurnSpec::agent()` 构造点仍只有命令层（O1） |
| **INV2** | **唯一工具路径**：工具执行仍由内核 Pipeline 完成；编排层不持有工具注册表、不自建执行 | §7.1 端口面 | 编排 crate 无 `assistant_tools` 引用（O5） |
| **INV3** | **观测只读**：编排层只订阅，`EventSink::emit` 签名不变、无返回值、无回执、无背压 | §7.2 | 端口 diff 为零；tap 实现不得阻塞（O2） |
| **INV4** | **失败不回灌**：编排层崩溃 / 丢事件 / 被杀三种情形下，turn 仍正常终态落库 | §10 F5–F7 | 单测：tap panic 后 turn 仍 `completed`（O15） |
| **INV5** | **执行节点集合受 O3 约束**：v1 只有 `turn` 一类执行节点；`hitl` 是等待节点、`group` 是模板分组（不是节点类别），二者均**不新增执行语义** | §6.1、D18 | 节点类别枚举逐项可映射：`turn`→N5 的 turn，`hitl`→无执行，`group`→不在枚举内（O3） |
| **INV6** | **双模板分离**：`WorkflowTemplate` 与 turn 模板不得共用类型名、不得互相引用；工作流模板不得引用 12 节点 id | §5.1、§6.1 | 类型名审查 + 编译期 `satisfies` 锁不变（O4） |
| **INV7** | **静态图**：保存期拒绝环与悬空边；运行时不得增删节点、不得注入临时节点 | §6.3、§9.1、§10 F8 | 校验单测（O13）；crate 内无 `add_node` 类 API |
| **INV8** | **台账唯一**：run 状态只从台账读；不得从前端投影、事件重放或时间戳反推 | §8.3 | 状态读取 API 只有 `RunLedger` 一处 |
| **INV9** | **取消树幂等**：run → node → turn；重复取消无副作用，取消后不再发起新节点 | §9.4 | 单测：二次 cancel 后状态不变（O7） |
| **INV10** | **会话互斥不被绕过**：同一 `conversation_id` 同一时刻最多一个 `running` 节点；编排层不得自建 turn 队列 | §9.2 | 单测：同会话两节点不并发（D6） |
| **INV11** | **预算分层且「未配置 ≠ 0」**：run 墙钟 / 节点数上限 / 并行上限为可执行闸门，token 只做读侧归因；四者口径独立；未配置一律显示为「未配置」，不得显示 `0`、也不得以 `0` 表示无限 | §9.6 | 单测 + 显示口径审查（O9） |
| **INV12** | **wire 最后冻结**：wire 冻结前零 DTO、零命令、零事件；冻结时一次性出契约 | [I69b §2](./I69b-agent-orchestration-contract-spec.md) | `git diff --stat src/types src/vault/core/contract` 在冻结前为空（O14） |
| **INV13** | **坐标不进语义**：模板语义字段（`nodes` / `edges`）与呈现字段（`layout`）分列；校验与调度只读语义 | §6.1、§8.1 | 校验单测：改坐标不改变任何校验结果 |

---

## 4 决议

分析文档 §7 的 D1–D10 与 §12 的 Q1–Q6 在此收敛。**每条决议给出「不做会怎样」**，因为编排阶段最大的风险不是做错，而是做了却没被约束。

| # | 取舍 | 决议 | 理由（不做会怎样） | 不可逆程度 |
|---|---|---|---|---|
| **D1** | 编排粒度 | **只做 L2（turn 之间）**。L3 留 P8.2 / P8.3 | 做 L3 要跨会话 Router，与 P8.3 重叠且需要后台推进；L2 只需 run 台账 | 中：L2 可后扩 L3，但 run 与会话的归属一旦绑死难解耦（D12 已预留解耦） |
| **D2** | 环的承载 | **图保持 DAG**；所有循环收敛到 turn 内部迭代；保存期拒环 | 允许图级环会让状态机从「每节点一次」变成「每节点 × 轮次」，需收敛判据与死循环保护，校验无法静态化 | 中：后加环要重做校验与调度 |
| **D3** | 节点结果缓存 | **不缓存**；schema 不引入 `replayable` 字段 | agent 节点非确定、有副作用、可被人工否决（分析 §4.2）；加字段等于承诺未实现的能力 | 低：不缓存没有历史依赖 |
| **D4** | 编排位置 | **后端独立 crate** `assistant-orchestrator` | 前端方案在「断开前端仍推进」「跨窗口一致」两条上不成立（O10）；「编排即模式」无法承载落库与取消树 | **高**：位置决定 crate 边界与宿主改动面 |
| **D5** | 落库范围 | **模板 + run + node_run + 结果引用**；**不落原始事件** | 落原始事件会让存储随 run 长度线性增长；只落引用已够审计与定位 | 中：后加原始事件只是新表 |
| **D6** | 并发键 | **`(run_id, node_id)`**；但执行仍受会话互斥约束 → **并行需不同会话** | 复用 `conversation_id` 与 N7 直接冲突；谎称「同会话可并行」会造出第二套 turn 键 | 中：键进事件与台账后改粒度要两端同改 |
| **D7** | 审批单元 | **保持调用级**；不做图级授权 | 图级授权会让长图「一次批准」变成绕过 `Gateway` 的权限旁路，与 I52「沙箱不替代审批」冲突 | **高**：一旦进审计字段，撤销要迁移 |
| **D8** | 预算分层 | **run 墙钟 + 节点数上限为可执行闸门；token 只做读侧归因** | 只有总预算则超支无法定位；token 实时闸门需要 turn 级用量回传（内核契约变更），本阶段不做 | 中：归因字段后加可兼容 |
| **D9** | HITL 回答去向 | **只进 run 台账，不进对话历史** | 进对话历史会把「图级确认」变成一条用户消息，与 `user-message-confirmed` 语义冲突，且污染会话上下文 | 中：节点类别进 schema 后删除需兼容旧模板 |
| **D10** | 与 P8.1 的先后 | **编排先立项，但 v1 schema 不含委派类别** | 分析 §4.11 的禁令针对「图内委派这条第二路径」；不含该类别即满足，不需等 P8.1 | 中：v2 加类别需 schema 版本升级 |
| **D11** | 参数与上游输出 | **参数只接受常量**，不接受引用（Q1） | 允许引用就要变量池 + 类型系统 + 求值器，等于把编排做成数据流引擎 | 中：后加引用要改 schema 与校验 |
| **D12** | run 与会话的归属（Q2） | run 是独立实体；`origin_conversation_id` 必填（UI 定位锚点）；**每个节点在发起时解析出自己的 `conversation_id`**（缺省 = `origin_conversation_id`），模板本身不绑会话 | 把 run 绑到单一会话会让并行分支无处落脚；完全不绑则 UI 无法定位；模板绑会话则模板不可复用 | 中：归属列进台账后改需迁移 |
| **D13** | 坐标与语义 | 模板语义字段与 `layout` **分列两列**；坐标不参与校验、不参与调度 | 坐标混进语义会让「挪一下卡片」触发版本变更与校验失败 | 低 |
| **D14** | 工作区与冲突域（Q3） | run **不持有工作区**；冲突域判据 = 各节点所属会话的 `resolve_workspace` 结果；同域写节点不得并行 | 让 run 持有工作区会与 P12 的唯一解析结果产生第二个真相 | 中：判据进调度器后改需重算 |
| **D15** | 节点级续跑（Q5） | **不支持**。run 中断后只能整体重跑（新 `run_id`）；台账不存节点输入快照 | 存快照等于把模板参数复制一份进 run，且「从中间续跑」的语义（已产生的副作用怎么办）无解 | 中：O6 由 `orphaned` 收口满足 |
| **D16** | 与 L3 / Router 的关系（Q6） | 本阶段不涉及；run 不承担跨会话路由 | 跨会话路由需要「哪个会话接哪一步」的策略，属 P8.3 | 低 |
| **D17** | 工具调用是否成节点 | **否**。工具调用在 turn 内，无独立生命周期；做成节点会产生第二条执行路径 | 与 O5 直接冲突；且工具节点无法表达「5 阶段」中的审批与落盘 | 中：后加需先解决执行路径归属 |
| **D18** | `group` 的形态 | `group` **不是节点类别**，而是模板级分组（`groups` + `node.group_id`）；不参与调度、不产生 node_run | 把容器做成节点会让就绪集合、台账、取消树三处都要处理「无执行节点」的空转 | 低 |
| **D19** | 环的报法与检测顺序 | **自环算环**（按该边单独报）；长环**逐回边报**，每条带 `edge_id`，使整环可高亮；**悬空边检查先于环检测**，环检测跳过未知端点 | 只报一条「存在环」无法定位；不先查悬空边会把同一缺陷报成两类错误；参照 Dify 的 Kahn 剥环 + 逐回边定位 | 低：只影响 `ValidateError` 的产出数量 |
| **D20** | 分组成员的校验 | `groups` 引用的节点必须存在；**空分组、分组 `id` 或 `title` 重复、同一节点属多个分组** 一律在保存期拒 | 分组只进编辑器呈现，若允许悬空成员，画布会画出指向不存在节点的框，且错误无法归因 | 低 |
| **D21** | `hitl` 源的边条件 | `hitl` 源节点**不允许 `always`**，只能用 `onSuccess` / `onFailure`；`turn` 源节点三种全允许 | `always` 会让被 reject 的 gate 仍顺流推进，等于让 gate 失去阻断义（D9）；参照 Dify 分支节点不给默认出口 | 中：进校验后放宽需改规则与用例 |
| **D22** | 模板 schema 跨版本加载 | `version` 与当前 schema 版本**不等即拒绝加载**并给出明确错误；**不降级、不迁移**；迁移策略留给引入 v2 的那个波次 | 静默降级会让「按 v1 解析 v2 字段」产生错误语义而不自知；现在就做迁移等于为零个历史模板维护迁移器 | 中：v2 引入时需补迁移或补拒绝提示 |

---

## 5 结构与依赖

### 5.1 crate 与模块

```
src-tauri/crates/assistant-orchestrator/src/
  lib.rs        // 对外 API：Orchestrator / RunHost / RunLedger / OrchestratorError
  model.rs      // WorkflowTemplate / WorkflowNode / WorkflowEdge / WorkflowLayout / NodeKind / EdgeCondition / TurnNodeParams
  validate.rs   // 保存期校验（纯函数：Template -> Result<(), Vec<ValidateError>>）
  schedule.rs   // 就绪集合 / 激活 / 冲突域（纯函数，唯一失效点）
  run.rs        // run 与 node_run 状态机（迁移表 + 非法迁移即 panic 于测试）
  ledger.rs     // RunLedger trait + MemoryLedger（内存版）+ StorageLedger（落库版）
  port.rs       // RunHost / TurnTap / LaunchTurnSpec / StartedTurn / LaunchError
  event.rs      // RunEvent 与广播端口（骨架与宿主接线分离，接线时机见 I69）
```

`assistant-storage/src/orchestration.rs`：三张表的 CRUD 与迁移。

**命名纪律**：`model.rs` 的类型名不得含与 turn 模板相关的词；turn 模板常量仍只在 `src/stores/agent/observation/template.ts` 与内核侧，编排 crate 不得引用（INV6）。

### 5.2 依赖方向

```
tauri 宿主（src-tauri）
  ├── assistant-orchestrator ──→ assistant-storage（落库后）
  │                          └─→ （仅通用库：serde / serde_json / thiserror / tracing / async-trait / tokio（事件广播与 gate 等待））
  ├── assistant-agent（内核，编排层不依赖）
  └── assistant-tools（工具，编排层不依赖）
```

**为什么必须切断对 `assistant-agent` 的依赖**：INV1 / INV2 是「不得成为第二个发射者 / 不得自建执行路径」。靠代码评审守不住，靠依赖图能守住——不依赖内核 crate，就物理上无法构造 `TurnSpec` 或 `ToolPipeline`。这是本阶段最强的一条结构性约束，也让 `cargo tree` 成为可自动化的门禁。

### 5.3 为什么不放进 `assistant-agent`

| 理由 | 具体后果 |
|---|---|
| 内核零编排概念 | 内核是「一次 turn 怎么跑」，加入 run 台账会让内核持有业务数据 |
| 内核零 `tauri` | 内核要能被测试与复用，不能反向依赖宿主 |
| 观测端口唯一 | `TurnObserver` 只有一个（N3）；编排若进内核，就必须给观测端口开第二个出口 |

### 5.4 宿主侧改动（最小面）

| 改动 | 位置 | 说明 |
|---|---|---|
| `start_turn` 回传 `StartedTurn` | `src-tauri/src/cmd/agent/mod.rs` | 现状返回 `()`，`tasks.begin` 生成的 `turn_id` 被丢弃；改为返回 `{ turn_id, assistant_message_id, turn_index }`。既有调用方忽略返回值即可，零行为变化 |
| sink 组合 tap | 同上 | `TauriEventSink` 外包一层 `TappedSink`（转发 + 通知 tap）。**不改变 `EventSink` trait**，tap 失败只记日志 |
| `cancel_conversation` | 同上 | 复用 `TurnTasks::cancel`，不新增取消路径 |

---

## 6 模板语义结构 v1 与保存期校验

### 6.1 类型

```rust
pub struct WorkflowTemplate {
    pub id: String,
    pub name: String,
    pub version: i64,              // schema 版本，非用户可见版本号（§15 OQ8）
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
    pub groups: Vec<WorkflowGroup>,
    pub layout: WorkflowLayout,
}

pub struct WorkflowNode {
    pub id: String,
    pub kind: NodeKind,
    pub title: String,
    pub params: Option<TurnNodeParams>,   // hitl 为 None
    pub group_id: Option<String>,
}

pub enum NodeKind { Turn, Hitl }          // 编译期闭集，不做运行时注册

pub struct TurnNodeParams {
    pub content: String,                  // 必填、常量
    pub mode_id: Option<String>,
    pub provider_id: Option<String>,      // None → 取会话当前值
    pub model_id: Option<String>,         // None → 取会话当前值
}

pub struct WorkflowEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub condition: EdgeCondition,
}

pub enum EdgeCondition { Always, OnSuccess, OnFailure }   // 闭集三值

pub struct WorkflowGroup { pub id: String, pub title: String, pub node_ids: Vec<String> }

pub struct WorkflowLayout { pub positions: HashMap<String, Position> }   // 只呈现，不参与校验与调度
```

### 6.2 结构纪律

| # | 约束 | 依据 |
|---|---|---|
| 1 | **节点类别是编译期闭集** `turn` / `hitl`；`group` 不在枚举内，而是模板级分组（`groups` + `node.group_id`） | INV5 / D18 |
| 2 | **语义与坐标分列**：`nodes` / `edges` / `groups` 参与校验与调度；`layout` 两者都不参与 | INV13 / D13 |
| 3 | **参数只接受常量**，不接受对上游输出的引用 | D11 |
| 4 | **模板不绑会话**：节点与 `conversation_id` 的绑定发生在发起 run 时，写入台账快照，不回写模板 | D12 |
| 5 | **双模板分离**：`WorkflowTemplate` 不得与 turn 模板（12 节点常量）共用类型名或互相引用 | INV6 |
| 6 | **静态图**：模板只能在保存期整体替换，运行时无增删节点 API | INV7 |
| 7 | `version` 只表示 schema 版本；用户可见版本号不在 v1 引入 | OQ8 |
| 8 | **版本相等才加载**：`version` 与当前 schema 版本不等 → 拒绝加载并报明确错误，不降级、不迁移 | D22 |

### 6.3 保存期校验

校验是纯函数：`validate(&WorkflowTemplate) -> Result<(), Vec<ValidateError>>`，只在保存时调用，不进入运行期。

```rust
pub struct ValidateError {
    pub kind: ValidateErrorKind,
    pub node_id: Option<String>,     // 定位到节点时填
    pub edge_id: Option<String>,     // 定位到边时填
    pub group_id: Option<String>,    // 定位到分组时填
    pub message: String,             // 人类可读，仅供展示，不作判据
}
```

| 序 | 非法类 | 判据 | 定位字段 |
|---|---|---|---|
| 1 | **重复 id** | 节点 `id` 或边 `id` 在同类内重复 → 拒 | 重复的 `node_id` / `edge_id` |
| 2 | **悬空边** | `from` 或 `to` 不指向任何已存在的 `node_id` → 拒 | `edge_id` |
| 3 | **环（含自环）** | 由 `edges` 构成的有向图存在回路 → 拒；图保持 DAG。自环按该边单独报，长环**逐回边报**，每条各带 `edge_id` | 回路的每条回边 `edge_id` |
| 4 | **缺必填参数** | `turn` 节点 `params.content` 缺失或为空 → 拒 | `node_id` |
| 5 | **未知类别** | `kind` 不在闭集 `turn` / `hitl` → 拒，**不得降级为默认态** | `node_id` |
| 6 | **条件与类别不匹配** | 边的 `condition` 与源节点类别不相容（相容表见下）→ 拒 | `edge_id` |
| 7 | **分组引用不存在节点** | `groups[].node_ids` 含不存在的 `node_id` → 拒 | `node_id`（悬空的那个） |
| 8 | **空分组** | `groups[].node_ids` 为空 → 拒 | `group_id` |
| 9 | **分组 id / title 重复** | 两个分组的 `id` 或 `title` 相同 → 拒 | `group_id`（两个分组各报一条） |
| 10 | **节点属多个分组** | 同一 `node_id` 出现在两个分组的 `node_ids` → 拒 | `node_id` |

**检测顺序**（D19 / D20）：`1 → 2 → 3` 是结构层，必须先于其余项 —— id 重复会让「节点是否存在」有歧义，悬空边会让环检测读到不存在的端点（环检测**跳过**未知端点，避免同一缺陷报成两类）。第 4–6 项按节点 / 边逐条查，第 7–10 项最后查分组。`node_id` / `edge_id` / `group_id` **至少一个非空**，保证编辑器总有可高亮的位置。

**第 6 类的相容表**（D21）：

| 源节点类别 | `always` | `onSuccess` | `onFailure` |
|---|---|---|---|
| `turn` | 允许 | 允许 | 允许 |
| `hitl` | **拒** | 允许 | 允许 |

> `hitl` 不给 `always`：`always` 表示「任意终态继续」，会让被 reject 的 gate 顺流推进，等于取消 gate 的阻断义（D9）。gate 必须由 `onSuccess` / `onFailure` 显式表达结果走向。

| 约束 | 说明 |
|---|---|
| 全部在保存期拒绝 | 非法图不产生 run，不进入运行期（F8 / INV7） |
| 错误必须定位 | 每个错误至少带 `node_id` / `edge_id` / `group_id` 之一，供编辑器高亮 |
| 错误可累积 | 一次保存返回**全部**命中项（`Vec<ValidateError>`），不首错即停，便于编辑器一次标全 |
| 类别闭集 | `ValidateErrorKind` 为 10 值闭集，与上表十类一一对应，不增不减；新增非法类必须改本表 |
| 坐标零影响 | 只改 `layout` 不得改变任何校验结果，也不得触发 `version` 变化（INV13） |
| 校验不看会话与运行态 | 校验输入只有模板本身，不读台账、不读 `TurnTasks` |

### 6.4 模块落点

| 文件 | 职责 |
|---|---|
| `model.rs` | §6.1 全部类型；不引用 `assistant-agent` / `assistant-tools` / `tauri` |
| `validate.rs` | §6.3 纯函数校验与 `ValidateError`；不读存储、不读运行态 |
| `schedule.rs` | **不读** `layout`；只读 `nodes` / `edges`（§9.1） |

### 6.5 与 wire 的关系

| 项 | 规则 |
|---|---|
| 本文产出 | 只有 crate 内模型与校验，**无 DTO、无命令、无事件**（INV12：冻结前零契约 diff） |
| wire 形态 | 模板 DTO 在 R4 一次性出齐，见 [I69b §2.1](./I69b-agent-orchestration-contract-spec.md) |
| 版本与快照 | 模板 `version` 的语义、run 对模板内容的引用方式属台账结构，见 §8.1；版本不等即拒绝加载（§6.2 第 8 条 / D22），发起 run 时同样适用 |
| 保存语义 | `save_workflow_template` 按 `id` 是否已存在决定行为：**已存在 → 整模板替换；不存在 → 创建**（`workflow_templates` 的 `id` 是主键，无 upsert 之外的语义）。不新增命令、不改 wire 形态（INV12）；删除模板无命令（§1.2 第 16 条） |
| 下游承诺 | 模板 schema 是画布模板编辑器（W3，[I69c](./I69c-agent-orchestration-canvas-spec.md)）的输入契约；schema 变更只在本波内发生，之后走新波次（[I69 §2](./I69-agent-orchestration-roadmap.md) C8） |

---

## 7 控制端口

### 7.1 `RunHost`（编排层 → 宿主）

```rust
#[async_trait::async_trait]
pub trait RunHost: Send + Sync {
    /// 发起一次 turn；实现内部调用 `start_turn`（唯一发射点）。
    /// 异步：`start_turn` 本身是 `async fn`，禁止在宿主实现里 `block_on`。
    async fn launch_turn(&self, spec: LaunchTurnSpec) -> Result<StartedTurn, LaunchError>;
    /// 按会话取消；语义与 `cancel_agent` 一致（`TurnTasks::cancel`）。同步：只置位令牌。
    fn cancel_conversation(&self, conversation_id: &str);
    /// 是否存在活跃 turn；实现即 `TurnTasks::is_busy`（§8.4 的 `orphaned` 判据）。
    fn is_conversation_busy(&self, conversation_id: &str) -> bool;
    /// 单调时钟（毫秒）；run 墙钟预算只用它，不用系统时间。
    fn now_ms(&self) -> i64;
    /// 会话的唯一工作区解析结果；用于冲突域判定（D14）。无工作区返回 `None`。
    fn workspace_root(&self, conversation_id: &str) -> Option<String>;
}

pub struct LaunchTurnSpec {
    pub conversation_id: String,
    pub content: String,                 // 已由模板参数填好，非空
    pub mode_id: Option<String>,
    pub provider_id: Option<String>,     // None → 宿主取会话当前值
    pub model_id: Option<String>,        // None → 宿主取会话当前值
    pub tap: Arc<dyn TurnTap>,           // 只读终态回调
}

pub struct StartedTurn {
    pub turn_id: String,
    pub assistant_message_id: String,
    pub turn_index: i64,
}

pub enum LaunchError {
    SessionBusy,      // 对应 `CommandErrorCodeDto::SessionBusy`
    InvalidInput,     // 空内容 / 参数不合法
    Storage,          // 建消息失败
}
```

**约束**：`LaunchTurnSpec` 只带纯数据（字符串与 tap），**不带任何 `assistant-protocol` DTO**。宿主负责把它映射成 `SendChatMessageArgsDto` 并解析缺省 provider / model。这样编排 crate 与 wire 解耦（INV12 的前提）。

**时钟实现约束**：`now_ms` 必须是**单调**毫秒（以 `Instant` 为基准），**不得**复用 `src-tauri/src/adapters/system_time_source.rs` 的 `now_millis`——该实现取 `chrono::Utc::now()`，是系统时间而非单调时钟；系统时钟被回拨或经 NTP 校正时，run 墙钟预算会倒退或瞬间超限，与 §8.4「禁止用时间戳判定活跃」同一条纪律。宿主侧须另建单调基准（进程启动 `Instant` + 已过毫秒），`now_ms` 是台账时间列与 run 事件 `at` 的唯一时间来源。

### 7.2 `TurnTap`（宿主 → 编排层，只读）

```rust
pub trait TurnTap: Send + Sync {
    fn on_terminated(&self, turn_id: &str, status: TurnStatusTap, error: Option<&str>);
}

pub enum TurnStatusTap { Completed, Cancelled, Failed, Interrupted }
```

| 约束 | 理由 |
|---|---|
| 只有 `on_terminated`，**不消费节点级事件** | 节点级事实属 L1 观测面（INV3）；run 只需终态 |
| 返回 `()`，实现不得阻塞、不得 panic 传播 | 与 `EventSink::emit` 同一条纪律（N2）；宿主侧实现须 `catch_unwind` 或直接吞掉异常 |
| `TurnStatusTap` 在编排 crate 内独立定义 | 不依赖 `assistant-agent`（INV1）；四值与内核 `TurnStatus` 1:1，映射在宿主 |

### 7.3 gate 端口

gate 解决**不经** `RunHost`，而是编排层自己的命令入口：

```
resolve_workflow_gate(run_id, node_id, decision: { proceed: bool, note: Option<String> })
```

宿主把它转成 `Orchestrator::resolve_gate(...)`。**禁止**把 gate 复用成 `resolve_agent_question`：后者属于 `ask_user` 工具，回填会进对话历史（与 D9 冲突）。

### 7.4 禁止事项

| # | 禁止 | 依据 |
|---|---|---|
| 1 | 编排层调用 `EventSink::emit` | INV3 |
| 2 | 编排层构造 `TurnSpec` / `TurnRunner` / `ToolPipeline` | INV1 / INV2 |
| 3 | 编排层持有 `ToolRegistry` 或读工具清单做可用性判断 | INV2；可用集仍由 `resolve_available_tools` 在发送时算 |
| 4 | 编排层读 `messages` 表反推 turn 是否完成 | INV8；终态只来自 tap |
| 5 | 编排层按时间戳判定 turn 是否还活着 | INV8；活跃判定只来自 `TurnTasks`（经宿主） |
| 6 | 用系统时间（`chrono::Utc::now()`）充当 `now_ms` 或写台账时间列 | §7.1 时钟实现约束；混入系统时间会让 run 墙钟预算随系统时钟回拨而倒退 |

---

## 8 run 台账与持久化

### 8.1 表结构

```sql
CREATE TABLE IF NOT EXISTS workflow_templates (
  id           TEXT PRIMARY KEY,
  name         TEXT NOT NULL,
  version      INTEGER NOT NULL,              -- schema 版本，非用户可见版本号
  spec_json    TEXT NOT NULL,                 -- 语义：nodes / edges / groups
  layout_json  TEXT NOT NULL DEFAULT '{}',    -- 呈现：坐标，不参与校验（INV13）
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS workflow_runs (
  id                      TEXT PRIMARY KEY,
  template_id             TEXT NOT NULL,
  template_version        INTEGER NOT NULL,   -- 发起时的 schema 版本，模板后续改动不影响已发起 run
  origin_conversation_id  TEXT NOT NULL,      -- UI 定位锚点（D12）
  client_run_key          TEXT,               -- 幂等键，见 §8.3
  state                   TEXT NOT NULL,
  reason                  TEXT,
  started_at              INTEGER NOT NULL,
  updated_at              INTEGER NOT NULL,
  finished_at             INTEGER,
  UNIQUE (template_id, origin_conversation_id, client_run_key)
);

CREATE TABLE IF NOT EXISTS workflow_node_runs (
  run_id                 TEXT NOT NULL,
  node_id                TEXT NOT NULL,
  kind                   TEXT NOT NULL,       -- turn / hitl
  conversation_id        TEXT,                -- hitl 为 NULL
  turn_id                TEXT,                -- hitl 为 NULL
  assistant_message_id   TEXT,                -- 结果引用，见 §8.5
  state                  TEXT NOT NULL,
  attempt                INTEGER NOT NULL DEFAULT 1,
  note                   TEXT,                -- hitl 的确认备注（D9）
  reason                 TEXT,
  started_at             INTEGER,
  finished_at            INTEGER,
  PRIMARY KEY (run_id, node_id)
);

CREATE INDEX IF NOT EXISTS idx_node_runs_turn ON workflow_node_runs(turn_id);
CREATE INDEX IF NOT EXISTS idx_runs_origin ON workflow_runs(origin_conversation_id, started_at DESC);
```

**`conversation_id` 的来源**：模板不绑会话（D12）；发起时由 `WorkflowRunStartInputDto.nodeConversations` 解析每个节点（缺省 = `originConversationId`），解析结果写入 `workflow_node_runs.conversation_id` 作为 run 内快照。模板后续被编辑不影响已发起 run。

**时间列的语义**：三张表的时间列（`created_at` / `updated_at` / `started_at` / `finished_at`）一律由宿主 `now_ms()` 写入（单调毫秒，§7.1 时钟实现约束、§9.6 时间口径一致），**不是 Unix 时间戳**；写入方不得改用 `chrono::Utc::now()`，读取侧不得把这些值当墙钟时间展示或与系统时间相减。

**`attempt` 的含义**：v1 无重试（D15），`attempt` 恒为 `1`；保留该列是为了让「未来若引入重试」不必改表结构，读取侧不得据 `attempt > 1` 推断任何行为。

**`turn_id` 的语义**：内核 `TurnTasks::begin` 生成的 `turn_id` 是 `"{conversation_id}:{counter}"` 形式的**进程内**标识，进程重启后计数归零、同一字符串会复用。因此：

- `workflow_node_runs.turn_id` **只用于进程内把 tap 回调对回节点**，不作为审计依据；
- 审计与 token 归因一律走 `assistant_message_id`（数据库主键、跨重启稳定）；
- 重启后旧行的 `turn_id` 允许变成「指向不存在的东西」，读取侧不校验（§8.5）。

### 8.2 状态闭集与迁移

**`RunState`（7 值）**

| 状态 | 含义 | 迁移到 |
|---|---|---|
| `pending` | 已落库、尚未启动 | `running`、`cancelled` |
| `running` | 至少一个节点在跑或可推进 | `waiting`、`finished`、`failed`、`cancelled`、`orphaned` |
| `waiting` | 无节点可推进，且存在 `hitl` 等待 | `running`、`failed`、`cancelled`、`orphaned` |
| `finished` | 汇点全部终态且无 `failed` 节点 | 终态 |
| `failed` | 汇点全部终态且存在 `failed` 节点 | 终态 |
| `cancelled` | 用户取消 | 终态 |
| `orphaned` | 启动扫描发现「台账在跑但无活跃 turn」 | 终态 |

**`NodeRunState`（7 值）**

| 状态 | 含义 | 迁移到 |
|---|---|---|
| `pending` | 未激活 | `running`（`turn`）· `waiting`（`hitl`）· `skipped` |
| `running` | `turn` 节点在跑（`hitl` 不进入此态） | `finished`、`failed`、`cancelled` |
| `waiting` | `hitl` 节点等人（无超时，§9.5） | `finished`（proceed）· `failed`（reject）· `cancelled` |
| `finished` | `turn` 的 turn `completed`；或 `hitl` 被 proceed | 终态 |
| `failed` | turn `failed` / `interrupted`、发起被拒、或 `hitl` 被 reject | 终态 |
| `skipped` | **调度阻断**：入边全部未激活（与 L1 的「报告未执行」不同义） | 终态 |
| `cancelled` | run 取消传播到此节点 | 终态 |

**终态集**：`finished` / `failed` / `skipped` / `cancelled`。终态不得再迁移；非法迁移在测试构建下 panic，生产构建记 `error` 并保持原状态（不静默改写台账）。

### 8.3 键与幂等

| 对象 | 键 | 说明 |
|---|---|---|
| run | `run_id`（宿主生成 UUID） | 不复用 `turn_id`（N7 已说明 `turn_id` 不可作持久主键） |
| 节点 | `(run_id, node_id)` | 即并发键（D6） |
| 事件路由 | `(run_id, node_id)` | 与台账同粒度（O12） |
| 幂等发起 | `(template_id, origin_conversation_id, client_run_key)` 唯一 | 第二次发起返回既有 run，不产生新 run、不双执行（O6） |

`client_run_key` 由前端在「点一次运行」时生成；为 `NULL` 时不参与唯一约束（允许同一模板并发多 run）。

### 8.4 `orphaned` 收口

启动时扫描 `workflow_runs.state IN ('running','waiting')`：

1. 对每一行，收集其 `workflow_node_runs.state = 'running'` 的 `conversation_id`；
2. 经宿主 `is_conversation_busy(conversation_id)`（实现即 `TurnTasks::is_busy`：存在当前代际且令牌未取消）判定是否真有活跃 turn；
3. 全部不活跃 → run 置 `orphaned`、在跑节点置 `failed(reason: orphaned)`；
4. 任一仍活跃 → **不改**该 run（可能是同进程重启前的残留窗口，交给下一轮扫描或 `reconcile`）。

**禁止用时间戳判定活跃**（§7.4 第 5 条）。扫描只在启动时跑一次，不做周期轮询（周期轮询需要额外线程与锁，收益低于 `reconcile`）。

### 8.5 引用列不加外键

`assistant_message_id` 与 `turn_id` **只存值、不加外键约束**。

理由：会话删除会级联删除 `messages`；若加外键，删会话会被 run 台账阻塞（或反过来级联删掉审计事实）。读取时容忍悬空引用，显示为「结果已删除」，台账行保留。

### 8.6 不落原始事件

事件是广播、可丢（N2）。落原始事件会让存储随 run 长度线性增长，而审计只需要「哪个节点、什么终态、对应哪个 turn」。节点级细节已在 L1 观测面与消息行里，不重复存（D5）。

### 8.7 `reconcile` 校正表

`reconcile_workflow_run` 是「事件丢了之后把台账拉回事实」的唯一入口（A26）。它**只读**三处事实：台账、`TurnTasks`（经 `RunHost::is_conversation_busy`）、`messages.status`（闭集 `streaming` / `completed` / `interrupted` / `failed`）。

| 台账 `node_run.state` | 会话忙 | `messages.status` | 校正为 |
|---|---|---|---|
| `running` | 是 | 任意 | **不动**（turn 确实在跑） |
| `running` | 否 | `completed` | `finished` |
| `running` | 否 | `failed` | `failed(reason: message_failed)` |
| `running` | 否 | `interrupted` | `failed(reason: interrupted)` |
| `running` | 否 | `streaming` | `failed(reason: orphaned)` |
| `waiting`（`hitl`） | — | — | **不动**（人工等待无超时，§9.5） |
| `pending` / 终态 | — | — | **不动** |

节点校正完成后按 §9.1 重算 run 状态：所有汇点终态且无 `failed` → `finished`；有 `failed` → `failed`；仍有可推进节点 → `running`；仅剩 `hitl` 等待 → `waiting`。

**禁止**：`reconcile` 不得发起新 turn、不得重放事件、不得把 `failed` 改回 `running`（只能前推，不能回退）。

---

## 9 调度语义

### 9.1 就绪集合与激活

| 概念 | 定义 |
|---|---|
| **入边** | 指向节点 `n` 的激活边集合 |
| **就绪** | `n.state = pending` 且 `n` 的所有入边都已判定（源节点为终态或源边条件为 `always` 时提前判定） |
| **激活** | 入边中至少一条的条件成立 → `n` 可推进；否则 → `skipped` |
| **汇点** | 无出边的节点；run 结束条件由汇点全部终态判定 |

**判定顺序**（唯一实现，`schedule.rs`）：

1. 收集所有 `pending` 节点；
2. 对每个 `pending` 节点，若存在未判定入边则跳过；
3. 入边判定完成后，按条件求值（§9.4）决定「激活」或「skipped」；
4. 激活节点进入待发起队列；
5. 从待发起队列中按「会话空闲 + 冲突域空闲 + 未超预算」筛出可发起者。

**就绪集合不落库**：它是纯派生量，落库会与节点状态双写（§2.2 第 6 条）。

### 9.2 串行与并行

| 约束 | 规则 |
|---|---|
| 会话互斥 | 同一 `conversation_id` 同时最多一个 `running` 节点（INV10）。`TurnTasks` 的 `OnBusy::Reject` 是兜底，不是主判据 |
| 并行条件 | 两个节点可并行，当且仅当：`conversation_id` 不同 **且** 冲突域不冲突（§9.3） |
| 并行上限 | run 级 `max_parallel`（默认 4，可配置）；超限时按节点 id 稳定排序取前 N，保证可复现 |
| 串行默认 | 模板只有一条链时天然串行，不需要单独模式 |

**为什么不做到「同会话并行」**：那需要第二套 turn 键与第二个 `TurnTasks` 语义，等于改内核（D6）。本阶段如实收缩：**并行 = 不同会话**。

### 9.3 冲突域

| 项 | 规则 |
|---|---|
| 域标识 | 节点所属会话的 `workspace_root`（经 `RunHost::workspace_root`） |
| 无工作区 | `None` 视为独立域（不与任何节点冲突） |
| 冲突判据 | 同域且两个节点都可能写（`turn` 节点默认视为可能写）→ 不得并行 |
| 执行期 | 冲突判定只在「发起前」做一次；运行中不重新求值（工作区变更不在 run 生命周期内生效） |

### 9.4 条件边与取消

**条件边闭集（`EdgeCondition`）**

| 值 | 成立条件 | 依据 |
|---|---|---|
| `always` | 源节点为任意终态（仅 `turn` 源可用，`hitl` 源在保存期即拒） | 无条件流转 |
| `onSuccess` | 源节点 `finished`（turn `completed`） | — |
| `onFailure` | 源节点 `failed`（turn `failed` / `interrupted`，或发起被拒） | — |

`cancelled` 与 `skipped` 的源节点**不激活任何下游**（除 `always` 外均不成立），保证取消不会顺流推进。

`hitl` 源节点不允许 `always`（保存期即拒，见 §6.3 相容表 / D21）：gate 的结果走向必须由 `onSuccess` / `onFailure` 显式表达。

**取消树**

1. `cancel_run(run_id)` → run 置 `cancelled`，记取消时间；
2. 对每个 `running` 节点：`RunHost::cancel_conversation(conversation_id)`；
3. 节点收到 `TurnStatusTap::Cancelled` → `cancelled`；若 turn 已 `completed` 则保留 `finished`（取消晚于完成不追改）；
4. `pending` / `waiting` 节点直接置 `cancelled`；
5. 重复取消：run 已是终态 → 直接返回，无副作用（INV9）。

### 9.5 HITL 节点

`hitl` 是**等待节点**，不是执行节点（INV5）：它不发起 turn、不产生 `turn_id`、不参与冲突域。

| 项 | 规则 |
|---|---|
| 进入 | 入边激活 → `waiting`；**不进入 `running`** |
| 解决通道 | `resolve_workflow_gate(run_id, node_id, decision)`，命令签名见 [I69b §2.1](./I69b-agent-orchestration-contract-spec.md) |
| `proceed: true` | 节点 `finished`；`note` 存入 `note` 列 |
| `proceed: false` | 节点 `failed(reason: rejected)`；`note` 存入 `note` 列 |
| 超时 | **无超时**（F11）；只有 cancel 或 resolve 能结束 |
| 回答去向 | **只进 run 台账**（`note` 列），**不进对话历史**（D9） |
| 重复 resolve | 节点已终态 → 返回既有结果，不二次改写 |
| run 状态影响 | 仅剩 `hitl` 等待时 run 置 `waiting`（§8.7） |

**禁止**复用 `resolve_agent_question`：后者属于 `ask_user` 工具、回填进对话历史，与 D9 冲突（§7.3）。

### 9.6 预算与归因

| 预算 | 口径 | 闸门 | 未配置时 |
|---|---|---|---|
| run 墙钟 | `now_ms() - started_at` | **是**：超限 → run `failed(reason: timeout)`，在跑节点取消 | 显示「未配置」 |
| 节点数上限 | 已发起过的节点数（含 `failed`，不含 `skipped`） | **是**：超限 → 后续激活节点不发起，run `failed(reason: node_limit)` | 显示「未配置」 |
| 并行上限 | 同时 `running` 节点数（§9.2） | 是（调度层） | 默认 4 |
| token | 读侧归因：按 `assistant_message_id` 关联用量 | **否**（只归因不闸门） | 显示「未配置」 |

**「未配置 ≠ 0」**（INV11）：`None` 表示无闸门，前端必须渲染为「未配置」，不得渲染 `0`，也不得以 `0` 表示无限。四者口径独立，不得用「一个总预算」折算（D8）。

**预算配置来源**：四项均由 `WorkflowRunStartInputDto.budgets` 传入（[I69b §2.2](./I69b-agent-orchestration-contract-spec.md)），**不读全局配置、不设隐式默认值**（`maxParallel` 除外，缺省 4）。`null` 一律解释为「未配置」。

**时间口径一致**：`started_at` / `updated_at` / `finished_at` 与事件 `at` **同源于 `now_ms()`**（单调毫秒，§7.1 时钟实现约束），不得混入 `chrono::Utc::now()`；否则 `now_ms() - started_at` 会变成两个不同基准相减。这些列是**进程内相对时间**，不作墙钟显示；跨进程的旧行时间值不参与任何预算计算（run 生命周期止于进程，§8.4 的 `orphaned` 收口，D15）。

token 不做实时闸门的原因：实时闸门需要内核回传 turn 级用量，属内核契约变更（OQ7）；本阶段只做读侧归因。

---

## 10 失败矩阵

| # | 情形 | 编排层行为 | 内核 / 台账 |
|---|---|---|---|
| **F1** | `launch_turn` 返回 `SessionBusy` | 节点 `failed(reason: busy)`；run 按边条件推进（不自动重试——重试属图语义，v1 无环） | turn 未创建；台账已写节点行 |
| **F2** | turn 终态 `failed` | 节点 `failed`；下游按 `onFailure` 决定 | turn 已落消息行与终态 |
| **F3** | turn 终态 `interrupted` | 同 `failed`（归入 `onFailure`） | 同上 |
| **F4** | turn 终态 `cancelled` | 节点 `cancelled`；不激活下游 | 同上 |
| **F5** | tap 丢事件（run 停在 `running`） | **不主动收口**；只降级为「停在最后已知节点」 | turn 正常终态 |
| **F6** | 编排层 panic | 进程内其他组件不受影响；重启后由 `orphaned` 扫描收口 | turn 正常终态 |
| **F7** | 台账写失败 | 节点结果不丢（turn 已落消息行）；run 置 `failed(reason: ledger)` 并记 `error` 日志 | **不回滚** turn |
| **F8** | 非法模板 | 保存期拒绝，不产生 run | 无 |
| **F9** | 重复发起（同 `client_run_key`） | 返回既有 run | 不双执行 |
| **F10** | 未知节点类别（DB 被手改 / 旧模板 + 新代码） | 节点 `failed(reason: unknown_kind)`；不渲染为默认态（O11） | 台账保留原值 |
| **F11** | gate 永不解决 | run 停在 `waiting`，无超时；只有 cancel / resolve 结束 | 无 |

**总纪律**：F1–F11 中任何一条都**不得**改变 turn 的终态，也不得让 `start_turn` 之外的路径产生 turn（INV4）。

---

## 11 缺口落点

本表只登记「缺口 → 约束落点」；**处置方式与所属波次以 [I69 §3.1](./I69-agent-orchestration-roadmap.md) 为唯一来源**。

| 缺口（分析 §6） | 落点 | 说明 |
|---|---|---|
| G1 run 级任务键与取消树 | §9.1 / §9.4 | `(run_id, node_id)` + 取消树；`TurnTasks` 键策略不动 |
| G2 run / node_run 持久化 | §8.1 | 三张表；edge 属模板 `spec_json`，不单独建表 |
| G3 编排控制端口定义 | §7.1 / §7.2 | — |
| G4 run 级事件归属键 | §8.1 / [I69b §1.1](./I69b-agent-orchestration-contract-spec.md) | 台账主键 + 事件键 |
| G5 并行调度与冲突域 | §9.2 / §9.3 | — |
| G6 模板 → turn 请求编译层 | §6.1 / §7.1 | `TurnNodeParams` → `LaunchTurnSpec`；宿主补缺省值 |
| G7 画布依赖与图数据模型 | §6.1（后端一半） | 图数据模型由 §6 定义；画布为自研设施（`src/features/canvas/`，无第三方图库），W1 / W2 已交付（I66 / I68，已归档）；模板编辑器见 [I69c](./I69c-agent-orchestration-canvas-spec.md) |
| G8 图级授权单元 | 不做（D7） | 保持调用级审批 |
| G9 run 级预算与归因 | §9.6 | — |
| G10 多 run 并行的事件路由 | [I69b §1.1](./I69b-agent-orchestration-contract-spec.md) / [I69b §1.2](./I69b-agent-orchestration-contract-spec.md) | 后端按 `(runId, nodeId)` 键控；前端按同一键做投影 |
| G11 图级 HITL 节点 | §9.5 | — |

---

## 12 与既有阶段的关系

| 阶段 | 关系 |
|---|---|
| P4（内核） | 复用 `start_turn` / `TurnTasks` / `TurnEvent::Terminated`；**不改内核语义** |
| P5（工具） | 编排层不接触 `ToolPipeline`；可用工具集仍由发送时的 `resolve_available_tools` 决定 |
| P12（工作区） | 冲突域取 `resolve_workspace` 的唯一解析结果（D14） |
| P15（观测） | 订阅 `agent-node-event` 做只读投影；不改 `EventSink`（INV3） |
| P16 / P17 | 无关系 |
| P8.1（`spawn_agent`） | 暂缓；v1 无委派类别（D10） |
| P8.2 / P8.3（L3） | 分层不同，互不前置（D1 / D16） |
| P9（后台任务） | run 生命周期止于应用进程；跨重启续跑不在本阶段（D15） |
| 画布 W1 / W2 | 可并行；W1 / W2 不读模板与台账 |
| 前端编排页占位 | **已存在**（`src/features/orchestration/`，路由 `/agent-orchestration`）：画布宿主 + 放置栏 + 预设标识，数据全为前端示例，不读模板与台账；编排已从观测页移除，观测页只做只读观测。R5 在此收敛为接缝，占位不接后端 |
| 画布 W3（模板编辑器） | 读取模板 schema 与模板命令；形态与验收见 [I69c](./I69c-agent-orchestration-canvas-spec.md) |
| 画布 W4（运行态叠加） | 读取 run 台账与 `workflow-run-event`；只读叠加，前端卸载不影响 run（[I69b §3.2](./I69b-agent-orchestration-contract-spec.md) K12） |

---

## 13 验收（A1–A24、A26、A28–A32）

| # | 判据 | 检查方式 |
|---|---|---|
| **A1** | 两个 `turn` 节点的串行 run 跑完，节点状态与 run 状态正确，`assistant_message_id` 指向真实消息行 | crate 集成测试（假 `RunHost` + 真台账） |
| **A2** | 全仓 turn 发起点仍只有命令层一处；编排层不构造 `TurnSpec` | `cargo tree` + grep `TurnSpec::agent()` |
| **A3** | `cargo tree -p assistant-orchestrator` 不含 `tauri` / `assistant-agent` / `assistant-tools` | 命令 |
| **A4** | 宿主返回 `SessionBusy` 时节点 `failed(busy)`、run 不挂死 | 单测（F1） |
| **A5** | tap 内 panic 时 turn 仍正常终态 | 单测（F6 / INV4） |
| **A6** | 零 wire diff（`git diff --stat src/types src/vault/core/contract` 为空） | 命令（INV12） |
| **A7** | §6.3 十类非法模板（重复 id / 悬空边 / 环 / 缺必填参数 / 未知类别 / 条件与类别不匹配 / 分组引用不存在节点 / 空分组 / 分组 id 或 title 重复 / 节点属多个分组）各被拒，并定位到 `node_id` / `edge_id` / `group_id` | 校验单测（F8） |
| **A8** | 合法模板通过校验并编译出正确的 `LaunchTurnSpec` 序列 | 单测 |
| **A9** | 环在保存期被拒，不进入运行期 | 校验单测（INV7） |
| **A10** | 只改 `layout` 不改变任何校验结果、不改变 `spec_json` | 单测（INV13） |
| **A11** | 未知节点类别被拒，不降级为默认态 | 单测（O11） |
| **A12** | 进程重启后 run / node_run 可读且状态与终止前一致 | 集成测试（真 DB） |
| **A13** | 同 `client_run_key` 重复发起返回既有 run，不双执行 | 集成测试（O6） |
| **A14** | 模拟进程被杀后启动扫描把在跑 run 置 `orphaned`、节点置 `failed(orphaned)` | 集成测试 |
| **A15** | 删除会话后台账行保留、引用显示「结果已删除」、无外键错误 | 集成测试（§8.5） |
| **A16** | 台账写失败时 run 置 `failed(ledger)`、turn 终态不变 | 单测（F7 / INV4） |
| **A17** | 两个不同会话的节点并行推进，墙钟区间明显重叠 | 集成测试 |
| **A18** | 同会话两节点不同时 `running`；`TurnTasks` 不出现 `SessionBusy` | 集成测试（INV10） |
| **A19** | `cancel_run` 使在跑节点收到取消；二次取消无副作用 | 单测（INV9） |
| **A20** | 条件边只激活一条下游，其余 `skipped`（调度阻断义） | 单测（§9.4） |
| **A21** | 同 `workspace_root` 的两个 `turn` 节点不并行 | 单测（§9.3） |
| **A22** | `hitl` 节点 proceed / reject 分别置 `finished` / `failed`，`note` 落台账、不进对话历史 | 集成测试（D9） |
| **A23** | run 墙钟超限 → `failed(timeout)` 且在跑节点被取消 | 单测（§9.6） |
| **A24** | 节点数超限 → 后续节点不发起，run `failed(node_limit)` | 单测（§9.6） |
| **A26** | 丢事件后 `reconcile` 把台账拉回事实；不发起新 turn、不回退状态 | 集成测试（§8.7） |
| **A28** | 同时 `running` 节点数不超过 `maxParallel`；超出的就绪节点排队而非并发，且不丢节点、不标失败 | 集成测试（§9.2 / §9.6） |
| **A29** | 重启后模板行可读，`spec` 与 `layout` 仍分列、`version` 不变，内容与保存时一致 | 集成测试（§8.1 / INV13） |
| **A30** | 自环按该边单独报一条错误；长环逐回边各报一条且每条带 `edge_id`；悬空边先于环检测，同一缺陷不被报成两类 | 校验单测（D19） |
| **A31** | `hitl` 源的 `always` 边被拒；`hitl` 的 `onSuccess` / `onFailure` 通过；`turn` 源三种条件全通过 | 校验单测（D21） |
| **A32** | 模板 `version` 与当前 schema 版本不等 → 读取与发起两处均拒绝并报明确错误，不降级、不迁移 | 集成测试（D22） |

> A25（多 run 事件不串台）与 A27（前端接缝）在 [I69b §4](./I69b-agent-orchestration-contract-spec.md)。

---

## 14 已拒绝方案

| # | 方案 | 拒绝理由 |
|---|---|---|
| RD1 | 把编排放进 `assistant-agent` | 内核要零编排概念、零 `tauri`；会迫使观测端口开第二个出口（§5.3） |
| RD2 | 前端编排（B 方案） | 断开前端无法推进（O10）；跨窗口不一致；应用最小化即停 |
| RD3 | 「编排即模式」（C 方案） | 无台账、无取消树；模式是运行配置，不是调度器 |
| RD4 | 复用 `TurnTasks` 作 run 键 | 互斥表不是队列，也不是 run 归属；会与 N7 直接冲突（D6） |
| RD5 | 节点结果缓存 + `replayable` | 假命中（分析 §4.2）；schema 一旦有该字段即为承诺（D3） |
| RD6 | 图级环 + 收敛判据 | 状态机膨胀为「节点 × run × iteration」，校验无法静态化（D2） |
| RD7 | 图级授权绕过 `Gateway` | 权限旁路，与 I52 冲突（D7） |
| RD8 | 复用 `ask_user` 作为图级 HITL | 回答进对话历史，污染上下文（D9） |
| RD9 | 把工具调用做成节点 | 产生第二条执行路径（D17 / O5） |
| RD10 | 变量池 / 参数引用上游输出 | 需要类型系统与求值器，等于把编排做成数据流引擎（D11） |
| RD11 | 原始事件落库 | 存储随 run 长度线性增长（D5） |
| RD12 | 用时间戳判定 turn 是否活着 | 时钟不可靠，会把正在跑的 run 误标 `orphaned`（§8.4） |

---

## 15 未决问题

| # | 问题 | 当前处置 | 何时裁定 |
|---|---|---|---|
| OQ1 | typed handoff（交接类别闭集） | v1 边只有激活语义；不引入类型校验 | 需要「节点输出能否接下游输入」时（分析 §12.2 #1） |
| OQ2 | `run` 与「应用关闭后继续跑」（P9） | run 生命周期止于进程；`orphaned` 收口 | P9 立项时 |
| OQ3 | run / 模板的归档与清理策略 | 本阶段不做；台账只增不删（删模板不删 run） | 存储压力出现时（可复用 P14 `ReclaimPolicy` 口径） |
| OQ4 | 会话内并行（同 `conversation_id` 多节点） | 不支持（D6） | 需要改 `TurnTasks` 键策略时，属内核契约变更 |
| OQ5 | 条件边扩展到「按输出内容路由」 | 不支持；闭集只有三值 | typed handoff（OQ1）定案后 |
| OQ6 | HITL 是否支持「多选项 + 表单」 | v1 只有 `proceed` + `note` | W3 编辑器定案时 |
| OQ7 | run 预算的 token 实时闸门 | 只归因不闸门（D8） | 内核回传 turn 级用量时 |
| OQ8 | 模板版本与用户可见版本号的关系 | `version` 只表示 schema 版本；用户版本号暂不引入 | W3 需要「历史版本」时 |
| OQ9 | 模板 schema 跨版本加载（v1 模板遇 v2 代码，或反向） | **已定案（D22）**：版本不等即拒绝加载并报明确错误，不降级、不迁移；迁移策略留给引入 v2 的波次 | 已关闭（R1 冻结前完成） |

> **关闭权**：OQ 只由本表关闭。下游卷（[I69b](./I69b-agent-orchestration-contract-spec.md) / [I69c](./I69c-agent-orchestration-canvas-spec.md)）可在自己的波次内**触发**裁定（如 OQ6 的多选项形态、OQ8 的历史版本），但必须回提本表，由本表同批次更新「当前处置 / 何时裁定」两列，下游不得自行关闭或另立编号。裁定后若改变约束，走 §4 决议新增，不就地改写本表既有行。

---

## 16 承接清单与冻结常量（附录）

> **用途**：本附录只解决一件事——**上游成果如何不丢地传到下游**。§16.1 规定下游必须逐条承接什么，§16.2 把散落在各卷的数字与闭集收敛成可核对的常量 ID。两表都不新增语义，值一律以正文为准。

### 16.1 下游承接清单

**维护纪律**：本表由上游维护。**上游新增或修改任何冻结项时，同一批次更新本表与对应下游卷的「上游承接清单」附录**，不得留到下一波。下游只引用 ID，不复述内容。

| 承接方 | 必须逐条承接的 ID | 落点 | 核对方式 |
|---|---|---|---|
| [I69b](./I69b-agent-orchestration-contract-spec.md) | `INV1` / `INV3` / `INV4` / `INV5` / `INV6` / `INV7` / `INV8` / `INV9` / `INV10` / `INV11` / `INV12` / `INV13` | §1.1 事件、§2 wire、K1–K12（§3.2） | 契约 diff + K 单测 |
| [I69b](./I69b-agent-orchestration-contract-spec.md) | `D5` / `D6` / `D8` / `D9` / `D12` / `D13` / `D14` / `D18` / `D22` | §2.1–§2.4 | 同上 |
| [I69b](./I69b-agent-orchestration-contract-spec.md) | `CONST-NODE-KIND-2` / `CONST-EDGE-COND-3` / `CONST-INVALID-GRAPH-10` / `CONST-RUN-STATE-7` / `CONST-NODERUN-STATE-7` / `CONST-CMD-10` / `CONST-EVENT-1` / `CONST-BUDGET-4` | §2 wire 与 §2.4 错误闭集 | 常量表逐项核对 |
| [I69c](./I69c-agent-orchestration-canvas-spec.md) | `INV5` / `INV7` / `INV13` | UI2 / UI4 / UI5 / UI6 | §6 判据 |
| [I69c](./I69c-agent-orchestration-canvas-spec.md) | `D3` / `D4` / `D9` / `D11` / `D13` / `D18` / `D20` / `D21` | UI2–UI9 | 同上 |
| [I69c](./I69c-agent-orchestration-canvas-spec.md) | `CONST-NODE-KIND-2` / `CONST-EDGE-COND-3` / `CONST-EDGE-COND-HITL-2` / `CONST-INVALID-GRAPH-10` / `CONST-NODERUN-STATE-7` / `CONST-KEY-2` / `CONST-CMD-10` / `CONST-EVENT-1` / `CONST-BUDGET-4` | §1.4 归属矩阵 / §2 / §4 / §6 | 同上 |
| [I69d](./I69d-agent-orchestration-card-spec.md) | `INV5` / `INV11` / `INV13` / `D3` / `D11` / `D13` / `D18` / `D21` / `D22` | §2 / §3 / §4 / §5 / §7（见其 §13 承接清单） | OC1–OC14 |

> **不承接即失效**：表中任一 ID 在下游卷找不到落点，视为**上游成果丢失**，该卷不得标记为可实施。

### 16.2 冻结常量表

> 下游引用一律写常量 ID，**禁止在正文再写阿拉伯数字或复述闭集元素**。值以「定义处」为准；本表与正文不符时改本表。

| 常量 ID | 值 | 定义处 | 引用者 |
|---|---|---|---|
| `CONST-INV-13` | 不变量 13 条（`INV1`–`INV13`） | §3 | I69b 头部 / I69c §0 |
| `CONST-DEC-22` | 决议 22 条（`D1`–`D22`） | §4 | I69b §2 / I69c §3–§4 |
| `CONST-NODE-KIND-2` | 节点类别闭集 2 值：`turn` / `hitl`（`group` 不是类别，见 `D18`） | §6.1 | I69b §2.2 / I69c UI2 |
| `CONST-EDGE-COND-3` | 边条件闭集 3 值：`always` / `onSuccess` / `onFailure` | §6.1、§9.4 | I69b §2.2 / I69c UI4 |
| `CONST-EDGE-COND-HITL-2` | `hitl` 源可用边条件 2 值：`onSuccess` / `onFailure`（无 `always`） | `D21`、§6.3 | I69c UI5 / A31 |
| `CONST-INVALID-GRAPH-10` | 保存期非法图 10 类：重复 id / 悬空边 / 环（含自环）/ 必填参数缺失 / 未知类别 / 条件与类别不匹配 / 分组引用不存在节点 / 空分组 / 分组 id 或 title 重复 / 节点属多个分组 | §6.3 | I69 §4.2 / I69b §2.4 / I69c UI5 / A7 / A9 / A11 |
| `CONST-RUN-STATE-7` | run 状态闭集 7 值：`pending` / `running` / `waiting` / `finished` / `failed` / `cancelled` / `orphaned` | §8.2 | I69b §2 |
| `CONST-NODERUN-STATE-7` | L2 节点状态闭集 7 值：`pending` / `running` / `waiting` / `finished` / `failed` / `skipped` / `cancelled`（`skipped` = 调度阻断） | §8.2 | I69b K5–K6 / I69c §4 |
| `CONST-KEY-2` | 并发键与事件路由键同粒度 `(run_id, node_id)` | §8.3、`D6` | I69b §1.1 / I69c §4 |
| `CONST-CMD-10` | 命令 10 条：`list_workflow_templates` / `get_workflow_template` / `save_workflow_template` / `start_workflow_run` / `cancel_workflow_run` / `get_workflow_run` / `list_workflow_runs` / `list_workflow_node_runs` / `resolve_workflow_gate` / `reconcile_workflow_run` | [I69b §2.1](./I69b-agent-orchestration-contract-spec.md) | I69 §4.5 / I69c 头部 |
| `CONST-EVENT-1` | run 事件 1 个：`workflow-run-event` | [I69b §1.1](./I69b-agent-orchestration-contract-spec.md) | I69b K3 / I69c UI10 |
| `CONST-BUDGET-4` | 预算 4 项：run 墙钟 / 节点数上限 / 并行上限 / token 归因；未配置 ≠ 0；`maxParallel` 缺省 4 | §9.6、`INV11` | I69b §2.2 / K4 |
| `CONST-F-11` | 失败矩阵 11 条（`F1`–`F11`） | §10 | I69b §3.2 |
| `CONST-A-32` | 判据 `A1`–`A32` 跨两卷统一定义（A1–A24 / A26 / A28–A32 在本卷 §13；A25 / A27 在 I69b §4） | §13 | 两卷验收 |
| `CONST-K-12` | 前端读取纪律 `K1`–`K12` | [I69b §3.2](./I69b-agent-orchestration-contract-spec.md) | I69c §6 |
| `CONST-UI-15` | 界面段判据 `UI1`–`UI15` | [I69c §6](./I69c-agent-orchestration-canvas-spec.md) | I69 §1.1 DoD7 |
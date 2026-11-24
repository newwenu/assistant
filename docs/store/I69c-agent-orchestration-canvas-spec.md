# I69c — 编排画布规格（界面段）

| 项 | 值 |
|---|---|
| 状态 | 草稿 |
| 日期 | 2026-11-23 |
| 阶段 | P18（编排轴 L2；见 [I38 §2](./I38-agent-roadmap.md) / §3） |
| 覆盖工作块 | **W3 编排模板编辑器**、**W4 运行态叠加**（[I69 §1.0](./I69-agent-orchestration-roadmap.md) 的界面段，即 P18 阶段目标本体） |
| 波次与交付顺序 | [I69](./I69-agent-orchestration-roadmap.md)（**唯一来源**：前置、退出条件、回退点；本文不排波次） |
| 事实来源 | `docs/analysis/AGENT-CANVAS-RENDERING-ANALYSIS.md`（2026-11-20 快照：C1–C16 事实、P1–P16 命题、G1–G13 缺口、K1–K15 判据、D1–D11 取舍、分析侧 Q1–Q6 未决） |
| 术语唯一出处 | `docs/analysis/ORCHESTRATION-CONCEPT-SEMANTIC-TRANSFER.md` §2.5（用户可见词）· §2.6（开发原语）；本文只引用，不复述定义 |
| 关联 | [I69a](./I69a-agent-orchestration-base-spec.md)（模板 schema / 台账 / 调度语义）· [I69b](./I69b-agent-orchestration-contract-spec.md)（DTO / 命令 / 事件 / 读取纪律 K1–K12）· I66 / I68（已归档，画布 W1 / W2 设施） |
| 影响范围 | 前端：`src/features/orchestration/`（编排画布）、`src/features/canvas/`（**只读复用**，不改设施）、`src/features/observation/canvasContent.ts` 的映射边界。后端：**零改动** |
| 契约影响 | **零新增**：只消费 [I69b §2](./I69b-agent-orchestration-contract-spec.md) 已冻结的 `CONST-CMD-10`（10 条命令）与 `CONST-EVENT-1`（`workflow-run-event`） |
| 上游承接清单 | 逐条 ID 见 [I69a §16.1](./I69a-agent-orchestration-base-spec.md)；本卷落点清单见 §9（**逐条，不用区间**），上游新增冻结项时同批次同步 |

> **本文管理**：界面段定位与边界（§0–§1）、结构约束（§2）、编辑语义 W3（§3）、运行态叠加 W4（§4）、与既有文档的关系（§5）、验收 UI1–UI15（§6）、未决问题 WQ1–WQ9（§7）、拒绝方案（§8）、上游承接清单（§9）。
> **本文不管理**：模板 schema 与校验规则（[I69a §6](./I69a-agent-orchestration-base-spec.md)）、台账与调度语义（[I69a §8–§9](./I69a-agent-orchestration-base-spec.md)）、DTO / 命令 / 事件形态与读取纪律（[I69b](./I69b-agent-orchestration-contract-spec.md)）、波次与退出条件（[I69](./I69-agent-orchestration-roadmap.md)）、画布设施与预设（I66 / I68，已归档）。
> **单向依赖**：本文只引用 I69 的波次与退出条件、I69a / I69b 的约束，不改其中任何一条；冲突时顺序与阶段级 DoD 以 I69 为准，语义与契约以 I69a / I69b 为准。

---

## 0 定位：界面段是编排层的消费者

一句话：**画布只发命令、只读投影，不决定调度**。

| 层 | 事实源 | 界面段的行为 |
|---|---|---|
| 模板 | 后端 `workflow_templates`（经 `save_workflow_template` / `get_workflow_template`） | 编辑与保存；本地视图只做未保存草稿 |
| run | 后端台账（经 `get_workflow_run` / `list_workflow_node_runs`） | 只读叠加；不缓存、不反推 |
| 事件 | `workflow-run-event`（单向广播） | 按键控增量更新视图；漏事件调 `reconcile_workflow_run` |

**为什么界面段不并入 R0–R5**：R0–R5 的门禁是「零契约 diff、wire 一次冻结」；界面段要读模板与台账，只能在其前置冻结之后消费，否则会把契约冻结与渲染形态绑在一起。

---

## 1 范围与边界

### 1.1 一张设施，两种画布

界面段只建**编排画布**，不改既有**观测栅格**。二者共用 `src/features/canvas/` 的渲染设施，但视图模型与策略分列（画布分析 P1–P2）：

| | 观测栅格（已存在，L1） | 编排画布（本文，L2） |
|---|---|---|
| 数据源 | 编译期模板 + `TurnObservation` 投影 | 工作流模板数据 + run 投影 |
| 节点形状 | 格子（行列号 + 状态徽标） | 盒子（坐标 + 尺寸 + 端口 + z-order） |
| 策略 | 只读 | 可编辑（W3） |
| 事实源 | 只读副本 | 模板 = 命令产物；run 进度 = 只读投影 |

**两套视图模型不得合并**（画布分析 D3）：格子与盒子是两种数据形状，合并会让观测栅格承担编辑语义，与只读纪律冲突。

### 1.2 边界：界面段不做的事

| # | 不交付 | 归属 |
|---|---|---|
| 1 | 后端调度、台账、取消树、冲突域判定 | [I69a §8–§9](./I69a-agent-orchestration-base-spec.md) |
| 2 | 就绪集合 / 并行 / 冲突域的前端计算 | [I69b](./I69b-agent-orchestration-contract-spec.md) K7 明令禁止 |
| 3 | 任何新 DTO / 命令 / 事件 | [I69b](./I69b-agent-orchestration-contract-spec.md) 已冻结 |
| 4 | 画布设施改造（补边 / 视口 / 预设 / 布局算法） | W1 / W2（I66 / I68，已归档，已交付） |
| 5 | 自动布局算法（分层 / 力导向 / 正交路由） | 不在本阶段；本文只要求「布局意图」与「渲染测量」分离 |
| 6 | 图级 HITL 的交互形态以外的控制 | gate 只经 `resolve_workflow_gate`；无自动 resolve |
| 7 | 节点结果缓存 / 已完成节点跳过 | [I69a §1.2](./I69a-agent-orchestration-base-spec.md) D3 / D15 |

### 1.3 前置

| 工作块 | 前置 | 说明 |
|---|---|---|
| **W3 编辑**（本地编辑与渲染） | R1 冻结的模板 schema | 只读 schema 即可开工，不依赖命令 |
| **W3 编辑**（读取与保存） | R4 的 `list/get/save_workflow_template` | [I69a §6.5](./I69a-agent-orchestration-base-spec.md) 明确 R1 无 DTO / 命令 / 事件 |
| **W4 叠加** | W3 + R4 的 run 命令与事件 + R5 的接缝 | 只读叠加，前端卸载不影响 run |

---

## 2 结构约束

| # | 约束 | 依据 |
|---|---|---|
| S1 | 画布只经 [I69b §2.1](./I69b-agent-orchestration-contract-spec.md) 命令与事件读写；不 import 任何控制路径（发送 / 取消 / 工具执行） | 画布分析 K1 / K2；[I69b](./I69b-agent-orchestration-contract-spec.md) K1 / K3 |
| S2 | 节点渲染走注册表，key 取自闭集 `turn` / `hitl`；未知类别走降级渲染，**不得**静默改写模板数据 | 画布分析 K4；[I69a §6.1](./I69a-agent-orchestration-base-spec.md) |
| S3 | 位置是意图（模板 `layout`），尺寸与端口锚点是测量（渲染侧）；store 中不出现尺寸 / 锚点字段 | 画布分析 K5；[I69a §6.1](./I69a-agent-orchestration-base-spec.md) INV13 |
| S4 | 视口变化只失效背景与边，不重算节点子树 | 画布分析 K6 |
| S5 | 策略（只读 / 可编辑）只有一个读取点，位于命令派发入口 | 画布分析 K8 |
| S6 | 快捷键接入统一 keymap，画布目录下不出现 `document.addEventListener('keydown')` | 画布分析 K9 |
| S7 | 颜色只来自 CSS 变量，无硬编码色值 | 画布分析 K10 |
| S8 | 画布卸载 / 路由切走不影响 run | 画布分析 K12；[I69b](./I69b-agent-orchestration-contract-spec.md) K12 |

---

## 3 编辑语义（W3）

### 3.1 可编辑的对象

| 对象 | 可编辑项 | 不可编辑项 |
|---|---|---|
| 节点 | 增删、类别（`turn` / `hitl`）、参数（常量）、位置 | 节点 id 复用、类别扩展 |
| 边 | 增删、条件（闭集）、方向 | 边的语义扩展 |
| 分组 | 归属、位置 | — |

**节点类别闭集** `turn` / `hitl` 与**边条件闭集** `always` / `onSuccess` / `onFailure` 直接引用 [I69a §6.1](./I69a-agent-orchestration-base-spec.md)，本文不另立值，也不提供第四值。

**参数只接受常量**：不提供字段级连线或引用输入（[I69a §1.2](./I69a-agent-orchestration-base-spec.md) 第 7 条）；参数控件集见 §7 `WQ7`。

### 3.2 保存与校验

| 规则 | 说明 |
|---|---|
| 校验权威在后端 | 保存期校验的唯一实现是 `assistant-orchestrator` 的 `validate.rs`（[I69a §6.3](./I69a-agent-orchestration-base-spec.md)）；前端预校验只能作为即时提示，**不得**作为唯一拦截 |
| 失败必须定位 | 保存返回 `issues` 列表（[I69b §2.4](./I69b-agent-orchestration-contract-spec.md)），每条带 `nodeId` / `edgeId` / `groupId` 至少一个，画布据此高亮；不得只读 `message` 做整体提示 |
| 全量标出 | 一次保存返回全部命中项，画布一次标全，不做「修一条再存一次」的串行修错 |
| 坐标不参与校验 | 拖动只改 `layout`，不触发校验结果变化（[I69a §6.2](./I69a-agent-orchestration-base-spec.md) INV13） |
| 整模板替换 | 保存语义是整模板替换（`WorkflowTemplateSaveInputDto`），不做增量 patch（[I69b §2.1](./I69b-agent-orchestration-contract-spec.md)） |
| 未保存草稿 | 草稿只存前端内存；**不得**写入 localStorage / IndexedDB 作为模板第二真相 |

### 3.3 编辑与运行的隔离

| 规则 | 说明 |
|---|---|
| run 使用发起时的模板快照 | 已发起 run 不受后续编辑影响（`template_version`，[I69a §8.1](./I69a-agent-orchestration-base-spec.md)） |
| 编辑不触发 run | 保存与发起是两个命令；画布不得在保存后自动发起 |
| 端口粒度 | 节点级或端口级由 §7 `WQ3` 定案后唯一实现，不得两套并存 |

---

## 4 运行态叠加（W4）

### 4.1 只读叠加

| 规则 | 说明 |
|---|---|
| 状态只来自台账与事件 | [I69b](./I69b-agent-orchestration-contract-spec.md) K2；不得从本地缓存、时间戳、消息列表反推 |
| 事件按 `(runId, nodeId)` 键控 | [I69b §3.3](./I69b-agent-orchestration-contract-spec.md)；多 run 并行不串台 |
| 重连先全量后增量 | 重连 / 页面恢复 → 拉全量，丢弃本地增量；漏事件调 `reconcile_workflow_run` |
| 状态闭集不互借 | `NodeRunState`（L2）与 `NodeState`（L1）各自独立类型（[I69b](./I69b-agent-orchestration-contract-spec.md) K5） |
| `skipped` 必带限定语 | 编排语境一律写「调度阻断」（[I69b](./I69b-agent-orchestration-contract-spec.md) K6） |
| 预算未配置 | 渲染为「未配置」，不渲染 `0`（[I69b](./I69b-agent-orchestration-contract-spec.md) K4） |

### 4.2 叠加与编辑的共存

| 规则 | 说明 |
|---|---|
| 运行中可编辑，但编辑不影响在跑 run | 叠加只读；模板编辑写的是新版本 |
| gate 交互 | 只发 `resolve_workflow_gate`；不自动 resolve、不代填 `note`（[I69b](./I69b-agent-orchestration-contract-spec.md) K8） |
| 多 run | 同一模板可并发多 run；视图按 `runId` 选择当前叠加对象，不做多 run 同屏叠加。可选对象经 `list_workflow_runs` 取得（[I69b §2.1](./I69b-agent-orchestration-contract-spec.md)），不从前端记录反推 |

---

## 5 与既有文档的关系

| 文档 | 关系 |
|---|---|
| [I69](./I69-agent-orchestration-roadmap.md) | 波次、前置、回退点与阶段级 DoD7 的唯一来源；本文只定 W3 / W4 的验收判据（§6），不排波次 |
| [I69a](./I69a-agent-orchestration-base-spec.md) | 模板 schema、校验规则、台账与调度语义唯一来源；本文只引用 |
| [I69b](./I69b-agent-orchestration-contract-spec.md) | DTO / 命令 / 事件与读取纪律 K1–K12 唯一来源；本文只引用 |
| 画布分析（[AGENT-CANVAS-RENDERING-ANALYSIS.md](../analysis/AGENT-CANVAS-RENDERING-ANALYSIS.md)） | 事实与判据来源；本文采纳其 K1–K15 作为 §6 判据的输入，不重复定义 |
| I66 / I68（已归档） | 画布设施与预设；本文只复用，不改设施 |
| 词表（[ORCHESTRATION-CONCEPT-SEMANTIC-TRANSFER.md](../analysis/ORCHESTRATION-CONCEPT-SEMANTIC-TRANSFER.md) §2.5 / §2.6） | 用户可见词与开发原语唯一出处；本文只引用 |

---

## 6 验收（UI1–UI15）

| # | 判据 | 检查方式 |
|---|---|---|
| **UI1** | 画布不 import 控制路径 | 依赖方向测试：`features/orchestration/` 不出现发送 / 取消 / 工具执行模块（画布分析 K1；I69b K1） |
| **UI2** | 节点类别 key ⊆ 闭集 | 注册表类型参数为闭集；未知类别降级渲染且模板数据零改写（画布分析 K4） |
| **UI3** | 参数只接受常量 | 参数面板无字段级连线入口；保存结果中参数无引用形态 |
| **UI4** | 边条件只有三值 | 类型层闭集；无第四值分支 |
| **UI5** | 非法图保存被拒并定位 | §6.3 十类非法图各一条用例，错误定位到节点 / 边（分组类在分组面板高亮）；`hitl` 的 `always` 边在编辑器侧即不可选（对齐 [I69a §6.3](./I69a-agent-orchestration-base-spec.md)） |
| **UI6** | 拖动不改校验结果 | 同图仅改坐标，保存结果与校验结果逐字节一致（[I69a §6.3](./I69a-agent-orchestration-base-spec.md) INV13） |
| **UI7** | 位置与测量分离 | store 类型中无尺寸 / 锚点字段（画布分析 K5） |
| **UI8** | 叠加只读 | 画布侧对 run 状态的写入点为零；gate 只经 `resolve_workflow_gate`（[I69b](./I69b-agent-orchestration-contract-spec.md) K2 / K8） |
| **UI9** | 断开不影响 run | 路由切走后 run 继续推进，重连后与台账一致（画布分析 K12；[I69b](./I69b-agent-orchestration-contract-spec.md) K12） |
| **UI10** | 事件订阅唯一入口 | 全仓 `listen("workflow-run-event")` 只有一处 |
| **UI11** | 零 wire 变更 | `npm run contract:verify` 无 diff；界面段无新增 DTO / 命令 / 事件（画布分析 K3） |
| **UI12** | 格子与盒子分型 | 两套视图模型，无共用可编辑类型（画布分析 D3） |
| **UI13** | 颜色只来自 CSS 变量 | 画布样式无硬编码色值（画布分析 K10） |
| **UI14** | 快捷键走统一 keymap | 画布目录下无自建 `keydown` 监听（画布分析 K9） |
| **UI15** | 观测栅格零回归 | 观测侧投影单测与依赖方向测试全绿（画布分析 K13） |

> UI1–UI15 全绿即关闭 [I69 §1.1](./I69-agent-orchestration-roadmap.md) 的 DoD7；本文不新增阶段级判据，也不改 I69 的波次、前置与退出条件。

---

## 7 未决问题（WQ1–WQ9）

| # | 问题 | 影响 | 定案时点 |
|---|---|---|---|
| **WQ1** | 节点规模上限（决定是否引第三方图库） | 自研与引库的成本交叉点（画布分析 D2） | W3 开工前 |
| **WQ2** | 编排画布是否复用观测页的路由与实例切换 | 两个 feature 还是一个 feature 的两种模式 | W3 开工前 |
| **WQ3** | 端口粒度：节点级 / 端口级 | 校验粒度与命中测试复杂度（D11） | W3 开工前 |
| **WQ4** | 是否允许「边上插入节点」 | 边渲染器是否需要命中区与挂载点 | W3 开工前 |
| **WQ5** | 缩放是否允许低于证据徽标可读阈值 | 小尺寸下的降级形态 | W4 开工前 |
| **WQ6** | 画布快捷键键位与全局键位的冲突归属 | keymap 注册策略 | W3 开工前 |
| **WQ7** | 参数控件集（下拉 / 数字 / 文本的闭集） | 参数面板形态 | W3 开工前 |
| **WQ8** | 非法图的错误呈现：行内标注 / 汇总列表 | 校验反馈形态 | W3 开工前 |
| **WQ9** | 预算输入的界面形态（墙钟 / 节点数 / 并行上限在哪填、是否暴露） | 发起表单形态；不填即「未配置」（[I69b §2.2](./I69b-agent-orchestration-contract-spec.md) INV11） | W4 开工前 |

> **编号空间**：本卷用 `WQ`，与 [I69a §15](./I69a-agent-orchestration-base-spec.md) 的 `OQ`、画布分析侧的 `Q` 是三套互不相干的编号，跨卷引用必须带前缀。
> **关闭权**：`WQ` 只由本表关闭，只约束形态、不改任何约束条款。涉及上游 `OQ6`（多选项 / 表单）/ `OQ8`（历史版本）的裁定，本卷只能**触发**，须回提 [I69a §15](./I69a-agent-orchestration-base-spec.md) 并由 I69a 同批次更新全表；本卷无权改上游表，也不得另行宣布其关闭。
> 未决问题只约束**形态**，不改变 [I69](./I69-agent-orchestration-roadmap.md) 的前置与退出条件，也不改变 [I69a](./I69a-agent-orchestration-base-spec.md) / [I69b](./I69b-agent-orchestration-contract-spec.md) 的约束。

---

## 8 拒绝方案

| # | 方案 | 拒绝理由 |
|---|---|---|
| 1 | 直接以第三方图库的 `nodes` 数组作为事实源 | 模板事实源在后端台账；图库数组只能是派生视图（画布分析 §13 的事故形态） |
| 2 | 复用观测投影作为编排视图模型 | 格子无端口 / 尺寸 / z-order，且会把只读语义带进可编辑画布（D3） |
| 3 | 前端自建校验作为唯一拦截 | 与后端 `validate.rs` 形成两套判定，必然分叉（[I69a §6.3](./I69a-agent-orchestration-base-spec.md)） |
| 4 | 画布直连控制命令（发送 / 取消 / 工具执行） | 形成绕过编排层的第二条路径（[I69b](./I69b-agent-orchestration-contract-spec.md) K1） |
| 5 | 由画布缓存 run 状态并据此渲染 | 前端缓存成为第二真相（[I69b](./I69b-agent-orchestration-contract-spec.md) K2） |

---

## 9 上游承接清单（附录）

> 与 [I69a §16.1](./I69a-agent-orchestration-base-spec.md) 一一对应：本表是本卷对上游冻结项的**落点清单**。上游新增冻结项时，本表与 [I69a §16.1](./I69a-agent-orchestration-base-spec.md) 同批次更新；缺任一落点即视为上游成果丢失，本卷不得标记为可实施。

| 承接 ID | 内容（不复述，回上游） | 本卷落点 |
|---|---|---|
| `INV5` | 执行节点集合受 O3 约束 | UI2 / UI4 |
| `INV7` | 静态图 | UI5 |
| `INV13` | 坐标不进语义 | UI6 / UI7 |
| `D3` | 不缓存节点结果 | UI12、§4（叠加只读、不缓存） |
| `D4` | 后端推进 | UI9 |
| `D9` | HITL 回答只进台账 | UI8 |
| `D11` | 参数只接受常量 | UI3 |
| `D13` | 坐标与语义分列 | UI6 / UI7 |
| `D18` | `group` 不是节点类别 | UI2 |
| `D20` | 分组校验 | UI5 |
| `D21` | `hitl` 源边条件 | UI5 |
| `CONST-NODE-KIND-2` | 节点类别闭集 | UI2 |
| `CONST-EDGE-COND-3` | 边条件闭集 | UI4 |
| `CONST-EDGE-COND-HITL-2` | `hitl` 源边条件闭集 | UI5 |
| `CONST-INVALID-GRAPH-10` | 十类非法图 | UI5 |
| `CONST-NODERUN-STATE-7` | L2 节点状态闭集 | §4 叠加映射 |
| `CONST-KEY-2` | 事件路由键 | §4 键控增量 |
| `CONST-CMD-10` | 命令面 | §2 数据来源 |
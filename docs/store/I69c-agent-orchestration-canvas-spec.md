# I69c — 编排画布规格（界面段）

| 项 | 值 |
|---|---|
| 状态 | 定稿（可实施） |
| 日期 | 2026-11-24 |
| 阶段 | P18（编排轴 L2；见 [I38 §2](./I38-agent-roadmap.md) / §3） |
| 覆盖工作块 | **W3 编排模板编辑器**、**W4 运行态叠加**（[I69 §1.0](./I69-agent-orchestration-roadmap.md) 的界面段，即 P18 阶段目标本体） |
| 波次与交付顺序 | [I69](./I69-agent-orchestration-roadmap.md)（**唯一来源**：前置、退出条件、回退点；本文不排波次） |
| 事实来源 | `docs/analysis/AGENT-CANVAS-RENDERING-ANALYSIS.md`（2026-11-20 快照：C1–C16 事实、P1–P16 命题、G1–G13 缺口、K1–K15 判据、D1–D11 取舍、分析侧 Q1–Q6 未决） |
| 术语唯一出处 | `docs/analysis/ORCHESTRATION-CONCEPT-SEMANTIC-TRANSFER.md` §2.5（用户可见词）· §2.6（开发原语）；本文只引用，不复述定义 |
| 关联 | [I69a](./I69a-agent-orchestration-base-spec.md)（模板 schema / 台账 / 调度语义）· [I69b](./I69b-agent-orchestration-contract-spec.md)（DTO / 命令 / 事件 / 读取纪律 K1–K12）· [I69d](./I69d-agent-orchestration-card-spec.md)（W3 前置：卡片形态）· I66 / I68（已归档，画布 W1 / W2 设施）· [dom-identifier-spec](./dom-identifier-spec.md)（DOM 标识的命名 / 取值 / 查找口径） |
| 影响范围 | 前端：`src/features/orchestration/`（编排画布）、`src/features/canvas/`（**补全示例实现缺失的功能**：边几何 / 锚点 / 命中 / 边渲染原语 / 连线提交（§2.1 M1–M6，不可关闭）+ 三处可关闭增量（[I69d §6](./I69d-agent-orchestration-card-spec.md)），其余只读复用）、`src/features/observation/canvasContent.ts` 的映射边界。后端：**零改动** |
| 契约影响 | **零新增**：只消费 [I69b §2](./I69b-agent-orchestration-contract-spec.md) 已冻结的 `CONST-CMD-10`（10 条命令）与 `CONST-EVENT-1`（`workflow-run-event`） |
| 上游承接清单 | 逐条 ID 见 [I69a §16.1](./I69a-agent-orchestration-base-spec.md)；本卷落点清单见 §9（**逐条，不用区间**），上游新增冻结项时同批次同步 |

> **本文管理**：界面段定位与边界（§0–§1，含 §1.4 归属矩阵：每条「不做」的承接方与闭合判据）、结构约束与既有设施对接（§2，含 §2.3 分组交互边界 / §2.4 落位兜底）、编辑语义 W3 与草稿状态机（§3，含 §3.7 模板生命周期）、运行态叠加 W4 与事件映射（§4，含 §4.4 叠加层纪律 / §4.6 取消与校正入口）、与既有文档的关系（§5）、验收 UI1–UI15（§6）、未决问题 WQ1–WQ9（§7，**已全部定案**）、拒绝方案（§8）、上游承接清单（§9）。
> **本文不管理**：模板 schema 与校验规则（[I69a §6](./I69a-agent-orchestration-base-spec.md)）、台账与调度语义（[I69a §8–§9](./I69a-agent-orchestration-base-spec.md)）、DTO / 命令 / 事件形态与读取纪律（[I69b](./I69b-agent-orchestration-contract-spec.md)）、**卡片形态与设施增量**（[I69d](./I69d-agent-orchestration-card-spec.md)）、波次与退出条件（[I69](./I69-agent-orchestration-roadmap.md)）、画布设施与预设（I66 / I68，已归档）、DOM 标识的命名 / 取值 / 查找口径（[dom-identifier-spec](./dom-identifier-spec.md)；本文只写落点，不另立标识规则）。
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

界面段不交付的事**共 15 条，唯一清单与承接方、闭合判据见 §1.4**；本节不再另列一表，避免两表并存后口径漂移。

| 一句话边界 | 详见 |
|---|---|
| 不建后端语义：调度 / 台账 / 取消树 / 冲突域归 I69a，前端不重算就绪集合与并行 | §1.4 第 1–2 行 |
| 不扩 wire：无新增 DTO / 命令 / 事件 | §1.4 第 3 行 |
| 不改造画布设施：只允许 §2.1 编辑结果导出回调与 [I69d §6](./I69d-agent-orchestration-card-spec.md) 的 INC1–INC3 两类可选增量 | §1.4 第 4 行 |
| 不引入自动布局与自动 run 级控制；不缓存节点结果 | §1.4 第 5–7 行 |

### 1.3 前置

| 工作块 | 前置 | 说明 |
|---|---|---|
| **W3 编辑**（本地编辑与渲染） | R1 冻结的模板 schema | 只读 schema 即可开工，不依赖命令 |
| **W3 编辑**（读取与保存） | R4 的 `list/get/save_workflow_template` | [I69a §6.5](./I69a-agent-orchestration-base-spec.md) 明确 R1 无 DTO / 命令 / 事件 |
| **W4 叠加** | W3 + R4 的 run 命令与事件 + R5 的接缝 | 只读叠加，前端卸载不影响 run |

### 1.4 归属矩阵：每条「不做」的承接方与闭合判据

「不做」必须完成**移交**，只写「归某卷」等于没归。本表逐条给出三列：不做的事 / 唯一承接方 / 闭合判据。**第三列填不出来的，就是真空洞**，就地在本卷补规则（补在 §2–§4，不新增 UI 编号，`CONST-UI-15` 的判据集保持 UI1–UI15 不变）。

| # | 不做的事 | 唯一承接方 | 闭合判据 |
|---|---|---|---|
| 1 | 后端调度 / 台账 / 取消树 / 冲突域 | [I69a §8–§9](./I69a-agent-orchestration-base-spec.md) | A1–A24、A28–A32（上游判据） |
| 2 | 就绪集合 / 并行 / 冲突域的前端计算 | [I69b](./I69b-agent-orchestration-contract-spec.md) K7 | UI8（画布对 run 状态零写入） |
| 3 | 新 DTO / 命令 / 事件 | [I69b §2](./I69b-agent-orchestration-contract-spec.md) | UI11（`contract:verify` 无 diff） |
| 4 | 画布设施改造 | 本卷 §2.1（设施对接：接缝规则 + **边几何改造清单 M1–M6**）+ [I69d §5.1](./I69d-agent-orchestration-card-spec.md)（边形态唯一出处）+ [I69d §6](./I69d-agent-orchestration-card-spec.md) INC1–INC3（可关闭增量） | §2.1 改造清单逐条闭合（M1–M6，**补全示例实现的功能缺失，不可关闭**）+ OC12（三处增量缺省不传即现状）+ OC14（色值只走 CSS 变量）+ §2.1 接缝规则全表（可选默认关闭 / 增量而非全量 / 删除只报被选对象 / 只导出结果 / 不反向受控 / 丢弃靠重挂载 / 回填坐标原样 / 禁止改造项）+ §3.4「丢弃必换 key」「草稿即真值」；UI15（观测栅格零回归）为**护栏**——约束补功能时不伤观测侧，不是「不动设施」的理由（观测页走 `GraphCanvas` 自绘 SVG，不经 `CanvasEdge`） |
| 5 | 自动布局算法 | **明确不做**（非移交）：位置由用户拖拽产生 | §2.4 落位兜底 + §3.5 拖拽 |
| 6 | run 级控制的自动行为 | 本卷 §4.6（显式取消 / 显式校正）；gate 在 §4.4 | UI8 + §4.6 三规则 |
| 7 | 节点结果缓存 / 已完成节点跳过 | **明确不做**（[I69a](./I69a-agent-orchestration-base-spec.md) D3 / D15） | §4.1（叠加只读、无缓存） |
| 8 | 模板**新建** | 本卷 §3.7（`save` 的 upsert 语义，零 wire 变更） | §3.7 + UI11 |
| 9 | 模板**删除** | **明确不做**（[I69a §1.2](./I69a-agent-orchestration-base-spec.md) 第 16 条） | §3.7「不提供删除」 |
| 10 | 模板**重命名** | 本卷 §3.7（改 `name` 后 `save`，不另立命令） | §3.7 + UI11 |
| 11 | 分组形态（标签 / 虚线框 / 度量） | [I69d §5.3 / §6 INC3](./I69d-agent-orchestration-card-spec.md) | OC13（分组只产出 `CanvasLabelView`、不产出卡片）+ OC12（INC3 缺省关闭） |
| 12 | 分组交互（选中 / 拖拽 / 错误定位） | 本卷 §2.3 第 2–5 条 | UI5（分组类错误标在标签上） |
| 13 | `layout` 缺位的落位 | 本卷 §2.4 | §2.4 四规则 + UI6（坐标不改校验） |
| 14 | 出口锚点 `exitAnchorOf` | **明确不立项**（[I69a §1.2](./I69a-agent-orchestration-base-spec.md) 第 12 条） | §2.1「不依赖」一行 |
| 15 | 发起表单与预算输入的形态 | 本卷 §4.5 唯一来源（[I69b §3.1](./I69b-agent-orchestration-contract-spec.md) 只定数据来源） | §4.5 + UI8 |

> **一致性纪律**：本表任一行改承接方，必须同批次改对应章节与 [I69a §16.1](./I69a-agent-orchestration-base-spec.md) 承接清单；不得只改本表。
> **表外补充（不是「不做」项）**：卡片形态（逐字段 / 六件构成 / 状态映射 / 非卡片对象）是**交付项**，不是本卷的「不做」，故不进本表；其唯一出处与闭合判据在 [I69d §2–§5](./I69d-agent-orchestration-card-spec.md) / OC1–OC9、OC13。本卷 §2.1 与 §4.4 只引用、不复述。

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

### 2.1 与既有画布设施的对接

W1 / W2（I66 / I68，已归档）交付了视口、卡片、连线、放置栏与快捷键，但**连线设施是为示例模板量身做的**：示例连线全部落在同一行或相邻行、全部正向，几何按 `right → left` / `bottom → top` 写死。编排画布的位置由用户自由拖拽，回边与上行是常态，**照原样「只叠加」必然画不出线**（目标在源左侧 / 上方时段长为负，CSS 丢弃该段）。

**因此「不碰设施」不成立**：不是「改了会带来观测回归，所以忍着不动」，而是**示例实现缺功能，必须补齐**——它缺方向判断（只按 `|dx| ≥ |dy|` 二选一）、缺反向 / 上行几何、缺按盒子相对位置推导的锚点、缺上行箭头取值（`ArrowDirection` 只有 `left` / `right` / `down`）、缺相对两端盒子的外轨参考系（现状取世界顶端 `y = gutter`）、缺多段命中区归一，且渲染原语只有矩形段（虚线 / 流动靠 `data-orientation` 二值切样式，表达不了左 / 上流向）。这些不是风格偏好，是「拖到左边就画不出线」的功能缺失。`UI15`（观测栅格零回归）是**护栏**，用来约束补功能时别伤到观测侧，**不是不动设施的理由**；且观测页走的是 [GraphCanvas.tsx](../../src/features/observation/GraphCanvas.tsx) 自己画的 SVG，不经 `CanvasEdge` 渲染路径，回归风险落在 `canvasContent.ts` 投影边界（本卷零改动），不在连线渲染层。

因此本节把设施改动分两类：**可关闭增量**（卡片 / 标签形态，缺省不传即现状）与**必须改造**（边几何、锚点、命中、边渲染原语、边身份；不可关闭，规则唯一出处是 [I69d §5.1](./I69d-agent-orchestration-card-spec.md)）。**参考项目对照**（2026-11-25 核对）：n8n 正向走 Vue Flow `getSmoothStepPath`（中缝取两端 X 中点，`borderRadius: 16`）、反向特判为「源下方 130px 局部下探」两段；Dify 恒用 `getBezierPath`（`curvature: 0.16`，不做方向判断）；ComfyUI / litegraph 每段为贝塞尔（控制点沿 slot 方向偏移 `0.25 × 弦长`）。三者都画 SVG 路径、不拼矩形段，故不存在负段长，虚线 / 流动动画也沿路径方向。**我们的结论是走 n8n 的正交走向 + 同一种渲染原语（SVG path + 圆角）**（M6）：矩形段拼装在本设施里无解——段宽高为负会被 CSS 丢弃，归一化又会抹掉方向符号，虚线角度 / 流动 keyframes 需按方向枚举翻倍（2×2 矩阵变 4×4），命中区在多段绕行时退化成「几乎整个画布的包围盒」而必须逐段拆按钮。换 path 后这四类问题一并消失；差异只剩**外轨 16px**（自选，理由见 I69d §5.1）。

| 设施 | 现状（2026-11-24 复核） | W3 / W4 用法 | 允许的改动 |
|---|---|---|---|
| `CanvasViewport`（[CanvasViewport.tsx](../../src/features/canvas/CanvasViewport.tsx)） | 视口缩放 / 平移、卡片拖拽、连线编辑、放置栏拖放（`onDropCard` / `placeRequest` 两个既有回调）、键盘平移；编辑结果（位移 / 删除 / 端点改写 / 新增边）为**组件内部会话状态**，无导出回调 | 编排画布的唯一渲染宿主 | **唯一新增接缝**：编辑结果导出（可选回调，见下表）；放置栏路径沿用现状不改；就**编辑结果导出接缝**而言，不传回调时行为 / DOM / 事件顺序与现状一致（边的绘制方式由 M6 独立替换，不属本接缝范围） |
| `CanvasCardView` / `CanvasEdgeView`（[types.ts](../../src/features/canvas/types.ts)） | 盒子视图模型：`box` / `kindBadge` / `params` / `ports` / `status` / `tone`；边 `from` / `to` / `route` / `phase` | 模板节点与边的**派生视图**（§2.2） | 不改既有字段语义；卡片内容形态与三处增量以 [I69d §3 / §6](./I69d-agent-orchestration-card-spec.md) 为准 |
| `viewport.ts` | `VIEWPORT_SCALE_MIN` 0.5 / `MAX` 2 / `STEP` 1.1；`clampScale` 纯函数 | 直接使用，不另立缩放区间 | 不改常量（见 WQ5 定案） |
| `geometry.ts`（[geometry.ts](../../src/features/canvas/geometry.ts)） | `edgeGeometryOf` 按 route 写死锚点与段长：`horizontal` 恒 `right → left` 且 y 取源中心、`vertical` 恒 `bottom → top` 且 x 取源中心、`outer-track` 只有「源在右、目标在左、上轨」一种朝向；斜置时箭头与线段不重合，反向 / 上行时段长为负、CSS 丢弃该段 | 边几何的**唯一改造点**：输出从「矩形段数组」改为**单条 SVG `d` 路径 + 末端切线角**（形状规则见 [I69d §5.1](./I69d-agent-orchestration-card-spec.md)，落点见 M2） | **必须改造**（不可关闭）；`anchorOf` 四个侧面语义不变 |
| `CanvasEdge.tsx`（[CanvasEdge.tsx](../../src/features/canvas/CanvasEdge.tsx)） | `anchorSidesOf` 只按 `route` 查常量表（vertical → bottom/top，否则 right/left），不看两端盒子相对位置；命中区为全部矩形段的并集包围盒；箭头只有 `left` / `right` / `down` 三值；虚线 / 流动动画靠 `data-orientation` 的二值切样式 | 按 `from` / `to` 两个完整盒子推导锚点；渲染改为 `<path>` 描边 + 同路径透明粗描边作命中区 + 末端切线旋转的箭头（M3 / M4 / M6） | **必须改造**（不可关闭）：`data-edge-id` / `data-phase` 保留且消费者不变；`data-orientation` **同批删除**（画布侧消费者随矩形段一并消失，按 [dom-identifier-spec C21 / N19](./dom-identifier-spec.md) 处理）；`CanvasLegend` 的图例小样改自带样式（不再硬写 `data-orientation="horizontal"` 借画布侧选择器） |
| `presets/cardKinds.ts` | 示例卡片类别表（含 `execution-turn` / `execution-hitl` 及端口占位） | 只取形与度量；内容由 [I69d §3](./I69d-agent-orchestration-card-spec.md) 另建构造器产出，**不调用** `cardViewOf`；其余示例类别（tool / conversation / …）不进编排放置栏 | 不改表；编排侧另建类别到模板 `kind` 的一一映射 |
| `presets/metrics.ts` | 卡片度量与网格；其中 `loopGutter = 0` 是示例预设「无回边」的前提 | `cellW` / `cellH` / `gap` / `PORT_METRICS` 直接使用；**`loopGutter` 不沿用 `CARD_METRICS` 的 0**，回边绕行取 `CANVAS_METRICS.loopGutter`（16）——`CanvasViewport` 的 `gutter` 缺省即该值，编排侧不得传 0 | 度量常量不改；绕行间距由编排侧显式取值 |
| 选择原语 | `selectedId`（受控 / 不受控两模式）+ `onSelect`，卡片与边共用同一个选中 id | W3 用它驱动参数面板、删除、条件下拉；W4 用它决定叠加焦点 | 不改；编排侧用受控模式，选中状态留在编排侧 |
| 出口锚点 `exitAnchorOf` | **未交付**（`geometry.ts` 只有 `anchorOf`） | W3 / W4 **不依赖**：出口角标用 `CanvasCardView.exit` 现有呈现，不引入新的锚点计算 | 不新增（属画布侧待建，P18 不实现） |
| `features/observation/canvasContent.ts` | 观测栅格投影（`TurnObservation` → 格子视图） | **不复用**（§1.1 两套视图模型不合并） | 零改动（UI15 零回归） |

**唯一新增接缝：编辑结果导出**。W3 需要把用户的位移 / 删改 / 重连结果转成模板草稿，而现状编辑状态留在组件内且不外泄。因此 `CanvasViewport` 增加**一个可选回调**（形如 `onEditResult?: (result: CanvasEditResult) => void`），在每次编辑会话提交时回调一次；`CanvasEditResult` 只含四类结果——`moved: { id; x; y }[]`、`removed: { nodes: string[]; edges: string[] }`、`addedEdges: { id; from; to }[]`、`rewired: { id; from; to }[]`，**不含尺寸、锚点或视口**（S3 / UI7）；**新增节点不在此结果内**，走既有 `onDropCard`（§3.5）。

| 规则 | 说明 |
|---|---|
| 可选且默认关闭 | 不传回调 → **就编辑结果导出接缝而言**组件行为、事件顺序与现状一致（UI15 的判据不因新增接缝放宽）。边的绘制方式按 M6 改为 SVG path，属独立的必须改造项，不属本接缝 |
| 增量而非全量 | 每次回调只带**自上次回调以来的增量**（同一手势 / 同一删除动作的提交集合），编排侧按 id 合并进草稿；重复上报同一 id 以最后一次为准，不做全量覆盖、不做整表替换 |
| 删除只报被选对象 | 删除节点时 `removed.nodes` 只报该节点 id，**级联删边由编排侧按草稿完成**（§3.5），组件不重复上报级联边；删除边时只报 `removed.edges`。两类分列的依据是 id 唯一域不同：节点 id 全模板唯一，边 id 只保证边集内唯一（§3.5） |
| 只导出结果，不导出过程 | 拖拽中的每帧位移不进回调；只在手势结束 / 删除 / 连线提交时回调，避免草稿被高频写入 |
| 不反向受控 | 编排侧不回写组件的内部编辑状态；`cards` / `edges` 仍是唯一内容输入，组件按现有「内容刷新不覆盖本地编辑」的语义工作 |
| 丢弃靠重挂载，不新增 prop | 组件的编辑层（位移 / 删除 / 新增边 / 端点改写）**不随 `cards` 刷新清空**，故「丢弃本地草稿」（撤销改动 / 切换模板）由编排侧换 `key` 重挂载 `CanvasViewport` 完成；代价是同时重置视口缩放与平移（已接受，不为此新增参数） |
| 回填坐标原样 | 保存成功或重新拉取后回填的 `cards`，其 `box` 必须等于 emit 时的坐标（**不取整、不夹取、不吸附**）；否则组件内部位移会盖住回填值，界面与草稿不一致 |
| 禁止改造项 | 视口数学（`viewport.ts`）、手势 reducer、帧合并调度、选择语义均不改；如需改，走新波次。**边几何 / 锚点 / 命中 / 边渲染原语 / 连线提交不在此列**——它们是必须改造项（M1–M6），不可关闭 |

**必须改造清单（M1–M6，不可关闭）**：示例连线是「同一行 / 相邻行、全部正向」，几何按 `right → left` / `bottom → top` 写死；编排画布自由拖拽，斜置、回边、上行是常态。形状规则唯一出处是 [I69d §5.1](./I69d-agent-orchestration-card-spec.md)，本表只定落点与闭合判据。M6 把渲染原语从矩形段换成 SVG path——这是 M2 / M4 能真正闭合的前提，不是可选优化：矩形段下负段长与多段命中区都无解。

| # | 落点 | 改什么 | 闭合判据 |
|---|---|---|---|
| **M1** | `src/features/orchestration/objects.ts` `orchEdgeOf` | route 不再只按「横纵位移绝对值大小」二选一：先判方向——目标在源右侧 / 下方取正向 `horizontal` / `vertical`；目标在源左侧 / 上方取 `outer-track` | 取值 ⊆ 三值；斜置与反向均派生非负几何所需的路由；单测覆盖四象限 |
| **M2** | `src/features/canvas/geometry.ts` `edgeGeometryOf` | 输出改为 `{ d: string; arrow: { x; y; angle } }`（不再产出 `EdgeSegment[]`）：正向斜置补固定三段并带圆角（中缝取两端盒子中心的中点，首末段落两端锚点）；`outer-track` 补「目标在源上方」朝向，外轨参考系从画布绝对顶端（现状 `y = gutter`）改为**两端盒子包围盒上沿**（`y = min(from.y, to.y) - gutter`）——现状首段竖线止于 `from.y` 而上沿锚点在中心，线与锚点并不相接；同行 / 同列退化为单段直线 | 四象限（右 / 下 / 左 / 上）× 斜置全组合的 `d` 均从源锚点起、到目标锚点止；拐角圆角半径取常量（不出现尖角拼接）；箭头角等于末段切线角、落点与目标锚点重合；外轨随两端盒子移动而非固定在世界顶端；现有同行 / 同列用例断言不变（`outer-track` 用例按新参考系更新，属规则变更非回归）；不再出现「负长度」概念（路径无此语义） |
| **M3** | `src/features/canvas/CanvasEdge.tsx` `anchorSidesOf` | 锚点从「按 route 查常量表」改为「按 `from` / `to` 盒子相对位置推导」，端点手柄跟随 | 手柄与箭头落在同一锚点；反向 / 上行手柄不悬空 |
| **M4** | `src/features/canvas/CanvasEdge.tsx` 命中区 | 命中区从「矩形段并集包围盒」改为**同一条 `d` 的透明粗描边**（`stroke-width` 取常量、`pointer-events: stroke`）；`hitBoxOf` 删除 | 线在哪就能点哪，空白处点不中（回边绕行不再产生覆盖卡片区的包围盒）；命中带宽度 ≥ 细线可点阈值；卡片拖拽 / 平移不被误抢 |
| **M5** | `CanvasViewport` 连线提交路径（新增边 / 端点改写 → `CanvasEditResult`） | 提交后按 M1 重算 route；`addedEdges` / `rewired` **不带 route**（route 是派生，不进草稿） | 重连后线随盒子更新；`CanvasEditResult` 字段集不变（本节接缝规则）；草稿无 route 字段 |
| **M6** | `src/features/canvas/CanvasEdge.tsx` + `canvas.module.css` + `CanvasViewport.tsx` 边层 | 边渲染原语从「绝对定位矩形 div 段」改为 SVG path：所有边收进边层内的一个 `<svg>`（根 `pointer-events: none`），每条边渲染描边 `<path>` + 命中 `<path>` + 箭头；键盘可达用 `<g role="button" tabIndex={0} aria-pressed>` + `onKeyDown`（Enter / Space），不套 `<button>`；`data-phase` 保留，`data-orientation` **同批删除**；虚线改 `stroke-dasharray`、流动改 `stroke-dashoffset` 动画（一套样式覆盖任意方向），`repeating-linear-gradient` 与两个方向 keyframes 删除；箭头改末端切线 `rotate(angle)`，`ArrowDirection` 三值与 `arrowOffset` 删除 | `src/**/*.tsx` 中不再渲染 `data-orientation`（G7 死标识扫描通过）；回边 / 上行流动方向与箭头一致（不再逆流）；上行箭头正常显示；`prefers-reduced-motion` 降级仍在；键盘可聚焦并可用 Enter / Space 选中边；图例小样不依赖画布侧选择器即可保持现状 |

### 2.2 模板 → 视图的派生映射

画布**不读 DTO**（S1）：`WorkflowTemplateDto` 先由编排侧投影成 `CanvasCardView` / `CanvasEdgeView`，再交给视口。

| 模板字段 | 视图字段 | 规则 |
|---|---|---|
| `nodes[].id` | `CanvasCardView.id` | 直接复用；节点 id 一旦分配不得复用（§3.1） |
| `nodes[].kind` | `kindBadge` | 取 [I69d §2.3](./I69d-agent-orchestration-card-spec.md) `ORCH_CARD_KINDS` 的 `kindBadgeKey`（`turn` / `hitl`）；**不取示例表 `execution-turn` / `execution-hitl`**；未知类别走降级卡（S2 / OC2） |
| `nodes[].title` | `title` | 原样；空标题用类别默认名，不回写模板 |
| `nodes[].params`（`content` / `mode_id` / `provider_id` / `model_id`） | `params`（最多三行摘要） | 摘要规则唯一出处是 [I69d §3.1](./I69d-agent-orchestration-card-spec.md)；摘要不是编辑控件（参数表单在参数面板即侧栏，[I69d §8.2](./I69d-agent-orchestration-card-spec.md)） |
| `layout.positions[nodeId]` | `box.x` / `box.y` | 位置意图直接映射；`box.w` / `box.h` 取 `presets/metrics.ts` 度量（**测量不进模板**，S3） |
| `edges[].from` / `to` | `CanvasEdgeView.from` / `to` | 直接映射（边端点是节点级，见 WQ3 定案） |
| `edges[].condition` | `phase` | **编辑态一律 `idle`**；运行态由 §4.4 驱动，`condition` 只进边标签文案 |
| `groups[]` / `node.group_id` | `labels`（分组标题标签，§2.3） | 分组只做归属与标题呈现，不产生节点（`group` 不是节点类别，`CONST-NODE-KIND-2`）；分组无独立坐标、默认无矩形框（可选虚线框见 [I69d §6 INC3](./I69d-agent-orchestration-card-spec.md)，缺省关闭） |

| 规则 | 说明 |
|---|---|
| 派生是单向的 | 视图 → 模板只经「编辑结果 + 草稿」回写（§3.4），不由视图反推模板 |
| 未知类别降级 | 渲染为降级卡片并保留原始 `kind` 文本，模板数据零改写（S2 / UI2） |
| 端口只作呈现 | 卡片端口是占位（无类型、不校验）；连线合法性由后端保存期校验决定（§3.2） |

### 2.3 分组的呈现与交互边界

分组的**形态**唯一来源是 [I69d §5.3](./I69d-agent-orchestration-card-spec.md)（组标题标签、`box` = 成员卡片包围盒外扩、`tone`、可选虚线框 INC3），本文不重复定义、不新增字段。本文只补**交互与校验定位**五条：

| # | 规则 |
|---|---|
| 1 | **呈现**：分组只产出 `CanvasLabelView`（不产出卡片）；虚线框走 [I69d §6 INC3](./I69d-agent-orchestration-card-spec.md)，默认关闭，开启与否都不改 `groups[]` 数据 |
| 2 | **不可交互**：分组标签不进选中序列、不可拖拽、不承载连线命中，点击穿透到画布（[I69d §5.3](./I69d-agent-orchestration-card-spec.md) + `CanvasLabel` 现状）。**不提供「拖分组带动成员」**——[I69d §1.2](./I69d-agent-orchestration-card-spec.md) 第 9 条已排除，且选择原语只有单选中 `selectedId`（§2.1） |
| 3 | **调整分组的路径**：分组面板改 `title` / 成员归属（§3.5），位置由成员各自拖动；即「改归属」与「移位置」是两类操作，不合成一个手势 |
| 4 | **分组类错误标在标签上**：`groupId` 定位的校验错误把该分组标签切到警示 `tone` 并浮首条 `message`（§3.6）；分组无标签时（成员全被删）不标，错误只进汇总列表 |
| 5 | **空分组无标签**（[I69d §5.3](./I69d-agent-orchestration-card-spec.md)）；多分组重叠时按 `groups[]` 顺序叠放，不提供 z 序编辑 |

### 2.4 `layout` 缺位的落位兜底

模板 `layout.positions` 可以缺项（新放置节点、空模板、外部导入）。缺项必须有确定性落位，否则「坐标是意图」（S3）会退化成「渲染侧临时测量」：

| # | 规则 |
|---|---|
| 1 | **确定性**：按 `nodes[]` 顺序，为每个缺坐标节点取「第一个不与已有 box 重叠的网格位」（网格步长取 `CARD_METRICS`，度量值以 [I69d §3.4](./I69d-agent-orchestration-card-spec.md) 为唯一来源）。同输入必得同输出，不引入随机与时间因素 |
| 2 | **写回草稿**：W3 编辑态下，落位结果**立即写入草稿 `layout`** 并置 `dirty`（§3.4）。渲染侧不得持有临时坐标；不因落位自动保存（§3.4 无自动保存） |
| 3 | **叠加态只渲染不写回**：W4 只读叠加遇到缺坐标（已保存模板仍缺项，如外部导入）时按同一规则渲染，**不写任何状态、不提示、不修改模板**；下次进入 W3 编辑时才写回 |
| 4 | **不参与校验**：落位不改 `layout` 之外的任何字段，`layout` 本身不参与保存期校验（INV13 / UI6）；落位也不产生校验 `issues` |

---

## 3 编辑语义（W3）

### 3.1 可编辑的对象

| 对象 | 可编辑项 | 不可编辑项 |
|---|---|---|
| 节点 | 增删、类别（`turn` / `hitl`）、参数（常量）、位置 | 节点 id 复用、类别扩展 |
| 边 | 增删、条件（闭集）、方向 | 边的语义扩展 |
| 分组 | 增删、标题、成员归属 | 分组 id 复用；分组无独立坐标、无框可拖（§2.3），位置只能由成员各自拖动 |

**节点类别闭集** `turn` / `hitl` 与**边条件闭集** `always` / `onSuccess` / `onFailure` 直接引用 [I69a §6.1](./I69a-agent-orchestration-base-spec.md)，本文不另立值，也不提供第四值。

**参数只接受常量**：不提供字段级连线或引用输入（[I69a §1.2](./I69a-agent-orchestration-base-spec.md) 第 7 条）。参数控件集是**闭集**（WQ7 定案）：`content` 多行文本（必填）、`mode_id` / `provider_id` / `model_id` 三个下拉（可留空 = 取会话当前值）；不提供数字、开关、文件、日期控件，也不提供「留空 = 未配置」以外的第三态。

**参数表单与参数面板**：控件集合称**参数表单**，承载它的界面区域称**参数面板**（词表 §2.5.2，二者不可互换）。v1 的参数面板定为**选中卡片时出现的侧栏面板**，卡片主体恒为只读摘要（[I69d §8.2](./I69d-agent-orchestration-card-spec.md)）；宿主只是落点选择，不改控件集，也不改卡片模式判定。

**节点类别与控件集一一对应**：`hitl` 节点无参数（`params` 为 `null`），参数面板对 `hitl` 只显示说明文案，不显示任何输入控件。

### 3.2 保存与校验

| 规则 | 说明 |
|---|---|
| 校验权威在后端 | 保存期校验的唯一实现是 `assistant-orchestrator` 的 `validate.rs`（[I69a §6.3](./I69a-agent-orchestration-base-spec.md)）；前端预校验只能作为即时提示，**不得**作为唯一拦截 |
| 失败必须定位 | 保存返回 `issues` 列表（[I69b §2.4](./I69b-agent-orchestration-contract-spec.md)），每条带 `nodeId` / `edgeId` / `groupId` 至少一个，画布据此高亮；不得只读 `message` 做整体提示 |
| 全量标出 | 一次保存返回全部命中项，画布一次标全，不做「修一条再存一次」的串行修错 |
| 坐标不参与校验 | 拖动只改 `layout`，不触发校验结果变化（[I69a §6.2](./I69a-agent-orchestration-base-spec.md) INV13） |
| 整模板替换 | 保存语义是整模板替换（`WorkflowTemplateSaveInputDto`），不做增量 patch（[I69b §2.1](./I69b-agent-orchestration-contract-spec.md)）；`id` 不存在即创建（§3.7） |
| 未保存草稿 | 草稿只存前端内存；**不得**写入 localStorage / IndexedDB 作为模板第二真相 |

### 3.3 编辑与运行的隔离

| 规则 | 说明 |
|---|---|
| run 使用发起时的模板快照 | 已发起 run 不受后续编辑影响（`template_version`，[I69a §8.1](./I69a-agent-orchestration-base-spec.md)） |
| 编辑不触发 run | 保存与发起是两个命令；画布不得在保存后自动发起 |
| 边端点是节点级 | 一条边只有 `from` / `to` 两个节点 id，无端口字段（WQ3 定案）：**端点手柄**命中区取卡片侧边整段、不按端口切分（锚点按 §2.1 M3 由盒子相对位置推导）；**边线本身**的命中区按 §2.1 M4 取同一条 `d` 的透明粗描边，两者不是一回事；`turn` 源可选三值条件，`hitl` 源只可选 `onSuccess` / `onFailure`（`CONST-EDGE-COND-HITL-2`，编辑器侧即不可选 `always`） |

### 3.4 草稿状态机

草稿是**前端内存态**（§3.2 未保存草稿），不进 localStorage / IndexedDB，也不进任何 DTO。

| 状态 | 含义 | 可发出的动作 |
|---|---|---|
| `loading` | 正在 `get_workflow_template` 拉取 | 无（画布只读空态） |
| `clean` | 已加载、与最近一次保存结果一致 | 编辑 / 保存 / 发起 run |
| `dirty` | 有未保存改动（位移、增删、参数、连线） | 保存 / 撤销改动（重新拉取）/ 发起 run（**用已保存版本**） |
| `saving` | `save_workflow_template` 在途 | 无（编辑控件禁用，画布仍可平移缩放） |
| `invalid` | 保存返回 `issues` 非空 | 按 `issues` 定位修改 / 再次保存 / 放弃 |

| 迁移 | 触发 | 规则 |
|---|---|---|
| `loading → clean` | 拉取成功 | 以返回的 `WorkflowTemplateDto` 为基线（含 `layout`） |
| `clean → dirty` | 任一编辑结果（§2.1 回调）或参数变更 | 只置脏，不预校验、不自动保存 |
| `dirty → saving` | 点保存 | 保存的是**整模板**（`WorkflowTemplateSaveInputDto`），不增量 |
| `saving → clean` | `issues` 为空 | 以返回的 `template` 替换基线与草稿，脏标记清除 |
| `saving → invalid` | `issues` 非空 | 草稿**保留**不丢；按 `issues` 一次标全（§3.6） |
| `invalid → dirty` | 用户改任一处 | 旧 `issues` 清空（不保留过期标注），等下次保存重新判定 |
| `dirty / invalid → clean` | 撤销改动（重新拉取） | 丢弃本地草稿；如已有未保存改动，需用户确认后才丢弃 |
| 任意 → `loading` | 切换模板 | 同上，未保存改动需确认 |

| 规则 | 说明 |
|---|---|
| 只有一处草稿 | 同一时刻只维护一个模板草稿；切换模板即丢弃旧草稿（确认后） |
| 丢弃必换 key | `dirty / invalid → clean`（撤销改动）与「切换模板」都要**换 `CanvasViewport` 的 `key` 重挂载**：组件内部编辑层不随 `cards` 刷新清空（§2.1），不重挂载则旧位移 / 删除会残留在界面上；视口缩放与平移随之复位属已接受代价 |
| 草稿即真值 | 草稿是界面唯一真值：编排侧把 emit 结果原样并进草稿再派生 `cards`（§2.1 回填坐标原样），不得在中间层做坐标修正 |
| 保存失败不静默 | `saving` 期间命令失败（非校验类，如 `StorageFailure`）→ 回 `dirty` 并提示，草稿不丢 |
| 不自动保存 | 无定时器、无防抖保存；保存只由显式动作触发 |
| 位置也算改动 | 仅拖动也进 `dirty`（`layout` 是模板的一部分）；坐标不触发校验（§3.2） |
| 版本不等即拒 | 拉取 / 保存遇 `version` 不等按 [I69a §6.2](./I69a-agent-orchestration-base-spec.md) D22 报错，不进入 `clean` |
| 进入路径 | 先 `list`：无任何模板 → 呈现「新建模板」入口，**不进入** `loading`（§3.7）；有模板 → 选中后进入 `loading` |
| 无命令期（R1~R3） | 以内置示例模板常量为初始草稿，直接进入 `clean` 并标注「示例模板、不可保存」；`saving` 不可达（§3.7） |

### 3.5 编辑操作矩阵

| 操作 | 输入 | 草稿结果 | 备注 |
|---|---|---|---|
| 放置节点 | 放置栏点选 / 拖入 / 回车 | 新增节点 + `layout.positions` 落点 | 新节点 id 由前端生成且全模板唯一；类别只取 `turn` / `hitl`；节点经 `onDropCard` 回调进入草稿并**立即进 `cards`**（编排侧不以 `placedCards` 作第二来源），落点即 `layout.positions` 初值，置 `dirty` |
| 拖拽节点 | 卡片拖拽 | 改 `layout.positions` | 只改坐标；不触发校验 |
| 删除节点 | 选中 + 删除 | 删节点 + 级联删其全部边 + 移出分组 | 级联在草稿侧完成，保存期校验仍会复核悬空边 / 空分组 |
| 连线 | 侧边拖拽 | 新增边 + 默认条件（`turn` 源 `always`、`hitl` 源 `onSuccess`） | 自环 / 环 / 条件不相容不在此拦截，由保存期校验报（§3.2） |
| 改条件 | 边选中 + 条件下拉 | 改 `edges[].condition` | 下拉项按源节点类别收窄（`hitl` 源无 `always`） |
| 重连端点 | 端点手柄拖拽 | 改 `edges[].from` / `to` | 与新增边同一条校验路径 |
| 删除边 | 边选中 + 删除 | 删边 | — |
| 改参数 | 参数面板 | 改 `nodes[].params` | 控件闭集见 §3.1；无即时校验 |
| 改标题 | 头部内联输入 | 改 `nodes[].title` | 空标题按类别默认名渲染，不回写 |
| 分组归属 | 分组面板 | 改 `groups[]` / `node.group_id` | 一节点最多一个分组；空分组由保存期拒绝 |
| 平移 / 缩放 | 滚轮 / 拖拽空白 / 键盘 | **不改草稿** | 视口是会话状态（S3），不进模板 |

### 3.6 校验反馈的呈现

WQ8 定案：**行内标注 + 汇总列表并存**，一次标全，不做串行修错。

| 呈现面 | 内容 | 规则 |
|---|---|---|
| 行内标注 | 命中节点 / 边 / 分组标题标签高亮 + 首条 `message` 浮层 | 定位取 `nodeId` / `edgeId` / `groupId`（§3.2）；分组类错误标在分组标题标签上（§2.3 第 4 条），不在成员节点上标 |
| 汇总列表 | 逐条 `kind` + `message`，可点击定位 | 点击后视口平移到该元素并选中；列表与行内标注一一对应 |
| 计数 | 顶部「N 项待修」 | 只计数，不做「已修 M/N」进度（前端不预校验，无法判定已修） |
| 过期清理 | 用户改任一处即清空旧 `issues` | 见 §3.4 `invalid → dirty`；不保留已失效的标注 |
| 文案来源 | `kind` → i18n key；`message` 只作补充 | 文案不以 `message` 为判据（`message` 仅供展示，[I69a §6.3](./I69a-agent-orchestration-base-spec.md)） |
| 不阻断 | 标注不锁画布 | 用户可继续编辑、平移、缩放；只有保存动作会重新判定 |
| 不做即时预校验 | 本阶段不实现 §3.2 允许的「即时提示」：判定只在保存后发生，因此也不存在「边改边标」与「已修计数」 |

> 定位一律经视图模型完成（`selectedId` + `box` + `tone`），不读 DOM 标识；DOM 标识的命名、取值与查找口径以 [dom-identifier-spec](./dom-identifier-spec.md) 为准，本文不另立要求。

### 3.7 模板的新建、重命名与删除

命令集已冻结（`CONST-CMD-10`：`list` / `get` / `save`），**不新增命令**。模板的生命周期操作全部落在 `save_workflow_template` 的 upsert 语义上（[I69a §6.5](./I69a-agent-orchestration-base-spec.md)）：

| 操作 | 做法 | 规则 |
|---|---|---|
| 新建 | 前端生成模板 `id`（全库唯一、非空），以空 `nodes` / `edges` / `groups` + 当前 schema `version` 调 `save` | `id` 不存在即创建（[I69a §6.5](./I69a-agent-orchestration-base-spec.md) 保存语义）；创建成功后进入 `clean`。不提供「另存为」 |
| 重命名 | 改 `name` 后 `save` | 不另立命令；`name` 允许为空串，为空时列表与头部展示「未命名模板」（展示兜底，不回写） |
| 删除 | **不提供** | 无删除命令（[I69a §1.2](./I69a-agent-orchestration-base-spec.md) 第 16 条）；不以「清空后保存」伪装删除 |
| 无模板时进入编辑器 | 首态 `loading` 前先 `list`：无任何模板则直接呈现「新建模板」入口，不进入画布 | 避免「进入编辑器却无对象可编辑」的空态歧义；新建完成后再走 §3.4 的 `loading` |

**R1~R3 期间的模板来源**：命令未落地时，W3 的「本地编辑与渲染」以**内置示例模板常量**为初始草稿（与现有 `OrchestrationPage` 的示例数据同源），标注为「示例模板、不可保存」；`save` 落地后该常量退化为新建模板的初始内容，示例标注移除。此期间不新增任何持久化路径。

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
| 取消与校正不在画布 | 画布不提供取消 / 校正入口，二者归 run 面板（§4.6）；画布不发起任何 run 级控制 |
| 多 run | 同一模板可并发多 run；视图按 `runId` 选择当前叠加对象，不做多 run 同屏叠加。可选对象经 `list_workflow_runs` 取得（[I69b §2.1](./I69b-agent-orchestration-contract-spec.md)），不从前端记录反推 |

### 4.3 事件 → 视图动作映射

事件形态与字段以 [I69b §1.1](./I69b-agent-orchestration-contract-spec.md) 为准，本表只规定**收到后视图做什么**，不改字段语义。

| 事件字段 | 视图动作 | 禁止 |
|---|---|---|
| `runId` | 与当前叠加 `runId` 比对：不等则**丢弃**（只更新 run 列表项） | 不得把其它 run 的节点状态写进当前叠加 |
| `nodeId` | 按 `(runId, nodeId)` 定位节点；模板中不存在该 `nodeId` → 忽略并计数告警（不新建节点） | 不得凭事件补建节点或改写模板 |
| `state` | 覆盖该节点的 `NodeRunState`（7 值闭集） | 不得与 L1 `NodeState` 互借（K5） |
| `runState` | 更新 run 概览徽标 | 不得由 `state` 反推 `runState` |
| `reason` | 进节点提示与 run 面板失败原因；`null` 即不显示 | 不得据此推断本地原因或改写文案判据 |
| `turnId` / `assistantMessageId` | 只存为「结果跳转」的锚点（跳转入口在 run 面板，点击时才按需取消息；卡片内只读文本，[I69d §8.1](./I69d-agent-orchestration-card-spec.md)） | 不得据此拉取消息列表或反推节点结果 |
| `note` | gate 节点在等待态展示备注 | 不得代填、不得自动 resolve（K8） |
| `at` | **只**用于同一 `(runId, nodeId)` 的乱序抑制（单调不减） | 不得显示为墙钟时间、不得与 `started_at` 混用 |

| 规则 | 说明 |
|---|---|
| 幂等 | 同一事件重复到达（同 `state` + 同 `at`）不得产生第二次可见变更 |
| 乱序 | 同键 `at` 小于已应用值时丢弃该事件；跨节点不排序（并行本就无序） |
| 不漏补建 | 事件只更新已存在的节点；缺失节点说明模板已变（编辑了新版本），以台账全量为准 |
| 订阅唯一 | 全仓只有一处 `listen("workflow-run-event")`（UI10）；画布不自行订阅 |

### 4.4 状态 → 视觉：卡片级映射归 I69d，本卷只管叠加层纪律

`NodeRunState` 7 值到**卡片**的映射（`tone` / 左缘色带 / 药丸 key 与用户可见词 / 附加区 `detail` 行）**唯一出处是 [I69d §4.2 / §4.3](./I69d-agent-orchestration-card-spec.md)**，本卷不另立表——两份表并存会让同一状态出现两套 `tone` 写法（中文语义名 vs `default` / `active` / … 枚举名），必然漂移。本卷只定**叠加层**纪律：

| 规则 | 说明 |
|---|---|
| 卡片级映射 | 见 [I69d §4.2 / §4.3](./I69d-agent-orchestration-card-spec.md)；本卷不复述，判据在 OC8（7 值逐行断言） |
| 未叠加即编辑态 | 无 run 或未选中 run → 卡片无药丸、无色带，边 `phase` 全 `idle`（[I69d §4.1](./I69d-agent-orchestration-card-spec.md)） |
| 边随节点 | 下游节点 `skipped` → 上游到它的边标 `untaken`；`running` 节点的出边标 `active`，已走通标 `passed`（取值仍来自 `EdgePhase` 闭集，不新增值） |
| 双通道 | 左缘色带（随缩放不消失）+ 药丸文案（0.5 缩放下仍可读）同时存在，不得只留其一 |
| gate 入口在卡外 | 决策表单落侧栏 / 浮层（[I69d §8.3](./I69d-agent-orchestration-card-spec.md)），卡内只读 `note`；只发 `resolve_workflow_gate`（[I69b](./I69b-agent-orchestration-contract-spec.md) K8） |
| 颜色来源 | 只取 CSS 变量（S7 / UI13）；不得为状态新增硬编码色值 |
| 缩放下限 | 0.5（`VIEWPORT_SCALE_MIN`）；不开放更低缩放，故不建「低于阈值」的降级形态（WQ5 定案） |
| 预算未配置 | 显示「未配置」，不显示 `0`（K4 / `CONST-BUDGET-4`） |

### 4.5 发起表单与预算（WQ9 定案）

| 项 | 形态 | 规则 |
|---|---|---|
| 模板 | 从当前打开的模板带入（可改选） | 只经 `list_workflow_templates` 取候选 |
| 来源会话 | 必填选择 | `originConversationId`；节点级会话绑定（`nodeConversations`）在 v1 **不提供界面**，缺省全部走来源会话（[I69b §2.2](./I69b-agent-orchestration-contract-spec.md)） |
| 墙钟上限 | 数字输入，单位分钟（可留空） | 留空 = 未配置；换算为 `wallClockMs` 时在派发点做一次，不做双向同步 |
| 节点数上限 | 整数输入（可留空） | 留空 = 未配置；`0` 是非法输入（「未配置 ≠ 0」，`INV11`） |
| 并行上限 | 整数输入（可留空） | 留空 = 未配置（后端缺省 4）；不在前端复述缺省值 |
| token 归因 | **无输入**，只读展示于 run 面板 | token 只做读侧归因，不是闸门（`CONST-BUDGET-4`） |
| 幂等键 | 无界面，前端生成 | `clientRunKey`；同一表单重复提交复用同一键 |

### 4.6 取消与校正：入口在 run 面板，不在画布

画布保持纯只读（S5 / W4），**不承载任何 run 级控制**。`cancel_workflow_run` 与 `reconcile_workflow_run` 都是已冻结命令（`CONST-CMD-10`），必须有唯一消费方，落在 run 详情面板的操作区：

| 操作 | 入口 | 规则 |
|---|---|---|
| 取消 run | run 面板操作区「取消」+ 二次确认 | 只由用户显式发起（[I69b K8](./I69b-agent-orchestration-contract-spec.md)）；命令幂等（[I69a](./I69a-agent-orchestration-base-spec.md) DoD4），重复点击不报错；成功后按 [I69b §3.3](./I69b-agent-orchestration-contract-spec.md) 重连路径整表重拉 |
| 校正（reconcile） | run 面板「校正」入口，仅在视图与台账不一致时出现 | 只由用户显式发起，禁止定时轮询、禁止自动触发；调 `reconcile_workflow_run` 后整表重拉，**不合并本地增量**（[I69b §3.3](./I69b-agent-orchestration-contract-spec.md)） |
| 画布上的对应物 | **无** | 画布不出现取消 / 校正 / 重试按钮；画布只呈现节点状态（§4.4），不发起控制 |

---

## 5 与既有文档的关系

| 文档 | 关系 |
|---|---|
| [I69](./I69-agent-orchestration-roadmap.md) | 波次、前置、回退点与阶段级 DoD7 的唯一来源；本文只定 W3 / W4 的验收判据（§6），不排波次 |
| [I69a](./I69a-agent-orchestration-base-spec.md) | 模板 schema、校验规则、台账与调度语义唯一来源；本文只引用 |
| [I69b](./I69b-agent-orchestration-contract-spec.md) | DTO / 命令 / 事件与读取纪律 K1–K12 唯一来源；本文只引用 |
| [I69d](./I69d-agent-orchestration-card-spec.md) | 编排卡片形态（逐字段 / 状态映射 / 非卡片对象）、三处可关闭增量与边几何形状（§5.1）的唯一来源；本文只引用，不复述 |
| 画布分析（[AGENT-CANVAS-RENDERING-ANALYSIS.md](../analysis/AGENT-CANVAS-RENDERING-ANALYSIS.md)） | 事实与判据来源；本文采纳其 K1–K15 作为 §6 判据的输入，不重复定义 |
| I66 / I68（已归档） | 画布设施与预设的**示例模板实现**；壳与度量复用，**边几何 / 锚点 / 命中 / 边渲染原语 / 连线提交按 §2.1 M1–M6 必须改造**，其余不改。归档不等于规范（[I69d §6](./I69d-agent-orchestration-card-spec.md)） |
| 词表（[ORCHESTRATION-CONCEPT-SEMANTIC-TRANSFER.md](../analysis/ORCHESTRATION-CONCEPT-SEMANTIC-TRANSFER.md) §2.5 / §2.6） | 用户可见词与开发原语唯一出处；本文只引用 |

---

## 6 验收（UI1–UI15）

| # | 判据 | 检查方式 |
|---|---|---|
| **UI1** | 画布不 import 控制路径 | 依赖方向测试：`features/orchestration/` 不出现发送 / 取消 / 工具执行模块（画布分析 K1；I69b K1） |
| **UI2** | 节点类别 key ⊆ 闭集 | 注册表类型参数为闭集；未知类别降级渲染且模板数据零改写（画布分析 K4） |
| **UI3** | 参数只接受常量 | 参数面板无字段级连线入口；保存结果中参数无引用形态 |
| **UI4** | 边条件只有三值 | 类型层闭集；无第四值分支 |
| **UI5** | 非法图保存被拒并定位 | §6.3 十类非法图各一条用例，错误定位到节点 / 边（分组类在分组标题标签上高亮，§2.3）；`hitl` 的 `always` 边在编辑器侧即不可选（对齐 [I69a §6.3](./I69a-agent-orchestration-base-spec.md)） |
| **UI6** | 拖动不改校验结果 | 同图仅改坐标，保存结果与校验结果逐字节一致（[I69a §6.3](./I69a-agent-orchestration-base-spec.md) INV13） |
| **UI7** | 位置与测量分离 | store 类型中无尺寸 / 锚点字段（画布分析 K5） |
| **UI8** | 叠加只读 | 画布侧对 run 状态的写入点为零；gate 只经 `resolve_workflow_gate`；取消 / 校正只出现在 run 面板、画布上无对应控件（[I69b](./I69b-agent-orchestration-contract-spec.md) K2 / K8；§4.6） |
| **UI9** | 断开不影响 run | 路由切走后 run 继续推进，重连后与台账一致（画布分析 K12；[I69b](./I69b-agent-orchestration-contract-spec.md) K12） |
| **UI10** | 事件订阅唯一入口 | 全仓 `listen("workflow-run-event")` 只有一处 |
| **UI11** | 零 wire 变更 | `npm run contract:verify` 无 diff；界面段无新增 DTO / 命令 / 事件（画布分析 K3） |
| **UI12** | 格子与盒子分型 | 两套视图模型，无共用可编辑类型（画布分析 D3） |
| **UI13** | 颜色只来自 CSS 变量 | 画布样式无硬编码色值（画布分析 K10） |
| **UI14** | 快捷键走统一 keymap | 画布目录下无自建 `keydown` 监听（画布分析 K9） |
| **UI15** | 观测栅格零回归 | 观测侧投影单测与依赖方向测试全绿（画布分析 K13）。**这是护栏，不是「不动设施」的理由**：编排侧必须补全示例实现缺失的连线功能（§2.1 M1–M6），护栏只约束补功能时不伤观测侧 |

> UI1–UI15 只提供 [I69 §1.1](./I69-agent-orchestration-roadmap.md) DoD7 的**验收细则**：本文不判定交付，也不判定 DoD7 是否被覆盖——交付成立与否只看 [I69 §1.4](./I69-agent-orchestration-roadmap.md) 的自证条件与 [I69 §1.4.6](./I69-agent-orchestration-roadmap.md) 的 DU 台账；本文不新增阶段级判据，也不改 I69 的波次、前置与退出条件。

---

## 7 未决问题（WQ1–WQ9，已全部定案）

> 定案依据：画布分析（2026-11-20 快照）+ 参考项目（ComfyUI / Dify / n8n）模式核对 + `src/features/canvas/` 现状复核（2026-11-24）。定案只约束**形态**，不新增任何约束条款，也不改上游。

| # | 问题 | 定案 | 落点 |
|---|---|---|---|
| **WQ1** | 节点规模上限（决定是否引第三方图库） | **不引第三方图库**，沿用 `src/features/canvas/` 自研设施（视口 / 拖拽 / 连线已齐）。模板**不设节点数上限**（上限属 run 预算 `maxNodes`，见 `CONST-BUDGET-4`）；验收性能基准为单模板 60 节点 / 120 边下拖拽与缩放不掉帧，超基准不承诺框选与小地图 | §2.1 / UI7 |
| **WQ2** | 编排画布是否复用观测页的路由与实例切换 | **不复用**。编排是独立 feature + 独立路由（现状 `/agent-orchestration` 已注册），观测页保持只读观测；两者**只共用** `src/features/canvas/` 渲染设施，不共用视图模型、实例切换与心跳 | §1.1 / §2.1 / UI12 |
| **WQ3** | 端口粒度：节点级 / 端口级 | **节点级**。模板 schema 的边只有 `from` / `to` 两个节点 id，无端口字段（引入端口级即契约变更，违反 UI11）；端点手柄命中区取卡片侧边整段、边线命中区取路径粗描边（§2.1 M4），端口只作呈现 | §2.2 / §3.3 |
| **WQ4** | 是否允许「边上插入节点」 | **v1 不支持**。边无挂载点（同 WQ3），插入需拆边重连且要新增边命中区，收益低；用户可先放节点再手动重连，路径已覆盖 | §3.5 |
| **WQ5** | 缩放是否允许低于证据徽标可读阈值 | **不开放低于 0.5**（沿用 `VIEWPORT_SCALE_MIN`），因此不建「低缩放降级形态」；状态表达走双通道（左侧色带 + 药丸文案），0.5 下仍可读 | §4.4 / UI13 |
| **WQ6** | 画布快捷键键位与全局键位的冲突归属 | **焦点所有权**：画布键只在画布容器持有焦点时生效，注册进统一 keymap；与全局键冲突时全局键优先，冲突在**注册期**显式报错，不在按键时排序 | S6 / UI14 |
| **WQ7** | 参数控件集（下拉 / 数字 / 文本的闭集） | **两类控件**：`content` 多行文本（必填）+ `mode_id` / `provider_id` / `model_id` 三个下拉（可留空 = 取会话当前值）；无数字 / 开关 / 文件控件，无即时校验 | §3.1 / §3.5 / UI3 |
| **WQ8** | 非法图的错误呈现：行内标注 / 汇总列表 | **两者并存**：行内标注（节点 / 边 / 分组标题标签 + 首条 `message`）+ 汇总列表（可点击定位），一次标全、不锁画布、改动即清空旧标注 | §3.6 / UI5 |
| **WQ9** | 预算输入的界面形态（墙钟 / 节点数 / 并行上限在哪填、是否暴露） | **发起表单暴露三项可留空输入**（墙钟按分钟、节点数、并行上限），留空 = 未配置、`0` 为非法输入；token 归因**无输入**，只在 run 面板只读展示；`nodeConversations` 在 v1 不提供界面 | §4.5 / UI8 |

> **编号空间**：本卷用 `WQ`，与 [I69a §15](./I69a-agent-orchestration-base-spec.md) 的 `OQ`、画布分析侧的 `Q` 是三套互不相干的编号，跨卷引用必须带前缀。
> **关闭权**：`WQ` 只由本表关闭，只约束形态、不改任何约束条款。涉及上游 `OQ6`（多选项 / 表单）/ `OQ8`（历史版本）的裁定，本卷只能**触发**，须回提 [I69a §15](./I69a-agent-orchestration-base-spec.md) 并由 I69a 同批次更新全表；本卷无权改上游表，也不得另行宣布其关闭。
> **定案不变量**：WQ1–WQ9 全部关闭后，本卷不保留「待定形态」；实施期如需推翻任一条，必须先改本表并同步 §2–§4 的落点，不得只改实现。
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
| 6 | 引入第三方图库承载编排画布 | 自研设施已覆盖视口 / 拖拽 / 连线，引库会把图库模型变成事实源（WQ1 定案） |
| 7 | 端口级连线（边挂在端口而非节点） | 模板 schema 无边端口字段，端口级必然引入契约变更（WQ3 定案 / UI11） |
| 8 | 「边上插入节点」手势 | 需边命中区与拆边重连两套新机制，收益低于手动重连（WQ4 定案） |
| 9 | 前端预校验并实时标红 | 与后端 `validate.rs` 形成两套判定（同第 3 条），故只在保存后标全（WQ8 定案） |
| 10 | 用 `CanvasCardView.container` 或新增矩形框渲染分组 | `container` 是卡片自身的虚线边框，分组框需要新的边框层与命中规则，属设施改造（§1.4 第 4 行 / §2.3） |
| 11 | 「拖分组带动成员」手势 | 设施无多选，需新原语与批量位移语义；已拆为「改归属」与「移位置」两类操作（§2.3 第 2–3 条） |
| 12 | 新增删除 / 重命名模板命令 | `CONST-CMD-10` 已冻结，违反零 wire 变更（UI11）；重命名走 `save` 改 `name`，删除需单独裁定（§3.7） |
| 13 | 画布上放取消 / 校正按钮，或定时 `reconcile` | 画布纯只读（S5 / W4）；自动 run 级控制违反 [I69b](./I69b-agent-orchestration-contract-spec.md) K8，入口归 run 面板（§4.6） |
| 14 | 缺坐标节点在渲染侧临时算位置、不写回草稿 | 把「位置意图」退化为「渲染测量」（S3 / INV13），且编辑态与叠加态会出现两套位置（§2.4 第 2–3 条） |

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
| `CONST-NODERUN-STATE-7` | L2 节点状态闭集 | §4.4 叠加层纪律；逐值映射见 [I69d §4.2](./I69d-agent-orchestration-card-spec.md) / OC8 |
| `CONST-KEY-2` | 事件路由键 | §4.3 键控增量 |
| `CONST-CMD-10` | 命令面 | §2 数据来源 / §3.7 模板生命周期（`save` upsert，无删除） / §4.6 取消与校正的唯一消费方 |
| `CONST-EVENT-1` | `workflow-run-event` | §4.3 事件映射 / UI10 |
| `CONST-BUDGET-4` | 预算 4 项与「未配置 ≠ 0」 | §4.4 / §4.5（「未配置」不渲染 `0`） |
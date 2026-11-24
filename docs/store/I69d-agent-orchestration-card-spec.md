# I69d — 编排画布卡片规格（W3 前置）

| 项 | 值 |
|---|---|
| 状态 | 定稿（可实施；先于 [I69c](./I69c-agent-orchestration-canvas-spec.md) 实施）；任务级进度只在 [I69d tasklist](./I69d-agent-orchestration-card-tasklist.md) 勾选 |
| 日期 | 2026-11-24 |
| 阶段 | P18（编排轴 L2；见 [I38 §2](./I38-agent-roadmap.md) / §3） |
| 覆盖工作块 | **W3 前置**：编排画布上真实卡片的形态（不含编辑交互、不含运行态叠加的读取路径） |
| 波次与交付顺序 | [I69](./I69-agent-orchestration-roadmap.md)（**唯一来源**：前置、退出条件、回退点；本文不排波次） |
| 事实来源 | `src/features/canvas/`（W1 / W2 已交付设施的**现状**，2026-11-24 复核：`types.ts` / `cardKinds.ts` / `metrics.ts` / `CanvasCard.tsx` / `canvas.module.css`） |
| 术语唯一出处 | `docs/analysis/ORCHESTRATION-CONCEPT-SEMANTIC-TRANSFER.md` §2.5（用户可见词）· §2.6（开发原语）；本文只引用，不复述定义 |
| 关联 | [I69a](./I69a-agent-orchestration-base-spec.md)（模板 schema / 校验 / 调度）· [I69b](./I69b-agent-orchestration-contract-spec.md)（DTO / 事件 / 读取纪律）· [I69c](./I69c-agent-orchestration-canvas-spec.md)（编辑与叠加语义）· I66 / I68（已归档，画布 W1 / W2） |
| 影响范围 | 前端：`src/features/orchestration/`（新增编排卡片表与内容构造）；`src/features/canvas/`（**三处增量**，§6，全部可默认关闭）；`src/i18n/locales/{zh-CN,en-US}.json`。后端：**零改动** |
| 契约影响 | **零新增**：不新增 DTO / 命令 / 事件；只消费 [I69b §1.1](./I69b-agent-orchestration-contract-spec.md) 的 `workflow-run-event` 与 §2.1 的模板 DTO |

> **本文管理**：编排画布上有哪些卡片（§2）、每张卡逐字段与六件构成（§3）、状态到视觉的映射（§4）、非卡片对象（§5）、对既有设施的三处增量（§6）、派生与数据来源（§7）、交互边界（§8）、对 I69c 的最小修订（§9）、验收 OC1–OC14（§10）、未决 CQ1–CQ8（§11，**已全部定案**）、拒绝方案（§12）、上游承接清单（§13）。
> **本文不管理**：模板 schema 与校验规则（[I69a §6](./I69a-agent-orchestration-base-spec.md)）、台账与调度语义（[I69a §8–§9](./I69a-agent-orchestration-base-spec.md)）、DTO / 命令 / 事件形态（[I69b](./I69b-agent-orchestration-contract-spec.md)）、编辑语义与草稿状态机、运行态事件消费纪律（[I69c §3–§4](./I69c-agent-orchestration-canvas-spec.md)）、波次与退出条件（[I69](./I69-agent-orchestration-roadmap.md)）。
> **与 I69c 的关系**：本卷是 I69c 的**前置冻结**（先定「卡片长什么样」，再定「怎么编辑、怎么叠加」）。两卷并列、互不改条款；I69c 需在其 §2.1 的落点处引用本卷（同步清单见 §9）。

---

## 0 定位：编排卡片不是示例卡片的复用

一句话：**编排画布用自己的一张卡片表，示例卡片表只贡献「形」，不贡献「内容」。**

W2（I68，已归档）交付的 5 类卡片是**示例卡片**：内容全是占位常量，头部挂「示例」徽标，轮次卡片主体是一条 12 条 L1 阶段轨道。编排画布读的是**真实模板 + 真实 run 台账**，若直接调用 `cardViewOf` / `exampleCardOf`：

| 撞车点 | 后果 |
|---|---|
| `execution-turn` 的 `body: "stages"` → 12 条 L1 阶段轨道 | 把 L1 观测语义带进 L2 画布，违反 [I69c §1.1](./I69c-agent-orchestration-canvas-spec.md)「两套视图模型不得合并」 |
| `exampleBadge` 恒填「示例」 | 真实数据被标成示例，等于对用户撒谎（I68 A25 的反向违约） |
| `params` 三行是「模型 / 预算 / 工具集」 | 模板参数是 `content` + `mode_id` / `provider_id` / `model_id`，字段集不同 |
| `exit: "stopped"` 是 L1 出口汇点语义 | 编排画布没有出口汇点，角标语义悬空 |
| `tone` 是示例色 | 运行态需要按 7 值 `NodeRunState` 取色 |

**结论**：`CARD_KINDS`（示例表）与编排卡片表**分列、不合并、不互相引用**；编排侧另建内容构造器，只在**度量**（`CARD_METRICS` / `PORT_METRICS`）与**渲染壳**（`CanvasCard` / `CanvasEdge` / `CanvasLabel`）上与 W1 / W2 共用。

---

## 1 范围与边界

### 1.1 交付什么

| # | 目标 | 判据 |
|---|---|---|
| 1 | 编排卡片清单与闭集：`turn` / `hitl` 两类 + 未知类别降级形态 | OC1 / OC2 |
| 2 | 每张卡的逐字段内容（徽标 / 标题 / 参数摘要 / 附加区 / 端口 / 脚部） | OC3 / OC4 / OC5 / OC7 |
| 3 | 7 值 `NodeRunState` → `tone` / 药丸文案 / 附加区内容的映射 | OC8 / OC9 |
| 4 | 非卡片对象（边、条件标签、分组、端口）的形态 | OC13 / OC7（端口只画圆点、无类型标签） |
| 5 | 对既有画布设施的三处**可关闭增量** | OC12 / OC14 |
| 6 | 卡片内零交互控件、参数表单与 gate 决策入口的落点 | OC10 |

### 1.2 不交付什么

「不做」必须完成**移交**：只写「归某卷」等于没归，故本表与 [I69c §1.4](./I69c-agent-orchestration-canvas-spec.md) 同构，逐行给出三列——不交付的事 / 唯一承接方 / 闭合判据。**第三列填不出来的就是真空洞**，就地在本卷或承接方补规则。

| # | 不交付 | 唯一承接方 | 闭合判据 |
|---|---|---|---|
| 1 | 编辑语义（增删 / 连线 / 重连 / 草稿状态机 / 校验反馈） | [I69c §3](./I69c-agent-orchestration-canvas-spec.md) | [I69c](./I69c-agent-orchestration-canvas-spec.md) UI3–UI6（参数闭集 / 条件闭集 / 保存定位 / 坐标不改校验） |
| 2 | 运行态事件消费（订阅、键控、乱序抑制、reconcile） | [I69c §4](./I69c-agent-orchestration-canvas-spec.md) | [I69c](./I69c-agent-orchestration-canvas-spec.md) UI8–UI10（叠加只读 / 断连不影响 run / 订阅唯一） |
| 3 | 任何新 DTO / 命令 / 事件 | [I69b](./I69b-agent-orchestration-contract-spec.md) 已冻结 | OC11（`contract:verify` 无 diff） |
| 4 | 就绪集合 / 并行 / 冲突域的前端计算 | [I69b](./I69b-agent-orchestration-contract-spec.md) K7 禁止 | [I69c](./I69c-agent-orchestration-canvas-spec.md) UI8（画布对 run 状态零写入） |
| 5 | 卡片主体的「概览」态（缩放降级形态） | [I69c WQ5](./I69c-agent-orchestration-canvas-spec.md)：不开放低于 0.5 缩放，不建降级形态 | OC8（7 值药丸在 0.5 下仍可读；无缩放阈值分支） |
| 6 | 自动布局 / 正交路由 / 边插入节点 | [I69c §1.4](./I69c-agent-orchestration-canvas-spec.md) 第 5 行 / WQ4 | OC13（边 `route` 只取 `horizontal` / `vertical`）+ [I69c](./I69c-agent-orchestration-canvas-spec.md) UI6 |
| 7 | 把示例卡片升级为真实卡片（改 `CARD_KINDS`） | 示例表冻结；真实卡片由本卷另建（§2.1） | OC1（编排侧不调用 `cardViewOf` / `exampleCardOf`）+ OC12（观测零回归） |
| 8 | 卡片主体内的参数**表单** | 落参数面板（侧栏，§8.2） | OC10（卡片子树内无 `input` / `select` / `textarea` / 内层 `button`） |
| 9 | 分组虚线框以外的分组能力（折叠 / 嵌套 / 拖入归组动画） | v1 只做组标题标签（§5.3） | OC13（分组只产出 `CanvasLabelView`、不产出卡片） |

> **一致性纪律**：本表任一行改承接方，必须同批次改对应章节、[I69c §1.4](./I69c-agent-orchestration-canvas-spec.md) 与本卷 §9 同步清单；不得只改本表。

### 1.3 前置

| 项 | 说明 |
|---|---|
| 模板 schema | 只读 [I69a §6.1](./I69a-agent-orchestration-base-spec.md) 的 `WorkflowNode` / `NodeKind` / `TurnNodeParams` / `WorkflowEdge` / `EdgeCondition` / `WorkflowGroup` |
| 卡片壳与度量 | 只读 `src/features/canvas/` 现状，不改既有字段语义 |
| 契约 | 只读 [I69b](./I69b-agent-orchestration-contract-spec.md)，零新增 |

---

## 2 卡片清单

### 2.1 两张表分列

| 表 | 位置 | 内容 | 谁用 |
|---|---|---|---|
| 示例卡片表 `CARD_KINDS` | `src/features/canvas/presets/cardKinds.ts`（**不改**） | 5 类示例卡片，占位常量 | 观测页（`features/observation/canvasContent.ts`） |
| 编排卡片表 `ORCH_CARD_KINDS` | `src/features/orchestration/cardKinds.ts`（**新增**） | 2 类真实卡片 + 1 类降级形态，无示例值 | 编排画布 |

| 纪律 | 说明 |
|---|---|
| 不合并 | 两表字段形状可相似，但**不得**互相 import；合并会让示例值成为真实卡片的默认值（I68 A1 的反例） |
| 不共用构造器 | 编排侧**不得**调用 `cardViewOf` / `exampleCardOf` / `paletteItemsOf`（OC1） |
| 共用度量与壳 | `CARD_METRICS` / `PORT_METRICS` / `CanvasCard` / `CanvasEdge` / `CanvasLabel` 直接使用，不改其数学与字段语义 |
| 类别 key 闭集 | 编排卡片 key 只有 `turn` / `hitl`（取自 [I69a §6.1](./I69a-agent-orchestration-base-spec.md) `NodeKind`），不提供第四值 |

### 2.2 编排画布上的对象

| 对象 | 渲染形态 | 是卡片？ | 来源 |
|---|---|---|---|
| `turn` 节点 | 执行卡片 · 轮次 | 是 | `WorkflowNode.kind === "turn"` |
| `hitl` 节点 | 执行卡片 · 人工介入 | 是 | `WorkflowNode.kind === "hitl"` |
| 未知 `kind` 节点 | 降级卡片 | 是（降级） | 模板 `kind` 不在闭集（防御形态，正常不可达，见 §3.3） |
| 分组 `WorkflowGroup` | 组标题标签（+ 可选虚线框） | **否** | `groups[]` |
| 边 `WorkflowEdge` | 连线 + 条件标签 | **否** | `edges[]` |
| 端口 | 圆点（左入 / 右出） | 构件 | 派生，非数据 |
| 出口汇点 | **不存在** | — | 编排画布无出口语义 |

**类别与分型**：`卡片类别` 的闭集由工作流模板 schema 定，现已由 [I69a §6.1](./I69a-agent-orchestration-base-spec.md) `NodeKind`（`turn` / `hitl`，编译期闭集）关闭；词表 §2.5.2 对应行已同批更新为**定案**（2026-11-24）。因此编排卡片按「执行卡片 · 轮次 / 人工介入」呈现，**不再带「示例分型」限定**（§11 CQ1）。

### 2.3 编排卡片表与构造器形态

表与构造器都在 `src/features/orchestration/` 内，**不 import** `presets/cardKinds.ts`（§2.1 纪律）：

| 项 | 形态 |
|---|---|
| 表项 | `OrchCardKind = { key: "turn" \| "hitl"; kindBadgeKey: string; defaultTitleKey: string; editTone: CardTone; hasParams: boolean }` |
| 表 | `ORCH_CARD_KINDS: readonly OrchCardKind[]`，**恰好两项**，顺序 `turn`、`hitl`；无示例徽标、无参数占位、无阶段轨道 |
| 构造器 | `orchCardOf(node: WorkflowNode, run: NodeRunOverlay \| null, ctx: OrchCardContext): CanvasCardView` |
| `ctx` | `{ box: CanvasBox; t: (key: string) => string; nowMs?: number }`：`box` 由 `layout` / [I69c §2.4](./I69c-agent-orchestration-canvas-spec.md) 落位给出（测量不进模板）；`t` 注入翻译函数，构造器内不读全局 i18n；`nowMs` 只在运行态算时长 |
| 降级构造器 | `orchFallbackCardOf(node: WorkflowNode, ctx: OrchCardContext): CanvasCardView`，形态见 §3.3；降级不是类别，**不进** `ORCH_CARD_KINDS` |
| 放置栏 | 只从 `ORCH_CARD_KINDS` 生成 `turn` / `hitl` 两项；不生成其余示例类别 |
| 纯函数 | 不读全局状态、不写模板、不缓存 run；同输入（含 `nowMs`）必得同输出 |

| 规则 | 说明 |
|---|---|
| 两态同源 | `orchCardOf` 的 `run === null` 即编辑态（无药丸 / 无色带），`run !== null` 即运行态（§4）；**不换主体内容** |
| `run` 只带视图量 | `NodeRunOverlay` 只含 `state` + 只读引用（`turnId` / `messageId` / `reason` / `note` / `startedAt`）；事件 `at` 不进（§4.3） |
| 无参数卡 | `hitl` 的 `hasParams === false` → 不产出 `params`（§3.2） |

---

## 3 逐卡设计

### 3.1 `turn` 卡（执行卡片 · 轮次）

| 区 | 内容 | 规则 |
|---|---|---|
| 头部 · 徽标 `kindBadge` | 「执行卡片 · 轮次」（`orch.card.kind.turn`） | 类别 + 分型，定案词，不带「示例」 |
| 头部 · 名称 `title` | `nodes[].title` | 空标题 → 分型默认名「轮次」，**不回写模板** |
| 头部 · 示例徽标 `exampleBadge` | **不填** | 真实数据不得标示例（OC5） |
| 头部 · 组名徽标 | **不填** | 组归属由组标题标签承载（§5.3） |
| 主体 · 参数摘要（3 行） | 见下表 | 只读；三行固定，不因字段增减改高 |
| 主体 · 附加区 | `detail` 变体：运行附加 | 编辑态为空提示；运行态见 §4.3 |
| 端口区 | 左 1 圆点（入）/ 右 1 圆点（出） | 只画圆点，**无类型标签**（OC7）；`typeLabel` 字段必填，填空串 `""`（`CanvasCard` 只把它放 `title`，空串即无提示） |
| 脚部 · 状态药丸 | `NodeRunState` 文案（§4.2） | 编辑态不画（无 run） |
| 脚部 · 运行时长 `measure` | 打开面板时算一次（台账 `started_at` → `now_ms`） | 不做秒级 tick（I69b §1.2） |
| 脚部 · 引用 `reference` | **不填** | 类别即引用，重复展示无信息 |
| 出口角标 `exit` | **不填** | 编排画布无出口汇点（OC6） |
| `tone` | 编辑态 = `default`；运行态 = 状态色（§4.2） | tone 单通道：编辑态表类别、运行态表状态 |

**参数摘要三行**（4 个字段压 3 行，WQ7 的控件集不变）：

| 行 | 标签 | 取值 | 空值显示 |
|---|---|---|---|
| 1 | 内容（`orch.card.param.content`） | `params.content`，单行截断（`text-overflow: ellipsis`） | 保存期已拒空值，正常不可达；加载到空值时显示 `—` |
| 2 | 模式（`orch.card.param.mode`） | `params.mode_id` | `null` → 「会话当前值」（`orch.card.value.inherited`） |
| 3 | 模型（`orch.card.param.model`） | `provider_id` / `model_id` 合成：两者都有 → `provider / model`；仅其一 → 该值；都 `null` → 「会话当前值」 | 同上 |

| 规则 | 说明 |
|---|---|
| 摘要是展示，不是控件 | 卡内无输入框、无下拉、无必填标记（OC10）；完整内容与编辑在参数面板（侧栏，§8.2） |
| 不截断成中文句子 | 摘要值只用原语量或所选值文本；文案（标签 / 空值）走 i18n |
| 三行固定 | 度量预算：`CARD_METRICS.cellH 360` → 头部 38 + 脚部 26 + 主体 294；参数块 3 × 18 + 内边距 = 66，余量归附加区（227） |

### 3.2 `hitl` 卡（执行卡片 · 人工介入）

| 区 | 内容 | 规则 |
|---|---|---|
| 头部 · 徽标 | 「执行卡片 · 人工介入」（`orch.card.kind.hitl`） | 同 §3.1 |
| 头部 · 名称 | `nodes[].title`；空 → 「人工介入」 | 不回写 |
| 头部 · 示例徽标 | **不填** | — |
| 主体 · 参数摘要 | **不渲染**（`params` 为 `null`，[I69a §6.1](./I69a-agent-orchestration-base-spec.md)） | 参数面板对 `hitl` 只显示说明文案 |
| 主体 · 附加区 | `detail` 变体：「人工介入」说明 + `note`（等待态） | 高度吸收参数区：66 + 1 + 227 = 294 |
| 端口区 | 左 1 圆点 / 右 1 圆点 | 同 §3.1 |
| 脚部 · 状态药丸 | 7 值文案；`waiting` → 「等待人工」 | — |
| 脚部 · 时长 / 引用 | 时长同 §3.1；引用不填 | — |
| 出口角标 | 不填 | — |
| `tone` | 编辑态 = `warning`（类别色）；运行态 = 状态色 | 编辑态用 `warning` 表「需人决策」，运行态由状态覆盖 |

**决策入口不在卡内**（§8.3）：`resolve_workflow_gate` 的按钮属**卡外浮层 / 侧栏**，卡片内只显示只读 `note`。

### 3.3 降级卡片（未知类别）

| 区 | 内容 | 规则 |
|---|---|---|
| 头部 · 徽标 | 原始 `kind` 文本原样显示 + 「未知类别」提示（`orch.card.kind.unknown`） | **不翻译**原始值、不改写模板数据（[I69c S2](./I69c-agent-orchestration-canvas-spec.md) / OC2） |
| 头部 · 名称 | `nodes[].title`；空 → 节点 `id` | 不回写 |
| 主体 · 参数摘要 | **不渲染**（未知 schema 不保证 `params` 形状） | — |
| 主体 · 附加区 | `detail` 变体，空提示「未知节点类别」 | — |
| 端口区 | **不画** | 未知类别的连接语义未知，画端口会暗示可连线 |
| 脚部 · 状态药丸 | **不画** | 事件 `kind` 闭集只有 `turn` / `hitl`，未知类别不会收到事件 |
| 可编辑性 | 可选中、可删除、可改标题；**不可改参数** | 参数面板只显示说明 |
| `tone` | `muted` | — |

> 正常路径不可达：版本不等即拒加载（[I69a §6.2](./I69a-agent-orchestration-base-spec.md) D22 / §6.3 第 5 类）。本形态只为「模板被外部改动 / 未来 schema 升级」时不崩溃、不静默改写。

### 3.4 六件构成与度量（沿用 W1 / W2，不改）

| 构件 | 编排卡处理 | 依据 |
|---|---|---|
| 头部（38px 实色身份带 + 左 3px 类别色竖条） | 渲染；竖条颜色由 `tone` | `canvas.module.css` 现状 |
| 主体（`params` 块 + 1px 分隔 + 附加区） | 渲染；`turn` 三行参数、`hitl` / 降级不画参数区 | §3.1–§3.3 |
| 端口区（8px 圆点骑边框） | 渲染；标签只走 `title` 提示，不进视觉层 | `PORT_METRICS` 现状 |
| 脚部（26px：药丸 + 引用 + 时长） | 渲染；只填药丸与时长 | §3.1 |
| 选择态（选中描边 / 焦点虚线内缩） | 渲染；编排侧用受控 `selectedId` | [I69c §2.1](./I69c-agent-orchestration-canvas-spec.md) |
| 拖拽柄（头部左端点阵） | 渲染；整卡仍是拖拽面 | W2 现状 |

**度量零改动**：`CARD_METRICS`（300 × 360，gap 32）、`PORT_METRICS`（圆点 8，spread 16）直接复用；卡片宽高不随内容变化（内容不得决定几何）。

---

## 4 状态与视觉

### 4.1 卡片主体只有两态

词表 §2.5.2 的卡片模式是「编辑 / 运行 × 详情 / 概览」四态。v1 只实现**详情**两态：

| 态 | 主体 | 附加区 | 脚部 |
|---|---|---|---|
| 编辑 · 详情（无 run 或未选中 run） | 参数摘要（`turn`）或说明（`hitl`） | 空提示 | 无药丸、无色带 |
| 运行 · 详情（选中 run） | 同上（**不换内容**） | 状态相关（§4.3） | 药丸 + 时长 + 左缘色带 |

| 规则 | 说明 |
|---|---|
| 概览两态不实现 | 不开放低于 0.5 缩放（[I69c WQ5](./I69c-agent-orchestration-canvas-spec.md)），0.5 下药丸仍可读，无需降级形态 |
| 编辑态主体不换表单 | 参数表单落参数面板（侧栏，§8.2）；卡片主体恒为只读摘要，避免「编辑模式换主体」引发几何抖动 |
| 切换只改附加区与脚部 | 叠加 / 取消叠加不改参数区高度，卡片不跳动 |

### 4.2 `NodeRunState` → 视觉通道（7 值闭集）

| `NodeRunState` | `tone` | 左缘色带 | 药丸文案（i18n） | 用户可见词 |
|---|---|---|---|---|
| `pending` | `default` | 中性（不画） | `orch.node.state.pending` | 待发起 |
| `running` | `active` | primary | `orch.node.state.running` | 进行中 |
| `waiting` | `warning` | warning | `orch.node.state.waiting` | 等待人工 |
| `finished` | `success` | success | `orch.node.state.finished` | 已完成 |
| `failed` | `danger` | error | `orch.node.state.failed` | 失败 |
| `skipped` | `muted` | 不画 | `orch.node.state.skipped` | **调度阻断**（必带限定语，I69b K6） |
| `cancelled` | `muted` | 不画 | `orch.node.state.cancelled` | 已取消 |

| 规则 | 说明 |
|---|---|
| 双通道 | 左缘色带 + 药丸文案同时存在；色带随缩放不消失，药丸在 0.5 下仍可读 |
| 颜色只来自 CSS 变量 | 无硬编码色值（[I69c S7 / UI13](./I69c-agent-orchestration-canvas-spec.md)） |
| 不互借 | 只用 `NodeRunState` 7 值；L1 `NodeState` 不进编排卡（I69b K5） |
| 预算未配置 | 卡片不显示预算；「未配置 ≠ 0」只在 run 面板生效（I69b K4 / INV11） |

### 4.3 附加区内容（`detail` 变体）

| 状态 | `turn` 卡附加区 | `hitl` 卡附加区 |
|---|---|---|
| 无 run | 空提示「未运行」 | 空提示「未运行」 |
| `pending` | 空提示「待发起」 | 空提示「待发起」 |
| `running` | 行「轮次」= `turnId`（只读文本） | 行「备注」= `note`（可为空） |
| `waiting` | 行「轮次」= `turnId` | 行「备注」= `note` + 说明「等待决策」 |
| `finished` | 行「结果」= `assistantMessageId`（只读文本） | 行「结果」= `assistantMessageId` |
| `failed` | 行「原因」= `reason`（无 `reason` → 「失败」） | 行「原因」= `reason` |
| `skipped` | 空提示「上游未走通」 | 空提示「上游未走通」 |
| `cancelled` | 空提示「已取消」 | 空提示「已取消」 |

| 规则 | 说明 |
|---|---|
| 引用是文本不是按钮 | 卡片是整块 `button`，不得嵌套可点击 chip（OC10）；跳转入口在 run 面板（[I69c §4.3](./I69c-agent-orchestration-canvas-spec.md)） |
| `reason` 只展示 | 不作为判据、不改写文案（[I69c §4.3](./I69c-agent-orchestration-canvas-spec.md)） |
| `at` 不进视图 | 事件 `at` 只用于乱序抑制，不得显示为墙钟时间 |
| 最多两行 | 附加区可容纳 2 行；超出截断，完整内容在 run 面板 |

---

## 5 非卡片对象

### 5.1 边

| 项 | 规则 |
|---|---|
| 端点 | 节点级：`from` / `to` 直接映射（[I69c WQ3](./I69c-agent-orchestration-canvas-spec.md)），无端口字段 |
| 路由 | 只产出 `horizontal` / `vertical`：按两端盒子中心 `|dx| ≥ |dy|` 取 horizontal，否则 vertical |
| 不产出 `outer-track` | 模板是 DAG（[I69a §6.3](./I69a-agent-orchestration-base-spec.md) 第 3 类），无回边；`CARD_METRICS.loopGutter = 0` |
| 不实现正交路由 | 属自动布局范畴（[I69c §1.4](./I69c-agent-orchestration-canvas-spec.md) 第 5 行） |
| `phase` | 编辑态一律 `idle`；运行态按 [I69c §4.4](./I69c-agent-orchestration-canvas-spec.md)（`untaken` / `active` / `passed`），不新增值 |
| 自环 / 环 | 不在渲染层拦截，由保存期校验报（[I69c §3.2](./I69c-agent-orchestration-canvas-spec.md)） |

### 5.2 条件标签

| `condition` | 标签 | 位置 |
|---|---|---|
| `always` | **不生成标签** | — |
| `onSuccess` | 「成功」（`orch.edge.cond.onSuccess`） | 边中点，`CanvasLabelView`，`tone: muted` |
| `onFailure` | 「失败」（`orch.edge.cond.onFailure`） | 同上 |

| 规则 | 说明 |
|---|---|
| 标签不是卡片 | 走 `CanvasLabelView`，不接点击、不进焦点序列 |
| `hitl` 源无 `always` | 编辑器侧即不可选（[I69c §3.3](./I69c-agent-orchestration-canvas-spec.md) / I69a D21），渲染层不必处理该组合 |
| 标签不参与几何 | 绝对定位覆盖层，不占卡片预算 |

### 5.3 分组

| 项 | 规则 |
|---|---|
| 形态 | 组标题标签（`CanvasLabelView`，`box` = 组内节点包围盒外扩 8px，`text` = `groups[].title`，`tone: muted`） |
| 虚线框 | 走 §6 INC3（`CanvasLabelView.frame?: boolean`），默认关闭；开启时该 label 画虚线框 + 左上标题 |
| 不是卡片 | 不产生 `CanvasCardView`，不进选中序列，不参与连线（[I69c §2.2](./I69c-agent-orchestration-canvas-spec.md)） |
| 无独立坐标 | 框由成员位置推导，不存模板（模板 `WorkflowGroup` 无坐标字段） |
| 空分组 | 保存期拒绝（[I69a §6.3](./I69a-agent-orchestration-base-spec.md) 第 8 类），渲染层不画 |

### 5.4 端口

| 项 | 规则 |
|---|---|
| 数量 | 每卡左 1 / 右 1（`hitl` 同）；降级卡不画 |
| 类型标签 | **不画**：字段必填，填空串 `""`；`CanvasCard` 只把 `typeLabel` 放 `title`，空串即无提示（[I69c §2.2](./I69c-agent-orchestration-canvas-spec.md)） |
| 命中区 | 连线命中取卡片侧边整段（[I69c WQ3](./I69c-agent-orchestration-canvas-spec.md)），不按端口切分 |
| 不可配置 | 端口数 / 方位不随模板变化，无端口级字段 |

---

## 6 对既有画布设施的增量（全部可关闭）

W1 / W2 已归档、设施冻结。本卷只允许三处**纯增量**，缺省不传时行为与现状逐字节一致（OC12）。

| # | 增量 | 位置 | 形态 | 缺省行为 |
|---|---|---|---|---|
| **INC1** | `CanvasCardBodyView` 新增通用变体 `detail` | `src/features/canvas/types.ts` + `CanvasCard.tsx` | `{ kind: "detail"; label: string; rows: readonly { label: string; value: string }[]; emptyHint?: string }`；渲染为只读行 + 空提示 | 其他调用方不产出该变体 → 不渲染 |
| **INC2** | 运行态整卡左缘 3px 色带 | `CanvasCard.tsx`（`status !== undefined` 时加 `data-run="true"`，并在色带元素上带 `data-tone={card.tone ?? "default"}`）+ `canvas.module.css` | 覆盖层，颜色取 `tone` 对应的 CSS 变量；`data-tone` 与既有药丸**同源同值**，不改药丸取值 | 无 `status` 时无 `data-run`，无变化 |
| **INC3** | `CanvasLabelView` 可选虚线框 | `types.ts` + `CanvasLabel.tsx` + CSS | `frame?: boolean` → 虚线边框 + 左上标题 | 不传 → 现有纯文字标签 |

| 约束 | 说明 |
|---|---|
| 通用而非编排专用 | INC1 的 `detail` 只承载「行列表」这一通用形状，**不带**任何编排语义（字段名不叫 `turnId` / `reason`）；语义由编排侧填文案 |
| 不改既有字段 | 三处增量都不改 `CanvasCardView` / `CanvasLabelView` 既有字段语义与取值域 |
| 不改数学与手势 | `viewport.ts`、`geometry.ts`、拖拽 reducer、帧合并调度、选择语义均不改 |
| 默认关闭 | 不传即等价现状；观测页零回归（OC12） |
| 需同步 I69c | **已同步（2026-11-24）**：I69c §2.1 两行已按 §9 改为「以本卷 §3 / §6 为准」 |

---

## 7 派生与数据来源

| 视图 | 数据来源 | 刷新 | 禁止 |
|---|---|---|---|
| 卡片集合 | `WorkflowTemplateDto.nodes`（经 `get_workflow_template`） | 加载 / 保存成功后替换 | 由视图反推模板 |
| 卡片位置 | `layout.positions[nodeId]` | 同上 | 尺寸 / 锚点写进模板（INV13） |
| 卡片状态 | `workflow-run-event` + `get_workflow_run` / `list_workflow_node_runs` | 事件到达 / 重连全量 | 从本地缓存、时间戳、消息列表反推（I69b K2） |
| 边 | `edges[]` + 两端盒子 | 同上 | 由 `condition` 反推 `phase` |
| 分组 | `groups[]` + `node.group_id` | 同上 | 分组产生节点 |

| 规则 | 说明 |
|---|---|
| 单向派生 | 模板 → 视图；视图 → 模板只经「编辑结果 + 草稿」（[I69c §3.4](./I69c-agent-orchestration-canvas-spec.md)） |
| 不缓存 run | 叠加是投影，不是缓存（I69b K2 / INV8） |
| 事件只更新已存在卡片 | 模板中不存在的 `nodeId` → 忽略并计数告警，不补建（[I69c §4.3](./I69c-agent-orchestration-canvas-spec.md)） |
| 未知类别零改写 | 降级渲染保留原始 `kind`，模板数据逐字节不变（OC2） |

---

## 8 交互边界

### 8.1 卡片内零交互控件

| 规则 | 说明 |
|---|---|
| 卡内无输入控件 | 卡片根是 `button`，不得嵌套输入框 / 下拉 / 内层按钮（HTML 语义 + 焦点纪律） |
| 卡内只读 | 参数摘要、附加区行、引用文本、`note` 全部只读 |
| 卡内手势 | 只有整卡选中、拖拽、端口拖出连线（设施既有） |

### 8.2 参数表单落参数面板（侧栏）

| 项 | 落点 |
|---|---|
| 参数表单 | 控件集合：`content` 多行文本（必填）+ 三个下拉（可留空）——控件闭集见 [I69c §3.1](./I69c-agent-orchestration-canvas-spec.md) / WQ7 |
| 参数面板 | 表单的宿主区域：**选中卡片时出现的侧栏面板**；卡片主体恒为只读摘要（§3.1），不随编辑模式换表单 |
| 术语对齐 | 词表 §2.5.2 已明确「参数表单 = 控件集合、宿主可换」「参数面板 = 宿主区域」，本卷落点即该定义下的侧栏宿主，**不构成形态偏离** |

### 8.3 gate 决策入口在卡外

| 规则 | 说明 |
|---|---|
| 只发 `resolve_workflow_gate` | 不自动 resolve、不代填 `note`（I69b K8） |
| 入口位置 | 选中 `waiting` 的 `hitl` 卡时，侧栏 / 浮层出现「通过 / 拒绝 + 备注」表单；卡内只显示 `note` |
| 不阻断编辑 | 等待态不锁画布（[I69c §3.6](./I69c-agent-orchestration-canvas-spec.md) 同理） |

---

## 9 对 I69c 的最小修订与词表同步（同步清单）

本卷先于 I69c 实施，I69c 需在其落点处引用本卷；**不改 I69c 的语义条款**，只补卡片形态的出处。**五处已全部同步（2026-11-24）**，下表保留为同步记录与复核判据。

| # | I69c 位置 | 现文 | 改为 |
|---|---|---|---|
| 1 | 头部「关联」行 | 只列 I69a / I69b / I66 / I68 | 增列本卷（W3 前置：卡片形态） |
| 2 | §2.1 `CanvasCardView` 行「允许的改动」列 | 「不改字段语义；编排侧只填 `CanvasCardView` 已有字段，不扩展类型」 | 「不改既有字段语义；卡片内容形态与三处增量以本卷 §3 / §6 为准」 |
| 3 | §2.1 `presets/cardKinds.ts` 行「W3 / W4 用法」列 | 「只取 `execution-turn` / `execution-hitl` 两类的形与度量」 | 「只取形与度量；内容由本卷 §3 另建构造器产出，不调用 `cardViewOf`」 |
| 4 | §4.4 状态 → 视觉 | 自带一张 `NodeRunState` 7 值映射表（`tone` 用中文语义名） | 删表，改为「卡片级映射唯一出处是本卷 §4.2 / §4.3」，只留叠加层纪律（去双权威） |
| 5 | §1.4 归属矩阵第 4 / 11 行「闭合判据」列 | 「UI15 + §2.1 接缝四规则」「§2.3 第 1 条（只引用不重定义）」 | 前者补 `OC12` / `OC14` 并把「四规则」改为「§2.1 接缝规则全表」，后者换成 `OC13`（元规则不算判据） |

**词表同步（同批，2026-11-24）**：`docs/analysis/ORCHESTRATION-CONCEPT-SEMANTIC-TRANSFER.md` §2.5.2 更新四处——`卡片类别` 由「未定案」改为「定案（W3 收窄，闭集 = `turn` / `hitl`）」；`卡片模式` 去掉「主体」限定并注明宿主不影响模式；`参数表单` 明确「控件集合、宿主可换」；新增 `参数面板`（宿主区域，v1 = 侧栏）。§2.5.4 增「参数面板 ≠ 参数表单」一行。I69c §3.1 同步新增「参数表单与参数面板」一段。

---

## 10 验收（OC1–OC14）

| # | 判据 | 检查方式 |
|---|---|---|
| **OC1** | 编排侧不调用示例构造器 | 依赖方向测试：`features/orchestration/` 不 import `features/observation/canvasContent.ts`；全仓无 `orch*` 代码调用 `cardViewOf` / `exampleCardOf` |
| **OC2** | 类别闭集 + 降级零改写 | 卡片 key ⊆ `turn` / `hitl`；未知 `kind` 走降级卡，序列化后的模板数据与输入逐字节一致 |
| **OC3** | 参数摘要三行规则 | 单测：`content` 截断、`mode_id` 空 → 「会话当前值」、`provider`/`model` 四种组合各一条 |
| **OC4** | `hitl` 无参数区 | 单测：`hitl` 卡 `params === undefined`，附加区高度吸收参数区 |
| **OC5** | 无示例标记 | 断言真实卡片 `exampleBadge === undefined`，且界面无「示例数据」提示 |
| **OC6** | 无出口角标 | 断言编排卡 `exit === undefined` / `null` |
| **OC7** | 端口只画圆点 | 断言 `ports` 的 `typeLabel` 全为空串、无标签视觉输出（`title` 为空） |
| **OC8** | 状态映射逐行 | 单测：7 值各一条，断言 `tone` / 药丸 key / 附加区行；`skipped` 文案含「调度阻断」 |
| **OC9** | 编辑态 / 运行态分界 | 无 run 时无药丸、无 `data-run`；有 run 时色带随 `tone` |
| **OC10** | 卡内零交互控件 | DOM 断言：卡片子树内无 `input` / `select` / `textarea` / 内层 `button`；参数表单与 gate 表单在侧栏 |
| **OC11** | 零 wire 变更 | `npm run contract:verify` 无 diff；无新增 DTO / 命令 / 事件 |
| **OC12** | 设施增量默认关闭 + 观测零回归 | 不传三处增量时快照与现状一致；观测侧投影单测与依赖方向测试全绿 |
| **OC13** | 分组与非卡片对象 | 分组只产出 `CanvasLabelView`；边 `route` 只取 `horizontal` / `vertical`；条件标签只对 `onSuccess` / `onFailure` 生成 |
| **OC14** | 颜色只来自 CSS 变量 | 编排样式无硬编码色值；`tone` 只经 `data-tone` 消费 |

> OC1–OC14 只提供卡片形态的**验收细则**：本文不判定交付，也不判定任何 DoD 是否被覆盖——交付成立与否只看 [I69 §1.4](./I69-agent-orchestration-roadmap.md) 的自证条件与 [I69 §1.4.6](./I69-agent-orchestration-roadmap.md) 的 DU 台账；本文不新增阶段级判据，不改 I69 的波次与退出条件。

---

## 11 未决问题（CQ1–CQ8，已全部定案）

| # | 问题 | 定案 | 落点 |
|---|---|---|---|
| **CQ1** | 卡片类别的用户可见名是否仍带「示例分型」限定 | **不带**：`NodeKind` 闭集已由 [I69a §6.1](./I69a-agent-orchestration-base-spec.md) 冻结，编排卡按「执行卡片 · 轮次 / 人工介入」呈现；词表 §2.5.2 对应行已同批更新为定案（2026-11-24） | §2.2 / §3.1 |
| **CQ2** | 参数表单在卡内还是卡外 | **卡外（参数面板 = 侧栏）**：卡片根是 `button`，卡内不可嵌套控件；卡片主体恒为只读摘要，避免编辑模式换主体引发几何抖动；控件集合与宿主分离的术语见词表 §2.5.2 | §4.1 / §8.2 |
| **CQ3** | 4 个参数如何压进 3 行摘要 | **`provider_id` 与 `model_id` 合成一行**「模型」；`content` 一行；`mode_id` 一行 | §3.1 |
| **CQ4** | 出口角标是否保留 | **不保留**：出口汇点是 L1 观测面语义，编排画布无此对象 | §2.2 / §3.1 |
| **CQ5** | 引用 chip（`turnId` / `assistantMessageId`）可否点击跳转 | **不可点击**：卡内只呈现只读文本；跳转入口在 run 面板 | §4.3 / §8.1 |
| **CQ6** | 运行态用哪条色带 | **整卡左缘 3px**（INC2），与药丸构成双通道；头部 3px 竖条继续表类别，二者不冲突（运行态下类别由徽标承载） | §4.2 / §6 |
| **CQ7** | 分组是否画虚线框 | **画，但走 INC3 可选字段**（`CanvasLabelView.frame`），默认关闭；不画框时只剩组标题标签 | §5.3 / §6 |
| **CQ8** | 概览态（缩放降级）是否实现 | **不实现**：不开放低于 0.5 缩放（I69c WQ5），0.5 下药丸仍可读 | §1.2 / §4.1 |

> **编号空间**：本卷用 `CQ`，与 [I69a §15](./I69a-agent-orchestration-base-spec.md) 的 `OQ`、[I69c §7](./I69c-agent-orchestration-canvas-spec.md) 的 `WQ` 是三套互不相干的编号，跨卷引用必须带前缀。
> **关闭权**：`CQ` 只由本表关闭，只约束卡片形态；涉及词表（CQ1 / CQ2）与上游 `OQ` 的裁定，本卷只能**触发**，须回提对应文档并由其同批次更新，本卷无权改上游。CQ1 / CQ2 对应的词表更新已于 2026-11-24 同批完成（§9「词表同步」）。
> **定案不变量**：CQ1–CQ8 全部关闭后本卷不留「待定形态」；实施期如需推翻任一条，必须先改本表并同步 §2–§8 的落点。

---

## 12 拒绝方案

| # | 方案 | 拒绝理由 |
|---|---|---|
| 1 | 直接复用 `cardViewOf` / `exampleCardOf` 产出编排卡 | 会把 12 条 L1 阶段轨道、「示例」徽标与 L1 出口语义带进 L2 画布（§0） |
| 2 | 把 `CARD_KINDS` 从示例表改成真实表 | 示例表是观测页的编译期常量，改动会让观测页失去示例内容并产生第二真相 |
| 3 | 把示例表扩成「示例 + 真实」双段表 | 一张表承载两种事实等级，取值来源无法在类型上区分 |
| 4 | 在卡片主体内嵌参数表单 | 卡片根是 `button`，嵌套控件违反 HTML 语义与焦点纪律（CQ2） |
| 5 | 卡内放可点击的结果 chip | 同上；且跳转需要按需拉消息，属面板职责（CQ5） |
| 6 | 保留出口角标 | 编排画布无出口汇点，角标无对应语义（CQ4） |
| 7 | 为卡片内容新增编排专用字段（`runState` / `turnId`） | 会让共享设施承载编排语义，违反「设施与内容分层」；改用通用 `detail` 变体 + 文案承载 |
| 8 | 运行态整卡换主体（进度条 / 输出预览） | 需节点结果缓存与前端反推，违反 I69b K2 / I69a D3 / D15 |
| 9 | 用缩放阈值切换概览态 | 不开放低于 0.5 缩放，切换无触发区间（CQ8） |
| 10 | 把分组渲染成卡片或容器卡片 | `group` 不是节点类别（`CONST-NODE-KIND-2`），容器卡片未立项 |

---

## 13 上游承接清单（附录）

| 承接 ID | 内容（不复述，回上游） | 本卷落点 |
|---|---|---|
| `INV5` | 节点类别编译期闭集 `turn` / `hitl` | §2.1 / §2.2 / OC2 |
| `INV11` | 未配置 ≠ 0 | §4.2（预算不在卡片显示） |
| `INV13` | 坐标不进语义 | §7（尺寸 / 锚点不进模板） |
| `D3` | 不缓存节点结果 | §7 / §12 第 8 条 |
| `D11` | 参数只接受常量 | §3.1（摘要只读，无引用形态） |
| `D13` | 坐标与语义分列 | §7 |
| `D18` | `group` 不是节点类别 | §5.3 / §12 第 10 条 |
| `D21` | `hitl` 源无 `always` | §5.2 |
| `D22` | 版本不等即拒加载 | §3.3（降级形态只作防御） |
| `K2` | 状态只来自台账与事件 | §7 |
| `K4` | 预算未配置渲染「未配置」 | §4.2 |
| `K5` | `NodeRunState` 与 `NodeState` 不互借 | §4.2 |
| `K6` | `skipped` 必带「调度阻断」 | §4.2 / OC8 |
| `K8` | gate 只经 `resolve_workflow_gate` | §8.3 |
| `WQ3` | 边端点节点级 | §5.1 / §5.4 |
| `WQ5` | 不开放低于 0.5 缩放 | §1.2 / §4.1 / CQ8 |
| `WQ7` | 参数控件闭集 | §3.1 / §8.2 |

> **行属性（门禁边界）**：`INV` / `D` 行是 [I69a §16.1](./I69a-agent-orchestration-base-spec.md) 的**上游冻结项**，缺一即视为上游成果丢失，本卷不得标记可实施；`K` 行是 [I69b](./I69b-agent-orchestration-contract-spec.md) 的读取纪律、`WQ` 行是 [I69c](./I69c-agent-orchestration-canvas-spec.md) 的形态定案，二者属**并列卷约束**，不进 I69a §16.1 清单，只在两卷间对齐（改任一条须同批次改本卷落点与对应卷条款）。
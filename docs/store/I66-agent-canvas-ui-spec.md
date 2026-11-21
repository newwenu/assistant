# I66 Agent 观测画布 UI 规格（W1）

| 项 | 值 |
|---|---|
| 状态 | 草案（术语与几何已定，待实现） |
| 日期 | 2026-11-21 |
| 术语唯一出处 | `docs/analysis/ORCHESTRATION-CONCEPT-SEMANTIC-TRANSFER.md` §2.5（用户可见词表）/ §2.6（开发原语词表） |
| 事实来源 | `docs/analysis/AGENT-CANVAS-RENDERING-ANALYSIS.md`（命题 P1–P13、缺口 G1–G13、验收 K1–K15） |
| 拓扑出处 | `docs/archived/agent/I55-agent-turn-observability-spec.md`（已归档：§3.1 模板、§3.2 出口、§6.2 渲染） |
| 视觉与主题约束 | `docs/DESIGN.md` §5（双层变量体系、§5.3 禁用 fallback） |
| 影响范围 | `src/stores/agent/observation/template.ts`、`.../projections/graph.ts`、`src/features/observation/`（GraphCanvas / NodeCell / ObservationPage / 新增 LegendPanel）、`observation.module.css`、`src/assets/styles/global.css`、`src/i18n/locales/{zh-CN,en-US}.json` |
| 契约影响 | 无。全部为前端本地视图模型，不进 wire、不触发 `npm run contract:gen` |

> 定位：把「12 格观测栅格」从「一格一状态 + 单条活动边」升级为**可读的静态拓扑图**——全图 12 条边、四相边相位、出口标记、节点选择、图例齐全，且全部几何由模板行列号推导、零 DOM 测量。

---

## 1 范围与边界

### 1.1 解决什么

| # | 目标 | 完成判据 |
|---|---|---|
| 1 | 全图边集可见：不止活动边，12 条边全部渲染并带相位 | K2 / K6 / K7 |
| 2 | 边相位可区分：已通行 / 活动中 / 待通行 / 未通行 | K3 / §7 |
| 3 | 出口可见：终态出口落在一格上，且不混入边集 | K4 / K5 / K8 |
| 4 | 节点可选中：键盘可达、可读、可取消 | K12 / K13 / K14 |
| 5 | 图例齐全：状态 / 证据 / 边 / 出口 / 圈与分支五组，中英双语 | K15 / K16 |

### 1.2 不解决什么

| # | 排除项 | 理由 |
|---|---|---|
| 1 | 任何编辑、连线、拖拽、参数表单 | 观测面只读（不变量 N1 / N2）；可编排画布属 L2，见 §1.3 |
| 2 | L2（turn 之间）编排与 L2 参数表单 | 本规格只做 W1 观测栅格。L2 画布要引入「盒子模型 + 自由坐标 + 端口句柄 + 参数表单」，与 W1 的「格子模型 + 推导坐标」是两套视图模型，混在一份规格会导致几何规则互相污染。L2 另立规格 |
| 3 | 卡片/标记两级缩放形态 | V1 只有「格子」一级（§2.5 四态表）；另外两级是反原语（§2.6），不建符号 |
| 4 | 时间线与格子联动高亮 | 需要跨面板 hover 状态与双向选中语义，V1 代价大于收益（OQ-3） |
| 5 | 百分比进度、逐秒跳动的计时 | 无观测时钟，不引入 JS 定时器（§11.4） |
| 6 | 引入图形库（reactflow / dagre / d3） | 12 个固定格 + 12 条推导边，DOM + CSS Grid 足够；引库会带入自己的坐标系、尺寸测量与主题体系，直接违反 N7 与主题约束 |
| 7 | 导出图片、分享链接 | 无需求来源 |
| 8 | 多圈并置（迭代矩阵） | `projectGraph` 只投影当前圈（§9.1）。2 行 × 11 列是常量几何，多圈并置要按圈数复制行，直接破坏常量几何；且 V1 回答的是「这一轮走到哪」，不是「历史对比」。历史圈只在 inspector 时间线里 |
| 9 | 全局快捷键 / 命令面板入口 | 快捷键表的生成源在 Rust（`DEFAULT_SHORTCUT_PATTERNS` → `npm run contract:gen`），加一条画布快捷键要改 Rust 常量并重新生成，不能前端手改（§3） |

### 1.3 与分析的差异（以本规格为准）

分析文档 §4.6 对几何给的是「候选方案」，本规格在落笔时按可实施性收敛。以下差异均为**有意收窄**，不是遗漏。

| # | 分析表述 | 本规格 | 理由 |
|---|---|---|---|
| D-1 | 出口渲染为「悬空短桩 + 汇点徽标」在格子外 | 出口收进格子右下角，用「内折角括号 + 汇点方块」（§8.2） | 12 格中 10 格的右边框紧邻下一格入锚点，8px 列间距放不下短桩 + 汇点；收进格内零碰撞，且同一时刻只有一格带出口，不会与常规边混淆 |
| D-2 | 回边可走「行间隙」或「外轨」 | 固定走顶部外轨（§6.3） | 行间隙要避让 ⑦ 底锚点与 ⑧ 顶锚点，且 ⑫→① 要横跨 11 列，走行间隙会横穿整行；外轨只与两个角相接 |
| D-3 | 端口桩（悬空短线）表达「可接」 | 不画桩。锚点即「格子边框上的交点」（§6.2） | 观测面不可接，桩会暗示可连线；且桩需要独立定位，与 N7 冲突 |
| D-4 | 百分比进度环 | 保留时长数字，文案改为「按事件刷新」（§11.4） | 无时钟源，百分比必然靠定时器伪造 |
| D-5 | 边可由 SVG/Canvas 绘制 | 只用 DOM + CSS Grid（§6.8） | 约束来源见 §3 |
| D-6 | 列宽可自动缩放 | 固定最小列宽 + 横向滚动（§6.7） | 缩放需测量，违反 N7 |
| D-7 | 选中「格子」（节点 × 圈） | 选中「节点」（§9.1） | 同一时刻只有一个圈可见（`projectGraph` 只投影当前圈），选圈是无意义自由度 |

---

## 2 术语引用（不重复定义）

本规格的每一个名词都指向词表，本文**不新增术语、不重复定义**。

| 层 | 术语 | 出处 |
|---|---|---|
| 用户可见 | 格子、卡片、连线、锚点、出口、图例、圈、工具分支 | 概念清单 §2.5 |
| 用户可见 | 填 vs 接、参数表单、参数项、参数摘要、卡片主体四态 | 概念清单 §2.5 |
| 开发原语 | 折叠、投影、调度、抑制、帧合并、锚点推导、模板原语、边集原语 | 概念清单 §2.6 |
| 开发原语 | 反原语清单（9 条） | 概念清单 §2.6 |

**用词纪律**（与本规格的行文约定）：

| 禁止 | 应写 | 说明 |
|---|---|---|
| 画布（指观测面时） | 观测栅格 / 栅格 | 「画布」保留给可编辑的编排画布（L2） |
| 节点 | 格子 | 用户可见的一格；`node` 只用于代码符号与 wire 字段 |
| 端口 | 锚点 | 锚点不是可交互实体，只是边框交点 |
| 连线的「一条边」 | 一条边 | 「连线」是动作/关系，计数单位是「条」 |
| 选中格子 | 选中节点 | §9.1 |
| 裸「节点模板」 | 「turn 模板」/ `AGENT_NODE_TEMPLATE` | 两层图模板必须分开命名（词表 §2.5.4）：本模板是 turn 模板（12 节点、常量），将来用户可编辑的是工作流模板（数据）。混用是最大返工来源 |
| 「未完成」用于边 | 「未通行」（`untaken`） | `observation.graph.unfinished` 专指「turn 收尾时格子仍停在中间态」，不得挪作边相位文案 |
| 「边」与「接线」互换 | 边（L1 观测）/ 接线（L2 编排） | 两者不同层；观测栅格里不存在接线 |

**正向纪律**：写用户词时必须同时给出对应原语词（如「活动边」↔ `activeEdge`、「相位」↔ `EdgePhase`）。只给一边，文案与实现就会漂移（词表 §2.6.7 第 4 条）。

**事实效力**：`I55 §3.1` 的 12 节点表是**抄录对象**，不是权威来源——权威在内核 Rust 代码，表只是转录，且节点数会随内核结构演进（I55 §11.3 自己标了「会变」）。因此本规格所有「12 格」「12 条边」的表述都是**快照值**，新增/删除节点时以 `template.ts` 的编译期锁为准。

---

## 3 约束来源与效力

| 约束 | 来源 | 效力 |
|---|---|---|
| 只读、不参与控制 | 概念清单 O6 / G7；I55 §6.2 | 硬：违反即不通过 |
| 渲染侧禁止 DOM 测量与 ResizeObserver | 分析 P11 / N7 | 硬 |
| 颜色只取 antd 变量、禁止硬编码与 fallback | DESIGN §5.3；项目规则「antd 主题对齐」 | 硬 |
| 文案只取 i18n、禁止内联中文 | 项目规则「i18n」；分析 P12 | 硬 |
| 键盘可达 + 焦点可见 + 非仅靠颜色编码 | 分析 P12 / P13 | 硬 |
| 快捷键若全局生效须注册 keymap | 分析 P13 | 软：本规格只用容器内 Esc（§9.4），不需注册。将来加全局快捷键**不能**在前端手写：`shortcutDefaults.generated.ts` 由 `npm run contract:gen` 从 Rust `DEFAULT_SHORTCUT_PATTERNS` 生成，手改会被 `contract:verify` 的 `git diff --exit-code` 拦下。加一条要改 Rust 常量并重新生成 |
| 单一失效调度点、wire 不带本地 UI 状态 | 分析 P6 / P9；项目规则「wire 不承载本地 UI 状态」 | 硬 |
| 动画只由状态驱动 | 分析 P10 | 硬 |

---

## 4 不变量

| # | 不变量 | 判据 |
|---|---|---|
| N1 | 栅格不是控制源：不写 store、不发事件、不回写 wire | 代码审查：`features/observation/` 无 `useObservationStore.setState` / `invoke` |
| N2 | 观测面只读：所有交互只是本地视图状态（选中），不改变观测数据 | K14 |
| N3 | 单一失效调度点：投影重算只经 `subscribeProjections` | `projections/*` 不被 `features/` 直接调用（`dependencyDirection.test.ts`） |
| N4 | wire 不承载本地 UI 状态：`selectedNodeId` 不进 `GraphView`、不进任何 DTO | K8 / typecheck |
| N5 | 边集与活动边同源：`edges` 与 `activeEdge` 出自同一份 `TEMPLATE_EDGES` | K3 |
| N6 | 格子必须不透明：任何格子不得使用 `opacity < 1` | K11 / §6.5 |
| N7 | 几何零测量：位置全部由模板 `row` / `column` 推导 | K9 |
| N8 | 出口不混入边集：`edges` 中不含出口边，出口只由 `exit` 字段表达 | K8 |
| N9 | 视图模型只从投影派生：除 `selectedNodeId` 外，`GraphCanvas` 不持有可跨帧存活的派生数据 | K14 |

---

## 5 视图模型（投影契约）

### 5.1 边集原语：`TEMPLATE_EDGES`

现状：边只由 `activeEdgeOf` 里的 `templatePredecessor`（按 `order - 1` 隐式推邻接）临时算出一条。这导致「全图边集」无处可取，画布与投影无法共享同一份边定义。

改为编译期常量，与 `AGENT_NODE_TEMPLATE` 同处 `template.ts`：

```ts
export type TemplateEdgeKind = "forward" | "loop";

export interface TemplateEdge {
  from: AgentNodeId;
  to: AgentNodeId;
  kind: TemplateEdgeKind;
}

/** 12 条边：11 条正向（相邻 order）+ 1 条回边 A（⑫ → ①，I55 §3.1）。 */
export const TEMPLATE_EDGES: readonly TemplateEdge[] = [/* ... */];
```

构成规则（**唯一来源**，不得在别处再推导一遍）：

| 类 | 数量 | 定义 | 举例 |
|---|---|---|---|
| 正向 | 11 | 对每个 `order >= 1` 的条目，连 `order - 1` 的条目 | `admit → prepare` … `persist-tools → after-iteration` |
| 回边 A | 1 | `after-iteration → admit` | ⑫ → ① |

`templatePredecessor(node)` 删除；需要前驱时改为 `TEMPLATE_EDGES.find(e => e.to === node && e.kind === "forward")`。`activeEdgeOf` 的**判定逻辑不变**（仍是「最靠后的进行中格子」+ ① 在 `iteration > 1` 时取回边），只把产物从「新建对象」改为「指向 `edges` 中的同一条」。

### 5.2 `GraphEdgeView` 增加相位

```ts
export type EdgePhase = "untaken" | "idle" | "passed" | "active";

export interface GraphEdgeView {
  from: AgentNodeId;
  to: AgentNodeId;
  kind: TemplateEdgeKind;
  phase: EdgePhase;
}

export interface GraphExitView {
  node: AgentNodeId;
  status: TurnObservationStatus;
}

export interface GraphView {
  // ... 现有字段不变
  edges: readonly GraphEdgeView[];
  /** 恒等于 `edges.find(e => e.phase === "active") ?? null`（同一引用，非拷贝）。 */
  activeEdge: GraphEdgeView | null;
  /** 出口不是节点、不是边（I55 §3.2）；`null` = 本实例没有出口。 */
  exit: GraphExitView | null;
}
```

字段纪律：

| 字段 | 允许 | 禁止 |
|---|---|---|
| `edges[i]` | `from` / `to` / `kind` / `phase` | `id` / `handle` / `label` / `path` / 任何像素坐标 |
| `activeEdge` | 与 `edges[i]` 同一引用 | 新建对象（否则 K3 无法用 `===` 判） |
| `exit` | `node` / `status` | 混入 `edges` |

### 5.3 相位判定（纯函数，无 DOM、无时钟）

设 `reached(n) = n.state !== "pending"`，`skipped(n) = n.state === "skipped"`。

正向边 `p → c`，按序取首个命中：

| 序 | 条件 | 相位 |
|---|---|---|
| 1 | 该边是 `activeEdge` | `active` |
| 2 | `skipped(c)` | `untaken` |
| 3 | `reached(c)` | `passed` |
| 4 | `reached(p)` | `idle` |
| 5 | 其余 | `untaken` |

回边 `⑫ → ①`，按序取首个命中：

| 序 | 条件 | 相位 |
|---|---|---|
| 1 | 该边是 `activeEdge` | `active` |
| 2 | `iteration >= 2` | `passed` |
| 3 | `reached(⑫)` | `idle` |
| 4 | 其余 | `untaken` |

说明：

- **计算顺序**：先由 `activeEdgeOf` 定出「哪条边是活动边」（判定依据是格子状态，与相位无关），再据此为 12 条边逐条赋 `phase`。相位表第 1 行引用的 `activeEdge` 指的就是这个已定好的身份，不是循环依赖。
- 第 2 条把「⑦ 判定不走工具分支」与「还没走到」合并为同一个 `untaken`：两者语义都是「这条边没有通行」，共用一种视觉（§7）。这是刻意的收敛，不是遗漏。
- 回边不能用 `reached(①)` 判 `passed`：① 只有一格（`iteration = 0`），第一圈跑完它也 `finished`，会把「没发生过的回边」判成已通行。故必须用圈数判。
- `error` / `cancelled` 视为已到达（边通行了，只是目的地出错/被取消）。

### 5.4 出口判定

候选集是两处出处的**并集**：I55 §3.2（已归档）的出口表 + 分析 §4.6 补的 ④。

| 来源 | `failed` | `cancelled` |
|---|---|---|
| I55 §3.2 | ⑤ / ⑥ | ⑤ / ⑨ |
| 分析 §4.6 | ④ | ④ |
| 本规格采用（并集） | ④ / ⑤ / ⑥ | ④ / ⑤ / ⑨ |

并集不会误报：未真正触发的候选仍是 `pending`，而消歧规则只取「非 `pending` 中最后到达者」，多余候选自然被跳过。

| `status` | 候选节点 | 消歧规则 | 结果 |
|---|---|---|---|
| `running` | — | — | `null`（无出口） |
| `completed` | ⑦ | 唯一 | ⑦ |
| `wrap_up` | ⑦ | 唯一 | ⑦ |
| `halted_by_budget` | ③ | 唯一 | ③ |
| `stopped` | ③ / ⑩ / ⑫ | 取最后到达者 | — |
| `cancelled` | ④ / ⑤ / ⑨ | 取最后到达者 | — |
| `failed` | ④ / ⑤ / ⑥ | 取最后到达者 | — |

「最后到达者」定义：在候选集中，先按 `iteration` 降序、再按模板 `order` 降序，取**首个非 `pending`** 的节点；若候选全部 `pending`，取 `order` 最小者（此时出口落在流程最早可能中断的位置）。

| 场景 | 候选与状态 | 取谁 |
|---|---|---|
| `failed` 于 ④ | ④ error，⑤ ⑥ pending | ④（非 pending 中 order 最大） |
| `failed` 于 ⑥ | ④ ⑤ finished，⑥ error | ⑥ |
| `cancelled` 于 ⑨ | ④ ⑤ finished，⑨ cancelled | ⑨ |
| `stopped` 于 ⑫ | ⑩ ⑪ ⑫ 皆 finished | ⑫ |
| `stopped` 全 pending | — | ③（order 最小） |

判定函数必须**纯**（入参只有 `GraphView` 已算好的节点数组与 `status`），放在 `projections/graph.ts`，由 `projectGraph` 一并产出。

**⑥ 的唯一例外**：`persist-assistant` 在同一迭代可**多次**落库（收尾 / 取消 / 预算中止），每次都发一条 `finished`，且不重发 `running`（I55 §4.6 第 4 条）。本判定按**节点聚合状态**（`GraphNodeView.state`）而不是按事实序列，所以多次 `finished` 不影响结果；但若将来改成按 fact 判定，必须按 `seq` 取最后一条，不能按数组顺序——否则出口会在 ⑥ 的多条 `finished` 之间抖动。其余 11 个节点每格每圈恰好一条终态。

---

## 6 几何模型

### 6.1 网格轨道

模板是 2 行 11 列（`column` 0..10），回边需要一条顶部外轨，故栅格为 **3 行**。以下为在现有 `.canvas`（`observation.module.css`）上增补后的完整定义：

```css
.canvas {
  display: grid;
  grid-template-columns: repeat(11, minmax(var(--canvas-cell-min-width), 1fr));
  grid-template-rows:
    var(--canvas-loop-gutter)                    /* 1 行：回边外轨 */
    minmax(var(--canvas-cell-min-height), auto)  /* 2 行：模板 row 0（①–⑦） */
    minmax(var(--canvas-cell-min-height), auto); /* 3 行：模板 row 1（⑧–⑫） */
  column-gap: var(--canvas-gap);
  row-gap: var(--canvas-gap);
  align-items: stretch;
}
```

现状差异（三条都要改）：

| # | 现状 | 改为 |
|---|---|---|
| 1 | `.canvas` 只有 `display: grid; gap: 8px; align-items: stretch`，**无 `grid-template-rows`** | 补 3 条轨道（外轨 + 两行格子） |
| 2 | 列定义在 `GraphCanvas.tsx` 的内联 `canvasGridStyle`（`repeat(11, minmax(132px, 1fr))`） | 移到 `.canvas` 规则内，列数仍由 `CANVAS_COLUMNS` 模板推导后以内联 `gridTemplateColumns` 覆盖（保留「不写字面量」的做法，只是把 `132px` 换成变量） |
| 3 | `gap: 8px` 字面量 | `column-gap` / `row-gap` 均用 `var(--canvas-gap)` |

| 模板坐标 | 栅格放置（1 起） |
|---|---|
| `row 0` | `grid-row: 2` |
| `row 1` | `grid-row: 3` |
| `column c` | `grid-column: c + 1` |
| 回边外轨 | `grid-row: 1` |

**现状必改**：`NodeCell` 当前用 `gridRow: node.row + 1` 自放置，加入外轨后必须改为 `node.row + 2`，否则全部 12 格会上移一行、盖住外轨。

同一行内所有格子 `align-self: stretch`，因此行内格子等高、垂直中点对齐——这是「同行边只需 `align-self: center`」的前提。

### 6.2 锚点与边的放置规则

锚点 = 格子边框上的交点，不画桩、不测量：

| 边类 | 出锚点 | 入锚点 | 放置规则（`from = (r0,c0)`，`to = (r1,c1)`） |
|---|---|---|---|
| 同行正向 | 左格右边框中点 | 右格左边框中点 | `grid-column: c0+1 / c0+3`；`grid-row: r0+2`；`align-self: center`；`height: 1px` |
| 跨行正向 | 上格底边中点 | 下格顶边中点 | `grid-column: c0+1`；`grid-row: r0+2 / r0+4`；`justify-self: center`；`width: 1px` |
| 回边（三段） | ⑫ 顶边右角 | ① 顶边左角 | 见 §6.3 |

边集按几何分类的实测数量（`TEMPLATE_EDGES` 必须与之一致，K2 / K6）：

| 类 | 数量 | 具体 |
|---|---|---|
| 同行正向 | 10 | ①→② … ⑥→⑦（row 0），⑧→⑨ … ⑪→⑫（row 1） |
| 跨行正向 | 1 | ⑦ → ⑧（同为 `column 6`，`row 0` → `row 1`） |
| 回边 | 1 | ⑫ → ①（三段元素） |

合计 11 条正向 + 1 条回边 = 12 条；渲染元素 11 + 3 = 14 个（§6.5）。

同行正向边元素横跨「左格 + 列间距 + 右格」两个轨道，可见部分正好是那 8px 列间距，两端精确落在两格边框上；跨行同理。**没有任何一处需要读取像素尺寸。**

### 6.3 回边三段

回边不参与 §6.2 的「两点直连」，改为三段拼出「上折 → 横穿 → 下折」：

| 段 | 放置 | 说明 |
|---|---|---|
| 横段 | `grid-row: 1; grid-column: 1 / -1; align-self: end; height: 1px` | 贴外轨底边，横跨全部 11 列，两端正好落在第 1 列左边框与第 11 列右边框 |
| 右竖段 | `grid-row: 1 / 4; grid-column: 11 / 12; justify-self: end; width: 1px; align-self: stretch; margin-block-start: var(--canvas-loop-gutter)` | 从外轨底边下行，经 `row 0` 第 11 列空格（全程可见），再进入 ⑫ 所在轨道；进入 ⑫ 的部分被覆盖，**可见终点 = ⑫ 顶边右角** |
| 左竖段 | 同上，但 `grid-row: 1 / 3`、`grid-column: 1 / 2`、`justify-self: start` | 从外轨底边下行，进入 ① 所在轨道；进入 ① 的部分被覆盖，**可见终点 = ① 顶边左角** |

竖段跨度规则：**`grid-row: 1 / (to.row + 3)`**——从外轨起，一直跨到入锚点所在轨道的底边（`to.row` 为 0 → `1 / 3`；为 1 → `1 / 4`）。跨度多算的部分由不透明格子覆盖，不算少算，因此不会出现「竖段够不到格子」的断口。之所以宁可跨到底边也不精确停在顶边：停在顶边需要知道格子顶边位置，而跨到轨道底边只需轨道号——**后者是模板常量，前者要靠测量**。

要点：

- 竖段用 **`align-self: stretch` + `margin-block-start: var(--canvas-loop-gutter)`**，而不是 `height: calc(100% - …)`：拉伸盒减去起始外边距后，顶边正好落在横段所在的那条线上，底边正好落在所跨栅格区域的底边。**全图零 `calc`、零百分比高度、零实测值**，也不依赖「百分比高度能否在 `auto` 轨道内解析」这类浏览器差异。
- 竖段伸进格内的部分被不透明格子覆盖（N6），因此视觉上只呈现「外轨 → 格子顶边」的连接段。
- 横段左端加一个 5px 左向三角（`::after`），位置 `inset-inline-start: 4px`。全图只有这一个箭头（§7.3）。

### 6.4 出口标记（格内右下角）

格子加 `position: relative`，出口标记由两个伪元素构成，只在 `data-exit="true"` 的格子上显示：

| 元素 | 形状 | 位置 |
|---|---|---|
| `::before` 内折角括号 | `8px × 8px`，只画 `border-inline-end` + `border-block-end` 各 1px | `inset-inline-end: 6px; inset-block-end: 6px` |
| `::after` 汇点方块 | `6px × 6px` 实心 | `inset-inline-end: 3px; inset-block-end: 3px` |

括号表示「流出」，方块表示「终止于此」。颜色取出口语义色（§8.3）。

**三个必须写死的实现前提**：

| # | 前提 | 原因 |
|---|---|---|
| 1 | 两个伪元素都必须 `position: absolute` | `.cell` 是 `display: grid` 且带 `gap: 2px`。未脱离文档流的 `::before` / `::after` 会变成该网格的隐式网格项，各占一行，把格内三段内容整体挤开 |
| 2 | `.cell` 必须新增 `position: relative` | 现状没有；绝对定位的伪元素会以最近的已定位祖先为参照，落到栅格外 |
| 3 | 伪元素不参与 `.cell` 的 `align-content: start` | 同上，脱离文档流后不占行 |

`.cell` 现状无任何 `::before` / `::after` 规则，不会与既有样式冲突。

### 6.5 渲染顺序与不透明要求

单层网格，所有元素都是该网格的直接子元素，按 DOM 顺序绘制：

| 顺序 | 元素 | 数量 |
|---|---|---|
| 1 | 边元素 | 14（11 正向 + 回边 3 段） |
| 2 | 格子 | 12 |
| 3 | 行内标注（工具分支） | 1 |

行内标注的放置（工具分支只有 `row 1` 才可能进入，左侧 6 列在 `row 1` 是空格，标注正好填在那里）：

| 项 | 规则 |
|---|---|
| 位置 | `grid-row: 3; grid-column: 1 / span N`，`N` = `row 1` 中最小的 `column`（当前为 6，即 ⑧ 之前的空白列数） |
| 现状必改 | `TOOL_LANE_LABEL_COLUMNS = 6` 是字面量，与「画布列数由模板推出」的既有做法不一致；改为由 `AGENT_NODE_TEMPLATE` 里 `row === 1` 的最小 `column` 推导 |
| 文案 | 仍用 `observation.graph.tool_lane` 全文，`font-size: 11px`，允许在 6 列内折行 |
| 不参与编码 | 标注是灰字说明，不带边框/背景，不占锚点 |

**N6 的落实**：格子必须覆盖其下方边线，因此

- 现状 `observation.module.css` 对 `derived` 用 `opacity: 0.72`、对 `assumed` 用 `opacity: 0.8`，而 `pending` 格子恰好全是 `derived` —— 12 格里最多 12 格半透明，边线会从格子里透出来。
- 改法：**删除这两处 `opacity`**，`derived` / `assumed` 的「不确定感」改由已有虚线边框 + 文字色降级承担；`pending` 保持 `--ant-color-bg-layout` 不透明底。
- 判据：`grep -n "opacity" src/features/observation/observation.module.css` 不得命中格子规则。

**同时修掉的两处文字对比度问题**（都不是本设计引入，是现状缺陷，W1b 顺手修）：

| # | 现状 | 改为 | 原因 |
|---|---|---|---|
| 1 | `.cell[data-state="pending"] { color: var(--ant-color-text-quaternary) }` | `var(--ant-color-text-tertiary)` | `pending` 是「尚未到达」，不是「禁用」；11px 小字在浅色主题下 quaternary 低于 4.5:1 |
| 2 | `.cell[data-state="skipped"]` / `[data-state="cancelled"]` 已为 `tertiary` | 保持 | 已达标，不需要再降 |

**颜色语义硬约束**：`--ant-color-error`（红）**只允许出现在** `state === "error"` 与 `status === "failed"` 两处。以下情形一律不得用红：

| 情形 | 正确色 | 理由 |
|---|---|---|
| ③ `brake` 的 `verdict: stop`（预算耗尽） | `success`（正常 `finished`） | 预算用尽是**正常收尾**，不是失败（I55 §5.1 末条） |
| `status === "halted_by_budget"` | `warning` | 用户可见的「提前结束」，不是错误 |
| `status === "stopped"` / `"cancelled"` | 中性灰（`text-tertiary`） | 用户主动停止，不是故障 |

把「预算用尽」画成红叉是**语义错误**，不是审美差异。

### 6.6 布局尺寸变量

尺寸常量集中在 `src/assets/styles/global.css` 的 `:root`（与主题无关，不进 `[data-theme="dark"]` 块）。

依据：`docs/DESIGN.md` §5.2 的变量前缀表已登记 `--sidebar-` / `--titlebar-` / `--orb-` 三组**布局尺寸**前缀（`--orb-ball-size` 等就在 `:root`），说明「布局尺寸进 `:root`、按域前缀分组」是本项目既有做法；§5.3 禁的是「在 `:root` 外定义主题变量」，布局尺寸不属主题变量。新增 `--canvas-` 组需在 §5.2 表里登记，否则下一个人会以为它是野生变量。

| 变量 | 值 | 用途 |
|---|---|---|
| `--canvas-gap` | `8px` | 行/列间距（搬自 `.canvas` 的 `gap: 8px`，`.canvasPane` 的 `gap: 8px` 不动） |
| `--canvas-loop-gutter` | `16px` | 回边外轨高度 |
| `--canvas-cell-min-width` | `132px` | 列最小宽度（搬自 `GraphCanvas` 的 `minmax(132px, 1fr)`） |
| `--canvas-cell-min-height` | `78px` | 行最小高度（搬自 `.cell` 的 `min-height: 78px`） |
| `--canvas-exit-size` | `6px` | 出口汇点方块边长 |

搬运纪律：三个「搬自」变量落地后，原处的字面量必须删除，不允许变量与字面量并存（否则改变量不生效，形成第二份真值）。

需在 `docs/DESIGN.md` §5.2 前缀表增补一行：`--canvas-` | 观测栅格布局 | `--canvas-gap`, `--canvas-loop-gutter`。

线宽（1px / 2px）直接写在规则里——DESIGN §5.3 只约束颜色与阴影的硬编码，不约束线宽。

### 6.6.1 圆角兜底修正

现状 `observation.module.css` 有 **3 处** `var(--radius-md, 8px)`（`.instanceItem`、`.laneLabel`、`.cell`）。`--radius-md` 在 `global.css` 的 `:root` 与 `[data-theme="dark"]` 中**均未定义**，因此这 3 处永远取硬编码兜底 `8px`，违反 DESIGN §5.3「禁用 `--ant-*` 变量 + 硬编码 fallback」的同源规则（自造变量名 + 硬编码兜底，问题等价）。

| 处置 | 内容 |
|---|---|
| 替换 | 3 处全部改为 `var(--ant-border-radius)`（antd 6 默认输出 CSS 变量，`src/vault/vault.module.css` 已在用同一写法） |
| 视觉影响 | antd 6 的 `borderRadius` 在 `theme.ts` 中配置为 `6px`，故圆角由 `8px` → `6px`。这是**与主题对齐的修正**，不是回归 |
| 另一种写法 | `var(--radius)`（`global.css` L37 定义，同为 `6px`，DESIGN §5.2 已登记）也能得到同一像素值。本规格取 `--ant-border-radius`，因为它与项目规则「仅使用 antd CSS 变量」直接一致，且 vault 已有同款用法；两种写法不得混用，一处组件内只用一种 |
| 判据 | `rg "radius-md" src/features/observation/` 为空 |

不动的圆角：`.cellBadge` 的 `border-radius: 4px` 保留。它是徽标内部的局部尺寸（与 `font-size: 11px` 配套），不是主题级圆角；改成 `--ant-border-radius`（6px）会让小徽标显得圆钝。

### 6.7 溢出与最小尺寸

| 场景 | 行为 |
|---|---|
| 容器宽度 < 11 × 132 + 10 × 8 = 1532px | 栅格容器 `overflow-x: auto`，横向滚动；**不缩放、不折叠列** |
| 容器高度不足 | 纵向由页面自然滚动（`mainPane` 已有滚动） |
| 极窄窗口 | 允许横向滚动条出现，不做响应式降级（V1 明确接受） |

### 6.8 禁止事项

| 禁止 | 原因 |
|---|---|
| `getBoundingClientRect` / `offsetWidth` / `clientWidth` / `ResizeObserver` / `MutationObserver` | N7；任何测量都会把「布局完成」变成渲染前置条件，破坏纯投影 |
| SVG / Canvas 画边 | D-5；会引入第二套坐标系与主题体系 |
| 绝对定位 + `px` 坐标写边 | 同上；且窗口变化即失真 |
| `transform: scale()` 做缩放 | 会让 1px 边变成非整数像素，出现断裂 |
| 给边设 `z-index` 高于格子 | 边会横穿文字（N6 已用 DOM 顺序解决） |

---

## 7 边的相位与视觉编码

### 7.1 编码表

| 相位 | 线型 | 颜色 | 附加 | 语义（图例文案） |
|---|---|---|---|---|
| `passed` | 1px solid | `--ant-color-text-tertiary` | — | 已通行 |
| `active` | 2px solid | `--ant-color-primary` | 呼吸动画（`opacity` 0.55 ↔ 1，1.6s） | 活动边 |
| `idle` | 1px dashed | `--ant-color-text-quaternary` | — | 待通行 |
| `untaken` | 1px dotted | `--ant-color-border` | — | 未通行 |

编码纪律：

- 四个相位用**三种线型**（solid / dashed / dotted）打底，再叠加线宽与颜色深浅：任意两个相位至少在一个非颜色通道上不同（P12）。
- 为什么 `idle` 与 `untaken` 不能都用 dashed：两者语义只差「还没走到」与「不会走到」，若线型也相同就只剩颜色差异，违反「非仅靠颜色编码」。故 `untaken` 取 dotted——与格子证据等级里 `assumed` 的 dotted 同一视觉语言，含义也一致：**虚线 = 推测/未定，点线 = 更低置信**。
- `active` 与 `passed` 靠线宽 + 颜色 + 动效三通道区分，不会误读。
- `active` 全图至多一条（K3）；`active` 存在时不再额外画「活动边」文字标签——图例已覆盖。

### 7.2 回边的相位色

回边三段共用同一相位样式（同一 `data-phase`）。三段必须同时切换，不允许一段 active、两段 passed。

### 7.3 方向

| 边类 | 方向表达 |
|---|---|
| 正向（11 条） | 不画箭头。由「左 → 右」阅读顺序与锚点位置承载 |
| 回边 | 横段左端一个 5px 左向三角，明确「回到左侧起点」 |

理由：正向边的可见段只有 8px，箭头会与相邻格子的边框视觉打架；回边横跨 11 列且方向与阅读顺序相反，必须显式标向。

### 7.4 活动边的端点不再由格子二次标注

现状 `NodeCell` 收 `activeSource` / `activeTarget` 两个布尔 prop，并在格子上打 `data-active-source` / `data-active-target`。

全图边集渲染后，这两个标注失去意义：活动边的两个端点已由那条 2px 主色边直接指出，再给格子加一层高亮等于用第三个通道重复同一信息，且会与格子的状态编码抢注意力。

| 处置 | 内容 |
|---|---|
| 删除 prop | `NodeCellProps.activeSource` / `activeTarget` |
| 删除属性 | `data-active-source` / `data-active-target` |
| 删除样式 | `observation.module.css` 中对应选择器 |
| 保留 | 进行中格子的 `data-state="running"` 呼吸动画（§11.3） |

---

## 8 出口

### 8.1 判定

见 §5.4。`projectGraph` 直接产出 `exit`，画布不做二次判断。

### 8.2 几何

见 §6.4。出口标记**始终在格内右下角**，不越出格子、不占用列间距、不新增边元素。

### 8.3 视觉编码

出口色必须与页面上已有的两处状态色**完全同表**：`GraphCanvas` 的 `STATUS_TAG_COLOR`、`ObservationPage` 的 `statusColor()`。两者当前一致，出口直接复用，不新增第三套映射。

| `status` | 出口色（括号 + 方块） | 对应 antd tag 色 |
|---|---|---|
| `completed` | `--ant-color-success` | `success` |
| `failed` | `--ant-color-error` | `error` |
| `halted_by_budget` | `--ant-color-warning` | `warning` |
| `wrap_up` | `--ant-color-warning` | `warning` |
| `stopped` | `--ant-color-text-tertiary` | `default` |
| `cancelled` | `--ant-color-text-tertiary` | `default` |
| `running` | 不适用（无出口） | `processing` |

纪律：`stopped` / `cancelled` 是**中性**收尾，不是异常，因此取中性灰而非警告色——否则「用户主动停止」会被读成「出问题了」。这条与 I55 §3.2「`brake` 的 `stop` 不是节点失败」同口径。中性灰选 `--ant-color-text-tertiary`，与 `.cell[data-state="cancelled"]` 的文字色同源，保证「取消格子」与「取消出口」看起来是同一件事。

实现约束：`status → 语义档位`（`processing` / `success` / `error` / `warning` / `default`，与 antd `Tag` 的 `color` 取值同集）应只定义一次（如 `STATUS_LEVEL: Record<TurnObservationStatus, …>`），`GraphCanvas` 的 `STATUS_TAG_COLOR` 与 `ObservationPage` 的 `statusColor()` 都从它派生，出口色再由档位映射到 CSS 变量。这样「加一个 status」只需改一处，不会出现「tag 是灰、出口是黄」。

### 8.4 文案

| 位置 | 内容 |
|---|---|
| 格子 `aria-label` | 见 §12.4 的拼接规则（含出口时末段追加「出口 + status 名」） |
| 图例「出口」组 | `observation.graph.exit` + 6 个终态 `observation.status.*` 取值 |
| 格子可见文字 | 不加「出口」二字，只留角标（避免 12 格里反复出现同词） |

### 8.5 无出口的情形

| 情形 | 表现 |
|---|---|
| `status === "running"` | 全部格子 `data-exit="false"` |
| 实例被 LRU 淘汰 | 不渲染栅格（现有 `Alert` 分支），出口无意义 |
| 候选节点尚未建立（`cells` 为空） | 按 §5.4 的「全部 pending」兜底取 `order` 最小者，出口仍显示 |

**evicted 的归属边界**（必须写死，否则将来把栅格挂到别处会露出假象）：

| 项 | 规则 |
|---|---|
| 谁处理 | `GraphCanvas` **不**处理 evicted；调用方（`ObservationPage`）必须先判 `selected.evicted` 并走 `Alert` |
| 原因 | `summarize()` 会把 `cells` 清空、`timeline` 清空，而 `projectGraph` 仍按模板恒输出 12 格全 `pending` + `derived`。画布本身无法区分「真的全 pending」与「摘要态」，只有调用方知道 |
| 复活 | `commit()` 对 evicted 实例收新事件会 `evicted: false` 复活（I55 §6.3 第 3 条的正确行为）。UI 上**不做过渡动画、不弹提示**——加 fade 会掩盖「摘要态 → 完整态」的性质跳变 |

**已核验（无悬挂）**：实例被 LRU 删除时，`flushPending` 取到 `observation = null` → `listener(null)` → `useProjections` 返回 `null` → 页面走 `TemplatePreview` 空态；同时 `useObservationInstances` 重新派生列表，`selected` 回落 `instances[0]`，列表空则 `null` → 空态。两条链路都收敛到空态，无悬挂引用、无白屏。

---

## 9 选择与键盘

### 9.1 选择粒度

选中对象是**节点**，不是格子、不是边。理由见 D-7：`projectGraph` 只投影当前圈，一格即一个节点，选圈是无意义自由度。

### 9.2 状态位置

| 状态 | 位置 | 是否进投影 |
|---|---|---|
| `selectedNodeId: AgentNodeId \| null` | `ObservationPage` 本地 `useState` | 否（N4 / N9） |

不得放进 `GraphView`、不得进 `useProjections` 入参、不得进 store。

最容易写错的一行代码（**禁止**）：

```ts
// 禁止：把 UI 状态传进投影
projectTimeline(obs, selectedNodeId);
projectGraph(obs, selectedNodeId);
```

理由：一旦入参带上 UI 状态，`schedule.ts` 的 `subscriber.projections` 缓存语义直接失效——同一个 `TurnObservation` 会对应多个投影结果，`observation !== subscriber.observation` 的失效判据不再成立。这是「wire / 投影不承载本地 UI 状态」在**函数签名层面**的落点，比 DTO 层更隐蔽。

### 9.3 交互规则

| 操作 | 结果 |
|---|---|
| 点击格子 | 选中该节点 |
| 再次点击同一格子 | 取消选中 |
| 点击另一格子 | 切换到该节点 |
| 按 `Esc`（焦点在栅格内） | 取消选中 |
| 切换观测实例 | **保留** `selectedNodeId`；若新实例不含该节点（不会发生，模板恒定）则忽略此规则 |
| 选中态下数据变化 | 保留选中；格子状态更新但选中不变 |

### 9.4 键盘

| 项 | 规则 |
|---|---|
| 元素 | 格子改为 `<button type="button">`（现状是纯 `div`，不可聚焦） |
| 先例 | 同一 feature 内的实例列表已是原生 `<button type="button">` + CSS 重置（`.instanceItem`：`background: transparent; border: 1px solid transparent; color: inherit; text-align: left; cursor: pointer`）。照抄这套重置即可：**不需要** `role="button"`、**不需要**手写 `tabIndex`、**不需要** antd 基元 |
| 重置要点 | `<button>` 不继承字体，必须显式 `font: inherit`；`.cell` 的 `display: grid` 已覆盖按钮默认布局，但 `text-align` 仍要写（否则格内文字居中）；`appearance: none` 视需要补 |
| 内嵌限制 | `<button>` 内不得再嵌按钮或链接。出口角标必须用伪元素（§6.4），图例不得放进格子 |
| Tab 顺序 | 等于 DOM 顺序，等于模板顺序（①→⑫）。不引入 roving tabindex（V1 12 个按钮可接受，W3 再评估） |
| 激活 | `Enter` / `Space` 原生触发 `onClick` |
| `Esc` | 绑在栅格容器的 `onKeyDown`（靠冒泡从聚焦格子收到），不注册全局 keymap（§3 软约束说明） |
| 焦点环 | `:focus-visible` 用 `outline: 2px solid var(--ant-color-primary); outline-offset: 2px`，不得用 `outline: none` |
| `aria-pressed` | 选中为 `true` |

### 9.5 选中态视觉

| 通道 | 值 |
|---|---|
| 外描边 | `outline: 2px solid var(--ant-color-primary)`（与 `:focus-visible` 同形，但常驻） |
| 背景 | `--ant-color-primary-bg`（浅底，非纯色填充，保证文字对比度） |

选中态**不得**改变状态编码（`data-state` 的边框线型与颜色、证据等级的边框样式必须原样保留）——选中是「你指了它一下」，不是「它的状态变了」。

### 9.6 与时间线的关系

V1 不联动（§1.2 第 4 条）。选中节点后不滚动时间线、不过滤时间线、不改变 InspectorPanel 内容。留待 OQ-3。

这条是对早期设想的**明确收窄**：早期方案把「选中后 inspector 只显示该节点事实 + 新增 `observation.inspector.no_node_facts` 空态」列进 W1c。收窄的理由：

| 理由 | 说明 |
|---|---|
| 需要新键 | 单节点空态不能用全局 `no_facts`（「该 turn 没有任何事实」≠「该节点没有事实」），必须新立键，属于新增文案面 |
| 需要新的空态语义 | ①② `admit` / `prepare` 的 `detail` 恒为 `null`（I55 §4.4），不是「加载失败」。做过滤后，这两格的 inspector 会长期显示空态，需要区分「无 detail」与「无时间线事实」两种空 |
| 与 V1 目标无关 | W1 的目标是「这一轮走到哪」在栅格上可读，选中只承担「指一下」的作用（§9.5）。过滤是 OQ-3 联动的一部分，应与滚动/高亮一起设计 |

因此 W1c **不改 `InspectorPanel`**，也不新增 `observation.inspector.no_node_facts`。若将来做 OQ-3，上面两条是必须先解的前置。

---

## 10 图例

### 10.1 位置与结构

图例固定在栅格下方、与 `InspectorPanel` 同级的条带，`flex-wrap` 允许换行；**不可交互**（不做筛选、不做显隐开关），纯标注。

结构：五个分组，每组「组标题 + 条目」。

### 10.2 条目

| 组 | 条目 | 视觉样本 |
|---|---|---|
| 节点状态 | 7 个 `observation.state.*` | 与格子同款 `data-state` 边框 |
| 证据等级 | 3 个 `observation.source.*` + 其 `*_hint` **可见文字** | 与格子同款边框样式（实/虚/点），条目下方一行 11px 灰字说明 |
| 边 | 已通行 / 活动边 / 待通行 / 未通行（4 条） | §7.1 的线型样本 |
| 出口 | `observation.graph.exit` + 6 个终态 `status` 取值 | §8.3 的角括号 + 方块 |
| 圈与工具分支 | `observation.graph.iteration`（带示例圈数）/ `observation.graph.tool_lane` | 行内标注同款 |

两处条目数的收敛：

| 项 | 现状/直觉 | 本规格 | 理由 |
|---|---|---|---|
| 证据 `*_hint` | 作为条目 `title`（悬停可见） | 直接渲染为可见文字 | 格子上的 `title` 在触屏与键盘下不可达（§12.4 已把它删掉）；若图例也只放 `title`，证据等级的解释就没有任何稳定可达的出口 |
| 出口组的 status 数 | 7 个（含 `running`） | 6 个终态 | `running` 没有出口（§5.4 第 1 行），列进图例等于承诺一个不存在的样本（违反 K15） |

图例必须顺带说明的三条「节点约定 → 视觉」事实（否则用户会按直觉误读）：

| 事实 | 图例里的落点 | 依据 |
|---|---|---|
| `waiting` **只可能**出现在 ⑤ `model` / ⑨ `pipeline` | 「节点状态」组 `waiting` 条目后加一句括注 | I55 §5.2 |
| 圈数徽标只出现在 10 个迭代内节点；①② 永不带「第 0 圈」 | 「圈与工具分支」组的示例只画在迭代内节点样式的样本上 | I55 §5.4 第 1 条 |
| ⑧–⑫ 的 `skipped` **恒为 `derived`**（由 ⑦ 的 `branch` detail 批量推导，后端从未下发该事实） | 「证据等级」组把 `derived` 的说明写成「由模板或 ⑦ 判定推导」，不要写成「后端未上报」 | I55 §4.6 + `deriveSkippedCells` 三道守卫 |

第 3 条尤其重要：如果图例把 `derived` 只解释成「缺数据时补的」，用户会以为后端漏发了 ⑧–⑫ 的 `skipped`，而那是设计上的推导结果。

### 10.3 i18n 键

新增 9 键（`zh-CN` / `en-US` 各一份）：

| 键 | zh-CN | en-US |
|---|---|---|
| `observation.legend.title` | 图例 | Legend |
| `observation.legend.group.state` | 节点状态 | Node state |
| `observation.legend.group.source` | 证据等级 | Evidence level |
| `observation.legend.group.edge` | 边 | Edges |
| `observation.legend.group.exit` | 出口 | Exit |
| `observation.legend.group.lane` | 圈与工具分支 | Iteration and tool branch |
| `observation.legend.edge.passed` | 已通行 | Traversed |
| `observation.legend.edge.idle` | 待通行 | Upcoming |
| `observation.legend.edge.untaken` | 未通行 | Not traversed |

`untaken` 必须新立键，**不得复用** `observation.graph.unfinished`：后者是「turn 收尾时格子仍停在中间态」的格子徽标文案（值「未完成」），与「这条边不会通行」不是同一件事。复用会让图例与格子上出现同一个词、两种含义。

复用既有键（不新增）：

| 用途 | 键 |
|---|---|
| 活动边 | `observation.graph.active_edge` |
| 正向 / 回边 | `observation.graph.forward` / `observation.graph.loop` |
| 出口 | `observation.graph.exit` |
| 圈数 / 工具分支 | `observation.graph.iteration` / `observation.graph.tool_lane` |
| 状态 / 证据 | `observation.state.*` / `observation.source.*` |

### 10.4 图例纪律

| 禁止 | 原因 |
|---|---|
| 图例条目点击切换显隐 | 会把图例变成控件，破坏「只读观测面」的一致性 |
| 图例里出现未实现的条目 | 图例是承诺；条目必须与 §7 / §8 实际渲染一一对应（K15） |
| 图例文案内联中文 | P12 |

---

## 11 失效与动画

### 11.1 三类失效

| 类 | 触发 | 重算范围 |
|---|---|---|
| 投影失效 | 观测数据变化（`subscribeProjections`） | `projectGraph` 全量（12 格 + 12 边 + 出口） |
| 选中失效 | 点击 / Esc | 仅 12 个格子的 `data-selected` 与 `aria-pressed` |
| 布局失效 | 窗口尺寸变化 | 纯 CSS，无 JS 参与（N7） |

### 11.2 选中不触发投影

`selectedNodeId` 在 `ObservationPage`，`useProjections` 的入参只有 `(conversationId, turnId)`，因此选中变化不进入调度链、不重算 `projectGraph`、不重建边元素。判据 K14。

### 11.3 动画规则

| 动画 | 触发条件 | 停止条件 |
|---|---|---|
| 活动边呼吸 | `data-phase="active"` | 该边相位不再为 `active`；实例终态时必然不再有 active 边 |
| 进行中格子呼吸（现状已有） | `data-state="running"` | 状态离开 `running` |

| 规则 | 内容 |
|---|---|
| 只由状态驱动 | 用 `data-*` 属性 + CSS `animation`，禁止 JS `setInterval` / `setTimeout` 驱动 |
| 无无限循环残留 | 终态实例不得有 `data-phase="active"` 的元素（活动边必然消失）；`data-state="running"` / `"waiting"` 的**未完成格子允许存在**，但动画必须停（见下方两条开关） |
| 减少动效 | `@media (prefers-reduced-motion: reduce)` 下 `animation: none`，保留全部静态视觉编码 |
| 不参与布局 | 动画只改 `opacity` / `box-shadow`，不改 `width` / `height` / `transform`（避免 1px 边抖动） |

**两条独立的停止开关**（不能互相替代）：

| 开关 | 触发 | 落点 |
|---|---|---|
| 用户偏好 | `@media (prefers-reduced-motion: reduce)` | 全部动画 `animation: none`；静态编码（线型 / 线宽 / 形状 / 文字）完整保留 |
| turn 生命周期 | 实例进入终态 | 进行中格子的呼吸必须停 |

第二条有一个现状陷阱：**终态时格子可能仍是 `data-state="running"` / `"waiting"`**——`NodeCell` 的 `terminal && node.unfinished` 会加「未完成」徽标，但 `data-state` 保持原值（I55 §6.1 收尾规则：不臆造终态）。因此「终态无动画」**不能**靠「终态没有 running 格子」实现，必须把 `terminal` 落到 DOM（如 `data-terminal="true"`）并在 CSS 里停掉呼吸：

```css
.cell[data-state="running"][data-terminal="true"],
.cell[data-state="waiting"][data-terminal="true"] { animation: none; }
```

现状：`NodeCell` 已收 `terminal` prop，但只用于徽标，**没有输出到 DOM**；`observation.module.css` **没有任何** `prefers-reduced-motion` 规则（全项目只有 `settings.css` 两处）。两处都要在 W1b 补上，否则终态实例会一直呼吸（K18 / K19）。

**已考虑并拒绝：`aria-live`**

观测数据随事件流高频更新，给栅格或格子加 `aria-live` 会让读屏每帧播报一次。V1 不加任何 live region，改为「用户主动选中后用 `aria-label` 读取该格摘要」（§12.4）。这条是拒绝，不是遗漏。

### 11.4 时长文案修正

现状 `observation.graph.no_percent` = 「V1 只显示状态，不显示百分比」，但栅格并不显示任何时长，该文案与实现不符。改为：

| 键 | 新值（zh-CN） | 新值（en-US） |
|---|---|---|
| `observation.graph.no_percent` | 耗时按事件刷新，不逐秒跳动 | Duration updates on events, not per second |

理由：`nowMs` 取自已观测事件的最大 `atMs`（`projections/graph.ts` 的 `observedNowMs`），只有新事件到达才前进；这正是「不逐秒跳动」的事实，比「不显示百分比」更准确。

**已考虑并拒绝：让时长真正跳动**

| 方案 | 做法 | 拒绝理由 |
|---|---|---|
| running-only tick | `useEffect` + `setInterval(1000)`，只在 `status === "running"` 时跑，把 tick 值传给 `NodeCell` 覆盖 `nowMs` | 破坏投影纯函数：`nowMs` 变成外部注入，`GraphView` 不再是 `TurnObservation` 的纯函数（P4） |
| tick 放进调度层 | `schedule.ts` 加 `tickNow()`，复用同帧合并与订阅者集合 | 投影变成「依赖时钟」，`subscriber.projections` 的缓存语义改变，等于给失效模型加第二个源 |

两个方案都把时间源引进投影层。V1 接受「数字只在事件到达时跳变」，并把感知落差交给文案说明：活动边脉冲负责表达「活着」，数字只负责「不误导」。

### 11.5 不引入 memo（有意取舍）

现状链路：`TurnObservation` 引用变化 → `scheduleFlush` → `flushPending` → 三投影**全量重算**（全部新建对象）→ `listener(setProjections)` → `ObservationPage` 整页 re-render。从投影到 DOM **没有任何 memo 边界**。

| 项 | 结论 |
|---|---|
| 规模 | 12 格 + 12 条边（14 个元素）+ 12 个 `button` ≈ 60–70 个 DOM 节点；60fps 全量重渲染可忽略 |
| 代价 | 加 `React.memo` 要同时维持「稳定回调 + `nodes` 数组引用 + 边数组引用」三层，这三层恰好是引用泄漏与策略漂移的高发区 |
| 定案 | **有意不加**。触发条件（满足任一即重新评估）：格子数 × 元素数 > 200；或引入 hover / 拖拽这类**每帧必变**的本地状态 |
| 副作用（已知） | 任一格子状态变化都会让 `timeline` 数组身份变化，`InspectorPanel` 整条时间线全量重渲染，哪怕新事件与当前可见事实无关。12 格规模下接受 |

### 11.6 已知冗余（不是缺陷）

`useProjections` 的 effect 内先 `setProjections(readProjections(...))`，再 `subscribeProjections(...)`，而后者内部会**立即**用同一份投影回调一次。两次都在同一 effect 的同步体内，React 18 自动批处理，**只渲染一次**；但切换实例时必然多算一次三投影。规模无影响，登记为「已知冗余、仅切换时发生、可接受」，不按性能问题处理。

---

## 12 主题 / i18n / 可访问性

### 12.1 颜色

| 规则 | 内容 |
|---|---|
| 唯一来源 | 全部 `var(--ant-*)`；`observation.module.css` 不得出现 `#xxxxxx` / `rgba(` / `hsl(` |
| 禁止 fallback | 不得写 `var(--ant-x, #fff)`（DESIGN §5.3）；也不得自造变量名 + 硬编码兜底（如 `var(--radius-md, 8px)`），问题等价 |
| 本规格用到的 token | 颜色：`--ant-color-primary` / `-primary-bg` / `-text-tertiary` / `-text-quaternary` / `-border` / `-border-secondary` / `-bg-container` / `-bg-layout` / `-error` / `-error-bg` / `-warning` / `-warning-bg` / `-success` / `-success-bg`；圆角：`--ant-border-radius` |
| 不自造色名 | 不新增 `--canvas-color-*` 之类；`--canvas-*` 只承载尺寸与布局（§6.6） |

### 12.2 尺寸与圆角

见 §6.6。要点复述：`--canvas-*` 进 `global.css` `:root`；删除 `--radius-md` 的硬编码兜底。

### 12.3 i18n 完整性

| 项 | 要求 |
|---|---|
| 新增键 | 9 个 `observation.legend.*` + 1 个改写值（`observation.graph.no_percent`），`zh-CN` / `en-US` 必须同时存在 |
| 参数占位 | 复用 `graph.iteration` 的 `{{iteration}}`，图例示例固定传 `2` |
| 禁止 | 组件内出现任何中文字面量（含 `title` / `aria-label`） |

### 12.4 可访问性

| 项 | 要求 |
|---|---|
| 栅格容器 | `role="group"` + `aria-label={t("observation.title")}` |
| 格子元素 | `<button type="button">` + `aria-pressed={selected}` |
| 格子 `aria-label` 拼接 | `<节点名>` + （`node.iteration > 0` 时）`· ` + `t("observation.graph.iteration", { iteration })` + `· <状态名>` + （带出口时）`· ` + `t("observation.graph.exit")` + ` ` + `<status 名>`；分隔符统一为 ` · `，与可见文字一致；不新增键 |
| 证据提示 | 从 `title` 迁到图例的 `*_hint`（`title` 在触屏与键盘下都不可达，图例是稳定出口）；格子不再挂 `title` |
| 对比度 | 所有前景色取 antd 语义 token，不叠加降低对比度的 `opacity`（与 N6 同一处理） |
| 焦点 | `:focus-visible` 可见；Tab 顺序 = 模板顺序 |
| 非颜色通道 | 状态靠边框线型 + 文字，边靠线型 + 线宽，出口靠形状 + 位置 |
| 图例 | 纯展示、`aria-hidden` 不需要——它承载 `*_hint` 文本，必须对读屏可见；因此不得整体 `aria-hidden` |
| `aria-live` | **不加**。高频事件流会让读屏每帧播报；读取改由用户主动选中触发（§11.3 末段） |

---

## 13 实施拆分（四刀）

四刀按依赖顺序，每刀可独立合入、独立验收。

| 刀 | 范围 | 内容 | 判据 | 依赖 |
|---|---|---|---|---|
| W1a | 投影 | `TEMPLATE_EDGES`、`GraphEdgeView.phase`、`GraphView.edges` / `activeEdge` 改引用、`GraphExitView` + 出口判定；删 `templatePredecessor`；扩 `projections.test.ts` | K2 / K3 / K4 / K5 / K8 | — |
| W1b | 几何与边渲染 | 3 行网格 + 外轨；`CanvasEdge` 子组件按 §6.2 / §6.3 推导放置；`--canvas-*` 变量；删 `opacity` 与 `--radius-md` 兜底 | K1 / K6 / K7 / K9 / K10 / K11 | W1a |
| W1c | 选择与出口 | `NodeCell` 改 `button` + 选中态 + 出口角标；`ObservationPage` 持 `selectedNodeId`；`GraphCanvas` 透传 `selectedNodeId` / `onSelect` / `exit`；Esc + `aria-label` | K12 / K13 / K14 | W1b |
| W1d | 图例与文案 | 新增 `LegendPanel.tsx`；9 键 × 2 语言；改 `no_percent` 文案 | K15 / K16 | —（可与 W1b 并行） |

### 13.1 W1a 细则

| 步骤 | 文件 | 动作 |
|---|---|---|
| a1 | `template.ts` | 新增 `TemplateEdgeKind` / `TemplateEdge` / `TEMPLATE_EDGES`（12 条，编译期常量，`satisfies readonly TemplateEdge[]`） |
| a2 | `projections/graph.ts` | 删 `templatePredecessor`；`activeEdgeOf` 改为返回 `edges` 中的引用；新增 `edgePhaseOf` / `exitOf`；`projectGraph` 产出 `edges` / `activeEdge` / `exit` |
| a3 | `projections.test.ts` | 新增：边数恒 12、`active` 至多 1 且与 `activeEdge` 同引用、相位表驱动（5 行 + 4 行）、出口 7 行表驱动 |
| a4 | `GraphCanvas.tsx` | 不改（仍只用 `activeEdge`）——W1a 单独合入后页面行为不变，可先验证投影无回归 |

### 13.2 W1b 细则

| 步骤 | 文件 | 动作 |
|---|---|---|
| b1 | `GraphCanvas.tsx` | 栅格从「12 格自动流」改为显式放置；新增 `CanvasEdge`（`kind` + 两端 `row/column` → 内联 `gridColumn` / `gridRow`）；回边渲染三段；边全部来自 `edges`；删 `activeSource` / `activeTarget` 传参；`TOOL_LANE_LABEL_COLUMNS` 改由模板推导；**行内标注 `gridRow` 由 `2` 改为 `3`**（加入外轨后 `row 1` 是第 3 行，不改会盖住 `row 0` 的格子） |
| b2 | `NodeCell.tsx` | 自放置 `gridRow` 由 `node.row + 1` 改为 `node.row + 2`；删 `activeSource` / `activeTarget` 两个 prop 与两个 `data-*` 属性；把已有的 `terminal` prop 输出为 `data-terminal`（供 §11.3 的终态停动画用） |
| b3 | `observation.module.css` | 新增 `.edge` / `.edgeForward` / `.edgeCross` / `.loopSeg*` / `[data-phase]` 样式；删格子两处 `opacity`；`pending` 文字色 `quaternary` → `tertiary`；删 `data-active-source` / `data-active-target` 选择器；3 处 `var(--radius-md, 8px)` → `var(--ant-border-radius)`；`.cell` 增 `position: relative`；新增 `[data-state="running"][data-terminal="true"] { animation: none }` 与 `@media (prefers-reduced-motion: reduce)` 规则 |
| b4 | `global.css` | `:root` 新增 5 个 `--canvas-*` |
| b5 | `docs/DESIGN.md` | §5.2 前缀表增补 `--canvas-` 一行 |
| b6 | 结构检查 | 边元素数 = 14（10 同行 + 1 跨行 + 3 回边段），与 `edges.length` 一致（K6） |

### 13.3 W1c 细则

| 步骤 | 文件 | 动作 |
|---|---|---|
| c1 | `NodeCell.tsx` | `div` → `button type="button"`；保留 `data-state` / `data-source`；新增 `data-selected` / `data-exit`；`onClick`；`aria-pressed`；`aria-label`（节点名 · 状态名 [+ 出口状态]）；删 `title`（证据提示迁到图例） |
| c2 | `ObservationPage.tsx` | 新增 `selectedNodeId` 与 `toggleSelect`；传给 `GraphCanvas`；切换实例时**不清空**（§9.3） |
| c3 | `GraphCanvas.tsx` | 透传 `selectedNodeId` / `onSelect` / `exit`；容器 `role="group"` + `aria-label` + `onKeyDown`（Esc） |
| c4 | `observation.module.css` | `[data-selected="true"]` 样式（outline + `--ant-color-primary-bg`）；`:focus-visible`；出口伪元素样式（`position: absolute`） |
| c5 | `NodeCell.tsx` 补充 | `button` 需 `appearance: none` 级别的重置：`.cell` 已有 `border` / `background` / `padding`，但要补 `font: inherit`、`color: inherit`、`text-align: start`、`cursor: pointer`，避免浏览器默认按钮样式污染状态编码 |
| c6 | `ObservationPage.tsx` / `GraphCanvas.tsx` | 抽出 `STATUS_LEVEL`（§8.3），`statusColor()` 与 `STATUS_TAG_COLOR` 改为从它派生；出口色由档位映射 |
| c7 | `InspectorPanel.tsx` | **不改**（§9.6）。不新增 `observation.inspector.no_node_facts`，不做选中过滤 |

### 13.4 W1d 细则

| 步骤 | 文件 | 动作 |
|---|---|---|
| d1 | `features/observation/LegendPanel.tsx` | 新增；五组；纯展示；不接任何交互 |
| d2 | `ObservationPage.tsx` | 在 `GraphCanvas` 之后挂载 `LegendPanel` |
| d3 | `i18n/locales/{zh-CN,en-US}.json` | 9 个 `observation.legend.*` 新键 + 改写 `observation.graph.no_percent` |

---

## 14 验收判据

### 14.1 验收分型（测试设施的现实约束）

| 事实 | 内容 |
|---|---|
| 测试环境 | `vitest.config.ts` 的 `include: src/**/*.test.{ts,tsx}`、`setupFiles: src/testSetup.ts`，**没有** `environment: 'jsdom'` |
| `testSetup.ts` | 只把 `window` mock 成 `{}`，**无 DOM** |
| 依赖 | `package.json` 无 `@testing-library/react`、无 jsdom、无 happy-dom |
| 既有决定 | I48 §12 第 7 条已明确「不为此引入 jsdom / testing-library 依赖」，并把组件级时序降级为人工清单（§9.3）；I43 §9 同样把 DOM 渲染类条目列为手工验收 |
| 禁止 | **不得**为 W1 引入 jsdom / RTL / happy-dom |

因此验收分两型，判据表「方式」列已按此标注：

| 型 | 刀 | 可验证内容 |
|---|---|---|
| 自动化（vitest 纯函数） | W1a | `TEMPLATE_EDGES` 一致性、`activeEdge` 同引用、`edgePhaseOf` 四态、`exitOf` 各 status |
| 人工（人眼 + code review） | W1b / W1c / W1d | 边元素位置、回边不穿格、出口不撞 ⑦→⑧ 边、选中环与焦点环叠加、暗色三档对比度、reduced-motion 降级、Tab 序、读屏摘要 |

| # | 判据 | 方式 |
|---|---|---|
| K1 | 无观测数据时仍渲染 12 格，全部 `pending` + `derived` | 现有 vitest 断言保留 |
| K2 | `TEMPLATE_EDGES` 恒 12 条：11 条正向覆盖 `order` 1..11，1 条回边 `after-iteration → admit`；几何分类为 10 同行 + 1 跨行 + 1 回边 | vitest |
| K3 | `edges` 中 `phase === "active"` 恰 0 或 1 条，且 `activeEdge === 该元素`（引用相等） | vitest |
| K4 | 出口判定 7 行表驱动全覆盖（§5.4 的 5 个场景 + 唯一 + 无出口） | vitest |
| K5 | `status === "running"` 时 `exit === null`；6 个终态各有出口 | vitest |
| K6 | 栅格内边元素数 = 14；无「只渲染 activeEdge」的旧路径残留 | 代码审查 + 结构检查 |
| K7 | 回边三段同相位、可见段位于顶部外轨、横段带唯一左向箭头 | 人工（截图比对） |
| K8 | `GraphEdgeView` 无 `id` / `handle` / `label`；`exit` 不在 `edges` 中 | typecheck + 代码审查 |
| K9 | `src/features/observation/` 无 `getBoundingClientRect` / `offsetWidth` / `clientWidth` / `ResizeObserver` | `rg` 检查为空 |
| K10 | `observation.module.css` 无 `#` / `rgba(` / `hsl(` | `rg` 检查为空 |
| K11 | 无 `var(--ant-*, <兜底>)`；无 `var(--radius-md, …)`；格子规则无 `opacity` | `rg` 检查为空 |
| K12 | 12 个格子均为 `button` 且带 `aria-pressed` / `aria-label` | 代码审查 + 键盘实测 |
| K13 | Tab 顺序为 ①→⑫；`Esc` 取消选中；焦点环可见 | 键盘实测 |
| K14 | 选中变化不触发 `projectGraph` 重算；`selectedNodeId` 不在 `GraphView` | 代码审查（入参核对） |
| K15 | 图例五组齐全，条目与 §7 / §8 实际渲染一一对应 | 人工比对 |
| K16 | `zh-CN` / `en-US` 键集合完全一致，无缺键 | diff 两份 locale 的 `observation.legend.*` |
| K17 | `npm run typecheck`、`npm run lint`、`npm run test` 全绿 | 命令 |
| K18 | `prefers-reduced-motion: reduce` 下无任何动画，且静态编码（线型 / 线宽 / 形状 / 文字）完整保留 | 系统设置实测 |
| K19 | 终态实例（6 种）页面上不存在 `data-phase="active"` 元素；**允许** `data-state="running"` / `"waiting"` 的未完成格子存在，但它们必须带 `data-terminal="true"` 且动画已停 | 代码审查 + 实测 |
| K20 | `pending` 格子文字色为 `tertiary`；`--ant-color-error` 只出现在 `state === "error"` 与 `status === "failed"` | `rg` 检查 + 人工比对 |

---

## 15 已拒绝方案

| # | 方案 | 拒绝理由 |
|---|---|---|
| D-1 | 出口悬空短桩置于格外 | 见 §1.3 D-1 |
| D-2 | 回边走行间隙 | 见 §1.3 D-2 |
| D-3 | 端口桩表达「可接」 | 观测面不可接，桩暗示可连线（§1.3 D-3） |
| D-4 | 百分比进度 | 无时钟源，必然伪造（§1.3 D-4） |
| D-5 | SVG / Canvas 画边 | 第二套坐标系与主题体系（§1.3 D-5） |
| D-6 | `ResizeObserver` 自动缩放列宽 | 违反 N7（§1.3 D-6） |
| D-7 | 选中「节点 × 圈」 | 同屏只有一个圈，自由度无意义（§1.3 D-7） |
| D-8 | 卡片 / 标记两级缩放形态 | V1 只做格子；另两级为反原语（§2.6） |
| D-9 | 图例做显隐开关 | 把只读标注变成控件（§10.4） |
| D-10 | 用 `opacity` 表达证据等级 | 会透出下层边线，与 N6 冲突（§6.5） |
| D-11 | 边加 `z-index` 覆盖格子 | 边会横穿文字（§6.8） |
| D-12 | 全局注册 `Esc` 快捷键 | 只需容器内响应；全局注册会与弹窗/输入框的 Esc 冲突（§3 软约束） |
| D-13 | 让耗时数字真正逐秒跳动 | 需给投影注入时钟源，破坏 `GraphView` 的纯函数性（§11.4） |
| D-14 | 选中后过滤 InspectorPanel + 新增 `observation.inspector.no_node_facts` | 属 OQ-3 联动，V1 不做；需新键与新空态语义（§9.6） |
| D-15 | 给 `NodeCell` / `CanvasEdge` 加 `React.memo` | 12 格规模下全量重渲染可忽略，memo 引入三层引用维持成本（§11.5） |
| D-16 | 栅格加 `aria-live` 播报更新 | 高频事件流会让读屏每帧播报（§11.3、§12.4） |
| D-17 | evicted 实例也渲染栅格（靠画布自行判断） | `summarize()` 已清空 `cells`，投影恒输出全 pending，画布无法区分摘要态与真全 pending（§8.5） |

---

## 16 未决问题

| # | 问题 | 现状 | 去向 |
|---|---|---|---|
| OQ-1 | ① 在第一圈运行时没有活动边（前驱为空） | 已知缺口，`activeEdge === null`；画面表现为「无活动边」而非错误 | 接受。若要补，需引入「turn 起点」伪节点，会破坏「出口不是节点」的对称性 |
| OQ-2 | 多实例并排对比 | 现为单实例 + 实例列表 | 后续规格 |
| OQ-3 | 时间线 ↔ 格子联动 | V1 不做（§9.6） | 后续规格 |
| OQ-4 | 12 个按钮进 Tab 序是否过重 | V1 接受 | W3 评估 roving tabindex |
| OQ-5 | 出口多候选并列展示（如 `stopped` 三个候选都 `finished`） | V1 只标一个（最后到达者） | 若出现歧义诉求，再议 |
| OQ-6 | 极窄窗口下的横向滚动体验 | 接受滚动条（§6.7） | 视使用反馈 |
| OQ-7 | 暗色主题下 `warning` / `success` / `text-tertiary` 三档出口色的实际对比度 | 三档取 antd 语义 token，理论上达标；但 `warning` 与中性灰在暗底上的区分度未实测 | W1d 完成后用 `[data-theme="dark"]` 实测；不达标则改用形状/文字后缀区分，不引入新色值 |

---

## 17 实施偏差记录

实现期间若与上述任一硬约束或判据发生偏离，在此登记；无偏差则保持空表。

| 日期 | 刀 | 偏差 | 理由 | 判据影响 |
|---|---|---|---|---|
| — | — | — | — | — |
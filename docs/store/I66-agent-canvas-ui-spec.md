# I66 画布 UI 规格（W1 · 画布设施 + 示例内容）

| 项 | 值 |
|---|---|
| 状态 | 定稿（可实施；按 §14 顺序落地） |
| 日期 | 2026-11-22（校正：W1 只交付画布设施与**示例内容**，真实内容在 W2） |
| 波次 | W1 = 本规格（设施 + 示例内容）；W2 = `docs/store/I68-agent-canvas-preset-spec.md`（真实内容 + 预设 + 宿主接线） |
| 术语唯一出处 | `docs/analysis/ORCHESTRATION-CONCEPT-SEMANTIC-TRANSFER.md` §2.5（用户可见词表）/ §2.6（开发原语词表） |
| 事实来源 | `docs/analysis/AGENT-CANVAS-RENDERING-ANALYSIS.md`（命题 P1–P13、缺口 G1–G13）；本规格 §15 的 K1–K25 是**自有编号**，与前者不同 |
| 视觉与主题约束 | `docs/DESIGN.md` §5（双层变量体系、§5.3 禁用 fallback） |
| 影响范围 | 新建 `src/features/canvas/`：`types.ts` / `layout.ts` / `geometry.ts` / `viewport.ts` / `CanvasViewport.tsx` / `CanvasCard.tsx` / `CanvasEdge.tsx` / `CanvasLabel.tsx` / `CanvasLegend.tsx` / `canvas.module.css`；示例内容 `demo/demoContent.ts` + `demo/CanvasDemoPage.tsx` + `module.ts`（deep-link 自检路由）；`src/features/registry.ts`（+1 段装载）；`src/assets/styles/global.css`（新增 `--canvas-*`）；`src/i18n/locales/{zh-CN,en-US}.json`（`canvas.*`）。**不改** `src/features/observation/`、**不改** `src/stores/agent/observation/`、**不删** `GraphCanvas.tsx` / `NodeCell.tsx`（接线与删除属 W2） |
| 契约影响 | 无。全部为前端本地视图模型，不进 wire、不触发 `npm run contract:gen` |
| 内容绑定 | 无。本规格正文与设施代码均不出现 `AgentNodeId` / `GraphView` / `TurnObservationStatus` / 任何观测 id；示例内容用 `demo-*` 假标识（K24 / K25） |

> 定位：交付**画布**（词表 §2.5.2：有视口、缩放、平移的编排面）的七件设施，以及一组**可丢弃的示例内容**用于设施自检。W1 的示例内容不是产品内容，也不是现有观测卡片的延续：它是 6 张 `demo-*` 卡片，只用来让视口、缩放、四相、出口、图例、键盘与 memo 行为可被看见、可被验收。真实内容（turn 观测投影）在 W2 才接入（§1.6）。

---

## 1 范围与边界

### 1.1 交付什么（七项设施 + 一项示例）

| # | 目标 | 判据 |
|---|---|---|
| 1 | 坐标布局：条目 `row` / `column` 经纯函数换算成盒子 `x / y / w / h`，id 泛型 `string`，零 DOM 测量 | K1 / K2 / K3 / K21 |
| 2 | 边几何：三类 route（同行 / 跨行 / 外轨）由盒子推导出线段与箭头 | K4 / K5 |
| 3 | 视口：三层结构、缩放、平移、复位，世界坐标与屏幕坐标分离且可互转 | K6 / K7 / K8 / K22 |
| 4 | 卡片壳：只接收盒子 + 选中 + 已本地化文案 + 色调 + 出口态，不决定内容 | K11 / K12 |
| 5 | 边渲染器：四相编码（线型 / 线宽 / 颜色 / 动画）+ 活动动画 | K10 / K12 |
| 6 | 图例编码：相位四值与出口三态两组，与画布元素共用同一 CSS 类 | K10 / K12 |
| 7 | 边界纪律：设施不依赖观测模块、不绑定 DTO、不持有内容数据 | K13 / K14 / K24 |
| 8 | **示例内容**：6 张 `demo-*` 卡片 + 4 条边（四相各一）+ 1 个出口，经 deep-link 自检路由可打开 | K19 / K20 / K25 |

W1 的交付物是**可被组合的设施 + 一次性示例**。示例只证明设施可用；它不含任何观测语义，W2 接入真实内容时整体删除（§11.3）。

### 1.2 不交付什么

| # | 排除项 | 理由 |
|---|---|---|
| 1 | 编辑模式：拖拽 / 连线 / 删除 / 参数表单 | 需要 L2 数据模型（`WorkflowNode`、参数 schema、交接类别），编排分析 §12.2 标为「缺」，工作流画布尚未立项。坐标模型已按盒子建（§5.5），编辑阶段只需接命令与命中区 |
| 2 | 端口 / 槽 / 接线 / 连接句柄 | 同上：运行内容无边可接，端口是 L2 概念（词表 §2.5.4） |
| 3 | 多圈并置、多实例并排 | 画布一次只呈现一层内容；并排对比另立规格（OQ-2） |
| 4 | 时间线与卡片联动高亮 | 需要跨面板 hover 与双向选中语义，W1 代价大于收益（OQ-3） |
| 5 | 百分比进度、逐秒跳动的计时 | 无观测时钟；引入定时器会破坏投影纯函数性（§12.4） |
| 6 | 图形库（reactflow / dagre / d3） | DOM + 绝对定位足够；引库会带入自己的坐标系、测量与主题体系（N7） |
| 7 | 导出图片、分享链接 | 无需求来源 |
| 8 | 全局快捷键 | 快捷键生成源在 Rust（`DEFAULT_SHORTCUT_PATTERNS` → `contract:gen`），前端手改会被 `contract:verify` 的 `git diff --exit-code` 拦下（§3） |
| 9 | 自动 fit-to-view / 响应式缩放 | 需要读容器尺寸（测量）。初始 `scale = 1`、原点对齐视口左上，缩放与平移全由用户驱动（§6.3） |
| 10 | **真实内容**：turn 观测的卡片 / 边 / 相位 / 出口 / 图例内容组 | 属 W2（§1.6）。W1 的示例内容不是真实内容，也不得被当作真实内容的默认值 |
| 11 | 宿主页接线：改造 `ObservationPage`、删除 `GraphCanvas` / `NodeCell` | 属 W2：真实内容接入时一次性替换，避免 W1 出现「半新半旧」两套视图 |
| 12 | 内容语义判定：哪条边活动、哪张卡片是出口、相位如何取 | 已落地在观测投影层（`src/stores/agent/observation/projections/graph.ts`），W2 只做映射；本规格不复述判定表 |

### 1.3 与分析文档的差异（有意校正）

分析文档 §1.1 的结论是「两套视图模型、一套渲染设施、两份交互策略」。本规格交付的是**设施**这一半，因此以下几处按画布语义收敛。

| # | 分析表述 | 本规格 | 理由 |
|---|---|---|---|
| D-1 | L1 观测内容用「格子」模型（`row` / `column`） | 画布内容用「盒子」模型（`x / y / w / h`），`row` / `column` 只作为布局输入 | 画布必须有坐标系才能承载缩放与平移；格子模型只能贴 CSS Grid，表达不了视口 |
| D-2 | 「观测栅格」是 L1 的只读栅格，W1 交付它 | **不交付观测栅格，也不交付观测内容**；W1 用 6 张示例卡片自检设施，真实内容由 W2 以画布预设方式接入（§1.6） | 「观测画布」在词表里不存在且属禁用叫法（词表 §2.5.4）；叫画布就必须有视口，有视口就不能是格子 |
| D-3 | 出口渲染为格外悬空短桩 + 汇点徽标 | 收进盒子右下角，内折角括号 + 汇点方块（§8.1） | 相邻盒子间隙只有 `--canvas-gap`，放不下短桩 + 汇点 |
| D-4 | 回边可走行间隙或外轨 | 外轨 route 固定走顶部留白（§6.4） | 行间隙要避让下锚点与上锚点；跨列回边走行间隙会横穿整行 |
| D-5 | 端口桩（悬空短线）表达「可接」 | 不画桩。锚点即盒子边框上的交点 | 运行内容不可接，桩会暗示可连线 |
| D-6 | 百分比进度环 | 保留时长数字，文案改为「按事件刷新」 | 无时钟源，百分比必然靠定时器伪造 |
| D-7 | 选中「格子 × 圈」 | 选中「卡片」（§9.1） | 画布只呈现一层内容，选圈是无意义自由度 |
| D-8 | `ResizeObserver` 自动缩放列宽 | 不做自动缩放；缩放由用户驱动的视口变换承担（§6.3） | 自动缩放需测量，违反 N7 |
| D-9 | 「两套视图模型」= 观测视图模型 + 编排视图模型 | 设施视图模型（`CanvasCardView` / `CanvasEdgeView`）与**内容视图模型**（W2 的映射产物）分开；设施类型不含内容语义 | 分析说的「两套」都指内容层；设施层需要第三种、且必须与两者都不耦合，否则换内容要改设施 |

### 1.4 与现状代码的关系

现状 `src/features/observation/` 的 `GraphCanvas` / `NodeCell` 是「12 格 CSS Grid + 单条活动边」的只读占位视图，不是画布：无坐标系、无视口、无缩放、无平移。**它们不是本规格的底座，也不被本规格改写**：

| 对象 | W1 处置 | W2 处置 |
|---|---|---|
| `GraphCanvas.tsx` / `NodeCell.tsx` | 不动、不 import | 真实内容接入画布后删除 |
| `ObservationPage.tsx` | 不动 | 改为画布宿主页（画布 + `InspectorPanel`） |
| `src/stores/agent/observation/`（模板 / 投影 / 调度） | 不动、不 import（K24） | 作为真实内容的数据源，由 W2 适配层读取 |
| `src/features/canvas/`（本规格） | 新建，设施 + 示例内容 | 设施保留，示例内容整体删除 |

### 1.5 与词表原语表的差异（落地后需回写词表）

词表 §2.6 的原语表是按「L1 = 观测栅格」写的，其中 §2.6.6 把「全局坐标系 / `viewport store`」与「`React.memo` 边界」列为**反原语**。本规格交付画布设施，这两条必须反转；落地后要回写词表（§2.6.7 第 3 条）。

| # | 词表条目 | 词表写法 | 本规格 | 理由 |
|---|---|---|---|---|
| T-1 | 容器词（§2.5.2 / §2.5.4） | 「画布」不加限定属禁用叫法，应写「观测栅格」/「编排画布」 | 就叫**画布**，不加任何限定词 | 用户已定：交付画布本体，不交付观测栅格；「观测画布」「只读画布」都是自造词（词表无），「画布」本身已是完整术语（§2.5.2 定义含视口 / 缩放 / 平移） |
| T-2 | §2.6.6 反原语「全局坐标系 / `viewport store`」 | 不造 | 造 `CanvasViewportState` + 世界/屏幕双坐标系 | 画布定义（§2.5.2）要求视口、缩放、平移；但仍是**组件本地 state**，不进 store、不进 wire（N4） |
| T-3 | §2.6.6 反原语「`React.memo` 边界（v1）」 | 不造 | `CanvasCard` / `CanvasEdge` / `CanvasLabel` / `CanvasLegend` 全部 memo | §12.5 的两条触发条件均已满足（拖拽每帧变 + 元素数随内容线性增长） |
| T-4 | §2.6.5 锚点 `AnchorPoint { xPct, yPct }` | 百分比锚点 | px 世界坐标（`CanvasBox` + 锚点推导） | 百分比随容器尺寸变化，破坏「世界坐标恒定」（§6.8） |
| T-5 | §2.6.5 出口锚 `exitAnchorOf` | 右边缘 75% 高度 | 盒内右下角内折角括号 | 相邻盒间隙只有 8px，放不下外置短桩（§1.3 D-3） |
| T-6 | §2.6.4 边集常量 `templateEdges` | 待建 | W1 不造；内容层的边集常量属 W2（已落地于 `template.ts`） | 设施不认识「模板边」，只认识 `CanvasEdgeView`（§5.1） |
| T-7 | §2.6.4 边索引 `templateEdgeIndex` | 待建 | 不造 | 内容层逐条 `find` 的开销远低于维护索引表 |
| T-8 | §2.5.3 相位取值「未开始 / 进行中 / 已完成 / 跳过…」 | 描述句 | 具名四值 `untaken` / `idle` / `passed` / `active`（本规格只定义**编码**，判定属观测投影） | 描述句不能进代码；编码见 §5.3 |
| T-9 | §2.5.3 出口边 `exit edge` | 独立数据类别 | W1 不画出口边元素；只提供盒内角标组件与三态编码（§8） | 角标内嵌后无出口边可画；「出口不是卡片」「出口不与边混类」两条由 W2 的内容不变量承接（N8） |
| T-10 | §2.5.4「观测格子叫『卡片』→ 应说单元格」（理由：格子无参数、无端口、无动作） | 无条件禁用 | 本规格用**卡片** | 该条的适用前提是「观测栅格容器」——其理由描述的是栅格内容物。本规格容器是画布（用户定案，§2.5.2 定义），渲染形态按 §2.5.1 编排侧取**卡片**，卡片模式为「运行模式 → 运行摘要」；端口区 / 拖拽柄 / 参数表单在只读模式**不渲染**，属卡片构件缺席，不等于「不是卡片」 |

**回写动作**：T-1 / T-2 / T-3 / T-6 / T-8 / T-9 / T-10 落地后需同步修改 `docs/analysis/ORCHESTRATION-CONCEPT-SEMANTIC-TRANSFER.md` §2.5.2 / §2.5.3 / §2.5.4 / §2.6.5 / §2.6.6，否则下次按词表评审会把本规格判为违规。

### 1.6 与 W2 的分界（真实内容在 W2 才出现）

| 层 | W1（本规格） | W2（`I68-agent-canvas-preset-spec.md`） |
|---|---|---|
| 几何 | `layoutTemplate` / `edgeGeometryOf` / 锚点纯函数 | `layoutPreset(preset)` 调 `layoutTemplate`，模块加载时算一次 |
| 视口 | `CanvasViewport` 三层 + 缩放平移复位 | 不改，直接使用 |
| 渲染 | `CanvasCard` / `CanvasEdge` / `CanvasLabel` / `CanvasLegend`（只认 §5.1 类型） | 不改，直接使用 |
| 内容 | **示例内容**：6 张 `demo-*` 卡片 + 4 条边 + 1 个出口（§11） | **真实内容**：观测投影 → §5.1 视图模型的映射（数量由数据源决定，当前快照 12 张 / 12 条，不硬编码） |
| 宿主 | deep-link 自检路由 `/canvas-demo`（§11.2） | 改造 `ObservationPage` 为画布宿主；删除示例内容、`GraphCanvas`、`NodeCell` |
| 语义 | 不判定相位、不判定出口；示例直接写死四相各一 | 消费观测投影已算好的 `phase` / `exit` |

三条纪律：

1. **设施不向上取数**：`src/features/canvas/`（`demo/` 之外）不得 `import` `@/stores/agent/observation`、`@/features/observation`（K24）。数据由调用方传入。
2. **内容不向下改设施**：W2 只能组合设施（传 props、加约束），不得在 `CanvasCard` / `CanvasEdge` 内写观测语义分支；需要新语义时先改本规格。
3. **W2 不交付「12 张卡片」**：W2 交付的是**映射与摆放**，卡片数量由数据源（模板）决定；模板增删卡片时 W2 不得编译失败或静默漏卡以外的任何行为变化（W2 §4.5 / A2 按 `AGENT_NODE_IDS.length` 断言，不写死 12）。

### 1.7 示例内容纪律（为什么有、为什么不用观测的 12 节点）

**为什么必须有示例**：设施没有可见结果就无法验收——视口缩放锚点、拖拽期 memo 行为、四相可区分性、活动动画、焦点环、出口角标，都只能在真实渲染下走查。只有单测的话，W1 交付的是一堆无法证明可用的函数。

**为什么不用观测的 12 个节点**：

| # | 若用观测 12 节点 | 后果 |
|---|---|---|
| 1 | 示例即真实内容 | W2 换内容时要重写示例与测试，示例反而成了第二套内容实现（重蹈覆辙） |
| 2 | 示例的卡片数 = 12 变成设施验收的隐含前提 | 测试会写死 12 个盒子，模板一变就红，逼出「12 是不变量」的错误结论 |
| 3 | 示例 import 观测模板 | 依赖方向反转（设施 → 观测），W2 的内容映射层失去意义（K24 失败） |
| 4 | 示例带观测文案键 | 图例与卡片会提前绑定观测语义，W2 无法换预设 |

**示例的四条纪律**：

1. id 一律 `demo-*`，**不得**出现任何观测 id（`admit` / `prepare` / …）与观测类型名（K25）；
2. 示例不得 `import` 观测模块、不得订阅事件流、不得读 store；
3. 示例是**临时脚手架**：W2 删除整个 `demo/`、`module.ts`，并从 `registry.ts` 去掉装载；
4. 示例只覆盖设施需要的最小形状：两行布局、四相各一、一个出口、一个标签（§11.1）。

---

## 2 术语引用（不重复定义）

### 2.1 引用表

| 层 | 术语 | 出处 |
|---|---|---|
| 容器（W1） | 画布、视口、缩放、平移 | 词表 §2.5.2 |
| 渲染（W1） | 卡片、盒子、锚点、边、连线 | 词表 §2.5.1 / §2.5.2 / §2.5.3 |
| 原语（W1） | 坐标系、视口状态、帧合并、抑制、锚点推导、反原语 | 词表 §2.6 |
| 内容（W2） | 卡片模式、运行摘要、模板边、前向边、回边、活动边、相位、圈、出口、工具分支 | 词表 §2.5.2 / §2.5.3 |

### 2.2 用词纪律

| 禁止 | 应写 | 说明 |
|---|---|---|
| 「观测画布」 | 画布 | 词表无此词；「画布」自带视口与缩放语义（词表 §2.5.2），旧称在本规格废弃 |
| 「画布」（不加限定） | 画布 | 「画布」是完整术语，不另造限定词（「只读画布」同样是自造词）；指 L2 编辑面时用「编排画布」，指 L1 栅格时用「观测栅格」（§1.5 T-1） |
| 「观测栅格」「栅格」 | 画布 / 布局网格 | 本文的交付物是画布；`row` / `column` 只是布局输入 |
| 「节点」 | 卡片 | 「节点」是原语词（`ObservationNode` / `WorkflowNode`），不是用户词；画布上的渲染形态是**卡片**（词表 §2.5.1） |
| 「格子」「单元格」 | 卡片 / 盒子 | 格子是 `row` / `column` 视图模型、单元格是**观测栅格**里的渲染形态；本规格画的是画布，用盒子（数据）+ 卡片（渲染）（§1.3 D-1 / §1.5 T-10） |
| 「示例卡片」用于用户文案 | 不出现 | 示例内容只存在于代码与自检路由，不进任何用户可见文案（OQ-1） |
| 「出口边」 | 出口（`exit`） | W1 不画出口边元素；出口语义由内容层的 `exit` 承载，设施只提供角标通道（§1.5 T-9） |
| 端口 | 锚点 | 运行内容不可接，端口是 L2 概念 |
| 连线的「一条边」 | 一条边 | 计数单位是「条」 |
| 「未完成」用于边 | 「未通行」（`untaken`） | 「未完成」专指卡片停在中间态，属内容层文案 |
| 「边」与「接线」互换 | 边（数据形状）/ 连线（渲染形态）/ 接线（L2 语义实例） | 词表 §2.5.4 |

**不出现的 L2 词**（不是漏写，是 W1 不交付）：

| 词 | 词表定义 | 为何 W1 不出现 |
|---|---|---|
| 执行卡片 execution card | 引用一个**执行单元**的卡片，`WorkflowNode` 的主类别 | W1 无工作流模板，没有 `WorkflowNode`，也就没有「引用执行单元」这层语义 |
| 容器卡片 container card | 承载**子图**的卡片（会话级 / 迭代容器），先占名、未立项 | W1 不渲染嵌套子图 |
| 执行单元 execution unit | 被 `WorkflowNode` 引用的实体 | 同上，无引用关系，故不出现 |
| 卡片类别 card kind | 执行卡片按执行单元分型（turn / 工具调用 / 会话 / HITL） | 闭集未定案（词表标「未定案」），W1 不得预设 |

---

## 3 约束来源与效力

| 约束 | 来源 | 效力 |
|---|---|---|
| 画布不是控制源：运行模式只读，不写 store、不发命令、不回写 wire | 概念清单 O6 / O10；分析 P1 | 硬：违反即不通过 |
| 布局与渲染零 DOM 测量 | 分析 P11 / N7 | 硬 |
| 颜色只取 antd 变量、禁止硬编码与 fallback | DESIGN §5.3；项目规则「antd 主题对齐」 | 硬 |
| 文案只取 i18n、禁止内联中文 | 项目规则「i18n」；分析 P12 | 硬（设施键 `canvas.*`；内容文案由调用方传入） |
| 键盘可达 + 焦点可见 + 非仅靠颜色编码 | 分析 P12 / P13 | 硬 |
| 视口是本地 UI 状态，不进投影入参、不进 wire | 项目规则「wire 不承载本地 UI 状态」；分析 P6 / P9 | 硬 |
| 单一失效调度点：投影重算只经 `subscribeProjections` | 分析 P6 | 硬（W1 设施**不订阅**，只接收 props） |
| 动画只由状态驱动 | 分析 P10 | 硬 |
| 全局快捷键须注册 keymap | 分析 P13 | 软：W1 只用容器内按键与指针手势，不注册。将来加全局快捷键**不能**在前端手写：`shortcutDefaults.generated.ts` 由 `npm run contract:gen` 从 Rust `DEFAULT_SHORTCUT_PATTERNS` 生成，手改会被 `contract:verify` 的 `git diff --exit-code` 拦下 |

---

## 4 不变量

| # | 不变量 | 判据 |
|---|---|---|
| N1 | 画布不是控制源：不写 store、不发事件、不回写 wire | K13（`features/canvas/` 无 store 写入 / `invoke` / `send`） |
| N2 | 内容只读：设施不产出、不改写内容数据，只把 props 画出来 | K13 / K14 |
| N3 | 单一失效调度点：设施不订阅事件流；投影重算只经 `subscribeProjections`（在内容层） | K13（不 import 观测模块 / 调度模块） |
| N4 | 本地 UI 状态不进 wire：`selectedId` / `viewport` 只是组件本地 state，不进任何 DTO、不进投影入参 | K14 + typecheck |
| N5 | 几何单源：盒子只由 `layoutTemplate` 产出，线段只由 `edgeGeometryOf` 产出，不存在第二套几何 | K1 / K4 |
| N6 | 卡片必须不透明：任何卡片不得使用 `opacity < 1` | K11 |
| N7 | 几何零测量：盒子坐标由纯函数从布局常量推导；视口变换只改 `transform` | K3 / K6 |
| N8 | 出口不混入边集：设施不产出「出口边」元素，角标只挂在卡片上 | K12 + W2 内容不变量 |
| N9 | 设施不缓存跨帧派生数据：组件内不持有除 `selectedId` / `viewport` 外的可跨帧状态 | 代码审查 |
| N10 | 设施不认识内容：不出现观测 id / 观测类型 / 固定数量（示例内容除外，且示例只允许出现在 `demo/`） | K24 / K25 |

---

## 5 设施视图模型

设施只认下面这套类型；它**不是**观测视图模型，也不含任何内容语义（谁 active、谁是出口由调用方决定）。

### 5.1 通用类型（`src/features/canvas/types.ts`）

```ts
export interface CanvasBox { x: number; y: number; w: number; h: number }

/** 卡片语义色调通道。设施只定义通道，取值由调用方映射。 */
export type CardTone = "default" | "active" | "success" | "warning" | "danger" | "muted";
export type BadgeTone = "neutral" | "positive" | "warning" | "negative";

/** 边相位：四个取值即编码词表（§5.3），判定在调用方。 */
export type EdgePhase = "untaken" | "idle" | "passed" | "active";

/** 出口三态：角标编码（§5.4），判定在调用方。 */
export type ExitTone = "failed" | "cancelled" | "stopped";

export interface CanvasBadgeView {
  text: string;
  tone?: BadgeTone;
}

export interface CanvasCardView {
  id: string;
  box: CanvasBox;
  /** 已本地化的标题与副标题；设施不调用任何内容 i18n 键。 */
  title: string;
  subtitle?: string;
  badges?: readonly CanvasBadgeView[];
  tone?: CardTone;
  /** `null` / 省略 = 不画出口角标（§8.4）。 */
  exit?: ExitTone | null;
}

export type EdgeRoute = "horizontal" | "vertical" | "outer-track";

export interface CanvasEdgeView {
  id: string;
  from: string;
  to: string;
  route: EdgeRoute;
  phase: EdgePhase;
}

/** 世界坐标里的纯文本标签（区带说明等），不参与选中与焦点。 */
export interface CanvasLabelView {
  id: string;
  box: CanvasBox;
  text: string;
  tone?: "muted" | "default";
}
```

字段纪律：

| 类型 | 允许 | 禁止 |
|---|---|---|
| `CanvasCardView` | `id` / `box` / 已本地化文案 / `tone` / `badges` / `exit` | 状态枚举、证据等级、耗时起点、圈数、任何观测字段 |
| `CanvasEdgeView` | `id` / `from` / `to` / `route` / `phase` | `kind` / 标签 / 路径字符串 / 像素坐标 / 箭头样式 |
| `CanvasLabelView` | `id` / `box` / `text` / `tone` | 点击处理器、图标组件、可聚焦标记 |

`from` / `to` 引用卡片 `id`；边渲染器按 id 在盒子表里取盒子，取不到即不渲染（并触发开发期断言，不做兜底画线）。

### 5.2 内容映射归属调用方

`内容视图模型 → CanvasCardView / CanvasEdgeView` 的映射由调用方提供：

- W1 示例：`demo/demoContent.ts`（§11.1）；
- W2 真实内容：观测投影适配层（W2 §4.3 / §7）。

本规格只保证：

- 设施**不** import 观测模块（K24）；
- 设施**不**出现观测类型名（K24）；
- 映射结果满足 §5.1 的字段纪律（调用方负责）。

这样「换一份内容」（编排画布、另一套预设、示例）不需要改设施，只换映射。

### 5.3 相位四值的编码契约

| 取值 | 词表描述句（§2.5.3） | 编码 |
|---|---|---|
| `untaken` | 未开始 / 跳过 | 虚线 1px、弱色、无箭头 |
| `idle` | —（词表未列，本规格补） | 实线 1px、弱色、无箭头 |
| `passed` | 已完成 | 实线 2px、主色、有箭头 |
| `active` | 进行中 | 实线 2px、主色、有箭头 + 虚线流动动画 |

判定（何时取哪一相）不在本规格：由观测投影层算出并随 `CanvasEdgeView.phase` 传入（W2 §6.3）。

### 5.4 出口三态的编码契约

| 取值 | 语义 | 编码 |
|---|---|---|
| `failed` | 失败收束 | 角标取 `--ant-color-error` |
| `cancelled` | 取消收束 | 角标取 `--ant-color-warning` |
| `stopped` | 正常收束 / 预算中止 | 角标取 `--ant-color-text-secondary` |

判定（哪张卡片带出口、取哪一态）不在本规格：由调用方随 `CanvasCardView.exit` 传入。

### 5.5 布局原语：`layoutTemplate`

画布的坐标不来自 CSS 布局，而来自**纯函数**：条目的 `row` / `column` + 布局常量 → 盒子。

```ts
export interface CanvasLayoutEntry {
  id: string;
  row: number;
  column: number;
}

export const CANVAS_METRICS = {
  cellW: 132, cellH: 78, gap: 8, loopGutter: 16,
} as const;

export interface CanvasLayout {
  boxes: ReadonlyMap<string, CanvasBox>;
  width: number;
  height: number;
}

export function layoutTemplate(entries: readonly CanvasLayoutEntry[]): CanvasLayout;
```

坐标公式（世界坐标，原点 = 内容左上角）：

| 项 | 公式 |
|---|---|
| `x` | `column * (cellW + gap)` |
| `y` | `loopGutter + gap + row * (cellH + gap)` |
| `w` / `h` | `cellW` / `cellH` |
| 世界宽 | `(maxColumn + 1) * cellW + maxColumn * gap` |
| 世界高 | `loopGutter + gap + (maxRow + 1) * cellH + maxRow * gap` |

纪律：

- 布局函数**只**吃条目与常量，不读 DOM、不吃视口；同一份输入在任何窗口尺寸下产出同一份坐标（N7）。
- `id` 是 `string`：设施不认 `AgentNodeId`（K24）。
- 布局结果是**常量**：由调用方在模块加载时算一次（§12.2），设施自身不缓存。
- `row` / `column` 是布局输入，不是视图模型；视图模型只有 `CanvasBox`（§1.3 D-1）。
- 空输入返回 `{ boxes: 空 Map, width: 0, height: 0 }`；调用方渲染空画布，不做占位卡片。

---

## 6 坐标模型与视口

### 6.1 三层结构与两个坐标系

```
CanvasViewport   （屏幕层：overflow: hidden，接收指针与滚轮）
└── world layer  （世界层：transform: translate(x, y) scale(s)，transform-origin: 0 0）
    ├── edges    （边元素，绝对定位，世界坐标）
    ├── cards    （盒子 → 卡片，绝对定位，世界坐标）
    └── overlays （标签 / 角标等，绝对定位，世界坐标）
```

**词与物**：`CanvasBox` 是**盒子**（数据形状，`x` / `y` / `w` / `h`）；`CanvasCard` 是**卡片**（渲染形态，把盒子画出来）。

| 坐标系 | 单位 | 谁产生 | 谁消费 |
|---|---|---|---|
| 世界坐标 | px（整数，来自 `layoutTemplate`） | `layoutTemplate()` | 边几何、卡片 `left` / `top` / `width` / `height` |
| 屏幕坐标 | px（含变换） | `viewport = { x, y, scale }` | 指针位置换算、缩放锚点 |

**唯一变换点**：`transform` 只出现在世界层这一处。卡片、边、标签内部**不得**再带任何 `translate` / `scale`，否则缩放会叠加成两级。

```ts
export interface CanvasViewportState {
  x: number;      // 世界层在屏幕上的平移
  y: number;
  scale: number;  // 缩放倍率
}

export const VIEWPORT_DEFAULT: CanvasViewportState = { x: 0, y: 0, scale: 1 };
export const VIEWPORT_SCALE_MIN = 0.5;
export const VIEWPORT_SCALE_MAX = 2;
```

初始值固定为 `{ x: 0, y: 0, scale: 1 }`：原点对齐视口左上，不测量容器、不做 fit（§1.2 第 9 条）。

### 6.2 屏幕坐标与世界坐标互转

两个纯函数，供缩放锚点与将来的编辑命中区共用：

```ts
export function screenToWorld(p: Point, v: CanvasViewportState): Point {
  return { x: (p.x - v.x) / v.scale, y: (p.y - v.y) / v.scale };
}

export function worldToScreen(p: Point, v: CanvasViewportState): Point {
  return { x: p.x * v.scale + v.x, y: p.y * v.scale + v.y };
}
```

`p` 的输入是相对**视口**的坐标（`event.clientX - viewportRect.left`）。这里需要一次视口矩形的读取——它只发生在**指针事件处理器**里，不在渲染路径上，且不参与几何推导（N7 只约束「几何来源」）。为避免每次事件都读，视口矩形在 `pointerdown` 时缓存一次、`pointerup` 时丢弃。

### 6.3 缩放与平移交互

| 手势 | 行为 | 约束 |
|---|---|---|
| `Ctrl` / `⌘` + 滚轮 | 缩放，以**指针位置**为锚点 | 步进 `1.1`；`scale` 夹在 `[0.5, 2]`；`preventDefault`（否则页面会整体缩放） |
| 滚轮（无修饰键） | 纵向平移，`y += deltaY`；`Shift` + 滚轮为横向平移 | `preventDefault` |
| 指针拖拽空白处 | 平移，`x += dx`、`y += dy` | 拖拽起点必须是视口或世界层背景，不是卡片/边 |
| 缩放按钮（`+` / `-` / 复位） | 以**视口中心**为锚点 | 中心由视口矩形算出（同 §6.2 的读取时机），不在渲染路径 |
| 双击卡片 | 无行为（W1 不做 fit-to-card） | 双击空白处也不复位，避免误触 |

缩放锚点的数学（保持锚点下的世界坐标不动）：

```
s' = clamp(s * k, MIN, MAX)
x' = px - (px - x) * (s' / s)
y' = py - (py - y) * (s' / s)
```

滚轮监听必须用**原生非 passive** 监听器（`useEffect` 里 `addEventListener("wheel", handler, { passive: false })`）：React 的 `onWheel` 在部分环境下是 passive，`preventDefault()` 无效，会导致整个页面跟着缩放。

**视口状态走 React state**（`useState<CanvasViewportState>`），不用 ref + 命令式写 DOM。代价是拖拽每帧触发一次重渲染，因此 `CanvasCard` / `CanvasEdge` 必须 `React.memo`，且 `transform` 只落在世界层——每帧重渲染的只有视口容器本身与子元素的 props 比对，不触碰内容数据（§12.5）。

### 6.4 锚点与边几何

锚点由盒子**推导**，不做测量：锚点 = 盒子边框上的固定交点。

| 锚点 | 位置（世界坐标） |
|---|---|
| 出锚点（右侧） | `(b.x + b.w, b.y + b.h / 2)` |
| 入锚点（左侧） | `(b.x, b.y + b.h / 2)` |
| 底锚点（下侧） | `(b.x + b.w / 2, b.y + b.h)` |
| 顶锚点（上侧） | `(b.x + b.w / 2, b.y)` |

设施提供三类 route，几何全部由两个盒子推出：

```ts
export interface EdgeSegment { x: number; y: number; w: number; h: number }
export interface EdgeArrow { x: number; y: number; direction: "left" | "right" | "down" }
export interface EdgeGeometry { segments: readonly EdgeSegment[]; arrow: EdgeArrow | null }

export function edgeGeometryOf(
  from: CanvasBox,
  to: CanvasBox,
  route: EdgeRoute,
  gutter: number,
): EdgeGeometry;
```

| route | 适用 | 几何 | 元素 |
|---|---|---|---|
| `horizontal` | 同一行相邻 | `x = from.x + from.w`，`y = yMid - 0.5`，`w = to.x - x`，`h = 1` | 1 段 + 箭头（指向右） |
| `vertical` | 上下相邻 | `x = xMid - 0.5`，`y = from.y + from.h`，`w = 1`，`h = to.y - y` | 1 段 + 箭头（指向下） |
| `outer-track` | 跨列回到起点（外轨） | 三段：右竖 + 顶横 + 左竖（下表） | 3 段 + 箭头（指向左） |

外轨三段（`R = from.x + from.w`、`L = to.x`、`G = gutter`）：

| 段 | x | y | w | h |
|---|---|---|---|---|
| 右竖 | `R - 0.5` | `G` | `1` | `from.y - G` |
| 顶横 | `L` | `G - 0.5` | `R - L` | `1` |
| 左竖 | `L` | `G` | `1` | `to.y - G` |

箭头：三角形，`horizontal` 落在目标入锚点、指向右；`vertical` 落在目标顶锚点、指向下；`outer-track` 落在顶横左端、指向左。尺寸 `border-width: 5px 7px 5px 0`（左向）；颜色取该边的 `currentColor`。

几何纪律：

- 边的端点**正好**停在盒子边框上，不越入盒子内部：坐标是精确值（不是网格轨道），没有「轨道间隙不确定」的问题。
- 所有线段 `width` 或 `height` 为 `1`，另一端对齐 `-0.5` 保证落在像素中心（`scale = 1` 时清晰）。
- 缩放后 1px 线允许亚像素抗锯齿；不得为了「始终 1 屏幕像素」而在缩放时改线宽（那会引入第二套几何）。
- route 由调用方给出（示例与 W2 各自决定），设施不做「自动选 route」：自动路由需要图算法，且会把布局语义塞进渲染器。

### 6.5 渲染顺序与不透明

世界层内按 DOM 顺序决定层叠（不使用 `z-index` 手工排序）：

1. `edges`——最低
2. `cards`——覆盖边端点的抗锯齿溢出一像素
3. `overlays`（标签等）

`N6`：卡片不得 `opacity < 1`（半透明会压暗文字、降低对比度，且边线会透出）。

### 6.6 布局尺寸变量

布局常量进 `global.css`（`:root`），与 `layout.ts` 的 `CANVAS_METRICS` **一一对应**，且由测试比对（K23）：

| 变量 | 值 | 用途 |
|---|---|---|
| `--canvas-cell-w` | `132px` | 盒子宽 |
| `--canvas-cell-h` | `78px` | 盒子高 |
| `--canvas-gap` | `8px` | 盒间距 |
| `--canvas-loop-gutter` | `16px` | 顶部外轨高 |

`CANVAS_METRICS` 是几何的唯一真值；CSS 变量只供 `padding` / 内边距 / 标签宽度等非几何用途，几何（`left` / `top` / `width` / `height`）一律由 JS 算好后写内联样式，**不**从 CSS 变量读回（读回需要测量）。

### 6.7 禁止事项

| 禁止 | 原因 |
|---|---|
| `getBoundingClientRect` / `offsetWidth` / `clientWidth` 参与几何推导 | 违反 N7；几何只能来自 `layoutTemplate` |
| `ResizeObserver` | 同上（§1.2 第 9 条） |
| CSS Grid / Flex 定位卡片 | 画布必须有坐标系；CSS 布局无法表达缩放与平移（§1.3 D-1） |
| 卡片内再写 `translate` / `scale` | 会与视口变换叠加（§6.1） |
| 用 SVG / Canvas 画边 | 会引入第二套尺寸体系（`viewBox` / 位图缩放）；直线边用绝对定位 div 足够 |
| 百分比尺寸 | 世界坐标是 px；百分比会随视口变化，破坏「世界坐标与屏幕坐标分离」 |

---

## 7 视觉编码（四相）

### 7.1 一表定完

| 相位 | 线型 | 线宽 | 颜色（`currentColor`） | 透明度 | 动画 | 箭头 |
|---|---|---|---|---|---|---|
| `untaken` 未通行 | 虚线 `3 3` | `1px` | `--ant-color-border` | — | 无 | 无 |
| `idle` 待通行 | 实线 | `1px` | `--ant-color-border` | — | 无 | 无 |
| `passed` 已通行 | 实线 | `2px` | `--ant-color-primary` | — | 无 | 有 |
| `active` 活动中 | 实线 | `2px` | `--ant-color-primary` | — | 虚线流动 `1.2s` 循环 | 有 |

区分手段有**四维**（线型 / 线宽 / 颜色 / 动画），任何一维单独即可判读，不依赖颜色单通道（分析 P12 / P13）。色弱用户靠线型与线宽即可分辨四相。

### 7.2 `currentColor` 单一着色点

每条边元素设 `color: <phaseColor>`，其内部的线（`border`）、箭头（`border-left-color`）、标签（`color: inherit`）全部用 `currentColor` 继承。禁止在子元素上再写一遍颜色——否则主题切换时会出现「线变了、箭头没变」。

### 7.3 动画

```css
@keyframes canvas-edge-flow { to { background-position: 12px 0; } }

.canvas-edge[data-phase="active"] {
  background-image: repeating-linear-gradient(
    90deg, currentColor 0 4px, transparent 4px 8px
  );
  background-size: 8px 100%;
  animation: canvas-edge-flow 1.2s linear infinite;
}
```

约束：

- 动画**只**由 `phase === "active"` 驱动（分析 P10）：没有活动边时页面上不得有任何正在运行的动画。
- 竖线段用 `180deg` 的同一 `repeating-linear-gradient`，`background-size: 100% 8px`，`@keyframes` 改为 `background-position: 0 12px`。
- 必须提供 `@media (prefers-reduced-motion: reduce)` 分支：`animation: none`，改用 `outline` 的虚线（`1px dashed currentColor`）表达活动态——不能因为关掉动画就丢掉「活动」这个语义。

### 7.4 编码只由属性驱动

设施组件**不得**根据内容语义分支渲染：`CanvasEdge` 只读 `data-phase`，`CanvasCard` 只读 `data-tone` / `data-selected` / `data-exit`，CSS 选择器只认这些属性。调用方换映射时不需要改组件与样式——这是「一套渲染设施、多套视图模型」（分析 §1.1）在代码上的落点。

---

## 8 出口角标（设施组件）

### 8.1 为什么收进盒子

出口徽标若画在盒子外侧，需要额外一段水平空间：相邻盒子之间只有 `--canvas-gap`（8px），放不下「短桩 + 汇点」；若加宽间隙，盒子会被迫重排。故改为**盒内角标**（§1.3 D-3），几何上零额外空间。

| 部件 | 位置 | 尺寸 |
|---|---|---|
| 折角括号（L 形） | 盒子右下角内侧，`right: 6`、`bottom: 6` | `border-right` + `border-bottom` 各 `2px`，长 `12px` |
| 汇点方块 | 折角括号内侧端点 | `6 × 6`，`border-radius: 50%` |

### 8.2 三态

出口用 `data-exit="failed|cancelled|stopped"` 区分三态，颜色见 §5.4。角标本身**不带文字**（盒子只有 132 × 78，塞不进标签）；语义由图例（§10）与调用方的头信息行承担。

### 8.3 出口与选中的关系

出口角标属于卡片，选中该卡片时角标不变化（不放大、不换色）。禁止把出口做成「第 13 个可选中对象」：它没有卡片身份，键盘焦点序列里不得出现。

### 8.4 无出口时

`exit` 为 `null` / 省略时不画角标，卡片布局与几何完全不变（角标是盒内装饰，不参与布局）。**不得**用「最近一张卡片」兜底：出口的判定属内容层，设施不做任何猜测。

---

## 9 选择与键盘

### 9.1 选中语义

- 选中对象是**卡片**（不是「格子 × 圈」，§1.3 D-7）。`selectedId: string | null`，存在 `CanvasViewport` 的本地 state（N4）。
- 选中是**受控/非受控二选一**：`CanvasViewport` 提供 `selectedId` + `onSelect` 两个可选 props；调用方不传时由设施内部 state 持有（示例与 W2 都用非受控，宿主若要与 Inspector 联动则传受控）。
- 选中态**不跨实例保留**：内容切换（`cards` 数组换源）时重置为 `null`（不同内容的状态不同，保留选中会让人误读）。视口状态**跨实例保留**（布局恒定，视口有意义）。
- 选中只影响：卡片 `data-selected` 属性 + 调用方的 `onSelect` 回调。**不**改变边相位、不改变出口、不写 store（N1 / N2）。

### 9.2 键盘

| 按键 | 行为 | 范围 |
|---|---|---|
| `Tab` / `Shift+Tab` | 焦点在卡片间循环（DOM 顺序 = `cards` 数组顺序） | 全局默认行为，无需自定义 |
| `Enter` / `Space` | 选中当前聚焦卡片（原生 `<button>` 行为） | 卡片元素 |
| `Esc` | 清空选中，焦点留在视口容器 | 容器内 `keydown`，不注册全局快捷键 |
| `Ctrl` / `⌘` + `+` / `-` | 缩放（同按钮） | 容器内 `keydown` + `preventDefault` |
| `Ctrl` / `⌘` + `0` | 视口复位 | 同上 |

卡片是原生 `<button type="button">`，`aria-pressed={selected}`，`aria-label` 由 `title` + `subtitle` 拼接（文案由调用方给，设施不拼 i18n 键）。焦点环用 `outline: 2px solid var(--ant-color-primary)` + `outline-offset: 2px`，缩放后仍在视野内（`outline` 随 `transform` 缩放，可接受）。

### 9.3 头信息行（视口外，不随缩放）

设施提供一条固定高度（`32px`）的头信息行，内容由调用方以 `header` 插槽传入（`ReactNode`），设施自身只放**缩放倍率 + 缩放按钮组**：

| 项 | 归属 | 说明 |
|---|---|---|
| 缩放倍率 | 设施 | `×1.00` 形式，只读展示 |
| 缩放按钮组 | 设施 | `+` / `-` / 复位，`aria-label` 走 i18n（`canvas.zoom_*`） |
| 内容摘要（活动边 / 圈 / 出口等） | 调用方 | 通过 `header` 插槽传入；设施不读任何内容字段 |

W1 示例页在 `header` 里放「示例内容 · 非真实数据」标记（§11.2）；W2 在 `header` 里放预设名、内置/只读标记与活动边摘要（W2 §8.1）。

### 9.4 指针手势与焦点的冲突

卡片上按下指针**不**开始平移（`pointerdown` 判断 `target.closest("[data-canvas-card]")`，命中即跳过）。拖拽平移期间设置 `cursor: grabbing`，且不改变选中——避免「拖了一下画布就换了选中」。

---

## 10 图例（设施编码）

### 10.1 两组设施图例 + 调用方可追加组

图例是画布的**固定部分**，放在视口下方（不随缩放、不随平移）。设施自带两组（相位、出口），其余组（卡片状态、证据等级等）由调用方通过 `groups` 追加：

```ts
export type CanvasLegendItem =
  | { kind: "edge"; phase: EdgePhase; label: string }
  | { kind: "exit"; tone: ExitTone; label: string }
  | { kind: "card"; tone: CardTone; label: string }
  | { kind: "badge"; tone: BadgeTone; label: string };

export interface CanvasLegendGroup {
  id: string;
  title: string;
  items: readonly CanvasLegendItem[];
}

export interface CanvasLegendProps {
  /** 设施两组（phase / exit）默认渲染；传 false 可关闭。 */
  showFacilityGroups?: boolean;
  /** 调用方追加的内容组，按数组顺序渲染。 */
  groups?: readonly CanvasLegendGroup[];
}
```

| 组 | 归属 | 条目 |
|---|---|---|
| 边相位 | 设施 | 四值（`untaken` / `idle` / `passed` / `active`） |
| 出口 | 设施 | 三态（failed / cancelled / stopped） |
| 卡片状态 / 证据等级 / 圈与分支 | 调用方（W2） | W2 §8 / §10 |

### 10.2 图例必须复用同一份编码

图例里的线样、角标、色调必须与画布上的元素**共用同一组 CSS 类**（`.canvas-edge[data-phase=...]`、`.canvas-card[data-tone=...]`、`.canvas-exit[data-exit=...]`），不得另写一套「图例专用样式」——否则改一处忘一处，图例与画布会分叉（K12）。

### 10.3 设施 i18n 键

`src/i18n/locales/{zh-CN,en-US}.json` 需新增（两语言同步）：

| 键 | 中文 | 英文 |
|---|---|---|
| `canvas.edge.untaken` | 未通行 | Not taken |
| `canvas.edge.idle` | 待通行 | Idle |
| `canvas.edge.passed` | 已通行 | Passed |
| `canvas.edge.active` | 活动中 | Active |
| `canvas.exit.failed` | 失败出口 | Failed exit |
| `canvas.exit.cancelled` | 取消出口 | Cancelled exit |
| `canvas.exit.stopped` | 收束出口 | Stopped exit |
| `canvas.legend.edge_phase` | 边相位 | Edge phase |
| `canvas.legend.exit` | 出口 | Exit |
| `canvas.viewport` | 画布 | Canvas |
| `canvas.zoom_in` | 放大 | Zoom in |
| `canvas.zoom_out` | 缩小 | Zoom out |
| `canvas.zoom_reset` | 复位视图 | Reset view |
| `canvas.zoom_level` | 缩放 | Zoom |

示例内容另有 `canvas.demo.*` 键（§11.1），W2 落地时随示例一并删除。

---

## 11 示例内容（W1 自检用）

### 11.1 形状（`src/features/canvas/demo/demoContent.ts`）

| 项 | 值 |
|---|---|
| 布局条目 | 6 条，两行：`row 0` 列 0–3（4 张）；`row 1` 列 3–4（2 张） |
| 卡片 | 6 张，id `demo-a` … `demo-f`；`title` 取 `canvas.demo.card.*`，`subtitle` 取 `canvas.demo.subtitle.*` |
| 边 | 4 条：3 条 `horizontal`（同行相邻）+ 1 条 `vertical`（列 3 的上下两张）；四相各一（`untaken` / `idle` / `passed` / `active`） |
| 出口 | 1 个，`demo-d` 带 `exit: "stopped"` |
| 标签 | 1 个，第二行左侧留白处 `text` 取 `canvas.demo.lane`（`CanvasLabelView`） |
| 图例 | 只用设施两组（不追加内容组） |

覆盖检查：两行布局覆盖 `y` 公式与跨行 route；3 + 1 条边覆盖三类 route 中的两类；四相各一覆盖 §7.1 四行；一个出口覆盖 §8.2；一个标签覆盖 `overlays` 层与 memo 边界。

### 11.2 自检路由

| 项 | 值 |
|---|---|
| 文件 | `src/features/canvas/module.ts`（`FeatureModule`，只有 `routes`，无 `sidebar`） |
| 路由 | `path: "/canvas-demo"`，`boundary: "画布设施自检"`，`visibility: "deepLinkOnly"` |
| 页面 | `demo/CanvasDemoPage.tsx`：`CanvasViewport`（`cards` / `edges` / `labels` / `legendGroups` 来自 `demoContent`）+ `header` 里的「示例内容 · 非真实数据」标记 |
| 装载 | `src/features/registry.ts` 在 `observationModule` 之后无条件 `await import("./canvas/module")`（示例不含数据依赖，无需编译期开关） |
| 侧栏 | 无条目：`deepLinkOnly` 保证不出现在导航里，只能手输 URL 或开发期直开 |
| 退场 | W2 删除 `demo/` 与 `module.ts`，并删除 `registry.ts` 的装载段（W2 §1.2 第 7 条） |

### 11.3 为什么示例必须可整体删除

示例是**唯一**允许出现「写死内容」的地方；把它关在 `demo/` 一个目录里，W2 的验收就是「删除该目录后编译通过、测试通过、真实宿主工作」——这同时证明了设施没有偷偷依赖示例（K19 / K20）。

---

## 12 失效与动画

### 12.1 三类失效，互不越界

| 失效源 | 触发 | 重算什么 | 不得触发什么 |
|---|---|---|---|
| 内容失效 | 调用方换 `cards` / `edges` props（W2 由 `subscribeProjections` 驱动） | 卡片 / 边 / 标签的渲染 | 不得重建布局（布局是调用方常量）；设施不订阅 |
| 视口失效 | 用户缩放 / 平移 | 世界层 `transform` | 不得重算内容、不得重新布局 |
| 选中失效 | 用户点击 / 键盘 | 卡片 `data-selected` + `onSelect` 回调 | 不得重算内容、不得改相位 |

三类失效各自的更新路径互不调用，这是「单一失效调度点」（N3）在画布上的展开：调度点只在内容层（`subscribeProjections`），视口与选中是纯本地视图状态。

### 12.2 布局在模块加载时算一次

布局结果是**调用方常量**：示例在 `demoContent.ts` 模块作用域算一次；W2 在 `layoutPreset` 的模块作用域算一次。设施不缓存布局，任何失效都不得重算它。

### 12.3 帧合并

事件流到画布的链路（W2 起）：Rust fact → `subscribeProjections`（合并）→ 内容映射 → 一次 React 提交。画布设施**不得**自己再订阅事件流（否则同一事实会有两条更新路径，出现「边闪一下」）。

### 12.4 无时钟

画布内禁止 `setInterval` / `setTimeout` / `requestAnimationFrame` 驱动数据更新：

- 时长数字只显示「已记录的耗时」，不做逐秒累加（§1.3 D-6）；
- 唯一允许的 `requestAnimationFrame` 用法是**输入合帧**（若滚轮缩放出现抖动，可把连续 wheel 事件合并到一帧再 `setState`），且不得用于动画推进；
- CSS 动画（§7.3）由 `data-phase` 驱动，不是由 JS 计时驱动。

### 12.5 memo 决策（条件已满足，必须执行）

分析文档 §12 记过一条：触发条件满足任一时必须重新评估 memo——「元素数 > 200」或「引入 hover / 拖拽这类每帧必变的本地状态」。视口拖拽正属后者，且元素数随内容线性增长（示例 6 卡片 + 4 边段 + 1 标签；W2 起 12 卡片 + 15 边段 + 1 标签），故**结论是必须 memo**：

| 组件 | memo | 依赖的 props | 说明 |
|---|---|---|---|
| `CanvasCard` | 是 | `card`、`selected` | 选中变化只让 2 张卡片重渲染 |
| `CanvasEdge` | 是 | `edge`、`from` / `to` 盒子引用 | 盒子引用来自调用方常量，稳定 |
| `CanvasLabel` | 是 | `label` | 标签不变 |
| `CanvasLegend` | 是 | `groups` | 挂载后不重渲染 |
| `CanvasViewport` | 否 | — | 视口 state 的持有者，本来每帧要变 |

memo 的前提是 props **引用稳定**：`cards` / `edges` 数组由调用方在模块加载或一次映射里构造（不得在渲染函数里新建），因此拖拽期间所有子组件的 props 比对都命中，只有世界层 `transform` 变化。

---

## 13 主题、文案与可达性

### 13.1 颜色

| 用途 | 变量 |
|---|---|
| 卡片背景 | `--ant-color-bg-container` |
| 卡片边框 | `--ant-color-border` |
| 主色（已通行 / 活动 / 焦点环） | `--ant-color-primary` |
| 错误 | `--ant-color-error` |
| 警告 | `--ant-color-warning` |
| 弱化文字（标签 / `muted` 色调） | `--ant-color-text-tertiary` |
| 次要文字（收束出口） | `--ant-color-text-secondary` |

禁止 `var(--x, #fff)` 形式的 fallback（DESIGN §5.3）；禁止 `rgba()` 硬编码；禁止在 CSS 里写十六进制颜色。

### 13.2 文案

- 设施自身的键（缩放 / 图例标题 / 相位名 / 出口名）全部走 `t()`，键见 §10.3；
- 卡片文案由调用方传入（`title` / `subtitle` / `badges[].text`），设施不调用任何内容 i18n 键；
- 中英双语同步提交：现有 `localeParity.test.ts` 只覆盖 storage 子树，其余键靠人工核对（新增键必须两边同时加）。

### 13.3 可达性

| 项 | 要求 |
|---|---|
| 卡片 | 原生 `<button>`，`Tab` 可达，`aria-pressed` 表达选中 |
| 视口 | `role="group"` + `aria-label`（`canvas.viewport`），不是 `role="application"`（避免吞掉读屏器快捷键） |
| 缩放按钮 | 有 `aria-label`，不使用纯符号 |
| 非颜色编码 | 四相靠线型 + 线宽区分（§7.1） |
| 减少动效 | `prefers-reduced-motion` 分支保留活动态语义（§7.3） |
| 焦点可见 | `outline` 2px，缩放后不隐藏 |

### 13.4 性能预算

| 指标 | 目标 |
|---|---|
| 拖拽期间单帧工作 | 1 次 `setState` + 1 次 `transform` 写入 + 子组件 memo 比对 |
| 内容重算 | 每次 props 变化最多 1 次（内容层帧合并保证） |
| 内存中常驻布局 | 1 份（调用方常量） |
| 元素数 | 随内容线性增长，设施不设上限、不补占位 |

---

## 14 实施顺序（四刀，可独立验收）

每刀都以「能单独跑通测试 / 能单独看到结果」为边界；顺序不可倒置（后一刀依赖前一刀的类型）。

### 刀 1：纯函数（无 UI）

| 产出 | 文件 | 验收 |
|---|---|---|
| `CanvasBox` / 视图模型类型 | `src/features/canvas/types.ts` | K24（类型不含观测字段） |
| `layoutTemplate` + `CANVAS_METRICS` | `src/features/canvas/layout.ts` | K1 / K2 / K3 / K21 |
| `anchorOf` / `edgeGeometryOf` | `src/features/canvas/geometry.ts` | K4 / K5 |
| `screenToWorld` / `worldToScreen` / `zoomAt` / `panBy` | `src/features/canvas/viewport.ts` | K22 |

这一刀不碰 React，`vitest` 全绿即可交付。

### 刀 2：视口容器

| 产出 | 文件 | 验收 |
|---|---|---|
| 三层结构 + `transform` + 头信息行（缩放部分） | `src/features/canvas/CanvasViewport.tsx` | K6 / K7 / K8 |
| 画布样式与变量 | `src/features/canvas/canvas.module.css`、`src/assets/styles/global.css` | K23 |
| 缩放 / 平移 / 复位 / 键盘 | 同上 | K7 / K8 |

### 刀 3：卡片、边、出口 + 示例内容

| 产出 | 文件 | 验收 |
|---|---|---|
| 卡片壳 + 选中 + 出口角标 | `src/features/canvas/CanvasCard.tsx` | K11 / K12 / K17 |
| 边渲染器（三类 route + 四相 + 动画） | `src/features/canvas/CanvasEdge.tsx` | K9 / K10 / K18 |
| 标签 | `src/features/canvas/CanvasLabel.tsx` | K19 |
| 示例内容 + 自检页 + 路由装载 | `demo/demoContent.ts` / `demo/CanvasDemoPage.tsx` / `module.ts` / `registry.ts` | K19 / K20 / K25 |

### 刀 4：图例与文案

| 产出 | 文件 | 验收 |
|---|---|---|
| 设施两组图例 + 追加组通道 | `src/features/canvas/CanvasLegend.tsx` | K12 / K16 |
| 设施 14 个键 + 示例键 + 双语 | `src/i18n/locales/*.json` | K15 |

依赖关系：刀 2 需要刀 1 的类型；刀 3 需要刀 2 的视口与布局；刀 4 需要刀 3 的样式类。刀 1 可与刀 2 并行（不同文件）。

---

## 15 验收判据

> 编号说明：本节 K1–K25 是本规格**自有编号**，与分析文档的 K1–K15（命题级验收）不同，引用时须带「本规格」字样。

### 15.1 自动化（`vitest`）

| # | 判据 | 断言方式 |
|---|---|---|
| K1 | `layoutTemplate` 按 §5.5 公式产出盒子；世界宽高正确 | 表驱动 + 公式比对 |
| K2 | 布局幂等：同一输入两次调用深相等 | 深比较 |
| K3 | 布局源码不含 `getBoundingClientRect` / `offsetWidth` / `clientWidth` / `ResizeObserver` | 源码扫描 |
| K4 | 三类 route 的线段数与坐标符合 §6.4 | 表驱动 |
| K5 | 边端点落在盒子边框（误差 ≤ 0.5px），不越入盒内 | 数值断言 |
| K6 | 视口只改世界层 `transform`，卡片/边元素无 `translate` / `scale` | 源码扫描 + 渲染快照 |
| K7 | `screenToWorld` / `worldToScreen` 互逆（含 `scale = 0.5` / `2`） | 属性测试 |
| K8 | 缩放锚点公式：锚点下世界坐标在缩放前后不变（误差 ≤ 0.5px） | 数值断言 |
| K9 | 四相在 `data-phase` 四值下样式互不相同（线型 / 线宽至少一项不同） | 快照比对 |
| K10 | `active` 有动画、无 `active` 边时无动画 | 快照 + `animation` 字段断言 |
| K21 | `layoutTemplate` 的 `id` 参数为 `string`；传 `demo-a` 与传任意 id 结果一致 | 类型 + 单测 |
| K22 | `CanvasCardView` / `CanvasEdgeView` 的 `keyof` 不含观测字段（`state` / `source` / `order` / `iteration`） | 类型断言 |
| K23 | `CANVAS_METRICS` 与 `global.css` 中 `--canvas-*` 数值一致 | 读 CSS 文本正则比对（不测 DOM） |
| K24 | `src/features/canvas/`（`demo/` 之外）不含 `AgentNodeId` / `GraphView` / `TurnObservationStatus` / `@/stores/agent` / `@/features/observation` | 源码扫描 |
| K25 | `demo/` 内不含任何观测 id（`admit` / `prepare` / `branch` …）与观测类型名 | 源码扫描 |

### 15.2 人工（走查，打开 `/canvas-demo`）

| # | 判据 |
|---|---|
| K11 | 无卡片使用 `opacity < 1`；无 `var(--x, fallback)` |
| K12 | 五类元素（卡片 / 边 / 标签 / 出口角标 / 图例）与图例共用同一 CSS 类；改一处即两处变 |
| K13 | `features/canvas/` 无 store 写入、无 `invoke`、无事件订阅 |
| K14 | 选中任意卡片后，内容数据、边相位、出口均不变（N2）；视口/选中不进任何 DTO |
| K15 | 设施图例两组齐全，中英切换无缺失 |
| K16 | 图例线样与画布元素完全一致 |
| K17 | 出口角标不进入 `Tab` 序列 |
| K18 | 缩放 0.5 / 1 / 2 下四相仍可区分；`active` 动画在 `prefers-reduced-motion` 下改为虚线描边 |
| K19 | 示例页显示「示例内容 · 非真实数据」标记；删除 `demo/` 后其余代码编译通过 |
| K20 | 删除 `demo/` 与 `module.ts` 后，`registry.ts` 去掉装载段即可编译，无残留引用 |
| K26 | 无活动边时页面零动画（DevTools Animations 面板为空） |
| K27 | 拖拽平移不改变选中；拖拽期间内容不重算（React DevTools 提交次数） |
| K28 | 缩放/平移后重挂载视口保留、选中清空 |

---

## 16 已拒绝的方案

| # | 方案 | 拒绝理由 |
|---|---|---|
| R1 | 继续用 CSS Grid 摆格子（现状 `GraphCanvas`） | 无坐标系、无法缩放平移；且「观测栅格」不是本规格交付物（§1.3 D-2） |
| R2 | 引入 reactflow / d3 / dagre | 自带坐标系与测量，会与本规格的 `layoutTemplate` 形成两套几何（N7） |
| R3 | 用 SVG 画边 | 引入 `viewBox` 第二套尺寸体系；直线边用绝对定位 div 足够 |
| R4 | `ResizeObserver` 自动缩放列宽 / 自动 fit | 需测量，违反 N7（§1.2 第 9 条） |
| R5 | 视口用 ref + 命令式写 DOM（绕过 React） | 会与「状态驱动」冲突；改为 state + memo 子组件（§12.5） |
| R6 | 卡片/边用 `opacity` 表达弱化 | 压暗文字、降低对比度（N6）；改用线型与线宽 |
| R7 | 出口画成第 13 张卡片 | 出口是汇点不是卡片；会污染边集与焦点序列（N8） |
| R8 | 每帧 `setState` 之外再订阅事件流 | 同一事实两条更新路径（§12.3） |
| R9 | 前端手写全局快捷键 | 生成源在 Rust，手改会被 `contract:verify` 拦下（§3） |
| R10 | 用 `zoom` 属性或百分比尺寸做缩放 | 会改变布局盒尺寸，破坏「世界坐标恒定」（§6.7） |
| R11 | **把观测的 12 个节点当 W1 示例内容** | 示例即真实内容：W2 换内容要重写示例与测试（重蹈覆辙）；卡片数 12 会被写进设施验收，模板一变就红；示例 import 观测模块会反转依赖方向（§1.7 / K24） |
| R12 | W1 不提供任何示例，只交纯函数与空画布 | 视口、memo、动画、焦点环、出口角标都无法走查；W1 的「设施可用」没有证据（§1.7） |
| R13 | 示例内容放在 `src/features/observation/` 里 | 示例会被读成真实内容的一部分，且 W2 无法「整体删除」（§11.3） |

---

## 17 未决问题

| # | 问题 | 影响 | 建议 |
|---|---|---|---|
| OQ-1 | 示例内容是否允许进生产包（`deepLinkOnly` 路由仍在产物里） | 产物多一个懒加载 chunk 与示例文案键 | W1 接受（仅 deep-link，无侧栏入口）；W2 删除后归零 |
| OQ-2 | 多实例 / 多内容层是否要并排对比 | 会影响画布是否引入「分屏视口」 | 另立规格；W1 单内容层 |
| OQ-3 | 时间线与卡片联动高亮 | 需要跨面板 hover 与双向选中 | 暂不做；若做，需定义「悬停是否算选中」 |
| OQ-4 | `Tab` 走全部卡片是否太长（W2 起 12 张） | 可达性体验 | 可考虑 roving tabindex（方向键移动），但会增加自定义键盘逻辑；先按原生 `Tab` |
| OQ-5 | 示例形状是否够覆盖设施 | 走查充分性 | 6 卡片 + 两行 + 三类 route中的两类 + 四相 + 出口；若走查发现缺口，只加示例条目，不改设施 |
| OQ-6 | 编辑模式（拖拽 / 连线 / 参数表单）何时立项 | 决定本规格的坐标模型是否够用 | 需要 L2 数据模型（`WorkflowNode` / 参数 schema / 交接类别）先定案；本规格已按盒子建坐标，预计可直接接 |
| OQ-7 | 暗色主题下四相与出口三态的对比度 | 可读性 | 走查时用 DESIGN §5 的暗色变量实测；不足则调整线宽而非加透明度 |
| OQ-8 | 是否允许「交互中测量」做 fit-to-view | 与 N7 的边界 | N7 约束几何来源，不约束交互；若将来做 fit，只能在按钮处理器里读一次容器尺寸，不得进入布局函数 |
| OQ-9 | 内容是否应收进「容器卡片」（子图），而非平铺 | 若收，则画布变成嵌套结构，`layoutTemplate` 需算内外两层盒子，选中与焦点语义也要分层 | W1 按平铺（词表把容器卡片标为「先占名、未立项」）；若 L2 立项后要收，按 §6 的盒子模型扩一层即可，不改数据形状 |

---

## 18 偏差记录（实施时回填）

| 日期 | 章节 | 计划 | 实际 | 原因 |
|---|---|---|---|---|
| — | — | — | — | 实施后回填；无偏差时写「无」 |

---

## 19 与其他文档的关系

| 文档 | 关系 |
|---|---|
| `docs/analysis/AGENT-CANVAS-RENDERING-ANALYSIS.md` | 命题与缺口的来源；本规格在 §1.3 记录了 9 处有意校正 |
| `docs/analysis/ORCHESTRATION-CONCEPT-SEMANTIC-TRANSFER.md` | 术语唯一出处；本规格不新增用户词 |
| `docs/store/I68-agent-canvas-preset-spec.md` | W2：真实内容映射 + 预设摆放 + 宿主接线；消费本规格全部设施，删除 §11 示例内容 |
| I55 Agent 轮次观测规格（已归档） | 真实内容拓扑与出口表的来源；权威仍是内核 Rust |
| `docs/DESIGN.md` §5 | 主题变量与 fallback 禁令 |
| 工作流画布规格（未立项） | 承接 OQ-6 的编辑模式；两者共用画布设施（视口 / 渲染器 / 失效调度），不共用内容视图模型 |
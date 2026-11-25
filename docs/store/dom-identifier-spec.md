# DOM 元素标识规范

| 项 | 值 |
|---|---|
| 状态 | 现行（§7 的 G2–G7 尚未接入门禁，见 §9 T1） |
| 日期 | 2026-11-25 |
| 适用 | `src/**/*.{ts,tsx}`、`index.html` 中写入 DOM 的 `id` 与 `data-*` 属性，以及按标识查找元素的代码；`src/**/*.module.css`、`src/assets/styles/*.css` 对这些属性的消费 |
| 依据 | [CODE-REVIEW-PRINCIPLE](../principle/CODE-REVIEW-PRINCIPLE.md) D5（一致性与可读性，合规判据唯一来源）、D15（平台兼容与国际化，含可访问性）、D11（安全与隐私，含注入与明文暴露）；[CODE-SPLIT-PRINCIPLE](../principle/CODE-SPLIT-PRINCIPLE.md)（组件职责）；[doc-naming-spec](./doc-naming-spec.md)（本文文件名）；[code-comment-spec](./code-comment-spec.md)（注释口径，本文不重复） |
| 关联 | [I69c §2.1 / §2.3 / §3.5 / §3.6](./I69c-agent-orchestration-canvas-spec.md)（画布标识的消费面）、[I69d §6](./I69d-agent-orchestration-card-spec.md)（INC2 / INC3 的设施增量，标识口径以本文为准）、[I48 §6.4](./I48-chat-region-layout-and-scroll-spec.md)（DOM 结构约束） |

> 一句话：DOM 标识只回答三件事——**这是谁、是哪类元素、处于什么状态**。身份用 `data-<域>-id`，标记用裸属性，状态用 `data-<状态>="true|false"` 或封闭枚举值；`id=` 只留给 ARIA 引用与根挂载点；禁止用类名查找元素。

本文中的「必须 / 禁止」为可验收约束；「应当」为默认做法，偏离需在被标识处的同一段落写明理由。

**编号约定**：`N1–N19` 现状事实、`C1–C21` 条款、`G1–G7` 门禁、`V1–V15` 验收判据、`T1–T7` 实施切片、`X1–X6` 拒绝方案、`OQ1–OQ4` 决议记录（原开放问题，均已定）。编号按引入顺序追加，新增条款不回填空号、不重排既有编号，以保证跨文档引用稳定。跨文档引用一律带文档前缀（`CODE-REVIEW-PRINCIPLE D5`、`I69c §3.5`），裸写 `§` 一律指本文。`D*`（审查维度）沿用 CODE-REVIEW-PRINCIPLE 定义，本文不重复定义。

---

## 1 定位与边界

### 1.1 三种用途

DOM 上的标识按**回答的问题**分三类，编码形态不同（C1–C3）。一个属性只能属于一类，不得兼作两类。

| 用途 | 回答什么 | 编码 | 消费者 | 现有实例 |
|---|---|---|---|---|
| 身份 | 「这是哪一个业务对象」 | `data-<域>-id="<业务 id>"`（非 id 的键用 `-key`） | `closest` / `querySelector` + `dataset`；`Map` / `Set` 键 | `data-card-id`、`data-edge-id`、`data-conv-id`、`data-message-id`、`data-section-id`、`data-row-key`、`data-setting-key` |
| 标记 | 「这是哪一类元素」 | 裸属性 `data-<域>-<对象>`（值为空串） | 结构定位（`closest("[data-x]")`）与结构样式 | `data-canvas-card`、`data-edge-handle`、`data-tool-panel`、`data-tool-panel-anchor`、`data-orb-window`、`data-screenshot-overlay`、`data-tauri-drag-region` |
| 状态 | 「当前处于什么状态」 | `data-<状态名>={布尔或封闭枚举值}` | CSS 状态样式；少量运行时读取 | `data-selected`、`data-tone`、`data-phase`、`data-state`、`data-kind`、`data-run`、`data-wide`、`data-empty`、`data-panning`、`data-modality`、`data-frame`、`data-streaming`、`data-role` |

### 1.2 与其它载体的分工

| 载体 | 回答什么 | 不回答什么 |
|---|---|---|
| CSS 类名 | 样式归属 | 元素身份与状态；类名是实现细节，禁止作为查找钩子（C5） |
| `data-*` | 身份、类别、状态 | 业务数据、几何测量、敏感信息（C13） |
| `id` | 文档级唯一引用（ARIA idref、根挂载点） | 业务定位（C4 / X1） |
| `ref` | 同一组件内对自身子树的直接引用 | 跨组件、跨层级查找（后者用标识） |
| 源码正则测试 | 标记形态的回归断言 | 运行时行为（仓库无 jsdom，见 N12） |

### 1.3 纳入与排除

**纳入**：属性选择、命名、取值、写入方式、查找方式、改名联动、门禁与验收。

**排除**：

1. 标识合规的**判据**——唯一来源是 CODE-REVIEW-PRINCIPLE D5 / D15，本文只给落点与操作细则；
2. 组件结构与拆分——见 [CODE-SPLIT-PRINCIPLE](../principle/CODE-SPLIT-PRINCIPLE.md)；
3. 可访问性语义本身（角色、焦点管理、键盘序列）——本文只管「ARIA 引用用什么承载」；
4. 样式组织与 CSS 命名——类名怎么写不在本文范围；
5. 文案与 i18n。

---

## 2 现状事实（以代码为准）

| 编号 | 事实 |
|---|---|
| N1 | 业务代码不用 `id` 定位元素：根挂载点 `#root` 只在 `index.html` 定义、由 `src/main.tsx` 与 `src/services/system/globalErrorHandler.ts` 读取；其余 `id` 全部是 ARIA idref 目标（N2） |
| N2 | 3 处 ARIA idref 目标：`src/history/sidebar/HistorySidebar.tsx` 的 `id="history-sidebar"`（配对 `HistorySidebarToggle.tsx` 的 `aria-controls`）；`src/settings/pages/storage/EntryDetail.tsx` 与 `ResidualTable.tsx` 的详情容器 id（分别配对 `EntryTable.tsx` / `ResidualTable.tsx` 的 `aria-controls`） |
| N3 | 两处动态 id 经字符白名单净化：`detailDomId` / `residualChildrenDomId` 把非 `[a-zA-Z0-9_-]` 替换为 `_`（`EntryTable.tsx` / `ResidualTable.tsx`） |
| N4 | 三种用途并存（见 §1.1），但**身份命名不统一**：`-id`（`card` / `edge` / `conv` / `message` / `section`）与 `-key`（`row` / `setting`）混用 |
| N5 | **状态编码混用**：布尔状态有 `={flag}`（恒渲染 `"true"/"false"`）、省略式 `data-streaming={flag \|\| undefined}`（`src/chat/message/MessageContent.tsx`，即「缺省 = false」）、反向布尔字面量 `? "false" : "true"`（`GraphCanvas.tsx` 的 `data-entry`）、硬编码 `data-wide="true"`（`VaultApp.tsx`）四种写法 |
| N6 | 同类对象标记不对称：卡片 = 裸标记 + 身份（`CanvasCard.tsx` 的 `data-canvas-card` + `data-card-id`），边 = 仅身份（`CanvasEdge.tsx` 的 `data-edge-id` 挂在内部命中区，根节点只有 `data-phase` / `data-selected`） |
| N7 | 画布标签（`CanvasLabel.tsx`）只有 `data-tone` / `data-frame`，**无任何身份属性**；而其视图模型 `CanvasLabelView.id` 存在（`src/features/canvas/types.ts`），且 `I69c §2.3` 第 4 条 / §3.6 要求按 `groupId` 定位到分组标签。该定位经视图模型（`id` + `box` + `tone`）完成、不读 DOM，故 `data-label-id` 不是 `I69c` 的前置，只在新增 DOM 反查点时必需（T3） |
| N8 | 放置栏条目（`src/features/canvas/CanvasPalette.tsx`）无 `data-kind`，类别只存在于拖拽 MIME 与 `aria-label` |
| N9 | **类名查找 3 处**：`SettingsGroup.tsx` 的 `closest(".settings-page-shell")`、`OrbExpand.ts` 的 `querySelector(".orb-panel")`（均为自有全局类，本可改 `data-*` 标记）；`useToolRegionState.ts` 的 `.tool-panel-dropdown`（经 antd `classNames` 注入、无对应样式定义，纯查找钩子；第三方弹层渲染在 portal 下、无法注入 `data-*`，是唯一有正当理由的例外） |
| N10 | **属性选择器插值无转义 2 处**：`HistorySidebar.tsx` 的 `[data-conv-id="${id}"]`、`SettingsGroup.tsx` 的 `[data-section-id="${id}"]` / `[data-setting-key="${key}"]` |
| N11 | **`dataset` 命令式改写与 JSX 静态值并存 1 处**：`src/mascot/renderers/gif/GifMascot.tsx` 直接赋 `img.dataset.mode`，同元素 JSX 又静态写 `data-mode="play"` |
| N12 | 测试环境为 node、无 jsdom / testing-library；现有 DOM 相关断言按**源码正则**比对标记文本（`src/features/__tests__/orchestrationBoundary.test.ts` 断言 `data-tone={...}` / `data-frame={...}` 的写法）——`data-*` 事实上是测试契约 |
| N13 | 门禁现状：`scripts/check.mjs` PHASE1 为格式 / 类型 / 规则 / 吞没 / 文案，PHASE2 为契约；`eslint.config.ts` 无任何 DOM 标识规则 |
| N14 | 无 `data-testid`，也无组件测试基建（`src/chat/tool/__tests__/panelStructure.test.ts` 明确不为此引入） |
| N15 | 跨域同名不同值的状态属性：`data-tone` 在 canvas 取 `CardTone`（`default` / `active` / `success` / `warning` / `danger` / `muted`，`src/features/canvas/types.ts`），在 observation 取出口色调 `ExitTone`（`done` / `stopped` / `cancelled` / `failed`，`GraphCanvas.tsx`）；各自 CSS 独立消费，互不共享 |
| N16 | 环境级属性 `data-theme` 由 `src/main.tsx` 在 `documentElement` 上命令式写入，供全局 CSS 与少量模块样式选择；与组件内 `data-*` 不同层 |
| N17 | **生成型列表的逐项身份缺位**：聊天消息在主视图只有 `data-role`（`MessageBubble.tsx`，无 CSS 消费）与流式内容的 `data-streaming` 布尔标记，无 `data-message-id`，而同一对象在历史详情按行带 `data-message-id`（`HistoryBranchRow.tsx`）；观察实例列表只有 `data-selected`，无实例身份（`ObservationPage.tsx`）。两处的选择与滚动都走 React state / ref，目前无按标识寻址的调用点 |
| N18 | **死标识 4 处**：`CanvasCard.tsx` 的 `data-run`（`I69d §6 INC2` 引入，该卷已同步改为不新增）、`MessageBubble.tsx` 的 `data-role`、`HistoryBranchRow.tsx` 的 `data-message-id` 在全仓无任何消费者（无 CSS 选择器、无 `dataset` 读取、无测试正则）；`CanvasEdge.tsx` 的 `data-edge-id` 只被 `CanvasViewport.tsx` 的 `closest("[data-edge-id], [data-edge-handle]")` 作**存在性判断**命中，其值从未被读取——实际起标记作用而非身份作用 |
| N19 | `data-orientation` 的唯一消费者是 `canvas.module.css` 的矩形段样式与两个方向 keyframes（虚线 `repeating-linear-gradient`、流动 `background-position`）；`CanvasEdge.tsx` 按段派生、`CanvasLegend.tsx` 的图例小样硬写 `"horizontal"` 复用同一组选择器。边渲染原语换成 SVG path 后（`I69c §2.1` M6：虚线改 `stroke-dasharray`、流动改 `stroke-dashoffset`），这些选择器整体删除，该属性在渲染处与消费处同时失去语义——**不是改口径保留，而是随 M6 同批删除**；图例小样须自带样式（独立 `stroke-dasharray` 或自绘 `<svg>`），不得再借画布侧选择器 |

---

## 3 属性选择

**C1 身份属性**：只用于「按业务对象定位元素」，值必须是该对象的业务 id（后端权威或前端分配），命名 `data-<域>-id`；值不是业务 id 而是列表内复合键时用 `data-<域>-key`（C9）。身份属性在同一父容器内必须唯一（唯一域的界定见 V9）。

**C2 标记属性**：只用于「按元素种类定位结构」，写成裸属性（`data-<域>-<对象>`），值为空串。标记不承载任何业务值；需要携带值即说明它是身份或状态。

**C3 状态属性**：值取自封闭枚举或布尔（C10），只供 CSS 与少量运行时读取；不得作为业务分支的唯一判据（业务判定读 store / props，X5）。需要携带值的**分类**（如 `data-kind="loop"`）按状态属性的封闭枚举处理——裸标记（C2）不带值。

**C4 `id` 只有两种合法用途**：

1. **根挂载点**：`index.html` 的 `#root` 与 `main.tsx` / `globalErrorHandler.ts` 的读取，新增根节点需在 G2 白名单登记；
2. **ARIA idref**：`aria-controls` / `aria-labelledby` / `aria-describedby` / `aria-activedescendant` / `aria-owns` 的指向目标，以及 `<label for>`。这些引用按规范必须用 `id`，不能用 `data-*` 替代。

用 `id` 作 idref 目标时：动态部分必须经字符白名单净化（沿用 N3 的 `replace(/[^a-zA-Z0-9_-]/g, "_")` 形态），且必须保证同一文档内唯一；用户可编辑字符串不得直接成为 `id`。

**C5 禁止类名查找**：`querySelector` / `querySelectorAll` / `closest` / `matches` 的参数不得以类名选择器开头。CSS Modules 类名经哈希、全局类名属样式实现细节，改名即静默失效。唯一例外是第三方组件渲染到 portal 的子树（如 antd 弹层）：`data-*` 无法注入，只能经其 `classNames` 传自有类名，且必须在调用处写明理由并登记进 G3 白名单。

**C6 声明式写入**：`data-*` 只由 JSX / 模板声明，随 props 与 state 重渲染。禁止 `element.dataset.x = …` 或 `setAttribute("data-x", …)` 在组件内改写；需要运行时切换时，把它提升为 state 或 props。环境级属性（`documentElement` 上的 `data-theme`，N16）是唯一例外，需在写入点注明「环境级、非组件态」。

**C18 标识写在语义根**：身份与标记写在承载该对象的语义根元素，不得为命中测试的方便把身份下沉到内部节点；内部命中区只放标记（如 `data-edge-handle`）。理由是外部查找只做向上 `closest`，身份一旦下沉到内部节点，根节点的使用者就拿不到身份，且同一对象会出现「根有标记、内部有身份」的分裂形态（N6 / N18）。

---

## 4 命名与取值

**C7 域前缀**：`data-<域>-…` 的域段与该特性目录名一致（`canvas` / `chat` / `history` / `settings` / `orb` / `screenshot` / `vault` / `observation`）。通用状态（`selected` / `tone` / `phase` / `disabled`）可省略域段，但同名属性必须在该域内有唯一含义；跨域同名而语义不同时应当加域前缀。现有 `data-tone` 的跨域同名经 OQ1 决议为**保留**——成立前提是 C20（属性选择器一律类名限定），作用域由模块类名界定。

**C8 身份值不透明**：身份属性值必须是不可解析的标识——不得把类别、端点、时间戳、序号拼进值（如 `turn#1`、`from->to-<ms>`），也不得由前端解析 id 的组成部分。语义一律来自独立属性或业务对象本身。

**C9 `-id` 与 `-key` 分工**：值是业务对象 id → `-id`；值是「列表 / 表格内的行键」（复合键或非业务 id）→ `-key`。同一域内表达同一概念不得两种混用。

**C10 状态取值**：

1. 枚举状态（相位、色调、模态）取封闭词表，词表由 TS 联合类型约束，前端不对未知值做窄化兜底以外的改写；
2. 布尔状态默认恒渲染 `"true" | "false"`（便于 `[data-x="false"]` 反向选择）；「缺省即 false」的省略式（`flag || undefined`）只允许存量白名单，新代码不得扩大；
3. 属性缺失与 `"false"` 在语义上不得混用：若两者都表示「没有」，只保留一种编码。

**C11 禁止反向布尔字面量**：不得写 `x ? "false" : "true"`。先算布尔、命名取反（`const isEntry = …`），再直接绑定。

**C12 选择器插值必须转义**：把动态值拼进属性选择器时，值必须经 `CSS.escape()`；查询参数由模板字面量构造时，`${…}` 两侧不得裸拼。

**C13 值的内容边界**：`data-*` 值不得含敏感信息（令牌、密钥、明文凭据）、本机绝对路径、用户名与机器名；也不得放几何测量与派生数据（宽高、缩放、锚点坐标）。这些属于视图内部状态，留在组件 state（与 `I69c §2` S3「位置是意图（模板 `layout`），尺寸与端口锚点是测量（渲染侧）」同向）。

**C19 身份值不得取渲染序数**：身份值必须来自业务 id 或稳定键，不得取渲染序号 / 数组下标。分页前插、过滤、虚拟化都会让序号漂移——历史详情的 `LOAD_MORE` 前插与虚拟列表滚动正是这类场景，用序号作身份会在前插后整体错位。React 的 `key={i}` 是协调用的 key，不属 DOM 标识，不受此约束；一旦为内容块新增 DOM 身份，必须换成稳定 id。

---

## 5 消费纪律

**C14 查找结果只取身份值**：`closest` / `querySelector` 命中后只读取身份属性（`dataset.<x>Id` / `<x>Key`），再回到业务 Map / Set 定位；不得依据类名、文本内容或 DOM 结构做二次判定。

**C15 改名同批联动**：身份属性与标记属性改名时，必须在同一变更内更新全部消费点——CSS 选择器、源码正则测试、`closest` / `querySelector` 查找点、文档锚点。任一遗漏即为 D5 缺陷。

**C16 属性名与 prop 名不错位**：组件 prop 名不得与它最终落到的 DOM 属性名冲突性错位（如 `SettingsSection` 的 `id` prop 实际渲染为 `data-section-id`）；新组件用 `sectionId` / `domKey` 一类名字，避免读者误判为 DOM `id`（存量见 OQ2）。

**C17 生成型列表逐项可寻址**：会被按标识寻址的生成型列表——流式消息、画布节点与标签、虚拟列表行、观察实例——每个可寻址项必须带身份属性（C1）；仅有类别或状态属性不足以定位单项。并发存在的对象尤其如此：`data-selected` 这类布尔状态在多个并发对象上彼此不可区分，必须与 `data-card-id` / `data-observation-key` 一类身份属性并存（`CanvasCard.tsx` 的 `data-card-id` 为正例）。单实例元素（会话工具条 `data-tool-panel-anchor`、滚动锚点 `data-approval-rules`）不受此约束。尚未出现寻址调用点的列表可暂缓补身份，但一旦新增按标识寻址的调用点，必须同批补齐（存量见 N17 / OQ4，结论为补）。

**C20 属性选择器必须类名限定**：组件态的属性选择器必须与模块类名组合（`.cls[data-x]`），禁止裸 `[data-x]` 作用于全局；`data-theme` 这类写在 `documentElement` 上的环境属性例外（它必须作为祖先选择器裸用）。这条是 C7「跨域同名可省略域前缀」成立的前提：作用域由类名界定，同名属性不会跨模块互相命中（现状所有组件态选择器都已限定，仅 `data-theme` 为裸祖先选择）。

**C21 每个 `data-*` 必须有消费者**：渲染出的属性必须至少有一处消费——CSS 选择器、`closest` / `querySelector`、`dataset` 读取、源码正则测试断言（N12 的测试契约），或外部契约（Tauri 的 `data-tauri-drag-region`、antd 经 `classNames` 注入的类名）。无消费者的死标识一律删除（N18）：它会误导后来者以为「已有标识可用」，且改名时无人发现。新增属性与新增消费者必须同批提交——「先埋标识、后接消费者」的纯预留同样属于死标识。

---

## 6 禁止事项

| 形态 | 判据 | 依据 |
|---|---|---|
| 业务元素用 `id` 定位 | 多实例 / 重复 id 静默冲突，重挂载后 id 复用 | C4 / X1 |
| 用类名查找元素 | CSS Modules 哈希不可预测，改名静默失效 | C5 / N9 |
| 属性选择器裸拼动态值 | 值含引号或空白时选择器失效或命中错误元素 | C12 / N10 |
| 组件内命令式写 `dataset` | 与声明式渲染竞争，静态值靠「恰好不变」维持 | C6 / N11 |
| 反向布尔三元 | 可读性缺陷，极易写反 | C11 / N5 |
| 身份值编码类别 / 端点 / 时间戳 | 语义不稳定，类别扩展即失效 | C8 / V8 |
| 同一状态两种编码并存 | `="false"` 与缺失语义混淆 | C10 / N5 |
| 生成型列表只给状态属性、不给逐项身份 | 多项并发时无法定位单项（`data-selected` 单用即属此形态，N17） | C17 / N17 |
| 身份下沉到内部命中节点 | 根节点拿不到身份，同一对象出现标记 / 身份分裂 | C18 / N6 / N18 |
| 身份值取渲染序数 | 前插分页与虚拟化后整体错位 | C19 |
| 裸 `[data-x]` 全局属性选择器 | 跨模块同名属性互相命中 | C20 / N15 |
| 无消费者的死标识 | 误导后来者，改名无人发现 | C21 / N18 |
| `data-*` 放敏感信息、本机路径、几何测量 | 截图、调试与日志可见面 | C13 |
| 以 `data-*` 为业务分支唯一判据 | 业务真相在 store / 后端，DOM 是投影 | C3 / X5 |
| 引入 `data-testid` 作为第二套钩子 | 无组件测试基建，形成双命名 | X2 / N14 |
| 未登记就扩大白名单 | 例外不得扩大 | C5 / G3 |

---

## 7 门禁

**G1 只做形态扫描**：门禁只检查可机读形态（`id=` 白名单、类名查找、未转义插值、`dataset` 赋值、反向布尔字面量、死标识），语义判断交给 D5 / D15 审查。任何「理解标识含义」的检查都会产生高误报，按 X6 拒绝。

**G2 `id=` 白名单扫描**：新增 `scripts/check-dom-identifier.mjs`，扫描 `src/**/*.tsx` 的 `id=` / `id={`，按标签首字母区分两类：**宿主元素**（小写标签）上的 `id` 仅放行「同文件存在对应 ARIA idref」或 `WHITELIST` 登记项；**组件 prop 名为 `id`** 的调用点（现有 `SettingsSection` 8 处、`EntryDetail` 1 处）先登记为例外，改名后（OQ2 / T4）再收紧。`index.html` 的 `#root` 单独允许。

**G3 类名查找扫描**：`querySelector(All)` / `closest` / `matches` 的参数若以 `.` 开头即失败；第三方库例外按 `WHITELIST` 登记理由（仿 `scripts/check-error-swallow.mjs`），`.tool-panel-dropdown` 首条入册，理由须写明「弹层由第三方渲染到 portal，只能经 `classNames` 注入类名」。

**G4 未转义插值扫描**：查询参数为模板字面量、含 `${` 且未包 `CSS.escape(` 即失败；存量 2 处（N10）须在 T2 先修。

**G5 `dataset` 赋值扫描**：`.dataset.<x> =` 与 `dataset[` 赋值即失败；`documentElement.setAttribute` 白名单（N16）。

**G6 反向布尔字面量扫描**：`? "false" : "true"` 形态即失败（N5 的 1 处）。

**G7 死标识扫描**：统计 `src/**/*.tsx` 中渲染出的每个 `data-*` 属性名，与全仓其它引用（CSS 属性选择器、`dataset.*` 读取、选择器字符串）比对，仅在渲染处出现一次即失败；外部契约属性与 `data-theme` 按 `WHITELIST` 登记（`data-tauri-drag-region` 由 Tauri 原生消费，antd 弹层类名例外见 G3）。存量 4 处（N18）须在 T6 先清；`data-orientation`（N19）在 `I69c §2.1` M6 落地前仍由矩形段 CSS 消费、不算死标识，M6 落地后必须随渲染处一并删除，G7 不得残留。

G2–G7 全部落在同一个脚本内，接入 `scripts/check.mjs` PHASE1（新增 `dom-id` 步骤，与「吞没」「文案」并列）。

---

## 8 验收判据

| 编号 | 判据 | 检查方式 |
|---|---|---|
| V1 | 每个新增标识能归入身份 / 标记 / 状态之一，且编码与用途一致 | 审查 |
| V2 | 业务代码无 `id=`，`id` 只出现在根挂载点与 ARIA idref | G2 |
| V3 | 无类名查找（第三方例外已注释并登记） | G3 |
| V4 | 属性选择器中的动态值均经 `CSS.escape` | G4 |
| V5 | 组件内无 `dataset` 命令式改写 | G5 |
| V6 | 无 `x ? "false" : "true"` 反向字面量 | G6 |
| V7 | 布尔状态编码在同域内一致；省略式只在白名单 | 审查（C10） |
| V8 | 身份值不透明：不含类别、端点、时间戳，前端不解析 | 审查（C8） |
| V9 | 身份唯一性成立：节点 id 全模板唯一、边 id 仅边集唯一（`I69c §2.1 / §3.5`），同一父容器内不出现重复身份值 | 审查 + 对应域测试 |
| V10 | `data-*` 无敏感信息、本机路径、几何测量值 | 审查（C13） |
| V11 | 身份 / 标记改名与 CSS、源码正则测试、查找点同批更新 | 审查（C15 / D5） |
| V12 | 生成型列表的每个可寻址项都有身份属性；布尔状态属性不与身份属性孤立使用 | 审查（C17） |
| V13 | 每个渲染出的 `data-*` 至少有一处消费者（外部契约已登记） | G7 |
| V14 | 身份与标记写在语义根，内部命中区只用标记 | 审查（C18 / N18） |
| V15 | 身份值不取渲染序数 | 审查（C19） |

---

## 9 实施切片

| 编号 | 动作 | 落点 | 退出条件 |
|---|---|---|---|
| T1 | 新增 `scripts/check-dom-identifier.mjs`（G2–G7 合一，带 `WHITELIST`）并接入 `scripts/check.mjs` PHASE1 | `scripts/` | 存量例外全部登记，`npm run check` 通过 |
| T2 | 存量形态整改：自有类名查找 2 处改 `data-*` 标记；插值 2 处加 `CSS.escape`；`GifMascot` 去 `dataset` 改写；`GraphCanvas` 反向三元；`VaultApp` 的 `data-wide="true"` 改按 `isWide` 绑定 | `SettingsGroup.tsx`、`OrbExpand.ts`、`HistorySidebar.tsx`、`GifMascot.tsx`、`GraphCanvas.tsx`、`VaultApp.tsx` | G3–G6 全绿 |
| T3 | 画布标识补齐（**W3 内实施项，不是 W3 前置**；只在新增按标识反查的调用点时实施）：`CanvasLabel` 加 `data-label-id={label.id}`；边根与卡片统一「标记 + 身份」形态（边根补裸标记，`data-edge-id` 由内部命中区上移到根，命中区只留 `data-edge-handle`）；放置栏条目加 `data-kind` | `src/features/canvas/` | 新增反查点满足 C17 / C18；`I69c §3.6` 的定位不依赖本切片（走视图模型，见 N7）；V9 在画布域成立 |
| T4 | 命名收敛：`data-approval-rules`（滚动锚点，非身份）改为 `data-anchor="approval-rules"`，同步 `agentMode/index.tsx` 的查找点；`-id` / `-key` 按 C9 复核 | `ToolAssemblySection.tsx`、`agentMode/index.tsx` | 命名无歧义，查找点同批更新 |
| T5 | 画布选择槽拆 `{kind, id}`（节点 id 与边 id 分属两个唯一域，现共用单槽） | `src/features/canvas/CanvasViewport.tsx` | 属 W3 设计，挂编排任务台账，不在本规范验收 |
| T6 | 死标识清理与生成型列表身份补齐：删 `data-run` / `data-role` / 历史详情未被读取的 `data-message-id`；`MessageBubble` 根加 `data-message-id={message.id}`，观察实例按钮加 `data-observation-key={key}`（复合键按 C9 用 `-key`），并同批补源码正则断言作为消费者（C21）。`data-orientation` 不在此切片单独删——它随 `I69c §2.1` M6 的矩形段 → SVG path 改造同批消失（`CanvasEdge.tsx` 渲染处、`canvas.module.css` 选择器与 keyframes、`CanvasLegend.tsx` 图例小样一处，见 N19），M6 落地后 G7 不得残留 | `CanvasCard.tsx`、`MessageBubble.tsx`、`HistoryBranchRow.tsx`、`ObservationPage.tsx`、`CanvasEdge.tsx`、`CanvasLegend.tsx`、`canvas.module.css`、对应 `__tests__` | G7 全绿；聊天 / 观察与历史详情的身份口径一致；`data-orientation` 无残留 |
| T7 | 动态 id 净化收敛：`detailDomId` / `residualChildrenDomId` 两个同形函数合并为共享工具，净化规则单点定义 | `src/settings/pages/storage/` | 无重复净化实现 |

---

## 10 拒绝方案

| 编号 | 方案 | 拒绝理由 |
|---|---|---|
| X1 | 用 `id` 做业务定位（`#card-…` / `#edge-…`） | 同页多实例即重复 id；重挂载后 id 复用；与 `id` 的「文档级唯一」语义冲突 |
| X2 | 引入 `data-testid` 作为测试钩子 | 无 jsdom 与 testing-library，测试靠源码正则；新增钩子会形成第二套命名并与身份属性重复 |
| X3 | 继续用类名查找 | CSS Modules 哈希不可预测、全局类改名即静默失效；已出现的 2 处自有类查找正是这类隐患 |
| X4 | 所有状态统一写裸属性 | 无法区分「false」与「缺省」，`[data-x="false"]` 反向选择失效 |
| X5 | 以 `data-*` 作为业务分支判据 | DOM 是投影，业务真相在 store / 后端；读 DOM 做判定会把渲染细节变成契约 |
| X6 | 门禁做语义检查（判断标识「是否合理」） | 不可机检、误报高；语义判断属 D5 / D15 审查 |
| X7 | 先埋标识、后接消费者（纯预留） | 预留期内的属性无任何验证路径，改名与删除都无人察觉；要预留就同批写出消费者（查找点或测试断言，C21） |

---

## 11 决议记录

原开放问题的最终结论如下；新增疑问先在本节标注「待定」，定稿后转为条款（C\*）或本节决议，编号不复用。

| 编号 | 结论 | 理由 | 落点 |
|---|---|---|---|
| OQ1 | `data-tone` 跨域同名**保留**，不加域前缀 | 所有组件态属性选择器都带模块类名（C20），跨模块不会互相命中；两个域的 `data-tone` 也不会落在同一元素上（canvas 卡片 vs observation 出口 / 边） | C7 + C20 |
| OQ2 | `SettingsSection` 的 `id` prop **改名**为 `sectionId` | prop 名叫 `id` 会让人以为在写 DOM id，实际渲染 `data-section-id`，属 C16 错位；8 处调用为机械替换，风险低 | T4 |
| OQ3 | `-id` / `-key` **保留分工**，不合并 | `-key` 是列表行键（复合键或非业务 id，如 `conversationId + turnId`、`row.name`），与业务对象 id 不同域；合并会让人误以为可跨列表寻址 | C9 |
| OQ4 | 聊天 / 观察**补身份**（`data-message-id` / `data-observation-key`），`data-role` **删除** | 与历史详情的 `data-message-id` 口径分裂；补身份的边际成本是一条属性加一条断言，缺了则一旦出现寻址需求就要回头改渲染层与测试；`data-role` 与 `css.rootRole*` 重复且无消费者 | T6 + C21 |
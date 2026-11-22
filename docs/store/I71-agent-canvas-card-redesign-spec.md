# I71 Agent 画布卡片构造重构规格（W2 修订）

| 项 | 值 |
|---|---|
| 状态 | 现行（**卡片构造、分区预算、视觉构造的唯一来源**；取代 I68-agent-canvas-preset-spec（已归档）§7 / §8 / §11 的相应条款）。**§3 尺寸、§4.3 阶段轨道、§4.4 端口、§4.5 脚部高度已被 [I72](./I72-agent-canvas-visual-and-stage-merge-spec.md) 修订，以 I72 为准**；其余条款（头部实色身份带、双色调通道、主体去内层容器、选择态 / 焦点态分离、分层纪律）继续有效 |
| 日期 | 2026-11-22 |
| 前置 | I66（已归档，画布设施 W1：视口 / 几何纯函数 / 分层纪律）、I68（已归档，画布预设 W2：预设实体 / 摆放 / 放置栏 / 示例标注） |
| 参考 | `references/9-workflow-orchestration/litegraph.js/src/litegraph.js`（同类画布节点实现：标题条、槽位端口、选择描边、连线形态） |
| 视觉约束 | `docs/DESIGN.md` §5（antd CSS 变量镜像、禁止硬编码色值与 fallback） |
| 影响范围 | `src/features/canvas/presets/metrics.ts`（`CARD_METRICS` 取值）；`src/features/canvas/canvas.module.css`（卡片全套分区样式）；`src/features/canvas/CanvasCard.tsx`（布局结构重排）；`src/features/canvas/types.ts`（`CanvasCardView` 增类别色通道）；`src/features/observation/canvasContent.ts`（类别色 / 状态色分通道投影）；`src/features/canvas/geometry.ts`（端口槽位锚点）；`src/features/canvas/presets/cardKinds.ts`（色调字段拆分为类别色）；`src/features/canvas/__tests__/preset.test.ts`（尺寸与坐标断言）；`src/i18n/locales/{zh-CN,en-US}.json`（新增状态药丸与空态键） |
| 契约影响 | 无（前端编译期常量 + 本地会话状态；不触碰 wire、不触发 `npm run contract:gen`） |

> **为什么要重做**：现有卡片严格按 I68 §7.4 的预算画（`32 + (66+1+115) + 26 = 240`），但六件构成里每一件都只做到「在场」，没有一件做到「成立」。头部是浅灰底 + 浅灰徽标，整卡没有视觉重心；阶段条在 3×4 网格里被压到 9px 字 / 11px 行高，必然裁字；端口只有一个垂直居中的锚点，连线只能从正中间接出；选择态与焦点态用同一圈 2px 实线，用户无法区分「我选中了它」和「键盘焦点在它身上」。结果是：**结构合规，但读起来不对劲**。
>
> 本文不改 W1 的任何设施约束（零测量、纯函数、分层纪律、示例标注），只重定义**卡片这一个东西长什么样**。

本文中「必须 / 禁止」为可验收约束；「应当」为默认做法，偏离需在代码注释里写明理由。

---

## 1 范围

### 1.1 交付什么

| # | 目标 | 判据 |
|---|---|---|
| 1 | 卡片尺寸与分区预算重算：每个分区都有**最低可读预算**，阶段名字不裁字 | A1 / A2 |
| 2 | 头部成为实色身份带（identity band）：整卡唯一的实色重心，名称反白 | A3 / A4 |
| 3 | 色调拆成**两条通道**：类别色（卡是谁）+ 状态色（卡现在怎样），互不覆盖 | A5 / A6 |
| 4 | 阶段轨道改为**横排两列式**阶段条：序号 + 名称 + 状态文案，一行读完 | A7 / A8 |
| 5 | 端口改为**槽位式**：按类别声明的槽位序号落在边框的确定位置，连线从槽位接出 | A9 / A10 / A11 |
| 6 | 选择态与焦点态**形态不同**：选择 = 实线外扩描边；焦点 = 虚线内缩环 | A12 |
| 7 | 卡片内部不再套娃：主体只有分隔线，没有内层描边容器 | A13 |
| 8 | 所有既有断言（示例标注、阶段与模板一一对应、六件构成在场、无硬编码色值）继续成立 | A14 / A15 / A16 |

### 1.2 不交付什么

| # | 排除项 | 理由 |
|---|---|---|
| 1 | 卡片尺寸可配置 / 多档尺寸 | 尺寸仍是编译期常量；运行时按内容缩放需测量（W1 N3） |
| 2 | 类型化端口与交接校验 | 卡片类别闭集未定案（I68 §2.4 / T-21）；端口仍是**占位** |
| 3 | 卡片折叠 / 展开（minimize / collapse） | 折叠需要尺寸二阶状态与几何重算；属编辑增强，不在本规格 |
| 4 | 卡片内嵌控件（参数输入 / 按钮） | 参数编辑属 L2（I68 §1.2 第 3 条） |
| 5 | 连线改贝塞尔曲线 | 现行连线是正交直线（`edgeGeometryOf` 只产水平 / 垂直段），改曲线会改命中区与端点手柄的全部几何，属独立议题（OQ-4） |
| 6 | 视觉回归截图 | 仓库未配置浏览器渲染基线 |
| 7 | 卡片阴影 / 渐变 / 主题外的新色值 | DESIGN §5.3 禁止硬编码色值；重心靠色块与层级，不靠阴影 |

### 1.3 与 I68 的关系

| I68 条款 | 本规格的处置 |
|---|---|
| §7.1 五张示例卡片 | 保持（类别、引用、参数、端口语义不变） |
| §7.2 阶段轨道 3 列 × 4 行 | **改**：横排两列式（§4.3） |
| §7.3 六件构成 | 保持（六件仍全部在场），但每件的构造重写（§4） |
| §7.4 尺寸与分区预算 `32 + (66+1+115) + 26 = 240` | **改**：`340 × 288`，预算见 §3 |
| §7.7 色调与状态 | **改**：拆双通道（§5） |
| §7.8 选择态 | **改**：选择态实线外扩、焦点态虚线内缩（§6） |
| §8.1 / §8.2 卡片外观与层级 | **改**：全套重写（§5 / §6） |
| §11 A8 / A9 / A18 / A19 / A22 | **改**：新增断言替换旧断言（§10） |
| §7.5 示例标注（A25）/ §7.6 视图模型 | **保持**，视图模型只增字段（§7） |

**单向关系**：本规格读 I68 的预设 / 摆放 / 放置栏模型，不改它们；I68 的卡片呈现条款被本文取代后，以本文为准。

---

## 2 现状诊断

对照 litegraph.js 的节点实现，六件构成逐一失守，根因是「分区按比例分配剩余空间」，而不是「先给每件最低可读预算」。

| # | 症状 | 根因 | 参考项目的做法 |
|---|---|---|---|
| P1 | 卡片没有视觉重心：头部浅灰条 `fill-quaternary`、徽标浅灰细边，整卡扁平 | 头部只是「一条带分隔线的区域」，没有色块 | 标题条是**实色块**，与节点竖体形成强对比；标题反白 |
| P2 | 阶段名裁字、英文名更裁 | 3 列 × 4 行 → 单格约 95×24，扣掉 padding 只剩 24px 宽的文字位，9px 字仍会省略 | 节点内容是「一行一个槽位」，行高充足 |
| P3 | 连线只能从卡片正中接出，两条同侧连线重合 | `anchorOf(box, side)` 只给一个中心点 | 端口按**行槽位**分布，第 N 个槽位有确定 y |
| P4 | 无法区分「选中」与「焦点」 | 两个状态共用 `outline: 2px solid` + `offset: 2px` | 选择态是**外扩描边**；焦点是输入法层面的提示，形态不同 |
| P5 | 边框色同时表示「类别」与「运行状态」，语义打架 | `tone` 一个字段同时驱动 `.card[data-tone]` 边框与药丸 | 节点类型与运行状态分属不同视觉通道 |
| P6 | 主体里 `.cardExtra` 带描边 + 底色，套在 `.cardBody` 里 → 双层容器 | 附加区被当成「小卡片」 | 节点主体无内层容器，内容直接落在节点体上 |
| P7 | 阶段状态点只有 4px 圆点 + 文案挤在同一行，读不出状态 | 阶段条承担了三件事（序号 / 名称 / 状态） | 状态是独立通道，有文字标签 |

---

## 3 尺寸与分区预算

### 3.1 卡片尺寸

`CARD_METRICS` 改为 `340 × 288`（宽 +20、高 +48），`gap` 保持 32：

```ts
export const CARD_METRICS = {
  cellW: 340,
  cellH: 288,
  gap: 32,
  loopGutter: 0,
} as const satisfies CanvasMetrics;
```

宽取 340 的理由：阶段条两列时单列宽 `(340 - 2*10 - 8) / 2 = 156`，扣掉条内 padding 后文字位约 130px，11px 字可容纳 11 个汉字（最长的阶段名「上下文收束」6 字，英文「Context wrap-up」13 字符约 95px），不裁字。

高取 288 的理由：`36(头部) + 8 + 8(主体上下内边距) + 54(参数摘要 3×18) + 1(分隔线) + 8 + 140(附加区) + 8 + 1(分隔线) + 24(脚部) = 288`（详见 §3.2）。

### 3.2 纵向预算

| 区 | 高度 | 内边距 / 说明 |
|---|---|---|
| 头部 identity band | 36（含 1px 底分隔线） | `0 12px 0 26px`（左留拖拽柄位，右留「示例」徽标位） |
| 主体 · 上内边距 | 8 | `padding-top` |
| 主体 · 参数摘要 | 54 | 3 行 × 18px |
| 主体 · 分隔线 | 1 | `--ant-color-split` |
| 主体 · 附加区 | 140 | 阶段轨道（轨道头 18 + 8 + 3 行 × 36 + 2 × 4 = 140）或内容占位 |
| 主体 · 下内边距 | 8 | `padding-bottom` |
| 脚部 meta band | 24（含 1px 顶分隔线） | `0 12px` |

闭合算式：`36 + 8 + 54 + 1 + 8 + 140 + 8 + 1 + 24 = 280 ≤ 288`，余 8 为弹性余量（`flex: 1` 分配给附加区，保证附加区在内容不足时不塌陷、在多行时吸收富余）。

**改 `CARD_METRICS.cellH` 必须同步改本表并保证 `≤ cellH`**（A1）。

### 3.3 横向预算

| 项 | 值 |
|---|---|
| 卡片内容宽 | `340 - 2 × 1(border) = 338` |
| 主体左右内边距 | `10` + `10`（比旧版 12 收 2px，给阶段条让宽） |
| 主体内容宽 | `338 - 20 = 318` |
| 阶段条两列 | `(318 - 8) / 2 = 155` 每条 |
| 阶段条内 padding | `4px 6px`；文字位 `155 - 12 - 2(border) = 141` |

### 3.4 覆盖层（不占内部预算）

| 覆盖层 | 尺寸 | 定位 |
|---|---|---|
| 端口圆点 | 10×10 | 骑边框、凸出 5px；**纵向位置由槽位序号决定**（§4.4） |
| 端口类型标签 | 10px 字 | 卡外绝对定位，`pointer-events: none`，随卡片移动 |
| 拖拽柄 | 2×3 点阵（8×12） | `top: 12px; left: 8px`（头部带内） |
| 出口角标 | 14×14 | `right: 10px; bottom: 6px`（轮次卡片专用） |
| 选择描边 | 2px 实线，offset `3px` | 外扩到卡片边框之外 |
| 焦点环 | 2px 虚线，offset `-3px` | 内缩到卡片边框之内 |

覆盖层不参与内部预算、不改盒子宽高（A2）。改 `CARD_METRICS` 只需同步 §3.2 / §3.3 的算式。

---

## 4 六件构成（重写）

### 4.1 头部 identity band

| 项 | 规格 |
|---|---|
| 高度 | 36（含 1px 底分隔线） |
| 底色 | 类别色实色块（§5.1），**唯一允许的整块实色** |
| 文字色 | 类别色对应的反白前景（`--ant-color-bg-container` 或 `--ant-color-text-light-solid`，按对比度选） |
| 左端 | 拖拽柄点阵（8×12，`currentColor` 半透明） |
| 中段 | 类别徽标（类别 + 分型，如「执行卡片 · 轮次」）：无边框、`currentColor` 60% 透明度、10px |
| 主标题 | 卡片名称，13px / 600，单行省略，反白 |
| 右端 | 「示例」徽标：虚线边框 + `currentColor`，10px；不得用警示色块（避免与状态色通道抢色） |
| 圆角 | 上两角 `var(--radius-lg)` |

**为什么实色**：头部承载「这是什么卡」，是卡片在画布上的识别符；浅灰底在大量卡片并置时与主体几乎同色，读不出层级。实色块让卡片在缩到 60% 时仍能一眼分辨类别（A3）。

**对比度**：实色块上的文字必须满足 `≥ 4.5:1`；类别色取值见 §5.1，全部为 antd 语义色的 `*-bg` / 主色，已满足。

### 4.2 主体

主体无底色、无描边，只有三段纵向排列（参数摘要 / 1px 分隔线 / 附加区），外层 `padding: 8px 10px`。

| 段 | 规格 |
|---|---|
| 参数摘要 | 3 行，每行 18px：参数名 11px `--ant-color-text-tertiary`（定宽不换行，`flex: none`），示例值 11px `--ant-color-text` + `tabular-nums`，右对齐省略 |
| 分隔线 | 1px `--ant-color-split`，左右与内容同宽（不贯通到卡片边框） |
| 附加区 | `flex: 1 1 auto`，`min-height: 0`，无描边、无底色 |

**去掉 `.cardExtra` 的描边与底色**（P6）：附加区不是「一张小卡片」，它的边界由分隔线表达。省下的 2px 上下 padding 归还给阶段条行高。

### 4.3 阶段轨道

| 项 | 规格 |
|---|---|
| 轨道头 | 一行 18px：左「阶段轨道」（11px `--ant-color-text-tertiary`），右计数 `12`（11px `tabular-nums`） |
| 轨道头与网格间距 | 8px |
| 网格 | **2 列 × 6 行**，阅读顺序 左→右、上→下（与模板 `order` 一一对应，A8） |
| 列/行间距 | 列 8px，行 4px |
| 阶段条 | 高 36px，`padding: 4px 6px`，`border: 1px solid var(--ant-color-split)`，`border-radius: 3px`，白底 |
| 阶段条内容 | 第一行：序号（10px `--ant-color-text-quaternary`）+ 名称（11px / 500 `--ant-color-text`，单行省略）；第二行：状态点（6px）+ 状态文案（10px `--ant-color-text-tertiary`） |
| 阶段条状态 | 状态点取状态色（§5.2）；`active` 阶段条额外加 2px 左侧色条（`border-left: 2px` + 状态色） |
| 交互 | 不可聚焦、不可点击、不可拖拽；`aria-hidden`（`title` / `aria-description` 由卡片 `aria-label` 承载） |

**为什么 2 列而不是 3 列**：3 列时单格文字位不足 95px，11px 字的阶段名必然省略（P2）。2 列 × 6 行的纵向预算 `6 × 36 + 5 × 4 = 236` 超过附加区 140——**因此阶段条改为两行式 36px 高，网格取 2 列 × 6 行需 236px，与 §3.2 的 140 冲突**；采用最终方案见下。

**最终方案（满足 140 预算）**：网格 2 列 × 6 行，行高 `20px`、行距 `4px`，单条内容压成**单行**——序号（10px）+ 名称（11px，单行省略）+ 状态点（6px）三件同行，状态**不再出文案**，改由状态点的 `title` 与卡片 `aria-label` 表达。纵向 `6 × 20 + 5 × 4 = 140`，恰好闭合。

> 这一取舍是本规格的核心判断：**名字的可读性优先于状态文案的可见性**。状态文案在脚部药丸里已有全局交代（轮次卡片显示当前轮次状态），阶段级的「已开始 / 已完成」由状态点颜色 + `title` 承载；名字被裁掉则是信息直接丢失。

### 4.4 端口区（槽位式）

旧实现的端口锚点固定在边框中点（P3）。新实现按**槽位序号**定位：

| 项 | 规格 |
|---|---|
| 槽位定义 | `CanvasCardPortView` 增加 `slot: number`（同侧从 0 起，自上而下 / 自左而右）；同侧第 `n` 个槽位的纵向（或横向）位置为 `offset(n) = headerH + slotStep × (n + 0.5)`，其中 `slotStep = 28`（左侧 / 右侧），顶部 / 底部按 `columnStep = 60` 自左向右 |
| 约束 | 槽位总数上限：左 / 右各 4 个（第 4 槽中心 `36 + 28 × 3.5 = 134 ≤ 288`）；上 / 下各 5 个（`60 × 5 = 300 ≤ 338`，居中裁剪） |
| 越界处理 | 槽位数超出上限时按上限截断，多出的槽位落在最后一个槽位位置（不报错，示例数据不会触发） |
| 圆点 | 10×10，`border: 1px solid`，白底；hover / 选中连线时 `border-color: 状态色` |
| 类型标签 | 10px `--ant-color-text-quaternary`；左入标签在圆点右侧 4px，右出标签在圆点左侧 4px（`flex-direction: row-reverse`）；不得进入卡片内部 |
| 几何对接 | `anchorOf` 扩展为 `anchorOf(box, side, slot)`；`slot` 省略时退回中心点（保持既有调用兼容） |

```ts
export const PORT_METRICS = {
  headerH: 36,
  sideStep: 28,
  sideMax: 4,
  horizontalStep: 60,
  horizontalMax: 5,
  dotSize: 10,
} as const;
```

**为什么槽位式**：同侧两条连线（如轮次卡片同时接出 `turn.out` 与跨行的 `bottom`）在中心点重合，视觉上像一条线；槽位让每条连线有独立出口，也让「端口 = 一个明确的交接位置」成立（A10 / A11）。

### 4.5 脚部 meta band

| 项 | 规格 |
|---|---|
| 高度 | 24（含 1px 顶分隔线） |
| 底色 | `--ant-color-fill-quaternary`（比头部弱一档，不抢重心） |
| 内容 | 左起：状态药丸（18px 高，状态色底 + 状态色前景）+ 引用 chip（`canvas.card.reference` 前缀 + 原语量，`--ant-color-fill-secondary` 底，16px 高）；右端：时长等原语量（11px `tabular-nums`） |
| 圆角 | 下两角 `var(--radius-lg)` |

### 4.6 选择态与拖拽柄

| 件 | 规格 |
|---|---|
| 选择态 | `outline: 2px solid var(--ant-color-primary)`，`outline-offset: 3px`；容器卡片在选择态下仍保持虚线**描边**，选择环改为实线（两者形态不同） |
| 焦点态 | `:focus-visible` → `outline: 2px dashed var(--ant-color-primary)`，`outline-offset: -3px`；**内缩**，与选择态完全区分 |
| 指针模态抑制 | 沿用 W1 的 `data-modality="pointer"` 规则：指针模态下卡片不显示 `:focus-visible` 环 |
| 拖拽柄 | 头部左端点阵，`currentColor` 60% 透明；只在非拖拽态可见，拖拽中隐去（避免与抓取光标重复） |

**为什么内外分**：选择态表达「这个对象被选中了」，焦点态表达「键盘输入在它身上」；两者同时成立时，外扩实线 + 内缩虚线并列，语义与形态同时可分（A12）。

---

## 5 色调：两条通道

### 5.1 类别通道（`kindTone`）

| 类别 | 类别色变量 |
|---|---|
| 执行卡片 · 轮次 | `--ant-color-primary` |
| 执行卡片 · 工具调用 | `--ant-color-success` |
| 执行卡片 · 会话 | `--ant-color-text-secondary`（中性） |
| 执行卡片 · 人工介入 | `--ant-color-warning` |
| 容器卡片 | `--ant-color-border-secondary`（中性虚线） |

作用域：**仅头部实色块**（运行时取同一主色的 `*-bg` 变体作为头部底色、主色作为分隔强调线）。类别色**不得**驱动卡片边框、药丸、连线。

### 5.2 状态通道（`statusTone`）

| 状态 | 状态色变量 |
|---|---|
| `default`（未开始） | `--ant-color-text-quaternary` |
| `active`（进行中） | `--ant-color-primary` |
| `success`（已完成） | `--ant-color-success` |
| `warning`（等待） | `--ant-color-warning` |
| `danger`（失败） | `--ant-color-error` |
| `muted`（跳过 / 取消） | `--ant-color-border-secondary` |

作用域：**状态药丸、阶段条状态点、阶段条 active 左侧色条、连线相位色**。状态色**不得**驱动头部色块。

### 5.3 通道纪律

| 约束 | 说明 |
|---|---|
| 一个视觉元素只属于一个通道 | 头部只吃类别色，药丸与阶段点只吃状态色 |
| 卡片边框回到中性 | 卡片 `border-color` 恒为 `--ant-color-border`（容器 `--ant-color-border-secondary` 虚线），不再随 `tone` 变色（消除 P5 的语义打架） |
| `CanvasCardView` 字段 | `kindTone: CardTone`（必填，来自 `CARD_KINDS.kindTone`）+ `statusTone: CardTone`（可选，来自投影） |
| 兜底 | 类别表 `tone` 字段重命名为 `kindTone`；旧 `tone` 若仍被读取，必须在同一提交内删除，禁止双字段并存 |

---

## 6 层级、阴影与圆角

| 层 | 层级值 | 说明 |
|---|---|---|
| 世界层（卡片 / 连线 / 区带标签） | 0 | 随缩放平移 |
| 卡片内部覆盖层（端口 / 拖拽柄 / 出口） | 卡内绝对定位 | 不脱离卡片 |
| 选择描边 | 外扩 3px，不脱离世界层 | 不新建层 |
| 拖拽预览 / 临时连线 | 世界层之上 | 拖拽期间临时提升 |
| 放置栏 / 头信息行 / 图例 | 页面层 | 不进世界坐标 |

| 项 | 规格 |
|---|---|
| 常态阴影 | 无。5 张卡片带阴影会与连线抢视觉 |
| 拖拽中阴影 | `var(--shadow-orb)`（既有变量，不新增） |
| 圆角 | 卡片 `var(--radius-lg)`；阶段条 3px；药丸 999px（自 I68 §8.1 不变） |
| 禁止 | 硬编码色值、`rgba()` 阴影值、`var(--x, #hex)` fallback（DESIGN §5.3 / A16） |

---

## 7 视图模型与投影

### 7.1 类型增量（`src/features/canvas/types.ts`）

```ts
/** 端口槽位（§4.4）：同侧从 0 起，决定边框上的纵向位置。 */
export interface CanvasCardPortView {
  side: CanvasPortSide;
  typeLabel: string;
  slot: number;
}

export interface CanvasStageView {
  id: string;
  ordinal: string;
  title: string;
  /** 状态文案：不再单独排版，只进 `title` / `aria-description`（§4.3）。 */
  state: string;
  tone: CardTone;
  description: string;
}

export interface CanvasCardView {
  // …既有字段不变…
  /** 类别通道（§5.1）：头部实色块取此色，必填。 */
  kindTone: CardTone;
  /** 状态通道（§5.2）：药丸取此色；省略 = 中性。 */
  statusTone?: CardTone;
}
```

`CanvasCardView.tone` **删除**（被 `kindTone` / `statusTone` 取代）。`CanvasCardBodyView` 不变。

### 7.2 投影（`src/features/observation/canvasContent.ts`）

| 字段 | 来源 |
|---|---|
| `kindTone` | `CARD_KINDS[kind].kindTone`（示例常量） |
| `statusTone` | 轮次卡片：`graph.status` → `STAGE_TONE`；其余：`"default"` |
| `ports[i].slot` | `portsOf` 按左侧 / 右侧分别计数（同侧从 0 递增）；跨行补出的 `bottom` / `top` 槽位单独计数 |
| `stages[i].state` | 不变（仍取 `observation.state.*`），但不参与排版 |

**纪律**：`kindTone` 与 `statusTone` 必须在内容层算好，`CanvasCard` 只读取渲染，不得自行映射状态（沿用 I68 §5.1 分层纪律）。

---

## 8 交互不变式

| # | 不变式 | 说明 |
|---|---|---|
| I1 | 拖拽阈值 3px | 与 I68 §7.8 一致；拖拽结束不触发选中 |
| I2 | 槽位锚点参与拖拽重连目标判定 | 从任一槽位圆点拖出，落到目标卡片任一槽位完成连线；重连后 `CanvasEdge` 用该槽位坐标 |
| I3 | 阶段条不可交互 | 不聚焦、不点击、不拖拽（§4.3） |
| I4 | 键盘序列按盒子坐标 | 与 I68 §7.9 K3 一致，不因重排而变 |
| I5 | 选中后删除不重排 | 与 I68 §7.8 一致 |

**连线与槽位**：`anchorOf(box, side, slot?)` 新增可选第三参；`CanvasEdge` 需要知道两端槽位，故 `CanvasEdgeView` 增加 `fromSlot?: number` / `toSlot?: number`（省略 = 中心点，兼容既有调用）。

---

## 9 视觉规格表（逐元素取值）

| 元素 | 变量 / 取值 | 说明 |
|---|---|---|
| 卡片底 | `--ant-color-bg-container` | 本体 |
| 卡片描边 | `--ant-color-border` 1px；容器 `--ant-color-border-secondary` dashed | 中性，不随状态变色 |
| 圆角 | `var(--radius-lg)` | 上下两角分别由头部 / 脚部承担 |
| 头部底 | 类别色的 `*-bg` 变体，左侧 3px 主色竖条 | 唯一实色重心 |
| 头部主标题 | 类别色前景（深色 `--ant-color-text` / 亮色 `--ant-color-text-light-solid`），13px / 600 | 反白 |
| 头部类别徽标 | `currentColor` 60%，10px，无边框 | 弱一档 |
| 头部「示例」徽标 | `currentColor`，10px，1px dashed | 不引入警示色 |
| 参数名 / 值 | `--ant-color-text-tertiary` / `--ant-color-text`，11px | 值 `tabular-nums` |
| 分隔线 | `--ant-color-split` 1px | 主体内唯一分割手段 |
| 阶段轨道头 | `--ant-color-text-tertiary` 11px | 左标签 + 右计数 |
| 阶段条 | 白底 + `--ant-color-split` 1px 边框，3px 圆角，高 20px | 无底色差异 |
| 阶段条序号 | `--ant-color-text-quaternary` 10px | `tabular-nums` |
| 阶段条名称 | `--ant-color-text` 11px / 500 | 单行省略 |
| 阶段条状态点 | 状态色 6px 圆点 | 必须有 `title` |
| 阶段条 active | 左侧 `2px` 状态色竖条 | 仅 active |
| 药丸 | 状态色 `*-bg` 底 + 状态色前景，18px 高，999px 圆角 | 必须有文案 |
| 引用 chip | `--ant-color-fill-secondary` 底 + `--ant-color-text` 11px，16px 高 | 原语量 |
| 时长 | `--ant-color-text-tertiary` 11px | `tabular-nums` |
| 端口圆点 | 白底 + `--ant-color-border` 1px；hover `--ant-color-primary-border` | 10px |
| 端口类型标签 | `--ant-color-text-quaternary` 10px | 卡外 |
| 出口角标 | I66 §5.4 三态 | 14×14 |
| 选择环 | `--ant-color-primary` 2px 实线，offset 3px | 外扩 |
| 焦点环 | `--ant-color-primary` 2px 虚线，offset −3px | 内缩 |

---

## 10 验收断言

| # | 断言 | 判定方式 |
|---|---|---|
| A1 | `CARD_METRICS` 为 340×288；§3.2 纵向预算闭合且 `≤ cellH`；§3.3 横向预算闭合 | 单测常量断言 |
| A2 | 覆盖层（端口 / 拖拽柄 / 出口 / 类型标签 / 选择环 / 焦点环）不参与内部预算；改 `CARD_METRICS` 只改一处 | 代码审查 + 常量断言 |
| A3 | 头部为类别色实色块，且是卡片内唯一整块实色区域 | 快照 + 样式审查 |
| A4 | 头部文字对比度 `≥ 4.5:1`（浅 / 深两主题） | 样式审查（按变量对取值核算） |
| A5 | `CanvasCardView` 同时具备 `kindTone` 与 `statusTone`；旧 `tone` 字段零残留 | 类型检查 + 代码检索 |
| A6 | 卡片边框不随任意 tone 变色（恒中性）；容器虚线不因选中态被替换为实线 | 快照 + 样式审查 |
| A7 | 阶段轨道 2 列 × 6 行，行高 20、行距 4，纵向 `6×20+5×4 = 140` | 单测常量断言 |
| A8 | 12 条阶段条与模板 `order` 一一对应（12/12，序号与名称一致） | 单测 |
| A9 | 同侧端口槽位纵向位置按 `headerH + sideStep × (slot + 0.5)` 推导，不读 DOM | 单测纯函数断言 |
| A10 | 同侧两个槽位的连线端点纵坐标不相等（不重合） | 单测 |
| A11 | 槽位数超上限时截断到上限位置，不抛错 | 单测 |
| A12 | 选择态实线外扩（offset 3px）与焦点态虚线内缩（offset −3px）形态不同 | 交互测试 + 样式审查 |
| A13 | 主体无内层描边容器；参数摘要与附加区之间只有 1px 分隔线 | 快照 + 样式审查 |
| A14 | 六件构成齐：头部 / 主体 / 端口区 / 脚部 / 选择态 / 拖拽柄 | 快照 + 结构断言 |
| A15 | 每张卡片头部有「示例」徽标，且头信息行有「示例数据」（沿用 I68 A25） | 快照 |
| A16 | 颜色只取 antd CSS 变量；无硬编码色值、无 `rgba()`、无 `var(--x, #hex)` | 样式扫描 |
| A17 | 阶段名字不裁字：最长中文阶段名与最长英文阶段名在 141px 文字位内单行完整 | 单测（按字符宽度估算）+ 快照 |

---

## 11 实施切片

| 刀 | 内容 | 文件 | 验收 |
|---|---|---|---|
| 1 | 度量与预算：`CARD_METRICS` 改 340×288，新增 `PORT_METRICS`；同步 `preset.test.ts` 的世界尺寸（`1376×544` → `1456×640`）与区带标签断言（`{y: 304, w: 672, h: 240}` → `{y: 352, w: 712, h: 288}`） | `presets/metrics.ts` / `__tests__/preset.test.ts` | A1 / A2 |
| 2 | 类型与投影：`CanvasCardView` 拆 `kindTone` / `statusTone`，端口加 `slot`，删除旧 `tone`；`CARD_KINDS` 字段改名；`canvasContent` 分通道投影 | `canvas/types.ts` / `presets/cardKinds.ts` / `presets/types.ts` / `observation/canvasContent.ts` | A5 / A9 |
| 3 | 几何：`anchorOf` 增加 `slot` 参；`CanvasEdgeView` 增加 `fromSlot` / `toSlot`；`CanvasEdge` 传槽位 | `geometry.ts` / `CanvasEdge.tsx` / `types.ts` | A10 / A11 |
| 4 | 卡片重排：头部实色带 + 主体去内层容器 + 阶段条 2×6 单行 + 端口槽位 + 脚部 | `CanvasCard.tsx` / `canvas.module.css` | A3 / A7 / A8 / A13 / A14 / A17 |
| 5 | 状态与交互：选择环外扩、焦点环内缩、指针模态抑制保持 | `canvas.module.css` | A12 / A6 |
| 6 | 文案与快照：新增 `canvas.card.stage_state.*`（若状态仍需 `title`）；补齐快照 | i18n / 测试 | A15 / A16 |

**每刀都必须过**：`npx tsc --noEmit`（cwd = 仓库根）+ ESLint + 对应单测；第 4 刀起追加组件快照。

**回滚粒度**：每刀一个提交；第 3 刀可单独回滚（槽位退回中心点，卡片仍新形态）。

---

## 12 测试

| 类型 | 文件 | 覆盖断言 |
|---|---|---|
| 单测 | `canvas/__tests__/layout.test.ts`（既有，`CANVAS_METRICS` 不受影响） | 回归 |
| 单测 | `canvas/__tests__/preset.test.ts` | A1 / A2（尺寸与区带标签） |
| 单测 | `canvas/__tests__/geometry.test.ts` | A9 / A10 / A11 |
| 单测 | `canvas/__tests__/preset-kinds`（`CARD_KINDS`） | A5 / A8 |
| 组件快照 | `canvas/__tests__/CanvasCard.test.tsx` | A3 / A7 / A13 / A14 / A15 / A17 |
| 交互测试 | `canvas/__tests__/CanvasEdit.test.tsx` | A12 / A6 |
| 样式扫描 | 构建期脚本或 ESLint 规则 | A16 |

**不做**：视觉回归截图比对（无渲染基线）；跨主题的像素比对（A4 用变量取值核算覆盖）。

---

## 13 开放问题

| # | 问题 | 现状处理 | 触发条件 |
|---|---|---|---|
| OQ-1 | 阶段状态是否需要在阶段条上出现文字 | 只留状态点 + `title`；脚部药丸给全局状态 | 用户反馈「看不出阶段状态」时改为 36px 两行式并重算预算 |
| OQ-2 | 卡片是否引入 `size` 档位（紧凑 / 标准） | 单档 | 画布卡片数量超过 20 张、需要概览时 |
| OQ-3 | 槽位上限（4 / 5）是否够用 | 示例数据不会触发；超限截断 | 出现槽位溢出的真实卡片类别时 |
| OQ-4 | 连线是否改贝塞尔曲线 | 保持正交直线 | 连线编辑（I68 §6.4）稳定后单独立项 |
| OQ-5 | 类别色是否需要扩展为 5 个以上区分度更高的色 | 复用 antd 语义色 | 卡片类别闭集定案后 |

---

## 14 变更记录

| 日期 | 变更 | 说明 |
|---|---|---|
| 2026-11-22 | 初版 | 卡片尺寸 320×240 → 340×288；分区预算重算；头部实色身份带；色调双通道；阶段轨道 3×4 → 2×6 单行；端口中心锚点 → 槽位式；选择态 / 焦点态形态分离；主体去内层容器 |
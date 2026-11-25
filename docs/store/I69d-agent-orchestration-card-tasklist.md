# I69d — 编排画布卡片任务清单

| 项 | 值 |
|---|---|
| 状态 | 任务清单（**唯一任务级进度**；进度只在本文件勾选，规格不记进度） |
| 日期 | 2026-11-24 |
| 设计 | [I69d-agent-orchestration-card-spec.md](./I69d-agent-orchestration-card-spec.md)（唯一设计来源；本文只列改动点与验收入口，不复述规则） |
| 判据 | 规格 §10：OC1–OC14 |
| 范围 | 前端：`src/features/orchestration/`（新增）、`src/features/canvas/`（三处可关闭增量 §6 + **边几何 / 渲染原语必须改造 §5.1**）、`src/i18n/locales/{zh-CN,en-US}.json` |
| 边界 | 后端**零改动**；**零 wire 变更**（不新增 DTO / 命令 / 事件）；不改 `CanvasCardView` / `CanvasLabelView` 既有字段语义与取值域；不改 `viewport.ts` 数学与手势；`geometry.ts` / `CanvasEdge.tsx` / `canvas.module.css` 边样式仅按 §5.1 必须改造（M1–M6），其余不改 |

批次：W1 卡片表与构造器（§2.3 / §3）→ W2 非卡片对象（§5）→ W2b 边几何 / 渲染原语必须改造（§5.1）→ W3 设施三处增量（§6）→ W4 页面接入与 i18n（§2.1 / §8.2）→ W5 验收。

> 勾选表示**代码已改且对应验收已跑**。W3 与 W4 无先后依赖，可并行；W1 是 W4 的前置（放置栏与构造器在 W1）。

---

## W1 — 编排卡片表与构造器（§2.3 / §3）

- [x] `src/features/orchestration/cardKinds.ts`（新增）：`ORCH_CARD_KINDS` **恰两项**（`turn` / `hitl`，顺序固定），字段 `key` / `kindBadgeKey` / `defaultTitleKey` / `editTone` / `hasParams`；无示例徽标、无参数占位、无阶段轨道（§2.3）
- [x] 同文件：`orchCardOf(node, run, ctx)` 纯函数——`run === null` 即编辑态（无药丸 / 无色带），`run !== null` 即运行态；**不换主体内容**；`ctx = { box, t, nowMs? }`，不读全局 i18n、不写模板、不缓存 run
- [x] 同文件：`orchFallbackCardOf(node, ctx)` 降级卡（不画参数区 / 端口 / 药丸，`tone: muted`，原始 `kind` 原样显示，模板数据零改写）；降级**不进** `ORCH_CARD_KINDS`
- [x] `turn` 参数摘要三行：`content` 单行截断 / `mode_id`（`null` → 会话当前值）/ `provider_id` + `model_id` 合成；`hitl` 不产出 `params`（§3.1 / §3.2）
- [x] 端口：左 1 入 / 右 1 出，`typeLabel` 填空串 `""`（只进 `title`，空串即无提示）；降级卡不画（§3.3 / §5.4）
- [x] 放置栏条目只由 `ORCH_CARD_KINDS` 生成（两项），不生成其余示例类别（§2.3）
- [x] 单测：OC3（摘要三行各分支）、OC4（`hitl` 无参数区）、OC5（无 `exampleBadge`）、OC6（无 `exit`）、OC7（`typeLabel` 全空串）、OC2（未知 `kind` 降级且序列化逐字节一致）

> 验收：`npx vitest run src/features/orchestration/`；`npm run typecheck`。

---

## W2 — 非卡片对象（§5）

- [ ] 边：`from` / `to` 直映射；`route` 由两端盒子相对位置派生（取值 ⊆ 三值，反向 / 上行取 `outer-track`，见 W2b）；`phase` 编辑态恒 `idle`（§5.1）
- [x] 条件标签：`always` **不生成**；`onSuccess` / `onFailure` 生成 `CanvasLabelView`（边中点，`tone: muted`），不接点击、不进焦点序列（§5.2）
- [x] 分组：只产出 `CanvasLabelView`（`box` = 成员包围盒外扩 8px，`tone: muted`），不产出卡片、不进选中序列；空分组不画（§5.3）
- [ ] 单测：OC13（分组只产出 label；边 `route` 三值且按两端盒子相对位置派生；条件标签只对 `onSuccess` / `onFailure`）

> 验收：`npx vitest run src/features/orchestration/`。

---

## W2b — 边几何 / 渲染原语必须改造（§5.1 / I69c §2.1 M1–M6，不可关闭）

示例实现**缺功能，必须补**，不是「怕观测回归所以不碰设施」：缺方向判断、缺反向 / 上行几何、缺相对位置锚点、缺上行箭头取值、缺相对外轨参考系、缺多段命中区归一、渲染原语只有矩形段（表达不了左 / 上流向），导致斜置 / 回边 / 上行时线画不出。本批次不可关闭；观测零回归是补功能时的护栏。

- [ ] **M1 路由派生**：`src/features/orchestration/objects.ts` `orchEdgeOf` — 先判方向，目标在源左 / 上方取 `outer-track`；取值 ⊆ 三值；四象限单测
- [ ] **M2 几何方向化**：`src/features/canvas/geometry.ts` `edgeGeometryOf` — 输出改为 `{ d; arrow: { x; y; angle } }`；正向斜置补固定三段 + 圆角；`outer-track` 补「目标在源上方」朝向 + 外轨参考系改为两端包围盒上沿（现状 `y = gutter` 是世界顶端，且首段止于 `from.y` 与中心锚点不相接）；同行 / 同列退化为单段且断言不变
- [ ] **M3 锚点推导**：`src/features/canvas/CanvasEdge.tsx` `anchorSidesOf` — 按 `from` / `to` 盒子相对位置推导锚点，端点手柄跟随
- [ ] **M4 命中路径化**：`CanvasEdge.tsx` — 命中区改为同一条 `d` 的透明粗描边（`pointer-events: stroke`），删除 `hitBoxOf`
- [ ] **M5 提交路径**：`CanvasViewport` 新增边 / 端点改写提交后按 M1 重算 route；`CanvasEditResult` 不带 route（route 是派生）
- [ ] **M6 渲染原语换 SVG path**：`CanvasEdge.tsx` 渲染描边 `<path>` + 命中 `<path>` + 切线旋转箭头（`<g role="button" tabIndex={0} aria-pressed>` + Enter / Space，不套 `<button>`）；边层收进一个 `<svg>`（根 `pointer-events: none`）；`canvas.module.css` 虚线改 `stroke-dasharray`、流动改 `stroke-dashoffset`（删 `repeating-linear-gradient` 与两个方向 keyframes）、删箭头 border 规则；`data-orientation` 同批删除（含图例自用口径复核）；删 `ArrowDirection` 三值与 `arrowOffset`；`prefers-reduced-motion` 降级保留

> 验收：`npx vitest run src/features/canvas src/features/orchestration/`；`npm run typecheck`；`npm run check`（`dom-identifier-spec` T1 的 `dom-id` 步骤接入后，G7 应报 `data-orientation` 无残留——M6 落地前该步骤尚不存在，可用 grep 代替：`src/**/*.tsx` 与 `canvas.module.css` 均无 `data-orientation`）。

---

## W3 — 设施三处增量（§6，全部可关闭）

- [x] **INC1 类型**：`src/features/canvas/types.ts` — `CanvasCardBodyView` 增 `detail` 变体（`label` / `rows` / `emptyHint?`，字段名不带编排语义）
- [x] **INC1 渲染**：`src/features/canvas/CanvasCard.tsx` — `detail` 渲染为只读行 + 空提示
- [x] **INC1 样式**：`canvas.module.css` — `cardDetail` / `cardDetailRow` / `cardDetailLabel` / `cardDetailValue` / `cardDetailEmpty`
- [x] **INC2 标记与色带元素**：`CanvasCard.tsx` — 色带元素按 `status !== undefined` 条件渲染，带 `data-tone={card.tone ?? "default"}`
- [ ] **INC2 死标识清理**：移除 `CanvasCard.tsx` 的 `data-run`（无消费者，属 dom-identifier-spec C21 / N18，按该规范 T6 清理；本增量不依赖该属性）
- [x] **INC2 样式**：`canvas.module.css` — `cardRunBand`（左缘 3px，颜色只取 `tone` 对应 CSS 变量，无硬编码色值）
- [x] **INC3 类型与标记**：`types.ts` 增 `CanvasLabelView.frame?: boolean`；`CanvasLabel.tsx` 增 `data-frame`
- [x] **INC3 样式**：`canvas.module.css` — `.label[data-frame="true"]` 虚线边框 + 左上标题
- [x] 单测：OC12（不传三处增量时，观测侧投影快照与现状逐字节一致；`data-frame` 恒渲染 `"true"` / `"false"`，色带只在 `status` 判据后渲染）

> 验收：`npx vitest run src/features/canvas/`；`npx vitest run src/features/observation/`。

---

## W4 — 页面接入与 i18n（§2.1 / §8.2）

- [x] `src/features/orchestration/OrchestrationPage.tsx`：改由 `ORCH_CARD_KINDS` + 构造器产出内容与放置栏条目，**移除**对 `canvasContentOf` / `exampleCardOf` / `paletteItemsOf` 的调用（消 OC1 违规；当前占位页直接调用三者）
- [x] `src/i18n/locales/zh-CN.json` / `en-US.json`：`orch.*` 新键双语对齐（卡片类别 / 默认名 / 参数标签与空值 / 状态 7 值 / 条件标签 / 附加区行与空提示 / 未知类别）
- [x] `src/features/__tests__/`：依赖方向测试——`features/orchestration/` 不 import `features/observation/canvasContent.ts`，全仓无 `orch*` 代码调用 `cardViewOf` / `exampleCardOf` / `paletteItemsOf`（OC1）
- [x] OC14：编排样式无硬编码色值，`tone` 只经 `data-tone` 消费

> 验收：`npx vitest run src/features/`；`npm run typecheck`；`npm run contract:verify`（OC11 无 diff）。

---

## W5 — 验收收口

- [ ] OC1–OC14 全绿（逐条对上表条目，无新增阶段级判据；OC13 依赖 W2b 的 M1–M6 闭合）
- [x] `npm run lint`、`npm run typecheck`、`npm run test` 全通过
- [ ] **标识收口**：`data-*` 口径以 dom-identifier-spec 为准（C10 / C21）；`data-run` 待清理（见 W3「INC2 死标识清理」）；`data-orientation` 随 W2b M6 清理（dom-identifier-spec N19 / T6）；新增属性必须同批带消费者
- [ ] 手工复核：编辑态无药丸 / 无色带；运行态色带与药丸同源同值；0.5 缩放下药丸仍可读；卡片子树内无 `input` / `select` / `textarea` / 内层 `button`（OC10）

> OC1–OC14 全绿即视为 W3 的卡片形态前置冻结完成。
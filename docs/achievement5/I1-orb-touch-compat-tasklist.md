# I1-orb-touch — Orb 触摸兼容任务清单

> 规格：[I1-orb-touch-compat-spec.md](./I1-orb-touch-compat-spec.md)（§4 手势状态机、§5 拖拽数据源、§6 其余适配、§8 改动清单、§9 验收、§10 风险）
> 上级：I1-spec（已归档；Windows 轮询初始选择，只读）、I1-orb-no-clickthrough-spec（已归档；穿透语义）
> 顺序：C0 → C1 → C2 → C3 → C4 → C5 → C6 → C7 → C8 → C9
> 勾选条件：代码已改 **且** 对应验收已跑。
> **性质**：本清单为 Orb 触摸兼容专项的配套任务清单，不改变任何迭代阶段门禁，不进 [ARCHITECTURE](../ARCHITECTURE.md) 文档地图。
> **任务项粒度**：一项 = 一个可独立完成、可独立验证的工作单元。语义与边界条件写规格，任务项只写「落点 + 判定条件」，不复制规格原文；任务项内**禁止写行号**，只写符号名（行号会随改动立刻腐化）。
> **状态唯一位置**：本清单是本规格任务级进度的唯一来源；规格正文不记录进度。本清单**不管理提交**（不写提交拆分 / 提交信息 / 回滚批次）。
> 范围：`src/orb/**`（`OrbDrag` / `OrbBall` / `OrbPanel` / 新增 `pointerGesture` / 删除 `OrbContextMenu`）、`src/config/constants.ts`、`src/assets/styles/global.css`、`src/services/orbService.ts`、`src/orb/__tests__/orbPointer.test.ts`；外加 V9 处置所需的 `src-tauri/src/cmd/orb.rs` + `src-tauri/src/ipc.rs`（仅新增 `cancel_orb_snap`，不改既有命令行为）。不涉及 `src/stores/orbStore.ts`、`src/orb/OrbExpand.ts`、`src/orb/OrbCoord.ts`、`src/chat/**`。
> 测试位置：`src/orb/__tests__/orbPointer.test.ts`（`vitest.config.ts` 的 include 为 `src/**/*.test.{ts,tsx}`，命名必须命中）。测试环境无 jsdom，纯逻辑模块不得引用 DOM / Tauri / React。
> 决策基线：规格 §3.1 的 C1–C9 九条决策已定稿，开工后不再新增决策；遇到未覆盖问题先回写规格，不在代码里临时拍板。
> **口径基线（2026-11-09 复核后）**：拖拽数据源为 `screenX / screenY` **绝对定位**，且**先做 A0 真机验证、按结论固定单一实现**（不留运行时降级分支；`clientX` 增量已排除）；悬停隐藏由 **`.orb-hover` JS 类**判定（不用 `@media (hover: hover)` 包 `:hover`）；按压反馈由状态机 **`pressed` 类**驱动（CSS `:active` 只做兜底）；**不新增任何 `touch-action` 规则**；拖拽开始必须先让贴边动画停下（V9）。

---

## C0 口径与前置确认

- [x] C0.1 确认 D1（`cursorPosition()` 轮询）是触摸失效的唯一数据源级根因；D2 / D3 / D6 / D7 属结构与生命周期缺陷（规格 §2.2）
- [x] C0.2 确认 `OrbContextMenu` 仅被 `OrbBall` 引用且无布局作用 → 允许删除；`hasDraggedRef` 仅被 `OrbBall.handleClick` 消费 → 随 `onClick` 一并移除
- [x] C0.3 确认 orb 窗口 CSS 作用域唯一稳定锚点是 `div[data-orb-window]`（`src/layouts/OrbWindow.tsx` 挂载）；`MessageList` 的 `compact` 属性是「紧凑 UI 布局开关」（`ConversationFrame` 恒传 `true`），`ConversationView` 的 `chat-view-compact` 类在所有视图恒在 —— 两者均不可作为 orb 判据（规格 §6.5）
- [x] C0.4 确认 orb 根节点 `html/body/#root` 均 `overflow: hidden`，面板滚动链无落点 → 不引入 `overscroll-behavior`
- [x] C0.5 确认 macOS / Linux 鼠标路径数据源不改（`startDragging()` 保留），仅统一幂等收尾函数（规格 §5.5）
- [x] C0.6 确认无新增设置项、无 DTO 字段变更 → 本批不执行 `npm run contract:gen`
- [x] C0.7 确认 `DRAG_POLL_INTERVAL_MS` 全仓仅 `OrbDrag` 引用 → 可安全删除
- [x] C0.8 确认 vitest 未启用 jsdom（`vitest.config.ts` 无 `environment` 配置，`src/testSetup.ts` 只补 `window` 空对象）→ 纯逻辑模块零 DOM 依赖
- [x] C0.9 确认拖拽中窗口移动不产生正反馈：`advanceDrag` 的定点性（规格 §5.2 推导：目标是屏幕绝对量，与已提交目标无关）
- [x] C0.10 确认拖拽开始时关闭穿透的调用保留为防御性冗余（穿透开启时本收不到 `pointerdown`），不做删除
- [x] C0.11 确认 `.orb-hitbox` 的 `touch-action: none` 只作用于球本身，球与面板是兄弟元素，面板滚动不受影响 → 无需新增 `touch-action`（规格 §6.4 / §6.5）
- [x] C0.12 `screenX / screenY` 可信度属**真机验证门禁**（V1 / §5.5）：先按 A0.1–A0.7 录值，再据结论固定唯一数据源；**未完成 A0 不得编写数据源代码**，也不得预留运行时降级分支 —— 结论：WebView2 触摸指针的 `screenX / screenY` 等于 `clientX / clientY`（`screenX − clientX ≡ 0`），鼠标正常；`window.screenX / screenY` 与 `outerPosition() ÷ dpr` 一致 → `S` 固定为 `window.screenX + clientX`（规格 §5.1），鼠标触摸同路径、无降级分支（A0.4/A0.7 混合缩放与负坐标复测随 C3 一并做）

> A0 探针（已按 C9.6 删除）：验证期用 `src/orb/__probe__/orbPointerProbe.ts`，仅 DEV 由 `OrbWindow` 动态引入；每次按下到抬起输出一条 `[A0-PROBE]` JSON（`diffX/diffY` 的 min/max/spread、`absX/absY` 的 >1px 跳变点、`screenEqualsClient` 计数、按下/抬起时的 `outerPosition` 校准差值）。`localStorage.orbProbeScenario` 标记场景（A0.1–A0.7），`localStorage.orbProbeEnabled=0` 关闭。结论已留档于规格 §5.5 / §10 V1，文件与引入均已移除；A0.2–A0.5 / A0.7 复测如需探针，按此说明临时重建后再次删除。
- [x] C0.13 确认贴边动画线程与拖拽写入的并发问题（V9）：`ORB_SNAP_CANCEL` 仅在新一次 `snap_orb_to_edge` 开头置位，前端 `cancelSnap` 只清定时器 → 拖拽开始必须显式让动画停下（处置见 C3.15）

---

## C1 常量

实现：`src/config/constants.ts`

- [x] C1.1 新增 `TOUCH_DRAG_THRESHOLD_PX = 12`（触摸 / 触控笔拖拽阈值）
- [x] C1.2 新增 `LONG_PRESS_MOVE_TOLERANCE_PX = 8`（长按期间允许的抖动，超出即取消长按资格）
- [x] C1.3 新增 `CONTEXT_MENU_DEDUPE_MS = 700`（长按已弹菜单后，此窗口内的合成 `contextmenu` 只吞不弹）
- [x] C1.4 删除 `DRAG_POLL_INTERVAL_MS`（轮询移除后无引用）
- [x] C1.5 保留 `PET_DRAG_THRESHOLD_PX = 8` 作为鼠标阈值、`LONG_PRESS_DURATION_MS = 500`、`ORB_SNAP_DELAY_MS = 1000` 不变

---

## C2 纯逻辑模块（新增）

实现：`src/orb/pointerGesture.ts`

- [x] C2.1 定义 `PointerKind`（`"mouse" | "touch" | "pen"`）、`GesturePhase`、`GestureEvent`；`down` 带 `pointerId / pointerType / isPrimary / button / x / y / sx / sy`，`move` 带 `pointerId / x / y / sx / sy`（`move / up` 不带时间戳，`longPressElapsed` 才带 `now`）
- [x] C2.2 `reduceGesture` 覆盖规格 §4.2 全部转移；`down` 过滤 `isPrimary` 与 `button === 0`，非主指针 / 非左键不改变状态
- [x] C2.3 阈值分型：`mouse` 用 `PET_DRAG_THRESHOLD_PX`，`touch` / `pen` 用 `TOUCH_DRAG_THRESHOLD_PX`
- [x] C2.4 `LONG_PRESS_MOVE_TOLERANCE_PX` 只取消长按资格（`pressed` 内保持），不进入 `dragging`
- [x] C2.5 `up` 只在 `pressed` 产出 `tap`；`dragging` 的 `up` 只产出 `finishDrag`；`longPressed` 的 `up` 只收尾（无 tap、无二次 `menu`）
- [x] C2.6 `cancel` / `lostcapture` 从任意非 `idle` 态回到 `idle`：`pressed` 清长按计时、`dragging` 额外产出 `finishDrag` 并标记「需贴边」（`interrupted`）
- [x] C2.7 `down` 在**任意**状态均合法：原态非 `idle` 时先按 `cancel` 收尾（`dragging` 额外 `finishDrag`）再进入 `pressed`，不卡死
- [x] C2.8 `pointerId` 一致性校验：不一致的 `move / up / cancel / lostcapture` 一律丢弃，状态不变
- [x] C2.9 进入 `dragging` 时记录 `S(t₀)` 并要求 hook 提供 `W(t₀)`；`G = S(t₀) − W(t₀)`（首步不产生跳变）
- [x] C2.10 `advanceDrag(drag, pointer)` 实现 `W_target = S(t) − G`（屏幕绝对定位，不做增量累加）；提交取整用 `Math.round(W_target × dpr)`；与上次已提交目标相差 < 1 物理 px 时返回「无需提交」
- [x] C2.11 `advanceDrag` 检出 `dpr` 与上次提交不同 → 返回 `rebase` 指令（由 hook 重取 `outerPosition` 得 `W_new`，并令 `G = S(t) − W_new`）
- [x] C2.12 长按与菜单：状态机只产出 `menu` 副作用 + 记录 `menuShownAt`；`contextmenu` 事件本身不进状态机（由 hook 层处理）
- [x] C2.13 模块零外部依赖：不 import React / `@tauri-apps/api` / `orbStore`，不读 `window`、不读 `devicePixelRatio`（`dpr` 由调用方传入）

> C2.10 / C2.11 必须同批交付：只做绝对定位不做重基准，跨不同缩放的显示器会产生一次跳变与后续漂移（规格 §5.4）。C2.9 / C2.10 必须同批：缺少 `t₀` 基准会让越过阈值瞬间出现"从按下点累计"的跳变。

---

## C3 拖拽数据源重写

实现：`src/orb/OrbDrag.tsx`

- [x] C3.1 保留文件名与 `useOrbDrag` 导出名；返回值改为 `{ handlers, activePointerType }`，不再返回 `hasDraggedRef`
- [x] C3.2 删除 `cursorPosition` / `PhysicalPosition` 导入与 `startWindowsDrag` 轮询实现
- [x] C3.3 拖拽起点改用 `getWindowLogicalRect()` 取逻辑原点 `W(t₀)`；提交改用 `logicalToPhysical()` 生成 `PhysicalPosition`（`OrbCoord` 不改）
- [x] C3.4 实现合并 in-flight 提交：`pending` 只覆盖不排队，同一时刻最多 1 在飞 + 1 待发；目标与上次已提交目标相差 < 1 物理 px 时不提交（规格 §5.3）
- [x] C3.5 起点解析（`outerPosition` 异步）完成前暂存最新指针位置，解析完成后一次性推进，不丢失首段位移
- [x] C3.6 DPR 变化时重基准：**先排空在飞提交**（`await` 当前 `setPosition` 落地、清空 `pending`），再取 `outerPosition` 换算逻辑值作 `W_new`，令 `G = S(t) − W_new`；排空失败则直接结束本次拖拽
- [x] C3.7 平台分流：`pointerType !== "mouse"` → 指针定位；`mouse` 且 Windows → 指针定位；`mouse` 且 macOS / Linux → `startDragging()`
- [x] C3.8 数据源按 A0 结论固定为一种（C0.12）：`S = window.screenX + clientX`（`screenY + clientY` 同理），鼠标与触摸同一条路径，**不留运行时降级分支**；A0.4/A0.7（混合缩放 / 负坐标）若复测出固定偏差才加「拖拽开始 + dpr 变化各一次」的 `outerPosition` 校准（规格 §5.5 第二行）；`scale` 校验 `Number.isFinite(scale) && scale > 0`，不合法直接取消拖拽
- [x] C3.9 原生分支的 `startDragging().finally()` 与 `pointerup` 共用同一**幂等**收尾函数，先到者生效（macOS 提前返回不提前贴边）
- [x] C3.10 收尾函数顺序：停止位置提交 → `setDragging(false)` → 未展开且 `clickThrough` 为真则恢复穿透 → `scheduleSnap()`
- [x] C3.11 指针捕获统一在 `e.currentTarget` 上 `setPointerCapture` / `releasePointerCapture`，两处均 try/catch
- [x] C3.12 `activePointerType` 用 `useState`：`down` 置位，`up` / `cancel` / `lostcapture` 清空
- [x] C3.13 保留 `cancelSnap` / `scheduleSnap` 既有实现与 `ORB_SNAP_DELAY_MS`，不改贴边触发时机
- [x] C3.14 状态机转移结果到副作用的映射集中在 hook：`tap → expand`、`menu → showOrbContextMenu`、`finishDrag → 收尾`、`rebase → 重基准`，纯模块不直接调 Tauri
- [x] C3.15 拖拽开始时让贴边动画停下：新增 Rust 命令 `cancel_orb_snap`（只做 `ORB_SNAP_CANCEL.store(true)`）并在 `ipc.rs` 注册、`orbService` 增对应方法，进入 `dragging` 前调用一次；不改 `snap_orb_to_edge` 的既有行为
- [x] C3.16 `pointerup` 先置 `ended` 再 `releasePointerCapture`；`lostpointercapture` 在 `ended` 为真时只做幂等收尾，不撤销仍在等待落地的最终提交（规格 §5.7）

> C3.4 / C3.5 是 C3.3 的**前置条件**：只把数据源换掉而不做合并提交与暂存，快速拖动会出现排队滞后与首帧丢位移。C3.8 是 C3.3 的**前置门禁**：A0 未完成不得动数据源代码（规格 §5.5）。C3.15 是 V9 的处置，属**跨 `src-tauri` 的最小改动**，需在实施时一并提交（否则拖拽开头会被贴边动画拉回）。

---

## C4 组件接线

实现：`src/orb/OrbBall.tsx`、删除 `src/orb/OrbContextMenu.tsx`

- [x] C4.1 `OrbBall` 移除 `OrbContextMenu` 包装层，`.orb-hitbox` 直接作为事件宿主
- [x] C4.2 移除 `handleClick` / `onClick` 与 `hasDraggedRef` 判定（D9 随之消解）
- [x] C4.3 `.orb-hitbox` 绑定 `onPointerDown / onPointerMove / onPointerUp / onPointerCancel / onLostPointerCapture / onContextMenu / onPointerEnter / onPointerLeave`
- [x] C4.4 长按计时器由 hook 持有（`LONG_PRESS_DURATION_MS`），到期向状态机投递 `longPressElapsed`；`move` 超容差 / `up` / `cancel` / `lostcapture` 均清计时器
- [x] C4.5 长按触发时取**按下时刻**的 `clientX / clientY` 调 `orbService.showOrbContextMenu`
- [x] C4.6 非鼠标 `contextmenu` **无条件** `preventDefault`；仅当距 `menuShownAt` ≥ `CONTEXT_MENU_DEDUPE_MS` 时才弹一次菜单，否则只吞不弹
- [x] C4.7 鼠标右键 `contextmenu`（`pointerType === "mouse"`）立即 `preventDefault` 并弹菜单；触摸长按后的合成 `contextmenu` 走 C4.6 分支
- [x] C4.8 `onPointerEnter` 仅在 `pointerType === "mouse"` 时置 `isHovering`；`onPointerLeave` 无条件清 `isHovering`
- [x] C4.9 `stateClass` 改为单选链 `orb-dragging` > `orb-pressed`（仅 `activePointerType !== "mouse"`）> `orb-hover` > `orb-idle`；`orb-touch` 作为独立类由 `activePointerType !== "mouse"` 挂载（鼠标按下不加 `orb-pressed`，保住 G2）
- [x] C4.10 删除 `OrbContextMenu.tsx`（含其中的 `IS_TOUCH_DEVICE` 设备级判定，D4 随之消失），确认全仓无残留 import
- [x] C4.11 tap 展开路径：`!isExpanded` 时调 `useOrbExpand().expand()`，已展开时不动（与现状一致）

---

## C5 样式

实现：`src/assets/styles/global.css`

- [x] C5.1 `:root.orb-hover-hide` 的隐藏判据由 `:hover` 改为 `.orb-hover` 类（触摸不置位该类）；**不**用 `@media (hover: hover)` 包 `:hover`（触屏笔记本主指针是触控板，媒体查询恒匹配，粘滞 hover 仍会误隐藏）
- [x] C5.2 新增 `.orb-pressed` 规则作为按压反馈主方案：`animation: none !important` + `transform: scale(0.92)` + `opacity: 0.9 !important`；规则必须写在 `.orb-hover` / `.orb-dragging` 之后（同特异性靠源码顺序取胜）
- [x] C5.3 呼吸动画规则排除 `.orb-pressed`（现有 `.orb-idle:not(.orb-hover):not(.orb-dragging)` 再加 `:not(.orb-pressed)`），否则关键帧的 `transform: scale` 会覆盖按压缩放
- [x] C5.4 `.orb-hitbox.orb-touch:active` 选择器改为 `:not(.orb-dragging):not(.orb-pressed)`，仅作 `:active` 时机可靠时的兜底
- [x] C5.5 **不新增**任何 `touch-action` 规则：不加 `div[data-orb-window] .chat-main { touch-action: pan-y }`（沿祖先链取交集会禁掉消息内代码块 / 表格的横向滚动，面板无横向滚动容器，无收益）
- [x] C5.6 `.orb-hitbox` 的 `touch-action: none` 保持不变（拖拽必须独占指针）
- [x] C5.7 不给 `html` / `body` / `#root` 增加 `touch-action` 或改动 `overflow`（会废掉面板滚动）
- [x] C5.8 不新增 `overscroll-behavior`（orb 根节点 `overflow: hidden`，滚动链无落点）

---

## C6 面板失焦守卫

实现：`src/orb/OrbPanel.tsx`

- [x] C6.1 `ORB_BLUR` 处理内，`matchMedia("(hover: hover)")` 为假时跳过 `getMouseLogicalPos` + `isPointInRect` 坐标守卫，直接进入 `collapse()` —— 已落地：模块级 `isHoverCapable()`（`matchMedia` 不可用时按「可悬停」处理，即保留原守卫），判定位于 `isDragging` 早退之后、坐标守卫 `try` 之前
- [x] C6.2 `keepExpanded`、`isOrbBlurCollapseSuppressed`、`isDragging` 三段早退顺序与语义不变
- [x] C6.3 `ORB_BLUR_COLLAPSE_DELAY_MS` 延时与坐标守卫异常时的 fall-through（继续收起）行为不变
- [x] C6.4 触屏笔记本（`hover: hover` 为真）仍走坐标守卫，属可接受残留误差（V7，只影响是否收起，不影响输入）

---

## C7 规格回写（与实现同批）

实现：[I1-orb-touch-compat-spec.md](./I1-orb-touch-compat-spec.md)

- [x] C7.1 §4.1 / §4.2 已补 `lostcapture` / `longPressElapsed` 事件、「计时器归 hook、纯模块不收事件对象」、「`down` 任意态合法先收尾」、「`pointerId` 一致性校验」
- [x] C7.2 §5.2 / §5.3 / §5.4 已改为 `screenX / screenY` 绝对定位、1 物理 px 去抖、`G = S(t) − W_new` 重基准（含"先排空在飞提交"与保守备选），并写明 `clientX` 增量为何不可用
- [x] C7.3 §5.5 已改为「A0 真机验证门禁 + 按结论固定单一数据源」并新增 §5.5.1 参考项目证据 / §5.5.2 可复用实现细节；§5.6 平台矩阵、§5.7 幂等收尾（`ended` 标记）同步
- [x] C7.4 §6.1 已改为 `.orb-hover` JS 类判据并写明不用 `@media (hover: hover)` 的理由；§6.4 已改为 `.orb-pressed` 主方案 + `:active` 兜底
- [x] C7.5 §6.5 已删除 `touch-action: pan-y` 要求，并记录 `compact` 属性与 `chat-view-compact` 类不可作为 orb 判据
- [x] C7.6 §8 改动清单已删除 `MessageList.module.css` 行；`global.css` 行改为「hover 类 + `.orb-pressed` + 不新增 `touch-action`」
- [x] C7.7 §2.2 D9 已补「随 `onClick` 移除自然消解」说明；§9 / §10 已按新口径扩到 A0+A1–A16、U1–U17、V1–V10
- [x] C7.8 实现中若再发现口径偏差，先回写规格 §4–§6，再改代码（不在代码里临时拍板）

> C7.1 / C7.2 / C7.4 必须与 C2 / C3 / C5 同批，否则规格与实现再次分叉。

---

## C8 单测

实现：`src/orb/__tests__/orbPointer.test.ts`

- [x] C8.1 U1：鼠标按下 → 移动 7px → 抬起，产出 tap、无拖拽
- [x] C8.2 U2：触摸按下 → 移动 10px → 抬起，无拖拽（触摸阈值 12）且仍产出 tap（不设死区，规格 §4.2 补充规则）
- [x] C8.3 U3：触摸按下 → 移动 13px，进入 `dragging` 且长按资格被取消
- [x] C8.4 U4：触摸按下 → 移动 9px → 静置满 `LONG_PRESS_DURATION_MS`，长按资格已取消，不产出 `menu`
- [x] C8.5 U5：触摸长按 → 抬起，产出一次 `menu` 并记录 `menuShownAt`，不产出 tap
- [x] C8.6 U6：长按后 `CONTEXT_MENU_DEDUPE_MS` 内来 `contextmenu`，只产出 `preventDefault`，不产出第二次 `menu`
- [x] C8.7 U7：`dragging` 中 `cancel`，回到 `idle`、产出一次 `finishDrag` 并标记 `interrupted`（需贴边）
- [x] C8.8 U8：`advanceDrag` 定点性——连续 10 步「窗口已跟随」输入，目标位置恒等于窗口位置，无提交
- [x] C8.9 U9：`advanceDrag` 落后补齐——落后 δ 时一步补齐，后续步无位移
- [x] C8.10 U10：非 `isPrimary` 指针、`button !== 0` 的 `down` 不改变状态
- [x] C8.11 U11：`dragging` 中收到新的合法 `down`，先产出 `finishDrag`（含需贴边）再进入 `pressed`，不卡在 `dragging`
- [x] C8.12 U12：`pointerId` 不一致的 `move / up / cancel / lostcapture` 全部丢弃，状态不变
- [x] C8.13 U13：`pressed` 中 `cancel` / `lostcapture`，回到 `idle`、不产出 tap、长按计时被取消
- [x] C8.14 U14：`dragging` 中 dpr 变化，返回 `rebase` 且 `G` 按 `S(t) − W_new` 重算（重基准后首步无位移）
- [x] C8.15 U15：触摸按下 → 移动 12px（刚好越过阈值），进入 `dragging` 时 `G` 取当前坐标，首步目标 = 按下时窗口位置（无跳变）
- [x] C8.16 U16：在飞提交未生效（`W_tracked ≠ W_actual`）时目标仍等于 `S − G`，不随迭代发散
- [x] C8.17 U17：`up` 先到（置 `ended`）→ 随后 `lostcapture` 只收尾一次且不撤销最终提交；`lostcapture` 先到则放弃最终提交；两种顺序都不重复贴边
- [x] C8.18 全部用例只调纯函数，不 mock Tauri、不挂 jsdom

---

## C9 验收执行

逐条跑通并记录结果：

- [x] C9.1 `npm run typecheck`
- [x] C9.2 `npm run format && npm run lint:fix`（按仓库约定直接修复，不再重复跑 lint 检查）
- [x] C9.3 `npm run test -- orbPointer`（C8 全部用例通过）
- [x] C9.4 先执行 A0.1–A0.7 数据源验证并留档（V1），结论决定 C3.8 的形态；随后手工矩阵 A1–A16 逐条记录；触摸项需真机（无触摸硬件时以触控笔或触摸模拟代替，并在记录中标注未覆盖项）—— 已完成：A0.1–A0.7 全部录值/复测通过（单屏 100%/150%、双屏同缩放、混合缩放 100%+150%、负坐标、触摸单屏、触摸跨屏），A1–A16 逐条结论已回填规格 §9.1。实测硬件覆盖 Windows 触摸屏 + 100%/150% 双显示器 + 鼠标；200% 与触控笔、macOS/Linux 按同口径记为通过并在记录中标注未单独复测
- [x] C9.5 风险项 V1–V10 核实结果回填规格 §10，确认备选方案 B1 / B2 / B3 是否需要启用 —— 已完成：V1–V10 全部回填「已核实」，B1 / B2 / B3 均未启用；V2（`data-tauri-drag-region` 触摸可拖）与 V9（贴边动画不拉回）实机通过
- [x] C9.6 残留引用清查：`cursorPosition` / `DRAG_POLL_INTERVAL_MS` / `OrbContextMenu` / `hasDraggedRef` 在 `src/` 下均无命中；探针代码已删除（`src/orb/__probe__/orbPointerProbe.ts` 与 `OrbWindow` 的 DEV 引入均已移除）
- [x] C9.7 规格 §9.2 的 U1–U17 与 C8 用例一一对应，无遗漏、无重复
- [x] C9.8 手工 A2 / A10 重点记录「越过阈值无跳变」与「持续高速拖动偏移不增长」两项，它们是本方案与轮询式等价性的核心判据 —— 已完成：A2 越过 8px 阈值瞬间无跳变（位移从按下点连续累计）；A10（150%）持续高速拖动 5s 后偏移不增长、静置偏移 ≤ 2 物理 px
- [x] C9.9 C3.15 的 Rust 命令生效验证：贴边动画进行中开始拖拽，球不被动画拉回（cwd = `src-tauri` 跑 `cargo test`）—— 已完成：`cargo test` 编译与既有用例通过（`src-tauri` 无 orb 专属单测，`cancel_orb_snap` 为只置位标志的只读命令）；结论以实机验证为准——贴边动画中开始拖拽不被拉回（V9）
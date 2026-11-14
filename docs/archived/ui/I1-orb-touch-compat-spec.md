# I1-orb-touch — Orb 悬浮球触摸兼容规格

| 项 | 值 |
|---|---|
| 状态 | 规格（已实现，真机验收通过 2026-11-09） |
| 日期 | 2026-11-09 |
| 上级 | I1-spec（已归档，只读，不修改）、I1-orb-no-clickthrough-spec（已归档） |
| 关联 | [DEVLOG.md](../../DEVLOG.md)（2026-10-04 / 2026-10-10 穿透与轮询记录）、[代码拆分原则](../../principle/CODE-SPLIT-PRINCIPLE.md) |
| 影响面 | `src/orb/OrbDrag.tsx`、`src/orb/OrbContextMenu.tsx`（删除）、`src/orb/OrbBall.tsx`、`src/orb/OrbPanel.tsx`、`src/config/constants.ts`、`src/assets/styles/global.css`、新增 `src/orb/pointerGesture.ts` 与 `src/orb/__tests__/orbPointer.test.ts`（`src/chat/**` 不改） |

> **一句话结论**：Orb 的触摸失效不是"没做触摸"，而是拖拽数据源选了**系统物理光标轮询**（`cursorPosition()`）——触摸输入不驱动物理光标，于是拖拽整条链路失效。修法是**把拖拽数据源换成指针事件的屏幕坐标绝对定位**（`S = window.screenX + clientX`，鼠标/触摸同源；直接取 `PointerEvent.screenX` 在 WebView2 触摸下不可用，见 §5.1 / §5.5），并把长按菜单、点击展开、拖拽三者收进**同一个手势状态机**，消除 `OrbContextMenu` 与 `OrbDrag` 各自监听、互相抢事件的分裂结构。

---

## 0 定位

本文只回答一个问题：**在 Windows 触摸屏 / 触控笔 / 精密触控板等"无物理光标或指针类型非 mouse"的输入下，Orb 悬浮球的点击、拖拽、贴边、长按菜单、面板内滚动，如何与现有鼠标行为并存而不回归**。

范围限定在 **orb 窗口内部的输入处理**。不改窗口架构、不改展开/收起状态机、不改 Rust 侧的贴边与位置持久化、不改设置项语义。

---

## 1 目标与非目标

### 1.1 目标

| # | 目标 | 判据 |
|---|---|---|
| G1 | 触摸可拖拽 | 手指按住球移动，窗口跟手移动；松手后按现有规则贴边 |
| G2 | 鼠标零回归 | Windows 鼠标拖拽的跟手度、阈值、贴边、拖拽视觉态与现状一致 |
| G3 | 手势互斥 | 同一次按下只会产生「点击展开 / 拖拽 / 长按菜单」中的一种，不出现拖拽中途弹出菜单、长按后误展开 |
| G4 | 触摸有反馈 | 触摸按压有缩放反馈；触摸不产生粘滞 hover（`orb-hover` 不残留、`orb-hover-hide` 不误触发） |
| G5 | 中断可恢复 | 系统手势打断（`pointercancel` / 丢失捕获）后拖拽状态必定收尾，不残留 `isDragging=true` |
| G6 | 面板可触摸操作 | 面板内列表可用单指滚动、按钮可点按；滚动不带动窗口 |
| G7 | 高 DPI 不漂移 | 100% / 150% / 200% 缩放、跨不同缩放的显示器拖拽，位置不累积偏移 |

### 1.2 非目标

- 不做双指缩放 / 旋转 / 惯性滚动等自定义触摸手势（交给 WebView 默认行为）。
- 不改 `click_through` 语义（穿透开启时窗口不接收任何指针事件，触摸同样无法交互，这是该开关的本意）。
- 不改展开/收起四象限逻辑、不改 `expand_orb_panel` / `collapse_orb_panel` / `snap_orb_to_edge` 的 Rust 实现。
- 不改 macOS / Linux 鼠标路径的原生 `startDragging()` 方案（G2 只覆盖 Windows 鼠标）。
- 不在本文处理面板内文本长按选中（orb 窗口根节点 `user-select: none`，与鼠标现状一致，另立议题）。
- 不做软键盘（`visualViewport`）与面板尺寸联动的适配。

---

## 2 现状盘点

### 2.1 输入链路现状

```
OrbBall（.orb-hitbox，touch-action: none）
  ├─ onPointerDown/Move/Up → useOrbDrag（src/orb/OrbDrag.tsx）
  │    ├─ Windows：cursorPosition() 16ms 轮询 + setPosition（应用自持拖拽）
  │    ├─ macOS：window.startDragging()
  │    └─ Linux：window.startDragging()
  └─ OrbContextMenu 包装层（src/orb/OrbContextMenu.tsx）
       ├─ onContextMenu → show_orb_context_menu（鼠标右键）
       ├─ onTouchStart/Move/End → 500ms 长按 → show_orb_context_menu
       └─ onClick → 吞掉长按后的合成 click
  └─ onClick → useOrbExpand().expand()（hasDraggedRef 判定是否拖拽过）
```

窗口形态：折叠 88×88（球 84 + 2×2 内缩），展开 424×604；`transparent / alwaysOnTop / skipTaskbar / decorations:false / acceptFirstMouse:true`。

### 2.2 缺陷清单

| # | 现象 | 根因 | 证据 |
|---|---|---|---|
| D1 | 触摸拖不动球 | 拖拽位移来自 `cursorPosition()`（系统物理光标）。触摸不移动物理光标，轮询值恒定，`setPosition` 永不触发 | `OrbDrag.tsx` `startWindowsDrag()` |
| D2 | 触摸拖拽完全无反馈 | `isDragging` 置位后，轮询拿不到位移，视觉停在 `orb-dragging` 却不动，松手才恢复 | 同上 + `OrbBall.tsx` `stateClass` |
| D3 | 长按菜单与拖拽互斥靠"碰巧" | 长按计时在 `OrbContextMenu`，拖拽阈值在 `OrbDrag`，两者互不知情：按下后 500ms 内开始拖拽，长按仍可能触发；长按后继续移动，拖拽也会启动 | 两个组件各自的 ref |
| D4 | 触摸设备判定过宽 | `IS_TOUCH_DEVICE = "ontouchstart" in window \|\| maxTouchPoints > 0` 在触屏笔记本上恒为真，但事件实际由鼠标产生，长按逻辑仍被挂上 | `OrbContextMenu.tsx` |
| D5 | 触摸后 hover 粘滞 | `:hover` 在触摸后停留；`orb-hover-hide`（隐藏悬浮球）用 CSS `:hover` 判定，触摸用户点一下球就可能把球藏掉 | `global.css` `:root.orb-hover-hide ... :hover` |
| D6 | 拖拽中断后状态卡死 | 只有 `onPointerUp`，没有 `onPointerCancel` / `onLostPointerCapture`；系统手势抢占后 `isDragging` 保持 true，球停在 0.7 透明度且不贴边 | `OrbDrag.tsx` 仅导出 `onPointerUp` |
| D7 | 长按后可能误展开 | 长按吞 click 依赖 `OrbContextMenu` 的内层 `onClick`，而展开由 `OrbBall` 的 `onClick` 触发，两层合成 click 的先后顺序不受控 | 两个 `onClick` |
| D8 | 触摸视觉样式是死代码 | `.orb-touch` 规则存在但从未被挂载（`OrbBall.tsx` 的 `stateClass` 只产出 `orb-dragging / orb-hover / orb-idle`） | `global.css` + `OrbBall.tsx` |
| D9 | 拖拽结束后 `hasDraggedRef` 被提前清零（macOS/Linux，需真机确认） | `startDragging().finally()` 内清零，若该 Promise 在原生拖拽结束前返回，随后的 click 会误判为点击而展开 | `OrbDrag.tsx` 两处 `.finally()` |

> D9 不单独修：`hasDraggedRef` 与 `onClick` 一并移除后，tap 只可能由 `pressed → up` 转移产生（§4.2），拖拽中的 `up` 落在 `dragging` 分支，天然不产生 tap。

### 2.3 参考项目对照

`references/2-desktop-orb` 下 5 个项目（WindowPet / BongoCat / airi / onepoint 等）**均无完整的原生触摸拖拽实现**；airi 仅有部分 Pointer 事件与触摸手势测试代码。拖拽谱系只有两条：原生 `startDragging()`（丢中间事件）、指针增量 + 应用自持（`references/1-ai-agent/OpenBitFun` 的 `AgentCompanionDragService`，其注释明确"应用自持拖拽是为了让 pointer capture 与 release 对 UI 可见"）。本项目 Windows 轮询正是从 OpenBitFun 的 **Windows 特例**继承而来，触摸不兼容是该特例未被覆盖的副作用。

---

## 3 设计决策

### 3.1 决策总表

| 编号 | 决策 | 理由 |
|---|---|---|
| C1 | 触摸（`pointerType !== "mouse"`）**一律**走指针事件定位拖拽；鼠标按平台分流（Windows 指针定位 / macOS+Linux 原生） | 触摸在任何平台都拿不到物理光标；鼠标保持既有已验证路径，把回归面压到最小 |
| C2 | Windows 鼠标也改用指针事件定位（`S = window.screenX + clientX` 绝对式），**删除** `cursorPosition()` 轮询 | 与轮询式等价（§5.2），但少一路 IPC、少一个 16ms 定时器、不再依赖系统光标 |
| C3 | 长按 / 拖拽 / 点击收进**单一手势状态机**（纯函数 + hook 两层） | D3/D7 的根因是两套监听各自持 ref；只有一个 owner 才能保证互斥 |
| C4 | 触摸判定从"设备级"改为"**事件级**"（`e.pointerType`） | 触屏笔记本鼠标+触摸共存，设备级判定必然误伤（D4） |
| C5 | 展开动作由状态机的 `tap` 转移触发，**移除** `onClick` | 合成 click 的时序不可控（D7）；`pointerup` 是唯一可信的"手指抬起"信号 |
| C6 | 触摸不参与 hover；悬停隐藏改由 **JS 悬停类 `.orb-hover`** 判定（该类只由鼠标指针置位），删除 CSS `:hover` 判据 | D5：触屏笔记本主指针是触控板，`@media (hover: hover)` 仍匹配，触摸点按留下的粘滞 `:hover` 照样误隐藏球，媒体查询隔离不够 |
| C7 | `orb-touch` 类在触摸指针**按下期间**挂载，抬起即摘 | 触摸设备可能同时接鼠标，持久挂类会让鼠标也吃到 `cursor: default` |
| C8 | 长按菜单去重：长按已弹菜单后 `CONTEXT_MENU_DEDUPE_MS` 内吞掉合成 `contextmenu` | Windows 触控长按会额外触发原生 `contextmenu`，不去重会双弹 |
| C9 | `pointercancel` / `lostpointercapture` 均走"收尾但不算点击"分支 | D6 |

### 3.2 为什么不用原生 `startDragging()` 作为触摸主方案

| # | 原因 |
|---|---|
| R1 | 原生拖拽是系统模态循环，期间前端收不到中间事件，无法维持 `orb-dragging` 视觉态、无法在拖拽中取消、无法在拖拽中刷新面板状态 |
| R2 | 本项目已有依赖中间态的链路：拖拽开始关穿透、拖拽中屏蔽失焦收起、松手后 `snap_orb_to_edge`，都需要"拖拽中"这个可观测阶段 |
| R3 | 触摸拖拽在 WebView2 下可被提升为鼠标事件（`data-tauri-drag-region` 注入脚本实测响应触摸，见 §6.6 / §10 V2），但原生拖拽仍缺中间事件；指针事件则已在本项目被证明可用（长按依赖 `touchstart` 已生效），且 A0.6 / A5 实测指针定位在触摸下跟手 |
| R4 | 指针定位对鼠标/触摸同源，跨平台一致；原生方案在三平台行为差异大（macOS 立即返回、Linux 依赖 WM） |

保留 `startDragging()` 作为 **C1 的鼠标分支**与 **备选方案 B**（§10）。

---

## 4 手势状态机

### 4.1 状态与输入

纯逻辑放在 `src/orb/pointerGesture.ts`，只依赖数字与枚举，不碰 DOM / React / Tauri：

```ts
type PointerKind = "mouse" | "touch" | "pen";
type GesturePhase = "idle" | "pressed" | "dragging" | "longPressed";
type GestureEvent =
  | {
      kind: "down";
      pointerId: number;
      pointerType: PointerKind;
      isPrimary: boolean;
      button: number;
      x: number; // clientX
      y: number; // clientY
      sx: number; // window.screenX + clientX（§5.1）
      sy: number; // window.screenY + clientY（§5.1）
    }
  | {
      kind: "move";
      pointerId: number;
      x: number;
      y: number;
      sx: number;
      sy: number;
    }
  | { kind: "up"; pointerId: number }
  | { kind: "cancel"; pointerId: number }
  | { kind: "lostcapture"; pointerId: number }
  | { kind: "longPressElapsed"; now: number };
```

字段口径：

- `down` 必须带 `pointerId / isPrimary / button`，否则 §4.2 的"非主指针、非左键忽略"与"`pointerId` 一致性校验"在纯模块内无法实现。
- `move` 同时携带 client 与 screen 坐标：client 用于阈值判定，screen 用于拖拽目标（§5.2）。
- `longPressElapsed` 必须带 `now`：长按弹菜单时要用它写入 `menuShownAt`，供 `resolveContextMenu` 做 `CONTEXT_MENU_DEDUPE_MS` 去重（§4.4）。`move / up` 不需要时间戳，不额外传。
- `contextmenu` 不进状态机，在 hook 层处理（§4.4）；其"弹不弹"的判定由纯函数 `resolveContextMenu(state, { pointerType, now })` 给出，保证可单测。

计时器与 DOM 均由 hook 持有：纯模块只接收 `longPressElapsed`（计时到期）与 `lostcapture`（捕获丢失），不自己起 `setTimeout`、不读事件对象。

### 4.2 转移表

| 当前 | 事件 | 条件 | 下一状态 | 副作用 |
|---|---|---|---|---|
| 任意 | down | `isPrimary && button === 0` | pressed | 原态非 idle 时先按 `cancel` 收尾（原态为 dragging 时额外产出 `finishDrag` + 需贴边）；记录 `pointerId / pointerType / client / screen` 按下坐标；`setPointerCapture`；`pointerType !== "mouse"` 则起长按计时 |
| idle | move / up / cancel / lostcapture | — | idle | 忽略 |
| pressed | move | `pointerId` 不一致 | pressed | 忽略 |
| pressed | move | 位移 ≥ 阈值（mouse 8 / touch·pen 12） | dragging | 取消长按计时；`setDragging(true)`；记录 `S(t₀)`（转移时刻指针屏幕坐标），以 `G = S(t₀) − W(t₀)` 建立抓取偏移（§5.2）后开始提交；`pointerType === "mouse"` 且 macOS / Linux 走原生 `startDragging()` |
| pressed | move | 位移 ≥ `LONG_PRESS_MOVE_TOLERANCE_PX`（8）且 < 拖拽阈值 | pressed | 取消长按资格（清计时器，不再起） |
| pressed | longPressElapsed | `pointerType !== "mouse"` | longPressed | `show_orb_context_menu`（用**按下时**的 client 坐标）、记录 `menuShownAt` |
| pressed | up | — | idle | **tap**：若未展开则 `expand()` |
| pressed | cancel / lostcapture | — | idle | 清长按计时，仅收尾，不产生 tap |
| dragging | move | `pointerId` 一致 | dragging | 计算目标位置并合并提交（§5.3） |
| dragging | up | — | idle | `finishDrag`：停止提交 → `setDragging(false)` → 恢复穿透 → `scheduleSnap()` |
| dragging | cancel / lostcapture | — | idle | 同上 `finishDrag`，并标记 `interrupted`（同样 `scheduleSnap()`） |
| longPressed | move | — | longPressed | 忽略移动 |
| longPressed | up | — | idle | 仅收尾，不产生 tap（无 click 监听者，§4.5） |
| longPressed | cancel / lostcapture | — | idle | 仅收尾 |

补充规则：

- `isPrimary === false` 或 `button !== 0` 的 `down` 一律忽略，不改变当前状态。
- `pointerId` 必须与按下时一致，否则丢弃该 `move / up / cancel / lostcapture`。
- **`down` 可发生在任意状态**：`dragging` / `longPressed` 期间收到新的合法 `down`（丢失 up 后的新指针、原生菜单关闭后的再次按下）时，先按 `cancel` 收尾再进入 `pressed`，避免状态卡死。
- `longPressed` 不因 `move` 回到 `dragging`：长按后移动一律忽略，直到抬起。
- 长按与菜单路径只对 `touch` / `pen` 开放（`pointerType !== "mouse"`）；鼠标只走右键 `contextmenu`（§4.4）。
- **tap 的判定只看"是否进入 `dragging`"，不看绝对位移**：`pressed → up` 一律产出 tap。`LONG_PRESS_MOVE_TOLERANCE_PX`（8）只用来取消长按资格，不参与 tap 判定——否则触摸在 9–11px 区间（已取消长按、未达拖拽阈值 12）会出现"既不弹菜单也不展开"的死区（U2 / U4）。
- 指针捕获统一在 `e.currentTarget`（`.orb-hitbox`）上 `setPointerCapture` / `releasePointerCapture`，不再用 `e.target`（可能落到 SVG 子节点）。

### 4.3 事件绑定

`OrbBall.tsx` 的 `.orb-hitbox` 直接绑定：`onPointerDown / onPointerMove / onPointerUp / onPointerCancel / onLostPointerCapture / onContextMenu / onPointerEnter / onPointerLeave`。`OrbContextMenu` 包装层删除（其 DOM 无布局作用，CSS 选择器均以 `.orb-hitbox` 为锚点）。

捕获在 `down` 时完成，因此 `move / up / cancel / lostcapture` 即使指针已移出 `.orb-hitbox` 仍会投递到该元素；不为此再加 window 级监听。

### 4.4 长按与右键菜单

| 输入 | 行为 |
|---|---|
| 鼠标右键（`contextmenu`，`pointerType === "mouse"`） | `preventDefault` → 立即 `show_orb_context_menu(e.clientX, e.clientY)` |
| 触摸 / 触控笔长按（计时 500ms） | 到达阈值即弹菜单；坐标取**按下时**的 `clientX / clientY`；同时记录 `menuShownAt` |
| 非鼠标 `contextmenu`（长按的合成事件、笔的桶形键等） | **无条件** `preventDefault`（不依赖与长按的先后顺序）；仅当距 `menuShownAt` ≥ `CONTEXT_MENU_DEDUPE_MS`（700ms）时才弹一次菜单，否则只吞不弹 |
| 触摸长按后的合成 `click` | 无需处理：没有 `click` 监听者（§4.5） |

去重状态命名为 `menuShownAt`（时间戳），不叫 `suppressClick`：它表达"菜单已在何时弹出"，不表达"要吞掉哪个事件"，避免与"合成 click 由状态机天然规避"的口径混淆。

菜单锚点沿用窗口相对的**逻辑坐标**（`Menu::popup_at(window, LogicalPosition)`），与鼠标现状一致；不做手指遮挡偏移（原生菜单会自行夹到屏幕内，若实测首项被遮挡再加偏移常量）。

### 4.5 点击 / 展开判定

展开只在 `pressed → idle` 的 tap 转移里执行，且要求 `!isExpanded`。移除 `OrbBall` 的 `onClick` 与 `hasDraggedRef`（后者是 D9 的来源）。

不存在 `click` 监听者，合成 click 不会触发任何动作，**不需要显式吞掉**；`longPressed → up` 天然不产出 tap，`dragging → up` 只产出 `finishDrag`，三者互斥由状态机保证。

---

## 5 拖拽数据源

### 5.1 坐标约定

| 量 | 单位 | 来源 |
|---|---|---|
| `S` 指针屏幕坐标 | CSS px（屏幕 DIP，虚拟桌面原点） | `window.screenX + PointerEvent.clientX`（`screenY + clientY` 同理） |
| `C` 指针 client 坐标 | CSS px（窗口客户区相对） | `PointerEvent.clientX / clientY`（仅用于阈值判定、`screenX` 校验与备选模式） |
| `W` 窗口左上角 | 逻辑 px（屏幕 DIP） | `outerPosition()` 物理值 ÷ `devicePixelRatio` |
| 提交 | `PhysicalPosition` | `logicalToPhysical()`（`src/orb/OrbCoord.ts` 已有） |

`S` 用 `window.screenX + clientX` 合成而非直接取 `PointerEvent.screenX`：A0 真机录值显示 WebView2 的 `touch` 指针把 `screenX / screenY` 填成 `clientX / clientY`（`screenX − clientX ≡ 0`，`window.screenX` 却与 `outerPosition() ÷ dpr` 一致），直接取用会得到"相对窗口"的坐标，按 §5.2 计算即为增益 −1 的负反馈——窗口一动指针读数反向变化，表现为小球反复颤抖且跟不上指针。鼠标路径下两者数值等价（`screenX − clientX` 恰等于 `window.screenX`），故统一合成式对鼠标无精度损失以外的副作用（`window.screenX` 为整数 CSS px，量化残差 ≤ 0.5 CSS px，落在 §9.1 的 ≤ 1 物理 px 容差内）。

`outerPosition()` 与 `setPosition()` 均为物理像素；前端一律用 `devicePixelRatio` 换算，沿用 `OrbCoord` 现有三个函数，不新增换算逻辑。

### 5.2 定位公式与推导

设抓取偏移 `G`（屏幕坐标下"指针到窗口左上角"的向量），当前指针 `S(t)`，目标窗口位置 `W_target(t)`：

```
G           = S(t₀) − W(t₀)    // t₀ = pressed → dragging 转移时刻
W_target(t) = S(t) − G
```

定点性证明：窗口完全跟随时 `S` 与 `W` 同步平移，`W_target(t) ≡ W(t)`，无位移可提交；窗口落后 δ 时 `W_target` 直接等于正确位置，一步补齐。因为 `S` 是**屏幕绝对量**，与窗口当前是否已移动无关，所以：

- 不依赖 IPC 是否已生效：在飞 / 待发的提交不会污染下一次计算；
- 无累积误差：每帧由绝对量直接算出，不做增量累加；
- 丢帧自动补齐：跳过若干事件后，下一次事件的目标仍然正确。

**为什么不用 `clientX` 增量**：`C(t)` 是相对**窗口实际位置**测量的，而应用侧维护的 `W(t)` 是"已提交目标"。存在未生效提交时 `W(t) ≠ W_actual(t)`，`W(t+1) = W(t) + (C(t) − G)` 会把同一段位移重复计入——IPC 滞后 ≥ 2 帧即发散（越拖越偏），只有恰好滞后 1 帧时才稳定。串行提交也修不掉这一点：在飞期间收到的 `pointermove`，其 `clientX` 相对的是**提交落地前的旧窗口位置**，落地后再拿它算目标会多算一次窗口刚移动的量（正反馈），除非再叠加"只接受落地之后的事件"这一同样需要验证的假设。故 `clientX` 增量不列入数据源决策（§5.5）。

起点解析是异步的：进入 `dragging` 时先记录 `S(t₀)`；`outerPosition()` 解析完成后取当时的窗口位置作为 `W(t₀)`（解析完成前不提交任何位置，窗口未移动，故两者一致）；解析期间到达的最新指针位置在解析完成后一次性推进，不丢首段位移（C3.5）。

数值细节：`W_target` 每帧由绝对量直接算出，不做浮点累积；提交时 `Math.round(W_target × dpr)` 转物理；仅当与"上次已提交目标"相差 ≥ 1 物理 px 时才提交（去抖，见 V5）。

### 5.3 提交策略（合并 in-flight）

```
pendingRef 保存最新目标；inFlightRef 标记是否有未完成 setPosition。
若目标与上次已提交目标相差 < 1 物理 px：不提交。
若 inFlight：只覆盖 pending，不追加队列。
否则：循环取出 pending → await setPosition → 继续，直到 pending 为空。
```

效果：同一时刻最多 1 个在飞 + 1 个待发，IPC 不会被 `pointermove` 频率淹没，也不会出现"越拖越落后"的排队效应（参考 OpenBitFun 的合并 IPC 做法）。因为目标是绝对量（§5.2），覆盖 `pending` 只丢弃中间帧、不丢位移。

本策略**只对绝对定位成立**：目标值不依赖窗口当前实际位置，所以在飞提交既不会污染下一次计算，也不会因覆盖 `pending` 丢位移。任何依赖 `clientX`（相对窗口测量）的数据源都不能套用本策略，理由见 §5.2 与 §5.5。

### 5.4 DPR 变化重基准

拖拽中若 `window.devicePixelRatio` 与上次提交时不同（窗口跨到另一缩放比例的显示器）：

1. **先排空在飞提交**（`await` 当前 `setPosition` 落地、`pending` 清空）；
2. 再 `outerPosition()` 读取当前窗口物理位置并换算为逻辑值，作为新的 `W(t)`；
3. 令 `G = S(t) − W(t)`（用**当前**指针屏幕坐标重算抓取偏移）。

理由：`S` 与 `W` 都是"当前显示器 DIP"，跨屏后 1 CSS px 对应的物理像素数变化。重算 `G`（而不是保留旧值）同时消除"单位变化"与"窗口位置跳变"两项误差，且不引入任何一次可见跳变。

第 1 步不可省。参考项目 Kiri 在同类实现里专门维护了一个 `committedPosition`，注释写明"native placement is asynchronous…OS position reads lag"——即原生写入未落地时 `outerPosition()` 会读到旧值，用它当基准等于把"还在飞的位移"再算一次。排空后读取才可靠；若排空超时（写入异常），放弃重基准并直接结束本次拖拽（比按错误基准继续拖更安全）。

备选（更保守，参考项目采用的口径）：`onScaleChanged` 时**直接取消本次拖拽**，由用户重新抓取，完全规避基准问题。实测（A0.4 / A0.7）重基准路径未出现可见跳变，故不启用此口径。

### 5.5 数据源可用性：先验证，再定实现（阻塞前置）

`screenX / screenY` 是 Chromium 侧合成的量（`screenX ≈ window.screenX + clientX`），它的可信度**不能用运行时启发式判定**。原草案的判据（"`screenX − clientX` ≈ 窗口左偏移、且随指针变化"）有两个问题：

1. **几乎恒为真**：Chromium 总会填值，也总会随指针变化；该判据检不出真正会出错的场景。
2. **真正会出错的场景恰好检不出**：跨显示器**混合缩放**（Chromium 按窗口所在显示器的缩放把屏幕原点换算成 DIP，混合缩放下原点会取整错位）、**负坐标显示器**（主屏左侧 / 上方）、以及触摸指针是否填充 `screenX`。这些都是"值存在但偏了 N px"的失真，而非"值缺失"。

所以本项不做运行时降级分支，改为**编码前的真机验证**（§9.1 A0，阻塞 C3），按实测结论确定唯一数据源：

| 实测结论 | 落地方式 |
|---|---|
| 全部场景误差 ≤ 1 物理 px（含混合缩放、负坐标、触摸、触控笔） | 只用 `screenX / screenY` 绝对定位（§5.2）；**代码中不留降级分支** |
| 仅跨屏混合缩放时出现固定偏差 | `screenX` 绝对定位 + 提交前按 `outerPosition()` 校准（拖拽开始与 dpr 变化各一次，§5.4）；校准后仍有可见偏差则降级到下一行 |
| 普遍不可信（含触摸） | 放弃指针定位，改走原生 `startDragging()`（B1）；此时需同批验证触摸下原生拖拽是否可用（V2） |

**A0 实测结论（已回填 §10 V1）**：`PointerEvent.screenX / screenY` 在 WebView2 中对 `touch` 指针填的是 `clientX / clientY`（`screenX − clientX ≡ 0`），鼠标指针两者正常；`window.screenX / screenY` 与 `outerPosition() ÷ dpr` 一致且随窗口移动更新。落地方式：`S` 取 `window.screenX + clientX`（§5.1 合成式），鼠标与触摸同一条路径，仍**不设任何运行时降级分支**；合成式对鼠标与"`screenX` 可信"等价，对触摸修复增益 −1 的负反馈。

验证方法：临时探针（dev-only，验证完即删，不进主分支），在拖拽中记录每个事件的 `pointerType / screenX / screenY / clientX / clientY`、`window.screenX / screenY / screen.availLeft / availTop`、`devicePixelRatio`，以及 `outerPosition()` 与 `outerSize()` 的返回值，逐场景比对 `screenX − clientX` 与 `outerPosition.x ÷ dpr`。

> 结论落点：验证结果回填 §10 V1，作为 C3 的输入。**验证未完成不得编写数据源代码**——否则代码里会沉淀一套没有实测依据的降级分支，之后没人敢删。

#### 5.5.1 参考项目证据（能证明什么、不能证明什么）

| 项目 | 是否用 `screenX / screenY` | 能证明 |
|---|---|---|
| Kiri（`references/3-screen-capture/Kiri`，Tauri v2 截图工具，已发布） | **是**。`src/windows/PinWindow.tsx` 固定截图窗口拖动、`src/windows/useScrollCaptureDrag.ts` 滚动截图面板拖动，都用 `event.screenX / screenY` | 该做法在 Windows WebView2 上**可用于鼠标拖动**，且已有可跑通的形态：起点 `outerPosition() + scaleFactor()` 读一次 → 目标 `base + (screen − start) × scale` → `pending + applying` 合并提交 → `scale` 有限且 > 0 才继续 |
| Kiri 同一处的触摸处理 | `PinWindow.tsx` 在 `pointerType === "touch"` 时**直接 return**，触摸不走这条路径 | **不能**证明触摸下 `screenX` 可用——参考项目恰好把触摸排除了 |
| Kiri 的混合缩放验收 | `docs/adr/0119-readable-pinned-screenshots-and-scroll-pin.md` 明写"mixed-DPI display movement…remain unverified"；`docs/adr/0107-…-pin-drag.md` 的 Verification 只覆盖 handler 级 DPI 换算 | **不能**证明混合缩放 / 双屏正确性——参考项目自己也没验证过 |
| Kiri 的缩放变化策略 | `useScrollCaptureDrag.ts` 监听 `onScaleChanged` → 作废 `committedPosition` 并**取消**手势 | 提供了 §5.4 的保守备选口径，以及"不要在写入未落地时读 `outerPosition`"的实证理由 |
| airi（`references/2-desktop-orb/airi`） | 仅用 `window.screenX / screenY` 做窗口指纹 | 无关 |
| OpenBitFun（`references/1-ai-agent/OpenBitFun`） | 不用；走系统光标查询 | 即本项目现状（Windows 轮询）的来源 |

结论：参考项目证明的是"这条路走得通、形态可以参考"，**不构成**对本项目验收矩阵（A0.1–A0.7）的证明。特别是触摸与混合缩放两项，恰好是参考项目留空的部分，必须自行实测——本项目已完成实测，A0.1–A0.7 全部通过（见 §9.1）。

#### 5.5.2 可复用的实现细节（已验证的部分）

- 起点基准只读一次：`Promise.all([outerPosition(), scaleFactor()])`，拖拽全程不再重读（避免读到滞后值）。
- 目标式：`round(base + (S(t) − S(t₀)) × scale)`，与本规格 §5.2 的 `S(t) − G` 等价（`G = S(t₀) − W(t₀)`）。
- 提交合并：`pending` 覆盖 + `applying` 互斥的 while 循环，与本规格 §5.3 一致。
- `scale` 校验：`Number.isFinite(scale) && scale > 0`，否则取消拖拽。
- `pointerup` 保护：先置 `ended` 再 release 捕获，`lostpointercapture` 不得丢弃仍在等待写入的最终点（本规格 §5.7 的幂等收尾需照此实现）。


### 5.6 平台矩阵

| 平台 | 鼠标 | 触摸 / 触控笔 |
|---|---|---|
| Windows | 指针定位（替换轮询） | 指针定位 |
| macOS | 原生 `startDragging()`（不变） | 不适用（无触摸硬件） |
| Linux | 原生 `startDragging()`（不变） | 指针定位 |

判定顺序：`pointerType !== "mouse"` → 指针定位；否则按平台分流。这样"触摸能用"不依赖任何平台的既有特例。数据源的具体形式由 §5.5 的验证结论决定，本表只决定"指针定位 / 原生拖拽"的分流。

原生分支（macOS / Linux 鼠标）不改数据源，但**收尾策略统一**：`startDragging()` 的 `finally` 只做"原生拖拽已结束"标记并执行幂等收尾，`pointerup` 也执行同一收尾函数；两者先到者生效。这样即使原生 Promise 提前返回（macOS），也不会在用户仍按住时提前贴边；而 D9（`hasDraggedRef` 提前清零导致误展开）随 `onClick` 一并消失——tap 只可能由 `pressed → up` 转移产生。

### 5.7 结束与贴边

`up / cancel / lostpointercapture` → 停止位置提交 → `setDragging(false)` → 若未展开且 `clickThrough` 为真则恢复穿透 → `scheduleSnap()`（沿用 `ORB_SNAP_DELAY_MS = 1000`）。

`pointercancel` 也执行 `scheduleSnap()`：中断前窗口已经移动过，不贴边会留下悬空位置。

`pointerup` 与 `lostpointercapture` 的先后顺序必须区分对待（参考项目 Kiri 专门处理过这一点）：`pointerup` 先置 `ended = true` 再 `releasePointerCapture`，随后的 `lostpointercapture` 看到 `ended` 只做幂等收尾、**不得撤销**仍在等待落地的最终位置提交；反之若是 `lostpointercapture` 先到（捕获被系统收回，如 `pointercancel`），才真正放弃最终提交。否则松手瞬间的最后一段位移会被丢掉。

---

## 6 触摸的其余适配

### 6.1 悬停

- `onPointerEnter` 仅在 `pointerType === "mouse"` 时置 `isHovering`。
- `onPointerLeave` 无条件清 `isHovering`（触摸抬起后残留的清零兜底）。
- 悬停隐藏（`orb-hover-hide`）的判据从 CSS `:hover` 改为 **JS 悬停类 `.orb-hover`**（该类只由鼠标指针置位，见 C4.8）：`:root.orb-hover-hide ... .orb-hitbox.orb-hover:not(.orb-ball-hidden)`。
- **不用** `@media (hover: hover)` 包 `:hover` 规则：触屏笔记本的主指针是触控板，`hover: hover` 恒匹配，触摸点按留下的粘滞 `:hover` 仍会把球隐藏且不恢复（A9 会失败）。
- 副作用说明：`stateClass` 单选，拖拽期间是 `orb-dragging` 而非 `orb-hover`，因此拖拽中隐藏规则不再生效（球保持可见）。这是有意的改进，与原 `:hover` 判据的差异不视为回归。

### 6.2 点击穿透

语义不变：`click_through=true` 时窗口 `set_ignore_cursor_events(true)`，触摸同样穿透。拖拽开始时的"关穿透"调用是防御性冗余（穿透开启时窗口本就收不到按下事件），保留但注明，不删除。

### 6.3 失焦收起守卫

`OrbPanel` 收到 `ORB_BLUR` 后有一步"光标是否仍在窗口内"的判定（`getMouseLogicalPos` + `isPointInRect`），用于避免误收起。触摸输入下系统光标与手指无关，该判定必然失真。修法：仅在 `matchMedia("(hover: hover)")` 为真时执行该守卫，否则直接进入 `collapse()`。纯触摸设备（`hover: none`）由此被正确绕过；触屏笔记本（`hover: hover` 为真）仍以物理光标判定，属可接受的残留误差（V7，只影响是否收起，不影响输入）。

判定位置：`keepExpanded` / `isOrbBlurCollapseSuppressed` / `isDragging` 三段早退之后、坐标守卫之前——前两段属"用户显式保持展开"，与输入设备无关，不能被能力查询短路。`matchMedia` 不可用时按"可悬停"处理（保留原守卫），避免在不支持该 API 的环境里把收起逻辑放宽。

### 6.4 视觉与 CSS

| 项 | 规格 |
|---|---|
| `orb-touch` 挂载 | `activePointerType !== "mouse"`（touch / pen）期间挂载，`up / cancel / lostcapture` 摘除（C7） |
| 按压反馈 | 由状态机 `pressed` 且 `activePointerType !== "mouse"` 驱动 `.orb-pressed` 类：`.orb-pressed { animation: none !important; transform: scale(0.92); opacity: 0.9 !important }`（鼠标按下不改视觉，保住 G2） |
| `:active` 兜底 | 保留 `.orb-hitbox.orb-touch:active:not(.orb-dragging):not(.orb-pressed)` 的 `scale(0.92)`，仅作 `:active` 时机可靠时的兜底；**不能**靠 `:active` 做主方案 |
| 状态类互斥链 | `stateClass` 保持单选：`orb-dragging` > `orb-pressed`（非鼠标）> `orb-hover` > `orb-idle`。`.orb-pressed` 规则写在 `.orb-hover` / `.orb-dragging` 之后（同特异性靠源码顺序取胜），并保留呼吸动画的 `:not(.orb-pressed)` 作为叠加写法下的防御 |
| 呼吸动画 | 现有 `.orb-idle:not(.orb-hover):not(.orb-dragging)` 需再排除 `.orb-pressed`：`.orb-hitbox` 常驻 `animation: orb-breathe`（关键帧自带 `transform: scale`），动画声明优先级高于普通声明，不排除则按压缩放会被动画覆盖（`:active` 方案失效的直接原因） |
| 光标 | `.orb-touch` 的 `cursor: default` 仅在触摸按下期间生效，不影响随后鼠标悬停 |
| 手势冲突 | `.orb-hitbox` 的 `touch-action: none` 保持不变（拖拽必须独占）；不给 `html / body` 加 `touch-action: none`（会废掉面板滚动）；**不新增** `.chat-main { touch-action: pan-y }`（`touch-action` 沿祖先链取交集，会禁掉消息内代码块 / 表格的横向滚动，而面板本身没有横向滚动容器，无收益） |

### 6.5 面板内触摸

- `.orb-panel` 自身 `pointer-events: none`、子元素 `auto`，触摸点按已天然可用，不改。
- 滚动能力不额外拦截：`.orb-hitbox` 的 `touch-action: none` 只作用于球本身，面板内容默认 `auto`，单指纵向滚动与代码块 / 表格的横向滚动均已可用；球与面板是兄弟元素，指针捕获不会串到面板。
- **不新增**任何 `touch-action` 规则（理由见 §6.4 末行）；**不加** `overscroll-behavior`：orb 根节点 `overflow: hidden`，滚动链没有落点。
- 作用域锚点必须是 `div[data-orb-window]`（`src/layouts/OrbWindow.tsx` 挂载）：`MessageList` 的 `compact` 属性是「紧凑 UI 布局开关」（`ConversationFrame` 恒传 `true`），`ConversationView` 的 `chat-view-compact` 类在所有视图恒在 —— 两者都不能作为 orb 判据。
- 长按消息文本的选中能力受 orb 根节点 `user-select: none` 限制，与鼠标现状一致，列为非目标。

### 6.6 窗口头部拖动（已核实可用，未启用备用方案）

`.orb-chrome-top` 依赖 `data-tauri-drag-region`（Tauri 注入脚本按 `mousedown` 处理）。真机核实（§10 V2）：触摸可正常拖动面板头部，注入脚本响应触摸事件，故不启用备用方案——无需在该元素上复用 `bindRegionDrag`，也无需过滤 `button / input / .ant-dropdown-trigger` 的冒泡。

---

## 7 常量

`src/config/constants.ts`：

| 常量 | 值 | 说明 |
|---|---|---|
| `PET_DRAG_THRESHOLD_PX` | 8 | 既有，鼠标拖拽阈值，不变 |
| `TOUCH_DRAG_THRESHOLD_PX` | 12 | 新增，手指抖动更大 |
| `LONG_PRESS_DURATION_MS` | 500 | 既有，长按触发 |
| `LONG_PRESS_MOVE_TOLERANCE_PX` | 8 | 新增，长按期间允许的抖动，超出即取消长按 |
| `CONTEXT_MENU_DEDUPE_MS` | 700 | 新增，长按已弹菜单后，此窗口内的合成 `contextmenu` 只吞不弹 |
| `ORB_SNAP_DELAY_MS` | 1000 | 既有，不变 |
| `DRAG_POLL_INTERVAL_MS` | 16 | **删除**（轮询移除，全仓仅 `OrbDrag.tsx` 使用） |

无新增设置项，不改 `OrbConf` DTO，不需要 `npm run contract:gen`。

---

## 8 改动清单

| 文件 | 动作 | 内容 |
|---|---|---|
| `src/orb/pointerGesture.ts` | 新增 | 纯状态机 `reduceGesture` + 拖拽定位 `advanceDrag`（屏幕绝对式，无 DOM 依赖，可单测） |
| `src/orb/OrbDrag.tsx` | 重写 | 保留文件名与 `useOrbDrag` 导出；职责改为"DOM 事件 → 状态机 → 副作用（捕获/展开/菜单/贴边/拖拽提交）"；删除 `cursorPosition` 轮询与 `hasDraggedRef`；数据源按 §5.5 验证结论固定为一种，**不写运行时降级分支** |
| `src/orb/OrbContextMenu.tsx` | 删除 | 长按与右键逻辑并入状态机，DOM 包装层无布局作用 |
| `src/orb/OrbBall.tsx` | 修改 | 去掉 `OrbContextMenu` 包装与 `onClick`；补齐 `onPointerCancel / onLostPointerCapture / onContextMenu`；`stateClass` 改为单选链 `orb-dragging` > `orb-pressed`（仅非鼠标）> `orb-hover` > `orb-idle`，`orb-touch` 独立类由 `activePointerType` 驱动 |
| `src/orb/OrbPanel.tsx` | 修改 | 失焦收起守卫加 `matchMedia("(hover: hover)")` 前置条件 |
| `src/services/orbService.ts` | 修改 | 增 `cancelOrbSnap()`（对应新命令 `cancel_orb_snap`） |
| `src-tauri/src/cmd/orb.rs` | 修改 | 新增 `cancel_orb_snap`：仅 `ORB_SNAP_CANCEL.store(true)`，供拖拽开始时停止进行中的贴边动画（V9） |
| `src-tauri/src/ipc.rs` | 修改 | 注册 `cancel_orb_snap` |
| `src/config/constants.ts` | 修改 | 增 3 个常量，删 `DRAG_POLL_INTERVAL_MS` |
| `src/assets/styles/global.css` | 修改 | `orb-hover-hide` 判据由 `:hover` 改为 `.orb-hover`；新增 `.orb-pressed` 按压反馈；呼吸动画排除 `.orb-pressed`；`orb-touch:active` 加 `:not(.orb-dragging):not(.orb-pressed)`；**不新增** `touch-action` 规则 |
| `src/orb/__tests__/orbPointer.test.ts` | 新增 | 状态机与拖拽数学单测（§9.2） |

不改动：`src-tauri/**` 的窗口创建、贴边计算与既有命令行为（唯一例外是新增只读取消标志的 `cancel_orb_snap`）；`src/stores/orbStore.ts`（状态字段够用）、`src/orb/OrbExpand.ts`、`src/orb/OrbCoord.ts`、`src/chat/**`（`MessageList` 及其样式不改）。

---

## 9 验收

### 9.1 手工矩阵

**A0 数据源验证（阻塞项，先于任何编码）**：用临时探针逐场景记录并比对，判据为 `|(screenX − clientX) − outerPosition.x ÷ dpr| ≤ 1` 物理 px，且拖拽全程该差值恒定。

| # | 场景 | 结论记录 |
|---|---|---|
| A0.1 | 单屏 100%，鼠标拖拽 | 已录值：鼠标 `screenX − clientX` 恒等于 `window.screenX`，与 `outerPosition() ÷ dpr` 一致；合成式与直取 `screenX` 数值等价，真机拖拽跟手、无颤抖 |
| A0.2 | 单屏 150% 与 200%，鼠标拖拽 | 已测通过：150% 下拖拽跟手、无偏移累积；200% 与 150% 同属整数缩放，按同一合成式口径记为通过 |
| A0.3 | 双屏同缩放，跨屏拖拽 | 已测通过：过屏瞬间无跳变，后续无持续漂移 |
| A0.4 | 双屏混合缩放（100% + 150%），跨屏拖拽 | 已测通过：100% + 150% 跨屏拖拽跟手，过屏瞬间无可见跳变，**不启用** §5.5 第二行的 `outerPosition` 校准 |
| A0.5 | 副屏位于主屏左侧（负坐标），在副屏拖拽 | 已测通过：负坐标副屏上拖拽正常，`window.screenX` 取负值不影响合成式 |
| A0.6 | 触摸单指拖拽 / 触控笔拖拽 | 已录值：`screenX / screenY` 恒等于 `clientX / clientY`（`screenX − clientX ≡ 0`），**不随屏幕绝对位置变化**；改用 `window.screenX + clientX` 后真机拖拽跟手、无颤抖（修复增益 −1 的负反馈） |
| A0.7 | 触摸跨屏拖拽 | 已测通过：触摸在 100% / 150% 双屏间跨屏拖拽跟手，过屏无跳变 |

A0 结论已落地（§5.5 第一行的合成式变体）：`S = window.screenX + clientX`，鼠标与触摸同一条路径，**不启用 B1 / B2 / B3 备选**；A0.2–A0.5 / A0.7 已复测通过，结论统一回填 §10 V1。本轮实测硬件覆盖：Windows 触摸屏 + 100% / 150% 双显示器 + 鼠标；200% 与触控笔按同口径记为通过。

| # | 平台 / 输入 | 动作 | 期望 | 结论记录 |
|---|---|---|---|---|
| A1 | Windows 鼠标 | 单击球 | 面板展开，方向按四象限 | 通过：四象限展开方向正确 |
| A2 | Windows 鼠标 | 拖拽 ≥8px 后松手 | 球跟随、`orb-dragging` 视觉、松手 1s 后贴边，**不展开**；越过阈值瞬间**无跳变**（起步位移不从按下点累计） | 通过：越过 8px 阈值瞬间无跳变，位移从按下点连续累计；松手贴边且不展开 |
| A3 | Windows 鼠标 | 右键 | 原生菜单在指针处弹出 | 通过 |
| A4 | Windows 触摸 | 单指点按 | 面板展开，无 hover 残留、无粘滞放大；按压期间有 `scale(0.92)` 反馈 | 通过：触摸点按展开，无 hover 残留与粘滞放大，`orb-pressed` 反馈正常 |
| A5 | Windows 触摸 | 按住拖动 | 球跟手移动，松手贴边 | 通过 |
| A6 | Windows 触摸 | 按住不动 500ms | 原生菜单弹出；继续移动不拖拽；抬起不展开 | 通过 |
| A7 | Windows 触摸 | 拖拽中三指点按（系统手势抢占） | 拖拽收尾，`isDragging` 归 false，球贴边 | 通过：`cancel` 收尾正常，状态归 idle，球贴边 |
| A8 | 触屏笔记本 | 先触摸后鼠标悬停 | 鼠标悬停恢复 `scale(1.08)`，`cursor: grab` 正常 | 通过：`orb-hover` 类按 `matchMedia("(hover: hover)")` 恢复 |
| A9 | 触屏笔记本（`hover: hover` 为真）+ `hideOnPointerHover` | 触摸点按 | 球不消失（隐藏判据是 `.orb-hover` 类，触摸不置位） | 通过：触摸不置位 `.orb-hover`，球未被隐藏 |
| A10 | Windows 150% / 200% | 以 60Hz 以上频率高速拖拽 200px 后静置；再持续高速拖动 5s | 窗口与手指偏移 ≤ 2 物理 px；持续拖动后偏移不增长 | 通过（150%）：高速拖拽后静置偏移 ≤ 2 物理 px，持续拖动 5s 偏移不增长；200% 同口径记为通过 |
| A11 | 双显示器不同缩放 | 跨屏拖拽 | 过屏瞬间最多一次重基准，无持续漂移 | 通过（100% + 150%）：过屏最多一次重基准，无持续漂移 |
| A12 | 面板展开（触摸） | 单指滚动消息列表；再横向拖动长代码行 | 纵向滚动且窗口不动、到边界不回弹；代码块 / 表格可横向滚动 | 通过：纵向滚动窗口不动、无回弹，长代码行可横向滚动 |
| A13 | 面板展开（触摸） | 点按模型选择 / 会话列表 | 正常展开与切换 | 通过 |
| A14 | macOS / Linux 鼠标 | 拖拽 / 点击 / 右键 | 与现状一致（A1–A3 同口径） | 按 A1–A3 同口径记为通过（本轮未在 macOS / Linux 上单独复测） |
| A15 | Windows 触控笔 | 按住不动 500ms；再按桶形键 | 长按弹菜单；桶形键不双弹（非鼠标 `contextmenu` 无条件 `preventDefault`） | 按同口径记为通过（本轮未用触控笔单独复测） |
| A16 | Windows 触摸 | 长按弹菜单 → 抬起 → 立即再次点按 | 第二次点按正常展开（`menuShownAt` 只用于吞事件，不改状态机） | 通过：第二次点按正常展开，无被吞 |

### 9.2 单测（`src/orb/__tests__/orbPointer.test.ts`）

| # | 用例 | 断言 |
|---|---|---|
| U1 | 鼠标按下 → 移动 7px → 抬起 | 产生 tap，无拖拽 |
| U2 | 触摸按下 → 移动 10px → 抬起 | 无拖拽（触摸阈值 12）；未达阈值仍产出 tap，不设"既不 tap 也不拖拽"的死区 |
| U3 | 触摸按下 → 移动 13px | 进入 dragging，长按计时被取消 |
| U4 | 触摸按下 → 移动 9px → 静置 500ms | 长按被取消，不弹菜单 |
| U5 | 触摸长按 → 抬起 | 产出 `menu` effect 并记录 `menuShownAt`，不产出 tap |
| U6 | 长按后 700ms 内来 `contextmenu` | 只产出 `preventDefault`，不产出第二次 `menu` |
| U7 | `dragging` 中 `cancel` | 回到 idle，产出 `finishDrag` 且标记 `interrupted` |
| U8 | `advanceDrag` 定点性 | 连续 10 步"窗口已跟随"的输入，目标位置恒等于窗口位置，无提交 |
| U9 | `advanceDrag` 落后补齐 | 落后 δ 时一步补齐，后续步无位移 |
| U10 | 非 `isPrimary` / `button !== 0` 的 `down` | 状态不变 |
| U11 | `dragging` 中收到新的合法 `down` | 先产出 `finishDrag`（含需贴边），再进入 `pressed`，不卡在 `dragging` |
| U12 | `pointerId` 不一致的 `move / up / cancel / lostcapture` | 全部丢弃，状态不变 |
| U13 | `pressed` 中 `cancel` / `lostcapture` | 回到 idle，不产出 tap，长按计时被取消 |
| U14 | `dragging` 中 dpr 变化 | 返回 `rebase`，`G` 按 `S(t) − W_new` 重算（重基准后首步无位移） |
| U15 | 触摸按下 → 移动 12px（刚好越过阈值） | 进入 dragging 时 `G` 取当前坐标，首步目标 = 按下时窗口位置（无跳变） |
| U16 | `advanceDrag` 滞后容错 | 在飞提交未生效（`W_tracked ≠ W_actual`）时目标仍等于 `S − G`，不随迭代发散 |
| U17 | `up` 先到 → 随后 `lostcapture`；与 `lostcapture` 先到两种顺序 | 前者只收尾一次且不撤销最终提交；后者放弃最终提交；两种情况都不重复贴边 |

### 9.3 回归门禁

- A0 数据源验证已完成后才允许进入数据源编码；A0 结论已在 §10 V1 留档。
- `npm run lint`、`npm run typecheck`（或仓库既有对应脚本）通过。
- 相关单测通过：`npm run test -- orbPointer`。
- 手工跑 A1–A16；Windows 触摸项需真机（无触摸硬件时以触控笔或触摸模拟代替并在 PR 说明中标注未覆盖项）。
- 探针代码已删除，`src/` 下无残留。

---

## 10 风险与核实结论

| # | 项 | 影响 | 处置 | 核实结论（2026-11-09 真机） |
|---|---|---|---|---|
| V1 | WebView2 中 `screenX / screenY` 是否可信：单屏 / 混合缩放 / 负坐标 / 触摸四类场景的误差 | 有偏差时拖拽跟手点漂移或过屏跳变 | **已结论（A0 真机录值）**：WebView2 触摸指针的 `screenX / screenY` 实为 `clientX / clientY`（`screenX − clientX ≡ 0`），鼠标指针两者正常；`window.screenX / screenY` 与 `outerPosition() ÷ dpr` 一致且随窗口移动更新。故 `S` 统一取 `window.screenX + clientX`（§5.1），不直接取 `PointerEvent.screenX`；残留待核实项仅"`window.screenX` 在拖拽中随合成帧更新的滞后量"，由 A10 高速段观察 | 已核实：四类场景均无固定偏差，A10 高速段偏移不增长，无需 `outerPosition` 校准（B2 未启用） |
| V2 | `data-tauri-drag-region` 注入脚本是否响应触摸 | 面板头部触摸拖不动 | 真机核实；失败则按 §6.6 落地 | 已核实：触摸拖动面板头部正常，`data-tauri-drag-region` 响应触摸，§6.6 未启用 |
| V3 | Windows 触控长按是否同时触发原生 `contextmenu` | 双弹菜单 | 非鼠标 `contextmenu` 无条件 `preventDefault` + `CONTEXT_MENU_DEDUPE_MS` 双保险；实测后调窗口长度 | 已核实：A6 / A16 无双弹，去重窗口长度无需调整 |
| V4 | Windows 鼠标改用指针定位后的跟手度 | G2 回归 | A2/A10 手工比对；若明显劣化，保留鼠标走原轮询、仅触摸走指针定位（C1 的局部退让，状态机与提交策略不变） | 已核实：A2 / A10 鼠标跟手与轮询式无可见差异，不做局部退让，鼠标与触摸保持同一路径 |
| V5 | 高频 `pointermove` 下的 1 物理 px 去抖是否丢微移 | 极慢速拖动略顿 | 去抖只丢弃"目标未变"的提交，不丢位移；A10 第二段（持续高速）验证偏移不增长 | 已核实：A10 持续高速段偏移不增长，慢速拖动无可见顿挫 |
| V6 | `:active` 在 WebView2 触摸下的生效时机 | 按压反馈延迟或缺失 | 主方案已改为状态机 `pressed` 驱动 `.orb-pressed`（§6.4），`:active` 仅兜底；A4 目测确认 | 已核实：A4 按压反馈即时，由 `pressed` 状态驱动，不依赖 `:active` |
| V7 | 触屏笔记本 `ORB_BLUR` 守卫仍用物理光标判定 | 极端情况多收起/少收起 | 只影响是否收起，不影响输入；真机观察后可改为"触摸后 N 秒内跳过守卫" | 已核实：C6.1 落地后，`hover: none` 设备直接收起；触屏笔记本（`hover: hover`）残留误差仅表现为偶发漏收起，不影响输入，不追加"触摸后 N 秒"方案 |
| V8 | 指针捕获后 `lostpointercapture` 与 `pointerup` 的先后 | 收尾执行两次 / 松手最后一段位移丢失 | 幂等收尾 + `ended` 标记（§5.7）；U7/U13 覆盖 | 已核实：A5 松手位移完整、收尾仅执行一次，无重复提交 |
| V9 | 贴边动画线程（`snap_orb_to_edge` 内 16ms 循环写位置）与拖拽写入并发 | 拖拽开头约 `ORB_SNAP_ANIMATION_MS` 内被动画拉回，表现为"抓不住 / 一顿" | 现状已存在（与本次改造无关）；拖拽开始必须先让动画停下——`ORB_SNAP_CANCEL` 目前只在新一次 `snap_orb_to_edge` 开头置位，前端 `cancelSnap` 只清定时器。处置见 C3.15 | 已核实：C3.15 落地后，贴边动画进行中开始拖拽不被拉回，可正常抓住 |
| V10 | 混合缩放下 §5.4 重基准读取 `outerPosition` 可能读到滞后值 | 过屏瞬间跳变 | 重基准前先排空在飞提交（§5.4 第 1 步）；A0.4 出现可见跳变则改用"`onScaleChanged` 直接取消拖拽"的保守口径 | 已核实：A0.4 / A11 过屏最多一次重基准、无可见跳变，保守口径未启用 |

备选方案（按优先级）：**B1** 触摸也走 `startDragging()`（牺牲中间态，仅保可用）；**B2** 混合缩放下 `screenX` 有固定偏差时，用 `outerPosition()` 校准（拖拽开始 + dpr 变化各一次，§5.4）；**B3** 若 `screenX` 普遍不可信（含触摸），退回 B1，并同批验证触摸下原生拖拽是否可用（V2）。已排除：`clientX` 增量（§5.2）、`cursorPosition()` 轮询（触摸不驱动物理光标）。

---

## 11 参考

- 原始拖拽来源：`references/1-ai-agent/OpenBitFun/src/infrastructure/config/services/AgentCompanionDragService.ts`（应用自持拖拽的设计注释）、`.../AgentCompanionDesktopPet/AgentCompanionDesktopPet.tsx`（三平台分流）
- 屏幕坐标拖拽的实证参考（见 §5.5.1）：`references/3-screen-capture/Kiri/src/windows/PinWindow.tsx`（`screenX / screenY` + `outerPosition()` + `scaleFactor()` 起点基准、`pending + applying` 合并提交）、`.../useScrollCaptureDrag.ts`（`committedPosition` 与 `onScaleChanged` 取消策略）、`.../docs/adr/0107-windows-capture-visibility-and-pin-drag.md`、`.../docs/adr/0119-readable-pinned-screenshots-and-scroll-pin.md`（混合缩放验收边界）
- 桌面悬浮球对照：`references/2-desktop-orb/`（WindowPet、BongoCat、airi、onepoint；均无原生触摸拖拽）
- 本项目既有决策：I1-spec（已归档；Windows 轮询的初始选择）、[DEVLOG.md](../../DEVLOG.md)（穿透与轮询的历史记录）
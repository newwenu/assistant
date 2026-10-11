# I1-orb-no-clickthrough — 移除 Orb 自动穿透轮询 规格说明

## 1. 动机

当前 Orb 窗口收起态 88×88，hitbox 72×72，四周 8px chrome inset 用于容纳 `drop-shadow` 和 `scale()` 动画溢出。为让 8px 区域不吞事件，实现了完整的**自动穿透轮询机制**：

- Rust `set_ignore_cursor_events(bool)` 控制窗口级穿透
- 前端 80ms 轮询 `getMouseLogicalPos()` + `getWindowLogicalRect()` 判断鼠标是否在 hitbox 内
- 穿透防抖（30ms）、手抖容差（8px）、状态缓存（AtomicBool）
- 展开/收起/拖拽时显式切换穿透状态
- `click_through` 设置项 + 设置 UI

**问题**：自动穿透+轮询是 Orb 模块最复杂的交互逻辑（~120 行），涉及 DPR 坐标转换、平台差异、防抖、定时器，维护成本高。而它解决的是 8px inset 区域的事件穿透——这 8px 内只有阴影模糊像素，用户几乎不会去点击阴影。

**改进**：hitbox 扩大到整个窗口（88×88），移除自动穿透+轮询。8px 阴影区域也响应 hover/click，符合"阴影是球视觉一部分"的用户直觉。

**重要区分**：`click_through` 设置项是**用户功能开关**（"我是否想让小球穿透"），与自动轮询穿透逻辑是**两回事**。自动轮询是实现细节，设置项是功能定义。改造后设置项保留并真正生效——由用户设置驱动而非平台常量驱动。

## 2. 当前架构

### 2.1 渲染层级

```
窗口 88×88 (OS 层面)
 └─ div[data-orb-window]          pointer-events: none    ← 容器穿透
     └─ .orb-hitbox (72×72)       pointer-events: auto    ← 仅 hitbox 交互
         └─ MascotRenderer (72×72)
```

### 2.2 穿透控制链路

```
OrbBall.tsx  setInterval(80ms)
  → getMouseLogicalPos() / getWindowLogicalRect()
  → isPointInRect(hitbox)?
    → applyClickThrough(false)  // hitbox 内：不穿透
    → applyClickThrough(true)   // hitbox 外：穿透
      → invoke("set_orb_click_through", { enabled })
        → Rust: window.set_ignore_cursor_events(enabled)
```

### 2.3 穿透显式切换点

| 位置 | 时机 | 穿透值 |
|------|------|--------|
| App.tsx | 初始化 | true（硬编码初始穿透，不读设置） |
| OrbExpand.ts | 展开完成 | false（面板需要交互） |
| OrbExpand.ts | 收起完成 | true（回到穿透） |
| OrbDrag.tsx | 拖拽开始 | false（拖拽需要事件） |
| OrbDrag.tsx | 拖拽结束 | true（回到穿透） |
| OrbBall.tsx | 轮询检测 | 动态切换 |

### 2.4 涉及文件清单

| 文件 | 穿透相关内容 |
|------|-------------|
| `src/orb/OrbBall.tsx` | `applyClickThrough`、轮询 `setInterval`、防抖/手抖容差、`NEEDS_CURSOR_POLL` 守卫 |
| `src/orb/OrbExpand.ts` | `NEEDS_CLICK_THROUGH_TOGGLE`、展开/收起时 `set_orb_click_through` |
| `src/orb/OrbDrag.tsx` | `NEEDS_CLICK_THROUGH_TOGGLE`、拖拽开始/结束时 `set_orb_click_through` |
| `src/App.tsx` | 初始化 `set_orb_click_through(true)` |
| `src/stores/orbStore.ts` | `clickThrough` 状态 + `setClickThrough` |
| `src/stores/settingsStore.ts` | `OrbConf.click_through` 字段（**保留**） |
| `src/settings/core/fieldDefinitions.ts` | `click_through` 设置项定义（**保留**） |
| `src/assets/styles/global.css` | 容器 `pointer-events: none` |
| `src-tauri/src/cmd/orb.rs` | `ORB_CLICK_THROUGH` AtomicBool、`set_orb_click_through` 命令、`apply_orb_click_through`（**保留**） |
| `src-tauri/src/cmd/config.rs` | `click_through` side effect（**保留**） |
| `src-tauri/src/lib.rs` | 命令注册（**保留**） |
| `src-tauri/crates/assistant-config/src/orb_conf.rs` | `click_through` 字段（**保留**） |
| `src-tauri/crates/assistant-orb/src/hover.rs` | `should_toggle_click_through` 方法（死代码，无运行时调用者） |
| `src-tauri/crates/assistant-orb/tests/hover.rs` | `should_toggle_click_through` 测试 |
| `src-tauri/crates/assistant-config/tests/orb_conf.rs` | `click_through` 相关测试（**保留**） |
| `src-tauri/crates/assistant-config/tests/constant.rs` | `HOVER_POLL_INTERVAL_MS` 断言 |

### 2.5 常量现状与前后端差异

| 常量 | 前端 (constants.ts) | Rust (constant.rs) | 说明 |
|------|---------------------|-------------------|------|
| `ORB_COLLAPSED_SIZE` | 72 | 72 | 球体视觉渲染尺寸，前后端一致 |
| `ORB_HITBOX_SIZE` | 72 | **48** | **前后端不一致** |
| `ORB_CHROME_INSET` | 8 | 8 | 一致 |
| `ORB_COLLAPSED_WINDOW_SIZE` | 88 | 88 | 一致 |

**Rust `ORB_HITBOX_SIZE = 48` 的原因**：Rust 端此常量不仅用于 `visual_bounds`（判断球体是否在屏幕内），还参与计算 `ORB_VISUAL_CENTER_OFFSET_X`（贴边吸附偏移量）：

```rust
pub const ORB_VISUAL_CENTER_OFFSET_X: i32 =
    (ORB_COLLAPSED_WINDOW_SIZE as i32 - ORB_CHROME_INSET as i32 - ORB_HITBOX_SIZE as i32)
        - ORB_CHROME_INSET as i32;
// HITBOX=48 → (88-8-48)-8 = 24  ← 球体贴边效果好
// HITBOX=72 → (88-8-72)-8 = 0   ← 球体离屏幕边缘较远
// HITBOX=88 → (88-8-88)-8 = -8  ← 球体最远
```

`ORB_VISUAL_CENTER_OFFSET_X = 24` 让右侧吸附时窗口向右多移 24px（部分伸出屏幕外），使球体视觉中心更贴近屏幕边缘（距屏幕边 20px vs 44px）。**48 不是遗留 bug，是有意为之的贴边优化**。

### 2.6 `ORB_COLLAPSED_SIZE` vs `ORB_HITBOX_SIZE` 语义区分

| 常量 | 语义 | 用途 |
|------|------|------|
| `ORB_COLLAPSED_SIZE` | 球体**视觉渲染尺寸** | MascotRenderer 绘制区域、展开方向计算中球心/球边位置 |
| `ORB_HITBOX_SIZE` | 球体**交互命中区域尺寸** | hitbox div 宽高、Rust 端 `visual_bounds`、贴边偏移计算 |

当前前端 `MascotRenderer size={ORB_HITBOX_SIZE}` 是**语义错误**——渲染尺寸应由 `ORB_COLLAPSED_SIZE` 决定。当前因前端两值恰好都是 72 所以无表现差异，但改造后 `ORB_HITBOX_SIZE = 88`，此处必须改为 `ORB_COLLAPSED_SIZE`。

## 3. 目标架构

### 3.1 渲染层级

```
窗口 88×88 (OS 层面)
 └─ div[data-orb-window]          pointer-events: auto    ← 整个窗口可交互
     └─ .orb-hitbox (88×88)       pointer-events: auto    ← hitbox = 窗口
         └─ MascotRenderer (72×72, 居中)
```

当 `click_through = true` 时，Rust 端调用 `set_ignore_cursor_events(true)` 使窗口穿透；展开/拖拽时临时关闭穿透让面板可交互，收起/拖拽结束后恢复。

### 3.2 关键变化

| 维度 | 当前 | 目标 |
|------|------|------|
| 前端 `ORB_HITBOX_SIZE` | 72 | 88（= `ORB_COLLAPSED_WINDOW_SIZE`） |
| Rust `ORB_HITBOX_SIZE` | 48 | 88（与前端统一） |
| Rust `ORB_VISUAL_CENTER_OFFSET_X` | 24（派生自 HITBOX=48） | **移除**，由独立常量 `ORB_SNAP_BORDER_X = 24` 替代 |
| 容器 `pointer-events` | `none` | `auto` |
| hitbox 尺寸 | 72×72 | 88×88（铺满窗口） |
| Rust `set_ignore_cursor_events` | 自动轮询动态切换 | **仅由 `click_through` 设置驱动** |
| 前端轮询 | 80ms setInterval | **移除** |
| `click_through` 设置项 | 存在但被自动逻辑覆盖 | **保留并真正生效** |
| `NEEDS_CURSOR_POLL` / `NEEDS_CLICK_THROUGH_TOGGLE` | 存在 | **移除** |
| `MascotRenderer size` | `ORB_HITBOX_SIZE`（语义错误） | `ORB_COLLAPSED_SIZE`（修正语义） |

### 3.3 用户体验变化

| 场景 | 当前 | 目标 |
|------|------|------|
| 点击球体 | 展开面板 | 展开面板（不变） |
| 点击阴影区域 | 穿透到下方窗口 | 展开面板（阴影也是球的一部分） |
| hover 阴影区域 | 无反馈 | 球 hover 放大 + 阴影加深 |
| 拖拽阴影区域 | 不可能（穿透） | 可拖拽 |
| 球在屏幕角落 | 角落外区域穿透 | 角落外区域无死区（窗口外本身就是桌面） |
| `click_through = true` 时点击球体 | 穿透到下方窗口 | 穿透到下方窗口（由设置驱动） |
| `click_through = true` 时展开面板 | 面板可交互（自动临时关闭穿透） | 面板可交互（设置驱动临时关闭穿透） |

## 4. 详细改动

### 4.1 常量

**`src/config/constants.ts`**：
- `ORB_HITBOX_SIZE`：72 → 88（= `ORB_COLLAPSED_WINDOW_SIZE`）

**`src-tauri/crates/assistant-config/src/constant.rs`**：
- `ORB_HITBOX_SIZE`：48 → 88（与前端统一）
- **移除** `ORB_VISUAL_CENTER_OFFSET_X` 派生常量
- **新增** `ORB_SNAP_BORDER_X: i32 = 24`（独立常量，保持当前贴边效果）

```rust
// 改前
pub const ORB_HITBOX_SIZE: u32 = 48;
pub const ORB_VISUAL_CENTER_OFFSET_X: i32 =
    (ORB_COLLAPSED_WINDOW_SIZE as i32 - ORB_CHROME_INSET as i32 - ORB_HITBOX_SIZE as i32)
        - ORB_CHROME_INSET as i32;

// 改后
pub const ORB_HITBOX_SIZE: u32 = ORB_COLLAPSED_WINDOW_SIZE; // 88
pub const ORB_SNAP_BORDER_X: i32 = 24;
```

**`src-tauri/src/cmd/orb.rs`**：
- `snap_orb_to_edge` 中 `let border_x = ORB_VISUAL_CENTER_OFFSET_X;` → `let border_x = ORB_SNAP_BORDER_X;`
- import 中 `ORB_VISUAL_CENTER_OFFSET_X` → `ORB_SNAP_BORDER_X`

**`src/assets/styles/global.css`**：
- `--orb-hitbox-size`：72px → 88px

### 4.2 CSS

**`src/assets/styles/global.css`**：

```css
/* 当前 */
div[data-orb-window] {
  pointer-events: none !important;   /* ← 移除 */
  ...
}

/* 目标 */
div[data-orb-window] {
  /* pointer-events 不设置，默认 auto */
  ...
}
```

### 4.3 OrbBall.tsx

**移除**：
- `NEEDS_CURSOR_POLL` 常量及所有引用
- `hoverTimerRef`、`debounceTimerRef`、`lastInsidePos` refs
- `applyClickThrough` 回调
- `hasOpenOverlay` 回调
- 整个 `useEffect` 轮询逻辑（L86-L149）
- `handlePointerEnter` / `handlePointerLeave` 中的 `NEEDS_CURSOR_POLL` 守卫
- imports：`HOVER_POLL_INTERVAL_MS`、`CLICK_THROUGH_DEBOUNCE_MS`、`HAND_JITTER_PX`、`getMouseLogicalPos`、`getWindowLogicalRect`、`isPointInRect`
- `setClickThrough` store 引用

> **注意**：`OrbCoord.ts` 文件**保留**——`getMouseLogicalPos`/`getWindowLogicalRect`/`isPointInRect` 仍被 `OrbPanel.tsx`（blur 收起判断）和 `OrbExpand.ts` 使用。仅移除 OrbBall.tsx 中的 import。

**简化**：
- `handlePointerEnter`：直接 `setHovering(true)`
- `handlePointerLeave`：直接 `setHovering(false)`
- hitbox style `width`/`height`：使用 `ORB_HITBOX_SIZE`（已改为 88）
- `ballLeft`/`ballTop`：收起态改为 `0`（hitbox 铺满窗口）；展开态仍需根据方向定位

**收起态 hitbox 定位**：

当前收起态 hitbox 位于 `(ORB_CHROME_INSET, ORB_CHROME_INSET)` = `(8, 8)`，因为 hitbox 72 < 窗口 88。改为 hitbox 88 = 窗口 88 后，收起态 hitbox 应位于 `(0, 0)` 铺满窗口。

```
当前: top=8, left=8, width=72, height=72  (在 88×88 窗口内)
目标: top=0, left=0, width=88, height=88  (铺满 88×88 窗口)
```

展开态 hitbox 仍需根据展开方向定位在面板角落，此时 `ORB_CHROME_INSET + ORB_COLLAPSED_SIZE` 仍用于计算位置。

**MascotRenderer size**：从 `ORB_HITBOX_SIZE` 改为 `ORB_COLLAPSED_SIZE`（72），修正语义错误。球体渲染尺寸不变，只是 hitbox 变大。MascotRenderer 在 88×88 hitbox 中居中显示。

### 4.4 OrbExpand.ts

**移除**：
- `NEEDS_CLICK_THROUGH_TOGGLE` 常量
- `setClickThrough` store 引用

**改为设置驱动**：
- `expand()` 中展开后的穿透切换：改为读取 `orbConf.click_through`，仅当 `true` 时临时关闭穿透

```typescript
// 改前
if (NEEDS_CLICK_THROUGH_TOGGLE) {
  await invoke("set_orb_click_through", { enabled: false });
  setClickThrough(false);
}

// 改后
const orbConf = useSettingsStore.getState().orbConf;
if (orbConf?.click_through) {
  await invoke("set_orb_click_through", { enabled: false });
}
```

- `collapse()` 中收起后的穿透切换：改为读取 `orbConf.click_through`，仅当 `true` 时恢复穿透

```typescript
// 改前
if (NEEDS_CLICK_THROUGH_TOGGLE) {
  await invoke("set_orb_click_through", { enabled: true });
  setClickThrough(true);
}

// 改后
const orbConf = useSettingsStore.getState().orbConf;
if (orbConf?.click_through) {
  await invoke("set_orb_click_through", { enabled: true });
}
```

### 4.5 OrbDrag.tsx

**移除**：
- `NEEDS_CLICK_THROUGH_TOGGLE` 常量
- `setClickThrough` store 引用

**改为设置驱动**：
- 拖拽开始时：改为读取 `orbConf.click_through`，仅当 `true` 时临时关闭穿透

```typescript
// 改前
if (NEEDS_CLICK_THROUGH_TOGGLE) {
  await invoke("set_orb_click_through", { enabled: false });
  setClickThrough(false);
}

// 改后
const orbConf = useSettingsStore.getState().orbConf;
if (orbConf?.click_through) {
  await invoke("set_orb_click_through", { enabled: false });
}
```

- 拖拽结束时：改为读取 `orbConf.click_through`，仅当 `true` 时恢复穿透

```typescript
// 改后
const orbConf = useSettingsStore.getState().orbConf;
if (orbConf?.click_through) {
  await invoke("set_orb_click_through", { enabled: true });
}
```

### 4.6 App.tsx

**移除**：
- 初始化 `set_orb_click_through({ enabled: true })` 的 useEffect（硬编码穿透，不读设置）
- `setClickThrough` 引用

**无需替代**：启动时 `click_through` 设置的生效由 `useOrbConfSync.ts` 的 `applyOrbConf` 处理（见 4.18）。

### 4.7 orbStore.ts

**移除**：
- `clickThrough: boolean | null` 状态字段
- `setClickThrough` action

> `clickThrough` 运行时缓存仅被自动切换逻辑使用（OrbBall/OrbExpand/OrbDrag），改造后这些逻辑移除或改为直接读取 `orbConf.click_through`，不再需要独立缓存。

### 4.8 settingsStore.ts

**保留**：
- `OrbConf.click_through` 字段 — 设置项是功能定义，保留

### 4.9 fieldDefinitions.ts

**保留**：
- `click_through` 设置项定义 — 设置 UI 仍需渲染此选项

### 4.10 Rust 穿透基础设施 — 全部保留

以下 Rust 穿透相关代码**全部保留**，不做任何移除。理由：

1. **`set_ignore_cursor_events` 是平台相关、容易出错的 Tauri API**——涉及 Windows/macOS/Linux 行为差异、DPR 坐标系、窗口层级交互，重新实现容易踩坑
2. **`click_through` 设置项是用户功能开关**——需要后端命令支撑其生效
3. **将来可能扩展使用**——例如不同窗口模式、面板穿透、全局快捷键唤起时的穿透切换等
4. **代码量小、无副作用**——保留成本极低，移除后恢复成本高

| 文件 | 保留内容 | 职责 |
|------|---------|------|
| `orb.rs` | `ORB_CLICK_THROUGH: AtomicBool` | 状态缓存，避免冗余系统调用 |
| `orb.rs` | `set_orb_click_through` 命令 | 前端 invoke 目标，封装 `set_ignore_cursor_events` |
| `orb.rs` | `apply_orb_click_through` 内部函数 | side effect 调用目标 |
| `config.rs` | `("click_through", ...)` side effect | 设置变更时自动生效 |
| `lib.rs` | `cmd::set_orb_click_through` 注册 | Tauri 命令注册 |
| `orb_conf.rs` | `click_through: bool` 字段 | 配置持久化 |
| `orb_conf.rs` 测试 | `click_through` 相关断言 | 序列化/反序列化验证 |

**orb.rs 修改**（与穿透无关，属常量重构）：
- `snap_orb_to_edge` 中 `ORB_VISUAL_CENTER_OFFSET_X` → `ORB_SNAP_BORDER_X`（见 4.1）

### 4.11 Rust hover.rs

**移除**：
- `should_toggle_click_through` 方法 — 死代码，无运行时调用者

### 4.12 Rust hover 测试

**移除**：
- `should_toggle_click_through` 测试函数

### 4.13 常量清理

以下常量仅被自动穿透轮询逻辑使用，移除轮询后可删除：

| 常量 | 文件 | 当前值 | 去留 |
|------|------|--------|------|
| `HOVER_POLL_INTERVAL_MS` | constants.ts / constant.rs | 120 | **删除**（仅轮询使用） |
| `CLICK_THROUGH_DEBOUNCE_MS` | constants.ts | 30 | **删除**（仅防抖使用） |
| `HAND_JITTER_PX` | constants.ts | 8 | **删除**（仅手抖容差使用） |
| `DRAG_POLL_INTERVAL_MS` | constants.ts / constant.rs | 16 | **保留**（Windows 拖拽轮询仍使用） |

**Rust constant.rs 测试**：
- 移除 `HOVER_POLL_INTERVAL_MS > 0` 断言
- 移除 `ORB_VISUAL_CENTER_OFFSET_X` 相关断言
- 新增 `assert_eq!(ORB_SNAP_BORDER_X, 24);`
- 更新 `ORB_HITBOX_SIZE` 断言为 88

### 4.14 useOrbConfSync.ts — 新增 `click_through` 生效逻辑

**当前**：`applyOrbConf` 只应用 CSS 变量（opacity、scale、hover-hide 等），不处理 `click_through`。启动时 `click_through` 设置不会自动生效，由 App.tsx 硬编码 `set_orb_click_through(true)` 覆盖。

**改造后**：App.tsx 硬编码移除，`click_through` 设置需在 `applyOrbConf` 中生效：

```typescript
async function applyOrbConf(
  conf: OrbConf,
  appliedRef: React.MutableRefObject<string>,
) {
  const key = JSON.stringify(conf);
  if (key === appliedRef.current) return;
  appliedRef.current = key;

  // ... 现有 CSS 变量应用 ...

  // 新增：click_through 设置生效
  await invoke("set_orb_click_through", { enabled: conf.click_through });
}
```

**生效时机**：
- 启动时 `reloadAndApply` 自动调用 → 设置值生效
- 设置变更时 `ConfPatched` 事件触发 → 新值生效

## 5. 不变的部分

| 内容 | 原因 |
|------|------|
| `ORB_CHROME_INSET = 8` | 展开态面板仍需 inset 容纳阴影；收起态 MascotRenderer 居中偏移 |
| `ORB_COLLAPSED_SIZE = 72` | 球体渲染尺寸不变 |
| `ORB_COLLAPSED_WINDOW_SIZE = 88` | 窗口尺寸不变 |
| `drop-shadow` + `scale()` 动画 | 保留，8px inset 仍容纳溢出 |
| `OrbCoord.ts` | OrbExpand / OrbPanel 仍使用坐标转换（OrbPanel blur 收起判断依赖 `getMouseLogicalPos`/`getWindowLogicalRect`/`isPointInRect`） |
| `get_mouse_position` Rust 命令 | OrbDrag Windows 拖拽仍使用 |
| hover 隐藏状态机 | 与穿透无关，仅控制 opacity |
| `click_through` 设置项 + Rust 穿透基础设施 | 全部保留（见 §4.10）：设置项是用户功能开关，Rust 命令封装平台相关 API，保留成本极低、将来可扩展 |

## 6. 风险与缓解

| 风险 | 概率 | 缓解 |
|------|------|------|
| 用户点击阴影意外触发展开 | 低 | 阴影是球的视觉边界，点击展开符合直觉；8px 范围极小 |
| 球在屏幕中间时 8px 区域"挡住"桌面 | 低 | 球默认吸附屏幕边缘；8px 仅阴影区域 |
| `click_through = true` 时展开面板无法交互 | 无 | OrbExpand 中展开时临时关闭穿透，收起时恢复（由设置驱动） |
| `click_through = true` 时拖拽无法触发 | 无 | OrbDrag 中拖拽开始时临时关闭穿透，结束时恢复（由设置驱动） |
| 旧配置文件 `click_through` 字段 | 无 | 字段保留，完全兼容 |
| `ORB_SNAP_BORDER_X` 硬编码 24 未来需调整 | 低 | 可改为从配置读取，但当前无需求；24 是经过验证的贴边效果值 |
| 未来需要恢复自动轮询 | 低 | Rust 穿透基础设施已保留（§4.10），前端轮询可从 git 历史恢复；但 hitbox=窗口 后轮询无意义 |

## 7. 验证清单

### 7.1 默认行为（`click_through = false`）

- [ ] 收起态：点击球体 → 展开面板
- [ ] 收起态：点击阴影区域 → 展开面板
- [ ] 收起态：hover 阴影区域 → 球放大 + 阴影加深
- [ ] 收起态：拖拽阴影区域 → 球跟随移动
- [ ] 展开态：面板内正常交互
- [ ] 展开态：点击面板外 → 收起（blur 事件）
- [ ] 拖拽 → 松手 → 吸附屏幕边缘
- [ ] hover 隐藏功能正常（状态机与穿透无关）

### 7.2 穿透模式（`click_through = true`）

- [ ] 收起态：点击球体 → 穿透到下方窗口
- [ ] 收起态：点击球体外 → 穿透到下方窗口
- [ ] 展开态：面板内正常交互（展开时临时关闭穿透）
- [ ] 收起后：恢复穿透
- [ ] 拖拽：可拖拽（拖拽时临时关闭穿透）
- [ ] 拖拽结束后：恢复穿透
- [ ] 设置变更：切换 `click_through` 后立即生效

### 7.3 启动与兼容

- [ ] 启动时 `click_through` 设置正确生效（不依赖硬编码）
- [ ] 旧配置文件兼容（`click_through` 字段正常读取）
- [ ] 设置 UI 中"鼠标穿透"选项正常显示和切换

### 7.4 贴边吸附

- [ ] 拖拽到屏幕右侧松手 → 球体贴边（`ORB_SNAP_BORDER_X = 24` 效果不变）
- [ ] 拖拽到屏幕左侧松手 → 球体贴边
- [ ] 多屏环境下吸附正确

### 7.5 代码质量

- [ ] Rust 常量测试通过（`ORB_HITBOX_SIZE = 88`、`ORB_SNAP_BORDER_X = 24`）
- [ ] TypeScript 类型检查通过
- [ ] ESLint 通过
- [ ] 无 `ORB_VISUAL_CENTER_OFFSET_X` 残留引用
- [ ] 无 `NEEDS_CURSOR_POLL` / `NEEDS_CLICK_THROUGH_TOGGLE` 残留引用
- [ ] 无 `HOVER_POLL_INTERVAL_MS` / `CLICK_THROUGH_DEBOUNCE_MS` / `HAND_JITTER_PX` 残留引用
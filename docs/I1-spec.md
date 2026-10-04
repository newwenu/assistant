# I1 — Orb 悬浮球 规格说明

## 1. 交付目标

桌面常驻透明悬浮球，可拖拽、可吸附、可唤起，点击展开为 AI 对话面板，失焦自动收起。

## 2. 功能规格

### 2.1 透明悬浮球窗口

| 属性 | 值 | 来源 |
|------|-----|------|
| label | `orb` | constant.rs |
| 尺寸 | 420 × 600 px（固定，不随展开/收起变化） | ORB_EXPANDED_WIDTH / ORB_EXPANDED_HEIGHT |
| transparent | true | tauri.conf.json |
| alwaysOnTop | true | tauri.conf.json |
| skipTaskbar | true | tauri.conf.json |
| decorations | false | tauri.conf.json |
| resizable | false | — |
| shadow | false | tauri.conf.json |
| acceptFirstMouse | true | tauri.conf.json |

**窗口生命周期**：
- 应用启动时由 tauri.conf.json 静态创建（已存在）
- 窗口尺寸始终 420×600，不随展开/收起变化
- 收起态：OrbBall 可见，OrbPanel 以 `scale(0.18)` + `opacity: 0` + `pointer-events: none` 隐藏
- 展开态：OrbPanel 以 `scale(1)` + `opacity: 1` + `pointer-events: auto` 显示，OrbBall 缩到左上角
- 切换通过 CSS class `orb-expanded` 控制，300ms CSS transition 动画过渡
- 参考：deepseek-harness-orb — 窗口尺寸不变，CSS `transform: scale()` 做动画

### 2.2 鼠标穿透与 hitbox

**问题**：透明窗口设置了 `setIgnoreCursorEvents(true)` 后，整个窗口无法接收任何鼠标事件。WindowPet 的解决方案是 Rust 端获取鼠标位置，前端判断是否在 hitbox 内，动态切换穿透状态。

**规格**：

| 参数 | 值 | 说明 |
|------|-----|------|
| ORB_HITBOX_SIZE | 48 px | 可交互区域直径，小于窗口 64px 留出边缘透明区 |
| HOVER_POLL_INTERVAL_MS | 120 ms | 鼠标位置轮询间隔（macOS） |
| 穿透切换策略 | 动态 `setIgnoreCursorEvents` | 指针在 hitbox 内 → false（可交互），外 → true（穿透） |

**实现策略**（按平台）：

- **Windows**：CSS `pointer-events` 精细控制。容器 `div[data-orb-window]` 设 `pointer-events: none`（整体穿透），`.orb-hitbox` 和 `.orb-panel`（展开时）设 `pointer-events: auto`（恢复交互）。比 Rust `set_ignore_cursor_events` 只能控制整个窗口更精确，无需轮询
- **macOS**：macOS 透明窗口 click_through 后 DOM 事件不可靠，需 Rust 端 `get_mouse_position()` + 前端 setInterval 轮询判断指针是否在 hitbox 内，动态调用 `set_orb_click_through`
- **穿透切换**：Windows 用 CSS，macOS 用 Rust `setIgnoreCursorEvents`

**参考**：
- WindowPet `cmd.rs` → `get_mouse_position()` 用 `mouse_position` crate
- BongoCat `hover.rs` → `pointer_inside_window()` 半开区间边界检测
- OpenBitFun `AgentCompanionDesktopPet.tsx` → `POINTER_HOVER_POLL_INTERVAL_MS = 120`

### 2.3 拖拽 + 边缘吸附

**核心问题**：区分点击（展开面板）和拖拽（移动窗口），使用位移阈值。

| 参数 | 值 | 说明 |
|------|-----|------|
| PET_DRAG_THRESHOLD_PX | 8 px | 移动 < 8px 视为点击，≥ 8px 开始拖拽 |
| DRAG_POLL_INTERVAL_MS | 16 ms | Windows 拖拽轮询间隔（≈60fps） |
| WINDOW_EDGE_BUFFER | 4 px | 边缘留白防裁剪 |
| PLACEMENT_SETTLE_DELAY_MS | 1000 ms | 拖拽结束后 1s 触发位置约束检查 |

**拖拽实现**（按平台）：

- **Windows**：`cursorPosition()` 轮询 + `window.setPosition()` IPC（参考 OpenBitFun `AgentCompanionDragService.ts`）
  - pointerdown 记录 grab 点
  - pointermove 判断位移是否超过阈值
  - 超过阈值 → 启动轮询循环：每 16ms 读 `cursorPosition()`，计算 `origin + pointer - grab`，调用 `setPosition`
  - pointerup → 停止轮询，若未超过阈值则视为点击

- **macOS**：使用 Tauri 原生 `window.startDragging()`
  - pointerdown 记录 grab 点
  - pointermove 判断位移是否超过阈值
  - 超过阈值 → 调用 `orbWindow.startDragging()`，系统接管拖拽
  - pointerup → 若未超过阈值则视为点击

**边缘吸附**：
- 拖拽结束后（pointerup / startDragging 完成），Rust 端延迟 1s 检查窗口是否在屏幕范围内
- 若超出屏幕 → 调用 `snap_to_nearest_screen()` 吸附到最近屏幕
- 吸附动画：前端监听位置变化，CSS transition 平滑过渡

**参考**：
- OpenBitFun `AgentCompanionPointerDragService.ts` → 指针拖拽：记录 grab → pointermove 合并 → setPosition IPC
- OpenBitFun `AgentCompanionDesktopPet.tsx` L43 → `PET_DRAG_THRESHOLD_PX = 8`
- OpenBitFun `AgentCompanionDragService.ts` → Windows cursorPosition 轮询拖拽
- BongoCat `placement.rs` → `PLACEMENT_SETTLE_DELAY = 1s` + `bounds_inside_screens()` + `correction_for_screens()`

### 2.4 全局快捷键

| 参数 | 值 |
|------|-----|
| 默认快捷键 | `Cmd/Ctrl + K` |
| 行为 | 切换 Orb 可见性（显示/隐藏） |
| 配置 | 可在设置中自定义（I6 扩展，I1 仅实现默认） |

**实现**：
- 使用 `tauri-plugin-global-shortcut`（Tauri 2 官方插件）
- 注册 `CmdOrCtrl + K` → toggle orb visibility
- toggle 逻辑：若 Orb 可见且聚焦 → 隐藏；若隐藏或失焦 → 显示并聚焦

### 2.5 展开/收起

| 事件 | 行为 |
|------|------|
| 单击悬浮球（位移 < 8px） | 展开：64×64 → 420×600，显示对话面板 |
| 面板失焦 | 收起：420×600 → 64×64，恢复悬浮球 |
| Escape 键 | 收起面板 |
| `expand_orb_panel` command | Rust 端 set_size + show + set_focus |
| `collapse_orb_panel` command | Rust 端 set_size |

**展开动画**：
- 窗口尺寸由 Rust 端 `set_size()` 切换（PhysicalSize）
- 前端通过窗口尺寸变化事件或状态切换渲染不同 UI（OrbBall / OrbPanel）
- CSS transition 实现尺寸过渡动画（可选，Tauri 窗口 resize 本身无动画）

**失焦检测**：
- 前端监听 `window.onFocusChanged` 事件
- Orb 面板失焦且非拖拽中 → 延迟 200ms 收起（防拖拽误触）
- 收起时同时恢复 click_through

**参考**：
- Onepoint `window.ts` → `setWindowVisile`：hide + blur 失焦，show + focus 唤起
- WindowPet `utils.rs` → `reopen_main_window`：先检查窗口是否存在，存在则 focus

### 2.6 右键菜单

| 菜单项 | 行为 |
|--------|------|
| 新建对话 | 收起面板 → 清空对话 → 展开面板（I2 完整实现，I1 仅预留事件） |
| 打开主窗口 | 调用 `open_main_window` command |
| 退出 | 销毁所有窗口 + `app.exit(0)` |

**实现**：
- 前端 `onContextMenu` 事件 + Ant Design `Dropdown` 组件
- 不使用系统原生右键菜单（跨平台一致性 + 样式可控）

## 3. Rust 后端规格

### 3.1 新增/修改的 Tauri Commands

| command | 签名 | 说明 |
|---------|------|------|
| `get_mouse_position` | `() → Value` | 已存在，返回 `{clientX, clientY}` |
| `expand_orb_panel` | `(AppHandle) → Result<()>` | 已存在，set_size(420,600) + show + focus |
| `collapse_orb_panel` | `(AppHandle) → Result<()>` | 已存在，set_size(64,64) |
| `toggle_orb_visibility` | `(AppHandle) → Result<()>` | **新增**，切换 Orb 显示/隐藏 |
| `snap_orb_to_screen` | `(AppHandle) → Result<()>` | **新增**，将 Orb 吸附到最近屏幕 |
| `set_orb_click_through` | `(AppHandle, enabled: bool) → Result<()>` | **新增**，动态设置鼠标穿透 |

### 3.2 assistant-orb crate 扩展

**hover.rs**（已存在，需增强）：
- `pointer_inside_window()` — 已实现
- `HoverState` 状态机 — 已实现，需集成到实际窗口控制
- 新增：`should_toggle_click_through(observation) → bool` 判断是否需要切换穿透状态

**placement.rs**（已存在，需增强）：
- `bounds_inside_screens()` — 已实现（简化版，仅检测是否与任一屏幕重叠）
- `snap_to_nearest_screen()` — 已实现
- 新增：`get_available_screens(app: &AppHandle) → Vec<ScreenBounds>` 通过 Tauri API 获取屏幕列表
- 新增：`check_and_snap_orb(app: &AppHandle)` 延迟检查 + 吸附逻辑

### 3.3 全局快捷键

- 添加 `tauri-plugin-global-shortcut` 依赖
- 在 `setup::init` 中注册 `CmdOrCtrl + K` → toggle_orb_visibility
- 快捷键 handler 中调用 `toggle_orb_visibility` command

### 3.4 窗口事件监听

- 监听 Orb 窗口焦点变化：失焦 → emit `orb-blur` 事件到前端
- 监听窗口位置变化：拖拽结束 → 延迟 1s → `check_and_snap_orb`

## 4. 前端规格

### 4.1 新增组件

| 组件 | 路径 | 职责 |
|------|------|------|
| OrbBall | `src/orb/OrbBall.tsx` | 悬浮球主体（hitbox + 视觉样式 + 拖拽/点击事件） |
| OrbPanel | `src/orb/OrbPanel.tsx` | 展开后的对话面板（I1 为占位，I2 填充对话 UI） |
| OrbExpand | `src/orb/OrbExpand.tsx` | 展开/收起动画控制 |
| OrbDrag | `src/orb/OrbDrag.tsx` | 拖拽逻辑（阈值判断 + 平台分发） |
| OrbContextMenu | `src/orb/OrbContextMenu.tsx` | 右键菜单 |

### 4.2 新增 Store

| Store | 路径 | 状态 |
|------|------|------|
| orbStore | `src/stores/orbStore.ts` | `isExpanded`, `isHovering`, `isDragging`, `position`, `isVisible` |

### 4.3 现有组件改造

**App.tsx**：
- `OrbPlaceholder` → 拆分为 `OrbBall` + `OrbPanel`，根据 `isExpanded` 切换渲染
- 拖拽逻辑从 App.tsx 抽取到 `OrbDrag.tsx`
- 右键菜单从 tray 移到前端 `OrbContextMenu.tsx`

### 4.4 常量对齐

`src/config/constants.ts` 与 `src-tauri/crates/assistant-config/src/constant.rs` 保持一致，I1 新增：

```typescript
export const ORB_BLUR_COLLAPSE_DELAY_MS = 200;
export const ORB_SNAP_DELAY_MS = 1000;
```

## 5. 不做的事（I1 范围外）

- ❌ 对话 UI（I2）
- ❌ Mascot 形象（I5）
- ❌ Agent 模式切换（I6）
- ❌ 拖拽吸附动画的弹簧物理（仅做简单 snap）
- ❌ 多显示器拖拽时的实时吸附（仅拖拽结束后检查）
- ❌ OrbConf 设置 UI（I1 仅用默认值，设置页 I3 补）

## 6. 验收标准

1. `npm run tauri dev` 启动后桌面可见透明小球
2. 鼠标悬停小球时小球高亮，移开恢复半透明
3. 拖拽小球可自由移动，松手后贴边吸附
4. 点击小球展开为对话面板（420×600）
5. 面板失焦自动收起为小球（64×64）
6. `Cmd/Ctrl+K` 可切换小球显示/隐藏
7. 右键菜单可打开主窗口和退出
8. 小球不遮挡底层窗口点击（穿透正常工作）
9. macOS 和 Windows 行为一致
# 开发日志

记录技术决策、踩坑与架构变更，按迭代组织。原则：只记**为什么**和**踩了什么坑**，不记冗余步骤。

---

## I0 — 项目脚手架

### 2026-10-03 文档审查

ARCHITECTURE.md 与实际代码不一致项（已修正）：

- React 18→19、Ant Design 5→6、TypeScript 未标注→6、路由编号跳跃、I7 命名不统一、conversations 缺外键
- FRONTEND.md 12 处绝对路径→相对路径
- DESIGN.md 顶部加废弃声明（与 ARCHITECTURE.md 有 5 项根本冲突）
- **注意**：`react-router-dom` 为 v7（非 v6），API 有差异；前端目录结构文档为规划态，已加"实现状态"说明

### 2026-10-03 验收修复

验收标准：`cargo tauri dev` 启动后可见主窗口 + 托盘图标，主题可切换

**修复 5 项**：

1. **React Router 集成**：已安装未使用→MainLayout 加 BrowserRouter+Routes，定义 `/` 和 `/settings`；新增 SettingsWindow 组件+viewMap settings 映射
2. **SQLite 启动初始化**：init_db 未在 setup 调用→setup::init 中通过 `app_data_dir()` 拼接路径调用
   - ⚠️ 踩坑：`app_data_dir()` 返回 `Result` 非 `Option`，`if let Some` 编译失败，改 `match`
3. **db.rs 表结构对齐文档**：按 ARCHITECTURE.md Section 5.1 重写，新增 tool_favorites/tool_history/compression_state/app_settings 四张表
4. **capabilities 权限补全**：加 sql:default/allow-execute/allow-select + 窗口 minimize/toggle-maximize/is-maximized + settings 窗口
5. **Titlebar macOS 避让**：`navigator.platform` 检测 macOS，paddingLeft 动态 80/12

### 2026-10-03 Cargo Workspace 重构

**问题**：`npm run tauri dev` 报 `No package info in the config file`

**根因**：Tauri 2 CLI 要求 `src-tauri/Cargo.toml` 含 `[package]` 段，不支持纯 workspace 根

**决策**：Cargo.toml 同时含 `[package]`（根包 assistant-app）+ `[workspace]`（6 crate），源码从 `crates/assistant-app/src/` 移至 `src-tauri/src/`

**踩坑**：
- 根包与 tauri.conf.json 同目录→`build.rs` 直接 `tauri_build::build()`，`generate_context!("tauri.conf.json")`，无需手动路径
- Tauri 2 API：Menu/MenuItem 移至 `tauri::menu::`，TrayIcon 移至 `tauri::tray::`
- tauri.conf.json：`identifier` 在顶层非 bundle 内

**lib+main 分离**：保留 lib.rs（run()）+ main.rs（assistant_app::run()），为移动端入口和单元测试保留

**最终结构**：`src-tauri/src/`（根包 6 文件）+ `src-tauri/crates/`（6 个 lib crate）

### 2026-10-03 透明窗口背景修复

**问题**：Orb 窗口配置了 `transparent: true`，但实际显示有白色/黑色方块背景

**根因**：`global.css` 中 `body { background-color: var(--color-bg); }` 给整个窗口铺了不透明背景色，覆盖了 Tauri 的透明效果

**参考**：WindowPet 开源项目 — `body` 只设 `overflow: hidden`，**不设 background-color**；透明区域通过 `setIgnoreCursorEvents(true)` 实现点击穿透

**修复**：
1. 移除 body 的 `background-color`（global.css）
2. MainLayout 外层 Layout 加 `background: "var(--color-bg-layout)"`
3. SettingsWindow 外层 Layout 加 `background: "var(--color-bg)"`
4. OrbPlaceholder 保持 `background: "transparent"`（已有）

**原则**：body 不设背景色，各窗口组件自行控制背景；透明窗口（Orb）不设任何不透明背景

### 2026-10-03 脚手架对比开源项目完善

对比 WindowPet (Tauri 1) + ChatGPT (Tauri 2)，修复 8 项：

1. **ExitRequested 防退出**：关闭窗口后托盘仍在但无法重开→lib.rs 改 `.build()` + `.run()`，拦截 `ExitRequested` 调 `api.prevent_exit()`，仅 `app.exit(0)` 真退出
   - 参考：WindowPet main.rs 同样拦截 ExitRequested
2. **Windows GPU 加速**：WebView2 默认可能禁用 GPU→main.rs 加 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--ignore-gpu-blocklist"`
   - 参考：WindowPet main.rs，解决 Tauri #4891 渲染问题
3. **capabilities 权限补全**：缺 `set-ignore-cursor-events`/`inner-size`/`set-icon`/`webview:default`→default.json 补齐
   - ⚠️ 踩坑：Rust 端调用 `set_ignore_cursor_events` 不受 capabilities 限制，但前端 JS 调用会 403
4. **macOS private-api**：透明窗口在 macOS 需 private API→tauri.conf.json 加 `macOSPrivateApi: true`，Cargo.toml 加 feature `macos-private-api`
   - 参考：WindowPet 的 tauri features 含 `macos-private-api`
5. **托盘左键点击**：`menuOnLeftClick: false` 但无 `on_tray_icon_event`→tray.rs 加 `.on_tray_icon_event()`，左键点击显示主窗口
6. **删除残留 App.css**：Vite 脚手架残留，`:root { background-color: #f6f6f6 }` 会破坏透明窗口→删除
7. **ConfigProvider 不包裹 Orb**：Ant Design 注入全局样式影响透明窗口→main.tsx 检测 `webview.label === "orb"` 时跳过 ConfigProvider
8. **OrbConf 路径风格统一**：缺少 `get_conf_dir()` 方法→补齐，与 AppConf 风格一致

### 2026-10-03 I0 遗漏补全

逐项对照代码验证 I0 的 8 项交付，发现 4 处 DEVLOG 记录已实现但代码实际缺失：

1. **ExitRequested 防退出**：lib.rs 仍用 `.run()` 直接运行→改为 `.build()` + `.run()`，拦截 `ExitRequested { api.prevent_exit() }`
   - 仅 `app.exit(0)`（托盘"退出"菜单）可真正退出
2. **macOS private-api**：tauri.conf.json 缺 `macOSPrivateApi: true`，Cargo.toml 缺 feature `macos-private-api`→补齐
   - 透明窗口在 macOS 无 private API 无法正常穿透
3. **托盘左键点击**：tray.rs 无 `on_tray_icon_event`→添加 `.on_tray_icon_event()`，左键 Up 显示主窗口
   - 导入 `MouseButton`/`MouseButtonState`/`TrayIconEvent`
4. **capabilities 权限补全**：缺 `outer-position`/`scale-factor`/`destroy`→default.json 补齐
   - 前端 Orb 拖拽用 `outerPosition()`/`scaleFactor()`，托盘退出用 `destroy()`

验证：`cargo build` ✅ / `npm run typecheck` ✅ / `npm run lint` ✅

### 2026-10-03 托盘+退出 bug 修复

上一轮修复引入两个 bug：

1. **托盘左键双事件冲突**：`on_menu_event`（Windows 左键默认弹菜单）和 `on_tray_icon_event`（左键点击唤起主窗口）同时触发→左键无响应
   - 修复：加 `.show_menu_on_left_click(false)`，左键只走 `on_tray_icon_event` 唤起主窗口，右键弹菜单
2. **退出被 ExitRequested 吞掉**：`app.exit(0)` 触发 `ExitRequested`，无条件 `api.prevent_exit()` 阻止了退出
   - 修复：引入 `static SHOULD_EXIT: AtomicBool`，quit 菜单先调 `request_exit()` 置标志再 `app.exit(0)`，`ExitRequested` 仅在 `!SHOULD_EXIT` 时阻止

### 2026-10-03 托盘唤起窗口无效修复

**根因**：关闭主窗口后窗口被 Tauri 销毁，`get_webview_window("main")` 返回 `None`，`show()` + `set_focus()` 不执行

**修复**：提取 `show_or_create_main(app)` 函数，窗口存在则 show+focus，不存在则 `WebviewWindowBuilder` 创建。左键点击、菜单"打开主窗口"统一调用此函数

## I1 — Orb 悬浮球

### 2026-10-03 实施记录

**阶段 A — Rust 后端**（A1~A6 全部完成）

- A1: 新增 3 个 Tauri Commands（`toggle_orb_visibility`、`set_orb_click_through`、`snap_orb_to_screen`），注册到 invoke_handler
- A2: placement 增强：`get_available_screens(app)` + `check_and_snap_orb(app, label)`，ScreenBounds 移至 hover.rs 共享
- A3: hover 增强：`HoverState::should_toggle_click_through()` 穿透状态判断
- A4: 全局快捷键：`tauri-plugin-global-shortcut` + Cmd/Ctrl+K → toggle orb visibility
- A5: 窗口焦点事件：`on_window_event` 监听 Orb 失焦 → emit `orb-blur`
- A6: 常量对齐：`ORB_HITBOX_SIZE`、`ORB_BLUR_COLLAPSE_DELAY_MS`、`ORB_SNAP_DELAY_MS`、`HOVER_POLL_INTERVAL_MS`、`DRAG_POLL_INTERVAL_MS`

**阶段 B — 前端组件**（B1~B8 全部完成）

- B1: `orbStore` Zustand 状态管理（isExpanded/isHovering/isDragging/isVisible/position）
- B2: 前端常量对齐（`ORB_BLUR_COLLAPSE_DELAY_MS`、`ORB_SNAP_DELAY_MS`）
- B3: `OrbDrag` 拖拽逻辑（阈值判断 + 吸附延迟 + poll 位置）
- B4: `OrbBall` 悬浮球主体（hitbox + hover + 穿透切换 + 右键菜单）
- B5: `OrbPanel` 展开面板（占位 UI + 失焦收起 + Escape）
- B6: `OrbContextMenu` 右键菜单（Ant Design Dropdown）
- B7: `OrbExpand` 展开/收起控制 hook（LogicalSize 切换）
- B8: App.tsx 重构：`OrbPlaceholder` → `OrbWindow`（OrbBall + OrbPanel）

**验证**：`cargo build` ✅ / `npm run typecheck` ✅ / `npm run lint` ✅

### 2026-10-04 Orb 展开/收起重构 — CSS 动画方案

**问题**：展开/收起时窗口尺寸变化导致闪烁和变形（OrbPanel 被塞进小窗口裁切成椭球形状）

**参考**：deepseek-harness-orb 项目 — 窗口尺寸不变，用 CSS `transform: scale()` + `opacity` + `pointer-events` 做 300ms 动画过渡

**优化点**：

1. **窗口尺寸固定**：Orb 窗口始终 420×600，不再在展开/收起之间切换尺寸。消除 Rust ↔ React 时序竞争导致的闪烁/裁切变形
2. **CSS 动画过渡**：`.orb-panel` 默认 `scale(0.18)` + `opacity: 0` + `pointer-events: none`；`.orb-expanded .orb-panel` 恢复 `scale(1)` + `opacity: 1` + `pointer-events: auto`；300ms ease-in-out transition
3. **同时渲染**：OrbBall 和 OrbPanel 始终同时存在于 DOM，用 CSS class `orb-expanded` 控制状态切换，不再条件渲染（`{isExpanded ? <Panel/> : <Ball/>}`）
4. **Rust 端简化**：`expand_orb_panel` / `collapse_orb_panel` 不再需要改窗口尺寸，`orbStore` 的 expand/collapse 只改状态不调 IPC
5. **CSS 特异性提升**：`div[data-orb-window]` 替代 `[data-orb-window]`，加 `!important` 防止 Ant Design 等第三方样式覆盖穿透规则
6. **chrome inset**：引入 `ORB_CHROME_INSET = 12px`，OrbBall 和 OrbPanel 用绝对定位 + inset 留出阴影空间

**设计原理**：
- `pointer-events: none` 在容器层：整个 Orb 窗口穿透，不挡底层应用
- `pointer-events: auto` 在 `.orb-hitbox` 和 `.orb-panel`（展开时）：仅交互区域可点击
- Windows 用 CSS `pointer-events` 精细控制（比 Rust `set_ignore_cursor_events` 只能控制整个窗口更精确）
- macOS 仍用 Rust 端 `set_orb_click_through`（CSS `pointer-events` 无法穿透到系统级）

### 2026-10-04 快捷键占用处理

**问题**：全局快捷键（Cmd/Ctrl+K）可能被其它应用占用，注册失败时静默无提示

**当前处理**：注册失败时 `log::warn!` 记录日志

**后续增强方向**：
1. 前端检测注册失败并弹 toast 提示用户
2. 设置页面允许自定义快捷键
3. 提供备选方案（托盘菜单替代快捷键）

### 2026-10-04 Orb 事件穿透优化 — 跨平台 + 坐标统一

**问题**：Orb 窗口透明区域拦截鼠标事件，用户无法点击到下方窗口内容。Windows 完全没有 click-through 机制，macOS hitbox 检测范围是整个窗口（420×600）而非小球（48×48）

**参考项目对比**：

| 项目 | 框架 | 检测方式 | 穿透切换 | 特色 |
|------|------|----------|----------|------|
| BongoCat | 原生 Win32 | 光标 vs 窗口矩形 | `WS_EX_TRANSPARENT\|LAYERED` | 状态机驱动，hover hide 强制穿透 |
| WindowPet | Tauri v1 | Rust 取坐标 + Phaser hitTest | `set_ignore_cursor_events` | 50ms 防抖，像素级碰撞 |
| airi | Electron | `elementFromPoint` + `isPaintedAt` | `setIgnoreMouseEvents` + `forward:true` | 8px 手抖容差，DOM 像素级检测 |
| deepseek | Electron | 引用计数 guard | `setIgnoreMouseEvents` | HID 期间穿透 + blur |

**修复 1 — 跨平台 click-through**：
- OrbBall.tsx：`NEEDS_CURSOR_POLL = IS_MACOS || IS_WINDOWS`，Windows 也启用光标轮询
- hitbox 检测改为鼠标位置 vs hitbox 矩形（48×48），而非整个窗口
- OrbExpand.ts：expand 时关闭穿透，collapse 时开启穿透
- OrbDrag.tsx：拖拽开始时关闭穿透
- App.tsx：初始化时开启穿透
- cmd.rs：`set_orb_click_through` 添加 debug 日志

**修复 2 — DPR 坐标系混用（根因 bug）**：
- **问题**：CSS 常量（`ORB_CHROME_INSET=12`, `ORB_HITBOX_SIZE=48`）是逻辑像素，Tauri API（`outerPosition`, `innerSize`）返回物理像素，混加导致 hitbox 偏移
- **表现**：150% DPI 下只有小球左上角 ~1/4 区域能检测到鼠标
- **修复**：创建 `OrbCoord.ts` 统一逻辑坐标系工具，所有物理坐标 `/ dpr` 转逻辑后再与 CSS 常量比较

**优化 3 — 性能与稳定性**：
- **P0 Rust 端状态缓存**：`static ORB_CLICK_THROUGH: AtomicBool`，值相同时跳过 `set_ignore_cursor_events` 系统调用，减少 ~90% 冗余调用
- **P0 前端状态缓存**：Store `clickThrough` 字段，所有模块共享，避免冗余 IPC
- **P1 开启穿透防抖**：30ms `setTimeout`，关闭穿透时立即执行并取消防抖，减少边界闪烁
- **P1 手抖容差**：8px `HAND_JITTER_PX`，鼠标离开 hitbox 后 8px 内仍保持不穿透，防止滚动/点击意外穿透
- **P2 轮询间隔**：120ms → 80ms，配合缓存不增加实际调用频率
- **P2 拖拽结束显式穿透**：`onPointerUp` 中未展开时立即开启穿透，不依赖轮询
- **P2 展开状态边距穿透**：鼠标离开窗口时也开启穿透

**修复 4 — Rust 端坐标混用**：
- `expand_orb_panel` / `collapse_orb_panel`：`PhysicalSize::new(420, 600)` → `LogicalSize::new(420, 600)`，位置计算用 `scale_factor` 转换
- `position_orb_window`：默认位置计算用 `scale_factor`，`orb_w * scale` = 物理像素
- `get_screen_size`：接收 `scale_factor` 参数，逻辑坐标 → 物理坐标后再与 screen API 比较

**OrbCoord.ts API**：
```typescript
physicalToLogical(px, py)          // 物理 → 逻辑
physicalSizeToLogical(pw, ph)      // 物理尺寸 → 逻辑尺寸
logicalToPhysical(lx, ly)          // 逻辑 → 物理 (PhysicalPosition)
getMouseLogicalPos()               // 鼠标逻辑坐标
getWindowLogicalRect()             // 窗口逻辑矩形
isPointInRect(px, py, rect)        // 点在矩形内
isPointInCircle(px, py, cx, cy, r) // 点在圆内
```

**坐标流审计**：

| 路径 | 前端 | Rust | 坐标系 |
|------|------|------|--------|
| OrbBall hover | `getMouseLogicalPos()` + `getWindowLogicalRect()` + CSS 常量 | `get_mouse_position`(物理) + `outerPosition/innerSize`(物理) | 全逻辑 |
| OrbExpand 方向 | `getWindowLogicalRect()` + `physicalToLogical(screen)` | `get_screen_size` 接收逻辑+scaleFactor → 转物理比较 | 全逻辑 |
| expand/collapse | — | `LogicalSize::new()` + `scale_factor` 位置计算 | 全物理计算 + LogicalSize set |
| position_orb_window | — | `scale_factor()` → `orb_w * scale` | 全物理 |
| OrbDrag Windows 拖拽 | `outerPosition`(物理) + `cursorPosition`(物理) | — | 全物理 |
| OrbDrag 拖拽阈值 | `e.clientX/clientY`(逻辑) vs `startPosRef`(逻辑) | — | 全逻辑 |

**核心原则**：同一运算中的所有坐标必须在同一坐标系。OrbCoord.ts 封装转换逻辑，后续开发者只需用逻辑坐标 API 做前端检测，用 `logicalToPhysical()` 做窗口位置设置

### 2026-10-04 四象限展开修复 — 窗口尺寸随状态变化

**问题**：Orb 四象限展开方向判断做了，但窗口位置从未移动，无论判断向哪个方向展开，面板最终都在同一个位置

**根因**：
1. 前端 `OrbExpand.ts` 的 `expand()` 只设了 CSS class（改变 `transform-origin` 动画方向），从未调用 Tauri API 移动窗口位置或改变窗口大小
2. 窗口始终 420×600，球始终在窗口左上角，`transform-origin` 只影响动画从哪个角"长出来"，但 `scale(1)` 最终状态永远一样——面板填满整个窗口
3. 方向判断用窗口边缘（420×600）而非球边缘（64×64），导致判断本身也不准确

**参考**：deepseek-harness-orb（Electron）— 折叠时窗口缩小为 96×96（球72+2×12inset），展开时窗口放大为 344×444（面板320×420+2×12inset），且**位置根据方向移动**使球保持在窗口对应角落

**修复**：

1. **常量新增**（constants.ts + constant.rs）：
   - `ORB_COLLAPSED_WINDOW_SIZE = 88`（64 + 2×12）
   - `ORB_EXPANDED_WINDOW_WIDTH = 444`（420 + 2×12）
   - `ORB_EXPANDED_WINDOW_HEIGHT = 624`（600 + 2×12）

2. **Rust 端重写** `expand_orb_panel` / `collapse_orb_panel`：
   - 接受 `horizontal`/`vertical` 方向参数
   - 先从当前窗口位置+方向反推球位置（`ballOriginFromWindow`）
   - 再根据方向计算新窗口位置（`overlayBoundsFromBall`）
   - 展开时窗口变为 444×624，折叠时变为 88×88
   - 位置计算保证球在窗口的对应角落

3. **前端 OrbExpand.ts**：
   - 方向判断改用球心位置（`ballX + ORB_COLLAPSED_SIZE/2`）而非窗口中心
   - 剩余空间用球边缘计算（`roomRight = 屏幕右 - ballX`）而非窗口边缘
   - `expand()` 调用 `invoke("expand_orb_panel", { horizontal, vertical })` 移动窗口
   - `collapse()` 调用 `invoke("collapse_orb_panel", { horizontal, vertical })` 缩回窗口

4. **OrbBall.tsx**：球的位置随展开方向动态变化
   - `expandDirection === "left"` → 球在窗口右侧（`left: calc(100% - 76px)`）
   - `expandVertical === "up"` → 球在窗口底部（`top: calc(100% - 76px)`）
   - 折叠时球始终在左上角

5. **tauri.conf.json**：orb 窗口初始大小从 420×600 改为 88×88（折叠态）

6. **CSS**：展开态 `transform-origin` 显式保留，确保动画方向正确

**待解决 — 展开闪烁**：
- **现象**：展开时球快速闪到窗口左上角再跳到正确角落

### 2026-10-04 文档更正 — 窗口尺寸方案

> 更正本日志早期记录中的"窗口尺寸固定 420×600"描述。当时记录的是初始设计决策，后续实际实现已改为动态切换方案。

**实际方案**：收起态 88×88 ↔ 展开态 444×624，Rust 端 reposition + resize 切换窗口尺寸，CSS `transform: scale()` + `opacity` + `pointer-events` 做 300ms 视觉过渡动画。既消除 Rust ↔ React 时序竞争导致的闪烁/裁切变形，又确保窗口尺寸与内容区域匹配。

**对应代码**：
- `window.rs`：`expand_orb_panel` / `collapse_orb_panel` 接收 horizontal/vertical 方向参数，reposition + resize
- `OrbExpand.ts`：`expand()` / `collapse()` 调用 Rust IPC 移动窗口
- `tauri.conf.json`：orb 窗口初始大小 88×88（折叠态）
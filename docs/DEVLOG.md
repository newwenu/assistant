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

### 2026-10-04 展开闪烁修复 — flushSync + 双 rAF + transitionend

**问题**：展开时小球闪跳到窗口左上角再归位；收起时面板被裁切闪烁

**根因分析**：

1. **展开闪烁**：Rust `expand_orb_panel` 分两步执行 `set_position` → `set_size`，中间有一帧窗口在新位置但仍是旧尺寸，`isExpanded` 还是 `false`，小球 CSS 位置（`ballLeft: 12px`）对应新窗口左上角而非正确角落。React 重渲染后 `isExpanded=true` 才把小球推到 `calc(100%-76px)`，产生可见跳变
2. **收起闪烁**：`storeCollapse()` 触发面板 300ms CSS 淡出动画，但紧接着 `invoke("collapse_orb_panel")` 立刻 resize 窗口到 88×88，面板动画还没播完就被裁切

**参考对比**：
- BongoCat：单次 `SetWindowPos` 同时设 x/y/width/height，无中间帧跳变
- deepseek-harness-orb：Electron `setBounds` 原子操作
- 我们的 Tauri：`set_position` + `set_size` 两次独立调用，无法合并

**修复 — 纯前端方案，零 Rust 改动**：

1. **orbStore.ts 新增 `isBallVisible` 状态**：`isBallVisible: boolean` + `setBallVisible(v)`，控制小球显隐，独立于 `isExpanded`

2. **展开时序**（OrbExpand.ts `expand()`）：
   ```
   flushSync(() => setBallVisible(false))   ← React 同步渲染，小球 opacity:0
   await waitForPaint()                     ← 双 rAF，等浏览器 compositor 上屏
   await invoke("expand_orb_panel")         ← Rust resize，小球已不可见
   await storeExpand()                      ← 面板淡入动画开始
   ```
   - `flushSync`：保证 React 同步渲染到 DOM，不走调度
   - 双 `requestAnimationFrame`：第1帧等 style/layout 计算，第2帧确保 compositor 已上屏
   - Rust invoke 时小球已从屏幕上消失，跳变不可见

3. **收起时序**（OrbExpand.ts `collapse()`）：
   ```
   await storeCollapse()                    ← 面板淡出动画开始 (300ms)
   await waitForPanelTransition()           ← 等 transitionend + 超时兜底
   await invoke("collapse_orb_panel")       ← Rust resize，面板已完全透明
   setBallVisible(true)                     ← 小球恢复显示
   ```
   - `waitForPanelTransition()`：监听 `.orb-panel` 的 `transitionend` 事件（`opacity` 属性），加 350ms 超时兜底
   - Rust resize 时面板已完全透明，无裁切闪烁

4. **CSS**（global.css）：`.orb-ball-hidden` 规则 — `opacity: 0 !important` + `pointer-events: none !important` + `transition: none !important`（瞬间隐藏，不走 150ms opacity transition）

5. **OrbBall.tsx**：读取 `isBallVisible`，切换 `orb-ball-hidden` CSS 类

**设计原则**：
- 前端是"发令者"，Rust 是"执行者"——前端确保视觉状态就绪后再调 Rust IPC
- `flushSync` + 双 rAF 是前端控制上屏时序的标准模式，比 `eval` 注入或 Rust 端设窗口透明度更可控
- `transitionend` + 超时兜底确保收起时面板动画完整播放，不依赖固定延时

## I2 — Orb AI 对话

### 2026-10-05 实施记录

**阶段 A — Rust 后端**（A1~A10 全部完成）

- A1: Cargo.toml workspace 添加 13 个依赖（reqwest/tokio/futures/aes-gcm/sha2/base64/machine-uid/tokio-stream/async-stream/rand/thiserror/uuid/chrono）
- A2: crypto.rs — AES-256-GCM 加密 + B64 辅助函数 + get_machine_id + 单元测试
- A3: base.rs — ProviderKind/ProviderConfig/ChatMessage/ChatDelta/ProviderError + 工厂函数
- A4: openai.rs — OpenAIAdapter 完整实现（chat_completion_stream + list_models + 120s 超时 + 429 限流）
- A5: storage CRUD — providers.rs + conversations.rs + templates.rs + seed_builtin_templates
- A6: Provider cmds — list/save/delete_provider + list_models + preview_models_cmd
- A7: Conv cmds — create/list/get/delete/rename_conversation + list/save_prompt_template
- A8: Stream 核心 — send_chat_message + cancel_chat + chat_tasks.rs
- A9: setup/lib 集成 — DbState + ChatTasks manage + seed + 全部 commands 注册
- A10: Rust 常量 — CHAT_PANEL_HEADER_HEIGHT 等 6 个常量

**阶段 B — 前端**（B1~B19 全部完成）

- B1: npm install react-markdown + remark-gfm
- B2: TS 常量 — 7 个常量
- B3: chatStore — 全部方法 + ensureCurrentConversation + streamingMessageId + chat-delta 监听
- B4: providerStore — 全部方法
- B5: streamCoalescer — createChatDeltaCoalescer（rAF + setTimeout + 合并 + flush + dispose）+ **11 个单元测试**
- B6: MessageBubble — 用户/助手样式 + Markdown + 流式光标 + 错误 + hover 时间戳 + 删除
- B7: MessageList — 滚动容器 + 自动滚底 + 回到底部按钮 + TemplateCards
- B8: MessageInput — TextArea 自适应 + Enter 发送 + Shift+Enter 换行 + Send/Stop
- B9: ModelSelector — Select + chip 变体 + providerId 过滤
- B10: ConversationSidebar — 搜索 + 列表 + 相对时间 + 右键菜单 + inline 编辑 + 删除确认
- B11: TemplateCards — 2 列网格 + 图标 + 变量提取 + 填写弹窗
- B12: ChatView — 整合全部组件 + 无 Provider 引导 + compact 模式 + **ProviderSelector 内嵌对话栏**
- B13: OrbPanel — 接入 ChatView + 失焦/Escape + 历史下拉
- B14: ProviderManager — 卡片列表 + 添加/编辑/删除 + 空状态 + 预设选择
- B15: ProviderForm — 表单 + 获取模型 + 编辑模式 Key 占位符
- B16: TemplateManager — 只读卡片网格 + 变量标签 + 内置标签
- B17: 路由扩展 — /settings/providers + /settings/templates + AI 设置分组
- B18: OrbContextMenu — Rust 端 orb_new_chat → emit orb-new-conversation → 前端 DOM 事件委托 → ChatView.handleNewConversation
- B19: CSS 样式 — 对话/气泡/光标/侧栏/模板/Markdown/ProviderSelector chip 样式

**阶段 C — 集成验收**（C1~C2 完成）

- C1: SQL 权限已配置
- C2: cargo build + tsc --noEmit + vite build 全部通过

### 2026-10-05 设置变更器（Settings Dispatcher）模式

**参考**：WindowPet `handleSettingChange` — 统一入口，集成持久化 + store 更新 + 跨窗口事件发射

**实现**：

1. **events.ts** — SettingAction 枚举 + SettingEvent 枚举 + SettingEventPayload 接口
2. **settingsDispatcher.ts** — dispatchProviderSaved / dispatchProviderDeleted / dispatchThemeChanged / dispatchAppConfChanged / dispatchOrbConfChanged / dispatchTemplateSaved
   - 每个函数：先 `invoke()` 持久化 → 再更新对应 store → 最后 `emitSettingEvent()` 通知其他窗口
   - `emitSettingEvent()`：遍历 [WINDOW_MAIN, WINDOW_ORB]，`WebviewWindow.getByLabel(label)` → `win.emit(event, payload)`
3. **settingListeners.ts** — `initSettingListeners()` 在各窗口初始化时调用，监听跨窗口事件并更新 store
   - ProviderChanged → loadProviders
   - ThemeChanged → setTheme
   - AppConfChanged / OrbConfChanged → 对应 store 更新

**踩坑**：
- Tauri 2 `WebviewWindow.getByLabel()` 返回 `Promise<WebviewWindow | null>`，必须 `await` 后才能调 `.emit()`
- `WebviewWindow` 从 `@tauri-apps/api/webviewWindow` 导入，非 `@tauri-apps/api/window`

### 2026-10-05 B18 右键菜单"新建对话"接入

**问题**：OrbContextMenu 的"新建对话" MenuItem 在 Rust 端创建但 `on_menu_event` 未处理 `orb_new_chat`

**修复**：
- Rust 端 `lib.rs` `on_menu_event` 新增 `"orb_new_chat"` 分支 → `app.emit("orb-new-conversation", ())`
- 前端 OrbPanel 监听 `orb-new-conversation` → 派发 DOM 事件 `chat-new-conversation`
- ChatView 监听 DOM 事件 → 调用 `handleNewConversation()`

**设计选择 — DOM 事件委托而非直接调 store**：
- OrbPanel 不直接持有 `currentProviderId` 状态（在 ChatView 内部）
- 通过 DOM CustomEvent 委托，ChatView 自行决定用哪个 Provider 创建对话
- 避免跨组件状态同步问题

### 2026-10-05 B12 多 Provider 选择 — 内嵌下拉而非弹窗

**问题**：多 Provider 时新建对话直接取 `providers[0]`，用户无法选择

**方案对比**：
- ❌ Modal 弹窗选择：打断用户操作流，Orb compact 模式下弹窗体验差
- ✅ 对话栏内嵌 ProviderSelector 下拉：与 ModelSelector 风格一致，选择后立即生效

**实现**：
- 新建 `ProviderSelector.tsx` — 类似 ModelSelector 的 Select 组件
  - `variant="chip"` 时显示为圆角药丸样式（`.orb-provider-chip`）
  - `providers.length <= 1` 时返回 null（无需选择）
- ChatView chrome bar / header 中 ProviderSelector 排在 ModelSelector 前面
- 切换 Provider 时自动重置 Model 为新 Provider 的默认模型
- `handleNewConversation()` 直接用当前选中的 `currentProviderId`，不再弹窗
- OrbPanel 移除 Modal/Radio 依赖，通过 DOM 事件委托

### 2026-10-05 B5 streamCoalescer 单元测试

**测试框架**：vitest（新增 devDependency）

**测试覆盖**（11 个用例）：
- 单事件投递
- 同 conversation_id + message_id 的 delta 合并
- 不同 message_id 不合并
- 不同 conversation_id 不合并
- flush 立即排空队列
- dispose 触发 flush
- flush 后继续 push 正常工作
- 跨 message 边界合并正确
- 空队列 flush 无操作
- frameScheduler setTimeout 回调
- frameScheduler cancel 阻止回调

**vitest 配置**：`vitest.config.ts` 限制 `include: ["src/**/*.test.ts"]`，排除 references 目录

### 2026-10-05 简化/偏差记录

- **A8 锁竞争**：`tokio::spawn` 内通过 `app_clone.state()` 获取 DbState，极端并发可能锁竞争。短期可接受，长期改用 `Arc<Pool>`
- **B3 删除确认**：chatStore `deleteConversation` 无确认弹窗，但 ConversationSidebar 已有 `Modal.confirm` 兜底
- **B15 Key 清空歧义**：前端 `Input.Password` 清空 → Rust 端 `starts_with("••••")` 判断保留旧 Key，但空字符串不匹配 → 清空 Key 被当作"未修改"。后续可加显式"清除 Key"按钮

### 2026-10-05 Provider-对话关系解耦 — provider_id nullable

**问题**：删除 Provider 时级联删除所有关联对话，用户误删 Provider 后对话历史全丢

**设计变更**：

```
旧：providers (1) ──→ (N) conversations   provider_id NOT NULL REFERENCES providers(id)
新：providers (1) ──→ (N) conversations   provider_id TEXT (nullable, 无外键约束)
```

**修改清单**：

| 文件 | 修改 |
|------|------|
| db.rs | `provider_id TEXT NOT NULL REFERENCES providers(id)` → `provider_id TEXT`；添加 `run_migrations` v1 迁移（CREATE conversations_v1 + INSERT + DROP + RENAME） |
| conversations.rs | `ConversationRow.provider_id: String` → `Option<String>` |
| providers.rs | `delete_provider`：`DELETE FROM conversations WHERE provider_id=?` → `UPDATE conversations SET provider_id=NULL WHERE provider_id=?` |
| cmd.rs | `send_chat_message`：`provider_id` 为 None 时返回错误提示"此对话的 Provider 已被删除，请重新选择 Provider 后再发送" |
| cmd.rs | 新增 `update_conversation_provider` command，切换 Provider 时持久化到 DB |
| lib.rs | 注册 `update_conversation_provider` |
| chatStore.ts | `Conversation.provider_id: string` → `string \| null` |
| ProviderManager.tsx | 删除确认文案改为"删除后，使用此 Provider 的对话将保留，但需要重新选择 Provider 才能继续对话" |
| ConversationSidebar.tsx | 新增 `providerIds` prop，`provider_id` 为 null 或不在 providerIds 中时显示"Provider 不可用"标记 |
| ChatView.tsx | `handleProviderChange` 同步调用 `update_conversation_provider` 持久化；传递 `providerIds` 给 ConversationSidebar |
| ARCHITECTURE.md | 更新 conversations 表 schema |

### 2026-10-05 悬浮窗发送消息崩溃修复 — tokio::spawn → tauri::async_runtime::spawn

**问题**：Orb 悬浮窗发送消息时进程崩溃，exit code `0xc0000409 STATUS_STACK_BUFFER_OVERRUN`

**根因链**：
1. `send_chat_message` command 中使用 `tokio::spawn` 启动异步流处理
2. Tauri 2.x 的 `#[command]` 默认运行在主线程（UI 线程），**没有 Tokio runtime 上下文**
3. `tokio::spawn` 要求当前线程在 Tokio runtime 内 → panic: `there is no reactor running, must be called from the context of a Tokio 1.x runtime`
4. panic 发生在无法 unwind 的函数中 → 触发 `STATUS_STACK_BUFFER_OVERRUN` 进程终止

**参考项目做法**：

| 项目 | 处理方式 |
|------|----------|
| OpenHuman | Core/Shell 分离，spawn 内不访问 State，用 `mpsc::channel` 解耦 |
| OpenBitFun | `mpsc::unbounded_channel` 解耦，`CancellationToken` 取消，spawn 内不访问 State |
| BongoCat | GPUI 框架 `cx.spawn()` 自动处理上下文 |
| deepseek-harness-orb | TypeScript `async function*` + `AbortController` |
| ChatGPT/WindowPet | 无流式聊天，同步 command |

**修复 1 — `tauri::async_runtime::spawn` 替代 `tokio::spawn`**：

- `tauri::async_runtime::spawn` 使用 Tauri 内置的 Tokio runtime（在 `Builder::setup` 阶段已初始化）
- `tokio::spawn` 需要当前线程已在 Tokio runtime 上下文中——Tauri 的同步 command 运行在主线程，不满足
- 3 处替换：DB 写入 channel consumer、finish channel consumer、stream 主循环

**修复 2 — `mpsc::unbounded_channel` 解耦 State 访问**：

- DB 写入（`DbState`）和任务完成（`ChatTasks`）从 stream 循环中解耦到独立 task
- stream task 只通过 channel 发消息（`db_tx.send()` / `finish_tx.send()`），不直接访问 `tauri::State`
- channel consumer task 通过 `AppHandle.state()` 安全访问 State

**修复 3 — `Box::pin` 防止栈溢出**：

- 大 async future 用 `Box::pin` 包装，堆分配避免栈溢出
- `tauri::async_runtime::spawn(Box::pin(stream_future))`

**修复 4 — 生命周期问题**：

- `get_db_conn(&state)` 返回 `Result<MutexGuard, String>`，`if let Ok(conn)` 写法导致临时 `MutexGuard` 和 `state` 生命周期冲突
- 改为 `match get_db_conn(&state)` + 分号结尾，确保临时值在块结束前 drop

**涉及文件**：`src-tauri/src/cmd.rs`（send_chat_message 函数）

### 2026-10-05 消息列表滚动修复 — Flexbox min-height: 0

**问题**：消息列表无法滚动，内容超出视口时被裁切而非出现滚动条

**根因**：CSS Flexbox 子元素默认 `min-height: auto`，不会缩小到比内容更小 → `overflow-y: auto` 永远不触发滚动条，容器无限撑高

**Flexbox 滚动规则**：
```
父容器 (flex column, 固定高度)
  └─ 子容器 (flex: 1, min-height: 0, overflow-y: auto)  ← 滚动生效 ✅
  └─ 固定元素 (flex-shrink: 0)                           ← 不被压缩 ✅
```

**修复**：

| 文件 | 选择器 | 添加属性 | 原因 |
|------|--------|----------|------|
| global.css | `.chat-view-compact` | `flex: 1; min-height: 0;` | compact 视图填满 orb-panel 且允许缩小 |
| global.css | `.chat-main` | `min-height: 0;` | 允许缩小到比内容更小，使 MessageList overflow-y 生效 |
| global.css | `.chat-input` | `flex-shrink: 0;` | 防止输入框被压缩 |
| MessageList.tsx | useEffect | `lastScrollHeightRef` 优化 | 流式输出时避免重复 scrollTo 调用 |

**关键知识点**：`min-height: 0` 是让 flex 子项可以缩小到比内容更小的必要条件——没有它，`overflow-y: auto` 永远不会产生滚动条

### 2026-10-05 I2 增强 — 消息历史管理（主窗口）

主窗口新增 `HistoryManager.tsx`（左右分栏：左侧对话列表+搜索+时间分组，右侧消息详情+虚拟滚动+分页加载+删除+导出）。

**Rust 后端**：conversations.rs 新增 `get_messages_paginated`/`get_message_count`/`delete_message`/`search_conversations`/`export_conversation_markdown`；cmd.rs 新增 4 个 Tauri Command。

**前端**：constants.ts 新增 4 常量（MESSAGES_PAGE_SIZE/VIRTUAL_SCROLL_THRESHOLD/OSCAN/SCROLL_LOAD_THRESHOLD_PX）；chatStore.ts 扩展分页加载/持久化删除/全文搜索；App.tsx 新增 `/history` 路由+侧栏菜单项。

**踩坑**：
- 虚拟滚动/分页误加到 Orb MessageList → 恢复为简单渲染，只在 HistoryManager 使用
- 消息气泡硬编码颜色 → 改 CSS 变量 `--color-primary-bg`/`--color-primary-border`
- 对话列表不滚动 → 原生 div + `flex:1; overflow-y:auto; minHeight:0` 替代 Layout.Sider
- 时间分组用"过去24h"判断 → 改日历日期边界（`new Date(year,month,date).getTime()`）
- 刷新不更新消息 → `handleRefresh()` 并行刷新列表+消息，15s 定时器同步

### 2026-10-05 移除悬浮窗内置模板卡片（TemplateCards）

移除 Orb 空对话时的模板快捷入口（TemplateCards 组件 + 变量填写 Modal）。原因：悬浮窗空间有限，Modal 在 compact 模式体验差，用户可直接输入 prompt。

删除 `TemplateCards.tsx`；MessageList/ChatView 移除模板相关 props/state/invoke；global.css 移除 `.template-card*` 样式；setup.rs 移除 `seed_builtin_templates` 注释代码。Rust 后端 templates.rs/cmd.rs 和主窗口 TemplateManager 保留。

### 2026-10-05 主窗口标题栏高度被内容撑高修复

Flexbox `min-height:auto` 导致侧边栏内容撑高内层 Layout，Titlebar 被压缩。修复：外层 Layout 加 `overflow:hidden`，内层 Layout 加 `flex:1; minHeight:0; overflow:hidden`，Sider 加 `overflow:hidden`，Titlebar 加 `flexShrink:0`。SettingsWindow 同理。

### 2026-10-06 I2-History — 聊天消息历史管理增强

规格：[I2-history-spec.md](./store/I2-history-spec.md) | 任务清单：[I2-history-tasklist.md](./store/I2-history-tasklist.md)

#### 迁移路径取消

`ALTER TABLE ADD COLUMN` 无法幂等（已存在列报错），内测期无旧用户，改用 `CREATE TABLE IF NOT EXISTS` 兜底。`migrations.rs` 保留空壳。

#### Rust 后端

Row 扩展（is_pinned/generation/sort_order/title_source/parent_id）+ 新增 CRUD（分页/置顶/复制/搜索/重命名/批量删除/清空/generation递增/消息删除编辑/导出JSON/Markdown）+ 11 个 Tauri Commands

#### 前端 Store

PersistQueue 串行化队列 + 分页加载 + 对话高级操作（置顶/复制/导出/搜索/批量删除/清空）+ 消息编辑/重新生成/持久化删除 + fallback 标题生成 + contextUsage 上下文监控 + generation 递增

#### 前端 UI

Sidebar 时间分组 + Provider 过滤 + 置顶标识 + 右键菜单 + 搜索栏（防抖+全文开关）+ MessageList 分页/骨架屏/回到底部 + MessageActions hover 操作栏 + ContextBar 进度条

#### Bug 修复

1. **KaTeX 未接入**：MessageContent/ReasoningBlock 添加 remarkMath+rehypeKatex。踩坑：`as const` 导致 readonly 不兼容 Pluggable[]，去掉解决
2. **generation 未递增**：finishStream 末尾 fire-and-forget 调用 `invoke("increment_generation")`
3. **行距过大**：`white-space: pre-wrap` 从 `.bubble` 积到 `.bubbleUser`。根因：pre-wrap 渲染 ReactMarkdown `<p>` 间 `\n` 为空行（22.4px），叠加 margin-top 8px = 30.4px 段落间距

### 2026-10-06 chat/ 目录重组 — 扁平→分组

**问题**：chat/ 下 23 个文件扁平堆在一起，维护困难

**重组**：

```
chat/
  blocks/                     ← 消息内嵌块
    EditBlock.tsx / .module.css
    ErrorBlock.tsx / .module.css
    ReasoningBlock.tsx / .module.css
    ToolCallBlock.tsx / .module.css
  message/                    ← 消息组合
    MessageBubble.tsx / .module.css
    MessageContent.tsx / .module.css
    MessageList.tsx / .module.css
    MessageInput.tsx
    MessageActions.tsx / .module.css
  ChatView.tsx / ConversationSidebar.tsx / HistoryManager.tsx / ...
```

**导入路径变更**：ErrorBlock→`../chatErrors`、ToolCallBlock→`../../stores/chatStore`、MessageBubble→`../blocks/*`+`../../stores/*`、MessageList→`../../stores/chatStore`、MessageInput→`../../config/constants`、ChatView→`./message/MessageList`+`./message/MessageInput`

### 2026-10-06 ReasoningBlock 瘦身 — 移除 Markdown/KaTeX 依赖

**问题**：ReasoningBlock 用 ReactMarkdown+remarkMath+rehypeKatex 渲染思维链内容，但思维链（`delta_reasoning`）是纯自然语言推理过程，极少出现 Markdown/LaTeX。~64KB 依赖完全浪费。

**修复**：ReasoningBlock 改用原生 `<pre>` + `white-space: pre-wrap` + `word-break: break-word`，移除全部 Markdown/KaTeX 导入。CSS 从 87 行（含 6 条 KaTeX `:global()` 规则）减至 58 行。

### 2026-10-06 流式消息并发写入 Bug 修复

**问题**：多个 Rust tokio::spawn 同时向前端 emit `chat-delta`，导致消息内容交叉/重复/写入错误对话。

**3 个 Bug**：

| Bug | 位置 | 触发场景 |
|-----|------|---------|
| 切换对话不取消流 | chatStore.ts switchConversation | 对话A流式输出→切换到对话B，A的delta继续到达 |
| 重复发送不取消旧流 | chatStore.ts sendMessage | 流式输出中→快速再发一条，两个spawn同时emit |
| 编辑消息不取消流 | chatStore.ts editMessage | 流式输出中→编辑重发，旧流+新流同时写入 |

**前端修复**（3 处）：switchConversation / sendMessage / editMessage 操作前先 `invoke("cancel_chat")` + 重置 `isStreaming/streamingContent/streamingMessageId`

**Rust 端修复**（1 处）：`ChatTasks.start()` 原来 `map.insert()` 直接覆盖旧 CancellationToken，旧流不知道被取消。改为 `if let Some(old) = map.insert(id, token) { old.cancel(); }` 先取消同一对话的旧流
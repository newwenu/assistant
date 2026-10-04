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

### 2026-10-03 I1 — Orb 悬浮球实施

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
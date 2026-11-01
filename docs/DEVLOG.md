# 开发日志

记录技术决策、踩坑与架构变更，按日期组织。原则：只记**为什么**和**踩了什么坑**，不记冗余步骤。

---

### 2026-10-03 文档审查

ARCHITECTURE.md 与实际代码不一致项（已修正）：React 18→19、Ant Design 5→6、TypeScript 未标注→6、路由编号跳跃、I7 命名不统一、conversations 缺外键；FRONTEND.md 12 处绝对路径→相对路径；DESIGN.md 顶部加废弃声明（与 ARCHITECTURE.md 有 5 项根本冲突）。

**注意**：`react-router-dom` 为 v7（非 v6），API 有差异；前端目录结构文档为规划态，已加"实现状态"说明。

### 2026-10-03 验收修复

验收标准：`cargo tauri dev` 启动后可见主窗口 + 托盘图标，主题可切换。修复 5 项：

1. **React Router 集成**：已安装未使用→MainLayout 加 BrowserRouter+Routes（`/`、`/settings`）+ SettingsWindow/viewMap
2. **SQLite 启动初始化**：`setup::init` 中按 `app_data_dir()` 拼接路径调用 `init_db`
   - ⚠️ `app_data_dir()` 返回 `Result` 非 `Option`，`if let Some` 编译失败，改 `match`
3. **db.rs 表结构对齐文档**：按 ARCHITECTURE.md 5.1 重写，新增 tool_favorites/tool_history/compression_state/app_settings
4. **capabilities 权限补全**：sql 系列 + 窗口 minimize/toggle-maximize/is-maximized + settings 窗口
5. **Titlebar macOS 避让**：`navigator.platform` 检测 macOS，paddingLeft 动态 80/12

### 2026-10-03 Cargo Workspace 重构

**问题**：`npm run tauri dev` 报 `No package info in the config file` — Tauri 2 CLI 要求 `src-tauri/Cargo.toml` 含 `[package]` 段，不支持纯 workspace 根

**决策**：Cargo.toml 同时含 `[package]`（根包 assistant-app）+ `[workspace]`（6 crate），源码移至 `src-tauri/src/`；最终结构 `src-tauri/src/`（根包 6 文件）+ `src-tauri/crates/`（6 个 lib crate）。保留 lib.rs（run()）+ main.rs，为移动端入口与单测保留

**踩坑**：
- 根包与 tauri.conf.json 同目录→`build.rs` 直接 `tauri_build::build()`，`generate_context!("tauri.conf.json")` 无需手写路径
- Tauri 2 API：Menu/MenuItem 移至 `tauri::menu::`，TrayIcon 移至 `tauri::tray::`
- tauri.conf.json 的 `identifier` 在顶层非 bundle 内

### 2026-10-03 透明窗口背景修复

**问题**：Orb 窗口 `transparent: true` 却显示白/黑方块背景

**根因**：`global.css` 的 `body { background-color: var(--color-bg) }` 给整窗铺了不透明底色，覆盖 Tauri 透明效果

**参考**：WindowPet — `body` 只设 `overflow: hidden` 不设背景色；透明区用 `setIgnoreCursorEvents(true)` 点击穿透

**修复**：移除 body 背景色；MainLayout 外层 `background: var(--color-bg-layout)`；SettingsWindow 外层 `var(--color-bg)`；OrbPlaceholder 保持 `transparent`

**原则**：body 不设背景色，各窗口组件自行控制背景；透明窗口不设任何不透明背景

### 2026-10-03 脚手架对齐开源项目（WindowPet / ChatGPT）+ 托盘与退出修复

对比后落地 8 项；逐项对照代码验证，发现其中 4 项日志已记录但代码缺失，一并补齐：

1. **ExitRequested 防退出**：关闭窗口后托盘仍在但无法重开→`lib.rs` 改 `.build()` + `.run()`，拦截 `ExitRequested` 调 `api.prevent_exit()`，仅 `app.exit(0)` 真退出（参考 WindowPet main.rs）
2. **Windows GPU 加速**：WebView2 默认可能禁用 GPU→`main.rs` 加 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--ignore-gpu-blocklist"`，解决 Tauri #4891 渲染问题
3. **capabilities 权限补全**：补 `set-ignore-cursor-events`/`inner-size`/`set-icon`/`webview:default`，另补 `outer-position`/`scale-factor`/`destroy`（前端 Orb 拖拽用 `outerPosition()`/`scaleFactor()`，托盘退出用 `destroy()`）
   - ⚠️ Rust 端调 `set_ignore_cursor_events` 不受 capabilities 限制，但前端 JS 调用会 403
4. **macOS private-api**：透明窗口在 macOS 需 private API→`macOSPrivateApi: true` + Cargo feature `macos-private-api`
5. **托盘左键点击**：`menuOnLeftClick: false` 但缺 `on_tray_icon_event`→tray.rs 补 `.on_tray_icon_event()`（需 `MouseButton`/`MouseButtonState`/`TrayIconEvent`）
6. **删除残留 App.css**：Vite 脚手架 `:root { background-color: #f6f6f6 }` 会破坏透明窗口
7. **ConfigProvider 不包裹 Orb**：Ant Design 全局样式影响透明窗口→`webview.label === "orb"` 时跳过
8. **OrbConf 路径风格统一**：补齐 `get_conf_dir()`，与 AppConf 一致

**另修的两个 bug**：
- **托盘左键双事件冲突**：`on_menu_event`（Windows 左键默认弹菜单）与 `on_tray_icon_event` 同时触发→左键无响应。加 `.show_menu_on_left_click(false)`，左键唤起主窗口、右键弹菜单
- **退出被 ExitRequested 吞掉**：`app.exit(0)` 触发 `ExitRequested`，无条件 `prevent_exit()` 阻止退出。引入 `static SHOULD_EXIT: AtomicBool`，quit 菜单先 `request_exit()` 置标志再 `app.exit(0)`，`ExitRequested` 仅在 `!SHOULD_EXIT` 时阻止

**托盘唤起窗口无效**：关闭主窗口后窗口被销毁，`get_webview_window("main")` 返回 `None`，`show()`/`set_focus()` 不执行。提取 `show_or_create_main(app)`：存在则 show+focus，不存在则 `WebviewWindowBuilder` 重建；左键与菜单"打开主窗口"统一调用

### 2026-10-03 实施记录

**阶段 A — Rust 后端**（A1~A6）：A1 新增 `toggle_orb_visibility`/`set_orb_click_through`/`snap_orb_to_screen` 三个 Command；A2 placement 增强 `get_available_screens` + `check_and_snap_orb`（ScreenBounds 移至 hover.rs 共享）；A3 `HoverState::should_toggle_click_through()`；A4 `tauri-plugin-global-shortcut` + Cmd/Ctrl+K 切换可见性；A5 `on_window_event` 监听 Orb 失焦 emit `orb-blur`；A6 常量对齐（hitbox / 失焦收起延迟 / 吸附延迟 / hover 与拖拽轮询间隔）

**阶段 B — 前端组件**（B1~B8）：B1 `orbStore`（isExpanded/isHovering/isDragging/isVisible/position）；B2 前端常量对齐；B3 `OrbDrag`（阈值判断 + 吸附延迟 + poll）；B4 `OrbBall`（hitbox + hover + 穿透 + 右键菜单）；B5 `OrbPanel`（占位 UI + 失焦收起 + Escape）；B6 `OrbContextMenu`（Ant Design Dropdown）；B7 `OrbExpand`（LogicalSize 切换）；B8 App.tsx 重构 `OrbPlaceholder`→`OrbWindow`

**验证**：`cargo build` ✅ / `npm run typecheck` ✅ / `npm run lint` ✅

### 2026-10-04 Orb 展开/收起重构 — CSS 动画方案

**问题**：展开/收起时窗口尺寸变化导致闪烁和变形（OrbPanel 被小窗口裁切成椭球）

**参考**：deepseek-harness-orb — 窗口尺寸不变，用 CSS `transform: scale()` + `opacity` + `pointer-events` 做 300ms 过渡

**要点**：
1. 窗口固定 420×600，消除 Rust ↔ React 时序竞争
2. `.orb-panel` 默认 `scale(0.18)` + `opacity: 0` + `pointer-events: none`；`.orb-expanded` 下恢复 `scale(1)`/`opacity: 1`/`auto`，300ms ease-in-out
3. Ball 与 Panel 始终同时渲染，用 `orb-expanded` class 切换状态，不再条件渲染
4. Rust `expand/collapse_orb_panel` 不再改尺寸；`orbStore` 只改状态不调 IPC
5. CSS 特异性：`div[data-orb-window]` 替代 `[data-orb-window]` + `!important`，防 Ant Design 等第三方样式覆盖穿透规则
6. `ORB_CHROME_INSET = 12px`，Ball/Panel 绝对定位 + inset 留出阴影空间

**设计原理**：容器层 `pointer-events: none` 让整窗穿透，`.orb-hitbox` / `.orb-panel`（展开时）`pointer-events: auto` 仅交互区可点。Windows 用 CSS 精细控制（比 Rust `set_ignore_cursor_events` 只能控整窗更精确）；macOS 仍用 Rust `set_orb_click_through`（CSS 无法穿透到系统级）

### 2026-10-04 快捷键占用处理

**问题**：全局快捷键（Cmd/Ctrl+K）可能被其它应用占用，注册失败时静默无提示

**当前处理**：注册失败 `log::warn!` 记录日志。**后续方向**：前端检测失败弹 toast、设置页允许自定义快捷键、提供托盘菜单备选

### 2026-10-04 Orb 事件穿透优化 — 跨平台 + 坐标统一

**问题**：Orb 窗口透明区域拦截鼠标事件，用户无法点击到下方窗口内容。Windows 完全没有 click-through 机制，macOS hitbox 检测范围是整个窗口（420×600）而非小球（48×48）

**参考对比**：BongoCat（原生 Win32，`WS_EX_TRANSPARENT|LAYERED` + 光标 vs 窗口矩形，状态机驱动）；WindowPet（Tauri v1，`set_ignore_cursor_events` + 50ms 防抖 + Phaser 像素碰撞）；airi（Electron，`setIgnoreMouseEvents` + `forward:true` + 8px 手抖容差 + `elementFromPoint` 检测）；deepseek（Electron，引用计数 guard，HID 期间穿透）

**修复 1 — 跨平台 click-through**：Windows 也启用光标轮询（`NEEDS_CURSOR_POLL = IS_MACOS || IS_WINDOWS`）；hitbox 检测改为鼠标 vs 48×48 hitbox 矩形而非整窗；`OrbExpand` 展开关穿透/收起开穿透，`OrbDrag` 拖拽开始关穿透，`App.tsx` 初始化开穿透

**修复 2 — DPR 坐标系混用（根因 bug）**：CSS 常量（`ORB_CHROME_INSET=12`、`ORB_HITBOX_SIZE=48`）是逻辑像素，Tauri API（`outerPosition`/`innerSize`）返回物理像素，混加导致 hitbox 偏移；表现为 150% DPI 下只有小球左上 ~1/4 可命中。创建 `OrbCoord.ts` 统一转换，物理坐标 `/ dpr` 转逻辑后再与 CSS 常量比较

**修复 3 — Rust 端坐标**：`expand/collapse_orb_panel` 由 `PhysicalSize::new(420, 600)` 改 `LogicalSize::new(420, 600)`，位置用 `scale_factor` 换算；`position_orb_window` 用 `orb_w * scale` 得物理像素；`get_screen_size` 接收 `scale_factor`，逻辑→物理后再与 screen API 比较

**修复 4 — 性能与稳定性**：Rust `static ORB_CLICK_THROUGH: AtomicBool` 缓存，值相同跳过 `set_ignore_cursor_events`（省 ~90% 调用）；前端 store `clickThrough` 共享避免冗余 IPC；开启穿透 30ms 防抖、关闭立即执行；8px 手抖容差；轮询 120ms→80ms（配合缓存不增实际调用）；拖拽 `onPointerUp` 显式穿透

**OrbCoord.ts API**：`physicalToLogical` / `physicalSizeToLogical` / `logicalToPhysical` / `getMouseLogicalPos` / `getWindowLogicalRect` / `isPointInRect` / `isPointInCircle`

**核心原则**：同一运算中的所有坐标必须同坐标系——前端检测用逻辑坐标 API，设置窗口位置用 `logicalToPhysical()`

### 2026-10-04 四象限展开修复 — 窗口尺寸随状态变化

**问题**：Orb 四象限展开方向判断做了，但窗口位置从未移动，无论判断向哪个方向展开，面板最终都在同一个位置

**根因**：① `OrbExpand.expand()` 只切 CSS class（改 `transform-origin`），从不调 Tauri API 移动窗口或改尺寸；② 窗口始终 420×600、球始终在左上角，`scale(1)` 终态永远一样，`transform-origin` 只影响从哪个角"长出来"；③ 方向判断用窗口边缘（420×600）而非球边缘（64×64），判断本身不准

**参考**：deepseek-harness-orb — 折叠 96×96（球72+2×12 inset），展开 344×444（面板320×420+2×12 inset），且**位置按方向移动**使球留在对应角落

**修复**：
1. 常量：`ORB_COLLAPSED_WINDOW_SIZE=88`、`ORB_EXPANDED_WINDOW_WIDTH=444`、`ORB_EXPANDED_WINDOW_HEIGHT=624`（均为内容 + 2×12 inset）
2. Rust 重写 `expand/collapse_orb_panel`：接 `horizontal`/`vertical`，先 `ballOriginFromWindow` 从窗口位置反推球位置，再 `overlayBoundsFromBall` 算新窗口位置；展开 444×624、折叠 88×88，保证球在对应角落
3. `OrbExpand.ts` 方向判断改用球心（`ballX + ORB_COLLAPSED_SIZE/2`），剩余空间用球边缘算；`expand()/collapse()` 调 IPC 移动/缩回窗口
4. `OrbBall.tsx` 球位置随方向动态：`expandDirection === "left"` → `left: calc(100% - 76px)`；`expandVertical === "up"` → `top: calc(100% - 76px)`；折叠时回左上角
5. `tauri.conf.json` orb 初始 88×88（折叠态）；展开态显式保留 `transform-origin`

**实际方案**：收起 88×88 ↔ 展开 444×624，Rust reposition + resize 切换尺寸，CSS 300ms 视觉过渡——既消除 Rust ↔ React 时序竞争导致的闪烁/裁切，又让窗口尺寸与内容区域匹配

### 2026-10-04 展开闪烁修复 — flushSync + 双 rAF + transitionend

**问题**：展开时小球闪跳到窗口左上角再归位；收起时面板被裁切闪烁

**根因**：① Rust `expand_orb_panel` 分两步 `set_position` → `set_size`，中间一帧窗口已在新位置但仍是旧尺寸，此时 `isExpanded` 仍为 `false`，小球 CSS 位置（`ballLeft: 12px`）落在新窗口左上角而非正确角落，React 重渲染后才推到 `calc(100%-76px)`，产生可见跳变；② `storeCollapse()` 触发 300ms 淡出后立刻 `invoke("collapse_orb_panel")` resize 到 88×88，动画未播完即被裁切

**参考**：BongoCat 单次 `SetWindowPos` 同时设 x/y/w/h 无中间帧；deepseek-harness-orb 用 Electron `setBounds` 原子操作；Tauri 的 `set_position` + `set_size` 两次调用无法合并

**修复 — 纯前端方案，零 Rust 改动**：

1. `orbStore` 新增 `isBallVisible` + `setBallVisible`，控制小球显隐，独立于 `isExpanded`
2. **展开时序**：`flushSync(setBallVisible(false))`（同步渲染，小球 opacity:0）→ `waitForPaint()`（双 rAF：第1帧等 style/layout，第2帧确保 compositor 上屏）→ `invoke("expand_orb_panel")`（resize 时小球已不可见）→ `storeExpand()`（面板淡入）
3. **收起时序**：`storeCollapse()`（淡出开始）→ `waitForPanelTransition()`（监听 `.orb-panel` 的 `transitionend`，350ms 超时兜底）→ `invoke("collapse_orb_panel")`（resize 时面板已全透明）→ `setBallVisible(true)`
4. CSS `.orb-ball-hidden`：`opacity: 0 !important` + `pointer-events: none !important` + `transition: none !important`（瞬间隐藏，不走 150ms 过渡）；`OrbBall` 读取 `isBallVisible` 切类

**设计原则**：前端是"发令者"、Rust 是"执行者"——前端确保视觉状态就绪后再调 IPC；`flushSync` + 双 rAF 比 `eval` 注入或 Rust 端设窗口透明度更可控；`transitionend` + 超时兜底不依赖固定延时

### 2026-10-05 实施记录

**阶段 A — Rust 后端**（A1~A10）：A1 workspace 加 13 个依赖（reqwest/tokio/futures/aes-gcm/sha2/base64/machine-uid/tokio-stream/async-stream/rand/thiserror/uuid/chrono）；A2 crypto.rs（AES-256-GCM + B64 + get_machine_id + 单测）；A3 base.rs（ProviderKind/ProviderConfig/ChatMessage/ChatDelta/ProviderError + 工厂）；A4 openai.rs（流式 chat + list_models + 120s 超时 + 429 限流）；A5 storage CRUD（providers/conversations/templates + seed_builtin_templates）；A6~A7 Provider 与 Conversation Commands；A8 流式核心（send_chat_message + cancel_chat + chat_tasks.rs）；A9 setup/lib 集成（DbState + ChatTasks manage + seed + commands 注册）；A10 6 个 Rust 常量

**阶段 B — 前端**（B1~B19）：B1 装 react-markdown + remark-gfm；B2 7 个 TS 常量；B3 chatStore（全部方法 + ensureCurrentConversation + streamingMessageId + chat-delta 监听）；B4 providerStore；B5 streamCoalescer（rAF + setTimeout 合并 + flush + dispose，含 11 个单测）；B6 MessageBubble（Markdown + 流式光标 + 错误 + hover 时间戳 + 删除）；B7 MessageList（自动滚底 + 回到底部 + TemplateCards）；B8 MessageInput（自适应 + Enter 发送 / Shift+Enter 换行 + Send/Stop）；B9 ModelSelector（chip 变体 + providerId 过滤）；B10 ConversationSidebar（搜索 + 相对时间 + 右键菜单 + inline 编辑 + 删除确认）；B11 TemplateCards（2 列网格 + 变量提取 + 填写弹窗）；B12 ChatView（整合 + 无 Provider 引导 + compact + ProviderSelector 内嵌）；B13 OrbPanel 接入 ChatView；B14 ProviderManager；B15 ProviderForm；B16 TemplateManager；B17 路由 `/settings/providers`、`/settings/templates` + AI 设置分组；B18 右键菜单新建对话链路；B19 CSS 样式

**阶段 C — 集成验收**：SQL 权限已配置；`cargo build` + `tsc --noEmit` + `vite build` 全通过

### 2026-10-05 设置变更器（Settings Dispatcher）

**参考**：WindowPet `handleSettingChange` — 统一入口，集成持久化 + store 更新 + 跨窗口事件

**实现**：`events.ts`（SettingAction/SettingEvent/Payload 枚举）→ `settingsDispatcher.ts`（dispatchProviderSaved/Deleted、dispatchThemeChanged、dispatchAppConfChanged、dispatchOrbConfChanged、dispatchTemplateSaved；每个先 `invoke()` 持久化 → 更新 store → `emitSettingEvent()` 遍历 [WINDOW_MAIN, WINDOW_ORB] 发事件）→ `settingListeners.ts`（`initSettingListeners()` 各窗口初始化时监听并更新 store）

**踩坑**：Tauri 2 `WebviewWindow.getByLabel()` 返回 `Promise<WebviewWindow | null>`，必须 `await` 后才能 `.emit()`；`WebviewWindow` 从 `@tauri-apps/api/webviewWindow` 导入，非 `@tauri-apps/api/window`

### 2026-10-05 右键菜单"新建对话"接入

**问题**：OrbContextMenu 的"新建对话" MenuItem 在 Rust 端创建但 `on_menu_event` 未处理 `orb_new_chat`

**修复**：`lib.rs` 新增 `"orb_new_chat"` 分支 → `app.emit("orb-new-conversation", ())`；OrbPanel 监听后派发 DOM 事件 `chat-new-conversation`；ChatView 监听并调 `handleNewConversation()`

**为何用 DOM 事件委托而非直接调 store**：`currentProviderId` 在 ChatView 内部，OrbPanel 不持有；委托后由 ChatView 自行决定 Provider，避免跨组件状态同步

### 2026-10-05 多 Provider 选择 — 内嵌下拉而非弹窗

**问题**：多 Provider 时新建对话直接取 `providers[0]`，用户无法选择

**方案**：❌ Modal 弹窗打断操作流、compact 模式体验差；✅ 对话栏内嵌 `ProviderSelector`（与 ModelSelector 同风格，`variant="chip"` 时圆角药丸，`providers.length <= 1` 返回 null），排在 ModelSelector 前，切换时重置 Model 为新 Provider 默认模型；`handleNewConversation()` 直接用当前 `currentProviderId`；OrbPanel 移除 Modal/Radio 依赖

### 2026-10-05 streamCoalescer 单元测试

**框架**：vitest（新增 devDependency），`vitest.config.ts` 限制 `include: ["src/**/*.test.ts"]` 排除 references 目录

**11 个用例**：单事件投递、同 conversation_id+message_id 合并、不同 message_id 不合并、不同 conversation_id 不合并、flush 立即排空、dispose 触发 flush、flush 后继续 push、跨 message 边界合并、空队列 flush、frameScheduler setTimeout 回调、frameScheduler cancel 阻止回调

### 2026-10-05 简化/偏差记录

- **A8 锁竞争**：`tokio::spawn` 内通过 `app_clone.state()` 取 DbState，极端并发可能锁竞争；短期可接受，长期改 `Arc<Pool>`
- **B3 删除确认**：chatStore `deleteConversation` 无确认弹窗，由 ConversationSidebar 的 `Modal.confirm` 兜底
- **B15 Key 清空歧义**：`Input.Password` 清空后 Rust 用 `starts_with("••••")` 判断保留旧 Key，但空串不匹配→清空被当作"未修改"；后续加显式"清除 Key"按钮

### 2026-10-05 Provider-对话关系解耦 — provider_id nullable

**问题**：删除 Provider 时级联删除所有关联对话，用户误删 Provider 后对话历史全丢

**变更**：`provider_id TEXT NOT NULL REFERENCES providers(id)` → `provider_id TEXT`（nullable，无外键）

**修改清单**：db.rs 改列定义 + `run_migrations` v1 迁移（CREATE conversations_v1 + INSERT + DROP + RENAME）；conversations.rs `provider_id: Option<String>`；providers.rs `delete_provider` 由 `DELETE FROM conversations` 改 `UPDATE conversations SET provider_id=NULL`；cmd.rs 发送时 provider_id 为 None 返回"此对话的 Provider 已被删除，请重新选择"；新增 `update_conversation_provider` command 并注册；前端 `Conversation.provider_id: string | null`；ProviderManager 删除文案改为"对话将保留，需重新选择 Provider"；ConversationSidebar 新增 `providerIds` prop，无效时显示"Provider 不可用"；ChatView 切换 Provider 时同步持久化；ARCHITECTURE.md 更新 schema

### 2026-10-05 悬浮窗发送消息崩溃 — tokio::spawn → tauri::async_runtime::spawn

**问题**：Orb 发送消息进程崩溃，exit code `0xc0000409 STATUS_STACK_BUFFER_OVERRUN`

**根因链**：Tauri 2 的 `#[command]` 默认在主线程（UI 线程）运行，无 Tokio runtime 上下文 → `tokio::spawn` panic `there is no reactor running` → panic 在无法 unwind 的函数中 → 触发 `STATUS_STACK_BUFFER_OVERRUN` 终止进程

**参考**：OpenHuman / OpenBitFun 用 `mpsc` 解耦 + spawn 内不访问 State；BongoCat 用 GPUI `cx.spawn()`；deepseek-harness-orb 用 TS `async function*` + `AbortController`；ChatGPT/WindowPet 无流式聊天

**修复**（`cmd.rs` 的 `send_chat_message`）：
1. `tokio::spawn` → `tauri::async_runtime::spawn`（Tauri 内置 runtime 已在 `Builder::setup` 初始化），共 3 处
2. `mpsc::unbounded_channel` 解耦：DB 写入（DbState）与任务完成（ChatTasks）拆成独立 task，stream task 只 `send()`，由 consumer task 通过 `AppHandle.state()` 安全访问
3. `Box::pin` 包装大 future，堆分配防栈溢出
4. `get_db_conn(&state)` 返回 `Result<MutexGuard, String>`，`if let Ok(conn)` 与临时 `state` 生命周期冲突 → 改 `match` + 分号结尾，确保临时值块结束前 drop

### 2026-10-05 消息列表滚动修复 — Flexbox min-height: 0

**问题**：消息列表无法滚动，内容超出视口被裁切

**根因**：Flexbox 子元素默认 `min-height: auto`，不会缩小到比内容更小 → `overflow-y: auto` 永不触发滚动条

**修复**：`.chat-view-compact` 加 `flex: 1; min-height: 0;`；`.chat-main` 加 `min-height: 0;`；`.chat-input` 加 `flex-shrink: 0;`；MessageList 用 `lastScrollHeightRef` 避免流式输出时重复 `scrollTo`

**关键**：`min-height: 0` 是 flex 子项可缩小到内容以下的必要条件

### 2026-10-05 I2 增强 — 消息历史管理（主窗口）

新增 `HistoryManager.tsx`（左：对话列表+搜索+时间分组，右：消息详情+虚拟滚动+分页+删除+导出）。

**Rust**：conversations.rs 新增 `get_messages_paginated`/`get_message_count`/`delete_message`/`search_conversations`/`export_conversation_markdown` + 4 个 Command。

**前端**：4 个常量（MESSAGES_PAGE_SIZE/VIRTUAL_SCROLL_THRESHOLD/SCROLL_LOAD_THRESHOLD_PX 等）；chatStore 扩展分页/删除/全文搜索；App.tsx 加 `/history` 路由+侧栏菜单。

**踩坑**：虚拟滚动/分页误加到 Orb MessageList → 恢复简单渲染，仅 HistoryManager 使用；气泡硬编码颜色 → CSS 变量 `--color-primary-bg`/`--color-primary-border`；对话列表不滚动 → 原生 div + `flex:1; overflow-y:auto; minHeight:0` 替代 Layout.Sider；时间分组用"过去24h"不准 → 改日历日期边界；刷新不更新消息 → `handleRefresh()` 并行刷新列表+消息，15s 定时器同步

### 2026-10-05 移除悬浮窗内置模板卡片

删除 Orb 空对话时的 TemplateCards（组件 + 变量填写 Modal）：悬浮窗空间有限、Modal 在 compact 模式体验差，用户可直接输入 prompt。同步清理 MessageList/ChatView 的模板 props、global.css `.template-card*`、setup.rs 的 seed 注释代码；Rust 后端与主窗口 TemplateManager 保留。

### 2026-10-05 主窗口标题栏被内容撑高

Flexbox `min-height: auto` 导致侧边栏内容撑高内层 Layout、压缩 Titlebar。修复：外层 Layout `overflow: hidden`；内层 Layout `flex: 1; minHeight: 0; overflow: hidden`；Sider `overflow: hidden`；Titlebar `flexShrink: 0`；SettingsWindow 同理。

### 2026-10-06 I2-History — 聊天消息历史管理增强

规格：`I2-history-spec.md` ｜ 任务清单：`I2-history-tasklist.md`

**迁移路径取消**：`ALTER TABLE ADD COLUMN` 无法幂等（列已存在报错），内测期无旧用户，改用 `CREATE TABLE IF NOT EXISTS` 兜底，`migrations.rs` 留空壳。

**Rust**：Row 扩展（is_pinned/generation/sort_order/title_source/parent_id）+ 新增 CRUD（分页/置顶/复制/搜索/重命名/批量删除/清空/generation 递增/消息删除编辑/导出 JSON+Markdown）+ 11 个 Command。

**前端 Store**：PersistQueue 串行化队列 + 分页加载 + 对话高级操作 + 消息编辑/重新生成/持久化删除 + fallback 标题生成 + contextUsage 监控 + generation 递增。

**前端 UI**：Sidebar 时间分组 + Provider 过滤 + 置顶标识 + 右键菜单 + 搜索栏（防抖+全文开关）；MessageList 分页/骨架屏/回到底部；MessageActions hover 操作栏；ContextBar 进度条。

**Bug 修复**：
1. **KaTeX 未接入**：MessageContent/ReasoningBlock 加 remarkMath+rehypeKatex。踩坑：`as const` 导致 readonly 不兼容 `Pluggable[]`，去掉解决
2. **generation 未递增**：finishStream 末尾 fire-and-forget 调 `increment_generation`
3. **行距过大**：`white-space: pre-wrap` 从 `.bubble` 积到 `.bubbleUser`，pre-wrap 让 `<p>` 间 `\n` 渲染为空行（22.4px）+ margin-top 8px = 30.4px 段距

### 2026-10-06 chat/ 目录重组 — 扁平→分组

**问题**：chat/ 下 23 个文件扁平堆放，维护困难

**重组**：`blocks/`（EditBlock/ErrorBlock/ReasoningBlock/ToolCallBlock，各带 .module.css）；`message/`（MessageBubble/MessageContent/MessageList/MessageInput/MessageActions）；顶层保留 ChatView/ConversationSidebar/HistoryManager 等。导入路径相应上移一层。

### 2026-10-06 ReasoningBlock 瘦身 — 移除 Markdown/KaTeX

思维链（`delta_reasoning`）是纯自然语言，极少 Markdown/LaTeX，~64KB 依赖纯浪费。改用原生 `<pre>` + `white-space: pre-wrap` + `word-break: break-word`，移除全部 Markdown/KaTeX 导入，CSS 从 87 行（含 6 条 KaTeX `:global()` 规则）减至 58 行。

### 2026-10-06 流式消息并发写入 Bug

**问题**：多个 `tokio::spawn` 同时 emit `chat-delta`，导致内容交叉/重复/写入错误对话

| Bug | 位置 | 触发 |
|-----|------|------|
| 切换对话不取消流 | `switchConversation` | A 输出中切到 B，A 的 delta 继续到达 |
| 重复发送不取消旧流 | `sendMessage` | 流式中快速再发，两个 spawn 同时 emit |
| 编辑消息不取消流 | `editMessage` | 流式中编辑重发，旧流+新流同时写 |

**修复**：前端 3 处操作前先 `invoke("cancel_chat")` + 重置 `isStreaming/streamingContent/streamingMessageId`；Rust `ChatTasks.start()` 由 `map.insert()` 直接覆盖改为 `if let Some(old) = map.insert(id, token) { old.cancel(); }`

### 2026-10-07 Orb 贴边命令语义拆分 + 右边缘视觉补偿 + 屏幕巡检

#### 命令语义拆分

**问题**：`snap_orb_to_screen` 同时承担"回到屏幕"和"贴边"两个语义，拖拽松手贴边合理，屏幕变化恢复时不该贴边

**拆分**：`snap_orb_to_screen` = 回到屏幕（巡检用：屏内不动，离屏按最小位移推入最近屏幕，不贴边、无动画）；`snap_orb_to_edge` = 贴边（拖拽松手用：先回屏幕，再吸附最近左/右边缘，250ms ease-out cubic 动画）。cmd.rs 原命令改为 `snap_orb_to_edge`，新建 `snap_orb_to_screen`，lib.rs 注册两者，OrbDrag.tsx 3 处 invoke 改 `snap_orb_to_edge`

#### 右边缘视觉不对称

**问题**：吸右边缘后球距屏幕右明显大于左边缘距屏幕左

**根因**：窗口 88px（`ORB_COLLAPSED_SIZE(64) + 2×ORB_CHROME_INSET(12)`），但 hitbox 只有 48px 且放在窗口 (12,12)：球左距窗左 12、右距窗右 `88-12-48=28`，球在窗口内偏左 16px。于是左吸附球距屏幕左 = 4+12 = 16px，右吸附球距屏幕右 = 4+28 = 32px

**修复**：constant.rs 加 `ORB_VISUAL_CENTER_OFFSET_X = 16`（`(88-12-48)-12`）；`snap_orb_to_edge` 加 `border_x: i32` 参数，`right_target_x += border_x`。修复后右吸附 `screen.width - 88 - 4 + 16 = screen.width - 76`，两侧均距边 16px

#### 屏幕巡检改用视觉边界

**问题**：右边缘补偿后窗口故意超出屏幕 12px，`bounds_inside_screens` 按窗口边界判断会认为出屏→巡检推回→抵消补偿

**修复**：巡检与 `snap_orb_to_screen` 改用球的视觉边界（hitbox 48×48 在窗口内偏移 12,12）判断：`visual_bounds = WindowBounds::new(pos.x + ORB_CHROME_INSET, pos.y + ORB_CHROME_INSET, ORB_HITBOX_SIZE, ORB_HITBOX_SIZE)`。右吸附后 visual 右边缘 = `screen.width - 16`，巡检不触发

**巡检线程**：`setup.rs` 的 `spawn_screen_inspection` 每 800ms（`PLACEMENT_INSPECTION_INTERVAL`）轮询，用 `SHOULD_EXIT` 原子布尔退出；离屏则 `snap_to_nearest_screen` 恢复并保存 OrbConf；99% 情况直接返回，开销极低

### 2026-10-07 RiveMascot canvas 分辨率在 CSS transform 下初始化错误

**问题**：收起态 `.orb-panel` 处于 `transform: scale(0.18)`，此时已挂载的 RiveMascot 内部 ResizeObserver 拿到的 `contentRect` 是缩放后的小值（≈11×11），canvas 像素缓冲被初始化为 11×11；展开后容器 layout 尺寸（44×44）未变，ResizeObserver 不再触发，canvas 分辨率停留在小值，Mascot 模糊

**根因链**：Rive `useResizeCanvas` 用 ResizeObserver 的 `contentRect` 触发重算、再以 `clientWidth/clientHeight` 设尺寸 → `scale(0.18)` 下 contentRect 取到 44×0.18≈11 → 展开后 layout 不变故不重触发

**修复 — `useFixCanvasUnderTransform`（三层防护）**：核心是用 `offsetWidth`/`offsetHeight`（不受 transform 影响，始终返回 layout 尺寸）替代 Rive 内部尺寸来源；校正逻辑为 `canvas.width = round(offsetWidth * dpr)`、同步 `style.width`，再 `rive.resizeDrawingSurfaceToCanvas()`。三层触发：① mount 时 rive 就绪即校正；② 容器 ResizeObserver；③ `.orb-panel` 的 `transform` `transitionend` + 350ms 超时兜底（transform 变化不触发 ResizeObserver）

**涉及**：`src/mascot/RiveMascot.tsx`（新增 hook + `containerRef` + `useOrbStore` 依赖）

### 2026-10-10 RiveMascot canvas 尺寸守卫重构 — 三层防护 → rAF 轮询 watchdog

**旧方案技术债**：Rive 渲染器依赖 `useOrbStore.isExpanded` 作为"transform 可能变了"的信号，还全局查询 `.orb-panel` 绑 `transitionend`，形成渲染器 → orbStore 的反向依赖；所有 WebGL 渲染器（Live2D/Spine/MMD）都会遇到同一问题

**根因**：CSS `transform` 变化不触发 `ResizeObserver`（不改 layout box），需额外信号触发校正

**新方案 — `useCanvasSizeWatchdog`**：每帧检查 canvas 像素尺寸是否匹配 layout 期望尺寸，不匹配即校正。零外部信号、零 DOM 事件监听、不依赖 `useOrbStore`；页面 `visibilitychange` 隐藏时暂停、恢复时重启

```typescript
const check = () => {
  const layoutW = el.offsetWidth, layoutH = el.offsetHeight;
  const expectedW = Math.round(layoutW * dpr), expectedH = Math.round(layoutH * dpr);
  if (canvas.width !== expectedW || canvas.height !== expectedH) {
    canvas.width = expectedW; canvas.height = expectedH;
    canvas.style.width = `${layoutW}px`; canvas.style.height = `${layoutH}px`;
    rive.resizeDrawingSurfaceToCanvas();
  }
  raf = requestAnimationFrame(check);
};
```

**对比旧方案**：外部依赖 旧=`useOrbStore.isExpanded` + `.orb-panel` DOM 查询 / 新=零；触发 旧=事件驱动（mount+ResizeObserver+transitionend+setTimeout）/ 新=每帧轮询；React 开销 旧=2 useEffect+1 useState+1 useCallback / 新=1 useEffect；校正条件 新=仅尺寸不匹配才调；通用性 新=所有 WebGL 渲染器通用；稳态开销 旧 ~0.05ms/s / 新 ~0.3ms/s（隐藏时 0）；延迟 新 ≤16ms

**为何可行**：`offsetWidth`/`offsetHeight` 读浏览器缓存（O(1)，不触发 layout），整数比较 <0.01ms/帧；页面隐藏时暂停轮询开销归零；16ms 延迟在 300ms transition 中不可感知

**涉及**：`src/mascot/RiveMascot.tsx`（`useFixCanvasUnderTransform` → `useCanvasSizeWatchdog`，移除 `useOrbStore` 依赖）

### 2026-10-10 RiveMascot canvas 尺寸守卫优化 — rAF 持续轮询 → 祖先 transition 感知 + 按需轮询

**问题**：watchdog 每帧检查，稳态持续占用主线程（~0.3ms/s）。纯事件驱动与 settling 守卫两种方案均未能解决，两个根因：

1. **DOM 结构**：`.orb-hitbox`（有 transform+transition）与 RiveMascot 的 `containerRef` 是父子关系，但 `.orb-panel`（另一个有 transform 的元素）是 `.orb-hitbox` 的兄弟节点——在 `containerRef` 上监听 `transitionend` 只收得到 `.orb-hitbox` 冒泡的事件，覆盖不全
2. **Rive 异步设错**：Rive 内部 ResizeObserver 可能在 transition 结束后才异步改 canvas 尺寸，此时 settling 循环已停，无人纠正

**新方案**：向上遍历所有祖先，逐层挂 `transitionrun`/`transitionend`/`transitioncancel`；`transitionrun` 时 `activeTransitionCount++` 并启动持续 rAF 轮询；`transitionend`/`transitioncancel` 时计数递减，归零即停轮询 + 跑 5 帧 settling 兜底；ResizeObserver / matchMedia(resolution) / visibilitychange / 初始 mount 均触发 5 帧 settling

```typescript
const ancestors = []; let node = el;
while (node) { ancestors.push(node); node = node.parentElement; }
// 每层挂 transitionrun/transitionend/transitioncancel
const onTransitionRun = (e) => {
  if (e.propertyName === "transform" || e.propertyName === "opacity") {
    activeTransitionCount++; startPolling();
  }
};
const onTransitionEnd = (e) => {
  activeTransitionCount--;
  if (activeTransitionCount === 0) { stopPolling(); runSettling(); }
};
```

**对比**：稳态开销 ~0.3ms/s → **0**；transition 期间均持续轮询；结束后由"继续轮询"改为"停止 + 5 帧兜底"；兄弟节点 transform 由轮询捕获改为祖先链冒泡覆盖；Rive 延迟设错由"下一帧纠正"改为"5 帧兜底"；初始 mount 由无条件轮询改为 5 帧 settling；外部依赖均零

**涉及**：`src/mascot/renderers/rive/RiveMascot.tsx`（`useCanvasSizeWatchdog` 重构）

### 2026-10-10 I1-orb-no-clickthrough — 移除自动穿透轮询，改用设置驱动穿透

规格：`I1-orb-no-clickthrough-spec.md`

#### 问题

Orb 球体用 80ms 轮询鼠标 vs hitbox 矩形来决定穿透，问题：

1. 球静止时仍每 80ms 调 `getMouseLogicalPos()` + `getWindowLogicalRect()` + `isPointInRect()`
2. 复杂度高：30ms 防抖、8px 手抖容差、双层状态缓存（`clickThrough` store + Rust `AtomicBool`）、多平台分支（`NEEDS_CURSOR_POLL`/`NEEDS_CLICK_THROUGH_TOGGLE`）
3. 语义混淆：`click_through` 用户开关与轮询实现细节耦合，关闭后轮询仍在跑
4. 常量语义不清：`ORB_HITBOX_SIZE` 前端=72（视觉尺寸）、Rust=48（贴边优化 hitbox），是两个概念

#### 设计决策

移除自动轮询，改由 `orbConf.click_through` 驱动穿透——轮询是实现细节、`click_through` 是功能开关，解耦；保留 Rust `set_orb_click_through`（封装平台 API，成本极低）与 `ORB_CLICK_THROUGH` 缓存；前端 `ORB_HITBOX_SIZE` 改为派生 `ORB_COLLAPSED_SIZE + 2*ORB_CHROME_INSET = 88`（hitbox 应铺满窗口），Rust 对齐为 `ORB_COLLAPSED_WINDOW_SIZE = 88`；原 `ORB_VISUAL_CENTER_OFFSET_X` 是 hitbox 派生量，语义已失效，改用独立 `ORB_SNAP_BORDER_X = 24` 解耦贴边偏移

#### 常量语义

`ORB_COLLAPSED_SIZE` = 72（球体视觉渲染尺寸，MascotRenderer 的 width/height）；`ORB_HITBOX_SIZE` = 88 = 72 + 2×8（交互命中区域，铺满窗口）；`ORB_SNAP_BORDER_X` = 24（贴边时右边缘补偿偏移）。Rust 原 hitbox=48 不是遗留 bug 而是有意的贴边优化，解耦后 hitbox 可独立变化

#### 穿透驱动逻辑

**旧**：轮询检测鼠标 → 自动切换 → `setClickThrough` store 同步；**新**：`click_through` 配置变更时生效 + 展开/拖拽临时互斥

```
收起态：穿透 ON（仅 click_through=true）
  │ 展开 / 拖拽 → 临时关闭（面板/球需要事件）
展开态/拖拽中：穿透 OFF
  │ 收起 / 松手 → 恢复
```

`click_through = false`（默认）时所有切换 if 块不执行，穿透始终 OFF

#### 修改清单

`constants.ts`：`ORB_HITBOX_SIZE` 72→88（派生），移除 `HOVER_POLL_INTERVAL_MS`/`CLICK_THROUGH_DEBOUNCE_MS`/`HAND_JITTER_PX`；`constant.rs`：hitbox 48→88（派生），`ORB_VISUAL_CENTER_OFFSET_X`→`ORB_SNAP_BORDER_X=24`，移除 `HOVER_POLL_INTERVAL_MS`；`global.css`：`--orb-hitbox-size` 72→88px，移除 `div[data-orb-window]` 的 `pointer-events: none !important`；`OrbBall.tsx`：删全部轮询（~100 行），`MascotRenderer size`→`ORB_COLLAPSED_SIZE`，收起态 `ballLeft/ballTop`→0，enter/leave 直接 `setHovering`；`OrbExpand.ts` / `OrbDrag.tsx`：移除 `NEEDS_CLICK_THROUGH_TOGGLE`/`setClickThrough`，改配置驱动；`App.tsx`：移除硬编码穿透初始化；`orbStore.ts`：移除 `clickThrough` 状态与 action；`useOrbConfSync.ts`：`applyOrbConf` 加 `invoke("set_orb_click_through", { enabled: conf.click_through })`；`orb.rs`：import 改 `ORB_SNAP_BORDER_X`；`hover.rs`：移除 `should_toggle_click_through` 及测试；`constant.rs` 测试：更新断言、移除废弃常量断言

#### 移除 vs 保留

**移除**：前端 `NEEDS_CURSOR_POLL`/`NEEDS_CLICK_THROUGH_TOGGLE`、三个轮询常量、OrbBall 全部轮询逻辑（`setInterval`/`applyClickThrough`/防抖/容差/refs）、`orbStore.clickThrough`、App.tsx 硬编码初始化；Rust `HOVER_POLL_INTERVAL_MS`、`ORB_VISUAL_CENTER_OFFSET_X`、`should_toggle_click_through` 及测试

**保留**：`set_orb_click_through` 命令（封装 `set_ignore_cursor_events`，将来可能复用于 macOS 原生穿透或快捷键切换）、`ORB_CLICK_THROUGH` AtomicBool 缓存（避免冗余系统调用）、`OrbConf.click_through` 设置项（经 `useOrbConfSync` 生效）、展开/拖拽的临时穿透切换

### 2026-10-11 首次悬浮触发小球消失闪烁

**问题**：球体首次鼠标悬浮瞬间消失再恢复，后续不再复现

**根因**（多处初始化时序竞争叠加）：

1. **Rive canvas 初始尺寸异常**：挂载后 canvas 像素尺寸是占位值（如 11×11），watchdog 需若干帧才修正到 72×72×DPR，期间布局跳变打断 CSS transition
2. **CSS 变量延迟就绪**：`useOrbConfSync` 在 `useEffect` 里异步 `invoke("get_orb_conf")`，`--orb-opacity`/`--orb-idle-opacity`/`--orb-scale` 首帧不存在，`opacity`/`transform` 用默认值；配置加载后变量写入 DOM 与 `orb-idle`→`orb-hover` 类切换同帧生效，transition 无起始值可过渡，直接跳变
3. **breathe 动画与 transition 互斥**：`orb-idle` 下 `orb-breathe` 动画持续跑 `transform: scale()`，首次 hover 时动画移除 + `scale(1.08)` transition 同帧启动，浏览器可能以动画中间值作起始值
4. **`orb-hover-hide` 类竞态**（`hide_on_pointer_hover` 启用时）：该类在鼠标已 hover 后才加到 `<html>`，`:hover` 与 `orb-hover-hide` 同时满足，`opacity: 0 !important` 立即生效而 `transition` 同帧设置

**核心矛盾**：首次 hover 时 transition 起始状态（canvas 尺寸、CSS 变量、动画中间值）未稳定，基于错误起始值运行

**修复**：`OrbBall` 挂载后引入"稳定期"（`orb-settling` 类）禁止 transition 和 animation，2 帧后再启用。`OrbBall.tsx`：新增 `settled` state，挂载后 `requestAnimationFrame` × 2 置 `true`，未稳定时 hitbox 加 `orb-settling` 类，`handlePointerEnter` 在未稳定时忽略；`global.css`：`div[data-orb-window] .orb-hitbox.orb-settling { transition: none !important; animation: none !important; }`

**时序**：挂载→2 帧内 settling 存在、transition/animation 禁止、hover 忽略；2 帧后全部启用。稳定期约 32ms（2 帧@60fps），用户无感知，足以让 watchdog 完成首次修正与 CSS 变量生效

**涉及**：`src/orb/OrbBall.tsx`、`src/assets/styles/global.css`

### 2026-10-13 Orb 初始化态稳定化 — 屏幕内检查统一 + 显式 collapse + 闪烁消除

#### 问题

1. 球可能出现在错误位置（上次退出为展开态时，保存的 `position_x/y` 是展开态窗口位置）
2. `click_through`/`expandDirection`/`expandVertical` 依赖隐式约定而非显式同步
3. 首次 hover 时 opacity 从 0 渐显 150ms（`orb-settling` 移除后 transition 生效）产生闪烁
4. React StrictMode 使 `collapseOrbOnInit` 被调用 3 次（`confReady` 引用变化触发 effect 重执行）

#### 修复 1 — 屏幕内检查统一

`bounds_inside_screens` + visual bounds + snap 修正的模式在 3 处重复（`setup.rs` 启动定位、`setup.rs` 巡检线程、`cmd/orb.rs` snap），且 `position_orb_window` 启动用 expanded 尺寸（420×600）+ outer bounds，与另两处不一致。提取 `ensure_orb_on_screen()`（`placement.rs`）统一 visual bounds 判断 + outer bounds snap，3 处全部改用；`position_orb_window` 改 collapsed 尺寸（88×88）；移除从未被调用的 `check_and_snap_orb`

#### 修复 2 — 初始化显式 collapse

`collapse_orb_panel` 从未被调用，Rust 侧窗口尺寸/位置与前端 `isExpanded`/`isBallVisible`/`click_through` 全靠隐式约定。新增 `collapseOrbOnInit()`（`OrbExpand.ts`），在 settling 流程中调用；与用户交互 `collapse()` 的区别是跳过 `waitForPanelTransition()`（初始化时面板不可见）。一次解决三件事：Rust 窗口归位、`isExpanded=false`/`isBallVisible=true` 显式设置、`click_through` 显式应用（内部 `await loadOrbConf()` 自加载，不依赖外部时序）

#### 修复 3 — 初始化时序链

`orb-settling`（opacity:0）移除时机偏早，多个异步依赖可能未就绪。完整时序：

```
mount → orb-settling (opacity:0, transition:none, animation:none)
  → await confReady       ← useOrbConfSync 首次 apply 完成（CSS 变量生效）
  → rAF × 2               ← DOM 稳定
  → collapseOrbOnInit()   ← Rust 归位 + store 显式同步
  → rAF                   ← 等 Rust resize 被 paint
  → settled + settledIn = true   ← 移除 orb-settling，启用 orb-settled-in（transition:none）
  → rAF                   ← opacity 瞬间跳到目标值
  → settledIn = false     ← 恢复 transition，值已在目标态无跳变
```

`useOrbConfSync` 改为返回 `{ ready: Promise<void> }` 暴露首次 apply 完成；`global.css` 加 `.orb-settled-in { transition: none !important }`，保证 settling 移除时 opacity 瞬间跳到 `--orb-idle-opacity` 而非 150ms 渐显

#### 修复 4 — StrictMode 下 effect 只执行一次

`confReady` 每次渲染是新 Promise，作 effect 依赖会重触发，StrictMode 双执行又加一次，共 3 次。改为 `confReadyRef = useRef(confReady)`（每渲染更新 `.current`，effect 依赖 `[]`）+ `initGuardRef = useRef(false)` 守卫，首次执行后置 `true`

#### 涉及文件

`placement.rs`：新增 `ensure_orb_on_screen()`，移除 `check_and_snap_orb` 与 `Manager` import；`setup.rs`：`position_orb_window` 改 collapsed 尺寸 + `ensure_orb_on_screen`，巡检同改，移除 `ORB_EXPANDED_*` import；`cmd/orb.rs`：`snap_orb_to_screen` 改用 `ensure_orb_on_screen`，用 `pos` 替代 `bounds` 比较；`OrbExpand.ts`：新增 `collapseOrbOnInit()`；`OrbBall.tsx`：`initGuardRef` + `confReadyRef` 守卫、`settled`/`settledIn` 双状态、`orb-settled-in` class；`useOrbConfSync.ts`：返回 `{ ready }` + `readyRef`；`global.css`：新增 `.orb-hitbox.orb-settled-in`

### 2026-10-30 死代码清理 + 特性清单动态装载

**清理**：`docs/store/` 6 份过期规格移入 `docs/achievement2/`（逐字节一致，该目录只留活跃规格）；删除零引用的 `assistant-vault/src/contract/mod.rs`（仅含未使用的 `Precondition` 枚举）与 `assistant-config/tests/constant.rs`；后端 `constant.rs` 删 12 个、前端 `constants.ts` 删 19 个无引用常量；`assistant-vault/Cargo.toml` 补登 `git-tests` feature，使 `tests/sync_engine.rs` 的 feature 门控真正生效

**特性清单**：`__VAULT_ENABLED__ ? [vaultModule] : aggregate([])` 静态三元折叠在 `VAULT_ENABLED=0` 下仍残留 7 个 vault chunk 与入口字段字面量。根因：折叠只删引用不删 import，`import { vaultModule } from "./vault/module"` 退化为副作用 import，模块内 `load: () => import("@/vault")` 等箭头函数里的 `import()` 照常被物化。对照实验：同一 import 只要源码完全不引用（`aggregate([])`）产物即 0 chunk，证明模块本身无副作用

**改为运行时动态装载 + 编译期守卫**：`loadFeatures()` 内 `if (!__VAULT_ENABLED__) return;` 早退后才 `await import("./vault/module")`，`main.tsx` bootstrap 首行 `await loadFeatures()`；守卫折叠成 `if (true) return;` 后整段不可达被删，chunk 边不存在

| 写法（`VAULT_ENABLED=0`） | 文件数 | vault chunk |
|---|---|---|
| 静态 import + 三元折叠 | 172 | 7 |
| 动态 import + 守卫早退 | 158 | 0 |

`VAULT_ENABLED=1`：175 文件 / 7 chunk，行为不变。

**踩坑**：
- 常量与集成测试硬绑定：删后端常量直接击穿 `tests/constant.rs` 编译，保留半数断言无意义只能整文件删——清理常量前先确认对应测试是否被连带
- 别用 `generateBundle` 插件删不可达 chunk 补救：Rolldown 下 `output.importers` 不存在（仅模块级 `ModuleInfo` 有），插件直接抛错，且属过度特化补丁
- 消费方若保留 `export const SETTINGS_PAGES = getFeatures()...` 这类静态导出，模块求值期就固化动态装载结果；派生必须改为「导出函数 + 内部缓存」，缓存以 `derived.features === features` 引用相等失效
- 验收不能只看字段字面量消失，必须数 chunk：静态方案字段删净、chunk 却留着
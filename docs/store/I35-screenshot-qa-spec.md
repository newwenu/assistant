# I35 — 截屏提问规格

- **状态**：Draft（rev10）
- **日期**：2026-11-03（rev10：2026-11-17）
- **修订**：按版本倒序记录
  - rev10：真机验收发现「开始截图时小球出现在屏幕左上角」——根因是菜单/托盘入口由 Rust 直接发起，前端没有工具栏入口那次 `beginPreparing()`，窗口被撑成全屏时仍渲染小球，绝对定位落到新窗口原点（F58）。新增 `screenshot-preparing` 事件，`start` 在抓屏前先通知前端进入覆盖层模式（不变式 25）。同时把 `restore` 的顺序由「先改几何后显影」改为「先隐藏 → 改几何 → 按快照恢复可见性」：可见的全屏窗口上 `set_size`/`set_position` 是两次独立主线程消息，中间会合成一帧「小球在原点」（F59，不变式 26）。另记一条构建口径：dev `opt-level=0` 下抓屏+JPEG 预览编码实测慢约 6 倍，`profile.dev` 已设 `opt-level = 1`（§13 风险 19）。**菜单淡出等待由「固定毫秒」改为「按帧排空」**：实测 Win11 默认动画的菜单关闭是 19 个 DWM 合成帧的线性过渡，与菜单项数量无关；固定毫秒在高刷屏上白等、低刷屏上漏帧，这正是此前「等待时长对不上」的根因（F60，`SCREENSHOT_MENU_FADE_FRAMES = 20` + `SCREENSHOT_MENU_FADE_CAP_MS = 800`）。
  - rev9：撤回菜单「捕获排除」路线（看护线程 + `WDA_EXCLUDEFROMCAPTURE`）：`TrackPopupMenu` 返回时菜单窗口可能已销毁，枚举不到窗口不等于屏幕上没有残影，打标时机也无法与点击对齐（F56）。改为「点击即锚定 + 抓屏前排空」：菜单事件回调（`menu.rs` / `tray.rs` 的截图项，即点击那一刻）无条件隐藏残余菜单并写入淡出起点，抓屏前按 `SCREENSHOT_MENU_FADE_MS` 补齐剩余时长再双 `DwmFlush`；锚点带新鲜度判定，避免上一次交互的旧锚点压住本次（否则等待会被算成 0）。F56/F57、不变式 23/24 与 §8.2 步骤 7、§8.7 测试口径同步改写。
  - rev8：几何写入按 §8.5.1 回退方案定稿——核对 Tauri 2.12.5 源码确认 `WebviewWindow` 没有 `set_bounds`（仅 `Webview` 有），覆盖层与恢复统一走「先 size 后 position」并写回快照值；macOS 权限前置校验补 `CGPreflightScreenCaptureAccess` 预检 + 显示器实测 + 成功缓存/失败短间隔重试，前端错误描述改为 `screenshot.permission_guide` 引导文案；`orb_overlay` 补 `FakeOps` 单测（隐藏序列、兜底、失败码、快照与还原）；DWM 过渡禁用扩到「隐藏 / 显影 / 还原」三段，写回值改用 `DwmGetWindowAttribute` 读到的原值；Windows 弹出菜单（`#32768`）从「只 `SW_HIDE`」升级为两层——菜单可见期由看护线程打 `WDA_EXCLUDEFROMCAPTURE`（F56），排除失败时抓屏前按 `SCREENSHOT_MENU_FADE_MS` 补齐淡出并双 `DwmFlush`（F57）。
  - rev7：底图与产物链路去掉文件系统——预览改「内存 JPEG data URL」，裁剪产物由 `crop_screenshot` 直接归档返回 `attachmentId`；删除 `screenshot-tmp` 目录、`release_screenshot` 命令、TTL 清扫、asset scope 改动与 `AttachmentStore` 新增函数；抓屏隐藏补「可见性校验 + `ShowWindow` 兜底 + `DwmFlush`」；裁剪管线收敛为单次编码；修正「`clearSession` 晚于几何恢复」的闪帧顺序；新增 §6.4 落地前置 spike 与「分层窗口无法捕获排除」的限制说明。
  - rev6：§14 参考项目对照逐条补「采用 / 部分采用 / 暂缓 / 不采用」决定，并新增 §14.4 采纳结论一览，明确「只借鉴语义与参数、不搬实现」的口径。
  - rev5：按当前代码重校既有事实——附件链路已改为「`import_attachment` 归档 + `attachmentId`」，`file_path` 通道被后端封闭；asset scope 是单层通配，须显式放行 `screenshot-tmp`；补 `ORB_CLICK_THROUGH` 镜像、`apply_orb_window_attrs` 两处守卫；`ChatView` 更正为 `ConversationView`；契约测试无数字断言。
  - rev4：抓屏只隐藏覆盖层宿主 `orb`，`main` 不再隐藏。
  - rev3：取消新增 webview，改为复用 Orb 窗口做全屏覆盖层；chat 自隐藏且保留草稿；快捷键注册改为表驱动并更换默认组合。
- **上游调研**：本仓库代码核对 + OpenBitFun、airi、Cap、Kiri、shiping 五个参考项目对照
- **关联规格**：I2e-multimodal-attachments-spec（已归档，附件链路）、I44-app-dir-management-spec（已归档，附件归档与 `import_attachment`）、I4-spec（已归档，全局快捷键）
- **影响范围**：复用 Orb 窗口（**不新增 webview**）、Rust 模块 `screenshot`、5 条命令、3 个 DTO、2 个事件、i18n、设置提示；`cmd/attachment.rs` 抽出 `archive_bytes` 供截图复用；`Cargo.toml` 新增 `xcap` 与直接 `image` 依赖。**不改** `tauri.conf.json`（asset scope 无改动）、`assistant-storage`、`assistant-agent` 的准入判断与上限

本文是落地规格。所有「既有事实」均已对照源码核对，标注了文件位置；**未核对的第三方库 API 单独标注为「落地时核实」**，不作为契约。

---

## 1 结论摘要

截屏提问 = **抓屏 → 框选 → 变成一条带图片的用户消息**。三段里第三段在仓库中已经完整存在。

**rev3 的核心决定：不再新增 `screenshot` webview，复用现有 Orb 窗口做全屏覆盖层。**

- **抓屏**（现状：无）
  - 改动：新增 `src-tauri/src/screenshot/`：`xcap` 后端 + 可注入 `CaptureBackend`；抓屏时只隐藏覆盖层宿主 `orb`，隐藏后做「可见性校验 → `ShowWindow(SW_HIDE)` 兜底 → `DwmFlush`」，不靠固定延时
- **框选**（现状：无）
  - 改动：把 Orb 窗口临时放大到光标所在显示器，前端在同一个 webview 里渲染覆盖层；chat/Orb 面板隐藏但保持挂载；底图是**内存 JPEG 的 data URL**，不写盘
- **图片进消息**（现状：已存在，内容寻址存储 + `attachmentId` 进消息）
  - 改动：`crop_screenshot` 裁剪后**在命令内直接归档** `AttachmentStore`（与 `import_attachment` 共用 `archive_bytes`）并返回 `attachmentId`；前端把结果构造成附件放进 `attachmentStore`，全程无临时文件

为什么复用 Orb 而不是新开窗口：

- **不新增 webview**：少一个 WebView2/WKWebView 实例、少一份 JS 运行上下文、少一套 `viewMap` / `capabilities` / 布局入口；这是 rev2 的主要重量来源。
- **结果直通**：裁剪在 Orb 的同一 JS 上下文里完成，`crop_screenshot` 直接返回已归档的 `attachmentId`，不需要 `screenshot-captured` 事件、跨 webview 状态同步或 `get_screenshot_session` 的常规路径（只保留兜底）。
- **chat 自隐藏**：`OrbWindow` 是唯一的覆盖层宿主；截图期间用 CSS 隐藏 `OrbBall`/`OrbPanel`，`ConversationView` **保持挂载**，因此输入草稿、附件列表和流式状态都不丢。
- **只隐藏宿主**：抓屏前只隐藏 `orb` 一个窗口，`main` / `settings` 保持用户原状并进入底图，因此「用户触发瞬间所见 = 冻结底图」，也不需要为它们维护可见性快照。
- **代价**：Orb 窗口的几何、可见性、焦点、置顶、点击穿透需要快照/恢复；`spawn_screen_inspection` 与 `ORB_BLUR` 必须暂停，否则会把全屏几何写回 `OrbConf` 或触发折叠。

本规格**不改动** `assistant-agent` 的附件准入判断与上限、`assistant-storage` 的附件存储格式、`contentBlockUtils` 的序列化、Provider 的请求投影，也**不改动** `tauri.conf.json` 的 asset scope。后端只在 `cmd/attachment.rs` 抽出 `archive_bytes`（`import_attachment` 与 `crop_screenshot` 共用同一段「大小上限 + `save_attachment`」逻辑），不改变任何准入结果；截图链路**不落地任何临时文件**。

### 1.1 rev3–rev7 关键修正（实现前必读）

- rev2：新开 `screenshot` webview，重量大 → rev3：复用 Orb 窗口；不新增窗口、不加 `capabilities`、不改 `viewMap`
- rev2：结果需要 `screenshot-captured` 事件 + 跨窗口订阅 → rev3：裁剪结果由 orb 直接 ingest；只保留 `screenshot-session` 启动事件与 `screenshot-error` 失败通知
- rev2：覆盖层拿不到会话 → rev3：orb 在截图前调用 `start_screenshot`；会话通过 `screenshot-session` 事件下发，`get_screenshot_session` 兜底
- rev2：截图时 chat 是否隐藏未定义 → rev3：新增 `screenshotStore`；`OrbWindow` 用 CSS 隐藏 `OrbBall`/`.orb-panel`，`ConversationView` 不卸载，草稿保留
- rev2：Orb 的几何/位置可能被巡检线程写坏 → rev3：截图期间暂停 `spawn_screen_inspection`、`snap_orb_to_*`、`save_orb_position_sync`；恢复后写回原几何
- rev2：失焦会把 Orb 折叠 → rev3：Rust 在截图期间跳过 `ORB_BLUR`；前端扩展 `isOrbBlurCollapseSuppressed()` 再加会话判断
- rev2：前端 `toPhysical` 与 Rust 重复换算 → rev3：前端只发归一化选区（0..1），Rust 按原图尺寸换算一次；跨 DPI 不再依赖 `scale_factor`
- rev3：抓屏隐藏全部应用窗口，底图与用户所见不符（从 Orb 触发时截不到 main） → rev4：只隐藏覆盖层宿主 `orb`；`main` / `settings` 保持原状并进入底图
- rev3：契约 5 个 DTO / 57 → 62 → rev3 只需 3 个 DTO（会话、裁剪参数、裁剪结果）
- rev3：快捷键注册写死数组 + `match` → 已落地：`SHORTCUT_SPECS` 元组表 + `assistant-config::shortcut::resolve_bindings` 纯函数解析；空配置=禁用、重复/非法绑定统一告警
- rev3：默认 `Ctrl+Shift+N` 冲突多 → 已落地：三条默认改为四键组合（见 §7.5.2）；前端默认值由 `contract:gen` 生成的 `shortcutDefaults.generated.ts` 提供
- **rev4：`file_path` 直通附件管道** → **rev5：I44 已把路径附件改为「`import_attachment` 归档 → `attachmentId`」，后端显式拒绝 `file_path`；截图改走归档入口，临时文件在导入时消费**
- **rev4：asset scope 写作 `attachments/**`** → **rev5：实际是 `attachments/*`（单层），必须显式追加 `attachments/screenshot-tmp/*`，否则底图加载被拒**
- **rev4：契约断言 57 → 60** → **rev5：`protocol_bindings.rs` 只比对派生名集合，无数字断言；新增 DTO 只需登记**
- **rev4：`ChatView.tsx` 为聊天入口** → **rev5：仓库没有该文件；聊天 UI 是 `ConversationView`，附件按钮在 `ConversationFrame` 工具栏**
- **rev4：截图按钮沿用 `!supportVision` 禁用** → **rev5：附件按钮只按流式禁用；vision 由准入层 `no_vision` 拦截，截图按钮沿用前者**
- **rev4：恢复 Orb 只调 `set_ignore_cursor_events`** → **rev5：必须同步 `ORB_CLICK_THROUGH` 镜像，否则设置里的点击穿透永久失效**
- **rev4：守卫表漏了 `apply_orb_window_attrs`** → **rev5：截图期间跳过，避免设置变更把全屏态改回 conf 值**
- **rev6：预览 PNG 写 `screenshot-tmp` 并改 asset scope** → **rev7：预览改为内存 JPEG data URL（CSP 已允许 `data:`/`blob:`），asset scope、`AppDirs`、`dir_name` 全部不改**
- **rev6：裁剪产物落盘 + `import_attachment` 消费后删源 + `release_screenshot` 兜底** → **rev7：`crop_screenshot` 在命令内直接归档返回 `attachmentId`；无临时文件、无 TTL 清扫、无任意路径删除命令**
- **rev6：隐藏 Orb 后固定 `sleep(150ms)` + `IsWindowVisible` 校验** → **rev7：可见性查询确认 → 对残留 HWND 补 `ShowWindow(SW_HIDE)`（Tao 的 `set_visible` 只比标志位，F53）→ `DwmFlush`；固定值降级为兜底上限**
- **rev6：裁剪 → PNG → `normalize_image`（再次 decode/resize/encode）** → **rev7：裁剪 → resize ≤2048 → 单次 Fast 压缩编码，超限才 JPEG q85（§8.4）**
- **rev6：前端 `await ingestPath` 之后才 `clearSession()`** → **rev7：先卸载覆盖层并进入处理中态，再归档/展开；否则全屏底图会闪进小球窗口（§9.4、不变式 21）**
- **rev6：把捕获排除整体推到 V2** → **rev7：写明 V2 捕获排除在 Orb 上大概率不可用（透明分层窗口 + `SetWindowDisplayAffinity` error 8），并新增 §6.4 前置 spike 决定窗口模型**
- **rev9 漏了「入口不在前端」这一层** → **rev10：菜单 / 托盘 / 快捷键入口由 Rust 直接调 `start`，前端没有工具栏那次 `beginPreparing()`；窗口撑成全屏时小球仍按绝对定位渲染，被钉在新窗口原点（左上角）。新增 `screenshot-preparing` 事件，`start` 在抓屏前先通知前端进入覆盖层模式（不变式 25、F58、§8.2 步骤 3）**
- **rev8 的恢复顺序「先改几何后显影」** → **rev10：可见的全屏窗口上 `set_size` 与 `set_position` 是两次独立主线程消息，中间会合成一帧「窗口已缩成小球但仍在原点」；改为「先隐藏 → 改几何 → 按快照恢复可见性」，异常残留复位同改（不变式 26、F59、§8.5.1）**

---

## 2 目标

1. 全局快捷键一次触发，光标所在显示器整屏变暗并进入框选。
2. 框选结果以图片附件形式进入 Orb 输入区，用户补一句话即可发送；不自动发送。
3. 截图附件与「粘贴 / 文件选择 / 拖拽」走完全相同的准入、上限、预览、存储路径。
4. 取消（ESC / 右键 / 关闭）后不留下任何临时文件，不残留覆盖层；截图链路全程不写临时文件（底图在内存，产物直接入库）。
5. 截图前后的 Orb 可见性、焦点、位置与尺寸恢复到用户原本状态；`main` / `settings` 全程不受影响。
6. **不新增 webview**；整个覆盖层运行在现有 Orb webview 中。
7. 截图期间 chat/Orb 面板自隐藏，输入草稿不丢，Orb 不折叠、不被巡检线程写回错误位置。
8. 4K / 混合 DPI 下选区与最终产物一致，产物 ≤ 8MB。
9. 全局快捷键注册可扩展、可诊断；默认组合避开高频应用占用。

## 3 非目标

- 标注能力（箭头、矩形、马赛克、文字）。
- 跨显示器拼接框选（V1 只截单屏；跨屏属 V2）。
- 窗口级捕获（按 HWND 截某个窗口）。
- 滚动截图、延时截图、录屏。
- 自动发送截图（不提供默认开启的开关，避免误触把敏感界面发出去）。
- 截图历史库、OCR 文本降级。
- 修改 `assistant-agent` 的附件准入策略与上限。
- 原生渲染覆盖层（Cap/GPUI 路线）V1 不引入；V2 视 Orb 复用效果再评估。
- 调用系统截图工具（Win+Shift+S / macOS Cmd+Shift+4）作为默认实现；V2 可作为轻量备选。
- V1 不做锁屏/保险库锁定的截屏 gate（F30 的已知缺口）。

---

## 4 既有事实

- **F1**：`tauri.conf.json` 的 `app.windows` 静态声明 `main` 与 `orb`；`settings` 是运行时创建
  - 位置：`src-tauri/tauri.conf.json`、`src-tauri/src/window.rs` `WINDOW_SPECS`
  - 影响：rev3 不新增窗口；`screenshot` 覆盖层直接复用 `orb`
- **F2**：前端按 webview label 选布局，未匹配 label 回退 `MainLayout`
  - 位置：`src/App.tsx` `viewMap`
  - 影响：不新增 label，因此不需要改 `viewMap`
- **F3**：`capabilities/default.json` 的 `windows` 只有 `main` / `orb`；`settings` 是运行时创建，不在列表中
  - 位置：`src-tauri/capabilities/default.json`
  - 影响：不新增窗口，因此不需要改 capabilities；asset scope 在 `tauri.conf.json`，与 capabilities 无关（见 F19）
- **F4**：全局快捷键注册已表驱动：`SHORTCUT_SPECS` 元组表 + `match id` 分派
  - 位置：`src-tauri/src/setup.rs` `register_global_shortcuts`
  - 影响：新增 `capture_screenshot` 只需加一行表项 + 一个分派分支
- **F5**：快捷键字符串解析支持 `Meta/Super/Cmd/Win/Windows`、`Ctrl/Control`、`Shift`、`Alt/Option`；单字母回退 `KeyX`，数字回退 `DigitX`
  - 位置：`src-tauri/src/setup.rs` `parse_shortcut`
  - 影响：四键组合可直接用，无需改解析器
- **F6**：快捷键配置变更会触发重新注册（`keybindings` 字段比对）
  - 位置：`src-tauri/src/cmd/config.rs` `APP_SIDE_EFFECTS`
  - 影响：新增/修改快捷键自动获得「设置页改键即时生效」
- **F7**：前端快捷键定义表含 `id/group/label/defaultBinding/kind`；默认值来自 `npm run contract:gen` 生成的 `src/types/shortcutDefaults.generated.ts`，`shortcutTypes.ts` 只做展开
  - 位置：`src/settings/core/types/shortcutTypes.ts`、`src/types/shortcutDefaults.generated.ts`
  - 影响：默认值唯一来源是 Rust `DEFAULT_SHORTCUT_PATTERNS`；改完后重跑 `contract:gen`，禁止手写前端默认值
- **F8**：默认值已收敛为 `DEFAULT_SHORTCUT_PATTERNS` + `default_keybindings()`，Rust 注册与配置默认值共用
  - 位置：`src-tauri/crates/assistant-config/src/app_conf.rs`
  - 影响：新增快捷键只改这一处常量表
- **F9**：设置页快照 fixture 只投影字段 key，不含默认值
  - 位置：`src/settings/__tests__/fixtures/snapshot.full.json`、`snapshot.vaultless.json`
  - 影响：改默认值本身不影响 fixture；新增快捷键会新增一行字段
- **F10**：附件模型 `Attachment` **已删除 `filePath`**：路径型附件由 `createPathAttachment()` 调 `import_attachment` 归档，用返回的 `attachmentId` 构造，`file` 是空 `File` 占位
  - 位置：`src/chat/attachment/types.ts`、`factory.ts`
  - 影响：截图产物必须走同一条归档入口；不存在「只传路径」的捷径
- **F11**：`createPathAttachment(filePath)` 是**异步**函数：按扩展名预筛 MIME → `import_attachment` → 用后端返回的 `mediaType`/`bytes`/`attachmentId` 构造附件；预览走 `attachmentUrl(id)`
  - 位置：`src/chat/attachment/factory.ts`、`paths.ts`
  - 影响：截图扩展名必须跟随真实编码；结果需 `await`，并可接受可选的 `width/height`
- **F12**：`useAttachmentIntake()` 内部已有 `ingestPath()`（异步），但接口未导出；拖拽订阅也在该 hook 内
  - 位置：`src/chat/attachment/intake.ts`
  - 影响：rev7 **不改它**——截图不再走路径导入，`ingestPath` 保持内部实现；`ConversationView` 的粘贴/选文件逻辑不受影响
- **F13**：`buildContentBlocks*` 只输出 `{ type: "image", attachment: { attachmentId, ... } }`，**已删除 `file_path` 分支**
  - 位置：`src/chat/core/contentBlockUtils.ts`、`contentBlockTypes.ts`
  - 影响：截图必须归档后才能进消息；无 `attachmentId` 时只剩 `cached_data` base64 通道
- **F14**：后端 `admit_attachments` 对 `file_path` 直接拒绝（「attachments must be archived first」）
  - 位置：`src-tauri/crates/assistant-agent/src/multimodal.rs`
  - 影响：准入零改动；截图临时文件在**导入**时消费，而不是准入时
- **F15**：`AttachmentStore::save_attachment` 按 sha256 内容寻址、同名即跳过
  - 位置：`src-tauri/crates/assistant-storage/src/attachment_store.rs`
  - 影响：同一张截图重复发送不会产生副本
- **F16**：前端 `admitAttachment` 只校验种类数量（图片 4 张）；单张 8MB 由后端 `import_attachment`（`ATTACHMENT_LIMITS.max_image_bytes`）与 `admit_attachments` 判定；`validateImageFile` 只对粘贴的 `File` 生效
  - 位置：`src/chat/attachment/admit.ts`、`src-tauri/src/cmd/attachment.rs` `plan_import`
  - 影响：裁剪产物仍须由 Rust 保证 ≤ 8MB；`import_attachment` 会二次校验，超限直接 `too_large`
- **F17**：`normalize_image()` 已实现：真实格式校验、长边 2048、PNG/JPEG 重编码、BMP/TIFF 转 PNG、GIF/WebP 透传；不会把 PNG 转 JPEG
  - 位置：`src-tauri/crates/assistant-agent/src/normalization.rs`
  - 影响：rev7 **不再调用它**——截图在裁剪命令内一次编码（F52）；它继续服务发送时的准入归一化
- **F18**：`image` 依赖只启用 `png/jpeg/bmp/tiff` feature
  - 位置：`src-tauri/Cargo.toml`
  - 影响：输出只能是 PNG 或 JPEG
- **F19**：asset protocol scope 是 `$APPDATA/mascot-assets/**` 与 `$APPDATA/attachments/*`——**后者单层通配，不覆盖子目录**；CSP `img-src` 含 `asset:`、`data:`、`blob:`
  - 位置：`src-tauri/tauri.conf.json` `app.security.assetProtocol.scope`、`app.security.csp`
  - 影响：rev7 底图走 data URL，**不动 scope**；只有将来回退成落盘预览才需要显式放行子目录
- **F20**：`opener:allow-open-path` 的 scope 是 `$APPDATA/attachments/*`（单层）
  - 位置：`src-tauri/capabilities/default.json`
  - 影响：不要依赖 `openPath` 打开截图
- **F21**：`clean_tmp()` 只扫 `attachments` 根目录的 `*.tmp`，不递归子目录
  - 位置：`assistant-storage/src/attachment_store.rs`
  - 影响：rev7 不再依赖它：截图不落盘，也就不需要子目录与独立 sweep
- **F22**：显示器枚举返回物理像素 bounds
  - 位置：`src-tauri/crates/assistant-orb/src/placement.rs` `get_available_screens`
  - 影响：抓屏、Orb 全屏定位、裁剪统一以物理像素为准
- **F23**：`get_screen_size(center_x, center_y, scale_factor)` 按「中心点最近」选显示器，但它是 Tauri command；`geometry.rs` 里已有同名私有 `nearest_screen(screens, WindowBounds)`（中心距 + 重叠面积）
  - 位置：`src-tauri/src/cmd/orb.rs`、`src-tauri/crates/assistant-orb/src/geometry.rs`
  - 影响：把 `geometry::nearest_screen` 提为 `pub` 并经 `placement` 导出；点坐标用零尺寸 `WindowBounds` 传入即可复用同一距离度量
- **F24**：`get_mouse_position()` 用 `mouse_position` crate 返回 `client_x/client_y`，**坐标空间未在代码中声明**；`get_screen_size` 却把前端值乘 `scale_factor` 当物理用
  - 位置：`src-tauri/src/cmd/orb.rs`
  - 影响：不依赖它做覆盖层定位：优先用 Tauri `Manager::cursor_position()`（物理坐标），不可用时才回退 `get_mouse_position()` 并按该显示器 `scale_factor` 换算；`None` 时回退 Orb 窗口中心（见 §8.2）
- **F25**：Orb 展开/收起由 Rust 重算窗口尺寸与位置；`hide()` 无过渡回调
  - 位置：`src-tauri/src/window.rs`
  - 影响：抓屏前隐藏窗口必须自行 delay
- **F26**：Orb 右键菜单项在 Rust 端构造；菜单点击统一在 `menu.rs` 分发
  - 位置：`src-tauri/src/window.rs`、`src-tauri/src/menu.rs`
  - 影响：新增菜单项要同时改这两处
- **F27**：聊天 UI 是 `ConversationView`（由 `OrbPanel` 引入），主窗口没有聊天输入框；附件按钮在 `ConversationFrame` 工具栏
  - 位置：`src/chat/ConversationView.tsx`、`src/chat/layouts/ConversationFrame.tsx`、`src/orb/OrbPanel.tsx`
  - 影响：结果固定回写 Orb；主窗口不参与覆盖层，也不被隐藏（rev4）。**仓库中不存在 `ChatView.tsx`**
- **F28**：契约测试比对 `declarations` 与 `src/protocol/dto` 的 TS 派生名集合，**没有数字断言**
  - 位置：`src-tauri/tests/protocol_bindings.rs`
  - 影响：新增 DTO 只需在 `protocol/dto/mod.rs` 登记并重跑 `contract:gen`，无需改常量
- **F29**：`MessageInput` 没有暴露 focus 句柄
  - 位置：`src/chat/message/MessageInput.tsx`
  - 影响：需要补 ref 或焦点事件
- **F30**：`ScreenLockMonitor` 只在 `#[cfg(feature = "vault")]` 的 vault 模块内构造，未作为全局 State 管理
  - 位置：`src-tauri/src/cmd/mod.rs`、`src-tauri/src/cmd/vault/mod.rs`
  - 影响：V1 无法直接复用锁屏 gate
- **F31**：`AttachmentStore::list_all` 与存储统计只扫根目录，不递归
  - 位置：`assistant-storage/src/attachment_store.rs`、`src-tauri/src/cmd/storage.rs`
  - 影响：rev7 不产生子目录，统计口径无需为截图做例外
- **F32**：`tauri` 已启用 `protocol-asset` feature，`convertFileSrc` 可用
  - 位置：`src-tauri/Cargo.toml`
  - 影响：rev7 不需要它（预览走 data URL）；feature 保持不动
- **F33**：应用已依赖 `uuid` v4
  - 位置：`src-tauri/Cargo.toml`
  - 影响：令牌生成不需要新增随机数依赖
- **F34**：`AttachmentStore` 未暴露根目录访问器；`AppDirs::attachments()` 已存在并用于建目录
  - 位置：`assistant-storage/src/attachment_store.rs`、`src-tauri/src/app_dirs/roots.rs`、`src-tauri/src/setup.rs`
  - 影响：rev7 不需要任何子目录：截图不落盘，`AppDirs`、`dir_name`、`asset_scopes()` 均不改
- **F35**：Orb 窗口是透明、无边框、置顶、`skipTaskbar`、不可缩放；`expand/collapse_orb_panel` 会改尺寸与位置
  - 位置：`src-tauri/tauri.conf.json`、`src-tauri/src/window.rs`
  - 影响：可被临时放大成全屏覆盖层；恢复时要写回原物理几何
- **F36**：`spawn_screen_inspection` 每 800ms 把 Orb 吸附到最近屏幕并写回 `OrbConf.position_x/y`
  - 位置：`src-tauri/src/setup.rs`
  - 影响：截图期间必须暂停，否则会把全屏几何持久化
- **F37**：`window::on_window_event` 在 Orb `Focused(false)` 时发 `ORB_BLUR`；`OrbPanel` 200ms 后折叠
  - 位置：`src-tauri/src/window.rs`、`src/orb/OrbPanel.tsx`
  - 影响：截图期间必须跳过，否则全屏窗口会被折叠
- **F38**：输入草稿已提升为 `ConversationView` 的受控 state（按 `conversationId` 持有），`MessageInput` 是受控组件
  - 位置：`src/chat/ConversationView.tsx`、`src/chat/message/MessageInput.tsx`
  - 影响：chat 只能隐藏不能卸载，否则草稿丢失；依据是「受控 state 在 `ConversationView`」而不是组件本地 state
- **F39**：`OrbConf.click_through` / `always_on_top` 会改 Orb 的点击穿透与置顶
  - 位置：`src-tauri/src/cmd/config.rs`
  - 影响：截图期间需要临时关闭穿透并强制置顶，结束后恢复
- **F40**：`expand_orb_panel` / `collapse_orb_panel` 基于当前 position/size 计算
  - 位置：`src-tauri/src/window.rs`
  - 影响：恢复必须写回精确的原物理几何，store 的 `isExpanded` 才能继续成立
- **F41**：`ConversationView` 的附件按钮**只在 `stream.isStreaming` 时禁用**，不检查 `supportVision`；vision 拦截在 `admitAttachment`（`unsupported` 才拦，`unknown` 放行）
  - 位置：`src/chat/ConversationView.tsx`、`src/chat/attachment/admit.ts`
  - 影响：截图按钮沿用「流式禁用」即可；无 vision 时由准入层返回 `no_vision`，按钮不禁用
- **F42**：`ShortcutSettings` 使用 `appConf.keybindings ?? DEFAULT_KEYBINDINGS`；`KeybindingField` 只检测内部重复
  - 位置：`src/settings/pages/ShortcutSettings.tsx`、`src/settings/shared/renderers/KeybindingField.tsx`
  - 影响：默认值漂移会让设置页显示与实际注册不一致；外部冲突无法可靠静态枚举，以注册失败为准
- **F43**：Cap 已加入部分快照
  - 位置：本机参考检出 `references/`（gitignored，不入仓）
  - 影响：作为原生覆盖层、捕获排除、权限校验的只读参考
- **F44**：Kiri（MIT）与 shiping（CC BY-NC 4.0）已完整克隆
  - 位置：本机参考检出 `references/`（gitignored，不入仓）
  - 影响：Kiri 可复制 Rust 抓屏实现；shiping 仅读流程与几何换算，禁止复制代码
- **F45**：`ORB_CLICK_THROUGH` 是进程内 `AtomicBool` 镜像；`apply_orb_click_through` 读到相同值即**直接早退**，不再调窗口 API
  - 位置：`src-tauri/src/cmd/orb.rs`
  - 影响：覆盖层不能只调 `set_ignore_cursor_events`：必须同步该 atomic，否则恢复后用户的「点击穿透」设置永久失效
- **F46**：`apply_orb_window_attrs` 是第三个写 Orb 属性的入口（启动与设置变更时把 `always_on_top` 写回窗口）
  - 位置：`src-tauri/src/window.rs`
  - 影响：截图期间必须跳过，否则改设置会把全屏态打回 conf 值
- **F47**：`suppressOrbBlurCollapse(ms)` 已提供时间窗抑制，`isOrbBlurCollapseSuppressed()` 供 blur 处理查询
  - 位置：`src/orb/OrbExpand.ts`、`src/orb/OrbPanel.tsx`
  - 影响：复用该机制，不在 `OrbPanel` 里散落第三套判断
- **F48**：`import_attachment` 用 `resolve_external_input` 校验「绝对路径 + 常规文件 + 不在受保护系统目录」，**不拒绝应用数据目录内的路径**
  - 位置：`src-tauri/src/cmd/attachment.rs` `plan_import`、`assistant-tools/src/os/path.rs`
  - 影响：rev7 不再经路径导入；这条只服务于普通路径附件
- **F49**：`tauri` 的 `Manager::cursor_position()` 返回 `PhysicalPosition<f64>`（物理坐标），与 `get_available_screens` 同一坐标空间
  - 位置：`src-tauri/Cargo.toml`（tauri 2）
  - 影响：覆盖层定位优先用它，避免混合 DPI 下二次换算
- **F50**：CSP `img-src` 已含 `data:` 与 `blob:`；应用当前**未注册任何自定义 URI scheme**
  - 位置：`src-tauri/tauri.conf.json` `app.security.csp`、`src-tauri/src/setup.rs`
  - 影响：底图可以 data URL 直接渲染，既不需要自定义 scheme，也不依赖 asset scope
- **F51**：`import_attachment` 只做 `plan_import`（扩展名 → `media_type`、常规文件、大小上限）与 `save_attachment`（sha256 内容寻址），**不做归一化、不读 EXIF**
  - 位置：`src-tauri/src/cmd/attachment.rs` `import_attachment`、`assistant-storage/src/attachment_store.rs` `save_attachment`
  - 影响：把这段抽成 `archive_bytes(bytes, file_name, media_type)` 给截图复用，不会改变任何准入/存储语义
- **F52**：图片归一化（长边 2048、PNG/JPEG 重编码）发生在**发送时**的 `admit_attachments`，不在导入时
  - 位置：`assistant-agent/src/multimodal.rs`、`assistant-agent/src/normalization.rs`
  - 影响：截图在归档前不需要再走一遍「校验不可信输入」式的解码；自己产出的字节只做一次编码
- **F53**：Tao 的 `set_visible(false)` 只比对窗口标志位；若其他路径（原生 `ShowWindow`）绕过它改过可见性，缓存值仍是 false，隐藏会变成空操作，而 `IsWindowVisible` 仍报可见
  - 位置：Tauri/Tao 2 窗口实现；实证见 Kiri ADR 0107（§14.3）
  - 影响：隐藏 Orb 后必须「可见性查询确认 → 对残留 HWND 补 `ShowWindow(SW_HIDE)` → `DwmFlush`」，不能只 sleep
- **F54**：Orb 窗口是 `transparent: true` 的分层窗口；`set_ignore_cursor_events` 在 Tao 里会追加 `WS_EX_LAYERED | WS_EX_TRANSPARENT`
  - 位置：`src-tauri/tauri.conf.json` `orb` 窗口、`src-tauri/src/cmd/orb.rs` `apply_orb_click_through`
  - 影响：Windows 对这类未初始化分层的窗口调 `SetWindowDisplayAffinity` 会失败（error 8，§14.3），所以 V2 的捕获排除路线挂在「复用 Orb」上大概率走不通
- **F55**：Windows 弹出菜单是**独立顶层窗口**，窗口类名 `#32768`，Orb 只是它的 owner 而非父窗口
  - 位置：Muda 0.20.0 `show_context_menu_for_hwnd`（`TrackPopupMenu`）；Tauri 2.12.5 `menu.popup_at` 走同一路径
  - 影响：`ShowWindow(orb)` / `set_visible(false)` 带不走菜单；从右键菜单项触发截图时，菜单可能仍在关闭动画，必须单独处理（不变式 23）
- **F56**：菜单的关闭淡出由 DWM 在合成器侧播完，`TrackPopupMenu` 返回时菜单窗口可能已被销毁——`EnumWindows` 枚举不到窗口，不等于屏幕上没有残影
  - 位置：`src-tauri/src/window.rs` `show_orb_context_menu`、`src-tauri/src/menu.rs` / `src-tauri/src/tray.rs` 菜单事件回调、`src-tauri/src/screenshot/orb_overlay.rs` `dismiss_menus_now` / `settle_menu_fade`
  - 影响：rev8 的「可见期打 `WDA_EXCLUDEFROMCAPTURE`」路线撤回（打标要求窗口存活，且打标时机无法与点击对齐）；改为按**时间**兜底——淡出起点锚定在菜单事件回调（点击那一刻）且**无条件写入**，抓屏前补齐剩余时长。若按「枚举到菜单窗口」才记时刻，实际枚举为空会把整段等待跳过，动画中间帧照样被冻进底图
- **F57**：`DwmFlush` 只保证「已提交的合成帧落地」，不保证「窗口关闭动画已经播完」；`BitBlt(GetDesktopWindow)` 读的是**当前合成结果**，菜单淡出的中间帧会被直接冻进底图
  - 位置：`xcap` 0.9.10 `src/windows/gdi.rs`（未开 `wgc` feature 时的默认路径）；`src-tauri/src/screenshot/orb_overlay.rs` `settle_menu_fade`
  - 影响：可见性校验只能证明「窗口已不可见」，不能证明「残影已消失」；抓屏前必须额外排空动画——从点击时刻起按帧排空剩余淡出（F60），而不是按固定毫秒等待
- **F58**：截图有三个入口，只有「聊天工具栏」在发起前把前端切进覆盖层模式；菜单 / 托盘 / 快捷键入口由 Rust 直接调 `start`，前端状态仍是「小球形态」（`session = null`、`preparing = false`，因此不加 `.orb-screenshot`）
  - 位置：`src/chat/layouts/ConversationFrame.tsx` `onScreenshot`（唯一调 `beginPreparing()` 的地方）、`src-tauri/src/menu.rs` / `src-tauri/src/tray.rs` 截图项、`src-tauri/src/setup.rs` 快捷键分派、`src/layouts/OrbWindow.tsx` `overlayActive`、`src/orb/OrbBall.tsx`（`position: absolute; top: ballTop; left: ballLeft`）
  - 影响：`present()` 把同一个窗口撑成显示器物理矩形并移到 `(0, 0)` 后，仍按小球窗口坐标系绝对定位的 `OrbBall` 会落到**新窗口原点**，即屏幕左上角——全屏窗口 + 左上角小球。必须在撑大之前让前端先进入覆盖层模式（不变式 25、§8.2 步骤 3）
- **F59**：`set_size` 与 `set_position` 是两次独立的主线程消息，两者之间会回到消息循环，DWM 可合成一帧；恢复几何时窗口仍可见，因此这一帧会被真实画出来
  - 位置：`src-tauri/src/screenshot/orb_overlay.rs` `restore_with` / `apply_geometry`（`WebviewWindow` 无 `set_bounds`，见 §8.5.1）
  - 影响：可见的全屏覆盖层缩回小球时，中间帧是「88×88 的窗口停在 `(0, 0)`」——用户看到左上角闪出小球。恢复必须先把窗口隐藏，改完几何再按快照恢复可见性（不变式 26）；`present()` 不受影响，因为它执行时窗口本就隐藏
- **F60**：菜单关闭淡出是**按合成帧推进**的线性过渡（Win11 默认动画实测 19 帧，与菜单项数量无关），不是按墙钟毫秒推进；同一段动画在 60Hz 上是约 317ms、在 144Hz 上只有约 132ms
  - 位置：`src-tauri/src/screenshot/orb_overlay.rs` `settle_menu_fade` / `drain_menu_fade_frames` / `frame_period` / `menu_animation_enabled`；`src-tauri/crates/assistant-config/src/constant.rs` `SCREENSHOT_MENU_FADE_FRAMES` / `SCREENSHOT_MENU_FADE_CAP_MS`
  - 影响：用固定毫秒等待必然两头不对——高刷屏白等（用户感知为「点了没反应」），低刷屏漏帧（残影冻进底图）。必须按帧排空：`DwmFlush` 即使空闲也会等到下一个合成帧，连打 N 次恰好消费 N 帧动画，与刷新率解耦；已过去的时间按 `DwmGetCompositionTimingInfo` 回报的刷新周期折算成已消费帧数，避免重复等待。系统关掉「菜单动画」时没有淡出，直接跳过（`SPI_GETMENUANIMATION`）

---

## 5 不变式

违反任一条即视为实现错误。

- **1**：图片字节只在「Rust 内存 → `AttachmentStore`」之间流动：消息里只有 `attachmentId`；`crop_screenshot` 返回的是**已归档结果**（`attachmentId` + 尺寸 + `mediaType`），不接受、也不返回任何磁盘路径（依据：F10 / F13 / F14 / F51）
- **2**：`crop/cancel` 只接受本次抓屏签发的一次性令牌，且只允许 `orb` webview 调用；不接受任意磁盘路径（依据：防越权读写）
- **3**：前端只发送归一化选区（相对底图显示区域，0..=1）；Rust 用原图尺寸换算，且只换算一次（依据：跨 DPI 稳定）
- **4**：前端不得自行裁剪图片（不允许把 base64 回传后端）（依据：不变式 1）
- **5**：**不新增 webview / 窗口 label**；覆盖层复用 `orb`（依据：重量约束）
- **6**：截图链路不产生临时文件：取消、失败、关闭只需释放内存中的原图与预览并清会话；产物在裁剪命令内即完成归档，归档失败即丢弃（依据：隐私 / F51）
- **7**：截图不自动发送；只把附件放入输入区（依据：误触防护）
- **8**：不新增附件上限、不改 `admit_attachments` 的准入判断；裁剪产物必须 ≤ 8MB（`import_attachment` 会二次校验）（依据：范围约束）
- **9**：新增 DTO 一律 `#[derive(TS)]` 并登记到 `protocol/dto/mod.rs`；`protocol_bindings.rs` 无数字断言，登记后重跑 `contract:gen` 即可（依据：F28 / 项目规则）
- **10**：默认快捷键只改 Rust `DEFAULT_SHORTCUT_PATTERNS`，再跑 `contract:gen` 生成前端默认值；禁止手写前端默认值（依据：F7 / F8）
- **11**：同一时刻只有一个活动会话；所有状态变更命令串行执行；忙时再次触发直接忽略（依据：并发约束）
- **12**：归档时声明的 `media_type` 必须与实际编码一致（PNG → `image/png`，JPEG → `image/jpeg`），`file_name` 的扩展名与之匹配（依据：F11 / F51）
- **13**：覆盖层只能通过 `screenshot-session` 事件或 `get_screenshot_session` 获取会话，不得依赖 URL/DOM 注入（依据：会话引导）
- **14**：chat/Orb 面板只隐藏（CSS），不卸载；`ConversationView` 的草稿、附件、流式状态必须保留（依据：F38）
- **15**：截图期间暂停 `spawn_screen_inspection`、`snap_orb_to_*`、`save_orb_position_sync`，并跳过 `apply_orb_window_attrs`；恢复后写回原几何（依据：F35 / F36 / F40 / F46）
- **16**：截图期间 Rust 跳过 Orb 的 `ORB_BLUR`；前端复用 `isOrbBlurCollapseSuppressed()` 机制再加会话判断；Orb 不得折叠（依据：F37 / F47）
- **17**：抓屏前只隐藏覆盖层宿主 `orb`；`main` / `settings` 保持用户原状并进入底图；成功后显示并聚焦 Orb，取消时按快照恢复 Orb（依据：目标 5 / §6.3）
- **18**：抓屏预览只以内存 data URL 下发，不落盘、不依赖 asset scope 或自定义 URI scheme；CSP 必须保持允许 `data:`（依据：F19 / F50）
- **19**：任何 `set_ignore_cursor_events` 必须与 `ORB_CLICK_THROUGH` atomic 同步；禁止只调窗口 API（依据：F45）
- **20**：覆盖层几何用回退序列「先 `set_size` 后 `set_position`」写入与恢复（`WebviewWindow` 无 `set_bounds`，见 §8.5.1）；恢复值来自快照，不用 `inner_size()` 重算（依据：F35 / F40）
- **21**：Rust 恢复 Orb 几何之前，前端必须已卸载覆盖层；顺序固定为「收到裁剪结果 → 卸载覆盖层并进入处理中态 → 落地附件 → 展开并聚焦」（依据：§8.4 步骤 7 / §9.4）
- **22**：隐藏 Orb 后必须做「可见性查询确认 → 对残留 HWND 补 `ShowWindow(SW_HIDE)` → `DwmFlush`」；`SCREENSHOT_HIDE_SETTLE_MS` 只能作为兜底上限（依据：F53）
- **23**：Windows 上隐藏 Orb 前必须先清掉本进程残留的弹出菜单窗口（类名 `#32768`，Orb 右键菜单与托盘菜单都是它）；菜单是**独立顶层窗口**，不是 Orb 的子窗口，隐藏 Orb 影响不到它，不清理就会被冻进底图（依据：§8.2、F55）
- **24**：菜单淡出的起点必须锚定在**菜单事件回调（点击那一刻）**且无条件写入，不依赖「是否枚举到菜单窗口」；抓屏前**按帧**排空剩余淡出帧（`SCREENSHOT_MENU_FADE_FRAMES`，兜底上限 `SCREENSHOT_MENU_FADE_CAP_MS`），不得用固定毫秒；「窗口已不可见 / 已销毁」都不等于「残影已消失」（依据：F56、F57、F60）
- **25**：`start` 必须在撑大全屏窗口**之前**先 `emit_orb_only(SCREENSHOT_PREPARING)`，让前端把 UI 切进覆盖层模式（加 `.orb-screenshot`、隐藏 `OrbBall`/`OrbPanel`）；不得只依赖紧邻 `present` 的 `screenshot-session` 事件——那是异步 IPC，Rust 不等前端渲染（依据：F58、§8.2 步骤 3）
- **26**：恢复 Orb 几何必须在窗口**不可见**状态下完成：`set_visible(false)` → `set_always_on_top` / 点击穿透 → `set_size` → `set_position` → 按快照恢复可见性。禁止在可见的全屏窗口上直接改几何（依据：F59、§8.5.1）

---

## 6 选型

### 6.1 抓屏

- **`xcap` crate**（推荐）：平台 Windows / macOS / Linux；新增 1 个 crate；枚举显示器 + 抓 RGBA，多屏与 DPI 由库处理，代码量最小
- **自研 GDI `BitBlt`**：平台仅 Windows；新增 0（`windows` crate 已有 GDI）；混合 DPI、DWM 遮挡、HDR 全要自己处理
- **WGC（`Windows.Graphics.Capture`）**：平台仅 Windows；需新增 Direct3D11 等 feature；面向持续流，静态截屏过度设计
- **macOS ScreenCaptureKit**：平台仅 macOS；需 objc2 桥接；重，`xcap` 内部已封装
- **Web `getDisplayMedia`**：**不可用**——WebView2/WKWebView 拿不到系统级屏幕流

**决定**：`xcap`。它只负责「拿到一张整屏 RGBA」，框选、裁剪、压缩、窗口管理全部由本仓库承担。

实现约束：

- 锁定 **`xcap` 0.9.x**：Kiri 0.9.8 的 Windows 冻结静帧就走它的 GDI 路径，是目前最靠得住的现网验证；落地时若 0.9 线上有更新的兼容版本，以实测为准并在修订里记一笔。
- 后端抽象为 `CaptureBackend`（`monitors()` / `capture()`，`Send + Sync + 'static`）；默认 `XcapBackend`，CI 用 `FakeBackend`。
- 抓屏、预览 JPEG 编码、裁剪与最终编码全部 `spawn_blocking`。

平台风险：

- Windows：无需额外权限；Win7/8 回退 GDI（V2）。
- macOS：需「屏幕录制」TCC 授权；未授权返回黑图 → 预检 + 实际可用性校验（§8.7）。
- Linux：X11 可用；Wayland 受合成器限制，不承诺支持。

### 6.2 覆盖层：复用 Orb，而不是新开 webview

- **复用 Orb webview（rev3）**：新增 webview 否；主要成本 Orb 几何/焦点/巡检/失焦状态管理；交互质量静态冻结底图、稳定；结论 **V1**
- **新开 `screenshot` webview（rev2）**：新增 webview 是；主要成本 额外 WebView2/WKWebView、viewMap、capabilities、跨窗口事件；交互质量最隔离、风险最低；结论 否决（重）
- **原生覆盖层（Cap/GPUI 路线）**：新增 webview 否；主要成本 自绘、手柄、吸附、字体、输入循环；交互质量最流畅、内存最低；结论 V2 评估（代码量大）
- **系统截图工具**：新增 webview 否；主要成本 平台差异、剪贴板依赖、无法统一品牌体验；交互质量依赖系统 UX；结论 V1 备选，不作为默认
- **实时覆盖层 + 捕获排除**：新增 webview 否；主要成本 需要 `WDA_EXCLUDEFROMCAPTURE` / `NSWindowSharingNone`，Linux 需回退；交互质量不冻结屏幕、捕获在确认时发生；结论 V2 优化项

复用 Orb 的可行性：

- Orb 窗口已经是透明、无边框、置顶、`skipTaskbar`、不可缩放；可以临时改成显示器物理矩形（先 `set_size` 后 `set_position`，见 §8.5.1；F35）。
- 覆盖层是**静态截图**：先隐藏 `orb` → 抓屏 → 把 Orb 放大并显示截图；宿主窗口自身不会入镜，也不需要透明穿透。`main` / `settings` 不隐藏，因此底图包含它们的内容。
- Orb 的 JS 上下文与 `ConversationView` 共享；`crop_screenshot` 在命令内归档后返回 `attachmentId`，前端直接 `addAttachment` 进 `attachmentStore`（不变式 14 的附加收益）。
- 主要风险是 Orb 的状态机（巡检、失焦、点击穿透、位置持久化），在 §8.6 逐条加守卫。

### 6.3 静态冻结 vs 实时覆盖层

V1 选择**静态冻结**：

- 抓屏时只隐藏覆盖层宿主 `orb`，抓到一张稳定的整屏图；后续框选不会因为桌面内容变化而错位。
- `main` / `settings` 保持用户触发瞬间的样子进入底图，「冻结底图 = 用户所见」；V1 不提供「隐藏本应用窗口」的开关，不想让其入镜时由用户自行挪开或最小化。
- 代价是原图要常驻内存：一个 4K 会话约 33MB，裁剪/取消即释放；预览另有一份按逻辑分辨率的 JPEG（约 0.3–1MB），随会话一起释放。
- 静态图让「选区 = 最终产物」的验收更确定。
- 预览不落盘：以 data URL 下发（F50），因此不涉及 asset scope、临时目录与清扫。

V2 可评估实时覆盖层 + 捕获排除（Cap 的 `overlay_backdrop.rs`）：不冻结屏幕、确认时才抓屏，但依赖平台 API，且用户看到的是变化中的桌面。

**rev7 补一条硬限制**：捕获排除在「复用 Orb」这条路上大概率不可用——Orb 是 `transparent` 的分层窗口，`set_ignore_cursor_events` 又会追加 `WS_EX_LAYERED | WS_EX_TRANSPARENT`（F54），而 Windows 对未初始化分层的窗口调 `SetWindowDisplayAffinity` 会返回 error 8（Kiri 实测，§14.3）。所以 V2 要上捕获排除，前提是先换覆盖层宿主窗口（§6.4 的 spike 要顺带验证这一项）。

### 6.4 落地前置 spike（窗口模型）

「复用 Orb」与「运行时专用覆盖层窗口」不在本规格内定案。先用最小 spike 取实测数据，再决定是否改写 §6.2 / §8.5 / §8.6：

- **spike 目标**：验证运行时创建一个专用覆盖层窗口能否在既有前端里跑通，且不引入新的稳定性问题。
- **必须测的项**：
  1. 运行时窗口（`WebviewWindowBuilder`）首屏到可交互的耗时；第二次触发是复用还是重建，各多少毫秒。
  2. 从主线程回调里创建窗口是否触发 WebView2 嵌套初始化卡死（Kiri 的 AppHangB1 事故，§14.3）；正确做法是把创建投递到事件循环边界或专用线程。
  3. 副屏（不同 DPI）首帧是否铺满、有无黑边或沿用旧尺寸——**这一项两条路线都要测**。专用窗口路线用 `WebviewWindowBuilder` 的初始几何；Orb 复用路线只能走「先 `set_size` 后 `set_position`」（`WebviewWindow` 无 `set_bounds`，rev8 已定稿，见 §8.5.1），重点观察是否出现一帧错位。
  4. 在该窗口上调 `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` 是否返回 0，用于判断 V2 捕获排除是否可行。
  5. `viewMap` 加一个 label、`capabilities/default.json` 的 `windows` 加一个 label 的真实改动量。
- **判据**：若（1）可接受且（2）可规避、（4）可用，则改用专用窗口并删掉 §8.6 的 Orb 守卫；否则保留 Orb 复用，把（3）（4）的结论回写 §6.3。
- **成本边界**：spike 代码不进主干；结论以「一条修订 + §6.2 表格更新」回收。

### 6.5 预览为什么仍然编码（rev10 新增）

一个被认真评估并**否决**的提案：抓屏后直接把全分辨率原始像素（BMP / RGBA）留在内存，不做任何编码，等用户框选后再编码。它听起来能省掉预览编码，但实际是亏的。

**前半段已经是现状**：抓到的 `Arc<RgbaImage>` 全程留在内存（2520×1680 约 16.1MB），不落盘；`crop` 直接对这张原图 `crop_imm` 再编码，所以**最终产物的画质只取决于原图，与预览编码质量无关**。预览编码只是一次性的「给 WebView 显示用的临时底图」。

**后半段（不编码直接下发）的代价**（以 2520×1680 为例，原始 RGBA 实测 16.1MB）：

| 方案 | 下发内容 | 走 IPC 的负载 |
|---|---|---|
| 现方案：预览 JPEG q80 | 0.3–1MB（§13 风险 14 口径） | base64 再 +33% |
| 不编码：24bit BMP | 12.1MB | base64 约 16.1MB |
| 不编码：RGBA | 16.1MB | base64 约 21.5MB |

- 省下：一次预览 JPEG 编码（几十毫秒量级，见 §13 风险 19）。
- 新增：base64 编码、十几 MB 字符串在 JSON 里的序列化 / 跨 IPC 拷贝 / JS 侧解析，以及前端还要再走一遍 BMP 解码或 `createImageBitmap`。
- 结论：**净亏**。被省掉的那段编码在整条链路里也不是主要部分。

**真要做「零编码」的正确形态**是自定义 URI scheme 直接把内存帧喂给 `<img src>`（Kiri 做法），省掉 base64 这一层；但它是另一条独立优化，与「编码 / 不编码」无关——底图仍必须是浏览器可解码的格式。该方向保留在 §13 风险 14 的备选里，不进 V1。

**这条结论的意义**：后续如果又有人提「全内存 BMP 不做编码」，直接指回本节，不要重新做一遍。

---

## 7 契约

### 7.1 常量

`src-tauri/crates/assistant-config/src/constant.rs`：

```rust
/// 隐藏覆盖层宿主 `orb` 后的兜底等待上限（毫秒）；正常路径靠可见性校验 + DwmFlush 提前返回
pub const SCREENSHOT_HIDE_SETTLE_MS: u64 = 150;
/// 会话事件发出后、显示覆盖层前的兜底等待上限（毫秒）；前端以 `img.decode()` 完成为准
pub const SCREENSHOT_OVERLAY_SETTLE_MS: u64 = 100;
/// 弹出菜单关闭淡出的帧数（Win11 默认动画实测 19 帧线性过渡，取 19 + 1 帧余量）；
/// 按帧而非毫秒计：60Hz 约 317ms、144Hz 约 132ms，固定毫秒在高刷屏白等、低刷屏漏帧
pub const SCREENSHOT_MENU_FADE_FRAMES: u32 = 20;
/// 排空淡出帧的兜底上限（毫秒）：远程桌面、窗口被遮挡时 `DwmFlush` 可能提前返回，防止退化成忙等
pub const SCREENSHOT_MENU_FADE_CAP_MS: u64 = 800;
/// 无覆盖层宿主时的孤儿会话存活时间
pub const SCREENSHOT_SESSION_TTL_MS: u64 = 10 * 60 * 1000;
/// 与 assistant-agent / 前端一致的图片单张上限
pub const SCREENSHOT_MAX_BYTES: u64 = 8 * 1024 * 1024;
/// 底图预览的长边上限（CSS px）；只影响底图清晰度，不影响裁剪产物
pub const SCREENSHOT_PREVIEW_LONG_EDGE: u32 = 2560;
/// 底图预览的 JPEG 质量
pub const SCREENSHOT_PREVIEW_JPEG_QUALITY: u8 = 80;
```

前两个 settle 值都降级为「兜底上限」，不再是正常路径的固定等待：隐藏走「可见性校验 + `DwmFlush`」（不变式 22），揭示走 `img.decode()`（§9.4）。调大它们只影响异常路径的最坏耗时。

菜单淡出是唯一按**帧**而非时间计的等待（F60、不变式 24）：`DwmFlush` 连打 N 次恰好消费 N 个合成帧，与刷新率解耦；锚点之后已过去的时间按 `DwmGetCompositionTimingInfo` 回报的刷新周期折算成已消费帧数，不重复等待。`SCREENSHOT_MENU_FADE_CAP_MS` 只防「`DwmFlush` 提前返回导致循环退化成忙等」，不是正常路径的耗时。

最小选区阈值是**纯前端 CSS 像素概念**，不放这里（Rust 只保证换算后的物理宽高 ≥ 1）：放 `src/config/constants.ts`，与 `ORB_*` 一组：

```ts
export const SCREENSHOT_MIN_SELECTION_CSS_PX = 8;
```

> **rev7：`src-tauri/src/app_dirs/**` 与 `tauri.conf.json` 均不改**。截图不落盘，因此不新增 `SCREENSHOT_TMP` 目录名、不新增 `AppDirs::screenshot_tmp()`、不改 `asset_scopes()`；`tests/app_dirs_contract.rs` 与 `roots.rs` 的内部断言也都不需要动。

### 7.2 DTO（新建 `src-tauri/src/protocol/dto/screenshot.rs`）

共 **3 个** DTO；在 `protocol/dto/mod.rs` 登记后重跑 `npm run contract:gen`。`protocol_bindings.rs` 只比对派生名集合，**没有数字断言需要改**（F28）。

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScreenshotSessionDto {
    /// 一次性令牌：裁剪/取消只认它
    pub token: String,
    /// 底图预览：`data:image/jpeg;base64,...`，内存生成，按逻辑分辨率压缩
    pub preview: String,
    /// 预览的像素尺寸（仅用于尺寸标注与诊断，不作为换算依据）
    pub preview_width: u32,
    pub preview_height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScreenshotCropArgsDto {
    pub token: String,
    /// 归一化选区，相对底图显示区域（0..=1）
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScreenshotCropResultDto {
    /// 已归档附件的 id；前端据此构造 `Attachment`，不需要再走一次导入
    pub attachment_id: String,
    pub file_name: String,
    pub media_type: String,
    pub bytes: u64,
    pub width: u32,
    pub height: u32,
}
```

字段说明：

- `ScreenshotSessionDto` 是覆盖层的引导数据；目标显示器、原图物理尺寸、原图本身留在 Rust 的 `PendingSession` 里，不下发前端。
- `preview` 是**底图专用**的低分辨率 JPEG data URL：只决定框选的视觉参照，不参与产物。前端不得把它当作附件来源（不变式 4）。
- 裁剪参数用归一化坐标，前端不需要知道 `scale_factor`，Rust 内部按原图宽高换算；跨 DPI 与窗口尺寸误差都被吸收。
- `ScreenshotCropResultDto` 是**终态**：命令返回时字节已经进 `AttachmentStore`，`attachment_id` 可以立刻进消息。没有「待消费路径」，因此不存在「忘了归档」或「归档后又删了」的中间态。
- `media_type` 与 `file_name` 扩展名必须一致（不变式 12）：兜底转 JPEG 时两者同时变为 `image/jpeg` / `.jpg`。
- 三个 DTO 都只承载 wire 数据，不带本地 UI 状态（如 `preparing` 留在前端 store）。
- 权限状态不引入 DTO：命令直接返回 `bool`。

### 7.3 命令（新建 `src-tauri/src/cmd/screenshot.rs`）

共 **5 条**：

```rust
#[command]
pub async fn start_screenshot(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, ScreenshotState>,
) -> Result<(), String>;

#[command]
pub fn get_screenshot_session(
    window: tauri::WebviewWindow,
    state: State<'_, ScreenshotState>,
) -> Result<ScreenshotSessionDto, String>;

/// 裁剪 + 编码 + 归档一次完成；返回的是已入库附件，不是路径
#[command]
pub async fn crop_screenshot(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, ScreenshotState>,
    args: ScreenshotCropArgsDto,
) -> Result<ScreenshotCropResultDto, String>;

#[command]
pub async fn cancel_screenshot(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, ScreenshotState>,
    token: String,
) -> Result<(), String>;

/// Windows / Linux 恒 true；macOS 走屏幕录制预检 + 实际显示器可用性校验
#[command]
pub fn screenshot_permission_granted(app: AppHandle) -> bool;
```

调用方约束：

- **`start_screenshot`**（允许的 webview label：`orb`）：快捷键/菜单/托盘路径由 Rust 内部直接调 `screenshot::start()`，不经过 command
- **`get_screenshot_session` / `crop_screenshot` / `cancel_screenshot`**（允许的 webview label：`orb`）：令牌之外的第二道边界
- **`screenshot_permission_granted`**（允许的 webview label：任意应用窗口）：只读
- **rev7：不再有任何命令接受或返回磁盘路径**（`release_screenshot` 已删除）

行为约定：

- `start_screenshot` 忙时（`Phase != Idle` 且覆盖层活跃）直接返回 `Ok(())`；若 `Phase != Idle` 但没有覆盖层宿主（异常残留），先清理再启动。
- `start_screenshot` 成功后返回 `()`；会话通过 `screenshot-session` 事件下发。`get_screenshot_session` 是 reload/错过事件时的兜底。
- `crop_screenshot` 在**命令内**完成「裁剪 → resize → 编码 → 归档」，返回已入库的 `attachmentId`，随后恢复 Orb 几何。前端拿到结果的第一件事是卸载覆盖层（不变式 21）。
- 会话在裁剪成功时即被消费，同一令牌不能再 crop 第二次；归档或编码失败时令牌仍有效，用户可以重选（§8.4 步骤 8）。
- `cancel_screenshot` 幂等；令牌不匹配不清理其他会话。
- 没有任何「用户放弃时删临时文件」的分支：失败重选与取消都只需要释放内存。

错误码（命令返回稳定字符串，前端用 `translateKey` 映射；未知错误原样显示）：

- `screenshot.err.permission_denied`：macOS 未授权或预检通过但无法枚举显示器
- `screenshot.err.no_monitor`：找不到可用显示器
- `screenshot.err.capture_failed`：抓屏、预览编码或窗口隐藏失败
- `screenshot.err.no_session`：覆盖层拉取会话时无活动会话
- `screenshot.err.token_invalid`：令牌缺失/不匹配
- `screenshot.err.too_small`：归一化选区换算后宽/高为 0
- `screenshot.err.too_large`：JPEG 兜底后仍超 8MB
- `screenshot.err.encode_failed`：编码失败
- `screenshot.err.archive_failed`：`AttachmentStore` 写入失败（含超限二次校验不过）

分层：`cmd/screenshot.rs` 只是薄壳，真正逻辑在 `screenshot::mod.rs` 的 `start()` / `cancel()` / `crop()` / `cleanup()`；命令负责「State 取出 + 调用方 label 校验 + 错误转字符串」。归档不复用 `import_attachment` 命令（避免 IPC 绕一圈），而是调用 `cmd::attachment::archive_bytes()`，与 `import_attachment` 共享同一段「大小上限 + `save_attachment`」。

注册：`src-tauri/src/ipc.rs` 的 `generate_handler!` 追加五条；`src-tauri/src/cmd/mod.rs` 加 `mod screenshot;` 与 `pub use screenshot::*;`；`src-tauri/src/lib.rs` 加 `mod screenshot;`；`src-tauri/src/setup.rs` 在 `handle.manage(AttachmentStore)` 之后追加 `handle.manage(ScreenshotState::new())`（不建任何目录）。

### 7.4 事件

`src-tauri/src/protocol/events.rs` 新增：

```rust
pub const SCREENSHOT_SESSION: &str = "screenshot-session";
pub const SCREENSHOT_PREPARING: &str = "screenshot-preparing";
pub const SCREENSHOT_ERROR: &str = "screenshot-error";
```

- `screenshot-preparing` 用 `emit_orb_only` 发送空载荷；在权限检查通过后、抓屏之前发出，只做一件事：让前端 `beginPreparing()`（`§8.2` 步骤 3、不变式 25）。菜单 / 托盘 / 快捷键入口没有前端那一跳，缺了它窗口撑大时会渲染出小球（F58）。
- `screenshot-session` 用 `emit_orb_only` 发送 `ScreenshotSessionDto`（含底图 data URL，约 0.3–1MB）；在抓屏与预览编码完成、覆盖层显示之前发出。
- `screenshot-error` 用 `emit_orb_only` 发送错误码字符串；只服务「快捷键 / 右键菜单 / 托盘」这类没有 invoke 返回值的入口；工具栏入口直接用 `start_screenshot` 的 `Err`。
- **不新增** `screenshot-captured` / `screenshot-cancelled`：裁剪结果由 orb 直接拿 invoke 返回值；取消由 Rust 恢复，不需要事件。
- 前端 `src/types/events.ts` 增加 `UiEvent.ScreenshotPreparing = "screenshot-preparing"`、`UiEvent.ScreenshotSession = "screenshot-session"` 与 `UiEvent.ScreenshotError = "screenshot-error"`。

### 7.5 快捷键注册与默认值

#### 7.5.1 注册流程（`src-tauri/src/setup.rs`）

rev3 之前是「定长数组 + `match id`」，新增一条要同时改数组长度、数组项、`match` 分支，且无法表达「用户主动清空」与「重复绑定」。**该重构已落地**（F4/F8），现状即下面的表驱动形式，本规格只需追加一项：

```rust
type ShortcutAction = fn(&tauri::AppHandle);

static SHORTCUT_SPECS: &[(&str, ShortcutAction)] = &[
    ("toggle_orb", toggle_orb_action),
    ("toggle_main", toggle_main_action),
    ("new_chat", new_chat_action),
];
```

解析与注册拆成「纯策略 + 副作用」两层：策略落在 `assistant-config::shortcut`（不依赖任何 UI 框架，可单测），快捷键文本的解析由调用方以 `parse` 回调注入：

```rust
// assistant-config::shortcut
pub fn resolve_bindings<A: Copy, S>(
    specs: &[(&str, A)],
    configured: &HashMap<String, String>,
    defaults: &HashMap<String, String>,
    parse: impl Fn(&str) -> Option<S>,
) -> (Vec<ResolvedBinding<A, S>>, Vec<BindingIssue>);
```

规则：

- 配置缺失 → 使用 `default_keybindings()`；
- 配置显式空字符串 → 视为用户禁用，不回落默认值；
- 解析失败、重复绑定 → 生成 `BindingIssue`，不注册该条，并通过 `SHORTCUT_WARNING` 事件告知前端；
- 先解析、后 `unregister_all`，避免一个坏配置把其他快捷键全部清掉；
- 新增快捷键要改三处：`SHORTCUT_SPECS` 加一项（含回调）、Rust `DEFAULT_SHORTCUT_PATTERNS` 加一行、前端 `SHORTCUT_DEFINITIONS` 加一条 `id/group/label/kind`；再跑 `npm run contract:gen`。**只有默认值来自生成文件，定义条目本身仍是手写表**。

已落地（实现时只需追加，不要重做）：`assistant-config/src/app_conf.rs` 已有 `DEFAULT_SHORTCUT_PATTERNS`、`default_keybindings()` 与 `primary_modifier()`；`setup.rs` 已有 `SHORTCUT_SPECS` 与 `resolve_bindings`；前端已有 `shortcutDefaults.generated.ts`。

#### 7.5.2 默认组合

- **`toggle_orb`**：macOS `Meta+Shift+Alt+O`，Windows / Linux `Control+Shift+Alt+O`；原 `Meta+Shift+O` / `Ctrl+Shift+O` 与编辑器「跳转符号」冲突
- **`toggle_main`**：macOS `Meta+Shift+Alt+M`，Windows / Linux `Control+Shift+Alt+M`；降低与 IDE 重构类快捷键的冲突
- **`new_chat`**：macOS `Meta+Shift+Alt+N`，Windows / Linux `Control+Shift+Alt+N`；**替换原 `Ctrl+Shift+N`**——浏览器无痕/新建窗口、资源管理器新建窗口的高频冲突
- **`capture_screenshot`**：macOS `Meta+Shift+Alt+S`，Windows / Linux `Control+Shift+Alt+S`；本规格实现时加入，避开 `Ctrl+Shift+A`（浏览器标签搜索）与 IM 截图

理由：

- 全局热键会抢占其他应用的本地快捷键，三键组合几乎没有完全无冲突的选项；四键组合显著降低冲突概率。
- 用户仍可在设置页改键；注册失败会通过 `SHORTCUT_WARNING` 提示（§9.7）。
- 已有配置文件不会被自动改写：旧默认仍按原值注册；设置页「恢复默认」会写入新组合。实现截图快捷键时，需把 `capture_screenshot` 加入 `DEFAULT_SHORTCUT_PATTERNS` 与 `SHORTCUT_SPECS`，再跑 `contract:gen`。

#### 7.5.3 前端默认值

默认值的唯一来源是 Rust `DEFAULT_SHORTCUT_PATTERNS`；`npm run contract:gen` 生成 `src/types/shortcutDefaults.generated.ts`，`shortcutTypes.ts` 里 `SHORTCUT_DEFINITIONS` 的 `defaultBinding` 用 `expandPrimary(DEFAULT_SHORTCUT_PATTERNS.xxx)` 展开。**禁止手写前端默认值**（`id/group/label/kind` 仍是手写表）。**不新增静态外部冲突表**：其他应用的快捷键无法可靠枚举，真正的冲突由操作系统注册失败暴露（`SHORTCUT_WARNING`）。

### 7.6 asset scope 与 capabilities（rev7：都不改）

- `src-tauri/capabilities/default.json`：**不改**。覆盖层运行在 `orb`，不新增窗口 label；自定义命令不受 plugin ACL 约束，事件监听走 `core:event:default`。
- `src-tauri/tauri.conf.json`：**不改**。底图是 data URL（CSP `img-src` 已含 `data:`，F50），产物直接进 `AttachmentStore` 并由现有 `attachments/*` scope 覆盖，因此 `assetProtocol.scope.allow` 不需要新增 `screenshot-tmp/*`。
- `AppDirs::asset_scopes()`、`dir_name`、`tests/app_dirs_contract.rs`、`roots.rs` 的内部断言：**都不改**。
- `opener:allow-open-path` 的 scope 仍是 `$APPDATA/attachments/*`；截图预览不通过 `openPath` 打开，不受影响（F20）。
- 前提：**CSP 的 `img-src` 必须保留 `data:`**。这是底图能显示的唯一依赖，写进验收（§11.2）。

### 7.7 依赖

`src-tauri/Cargo.toml`（本仓库约定第三方版本统一写在 `[workspace.dependencies]`，成员 crate 只写 `{ workspace = true }`）：

```toml
# [workspace.dependencies]
xcap = "0.9"

# [dependencies]（assistant-app）
xcap = { workspace = true }
image = { workspace = true }   # 截图/裁剪直接使用 RgbaImage、crop_imm、PngEncoder、JpegEncoder
```

- `image` 已在 `[workspace.dependencies]`（`0.25`，features `png/jpeg/bmp/tiff`），**只需在 app crate 的 `[dependencies]` 补一行**；当前只有 `assistant-agent` 引用它。
- `xcap` 是新依赖，两处都要加；版本落地时核实并提交 `Cargo.lock`。
- 令牌用既有 `uuid` v4（app crate 已依赖 `uuid = { workspace = true }`），不新增随机数依赖。
- `image` feature 不扩 webp。

**构建口径（rev10 新增）**：`[profile.dev]` 设 `opt-level = 1`。

```toml
[profile.dev]
opt-level = 1
debug = 1
```

这不是截图链路独享的开关，但它由截图的实测数据推动，且直接影响本规格的可感知延迟。`image` 的 JPEG 编码与 `xcap` 的 `BitBlt` + 像素格式转换在 `opt-level = 0` 下慢约 6 倍（实测见 §13 风险 19）；取 `1` 而非 `3` 是因为 `1` 已拿到几乎全部收益，而 `3` 会显著拖慢日常增量编译。`debug = 1` 维持原有设定（本 workspace 集成测试 target 多，完整调试信息会产出大量 PDB）。

---

## 8 Rust 设计

### 8.1 模块布局与状态

```
src-tauri/src/screenshot/
├── mod.rs         入口：Phase 状态机 / start / cancel / crop / cleanup
├── capture.rs     CaptureBackend trait + XcapBackend + 显示器定位 + 抓屏 + 预览 JPEG 编码
├── crop.rs        令牌校验 + 归一化→原图换算 + 裁剪 + 单次编码 + 归档
└── orb_overlay.rs Orb 窗口隐藏/全屏/恢复快照 + SCREENSHOT_ACTIVE
```

```rust
pub struct ScreenshotState {
    /// 串行化所有状态变更；持有期间允许 await
    op: tokio::sync::Mutex<()>,
    inner: std::sync::Mutex<Inner>,
    backend: Arc<dyn CaptureBackend>,
}

struct Inner {
    phase: Phase,
    session: Option<PendingSession>,
}

enum Phase {
    Idle,
    Capturing,
    Selecting,
    Cropping,
}

struct PendingSession {
    token: String,
    /// 底图 data URL（JPEG、逻辑分辨率）；与 `screenshot-session` 下发的内容完全一致
    preview: String,
    preview_width: u32,
    preview_height: u32,
    /// 抓屏原图常驻内存，裁剪不再解码任何文件
    image: Arc<RgbaImage>,
    target: Target,
    created_at: Instant,
    orb: OrbWindowSnapshot,
}

struct Target {
    monitor_x: i32,
    monitor_y: i32,
    monitor_width: u32,
    monitor_height: u32,
    scale_factor: f64,
}

struct OrbWindowSnapshot {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    visible: bool,
    focused: bool,
    always_on_top: bool,
    click_through: bool,
}
```

- `SCREENSHOT_ACTIVE: AtomicBool`，由 `orb_overlay` 维护；巡检线程、`snap_orb_to_*`、`save_orb_position_sync`、`expand/collapse_orb_panel`、`ORB_BLUR`、`apply_orb_window_attrs` 都读取它。
- `OrbWindowSnapshot.click_through` 必须与 `ORB_CLICK_THROUGH` atomic 一起快照/恢复（F45）：只写窗口 API 会让镜像与实际状态分叉。
- `ScreenshotState::new()` 使用 `XcapBackend`；单测用 `ScreenshotState::with_backend(FakeBackend)`。
- 内存：4K RGBA 约 33MB + 预览 data URL 约 0.3–1MB；只保留一个会话，裁剪/取消即释放。
- **rev7：状态里没有 `preview_path`**，也没有任何磁盘产物，因此不存在「清理磁盘」的状态；`get_screenshot_session` 原样返回 `preview` 字段。

### 8.2 抓屏流程（`start`）

0. 取 `op` 锁；`prune_stale(app)`：清理无覆盖层宿主且超 TTL 的孤儿会话；关闭异常的残留全屏 Orb。
1. `Phase != Idle` 且覆盖层活跃 → 记日志并返回 `Ok(())`；无覆盖层宿主则视为孤儿，已在上一步清理。
2. macOS 权限预检（未授权 → 返回错误码，**不隐藏任何窗口**）。
3. **通知前端进入覆盖层模式**（不变式 25）：`emit_orb_only(SCREENSHOT_PREPARING)`。菜单 / 托盘 / 快捷键入口都由 Rust 直接发起，前端没有工具栏入口那次 `beginPreparing()`，此刻仍在渲染小球；而步骤 11 会把同一个窗口撑成全屏，`OrbBall` 的绝对定位会落到新窗口原点（左上角，F58）。必须在这里发，不能等到步骤 10 的会话事件——那一步紧邻 `present`，前端来不及渲染。
4. 光标位置（物理坐标）：优先 `app.cursor_position()`（F49）→ 失败回退 `get_mouse_position()` 并按该显示器 `scale_factor` 换算（F24）→ 再失败回退 Orb 窗口中心 → 最后回退主屏中心；用 `placement::nearest_screen`（提为 `pub`，零尺寸 `WindowBounds` 表示一个点）选显示器。
5. 从 `CaptureBackend::monitors()` 中按物理 bounds 匹配显示器；`scale_factor` 以 Tauri `available_monitors()` 为准。
6. 快照 Orb 几何/可见性/焦点/置顶/点击穿透（F35/F39/F45）；只隐藏 `orb`（若其可见），`main` / `settings` 保持用户原状并进入底图；`SCREENSHOT_ACTIVE = true`。
7. **确认 Orb 真的不可见**（不变式 22）：`set_visible(false)` 后先查可见性，仍可见则对残留 HWND 补 `ShowWindow(SW_HIDE)`，再 `DwmFlush`；`SCREENSHOT_HIDE_SETTLE_MS` 只作为兜底上限，不是正常路径的固定等待（F53）。
   在此之前先做 **Windows 弹出菜单清理**（不变式 23）：枚举本进程的 `#32768` 窗口（**不过滤可见性**，淡出期间窗口仍存活但已不可见），可见的逐个 `ShowWindow(SW_HIDE)`，预算内仍可见则返回 `capture_failed`。从 Orb 右键菜单 / 托盘菜单触发时，菜单窗口在 `TrackPopupMenu` 返回后可能仍在关闭动画或未隐藏，而它不是 Orb 的子窗口，只隐藏 Orb 会把它冻进底图（F55）。
   隐藏完成后做 **菜单淡出排空**（不变式 24）：淡出起点已在菜单事件回调（点击那一刻）无条件锚定，抓屏前按「锚点已过去的帧数」算出剩余帧并逐帧 `DwmFlush` 排空（`SCREENSHOT_MENU_FADE_FRAMES`，兜底 `SCREENSHOT_MENU_FADE_CAP_MS`），确保 `BitBlt` 读到的是动画之后的合成帧（F56、F57、F60）。
8. `spawn_blocking`：抓 RGBA（首帧为空或全黑重试一次）→ 生成预览 JPEG（长边 `SCREENSHOT_PREVIEW_LONG_EDGE`、质量 `SCREENSHOT_PREVIEW_JPEG_QUALITY`）→ base64 成 data URL。
9. 生成一次性令牌（uuid v4 32 hex），写入 `PendingSession`（含 `preview`），`Phase = Selecting`。
10. **在 Orb 仍隐藏时** `emit_orb_only(SCREENSHOT_SESSION, session)`，让 orb 在隐藏状态下隐藏 chat 并挂载 Overlay。
11. 把 Orb 窗口设为 `always_on_top(true)`、`apply_orb_click_through(false)`、几何写成目标显示器物理矩形（先 `set_size` 后 `set_position`，§8.5.1）；`show()` + `set_focus()`。返回 `Ok(())`。
12. 任一步失败 → 释放预览与内存原图 → 恢复 Orb 窗口/几何 → `SCREENSHOT_ACTIVE = false` → `Phase = Idle` → 返回错误码。步骤 3 之后失败时，前端的 `preparing` 由 `screenshot-error` 事件的 `endPreparing()` 复位；步骤 3 之前（权限/无显示器）失败不碰前端状态。

> **步骤 3 与步骤 10 的分工**：步骤 3 是「提前把 UI 切到覆盖层形态」，只服务于窗口撑大那一刻的渲染正确性；步骤 10 才是「下发会话」。两者都发，不是重复——只有步骤 10 时，`present` 会抢在前端 React 提交之前执行（F58）。

> 先确定显示器再隐藏窗口：鼠标位置不受隐藏影响，权限/无显示器错误不会造成窗口闪烁。
> 隐藏不能只靠固定延时：Tao 的 `set_visible` 只比对标志位，可能变成空操作；必须查可见性、必要时补 `ShowWindow(SW_HIDE)`，再 `DwmFlush`（F53，参考 Kiri ADR 0107）。
> 隐藏范围不只是 Orb：Windows 弹出菜单（`#32768`）是**独立顶层窗口**，Orb 是它的 owner 而非父窗口，`ShowWindow(orb)` 不会带走它。从右键菜单项触发截图时，菜单可能仍处于关闭动画，必须按不变式 23 主动清掉，否则会被冻进底图（F55）。
> 清理菜单不等于清掉残影：DWM 会为关闭中的菜单继续合成淡出帧，而 `xcap` 走的是 GDI `BitBlt(GetDesktopWindow)`（未开 `wgc` feature），读到的就是这些中间帧（F57）。因此 `popup_at` 期间就要给菜单打 `WDA_EXCLUDEFROMCAPTURE`（F56），打标失败才退回「等淡出时长」；不要靠加长 `SCREENSHOT_HIDE_SETTLE_MS` 来掩盖，它只兜底「隐藏是否落地」。
> 抓屏前若首帧为空或全黑，重试一次再判定失败（参考 shiping `take-screenshot.ts`）。
> 预览编码用 JPEG 而非 PNG：底图只用于框选参照，PNG 在 4K 上的编码耗时是 JPEG 的数倍，且没有收益。
> 覆盖层揭示不靠 `SCREENSHOT_OVERLAY_SETTLE_MS`：前端在 `img.decode()` 完成后才显形（§9.4），固定值只兜底。

### 8.3 会话拉取（`get_screenshot_session`）

- 校验调用方 label == `orb`。
- `Phase == Selecting` 且会话存在 → 返回 `ScreenshotSessionDto`；否则 `screenshot.err.no_session`。
- 覆盖层 reload / 错过事件时使用；正常路径以 `screenshot-session` 事件为准。

### 8.4 裁剪（`crop`）

1. 取 `op` 锁；校验调用方 label；校验 token 与 `Phase == Selecting`，否则 `token_invalid`。
2. 置 `Phase = Cropping`；克隆 `Arc<RgbaImage>` 与 target 后释放 inner 锁。
3. 归一化选区 → 原图像素：`sx = round(x * image_w)`、`ex = round((x + width) * image_w)`，y 同理；夹到图片边界；宽或高为 0 → `too_small`。
4. `spawn_blocking`：`crop_imm` → 长边 > 2048 时 `resize(2048, 2048, Lanczos3)` → **一次** `PngEncoder::new_with_quality(Fast, Sub)` 编码。
5. 体积兜底：若结果 > `SCREENSHOT_MAX_BYTES`，改用 `JpegEncoder::new_with_quality(&mut out, 85)` 重编码为 JPEG；仍超限 → `too_large`。
6. 调 `cmd::attachment::archive_bytes(bytes, "<uuid>.png|.jpg", media_type)` **直接归档**，返回 `attachmentId`；失败 → `archive_failed`（令牌保留，用户可重选）。
7. 成功：再次校验 token 仍匹配（防止裁剪期间被取消）→ 清除会话并释放原图/预览 → `Phase = Idle` → 恢复 Orb 几何/可见性/焦点/置顶/点击穿透（含 `ORB_CLICK_THROUGH` 镜像）→ `SCREENSHOT_ACTIVE = false` → 返回 `ScreenshotCropResultDto`。
8. 失败：若 token 仍匹配，`Phase` 回到 `Selecting` 并保留会话，让用户可以重选；若已取消则丢弃中间结果并返回 `token_invalid`。

> **只编码一次**：剪出的 `RgbaImage` 是内存对象，`resize` 后直接编码即可。不要「先编码 PNG 再交给 `normalize_image`」——那是 decode + Lanczos3 + 再编码的第二次完整往返（F52）。我们生成的字节不需要「校验不可信输入」式的解码，长边与体积上限在这里就地保证。
> **Fast 压缩有依据**：Cap 对 >8MP 的截图用 `CompressionType::Fast`，Kiri 的冻结静帧固定用 `Fast + FilterType::Sub`；默认压缩的耗时是数倍于 Fast，而 UI 截图上几乎看不出差异。
> **归档在命令内完成**，因此没有「待消费凭据」：不存在 `ingestPath` 失败后删临时文件的分支，也不需要 TTL 清扫。
> 前端收到结果后必须**先卸载覆盖层**再落地附件（不变式 21）：Rust 在返回前已恢复 Orb 几何，晚一帧卸载就会把全屏底图压进小球窗口。

### 8.5 Orb 全屏与恢复（`orb_overlay.rs`）

```rust
// 进入覆盖层
let snapshot = OrbWindowSnapshot::capture(&window)?;
window.set_always_on_top(true)?;
apply_orb_click_through(app, false)?;   // 同步 ORB_CLICK_THROUGH 镜像，不直接调窗口 API
// WebviewWindow 没有 set_bounds（见 §8.5.1），先 size 后 position
window.set_size(PhysicalSize::new(target.monitor_width, target.monitor_height))?;
window.set_position(PhysicalPosition::new(target.monitor_x, target.monitor_y))?;
window.show()?;
window.set_focus()?;

// 退出覆盖层（取消/成功）：先隐藏，再改几何，最后按快照恢复可见性
window.hide()?;                         // ① 必须在不可见状态下改几何（不变式 26）
window.set_size(snapshot.size)?;        // ②
window.set_position(snapshot.position)?;// ③
window.set_always_on_top(snapshot.always_on_top)?;
apply_orb_click_through(app, snapshot.click_through)?;
if snapshot.visible { window.show()?; } // ④ 快照本来不可见时不再补 show
```

要点：

- Orb 窗口本来是透明、无边框、置顶、`skipTaskbar`、不可缩放；程序化写几何仍有效（F35）。
- 恢复必须写回**精确物理几何**（F40）；前端 `useOrbStore.isExpanded` 不需要改。
- **几何写入用回退序列（先 size 后 position），恢复同样写回快照值**（不变式 20）：rev8 核对 Tauri 2.12.5 源码，`set_bounds` 只存在于 `Webview`，`WebviewWindow` 没有该方法，因此直接采用 §8.5.1 的回退方案；写入与恢复都必须来自快照，不要用 `inner_size()` 重算。
- **恢复必须先隐藏**（不变式 26）：两次几何写入之间会回到消息循环，DWM 可合成一帧；窗口此刻可见且位于显示器原点，那一帧就是「小球停在左上角」（F59）。`present()` 无需隐藏前置——它执行时窗口本就不可见。
- **异常残留复位同理**：`recover_leftover` 面对的也是「可见的全屏覆盖层停在原点」，缩回小球前必须先 `hide()`。
- **点击穿透必须走 `apply_orb_click_through`**（不变式 19）：直接 `set_ignore_cursor_events` 不会更新 `ORB_CLICK_THROUGH`，恢复后 `apply_orb_click_through(true)` 会早退，用户的穿透设置永久失效（F45）。
- **DWM 过渡在「隐藏 / 显影 / 还原」三段内都要禁用，离开时写回原值**（Windows）：只禁隐藏不够——`orb` 是 `transparent: true` 的分层窗口，显影与缩回小球时的淡入淡出会把实时桌面透出来，和「冻结底图」冲突；写回值必须用 `DwmGetWindowAttribute` 读到的原值，不能无条件写成「启用」，否则会覆盖系统「关闭动画」设置。
- 成功时无论 Orb 原先是否可见，都 `show()` + 聚焦 + 事件驱动 `expand()`；取消时按快照恢复可见性/焦点。
- `SCREENSHOT_ACTIVE` 在进入前置 true、退出后置 false；任何失败路径都必须复位。

#### 8.5.1 几何写入与恢复（`set_bounds` 回退已定稿）

规格原计划用一次 `set_bounds`（物理 `Rect`）写入覆盖层几何，以规避「先迁移 DPI 再应用尺寸」的一帧错位。rev8 核对 Tauri 2.12.5 源码后确认**这条路不可用**：

- `set_bounds` 定义在 `tauri::webview::Webview`（`src/webview/mod.rs`），`WebviewWindow`（`src/webview/webview_window.rs`）只暴露 `set_size` / `set_position`，没有 `set_bounds`。
- Orb 覆盖层宿主持有的是 `WebviewWindow`，因此直接采用回退方案：

```rust
window.set_size(PhysicalSize::new(w, h))?;
window.set_position(PhysicalPosition::new(x, y))?;
```

- **先 size 后 position**，恢复路径同样写回快照值而不是 `inner_size()` 重算；实测依据是副屏首帧铺满与取消后位置一致（§11.3「混合 DPI」「Orb 在副屏」两条）。
- **写入覆盖层时窗口不可见，恢复时必须先隐藏**（不变式 26）：这两次写入之间会回到消息循环，DWM 可合成一帧。进入路径上窗口是隐藏的，中间帧不可见；恢复路径上窗口是可见的全屏覆盖层，中间帧就是「缩成小球的窗口停在原点」——必须先 `set_visible(false)` 再改几何，最后按快照恢复可见性（F59）。
- 若后续 Tauri 为 `WebviewWindow` 补上 `set_bounds`，再切回单次写入，并同步更新不变式 20（「恢复前先隐藏」仍然保留，它与写入次数无关）。

#### 8.5.2 菜单淡出排空：按帧，不按毫秒

`src-tauri/src/screenshot/orb_overlay.rs`（Windows `platform` 模块）负责这一段，分两个时机：

- **点击那一刻**（`menu.rs` / `tray.rs` 的截图项分支）：`dismiss_menus_now()` 先 `SW_HIDE` 掉仍存活的菜单窗口，再**无条件**写入淡出锚点。写锚点与「枚举到几个窗口」无关——DWM 会在合成器侧用缓存表面把关闭动画播完，枚举为空不代表没有残影（F56）。
- **抓屏之前**（`settle_menu_fade()`）：读取锚点，算剩余帧并逐帧消费。

```rust
fn drain_menu_fade_frames(age: Duration) -> u32 {
    let period = frame_period();                                  // DwmGetCompositionTimingInfo，失败按 60Hz 兜底
    let already = (age.as_micros() / period.as_micros().max(1)) as u32;
    let remaining = SCREENSHOT_MENU_FADE_FRAMES.saturating_sub(already);
    let started = Instant::now();
    let mut flushed = 0u32;
    for _ in 0..remaining {
        unsafe { let _ = DwmFlush(); }                            // 空闲时也会等到下一个合成帧
        flushed += 1;
        if started.elapsed() >= Duration::from_millis(SCREENSHOT_MENU_FADE_CAP_MS) { break; }
    }
    flushed
}
```

要点：

- **帧数而非毫秒**（F60）：实测 Win11 默认动画是 19 个合成帧的线性过渡，与菜单项数量无关。固定毫秒在高刷屏上白等、低刷屏上漏帧，正是此前「等待时长对不上」的根因。
- **帧周期用查询而不是测量**：`DwmGetCompositionTimingInfo` 直接回报 `rateRefresh`，不需要靠 `DwmFlush` 消费帧来反推——后者会在关键路径上白等两帧。
- **已过去的帧要扣掉**：从点击到抓屏之间还有枚举显示器、取光标、隐藏 Orb 等步骤，这段时间里动画已经播掉一部分；按 `age / period` 折算成已消费帧数，避免重复等待。
- **系统关掉「菜单动画」时不等待**：`SPI_GETMENUANIMATION` 为 0 时没有淡出，只做常规合成刷新（隐藏序列自带的那次 `DwmFlush`）。
- **上限只防退化**：`DwmFlush` 在远程桌面、窗口被遮挡等场景可能提前返回，`SCREENSHOT_MENU_FADE_CAP_MS` 防止循环变成忙等，不是正常路径的耗时。
- **日志口径**：`screenshot: menu fade anchor={age}ms period={period}ms flushed_frames={n} animation={bool} live_windows={n}`——`anchor` 远小于帧数 × 周期说明锚点确实落在点击那一刻；`flushed_frames` 明显小于剩余帧说明被上限截断（异常路径）。

### 8.6 状态守卫（关键）

- **`setup.rs` `spawn_screen_inspection`**：守卫 `if screenshot::is_active() { continue; }`；原因 防止每 800ms 把全屏几何写回 `OrbConf.position_x/y`（F36）
- **`cmd/orb.rs` `snap_orb_to_screen` / `snap_orb_to_edge` / `save_orb_position_sync`**：守卫 `if screenshot::is_active() { return Ok(()); }`；原因 防止吸附/保存把全屏几何当作用户位置
- **`window.rs` `expand_orb_panel` / `collapse_orb_panel`**：守卫 `if screenshot::is_active() { return Ok(()); }`；原因 防止覆盖层被前端或残留调用改尺寸
- **`window.rs` `apply_orb_window_attrs`**：守卫 `if screenshot::is_active() { return; }`；原因 设置变更/启动会把 `always_on_top` 写回 conf，打掉全屏置顶（F46）
- **`cmd/orb.rs` `apply_orb_click_through`**：守卫 覆盖层只经它写穿透，它本身不改，`ORB_CLICK_THROUGH` 保持同步；原因 绕过 atomic 会让后续穿透设置永久失效（F45）
- **`window.rs` `on_window_event` Orb `Focused(false)`**：守卫 `if screenshot::is_active() { return; }`；原因 不发送 `ORB_BLUR`，避免 `OrbPanel` 折叠（F37）
- **`window.rs` `on_window_event` Orb `CloseRequested`**：守卫 截图期间 `prevent_close()` + 走取消清理；原因 防止截图会话悬空
- **`src/orb/OrbExpand.ts` `isOrbBlurCollapseSuppressed()`**：守卫 扩展为「时间窗未到 **或** 截图会话活跃」；原因 复用现有抑制机制，不新增第三套判断（F47）
- **`src/layouts/OrbWindow.tsx`**：守卫 覆盖层期间加 `orb-screenshot` class，`OrbBall`/`OrbPanel` 只 CSS 隐藏；原因 保留 `ConversationView` 草稿（F38）
- **`src/chat/ConversationView.tsx` 截图按钮**：守卫 `stream.isStreaming` 时禁用；原因 与附件按钮一致，vision 由准入层拦（F41）

### 8.7 隐私与权限

- macOS：`CGPreflightScreenCaptureAccess` 预检后，还要用 `CaptureBackend::monitors()` / `SCShareableContent` 验证「预检通过但实际无显示器可抓」的陈旧 TCC 状态；验证成功可缓存，失败按短间隔重试（参考 Cap `permissions/screen_capture.rs`）。预检走 `CoreGraphics` 框架的 C 接口；该分支受 `cfg(target_os = "macos")` 保护，Windows 构建只做语法审查，须由 macOS 构建与真机验收确认。
- Windows / Linux：预检恒 `true`。
- 保险库锁屏、系统空闲锁定：`ScreenLockMonitor` 只在 vault feature 内构造（F30），宿主层拿不到。**V1 明确不做锁屏 gate，在验收里标注为已知缺口；V2 把 monitor 提升为全局 `ScreenLockState` 后再接入。**
- 令牌一次性、只发给 orb；`crop/cancel` 校验调用方 label 与令牌。
- 原图与预览只存在内存，随会话释放；不写系统临时目录，也不写应用数据目录。
- 参考 Cap 的 `overlay_backdrop.rs`：Windows `WDA_EXCLUDEFROMCAPTURE` / macOS `NSWindowSharingNone` 可把自身窗口排除出截图。V1 仍用「隐藏 `orb` + 可见性校验 + `DwmFlush`」保证跨平台一致；V2 若要上捕获排除，必须先解决「Orb 是分层窗口，`SetWindowDisplayAffinity` 会失败」这一限制（F54、§6.3）。

### 8.8 生命周期与清理

截图链路不落盘，所以「清理」只剩释放内存与复位窗口：

- **取消（ESC / 右键 / `cancel_screenshot`）**：清会话（释放原图与预览 data URL）；恢复 Orb 几何/可见性/焦点/置顶/穿透；`Phase = Idle`
- **Orb `CloseRequested` 且截图活跃**：`prevent_close` + 取消清理
- **抓屏/编码/建覆盖层失败**：释放已产生的中间结果；恢复 Orb；`SCREENSHOT_ACTIVE = false`；`Phase = Idle`
- **裁剪成功**：字节已进 `AttachmentStore`；清会话、释放原图与预览；恢复 Orb 几何
- **归档失败**：不落任何字节；令牌保留，覆盖层停在选图状态，用户可重选或取消
- **进程异常退出**：没有残留（本就不落盘）

实现要点：

- `AttachmentStore` **不新增任何函数**（无 `is_screenshot_tmp_path` / `remove_screenshot_tmp` / `sweep_screenshot_tmp`），`clean_tmp()`（F21）保持原语义。
- `cmd/attachment.rs` 只新增 `archive_bytes(bytes, file_name, media_type) -> Result<AttachmentImportDto>`：校验 `bytes.len() <= ATTACHMENT_LIMITS.max_image_bytes` → `save_attachment`。`import_attachment` 改为「`plan_import` → 读文件 → 调 `archive_bytes`」，行为不变（F51）。
- 「发送失败后重试」直接复用已归档的 `attachmentId`；「移除未发送的截图」走既有 `removeAttachment` 的归档删除流程，截图侧无需配合。
- 唯一需要长期存在的是 `SCREENSHOT_SESSION_TTL_MS`：覆盖层宿主消失（reload / 崩溃）后，Rust 侧残留会话要能自行释放那 33MB。

### 8.9 测试缝

- `CaptureBackend`：`FakeBackend` 提供固定尺寸图片与可配置显示器列表，覆盖单屏、双屏、副屏负坐标。
- `crop::to_image_rect(normalized, image_w, image_h)` 纯函数，覆盖 0.1/0.25/0.5/0.75/1.0 与边界夹取。
- `Phase` 状态机脱离 Tauri 测试：忙时 `start` 忽略、`cancel` 幂等、裁剪期间取消。
- TTL 判断注入 `now`，避免测试 sleep。
- `preview` 生成：给定 `FakeBackend` 的固定图与 `scale_factor`，断言 JPEG 长边 ≤ `SCREENSHOT_PREVIEW_LONG_EDGE`、data URL 以 `data:image/jpeg;base64,` 开头、`preview_width/height` 与实际编码一致。
- `archive_bytes` 单测：正常归档返回 `attachmentId` 且 `bytes` 与入参一致；超 8MB 返回 `too_large`；同一内容重复归档命中内容寻址、不产生第二份。
- `orb_overlay` 的 `begin()/exit()`：用 `FakeOps` 断言隐藏序列「`set_visible(false)` → 可见性仍为真时补 `ShowWindow(SW_HIDE)` → `DwmFlush`」与「补 `ShowWindow` 仍可见时返回 `capture_failed`」，以及「快照读全字段、还原按快照写回置顶/几何/可见性并把 `click_through` 交给回调」「任一项失败仍走完剩余步骤并返回 `cancel_failed`」。
- Windows 弹出菜单清理（不变式 23）：无菜单时为空操作、不阻塞；有菜单时 `SW_HIDE`；预算内仍可见返回 `capture_failed`。`#32768` 枚举与 `DwmFlush` 属 Win32 副作用，不入单测，由 §11.3「右键菜单触发」一项人工验收。
- 菜单淡出排空（不变式 24、F60）：`dismiss_menus_now` 的锚定是无条件写入的内存状态（不依赖枚举结果）；`settle_menu_fade` / `drain_menu_fade_frames` 是 Win32 副作用（`DwmFlush`、`DwmGetCompositionTimingInfo`、`SPI_GETMENUANIMATION`），不入单测；由 §11.3 人工验收（对照日志里的 `screenshot: menu fade anchor=…ms period=…ms flushed_frames=…`）。「剩余帧数」的折算公式（`age / period` 向下取整、饱和相减）若要单测，应抽成不依赖 Win32 的纯函数再测，不要为了测它去 mock 合成器。
- `setup.rs` 的快捷键解析：逻辑已下沉到 `assistant-config::shortcut::resolve_bindings`（无 tauri 依赖），由 `crates/assistant-config/tests/shortcut.rs` 覆盖默认、覆盖、显式清空、非法、重复、缺默认六类场景；`setup.rs` 只保留副作用（`parse_shortcut` 回调 + 注册 + 告警广播）。

---

## 9 前端设计

### 9.1 覆盖层宿主

不新增布局/路由。`src/layouts/OrbWindow.tsx`：

```tsx
const session = useScreenshotStore((s) => s.session);
const preparing = useScreenshotStore((s) => s.preparing);
const overlayActive = session !== null || preparing;

return (
  <div
    data-orb-window
    className={[orbClassName, overlayActive ? "orb-screenshot" : ""]
      .filter(Boolean)
      .join(" ")}
  >
    <OrbBall />
    <OrbPanel />
    {session && <ScreenshotOverlay session={session} />}
  </div>
);
```

`src/assets/styles/global.css`：

```css
div[data-orb-window].orb-screenshot .orb-hitbox,
div[data-orb-window].orb-screenshot .orb-panel {
  display: none;
}
```

- `OrbBall`/`OrbPanel` 仍然挂载，`ConversationView` 的 `MessageInput` 草稿、附件、流式状态保留。
- `ScreenshotOverlay` 用 `position: fixed; inset: 0; z-index: 1;` 覆盖；底图铺满。
- `preparing` 用于工具栏入口：点击后立刻隐藏 chat，避免抓屏/放大过程露出旧 UI；失败或事件到达后复位。
- 快捷键入口不需要 `preparing`：抓屏时 Orb 已被 Rust 隐藏，会话事件到达后才显示。

### 9.2 会话订阅

`src/screenshot/useScreenshotSession.ts`（在 `OrbWindow` 中调用）：

```ts
useEffect(() => {
  const unPreparing = listen(UiEvent.ScreenshotPreparing, () => {
    useScreenshotStore.getState().beginPreparing();
  });
  const unSession = listen<ScreenshotSessionDto>(
    UiEvent.ScreenshotSession,
    (e) => {
      useScreenshotStore.getState().setSession(e.payload);
      useScreenshotStore.getState().endPreparing();
    },
  );
  const unError = listen<string>(UiEvent.ScreenshotError, (e) => {
    useScreenshotStore.getState().endPreparing();
    notifyScreenshotError(e.payload);
  });
  void screenshotService
    .getSession()
    .then((s) => {
      if (!useScreenshotStore.getState().session) {
        useScreenshotStore.getState().setSession(s);
      }
    })
    .catch(() => {});
  return () => {
    void unPreparing.then((f) => f());
    void unSession.then((f) => f());
    void unError.then((f) => f());
  };
}, []);
```

- `screenshot-preparing` 只做 `beginPreparing()`：这是菜单 / 托盘 / 快捷键入口进入覆盖层模式的**唯一**途径（工具栏入口自己已经先调过一次，重复设置幂等）。缺了它，窗口被撑成全屏时小球会落在左上角（F58、不变式 25）。
- `get_screenshot_session` 是兜底：reload、错过事件、启动时序。
- `no_session` 错误静默忽略；其他错误走 toast。
- `notifyScreenshotError(code)` 放 `src/screenshot/session.ts`：用既有 `notifyError(translateKey(code), translateKey(code))`（`src/stores/notifyStore.ts`），未知错误码原样作为文本，不新建通知组件。
- `screenshotStore`：`session`、`preparing`、`finishing`、`setSession`、`clearSession`、`beginPreparing`、`endPreparing`、`beginFinishing`、`endFinishing`。
- `finishing` 的用途见 §9.4：从「发出 `crop`/`cancel`」到「Rust 恢复几何」之间，覆盖层必须已经看不见。

### 9.3 选区几何（纯函数，可单测）

`src/screenshot/selection.ts`：

```ts
export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}
export interface NormRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export function normalizeDrag(
  startX: number,
  startY: number,
  curX: number,
  curY: number,
): Rect;
export function clampToBounds(rect: Rect, bounds: Rect): Rect;
export function resizeByHandle(
  rect: Rect,
  handle: HandleId,
  dx: number,
  dy: number,
  bounds: Rect,
): Rect;
export function isValidSelection(rect: Rect): boolean; // 宽高均 >= SCREENSHOT_MIN_SELECTION_CSS_PX
export function toNormalizedRect(
  rect: Rect,
  viewport: { width: number; height: number },
): NormRect;
```

- 前端只产出 CSS 逻辑像素与归一化值；**不提供** `toPhysical`（不变式 3）。
- 归一化基于底图显示区域（`img.getBoundingClientRect()`），即使窗口尺寸与显示器物理尺寸有细微差异也不会错位。
- 全部纯函数、不依赖 DOM 与 store，直接用 vitest 覆盖。

### 9.4 Overlay 组件

`src/screenshot/Overlay.tsx`：

- 底图 `<img src={session.preview}>`（data URL，不需要 `convertFileSrc`），`width/height: 100%; object-fit: fill`。
- **揭示时机**：`await img.decode()` 成功后把 `revealed` 置 true，覆盖层在此之前保持透明/中性底色。这比固定 `SCREENSHOT_OVERLAY_SETTLE_MS` 确定：底图没解码完就显形，才会露出被拉成全屏的旧 UI。
- `onError`（data URL 不该失败）显示错误并取消。
- 一层 SVG/canvas 叠加：遮罩（`rgba(0,0,0,0.45)`）、选框边框、尺寸标注、8 个手柄。
- 键盘：`ESC` 取消，`Enter` 确认；右键取消；双击选区内部 = 确认。
- 确认期间 `isCropping = true`，禁用重复确认/取消；命令失败保持覆盖层并显示错误。
- **确认（顺序固定，见不变式 21）**：

```ts
useScreenshotStore.getState().beginFinishing();   // ① 同步把底图/选框设为不可见
try {
  const result = await screenshotService.crop({ token, ...toNormalizedRect(selection, viewport) });
  useScreenshotStore.getState().clearSession();    // ② 会话结束（此时 Rust 已恢复几何）
  // ③ 落地附件 + 展开 + 聚焦（§9.5）
} catch (e) {
  useScreenshotStore.getState().endFinishing();    // 失败：恢复底图与选区，允许重选
  notifyScreenshotError(e);
}
```

- ① 必须在任何 `await` 之前：Rust 在 `crop` 返回**之前**就恢复了 Orb 几何，晚一帧隐藏就会把全屏底图压进 88px 的窗口。`finishing` 下覆盖层只渲染中性底色，不渲染底图。
- **取消**同样是「先 `beginFinishing()`，再 `await cancel(token)`，最后 `clearSession()`」；`cancel` 失败时 `endFinishing()` 并提示，会话仍可重试。
- 窗口失焦不自动取消（用户可能切去查资料）；无覆盖层宿主后由 Rust TTL 兜底。
- 光标 `crosshair`；`user-select: none`；阻止右键菜单默认行为。

### 9.5 结果接入

只需要在 `src/chat/attachment/factory.ts` 增加一个「把已归档结果包成 `Attachment`」的构造器，其余不动：

```ts
import type { AttachmentImportDto } from "@/types/protocol.contract";

/** 把已归档的导入结果包成 Attachment；不触发任何 IPC */
export function attachmentFromImport(
  dto: AttachmentImportDto,
  meta?: { width?: number; height?: number },
): AttachmentResult;
```

- 与 `createPathAttachment` 共用后半段构造逻辑（`id`、`file` 占位、`mimeType`、`originalSizeBytes`、`previewUrl = attachmentUrl(dto.attachmentId)`），区别只是**不调用** `importAttachment`。
- `meta.width/height` 写进附件，避免缩略图首帧按 1:1 布局再跳变（`ImageThumbnail` 只在两者都存在时用 `aspectRatio`）。
- 不接收 `bytes`/MIME 覆盖值：一律以 `dto` 为准（F51）；`mediaType` 必须与 `fileName` 扩展名一致（不变式 12）。
- 前端不做体积预判：超限在 Rust 的 `archive_bytes` 就返回 `too_large`，与 `import_attachment` 同一条错误串（F16）。

Overlay 侧（不需要跨窗口事件，也不需要 `ingestPath`）：

```ts
const result = await screenshotService.crop({
  token: session.token,
  ...toNormalizedRect(selection, viewport),
});
useScreenshotStore.getState().clearSession();
const ingested = attachmentFromImport(result, {
  width: result.width,
  height: result.height,
});
if (!ingested.ok) {
  notifyAttachError(ingested.error);
} else {
  const err = useAttachmentStore.getState().addAttachment(ingested.attachment);
  if (err) notifyAttachError(err);   // 与粘贴/选文件同一条提示路径
}
if (!useOrbStore.getState().isExpanded) await expand();
requestChatInputFocus();
```

- `expand()` 来自 `src/orb/OrbExpand.ts` 的 `useOrbExpand()`，复用「先请求 Rust 改窗口尺寸 → 再切 store 状态」。
- 不自动发送；准入仍由 `admitAttachment` 决定（vision 校验、4 张上限），截图不绕过任何校验（不变式 8）。
- `useAttachmentIntake()` / `ingestPath()` **不需要改动**：截图不再走路径导入，rev6 的「导出 `ingestPath`」任务取消。
- **不需要**「移除附件时释放文件」与「发送成功后再清理」：字节从一开始就在归档库里，移除走既有 `removeAttachment`，发送失败重试复用同一个 `attachmentId`。

### 9.6 入口

- **全局快捷键**：位置 `src-tauri/src/setup.rs`；改动 `SHORTCUT_SPECS` 加 `capture_screenshot`，按下调用 `screenshot::start()`，失败发 `SCREENSHOT_ERROR`
- **Orb 右键菜单**：位置 `src-tauri/src/window.rs` `show_orb_context_menu` + `src-tauri/src/menu.rs`；改动 新增 `orb_screenshot` 菜单项 + 事件分支（F26）
- **聊天工具栏**：位置 `src/chat/ConversationView.tsx`（按钮在 `src/chat/layouts/ConversationFrame.tsx`）；改动 在 `PaperClipOutlined` 旁加 `ScissorOutlined`，`beginPreparing()` 后 `start()`，失败 toast（F41）
- **托盘菜单**：位置 `src-tauri/src/tray.rs`；改动 新增 `screenshot` 菜单项（可选，M2）

> **前三个 Rust 侧入口都不在前端发起**：它们直接调 `screenshot::start()`，前端状态仍是「小球形态」。覆盖层模式的切换统一由 `start` 里的 `SCREENSHOT_PREPARING` 事件驱动（不变式 25）；工具栏入口自己先调一次 `beginPreparing()` 只是让 UI 更早消失，不能因此省掉事件——否则同一个 `start` 会有两套前置条件，后续新增入口又会漏（F58）。
> 全局快捷键不受 `supportVision` 限制：无 vision 模型时仍会截图，但落进输入区时会得到与粘贴一致的 `no_vision` 提示。

### 9.7 冲突检测的边界

- 设置页只检测应用内部重复（`detectConflicts`），**不维护其他应用快捷键的静态表**。
- 外部快捷键无法可靠枚举：同一组合在不同应用/系统版本可能不同，静态表会过时且误导。
- 真正的冲突由操作系统在 `register_global_shortcuts` 时暴露：解析失败、已被占用都会通过 `SHORTCUT_WARNING` 返回 id/binding/error。
- 产品上只做两件事：默认值选低冲突的四键组合；注册失败可见（前端提示）。
- 「恢复默认」使用新组合；已有配置不会被静默改写。

### 9.8 输入框聚焦

`MessageInput` 目前没有 focus 句柄（F29）。增加：

- `Input.TextArea` 绑定 `ref`；
- 监听 `window` 事件 `chat-focus-input`，收到后 `focus()`；
- `src/screenshot/session.ts` 提供 `requestChatInputFocus()` 派发该事件。

### 9.9 i18n

`src/i18n/locales/zh-CN.json` 与 `en-US.json` 新增：

```json
"settings.shortcuts.capture_screenshot": "截图提问",
"chat.input.screenshot": "截图",
"screenshot.select_hint": "拖动框选（只截光标所在屏幕），Enter 确认，Esc 取消",
"screenshot.too_small": "选区太小",
"screenshot.permission_denied": "需要「屏幕录制」权限",
"screenshot.permission_guide": "前往 系统设置 → 隐私与安全性 → 屏幕录制 授权后重试",
"screenshot.capture_failed": "截屏失败",
"screenshot.session_expired": "截图会话已过期，请重新截图",
"screenshot.too_large": "截图过大，请缩小选区",
"screenshot.err.no_monitor": "未找到可用显示器",
"screenshot.err.encode_failed": "截图处理失败",
"screenshot.err.archive_failed": "截图保存失败",
"orb.context_screenshot": "截图提问"
```

Rust 侧文案（托盘 / 右键菜单）走 `assistant_i18n::t()`，键名同步加入 `src-tauri/crates/assistant-i18n/locales/*.json`。

---

## 10 文件清单

### 10.1 新建

- `src-tauri/src/screenshot/mod.rs`：`Phase` 状态机、`start` / `cancel` / `crop` / `cleanup`、op lock
- `src-tauri/src/screenshot/capture.rs`：`CaptureBackend` trait + `XcapBackend`、显示器定位、抓屏、预览 JPEG 与 data URL 编码
- `src-tauri/src/screenshot/crop.rs`：令牌校验、归一化→原图换算、裁剪、单次编码、JPEG 兜底、调用 `archive_bytes`
- `src-tauri/src/screenshot/orb_overlay.rs`：Orb 隐藏校验（`ShowWindow` + `DwmFlush`）、全屏与恢复快照（恢复先隐藏再改几何）、`SCREENSHOT_ACTIVE`
- `src-tauri/src/cmd/screenshot.rs`：五条命令与调用方校验
- `src-tauri/src/protocol/dto/screenshot.rs`：三个 DTO
- `src/screenshot/Overlay.tsx`：覆盖层组件（data URL 底图、`img.decode()` 揭示、`finishing` 态）
- `src/screenshot/selection.ts`：选区几何纯函数（含归一化）
- `src/screenshot/session.ts`：invoke 封装、错误码翻译与 `notifyScreenshotError`、`requestChatInputFocus`
- `src/screenshot/useScreenshotSession.ts`：preparing / 会话 / 错误事件订阅 + 兜底拉取
- `src/stores/screenshotStore.ts`：`session` / `preparing` / `finishing`
- `src/screenshot/__tests__/selection.test.ts`：几何单测

### 10.2 修改

- `src-tauri/Cargo.toml`：`[workspace.dependencies]` 加 `xcap = "0.9"`；`[dependencies]` 加 `xcap = { workspace = true }` 与 `image = { workspace = true }`
- `src-tauri/src/ipc.rs`：五条命令登记
- `src-tauri/src/cmd/mod.rs`：`mod screenshot` + re-export
- `src-tauri/src/protocol/dto/mod.rs`：`pub mod screenshot;` + `pub use screenshot::*;`
- `src-tauri/src/protocol/events.rs`：两个事件常量
- `src-tauri/src/setup.rs`：`ScreenshotState` 注册 + 巡检线程守卫（**不建目录、不做 sweep**；快捷键表驱动已落地，只加一项）
- `src-tauri/src/window.rs`：右键菜单加项；Orb 关闭/失焦守卫；`expand/collapse_orb_panel` 与 `apply_orb_window_attrs` 守卫
- `src-tauri/src/menu.rs`：`orb_screenshot` 事件分支
- `src-tauri/src/lib.rs`：`mod screenshot;`
- `src-tauri/tests/protocol_bindings.rs`：三个 DTO 登记（无数字断言）
- `src-tauri/crates/assistant-config/src/constant.rs`：八个常量（两个 settle 兜底、菜单淡出帧数 + 排空上限、会话 TTL、单张上限、两个预览参数）
- `src-tauri/crates/assistant-config/src/app_conf.rs`：`DEFAULT_SHORTCUT_PATTERNS` 加 `capture_screenshot`（其余已落地）
- `src-tauri/src/cmd/attachment.rs`：抽出 `pub(crate) async fn archive_bytes(bytes, file_name, media_type)`；`import_attachment` 改为复用它（行为不变）
- `src-tauri/crates/assistant-orb/src/geometry.rs`：`nearest_screen` 由 `fn` 提为 `pub fn`（实现不变）
- `src-tauri/crates/assistant-orb/src/placement.rs`：re-export `nearest_screen`（`pub use super::geometry::{..., nearest_screen}`）
- `src-tauri/src/cmd/orb.rs`：`get_screen_size` 改用 `placement::nearest_screen`；`snap_*`/`save_orb_position_sync` 截图守卫
- `src-tauri/src/tray.rs`：M2 托盘菜单项
- `src/types/protocol.contract.ts`：`npm run contract:gen` 生成（禁止手改）
- `src/types/shortcutDefaults.generated.ts`：`contract:gen` 生成（禁止手改）
- `src/settings/core/types/shortcutTypes.ts`：`SHORTCUT_DEFINITIONS` 加 `capture_screenshot` 项（`defaultBinding` 从生成文件展开，不手写）
- `src/settings/__tests__/fixtures/snapshot.full.json`、`snapshot.vaultless.json`：新增 `keybindings.capture_screenshot` 一行（`-u` 重新生成后人工核对）
- `src/types/events.ts`：`UiEvent.ScreenshotSession` / `ScreenshotError`
- `src/config/constants.ts`：`SCREENSHOT_MIN_SELECTION_CSS_PX`
- `src/i18n/locales/*.json`：§9.9 前端键
- `src/chat/attachment/factory.ts`：新增 `attachmentFromImport(dto, meta)`（`createPathAttachment` **不改**）
- `src/chat/ConversationView.tsx`：截图按钮（`onScreenshot` 回调传给 `ConversationFrame`）
- `src/chat/layouts/layoutProps.ts`：`ConversationLayoutProps` 加 `onScreenshot: () => void`（`chat`/`agent` 两条链路共用）
- `src/chat/layouts/ConversationFrame.tsx`：工具栏加 `ScissorOutlined` 按钮
- `src/chat/message/MessageInput.tsx`：`ref` + `chat-focus-input` 监听
- `src/orb/OrbExpand.ts`：`isOrbBlurCollapseSuppressed()` 增加「截图会话活跃」条件
- `src/layouts/OrbWindow.tsx`：覆盖层条件渲染 + `orb-screenshot` class
- `src/assets/styles/global.css`：隐藏 OrbBall/OrbPanel 的截图态样式
- `src-tauri/crates/assistant-i18n/locales/*.json`：Rust 侧右键/托盘文案

> **rev7 明确不需要**：`tauri.conf.json`、`capabilities/default.json`、`src-tauri/src/app_dirs/**`、`tests/app_dirs_contract.rs`、`assistant-storage/src/attachment_store.rs`、`src/chat/attachment/intake.ts`、`App.tsx` 的 `viewMap`、`src/layouts/ScreenshotWindow.tsx`、`assistant-agent` 的任何改动。

---

## 11 验收

### 11.1 单测

- **`selection.ts`**：反向拖拽归一化、越界夹取、8 向手柄最小尺寸、最小选区判定、归一化在任意 viewport 下的边界
- **`crop.rs`**：令牌缺失/不匹配 → 错误且不归档；归一化越界 → 夹取；宽/高为 0 → `too_small`；超 8MB → JPEG 兜底后仍在限内且 `media_type = image/jpeg`、`file_name` 以 `.jpg` 结尾；否则 `image/png` + `.png`；用注入的编码器断言**只编码一次**（不经过 `normalize_image`）
- **`crop` 换算**：0.1/0.25/0.5/0.75/1.0 归一化在 1000×500 / 3840×2160 下的取整与夹取
- **`capture.rs`**：显示器选择——单屏、双屏（光标在副屏）、副屏负坐标；`FakeBackend`
- **状态机**：忙时 `start` 忽略；`cancel` 幂等；裁剪期间取消丢弃产物
- **`orb_overlay`**：只快照/隐藏 `orb`；`main` / `settings` 不进入快照；隐藏序列为「可见性 → `ShowWindow(SW_HIDE)` → `DwmFlush`」；隐藏失败时按快照复位 Orb
- **`orb_overlay` 恢复序列（rev10）**：`restore_hides_before_geometry_change` 断言 `hide` 的调用顺序**早于** `set_size` / `set_position`（不变式 26、F59）；`restore_writes_snapshot_and_click_through` 断言按快照写回置顶/几何/可见性并把 `click_through` 交给回调；`restore_keeps_going_and_reports_cancel_failed` 断言任一步失败仍走完剩余步骤并返回 `cancel_failed`
- **`start` 的 preparing 事件（rev10）**：`FakeBackend` 下断言 `SCREENSHOT_PREPARING` 在 `capture()` 之前发出（不变式 25、F58）；权限/无显示器失败时不发
- **`archive_bytes`**：正常返回 `attachmentId` 与 `bytes`；超 8MB → `too_large`；同一内容重复归档命中 sha256 去重、不产生第二份
- **`setup.rs`**：快捷键解析——默认、覆盖、显式清空、非法、重复、缺默认，逻辑下沉到 `assistant-config::shortcut::resolve_bindings`，由 `crates/assistant-config/tests/shortcut.rs` 覆盖

### 11.2 契约与快照

- `npm run contract:gen` 后 `src/types/protocol.contract.ts` 出现三个新类型，`shortcutDefaults.generated.ts` 出现 `capture_screenshot`，`git diff` 只含新增。
- `src-tauri/tests/protocol_bindings.rs` 登记三个 DTO 后 `cargo test --test protocol_bindings` 通过（该文件只校验类型存在与字段一致，无数量断言）。
- `npm run contract:check` 干净。
- `npm run test` 全绿。新增 `capture_screenshot` 会新增一行 `keybindings.capture_screenshot` 字段，**必须**同步更新 `src/settings/__tests__/fixtures/snapshot.full.json` 与 `snapshot.vaultless.json`（用 `npx vitest run src/settings/__tests__/settingsSnapshot.golden.test.ts -u` 重新生成后人工确认 diff 只含新增行），禁止用「跳过测试」替代 fixture 更新。
- `npm run typecheck`、`npm run lint` 干净。
- `cargo test` 干净（含 `setup.rs` 快捷键解析用例）。
- **CSP 守卫**：确认 `tauri.conf.json` 的 `img-src` 仍含 `data:`（删掉会让底图白屏）；同时确认 `assetProtocol.scope.allow` 与 `AppDirs::asset_scopes()` **没有**因本规格发生改动。

### 11.3 手动

- **快捷键触发**：该屏变暗进入框选；没有新窗口出现；Orb 不可见，主窗/设置窗保持原状并出现在底图
- **三个入口都不出小球（rev10）**：右键菜单、托盘菜单、全局快捷键各触发多次，从按下到覆盖层铺满的整个过程**任何一帧**都不能看到小球（尤其左上角）；聊天工具栏入口同样。这一项直接对应 F58：Rust 直接发起的入口必须先收到 `screenshot-preparing` 才能撑大窗口
- **取消/成功后无左上角闪帧（rev10）**：取消、ESC、右键退出各做 5 次，Orb 直接从全屏回到原位，不出现「88px 小球停在左上角」的中间帧（不变式 26、F59）
- **耗时基线（rev10）**：dev 构建下从点击到覆盖层可框选，`captured` 阶段应在百毫秒量级而非数秒；日志里 `captured` / `menu-fade` 的占比不再出现「抓图+编码占七成」的形态。若仍明显偏慢，先确认 `[profile.dev] opt-level = 1` 是否生效（`cargo build -v` 看 `-C opt-level`），不要去改链路（§13 风险 19）
- **从 Orb 触发且主窗可见**：底图包含主窗内容，框选主窗区域后产物与所见一致；主窗未被隐藏或移动
- **chat 自隐藏**：截图期间看不到 OrbBall/OrbPanel；结束或取消后恢复
- **草稿保留**：在输入框输入文字后触发截图、取消，文字仍在
- **取消 / 右键 / ESC**：覆盖层消失，Orb 回到原位置/尺寸/可见性/焦点；`attachments/` 无任何新增（全链路不落盘）
- **框选确认**：Orb 展开并聚焦输入框，输入区出现缩略图，未自动发送
- **确认瞬间无闪帧**：框选大区域确认后 Orb 直接展开并显示缩略图，看不到全屏底图被压进 88px 窗口的一帧
- **底图走 data URL**：覆盖层不请求 asset 协议，仓库里没有 `screenshot-tmp` 目录，`tauri.conf.json` 未改
- **Orb 隐藏可靠**：连续触发 10 次，底图中都不出现 Orb 小球（验证可见性校验 + `ShowWindow` 兜底）
- **右键菜单触发**：从 Orb 右键菜单点「截图」、菜单仍处于关闭动画时进入框选，底图与覆盖层上都没有菜单残留；托盘菜单触发同样不残留（验证 `#32768` 清理 + 淡出排空，不变式 23/24）。连续触发 5 次，每次底图菜单区域与周围桌面像素一致（无半透明矩形、无菜单文字的鬼影）；日志中出现 `screenshot: menu fade anchor=…ms period=…ms flushed_frames=… animation=… live_windows=…`，其中 `anchor` 远小于「`SCREENSHOT_MENU_FADE_FRAMES` × `period`」（说明锚定确实落在点击那一刻）、`period` 与该屏刷新率一致（60Hz≈16.7ms、144Hz≈6.9ms）、`flushed_frames` 等于剩余帧数（未撞上限）
- **高刷屏不白等（rev10）**：在 120Hz/144Hz 显示器上从右键菜单触发，点击到覆盖层可框选不应有明显停顿；同一操作在 60Hz 上也应看到动画排空完成、底图无残影（验证按帧排空而非固定毫秒，F60）
- **关闭系统菜单动画**：控制面板关闭「菜单动画」后从右键菜单触发，底图仍无菜单残留，且不出现额外的固定等待（日志 `animation=false`、`flushed_frames=0`）
- **无过渡动画透出**：显影瞬间底图直接铺满（看不到实时桌面透过渐显的底图），取消后 Orb 瞬间回到小球位置（无淡入淡出）；系统「关闭动画」辅助功能开启时行为一致
- **归档即完成**：确认后 `attachments/` 直接出现一个 sha256 命名的文件，无任何中间临时文件
- **发送失败后重试**：重试直接复用已归档 `attachmentId`，不报「附件丢失」
- **移除未发送的截图**：缩略图消失，归档文件随 `removeAttachment` 删除
- **4K 屏整屏框选**：附件不触发 `too_large`，缩略图清晰
- **混合 DPI（125% 副屏）**：选区与最终图片内容一致（归一化换算，不偏移）；覆盖层铺满无黑边
- **JPEG 兜底**：返回 `mediaType = image/jpeg`、`fileName` 以 `.jpg` 结尾，预览正常
- **无 vision 模型**：与粘贴图片一致的 `no_vision` 提示
- **连续触发两次**：第二次被忽略，不叠加会话、不重新抓屏
- **Orb 在副屏 / 副屏负坐标**：覆盖层落在光标所在屏；取消后 Orb 回到原位置（无 DPI 抖动）
- **Orb 点击穿透开启**：覆盖层期间可正常框选；取消后点击穿透仍生效（穿透设置未被永久失效）
- **Orb 原先隐藏 / 展开**：取消后恢复原状态；成功时显示并展开
- **截图期间改设置**：切换「始终置顶」「点击穿透」不打断覆盖层；取消后按快照恢复
- **巡检线程**：截图期间移动/缩放 Orb 不写入 `OrbConf`；取消后位置不变
- **失焦**：截图期间点击其他窗口，Orb 不折叠、覆盖层不消失
- **设置页**：新默认值显示为四键；旧 `Ctrl+Shift+N` 仍可保存；注册失败时出现 `SHORTCUT_WARNING` 提示；恢复默认写入新组合
- **macOS 未授权**：明确错误码与引导文案，不进入框选

---

## 12 里程碑

- **M0（spike）**：§6.4 的窗口模型实验（运行时窗口耗时、WebView2 二次初始化、副屏首帧、`SetWindowDisplayAffinity` 可用性）；产出一份结论，决定保留 Orb 复用还是改专用覆盖层窗口（副屏首帧在 Orb 路线已改为「先 size 后 position」的实测项，见 §8.5.1）
- **M1**：覆盖层宿主（按 M0 结论）+ 会话引导 + 框选 + 裁剪 + 命令内归档 + 快捷键表新增一项 + 默认值调整 + 隐藏校验与全部状态守卫；交付判据 §11.3 全部通过
- **M2**：Orb 右键菜单、托盘菜单、macOS 权限引导（`permission_denied` 标题 + `permission_guide` 系统设置路径说明，不新开引导页）、标注能力；交付判据 设置页可改快捷键、macOS 未授权时有明确引导
- **M3**：多屏拼接框选、实时覆盖层 + 捕获排除（须先解决分层窗口限制）、截图历史、无 vision 时的 OCR 降级、锁屏 gate、原生覆盖层评估；交付判据 与 `pdf_text.rs` 的降级策略一致、锁屏 gate 抽为公共 State

---

## 13 风险

- **1**：Orb 窗口被临时放大后未恢复，或位置被巡检线程写坏 → 快照/恢复精确物理几何；`SCREENSHOT_ACTIVE` 全面守卫；失败路径强制复位
- **2**：全屏放大瞬间露出被拉伸的 chat → 底图 `img.decode()` 完成后才显形；工具栏入口先 `preparing` 隐藏；两个固定 settle 只作兜底
- **3**：失焦触发折叠，覆盖层被缩回 → Rust 跳过 `ORB_BLUR`；前端扩展 `isOrbBlurCollapseSuppressed()`
- **4**：混合 DPI / 跨屏移动导致选区错位 → 前端归一化、Rust 按原图尺寸换算；Orb 先移到目标屏再显示；副屏首帧铺满纳入 M0 spike 与 §11.3
- **5**：macOS 屏幕录制权限（未授权返回黑图；预检通过但实际无显示器）→ 预检 + `monitors()`/`SCShareableContent` 校验 + 引导；参考 Cap
- **6**：4K 产物超 8MB → 长边 2048 + 单次 Fast 编码 + JPEG q85 兜底
- **7**：4K 原图常驻内存约 33MB（另加预览 data URL 约 0.3–1MB）→ 单会话；裁剪/取消立即释放；不做历史缓存
- **8**：连按快捷键/重复 invoke → op lock + `Phase` 状态机；忙时忽略
- **9**：`xcap` 版本/API 不一致 → 锁 0.9.x 并提交 `Cargo.lock`；坐标缺失时用 `placement::get_available_screens` 补齐
- **10**：隐私——锁屏/保险库解锁期间截屏 → V1 明确不做（F30）；V2 抽公共 `ScreenLockState`
- **11**：全局热键抢占其他应用快捷键 → 四键默认 + 注册失败 `SHORTCUT_WARNING` + 设置页可改
- **12**：macOS 全屏 Space 下 Orb 覆盖层不可见 → V1 记录限制；V2 监听 Space 或 `set_visible_on_all_workspaces`
- **13**：显示器热插拔导致几何过期 → 抓屏到裁剪期间配置变化可能错位；V1 日志 + V2 监听取消
- **14**：底图 data URL 经 IPC 的体积（约 0.3–1MB，base64 再 +33%）→ 预览按逻辑分辨率 + JPEG q80；若实测有可感知的 IPC 抖动，改自定义 URI scheme（Kiri 做法），**不要**回退成落盘预览
- **15**：Cap 参考代码为 AGPL-3.0 → 只读参考、不复制；仅采纳时序参数、失败语义与交互尺寸（§14.1、§14.4）
- **16**：主窗/设置窗遮挡用户想截的目标 → 与系统截图工具一致，由用户自行挪开或最小化；V2 可提供「抓屏时隐藏本应用窗口」的可选项
- **17**：捕获排除在分层窗口上不可用 → V2 若要做实时覆盖层，必须先换覆盖层宿主窗口（F54、§6.3、§6.4）
- **18**：归档在裁剪命令内，`AttachmentStore` 写失败时用户已框选却拿不到附件 → 保留令牌允许重选，用 `screenshot.err.archive_failed` 显式提示
- **19**：dev 构建（`opt-level = 0`）下抓屏与编码慢约 6 倍，整条链路可感知延迟达数秒 → `[profile.dev]` 设 `opt-level = 1`（§7.7）；这是构建口径问题，不是链路设计问题，**不要**为它去改链路结构

  实测基线（本机 2520×1680、`cargo test --release` 与 dev 对照，用一次性 bench 测试取得，测试本身已删除）：

  | 阶段 | `opt-level = 0`（dev 默认） | 说明 |
  |---|---|---|
  | `monitors()` | 0ms | 显示器枚举 |
  | `capture()`（`BitBlt` + 格式转换） | 240ms | 抓图本身 |
  | `to_rgb8()`（RGBA→RGB） | 38ms | 编码前转换 |
  | 全分辨率 JPEG q80 | 567ms | 预览编码的主要成本 |

  结论：`captured` 阶段（抓图 + 编码 + base64）占总耗时 74%，菜单淡出排空只占 18%——**瓶颈不在淡出，在编码**。设 `opt-level = 1` 后编码耗时降到几十毫秒量级，与淡出排空同一数量级。若要复测，按 §11.1 的方式临时加 bench 测试，测完删除，不留在主干。
- **20**：`profile.dev` 的 `opt-level` 是**全 workspace** 口径，改动会影响所有 crate 的 dev 增量编译时间 → 取 `1` 不取 `3`；若后续有人再提「dev 太慢」，先看是不是把 `opt-level` 又抬高了，而不是去拆截图链路（§7.7、风险 19）

---

## 14 参考项目对照

### 14.1 Cap（新增，部分快照）

Cap（上游 `CapSoftware/Cap`）是部分检出（提交 `620f65b`，AGPL-3.0；仅阅读，不复制代码）。下列各条中的「决定」只表示是否采纳其做法（语义/参数），不代表取用其实现。

- **`apps/desktop-gpui/src/screenshot.rs`**：可借鉴 隐藏自身覆盖层 → 150ms 合成等待 → 抓屏 → 落盘，>8MP 用 `CompressionType::Fast` 编码，失败语义与「截图不是录制」的边界；决定 **采用**——`SCREENSHOT_HIDE_SETTLE_MS = 150` 作为兜底上限、「隐藏 → 抓屏 → 编码」顺序与失败语义同源；**rev7 追加采用**其 Fast 压缩（落到我们自己的单次编码，§8.4）；失败必须显式报错，不用占位图
- **`apps/desktop-gpui/src/target_overlay.rs`**：可借鉴 每个显示器一个全屏覆盖层、截屏最小选区 1px、手柄抓取区 18px、光标探测 50–80ms；决定 **部分采用**——手柄抓取区取 18px；**不采用**「每显示器一个覆盖层」（V1 单屏复用 Orb）、「最小选区 1px」（改用 `SCREENSHOT_MIN_SELECTION_CSS_PX = 8` 防误触）、光标探测（V1 无悬停窗口高亮）
- **`apps/desktop-gpui/src/overlay_backdrop.rs`**：可借鉴 `WDA_EXCLUDEFROMCAPTURE`（Windows）/ `NSWindowSharingNone`（macOS）把覆盖层排除出截图，40ms settle、3s 过期；决定 **暂缓（V2）**——V1 用「隐藏 `orb` + 可见性校验 + `DwmFlush`」达到同等效果；且捕获排除在 Orb 这种透明分层窗口上会因 `SetWindowDisplayAffinity` 失败（error 8，F54、§14.3）而不可用，所以它不是「到 V2 就能开」的开关
- **`apps/desktop-gpui/src/permissions/screen_capture.rs`**：可借鉴 预检通过后仍用 `SCShareableContent` 验证真实可抓，缓存成功、失败重试、主线程不阻塞；决定 **采用（仅语义）**——macOS 预检 + `monitors()` 实测校验 + 缓存与短间隔重试（§8.7），不搬其代码
- **`apps/desktop-gpui/src/screenshot_crop.rs`**：可借鉴 全分辨率图片解码（5K 约 59MB）与关闭时释放，比例锁定、吸附、8 向手柄；决定 **部分采用**——8 向手柄、会话结束即释放原图；**不采用**比例锁定与边缘吸附（V1 自由框选）
- **`apps/desktop-gpui/src/editor_crop.rs`**：可借鉴 裁剪状态机、命中测试、吸附参考；决定 **部分采用**——裁剪状态机与手柄命中测试；**不采用**吸附
- **`apps/desktop-gpui/kernels/src/screenshot.rs`**：可借鉴 原生抓屏内核；决定 **不采用**——V1 用 `xcap`（§6.1），原生内核列入 V3 评估

关键结论：Cap 新版已经从 Tauri webview 覆盖层迁移到原生 GPUI 覆盖层，说明「每个显示器一个覆盖层」是成熟形态；但它的代价是一整套原生 UI 框架。本规格 V1 复用 Orb webview，只采纳 Cap 的**时序参数、失败语义、压缩参数与交互尺寸**，不采纳其窗口模型与原生内核；V2 再评估原生覆盖层或「捕获排除 + 实时覆盖层」。

rev7 补一句：Cap 能用捕获排除，是因为它走自研捕获后端；我们的 `xcap` 路线与 Kiri 一致（GDI 冻结帧），只能「隐藏 + 合成校验」，这也是 §6.3 那条硬限制的由来。

### 14.2 既有参考

- **OpenBitFun · 原生捕获**：`windows_wgc_capture.rs`、`macos_capture.rs`、`AGENTS.md`，用于原生捕获的失败语义与平台差异；决定 **采用（仅语义）**——平台差异清单与错误分类，不采用其 WGC/BitBlt 实现
- **OpenBitFun · 图片上下文**：`imagePayload.ts` / `imageUtils.ts`，截图作为「图片上下文」的前端结构；决定 **不采用**——本仓库已有归档式附件管道（F10–F13），不需要第二套图片上下文结构
- **airi**：`services/computer-use-mcp/src/utils/screenshot.ts`，截图压缩、尺寸上限、失败兜底；决定 **不采用**——其抓屏依赖外部二进制、失败落占位 PNG；本规格要求失败显式报错，压缩在裁剪命令内一次编码完成 + JPEG 兜底（§8.4）
- **openhuman**：`capabilities/default.json` 的 `screen_intelligence_capture_now`，能力命名与权限声明；决定 **不采用**——V1 不新增窗口与能力（F3），命名仅作记录

第三方开源截图工具（Flameshot / ShareX，均为 GPL-3）只可参考交互习惯，**不可复制代码**。

### 14.3 Kiri / shiping（新增，完整克隆）

Kiri 与 shiping 需从本地参考检出中取得（`references/` 已被 `.gitignore` 忽略，不参与构建与分发）。

- **Kiri · 捕获错误分类**（MIT，可复制）：`src-tauri/src/capture/mod.rs` 的 `CaptureHealth`（`expected_stop` / `unexpected_failure` 边界）与 `VIDEO_FRAME_QUEUE_CAPACITY = 2` 有界帧队列，以及 `capture/{windows,macos,linux}.rs` 三平台实现；用于 Rust 抓屏错误分类与平台差异；决定 **部分采用**——借鉴「预期停止 / 非预期失败」的错误分类语义（写进 `screenshot.err.*`）；**不采用**有界帧队列与三平台自研捕获（V1 是单张静态抓屏 + `xcap`，无帧流）
- **Kiri · 冻结抓屏后端**（MIT）：`capture/windows.rs` 用 `xcap::Monitor::capture_image()` 的 GDI 路径抓冻结静帧，`encode_frozen_capture_png` 固定 `CompressionType::Fast + FilterType::Sub`；决定 **采用**——`xcap` 锁 0.9.x 的现网依据（Kiri 0.9.8）；Fast + Sub 用于我们的最终编码（§8.4）
- **Kiri · 冻结图不落盘**（MIT）：`docs/architecture.md`「The image is not written to disk」，整张冻结图常驻内存，overlay 通过 `kiri://` 自定义 scheme + blob URL 获取；决定 **采用（语义）**——底图不写盘、由内存生成；我们用降采样 JPEG 的 data URL 替代自定义 scheme，因为底图只用于框选参照
- **Kiri · 窗口隐藏时序**（MIT）：`docs/adr/0107-windows-capture-visibility-and-pin-drag.md`——固定延时不够（DWM 过渡会被冻进截图），Tao 的 `set_visible` 只比标志位可能空操作，必须「可见性查询确认 → 残留 HWND 补 `ShowWindow(SW_HIDE)` → `DwmFlush`」；决定 **采用**（不变式 22、§8.2），把 rev6 的「150ms + `IsWindowVisible`」升级为这套校验
- **Kiri · 第二 WebView2 控制器风险**（MIT）：`docs/windows-capture-incident.md` 记录在主线程回调里创建第二个 WebView2 控制器会 `AppHangB1`，最终把启动放到专用线程；决定 **采用（作为 §6.4 spike 的必测项）**——这也是「复用 Orb 不新增 webview」的实证支持
- **Kiri · 分层窗口与捕获排除**（MIT）：`docs/qa/scrolling-capture.md` 里 `SetWindowDisplayAffinity` 对带 `WS_EX_LAYERED` 的窗口返回 error 8（affinity 失败），只能退化为「隐藏控件」；决定 **采用（作为限制记录）**——F54、§6.3
- **shiping · 抓屏流程**（**CC BY-NC 4.0，禁止商用**）：`src/main/take-screenshot.ts`（首抓空帧重试、按 DIP 尺寸 resize 后再编码）、`src/shared/capture-region.ts`（`RegionRect` / `regionToPixels`）；只读借鉴流程与几何换算，**不得复制代码**；决定 **部分采用**——空帧重试一次（§8.2）；**不采用**按 DIP 尺寸 resize 后编码（V1 按原图像素裁剪，避免二次缩放损失）
- **shiping · 底图传渲染层**（CC BY-NC 4.0）：`src/renderer/src/region-picker/index.tsx` 直接用 `data:image/jpeg;base64,...` 作为框选底图；决定 **采用（语义）**——底图走 data URL 是成熟做法（§7.2、不变式 18）
- **shiping · 窗口隐身与框选**（CC BY-NC 4.0）：`src/main/window-privacy.ts`（`setContentProtection` + Windows 软隐藏后重置）、`src/main/region-picker.ts`（框选窗口生命周期）；决定 **暂缓（V2）**——`setContentProtection` 属「捕获排除」路线（§6.3），同样受分层窗口限制；框选窗口生命周期**不采用**（覆盖层复用 Orb，无独立 picker 窗口）

与 rev4 的关系：Kiri ADR 0107 证明「隐藏窗口后不能只等固定延时，必须确认窗口真的不可见并 flush DWM」，这是 rev7 把 `SCREENSHOT_HIDE_SETTLE_MS` 降级为兜底上限的依据；shiping 的 `take-screenshot.ts` 说明「先抓空帧重试一次」是必要兜底；Kiri 的内存底图与 Fast 编码说明 rev7 的「不落盘 + 单次编码」是成熟做法而非省事。

### 14.4 采纳结论一览

- **采用**：Cap 的「隐藏 → 抓屏 → 编码」时序与失败语义、>8MP 的 Fast 压缩、macOS 权限预检 + 实测校验语义、手柄抓取区 18px；Kiri ADR 0107 的「可见性确认 → `ShowWindow(SW_HIDE)` → `DwmFlush`」三件套；Kiri 的「冻结图不落盘 + 内存供图」、`xcap` 0.9.x 的 GDI 路径与 `Fast + Sub` 编码；shiping 的「底图以 data URL 传给渲染层」与「首抓空帧重试一次」
- **部分采用**：Cap 的 8 向手柄与「会话结束即释放原图」、裁剪状态机与命中测试；Kiri 的「预期停止 / 非预期失败」错误分类
- **暂缓（V2）**：Cap 的 `WDA_EXCLUDEFROMCAPTURE` / `NSWindowSharingNone` 捕获排除（受分层窗口 error 8 限制，§6.3）；shiping 的 `setContentProtection`
- **不采用**：Cap 的「每显示器一个覆盖层」「最小选区 1px」「光标探测 / 悬停窗口高亮」「比例锁定与吸附」「原生 GPUI 抓屏内核」；OpenBitFun 的 WGC/BitBlt 实现与 `imagePayload` 图片上下文结构；airi 的外部二进制抓屏 + 占位 PNG 兜底；openhuman 的能力声明命名；shiping 的按 DIP resize 后编码与独立 picker 窗口
- **作为约束记录**：Kiri 的第二个 WebView2 控制器 `AppHangB1`（§6.4 spike 必测项）、Kiri 的分层窗口 `SetWindowDisplayAffinity` error 8（F54）

> 判定口径：只借鉴**行为语义、时序参数与交互尺寸**，不搬实现；与 V1 的「复用 Orb（待 M0 spike 定案）+ `xcap` + 静态冻结 + 内存底图」冲突的做法一律不采用，需要额外平台 API 的一律进 V2。

---

## 15 rev3 修订摘要

- **§1**：rev2 新增 `screenshot` webview + 独立覆盖层 → rev3 复用 Orb webview、不新增窗口
- **§4/§5**：rev2 需为 F2/F3 加 viewMap/capabilities → rev3 不新增 label，F2/F3 不再需要改动
- **§7.2**：rev2 5 个 DTO、57 → 62 → rev3 3 个 DTO（会话/裁剪参数/裁剪结果）；「57 → 60」是当时的估算，实际 `protocol_bindings.rs` 无数量断言，见 rev5
- **§7.3**：rev2 `start_screenshot` 返回会话、`get_screenshot_session` 为常规路径 → rev3 `start_screenshot` 返回 `()` + `screenshot-session` 事件、`get_screenshot_session` 仅兜底
- **§7.4**：rev2 `screenshot-captured` + `screenshot-cancelled` → rev3 `screenshot-session` + `screenshot-error`；结果走 invoke 返回值
- **§7.5**：rev2 默认 `Ctrl+Shift+A`、固定数组注册 → rev3 四键默认；`SHORTCUT_SPECS` 元组表 + 上游 crate 纯函数解析 + 清空/重复/非法诊断
- **§8**：rev2 覆盖窗口构建/关闭 → rev3 Orb 全屏/恢复快照 + 巡检/失焦/穿透守卫
- **§9**：rev2 新布局、跨窗口事件、`useScreenshotResult` → rev3 `OrbWindow` 条件渲染、`screenshotStore`、直接 ingest、chat 只隐藏不卸载
- **§9.7**：rev2 无冲突检测边界说明 → rev3 明确只检测内部重复；外部冲突以 OS 注册失败为准，不维护静态表
- **§10**：rev2 新窗口文件 → rev3 移除 `ScreenshotWindow`，新增 `orb_overlay.rs` / `screenshotStore.ts` / `useScreenshotSession.ts`
- **§14**：rev2 无截屏参考项目 → rev3 新增 Cap 部分快照

---

## 16 rev4 修订摘要

- **§1**：rev3 抓屏时隐藏所有应用窗口 → rev4 只隐藏覆盖层宿主 `orb`
- **§1.1**：rev3 无 → rev4 新增一行修正记录（rev3 底图与所见不符）
- **§2 目标 5**：rev3 恢复所有窗口的可见性/焦点 → rev4 只恢复 Orb；`main` / `settings` 全程不受影响
- **§4 F27**：rev3 主窗口作为被隐藏/恢复对象 → rev4 主窗口不参与覆盖层、不被隐藏
- **§5 不变式 17**：rev3 抓屏前隐藏所有应用窗口 → rev4 只隐藏 `orb`；`main` / `settings` 进入底图
- **§6.3**：rev3 先隐藏所有窗口 → 抓屏 → rev4 先隐藏 `orb` → 抓屏；底图 = 用户所见
- **§7.1**：rev3 常量注释「隐藏所有应用窗口」 → rev4 注释改为「隐藏 `orb`」，值仍 150ms
- **§8.2**：rev3 隐藏 main / orb / settings → rev4 只快照并隐藏 `orb`；补 Windows 可见性校验与空帧重试
- **§11.3**：rev3 期望三窗均不可见 → rev4 主窗/设置窗保持原状并进入底图；新增「从 Orb 触发」用例
- **§13**：rev3 无 → rev4 新增主窗遮挡风险（与系统截图工具行为一致）
- **§14**：rev3 仅 Cap → rev4 新增 §14.3 Kiri / shiping 对照（含许可证约束）

---

## 17 rev5 修订摘要

rev5 的定位是**对齐当前代码库**，把 rev4 里过期的事实、契约与流程改成可直接执行的形式。

- **修订头**：rev4 无 → rev5 记录 rev5 目标与核对基准日期
- **§4 既有事实**：rev4 附件直通 `file_path`、scope `attachments/**`、DTO 数量断言 → rev5 附件链路改为归档模式（F10–F14）；scope 为单层 `attachments/*`（F19）；无数量断言（F28）；目录由 `AppDirs::screenshot_tmp()` 提供（F34）；新增 F45–F49（`ORB_CLICK_THROUGH` 镜像、`apply_orb_window_attrs`、`suppressOrbBlurCollapse`、`resolve_external_input`、`Manager::cursor_position()`）
- **§5 不变式**：rev4 17 条 → rev5 20 条，新增「穿透只经 `apply_orb_click_through`」「几何单次 `set_bounds`」「scope 只追加子目录」
- **§7.1 常量**：rev4 前端/后端分散 → rev5 后端常量 5 个，`SCREENSHOT_MIN_SELECTION_CSS_PX` 归前端 `src/config/constants.ts`
- **§7.2 DTO**：rev4 含 `file_path` 直通字段 → rev5 结果只返回 `path`（待消费凭据）+ `bytes` + 尺寸 + `mediaType`（信息性，最终以 `import_attachment` 返回值为准）
- **§7.3 命令**：rev4 `crop_screenshot` 结果可直接用 → rev5 返回路径是「待消费凭据」，前端必须 `await ingestPath`，失败才 `release_screenshot`
- **§7.5 快捷键**：rev4 前端同步改默认值 → rev5 默认值唯一来源为 Rust `DEFAULT_SHORTCUT_PATTERNS`，前端用 `contract:gen` 生成的 `shortcutDefaults.generated.ts`
- **§7.6 scope**：rev4 未单列 → rev5 新增一节：只追加 `$APPDATA/attachments/screenshot-tmp/*`，不放宽全局
- **§8.3 抓屏**：rev4 `set_position` + `set_size` → rev5 优先 `app.cursor_position()`；几何一次 `set_bounds`；`apply_orb_click_through` 同步 atomic
- **§8.4 裁剪**：rev4 裁剪后删产物 → rev5 产物保留，交给 `import_attachment` 消费
- **§8.5 恢复**：rev4 直接 `set_ignore_cursor_events` → rev5 走 `apply_orb_click_through`；新增 §8.5.1 `set_bounds` 回退方案
- **§8.6 守卫**：rev4 缺 `apply_orb_window_attrs` → rev5 补 `apply_orb_window_attrs` 守卫；前端改为扩展 `isOrbBlurCollapseSuppressed()`
- **§8.8 清理**：rev4 `admit_attachments` 删源 → rev5 `import_attachment` 归档成功后删源；`ingestPath` 失败时前端 release；TTL 兜底
- **§9.5 接入**：rev4 抽出 `ingestPathToStore`、移除附件时 release → rev5 复用现有 `ingestPath`；`createPathAttachment` 加可选 `meta`；移除附件无需额外处理
- **§9.6/§10.2**：rev4 `ChatView.tsx`、`OrbPanel.tsx`、`assistant-agent` → rev5 改为 `ConversationView.tsx` / `ConversationFrame.tsx` / `OrbExpand.ts`；删掉 `assistant-agent` 改动
- **§11**：rev4 DTO 断言 60、发送后清理 → rev5 无数量断言；验收改为「归档即时消费」「发送失败重试复用 `attachmentId`」

---

## 18 rev6 修订摘要

rev6 的定位是**给参考项目对照补决策**，不改任何契约、流程与实现约束。

- **§14.1 Cap**：rev5 只列「可借鉴内容」 → rev6 每行补「决定」列——明确采用（时序 150ms、失败语义、权限校验语义、手柄 18px）、部分采用（8 向手柄、释放时机、裁剪状态机）、暂缓（捕获排除）、不采用（每显示器一个覆盖层、最小选区 1px、光标探测、比例锁定与吸附、原生内核）
- **§14.2 既有参考**：rev5 只列「可参考内容 / 用途」 → rev6 每行补「决定」列——OpenBitFun 仅采用失败语义、不采用其捕获实现与图片上下文结构；airi / openhuman 均不采用
- **§14.3 Kiri / shiping**：rev5 只列「可借鉴内容 / 用途」 → rev6 每行补「决定」列——Kiri 采用 ADR 0107 的 Windows 可见性校验、部分采用错误分类；shiping 部分采用空帧重试、捕获排除暂缓、picker 窗口与 DIP resize 不采用
- **§14.4**：rev5 无 → rev6 新增「采纳结论一览」，按 采用 / 部分采用 / 暂缓 / 不采用 四类汇总，并给出判定口径

---

## 19 rev7 修订摘要

rev7 的定位是**把链路里的文件系统与重复编码删掉**，并用参考实现的实证补两处可靠性缺口。

- **§1/§2/§4**：新增 F50–F54（CSP 允许 `data:`、`import_attachment` 只做大小校验 + 内容寻址、归一化发生在发送时、Tao 的 `set_visible` 只比标志位、Orb 是分层窗口）；F19/F21/F31/F32/F34/F48 的「影响」改为「rev7 不再触及」
- **§5 不变式**：1（产物直接归档、不含路径）、6（不产生临时文件）、12（`media_type` 与 `file_name` 一致）、18（底图只走内存 data URL）重写；新增 21（恢复几何前必须卸载覆盖层）、22（隐藏必须可见性校验 + `ShowWindow` + `DwmFlush`）
- **§6.1**：`xcap` 锁定 0.9.x，依据是 Kiri 0.9.8 在 Windows 上的 GDI 冻结路径
- **§6.3/§6.4**：新增「Orb 是分层窗口，捕获排除会 `SetWindowDisplayAffinity` error 8」；新增落地前置 spike（窗口模型、WebView2 二次初始化、副屏首帧、exclusion 可用性），窗口选型改为 spike 后再定
- **§7.1**：删除 `SCREENSHOT_TMP_TTL_MS`、`SCREENSHOT_TMP` 目录名与 `AppDirs::screenshot_tmp()`、`asset_scopes()` 追加；新增两个预览参数；两个 settle 值降级为兜底上限
- **§7.2 DTO**：`ScreenshotSessionDto.path` → `preview`（data URL）+ `previewWidth/Height`；`ScreenshotCropResultDto.path` → `attachmentId` + `fileName` + `mediaType` + `bytes` + 尺寸
- **§7.3 命令**：6 条 → 5 条，删除 `release_screenshot`（不再有任何命令接受/返回磁盘路径）；`crop_screenshot` 在命令内完成归档；新增错误码 `screenshot.err.archive_failed`
- **§7.4/§7.6/§7.7**：会话事件携带 data URL；asset scope 与 capabilities 都不改（CSP 保留 `data:` 是唯一依赖）；依赖写为 `xcap = "0.9"`
- **§8.1/§8.2**：状态去掉 `preview_path`，改存 `preview`；抓屏流程改为「隐藏 → 可见性校验/`ShowWindow`/`DwmFlush` → 抓屏 → JPEG 预览 → data URL → 事件 → 全屏」
- **§8.4**：裁剪 → resize ≤2048 → 一次 Fast 编码 → `archive_bytes`；不再调用 `normalize_image`；明确「只编码一次」与「前端先卸载覆盖层」
- **§8.8**：清理章节从「临时文件生命周期」改为「生命周期与清理」，`AttachmentStore` 不新增任何函数
- **§9.2/§9.4/§9.5**：`screenshotStore` 增加 `finishing`；`Overlay` 用 `img.decode()` 揭示、确认/取消先 `beginFinishing()`；结果接入改为 `attachmentFromImport` + `addAttachment`，不再用 `ingestPath`/`release`
- **§10/§11/§12/§13**：文件清单按新链路重写（明确 `tauri.conf.json`、`app_dirs`、`attachment_store`、`intake.ts` 不改）；验收新增「确认瞬间无闪帧」「底图走 data URL」「Orb 隐藏可靠」「CSP 守卫」；里程碑增加 M0 spike；风险 14/17/18 重写
- **§14**：Cap 追加采用 Fast 压缩、捕获排除暂缓理由改为分层窗口限制；Kiri 追加「冻结图不落盘」「GDI 后端与 Fast + Sub」「第二 WebView2 控制器 AppHangB1」「分层窗口 affinity error 8」；shiping 追加「底图 data URL」；§14.4 汇总同步

---

## 20 rev8 修订摘要

rev8 的定位是**把 M2 剩余项与实现细节对齐到真实代码**：几何写入的库能力结论、macOS 权限前置校验、以及 `orb_overlay` 的可测性补测。

- **§8.5.1**：`set_bounds` 回退方案从「待实测」改为「已定稿」——核对 Tauri 2.12.5 源码，`set_bounds` 只存在于 `Webview`，`WebviewWindow` 没有该方法；覆盖层写入与恢复统一为「先 `set_size` 后 `set_position`」，恢复写回快照值
- **§5 不变式 20**：同步改为「回退序列写入与恢复」，不再要求单次 `set_bounds`
- **§6.2 / §8.3 步骤 10**：几何描述同步为回退序列，避免与 §8.5.1 冲突
- **§8.7**：macOS 权限前置校验补 `CGPreflightScreenCaptureAccess` 预检 + `CaptureBackend::monitors()` 实测，成功缓存、失败按 3 秒间隔重试；Windows / Linux 仍恒真。该分支只能在 macOS 构建上验证，已标注为跨平台验收项
- **§8.9**：`orb_overlay` 测试缝由 `FakeWindowOps` 改为 `FakeOps`（实现里对应 `WindowOps` trait 的假实现），并明确断言项：隐藏序列、`ShowWindow` 兜底、兜底失败返回 `capture_failed`、快照读全字段、还原按快照写回、任一步失败仍走完并返回 `cancel_failed`
- **§9.9 / 前端**：`screenshot.err.permission_denied` 的 toast 描述改为 `screenshot.permission_guide` 引导文案（`zh-CN` / `en-US` 均已补键），未知错误码仍原样回显
- **§8.2 / §5 不变式 23 / F55**：隐藏前新增「Windows 弹出菜单清理」——弹出菜单（`#32768`）是独立顶层窗口，Orb 只是 owner，`ShowWindow(orb)` 带不走它；从右键菜单项触发截图时菜单可能仍在关闭动画而被冻进底图，改为枚举本进程可见菜单窗口、逐个禁用 DWM 过渡后 `SW_HIDE`，预算内仍可见返回 `capture_failed`
- **§11.3**：新增「右键菜单触发」验收项（Orb 右键菜单与托盘菜单均不残留）
- **M2 交付**：Orb 右键菜单、托盘菜单、macOS 未授权引导已落地；标注能力尚未实现，仍是 M2 唯一未完成项
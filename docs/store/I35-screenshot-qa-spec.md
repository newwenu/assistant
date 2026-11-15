# I35 — 截屏提问规格

| 项 | 值 |
|---|---|
| 状态 | Draft（rev4） |
| 日期 | 2026-11-03（rev4：2026-11-08） |
| 修订 | rev4：抓屏只隐藏覆盖层宿主 `orb`，`main` 不再隐藏，底图与用户触发瞬间所见一致；rev3：取消新增 webview，改为复用 Orb 窗口做全屏覆盖层；chat 自隐藏且保留草稿；快捷键注册改为表驱动并更换默认组合；补充 Cap 参考项目 |
| 上游调研 | 本仓库代码核对 + OpenBitFun、airi、Cap、Kiri、shiping 五个参考项目对照 |
| 关联规格 | I2e-multimodal-attachments-spec（已归档，附件链路）、I4-spec（已归档，全局快捷键） |
| 影响范围 | 复用 Orb 窗口（**不新增 webview**）、Rust 模块 `screenshot`、6 条命令、3 个 DTO、2 个事件、快捷键注册重构与默认值调整、临时目录与清理、i18n、设置提示；`assistant-storage` / `assistant-agent` 各一处受控清理；`Cargo.toml` 新增 `xcap` 与直接 `image` 依赖 |

本文是落地规格。所有「既有事实」均已对照源码核对，标注了文件位置；**未核对的第三方库 API 单独标注为「落地时核实」**，不作为契约。

---

## 1 结论摘要

截屏提问 = **抓屏 → 框选 → 变成一条带图片的用户消息**。三段里第三段在仓库中已经完整存在。

**rev3 的核心决定：不再新增 `screenshot` webview，复用现有 Orb 窗口做全屏覆盖层。**

| 环节 | 现状 | 本规格的改动 |
|---|---|---|
| 抓屏 | 无 | 新增 `src-tauri/src/screenshot/`：`xcap` 后端 + 可注入 `CaptureBackend`；抓屏时只隐藏覆盖层宿主 `orb` |
| 框选 | 无 | 把 Orb 窗口临时放大到光标所在显示器，前端在同一个 webview 里渲染覆盖层；chat/Orb 面板隐藏但保持挂载 |
| 图片进消息 | 已存在，支持 `file_path` 直通 | 裁剪结果由 orb 直接 `ingestPathToStore`；不再需要跨窗口结果事件 |

为什么复用 Orb 而不是新开窗口：

- **不新增 webview**：少一个 WebView2/WKWebView 实例、少一份 JS 运行上下文、少一套 `viewMap` / `capabilities` / 布局入口；这是 rev2 的主要重量来源。
- **结果直通**：裁剪在 Orb 的同一 JS 上下文里完成，直接写入 `attachmentStore`，不需要 `screenshot-captured` 事件、跨 webview 状态同步或 `get_screenshot_session` 的常规路径（只保留兜底）。
- **chat 自隐藏**：`OrbWindow` 是唯一的覆盖层宿主；截图期间用 CSS 隐藏 `OrbBall`/`OrbPanel`，`ChatView` **保持挂载**，因此输入草稿、附件列表和流式状态都不丢。
- **只隐藏宿主**：抓屏前只隐藏 `orb` 一个窗口，`main` / `settings` 保持用户原状并进入底图，因此「用户触发瞬间所见 = 冻结底图」，也不需要为它们维护可见性快照。
- **代价**：Orb 窗口的几何、可见性、焦点、置顶、点击穿透需要快照/恢复；`spawn_screen_inspection` 与 `ORB_BLUR` 必须暂停，否则会把全屏几何写回 `OrbConf` 或触发折叠。

本规格**不改动** `assistant-agent` 的附件准入判断与上限、`assistant-storage` 的附件存储格式、`contentBlockUtils` 的序列化、Provider 的请求投影。后端只增加一处受控清理：**准入成功后删除 `attachments/screenshot-tmp/` 下的源文件**（不改变任何准入结果）。

### 1.1 rev3 / rev4 关键修正（实现前必读）

| rev2 的问题 | rev3 处理 |
|---|---|
| 新开 `screenshot` webview，重量大 | 复用 Orb 窗口；不新增窗口、不加 `capabilities`、不改 `viewMap` |
| 结果需要 `screenshot-captured` 事件 + 跨窗口订阅 | 裁剪结果由 orb 直接 ingest；只保留 `screenshot-session` 启动事件与 `screenshot-error` 失败通知 |
| 覆盖层拿不到会话 | orb 在截图前调用 `start_screenshot`；会话通过 `screenshot-session` 事件下发，`get_screenshot_session` 兜底 |
| 截图时 chat 是否隐藏未定义 | 新增 `screenshotStore`；`OrbWindow` 用 CSS 隐藏 `OrbBall`/`.orb-panel`，`ChatView` 不卸载，草稿保留 |
| Orb 的几何/位置可能被巡检线程写坏 | 截图期间暂停 `spawn_screen_inspection`、`snap_orb_to_*`、`save_orb_position_sync`；恢复后写回原几何 |
| 失焦会把 Orb 折叠 | Rust 在截图期间跳过 `ORB_BLUR`；`OrbPanel` 的 blur 处理再加一层 store 判断 |
| 前端 `toPhysical` 与 Rust 重复换算 | 前端只发归一化选区（0..1），Rust 按原图尺寸换算一次；跨 DPI 不再依赖 `scale_factor` |
| rev3 抓屏隐藏全部应用窗口，底图与用户所见不符（从 Orb 触发时截不到 main） | rev4：只隐藏覆盖层宿主 `orb`；`main` / `settings` 保持原状并进入底图 |
| 契约 5 个 DTO / 57 → 62 | rev3 只需 3 个 DTO（会话、裁剪参数、裁剪结果），57 → 60 |
| 快捷键注册写死数组 + `match` | 改为 `SHORTCUT_SPECS` 元组表 + 上游 crate 纯函数解析；空配置=禁用、重复/非法绑定统一告警 |
| 默认 `Ctrl+Shift+N` 冲突多 | 三条默认改为四键组合（见 §7.5）；前端同步默认值，外部冲突以注册失败为准 |

---

## 2 目标

1. 全局快捷键一次触发，光标所在显示器整屏变暗并进入框选。
2. 框选结果以图片附件形式进入 Orb 输入区，用户补一句话即可发送；不自动发送。
3. 截图附件与「粘贴 / 文件选择 / 拖拽」走完全相同的准入、上限、预览、存储路径。
4. 取消（ESC / 右键 / 关闭）后不留下任何临时文件，不残留覆盖层；成功发送后临时源文件也被消费删除。
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

| # | 事实 | 位置 | 对本方案的影响 |
|---|---|---|---|
| F1 | `tauri.conf.json` 的 `app.windows` 静态声明 `main` 与 `orb`；`settings` 是运行时创建 | `src-tauri/tauri.conf.json`、`src-tauri/src/window.rs` `WINDOW_SPECS` | rev3 不新增窗口；`screenshot` 覆盖层直接复用 `orb` |
| F2 | 前端按 webview label 选布局，未匹配 label 回退 `MainLayout` | `src/App.tsx` `viewMap` | 不新增 label，因此不需要改 `viewMap` |
| F3 | `capabilities/default.json` 的 `windows` 只有 `main` / `orb` / `settings` | `src-tauri/capabilities/default.json` | 不新增窗口，因此不需要改 capabilities |
| F4 | 全局快捷键在 `register_global_shortcuts()` 中由**定长数组**驱动，且 `match id` 重复分派 | `src-tauri/src/setup.rs` | rev3 改为 `SHORTCUT_SPECS` 元组表 + 上游 crate 纯函数解析 |
| F5 | 快捷键字符串解析支持 `Meta/Super/Cmd/Win/Windows`、`Ctrl/Control`、`Shift`、`Alt/Option`；单字母回退 `KeyX`，数字回退 `DigitX` | `src-tauri/src/setup.rs` `parse_shortcut` | 四键组合可直接用，无需改解析器 |
| F6 | 快捷键配置变更会触发重新注册（`keybindings` 字段比对） | `src-tauri/src/cmd/config.rs` `APP_SIDE_EFFECTS` | 新增/修改快捷键自动获得「设置页改键即时生效」 |
| F7 | 前端快捷键定义表含 `id/group/label/defaultBinding/kind`，`DEFAULT_KEYBINDINGS` 由该表派生 | `src/settings/core/types/shortcutTypes.ts` | 默认值必须与 Rust `default_keybindings()` 同步 |
| F8 | `AppConf::new()` 原本硬编码三条 `Meta+...` 默认值 | `src-tauri/crates/assistant-config/src/app_conf.rs` | rev3 抽出 `default_keybindings()`，Rust 注册与配置默认值共用 |
| F9 | 设置页快照 fixture 只投影字段 key，不含默认值 | `src/settings/__tests__/fixtures/snapshot.full.json`、`snapshot.vaultless.json` | 改默认值本身不影响 fixture；新增快捷键会新增一行字段 |
| F10 | 附件模型 `ImageAttachment` 允许「无 `file` 字节、只有 `filePath`」；路径型附件的 `file` 是空 `File` | `src/chat/attachment/types.ts`、`factory.ts` | 截图可以只传路径 |
| F11 | `createImageAttachmentFromPath()` 用 `convertFileSrc(filePath)` 作预览 URL，按扩展名推断 MIME，且 `originalSizeBytes = 0` | `src/chat/attachment/factory.ts` | 截图扩展名必须跟随真实编码；建议接收 `mediaType/bytes/尺寸` |
| F12 | `useAttachmentIntake()` 内部有 `ingestPath()`，但未导出；拖拽订阅也在这个 hook 内 | `src/chat/attachment/intake.ts` | 不能复制 hook；抽出独立 `ingestPathToStore()` |
| F13 | `buildContentBlocksAsync` 对无 `attachmentId` 但有 `filePath` 的图片生成 `{ type: "image", file_path }` | `src/chat/core/contentBlockUtils.ts` | 截图天然走 `file_path` 通道 |
| F14 | 后端 `admit_attachments` 支持 `file_path`，读盘后写 `AttachmentStore` | `src-tauri/crates/assistant-agent/src/multimodal.rs` | 准入判断零改动；只增加成功后删除 screenshot-tmp 源文件 |
| F15 | `AttachmentStore::save_attachment` 按 sha256 内容寻址、同名即跳过 | `src-tauri/crates/assistant-storage/src/attachment_store.rs` | 同一张截图重复发送不会产生副本 |
| F16 | 前端 `admitAttachment` 只校验种类数量（图片 4 张）；单张 8MB 仅在 `validateImageFile` 对 `File` 生效；路径型附件不经过尺寸校验 | `src/chat/attachment/admit.ts`、`factory.ts` | 截图产物必须由 Rust 保证 ≤ 8MB |
| F17 | `normalize_image()` 已实现：真实格式校验、长边 2048、PNG/JPEG 重编码、BMP/TIFF 转 PNG、GIF/WebP 透传；不会把 PNG 转 JPEG | `src-tauri/crates/assistant-agent/src/normalization.rs` | 可直接复用；体积兜底另写 JPEG 重编码 |
| F18 | `image` 依赖只启用 `png/jpeg/bmp/tiff` feature | `src-tauri/Cargo.toml` | 输出只能是 PNG 或 JPEG |
| F19 | asset protocol scope 为 `$APPDATA/mascot-assets/**` 与 `$APPDATA/attachments/**`；CSP `img-src` 含 `asset:` | `src-tauri/tauri.conf.json` | 截图临时文件必须在 `attachments` 目录树内 |
| F20 | `opener:allow-open-path` 的 scope 是 `$APPDATA/attachments/*`（单层） | `src-tauri/capabilities/default.json` | 不要依赖 `openPath` 打开截图 |
| F21 | `clean_tmp()` 只扫 `attachments` 根目录的 `*.tmp`，不递归子目录 | `assistant-storage/src/attachment_store.rs` | 必须用独立 `screenshot-tmp` 子目录 + 独立 sweep |
| F22 | 显示器枚举返回物理像素 bounds | `src-tauri/crates/assistant-orb/src/placement.rs` `get_available_screens` | 抓屏、Orb 全屏定位、裁剪统一以物理像素为准 |
| F23 | `get_screen_size(center_x, center_y, scale_factor)` 按「中心点最近」选显示器，但它是 Tauri command | `src-tauri/src/cmd/orb.rs` | 抽出 `placement::nearest_screen` 纯函数供抓屏复用 |
| F24 | `get_mouse_position()` 用 `mouse_position` crate；现有用法按物理坐标处理 | `src-tauri/src/cmd/orb.rs` | 坐标空间落地时核实；`None` 回退 Orb 中心/主屏 |
| F25 | Orb 展开/收起由 Rust 重算窗口尺寸与位置；`hide()` 无过渡回调 | `src-tauri/src/window.rs` | 抓屏前隐藏窗口必须自行 delay |
| F26 | Orb 右键菜单项在 Rust 端构造；菜单点击统一在 `menu.rs` 分发 | `src-tauri/src/window.rs`、`src-tauri/src/menu.rs` | 新增菜单项要同时改这两处 |
| F27 | `ChatView` 只被 `OrbPanel` 使用，主窗口没有聊天输入框 | `src/orb/OrbPanel.tsx` | 结果固定回写 Orb；主窗口不参与覆盖层，也不被隐藏（rev4） |
| F28 | 契约测试硬断言 57 个 DTO | `src-tauri/tests/protocol_bindings.rs` | rev3 新增 3 个 DTO，断言改为 **60** |
| F29 | `MessageInput` 没有暴露 focus 句柄 | `src/chat/message/MessageInput.tsx` | 需要补 ref 或焦点事件 |
| F30 | `ScreenLockMonitor` 只在 `#[cfg(feature = "vault")]` 的 vault 模块内构造，未作为全局 State 管理 | `src-tauri/src/cmd/mod.rs`、`src-tauri/src/cmd/vault/mod.rs` | V1 无法直接复用锁屏 gate |
| F31 | `AttachmentStore::list_all` 与存储统计只扫根目录，不递归 | `assistant-storage/src/attachment_store.rs`、`src-tauri/src/cmd/storage.rs` | `screenshot-tmp/` 不会计入用户附件统计 |
| F32 | `tauri` 已启用 `protocol-asset` feature，`convertFileSrc` 可用 | `src-tauri/Cargo.toml` | 预览无需新增 feature |
| F33 | 应用已依赖 `uuid` v4 | `src-tauri/Cargo.toml` | 令牌生成不需要新增随机数依赖 |
| F34 | `AttachmentStore` 未暴露根目录访问器 | `assistant-storage/src/attachment_store.rs` | 需要新增 `dir()/screenshot_tmp_dir()` 等只读方法 |
| F35 | Orb 窗口是透明、无边框、置顶、`skipTaskbar`、不可缩放；`expand/collapse_orb_panel` 会改尺寸与位置 | `src-tauri/tauri.conf.json`、`src-tauri/src/window.rs` | 可被临时放大成全屏覆盖层；恢复时要写回原物理几何 |
| F36 | `spawn_screen_inspection` 每 800ms 把 Orb 吸附到最近屏幕并写回 `OrbConf.position_x/y` | `src-tauri/src/setup.rs` | 截图期间必须暂停，否则会把全屏几何持久化 |
| F37 | `window::on_window_event` 在 Orb `Focused(false)` 时发 `ORB_BLUR`；`OrbPanel` 200ms 后折叠 | `src-tauri/src/window.rs`、`src/orb/OrbPanel.tsx` | 截图期间必须跳过，否则全屏窗口会被折叠 |
| F38 | `ChatView` 的 `MessageInput` 草稿是组件本地 state | `src/chat/message/MessageInput.tsx` | chat 只能隐藏不能卸载，否则草稿丢失 |
| F39 | `OrbConf.click_through` / `always_on_top` 会改 Orb 的点击穿透与置顶 | `src-tauri/src/cmd/config.rs` | 截图期间需要临时关闭穿透并强制置顶，结束后恢复 |
| F40 | `expand_orb_panel` / `collapse_orb_panel` 基于当前 position/size 计算 | `src-tauri/src/window.rs` | 恢复必须写回精确的原物理几何，store 的 `isExpanded` 才能继续成立 |
| F41 | `ChatView` 的附件按钮在流式时禁用；`supportVision` 由当前模型决定 | `src/chat/ChatView.tsx` | 截图按钮应沿用同样的禁用条件 |
| F42 | `ShortcutSettings` 使用 `appConf.keybindings ?? DEFAULT_KEYBINDINGS`；`KeybindingField` 只检测内部重复 | `src/settings/pages/ShortcutSettings.tsx`、`src/settings/shared/renderers/KeybindingField.tsx` | 默认值漂移会让设置页显示与实际注册不一致；外部冲突无法可靠静态枚举，以注册失败为准 |
| F43 | Cap 已加入部分快照 | 本机参考检出 `references/`（gitignored，不入仓） | 作为原生覆盖层、捕获排除、权限校验的只读参考 |
| F44 | Kiri（MIT）与 shiping（CC BY-NC 4.0）已完整克隆 | 本机参考检出 `references/`（gitignored，不入仓） | Kiri 可复制 Rust 抓屏实现；shiping 仅读流程与几何换算，禁止复制代码 |

---

## 5 不变式

违反任一条即视为实现错误。

| # | 不变式 | 依据 |
|---|---|---|
| 1 | 截图产物只通过 `file_path` 进入附件管道，不在 IPC 上传输图片字节 | F10 / F13 |
| 2 | `crop/cancel` 只接受本次抓屏签发的一次性令牌，且只允许 `orb` webview 调用；不接受任意磁盘路径 | 防越权读写 |
| 3 | 前端只发送归一化选区（相对底图显示区域，0..=1）；Rust 用原图尺寸换算，且只换算一次 | 跨 DPI 稳定 |
| 4 | 前端不得自行裁剪图片（不允许把 base64 回传后端） | 不变式 1 |
| 5 | **不新增 webview / 窗口 label**；覆盖层复用 `orb` | 重量约束 |
| 6 | 取消、失败、关闭都必须删除抓屏预览；发送准入成功后删除裁剪临时文件；启动/新抓屏按 TTL 清扫 | 隐私 |
| 7 | 截图不自动发送；只把附件放入输入区 | 误触防护 |
| 8 | 不新增附件上限、不改 `admit_attachments` 的准入判断；裁剪产物必须 ≤ 8MB | 范围约束 |
| 9 | 新增 DTO 一律 `#[derive(TS)]` + 登记到 `protocol_bindings.rs`；rev3 共 3 个，契约总数 57 → 60 | F28 / 项目规则 |
| 10 | 默认快捷键必须同时改 Rust `default_keybindings()` 与前端 `SHORTCUT_DEFINITIONS`，禁止只改一边 | F7 / F8 |
| 11 | 同一时刻只有一个活动会话；所有状态变更命令串行执行；忙时再次触发直接忽略 | 并发约束 |
| 12 | 裁剪产物扩展名必须与实际编码一致（PNG → `.png`，JPEG → `.jpg`），`media_type` 同步 | F11 |
| 13 | 覆盖层只能通过 `screenshot-session` 事件或 `get_screenshot_session` 获取会话，不得依赖 URL/DOM 注入 | 会话引导 |
| 14 | chat/Orb 面板只隐藏（CSS），不卸载；`ChatView` 的草稿、附件、流式状态必须保留 | F38 |
| 15 | 截图期间暂停 `spawn_screen_inspection`、`snap_orb_to_*`、`save_orb_position_sync`；恢复后写回原几何 | F35 / F36 / F40 |
| 16 | 截图期间 Rust 跳过 Orb 的 `ORB_BLUR`，前端 blur 处理再加 store 判断；Orb 不得折叠 | F37 |
| 17 | 抓屏前只隐藏覆盖层宿主 `orb`；`main` / `settings` 保持用户原状并进入底图；成功后显示并聚焦 Orb，取消时按快照恢复 Orb | 目标 5 / §6.3 |

---

## 6 选型

### 6.1 抓屏

| 方案 | 跨平台 | 新增依赖 | 评价 |
|---|---|---|---|
| `xcap` crate | Windows / macOS / Linux | 1 个 crate | 枚举显示器 + 抓 RGBA，多屏与 DPI 由库处理，代码量最小。**推荐** |
| 自研 GDI `BitBlt` | 仅 Windows | 0（`windows` crate 已有 GDI） | 混合 DPI、DWM 遮挡、HDR 全要自己处理 |
| WGC（`Windows.Graphics.Capture`） | 仅 Windows | 需新增 Direct3D11 等 feature | 面向持续流，静态截屏过度设计 |
| macOS ScreenCaptureKit | 仅 macOS | objc2 桥接 | 重；`xcap` 内部已封装 |
| Web `getDisplayMedia` | — | — | **不可用**：WebView2/WKWebView 拿不到系统级屏幕流 |

**决定**：`xcap`。它只负责「拿到一张整屏 RGBA」，框选、裁剪、压缩、窗口管理全部由本仓库承担。

实现约束：

- 在 `Cargo.toml` 锁定确切版本并提交 `Cargo.lock`；本规格不写无效占位。
- 后端抽象为 `CaptureBackend`（`monitors()` / `capture()`，`Send + Sync + 'static`）；默认 `XcapBackend`，CI 用 `FakeBackend`。
- 抓屏、PNG 编码、裁剪、JPEG 兜底全部 `spawn_blocking`。

平台风险：

- Windows：无需额外权限；Win7/8 回退 GDI（V2）。
- macOS：需「屏幕录制」TCC 授权；未授权返回黑图 → 预检 + 实际可用性校验（§8.7）。
- Linux：X11 可用；Wayland 受合成器限制，不承诺支持。

### 6.2 覆盖层：复用 Orb，而不是新开 webview

| 方案 | 新增 webview | 主要成本 | 交互质量 | 结论 |
|---|---|---|---|---|
| **复用 Orb webview（rev3）** | 否 | Orb 几何/焦点/巡检/失焦状态管理 | 静态冻结底图，稳定 | **V1** |
| 新开 `screenshot` webview（rev2） | 是 | 额外 WebView2/WKWebView、viewMap、capabilities、跨窗口事件 | 最隔离，风险最低 | 否决：重 |
| 原生覆盖层（Cap/GPUI 路线） | 否 | 自绘、手柄、吸附、字体、输入循环 | 最流畅，内存最低 | V2 评估：代码量大 |
| 系统截图工具 | 否 | 平台差异、剪贴板依赖、无法统一品牌体验 | 依赖系统 UX | V1 备选，不作为默认 |
| 实时覆盖层 + 捕获排除 | 否 | 需要 `WDA_EXCLUDEFROMCAPTURE` / `NSWindowSharingNone`，Linux 需回退 | 不冻结屏幕，捕获在确认时发生 | V2 优化项 |

复用 Orb 的可行性：

- Orb 窗口已经是透明、无边框、置顶、`skipTaskbar`、不可缩放；可以临时 `set_size`/`set_position` 成显示器物理尺寸（F35）。
- 覆盖层是**静态截图**：先隐藏 `orb` → 抓屏 → 把 Orb 放大并显示截图；宿主窗口自身不会入镜，也不需要透明穿透。`main` / `settings` 不隐藏，因此底图包含它们的内容。
- Orb 的 JS 上下文与 `ChatView` 共享；裁剪结果直接进 `attachmentStore`（不变式 14 的附加收益）。
- 主要风险是 Orb 的状态机（巡检、失焦、点击穿透、位置持久化），在 §8.6 逐条加守卫。

### 6.3 静态冻结 vs 实时覆盖层

V1 选择**静态冻结**：

- 抓屏时只隐藏覆盖层宿主 `orb`，抓到一张稳定的整屏图；后续框选不会因为桌面内容变化而错位。
- `main` / `settings` 保持用户触发瞬间的样子进入底图，「冻结底图 = 用户所见」；V1 不提供「隐藏本应用窗口」的开关，不想让其入镜时由用户自行挪开或最小化。
- 代价是抓屏后需要把图写盘（预览）并保留在内存；一个 4K 会话约 33MB，裁剪/取消即释放。
- 静态图让「选区 = 最终产物」的验收更确定。

V2 可评估实时覆盖层 + 捕获排除（Cap 的 `overlay_backdrop.rs`）：不冻结屏幕、确认时才抓屏，但依赖平台 API，且用户看到的是变化中的桌面。

---

## 7 契约

### 7.1 常量

`src-tauri/crates/assistant-config/src/constant.rs`：

```rust
/// 抓屏前隐藏覆盖层宿主 `orb` 后的等待时间（毫秒），等待 DWM 完成一次合成
pub const SCREENSHOT_HIDE_SETTLE_MS: u64 = 150;
/// 会话事件发出后、显示覆盖层前的等待时间（毫秒），让 orb 先隐藏 chat 并挂载 Overlay
pub const SCREENSHOT_OVERLAY_SETTLE_MS: u64 = 100;
/// 无覆盖层宿主时的孤儿会话存活时间
pub const SCREENSHOT_SESSION_TTL_MS: u64 = 10 * 60 * 1000;
/// screenshot-tmp 残留文件清扫阈值；启动与新抓屏时执行
pub const SCREENSHOT_TMP_TTL_MS: u64 = 60 * 60 * 1000;
/// 与 assistant-agent / 前端一致的图片单张上限
pub const SCREENSHOT_MAX_BYTES: u64 = 8 * 1024 * 1024;
/// 前端最小选区（CSS 逻辑像素）；Rust 侧再保证物理宽高 >= 1
pub const SCREENSHOT_MIN_SELECTION_CSS_PX: u32 = 8;
```

`src-tauri/crates/assistant-storage/src/attachment_store.rs`：

```rust
/// 截图临时子目录名（位于 attachments 根目录下）
pub const SCREENSHOT_TMP_SUBDIR: &str = "screenshot-tmp";
```

### 7.2 DTO（新建 `src-tauri/src/protocol/dto/screenshot.rs`）

共 **3 个** DTO；契约落盘后 `protocol_bindings.rs` 的断言从 57 改为 **60**。

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScreenshotSessionDto {
    /// 一次性令牌：裁剪/取消只认它
    pub token: String,
    /// 抓屏预览图绝对路径（位于 screenshot-tmp 内）
    pub path: String,
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
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub media_type: String,
    pub bytes: u64,
}
```

字段说明：

- `ScreenshotSessionDto` 只是覆盖层的引导数据；目标显示器、原图尺寸留在 Rust 的 `PendingSession` 里，不下发前端。
- 裁剪参数用归一化坐标，前端不需要知道 `scale_factor`，Rust 内部按原图宽高换算；跨 DPI 与窗口尺寸误差都被吸收。
- `media_type` 出现在结果里，因为兜底可能从 PNG 转 JPEG；**落盘扩展名必须跟随 `media_type`**。
- 权限状态不引入 DTO：命令直接返回 `bool`。
- 新增 DTO 必须登记到 `protocol_bindings.rs` 的 `declarations`，并同步改断言。

### 7.3 命令（新建 `src-tauri/src/cmd/screenshot.rs`）

共 **6 条**：

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

/// 显式移除附件时调用；只删除 screenshot-tmp 内路径，其余路径忽略
#[command]
pub async fn release_screenshot(app: AppHandle, paths: Vec<String>) -> Result<(), String>;

/// Windows / Linux 恒 true；macOS 走屏幕录制预检 + 实际显示器可用性校验
#[command]
pub fn screenshot_permission_granted(app: AppHandle) -> bool;
```

调用方约束：

| 命令 | 允许的 webview label | 说明 |
|---|---|---|
| `start_screenshot` | `orb` | 快捷键/菜单/托盘路径由 Rust 内部直接调 `screenshot::start()`，不经过 command |
| `get_screenshot_session` / `crop_screenshot` / `cancel_screenshot` | `orb` | 令牌之外的第二道边界 |
| `release_screenshot` | 任意应用窗口 | 路径校验即可，无越权副作用 |
| `screenshot_permission_granted` | 任意应用窗口 | 只读 |

行为约定：

- `start_screenshot` 忙时（`Phase != Idle` 且覆盖层活跃）直接返回 `Ok(())`；若 `Phase != Idle` 但没有覆盖层宿主（异常残留），先清理再启动。
- `start_screenshot` 成功后返回 `()`；会话通过 `screenshot-session` 事件下发。`get_screenshot_session` 是 reload/错过事件时的兜底。
- `crop_screenshot` 成功后由 Rust 恢复 Orb 几何并返回结果；前端负责关闭覆盖层、ingest、展开与聚焦。
- `cancel_screenshot` 幂等；令牌不匹配不清理其他会话。
- `release_screenshot` 幂等；非 `screenshot-tmp` 路径只记日志不删除。

错误码（命令返回稳定字符串，前端用 `translateKey` 映射；未知错误原样显示）：

| 错误码 | 触发 |
|---|---|
| `screenshot.err.permission_denied` | macOS 未授权或预检通过但无法枚举显示器 |
| `screenshot.err.no_monitor` | 找不到可用显示器 |
| `screenshot.err.capture_failed` | 抓屏、预览编码或窗口隐藏失败 |
| `screenshot.err.no_session` | 覆盖层拉取会话时无活动会话 |
| `screenshot.err.token_invalid` | 令牌缺失/不匹配 |
| `screenshot.err.too_small` | 归一化选区换算后宽/高为 0 |
| `screenshot.err.too_large` | JPEG 兜底后仍超 8MB |
| `screenshot.err.encode_failed` | 编码失败 |

分层：`cmd/screenshot.rs` 只是薄壳，真正逻辑在 `screenshot::mod.rs` 的 `start()` / `cancel()` / `crop()` / `release()`；命令负责「State 取出 + 调用方 label 校验 + 错误转字符串」。

注册：`src-tauri/src/ipc.rs` 的 `generate_handler!` 追加六条；`src-tauri/src/cmd/mod.rs` 加 `mod screenshot;` 与 `pub use screenshot::*;`；`src-tauri/src/lib.rs` 加 `mod screenshot;`；`src-tauri/src/setup.rs` 在 `handle.manage(AttachmentStore)` 之后追加 `handle.manage(ScreenshotState::new())`，并创建 `screenshot-tmp` 目录。

### 7.4 事件

`src-tauri/src/protocol/events.rs` 新增：

```rust
pub const SCREENSHOT_SESSION: &str = "screenshot-session";
pub const SCREENSHOT_ERROR: &str = "screenshot-error";
```

- `screenshot-session` 用 `emit_orb_only` 发送 `ScreenshotSessionDto`；在抓屏完成、覆盖层显示之前发出，让 orb 先隐藏 chat 并挂载 Overlay。
- `screenshot-error` 用 `emit_orb_only` 发送错误码字符串；只服务「快捷键 / 右键菜单 / 托盘」这类没有 invoke 返回值的入口；工具栏入口直接用 `start_screenshot` 的 `Err`。
- **不新增** `screenshot-captured` / `screenshot-cancelled`：裁剪结果由 orb 直接拿 invoke 返回值；取消由 Rust 恢复，不需要事件。
- 前端 `src/types/events.ts` 增加 `UiEvent.ScreenshotSession = "screenshot-session"` 与 `UiEvent.ScreenshotError = "screenshot-error"`。

### 7.5 快捷键注册与默认值

#### 7.5.1 注册流程重构（`src-tauri/src/setup.rs`）

当前是「定长数组 + `match id`」；新增一条要同时改数组长度、数组项、`match` 分支，且无法表达「用户主动清空」与「重复绑定」。rev3 改为：

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
- 新增快捷键只需要在 `SHORTCUT_SPECS` 加一项 + 前端 `SHORTCUT_DEFINITIONS` 加一项。

已落地：`assistant-config/src/app_conf.rs` 新增 `default_keybindings()` 与 `primary_modifier()`；`setup.rs` 使用同一份默认值，`AppConf::new()` 也从它初始化。

#### 7.5.2 默认组合

| id | 默认（macOS） | 默认（Windows / Linux） | 说明 |
|---|---|---|---|
| `toggle_orb` | `Meta+Shift+Alt+O` | `Control+Shift+Alt+O` | 原 `Meta+Shift+O` / `Ctrl+Shift+O` 与编辑器「跳转符号」冲突 |
| `toggle_main` | `Meta+Shift+Alt+M` | `Control+Shift+Alt+M` | 降低与 IDE 重构类快捷键的冲突 |
| `new_chat` | `Meta+Shift+Alt+N` | `Control+Shift+Alt+N` | **替换原 `Ctrl+Shift+N`**：浏览器无痕/新建窗口、资源管理器新建窗口的高频冲突 |
| `capture_screenshot` | `Meta+Shift+Alt+S` | `Control+Shift+Alt+S` | 本规格实现时加入；避开 `Ctrl+Shift+A`（浏览器标签搜索）与 IM 截图 |

理由：

- 全局热键会抢占其他应用的本地快捷键，三键组合几乎没有完全无冲突的选项；四键组合显著降低冲突概率。
- 用户仍可在设置页改键；注册失败会通过 `SHORTCUT_WARNING` 提示（§9.7）。
- 已有配置文件不会被自动改写：旧默认仍按原值注册；设置页「恢复默认」会写入新组合。实现截图快捷键时，需同时把 `capture_screenshot` 加入 `default_keybindings()` / `SHORTCUT_DEFINITIONS` / `SHORTCUT_SPECS`。

#### 7.5.3 前端默认值

`src/settings/core/types/shortcutTypes.ts` 的三条 `defaultBinding` 改为 `${PRIMARY_MOD}+Shift+Alt+O/M/N`，与 Rust `default_keybindings()` 保持一致。**不新增静态外部冲突表**：其他应用的快捷键无法可靠枚举，真正的冲突由操作系统注册失败暴露（`SHORTCUT_WARNING`）。

### 7.6 capabilities

**不改** `src-tauri/capabilities/default.json`：不新增窗口 label，覆盖层运行在 `orb`，其现有权限已足够。覆盖层只调用应用自定义命令（不受 plugin ACL 约束）；事件监听走 `core:event:default`；预览走已启用的 asset protocol。

### 7.7 依赖

`src-tauri/Cargo.toml`：

```toml
xcap = "<落地时锁定的确切版本>"
image = { workspace = true }   # 截图/裁剪直接使用 RgbaImage、crop_imm、JpegEncoder
```

- 版本落地时核实并提交 `Cargo.lock`。
- 令牌用既有 `uuid` v4（32 hex），不新增随机数依赖。
- `image` feature 仍是 workspace 的 `png/jpeg/bmp/tiff`；不扩 webp。

---

## 8 Rust 设计

### 8.1 模块布局与状态

```
src-tauri/src/screenshot/
├── mod.rs         入口：Phase 状态机 / start / cancel / crop / release / 清理
├── capture.rs     CaptureBackend trait + XcapBackend + 显示器定位 + 抓屏编码
├── crop.rs        令牌校验 + 归一化→原图换算 + 裁剪 + 压缩兜底 + 落盘
└── orb_overlay.rs Orb 窗口全屏/恢复快照 + SCREENSHOT_ACTIVE
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
    preview_path: PathBuf,
    /// 抓屏原图保存在内存，裁剪不再解码磁盘文件
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

- `SCREENSHOT_ACTIVE: AtomicBool`，由 `orb_overlay` 维护；巡检线程、`snap_orb_to_*`、`save_orb_position_sync`、`expand/collapse_orb_panel`、`ORB_BLUR` 都读取它。
- `ScreenshotState::new()` 使用 `XcapBackend`；单测用 `ScreenshotState::with_backend(FakeBackend)`。
- 4K RGBA 约 33MB；只保留一个会话，裁剪/取消即释放。

### 8.2 抓屏流程（`start`）

0. 取 `op` 锁；`prune_stale(app)`：清理无覆盖层宿主且超 TTL 的孤儿会话；清扫 `screenshot-tmp` 超阈值文件；关闭异常的残留全屏 Orb。
1. `Phase != Idle` 且覆盖层活跃 → 记日志并返回 `Ok(())`；无覆盖层宿主则视为孤儿，已在上一步清理。
2. macOS 权限预检（未授权 → 返回错误码，**不隐藏任何窗口**）。
3. `get_mouse_position()`（F24）→ 失败回退 Orb 中心 → 失败回退主屏中心；用 `placement::nearest_screen` 选显示器。
4. 从 `CaptureBackend::monitors()` 中按物理 bounds 匹配显示器；`scale_factor` 以 Tauri `available_monitors()` 为准。
5. 快照 Orb 几何/可见性/焦点/置顶/点击穿透（F35/F39）；只隐藏 `orb`（若其可见），`main` / `settings` 保持用户原状并进入底图；`SCREENSHOT_ACTIVE = true`；若 Orb 被隐藏，`sleep(SCREENSHOT_HIDE_SETTLE_MS)`（F25）。
6. `spawn_blocking`：抓 RGBA + 编码预览 PNG + 写入 `attachments/screenshot-tmp/<uuid>.png`。
7. 生成一次性令牌（uuid v4 32 hex），写入 `PendingSession`，`Phase = Selecting`。
8. **在 Orb 仍隐藏时** `emit_orb_only(SCREENSHOT_SESSION, session)`；sleep `SCREENSHOT_OVERLAY_SETTLE_MS`，让 orb 隐藏 chat、挂载 Overlay、开始加载底图。
9. 把 Orb 窗口设为 `always_on_top(true)`、`set_ignore_cursor_events(false)`、物理位置/尺寸 = 目标显示器；`show()` + `set_focus()`。返回 `Ok(())`。
10. 任一步失败 → 删除预览 → 恢复 Orb 窗口/几何 → `SCREENSHOT_ACTIVE = false` → `Phase = Idle` → 返回错误码。

> 先确定显示器再隐藏窗口：鼠标位置不受隐藏影响，权限/无显示器错误不会造成窗口闪烁。
> Windows 上隐藏后需确认 `IsWindowVisible` 为 false 并 `DwmFlush`，不能只依赖固定延时（参考 Kiri ADR 0107，见 §14.3）。
> 抓屏前若首帧为空或全黑，重试一次再判定失败（参考 shiping `take-screenshot.ts`）。
> 覆盖层显示前先发会话事件：orb 可以在隐藏状态下隐藏 chat、挂载 Overlay；`OVERLAY_SETTLE_MS` 减少「chat 被拉成全屏」的一帧闪烁。

### 8.3 会话拉取（`get_screenshot_session`）

- 校验调用方 label == `orb`。
- `Phase == Selecting` 且会话存在 → 返回 `ScreenshotSessionDto`；否则 `screenshot.err.no_session`。
- 覆盖层 reload / 错过事件时使用；正常路径以 `screenshot-session` 事件为准。

### 8.4 裁剪（`crop`）

1. 取 `op` 锁；校验调用方 label；校验 token 与 `Phase == Selecting`，否则 `token_invalid`。
2. 置 `Phase = Cropping`；克隆 `Arc<RgbaImage>` 与 target 后释放 inner 锁。
3. 归一化选区 → 原图像素：`sx = round(x * image_w)`、`ex = round((x + width) * image_w)`，y 同理；夹到图片边界；宽或高为 0 → `too_small`。
4. `spawn_blocking`：`crop_imm` → 编码 PNG → `assistant_agent::normalization::normalize_image(data, "image/png")`（长边 2048，F17）。
5. 体积兜底：若结果 > `SCREENSHOT_MAX_BYTES`，改用 `JpegEncoder::new_with_quality(&mut out, 85)` 重编码为 JPEG；仍超限 → `too_large`。
6. 落盘扩展名跟随最终格式（`.png` / `.jpg`），文件名 `<uuid>.<ext>`，目录 `screenshot-tmp`；返回 `ScreenshotCropResultDto`。
7. 成功：再次校验 token 仍匹配（防止裁剪期间被取消）→ 清除会话 → 删除预览 → `Phase = Idle` → 恢复 Orb 几何/可见性/焦点/置顶/点击穿透 → `SCREENSHOT_ACTIVE = false`。返回结果。
8. 失败：若 token 仍匹配，`Phase` 回到 `Selecting` 并保留会话，让用户可以重选；若已取消则丢弃产物并返回 `token_invalid`。

> 裁剪成功后**不**由 Rust 关闭 Overlay：前端收到结果后自行 `clearSession()`；Rust 只负责把 Orb 窗口恢复到原几何。

### 8.5 Orb 全屏与恢复（`orb_overlay.rs`）

```rust
// 进入覆盖层
let snapshot = OrbWindowSnapshot::capture(&window)?;
window.set_always_on_top(true)?;
window.set_ignore_cursor_events(false)?;
window.set_position(PhysicalPosition::new(target.monitor_x, target.monitor_y))?;
window.set_size(PhysicalSize::new(target.monitor_width, target.monitor_height))?;
window.show()?;
window.set_focus()?;

// 退出覆盖层（取消/成功）
window.set_position(snapshot.position)?;
window.set_size(snapshot.size)?;
window.set_always_on_top(snapshot.always_on_top)?;
window.set_ignore_cursor_events(snapshot.click_through)?;
if snapshot.visible { window.show()?; } else { window.hide()?; }
if snapshot.focused { window.set_focus()?; }
```

要点：

- Orb 窗口本来是透明、无边框、置顶、`skipTaskbar`、不可缩放；`set_size` 程序化写入仍有效（F35）。
- 恢复必须写回**精确物理几何**（F40）；前端 `useOrbStore.isExpanded` 不需要改。
- 成功时无论 Orb 原先是否可见，都 `show()` + 聚焦 + 事件驱动 `expand()`；取消时按快照恢复可见性/焦点。
- `SCREENSHOT_ACTIVE` 在进入前置 true、退出后置 false；任何失败路径都必须复位。

### 8.6 状态守卫（关键）

| 位置 | 守卫 | 原因 |
|---|---|---|
| `setup.rs` `spawn_screen_inspection` | `if screenshot::is_active() { continue; }` | 防止每 800ms 把全屏几何写回 `OrbConf.position_x/y`（F36） |
| `cmd/orb.rs` `snap_orb_to_screen` / `snap_orb_to_edge` / `save_orb_position_sync` | `if screenshot::is_active() { return Ok(()); }` | 防止吸附/保存把全屏几何当作用户位置 |
| `window.rs` `expand_orb_panel` / `collapse_orb_panel` | `if screenshot::is_active() { return Ok(()); }` | 防止覆盖层被前端或残留调用改尺寸 |
| `window.rs` `on_window_event` Orb `Focused(false)` | `if screenshot::is_active() { return; }` | 不发送 `ORB_BLUR`，避免 `OrbPanel` 折叠（F37） |
| `window.rs` `on_window_event` Orb `CloseRequested` | 截图期间 `prevent_close()` + 走取消清理 | 防止截图会话悬空 |
| `src/orb/OrbPanel.tsx` blur 处理 | `if (useScreenshotStore.getState().session || preparing) return;` | 前端第二道防线 |
| `src/orb/OrbWindow.tsx` | 覆盖层期间加 `orb-screenshot` class；`OrbBall`/`OrbPanel` 只 CSS 隐藏 | 保留 `ChatView` 草稿（F38） |
| `src/chat/ChatView.tsx` 截图按钮 | `stream.isStreaming || !supportVision` 时禁用 | 与附件按钮一致（F41） |

### 8.7 隐私与权限

- macOS：`CGPreflightScreenCaptureAccess` 预检后，还要用 `CaptureBackend::monitors()` / `SCShareableContent` 验证「预检通过但实际无显示器可抓」的陈旧 TCC 状态；验证成功可缓存，失败按短间隔重试（参考 Cap `permissions/screen_capture.rs`）。
- Windows / Linux：预检恒 `true`。
- 保险库锁屏、系统空闲锁定：`ScreenLockMonitor` 只在 vault feature 内构造（F30），宿主层拿不到。**V1 明确不做锁屏 gate，在验收里标注为已知缺口；V2 把 monitor 提升为全局 `ScreenLockState` 后再接入。**
- 令牌一次性、只发给 orb；`crop/cancel` 校验调用方 label 与令牌。
- 临时文件位于应用数据目录内，随会话删除；不写系统临时目录。
- 参考 Cap 的 `overlay_backdrop.rs`：Windows `WDA_EXCLUDEFROMCAPTURE` / macOS `NSWindowSharingNone` 可把自身窗口排除出截图；V1 仍用「隐藏 `orb` + 150ms + 可见性校验」保证跨平台一致，V2 可切到捕获排除以减少闪烁。

### 8.8 清理与临时文件生命周期

| 触发 | 动作 |
|---|---|
| 取消（ESC / 右键 / `cancel_screenshot`） | 删除抓屏预览；清会话；恢复 Orb 几何/可见性/焦点；`Phase = Idle` |
| Orb `CloseRequested` 且截图活跃 | `prevent_close` + 取消清理 |
| 抓屏/编码/建覆盖层失败 | 删除预览；恢复 Orb；`SCREENSHOT_ACTIVE = false`；`Phase = Idle` |
| 裁剪成功 | 删除抓屏预览；保留裁剪产物 |
| 消息发送准入成功 | `admit_attachments` 在整条消息准入完成后批量删除 `screenshot-tmp` 源文件 |
| 用户显式移除附件 | 前端 `release_screenshot([filePath])` 立即删除 |
| 启动 / 新抓屏 | `sweep_screenshot_tmp` 删除超过 `SCREENSHOT_TMP_TTL_MS` 的残留 |

实现要点：

- `AttachmentStore` 新增 `dir()`、`screenshot_tmp_dir()`、`is_screenshot_tmp_path(&Path)`（canonicalize 后判断前缀）、`remove_screenshot_tmp(&Path)`、`sweep_screenshot_tmp(Duration)`。
- `admit_attachments` 在 `admit_image` 内收集源路径，整条消息循环结束后统一删除；中途失败不删除，保留重试能力。
- 只删除 `screenshot-tmp` 内的路径；用户主动选择的文件不会被误删。
- 临时子目录不递归进入 `list_all` / 存储统计（F31）。
- `clean_tmp()`（F21）保持原语义，不再被截图功能复用。

### 8.9 测试缝

- `CaptureBackend`：`FakeBackend` 提供固定尺寸图片与可配置显示器列表，覆盖单屏、双屏、副屏负坐标。
- `crop::to_image_rect(normalized, image_w, image_h)` 纯函数，覆盖 0.1/0.25/0.5/0.75/1.0 与边界夹取。
- `Phase` 状态机脱离 Tauri 测试：忙时 `start` 忽略、`cancel` 幂等、裁剪期间取消。
- TTL 判断注入 `now`，避免测试 sleep。
- `AttachmentStore` 临时路径判断/清扫用 `tempfile` 单测。
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

- `OrbBall`/`OrbPanel` 仍然挂载，`ChatView` 的 `MessageInput` 草稿、附件、流式状态保留。
- `ScreenshotOverlay` 用 `position: fixed; inset: 0; z-index: 1;` 覆盖；底图铺满。
- `preparing` 用于工具栏入口：点击后立刻隐藏 chat，避免抓屏/放大过程露出旧 UI；失败或事件到达后复位。
- 快捷键入口不需要 `preparing`：抓屏时 Orb 已被 Rust 隐藏，会话事件到达后才显示。

### 9.2 会话订阅

`src/screenshot/useScreenshotSession.ts`（在 `OrbWindow` 中调用）：

```ts
useEffect(() => {
  const unSession = listen<ScreenshotSessionDto>(UiEvent.ScreenshotSession, (e) => {
    useScreenshotStore.getState().setSession(e.payload);
    useScreenshotStore.getState().endPreparing();
  });
  const unError = listen<string>(UiEvent.ScreenshotError, (e) => {
    useScreenshotStore.getState().endPreparing();
    notifyScreenshotError(e.payload);
  });
  void screenshotService.getSession().then((s) => {
    if (!useScreenshotStore.getState().session) {
      useScreenshotStore.getState().setSession(s);
    }
  }).catch(() => {});
  return () => {
    void unSession.then((f) => f());
    void unError.then((f) => f());
  };
}, []);
```

- `get_screenshot_session` 是兜底：reload、错过事件、启动时序。
- `no_session` 错误静默忽略；其他错误走 toast。
- `screenshotStore`：`session`、`preparing`、`setSession`、`clearSession`、`beginPreparing`、`endPreparing`。

### 9.3 选区几何（纯函数，可单测）

`src/screenshot/selection.ts`：

```ts
export interface Rect { x: number; y: number; width: number; height: number }
export interface NormRect { x: number; y: number; width: number; height: number }

export function normalizeDrag(startX: number, startY: number, curX: number, curY: number): Rect;
export function clampToBounds(rect: Rect, bounds: Rect): Rect;
export function resizeByHandle(rect: Rect, handle: HandleId, dx: number, dy: number, bounds: Rect): Rect;
export function isValidSelection(rect: Rect): boolean;  // 宽高均 >= SCREENSHOT_MIN_SELECTION_CSS_PX
export function toNormalizedRect(rect: Rect, viewport: { width: number; height: number }): NormRect;
```

- 前端只产出 CSS 逻辑像素与归一化值；**不提供** `toPhysical`（不变式 3）。
- 归一化基于底图显示区域（`img.getBoundingClientRect()`），即使窗口尺寸与显示器物理尺寸有细微差异也不会错位。
- 全部纯函数、不依赖 DOM 与 store，直接用 vitest 覆盖。

### 9.4 Overlay 组件

`src/screenshot/Overlay.tsx`：

- 底图 `<img src={convertFileSrc(session.path)}>`，`width/height: 100%; object-fit: fill`；`onError` 显示错误并取消。
- 一层 SVG/canvas 叠加：遮罩（`rgba(0,0,0,0.45)`）、选框边框、尺寸标注、8 个手柄。
- 键盘：`ESC` 取消，`Enter` 确认；右键取消；双击选区内部 = 确认。
- 确认期间 `isCropping = true`，禁用重复确认/取消；命令失败保持覆盖层并显示错误。
- 成功后：`ingestPathToStore(result.path, meta)` → `clearSession()` → 若未展开 `await expand()` → `requestChatInputFocus()`。
- 取消：`await cancel(token)` → `clearSession()`；Rust 已恢复 Orb。
- 窗口失焦不自动取消（用户可能切去查资料）；无覆盖层宿主后由 Rust TTL 兜底。
- 光标 `crosshair`；`user-select: none`；阻止右键菜单默认行为。

### 9.5 结果接入

`src/chat/attachment/intake.ts`：

1. 抽出独立函数（不依赖 hook）：

```ts
export function ingestPathToStore(
  filePath: string,
  meta?: { mediaType?: AllowedImageMimeType; bytes?: number; width?: number; height?: number },
): void;
```

`useAttachmentIntake()` 内部改为调用它，避免把拖拽订阅带到第二个 hook（F12）。

2. `AttachmentIntake` 接口增加 `ingestPath`（供 ChatView 复用）。
3. `createImageAttachmentFromPath(filePath, meta?)` 支持用 `meta` 覆盖 MIME/字节数/尺寸；无 `meta` 时保持扩展名推断；若 `meta.bytes > 8MB` 返回 `too_large`（F16 的前端提前提示）。

结果直接进入 Orb 的 store，不需要跨窗口事件：

```ts
const result = await screenshotService.crop({
  token: session.token,
  ...toNormalizedRect(selection, viewport),
});
ingestPathToStore(result.path, {
  mediaType: result.mediaType,
  bytes: result.bytes,
  width: result.width,
  height: result.height,
});
useScreenshotStore.getState().clearSession();
if (!useOrbStore.getState().isExpanded) await expand();
requestChatInputFocus();
```

- `expand()` 来自 `src/orb/OrbExpand.ts` 的 `useOrbExpand()`，复用「先请求 Rust 改窗口尺寸 → 再切 store 状态」。
- 不自动发送；准入仍由 `attachmentStore.addAttachment` → `admitAttachment` 决定（vision 校验、4 张上限），截图不绕过任何校验（不变式 8）。
- 显式移除截图附件时立即释放临时文件：`ChatView` 的 `AttachmentPreviewBar` 包一层，先查 `filePath` 再调 `release_screenshot([filePath])`，随后 `removeAttachment`。Rust 对非 `screenshot-tmp` 路径是 no-op。
- 发送成功后的清理不依赖前端：后端准入时已删除源文件。发送失败时文件保留，便于重试；最终由 TTL sweep 兜底。

### 9.6 入口

| 入口 | 位置 | 改动 |
|---|---|---|
| 全局快捷键 | `src-tauri/src/setup.rs` | `SHORTCUT_SPECS` 加 `capture_screenshot`（实现时）；按下调用 `screenshot::start()`，失败发 `SCREENSHOT_ERROR` |
| Orb 右键菜单 | `src-tauri/src/window.rs` `show_orb_context_menu` + `src-tauri/src/menu.rs` | 新增 `orb_screenshot` 菜单项 + 事件分支（F26） |
| 聊天工具栏 | `src/chat/ChatView.tsx` | 在 `PaperClipOutlined` 旁加 `ScissorOutlined`；`beginPreparing()` 后 `start()`；失败 toast（F41） |
| 托盘菜单 | `src-tauri/src/tray.rs` | 新增 `screenshot` 菜单项（可选，M2） |

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
"screenshot.select_hint": "拖拽选择区域，Enter 确认，Esc 取消",
"screenshot.too_small": "选区太小",
"screenshot.permission_denied": "需要「屏幕录制」权限",
"screenshot.permission_guide": "前往 系统设置 → 隐私与安全性 → 屏幕录制 授权后重试",
"screenshot.capture_failed": "截屏失败",
"screenshot.session_expired": "截图会话已过期，请重新截图",
"screenshot.too_large": "截图过大，请缩小选区",
"screenshot.err.no_monitor": "未找到可用显示器",
"screenshot.err.encode_failed": "截图处理失败",
"orb.context_screenshot": "截图提问"
```

Rust 侧文案（托盘 / 右键菜单）走 `assistant_i18n::t()`，键名同步加入 `src-tauri/crates/assistant-i18n/locales/*.json`。

---

## 10 文件清单

### 10.1 新建

| 文件 | 内容 |
|---|---|
| `src-tauri/src/screenshot/mod.rs` | `Phase` 状态机、`start` / `cancel` / `crop` / `release` / 清理、op lock |
| `src-tauri/src/screenshot/capture.rs` | `CaptureBackend` trait + `XcapBackend`、显示器定位、抓屏与预览编码 |
| `src-tauri/src/screenshot/crop.rs` | 令牌校验、归一化→原图换算、裁剪、压缩兜底、落盘 |
| `src-tauri/src/screenshot/orb_overlay.rs` | Orb 窗口全屏/恢复快照、`SCREENSHOT_ACTIVE` |
| `src-tauri/src/cmd/screenshot.rs` | 六条命令与调用方校验 |
| `src-tauri/src/protocol/dto/screenshot.rs` | 三个 DTO |
| `src/screenshot/Overlay.tsx` | 覆盖层组件 |
| `src/screenshot/selection.ts` | 选区几何纯函数（含归一化） |
| `src/screenshot/session.ts` | invoke 封装、错误码翻译、`requestChatInputFocus` |
| `src/screenshot/useScreenshotSession.ts` | 会话/错误事件订阅 + 兜底拉取 |
| `src/stores/screenshotStore.ts` | `session` / `preparing` 状态 |
| `src/screenshot/__tests__/selection.test.ts` | 几何单测 |

### 10.2 修改

| 文件 | 改动 |
|---|---|
| `src-tauri/Cargo.toml` | 加 `xcap`（锁定版本）与 `image` 直接依赖 |
| `src-tauri/src/ipc.rs` | 六条命令登记 |
| `src-tauri/src/cmd/mod.rs` | `mod screenshot` + re-export |
| `src-tauri/src/protocol/dto/mod.rs` | `pub mod screenshot;` + `pub use screenshot::*;` |
| `src-tauri/src/protocol/events.rs` | 两个事件常量 |
| `src-tauri/src/setup.rs` | 快捷键表驱动重构、`ScreenshotState` 注册、临时目录创建与 sweep、巡检线程守卫 |
| `src-tauri/src/window.rs` | 右键菜单加项；Orb 关闭/失焦守卫；`expand/collapse_orb_panel` 守卫 |
| `src-tauri/src/menu.rs` | `orb_screenshot` 事件分支 |
| `src-tauri/src/lib.rs` | `mod screenshot;` |
| `src-tauri/tests/protocol_bindings.rs` | 三个 DTO 登记；断言 57 → 60 |
| `src-tauri/crates/assistant-config/src/constant.rs` | 六个常量 |
| `src-tauri/crates/assistant-config/src/app_conf.rs` | `primary_modifier()` / `default_keybindings()`；`AppConf::new` 使用它 |
| `src-tauri/crates/assistant-storage/src/attachment_store.rs` | `SCREENSHOT_TMP_SUBDIR`、`screenshot_tmp_dir`、`is_screenshot_tmp_path`、`remove_screenshot_tmp`、`sweep_screenshot_tmp` |
| `src-tauri/crates/assistant-agent/src/multimodal.rs` | 准入成功后批量删除 screenshot-tmp 源文件（不改准入判断） |
| `src-tauri/crates/assistant-orb/src/placement.rs` | 抽出 `nearest_screen` 纯函数 |
| `src-tauri/src/cmd/orb.rs` | `get_screen_size` 改用 `nearest_screen`；`snap_*`/`save_orb_position_sync` 截图守卫 |
| `src-tauri/src/tray.rs` | M2 托盘菜单项 |
| `src/types/protocol.contract.ts` | `npm run contract:gen` 生成（禁止手改） |
| `src/types/events.ts` | `UiEvent.ScreenshotSession` / `ScreenshotError` |
| `src/settings/core/types/shortcutTypes.ts` | 三条默认值改为四键组合（与 Rust `default_keybindings()` 同步） |
| `src/i18n/locales/*.json` | §9.9 前端键 |
| `src/chat/attachment/factory.ts` | `createImageAttachmentFromPath` 接收 metadata |
| `src/chat/attachment/intake.ts` | 抽出 `ingestPathToStore`，接口暴露 `ingestPath` |
| `src/chat/ChatView.tsx` | 截图按钮；移除附件时调用 `release_screenshot` |
| `src/chat/message/MessageInput.tsx` | `ref` + `chat-focus-input` 监听 |
| `src/orb/OrbPanel.tsx` | blur 处理加 `screenshotStore` 判断 |
| `src/layouts/OrbWindow.tsx` | 覆盖层条件渲染 + `orb-screenshot` class |
| `src/assets/styles/global.css` | 隐藏 OrbBall/OrbPanel 的截图态样式 |
| `src-tauri/crates/assistant-i18n/locales/*.json` | Rust 侧右键/托盘文案 |

> 不再需要：`src/layouts/ScreenshotWindow.tsx`、`src/config/constants.ts` 的 `WINDOW_SCREENSHOT`、`capabilities` 改动、`App.tsx` `viewMap` 改动、`src/screenshot/useScreenshotResult.ts`。

---

## 11 验收

### 11.1 单测

| 目标 | 用例 |
|---|---|
| `selection.ts` | 反向拖拽归一化、越界夹取、8 向手柄最小尺寸、最小选区判定、归一化在任意 viewport 下的边界 |
| `crop.rs` | 令牌缺失/不匹配 → 错误且不落盘；归一化越界 → 夹取；宽/高为 0 → `too_small`；超 8MB → JPEG 兜底后仍在限内且扩展名为 `.jpg`；PNG 路径扩展名为 `.png` |
| `crop` 换算 | 0.1/0.25/0.5/0.75/1.0 归一化在 1000×500 / 3840×2160 下的取整与夹取 |
| `capture.rs` | 显示器选择：单屏、双屏（光标在副屏）、副屏负坐标；`FakeBackend` |
| 状态机 | 忙时 `start` 忽略；`cancel` 幂等；裁剪期间取消丢弃产物 |
| `orb_overlay` | 只快照/隐藏 `orb`；`main` / `settings` 不进入快照；隐藏失败时按快照复位 Orb |
| `attachment_store` | `is_screenshot_tmp_path` 只认子目录内路径；`sweep` 只删超阈值文件；`remove` 幂等 |
| `admit_attachments` | 带 screenshot-tmp `file_path` 的图片准入成功后源文件被删；普通路径不被删；准入失败不删 |
| `setup.rs` | 快捷键解析：默认、覆盖、显式清空、非法、重复、缺默认 —— 逻辑下沉到 `assistant-config::shortcut::resolve_bindings`，由 `crates/assistant-config/tests/shortcut.rs` 覆盖 |

### 11.2 契约与快照

- `npm run contract:gen` 后 `src/types/protocol.contract.ts` 出现三个新类型，`git diff` 只含新增。
- `src-tauri/tests/protocol_bindings.rs` 的 DTO 数量断言为 **60**。
- `npm run contract:check` 干净。
- `npm run test` 全绿（含两个 settings 快照 fixture）。
- `npm run typecheck`、`npm run lint` 干净。
- `cargo test` 干净（含 `setup.rs` 快捷键解析用例）。

### 11.3 手动

| 场景 | 期望 |
|---|---|
| 快捷键触发 | 该屏变暗进入框选；没有新窗口出现；Orb 不可见，主窗/设置窗保持原状并出现在底图 |
| 从 Orb 触发且主窗可见 | 底图包含主窗内容，框选主窗区域后产物与所见一致；主窗未被隐藏或移动 |
| chat 自隐藏 | 截图期间看不到 OrbBall/OrbPanel；结束或取消后恢复 |
| 草稿保留 | 在输入框输入文字后触发截图、取消，文字仍在 |
| 取消 / 右键 / ESC | 覆盖层消失，Orb 回到原位置/尺寸/可见性/焦点；`screenshot-tmp` 为空 |
| 框选确认 | Orb 展开并聚焦输入框，输入区出现缩略图，未自动发送 |
| 发送 | 消息含 `{ type: "image", file_path }`；发送后 `screenshot-tmp` 为空 |
| 移除未发送的截图 | 缩略图消失，`screenshot-tmp` 对应文件立即删除 |
| 发送失败后重试 | 临时文件仍存在，重试可成功 |
| 4K 屏整屏框选 | 附件不触发 `too_large`，缩略图清晰 |
| 混合 DPI（125% 副屏） | 选区与最终图片内容一致（归一化换算，不偏移） |
| JPEG 兜底 | 返回 `mediaType = image/jpeg`，路径扩展名 `.jpg`，预览正常 |
| 无 vision 模型 | 与粘贴图片一致的 `no_vision` 提示 |
| 连续触发两次 | 第二次被忽略，不叠加会话、不重新抓屏 |
| Orb 在副屏 / 副屏负坐标 | 覆盖层落在光标所在屏；取消后 Orb 回到原位置 |
| Orb 点击穿透开启 | 覆盖层期间可正常框选；取消后恢复点击穿透 |
| Orb 原先隐藏 / 展开 | 取消后恢复原状态；成功时显示并展开 |
| 巡检线程 | 截图期间移动/缩放 Orb 不写入 `OrbConf`；取消后位置不变 |
| 失焦 | 截图期间点击其他窗口，Orb 不折叠、覆盖层不消失 |
| 设置页 | 新默认值显示为四键；旧 `Ctrl+Shift+N` 仍可保存；注册失败时出现 `SHORTCUT_WARNING` 提示；恢复默认写入新组合 |
| macOS 未授权 | 明确错误码与引导文案，不进入框选 |

---

## 12 里程碑

| 阶段 | 内容 | 交付判据 |
|---|---|---|
| M1 | Orb 复用覆盖层 + 会话引导 + 框选 + 裁剪 + 直通 ingest + 快捷键表驱动 + 默认值调整 + 临时文件全生命周期 + 状态守卫 | §11.3 全部通过 |
| M2 | Orb 右键菜单、托盘菜单、macOS 权限引导页、标注能力、实时覆盖层 + 捕获排除（Cap 路线） | 设置页可改快捷键；macOS 未授权时有明确引导 |
| M3 | 多屏拼接框选、截图历史、无 vision 时的 OCR 降级、锁屏 gate、原生覆盖层评估 | 与 `pdf_text.rs` 的降级策略一致；锁屏 gate 抽为公共 State |

---

## 13 风险

| # | 风险 | 处置 |
|---|---|---|
| 1 | Orb 窗口被临时放大后未恢复，或位置被巡检线程写坏 | 快照/恢复精确物理几何；`SCREENSHOT_ACTIVE` 全面守卫；失败路径强制复位 |
| 2 | 全屏放大瞬间露出被拉伸的 chat | 会话事件先于显示、`SCREENSHOT_OVERLAY_SETTLE_MS`；工具栏入口先 `preparing` 隐藏 |
| 3 | 失焦触发折叠，覆盖层被缩回 | Rust 跳过 `ORB_BLUR`；`OrbPanel` blur 处理加 store 判断 |
| 4 | 混合 DPI / 跨屏移动导致选区错位 | 前端归一化、Rust 按原图尺寸换算；Orb 先移到目标屏再显示 |
| 5 | macOS 屏幕录制权限：未授权返回黑图；预检通过但实际无显示器 | 预检 + `monitors()`/`SCShareableContent` 校验 + 引导；参考 Cap |
| 6 | 4K PNG 超 8MB | 长边 2048 + JPEG q85 兜底 |
| 7 | 4K 原图常驻内存约 33MB | 单会话；裁剪/取消释放；不做历史缓存 |
| 8 | 连按快捷键/重复 invoke | op lock + `Phase` 状态机；忙时忽略 |
| 9 | `xcap` 版本/API 不一致 | 落地锁定版本；坐标缺失时用 `placement::get_available_screens` 补齐 |
| 10 | 隐私：锁屏/保险库解锁期间截屏 | V1 明确不做（F30）；V2 抽公共 `ScreenLockState` |
| 11 | 全局热键抢占其他应用快捷键 | 四键默认 + 注册失败 `SHORTCUT_WARNING` + 设置页可改 |
| 12 | macOS 全屏 Space 下 Orb 覆盖层不可见 | V1 记录限制；V2 监听 Space 或 `set_visible_on_all_workspaces` |
| 13 | 显示器热插拔导致几何过期 | 抓屏到裁剪期间配置变化可能错位；V1 日志 + V2 监听取消 |
| 14 | 截图临时文件残留 | 发送准入删除 + 显式移除释放 + 启动/新抓屏 sweep 三重兜底 |
| 15 | Cap 参考代码为 AGPL-3.0 | 只读参考、不复制；交互与失败语义可借鉴 |
| 16 | 主窗/设置窗遮挡用户想截的目标 | 与系统截图工具一致，由用户自行挪开或最小化；V2 可提供「抓屏时隐藏本应用窗口」的可选项 |

---

## 14 参考项目对照

### 14.1 Cap（新增，部分快照）

Cap（上游 `CapSoftware/Cap`）是部分检出（提交 `620f65b`，AGPL-3.0；仅阅读，不复制代码）：

| 文件 | 可借鉴内容 |
|---|---|
| `apps/desktop-gpui/src/screenshot.rs` | 隐藏自身覆盖层 → 150ms 合成等待 → 抓屏 → 落盘；失败语义与「截图不是录制」的边界 |
| `apps/desktop-gpui/src/target_overlay.rs` | 每个显示器一个全屏覆盖层；截屏最小选区 1px、手柄抓取区 18px、光标探测 50–80ms |
| `apps/desktop-gpui/src/overlay_backdrop.rs` | `WDA_EXCLUDEFROMCAPTURE`（Windows）/ `NSWindowSharingNone`（macOS）把覆盖层排除出截图；40ms settle、3s 过期 |
| `apps/desktop-gpui/src/permissions/screen_capture.rs` | 预检通过后仍用 `SCShareableContent` 验证真实可抓；缓存成功、失败重试、主线程不阻塞 |
| `apps/desktop-gpui/src/screenshot_crop.rs` | 全分辨率图片解码（5K 约 59MB）与关闭时释放；比例锁定、吸附、8 向手柄 |
| `apps/desktop-gpui/src/editor_crop.rs` | 裁剪状态机、命中测试、吸附参考 |
| `apps/desktop-gpui/kernels/src/screenshot.rs` | 原生抓屏内核 |

关键结论：Cap 新版已经从 Tauri webview 覆盖层迁移到原生 GPUI 覆盖层，说明「每个显示器一个覆盖层」是成熟形态；但它的代价是一整套原生 UI 框架。本规格 V1 复用 Orb webview，把 Cap 的交互/失败语义作为参考；V2 再评估原生覆盖层或「捕获排除 + 实时覆盖层」。

### 14.2 既有参考

| 项目 | 可参考内容 | 用途 |
|---|---|---|
| OpenBitFun | `windows_wgc_capture.rs`、`macos_capture.rs`、`AGENTS.md` | 原生捕获的失败语义与平台差异 |
| OpenBitFun | `imagePayload.ts` / `imageUtils.ts` | 截图作为「图片上下文」的前端结构 |
| airi | `services/computer-use-mcp/src/utils/screenshot.ts` | 截图压缩、尺寸上限、失败兜底 |
| openhuman | `capabilities/default.json` 的 `screen_intelligence_capture_now` | 能力命名与权限声明 |

第三方开源截图工具（Flameshot / ShareX，均为 GPL-3）只可参考交互习惯，**不可复制代码**。

### 14.3 Kiri / shiping（新增，完整克隆）

Kiri 与 shiping 需从本地参考检出中取得（`references/` 已被 `.gitignore` 忽略，不参与构建与分发）。

| 项目 | 许可证 | 可借鉴内容 | 用途 |
|---|---|---|---|
| Kiri | MIT，可复制 | `src-tauri/src/capture/mod.rs` 的 `CaptureHealth`（`expected_stop` / `unexpected_failure` 边界）与 `VIDEO_FRAME_QUEUE_CAPACITY = 2` 有界帧队列；`capture/{windows,macos,linux}.rs` 三平台实现 | Rust 抓屏错误分类与平台差异 |
| Kiri | MIT | `docs/adr/0107-windows-capture-visibility-and-pin-drag.md`（隐藏后 DWM flush 与可见性校验）、`0029` / `0047`（macOS 全屏 Space 覆盖层）、`0051` / `0052`（Linux 边界与授权恢复） | 跨平台风险与验收口径 |
| shiping | **CC BY-NC 4.0，禁止商用** | `src/main/take-screenshot.ts`（首抓空帧重试、按 DIP 尺寸 resize 后再编码）、`src/shared/capture-region.ts`（`RegionRect` / `regionToPixels`） | 只读借鉴流程与几何换算，**不得复制代码** |
| shiping | CC BY-NC 4.0 | `src/main/window-privacy.ts`（`setContentProtection` + Windows 软隐藏后重置）、`src/main/region-picker.ts`（框选窗口生命周期） | 自身窗口隐身与框选交互 |

与 rev4 的关系：Kiri ADR 0107 证明「隐藏窗口后不能只等固定延时，必须确认窗口真的不可见并 flush DWM」，这是 `SCREENSHOT_HIDE_SETTLE_MS` 之外仍需在 Windows 上补可见性校验的依据；shiping 的 `take-screenshot.ts` 说明「先抓空帧重试一次」是必要兜底。

---

## 15 rev3 修订摘要

| 章节 | rev2 | rev3 |
|---|---|---|
| §1 | 新增 `screenshot` webview + 独立覆盖层 | 复用 Orb webview；不新增窗口 |
| §4/§5 | F2/F3 需要加 viewMap/capabilities | 不新增 label，F2/F3 不再需要改动 |
| §7.2 | 5 个 DTO，57 → 62 | 3 个 DTO（会话/裁剪参数/裁剪结果），57 → 60 |
| §7.3 | `start_screenshot` 返回会话；`get_screenshot_session` 常规路径 | `start_screenshot` 返回 `()` + `screenshot-session` 事件；`get_screenshot_session` 仅兜底 |
| §7.4 | `screenshot-captured` + `screenshot-cancelled` | `screenshot-session` + `screenshot-error`；结果走 invoke 返回值 |
| §7.5 | 默认 `Ctrl+Shift+A`；固定数组注册 | 四键默认；`SHORTCUT_SPECS` 元组表 + 上游 crate 纯函数解析 + 清空/重复/非法诊断 |
| §8 | 覆盖窗口构建/关闭 | Orb 全屏/恢复快照 + 巡检/失焦/穿透守卫 |
| §9 | 新布局、跨窗口事件、`useScreenshotResult` | `OrbWindow` 条件渲染、`screenshotStore`、直接 ingest、chat 只隐藏不卸载 |
| §9.7 | 无冲突检测边界说明 | 明确只检测内部重复；外部冲突以 OS 注册失败为准，不维护静态表 |
| §10 | 新窗口文件 | 移除 `ScreenshotWindow`，新增 `orb_overlay.rs` / `screenshotStore.ts` / `useScreenshotSession.ts` |
| §14 | 无截屏参考项目 | 新增 Cap 部分快照 |

---

## 16 rev4 修订摘要

| 章节 | rev3 | rev4 |
|---|---|---|
| §1 | 抓屏时隐藏所有应用窗口 | 只隐藏覆盖层宿主 `orb` |
| §1.1 | — | 新增一行修正记录（rev3 底图与所见不符） |
| §2 目标 5 | 恢复所有窗口的可见性/焦点 | 只恢复 Orb；`main` / `settings` 全程不受影响 |
| §4 F27 | 主窗口作为被隐藏/恢复对象 | 主窗口不参与覆盖层、不被隐藏 |
| §5 不变式 17 | 抓屏前隐藏所有应用窗口 | 只隐藏 `orb`；`main` / `settings` 进入底图 |
| §6.3 | 先隐藏所有窗口 → 抓屏 | 先隐藏 `orb` → 抓屏；底图 = 用户所见 |
| §7.1 | 常量注释「隐藏所有应用窗口」 | 注释改为「隐藏 `orb`」，值仍 150ms |
| §8.2 | 隐藏 main / orb / settings | 只快照并隐藏 `orb`；补 Windows 可见性校验与空帧重试 |
| §11.3 | 期望三窗均不可见 | 主窗/设置窗保持原状并进入底图；新增「从 Orb 触发」用例 |
| §13 | — | 新增主窗遮挡风险（与系统截图工具行为一致） |
| §14 | 仅 Cap | 新增 §14.3 Kiri / shiping 对照（含许可证约束） |
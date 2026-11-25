# WebView 右键菜单接管分析

> 日期：2026-11-22｜性质：**预先分析**（只盘点现状、能力面与可选路径；**不含取舍决议**——采用哪一档、何时落地，属规格职责）
> 判据来源：`docs/principle/CODE-REVIEW-PRINCIPLE.md`、`docs/principle/CODE-SPLIT-PRINCIPLE.md`
> 事实快照：截至 2026-11-22 的仓库代码与依赖锁（`tauri 2.12.5` / `wry 0.57.1` / `webview2-com 0.39.1` / `windows 0.62.2`）。**时点记录，不随代码演进追改**，需要最新事实时以代码为准。
> 范围界定：本文只讨论「右键弹什么菜单、由谁弹」；窗口创建、几何恢复、截图链路本身不属本文，仅在交接处点到为止。
> 时效与终止：本文是**一次性决策输入**。接管方案定案后由规格承接，本文整体归档冻结。

---

## 0 命题核对

| # | 命题 | 判定 | 依据 |
|---|---|---|---|
| 1 | 应用当前到处都能看到「刷新 / 另存为 / 打印 / 检查」 | **成立** | 全仓只有 4 处右键处理点，绝大多数区域无处理，回落到 WebView2 默认菜单（§1.2） |
| 2 | 页面 `preventDefault()` 挡不住 WebView2 原生菜单，必须走 Rust | **不成立** | WebView2 默认菜单与 Edge 同源，尊重 DOM `contextmenu` 的 `defaultPrevented`；页内 `preventDefault` 即可抑制（§3.1）。现有 Orb 正是这么用的，且确实没有冒出浏览器菜单 |
| 3 | 只要加一个全局 DOM 监听就能覆盖「不要浏览器菜单」 | **成立（有边界）** | 事件委托挂 `document` 即可覆盖全部可点击区域；残留边界见 §3.1 末条 |
| 4 | 主窗口与 Orb 的窗口创建路径是同一个 | **部分成立** | 两者都在 `src-tauri/tauri.conf.json` 的 `app.windows` 里静态声明；`src-tauri/src/window.rs` 的 `show_or_create` 只负责「窗口丢失后重建」（§1.1） |
| 5 | Tauri 有现成的右键菜单开关 | **不成立** | `tauri 2.12.5` 源码内 `default_context_menus` 零命中；wry 有该开关但默认 `true`，Tauri 从未调用（§3.2） |
| 6 | 需要新增依赖才能做原生接管 | **部分成立** | `webview2-com 0.39.1` 已在依赖图中（wry 传递依赖，`Cargo.lock` 可查），但**不是直接依赖**，要在 `src-tauri/Cargo.toml` 显式声明才能 `use`（§3.3） |
| 7 | 原生菜单的语言/主题会跟随应用 | **不成立** | 原生菜单由 WebView2 runtime 绘制，语言与主题跟系统/runtime 走，不受 antd 主题与 `i18n` 控制（§2.3） |
| 8 | 关闭默认菜单后仍能收到 `ContextMenuRequested` | **待实测** | 社区实践两种说法并存，同一 runtime 版本行为不一致；因此「先关默认菜单再接管」的顺序有风险（§6.2） |
| 9 | 接管需要在每个窗口各挂一次且可重复挂 | **不成立** | `with_webview` 每次调用都会新增一个事件处理器；重建窗口时必须重挂，同一 webview 上重复挂会弹两次（§5.2） |
| 10 | 截图期间右键语义与普通窗口相同 | **不成立** | 截图覆盖层右键是「取消截图」，且此时弹出的任何菜单都会被合成进底图（§5.4） |

核对结果：三条事实决定了方案选型——(a) 默认菜单是**页面级**行为，DOM 层就能挡住；(b) Rust 层接管只在需要「原生观感 / 目标感知」时才必要；(c) 无论选哪档，都要处理**窗口重建重挂**与**截图期豁免**两个工程点。

---

## 1 现状盘点

### 1.1 窗口与创建路径

| 事实 | 位置 | 说明 |
|---|---|---|
| 窗口静态声明 | `src-tauri/tauri.conf.json` `app.windows` | 只有 `main` 与 `orb` 两个窗口；`main` 无边框、`orb` 透明置顶跳过任务栏 |
| 重建路径 | `src-tauri/src/window.rs` `show_or_create` | 仅在 `get_webview_window` 取不到时走 `WebviewWindowBuilder`；当前 `builder.build()` 的返回值被丢弃 |
| 启动装配 | `src-tauri/src/setup.rs` | `setup` 阶段窗口已存在（`position_orb_window`、`show_orb_window` 直接 `get_webview_window`），是**唯一稳定的挂载时机** |
| devtools 门 | `src-tauri/src/window.rs` | 仅 release 下 `devtools(false)`；debug 版默认菜单因此多出「检查」项 |

推论：Rust 层挂载点有两处——`setup.rs`（覆盖静态声明的两个窗口）与 `show_or_create` 的成功分支（覆盖重建窗口）。只挂一处会漏。

### 1.2 前端右键处理点（全仓 4 处）

| 位置 | 行为 | 结果 |
|---|---|---|
| `src/orb/OrbDrag.tsx` `onContextMenu` | `preventDefault()` + 手势判定 + 调 Rust `show_orb_context_menu` | 自定义原生菜单，无浏览器菜单 |
| `src/screenshot/Overlay.tsx` `onContextMenu` | `preventDefault()` + `cancel()` | 右键取消截图 |
| `src/vault/entry/RevealFieldButton.tsx` | `onContextMenu={(e) => e.preventDefault()}` | 屏蔽默认菜单 |
| `src/history/sidebar/ConversationListItem.tsx` | antd `Dropdown trigger={["contextMenu"]}` | 前端自绘菜单 |

其余区域（聊天正文、输入框外空白、设置页、侧栏空白、画布等）**没有任何右键处理**，右键一律落到 WebView2 默认菜单。这不是「接管失败」，是「根本没接管」。

### 1.3 已有的自定义菜单范式

| 范式 | 位置 | 形状 | 可复用性 |
|---|---|---|---|
| 原生菜单 + 全局分发 | `src-tauri/src/window.rs` `show_orb_context_menu` → `src-tauri/src/menu.rs` `on_menu_event` | 前端触发 → Rust 建 `Menu` → `popup_at` → 菜单 id 回全局分发 | **高**：原生接管（§4 方案 C）应复用同一分发，不另开事件通道 |
| 前端自绘菜单 | antd `Dropdown` + `trigger={["contextMenu"]}` | 组件内声明菜单项，点击即回调 | **高**：DOM 层接管（§4 方案 A）应与它统一风格 |
| 截图期菜单清理 | `crate::screenshot::orb_overlay::dismiss_menus_now` | 菜单淡出动画会污染抓屏，故主动立即隐藏 | **必须遵守**：新增任何菜单都不得绕过 |

---

## 2 WebView2 默认菜单的构成

### 2.1 按命中目标动态拼装

WebView2 不提供固定菜单清单，菜单项随「右键落在什么上」与 runtime 版本变化。常见分组：

| 目标（`COREWEBVIEW2_CONTEXT_MENU_TARGET_KIND`） | 典型菜单项 |
|---|---|
| `PAGE`（空白/普通内容） | 刷新、另存为、打印、检查（devtools 开启时） |
| `IMAGE` | 在新窗口打开图片、复制图片、复制图片地址、图片另存为 |
| `SELECTED_TEXT` | 复制、全选、在 Web 中搜索 / 朗读 / 分享（随版本） |
| `AUDIO` / `VIDEO` | 播放/暂停、静音、循环、全屏、下载、画中画（随版本） |
| 链接（`ContextMenuTarget` 带 `LinkUri`） | 在新窗口打开链接、复制链接地址、链接另存为 |
| 可编辑区（input/textarea/contenteditable） | 撤销、重做、剪切、复制、粘贴、全选、拼写建议 |

### 2.2 不要靠猜，靠枚举

任何「删掉第 N 项」的硬编码都会在 runtime 升级后失效。定案前应先跑一次枚举（`Name` 为不本地化的标识名，`Label` 为本地化显示文本）：

```rust
unsafe fn dump_menu(args: &ICoreWebView2ContextMenuRequestedEventArgs) -> windows_core::Result<()> {
    let items = args.MenuItems()?;
    let mut count = 0u32;
    items.Count(&mut count)?;
    for i in 0..count {
        let item = items.GetValueAtIndex(i)?;
        let (mut name, mut label) = (PWSTR::null(), PWSTR::null());
        let mut id = 0i32;
        item.Name(&mut name)?;      // 不本地化，适合做判据
        item.Label(&mut label)?;    // 本地化，只用于展示
        item.CommandId(&mut id)?;   // 稳定数值 id，可作为兜底判据
        log::info!("ctx item {i}: name={:?} label={:?} cmd={id}", name.to_string(), label.to_string());
    }
    Ok(())
}
```

注意：`Name` / `Label` 返回的 `PWSTR` 由 COM 分配，取完必须 `CoTaskMemFree`（需要 `windows` 的 `Win32_System_Com` 特性）；不释放就是每次右键泄漏一次。

### 2.3 两个不受应用控制的维度

- **语言**：菜单文案跟 WebView2 runtime 的 UI 语言（通常即系统语言），不随应用 `i18n` 切换。应用是中文、系统英文时菜单是英文。
- **主题**：菜单是系统级弹出窗口，跟系统亮暗，不跟 antd 主题。
- **项集合**：随 runtime 版本漂移；release 下 `devtools(false)` 会少「检查」项，debug 下会出现。

这三点是「要不要接管」的主要观感动机，也是「接管后不可能完全一致」的代价来源。

---

## 3 能力面

### 3.1 前端 DOM 层

Chromium 系（WebView2）与 WKWebView、WebKitGTK 都尊重 DOM `contextmenu` 的 `defaultPrevented`，因此这是**唯一跨三平台**的路径：

```ts
const onContextMenu = (e: MouseEvent) => {
  const el = e.target as HTMLElement | null;
  if (el?.closest("input, textarea, [contenteditable='true']")) return; // 放行编辑菜单
  e.preventDefault();
  // 依据 el / 选区 / data-* 决定弹哪个菜单
};
document.addEventListener("contextmenu", onContextMenu);
```

边界：

| 边界 | 处理 |
|---|---|
| 输入框/可编辑区 | 默认放行，保留系统复制粘贴与拼写；若要自绘编辑菜单，需自行实现剪贴板 |
| 两个窗口共用 `main.tsx` | 用 `getCurrentWebview().label` 区分 `main` / `orb`，Orb 已被 `OrbDrag` 接管，全局监听应跳过 |
| 截图期 | 与 `crate::screenshot::is_active()` 联动；覆盖层已自行 `cancel()`，全局监听不得抢先弹菜单 |
| 事件叠加 | 只挂一次（入口或单一全局 hook），组件级再挂会出现双菜单 |
| 残留 | WebView2 自身绘制的非 DOM 表面（如某些 devtools 区域）不派发 `contextmenu`；是否需要兜底（§4 方案 D）应实测确认 |

### 3.2 Tauri / wry 层

| 事实 | 结论 |
|---|---|
| `tauri 2.12.5` 源码无 `default_context_menus` | 没有声明式开关 |
| `wry 0.57.1` 的 `WebViewBuilderExtWindows::with_default_context_menus(enabled)` | 存在，默认 `true`；只能通过 `WebviewWindowBuilder` 的 wry 扩展传入 |
| Tauri 的 `WebviewWindowBuilder` 未暴露该扩展 | 重建窗口时也无法声明式关闭，只能 `with_webview` 事后改 |
| `WebviewWindow::with_webview` | 提供 `PlatformWebview`，Windows 下 `controller()` 可取 `ICoreWebView2Controller` → `CoreWebView2` |

### 3.3 webview2-com 层（Windows）

`webview2-com 0.39.1` 与仓库 `windows 0.62.2` 版本对齐（`Cargo.lock` 已锁），所需能力齐全：

| 能力 | 接口/方法 | 用途 |
|---|---|---|
| 全局开关 | `ICoreWebView2Settings::SetAreDefaultContextMenusEnabled` | 整体关闭默认菜单（方案 D） |
| 事件 | `ICoreWebView2::add_ContextMenuRequested` / `remove_ContextMenuRequested` | 接管入口，需保存 token |
| 处理器封装 | `webview2_com::ContextMenuRequestedEventHandler::create` | 闭包签名 `FnMut(Option<ICoreWebView2>, Option<...EventArgs>) -> Result<()>` |
| 菜单集合 | `EventArgs::MenuItems()` → `ICoreWebView2ContextMenuItemCollection` | `Count` / `GetValueAtIndex` / `RemoveValueAtIndex` / `InsertValueAtIndex` |
| 单项信息 | `ICoreWebView2ContextMenuItem` 的 `Name` / `Label` / `CommandId` / `Kind` / `IsEnabled` / `Children` | 判据与裁剪 |
| 命中目标 | `EventArgs::ContextMenuTarget()` → `Kind` / `LinkUri` / `SourceUri` / `SelectionText` / `HasLinkText` | 目标感知菜单 |
| 坐标 | `EventArgs::Location()` → `POINT`（webview 客户区物理坐标） | 自绘菜单定位，需 DPI 换算 |
| 抑制默认 | `EventArgs::SetHandled(true)` | 完全接管（方案 C） |
| 异步 | `EventArgs::GetDeferral()` | 菜单在异步决策完成后弹出时保命 |
| 新建项 | `ICoreWebView2Environment9::CreateContextMenuItem` | 往原生菜单里插自定义项（方案 B） |
| 回填选中 | `EventArgs::SetSelectedCommandId(id)` | 让 WebView2 执行原生动作（复制/粘贴等） |

依赖声明要求：

- `src-tauri/Cargo.toml` 的 `[target.'cfg(target_os = "windows")'.dependencies]` 增加 `webview2-com = "0.39.1"`（与 `Cargo.lock` 一致，避免拉第二份 `windows`）。
- 若用 `CoTaskMemFree`，显式补 `windows` 的 `Win32_System_Com` 特性；若用 `CreateContextMenuItem`，还需 `IStream`（同属该特性）。
- 全部实现必须包在 `#[cfg(target_os = "windows")]`，macOS/Linux 无等价 API。

---

## 4 路径对照

| 方案 | 做法 | 覆盖范围 | 观感 | 成本 | 适用 |
|---|---|---|---|---|---|
| **A 前端委托** | `document` 级 `contextmenu` + 自绘菜单（antd `Dropdown` 或自建面板） | 三平台 | 非原生，但与 antd 主题/i18n 一致 | 低（纯前端，一处注册） | 主窗口内容区、列表、设置页 |
| **B 原生裁剪** | `ContextMenuRequested` 里 `RemoveValueAtIndex` 删项，不 `SetHandled` | Windows | 原生，保留系统动作 | 中（COM + 判据维护） | 想保留原生菜单但去掉「刷新/另存为/打印」 |
| **C 原生完全接管** | `SetHandled(true)` + `Menu::popup_at` + `SetSelectedCommandId` 回填 | Windows | 原生外观、可控项集合 | 高（COM + 坐标/DPI + 生命周期） | 需要目标感知的原生菜单（链接/图片/选区） |
| **D 全局关闭** | `SetAreDefaultContextMenusEnabled(false)` | Windows | 无菜单（完全靠 A 补） | 极低（一行设置） | 只要「别出现浏览器菜单」的兜底 |

选型判据（供规格阶段使用，不在本文定案）：

- 只要「不要浏览器菜单」→ **A** 已足够，D 作为实测后的兜底。
- 要「原生观感 + 去掉少数项」→ **B**。
- 要「原生观感 + 自定义项 + 目标感知」→ **C**，且必须复用 `menu.rs` 的全局分发。
- **B/C 与 D 不要混用**：D 之后事件是否还会来，见 §6.2。

---

## 5 落地形状（若走 Rust 路径）

### 5.1 挂载点

| 场景 | 位置 | 说明 |
|---|---|---|
| 静态声明的 `main` / `orb` | `src-tauri/src/setup.rs`，窗口已存在之后 | 遍历目标 label，逐个 `with_webview` |
| 重建窗口 | `src-tauri/src/window.rs` `show_or_create` 成功分支 | 需先把 `builder.build()` 的返回值接住，再挂 |
| 挂载函数归属 | 新增 `src-tauri/src/context_menu.rs` 或并入 `window.rs` | 与 `menu.rs` 的分发职责分开：一个管「弹什么」，一个管「点了做什么」 |

### 5.2 生命周期

- 每个 `ICoreWebView2` 只注册一次；保存 `add_ContextMenuRequested` 返回的 token，窗口销毁时 `remove` 或随 webview 一并释放。
- `with_webview` 是「注册」而非「设置」，重复调用会叠加处理器 → 同一次右键弹两次菜单。重建窗口是新 webview，重挂是必须的。
- 处理器闭包捕获的 `WebviewWindow` / `AppHandle` 必须 `Send + 'static`；实际执行在主线程。

### 5.3 坐标与弹出

- `Location()` 是 webview 客户区**物理**坐标；`Menu::popup_at` 接收窗口内**逻辑**坐标，需按 `scale_factor` 换算（参照 `show_orb_context_menu` 的用法）。
- `popup_at` 内部是 `TrackPopupMenu`，会阻塞消息循环；在事件回调里同步弹是可行的，但任何异步决策（如先算菜单项）都必须用 `GetDeferral()` 包住，完成后 `Complete()`。
- 高 DPI 多屏要实弹校准一次；不要用 `Location` 直接当逻辑坐标。

### 5.4 与截图链路交接

- 截图期间（`crate::screenshot::is_active()`）应直接放行或抑制菜单：弹出的任何顶层菜单都会进入底图。
- 若在截图期接管，必须像 Orb 菜单那样在点击瞬间调 `dismiss_menus_now()`。
- 透明窗口（`orb`）上的原生菜单是独立顶层窗口，`#32768` 清理逻辑不能绕过。

### 5.5 菜单项与 i18n

- 自建项文案走 `assistant_i18n::t(...)`，与 Orb 菜单一致。
- 若做 B（裁剪），判据优先 `Name`（不本地化），`CommandId` 兜底，**不要用 `Label` 匹配**——`Label` 随系统语言变化。
- 菜单 id 用统一前缀（如 `ctx_*`）并复用 `menu::on_menu_event` 的 `match`，不要新建第二套事件通道。

---

## 6 风险与待实测

### 6.1 重复注册

`setup.rs` 与 `show_or_create` 都挂、又没做去重时，重建窗口会叠加处理器。判据：同一次右键只应出现一个菜单；日志里处理器触发次数应为 1。

### 6.2 「先关默认菜单」的顺序风险

社区两种说法并存：一种认为 `AreDefaultContextMenusEnabled=false` 后 `ContextMenuRequested` 不再触发（此时只能走 JS `contextmenu` + WebMessage 回传）；另一种认为仍会触发、可用 `SetHandled` 替换。**同一 runtime 版本下也可能不一致**。

因此 B/C 的安全顺序是：保持默认菜单开关为 `true`，在事件里 `SetHandled(true)` 抑制；D 只在「确认事件不再需要」或「纯兜底」时使用。上 B/C 前必须先打一行日志验证事件确实到达。

### 6.3 平台与版本

- macOS/Linux 无 `ContextMenuRequested` 等价物；WKWebView 的右键菜单不在应用控制范围内，DOM 层是唯一路径。
- `webview2-com` 必须与 wry 使用的版本一致；Tauri 文档明确提示 `with_webview` 下应把 Tauri 锁到至少 minor 版本。
- runtime 升级会改变默认项集合，B 的删除判据要允许「未命中即跳过」并记日志，不能报错。

### 6.4 观感不可完全对齐

原生菜单的语言/主题由系统决定，与 antd 主题、应用 i18n 不可能完全一致。若产品要求「菜单与应用同语言同主题」，只能选 A（自绘）。

### 6.5 键盘可达性

自绘菜单需补：`Escape` 关闭、方向键导航、屏幕边缘翻转、点击外部关闭。缺一项就比原生菜单难用。

---

## 7 待定案项

| # | 待定 | 影响 |
|---|---|---|
| 1 | 接管范围：只 `main`，还是 `main` + `orb`（`orb` 已被 `OrbDrag` 接管，是否叠加全局监听） | 决定挂载点与跳过条件 |
| 2 | 观感目标：antd 风格自绘，还是原生菜单 | 决定 A 还是 B/C |
| 3 | 编辑区策略：放行系统菜单，还是自绘（需自实现剪贴板） | 决定 DOM 层放行白名单 |
| 4 | 是否保留「复制/全选/粘贴」等原生动作 | 决定 C 是否要用 `SetSelectedCommandId` 回填 |
| 5 | debug 版是否也要屏蔽「检查」 | 决定是否额外按 `devtools` 状态分支 |
| 6 | 截图期右键语义（取消 vs 菜单） | 决定 `is_active()` 分支 |

---

## 8 验收要点

| # | 判据 |
|---|---|
| 1 | 主窗口任意空白处右键，不再出现「刷新/另存为/打印/检查」 |
| 2 | 输入框内右键仍可复制/粘贴（或自绘菜单功能等价） |
| 3 | 一次右键只出现一个菜单（无叠加） |
| 4 | 窗口销毁后重建（托盘「显示主窗口」等路径）右键行为与首次一致 |
| 5 | 截图进行中右键仍为「取消截图」，且菜单不会进入底图 |
| 6 | Orb 右键行为与现状一致，未被全局监听改变 |
| 7 | 菜单文案随应用语言切换；原生方案下明确记录「不随语言」的例外 |
| 8 | release 与 debug 行为差异有记录（「检查」项） |
| 9 | macOS/Linux 构建不因 Windows 专属代码失败（`cfg` 门生效） |

---

## 9 参考资料

- 微软官方：在 WebView2 中自定义上下文菜单（`ContextMenuRequested`、`MenuItems`、`CreateContextMenuItem`、`SetSelectedCommandId`）—— <https://learn.microsoft.com/zh-cn/microsoft-edge/webview2/how-to/context-menus>
- 微软官方：WebView2 功能与 API 概览（`AreDefaultContextMenusEnabled`、`ContextMenuRequested`）—— <https://learn.microsoft.com/zh-cn/microsoft-edge/webview2/concepts/overview-features-apis>
- wry `WebViewBuilderExtWindows::with_default_context_menus` —— <https://docs.rs/wry/0.57.1/wry/trait.WebViewBuilderExtWindows.html>
- Tauri `WebviewWindow::with_webview`（`PlatformWebview` 各平台用法）—— <https://docs.rs/tauri/2.12.5/tauri/webview/struct.WebviewWindow.html#method.with_webview>
- webview2-com `ContextMenuRequestedEventHandler` / `CustomItemSelectedEventHandler` —— <https://docs.rs/webview2-com/0.39.1/webview2_com/>
- .NET 混合开发系列：自定义 WebView2 上下文菜单（含「页内 JS 亦可禁用」与「关闭默认菜单后事件是否触发」的实践说明）—— <https://cloud.tencent.com/developer/article/2001162>
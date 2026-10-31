# 主窗口小窗（Mini）模式分析

> 日期：2026-10-31｜性质：**纯分析文档**，不含执行计划，不修改代码
> 前提约束：**不新增窗口**，目标是以最少内存与资源开销提供"工具悬浮小窗"形态
> 范围：Tauri 窗口层（`src-tauri/src/window.rs`）+ 主窗口前端外壳（`src/layouts/MainLayout.tsx`）+ 工具页（`src/tools/`）

---

## 0 需求澄清（先纠正一处误读）

此前分析中把"下次启动是否以小窗打开"列为待确认项，容易被读成"建议 mini 作为默认形态"。**这不是本意，也不是合理设计**。正确表述：

| 项 | 正确定义 |
|---|---|
| 默认形态 | **完整主窗口**（`normal`），960×640，带侧栏，工具箱全貌 |
| mini 是什么 | **工具悬浮小窗**：用户主动切换的临时呈现态，用于把当前工具变成常驻置顶的小窗口，边用边看 |
| mini 何时出现 | **按需触发**（标题栏按钮 / 托盘 / 快捷键 / Orb 菜单），用完可退回完整窗口 |
| mini 是否记忆 | 默认**不记忆**，每次启动都从 `normal` 开始；是否记忆作为可选设置项，由用户显式开启 |
| mini 生命周期 | 与 `main` 窗口共用同一 webview，只是窗口参数与 React 外壳变化，**不新建窗口、不新建 webview** |

一句话：**mini 是"工具的一种使用形态"，不是应用的启动形态。**

---

## 1 为什么必须复用 `main` 窗口

### 1.1 新增窗口的真实开销

新增一个 label（例如 `mini`）在 Tauri 2 下不是"再画一个框"，而是整套前端运行时再来一遍：

| 开销项 | 说明 | 复用 main | 新增窗口 |
|---|---|:-:|:-:|
| WebView2 / WKWebView 实例 | 每个窗口一个独立 webview，独立渲染进程或独立渲染上下文 | 0 | +1 |
| JS 运行时 + V8 堆 | 第二个 React 应用实例、第二份模块图 | 0 | +1 |
| React 组件树 | `MainLayout` 全量挂载（侧栏、路由、CommandPalette） | 0 | +1 |
| Zustand store 实例 | `toolStore` / `settingsStore` / `providerStore` 等每窗口独立，需跨窗口同步才一致 | 0 | +N |
| 静态资源 | Vite 产物虽可缓存，但组件与 antd 运行时对象需重新实例化 | 0 | +1 份运行时 |
| IPC 与事件 | 需接入 `crossWindowEvent` / `emitSettingEventToPeers` 的全套同步，否则状态漂移 | 0 | 必须补齐 |
| 生命周期 | 需新增 `WindowSpec`、capability 授权、`CloseRequested` / `Focused` 分支、销毁逻辑 | 0 | 必须补齐 |

> 以上是**结构性开销**，不是精确字节数。具体数值（进程数、工作集、JS 堆）需在第 9 章的方法下实测，本文件不给出未经验证的数字。

### 1.2 复用的额外收益（比内存更重要）

1. **工具输入状态天然保留**：`/tools/:id` 页面内的输入框内容、计算结果、滚动位置在切换形态时**不丢失**。若新开窗口，等于要求用户重做一遍操作。
2. **无状态同步成本**：收藏、最近使用、搜索词、当前工具全部在同一个 store 里，切形态不会出现"两个窗口收藏不一致"。
3. **无新增 capability 授权面**：`src-tauri/capabilities/default.json` 现有授权已覆盖 `set-size` / `set-position` / `set-always-on-top` / `set-skip-taskbar` / `set-decorations` / `start-dragging` 等，够用。
4. **路由复用**：`App.tsx` 按 `webview.label` 选择布局，`main` 只有一套路由树；小窗只是换外壳，不用维护两套路由。

### 1.3 复用必须接受的代价

| 代价 | 影响 | 缓解 |
|---|---|---|
| 不能同时存在"完整主窗口 + 悬浮小窗" | 用户无法一边看完整工具箱一边用小窗 | 这正是"省内存"的必然取舍；需要并存时再评估独立窗口 |
| 切换瞬间窗口尺寸跳变 | 可能出现内容重排闪烁 | 切换前先 `set_size` 再切前端外壳，或反之，需固定顺序（见 5.3） |
| 小窗状态污染完整窗口 | 例如小窗里选了工具，退回完整窗口后仍停在该工具页 | 这是期望行为（保留上下文），不是缺陷 |
| 单一窗口 = 单点 | 小窗崩溃影响主窗口 | 已有 `FeatureErrorBoundary` / `AppErrorBoundary` 兜底 |

---

## 2 现状盘点（代码事实）

### 2.1 已有能力

| 能力 | 位置 | 对小窗的意义 |
|---|---|---|
| 主窗口已无边框 | `src-tauri/tauri.conf.json` 中 `main` 的 `decorations: false` | 小窗外观可直接复用自绘标题栏，无需改窗口装饰 |
| 主窗口最小尺寸 300×200 | `src-tauri/crates/assistant-config/src/constant.rs` | 小窗尺寸（约 380×560）落在合法区间，**无需改 min_size** |
| 窗口动态创建 | `src-tauri/src/window.rs` 的 `WINDOW_SPECS` + `show_or_create` | 已有"按 label 建窗"的统一入口，扩展模式参数成本低 |
| 主窗口置顶已存在 | `src-tauri/src/cmd/config.rs` 的 `stay_on_top` side effect；`src/components/PinToggle.tsx` | 小窗"默认置顶"有现成开关链路，但存在双真值冲突（见 7.1） |
| 关闭到托盘 | `src-tauri/src/lib.rs` 的 `CloseRequested` 分支 | 小窗关闭语义可直接沿用 |
| 跨窗口导航事件 | `src-tauri/src/cmd/app.rs` 的 `open_main_window_and_navigate` + `src/hooks/useNavigationListener.ts` | 托盘跳设置页已有链路，小窗也可复用 |
| 自绘标题栏 | `src/components/Titlebar.tsx`，`data-tauri-drag-region` 在 `src/assets/styles/global.css` | 小窗标题栏只需精简，不需重建拖拽机制 |
| 主窗口 toggle | `src-tauri/src/window.rs` 的 `toggle`，被 `toggle_main` 快捷键调用 | 小窗与"显示/隐藏"是两种语义，需区分（见 7.4） |

### 2.2 明确缺失

| 缺失项 | 现状 | 影响 |
|---|---|---|
| 运行时"呈现模式"概念 | 无。窗口形态完全由创建时的 `WindowSpec` 决定 | 需要引入 `normal / mini` 运行时状态 |
| 主窗口几何持久化 | 无。`save_orb_position` 只存 Orb；主窗口每次 `center: true` | 小窗位置/尺寸无处存，需新增存储 |
| 小窗布局外壳 | 无。`MainLayout` 固定 Titlebar + Sidebar + Content | 需要外壳分叉 |
| 工具页密度适配 | `ToolPage` 硬编码 `padding: 24px 32px`、标题 `fontSize: 28` | 小窗里空间浪费严重 |
| 小窗首页形态 | `ToolBox` 是完整工具箱（分类 + 收藏 + 最近 + 新品） | 小窗装不下，需要"启动器"形态 |
| 宽工具降级策略 | `widthMode` 仅影响 `maxWidth`，无小窗提示 | SQL/Diff/SVG 在小窗里必然局促 |

---

## 3 模式定义

### 3.1 三态而非两态

```
            ┌────────── hidden（托盘/关闭到托盘） ──────────┐
            │                                              │
            ▼                                              │
        ┌────────┐   enter_mini()   ┌────────┐             │
        │ normal │ ───────────────► │  mini  │             │
        │        │ ◄─────────────── │        │             │
        └────────┘   exit_mini()    └────────┘             │
            │                              │               │
            └──────────── hide() ──────────┴────── hide() ──┘
```

- `hidden` 是既有概念（`close_to_tray` 生效时 `hide()`），不是新模式。
- `normal` 与 `mini` 是**同一个窗口的两种呈现**，互斥。
- 模式切换**不重建窗口**，只改窗口参数 + 前端外壳。

### 3.2 触发点矩阵

| 触发 | 进入 mini | 退出 mini | 说明 |
|---|:-:|:-:|---|
| 标题栏"小窗"按钮（normal 态） | ✅ | — | 携带当前 `location.pathname` |
| 小窗标题栏"展开"按钮 | — | ✅ | 恢复 normal 几何 |
| 托盘菜单"以小窗打开" | ✅ | — | 需新增菜单项（`src-tauri/src/tray.rs`） |
| Orb 右键"以小窗打开" | ✅ | — | 需新增菜单项（`src-tauri/src/window.rs` 的 `show_orb_context_menu`） |
| 全局快捷键（可选新 id `toggle_main_mini`） | ✅ | ✅ | 与 `toggle_main` 区分 |
| 打开 `/settings/*`、`/history`、`/vault` | — | ✅ 强制 | 这三类页面在小窗里不可用（见 7.3） |
| 打开 `widthMode: wide/full` 的工具 | — | ✅ 建议强制 | 或改为"提示 + 一键展开"，需产品决策 |

**核心规则：模式跟随场景。** 用户点"设置/历史/保险库"时自动展开，比让用户手动点"展开"更顺；反过来，从小窗进入这些页面后，用户不会期待它继续缩在角落。

### 3.3 模式状态放哪里

| 候选 | 评价 |
|---|---|
| 新增 `settingsStore` 字段（`AppConf`） | ❌ 不适合。模式是窗口呈现态，不是用户偏好；写配置会带来不必要的磁盘 IO 与跨窗口广播 |
| 新增独立 `windowModeStore`（Zustand） | ✅ 推荐。只存 `mode` 与切换 action，不落盘 |
| 挂在 `toolStore` | ❌ 职责不符 |
| Rust 端持有 | ✅ 必要。窗口参数变更必须在 Rust 执行，Rust 侧需知道当前模式以正确解释 `stay_on_top`（见 7.1） |

建议：**前端 `windowModeStore` 负责 UI 外壳，Rust 侧持有一份 `AtomicBool`/`Mutex<Mode>` 负责窗口参数与置顶语义**，二者通过命令同步。这与 Orb 的 `click_through` 双端缓存（`AtomicBool` + `orbStore.clickThrough`）是同一套模式，项目内已有先例。

---

## 4 窗口参数矩阵

| 参数 | normal | mini | 运行时可否切换 | 备注 |
|---|---|---|---|---|
| `width` | 960（记忆值） | 约 380~420 | ✅ `set_size` | 小窗宽度建议锁定 |
| `height` | 640（记忆值） | 约 520~600 | ✅ `set_size` | 小窗高度允许用户拉伸 |
| `min_width` / `min_height` | 300×200 | 320×420 | ⚠️ `set_min_size` 需确认 capability | 小窗禁止拖到畸形尺寸 |
| `resizable` | true | true（宽度用 min_size 锁住） | ✅ `set_resizable`（需授权） | 若授权成本高，改用 min_size + max_width 约束 |
| `maximizable` | true | false | ✅ `set_maximizable`（需授权） | 小窗"最大化"语义应替换为"展开" |
| `always_on_top` | 跟随 `stay_on_top` | **强制 true** | ✅ `set_always_on_top`（已授权） | 见 7.1 双真值处理 |
| `skip_taskbar` | false | 建议 true | ⚠️ 平台差异，需实测 | Windows 运行时切换可能需 `hide()` + `show()` 才生效 |
| `decorations` | false | false | ✅ 无需变更 | 已是无边框 |
| `center` | 首次创建居中 | 不适用 | 创建期参数 | 小窗用记忆位置 |
| `transparent` | false | **保持 false** | ❌ **仅创建期可设** | 这是关键限制，见 8.1 |
| `shadow` | 默认 | 默认 | ❌ 创建期 | 无边框窗口在 Windows 11 自带圆角与阴影 |

> capability 缺口提示：`src-tauri/capabilities/default.json` 当前未列 `set-resizable` / `set-maximizable` / `set-min-size`。**推荐全部窗口操作走 Rust 命令（cmd 薄壳）**，前端只 `invoke`，从而完全不动 capability 授权面。这与项目"cmd 层薄壳 + Rust 执行业务"的既有分层一致。

---

## 5 前端改造点

### 5.1 外壳分叉，而不是路由分叉

`src/layouts/MainLayout.tsx` 当前是固定结构：

```
Layout > Titlebar > (Layout > AppSidebar + Layout.Content > Routes)
```

改为按模式渲染两种外壳，**`Routes` 子树保持同一份**：

| 模式 | 外壳 |
|---|---|
| `normal` | `Titlebar` + `AppSidebar` + `Content`（现状） |
| `mini` | `MiniTitlebar` + `Content`（无侧栏） |

要点：

- `AppSidebar` 整个组件不挂载，而非"宽度设 0"。不挂载才能真正省掉 antd `Menu` 的构建成本。
- `useAppInitialization` / `useErrorNotifications` / `useNavigationListener` / `CommandPalette` 保持在 `MainLayout` 顶层，两种模式都生效（其中 `CommandPalette` 在小窗需要特殊处理，见 5.6）。
- 建议用 `data-window-mode="mini"` 属性挂在根节点，便于 CSS 差异化，与项目现有 `div[data-orb-window]` 的属性驱动风格一致。

### 5.2 小窗标题栏

`src/components/Titlebar.tsx` 现在固定放了 `PinToggle` + `SettingsEntry` + `ThemeToggle` + `WindowControls`。小窗需要另一套：

| 区域 | 小窗内容 |
|---|---|
| 左侧 | 当前工具图标 + 工具名（可点击唤出工具切换） |
| 中部 | 拖拽区（`data-tauri-drag-region`） |
| 右侧 | 工具切换按钮（下拉/紧凑列表）、展开按钮、关闭按钮 |

**不要在小窗里放 `PinToggle`**：小窗强制置顶，放图钉会让用户以为可以取消置顶却没效果（见 7.1）。
**不要在小窗里放 `ThemeToggle` / `SettingsEntry`**：这两个都属于"完整窗口"语境，小窗里点击应自动展开回 normal。

小窗窗口控制按钮语义需重定义：

| 图标 | normal 行为 | mini 行为 |
|---|---|---|
| `MinusOutlined` | 最小化 | 最小化（或改为"隐藏到托盘"） |
| `BorderOutlined` | 最大化 | **改为"展开"（退出 mini）** |
| `CloseOutlined` | 关闭（走 `close_to_tray`） | 同左 |

### 5.3 工具页密度适配

`src/tools/ToolPage.tsx` 的硬编码需要按模式降级：

| 元素 | normal | mini | 说明 |
|---|---|---|---|
| 外层 padding | `24px 32px` | `10px 14px` | 小窗里 32px 侧边距太奢侈 |
| 标题 `fontSize` | 28 | 18 | 小窗标题栏已显示工具名，正文标题可再降级或隐藏 |
| 返回按钮 | 保留 | 改为"工具切换" | 小窗的返回语义是回启动器 |
| 描述文案 | 显示 | 隐藏或单行省略 | 小窗垂直空间紧张 |
| 分割线 | 200×2 | 隐藏 | 纯装饰 |

判定方式建议用**容器宽度**而不是视口断点：小窗宽度固定，用 `matchMedia` 或简单的 `mode` 判断都行；但工具内部组件的响应式最好继续依赖 antd `Col` 的 `xs/sm/md/lg`，窄宽度下自动单列，避免二次改造。

另外 `src/config/constants.ts` 的 `TOOL_DEFAULT_MAX_WIDTH = 640` / `TOOL_WIDE_MAX_WIDTH = 960` 在小窗中无意义（窗口本身更窄），需要在小窗下跳过 `maxWidth` 约束。

### 5.4 小窗首页形态

不建议把小窗首页做成完整工具箱（`src/tools/ToolBox.tsx` 是分类 + 收藏 + 最近 + 新品四段式，在 380 宽下会非常拥挤）。小窗首页建议是"工具启动器"：

```
┌─────────────────────────────┐
│ [搜索框]                     │
│ ★ 收藏    ⏱ 最近            │   ← 横向 chip，点击直接进入工具
│ ┌─────────────────────────┐ │
│ │  （若已有当前工具）       │ │   ← 直接显示该工具，省一次点击
│ │   工具内容占满剩余空间    │ │
│ └─────────────────────────┘ │
└─────────────────────────────┘
```

可复用现有 `toolStore` 的 `favoriteToolIds` / `recentTools` / `search`，无需新数据源。`src/tools/ToolSearchBar.tsx` 可直接复用（已是受控组件 + store 驱动）。

**关键体验点**：从小窗进入后，若上次停留在某工具页，下次唤起小窗应直接进入该工具（记住 `lastToolId`），而不是每次都回到启动器。这只需在 `toolStore` 增加一个 `lastOpenedToolId`（或复用 `recentTools[0]`）。

### 5.5 `widthMode` 的处理（必须显式决策）

`src/tools/types.ts` 中 `widthMode: "default" | "wide" | "full"`，注册表里 `wide` 有 3 个、`full` 有 2 个（`src/tools/registry.ts`）。三种可选策略：

| 策略 | 行为 | 优点 | 缺点 |
|---|---|---|---|
| A. 提示 + 一键展开 | 小窗中显示"该工具建议在完整窗口使用"+ 展开按钮 | 尊重用户选择 | 用户可能忽略提示，体验差 |
| B. 自动展开 | 进入 wide/full 工具时自动退出 mini | 体验顺滑 | 用户"刚切到小窗"的意图被推翻 |
| C. 禁止 | 小窗中灰掉 wide/full 工具 | 语义最干净 | 高频工具（SQL/Diff）无法小窗使用 |

建议 **B + 可撤销**：自动展开时给一个一次性提示"已为你展开，因该工具需要更宽空间"，允许用户点"仍在小窗打开"。实现上只需在 `ToolPage` 里读 `tool.widthMode` 与当前 `mode`。

### 5.6 `CommandPalette` 在小窗

`src/components/commandPalette/CommandPalette.tsx` 是覆盖层，内容宽度与条目数量按完整窗口设计。小窗下：

- 空间不足，列表会溢出
- 命令中包含"打开设置""打开历史"等，触发后会自动退出 mini，与 5.5 一致

建议小窗下**保留但收缩**（限制展示条数，复用已有的 `COMMAND_PALETTE_MAX_PER_CATEGORY`），而不是直接禁用——命令面板是键盘用户的高频入口。

---

## 6 几何与持久化

### 6.1 两套几何，分开存

| 键 | 内容 |
|---|---|
| `main.geometry.normal` | `{ x, y, width, height }`（上次完整窗口） |
| `main.geometry.mini` | `{ x, y, height }`（小窗宽度固定，不存） |

必须分开的原因：用户在小窗里拖到屏幕角落，退回完整窗口时不能继承那个位置（否则完整窗口跑到角落）；反之亦然。

### 6.2 坐标系

- **存储用逻辑像素**（`LogicalSize` / `LogicalPosition`），与 `src/orb/OrbCoord.ts` 的 `physicalToLogical` / `logicalToPhysical` 约定一致。
- **恢复时乘 `scale_factor` 转物理像素**，参考 `src-tauri/src/window.rs` 中 `expand_orb_panel` 的换算写法。
- **边界收敛用工作区而非整屏**：`assistant_orb::placement` 已有 `get_available_screens` + `ensure_orb_on_screen`，可抽出等价的"主窗口几何收敛"函数，避免小窗恢复到任务栏下方或屏幕外。

### 6.3 存放位置

| 数据 | 建议位置 | 理由 |
|---|---|---|
| 两套几何 | 新增 `WindowConf` 配置文件（或 `AppConf` 新字段） | 需要 Rust 侧在窗口创建前读取，前端 localStorage 读不到 |
| 当前模式 | 仅内存（Rust `Mutex<Mode>` + 前端 store） | 呈现态，不需要跨启动记忆 |
| "启动时进入 mini" | **不提供**（默认 false） | 见第 0 章；mini 不是启动形态 |
| `lastToolId` | `toolStore` + 现有最近使用机制 | 已有 `recordUsage`，直接复用 `recentTools[0]` 即可 |

**若最终决定把"记住上次模式"做成用户设置项**，按项目规则必须同步三处：
`src/settings/core/types/confTypes.ts` 的 `AppConf` 接口 + `src-tauri/crates/assistant-config/src/app_conf.rs` 的 struct（含 `#[serde(default)]`）+ `src/settings/core/field/fieldDefinitions.ts` 的字段定义。
但**本分析建议不引入这个设置项**，mini 始终按需进入。

---

## 7 与既有机制的冲突（最易翻车的地方）

### 7.1 `stay_on_top` 双真值

`src-tauri/src/cmd/config.rs` 的 side effect 在配置变更时直接对 `main` 窗口 `set_always_on_top(conf.stay_on_top)`。小窗若强制置顶，会出现：用户把 `stay_on_top` 关掉 → side effect 把窗口置顶取消 → 但小窗本该置顶。

建议定义单一求值规则：

```
effective_on_top = (mode == mini) || stay_on_top
```

- 所有 `set_always_on_top` 调用统一走这个求值函数，不再直接读 `conf.stay_on_top`。
- 退出 mini 时重新求值一次（若 `stay_on_top=false` 则取消置顶）。
- 小窗内隐藏 `PinToggle`，避免用户在小窗里尝试取消置顶。

### 7.2 `close_to_tray` 语义

`src-tauri/src/lib.rs` 的 `CloseRequested`：`main` 窗口关闭时若 `close_to_tray` 为 true 则 `hide()`。小窗沿用同一逻辑是对的，但需要在 UI 上区分两个按钮：

| 按钮 | 语义 | 调用 |
|---|---|---|
| 关闭（`X`） | 隐藏到托盘（若 `close_to_tray`） | `window.close()` |
| 展开 | 退出 mini 回 normal | 自定义命令 `set_main_window_mode("normal")` |

不要把小窗的"关闭"做成"退回完整窗口"——用户点 `X` 的预期是"关掉"。

### 7.3 页面适用性

| 路由 | 小窗可用性 | 处理 |
|---|---|---|
| `/`（启动器） | ✅ | 小窗首页 |
| `/tools/:id` | ✅（`default` 宽度） | 核心场景 |
| `/tools/:id`（`wide/full`） | ⚠️ | 见 5.5 |
| `/history` | ❌ | 强制退出 mini |
| `/vault` | ❌ | 强制退出 mini（保险库条目列表需要宽度） |
| `/settings/*` | ❌ | 强制退出 mini |

实现位置：`MainLayout` 的 `useNavigationListener` 与路由变更处统一判断 `location.pathname`，命中"必须 normal"的路径时调用 `exit_mini()`。注意托盘 `settings_general` 等菜单已走 `show_or_create` + `emit_broadcast(NAVIGATE)`，天然会触发这条规则。

### 7.4 与 `toggle_main` 快捷键

`src-tauri/src/setup.rs` 的 `toggle_main` 调用 `crate::window::toggle`，其语义是"可见且聚焦则隐藏，否则显示并聚焦"。这与模式无关，是正确设计——**不要**让 `toggle_main` 变成"切换 mini/normal"，否则用户按两次快捷键得到的是"隐藏/显示"还是"形态切换"会混乱。

若需要形态切换快捷键，**新增独立 id**（例如 `toggle_main_mini`），并在 `src/settings/core/types/shortcutTypes.ts` 的 `SHORTCUT_DEFINITIONS` 与 `DEFAULT_KEYBINDINGS` 同步登记，否则设置页的快捷键列表不会出现该项。

### 7.5 Orb 的层级与遮挡

Orb 窗口 `alwaysOnTop: true` 且 `skipTaskbar: true`。小窗置顶后两者会争抢 z-order：

- 小窗通常更"大"，Orb 更"小"，系统置顶窗口之间按最近激活顺序叠放，可能出现 Orb 盖住小窗标题栏。
- 建议：进入 mini 时若 Orb 处于展开态，先收起 Orb 面板（`collapse_orb_panel`）；位置计算上让 mini 的默认落点避开 Orb 的 outer bounds（`ensure_orb_on_screen` 已有屏幕边界工具，可扩展为"避开 Orb 矩形"）。

### 7.6 拖拽与点击区域

`src/assets/styles/global.css` 的 `[data-tauri-drag-region]` 已处理"拖拽区内的 input/button 不拖拽"。小窗标题栏复用同一机制即可，但要注意：

- 小窗标题栏高度建议略降（38 → 32~34），因为小窗本身矮。
- 若标题栏放下拉菜单（工具切换），antd `Dropdown` 的浮层在极窄窗口下可能被裁剪，需要设 `getPopupContainer` 到窗口根节点或 `placement` 自适应。

---

## 8 平台与运行期限制

### 8.1 `transparent` 不可运行时切换（关键）

Tauri 的 `transparent` 是**创建期参数**，运行时无对应 setter。这意味着：

- **不能**做"完整窗口不透明、小窗半透明/毛玻璃"这种形态差异。
- 若确实需要半透明小窗，只能二选一：① 创建时即 `transparent: true` 并让完整窗口也用不透明背景（但透明窗口在 Windows 上会失去系统阴影与圆角，且需处理 `background: transparent` 的绘制成本）；② 放弃半透明。
- **本分析建议：保持 `transparent: false`**。无边框 + Windows 11 的系统圆角已足够"悬浮感"，而透明窗口会引入 WebView 合成层开销，与"省资源"目标冲突。

### 8.2 `skip_taskbar` 运行期生效差异

Windows 上 `set_skip_taskbar` 在窗口已显示时可能不立即刷新任务栏按钮。稳妥做法：进入 mini 时 `set_skip_taskbar(true)` 后再 `hide()` + `show()` 一次（参考 Orb 切换 `set_ignore_cursor_events` 的"值相同则跳过系统调用"缓存策略，避免频繁调用）。

若实测发现闪烁明显，则**放弃 `skip_taskbar`**，保持 `false`——任务栏多一个图标不是致命问题，闪烁是。

### 8.3 多屏与 DPI

- 小窗记忆位置可能落在已断开的副屏 → 恢复时必须做边界收敛（见 6.2）。
- 跨 DPI 显示器拖动后 `scale_factor` 变化，几何需按逻辑像素存储才能正确还原。
- `outer_position()` 是物理像素，`set_position` 接受 `LogicalPosition`，混用会导致偏移——`src/orb/OrbCoord.ts` 已明确这条约定，主窗口改造应沿用。

### 8.4 最小尺寸与内容溢出

小窗宽 380 时，部分工具内部组件（如正则测试器的多列布局、文本 Diff 的双栏）会横向溢出。这不是窗口问题，而是工具组件问题：

- 短期：小窗下允许横向滚动（`overflow: auto` 已存在）。
- 中期：给 `ToolDefinition` 增加 `miniFriendly?: boolean` 元数据，让注册表显式标注"是否适合小窗"，比靠 `widthMode` 推断更准（`widthMode` 表达的是"需要多宽"，不是"能否压缩"）。

---

## 9 需要实测的指标

本文件不给未经验证的数字。落地前应实测以下指标，作为"复用窗口"决策的验证：

| 指标 | 方法 |
|---|---|
| 进程数 | 任务管理器 / `Get-Process` 查看 WebView2 相关进程数量；对比"新增 mini 窗口"方案 |
| 工作集内存 | 分别记录 `normal` 态、`mini` 态、以及"假设新开窗口"的工作集 |
| JS 堆 | WebView2 开发者工具 → Memory 面板，记录 `normal` 与 `mini` 的 usedJSHeapSize |
| 切换耗时 | 记录 `enter_mini` / `exit_mini` 从命令发出到 `set_size` 完成的耗时 |
| 闪烁次数 | 目视 + 录屏逐帧检查切换过程中内容重排 |
| 置顶层级 | 与 Orb、其他置顶应用（如聊天工具）叠放测试 |

**注意**：`mini` 态省下的内存主要来自"不挂载 `AppSidebar` + 不加载未使用工具组件"，而不是"少一个 webview"。真正省 webview 的是"不新增窗口"这一前提本身。因此验证重点应放在：**复用窗口相对新开窗口节省了多少**，以及**mini 外壳相对 normal 外壳又节省了多少**。

---

## 10 待确认决策点

| # | 问题 | 建议默认 | 影响面 |
|---|---|---|---|
| D1 | 小窗宽度固定还是可调 | **固定 380~420** | 窗口参数、工具布局 |
| D2 | 小窗关闭 = 隐藏托盘 还是 退回完整窗口 | **隐藏托盘**（`X`）；退回用独立"展开"按钮 | `CloseRequested` 语义、UI |
| D3 | 是否记住上次模式 | **不记住**，始终 normal 启动 | 配置项、启动流程 |
| D4 | 进入 `/settings` `/history` `/vault` 是否强制展开 | **强制展开** | 导航拦截逻辑 |
| D5 | `wide/full` 工具在小窗如何处理 | **自动展开 + 可撤销提示** | `ToolPage`、`widthMode` |
| D6 | 小窗是否 `skip_taskbar` | **先不启用**，实测后再定 | 平台差异 |
| D7 | 小窗内是否允许"取消置顶" | **不允许**，小窗恒置顶 | `stay_on_top` 求值 |
| D8 | 是否允许"完整窗口 + 小窗"并存 | **不允许**（省内存前提） | 窗口模型 |

D3 是关键：它决定了"mini 是否为默认形态"这个问题不存在——**mini 永远不是默认**。

---

## 11 分阶段落地建议

### P0 最小可用

1. 引入运行时模式（Rust `Mutex<Mode>` + 前端 `windowModeStore`），命令 `set_main_window_mode(mode)`。
2. 主窗口几何持久化（两套键 + 逻辑像素 + 工作区收敛）。
3. `MainLayout` 外壳分叉：mini 态不挂载 `AppSidebar`。
4. `MiniTitlebar`：工具名 + 拖拽区 + 工具切换 + 展开 + 关闭。
5. `ToolPage` 密度降级（padding / 标题 / 隐藏描述与分割线）。
6. 标题栏"小窗"按钮 + 小窗"展开"按钮。

### P1 体验完善

1. 小窗首页"启动器"形态（搜索 + 收藏/最近 chip + 记住上次工具）。
2. 路由守卫：`/settings` `/history` `/vault` 强制退出 mini。
3. `wide/full` 工具自动展开 + 可撤销提示。
4. `stay_on_top` 统一求值（`mode == mini || stay_on_top`）。
5. 与 Orb 的避让（进入 mini 时收起 Orb 面板）。

### P2 平台打磨与入口补全

1. 托盘菜单"以小窗打开"、Orb 右键菜单项。
2. 可选快捷键 `toggle_main_mini`（含 `SHORTCUT_DEFINITIONS` 登记）。
3. `skip_taskbar` 实测与取舍。
4. `miniFriendly` 元数据（工具注册表扩展）。
5. 多屏 / 高 DPI / 跨显示器拖拽回归。

---

## 12 手工回归清单

- [ ] normal → mini：窗口尺寸变化无内容溢出、无白屏、无闪烁
- [ ] mini → normal：恢复的是 normal 几何，不是 mini 位置
- [ ] 小窗内输入内容，切换回 normal 后内容仍在
- [ ] 小窗关闭（`X`）：`close_to_tray=true` 时隐藏，`false` 时退出
- [ ] `stay_on_top=false` 时进入 mini 仍置顶；退出 mini 后恢复不置顶
- [ ] 小窗中点击 `/settings`：自动展开且正确落到目标页
- [ ] 从小窗打开 `wide` 工具（SQL 格式化）：按策略自动展开
- [ ] 拖动小窗到副屏后退出 mini，再进入 mini：位置正确还原
- [ ] 跨 DPI 显示器拖动小窗：位置不偏移
- [ ] Orb 展开态时进入 mini：Orb 自动收起，无遮挡
- [ ] `toggle_main` 快捷键在 mini 态：只隐藏/显示，不改变形态
- [ ] 托盘"以小窗打开"：窗口以 mini 呈现且聚焦
- [ ] `CommandPalette` 在小窗下不溢出、不遮挡工具内容

---

## 13 结论

1. **mini 不是默认形态**，是工具的一种按需悬浮呈现；启动永远从 `normal` 开始。
2. **不新增窗口是硬前提**，因此 mini 与 normal 互斥，同一 webview 切换参数与外壳。
3. 改造重心不在窗口创建，而在四处：
   - 两套几何持久化（含 DPI 与多屏收敛）
   - 小窗外壳与工具页密度
   - `stay_on_top` / `close_to_tray` / 路由适用性的语义统一
   - 与 Orb 的层级避让
4. 最大技术限制是 `transparent` 不可运行期切换——**建议小窗保持不透明**，用无边框 + 系统圆角实现悬浮感。
5. 省内存的主要来源是"不新增 webview"，其次是"mini 态不挂载侧栏与未用工具组件"；后者收益需实测确认，不宜假设。
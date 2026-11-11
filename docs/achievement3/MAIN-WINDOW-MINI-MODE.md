# 主窗口小窗（Mini）模式分析

> 日期：2026-11-01｜关联：[CLOCK-NOTES-REFERENCE-ANALYSIS.md](../analysis/CLOCK-NOTES-REFERENCE-ANALYSIS.md) · `docs/ARCHITECTURE.md`
> 前提约束：**不新增窗口**，以最少内存与资源开销提供工具悬浮小窗形态

---

## 1 mini 的定义：三个误读

### 1.1 误读一：mini 是启动形态

把「下次启动是否以小窗打开」当成待确认项，隐含「mini 可能是默认形态」的假设。这个假设一旦成立，后面所有设计都会围绕「启动即小窗」展开。

**默认形态必须是完整窗口**，理由不只是产品偏好，还有三条可验证的技术成本（见 3.2）。

### 1.2 误读二：mini 是第二个窗口

「小窗」这个词暗示「再开一个小窗口」。但在 Tauri 2 下，新增一个 label 不是「再画一个框」，而是整套前端运行时再来一遍（见 2.1）。更关键的是：**新窗口等于放弃「工具输入状态保留」这个最大体验收益**——用户切到小窗时，`/tools/:id` 页面里已输入的内容、算出的结果、滚动位置会全部重来。

### 1.3 误读三：mini 是一种「记住的状态」

既然是「形态」，似乎就该像主题、语言一样被记住。但形态是系统对用户当前意图的推断，偏好是用户显式表达的选择。混为一谈会得出「记住上次是不是小窗」这种看似合理、风险很高的结论（见 3.2、3.5）。

### 1.4 正确定义

| 项 | 定义 |
|---|---|
| 默认形态 | **完整主窗口**（`normal`），带侧栏，工具箱全貌 |
| mini 是什么 | **工具悬浮小窗**：用户主动切换的临时呈现态，把当前工具变成常驻置顶的小窗口 |
| mini 何时出现 | **按需触发**（标题栏按钮 / 托盘 / 快捷键 / Orb 菜单），用完可退回完整窗口 |
| mini 与 normal 的关系 | **同一个 webview 的两种呈现，互斥**，切换只改窗口参数与 React 外壳 |
| mini 是否跨启动记忆 | **否**（形态不记） |
| mini 是否记忆位置与工具 | **是**（见第 3、4 章） |

---

## 2 判断一：不新增窗口

### 2.1 新增 label 的真实开销

| 开销项 | 性质 | 复用 main | 新增窗口 |
|---|---|:-:|:-:|
| WebView2 / WKWebView 实例 | 独立渲染进程或独立渲染上下文 | 0 | +1 |
| JS 运行时 + V8 堆 | 第二个 React 应用实例、第二份模块图 | 0 | +1 |
| React 组件树 | `MainLayout` 全量挂载 | 0 | +1 |
| Zustand store 实例 | 每窗口独立，需跨窗口同步才一致 | 0 | +N |
| IPC 与事件 | 需接入 `crossWindowEvent` / `emitSettingEventToPeers` 全套，否则状态漂移 | 0 | 必须补齐 |
| 生命周期 | 新增 `WindowSpec`、capability 授权、`CloseRequested` 分支、销毁逻辑 | 0 | 必须补齐 |

以上是结构性开销，不是精确字节数。

### 2.2 复用的隐性收益

1. **工具输入状态天然保留**：切换形态时 `/tools/:id` 内的输入、结果、滚动位置不丢。新窗口方案无法用工程手段弥补。
2. **无状态同步成本**：收藏、最近使用、搜索词、当前工具全在同一个 store，不会出现「两个窗口收藏不一致」。
3. **无新增授权面**：`src-tauri/capabilities/default.json` 现有授权已覆盖 `set-size` / `set-position` / `set-always-on-top` / `set-skip-taskbar` / `set-decorations` / `start-dragging`。
4. **路由复用**：`src/App.tsx` 按 `webview.label` 选布局，`main` 只有一套路由树；小窗只是换外壳。

### 2.3 复用的代价

| 代价 | 性质 |
|---|---|
| 不能同时存在「完整窗口 + 悬浮小窗」 | 省内存的必然取舍 |
| 切换瞬间窗口尺寸跳变，可能内容重排 | 可通过固定切换顺序缓解，无法完全消除 |
| 小窗状态会「污染」完整窗口（如小窗里选的工具，退回后仍停在该页） | 期望行为，保留上下文 |
| 单一窗口 = 单点 | 已有 `FeatureErrorBoundary` / `AppErrorBoundary` 兜底 |

不新增窗口是硬前提，也是后面所有取舍的源头——尤其是「normal 与 mini 互斥」和「几何必须存两套」。

---

## 3 判断二：三种「记忆」必须分开判断

「是否需要记忆窗口模式」把三件事混成了一件，拆开后结论方向相反。

### 3.1 拆分

| # | 记忆对象 | 判断 | 理由 |
|---|---|---|---|
| A | 上次退出时是 mini，下次启动仍是 mini | **不做** | 形态是系统推断，不是用户表达；猜错用户无法自愈 |
| B | normal 与 mini 各自的位置/尺寸 | **必须做** | 位置是用户亲手拖出来的意图；不记等于每次重置 |
| C | 上次在用的工具（`lastToolId`） | **必须做**（且已有数据） | 小窗的身份就是工具；无它则小窗退化为第二个工具箱入口 |

### 3.2 为什么 A（模式）不该记

**理由一：启动时序不友好。** `main` 窗口不是走 `src-tauri/src/window.rs` 的 `show_or_create` 建的，而是由 `src-tauri/tauri.conf.json` 在启动时声明式直接创建（`width: 960, height: 640, center: true`，默认可见）。要「启动即 mini」，只能在窗口创建后立刻 `set_size` + `set_position`，用户会先看到一帧完整窗口再缩成小窗；要消除这一帧，就得把 conf 改成 `visible: false` 并在 setup 内 `resize → position → show`，等于把启动流程从声明式改成命令式并引入白屏窗口期。

**理由二：首帧错位会抵消省内存的收益。** 3.3 的方案是 Rust 持一份 `Mode`、前端持一份 `windowModeStore`。若 mode 需要落盘，`MainLayout` 首帧时前端 store 还没读到值，会先按 normal 外壳渲染（挂上 `AppSidebar`）再切 mini——恰好把「mini 态不挂载侧栏」的收益吃掉。要避免就得在 store 初始化时同步 `invoke` 拿 mode 再渲染，多一层启动阻塞。

**理由三：与「托盘常驻 + 自启 + 强制置顶」三重叠加。** `close_to_tray` 默认 `true`（`src-tauri/crates/assistant-config/src/app_conf.rs`），另有 `launch_at_login`。用户上次在小窗里点 `X`（隐藏到托盘）→ 下次开机自启 → 直接弹出一个强制置顶的小窗，主观感受是「开机弹窗」。

若产品上非要支持「启动即 mini」，唯一可接受的形式是显式开关（如 `startup_mode`，默认 `normal`）：

| | 隐式记忆 | 显式开关 |
|---|---|---|
| 可预测性 | 用户无法预知下次形态 | 用户自己设定 |
| 出问题时的修复路径 | 只能改代码 | 改设置即可 |
| 与自启的冲突 | 必然冲突 | 可在自启拉起时忽略该设置 |
| 实现 | 仍需解决首帧与时序 | 同样需要，但意图明确 |

判据：**能被用户显式关闭的东西才可以被默认开启。**

### 3.3 为什么 B（几何）必须记

位置是意图，不是状态。记忆模式是在猜用户想怎样，记忆位置是在执行用户已经做出的选择。普通窗口的位置影响「顺不顺手」，小窗的位置影响「能不能用」：

- 小窗**恒置顶**，置顶窗口会主动遮挡。放在屏幕中心 = 正好压在用户正在看的焦点区，是 mini 的最坏默认值。
- mini 的设计意图是「边用边看」，必须与用户的主工作窗口建立空间关系（贴右侧、贴副屏、贴编辑器下方）。这个关系只能由用户拖出来。

频次决定成本结构：mini 是高频短时形态（一天十几次，每次几十秒），normal 是低频长时。一次拖动 2 秒，一天 10 次 = 20 秒/天，且每次发生在用户最想要快的时刻。感受不是「麻烦 2 秒」，而是「这个小窗不好用」。

不记忆位置还会**增加**资源占用：用户为了不丢位置会倾向于不关小窗、长期挂着，违背省内存前提。记忆位置本质上是让「用完就关」变得无痛。

两套几何是数学上的必然：

```
normal(x=100, y=100, 960×640)
        │ enter_mini: set_size + set_position
        ▼
mini(x=2000, y=50, 380×560)     ← 窗口此时只有一个真实位置
        │ exit_mini
        ▼
normal(???)                      ← 只存一套几何时，这里会拿到副屏位置 + 380 宽
```

所以 normal 也要记忆，它的首要目的不是启动还原，而是给 `exit_mini` 提供一个正确的落点。

仓库内已有先例：`src-tauri/crates/assistant-config/src/orb_conf.rs` 存 `position_x` / `position_y`；`src-tauri/src/setup.rs` 的 `position_orb_window` 启动时读回并做边界收敛；`spawn_screen_inspection` 定期校正并回写，防止窗口跑到屏幕外。mini 与 Orb 是完全同类的形态（置顶、悬浮、小尺寸、手动摆放）。

现状是 `main` 窗口**完全无记忆**——`WINDOW_SPECS` 里的 `center: true` 只在窗口被销毁后重建时才生效，正常启动走 conf 的 `center`。这解释了为什么引入 mini 时必须同时补上 normal 的几何：否则 `exit_mini` 无处可回。

### 3.4 为什么 C（工具身份）必须记

mini 的宾语是「当前工具」，不是「工具箱」；一旦丢失持续身份，它就只是第二个工具箱入口，功能与侧栏、CommandPalette、托盘菜单重复，只是更小更难用。展开在第 4 章。

### 3.5 统一标尺：可自愈与不可自愈

| 维度 | 记忆几何 / 工具 | 记忆模式 |
|---|---|---|
| 谁表达了意图 | 用户（亲手拖的、亲手点的） | 系统（推断上次状态） |
| 记错的后果 | 位置偏一点、多看一次启动器，**一次操作可修复** | 启动形态错了，**用户无法用一次操作修复** |
| 兜底机制 | 边界收敛 / `toolsMap` 查不到则回落启动器 | 无（只能改设置或改代码） |
| 实现成本 | 局部：一个配置项 + 一个收敛函数 | 全局：启动时序改造 + 首帧 + 与自启/置顶冲突 |

判据：位置与内容记错是「不顺手」，形态记错是「不可控」。只接受前者。

---

## 4 判断三：`lastToolId` 已经是既有数据

### 4.1 证据链

| 环节 | 位置 | 语义 |
|---|---|---|
| 表结构 | `src-tauri/crates/assistant-storage/src/db.rs` 的 `tool_history(tool_id PRIMARY KEY, used_at INTEGER)` | 每个工具一行，记录最后使用时间 |
| 写入 | `src-tauri/crates/assistant-storage/src/tool_history.rs` 的 `record_usage`（`INSERT OR REPLACE` + `timestamp_millis`） | 同一工具反复使用只更新时间 |
| 裁剪 | 同文件 `cleanup_old_history`（保留最近 10 条） | 表规模恒定 |
| 读取 | 同文件 `get_recent_tools`（`ORDER BY used_at DESC LIMIT ?1`） | 第 0 条即 lastToolId |
| IPC | `src-tauri/src/cmd/tool.rs` 的 `record_tool_usage` / `get_recent_tools` | 薄壳 |
| 前端写入 | `src/tools/ToolPage.tsx` 的 mount effect → `recordUsage(id)` | 进入工具页即记录 |
| 前端读取 | `src/hooks/useAppInitialization.ts` → `loadRecentTools()` → `src/stores/toolStore.ts` 的 `recentTools` | 启动加载 |

`recentTools[0]` 已经等价于「上次用的工具」，且跨重启有效。

### 4.2 不需要第二真值

引入 `lastOpenedToolId` 会产生两个问题：与 `recentTools[0]` 漂移（例如 `recordUsage` 走 localStorage 兜底时两边不一致）；还要再决定它存在哪（DB？conf？localStorage？）。第二真值只在两个值语义确实不同时才值得引入，而这里语义相同。

### 4.3 真正的分歧：全局 MRU 还是 mini 专属记忆

| 方案 | 语义 | 适用前提 | 风险 |
|---|---|---|---|
| A. 直接读 `recentTools[0]`（全局 MRU） | 用户在 normal 里用的工具，就是小窗要显示的工具 | 用户「按需进入 mini、用完退回」 | 用户把小窗当常驻挂件时，会被白天在 normal 的其它操作污染 |
| B. 新增「仅在小窗内使用」的记录 | 小窗可以稳定成为「那个工具」 | 用户把小窗当长期挂件 | 又造了一个双真值，且需区分「在哪种模式下进入工具」 |

**判断：先 A，不提前引入 B。** A 在「按需进入」模型下是正确的，B 只在「长期挂件」模型下需要，后者尚未被验证。若实测出现「小窗老是变成别的工具」的抱怨，再引入 B，且仅在 mini 内记录，并规定「为空时回落 `recentTools[0]`」。

系统只负责把「用户刚做过的事」递到下一个形态，不负责预测用户想进哪个形态——与 3.2 的「形态不记、内容记」一致。

### 4.4 三个边界

**边界一：记住的工具是 `wide` / `full`，会与「自动展开」形成死循环。**

`src/tools/registry.ts` 中 `wide` 有 3 个、`full` 有 2 个。若采用「进入 wide/full 工具时自动退出 mini」：

```
mini 唤起 → recentTools[0] = SQL 格式化(wide) → 自动展开退出 mini
          → 用户再点小窗 → 又是 SQL → 又展开 → ...
```

小窗每次唤起都弹回完整窗口，等于不可用。可选处理：

- mini 恢复工具时校验 `widthMode`，`wide`/`full` 则回落到启动器或最近一个 `default` 工具；
- 或区分「恢复」与「新选择」：小窗主动恢复的 wide/full 工具不自动展开，仅当用户在小窗内新选时才提示展开。

后者更合理，因为前者会让「小窗永远记不住你常用的那个宽工具」。但它是产品决策，不是技术结论。

**边界二：启动时 `recentTools` 是异步的。**

`loadRecentTools` 走 IPC 到 SQLite，是 async。若在首帧就读 `recentTools[0]`，拿到的是 `undefined`。这又是「不记忆模式」的一条论据：模式不记，mini 必然是用户稍后主动触发的，那时数据早已加载完毕；反之若启动即 mini，第一帧就会渲染空启动器再跳工具，出现可感知闪烁。

**边界三：只恢复「身份」，不恢复「内容」。**

`ToolPage` 的输入状态是组件内 React state。窗口 `hide()`/`show()`（关闭到托盘再唤起）时 webview 存活、路由不变、`key={id}` 不变，内容还在；应用重启则 JS 运行时重建，内容全丢，只剩工具身份。工具输入持久化是另一个需求，不应由 `lastToolId` 的引入制造预期。

**仓库级小问题**：同一个数字 10 出现在三处——`tool_history.rs` 的 `MAX_HISTORY_ENTRIES`、`src/config/constants.ts` 的 `RECENT_TOOLS_LIMIT`、`get_recent_tools` 的 `limit.unwrap_or(10)`。引入「mini 恢复 lastTool」后，这个数字的语义会变成「小窗能回退到多旧的工具」的边界，不宜再出现第四个 10。

---

## 5 现状盘点

### 5.1 已有能力

| 能力 | 位置 | 对小窗的意义 |
|---|---|---|
| 主窗口已无边框 | `src-tauri/tauri.conf.json` 的 `decorations: false` | 小窗外观可直接复用自绘标题栏 |
| 主窗口最小尺寸 300×200 | `src-tauri/crates/assistant-config/src/constant.rs` | 小窗尺寸落在合法区间，无需改 min_size |
| 窗口动态创建 | `src-tauri/src/window.rs` 的 `WINDOW_SPECS` + `show_or_create` | 已有「按 label 建窗」的统一入口 |
| 主窗口置顶链路 | `src-tauri/src/cmd/config.rs` 的 `stay_on_top` side effect；`src/components/PinToggle.tsx` | 小窗默认置顶有现成开关，但有双真值冲突（9.1） |
| 关闭到托盘 | `src-tauri/src/window.rs` 的 `on_window_event` → `CloseRequested` | 小窗关闭语义可沿用 |
| 布局按 label 分派 | `src/App.tsx` 的 `viewMap` | 小窗外壳不需要新增路由树 |
| 自绘标题栏 | `src/components/Titlebar.tsx`；`src/assets/styles/global.css` 的 `[data-tauri-drag-region]` | 小窗标题栏只需精简 |
| 工具使用记录 | `tool_history` 表 + `src/stores/toolStore.ts` | lastToolId 已存在（第 4 章） |
| 悬浮位置持久化先例 | `src-tauri/crates/assistant-config/src/orb_conf.rs` + `src-tauri/src/setup.rs` | 几何持久化有可复用范式 |
| 主窗口 toggle | `src-tauri/src/window.rs` 的 `toggle`，被 `toggle_main` 快捷键调用 | 与形态切换是两种语义（9.4） |

### 5.2 明确缺失

| 缺失项 | 现状 | 为什么重要 |
|---|---|---|
| 运行时「呈现模式」概念 | 无，形态完全由创建时的 `WindowSpec` 决定 | 需要 `normal / mini` 运行时状态 |
| 主窗口几何持久化 | 无，`save_orb_position_sync` 只存 Orb | 见 3.3 |
| 小窗布局外壳 | 无，`MainLayout` 固定 Titlebar + Sidebar + Content | 需要外壳分叉 |
| 工具页密度适配 | `ToolPage` 硬编码 `padding: 24px 32px`、标题 `fontSize: 28` | 小窗里空间浪费严重 |
| 小窗首页形态 | `ToolBox` 是分类 + 收藏 + 最近 + 新品四段式 | 小窗装不下 |
| 宽工具降级策略 | `widthMode` 仅影响 `maxWidth` | 见 4.4 边界一 |
| 窗口事件里没有 `Resized` / `Moved` | `on_window_event` 只处理 `CloseRequested` 与 `Focused(false)` | 见 9.7 |

---

## 6 模式切换的状态归属

模式切换需要 Rust 改窗口参数、前端换外壳，因此存在双端真值问题：

| 候选 | 评价 |
|---|---|
| 写进 `settingsStore`（`AppConf`） | 模式是窗口呈现态，不是用户偏好；落盘带来无谓 IO 与跨窗口广播 |
| 独立 `windowModeStore`（Zustand，不落盘） | 只存 `mode` 与切换 action |
| 挂在 `toolStore` | 职责不符 |
| 仅 Rust 持有 | 前端外壳无法响应 |

结构：**前端 store 负责 UI 外壳，Rust 侧持一份 `AtomicBool` / `Mutex<Mode>` 负责窗口参数与置顶语义**，两者通过命令同步。

这与 Orb 的 `click_through` 双端缓存（Rust `AtomicBool` + `orbStore.clickThrough`）是同一套模式，属于既有范式的复用。

---

## 7 窗口参数的约束分类

### 7.1 可运行时切换

`set_size` / `set_position` / `set_always_on_top` / `set_skip_taskbar` / `set_decorations` 均有运行时 setter，`capabilities/default.json` 已授权这五项。

### 7.2 仅创建期

`transparent` 与 `shadow` 是创建期参数，运行时无 setter。这条约束否掉了「完整窗口不透明、小窗半透明/毛玻璃」这种形态差异（详见 10.1）。

### 7.3 平台差异

`set_skip_taskbar` 在 Windows 上对已显示窗口可能不立即刷新任务栏按钮，需要 `hide()` + `show()` 才生效（详见 10.2）。

### 7.4 capability 面：走 Rust 命令

`capabilities/default.json` 当前没有 `set-resizable` / `set-maximizable` / `set-min-size`。前端直接 `invoke` 这些 window API 需要扩授权面；全部窗口操作走 Rust 命令薄壳则完全不动授权面。选后者，与项目「cmd 层薄壳 + Rust 执行业务」的既有分层一致，且授权面越小越安全。

### 7.5 尺寸取值

小窗宽度锁定而非自由拉伸，理由来自代码事实：

- `ToolPage` 的 `padding: 24px 32px` 与 `maxWidth` 按完整窗口设计，宽度自由拉伸会让二者在不同宽度下产生不一致的可用宽度；
- `ToolBox` 的 `Col xs/sm/md/lg` 断点会在 380~420 区间内产生列数跳变，若宽度可拖，用户会拖到「半列」状态；
- 高度相反：`ToolPage` 是纵向滚动容器，高度可拉不影响布局正确性，且「想看长一点」是真实需求。

结论：**宽度锁、高度放**。具体数值留给实施期用真实工具页测量（正则测试器、文本 Diff 最宽，是下限约束）。

---

## 8 前端改造的难点

### 8.1 外壳分叉，不是路由分叉

`src/layouts/MainLayout.tsx` 当前是固定结构 `Layout > Titlebar > (Layout > AppSidebar + Content > Routes)`。

必须让 `Routes` 子树保持同一份，只分叉外壳。路由分叉意味着两套 `Routes`，而 `toolStore`、当前工具、输入状态都挂在路由树上——分叉即丢上下文，等于把「复用窗口」的收益又还回去。

### 8.2 「不挂载」而不是「宽度归零」

`AppSidebar` 必须整个组件不挂载，而不是把宽度设 0。只有不挂载才能省掉 antd `Menu` 的构建成本；宽度 0 只是视觉隐藏，组件树照样构建。这是「mini 态省内存」的唯一真实来源（需实测，见 12 章）。

### 8.3 密度适配的判据

| 判据 | 评价 |
|---|---|
| `mode` 判断（store 读取） | 最直接，但工具内部组件读不到 mode 时需 prop 透传 |
| 容器宽度（`ResizeObserver` / `matchMedia`） | 更通用，未来「用户把窗口拉窄」也能自适应 |
| antd `Col` 的 `xs/sm/md/lg` | 工具内部响应式的首选，窄宽度自动单列，避免二次改造 |

判断：**外壳层用 mode，工具层用断点**。外壳只需知道「是不是 mini」，工具内部只需对窄做出反应，不需要知道「为什么窄」。这个分工能避免 mode 概念渗透进每个工具组件。

### 8.4 `widthMode` 的三种策略

`src/tools/types.ts` 的 `widthMode: "default" | "wide" | "full"`：

| 策略 | 行为 | 代价 |
|---|---|---|
| A. 提示 + 一键展开 | 小窗中提示「建议在完整窗口使用」 | 用户可能忽略提示 |
| B. 自动展开 | 进入 wide/full 时自动退出 mini | 推翻用户「刚切到小窗」的意图 |
| C. 禁止 | 小窗中灰掉 wide/full 工具 | 高频工具（SQL/Diff）无法小窗使用 |

**判断：B + 可撤销**——自动展开时给一次性提示，允许「仍在小窗打开」。必须与 4.4 边界一配套：**恢复时不展开，新选时才展开**，否则形成死循环。

### 8.5 `CommandPalette`

`src/components/commandPalette/CommandPalette.tsx` 是覆盖层，宽度与条目数按完整窗口设计，小窗下空间不足会溢出。

**判断：保留但收缩**（限制展示条数，复用已有的 `COMMAND_PALETTE_MAX_PER_CATEGORY`），不直接禁用——命令面板是键盘用户的高频入口。

### 8.6 小窗首页的两种候选

| 候选 | 问题 |
|---|---|
| 直接复用 `ToolBox` | 分类 + 收藏 + 最近 + 新品四段式，窄窗口下非常拥挤 |
| 「启动器」形态（搜索 + 收藏/最近 chip + 若已有当前工具则直接显示） | 需要新组件，但可复用 `toolStore` 的 `favoriteToolIds` / `recentTools` / `search` 与 `ToolSearchBar`（已是受控组件） |

判断：后者。其必要性依赖「小窗是否记住 lastTool」——若 4.3 采用方案 A 且记录有效，首页多数情况会被工具页直接覆盖，启动器只作为回落。

---

## 9 冲突面

### 9.1 `stay_on_top` 双真值

`src-tauri/src/cmd/config.rs` 的 side effect 在配置变更时直接对 `main` 执行 `set_always_on_top(conf.stay_on_top)`。小窗若强制置顶，会出现：用户关掉 `stay_on_top` → side effect 取消置顶 → 但小窗本该置顶。

需要的不是加个 if，而是单一求值规则：

```
effective_on_top = (mode == mini) || stay_on_top
```

所有 `set_always_on_top` 调用统一走该求值，退出 mini 时重新求值一次。小窗内不应出现 `PinToggle`——放了图钉会让用户以为可以取消置顶却无效。

### 9.2 `close_to_tray` 语义

`on_window_event` 的 `CloseRequested`：`main` 关闭时若 `close_to_tray` 为 true 则 `prevent_close` + `hide()`。小窗沿用同一逻辑，但 UI 上必须区分两个按钮：

| 按钮 | 语义 | 走哪条链路 |
|---|---|---|
| 关闭（`X`） | 隐藏到托盘（若 `close_to_tray`） | `window.close()` → 现有 `CloseRequested` |
| 展开 | 退出 mini 回 normal | 自定义命令 `set_main_window_mode("normal")` |

不要把「关闭」做成「退回完整窗口」——用户点 `X` 的预期是关掉。

### 9.3 页面适用性

| 路由 | 小窗可用性 | 原因 |
|---|---|---|
| `/`（启动器） | 可用 | 小窗首页 |
| `/tools/:id`（`default`） | 可用 | 核心场景 |
| `/tools/:id`（`wide/full`） | 有条件 | 见 8.4 |
| `/history` | 不可用 | 列表 + 详情需要宽度 |
| `/vault` | 不可用 | 条目列表需要宽度 |
| `/settings/*` | 不可用 | 设置页是多栏表单 |

处理方式是**模式跟随场景**：用户点「设置/历史/保险库」时自动展开，从小窗进入这些页面后用户也不会期待它继续缩在角落。托盘 `settings_general` 等菜单已走 `show_or_create` + 广播导航，天然会触发这条规则。

### 9.4 与 `toggle_main` 快捷键

`src-tauri/src/setup.rs` 的 `toggle_main` 调用 `crate::window::toggle`，语义是「可见且聚焦则隐藏，否则显示并聚焦」，与模式无关。

不要让 `toggle_main` 变成「切换 mini/normal」——否则用户按两次快捷键得到的是「隐藏/显示」还是「形态切换」会无法预期。若需要形态切换快捷键，必须新增独立 id，并在 `src/settings/core/types/shortcutTypes.ts` 的 `SHORTCUT_DEFINITIONS` 与 `DEFAULT_KEYBINDINGS` 同步登记，否则设置页不会出现该项。

### 9.5 Orb 的层级

Orb 窗口 `alwaysOnTop: true` 且 `skipTaskbar: true`。小窗置顶后两者争抢 z-order：系统置顶窗口之间按最近激活顺序叠放，可能出现 Orb 盖住小窗标题栏。

进入 mini 时若 Orb 处于展开态，先收起 Orb 面板；默认落点应避开 Orb 的 outer bounds（`ensure_orb_on_screen` 已有屏幕边界工具，可扩展为「避开 Orb 矩形」）。

### 9.6 拖拽与浮层裁剪

`[data-tauri-drag-region]` 已处理「拖拽区内的 input/button 不拖拽」，小窗标题栏可复用同一机制。两点需要注意：小窗标题栏高度应略降（小窗本身矮）；若标题栏放工具切换下拉，antd `Dropdown` 的浮层在极窄窗口下可能被裁剪，需设 `getPopupContainer` 或让 `placement` 自适应。

### 9.7 切换期的 `Resized` / `Moved` 会污染几何

`on_window_event` 目前没有监听 `Resized` / `Moved`。一旦加上「窗口变动即写几何」，`enter_mini` 内部的 `set_size` 会触发 `Resized`，若不加防护，这次事件会把小窗尺寸写进 normal 的键（或反之），症状是「退出小窗后完整窗口变窄了，且每次漂一点」。

两种防护：

- 切换期间置一个 `transitioning` 标志，事件回调直接丢弃；
- 或严格「按当前 mode 决定写哪个键」，且 mode 在 `set_size` 之前就切换好。

前者的顺序无关、更安全；后者依赖严格顺序，一旦有人重排代码就失效。

---

## 10 不可变约束

### 10.1 `transparent` 不可运行时切换

`transparent` 是创建期参数，运行时无 setter。因此不能做「完整窗口不透明、小窗半透明/毛玻璃」这种形态差异；若确实需要半透明小窗，只能创建时即 `transparent: true`，代价是完整窗口也失去系统阴影与圆角，且需处理 `background: transparent` 的绘制成本。

**判断：保持 `transparent: false`。** 无边框 + Windows 11 系统圆角已足够「悬浮感」，透明窗口会引入 WebView 合成层开销，与省资源目标冲突。

### 10.2 `skip_taskbar` 的运行期差异

Windows 上对已显示窗口 `set_skip_taskbar` 可能不立即刷新。稳妥做法是 `set_skip_taskbar(true)` 后 `hide()` + `show()` 一次（参考 Orb 切换 `set_ignore_cursor_events` 的「值相同则跳过系统调用」缓存策略）。若实测闪烁明显，应放弃 `skip_taskbar`：任务栏多一个图标不是致命问题，闪烁是。

### 10.3 多屏与 DPI

- 记忆位置可能落在已断开的副屏 → 恢复时必须做边界收敛；
- 跨 DPI 显示器拖动后 `scale_factor` 变化，几何需按逻辑像素存储才能正确还原；
- `outer_position()` 返回物理像素，`set_position` 接受 `LogicalPosition`，混用会导致偏移。

既有约定差异：Orb 存的是物理像素（`save_orb_position_sync` 直接把 `outer_position()` 写进 `OrbConf`），主窗口几何建议存逻辑像素（跨 DPI 更稳）。两者基准不同，实现时不能混用，也不应为了「统一」而把 Orb 改掉。

### 10.4 最小尺寸与内容溢出

小窗窄宽度下，部分工具内部组件（正则测试器的多列布局、文本 Diff 的双栏）会横向溢出。这不是窗口问题，而是工具组件问题：

- 短期：允许横向滚动（`overflow: auto` 已存在）；
- 中期：给 `ToolDefinition` 增加 `miniFriendly?: boolean`，让注册表显式标注「是否适合小窗」。这比靠 `widthMode` 推断更准——`widthMode` 表达的是「需要多宽」，不是「能否压缩」。

---

## 11 结论

1. **mini 不是默认形态**，是工具的一种按需悬浮呈现；启动永远从 `normal` 开始。
2. **不新增窗口是硬前提**，因此 mini 与 normal 互斥，同一 webview 切换参数与外壳。
3. **三种记忆分开判断**：形态不记（3.2）、几何必记（3.3）、工具身份必记且已有数据（第 4 章）。统一标尺是「记错后用户能否一次操作自愈」。
4. **`lastToolId` 不是新增能力，而是 `tool_history` 的新用法**（`recentTools[0]`）。独立字段会造第二真值。
5. 改造重心不在窗口创建，而在四处：两套几何持久化（含 DPI、多屏收敛、切换期事件污染）；小窗外壳与工具页密度；`stay_on_top` / `close_to_tray` / 路由适用性的语义统一；与 Orb 的层级避让。
6. 最大技术限制是 `transparent` 不可运行期切换，小窗保持不透明。
7. 省内存的来源是「不新增 webview」（结构性）与「mini 态不挂载侧栏」（可测），二者量级不同。

---

## 12 未验证的前提

| 结论 | 依赖的前提 | 验证方式 |
|---|---|---|
| 复用窗口比新开窗口省内存 | WebView2 进程/工作集确实随窗口数增长 | 对比「新增 mini 窗口」方案的进程数与工作集 |
| mini 外壳比 normal 外壳省内存 | 不挂载 `AppSidebar` 的收益可观测 | 记录两种外壳的 usedJSHeapSize |
| 切换无感知 | `set_size` + 外壳切换的顺序不产生明显重排 | 录屏逐帧检查 |
| `skip_taskbar` 可用 | Windows 运行期切换可接受 | 实测闪烁；不可接受则放弃 |
| 小窗宽度锁定不影响工具可用性 | 最宽的工具在锁定宽度下可操作 | 用正则测试器 / 文本 Diff 实测 |

「复用窗口」与「mini 外壳省资源」是两个量级不同的命题：前者是结构性节省（少一个 webview），后者是局部节省（少挂几个组件）。混为一谈会得出「既然 mini 也省不了多少，不如直接新开窗口」的错误推论——真正省 webview 的是「不新增窗口」这一前提本身，不是 mini 形态。
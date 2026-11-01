# I26 — mini 模式与时钟 / 便签工具规格

| 项 | 值 |
|---|---|
| 状态 | Draft |
| 日期 | 2026-11-01 |
| 上游分析 | `docs/analysis/MAIN-WINDOW-MINI-MODE.md`、`docs/analysis/CLOCK-NOTES-REFERENCE-ANALYSIS.md` |
| 关联规格 | `docs/store/I27-tool-data-store-spec.md`（便签数据落在工具数据存储，目录可由用户重定位） |
| 参考实现 | `references/6-productivity/pomatez`（Tauri 2 compact 模式）、`references/6-productivity/stretchly`（墙钟计时） |
| 影响范围 | 主窗口外壳、工具分类、工具注册、AppConf、SQLite、i18n、capabilities、Cargo 依赖 |

本文档是落地规格：定义字段、命令、常量、渲染分支与验收项。**为什么这样定** 由两份上游分析文档给出，本文不重复论证。

---

## 1. 概述

新增一个主窗口形态 `mini`，以及两个工具 `clock`（计时）与 `notes`（便签）。三者耦合点只有一个：时钟与便签必须在 mini 下可用，因此它们要声明 `miniFriendly`，而 mini 外壳要能容纳窄容器下的单个工具页。

交付分三块，可独立验收：

1. **mini 模式**——同一主窗口的第二种尺寸/密度，不新增窗口。
2. **clock 工具**——运行态放在 Rust，前端只渲染。
3. **notes 工具**——数据落 SQLite，前端只读写。

---

## 2. 目标

- 主窗口可在 `normal` / `mini` 两形态间切换，形态与各自位置跨重启保持。
- mini 下可完整使用 clock 与 notes，不出现横向溢出。
- 时钟在窗口隐藏、系统休眠后仍按时到点。
- 便签内容在崩溃/断电后不丢（写盘成功才视为已保存）。
- 两个工具归入新分类 `productivity`，且分类由关键词打分自动得出，不手工指定。
- 不新增 Tauri 窗口，不新增 `lastToolId` 字段。

## 3. 非目标

- 主窗口与 mini 同时存在（同一窗口，互斥）。
- 任意工具都能进 mini：只有声明 `miniFriendly: true` 的工具允许在 mini 中被直接选中。
- 时钟状态持久化（关闭后恢复计时）。
- 便签的多标签、全文索引、富文本块编辑。
- 时钟的系统级常驻提醒（托盘倒计时图标）。
- 修改 `transparent` / `shadow` / `decorations`：三者只在创建期生效，本项目三窗口均在 `tauri.conf.json` 声明。

---

## 4. 既有事实

落地必须迁就的现状，均已核对代码：

| 事实 | 位置 |
|---|---|
| 主窗口 960×640、最小 300×200、`decorations: false`、居中 | `src-tauri/tauri.conf.json`、`src-tauri/crates/assistant-config/src/constant.rs` |
| 主窗口在 `tauri.conf.json` 静态声明，非运行时创建 | `src-tauri/tauri.conf.json` `app.windows` |
| `WINDOW_SPECS` 只含 `main` / `settings`，供 `show_or_create` 使用 | `src-tauri/src/window.rs` |
| `on_window_event` 只处理 `CloseRequested`（main）与 `Focused(false)`（orb） | `src-tauri/src/window.rs` |
| 配置按组 `app` / `orb` / `mascot` 走 `patch_conf`，浅合并后整份保存 | `src-tauri/src/cmd/config.rs`、`src-tauri/src/cmd/mod.rs` `conf_side_effects` |
| 新增配置项须同时改 TS 接口与 Rust struct（`#[serde(default)]`） | `src/settings/core/types/confTypes.ts`、`src-tauri/crates/assistant-config/src/app_conf.rs` |
| 工具分类由 `classifyKeywords(keywords)` 打分得出，`defineTool` 不接受显式 category | `src/tools/registry.ts`、`src/tools/categories.ts` |
| 打分为精确词匹配；平局取靠前分类；全 0 分落 `dev` | `src/tools/categories.ts` `classifyKeywords` |
| 分类集合为 `dev` / `converter` / `security` / `media` | `src/tools/categories.ts` `CATEGORY_KEYS` |
| `ToolPage` 内边距 `24px 32px`、标题字号 28 硬编码；`WIDTH_MAP` 为 640 / 960 / 0 | `src/tools/ToolPage.tsx`、`src/config/constants.ts` |
| 工具页打开即 `recordUsage(id)`，历史表上限 10 条 | `src/tools/ToolPage.tsx`、`src-tauri/crates/assistant-storage/src/tool_history.rs` |
| 主窗口关闭按钮按 `close_to_tray` 转为隐藏 | `src-tauri/src/window.rs` `on_window_event` |
| Orb 位置恢复走 `OrbConf.position_x/y` + `placement::ensure_orb_on_screen` | `src-tauri/src/setup.rs`、`src-tauri/crates/assistant-orb/src/placement.rs` |
| Orb 展开由 Rust 端重算尺寸与位置（`expand_orb_panel`） | `src-tauri/src/window.rs` |
| 窗口能力已含 `set-size` / `set-position` / `set-always-on-top` / `set-decorations` / `start-dragging` | `src-tauri/capabilities/default.json` |
| 未引入 `tauri-plugin-notification` | `src-tauri/Cargo.toml`、`src-tauri/src/plugins.rs` |
| `react-markdown` / `remark-gfm` 已在依赖中 | `package.json` |
| 侧栏宽 240，主窗口 960 时工具页可用宽约 656 | `src/config/constants.ts`、`src/components/AppSidebar.tsx` |

两条从现状直接推出的结论，写进实现约束：

- `widthMode: "wide"`（960）在 240 侧栏下永远被内容区宽度截断，实际效果等于「占满内容区」。mini 下 `WIDTH_MAP` 全部失效，必须整体跳过。
- `tool_history` 已有 `ORDER BY used_at DESC LIMIT ?` 的能力，`recentTools[0]` 即最后使用的工具，无需新增字段。

---

## 5. 不变式

违反任一条即为实现错误。来源列中 `mini §n` / `CLOCK §n` 指两份上游分析文档的章节号：

| # | 不变式 | 来源 |
|---|---|---|
| 1 | 不新增 `WebviewWindow`；mini 是主窗口的尺寸与密度变化 | mini §2 |
| 2 | 不新增 `lastToolId` 字段，mini 的「上次工具」取 `recentTools[0]` | mini §4 |
| 3 | `padding` / 标题字号 / 描述可见性只在 `ToolPage` 内按 `mode` 分支，工具组件不得感知 mode | mini §10 |
| 4 | 工具组件只对容器宽度做断点反应，不得读取窗口 mode 或改外壳常量 | mini §10 |
| 5 | 时钟运行态只在 Rust 内存，不写 SQLite、不写 AppConf | CLOCK §4 |
| 6 | 便签必须写盘成功才算保存，前端不得把未落盘内容当已保存 | CLOCK §4 |
| 7 | `widthMode` 在 mini 下不参与计算，`miniFriendly` 优先 | CLOCK §6 |
| 8 | `transparent` / `shadow` / `decorations` 运行时不改 | mini §7 |
| 9 | 分类只由 `keywords` 打分得出，不得给 `defineTool` 加 category 入参 | CLOCK §5 |
| 10 | 便签数据只写 `tool-data.db`，不写 `assistant.db` | I27 §5 |

---

## 6. 配置

### 6.1 `AppConf` 新增字段

Rust（`src-tauri/crates/assistant-config/src/app_conf.rs`）：

```rust
#[serde(default = "default_window_mode")]
pub main_window_mode: String,          // "normal" | "mini"
#[serde(default)]
pub main_window_x: Option<i32>,        // 物理像素
#[serde(default)]
pub main_window_y: Option<i32>,
#[serde(default)]
pub main_window_width: Option<u32>,    // 物理像素
#[serde(default)]
pub main_window_height: Option<u32>,
#[serde(default)]
pub mini_window_x: Option<i32>,        // 物理像素
#[serde(default)]
pub mini_window_y: Option<i32>,
```

`default_window_mode()` 返回 `"normal"`。`new()` 中同值初始化。`validate()` 增加：

```rust
if !["normal", "mini"].contains(&self.main_window_mode.as_str()) {
    return Err(format!("main_window_mode must be normal/mini, got {}", self.main_window_mode));
}
```

TS（`src/settings/core/types/confTypes.ts` 的 `AppConf`）：

```ts
mainWindowMode: "normal" | "mini";
mainWindowX: number | null;
mainWindowY: number | null;
mainWindowWidth: number | null;
mainWindowHeight: number | null;
miniWindowX: number | null;
miniWindowY: number | null;
```

前端 `settingService.patchConf` 已在调用前做 `patchToSnakeCase`，新增字段无需额外映射。

### 6.2 几何落盘时机

不监听 `Moved` / `Resized`。只在三个时机写入：

| 时机 | 行为 |
|---|---|
| `set_main_window_mode` 前 | 把**当前 mode** 的几何写入对应字段 |
| `close_to_tray` 隐藏前 | 同上 |
| `perform_exit` 前 | 同上 |

理由：几何只需在下次启动时正确，逐帧写盘无收益；`Resized` 在拖拽期间每秒触发数十次。

写入规则：
- `normal` 记 `x` / `y` / `width` / `height`（`outer_position()` + `inner_size()`，均为物理像素）。
- `mini` 只记 `x` / `y`（mini 尺寸固定且不可 resize）。

### 6.3 常量

`src-tauri/crates/assistant-config/src/constant.rs` 新增：

```rust
pub const MAIN_WINDOW_MINI_WIDTH: f64 = 280.0;
pub const MAIN_WINDOW_MINI_HEIGHT: f64 = 420.0;
pub const MAIN_WINDOW_MINI_MIN_WIDTH: f64 = 240.0;
pub const MAIN_WINDOW_MINI_MIN_HEIGHT: f64 = 320.0;
```

`src/config/constants.ts` 新增：

```ts
export const MINI_WINDOW_WIDTH = 280;
export const MINI_WINDOW_HEIGHT = 420;
export const MINI_DENSITY_BREAKPOINT = 320;
```

四个 Rust 常量是逻辑像素（配 `LogicalSize` / `set_min_size`）；两个位置字段是物理像素（配 `PhysicalPosition`），与 `OrbConf.position_x/y` 一致。跨两套坐标的换算在 Rust 侧完成，前端不参与。

数值标定依据：280 宽是单列工具页在 12px 内边距下的可用下限；420 高 = 标题栏 32 + 单工具页可滚动区。**具体数值待 §16 的实测项校准，常量集中定义以便单点调整。**

---

## 7. mini 模式

### 7.1 状态归属

| 状态 | 存放位置 | 理由 |
|---|---|---|
| `mode` | `AppConf.main_window_mode` + Rust 静态量 | 需跨重启，且 Rust 要先于前端决定窗口尺寸 |
| 几何 | `AppConf` 六个字段 | 同上 |
| 当前工具 | 不新增，路由 `/tools/:id` + `recentTools[0]` | 已有数据 |

Rust 侧新增进程内状态：

```rust
static MAIN_WINDOW_MODE: LazyLock<Mutex<String>> = ...;  // 与 AppConf 同步，避免每命令读盘
```

### 7.2 切换命令

`src-tauri/src/window.rs`：

```rust
#[command]
pub fn set_main_window_mode(app: AppHandle, mode: String) -> Result<(), String>

pub fn restore_main_window(app: &AppHandle) -> Result<(), String>   // setup 调用
pub fn persist_main_geometry(app: &AppHandle) -> Result<(), String> // 三处时机调用
```

`set_main_window_mode` 步骤：

1. 校验 `mode ∈ {"normal","mini"}`，非法返回错误。
2. 读当前 mode，相同则直接返回。
3. `persist_main_geometry(app)`（写当前 mode 的几何）。
4. 按 §7.3 顺序施加尺寸与位置。
5. 更新 `AppConf.main_window_mode` 并 `save()`。
6. 更新 `MAIN_WINDOW_MODE`。
7. `emit` 事件 `ui:window-mode-changed`，载荷 `{ "mode": "normal" | "mini" }`。

### 7.3 施加顺序

顺序不可交换，否则被 `min_size` 钳制：

| 方向 | 顺序 |
|---|---|
| normal → mini | `set_resizable(false)` → `set_min_size(240, 320)` → `set_size(LogicalSize 280, 420)` → `set_position` |
| mini → normal | `set_size(LogicalSize 保存值或 960, 640)` → `set_min_size(300, 200)` → `set_resizable(true)` → `set_position` |

位置取值：
- 进入 mini：`mini_window_x/y` 有值则用之，否则沿用当前 `outer_position()` 并按 `placement::ensure_orb_on_screen` 同款逻辑收进可用屏幕。
- 退出 mini：`main_window_x/y` 有值则用之，否则沿用当前位置；`main_window_width/height` 无值时用 960×640 并居中。

`set_resizable(false)` 在 mini 中生效，使 mini 尺寸不可被用户拖动破坏；退出时恢复 `true`。

### 7.4 启动恢复

`src-tauri/src/setup.rs` 的 `init` 在 `position_orb_window(handle)` 之后调用 `window::restore_main_window(handle)`：

1. 读 `AppConf`。
2. `mode == "mini"`：`set_resizable(false)` → `set_min_size(240,320)` → `set_size(280,420)` → 恢复 mini 位置。
3. `mode == "normal"`：`set_size(保存值或 960×640)` → 恢复位置（无值时保持 `tauri.conf.json` 的居中）。
4. 不改 `decorations`。

前端启动后由 `MainLayout` 调 `loadConf("app")` 得到 `mainWindowMode`，与 Rust 侧一致；不一致时以 Rust 事件为准（`ui:window-mode-changed` 会在切换时广播）。

### 7.5 前端模式 store

新增 `src/stores/windowModeStore.ts`：

```ts
export type WindowMode = "normal" | "mini";

interface WindowModeState {
  mode: WindowMode;
  hydrated: boolean;
  hydrate: (mode: WindowMode) => void;
  setMode: (mode: WindowMode) => Promise<void>;
  toggleMode: () => Promise<void>;
}
```

`setMode` 调 `windowService.setMainWindowMode(mode)` 后本地置位；同时 `listen(UiEvent.WindowModeChanged)` 同步（防多入口分叉）。

新增 `src/services/windowService.ts`（沿用 `settingService` 的写法，从 `./tauriInvoke` 导入 `invoke`）：

```ts
export const windowService = {
  setMainWindowMode: (mode: "normal" | "mini"): Promise<void> =>
    invoke("set_main_window_mode", { mode }),
};
```

工具内的命令代理不放进 `src/services/`，而是放在工具目录内（`clock.service.ts` / `notes.service.ts`），与既有 22 个工具同构。

`UiEvent` 增加 `WindowModeChanged = "ui:window-mode-changed"`（`src/types/events.ts`），Rust `event_name` 增加同名常量。

### 7.6 外壳渲染

`src/layouts/MainLayout.tsx` 读 `useWindowModeStore`：

| 区域 | normal | mini |
|---|---|---|
| `Titlebar` | 现有 38 高，含主题/置顶/设置入口 | 32 高，仅保留拖拽区、模式切换按钮、`WindowControls` |
| `AppSidebar` | 240 / 64 可折叠 | 不渲染 |
| `Content` | 现有 | 不渲染侧栏，内容区即全宽 |
| `CommandPalette` | 保留 | 保留（选中非 `miniFriendly` 工具时先切 normal） |

`Titlebar` 新增 props `mode` 与 `onToggleMode`，mini 下隐藏 `ThemeToggle` / `PinToggle` / `SettingsEntry`。`decorations: false` 意味着拖拽区必须保留，否则 mini 无法移动。

`Layout` 根节点加 `data-window-mode={mode}`，供 CSS 分支；不引入新的全局样式文件，mini 差异写在现有内联样式或 `tool-page.css`。

### 7.7 mini 下的工具列表

`ToolBox` 在 `mode === "mini"` 时渲染 `MiniToolBox`（新文件 `src/tools/MiniToolBox.tsx`）：

- 顶部一个紧凑搜索框（复用 `ToolSearchBar`，`size="small"`）。
- 单列列表，行高 40：图标 18 + 名称 14px。
- 顺序：`recentTools`（最多 10）→ 收藏 → 其余按 `registry` 顺序。
- 不做分类分组、不做拖拽排序、不渲染 `ToolCard`。

选中文具时的分支（`MiniToolBox` 与 `CommandPalette` 共用同一函数，放在 `src/tools/miniNavigate.ts`）：

```
if (tool.miniFriendly === true) navigate(`/tools/${tool.id}`)
else { await setMode("normal"); navigate(`/tools/${tool.id}`) }
```

### 7.8 启动落点

mini 模式启动时：

| 条件 | 落点 |
|---|---|
| `recentTools[0]` 存在且该工具 `miniFriendly === true` | `/tools/{recentTools[0]}` |
| `recentTools[0]` 不存在，或该工具不是 `miniFriendly` | `/`（`MiniToolBox`） |

这条同时满足「恢复不展开」与「新选才展开」：恢复时不因工具不适配而擅自放大窗口，只在用户显式选择时才展开。

---

## 8. 分类扩展：`productivity`

### 8.1 为什么必须新增分类

时钟的关键词无论怎么配都会与既有分类撞词（`timer`、`schedule` 在 `dev` 签名内），撞词即平局，平局按 `CATEGORY_KEYS` 顺序归 `dev`；便签若关键词全不命中则全 0 分，同样落 `dev`。两种结果都让工具出现在「开发工具」下，因此必须新增分类并保证签名词独占。

### 8.2 定义

`src/tools/categories.ts`：

```ts
export const CATEGORY_KEYS = ["dev", "converter", "security", "media", "productivity"] as const;

productivity: [
  "pomodoro", "focus", "countdown", "stopwatch", "session", "break",
  "note", "notes", "memo", "scratchpad", "sticky", "todo", "checklist",
  "journal", "diary", "productivity", "reminder", "deadline", "task", "routine",
],
```

`TOOL_CATEGORIES` 追加：

```ts
{ key: "productivity", label: "tools.category.productivity", icon: CheckSquareOutlined, order: 4 }
```

**必须追加在数组末尾**：`classifyKeywords` 以 `score > bestScore` 比较，平局保留先出现的分类；插到中间会改变既有 22 个工具的归类结果。

### 8.3 关键词独占校验

两个新工具的 `keywords` 与四个旧签名、新签名的交集必须为空，否则产生平局：

| 工具 | keywords | 旧签名命中 | productivity 命中 | 结果 |
|---|---|---|---|---|
| `clock` | `pomodoro`, `focus`, `countdown`, `stopwatch`, `session`, `break` | 0 | 6 | `productivity` |
| `notes` | `note`, `notes`, `memo`, `scratchpad`, `sticky`, `todo`, `checklist`, `journal`, `diary` | 0 | 9 | `productivity` |

刻意排除的词：`timer`（在 `dev`）、`schedule`（在 `dev`）、`time` / `timestamp`（在 `converter`）、`edit`（在 `media`）、`count`（在 `dev`）。

新增签名词同样不得与旧签名重叠，`productivity` 列表已按此筛选；后续任何人往该列表加词，须先跑一次全量归类回归（§15 的 `registry.category` 快照测试）。

---

## 9. clock 工具

### 9.1 注册

`src/tools/registry.ts`：

```ts
const clock = defineTool({
  id: "clock",
  name: "tools.clock.name",
  description: "tools.clock.description",
  icon: FieldTimeOutlined,
  keywords: ["pomodoro", "focus", "countdown", "stopwatch", "session", "break"],
  component: lazyWithRecovery(() => import("./clock")),
  widthMode: "default",
  miniFriendly: true,
});
```

目录 `src/tools/clock/`：`index.tsx`、`clock.service.ts`、`clock.module.css`（与既有 22 个工具同构）。

### 9.2 运行态在 Rust

新增 `src-tauri/src/clock.rs`：

```rust
#[derive(Clone, Copy, PartialEq)]
pub enum ClockStatus { Idle, Running, Paused, Finished }

struct ClockInner {
    status: ClockStatus,
    duration_ms: i64,
    started_at_ms: Option<i64>,   // 当前运行段起点，墙钟
    elapsed_before_ms: i64,       // 已结束运行段的累计
}

pub struct ClockRuntime {
    inner: Mutex<ClockInner>,
    generation: AtomicU64,
}
```

剩余时间只有一个算法：

```
Running: duration_ms - elapsed_before_ms - (now_ms - started_at_ms)
Paused : duration_ms - elapsed_before_ms
Finished / Idle: 0 / duration_ms
```

`now_ms` 取 `chrono::Utc::now().timestamp_millis()`。**不使用递减计数**，因此任何一次读取都能自愈——这正是 `references/6-productivity/stretchly` 的 `timeLeft()` 做法。

ticker 线程：

```rust
fn spawn_ticker(app: AppHandle, rt: Arc<ClockRuntime>, generation: u64) {
    std::thread::spawn(move || loop {
        if rt.generation() != generation { return; }
        let remaining = rt.remaining_ms();
        if remaining <= 0 { rt.finish(&app); return; }
        std::thread::sleep(Duration::from_millis(remaining.min(1000) as u64));
    });
}
```

每次最多睡 1s 即重算墙钟。Tauri 2 不提供 Electron `powerMonitor` 等价物，因此不监听休眠/唤醒，而是靠「每次唤醒后立刻按真实时间重算」吸收休眠造成的偏差。`generation` 让 `pause` / `reset` / 再次 `start` 时旧线程自然退出。

`start` / `resume` 时 `generation += 1` 后以新值 spawn，杜绝双线程并存。

### 9.3 命令

`src-tauri/src/cmd/clock.rs`：

```rust
#[command] pub fn clock_start(app: AppHandle, duration_ms: i64) -> Result<ClockSnapshotDto, String>
#[command] pub fn clock_pause(app: AppHandle) -> Result<ClockSnapshotDto, String>
#[command] pub fn clock_resume(app: AppHandle) -> Result<ClockSnapshotDto, String>
#[command] pub fn clock_reset(app: AppHandle) -> Result<ClockSnapshotDto, String>
#[command] pub fn clock_snapshot(app: AppHandle) -> Result<ClockSnapshotDto, String>
```

`ClockSnapshotDto`：

```rust
pub struct ClockSnapshotDto {
    pub status: String,            // idle | running | paused | finished
    pub duration_ms: i64,
    pub remaining_ms: i64,         // 服务端算好的值，供首帧渲染
    pub started_at_ms: Option<i64>,// 前端据此本地续算
    pub elapsed_before_ms: i64,
}
```

约束：`60_000 <= duration_ms <= 10_800_000`（1–180 分钟），越界返回错误。`pause` 在非 Running 时为空操作并返回当前快照；`resume` 在非 Paused 时同样为空操作。

`ClockRuntime` 在 `setup::init` 中 `handle.manage(...)`。

### 9.4 前端

`src/tools/clock/clock.service.ts` 五个方法一一对应命令（从 `@/services/tauriInvoke` 导入 `invoke`）。

`src/tools/clock/index.tsx`：

- 挂载时 `clock_snapshot()` 取首帧，之后用**本地 1s 定时器**按 `started_at_ms + elapsed_before_ms + duration_ms` 续算，不做每秒 IPC。
- `document.visibilityState === "hidden"` 时停止本地定时器，`visibilitychange` 回可见时重新取快照。窗口隐藏期间显示本来就不需要刷新。
- 展示：剩余 `mm:ss`、进度环、四个按钮（开始/暂停/继续/重置）、预设 15/25/45/60 分钟 + 自定义 1–180。
- 到点后显示「已完成」并保留重置入口，状态由 `clock:finished` 事件驱动。

### 9.5 到点提醒

新增事件 `clock:finished`，载荷 `ClockSnapshotDto`。

Rust 侧在 `finish()` 中：

```
if main 窗口不可见 || 未聚焦 → 发系统通知
   若通知权限未授予 → show_or_create_main + set_focus 作为兜底
else → 仅 emit 事件，由界面自行提示
```

系统通知依赖 `tauri-plugin-notification`（§13）。兜底顺序的理由：通知是「不打断」的首选，但权限未授予时静默丢弃等于提醒失效，此时退化为显示窗口是唯一还能触达用户的手段。

通知正文使用 i18n（`assistant_i18n::t`），与既有 `tray.*` 一致。

---

## 10. notes 工具

### 10.1 注册

```ts
const notes = defineTool({
  id: "notes",
  name: "tools.notes.name",
  description: "tools.notes.description",
  icon: EditOutlined,
  keywords: ["note", "notes", "memo", "scratchpad", "sticky", "todo", "checklist", "journal", "diary"],
  component: lazyWithRecovery(() => import("./notes")),
  widthMode: "wide",
  miniFriendly: true,
});
```

`widthMode: "wide"` 表达「完整窗口下需要多宽」（列表 + 编辑器分栏）；`miniFriendly: true` 表达「窄容器下能压缩为单列」。两者语义不同，不互相推断。

### 10.2 存储

表建在**工具数据存储**中（`tool-data.db`），不在 `assistant.db`。原因与目录重定位规则见 `docs/store/I27-tool-data-store-spec.md` §6.2 与 §9：便签是用户产出，只有独立成文件才能单独搬。

```sql
CREATE TABLE IF NOT EXISTS note_entries (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_note_entries_active
    ON note_entries(deleted_at, updated_at);
```

`deleted_at IS NULL` 为活动便签。`id` 用 `uuid::Uuid::new_v4()`（已在依赖中）。建表语句加入 I27 §6.2 的工具数据库建库批次。

CRUD 模块 `src-tauri/crates/assistant-storage/src/note_entries.rs`，全部为接受 `&Connection` 的纯函数，连接由 `ToolDataStore` 提供：

```rust
pub struct NoteRow { id, title, content, created_at, updated_at, deleted_at }

pub fn list(conn: &Connection, query: Option<&str>, include_deleted: bool) -> Result<Vec<NoteRow>, String>
pub fn get(conn: &Connection, id: &str) -> Result<NoteRow, String>
pub fn create(conn: &Connection, title: &str, content: &str) -> Result<NoteRow, String>
pub fn update(conn: &Connection, id: &str, title: &str, content: &str) -> Result<NoteRow, String>
pub fn soft_delete(conn: &Connection, id: &str) -> Result<(), String>
pub fn restore(conn: &Connection, id: &str) -> Result<(), String>
pub fn purge(conn: &Connection, id: &str) -> Result<(), String>
```

`list` 的 `query` 走 `title LIKE '%q%' OR content LIKE '%q%'`，转义 `%` / `_`；排序 `updated_at DESC`。

### 10.3 命令

`src-tauri/src/cmd/note.rs`：

```rust
#[command] pub fn list_notes(app, query: Option<String>, include_deleted: Option<bool>) -> Result<Vec<NoteSummaryDto>, String>
#[command] pub fn get_note(app, id: String) -> Result<NoteDto, String>
#[command] pub fn create_note(app, title: String, content: String) -> Result<NoteDto, String>
#[command] pub fn update_note(app, id: String, title: String, content: String) -> Result<NoteDto, String>
#[command] pub fn delete_note(app, id: String) -> Result<(), String>
#[command] pub fn restore_note(app, id: String) -> Result<(), String>
#[command] pub fn purge_note(app, id: String) -> Result<(), String>
```

连接来源：`app.state::<ToolDataStore>()`，经 `with_conn` 访问（I27 §8.2）；不得用 `DbState`。存储为 `unavailable` 或正在切换时，命令按 I27 的错误键原样返回，前端显示对应提示。

DTO：

```rust
pub struct NoteSummaryDto { id, title, excerpt, updated_at, deleted_at }  // excerpt 取 content 前 120 字符
pub struct NoteDto { id, title, content, created_at, updated_at }
```

列表只回 `excerpt`，不回全文：便签条数增长后，把全部 `content` 过 IPC 是无谓开销。

### 10.4 前端

`src/tools/notes/notes.service.ts` 七个方法；界面与样式在 `src/tools/notes/` 的 `index.tsx` 与 `notes.module.css`。

编辑行为：
- 标题单行输入，正文 `textarea`（不做富文本块编辑）。
- 预览用 `react-markdown` + `remark-gfm`，与既有工具一致，不新增依赖。
- 自动保存：输入后 800ms 防抖调 `update_note`；失焦、切换便签、卸载时立即 flush。
- **写盘成功才算保存**：`update_note` 失败时保留本地内容、显示错误并保持 dirty 状态，下次输入继续重试；不得先清 dirty 再落盘。
- 新建：`create_note("", "")` 后立即进入编辑，标题为空时列表显示「未命名」。
- 删除走 `delete_note`（软删），回收站视图用 `list_notes(include_deleted: true)` 过滤 `deleted_at != null`，提供恢复与彻底删除。

布局：
- normal：左列表 240 + 右编辑区，列表项显示标题 + 摘要两行。
- mini：只渲染列表或编辑器之一，通过组件内局部 state 切换（列表 → 点开 → 编辑器 → 返回）。**该分支按容器宽度判断，不读窗口 mode**（不变式 4）。

### 10.5 容器宽度断点

`ToolPage` 在内容区外层加 `container-type: inline-size`，工具组件用 `@container (max-width: 320px)` 写窄态样式。这样 notes 的「列表/编辑器单栏」由容器宽度决定，在 normal 下用户把窗口拖窄也会自动切单栏——与 mini 无关，符合不变式 4。

---

## 11. `miniFriendly` 与 `widthMode`

`src/tools/types.ts` 的 `ToolDefinition` 新增：

```ts
miniFriendly?: boolean;
```

语义：该工具在窄容器（`MINI_DENSITY_BREAKPOINT` 及以下）下仍可用，允许在 mini 中被直接选中。

优先级规则（唯一一处判断，位于 `miniNavigate.ts`）：

| `miniFriendly` | `widthMode` | mini 中直接选中 | normal 中 |
|---|---|---|---|
| `true` | 任意 | 允许 | 按 `widthMode` |
| 缺省 / `false` | 任意 | 不允许，先切 normal | 按 `widthMode` |

`ToolPage` 在 mini 下**整体跳过** `WIDTH_MAP`，`maxWidth` 置为 `100%`。normal 下逻辑不变。

`ToolPage` 的 mini 分支：

| 属性 | normal | mini |
|---|---|---|
| 外层 `padding` | `24px 32px` | `8px 12px` |
| 标题字号 | 28 | 18 |
| 描述 | 显示 | 不显示 |
| 收藏按钮 | 显示 | 不显示 |
| `maxWidth` | `WIDTH_MAP[widthMode]` | `100%` |
| `key={id}` 重挂载 | 保留 | 保留 |

mini 下仍保留返回按钮（回到 `MiniToolBox`），因为 mini 里换工具是主要操作。

---

## 12. i18n

`src/i18n/locales/zh-CN.json` 与 `en-US.json` 同步新增（`i18next.d.ts` 的类型从 `zh-CN` 推导，两个文件键集必须一致）：

| 键 | 说明 |
|---|---|
| `tools.category.productivity` | 分类名 |
| `tools.clock.name` / `tools.clock.description` | 工具注册 |
| `tools.clock.start` / `pause` / `resume` / `reset` / `finished` | 界面 |
| `tools.clock.minutes` / `custom_minutes` / `invalid_duration` | 时长 |
| `tools.notes.name` / `tools.notes.description` | 工具注册 |
| `tools.notes.untitled` / `new_note` / `empty` / `search_placeholder` | 列表 |
| `tools.notes.placeholder_title` / `placeholder_body` | 编辑器 |
| `tools.notes.trash` / `restore` / `purge` / `delete_confirm` | 回收站 |
| `tools.notes.save_failed` | 写盘失败提示 |
| `mini.toggle_to_normal` / `mini.toggle_to_mini` | 模式切换按钮 |
| `clock.notification_title` / `clock.notification_body` | Rust 侧通知（`assistant_i18n::t`） |

Rust 侧 i18n 资源与前端 JSON 是两套文件，`clock.notification_*` 需同时加入 Rust 的资源文件。

---

## 13. 依赖与能力

| 变更 | 位置 |
|---|---|
| `tauri-plugin-notification = "2"` 加入 workspace 与根包 `[dependencies]` | `src-tauri/Cargo.toml` |
| `.plugin(tauri_plugin_notification::init())` | `src-tauri/src/plugins.rs` |
| `"notification:default"` 加入 permissions | `src-tauri/capabilities/default.json` |

不引入 `@tauri-apps/plugin-notification`：通知只从 Rust 侧发起，前端不调 JS API。

`react-markdown` / `remark-gfm` 已在 `package.json`，不加依赖。

capability 的具体标识符以构建期生成的权限 schema（`src-tauri/gen/schemas/`，未纳入版本库）为准，落地时核对；`notification:default` 不覆盖所需子权限时按 schema 补 `notification:allow-*`。

`set_resizable` / `set_min_size` 不在 `default.json` 的窗口权限列表中，但 §7 的全部窗口操作都在 Rust 侧直接调用 `WebviewWindow` 方法，capabilities 只约束前端 invoke，因此**不需要为它们新增前端权限**。

---

## 14. 文件清单

新增：

| 文件 | 内容 |
|---|---|
| `src/stores/windowModeStore.ts` | 模式 store |
| `src/services/windowService.ts` | `set_main_window_mode` 代理 |
| `src/tools/MiniToolBox.tsx` | mini 下的紧凑列表 |
| `src/tools/miniNavigate.ts` | 选中工具的统一分支 |
| `src/tools/clock/{index.tsx,clock.service.ts,clock.module.css}` | 时钟工具与命令代理 |
| `src/tools/notes/{index.tsx,notes.service.ts,notes.module.css}` | 便签工具与命令代理 |
| `src-tauri/src/clock.rs` | 时钟运行态与 ticker |
| `src-tauri/src/cmd/clock.rs` | 时钟命令 |
| `src-tauri/src/cmd/note.rs` | 便签命令 |
| `src-tauri/crates/assistant-storage/src/note_entries.rs` | 便签 CRUD（`&Connection` 纯函数，由 `ToolDataStore` 提供连接） |

修改：

| 文件 | 改动 |
|---|---|
| `src-tauri/tauri.conf.json` | 无（主窗口保持静态声明） |
| `src-tauri/crates/assistant-config/src/app_conf.rs` | 7 个字段 + `validate` |
| `src-tauri/crates/assistant-config/src/constant.rs` | 4 个 mini 常量 |
| `src/settings/core/types/confTypes.ts` | `AppConf` 7 个字段 |
| `src-tauri/src/window.rs` | `set_main_window_mode` / `restore_main_window` / `persist_main_geometry` |
| `src-tauri/src/setup.rs` | `ClockRuntime` manage、`restore_main_window` 调用 |
| `src-tauri/src/cmd/mod.rs` | 导出 `clock` / `note` 模块 |
| `src-tauri/src/cmd/app.rs` | `perform_exit` 前 `persist_main_geometry` |
| `src-tauri/src/ipc.rs` | 注册 13 个新命令（时钟 5 + 便签 7 + 模式 1） |
| `src-tauri/src/protocol/events.rs` | `CLOCK_FINISHED` / `WINDOW_MODE_CHANGED` |
| `src-tauri/src/plugins.rs` | notification 插件 |
| `src-tauri/capabilities/default.json` | notification 权限 |
| `src-tauri/crates/assistant-storage/src/db.rs` | 无（便签表建在 `tool-data.db`，见 I27 §6.2） |
| `src/tools/types.ts` | `miniFriendly` |
| `src/tools/categories.ts` | `productivity` 键 / 签名 / meta |
| `src/tools/registry.ts` | 两个 `defineTool` |
| `src/tools/ToolBox.tsx` | mini 分支 |
| `src/tools/ToolPage.tsx` | 密度分支 + 容器查询锚点 |
| `src/tools/tool-page.css` | 容器查询 |
| `src/layouts/MainLayout.tsx` | 按 mode 渲染外壳、`loadConf("app")` |
| `src/components/Titlebar.tsx` | `mode` / `onToggleMode` |
| `src/types/events.ts` | `UiEvent.WindowModeChanged` |
| `src/config/constants.ts` | mini 常量 |
| `src/components/commandPalette/CommandPalette.tsx` | 选中工具走 `miniNavigate` |
| `src/i18n/locales/{zh-CN,en-US}.json` | 键集 |
| `package.json` | 无（依赖已具备） |

---

## 15. 验收

构建与静态检查：

- [ ] `npm run typecheck` 通过
- [ ] `npm run lint` 通过
- [ ] `npm run test` 通过
- [ ] `cargo fmt` 后 `cargo test`（cwd = `src-tauri`）通过

单元测试（新增）：

- [ ] `classifyKeywords` 回归：22 个既有工具的归类结果与改动前逐项相同
- [ ] `clock` 与 `notes` 的 keywords 归类为 `productivity`
- [ ] `ClockRuntime`：`start → 前进 now → remaining` 单调递减；`pause` 后 remaining 冻结；`resume` 后继续；`finish` 幂等；`generation` 使旧 ticker 退出
- [ ] `note_entries`：`list` 的 `query` 转义、软删后不出现在活动列表、`restore` 可恢复

手工回归：

- [ ] normal → mini → normal 往返，窗口尺寸/位置正确，mini 内不可拖拽改尺寸
- [ ] 重启后回到上次的 mode 与位置（两形态分别验证）
- [ ] mini 下 clock 与 notes 无横向溢出、无被裁切按钮
- [ ] normal 下把窗口拖到 300 宽，notes 自动切单栏（验证容器查询而非 mode）
- [ ] mini 下从 `MiniToolBox` 选一个非 `miniFriendly` 工具（如 `json-viewer`）：窗口先展开再进入
- [ ] 时钟运行中隐藏主窗口，到点仍触发提醒
- [ ] 便签编辑中断网/写盘失败时内容仍在，且提示保存失败
- [ ] 分类页出现 `productivity` 分组，仅含 clock 与 notes

---

## 16. 未验证前提

以下在实现前无法凭代码确定，落地时需实测并回填本文：

1. **280px 下 Ant Design 组件的最小宽度**：`Input`、`Select`、`Button` 组合是否溢出。若溢出，调 `MINI_WINDOW_WIDTH` 而非改组件尺寸。
2. **Windows 上 `set_min_size` 小于当前尺寸时的即时行为**：部分版本会立刻把窗口放大，验证 §7.3 顺序是否需要额外一次 `set_size`。
3. **通知权限首次请求的时机**：macOS 在首次 `notify` 时弹授权，可能在 mini 下打断操作；若体验不佳，改为设置页显式授权。
4. **系统休眠期间 `thread::sleep` 的返回语义**：Windows 唤醒后 sleep 是否立即返回并重算，需实测；若不返回，ticker 会在唤醒后延迟至下一次 1s 边界，属可接受误差，但要确认不是「永久卡死」。
5. **便签条数上限**：`list` 未分页，条数达到数千时列表是否仍可接受，需实测后决定是否加 `LIMIT` 与分页。
6. **`LIKE` 对中文的匹配行为**：SQLite 无分词，中文按子串匹配，符合预期；但大小写不敏感仅对 ASCII 成立，需确认是否符合便签搜索的期望。
7. **mini 下 `CommandPalette` 的可用性**：全局面板在 280px 宽度下是否仍可操作，若不可用则在 mini 中禁用并只保留 `MiniToolBox` 搜索框。
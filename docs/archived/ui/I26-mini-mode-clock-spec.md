# I26 — mini 模式与时钟工具规格

| 项 | 值 |
|---|---|
| 状态 | Implemented（2026-11-03） |
| 日期 | 2026-11-03（首版 2026-11-01） |
| 上游分析 | `docs/archived/ui/MAIN-WINDOW-MINI-MODE.md`、`docs/analysis/CLOCK-NOTES-REFERENCE-ANALYSIS.md` |
| 关联规格 | `docs/store/I27-tool-data-store-spec.md`（便签延期，本阶段不消费） |
| 参考实现 | `references/6-productivity/pomatez`（紧凑模式 + 状态控制）、`references/6-productivity/stretchly`（墙钟计时） |
| 影响范围 | 主窗口外壳、工具分类、工具注册、AppConf、i18n、capabilities、Cargo 依赖 |

本文档是落地规格：定义字段、命令、常量、渲染分支与验收项。**为什么这样定** 由两份上游分析文档给出，本文不重复论证。

**本阶段范围**：mini 模式 + `clock` 工具。便签（notes）不在本阶段，相关设计保留在 §16 作为延期项；I27 工具数据存储暂不接入。

---

## 1. 概述

新增主窗口形态 `mini`（同一窗口的第二种尺寸/密度）与一个工具 `clock`（倒计时）。

耦合点只有一个：时钟必须在 mini 下可用，因此声明 `miniFriendly`，而 mini 外壳要能容纳窄容器下的单个工具页。

交付分两块，可独立验收：

1. **mini 模式**——不新增窗口；形态只在运行时存在，不跨重启记忆；normal / mini 各自的位置跨重启保持。
2. **clock 工具**——运行态在 Rust，前端只渲染；到点用系统通知提醒。

---

## 2. 目标

- 主窗口可在 `normal` / `mini` 两形态间切换，切换不重挂载工具组件（输入状态不丢）。
- normal 与 mini 各自的位置跨重启保持；normal 的尺寸也保持。
- **mini 形态本身不落盘**：每次启动都从 `normal` 开始，避免开机直接弹置顶小窗。
- **进入 mini 不替用户选择工具**：切换按钮只改窗口形态，不改变当前路由；当前页面不兼容时给出提示并保持 `normal`。
- mini 下可完整使用 clock，不出现横向溢出。
- 时钟在窗口隐藏、系统休眠后仍按时到点。
- mini 默认置顶，退出 mini 后恢复用户 `stay_on_top` 设置。
- 时钟归入新分类 `productivity`，分类由关键词打分自动得出。

## 3. 非目标

- 主窗口与 mini 同时存在（同一窗口，互斥）。
- 任意工具都能进 mini：只有声明 `miniFriendly: true` 的工具允许在 mini 中被直接选中。
- 时钟状态持久化（进程退出后不恢复计时）。
- 便签、多标签、全文索引、富文本块编辑（见 §16）。
- 时钟的系统级常驻提醒（托盘倒计时图标）。
- 修改 `transparent` / `shadow` / `decorations`：三者只在创建期生效，本项目三窗口均在 `tauri.conf.json` 声明。
- 新增全局快捷键切换形态（不复用 `toggle_main`，避免「隐藏/显示」与「形态切换」语义混淆）。

---

## 4. 既有事实

落地必须迁就的现状，均已核对代码：

| 事实 | 位置 |
|---|---|
| 主窗口 960×640、最小 300×200、`decorations: false`、居中 | `src-tauri/tauri.conf.json`、`src-tauri/crates/assistant-config/src/constant.rs` |
| 主窗口在 `tauri.conf.json` 静态声明，非运行时创建 | `src-tauri/tauri.conf.json` `app.windows` |
| `WINDOW_SPECS` 只含 `main` / `settings` | `src-tauri/src/window.rs` |
| `on_window_event` 只处理 `CloseRequested`（main）与 `Focused(false)`（orb），未监听 `Moved` / `Resized` | `src-tauri/src/window.rs` |
| 配置按组 `app` / `orb` / `mascot` 走 `patch_conf`，浅合并后整份保存 | `src-tauri/src/cmd/config.rs`、`src-tauri/src/cmd/mod.rs` `conf_side_effects` |
| 新增配置项须同时改 DTO、Rust struct 与前端契约 | `src-tauri/src/protocol/dto/config.rs`、`src-tauri/crates/assistant-config/src/app_conf.rs`、`src/types/protocol.contract.ts` |
| 工具分类由 `classifyKeywords(keywords)` 打分得出，`defineTool` 不接受显式 category | `src/tools/registry.ts`、`src/tools/categories.ts` |
| 打分为精确词匹配；平局取靠前分类；全 0 分落 `dev` | `src/tools/categories.ts` `classifyKeywords` |
| 分类集合为 `dev` / `converter` / `security` / `media` | `src/tools/categories.ts` `CATEGORY_KEYS` |
| `ToolPage` 内边距 `24px 32px`、标题字号 28 硬编码；`WIDTH_MAP` 为 640 / 960 / 0 | `src/tools/ToolPage.tsx`、`src/config/constants.ts` |
| 工具页打开即 `recordUsage(id)`，历史表上限 10 条 | `src/tools/ToolPage.tsx`、`src-tauri/crates/assistant-storage/src/tool_history.rs` |
| 主窗口关闭按钮按 `close_to_tray` 转为隐藏 | `src-tauri/src/window.rs` `on_window_event` |
| Orb 位置恢复走 `OrbConf.position_x/y` + `placement::ensure_orb_on_screen` | `src-tauri/src/setup.rs`、`src-tauri/crates/assistant-orb/src/placement.rs` |
| `placement::snap_to_nearest_screen` 可对任意窗口做屏幕收敛 | `src-tauri/crates/assistant-orb/src/geometry.rs` |
| `stay_on_top` 的写入副作用直接对 `main` 调 `set_always_on_top` | `src-tauri/src/cmd/config.rs` `APP_SIDE_EFFECTS` |
| 未引入 `tauri-plugin-notification` | `src-tauri/Cargo.toml`、`src-tauri/src/plugins.rs` |
| 侧栏宽 240，主窗口 960 时工具页可用宽约 656 | `src/config/constants.ts`、`src/components/AppSidebar.tsx` |

两条从现状直接推出的结论：

- `widthMode: "wide"`（960）在 240 侧栏下永远被内容区宽度截断，实际效果等于「占满内容区」。mini 下 `WIDTH_MAP` 全部失效，必须整体跳过。
- `tool_history` 已有 `ORDER BY used_at DESC LIMIT ?` 的能力，`recentTools[0]` 即最后使用的工具，无需新增 `lastToolId` 字段。

---

## 5. 不变式

违反任一条即为实现错误。来源列中 `mini §n` / `CLOCK §n` 指两份上游分析文档的章节号：

| # | 不变式 | 来源 |
|---|---|---|
| 1 | 不新增 `WebviewWindow`；mini 是主窗口的尺寸与密度变化 | mini §2 |
| 2 | **mini 形态不落盘**；启动永远 `normal`，只有两套几何与最近工具被记忆 | mini §3.2 |
| 3 | 不新增 `lastToolId`；`recentTools` 只用于 `MiniToolBox` 排序，不再自动决定进入 mini 的落点 | mini §4（落点部分已收敛） |
| 4 | `padding` / 标题字号 / 描述可见性只在 `ToolPage` 内按 `mode` 分支，工具组件不得感知 mode | mini §10 |
| 5 | 工具组件只对容器宽度做断点反应，不得读取窗口 mode 或改外壳常量 | mini §10 |
| 6 | 时钟运行态只在 Rust 内存，不写 SQLite、不写 AppConf | CLOCK §4、§7 |
| 7 | `widthMode` 在 mini 下不参与计算，`miniFriendly` 优先 | CLOCK §6 |
| 8 | `transparent` / `shadow` / `decorations` 运行时不改 | mini §7 |
| 9 | 分类只由 `keywords` 打分得出，不得给 `defineTool` 加 category 入参 | CLOCK §5 |
| 10 | 窗口置顶唯一求值规则：`effective_on_top = mode == mini \|\| stay_on_top` | mini §9.1 |
| 11 | 位置存物理像素，尺寸存**逻辑像素**；跨两套坐标的换算只在 Rust 侧完成 | mini §10.3 |
| 12 | 切换形态期间的窗口事件不得写几何；本项目不监听 `Moved` / `Resized`，几何只在三个时机显式落盘 | mini §9.7 |
| 13 | 从 mini 进入不兼容路由（设置 / 历史 / 特性页 / 非 `miniFriendly` 工具）时先切回 `normal` | mini §9.3 |
| 14 | 标题栏的 mini / 展开按钮只改形态，不导航；当前页面不兼容时提示并保持 `normal` | 本轮修订 |

---

## 6. 配置

### 6.1 `AppConf` 新增字段

Rust（`src-tauri/crates/assistant-config/src/app_conf.rs`）：

```rust
#[serde(default)]
pub main_window_x: Option<i32>,        // 物理像素
#[serde(default)]
pub main_window_y: Option<i32>,        // 物理像素
#[serde(default)]
pub main_window_width: Option<u32>,    // 逻辑像素
#[serde(default)]
pub main_window_height: Option<u32>,   // 逻辑像素
#[serde(default)]
pub mini_window_x: Option<i32>,        // 物理像素
#[serde(default)]
pub mini_window_y: Option<i32>,        // 物理像素
```

- 不新增 `main_window_mode`；形态是运行时状态。
- `new()` 中六个字段均为 `None`。
- `validate()` 不新增分支；几何数值在写入与恢复时做合理范围收敛（见 §6.3）。

DTO（`src-tauri/src/protocol/dto/config.rs` 的 `AppConfDto`）同步六个字段，`From<AppConf>` 逐一透出；生成后前端 `AppConf` 自动得到 `number | null`。

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

- `normal` 记 `x` / `y`（`outer_position()`，物理像素）与 `width` / `height`（`inner_size()` / `scale_factor` 后取整，逻辑像素）。
- `mini` 只记 `x` / `y`（mini 尺寸固定且不可 resize）。

几何写入复用 `apply_app_conf` 的同一把配置锁，避免与用户改主题等 `patch_conf` 操作发生「整份保存互相覆盖」。

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
export const MINI_TITLEBAR_HEIGHT = 32;
export const MINI_DENSITY_BREAKPOINT = 320;
```

四个 Rust 常量是逻辑像素（配 `LogicalSize` / `set_min_size`）；位置字段是物理像素（配 `PhysicalPosition`），与 `OrbConf.position_x/y` 一致。normal 尺寸以逻辑像素存储，恢复时用 `LogicalSize`，避免跨 DPI 显示器后窗口变大或变小。

数值标定依据：280 宽是单列工具页在 12px 内边距下的可用下限；420 高 = 标题栏 32 + 单工具页可滚动区。**具体数值待 §15 的实测项校准，常量集中定义以便单点调整。**

---

## 7. mini 模式

### 7.1 状态归属

| 状态 | 存放位置 | 理由 |
|---|---|---|
| `mode` | Rust 静态量 + 前端 `windowModeStore` | 运行时状态，不落盘；Rust 先于前端决定窗口参数 |
| 几何 | `AppConf` 六个字段 | 跨重启保持位置 |
| 当前工具 | 不新增；路由 `/tools/:id` 决定，`recentTools` 仅用于列表排序 | 已有数据 |

Rust 侧新增进程内状态：

```rust
static MAIN_WINDOW_MODE: LazyLock<Mutex<MainWindowMode>> = ...;  // Normal | Mini
```

`MainWindowMode` 是 Rust 内部枚举；wire 上的 `WindowModeDto` 是独立封闭集合，二者显式转换，内部类型不直接解析前端契约。

### 7.2 切换命令

`src-tauri/src/window.rs`：

```rust
#[command]
pub fn set_main_window_mode(app: AppHandle, mode: WindowModeDto) -> Result<(), String>

pub fn restore_main_window(app: &AppHandle) -> Result<(), String>   // setup 调用
pub fn persist_main_geometry(app: &AppHandle) -> Result<(), String> // 三处时机调用
```

`set_main_window_mode` 步骤：

1. 把 `WindowModeDto` 转成内部 `MainWindowMode`。
2. 读当前 mode，相同则直接返回。
3. `persist_main_geometry(app)`（写当前 mode 的几何）。
4. 按 §7.3 顺序施加尺寸、位置与置顶。
5. 更新 `MAIN_WINDOW_MODE`。
6. `emit` 事件 `ui:window-mode-changed`，载荷 `WindowModeChangedPayloadDto { mode }`。

### 7.3 施加顺序

顺序不可交换，否则被 `min_size` 钳制：

| 方向 | 顺序 |
|---|---|
| normal → mini | `set_resizable(false)` → `set_min_size(240, 320)` → `set_size(LogicalSize 280, 420)` → `set_position` → `set_always_on_top(true)` |
| mini → normal | `set_size(LogicalSize 保存值或 960, 640)` → `set_min_size(300, 200)` → `set_resizable(true)` → `set_position` → `set_always_on_top(stay_on_top)` |

位置取值：

- 进入 mini：`mini_window_x/y` 有值则用之，否则沿用当前 `outer_position()`；随后按 mini 物理尺寸做 `snap_to_nearest_screen` 收敛。
- 退出 mini：`main_window_x/y` 有值则用之，否则沿用当前位置；`main_window_width/height` 无值或小于最小尺寸时用 960×640。
- 启动永远 `normal`：只恢复 normal 尺寸与位置；`mini_window_x/y` 仅在首次进入 mini 时读取。

`set_resizable(false)` 在 mini 中生效，使 mini 尺寸不可被用户拖动破坏；退出时恢复 `true`。

### 7.4 启动恢复

`src-tauri/src/setup.rs` 的 `init` 在 `position_orb_window(handle)` 之后调用 `window::restore_main_window(handle)`：

1. 读 `AppConf`。
2. `main_window_width/height` 有效（≥ 300×200）时 `set_size(LogicalSize)`。
3. `main_window_x/y` 有值时收敛到可用屏幕后 `set_position(PhysicalPosition)`；无值时保持 `tauri.conf.json` 的居中。
4. 不恢复 mini 形态，不改 `decorations`。

前端启动后 `windowModeStore` 默认 `normal`，与 Rust 静态量一致；运行期切换由 `ui:window-mode-changed` 广播同步。

### 7.5 前端模式 store

新增 `src/stores/windowModeStore.ts`：

```ts
export type WindowMode = "normal" | "mini";

interface WindowModeState {
  mode: WindowMode;
  setMode: (mode: WindowMode) => Promise<void>;
  toggleMode: () => Promise<void>;
  applyExternalMode: (mode: WindowMode) => void;
}
```

- 初始值固定 `normal`，不做异步 hydrate。
- `setMode` 调 `windowService.setMainWindowMode(mode)` 成功后本地置位。
- `applyExternalMode` 只由 `ui:window-mode-changed` 监听器调用，处理多入口分叉。

新增 `src/services/windowService.ts`（沿用 `settingService` 的写法，从 `./tauriInvoke` 导入 `invoke`）：

```ts
export const windowService = {
  setMainWindowMode: (mode: "normal" | "mini"): Promise<void> =>
    invoke("set_main_window_mode", { mode }),
};
```

工具内的命令代理不放进 `src/services/`，而是放在工具目录内（`clock.service.ts`），与既有工具同构。

`UiEvent` 增加 `WindowModeChanged = "ui:window-mode-changed"`（`src/types/events.ts`）；Rust `event_name` 增加同名常量。

### 7.6 外壳渲染

`src/layouts/MainLayout.tsx`：

| 区域 | normal | mini |
|---|---|---|
| `Titlebar` | 38 高，含置顶/设置/主题入口 + 模式切换按钮 | 32 高，仅拖拽区、模式切换按钮、`WindowControls` |
| `AppSidebar` | 240 / 64 可折叠 | 不渲染（卸载，不是宽度归零） |
| `Content` | 现有 | 全宽 |
| `CommandPalette` | 保留 | 保留（选中不兼容目标时先切 normal，见 §7.9） |

`Titlebar` 新增 props `mode` 与 `onToggleMode`，mini 下隐藏 `ThemeToggle` / `PinToggle` / `SettingsEntry`。`decorations: false` 意味着拖拽区必须保留，否则 mini 无法移动。

`Layout` 根节点加 `data-window-mode={mode}`，供 CSS 分支；不引入新的全局样式文件。

### 7.7 mini 下的工具列表

`ToolBox` 在 `mode === "mini"` 时渲染 `MiniToolBox`（新文件 `src/tools/MiniToolBox.tsx`）：

- 顶部一个紧凑搜索框（复用 `ToolSearchBar`，`size="small"`）。
- 单列列表，行高 40：图标 18 + 名称 14px。
- 顺序：`recentTools`（最多 10）→ 收藏 → 其余按 `registry` 顺序；三组去重。
- 不做分类分组、不做拖拽排序、不渲染 `ToolCard`。

### 7.8 进入 mini 的落点

形态不跨重启，因此「启动落点」不再存在；切换按钮也**不负责导航**：

| 当前状态 | 行为 |
|---|---|
| `normal` + 当前路由兼容（`/` 或 `miniFriendly` 工具页） | 切到 `mini`，保持当前路由 |
| `normal` + 当前路由不兼容（设置、历史、非 `miniFriendly` 工具页、特性路由） | 提示 `mini.page_unsupported`，保持 `normal`，不导航 |
| `mini` | 切回 `normal`，保持当前路由 |

**不按 `recentTools` 自动挑工具**：用户刚切到 mini 时，意图是换形态，不是换工具；自动跳到最近使用过的 clock 会造成「任意位置进小窗都变成时钟」的错觉。最近使用只体现在 `MiniToolBox` 的列表顺序里。

恢复不展开、新选才展开的规则保留：mini 内从列表选中非 `miniFriendly` 工具时先切 normal。

### 7.9 路由与形态的兼容规则

唯一判断函数放在 `src/tools/miniNavigate.ts`：

```ts
resolveNavigateMode(pathname): "normal" | "mini";
navigateInCurrentMode(navigate, path): Promise<void>;
```

规则：

- `/` 与 `/tools/{miniFriendly}`：保持当前形态。
- `/tools/{非 miniFriendly}`、`/settings/*`、`/history`、特性路由：目标形态为 `normal`。
- 目标形态与当前不同时，先 `await setMode("normal")` 再 `navigate`。

两处消费：

1. `MiniToolBox` 与 `CommandPalette` 的工具/路由动作统一走 `navigateInCurrentMode`。
2. `useNavigationListener`（托盘菜单、Rust 广播导航）在 `navigate` 前调用同一判断，保证从托盘打开设置时窗口先展开。

---

## 8. 分类扩展：`productivity`

### 8.1 为什么必须新增分类

时钟的自然关键词会与既有分类撞词（`time` / `timestamp` 在 `converter`，`timer` / `schedule` 在 `dev`），撞词即平局，平局按 `CATEGORY_KEYS` 顺序归靠前分类，最终会落在「开发工具」下。因此新增分类并保证签名词独占。

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
{ key: "productivity", label: "tools.category.productivity", icon: ClockCircleOutlined, order: 4 }
```

**必须追加在数组末尾**：`classifyKeywords` 以 `score > bestScore` 比较，平局保留先出现的分类；插到中间会改变既有工具的归类结果。

### 8.3 关键词独占校验

`clock` 的 `keywords` 与旧签名交集为空：

| 工具 | keywords | 旧签名命中 | productivity 命中 | 结果 |
|---|---|---|---|---|
| `clock` | `pomodoro`, `focus`, `countdown`, `stopwatch`, `session`, `break` | 0 | 6 | `productivity` |

刻意排除的词：`timer`（在 `dev`）、`schedule`（在 `dev`）、`time` / `timestamp`（在 `converter`）、`count`（在 `dev`）。

后续任何人往 `productivity` 列表加词，须先跑一次全量归类回归（§14 的 `registry.category` 快照测试）。

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

目录 `src/tools/clock/`：`index.tsx`、`clock.service.ts`、`clock.module.css`。

### 9.2 运行态在 Rust

新增 `src-tauri/src/clock.rs`：

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ClockStatus { Idle, Running, Paused, Finished }

struct ClockInner {
    status: ClockStatus,
    duration_ms: Millis,
    started_at_ms: Option<Millis>,   // 当前运行段起点，墙钟
    elapsed_before_ms: Millis,       // 已结束运行段的累计
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
        let remaining = rt.remaining_ms(now_millis());
        if remaining <= 0 {
            if let Some(snapshot) = rt.mark_finished() {
                notify_and_emit(&app, snapshot);
            }
            return;
        }
        std::thread::sleep(Duration::from_millis(remaining.min(1000) as u64));
    });
}
```

每次最多睡 1s 即重算墙钟。Tauri 2 不提供 Electron `powerMonitor` 等价物，因此不监听休眠/唤醒，而是靠「每次唤醒后立刻按真实时间重算」吸收休眠造成的偏差。`generation` 让 `pause` / `reset` / 再次 `start` 时旧线程自然退出。

`start` / `resume` 时 `generation += 1` 后以新值 spawn，杜绝双线程并存。

### 9.3 命令

`src-tauri/src/cmd/clock.rs`：

```rust
#[command] pub fn clock_start(app: AppHandle, rt: State<Arc<ClockRuntime>>, duration_ms: Millis) -> Result<ClockSnapshotDto, String>
#[command] pub fn clock_pause(rt: State<Arc<ClockRuntime>>) -> Result<ClockSnapshotDto, String>
#[command] pub fn clock_resume(app: AppHandle, rt: State<Arc<ClockRuntime>>) -> Result<ClockSnapshotDto, String>
#[command] pub fn clock_reset(rt: State<Arc<ClockRuntime>>) -> Result<ClockSnapshotDto, String>
#[command] pub fn clock_snapshot(rt: State<Arc<ClockRuntime>>) -> Result<ClockSnapshotDto, String>
```

`ClockSnapshotDto`（`src-tauri/src/protocol/dto/clock.rs`）：

```rust
pub enum ClockStatusDto { Idle, Running, Paused, Finished }

pub struct ClockSnapshotDto {
    pub status: ClockStatusDto,
    pub duration_ms: Millis,
    pub remaining_ms: Millis,          // 服务端算好的值，供首帧渲染
    pub started_at_ms: Option<Millis>, // 前端据此本地续算
    pub elapsed_before_ms: Millis,
}
```

约束：`60_000 <= duration_ms <= 10_800_000`（1–180 分钟），越界返回错误。

- `pause` 在非 Running 时为空操作并返回当前快照。
- `resume` 在非 Paused 时同样为空操作。
- `reset` 任何状态都回到 `Idle`；`duration_ms` 保留当前值，便于前端显示上一轮时长。
- 初始 `Idle` 的 `duration_ms` 为 25 分钟，作为首帧默认值。

`ClockRuntime` 在 `setup::init` 中 `handle.manage(Arc::new(...))`。

### 9.4 前端

`src/tools/clock/clock.service.ts` 五个方法一一对应命令（从 `@/services/tauriInvoke` 导入 `invoke`）。

`src/tools/clock/index.tsx`：

- 挂载时 `clock_snapshot()` 取首帧，之后用**本地 1s 定时器**按 `started_at_ms + elapsed_before_ms + duration_ms` 续算，不做每秒 IPC。
- `document.visibilityState === "hidden"` 时停止本地定时器，`visibilitychange` 回可见时重新取快照。
- 展示：剩余 `mm:ss`、进度环、开始/暂停/继续/重置按钮、预设 15/25/45/60 分钟 + 自定义 1–180。
- 到点后显示「已完成」并保留重置入口，状态由 `clock:finished` 事件驱动。
- 窄容器下（`@container (max-width: 320px)`）字号与按钮间距降级；工具不读窗口 mode。

### 9.5 到点提醒

新增事件 `clock:finished`，载荷 `ClockSnapshotDto`。

Rust 侧在 `mark_finished()` 之后：

```
if main 窗口不可见 || 未聚焦 → 发系统通知
   若通知发送失败 → show_or_create_main + set_focus 作为兜底
else → 仅 emit 事件，由界面自行提示
```

系统通知依赖 `tauri-plugin-notification`（§12）。兜底顺序的理由：通知是「不打断」的首选，但发送失败时静默丢弃等于提醒失效，此时退化为显示窗口是唯一还能触达用户的手段。

通知正文使用 i18n（`assistant_i18n::t`），与既有 `tray.*` 一致。

---

## 10. `miniFriendly` 与 `widthMode`

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
| 内容容器 | 无容器查询 | `container-type: inline-size` |
| `key={id}` 重挂载 | 保留 | 保留 |

mini 下仍保留返回按钮（回到 `MiniToolBox`），因为 mini 里换工具是主要操作。

---

## 11. i18n

`src/i18n/locales/zh-CN.json` 与 `en-US.json` 同步新增（`i18n.d.ts` 的类型从 `zh-CN` 推导，两个文件键集必须一致）：

| 键 | 说明 |
|---|---|
| `tools.category.productivity` | 分类名 |
| `tools.clock.name` / `tools.clock.description` | 工具注册 |
| `tools.clock.start` / `pause` / `resume` / `reset` / `finished` | 界面 |
| `tools.clock.minutes` / `custom_minutes` / `invalid_duration` | 时长 |
| `mini.toggle_to_normal` / `mini.toggle_to_mini` | 模式切换按钮 |
| `mini.page_unsupported` / `mini.page_unsupported_desc` | 当前页面不支持小窗的提示（走 `notifyStore`） |
| `clock.notification_title` / `clock.notification_body` | Rust 侧通知（`assistant_i18n::t`） |

Rust 侧 i18n 资源与前端 JSON 是两套文件，`clock.notification_*` 需同时加入 Rust 的资源文件。

---

## 12. 依赖与能力

| 变更 | 位置 |
|---|---|
| `tauri-plugin-notification = "2"` 加入 workspace 与根包 `[dependencies]` | `src-tauri/Cargo.toml` |
| `.plugin(tauri_plugin_notification::init())` | `src-tauri/src/plugins.rs` |
| `"notification:default"` 加入 permissions | `src-tauri/capabilities/default.json` |

不引入 `@tauri-apps/plugin-notification`：通知只从 Rust 侧发起，前端不调 JS API。

capability 的具体标识符以构建期生成的权限 schema（`src-tauri/gen/schemas/`，未纳入版本库）为准，落地时核对；`notification:default` 不覆盖所需子权限时按 schema 补 `notification:allow-*`。

`set_resizable` / `set_min_size` 不在 `default.json` 的窗口权限列表中，但 §7 的全部窗口操作都在 Rust 侧直接调用 `WebviewWindow` 方法，capabilities 只约束前端 invoke，因此**不需要为它们新增前端权限**。

---

## 13. 文件清单

新增：

| 文件 | 内容 |
|---|---|
| `src/stores/windowModeStore.ts` | 模式 store |
| `src/services/windowService.ts` | `set_main_window_mode` 代理 |
| `src/tools/MiniToolBox.tsx` | mini 下的紧凑列表 |
| `src/tools/miniNavigate.ts` | 路由/工具与形态的统一判断 |
| `src/tools/clock/{index.tsx,clock.service.ts,clock.module.css}` | 时钟工具与命令代理 |
| `src-tauri/src/clock.rs` | 时钟运行态与 ticker |
| `src-tauri/src/cmd/clock.rs` | 时钟命令 |
| `src-tauri/src/protocol/dto/clock.rs` | 时钟 DTO 与模式 DTO |
| `src-tauri/tests/clock_runtime.rs` | 时钟状态机集成测试 |

修改：

| 文件 | 改动 |
|---|---|
| `src-tauri/tauri.conf.json` | 无（主窗口保持静态声明） |
| `src-tauri/crates/assistant-config/src/app_conf.rs` | 6 个几何字段 |
| `src-tauri/crates/assistant-config/src/constant.rs` | 4 个 mini 常量 |
| `src-tauri/src/protocol/dto/config.rs` | `AppConfDto` 6 个字段 |
| `src/settings/core/types/confTypes.ts` | 自动经 `AppConfDto` 获得新字段，无需手改 |
| `src-tauri/src/window.rs` | `set_main_window_mode` / `restore_main_window` / `persist_main_geometry` / `effective_on_top` |
| `src-tauri/src/setup.rs` | `ClockRuntime` manage、`restore_main_window` 调用 |
| `src-tauri/src/cmd/mod.rs` | 导出 `clock` 模块 |
| `src-tauri/src/cmd/config.rs` | `stay_on_top` 走 `effective_on_top`；几何写入复用配置锁 |
| `src-tauri/src/cmd/app.rs` | `perform_exit` 前 `persist_main_geometry` |
| `src-tauri/src/ipc.rs` | 注册 6 个新命令（时钟 5 + 模式 1） |
| `src-tauri/src/protocol/events.rs` | `CLOCK_FINISHED` / `WINDOW_MODE_CHANGED` |
| `src-tauri/src/protocol/dto/mod.rs` | 导出 `clock` |
| `src-tauri/src/plugins.rs` | notification 插件 |
| `src-tauri/capabilities/default.json` | notification 权限 |
| `src-tauri/src/lib.rs` | `mod clock` |
| `src-tauri/tests/protocol_bindings.rs` | 新 DTO 加入导出列表 |
| `src/tools/types.ts` | `miniFriendly` |
| `src/tools/categories.ts` | `productivity` 键 / 签名 / meta |
| `src/tools/registry.ts` | `clock` 的 `defineTool` |
| `src/tools/ToolBox.tsx` | mini 分支 |
| `src/tools/ToolPage.tsx` | 密度分支 + 容器查询锚点 |
| `src/tools/tool-page.css` | 容器查询 |
| `src/layouts/MainLayout.tsx` | 按 mode 渲染外壳、模式监听、切换落点 |
| `src/components/Titlebar.tsx` | `mode` / `onToggleMode` |
| `src/types/events.ts` | `UiEvent.WindowModeChanged` |
| `src/config/constants.ts` | mini 常量 |
| `src/components/commandPalette/commandAggregation.tsx` | 路由/工具动作走 `miniNavigate` |
| `src/hooks/useNavigationListener.ts` | 广播导航前先切形态 |
| `src/i18n/locales/{zh-CN,en-US}.json` | 键集 |
| `src-tauri/crates/assistant-i18n/locales/{zh-CN,en-US}.json` | 通知文案 |

---

## 14. 验收

构建与静态检查：

- [ ] `npm run typecheck` 通过
- [ ] `npm run lint` 通过
- [ ] `npm run test` 通过
- [ ] `npm run contract:check` 通过（提交后跑；`contract:gen` 会先改写目标文件，未提交时 diff 必然非空）
- [ ] `cargo fmt` 后运行集成测试（cwd = `src-tauri`）：
  `cargo test -p assistant-app --test clock_runtime --test protocol_bindings --test protocol_contract --test assembly_contract`
  - 本机已知限制：Windows 上 `assistant-app` 的 lib 测试二进制不带 Common Controls v6 清单，链接 tauri dialog 代码后启动即 `STATUS_ENTRYPOINT_NOT_FOUND`，因此时钟单测放在集成测试目标中

单元测试（新增）：

- [ ] `classifyKeywords` 回归：既有工具的归类结果与改动前逐项相同
- [ ] `clock` 的 keywords 归类为 `productivity`
- [ ] `ClockRuntime`：`start → 前进 now → remaining` 单调递减；`pause` 后 remaining 冻结；`resume` 后继续；`mark_finished` 幂等；`generation` 使旧 ticker 退出（`src-tauri/tests/clock_runtime.rs`）
- [ ] `resolveMiniLanding` 已移除；切换按钮不导航，不兼容页面只提示
- [ ] `miniNavigate`：`resolveNavigateMode` 对 `/`、`/tools/{miniFriendly}`、`/tools/{非 miniFriendly}`、`/settings/*`、`/history` 的判定

手工回归：

- [ ] normal → mini → normal 往返，窗口尺寸/位置正确，mini 内不可拖拽改尺寸
- [ ] 重启后回到上次的 normal 位置/尺寸；重启后不会直接进入 mini
- [ ] 再次进入 mini 停在上一轮的 mini 位置
- [ ] mini 下 clock 无横向溢出、无被裁切按钮
- [ ] normal 下把窗口拖到 300 宽，clock 自动切窄态（验证容器查询而非 mode）
- [ ] mini 下从 `MiniToolBox` 选一个非 `miniFriendly` 工具（如 `json-viewer`）：窗口先展开再进入
- [ ] mini 下从托盘打开设置：窗口先展开再进入
- [ ] 时钟运行中隐藏主窗口，到点仍触发提醒
- [ ] 分类页出现 `productivity` 分组，仅含 clock

---

## 15. 未验证前提

以下在实现前无法凭代码确定，落地时需实测并回填本文：

1. **280px 下 Ant Design 组件的最小宽度**：`Input`、`Button` 组合是否溢出。若溢出，调 `MINI_WINDOW_WIDTH` 而非改组件尺寸。
2. **Windows 上 `set_min_size` 小于当前尺寸时的即时行为**：部分版本会立刻把窗口放大，验证 §7.3 顺序是否需要额外一次 `set_size`。
3. **通知权限首次请求的时机**：macOS 在首次 `notify` 时弹授权，可能在 mini 下打断操作；若体验不佳，改为设置页显式授权。
4. **系统休眠期间 `thread::sleep` 的返回语义**：Windows 唤醒后 sleep 是否立即返回并重算，需实测；若不返回，ticker 会在唤醒后延迟至下一次 1s 边界，属可接受误差，但要确认不是「永久卡死」。
5. **切换形态时的窗口重排**：`set_size` + 外壳切换是否产生明显闪烁，需录屏逐帧检查。
6. **mini 下 `CommandPalette` 的可用性**：全局面板在 280px 宽度下是否仍可操作，若不可用则在 mini 中禁用并只保留 `MiniToolBox` 搜索框。

---

## 16. 延期项：便签（notes）

本阶段不实现。保留以下已定结论，供后续单独排期（消费者接入时同时启用 `docs/store/I27-tool-data-store-spec.md`）：

- 便签是原始数据，落 `tool-data.db` 的 `note_entries` 表，不写 `assistant.db`。
- `widthMode: "wide"` + `miniFriendly: true`：normal 双栏（列表 240 + 编辑器），mini 单栏（列表/编辑器切换）。
- 写盘成功才算保存；自动保存 800ms 防抖，失焦/切换/卸载立即 flush。
- 列表只回 `excerpt`，不回全文；软删 + 回收站恢复。
- 关键词使用 `note` / `memo` / `scratchpad` / `sticky` / `todo` / `checklist` / `journal` / `diary`，与 clock 同属 `productivity`。
</content>

# I75 — 启动链路分层机制规格

| 项 | 值 |
|---|---|
| 日期 | 2026-11-25 |
| 类型 | 设计规格（机制与契约） |
| 关联 | APP-STARTUP-ANALYSIS（预先分析，事实来源，保持冻结）、I67（E14 WebView2 profile）、I44（app 目录管理）、I42（日志策略）、I62（协议 crate）、I63（领域放置）、feature-mount-traversal-spec（已归档，装配契约范式）、CODE-SPLIT-PRINCIPLE、CODE-REVIEW-PRINCIPLE |
| 一句话 | 把启动从「一串按顺序执行的步骤」认成「一张资源表」：顺序是表序，泳道是一列，失败语义是集合成员，就绪状态只有一个 `ensure` |

编号前缀约定：`N` 现状、`J` 判定、`S` 表约束、`C` 能力契约、`F` 前端改动、`M` 度量口径、`G` 门禁、`I` 不变式、`A` 验收、`X` 不做、`R` 风险、`P` 实施阶段。前缀互不重叠，正文内引用不再歧义。

---

## 0 判定

本规格要拍板的内容。每条给出结论与依据；未列入此表的取舍由实施者按 §3.3 判据自行定档。

| 编号 | 判定 | 依据 |
|---|---|---|
| J1 | 启动时延拆成三个指标（T_process / T_visible / T_wake），不再用单一「启动耗时」 | 两个窗口 `visible: false`，用户首个可见像素是 Orb（`src-tauri/tauri.conf.json`）；主窗口唤起是独立链路 |
| J2 | 启动期的唯一声明是 `RESOURCES` 一张表；`lane` 是表的一列，表序即执行序；`needs` 只做静态校验，不做拓扑排序 | 拓扑排序会静默改变既有顺序；单表可读、可 diff，与 `MOUNTS` 范式一致；D10 亦要求「启动初始化必须可并行、可延迟；阻塞首屏的串行链即为缺陷」 |
| J3 | 预热不需要独立清单：`lane: Idle` 的项就是预热清单；`Capability` 枚举与 `WARMUP` 表都不存在，id 即能力名 | 四份声明（阶段表 / 能力枚举 / 预热清单 / 前端需求表）描述同一件事，同步成本全转嫁到门禁；单表 + 一列即可 |
| J4 | `severity` 不作为表字段手写，由四份集合（`PRIMARY_ENTRY` / `BOOT_REQUIRED` / `OBSERVABLE` / `BEST_EFFORT`）的成员关系派生；G1 断言四份集合构成表的全划分 | 手写无法表达「入口类至少存活一个」；派生让「tray + orb_shell 同时降级 = 幽灵进程」在结构上不可能出现 |
| J5 | `budget_ms: 0` 表示未测量；G1 放行 0，G8 只对非零项断言 p95 | 若要求 `> 0`，P0 脚手架填任意值即可全绿，门禁形同虚设 |
| J6 | 配置只在启动期做一次快照（`BootConf`），不引入带失效机制的全局缓存 | 运行期「改设置立即生效」是既有语义，缓存会与之冲突 |
| J7 | 窗口规格的唯一来源是 `WINDOW_SPECS`；其 `confs` 列是前端窗口配置需求表的唯一来源，经 `contract:gen` 生成前端常量与 `WindowLabel` 联合类型 | 跨语言「两边各写一份 + 正则断言相等」既脆又漂；生成物把 G4 变成类型等式（漏一个窗口编译不过）。后端本来就知道 label，把知识拆到两种语言才需要补丁 |
| J8 | 主窗口懒建不是特例：`window_main` 是普通表项，`lane` 就是性能开关（`Idle` ↔ `Boot` 一行切换） | 有了统一 `ensure`，预热调度器、抢焦点启发式、幂等约束都是「缺少统一机制」的产物，随之消失 |
| J9 | 时钟同域：`startup_id` 与 `t0` 由 `initialization_script` 注入 webview；前端回报 epoch 毫秒（`performance.timeOrigin + performance.now()`），Rust 直接相减；首帧回报不占 IPC 往返 | 现设计「Rust 一段 + 前端一段相加」是跨时钟域相加，`startup_id` 只解决关联不解决时基 |
| J10 | 前端核心配置槽（`app`/`orb`/`mascot`）静态声明（零 `import`）；特性槽随页面进入时 `import` 并注册 | `await loadFeatures()` 留在 render 前会把 vault/clock/observation/orchestration 全量拉入首帧，G7 会绿而 T_visible 几乎不动 |

---

## 1 定位与范围

### 1.1 本规格管什么

- 启动链路的**结构**：资源表、泳道归属、失败语义、就绪状态、单一来源。
- 启动链路的**可观测与门禁**：`startup_id`、阶段日志、首帧回传、预算、契约测试。
- 首帧前的最小依赖集，以及前后端各自的最小启动路径。

### 1.2 本规格不管什么

- 具体某一项耗时怎么削（例如 DB 迁移是否再拆），由 P0 度量数据决定，不在本文预设。
- 运行期策略（审批超时每次现读、几何在关窗时落盘）保持原状。
- 视觉与交互调整（Orb 显示时机除外，见 §6）。

### 1.3 与 `APP-STARTUP-ANALYSIS.md` 的关系

分析文档是**事实快照**（其 §7 已自述此定位）。本规格承接其结论，但**纠正其目标定义**：分析文档通篇用「首帧」指主窗口首帧，而主窗口启动期不可见，因此优化目标必须按 §3.1 拆成三个指标。此后以本规格为唯一执行口径；分析文档不再修改。

---

## 2 现状取证

以下为撰写本规格时逐条核对代码得到的事实，作为设计输入。

| 编号 | 事实 | 位置 |
|---|---|---|
| N1 | `setup::init` 单函数串起 17 个步骤，覆盖配置、DB、附件、状态槽、工具装配、托盘、窗口、快捷键 8 类变化原因 | `src-tauri/src/setup.rs` |
| N2 | 同一函数内失败语义并存：特性 `on_setup`、`AppConf::load`、`OrbConf::load` 只记日志；`AppDirs::resolve`、DB 初始化、附件目录用 `?` 终止 | `src-tauri/src/setup.rs` |
| N3 | 插件顺序约束只存在于注释（「single-instance 必须排在 log 之前」），无断言 | `src-tauri/src/plugins.rs` |
| N4 | 启动期 `AppConf::load()` 8 次（`setup.rs` 4 次、`window.rs` 1 次、`tool_setup.rs` 3 次）；每次都是真读盘（`conf_dir_root()` 重新解析目录 + 读文件） | `src-tauri/src/setup.rs`、`src-tauri/src/window.rs`、`src-tauri/src/tool_setup.rs` |
| N5 | `OrbConf::load()` 在启动期 4 次（配置校验、Orb 定位、窗口属性、巡检线程内回写） | `src-tauri/src/setup.rs`、`src-tauri/src/window.rs` |
| N6 | 窗口规格双来源：`tauri.conf.json` 声明 main + orb，`WINDOW_SPECS` 只含 main；Orb 的属性由 `apply_orb_window_attrs` 二次修补 | `src-tauri/tauri.conf.json`、`src-tauri/src/window.rs` |
| N7 | `show_or_create` 是唯一建窗入口，显式 `data_directory(dirs.webview_data_directory())`；conf 声明的窗口走 Tauri 默认 | `src-tauri/src/window.rs` |
| N8 | 主窗口几何恢复依赖窗口已存在（`get_or_error` 失败即 `Err`），由 `setup` 调用一次 | `src-tauri/src/window.rs` |
| N9 | **沙箱探测失败今日已是降级语义**：`tool_setup::init` 探测失败只 `log::warn!("shell tool not registered")` 后继续。该函数内真正的 `?` 是首行重复的 `AppDirs::resolve()?` 与工具结果目录 I/O | `src-tauri/src/tool_setup.rs` |
| N10 | 命令层与适配层直接 `state::<Arc<ToolRegistry>>()` / `state::<Arc<PipelineToolExecution>>()` 取用共 5 处（`cmd/agent/mod.rs` 3 处、`cmd/mcp.rs` 1 处、`adapters/runner.rs` 1 处）；另有 2 处装配内部取用（`tool_setup.rs`、`mcp/config.rs`）；`cmd/storage` 的 2 处已用 `try_state` 可降级。未就绪即 panic | 同上 |
| N11 | 前端 `bootstrap()` 无条件 `loadFeatures()` + 加载**全部**配置组，再单独 `applyLocaleChange`；`loadConf("app")` 内部已经会应用语言 | `src/main.tsx`、`src/stores/settingsStore.ts` |
| N12 | `themeStore.init` 再次 `fetchConf("app")`，与 `bootstrap` 的 `app` 组重复 IPC | `src/stores/system/themeStore.ts` |
| N13 | `App.tsx` 静态 `import` 两个布局，Orb 窗口也会拉入 `MainLayout` 整包；`loadFeatures` 无条件加载 vault/clock/observation/orchestration | `src/App.tsx`、`src/features/registry.ts` |
| N14 | 启动路径无 `Instant`、无预算、无回归阈值；D10 判据：「启动关键路径必须有可测量的预算与回归阈值；无预算记录或持续超标且无处理即为缺陷」 | 全链路 |
| N15 | `AppDirs::resolve()` 在启动期被解析 6 处：`main.rs`、`plugins.rs::log_plugin`（取日志目录）、`setup.rs`、`tool_setup.rs`、`window.rs::show_or_create`（每次建窗各一次）、特性 `on_setup`（`vault_plugin.rs`）。此外 `assistant-config::conf_dir_root()` 在另一 crate 内自解析，启动期每次配置读盘各一次——该点不在 `src-tauri/src` 内，源码断言天然覆盖不到 | 同上 |
| N16 | `ipc.rs` 的 `generate_handler!` 未含任何 startup 命令；`assistant-protocol` 的 `lib.rs` 无 startup 模块，新增 DTO 需同步登记 | `src-tauri/src/ipc.rs`、`src-tauri/crates/assistant-protocol/src/lib.rs` |

---

## 3 分层模型

### 3.1 三个时延指标

| 指标 | 起点 | 终点 | 用户感受 |
|---|---|---|---|
| **T_process** | 进程创建 | `Builder::run` 进入事件循环 | 双击到「有反应」 |
| **T_visible** | 进程创建 | Orb 首帧可见且位置正确 | 真正的「启动完成」 |
| **T_wake** | 托盘 / 快捷键 / Orb 触发 | 主窗口首帧 | 「唤起快不快」 |

优化取舍必须同时报告三个值。只降 T_process 不降 T_visible 没有意义；只降 T_visible 而抬高 T_wake 属于体验倒退。

### 3.2 泳道 = `lane` 列

| `lane` | 时点 | 触发 | 失败策略 | 判据落点 |
|---|---|---|---|---|
| **`Boot`**（A 首帧前） | 同步，有预算 | `run_boot` 按表序 | Fatal 或 Degraded | 错过即无第二次机会、或首帧直接依赖 |
| **`Idle`**（B 首次使用前） | 异步，Orb 首帧后 | `run_idle` 遍历本泳道调 `ensure` | Degraded / Ignore | 功能可用性前提，用户尚未需要 |
| **`OnDemand`**（C 完全懒） | 首次进入对应界面 | 命令层 / 页面 `ensure(id)` | Degraded | 页面级资源与运行时 |

`Boot` 分两段：**Builder 段**（插件注册、`generate_context`）与 **boot 段**（`run_boot`）。Builder 段的顺序约束不进表（Tauri 决定其执行时机），由 §8 G2 的源码断言守住。

`Idle` 泳道就是预热清单本身，**没有独立调度器**：`report_first_frame` 命令写入首帧记录后，若该进程尚未启动 idle pass，则 `spawn` 一次 `run_idle`（一次性，由 `OnceLock` 守住）。不需要「何时预热 / 是否让位 Orb 交互」的启发式——`ensure` 的 single-flight 已保证幂等，预热与首次使用是同一个函数。

### 3.3 泳道判据（决策程序）

任何新增启动工作，按第一个「是」定档：

1. 错过就有不可恢复后果（唯一安全时点 / 会写错数据 / 失去入口）→ **`Boot` + 致命**
2. 首帧必须可见或必须依赖 → **`Boot`**
3. 首次使用前必须完成，且失败可降级 → **`Idle`**
4. 其余 → **`OnDemand`**

这条判据替代「步骤清单」：清单随功能增长而失效，判据不会。判据写进规格，是为了让新增条目不需要重新开会讨论。

### 3.4 唯一入口：`ensure`

禁止形态：

```text
// 错误：启动后延迟 N 秒装配工具
spawn(async { sleep(3s); tool_setup::init(&app); })
```

该形态下「第 2 秒点发送」与「第 3 秒装配」之间存在竞态，且每加一个 `Idle` 项就多一处竞态。

规定形态：

```text
命令层 / 适配层 / idle pass → ensure(app, "tool_runtime").await
                                ↑ single-flight：并发共享同一次装配，失败保留原因并可重试
```

`run_idle` 与首次使用调用的是同一个 `ensure`，不存在第二套装配实现。

### 3.5 主入口唯一性

「无入口」必须致命，但平面表的逐行判定表达不了「至少存活一个」。解法是把主入口收敛为一个：**`orb_shell` 是唯一 `PRIMARY_ENTRY`**（Orb 是启动期唯一可见窗口，T_visible 的终点）。托盘是加速器而非入口（Orb 面板可覆盖托盘主路径），因此 `tray` 可降级。这样「幽灵进程」在结构上不可能出现，而不需要额外的条件规则。

---

## 4 后端契约

### 4.1 `RESOURCES` 表

```rust
// src-tauri/src/boot/resources.rs —— 只放声明，不放逻辑
pub enum Lane { Boot, Idle, OnDemand }

pub struct Res {
    pub id: &'static str,
    pub lane: Lane,
    pub needs: &'static [&'static str],
    pub budget_ms: u32,          // 0 = 未测量
    pub build: BuildFn,          // 幂等装配；不读配置；只从 ctx 取
}

pub type BuildFn = fn(&StartupCtx) -> Result<CapValue, CapErr>;
pub type CapValue = std::sync::Arc<dyn std::any::Any + Send + Sync>;

pub static RESOURCES: &[Res] = &[ /* 见 §9 接入矩阵 */ ];
```

约束：

| 编号 | 约束 | 由谁保证 |
|---|---|---|
| S1 | 执行顺序 = 表内声明顺序 | `run_boot` |
| S2 | `needs` 中的每个 id 必须在本项之前声明，且 `needs` 闭包完整 | §8 G1 |
| S3 | `id` 唯一；`PRIMARY_ENTRY ⊆ lane Boot`；`BOOT_REQUIRED ⊆ lane Boot`；`lane Idle` 不得致命 | §8 G1 |
| S4 | `budget_ms` 允许为 0（未测量），非零项由 G8 断言 | §8 G1 / G8 |
| S5 | `build` 内不得读配置、不得 `manage` 之外产生全局副作用；跨项产物只经 `ctx` 取。唯一例外是 `boot_conf` 自身（它就是唯一读盘点，且只经 `ctx.dirs()` 定位路径） | §8 G3 |
| S6 | 被 `BOOT_REQUIRED` 依赖的 `Degraded` 项，其 `build` 不得返回 `Err`：失败必须回退默认值并 `ctx.report.degrade(id, reason)`。否则下游拿到 `NotReady` 会升级为 Fatal，等于「配置读失败 = 进程退出」，与 §4.3 的「只记日志」相反。`boot_conf` 即此类 | §4.3 / §11 I4 |
| S7 | 依赖项失败只让本项 `NotReady` 并记一次降级，**不把本项升级为 Fatal**；`Fatal` 只由本项自身失败或 `BOOT_REQUIRED` 成员自身失败产生 | §11 I4 |

`needs` 取代原设计的 `after`：`after` 在「顺序 = 表序」下恒真，是没有信息量的第二份顺序副本；`needs` 可被 G1 断言为真实依赖闭包（例如「凡读 `ctx.boot()` 的项必须 `needs` 含 `boot_conf`」）。由于 `build` 是函数指针、运行期不可内省，该闭包断言必须由**源码扫描**实现：`boot/build/*.rs` 中出现 `ctx.boot()` 的文件，其对应表项 `needs` 必含 `boot_conf`（文件↔id 映射见 §9）。

### 4.2 `StartupCtx` / `BootClock` / `StartupReport`

```rust
pub struct StartupCtx<'a> {
    pub app: &'a AppHandle,
    pub clock: &'a BootClock,
    pub report: &'a StartupReport,
    slots: Mutex<HashMap<&'static str, CapValue>>,   // 与 ensure 共用的产出槽
}

impl StartupCtx<'_> {
    pub fn dirs(&self) -> Result<Arc<AppDirs>, CapErr>;      // 槽缺失返回 NotReady
    pub fn boot(&self) -> Result<Arc<BootConf>, CapErr>;
}

pub struct BootClock {
    pub startup_id: String,      // uuid v4，一次进程一份
    pub t0: Instant,             // 单调时钟，进程内分段计时
    pub t0_epoch_ms: i64,        // 与前端 performance.timeOrigin 同域，跨端相减
    pub last: Mutex<Instant>,
}

pub struct StartupReport {
    pub phases: Mutex<Vec<PhaseRecord>>,
    pub degraded: Mutex<Vec<CapIssue>>,
    pub first_frame: Mutex<Option<FrameRecord>>,
}
```

- `BootClock` 与 `StartupReport` 均 `manage` 进 Tauri state，供命令层与退出期读取。
- `t0` 与 `t0_epoch_ms` 在同一行同时采集；跨端只做一次减法（§7.3 M4），不做时钟校准。
- `PhaseRecord { id, ms, outcome }` 只保留 id 级粒度，不记录参数，避免把配置内容写进日志。
- `BootConf` 由 `boot_conf` 表项构建后进入槽；`window.rs` 中**启动路径上的**几何与 Orb 属性函数改为收 `&BootConf`。运行期调用点（`set_main_window_mode`、`cmd/config.rs` 的 Orb 属性刷新、`orb_overlay`）保持现读 `AppConf`/`OrbConf`，否则会把配置冻结在启动时刻（§11 I2）。

### 4.3 失败语义：集合成员派生

```rust
// P1 基线（严格复刻现状档位）；P3 的降档见本节末
pub const PRIMARY_ENTRY: &[&str] = &["orb_shell"];
pub const BOOT_REQUIRED: &[&str] = &["dirs", "storage", "runtime_state", "i18n", "tool_runtime"];
pub const OBSERVABLE: &[&str] = &[
    "boot_conf", "attachment_store", "feature_on_setup", "tray",
    "window_main", "storage_maintenance",
];
pub const BEST_EFFORT: &[&str] = &["screen_inspection", "shortcuts", "mcp_start"];
pub const ACTIONABLE: &[&str] = &[];          // P3 变为 &["tool_runtime"]

pub fn severity(id: &str) -> Severity {
    if PRIMARY_ENTRY.contains(&id) || BOOT_REQUIRED.contains(&id) { Severity::Fatal }
    else if OBSERVABLE.contains(&id) { Severity::Degraded }
    else { Severity::Ignore }                 // BEST_EFFORT 及未列项
}
```

四份集合必须构成全表的**互斥全划分**（每个 id 恰属一份，由 G1 断言）。`BEST_EFFORT` 与 `ACTIONABLE` 是显式集合而非注释，因为 `Ignore` 项也要能被遍历（G1 覆盖全表、C4 广播可枚举），而「可操作提示」需要一个可断言的名单。由此获得：

| `Severity` | runner 行为 | 适用判据 | 典型表项 |
|---|---|---|---|
| `Fatal` | 记日志 + 记 `PhaseRecord` + 终止启动 | 继续跑会写错数据，或失去唯一入口 | `dirs`、`storage`、`i18n`、`runtime_state`、`orb_shell`、`tool_runtime`（P1） |
| `Degraded` | 记日志 + 记 `CapIssue` + 广播 + 继续 | 功能缺失但应用可用，且**可观测**（`StartupReport.degraded` 有记录）、**不级联** | `boot_conf`、`attachment_store`、`feature_on_setup`、`tray`、`window_main`、`storage_maintenance`、`tool_runtime`（P3） |
| `Ignore` | 只 warn | 尽力而为 | `screen_inspection`、`shortcuts`、`mcp_start` |

降级规则比原设计更宽松也更诚实：原设计要求 Degraded 必须有「用户可见降级提示」，但表内 Degraded 项大多没有 UI，规则自身不成立。现规则改为「可观测 + 不级联」，**仅当降级对用户可操作（如工具不可用）时才额外要求前端提示**，该提示由 `ACTIONABLE` 集合驱动（P3 的 `startupStore` 只覆盖 `tool_runtime`）。

`tool_runtime` 的降档（P3）必须是同一提交内的显式改动：`lane` 由 `Boot` 改为 `Idle`，从 `BOOT_REQUIRED` 移到 `OBSERVABLE`，并加入 `ACTIONABLE`。三处同批改，diff 即设计意图，**不允许只改其中一处**。

### 4.4 能力门禁

```rust
// src-tauri/src/boot/mod.rs
pub async fn ensure(app: &AppHandle, id: &'static str) -> Result<CapValue, CapErr>;
pub fn state_of(app: &AppHandle, id: &str) -> CapState;   // Pending / Ready / Failed

// src-tauri/src/boot/access.rs —— 每个资源一行类型化访问器
pub fn tools(app: &AppHandle) -> Result<Arc<ToolRegistry>, CapErr>;
pub fn boot_conf(app: &AppHandle) -> Result<Arc<BootConf>, CapErr>;
pub fn db(app: &AppHandle) -> Result<DbState, CapErr>;
// ...

pub enum CapErr {
    NotReady { id: &'static str },                  // 未装配，非错误状态
    Build { id: &'static str, message: String, retryable: bool },
}
```

| 编号 | 契约 |
|---|---|
| C1 | `ensure` 对同一 `id` 做 single-flight：并发调用共享一次装配结果 |
| C2 | 装配失败时保留 `CapErr::Build`；下一次 `ensure` 允许重试（`retryable` 为真时） |
| C3 | 命令层与适配层禁止对 `boot/access.rs` **已登记**的类型做 `state::<Arc<T>>()` / `try_state::<Arc<T>>()` 直取，也禁止调用 `boot::registry` 内部 API；只能经 `access::*`。未登记的类型（如今日的 `McpServerManager`，`cmd/mcp.rs`、`mcp/config.rs`）暂不在禁令内，随迁移逐项并入 `access.rs` 与 G5 清单 |
| C4 | 状态变更广播 `startup:capability-changed`（遵循既有 `domain:event` 事件名形式），payload 的 id 使用生成的前端联合类型 |
| C5 | 前端在 `Pending` 期间把「发送 / 工具面板」置为「正在准备」，在 `Failed` 时显示可读原因 + 重试入口 |
| C6 | `access::*` 在未就绪时返回 `NotReady`，**不 panic、不隐式装配**；需要装配的调用点显式 `ensure` |

资源的 `manage` 时机：`build` 成功后由 registry 统一 `manage` 一次（single-flight 保证仅一次，重复 `manage` 会 panic）。这保留了对尚未迁移的 `state::<>()` 调用的兼容，但 G5 禁止新增直取。

### 4.5 单一来源清单

| 事项 | 唯一来源 | 被约束的副本 |
|---|---|---|
| 启动顺序 | `RESOURCES` 表序 | `run_boot` 不写顺序 |
| 泳道归属与预热清单 | `lane` 列 | 无第二份 `WARMUP` |
| 失败语义 | 四份集合 + `severity()` | 表项不写 severity 字段 |
| 窗口规格 | `WINDOW_SPECS` | `tauri.conf.json` 的 `windows` 必须为空数组 |
| 窗口配置需求（前端） | `WINDOW_SPECS[i].confs` → 生成物 | 前端不手写、组件不自调 `loadConf` |
| 窗口 profile 落点 | `show_or_create` 中的 `data_directory` | 无第二处 |
| 目录解析 | `main.rs` 一次，`run(dirs)` 沿调用链下传，注入 `dirs` 表项 | 启动路径（`plugins.rs`、`setup.rs`、`tool_setup.rs`、`window.rs` 建窗路径、特性 `on_setup`）内不再 `AppDirs::resolve()` |
| 配置目录 | `dirs` 表项（`BootConf::load_all(&dirs)` 经 `load_from` 显式传路径） | `assistant-config::conf_dir_root()` 在启动路径上不再被触发（它在另一 crate，源码断言覆盖不到，只能靠这条约束守） |
| 启动期配置 | `BootConf`（`boot_conf` 表项） | 启动路径不得再 `AppConf::load()` / `OrbConf::load()` |
| 能力就绪 | `ensure` + `access` | 命令层不得直取未就绪实例 |

---

## 5 目录切分

`src-tauri/src/setup.rs` → `src-tauri/src/boot/`，按变化原因切分（对齐 CODE-SPLIT-PRINCIPLE 的目录模块模式）。原设计的 `caps/` 目录取消：能力门禁就是 `boot/registry.rs` 的 single-flight 状态机，`Capability` 枚举不需要存在。

| 文件 | 职责（唯一变化原因） | 上限 |
|---|---|---|
| `boot/mod.rs` | 对外导出 `ensure` / `access` / `run_boot` / `run_idle`；不含业务 | 60 行 |
| `boot/resources.rs` | `RESOURCES` 声明表 + 四份集合 + `Lane` | 表项数 ×4 行 |
| `boot/registry.rs` | single-flight 槽、重试状态机、`manage` 一次性 | 90 行 |
| `boot/runner.rs` | `run_boot`（表序 + 计时 + severity 分派）、`run_idle` | 80 行 |
| `boot/ctx.rs` | `StartupCtx` / `BootClock` / `StartupReport` / `PhaseRecord` / `CapIssue` | 120 行 |
| `boot/conf.rs` | `BootConf::load_all(&dirs)`：经 `load_from` 显式传路径，启动期唯一读盘点；失败回退默认值 + `ctx.report.degrade`，永不返回 `Err`（S6） | 80 行 |
| `boot/access.rs` | 每资源一行的类型化访问器 | 资源数 ×6 行 |
| `boot/build/dirs.rs` | `AppDirs` 注入 + profile 建目录与清理标记消费 | 60 行 |
| `boot/build/storage.rs` | DB 打开、迁移、种子、`DbState` | 120 行 |
| `boot/build/attachments.rs` | 附件目录 + `AttachmentStore` | 60 行 |
| `boot/build/state.rs` | 5 个运行状态槽 | 40 行 |
| `boot/build/tools.rs` | 工具基础设施装配（现 `tool_setup::init` 主体迁入，保留 `tool_setup.rs` 的纯函数） | 现状 + 少量 |
| `boot/build/mcp.rs` | `mcp_start`（拉起已启用 server） | 40 行 |
| `boot/build/windows.rs` | `orb_shell` / `window_main` 的 build 函数（调 `window.rs` 的 spec 与建窗） | 80 行 |
| `boot/build/shell.rs` | 托盘、快捷键、屏幕巡检线程 | 180 行 |
| `boot/build/maintenance.rs` | 审计裁剪、工具结果清理、附件 `clean_tmp` | 80 行 |

`WindowSpec` 与 `WINDOW_SPECS` 迁到 `assistant-config/src/window_spec.rs`（纯数据、无 tauri 依赖），`window.rs` 与 `boot/build/windows.rs` 都只是调用方。这样 `contract:gen` 的生成器可以落在 `assistant-config` 的测试里——`assistant-protocol` 只依赖 `serde`/`ts-rs`，读不到这张表。`boot/mod.rs` 只做组装，永远不成为最大文件——这是拆分原则对「god file」的直接约束。

---

## 6 前端对称设计

### 6.1 首帧最小依赖 = 本窗口的 `requirements`

```text
initSettingSystem()                                // 注册核心三组静态槽（零 import）
await loadConfs(WINDOW_CONF_REQUIREMENTS[label])   // 本窗口首帧依赖，含 app（语言在内部应用）
await themeStore.initLocal()                       // 从 confs.app 读 theme，只等 resolved
→ render
→ 渲染后：themeStore.subscribe()（onThemeChanged + ThemeChanged 监听）
→ 渲染后：主窗口 void loadFeatures()（不阻塞首帧）
→ rAF 回报首帧
```

render 前加载的是**本窗口的 requirements**，不是固定的 `app` 组：Orb 需要 `app`+`orb`+`mascot`，若只等 `app` 就会先以默认主题/吉祥物画一帧再重绘，正是要消除的错帧。

改动点：

| 编号 | 改动 | 依据 |
|---|---|---|
| F1 | 删除 `bootstrap()` 末尾单独的 `applyLocaleChange` 调用 | `loadConf("app")` 已应用语言（N11） |
| F2 | `themeStore.init` 改为从 `settingsStore.confs.app` 读 `theme`，不再 `fetchConf("app")` | N12 |
| F3 | `themeStore.init` 拆成 `initLocal()`（读 `confs.app.theme` + 一次 `getCurrentWindow().theme()` 得到 resolved，render 前 `await`）与 `subscribe()`（两个监听，render 后）；`useTheme` 只保留订阅职责 | 现 `useTheme` 在 `useEffect` 里调 `init`，首帧后才解析主题，存在闪变 |
| F4 | 删除 render 前的 `await loadFeatures()`，改为渲染后按需触发 | N13：它无条件 import 四个特性模块，Orb 路径为「注册槽」付出全部加载成本 |
| F5 | 删除 `bootstrap` 的 `getConfGroups()` 全量循环，改按 `WINDOW_CONF_REQUIREMENTS[label]` 加载；核心三组的槽为静态声明，`getConfSlot` 未注册时 `loadConf` 走「无此组」分支而非抛错 | J10；`if (!getConfSlot(group)) return;` 与三组静态槽是既有行为（`settingsStore.ts`、`builtinRegistry.ts`），本项只需删掉循环并用测试固定该分支 |

取舍：`initLocal()` 保留一次 `theme()` IPC，换取首帧无闪变；`fetchConf("app")` 与两个 `listen` 全部移出首帧。若不接受这一次 IPC，就只能用「首帧后设主题」，即接受一次闪变——两者不可同时消除。

首帧依赖按窗口判定：Orb 需要 `app`+`orb`+`mascot`（吉祥物与视觉在首帧就要正确，否则是错帧后再重绘），主窗口只需 `app`。因此不变式写「首帧只依赖本窗口的 requirements」，而不是笼统的「只依赖 app」。

### 6.2 生成物：窗口配置需求

```rust
// src-tauri/crates/assistant-config/src/window_spec.rs —— 纯数据，无 tauri 依赖
pub struct WindowSpec {
    pub label: &'static str,
    pub url: &'static str,
    pub title: &'static str,
    pub width: f64,
    pub height: f64,
    pub min_width: Option<f64>,
    pub min_height: Option<f64>,
    pub center: bool,
    pub resizable: bool,
    pub decorations: bool,
    pub transparent: bool,
    pub always_on_top: bool,
    pub skip_taskbar: bool,
    pub visible: bool,
    pub shadow: bool,
    pub maximizable: bool,
    pub minimizable: bool,
    pub accept_first_mouse: bool,
    pub zoom_hotkeys_enabled: bool,
    pub focus: bool,
    pub background_color: Option<&'static str>,   // None = 沿用 Tauri 默认
    pub confs: &'static [&'static str],            // 本窗口首帧所需配置组
}

pub static WINDOW_SPECS: &[WindowSpec] = &[ /* main, orb */ ];
```

用 `&'static [WindowSpec]` 而不是 `HashMap`：表序即生成顺序，`git diff` 稳定（G4 依赖 `--exit-code`）；`HashMap` 的迭代序不确定，生成物会随机抖动。按 label 查找走 `spec(label)` 线性查找（2 项，不值得建索引）。字段集合按 `tauri.conf.json` 现有的 orb/main 项逐项对齐（R3），迁移期不允许「先少几个字段，以后再补」——少一个字段就是静默丢一个行为。

`contract:gen` 从 `WINDOW_SPECS` 生成：

```ts
// src/settings/core/runtime/windowRequirements.generated.ts（勿手改）
export const WINDOW_LABELS = ["main", "orb"] as const;
export type WindowLabel = (typeof WINDOW_LABELS)[number];
export const WINDOW_CONF_REQUIREMENTS: Record<WindowLabel, readonly ConfGroup[]> = {
  main: ["app"],
  orb: ["app", "orb", "mascot"],
};
```

- 表里未列出的组：不加载（懒到对应页面自行 `loadConf`）。
- `OrbWindow` 现有的 `loadConf("orb") / loadConf("mascot")` 副作用改由该表统一驱动，组件内删除重复调用。
- 新增窗口必须在 `WINDOW_SPECS` 登记（生成物自动跟随，前端 `Record<WindowLabel, …>` 漏键即编译失败）。
- 生成路径沿用既有 `contract:gen`：`assistant-config` 已有 `shortcut_defaults_bindings` 测试，在此增 `window_requirements_bindings.rs` 输出该文件（生成器不能放 `assistant-protocol`——它只依赖 `serde`/`ts-rs`，读不到 `WINDOW_SPECS`）。新增 `contract:gen:windows` 脚本并接入 `contract:gen` 链；`contract:verify` 的文件清单同步增补。该脚本用 `git ls-files --error-unmatch` + `git diff --exit-code`，因此新生成物必须先 `git add` 才可能通过。

### 6.3 视图按 label 动态 import

`App.tsx` 改为：

```ts
const viewMap = {
  main: () => import("@/layouts/MainLayout").then((m) => ({ default: m.MainLayout })),
  orb: () => import("@/layouts/OrbWindow").then((m) => ({ default: m.OrbWindow })),
};
```

否则 Orb 窗口仍会把 `MainLayout` 整包拉进首帧（N13），前端减载是假的。视图加载失败按现有 `AppErrorBoundary` 处理，不新增兜底机制。

### 6.4 首帧回传（同域时钟，零 IPC 往返）

`main.rs` 生成 `startup_id` 后，通过 webview 的 `initialization_script` 注入：

```js
window.__STARTUP_ID__ = "<uuid>";   // 与 t0 同时刻生成
```

渲染后：

```ts
requestAnimationFrame(() => {
  const frameEpochMs = Math.round(performance.timeOrigin + performance.now());
  void tauriInvoke("report_first_frame", { startupId: window.__STARTUP_ID__, frameEpochMs });
});
```

`performance.timeOrigin + performance.now()` 是 **epoch 毫秒**，与 Rust `SystemTime` 同一时钟域，Rust 侧直接与进程起点相减即得 T_visible，无需「两段相加」的估计。`startup_id` 不再需要经命令下发，首帧关键路径上少一次 IPC 往返。

主窗口懒建时（P2 之后）由 `show_or_create` 注入 `wake-<uuid>` 前缀的新 id，T_wake 用同一机制测量。

---

## 7 度量与预算

### 7.1 结构化日志

`main.rs` 首行 `let t0 = Instant::now()`，`manage(BootClock)`。每次阶段结束输出一行：

```text
startup stage=<id> lane=<boot|idle> startup_id=<uuid> ms=<n> budget_ms=<n> outcome=<ok|degraded|fatal|ignored>
```

`budget_ms > 0` 且 `ms > budget_ms` 时额外 warn 一行（`startup budget exceeded`）。`budget_ms = 0` 表示未测量，不参与比较——否则每个未测项在每次启动时都会刷一行假告警。

### 7.2 预算来源

`budget_ms` **不预设**：P0 在 release 包上实测，取「中位数 ×2」回填，并把该值写入表。`0` 表示未测量：G1 放行，G8 跳过。预算表在实施期回填，字段格式固定如下（示例行为 P0 产出后填写）：

| 表项 | budget_ms | 来源 |
|---|---|---|
| `dirs` | 0 | P0 |
| `boot_conf` | 0 | P0 |
| `storage` | 0 | P0 |
| `orb_shell` | 0 | P0 |
| `tray` | 0 | P0 |
| `shortcuts` | 0 | P0 |

### 7.3 度量口径

| 编号 | 口径 |
|---|---|
| M1 | 只在 **release** 包取数：dev 为 Vite dev server + devtools + stdout 日志，数字失真 |
| M2 | 取数环境固定：冷启动（进程不在内存）+ 固定屏幕缩放 + 固定库规模，规模写入日志头 |
| M3 | 每组至少 10 次，报 p50 / p95 |
| M4 | T_visible = Orb 首帧 epoch 毫秒 − 进程起点 epoch 毫秒；两端同域，不做分段相加 |
| M5 | T_wake 单列：`show_or_create_main` 入口打点生成 `wake-<uuid>`，主窗口回传同域首帧 |

### 7.4 预算基线文件格式

G8 需要一份机器可读的基线，格式固定为 JSON（不是 md 表格——门禁要解析，md 表格解析脆弱）：

```json
// docs/analysis/startup-budget-baseline.json
{
  "date": "YYYY-MM-DD",
  "profile": "release",
  "scale": { "screen_scaling": 1.0, "db_rows": 0 },
  "runs": 10,
  "phases": { "dirs": { "p50": 0, "p95": 0 }, "...": {} },
  "totals": { "t_process": { "p50": 0, "p95": 0 },
              "t_visible": { "p50": 0, "p95": 0 },
              "t_wake":    { "p50": 0, "p95": 0 } }
}
```

- `phases` 的键必须与 `RESOURCES` 的 id 一一对应；多出的键 G8 报错（防止改名后留下死数据）。
- 文件缺失即 G8 失败（不静默跳过）；`budget_ms == 0` 的项从比较中剔除，与 §7.2 一致。
- 每次重新取数覆盖该文件，`date` 变化即历史对比点；不做多份历史文件（git 历史就是历史）。

---

## 8 门禁与测试

Rust 侧沿用 `src-tauri/tests/assembly_contract.rs` 的「读源码 + 断言结构」范式（不引入新框架）。新增 `src-tauri/tests/startup_contract.rs`。

| 编号 | 门禁 | 断言方式 | 失败含义 |
|---|---|---|---|
| G1 | 资源表自洽 | (a) 遍历 `RESOURCES`：id 唯一；`needs ⊆ 表` 且指向声明在前；四份集合（`PRIMARY_ENTRY`/`BOOT_REQUIRED`/`OBSERVABLE`/`BEST_EFFORT`）互斥且覆盖全表；`ACTIONABLE ⊆ OBSERVABLE`；`PRIMARY_ENTRY ⊆ Boot`；`BOOT_REQUIRED ⊆ Boot`；`lane Idle` 无 Fatal。(b) `needs` 闭包用源码扫描补足：`boot/build/*.rs` 中出现 `ctx.boot()` 的文件，其对应表项（文件↔id 映射按 §9）`needs` 必含 `boot_conf`——`build` 是函数指针，运行期不可内省 | 有人删/错序/错泳道/错档；闭包漏声明 |
| G2 | Builder 段顺序 | 读 `src-tauri/src/plugins.rs` 源码，断言 `single_instance` 出现位置早于 `log_plugin()` | 注释约束被破坏（N3） |
| G3 | 启动路径无第二来源 | 断言 `boot/` 下除 `conf.rs` 外不出现 `AppConf::load()` / `OrbConf::load()`；`AppDirs::resolve()` 在**启动路径白名单**（`main.rs`、`plugins.rs`、`window.rs`、`tool_setup.rs`、`vault_plugin.rs`、`clock_plugin.rs`、`boot/**`）内只允许出现在 `main.rs`。`cmd/**`、`adapters/**` 的运行期解析不在断言范围（今日合法且应保留）；`assistant-config::conf_dir_root()` 在另一 crate，由 §4.5「配置目录」一行 + `BootConf::load_all(&dirs)` 守住 | 单点被绕过（N4/N5/N15） |
| G4 | 窗口单一来源 + 生成物新鲜 | 解析 `tauri.conf.json`：`app.windows` 为空数组；`contract:verify` 对生成物 `git diff --exit-code` | 双来源复发（N6）/ 生成物过期 |
| G5 | 能力门禁 | 断言 `cmd/`、`adapters/` 源码不出现对 `boot/access.rs` 已登记类型的 `state::<Arc<T>>()` / `try_state::<Arc<T>>()`，且不出现 `boot::registry::` 内部符号；类型清单由 `boot/access.rs` 访问器的返回类型**源码扫描派生**，不手工维护 | 绕过 `ensure`（N10） |
| G6 | 前端槽自洽 | 核心三组的静态槽齐全；`WINDOW_CONF_REQUIREMENTS` 的每个组都能被 `getConfSlot` 解析（特性组按「页面注册」豁免）；`getConfSlot` 未注册时 `loadConf` 返回而非抛错 | 死组 / 漏组 |
| G7 | 前端视图懒加载 | 断言 `src/App.tsx` 不含对 `MainLayout` / `OrbWindow` 的静态 `import` | 减载回退（N13） |
| G8 | 预算回归 | 只对 `budget_ms > 0` 的表项断言 p95 ≤ `budget_ms`；数据源固定为 `docs/analysis/startup-budget-baseline.json`（格式见 §7.4），文件缺失即失败 | 静默劣化（N14） |

G8 的取数文件入库（便于门禁读取与历史对比），格式与判定口径见 §7.4。

### 8.1 既有测试的连带修改

`src-tauri/tests/assembly_contract.rs` 中与启动目录相关的常量必须同步，否则 P1 一落地就红：

| 常量 / 测试 | 现值 | 影响 |
|---|---|---|
| `CALLER_FILES` | 含 `"setup.rs"` | 目录化后需改为 `boot/mod.rs` 等实际路径 |
| `LIB_RS_LINE_LIMIT` | 40 | 新增 `mod boot` 声明会增加 `lib.rs` 行数，需按实际调整 |
| `PLUGIN_FILE_SUFFIX` / 插件发现 | 扫 `src` 根 | 新增 `boot/` 子目录不应被误判为插件文件 |
| `window_labels()`（`b10_feature_capabilities_are_minimal`） | 从 `tauri.conf.json` 的 `app.windows` 读 label 并断言非空 | `windows` 置空后该断言必红；须改读 `assistant_config::window_spec::WINDOW_SPECS`，**与 P2 同批**，否则 P2 一落地就红 |

这些修改属于机械跟随，但**必须与对应阶段同一批提交**（前三条随 P1，第四条随 P2）。

---

## 9 接入矩阵（现状 1:1 映射）

首版表复刻现状顺序，唯一例外是 `dirs` / `boot_conf` / `feature_on_setup` 三者的相对次序：特性 `on_setup` 今日自行解析 `AppDirs`（N15），若把它留在第 1 位就必须保留第二个目录解析点，与 §4.5 冲突。因此把它挪到 `dirs`、`boot_conf` 之后，并让 `SetupHook` 改收 `&StartupCtx`。`severity` 列写派生结果（表内不存该字段）；「变化」列只写机制性变化，不写优化。

| # | 表项 id | `lane` | `needs` | severity | 现状步骤 | 变化 |
|---|---|---|---|---|---|---|
| 1 | `dirs` | Boot | `[]` | Fatal | `AppDirs::resolve()` | 唯一解析点；结果由 `run(dirs)` 下传并注入槽；`tool_setup` / `plugins` / `window` / 特性 `on_setup` 内不再解析 |
| 2 | `boot_conf` | Boot | `[dirs]` | Degraded | `AppConf::load()` + `OrbConf::load()`（均忽略错误） | 合并为一次 `BootConf` 快照，经 `load_from(dirs)` 显式路径；失败回退默认值 + 记降级（S6） |
| 3 | `i18n` | Boot | `[boot_conf]` | Fatal | `init_global` | 语言取自 `BootConf`，不再二次 `load` |
| 4 | `feature_on_setup` | Boot | `[dirs, boot_conf]` | Degraded | 特性 `on_setup` 循环（vault/clock） | 按 `feature_mounts` 遍历；`SetupHook` 签名改收 `&StartupCtx`，dirs 从 ctx 取 |
| 5 | `storage` | Boot | `[dirs, boot_conf]` | Fatal | DB 打开 + 审计裁剪 + `manage(DbState)` | 审计裁剪移出，改由 `storage_maintenance` |
| 6 | `attachment_store` | Boot | `[dirs]` | Degraded | 附件目录 + `AttachmentStore` + `clean_tmp` | `clean_tmp` 改由 `storage_maintenance` |
| 7 | `runtime_state` | Boot | `[storage]` | Fatal | 5 个状态槽 `manage` | 不变 |
| 8 | `tool_runtime` | Boot（P1）→ Idle（P3） | `[dirs, boot_conf]` | Fatal（P1）→ Degraded（P3） | `tool_setup::init` | 装配函数化，由 `ensure` 调用；P3 与降档（lane + 集合 + `ACTIONABLE`）同批改 |
| 9 | `tray` | Boot | `[boot_conf, i18n]` | Degraded | `tray::create_tray` | 不变 |
| 10 | `orb_shell` | Boot | `[dirs, boot_conf]` | Fatal | `position_orb_window` + `apply_orb_window_attrs` + `show_orb_window` + E14 漂移守卫 | 合并为一项；属性在构建期一次给出；T_visible 终点 |
| 11 | `screen_inspection` | Boot | `[]` | Ignore | `spawn_screen_inspection` | 不变（线程自身循环） |
| 12 | `shortcuts` | Boot | `[boot_conf]` | Ignore | `register_global_shortcuts` | 收 `&BootConf` |
| 13 | `window_main` | Boot（P1）→ Idle（P2） | `[boot_conf]` | Degraded | `restore_main_window`（P1 保留原路径，P2 并入） | 几何挂建窗路径；`visible(false)`；托盘/快捷键/单实例经 `ensure` 触发 |
| 14 | `storage_maintenance` | Idle | `[storage, attachment_store]` | Degraded | 步骤 5/6 中被移出的清理 | 审计裁剪 + 工具结果清理 + `clean_tmp` |
| 15 | `mcp_start` | Idle | `[tool_runtime]` | Ignore | `tool_setup::init` 内的 mcp 拉起 | 单 server 失败只记日志（现状语义） |

进程级（不进表，Builder 段之前，`main.rs`）：`crash::install`、`ensure_webview_profile`、`run_pending_purge`。其结果注入 `dirs` 表项；清理改为限量（见 §14 R2）。插件注册在 Builder 段，顺序由 G2 守。

`window_main` 的 `lane` 是 P2 的唯一性能赌注，也是**回滚点**：若 P2 实测 T_wake 劣化，把该行 `Idle` 改回 `Boot`（一行 diff），无需恢复 `tauri.conf.json` 的窗口声明。

---

## 10 改动面

### 10.1 新增

| 路径 | 内容 |
|---|---|
| `src-tauri/src/boot/`（16 个文件，见 §5） | 资源表、registry、runner、ctx、conf、access、各 build 函数 |
| `src-tauri/src/cmd/startup.rs` | `report_first_frame`、`get_startup_report` |
| `src-tauri/tests/startup_contract.rs` | G1–G8（按 §12.4 分阶段启用） |
| `src-tauri/crates/assistant-protocol/src/startup.rs` | `StartupReportDto`、`CapIssueDto`、`CapStateDto` |
| `src-tauri/crates/assistant-config/src/window_spec.rs` | `WindowSpec` + `WINDOW_SPECS`（纯数据，无 tauri 依赖，`&'static [WindowSpec]`） |
| `src-tauri/crates/assistant-config/tests/window_requirements_bindings.rs` | 生成 `windowRequirements.generated.ts`（与既有 `shortcut_defaults_bindings` 同形） |
| `src/settings/core/runtime/windowRequirements.generated.ts` | 生成物，勿手改 |
| `src/stores/system/startupStore.ts` | 能力状态与降级提示（订阅 `startup:capability-changed`） |
| `src/types/protocol.contract.ts`（生成） | 由 `npm run contract:gen` 产出，勿手改 |
| 前端测试 | G6、G7 对应断言（放 `src/settings/core/runtime/__tests__/`、`src/__tests__/app-view.test.tsx`） |

### 10.2 修改

| 路径 | 改动 |
|---|---|
| `src-tauri/crates/assistant-config/src/lib.rs` | 增 `pub mod window_spec;` |
| `src-tauri/src/main.rs` | 起始计时、`startup_id` 生成、`AppDirs::resolve()` 一次后 `assistant_app::run(dirs)` 下传；清理标记限量 |
| `src-tauri/src/lib.rs` | `pub fn run(dirs: AppDirs)`；`plugins::attach(builder, &dirs)`；`.setup(move \|app\| boot::run_boot(app, &dirs))`；删 `mod setup` |
| `src-tauri/src/plugins.rs` | 顺序注释改为引用 G2（注释保留，断言新增）；`log_plugin(&dirs)` 不再自行 `AppDirs::resolve()` |
| `src-tauri/src/window.rs` | `WINDOW_SPECS` 迁出（改为从 `assistant_config::window_spec` 读取）；`show_or_create(app, label, &dirs)` 收 dirs、应用几何 + 注入 `initialization_script`；`restore_main_window` 删除 |
| `src-tauri/tauri.conf.json` | `app.windows` 置空数组 |
| `src-tauri/src/tool_setup.rs` | 装配函数化（不直接 `manage`，交由 registry）；删除内部 `AppDirs::resolve()` |
| `src-tauri/src/feature_mounts.rs` | `SetupHook` 改为 `fn(&StartupCtx) -> Result<(), String>` |
| `src-tauri/src/vault_plugin.rs`、`clock_plugin.rs` | `on_setup` 改收 `&StartupCtx`，dirs 从 `ctx.dirs()` 取 |
| `src-tauri/src/cmd/agent/mod.rs`、`cmd/mcp.rs`、`cmd/storage/mod.rs`、`adapters/runner.rs` | 改为 `access::*` 取用（直取 5 处 + `try_state` 2 处统一走同一入口） |
| `src-tauri/src/ipc.rs` | `generate_handler!` 增 `report_first_frame`、`get_startup_report`（N16） |
| `src-tauri/crates/assistant-protocol/src/lib.rs` | 增 `pub mod startup;`（N16） |
| `src-tauri/tests/assembly_contract.rs` | `CALLER_FILES` 等常量跟随（§8.1）；`window_labels()` 改读 `WINDOW_SPECS`（P2） |
| `package.json` | 新增 `contract:gen:windows` 并接入 `contract:gen`；`contract:verify` 文件清单增生成物 |
| `src/main.tsx` | 最小依赖 bootstrap（按 `WINDOW_CONF_REQUIREMENTS[label]`）、其余组延后、首帧回传 |
| `src/stores/system/themeStore.ts` | 拆 `initLocal()` / `subscribe()`；去重 `fetchConf`，从 `settingsStore` 读 |
| `src/stores/settingsStore.ts` | 删除 `getConfGroups()` 全量循环；保留并测试「槽未注册」分支 |
| `src/App.tsx` | 视图动态 `import` |
| `src/layouts/OrbWindow.tsx` | 删除自带 `loadConf`，由需求表驱动 |

### 10.3 删除

| 路径 | 原因 |
|---|---|
| `src-tauri/src/setup.rs` | 目录化为 `boot/` |
| `window::restore_main_window` | 几何恢复并入建窗路径 |
| 原设计的 `caps/` 目录与 `Capability` 枚举 | 能力门禁并入 `boot/registry.rs`，id 即能力名 |

---

## 11 不变式

| 编号 | 不变式 |
|---|---|
| I1 | 启动期配置只有一个读盘点（`BootConf`），运行期读路径不变 |
| I2 | 运行期「改设置立即生效」语义不变：审批超时、几何落盘仍按需现读 |
| I3 | 任何表项不得在未声明 `needs` 的情况下消费前序产物 |
| I4 | `Degraded` 表项失败不得导致其他表项失败（隔离）：依赖项失败只让本项 `NotReady` 并记一次降级，**不升级为 Fatal**（S7）；被 `BOOT_REQUIRED` 依赖的 Degraded 项不得返回 `Err`（S6） |
| I5 | 主窗口几何在 `show()` 之前套用，用户看不到「先默认位置再跳」 |
| I6 | `ensure` 幂等：已 Ready 直接返回，已 Pending 共享同一次装配，已 Failed 可重试；不重建窗口、不抢焦点 |
| I7 | 能力未就绪时命令层返回可判定错误（`NotReady`），不 panic |
| I8 | 前端首帧只依赖本窗口的 `WINDOW_CONF_REQUIREMENTS[label]`；其余组不得阻塞 `render` |
| I9 | 所有启动期清理项都必须在「安全点」执行；`purge` 唯一安全点仍是 `Builder::run` 之前 |
| I10 | 生成物（`windowRequirements.generated.ts`、`protocol.contract.ts`）不得手改；改动源头在 Rust 侧并重跑 `contract:gen` |
| I11 | `lane` 与集合成员是同一份判定的两个投影：改档必须在同一提交内同时改两者（`window_main` 随 P2、`tool_runtime` 随 P3，各一例） |
| I12 | `WINDOW_SPECS` 的容器必须有序（`&'static [WindowSpec]`），保证生成物与 `git diff` 稳定 |

---

## 12 验收

### 12.1 结构验收

| 编号 | 验收项 | 判定 |
|---|---|---|
| A1 | `RESOURCES` 表可读：新增一个资源只需加一行 + 归入一份集合 | 改一处 + 通过 G1 |
| A2 | `boot/mod.rs` ≤ 60 行且无业务分支 | 行数检查 |
| A3 | 当前阶段对应的门禁子集全绿（见 §12.4） | `cargo test --test startup_contract`、`npm test` |
| A4 | `tauri.conf.json` 无窗口声明；两窗口均由 `show_or_create` 创建 | G4 |
| A5 | 启动路径无第二配置读盘点、无第二目录解析点 | G3 |
| A6 | 命令层无未就绪直取 | G5 |

### 12.2 行为验收

| 编号 | 验收项 |
|---|---|
| A7 | Orb 启动位置正确、无跳动；托盘与快捷键可用 |
| A8 | 首次唤起主窗口：几何正确、无闪烁、无空白帧（idle pass 已建好窗口） |
| A9 | 工具结果目录 I/O 失败时应用仍启动，发送消息处显示「工具暂不可用」+ 重试可用（沙箱探测失败今日已降级，不在验收范围，见 N9） |
| A10 | 设置页改语言/主题后，主窗口与 Orb 均立即生效（I2 未破） |
| A11 | 关闭主窗口后重开：几何与上次一致（几何落盘路径未破） |
| A12 | 单实例：第二个实例唤起主窗口成功（`ensure(window_main)` 懒建路径可用） |

### 12.3 度量验收

| 编号 | 验收项 |
|---|---|
| A13 | release 包能输出完整 `startup stage=…` 链路 + 前端首帧回传，同一 `startup_id` |
| A14 | 三个阶段指标均有 p50/p95 记录；T_visible 与 T_wake 分列 |
| A15 | 相对 P0 基线：T_visible 下降有数据支撑；T_wake 不劣化（允许 ±5%） |

### 12.4 阶段门禁子集

G1–G8 不是单点门禁，按阶段生效，避免「P1 就要 G8 数据」这种不可能要求：

| 阶段 | 必须绿 | 尚未生效 |
|---|---|---|
| P0 | G2 | G1、G3–G8 |
| P1 | G1、G3、G7 | G4、G5、G6、G8 |
| P2 | G4、G6 | G5、G8 |
| P3 | G5、G8 | — |

G6 依赖 `WINDOW_CONF_REQUIREMENTS` 生成物，因此不能早于 P2；P1 只做 F1–F5 中不依赖生成物的部分（`bootstrap` 去掉全量循环、主题去重），G7 可独立成立。

---

## 13 明确不做

| 编号 | 不做 | 理由 |
|---|---|---|
| X1 | 带失效机制的全局配置缓存 | 与 I2 冲突；失效点虽收敛在 `cmd/config.rs`，但收益属 P3 之后再评估 |
| X2 | 通用 DI 容器 / 插件框架 | 一张 `&'static` 表 + 一个 `ensure` 足够；抽象层级越高越难维护 |
| X3 | 把 DB 打开移出 `Boot` | 会引入全局命令门禁，复杂度高、收益未测 |
| X4 | 用 trait 对象 / 动态注册实现资源表 | 破坏可静态检查与可 diff 审查 |
| X5 | 定时器式预热（`sleep` 后装配） | 竞态源（§3.4） |
| X6 | 独立的预热调度器（例如「Orb 无交互时才预热」的启发式） | `Idle` 泳道 + `ensure` 已覆盖；启发式无法验收，且引入新的不确定性（原设计因缺少统一机制才需要它） |
| X7 | `after` 字段 | 在「顺序 = 表序」下恒真，是无信息量的第二份顺序副本，由 `needs` 取代 |
| X8 | 把 `loadFeatures` 拆成按窗口的模块清单 | 核心三组已静态化（J10）；特性组涉及跨窗口槽注册与特性间依赖，整体按页面触发即可 |

---

## 14 风险

| 编号 | 风险 | 缓解 |
|---|---|---|
| R1 | `tool_runtime` 降档后，用户遇到「能启动但工具不可用」却不知情 | 与 P3 前端降级提示同批交付；A9 验收；禁止只改一列（`lane` 与集合成员必须同批，见 §4.3） |
| R2 | `purge_cache` 改限量后，磁盘可能长期不回落 | 保留「整目录清理」入口（设置页手动触发）；标记文件记录剩余子目录，逐次推进 |
| R3 | Orb 移入 `WINDOW_SPECS` 时漏字段 | 迁移前把 `tauri.conf.json` 的 orb 项逐字段列成对照表，至少覆盖 `url`/`title`/`width`/`height`/`resizable`/`decorations`/`transparent`/`alwaysOnTop`/`skipTaskbar`/`visible`/`shadow`/`maximizable`/`minimizable`/`acceptFirstMouse`/`zoomHotkeysEnabled`/`focus`/`backgroundColor`；程序化建窗的默认值与 conf 不同（`visible` 默认 true，漏填即启动弹窗）；A7 覆盖视觉与点击穿透 |
| R4 | `Idle` pass 与用户首次点击并发时，`ensure` 的装配在 async runtime 上占线程，可能拖慢 Orb 交互 | 装配内的文件 I/O 走 `spawn_blocking`；`run_idle` 按表序逐项 `ensure` 并在项间让出（`.await` 天然让出）；A15 同时观察 T_wake 与 CPU 尖峰 |
| R5 | 目录化触发 `assembly_contract` 大面积变红，被误判为回归 | §8.1 常量跟随与目录化同批提交；先单独跑该测试确认基线 |
| R6 | `ensure` 迁移期出现「已移除 `manage`、命令层未改」的中间态 | 提交粒度约束：C3 的调用点替换与 `manage` 移除必须同一提交 |
| R7 | 生成物被手改或忘记重跑 `contract:gen` | G4 的 `contract:verify` 步骤对生成物执行 `git diff --exit-code`；I10 明令禁止手改；新生成物须先 `git add`（`verify` 用 `git ls-files --error-unmatch`） |
| R8 | `feature_on_setup` 挪到 `dirs` / `boot_conf` 之后，改变了特性 `on_setup` 的执行时点（今日在配置读盘前） | 逐个复核 `on_setup` 体，只允许读 `SectionRegistry` 与 `ctx`；`clock` 的首次调度若依赖「配置尚未加载」的时序，改为在 `on_setup` 内自行延迟首次 tick |
| R9 | G3 的启动路径白名单会随文件增长而漂移（新增启动期文件时漏登记） | 白名单作为常量集中在 `startup_contract.rs` 顶部，与 §8 G3 同源；新增启动期文件必须同时改该常量，否则断言直接失败（默认拒绝） |
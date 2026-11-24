# 应用启动链路：必要性与耗时来源分析

> 日期：2026-11-23
> 性质：**预先分析**（只核对「启动时做了什么、哪些不可省、哪些只是被放到了关键路径上」；**不含取舍决议**——采用哪套改法、何时落地，属规格职责）
> 判据来源：`docs/principle/CODE-REVIEW-PRINCIPLE.md`、`docs/principle/CODE-SPLIT-PRINCIPLE.md`
> 事实快照：截至 2026-11-23 的仓库代码（**时点记录**，不随代码演进追改，需要最新事实时以代码为准）
> 引用已归档规格时只写「文档名 +（已归档）」，不给路径。
> 时效与终止：本文是**一次性决策输入**。启动优化若立规格，有效内容由规格承接；事实条目作为历史快照保留。

---

## 0 命题核对

| # | 命题 | 判定 | 依据 |
|---|---|---|---|
| 1 | 启动时做的每件事都是「必须」的 | **不成立** | 硬必须只有一条短链；审计裁剪、结果清理、MCP 拉起、主窗口建 WebView 都能移出关键路径（§2、§3） |
| 2 | 主窗口必须启动即创建 WebView | **不成立** | `tauri.conf.json` 里两个窗口都是 `visible: false`，主窗口启动期对用户不可见；代码已有动态创建入口 `window::show_or_create`（§3.3） |
| 3 | Orb 窗口必须加载全部特性与全部配置组 | **不成立** | 前端 `bootstrap()` 对每个 webview 都跑同一份逻辑：`loadFeatures()` + 遍历全部配置组；Orb 只需 `orb` / `mascot`（§3.4） |
| 4 | 配置读取已最小化 | **不成立** | `AppConf` 在 Rust 侧启动期读 5 次，前端每个窗口再各取一次 `app`；`OrbConf` 读 3~4 次（§4.1） |
| 5 | 启动耗时已被度量 | **不成立** | `src-tauri/src` 下启动路径无 `Instant` 计时点，只有一行 `Assistant started` 日志（§4.3） |
| 6 | 启动期清理必须在启动期同步完成 | **部分成立** | WebView2 profile 清理成立（唯一安全时点在 `Builder::run` 之前）；审计裁剪与工具结果清理不成立（§3.2） |
| 7 | 工具基础设施装配必须与 UI 启动共用一条失败路径 | **不成立（现状如此）** | `setup::init` 用 `?` 传播 `tool_setup::init` 的错误，工具侧任何失败都会终止整个启动；而特性 `on_setup` 失败只记日志（§4.2） |
| 8 | 前端首帧必须等待全部配置加载完 | **不成立** | 首帧真正依赖的只有 `app.language`（决定 i18n 语言）；`orb` / `mascot` / `vault` 可延后（§3.4） |
| 9 | 运行期状态槽的 `manage` 是零成本 | **成立** | `TurnTasks` / `SessionStates` / `ScreenshotState` / `InventoryCache` / `ReclaimPlanStore` 均为 `Arc::new`，不含 I/O（`setup.rs:112-118`） |
| 10 | 单实例假设成立且顺序约束已被固定 | **成立** | `single_instance` 排在日志插件之前，注释已写明原因（`plugins.rs:17-22`） |

核对结果：启动链路的形状是**「一条真正的硬必须短链 + 一串被顺手放在 setup 里的工作」**。可省的部分不是"多余功能"，而是**执行时机**——它们大多只需在首次使用前完成，却都被放在了首帧之前。

---

## 1 启动链路的四段结构

| 段 | 入口 | 时机 | 是否阻塞首帧 |
|---|---|---|---|
| 0 进程级 | `src-tauri/src/main.rs` | `Builder::run` 之前 | 是（同步） |
| 1 Builder 阶段 | `src-tauri/src/lib.rs`、`plugins.rs` | `build()` 内，窗口创建 + 插件 setup | 是 |
| 2 App setup | `src-tauri/src/setup.rs` | 窗口创建之后 | 是（同步为主） |
| 3 前端 bootstrap | `src/main.tsx` | 每个 webview 各跑一次 | 是（首帧被挡住） |

### 1.1 第 0 段：进程级（`main.rs`）

| 动作 | 位置 | 说明 |
|---|---|---|
| 解析受管目录 `AppDirs` | `src-tauri/src/app_dirs/roots.rs:42` | 失败 `exit(1)`，**无任何回退**（不落 cwd、不落相对路径） |
| 安装崩溃处理器 | `main.rs:14` | 崩溃目录与日志同根（local 根） |
| 保证 WebView2 profile 目录存在 | `roots.rs:114` | 设计约束：必须在任何 webview 打开 profile 之前 |
| 消费「待清理」标记并同步删缓存 | `app_dirs/webview_profile.rs:77` | 唯一安全执行点；**无标记时零副作用** |
| Windows GPU 参数 | `main.rs:32-38` | 与功能无关的渲染调优 |

### 1.2 第 1 段：Builder 阶段

- `manage(SectionRegistry)` 早于所有插件：插件 `setup` 早于 app `setup`，特性分区注册时槽必须已存在（`plugins.rs:8-12`）。
- 插件顺序有硬约束：`single_instance` 必须排在日志插件之前，否则第二实例会先把正在写的日志轮转掉（`plugins.rs:17-22`）。
- 挂载顺序：opener / dialog / notification / single-instance / log / global-shortcut / autostart，再按特性挂 vault、clock（`feature_mounts.rs:14-27`）。
- 日志档位由 `debug_assertions` 决定、**不读配置**；文件落 `local/logs`，`KeepSome(3)` + 10 MiB（`plugins.rs:38-77`）。
- `tauri.conf.json` 声明 `main`（960×640，`visible:false`）与 `orb`（88×88，透明、置顶、跳任务栏、`visible:false`）。**两个 webview 在启动期都会创建并加载页面**，只是主窗口不显示。

### 1.3 第 2 段：`setup::init`（`src-tauri/src/setup.rs:48`）

| 序 | 动作 | 位置 |
|---|---|---|
| 1 | 特性 `on_setup`（vault 读配置 + 注册存储分区；clock 建运行时） | `setup.rs:51-55` |
| 2 | `AppConf::load()` / `OrbConf::load()`（仅校验） | `setup.rs:56`、`59` |
| 3 | 再读一次 `AppConf` 取语言 → `assistant_i18n::init_global` | `setup.rs:63-68` |
| 4 | `AppDirs::resolve()` | `setup.rs:70` |
| 5 | 打开 DB：建表 + 列迁移 + 内置模式种子；随后按保留期裁剪 `permission_audit` | `setup.rs:73-93` |
| 6 | 建 `attachments/` 目录 + `AttachmentStore`；`clean_tmp` 丢异步 | `setup.rs:96-108` |
| 7 | `manage` 五个运行期状态槽 | `setup.rs:112-118` |
| 8 | 工具基础设施装配（含 MCP 进程拉起） | `setup.rs:120` → `tool_setup.rs:221` |
| 9 | 创建托盘 | `setup.rs:122` |
| 10 | Orb 定位 → 主窗口几何恢复 → Orb 属性 → 显示 Orb | `setup.rs:124-129` |
| 11 | WebView2 profile 漂移守卫（仅日志） | `setup.rs:131-140` |
| 12 | 启动屏幕巡检线程（800 ms） | `setup.rs:142` |
| 13 | 注册全局快捷键 | `setup.rs:144` |

DB 初始化内部（`crates/assistant-storage/src/db.rs:35`）：建父目录 → 打开连接 → `execute_batch` 建表 → `seed_builtin_modes`（`db.rs:326`）→ `schema_version` 判定 → 存量库逐列探测补列（`provider_id` / `model_id` / `status` / `tool_call_id` / `is_error` / `mode_id` / `turn_index` / `iteration` / `next_turn_index`）→ 建索引 → `run_migrations` → `permission_audit` 补归属列。

工具装配内部（`tool_setup.rs:221`）：沙箱探测（**注册期一次**，结论缓存）→ 注册表分组注册（`shell` 受探测结论门禁，`tool_setup.rs:297-298`）→ 授权/审计存储 → 审批装饰器 → 网关 → 结果落盘 + **清理线程**（`tool_setup.rs:290`）→ 流水线 → 执行器（含工作区解析与升级通道）→ MCP 管理器 `manage` + **拉起已启用 server**（`tool_setup.rs:363` → `mcp/config.rs:124`）。

### 1.4 第 3 段：前端 bootstrap（`src/main.tsx`）

```
L38 initSettingSystem()  L40 bootstrapLogger()  L41 全局错误兜底  L42 缩放守卫
L45 await loadFeatures()            // 动态 import vault/clock/observation/orchestration
L47-49 for (group of getConfGroups()) await loadConf(group)   // app / orb / mascot / vault
L51-52 getConfValue("app").language → await applyLocaleChange(lang)
L91-92 bootstrap().then(→ createRoot().render())
```

- 渲染后：`useTheme()` 触发 `themeStore.init()`，**又取一次 `app` 配置**（`src/stores/system/themeStore.ts:58`）；`subscribeSettingEvents()` 建 3 个事件监听。
- 窗口分支：Orb 窗口在 `OrbWindow.tsx:22-24` **再加载一次 `orb` / `mascot`**；主窗口走 `useAppInitialization`（收藏 + 最近工具 + 观察事件订阅，`src/hooks/useAppInitialization.ts:9-12`）。
- 历史会话、工具索引、存储清点均**不在启动期**加载（懒加载设计，正确）。

---

## 2 必要性分级

分级判据（本文自用，非项目门禁）：

- **A 硬必须**：缺失或延后会导致 ①数据写错/丢失，②错过唯一安全执行点，③用户无任何入口（应用不可用），或 ④排障链断裂。
- **B 必要但可移出关键路径**：结果正确性与执行时点无关，只需在首次使用前完成。
- **C 收益低/可省**：重复劳动，或对当前功能无贡献。

### 2.1 A 类（硬必须）

| 项 | 不可省的原因 |
|---|---|
| `AppDirs::resolve` + 失败即退出 | 所有受管路径的唯一来源；回退会静默写错位置 |
| single-instance（且排在日志前） | 防双实例写同一 DB / 同一日志文件 |
| WebView2 profile 建目录 + 消费清理标记 | **唯一安全时点**在 webview 打开 profile 之前，之后无法保证 |
| 崩溃处理器安装 + 日志插件 | 成本极低，是排障前提；日志配置错误会让排障链本身失效 |
| DB 打开 + 建表/迁移 + 内置模式种子 | 核心数据可用性；失败必须终止（fail-closed） |
| `AppConf` + i18n 初始化（且在托盘之前） | 托盘菜单文案、快捷键解析、主题、语言全部依赖 |
| 托盘创建 | 应用是托盘常驻型，主窗口默认隐藏；没有托盘就没有入口 |
| Orb 定位 + 置顶属性 + 显示 | 启动期唯一可见 UI |
| 全局快捷键注册 | 核心交互入口 |
| 工具基础设施装配 | agent / 工具能力的前提，且装配顺序本身是依赖链（storage 晚于 registry，`shell` 晚于 storage） |
| 特性 `on_setup`（启用时） | vault 配置与存储分区、clock 运行时 |
| 五个状态槽 `manage` | 任何命令都可能读它们；成本为零 |
| `attachments/` 建目录 + `AttachmentStore` | 附件命令的前提；成本仅一次 `create_dir_all` |
| 窗口事件 / 菜单事件 / `invoke_handler` 注册 | `build()` 的一部分，无替代 |

### 2.2 B 类（必要但不必在首帧前）

| 项 | 现状 | 为什么可以移出 |
|---|---|---|
| `permission_audit` 裁剪 | `setup.rs:83-84` 同步 `DELETE` | 只追加表的裁剪结果与执行时刻无关；只是"越早越好" |
| 工具结果清理 | `tool_setup.rs:290` 独立线程 | 已在线程里，但可与首帧争 I/O，可再延迟 |
| 附件 `clean_tmp` | `setup.rs:104` 异步 | 同上 |
| 主窗口几何恢复 | `setup.rs:125` 对隐藏窗口执行 | 目的是避免"先跳后正"；在 `show_or_create` 时做同样能避免跳动 |
| vault 存储分区注册 | `vault_plugin.rs:370-389` | 只有打开存储设置页才需要；`VaultConf::load` 仅为校验 |
| MCP 已启用 server 拉起 | `mcp/config.rs:124` 启动即 spawn 外部进程 | 外部进程启动耗时不可控；首次会话/首次工具列表时拉起即可 |
| 屏幕巡检线程 | `setup.rs:142` 启动即 800 ms 轮询 | 只在 Orb 可见且需校正时才有意义 |
| WebView2 缓存清理 | `main.rs:19-30` 同步 | **位置不能改**（唯一安全点），但可加预算（体积阈值/条目上限） |

### 2.3 C 类（收益低）

| 项 | 证据 |
|---|---|
| 主窗口启动即建 WebView 并跑完整前端 bootstrap | `tauri.conf.json` 声明 + 隐藏；已有动态创建路径 `window.rs:135` |
| Orb 窗口加载全部特性模块 + 全部配置组 | `main.tsx:45-49`；`OrbWindow.tsx:22-24` 还重复取 `orb`/`mascot` |
| 同一配置组多次读盘 | Rust 侧 `AppConf` 5 次、`OrbConf` 3~4 次（§4.1） |
| 前端 `themeStore.init` 重复取 `app` | `themeStore.ts:58`；bootstrap 已取过 |
| WebView2 profile 漂移守卫 | `setup.rs:131-140` 只写日志 |
| Windows GPU 参数 | `main.rs:32-38` 与功能无关 |

---

## 3 耗时来源与耦合风险

### 3.1 按量级的耗时来源（推测排序，非实测）

> 仓库**没有启动计时设施**（§4.3），下列排序依据是各步骤的阻塞性质，不是测量值。

1. **两个 WebView2 创建 + 两份前端 bootstrap**：主窗口对用户不可见，却付出完整代价（含全部特性动态 import、主题与 i18n 初始化）。
2. **前端串行配置 IPC**：`await` 逐个配置组，主窗口 5 次、Orb 窗口 7 次（§4.1），且全部在首帧之前。
3. **WebView2 缓存同步删除**：位于进程最前段，删多则首帧等待（缓存可达上万文件）。
4. **MCP 进程拉起**：spawn 外部进程，失败/慢启动不阻塞 setup，但会占用 CPU 与日志带宽。
5. **DB 迁移**：新库是建表（快）；存量库是逐列 `pragma_table_info` 探测 + 可能的 `ALTER`（列数已达 9 项），只在版本跨越多时变慢。
6. **沙箱探测**：Windows 后端会真实创建受限令牌与 Job 对象，并读一次目标目录 DACL（不落文件改动，见 `crates/assistant-tools/src/sandbox/windows/mod.rs:415-440`）；属系统调用级探测，成本低于建窗但非零。

### 3.2 启动期「必须同步」与「只是被同步了」的分界

- **真同步**：`main.rs` 的 profile 建目录/清理——理由写在 `webview_profile.rs:3-5`：WebView2 运行期持文件句柄，运行期删除会失败或损坏 profile。
- **假同步**：审计裁剪、结果清理、附件 tmp 清理——三者都是"越早越好"，不是"必须此刻"。
- **耦合风险**：`setup.rs:120` 的 `tool_setup::init(handle)?` 与 `setup.rs:74` 的 DB 初始化都用 `?` 传播错误，即**工具侧/DB 侧任何失败都会终止整个启动**；而特性 `on_setup` 失败只 `log::error`（`setup.rs:51-55`）。同一份 setup 里存在两种失败语义，需要明确哪一种是有意的。

### 3.3 主窗口懒建的前提

- 前端已有按窗口 label 分派的结构（`src/App.tsx:8-15`），主窗口不存在时不会有人渲染它。
- 后端已有 `show_or_create`（`window.rs:135`），且已处理 `data_directory`（`window.rs:172`）——即"动态创建主窗口"的路径已经可用，只是启动期走的是配置声明路径。
- 约束：`restore_main_window`（`window.rs:324`）依赖窗口已存在；若改懒建，几何恢复必须挂到创建路径上，且 `WINDOW_SPECS` 里的尺寸需与恢复值同源。

### 3.4 Orb 窗口减载的前提

- `bootstrap()` 是**无条件**的：`loadFeatures()` 与全部配置组对两个窗口一视同仁（`main.tsx:44-53`）。
- Orb 实际消费：`orb`、`mascot` 两个配置组（`OrbWindow.tsx:22-24`）+ `screenshot` 会话 + `OrbBall`/`OrbPanel`。
- 约束：`initSettingSystem()` 注册的是全局槽表，`getConfGroups()` 返回全部槽；按窗口裁剪需要"窗口 → 所需组"的显式映射，且 `applyLocaleChange` 仍必须执行（Orb 面板有文案）。

---

## 4 现状漂移与缺口

### 4.1 同一配置被反复读取

| 配置组 | 读取点 | 次数 |
|---|---|---|
| `app` | `setup.rs:56`（校验）、`:64`（语言）、`:83`（审计保留期）、`:232`（快捷键）、`window.rs:326`（几何恢复）、`themeStore.ts:58`（前端） | Rust 5 次 + 前端 1 次/窗口 |
| `orb` | `setup.rs:59`、`:280`（定位）、`window.rs:78`（属性）、`setup.rs:361`（巡检内，仅吸附时） | 3~4 次 |
| `mascot` | 前端 bootstrap + `OrbWindow.tsx:23` | 2 次（Orb 窗口） |
| `vault` | 前端 bootstrap + `vault_plugin.rs:370` | 1 次 Rust + 1 次/窗口 |

`ConfGroup::load()` 每次都是真实读盘（`crates/assistant-config/src/conf_group.rs:58-64`），没有进程内缓存。

### 4.2 失败语义不一致

| 位置 | 失败后果 |
|---|---|
| `setup.rs:70`（`AppDirs`） | 终止启动 |
| `setup.rs:74`（DB） | 终止启动 |
| `setup.rs:120`（工具装配） | 终止启动 |
| `setup.rs:51-55`（特性 `on_setup`） | 只记 error，继续 |
| `setup.rs:56-60`（配置校验） | 只记 error，继续 |

### 4.3 缺启动计时

`src-tauri/src` 启动路径（`main.rs` / `lib.rs` / `plugins.rs` / `setup.rs` / `tool_setup.rs`）无 `Instant::now()`；`Instant` 仅出现在截图、审批、工作区探测等功能模块。现有唯一时点信号是 `setup.rs:146` 的 `Assistant started` 日志行——**它只标记结束，不提供分段耗时**。前端也无 `performance.mark` 类埋点。

### 4.4 DB 未设连接级参数

全仓无 `journal_mode` / `busy_timeout` / `synchronous` 设置；串行化依赖进程内 `Mutex<Connection>`（`setup.rs:93`）+ 单实例。这是现状事实，不是缺陷判定——是否需要参数化属规格职责。

---

## 5 候选方向（待规格定案，不构成决议）

> 本节只列**可能成立**的方向与各自的判据，不下结论、不排优先级。

| 方向 | 前置条件 | 需先定案 |
|---|---|---|
| 主窗口懒创建 | 几何恢复挂到创建路径；`WINDOW_SPECS` 与恢复值同源 | 首启无主窗口时，托盘/快捷键/单实例唤起是否都走 `show_or_create` |
| Orb 窗口按需加载 | 建立「窗口 → 所需配置组」映射；保留 `applyLocaleChange` | 映射的持有者（前端常量表 or 后端注入） |
| 配置读取收敛 | 引入进程内配置快照（谁持有、何时失效） | 与「运行期改设置立即生效」的既有语义如何共存（如审批超时是"每次现读"） |
| 启动期工作移出关键路径 | 明确"首次使用前"的挂钩点（首次命令 / 首次会话 / 首次开设置页） | 审计裁剪与结果清理是否接受"本次运行不清理" |
| 启动计时埋点 | 分段 `Instant` + 结构化日志 | 埋点粒度与是否进前端首帧指标 |
| 失败语义统一 | 明确哪些失败允许降级 | 工具装配失败时是否允许"无工具启动" |

---

## 6 待定案项

| # | 问题 | 影响面 |
|---|---|---|
| Q1 | 「工具装配失败即启动失败」是否为有意设计 | 启动可用性 vs 功能完整性 |
| Q2 | 主窗口是否允许首启不创建（完全懒建） | 窗口几何恢复、单实例唤起、`data_directory` 一次性设置 |
| Q3 | Orb 窗口加载范围是否允许与主窗口不同 | 配置组注册表的按窗口视图 |
| Q4 | 配置读取是否引入进程内快照 | 与"运行期改设置立即生效"的既有语义 |
| Q5 | 启动期清理的"最晚可接受时点" | 审计保留期语义、工具结果保留策略 |
| Q6 | 是否接受"本次启动不清理、下个安全点清理" | WebView2 已采用该模式（标记 + 启动消费），其余清理是否对齐 |

---

## 7 事实边界

- 本文只覆盖**启动到首帧**，不含运行期热路径（流式、工具执行、存储清点）。
- 耗时排序是**阻塞性质推断**，仓库无实测数据；任何优化决策前应先补 §4.3 的分段计时。
- 「必须」的判定基于当前设计约束（唯一安全执行点、单实例、托盘常驻、窗口隐藏），约束变更后结论需重核。
- 已归档规格只以文档名引用；未在本文引用的历史规格不构成判据。
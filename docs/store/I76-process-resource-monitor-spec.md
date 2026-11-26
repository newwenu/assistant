# I76 — 进程资源监视规格

| 项 | 值 |
|---|---|
| 状态 | 待实施（观测层规格：资源监视器页面容器 + **进程树** tab） |
| 日期 | 2026-11-25 |
| 类型 | 设计规格（取数模型、契约、UI、验收） |
| 关联 | **无外部依赖**：正文所需的定义、词表、字段、装配点全部写在本文件内，不引用任何其它文档，也不存在「见某文档」的悬空引用。正文提及的其它文件只有两份，且均非权威：`I54-resource-monitor-spec.md`（**历史参考**，已被本文取代）、`I61 存储管理器规格`（**已归档**，仅用于说明存储 tab 的挂载位，其内容不属本文） |
| 平台 | **三平台全交付（代码与能力层）**：Windows / Linux / macOS。仓库既有三平台后端范式（沙箱、生物识别、解锁对话框、屏幕锁定、崩溃转储均各有 `cfg` 分模块实现），本文沿用同一范式；非目标 OS 构建落 `notImplemented`（§2.7）。Linux / macOS 由 CI 真机验证、Windows 由开发机验证（§10.1）。三平台安装包不属本期（打包发布为 `ARCHITECTURE.md` I8，未开始） |
| 影响范围 | 前端：`src/settings/pages/StorageSettings.tsx`（改为容器）、`src/settings/pages/storage/StorageContent.tsx`（去壳）、`src/settings/core/registry/builtinRegistry.ts`（label 值与 keywords）、`src/settings/pages/resource/ProcessTreeTab.tsx`（新增）、`src/services/desktop/resourceService.ts`（新增）、i18n 两份 locale。后端：`src-tauri/src/cmd/resource.rs`（新增）、`src-tauri/src/cmd/mod.rs`、`src-tauri/src/ipc.rs`、`src-tauri/Cargo.toml`、`src-tauri/crates/assistant-resource-probe/`（新增，含三平台后端与 mock）、`src-tauri/crates/assistant-protocol/src/resource.rs`（新增）与其 `lib.rs` / `tests/protocol_bindings.rs`、`src/types/protocol.contract.ts`（生成）。golden fixture 只允许 `snapshot.full.json` / `snapshot.vaultless.json` 的 `pages[].id` 一行变化，由 `vitest -u` 重生成；`snapshot.diff.json` 不变（§10.2 V4） |
| 一句话 | 用一次全系统快照 + 父子链过滤 + DFS 子树，把「本应用自己这一棵进程树」摊成一张表，表头给「自身」与「全树」两个合计；采样是纯拉取 + CPU 差分缓存，不常驻、不预热、不登记进启动分层 |

> **本文取代 `I54-resource-monitor-spec.md`。I54 只作历史参考，不再作为权威来源**：两者冲突时以本文为准，实施一律不参照 I54，也不要求 I54 与本文保持同步。
>
> **本文自包含**：所有被引用的定义、词表、字段、装配点都写在本文内；不存在附录、不存在「见某未立项文档」的悬空引用。凡本期不做的内容，一律在 §11 明确登记为推迟项，而不是留在正文里当待补章节。
>
> **范围**：设置页「资源监视器」的**页面容器**与其中的**进程树 tab**。存储 tab 已完成，其内容**不在本文范围内**（以既有实现为准，本文不改动其行为），本文只为其保留挂载位（§3.4）。

**编号约定**：`P1–P6` 指标方向、`V1–V26` 验收判据、`R0` / `R1a` / `R1b` / `R1c` / `R2` 实施批次、`OQ1–OQ9` 开放问题。前缀互不重叠，正文内引用不再歧义。本文中的「必须 / 禁止」为可验收约束。

**进度表**：`docs/store/I76-process-resource-monitor-tasklist.md` 只记「改哪里、改没改」，语义一律以本文为准。

---

## 0 判定

本规格要拍板的结论，一条一行；依据在后续章节。

| # | 判定 | 依据 |
|---|---|---|
| 0.1 | 进程树与「自身占用」合并为**一个 tab**（两张表变一张表） | 「宿主禁止进任务表」是**行**级约束，不是 tab 级；拆 tab 只为绕开它属于跳步（§3.2） |
| 0.2 | 页面 id 由 `storage` 改为 `resource`（路由随之变为 `/settings/resource`），label 键同步改为 `common.navigation.resource.title` | 页面语义已从「存储管理器」变为「资源监视器」，id 与 URL 必须跟着改；golden fixture 由 `vitest -u` 重新生成，不手改（§3.1、§10.2 V4） |
| 0.3 | 采样只用**一次全系统快照**，禁止逐 PID 查询 | O(N) 次系统调用开销随时序恶化（§2.2） |
| 0.4 | 角色判定分**三层**（归属 → 家族 → 命令行），命令行读不到落 `webviewUnknown` 且**仍计入自身** | 一次读取失败不得把整棵 WebView 子树踢出「自身」合计（§2.3） |
| 0.5 | 可加性是**两个独立标志**（可求和 / 等于物理占用），挂在快照顶层 | 一个标志无法同时表达工作集与私有工作集的区别（§2.4） |
| 0.6 | 合计是否显示数字**只由 `memoryAdditive` 决定**；为 `false` 时不显示数字 | 「显示合计 + 加一句标注」等于把不可加的数当合计用（§5.4） |
| 0.7 | 线程数只取**宿主**、只进合计头，**不进**表列 | 子树内其它进程的线程数无可解释性，且跨进程取会破坏「一次快照」（§5.3） |
| 0.8 | **峰值本期不交付**，且不留任何占位 UI | 证据自指：进程内采样在 OOM / 卡死时必然丢失最后数据（§5.6、§11.1） |
| 0.9 | 命令只有**两条**，能力声明独立成命令、不含任何数字 | 能力与数字同响应必然需要跨 tab 聚合响应（§6.1） |
| 0.10 | 推迟项（峰值 / 停止 / 任务归因）全部登记在 §11，正文不留待补章节 | 悬空引用使文档无法闭环（§11） |
| 0.11 | 本期**三平台全交付**（代码与能力层：Windows / Linux / macOS），不按平台裁剪 | 仓库既有三平台后端范式（沙箱 / 生物识别 / 解锁对话框 / 屏幕锁定 / 崩溃转储），CI 已在 `ubuntu-latest` + `macos-latest` 真机跑全工作区测试；§2.4 的三种内存口径各需对应实现与证据（§2.4、§10.1、§10.2 V14） |
| 0.12 | 平台后端**按平台分批**（R1a → R1b → R1c），每批单独可编译、可合入 | 三平台一次性合入会把三套编译差异压进同一次 `contract:gen` + 装配，回滚粒度过大（§10.1） |
| 0.13 | **R0 / R2 是原子批次**：R0 内含 11 个 DTO + 契约生成 + 两条命令骨架（返回 `notImplemented`），R2 一次性交付表 + 两个合计 + 宿主线程数 + 口径分支 + 全部状态展示 | 批次若留中间态，合入后必然出现「invoke 到不存在的命令」或「有表无合计」，都属半成品；命令骨架要返回类型化响应，因此 DTO 必须在 R0 而非 R1a（§10.1） |
| 0.14 | `notImplemented` **保留**，语义收窄为「非目标 OS 构建 / 该平台后端未定案」 | 三平台交付后若删除该取值，§2.7「四个取值全部被使用」即自相矛盾；保留并明确产出条件（§2.7、§10.2 V25） |
| 0.15 | OS 层 `SnapshotSource` trait + mock 实现**计入交付物** | V8 / V10 / V11 / V12 都要注入假快照；不列为交付物，这四条判据无法落地（§7.1） |

---

## 1 范围与交付

### 1.1 本期交付

| 项 | 交付物 |
|---|---|
| 页面容器 | 一个页面（id `resource`，路由 `/settings/resource`）、页内两个 tab（存储 / 进程树）、统一 sticky 基准 |
| 进程树 tab | 一张实时表 + 两个合计 + 宿主线程数 + 口径与降级展示 |
| 契约 | 两条命令（§6.1）、11 个 DTO（§6.2）、生成后的 `src/types/protocol.contract.ts` |
| 采样 | 新 crate `assistant-resource-probe`（§6.4）：OS 层 `SnapshotSource` trait + 三平台后端（`win.rs` / `linux.rs` / `macos.rs`）+ 纯逻辑层（建父子表 / DFS / 角色分类 / PID 复用校验）+ mock 实现 |
| 平台覆盖 | Windows / Linux / macOS 三平台 `processTree` 均为 `available`；非目标 OS 构建为 `notImplemented`（§2.7） |
| 验证覆盖 | Linux / macOS 走 CI 真机，Windows 走开发机（§10.1「平台验证分工」）；不新增交叉编译 job |

### 1.2 本期明确不交付

| 项 | 理由 | 登记处 |
|---|---|---|
| 峰值区（峰值内存 / 峰值 CPU / 热点） | 进程内采样无法在 OOM / 卡死时保留最后数据；写端必须是独立进程，且受「单 package 单产物」约束不能进 bundle，分发路径未定 | §11 |
| 停止能力（行级停止按钮） | 句柄已具备，缺「MCP 纳入统一回收抽象」与「句柄暴露成标识」；属范围选择，不是被任何登记表阻塞 | §11 |
| 任务级归因（任务名列） | 需进程登记表 + 身份代际 | §11 |
| 三平台安装包 / 分发 | 打包发布是独立立项（`ARCHITECTURE.md` I8，当前未开始）；本文只交付代码与能力层 | §10.1「交付边界」 |
| 全机视图（全系统进程表） | 不同产品形态，涉及隐私边界与按需指标机制 | §12 OQ3 |
| 非目标 OS（Android / iOS / BSD 等） | 本期只承诺三平台；其它目标构建一律 `notImplemented`，不做「部分可用」 | §2.7 |

**不交付项一律不出现占位 UI**：不放灰色按钮、不放「暂无数据」空块、不建永空的读取器。能力声明（§6.1）必须能让前端知道该 tab 处于 `notImplemented`。

### 1.3 指标方向编号

本文用 `P1`–`P6` 指代「问题方向」，只用于表格互指，不表示批次。

| 编号 | 方向 | 本期 |
|---|---|---|
| P1 | 进程存在与父子关系 | 做 |
| P2 | 进程 CPU | 做 |
| P3 | 进程内存 | 做 |
| P4 | 进程角色 | 做 |
| P5 | 进程归属任务 | 推迟（§11） |
| P6 | 进程回收 / 停止 | 推迟（§11） |

---

## 2 术语与硬约束

### 2.1 观测者与被观测者

监视器跑在**被监视的那个进程里**。由此产生两个自指禁区，表现不同，约束也不同：

| 禁区 | 后果 | 约束 |
|---|---|---|
| **动作自指**（停止） | 宿主进程进受管任务表 → 停止自己 = 应用自杀 | 宿主**禁止**登记进任务表；停止能力按**行**禁用（§11） |
| **证据自指**（峰值） | 采样器与被观测者同生共死 → OOM / 卡死时最后那次采样必然丢失，峰值永远是「还没崩的时候」 | 峰值**禁止**由自身采样提供，只能由外部留痕；因此本期不做峰值（§5.6） |

两条同源于「观测者 = 被观测者」，但第一条可以靠「不登记」绕开，第二条无法绕开，只能换观测者。

### 2.2 一次快照（硬约束，四条）

采样**只能发起一次全系统快照**，之后全部在内存里做。四条约束：

| # | 约束 |
|---|---|
| 1 | **每拍只做一轮全量枚举**：每个进程在**一轮**里最多取一次基础字段，**禁止**按字段分次枚举（先取一遍 pid、再取一遍内存……），也**禁止**按子树分组重复枚举。Windows 用 `NtQuerySystemInformation(SystemProcessInformation)` 一轮拿全；Linux 遍历 `/proc` 每进程读 `stat` / `statm`（属同一轮，不是「第二次快照」）；macOS `proc_listpids` 一轮列出 PID 后 `proc_pidinfo` 每进程取一次。三者的「一轮」等价，判据是**每拍的总枚举次数为 1**，不是「系统调用次数为 1」 |
| 2 | 一次快照必须能拿到：`pid` / `parentPid` / `startTime` / 内存 / CPU 时间（可选 `threadCount`）。拿不到的字段输出 `null`，**禁止**为它补发第二次全系统快照 |
| 3 | **镜像名只能作粗筛**，**禁止**用它判定归属。`msedgewebview2.exe` 是全机器共享的名字，按名字收进程必然把别的应用的进程算进来 |
| 4 | 建父子链后**必须**做 PID 复用校验：`child.startTime >= parent.startTime` 且父 PID 在快照内。不通过则排除该进程并标注，**禁止**静默并入子树 |

**子树边界**：只观测本进程的后代。主动 `detach` / `daemonize` 的子进程会脱离父子链，从而永久不可见——这是这类进程的设计意图，不是采样失败。此边界必须写在 UI 上（§5.5）。

### 2.3 角色判定（三层，取数成本递增）

把三层压成一层（「命令行含 `--type=` 就是 WebView」）是本规格最容易犯的错：命令行不在系统快照里，逐进程读命令行会让 §2.2 的「一次快照」失效，并让「读不到命令行」变成一个**错误结论**而不是**未知**。

**唯一的例外**：第 ③ 层对**已确认在子树内且家族命中**的进程读命令行，是 §2.2 约束 1 允许的有界例外——范围被 ①② 层收窄到极小集合，不是第二次全量枚举。命令行走不到时不重试、不降级归属，只落 `webviewUnknown`。

| 层 | 判据 | 取数 | 作用 |
|---|---|---|---|
| ① 归属 | 从 root 沿 `parentPid` 链可达 | 一次快照内 | 决定「在不在本应用子树」 |
| ② 家族 | `exeName` ∈ 本平台 WebView 家族：Windows `msedgewebview2.exe` / `msedge.exe` / `chrome.exe`；Linux `WebKitWebProcess` / `WebKitNetworkProcess`；macOS `com.apple.WebKit.WebContent` / `com.apple.WebKit.Networking` / `com.apple.WebKit.GPU` | 一次快照内 | 决定「是不是 WebView 系」 |
| ③ 细分 | 命令行 `--type=renderer` / `--type=gpu-process` / 其余 | 逐进程额外读取（仅对已确认在子树内且家族命中的进程） | 决定「是哪一类 WebView 进程」 |

第 ② 层用镜像名不违反 §2.2 约束 3：那里禁止的是用镜像名**判定归属**，归属已由第 ① 层定死；镜像名只在**已确定的子树内**分家族。

| 角色 | 判据 | 归入哪个合计 |
|---|---|---|
| `host` | PID == root | 自身 |
| `webviewRenderer` | 家族命中 + 命令行含 `--type=renderer` | 自身 |
| `webviewGpu` | 家族命中 + 命令行含 `--type=gpu-process` | 自身 |
| `webviewUtility` | 家族命中 + `--type=` 的其余取值 | 自身 |
| `webviewBrowser` | 家族命中 + 命令行可读但无 `--type=` | 自身 |
| `webviewUnknown` | 家族命中但**命令行读不到** | **自身** |
| `otherChild` | 其余全部后代 | 全树 |

两条必须写死的规则：

1. **`webviewUnknown` 必须计入自身合计**。若把「命令行读不到」降级成 `otherChild`，一次读取失败就会把整棵 WebView 子树踢出「自身」合计——而「本应用占多少」正是本 tab 要回答的核心问题，那会是一次静默错误，比少细分一档严重得多。读不到命令行只影响第 ③ 层，**永远不得影响 ① ② 层**。
2. **DFS 必须覆盖任意深度**。WebView2 的 renderer / GPU / utility 是 `msedgewebview2.exe` 根进程的子进程，即宿主的**孙进程**；只取直接子进程会漏掉真正吃内存的那几个。

**平台可达性差异（三平台必读）**：Windows 与 Linux 的 WebView 进程是宿主的后代（`msedgewebview2.exe` 根进程由宿主创建、`WebKitWebProcess` 由宿主 fork），① 层可达，角色细分成立。macOS 的 WKWebView 内容进程由系统 `launchd` / XPC 托管，**通常不是宿主的后代**，① 层会判为不可达——此时表内不含 WebView 行、`selfGroup` 只含宿主，必须在 UI 用 `coverage` 说明「WebView 子进程由系统托管、不在本应用子树内」；**禁止**用镜像名绕过 ① 层把它们收进来（§2.2 约束 3）。是否改用 XPC 归属辅助见 §12 OQ9。

`host` 行的 `parentPid` 是宿主的父进程（如 `explorer.exe`），不在 `entries` 内，只作诊断信息，**禁止**前端据此查父行（树序由后端出，§5.1）。

### 2.4 内存口径与可加性

可加性不是一件事，是**两件事**，必须分开判定，否则必然错一边：

| 口径 | ① 可求和（不重复计数） | ② 等于物理占用 | 平台 |
|---|---|---|---|
| 工作集 / RSS（`workingSet`） | **否**：共享页（COW、共享 DLL、WebView 共享段）在每个进程里各记一次，求和必然**大于**物理占用 | 否 | Linux |
| 私有工作集（`privateWorkingSet`） | **是**（近似） | 否（近似） | Windows |
| `phys_footprint`（`physFootprint`） | **是**（近似） | 是（近似） | macOS |

「近似」是固定含义：**误差不来自共享页重复计数**。所以：

- `memoryAdditive == false` 时，**禁止**把子项求和当总量，也**禁止**在合计位显示数字（§5.4）；
- 两个标志是**指标属性**，挂在快照顶层，**禁止**下放到组或行——否则两个组的口径可被独立改写；
- 列名必须带口径（「工作集」/「私有工作集」/「物理占用」），**不靠脚注**。

### 2.5 CPU 口径

| 项 | 定义 |
|---|---|
| 单位 | `singleCorePercent`：单逻辑核百分比，**可超过 100** |
| 可加性 | CPU 天然可加，**不设可加性标志**；合计必须注明「单核百分比之和，可超过 100」 |
| 首拍 | 差分需要两次读数，**首拍必然 `null`**，禁止填 `0` |
| 新增行 | 刚 spawn 的进程在下一个刷新周期前没有数字（`null`），不是 `0` |

### 2.6 采样生命周期

| 项 | 规则 |
|---|---|
| 驱动 | **纯拉取**：每次 `get_process_snapshot` 调用都读一次快照 + 差分缓存 |
| 缓存 | 只缓存上一拍的**逐 PID CPU 累计时间**，用于算差分；超时即整体丢弃。缓存**类型**定义在 `assistant-resource-probe`（纯逻辑，可单测）；缓存**实例**是 `cmd/resource.rs` 内的 `OnceLock<Mutex<CpuDiffCache>>`，首次命令调用时才初始化——**禁止**用 `AppHandle::state` 持有、**禁止**挂到 `boot::access.rs` 的访问器、**禁止**成为 `boot::RESOURCES` 的表项，否则违反「页面未打开时不执行采样代码」 |
| 空闲 | 超过 `CPU_CACHE_IDLE_TIMEOUT_MS` 无调用 → 缓存失效，下一次调用重新从首拍开始（CPU 为 `null`） |
| 禁止 | **禁止**启动预热采样；**禁止**后台常驻采样线程；**禁止**把采样登记为**启动分层资源项**——该禁令覆盖两个入口：`boot::RESOURCES` 中 `Lane::Boot` 的项（由 `boot::run_boot` 在启动期按序 `ensure`），以及 `Lane::Idle` 的项（由 `boot::runner::run_idle` 在启动完成后惰性 `ensure`）；判定方式统一为「页面未打开时，采样相关代码不得执行」；**禁止**为「开始 / 停止采样」增设命令 |
| 常量 | `SAMPLE_INTERVAL_MS = 1000`（前端拉取间隔）、`CPU_CACHE_IDLE_TIMEOUT_MS = 3000`、`MIN_CPU_DELTA_MS = 500`（同一 PID 两次读数最小间隔，不足则沿用缓存并输出 `null`）、`MAX_ENTRIES = 512`、`MAX_DEPTH = 16`。全部为命名常量，禁止魔法数字；`CPU_CACHE_IDLE_TIMEOUT_MS` 必须落在 `SAMPLE_INTERVAL_MS` 的 2–3 倍区间 |

「停表」不是一条命令，而是**没有调用**这一事实本身：没有常驻线程、没有单例，因此不存在泄漏路径。

### 2.7 可用性词表（封闭）

`ResourceAvailabilityDto` 只有四个取值，**全部在本文被使用**：

| 取值 | 含义 | 出现位置 |
|---|---|---|
| `available` | 有数字，可直接展示 | 能力声明、快照顶层、组的 `memoryAvailability` |
| `unavailable` | 本平台无该口径 / 该指标不可得（如 Linux 无可求和内存口径） | 快照顶层、组的 `memoryAvailability` |
| `notImplemented` | 未实现：**非目标 OS 构建**（本期三平台之外）或该平台后端尚未定案 | 能力声明、快照顶层 |
| `unreadable` | 读到了但读取 / 解析失败 | 快照顶层 |

约束：

1. 新增取值必须**同时**改 DTO、后端产出与前端渲染，三处缺一即视为未完成；**禁止**前端用字符串兜底匹配。
2. 该词表与存储侧 `StorageAvailabilityDto` **同形不同集**（后者多出 `missing` / `locked` 等存储语义），**不复用**，也不允许互相传值。
3. **wire 不承载本地 UI 状态**：前端「能力声明读取失败」是本地状态，用交叉类型扩展（§7.2），不得新增 wire 取值。

---

## 3 页面与容器

### 3.1 页面身份（id 改名，结构性字段其余不动）

| 项 | 值 | 约束 |
|---|---|---|
| 页面 id | `resource` | **由 `storage` 改为 `resource`**。理由：页面语义已从「存储管理器」变为「资源监视器」，id 是导航、路由、投影三者共用的键，留着 `storage` 会让 URL 与标题长期不一致 |
| 路由 | `/settings/resource` | **随 id 自动变化，不单独配置**。全仓只有一条参数化路由 `/settings/:pageId`（`src/layouts/MainLayout.tsx`），URL 由页面 `id` 拼出；**禁止**新增 redirect 或别名路由——`isSettingsPageId()` 的实现是 `pageMap.has(value)`，加别名只能靠再登记一个 manifest 项，那等于多出第二个导航项（违反 §10.2 V1） |
| label 键 | `common.navigation.resource.title` | **键名随 id 同步改名**（原 `common.navigation.storage.title`），值改为「资源监视器」/`Resource Monitor`。键名与 id 保持一致，避免出现「id 是 resource、键叫 storage」的错位 |
| `keywords` | 追加 `resource` / `process` / `cpu` / `memory` / `资源` / `进程`，**保留**原有 `storage` / `attachment` / `orphan` / `disk` / `存储` / `附件` / `孤儿` | 存储 tab 仍在，旧词必须保留，否则存储相关搜索失效 |
| `categoryId` / `order` / `icon` / `emptyPage` | `general` / `60` / `DatabaseOutlined` / `true` | **全部不改**。`order` 决定导航位置，改动会打乱既有排序；`icon` 沿用 `DatabaseOutlined` 属待评估项（§12 OQ8） |
| 文件名 | `src/settings/pages/StorageSettings.tsx`、`src/settings/pages/storage/**`、CSS 类名 `.storage-*`、日志标签 `createLogger("storage")`、i18n 内容键 `settings.storage.*` | **全部不改**。它们指的是「存储这一块内容」，与页面 id 无关；跟着改名会扩大改动面且无收益 |
| tab key | `storage` / `processTree` | **tab key 不跟着改**。tab key 是页内身份，只在 `Tabs` 内部比较，不参与路由与投影 |

### 3.2 两个 tab

| tab key | 内容 | 归属 |
|---|---|---|
| `storage` | 存储管理器（已完成） | 不属本文，只保留挂载位（§3.4） |
| `processTree` | 进程树（本文主体） | §4、§5 |

合并成一个页面内的两个 tab 而不是两个页面，理由只有一条：**两者回答的是同一个问题「本应用占了多少资源」的两半**。原「宿主禁止进任务表 ⇒ 必须独立 tab」是**跳步**——该约束落在**行**上，不落在 tab 上（§2.1）。

### 3.3 容器装配（三件事必须做对）

| # | 事项 | 做法 |
|---|---|---|
| 1 | **壳只能有一个** | 现在 `StorageContent` 自己在「加载/失败」与「正常」两个返回分支里各渲染一次 `SettingsPageShell wide`。容器化后由 `StorageSettings.tsx` 渲染**唯一一次** `SettingsPageShell wide`，`StorageContent` 改为**只渲染 body**（去掉自带壳）。否则会出现壳套壳：padding 双算、`--storage-sticky-top` 作用域嵌套、页面标题重复 |
| 2 | **tab 栏与表头共用 sticky 基准** | 复用既有 CSS 变量 `--storage-sticky-top`（定义在 `.settings-page-shell` 上，当前值 40px）。**禁止**为第二个 tab 引入第二个偏移常量 |
| 3 | **切换不重建壳** | `Tabs` 用受控写法（`activeKey` + `onChange` + `items`），切 tab 只换 `children`，壳与滚动容器保持。滚动位置按 tab 独立记忆：由**容器层**（`StorageSettings`）通过页面根节点的 `ref` 向上取第一个可滚动祖先（即 `AppShell` 的 `Layout.Content`），按 `activeKey` 保存 / 恢复 `scrollTop`；**禁止**为 tab 新建内层滚动容器（会改变 sticky 基准，破坏 §3.3 第 2 条），也**禁止**为此修改 `AppShell`（§7.2） |

### 3.4 存储 tab 挂载位

存储 tab 已完成，以 I61 存储管理器规格（已归档）为准，本文不涉及其内容；容器只需为其保留挂载位（`Tabs` 的第一个 `item`，`key: "storage"`），并把壳统一到容器层（§3.3 第 1 条）。

---

## 4 取数模型

### 4.1 主链路

```
一次全系统快照
  → 内存建 pid → 条目 表
  → 从 rootPid 沿 parentPid 反向遍历（DFS 先序）
  → 每跳校验 startTime 单调 + 父存在（§2.2 约束 4）
  → 逐行分类角色（§2.3 三层）
  → 累加两个合计（§5.2）
  → 输出树序 entries + 两个组 + 顶层口径
```

| 项 | 规则 |
|---|---|
| 排序 | 后端按 DFS 先序输出，**前端不重建树** |
| 去重 | 一个进程只出现一次，`entries` 中 `pid` 不重复 |
| 截断 | 深度超 `MAX_DEPTH` 或条数超 `MAX_ENTRIES` → `truncated = true`，UI 必须显示「本次扫描不完整」，**禁止**把截断结果当完整快照 |
| 中途退出 | 遍历期间进程消失 → 该行跳过，不报错；快照是时刻值，不承诺持续状态 |
| 失败 | 快照本身取不到 → 顶层 `availability = unreadable` + `reason`，**不抛命令错误**（§6.3） |

### 4.2 CPU 差分

| 项 | 规则 |
|---|---|
| 缓存键 | `pid` + `startTime`（只用 `pid` 会在 PID 复用后把两个进程的累计时间相减，算出负值或巨额） |
| 算法 | `(t2 - t1) / (wall2 - wall1) / 核数 × 100`，结果按 `singleCorePercent` 输出 |
| 首拍 | 无缓存键 → `null` |
| 间隔不足 | `wall2 - wall1 < MIN_CPU_DELTA_MS` → 沿用上一拍的百分比，不重算（避免除以极小值放大抖动） |
| 缓存淘汰 | 快照里不再出现的 `pid + startTime` 从缓存删除；整体超时按 §2.6 丢弃 |

---

## 5 展示

### 5.1 一张表：行 = 进程，全树只出现一次

| 项 | 定义 |
|---|---|
| 行 | 整棵子树（root = 宿主 PID）中的**每一个进程，只出现一次** |
| 列 | 进程（`exeName` + `pid`）/ 角色 / CPU / 内存（列名带口径） |
| 不设「存在」列 | 快照里出现的每一行**本身就是存活的**（死进程不在快照内），该列恒为真、无信息量；「刚退出」由行消失表达 |
| 不设「投影」列 | 归入哪个合计完全由 `role` 决定，投影只是 `role` 的派生 |
| 不设线程数列 | 线程数只对宿主有意义，且会变成一列几乎全空的表（§5.3） |
| 不设任务名列 | 需登记表，本期推迟（§11） |
| 不设停止列 | 本期推迟，且将来也是**行级**禁用（§11） |

**无归因边界**（必须写在 UI 上）：同一角色的多个进程在本期是**可区分但不可命名**的（按 PID 区分），用户看到的是 `msedgewebview2.exe (pid 1234)` 而不是「渲染进程 #2」。

### 5.2 两个合计

| 合计 | 成员 | 用途 |
|---|---|---|
| **自身** | `role ∈ {host, webviewRenderer, webviewGpu, webviewUtility, webviewBrowser, webviewUnknown}` | 回答「本应用占多少」——最常被问的那个问题 |
| **全树** | 全部行（含 `otherChild`） | 回答「连同我启动的子进程一共多少」 |

约束：

1. 默认口径就是「自身 = 宿主 + 全部 WebView 系进程」，**不是**「仅主进程」。不选口径等于默认选了「仅主进程」，而它恰好是最容易被质疑的那个（用户看 200 MB、任务管理器看 900 MB）。
2. 两者是**包含关系**（自身 ⊆ 全树），必须同时显示并标注包含关系；**禁止**把二者相加。
3. 合计与逐行数字**必须**出自同一 `sampledAt`，因此只能有一条命令（§6.1）。
4. 合计的内存数字受 §5.4 分支控制；CPU 合计不受限，但必须注明「单核百分比之和，可超过 100」。

### 5.3 宿主线程数（不是表列）

线程数只在**宿主行**有诊断价值：子树内其它进程（WebView 内部）的线程数不具可解释性，且跨进程取线程数在部分后端需要额外一次系统枚举，会破坏 §2.2 约束 2。

因此：

| 项 | 结论 |
|---|---|
| DTO 落点 | 快照顶层 `hostThreadCount`（`u32 \| null`），**不进** `ProcessEntryDto` |
| UI 落点 | 合计头部的宿主摘要（`host · pid 1234 · 42 线程`），**不做**表格列 |
| 取不到 | `null`，**禁止**填 `0` |
| 不做 | 线程语义归属（哪个子系统 / 哪个任务）、线程管理（Rust 无安全取消）。线程数多通常是正常的；真正的并发压力指标是 tokio task 数 / 阻塞池排队深度，需另立 metrics 导出项 |

### 5.4 内存口径分支（唯一规则）

**唯一规则**：合计位是否显示数字，只由 `memoryAdditive` 决定。

| `memoryAdditive` | 合计位 | 逐行内存 | 列名 |
|---|---|---|---|
| `true` | 显示数字；`memoryEqualsFootprint == true` 时标注「≈ 物理占用」，否则标注「不含共享页重复计数」 | 显示 | 按 `memoryBasis`（私有工作集 / 物理占用） |
| `false` | **不显示数字**：显示 `unavailable` + `memoryReason` 文案（如「本平台无可求和内存口径」） | **照常显示** | 「工作集」并在列头标注「含共享页重复计数」 |

三条必须写死：

1. **禁止**「显示合计但加一句标注」——那是把不可加的数当合计用，标注救不回来。
2. 分支必须由**字段驱动**，不能靠文案或人自觉（§10.2 V14）。
3. `memoryBytes` 取不到时输出 `null`，**禁止**填 `0`。

### 5.5 状态与降级展示

| 情形 | UI |
|---|---|
| 快照顶层 `notImplemented` | 整个 tab 显示「本平台后端未定案 / 非目标平台」+ `reason`，不显示空表 |
| 快照顶层 `unreadable` | 显示读取失败 + `reason`，保留上一次成功快照并标注其 `sampledAt` |
| 组 `memoryAvailability == unavailable` | 见 §5.4 第二行 |
| `truncated == true` | 表上方横幅「本次扫描不完整（已达深度/条数上限）」 |
| CPU `null` | 单元格显示「本拍无基准」，**禁止**进错误态、**禁止**提示「数据加载失败」。首帧必然如此（§2.6 纯拉取，每次打开 tab 都是首拍） |
| `coverage == subtreeOnly` | 常驻一行说明「只覆盖本进程子树；主动脱离父子链的进程不可见」 |
| `webviewDetached == true`（本期即 macOS） | 常驻一行说明「WebView 子进程由系统托管，不在本应用子树内」（§2.3 平台可达性差异） |
| 能力声明读取失败 | **本地**横幅 + 保留上次渲染；**禁止**默认判为可用，**禁止**改 wire（§2.7 约束 3） |

### 5.6 峰值：本期不交付

峰值区块**本期不做**，且不是「先放个空块等数据」——按 §2.1 证据自指，进程内采样提供的峰值在 OOM / 卡死时必然丢失，那正是最需要它的时刻。因此：

| 项 | 结论 |
|---|---|
| 形态 | 未来必须是「外部产物 + 只读报告」，与实时树**分块渲染**，且必须显示产物生成时刻与「这是 X 之前的历史快照，不是当前值」 |
| 写端 | 必须是独立进程（不在宿主内），受「单 package 单产物」约束不进 bundle，分发路径未定 → §11 |
| 本期 UI | **不出现**峰值区块、不出现「暂无峰值数据」、不出现任何生成按钮 |
| 禁止 | 在进程内自采样峰值；让峰值与实时树共享 `sampledAt`；用 `0` 冒充未记录的峰值 |

---

## 6 契约

### 6.1 命令（两条，无输入参数）

| 命令 | 职责 | 失败语义 |
|---|---|---|
| `get_resource_capabilities` | 只返回每个 tab 的**可用性状态与原因**，**不含任何数字**（字节、条数、百分比皆无） | 自身异常走命令错误（`CommandErrorDto`）；前端降级为本地横幅（§5.5） |
| `get_process_snapshot` | 一次采样的全部结果：顶层口径 + 两个合计 + 树序 `entries` + 宿主线程数 | **取数失败不抛命令错误**，返回 `availability = unreadable` + `reason`（可解释失败比异常更好用） |

两条命令**无输入参数**，因此本期**不存在输入 DTO**，不涉及 `deny_unknown_fields` / `ts(optional = nullable)`。

**根 PID 由命令层注入**：`cmd/resource.rs` 调用采样时传 `std::process::id()`，作为 `rootPid` 与 DFS 起点。`assistant-resource-probe` **不自行读取宿主身份**（§6.4），保证 crate 可被单测注入任意根。

三条设计理由（写死，防止后续被拆）：

1. **能力声明必须独立成命令**：若把能力与数字塞进同一响应，就需要一个「跨 tab 聚合响应」，而那会让一个 tab 的失败连坐另一个 tab。
2. **快照必须只有一条命令**：实时树与两个合计**必须**来自同一次采样；拆成两条，两次调用之间的时间差会让「自身 + 子进程」不可加、不可解释。这条命令同时承担「缓存心跳」：每次调用刷新缓存时间戳，超时即丢弃（§2.6）。
3. **不为峰值设第三条命令**：峰值本期不交付（§5.6），不留永空的读取器。

### 6.2 DTO

全部落在 `assistant-protocol/src/resource.rs`，`#[serde(rename_all = "camelCase")]`，封闭集合用 `enum`。

**能力声明侧**

| 类型 | 字段 | 类型 | 约束 |
|---|---|---|---|
| `ResourceCapabilitiesDto` | `tabs` | `ResourceTabCapabilityDto[]` | 平铺数组，**不是 map**；前端按 `tab` 取值遍历，不猜键名 |
| `ResourceTabCapabilityDto` | `tab` | `ResourceTabDto` | `storage` / `processTree` |
| | `availability` | `ResourceAvailabilityDto` | §2.7 |
| | `reason` | `String \| null` | 键恒在；`available` 时为 `null` |

`ResourceTabDto` = `{ storage, processTree }`。`tabs` 恒含两项且 `tab` 不重复：`storage` 恒为 `available`（存储已交付），`processTree` 为 `available` 或 `notImplemented`（平台后端未定案时）。

**快照侧**

| 类型 | 字段 | 类型 | 约束 |
|---|---|---|---|
| `ProcessSnapshotDto` | `availability` | `ResourceAvailabilityDto` | 顶层状态；非 `available` 时 `entries` 为空、两个组数字为 `null` |
| | `reason` | `String \| null` | 键恒在 |
| | `sampledAt` | `i64 \| null` | 一次遍历一个时刻；非 `available` 时为 `null`（**不填 `0`**） |
| | `rootPid` | `u32` | 采样根 = 宿主 PID |
| | `memoryBasis` | `MemoryBasisDto` | 口径进 DTO，前端据此决定列名 |
| | `memoryAdditive` | `bool` | 内存指标是否可求和（§2.4） |
| | `memoryEqualsFootprint` | `bool` | 内存合计是否≈物理占用（§2.4）；与上者**相互独立**，禁止合并 |
| | `cpuBasis` | `CpuBasisDto` | `singleCorePercent` |
| | `coverage` | `CoverageDto` | `subtreeOnly`：明确「只覆盖本进程子树」，不是采样失败 |
| | `webviewDetached` | `bool` | 本平台 WebView 进程由系统托管、不属本应用子树（本期仅 macOS 为 `true`）。前端据此展示 §5.5 的平台托管说明，**不得**自行按平台名推断 |
| | `truncated` | `bool` | 是否因深度 / 条数上限截断 |
| | `hostThreadCount` | `u32 \| null` | 仅宿主；取不到为 `null`，禁止填 `0`（§5.3） |
| | `selfGroup` | `ProcessGroupDto` | 自身合计 |
| | `subtreeGroup` | `ProcessGroupDto` | 全树合计 |
| | `entries` | `ProcessEntryDto[]` | 树序、一行一进程、`pid` 不重复 |

| 类型 | 字段 | 类型 | 约束 |
|---|---|---|---|
| `ProcessGroupDto` | `pidCount` | `u32` | 组内进程数 |
| | `cpuPercent` | `f32 \| null` | 按 `cpuBasis` 求和；首拍 `null`，禁止填 `0` |
| | `memoryBytes` | `u64 \| null` | 按 `memoryBasis` 求和；取不到为 `null`，禁止填 `0` |
| | `memoryAvailability` | `ResourceAvailabilityDto` | 只取 `available` / `unavailable` |
| | `memoryReason` | `String \| null` | `unavailable` 时必须非空（§10.2 V14） |

可加性标志**不在此处**，上提到快照顶层：它们是**指标属性**，同一快照内所有组共享；放进组里会导致两个组的合计口径可被独立改写。

| 类型 | 字段 | 类型 | 约束 |
|---|---|---|---|
| `ProcessEntryDto` | `pid` | `u32` | |
| | `parentPid` | `u32` | 原值；前端不据此重建树（树序由后端出） |
| | `role` | `ProcessRoleDto` | 7 值见 §2.3；决定该行归入哪个合计；**全表可见** |
| | `exeName` | `String` | 开放集合，前端兜底不窄化；**不含命令行全文**（可能含凭据） |
| | `cpuPercent` | `f32 \| null` | 单核百分比；首拍 / 未取到为 `null` |
| | `memoryBytes` | `u64 \| null` | 按 `memoryBasis`；未取到为 `null` |
| | `startTime` | `i64` | 进程创建时刻；PID 复用校验与建父子链的必需字段，**必须**随附 |

**封闭枚举清单**（本期只有这些成员，无保留值、无未用成员）

| 枚举 | 成员 |
|---|---|
| `ResourceTabDto` | `storage` / `processTree` |
| `ResourceAvailabilityDto` | `available` / `unavailable` / `notImplemented` / `unreadable` |
| `ProcessRoleDto` | `host` / `webviewBrowser` / `webviewRenderer` / `webviewGpu` / `webviewUtility` / `webviewUnknown` / `otherChild` |
| `MemoryBasisDto` | `workingSet` / `privateWorkingSet` / `physFootprint` |
| `CpuBasisDto` | `singleCorePercent` |
| `CoverageDto` | `subtreeOnly` |

`ProcessSnapshotDto` **不含**任何任务名、任务状态、停止入口、峰值字段。

### 6.3 通用约束

1. **任何输出 DTO 的 `Option` 字段都不加 `skip_serializing_if`**：键恒在、值 `null`。「缺键」与「值为空」在前端是两种完全不同的语义。
2. 改 DTO 后必须跑 `npm run contract:gen` 并提交 `src/types/protocol.contract.ts`；**禁止**手改生成类型；**禁止**用 `Record<string, unknown>` 传部分字段。
3. 开放集合（如 `exeName`）保留 `String` 且前端兜底不窄化；封闭集合一律 `enum`。
4. **wire 不承载本地 UI 状态**（§2.7 约束 3）。

### 6.4 模块归属

| 层 | 落点 | 职责边界 |
|---|---|---|
| 命令层 | `src-tauri/src/cmd/resource.rs`（新增） | 只做能力声明与快照转发，**不实现采样**、不承载其它 tab 明细；注入 `std::process::id()` 作为根 PID，持有 `OnceLock<Mutex<CpuDiffCache>>`（§2.6） |
| 采样 crate | `src-tauri/crates/assistant-resource-probe`（新增） | 只依赖 OS API 与 `assistant-protocol`；**禁止**反向引用应用层（路径、根 PID 由调用方注入）。内部结构：`lib.rs`（导出面）/ `source.rs`（`SnapshotSource` trait 与 `RawProcess` 快照行）/ `win.rs`·`linux.rs`·`macos.rs`（`#[cfg(target_os = ...)]` 三平台后端）/ `classify.rs`（§2.3 三层角色判定）/ `tree.rs`（建父子表、DFS、PID 复用校验）/ `cpu.rs`（`CpuDiffCache`）/ `mock.rs`（测试用假快照） |
| 平台依赖 | 该 crate 的 `Cargo.toml` | 需要 `[target.'cfg(target_os = "linux")'.dependencies]` 与 `[target.'cfg(target_os = "macos")'.dependencies]` 两个段；Unix 侧共用项用 `cfg(any(target_os = "linux", target_os = "macos"))` 段。Windows 侧复用工作区 `windows` 依赖（根 manifest 已有该段，Linux / macOS 依赖一律下沉到各自 crate，不改根 manifest 的平台段） |
| DTO | `assistant-protocol/src/resource.rs`（新增） | 纯类型，不依赖任何内部 crate |
| 前端服务 | `src/services/desktop/resourceService.ts`（新增） | 与 `storageService.ts` **分开文件、互不引用** |
| 前端 UI | `src/settings/pages/resource/ProcessTreeTab.tsx`（新增） | 只消费 `get_process_snapshot` 与能力声明 |

---

## 7 装配点与前端落点

### 7.1 Rust 侧（八处，缺一即构建或契约失败）

| # | 文件 | 变更 |
|---|---|---|
| 1 | `src-tauri/Cargo.toml` | `[workspace] members` 加 `"crates/assistant-resource-probe"`（当前 14 个成员）；`[workspace.dependencies]` 加 `assistant-resource-probe = { path = "crates/assistant-resource-probe" }`；`[dependencies]` 加 `assistant-resource-probe = { workspace = true }` |
| 2 | `src-tauri/crates/assistant-protocol/src/lib.rs` | 加 `pub mod resource;` 与 `pub use resource::*;`，两处各一次：该文件 `pub mod` 组与 `pub use` 组分开且均为字母序，落点都是 `provider` 之后、`screenshot` 之前 |
| 3 | `src-tauri/crates/assistant-protocol/tests/protocol_bindings.rs` | **两处各加一次**：`use assistant_protocol::{...}` 导入清单、`export_protocol_contract` 的 `declarations` 数组。该测试会断言「源码中 `#[derive(TS)]` 的类型名集合」与「导入清单」一致，漏加会直接失败 |
| 4 | `src-tauri/src/cmd/mod.rs` | 加 `pub mod resource;` 与 `pub use resource::*;`，两处各一次：`mod` / `pub mod` 混排一组、`pub use` 一组，均为字母序，落点都是 `provider` 之后、`screenshot` 之前 |
| 5 | `src-tauri/src/ipc.rs` | `generate_handler![...]` 内加 `cmd::get_resource_capabilities, cmd::get_process_snapshot` |
| 6 | `src-tauri/crates/assistant-resource-probe/src/**` | 内部结构见 §6.4：`source.rs` 的 `SnapshotSource` trait + `RawProcess`、三平台 `win.rs` / `linux.rs` / `macos.rs`、纯逻辑 `classify.rs` / `tree.rs` / `cpu.rs`、测试用 `mock.rs`。**trait + mock 是交付物**，V8 / V10 / V11 / V12 依赖它 |
| 7 | `src-tauri/crates/assistant-resource-probe/Cargo.toml` | 按 §6.4「平台依赖」加 `[target.'cfg(target_os = "linux")']` / `[target.'cfg(target_os = "macos")']` 两个依赖段；Windows 复用工作区 `windows` |
| 8 | `src-tauri/Cargo.toml` 的 `windows` feature 清单 | 若某平台后端走手写（OQ1 未选 `sysinfo`），Windows 侧需追加进程枚举 feature（`Win32_System_ProcessStatus`、`Win32_System_Diagnostics_ToolHelp`）；新增 feature / 新增依赖一律**单独过审**后再改 |

**批次归属**：第 2 / 3 / 4 / 5 处随 **R0** 交付（命令骨架必须返回类型化响应，前端能力声明也要读生成类型）；第 1 / 6 / 7 / 8 处随 **R1a** 交付（crate 与平台后端）。

随后执行 `npm run contract:gen`（其 `contract:gen:protocol` 子命令跑上面第 3 处），产出 `src/types/protocol.contract.ts`；`npm run contract:verify` 必须通过。

**不新增 `[[bin]]`**：峰值推迟（§11），本期没有进程外二进制，因此不涉及「单 package 单产物」约束，也不预设 `src/bin/` 目录。

### 7.2 前端侧（七处）

| # | 文件 | 变更 |
|---|---|---|
| 1 | `src/settings/core/registry/builtinRegistry.ts` | 该页面项：`id` 改为 `resource`、`label` 键改为 `common.navigation.resource.title`、`keywords` 追加资源词（保留原存储词）；**categoryId / order / icon / emptyPage / load 不动**（icon 是否更换见 §12 OQ8，未定案前保持 `DatabaseOutlined`） |
| 2 | `src/i18n/locales/zh-CN.json`、`en-US.json` | `common.navigation.storage.title` **键名改为** `common.navigation.resource.title`，值为「资源监视器」/`Resource Monitor`；新增 `settings.resource.*`（键表见下）。**`settings.storage.*` 内容键一律保留** |
| 3 | `src/settings/pages/StorageSettings.tsx` | 改为容器：`SettingsPageShell wide` 唯一一次 + antd `Tabs`（受控写法：`activeKey` + `onChange` + 两个 `items`）；同时实现 §3.3 第 3 条的滚动记忆（`ref` → 向上取可滚动祖先 → 按 `activeKey` 存取 `scrollTop`） |
| 4 | `src/settings/pages/storage/StorageContent.tsx` | **去掉自带的 `SettingsPageShell`**（当前「加载/失败」与「正常」两个分支各包一次），改为只渲染 body（§3.3 第 1 条） |
| 5 | `src/settings/pages/resource/ProcessTreeTab.tsx`（新增） | 实时表 + 两个合计 + 宿主线程数 + 状态展示（§5） |
| 6 | `src/services/desktop/resourceService.ts`（新增） | `getResourceCapabilities()` / `getProcessSnapshot()`，沿用 `tauriInvoke` 封装 |
| 7 | `src/settings/__tests__/fixtures/snapshot.full.json`、`snapshot.vaultless.json` | 用 `vitest -u` 重生成；**只允许 `pages[].id` 由 `storage` → `resource` 一行变化**，`snapshot.diff.json` 不变（§10.2 V4） |

**`settings.resource.*` 最小键表**（本期只允许这些键，新增键须回写本节）：

| 键 | 用途 |
|---|---|
| `settings.resource.tab.storage` / `settings.resource.tab.processTree` | 两个 tab 标题 |
| `settings.resource.column.process` / `role` / `cpu` / `memory` | 四列表头 |
| `settings.resource.summary.self` / `summary.subtree` / `summary.hostThreads` | 两个合计 + 宿主线程数标签 |
| `settings.resource.status.notImplemented` / `unreadable` / `memoryUnavailable` / `truncated` / `cpuFirstSample` / `coverageSubtreeOnly` / `coveragePlatformManaged` / `capabilitiesLoadFailed` | §5.5 的八种展示状态 |
| `settings.resource.caliber.winPrivate` / `caliber.linuxWorkingSet` / `caliber.macFootprint` | §5.4 三种口径说明 |
| `settings.resource.reason.*` | 仅当 `reason` 需要本地化时使用；后端给英文原文时前端直接展示，不建键 |

**前端本地状态类型**（承载 §2.7 约束 3）：

```ts
type CapabilitiesView = ResourceCapabilitiesDto & { loadFailed?: boolean };
```

能力声明读取失败时只置 `loadFailed`，**不改 wire、不新增枚举取值、不默认判为可用**。

---

## 8 失败模式

| 失败模式 | 表现 | 必须的语义 |
|---|---|---|
| 任何 tab 取数失败 | 命令抛错或返回非 `available` | 只降级该 tab；**禁止**让一个 tab 的失败连坐另一个 |
| 采样阻塞 UI | 全系统快照耗时 | 异步执行；前端有 loading 与上次快照时刻 |
| 快照取不到 | OS 接口失败 | 顶层 `unreadable` + `reason`；保留上次成功快照并标注时间 |
| PID 复用 | 父进程死后 PID 被复用，而 `parentPid` 是创建时的快照值、不会更新 | 校验 `child.startTime >= parent.startTime` 且父 PID 在快照内；不通过则**排除并标注**，禁止静默并入 |
| 进程在采样中退出 | 遍历期间消失 | 该行跳过，不报错；`sampledAt` 之后才死的进程允许出现在快照里（快照是时刻值） |
| 子树遍历被截断 | 深度 / 条数上限触发 | `truncated = true` + UI 横幅；禁止当完整快照 |
| CPU 首拍 | 只刷新一次 | 输出 `null`；UI 显示「本拍无基准」，**禁止**错误态 |
| 新出现的行 | 刚 spawn 或 tab 刚打开 | 下一拍之前没有数字（不是 `0`）；UI 必须能渲染「待定」 |
| 内存指标不可得 | 平台无该口径 | 组 `memoryAvailability = unavailable` + 非空 `memoryReason`；**禁止**填 `0` |
| 命令行读不到 | 权限 / 平台不支持 / 读取失败 | 落 `webviewUnknown` 并**仍计入自身合计**；禁止降级成 `otherChild`，禁止丢弃该进程 |
| 能力声明读取失败 | 新命令异常 | 前端本地横幅 + 保留上次渲染；**禁止**默认判为可用 |
| 非目标 OS 构建 | 编译目标不在三平台内 | 能力声明与快照顶层均 `notImplemented` + `reason`，tab 显示原因；**禁止**编译期剔除页面、禁止显示空表 |

---

## 9 反模式

| 反模式 | 后果 | 替代 |
|---|---|---|
| 采样 crate 反向引用应用层 | 豁免被滥用，等于把应用层搬进 crate | crate 只依赖 OS API，路径与根 PID 由调用方注入（§6.4） |
| 把读不到显示成 `0` | 伪精确，用户据此做出错误决策 | 按 §2.7 分状态，数字位保持 `null` |
| 对每个后代各查一次系统接口 | O(N) 次系统调用，开销随时序恶化 | 一次全系统快照 + 内存父子表 + 从根 DFS（§2.2） |
| 为 `threadCount` 补发第二次系统枚举 | 破坏「一次快照」约束 | 取不到就 `null`；线程数只对宿主取（§5.3） |
| 按镜像名判定归属 | 同名镜像全机器共享，必然收进别的应用的进程 | 祖先链过滤，镜像名只作粗筛（§2.2 约束 3） |
| 只取直接子进程 | WebView2 的 renderer / GPU / utility 是孙进程，全被漏掉 | 递归到任意深度（§2.3） |
| 把「命令行读不到」降级成 `otherChild` | 整棵 WebView 子树被踢出自身合计，静默少报最核心的数字 | 落 `webviewUnknown` 并仍计入自身（§2.3） |
| 沿 `parentPid` 上溯但不校验创建时间 | PID 复用后把无关进程认成自己的子进程 | 校验 `startTime` 单调 + 父存在（§2.2 约束 4） |
| 把工作集求和当物理占用 | 共享页重复计数，合计偏高且无法解释 | 用私有工作集 / `phys_footprint`；不可加时合计位不显示数字（§5.4） |
| 「显示合计但加一句标注」 | 把不可加的数当合计用，标注救不回来 | `memoryAdditive == false` 时合计位不显示数字（§5.4 第 1 条） |
| 把两个合计相加 | 自身 ⊆ 全树，相加即重复计数 | 同时显示并标注包含关系（§5.2） |
| 同一进程在表里出现两次 | 数字互相打架，合计无法解释 | 一行一进程，`pid` 不重复（§5.1） |
| 前端按 `parentPid` 重建树 | 与后端树序冲突，且 `host` 行的 `parentPid` 不在表内 | 只渲染后端树序（§5.1） |
| 把宿主进程登记进任务表 | 停止自己 = 应用自杀 | 宿主禁止登记；停止按行禁用（§11） |
| 在进程内采样自己的峰值 | OOM / 卡死时那次采样必然丢失 | 本期不做峰值；将来只允许外部产物（§5.6） |
| 为推迟项留空块 / 灰按钮 / 永空读取器 | 用户以为功能坏了，且制造无人维护的死代码 | 不出现任何占位 UI，能力声明报 `notImplemented`（§1.2） |
| 后台常驻采样 / 启动预热采样 | 监视自身的功能自身成为最大开销，逻辑自相矛盾 | 纯拉取 + 缓存 TTL，过期即丢弃（§2.6） |
| 把采样器登记进启动分层的 `RESOURCES` 表（`Lane::Boot` 或 `Lane::Idle`） | 会被 `boot::run_boot` 或 `boot::runner::run_idle` 以 `ensure` 建一次并常驻，违反「无人看即停表」 | 不登记，只留命令层缓存（§2.6） |
| 为「开始 / 停止采样」增设命令 | 停表路径依赖前端自觉，反而更易泄漏 | 停表 = 没有调用；无常驻线程可停（§2.6） |
| 能力声明与数字塞进同一响应 | 必须引入跨 tab 聚合响应，一个 tab 失败连坐另一个 | 能力声明独立成命令（§6.1） |
| 把能力声明做成 map | 前端要猜键名，新增 tab 静默不生效 | 平铺数组 + `tab` 判别字段（§6.2） |
| 两个 tab 各自渲染 `SettingsPageShell` | 壳套壳：padding 双算、sticky 变量作用域嵌套、标题重复 | 容器渲染唯一一次壳，tab 只渲染 body（§3.3） |
| 手改 golden fixture 让测试变绿 | fixture 是投影的**输出**，手改等于把断言改成同义反复，之后任何结构回归都不会被发现 | 只允许 `vitest -u` 重新生成，并人工核对 diff（§10.2 V4） |
| 为旧 id 加 redirect / 别名路由 | `isSettingsPageId()` 只认 manifest id，加别名只能再登记一项，等于多出第二个导航项 | id 直接改名，路由随 id 走，不留兼容层（§3.1） |
| 页面改名后把文件 / CSS 类 / 日志标签一起改 | 这些指「存储这块内容」，与页面 id 无关；跟着改名扩大改动面且无收益 | 只改 id、label 键、keywords（§3.1） |
| 前端用字符串兜底匹配可用性 | 新增取值静默走默认分支，错误被吞掉 | 封闭枚举 + 编译期穷尽（§2.7 约束 1） |
| 为 tab 新建内层滚动容器 | 改变 sticky 基准，表头吸顶位置偏移；且与「壳只有一个」重复 | 复用 `AppShell` 的 `Layout.Content`，在容器层存取 `scrollTop`（§3.3 第 3 条） |
| 三平台后端一次性合入 | 三套编译差异压在同一个批次，任一处失败拖累全部 | 按平台拆 R1a / R1b / R1c，每批单独可编译（§10.1） |
| 批次留中间态（有表无合计 / 悬空 invoke） | 合入即半成品，验证不可执行 | R0 内含命令骨架、R2 一次性齐备（§0.13、§10.1） |
| 用 `AppHandle::state` / `boot::access.rs` 访问器持有 CPU 缓存 | 对象随启动期装配即存在，违反「页面未打开时不执行采样代码」 | `cmd/resource.rs` 内 `OnceLock` 惰性初始化（§2.6） |

---

## 10 批次与验收

### 10.1 批次

**批次原子性（硬约束）**：每个批次必须满足「单独合入即可运行、可验证」，不允许留中间态。

| 批次 | 内容 | 依赖 |
|---|---|---|
| **R0** 容器与契约（原子） | 页面登记（id / label 键 / keywords）、i18n（含 `settings.resource.*` 最小键表）、`Tabs` 壳、`StorageContent` 去壳、滚动记忆、**11 个 DTO + §7.1 第 2/3/4/5 处装配 + `contract:gen`**、**两条命令骨架（`cmd/resource.rs` 返回 `notImplemented` + 装配进 `ipc.rs`）**、能力声明读取接入、进程树 tab 按 §5.5 的 `notImplemented` 分支渲染（非空占位） | 无（可立即开工） |
| **R1a** 采样 crate 与 Windows 后端 | 新 crate（`SnapshotSource` trait + mock + `classify` / `tree` / `cpu` 纯逻辑）+ `win.rs` + §7.1 第 1/6/7/8 处装配 + 两条命令骨架换成真实调用 + V5–V12 单测 | R0 的 DTO 与命令骨架；Windows 后端选型（§12 OQ1） |
| **R1b** Linux 后端 | `linux.rs` + Linux 段依赖 + `memoryAdditive == false` 路径的单测与真机验证（V14） | R1a 的 trait 定型（DTO 已在 R0） |
| **R1c** macOS 后端 | `macos.rs` + macOS 段依赖 + `phys_footprint` 口径单测与真机验证 | R1a 的 trait 定型（DTO 已在 R0） |
| **R2** 实时树 UI（原子） | 表 + 两个合计 + 宿主线程数 + 口径分支 + §5.5 全部状态展示，**一次性齐备** | R0 的 DTO 与命令骨架 |

R1b / R1c 可与 R2 并行；R2 不等 R1b / R1c，但要求能力声明在未完成平台上如实报 `notImplemented`（§2.7）。

**平台验证分工（依据仓库现状，不是本文新定）**：

| 平台 | 编译 / 测试在哪跑 | 依据 |
|---|---|---|
| Linux | CI 真机（`ubuntu-latest`），跑 `cargo test --workspace --all-targets` | `.github/workflows/ci.yml` 的 rust job 矩阵 |
| macOS | CI 真机（`macos-latest`），同一 job | 同上 |
| Windows | **开发机**（`cargo check` + `cargo test`），不进 CI 矩阵 | 同上文件头「开发机只有 Windows」的既定分工 |

因此 R1b / R1c 的编译与纯逻辑单测由既有 CI 天然覆盖，**不需要**在本文另加交叉编译门；Windows 侧的风险点是 `windows` crate 的 feature 缺项（§7.1 第 8 条），在开发机首次编译即暴露。依赖 GUI / 系统弹窗的路径（如 macOS Touch ID、Linux zenity）CI 验不到，但本规格不涉及这类路径。

**交付边界**：本节的「三平台交付」指**代码与能力层**。三平台安装包属打包发布立项（`ARCHITECTURE.md` 的 I8，当前未开始），本文不承诺产物。

### 10.2 验收判据

| # | 判据 | 类型 |
|---|---|---|
| V1 | 页面 **id 为 `resource`**、路由 `/settings/resource`，label 显示「资源监视器」，左侧导航仍是**一项** | 手工 |
| V2 | 页内两个 tab 可切换；切换**不重建**壳；滚动位置按 tab 独立（机制见 §7.2 第 3 条） | 手工 |
| V3 | tab 栏与表头共用 `--storage-sticky-top`，代码中**不存在**第二个偏移常量 | 代码审查 |
| V4 | 三份 golden fixture 用 `vitest -u` 重生成后，`snapshot.full.json` / `snapshot.vaultless.json` 相对旧版**只有 `pages[].id` 由 `storage` → `resource` 一行变化**，`snapshot.diff.json` 无变化；diff 需人工核对。理由：投影含 `id`，id 改名必然反映到 fixture；只允许这一处变化，其余变化一律视为结构回归 | 单测（`settingsSnapshot.golden.test.ts`） |
| V5 | `get_resource_capabilities` 响应**不含任何数字字段** | 契约审查 |
| V6 | `tabs` 恒含两项、`tab` 不重复，`storage` 恒 `available` | 单测 |
| V7 | 同一响应内 `selfGroup` / `subtreeGroup` / `entries` 与 `sampledAt` 同源（一次采样只产一个时间戳） | 单测 |
| V8 | 孙进程出现在 `entries` 且 `role` 正确（构造 root → child → grandchild） | 单测 |
| V9 | `entries` 中 `pid` 不重复 | 单测 |
| V10 | 同名镜像但不在子树内的进程**必须被排除** | 单测 |
| V11 | PID 复用防护：`startTime` 倒挂或父 PID 不在快照内时，该进程被排除并标注 | 单测 |
| V12 | 只发起**一次**全系统快照（mock 计数为 1，不随进程数增长） | 单测 |
| V13 | 首拍 CPU 为 `null` 而非 `0`；UI 渲染「本拍无基准」而非错误态 | 单测 + 手工 |
| V14 | `memoryAdditive == false` 时：合计位**无数字**、`memoryAvailability == unavailable`、`memoryReason` 非空、逐行数字照常显示 | 单测 + 手工 |
| V15 | 内存列名由 `memoryBasis` 驱动，三个取值各有对应文案 | 手工 |
| V16 | `hostThreadCount` 只描述宿主；`ProcessEntryDto` **无** `threadCount` 字段 | 契约审查 |
| V17 | 页面内**无**峰值区块、无停止按钮、无任务名列、无任何推迟项占位 UI | 手工 |
| V18 | `npm run contract:gen` 产出含全部 11 个新类型；`npm run contract:verify` 通过 | 命令 |
| V19 | 能力声明读取失败时：wire 不变、无新增枚举取值、保留上次渲染、仅本地横幅 | 单测 + 代码审查 |
| V20 | 平台后端未定案时，`processTree` 能力为 `notImplemented` 且 tab 显示原因而非空表 | 单测 + 手工 |
| V21 | **R0 原子性**：R0 合入后单独启动应用，进程树 tab 不报「命令不存在」，而是按 §5.5 渲染 `notImplemented` 文案 + `reason`；`get_resource_capabilities` / `get_process_snapshot` 均在 `ipc.rs` 注册 | 手工 + 代码审查 |
| V22 | **三平台可编译**：Linux / macOS 由既有 CI 矩阵（`ubuntu-latest` / `macos-latest`）的 `cargo test --workspace --all-targets` 覆盖；Windows 由开发机 `cargo check` + `cargo test` 覆盖。本文**不新增**交叉编译 job | 命令（CI + 开发机） |
| V23 | **三平台口径各一**：Windows 私有工作集、Linux 工作集（`memoryAdditive == false`）、macOS `phys_footprint` 各取一次真机快照，`memoryBasis` / `memoryAdditive` 与 §2.4 一致。执行位置按 §10.1「平台验证分工」：Linux / macOS 走 CI 真机（可用 `#[cfg]` 门控的实机用例），Windows 走开发机。macOS 还需确认 WebView 进程不在子树内时如实展示平台托管说明（§2.3） | 手工 / 实机用例 |
| V24 | **滚动记忆不改壳**：实现后 `AppShell` 无 diff、无新增内层滚动容器，表头吸顶位置与 R0 前一致 | 代码审查 + 手工 |
| V25 | **`notImplemented` 产出条件收窄**：仅在非三平台目标构建或该平台后端未定案时出现；三平台齐备后，正常构建中该取值不出现在 `processTree` 能力上 | 单测 + 代码审查 |
| V26 | **`webviewDetached` 按平台确定**：macOS 为 `true`、Windows / Linux 为 `false`；为 `true` 时 UI 必现平台托管说明，且 WebView 家族进程**不**因镜像名被收进 `entries` | 单测 + 手工 |

---

## 11 推迟项登记

三项推迟项**都不在本期任何批次内**，且本期不为其写占位 UI。每项都写清「阻塞原因」与「将来落点」，避免将来重新论证。

### 11.1 峰值与热点

| 项 | 内容 |
|---|---|
| 需求 | 峰值内存、峰值 CPU、CPU 热点（历史最坏情况，而非当前值） |
| 阻塞原因 | 证据自指（§2.1）：进程内采样在 OOM / 卡死时必然丢失最后数据；因此写端必须是**独立进程**，而独立二进制受「单 package 单产物」约束不能进 bundle，分发路径（随诊断包 / 手动获取）未定 |
| 将来形态 | 外部产物 + 只读报告；与实时树**分块渲染**；必须带产物生成时刻与「历史快照非当前值」标注；产物损坏按 `unreadable` 处理，**禁止**静默退回上次成功值；产物需带 schema 版本号（写端与读端版本可能不一致，无版本号时格式变更会静默解析失败） |
| 落点 | 另行立项；需先定「写端如何分发」 |

### 11.2 停止能力（行级）

| 项 | 内容 |
|---|---|
| 需求 | 表内按行停止某个子进程 |
| **不阻塞于登记表** | 停止只需要**句柄 + 统一回收抽象**，与「这个进程属于哪个任务」无关 |
| 阻塞原因 | 缺两处代码工作：① MCP server 未纳入任何回收组（当前裸 `Command::spawn` + `kill_on_drop`，而 `kill_on_drop` 只杀直接子进程，其孙进程变孤儿，同一份「子进程」存在两种回收语义）；② 句柄未暴露成标识（按 PID kill 在复用后会杀错进程，需句柄化：Windows 持 Process/Job handle，Linux 用 `pidfd`，macOS 无 `pidfd` 只能 `kill(pgid)` + `startTime` 校验） |
| 已有基础 | Windows Job Object（`AssignProcessToJobObject` / `TerminateJobObject` / `ActiveProcesses`）、Unix `setsid(2)` + `kill(-pgid)`（幂等，ESRCH 忽略）、可克隆跨线程的 `KillHandle` |
| 将来约束 | 停止**按行**决定，**禁止**按 tab：`role ∈ {host, webview*}` 的行**恒 disabled** 并注明「停止自己 = 应用自杀」。宿主**禁止**进回收组——Linux 下宿主与未 `setsid` 的子进程同 pgid，`kill(-pgid)` 会杀掉宿主自己 |
| 归属 | 监视器**只能请求**，真实实现复用既有回收机制（shell 的 `kill_tree` / Job Object），**禁止**另建第二套回收 |

### 11.3 任务级归因（需求登记，非接口约定）

**本节只登记需求**，不主张任何登记表的所有权，也不对未立项的设计下命令。

| 项 | 内容 |
|---|---|
| 需求 | 进程树里多一列「属于哪个后台任务」，并能显示「排队中（尚无 PID）」的任务 |
| 阻塞原因 | 需进程登记表 + 身份代际。没有身份代际时，重启或停止一次之后归因必然错乱，**错归因比没有归因更危险** |
| 本 tab 只消费三样 | ① `pid + startTime` 作为 join key；② `ownerTaskId → Task.title` 用于任务名列；③ `state == queued` 用于显示无 PID 的行。其余字段（`kill` / `groupKind` / `generation`）本 tab **一律不消费** |
| 边界 | OS 级数字（存在 / CPU / 内存 / 角色）**不依赖**登记表；只有「属于哪个任务」与「排队中」依赖。因此本期能出树级数字，缺的是 `Task`，不是数字 |
| 登记表若要立项，必须满足 | **两段式登记**：spawn 前先登记占位（reserve），拿到 PID 后补写 `pid + startTime`，消除「先有 PID 后登记」的空窗；**`Task` / `TaskGroup` 拆分**：`Task` = 一次可归属的执行单元，`TaskGroup` = 同一逻辑任务的多进程集合；**防环**：`Task` 不得归属自身，`TaskGroup` 不得把自己列为成员；**宿主禁止登记**（§2.1 动作自指） |
| 将来立项时的前瞻断言 | 每个 `pid + startTime` 最多属于一个 `Task`；`Task` 记录的 `pid` 必须能在快照内找到（否则为陈旧记录）；`state == queued` 的 `Task` 不得有 `pid` |
| 归属 | 由该登记表所属的架构决策记录定案；本文**不建第二套登记表** |

---

## 12 开放问题

只列**不影响本期闭环**的问题：每条都给出「未定案时的确定行为」，因此不阻塞开工。

| # | 问题 | 未定案时的确定行为 |
|---|---|---|
| OQ1 | 平台后端选 `sysinfo` 还是手写 API（**三平台各自独立定案**） | **仓库当前没有 `sysinfo`**（`Cargo.lock` 里的 `procfs-core` 是 `minidump-writer` 的传递依赖，不可直接当采样后端）。优先 `sysinfo`（一套代码覆盖三平台），前置条件：单次遍历即能同时给出 `pid` / `parentPid` / `startTime` / 内存 / CPU 累计时间 / `threadCount`，且新增依赖过审。不满足则该平台改手写：Windows `NtQuerySystemInformation`（需补 `Win32_System_ProcessStatus`、`Win32_System_Diagnostics_ToolHelp` feature，现有 feature 清单无进程枚举项）、Linux 遍历 `/proc`（可复用现有 `libc`）、macOS `sysctl` + `proc_pidinfo`。某平台未定案时，该平台的 `processTree` 能力报 `notImplemented`，tab 显示原因（§2.7、§10.2 V25）。新增依赖 / 新增 feature 一律单独过审 |
| OQ2 | 角色分类的数据源细节 | 判据已定（§2.3 三层），来源随 OQ1：Windows 读命令行、Linux 读 `/proc/<pid>/cmdline`、macOS 需 `sysctl`。**必须**有「读不到命令行」的降级角色 `webviewUnknown` 并仍计入自身 |
| OQ3 | 将来是否扩展为「全机视图」 | 本期不做。若扩展，必须重新引入**按需指标机制**（原位掩码）与隐私边界（会看到其他应用的进程名）；只有出现**第二个消费者**时才引入按需指标，单 tab 单消费者下它只增加复杂度 |
| OQ4 | 采样 crate 的平台相关单测是否进 CI | 跨平台单测只覆盖纯逻辑（建父子表、DFS、角色分类、PID 复用校验）；OS 调用层用 trait 隔离，CI 跑 mock 实现。CI 矩阵已含 `ubuntu-latest` + `macos-latest`（`cargo test --workspace --all-targets`），新增 crate 自动纳入，无需另加 job；Windows 由开发机验证（§10.1、§10.2 V22） |
| OQ5 | 是否与「工具运行面板」共享组件 | 不共享。工具运行态已在聊天卡片侧交付，两者行模型不同（任务 vs 进程）；强行共用会把 `Task` 语义漏进本 tab |
| OQ6 | WebView 角色是否可用 WebView profile 目录作辅助判据 | 可作辅助，**不得**作主判据：目录归属依赖外部布局，角色分类必须自足于命令行特征 |
| OQ7 | 能力声明是否需要缓存 | 不需要。只返回状态，成本可忽略；缓存反而引入「状态陈旧」这类新问题 |
| OQ8 | 页面 icon 是否继续用 `DatabaseOutlined` | 未定案前**保持** `DatabaseOutlined` 不改（§3.1、§7.2 第 1 条）；改 icon 属纯视觉调整，不阻塞任何批次 |
| OQ9 | macOS 的 WebView 进程由 XPC 托管、不在子树内，是否引入 XPC 归属辅助 | 本期**不引入**：按 §2.3「平台可达性差异」如实展示 `coverage` 说明，`selfGroup` 只含宿主。引入 XPC 归属必须同时给出「如何证明该 XPC 进程属于本应用」的判据，否则等于按镜像名归属（违反 §2.2 约束 3） |
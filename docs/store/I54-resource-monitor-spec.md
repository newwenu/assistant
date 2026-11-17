# I54 — 资源监视器规格（存储 / 进程 / 自身占用）

| 项 | 值 |
|---|---|
| 状态 | 草稿（观测层规格；不含回收动作与策略治理） |
| 日期 | 2026-11-15 |
| 关联 | [STORAGE-MONITOR-ANALYSIS](./../analysis/STORAGE-MONITOR-ANALYSIS.md)（存储观测机制的唯一事实基线）、[TASK-RESOURCE-MANAGER-ANALYSIS](./../analysis/TASK-RESOURCE-MANAGER-ANALYSIS.md)（进程与自身占用；其 §8 是自监控的唯一来源）、I44 应用目录管理规格（已归档）（目录归属与字面量门禁）、I53（已归档）（沙箱准备；进程树回收的判据来源）、feature-mount-traversal-spec（已归档）（可选能力挂载边界）、[CODE-SPLIT-PRINCIPLE](./../principle/CODE-SPLIT-PRINCIPLE.md)（拆分原则唯一来源，本文只引用不复述）。参考实现：Chromium（浏览器内核参考项目，按需稀疏检出、不在仓内；本文取其任务管理器与进程度量两处形态），本文中标注「参考实现」的形态判断均出自该处 |
| 影响范围 | 前端：`src/settings/pages/StorageSettings.tsx`（改造为容器）、`src/settings/core/builtinRegistry.ts`（页面 label 与 keywords）、`src/services/storageService.ts`、i18n 两份 locale、`src/settings/__tests__/fixtures/*.json`（golden 投影）。后端：`src-tauri/src/cmd/storage.rs`（存储 tab 明细）、新增 `src-tauri/src/cmd/resource.rs`（能力声明与两条转发）、`src-tauri/src/protocol/dto/storage.rs`（DTO 扩展）、`src-tauri/src/ipc.rs`（命令注册）、新增 `src-tauri/crates/assistant-resource-probe/`（进程内采样缝，不依赖 `dirs`）、新增 `src-tauri/src/bin/resource_report.rs`（进程外诊断二进制，不进 bundle）、`src-tauri/crates/assistant-crash/`（补 stats 与保留上限，属该 crate 自身欠账）。进程与自身占用**本期交付树级数字与当前值，不交付任务级归因**（§5.4） |

> 一句话：本规格把现有「存储管理」设置页升级为**资源监视器**，升级的不是能力而是**结构**——存储 tab 走「各 owner 报数」，进程与自身占用 tab 走「**一个进程内采样缝、一个根、一棵子树、两种投影**」；任何 tab 都不许用 `0` 冒充「读不到」，也不许把不同单位的数相加成一个「总占用」，更不许把不可加的量求和当作总量。

本文中的「必须 / 禁止」为可验收约束；「应当」为默认做法，偏离需在代码注释里写明理由。

**编号约定**：`D1–D6` 是**观测方向**（存储侧），`P1–P5` 是受管进程侧方向，`S1–S4` 是自身占用侧方向，`N1–N16` 是现状事实编号。方向编号与 [STORAGE-MONITOR-ANALYSIS](./../analysis/STORAGE-MONITOR-ANALYSIS.md) §4 / §7 对齐，不另立一套。

---

## 1 目标与非目标

### 1.1 现状（以代码为准）

| # | 事实 | 落点 |
|---|---|---|
| N1 | 设置页 id 为 `storage`，label 键 `common.navigation.storage.title`（当前文案「存储管理」），`categoryId: "general"`、`order: 60`、`icon: DatabaseOutlined` | `src/settings/core/builtinRegistry.ts` |
| N2 | 该页组件只消费 `get_storage_breakdown` 一个命令，UI 只有「附件总量 + 孤儿分组 + 按会话分组 + 删除选中」 | `src/settings/pages/StorageSettings.tsx` |
| N3 | `StorageBreakdownDto` 只有 4 个字段：`totalBytes` / `totalCount` / `orphans` / `conversations`，无任何目录级、时间级、可用性级字段 | `src-tauri/src/protocol/dto/storage.rs` |
| N4 | `get_storage_breakdown` 用「磁盘集合 − `attachment_refs` 引用集合」判孤儿，按会话分组用 `SELECT DISTINCT conversation_id, attachment_id` | `src-tauri/src/cmd/storage.rs` |
| N5 | `list_all` 只取第一层 `is_file()`，`derived/` 因是目录被静默跳过；`AttachmentEntry.created_at` 的取值是 `metadata.modified()` | `src-tauri/crates/assistant-storage/src/attachment_store.rs` |
| N6 | `AppDirs` 只暴露 6 个无条件入口（`database` / `attachments` / `tool_results` / `mascot_assets` / `logs` / `crashes`），`dir_name.rs` 是这 6 个名字的唯一字面量 | `src-tauri/src/app_dirs/roots.rs`、`dir_name.rs` |
| N7 | 安全域目录（`security.db` / `vault/` / `recovery/` / `backups/` / `audit/`）的字面量自持于可选能力模块，`feature_mounts.rs` 的挂载表当前只有 `vault` 一项 | `src-tauri/src/cmd/vault/mod.rs`、`feature_mounts.rs` |
| N8 | 全仓**无进程登记表**：无 PID 登记、无 `JoinSet`、无进程组；`spawn_screen_inspection` 是唯一的固定间隔线程，且为 UI 逻辑（间隔为常量、无失败语义） | `src-tauri/src/setup.rs` |
| N9 | 全仓**无自身占用采样**：无 `sysinfo`，无 `GetProcessMemoryInfo`，无 `/proc/self` 读取 | `src-tauri/Cargo.toml` |
| N10 | `windows` crate 启用 22 个特性（其中 18 个 `Win32_*`），**含** `Win32_System_Threading`（`GetProcessTimes` 可用），**不含** `Win32_System_ProcessStatus`（`GetProcessMemoryInfo` 不可用） | `src-tauri/Cargo.toml` |
| N11 | 单实例已启用（`tauri_plugin_single_instance::init`）；日志轮转为 `KeepSome(3)` + 10 MiB + `Rotate` | `src-tauri/src/plugins.rs` |
| N12 | 设置页有 snapshot golden 投影（`full` 与 `vaultless` 两份 fixture），页面 label / keywords / order 变更会使该测试失败 | `src/settings/__tests__/` |
| N13 | 已有独立 crate `assistant-crash`，其依赖只有 `crash-handler` / `minidump-writer` / `chrono`（**无 `dirs`**），对外只暴露 `install(Option<PathBuf>)`；路径由 `main.rs` 调 `AppDirs::resolve()` 后注入 | `src-tauri/crates/assistant-crash/`、`src-tauri/src/main.rs` |
| N14 | `assistant-crash` 无 `list` / `stat` / `prune` 任何只读或清理入口，且每次事件写 `panic_<ts>.log` / `crash_<ts>.dmp` / `crash_<ts>.log` 三个文件，**无轮转、无单文件上限、无保留数量**；`MAX_BACKUPS` 只存在于 `assistant-vault/src/security_db/mod.rs`，全仓无任何清理 `crashes/` 的代码 | 同上 |
| N15 | `assistant_crash::install` 在 `assistant_app::run()` **之前**、同一进程内调用；`install_crash_handler` 的回调在异常处理上下文内做 `fs::write` 与 minidump 写入 | `src-tauri/src/main.rs`、`crash_dump.rs` |
| N16 | 仓库已有「独立二进制但不进 Tauri bundle」的先例：`dev_mcp_server` 以显式 `[[bin]]` 声明，注释写明「仅测试用，不进 Tauri bundle」 | `src-tauri/Cargo.toml` |

### 1.2 目标

1. 把设置页从「附件清理」升级为**多 tab 资源监视器**，页面身份从「存储管理」改为「资源监视器」；
2. 三个 tab 各自由**自己的取数缝**供数：存储 tab 走「各 owner 报数」，进程 tab 与自身占用 tab 共用**同一个进程内采样缝**（根 = 自身 PID，一次全系统快照推导出整棵子树，§2.3）并投影成两种视图（§6.2）；三者不共用回收语义、不共用停止语义；
3. 存储 tab 在 v1 交付 D1–D4 四个方向（体量 / 时间 / 分层归属 / 可用性状态）；
4. 进程 tab 与自身占用 tab 在 v1 交付**采样缝 + 树级数字 + 口径定案**，数字只到「树级、无任务归因」的粒度（§5.4、§6.6）；
5. 任何数字必须携带「来源 owner 或采样器、口径、采样时刻、可用性状态」四件套；
6. 任何读不到的对象显示**状态与原因**，禁止显示 `0`。

### 1.3 非目标

- **不做跨 tab 汇总**：磁盘字节与内存字节不是同一维度，相加无意义（§3.4）；
- 不做回收动作：`delete_attachments` 是既有能力，本期不新增任何删除入口（§4.2 末条）；
- 不做资格（D5）与策略（D6）：二者是回收的下游，本期无回收即无落点（§10 第 1 项）；
- 不做进程登记表、不做任务级归因（§5.2）；
- 不做「峰值」的自采样实现：峰值只能由外部留痕（§6.5 约束 2）；
- **不新建承载「存储观测」的 crate**：存储侧必须放应用层，因为它需要 `AppDirs` 与各 owner 的 stats（同 [STORAGE-MONITOR-ANALYSIS](./../analysis/STORAGE-MONITOR-ANALYSIS.md) §10 第 1 条）；
- **允许新建承载「自身与子树采样」的 crate**：它只依赖 OS API、不读 `dirs`、路径与根 PID 由调用方注入（§6.4）；这条豁免的边界是「不得引用应用层任何模块」，越界即退回上一条；
- 不自持目录清单：路径只从 `AppDirs` 与可选能力模块的 getter 来；
- 不改 `get_storage_breakdown` 的既有 wire 语义：新增字段不改变 `totalBytes` / `totalCount` / `orphans` / `conversations` 的既有含义（§4.4 的账本闭合以新增字段实现，不重定义旧字段）。

---

## 2 术语与分层（唯一出处）

### 2.1 三个身份

| 身份 | 职责 | 谁来做 | 监视器可否越界 |
|---|---|---|---|
| **所有者** | 决定目录名、拼基根、建目录 | `app_dirs`（无条件）、可选能力模块（如 `cmd/vault`）、各库（库内子布局） | **不能**，只能调用 getter |
| **观测者** | 读字节、条数、时间 | 各 owner 自己提供的只读方法 | 能，且只走方法 |
| **回收者** | 删除、轮转、清空 | owner 的既有方法 | 只能**请求**，不能自己 `remove_*` |

这条分工对应 I44（已归档）的两条门禁：目录字面量只允许命中 `app_dirs::dir_name` 与可选能力模块；`dirs::` 只允许出现在 `app_dirs` 与 `assistant-config`。监视器一旦自己 `read_dir` + `join`，两条门禁同时失效。

### 2.2 行与列：对象 ≠ 方向

这是本规格最容易混淆的一处，必须先分清，否则「几个监视方向」会算错：

- **观测对象**是**行**：`attachments/`、`tool-results/`、`security/audit/` 等，共 12 行（§4.1）；
- **观测方向**是**列**：体量、时间、归属、状态、资格、策略，共 6 列（§4.2 只做前 4 列）；
- 一个「tab」是**一组行 + 一组合法列**，不是一根方向轴。

### 2.3 两条取数缝，不是三套采样器

| 缝 | 观测对象 | 数据源 | 失败模式 |
|---|---|---|---|
| **缝 A：owner 报数** | 有稳定路径的目录 / 文件 | 文件系统 + 各 owner 的 stats | 单项不可读 → 该行标状态，整账仍返回 |
| **缝 B：进程内子树采样** | 自身进程 + 由父子链推导出的全部后代 | OS 的**一次全系统进程快照**（按 `root = 自身 PID` 在内存里过滤出子树） | 采样器与被观测者同生共死；PID 复用会误归属 |

缝 B 只有**一个采样器、一个根、一次遍历**，输出**两种投影**：

```
缝 B（进程内，root = 自身 PID）
   ├─ ① 一次全系统进程快照（单次系统调用，不是逐 PID 查询）
   ├─ ② 在内存里按 PID 建表、按 parent_pid 建父子链
   ├─ ③ 从 root 做一次 DFS → 得到整棵子树
   └─ ④ 按角色切成两种投影
        ├─ 投影 A：宿主 + WebView 角色进程（按 role 细分） → 「自身占用」tab
        └─ 投影 B：其余全部后代进程 → 「进程」tab
```

**「一次遍历」的准确含义是一次全系统快照 + 内存建父子表 + 从根 DFS**（Chromium 的 `SharedSampler` 即此形态：一次 `NtQuerySystemInformation(SystemProcessInformation)` 拿全表，再在内存里过滤与分组）。

**约束（实现形态）**：

1. **禁止**「对每个后代各查一次系统接口」的逐 PID 递归查询——那是 O(N) 次系统调用，而一次快照是 O(1) 次；
2. 既然一次快照必然看到不属于本应用的进程，**过滤是采样的必需步骤而非优化**；
3. 过滤**必须按祖先链**（从 root 沿父子链可达），**镜像名只能作前置粗筛**——`msedgewebview2.exe` 是全机器共享的镜像名，只按名字过滤必然收进别的应用的进程；
4. 快照里必须**同时**取到 `parent_pid` 与 `start_time`，否则第 ② 步无法建链、§9 的 PID 复用校验也无从执行。

**关键约束**：两个 tab **必须共用同一次采样结果**，禁止各起一个采样线程、各用一套间隔。共用的理由不是省资源，而是**两个视图的数字必须能互相解释**——用户看到「自身 300 MB、子进程 200 MB」时，这两个数来自同一时刻的同一次遍历；若分两次采样，两者不可比较。

**仍然不共用的是**：回收语义与停止语义。宿主进程**禁止**登记进受管任务表（§6.1），因为「停止自己 = 应用自杀」这条自指约束不会因为共用采样器而消失。缝 A 与缝 B 也只共用「投影通道（快照 vs 事件）」与「DTO 约束」。

### 2.4 可用性状态词表（封闭）

| 状态 | 含义 | 典型来源 |
|---|---|---|
| `available` | 能出数 | owner 的 stats 正常返回 |
| `missing` | 目录/文件不存在 | 首次运行、旧位置迁移后；**计 0，不报错，不创建** |
| `locked` | 存在但受保护读不到 | `security/` 未解锁 |
| `unreadable` | 存在但不可读 | 权限不足、路径异常 |
| `notImplemented` | owner 尚无 stats 入口 | `tool-results/`、安全域各子目录、`logs/`、`crashes/` |
| `noReportYet` | 外部诊断产物尚不存在 | 进程外报告从未生成过 |
| `staleReport` | 产物存在但已过期 | 报告生成时刻早于陈旧阈值 |

约束：

1. `missing` 与 `locked` **禁止**合并为 `0`：前者是「确实没有」，后者是「有但看不见」，用户对两者的下一步动作完全不同；
2. `notImplemented` 必须带原因（缺少哪个 owner 的哪个方法），否则用户会把「未实现」误读为「系统认为没问题」；
3. 该词表是**封闭集合**，前端按枚举分支渲染，不做字符串兜底（与开放集合的处理相反）；
4. `noReportYet` 与 `staleReport` 是**外部产物类**状态，**必须**随附产物生成时刻；只有 `staleReport` 允许展示产物里的数字，且必须同时显示「生成于 X 之前」——外部产物是历史快照，不是当前值。

---

## 3 tab 划分

### 3.1 tab 表

| tab | 取数缝 | v1 交付 | 不可用时的显示 |
|---|---|---|---|
| **存储** | 缝 A（owner 报数） | **数字**（12 行 × D1–D4） | —（v1 恒可用） |
| **进程** | 缝 B 投影 B | **树级数字**（存在 / CPU / 内存 / `exeName`），**无任务归因、无角色列**；内存列名必须带口径（§6.3.2 约束 3） | `notImplemented` + 原因（平台方案未定案，§10 第 8 项） |
| **自身占用** | 缝 B 投影 A + 外部报告 | **当前值**（宿主 + WebView）**+ 峰值区为报告阅读器** | 当前值 `notImplemented`；峰值区 `noReportYet` / `staleReport`（§6.6） |

**三个 tab 的数字不可互相替代，但后两个来自同一次采样**：进程 tab 的数字与自身占用 tab 的当前值必须出自同一 `sampledAt`（§2.3）。存储 tab 与它们完全独立。

### 3.2 为什么是页内 tab，不是三个设置页

必须做成**一个设置页内的三个 tab**，而不是在 `builtinRegistry` 注册三个 page：

| 方案 | 后果 |
|---|---|
| 注册三个 page（`storage` / `processes` / `self-usage`） | 左侧导航出现三项，用户需要「先选类别再选 tab」两次点击；且 `settingsSnapshot` 的 golden 投影整体改写（N12），三条新路由与三份空页面同时产生 |
| **一个 page 内三个 tab**（采用） | 左侧导航仍是「资源监视器」一项；golden 投影只需改一个 label 与 keywords；tab 之间切换不重新挂载页面壳 |

实现形态用 antd `Tabs`，仓库已有先例（`src/vault/settings/VaultSettings.tsx` 的 `activeKey` + `Tabs` 用法），不引入新组件库。

### 3.3 页面身份变更

| 项 | 现值 | 目标值 | 约束 |
|---|---|---|---|
| 页面 label | `common.navigation.storage.title` = 「存储管理」 | 文案改为「资源监视器」 | **i18n 键名保留**，只改两份 locale 的值：键名是内部标识，改键会牵动 golden 与路由文案解析 |
| 页面 id | `storage` | **保留 `storage`** | 改 id 会牵动 `/settings/storage` 路由、`pageRegistry` 映射与 golden fixture，收益为零。**保留** |
| keywords | `storage` / `attachment` / `orphan` / `disk` / 存储 / 附件 / 孤儿 | 追加 `resource` / `process` / `memory` / `cpu` / 资源 / 进程 / 内存 | 搜索命中面扩大，属纯增量 |
| icon | `DatabaseOutlined` | 保持（或换 `DashboardOutlined`） | 换 icon 需同步 golden；本期**保持**以减少无关改动 |

**必须**在实施时同步更新 `src/settings/__tests__/fixtures/snapshot.full.json` 与 `snapshot.vaultless.json`，并跑 `settingsSnapshot.golden.test.ts`；禁止用「跳过测试」替代 fixture 更新。

### 3.4 不做跨 tab 汇总（硬约束）

**禁止**在任何 tab 或页面顶部显示「本应用总占用 = 磁盘 + 内存」这类数字。理由有三条，任一条成立即否决：

1. **单位不同**：字节与工作集不可相加；
2. **口径不同**：磁盘侧是文件长度之和，内存侧是工作集或私有工作集（§6.3.2），两者都不含「将来的实际占用」；
3. **归属不同**：磁盘侧按 owner 目录归属，内存侧按进程树归属（§6.2），两个归属集合无交集语义。

用户要问「占了多少盘」和「占了多少内存」，是**两个问题、两个 tab、两个数字**。合并成一个数会同时错两次。

---

## 4 存储 tab

### 4.1 观测表（12 行）

| 行 | owner | stats 现状 | 状态判定 |
|---|---|---|---|
| `attachments/` | `AttachmentStore` | `list_all`（第一层） | `available` |
| `attachments/derived/` | `AttachmentStore` | 无（`list_all` 跳过目录，N5） | `notImplemented` |
| `tool-results/` | `FileToolResultStorage` | 无（只有 `persist` 与 `root()`） | `notImplemented` |
| `mascot-assets/` | `cmd/mascot_asset` | 无 | `notImplemented` |
| `assistant.db` | `setup` / `DbState` | 仅文件长度可得 | `available`（仅体量，无表级） |
| `security/security.db` | `SecurityDbState` | 无 | `notImplemented`，未解锁时 `locked` |
| `security/backups/` | `SecurityDbState` | 无（回收有 `prune_backups`） | `notImplemented` |
| `security/vault/` | `VaultFileManager` | 无 | `notImplemented` |
| `security/recovery/` | 可选能力模块 | 无 | `notImplemented`（瞬态） |
| `security/audit/` | `SecurityDbState` | 无 | `notImplemented` |
| `logs/` | `tauri-plugin-log` | 插件不暴露 | `notImplemented` |
| `crashes/` | `assistant-crash` | 无（对外只有 `install`，N14） | 当前 `notImplemented`（缺 `list` / `stat` / `prune`）；**R4 补完后转 `available`** |

**约束**：表的每一行由 owner 提供 stats；**禁止**为凑满 12 行而由监视器自己 `read_dir`。12 行里 11 行是 `notImplemented` 不是缺陷，是本期的**诚实结论**——它恰好告诉用户「这些目录现在看不到数」，而不是给一个假的 0。

**`crashes/` 这一行要单独说明**：它是 11 行里唯一「修它不需要监视器」的行。`assistant-crash` 已经是一个干净的独立 crate（N13），它欠的只是自己的两个缺口——**只读入口**（`list` / `stat`）与**保留上限**（`prune`，N14）。这两个缺口是 `crashes/` 作为 owner 的欠账，与监视器无关；先把它们补上，这一行才能从 `notImplemented` 变成 `available`。反过来，**禁止**把周期性快照写进 `crashes/`——那会让一个已经无轮转的目录再增一类无上限产物（§7）。

**`crashes/` 的 `missing` 不可达**：`prepare_dir` 在启动时即 `create_dir_all`（[crash_dump.rs](../../src-tauri/crates/assistant-crash/src/crash_dump.rs)），故该行状态只可能是 `available` 或 `notImplemented`。

### 4.2 四个方向（v1）

| 方向 | 字段 | 口径约束 |
|---|---|---|
| **D1 体量** | `bytes`、`count` | 字节 = 文件长度之和；**禁止**目录 `metadata.len()`（Windows 上恒 0）；`count` 是条目数，不是会话数 |
| **D2 时间** | `oldestAt`、`newestAt` | 取文件 `modified()`；审计侧取 JSONL 记录内 `at`（一个文件跨多事件），不取 mtime |
| **D3 分层归属** | `layer`（`data` / `local`）+ 层内项 | 每层只对直接子项求和，不跨层累加；嵌套项在父层标 `includesDerived` 等标志 |
| **D4 可用性状态** | `availability` + `availabilityReason` | 按 §2.4 封闭词表；`notImplemented` 必须带原因 |

**D2 的一处既有缺陷必须先修**：`AttachmentEntry.created_at` 的取值是 `metadata.modified()`（N5），字段名却在断言「创建时间」。这是命名撒谎，不是口径争议——**必须**在实施时把字段语义改为「最后修改时间」并在 DTO 上以 `modifiedAt` 命名，或在 owner 侧改为真实创建时间（平台差异需另行定案）。两者择一，不允许保持现状。

**回收不在本期**：`delete_attachments` 已在 [StorageSettings](../../src/settings/pages/StorageSettings.tsx) 中使用，本期**不新增**任何删除入口，也不改其全量校验语义（任一 ID 非法则整体失败）。

### 4.3 三个必须写死的口径

**4.3.1 字节口径**：用户可见数字一律用**文件长度之和**，文案写「文件大小」；磁盘簇占用不作为展示口径；目录 `metadata.len()` 禁止当体积。

**4.3.2 包含范围**：以下四类逐条写死，不允许「看情况」：

| 类 | 对象 | 本期定案 |
|---|---|---|
| 子布局 | `attachments/` 是否含 `derived/` | **不含**，且必须由 owner 以 `includesDerived: false` 自述，不允许靠「`is_file()` 顺手跳过」这种隐式行为 |
| 子布局 | `security/` 是否含 `backups/`、`audit/`、`recovery/` | **父项与子项分别成行**，父项不吸收子项 |
| 伴生文件 | 主库 `-journal` / 未来 `-wal` / `-shm` | **不计入**，由 DB 层自述，文件系统层不猜 |
| 伴生文件 | 素材目录的 `custom.tmp` | **不计入**（`atomic_write` 产物归其 owner 管） |
| 旧位置残留 | 迁移前落在 data 根的旧 `crashes` | **不计入**（I44（已归档）明确不迁移） |
| 自身产物 | 监视器自己写进 `logs/` 的日志 | **计入但标注**「含本应用日志」，不做递归扣除 |

**4.3.3 时间口径**：展示排序与（将来的）回收排序必须同源。本期无回收，但字段语义必须先定死，否则 v2 加资格时「显示最旧」与「允许删最旧」会错位。

### 4.4 账本闭合（v1 必须满足）

现有实现有一个可验证的不闭合（N4）：

```
顶层 totalBytes = 全部磁盘条目之和
每会话 totalBytes = 该会话引用到的条目之和
分组键 = (conversation_id, attachment_id) 去重
```

后果：**同一附件被两个会话引用时，其字节被计入两次会话合计**，于是

```
totalBytes ≠ Σ conversations.totalBytes + Σ orphans.bytes
```

**约束**：v1 必须以新增字段实现闭合，且**不重定义**旧字段含义：

1. 新增 `sharedBytes`：被 ≥2 个会话引用的附件字节之和（同一附件只计一次）；
2. 新增 `referencedOnceBytes`：仅被 1 个会话引用的附件字节之和；
3. 必须满足恒等式：`totalBytes == referencedOnceBytes + sharedBytes + Σ orphans.bytes`；
4. 前端在会话分组区**必须**标注「跨会话共享，会话合计大于物理占用」，否则用户会看到会话合计大于总量并判定数据造假。

该恒等式应当写成断言（后端单测），而不是只写进文档。

---

## 5 进程 tab

### 5.1 现状

- 全仓无进程登记表（N8）：无 PID 登记、无 `JoinSet`、无进程组；
- `spawn_screen_inspection` 是唯一固定间隔线程，但它是 UI 逻辑（间隔为常量、无失败语义、不可配置），**只能参照形态、不能复用**；
- 受管子进程当前仅存在于未来（`shell` 见 [I52](./I52-shell-sandbox-spec.md)，MCP server 见 I38e MCP 工具规格（已归档））；
- 但**OS 层事实已经存在**：本进程有 PID、有存活状态、有父子链、有 CPU 与内存计数。这部分不依赖任何应用级登记。

### 5.2 登记表管什么、不管什么

这是本节最容易做过头的判断。**登记表不是进程数字的前置，只是任务归因的前置**：

| 要回答的问题 | 需要登记表 | 数据来源 |
|---|---|---|
| 存在与 PID 存活 | **不需要** | OS（进程表） |
| 父子关系 | **不需要** | OS（`parent_pid`，但**必须**加创建时间校验，§9） |
| CPU / 内存 | **不需要** | OS |
| 角色分类（WebView / 其他子进程） | **不需要** | 命令行特征（§6.2） |
| **这个进程是哪个任务**（「MCP server foo」） | **需要** | 应用级语义，OS 不知道 |
| **排队中**（尚未启动、无 PID） | **需要** | 没有 PID 就没有 OS 事实 |
| **代际身份**（重启后不混淆同一任务） | **需要** | OS 的 PID 会复用 |

**约束（已收窄）**：OS 级数字（存在 / CPU / 内存 / 角色）**不依赖**登记表；只有**任务归因**与**排队态**依赖。因此 v1 可以出「树级数字」，只是不能出「哪个任务」——原先把两者绑在一起是错的，它会让本 tab 白等一个与本 tab 无关的前置。

登记表六件事仍然成立，但只影响「归因」列：

| # | 要素 | 现状 | 影响 |
|---|---|---|---|
| 1 | **身份**：代际写进身份本身，不在回调里加时间戳比较 | 无 | 归因 |
| 2 | **登记**：先登记后启动 | 无 | 归因 + 排队态 |
| 3 | **准入**：长驻与一次性分治 | 无 | 排队态 |
| 4 | **调度**：按归属分组、按自报上限准入 | 无 | 排队态 |
| 5 | **状态**：`Queued` 必须显式 | 无 | 排队态 |
| 6 | **可见**：投影到前端 | 无 | 归因 |

**仍然禁止**：在登记表成立前显示**任务级**数字（如「MCP server foo 占 40 MB」）。没有身份代际，重启一次、停止一次之后归因必然错乱——错错的归因比没有归因更危险。

**5.2.1 归因与数字应在类型上分开**

参考实现（Chromium）把这件事做成了**两个对象**，值得照抄结构：

| 对象 | 承载 | 数量关系 |
|---|---|---|
| `Task` | **归因身份**（标题、类型、所属页面、能否停止） | N 个 |
| `TaskGroup` | **数字**（CPU / 内存 / 句柄），挂在 **PID** 上 | 1 个 / 进程 |

「多个 task 可以共享同一个进程」正是这种拆分的理由。它的价值在于：**「数字不依赖归因」在类型系统上就成立了，不靠约定**。v1 没有登记表，那就只有 `TaskGroup` 这一半——这恰好解释了为什么 v1 能出树级数字：**缺的是 `Task`，不是数字**。

**5.2.2 归因树必须自己防环**

**不能指望父子关系一定无环**。OS 的进程树在「快照 + 校验创建时间」下可以保证无环（§9），但**应用层自己连出来的归因父子链会成环**——连错一次就死循环。因此：

1. 遍历归因父链**必须**带 `visited` 集合，命中重复即判定成环并报错，**禁止**用「父链一定有终点」作为循环终止条件；
2. 这条约束只在登记表落地后生效，但**设计登记表时必须先写进去**，因为它是结构性缺陷，事后补要改遍历实现。

参考实现的注释原文即为此：「It is not guaranteed that the nodes returned are a tree (acyclic)」。

### 5.3 方向（P1–P5）

| 方向 | 数据 | 来源 | v1 |
|---|---|---|---|
| P1 存在与状态 | PID、存活、父子 | OS | **交付** |
| P2 CPU | 单核百分比（可超 100，§6.3.3） | OS | **交付** |
| P3 内存 | 工作集（§6.3.2） | OS | **交付** |
| P4 树归属与角色 | 谁是 WebView、谁是其他子进程 | OS 父子链 + 命令行特征 | **交付**（仅角色，不含任务名） |
| P4b 任务归因 | 这个进程属于哪个任务 | 登记表 | 推迟（§10 第 9 项） |
| P5 回收 | 整棵树 | Job Object（Windows）/ `kill(-pgid)`（Unix），判据见 I53（已归档） | 不做（本期无回收入口） |

### 5.4 v1 边界

v1 的进程 tab 交付**树级数字、无任务归因**：

1. 列出缝 B 投影 B 的全部后代进程，每行显示 PID、父 PID、`exeName`、CPU、内存（**不设角色列**，§6.2）；
2. 每行标注 `sampledAt`，并说明「本表只覆盖本进程的子树，脱离父子链的进程不可见」；
3. 不显示任何任务名、不显示排队态、不提供停止按钮。

**两条必须写在 UI 上的边界**（否则数字会被误读）：

| 边界 | 说明 |
|---|---|
| **子树边界** | 只观测本进程的后代。主动 `detach` / `daemonize` 的子进程会脱离父子链，从而**永久不可见**——这不是采样失败，是这类进程的设计意图 |
| **无归因边界** | 同一角色的多个进程在 v1 里是**可区分但不可命名**的（按 PID 区分），用户看到的是 `msedgewebview2.exe (pid 1234)` 而不是「渲染进程 #2」 |

**禁止**在 v1 显示「0 个进程」「0 MB」这类占位数字：它们看起来是结论，实际是没测。**也禁止**把宿主自身混进这张表（那是投影 A 的范围，§6.1）。

---

## 6 自身占用 tab

### 6.1 两个自指禁区（硬约束）

| 禁区 | 后果 | 约束 |
|---|---|---|
| **停止自指** | 宿主进程登记进受管任务表 → 停止自己 = 应用自杀 | 宿主**禁止**登记进任务表；自身占用必须是独立 tab，不共用停止语义 |
| **峰值自指** | 采样器与被观测者同生共死 → OOM / 卡死时最后那次采样必然丢失 | **峰值禁止**由自身采样提供，只能由外部留痕（§6.5 约束 2） |

两条禁区同源于「观测者与被观测者是同一个东西」，但表现不同：前者是**动作**上的自指，后者是**证据**上的自指。第一条可以靠「不登记」绕开；第二条无法绕开，只能**换观测者**。因此自身占用 tab 与进程 tab 共用采样（§2.3），但峰值区必须是**外部产物阅读器**（§6.6）。

### 6.2 一个根、一棵树、两种投影

缝 B 以 `root = 自身 PID` 从**一次全系统快照**里按父子链推导出整棵子树（形态见 §2.3），再按**角色**切成两种投影：

| 角色 | 判据 | 归属投影 |
|---|---|---|
| 宿主自身 | PID == root | A（自身占用） |
| WebView renderer | 命令行含 `--type=renderer` | A（自身占用） |
| WebView GPU | 命令行含 `--type=gpu-process` | A（自身占用） |
| WebView utility / 其他 WebView 角色 | 命令行含 `--type=` 的其余取值 | A（自身占用） |
| 其他子进程 | 其余全部后代 | B（进程 tab） |

**DFS 必须覆盖任意深度**（不是只走一层）：WebView2 的 renderer / GPU / utility 是 `msedgewebview2.exe` **根进程**的子进程，即宿主的**孙进程**。只取直接子进程会漏掉真正吃内存的那几个（§7）。

角色判据照参考实现 openhuman 的 `classify_process`：先判是否等于根 PID，再看命令行 `--type=` 特征，其余归 `OtherChild`。

**角色列的用途要说清**：`role` 只用于**投影 A 内部细分**（宿主 / renderer / GPU / utility 分开展示，否则三个 WebView 进程混成一个聚合数说不清）。投影 B 的成员按定义**全部是 `otherChild`**，所以**进程 tab 不设角色列**——一列只有一个取值等于没有这一列。两个投影因此**互不重叠、不重复计数**：一个进程要么在自身占用 tab，要么在进程 tab。

### 6.3 口径与单位（两件事分开定）

**6.3.1 默认口径**：取投影 A 的全部进程（宿主 + WebView），不取「仅主进程」。**不选口径就是默认选了「仅主进程」**，而它恰好是最容易被质疑的那个（用户看 200 MB、任务管理器看 900 MB）。

**6.3.2 内存单位与可加性**（本节最要紧的一条）：

| 单位 | 可加性 | 含义 |
|---|---|---|
| **工作集 / RSS**（resident） | **不可加** | 共享页（COW、共享 DLL、WebView 共享段）在每个进程里各记一次，求和必然**大于**物理占用 |
| **私有工作集**（private working set） | **可加**（近似） | Windows 任务管理器「内存」列用的就是这个 |
| **PSS**（proportional set size） | **子树内不可加** | 共享页按**全系统**共享者数量摊 1/N；N 是全局的，不是子树内的 |
| 提交量 / commit | 可加 | 含未驻留部分，不是「占了多少内存条」 |
| 虚拟内存 | 可加（但对「占多少内存」无意义） | 禁止作展示口径 |

**约束（与 §4.4 账本闭合对称）**：

1. **可加性是「按指标」的属性，不是「按视图」的属性**：同一个视图里可以同时有可加的指标（CPU 百分比）与不可加的指标（工作集）。因此 DTO 里该标志挂在**指标**上（`memoryAdditive`），不挂在 tab 或行上（§8.2）；
2. 若展示内存合计，**必须**用私有工作集；若用工作集，**禁止**把子项求和当作总量，只能标注「各进程工作集之和（含共享页重复计数）」；
3. **任何展示内存的列，列名必须自带口径**（「工作集」/「私有工作集」），**禁止**出现不带口径修饰的「内存」「占用」列——列名即口径，不靠脚注兜底。这条比 `memoryAdditive` 字段更能防住「后端换了口径、前端忘了分支」；
4. 单进程数字用工作集没有问题；**只有跨进程求和时**不可加性才成立；
5. **PSS 只对全机视角成立，对子树视角不成立**：子树内求和会偏小，因为与树外进程（系统 DLL、其他应用的 WebView2）共享的页只摊了 1/N 却要算全额。若将来引入 `pss` 口径，`memoryAdditive` **必须仍为 `false`**；
6. 这与存储侧「同一附件被两个会话引用被计两次」是**同一个病、换了器官**：父项 ≠ Σ 子项，因为存在共享。

**6.3.3 CPU 单位**：单逻辑核百分比，**可超过 100**。禁止除以核数改成「整机占比」而不标注——用户对照任务管理器会看到不同数字。

CPU 百分比**天然可加**（各进程的 CPU 时间互相独立、无共享页问题），因此 DTO 里不为 CPU 设可加性标志；它的合计**可以直接显示**，但必须说明是「单核百分比之和，可超过 100」。

### 6.4 三层拆分与平台方案

「监视自身」是三个可行性完全不同的问题，必须分开实现：

| 层 | 回答什么 | 数据源 | 能否进程内 | 落点 |
|---|---|---|---|---|
| **A 崩溃留痕** | 死的那一刻发生了什么 | 异常处理上下文 | **必须进程内** | `assistant-crash`（既有，N13 / N15） |
| **B 当前值采样** | 现在占多少 | OS API 读自己与子树 | **必须进程内** | 新增 crate `assistant-resource-probe` |
| **C 峰值与热点** | 峰值多少、谁在吃 CPU | 需活得比被观测者久 | **必须进程外** | `src-tauri/src/bin/resource_report.rs`（不进 bundle，N16） |

**为什么 B 要独立 crate 而不是放应用层**：它只依赖 OS API，不需要 `AppDirs`、不需要任何 owner；独立后可在不启动 Tauri 的情况下跑单测，并把平台依赖隔离在一个 crate 内。它的缝照抄 `assistant-crash`：**不读 `dirs`、不算基根、路径与根 PID 由调用方注入**（N13）。这条豁免的边界是「不得引用应用层任何模块」（§1.3）。

**平台方案（待定案，§10 第 8 项）**：

| 方案 | Windows | Linux | macOS | 新依赖 | 代价 |
|---|---|---|---|---|---|
| **A. `sysinfo`** | 一条覆盖 | 一条覆盖 | 一条覆盖 | **1 个**（当前为 0，N9） | 引入新依赖（D12 范畴）；但只污染采样 crate，不进应用层 |
| **B. 手写平台 API** | 需补 `Win32_System_ProcessStatus`（`GetProcessMemoryInfo`，N10） | `/proc` 零依赖 | 需 `libc` | 0 但三套代码 | 本形态要遍历**整棵子树**，三套「遍历 + 分类 + 读内存」成本 ×3 |

**倾向 A**：参考实现 openhuman 已用 `sysinfo` 验证该场景（含整棵子树遍历与峰值统计），且 B 的三套代码在「递归 + 角色分类」上会显著放大。但**新增依赖必须单独过审**，故列为待定而非定案。

### 6.5 五条硬约束

| # | 约束 | 依据 |
|---|---|---|
| 1 | **停止自指**：宿主不得登记进受管任务表 | §6.1 |
| 2 | **峰值必须外部留痕**：OOM / 卡死时自采样必然失效 | 采样器与被观测者同生共死（§6.1 第二条） |
| 3 | **采样频率必须有上限**：秒级起步 + 事件驱动 + **采样失败不重试** + **不得在主线程** + **同一拍不得重入**（上一拍的异步结果未回投则本拍不发新请求）；间隔取平台最小刷新间隔的下限作常量，不得用魔法数 | D10；参考实现用 `max(MINIMUM_CPU_UPDATE_INTERVAL)`；Chromium 用 `waiting_for_memory_dump_` 布尔量防重入 |
| 4 | **落盘产物必须自限**：自监控快照落 `logs()` 同级，复用 `KeepSome` + 单文件上限；**禁止**写进 `crashes/`（该目录已无轮转，N14） | D10 |
| 5 | **采样由消费者引用计数驱动**：最后一个消费者（tab / 页面）移除时即停表、**清空缓存、失效全部弱引用**；**禁止**后台常驻轮询，**禁止**在应用启动时预热采样 | 一个「监视自身」的功能若自身成为最大的开销来源，逻辑上自相矛盾（§7） |

**约束 5 为什么用引用计数而不是「可见性判断」**：

1. 引用计数**不依赖任何 UI 概念**——后端只知道「还有没有人在看」，不需要知道 tab 是否可见、窗口是否最小化；
2. 多个消费者（两个进程视图 tab 都需要缝 B）天然聚合：**任一消费者存在即采样**，不会出现「各自判断、某一处漏判」；
3. 它天然满足 §2.3 的「两个 tab 共用一次采样」：两个消费者各注册一次，后端取最小间隔并合并指标请求（见 §6.5.1）；
4. 「tab 切走就停」是引用计数的**结果**，不是前提；把可见性写成前提会逼前端承担采样生命周期，而前端无法保证不漏。

**约束 5 的直接推论（必须同步到 §9）**：既然停表时清空缓存，那么**每次重新打开面板都是新一次采样的第一帧**——CPU 为 `null` 是**每次打开都会发生的常态**，不是异常，前端不得据此进入错误态。

**6.5.1 指标按需开启（位掩码）**

消费者不是「要么全采要么不采」，而是**各自声明需要哪些指标**，后端按位或后只计算被声明的指标。理由：三个 tab 需要的指标完全不同（存储 tab 一个都不需要，进程 tab 需要 CPU + 内存，自身占用 tab 需要 CPU + 内存 + 峰值区），而 §2.3 又要求两个进程视图共用一次采样——**位掩码正是「共用一个采样器」与「各自只算自己需要的」这对矛盾的解**。

约束：

1. 拉模型下，「声明」通过 `get_process_snapshot` 的入参携带（前端每次调用都带自己需要的 `requestedMetrics`，§8.2）；
2. **同一刷新间隔内的多次调用必须返回同一份快照**（同一 `sampledAt`），不得为第二次调用重新采样——这是 §2.3「两个 tab 共用一次采样」在拉模型下的落地形态；两个 tab 各调一次得到的是**同一拍**，指标声明按并集在**下一拍**生效；
3. 未被任何消费者声明的指标**禁止**计算，也**禁止**在 DTO 里填 `0` 或空占位——应输出 `null`（§9）。

**6.5.2 拉取模型下的等价实现（空闲超时）**

本项目的取数是**拉模型**（前端调命令，§8.1），后端没有「观察者对象」可以引用计数。因此约束 5 的落地形态是**空闲超时**：

| 事件 | 后端行为 |
|---|---|
| 收到 `get_process_snapshot` | 若采样器未运行则启动；刷新「最后取数时刻」 |
| 距最后一次取数超过 `idleTimeout` | **停表并清空缓存、失效弱引用**；下一次取数重新从空缓存开始（首拍 CPU 为 `null`） |
| 应用启动 | **不预热**；采样器处于停止态 |

`idleTimeout` **必须**取「刷新间隔的整数倍」常量（建议 2–3 倍），不得用魔法数（§10 第 10 项同源）。**禁止**用「前端主动调 stop 命令」作为唯一停表路径——前端崩溃或页面卸载漏调时，采样会永久泄漏；空闲超时是兜底且**必须**是主路径。

若将来改为事件推送（`listen` 通道），则必须换成真正的注册/注销引用计数，且**仍然保留**空闲超时作兜底。

**对约束 2 的一处修正**：先前把「Job Object 内核统计」列为可提供峰值的方案，**该条降级为待验证**——Job Object 的峰值统计在**作业句柄存活期间**可查，而句柄通常由宿主自己持有；宿主死亡时句柄一并消失，与自采样同样失效。除非由**外部进程**持有句柄，否则它不能算「外部留痕」。当前唯一确定成立的外部来源是 crash dump 与 OS 侧记录。

**约束 2 与整个 §6 的冲突必须写明**：缝 B 在进程内，所以它**永远给不出自己的峰值**。这不是实现缺陷，是这条路径的固有边界；峰值区只能读 C 的产物（§6.6）。

### 6.6 v1 边界

v1 的自身占用 tab 由两块组成，**两块的数据性质不同，必须分开渲染**：

| 块 | 内容 | 数据来源 | 状态 |
|---|---|---|---|
| **当前值** | 宿主 + WebView 的 CPU / 内存 | 缝 B（进程内） | `available`（平台方案定案后） |
| **峰值与热点** | 峰值内存、峰值 CPU、CPU 热点 | 外部报告（C） | `noReportYet` / `staleReport` / `available` |

**峰值区是报告阅读器，不是监视器**：它读上次诊断产物，必须显示产物生成时刻，且必须在 UI 上说明「这是 X 之前的历史快照，不是当前值」。无产物时显示 `noReportYet` 并给出「如何生成」的说明；产物过期时显示 `staleReport`，仍可看数字但必须带时间标注（§2.4 约束 4）。

**禁止**把峰值区做成「实时峰值」——那等于在进程内自采样峰值，违反 §6.1 第二条与 §6.5 约束 2。**禁止**在平台方案定案前显示任何当前值占位数字。

---

## 7 反模式

| 反模式 | 后果 | 替代 |
|---|---|---|
| 自己 `read_dir` 遍历受管目录求和 | 绕过 owner，绕过 `is_plain_file_name` 与库内子布局约定 | 调 owner 的 stats |
| 自己 `remove_file` / `remove_dir_all` | 引用与文件两套语义分叉 | 调 owner 的回收方法 |
| 自己拼基根或目录名 | 第二套字面量，I44（已归档）门禁失效 | 调 `AppDirs` 的 getter |
| **新建 crate 承载「存储观测」** | 反向依赖，无法注入 `AppDirs` 与 owner | 放应用层（`src-tauri/src/cmd/`）；采样类 crate 另有豁免（§1.3） |
| **采样 crate 反向引用应用层** | 豁免被滥用，等于把应用层搬进 crate | crate 只依赖 OS API，路径与根 PID 由调用方注入（§6.4） |
| 把 `config.json` 等纳入观测表 | 破坏 `assistant-config` 的既有例外 | 明确排除 |
| 把工作区根 / 导入源 / WebView 缓存 / 系统 temp 纳入 | 造第 5 套基根 | 明确排除 |
| 用 `to_string_lossy` 拼比较键 | 非 UTF-8 被改写，统计与去重失真 | 非 UTF-8 显式报错 |
| **把读不到显示成 `0`** | 伪精确，用户据此做出错误清理决定 | `missing` / `locked` / `unreadable` / `notImplemented` 分开 |
| **缺失时间回退到当前时刻** | 把「不知道」伪装成「刚刚更新」，比不显示更糟 | 输出 `null`，键恒在 |
| **跨 tab 相加成一个「总占用」** | 单位不同、口径不同、归属不同，同时错两次 | 每 tab 独立数字 |
| **把工作集求和当作物理占用** | 共享页被重复计数，合计必然偏高且无法解释 | 用私有工作集，或标注「含共享页重复计数」（§6.3.2） |
| **把宿主进程登记进任务表** | 停止自己 = 应用自杀 | 独立 tab（§6.1） |
| **在进程内采样自己的峰值** | OOM / 卡死时那次采样必然丢失，峰值永远是「还没崩的时候」 | 峰值由外部产物提供（§6.5 约束 2） |
| **只取直接子进程** | WebView2 的 renderer / GPU / utility 是孙进程，全被漏掉 | 递归到任意深度（§6.2） |
| **对每个后代各查一次系统接口** | O(N) 次系统调用，N 随子树增长，开销随时序恶化 | 一次全系统快照 + 内存建父子表 + 从根 DFS（§2.3） |
| **按镜像名判定归属** | `msedgewebview2.exe` 是全机器共享的名字，必然收进别的应用的进程 | 按祖先链过滤，镜像名只作粗筛（§2.3 约束 3） |
| **后台常驻采样 / 启动时预热采样** | 监视自身的功能自身成为最大的开销来源，逻辑自相矛盾 | 消费者引用计数驱动，无人看即停表清缓存（§6.5 约束 5） |
| **内存列不带口径修饰** | 「内存」一列必然错，用户无法判断含不含共享页 | 列名即口径（「工作集」/「私有工作集」），不靠脚注（§6.3.2 约束 3） |
| **把首帧 CPU `null` 当错误** | 每次打开面板都是第一帧，会把常态渲染成故障 | 显示「本拍无基准」，不进错误态（§9） |
| **沿 `parent_pid` 上溯但不校验创建时间** | PID 复用后会把无关进程认成自己的子进程，统计凭空多出一块 | 校验 `child.start_time >= parent.start_time` 且父进程确实存在（§9） |
| **在登记表成立前显示任务级归因** | 归因错乱比没有归因更危险 | 只显示树级数字，不显示任务名（§5.2） |
| **让两个投影重叠**（进程 tab 里也列 WebView） | 同一进程出现在两个 tab，数字互相打架，且无法解释「自身占用 300 MB」为何不在此表中 | 投影互斥：A 归自身占用、B 归进程 tab（§6.2） |
| **在投影 B 里设角色列** | 该列取值恒为 `otherChild`，等于没有这一列，还会让人以为「没有 WebView」 | 角色只用于投影 A 细分（§6.2） |
| **把周期性快照写进 `crashes/`** | 给一个已无轮转的目录再增一类无上限产物 | 写 `logs()` 同级并复用 `KeepSome`（§6.5 约束 4） |
| **让监视器自己读配置当判据** | 开第 6 个配置入口；且拥有「推出该删什么」的全部素材 | 策略由 owner 自述（v3） |
| **为凑满观测表而自建目录清单** | 第二套字面量 | 允许 11 行为 `notImplemented` |

---

## 8 契约

### 8.1 命令与模块划分

| 命令 | 职责 | 模块 |
|---|---|---|
| `get_storage_breakdown` | 存储 tab 明细（既有命令，扩展字段） | `src-tauri/src/cmd/storage.rs` |
| `get_resource_capabilities`（新增） | 只返回三个 tab 的**可用性状态与原因**，**不含任何数字** | `src-tauri/src/cmd/resource.rs`（新增） |
| `get_process_snapshot`（新增） | 缝 B 的一次采样结果：`sampledAt` + 整棵子树（两种投影聚合 + 逐进程明细），供进程 tab 与自身占用当前值共用 | `src-tauri/src/cmd/resource.rs` → `assistant-resource-probe` |
| `get_self_usage_report`（新增） | 只读外部诊断产物（C），返回产物内容 + 产物生成时刻 + `availability` | `src-tauri/src/cmd/resource.rs` |

**为什么能力声明要单独一条命令**：若把能力与数字塞进同一个响应，就必须有一个「跨 tab 聚合响应」，而那正是 §3.4 禁止的东西。分开后，每个 tab 各自取数、各自失败、互不影响——存储 tab 读失败不会让进程 tab 显示「不可用」。

**为什么 `get_process_snapshot` 只有一条命令**：进程 tab 与自身占用 tab 的当前值**必须来自同一次采样**（§2.3）。若分成两条命令，两次调用之间的时间差会让「自身 + 子进程」不可加、不可解释。响应里同时带两种投影，前端各取所需。

**这条命令同时承担「采样心跳」**：每次调用都刷新采样器的最后取数时刻，超过 `idleTimeout` 无调用则停表清缓存（§6.5.2）。因此**禁止**为「开始/停止采样」再增设命令——那会让停表路径依赖前端自觉，反而更易泄漏。

**为什么报告要单独一条命令**：外部产物是**历史快照**，与实时采样是两种数据性质（§6.6）。混在 `get_process_snapshot` 里会让「当前值」与「峰值」共享一个 `sampledAt`，那是最容易被误读的写法。

模块归属：新增 `cmd/resource.rs` 只做能力声明与两条转发（**不承载**存储明细，也不自己实现采样）。采样实现在 `assistant-resource-probe`，报告读取在 `cmd/resource.rs` 内做最小解析。这符合 [CODE-SPLIT-PRINCIPLE](./../principle/CODE-SPLIT-PRINCIPLE.md) 的「按能力域拆分」，本文不复述其判据。

### 8.2 DTO 字段口径

**存储侧新增字段**（追加到既有 `StorageBreakdownDto`，不改旧字段含义）：

| 字段 | 类型 | 约束 |
|---|---|---|
| `sampledAt` | `i64` | 采样时刻（秒），遍历非原子，必须有 |
| `byteBasis` | 封闭枚举 | 本期恒 `fileLength` |
| `sharedBytes` | `u64` | §4.4 恒等式 |
| `referencedOnceBytes` | `u64` | 同上 |
| `entries` | `Vec<StorageEntryDto>` | 12 行观测表 |

**`StorageEntryDto`**：

| 字段 | 类型 | 约束 |
|---|---|---|
| `key` | `String` | 行标识（如 `attachments`、`security/audit`），**不是路径**，前端不据此拼路径 |
| `layer` | 封闭枚举 | `data` / `local` |
| `owner` | `String` | 提供 stats 的 owner 名，用于文案「由 X 提供」 |
| `bytes` / `count` | `u64 \| null` | D1；**不可用时为 `null`，禁止填 `0`**（否则 §7 的「用 0 冒充读不到」从文案问题升级成契约问题）；键恒在 |
| `oldestAt` / `newestAt` | `i64 \| null` | D2；**输出侧 `Option` 不加 `skip_serializing_if`**，键恒在、值 `null` |
| `availability` | 封闭枚举 | §2.4 词表 |
| `availabilityReason` | `String \| null` | `notImplemented` 时必填，键恒在 |
| `includesDerived` | `bool` | 口径进 DTO，不只进文档 |
| `countsSelfLogs` | `bool` | 自指标注（`logs/` 为 `true`） |

**能力声明侧**：

| 字段 | 类型 | 约束 |
|---|---|---|
| `tab` | 封闭枚举 | `storage` / `processes` / `selfUsage` |
| `availability` | 封闭枚举 | §2.4 |
| `reason` | `String \| null` | 键恒在 |

**进程快照侧（`ProcessSnapshotDto`，一条命令同时供两个 tab）**：

**入参**（`get_process_snapshot` 的请求体）：`requestedMetrics`（位标志集合，可缺省，缺省时按「进程 tab + 自身占用当前值」所需取 CPU + 内存）。入参只影响**下一拍**计算什么，不影响本拍返回的缓存快照（§6.5.1 约束 2）。

| 字段 | 类型 | 约束 |
|---|---|---|
| `sampledAt` | `i64` | 一次遍历一个时刻，两个投影共用 |
| `rootPid` | `u32` | 采样根，即宿主 PID |
| `memoryBasis` | 封闭枚举 | `workingSet` / `privateWorkingSet`；**口径进 DTO**，前端据此决定是否显示内存合计（§6.3.2）。`pss` 为**保留值**，本期不产出（Windows 无此口径，且子树视角不可加，§6.3.2 约束 5） |
| `cpuBasis` | 封闭枚举 | `singleCorePercent`（可超 100，§6.3.3） |
| `requestedMetrics` | 位标志集合 | 本拍实际计算的指标（由消费者声明合并而来，§6.5.1）；未在其中的指标一律 `null` |
| `coverage` | 封闭枚举 | `subtreeOnly`：**明确写出「只覆盖本进程子树」**，不是采样失败 |
| `truncated` | `bool` | 遍历是否因深度/数量上限被截断；为 `true` 时**禁止**把结果当完整快照展示 |
| `selfProjection` | `ProcessGroupDto` | 投影 A（宿主 + WebView）的聚合 |
| `childrenProjection` | `ProcessGroupDto` | 投影 B（其余后代）的聚合 |
| `entries` | `Vec<ProcessEntryDto>` | **全部**进程逐条明细，每条带 `projection` 字段；两个 tab 各自过滤，**不重复采样** |

**`ProcessGroupDto`**：

| 字段 | 类型 | 约束 |
|---|---|---|
| `pidCount` | `u32` | 组内进程数 |
| `memoryBytes` | `u64 \| null` | 按 `memoryBasis` 求和；未声明该指标时为 `null`，**禁止填 `0`**（键恒在） |
| `cpuPercent` | `f32 \| null` | 按 `cpuBasis` 求和；**首次采样为 `null`，禁止填 `0`**（§9）；CPU 天然可加，不设标志（§6.3.3） |
| `memoryAdditive` | `bool` | **内存**这一指标是否可加：`privateWorkingSet` 为 `true`，`workingSet` 为 `false`。标志挂在**指标**上而非组/视图上（§6.3.2 约束 1）；前端据此决定能否显示内存合计 |

**`ProcessEntryDto`**：

| 字段 | 类型 | 约束 |
|---|---|---|
| `pid` / `parentPid` | `u32` | 父子链原值，前端不据此重建树（排序与分组由后端做） |
| `projection` | 封闭枚举 | `self` / `children`；决定这条明细落在哪个 tab，后端算一次、前端各取所需（§2.3） |
| `role` | 封闭枚举 | `host` / `webviewRenderer` / `webviewGpu` / `webviewUtility` / `webviewOther` / `otherChild` |
| `exeName` | `String` | 开放集合，前端兜底不窄化；**不含命令行全文**（命令行可能含凭据） |
| `cpuPercent` | `f32 \| null` | 单核百分比；首次采样或未声明该指标时为 `null`，**禁止填 `0`** |
| `memoryBytes` | `u64 \| null` | 按 `memoryBasis`；未声明该指标时为 `null`，**禁止填 `0`**（键恒在） |
| `startTime` | `i64` | 进程创建时刻；**必须**随附，用于 PID 复用的可解释性（§9）；同时也是「建父子链」的必需字段（§2.3 约束 4） |

**`SelfUsageReportDto`（外部产物，历史快照）**：

| 字段 | 类型 | 约束 |
|---|---|---|
| `availability` | 封闭枚举 | 只允许 `available` / `staleReport` / `noReportYet` / `unreadable` |
| `generatedAt` | `i64 \| null` | 产物生成时刻；**键恒在、值可为 `null`**（`noReportYet` 时为 `null`） |
| `staleAfterSeconds` | `u64` | 陈旧阈值，与后端判定同源，前端不自己算 |
| `peakMemoryBytes` | `u64 \| null` | 峰值；`null` 表示产物存在但未记录该指标，**不得填 `0`** |
| `peakCpuPercent` | `f32 \| null` | 同上 |
| `sampleCount` | `u32 \| null` | 产物覆盖的采样次数；`null` 表示未记录 |
| `attributionNote` | `String` | 必须非空：说明「这是生成于 X 之前的历史快照，不是当前值」 |

**约束**：`ProcessSnapshotDto` **不含任何任务名、任务状态、停止入口**字段（§5.2 收窄后的边界）。`SelfUsageReportDto` **不含任何当前值字段**——当前值只能来自 `get_process_snapshot`（§6.6 两块分离）。

**生成与校验**：改 DTO 后必须跑 `npm run contract:gen` 并提交 `src/types/protocol.contract.ts`；禁止手改生成类型；禁止用 `Record<string, unknown>` 传部分字段。封闭集合用 `enum`，开放集合（如 `owner` 名）保留 `String` 且前端兜底不窄化。

### 8.3 前端落点

| 项 | 变更 |
|---|---|
| `StorageSettings.tsx` | 改造为容器：内嵌 antd `Tabs`（`activeKey` + 三 tab），存储 tab 内容基本沿用现有渲染逻辑，只增加「观测表」区块与「共享引用」标注 |
| 进程 tab 组件 | 渲染 `get_process_snapshot` 中 `projection == children` 的明细（PID / 父 PID / `exeName` / CPU / 内存）+ 顶部 `sampledAt` 与 `coverage` / `truncated` 说明；**无停止按钮、无任务名、无角色列** |
| 自身占用 tab 组件 | **两块分开渲染**：当前值（同一次 `get_process_snapshot` 的 `selfProjection` + `projection == self` 明细）+ 峰值区（`get_self_usage_report`，必须显示 `generatedAt` 与 `attributionNote`）；内存合计仅在 `memoryAdditive == true` 时显示，内存列名必须带口径（§6.3.2 约束 3） |
| `storageService.ts` | 追加 `getResourceCapabilities()` / `getProcessSnapshot()` / `getSelfUsageReport()`；既有 `getStorageBreakdown` 返回类型随契约生成自动扩展 |
| i18n | 两份 locale 同步新增 tab 标题、状态文案、原因文案、口径说明；`common.navigation.storage.title` 只改值不改键（§3.3） |

**前端必须按 `memoryAdditive` 分支**，不能无条件显示内存合计：`memoryAdditive == false` 时只显示各进程行，或显示合计但带「含共享页重复计数」标注（§6.3.2）。这条判断不能放在文案里靠人自觉，必须由字段驱动。CPU 合计不受此约束（天然可加），但必须注明「单核百分比之和，可超过 100」（§6.3.3）。

---

## 9 失败模式

| 失败模式 | 表现 | 必须的语义 |
|---|---|---|
| 目录不存在 | 首次运行、旧位置迁移后 | `missing`，计 0，**不报错、不创建目录**（观测不得产生副作用） |
| 目录不可读 | 权限不足 | `unreadable`，单项标状态，**整账仍返回** |
| 安全库未解锁 | `security/` 不可读 | `locked`，**不是 0 字节** |
| 文件在遍历中被删除 | 并发回收 | 单项跳过，不计入错误 |
| 非 UTF-8 文件名 | 少见但可能 | 跳过并计数，**禁止** `to_string_lossy` 混入展示键 |
| owner 无 stats 入口 | 本期 11 行 | `notImplemented` + 原因 |
| 自指 | 监视器日志落在 `logs/` | `countsSelfLogs: true`，文案标注「含本应用日志」，不做递归扣除 |
| 采样阻塞 UI | 大目录遍历 | 异步执行；前端有 loading 与上次快照时刻 |
| 能力声明读取失败 | 新命令异常 | 前端降级为「状态未知」并保留上次快照，**不得**默认判为可用 |
| **PID 复用** | 父进程死亡后 PID 被复用，而 `parent_pid` 是**创建时的快照值**、不会更新 | 校验 `child.startTime >= parent.startTime` 且父 PID 确实存活；不通过则**排除该进程并标注**，禁止静默并入子树 |
| 进程在采样中退出 | 遍历期间进程消失 | 该行跳过，不报错；`sampledAt` 后已死的进程允许出现在快照里（快照是时刻值，不是持续状态） |
| **子树遍历被截断** | 深度或数量上限触发 | `truncated: true`，UI 必须显示「本次扫描不完整」；**禁止**把截断结果当作完整快照 |
| **CPU 增量语义未满足** | 只刷新一次就取百分比 | 首次采样 CPU 输出 `null`，**禁止**输出 `0`；间隔取平台最小刷新间隔（§6.5 约束 3） |
| **把「首帧 CPU 为 `null`」当作异常** | 采样由引用计数驱动（§6.5 约束 5），停表即清缓存，**每次重新打开面板都是新一次采样的第一帧** | CPU 为 `null` 是**常态**，UI 显示「本拍无基准，下一拍给出」，**禁止**进入错误态、**禁止**提示「数据加载失败」 |
| **新出现的行立即给出数字** | 子进程刚 spawn、或面板刚打开 | 新增进程在**下一个刷新周期之前没有数字**（不是 `0`）；UI 必须能渲染「待定」态 |
| 外部产物不存在 | 从未生成过报告 | `noReportYet`，`generatedAt = null`，给出「如何生成」的说明 |
| 外部产物过期 | 生成时刻早于陈旧阈值 | `staleReport`，仍可显示数字但**必须**带 `generatedAt` 标注 |
| 外部产物损坏 | JSON 解析失败 | `unreadable` + 原因；**禁止**退回上一次成功读取的缓存（那会把两个时刻的数字混成一个） |

---

## 10 待定案项

| # | 待定 | 为什么必须定 | 本期态度 |
|---|---|---|---|
| 1 | D5 资格与 D6 策略 | 二者是回收的下游；无回收即无落点 | **本期不做**（见下） |
| 2 | `tool-results` 是否允许回收 | 决定 `<persisted-output>` 跨进程重放是否仍成立 | **已定：允许**。现有 `cleanup_expired_results` 已按 mtime 年龄回收，代价是超过保留期的定位符失效；参考项目 deepseek-harness 的 spill 清理给出同一结论，并明确「不做定时器、不递归删会话目录、只删文件、符号链接永不跟随」。v1 只标 `notImplemented`，不新增动作 |
| 3 | `security/audit/` 保留策略 | 唯一「会无限增长 + 不在归属表 + 无回收入口」的目录 | 推迟；v1 只标 `notImplemented` |
| 4 | `derived/` 是否计入附件总量 | 影响用户对「删了没释放」的感知 | **已定：不含**，由 owner 自述 `includesDerived: false` |
| 5 | `created_at` 语义修正方式 | 现字段名与取值不符（N5） | **必须修**；改名 `modifiedAt` 或改取真实创建时间，二者择一 |
| 6 | 自身占用默认口径 | 决定所有内存数字 | **已定：投影 A**（宿主 + WebView，§6.3.1） |
| 7 | 内存单位与可加性 | 决定文案与能否显示「合计」 | **已定**：单进程用工作集；跨进程合计用私有工作集，否则必须标注不可加（§6.3.2） |
| 8 | 平台方案 A（`sysinfo`）还是 B（手写三套） | 决定采样 crate 的依赖与工作量 | **待定**；倾向 A，但新增依赖需单独过审（§6.4） |
| 9 | 进程登记表归属 | 决定任务级归因能否启动 | 独立轴，不在本期；**不影响**树级数字（§5.2） |
| 10 | 采样间隔常量与 `idleTimeout` 取值 | 决定 CPU 精度、开销与停表时机 | 待定；间隔必须取平台最小刷新间隔作下限，`idleTimeout` 取间隔的 2–3 倍，二者均不得用魔法数（§6.5 约束 3、§6.5.2） |
| 11 | 外部报告产物的格式与落点 | 决定峰值区能否读、读什么 | 待定；倾向与 openhuman profiling 的 `resources.json` 同构，落 `logs()` 同级并受 `KeepSome` 约束（§6.5 约束 4） |
| 12 | 角色分类数据源：命令行 `--type=` 嗅探 vs WebView2 进程 API | 决定 §6.2 的角色表是否可靠 | 待定；嗅探零依赖但脆弱（命令行可被改写、读取有隐私面），WebView2 的 `ProcessFailed` 事件与新 SDK 的进程枚举只给 kind + pid。**两者只影响分类，不影响采样**——它们给不出内存与 CPU，替代不了缝 B 的任何工作量 |
| 13 | 全系统快照的取得方式（是否复用 `NtQuerySystemInformation`） | 决定「一次遍历」能否真的做到单次系统调用 | 待定；`sysinfo` 内部已做一次全表遍历，需核实其 API 是否会因多次查询退化为多次系统调用（§2.3 约束 1） |

**关于第 1 项为何本期不做**：资格（D5）与策略（D6）挂在「回收动作」下面。链路是「回收 → 需要资格 → 资格需要理由 → 理由常常是策略」。v1 没有回收入口，这条链路的第一环缺失，后两环就没有落点。提前做会付三笔代价：① 策略有 4 个来源（2 个配置项 + 3 个代码常量），一半不可调，显示等于假承诺；② 两处 owner 的降级方向相反（审计读配置失败 → 永不清理；工具结果读配置失败 → 照 30 天删），一旦把「策略来源」写进 DTO，这个不一致就从实现问题升级为契约承诺；③ `effectiveSince` 要求 owner 自述「本次启动生效值」，而多数 owner 连 stats 都没有。正确顺序是**先让数字自己说话**。

---

## 11 验收判据

| 编号 | 判据 | 失败含义 |
|---|---|---|
| V1 | 页面 label 显示「资源监视器」，页面 id 仍为 `storage`，左侧导航仍为一项 | 误改成三个 page |
| V2 | 页面内存在三个 tab，`activeKey` 可切换且切换不重新挂载页面壳 | tab 实现退化为路由跳转 |
| V3 | 存储 tab 显示 12 行观测表，除 `available` 的行外全部为 `notImplemented` 且**带原因**（R4 前 11 行、R4 后 10 行） | 用 0 冒充读不到 |
| V4 | 恒等式 `totalBytes == referencedOnceBytes + sharedBytes + Σ orphans.bytes` 在后端单测中成立 | 账本不闭合 |
| V5 | 跨会话共享附件在会话分组区有「共享」标注 | 用户看到会话合计大于总量 |
| V6 | 进程 tab 只显示树级数字（PID / 父 PID / `exeName` / CPU / 内存），**无任务名、无停止按钮、无排队态、无角色列** | 越界到任务归因 |
| V7 | 页面内无「总占用」聚合数字 | 跨 tab 相加 |
| V8 | `AttachmentEntry` 时间字段不再叫 `created_at` 却装 mtime | 命名撒谎 |
| V9 | `get_resource_capabilities` 响应不含任何 `bytes` / `count` 字段 | 能力声明与数字混装 |
| V10 | `settingsSnapshot.golden.test.ts` 通过，两份 fixture 已同步更新 | 用跳过测试替代 fixture 更新 |
| V11 | 全后端 `AppDirs` 之外无新增目录字面量；无新增 `dirs::` 调用点 | I44（已归档）门禁失效 |
| V12 | `npm run contract:gen` 已跑，`src/types/protocol.contract.ts` 已提交 | 手改生成类型 |
| V13 | 两个 tab 的数字出自同一 `sampledAt`；同一刷新间隔内的多次调用返回同一份缓存快照（有单测断言两次调用的 `sampledAt` 相同） | 两个采样器，数字不可比 |
| V14 | 子树遍历有单测覆盖**孙进程**（构造三层父子链，断言孙进程出现在结果里） | 只取直接子进程 |
| V15 | PID 复用校验有单测：构造「父已死、PID 被无关进程复用」的用例，断言该进程**被排除**且标记 | 误归属 |
| V16 | 内存合计仅在 `memoryAdditive == true` 时显示；否则带「含共享页重复计数」标注；内存列名一律带口径 | 工作集求和当物理占用 |
| V17 | 自身占用 tab 的峰值区显示 `generatedAt` 与 `attributionNote`，且无「实时峰值」字样 | 进程内自采样峰值 |
| V18 | `assistant-resource-probe` 的依赖不含 `dirs`，且不引用应用层任何模块 | 采样 crate 越界 |
| V19 | 子树推导只发**一次**全系统快照调用（有单测断言调用次数为常数，不随进程数增长） | 逐 PID 递归查询，O(N) 次系统调用 |
| V20 | 过滤按祖先链判定；单测构造「同名镜像但不在子树内」的进程，断言其**被排除** | 按镜像名过滤，收进别的应用的进程 |
| V21 | 最后一个消费者移除后采样停止、缓存清空；再次注册时从空缓存重新开始（首拍 CPU 为 `null`） | 后台常驻轮询 |
| V22 | 未被任何消费者声明的指标不出现在响应里（输出 `null`，不填 `0`） | 指标位掩码失效，白算 |
| V23 | 归因父链遍历带 `visited` 集合；单测构造成环的父子关系，断言**报错而非死循环** | 应用层归因树成环 |

---

## 12 分批

| 批次 | 内容 | 前置 |
|---|---|---|
| **R0** | 页面身份变更（label / keywords / golden fixture）+ 页内 tab 骨架 + `get_resource_capabilities` | 无 |
| **R1** | 存储 tab 观测表 12 行 + D1/D2/D4 + 口径字段进 DTO | R0 |
| **R2** | 账本闭合（`sharedBytes` / `referencedOnceBytes` + 恒等式断言 + 共享标注） | R1 |
| **R3** | `created_at` 语义修正（§10 第 5 项定案后） | R1 |
| **R4** | `assistant-crash` 补 `list` / `stat` / `prune`（含保留上限），`crashes/` 行转 `available` | **无**（可完全并行，与本监视器无依赖） |
| **R5** | `assistant-resource-probe` 骨架 + 平台方案定案（§10 第 8 项）+ **一次全系统快照 + 内存父子表 + 从根 DFS**（§2.3）+ 角色分类 + PID 复用校验 + 空闲超时停表（§6.5.2）+ 指标位掩码（§6.5.1） | 平台方案定案 |
| **R6** | 进程 tab 树级数字（投影 B）+ 同一 `sampledAt` 共用 + 首拍 `null` 的「待定」渲染态 | R5 |
| **R7** | 自身占用 tab 当前值（投影 A）+ `memoryAdditive` 驱动内存合计显示 + 内存列名带口径 | R5 |
| **R8** | 进程外报告二进制（C）+ `get_self_usage_report` + 峰值区报告阅读器（`noReportYet` / `staleReport`） | R7 |
| **推迟** | 任务级归因（需登记表）、D5 资格、D6 策略 | — |

**R0 必须先做**：页内 tab 骨架是后面所有内容区的容器；先改 label 与 fixture 也能让「资源监视器」这个名字在最早一批就生效，避免文档与 UI 长期不一致。

**R4 可以插队**：它是 `crashes/` 作为 owner 的欠账（N14），不依赖监视器的任何东西，也不需要 R0。把它排在 R5 之前还有个额外好处——**先修好一个「无上限产物」的目录，再谈给自监控加落盘**，顺序反过来容易顺手把新产物写进 `crashes/`（§7）。

**R8 不可提前**：峰值区是报告阅读器，报告由进程外二进制生成；先做阅读器而没有产物，UI 会长期停在 `noReportYet`，用户会以为功能坏了。

---

## 13 开放问题

| # | 问题 | 影响 |
|---|---|---|
| OQ1 | `security/audit/` 是否先进 I44（已归档）的归属表 | 不进表则观测表以「代码事实」覆盖「规格事实」，漂移被固化 |
| OQ2 | 观测表 12 行的 `key` 命名规范（`attachments` vs `attachments/derived` 混用层级分隔符） | 前端分组与 i18n 映射 |
| OQ3 | 存储 tab 是否需要「按层折叠」（data 根 / local 根） | 12 行的纵向长度 |
| OQ4 | 能力声明是否需要缓存 | 每次打开设置页都调一次命令，成本可忽略，但需定案 |
| OQ5 | 进程 tab 与「工具运行面板」是否共享组件 | 两处都展示子进程，重复实现风险 |
| OQ6 | 采样 crate 的平台单测是否进 CI（Windows 专有部分如何门禁） | 三平台实现差异无法在单机验证 |
| OQ7 | 外部报告产物是否需要 schema 版本号 | 产物是跨版本持久文件，格式演进会让旧报告解析失败 |
| OQ8 | 进程 tab 是否将来扩展为「全机视图」 | 当前明确只覆盖子树（§5.4）；扩到全机需重新定案隐私与权限边界 |
| OQ9 | `crashes/` 的保留上限取多少 | R4 需一个具体值，且必须与 `logs/` 的 `KeepSome(3)` 语义对齐或明确说明差异 |
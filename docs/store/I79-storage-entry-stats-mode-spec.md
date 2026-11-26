# I79 — 存储条目异步统计规格

| 项 | 值 |
|---|---|
| 状态 | 已定稿（OQ1–OQ4 已决，待实施） |
| 日期 | 2026-11-26 |
| 类型 | 设计规格（采样分档、缓存状态机、契约字段、一致性护栏、验收） |
| 依据 | [CODE-REVIEW-PRINCIPLE](../principle/CODE-REVIEW-PRINCIPLE.md) D1（边界与确定行为）、D2（契约单一来源）、D3（失败可分类）、D4（职责边界）、D7（并发 / 异步 / 时序）、D8（状态与资源生命周期）、D10（性能与资源占用）、D11（安全与隐私）、D15（平台兼容与国际化）；[CODE-SPLIT-PRINCIPLE](../principle/CODE-SPLIT-PRINCIPLE.md)（模块职责）；[doc-naming-spec](./doc-naming-spec.md)（本文文件名） |
| 关联 | I61 存储管理器规格（已归档）：沿用其「所有者 / 观测者 / 回收者」三分与「管理器只发起」原则，本文只改其 §3.10 的单条 500ms 超时口径与 §3.11 的「同响应同源」边界；I67 存储余量对账规格（已归档）：§3.5 / §3.6 的减法与下界口径沿用；I77 工具临时区所有者与用户清空规格（待归档）：`E19` 已 owner 化，本文不改其行为；[I75 启动分层规格](./I75-startup-layering-spec.md)：后台补算**不进**启动关键路径（§8.3） |
| 影响范围 | 后端：`src-tauri/src/app_dirs/registry.rs`、`src-tauri/src/cmd/storage/{sample/mod.rs, sample/collect.rs, sample/timeout.rs, sample/cache.rs（新增）, entries/{mod.rs, view.rs, assemble.rs}, section.rs, inventory/{cache.rs, subtract.rs}, reclaim/{plan.rs, execute.rs}, mod.rs}`、`src-tauri/src/boot/build/state.rs`、`src-tauri/src/protocol/events.rs`、`src-tauri/src/feature_plugins/vault_plugin.rs`、`src-tauri/crates/assistant-protocol/src/storage.rs`；前端：`src/settings/pages/storage/{useStorageSnapshot.ts, entryView.ts, SnapshotToolbar.tsx}`、i18n 两份 locale、`src/types/protocol.contract.ts`（生成）。**无新增 crate、无新增第三方依赖** |
| 一句话 | 把「请求内算不动」的条目改成**主动声明的异步档**：声明为 `Deferred` 的条目只读缓存、零等待返回，缺失 / 过期由后台单飞 worker 补算并落缓存、推事件；`Inline` 档保留预算，超时降级为「声明说谎」的告警探针。 |

**文档性质**：设计规格。本文只定契约、判据与不变量，不记进度。实施顺序见 §11，验收见 §14。

**编号约定**：`D1–D10` 判定、`INV1–INV13` 不变量、`R0–R3` 实施批次、`X1–X7` 拒绝方案、`OQ1–OQ7` 开放问题（§13.1 已决 / §13.2 残余）、`V1–V26` 验收判据。跨文档引用一律带文档前缀（`CODE-REVIEW-PRINCIPLE D7`、`I61 §3.10`），裸写 `§` 一律指本文。`D*`（审查维度）沿用 CODE-REVIEW-PRINCIPLE 定义，本文不重复。

本文中的「必须 / 禁止」为可验收约束；「应当」为默认做法。

---

## 1 现状与问题

### 1.1 现有机制（事实）

| 环节 | 现状 | 后果 |
|---|---|---|
| 采样编排 | `sample::collect_with` 对除 `E1` 外每条一个任务，`futures::join_all` 并行 | 整页延迟由**最慢条目**决定 |
| 单条预算 | `SAMPLE_TIMEOUT = 500ms`，`sample/timeout.rs` 的 `timed` / `timed_blocking` | 慢条目每次必然等满 500ms |
| 超时语义 | `spawn_blocking` **不可 abort**，超时只是放弃等待；线程仍会把整棵树遍历完 | 白算一遍，结果被 `timeout_sample()` 丢弃 |
| 结果复用 | 每次刷新重新起任务，无缓存 | 同一棵树被反复重算，成本重复支付 |
| `complete` | `entries/assemble.rs` 把「数字必然滞后」的条目从 `complete` 判据中**按 core 枚举排除**（现状只有 `E12` / `E14`） | 该排除是「数字必然滞后」的补丁，未解决根因；本文改为按 `StatsMode` 判据统一处理（§7.2） |

### 1.2 契约里已有、后端从未产出的两个取值

`assistant-protocol/src/storage.rs` 的 `StorageAvailabilityDto` 已含 `NoReportYet` / `StaleReport`；前端也已接好：

| 位置 | 现状 |
|---|---|
| `src/settings/pages/storage/entryView.ts` | `staleReport → warning`、`noReportYet → neutral` 的 tone 已定义 |
| i18n `zh-CN` / `en-US` | 已有「暂无统计 / 数据已过期」文案 |
| 后端 | 全仓没有任何地方产出这两个取值（只有枚举定义与前端消费） |

结论：**「尚未算完」与「算了但过期」在契约里本来就有位置，缺的是缓存与后台 worker，而不是新枚举值。**

### 1.3 三条不可动的约束

| # | 约束 | 代码位置 |
|---|---|---|
| C1 | `E1` 与顶层字段同源，账本恒等式 `total = once + shared + orphan` 只依赖那一份 `list_all`；失败即整命令 `Err` | `cmd/storage/ledger.rs` 的 `build_base`、`cmd/storage/mod.rs` 的 `get_storage_breakdown` |
| C2 | 回收计划必须**新鲜**：`plan_storage_reclaim` 现场重采样，执行时 owner 再重判 | `cmd/storage/reclaim/plan.rs`、`reclaim/execute.rs` |
| C3 | 余量减法要**同源**：减数就是同一响应里的条目值；`null` 只表示下界，绝不填 `0` | `cmd/storage/inventory/subtract.rs` 的 `subtract_declared`、`inventory/cache.rs` 的 `scope_totals_dto` |

任何异步化方案都不得破坏这三条。§8 把它们写成不变量。

---

## 2 判定

| # | 判定 | 依据 |
|---|---|---|
| D1 | 引入 `StatsMode::{Inline, Deferred}` 作为**路由声明**，语义是「这条在不在响应内算」，不是「这条大概多慢」 | §4.1；耗时估计随机器与数据量漂移，无法诚实声明 |
| D2 | 分档判据按**计算形状**：子树大小有上界且无第三方写入 → `Inline`；否则 → `Deferred` | §3.1；判据与实现可验证，不依赖实测 |
| D3 | 声明位置：core 在 `ManagedRoot`、分区在 `SectionEntry`（与 `scope` / `inventory` 同层），由 `EntryView` 透传；**禁止**放进 `EntryStatic`（`entries/decl.rs`）与 `EntryDecl`（回收静态声明），**禁止**在管理器里按 `key` 字面量分档 | §4.2；`EntryStatic` / `EntryDecl` 是「能不能回收」的静态表，`stats_mode` 是「怎么取数」，两种语义不得混表 |
| D4 | `Deferred` 条目在请求内**只读缓存**，零等待；首次 / 过期由后台 worker 补算 | §5、§6 |
| D5 | 缓存**只由 worker 写**；读路径与计划命令都不写缓存 | §6.1、INV6；状态写入唯一（CODE-REVIEW-PRINCIPLE D8） |
| D6 | `noReportYet`（尚未算）与 `unreadable` / `unavailable`（算了失败）必须是不同取值；成功但过期回放旧数字 + `staleReport` | §5.2；否则「不知道」与「出错」再次混成一种 |
| D7 | `E1` 必须 `Inline`，并用单测锁死；账本与减法不读缓存 | §8.1 |
| D8 | 计划命令**绕过缓存**，按请求 key 现场采样；超时 → `judgeable = false`，不回写缓存 | §8.2 |
| D9 | 减法引入「登记值年龄上限」：超过即按未知（下界），禁止凭空余量 | §8.4 |
| D10 | 后台完成事件**按批合并**，前端重拉采用尾随刷新；禁止每完成一条触发一次整页重拉 | §6.3、§9.1 |

---

## 3 分档判据与条目归属

### 3.1 判据

> **子树大小有上界且无第三方写入 → `Inline`；否则 → `Deferred`。**

补充口径：

1. 单文件 / 单层文件枚举（`metadata`、`read_dir` 第一层）成本与数据量近似无关 → `Inline`。
2. 递归整树、条目数由用户行为或外部进程决定（派生侧车、工具结果、索引、日志、WebView profile、临时区、功能分区） → `Deferred`。
3. 「有上界」指**结构性上界**（文件数固定、由登记表决定），不是「一般不会太大」。
4. 判据只看**计算形状**，不看「要不要在同一次响应里另算一遍」：即使某条目本身便宜，若它的取数会重复已付的遍历成本（如 `E2` 复用 `list_all`），也归 `Deferred`。

### 3.2 core 条目归属

| 条目 | 档 | 理由 |
|---|---|---|
| `E1` `attachments` | `Inline` | C1：账本来源，必须与顶层字段同源；`list_all` 已在命令层做过一次 |
| `E2` `attachments/derived` | `Deferred` | `list_derived` 内部会再枚举一次附件目录（`bases = list_all()`），即打开一次页面附件目录被扫两遍；条数由附件数驱动，无结构上界。`E1` 已付的成本不得在响应内再付一遍 |
| `E3` `tool-results` | `Deferred` | 递归整树 + `collect_expired` 二次遍历；规模由会话与工具调用量决定 |
| `E4` `tool-index` | `Deferred` | `index_stats` → `dir_tree_stats` 是**递归**遍历（不是单层）；索引文件数 = 被工具读写过的源文件数 × 修订数，由用户行为驱动、无结构上界 |
| `E5` `mascot-assets` | `Inline` | 素材类型是封闭枚举、单文件有 `MAX_ASSET_BYTES` 上限，递归但结构性有界 |
| `E6` `assistant.db` | `Inline` | 单文件 `metadata` |
| `E12` `logs` | `Deferred` | 递归整树 + 外部插件持续写入 |
| `E13` `crashes` | `Inline` | 递归但条数按崩溃事件增长，量级极小（弱上界，实测超预算即改档） |
| `E14` `EBWebView` | `Deferred` | 整棵树由 WebView2 接管，规模无上界且持续写入 |
| `E15` / `E17` / `E18` 配置文件 | `Inline` | 单文件 `metadata` |
| `E19` `tool-scratch` | `Deferred` | 会话段 + `shared` 段整根，规模无上界（I77） |

### 3.3 分区条目

全部分区条目（`E7`–`E11` / `E16`）为 `Deferred`：规模由功能侧数据决定，且带**锁语义**（vault 锁定时 `stats()` 返回 `Locked`）。分区条目在 `SectionEntry` 上自述档位，core 不识别任何分区 `key`（沿用 I61 的 G17(b)）。

---

## 4 声明与数据结构

### 4.1 `StatsMode`

```rust
/// 采样档：回答「这条在不在响应内算」，不回答「这条大概多慢」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsMode {
    /// 在响应内计算；`SAMPLE_TIMEOUT` 是它的预算，超时视为声明说谎（§4.4）。
    Inline,
    /// 不在响应内计算：读缓存，缺失 / 过期由后台补算（§5、§6）。
    Deferred,
}
```

定义位置：`src-tauri/src/app_dirs/registry.rs`（与 `RootScope` / `RootCoverage` / `RiskTier` 同层），供 core 与分区共用。

### 4.2 声明位置

| 载体 | 新增字段 | 说明 |
|---|---|---|
| `ManagedRoot`（core 登记表） | `stats_mode: StatsMode` | 与 `risk` / `inventory` 并列的登记事实 |
| `SectionEntry`（分区条目声明） | `stats_mode: StatsMode` | 与 `scope` / `coverage` / `inventory` 同层 |
| `EntryDecl`（分区回收静态声明） | **禁止**新增 | 只承载「能不能回收」 |
| `EntryView` | `stats_mode: StatsMode` | 采样、装配、减法只认这一份清单 |
| `EntryStatic`（`entries/decl.rs`） | **禁止**新增 | 见 D3 |
| wire（`StorageEntryDto`） | **禁止**新增 | 前端只认 `availability`，见 §7.1 |

### 4.3 `EntrySample.observed_at`

`entries/mod.rs` 的 `EntrySample` 增加：

```rust
/// `None` = 本次响应内实测（`Inline` 或计划强制采样）；
/// `Some(t)` = 该条目最近一次观测 / 尝试的时刻（秒），来自缓存。
pub observed_at: Option<i64>,
```

三个构造器（`observed` / `missing` / `degraded`）保持 `None`；只有缓存读路径与 worker 落库时写 `Some`。**禁止**用与样本平行的第二个数组传时刻（X2）。

### 4.4 `Inline` 预算的角色变化

`SAMPLE_TIMEOUT`（500ms）保留，但语义从「业务降级阈值」变为「声明说谎探针」：

- 超时仍降级该条为 `unavailable`（不拖整页）；
- 追加 `log::warn!("entry {key} declared Inline but exceeded {timeout:?}")`；
- 该日志出现即表示 `stats_mode` 填错或 owner 实现退化，应改档或修 owner，而不是调大超时。

---

## 5 读路径

### 5.1 采样编排

`sample/collect_with` 增加模式参数：

```rust
pub enum SampleMode<'a> {
    /// 页面快照：`Inline` 现场采样，`Deferred` 读缓存（§5.2）。
    Page,
    /// 回收计划：`Deferred` 也现场采样；`force = None` 表示清单内全部条目。
    Plan { force: Option<&'a [&'static str]> },
}
```

`Page` 模式的步骤：

1. `views(&sections)` 定清单（core 在前，分区紧随）；
2. `Inline` 条目按现状并行采样，保留 `timed` / `timed_blocking`；
3. `Deferred` 条目：`EntryStatsCache::read(now, views)` → 返回 `(samples, triggers)`；
4. 对 `triggers` 调用 `trigger_stats(app, &triggers)`（幂等，§6.1）；
5. 返回 `Sampled`（`views` 与 `samples` 仍严格一一对齐）。

句柄缺失的条目（`E3` / `E19` 在特性关闭构建下）**不进缓存**：仍按现状返回 `NotImplemented` 降级，也不触发补算。

### 5.2 缓存状态机

单条缓存记录：

```rust
struct EntryStat {
    observed_at: i64,             // 最近一次尝试时刻（成功或失败）
    availability: StorageAvailabilityDto,
    bytes: Option<u64>,
    count: Option<u64>,
    oldest_at: Option<i64>,
    newest_at: Option<i64>,
    reclaimable: Option<(u64, u64)>,
    policy: Option<ReclaimPolicyDto>,
    eligibility: Option<(StorageEligibilityDto, StorageReclaimReasonDto)>,
    /// 单飞 + 租约：`(token, started_at)`；`None` 表示无任务在跑（§6.1 约束 5）。
    running: Option<(u64, i64)>,
}
```

读映射（`now` 取响应时刻；`age = now - observed_at`）：

| 缓存状态 | `availability` | 数字 | 触发补算 |
|---|---|---|---|
| 无记录 | `noReportYet` | 全 `null` | 是 |
| 成功，`age < ENTRY_STATS_TTL_SECONDS` | 原值（`available` / `missing`） | 原值 | 否 |
| 成功，`age >= ENTRY_STATS_TTL_SECONDS` | `staleReport` | **原值**（旧数字仍可展示） | 是 |
| 失败（`unreadable` / `unavailable`），`age >= ENTRY_STATS_RETRY_SECONDS` | 原失败类别 | 全 `null` | 是 |
| 失败，`age < ENTRY_STATS_RETRY_SECONDS` | 原失败类别 | 全 `null` | 否 |
| `locked` | `locked` | 全 `null` | 否（等解锁失效，§8.5） |
| `notImplemented` | `notImplemented` | 全 `null` | 否（重试无意义） |
| 正在补算且无成功值 | `noReportYet` | 全 `null` | 否（已 in-flight） |
| 正在补算且有成功值，`age >= TTL` | `staleReport` | 原值 | 否（已 in-flight） |
| 正在补算，已超时但闭包未结束 | `unavailable` | 全 `null` | 否（同一任务仍在跑，结束后覆盖，约束 4） |

约束：

1. `staleReport` **只替换可用性取值，不改数字**；数字的年龄由 `observed_at` 表达（§7.1）。
2. 失败不覆盖上一次成功数字：失败时整条替换为 `null` + 失败类别（「上次成功值」不作为失败时的展示值，避免把过期数字当当前值）。
3. `read` 只读，不修改任何状态（INV6）。
4. **超时标记可被同一任务覆盖**（约束 2 的唯一例外）：`ENTRY_STATS_JOB_TIMEOUT_MS` 到期先写一条 `unavailable` 让 UI 脱离「统计中」；该阻塞闭包真正跑完后，用真值 `finish` 覆盖它（§6.1）。因此「超时」不是终态，只是「这条比预期慢」的临时占位。
5. 超时标记与后续覆盖必须来自**同一任务**：`finish` 只在 `running` 仍属于该任务时才生效，被 `invalidate` 或新任务接管的旧结果直接丢弃（避免旧值覆盖新值）。

### 5.3 常量

| 常量 | 值 | 理由 |
|---|---|---|
| `ENTRY_STATS_TTL_SECONDS` | `60` | 前端无轮询（只在初次 / 手动刷新 / 删除后 / 事件到达时读），TTL 只决定「重开或点刷新时要不要重算」；60s 保住「打开即接近新鲜」，重算已被单飞与并发上限限流。不对齐 `SCAN_TTL_SECONDS`：扫描是整棵 data/local 树，观测只是几棵子树 |
| `ENTRY_STATS_RETRY_SECONDS` | `30` | 失败退避，避免每次重拉都重试 |
| `ENTRY_STATS_JOB_TIMEOUT_MS` | `5000` | 单条后台任务的 **UI 占位** 时限（写 `unavailable`），不是任务终止时限；闭包跑完仍会覆盖（§5.2 约束 4） |
| `ENTRY_STATS_MAX_INFLIGHT` | `2` | 后台并发上限，避免多个整树遍历同时抢磁盘；实测填充慢再抬到 3–4 |
| `ENTRY_STATS_LEASE_SECONDS` | `120` | `running` 租约：owner 真卡死时到期允许重试，避免两条卡死条目永久占满 permit；重试可能重复只读遍历，无害 |
| 减法年龄上限 | `SCAN_TTL_SECONDS`（`300`，**引用同一常量**） | 减法两侧时间窗一致，见 §8.4 |

---

## 6 写路径

### 6.1 触发与单飞

新增 `src-tauri/src/cmd/storage/sample/cache.rs`：

```rust
#[derive(Clone, Default)]
pub struct EntryStatsCache { /* Arc<Mutex<State>> */ }

impl EntryStatsCache {
    /// 只读：返回 (samples, 需要补算的 key)。`samples` 与 `views` 一一对齐。
    pub fn read(&self, now: i64, views: &[EntryView]) -> (Vec<EntrySample>, Vec<&'static str>);

    /// single-flight + 租约：`running` 且未超租约时返回 `None`（合并，不排队）；
    /// 否则分配新 token 并返回。token 用于「只有当前任务能结算」。
    fn begin(&self, key: &'static str, now: i64) -> Option<u64>;

    /// 结算：token 不匹配（已被 invalidate 或新任务接管）则丢弃，不覆盖新值。
    fn finish(&self, key: &'static str, token: u64, sample: EntrySample, at: i64);
    fn fail(&self, key: &'static str, token: u64, availability: StorageAvailabilityDto, at: i64);

    /// 回收成功后按条目失效（§8.5）。
    pub fn invalidate(&self, keys: &[&str]);
    /// vault 锁 / 解锁等整体失效（§8.5）。
    pub fn invalidate_sections(&self);
}

/// 幂等触发：只为 `begin` 成功且仍有需要的 key 起任务。
pub fn trigger_stats(app: &tauri::AppHandle, keys: &[&'static str]);
```

约束：

1. **取数实现唯一**：worker 只调用 `sample/rows.rs` 既有函数（core 按 `view.core` 穷尽匹配、分区按 `view.section` 路由），**禁止**在 `cache.rs` 里写第二套 `read_dir` / owner 调用。
2. 句柄在**触发时**取一次（`AppDirs::resolve`、`SectionRegistry::snapshot`、`boot::tool_exec`、`boot::tool_scratch`、`DbState`），与 `inventory::trigger_scan` 同范式。
3. 有界并发：`tokio::sync::Semaphore::new(ENTRY_STATS_MAX_INFLIGHT)`（`tokio` 已启用 `full`，无新增依赖）。permit 在**闭包结束时**释放。
4. **闭包自结算**（关键）：`finish` / `fail` / 清 `running` / 释放 permit 全部放进 `spawn_blocking` 的闭包内部，而不是放在 `await` 之后。超时只是放弃 `await`，线程跑完仍会自己落库——因此「白算」不再发生，结果永不丢弃。任务外层的 `tokio::time::timeout` 只做 UI 占位（写 `unavailable`），不参与结算。
5. **租约**：`running` 携带 `(token, started_at)`；`now - started_at >= ENTRY_STATS_LEASE_SECONDS` 时 `begin` 视为失效并分配新 token。旧任务结束时 token 不匹配 → 结果丢弃（§5.2 约束 5）。这保证 owner 真卡死不会永久占满 permit。
6. `spawn_blocking` 不可 abort：超时只是放弃等待，线程仍会跑完——因此 owner stats 继续必须「只读、幂等、不持锁长遍历」（沿用 I61 §3.10 第 3 条）。**不为 owner API 加 deadline / cancel**：需要改 5 处入口并给递归遍历逐层穿 token，而代价仅是「白跑一次本来就要跑的只读 IO」，收益与改动面不成比例（OQ4）。

### 6.2 事件

- 常量：`src-tauri/src/protocol/events.rs` 新增 `STORAGE_ENTRY_STATS_UPDATED = "storage:entry-stats-updated"`；
- 载荷：`()`（不带数据），**本期定稿，非「后续优化」**：`inventory` 的 `declared` 数字由当前响应里的条目样本派生（`node_dto(node, views, samples)`），若前端按 key 局部打补丁，会出现「条目新、对账旧」的不同源；要补就得在前端重算聚合，等于把减法逻辑复制一份。整页重拉是唯一能保住同响应同源的做法（与 X3 同一理由）；
- 时机：worker 一个 drain 周期（≤ 250ms）内完成 / 失败的条目**合并为一次事件**，禁止逐条 emit（D10）；
- 失败也发事件（前端据此把「统计中」变成失败类别），但 `locked` / `notImplemented` 不触发补算、也不发事件。

### 6.3 启动期

**禁止**在启动关键路径预热统计（I75 启动分层）。首次打开设置页时 `noReportYet` + 后台补算是唯一入口；如需预热，只能挂在空闲任务且可整体裁掉。

---

## 7 契约变更

### 7.1 `StorageEntryDto.observed_at`

```rust
pub struct StorageEntryDto {
    // ... 现有字段不变 ...
    /// 该条数字的观测时刻（秒）。
    /// `Inline` = 本次响应的 `sampled_at`；`Deferred` 首次尚未算完时为 `null`。
    pub observed_at: Option<i64>,
}
```

约束：

1. 输出 DTO 的 `Option` **不加** `skip_serializing_if`：键恒在、值 `null`；前端类型为 `number | null`（必填键）。
2. 生成类型一律 `npm run contract:gen`，禁止手改 `src/types/protocol.contract.ts`。
3. `sampled_at`（响应时刻）语义不变；`observed_at` 是**单条**的年龄，二者不得混用。

### 7.2 `complete` 语义收窄

`entries/assemble.rs` 的 `complete` 改为：

```rust
let complete = views.iter().zip(samples.iter()).all(|(view, sample)| {
    view.stats_mode == StatsMode::Deferred
        || matches!(
            sample.availability,
            StorageAvailabilityDto::Available | StorageAvailabilityDto::Missing
        )
});
```

- 删除现有对 `E12` / `E14` 的 core 枚举特判（它们已是 `Deferred`）；
- `complete` 只表达「**同步档全部有结论**」；异步档的年龄由各条 `availability` / `observed_at` 表达；
- 前端当前未消费 `complete`，本次不改前端行为。

### 7.3 `availability` 取值

不新增取值，启用既有两个：

| 取值 | 语义 | 数字 |
|---|---|---|
| `noReportYet` | 尚未算过（首次 / 失效后） | `null` |
| `staleReport` | 算过但已过期，**数字仍可展示** | 旧值 |
| `locked` | 分区自述受保护 | `null` |
| `unreadable` / `unavailable` | 算了但失败 | `null` |

同时修正 `StorageAvailabilityDto` 的注释「存储条目只用前六个值」——本次起八个值全部在用。

---

## 8 一致性护栏（不变量）

| # | 不变量 | 落地方式 |
|---|---|---|
| INV1 | `E1` 恒 `Inline` | `registry.rs` 单测：`MANAGED_ROOTS` 中 `Attachments.stats_mode == Inline`；改档即测试红 |
| INV2 | 账本与减法不读缓存 | `E1` 由调用方给的 `list_all` 派生（现状不变）；减法只读同响应 `samples` |
| INV3 | 计划绕过缓存且不回写 | `SampleMode::Plan` 对 `Deferred` 现场采样；plan 路径不调用 `finish`（§8.2） |
| INV4 | 减法年龄上限 | 任一参与减法的样本 `observed_at` 年龄 `>= SCAN_TTL_SECONDS` → 该条 `declared` 视为未知（`null`），`residual` 随之为 `null`（下界） |
| INV5 | 回收成功后按条失效 | `execute_storage_reclaim` 对 `removed_bytes > 0` 的结果调 `invalidate(&[key])`；`E14` 只登记待执行，`removed_bytes == 0`，自然不失效 |
| INV6 | 缓存单写者 | 只有 `finish` / `fail` / `invalidate` 改状态；`read` 纯只读 |
| INV7 | 事件按批合并 | worker drain 周期合并，≤ 250ms 一次 |
| INV8 | `Deferred` 不参与 `complete` | §7.2 |
| INV9 | 锁定状态不缓存数字 | 分区返回 `Locked` 时落 `availability = locked` + 全 `null` |
| INV10 | 声明说谎可观测 | `Inline` 超时 `log::warn!`（§4.4） |
| INV11 | 结算权唯一（token） | `begin` 分配 token；`finish` / `fail` 仅在 token 仍匹配时生效，被 `invalidate` 或新任务接管的旧结果直接丢弃（§5.2 约束 5、§6.1 约束 5） |
| INV12 | 结果不丢弃 | `finish` / `fail` / 清 `running` / 释放 permit 全在 `spawn_blocking` 闭包内部完成；超时只做 UI 占位（§6.1 约束 4） |
| INV13 | 附件删除后 `E2` 立即失效 | `delete_attachments` 在 `removed_count > 0` 时 `invalidate(&["attachments/derived"])`（§8.5） |

### 8.1 `E1` 与账本

`E1` 保持在响应内由调用方传入的 `entries` 派生（`attachment_sample`），不经缓存、不落后台。`build_base` 与 `E1` 同源的不变量（I61 §3.10 第 1 条）不变。

### 8.2 计划命令

`plan_storage_reclaim` 改为：

1. 先取 `views(&ports.sections)` 并校验请求 key（未知 / 重复整体拒绝），**再采样**——避免坏请求付出采样成本；
2. `collect_with(..., SampleMode::Plan { force })`：`force = None`（请求为空 = 清单全部）或请求的 key 集合；
3. `Deferred` 条目在计划模式下现场采样，预算 `PLAN_SAMPLE_TIMEOUT = 2000ms`；
4. 超时 → `unavailable` → `judgeable = false`（`StorageReclaimPlanDto.complete = false`）；
5. 计划采样**不写** `EntryStatsCache`（INV3）：统计缓存只由 worker 写，避免「计划一次把页面数字也改了」；计划自身的 `CachedPlan`（执行前校验用，`reclaim/store.rs`）行为不变。

### 8.3 启动期

见 §6.3。

### 8.4 减法下界

`inventory/subtract.rs` 的 `subtract_declared` 增加 `now` 参数：

```rust
pub fn subtract_declared(
    node: &RawNode,
    views: &[EntryView],
    samples: &[EntrySample],
    now: i64,
) -> (Option<u64>, Option<u64>)
```

规则：

- `sample.observed_at == None` → 本次响应实测，视为新鲜，照旧参与减法；
- `Some(at)` 且 `now - at >= SCAN_TTL_SECONDS` → 该样本的 `bytes` / `count` 按 `null` 参与（结果整体 `None`）；
- `sample.bytes == None` 已由现状折成 `None`（下界），不新增分支。

理由：减法两侧允许的时间窗应当一致——扫描值本身允许 300s 的年龄（`SCAN_TTL_SECONDS`），登记值若也允许 300s，则两侧同一量级；超过即降为下界，而不是把「很久以前的登记值」减在「现在的扫描值」上，产出**凭空余量**。

### 8.5 失效点

| 触发 | 动作 | 位置 |
|---|---|---|
| 回收执行成功且 `removed_bytes > 0` | `invalidate(&[key])` | `reclaim/execute.rs` |
| **附件删除成功**（`E2` 的孤儿判据依赖附件集合） | `invalidate(&["attachments/derived"])` | `cmd/storage/mod.rs` 的 `delete_attachments`（`removed_count > 0` 时） |
| vault 锁定 / 解锁 | `invalidate_sections()` | `feature_plugins/vault_plugin.rs`（事件发出处） |
| TTL 到期 | 读路径自动触发补算 | `sample/cache.rs` |
| 特性卸载 / 分区不再注册 | 读路径按当前 `views` 过滤，不主动清理 | `sample/cache.rs` |

`locked` 条目不靠 TTL 重试：锁定时重试无意义，解锁事件才失效（INV9）。

`E2` 必须显式失效的理由：`list_derived` 的 `orphan` 判据是「派生文件找不到对应基附件」，删附件会让原本非孤儿的侧车立刻变孤儿；不失效则 `E2` 的可回收量最多滞后一个 `ENTRY_STATS_TTL_SECONDS`，与「删完立刻能清」的用户预期冲突。`E2` 归 `Deferred` 后，这是唯一新增的显式失效点。

---

## 9 前端

### 9.1 订阅与尾随刷新

`src/settings/pages/storage/useStorageSnapshot.ts`：

- 新增 `STORAGE_ENTRY_STATS_UPDATED = "storage:entry-stats-updated"` 常量并 `listen`；
- 回调复用现有 `load("refresh")`；
- **尾随刷新**：事件到达时若已有请求在飞（`inFlightRef` 非空），登记一次 `pendingRef`，当前请求结束后再跑一次。现状是「在飞即丢弃」，会丢掉最后一次完成事件。

**不新增「强制刷新统计」命令**（本期定稿）：工具栏的刷新按钮保持现状（重新拉取，`Deferred` 条目按 TTL 决定是否重算）。`ENTRY_STATS_TTL_SECONDS = 60` 下最坏陈旧度 60s，且每行有 `observedAt` 明示年龄，为此新增命令或给 `get_storage_breakdown` 加入参不值得。若将来把 TTL 放宽到 300，则**必须**补一个与 `refresh_storage_inventory` 同构的镜像命令（`refresh_storage_entry_stats(app) -> ()`，只触发后台、类型层面不回数字），不要给现有命令加入参。

### 9.2 展示

| 项 | 现状 | 本次 |
|---|---|---|
| `noReportYet` badge | neutral + 「暂无统计」 | 文案改「统计中」（en: `Measuring…`）；补算失败会转成失败类别，不会永久停在「统计中」 |
| `staleReport` badge | warning + 「数据已过期」 | 不变 |
| 详情 `observedAt` | 无 | 新增一行「统计于 {time}」；`null` 时不显示该行 |
| 工具栏 `sampledAt` 文案 | 「采样于」 | 改「快照于」：引入 `Deferred` 后它只是响应时刻，不再是全部条目的采样时刻，继续叫「采样于」会与各条的 `observedAt` 自相矛盾 |

文案只改 i18n 两份 locale，不在代码里写中文（CODE-REVIEW-PRINCIPLE D15）。

---

## 10 改动面清单

| 文件 | 改动 |
|---|---|
| `src-tauri/src/app_dirs/registry.rs` | `StatsMode` 定义；`ManagedRoot.stats_mode` + 13 条登记；`E1` 恒 `Inline` 单测 |
| `src-tauri/src/cmd/storage/section.rs` | `SectionEntry.stats_mode`（分区自述） |
| `src-tauri/src/cmd/storage/entries/view.rs` | `EntryView.stats_mode`（core 取登记表、分区取自述） |
| `src-tauri/src/cmd/storage/entries/mod.rs` | `EntrySample.observed_at` + 构造器保持 `None` |
| `src-tauri/src/cmd/storage/entries/assemble.rs` | `build_entry` 填 `observed_at`；`complete` 只看 `Inline` |
| `src-tauri/src/cmd/storage/sample/mod.rs` | `SampleMode`；常量；`mod cache` |
| `src-tauri/src/cmd/storage/sample/collect.rs` | 拆 `Inline` 现场采样 / `Deferred` 读缓存 + 触发；`Plan` 模式 |
| `src-tauri/src/cmd/storage/sample/timeout.rs` | `Inline` 超时 `log::warn!`（声明说谎探针） |
| `src-tauri/src/cmd/storage/sample/cache.rs`（新增） | `EntryStatsCache` 状态机、single-flight + 租约、有界并发、闭包自结算、事件合并、`trigger_stats` |
| `src-tauri/src/cmd/storage/inventory/subtract.rs` | `subtract_declared` 加 `now` 与年龄上限；`node_dto` 透传 `now` |
| `src-tauri/src/cmd/storage/inventory/cache.rs` | `scope_totals_dto` 的 `partial` 纳入「登记值过期」 |
| `src-tauri/src/cmd/storage/reclaim/plan.rs` | 先校验 key 再采样；`Plan` 模式 + `PLAN_SAMPLE_TIMEOUT` |
| `src-tauri/src/cmd/storage/reclaim/execute.rs` | 成功后按条 `invalidate` |
| `src-tauri/src/cmd/storage/mod.rs` | `collect` 返回触发列表 → `trigger_stats`；`delete_attachments` 成功后 `invalidate(&["attachments/derived"])` |
| `src-tauri/src/boot/build/state.rs` | `manage(EntryStatsCache::new())` |
| `src-tauri/src/protocol/events.rs` | `STORAGE_ENTRY_STATS_UPDATED` |
| `src-tauri/src/feature_plugins/vault_plugin.rs` | 锁 / 解锁处 `invalidate_sections()` |
| `src-tauri/crates/assistant-protocol/src/storage.rs` | `StorageEntryDto.observed_at`；修正 `availability` 注释 |
| `src/settings/pages/storage/useStorageSnapshot.ts` | 订阅新事件 + 尾随刷新 |
| `src/settings/pages/storage/entryView.ts` | 详情展示 `observedAt` |
| `src/settings/pages/storage/SnapshotToolbar.tsx` | `sampledAt` 标签改「快照于」（i18n key 不变，改文案值） |
| `src/i18n/locales/{zh-CN,en-US}.json` | `noReportYet` 文案、`observedAt`、`toolbar.sampled_at` 文案 |
| `src/types/protocol.contract.ts` | `npm run contract:gen` 生成（不手改） |

---

## 11 实施批次

| 批次 | 内容 | 可独立合入 |
|---|---|---|
| R0 | 契约与声明：`StatsMode` + 登记表 / `SectionEntry` / `EntryView`；`observed_at` DTO + `contract:gen`；`E1` 恒 `Inline` 单测 | 是（行为不变，全部仍 `Inline`） |
| R1 | 读路径分档 + `EntryStatsCache`（含闭包自结算与租约）+ worker + 事件 + `complete` 收窄 + 计划 `Plan` 模式（强制现场采样，保持计划新鲜）+ 登记表把 `E2` / `E4` 改 `Deferred` | 是（`Deferred` 上线即零等待；计划仍新鲜） |
| R2 | 减法年龄上限 + 回收后失效 + **删除附件后失效 `E2`** + vault 锁 / 解锁失效 + 计划「先校验后采样」优化 | 是（纯护栏，不改页面语义） |
| R3 | 前端订阅 / 尾随刷新 / `observedAt` 展示 / 「统计中」与工具栏「快照于」文案 | 是（后端事件已就绪，缺前端只是不自动刷新） |

批次顺序不可颠倒：R1 必须同时交付 `Plan` 模式，否则计划会读到缓存、破坏 C2；R1 把 `E2` / `E4` 改档与 R2 的 `E2` 失效点必须同版上线，否则「删附件后 `E2` 可回收量滞后」会先暴露一个 TTL。

---

## 12 拒绝方案

| # | 方案 | 拒绝理由 |
|---|---|---|
| X1 | 把 `SAMPLE_TIMEOUT` 调大（500ms → 2s） | 页面从卡 500ms 变成卡 2s；`spawn_blocking` 仍白算并丢弃；慢条目继续拖整页 |
| X2 | 用与 `samples` 平行的 `observed_ats` 数组传时刻 | 两数组必须手工对齐，`set_sample` 等寻址路径容易错位；放入 `EntrySample` 才与样本同生命周期（X15） |
| X3 | 第二条命令 `get_storage_entry_stats(keys)` 由前端补丁 | 新增第二套装配路径与契约面；每次仍重算；计划一致性还要另做 |
| X4 | 全异步（含 `E1`） | 破坏账本闭合（C1）与计划新鲜度（C2）；页面 `total` 与条目来自不同时刻 |
| X5 | 给 `get_storage_breakdown` 加 `force` 入参做「强制重算」 | 只为 60s 的陈旧度引入入参分叉；TTL 放宽到 300 时改用与 `refresh_storage_inventory` 同构的镜像命令，不加参 |
| X6 | 给 owner stats 加 deadline / cancel | 需改 5 处 owner 入口并给递归遍历逐层穿 token；闭包自结算已消除「结果被丢弃」，不 abort 的唯一代价是白跑一次只读 IO |
| X7 | 事件携带 key，前端按 key 局部更新 | `inventory.declared` 由同响应样本派生，局部更新会造成「条目新、对账旧」，除非前端重算聚合（等于复制减法逻辑） |

---

## 13 已决问题与残余开放项

### 13.1 已决（本次定稿）

| # | 问题 | 结论 | 理由摘要 |
|---|---|---|---|
| OQ1 | `E2` / `E4` 是否也该 `Deferred` | **是，两者都改 `Deferred`** | `E4` 实际是递归遍历（非单层）；`E2` 内部会再枚举一次附件目录，重复 `E1` 已付的成本。二者都不满足「结构性上界」。连带新增 `delete_attachments` → `invalidate(E2)`（§8.5） |
| OQ2 | `ENTRY_STATS_TTL_SECONDS` 取 60 还是 300 | **60** | 前端无轮询，TTL 只决定「重开 / 手动刷新时要不要重算」；300 会让 4 分钟前的数字被当作当前值。重算已被单飞与并发上限限流 |
| OQ3 | 是否提供「强制刷新统计」入口、事件是否携带 key | **本期不加 force；载荷恒 `()`** | TTL=60 下最坏陈旧度 60s，已有 `observedAt` 表达年龄；事件带 key 会破坏对账同源（X7）。TTL 若放宽到 300，则必须补镜像命令（§9.1） |
| OQ4 | `spawn_blocking` 超时后线程不可 abort 的长期方案 | **不改 owner API，改为闭包自结算 + 租约** | 结果由阻塞闭包自己落库，不再被丢弃；卡死由租约兜底；加 deadline/cancel 的改动面与收益不成比例（X6） |

### 13.2 残余开放项

| # | 问题 | 当前取值 |
|---|---|---|
| OQ5 | `E13` 是「弱上界」`Inline`，实测是否会超 500ms | 暂 `Inline`；若 `Inline` 说谎告警（§4.4）在真实机器上出现，改档只动登记表一行 |
| OQ6 | `ENTRY_STATS_MAX_INFLIGHT = 2` 是否够用 | 暂 2（机械盘友好）；若「统计中」持续过久（补齐慢）再抬到 3–4 |
| OQ7 | 常开页面的「自动保鲜」（可见时定时刷新） | 本期不做：会每分钟重付一次 `E1` 的 `list_all`，代价高于收益；靠 `observedAt` 展示年龄 |

---

## 14 验收判据

| # | 判据 | 验证方式 |
|---|---|---|
| V1 | `E1` 在登记表中恒 `Inline` | `registry.rs` 单测；人为改档测试必须红 |
| V2 | `Deferred` 条目首次读取返回 `noReportYet` + 全 `null`，且耗时不含 500ms 等待 | `EntryStatsCache::read` 单测 + 命令层计时 |
| V3 | `Deferred` 首次读取**不填 0** | 断言 `bytes` / `count` 为 `None` |
| V4 | 成功且未过期：`available` / `missing` + 原数字 + 不触发 | 状态机单测覆盖 `age < TTL` |
| V5 | 成功但过期：`staleReport` + 旧数字 + 触发一次 | 状态机单测 |
| V6 | 失败且退避内：保留失败类别 + `null` + 不触发 | 状态机单测 |
| V7 | 失败且超过退避：触发一次 | 状态机单测 |
| V8 | 同一 key 在 `running` 期间重复触发只起一个任务 | `begin` 单测 + 计数断言 |
| V9 | 后台并发不超过 `ENTRY_STATS_MAX_INFLIGHT` | worker 单测（注入阻塞采样 + 计数） |
| V10 | 一个 drain 周期内多条完成只发一次事件 | 事件计数断言 |
| V11 | `complete` 只由 `Inline` 条目决定 | `assemble` 单测：`Deferred` 全 `noReportYet` 时 `complete` 仍可为 `true` |
| V12 | 计划对 `Deferred` 现场采样且 `judgeable` 反映真实新鲜度 | `plan` 单测：注入慢采样 → `complete = false` |
| V13 | 计划不写缓存 | 计划调用前后缓存状态不变（单测） |
| V14 | 登记值年龄 `>= SCAN_TTL_SECONDS` 时 `residual` 为 `null` | `subtract` 单测 |
| V15 | 回收成功（`removed_bytes > 0`）后该条缓存失效 | `execute` 单测 / 集成 |
| V16 | `locked` 不缓存数字，解锁失效后重新补算 | 状态机单测 |
| V17 | `StorageEntryDto` 恒含 `observedAt` 键；`Inline` 等于 `sampledAt`，首次 `Deferred` 为 `null` | `protocol` 序列化测试 + 前端类型检查 |
| V18 | 前端事件到达时若在飞，结束后补一次刷新 | hook 单测（mock service 计数） |
| V19 | `noReportYet` 显示「统计中」，`staleReport` 显示「数据已过期」 | 组件 / 文案测试 |
| V20 | `npm run contract:gen` 幂等，生成类型无手改；`cargo test`、前端 lint 无新增告警 | CI / 本地门禁 |
| V21 | 超时占位后，同一任务完成仍覆盖为真值并发事件 | 状态机单测：注入「先超时后成功」的采样 |
| V22 | token 不匹配的旧结果被丢弃，不覆盖新值 | 状态机单测：`invalidate` 后旧任务 `finish` 无效 |
| V23 | `running` 超过 `ENTRY_STATS_LEASE_SECONDS` 后允许重新触发 | `begin` 单测（时间注入） |
| V24 | `delete_attachments` 成功删除后 `E2` 缓存失效，下次读取触发补算 | 命令层单测 / 集成 |
| V25 | 工具栏标签为「快照于」，`noReportYet` 为「统计中」 | i18n 文案测试 + locale 键对齐（`localeParity`） |
| V26 | `E2` / `E4` 在登记表中为 `Deferred`，且 `E1` 仍为 `Inline` | `registry.rs` 单测（与 V1 同文件） |
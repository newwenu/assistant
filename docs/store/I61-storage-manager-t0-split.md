# I61 T0 — 长文件按职责拆分（T1-g 前置步骤）

> 性质：**执行步骤文档**（T0），不是任务基线。I61 待办仍以 [I61-storage-manager-remaining.md](./I61-storage-manager-remaining.md) 为唯一来源；规格仍以 [I61-storage-manager-spec.md](./I61-storage-manager-spec.md) 为唯一判据基线。本文件只定义「`R-01` 动手前把文件切在哪、按什么顺序切」。
>
> 拆分判据只引用 [CODE-SPLIT-PRINCIPLE.md](../principle/CODE-SPLIT-PRINCIPLE.md)（§1 变化原因 / §3 测试处理 / §4 目录模块 / §6 依赖方向 / §7 禁止事项），不复述条款。
>
> 本步是**纯机械搬移**：不改函数体、不改对外可见性、不改对外 `pub use` 路径、不动 wire DTO、不跑 `contract:gen`、不动前端、不外移任何测试（内联测试随被测实现一起搬，见 §2.1 / §2.2 / §2.3）。唯一允许的可见性调整是「跨子模块引用」：原本同文件内可见的私有项在拆分后按需放宽到 `pub(super)` / `pub(crate)`，`pub` 项一律不变（§2 首段）。

## 0 为什么排在 T1-g 之前

`R-01`–`R-15` 要在同一批文件上做整片改写（`probe` 退役、`E7`–`E11` 删除、定长数组换动态切片、能力名白名单换分区自述、`confirmation` 改两步派生）。现状这几个文件各混着 2–4 类互不相关的变化原因（见 §1），改造与结构搅在一起，评审只能按整文件走、回滚只能整文件回滚。

先按职责切开，收益三条：

1. `R-08`–`R-11` 每条只落一个子模块，改动面与评审面可读；
2. `probe` 退役件被隔离成 `sample/probe.rs` 一个文件，`R-01` / `R-08` 的删除退化成**整文件删除**；
3. 后续（T4 之后）新增条目、新增分区只碰对应子模块，不再回到 900 行的文件里做外科手术。

T0 **按步独立提交**（每步单独编译通过、不改语义），不与 `R-01`–`R-15` 的整组混提交；`R-01` 仍须在 T0-a/T0-b 之后开工（否则 `probe` 退役时 `sample.rs` 是整片改写）。

## 1 现状快照（行数为非空行，2026-11-21 统计）

| 文件 | 总行 / 非空 | 变化原因（职责） | 结论 |
|---|---|---|---|
| `src-tauri/src/cmd/storage/entries.rs` | 953 / 905 | 静态声明表、资格判定、条目装配、样本槽位寻址、（内联测试 339 行） | **拆目录** |
| `src-tauri/src/cmd/storage/inventory.rs` | 861 / 802 | 有界遍历扫描（唯一 `read_dir`）、分类归组、声明减法、缓存与状态机 | **拆目录** |
| `src-tauri/src/cmd/storage/reclaim.rs` | 802 / 756 | 计划缓存、计划构造、执行路由、各条目执行器、结果报告 | **拆目录** |
| `src-tauri/src/cmd/storage/sample.rs` | 808 / 731 | 采样编排、各条目取数、`StatsView`→样本映射、策略解析、超时包装、旧 probe 路由 | **拆目录** |
| `src-tauri/src/cmd/storage/mod.rs` | 418 / 368 | 命令入口 + 组合上下文、附件引用账本（DTO 适配 + 测试） | **抽同级文件** `ledger.rs` |
| `src-tauri/src/cmd/storage/section.rs` | 547 / 485 | 分区契约类型 + 注册校验 | **不拆**（§2.6） |
| `src-tauri/src/cmd/storage/probe.rs` | 268 / 233 | 旧 `ProbeRegistry` | **不动**（`R-01` 整文件删除） |
| `src-tauri/src/app_dirs/registry.rs` | 450 / 418 | 受管根登记表 | **不动**（`R-02` 整条重写） |
| `src-tauri/src/vault_plugin.rs` | 281 / 258 | 分区适配器 | **不动**（`R-04`/`R-06`/`R-07` 整条重写） |
| 前端 `src/settings/pages/storage/**`、`src/services/desktop/storageService.ts` | 最大 304（测试）/ 252（实现） | 单一 tab 的视图与流程 | **不动**（均未过阈值，且 T4 整页改造） |
| `src-tauri/tests/storage_{breakdown,inventory,reclaim}.rs` | — | 测试文件 | **不动**（主题改写归 `R-12`–`R-14`） |

## 2 拆分决定

统一模式（CODE-SPLIT §4）：`foo.rs` → `foo/mod.rs` + 按职责命名的子模块；`mod.rs` 只放类型定义、共享常量、`mod` 声明、组合入口、`pub use`；原文件的 `pub fn` / `pub struct` 全部由 `pub use` 原样导出，**对外路径零变化**（`crate::cmd::storage::{entries,inventory,reclaim,sample}` 下的名字与签名都不变）。

可见性口径：`pub` 项一律不变；`pub(crate)` 项用 `pub(crate) use` 原样导出；原本「同文件内随便用」的私有项（如 `static_for`、`from_dir_stats`、`attachment_reason`、`layer_of`）在拆分后按实际调用关系放宽到 `pub(super)` 或 `pub(crate)`，**只放宽到模块边界，不新增对外 `pub`**。

### 2.1 `entries.rs` → `entries/`

| 子模块 | 承接项 | 依赖 |
|---|---|---|
| `entries/mod.rs` | `ENTRY_COUNT`、`LOGS_ENTRY`、`EntrySample`（含 `observed`/`missing`/`degraded`/`with_policy`）、`BreakdownBase`、`pub use` | — |
| `entries/decl.rs` | `EntryStatic`、`constant_policy`、`owner_policy`、`static_for` | `super` 类型 |
| `entries/eligibility.rs` | `eligibility_for`、`orphans_eligibility`、`tool_results_eligibility`、`backups_eligibility`、`confirmation_for` | `decl` |
| `entries/assemble.rs` | `layer_of`、`build_entry`、`judgeable`、`build_plan_entry`、`assemble_breakdown` | `decl`、`eligibility`、`slot` |
| `entries/slot.rs` | `index_of_key`、`sample_for`、`set_sample`、`unmounted_probe_sample` | `super` 类型 |

- 测试处理：内联测试 9 个（约 386 行）全部围绕 `assemble_breakdown`，且其中 `entries_are_thirteen_in_registry_order` 直接调私有 `layer_of` 与 `registry::MANAGED_ROOTS` → 整块随实现留在 `entries/assemble.rs`，**不外移、不改可见性、不加 `pub`**。
- 依赖方向：`assemble → eligibility → decl`，`slot` 只被外部（`sample` / `reclaim`）调用，无环。
- `unmounted_probe_sample` 留在 `slot.rs` 是为了 `R-01` 的删除落点单一（与 `sample/probe.rs` 同批删）。

### 2.2 `sample.rs` → `sample/`

| 子模块 | 承接项 | 依赖 |
|---|---|---|
| `sample/mod.rs` | `SAMPLE_TIMEOUT`、`SampleInputs`、`pub use` | — |
| `sample/collect.rs` | `collect`、`collect_with`（编排 + 降级填充） | `rows`、`timeout`、`stats`、`policy`、`probe` |
| `sample/rows.rs` | `attachment_sample`、`derived_sample`、`sample_index`、`sample_mascot`、`sample_crashes`、`sample_database`、`sample_tool_results` | `stats`、`policy` |
| `sample/stats.rs` | `StatsView` trait、`impl_stats_view!`、`io_reason`、`from_dir_stats`、`from_io`、`attachment_reason` | `super` 类型、DTO |
| `sample/policy.rs` | `config_policy`、`keep_policy` | DTO |
| `sample/timeout.rs` | `SampleFuture`、`timeout_sample`、`timed`、`timed_blocking` | — |
| `sample/probe.rs` | `probe_sample`、`probe_error`、`probe_sample_for`、`audit_policy_of`、`max_backups_of` | `super` 类型 |

- `sample/probe.rs` 是**退役件隔离区**：`R-01`（删 `probe.rs` 注册表）与 `R-08`（换分区自述取数）落地时整文件删除，不牵连 `rows.rs` / `collect.rs`。
- 测试处理：内联测试**随被测子模块一起搬**（被测项多为私有，如 `probe_sample`、`attachment_sample`、`from_dir_stats`），不新增 `internal-tests` feature（CODE-SPLIT §3 的替代机制本 crate 未启用）。`R-08` 删 `probe.rs` 时其测试一并消失；`rows` / `stats` 的测试在 `R-08`/`R-15` 改写清单里。

### 2.3 `inventory.rs` → `inventory/`

| 子模块 | 承接项 | 依赖 |
|---|---|---|
| `inventory/mod.rs` | `SCAN_TTL_SECONDS`、`now_seconds`、`RawNode`、`ScanResult`、`pub use` | — |
| `inventory/walk.rs` | `MAX_DEPTH`/`MAX_ENTRIES`/`MAX_DURATION`/`MAX_CHILDREN`、`Budget`、`Stop`、`Observed`、`observe`、`walk`、`children_of`、`list_root`、`build_node`、`scan` —— **全仓 `read_dir` 唯一落点**（现 3 处） | `mod` 类型、`classify` |
| `inventory/classify.rs` | `COMPANION_SUFFIXES`、`classify`、`is_safe_segment`、`layer_of` | — |
| `inventory/subtract.rs` | `subtract_declared`、`residual`、`node_dto`、`sum_unregistered` | `mod` 类型、`entries` |
| `inventory/cache.rs` | `status_of`、`CacheState`、`InventoryCache`（`get`/`store`/`touch`/`invalidate`/`begin`/`fail`/`report`）、`trigger_scan` | `walk`（`scan`）、`subtract`（`node_dto`） |

- 依赖方向：`walk → classify`、`cache → walk/subtract`，叶子是 `classify`，无环。
- 测试处理：内联测试随被测子模块搬（`walk` 的预算/深度用例进 `walk.rs`，`cache` 的 TTL/状态机用例进 `cache.rs`）。
- 本条会改动规格里的**文件路径字面量**，处理见 §5.1（T0-c 同提交完成，只改路径不改判据）。

### 2.4 `reclaim.rs` → `reclaim/`

| 子模块 | 承接项 | 依赖 |
|---|---|---|
| `reclaim/mod.rs` | `PLAN_TTL_SECONDS`、`ReclaimPorts`、`pub use` | — |
| `reclaim/store.rs` | `CachedPlanEntry`、`CachedPlan`（含 `policy_of`）、`PlanLookupError`、`ReclaimPlanStore`、`plan_age`、`ExecutionGuard` | — |
| `reclaim/plan.rs` | `plan_storage_reclaim`、`requested_roots`、`total_expected_bytes` | `store`、`entries`、`sample`、`ledger` |
| `reclaim/execute.rs` | `execute_storage_reclaim`、`executor_action`、`validated_targets` | `store`、`executors`、`report` |
| `reclaim/executors.rs` | `execute_orphans`、`execute_derived`、`execute_index`、`execute_tool_results`、`execute_backups` | `report`、`sample`（`attachment_reason`） |
| `reclaim/report.rs` | `MAX_ERRORS`、`Totals`、`status_of`、`failure_errors`、`error_dto`、`entry_result` | DTO |

- 本文件**无内联测试**（测试在 `tests/storage_reclaim.rs`），无测试搬移。
- `plan.rs` 里 `super::load_attachment_refs` 的路径随 T0-d（`ledger.rs`）改为 `super::ledger::load_attachment_refs` —— 所以 **T0-d 必须先于 T0-e**（§3 顺序已按此排）。

### 2.5 `cmd/storage/mod.rs` → 抽同级文件 `ledger.rs`

| 落点 | 承接项 | 依赖 |
|---|---|---|
| `src/cmd/storage/ledger.rs`（新） | `AttachmentRefs`、`ConvAttachmentRow`、`load_attachment_refs`、`summary`、`build_base`、`media_type_from_id` + 其 2 个内联测试 | `entries::BreakdownBase`、DTO |
| `src/cmd/storage/mod.rs`（保留） | 6 个命令（`get_storage_breakdown` / `refresh_storage_inventory` / `get_storage_reclaim_plan` / `reclaim_storage` / `delete_attachments` / `open_attachment`）、`ReclaimContext`、`now_seconds`、`mod` 声明 | 全部子模块 |

- 抽同级文件而非目录：`mod.rs` 拆分后只剩命令入口与组合上下文（约 130 非空行），已是单一职责，再套一层目录只增加路径深度。
- 附件引用账本的职责是「会话库 → `AttachmentRefs` 的适配与装配」，与命令入口无关；`E1` 的孤儿判据依赖它，`R-09`/`R-11` 会继续改它。

### 2.6 不拆清单（理由）

- `section.rs`：契约类型与注册校验共用同一批类型，且 `R-02`（风险档）、`R-05`（`EntryDecl` 自述）、`R-16`（`SectionError` 收敛）会连续重写，现在拆只放大冲突面。
- `app_dirs/registry.rs` / `vault_plugin.rs` / `probe.rs`：单一职责文件，且分别由 `R-02` / `R-04`–`R-07` / `R-01` 整条重写或删除。
- 前端 storage 目录：最大实现文件 252 非空行，且 T4 会整页重构；现在拆分与 T4 落点重叠。
- `tests/storage_*.rs`：测试文件不适用职责拆分判据，主题改写归 `R-12`–`R-14`。

## 3 步骤（每步一个提交，按序执行）

> 全部命令在 `src-tauri` 目录执行（不要用 `--manifest-path`）。

### T0-a `entries` 目录化

1. 新建 `entries/{mod,decl,eligibility,assemble,slot}.rs`，按 §2.1 搬移；`mod.rs` 用 `pub use` 导出原有 `pub` 项。
2. 删除 `entries.rs`。
3. 内联测试整块搬入 `entries/assemble.rs`（`use super::slot::set_sample` 等导入按新层级调整，断言与用例名不改）。
4. 验收：`cargo check -p assistant-app`、`cargo test -p assistant-app --test storage_breakdown --test storage_inventory`、`cargo clippy -p assistant-app --tests` 无新增告警。

### T0-b `sample` 目录化

1. 按 §2.2 建 `sample/`，`probe` 相关函数单独落 `sample/probe.rs`。
2. 删除 `sample.rs`；`super::entries::*` 的导入路径**不改**（T0-a 已用 `pub use` 保住路径）。
3. 验收：`cargo check -p assistant-app`、`cargo test -p assistant-app --test storage_breakdown --test storage_reclaim`、`cargo clippy -p assistant-app --tests`。

### T0-c `inventory` 目录化（含规格锚点同步）

1. 按 §2.3 建 `inventory/`，`read_dir` 只落在 `walk.rs`。
2. 原文件头「全仓库唯一允许对基根树 `read_dir`」的注释随 `walk.rs` 搬，并在 `inventory/mod.rs` 顶部保留一句模块级说明。
3. **同一提交**完成 §5.1 中与 `inventory` 相关的 5 处路径字面量同步（只改路径，不改判据、不改验收）；「影响范围」行要等四个目录都在（T0-e 之后）才改，落在 T0-f。
4. 验收：`cargo check -p assistant-app`、`cargo test -p assistant-app --test storage_inventory`、`cargo clippy -p assistant-app --tests`；手工门禁 `rg -n "read_dir" src-tauri/src/cmd/storage/inventory/` 只命中 `walk.rs`。

### T0-d `cmd/storage/mod.rs` → `ledger.rs`

1. 按 §2.5 抽出 `ledger.rs`（含 2 个内联测试），`mod.rs` 只留命令、`ReclaimContext`、`now_seconds`。
2. 本步 `reclaim` 还是单文件：`reclaim.rs` 里的 `super::load_attachment_refs` 改为 `super::ledger::load_attachment_refs`（T0-e 拆目录后该调用落在 `reclaim/plan.rs`，路径已是 `super::ledger::*`，不再二次改）。
3. 验收：`cargo check -p assistant-app`、`cargo test -p assistant-app --test storage_reclaim --test storage_breakdown`。

### T0-e `reclaim` 目录化

1. 按 §2.4 建 `reclaim/`，`plan.rs` 引用 `super::ledger::load_attachment_refs`。
2. 删除 `reclaim.rs`。
3. 验收：`cargo check -p assistant-app`、`cargo test -p assistant-app --test storage_reclaim`、`cargo clippy -p assistant-app --tests`。

### T0-f 收尾：全量验收 + 文档指针同步

1. 全量：`cargo test -p assistant-app`（含 `storage_breakdown` / `storage_inventory` / `storage_reclaim` 三个集成测试）、`cargo clippy -p assistant-app --tests`。
2. 对外 API 自查：`crate::cmd::storage::{entries,inventory,reclaim,sample}` 下的 `pub` 项与拆分前逐一对照，签名不得变化。
3. 文档指针：把 [I61-storage-manager-remaining.md](./I61-storage-manager-remaining.md) 中指向已拆分文件的**路径指针**更新为 §4 的新落点（只改路径，不改任务语义、不改验收）；`R-10` 的 `inventory.rs` 指针改为 `inventory/`（分类 `classify.rs` / 减法 `subtract.rs` / 有界遍历 `walk.rs`），`R-15` 补充 2 的文件清单改为目录递归（§5.2）；同时补完 §5.1 剩下的「影响范围」行（四个目录此时都已存在）。
4. 前端不动，`npm run typecheck` / 前端测试无需跑。

## 4 与 T1-g 的落点映射（`R-01`–`R-17` 改写时按此表找文件）

| 任务 | 拆分前 | 拆分后落点 |
|---|---|---|
| `R-01` 删 `probe` 注册表 | `probe.rs`、`mod.rs`、`sample.rs` | `probe.rs`（整文件删）、`mod.rs`、`sample/probe.rs`（整文件删）、`entries/slot.rs`（`unmounted_probe_sample` 删） |
| `R-02` 登记表瘦身 + 风险档 | `registry.rs`、`entries.rs` | `app_dirs/registry.rs`、`entries/decl.rs`（`EntryStatic` 加 `risk`） |
| `R-04`–`R-07` 分区注册 / 适配器 | `vault_plugin.rs`、`mod.rs`、`sample.rs` | `vault_plugin.rs`、`mod.rs`、`sample/collect.rs`（注册入口接线）、新 `assistant-vault/src/storage_section.rs` |
| `R-08` 取数换分区自述 | `sample.rs`、`probe.rs` | `sample/collect.rs`（入参）、`sample/rows.rs`（分区取数）、`sample/stats.rs`（`SectionStats` → `EntrySample`）、`sample/probe.rs`（删） |
| `R-09` 删 `E7`–`E11` + 动态长度 | `entries.rs`、`sample.rs` | `entries/decl.rs`、`entries/eligibility.rs`、`entries/assemble.rs`（含内联测试）、`entries/mod.rs`（`ENTRY_COUNT` 去定长）、`sample/rows.rs`（`E7` 不再判 `locked`） |
| `R-10` 对账改分区声明 | `inventory.rs` | `inventory/walk.rs`（`mounted` 入参）、`inventory/classify.rs`（首段判定）、`inventory/subtract.rs`（`Vec` 签名）、`inventory/cache.rs` |
| `R-11` 回收白名单 + 分区转发 | `reclaim.rs` | `reclaim/plan.rs`（统一查找）、`reclaim/execute.rs`（白名单）、`reclaim/executors.rs`、`reclaim/store.rs`（`policy` 快照） |
| `R-12`–`R-14` 测试改写 | `tests/storage_*.rs` | 路径不变 |
| `R-15` 静态检查门禁 | — | §5.2 的目录递归改法 |
| `R-16` 删 `availabilityReason` | `entries.rs`、`section.rs` | `entries/mod.rs`（`EntrySample`）、`section.rs`（`SectionError::reason`） |
| `R-17` 随 `R-08` 关闭 | `sample.rs` | `sample/timeout.rs` |
| `R-39` 散文文案 | `entries.rs`、`sample.rs`、`reclaim.rs` | `entries/decl.rs`、`sample/stats.rs`、`reclaim/report.rs` |
| `R-40` 规格内部冲突修正 | 规格 | 不变；T0 只改路径锚点（§5.1），语义冲突仍归 `R-40` |

## 5 静态检查与规格锚点的口径交接

### 5.1 目录化引起的 6 处路径字面量（T0-c 改 5 处、T0-f 改 1 处）

| 位置 | 现状 | 改为 |
|---|---|---|
| 规格 §3.11.2 标题 | `cmd/storage/inventory.rs`（唯一 `read_dir` 例外） | `cmd/storage/inventory/`（目录，唯一 `read_dir` 例外） |
| 规格 G2 | 唯一例外：`cmd/storage/inventory.rs` | 唯一例外：`cmd/storage/inventory/`（目录） |
| 规格 V10 | `read_dir` 只允许出现在 `cmd/storage/inventory.rs` | `read_dir` 只允许出现在 `cmd/storage/inventory/`（目录，实际落点 `walk.rs`） |
| 规格「影响范围」行 | 列举 `inventory.rs` / `reclaim.rs` / `mod.rs` | 列举 `entries/`、`sample/`、`inventory/`、`reclaim/`、`ledger.rs`、`section.rs` |
| 规格 §1 第 4 条（模块职责） | `inventory.rs` 承载 §3.11 的对账扫描 | `inventory/` 承载 §3.11 的对账扫描（同一行里的 `reclaim.rs` → `reclaim/` 随 T0-f 一并改） |
| 规格 §10「T1 内部顺序」第 5 条 | 登记表必须先于 `inventory.rs` 扫描落地 | 登记表必须先于 `inventory/` 扫描落地 |

只改路径，判据 ID、门禁 ID、验收措辞不动。规格里**描述性**提及（如 V22④「`sample.rs` 的下标常量与 `entries.rs` 的 `LOGS_INDEX` 已删除」——这两个文件在 `R-09` 后本就不存在）不逐处改，统一按「同名目录」理解；不在规格里做全局替换（`mod.rs` 在多处指其它 crate，替换会误伤）。**若评审要求规格零改动**：`inventory` 跳过 T0-c、保持单文件（`read_dir` 白名单字面量因此不用动），其余 4 个落点不受影响（§6 回退项）。

### 5.2 `R-15` 补充 2 的静态检查改法

原做法是逐个文件读源码断言；拆分后改为**目录递归**：

- 目标集合：`src/cmd/storage/{entries,sample,inventory,reclaim}/` 下全部 `.rs`（递归）+ `src/app_dirs/registry.rs`；
- 断言一：拼接后的源码零 `security/` 字面量、零 `SECURITY_OBSERVABILITY` 常量；
- 断言二：`read_dir` 全仓只命中 `src/cmd/storage/inventory/walk.rs`（文件白名单随之收窄到 `walk.rs`，比原来更严）。

### 5.3 手工门禁命令（T0 后随时可跑）

- `rg -n "read_dir" src-tauri/src/cmd/storage/inventory/` → 只命中 `walk.rs`；
- `rg -n "security/|SECURITY_OBSERVABILITY" src-tauri/src/cmd/storage/ src-tauri/src/app_dirs/registry.rs` → `R-15` 落地后应零命中。

## 6 风险与回退

- **`pub use` 漏项**：集成测试与 `reclaim`/`mod` 的跨模块引用会立刻编译失败，属编译期暴露，不会静默；T0-f 第 2 条专门自查一遍。
- **`inventory/` 触发规格字面路径**：由 §5.1 的锚点同步覆盖（T0-c 改 5 处、T0-f 补「影响范围」行）；漏改会在 `R-15` 的门禁实现时暴露（白名单按目录写、实际文件不符）。
- **与 T1-g 混提交**：禁止。T0 每步独立提交，`R-01`–`R-15` 另行整组提交，两者不混，便于 `git revert` 单步回退（T0 不改语义，回退无数据风险）。
- **规格零改动的评审诉求**：按 §5.1 末段退化为「`inventory` 跳过 T0-c、保持单文件」；其余 4 个落点照旧，`R-15` 的 `read_dir` 白名单也不用改。
# I61 — 存储管理器 任务清单

> 规格：[I61-storage-manager-spec.md](./I61-storage-manager-spec.md)（**唯一事实基线**，判据 / 词表 / 白名单 / 门禁全部以它为准；本清单只拆解「改哪里、验收跑什么」，不重述规格语义）
> 事实基线：I61 §2 的 N1–N18；分析：[STORAGE-MONITOR-ANALYSIS.md](../analysis/STORAGE-MONITOR-ANALYSIS.md)
> 关联：[I54-resource-monitor-spec.md](./I54-resource-monitor-spec.md) §3（资源监视器页面壳，存储 tab 的挂载容器）

切片对应规格章节：T1 观测补全（§3.1–§3.2 / §3.9 / §3.10 / §3.11 / §6.2 / §7.1）→ T2 骨架 + 低风险回收（§5.1–§5.4 / §6.1 / §3.3 白名单）→ T3 中风险回收（§3.10 接口形状 / §5.5 / §3.6）→ T4 前端与收尾（[前端设计文档](./I61-storage-manager-frontend.md) / §10 / §12；规格 §6.3 已整体划出作废）。

> **条目计数口径**：本清单记录的「core 8 条」「13 条 / 8 行」均为 T1–T4 实施期口径。T5 / T6（[I67](./../store/I67-storage-inventory-totals-spec.md) §13）登记 `E14`–`E18` 后，core 条目为 **13 条**，`entries` 合计为 `vault` 构建 18 条 / `vaultless` 构建 13 条；下文的历史计数按当时事实保留，现行口径以 I61 §3.1 与 I67 §13 为准。

**可选功能分区自管理**（§3.1.1 / §3.10「可选功能分区自注册」/ §3.8 G16–G19 / §3.11.1 登记纪律 2–6 / V21–V24）是 T1 之后的装配层修订，任务落在 **T1-g 修订**，替换 T1-c 与 T1-c 修订的旧实现；该修订未完成前，T4 不得按「未挂载补占位行」实现。

依赖：T1 无前置（后端可独立验收）；T2 依赖 T1；T3 依赖 T2（骨架 + 按 ID 锁）；T4 依赖 T1–T3 **且**依赖 I54 的 `R0`（页面容器与三 tab 挂载位，已落地）。

**进度总览**（勾选状态与本节同步；T1-g 修订落地后，T2 / T3 的旧形态断言按该节表格改写）：

| 切片 | 状态 | 阻塞 |
|---|---|---|
| T1 观测补全 | 完成 2026-11-20；`T1-h` 剩余项已全部落地（见 [剩余清单](./I61-storage-manager-remaining.md) `R-16` / `R-17`） | — |
| T1-g 修订（分区自管理） | 完成 2026-11-21（`R-01`–`R-15` 全部落地，见 [剩余清单](./I61-storage-manager-remaining.md)） | — |
| T2 骨架 + 低风险回收 | 完成 2026-11-20 | — |
| T3 中风险回收 | 完成 2026-11-20（`E8` 形态已按 T1-g 改写） | — |
| T4 前端与收尾 | 主体完成（`R-19`–`R-36` 已落地）；`R-37` 附件明细区手动回归待做 | — |
| T5 登记 `EBWebView`（`E14`） | 完成（[I67](./../store/I67-storage-inventory-totals-spec.md) §13） | — |
| T6 登记四个配置文件（`E15`–`E18`） | 完成（[I67](./../store/I67-storage-inventory-totals-spec.md) §13） | — |

> 本清单不管理提交。提交粒度按代码依赖与可编译性现场决定；这里只回答「代码改没改、验收跑没跑」。
> 勾选表示代码已改且对应验收已跑。

> **路径读法**：文中 `src-tauri/src/cmd/storage.rs` 一律指「T1-c 首条目录化完成后」的 `src-tauri/src/cmd/storage/mod.rs`（模块路径仍是 `cmd::storage`）；其中带 `:130` 之类行号的是**改造前现状**行号，仅用于定位 `delete_attachments` 等既有函数，不要当作改动后的行号。目录化后的完整布局：`storage/mod.rs`（命令 + 装配）、`storage/section.rs`（可选功能分区契约，**T1-g 修订后**取代 `storage/probe.rs`）、`storage/inventory.rs`（§3.11 对账扫描，**唯一 `read_dir` 例外**）、`storage/reclaim.rs`（T2 起的计划与执行）、`storage/entries.rs`（条目静态档装配）、`storage/sample.rs`（采样）。

---

## 阶段 T1 — 观测补全（只读，无回收）（主体完成 2026-11-20；遗留项已迁到 [剩余清单](./I61-storage-manager-remaining.md)）

退出条件：V1–V5、V10、V13、V18、V19、V20 通过；golden 可断言 `vault` 构建 13 行、`vaultless` 构建 8 行（T1-g 修订前为「13 行恒在」，见该节）；`reclaim.action` 按终态填、`eligibility` 按切片期填；`inventory` 键恒在（首次读 `status = neverRun`）；T1-h 遗留清零（A-1 / D 组已于 2026-11-21 完成，剩余项见 [剩余清单](./I61-storage-manager-remaining.md) `R-16` / `R-17`）。

### T1-a 契约与 DTO

- [x] `src-tauri/src/protocol/dto/storage.rs`：新增 9 个封闭枚举（`Dto` 后缀 + `rename_all = "camelCase"`，取值以 §6.2 表为准）
  - 核对（2026-11-20）：`StorageLayerDto` / `StorageByteBasisDto` / `StorageReclaimActionDto` / `StorageEligibilityDto` / `StorageReclaimReasonDto` / `StoragePolicySourceDto` / `StorageConfirmLevelDto` / `StorageReclaimStatusDto` / `StorageReclaimErrorCodeDto` 全部在 `storage.rs:90`–`:195`，取值与 §6.2 逐字一致（`StorageByteBasisDto` 仅 `FileLength` 且带 `#[default]`）
- [x] `src-tauri/src/protocol/dto/storage.rs`：新增 `ReclaimPolicyDto` / `ReclaimViewDto` / `StorageEntryDto`，字段顺序与 I61 §3.2 / §6.2 一致；输出侧 `Option` **不加** `skip_serializing_if`（键恒在、值 `null`）
  - 核对（2026-11-20）：三者在 `storage.rs:196`–`:250`，字段顺序与 §3.2 逐条对应；全部 `Option` 无 `skip_serializing_if`；`ReclaimPolicyDto` 另有 `Default`（`source = None`、`fail_closed = true`）
- [x] `src-tauri/src/protocol/dto/storage.rs`：`StorageBreakdownDto` **只追加** `sampledAt` / `byteBasis` / `sharedBytes` / `referencedOnceBytes` / `entries` / `complete`，全部 `#[serde(default)]`；既有 4 字段零改动（G13 / V16）
  - 核对（2026-11-20）：`storage.rs:30`–`:52` 六个字段全部带 `#[serde(default)]`，既有 `total_bytes` / `total_count` / `orphans` / `conversations` 键名与类型未动；实际还多一个 `inventory`（T1-f 追加，同样 `#[serde(default)]`），与本条不冲突
- [x] `src-tauri/src/protocol/dto/storage.rs`：`StorageEntryDto` 必填 / 可选字段划分正确，且未新增 §6.2 之外的 DTO（provider 返回类型走普通结构：T1-g 前为 `probe.rs` 的 `ProbeStats`，T1-g 后为 `section.rs` 的 `SectionStats` / `SectionError`）
  - 核对（2026-11-20）：`storage.rs:240`–`:250` 必填 `key` / `layer` / `owner` / `includes_derived` / `counts_self_logs` / `availability` / `reclaim`，可选 `bytes` / `count` / `oldest_at` / `newest_at` / `availability_reason`；`probe.rs` 的 `ProbeStats` / `ProbeError` 是普通结构，不在 DTO 模块（T1-g 落地时随 `probe.rs` 退役改为 `SectionStats` / `SectionError`，字段语义不变）
- [x] `src-tauri/tests/protocol_bindings.rs`：新增类型全部登记进 `use` 与 `decl(&cfg)` 列表（漏一个直接红，N18）
  - 核对（2026-11-20）：`protocol_bindings.rs:30`–`:43` 的 `use` 与 `:221`–`:265` 的 `decl(&cfg)` 两侧条目一致，含全部 `Storage*` / `Reclaim*` 新类型
- [x] `npm run contract:gen` 后提交 `src/types/protocol.contract.ts`（禁止手改生成文件）
  - 核对（2026-11-20）：生成文件已含 `StorageEntryDto` / `ReclaimViewDto` / `StorageInventoryNodeDto` 等同名类型（camelCase 字段），无手写痕迹

### T1-b owner stats 入口（条目全集，按 I61 §3.10 表逐个落地）

> 本节「13 条」是 T1-g 修订前的旧口径（`vault` 构建 13 条、`vaultless` 构建 8 条）；下方带「13 条共用」的两条只描述**遍历 / 伴生口径**，口径本身与条目数无关，其余条目均为 core 8 条 + 分区条目按 `entries()` 快照顺序（改写表见 T1-g 修订末尾）。

- [x] **遍历口径（13 条共用，唯一解释）**：各 owner stats 一律「递归 + `symlink_metadata`（不跟随链接）」；只累加常规文件长度，符号链接 / 非常规文件跳过、不计数、**不**使条目降级（§3.9 表末行按「不跟随、不计数、只跳过」执行）；`unreadable` 只由 `read_dir` / `metadata` 失败触发（§7.1）
  - 核对（2026-11-20）：`for_each_file`（`assistant-tools/src/runtime/storage.rs:409`）实现递归 + 跳符号链接 + 跳非常规文件，`dir_tree_stats` / `purge_tree_files` 共用；`assistant-vault/src/storage_stats.rs`、`app_dirs/fs_stats.rs`、`assistant-crash/src/crash_dump.rs` 各自同规则（`NotFound` → 空态，其余 `Err`）；`attachment_store::list_all` / `list_derived` 已改用 `file_type()` / `symlink_metadata` 跳过链接 → 已落地
- [x] **伴生文件口径（13 条共用）**：`-journal` / `-wal` / `-shm` / `.tmp` / `.lock` / 部分写入残留**不计入** `bytes` / `count`，也不单列条目；由 owner 在回收时一并清理（§3.9 包含范围）
  - 核对（2026-11-20）：`COMPANION_SUFFIXES` + `is_companion_file` 已落地于 `assistant-tools/src/runtime/storage.rs`（`dir_tree_stats` 跳过 / `purge_tree_files` 清理）、`app_dirs/fs_stats.rs`（`tree_stats` 跳过）、`assistant-vault/src/storage_stats.rs`、`attachment_store.rs`（`list_derived` 跳过 / `clean_derived_companions` 清理）；单文件条目（`setup::db_file_stats`、vault `file_stats`）只 `metadata` 主文件、不读边车；`crash_dump::walk_stats` 无后缀过滤（该目录不产生伴生文件，E13 的三类文件名过滤见下方条目）→ 统计侧已落地
- [x] `src-tauri/crates/assistant-storage/src/attachment_store.rs`：`list_all` 弃用会**跟随**链接的 `Path::is_file()`，改「不跟随」判定（`symlink_metadata` / `DirEntry::file_type`）并跳过符号链接与非常规文件；其余语义（`created_at = modified()`、`read_dir` / `metadata` 失败仍 `?` 上抛、`E1` 不参与 500 ms 超时、失败即整命令 `Err`）原样不动（§3.9 / G5 / §7.1 特例 1）
  - 核对（2026-11-20）：`list_all` 现为 `entry.file_type().await?` + `is_symlink()` / `!is_file()` 跳过（`attachment_store.rs:443`–`:450`），未用字面 `symlink_metadata(&path)` 但同为不跟随链接；失败仍 `?` 上抛；无对应符号链接用例（见 T1-e 条）
- [x] **跨 crate 类型约定**：不为共享 `DirStats` 新增跨 crate 依赖——`assistant-storage` 用自有 `AttachmentDirStats`，`assistant-tools` / `assistant-vault` / `assistant-crash` 各自定义同形状 `{ bytes, count, oldest_at, newest_at }`（`bytes` / `count` 为 `u64`，`Option` 语义只在 DTO 层表达）；统一由 `cmd/storage` 映射进 `StorageEntryDto`
  - 核对（2026-11-20）：`AttachmentDirStats`（`attachment_store.rs:91`）与 `assistant_tools::DirStats` / `assistant_crash::DirStats` / `assistant_vault::storage_stats::DirStats` 四份同形状副本均在；`cmd/storage/sample.rs` 用 `StatsView` + `impl_stats_view!` 统一取值，`E1` / `E2` 走 `attachment_sample` / `derived_sample`；`mascot_asset::asset_stats` 实际复用 app 内 `app_dirs/fs_stats::DirStats`（非条目字面的 `assistant_tools` 副本），同样未新增跨 crate 依赖 → 已落地
- [x] `src-tauri/crates/assistant-storage/src/attachment_store.rs`：新增 `pub async fn derived_stats(&self) -> Result<AttachmentDirStats, AttachmentError>`——统计 `derived/` 下常规文件（跳符号链接 / 非常规 / 伴生），`oldestAt` / `newestAt` 取 `modified()`，空目录两端 `None`
  - 核对（2026-11-20）：已实现（`attachment_store.rs:293`），经 `list_derived` 复用同一遍历与伴生过滤；`derived/` 是扁平目录（`derived_path` 不嵌套），无递归分支；用例 `tests/attachment_store.rs:108` 覆盖空目录 / `.tmp` / 字节合计（无符号链接用例，见 T1-e 条）
- [x] `src-tauri/crates/assistant-tools/src/runtime/storage.rs`：新增 `pub struct DirStats { pub bytes: u64, pub count: u64, pub oldest_at: Option<i64>, pub newest_at: Option<i64> }`——字段形状全仓库统一（`bytes` / `count` 为 `u64`，`null` 语义只在 DTO 层；`oldest_at` / `newest_at` 取 `modified()`，空目录 `None`）
  - 核对（2026-11-20）：结构在 `storage.rs:315`–`:321`（`#[derive(Default)]`），`assistant_tools::DirStats` 经 `lib.rs` 的 `pub use storage::*` 对外可达；`setup::DbState::db_file_stats` 实际返回 app 内 `app_dirs/fs_stats::DirStats`（同形状），未复用本副本 → 结构已落地
- [x] `src-tauri/crates/assistant-tools/src/runtime/storage.rs`：新增自由函数 `pub fn result_stats(root: &Path) -> io::Result<DirStats>`（**不带 `retention_days`**，只按目录遍历）；`NotFound` → `Ok(DirStats::default())`（空态，由管理器判 `missing` 计 `0`），其余 io 错误 → `Err`（由管理器判 `unreadable` + `null`，V2）；供 owner 内部与测试用，管理器只经 trait 方法取
  - 核对（2026-11-20）：`result_stats`（`storage.rs:333`）转调 `dir_tree_stats`，`for_each_file` 对根 `NotFound` 静默返回、其余失败经 `traversal_error` 上抛；用例 `assistant-tools/tests/storage.rs:559`（统计 / 清理同口径）、`:581`（伴生跳过）
- [x] `src-tauri/crates/assistant-tools/src/runtime/storage.rs`：抽一层共用遍历 `for_each_file(root, on_file, on_error)`（递归 / 跳符号链接 / 跳非常规文件），`result_stats` 与 `cleanup_expired_results`（及 T3 的 `collect_expired`）**全部基于它**，禁止各自写 `read_dir` 循环（§3.10 接口形状 2）
  - 核对（2026-11-20）：`for_each_file`（`storage.rs:409`）为唯一遍历点，`dir_tree_stats`（`:336`）、`purge_tree_files`（`:374`）、`sweep_directory`（`:502`）均经它；`collect_expired` / `delete_candidates` 现为 trait 默认 `None`（T3 实现时须复用本函数）
- [x] `src-tauri/crates/assistant-tools/src/runtime/storage.rs`：`FileToolResultStorage` 增 `retention_days: AtomicU32`（默认 `0` = 禁用，即 fail-closed）与 `effective_since: i64`，并提供**链式注入** `pub fn with_retention(self, days: u32, effective_since: i64) -> Self`——**不改 `new(root)` 签名**；`result_stats` / `result_retention` 与清理入口都**只取注入值**，不自行读配置
  - 核对（2026-11-20）：字段在 `storage.rs:129`–`:134`，`new`（`:137`）置 `0` + `now_seconds()`，`with_retention` 在 `:149`；`result_stats`（`:160`）/ `result_retention`（`:165`）只读注入值；`FileToolResultStorage::new(...)` 调用点未改（`assistant-tools/tests` 现 17 处）→ 已落地
- [x] `src-tauri/crates/assistant-tools/src/runtime/storage.rs`：`ToolResultStorage` trait 增 `fn result_stats(&self) -> Option<io::Result<DirStats>>`（默认 `None` = 该实现不提供 stats）与 `fn result_retention(&self) -> Option<(u32, i64)>`（保留天数 + 注入时刻，供 `E3` 的 `policy` 自述；默认 `None` = 未注入）；`FileToolResultStorage` 覆盖两者
  - 核对（2026-11-20）：trait 默认实现在 `storage.rs:89` / `:94`（返回类型为 `io::Result<DirStats>`，非条目早期写的 `String`）；`FileToolResultStorage` 在 `:227` / `:231` 覆盖 → 已落地
- [x] `src-tauri/crates/assistant-tools/src/runtime/storage.rs`：`ToolResultStorage` 增 `async fn collect_expired(&self) -> Option<Vec<ExpiredCandidate>>` 与 `async fn delete_candidates(&self, &[ExpiredCandidate]) -> Option<DeleteReport>`（默认 `None` = 不提供，T3 用）
  - 核对（2026-11-20）：默认实现与 `ExpiredCandidate { path, bytes }` / `DeleteReport { removed, removed_bytes, failed }` 已在 `storage.rs:101` / `:108` / `:119`–`:127`；`FileToolResultStorage` 尚未覆盖（T3）
- [x] `src-tauri/crates/assistant-tools/src/os/tools/super_index/`：新增 `pub fn index_stats(root: &Path) -> io::Result<DirStats>`（`NotFound` → `Ok` 空态、其余 io 错误 → `Err`，与 `result_stats` 同语义），并由 `os` 模块 `pub use` 再导出，管理器只调公开入口（G1，§3.10 接口形状 3）
  - 核对（2026-11-20）：`super_index/mod.rs:29` 的 `index_stats` 复用 `dir_tree_stats`；`os/tools/mod.rs:15` 保持 `pub(crate) mod super_index;`、`:18` 为 `pub use super_index::{index_stats, purge_index_files, IndexPurgeReport};`，`os/mod.rs:12` 再导出 → `assistant_tools::os::index_stats` 可达；用例 `assistant-tools/tests/index_stats.rs`
- [x] `src-tauri/src/cmd/mascot_asset.rs`：新增 `pub fn asset_stats(root: &Path) -> io::Result<DirStats>`——遍历素材子树，**排除 `.tmp` 伴生文件**，符号链接与非常规文件不计入（§3.9 包含范围）
  - 核对（2026-11-20）：`asset_stats`（`mascot_asset.rs:51`）直接转调 `app_dirs::fs_stats::tree_stats`（伴生过滤 + 跳链接）；签名为取 `root` 入参、返回 `io::Result`（非条目早期写的 `pub(crate) ... Result<_, String>`）；用例 `:362`（`.tmp` 排除）、`:376`（缺失目录空态）
- [x] `src-tauri/src/setup.rs`：`DbState` 扩为 `pub struct DbState(pub Arc<Mutex<Connection>>, pub PathBuf)`——`Arc` 让 future 满足 `Send`（`reclaim` 要 `await` async 删除），既有 `state.0.lock()` 靠自动解引用不受影响；构造点改为 `DbState(Arc::new(Mutex::new(conn)), dirs.database())`；新增 `pub fn db_file_stats(&self) -> io::Result<DirStats>`：**只 `metadata` 主库文件**，不打开 DB、不读 `-journal` / `-wal` / `-shm`；`count` 恒 `0` / `1`，`oldest_at` / `newest_at` 取该文件 `modified()`（§3.9 / §7.1 特例 2）
  - 核对（2026-11-20）：结构在 `setup.rs:25`–`:28`（另有 `Clone` 实现），构造点 `:92`（`db_path` 在 `:72` 取一次）；`db_file_stats`（`:43`）转调 `fs_stats::file_stats`（只 `symlink_metadata`，`NotFound` → 零值）；单测 `:359`（缺失不建库）、`:373`（存在按长度计）
- [x] `src-tauri/crates/assistant-vault/src/security_db/state.rs`：新增 `pub fn db_file_stats(&self)` / `pub fn backup_stats(&self)` / `pub fn audit_stats(&self)`（均 `io::Result<DirStats>`），分别对应主库文件、备份目录、审计 JSONL 目录；备份目录路径经内部 `backup_dir_for` 取、**不对外暴露**；`DirStats` 定义在 `assistant-vault` 内（与 `assistant-tools::DirStats` 同名但独立，跨 crate 不共享），`security_db/mod.rs` 需补 `pub use state::{DirStats, SecurityDbState};` 供 `vault_plugin.rs` 取类型
  - 核对（2026-11-20）：三方法在 `state.rs:539` / `:544` / `:551`（`audit_stats` 走 `audit_writer().stats()`，与 `prune_before` 同口径；备份侧用 `is_backup_file` 判据过滤）；`security_db/mod.rs:16` 已导出 `DirStats` → 已落地
- [x] `src-tauri/crates/assistant-vault/src/security_db/state.rs`：`set_audit_retention_days` 内一并写入注入时刻，并新增 `pub fn audit_policy(&self) -> Option<(u32, i64)>`（保留天数 + 生效时刻）供 `E11` 的 `policy` 自述；管理器不读配置（§3.5 硬约束 1）
  - 核对（2026-11-20）：字段名 `audit_effective_since: AtomicI64`（`state.rs:30`，构造于 `:47` 置本刻）；`set_audit_retention_days`（`:59`–`:64`）同一次写入天数与时刻；`audit_policy`（`:559`）返回 `Option`（本类型恒 `Some`，取不到状态时由 provider 层回 `None`）→ 已落地
- [x] `src-tauri/crates/assistant-vault/src/security_db/state.rs`：`prune_backups` / `MAX_BACKUPS` 的可达性留给 T3；T1 只读，不改可见性
  - 核对（2026-11-20）：现状 `prune_backups` 为 `pub(super)`（`state.rs:502`）、`MAX_BACKUPS` 为 `mod.rs:27` 的私有 `const`，T1 未改；`purge_backups` 已是 `pub(crate)`（`state.rs:484`），T3 接入回收入口时再定
- [x] `src-tauri/crates/assistant-crash/`：`pub fn dump_stats(dir: &Path) -> io::Result<DirStats>`（已在 `crash_dump.rs:26`）；`crash_dump.rs` 内定义 `DirStats`（本 crate **不依赖** `assistant-tools`，不得为共享结构新增依赖），`lib.rs` 再导出 `pub use crash_dump::{dump_stats, install, DirStats};`（已在 `lib.rs:3`）；只统计 `panic_*.log` / `crash_*.dmp` / `crash_*.log` 三类常规文件——**剩余缺口见 T1-h A-1**（`walk_stats` 文件名判据 + 单测）
  - 核对（2026-11-20）：`dump_stats`（`crash_dump.rs:26`）、`DirStats`（`:17`）、`lib.rs:3` 的 `pub use crash_dump::{dump_stats, install, DirStats};` 均已落地，管理器入口 `sample.rs:243` 已在用；**但 `walk_stats`（`:30`）按「任意常规文件」计数，没有三类文件名判据**，与条目口径不一致；且 `crash_dump.rs` 的 `mod tests`（`:236`）只有 `prepare_dir` / `panic_hook` 三例，**无 `dump_stats` 用例**、`src-tauri/tests` 也无 E13 验收 → 本条不勾选
  - 核对（2026-11-21）：`walk_stats` 已加 `is_crash_artifact()`（`crash_dump.rs:37`，`panic_*.log` / `crash_*.dmp` / `crash_*.log` 前缀 + 后缀双匹配，其余文件跳过、不降级），`mod tests` 补 3 例、`tests/storage_breakdown.rs` 补 2 例 E13 → 已落地（T1-h A-1 关闭）
- [ ] ~~`E12`（`logs/`）**不新增任何 owner 方法**：管理器直接产出 `availability = notImplemented` + `availabilityReason` 固定文案「`tauri-plugin-log` 不暴露 stats」+ `bytes` / `count = null`（§3.10 表末行）；该文案进 golden 快照，**不得**改写为其他措辞~~ **（已作废：方案 C 删除 `availabilityReason`，缺口说明改由 `reclaim.note` 承载——见 [剩余清单](./I61-storage-manager-remaining.md) `R-16` / `R-40`）**
  - 核对（2026-11-20）：`sample.rs:407`–`:411` 已产出 `NotImplemented` + 该固定文案（无 owner 方法、无 `logs` 目录遍历）；`entries.rs:230`–`:238` 的 `E12` 静态项为 `rotate` / `ownerManaged` / `externalOwner` / `keepCount = 3`。**但仓库内不存在任何 golden 快照夹具**（全仓无 `golden*` 文件、`tests/` 无对应断言），文案未被测试锁死 → 本条不勾选（剩余缺口见 T1-h A-2）

### T1-c 可选能力 provider（唯一 feature 缝）〔已被 T1-c 修订 → 再被 T1-g 修订取代，仅存历史核对〕

> **已被 T1-g 修订取代**：本节按「可选条目由 core 登记表声明、provider 按能力名接收」实现，条目清单 / 策略 / 资格判据仍在 core。保留为历史核对记录，**不作验收依据**；实现条目以 T1-g 修订为准。

- [x] **结构改造**：`src-tauri/src/cmd/storage.rs` 改为目录模块 `src-tauri/src/cmd/storage/mod.rs`（子模块 `probe.rs` / `inventory.rs` / `reclaim.rs` / `entries.rs` / `sample.rs` 只在 `mod.rs` 声明，`inventory.rs` 文件头注明「G2 唯一例外」）；模块路径仍是 `cmd::storage`，`cmd/mod.rs` 与 `ipc.rs` 的命令引用**零改动**；**禁止** `#[path]` 挂载或残留同名 `storage.rs`（`assembly_contract` 按常规布局扫描源码）
  - 核对（2026-11-20）：`cmd/storage/mod.rs:1`–`:5` 声明 `pub mod entries; pub mod inventory; pub(crate) mod probe; pub mod reclaim; mod sample;`；`cmd/storage.rs` 已不存在；`inventory.rs:3` 文件头写明 G2 例外边界；`ipc.rs` 的 `cmd::storage::…` 引用未动 → 已落地

- [x] `src-tauri/src/cmd/storage/probe.rs`：定义 `pub trait StorageOwnerProbe: Send + Sync`，stats 方法**同步阻塞**、另加例外 `fn audit_policy(&self) -> Option<(u32, i64)>`（`None` = 取不到 `SecurityDbState`，只让 `E11` 的 `policy` 两数字填 `null`、**不**整条降级）；参数与返回值只用普通数据，**不得**出现 `assistant_vault` / `assistant_security` 类型（§3.10 provider 第 1 条）——本形态**已被 G17 取代**（五个具名方法收敛为 `stats(key)`，见下方「T1-c 修订」）
  - 核对（2026-11-20）：T1 原形态（`probe.rs:37`–`:60`，五个具名 stats 方法 + `audit_policy`）已被下方「T1-c 修订」取代并落地：现 trait 只有 `capability()` / `stats(key)` / `audit_policy()`（`probe.rs:41`–`:56`），全文件仍无 vault 类型引用、无 `#[cfg]` 门控
- [x] `src-tauri/src/cmd/storage/probe.rs`：trait 返回值用 `probe.rs` 内定义的普通结构 `ProbeStats { bytes / count / oldest_at / newest_at }`（后两者 `Option<i64>`）；vault 侧 provider 负责把 `assistant_vault` 的 stats 映射成它，保证 vault 类型一个都不出现在管理器
  - 核对（2026-11-20）：`ProbeStats` 在 `probe.rs:16`–`:21`（四字段全 `Option`）；映射函数 `stats_of`（`vault_plugin.rs:27`）把 `DirStats` 的四字段一一转成 `ProbeStats` → 已落地
- [x] `src-tauri/src/cmd/storage/probe.rs`：trait 每个方法返回 `Result<ProbeStats, ProbeError>`（收敛后为 `stats(key) -> Option<Result<ProbeStats, ProbeError>>`，`None` = 不归本 owner），并定义 `pub enum ProbeError { Locked, Unavailable, Unreadable }`（**封闭三值，不新增**）；管理器固定映射 `Locked/Unavailable/Unreadable` → 同名 `availability` 且 `bytes` / `count = null`；**`E9` 锁定只能靠 `Locked` 表达**（`ProbeStats` 无状态位）；**`E7` 的 `metadata` 失败必须回 `Unavailable`**（不是 `Unreadable`），管理器据此置 `eligibility = locked` + `reason = locked`（§3.1 E7 行）
  - 核对（2026-11-20）：枚举在 `probe.rs:28`–`:32`；映射 `probe_error`（`sample.rs:132`）三值一一对应；`E9` 锁定在 `entries.rs:269`、`E7` 的 `Unavailable` → `eligibility = locked` + `reason = locked` 在 `entries.rs:275`–`:278`；`vault_plugin.rs:33` 的 `map_file_io` 把单文件 `metadata` 失败固定归 `Unavailable` → 已落地
- [x] `src-tauri/src/cmd/storage/probe.rs`：定义 `pub type StorageOwnerProbeHandle = Arc<dyn StorageOwnerProbe>;`；**注入端与取用端必须用同一别名**（类型不一致会静默取不到，表现为「永远降级」）
  - 核对（2026-11-20）：别名在 `probe.rs:59`；provider 端 `vault_plugin.rs:129` 显式标注该别名后交给 `ProbeRegistry::register`（不再 `app.manage`）；管理器取用端统一 `try_state::<Arc<ProbeRegistry>>()`（`sample.rs:311` / `mod.rs:208` / `inventory.rs:586`），槽类型与注册类型同源 → 已落地
- [x] `src-tauri/src/cmd/storage/probe.rs`：trait 与别名**不得**被 `#[cfg]` 门控（vaultless 构建也要有类型可写降级分支）
  - 核对（2026-11-20）：`probe.rs` 全文无 `#[cfg(feature = ...)]`（仅 `mod.rs:3` 的 `pub(crate) mod probe;` 无门控），文件头 `:3`–`:4` 明示「无条件编译」；`--no-default-features` 构建已通过 → 已落地
- [x] `src-tauri/src/vault_plugin.rs`：在 `#[cfg(feature = "vault")] mod mounted` 内实现 vault provider（`security_db_stats` / `backup_stats` / `audit_stats` 转调 `SecurityDbState`；`repo_stats` 先判 `is_locked()`（锁定 → `Err(ProbeError::Locked)`）再走仓库路径观测；`recovery_stats` 走恢复副本目录），并在插件 `setup` 里以 `StorageOwnerProbeHandle` 别名 `manage`；`AppDirs` **不是** managed state，每次调用 `AppDirs::resolve()`，其失败必须映射为 `Err(ProbeError::Unavailable)`（**不得** `?` 直接传播成别的错误类型）。五个具名方法在收敛改造后合并为 `stats(key)` 分派，逻辑与错误映射不变
  - 核对（2026-11-20）：实现体为 `VaultStorageProbe`（`vault_plugin.rs:24`–`:26`），`impl StorageOwnerProbe` 在 `:62`–`:113`：`stats(key)` 按 `security/security.db` / `security/backups` / `security/vault` / `security/recovery` / `security/audit` 五个登记 key 分派，`security/vault` 先 `state.is_locked()`（锁定 → `ProbeError::Locked`）再调 `vault::repo_stats`（内部用 `repo_dir`），`security/recovery` 调 `vault::recovery_stats`（内部用 `recovery_dir`）；`dirs()`（`:56`–`:58`）把 `AppDirs::resolve()` 失败固定归 `Unavailable`；`map_file_io`（`:39`）归 `Unavailable`、`map_io`（`:44`）归 `Unreadable`；注入改在 `:129` 以别名交给 `ProbeRegistry`（`:127`–`:142`），位于 `vault_setup`（`:126`）之后 → 已落地
- [x] `src-tauri/src/vault_plugin.rs`：provider 持 `AppHandle`，**调用时**才 `try_state::<SecurityDbState>()`（不在构造期缓存句柄，避免 vault 重建后持到失效引用；取不到 → 返回 `Err`，管理器按 `unreadable` 处理，**不** panic）；注入点必须在 `crate::cmd::vault::vault_setup(app)?` **之后**（`SecurityDbState` 在该函数内 `manage`），否则 `try_state` 恒失败、`E7`–`E11` 静默永久降级
  - 核对（2026-11-20）：`db_state()`（`vault_plugin.rs:49`–`:53`）每次调用 `try_state`、取不到回 `ProbeError::Unreadable`；注册 `:127`–`:142` 在 `vault_setup` `:126` 之后 → 已落地
- [x] `src-tauri/src/vault_plugin.rs`：`#[cfg(not(feature = "vault"))] mod mounted` 保持**不注入**（不得塞一个假实现）
  - 核对（2026-11-20）：`vault_plugin.rs:238`–`:251` 的非 vault 分支只有 `init` / `on_setup` / `on_exit`，无任何 `manage` / `register` → 已落地
- [x] `src-tauri/src/cmd/storage/`：只 `app.try_state::<StorageOwnerProbeHandle>()`；取不到 → `E7`–`E11` 五条 `availability = notImplemented` + `availabilityReason` 写「可选能力未装配」+ `bytes` / `count = null`，`eligibility` 按 §3.10 第 5 条填（`E7` `notEligible`/`businessData`、`E9` `notEligible`/`userData`、`E8` `notImplemented`/`noReclaimApi`、`E10`/`E11` `ownerManaged`）
  - 核对（2026-11-20）：`sample.rs:310`–`:312` 取 `try_state::<Arc<ProbeRegistry>>()`；`E7`–`E11` 统一走 `probe_sample_for`（`:145`–`:170`）：登记表 `Feature` 行 → `probes.find(capability)`，取不到（槽缺失 / 能力未注册）→ `unmounted_probe_sample()`（`entries.rs:430`–`:434`，文案「可选能力未装配（StorageOwnerProbe 未注入）」）；`eligibility` 来自 `entries.rs:static_for` 的条目 id 匹配：`E7`（`:182`）`notEligible`/`businessData`、`E8`（`:191`）`notImplemented`/`noReclaimApi`、`E9`（`:200`）`notEligible`/`userData`、`E10`（`:209`）/`E11`（`:218`）`ownerManaged` → 已落地
- [x] 门禁自查：`cmd/storage/` **无** `use assistant_vault` / `assistant_security`、**无** `#[cfg(feature = ...)]`（G1）
  - 核对（2026-11-20）：`cmd/storage/` 五个文件均无 vault / security crate 的 `use`，也无 `#[cfg(feature = …)]`（`vault_plugin.rs` 才是唯一 feature 缝）→ 已落地

### T1-c 修订 — 声明与接收统一为「能力名 + key」+ 通用槽（I61 §3.8 G17 / G18 / G19 / §3.10 取用第 3–5 条 / §3.11.1 登记纪律 4–6 / X15 / X16 / X17 / X18 / V22 / V23 / V24）〔已被 T1-g 修订取代，仅存历史核对〕

> **已被 T1-g 修订取代**：本节把「可选条目仍由 core 登记表声明」保留了下来（`SECURITY_OBSERVABILITY` 能力名 + `RootMount::Feature` + `E7`–`E11` 五行），且 V22–V24 的断言按能力名口径写死。保留为历史核对记录，**不作验收依据**；对应断言需按 T1-g 修订删除或改写。

> 触发原因：原实现有三套词汇描述同一件事——登记表用 `key` 字符串声明身份、管理器用**数字下标**接收（`sample.rs` 的 `const SECURITY_DB: usize = 6` + 一句「与登记表同序」的注释）、provider 用五个具名方法声明归属。三者之间唯一的连接是注释，没有任何断言：在 `MANAGED_ROOTS` 中间插一条登记，下标全部静默错位一格，编译通过、测试通过，`E7` 显示 `E6` 的字节数。规格已改为「登记表 `Feature(SECURITY_OBSERVABILITY)` + `capability()` 声明、按 `key` 接收」；**槽位名是能力名（`"security-observability"`），不是实现类型名（`"VaultProbe"`）、不是 feature 名（`"vault"`）**——换实现 / 改类型名 / 特性退役时登记表不动（G18(g)）。

- [x] `src-tauri/src/app_dirs/registry.rs`：加 `pub const SECURITY_OBSERVABILITY`（单点定义）；`RootMount::Feature` 改携带能力名；`E7`–`E11` 填 `Feature(SECURITY_OBSERVABILITY)`；`owner` 标签按 G17(a) 改净并同步 golden（能力名只作路由 token，不进 DTO）
  - 核对（2026-11-20）：`registry.rs:29` 定义 `pub const SECURITY_OBSERVABILITY: &str = "security-observability"`；`RootMount::Feature(&'static str)`（`:39`）携带能力名，`E7`–`E11`（`:126`/`:136`/`:146`/`:156`/`:166`）全部填 `Feature(SECURITY_OBSERVABILITY)`；`validate_registry` 增「`Feature` 行能力名不得为空」（`:276`）；`owner` 标签已改净（无 `::`、非 feature 名、非类型名，V22 单测逐条断言）→ 已落地
- [x] `src-tauri/src/cmd/storage/probe.rs`：`StorageOwnerProbe` 增 `capability()`，五个具名 stats 方法收敛为 `stats(key)`（`None` = 不归本 provider），`audit_policy()` 保留（策略自述，非按条目路由）
  - 核对（2026-11-20）：`probe.rs:41`–`:56` 只剩 `capability()` / `stats(key)` / `audit_policy()` 三个方法，五个具名方法已删除；`None` 语义与「不得兜底成空目录」写进 doc → 已落地
- [x] `src-tauri/src/vault_plugin.rs`：`VaultStorageProbe::capability()` 返回 `crate::app_dirs::registry::SECURITY_OBSERVABILITY`；`stats(key)` 按 `key` 分派到既有 `SecurityDbState` / `repo_dir` / `recovery_dir` 实现（逻辑与错误映射沿用现五个方法），并删除五个具名方法
  - 核对（2026-11-20）：`vault_plugin.rs:62`–`:112` 的 `capability()` 返回 `registry::SECURITY_OBSERVABILITY`；`stats(key)` 按 `E7`–`E11` 五个登记 key 分派（锁定态先判锁再观测的语义保留），其余 key 一律 `None`；五个具名方法已删除 → 已落地
- [x] `src-tauri/src/cmd/storage/probe.rs`：新增 `pub struct ProbeRegistry`（`Mutex<Vec<StorageOwnerProbeHandle>>`）与 `new` / `register`（同能力名返回 `Err` 且**不覆盖**）/ `find(capability)` / `snapshot()` / `capabilities()`；**无条件编译、不含任何 `#[cfg(feature = ...)]`**（G18(a)(c)(d)(e)）；provider 不得各自 `app.manage` 成独立 state（`TypeId` 单槽会互相覆盖，X17）
  - 核对（2026-11-20）：`probe.rs:66`–`:113` 落地 `ProbeRegistry` 与全部方法；`register` 对空能力名与重复能力名都返回 `Err` 且不覆盖先到者；`capabilities()` 去重由注册表唯一性保证；全文件无 `#[cfg(feature)]`；`cargo check --no-default-features --lib` 通过（vaultless 构建同源编译）→ 已落地
- [x] `src-tauri/src/plugins.rs` + `src-tauri/src/vault_plugin.rs`：**建槽与注册同层**（G18(b)）——`plugins::attach` 的 `Builder` 阶段 `.manage(Arc::new(ProbeRegistry::new()))`（插件 `setup` 早于 app `setup`），`vault_plugin` 的 `setup` 改为 `registry.register(probe)`，`Err` 时 `log::error!` 并继续（不得 panic、不得覆盖）
  - 核对（2026-11-20）：`plugins.rs:8`–`:11` 在 `Builder` 阶段 `manage(Arc::new(ProbeRegistry::new()))`；`vault_plugin.rs:126`–`:141` 改为 `try_state::<Arc<ProbeRegistry>>()` → `register(probe)`，`Err` / 取不到槽都只 `log::error!` 后继续，不再 `app.manage` → 已落地
- [x] `src-tauri/src/cmd/storage/mod.rs`：取用改为 `app.try_state::<Arc<ProbeRegistry>>()`（取不到按「可选能力未装配」降级，不 panic）→ 遍历 `MANAGED_ROOTS` 的 `Feature(capability)` 行 → `registry.find(capability)` → 逐行 `stats(row.key)`；**禁止**「遍历全部 provider 取第一个返回 `Some` 的」（G18(d)）；**禁止**在登记表里塞兜底 / 默认值语义（G19）
  - 核对（2026-11-20）：`mod.rs:206`–`:209` 改 `try_state::<Arc<ProbeRegistry>>()`；路由本体落在 `sample.rs:probe_sample_for`（`Feature` 行 → `probes.find(capability)` → `stats(root.key)`，`Always` 行直接 `NotImplemented`），全仓无「遍历全部 provider 取第一个 `Some`」；登记表仍只有声明、无兜底语义 → 已落地
- [x] `src-tauri/src/cmd/storage/sample.rs`：**删除** 12 个下标常量（`DERIVED = 1` … `CRASHES = 12`）与 `match index { … _ => audit_stats() }` 兜底分派，改为按 `key` 分派（`samples` 按 `key` 写入，或遍历 `MANAGED_ROOTS` 按 `root.key` 匹配）；`E7`–`E11` 改为遍历 `Feature(capability)` 行 → `registry.find(capability)` → 逐行 `stats(row.key)`
  - 核对（2026-11-20）：12 个下标常量与 `match index` 兜底分派已全部删除；`samples` 一律经 `set_sample(entry)` 写入，`E7`–`E11` 由 `for root in MANAGED_ROOTS { if Feature }` 自动路由（`sample.rs:399`–`:408`），策略经 `audit_policy_of`（`:169`）按 `E11` 的登记能力名取 → 已落地
- [x] `src-tauri/src/cmd/storage/entries.rs`：`LOGS_INDEX = 11` 改为按 `key` 判定；`MANAGED_ROOTS[index]` 改为按 `key` 查登记条目（**不要**复用 `ProbeRegistry::find(capability)`，那是能力名→provider 的查找）；`static_for(index)` 改为按 `key` / `entry` 匹配，去掉「下标即顺序」的隐含契约
  - 核对（2026-11-20）：`LOGS_INDEX` 改为 `LOGS_ENTRY = "E12"` 字符串常量；`build_entry` 的登记项改由 `find_by_entry(root.entry)` 取；`static_for(entry: &str)` 按条目 id 匹配；样本槽位由 `index_of_key(key)` 现算（不写下标常量），`set_sample` / `sample_for` 都按 `key` 定位；`inventory::subtract_declared` 也从 `samples[index]` 改为 `sample_for(samples, root)` → 已落地
- [x] 验收：V22 新增单测——遍历 `MANAGED_ROOTS`，每条 `Feature(capability)` 行 `stats(key)` 返回 `Some`、非本 provider 的 `key` 返回 `None`；登记表能力名 ⊆ `ProbeRegistry::capabilities()` 且两侧引用同一个 `const SECURITY_OBSERVABILITY`；`owner` 不含 `::`、不等于任何 feature 名、不是可选 crate 类型名；静态断言 `cmd/storage/` 下无「下标常量 + `MANAGED_ROOTS[index]`」组合。负例：①`Feature` 行改成 provider 不认识的 `key`；②`Feature(SECURITY_OBSERVABILITY)` 改成 `Feature("VaultProbe")` / `Feature("vault")` / 未注册常量；③`owner` 含 `::` / 等于 feature 名 / 是可选 crate 类型名
  - 核对（2026-11-20）：`sample.rs` 的 `every_feature_row_is_answered_by_its_declared_key`（五条 `Feature` 行全 `Available`、非本 provider key 与未登记 key 均 `None`、`feature_capabilities()` ⊆ `ProbeRegistry::capabilities()`）与 `unregistered_capability_degrades_every_feature_row`（负例②）、`feature_rows_route_by_capability_then_key`（负例①）覆盖；`registry.rs` 的 `owner_labels_are_not_routing_tokens` 覆盖负例③；`cmd/storage/` 已无下标常量（`grep "const .*: usize ="` 只剩数量上限，`MANAGED_ROOTS[index]` 仅存于「验证顺序」的测试断言）→ 已落地
- [x] 验收：V23 新增单测（`ProbeRegistry` 三态）——零槽（13 条恒在、`Feature` 行全 `notImplemented`、非 `Feature` 行不受影响）/ 单槽（与改造前逐字段等价）/ 双能力名（各答自己的 `key`、互不串台）。负例：①同能力名重复 `register` 返回 `Err` 且 `find` 仍返回先注册实例、`capabilities()` 无重复；②某行 `Feature("A")` 改成 `Feature("B")`（B 未注册）→ 该行 `notImplemented`、其余行逐字段不变；③`snapshot()` 后新增注册不影响已取快照
  - 核对（2026-11-20）：`probe.rs` 测试覆盖零槽 `empty_registry_answers_nothing`、单槽 `single_slot_answers_its_keys_only`、双能力名 `two_capabilities_do_not_cross_talk`、负例①`duplicate_capability_is_rejected_without_overwrite` + `empty_capability_is_rejected`、负例③`snapshot_ignores_later_registrations`；负例②由 `sample.rs` 的 `unregistered_capability_degrades_every_feature_row`（未注册能力 → 五条 `Feature` 行全 `notImplemented`，`Always` 行由 `always_rows_never_route_through_provider` 保证不受影响）→ 已落地
- [x] 验收：V24 新增静态断言——全仓能力名字面量只能出现在 `const SECURITY_OBSERVABILITY` 的定义与引用；能力名不含 `::`、不等于任何实现类型名 / feature flag 名；负例：把该常量值临时改成 `"VaultProbe"` 或 `"vault"` 断言失败
  - 核对（2026-11-20）：`registry.rs` 的 `capability_literal_only_lives_in_its_constant` 递归扫 `src` / `tests` / `crates` 下全部 `.rs`（排除定义文件 `app_dirs/registry.rs`）断言字面量零命中；同时断言不含 `::`、无大写、不等于 `"vault"` / `"VaultProbe"`（改值即判红）；测试内一律引用常量而非字面量 → 已落地

### T1-g 修订 — 可选功能分区自管理（`StorageSection` / `SectionRegistry` / `OPTIONAL_SEGMENTS`）（I61 §3.1.1 / §3.10「可选功能分区自注册」/ §3.8 G16–G19 / §3.11.1 登记纪律 2–6 / V21–V24）

> 未完成条目已迁到 [剩余清单](./I61-storage-manager-remaining.md)（`R-01` – `R-15`），本节的未勾选项保留原文并划掉，只作对照；完成状态以剩余清单为准。

> **本修订取代 T1-c 与 T1-c 修订的全部实现条目**。旧实现把「可选条目仍由 core 登记表声明、provider 按能力名接收」保留了下来：条目清单、策略、资格判据仍有一份在 core，功能侧只提供取数。规格已改为「条目清单与全部行为由功能分区自述，core 只做通用接收」——条目 `key` / `segments` / 策略 / 判据 / 回收实现全部在功能 crate，core 只保留首段磁盘事实 `OPTIONAL_SEGMENTS`。T1-c / T1-c 修订的代码与核对记录保留为历史事实，**不再作为验收依据**；其中 `SECURITY_OBSERVABILITY` 能力名、`RootMount::Feature` 行、`ProbeRegistry` 三态断言（V22–V24 旧口径）随本节作废。
>
> 影响面：`app_dirs/registry.rs`（8 条 + `OPTIONAL_SEGMENTS`）、`cmd/storage/section.rs`（新契约）、`cmd/storage/{sample,entries,inventory,reclaim}.rs`（按分区自述装配）、`plugins.rs`（槽）、`feature_mounts.rs`（注册时序）、`vault_plugin.rs`（适配器）、`assistant-vault/src/storage_section.rs`（自述）、`tests/{storage_breakdown,storage_inventory,storage_reclaim}.rs` 与 golden。

- [x] `src-tauri/src/cmd/storage/section.rs`（新建）：定义**无条件编译**的通用分区契约——`trait StorageSection`（`id` / `entries` / `stats` / `eligibility` / `policy` / `reclaim_actions` / `reclaim`）、`SectionEntry`（`entry` / `key` / `segments` / `scope` / `coverage` / `owner` / `inventory` / `decl`）、`SectionStats` / `SectionError` / `SectionReclaimReport`、`StorageSectionHandle = Arc<dyn StorageSection>`、`SectionRegistry { sections: Mutex<Vec<StorageSectionHandle>> }`；方法**同步阻塞**（管理器在 `spawn_blocking` 里调）；全文件无 `#[cfg(feature = ...)]`、无功能 crate 类型、无条目 `key` / `security` 字面量（G1 / G16 / §3.10 第 1–3 条）
  - 验收：`cargo check --no-default-features --lib` 绿；该文件检索 `vault` / `security` / `E7` 零命中
  - 核对（2026-11-21）：`cargo check --no-default-features --lib` 绿（37s，无 error / warning）；`section.rs` 检索 `vault` / `security` / `E7`–`E11` / `cfg(feature` 零命中（`is_empty` 注释由「vaultless 构建」改为「关闭特性构建」，与其它文件的措辞对齐）；另附 `SectionSample`（观测四元组 + 策略快照，`from_stats` / `from_error` / `with_policy` 三个构造器）与四个 `const fn` 策略助手（`no_policy` / `constant_policy` / `owner_policy` / `config_policy`），供 `entries.rs` / `reclaim.rs` 共用，避免 core 侧再写一份策略字面量
- [x] `src-tauri/src/cmd/storage/section.rs`：`SectionRegistry::register` 只做**结构校验**——分区 `id` 非空且唯一、`entries()` 非空、每条 `key == segments.join("/")`、`segments` 首段命中 `OPTIONAL_SEGMENTS`、条目 `entry` / `key` 不与 core 条目或其它已注册分区重名；失败返回 `Err` 且**不覆盖**先注册者；`snapshot()` 克隆 `Vec<Arc<…>>` 后立即释放锁（G18(c)(e) / V22 / V23）
  - 验收：单测覆盖「首段不在 `OPTIONAL_SEGMENTS`」「`key` 与 `segments` 不一致」「重名条目」「空清单」四组拒绝；`snapshot` 不随后续注册增长
  - 核对（2026-11-21）：8 条单测全绿——`empty_registry_has_no_sections` / `first_segment_outside_optional_segments_is_rejected` / `key_must_equal_segments_join` / `duplicate_core_entry_is_rejected` / `empty_entry_list_is_rejected` / `duplicate_section_id_and_key_are_rejected`（同一条覆盖「分区标识重复」「条目 key 重复」「条目 id 重复」三种拒绝）/ `snapshot_does_not_grow_after_later_registration` / `section_error_maps_to_fixed_availability`；拒绝路径在写入前返回，先注册者不被覆盖；`snapshot()` 在锁内 `clone` 后立刻出作用域，锁不跨调用方
- [ ] ~~`src-tauri/src/cmd/storage/mod.rs`：`mod section;` 引入新模块；删除 `pub(crate) mod probe;`，`cmd/storage/probe.rs` 整体移除（旧 trait / 注册表退役，不留双份契约）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
  - 验收：全仓检索 `StorageOwnerProbe` / `ProbeRegistry` / `StorageOwnerProbeHandle` 零命中
  - 进度（2026-11-21，部分完成）：`pub mod section;` 已加；`pub(crate) mod probe;` 仍在（`sample` / `inventory` / `reclaim` / `plugins` / `vault_plugin` 尚未切换，删早了会整片编译不过）——本项与第 5–9 项必须成组落地
- [ ] ~~`src-tauri/src/app_dirs/registry.rs`：`MANAGED_ROOTS` 删 `E7`–`E11` 五行（收敛为 8 条：`E1`–`E6` / `E12` / `E13`）；删除 `SECURITY_OBSERVABILITY` 与 `RootMount`（`Feature` 分支随条目一起消失，只剩 `Always` 时枚举无意义）；新增 `pub const OPTIONAL_SEGMENTS: &[&str] = &["security"];`（**只写首段**，不写条目 / 策略 / 实现）；`validate_registry` 改为校验 8 条 core 行 + `OPTIONAL_SEGMENTS` 非空、不含 `::`、不等于 feature 名 / 实现类型名（G16 / G19 / V19 / V24）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
  - 验收：`app_dirs_contract` 绿；`registry.rs` 内检索 `security/`（带子段）零命中；`OPTIONAL_SEGMENTS` 每项都是单段且无大写
  - 进度（2026-11-21，部分完成）：`OPTIONAL_SEGMENTS` 已新增并附语义边界注释（只写首段，条目 / 策略 / 实现留给功能 crate）；`MANAGED_ROOTS` 的 `E7`–`E11` 五行、`SECURITY_OBSERVABILITY`、`RootMount`、`validate_registry` 的 13 条口径均**未改**——它们被 `section.rs` 的 core 重名校验引用，须与第 5–9 项同批切换，否则注册校验失去对照表
- [ ] ~~`src-tauri/src/plugins.rs`：`manage(Arc::new(ProbeRegistry::new()))` 改 `manage(Arc::new(SectionRegistry::new()))`——仍在 `Builder` 阶段、先于 `feature_mounts::on_setup`（G18(b)，建槽与注册不得跨层）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
  - 验收：槽类型与 `sample.rs` / `inventory.rs` / `reclaim.rs` 的 `try_state::<Arc<SectionRegistry>>()` 同源
- [ ] ~~`src-tauri/src/feature_mounts.rs`：`on_setup` 钩子改为调用各功能装配缝导出的注册入口（当前只有 vault）；取不到槽或 `register` 返回 `Err` 时只 `log::error!` 后继续（不 panic、不覆盖、不静默降级成占位行）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
  - 验收：vaultless 构建下 `on_setup` 不注册任何分区，`entries` 只剩 core 8 条
- [ ] ~~`src-tauri/crates/assistant-vault/src/storage_section.rs`（新建）：用功能**自有普通类型**自述 `E7`–`E11`——`VaultStorageEntry` 清单（`entry` / `key` / `segments` / `scope` / `coverage` / `owner` / `inventory` / `decl`）+ `VaultStats` / `VaultSectionError`；`owner` 用 `SecurityDb` / `SecurityRepo` / `SecurityRecovery`，动作 / 确认档 / `autoActive` / `note` / 策略 / 资格判据 / 回收入口全部在此；**不依赖 app crate**（§3.1.1 依赖方向）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
  - 验收：`cargo test -p assistant-vault` 绿；crate 依赖图里无 `assistant-app`
- [ ] ~~`src-tauri/src/vault_plugin.rs`：`#[cfg(feature = "vault")] mod mounted` 提供 `StorageSection` 适配器——`id()` 返回分区标识（只用于日志 / 去重）、`entries()` 转发功能侧清单、`stats(key)` / `eligibility` / `policy` / `reclaim_actions` / `reclaim` 按 `key` 分派到功能侧并**无损映射**成 core 类型；适配器内**不出现**条目 `key` / `security` 字面量（G17(b) / V21 / V24）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
  - 验收：故意漏搬 `newestAt` 的负例被 V21 判红；适配器源码检索 `security/` 零命中
- [ ] ~~`src-tauri/src/vault_plugin.rs`：删除 `VaultStorageProbe` / `StorageOwnerProbe` 实现与 `ProbeRegistry` 注册；改为导出注册入口（`fn register_storage_section(app: &AppHandle)`）供 `feature_mounts::on_setup` 调用，分区实例由 app 侧 `SectionRegistry` 接收，插件不 `manage` 独立 state（G18(a)(d)）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
  - 验收：`vault_plugin.rs` 内 `app.manage` 零命中；注册入口只在 `on_setup` 被调用
- [ ] ~~`src-tauri/src/cmd/storage/sample.rs`：条目清单改为「core 8 条（登记表）+ 各分区 `entries()` 快照」；采样按 `key` → 所属分区索引取数（分区条目走 `stats(key)` + `eligibility` / `policy`，core 条目走既有入口）；分区 stats 同步阻塞，统一在 `spawn_blocking` 里调用并套同一套 **500 ms** 超时（超时只放弃等待、不取消）；删除 `RootMount::Feature` 分支、`probe_sample_for`、`unmounted_probe_sample`——**未挂载 = 该行不存在**，不是 `notImplemented` 占位（G18(f) / §3.10 第 5–6 条）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
  - 验收：`sample.rs` 内 `notImplemented` 只剩 `E12` 一处；分区条目零硬编码
- [ ] ~~`src-tauri/src/cmd/storage/entries.rs`：`ENTRY_COUNT = 13` 与 `static_for` 的 `E7`–`E11` 分支删除；静态档只保留 core 8 条；分区条目的 `action` / 静态 `eligibility` / `reason` / `policy` / `note` 一律从 `SectionEntry::decl` + 分区 `eligibility` / `policy` 取，core 不写功能条目 `key` / `entry` 字面量（G17(b) / V22④）；`complete` 改为「除 `E12` 外 `entries` 中全部 `availability ∈ {available, missing}`」（条目数动态、未挂载分区条目不参与）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
  - 验收：`entries.rs` 检索 `security` / `E7`–`E11` 零命中；`assemble_breakdown` 入参改为动态长度（不再 `&[EntrySample; 13]`）
- [ ] ~~`src-tauri/src/cmd/storage/inventory.rs`：挂载判定从「`ProbeRegistry::capabilities()` 含能力名」改为「首段 ∈ `OPTIONAL_SEGMENTS` 且**无**已注册分区声明该首段 → `optionalNotMounted`；有分区声明 → 按分区 `entries()` 的 `segments` / `coverage` 做减法归 `declared`」；`E7`–`E11` 的 `declaredBytes` 只来自分区条目（G16 / G18(d) / §3.11.3）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
  - 验收：零分区时 `security` 归 `optionalNotMounted`（不是 `unregistered`）；单分区时与改造前逐字段等价
- [ ] ~~`src-tauri/src/cmd/storage/reclaim.rs`：可执行动作校验从「core 硬编码 `E1`–`E8` 白名单」改为「core 8 条动作表 + 分区 `reclaim_actions()` ∪ 按 `key` 调分区 `reclaim`」；`E8` 的 `pruneToLimit` 只由分区自述，core 不出现 `security/backups` 字面量；vaultless 构建下该 `key` 不存在 → 请求按未知 key 拒绝（G17(b) / V23⑤）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
  - 验收：`reclaim.rs` 检索 `security` 零命中；`E8 + A4` 在 vault 构建放行、vaultless 构建拒绝且零副作用
- [ ] ~~`src-tauri/tests/storage_breakdown.rs`：golden 改为两种构建分别断言——`vault` 13 行、`vaultless` 8 行；`vaultless` 断言 `security` 组**整体不存在**（不得出现 `notImplemented` 占位行），core 条目逐字段与 `vault` 构建一致（V13 / V23①⑤）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
  - 验收：删掉 `unmounted_probe_keeps_e7_to_e11_visible_and_not_reclaimable`，改为「未挂载 = 行缺失」用例
  - 进度（2026-11-21，顺带完成）：本文件内重复的 `file_link` 辅助函数已删除，改为复用 `assistant-tools` 测试树里的全仓唯一实现（避免各 crate 各写一份后在 Windows 侧漂成两种建链策略）；golden 断言本身未改，仍待本条正式改写
  - 进度（2026-11-22）：该复用改为经 `assistant-tools` 的 `test-support` feature 公开（`use assistant_tools::os::test_support::link::file_link;`）。原先跨 package 的 `#[path = "../crates/assistant-tools/tests/support/link.rs"]` 虽能通过 rustc，但 rust-analyzer 不认跨源根的 `#[path]`、误报 E0583「unresolved module」
- [ ] ~~`src-tauri/tests/storage_inventory.rs`：`optionalNotMounted` 断言改为按 `OPTIONAL_SEGMENTS` 首段判定（不再依赖能力名注册）；覆盖「槽缺失 / 零分区 / 单分区 / 双分区」四态（V19 / V23）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
  - 验收：注册一个首段不在 `OPTIONAL_SEGMENTS` 的假分区 → `register` 拒绝且 breakdown 不出现该行
- [ ] ~~`src-tauri/tests/storage_reclaim.rs`：`entryKeys` 空 = core 8 条 + 已挂载分区条目（不再写 13）；白名单用例改为分区自述动作驱动；请求已退役分区的 `key` → 按未知 key 拒绝（V20 / V23）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
  - 验收：上述两组在 `vault` / `vaultless` 各跑一遍
- [ ] ~~验收：V21–V24 按新口径落地（口径四元组一致 / 分区自述与 core 事实一致 / 通用槽四态 / core 不感知具体功能），并把 `T1-c 修订` 的 V22–V24 旧断言（能力名 `SECURITY_OBSERVABILITY`、`ProbeRegistry`、`Feature(capability)` 行）在测试中删除~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**

**随本修订同步作废 / 改写的旧断言**（改写时逐条按本节新口径，不要保留双份）：

| 位置 | 旧断言 | 新口径 |
|---|---|---|
| T1-b 标题 / 遍历口径 | 「13 条共用」 | 口径本身不变，条目数不再是常量 13（`vault` 13 / `vaultless` 8） |
| T1-d 装配 | 「13 条按 `E1`→`E13` 固定顺序」、`ENTRY_COUNT`、`&[EntrySample; 13]` | core 8 条固定 + 分区条目按 `entries()` 快照顺序 |
| T1-e 测试 | 「两种构建 `entries` 恒 13 条」 | 两种构建分别断言 13 / 8，`vaultless` 无占位行 |
| T1-f 对账 | 「13 条登记」、`subtract_declared(&RawNode, &[EntrySample; ENTRY_COUNT])` | 8 条 core + 已挂载分区条目（长度动态），`optionalNotMounted` 按首段判 |
| T2-a 计划 | 「`entryKeys` 空 = 全部 13 条」 | core 8 条 + 已挂载分区条目 |
| T3-b `E8` | `StorageOwnerProbe::prune_backups`、core 侧 `E8 + A4` 白名单、vaultless 时 `E8` 为 `notImplemented` | 分区 `reclaim_actions` / `reclaim` 自述，core 无 `security/backups` 字面量；vaultless 时该行不存在 |
| T4-b vaultless 回归 | 「`entries` 恒 13 条；`E7`–`E11` 为 `notImplemented`」 | `entries` 8 条，安全域整组不出现 |
| 门禁 G10 | 「不把 13 条相加」 | 「不把 core 8 条 + 已挂载分区条目相加」 |
| 门禁表 | 只有 G1–G14 | 补 G15–G19（已落在文末「门禁自查表（G1–G19）」G15–G19 五行） |

> **阅读顺序提示**：上一节 T1-g 修订取代 T1-c / T1-c 修订。T1-d–T1-h 中与「13 条 / 可选条目 / provider」相关的表述在 T1-g 落地时按该节末尾的改写表执行；其余条目（core 8 条、对账余量、超时与并行、遗留收尾）不受影响。

### T1-d 管理器装配（`get_storage_breakdown` 扩展）

- [x] `src-tauri/src/cmd/storage/mod.rs`：`sampledAt` 只取一次（采样开始时刻，秒），**禁止**每条各取一次
  - 核对（2026-11-20）：`mod.rs:165` 取一次 `now_seconds()`，随 `assemble_breakdown(…, sampled_at, …)` 传入（`entries.rs:394`）→ 已落地
- [x] `src-tauri/src/cmd/storage/mod.rs`：`E1` 与顶层字段**同源**——一次 `list_all()` + 一次 `attachment_refs` 查询，同时填 `totalBytes` / `totalCount` / `orphans` / `conversations` / `E1` 的 `bytes` / `count`；**不得**为条目再遍历一次目录
  - 核对（2026-11-20）：`mod.rs:170` 一次 `store.list_all()`，同一 `all_entries` 既喂 `build_base`（`:180`）又喂 `sample::collect`（`:184`）→ `E1` 由 `attachment_sample(entries)`（`sample.rs:329`）派生 → 已落地
- [x] `src-tauri/src/cmd/storage/mod.rs`：会话标题 / 更新时间改为一次 `JOIN`（或复用 `conversations` 表行），消除 N16 的每会话 2 次 `query_row`
  - 核对（2026-11-20）：`load_attachment_refs`（`mod.rs:48`–`:79`）单条 SQL `attachment_refs LEFT JOIN conversations` 一次带回 `title` / `updated_at`；有单测 `conversations_group_by_id_with_title_from_join`（`:414`）→ 已落地
- [x] `src-tauri/src/cmd/storage/mod.rs`：`sharedBytes` / `referencedOnceBytes` 由 `attachment_refs` 的「附件 → 会话数」直接算，保证 `totalBytes = referencedOnceBytes + sharedBytes + orphanBytes`（V5）
  - 核对（2026-11-20）：`build_base`（`mod.rs:132`–`:146`）按 `conv_count` 分档求和；V5 恒等式由 `ledger_identity_splits_shared_once_and_orphan`（`:381`–`:411`）断言 → 已落地
- [ ] ~~`src-tauri/src/cmd/storage/sample.rs`：`E2`–`E13` 与 `E1` **并行**采样；异步 owner（`AttachmentStore` / `ToolResultStorage`）用 `tokio::time::timeout`，同步 owner（provider / `metadata` / 目录遍历）用 `spawn_blocking` + 对 `JoinHandle` 超时；单条 **500 ms** 超时 → `bytes/count = null` + `availability = unavailable` + `availabilityReason = timeout`（超时只放弃等待，**不取消**，因此 owner stats 必须只读 / 幂等 / 不持锁长遍历）~~ **（剩余缺口只剩 `E7`–`E11` 的 `spawn_blocking`，见 [剩余清单](./I61-storage-manager-remaining.md) `R-17`；`availabilityReason` 部分已随方案 C 删除）**
  - 核对（2026-11-20）：`SAMPLE_TIMEOUT = 500ms`（`sample.rs:31`）、`timed` 用 `tokio::time::timeout`（`:170`）、`timed_blocking` 用 `spawn_blocking` + 超时（`:176`–`:188`）、`join_all` 并行（`:413`）均已落地；**但 `E7`–`E11` 走 `timed_probe`（`:154`–`:164`），其 `call()` 在 async future 内联同步执行、未走 `spawn_blocking`** → 本条未完全满足，保持未勾选（缺口编号 T1-h B：T1-g 删除 `timed_probe`、分区采样统一进 `spawn_blocking` 后一并勾选）
- [x] `src-tauri/src/cmd/storage/mod.rs`：`E1` **不参与** 500 ms 超时；`list_all` 失败即整命令 `Err`（旧字段非 `Option`）；**禁止**把旧字段改成 `Option` 或加 `default` 伪装成 0
  - 核对（2026-11-20）：`mod.rs:170` 的 `list_all().await.map_err(…)?` 直接传播为命令 `Err`；DTO 顶层 `total_bytes` / `total_count` 仍非 `Option`（`entries.rs:390`–`:391`）→ 已落地
- [x] `src-tauri/src/cmd/storage/entries.rs`：13 条按 `E1`→`E13` 固定顺序装配（key 与 §3.1 逐字一致），`missing` 计 `0`、其余取不到数为 `null`（G6）；`complete` = 除 `E12` 外 12 条全部 `availability ∈ {available, missing}`〔**旧口径，T1-g 落地时改写**：core 8 条固定 + 分区条目按 `entries()` 快照顺序，`complete` 覆盖 `entries` 中全部条目〕
  - 核对（2026-11-20）：`assemble_breakdown` 按 `0..ENTRY_COUNT` 顺序装配（`entries.rs:379`）；`EntrySample::missing()` 为 `observed(0, 0, …)`（`:61`–`:63`）；`complete` 判定排除 `LOGS_INDEX`（`:381`–`:388`），有单测 `complete_ignores_only_logs_entry`（`tests/storage_breakdown.rs:174`）→ 已落地
- [x] `src-tauri/src/cmd/storage/entries.rs`：`reclaim.action` **按终态矩阵**填（§3.1 本期动作口径表）——`action` 必须保持终态动作（`E1` = `deleteOrphans`、`E2`/`E4` = `rebuildCache`、`E3` = `sweepExpired`、`E8` = `pruneToLimit`、`E13` = `none`），禁止随 `eligibility` 一起降级成 `none`；`E5`/`E6`/`E7`/`E9` = `none` + `notEligible` + `userData`/`businessData`；`E10`/`E11` = `sweepExpired` + `ownerManaged`；`E12` = `rotate` + `ownerManaged` + `externalOwner`
  - 核对（2026-11-20）：`static_for`（`entries.rs:120` 起）逐条核对：0=`DeleteOrphans`、1=`RebuildCache`、2=`SweepExpired`、3=`RebuildCache`、4/5/6/8=`None`、7=`PruneToLimit`、9/10=`SweepExpired`+`OwnerManaged`、11=`Rotate`+`OwnerManaged`+`ExternalOwner`、12=`None`；`E2`/`E4` 已随 T2 落地转为 `Eligible`（`reclaim.rs:288`–`:290` 的 `executor_action` 仅放行 1|3），`E1`/`E3`/`E8`/`E13` 仍 `NotImplemented`+`NoReclaimApi`+`note` → 已落地
- [x] `src-tauri/src/cmd/storage/entries.rs` + `sample.rs`：`policy` 按 §3.5 逐条填——`E3` = `config` + `retentionDays`（owner 注入值）+ `effectiveSince` + `mutable = true`；`E11` = `config` + `retentionDays`（`audit_policy()` 自述值）+ `effectiveSince` + `mutable = true`；`E8` = `constant` + `keepCount = 5` + `autoActive = true`（与 `security_db::MAX_BACKUPS = 5` 手工对齐，改常量须同步）；`E10` = `constant`（无上限）；`E12` = `constant` + `keepCount = 3`，10 MiB 单文件上限写 `note`、`maxBytes = null`；`E4` = `owner` + `maxBytes = null` 且 64 MiB 上限写 `note`；其余 = `none`；`failClosed` 恒 `true`，`mutable` 仅 `config` 为 `true`
  - 核对（2026-11-20）：`ReclaimPolicyDto::default` 已 `fail_closed = true` / `mutable = false` / `max_bytes = None`（`protocol/dto/storage.rs:210`–`:221`）；`E3` 由 `sample_tool_results` 的 `result_retention()` 注入 `config_policy(days, effective_since)`（`sample.rs:205`），`E11` 由 `audit_policy` 注入（`sample.rs:418`）；`E8` = `constant_policy(Some(5))`、`E12` = `constant_policy(Some(3))` + note 10 MiB、`E10` = `constant_policy(None)`、`E4` = `owner_policy()` + note 64 MiB、`E11` policy `config` + `mutable: true`（`entries.rs:214`–`:224`）→ 已落地
- [x] `src-tauri/src/cmd/storage/entries.rs`：`confirmation` 两步判定（§3.6 硬约束 5）——第一步 `eligibility ≠ eligible` 恒 `none`；第二步 `eligible` 按条目风险档定档（`E1` 高 → `strongConfirm`，`E2`/`E4`/`E3`/`E8` 低/中 → `confirm`），`expectedBytes` 不参与判定；`reclaimableBytes` / `reclaimableCount` 只对 `eligible` 给数，且必须 `≤ bytes`（V17 前半）
  - 核对（2026-11-20）：`confirmation_for`（`entries.rs:284`–`:296`）先判 `eligibility != Eligible → None`，再 `index 0 → StrongConfirm` / 其余 `Confirm`；`build_entry`（`:305`–`:309`）只在 `eligible` 时保留 `sample.reclaimable`，否则 `None` → 已落地（T2 后 `E2`/`E4` 为 `eligible`，因此不再是「T1 全部 `null`」）
- [ ] ~~`src-tauri/src/cmd/storage/entries.rs` + `sample.rs`：`E9` 锁定态 `availability = eligibility = locked`（`availabilityReason` 写「vault 未解锁」）；`E7` **不**因 vault 锁定变 `locked`（只 `metadata`）——正常为 `notEligible` + `reason = businessData`，仅当 `metadata` 失败时 `availability = unavailable` + `availabilityReason` 写「文件正在替换 / 迁移」**且** `eligibility = locked` + `reason = locked`（`availability` 与 `eligibility` 在这里**不同值**，不要「统一」）；vaultless 构建 `E7`–`E11` 的 `availability = notImplemented`（`eligibility` 仍按 §3.10 第 5 条：`E7` `notEligible`/`businessData`、`E9` `notEligible`/`userData`、`E8` `notImplemented`/`noReclaimApi`、`E10`/`E11` `ownerManaged`）~~ **（`availability` / `eligibility` 部分已落地；唯一缺口是原因文案，而方案 C 直接删除该字段——代码改动见 [剩余清单](./I61-storage-manager-remaining.md) `R-16`，原「缺口 C」作废）**
  - 核对（2026-11-20）：`eligibility_for`（`entries.rs:264`–`:280`）对 index 8（`E9`）`locked → eligibility = locked` + `reason = locked`、对 index 6（`E7`）`unavailable → locked/locked` 均已落地；`E7` 正常态 `notEligible`/`businessData`、vaultless 五条 `notImplemented` 且 `eligibility` 取 `static_for` 静态值均符合；**唯一偏差：`ProbeError::Locked` 的 `availabilityReason` 实为「尚未解锁」（`sample.rs:147`），规格要求「vault 未解锁」** → 本条保持未勾选（缺口编号 T1-h C：T1-g 改写分区错误映射时一并改文案）
- [x] `src-tauri/src/cmd/storage/entries.rs`：仍为 `notImplemented` 的条目 `note` **逐字点名缺的方法**（T2/T3 补齐后对应 `note` 置 `null`、`eligibility` 转 `eligible`）：`E1` → `AttachmentStore::delete_orphans_by_ids`；`E3` → `ToolResultStorage::collect_expired` / `delete_candidates`；`E8` → `SecurityDbState::prune_backups_to`；`E13` → `assistant_crash::prune_to_limit`（**唯一例外**：`E13` 全程保持 `notImplemented` / `noReclaimApi` / `action = none`，OQ8 未定案，T2–T4 都**不**转 `eligible`、`note` 也**不**置 `null`）
  - 核对（2026-11-20）：`static_for` 中 index 0 / 2 / 7 / 12 的 `note` 分别逐字含上述方法名（`entries.rs:132` / `:149`–`:151` / `:197` / `:246`），且 `E13` 为 `action = None` + `NotImplemented` + `NoReclaimApi`；`E2`（index 1）/`E4`（index 3）已随 T2 转 `Eligible`、`note = None`（`E4` 仅留 64 MiB 说明），符合「补齐后置 null」→ 已落地。注：`tests/storage_breakdown.rs:91` 的 V4 只断言 `note` 非空，仓库内无 golden 快照锁死字面量（与 E12 条同一缺口）
- [x] `src-tauri/src/tool_setup.rs`：**fail-closed 修正**——读配置失败回退 `0`（禁用），不再回退常量 30 天（N6 / V12 前半）
  - 核对（2026-11-20）：`load_result_retention`（`tool_setup.rs:39`–`:51`）`Err` 分支 `log::warn!` 后取 `0`，注释明示「绝不回退常量天数」→ 已落地
- [x] `src-tauri/src/tool_setup.rs`：把 `result_retention_days` 与注入时刻注入 `FileToolResultStorage`（`with_retention(days, since)`）——**`AppConf::load()` 只读一次**，同一个 `retention_days` 同时喂给 `spawn_result_cleanup` 与 `with_retention`（两次各读会得到不同的 `effective_since`，`E3` 的 `policy` 与启动清扫就会对不上）；管理器只读 owner 自述、**不**读配置
  - 核对（2026-11-20）：`retention` 只由一次 `load_result_retention()` 取得（`tool_setup.rs:289`），同一 `ResultRetention` 既喂 `spawn_result_cleanup`（`:290`）又喂 `with_retention(retention.days, retention.effective_since)`（`:293`）；管理器侧无 `AppConf::load()` → 已落地
- [x] `src-tauri/src/adapters/tool_exec.rs`：新增 `pub fn storage(&self) -> &Arc<dyn ToolResultStorage>` 转发 `self.pipeline.storage()`（`ToolPipeline::storage()` 已存在，`PipelineToolExecution` 原先**没有**该访问器，不加就无法从 `app.state` 拿到 E3 的 owner）
  - 核对（2026-11-20）：`tool_exec.rs:237`–`:239` 已新增 `pub fn storage()` 直接转发 `self.pipeline.storage()` → 已落地
- [x] `src-tauri/src/cmd/storage/`：取 `E3` / `E4` 的 owner 入口——`E3` 经 `app.try_state::<Arc<PipelineToolExecution>>()` → `.storage()`；取不到 → `E3` `availability = notImplemented` + `availabilityReason` 点名 owner 缺失 + `bytes` / `count = null` + `eligibility = notImplemented` + `reason = noReclaimApi`（V17：方法不可达即不可执行）。`E4` 用 `AppDirs::indexes()` 传给公开的 `index_stats`；`AppDirs::resolve()` 失败 → 整命令 `Err`（与 `E1` 同级）。管理器**不得**自行拼受管目录路径（G1）
  - 核对（2026-11-20）：`sample.rs:299`–`:301` 取 `try_state::<Arc<PipelineToolExecution>>()` → `.storage()`，`None` 分支 `:353`–`:354` 产 `NotImplemented` + 文案「工具执行管线未装配」（语义等同「owner 未装配」，规格 §3.10 只要求点名缺失者，未锁字面量）；`eligibility`/`reason` 由 `static_for` index 2 给 `notImplemented`/`noReclaimApi`；`E4` 走 `dirs.indexes()`（`:359`–`:361`）→ `index_stats`；`AppDirs::resolve()` 在 `mod.rs:165` 整命令 `Err` → 已落地
- [x] `src-tauri/src/cmd/storage/entries.rs`：把「13 条装配」抽成**纯函数** `pub fn assemble_breakdown(…) -> StorageBreakdownDto`（入参类型与字段同为 `pub`，**不能** `pub(crate)`——`tests/storage_breakdown.rs` 是 lib 的集成测试，跨 crate 只能看见 `pub`）——入参只含普通数据（`sampled_at`、附件采样结果、各条样本、`policy` 快照）；命令函数只负责取 state / 调 owner / 喂入参。**没有这一步，`tests/storage_breakdown.rs` 的 V1–V5 / V13 无法落地**〔T1-g 落地后 `&[EntrySample; ENTRY_COUNT]` 改为动态长度 `&[EntrySample]`〕
  - 核对（2026-11-20）：实现为 `pub fn assemble_breakdown(base: BreakdownBase, samples: &[EntrySample; ENTRY_COUNT], sampled_at: i64, inventory: StorageInventoryDto)`（`entries.rs:373`–`:378`）——**未引入 `BreakdownInputs` 结构，改为 `pub BreakdownBase` + 三个显式入参**（同样只含普通数据、同样是纯函数，测试可完全伪造）；`BreakdownBase` 为 `pub`（`:87`），`tests/storage_breakdown.rs:8`–`:11` 已跨 crate 导入并驱动 V1–V5 / V13 → 目标已达成

### T1-e 测试

- [x] **可见性（测试的前置条件，先做）**：`src-tauri/src/lib.rs` 的 `mod cmd;` 改为 `pub mod cmd;`，`src-tauri/src/cmd/mod.rs` 的 `mod storage;` 改为 `pub mod storage;`——现有 `src-tauri/tests/*` 全部只 `use` 已 `pub` 的模块，`assistant_app::cmd::storage::…` 不改则取不到，`storage_breakdown.rs` / `storage_reclaim.rs` 无法编译。`a3_lib_rs_is_assembly_only` 只校验 `lib.rs` 行数与「不得内联 `generate_handler!`」，`pub` 化不影响；也**不要**为测试新建 `pub mod storage_manager` 门面
  - 核对（2026-11-20）：`lib.rs:8` 为 `pub mod cmd;`、`cmd/mod.rs:18` 为 `pub mod storage;`；`tests/storage_breakdown.rs:8`–`:11` 与 `tests/storage_inventory.rs` 均已跨 crate 导入编译通过 → 已落地

- [x] `src-tauri/crates/assistant-storage/tests/attachment_store.rs`：`derived_stats` 用例（空目录 → `count = 0` 且两端 `None`；符号链接不计入；`bytes` = 常规文件长度之和）
  - 核对（2026-11-20）：`derived_stats_counts_regular_files_and_skips_companions`（`attachment_store.rs:108`–`:133`）已覆盖空目录空态、`.tmp` 伴生排除、`bytes = 7` 求和；**无符号链接用例**（全文件无 `symlink` 字样）→ 缺一项，保持未勾选（缺口编号 T1-h D-1）
  - 核对（2026-11-21）：补 `derived_stats_skips_symlinks`（指向目录外的文件链接 → `count = 0` / `bytes = 0`）；建链失败时显式 `eprintln` 跳过，本机无 skip 输出（真实建链）。`cargo test -p assistant-storage --test attachment_store` 11 项绿 → 已落地（T1-h D-1 关闭）
- [x] `src-tauri/crates/assistant-tools/tests/storage.rs`：`result_stats` 与 `cleanup_expired_results` 判据一致（同目录先统计再清理，`removedBytes` 与统计值可对上）；`retention_days = 0` 时 stats 仍可出数但 `E3` 判 `policyDisabled`
  - 核对（2026-11-20）：`result_stats_shares_the_cleanup_scope`（`:559`–`:576`）先统计后清理、前后 `count` / `bytes` 对得上，`result_stats_ignores_companion_files`（`:581`）另覆盖伴生文件；`retention_is_injected_once_and_survives_disabled_policy`（`:592`–`:611`）覆盖 `retention = 0` 时 stats 仍出数。**但「`E3` 判 `policyDisabled`」无任何实现或用例**（全仓 `PolicyDisabled` 仅出现在 `protocol/dto/storage.rs:155` / `:192` 的枚举定义）→ 本条保持未勾选（缺口编号 T1-h D-2）
  - 核对（2026-11-21）：`E3` 判据已在 T3 落地——`tool_results_eligibility`（`entries.rs:341`–`:367`）在 `retention_days` 读不到或 `= 0` 时给 `notEligible` + `policyDisabled`；用例 `t3_dynamic_eligibility_follows_policy_and_count`（`entries.rs:678`）、`retention_zero_is_policy_disabled_and_deletes_nothing`（`storage_reclaim.rs:682`）与 `retention_is_injected_once_and_survives_disabled_policy`（`storage.rs:592`）三条均绿 → 已落地（T1-h D-2 关闭，无需再改代码）
- [x] `src-tauri/crates/assistant-tools/tests/index_stats.rs`：`index_stats` 覆盖跨 revision 残留与符号链接跳过
  - 核对（2026-11-20）：`cross_revision_residuals_are_all_counted`（`:19`–`:33`）、`companion_files_are_not_counted`（`:35`）、`symlinks_are_skipped`（`:50`，`#[cfg(unix)]`）齐备 → 已落地（Windows 上符号链接用例按 `cfg` 不编译）
- [x] `src-tauri/src/cmd/mascot_asset.rs`（`mod tests`）：`asset_stats` 排除 `.tmp`、跳过子目录符号链接
  - 核对（2026-11-20）：`asset_stats_counts_regular_files_and_skips_companions`（`mascot_asset.rs:362`）已断言 `.tmp` 不计入；**无符号链接用例** → 缺一项，保持未勾选（缺口编号 T1-h D-3）
  - 核对（2026-11-21）：补 `asset_stats_skips_symlinks`（`rive/` 下指向目录外的文件链接 → `count = 0` / `bytes = 0`）；建链失败时显式跳过，本机无 skip 输出。`cargo test --lib asset_stats` 3 项绿 → 已落地（T1-h D-3 关闭）
- [x] `src-tauri/src/setup.rs`：新增 `#[cfg(test)] mod tests`，用例覆盖 `db_file_stats`——文件缺失按空态（由管理器翻译成 `missing` 计 0），且**不打开 / 不创建 DB**
  - 核对（2026-11-20）：`setup.rs:344` 起 `mod tests`，`missing_db_file_reports_zero_and_creates_nothing`（`:355`）断言缺失时 `bytes/count = 0`、两端 `None`、`!path.exists()`；`existing_db_file_is_measured_by_metadata_only`（`:369`）→ 已落地
- [x] `src-tauri/tests/storage_breakdown.rs`（集成测试）：13 条 key 顺序（V1）；伪造 owner 失败 → `null` + `unreadable`（V2）；`action = none` 的条目 `eligibility ∈ {notEligible, ownerManaged, notImplemented, locked}`（V3；**`locked` 必列**）；`reason` / `note` 非空（V4）；账本恒等式（V5）〔V1 的条数随 T1-g 改写为 `vault` 13 / `vaultless` 8〕
  - 核对（2026-11-20）：V1 `entries_follow_registry_order`（`:31`）、V2 `owner_failure_degrades_to_null_and_unreadable`（`:49`）、V3 `action_none_entries_are_never_eligible`（`:69`，含 `Locked`）、V4 `non_eligible_entries_carry_reason_and_not_implemented_carries_note`（`:91`）均在本文件；**V5 恒等式落在 `cmd/storage/mod.rs:381` 的单元测试 `ledger_identity_splits_shared_once_and_orphan`（不在本文件）** → 覆盖齐备，已落地
- [x] `src-tauri/tests/storage_breakdown.rs`：注入假 provider 与**不注入**各跑一遍——不注入时 `E7`–`E11` 的 `availability = notImplemented`、`eligibility` 分别为 `notEligible`/`notImplemented`/`notEligible`/`ownerManaged`/`ownerManaged`（V13 后半）；两种构建 `entries` 恒 13 条、顺序不变〔**旧断言，T1-g 落地时改写**：`vault` 13 / `vaultless` 8，未挂载 = 行不存在〕
  - 核对（2026-11-20）：`unmounted_probe_keeps_e7_to_e11_visible_and_not_reclaimable`（`:120`–`:170`）用 `unmounted_probe_sample()` 驱动「未装配」一侧、`available()` 驱动正常一侧，断言 13 条恒在、`bytes/count = None`、`eligibility` 五元组与规格一致。偏差：测试在**样本层**注入（`assemble_breakdown` 不吃 provider），未构造假 `StorageOwnerProbe` trait 对象；行为契约等价 → 已落地（该用例本身随 T1-g 删除，见 T1-g「`storage_breakdown` golden」条）
- [x] 静态门禁：`cmd/storage/` 无 `fs::remove_*` / `read_dir`（唯一例外 `inventory.rs`）/ 受管目录名字面量 / `AppConf::load()`（V10 / G1–G3）
  - 核对（2026-11-20）：全目录检索 `remove_file` / `remove_dir` / `fs::remove` 无命中；`read_dir` 仅 `inventory.rs:222` / `:259` / `:287`；`AppConf::load` 无命中 → 已落地

### T1-f 对账余量（§3.11，只读旁路，T1 内**独立成链**）

> 与 T1-b/c/d **无依赖**（不读 owner 数字、不进 `entries`），但**必须**在 T1 内完成：`inventory` 是 `StorageBreakdownDto` 的必填字段（键恒在），晚做会让 T2–T4 的 golden 全部返工。

- [x] 新建 `src-tauri/src/app_dirs/registry.rs`（登记表本体）；`app_dirs/mod.rs` 加 `pub mod registry;`；`app_dirs/dir_name.rs` 的模块声明收紧为 `pub(crate)`
  - 验收：`cargo test -p assistant-app` 的 registry 单测绿
- [x] `src-tauri/src/app_dirs/dir_name.rs`：加常量清单 `ALL`
  - 验收：`ALL` 覆盖全部常量，缺项负例判红
- [x] `src-tauri/src/app_dirs/registry.rs`：加自检函数 `validate_registry(roots, dir_names)`
  - 验收：缺登记 / 缺常量 / 重复 key 三个负例各自判红
- [x] `src-tauri/src/app_dirs/roots.rs`：7 个 getter 改为经登记表派生，拼接仍留在本文件
  - 验收：`maps_every_managed_entry` / `diagnostics_live_under_local_root` / `resolve_is_idempotent` 绿
- [x] `src-tauri/src/app_dirs/registry.rs`：加 `#[cfg(test)] mod tests`（必须在 crate 内，`dir_name` 收紧后 `tests/` 取不到）
  - 验收：该模块五项用例绿
- [x] `src-tauri/src/protocol/dto/storage.rs`：加 5 个 inventory DTO；`StorageBreakdownDto` 追加 `inventory`（`#[serde(default)]`）
  - 验收：`cargo test -p assistant-app --test protocol_bindings` 绿；`npm run contract:gen` 产出同名类型
- [x] 新建 `src-tauri/src/cmd/storage/inventory.rs`（文件头注明「G2 唯一例外」）：扫描入口 + 五类判定 + 遍历 + 有界，全程只读
  - 验收：守恒 / 有界 / `failed` 保留用例绿；文件内检索 `remove_*` / `fs::write` / `create(` / `log::` 为空
- [x] `src-tauri/src/cmd/storage/inventory.rs`：加减法与余量纯函数（登记值只取已登记条目）
  - 验收：减法 / 余量 / `null` 传染 / `overRegistered` 五例绿
- [x] `src-tauri/src/cmd/storage/inventory.rs`：加快照状态机 `InventoryCache`（进程内不落盘）+ `trigger_scan`
  - 验收：TTL 与失败态 / single-flight 与读不阻塞 / 过期快照回旧数字且只触发一次，三例绿
- [x] `src-tauri/src/setup.rs`：`handle.manage(InventoryCache::default())`（命令用 `State` 取，不用全局静态）
  - 验收：集成测试能构造可重置的独立实例
- [x] `src-tauri/src/protocol/events.rs`：加 `STORAGE_INVENTORY_UPDATED` 常量，广播复用 `emit_broadcast`
  - 验收：扫描完成后广播该事件名
- [x] `src-tauri/src/cmd/storage/mod.rs`：`get_storage_breakdown` 只读快照、读路径不 `await` 扫描；装配入参带 `inventory`
  - 验收：`running` / `failed` 两态下 `entries` 与 `totalBytes` 逐字段不变
- [x] `src-tauri/src/cmd/storage/mod.rs`：加 `refresh_storage_inventory` 命令（只回状态、不回数字）
  - 验收：返回类型不含任何数字字段
- [x] `src-tauri/src/ipc.rs`：注册 `cmd::refresh_storage_inventory`（核心注册表、无 `#[cfg]`、不动 permission / `capabilities/*.json`）
  - 验收：`cargo check` 绿，permission 与 capabilities 文件零改动
- [x] 新建 `src-tauri/tests/storage_inventory.rs`：V18 / V20 前半 / 有界 / `failed` 保留（V20 后半的白名单负例留给 T2-d）
  - 验收：该测试文件全绿
- [x] 静态门禁：`read_dir` 只允许出现在 `cmd/storage/inventory.rs`；`inventory.rs` 内不得出现 `remove_*` / `fs::write` / `create(` / `log::`
  - 验收：按上述两条检索 `cmd/storage/` 判绿

> 核对（2026-11-20）：16 条全部落地——`registry.rs` 三枚举 + 13 条登记（`:52`–`:184`；其中 `E7`–`E11` 五行随 T1-g 修订移除，届时只剩 8 条 core 行 + `OPTIONAL_SEGMENTS`）、`validate_registry`（`:207`）、`dir_name::ALL`（`:19`，实际带 `#[cfg(test)]`）、`roots.rs` 七个 getter 全转 `path_for`（`:100`）；DTO 五类型（`storage.rs:257`–`:326`，`Default` 在 `:313`）+ `inventory` 字段（`:48`）；`inventory.rs` 的 `scan`（`:327`）、`subtract_declared`（`:393`）、`residual`（`:419`）、`InventoryCache`（`:496`）、`status_of`（`:463`）、`trigger_scan`（`:581`）；`setup.rs:116` 已 `manage`；`events` 常量在 `protocol/events.rs:27`；`mod.rs` 读路径在 `:175`–`:186`、`refresh_storage_inventory` 在 `:188`；`ipc.rs:98` 已注册；`tests/storage_inventory.rs` 8 例。偏差三处（均已按实际形状记录，行为契约等价）：①扫描入口收 `(dirs, provider_mounted: bool)`、时刻由调用方在完成后打点，非条目原写的 `scan_inventory(dirs, &dyn Fn, now)`（T1-g 落地时 `provider_mounted` 改为「已注册分区声明的首段集合」，判定口径见 T1-g `inventory.rs` 条）；②减法为 `subtract_declared(&RawNode, &[EntrySample; ENTRY_COUNT])` 三函数组合，非 `subtract(&mut [DTO], &HashMap)`（T1-g 落地时第二参数改为动态长度）；③`trigger_scan` 只收 `app`（缓存从 `try_state` 取），无 `cache` / `force` 入参。

> 验收记录（2026-11-20）：`cargo test -p assistant-app --test storage_breakdown --test storage_inventory` 14 项全绿（`storage_breakdown` 6 项覆盖 V1–V5 / V13，`storage_inventory` 8 项覆盖 V18 / V20 前半 / 有界 / `failed` 保留）；`cargo check --workspace --all-targets` 通过；`--no-default-features`（vaultless）编译通过；`cargo clippy` 无告警。门禁签字：G1（`cmd/storage` 无 `use assistant_vault` / `assistant_security`、无 `#[cfg(feature)]`）、G2（`read_dir` 仅 `cmd/storage/inventory.rs`）、G3（无 `remove_*` / `fs::write` / `create(` / `log::` / 受管目录字面量）、G5（符号链接一律 `symlink_metadata`、不跟随）、G6（`missing` 计 0、其余取不到数为 `null`）、G13 / G14（既有 4 字段键名 / 类型 / 含义与既有能力零改动）逐条通过。伴生文件口径（`-journal` / `-wal` / `-shm` / `.tmp` / `.lock`）在 owner 观测与 `inventory.rs` 分类两侧一致，无漂移。

### T1-h 遗留收尾（原 7 项，已清 4 项；剩 B → 剩余清单 `R-17`，A-2 / C 合并为 `R-16`）

> 原 7 项在各自条目下都带「未满足」核对注（不是「已落地但没勾」）。A-1 / D-1 / D-2 / D-3 已于 2026-11-21 清零（D-2 由 T3 顺带落地，只做复核）；剩余项都已迁到 [剩余清单](./I61-storage-manager-remaining.md)，本表只留编号对照。

| 编号 | 位置 | 缺口 | 去向 |
|---|---|---|---|
| A-1 | T1-b `assistant-crash` | 已落地（2026-11-21） | 无（下方详列） |
| A-2 | T1-b `E12` | 固定文案无 golden 锁死字面量 | 方案 C 删除 `availabilityReason` → 合并到 `R-16`（作废） |
| B | T1-d `sample.rs` | `E7`–`E11` 走 `timed_probe`，`call()` 在 async future 内联同步执行，未 `spawn_blocking` | `R-17` |
| C | T1-d `entries.rs` | `ProbeError::Locked` 的 `availabilityReason` 文案与规格不符 | 方案 C 删除该字段 → 合并到 `R-16`（作废） |
| D-1 | T1-e `attachment_store.rs` | 已落地（2026-11-21） | 无 |
| D-2 | T1-e `assistant-tools/tests/storage.rs` | 已落地（2026-11-21，随 T3 一起实现，本条仅需复核） | 无 |
| D-3 | T1-e `mascot_asset.rs` | 已落地（2026-11-21） | 无 |

A-2 / B / C 的验收已迁到剩余清单，不在此重复；下面只展开 A 组剩余一项。

- [x] `src-tauri/crates/assistant-crash/src/crash_dump.rs`：`dump_stats` / `DirStats` 已在（`crash_dump.rs:26` / `:17`，`lib.rs` 已再导出），本条只剩两件——①`walk_stats`（`:30`）加三类文件名判据（`panic_*.log` / `crash_*.dmp` / `crash_*.log`），其余文件不计数、不降级；②补 `dump_stats` 单测（三类各命中一份 + 一份无关文件不计 + 符号链接不计）
  - 验收：`cargo test -p assistant-crash` 绿；`src-tauri/tests` 补一条 `E13` 观测验收（伪造目录 → `bytes` / `count` 与三类文件严格对上）
  - 核对（2026-11-21）：`walk_stats` 加 `is_crash_artifact()` 判据（前缀 + 后缀双匹配），递归目录内同样过滤；单测 3 条（三类命中 / 符号链接跳过 / 缺失目录空态）；`tests/storage_breakdown.rs` 补 `e13_dump_stats_counts_only_crash_artifacts` 与 `e13_dump_stats_skips_symlinks`。`cargo test -p assistant-crash` 7 项绿、`cargo test --test storage_breakdown` 全绿，符号链接用例在本机真实建链（无 skip 输出）→ 已落地
- [ ] ~~`E12` 固定文案锁死：golden 断言改为「按 `key` 查 `E12` 的 `availability = notImplemented` + `availabilityReason` 与 §3.10 表末行逐字一致」，不再依赖外部快照文件（随 T1-g 的 `storage_breakdown` golden 一起落）~~ **（已作废：方案 C 删除 `availabilityReason`，改为断言 `availability = notImplemented` + `reclaim.note` 非空，见 [剩余清单](./I61-storage-manager-remaining.md) `R-16`）**
  - 验收：把文案临时改成任意别的措辞，该断言判红
  - 注：本项与 T1-g「golden 改为 `vault` 13 行 / `vaultless` 8 行」是同一次改动，按 T1-g 的用例落地即可

---

## 阶段 T2 — plan / execute 骨架 + 低风险回收（`E2` / `E4`）（已完成 2026-11-20）

退出条件：V6–V10 通过；单条失败隔离；白名单外请求被拒。

### T2-a 骨架

- [x] `src-tauri/src/cmd/storage/reclaim.rs`：`plan` / `execute` 抽成不依赖 `AppHandle` 的 `pub` 异步纯函数（`ReclaimPorts` 只装普通句柄 + `now`）；命令体只取 `app.state` 再调用
  - 验收：`tests/storage_reclaim.rs` 能在无 `AppHandle` 下编译并驱动
- [x] `src-tauri/src/cmd/mod.rs`：抽 `pub(crate) fn lock_conn(&Mutex<Connection>)`，`get_db_conn` 改为转调（`error.db_lock` 文案唯一）
  - 验收：全仓 `error.db_lock` 只出现一处
- [x] `src-tauri/src/protocol/dto/storage.rs`：加 8 个 reclaim DTO，输入侧加 `deny_unknown_fields`
  - 验收：`cargo test -p assistant-app --test protocol_bindings` 绿；`npm run contract:gen` 产出同名类型
- [x] `src-tauri/src/cmd/storage/reclaim.rs`：加计划缓存 `ReclaimPlanStore`（进程内 `Mutex<HashMap<planId, CachedPlan>>`，不落盘），`CachedPlan` 含 `sampledAt` / 条目集合 / `policy` 快照 / `createdAt`
  - 验收：执行前能读到计划期的 `policy` 快照
- [x] `src-tauri/src/cmd/storage/reclaim.rs`：`planId` 不透明、10 分钟有效期；过期 → `planExpired`、不存在 → `planNotFound`，零副作用
  - 验收：`execute_rejects_unknown_and_expired_plans` 绿
- [x] `src-tauri/src/cmd/storage/reclaim.rs`：单实例串行，第二个执行回 `busy`（不排队、不用全局锁）
  - 验收：并发两次执行，第二次零副作用（用例在 T2-d）
- [x] `src-tauri/src/cmd/storage/mod.rs`：`get_storage_reclaim_plan` 自行重新采样；`entryKeys` 空 = 全部条目（T1-g 落地后为 core 8 条 + 已挂载分区条目，不再是常量 13），未知 key 整体拒绝；`totalReclaimableBytes` 只累加 `eligible` 条目，任一算不出则 `null`
  - 验收：空 `entryKeys` 回全部条目（`vault` 13 / `vaultless` 8）；未知 / 重复 key 整体拒绝
- [x] `src-tauri/src/cmd/storage/mod.rs`：`reclaim_storage` 整批校验 `confirmed` / 非空 / 计划命中 / 白名单，任一不符拒绝整个请求、零副作用
  - 验收：`execute_rejects_keys_outside_whitelist` 绿（T2 只放行 `E2+A5` / `E4+A5`）
- [x] `src-tauri/src/cmd/storage/reclaim.rs`：执行时重新校验资格（重算 / 重 stat / 重计数），不按计划里的数量删；计划后被改动的项计 `skipped`
  - 验收：二次执行全 `skipped`，计划里的 `expectedCount` / `expectedBytes` 在执行路径零引用
- [x] `src-tauri/src/cmd/storage/reclaim.rs`：结果组装——`removedCount` / `removedBytes` 只记实际删除，`failed` 与 `skipped` 分开，`errors` 最多 5 条且文案不含路径与文件名，单条失败不中断其余条目
  - 验收：`MAX_ERRORS` 生效；失败文案检索无路径拼接
- [x] `src-tauri/src/cmd/storage/reclaim.rs`：每条目一条结构化 `log::info!`，只记 planId / key / action / 三计数，不写路径与文件名
  - 验收：日志行内检索无路径拼接
- [x] `src-tauri/src/ipc.rs`：按现有顺序追加 `cmd::get_storage_reclaim_plan` / `cmd::reclaim_storage`（核心注册表、无 `#[cfg]`、不新建 permission、不改 `capabilities/*.json`）
  - 验收：`cargo check` 绿，permission 与 capabilities 文件零改动
- [x] `src-tauri/tests/protocol_bindings.rs`：全部新 DTO 登记进 `use` 与 `decl(&cfg)`
  - 验收：该测试绿

> 核对（2026-11-20）：12 条全部落地——`plan_storage_reclaim`（`reclaim.rs:162`）/ `execute_storage_reclaim`（`:218`）、`ReclaimPlanStore`（`:99` 起，`begin_execution` 在 `:127`，`ExecutionGuard` 在 `:154`）、`plan_age`（`:144`）、`validated_targets`（`:295`）、`execute_derived`（`:338`）/ `execute_index`（`:375`）、`MAX_ERRORS`（`:31`）与 `failure_errors`（`:430`）、`log::info!`（`:252`）、`ipc.rs:99`–`:100` 已注册、`protocol_bindings.rs:39`–`:43` 与 `:254`–`:265` 两侧齐全。四处实现与清单原写的形状不同（均按实际可装配性落地、行为契约等价）：①两个纯函数回 `Result<_, String>`（规格 §6.1 第 5 条要求沿用现有 Tauri 错误串，`StorageReclaimErrorDto` 只用于条目内 `errors`）；②`ReclaimPorts` 用 `attachments: &AttachmentStore`（`E1` / `E2` 恒装配）、`probe: Option<StorageOwnerProbeHandle>`、`db: Option<DbState>`、`tool_results: Option<Arc<dyn ToolResultStorage>>`，均为可克隆 / 可借用形态（`probe` 字段随 T1-g 修订改为分区句柄，回收按 `key` 分派到分区 `reclaim`）；③`totalReclaimableBytes` 未额外绑定 `complete`（以 §6.2 表「任一 `eligible` 算不出即 `null`」为准）；④`CachedPlanEntry` 本次补上 `policy` 快照，`ReclaimPolicyDto` 无 `Eq` 故相关 derive 降为 `PartialEq`。`busy` 并发用例仍待 T2-d 补齐。

### T2-b `E2`（`attachments/derived`，`A5`）

- [x] `src-tauri/crates/assistant-storage/src/attachment_store.rs`：加 `pub async fn list_derived(&self) -> Result<Vec<DerivedEntry>, AttachmentError>`，孤儿判定留在 owner 内（`derived/` 文件名反查基附件 `stem`），`DERIVED_DIR` / `EXTRACTOR_VERSION` / 拼接规则不外泄
  - 验收：`derived_stats` 与 `list_derived` 同口径（伴生文件 / 符号链接 / 非常规文件不计）
- [x] `src-tauri/crates/assistant-storage/src/attachment_store.rs`：加 `pub async fn purge_derived_orphans(&self, names: &[String], sampled_at: i64) -> DerivedPurgeReport`，执行侧逐个重校验（单段文件名 / 伴生后缀 / 存在性 / 常规文件 / `mtime < sampled_at`），顺带清 `*.tmp` 伴生残留
  - 验收：重复执行第二次全 `skipped`（用例在 T2-d）
- [x] `src-tauri/src/cmd/storage/entries.rs`：`E2` 的资格恒 `eligible`、`reason` 固定 `derivedCache`、`reclaimableCount` / `Bytes` 取孤儿集合（可为 0）
  - 验收：`E2` 在无孤儿时仍 `eligible`，`reclaimableBytes ≤ bytes`
- [x] `src-tauri/src/cmd/storage/sample.rs`：`E2` 采样走 `list_derived`，执行时重跑 `list_derived()` 取当前孤儿名单，不信任计划名单
  - 验收：`derived_sample` 的 `reclaimable` 与孤儿集合一致
- [x] `src-tauri/src/cmd/storage/reclaim.rs`：`E2` 执行走 owner 方法，管理器不 `fs::remove_file`
  - 验收：`cmd/storage/` 内检索 `remove_*` / `fs::write` 为空

> 核对（2026-11-20）：5 条全部落地——`list_derived` 在 `src-tauri/crates/assistant-storage/src/attachment_store.rs:310`（`read_dir` 后 `symlink_metadata` 过滤符号链接 / 非常规 / 伴生，`:334` 判 `orphan`）、`purge_derived_orphans`（同文件 `:350`，逐项 `is_plain_file_name` / `is_companion_file` / `NotFound` / `mtime >= sampled_at` → `skipped`，`clean_derived_companions` 在 `:408`）；`src-tauri/src/cmd/storage/entries.rs:137` 的 `E2` 静态档为 `Eligible` + `DerivedCache`、`policy` 为 `default()`；采样在 `src-tauri/src/cmd/storage/sample.rs:337` 走 `store.list_derived()`，`derived_sample`（`:225`）把孤儿集合折成 `reclaimable`；执行侧 `src-tauri/src/cmd/storage/reclaim.rs:338` 的 `execute_derived` 重跑 `list_derived` 后调 owner 方法。偏差一处：`E2` 的 `bytes` 取 `list_derived` 汇总（等价 `derived_stats`，两者都排除伴生 / 符号链接 / 非常规文件），非另跑 `derived_stats`。`purge_derived_orphans` 幂等用例仍待 T2-d 补齐。

### T2-c `E4`（`tool-index`，`A5`）

- [x] `src-tauri/crates/assistant-tools/src/os/tools/super_index/mod.rs`：加 `pub fn purge_index_files(index_root: &Path) -> IndexPurgeReport`，只删 `*.idx` 常规文件、跳符号链接与非常规文件、不提供「按 key 删」、内存 LRU 不参与
  - 验收：`purge_index_files` 幂等，只删缓存文件（用例在 T2-d）
- [x] `src-tauri/crates/assistant-tools/src/os/tools/super_index/mod.rs`：`index_stats` 与 `purge_index_files` 共用同一遍历实现（`runtime/storage.rs` 的 `for_each_file`）
  - 验收：`tests/index_stats.rs` 的伴生文件 / 符号链接 / 跨 revision 残留用例绿
- [x] `src-tauri/src/cmd/storage/sample.rs`：`E4` 的可回收量取索引文件全集（owner 判定可重建，管理器不自判）
  - 验收：`sample_index` 的 `reclaimable` 等于 `bytes` / `count`
- [x] `src-tauri/src/cmd/storage/entries.rs`：`E4` 资格恒 `eligible`、`reason = derivedCache`、`policy.source = owner`、`confirmation = confirm`
  - 验收：`E4` 在无索引文件时仍 `eligible`，`reclaimableCount` 可为 0

> 核对（2026-11-20）：4 条全部落地——`purge_index_files` 在 `src-tauri/crates/assistant-tools/src/os/tools/super_index/mod.rs:42`，`index_stats`（同文件 `:29`）与它同走 `dir_tree_stats` / `purge_tree_files`（`src-tauri/crates/assistant-tools/src/runtime/storage.rs` 的 `for_each_file`，`:403` 起）；`IndexPurgeReport = PurgeReport`（`mod.rs:25`）。`entries.rs` 的 index 3 档为 `Eligible` + `DerivedCache` + `owner_policy()` + `note` 写「单文件 64 MiB、内存 LRU 64 MiB 由 owner 自述」，`confirmation_for` 对该档给 `Confirm`。`sample.rs:212` 的 `sample_index` 把 `bytes` / `count` 直接作为 `reclaimable`。偏差两处：①遍历为递归（`dir_tree_stats`），非条目原写的「单层」——索引根实际只有一层文件，递归结果等价；②单文件 64 MiB 上限（`format.rs:35` `MAX_INDEX_FILE_BYTES`）只进 `note`，未填 `maxBytes`，与规格一致。`purge_index_files` 幂等用例仍待 T2-d 补齐。

### T2-d 测试

- [x] `src-tauri/tests/storage_reclaim.rs`（新建）：V6——`get_storage_reclaim_plan` 前后目录字节与 mtime 不变
  - 验收：计划前后 `signature()` 逐项相等
- [x] `src-tauri/tests/storage_reclaim.rs`：V8 四组负例——`confirmed = false` / `planId` 过期 / `entries` 为空 / `action` 与计划不符，全部拒绝且零副作用
  - 验收：四组各断言对应错误串
- [x] `src-tauri/tests/storage_reclaim.rs`：白名单负例——`E3+A3`（T2 未接入）与 `E10+A3` / `E11+A3` / `E13+A4` 请求被拒
  - 验收：上述四组 key+action 各自回 `entryNotAllowed`
- [x] `src-tauri/tests/storage_reclaim.rs`：V20 后半——请求 `key = <某个 unregistered 节点名>`（以及 `"inventory"` 本身）被拒，零副作用
  - 验收：两种 key 在 plan 与 execute 两侧均被拒，且目录签名不变
- [x] `src-tauri/tests/storage_reclaim.rs`：V9——注入 owner 失败，其余条目仍完成，`status` 如实为 `done/partial/failed/skipped`
  - 验收：同批两条一 `done` 一 `failed`，二次执行回 `skipped`
- [x] `src-tauri/tests/storage_reclaim.rs`：`busy`——并发两次执行，第二次零副作用
  - 验收：第二次回 `busy`，目录签名不变
- [x] `src-tauri/crates/assistant-storage/tests/attachment_store.rs`：`purge_derived_orphans` 幂等（重复执行第二次全 `skipped`，不报错）
  - 验收：第二次 `removed = 0`、`failed = 0`
- [x] `src-tauri/crates/assistant-tools/tests/index_stats.rs`：`purge_index_files` 幂等 + 只删缓存文件
  - 验收：第二次 `removed = 0`，非 `.idx` 文件仍在
- [x] golden 断言：T2 结束时 `E2` / `E4` 为 `eligible`（`confirm`），其余按 §11 切片期表（V17）
  - 验收：条目资格逐条比对切片期表（T1-g 落地后按 `vault` 13 / `vaultless` 8 分别断言），`E2` / `E4` 的 `reason` / `confirmation` / 可回收数字固定

> 核对（2026-11-20）：9 条全部落地——V6 在 `src-tauri/tests/storage_reclaim.rs:131`；V8 四组负例在 `:194`（未确认 / 空条目 / action 不符 / 不在计划内）+ `:241`（`planNotFound` / `planExpired`）；白名单负例 `execute_rejects_whitelisted_out_entry_action_pairs`（`:308` 起，`E3+A3` / `E10+A3` / `E11+A3` / `E13+A4` 四组各回 `entryNotAllowed` 且整根签名不变）；V20 后半 `inventory_and_unregistered_keys_are_rejected_on_both_sides`（`:350` 起，`"inventory"` 与未登记子节点 `security/audit/tmp` 在 plan / execute 两侧均被拒）；`busy` `concurrent_execution_returns_busy_without_side_effects`（`:387` 起，持闸门复现并发 → 第二次 `busy`、两份签名不变，释放后同计划可正常执行）；V9 在 `:380`（索引根被替换成文件 → E4 `failed`、E2 `done`），`skipped` 由 `:367` 的二次执行断言；owner 侧幂等两处在 `crates/assistant-storage/tests/attachment_store.rs:106`（`purge_derived_orphans_is_idempotent`，第二次 `removed = 0` / `failed = 0` / 全 `skipped`）与 `crates/assistant-tools/tests/index_stats.rs:47`（`purge_index_files_is_idempotent_and_only_removes_idx`，非 `.idx` 文件两轮都在）；golden 在 `src-tauri/src/cmd/storage/entries.rs:453`（13 条资格比对；T1-g 落地后改为 core 8 条 + 分区条目按 `entries()` 快照逐条比对）+ `:488`（`E2` / `E4` 的 `Eligible` / `DerivedCache` / `Confirm` / 可回收数字）。`busy` 用例不真起线程：直接用 `ReclaimPlanStore::begin_execution()` 占住闸门，语义与「第一条执行未结束」等价且不引入时序抖动。另：V9 的 `partial` 态当前无路径可触发（`E2` / `E4` 各自只会 `done` / `failed` / `skipped`），故该条按已覆盖三态勾选。测试结果：`storage_reclaim` 10 条、`attachment_store` 7 条、`index_stats` 5 条（Windows 下 unix 用例按 cfg 排除）全绿。

---

## 阶段 T3 — 中风险回收（`E3` / `E8` / `E1`）（已完成 2026-11-20）

退出条件：V7、V9、V11、V12 通过；并发用例通过。

### T3-a `E3`（`tool-results`，`A3`）

- [x] `src-tauri/crates/assistant-tools/src/runtime/storage.rs`：新增 `pub fn collect_expired(root, retention_days) -> Vec<ResultCandidate { path, bytes }>`——**复用 T1 的 `for_each_file`** 与 `cleanup_expired_results` 同一判据（`mtime` 早于「今天 − 保留天数」、只常规文件、跳符号链接）
- [x] `src-tauri/crates/assistant-tools/src/runtime/storage.rs`：新增 `pub fn delete_candidates(candidates) -> ResultPurgeReport { removed, removedBytes, failed, skipped }`——**禁止**复用 `CleanupReport` 后自己补算字节（删完取不到长度，§3.10 接口形状 1）
- [x] `src-tauri/crates/assistant-tools/src/runtime/storage.rs`：`mtime` 在未来（时钟偏移）时按「未过期」处理（§5.5）
- [x] `src-tauri/crates/assistant-tools/src/runtime/storage.rs`：`ToolResultStorage` trait 增 `fn collect_expired(&self) -> Option<...>` 与 `fn delete_candidates(&self, ...)`（默认 `None` = 不提供）；`FileToolResultStorage` 覆盖
- [x] `src-tauri/src/cmd/storage.rs`：`E3` 白名单组合 `E3 + A3` 打开；`retentionDays = 0` → `notEligible` + `policyDisabled`（**不得**翻译成「无保留期」，§3.5 硬约束 2）
- [x] `src-tauri/src/cmd/storage.rs`：执行时重新 `collect_expired`（不复用计划候选）；计划后被写入的新文件天然不满足判据

> 核对（2026-11-20）：6 条全部落地——候选收集 `crates/assistant-tools/src/runtime/storage.rs:332`（复用 `for_each_file` + 与 `cleanup_expired_results` 共用的 `retention_cutoff` / `is_expired`，`retention_days == 0` 返回空集）；删除 `:359`（`DeleteReport { removed, removed_bytes, failed, skipped }`，`NotFound` 计 `skipped`、其它错只计 `failed` 不中断）；`mtime` 在未来的文件由 `is_expired` 的「严格早于截止」判据天然排除（`:379` 起）；trait 默认 `None` 在 `:103` / `:110`，`FileToolResultStorage` 覆盖在 `:242` / `:249`；白名单 `E3 + A3` 在 `src-tauri/src/cmd/storage/reclaim.rs:314`（`executor_action`），`retentionDays = 0` → `notEligible` + `policyDisabled` 在 `src-tauri/src/cmd/storage/entries.rs:294` 起的 `tool_results_eligibility`；执行侧重新 `collect_expired` 在 `reclaim.rs` 的 `execute_tool_results`（计划只给数量与字节）。

### T3-b `E8`（`security/backups`，`A4`）

> **实现形态已被 T1-g 修订取代**：条目声明、`pruneToLimit` 动作、`keep` 策略、资格判据与 `prune_backups` 入口全部由 vault 分区自述（`reclaim_actions` / `reclaim`），core 不再持有 `E8 + A4` 白名单与 `security/backups` 字面量。行为契约（先重新计数后裁剪、恒留最新 5 份、`locked` 态仍可 `eligible`、`keep` 不硬编码）**不变**。

- [x] `src-tauri/crates/assistant-vault/src/security_db/state.rs`：现有 `prune_backups(dir, keep) -> Result<()>` 为 `pub(super)` 且**不报字节**；新增 `pub fn prune_backups_to(db_path: &Path, keep: usize) -> Result<PruneReport { removed, removedBytes, failed }, VaultError>`（内部复用 `backup_dir_for` + `BACKUP_PREFIX` / `BACKUP_SUFFIX` 匹配），并把 `MAX_BACKUPS` 暴露给 provider 可达的常量入口
- [x] `src-tauri/crates/assistant-vault/src/security_db/state.rs`：`keep` **至少 1**，**不得删最新一份**；只按 `BACKUP_PREFIX` / `BACKUP_SUFFIX` 命名匹配，跳符号链接
- [x] `src-tauri/src/cmd/storage/probe.rs`：`StorageOwnerProbe` 增回收方法 `fn prune_backups(&self, keep: u32) -> ...`（返回值只用普通数据；vaultless 分支自然不提供 → `E8` 保持 `notImplemented` + `noReclaimApi`）
- [x] `src-tauri/src/vault_plugin.rs`：`VaultProbe` 实现该回收方法
- [x] `src-tauri/src/cmd/storage.rs`：`E8` 白名单组合 `E8 + A4` 打开；`备份数 > MAX_BACKUPS(5)` → `eligible` + `overLimit` + `confirm`；`≤ 5` → `notEligible` + `reason = overLimit`（§3.1 E8 行只给这一个词，`reason` 仅 `notEligible` 时强制非空，语义按「已在上限内、无可裁剪」读）；`autoActive = true` 不变；备份计数走目录列举，**不**依赖 vault 解锁，故 `locked` 态下 `E8` 仍可 `eligible`
- [x] `src-tauri/src/cmd/storage.rs`：`E8` 执行**必须重新计数**再裁剪（§5.2 第 4 条），并且只删「按时间排序后最旧的超限部分」、恒保留最新 `keep = 5` 份（§5.5 E8 行「先创建、后裁剪；不得删最新一份」）；计划后被新增的备份只会把更旧的挤出去，**不会**被删——因此无需额外比对 `sampledAt`；`keep` 由 owner 侧常量 `MAX_BACKUPS` 传入，**不得**在管理器里硬编码 5

> 核对（2026-11-20）：6 条全部落地——owner 侧 `crates/assistant-vault/src/security_db/state.rs:511` 的 `prune_backups_to`（`keep.max(1)` 恒留最新一份、`symlink_metadata` 跳符号链接与非常规文件、目录缺失返回全零报告、按文件名升序裁最旧超限部分、单份失败只计 `failed`）；`MAX_BACKUPS` 在 `security_db/mod.rs` 改为 `pub`（provider 可达常量入口）；trait 默认方法 `None` 在 `src-tauri/src/cmd/storage/probe.rs:70`（`BackupPruneReport` 只用普通数据）；vault 侧实现在 `src-tauri/src/vault_plugin.rs:117`（`MAX_BACKUPS` 由 provider 读、管理器不出现字面量 5 的份数语义，`keep` 由计划策略快照传入）；白名单 `E8 + A4` 在 `reclaim.rs:315`，执行入口 `execute_backups`（`reclaim.rs:534`）在 `spawn_blocking` 里让 owner 重新计数后裁剪、`keep == 0` 视为策略不可用回 `skipped`；资格判定 `entries.rs` 的 `backups_eligibility`（`:337` 起，`count > keep` → `eligible`，否则 `notEligible` + `overLimit`），静态项 `:189` 为 `Eligible` + `OverLimit` + `constant_policy(Some(5))` + `autoActive = true`，且 E8 走目录列举、`locked` 态不降级。E8 的可回收字节需逐文件长度，stats 契约（`{bytes,count,oldestAt,newestAt}`）给不出，故 `reclaimableBytes/Count` 留 `null`（§6.2「任一 eligible 条目算不出则 `null`」）。测试：`storage_reclaim` / `storage_breakdown` / `storage_inventory` 24 条全绿，`cargo test --lib storage` 36 条全绿。

### T3-c `E1`（`attachments`，`A1`）

- [x] `src-tauri/crates/assistant-storage/src/attachment_store.rs`：新增**按附件 ID 分片**的同步锁 `locks: Arc<[std::sync::Mutex<()>; 64]>`（`AttachmentStore` 已是 `#[derive(Clone)]` + `Arc` 共享，锁数组必须同样进 `Arc`，否则每个 clone 各拿一把锁、串行化失效），提供 `fn id_guard(&self, id: &str) -> std::sync::MutexGuard<'_, ()>`（对 `id` 的稳定哈希取模选桶；桶碰撞只增加争用，不破坏「同一 ID 必互斥」）。**不能用 `tokio::sync::Mutex`**：引用写入路径 `insert_attachment_refs` 在 `cmd/chat/insert.rs:330` 是**同步**函数（已持 `&Connection`，其调用者 `cmd/agent/mod.rs` 在 async 命令内、无法 `.await`），`blocking_lock()` 在运行时线程内会 panic
- [x] `src-tauri/crates/assistant-storage/src/attachment_store.rs`：`id_guard` 的**临界区内禁止 `.await`**（否则 `clippy::await_holding_lock` + 跨 await 持锁），只用同步 IO（`std::fs::remove_file` / `std::fs::metadata`）与同步 DB 调用；为此新增同步变体 `pub fn delete_blocking(&self, id: &str) -> Result<Option<u64>, AttachmentError>`（`Ok(Some(len))` = 已删并返回字节、`Ok(None)` = 文件不存在（`ErrorKind::NotFound`，不是错误）、非法 id / 其它 IO 错 = `Err`；顺带清 `derived/{stem}.extract-v1.txt`），既有 `async fn delete` 保留不动（其它调用点零改动）
- [x] 三处调用点共用 `id_guard`（§5.5 第 1–2 行）：`cmd/chat/insert.rs:330`（`insert_attachment_refs` 写引用）、`cmd/storage.rs:130`（`delete_attachments` 的「删引用 + 删文件」）、`A1` 的 `delete_orphans_by_ids`（删文件）。**`insert_attachment_refs` 现在是自由函数且只收 `conn`**（`attachment_store.rs:264`，全仓库**只有 `cmd/chat/insert.rs:330` 一个调用点**），必须改签名为 `pub fn insert_attachment_refs(store: &AttachmentStore, conn: &Connection, attachment_ids: &[String], conversation_id: &str, message_id: &str) -> Result<(), String>`：逐 id 取 `id_guard` → `symlink_metadata` 确认文件仍在（不在 → 返回 `Err`，让整条写入失败，**不**落悬空引用）→ 再 `INSERT OR IGNORE`。**锁顺序恒为「DB 连接锁 → 附件 ID 锁」**（`insert_chat_messages` 已先持 `conn`，A1 也先取 `conn`），反向取锁会 ABBA 死锁。**`save_attachment` 不持锁**：它是内容寻址 `create_new(true)` 写、本身幂等，且 `save_attachment` 是 `async`（tokio fs），持 std 锁会跨 await；它与删除的竞态由上面的锁内存在性校验兜底——先 insert 则引用已落库、A1 查库时跳过；先删则 insert 校验失败并报错，两种情况都不会留下悬空引用
- [x] `src-tauri/src/cmd/storage.rs:130`：`delete_attachments` 改造为「取 `conn` → 逐 id 取 `id_guard` → 删引用（`delete_attachment_refs_by_attachment_id`）→ `delete_blocking` 删文件」，**删掉**现有「先 `list_all()` 建 `size_map`」那一步，字节直接用 `delete_blocking` 的返回值（顺带去掉一次全目录扫描；`removed_count` / `removed_bytes` 语义与 DTO 不变，G13）；`AttachmentDeleteResultDto` 字段一个不改
- [x] `src-tauri/src/cmd/storage.rs:130`：`delete_attachments` 的「文件不存在」语义保持现状——`delete_blocking` 返回 `Ok(None)` 时**既不计 `removed` 也不计失败**（现状是 `delete()` 的 `NotFound` 走 `log::warn` 后不计入），**不得**改成计 `removed`（会虚增字节）或计失败（会改变前端明细区行为，G14）；id 预校验（`is_plain_file_name` → `Err`）原样保留
- [x] `src-tauri/crates/assistant-storage/src/attachment_store.rs`：新增 `pub fn delete_orphans_by_ids(&self, conn: &Connection, ids: &[String], sampled_at: i64) -> OrphanPurgeReport { removed, removedBytes, failed, skipped }`——**同步方法**（锁内要查 DB，不能 async），**方法名与 §3.1 末条 `note` 点名的缺口一致**（T1 的 `note` 就写这个名字，T3 补上后 golden 才逐字对得上）；每个 id 的处理固定为：取 `id_guard` → **在锁内**做单 id 引用存在性查询（`SELECT 1 FROM attachment_refs WHERE attachment_id = ?1 LIMIT 1`，走已有索引 `idx_attachment_refs_att`，见 `db.rs:213`）→ 仍被引用 → `skipped` → `metadata` 确认存在且为常规文件（不存在 → `skipped`）→ **`mtime` 不早于 `sampled_at` → `skipped`**（§5.2 第 5 条，挡住「采样后被导入、引用尚未落库」的窗口）→ `delete_blocking` → 累加 `removedBytes`；ID 非法 → `skipped`；单条失败只计 `failed` 不中断
- [x] `src-tauri/src/cmd/storage.rs`：`E1` 执行顺序固定为「取 `conn` → 调 `delete_orphans_by_ids(conn, 重算的孤儿 ids, plan.sampledAt)`」，引用重算在 owner 的锁内完成，**禁止**信任计划里的孤儿列表（计划只给数量与字节，§3.3 动作边界 2 / §5.2 第 4 条）
- [x] `src-tauri/src/cmd/storage.rs`：`E1 + A1` 打开后 `confirmation = strongConfirm`（风险档由条目决定，不由动作决定，§3.6 硬约束 6）；有孤儿 → `eligible` + `reason = orphan`；无孤儿 → `notEligible` + `reason = referenced`
- [x] `src-tauri/src/cmd/storage.rs`：`A2` **不**经 `reclaim_storage`（继续走既有 `delete_attachments`，语义零改动）

> 核对（2026-11-20）：9 条全部落地（`cmd/storage.rs` 已按域拆分为 `cmd/storage/mod.rs` / `reclaim.rs` / `entries.rs` / `sample.rs`，行号以拆分后为准）——分片锁 `ID_LOCK_BUCKETS`（`crates/assistant-storage/src/attachment_store.rs:11`）、`locks: Arc<[Mutex<()>; 64]>`（`:22`，构造时 `Arc::new(std::array::from_fn(...))` 于 `:216`）、`lock_bucket`（`:72`，`DefaultHasher` 稳定哈希取模）、`id_guard`（`:226`，poison 时 `into_inner` 不 panic）；同步变体 `delete_blocking`（`:521`，`NotFound` → `Ok(None)`、先清 `derived/{stem}.{EXTRACTOR_VERSION}.txt`、`ensure_plain` 前置，既有 `async fn delete` 未改）。三处调用点：引用写入 `insert_attachment_refs`（`:639`，签名已改为 `(store, conn, ids, conversation_id, message_id)`，逐 id `id_guard` → `symlink_metadata` 非 NotFound 且为常规文件才落 `INSERT OR IGNORE`，否则整条 `Err`）、手动删除 `delete_attachments`（`src-tauri/src/cmd/storage/mod.rs:263`，`:284` 取锁 → `:285` 删引用 → `:286` `delete_blocking`，`size_map` 那一步已删，`Ok(None)` 不计 `removed` 也不计失败、`Err` 走 `log::warn` 保持 G14，id 预校验原样保留）、孤儿回收 `delete_orphans_by_ids`（`attachment_store.rs:546`，锁内 `is_attachment_referenced`（`:623`，`SELECT 1 ... LIMIT 1` 走 `idx_attachment_refs_att`）→ `symlink_metadata` 跳符号链接/非常规文件 → `mtime_seconds`（`:173`）`>= sampled_at` 即 `skipped` → `delete_blocking`；非法 id / 不存在 / 仍被引用均 `skipped`，单条失败只计 `failed`，`OrphanPurgeReport` 于 `:143`）。执行侧 `execute_orphans`（`reclaim.rs:390`）：`spawn_blocking` 内先 `get_all_referenced_attachment_ids`（`attachment_store.rs:705`）重算集合、再算孤儿 ids 传给 owner，**不认计划里的列表**；白名单 `E1 + A1` 在 `executor_action`（`reclaim.rs:334`）。资格判定 `orphans_eligibility`（`entries.rs:308`）三态严格分开：`Some(n>0)` → `eligible` + `orphan`、`Some(0)` → `notEligible` + `referenced`、`None`（引用集合取不到）→ `notEligible` + `unknown`（§3.4 Q2，不得把「不知道」说成「仍被引用」）；静态项 `E1` 为 `Eligible` + `Orphan`（`:126`），`confirmation = strongConfirm` 由 `build_entry` 的风险档派生（§3.6 硬约束 6）。可回收量与顶层孤儿列表同源：`attachment_sample`（`sample.rs:102`）收同一份 `entries` + 同一份 `referenced` 只算数字、不做判定，资格与执行判定分别在 `entries.rs` 与 owner 锁内。`A2` 未接入 `reclaim_storage`。测试：`cargo test --lib cmd::storage` 37 条全绿，`cargo clippy --lib --tests` 零警告。

### T3-d 测试

- [x] `src-tauri/tests/storage_reclaim.rs`：V7——`expectedBytes` 与 `removedBytes` 偏差可解释；不可回收条目 `expectedCount` / `expectedBytes = null`
- [x] V11——`E1` 的 `A1` 与 `delete_attachments` 并发，无「删了仍被引用」，同一 ID 串行
- [x] `src-tauri/tests/storage_reclaim.rs`：V12——`E3` 配置读失败回退 `0` 不删；`E11` 无游标时 `eligibility ≠ eligible`
- [x] `src-tauri/crates/assistant-vault/tests/security_db.rs`：`E8` 裁剪不删最新一份、`keep = 1` 时只留 1 份、失败保留多余备份并计 `failed`
- [x] `src-tauri/crates/assistant-tools/tests/storage.rs`：`collect_expired` 与 `cleanup_expired_results` 候选集合逐条一致；`mtime` 在未来的文件不进候选
- [x] golden 断言：T3 结束时 `E1` / `E3` / `E8` 按 §11 切片期表进入 `eligible`（`strongConfirm` / `confirm` / `confirm`）

> 核对（2026-11-20）：6 条全部落地。
> - V7：`storage_reclaim.rs` 的 `execution_resamples_so_removed_bytes_can_deviate_from_expected_bytes`（计划后新增 / 消失的候选由执行侧重算，`removedBytes` 与 `expectedBytes` 的差额逐项可归因）与 `non_eligible_entries_report_no_expected_numbers`（不可回收条目 `expectedCount` / `expectedBytes` 恒 `null`、`confirmation` 恒 `none`）。
> - V11：**下沉到 owner 侧** `crates/assistant-storage/tests/attachment_store.rs`——`a1_and_writer_never_leave_a_dangling_reference`（`A1` 与引用写入并发，两种结果都不得留下悬空引用）、`a1_waits_for_the_same_id_guard`（持同一 ID 锁时 `A1` 阻塞）、`a1_and_manual_delete_are_serialized_per_id`（`A1` 与手动删除序列并发，同一 ID 只删一次且无残留引用）。改位置的依据：`A1` 的锁与锁内引用校验都在 owner 内，且 `E1` 执行入口要求 `ReclaimPorts.db`（`DbState` 不在集成层可构造范围），因此集成层无法真正驱动 `A1` 执行路径，重复写一遍只会得到同一断言的副本。
> - V12：`retention_zero_is_policy_disabled_and_deletes_nothing`（`E3` 保留天数 `0` → 计划 `policyDisabled`、执行 `skipped`、目录签名前后一致）与 `audit_without_cursor_is_never_eligible`（`E11` 无游标 → `ownerManaged` + `reason = expired`、策略与可回收数字全 `null`）。
> - E8：`crates/assistant-vault/tests/security_db.rs` 的 `prune_backups_to_keeps_the_newest_and_ignores_other_files` / `prune_backups_to_keeps_one_copy_even_when_keep_is_zero` / `prune_backups_to_is_a_noop_within_keep_or_without_a_backup_dir` / `prune_backups_to_counts_failure_and_keeps_the_extra_copy` / `prune_backups_to_skips_symlinks`。
> - 候选集合一致：`crates/assistant-tools/tests/storage.rs` 的 `collect_expired_candidates_match_cleanup_one_by_one`（逐条比对 `collect_expired` 与启动清理的候选）、`storage_trait_candidates_match_the_free_functions`、`future_mtime_is_not_an_expired_candidate`、`collect_expired_is_empty_when_retention_is_zero`。
> - golden：`src-tauri/src/cmd/storage/entries.rs` 的 `t3_golden_slice_table_marks_e1_e3_e8_eligible` 直接按 §11 切片期表断言三条目进 `eligible`，且 `reason` / `confirmation` 分别为 `orphan`+`strongConfirm`、`expired`+`confirm`、`overLimit`+`confirm`（确认档位由条目风险档派生，不由动作决定）。
> - 全绿：`cargo test --test storage_reclaim` 14 条、`--test storage_breakdown` 6 条、`--test storage_inventory` 8 条、`--test app_dirs_contract` 2 条、`cargo test --lib cmd::storage` 38 条、`cargo test -p assistant-storage -p assistant-tools -p assistant-vault` 全部通过；`cargo clippy --lib --tests` 零警告。

---

## 阶段 T4 — 前端与收尾（未开工；条目已迁到 [剩余清单](./I61-storage-manager-remaining.md)）

前置：I54 `R0` 已交付（页面容器 + 三 tab 挂载位）。R0 未开工前，T4 只能先做与容器无关的组件内部改造，**不得**自行新建第二个页面。

> 前端实现基线：[I61 存储管理器前端设计](./I61-storage-manager-frontend.md)（页面结构、组件拆分、视图模型、列与分组、文案键树、样式类、测试矩阵）。本清单只保留「改哪里 + 验收动作」，实现细节不在此重述；规格 §6.3 已整体划出作废，前端口径以该设计文档为准，判据 / 字段语义 / 资格与档位仍以规格 §3 / §5 / §6.1 / §6.2 / §7 / §10 为准（冲突时以规格为准并回改设计文档）。

### T4-a 数据与交互

- [ ] ~~`src/services/desktop/storageService.ts`：**扩展现有 service**（不新建 store、不新建 service）——加 `getStorageReclaimPlan(entryKeys)` 与 `reclaimStorage(planId, entries, confirmed)`，以及 `refreshStorageInventory()`（返回 `StorageInventoryStatusDto`，只取 `status`，**不取数字**）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
- [ ] ~~`src/settings/pages/StorageSettings.tsx`：**保留附件明细区全部既有能力**（会话分组 / 孤儿分组 / 折叠 / 组全选 / 勾选删除，服务 `A2`），在其后**追加**已知条目清理区块（4 列：名称 / 占用·条数 / 策略 / 清理；条目数、分组与行序**完全由后端 `entries` 驱动**，前端不假设 13 条、不补占位行、不写死条目名或分区名；未挂载的可选分区（如安全域）整组不渲染，G14 / V16）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
- [ ] ~~`src/settings/pages/StorageSettings.tsx`：条目行按前端设计 §5.4 / §5.6 / §5.7 渲染（可用性 = 占用列角标 + 详情「诊断」行，资格 = 清理按钮可用态，不单独成列）；`bytes = null` 显示占位符（**不得**直接传 `formatBytes`，N17），不改 `src/utils/formatBytes.ts` 的签名~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
- [ ] ~~`src/settings/pages/StorageSettings.tsx`：不可回收条目按钮禁用并显示 `reason` 文案；`notImplemented` 额外显示 `note`（「缺少 owner 方法」）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
- [ ] ~~`src/settings/pages/StorageSettings.tsx`：回收流程固定为 `get_storage_reclaim_plan` → 确认（`strongConfirm` 需二次输入条目名或等价强交互）→ `reclaim_storage` → 结果 toast → 重新 `get_storage_breakdown`~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
- [ ] ~~`src/settings/pages/StorageSettings.tsx`：**禁止**前端自算「可回收字节 / 是否安全」，**禁止**按 `expectedBytes` 自行降档确认强度（档位只信后端 `confirmation`）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
- [ ] ~~`src/settings/pages/StorageSettings.tsx`：`planExpired` / `planNotFound` / `busy` 提示重试，**不自动重放**；`complete = false` 时不展示跨条目合计（G10）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
- [ ] ~~`src/settings/pages/StorageSettings.tsx`：条目清理区块**不**提供第二套「删指定附件」入口，也**不得**把明细区删除按钮搬进条目表~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
- [ ] ~~`src/settings/pages/StorageSettings.tsx`：**第二段只读余量区（未登记占用）**（`breakdown.inventory`）——五态渲染、`partial` / `overRegistered` 提示、列示范围与 `declared` 折叠条件全部按前端设计 §7；**禁止**任何删除 / 清理按钮，**禁止**把余量数字加进页面任何合计（G10）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
- [ ] ~~`src/settings/pages/StorageSettings.tsx`：`useEffect` 内 `listen("storage:inventory-updated", ...)`（与 `src/vault/core/hooks/useVaultEvents.ts` 同款写法，清理函数里调 `UnlistenFn`），收到后**只重拉一次** `getStorageBreakdown`，**不新增轮询**；「重新扫描」按钮调 `refreshStorageInventory()` 后按返回的 `status` 刷新按钮态~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
- [ ] ~~`src/settings/styles/settings.css`：新增条目表样式（与既有 `storage-*` 类名同族，不重命名既有类）＋ 余量区样式（含 `overRegistered` 警示态）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
- [ ] ~~`src/i18n/locales/zh-CN.json` / `en-US.json`：新增 `availability` / `eligibility` / `reason` 词表、策略摘要、确认文案（`strongConfirm` 必须含条目 / 数量 / 字节三件套）、结果 toast 文案，以及余量区文案（按前端设计 §11 键树；`residual.hint` 必须覆盖 `declared` 节点，不能写成「不属于任何条目」）；条目名是**可选覆盖层**（`entries.names.<key 规范化>`，未命中回退 `key` 原文，不要求穷举、不随挂载状态变化），**不写死分组标题 / 行序**；两份 key 结构必须一致~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
- [ ] ~~组件测试（按前端设计 §13 测试矩阵）：`entries` 13 条（全挂载）/ 8 条（未挂载，安全域整组不出现）/ 空 `entries` 三组夹具各一例；断言行数与分组完全来自夹具，不出现占位行、不写死条目名~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
  - 验收：把夹具从 13 条改成 8 条，组件渲染行数随之变化且无残留分组标题

### T4-b 收尾

- [ ] ~~`npm run contract:gen` 后确认 `src/types/protocol.contract.ts` 无手改、无残留~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
- [ ] ~~留痕复核：`execute` 日志只含 `planId` / key / action / 计数 / `sampledAt`（V15）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
- [ ] ~~全量回归：`cargo check --workspace --all-targets`、`cargo test -p assistant-app --lib`、`cargo test -p assistant-storage`、`cargo test -p assistant-tools`、`cargo test --test storage_breakdown`、`cargo test --test storage_inventory`、`cargo test --test storage_reclaim`、`cargo test --test protocol_bindings`、`cargo test --test app_dirs_contract`、`cargo test --test assembly_contract`~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
- [ ] ~~vaultless 构建再跑一遍：`cargo test --no-default-features --test storage_breakdown`（`entries` **8 条**，安全域 `E7`–`E11` 整组不出现，**不得**出现 `notImplemented` 占位行；`security` 首段在 `inventory` 归 `optionalNotMounted`，不是 `unregistered`，见 T1-g 修订）~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
- [ ] ~~前端：`npm run typecheck`、`npm run lint`、`npm run test`~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
- [ ] ~~附件明细区能力逐项回归（V16 的手动清单）：会话分组 / 孤儿分组 / 折叠 / 组全选 / 单选删除 / 删除结果提示~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**
- [ ] ~~I61 §12 迁移清单复核：I54 中存储 tab 的残留条目已全部改为指针，且**未**删除任何共享 DTO 旧字段~~ **（已迁到 [剩余清单](./I61-storage-manager-remaining.md)，以该文件编号为准）**

---

## 门禁自查表（G1–G19，代码审查逐条签字）

| # | 检查点 | 自查方式 |
|---|---|---|
| G1 | 管理器无受管目录名 / 无路径拼接；`cmd/storage.rs` 无 `assistant_vault` / `assistant_security` / `#[cfg(feature)]` | 对 `src-tauri/src/cmd/storage.rs`、`src-tauri/src/cmd/storage/` 检索 `assistant_vault`、`assistant_security`、`cfg(`、`join(` |
| G2 | 管理器无 `read_dir` 遍历受管目录；**唯一例外** `cmd/storage/inventory.rs`（边界见 §3.11.2 第 1 条） | 检索 `read_dir`，只允许命中 `inventory.rs`；再在 `inventory.rs` 检索 `remove_*` / `fs::write` / `log::`，应为空 |
| G3 | 管理器无 `fs::remove_*` | 检索 `remove_file` / `remove_dir` / `remove_dir_all` |
| G4 | 每个条目都有 owner getter + stats 入口（`E12` 明确记为无入口）；**core 条目的声明来源是 `MANAGED_ROOTS`**（新增 `dir_name` 常量必须同时新增登记，V19）；**分区条目的声明来源是功能分区**，core 不补声明、不写其 `key` / 子段字面量 | 对照 §3.10 表逐条核对 + V19 单测；`registry.rs` 检索功能条目 `key` 零命中 |
| G5 | fail-closed：策略读不到 / 判据推不出 / 非普通文件 / 符号链接 → 不删 | owner 单测覆盖符号链接与非常规文件 |
| G6 | 取不到数 `null` + `availability` + `reason`；`missing` 计 `0` | `storage_breakdown` 单测 |
| G7 | 观测 / 计划 / 执行三步分离；计划无副作用；执行带 `planId` + `confirmed` | V6 / V8 |
| G8 | 不自动回收 `E1` / `E5` / `E6` / `E7` / `E9` | 白名单 + `autoActive` 快照 |
| G9 | 回收后重新采样，账本恒等式仍成立 | 执行后 `get_storage_breakdown` 断言 |
| G10 | 不把 core 8 条 + 已挂载分区条目相加成「总占用」；只允许附件账本与 `totalReclaimableBytes` 两处合计；余量数字不进任何合计、不回流 `entries` | DTO / UI 复核 + V20 |
| G11 | 无全局回收锁；只串行化自己的执行请求 | 计划缓存状态复核 |
| G12 | 留痕无绝对路径 | 日志断言 |
| G13 | 既有 wire 字段零改动 | `protocol.contract.ts` diff 只增不改 |
| G14 | 前端附件明细区能力全保留 | T4-b 手动回归 |
| G15 | 口径声明唯一（§3.9）、遍历实现归 owner；同一棵 fixture 喂给任意 owner stats 入口，四元组逐字一致 | V21 单测 + 检索第二份口径表 |
| G16 | 条目身份归声明方：`E7`–`E11` 的 `key` / `segments` 只在功能分区声明，core 登记表只保留首段 `OPTIONAL_SEGMENTS` | `app_dirs/registry.rs` 检索 `security/` 零命中；V19 / V24 |
| G17 | 声明与接收同一套词汇：`owner` 只作展示不承载路由；分区自答 `entries()` + `stats(key)`；按 `key` 分派，无下标常量 | V22；检索 `const .*: usize =` 与 `MANAGED_ROOTS[` |
| G18 | 分区注册进唯一通用槽 `SectionRegistry`，建槽先于注册（`Builder` 阶段建槽、`feature_mounts::on_setup` 注册）；按声明取数，未挂载 = 条目不存在 | V23；`plugins.rs` / `feature_mounts.rs` 时序复核 |
| G19 | 登记表陈述磁盘事实、不承载策略；`security` 首段恒在、条目随功能；死事实删除 | 事实检验法（删掉分区后首段仍成立）+ V24 |

---

## 验收判据归属

| 判据 | 切片 | 说明 |
|---|---|---|
| V1 / V2 / V3 / V4 / V5 | T1 | `vault` 13 行 / `vaultless` 8 行 key 顺序 / `null` 语义 / `action = none` 约束 / `reason`·`note` / 账本恒等式 |
| V6 / V8 / V9 / V10 | T2 | 计划无副作用 / 四组负例 / 失败隔离 / 静态门禁 |
| V7 / V11 / V12 | T3 | 预期与实测偏差 / `E1` 并发 / fail-closed 与游标 |
| V13 | T1 + T4 | `vault` / `vaultless` 两种构建各跑一遍 |
| V14 | T1 / T2 / T4 | 每次改 DTO 都跑 `contract:gen` |
| V15 | T2 / T4 | 留痕字段与不含路径 |
| V16 | T4 | 超集不窄化（后端全绿 + 前端手动回归） |
| V17 | T1–T4 | 每个切片结束都用 golden 对一遍切片期资格表 |
| V18 / V19 | T1 | 余量守恒与互斥 / 登记表双向一致（判据见 §10） |
| V20 | T1 + T2 + T4 | 余量不影响条目（T1 字段比对、T2 白名单负例、T4 前端无合计） |
| V21 | T1-g 修订 | 口径行为一致（G15）：同一棵 fixture（含伴生文件 / 符号链接 / 非常规文件 / 嵌套子目录）喂给每个 owner stats 入口，四元组逐字一致；适配函数漏搬字段即红 |
| V22 | T1-g 修订 | 分区自述与 core 事实一致（G17）：分区 `entries()` 的 `key` 在自身 `stats(key)` 返 `Some`、非本分区 `None`；首段 ⊆ `OPTIONAL_SEGMENTS` 且 `vault` 构建下反向也满；`owner` 命名域合规；无数字下标接收 |
| V23 | T1-g 修订 | 通用槽路由（G18）：零 / 单 / 双分区 + 重名 + 未挂载 + 快照六态；零分区时 `entries` 8 条、`security` 归 `optionalNotMounted`、无占位行；单分区 13 条与改造前逐字段等价 |
| V24 | T1-g 修订 | core 不感知具体功能（G19(c)）：`security` 只出现在 `OPTIONAL_SEGMENTS` 定义处；不是实现类型名 / feature 名 / 不含 `::`；改实现类型名时登记表零改动 |

---

## 明确不改动项（避免顺手扩大范围）

1. `StorageBreakdownDto` 既有 4 字段、`AttachmentSummary` / `ConversationAttachmentSummary` / `AttachmentDeleteResultDto` / `AssetDirsDto` / `AttachmentLimitsDto` / `AttachmentImportDto`：键名 / 类型 / 含义全部原样（G13）。
2. `delete_attachments` / `open_attachment` / `import_attachment` / `get_asset_dirs` / `get_attachment_limits`：签名与语义不变。
3. `E11` 的审计裁剪**不**加手动入口；`prune_before` 只由 owner 带补录游标调度。
4. `E10` 的 `cleanup_recovery_dir` **不**加手动入口（避免删进行中的恢复）。
5. `E5` 的 `.tmp` 清扫留在 owner 的 `atomic_write` 路径，**不是**管理器动作。
6. `E13` 的 `prune` 本期不做（OQ8 未定案），只补 stats。
7. `logs/` 仍由 `tauri-plugin-log` 轮转，管理器只展示（`A6`），不调用插件内部 API。
# I61 — 存储管理器 任务清单

> 规格：[I61-storage-manager-spec.md](./I61-storage-manager-spec.md)（**唯一事实基线**，判据 / 词表 / 白名单 / 门禁全部以它为准；本清单只拆解「改哪里、验收跑什么」，不重述规格语义）
> 事实基线：I61 §2 的 N1–N18；分析：[STORAGE-MONITOR-ANALYSIS.md](../analysis/STORAGE-MONITOR-ANALYSIS.md)
> 关联：[I54-resource-monitor-spec.md](./I54-resource-monitor-spec.md) §3（资源监视器页面壳，存储 tab 的挂载容器）

切片对应规格章节：T1 观测补全（§3.1–§3.2 / §3.9 / §3.10 / §3.11 / §6.2 / §7.1）→ T2 骨架 + 低风险回收（§5.1–§5.4 / §6.1 / §3.3 白名单）→ T3 中风险回收（§3.10 接口形状 / §5.5 / §3.6）→ T4 前端与收尾（§6.3 / §10 / §12）。

依赖：T1 无前置（后端可独立验收）；T2 依赖 T1；T3 依赖 T2（骨架 + 按 ID 锁）；T4 依赖 T1–T3 **且**依赖 I54 的 `R0`（页面容器与三 tab 挂载位，当前未开工）。

> 本清单不管理提交。提交粒度按代码依赖与可编译性现场决定；这里只回答「代码改没改、验收跑没跑」。
> 勾选表示代码已改且对应验收已跑。

> **路径读法**：文中 `src-tauri/src/cmd/storage.rs` 一律指「T1-c 首条目录化完成后」的 `src-tauri/src/cmd/storage/mod.rs`（模块路径仍是 `cmd::storage`）；其中带 `:130` 之类行号的是**改造前现状**行号，仅用于定位 `delete_attachments` 等既有函数，不要当作改动后的行号。目录化后的完整布局：`storage/mod.rs`（命令 + 装配）、`storage/probe.rs`（可选能力 trait）、`storage/inventory.rs`（§3.11 对账扫描，**唯一 `read_dir` 例外**）、`storage/reclaim.rs`（T2 起的计划与执行）。

---

## 阶段 T1 — 观测补全（只读，无回收）

退出条件：V1–V5、V10、V13、V18、V19、V20 通过；golden 可断言 13 行；`reclaim.action` 按终态填、`eligibility` 按切片期填；`inventory` 键恒在（首次读 `status = neverRun`）。

### T1-a 契约与 DTO

- [ ] `src-tauri/src/protocol/dto/storage.rs`：新增 9 个封闭枚举（按既有命名约定加 `Dto` 后缀：`StorageLayerDto` / `StorageByteBasisDto` / `StorageReclaimActionDto` / `StorageEligibilityDto` / `StorageReclaimReasonDto` / `StoragePolicySourceDto` / `StorageConfirmLevelDto` / `StorageReclaimStatusDto` / `StorageReclaimErrorCodeDto`），取值与 I61 §6.2 表**逐字一致**，`#[serde(rename_all = "camelCase")]`（`StorageByteBasisDto` 唯一值 `fileLength`）
- [ ] `src-tauri/src/protocol/dto/storage.rs`：新增 `ReclaimPolicyDto` / `ReclaimViewDto` / `StorageEntryDto`，字段顺序与 I61 §3.2 / §6.2 一致；输出侧 `Option` **不加** `skip_serializing_if`（键恒在、值 `null`）
- [ ] `src-tauri/src/protocol/dto/storage.rs`：`StorageBreakdownDto` **只追加** `sampledAt`（`i64`）/ `byteBasis` / `sharedBytes`（`u64`）/ `referencedOnceBytes`（`u64`）/ `entries`（`Vec<StorageEntryDto>`）/ `complete`（`bool`），全部 `#[serde(default)]`；既有 4 字段键名 / 类型 / 含义**零改动**（G13 / V16）
- [ ] `src-tauri/src/protocol/dto/storage.rs`：`StorageEntryDto` 的 `key` / `layer` / `owner` / `includesDerived` / `countsSelfLogs` / `availability` / `reclaim` **必填**；`bytes` / `count` / `oldestAt` / `newestAt` / `availabilityReason` 为 `Option`；**不新增** §6.2 之外的 DTO（provider 的返回类型用 `probe.rs` 的普通结构，见 T1-c）
- [ ] `src-tauri/tests/protocol_bindings.rs`：新增类型全部登记进 `use` 与 `decl(&cfg)` 列表（漏一个直接红，N18）
- [ ] `npm run contract:gen` 后提交 `src/types/protocol.contract.ts`（禁止手改生成文件）

### T1-b owner stats 入口（13 条，按 I61 §3.10 表逐个落地）

- [ ] **遍历口径（13 条共用，唯一解释）**：只 `metadata` 累加**常规文件**长度；符号链接 / 非常规文件 → 跳过且不计入 `bytes` / `count`，**不**使该条降级；`unreadable` 只用于目标路径本身的 `read_dir` / `metadata` 失败（§7.1）。即 §3.9 表末行「观测记 `unreadable`」按「不跟随、不计数、只跳过」执行——整条降级会让含单个符号链接的目录永久不可用，与 §3.10 接口形状 2 的「stats 与清扫同一套遍历规则」互相矛盾
- [ ] **伴生文件口径（13 条共用）**：`-journal` / `-wal` / `-shm` / `.tmp` / `.lock` / 部分写入残留**不计入** `bytes` / `count`，也不单列条目；由 owner 在回收时一并清理（§3.9 包含范围）
- [ ] `src-tauri/crates/assistant-storage/src/attachment_store.rs`：`list_all` 的 `path.is_file()` 改为 `std::fs::symlink_metadata(&path)?.file_type().is_file()`——`Path::is_file()` 会**跟随**符号链接，把链接目标算成附件，直接违反 §3.9 与 G5（fail-closed）；`E1` 与顶层旧字段同源，所以这处必须改，否则 `totalBytes` 与 `E1.bytes` 一起算错。预期可见变化：附件目录里若有符号链接，该条不再出现在明细区（属正确行为，G14 保留的是「既有能力」而非异常文件）；**其余语义（`created_at` = `modified()`、`read_dir` / `metadata` 失败仍 `?` 上抛，`E1` 不参与 500 ms 超时、失败即整命令 `Err`，§7.1 特例 1）原样不动**
- [ ] **跨 crate 类型约定**：`assistant-storage` 与 `assistant-tools` 互不依赖（不得为共享 `DirStats` 新增依赖），因此 `derived_stats` 返回自己的 `AttachmentDirStats { bytes, count, oldest_at, newest_at }`（定义在 `attachment_store.rs`），`assistant-tools` 与 `assistant-crash` 各自定义同名结构；**统一由 `cmd/storage.rs` 映射进 `StorageEntryDto`**，`cmd::mascot_asset` 直接复用 `assistant_tools` 的 `DirStats`（app 已依赖，不再造第三份）——注意路径是 `assistant_tools::storage::DirStats`（`lib.rs` 的 `pub use runtime::{... storage}` 再导出），`assistant_tools::runtime::storage::DirStats` 在 crate 外**不可达**（`mod runtime;` 是私有的）
- [ ] `src-tauri/crates/assistant-storage/src/attachment_store.rs`：新增 `pub async fn derived_stats(&self) -> Result<AttachmentDirStats, AttachmentError>`——递归 `derived/`（owner 内部拼路径），只算常规文件，跳符号链接与非常规文件，`oldestAt` / `newestAt` 取 `modified()`，空目录两端为 `None`
- [ ] `src-tauri/crates/assistant-tools/src/runtime/storage.rs`：新增 `pub struct DirStats { pub bytes: u64, pub count: u64, pub oldest_at: Option<i64>, pub newest_at: Option<i64> }`（**字段形状全仓库统一**：`bytes` / `count` 是 `u64` 而非 `Option`，`null` 语义只由管理器的 `StorageEntryDto` 表达——owner 侧「取不到」一律走 `Err`，所以这里不需要 `Option`；`oldest_at` / `newest_at` 取 `modified()`，空目录为 `None`）。`assistant-storage` / `assistant-vault` / `assistant-crash` 各自定义同形状副本（不得为共享结构新增跨 crate 依赖），app 层（`setup.rs` / `cmd/mascot_asset.rs`）直接复用 `assistant_tools::storage::DirStats`
- [ ] `src-tauri/crates/assistant-tools/src/runtime/storage.rs`：新增自由函数 `pub fn result_stats(root: &Path) -> io::Result<DirStats>`（**不带 `retention_days`**：§3.10 表里那个参数只在判据函数用，stats 只按目录遍历；若照抄会得到未使用参数）；`NotFound` → `Ok(DirStats::default())`（空态，由管理器判 `missing` 计 `0`），其余 io 错误 → `Err`（由管理器判 `unreadable` + `null`，V2）；显式参数形式供 owner 内部与测试用，管理器不直接调它（见下条 trait 方法）
- [ ] `src-tauri/crates/assistant-tools/src/runtime/storage.rs`：抽一层共用遍历 `for_each_file(root, |path, len, mtime| ...)`（递归会话子目录 / 跳符号链接 / 跳非常规文件），`result_stats` 与 `cleanup_expired_results`（以及 T3 的 `collect_expired`）**全部基于它**；否则「统计 100 MiB、清理 0 字节」的漂移必然出现（§3.10 接口形状 2）
- [ ] `src-tauri/crates/assistant-tools/src/runtime/storage.rs`：`FileToolResultStorage` 增 `retention: AtomicU32`（默认 `0` = 禁用，即 fail-closed）与 `effective_since: i64`，并提供**链式注入** `pub fn with_retention(self, days: u32, effective_since: i64) -> Self`——**不改 `new(root)` 签名**（`tests/` 有 13 处 `FileToolResultStorage::new(dir.path())` 调用点）；`result_stats()` / `result_retention()` 与清理入口都**只取注入值**，不自行读配置
- [ ] `src-tauri/crates/assistant-tools/src/runtime/storage.rs`：`ToolResultStorage` trait 增 `fn result_stats(&self) -> Option<Result<DirStats, String>>`（默认 `None` = 该实现不提供 stats）与 `fn result_retention(&self) -> Option<(u32, i64)>`（保留天数 + 注入时刻，供 `E3` 的 `policy` 自述；默认 `None` = 未注入）；`FileToolResultStorage` 覆盖两者
- [ ] `src-tauri/crates/assistant-tools/src/runtime/storage.rs`：`ToolResultStorage` 增 `fn collect_expired(&self) -> Option<...>` 与 `fn delete_candidates(&self, ...)`（默认 `None` = 不提供，T3 用）
- [ ] `src-tauri/crates/assistant-tools/src/os/tools/super_index/`：新增 `pub fn index_stats(root: &Path) -> io::Result<DirStats>`（`NotFound` → `Ok` 空态，其余 io 错误 → `Err`，与 `result_stats` 同语义）。**必须是 `pub` 而非 `pub(crate)`**：`os/tools/mod.rs` 里 `pub(crate) mod super_index;`（该模块不对外），要跨 crate 可达只能在 `os/mod.rs` 加 `pub use tools::super_index::{index_stats, purge_index_files, IndexPurgeReport};`——`pub use` 再导出 `pub(crate)` 条目会编译报错（E0365），`super_index` 模块本身的可见性不用动（`pub use` 只要求条目是 `pub`）。管理器只调 `assistant_tools::os::index_stats` 这个公开入口（G1，§3.10 接口形状 3）
- [ ] `src-tauri/src/cmd/mascot_asset.rs`：新增 `pub(crate) fn asset_stats() -> Result<DirStats, String>`——遍历素材子目录，**排除 `.tmp` 伴生文件**（§3.9 包含范围），符号链接与非常规文件不计入
- [ ] `src-tauri/src/setup.rs`：`DbState` 扩为 `pub struct DbState(pub Arc<Mutex<Connection>>, pub PathBuf)`——`Arc` 是必需的：`reclaim` 的执行是 `async fn`（要 `await` `ToolResultStorage::delete_candidates`），而 `&Connection` **不是 `Send`**（`rusqlite::Connection` 只 `Send` 不 `Sync`），直接把它放进 `async fn` 入参会让 future 非 `Send`、Tauri 命令编译失败；`Arc<Mutex<Connection>>` 是 `Send + Sync`，且集成测试能自行构造。既有 8 处 `state.0.lock()` **不受影响**（`Arc` 自动解引用到 `Mutex`）：`tool_setup.rs:277` / `:293`、`mcp/config.rs:128`、`cmd/mod.rs:75`、`cmd/agent/mod.rs:71` / `:95`、`adapters/permission_store.rs:16`、`adapters/compaction_sink.rs:21`。构造点只有 `setup.rs:70` 一处，改成 `handle.manage(DbState(std::sync::Arc::new(std::sync::Mutex::new(conn)), dirs.database()))`（`dirs.database()` 在同函数第 48 行已算好，直接复用，**不要**再 `AppDirs::resolve()` 一次）。新增 `pub fn db_file_stats(&self) -> io::Result<DirStats>`：**只 `fs::metadata(&self.1)`**，不打开 DB、不读 `-journal` / `-wal` / `-shm`（§3.9 伴生文件不计入）；`count` 恒 `0` 或 `1`（文件存在 → `1`，`NotFound` → `0`），`oldest_at` / `newest_at` 同取该文件的 `modified()`（§7.1 特例 2）
- [ ] `src-tauri/crates/assistant-vault/src/security_db/state.rs`：新增 `pub fn db_file_stats(&self) -> io::Result<DirStats>`（只 `metadata`，`db_path()` 已有）/ `pub fn backup_stats(&self) -> io::Result<DirStats>`（`backup_dir_for` 内部用，**不对外暴露路径**）/ `pub fn audit_stats(&self) -> io::Result<DirStats>`（`audit_dir`）；`DirStats` 定义在 `assistant-vault` 内（与 `assistant-tools::DirStats` 同名但独立，跨 crate 不共享）；`security_db/mod.rs` 现在只写 `pub use state::SecurityDbState;`（`mod state;` 私有），必须补成 `pub use state::{DirStats, SecurityDbState};`，否则 `vault_plugin.rs` 拿不到该类型（路径 `assistant_vault::security_db::DirStats`）
- [ ] `src-tauri/crates/assistant-vault/src/security_db/state.rs`：`audit_retention_days()` 已存在，需新增 `effective_since: AtomicI64`（在 `set_audit_retention_days` 内一并写入注入时刻）与 `pub fn audit_policy(&self) -> (u32, i64)`，供 `E11` 的 `policy` 自述（管理器不读配置，§3.5 硬约束 1）
- [ ] `src-tauri/crates/assistant-vault/src/security_db/state.rs`：`prune_backups` / `MAX_BACKUPS` 的可达性留给 T3；T1 只读，不改可见性
- [ ] `src-tauri/crates/assistant-crash/`：新增 `pub fn dump_stats(dir: &Path) -> io::Result<DirStats>`（现只有 `install`）；`crash_dump.rs` 里定义 `DirStats`（本 crate **不依赖** `assistant-tools`，`Cargo.toml` 只有 `crash-handler` / `minidump-writer` / `chrono`，不得为共享结构新增依赖），且 `lib.rs` 现在是 `mod crash_dump;` + `pub use crash_dump::install;`，必须**同时**补 `pub use crash_dump::{dump_stats, DirStats};`——否则模块私有、外部取不到。只统计 `panic_*.log` / `crash_*.dmp` / `crash_*.log` 三类常规文件（文件名格式由 `timestamp_str()` 决定，见 `crash_dump.rs:64` / `:76` / `:77`）
- [ ] `E12`（`logs/`）**不新增任何 owner 方法**：管理器直接产出 `availability = notImplemented` + `availabilityReason` 固定文案「`tauri-plugin-log` 不暴露 stats」+ `bytes` / `count = null`（§3.10 表末行）；该文案进 golden 快照，**不得**改写为其他措辞

### T1-c 可选能力 provider（唯一 feature 缝）

- [ ] **结构改造（先做，否则 `probe.rs` 无处安放）**：`src-tauri/src/cmd/storage.rs` 改为目录模块 `src-tauri/src/cmd/storage/mod.rs`（T1 内新增子模块 `probe.rs` 与 `inventory.rs`，T2 再加 `reclaim.rs`；`mod probe;` / `mod inventory;` 只在 `mod.rs` 里声明，`inventory.rs` 的文件头注释写明「G2 唯一例外」）——模块路径仍是 `cmd::storage`，`cmd/mod.rs` 的 `pub mod storage;` 与 `ipc.rs` 的 `cmd::storage::get_storage_breakdown` / `cmd::storage::delete_attachments` **零改动**；**禁止**用 `#[path]` 挂载或新建同名 `storage.rs`（`assembly_contract` 按常规布局扫描源码）

- [ ] `src-tauri/src/cmd/storage/probe.rs`：定义 `pub trait StorageOwnerProbe: Send + Sync`，方法**同步阻塞**（`fn security_db_stats(&self) -> Result<ProbeStats, ProbeError>` / `backup_stats` / `repo_stats` / `recovery_stats` / `audit_stats`，签名同形）**外加一个例外** `fn audit_policy(&self) -> Option<(u32, i64)>`（保留天数 + 注入时刻；`None` = 取不到 `SecurityDbState`，此时 `E11` 的 `policy.retentionDays` / `effectiveSince` 填 `null`，**不**整条降级）；参数与返回值**只用普通数据**（纯 `u64/i64/u32` 与 `ProbeStats`），**不得**出现 `assistant_vault` / `assistant_security` 类型（§3.10 provider 第 1 条）
- [ ] `src-tauri/src/cmd/storage/probe.rs`：trait 返回值用 `probe.rs` 内定义的普通结构 `ProbeStats { bytes: Option<u64>, count: Option<u64>, oldest_at: Option<i64>, newest_at: Option<i64> }`；`VaultProbe` 负责把 `assistant_vault` 的 stats 映射成它（这样 vault 侧类型一个都不出现在 `cmd/storage.rs`）
- [ ] `src-tauri/src/cmd/storage/probe.rs`：trait 每个方法返回 `Result<ProbeStats, ProbeError>`，并定义 `pub enum ProbeError { Locked, Unavailable, Unreadable }`（**封闭三值，不新增**）。管理器映射固定为：`Locked` → `availability = locked`、`Unavailable` → `availability = unavailable`、`Unreadable` → `availability = unreadable`；三种错误都使 `bytes` / `count = null`。**`E9` 的锁定只能靠 `ProbeError::Locked` 表达**（`ProbeStats` 里没有状态位）；**`E7` 的 `metadata` 失败必须返回 `ProbeError::Unavailable`**（不是 `Unreadable`），管理器据此把 `eligibility` 置 `locked` + `reason = locked`（§3.1 E7 行）
- [ ] `src-tauri/src/cmd/storage/probe.rs`：定义 `pub type StorageOwnerProbeHandle = Arc<dyn StorageOwnerProbe>;`；**注入端与取用端必须用同一别名**（类型不一致会静默取不到，表现为「永远降级」）
- [ ] `src-tauri/src/cmd/storage/probe.rs`：trait 与别名**不得**被 `#[cfg]` 门控（vaultless 构建也要有类型可写降级分支）
- [ ] `src-tauri/src/vault_plugin.rs`：在 `#[cfg(feature = "vault")] mod mounted` 内实现 `VaultProbe`（`security_db_stats` / `backup_stats` / `audit_stats` 转调 `SecurityDbState`；`repo_stats` 用 `cmd::vault::repo_dir(&AppDirs::resolve()?)` + `SecurityDbState::is_locked()`（锁定 → 返回 `Err(ProbeError::Locked)`）；`recovery_stats` 用 `cmd::vault::recovery_dir(&dirs)`），并在插件 `setup` 里 `app.manage(Arc::new(VaultProbe) as StorageOwnerProbeHandle)`。三处路径来源已存在且均为 `pub(crate)`（`cmd/vault/mod.rs:39` `repo_dir` / `:71` `recovery_dir` / `:30` `security_dir`），`VaultProbe` 与本文件同 crate 可直接用；`AppDirs` **不是** managed state，只能每次调用 `AppDirs::resolve()`（廉价、无副作用），其失败必须映射为 `Err(ProbeError::Unavailable)`（**不得** `?` 直接传播成别的错误类型）
- [ ] `src-tauri/src/vault_plugin.rs`：`VaultProbe` 持 `AppHandle`，**调用时**才用 `try_state::<SecurityDbState>()` 解析（不在构造期缓存句柄，避免 vault 重建后持到失效引用；`try_state` 取不到 → 返回 `Err`，管理器按 `unreadable` 处理，**不** panic）；注入点必须在 `crate::cmd::vault::vault_setup(app)?` **之后**（`SecurityDbState` 在该函数内 `manage`），否则 `try_state` 恒失败、`E7`–`E11` 静默永久降级
- [ ] `src-tauri/src/vault_plugin.rs`：`#[cfg(not(feature = "vault"))] mod mounted` 保持**不注入**（不得塞一个假实现）
- [ ] `src-tauri/src/cmd/storage.rs`：只 `app.try_state::<StorageOwnerProbeHandle>()`；取不到 → `E7`–`E11` 五条 `availability = notImplemented` + `availabilityReason` 写「可选能力未装配」+ `bytes` / `count = null`，`eligibility` 按 §3.10 第 5 条填（`E7` `notEligible`/`businessData`、`E9` `notEligible`/`userData`、`E8` `notImplemented`/`noReclaimApi`、`E10`/`E11` `ownerManaged`）
- [ ] 门禁自查：`cmd/storage.rs` **无** `use assistant_vault` / `assistant_security`、**无** `#[cfg(feature = ...)]`（G1）

### T1-d 管理器装配（`get_storage_breakdown` 扩展）

- [ ] `src-tauri/src/cmd/storage.rs`：`sampledAt` 只取一次（采样开始时刻，秒），**禁止**每条各取一次（§3.10 第 4 条）
- [ ] `src-tauri/src/cmd/storage.rs`：`E1` 与顶层字段**同源**——一次 `list_all()` + 一次 `attachment_refs` 查询，同时填 `totalBytes` / `totalCount` / `orphans` / `conversations` / `E1` 的 `bytes` / `count`；**不得**为条目再遍历一次目录（§3.10 第 1 条）
- [ ] `src-tauri/src/cmd/storage.rs`：会话标题 / 更新时间改为一次 `JOIN`（或复用 `conversations` 表行），消除 N16 的每会话 2 次 `query_row`
- [ ] `src-tauri/src/cmd/storage.rs`：`sharedBytes` / `referencedOnceBytes` 由 `attachment_refs` 的「附件 → 会话数」直接算（同一附件只归属一次），保证 `totalBytes = referencedOnceBytes + sharedBytes + orphanBytes`（V5）
- [ ] `src-tauri/src/cmd/storage.rs`：`E2`–`E13` 与 `E1` **并行**采样；异步 owner（`AttachmentStore` / `ToolResultStorage`）用 `tokio::time::timeout`，同步 owner（provider / `metadata` / 目录遍历）用 `spawn_blocking` + 对 `JoinHandle` 超时；单条 **500 ms** 超时 → `bytes/count = null` + `availability = unavailable` + `availabilityReason = timeout`（超时只放弃等待，**不取消**，因此 owner stats 必须只读 / 幂等 / 不持锁长遍历）
- [ ] `src-tauri/src/cmd/storage.rs`：`E1` **不参与** 500 ms 超时；`list_all` 失败即整命令 `Err`（旧字段非 `Option`，§7.1 特例 1）；**禁止**把旧字段改成 `Option` 或加 `default` 伪装成 0
- [ ] `src-tauri/src/cmd/storage.rs`：13 条按 `E1`→`E13` 固定顺序装配（key 与 §3.1 逐字一致），`missing` 计 `0`、其余取不到数为 `null`（G6）；`complete` = 除 `E12` 外 12 条全部 `availability ∈ {available, missing}`
- [ ] `src-tauri/src/cmd/storage.rs`：`reclaim.action` **按终态矩阵**填（§3.1 本期动作口径表），T1 只把 `eligibility` 降级为 `notImplemented` + `reason = noReclaimApi` + `note` 点名缺的方法——**`action` 必须保持终态动作**（`E2`/`E4` = `rebuildCache`、`E3` = `sweepExpired`、`E8` = `pruneToLimit`、`E1` = `deleteOrphans`、`E13` = `none`），禁止把 `action` 也降级成 `none`（§11 T1 交付列）；`E5`/`E6`/`E7`/`E9` = `none` + `notEligible` + `userData`/`businessData`；`E10`/`E11` = `sweepExpired` + `ownerManaged`；`E12` = `rotate` + `ownerManaged` + `externalOwner`
- [ ] `src-tauri/src/cmd/storage.rs`：`policy` 按 §3.5 逐条填——`E3` = `config` + `retentionDays`（owner 注入值）+ `effectiveSince` + `mutable = true`；`E11` = `config` + `retentionDays`（`audit_policy()` 自述值）+ `effectiveSince` + `mutable = true`；`E8` = `constant` + `keepCount = 5` + `autoActive = true`（**与 `security_db::MAX_BACKUPS = 5` 手工对齐**：T3 的 `prune_backups_to()` 内部就用 `MAX_BACKUPS`，因此改常量必须同步此处的展示值，否则「展示 5、执行保留 N」会漂移；不为此新增自述方法，保持 §3.5 的「常量」语义）；`E10` = `constant`（无数量/时间上限，`note` 不写）；`E12` = `constant` + `keepCount = 3`（插件 `KeepSome(3)`），10 MiB 单文件上限写 `note`、`maxBytes = null`；`E4` = `owner` + `maxBytes = null` 且 64 MiB 单文件上限写 `note`；`E1`/`E2`/`E5`/`E6`/`E7`/`E9`/`E13` = `none`；`failClosed` 恒 `true`，`mutable` 仅 `config` 为 `true`
- [ ] `src-tauri/src/cmd/storage.rs`：`confirmation` 两步判定（§3.6 硬约束 5）——第一步 `eligibility ≠ eligible` 恒 `none`；第二步 `eligible` 按条目风险档定档（`E1` 高 → `strongConfirm`，`E2`/`E4`/`E3`/`E8` 低/中 → `confirm`），`expectedBytes` 不参与判定；`reclaimableBytes` / `reclaimableCount` 只对 `eligible` 给数（T1 全部 `null`），且必须 `≤ bytes`（V17 前半）
- [ ] `src-tauri/src/cmd/storage.rs`：`E9` 锁定态 `availability = eligibility = locked`（`availabilityReason` 写「vault 未解锁」）；`E7` **不**因 vault 锁定变 `locked`（只 `metadata`）——正常为 `notEligible` + `reason = businessData`，仅当 `metadata` 失败（文件正被替换 / 迁移）时 `availability = unavailable` + `availabilityReason` 写「文件正在替换 / 迁移」**且** `eligibility = locked` + `reason = locked`（§3.1 E7 行是唯一解表，`availability` 与 `eligibility` 在这里**不同值**，不要「统一」成一个）；vaultless 构建 `E7`–`E11` 的 `availability = notImplemented`（`eligibility` 仍按 §3.10 第 5 条：`E7` `notEligible`/`businessData`、`E9` `notEligible`/`userData`、`E8` `notImplemented`/`noReclaimApi`、`E10`/`E11` `ownerManaged`）（§7.1 特例 2/3/4）
- [ ] `src-tauri/src/cmd/storage.rs`：T1 阶段 `notImplemented` 的 `note` **逐字点名缺的方法**（golden 断言依赖这些字面量，T2/T3 补齐后对应 `note` 置 `null`、`eligibility` 转 `eligible`）：`E1` → `AttachmentStore::delete_orphans_by_ids`；`E2` → `AttachmentStore::list_derived` / `AttachmentStore::purge_derived_orphans`；`E3` → `ToolResultStorage::collect_expired` / `delete_candidates`；`E4` → `super_index::purge_index_files`；`E8` → `SecurityDbState::prune_backups_to`；`E13` → `assistant_crash::prune_to_limit`（**唯一例外**：`E13` 全程保持 `notImplemented` / `noReclaimApi` / `action = none`，OQ8 未定案，T2–T4 都**不**转 `eligible`、`note` 也**不**置 `null`；其余条目在对应切片补齐后 `note` 置 `null`、`eligibility` 转 `eligible`）
- [ ] `src-tauri/src/tool_setup.rs`：**fail-closed 修正**——`spawn_result_cleanup` 读配置失败回退 `0`（禁用），不再回退常量 30 天（N6 / V12 前半）
- [ ] `src-tauri/src/tool_setup.rs`：把 `result_retention_days` 与注入时刻注入 `FileToolResultStorage`（`with_retention(days, since)`）——**`AppConf::load()` 只读一次**，同一个 `retention_days` 同时喂给 `spawn_result_cleanup` 与 `with_retention`（两次各读会得到不同的 `effective_since`，`E3` 的 `policy` 与启动清扫就会对不上）；管理器只读 owner 自述、**不**读配置（§3.5 硬约束 1）
- [ ] `src-tauri/src/adapters/tool_exec.rs`：新增 `pub fn storage(&self) -> &Arc<dyn ToolResultStorage>` 转发 `self.pipeline.storage()`（`ToolPipeline::storage()` 已存在，`PipelineToolExecution` 当前**没有**该访问器，不加就无法从 `app.state` 拿到 E3 的 owner）
- [ ] `src-tauri/src/cmd/storage.rs`：取 `E3` / `E4` 的 owner 入口——`E3` 经 `app.try_state::<Arc<PipelineToolExecution>>()` → `.storage()`；取不到 → `E3` `availability = notImplemented` + `availabilityReason` 写「owner 未装配」+ `bytes` / `count = null` + `eligibility = notImplemented` + `reason = noReclaimApi`（V17：方法不可达即不可执行）。`E4` 用 `AppDirs::indexes()` 传给公开的 `index_stats`；`AppDirs::resolve()` 失败 → 整命令 `Err`（与 `E1` 同级）。管理器**不得**自行拼受管目录路径（G1）
- [ ] `src-tauri/src/cmd/storage.rs`：把「13 条装配」抽成**纯函数** `pub fn assemble_breakdown(inputs: BreakdownInputs) -> StorageBreakdownDto`（`BreakdownInputs` 与字段同为 `pub`，**不能** `pub(crate)`——`tests/storage_breakdown.rs` 是 lib 的集成测试，跨 crate 只能看见 `pub`）——`BreakdownInputs` 只含普通数据（`sampled_at`、附件采样结果、各条 `Result<ProbeStats, ProbeError>`、`policy` 快照）；命令函数只负责取 state / 调 owner / 喂入参。**没有这一步，`tests/storage_breakdown.rs` 的 V1–V5 / V13 无法落地**（现有命令直接吃 `AppHandle`，伪造 owner 失败与「不注入 provider」都无从构造）

### T1-e 测试

- [ ] **可见性（测试的前置条件，先做）**：`src-tauri/src/lib.rs` 的 `mod cmd;` 改为 `pub mod cmd;`，`src-tauri/src/cmd/mod.rs` 的 `mod storage;` 改为 `pub mod storage;`——现有 `src-tauri/tests/*` 全部只 `use` 已 `pub` 的模块（`adapters` / `protocol` / `mcp` / `clock` / `app_dirs`），`assistant_app::cmd::storage::…` 当前**取不到**，不改则 `storage_breakdown.rs` / `storage_reclaim.rs` 无法编译。`a3_lib_rs_is_assembly_only` 只校验 `lib.rs` 行数与「不得内联 `generate_handler!`」，`pub` 化不影响；也**不要**为测试新建 `pub mod storage_manager` 门面（会多一层与规格 §3 的「管理器 = `cmd/storage`」不一致的别名）

- [ ] `src-tauri/crates/assistant-storage/tests/attachment_store.rs`：`derived_stats` 用例（空目录 → `count = 0` 且两端 `None`；符号链接不计入；`bytes` = 常规文件长度之和）
- [ ] `src-tauri/crates/assistant-tools/tests/storage.rs`：`result_stats` 与 `cleanup_expired_results` 判据一致（同目录先统计再清理，`removedBytes` 与统计值可对上）；`retention_days = 0` 时 stats 仍可出数但 `E3` 判 `policyDisabled`
- [ ] `src-tauri/crates/assistant-tools/tests/index_stats.rs`（新建）：`index_stats` 覆盖跨 revision 残留与符号链接跳过
- [ ] `src-tauri/src/cmd/mascot_asset.rs`（`mod tests`）：`asset_stats` 排除 `.tmp`、跳过子目录符号链接
- [ ] `src-tauri/src/setup.rs`：新增 `#[cfg(test)] mod tests`（当前没有），用例覆盖 `db_file_stats`——文件缺失返回 `NotFound`（由管理器翻译成 `missing` 计 0），且**不打开 DB**
- [ ] `src-tauri/tests/storage_breakdown.rs`（新建集成测试）：13 条 key 顺序（V1）；伪造 owner 失败 → `null` + `unreadable`（V2）；`action = none` 的条目 `eligibility ∈ {notEligible, ownerManaged, notImplemented, locked}`（V3；**`locked` 必列**——`E9` 锁定态与 `E7` 的 `metadata` 失败都会落在这里，V3 原句漏列会误判为违约）；`reason` / `note` 非空（V4）；账本恒等式（V5）
- [ ] `src-tauri/tests/storage_breakdown.rs`：注入假 `StorageOwnerProbe` 与**不注入**各跑一遍——不注入时 `E7`–`E11` 的 `availability = notImplemented`、`eligibility` 分别为 `notEligible`/`notEligible`/`notImplemented`/`ownerManaged`/`ownerManaged`（V13 后半）；两种构建 `entries` 恒 13 条、顺序不变
- [ ] 静态门禁：`cmd/storage.rs` 无 `fs::remove_*` / `read_dir` / 受管目录名字面量 / `AppConf::load()`（V10 / G1–G3）

### T1-f 对账余量（§3.11，只读旁路，T1 内**独立成链**）

> 与 T1-b/c/d **无依赖**（不读 owner 数字、不进 `entries`），但**必须**在 T1 内完成：`inventory` 是 `StorageBreakdownDto` 的必填字段（键恒在），晚做会让 T2–T4 的 golden 全部返工。

- [ ] `src-tauri/src/app_dirs/registry.rs`（新建）：按 §3.11.1 落 `RootScope` / `RootCoverage` / `RootMount` / `ManagedRoot` + `MANAGED_ROOTS`（13 条，顺序 `E1→E13`）+ `IGNORED_NAMES`；`app_dirs/mod.rs` 加 `pub mod registry;`（`cmd/storage` 与集成测试都要用，**不能** `pub(crate)`）；`dir_name` 改为 `pub(crate) mod dir_name;`（常量仍 `pub`——全仓库 `dir_name::` 引用只在 `app_dirs` 内，已核对 11 处）
- [ ] `src-tauri/src/app_dirs/dir_name.rs`：新增 `pub const ALL: &[&str]`（常量显式清单，V19 靠它枚举；新增常量必须同时进 `ALL`，否则 V19 看不见）
- [ ] `src-tauri/src/app_dirs/registry.rs`：新增 `pub fn validate_registry(roots: &[ManagedRoot], dir_names: &[&str]) -> Result<(), String>`——纯函数、**接受切片**（测试才能传缺项负例），校验项即 V19 的三条
- [ ] `src-tauri/src/app_dirs/roots.rs`：7 个 getter 改为按 key 从 `MANAGED_ROOTS` 取 `segments` + `scope` 派生；拼接仍留在 `app_dirs`（G1 不变），签名与返回值零改动（现有单测继续绿）
- [ ] `src-tauri/src/app_dirs/registry.rs`（`#[cfg(test)] mod tests`）：V19 单测（正例 + 「缺登记」「缺常量」两个负例）；**必须放 crate 内**——`dir_name` 收紧后 `tests/` 集成测试取不到
- [ ] `src-tauri/src/protocol/dto/storage.rs`：按 §6.2 新增 `StorageInventoryStatusDto` / `StorageInventoryClassDto` / `StorageInventoryNodeDto` / `StorageInventoryItemDto` / `StorageInventoryDto`；`StorageInventoryDto` 实现 `Default`（`status = NeverRun`），`StorageBreakdownDto` 追加 `#[serde(default)] pub inventory`（键恒在，旧夹具不炸）
- [ ] `src-tauri/src/cmd/storage/inventory.rs`（新建，**文件头注释写明「G2 唯一例外」**）：`pub fn scan_inventory(dirs: &AppDirs, mounted: &dyn Fn(&str) -> bool, now: i64) -> StorageInventoryDto`——分类、遍历、有界、只读全部按 §3.11.2；`scanBytes` 与 `scanCount` 必须同一次遍历产出（分两次会漂移）；禁止 `remove_*` / `fs::write` / `create(` / `log::`
- [ ] `src-tauri/src/cmd/storage/inventory.rs`：`pub fn subtract(nodes: &mut [StorageInventoryNodeDto], declared: &HashMap<&str, (Option<u64>, Option<u64>)>)`——纯函数、无 IO，规则按 §3.11.3（含 `null` 传染与 `overRegistered`）
- [ ] `src-tauri/src/cmd/storage/inventory.rs`：`InventoryCache`（`Mutex<Snapshot>`，进程内不落盘）按 §3.11.4 实现五态 / TTL / single-flight / 失败保留；`pub fn trigger_scan(app: &AppHandle, cache: &InventoryCache, force: bool)` 在 `running` 时直接返回，否则 spawn：`spawn_blocking(scan_inventory)` → 写快照 → `emit_broadcast`
- [ ] `src-tauri/src/setup.rs`：`handle.manage(InventoryCache::default())`（与 `DbState` 同处装配；命令用 `State` 取，**不用**全局静态——测试需要可重置的独立实例）
- [ ] `src-tauri/src/protocol/events.rs`：新增 `pub const STORAGE_INVENTORY_UPDATED: &str = "storage:inventory-updated";`，广播复用已有 `emit_broadcast`
- [ ] `src-tauri/src/cmd/storage/mod.rs`：`get_storage_breakdown` 只读快照、按 §3.11.4「读路径零等待」触发，**禁止**在命令内 `await` 扫描；`BreakdownInputs` 增 `inventory` 字段（`assemble_breakdown` 只搬运，减法留在 `inventory.rs`）
- [ ] `src-tauri/src/cmd/storage/mod.rs`：新增 `#[tauri::command] pub fn refresh_storage_inventory(...) -> StorageInventoryStatusDto`——签名与边界按 §6.1 第 9 条（只回状态、不回数字）
- [ ] `src-tauri/src/ipc.rs`：`generate_handler![...]` 追加 `cmd::refresh_storage_inventory`（核心注册表、无 `#[cfg]`、不动 permission / `capabilities/*.json`，理由同 T2-a 末条）
- [ ] `src-tauri/tests/storage_inventory.rs`（新建）：V18（分类互斥穷尽 + `declared` 节点减法 + `overRegistered` 触发）、V20 前半（`running` / `failed` 两态下拉 breakdown，`entries` 与 `totalBytes` 逐字段不变）、有界（触界 → `partial` + `truncatedNodes`，其余节点仍出数）、`failed` 保留上次成功数字；V20 后半（白名单负例）留给 T2-d
- [ ] 静态门禁：`read_dir` 只允许出现在 `cmd/storage/inventory.rs`（`mod.rs` / `probe.rs` / `reclaim.rs` 出现即红）；`inventory.rs` 内检索 `remove_file` / `remove_dir` / `remove_dir_all` / `fs::write` / `create(` / `log::` 应为空

---

## 阶段 T2 — plan / execute 骨架 + 低风险回收（`E2` / `E4`）

退出条件：V6–V10 通过；单条失败隔离；白名单外请求被拒。

### T2-a 骨架

- [ ] `src-tauri/src/cmd/storage/reclaim.rs`：`plan` / `execute` 抽成**不依赖 `AppHandle`** 的纯函数 `pub async fn plan_storage_reclaim(ports: ReclaimPorts<'_>, req: StorageReclaimPlanRequestDto) -> Result<StorageReclaimPlanDto, StorageReclaimErrorDto>` / `pub async fn execute_storage_reclaim(ports: ReclaimPorts<'_>, req: StorageReclaimRequestDto) -> StorageReclaimResultDto`（`ReclaimPorts<'a> { plans: &'a ReclaimPlanStore, db: Option<Arc<Mutex<Connection>>>, probe: Option<&'a dyn StorageOwnerProbe>, attachments: Option<&'a AttachmentStore>, tool_results: Option<&'a dyn ToolResultStorage>, now: i64 }`，全部 `pub`，**不能** `pub(crate)`）；命令体只做 `app.state` 取句柄（`db` 用 `state.0.clone()`）+ 调用。两条硬理由：①集成测试 `tests/storage_reclaim.rs`（V6–V12）拿不到 `AppHandle`；②`db` 必须是 `Arc<Mutex<Connection>>` 而**不是** `&Connection`——后者非 `Send`，会让 `async fn` 的 future 编译不过。锁只在**同步块**内取用，`await` 前必须 `drop`（`clippy::await_holding_lock` + 锁顺序「DB → 附件 ID」）。**取锁的错误文案**：`cmd/mod.rs` 的 `get_db_conn` 只接受 `tauri::State`，纯函数用不上；抽 `pub(crate) fn lock_conn(conn: &Mutex<Connection>) -> Result<MutexGuard<'_, Connection>, String>`（`error.db_lock` 文案唯一来源），`get_db_conn` 改为转调它，两处文案不得各写一份

- [ ] `src-tauri/src/protocol/dto/storage.rs`：新增 `StorageReclaimPlanRequestDto`（`entryKeys`，`deny_unknown_fields`）/ `StorageReclaimPlanDto` / `StorageReclaimPlanEntryDto` / `StorageReclaimRequestDto`（`planId` + `entries` + `confirmed`）/ `StorageReclaimEntryRequestDto`（仅 `key` + `action`）/ `StorageReclaimResultDto` / `StorageReclaimEntryResultDto` / `StorageReclaimErrorDto`，字段与 §6.2 一致；输入侧 `Option` 用 `#[ts(optional = nullable)]`
- [ ] `src-tauri/src/cmd/storage/`（或 `cmd/storage.rs` 内）：新增计划缓存状态 `ReclaimPlanStore`（进程内 `Mutex<HashMap<planId, CachedPlan>>`，不落盘），`CachedPlan` 含 `sampledAt` / 条目集合 / `policy` 快照 / `createdAt`
- [ ] `src-tauri/src/cmd/storage/`：`planId` 生成（不透明、不可解析）；**10 分钟**有效期；过期 → `planExpired`、不存在 → `planNotFound`，零副作用（§5.4）
- [ ] `src-tauri/src/cmd/storage/`：单实例串行——同一时刻只允许一个 `reclaim_storage`，第二个返回 `busy`（不排队、不合并；**禁止**全局回收锁，G11）
- [ ] `src-tauri/src/cmd/storage.rs`：`get_storage_reclaim_plan`——自己重新采样（不复用上次 snapshot 数字）；`entryKeys` 空 = 全部 13 条，未知 key 整体拒绝；`totalReclaimableBytes` 只累加 `eligible` 且 `complete = true` 时给数，否则 `null`（G10）
- [ ] `src-tauri/src/cmd/storage.rs`：`reclaim_storage`——校验 `confirmed = true`、`entries` 非空、每项 `key + action` 既在计划内**又在** §3.3 白名单（`E1+A1` / `E2+A5` / `E3+A3` / `E4+A5` / `E8+A4`），任一不符**拒绝整个请求**，不做部分执行（§6.1 第 3 条）
- [ ] `src-tauri/src/cmd/storage.rs`：执行时**重新校验资格**（引用重算 / 重新 stat / 重新计数），**禁止**按计划里的数量删（§5.2 第 4 条）；计划后被改动的项计入 `skippedCount`，不计失败
- [ ] `src-tauri/src/cmd/storage.rs`：结果组装——`removedCount` / `removedBytes` 只记实际删除；`failedCount` 与 `skippedCount` 分开；`errors` 最多 5 条且 `message` 不含绝对路径与文件名；单条失败不中断其余条目（V9）
- [ ] `src-tauri/src/cmd/storage.rs`：留痕日志一条结构化 `log::info!`（`planId` / key / action / `removedCount` / `removedBytes` / `failedCount` / `skippedCount` / `sampledAt`），**禁止**写路径与文件名（G12 / V15）
- [ ] `src-tauri/src/ipc.rs`：在 `generate_handler![...]` 里按现有顺序追加 `cmd::get_storage_reclaim_plan` / `cmd::reclaim_storage`（纯核心注册表，**不得**出现 `#[cfg]`）。**核心命令没有 ACL 条目**：`build.rs` 只从 `src/*_plugin.rs` 派生插件命令，`assembly_contract::a4` 也只比对「插件注册表 ↔ ACL」，所以**不要**新建 permission 文件、不要改 `capabilities/*.json`（改了反而会让 `b10_feature_capabilities_are_minimal` 失配）
- [ ] `src-tauri/tests/protocol_bindings.rs`：全部新 DTO 登记

### T2-b `E2`（`attachments/derived`，`A5`）

- [ ] `src-tauri/crates/assistant-storage/src/attachment_store.rs`：新增 `pub async fn list_derived(&self) -> Result<Vec<DerivedEntry>, AttachmentError>`（`DerivedEntry { name, bytes, modified_at, orphan }`，`name` 是 `derived/` 下的**文件名**）——**孤儿判定留在 owner 内**：内部先 `list_all()` 取基附件的 `file_stem` 集合，再扫 `derived/` 的 `{stem}.{EXTRACTOR_VERSION}.txt`，`stem` 不在集合内即 `orphan = true`（基附件扩展名不同但 `stem` 相同仍算存在，这是 `derived_path` 丢扩展名的必然结果）；`DERIVED_DIR` / `EXTRACTOR_VERSION` / 拼接规则**不**外泄给管理器（G1）
- [ ] `src-tauri/crates/assistant-storage/src/attachment_store.rs`：新增 `pub async fn purge_derived_orphans(&self, names: &[String], sampled_at: i64) -> DerivedPurgeReport`（`removed` / `removedBytes` / `failed` / `skipped`）——入参是**派生文件名**（不是基附件 id，`delete_derived_text(base_id)` 在这里用不上），逐个 `ensure_plain` 后拼 `derived/{name}`；删除前 `metadata` 确认存在且为常规文件，已不存在 → `skipped`（幂等）；**`mtime` 不早于 `sampled_at` → `skipped`**（§5.2 第 5 条：采样后新生成 / 被改写的派生文件不进本次回收，否则「计划时无孤儿、执行时删新文件」）；顺带清 `derived/*.txt.tmp` 伴生残留；只删常规文件、跳符号链接、失败只计数
- [ ] `src-tauri/src/cmd/storage.rs`：`E2` 的 `eligibility` **T2 起恒为 `eligible`**（§3.1 终态表「T2 后 → `eligible`」+ V17：`E2+A5` 在白名单且 `list_derived` / `purge_derived_orphans` 均存在 ⇒ 必 `eligible`；**不得**因「当前无孤儿」降级为 `notEligible`，`reason` 词表里也没有对应词）；`reason` **固定填 `derivedCache`**（§3.1 唯一解表；`eligible` 时本可为 `null`，但这里统一填该值，禁止实现者自行选 `orphan` 或 `null`）；`reclaimableCount` = `orphan = true` 的条数（可为 0）、`reclaimableBytes` = 这些条的字节和，且 `≤ bytes`（V17 前半）；**执行时重跑** `list_derived()` 取当前孤儿名单（不信任计划里的名单，§3.3 动作边界 2 / §5.2 第 4 条）
- [ ] `src-tauri/src/cmd/storage.rs`：`E2` 执行走 owner 方法；管理器**不得** `fs::remove_file`（G3）

### T2-c `E4`（`tool-index`，`A5`）

- [ ] `src-tauri/crates/assistant-tools/src/os/tools/super_index/`：新增公开 `pub fn purge_index_files(root: &Path) -> IndexPurgeReport { removed, removedBytes, failed, skipped }`——落盘索引是 `index_root` 下的**单层** `<digest>_<safe_revision>.idx`（`format::index_file_name`），因此只删该层 `*.idx` 常规文件，跳符号链接 / 非常规文件 / 子目录；**不提供**「按 key 删」的入口（`E4` 是 `A5` 全量重建缓存，没有候选名单，也不需要把文件名外泄给管理器）；内存 LRU 不参与统计、不因删除而失效（后续 miss 会重建，非错误）
- [ ] `src-tauri/crates/assistant-tools/src/os/tools/super_index/`：`index_stats` 与 `purge_index_files` **共用同一遍历规则**（同层 / 同后缀 / 跳符号链接 / 跳非常规文件），否则「统计 N 字节、清理 0 字节」的漂移必然出现（§3.10 接口形状 2）；单文件 64 MiB 上限只在 `note` 展示，不填 `maxBytes`
- [ ] `src-tauri/src/cmd/storage.rs`：`E4` 的 `eligible` **恒成立**（缓存可重建，`reason = derivedCache`；`reclaimableCount = count` 可为 0），`confirmation = confirm`；`policy.source = owner`

### T2-d 测试

- [ ] `src-tauri/tests/storage_reclaim.rs`（新建）：V6——`get_storage_reclaim_plan` 前后目录字节与 mtime 不变
- [ ] `src-tauri/tests/storage_reclaim.rs`：V8 四组负例——`confirmed = false` / `planId` 过期 / `entries` 为空 / `action` 与计划不符，全部拒绝且零副作用
- [ ] `src-tauri/tests/storage_reclaim.rs`：白名单负例——`E3+A3`（T2 未接入）与 `E10+A3` / `E11+A3` / `E13+A4` 请求被拒
- [ ] `src-tauri/tests/storage_reclaim.rs`：V20 后半——请求 `key = <某个 unregistered 节点名>`（以及 `"inventory"` 本身）被拒，零副作用
- [ ] `src-tauri/tests/storage_reclaim.rs`：V9——注入 owner 失败，其余条目仍完成，`status` 如实为 `done/partial/failed/skipped`
- [ ] `src-tauri/tests/storage_reclaim.rs`：`busy`——并发两次执行，第二次零副作用
- [ ] `src-tauri/crates/assistant-storage/tests/attachment_store.rs`：`purge_derived_orphans` 幂等（重复执行第二次全 `skipped`，不报错）
- [ ] `src-tauri/crates/assistant-tools/tests/index_stats.rs`：`purge_index_files` 幂等 + 只删缓存文件
- [ ] golden 断言：T2 结束时 `E2` / `E4` 为 `eligible`（`confirm`），其余按 §11 切片期表（V17）

---

## 阶段 T3 — 中风险回收（`E3` / `E8` / `E1`）

退出条件：V7、V9、V11、V12 通过；并发用例通过。

### T3-a `E3`（`tool-results`，`A3`）

- [ ] `src-tauri/crates/assistant-tools/src/runtime/storage.rs`：新增 `pub fn collect_expired(root, retention_days) -> Vec<ResultCandidate { path, bytes }>`——**复用 T1 的 `for_each_file`** 与 `cleanup_expired_results` 同一判据（`mtime` 早于「今天 − 保留天数」、只常规文件、跳符号链接）
- [ ] `src-tauri/crates/assistant-tools/src/runtime/storage.rs`：新增 `pub fn delete_candidates(candidates) -> ResultPurgeReport { removed, removedBytes, failed, skipped }`——**禁止**复用 `CleanupReport` 后自己补算字节（删完取不到长度，§3.10 接口形状 1）
- [ ] `src-tauri/crates/assistant-tools/src/runtime/storage.rs`：`mtime` 在未来（时钟偏移）时按「未过期」处理（§5.5）
- [ ] `src-tauri/crates/assistant-tools/src/runtime/storage.rs`：`ToolResultStorage` trait 增 `fn collect_expired(&self) -> Option<...>` 与 `fn delete_candidates(&self, ...)`（默认 `None` = 不提供）；`FileToolResultStorage` 覆盖
- [ ] `src-tauri/src/cmd/storage.rs`：`E3` 白名单组合 `E3 + A3` 打开；`retentionDays = 0` → `notEligible` + `policyDisabled`（**不得**翻译成「无保留期」，§3.5 硬约束 2）
- [ ] `src-tauri/src/cmd/storage.rs`：执行时重新 `collect_expired`（不复用计划候选）；计划后被写入的新文件天然不满足判据

### T3-b `E8`（`security/backups`，`A4`）

- [ ] `src-tauri/crates/assistant-vault/src/security_db/state.rs`：现有 `prune_backups(dir, keep) -> Result<()>` 为 `pub(super)` 且**不报字节**；新增 `pub fn prune_backups_to(db_path: &Path, keep: usize) -> Result<PruneReport { removed, removedBytes, failed }, VaultError>`（内部复用 `backup_dir_for` + `BACKUP_PREFIX` / `BACKUP_SUFFIX` 匹配），并把 `MAX_BACKUPS` 暴露给 provider 可达的常量入口
- [ ] `src-tauri/crates/assistant-vault/src/security_db/state.rs`：`keep` **至少 1**，**不得删最新一份**；只按 `BACKUP_PREFIX` / `BACKUP_SUFFIX` 命名匹配，跳符号链接
- [ ] `src-tauri/src/cmd/storage/probe.rs`：`StorageOwnerProbe` 增回收方法 `fn prune_backups(&self, keep: u32) -> ...`（返回值只用普通数据；vaultless 分支自然不提供 → `E8` 保持 `notImplemented` + `noReclaimApi`）
- [ ] `src-tauri/src/vault_plugin.rs`：`VaultProbe` 实现该回收方法
- [ ] `src-tauri/src/cmd/storage.rs`：`E8` 白名单组合 `E8 + A4` 打开；`备份数 > MAX_BACKUPS(5)` → `eligible` + `overLimit` + `confirm`；`≤ 5` → `notEligible` + `reason = overLimit`（§3.1 E8 行只给这一个词，`reason` 仅 `notEligible` 时强制非空，语义按「已在上限内、无可裁剪」读）；`autoActive = true` 不变；备份计数走目录列举，**不**依赖 vault 解锁，故 `locked` 态下 `E8` 仍可 `eligible`
- [ ] `src-tauri/src/cmd/storage.rs`：`E8` 执行**必须重新计数**再裁剪（§5.2 第 4 条），并且只删「按时间排序后最旧的超限部分」、恒保留最新 `keep = 5` 份（§5.5 E8 行「先创建、后裁剪；不得删最新一份」）；计划后被新增的备份只会把更旧的挤出去，**不会**被删——因此无需额外比对 `sampledAt`；`keep` 由 owner 侧常量 `MAX_BACKUPS` 传入，**不得**在管理器里硬编码 5

### T3-c `E1`（`attachments`，`A1`）

- [ ] `src-tauri/crates/assistant-storage/src/attachment_store.rs`：新增**按附件 ID 分片**的同步锁 `locks: Arc<[std::sync::Mutex<()>; 64]>`（`AttachmentStore` 已是 `#[derive(Clone)]` + `Arc` 共享，锁数组必须同样进 `Arc`，否则每个 clone 各拿一把锁、串行化失效），提供 `fn id_guard(&self, id: &str) -> std::sync::MutexGuard<'_, ()>`（对 `id` 的稳定哈希取模选桶；桶碰撞只增加争用，不破坏「同一 ID 必互斥」）。**不能用 `tokio::sync::Mutex`**：引用写入路径 `insert_attachment_refs` 在 `cmd/chat/insert.rs:330` 是**同步**函数（已持 `&Connection`，其调用者 `cmd/agent/mod.rs` 在 async 命令内、无法 `.await`），`blocking_lock()` 在运行时线程内会 panic
- [ ] `src-tauri/crates/assistant-storage/src/attachment_store.rs`：`id_guard` 的**临界区内禁止 `.await`**（否则 `clippy::await_holding_lock` + 跨 await 持锁），只用同步 IO（`std::fs::remove_file` / `std::fs::metadata`）与同步 DB 调用；为此新增同步变体 `pub fn delete_blocking(&self, id: &str) -> Result<Option<u64>, AttachmentError>`（`Ok(Some(len))` = 已删并返回字节、`Ok(None)` = 文件不存在（`ErrorKind::NotFound`，不是错误）、非法 id / 其它 IO 错 = `Err`；顺带清 `derived/{stem}.extract-v1.txt`），既有 `async fn delete` 保留不动（其它调用点零改动）
- [ ] 三处调用点共用 `id_guard`（§5.5 第 1–2 行）：`cmd/chat/insert.rs:330`（`insert_attachment_refs` 写引用）、`cmd/storage.rs:130`（`delete_attachments` 的「删引用 + 删文件」）、`A1` 的 `delete_orphans_by_ids`（删文件）。**`insert_attachment_refs` 现在是自由函数且只收 `conn`**（`attachment_store.rs:264`，全仓库**只有 `cmd/chat/insert.rs:330` 一个调用点**），必须改签名为 `pub fn insert_attachment_refs(store: &AttachmentStore, conn: &Connection, attachment_ids: &[String], conversation_id: &str, message_id: &str) -> Result<(), String>`：逐 id 取 `id_guard` → `symlink_metadata` 确认文件仍在（不在 → 返回 `Err`，让整条写入失败，**不**落悬空引用）→ 再 `INSERT OR IGNORE`。**锁顺序恒为「DB 连接锁 → 附件 ID 锁」**（`insert_chat_messages` 已先持 `conn`，A1 也先取 `conn`），反向取锁会 ABBA 死锁。**`save_attachment` 不持锁**：它是内容寻址 `create_new(true)` 写、本身幂等，且 `save_attachment` 是 `async`（tokio fs），持 std 锁会跨 await；它与删除的竞态由上面的锁内存在性校验兜底——先 insert 则引用已落库、A1 查库时跳过；先删则 insert 校验失败并报错，两种情况都不会留下悬空引用
- [ ] `src-tauri/src/cmd/storage.rs:130`：`delete_attachments` 改造为「取 `conn` → 逐 id 取 `id_guard` → 删引用（`delete_attachment_refs_by_attachment_id`）→ `delete_blocking` 删文件」，**删掉**现有「先 `list_all()` 建 `size_map`」那一步，字节直接用 `delete_blocking` 的返回值（顺带去掉一次全目录扫描；`removed_count` / `removed_bytes` 语义与 DTO 不变，G13）；`AttachmentDeleteResultDto` 字段一个不改
- [ ] `src-tauri/src/cmd/storage.rs:130`：`delete_attachments` 的「文件不存在」语义保持现状——`delete_blocking` 返回 `Ok(None)` 时**既不计 `removed` 也不计失败**（现状是 `delete()` 的 `NotFound` 走 `log::warn` 后不计入），**不得**改成计 `removed`（会虚增字节）或计失败（会改变前端明细区行为，G14）；id 预校验（`is_plain_file_name` → `Err`）原样保留
- [ ] `src-tauri/crates/assistant-storage/src/attachment_store.rs`：新增 `pub fn delete_orphans_by_ids(&self, conn: &Connection, ids: &[String], sampled_at: i64) -> OrphanPurgeReport { removed, removedBytes, failed, skipped }`——**同步方法**（锁内要查 DB，不能 async），**方法名与 §3.1 末条 `note` 点名的缺口一致**（T1 的 `note` 就写这个名字，T3 补上后 golden 才逐字对得上）；每个 id 的处理固定为：取 `id_guard` → **在锁内**做单 id 引用存在性查询（`SELECT 1 FROM attachment_refs WHERE attachment_id = ?1 LIMIT 1`，走已有索引 `idx_attachment_refs_att`，见 `db.rs:213`）→ 仍被引用 → `skipped` → `metadata` 确认存在且为常规文件（不存在 → `skipped`）→ **`mtime` 不早于 `sampled_at` → `skipped`**（§5.2 第 5 条，挡住「采样后被导入、引用尚未落库」的窗口）→ `delete_blocking` → 累加 `removedBytes`；ID 非法 → `skipped`；单条失败只计 `failed` 不中断
- [ ] `src-tauri/src/cmd/storage.rs`：`E1` 执行顺序固定为「取 `conn` → 调 `delete_orphans_by_ids(conn, 重算的孤儿 ids, plan.sampledAt)`」，引用重算在 owner 的锁内完成，**禁止**信任计划里的孤儿列表（计划只给数量与字节，§3.3 动作边界 2 / §5.2 第 4 条）
- [ ] `src-tauri/src/cmd/storage.rs`：`E1 + A1` 打开后 `confirmation = strongConfirm`（风险档由条目决定，不由动作决定，§3.6 硬约束 6）；有孤儿 → `eligible` + `reason = orphan`；无孤儿 → `notEligible` + `reason = referenced`
- [ ] `src-tauri/src/cmd/storage.rs`：`A2` **不**经 `reclaim_storage`（继续走既有 `delete_attachments`，语义零改动）

### T3-d 测试

- [ ] `src-tauri/tests/storage_reclaim.rs`：V7——`expectedBytes` 与 `removedBytes` 偏差可解释；不可回收条目 `expectedCount` / `expectedBytes = null`
- [ ] `src-tauri/tests/storage_reclaim.rs`：V11——`E1` 的 `A1` 与 `delete_attachments` 并发，无「删了仍被引用」，同一 ID 串行
- [ ] `src-tauri/tests/storage_reclaim.rs`：V12——`E3` 配置读失败回退 `0` 不删；`E11` 无游标时 `eligibility ≠ eligible`
- [ ] `src-tauri/tests/storage_reclaim.rs`：`E8` 裁剪不删最新一份、`keep = 1` 时只留 1 份、失败保留多余备份并计 `failed`
- [ ] `src-tauri/crates/assistant-tools/tests/storage.rs`：`collect_expired` 与 `cleanup_expired_results` 候选集合逐条一致；`mtime` 在未来的文件不进候选
- [ ] golden 断言：T3 结束时 `E1` / `E3` / `E8` 按 §11 切片期表进入 `eligible`（`strongConfirm` / `confirm` / `confirm`）

---

## 阶段 T4 — 前端与收尾

前置：I54 `R0` 已交付（页面容器 + 三 tab 挂载位）。R0 未开工前，T4 只能先做与容器无关的组件内部改造，**不得**自行新建第二个页面。

### T4-a 数据与交互

- [ ] `src/services/desktop/storageService.ts`：**扩展现有 service**（不新建 store、不新建 service）——加 `getStorageReclaimPlan(entryKeys)` 与 `reclaimStorage(planId, entries, confirmed)`，以及 `refreshStorageInventory()`（返回 `StorageInventoryStatusDto`，只取 `status`，**不取数字**）
- [ ] `src/settings/pages/StorageSettings.tsx`：**保留附件明细区全部既有能力**（会话分组 / 孤儿分组 / 折叠 / 组全选 / 勾选删除，服务 `A2`），在其后**追加** 13 行条目表（G14 / V16）
- [ ] `src/settings/pages/StorageSettings.tsx`：条目行渲染名称、字节、条数、`availability`、`reclaim.eligibility` 徽标、`reclaim.action` 按钮、策略摘要；`bytes = null` 显示占位符（**不得**直接传 `formatBytes`，N17），不改 `src/utils/formatBytes.ts` 的签名
- [ ] `src/settings/pages/StorageSettings.tsx`：不可回收条目按钮禁用并显示 `reason` 文案；`notImplemented` 额外显示 `note`（「缺少 owner 方法」）
- [ ] `src/settings/pages/StorageSettings.tsx`：回收流程固定为 `get_storage_reclaim_plan` → 确认（`strongConfirm` 需二次输入条目名或等价强交互）→ `reclaim_storage` → 结果 toast → 重新 `get_storage_breakdown`
- [ ] `src/settings/pages/StorageSettings.tsx`：**禁止**前端自算「可回收字节 / 是否安全」，**禁止**按 `expectedBytes` 自行降档确认强度（档位只信后端 `confirmation`）
- [ ] `src/settings/pages/StorageSettings.tsx`：`planExpired` / `planNotFound` / `busy` 提示重试，**不自动重放**；`complete = false` 时不展示跨条目合计（G10）
- [ ] `src/settings/pages/StorageSettings.tsx`：13 行条目表**不**提供第二套「删指定附件」入口，也**不得**把明细区删除按钮搬进条目表
- [ ] `src/settings/pages/StorageSettings.tsx`：**第三段只读余量区**（`breakdown.inventory`）——五态渲染、`partial` / `overRegistered` 提示、列示范围与 `declared` 折叠条件全部按 §6.3 第 9 条；**禁止**任何删除 / 清理按钮，**禁止**把余量数字加进页面任何合计（G10）
- [ ] `src/settings/pages/StorageSettings.tsx`：`useEffect` 内 `listen("storage:inventory-updated", ...)`（与 `src/vault/core/hooks/useVaultEvents.ts` 同款写法，清理函数里调 `UnlistenFn`），收到后**只重拉一次** `getStorageBreakdown`，**不新增轮询**；「重新扫描」按钮调 `refreshStorageInventory()` 后按返回的 `status` 刷新按钮态
- [ ] `src/settings/styles/settings.css`：新增条目表样式（与既有 `storage-*` 类名同族，不重命名既有类）＋ 余量区样式（含 `overRegistered` 警示态）
- [ ] `src/i18n/locales/zh-CN.json` / `en-US.json`：新增条目名、`availability` / `eligibility` / `reason` 词表、策略摘要、确认文案（`strongConfirm` 必须含条目 / 数量 / 字节三件套）、结果 toast 文案，以及余量区文案（覆盖 §6.3 第 9 条涉及的全部提示）；两份 key 结构必须一致

### T4-b 收尾

- [ ] `npm run contract:gen` 后确认 `src/types/protocol.contract.ts` 无手改、无残留
- [ ] 留痕复核：`execute` 日志只含 `planId` / key / action / 计数 / `sampledAt`（V15）
- [ ] 全量回归：`cargo check --workspace --all-targets`、`cargo test -p assistant-app --lib`、`cargo test -p assistant-storage`、`cargo test -p assistant-tools`、`cargo test --test storage_breakdown`、`cargo test --test storage_inventory`、`cargo test --test storage_reclaim`、`cargo test --test protocol_bindings`、`cargo test --test app_dirs_contract`、`cargo test --test assembly_contract`
- [ ] vaultless 构建再跑一遍：`cargo test --no-default-features --test storage_breakdown`（`entries` 恒 13 条；`E7`–`E11` 的 `availability = notImplemented`，`eligibility` 分别为 `notEligible`/`notEligible`/`notImplemented`/`ownerManaged`/`ownerManaged`）
- [ ] 前端：`npm run typecheck`、`npm run lint`、`npm run test`
- [ ] 附件明细区能力逐项回归（V16 的手动清单）：会话分组 / 孤儿分组 / 折叠 / 组全选 / 单选删除 / 删除结果提示
- [ ] I61 §12 迁移清单复核：I54 中存储 tab 的残留条目已全部改为指针，且**未**删除任何共享 DTO 旧字段

---

## 门禁自查表（G1–G14，代码审查逐条签字）

| # | 检查点 | 自查方式 |
|---|---|---|
| G1 | 管理器无受管目录名 / 无路径拼接；`cmd/storage.rs` 无 `assistant_vault` / `assistant_security` / `#[cfg(feature)]` | 对 `src-tauri/src/cmd/storage.rs`、`src-tauri/src/cmd/storage/` 检索 `assistant_vault`、`assistant_security`、`cfg(`、`join(` |
| G2 | 管理器无 `read_dir` 遍历受管目录；**唯一例外** `cmd/storage/inventory.rs`（边界见 §3.11.2 第 1 条） | 检索 `read_dir`，只允许命中 `inventory.rs`；再在 `inventory.rs` 检索 `remove_*` / `fs::write` / `log::`，应为空 |
| G3 | 管理器无 `fs::remove_*` | 检索 `remove_file` / `remove_dir` / `remove_dir_all` |
| G4 | 每条目都有 owner getter + stats 入口（`E12` 明确记为无入口）；登记表 `MANAGED_ROOTS` 是 getter 唯一来源 | 对照 §3.10 表逐条核对 + V19 单测 |
| G5 | fail-closed：策略读不到 / 判据推不出 / 非普通文件 / 符号链接 → 不删 | owner 单测覆盖符号链接与非常规文件 |
| G6 | 取不到数 `null` + `availability` + `reason`；`missing` 计 `0` | `storage_breakdown` 单测 |
| G7 | 观测 / 计划 / 执行三步分离；计划无副作用；执行带 `planId` + `confirmed` | V6 / V8 |
| G8 | 不自动回收 `E1` / `E5` / `E6` / `E7` / `E9` | 白名单 + `autoActive` 快照 |
| G9 | 回收后重新采样，账本恒等式仍成立 | 执行后 `get_storage_breakdown` 断言 |
| G10 | 不把 13 条相加成「总占用」；只允许附件账本与 `totalReclaimableBytes` 两处合计；余量数字不进任何合计、不回流 `entries` | DTO / UI 复核 + V20 |
| G11 | 无全局回收锁；只串行化自己的执行请求 | 计划缓存状态复核 |
| G12 | 留痕无绝对路径 | 日志断言 |
| G13 | 既有 wire 字段零改动 | `protocol.contract.ts` diff 只增不改 |
| G14 | 前端附件明细区能力全保留 | T4-b 手动回归 |

---

## 验收判据归属

| 判据 | 切片 | 说明 |
|---|---|---|
| V1 / V2 / V3 / V4 / V5 | T1 | 13 行 key / `null` 语义 / `action = none` 约束 / `reason`·`note` / 账本恒等式 |
| V6 / V8 / V9 / V10 | T2 | 计划无副作用 / 四组负例 / 失败隔离 / 静态门禁 |
| V7 / V11 / V12 | T3 | 预期与实测偏差 / `E1` 并发 / fail-closed 与游标 |
| V13 | T1 + T4 | `vault` / `vaultless` 两种构建各跑一遍 |
| V14 | T1 / T2 / T4 | 每次改 DTO 都跑 `contract:gen` |
| V15 | T2 / T4 | 留痕字段与不含路径 |
| V16 | T4 | 超集不窄化（后端全绿 + 前端手动回归） |
| V17 | T1–T4 | 每个切片结束都用 golden 对一遍切片期资格表 |
| V18 / V19 | T1 | 余量守恒与互斥 / 登记表双向一致（判据见 §10） |
| V20 | T1 + T2 + T4 | 余量不影响条目（T1 字段比对、T2 白名单负例、T4 前端无合计） |

---

## 明确不改动项（避免顺手扩大范围）

1. `StorageBreakdownDto` 既有 4 字段、`AttachmentSummary` / `ConversationAttachmentSummary` / `AttachmentDeleteResultDto` / `AssetDirsDto` / `AttachmentLimitsDto` / `AttachmentImportDto`：键名 / 类型 / 含义全部原样（G13）。
2. `delete_attachments` / `open_attachment` / `import_attachment` / `get_asset_dirs` / `get_attachment_limits`：签名与语义不变。
3. `E11` 的审计裁剪**不**加手动入口；`prune_before` 只由 owner 带补录游标调度。
4. `E10` 的 `cleanup_recovery_dir` **不**加手动入口（避免删进行中的恢复）。
5. `E5` 的 `.tmp` 清扫留在 owner 的 `atomic_write` 路径，**不是**管理器动作。
6. `E13` 的 `prune` 本期不做（OQ8 未定案），只补 stats。
7. `logs/` 仍由 `tauri-plugin-log` 轮转，管理器只展示（`A6`），不调用插件内部 API。
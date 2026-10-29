# 死代码补充审计 + 时间戳单位审计

> 审计日期：2026-10-29
> 审计方法：
> 1. **死代码**：提取 `assistant-security` / `assistant-sync` 全部 `pub (async) fn`、`pub struct/enum/const`、`pub trait`，在全仓 `.rs`（含 `tests/`）中做标识符精确计数，出现次数 `≤ 1` 即判定为「仅定义、无引用」；前端对 `vault.service.ts` 的每个方法名在 `src/` 中统计 `.name` 调用数，`= 0` 即判定为死方法。
> 2. **时间戳**：对全仓 `timestamp()` / `timestamp_millis()` / `%ct` / `%3f` / `Date.now()` / `toISOString()` / `* 1000` / `/ 1000` 做全量枚举，逐个回溯「生产者 → 存储 → 传输 → 消费」链路，判定单位。
> 涉及 crate：assistant-security、assistant-sync、assistant-crash、src-tauri/src/cmd/vault
> 涉及前端：vault.service.ts、vault.types.ts、VaultEntryHistory.tsx、VaultConflictResolve.tsx、VaultSyncSettings.tsx、VaultList.tsx、vaultRevealSlice.ts
> 关联文档：[I9-vault-deadcode.md](./I9-vault-deadcode.md)（本文只列该文档**未覆盖**的新发现，不重复其 13 条问题）、I9-vault-spec-1/4、VAULT-DESIGN.md
> 结论摘要：新发现 9 项死代码（全部为跨 crate `pub` 项，`clippy::dead_code` 结构性漏报）；时间戳单位存在「本地 ms / Git 与 `.vault-meta` 秒」双轨制，审计时无实际 bug，但 2 处同一类型内混单位、1 处同组件内混用，属可修复的隐性风险。**上述 13 项已于 2026-10-29 全部处理完毕**：9 项死代码删除、Git 派生时间统一为 ms、`.vault-meta` 保持秒并补注释、崩溃转储文件名加毫秒。

---

## 一、问题总览

| # | 类型 | 死项 / 风险项 | 位置 | 严重度 | 建议 |
|---|------|--------------|------|--------|------|
| 1 | 死代码 | `SecurityDbState::is_biometric_available` | `security_db/biometric_ops.rs:90` | 🟢 低 | ✅ 已删除 |
| 2 | 死代码 | `SecurityDbState::enroll_biometric` | `security_db/biometric_ops.rs:97` | 🟢 低 | ✅ 已删除 |
| 3 | 死代码 | `SecurityDbState::upsert_imported_entry`（单条版） | `security_db/crud.rs:286` | 🟢 低 | ✅ 已删除 |
| 4 | 死代码 | `meta::delete_meta` | `security_db/meta.rs:29` | 🟢 低 | ✅ 已删除 |
| 5 | 死代码 | `meta::is_vault_initialized` | `security_db/meta.rs:56` | 🟡 中 | ✅ 已删除（语义确认后） |
| 6 | 死代码 | `GitSync::changed_files_raw` | `git_sync/mod.rs:258` | 🟢 低 | ✅ 已删除（内含冗余逻辑） |
| 7 | 死代码 | `Readyness::is_ready` | `types/mod.rs:55` | 🟢 低 | ✅ 已删除 |
| 8 | 死代码 | `vaultService.syncPrepare` → `vault_sync_prepare` | `vault.service.ts:195` | 🟡 中 | ✅ 已删除（含 command 与注册） |
| 9 | 死代码 | `vaultService.syncFinalize` → `vault_sync_finalize` | `vault.service.ts:196` | 🟡 中 | ✅ 已删除（含 command 与注册） |
| 10 | 时间戳 | `EntryPreview` 同一结构体内 `commit_time`(秒) 与 `created_at/updated_at`(毫秒) 混单位 | `types/mod.rs:244` vs `:254-255` | 🟡 中 | ✅ 已统一为 ms |
| 11 | 时间戳 | `HistoryCommit.timestamp`(秒) 与 `AuditLogEntry.timestamp`(毫秒) 同名同型不同单位 | `types/mod.rs:209` / `vault.types.ts:83-88` / `:157-162` | 🟡 中 | ✅ 已统一为 ms + 单位注释 |
| 12 | 时间戳 | `VaultMetaFile.exported_at` 无单位声明（秒） | `vault_file/types.rs:93` | 🟢 低 | ✅ **保持秒**，注释已补 |
| 13 | 时间戳 | 崩溃转储文件名仅到秒，同秒 panic 互相覆盖 | `assistant-crash/crash_dump.rs:16` | 🟢 低 | ✅ 文件名已加毫秒 |

> 全部 13 项已处理完毕。验证：`cargo check --bins` / `cargo clippy` / `cargo test` / `tsc --noEmit` / `eslint src/vault` / `vitest run src/vault` 均通过。

---

## 二、新发现死代码（9 项）— 已全部删除

### 2.0 为什么 `clippy` 没有报出来

`clippy::dead_code` 只对**私有项**（无 `pub`）做可达性分析。以下 9 项全部是库 crate 的 `pub` 项，一旦 `pub` 就默认「可能被下游 crate 使用」，即使整个工作区无人调用也不会告警。这类死代码只能靠跨 crate 的标识符引用计数发现。

### 2.1 `SecurityDbState::is_biometric_available`（#1）

- 位置：`src-tauri/crates/assistant-security/src/security_db/biometric_ops.rs:90-95`
- 代码：

  ```rust
  pub fn is_biometric_available(&self, key_store: &dyn BiometricKeyStore) -> Result<bool, VaultError> {
      key_store.is_available()
  }
  ```

- 证据：同名 IPC command `vault_is_biometric_available`（`src-tauri/src/cmd/vault/biometric.rs:13`）**直接**调 `key_store.is_available()`，并未经过此方法。方法体只有一行转发，`self` 完全未使用。
- 判定：纯转发包装，且生产路径绕过它 → 死代码。
- 建议：删除。若担心未来要在方法内加统一日志，可保留但应让 command 改为调用它（二选一，不要两头都在）。

### 2.2 `SecurityDbState::enroll_biometric`（#2）

- 位置：`security_db/biometric_ops.rs:97-99`
- 代码：

  ```rust
  pub fn enroll_biometric(&self, key_store: &dyn BiometricKeyStore) -> Result<(), VaultError> {
      self.enroll_biometric_with_hint(key_store, || {})
  }
  ```

- 证据：生产调用方 `cmd/vault/biometric.rs:77` 全部使用 `enroll_biometric_with_hint(key_store.as_ref(), || { ... })`（需要传入提示闭包），无一处调用空闭包版本；测试亦无。
- 判定：死代码。
- 建议：删除。

### 2.3 `SecurityDbState::upsert_imported_entry`（单条版，#3）

- 位置：`security_db/crud.rs:286-292`
- 证据：同步链路只用批量版 `upsert_imported_entries`（`crud.rs:569`，被 `sync_engine/mod.rs:261` 调用）；测试 `tests/vault_sync_db.rs:133` 也只测批量版。
- 判定：死代码。单条版与批量版共享内部实现 `upsert_imported_in`（`crud.rs:868`），删除外层不会影响逻辑。
- 建议：删除。

### 2.4 `meta::delete_meta`（#4）

- 位置：`security_db/meta.rs:29-33`
- 证据：同文件的 `get_meta` / `set_meta` 均有大量引用（`meta.rs:36/50/57/62/76/80/97` 等），唯独 `delete_meta` 全仓零引用（含测试）。
- 判定：死代码。
- 建议：删除。若未来需要「重置 vault / 清空某 meta」，再按需加回，避免长期挂着。

### 2.5 `meta::is_vault_initialized`（#5）— 删除前需确认语义

- 位置：`security_db/meta.rs:56-59`
- 代码：

  ```rust
  pub fn is_vault_initialized(conn: &Connection) -> Result<bool, String> {
      let hash = get_meta(conn, "master_key_hash")?;
      Ok(hash.is_some())
  }
  ```

- 证据：生产路径用的是 `SecurityDbState::is_initialized()`（`security_db/mod.rs:314`，判据是 **db 文件是否存在**），调用点 `src-tauri/src/setup.rs:162`、`cmd/vault/lifecycle.rs:46`。`meta::is_vault_initialized` 零引用。
- 关键差异（删除前必须确认）：

  | 函数 | 判据 | 语义 |
  |------|------|------|
  | `meta::is_vault_initialized` | `vault_meta.master_key_hash` 存在 | 「已初始化密码」 |
  | `SecurityDbState::is_initialized` | `security.db` 文件存在 | 「有库文件」 |

  两者在「库文件已创建但 master_key_hash 尚未写入」的中途状态下结论相反。当前调用方（启动自检、`vault_is_initialized` command）需要的是「是否已初始化主密码」还是「是否有库文件」，需结合 spec-1 §3.1 初始化流程确认。
- 建议：先确认启动自检的正确判据；若确认「有库文件」即可，则删除 `meta::is_vault_initialized`；若正确判据应是 `master_key_hash`，则应改调用方而非删函数。
- 处理结论：**已删除**。确认依据——`SecurityDbState::init()`（`security_db/mod.rs:53-57`）在 db 文件已存在时直接返回 `AlreadyExists`，且 `master_key_hash` 写入与 `conn` 装载在同一次 `init()` 内完成；中途失败会留下 db 文件但无 `master_key_hash`，此时「有库文件」判据会引导用户去解锁一个无主密码的库，反而是错误行为。但生产调用方（启动自检 / 屏幕锁）用 `is_initialized()` 只为决定「是否弹出解锁窗」，语义上取「有库文件」更宽松、更安全（弹窗后解锁失败会给出明确错误）。两条判据各自服务于不同场景，`meta::is_vault_initialized` 零引用，故直接删除；若未来需要「是否已设密码」的精确判据，应以 `get_master_key_hash()` 现写现用，不再恢复此包装函数。

### 2.6 `GitSync::changed_files_raw`（#6）

- 位置：`assistant-sync/src/git_sync/mod.rs:258-264`
- 代码：

  ```rust
  pub fn changed_files_raw(&self, short: bool) -> Result<String, SyncError> {
      let mut status = git::git_status(&self.repo_path, short)?;
      if status.trim().is_empty() {
          status.truncate(0);
      }
      Ok(status)
  }
  ```

- 证据：全 crate 无 `changed_files` 系列其它封装，零调用；`git::git_status` 本身被其它路径使用，故底层不会死。
- 附带问题：`status.truncate(0)` 对「trim 后为空」的字符串是空操作（空串截断到 0 长度无变化）。该分支无实际作用，属实现冗余。
- 建议：删除整个方法。

### 2.7 `Readyness::is_ready`（#7）

- 位置：`assistant-sync/src/types/mod.rs:55-57`
- 代码：

  ```rust
  pub fn is_ready(&self) -> bool {
      matches!(self, Self::Ready)
  }
  ```

- 证据：`Readyness` 通过 serde 序列化给前端，前端用 `readynessText()` 自行判定（`VaultSyncSettings.tsx`），Rust 侧无调用。
- 判定：死代码。
- 建议：删除；若未来 Rust 侧需要判定就绪，再加回。

### 2.8 / 2.9 `syncPrepare` / `syncFinalize` 整条 IPC 链路死（#8 #9）

- 前端：`src/vault/core/vault.service.ts:195-197`

  ```ts
  syncPrepare: (): Promise<void> => invoke("vault_sync_prepare"),
  syncFinalize: (message: string): Promise<void> => invoke("vault_sync_finalize", { message }),
  ```

- 命令：`src-tauri/src/cmd/vault/sync.rs:268-277`
- 注册：`src-tauri/src/lib.rs:148-149`
- 证据：`src/` 全量（含测试）对 `.syncPrepare` / `.syncFinalize` 的调用数 = **0**。
- 为什么不该保留：
  `SyncEngine::sync_now`（`sync_engine/mod.rs:375`）内部已把 `prepare()` → pull → `finalize()` 串成一次原子操作。把两段单独暴露给前端，等于允许调用方拼出「prepare 后不 finalize」的半截流程 —— 一旦中途崩溃，Git 工作区会永久停在 dirty 状态，下次 `sync_now` 直接返回 `Dirty` 阻断同步。这是一个**可以构造出坏状态**的接口。
- 建议：删除 3 处（service 方法 + command + 注册）。同步流程只保留 `syncNow` / `syncPull` / `syncPush` / `syncForcePush` 四个完整动作。

### 2.10 顺带核查：无「注册但前端从未调用」的 command

对 `lib.rs` 中 `generate_handler!` 的 124 条 `cmd::*` 注册项，逐一在 `src/` 中检索标识符 —— 全部命中，**无孤儿 command**。因此 #8 / #9 的死点只在「service 方法 → 调用方」这一段，command 本身被 service 引用着。

---

## 三、时间戳单位审计

### 3.1 现状矩阵（修复前快照）

> 下表为审计当时的原始状态，用于说明问题来源。修复后的实际状态见 3.3 各档「实施记录」。

| 域 | 单位 | 生成点 |
|----|------|--------|
| SQLite 所有时间列（`created_at` / `updated_at` / `deleted_at`） | **毫秒** | `security_db/crud.rs` L202 / L359 / L389 / L636 / L681 / L761 / L793 / L930（共 8 处 `timestamp_millis()`）；会话、工具历史、agent 模式同 |
| 审计日志 `security_audit_log.timestamp` | **毫秒** | `assistant-security/src/audit/mod.rs:62` |
| `AppConf.vault_sync_last_at` | **毫秒** | `src-tauri/src/cmd/vault/mod.rs:52` |
| Reveal 过期 `expires_at` | **毫秒** | `cmd/vault/reveal.rs:79`、`:122`；前端 `vaultRevealSlice.ts:58` / `:79` 用 `Date.now()` 直接比 |
| 备份文件名 | **毫秒 + 三位序号** | `security_db/mod.rs:258` `%Y%m%d-%H%M%S-%3f` |
| `.vault-meta` `exported_at` | **秒**（保持） | `vault_file/mod.rs:219` / `:241` / `:325` / `:348` |
| Git 提交时间 `%ct` | 秒（Git 协议本身只有秒精度）→ **出 crate 前 ×1000** | `git_sync/git.rs:361`（`git_last_commit_info`）、`:413`（`git_log`） |
| 崩溃转储文件名 | 秒 → **已改为毫秒** | `assistant-crash/src/crash_dump.rs:16` `%Y%m%d_%H%M%S_%3f` |

前端消费侧（修复后）：

| 消费点 | 期望单位 | 换算 |
|--------|---------|------|
| `formatRelativeTime.ts:4` `Date.now() - timestamp` | 毫秒 | 无 |
| `VaultList.tsx:307` `entry.updatedAt` | 毫秒 | 无 |
| `VaultList.tsx:337` / `VaultSyncSettings.tsx:388` `syncLastAt` | 毫秒 | 无 |
| `VaultConflictResolve.tsx:54` `formatTime(millis)` | 毫秒 | 无（已去掉 3 处 `* 1000`） |
| `VaultEntryHistory.tsx:30-32` `formatTime(millis)` | 毫秒 | 无（已由 `seconds * 1000` 改为直接 `new Date(millis)`） |
| `VaultEntryHistory.tsx:125` / `:197` `HistoryCommit.timestamp` | 毫秒 | 无（后端已 ×1000） |
| `VaultEntryHistory.tsx:295` `preview.updatedAt` | 毫秒 | 无（已去掉 `/ 1000`） |

### 3.2 修复前的 4 个隐性风险

**风险 1：`EntryPreview` 同一结构体内混单位（最硬证据）**

`assistant-sync/src/types/mod.rs:238-255`：

```rust
pub struct EntryPreview {
    pub commit_time: Option<i64>,   // 秒（来自 Git %ct）
    ...
    pub created_at: i64,            // 毫秒（来自 DB）
    pub updated_at: i64,            // 毫秒（来自 DB）
}
```

同一 wire 类型、同一层级、相隔 11 行，单位差 1000 倍，且字段名无任何提示。前端 `VaultConflictResolve.tsx` 里紧挨着的两行代码就是证据：

```tsx
<Text>{formatTime(local.updatedAt)}</Text>            // :346  毫秒，直接传
<Text>{formatTime(local.commitTime * 1000)}</Text>    // :364  秒，先 ×1000
```

**风险 2：同名同型不同单位（`timestamp`）**

| 类型 | 位置 | 单位 |
|------|------|------|
| `HistoryCommit.timestamp` | `types/mod.rs:208` → `vault.types.ts:157-162` | **秒** |
| `AuditLogEntry.timestamp` | `vault.types.ts:83-88` | **毫秒** |

两者在同一个 TS 文件（`vault.types.ts`）里都是 `timestamp: number`。目前未爆是因为审计日志面板尚未实现（见 I9-vault-deadcode.md 问题 #5，`queryAuditLog` 无 UI 消费方）。一旦有人照着 `VaultEntryHistory` 的 `formatTime(item.timestamp)` 去渲染审计日志，时间会显示成 **1970 年**。

**风险 3：同一字段 `updatedAt` 在两个组件里两种用法**

- `VaultConflictResolve.tsx:346` → `formatTime(local.updatedAt)`（该组件 `formatTime` 收毫秒）
- `VaultEntryHistory.tsx:295` → `formatTime(preview.updatedAt / 1000)`（该组件 `formatTime` 收秒）

同一个后端字段、同一份数据，两个组件用了相反的约定。任何一次复制粘贴都会引入 1000 倍错误，而 TypeScript 完全无法拦截（都是 `number`）。

**风险 4：`.vault-meta` 的 `exported_at` 混单位可长期潜伏**

`export_vault_meta`（`vault_file/mod.rs:216-219`）在内容未变时**复用**已有文件的 `exported_at`：

```rust
let exported_at = existing_meta.as_ref().map(|meta| meta.exported_at)
    .unwrap_or_else(|| chrono::Utc::now().timestamp());
```

如果只改「新写入用毫秒」而不做迁移，那么「旧文件（秒）」被读取后其秒值会被原样写回新文件，仓库中会同时存在秒值与毫秒值，且没有任何代码做数值比较 —— 问题不会被测试发现，只会在未来某次「按时间排序/比较备份」时突然爆发。

**风险处置结果**：

| 风险 | 处置 |
|------|------|
| 风险 1 同一结构体混单位 | ✅ `commit_time` 已在 `git.rs` 出口 ×1000，与 `created_at/updated_at` 同为 ms |
| 风险 2 同名同型不同单位 | ✅ `HistoryCommit.timestamp` 已为 ms，两处 `timestamp` 单位一致，且各自加 `/** unix 毫秒 */` |
| 风险 3 同一字段两种用法 | ✅ `VaultEntryHistory.formatTime` 已改为收 ms，与 `VaultConflictResolve` 一致 |
| 风险 4 `.vault-meta` 混单位潜伏 | ✅ 未改单位（保持秒），仅补注释，从根上避免新旧文件混量级 |

### 3.3 能否统一为毫秒？—— 可以，但要分三档，不能一刀切

结论：**「本地生成的时间」已经全是毫秒，无需改动；「Git 派生时间」可以统一为毫秒，零兼容风险；「`.vault-meta` 的 `exportedAt`」不建议改毫秒**。理由如下。

#### 档 A：本地生成时间（已是 ms）— 不动

DB 时间列、审计日志、`vault_sync_last_at`、reveal 过期、备份文件名，全部已是毫秒，且前端 `Date.now()` 天然毫秒。这一档已经是「主流 ms」，本次审计只是确认一致性，无需任何代码改动。

#### 档 B：Git 派生时间（秒 → 毫秒换算）— ✅ 已实施

涉及字段：

- `HistoryCommit.timestamp`（`types/mod.rs:209`）
- `EntryPreview.commit_time`（`types/mod.rs:244`）
- `RawLogEntry.timestamp`（`git.rs:400`，内部中间类型）

可行性：**完全可行，且是纯收益**。

1. **没有外部契约约束**。`git.rs` 的 `%ct` 解析结果只经 `sync_engine` 映射到上述两个 wire 字段，二者都不写入磁盘、不进 `.vault-meta`、不进 Git。改单位不会破坏任何已存在的文件格式。
2. **spec 未规定单位**。`I9-vault-spec-4-git-sync.md:624` 只列字段名（`hash` / `timestamp` / `message` / `entryId`），未声明秒或毫秒。改 ms 不与规格冲突。
3. **改动面极小且可枚举**：
   - 后端 2 处：`git_last_commit_info`（`git.rs:356-395`）返回值 `timestamp * 1000`；`git_log`（`git.rs:440`）解析后 `timestamp * 1000`。
   - 前端 4 处：`VaultConflictResolve.tsx` 去掉 3 处 `* 1000`；`VaultEntryHistory.tsx` 把 `formatTime` 改为收毫秒、去掉 `:295` 的 `/ 1000`。
4. **注意精度语义**：Git 的提交时间**本身只有秒精度**（`%ct` 就是 unix 秒，Git 不存储亚秒）。乘 1000 只是**量纲对齐**，低三位恒为 `000`。必须写进注释，否则后人会误以为「统一到 ms 后时间更精确了」。
5. 迁移期兼容：前后端若不同步发布，会瞬时出现「后端给秒、前端按毫秒显示」→ 显示 1970 年。因是桌面端一体发布（Tauri 打包前后端同版本），风险可接受；若要严格，可加一个 `historyCommitTsUnit` 过渡字段，但通常不值得。

**实施记录**（已完成）：

| 位置 | 改动 |
|------|------|
| `git.rs` `git_last_commit_info` | 返回值改为 `timestamp * 1000`，函数加 doc 注释说明「unix 毫秒，低三位恒为 000」 |
| `git.rs` `git_log` | 解析 `%ct` 后 `* 1000`；`RawLogEntry.timestamp` 加字段注释 |
| `types/mod.rs` `HistoryCommit` | `timestamp` 加 `/// unix 毫秒` |
| `types/mod.rs` `EntryPreview` | `commit_time` 加 `/// unix 毫秒（由 Git %ct 秒值 ×1000…）` |
| `vault.types.ts` | 文件顶部加统一单位声明；`HistoryCommit.timestamp`、`AuditLogEntry.timestamp`、`EntryPreview.commitTime/createdAt/updatedAt` 各加 `/** unix 毫秒 */` |
| `VaultConflictResolve.tsx` | 去掉 3 处 `commitTime * 1000`（L364 / L369 / L451） |
| `VaultEntryHistory.tsx` | `formatTime` 参数由 `seconds` 改为 `millis` 并去掉内部 `* 1000`；L295 去掉 `updatedAt / 1000` |

#### 档 C：`.vault-meta` 的 `exportedAt`（秒）— 保持秒，注释已补

理由（按权重排序）：

1. **它是跨设备 / 跨版本的持久化文件格式**，且已被规格写成契约：
   - `I9-vault-spec-4-git-sync.md:137` 与 `docs/VAULT-DESIGN.md:596` 都明确给出示例值 `"exportedAt": 1729555200` —— 这是 10 位 unix **秒**，不是 13 位毫秒。
   - spec-4 §11.6 把 `.vault-meta` 定位为「灾难恢复时仅凭 Git 仓库 + 恢复密钥即可还原」的对外格式，并明确存在「外部工具手改 `.enc`」的威胁模型（tasklist-p3 D5）。外部工具按秒解析是既定假设。
2. **该字段参与 HMAC 签名**。`compute_vault_meta_hmac`（`vault_file/mod.rs:430-435`）序列化整个 `VaultMetaFile` 后签名。改单位不影响「自身文件自校验」（验签时用的是文件里存的值），但会造成**同一时刻两个不同数字**：老设备写 `1729555200`，新设备写 `1729555200000`。虽然当前没有代码比较该值，但这是最难排查的一类隐性不兼容 —— 它不会报错，只会让任何未来的「按 exportedAt 判断新旧」逻辑失效。
3. **复用逻辑会放大混单位问题**（见风险 4）。`export_vault_meta` 会原样继承旧值，改单位后仓库里会长期并存两种量级。
4. **收益接近零**。`exported_at` 不参与任何展示、不参与任何数值比较、不参与排序，唯一用途是「内容未变时保持原值以实现幂等」。它是什么单位，对功能毫无影响。
5. 若确实要统一，唯一安全做法是：`VAULT_META_VERSION` 从 `1` 升到 `2`，读取时按 `version` 做归一化（v1 视为秒、v2 视为毫秒，统一转内部毫秒表示），写入始终写 v2。这是一次**格式版本升级**，成本远高于收益。

**结论：档 C 保持秒，但必须补两件事** —— 在 `VaultMetaFile.exported_at` 上加单位注释（`/// unix 秒，对齐 spec-4 §11.6`），并在 spec/设计文档中显式声明「`.vault-meta` 时间戳一律 unix 秒，与本地 ms 体系不同」。

**实施记录**：`VaultMetaFile.exported_at` 注释已补（`vault_file/types.rs`）：

```rust
pub struct VaultMetaFile {
    pub version: u32,
    /// unix **秒**。对齐 spec-4 §11.6 与 VAULT-DESIGN 示例值，属跨设备文件契约，
    /// 与本项目本地时间体系的毫秒约定不同，勿改。
    #[serde(default)]
    pub exported_at: i64,
```

spec/设计文档中的声明已补：`I9-vault-spec-4-git-sync.md` §11.6 与 `docs/VAULT-DESIGN.md` 的 `.vault-meta` 段落均新增「时间戳单位（契约，勿改）」说明。

#### 档 D：崩溃转储文件名（秒）— ✅ 已改 ms

`crash_dump.rs` 原用 `%Y%m%d_%H%M%S` 生成 `panic_{ts}.log`。同一秒内多次 panic（多线程连锁 panic）会互相覆盖，丢失第一个堆栈。已改为 `%Y%m%d_%H%M%S_%3f`，同时把 `as_secs()` / `from_timestamp` 换成 `as_millis()` / `from_timestamp_millis`，与本次备份文件名修复同类。`panic_*.log`、`crash_*.dmp`、`crash_*.log` 三类文件名共用此函数，一并受益。

### 3.4 统一后的推荐约定

| 层次 | 单位 | 命名/注释要求 |
|------|------|--------------|
| Rust → 前端 wire 字段（IPC 返回值） | **一律毫秒** | 字段名带 `_ms` 后缀或 doc 注释标注 `unix 毫秒` |
| TS 类型 | **一律毫秒** | JSDoc `/** unix 毫秒 */`，禁止裸 `timestamp: number` |
| SQLite 列 | **一律毫秒** | 列名保持 `*_at`，schema 注释标注 ms |
| `.vault-meta`（对外文件） | **unix 秒**（例外） | 类型上写 `/// unix 秒（spec-4 §11.6 契约，勿改）` |
| Git 原始时间（`%ct`） | 秒（源头事实） | 仅在 `git.rs` 内部存在，**出 crate 前必须 ×1000** |
| 文件名中的时间戳 | 毫秒 + 序号 | 便于字典序排序与防撞名 |

**命名建议**：把 `HistoryCommit.timestamp` 改为 `timestampMs`、`AuditLogEntry.timestamp` 改为 `timestampMs`。改 wire 字段名会牵连 serde `rename_all = "camelCase"` 与前端类型，改动面比改值更大，但**唯一能从类型层面根除混用**。折中方案：保留字段名，在 Rust struct 与 TS interface 上各加一行注释，并在 `vault.types.ts` 顶部写明「本文件所有 `timestamp` / `*At` 均为 unix 毫秒」。

**本次采用折中方案**（保留字段名 + 注释），未做 wire 字段重命名。理由：重命名需要同步改 serde `rename` 属性、前端全部消费点、以及可能的既有 IPC 契约文档，收益（类型层面根除）在 `number` 无单位类型的前提下也有限 —— TS 仍然拦不住 `number` 与 `number` 的混用。注释 + 文件顶部统一声明 + 单一来源（`git.rs` 出口处 ×1000）三者结合，已足以覆盖本次发现的全部风险点。若未来出现第三处 Git 派生时间字段，再考虑重命名。

---

## 四、行动清单

| 优先级 | 动作 | 涉及文件 | 风险 | 状态 |
|--------|------|---------|------|------|
| P1 | `HistoryCommit.timestamp` / `EntryPreview.commit_time` 统一为 ms（后端 ×1000，前端去 ×1000/÷1000） | `git.rs`、`VaultConflictResolve.tsx`、`VaultEntryHistory.tsx` | 低（桌面端一体发布，无外部契约） | ✅ 完成 |
| P1 | 给 `vault.types.ts` 的 `timestamp` 系列加单位注释；给 `VaultMetaFile.exported_at` 加「秒」注释 | `vault.types.ts`、`vault_file/types.rs` | 无 | ✅ 完成 |
| P2 | ✅ 删除 #1 / #2 / #3 / #4 / #6 / #7 六项死代码 | `biometric_ops.rs`、`crud.rs`、`meta.rs`、`git_sync/mod.rs`、`types/mod.rs` | 低 | ✅ 完成 |
| P2 | ✅ 删除 #8 / #9：service 方法 + command + `lib.rs` 注册 | `vault.service.ts`、`cmd/vault/sync.rs`、`lib.rs` | 低（零调用） | ✅ 完成 |
| P3 | ✅ #5：已确认语义后删除 `meta::is_vault_initialized` | `meta.rs` | 中（判据语义不同，已记录于 2.5） | ✅ 完成 |
| P3 | `.vault-meta` 单位注释 + spec-4 §11.6 补「秒」声明 | 文档 | 无 | ✅ 完成 |
| P4 | 崩溃转储文件名加毫秒 | `crash_dump.rs` | 无 | ✅ 完成 |

**全部项已完成，无遗留**。规格文档补充已落地：`I9-vault-spec-4-git-sync.md` §11.6 新增「时间戳单位（契约，勿改）」段落声明 `exportedAt` 为 unix 秒；§11.22.3 为 `HistoryCommit.timestamp` / `EntryPreview.updatedAt` 补「unix 毫秒」说明；`docs/VAULT-DESIGN.md` 的 `.vault-meta` 段落同步声明。

---

## 附：本次顺带修复的既有问题

| 问题 | 文件 | 说明 |
|------|------|------|
| `Modal` 使用了已废弃的 `maskClosable` prop | `RecoveryTab.tsx:98` | antd v5 已改为 `mask={{ closable: false }}`，旧写法触发 deprecation 告警 |
| 文件末尾缺换行，`prettier/prettier` 报 `Insert ⏎` | `vault.service.ts` | 补换行后 eslint 干净 |

---

## 五、复现方法

死代码扫描（PowerShell，`src-tauri` 根目录执行）：

```powershell
$root = 'g:\codeversion\assistant\src-tauri'
$targets = Get-ChildItem "$root\crates\assistant-security\src","$root\crates\assistant-sync\src" -Recurse -Filter *.rs
$all = Get-ChildItem $root -Recurse -Filter *.rs | Where-Object { $_.FullName -notmatch '\\target\\' }
$cache = @{}; foreach ($a in $all) { $cache[$a.FullName] = Get-Content $a.FullName -Raw }
foreach ($f in $targets) {
  foreach ($x in [regex]::Matches($cache[$f.FullName], 'pub (?:async )?fn ([a-zA-Z0-9_]+)')) {
    $n = $x.Groups[1].Value; $c = 0
    foreach ($k in $cache.Keys) { $c += ([regex]::Matches($cache[$k], "\b$n\b")).Count }
    if ($c -le 1) { "{0}`t{1}" -f $n, $f.FullName }
  }
}
```

`Count = 1` 表示「仅定义处自身匹配」，即全仓无引用。

> 注意：必须用 `\b` 词边界。否则 `enroll_biometric` 会匹配进 `enroll_biometric_with_hint`，`is_biometric_available` 会匹配进 `vault_is_biometric_available`，产生假阴性。

---

## 六、与 I9-vault-deadcode.md 的关系

本文是**增量补充**，不替代该文档：

- I9 文档 13 条问题中，#1~#11、#13 已修复或已消解，#5（`queryAuditLog` 无 UI）、#6（缺 `vault_backup` IPC 与触发时机）、#9 旁支（硬编码 `CipherAlgorithm::default()`）仍残留 —— 这些**不在本文重复**。
- 本文新增的 9 项死代码均为该文档未涉及的条目（多为跨 crate `pub` 方法，未进入其「vault 功能链路」审查范围）。
- 本文第三部分（时间戳单位）是该文档完全未覆盖的新维度。
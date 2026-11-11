# I9 密码箱 — 仓库与同步完整性规格（S3）

> 状态：规格（2026-11-11）
> 上游：[I9-vault-key-hierarchy-design.md](./I9-vault-key-hierarchy-design.md)、[I9-vault-key-hierarchy-spec.md](./I9-vault-key-hierarchy-spec.md)
> 同层：[I9-vault-entry-cipher-spec.md](./I9-vault-entry-cipher-spec.md)（条目字节）
> 本规格补上 key-hierarchy-spec 中 **G22「同步层未覆盖」** 的缺口：`.vault-meta` 由谁写、pull/push 顺序、冲突、历史、`dekVersion` 失配、仓库内容白名单。
> 路径约定：全部为仓库相对路径。

---

## 0. 范围

| 落点 | 文件 | 说明 |
|---|---|---|
| 仓库文件 | `.vault-meta`、`entries/*.enc` | 进入 git 的全部内容 |
| 写 meta | `sync_engine/export.rs`、`vault_file/mod.rs` | 当前有两套写法 |
| pull/import | `sync_engine/pull_push.rs` | 读取、校验、导入、删除检测 |
| 冲突 | `sync_engine/history.rs`、`preview.rs` | 文件冲突、预览、揭示 |
| 历史 | `sync_engine/history.rs` | 历史提交预览/恢复 |
| 恢复封装 | `sync_engine/emergency.rs` | v1 `emergency` 读写、灾难导出 |
| 远端 | `sync_engine/{mod,remote_admin}.rs`、`cmd/vault/sync.rs` | URL、凭据、状态 |
| 本地残留 | `cmd/vault/recovery.rs`、`cmd/vault/mod.rs` | `security/recovery/`、临时文件 |

不覆盖：本机库重建（key-hierarchy-spec §4.7）、恢复纸面（key-hierarchy-spec §6）、导出文件（egress-spec）。

---

## 1. 现状

```
写路径（两条，重复）
A. export_all_entries() → export_entry(...) × N → read_emergency_data → export_vault_meta(master_key, crypto, emergency)
B. write_signed_vault_meta() → export_vault_meta(master_key, crypto, emergency)

读路径
pull() → git pull → import_from_repo()
  → verify_vault_meta_integrity(master_key)         // HMAC over VaultMetaFile
  → read_all_entries(master_key)                     // 逐条解密
  → upsert_imported_entries()
  → 比较 list_entry_files() 的 before/after → removed_ids
  → 若 unreadable_ids 非空且 remote_uses_other_master_key() → VaultKeyMismatch
```

### 1.1 缺陷

| 编号 | 问题 | 后果 | 证据 |
|---|---|---|---|
| **F16** | HMAC 直接以 `master_key` 为密钥，无域分离 | 与条目密钥、SQLCipher 密钥共用同一根材料，降低域隔离 | `vault_file/mod.rs:432-449` |
| **F18** | `vault_sync_remote_url` 可能内嵌凭据，日志/错误无脱敏约定 | 凭据可能进入日志或崩溃报告 | `assistant-config/src/vault_conf.rs:52`、`cmd/vault/mod.rs:61-77` |
| **F21** | `.vault-meta` 有两条写路径 | 一处改了签名/版本，另一处漏改，产生「同一仓库两种 meta」 | `sync_engine/export.rs:15-86` |
| **F22** | pull 时先验 meta HMAC，再逐条解密；任一不可读只记入 `unreadable_ids` | 若密钥未变但文件损坏，用户只看到「部分条目不可读」，没有修复路径 | `pull_push.rs:131-209` |
| **F23** | `removed_ids` 由「pull 前后文件列表差」推断 | 本地新建但尚未导出的条目会被误判为「远端删除」，触发 `PendingRemovals` | `pull_push.rs:181-194` |
| **F24** | `vault_import` 不立即写 `.enc`/不提交 | 导入数据在下次 push 前只存在于本机库（与 entry-cipher-spec §5.2 同一问题） | `cmd/vault/entries.rs:354-405` |
| **F25** | 灾难恢复 clone 到 `security/recovery/` 后不清理 | 远端仓库副本长期留在本机 | `cmd/vault/recovery.rs:64-207` |
| **F26** | `dekVersion` 失配无专门错误 | rotate_dek 后旧远端条目只会表现为「不可读」 | 尚无实现（v1.5 新增） |

---

## 2. 目标规则

### 2.1 仓库内容白名单

仓库根目录**只允许**出现：

```
.vault-meta
entries/*.enc
```

禁止出现：

| 禁止项 | 原因 |
|---|---|
| `security.db` / `-wal` / `-shm` | 本机缓存，含明文索引与审计 |
| `backups/*.bak` | 旧密钥代际，可被旧口令打开 |
| `.audit/*.jsonl` | 审计元数据，不应跨设备 |
| `vault-config.json` | 含远端 URL/凭据 |
| `*.tmp` | 原子写残留 |
| `recovery/` | 灾难恢复的临时 clone |

实现要求：

1. `VaultFileManager::ensure_structure` 只创建 `entries/`，不得创建其他目录。
2. 写入前检查仓库根目录，发现禁止项 → `RepositoryContainsForbiddenFile`，并在 UI 提示（不自动删除，避免误删用户文件）。
3. `.gitignore` 不得依赖用户手工维护；应用应在初始化时写入最小 `.gitignore`，只包含上述禁止项。

### 2.2 单一写入者

**规则：`.vault-meta` 只能由编排层写入。**

```
lifecycle::initialize / unlock / change_password / rotate_dek
  └─ VaultFileManager::write_vault_meta_v2
sync_engine 只能读，不得调用任何签名/写 meta 的函数
```

改动：

- 删除 `VaultFileManager::export_vault_meta`、`write_emergency_data`、`clear_emergency_data`、`rewrite_vault_meta_with_new_key`、`sign_vault_meta`、`compute_vault_meta_hmac`（与 key-hierarchy-spec §4.2 一致）。
- 删除 `SyncEngine::write_signed_vault_meta`、`to_crypto_meta`。
- `SyncEngine::export_all_entries` 改为：只写 `entries/*.enc`，然后调用 `lifecycle::sync_write_meta(repo, dek)` 这一唯一出口（由编排层负责 manifest + integrity + 原子写）。
- `vault_resolve_conflict` / `history_restore` 等需要改 meta 的场景，必须走同一出口。

理由：当前 F21 的两条写路径已经导致「meta 的字段集合」在两个调用点分别维护；v1.5 增加 `manifest` / `anchor` 后，任何一个漏改都会让 anchor 失真。

### 2.3 push 顺序（提交点唯一）

```
1. 确认已解锁，取得 dek（不取得 dbKey）
2. 遍历 DB 快照 → 写 entries/*.enc（原子写）
3. 删除回收站/远端确认删除对应的 .enc
4. compute_manifest(dekVersion)（遍历实际文件字节的 sha256）
5. 构造 VaultMetaFileV2（slots 不变、manifest 更新、recoveryBackup 不变）
6. write_vault_meta_v2（原子写；唯一提交点）
7. git add + commit("vault: sync")
```

约束：

1. 第 2–4 步失败 → 不写 meta、不 commit；`.enc` 可能已部分更新，但 manifest 仍是旧的，下次启动 `verify_manifest` 会发现并触发修复。
2. **禁止**在写 meta 之前 commit。
3. **禁止**在未解锁状态下导出（当前 `export_all_entries` 需要 `get_master_key`，保持）。

### 2.4 pull 顺序

```
1. 确认已解锁
2. git pull
   ├─ 冲突 → Conflict，不导入
   └─ 成功 → 继续
3. read_vault_meta_v2（严格解析）
   ├─ version=1 → UnsupportedVaultVersion
   └─ 非法 → VaultCorrupted
4. 比较 vaultId：
   ├─ 不同 → VaultKeyMismatch（远端是另一个仓库）
   └─ 相同 → 继续
5. 比较 dekVersion：
   ├─ remote > local → VaultDekVersionAhead（见 §2.6）
   └─ remote <= local → 继续
6. verify_meta_integrity(metaKey = HKDF(dek, "i9:v1:meta-integrity"))
7. verify_manifest()（逐条 sha256）
   ├─ 不符 → VaultManifestMismatch
   └─ 通过 → 继续
8. 逐条 decode_entry_file → import
9. 更新本机 anchor：metaHash = 当前 meta 文件 hash，dekVersion = 当前
10. 若有 removed_ids → 只登记待确认，不自动删
```

约束：

1. **第 6 步必须用 `metaKey`，不得再用 `master_key`**（修复 F16）。
2. **第 7 步必须在第 8 步之前**：先证明「文件集合与 manifest 一致」，再解密；否则会把「被替换但恰好能解密」的文件导入。
3. **第 9 步 anchor 更新是 pull 的一部分**：pull 成功后本机基线必须前移，否则下次解锁会误报 `VaultRollbackDetected`。
4. `removed_ids` 的推断必须改为「以 manifest 为准」：远端 manifest 中不存在、但本地 `entries/` 中存在的 ID 才是候选删除；本地新建未导出的条目不在此列（修复 F23）。

### 2.5 冲突

| 冲突类型 | 处理 |
|---|---|
| `entries/*.enc` 文本冲突 | 保持现状：预览两边 → 用户选本地/远端/两者保留 |
| `.vault-meta` 冲突 | **禁止自动合并**；必须让用户选择一边，选择后执行完整校验 |
| 同一 `dekVersion` 下 meta 冲突 | 选择后以所选 meta 的 manifest 重新校验全部 `.enc` |
| 不同 `dekVersion` 下 meta 冲突 | 见 §2.6 |

`.vault-meta` 冲突的额外要求：

1. 预览必须显示：`dekVersion`、`manifest.count`、`recoveryBackup`、文件 hash，不含密钥。
2. 选择远端 meta 后，若本机没有对应 `dek`（例如远端是更高 `dekVersion`），必须提示「需要远端口令/恢复纸面」，不得静默失败。
3. 选择本地 meta 后，push 会覆盖远端 meta；必须在 UI 明确「将覆盖远端密钥代数」。

### 2.6 `dekVersion` 失配

| 场景 | 错误 | 处置 |
|---|---|---|
| remote.dekVersion > local.dekVersion | `VaultDekVersionAhead` | 提示先拉取新口令/纸面；禁止用本地旧 dek 强行导入 |
| remote.dekVersion < local.dekVersion | `VaultDekVersionBehind` | 本地已 rotate；远端条目为旧代，禁止导入；提示 force push 覆盖 |
| 条目文件 `dekVersion` ≠ meta.dekVersion | `VaultCorrupted`（manifest 也应不符） | 拒绝该条，记入 `unreadable_ids` |
| rotate_dek 后旧历史提交 | 历史预览返回 `HistoryDekUnavailable` | UI 提示「该历史版本使用旧数据密钥，已不可读」 |

理由：`rotate_dek` 换掉 `dek` 后，旧 `dek` 不保留（key-hierarchy-spec §3.3），因此旧历史必然不可读。这是**有意的不可逆点**，不能伪装成「文件损坏」。

### 2.7 历史与预览

1. `history_preview` / `conflict_preview` 必须走 `mask_entry_preview`，且 `notes` 也必须脱敏（当前只脱敏 password/totp/secret 自定义字段，`notes` 明文返回）。
2. 预览中的 `title` / `username` / `url` 可显示（列表也需要），但必须标注「仅本机、未审计」。
3. `vault_conflict_reveal` 必须：
   - 走 `RevealGuard` 限速；
   - 写审计 `VaultReveal`（detail 带 `conflict:{id}:{side}:{field}`）；
   - 返回值设置 TTL（与 `vault_reveal_password` 一致）。
4. `history_restore` 必须重新加密并写 `.enc`，不得直接把历史明文写库。

### 2.8 远端与凭据

1. `vault_sync_remote_url` 视为**含密材料**：任何日志、错误、UI 展示前必须脱敏为 `scheme://host/path`，去掉 `user:token@`。
2. `vault-config.json` 不属于保险箱加密范围，但必须：
   - 文件权限 0600（平台允许时）；
   - 不进入仓库（§2.1）；
   - 不写入崩溃报告。
3. `git_test_connection` / `remote_status` 的错误消息同样脱敏。
4. `vault_get_sync_config` 仍返回完整 URL（编辑需要），但**前端展示时必须脱敏**，且不得把完整 URL 写进日志/错误/状态条。

### 2.9 临时目录与残留

1. `security/recovery/` 仅用于灾难恢复期间的临时 clone；导出结束（成功或失败）后必须删除。
2. `write_atomically` 的 `.tmp` 文件在下次启动时清理（仅限 `entries/` 与 `.vault-meta.tmp`）。
3. `security/backups/` 不进入仓库；改密后的旧备份策略见 runtime-session-spec §4.3。

---

## 3. 接口

```rust
// vault_file/mod.rs
pub fn write_vault_meta_v2(&self, meta: &mut VaultMetaFileV2, meta_key: &MetaKey)
    -> Result<(), VaultError>;
pub fn compute_manifest(&self, dek_version: u32) -> Result<VaultManifest, VaultError>;
pub fn verify_manifest(&self) -> Result<(), VaultError>;

// lifecycle.rs（唯一编排出口）
pub fn sync_write_meta(repo: &VaultFileManager, dek: &Dek, dek_version: u32)
    -> Result<VaultMetaFileV2, VaultError>;

// sync_engine/pull_push.rs
pub fn pull(&self, db: &SecurityDbState) -> Result<SyncResult, SyncError>;
pub fn import_from_repo(&self, db: &SecurityDbState) -> Result<ImportReport, SyncError>;
```

约束：`sync_engine` 不出现 `MasterKey` 参数（v1.5 改为 `Dek`），不出现任何 meta 签名函数。

---

## 4. 文件改动清单

| 文件 | 改动 |
|---|---|
| `src-tauri/crates/assistant-vault/src/sync_engine/export.rs` | 删除 `to_crypto_meta`、`write_signed_vault_meta`；`export_all_entries` 只写 `.enc` 并调用 `lifecycle::sync_write_meta` |
| `src-tauri/crates/assistant-vault/src/sync_engine/emergency.rs` | 删除 `write/read/clear_emergency_data`、`rewrite_vault_meta_with_new_key`；`disaster_recovery_export` 改走 `RecoverySession` |
| `src-tauri/crates/assistant-vault/src/sync_engine/pull_push.rs` | 重写 pull 顺序（§2.4）；`remote_uses_other_master_key` → `remote_vault_id`；新增 dekVersion 判定；`removed_ids` 以 manifest 为准 |
| `src-tauri/crates/assistant-vault/src/sync_engine/history.rs` | 预览脱敏 notes；`vault_conflict_reveal` 加限速+审计+TTL；历史不可读返回 `HistoryDekUnavailable` |
| `src-tauri/crates/assistant-vault/src/sync_engine/preview.rs` | `mask_entry_preview` 增加 notes 脱敏；新增 meta 预览结构 |
| `src-tauri/crates/assistant-vault/src/sync_engine/mod.rs` | 新增 `VaultDekVersionAhead` / `VaultDekVersionBehind` / `HistoryDekUnavailable` 映射 |
| `src-tauri/crates/assistant-vault/src/vault_file/mod.rs` | 删除 5 个 v1 meta 函数；新增 manifest/严格解析（见 key-hierarchy-spec §4.2–4.4） |
| `src-tauri/crates/assistant-vault/src/security_db/entries/query.rs` | 删除 `vault_crypto_info`（v1 专用 salt 导出，F21） |
| `src-tauri/crates/assistant-vault/src/security_db/rekey.rs` | 改密后不再清 `emergency`（v1.5 无此字段）；rotate_dek 清 `recoveryBackup` |
| `src-tauri/src/cmd/vault/entries.rs` | `vault_import` 增加 `.enc` 写入与 `finalize`；`vault_change_master_password` 删除 emergency/强制重导出 |
| `src-tauri/src/cmd/vault/recovery.rs` | 灾难恢复结束后删除 `security/recovery/` |
| `src-tauri/src/cmd/vault/sync.rs` | 远端 URL 脱敏；错误消息脱敏 |
| `src-tauri/src/cmd/vault/mod.rs` | 初始化时写入最小 `.gitignore`；启动清理 `.tmp` |

---

## 5. 错误码

| 错误码 | 触发 | retryable |
|---|---|---|
| `VaultDekVersionAhead` | remote dekVersion > local | 否 |
| `VaultDekVersionBehind` | remote dekVersion < local | 否 |
| `VaultManifestMismatch` | manifest 与 `.enc` 不符 | 否 |
| `VaultRollbackDetected` | anchor 不符 | 否 |
| `VaultKeyMismatch` | `vaultId` 不同 | 否 |
| `HistoryDekUnavailable` | 历史条目用旧 dek 加密 | 否 |
| `RepositoryContainsForbiddenFile` | 仓库出现禁止项 | 否 |
| `EntryFileNotReadable` | 条目文件结构化失败 | 否 |

同步相关错误一律**不可重试**（用户必须做决策）；`GitUnavailable` 保持可重试。

---

## 6. 测试

| 编号 | 用例 | 断言 |
|---|---|---|
| T-S1 | push 后检查仓库 | 只有 `.vault-meta` 与 `entries/*.enc`；无 `security.db`/`.tmp`/`backup` |
| T-S2 | 手工改 `entries` 中某文件一字节 → pull | `VaultManifestMismatch`，不导入任何条目 |
| T-S3 | 手工删 `entries` 中一条 → pull | 进入 `PendingRemovals`，不自动删本地 |
| T-S4 | 本地新建条目未 push → pull | 不被判为远端删除 |
| T-S5 | 远端 `vaultId` 不同 | `VaultKeyMismatch`，不导入 |
| T-S6 | 远端 `dekVersion` 更高 | `VaultDekVersionAhead`，不导入 |
| T-S7 | rotate_dek 后预览旧历史 | `HistoryDekUnavailable` |
| T-S8 | `.vault-meta` 冲突 | 不自动合并；选择后重新校验 manifest |
| T-S9 | 冲突预览含 notes | notes 被脱敏 |
| T-S10 | `vault_conflict_reveal` 连点 4 次 | 第 4 次 `RateLimited`；每次成功都有审计 |
| T-S11 | 灾难恢复成功/失败 | `security/recovery/` 均被删除 |
| T-S12 | 远端 URL 含 `user:token@` 的失败日志 | 输出中不含 token |
| T-S13 | 旧 meta（version=1）pull | `UnsupportedVaultVersion`，不修改任何本地文件 |

测试落点：`src-tauri/crates/assistant-vault/tests/sync_engine.rs`、`vault_sync_db.rs`（已存在，扩展）；仓库白名单与清理用例可新建 `tests/repo_layout.rs`。

---

## 7. 禁止事项

1. **禁止** `sync_engine` 直接写 `.vault-meta` 或调用任何签名函数。
2. **禁止**在写 meta 之前 git commit。
3. **禁止**自动合并 `.vault-meta` 冲突。
4. **禁止**用本地旧 `dek` 强行导入更高 `dekVersion` 的远端条目。
5. **禁止**把「远端删除」与「本地未导出新建」混为一谈。
6. **禁止**在日志/错误/UI 中输出未脱敏的远端 URL。
7. **禁止**把 `security.db`、备份、审计 JSONL、配置文件纳入仓库。

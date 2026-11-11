# I9 密码箱 — 审计与可观测性规格（原 key-hierarchy-spec §8 迁出）

> 状态：规格（2026-11-11 分派）
> 来源：[I9-vault-key-hierarchy-spec.md](./I9-vault-key-hierarchy-spec.md) §8、§9.1 部分审计、附录 A 审计常量、附录 D-8。
> 上游：[I9-vault-key-hierarchy-design.md](./I9-vault-key-hierarchy-design.md) §7.3
> 同层：[I9-vault-runtime-session-spec.md](./I9-vault-runtime-session-spec.md)（会话事件）、[I9-vault-biometric-spec.md](./I9-vault-biometric-spec.md)（生物识别事件）、[I9-vault-recovery-channel-spec.md](./I9-vault-recovery-channel-spec.md)（恢复事件）、[I9-vault-sync-crypto-spec.md](./I9-vault-sync-crypto-spec.md)（同步事件）
> 本规格只负责**审计事件集合、双写存储、补录、失败降级**。

---

## 0. 为什么需要 JSONL

恢复通道与生物识别路径都可能在**没有 `security.db` 连接**时发生：

- 纸面恢复：只持 `dek`，不打开库；
- 生物识别失败：`conn` 未打开或探针已失败；
- 重置仓库：库已删除。

如果审计只写 `security_audit_log` 表，这些动作**完全不可审计**。因此必须双写：

| 通道 | 位置 | 加密 | 进 git |
|---|---|---|---|
| 主存储 | `security.db` 的 `security_audit_log` | SQLCipher | 否 |
| 追加日志 | `<data>/security/audit/audit-YYYYMMDD.jsonl`（与 `security.db` 同级） | **不加密** | 否 |

JSONL 只允许出现：时间、事件类型、条目 ID / 数量 / 原因。**禁止**出现密钥、口令、明文字段、恢复码、API key。

---

## 1. `AuditEvent` 扩展

文件：`src-tauri/crates/assistant-vault/src/audit/mod.rs`。

新增（`Display` 映射同时补齐）：

| 变体 | 字符串 | 归属域 |
|---|---|---|
| `RecoveryKeyExported` | `recovery_key_exported` | 恢复 |
| `RecoveryExportUsed` | `recovery_export_used` | 恢复 |
| `DekRotated` | `dek_rotated` | 密钥 |
| `LocalDbRepaired` | `local_db_repaired` | 运行时 |
| `LocalDbRebuilt` | `local_db_rebuilt` | 运行时 |
| `AnchorRollbackRejected` | `anchor_rollback_rejected` | 同步 |
| `BiometricUnlocked` | `biometric_unlocked` | 生物识别 |
| `BiometricFailed` | `biometric_failed` | 生物识别 |
| `BiometricRevoked` | `biometric_revoked` | 生物识别 |
| `VaultRepositoryReset` | `vault_repository_reset` | 能力 |
| `VaultUnsupportedVersion` | `vault_unsupported_version` | 能力 |
| `VaultEntriesExported` | `vault_entries_exported` | 明文出口 |
| `VaultEntriesImported` | `vault_entries_imported` | 明文出口 |

删除：

- `RecoveryKeyGenerated`（被 `RecoveryKeyExported` 取代）
- `RecoveryKeyInvalidated`（改密不再使纸面失效——`dek` 未变）

保留现有：`VaultUnlock` / `VaultUnlockFailed` / `VaultLock` / `VaultReveal` / `VaultRevealRateLimited` / 条目 CRUD / `VaultCopy*` / HIBP / `MasterPasswordChange` / `BiometricEnrolled` / `SshKey*`。

---

## 2. JSONL 双写

新增文件：`src-tauri/crates/assistant-vault/src/audit/jsonl.rs`。

```rust
pub struct AuditJsonlWriter { dir: PathBuf }

impl AuditJsonlWriter {
    pub fn new(dir: PathBuf) -> Self;
    pub fn append(&self, event: AuditEvent, detail: Option<&str>) -> Result<(), VaultError>;
    pub fn read_since(&self, since_ms: i64) -> Result<Vec<JsonlRecord>, VaultError>;
}

#[derive(Serialize, Deserialize)]
pub struct JsonlRecord { pub at: i64, pub event: String, pub detail: Option<String> }
```

| 项 | 规定 |
|---|---|
| 路径 | `<data>/security/audit/`（与 `security.db` 同级，`cmd/vault/mod.rs` 的 `security/` 目录下） |
| 文件名 | `audit-YYYYMMDD.jsonl`（本地时区） |
| 行格式 | 单行紧凑 JSON：`{"at":<ms>,"event":"<display>","detail":<string\|null>}` |
| 写入方式 | `OpenOptions::new().create(true).append(true)` + 单次 `write_all` + 行尾 `\n` |
| 原子性 | 追加写，无 rename；崩溃最多丢最后一行 |
| 轮转 | 跨日新建文件；单文件 > 8 MiB 时新建 `audit-YYYYMMDD-{n}.jsonl` |
| 编码 | UTF-8 |
| 权限 | 0600（平台允许时） |

### 2.1 双写封装

```rust
pub fn log_dual(
    conn: Option<&Connection>,
    writer: Option<&AuditJsonlWriter>,
    event: AuditEvent,
    detail: Option<&str>,
) -> Result<(), VaultError>;
```

规则：

1. **两路都尝试，任一路成功即返回 `Ok`**；两路都失败才返回错误。
2. 连接与 writer 均为 `Option`：恢复通道（`conn = None`）、未初始化（`writer = None`）都能调。
3. `audit::log` 保留为 `log_dual(Some(conn), None, ..)` 的别名，**现有 40+ 调用点不必改签名**。
4. JSONL 写入失败只 `warn`，**不得阻断**主业务；DB 写入失败按原逻辑返回错误。

---

## 3. 各域事件落点

### 3.1 恢复通道

| 事件 | 落点 | 理由 |
|---|---|---|
| `RecoveryKeyExported` | 命令层 `log_dual`（conn + jsonl） | 导出时库是解锁的 |
| `RecoveryExportUsed` | 命令层 `log_dual(None, jsonl)` | 恢复时**没有** `security.db` 连接 |

### 3.2 密钥与运行时

| 事件 | 落点 |
|---|---|
| `MasterPasswordChange` | `lifecycle::change_password` |
| `DekRotated` | `lifecycle::rotate_dek` |
| `LocalDbRepaired` / `LocalDbRebuilt` | `rekey.rs` 修复路径 |
| `AnchorRollbackRejected` | `SecurityDbState::unlock` 失败分支（conn 已打开，双写） |
| `VaultLock` | `lock_session`（detail 为 reason） |
| `VaultRepositoryReset` | 重置命令（库已删，仅 JSONL） |
| `VaultUnsupportedVersion` | 版本拒绝路径（conn 可能不可用，仅 JSONL） |

### 3.3 生物识别

| 事件 | 落点 | 理由 |
|---|---|---|
| `BiometricEnrolled` / `BiometricRevoked` | 双写 | 解锁态 |
| `BiometricUnlocked` | 双写 | 解锁成功后 |
| `BiometricFailed` | 视 `conn` 可用性；失败早期仅 JSONL | 不能因审计失败阻断解锁 |

`BiometricFailed.reason` 取值必须来自封闭集合：`payload_len` / `stale_payload` / `probe_failed` / `sig_hash_missing` / `sig_changed` / `decrypt_failed` / `wrapping_key_len` / `delete_failed`。

### 3.4 同步

| 事件 | 落点 |
|---|---|
| `VaultManifestMismatch`（可选） | pull 失败分支 |
| `AnchorRollbackRejected` | 同 3.2 |
| 远端删除确认 | 现有 `VaultEntryDeleted`（detail `remote`） |
| 冲突解决 | 现有恢复/更新事件 + detail 带 `conflict` |

### 3.5 明文出口

| 事件 | detail |
|---|---|
| `VaultEntriesExported` | `{format}:{count}:{pathHash}` |
| `VaultEntriesImported` | `{format}:{count}:{pathHash}` |
| `VaultReveal` | `{entryId}` 或 `conflict:{entryId}:{side}:{field}` |
| `VaultCopyPassword` / `VaultCopyField` | `{entryId}` 或 `totp:{entryId}` |

`pathHash` 为导出/导入路径的 SHA-256 小写 hex，**禁止**记录明文路径。

---

## 4. 解锁补录

`SecurityDbState::unlock` 成功后：

```
1. synced_at = meta::get_kv("audit_jsonl_synced_at")（缺失 → 0）
2. records = writer.read_since(synced_at)
3. 对每条 record 且 event ∈ {recovery_export_used, dek_rotated, local_db_rebuilt, vault_repository_reset}：
   INSERT INTO security_audit_log (id, event_type, detail, timestamp)
   VALUES ('jsonl-' || at || '-' || event, event, detail, at)
   └─ 已存在（UNIQUE 冲突）则跳过
4. meta::set_kv("audit_jsonl_synced_at", now)
```

约束：

1. **去重键** = `jsonl-{at}-{event}`，天然幂等。
2. 只补录「恢复态发生、解锁态才可见」的事件类型；其余事件解锁态本来就能直接写库。
3. 补录失败**不阻断解锁**（记 `warn`）。
4. `audit_jsonl_synced_at` 是 `vault_meta` 表的 KV 行，不改 DDL。

---

## 5. 失败降级

| 失败 | 行为 |
|---|---|
| JSONL 目录不可写 | `warn`；不阻断业务；下次启动重试创建 |
| DB 写入失败（已解锁） | 返回错误（与现有 `audit::log` 行为一致） |
| 双写都失败 | `log_dual` 返回错误；调用方按原逻辑决定是否阻断 |
| 补录失败 | `warn`；不阻断解锁 |

原则：**审计不能成为永久锁死的原因**，但也不能静默丢失已解锁态的审计。

---

## 6. 测试

| 编号 | 用例 | 断言 |
|---|---|---|
| T-A1 | 纸面恢复导出 | JSONL 有 `RecoveryExportUsed`；`security.db` 无写入 |
| T-A2 | 导出恢复纸面 | 双写有 `RecoveryKeyExported` |
| T-A3 | 改密 | 双写有 `MasterPasswordChange`；没有 `RecoveryKeyInvalidated` |
| T-A4 | `rotate_dek` | 双写有 `DekRotated`；JSONL 有 |
| T-A5 | 生物识别解锁失败 | JSONL 有 `BiometricFailed{reason}`；未解锁 |
| T-A6 | 重置仓库 | JSONL 有 `VaultRepositoryReset`；DB 无（库已删） |
| T-A7 | 下次解锁补录 | `security_audit_log` 出现 JSONL 中的恢复事件；重复解锁不重复插入 |
| T-A8 | JSONL 不可写 | 业务成功；日志有 warn |
| T-A9 | 日志扫描 | JSONL/DB 中无密钥、口令、明文字段、恢复码 |
| T-A10 | 导出文件审计 | detail 含 `pathHash`，不含明文路径 |

---

## 7. 禁止事项

1. **禁止**在审计 detail 中放明文字段、密钥、口令、恢复码、API key。
2. **禁止**把 JSONL 放进仓库目录。
3. **禁止**因为 JSONL 写失败而阻断解锁或恢复。
4. **禁止**在已解锁态只写 JSONL 而不写 DB（双写必须两路都尝试）。
5. **禁止**用时间戳以外的字段做补录去重键。
6. **禁止**新增事件只改 `Display` 而不加 `AuditEvent` 变体（会导致补录无法识别）。

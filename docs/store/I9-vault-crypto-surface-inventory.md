# I9 密码箱加密体系全景清单

> 状态：清单（2026-11-11，以当前代码为基线；v1.5 密钥体系尚未实施）
> 职责：把当前加密体系涉及的**全部文件、全部密钥材料、全部密文与明文落点**列成一张可核对的地图，并为每一项标注归属规格。
> 不重复：[I9-vault-key-hierarchy-design.md](./I9-vault-key-hierarchy-design.md) 的论证与 [I9-vault-key-hierarchy-spec.md](./I9-vault-key-hierarchy-spec.md) 的类型/函数定义。
> 配套规格：
> - [I9-vault-key-hierarchy-spec.md](./I9-vault-key-hierarchy-spec.md)（核心：值类型/原语/meta v2/编排/核心错误）
> - [I9-vault-entry-cipher-spec.md](./I9-vault-entry-cipher-spec.md)（条目层加密）
> - [I9-vault-sync-crypto-spec.md](./I9-vault-sync-crypto-spec.md)（仓库/同步完整性）
> - [I9-vault-runtime-session-spec.md](./I9-vault-runtime-session-spec.md)（运行时密钥与会话驻留）
> - [I9-vault-plaintext-egress-spec.md](./I9-vault-plaintext-egress-spec.md)（明文出口与辅助密钥）
> - [I9-vault-biometric-spec.md](./I9-vault-biometric-spec.md)（生物识别凭据）
> - [I9-vault-recovery-channel-spec.md](./I9-vault-recovery-channel-spec.md)（纸面与恢复通道）
> - [I9-vault-audit-spec.md](./I9-vault-audit-spec.md)（审计与 JSONL）
> - [I9-vault-capability-gating-spec.md](./I9-vault-capability-gating-spec.md)（能力开关/配置/Schema 门禁）
> 执行清单：[I9-vault-v15-tasklist.md](./I9-vault-v15-tasklist.md)（M0–M7 任务项与依赖）
> 路径约定：全部为仓库相对路径。

---

## 0. 六个加密面

本清单把「加密体系」切成六个互不重叠的面。任何改动都必须先落在这张表里的某一面，避免「改了密钥层次但漏了明文出口」：

| 面 | 名称 | 保护对象 | 主要位置 | 权威规格 |
|---|---|---|---|---|
| **S1** | 密钥层次 | 口令 → 数据密钥 | `assistant-crypto`、`security_db`、`recovery_key` | I9-key-hierarchy-spec |
| **S2** | 条目数据加密 | 条目明文（T1） | `security_db/entries`、`vault_file` | I9-entry-cipher-spec |
| **S3** | 仓库/同步完整性 | git 中的密文与元数据 | `sync_engine`、`vault_file` | I9-sync-crypto-spec |
| **S4** | 运行时会话 | 内存中的密钥与明文 | `security_db`、`cmd/vault`、前端 store | I9-runtime-session-spec |
| **S5** | 明文出口 | 离开进程/设备的明文 | 导出、剪贴板、揭示、预览、HIBP、SSH | I9-plaintext-egress-spec |
| **S6** | 能力与版本门禁 | 是否启用、用什么算法、什么版本 | `vault_plugin.rs`、`vault_conf.rs`、schema | I9-capability-gating-spec |

判据：

- **S1** 只看「谁能解出密钥」；
- **S2** 只看「一个条目字节被谁加密、被谁认证」；
- **S3** 只看「仓库被替换/回滚/冲突时能否发现」；
- **S4** 只看「解锁到锁定之间，密钥和明文在哪些内存里」；
- **S5** 只看「明文以什么形态离开应用边界」；
- **S6** 只看「开关、算法、版本不一致时会发生什么」。

---

## 1. 文件清单

### 1.1 `assistant-crypto`（密码学原语）

| 文件 | 加密职责 | 归属 |
|---|---|---|
| `src-tauri/crates/assistant-crypto/src/vault_crypto/mod.rs` | `derive_master_key` / `verify_master_password` / `derive_entry_key` / `derive_sqlcipher_key` / `encrypt_field` / `decrypt_field` | S1、S2 |
| `src-tauri/crates/assistant-crypto/src/vault_crypto/types.rs` | `MasterKey` / `PerEntryKey` / `VaultPlaintext` / `SecureString` / `CipherAlgorithm` | S1、S4 |
| `src-tauri/crates/assistant-crypto/src/vault_crypto/totp.rs` | TOTP 解析与码计算（持有 TOTP secret） | S2、S5 |
| `src-tauri/crates/assistant-crypto/src/api_key.rs` | 用 `SHA-256(machine_uid)` 做 AES-256-GCM 加密 API key | S5 |
| `src-tauri/crates/assistant-crypto/src/error.rs` | `VaultError` 全部变体 | S1–S6 |
| `src-tauri/crates/assistant-crypto/tests/vault_crypto.rs` | 原语单测（37 处密钥/加解密断言） | S1、S2 |
| `src-tauri/crates/assistant-crypto/tests/api_key.rs` | machine-id 加解密单测 | S5 |

### 1.2 `assistant-vault`（保险箱主体）

| 文件 | 加密职责 | 归属 |
|---|---|---|
| `src-tauri/crates/assistant-vault/src/security_db/mod.rs` | `BOOTSTRAP_SALT`、`SQLCIPHER_PRAGMAS`、`init`/`unlock`/`lock`、`master_key`/`bootstrap_key`/`conn` 三把 `Mutex` | S1、S4 |
| `src-tauri/crates/assistant-vault/src/security_db/schema.rs` | `vault_entries` / `security_audit_log` / `vault_meta` DDL | S2、S4 |
| `src-tauri/crates/assistant-vault/src/security_db/meta.rs` | `master_key_salt` / `hash_salt` / `master_key_hash` 的 KV 读写 | S1 |
| `src-tauri/crates/assistant-vault/src/security_db/rekey.rs` | `change_master_password`：全量重加密 + `PRAGMA rekey` + 备份 | S1、S2 |
| `src-tauri/crates/assistant-vault/src/security_db/biometric_ops.rs` | `BK‖MK` 64B payload、签名派生 wrapping key、`restore_keys_from_plaintext` | S1、S4 |
| `src-tauri/crates/assistant-vault/src/security_db/crypto_ops.rs` | 字段级解密：密码、备注、TOTP、自定义字段、SSH 私钥、整条解密 | S2 |
| `src-tauri/crates/assistant-vault/src/security_db/import_export.rs` | 导出为**明文**结构、导入查重 | S2、S5 |
| `src-tauri/crates/assistant-vault/src/security_db/entries/write.rs` | 条目创建/更新：加密 password/notes/totp/custom_fields；title/username/url/tags 明文列 | S2 |
| `src-tauri/crates/assistant-vault/src/security_db/entries/import.rs` | 同步导入：重新加密并写库 | S2、S3 |
| `src-tauri/crates/assistant-vault/src/security_db/entries/snapshot.rs` | 读快照：解出全部字段 | S2、S4 |
| `src-tauri/crates/assistant-vault/src/security_db/entries/deleted.rs` | 回收站、历史恢复、永久删除 | S2 |
| `src-tauri/crates/assistant-vault/src/security_db/entries/field_crypto.rs` | `encrypt_opt` / `decrypt_opt` / 自定义字段编解码 | S2 |
| `src-tauri/crates/assistant-vault/src/security_db/entries/query.rs`、`tx.rs`、`mod.rs` | 查询与事务包装（明文列参与检索） | S2 |
| `src-tauri/crates/assistant-vault/src/vault_file/mod.rs` | `.vault-meta` 读写/签名、`entries/*.enc` 编解码、HMAC | S2、S3 |
| `src-tauri/crates/assistant-vault/src/vault_file/types.rs` | `EntryFile` / `EntryBlob` / `VaultMetaFile` / `VaultCryptoMeta` / `EmergencyDataFile` | S2、S3 |
| `src-tauri/crates/assistant-vault/src/recovery_key/mod.rs` | v1 恢复码生成/解密/解锁、`EmergencyData` | S1、S5 |
| `src-tauri/crates/assistant-vault/src/sync_engine/export.rs` | 全量导出 + 写签名 meta | S3 |
| `src-tauri/crates/assistant-vault/src/sync_engine/pull_push.rs` | pull/push/sync、密钥失配判定、导入 | S3 |
| `src-tauri/crates/assistant-vault/src/sync_engine/emergency.rs` | 恢复封装读写、灾难导出 | S3、S5 |
| `src-tauri/crates/assistant-vault/src/sync_engine/history.rs` | 冲突解决、历史预览/恢复、冲突字段揭示 | S3、S5 |
| `src-tauri/crates/assistant-vault/src/sync_engine/preview.rs` | 预览脱敏、字段 diff | S5 |
| `src-tauri/crates/assistant-vault/src/sync_engine/{mod,lifecycle,remote_admin}.rs` | git 生命周期、远端管理 | S3 |
| `src-tauri/crates/assistant-vault/src/audit/mod.rs` | `AuditEvent`、写 `security_audit_log` | S4、S5 |
| `src-tauri/crates/assistant-vault/src/biometric/mod.rs` | `BiometricKeyStore` trait、平台分派 | S1、S4 |
| `src-tauri/crates/assistant-vault/src/biometric/{windows,macos,linux}.rs` | 平台 keystore 实现 | S1、S4 |
| `src-tauri/crates/assistant-vault/src/dto/{error,convert,recovery,integrity,totp,ssh,audit,sync}.rs` | 错误码、契约、恢复/完整性 DTO | S1–S6 |
| `src-tauri/crates/assistant-vault/src/types/mod.rs` | `VaultEntrySnapshot` / `VaultEntryExport` / `VaultCryptoInfo` / `VaultError` 再导出 | S2、S5 |
| `src-tauri/crates/assistant-vault/src/shared/{mod,totp}.rs` | TOTP 存储格式与码计算 | S2 |
| `src-tauri/crates/assistant-vault/src/password_strength/mod.rs` | 主密码强度门槛（init/改密） | S1 |
| `src-tauri/crates/assistant-vault/src/lib.rs` | 模块导出边界 | S1–S6 |
| `src-tauri/crates/assistant-vault/tests/vault_crud.rs` | 条目 CRUD 加密回归 | S2 |
| `src-tauri/crates/assistant-vault/tests/sync_common/mod.rs` | 同步测试夹具（master key、meta） | S3 |

### 1.3 `assistant-security`（系统集成）

| 文件 | 加密职责 | 归属 |
|---|---|---|
| `src-tauri/crates/assistant-security/src/vault_crypto/mod.rs` | `pub use assistant_crypto::vault_crypto::*` 转发 | S1 |
| `src-tauri/crates/assistant-security/src/clipboard_guard.rs` | 剪贴板定时清除、保留原内容 | S5 |
| `src-tauri/crates/assistant-security/src/reveal_guard.rs` | 揭示速率限制（3 次/60s） | S5 |
| `src-tauri/crates/assistant-security/src/screen_lock/*` | 系统锁屏事件 → 锁定保险箱 | S4 |
| `src-tauri/crates/assistant-security/src/unlock_dialog/*` | 原生口令输入窗口（口令明文驻留） | S4、S5 |
| `src-tauri/crates/assistant-security/src/ssh_agent/*` | 私钥注入 agent、指纹、平台桥 | S5 |
| `src-tauri/crates/assistant-security/src/hibp/mod.rs` | HIBP k-anonymity 网络请求 | S5 |

### 1.4 `assistant-config`（配置）

| 文件 | 加密职责 | 归属 |
|---|---|---|
| `src-tauri/crates/assistant-config/src/vault_conf.rs` | `VaultConf`：自动锁、剪贴板秒数、算法、同步 URL、SSH 选项 | S4、S6 |
| `src-tauri/crates/assistant-config/tests/vault_conf.rs` | 配置默认值/迁移/非法值回归 | S6 |

### 1.5 `src-tauri`（命令层与装配）

| 文件 | 加密职责 | 归属 |
|---|---|---|
| `src-tauri/src/cmd/vault/mod.rs` | 状态装配、`with_unlocked`、锁定时副作用、路径、屏幕锁监听 | S4、S6 |
| `src-tauri/src/cmd/vault/lifecycle.rs` | `vault_init` / `vault_unlock` / `vault_lock` / 原生解锁窗口 | S1、S4 |
| `src-tauri/src/cmd/vault/entries.rs` | 条目 CRUD、改密、导入/导出（明文） | S2、S3、S5 |
| `src-tauri/src/cmd/vault/recovery.rs` | 恢复码生成、完整性校验、灾难导出 | S1、S5 |
| `src-tauri/src/cmd/vault/biometric.rs` | 生物识别命令 | S1、S4 |
| `src-tauri/src/cmd/vault/reveal.rs` | 复制/揭示密码与备注 | S5 |
| `src-tauri/src/cmd/vault/audit.rs` | TOTP、审计查询、HIBP、强度检查 | S4、S5 |
| `src-tauri/src/cmd/vault/ssh.rs` | SSH 私钥解密、注入 agent、自动增删 | S5 |
| `src-tauri/src/cmd/vault/sync.rs` | 同步命令、远端状态 | S3 |
| `src-tauri/src/cmd/vault/conf.rs` | 读取 `VaultConf` | S6 |
| `src-tauri/src/vault_plugin.rs` | `feature = "vault"` 门禁、命令注册、退出清理 | S6 |
| `src-tauri/src/app_dirs.rs` | `security/`、`security/vault/`、`security/recovery/` 路径 | S6 |
| `src-tauri/src/adapters/input_source.rs` | 解密 provider API key | S5 |
| `src-tauri/src/cmd/provider.rs`、`src-tauri/src/cmd/compaction.rs` | provider API key 加解密 | S5 |

### 1.6 前端（`src/vault` 与相关）

| 文件 | 加密职责 | 归属 |
|---|---|---|
| `src/vault/core/vault.service.ts` | 全部 IPC 调用（含恢复/导出） | S1–S6 |
| `src/vault/core/vault.types.ts` | ts-rs barrel（契约类型） | S1–S6 |
| `src/vault/core/contract/vault.contract.ts` | 生成契约（禁止手改） | S1–S6 |
| `src/vault/core/vaultError.ts` | 错误码数组与判定 | S1、S6 |
| `src/vault/core/vaultStore.ts` | Zustand 根 store | S4 |
| `src/vault/core/slices/vaultLifecycleSlice.ts` | `isLocked`、自动锁定时器 | S4 |
| `src/vault/core/slices/vaultRevealSlice.ts` | 明文 `revealedField` 驻留 | S4、S5 |
| `src/vault/core/slices/vaultRecoverySlice.ts` | 恢复码/灾难导出状态 | S5 |
| `src/vault/core/slices/vaultBiometricSlice.ts` | 生物识别缓存状态 | S4 |
| `src/vault/core/slices/vaultEntriesSlice.ts` | 条目摘要/详情（含明文标题/用户名/URL） | S4、S5 |
| `src/vault/core/slices/vaultAuditSlice.ts` | 审计列表 | S4 |
| `src/vault/core/useVaultAutoLock.ts` | 用户活动重置自动锁 | S4 |
| `src/vault/core/useVaultUnlock.ts` | 解锁流程 | S1 |
| `src/vault/unlock/{PasswordForm,BiometricButton,DisasterRecovery}.tsx` | 口令/生物识别/灾难恢复入口 | S1、S5 |
| `src/vault/settings/{RecoveryTab,ChangePasswordTab,DangerZoneTab,ImportExportTab}.tsx` | 恢复码、改密、导入导出 | S1、S5 |
| `src/vault/detail/{VaultEntryDetail,RevealFieldButton,TotpDisplay}.tsx` | 明文展示与揭示 | S5 |
| `src/vault/VaultList.tsx`、`VaultEntryRow.tsx`、`VaultToolbar.tsx` | 明文摘要展示、搜索 | S5 |
| `src/features/vault/module.ts`、`src/services/featureIpc.ts` | 能力门禁与 IPC 通道 | S6 |
| `src/i18n/locales/*/vault.json` | 警告文案 | S5 |

---

## 2. 受影响部分矩阵

### 2.1 密钥材料

| 材料 | 生成 | 存放 | 生命周期 | 销毁 | 归属 |
|---|---|---|---|---|---|
| 主口令明文 | 用户输入 | IPC → `SecureString` / 前端 state | 单次调用 | Drop 清零（`SecureString`） | S1、S4 |
| `master_key` | `Argon2id(口令, master_key_salt)` | 进程内存 `Mutex<Option<MasterKey>>` | 解锁 → 锁定 | `Zeroize` + `Drop` | S1、S4 |
| `bootstrap_key` | `Argon2id(口令, BOOTSTRAP_SALT)` | 进程内存 | 解锁 → 锁定 | 同上 | S1、S4 |
| `master_key_hash` | `Argon2id(口令, hash_salt)` | `security.db` 的 `vault_meta` | 持久 | 随库删除 | S1 |
| `master_key_salt` / `hash_salt` | 随机 32B | `security.db` 的 `vault_meta` | 持久 | 改密时替换 | S1 |
| `BOOTSTRAP_SALT` | 硬编码常量 | 二进制 | 永久 | 不可变 | S1 |
| `entry_key` | `HKDF(master_key, entryId)` | 栈上临时 | 单次加解密 | `PerEntryKey::Drop` | S2 |
| SQLCipher 页密钥 | `HKDF(bootstrap_key, "sqlcipher")` | 栈上临时 | 打开连接 | 局部 `zeroize` | S1、S4 |
| 恢复码 | 随机 20B | 用户纸面 | 用户保管 | 重新生成 | S1、S5 |
| `emergency.token` | `AES-GCM(recovery_key, BK‖MK)` | `.vault-meta`（进 git） | 持久 | 改密时清除 | S1、S3 |
| 生物识别 payload | `BK‖MK` 64B | OS keystore（包裹后） | 注册 → 反注册 | 反注册/改密 | S1、S4 |
| provider API key | 用户输入 | provider 库 `api_key_encrypted` | 持久 | 用户删除 | S5 |
| TOTP secret | 用户输入 | 条目 blob / DB 列 | 持久 | 条目删除 | S2 |
| SSH 私钥 | 用户输入 | 条目 blob | 持久 | 条目删除 | S2、S5 |
| 同步远端凭据 | 用户输入 | `vault-config.json` 的 `vault_sync_remote_url` | 持久 | 用户清除 | S3、S6 |

### 2.2 密文与认证点

| 密文/认证 | 算法 | 位置 | 认证对象 | 归属 |
|---|---|---|---|---|
| `security.db` 整库 | SQLCipher（AES-256-CBC + HMAC-SHA512，`kdf_iter=256000`） | 本机 `security/security.db` | 整页 | S2、S4 |
| 条目字段（DB） | AES-256-GCM / ChaCha20-Poly1305 | `encrypted_password` / `encrypted_notes` / `encrypted_totp_secret` / `encrypted_custom_fields` | 单字段 | S2 |
| 条目文件（仓库） | AES-256-GCM / ChaCha20-Poly1305 | `entries/{id}.enc` 的 `encryptedBlob` | 整个 `EntryBlob` JSON | S2、S3 |
| `.vault-meta` 完整性 | HMAC-SHA256(`master_key`, 规范化 JSON) | `.vault-meta.integrity` | 全字段 | S3 |
| 恢复封装 | AES-256-GCM(Argon2id(恢复码), `BK‖MK`) | `.vault-meta.emergency.token` | payload | S1、S3 |
| 生物识别包裹 | AES-256-GCM(HKDF(签名)) | OS keystore `vault-master-ciphertext` | `BK‖MK` | S1、S4 |
| API key | AES-256-GCM(`SHA-256(machine_uid)`) | provider 库 | key | S5 |

### 2.3 明文驻留点

| 位置 | 明文内容 | 清理时机 | 归属 |
|---|---|---|---|
| 进程内存 | `master_key` / `bootstrap_key` / SQLCipher 页缓存 | 锁定 | S4 |
| 进程内存 | 解密后的 `VaultEntrySnapshot` / `VaultEntryDecrypted` | 函数返回后 Drop | S4 |
| IPC | `VaultEntryDetail` / `RevealResult` / 导入导出 DTO | 前端消费后 | S4、S5 |
| 前端 store | 条目摘要（title/username/url/tags）、详情、`revealedField.value` | 锁定/超时 | S4、S5 |
| 剪贴板 | 密码/字段/TOTP 码 | 定时器或锁定 | S5 |
| 导出文件 | JSON/CSV 全字段明文 | 用户自行处置 | S5 |
| 灾难恢复导出 | 全条目 JSON 明文 | 用户自行处置 | S5 |
| 日志 | 事件、条目 ID、错误消息 | 无自动清理 | S4 |
| 临时文件 | `write_atomically` 的 `.tmp`；恢复目录 `security/recovery/` 的 clone | 崩溃可能残留 | S3、S5 |
| 备份 | `security/backups/security-*.db.bak` | 保留 5 份 | S4 |
| HIBP 请求 | SHA-1 前缀（k-anonymity） | 网络 | S5 |
| SSH agent | 解密后的私钥 | 锁定/生命周期 | S5 |

### 2.4 命令面（对外能力）

| 类别 | 命令 | 明文/密钥出口 | 归属 |
|---|---|---|---|
| 生命周期 | `vault_init` / `vault_unlock` / `vault_lock` / `vault_show_unlock_window` | 口令入口 | S1、S4 |
| 条目 | `vault_create_entry` / `vault_update_entry` / `vault_get_entry` / `vault_list_entries` | 明文 DTO | S2、S4 |
| 明文出口 | `vault_copy_password` / `vault_copy_field` / `vault_reveal_password` / `vault_reveal_notes` / `vault_copy_totp` | 明文 | S5 |
| 批量出口 | `vault_export_to_file` / `vault_import` | 明文文件 | S5 |
| 恢复 | `vault_generate_recovery_key` / `vault_has_recovery_key` / `vault_verify_vault_meta_integrity` / `vault_disaster_recovery_export` | 恢复码/明文文件 | S1、S5 |
| 同步 | `vault_sync` / `vault_resolve_conflict` / `vault_conflict_preview` / `vault_conflict_reveal` / `vault_history_*` | 密文/预览明文 | S3、S5 |
| 生物识别 | `vault_enroll_biometric` / `vault_unlock_biometric` / `vault_disenroll_biometric` | keystore | S1、S4 |
| SSH | `vault_ssh_*` | 私钥 | S5 |
| 审计/检测 | `vault_query_audit_log` / `vault_hibp_*` | 事件/哈希前缀 | S4、S5 |

---

## 3. 现状缺陷清单

编号用于配套规格的「来源」列。严重度：P0 数据/密钥可被错误处理；P1 语义缺口；P2 加固项。

| 编号 | 严重度 | 缺陷 | 证据（仓库相对路径） | 归属 |
|---|---|---|---|---|
| **F1** | P1 | `EntryFile` 的 `id` / `category` / `revision` 在密文之外，未进 GCM AAD，仓库可被整块重放或篡改 revision | `src-tauri/crates/assistant-vault/src/vault_file/types.rs:3-10`、`vault_file/mod.rs:139-166` | entry-cipher |
| **F2** | P2 | `EntryBlob` 的 `totpDigits`/`totpPeriod` 是**冗余字段**：`export_entry` 硬编码 6/30，`ImportedEntry::into_import` 完全忽略它们；真实参数藏在 `totpSecret` 的 JSON payload 里（`shared/totp.rs:7-14,59-67`），因此**不丢数据**，但字段语义误导、后续容易被误用 | `vault_file/types.rs:12-40`、`vault_file/mod.rs:95-96,390-417` | entry-cipher |
| **F3** | P0 | 解密失败被静默吞掉：`decrypt_opt` 用 `unwrap_or_default()`，受保护自定义字段用 `.ok()?` 直接丢弃 | `entries/field_crypto.rs:19-29`、`security_db/crypto_ops.rs:225-254` | entry-cipher |
| **F4** | P1 | `vault_entries` 的 `title` / `username` / `url` / `tags` 是明文字段（仅 SQLCipher 整库保护），字段级加密策略未成文 | `security_db/schema.rs:9-27`、`entries/write.rs:160-181` | entry-cipher |
| **F5** | P0 | `vault_export_to_file` 把密码/备注/TOTP/自定义字段写成 JSON/CSV 明文，无二次确认、无审计、无文件权限设置 | `src-tauri/src/cmd/vault/entries.rs:302-352` | egress |
| **F6** | P1 | `vault_import` 只写库，不立即写 `entries/*.enc` 也不提交；导入后若发生重建/回滚，导入数据可能只存在于本机库 | `src-tauri/src/cmd/vault/entries.rs:354-405` | sync-crypto |
| **F7** | P0 | 冲突/历史预览直接返回 `notes`/`title`/`username`/`url` 明文；`vault_conflict_reveal` 无审计、无限速 | `sync_engine/preview.rs:9-47`、`sync_engine/history.rs:263-308` | egress |
| **F8** | P1 | 剪贴板守卫把明文密码和原剪贴板内容长期保存在 `Arc<Mutex<String>>`，无 `zeroize` | `src-tauri/crates/assistant-security/src/clipboard_guard.rs:8-117` | runtime |
| **F9** | P1 | 揭示的明文保存在前端 Zustand `revealedField.value`，TTL 固定 30s，锁定外无主动清零 | `src/vault/core/slices/vaultRevealSlice.ts:41-67`、`src-tauri/src/cmd/vault/reveal.rs:7` | runtime |
| **F10** | P2 | SQLCipher 未设置 `cipher_memory_security` / `temp_store` 等内存与临时表加固项 | `security_db/mod.rs:27-28` | runtime |
| **F11** | P1 | API key 的加密密钥是 `SHA-256(machine_uid)`，machine_uid 同机可读，属混淆而非保密 | `assistant-crypto/src/api_key.rs:9-48` | egress |
| **F12** | P1 | 灾难恢复把远端仓库 clone 到 `security/recovery/`，成功后不清理 | `src-tauri/src/cmd/vault/recovery.rs:64-207`、`cmd/vault/mod.rs:33-35` | egress |
| **F13** | P0 | 改密前的 `security-*.db.bak` 保留旧 `bootstrap_key`/`master_key` 派生材料，旧口令仍可打开旧备份并读出条目 | `security_db/rekey.rs:44`、`security_db/mod.rs:248-330` | runtime |
| **F14** | P1 | SSH 自动注入/自动移除不写审计；agent 中的私钥生命周期只由锁定清理 | `src-tauri/src/cmd/vault/ssh.rs:223-281` | egress |
| **F15** | P1 | `SecureString::into_inner` 用 `mem::forget` 移交 `String`，后续副本不再受 Drop 清零保护 | `assistant-crypto/src/vault_crypto/types.rs:88-93` | runtime |
| **F16** | P2 | `.vault-meta` 的 HMAC 直接以 `master_key` 为密钥，无域分离；版本号未校验 | `vault_file/mod.rs:432-449`、`vault_file/mod.rs:267-270` | sync-crypto |
| **F17** | P1 | `with_unlocked` 先 `drop` 连接守卫再执行业务，锁定与业务之间存在竞态窗口 | `src-tauri/src/cmd/vault/mod.rs:168-175` | runtime |
| **F18** | P1 | 同步远端 URL 可能存在 `vault-config.json` 中且含内嵌凭据，错误/日志无脱敏约定 | `assistant-config/src/vault_conf.rs:52`、`cmd/vault/mod.rs:61-77` | sync-crypto |
| **F19** | P2 | 审计只写 `security.db`，恢复通道/未解锁路径不可审计 | `assistant-vault/src/audit/mod.rs:61-76` | runtime |
| **F20** | P0 | 旧库（v1 schema/密钥体系）无显式拒绝与重置入口 | `security_db/mod.rs:116-196`、`vault_file/mod.rs:267-270` | capability |
| **F21** | P1 | `vault_crypto_info` 仍从 `security.db` 读 `master_key_salt`/`hash_salt` 并写进 `.vault-meta`；v1.5 无此契约 | `security_db/entries/query.rs:132-155`、`sync_engine/export.rs:27-29` | sync-crypto |
| **F22** | P1 | **不存在** `VaultStatusDto` / `vault_get_status`；「是否已备份纸面」没有查询通道 | 前端与命令层均无匹配 | recovery-channel |

> F3/F13 是**当前代码就存在**的数据正确性与密钥卫生问题，与本轮是否采纳 v1.5 无关，建议优先修。F2 是字段冗余，不构成数据丢失，排在 P2。

---

## 4. 规格索引

| 规格 | 覆盖缺陷 | 覆盖面 |
|---|---|---|
| [I9-vault-key-hierarchy-spec.md](./I9-vault-key-hierarchy-spec.md) | 核心：值类型、`derive_*`/`seal_*`、`.vault-meta` v2、init/unlock/change/rotate、核心错误码 | S1 |
| [I9-vault-entry-cipher-spec.md](./I9-vault-entry-cipher-spec.md) | F1、F2、F3、F4 | S2 |
| [I9-vault-sync-crypto-spec.md](./I9-vault-sync-crypto-spec.md) | F6、F16、F18，以及原 G22 | S3 |
| [I9-vault-runtime-session-spec.md](./I9-vault-runtime-session-spec.md) | F8、F9、F10、F13、F15、F17、F19 | S4 |
| [I9-vault-plaintext-egress-spec.md](./I9-vault-plaintext-egress-spec.md) | F5、F7、F11、F12、F14 | S5 |
| [I9-vault-biometric-spec.md](./I9-vault-biometric-spec.md) | 生物识别 payload、keystore、重包、R8/R9、删除语义 | S1、S4 |
| [I9-vault-recovery-channel-spec.md](./I9-vault-recovery-channel-spec.md) | 纸面编解码、`RecoverySession`、恢复命令/DTO/UI | S1、S5 |
| [I9-vault-audit-spec.md](./I9-vault-audit-spec.md) | 审计事件、JSONL 双写、补录 | S4、S5 |
| [I9-vault-capability-gating-spec.md](./I9-vault-capability-gating-spec.md) | F20，以及 feature/conf/schema 门禁 | S6 |

---

## 5. 测试与回归资产

| 资产 | 现状 | 缺口 |
|---|---|---|
| `src-tauri/crates/assistant-crypto/tests/vault_crypto.rs` | 原语往返、错误路径 | 无 AAD/域分离/零化断言 |
| `src-tauri/crates/assistant-crypto/tests/api_key.rs` | machine-id 加解密 | 无「同机可解密」的性质断言 |
| `src-tauri/crates/assistant-vault/tests/vault_crud.rs` | 条目 CRUD | 无「密文列不得含明文」断言 |
| `src-tauri/crates/assistant-vault/tests/security_db.rs` | 本机库初始化/解锁 | 无改密后旧备份断言、无探针读断言 |
| `src-tauri/crates/assistant-vault/tests/vault_file.rs` | `.vault-meta` 与条目文件 | 无严格版本解析、无 AAD/重放断言 |
| `src-tauri/crates/assistant-vault/tests/recovery_key.rs` | **v1** 32 字符恢复码 | 必须重写为 40 字符纸面（T1–T5b） |
| `src-tauri/crates/assistant-vault/tests/command_registry.rs` | 命令声明 ↔ `generate_handler!` 一致性 | 新增/删除命令后自动覆盖 |
| `src-tauri/crates/assistant-vault/tests/command_gate.rs` | feature 门禁 | 需补「关闭 vault 时无命令」断言 |
| `src-tauri/crates/assistant-vault/tests/conf_keys.rs` | 配置键一致性 | 新增 conf 字段需同步 |
| `src-tauri/crates/assistant-vault/tests/dto_contract.rs` | 错误码契约 | 新增错误码需同步 |
| `src-tauri/crates/assistant-vault/tests/sync_engine.rs` / `vault_sync_db.rs` | 同步集成 | 无 manifest/回滚/冲突明文断言 |
| `src-tauri/crates/assistant-vault/tests/sync_common/mod.rs` | 同步夹具 | 仍以 `MasterKey` 构造，需换 `Dek` |
| `src/vault/core/__tests__/vaultStore.test.ts` | 前端 store | 无「锁定时明文必须清空」断言 |
| `src/vault/core/__tests__/syncDisplay.test.ts` | 同步展示 | 无预览脱敏断言 |

---

## 6. 维护约定

1. 新增任何**密钥材料**、**密文落点**或**明文出口**，必须先更新本清单的第 2 节，再写实现。
2. 新增命令必须在第 2.4 节点名，并标注它是否出口明文。
3. 本清单不复制设计论证；与设计冲突时以设计文档的决策记录为准，并在对应规格的「差异记录」里写明。
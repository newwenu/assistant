# I9 密码箱 — 实施任务清单（v1.5）

> 状态：待执行（2026-11-11）
> 基线：当前代码；v1.5 尚未实施。
> 依据规格：
> [核心](./I9-vault-key-hierarchy-spec.md)、[条目](./I9-vault-entry-cipher-spec.md)、[同步](./I9-vault-sync-crypto-spec.md)、[运行时](./I9-vault-runtime-session-spec.md)、[明文出口](./I9-vault-plaintext-egress-spec.md)、[生物识别](./I9-vault-biometric-spec.md)、[恢复通道](./I9-vault-recovery-channel-spec.md)、[审计](./I9-vault-audit-spec.md)、[能力门禁](./I9-vault-capability-gating-spec.md)。
> 路径约定：全部为仓库相对路径。
> 状态取值：`未开始` / `进行中` / `完成` / `阻塞`。

---

## 0. 里程碑与发布门禁

| 里程碑 | 内容 | 可发布 | 依赖 |
|---|---|---|---|
| **M0** | 当前代码缺陷修复（与本轮密钥体系无关） | 是（独立合入） | 无 |
| **M1** | P0-1：`kdf_iter` 与 PRAGMA | 是 | M0-1 |
| **M2** | P0-2：`dek` + `.vault-meta` v2 + 改密 O(1) | **否** | M1 |
| **M3** | P0-3：纸面恢复通道 + 审计 JSONL | **是**（M2+M3 一起） | M2 |
| **M4** | P1-1：manifest + anchor + 同步整合 | 是 | M3 |
| **M5** | P1-2：生物识别 52B + 探针 + 重包 | 是 | M4 |
| **M6** | P1-3：`rotate_dek` + 强制重注册 | 是 | M5 |
| **M7** | 运行时/同步/导出/配置加固 | 是 | 可并行 |

**发布门禁**：

- M2 与 M3 之间**没有恢复通道**，不得单独发布；
- M2 合并前必须完成 M0-1（解密失败不再静默），否则新库的错误会被掩盖；
- M6 合并前必须完成 M5-4（`delete_key` 可失败语义），否则 R8 是假约束。

---

## 1. M0：当前代码缺陷修复

> 这些任务不依赖 v1.5，可以先合入主干。

| 编号 | 任务 | 主要文件 | 依赖 | 验收 | 规格 |
|---|---|---|---|---|---|
| **M0-1** | 解密失败不再静默：`decrypt_opt` 去掉 `unwrap_or_default`；受保护自定义字段失败不再 `.ok()?` 丢弃；整条解密失败返回 `EntryCorrupted`（或现有 `Crypto`，若暂不新增错误码） | `security_db/entries/field_crypto.rs`、`security_db/crypto_ops.rs` | 无 | 截断/非法 UTF-8/字段互换的密文均报错；新增回归用例 | entry-cipher E3、§5.3 |
| **M0-2** | 改密成功后处理旧备份：删除改密前的 `security-*.db.bak`，或对新库/备份重新 rekey；固定一种策略，不留给用户 | `security_db/rekey.rs`、`security_db/mod.rs` | 无 | 改密后用旧口令无法打开任何残留备份；`security_db.rs` 新增断言 | runtime §4.3、F13 |
| **M0-3** | 导出护栏：`vault_export_to_file` 增加 `confirm_token("EXPORT-PLAINTEXT")`、路径校验、0600、临时文件 + rename、审计 | `src-tauri/src/cmd/vault/entries.rs`、`src/vault/settings/ImportExportTab.tsx` | 无 | 未传 token/非法路径被拒；导出后文件权限正确 | egress §2.1 |
| **M0-4** | 预览与冲突揭示：`mask_entry_preview` 脱敏 `notes`；`vault_conflict_reveal` 接 `RevealGuard` + 审计 + TTL | `sync_engine/preview.rs`、`sync_engine/history.rs`、`cmd/vault/sync.rs` | 无 | notes 显示 `•••`；连点 4 次触发 `RateLimited` | egress §1.3、sync §2.7 |
| **M0-5** | 灾难恢复临时目录清理：成功/失败都删除 `security/recovery/` | `cmd/vault/recovery.rs` | 无 | 导出后目录不存在 | egress §2.3、F12 |
| **M0-6** | 导入一致性：`vault_import` 写 `entries/*.enc` 并 `finalize`，不再只写库 | `cmd/vault/entries.rs` | 无 | 导入后立即检查 `.enc` 与 git 提交 | entry-cipher §5.2、F6 |
| **M0-7** | 回归测试：为 M0-1–M0-6 增加断言 | `tests/security_db.rs`、`tests/vault_file.rs`、`tests/egress_guard.rs`（新增） | M0-1–M0-6 | 全部通过 | 各规格测试节 |

---

## 2. M1：P0-1（`kdf_iter` 与 PRAGMA）

| 编号 | 任务 | 主要文件 | 依赖 | 验收 | 规格 |
|---|---|---|---|---|---|
| **M1-1** | 写 `sqlcipher_kdf_iter_one_is_noop` 测试，验证 raw key 下 `kdf_iter` 语义 | `tests/security_db.rs` 或新增 `tests/sqlcipher_pragma.rs` | 无 | 两个断言通过；失败则按规格回退方案 | key §11.1 T-P0-1 |
| **M1-2** | `SQLCIPHER_PRAGMAS` 改 `kdf_iter = 1`，去重，补 T-P0-2/T-P0-3 | `security_db/mod.rs` | M1-1 | 无重复键；包含 `kdf_iter=1` | key §11.1 |
| **M1-3** | `recovery_key` 里的 PRAGMA 字面量改为走 `SQLCIPHER_PRAGMAS` | `recovery_key/mod.rs` | M1-2 | 全仓无重复 PRAGMA 字面量 | key §6.4 |

---

## 3. M2：P0-2（核心切换）

> 这一阶段是**原子切换**：类型、原语、meta、本机库、编排、条目格式、错误码同批合入；中间态不可发布。

### 3.1 类型与原语

| 编号 | 任务 | 主要文件 | 依赖 | 验收 | 规格 |
|---|---|---|---|---|---|
| **M2-1** | 新增 `Dek` / `Kek` / `DbKey` / `MetaKey`（`Zeroize + Drop`，禁 `Clone/Copy/Serialize`） | `assistant-crypto/src/vault_crypto/types.rs` | 无 | 类型单测；`Debug` 脱敏 | key §2.1–2.2 |
| **M2-2** | 新增 `derive_kek` / `derive_db_key` / `derive_entry_key` / `derive_meta_key` / `seal_dek` / `open_dek` / `build_slot_aad` / `canonical_meta_bytes`；删除 v1 `derive_master_key` / `verify_master_password` / `derive_sqlcipher_key` 的生产调用 | `assistant-crypto/src/vault_crypto/mod.rs` | M2-1 | T-G1/T-G2 黄金向量通过 | key §3.1–3.7 |

### 3.2 `.vault-meta` v2

| 编号 | 任务 | 主要文件 | 依赖 | 验收 | 规格 |
|---|---|---|---|---|---|
| **M2-3** | 新增 `VaultMetaFileV2` / `VaultSlot` / `KdfParams` / `VaultManifest` / `ManifestEntry` / `RecoveryBackup`；删除 `VaultCryptoMeta` / `EmergencyDataFile` | `vault_file/types.rs` | M2-1 | 序列化字段与规格一致 | key §2.3 |
| **M2-4** | 实现 `read_vault_meta_v2`（严格解析、v1 拒绝）、`write_vault_meta_v2`（原子写 + 签名）、`compute_manifest`、`meta_file_hash`、`verify_manifest`；删除 5 个 v1 meta 函数 | `vault_file/mod.rs` | M2-2、M2-3 | I5/I10 通过；不修改旧文件 | key §4.2–4.4 |

### 3.3 本机库与编排

| 编号 | 任务 | 主要文件 | 依赖 | 验收 | 规格 |
|---|---|---|---|---|---|
| **M2-5** | `SecurityDbState` 合并为 `session: Mutex<Option<VaultSession>>` + `epoch`；`with_session`；`init`/`unlock` 改签名；加探针读 | `security_db/mod.rs`、`security_db/entries/tx.rs` | M2-2 | 锁定无半状态；探针失败 → `LocalDbKeyMismatch` | runtime §2、key §4.5 |
| **M2-6** | 新增 `lifecycle.rs`：`initialize` / `unlock` / `change_password` / `rotate_dek` 骨架 | `assistant-vault/src/lifecycle.rs`、`lib.rs` | M2-4、M2-5 | 命令层不再拼跨层顺序 | key §4.1 |
| **M2-7** | `change_master_password` 改 O(1)：只重包 + `PRAGMA rekey`，删除全量条目重加密；处理旧备份 | `security_db/rekey.rs` | M2-6、M0-2 | I2/I3 通过；条目文件 sha256 不变 | key §4.8 |
| **M2-8** | `LocalDbKeyMismatch` 两条修复入口 | `security_db/rekey.rs` | M2-7 | I17 两条路径各成功一次 | key §4.7 |
| **M2-9** | `anchor` 三键读写 + `get_anchor`/`set_anchor` + 校验顺序 | `security_db/meta.rs` | M2-5 | I4 回滚检测通过 | key §4.6 |

### 3.4 条目层

| 编号 | 任务 | 主要文件 | 依赖 | 验收 | 规格 |
|---|---|---|---|---|---|
| **M2-10** | `EntryFileV2` / `EntryBlobV2` / `TotpBlob`；条目文件 AAD；字段级 AAD（`password` / `notes` / `totp` / `custom:{name}` / `ssh-private-key`） | `vault_file/types.rs`、`vault_file/mod.rs`、`security_db/entries/field_crypto.rs` | M2-2、M2-4 | T-E1–T-E12 通过 | entry-cipher §2 |
| **M2-11** | 替换全部 `derive_entry_key` 调用点（17 处/7 文件）；`tx.rs` 回调改 `&VaultSession` | `security_db/{crypto_ops,entries/*}.rs`、`vault_file/mod.rs` | M2-10 | 全仓无 `MasterKey` 实参残留 | entry-cipher §3、key §4.10 |
| **M2-12** | 删除 v1 死代码：`BOOTSTRAP_SALT`、`hash_salt`、第三次 Argon2id、`emergency` 全链路、`vault_crypto_info`、v1 生物识别 key id、v1 恢复码 | 跨 `assistant-vault`、命令层、前端 | M2-7–M2-11 | 编译通过；无 v1 字段残留 | key §4.2、biometric §2、recovery §0.2 |

### 3.5 契约与命令

| 编号 | 任务 | 主要文件 | 依赖 | 验收 | 规格 |
|---|---|---|---|---|---|
| **M2-13** | 核心错误码 5 项 + `convert.rs` 穷尽映射 + 前端数组 | `assistant-crypto/src/error.rs`、`dto/error.rs`、`dto/convert.rs`、`src/vault/core/vaultError.ts` | M2-6 | T-D1 双向一致 | key §7 |
| **M2-14** | 命令层：`vault_init` / `vault_unlock` / `vault_lock` / `vault_change_master_password` 改调 `lifecycle`；新增 `vault_rotate_dek`（骨架）；`vault_reset_repository` | `cmd/vault/lifecycle.rs`、`cmd/vault/entries.rs`、`vault_plugin.rs` | M2-6、M2-13 | command_registry 通过 | key §9、capability §3.2 |
| **M2-15** | P0-2 验收：I1–I5、I10、T-G1/T-G2/T-D1 | `tests/lifecycle_integration.rs`（新增）、`tests/dto_contract.rs` | M2-1–M2-14 | 全部通过 | key §11.2 |

---

## 4. M3：P0-3（恢复通道 + 审计）

| 编号 | 任务 | 主要文件 | 依赖 | 验收 | 规格 |
|---|---|---|---|---|---|
| **M3-1** | 纸面编解码：Crockford Base32 + CRC32C + 归一化 + 纠错定位；新增 `crc` 依赖 | `recovery_key/mod.rs`、`assistant-vault/Cargo.toml`、workspace `Cargo.toml` | M2-1 | T1–T5b 通过 | recovery §1 |
| **M3-2** | `RecoverySession`（`dek` + `entries_dir` + `manifest`）+ `open_recovery_session` + `export_all`；删除 v1 恢复函数 | `recovery_key/mod.rs` | M3-1、M2-4 | I7–I11 通过；不触碰 `security.db` | recovery §2–§3 |
| **M3-3** | 恢复 DTO + 命令：`vault_export_recovery_key` / `vault_recover_from_paper` / `vault_locate_paper_error` / `vault_has_recovery_backup`；删除 4 个 v1 命令 | `dto/recovery.rs`、`dto/mod.rs`、`cmd/vault/recovery.rs`、`vault_plugin.rs` | M3-2、M2-13 | command_registry 通过 | recovery §4 |
| **M3-4** | 审计基础设施：`AuditEvent` 增删、`audit/jsonl.rs`、`log_dual`、解锁补录 | `audit/mod.rs`、`audit/jsonl.rs`、`security_db/mod.rs` | M2-5 | T-A1–T-A10 通过 | audit §1–§4 |
| **M3-5** | 前端：`RecoveryKeyDialog` / `PaperRecoveryFlow` / `RecoveryKeyForm`；`RecoveryTab` / `DisasterRecovery` 改造；service 方法；i18n | `src/vault/**` | M3-3 | 手抄 40 字符可用；定位错误到组位 | recovery §5 |
| **M3-6** | 重写 `tests/recovery_key.rs` 为 40 字符纸面测试 | `tests/recovery_key.rs` | M3-1 | 旧 4 组测试全部替换 | recovery §7 |

**门禁**：M3 完成后才允许发布；M2 与 M3 之间只能内部构建。

---

## 5. M4：P1-1（manifest + anchor 同步整合）

| 编号 | 任务 | 主要文件 | 依赖 | 验收 | 规格 |
|---|---|---|---|---|---|
| **M4-1** | 条目增/删/改后立即更新 manifest；`change_password` 不重算；`rotate_dek` 全量重算 | `security_db/entries/*.rs`、`vault_file/mod.rs` | M2-10 | I6 通过 | key §4.2、sync §2.3 |
| **M4-2** | push 顺序固定：写 `.enc` → manifest → meta → commit；删除 `sync_engine` 的 meta 写入 | `sync_engine/export.rs`、`lifecycle.rs` | M4-1 | 单一写入者；T-S1 通过 | sync §2.2–§2.3 |
| **M4-3** | pull 顺序固定：git pull → 严格 meta → `vaultId` → `dekVersion` → integrity → manifest → import → anchor | `sync_engine/pull_push.rs` | M4-2 | T-S2–T-S7 通过 | sync §2.4–§2.6 |
| **M4-4** | 冲突：`.vault-meta` 不自动合并；条目预览脱敏；`removed_ids` 以 manifest 为准 | `sync_engine/history.rs`、`preview.rs`、`pull_push.rs` | M4-3、M0-4 | T-S3/T-S4/T-S8/T-S9 通过 | sync §2.5–§2.7 |
| **M4-5** | 仓库白名单 + `.gitignore` + `.tmp` 清理 + `security/recovery/` 清理 | `vault_file/mod.rs`、`cmd/vault/mod.rs`、`cmd/vault/recovery.rs` | M4-2 | T-S1/T-S11 通过 | sync §2.1、§2.9 |

---

## 6. M5：P1-2（生物识别）

| 编号 | 任务 | 主要文件 | 依赖 | 验收 | 规格 |
|---|---|---|---|---|---|
| **M5-1** | key id 改名；payload 改 52B `dek‖dbKey`；新增 `fingerprint` | `security_db/biometric_ops.rs` | M2-5 | 旧 64B payload 明确拒绝 | biometric §2–§4 |
| **M5-2** | 重写 `restore_keys_from_plaintext`：指纹 → 开库 → 探针 → anchor → 写 session | 同上 | M5-1 | I12 通过；假解锁消失 | biometric §5 |
| **M5-3** | 平台解锁入口：Windows 先校验 `vault-sig-hash` 再解密；非 Windows 分步；失败映射 `BiometricReset` | 同上 | M5-2 | T-B4/T-B5 通过 | biometric §6 |
| **M5-4** | `delete_key` 区分 not-found 与真实失败（Windows/Linux 当前恒 `Ok`） | `biometric/{windows,linux}.rs`、`mod.rs` | 无 | T-B1/T-B3 可失败 | biometric §11.2 |
| **M5-5** | 注册 / 反注册 / 自动重包；改密后失败只删 fingerprint | `biometric_ops.rs`、`lifecycle.rs` | M5-1 | T-B6 通过 | biometric §7–§8 |
| **M5-6** | R8：`rotate_dek` 第 1 步删除 `vault-payload`；失败中止 | `lifecycle.rs` | M5-4、M6-1 | T-B1 通过 | biometric §9 |
| **M5-7** | R9：平台 ACL 判定，不满足即禁用 | `biometric/*`、`biometric_ops.rs` | 无 | 不满足平台返回 `BiometricUnavailable` | biometric §10 |
| **M5-8** | `vault_reset_repository` 清空 4 个 key id；失败中止 | `cmd/vault/recovery.rs` 或 `lifecycle.rs` | M5-1、M5-4 | T-B2/T-B3 通过 | biometric §11.1 |
| **M5-9** | 前端：`BiometricReset` 只认该错误码，不再用 `crypto`/`notFound`；强制重注册引导 | `vaultBiometricSlice.ts`、`ChangePasswordTab.tsx`、`BiometricButton.tsx` | M5-3 | 重置后不自动重试 | runtime §4.2、biometric §12 |

---

## 7. M6：P1-3（`rotate_dek`）

| 编号 | 任务 | 主要文件 | 依赖 | 验收 | 规格 |
|---|---|---|---|---|---|
| **M6-1** | `lifecycle::rotate_dek`：删凭据 → 生成 `dek_new` → 全量重加密（临时目录 + 批量 rename）→ 新 slot → manifest → meta → anchor | `lifecycle.rs`、`vault_file/mod.rs` | M4-1、M5-4 | I14/I15 通过 | key §4.9 |
| **M6-2** | 命令与前端：`vault_rotate_dek` + 「轮换数据密钥」入口 + 强警告 | `cmd/vault/entries.rs`、`RecoveryTab.tsx`、`vault.service.ts` | M6-1 | 命令注册通过 | key §9.1、recovery §5.2 |
| **M6-3** | 轮换后强制重新注册生物识别；拒绝则视为停用 | `lifecycle.rs`、biometric 前端 | M5-6、M6-1 | I13/T-B1 通过 | biometric §9 |
| **M6-4** | 历史不可读：`HistoryDekUnavailable` | `sync_engine/history.rs`、`dto` | M6-1 | T-S7 通过 | sync §2.6 |

---

## 8. M7：加固与收尾

| 编号 | 任务 | 主要文件 | 依赖 | 验收 | 规格 |
|---|---|---|---|---|---|
| **M7-1** | 运行时零化：剪贴板 `Zeroizing`、`SecureString::into_zeroizing`、`cipher_memory_security`/`temp_store`（先验证支持） | `clipboard_guard.rs`、`vault_crypto/types.rs`、`security_db/mod.rs` | M2-5 | T-R5/T-R11 通过 | runtime §4.1 |
| **M7-2** | 锁定路径统一：`lock_session(reason)`；退出先锁再清剪贴板 | `security_db/mod.rs`、`cmd/vault/mod.rs`、`vault_plugin.rs` | M2-5 | T-R1–T-R9 通过 | runtime §3 |
| **M7-3** | 同步 URL 脱敏：日志/错误/状态条 | `cmd/vault/sync.rs`、`cmd/vault/mod.rs` | M4-3 | T-S12 通过 | sync §2.8 |
| **M7-4** | 配置校验与门禁：`VaultConf::validate`、`autoLock=0`、URL 形态、feature-off 行为 | `assistant-config/src/vault_conf.rs`、`command_gate.rs` | M2-14 | T-C1–T-C14 通过 | capability §2–§4 |
| **M7-5** | 恢复码展示通道加固：打印/剪贴板/二维码提示；禁止自动复制 | `RecoveryTab.tsx`、`RecoveryKeyDialog.tsx` | M3-5 | T-P9 通过 | egress §3 |
| **M7-6** | API key 文档与注释澄清「非保密」 | `assistant-crypto/src/api_key.rs` | 无 | 注释与 UI 不宣称加密保护 | egress §4.1 |
| **M7-7** | 全量回归：`cargo test`、`cargo test --test assembly_contract`、`pnpm test`、手工验收 | 全仓 | M0–M6 | 全绿 | key §11.5 |

---

## 9. 依赖关系（关键路径）

```
M0-1 ─┬─▶ M1-1 ─▶ M1-2 ─▶ M1-3
      │
      └─▶ M2-1 ─▶ M2-2 ─┬─▶ M2-3 ─▶ M2-4 ─┬─▶ M2-6 ─▶ M2-7 ─▶ M2-8
                        │                  │
                        └─▶ M2-10 ─▶ M2-11 ┘
                                           │
M2-5 ──────────────────────────────────────┘
                                           ▼
                        M2-9 / M2-12 / M2-13 / M2-14 / M2-15
                                           ▼
                        M3-1 ─▶ M3-2 ─▶ M3-3 ─▶ M3-5
                        M3-4（可与 M3-1 并行）─▶ M3-3
                                           ▼
                        M4-1 ─▶ M4-2 ─▶ M4-3 ─▶ M4-4 ─▶ M4-5
                                           ▼
                        M5-1 ─▶ M5-2 ─▶ M5-3 ─▶ M5-5
                        M5-4（可并行）─▶ M5-6 ─▶ M6-1 ─▶ M6-3
                                           ▼
                        M6-1 ─▶ M6-2 / M6-4
                                           ▼
                        M7-1 … M7-7
```

关键约束：

1. **M2 是一个原子提交批次**，不要按子任务分批发布。
2. M2-12（删除 v1 死代码）必须与 M2-10/M2-11 同批，否则中间态编译不过。
3. M5-4（`delete_key` 可失败）必须早于 M5-6/M6-1。
4. M3-4（审计 JSONL）必须早于 M5-2（生物识别失败审计）与 M3-3（恢复审计）。

---

## 10. 任务验收模板

每个任务合入前需回答：

1. 本次改动对应哪条规格的哪一节？
2. 新增/修改了哪些文件？是否已在 `I9-vault-crypto-surface-inventory.md` 登记？
3. 是否新增了命令、DTO、错误码、配置项？若新增，是否同步了 contract / 前端数组 / conf 测试？
4. 是否触碰 `.vault-meta` 或 `entries/*.enc` 格式？若是，是否同步了 manifest / anchor / 版本判定？
5. 是否有对应的测试？测试文件在哪？
6. 是否引入了规格「禁止事项」中的行为？

---

## 11. 建议的首批执行顺序

如果只能先做 5 件事：

1. **M0-1**：解密失败不再静默（数据正确性最高优先级）。
2. **M0-2**：改密后旧备份处理（密钥卫生）。
3. **M1-1/M1-2**：`kdf_iter` 验证与 PRAGMA 修正（最小、独立）。
4. **M2-1/M2-2**：类型与原语（后续所有工作的地基）。
5. **M3-4**：审计 JSONL（恢复与生物识别的可观测性前提）。
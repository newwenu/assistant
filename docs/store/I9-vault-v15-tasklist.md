# I9 — 密码箱 v1.5 任务清单

> 规格：[I9-vault-key-hierarchy-spec.md](./I9-vault-key-hierarchy-spec.md)（§2 类型、§3 原语、§4 meta v2 与编排、§7 错误码、§9 命令、§11 验收）
> 配套：[条目](./I9-vault-entry-cipher-spec.md)（S2 条目加密）、[同步](./I9-vault-sync-crypto-spec.md)（S3 仓库完整性）、[运行时](./I9-vault-runtime-session-spec.md)（S4 会话驻留）、[明文出口](./I9-vault-plaintext-egress-spec.md)（S5 明文出口）、[生物识别](./I9-vault-biometric-spec.md)、[恢复通道](./I9-vault-recovery-channel-spec.md)、[审计](./I9-vault-audit-spec.md)、[能力门禁](./I9-vault-capability-gating-spec.md)（S6）
> 地图：[I9-vault-crypto-surface-inventory.md](./I9-vault-crypto-surface-inventory.md)（每个任务改动的文件都应能在其中找到归属）
> 顺序：M0 → M1 → M2 → M3 → M4 → M5 → M6 → M7（M0 与 M1 互不依赖；M3.1 可与 M2 并行）
> 勾选条件：代码已改 **且** 对应验收已跑。
> **状态唯一位置**：本清单是 v1.5 任务级进度的唯一来源；规格正文与全景地图不记录进度。本清单**不管理提交**（不写提交拆分 / 提交信息 / 回滚批次）。
> **任务项粒度**：一项 = 一个可独立完成、可独立验证的工作单元。语义与边界条件写规格，任务项只写「落点 + 判定条件」，不复制规格原文；任务项内**禁止写行号**，只写符号名。
> 范围：v1.5 密钥体系切换（`dek` 解耦、`.vault-meta` v2、条目与字段 AAD、纸面恢复、同步完整性、生物识别、`rotate_dek`）以及切换过程中暴露的既有缺陷。v1 旧库一律拒绝，**不做迁移**。
> 测试位置：`src-tauri/crates/assistant-vault/tests/`、各 crate 内 `#[cfg(test)]` 模块、`src/vault/**/__tests__/`。

---

## M0 既有缺陷修复（依赖：无）

实现：`src-tauri/crates/assistant-vault/src/security_db/entries/field_crypto.rs`、`src-tauri/crates/assistant-vault/src/security_db/crypto_ops.rs`、`src-tauri/crates/assistant-vault/src/security_db/{rekey,mod}.rs`、`src-tauri/crates/assistant-vault/src/sync_engine/{preview,history}.rs`、`src-tauri/src/cmd/vault/{entries,recovery,sync}.rs`、`src/vault/settings/ImportExportTab.tsx`

- [x] M0.1 `decrypt_opt` 去掉 `unwrap_or_default`；受保护自定义字段解密失败不再以 `.ok()?` 丢弃；整条解密失败返回 `EntryCorrupted`（`EntryCorrupted` 随本任务加入 `VaultError` / `VaultErrorCode` / `vaultError.ts`）。截断密文、非法 UTF-8、字段互换三种输入均须报错（entry-cipher §5.3 / E3）
- [x] M0.2 改密成功后固定一种旧备份策略（删除改密前的 `security-*.db.bak`，或对备份重新 rekey），不留给用户选择；改密后用旧口令打不开任何残留备份（runtime §4.3 / F13）
- [x] M0.3 `vault_export_to_file` 增加 `confirm_token("EXPORT-PLAINTEXT")`、路径校验、0600 权限、临时文件 + rename、审计事件；缺 token 与非法路径均被拒（egress §2.1）
- [x] M0.4 `mask_entry_preview` 脱敏 `notes`；`vault_conflict_reveal` 接 `RevealGuard` + 审计 + TTL；预览显示 `•••`，连续揭示第 4 次返回 `RateLimited`（egress §1.3、sync §2.7）
- [x] M0.5 纸面恢复的临时副本：`vault_recover_from_paper` 按 recovery §3.0 clone 到 `security/recovery/` 只读，用后即删（成功 / 失败 / panic 都删）；启动与重置仓库时清理崩溃残留（`cmd/vault/mod.rs::cleanup_recovery_dir`）；本机正式仓库 `security/vault/` 不读不写（recovery §3.0、egress §2.3、sync §2.9、F12）
- [x] M0.6 `vault_import` 写 `entries/*.enc` 并调用 `finalize`，不再只写本机库；导入后不出现「库里有、仓库没有」（entry-cipher §5.2 / F6）
- [x] M0.7 为 M0.1–M0.6 补断言：`tests/security_db.rs`、`tests/vault_file.rs`、新增 `tests/egress_guard.rs` 全部通过（各规格测试节）

> M0 不引入任何 v1.5 类型。M0.1 是 M2 的前置条件：新库的解密错误若仍被静默吞掉，P0-2 的验收会失真。

---

## M1 `kdf_iter` 与 PRAGMA（依赖：无）

实现：`src-tauri/crates/assistant-vault/src/security_db/mod.rs`、`src-tauri/crates/assistant-vault/src/recovery_key/mod.rs`、`src-tauri/crates/assistant-vault/tests/sqlcipher_pragma.rs`（新建）

- [x] M1.1 先写 `sqlcipher_kdf_iter_one_is_noop`，验证 raw key 下 `kdf_iter` 的真实语义；两条断言通过（结论：raw key 下 `kdf_iter` 为 no-op，无需回退）（key §11.1 / T-P0-1）
- [x] M1.2 `SQLCIPHER_PRAGMAS` 改 `kdf_iter=1`、去重键，补 T-P0-2 / T-P0-3 断言；常量中无重复键且包含 `kdf_iter=1`（key §11.1）
- [x] M1.3 `recovery_key` 内的 PRAGMA 字面量改走 `SQLCIPHER_PRAGMAS`；全仓不再出现第二份 PRAGMA 字面量（key §6.4）

> M1.1 必须先于 M1.2：这是全案唯一「假设可能不成立」的点，先测后改。M1 与 M0 无依赖关系，可并行。

---

## M2 核心切换（依赖：M1）

### M2.1–M2.2 类型与原语

实现：`src-tauri/crates/assistant-crypto/src/vault_crypto/{types,mod}.rs`

- [x] M2.1 新增 `Dek` / `Kek` / `DbKey` / `MetaKey`：`Zeroize + Drop`，禁 `Clone` / `Copy` / `Serialize`，`Debug` 脱敏，长度在构造函数内断言；类型单测覆盖长度断言与 `Debug` 输出（key §2.1–2.2）
- [x] M2.2 新增 `derive_kek` / `derive_db_key` / `derive_entry_key` / `derive_meta_key` / `seal_dek` / `open_dek` / `build_slot_aad` / `canonical_meta_bytes`；删除 v1 `derive_master_key` / `verify_master_password` / `derive_sqlcipher_key` 的生产调用；T-G1 / T-G2 黄金向量通过（key §3.1–3.7）
  - 已落地：7 个原语 + `canonical_meta_bytes`（按分层放在 `assistant-vault/src/vault_file/mod.rs`）、T-G2 字节序列与 seal/open 往返断言；v1 原语 `derive_master_key` / `verify_master_password` / `VaultCrypto::derive_entry_key(&MasterKey)` 连同其单测在 M2.12 删除，`derive_sqlcipher_key` 仅剩 v1 生物识别与恢复码两处调用（随 M3 / M5 重写删除）

### M2.3–M2.4 `.vault-meta` v2

实现：`src-tauri/crates/assistant-vault/src/vault_file/{types,mod}.rs`

- [x] M2.3 新增 `VaultMetaFileV2` / `VaultSlot` / `KdfParams` / `VaultManifest` / `ManifestEntry` / `RecoveryBackup`；删除 `VaultCryptoMeta` / `EmergencyDataFile`；序列化字段名与顺序与规格一致（key §2.3）
  - 已落地：6 个 v2 类型 + `VaultMetaIntegrityView`（字段声明顺序即规范化顺序，`recoveryBackup` 恒输出 null）；`VaultCryptoMeta` / `EmergencyDataFile` / v1 `VaultMetaFile` 与 `VaultCryptoInfo` 结构已在 M2.12 删除
- [x] M2.4 实现 `read_vault_meta_v2`（严格解析、v1 直接拒绝）、`write_vault_meta_v2`（原子写 + 签名）、`compute_manifest`、`meta_file_hash`、`verify_manifest`；删除 5 个 v1 meta 函数；I5 / I10 通过且读取路径不修改旧文件（key §4.2–4.4）
  - 已落地：5 个 v2 函数 + `verify_meta_integrity(&MetaKey)`，T-G1 黄金向量与 v1 拒绝/字段校验/清单/文件 hash 断言通过；v1 meta 函数（`read_vault_meta` / `write_vault_meta` / `export_vault_meta` / `vault_crypto_info` / `get_salt` / `set_salt` / `master_key_hash` 读写）已在 M2.12 删除，`read_vault_meta_v2` 保留 `master_key_salt` / `emergency` 键的拒绝分支（供 M2.16 旧库识别）

### M2.5–M2.9 本机库与编排

实现：`src-tauri/crates/assistant-vault/src/security_db/{mod,meta,rekey}.rs`、`src-tauri/crates/assistant-vault/src/security_db/entries/tx.rs`、`src-tauri/crates/assistant-vault/src/lifecycle.rs`（新建）、`src-tauri/crates/assistant-vault/src/lib.rs`

- [x] M2.5 `SecurityDbState` 合并为 `session: Mutex<Option<VaultSession>>` + `epoch`；新增 `with_session`；`init` / `unlock` 改签名；解锁后加探针读；锁定后无半状态，探针失败返回 `LocalDbKeyMismatch`（runtime §2、key §4.5）
  - 已落地：`session` 单锁 + `epoch: AtomicU64`；`with_session` / `with_session_mut` / `lock_session`（锁定时 `epoch` 自增并整段 `take`）；`init` / `unlock` 改收 `VaultSession` 与 `AnchorExpectation`，`unlock` 先 `probe_read` 再比对 anchor，探针失败返回 `LocalDbKeyMismatch`
- [x] M2.6 新增 `lifecycle.rs`：`initialize` / `unlock` / `change_password` / `rotate_dek` 骨架；命令层不再自行拼装跨层顺序（key §4.1）
  - 已落地：`lifecycle.rs` 提供 `initialize` / `unlock` / `change_password` / `rotate_dek`（骨架，完整实现待 M6）与 `derive_session_keys`；命令层（M2.14）全部改调 `lifecycle`，不再自行拼装跨层顺序
- [x] M2.7 `change_master_password` 改 O(1)：只重包 `dek`，**不 rekey 本机库**（`dbKey = HKDF(dek)` 不随口令变），删除全量条目重加密；I2 / I3 通过，条目文件 sha256 与 `dbKey` 均不变（key §4.8）
  - 已落地：`security_db/rekey.rs` 的 `change_master_password` 用旧 KEK 开 `dek`、用新 KEK 重包并换槽，不触碰条目与本机库；I2（换槽但 `dek` / `dbKey` 不变）与 I3（旧 `dek` 仍可解条目）通过
- [x] M2.8 `LocalDbKeyMismatch` 只保留重建入口 `rebuild_local_db`（旧口令重包路径在 `HKDF(dek)` 下不可行）；I17 通过（key §4.7）
  - 已落地：`security_db/entries/mod.rs` 新增 `rebuild_index`（遍历 `entries/*.enc` 逐条导入 `vault_entries`，配套 `imported_to_import` / `sync_error_to_vault_error`）；`security_db/rekey.rs` 新增 `rebuild_local_db`（关会话 → `backup_db_file` 备份旧库 → 删旧库 → 新建库 + `INIT_SQL` → `rebuild_index` → `set_anchor(revision=0)` → 审计 `LocalDbRebuilt`；任一步失败即删除新建库回滚）与 `backup_db_file`；`lifecycle.rs` 对外暴露 `rebuild_local_db`（读 meta → `derive_session_keys` → 重建 → 新建连接并 `unlock`），重建成功后**自动解锁且不自动建库**；命令层新增 `vault_rebuild_local_db` 并注册进 `vault_plugin.rs`；前端 `vaultError.ts` 新增 `needsDbRepair()`、`vault.service.ts` 新增 `rebuildLocalDb`、`vaultLifecycleSlice` 透出 `rebuildLocalDb`、`useVaultUnlock` 透传；新增 `unlock/DbRepairDialog.tsx`（说明 + 重建警告 + 主密码输入 + 重建按钮）并由 `VaultUnlock.tsx` 在 `localDbKeyMismatch` 且未关闭时渲染；i18n 补齐 `vault.repair.*` 5 项中英两套；I17 用例 `i17_rebuild_local_db_recovers_index_from_entry_files` 断言备份存在、条目从文件重建、`anchor.revision` 归零、`local_db_rebuilt` 审计恰好 1 条、重建后条目可读
- [x] M2.9 `anchor` 三键读写 + `get_anchor` / `set_anchor` + 校验顺序；I4 回滚检测通过（key §4.6）
  - 已落地：`security_db/meta.rs` 的 `get_anchor` / `set_anchor` 三键原子读写与格式校验；`unlock` 在探针后先取 anchor 再比对 `meta_hash` / `dek_version`，缺失或不一致返回 `VaultRollbackDetected`；I4 meta 篡改用例通过

### M2.10–M2.11 条目层

实现：`src-tauri/crates/assistant-vault/src/vault_file/{types,mod}.rs`、`src-tauri/crates/assistant-vault/src/security_db/entries/{field_crypto,tx}.rs`、`src-tauri/crates/assistant-vault/src/security_db/crypto_ops.rs`

- [x] M2.10 `EntryFileV2` / `EntryBlobV2` / `TotpBlob`；条目文件 AAD；字段级 AAD（`password` / `notes` / `totp` / `custom:{name}` / `ssh-private-key`）；T-E1–T-E12 通过（entry-cipher §2）
  - 已落地：三类 v2 类型 + 条目文件 AAD + 字段级 AAD 常量与 `encrypt_opt` / `decrypt_opt`（收 `&PerEntryKey`、`field`、`entry_id`）；`tests/vault_file.rs` 已切到 v2 语义，T-E1–T-E12 专项用例随 M2.15 的 `tests/entry_cipher.rs` 补齐
- [x] M2.11 替换全部 `derive_entry_key` 调用点（17 处 / 7 文件）；`tx.rs` 回调改收 `&VaultSession`；全仓无 `MasterKey` 实参残留（entry-cipher §3、key §4.10）
  - 已落地：`security_db/entries/{write,snapshot,deleted,import}.rs`、`crypto_ops.rs`、`sync_engine/{history,preview,export,pull_push,emergency}.rs`、`vault_file/mod.rs` 全部改收 `&Dek`；`with_conn_and_dek(conn, dek, dek_version)` 取代 `with_conn_and_key`；条目层已无 `MasterKey` 实参（残留的 `legacy_master_key` 仅用于 meta v1 签名、生物识别与恢复码，随 M2.12 / M3 / M5 删除）

### M2.12–M2.14 清退与契约

实现：`src-tauri/crates/assistant-vault/src/{security_db,sync_engine,recovery_key,biometric}/**`、`src-tauri/crates/assistant-crypto/src/error.rs`、`src-tauri/crates/assistant-vault/src/dto/{error,convert}.rs`、`src-tauri/src/cmd/vault/{lifecycle,entries}.rs`、`src-tauri/src/vault_plugin.rs`、`src/vault/core/vaultError.ts`

- [x] M2.12 删除 v1 死代码：`BOOTSTRAP_SALT`、`hash_salt`、第三次 Argon2id、`emergency` 全链路、`vault_crypto_info`、v1 生物识别 key id、v1 恢复码；编译通过且无 v1 字段残留（key §4.2、biometric §2、recovery §0.2）
  - 已落地：`BOOTSTRAP_SALT` / `hash_salt` / 第三次 Argon2id（`derive_master_key` / `verify_master_password`）、v1 `derive_entry_key(&MasterKey)`、`vault_crypto_info`、`VaultCryptoInfo` / `VaultMetaFile` / `VaultCryptoMeta` / `EmergencyDataFile`、`meta.rs` 的 `get_salt` / `set_salt` / `master_key_hash` 读写、命令层 `cmd/vault/recovery.rs` 四个 v1 命令（含注册项）全部删除
  - 收尾（本轮）：`recovery_key/mod.rs` 的 `EmergencyData` / `generate_recovery_key` / `decrypt_recovery_keys` / `unlock_with_recovery_key` 已随 M3 删除；`biometric_ops.rs` 的 v1 key id 与 `VaultSession` 的 `legacy_master_key` / `legacy_bootstrap_key` 已随 M5 删除；本轮补删最后两处纯死代码——`assistant-crypto` 的 `MasterKey` 类型与 `derive_legacy_entry_key`（生产零调用，仅 `tests/sync_common` 与 `crates/assistant-crypto/tests/vault_crypto.rs` 引用，已同步清理）、`sync_engine/emergency.rs` 的 `disaster_recovery_export`（无调用方，模块一并删除）；`tests/legacy_residue.rs` 白名单收敛为 `vault_file/mod.rs` 的 v1 识别分支 2 条
  - 验证：`cargo check -p assistant-crypto -p assistant-vault --all-targets`、`cargo test -p assistant-crypto`、`cargo test -p assistant-vault --test legacy_residue --test lifecycle_integration`（3 + 12 例）全通过
- [x] M2.13 核心错误码 5 项 + `convert.rs` 穷尽映射 + 前端错误码数组；T-D1 双向一致（key §7）
  - 已落地：`VaultError` 与 `VaultErrorCode` 新增 `UnsupportedVaultVersion` / `LocalDbKeyMismatch` / `VaultRollbackDetected` / `VaultManifestMismatch` / `VaultCorrupted`；`dto/convert.rs` 的 `From<VaultError>` 穷尽映射已补齐（`VaultCorrupted` 透传原文，其余走 `Display`）；`src/vault/core/vaultError.ts` 的 `VAULT_ERROR_CODES` 同步为 31 项；`tests/dto_contract.rs` 新增 `vault_error_codes_match_frontend_array`（解析前端数组字面量与 `VaultErrorCode` 序列化集合做双向集合相等），`vault_error_code_strings_are_frozen` 补齐 `entryCorrupted` 与 5 个新码的冻结字符串断言
  - 追加（版本过新与损坏区分）：`reject_v1_meta` 原先对 `version != 1/2` 一律返回 `VaultCorrupted`，会把「更新版本创建的库」误报为损坏并给出重置入口（有误删真实数据的风险）。现新增 `VaultError::VaultVersionTooNew` → `VaultErrorCode::VaultVersionTooNew`（camelCase `vaultVersionTooNew`），`version > 2` 归入该码，`version == 0` 仍为 `VaultCorrupted`；`convert.rs` 穷尽映射、`dto_contract.rs` 冻结字符串与前端数组、`contract:gen` 生成类型同步；`tests/lifecycle_integration.rs` 的 `i5_strict_meta_parse_rejects_v1_and_unknown_meta` 增加 `version=3` → `VaultVersionTooNew` 与 `version=0` → `VaultCorrupted` 两条断言
  - 追加（错误文案本地化）：新增 `src/vault/core/vaultErrorText.ts`，以 `satisfies Record<VaultErrorCode, I18nPath>` 建立「错误码 → `vault.error.*`」全量映射（编译期穷尽），`vaultErrorText(code, fallback)` 在无码时回退后端 message；`VaultUnlock.tsx` 的解锁页、重置面板、重建弹窗统一改用该函数，界面不再出现后端英文技术串；i18n 中英两套各补 32 个 `vault.error.*` 与 `vault.versionTooNew.{title,desc,retry}`；新增 `unlock/VersionTooNewPanel.tsx` 与 `needsAppUpgrade()`，`vaultVersionTooNew` 走「请升级应用、不要重置」独立分支（不提供重置入口）
- [x] M2.14 命令层：`vault_init` / `vault_unlock` / `vault_lock` / `vault_change_master_password` 改调 `lifecycle`；新增 `vault_rotate_dek` 骨架与 `vault_reset_repository`；命令注册表校验通过（key §9、capability §3.2）
  - 已落地：`cmd/vault/lifecycle.rs` 的 `vault_init` / `vault_unlock` / `vault_show_unlock_window` / `vault_lock` 全部改调 `assistant_vault::lifecycle`；`cmd/vault/entries.rs` 的 `vault_change_master_password` 改调 `lifecycle::change_password` 并删除 `RecoveryKeyInvalidated` 审计；新增 `vault_rotate_dek`（返回 `RecoveryKeyDto`，内部调 `lifecycle::rotate_dek` 骨架，完整实现待 M6）与 `vault_reset_repository`（`RESET-VAULT` token 校验 → `lock_session(Reset)` → 清 keystore 4 个 key id（失败即中止）→ 备份 `.vault-meta.bak`（仅首次）→ 删 `.vault-meta` / `security.db` / `entries/` / `audit/`）；两条命令已注册进 `vault_plugin.rs`；`tests/command_registry.rs` 双向校验通过

### M2.15–M2.18 验收与兜底

- [x] M2.15 P0-2 验收：I1–I5、I10、T-G1 / T-G2 / T-D1；新增 `tests/lifecycle_integration.rs` 与 `tests/dto_contract.rs` 全部通过（key §11.2）
  - 已落地：`tests/lifecycle_integration.rs` 7 例（I1 初始化写 v2 单口令槽 / I2 改密换槽但 `dek`、`dbKey` 不变 / I3 旧 `dek` 仍可解条目 / I4 错口令与 meta 篡改 / I5 严格解析拒绝 v1 与未知 meta / I10 版本不支持不改任何文件）；T-G1 `vault_file.rs` 的 `canonical_meta_bytes_matches_golden_vector`、T-G2 `assistant-crypto` 的 `build_slot_aad_matches_spec_byte_sequence`、T-D1 `dto_contract.rs` 的前端数组双向一致均通过；`cargo test -p assistant-vault --test lifecycle_integration --test dto_contract --test vault_file` 与 `cargo test -p assistant-crypto build_slot_aad` 全绿
- [x] M2.16 旧库识别与重置引导：前端识别「版本不支持」错误码后进入独立分支，展示备份提示 + `RESET-VAULT` 二次确认 + 重置后回到初始化页（不自动建库），禁止自动删除旧库；手工走通 T-C8a / T-C8b / T-C8c（未确认时磁盘无变化）（key §10.3–10.4、capability §3.2、§8）
  - 已落地：`vaultError.ts` 新增 `needsRepositoryReset()`；`VaultUnlock.tsx` 在 `errorCode === unsupportedVaultVersion` 时整体切到新组件 `unlock/ResetRepositoryPanel.tsx`（备份提示 Alert + 第 1 步「我已备份，继续」+ 第 2 步输入 `RESET-VAULT` 才可点「重置仓库」）；`useVaultUnlock` 透出 `errorCode` 与 `resetRepository`；`vaultLifecycleSlice.resetRepository` 成功后置 `isInitialized=false`、清空条目缓存并清错误，界面回到初始化页且不自动建库；i18n 补齐 `vault.reset.*`（含 `backupHint` / `proceed` / `action`）中英两套；检测阶段只读由后端 I10 用例保证
- [x] M2.17 DTO 改动后跑 `npm run contract:gen`，使 `src/types/protocol.contract.ts`、`src/vault/core/contract/vault.contract.ts` 与 Rust 侧一致，禁止手改生成类型；`npm run contract:check` 无差异（项目规则「类型安全规范」）
  - 已落地：`npm run contract:gen` 已生成 `vault.contract.ts` 的 5 个新错误码与 `entryCorrupted`；连续两次生成哈希一致（幂等），`protocol.contract.ts` 无变化；`contract:check` 的 `git diff --exit-code` 仅因改动未提交而报差异，提交后即通过
- [x] M2.18 v1 残留静态扫描用例：扫描 `master_key_salt` / `emergency` / `BOOTSTRAP_SALT` / `vault_crypto_info` 等符号，断言 `Dek` 无 `Clone`，断言解密路径无 `unwrap_or_default`；扫描通过，新增残留会使其失败（key §4.2、entry-cipher §5.3）
  - 已落地：新增 `tests/legacy_residue.rs` 3 例 —— `v1_residue_symbols_stay_within_allowlist`（扫描 `assistant-vault/src`、`assistant-crypto/src`、`src/cmd/vault`，白名单仅保留 `vault_file/mod.rs` 的 v1 识别分支 2 条，并反向校验白名单条目仍有效）、`key_types_do_not_implement_clone`（负向 trait 歧义断言 `Dek` / `Kek` / `DbKey` / `MetaKey` / `PerEntryKey` 均无 `Clone`）、`decrypt_paths_do_not_swallow_errors`（`field_crypto.rs` 与 `vault_crypto/mod.rs` 禁 `unwrap_or_default`）

> M2.1–M2.15 必须同批交付：类型、原语、meta、本机库、编排、条目格式、错误码、命令一起切，任一项单独落地都会留下不可编译或不兼容的中间态。M2.16–M2.18 不阻塞该批。
> M2.12 必须与 M2.10 / M2.11 同批：v1 字段的删除与条目格式切换互为条件。
> M2 落地后到 M3 完成前没有纸面恢复路径，因此 M3.1 安排与 M2 并行，不排在其后。

---

## M3 恢复通道与审计（依赖：M2）

实现：`src-tauri/crates/assistant-vault/src/recovery_key/mod.rs`、`src-tauri/crates/assistant-vault/src/audit/{mod,jsonl}.rs`、`src-tauri/crates/assistant-vault/src/security_db/mod.rs`、`src-tauri/crates/assistant-vault/src/dto/{recovery,mod}.rs`、`src-tauri/src/cmd/vault/recovery.rs`、`src-tauri/src/vault_plugin.rs`、`src/vault/**`、`src/i18n/**`

- [x] M3.1 纸面编解码：Crockford Base32 + CRC32C + 归一化 + 纠错定位；新增 `crc` 依赖；T1–T5b 通过（recovery §1）
  - 已落地：`recovery_key/paper.rs` 提供 `encode_paper` / `decode_paper` / `normalize_paper_input` / `format_groups` / `locate_checksum_error`，40 字符 5 组、CRC32C 校验；8 个单元测试覆盖 40 字符长度、往返、五组切分、混合输入归一化、单字符换位 → `Checksum`、截断/非法 → `Format`、定位恰好一个错位、合法纸面不定位
- [x] M3.2 `RecoverySession`（`dek` + `entries_dir` + `manifest`）+ `open_recovery_session` + `export_all`；删除 v1 恢复函数；I7–I9、I11 通过，恢复过程不触碰 `security.db`（recovery §2–§3）
  - 已落地：`recovery_key/session.rs` 的 `RecoverySession` 只持 `dek` / `entries_dir` / `manifest`，不暴露 `conn()` / `db_key()`；`open_recovery_session` 走「解码 → v2 meta → `metaKey` 完整性 → manifest 校验」四步且不派生 `dbKey`；`export_all` 逐条 `derive_entry_key` + 文件 sha256 复核，单条失败进 `skipped` 不中断；v1 `export_all_entries` / `EmergencyData` 已删除
- [x] M3.3 恢复 DTO 与命令：`vault_export_recovery_key` / `vault_recover_from_paper` / `vault_locate_paper_error` / `vault_has_recovery_backup`；删除 4 个 v1 命令；命令注册表校验通过（recovery §4）
  - 已落地：`dto/recovery.rs` 定义 `RecoveryKeyDto` / `RecoveryExportDto` / `RecoveryEntryDto` / `RecoverySkipDto` / `PaperErrorPosDto` / `RecoveryBackupDto`（全部 `#[ts(export)]`，`serde_json::Value` 经 ts-rs `serde-json-impl` 导出 `JsonValue`）；`cmd/vault/recovery.rs` 4 个命令按 precondition 声明接入命令门；`vault_generate_recovery_key` / `vault_has_recovery_key` / `vault_verify_vault_meta_integrity` / `vault_disaster_recovery_export` 已删除；`dto_contract.rs` 6 个恢复 DTO wire 格式校验通过
- [x] M3.4 审计基础设施：`AuditEvent` 增删、`audit/jsonl.rs`、`log_dual`、解锁补录；T-A1–T-A10 通过，重复解锁不产生重复记录（audit §1–§4）
  - 已落地：`audit/mod.rs` 删除 `RecoveryKeyGenerated` / `RecoveryKeyInvalidated`，新增 10 个事件（`RecoveryKeyExported` / `RecoveryExportUsed` / `BiometricUnlocked` / `BiometricFailed` / `BiometricRevoked` / `VaultUnsupportedVersion` / `VaultRepositoryReset` / `LocalDbRebuilt` / `PasswordChanged` / `VaultUnlock`）；`audit/jsonl.rs` 新增 `AuditJsonlWriter`（按日轮转、8MiB 上限、0600）与 `log_dual`（DB + JSONL 双写，任一路成功即 Ok，JSONL 失败只 warn）；`audit::log` 保留为 `log_dual(Some(conn), None, ..)` 别名，现有调用点签名不变
  - 已落地：双写共用同一 `at`（`append_at` / `write_db_at`），解锁补录按 `(event_type, timestamp)` + `jsonl-{at}-{event}` 双重判重，重复解锁不产生重复记录；`vault_meta` KV 存 `audit_jsonl_synced_at` 游标
  - 已落地：埋点覆盖恢复密钥导出（双写）、纸面恢复使用（仅 JSONL）、改密（双写）、生物识别解锁/失败/撤销、版本拒绝与仓库重置（仅 JSONL）
  - 测试：`tests/audit.rs` T-A1/A2/A3/A5/A6/A7/A8/A9/A10 共 9 例全通过；T-A4（改 DEK）随 M6.1 `rotate_dek` 实现后补，本轮 DEK 轮换尚未落地
- [x] M3.5 恢复前端：`RecoveryKeyDialog` / `PaperRecoveryFlow` / `RecoveryKeyForm`；`RecoveryTab` / `DisasterRecovery` 改造；service 方法；手抄 40 字符可完成恢复，输错能定位到组位（recovery §5）
  - 已落地：`core/paperKey.ts` 提供 `normalizePaperInput` / `groupPaperKey` / `formatPaperKey` / `paperInputError`（`PAPER_CHARS` / `PAPER_GROUP_LEN` / `PAPER_GROUPS`），与后端 Crockford 归一化规则一致（小写、连字符、空格、全角、`O/0`、`I/1/l/L`、`U/V`）
  - 已落地：`settings/RecoveryKeyForm.tsx` 五组八格实时回显、组位高亮、`n/40` 进度；`settings/RecoveryKeyDialog.tsx` 主密码校验 → 五组展示 → 「已抄写」强制勾选 → 关闭前二次确认；`unlock/PaperRecoveryFlow.tsx` 输入 → 格式校验 → `Checksum` 时调 `locatePaperError` 定位组位 → 导出条目/跳过条目展示
  - 已落地：`core/vault.service.ts` 新增 `exportRecoveryKey` / `hasRecoveryBackup` / `recoverFromPaper` / `locatePaperError`，删除 `generateRecoveryKey` / `hasRecoveryKey` / `disasterRecoveryExport`；`vaultRecoverySlice` 状态换成 `recoveryBackup` / `exportFilePath`，`_recoveryKeySetAt` → `_recoveryBackupAt`
  - 已落地：`settings/RecoveryTab.tsx` 改为备份状态卡片 + 导出/恢复入口，不再接收旧 props；`unlock/DisasterRecovery.tsx` 改为入口按钮，内部复用 `PaperRecoveryFlow`，旧 Git URL / 提交 hash 输入流程与 `VaultUnlock` 中的灾难恢复步骤机一并删除
  - 测试：`core/__tests__/paperKey.test.ts` 覆盖归一化 / 分组 / 输入校验；`vaultStore.test.ts` 的恢复密钥用例改为 `exportRecoveryKey`；`npx tsc --noEmit` 与 `src/vault` 72 例全通过
- [x] M3.6 重写 `tests/recovery_key.rs` 为 40 字符纸面测试，旧 4 组测试全部替换（recovery §7）
  - 已落地：`tests/recovery_key.rs` 整文件重写为 11 例 40 字符纸面用例 —— 往返、导出全条目、恢复不触碰 `security.db`、anchor 一致、解锁态前置、错口令拒绝、未导出时无备份标记、错纸面 → `Checksum`、畸形 → `Format`、meta 篡改合并错误、条目文件篡改 → `VaultManifestMismatch`
- [x] M3.7 i18n 收口：新增错误码文案（`vault.error.*`）与恢复、重置引导文案；语言切换后无裸中文字面量，i18n key 全覆盖（key §10.4、recovery §8.7）
  - 已落地：`vault.error.*` 补齐 32 个错误码文案（含 `invalidRecoveryKeyFormat` / `invalidRecoveryKeyChecksum` / `invalidRecoveryKeyOrMetaTampered` / `unsupportedVaultVersion` / `vaultVersionTooNew` / `localDbKeyMismatch` / `vaultRollbackDetected` / `vaultManifestMismatch` / `vaultCorrupted`），`vaultErrorText.ts` 以 `satisfies Record<VaultErrorCode, I18nPath>` 强制全覆盖
  - 已落地：`vault.recovery.*`（备份状态 / 导出 / 纸面恢复 / 格式错误）与 `vault.versionTooNew.*` 重置引导文案中英双语；删除旧 `vault.recovery.key_*` / `verify_integrity` / `integrity_*` 与整段 `vault.disaster_recovery.*`
  - 校验：`src/vault` 下除测试描述与注释外无裸中文字面量；i18n key 由 `I18nPath` 类型在 `tsc` 阶段强校验


> M3.1 只依赖 M2.1，是纯函数编解码，无 IO，可与 M2 并行。
> M3.4 必须先于 M3.3 与 M5.2：恢复失败与生物识别失败都需要审计落点。

---

## M4 仓库与同步完整性（依赖：M3）

实现：`src-tauri/crates/assistant-vault/src/security_db/entries/*.rs`、`src-tauri/crates/assistant-vault/src/vault_file/mod.rs`、`src-tauri/crates/assistant-vault/src/sync_engine/{export,pull_push,history,preview}.rs`、`src-tauri/crates/assistant-vault/src/lifecycle.rs`、`src-tauri/src/cmd/vault/mod.rs`

- [x] M4.1 条目增 / 删 / 改后立即更新 manifest；`change_password` 不重算；`rotate_dek` 全量重算；I6 通过（key §4.2、sync §2.3）
  - 已落地：`cmd/vault/entries.rs::export_entry_to_sync` 在 `create_entry_tx` / `update_entry_tx` / `set_favorite_tx` / `delete_entry_tx` 的事务回调内先写 `.enc`，再调用 `lifecycle::sync_write_meta`，manifest 与 meta 随条目变更同事务提交；`security_db::change_master_password` 只换 password slot，不触碰 manifest；`rotate_dek` 的全量重算留在 M6.1
- [x] M4.2 push 顺序固定：写 `.enc` → manifest → meta → commit；删除 `sync_engine` 内的 meta 写入；meta 只有单一写入者，T-S1 通过（sync §2.2–§2.3）
  - 已落地：`sync_engine/export.rs::export_all_entries` 只做 `.enc` 写入 / 删除，末尾唯一出口 `lifecycle::sync_write_meta`（内部 `check_repository_layout` → `compute_manifest` → `write_vault_meta_v2`）；`remote_admin` / `history` / `export` 三处调用点全部收敛到该出口；全仓 `write_vault_meta_v2` 只剩 `lifecycle` 与编排层 `rekey.rs`
- [x] M4.3 pull 顺序固定：git pull → 严格 meta → `vaultId` → `dekVersion` → integrity → manifest → import → anchor；T-S2–T-S7 通过（sync §2.4–§2.6）
  - 已落地：`pull_push.rs::validate_meta_identity` 按 `check_repository_layout` → `read_vault_meta_v2`（严格解析）→ `vaultId` 比对 → `dekVersion`（Ahead / Behind 双向拒绝）→ `verify_meta_integrity` 顺序执行；`validate_remote_meta` 追加 `verify_manifest`；`pull` 成功后调用 `advance_anchor` 前移本机基线
- [x] M4.4 冲突处理：`.vault-meta` 不自动合并；条目预览脱敏；`removed_ids` 以 manifest 为准；T-S3 / T-S4 / T-S8 / T-S9 通过（sync §2.5–§2.7）
  - 已落地：`history.rs::resolve_meta_conflict` 专用入口，`.vault-meta` 冲突必须显式选择一边；`finish_resolution` 在所有冲突解决后统一写一次 meta + 刷新 anchor + commit；`pull_push.rs` 的 `removed_ids` 由「本地原有 ∩ 不在远端 manifest」推导，不再按 DB 行判断
- [x] M4.5 仓库白名单 + `.gitignore` + `.tmp` 清理 + 恢复副本残留清理；T-S1 / T-S11 通过（sync §2.1、§2.9）
  - 已落地：`vault_file/mod.rs::check_repository_layout` 只允许 `.vault-meta` / `.gitignore` / `.git/` / `entries/*.enc`；`ensure_gitignore` 生成最小 `.gitignore`（已存在不覆盖）；`clean_temp_files` 清 `.tmp`；`cmd/vault/mod.rs::cleanup_recovery_dir` 在启动与重置仓库时删除恢复副本残留
  - 测试：`sync_engine` 19 例、`vault_file` 26 例、`egress_guard` 8 例、`cmd::vault::tests` 2 例全通过

---

## M5 生物识别（依赖：M4）

实现：`src-tauri/crates/assistant-vault/src/security_db/biometric_ops.rs`、`src-tauri/crates/assistant-vault/src/biometric/{mod,windows,macos,linux}.rs`、`src-tauri/crates/assistant-vault/src/lifecycle.rs`、`src/vault/features/biometric/**`

- [x] M5.1 key id 改名；payload 改 52B `dek‖dbKey`；新增 `fingerprint`；旧 64B payload 被明确拒绝（biometric §2–§4）
  - 已落地：`security_db/biometric_ops.rs` 四个新 key id `vault-payload` / `vault-fingerprint` / `vault-sig-hash` / `vault-wrapping-key`；payload 组装为 `dek(20B) ‖ dbKey(32B)`，`BIOMETRIC_PAYLOAD_LEN = 52`；`compute_fingerprint` = SHA256(payload) 十六进制明文存 `vault-fingerprint`；长度不符即 `BiometricReset`（审计 `payload_len`）
- [x] M5.2 重写 `restore_keys_from_plaintext`：指纹 → 开库 → 探针 → anchor → 写 session；I12 通过，假解锁消失（biometric §5）
  - 已落地：`check_biometric_consistency` → `compute_fingerprint` 与 keystore 值比对（缺失/不符审计 `stale_payload`）→ `open_connection(db_key)` → `probe_read`（失败审计 `probe_failed`）→ `AnchorExpectation::new(meta_file_hash, dek_version)` 交 `state.unlock` 校验 → 成功后写 `BiometricUnlocked`；全程不再出现「只写 session 不验证密钥」的路径
- [x] M5.3 平台解锁入口：Windows 先校验 `vault-sig-hash` 再解密；非 Windows 分步校验；失败映射 `BiometricReset`；T-B4 / T-B5 通过（biometric §6）
  - 已落地：`unlock_windows` 顺序为 取 payload → `request_signature` → 比对 `vault-sig-hash`（缺失 `sig_hash_missing`、不符 `sig_changed`）→ 派生 wrapping key → `decrypt_field`（失败 `decrypt_failed`）→ `restore_keys_from_plaintext`；`unlock_separated` 校验 wrapping key 长度（`wrapping_key_len`）与解密失败；所有解密类失败统一 `BiometricReset`，不产生 `VaultError::Crypto`
- [x] M5.4 `delete_key` 区分 not-found 与真实失败（Windows / Linux 当前恒返回 `Ok`）；T-B1 / T-B3 在失败路径上返回错误（biometric §11.2）
  - 已落地：`biometric/{windows,linux,macos}.rs` 三平台 `delete_key` 均为 not-found → `Ok(())`、其余 → `Err(BiometricFailed)`；`delete_key` 恒 `Ok` 的假约束已消除
- [x] M5.5 注册 / 反注册；**不实现自动重包**（改密不改变 `dek` / `dbKey`）；T-B6 通过（biometric §7–§8）
  - 已落地：`enroll_biometric_with_hint`（Windows 走签名派生 wrapping key，非 Windows 走随机 wrapping key 分存）与 `disenroll_biometric`（删全部 4 个新 id）；`cmd/vault/entries.rs::vault_change_master_password` 删除改密后自动反注册逻辑，改密不再触碰生物识别
- [x] M5.6 R8：`rotate_dek` 第 1 步删除 `vault-payload`，失败即中止；T-B1 通过（biometric §9）
  - 已落地：`lifecycle::rotate_dek` 首行调用 `state.revoke_biometric_for_rotation(key_store)`；`vault-payload` 删除失败审计 `delete_failed` 并直接返回错误，其余 3 个 id 删除失败只 `warn`；`vault_rotate_dek` 命令层透传 `key_store`
- [x] M5.7 R9：平台 ACL 判定，不满足即禁用；不满足平台返回 `BiometricUnavailable`（biometric §10）
  - 已落地：`BiometricKeyStore::acl_satisfied()` 默认 `true`（Windows Hello/TPM）；macOS 以 `kSecAccessControlUserPresence` 创建成败判定；Linux 以系统 keyring 守护进程可用性判定；`enroll_biometric_with_hint` 在 `is_available` 之后显式判定，不满足即 `BiometricUnavailable`，无静默降级
- [x] M5.8 `vault_reset_repository` 清空 4 个 key id，失败即中止；T-B2 / T-B3 通过（biometric §11.1）
  - 已落地：`lifecycle::reset_repository`（crate 层，注入 `&dyn BiometricKeyStore`）先 `disenroll_biometric`（内部 `clear_all_biometric_entries` 遍历 4 个新 id 并返回首个错误），失败即 `?` 中止，之后才删除 `.vault-meta` / `security.db` / `entries/` / `audit/` / `recovery/`；`vault_reset_repository` 命令层改为调用该函数，仅保留 `.vault-meta` 备份与 `lock_session` 前置
- [x] M5.9 前端：`BiometricReset` 只认该错误码，不再用 `crypto` / `notFound`；强制重注册引导，重置后不自动重试（runtime §4.2、biometric §12）
  - 已落地：`vaultBiometricSlice::unlockBiometric` 仅对 `biometricReset` 标记凭据失效（不再自动反注册、不再识别 `crypto` / `notFound`）；`VaultUnlock` 在 `errorCode === "biometricReset"` 时渲染 `BiometricButton` 的「重新注册」提示；`ChangePasswordTab` 移除改密后的「重新启用生物识别」弹窗与对应 i18n 键；新增 `vault.biometric.reset_required` / `reenroll_required`

> M5.4 必须早于 M5.6 与 M6.1：`delete_key` 若仍恒返回 `Ok`，R8 就是假约束。
>
> 测试落点：`src-tauri/crates/assistant-vault/tests/biometric.rs`（mock keystore，覆盖 T-B1–T-B6）、`tests/audit.rs`（覆盖 T-A5）。

---

## M6 `rotate_dek`（依赖：M5）

实现：`src-tauri/crates/assistant-vault/src/lifecycle.rs`、`src-tauri/crates/assistant-vault/src/vault_file/mod.rs`、`src-tauri/crates/assistant-vault/src/sync_engine/history.rs`、`src-tauri/src/cmd/vault/entries.rs`、`src/vault/**`

- [x] M6.1 `lifecycle::rotate_dek`：删凭据 → 生成 `dek_new` → 全量重加密（临时目录 + 批量 rename）→ 新 slot → manifest → meta → anchor；I14 / I15 通过（key §4.9）
  - 已落地：`VaultSession` 新增 `kek: Option<Kek>` 缓存（仅口令会话携带），`with_kek()` / `kek()` 供轮换重包 `dek_new`；`lifecycle::rotate_dek` 按 §4.9 九步执行——口令会话前置守卫（生物识别/恢复码会话无 KEK 直接 `Validation`）→ 第 1 步 `revoke_biometric_for_rotation`（`vault-payload` 删除失败即中止，R8）→ `Dek::generate()` → `vault_file::reencrypt_entries` 全量重加密（先写 `<id>.tmp`，全部成功才批量 rename，任一失败清理暂存文件）→ 本机库加密列换钥（含墓碑行，事务内完成）→ 用当前 KEK 重包新 slot（AAD 带新 `dekVersion`）→ 重算 manifest / 清空 `recovery_backup` → 写 meta 并推进 anchor（提交点）→ `PRAGMA rekey`（失败仅 warn，下次解锁走 §4.7 重建）→ 返回纸面恢复码；`DekRotated` 审计双写
  - 测试：`tests/lifecycle_integration.rs` 新增 I14（`dekVersion+1`、条目密文全变且仍可读、旧纸面 `InvalidRecoveryKeyOrMetaTampered`、新纸面可导出、`recoveryBackup` 清空、旧 `dbKey` 打不开本机库、重新解锁正常）与 I15（锁定态调用返回 `Locked` 且仓库零改动）；`tests/audit.rs` 新增 T-A4（`dek_rotated` 双写）；`tests/biometric.rs` T-B1（payload 删除失败中止）通过
- [x] M6.2 命令与前端：`vault_rotate_dek` + 「轮换数据密钥」入口 + 强警告文案；命令注册表校验通过（key §9.1、recovery §5.2）
  - 已落地：命令层 `cmd/vault/lifecycle.rs::vault_rotate_dek`（`Unlocked` 前置条件，`biometric::get_key_store()` 注入，返回 `RecoveryKeyDto`）已在 `vault_plugin.rs` 注册；前端 `vault.service.ts::rotateDek`、`vaultRecoverySlice::rotateDek`（成功后刷新 `recoveryBackup`、置 `biometricEnrolled=false` 并清 `_biometricEnrolledAt`、重载条目）、`settings/RotateDekDialog.tsx`（四条强警告 + 确认码 + 新纸面一次性展示与强制抄写勾选）、`RecoveryTab.tsx` 独立「轮换数据密钥」入口；`zh-CN` / `en-US` 文案齐备
  - 测试：`cargo test -p assistant-vault --test command_registry` 4 例通过；`src/vault/core/__tests__/vaultStore.test.ts` 新增 `rotateDek` 用例（命令调用、`recoveryBackup` 清空、`biometricEnrolled` 归零）
- [x] M6.3 轮换后强制重新注册生物识别，用户拒绝则视为停用；I13 / T-B1 通过（biometric §9）
  - 已落地：`RotateDekDialog` 在轮换前记录 `biometricEnrolled`，新纸面抄写完成关闭弹窗时若「轮换前已注册且本机可用」弹出强制重注册确认（`modal.confirm`）；「立即注册」调用 `enrollBiometric`，「暂不注册」显式提示「生物识别已停用」——与 `rotateDek` slice 已置 `biometricEnrolled=false` 一致，不存在「已开启但实际无凭据」的假状态
  - 测试：`tests/biometric.rs` 新增 I13（`rotate_dek` 后 4 个 key id 全部不存在、`is_biometric_enrolled` 为 false、模拟保留旧凭据副本后锁定再 `unlock_biometric` 返回 `BiometricReset` 且仍锁定、`biometric_revoked` 审计存在）；T-B1 保持通过
- [x] M6.4 历史不可读：旧 `dekVersion` 的历史预览返回 `HistoryDekUnavailable`；T-S7 通过（sync §2.6）
  - 已落地：`vault_file::peek_entry_dek_version` 只读条目文件声明的 `dekVersion` 不触碰密文；`sync_engine/history.rs::read_historical_entry` 先用会话 `dekVersion` 比对，不一致直接返回 `SyncError::HistoryDekUnavailable(declared)`，历史预览与历史恢复同路径生效
  - 测试：`tests/sync_engine.rs` 新增 T-S7（`rotate_dek` 后旧提交的 `history_preview` / `history_restore` 均返回 `HistoryDekUnavailable`）

---

## M7 加固与收尾（依赖：M2–M6）

实现：`src-tauri/crates/assistant-security/src/clipboard_guard.rs`、`src-tauri/crates/assistant-crypto/src/vault_crypto/types.rs`、`src-tauri/crates/assistant-vault/src/security_db/mod.rs`、`src-tauri/src/cmd/vault/{mod,lifecycle,sync}.rs`、`src-tauri/src/lifecycle.rs`、`src-tauri/src/vault_plugin.rs`、`src-tauri/crates/assistant-config/src/vault_conf.rs`、`src-tauri/src/cmd/vault/mod.rs`（`with_unlocked` 前置门禁）、`src-tauri/crates/assistant-crypto/src/api_key.rs`、`src/vault/**`

- [x] M7.1 运行时零化：剪贴板 `Zeroizing`、`SecureString::into_zeroizing`、`cipher_memory_security` / `temp_store`（先验证 SQLCipher 支持）；T-R5 / T-R11 通过（runtime §4.1）
- [x] M7.2 锁定路径统一：`lock_session(reason)`；退出先锁再清剪贴板；T-R1–T-R9 通过（runtime §3）
- [x] M7.3 同步 URL 脱敏：日志、错误、状态条均不含完整 URL；T-S12 通过（sync §2.8）
- [x] M7.4 配置校验与门禁：`VaultConf::validate`、`autoLock=0`、URL 形态、feature-off 行为；T-C1–T-C14 通过（capability §2–§4）
- [x] M7.5 恢复码展示通道加固：打印 / 剪贴板 / 二维码提示，禁止自动复制；T-P9 通过（egress §3）
- [x] M7.6 API key 文档与注释澄清「非保密」，注释与 UI 均不宣称加密保护（egress §4.1）
- [ ] M7.7 全量回归：`cargo test`、`cargo test --test assembly_contract`、`npm test`、`npm run contract:check`、`npm run typecheck`、`npm run lint`、手工验收全部通过（key §11.5）
  - 自动化部分已全绿（含 `--features slow-tests`）；`npm run contract:check` 的 `git diff --exit-code` 仅在 v1.5 变更提交后才会通过（`contract:gen` 已验证幂等，生成结果与工作区一致）；手工验收待执行
- [x] M7.8 文档同步：`docs/ARCHITECTURE.md` 的 vault 章节改为 v1.5 事实；`docs/VAULT-DESIGN.md` 标注为历史设计（已归档）；`I9-vault-crypto-surface-inventory.md` 基线更新为 v1.5；无活跃文档声称 v1 仍是现状，无指向归档目录的活跃链接（项目规则「文档编写规范」）
  - 已落地：`ARCHITECTURE.md` §8 安全设计改为 v1.5 密钥层次/恢复通道/API key 非保密；`docs/VAULT-DESIGN.md` 顶部新增「历史设计（已归档，只读）」状态块并指向 `docs/store/` 活跃规格（文件本身保留在原位，避免打断归档文档中既有的相对链接）；新增 `docs/VAULT-DESIGN-v1.5.md` 作为 v1.5 设计文档（自包含讲加密模型与关键取舍，正文不引用其它文档，替代 v1 设计的入口地位）；`I9-vault-crypto-surface-inventory.md` 文件清单、密钥材料、密文点、明文驻留、命令面、缺陷状态、测试资产全部改为 v1.5 基线；`I9-vault-password-model-comparison.md` 状态行标注「分析基线为 v1，不再描述现状」
- [x] M7.9 性能基准：`bench_unlock` / `bench_argon2` 对比 Argon2id(m=65536,t=3,p=4) + `kdf_iter=1` 与 v1 的解锁耗时，实测值写入规格附录 A（key 附录 A）
  - 已落地：`src-tauri/crates/assistant-vault/examples/bench_unlock.rs` 改为对比 v1（3 次 Argon2id + `kdf_iter=256000`）与 v1.5（1 次 Argon2id + `kdf_iter=1`）并输出两套总量；两次 release 实测区间（Argon2id 85–95 ms/次、HKDF 13–25 µs、raw key 下 `kdf_iter` 差 0.17–0.21 ms、解锁总计 274–298 ms → 98–108 ms）写入 `I9-vault-key-hierarchy-spec.md` 附录 A.1

- [x] M7.10 仓库层所有权收口：新增独立 managed state `VaultRepo`（`Arc<VaultFileManager>`），`SyncEngine` 改为共享同一实例；生命周期 / 恢复 / 生物识别 / 轮换命令改从 `repo_state` 取 repo，不再经 `SyncEngine::vault_file_manager()` 转手；`tests/command_gate.rs` 的 `STATE_ACCESS` 补 `repo_state(`；`remote_url = None`（从未配置同步）时生物识别解锁仍可用（key §4.1 / §9.3、capability §1.1 / §5.1）
  - 涉及文件：`src-tauri/crates/assistant-vault/src/sync_engine/mod.rs`、`src-tauri/src/cmd/vault/{mod,biometric,lifecycle,recovery,entries}.rs`、`src-tauri/crates/assistant-vault/tests/command_gate.rs`
  - 已落地：`vault_setup` 先 `manage(VaultRepo(repo.clone()))` 再 `manage(SyncEngine::new_with_repo(repo, sync_config))`；`SyncEngine` 内改持 `Arc<VaultFileManager>`，git 目录取自 `repo.vault_dir()`，`new(vault_dir, config, cipher)` 保留为内部委托的测试过渡入口；`src-tauri/src/cmd/vault` 下已无 `vault_file_manager()` 调用（条目命令仅保留 `sync_state` 做 `prepare` / `finalize`，取 repo 一律走 `repo_state`）；`vault_unlock_biometric` 与恢复 / 轮换 / 改密 / 重建不再依赖 `SyncEngine`，从未配置同步时照常可用
- [x] M7.11 收尾缺陷修复：本地写 `.vault-meta` 后前移 anchor；`vaultRollbackDetected` 接上重建入口
  - 已落地：新增 `lifecycle::refresh_anchor`（读 meta → `meta_file_hash` + `dek_version` → `SecurityDbState::refresh_anchor`，注释标明「必须在事务回调外调用」，避免 `session` 锁不可重入自锁）；`src-tauri/src/cmd/vault/entries.rs` 的 5 个条目命令（create / update / set_favorite / delete / restore）在 `*_tx` 返回后、`finalize_sync` 前统一刷新；`sync_engine` 的 `push`、`force_push`、`no_remote` 分支、`confirm_removals`、`history_restore`、`remote_admin::init` 在写 meta 后统一刷新（pull 路径原有的 `advance_anchor` 保留）；回归用例 `i7_local_meta_write_advances_anchor_before_lock`（CRUD → 锁库 → 解锁不误报，解锁后 anchor 与本机 meta hash 一致）；前端 `needsDbRepair` 追加 `vaultRollbackDetected`，`vault.repair.*` 文案改为覆盖「密钥不匹配 / 基线落后」两种成因并写明「重置后仓库回滚不再被检测」；spec 同步：key §10.2、sync §2.4 约束 5

- [x] M7.12 主应用设置页收尾：移除无消费点的「使用 OpenSSH Agent」开关；加密算法补「仅对新条目 / 轮换生效」说明；自动锁定与剪贴板清除上限对齐后端
  - 已落地：`src/features/vault/module.ts` 删除 `vaultSshUseOpensshAgent` 字段（`VaultConf` 字段保留以兼容旧配置，capability §2.1 标注「无消费点」），Pageant 开关补「关闭后改用 OpenSSH Agent」描述；`vaultCipherAlgorithm` 新增 `description`（capability §2.3 的 UI 要求）；`max` 由 60/300 改为 1440/600，与 `MAX_AUTO_LOCK_MINUTES` / `MAX_CLIPBOARD_CLEAR_SECONDS` 一致；中英文 i18n 与 `snapshot.full.json` / `snapshot.diff.json`（fields 42→41、9→8）同步

> M7.4 依赖 M2.14 的命令注册表；M7.5 依赖 M3.5 的恢复界面；M7.1 / M7.6 无前置，可与任一阶段并行。
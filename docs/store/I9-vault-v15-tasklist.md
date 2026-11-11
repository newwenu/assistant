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

- [ ] M0.1 `decrypt_opt` 去掉 `unwrap_or_default`；受保护自定义字段解密失败不再以 `.ok()?` 丢弃；整条解密失败返回 `EntryCorrupted`（未新增错误码前先用现有 `Crypto`）。截断密文、非法 UTF-8、字段互换三种输入均须报错（entry-cipher §5.3 / E3）
- [ ] M0.2 改密成功后固定一种旧备份策略（删除改密前的 `security-*.db.bak`，或对备份重新 rekey），不留给用户选择；改密后用旧口令打不开任何残留备份（runtime §4.3 / F13）
- [ ] M0.3 `vault_export_to_file` 增加 `confirm_token("EXPORT-PLAINTEXT")`、路径校验、0600 权限、临时文件 + rename、审计事件；缺 token 与非法路径均被拒（egress §2.1）
- [ ] M0.4 `mask_entry_preview` 脱敏 `notes`；`vault_conflict_reveal` 接 `RevealGuard` + 审计 + TTL；预览显示 `•••`，连续揭示第 4 次返回 `RateLimited`（egress §1.3、sync §2.7）
- [ ] M0.5 灾难恢复导出成功与失败路径都删除 `security/recovery/`，导出结束后该目录不存在（egress §2.3 / F12）
- [ ] M0.6 `vault_import` 写 `entries/*.enc` 并调用 `finalize`，不再只写本机库；导入后不出现「库里有、仓库没有」（entry-cipher §5.2 / F6）
- [ ] M0.7 为 M0.1–M0.6 补断言：`tests/security_db.rs`、`tests/vault_file.rs`、新增 `tests/egress_guard.rs` 全部通过（各规格测试节）

> M0 不引入任何 v1.5 类型。M0.1 是 M2 的前置条件：新库的解密错误若仍被静默吞掉，P0-2 的验收会失真。

---

## M1 `kdf_iter` 与 PRAGMA（依赖：无）

实现：`src-tauri/crates/assistant-vault/src/security_db/mod.rs`、`src-tauri/crates/assistant-vault/src/recovery_key/mod.rs`、`src-tauri/crates/assistant-vault/tests/sqlcipher_pragma.rs`（新建）

- [ ] M1.1 先写 `sqlcipher_kdf_iter_one_is_noop`，验证 raw key 下 `kdf_iter` 的真实语义；两条断言通过。若断言失败，按规格回退为「显式设置 + 解锁端镜像设置」，不改本任务之外的代码（key §11.1 / T-P0-1）
- [ ] M1.2 `SQLCIPHER_PRAGMAS` 改 `kdf_iter = 1`、去重键，补 T-P0-2 / T-P0-3 断言；常量中无重复键且包含 `kdf_iter=1`（key §11.1）
- [ ] M1.3 `recovery_key` 内的 PRAGMA 字面量改走 `SQLCIPHER_PRAGMAS`；全仓不再出现第二份 PRAGMA 字面量（key §6.4）

> M1.1 必须先于 M1.2：这是全案唯一「假设可能不成立」的点，先测后改。M1 与 M0 无依赖关系，可并行。

---

## M2 核心切换（依赖：M1）

### M2.1–M2.2 类型与原语

实现：`src-tauri/crates/assistant-crypto/src/vault_crypto/{types,mod}.rs`

- [ ] M2.1 新增 `Dek` / `Kek` / `DbKey` / `MetaKey`：`Zeroize + Drop`，禁 `Clone` / `Copy` / `Serialize`，`Debug` 脱敏，长度在构造函数内断言；类型单测覆盖长度断言与 `Debug` 输出（key §2.1–2.2）
- [ ] M2.2 新增 `derive_kek` / `derive_db_key` / `derive_entry_key` / `derive_meta_key` / `seal_dek` / `open_dek` / `build_slot_aad` / `canonical_meta_bytes`；删除 v1 `derive_master_key` / `verify_master_password` / `derive_sqlcipher_key` 的生产调用；T-G1 / T-G2 黄金向量通过（key §3.1–3.7）

### M2.3–M2.4 `.vault-meta` v2

实现：`src-tauri/crates/assistant-vault/src/vault_file/{types,mod}.rs`

- [ ] M2.3 新增 `VaultMetaFileV2` / `VaultSlot` / `KdfParams` / `VaultManifest` / `ManifestEntry` / `RecoveryBackup`；删除 `VaultCryptoMeta` / `EmergencyDataFile`；序列化字段名与顺序与规格一致（key §2.3）
- [ ] M2.4 实现 `read_vault_meta_v2`（严格解析、v1 直接拒绝）、`write_vault_meta_v2`（原子写 + 签名）、`compute_manifest`、`meta_file_hash`、`verify_manifest`；删除 5 个 v1 meta 函数；I5 / I10 通过且读取路径不修改旧文件（key §4.2–4.4）

### M2.5–M2.9 本机库与编排

实现：`src-tauri/crates/assistant-vault/src/security_db/{mod,meta,rekey}.rs`、`src-tauri/crates/assistant-vault/src/security_db/entries/tx.rs`、`src-tauri/crates/assistant-vault/src/lifecycle.rs`（新建）、`src-tauri/crates/assistant-vault/src/lib.rs`

- [ ] M2.5 `SecurityDbState` 合并为 `session: Mutex<Option<VaultSession>>` + `epoch`；新增 `with_session`；`init` / `unlock` 改签名；解锁后加探针读；锁定后无半状态，探针失败返回 `LocalDbKeyMismatch`（runtime §2、key §4.5）
- [ ] M2.6 新增 `lifecycle.rs`：`initialize` / `unlock` / `change_password` / `rotate_dek` 骨架；命令层不再自行拼装跨层顺序（key §4.1）
- [ ] M2.7 `change_master_password` 改 O(1)：只重包 `dek` + `PRAGMA rekey`，删除全量条目重加密；I2 / I3 通过，条目文件 sha256 不变（key §4.8）
- [ ] M2.8 `LocalDbKeyMismatch` 两条修复入口；I17 两条路径各成功执行一次（key §4.7）
- [ ] M2.9 `anchor` 三键读写 + `get_anchor` / `set_anchor` + 校验顺序；I4 回滚检测通过（key §4.6）

### M2.10–M2.11 条目层

实现：`src-tauri/crates/assistant-vault/src/vault_file/{types,mod}.rs`、`src-tauri/crates/assistant-vault/src/security_db/entries/{field_crypto,tx}.rs`、`src-tauri/crates/assistant-vault/src/security_db/crypto_ops.rs`

- [ ] M2.10 `EntryFileV2` / `EntryBlobV2` / `TotpBlob`；条目文件 AAD；字段级 AAD（`password` / `notes` / `totp` / `custom:{name}` / `ssh-private-key`）；T-E1–T-E12 通过（entry-cipher §2）
- [ ] M2.11 替换全部 `derive_entry_key` 调用点（17 处 / 7 文件）；`tx.rs` 回调改收 `&VaultSession`；全仓无 `MasterKey` 实参残留（entry-cipher §3、key §4.10）

### M2.12–M2.14 清退与契约

实现：`src-tauri/crates/assistant-vault/src/{security_db,sync_engine,recovery_key,biometric}/**`、`src-tauri/crates/assistant-crypto/src/error.rs`、`src-tauri/crates/assistant-vault/src/dto/{error,convert}.rs`、`src-tauri/src/cmd/vault/{lifecycle,entries}.rs`、`src-tauri/src/vault_plugin.rs`、`src/vault/core/vaultError.ts`

- [ ] M2.12 删除 v1 死代码：`BOOTSTRAP_SALT`、`hash_salt`、第三次 Argon2id、`emergency` 全链路、`vault_crypto_info`、v1 生物识别 key id、v1 恢复码；编译通过且无 v1 字段残留（key §4.2、biometric §2、recovery §0.2）
- [ ] M2.13 核心错误码 5 项 + `convert.rs` 穷尽映射 + 前端错误码数组；T-D1 双向一致（key §7）
- [ ] M2.14 命令层：`vault_init` / `vault_unlock` / `vault_lock` / `vault_change_master_password` 改调 `lifecycle`；新增 `vault_rotate_dek` 骨架与 `vault_reset_repository`；命令注册表校验通过（key §9、capability §3.2）

### M2.15–M2.18 验收与兜底

- [ ] M2.15 P0-2 验收：I1–I5、I10、T-G1 / T-G2 / T-D1；新增 `tests/lifecycle_integration.rs` 与 `tests/dto_contract.rs` 全部通过（key §11.2）
- [ ] M2.16 旧库识别与重置引导：前端识别「版本不支持」错误码后进入独立分支，展示备份提示 + `RESET-VAULT` 二次确认 + 重置后重新初始化，禁止自动删除旧库；手工走通「旧库 → 提示 → 确认 → 新库」，未确认时磁盘无变化（key §4.2、capability §3.2）
- [ ] M2.17 DTO 改动后跑 `npm run contract:gen`，使 `src/types/protocol.contract.ts`、`src/vault/core/contract/vault.contract.ts` 与 Rust 侧一致，禁止手改生成类型；`npm run contract:check` 无差异（项目规则「类型安全规范」）
- [ ] M2.18 v1 残留静态扫描用例：扫描 `master_key_salt` / `emergency` / `BOOTSTRAP_SALT` / `vault_crypto_info` 等符号，断言 `Dek` 无 `Clone`，断言解密路径无 `unwrap_or_default`；扫描通过，新增残留会使其失败（key §4.2、entry-cipher §5.3）

> M2.1–M2.15 必须同批交付：类型、原语、meta、本机库、编排、条目格式、错误码、命令一起切，任一项单独落地都会留下不可编译或不兼容的中间态。M2.16–M2.18 不阻塞该批。
> M2.12 必须与 M2.10 / M2.11 同批：v1 字段的删除与条目格式切换互为条件。
> M2 落地后到 M3 完成前没有纸面恢复路径，因此 M3.1 安排与 M2 并行，不排在其后。

---

## M3 恢复通道与审计（依赖：M2）

实现：`src-tauri/crates/assistant-vault/src/recovery_key/mod.rs`、`src-tauri/crates/assistant-vault/src/audit/{mod,jsonl}.rs`、`src-tauri/crates/assistant-vault/src/security_db/mod.rs`、`src-tauri/crates/assistant-vault/src/dto/{recovery,mod}.rs`、`src-tauri/src/cmd/vault/recovery.rs`、`src-tauri/src/vault_plugin.rs`、`src/vault/**`、`src/i18n/**`

- [ ] M3.1 纸面编解码：Crockford Base32 + CRC32C + 归一化 + 纠错定位；新增 `crc` 依赖；T1–T5b 通过（recovery §1）
- [ ] M3.2 `RecoverySession`（`dek` + `entries_dir` + `manifest`）+ `open_recovery_session` + `export_all`；删除 v1 恢复函数；I7–I9、I11 通过，恢复过程不触碰 `security.db`（recovery §2–§3）
- [ ] M3.3 恢复 DTO 与命令：`vault_export_recovery_key` / `vault_recover_from_paper` / `vault_locate_paper_error` / `vault_has_recovery_backup`；删除 4 个 v1 命令；命令注册表校验通过（recovery §4）
- [ ] M3.4 审计基础设施：`AuditEvent` 增删、`audit/jsonl.rs`、`log_dual`、解锁补录；T-A1–T-A10 通过，重复解锁不产生重复记录（audit §1–§4）
- [ ] M3.5 恢复前端：`RecoveryKeyDialog` / `PaperRecoveryFlow` / `RecoveryKeyForm`；`RecoveryTab` / `DisasterRecovery` 改造；service 方法；手抄 40 字符可完成恢复，输错能定位到组位（recovery §5）
- [ ] M3.6 重写 `tests/recovery_key.rs` 为 40 字符纸面测试，旧 4 组测试全部替换（recovery §7）
- [ ] M3.7 i18n 收口：新增错误码文案（`vault.error.*`）与恢复、重置引导文案；语言切换后无裸中文字面量，i18n key 全覆盖（key §10.4、recovery §8.7）

> M3.1 只依赖 M2.1，是纯函数编解码，无 IO，可与 M2 并行。
> M3.4 必须先于 M3.3 与 M5.2：恢复失败与生物识别失败都需要审计落点。

---

## M4 仓库与同步完整性（依赖：M3）

实现：`src-tauri/crates/assistant-vault/src/security_db/entries/*.rs`、`src-tauri/crates/assistant-vault/src/vault_file/mod.rs`、`src-tauri/crates/assistant-vault/src/sync_engine/{export,pull_push,history,preview}.rs`、`src-tauri/crates/assistant-vault/src/lifecycle.rs`、`src-tauri/src/cmd/vault/mod.rs`

- [ ] M4.1 条目增 / 删 / 改后立即更新 manifest；`change_password` 不重算；`rotate_dek` 全量重算；I6 通过（key §4.2、sync §2.3）
- [ ] M4.2 push 顺序固定：写 `.enc` → manifest → meta → commit；删除 `sync_engine` 内的 meta 写入；meta 只有单一写入者，T-S1 通过（sync §2.2–§2.3）
- [ ] M4.3 pull 顺序固定：git pull → 严格 meta → `vaultId` → `dekVersion` → integrity → manifest → import → anchor；T-S2–T-S7 通过（sync §2.4–§2.6）
- [ ] M4.4 冲突处理：`.vault-meta` 不自动合并；条目预览脱敏；`removed_ids` 以 manifest 为准；T-S3 / T-S4 / T-S8 / T-S9 通过（sync §2.5–§2.7）
- [ ] M4.5 仓库白名单 + `.gitignore` + `.tmp` 清理 + `security/recovery/` 清理；T-S1 / T-S11 通过（sync §2.1、§2.9）

---

## M5 生物识别（依赖：M4）

实现：`src-tauri/crates/assistant-vault/src/security_db/biometric_ops.rs`、`src-tauri/crates/assistant-vault/src/biometric/{mod,windows,macos,linux}.rs`、`src-tauri/crates/assistant-vault/src/lifecycle.rs`、`src/vault/features/biometric/**`

- [ ] M5.1 key id 改名；payload 改 52B `dek‖dbKey`；新增 `fingerprint`；旧 64B payload 被明确拒绝（biometric §2–§4）
- [ ] M5.2 重写 `restore_keys_from_plaintext`：指纹 → 开库 → 探针 → anchor → 写 session；I12 通过，假解锁消失（biometric §5）
- [ ] M5.3 平台解锁入口：Windows 先校验 `vault-sig-hash` 再解密；非 Windows 分步校验；失败映射 `BiometricReset`；T-B4 / T-B5 通过（biometric §6）
- [ ] M5.4 `delete_key` 区分 not-found 与真实失败（Windows / Linux 当前恒返回 `Ok`）；T-B1 / T-B3 在失败路径上返回错误（biometric §11.2）
- [ ] M5.5 注册 / 反注册 / 自动重包；改密失败时只删 fingerprint；T-B6 通过（biometric §7–§8）
- [ ] M5.6 R8：`rotate_dek` 第 1 步删除 `vault-payload`，失败即中止；T-B1 通过（biometric §9）
- [ ] M5.7 R9：平台 ACL 判定，不满足即禁用；不满足平台返回 `BiometricUnavailable`（biometric §10）
- [ ] M5.8 `vault_reset_repository` 清空 4 个 key id，失败即中止；T-B2 / T-B3 通过（biometric §11.1）
- [ ] M5.9 前端：`BiometricReset` 只认该错误码，不再用 `crypto` / `notFound`；强制重注册引导，重置后不自动重试（runtime §4.2、biometric §12）

> M5.4 必须早于 M5.6 与 M6.1：`delete_key` 若仍恒返回 `Ok`，R8 就是假约束。

---

## M6 `rotate_dek`（依赖：M5）

实现：`src-tauri/crates/assistant-vault/src/lifecycle.rs`、`src-tauri/crates/assistant-vault/src/vault_file/mod.rs`、`src-tauri/crates/assistant-vault/src/sync_engine/history.rs`、`src-tauri/src/cmd/vault/entries.rs`、`src/vault/**`

- [ ] M6.1 `lifecycle::rotate_dek`：删凭据 → 生成 `dek_new` → 全量重加密（临时目录 + 批量 rename）→ 新 slot → manifest → meta → anchor；I14 / I15 通过（key §4.9）
- [ ] M6.2 命令与前端：`vault_rotate_dek` + 「轮换数据密钥」入口 + 强警告文案；命令注册表校验通过（key §9.1、recovery §5.2）
- [ ] M6.3 轮换后强制重新注册生物识别，用户拒绝则视为停用；I13 / T-B1 通过（biometric §9）
- [ ] M6.4 历史不可读：旧 `dekVersion` 的历史预览返回 `HistoryDekUnavailable`；T-S7 通过（sync §2.6）

---

## M7 加固与收尾（依赖：M2–M6）

实现：`src-tauri/crates/assistant-vault/src/clipboard_guard.rs`、`src-tauri/crates/assistant-crypto/src/vault_crypto/types.rs`、`src-tauri/crates/assistant-vault/src/security_db/mod.rs`、`src-tauri/src/cmd/vault/{mod,sync}.rs`、`src-tauri/src/vault_plugin.rs`、`src-tauri/crates/assistant-config/src/vault_conf.rs`、`src-tauri/src/command_gate.rs`、`src-tauri/crates/assistant-crypto/src/api_key.rs`、`src/vault/**`

- [ ] M7.1 运行时零化：剪贴板 `Zeroizing`、`SecureString::into_zeroizing`、`cipher_memory_security` / `temp_store`（先验证 SQLCipher 支持）；T-R5 / T-R11 通过（runtime §4.1）
- [ ] M7.2 锁定路径统一：`lock_session(reason)`；退出先锁再清剪贴板；T-R1–T-R9 通过（runtime §3）
- [ ] M7.3 同步 URL 脱敏：日志、错误、状态条均不含完整 URL；T-S12 通过（sync §2.8）
- [ ] M7.4 配置校验与门禁：`VaultConf::validate`、`autoLock=0`、URL 形态、feature-off 行为；T-C1–T-C14 通过（capability §2–§4）
- [ ] M7.5 恢复码展示通道加固：打印 / 剪贴板 / 二维码提示，禁止自动复制；T-P9 通过（egress §3）
- [ ] M7.6 API key 文档与注释澄清「非保密」，注释与 UI 均不宣称加密保护（egress §4.1）
- [ ] M7.7 全量回归：`cargo test`、`cargo test --test assembly_contract`、`npm test`、`npm run contract:check`、`npm run typecheck`、`npm run lint`、手工验收全部通过（key §11.5）
- [ ] M7.8 文档同步：`docs/ARCHITECTURE.md` 的 vault 章节改为 v1.5 事实；`docs/VAULT-DESIGN.md` 标注为历史设计（已归档）；`I9-vault-crypto-surface-inventory.md` 基线更新为 v1.5；无活跃文档声称 v1 仍是现状，无指向归档目录的活跃链接（项目规则「文档编写规范」）
- [ ] M7.9 性能基准：`bench_unlock` / `bench_argon2` 对比 Argon2id(m=65536,t=3,p=4) + `kdf_iter=1` 与 v1 的解锁耗时，实测值写入规格附录 A（key 附录 A）

> M7.4 依赖 M2.14 的命令注册表；M7.5 依赖 M3.5 的恢复界面；M7.1 / M7.6 无前置，可与任一阶段并行。
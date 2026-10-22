# I9 — 密码管理箱（Password Vault）任务清单

> 规格：[I9-vault-spec.md](./I9-vault-spec.md)
> 设计：[VAULT-DESIGN.md](../VAULT-DESIGN.md)
> 前置：I8 ✅（声明式设置系统）、I9 ✅（Provider 多 API 适配）

---

## 阶段 A — P1-A：assistant-security crate 骨架 + security.db

### A1. 新建 assistant-security crate

**文件**：`src-tauri/crates/assistant-security/`

- [ ] A1.1 创建 `Cargo.toml`：package 元数据 + 依赖（`aes-gcm` / `chacha20poly1305` / `argon2` / `hkdf` / `zeroize` / `subtle` / `rusqlite` bundled-sqlcipher / `base64` / `rand` / `serde` / `serde_json` / `chrono` / `uuid` / `sha1` / `totp-rs` / `zxcvbn`）；~~`secrecy`~~ 已移除（见 spec-1 §5.1）
- [ ] A1.2 创建 `src/lib.rs`：模块声明（crypto / vault_crypto / security_db / audit / password_strength / types）
- [ ] A1.3 workspace `Cargo.toml` members 新增 `"crates/assistant-security"`，workspace.dependencies 新增 `assistant-security = { path = "crates/assistant-security" }`
- [ ] A1.4 workspace `Cargo.toml` workspace.dependencies 新增 vault 专用依赖：`argon2` / `hkdf` / `zeroize` / `subtle` / `chacha20poly1305` / `totp-rs` / `sha1` / `zxcvbn`；~~`secrecy`~~ 已移除

**依赖**：无

---

### A2. 安全类型系统

**文件**：`assistant-security/src/types/` + `assistant-security/src/vault_crypto/types.rs`

- [ ] A2.1 `types/mod.rs`：`VaultError` 枚举（Locked / InvalidMasterPassword / EntryNotFound / RateLimited / CryptoError / DbError / IoError / BiometricUnavailable / BiometricCancelled / BiometricFailed(String)）+ `VaultCategory` 枚举（Login/Card/Identity/Note/SshKey）+ `VaultEntry` / `VaultEntryDecrypted` / `VaultEntrySummary` / `CustomField` struct；`VaultEntryDecrypted` **不实现 Clone**（防止解密后多份明文残留，需副本时逐字段重新构造）
- [ ] A2.2 `vault_crypto/types.rs`：`CipherAlgorithm` 枚举（`#[repr(u8)]` + 显式 discriminant：Aes256Gcm(0) / ChaCha20Poly1305(1)，Default = Aes256Gcm，`serde` rename_all="camelCase"）+ `VaultPlaintext`（纯 `Vec<u8>` + Zeroize + Drop；不实现 Clone/Debug/Serialize，自定义 Debug 显示 REDACTED）+ `MasterKey`（[u8; 32] + Zeroize + Drop，不实现 Clone/Debug）+ `PerEntryKey`（[u8; 32]，条目删除后不可复用，无持久化路径）
- [ ] A2.3 `VaultPlaintext` 实现：`From<Vec<u8>>`（自动 zeroize 原始 buffer）+ `expose() -> &[u8]` + `is_empty()` + `from_str()`
- [ ] A2.4 `MasterKey` 实现：`From<[u8; 32]>` + `expose() -> &[u8; 32]` + Drop 自动 zeroize

**依赖**：A1

---

### A3. crypto 模块迁出

**文件**：`assistant-security/src/crypto/` + `assistant-providers/src/`

- [ ] A3.1 `crypto/mod.rs` + `crypto/api_key.rs`：从 `assistant-providers::crypto` 迁出 `get_machine_id` / `encrypt_api_key` / `decrypt_api_key` / `encrypt_api_key_b64` / `decrypt_api_key_b64`，公开 API 签名不变
- [ ] A3.2 `assistant-providers/Cargo.toml` 新增 `assistant-security` 依赖，移除 `aes-gcm` / `sha2` / `machine-uid`（改由 assistant-security 间接提供）
- [ ] A3.3 `assistant-providers/src/crypto.rs` 改为 re-export：`pub use assistant_security::crypto::api_key::*`
- [ ] A3.4 `assistant-providers/src/lib.rs` 无需变更（crypto 模块仍公开）

**依赖**：A2

---

### A4. vault_crypto 核心实现

**文件**：`assistant-security/src/vault_crypto/mod.rs`

- [ ] A4.1 `VaultCrypto::derive_master_key(password, salt)` — Argon2id(m=65536, t=3, p=4, V0x13) 派生；版本号 0x13 对应 Argon2 v1.3（参考 KeePassXC `ARGON2_DEFAULT_VERSION = 0x13`），影响派生结果，不可省略
- [ ] A4.2 `VaultCrypto::verify_master_password(password, stored_hash, hash_salt)` — 派生 + `subtle::ConstantTimeEq` + **立即显式 zeroize derived**；⚠️ `[u8; 32]` 是 Copy 类型，`drop(derived)` 不会清零（Copy 类型 drop 是 no-op），必须 `mut derived; derived.zeroize()`
- [ ] A4.3 `VaultCrypto::derive_entry_key(master_key, entry_id)` — HKDF-SHA256(salt=None, info=entry_id)；salt=None 时使用全零 salt（HKDF 标准行为）
- [ ] A4.4 `VaultCrypto::derive_sqlcipher_key(master_key)` — HKDF-SHA256(salt=None, info="sqlcipher")
- [ ] A4.5 `VaultCrypto::encrypt_field(plaintext, entry_key, algo)` — nonce(12B random) + AES-256-GCM / ChaCha20-Poly1305 → base64(algo_byte‖nonce‖ciphertext‖tag)；algo_byte = CipherAlgorithm 的 repr(u8) discriminant
- [ ] A4.6 `VaultCrypto::decrypt_field(encrypted_b64, entry_key)` — base64 decode → 首字节识别算法（与 discriminant 一致）→ AEAD 解密 → VaultPlaintext（Drop 自动 zeroize）

**依赖**：A2

---

### A5. security_db 模块

**文件**：`assistant-security/src/security_db/`

- [ ] A5.1 `SecurityDbState` struct：`conn: Mutex<Option<Connection>>` + `master_key: Mutex<Option<MasterKey>>` + `locked()` 构造
- [ ] A5.2 `schema.rs`：`vault_entries` / `vault_meta` / `security_audit_log` DDL + 索引（idx_vault_entries_category / idx_vault_entries_updated / idx_vault_entries_deleted）
- [ ] A5.3 `meta.rs`：`get_meta(conn, key) → Option<String>` / `set_meta(conn, key, value)` / `delete_meta(conn, key)`；vault_meta 存储项：master_key_hash / master_key_salt / hash_salt / biometric_key_enc / hibp:{entry_id} 等（P2A 追加 sync_remote_etag / sync_last_pull_at / sync_config）
- [ ] A5.4 `SecurityDbState::init(db_path)` — 创建 security/ 目录 + 两阶段解锁 Phase 1：Argon2id(password, BOOTSTRAP_SALT) → HKDF 派生 sqlcipher_key → 打开 Connection + `PRAGMA key` + 执行 schema DDL + Phase 2：生成双 salt（master_key_salt / hash_salt）→ 派生 master_key → 写入 vault_meta → 若数据库已存在返回 `VaultError::AlreadyExists`
- [ ] A5.5 `SecurityDbState::unlock(master_password, db_path)` — Phase 1（bootstrap）：Argon2id(password, BOOTSTRAP_SALT) → HKDF 派生 sqlcipher_key → 打开 Connection + `PRAGMA key` → Phase 2（full）：从 vault_meta 读取 master_key_salt / hash_salt / master_key_hash → Argon2id 派生 master_key → 常量时间验证 hash → 存入 conn + master_key
- [ ] A5.6 `SecurityDbState::lock()` — `catch_unwind` 包裹（防止 MasterKey Drop panic 导致进程 abort）→ zeroize master_key → close conn（置 None）→ security.db 文件回退为 SQLCipher 密文
- [ ] A5.7 `SecurityDbState::is_locked()` / `get_conn()` / `get_master_key()`
- [ ] A5.8 `SecurityDbState::backup(db_path)` — 复制 security.db 到 backups/ 目录，保留最近 5 份；触发时机：vault 锁定时 / 主密码变更前 / 应用退出时

**依赖**：A4

---

### A6. audit + password_strength 模块

**文件**：`assistant-security/src/audit/` + `assistant-security/src/password_strength/`

- [ ] A6.1 `audit/mod.rs`：`AuditEvent` 枚举（vault_unlock / vault_unlock_failed / vault_lock / vault_reveal / vault_reveal_rate_limited / hibp_audit / hibp_leak_found / master_password_change / recovery_key_generated / biometric_enrolled / ssh_key_loaded / ssh_key_removed）+ `log(conn, event, detail?)` → 写入 security_audit_log；vault_lock 的 detail 含锁定原因：timeout / manual / screen_lock / app_exit
- [ ] A6.2 `password_strength/mod.rs`：`evaluate_strength(password) → Score`（zxcvbn 包装）+ `is_acceptable(score) → bool`

**依赖**：A5

---

### A7. assistant-storage 变更

**文件**：`assistant-storage/Cargo.toml` + `assistant-storage/src/`

- [ ] A7.1 `Cargo.toml`：`rusqlite` feature 从 `bundled` 改为 `bundled-sqlcipher`，新增 `assistant-security` 依赖
- [ ] A7.2 新增 `vault/mod.rs`：vault_entries CRUD 函数签名（create / read / update / delete / list），调用 `assistant_security::vault_crypto` 加解密
- [ ] A7.3 新增 `vault_meta/mod.rs`：KV 读写函数（get_master_key_hash / set_master_key_hash / get_salt / set_salt 等），委托 `assistant_security::security_db::meta`

**依赖**：A5

---

### A8. 验证

- [ ] A8.1 `cargo check` 通过（含 assistant-security + 修改后的 assistant-providers/assistant-storage）
- [ ] A8.2 `cargo test -p assistant-security` 通过
- [ ] A8.3 `cargo test -p assistant-providers` 通过（crypto 迁出后零功能回归：`encrypt_api_key` / `decrypt_api_key` 仍正常工作）
- [ ] A8.4 `assistant.db` 正常打开（SQLCipher 兼容性：不设 PRAGMA key 时行为等同普通 SQLite）
- [ ] A8.5 `security.db` 解锁后 SQL 操作正常，锁定后 `conn` 置 None
- [ ] A8.6 `MasterKey` Drop 自动 zeroize（单元测试：构造 → drop → 验证原地址全零）
- [ ] A8.7 `VaultPlaintext` Drop 后内存清零（单元测试：构造 → drop → 验证原地址全零）
- [ ] A8.8 `CipherAlgorithm` discriminant 与 `algo_byte` 一致（单元测试：加密 → 解密 → 验证首字节）
- [ ] A8.9 `VaultEntryDecrypted` 未实现 Clone（编译验证：`let _ = entry.clone()` 编译失败）
- [ ] A8.10 `VaultPlaintext` 未实现 Serialize（编译验证：防止序列化明文）
- [ ] A8.11 master_key 不持久化到磁盘（代码审查：无 `fs::write` 含 master_key）
- [ ] A8.12 密码不进 JS 堆（代码审查：`vault_copy_password` 返回 void）

**依赖**：A3, A5, A6, A7

---

## 阶段 B — P1-B：vault CRUD + 最小暴露 API

### B1. Tauri commands：生命周期

**文件**：`src-tauri/src/cmd/vault.rs`（新建）

- [ ] B1.1 `vault_init(master_password)` — 生成 salt → 派生 master_key + hash → init security.db → 写入 vault_meta → 备份
- [ ] B1.2 `vault_unlock(master_password)` — 委托 SecurityDbState::unlock + 审计日志
- [ ] B1.3 `vault_is_locked()` — 委托 SecurityDbState::is_locked
- [ ] B1.4 `vault_lock()` — 委托 SecurityDbState::lock + 审计日志

**依赖**：A8

---

### B2. Tauri commands：CRUD

**文件**：`src-tauri/src/cmd/vault.rs`

- [ ] B2.1 `vault_list_entries(category?, search?, tag?)` — SQL 查询 vault_entries（不解密敏感字段）→ `Vec<VaultEntrySummary>`
- [ ] B2.2 `vault_get_entry(id)` — 单条详情（不解密，返回加密字段 + 元数据）→ `VaultEntry`
- [ ] B2.3 `vault_create_entry(entry)` — 逐字段加密 → INSERT → 返回 id
- [ ] B2.4 `vault_update_entry(id, entry)` — 逐字段加密 → UPDATE + revision++
- [ ] B2.5 `vault_delete_entry(id)` — 软删除：`UPDATE SET deleted_at = now()`

**依赖**：B1

---

### B3. Tauri commands：最小暴露 API

**文件**：`src-tauri/src/cmd/vault.rs`

- [ ] B3.1 `vault_copy_password(id)` — 解密 password → ClipboardGuard::copy_and_guard → 密码不进 JS 堆
- [ ] B3.2 `vault_copy_field(id, field_name)` — 解密 customField → ClipboardGuard::copy_and_guard
- [ ] B3.3 `RevealGuard` struct：`last_reveal: Mutex<Instant>` + `reveal_count: AtomicU32`（`Ordering::SeqCst`，速率限制需跨线程一致）+ 速率限制逻辑（3次/分钟）+ 审计日志（每次 reveal 写入 security_audit_log，event_type = `vault_reveal` / `vault_reveal_rate_limited`）+ UI 通知（每次 reveal 显示 toast）+ 单条限制（同一时间只有一个 reveal 有效，防并行批量请求）
- [ ] B3.4 `vault_reveal_password(id)` — RevealGuard 检查 → 解密 → 返回 `{ value, expiresAt }` + 审计日志
- [ ] B3.5 `vault_reveal_notes(id)` — 同上

**依赖**：B2

---

### B4. Tauri commands：主密码变更

**文件**：`src-tauri/src/cmd/vault.rs`

- [ ] B4.1 `vault_change_master_password(current, new)` — 原子流程：验证当前主密码 → WAL checkpoint(FULL) → 备份 → 派生新 master_key + 新 sqlcipher_key → `PRAGMA rekey` → 更新 vault_meta → **disenroll_biometric()**（主密码变更后旧 biometric_key_enc 失效，见 spec-3 §8.5）→ 验证新主密码可解密 → 失败回滚（从 backup 恢复）

**依赖**：B3

---

### B5. Tauri commands：导出/导入

**文件**：`src-tauri/src/cmd/vault.rs`

- [ ] B5.1 `vault_export(format: "csv" | "json") → Path` — 导出全部条目（解密后写入文件，用完 zeroize）
- [ ] B5.2 `vault_import(path, format) → count` — 从 CSV/JSON 导入条目

---

### B6. invoke_handler 注册 + State 管理

**文件**：`src-tauri/src/lib.rs` + `src-tauri/src/cmd/mod.rs` + `src-tauri/src/setup.rs`

- [ ] B6.1 `cmd/mod.rs` 新增 `mod vault; pub use vault::*;`
- [ ] B6.2 `setup.rs` 新增 `SecurityDbState::locked()` 管理 + `RevealGuard` 管理 + `ClipboardGuard` 管理
- [ ] B6.3 `lib.rs` invoke_handler 新增所有 `cmd::vault_*` 条目
- [ ] B6.4 `Cargo.toml` 新增 `assistant-security` 依赖

**依赖**：B5

---

### B7. 验证

- [ ] B7.1 `cargo check` 通过
- [ ] B7.2 完整生命周期：`vault_init` → `vault_unlock` → `vault_list_entries` → `vault_get_entry` → `vault_lock`
- [ ] B7.3 `vault_copy_password` 不返回密码字符串（返回 void，密码不进 JS 堆）
- [ ] B7.4 `vault_reveal_password` 速率限制生效（第 4 次返回 RateLimited）
- [ ] B7.5 `vault_change_master_password` 后可用新密码解锁
- [ ] B7.6 `vault_change_master_password` 原子性：中途崩溃可从 backup 恢复
- [ ] B7.7 `vault_export` 导出后内存中明文已 zeroize
- [ ] B7.8 `vault_get_entry` 返回加密字段 + 元数据（不解密）
- [ ] B7.9 RevealGuard 审计日志写入（event_type = vault_reveal / vault_reveal_rate_limited）
- [ ] B7.10 VaultError 各变体序列化为可读字符串，前端可正确区分处理

**依赖**：B6

---

## 阶段 C — P1-C：前端 UI + 工具注册

### C1. 前端服务层 + 类型

**文件**：`src/tools/password-vault/vault.service.ts`

- [ ] C1.1 定义前端类型：`VaultEntrySummary` / `VaultEntry` / `VaultEntryCreate` / `VaultEntryUpdate` / `VaultCategory` / `RevealResult` / `CustomField`
- [ ] C1.2 实现 invoke 封装：`init` / `unlock` / `lock` / `isLocked` / `listEntries` / `getEntry` / `createEntry` / `updateEntry` / `deleteEntry` / `copyPassword` / `copyField` / `revealPassword` / `revealNotes` / `changeMasterPassword` / `export` / `import` / `isBiometricAvailable` / `unlockBiometric` / `enrollBiometric` / `disenrollBiometric`

**依赖**：B6

---

### C2. vaultStore

**文件**：`src/tools/password-vault/vaultStore.ts`

- [ ] C2.1 状态字段：`isLocked` / `isInitialized` / `entries` / `searchQuery` / `categoryFilter` / `tagFilter` / `biometricAvailable` / `loading` / `error: VaultError | null` / `revealedField: { kind: 'password' | 'notes', value: string, expiresAt: number } | null`（reveal 密码/笔记状态独立，kind 区分，不互相覆盖） / `hibpResults: Map<string, number>`（条目泄露检测结果缓存）
- [ ] C2.2 actions：`init` / `unlock` / `lock` / `loadEntries` / `createEntry` / `updateEntry` / `deleteEntry` / `copyPassword` / `revealPassword` / `revealNotes` / `clearRevealedField`（基于 `expiresAt` 精确超时清零，非固定 30s） / `checkHibp` / `hibpAudit`
- [ ] C2.3 自动锁定计时器：`resetAutoLockTimer()` + 三事件监听（`mousemove` / `keydown` / `visibilitychange`）；`visibilitychange` 切换回前台时重置计时器

**依赖**：C1

---

### C3. UI 组件：VaultUnlock

**文件**：`src/tools/password-vault/VaultUnlock.tsx`

- [ ] C3.1 主密码输入框 + 强度指示条（zxcvbn 实时评估）
- [ ] C3.2 解锁按钮 + 创建密码箱按钮
- [ ] C3.3 生物识别按钮（条件显示）
- [ ] C3.4 恢复密钥入口

**依赖**：C2

---

### C4. UI 组件：VaultList

**文件**：`src/tools/password-vault/VaultList.tsx`

- [ ] C4.1 搜索框 + 分类筛选 Tabs + 标签筛选
- [ ] C4.2 条目卡片列表：标题 + 用户名 + 分类图标 + 收藏标记 + 泄露标记
- [ ] C4.3 新增按钮 + 同步按钮（Phase 2A 启用）
- [ ] C4.4 底部状态栏：条目数 + 泄露数

**依赖**：C2

---

### C5. UI 组件：VaultEntryForm + VaultEntryDetail

**文件**：`src/tools/password-vault/VaultEntryForm.tsx` + `VaultEntryDetail.tsx`

- [ ] C5.1 VaultEntryForm：分类选择 + title/username/password/url/notes/tags + 密码生成器集成（随机字符 + Diceware）+ 自定义字段 + TOTP 字段
- [ ] C5.2 VaultEntryDetail：元信息（has_password / has_notes 布尔标记，不返回明文）+ 复制密码/用户名/URL 按钮 + reveal 密码/笔记（基于服务端 `expiresAt` 精确超时清零，密码/笔记状态独立）+ TOTP 区域 + 编辑/删除按钮

**依赖**：C4

---

### C6. 工具注册 + 路由

**文件**：`src/tools/registry.ts` + `src/tools/password-vault/index.tsx`

- [ ] C6.1 `index.tsx`：defineTool 入口，lazy 加载 VaultApp
- [ ] C6.2 `registry.ts`：新增 `passwordVault` 工具定义 + 加入 tools 数组
- [ ] C6.3 `VaultApp.tsx`：根据 isLocked 切换 VaultUnlock / VaultList

**依赖**：C5

---

### C7. 设置项 + i18n

**文件**：`src/settings/` + `src-tauri/crates/assistant-config/src/app_conf.rs` + `src-tauri/crates/assistant-i18n/locales/`

- [ ] C7.1 Rust `AppConf` struct 新增 5 个 vault 字段 + `#[serde(default)]`
- [ ] C7.2 前端 `AppConf` interface 同步新增 5 个字段
- [ ] C7.3 `AppConfDto` 新增对应字段 + `From<AppConf>` impl 同步更新（四处一致：Rust AppConf + 前端 AppConf + AppConfDto + From<AppConf>）
- [ ] C7.4 `zh-CN.json` + `en-US.json` 新增 `tools.password_vault` 命名空间约 50 词条
- [ ] C7.5 VaultSettings.tsx：自动锁定时间 + 剪贴板清零时间 + 加密算法选择

**依赖**：C6

---

### C8. 验证

- [ ] C8.1 `tsc --noEmit` 通过
- [ ] C8.2 工具箱 security 分类显示密码箱
- [ ] C8.3 完整 CRUD：创建 → 列表 → `getEntry` 详情 → 编辑 → 删除
- [ ] C8.4 复制密码走 Rust 端（不进 JS 堆）
- [ ] C8.5 reveal 密码/笔记基于服务端 `expiresAt` 精确超时清零（非固定 30s）
- [ ] C8.6 reveal 密码和笔记状态独立（kind 区分，reveal 密码不影响笔记显示，反之亦然）
- [ ] C8.7 自动锁定计时器生效（`mousemove` / `keydown` / `visibilitychange` 三事件监听）
- [ ] C8.8 标签页切换回前台时重置自动锁定计时器
- [ ] C8.9 设置项四处同步：Rust `AppConf` + 前端 `AppConf` + `AppConfDto` + `From<AppConf>` 字段一致
- [ ] C8.10 `vault_change_master_password` 中途崩溃可从 backup 恢复
- [ ] C8.11 VaultError 各变体前端正确处理（Locked → 显示解锁界面 / RateLimited → 显示提示 / EntryNotFound → 404 等）
- [ ] C8.12 `VaultEntryDetail` 仅含 `has_password` / `has_notes` 布尔标记，不含明文

**依赖**：C7

---

## 阶段 D — P1-D：TOTP + HIBP + 密码强度

### D1. TOTP 实现

**文件**：`assistant-security/src/vault_crypto/totp.rs` + `src-tauri/src/cmd/vault.rs`

- [ ] D1.1 `totp.rs`：`generate_totp(secret_b32, digits, period) → String` + `time_remaining(period) → u32` + `from_otpauth_uri(uri) → (secret, digits, period, algorithm)`；支持 SHA-1/SHA-256/SHA-512 算法（默认 SHA-1）+ 6/8 位数字 + Steam Guard 模式
- [ ] D1.2 `vault_totp_generate(id)` command：解密 totpSecret → 生成验证码 → totpSecret 不出 Rust
- [ ] D1.3 `vault_totp_time_remaining(period)` command
- [ ] D1.4 前端 VaultEntryDetail：TOTP 圆形倒计时环 + 6 位数字 + 一键复制

**依赖**：C8

---

### D2. HIBP 泄露检测

**文件**：`assistant-security/src/vault_crypto/hibp.rs` + `src-tauri/src/cmd/vault.rs`

- [ ] D2.1 `hibp.rs`：`check_password(password) → Result<u32>` — SHA1 → 前5位hex → GET /range/{prefix} → 本地匹配后缀 + `Add-Padding: true` header（k-匿名增强，服务端返回 100% 范围大小，防流量分析）+ User-Agent: `"Assistant-Security"` + 完整哈希不出 Rust（仅发送前 5 位，后缀本地比对）
- [ ] D2.2 `vault_hibp_check(id)` command：解密 password → check → zeroize → 写入 vault_meta hibp:{id}
- [ ] D2.3 `vault_hibp_audit()` command：逐条解密 → 检测 → 立即 zeroize → 返回 `Vec<(id, count)>`
- [ ] D2.4 前端：条目泄露标记（⚠ hibp_count > 0）+ 全库审计按钮

**依赖**：C8

---

### D3. 密码强度 + Diceware

**文件**：`assistant-security/src/password_strength/` + `src/tools/password-generator/password-generator.service.ts`

- [ ] D3.1 后端：`vault_check_master_password_strength(password) → Score` command
- [ ] D3.2 前端 VaultUnlock：主密码输入实时强度条 + 破解时间 + 拒绝弱密码（Score < Fair 时禁用提交按钮）
- [ ] D3.3 前端 `password-generator.service.ts` 新增 `generatePassphrase(wordCount, separator, capitalize)` — EFF 大词表（7776 词，~50KB gzip ~15KB，内嵌为 const 或 lazy_static）
- [ ] D3.4 VaultEntryForm：密码生成模式切换（随机字符 / 密码短语）

**依赖**：C8

---

### D4. 验证

- [ ] D4.1 TOTP 生成与 Google Authenticator 对比验证（SHA-1 默认 + SHA-256/SHA-512 + 6/8 位）
- [ ] D4.2 TOTP `otpauth://` URI 解析正确（secret / digits / period / algorithm 全部提取）
- [ ] D4.3 HIBP 只发 SHA-1 前 5 位（网络请求验证：抓包确认请求 URL 仅含 5 位 hex）
- [ ] D4.4 HIBP 完整哈希不离境（代码审查：`hibp.rs` 中完整 SHA-1 仅用于本地后缀比对，不传递给 JS）
- [ ] D4.5 HIBP `Add-Padding: true` header 发送（网络请求验证：抓包确认 header 存在）
- [ ] D4.6 弱主密码被拒绝（Score < Fair 时提交按钮禁用）
- [ ] D4.7 Diceware 生成 4 词密码短语（验证词表来源为 EFF 大词表 7776 词）

**依赖**：D1, D2, D3

---

## 阶段 E — P1-E：自动锁定 + 剪贴板守护 + CSP

### E1. ClipboardGuard 实现

**文件**：`assistant-security/src/clipboard_guard.rs`

- [ ] E1.1 `ClipboardGuard` struct：`thread: Option<JoinHandle<()>>` + `original: Mutex<Option<String>>`
- [ ] E1.2 `copy_and_guard(password, timeout)` — 保存原剪贴板 → 写入密码 → spawn 守护线程（sleep timeout → **校验剪贴板仍为密码** → 仅当仍为密码时恢复原内容，防覆盖用户后续复制）
- [ ] E1.3 `force_clear()` — 立即恢复原剪贴板内容
- [ ] E1.4 注册退出钩子：应用关闭时调用 force_clear

**依赖**：D4

---

### E2. 屏幕锁监听

**文件**：`assistant-security/src/screen_lock/`（条件编译）

- [ ] E2.1 `mod.rs`：`ScreenLockListener` trait + `spawn_listener(callback)` 工厂函数
- [ ] E2.2 `windows.rs`：`WTS_SESSION_LOCK` + `PBT_APMSUSPEND` 监听（参考 KeePassXC ScreenLockListenerWin.cpp）
- [ ] E2.3 `macos.rs`：`com.apple.screenIsLocked` Darwin 通知监听
- [ ] E2.4 `linux.rs`：logind `PrepareForSleep` + GNOME Session `StatusChanged` DBus 监听
- [ ] E2.5 `setup.rs` 集成：spawn 屏幕锁监听 → 触发 `SecurityDbState::lock()`

**依赖**：D4

---

### E3. CSP 配置 + DevTools 禁用

**文件**：`src-tauri/tauri.conf.json` + `src-tauri/src/window.rs`

- [ ] E3.1 `tauri.conf.json` security.csp 设为严格策略（`script-src 'self'` + `style-src 'self' 'unsafe-inline'` + `object-src 'none'` 等）
- [ ] E3.2 `window.rs` create_window()：`#[cfg(not(debug_assertions))] { builder = builder.devtools(false); }`

**依赖**：D4

---

### E4. 独立解锁窗口

**文件**：`src-tauri/src/cmd/vault.rs` + `src/vault-unlock.html`

- [ ] E4.1 `vault_show_unlock_window()` command：WebviewWindowBuilder 创建 "vault-unlock" 窗口（400×200, center, resizable=false, devtools=false, 独立 CSP）；若窗口已存在则 **focus 而非新建**
- [ ] E4.2 `vault-unlock.html`：极简页面（<50 行 JS），password input + unlock button + 立即清除 DOM
- [ ] E4.3 解锁成功后通过 Tauri event `vault:unlocked` 通知主窗口 → 主窗口刷新 vaultStore → window.close()

**依赖**：E3

---

### E5. 验证

- [ ] E5.1 OS 屏幕锁 → vault 自动锁定
- [ ] E5.2 5 分钟无操作 → 自动锁定
- [ ] E5.3 复制密码 30s 后剪贴板恢复（仅当剪贴板仍为密码时）
- [ ] E5.4 应用关闭时剪贴板恢复
- [ ] E5.5 CSP 生效（`script-src 'self'`，无 `unsafe-eval`；`eval()` / `new Function()` 抛错）
- [ ] E5.6 release 构建无法打开 DevTools
- [ ] E5.7 独立解锁窗口 V8 isolate 隔离
- [ ] E5.8 独立解锁窗口已存在时 focus 而非新建

**依赖**：E1, E2, E3, E4

---

## 阶段 F — P1-F：生物识别快速解锁

### F1. BiometricKeyStore trait + 平台实现

**文件**：`assistant-security/src/biometric/`

- [ ] F1.1 `mod.rs`：`BiometricKeyStore` trait（is_available / set_key / get_key / delete_key）+ 工厂函数 `create_key_store() → Box<dyn BiometricKeyStore>`
- [ ] F1.2 `windows.rs`：TPM `KeyCredentialManager` + `RequestSignAsync(challenge)` → SHA256(signature) 派生加密密钥（参考 KeePassXC WindowsHello.cpp）
- [ ] F1.3 `macos.rs`：Keychain `kSecClassGenericPassword` + `kSecAccessControlBiometryCurrentSet`（参考 KeePassXC TouchID.mm）
- [ ] F1.4 `linux.rs`：`libsecret` + Polkit（降级为系统密码，无门控）
- [ ] F1.5 `Cargo.toml` 平台特定依赖（条件编译）：windows → `windows` crate; macos → `core-foundation-rs` + `security-framework`; linux → `libsecret`

**依赖**：E5

---

### F2. 生物识别 Tauri commands

**文件**：`src-tauri/src/cmd/vault.rs`

- [ ] F2.1 `vault_enroll_biometric(master_password)` — 派生 master_key → 平台 `set_key(challenge)`（OS 弹确认）→ 生成随机 wrapping_key(32B) → AES-256-GCM(wrapping_key, master_key) → biometric_key_enc → 存入 vault_meta.biometric_key_enc → 审计日志（biometric_enrolled）
- [ ] F2.2 `vault_unlock_biometric()` — 平台 `get_key(challenge)`（OS 弹生物识别）→ 返回 wrapping_key → AES-256-GCM-Decrypt(wrapping_key, biometric_key_enc) → 恢复 master_key → 解锁（master_key 不经 IPC）
- [ ] F2.3 `vault_disenroll_biometric()` — 删除 vault_meta.biometric_key_enc → 平台 `delete_key()` → 审计日志
- [ ] F2.4 `vault_is_biometric_available() → bool`
- [ ] F2.5 前端 VaultUnlock：生物识别按钮条件显示 + 调用 unlock_biometric + 失败回退主密码

**依赖**：F1

---

### F3. 验证

- [ ] F3.1 Windows Hello 弹窗 → 通过 → 解锁成功
- [ ] F3.2 macOS Touch ID 弹窗 → 通过 → 解锁成功
- [ ] F3.3 生物识别不可用 → 回退主密码输入
- [ ] F3.4 生物识别解锁后 master_key 不经 IPC（代码审查：unlock_biometric 不返回 master_key）
- [ ] F3.5 `vault_disenroll_biometric` 后：vault_meta.biometric_key_enc 已删除 + 平台 key 已删除 + 无法再生物识别解锁
- [ ] F3.6 `vault_change_master_password` 后自动 disenroll biometric（旧 biometric_key_enc 失效）
- [ ] F3.7 `cargo check` 通过（三平台条件编译：windows / macos / linux）

**依赖**：F2

---

## 阶段 G — 验收

### G1. 编译验证

- [ ] G1.1 `cargo check` 通过
- [ ] G1.2 `cargo clippy` 无新增 warning
- [ ] G1.3 `cargo test` 通过
- [ ] G1.4 `tsc --noEmit` 通过

**依赖**：F3

---

### G2. 安全不变式验证

- [ ] G2.1 master_key 不持久化到磁盘（代码审查：无 `fs::write` 含 master_key）
- [ ] G2.2 密码不进 JS 堆（`vault_copy_password` 返回 void）
- [ ] G2.3 reveal 速率限制（4 次返回 RateLimited）
- [ ] G2.4 `MasterKey` Drop zeroize（单元测试）
- [ ] G2.5 `VaultPlaintext` Drop zeroize（单元测试）
- [ ] G2.6 `CipherAlgorithm` discriminant 与 algo_byte 一致（单元测试）
- [ ] G2.7 `VaultEntryDecrypted` 未实现 Clone（编译验证）
- [ ] G2.8 `VaultPlaintext` 未实现 Serialize（编译验证）
- [ ] G2.9 CSP 生效（`script-src 'self'`，无 `unsafe-eval`）
- [ ] G2.10 HIBP 完整哈希不离境（代码审查）
- [ ] G2.11 TOTP secret 不出 Rust（代码审查：`vault_totp_generate` 仅返回 6 位验证码）
- [ ] G2.12 `vault_export` 导出后内存中明文已 zeroize
- [ ] G2.13 `verify_master_password` 中 derived key 显式 zeroize（Copy 类型 drop 是 no-op）

**依赖**：G1

---

### G3. 功能验证

- [ ] G3.1 完整生命周期：初始化 → 解锁 → 创建条目 → 列表 → `getEntry` → 编辑 → 复制密码 → 锁定
- [ ] G3.2 主密码变更后可用新密码解锁
- [ ] G3.3 主密码变更后旧 biometric_key_enc 已删除（disenroll）
- [ ] G3.4 主密码变更中途崩溃可从 backup 恢复
- [ ] G3.5 TOTP 生成正确（SHA-1/SHA-256/SHA-512 + 6/8 位 + otpauth URI 解析）
- [ ] G3.6 HIBP 检测正常（k-匿名 + Add-Padding + 完整哈希不离境）
- [ ] G3.7 生物识别解锁正常（Windows / macOS）
- [ ] G3.8 `vault_disenroll_biometric` 后无法再生物识别解锁
- [ ] G3.9 自动锁定正常（屏幕锁 + 超时 + visibilitychange 重置）
- [ ] G3.10 剪贴板守护正常（30s 条件恢复 + 应用关闭恢复）
- [ ] G3.11 reveal 密码/笔记状态独立（kind 区分，基于 expiresAt 精确超时）
- [ ] G3.12 VaultError 各变体前端正确处理
- [ ] G3.13 `VaultEntryDetail` 仅含 has_password/has_notes 布尔标记
- [ ] G3.14 弱主密码被拒绝（Score < Fair 禁用提交）
- [ ] G3.15 Diceware 生成密码短语（EFF 大词表 7776 词）

**依赖**：G2

---

### G4. 跨阶段约束验证（spec-1 §4）

- [ ] G4.1 类型安全：`as unknown as` 仅允许 2 处（`settings/core/types/confUtils.ts` + `i18n/index.ts`），vault 代码禁止使用
- [ ] G4.2 Tauri Command 错误映射：VaultError 10 变体（Locked / InvalidMasterPassword / EntryNotFound / RateLimited / CryptoError / DbError / IoError / BiometricUnavailable / BiometricCancelled / BiometricFailed）均序列化为可读字符串，前端可区分处理
- [ ] G4.3 安全不变式 5 项完整验证（对应 spec-1 §4.4）：① master_key 不持久化 ② 密码不进 JS 堆 ③ reveal 速率限制 ④ MasterKey/VaultPlaintext Drop zeroize ⑤ CSP 生效
- [ ] G4.4 向后兼容：`assistant.db` 零改动（security.db 为独立文件）+ providers crypto 迁出零破坏（re-export 签名不变）
- [ ] G4.5 条件编译策略：三平台代码均编写完整（biometric: windows.rs + macos.rs + linux.rs; screen_lock: windows.rs + macos.rs + linux.rs），`cargo check` 通过
- [ ] G4.6 设置项同步：Rust `AppConf` + 前端 `AppConf` + `AppConfDto` + `From<AppConf>` 四处字段一致

**依赖**：G3
# I9 — 密码管理箱（Password Vault）任务清单

> 规格：[I9-vault-spec.md](./I9-vault-spec.md)
> 设计：[VAULT-DESIGN.md](../VAULT-DESIGN.md)
> 前置：I8 ✅（声明式设置系统）、I9 ✅（Provider 多 API 适配）

---

## 阶段 A — P1-A：assistant-security crate 骨架 + security.db

### A1. 新建 assistant-security crate

**文件**：`src-tauri/crates/assistant-security/`

- [ ] A1.1 创建 `Cargo.toml`：package 元数据 + 依赖（`aes-gcm` / `chacha20poly1305` / `argon2` / `hkdf` / `zeroize` / `secrecy` / `subtle` / `rusqlite` bundled-sqlcipher / `base64` / `rand` / `serde` / `serde_json` / `chrono` / `uuid` / `sha1` / `totp-rs` / `zxcvbn`）
- [ ] A1.2 创建 `src/lib.rs`：模块声明（crypto / vault_crypto / security_db / audit / password_strength / types）
- [ ] A1.3 workspace `Cargo.toml` members 新增 `"crates/assistant-security"`，workspace.dependencies 新增 `assistant-security = { path = "crates/assistant-security" }`
- [ ] A1.4 workspace `Cargo.toml` workspace.dependencies 新增 vault 专用依赖：`argon2` / `hkdf` / `zeroize` / `secrecy` / `subtle` / `chacha20poly1305` / `totp-rs` / `sha1` / `zxcvbn`

**依赖**：无

---

### A2. 安全类型系统

**文件**：`assistant-security/src/types/` + `assistant-security/src/vault_crypto/types.rs`

- [ ] A2.1 `types/mod.rs`：`VaultError` 枚举（Locked / InvalidMasterPassword / EntryNotFound / RateLimited / CryptoError / DbError / IoError）+ `VaultCategory` 枚举（Login/Card/Identity/Note/SshKey）+ `VaultEntry` / `VaultEntryDecrypted` / `VaultEntrySummary` / `CustomField` struct
- [ ] A2.2 `vault_crypto/types.rs`：`CipherAlgorithm` 枚举（Aes256Gcm / ChaCha20Poly1305）+ `VaultPlaintext`（secrecy::Secret + Zeroize + Drop）+ `MasterKey`（[u8; 32] + Zeroize + Drop）+ `PerEntryKey`
- [ ] A2.3 `VaultPlaintext` 实现：`From<Vec<u8>>`（自动 zeroize 原始 buffer）+ `expose()` + `is_empty()` + `from_str()`
- [ ] A2.4 `MasterKey` 实现：`From<[u8; 32]>` + `expose()` + Drop 自动 zeroize

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

- [ ] A4.1 `VaultCrypto::derive_master_key(password, salt)` — Argon2id(m=65536, t=3, p=4) 派生
- [ ] A4.2 `VaultCrypto::verify_master_password(password, stored_hash, hash_salt)` — 派生 + `subtle::ConstantTimeEq` + 立即 zeroize derived
- [ ] A4.3 `VaultCrypto::derive_entry_key(master_key, entry_id)` — HKDF-SHA256(info=entry_id)
- [ ] A4.4 `VaultCrypto::derive_sqlcipher_key(master_key)` — HKDF-SHA256(info="sqlcipher")
- [ ] A4.5 `VaultCrypto::encrypt_field(plaintext, entry_key, algo)` — nonce(12B random) + AES-256-GCM / ChaCha20-Poly1305 → base64(algo_byte‖nonce‖ciphertext‖tag)
- [ ] A4.6 `VaultCrypto::decrypt_field(encrypted_b64, entry_key)` — base64 decode → 识别算法 → AEAD 解密 → VaultPlaintext

**依赖**：A2

---

### A5. security_db 模块

**文件**：`assistant-security/src/security_db/`

- [ ] A5.1 `SecurityDbState` struct：`conn: Mutex<Option<Connection>>` + `master_key: Mutex<Option<MasterKey>>` + `locked()` 构造
- [ ] A5.2 `schema.rs`：`vault_entries` / `vault_meta` / `security_audit_log` DDL + 索引
- [ ] A5.3 `meta.rs`：`get_meta(conn, key) → Option<String>` / `set_meta(conn, key, value)` / `delete_meta(conn, key)`
- [ ] A5.4 `SecurityDbState::init(db_path)` — 创建 security/ 目录 + 打开 Connection（不设 PRAGMA key，首次创建）+ 执行 schema DDL
- [ ] A5.5 `SecurityDbState::unlock(master_password, db_path)` — 读 salt → 派生 master_key → 验证 hash → 派生 sqlcipher_key → 打开 Connection + `PRAGMA key` → 验证可读 vault_meta → 存入 conn + master_key
- [ ] A5.6 `SecurityDbState::lock()` — `catch_unwind` 包裹 → zeroize master_key → close conn（置 None）
- [ ] A5.7 `SecurityDbState::is_locked()` / `get_conn()` / `get_master_key()`
- [ ] A5.8 `SecurityDbState::backup(db_path)` — 复制 security.db 到 backups/ 目录，保留最近 5 份

**依赖**：A4

---

### A6. audit + password_strength 模块

**文件**：`assistant-security/src/audit/` + `assistant-security/src/password_strength/`

- [ ] A6.1 `audit/mod.rs`：`AuditEvent` 枚举 + `log(conn, event)` → 写入 security_audit_log
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

- [ ] A8.1 `cargo check` 通过
- [ ] A8.2 `cargo test -p assistant-security` 通过
- [ ] A8.3 `cargo test -p assistant-providers` 通过（crypto 迁出后零回归）
- [ ] A8.4 手动验证：`assistant.db` 正常打开（SQLCipher 兼容性）

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
- [ ] B2.2 `vault_create_entry(entry)` — 逐字段加密 → INSERT → 返回 id
- [ ] B2.3 `vault_update_entry(id, entry)` — 逐字段加密 → UPDATE + revision++
- [ ] B2.4 `vault_delete_entry(id)` — 软删除：`UPDATE SET deleted_at = now()`

**依赖**：B1

---

### B3. Tauri commands：最小暴露 API

**文件**：`src-tauri/src/cmd/vault.rs`

- [ ] B3.1 `vault_copy_password(id)` — 解密 password → ClipboardGuard::copy_and_guard → 密码不进 JS 堆
- [ ] B3.2 `vault_copy_field(id, field_name)` — 解密 customField → ClipboardGuard::copy_and_guard
- [ ] B3.3 `RevealGuard` struct：`last_reveal: Mutex<Instant>` + `reveal_count: AtomicU32` + 速率限制逻辑（3次/分钟）
- [ ] B3.4 `vault_reveal_password(id)` — RevealGuard 检查 → 解密 → 返回 `{ value, expiresAt }` + 审计日志
- [ ] B3.5 `vault_reveal_notes(id)` — 同上

**依赖**：B2

---

### B4. Tauri commands：主密码变更

**文件**：`src-tauri/src/cmd/vault.rs`

- [ ] B4.1 `vault_change_master_password(current, new)` — 原子流程：验证 → WAL checkpoint → 备份 → PRAGMA rekey → 更新 vault_meta → 验证 → 失败回滚

**依赖**：B3

---

### B5. invoke_handler 注册 + State 管理

**文件**：`src-tauri/src/lib.rs` + `src-tauri/src/cmd/mod.rs` + `src-tauri/src/setup.rs`

- [ ] B5.1 `cmd/mod.rs` 新增 `mod vault; pub use vault::*;`
- [ ] B5.2 `setup.rs` 新增 `SecurityDbState::locked()` 管理 + `RevealGuard` 管理 + `ClipboardGuard` 管理
- [ ] B5.3 `lib.rs` invoke_handler 新增所有 `cmd::vault_*` 条目
- [ ] B5.4 `Cargo.toml` 新增 `assistant-security` 依赖

**依赖**：B4

---

### B6. 验证

- [ ] B6.1 `cargo check` 通过
- [ ] B6.2 完整生命周期：`vault_init` → `vault_unlock` → `vault_list_entries` → `vault_lock`
- [ ] B6.3 `vault_copy_password` 不返回密码字符串（返回 void）
- [ ] B6.4 `vault_reveal_password` 第 4 次返回 RateLimited
- [ ] B6.5 `vault_change_master_password` 后可用新密码解锁

**依赖**：B5

---

## 阶段 C — P1-C：前端 UI + 工具注册

### C1. 前端服务层 + 类型

**文件**：`src/tools/password-vault/vault.service.ts`

- [ ] C1.1 定义前端类型：`VaultEntrySummary` / `VaultEntryCreate` / `VaultEntryUpdate` / `VaultCategory` / `RevealResult`
- [ ] C1.2 实现 invoke 封装：`init` / `unlock` / `lock` / `isLocked` / `listEntries` / `createEntry` / `updateEntry` / `deleteEntry` / `copyPassword` / `copyField` / `revealPassword` / `revealNotes` / `changeMasterPassword`

**依赖**：B6

---

### C2. vaultStore

**文件**：`src/tools/password-vault/vaultStore.ts`

- [ ] C2.1 状态字段：`isLocked` / `isInitialized` / `entries` / `searchQuery` / `categoryFilter` / `tagFilter` / `biometricAvailable` / `loading`
- [ ] C2.2 actions：`init` / `unlock` / `lock` / `loadEntries` / `createEntry` / `updateEntry` / `deleteEntry` / `copyPassword` / `revealPassword`
- [ ] C2.3 自动锁定计时器：`resetAutoLockTimer()` + `visibilitychange` 监听

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
- [ ] C5.2 VaultEntryDetail：元信息 + 复制密码/用户名/URL 按钮 + reveal 密码（30s 超时清零）+ TOTP 区域 + 编辑/删除按钮

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
- [ ] C7.3 `zh-CN.json` + `en-US.json` 新增 `tools.password_vault` 命名空间约 50 词条
- [ ] C7.4 VaultSettings.tsx：自动锁定时间 + 剪贴板清零时间 + 加密算法选择

**依赖**：C6

---

### C8. 验证

- [ ] C8.1 `tsc --noEmit` 通过
- [ ] C8.2 工具箱 security 分类显示密码箱
- [ ] C8.3 完整 CRUD：创建 → 列表 → 编辑 → 删除
- [ ] C8.4 复制密码走 Rust 端
- [ ] C8.5 reveal 密码 30s 后清零
- [ ] C8.6 自动锁定计时器生效

**依赖**：C7

---

## 阶段 D — P1-D：TOTP + HIBP + 密码强度

### D1. TOTP 实现

**文件**：`assistant-security/src/vault_crypto/totp.rs` + `src-tauri/src/cmd/vault.rs`

- [ ] D1.1 `totp.rs`：`generate_totp(secret_b32, digits, period) → String` + `time_remaining(period) → u32` + `from_otpauth_uri(uri) → (secret, digits, period)`
- [ ] D1.2 `vault_totp_generate(id)` command：解密 totpSecret → 生成验证码 → totpSecret 不出 Rust
- [ ] D1.3 `vault_totp_time_remaining(period)` command
- [ ] D1.4 前端 VaultEntryDetail：TOTP 圆形倒计时环 + 6 位数字 + 一键复制

**依赖**：C8

---

### D2. HIBP 泄露检测

**文件**：`assistant-security/src/vault_crypto/hibp.rs` + `src-tauri/src/cmd/vault.rs`

- [ ] D2.1 `hibp.rs`：`check_password(password) → Result<u32>` — SHA1 → 前5位hex → GET /range/{prefix} → 本地匹配后缀 + Add-Padding header
- [ ] D2.2 `vault_hibp_check(id)` command：解密 password → check → zeroize → 写入 vault_meta hibp:{id}
- [ ] D2.3 `vault_hibp_audit()` command：逐条解密 → 检测 → 立即 zeroize → 返回 `Vec<(id, count)>`
- [ ] D2.4 前端：条目泄露标记（⚠ hibp_count > 0）+ 全库审计按钮

**依赖**：C8

---

### D3. 密码强度 + Diceware

**文件**：`assistant-security/src/password_strength/` + `src/tools/password-generator/password-generator.service.ts`

- [ ] D3.1 后端：`vault_check_master_password_strength(password) → Score` command
- [ ] D3.2 前端 VaultUnlock：主密码输入实时强度条 + 破解时间 + 拒绝弱密码
- [ ] D3.3 前端 `password-generator.service.ts` 新增 `generatePassphrase(wordCount, separator, capitalize)` — EFF 大词表
- [ ] D3.4 VaultEntryForm：密码生成模式切换（随机字符 / 密码短语）

**依赖**：C8

---

### D4. 验证

- [ ] D4.1 TOTP 生成与 Google Authenticator 对比验证
- [ ] D4.2 HIBP 只发 SHA-1 前 5 位（网络请求验证）
- [ ] D4.3 弱主密码被拒绝
- [ ] D4.4 Diceware 生成 4 词密码短语

**依赖**：D1, D2, D3

---

## 阶段 E — P1-E：自动锁定 + 剪贴板守护 + CSP

### E1. ClipboardGuard 实现

**文件**：`assistant-security/src/clipboard_guard.rs`

- [ ] E1.1 `ClipboardGuard` struct：`thread: Option<JoinHandle<()>>` + `original: Mutex<Option<String>>`
- [ ] E1.2 `copy_and_guard(password, timeout)` — 保存原剪贴板 → 写入密码 → spawn 守护线程（sleep timeout → 校验仍为密码 → 恢复原内容）
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

- [ ] E4.1 `vault_show_unlock_window()` command：WebviewWindowBuilder 创建 "vault-unlock" 窗口（400×200, center, resizable=false, devtools=false, 独立 CSP）
- [ ] E4.2 `vault-unlock.html`：极简页面（<50 行 JS），password input + unlock button + 立即清除 DOM
- [ ] E4.3 解锁成功后 window.close()

**依赖**：E3

---

### E5. 验证

- [ ] E5.1 OS 屏幕锁 → vault 自动锁定
- [ ] E5.2 5 分钟无操作 → 自动锁定
- [ ] E5.3 复制密码 30s 后剪贴板恢复
- [ ] E5.4 应用关闭时剪贴板恢复
- [ ] E5.5 CSP 生效（`eval()` 抛错）
- [ ] E5.6 release 构建无法打开 DevTools
- [ ] E5.7 独立解锁窗口 V8 isolate 隔离

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

- [ ] F2.1 `vault_enroll_biometric(master_password)` — 派生 master_key → 平台 set_key（OS 弹确认）→ 加密 master_key → 存入 vault_meta.biometric_key_enc
- [ ] F2.2 `vault_unlock_biometric()` — 平台 get_key（OS 弹生物识别）→ 解密 biometric_key_enc → 恢复 master_key → 解锁
- [ ] F2.3 `vault_is_biometric_available() → bool`
- [ ] F2.4 前端 VaultUnlock：生物识别按钮条件显示 + 调用 unlock_biometric + 失败回退主密码

**依赖**：F1

---

### F3. 验证

- [ ] F3.1 Windows Hello 弹窗 → 通过 → 解锁成功
- [ ] F3.2 macOS Touch ID 弹窗 → 通过 → 解锁成功
- [ ] F3.3 生物识别不可用 → 回退主密码
- [ ] F3.4 生物识别解锁后 master_key 不经 IPC
- [ ] F3.5 `cargo check` 通过（三平台条件编译）

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

- [ ] G2.1 master_key 不持久化到磁盘（代码审查）
- [ ] G2.2 密码不进 JS 堆（vault_copy_password 返回 void）
- [ ] G2.3 reveal 速率限制（4 次返回 RateLimited）
- [ ] G2.4 MasterKey Drop zeroize（单元测试）
- [ ] G2.5 CSP= CSP 生效（eval() 抛错）

**依赖**：G1

---

### G3. 功能验证

- [ ] G3.1 完整生命周期：初始化 → 解锁 → 创建条目 → 列表 → 编辑 → 复制密码 → 锁定
- [ ] G3.2 主密码变更后可用新密码解锁
- [ ] G3.3 TOTP 生成正确
- [ ] G3.4 HIBP 检测正常
- [ ] G3.5 生物识别解锁正常（Windows / macOS）
- [ ] G3.6 自动锁定正常（屏幕锁 + 超时）
- [ ] G3.7 剪贴板守护正常（30s 恢复 + 应用关闭恢复）

**依赖**：G2
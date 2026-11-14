# I9-1 — 密码管理箱：安全基础与总览

> 本文档为 I9 密码管理箱规格的 **第 1 部分**，覆盖交付目标、实施路线、P1-A 安全基础 crate、跨阶段约束。
> 关联文档：[I9-vault-spec-2-crud-and-ui.md](./I9-vault-spec-2-crud-and-ui.md) · [I9-vault-spec-3-security-enhance.md](./I9-vault-spec-3-security-enhance.md)
> 设计文档：[VAULT-DESIGN.md](../../VAULT-DESIGN.md)
> 任务清单：[I9-vault-tasklist.md](./I9-vault-tasklist.md)

---

## 1. 交付目标

在现有工具箱体系中新增 **密码管理箱** 工具，提供本地加密存储 + 云同步能力。以独立 `assistant-security` crate 集中安全逻辑，独立 `security.db`（SQLCipher）隔离敏感数据，最小暴露 IPC API 防止 XSS 窃密。

> **前置依赖**：I8 ✅（声明式设置系统）、I9 ✅（Provider 多 API 适配）
> **参考项目**：prs（Git 同步）、KeePassXC（屏幕锁/生物识别/TOTP/HIBP）、Vaultwarden（条目级加密/紧急访问）

---

## 2. 实施路线

### 2.1 分阶段交付

| 阶段 | 交付物 | 核心能力 | 依赖 | 规格文档 |
|------|--------|---------|------|---------|
| **P1-A** | assistant-security crate 骨架 + security.db | crypto 迁出 + SQLCipher 安全数据库 + vault 生命周期 | 无 | 本文档 §3 |
| **P1-B** | vault CRUD + 最小暴露 API | 条目增删改查 + 复制走 Rust + reveal 速率限制 | P1-A | [spec-2](./I9-vault-spec-2-crud-and-ui.md) §4 |
| **P1-C** | 前端 UI + 工具注册 | VaultUnlock + VaultList + VaultEntryForm + VaultEntryDetail | P1-B | [spec-2](./I9-vault-spec-2-crud-and-ui.md) §5 |
| **P1-D** | TOTP + HIBP + 密码强度 | 两步验证 + 泄露检测 + zxcvbn + Diceware | P1-C | [spec-3](./I9-vault-spec-3-security-enhance.md) §6 |
| **P1-E** | 自动锁定 + 剪贴板守护 + CSP | 屏幕锁联动 + Rust 剪贴板线程 + 严格 CSP + DevTools 禁用 | P1-C | [spec-3](./I9-vault-spec-3-security-enhance.md) §7 |
| **P1-F** | 生物识别快速解锁 | Windows Hello / Touch ID / Polkit 门控密钥释放 | P1-E | [spec-3](./I9-vault-spec-3-security-enhance.md) §8 |
| **P2A** | Git 同步 + assistant-sync crate | prepare→操作→finalize + .enc 双写 + 冲突解决 | P1-F | 后续规格 |
| **P2A'** | SSH Agent 集成 | 解锁自动加载 + 锁定自动移除 | P2A | 后续规格 |
| **P2A''** | 恢复密钥 + 紧急访问 | 恢复密钥生成/使用 + 灾难恢复验证 | P2A | 后续规格 |

### 2.2 初始阶段范围（P1-A ~ P1-F）

本规格覆盖 **P1-A 至 P1-F**，即本地密码箱的完整可用版本。P2A/P2A'/P2A'' 在后续规格文档中展开。

> **已砍掉阶段**：~~P2B WebDAV/S3 同步 + AutoType~~（Git 同步已够用，浏览器扩展填充优于 AutoType）、~~P3 KDBX4 导入导出 + Passkey~~（CSV/JSON 导入已覆盖迁移，Passkey 与硬件绑定安全模型矛盾）。

### 2.3 各阶段代码量估算

| 阶段 | 新增行数 | 累计 | 主要新增 |
|------|---------|------|---------|
| P1-A | ~735 | ~735 | crypto 迁出 + vault_crypto + security_db + types |
| P1-B | ~350 | ~1085 | audit + Tauri commands（vault_*） + RevealGuard + CRUD |
| P1-C | ~600 | ~1685 | 前端 6 组件 + vaultStore + vault.service + 工具注册 + i18n |
| P1-D | ~200 | ~1885 | TOTP + HIBP + password_strength + Diceware |
| P1-E | ~280 | ~2165 | clipboard_guard + screen_lock + CSP + 独立解锁窗口 |
| P1-F | ~250 | ~2415 | biometric 三平台实现 |

---

## 3. P1-A — assistant-security crate 骨架 + security.db

### 3.1 交付目标

新建 `assistant-security` crate，从 `assistant-providers` 迁出 `crypto.rs`，实现 SQLCipher 安全数据库初始化/解锁/锁定/备份，建立安全类型系统（`VaultPlaintext` / `MasterKey`）。

### 3.2 新 crate 结构

P1-A 阶段仅建立骨架模块，后续阶段按需填充：

```
assistant-security/
├── src/
│   ├── lib.rs                    ← crate 入口，pub mod 声明
│   ├── crypto/                   ← P1-A：AES-256-GCM / ChaCha20-Poly1305（从 providers 迁出）
│   │   ├── mod.rs                ← 公开 re-export
│   │   └── api_key.rs            ← encrypt/decrypt_api_key（原 crypto.rs，~55 行）
│   ├── vault_crypto/             ← P1-A：Argon2id + HKDF + vault 加解密
│   │   ├── mod.rs                ← VaultCrypto 全部方法（~180 行）
│   │   └── types.rs              ← CipherAlgorithm / VaultPlaintext / MasterKey / PerEntryKey（~90 行）
│   ├── security_db/              ← P1-A：security.db 管理
│   │   ├── mod.rs                ← SecurityDbState + init/unlock/lock/backup（~200 行）
│   │   ├── schema.rs             ← vault_entries / vault_meta / security_audit_log DDL（~50 行）
│   │   └── meta.rs               ← vault_meta KV 读写（~60 行）
│   ├── audit/                    ← P1-B 填充：security_audit_log 写入（~40 行）
│   │   └── mod.rs
│   ├── reveal_guard.rs           ← P1-B 填充：RevealGuard 速率限制（~30 行）
│   ├── clipboard_guard.rs        ← P1-E 填充：剪贴板守护线程（~80 行）
│   ├── screen_lock/              ← P1-E 填充：OS 屏幕锁监听（~120 行）
│   │   ├── mod.rs                ← ScreenLockListener trait + spawn_listener 工厂
│   │   ├── windows.rs            ← WTS_SESSION_LOCK + PBT_APMSUSPEND（~50 行）
│   │   ├── macos.rs              ← com.apple.screenIsLocked Darwin 通知（~40 行）
│   │   └── linux.rs              ← logind PrepareForSleep + GNOME Session（~50 行）
│   ├── biometric/                ← P1-F 填充：生物识别门控密钥释放（~200 行）
│   │   ├── mod.rs                ← BiometricKeyStore trait + 工厂函数
│   │   ├── windows.rs            ← TPM KeyCredentialManager + RequestSignAsync
│   │   ├── macos.rs              ← Keychain + kSecAccessControlBiometryCurrentSet
│   │   └── linux.rs              ← libsecret + Polkit
│   ├── password_strength/        ← P1-D 填充：zxcvbn 主密码强度评估（~30 行）
│   │   └── mod.rs
│   └── types/                    ← P1-A：公共安全类型
│       └── mod.rs                ← VaultError / VaultCategory / VaultEntry / VaultEntryDecrypted（~120 行）
├── Cargo.toml
└── tests/
    ├── vault_crypto.rs           ← P1-A（~100 行）
    └── security_db.rs            ← P1-A（~80 行）
```

> **条件编译说明**：`screen_lock/` 和 `biometric/` 的三平台代码（windows.rs / macos.rs / linux.rs）均编写完整，`#[cfg(target_os)]` 条件编译正常实现。macOS / Linux 验收暂缓（当前仅 Windows 可测试）。

### 3.3 关键接口

> 以下为接口行为说明，具体实现签名在设计文档 §2.2 / §3.1 中给出。spec 层仅约束输入/输出/副作用，不绑定内部字段布局。

**SecurityDbState**（`security_db/mod.rs`）：

| 方法 | 输入 | 输出 | 副作用 / 约束 |
|------|------|------|---------------|
| `locked()` | — | `Self` | 构造锁定状态：conn=None, master_key=None |
| `init(db_path)` | 数据库路径 | `Result<()>` | **仅首次创建**：创建 security/ 目录 → **Phase 1（bootstrap）**：Argon2id(password, BOOTSTRAP_SALT) → HKDF 派生 sqlcipher_key → 打开 Connection + `PRAGMA key` + `PRAGMA cipher_page_size = 4096; PRAGMA kdf_iter = 256000` → 执行 schema DDL → **Phase 2（full）**：生成双 salt（master_key_salt + hash_salt）→ Argon2id 派生 master_key → 写入 vault_meta（master_key_salt / hash_salt / master_key_hash）；若数据库已存在则返回 `VaultError::AlreadyExists`。⚠️ **两阶段解锁设计**：BOOTSTRAP_SALT 为编译期固定常量，仅用于打开 SQLCipher；真正的 master_key 由随机 salt 派生，存储在加密数据库内部（见 §5.3） |
| `unlock(master_password, db_path)` | 主密码 + 路径 | `Result<()>` | **Phase 1（bootstrap）**：Argon2id(password, BOOTSTRAP_SALT) → HKDF 派生 sqlcipher_key → 打开 Connection + `PRAGMA key` → **Phase 2（full）**：从 vault_meta 读取 master_key_salt / hash_salt / master_key_hash → Argon2id 派生 master_key → 常量时间验证 hash → 存入 conn + master_key |
| `lock()` | — | `Result<()>` | `catch_unwind` 包裹（防止 MasterKey Drop panic 导致进程 abort）→ zeroize master_key → close conn（置 None）→ security.db 文件回退为 SQLCipher 密文 |
| `is_locked()` | — | `bool` | 检查 conn 是否为 None |
| `get_conn()` | — | `Result<Connection>` | 锁定时返回 VaultError::Locked |
| `get_master_key()` | — | `Result<MasterKey>` | 锁定时返回 VaultError::Locked；MasterKey 实现 Zeroize + Drop 自动清零 |
| `backup(db_path)` | 数据库路径 | `Result<()>` | 复制 security.db 到 backups/ 目录，保留最近 5 份；触发时机：vault 锁定时 / 主密码变更前 / 应用退出时（参考 VAULT-DESIGN.md §2.3）；备份为加密副本（锁定后 security.db 为 SQLCipher 密文） |

**VaultCrypto**（`vault_crypto/mod.rs`）：

| 方法 | 输入 | 输出 | 副作用 / 约束 |
|------|------|------|---------------|
| `derive_master_key(password, salt)` | 密码 + 32B salt | `Result<[u8; 32]>` | Argon2id(m=65536, t=3, p=4, V0x13)；版本号 0x13 对应 Argon2 v1.3（参考 KeePassXC `ARGON2_DEFAULT_VERSION = 0x13`），影响派生结果，不可省略 |
| `verify_master_password(password, stored_hash, hash_salt)` | 密码 + 哈希 + salt | `Result<bool>` | 派生后用 subtle::ConstantTimeEq 常量时间比较，**立即 zeroize 派生结果**；⚠️ `[u8; 32]` 是 Copy 类型，`drop(derived)` 不会清零（Copy 类型 drop 是 no-op），必须 `mut derived; derived.zeroize()` 显式清零 |
| `derive_entry_key(master_key, entry_id)` | 主密钥 + 条目 ID | `[u8; 32]` | HKDF-SHA256(salt=None, info=entry_id)；salt=None 时使用全零 salt（HKDF 标准行为） |
| `derive_sqlcipher_key(master_key)` | 主密钥 | `[u8; 32]` | HKDF-SHA256(salt=None, info="sqlcipher") |
| `encrypt_field(plaintext, entry_key, algo)` | VaultPlaintext + 密钥 + 算法 | `Result<String>` | nonce(12B random) + AEAD 加密 → base64(algo_byte‖nonce‖ciphertext‖tag) |
| `decrypt_field(encrypted_b64, entry_key)` | base64 密文 + 密钥 | `Result<VaultPlaintext>` | base64 decode → 首字节识别算法 → AEAD 解密 → VaultPlaintext（Drop 自动 zeroize） |

**安全类型**（`vault_crypto/types.rs` + `types/mod.rs`）：

| 类型 | 约束 |
|------|------|
| `VaultPlaintext` | 纯 `Vec<u8>` + `zeroize::Zeroize` + Drop 自动清零；访问通过 `expose() -> &[u8]`；`From<Vec<u8>>` move 语义；不实现 `Clone`/`Debug`（自定义 Debug 显示 REDACTED）；不实现 `Serialize`（防止序列化明文） |
| `MasterKey` | 包装 `[u8; 32]`，实现 Zeroize + Drop 自动清零；不实现 Clone/Debug（防止意外复制/打印） |
| `PerEntryKey` | 包装 `[u8; 32]`，条目删除后不可复用（无持久化路径） |
| `VaultEntryDecrypted` | 与 `VaultEntry` 字段一一对应，`encrypted_*` 替换为 `VaultPlaintext` 明文类型；**不实现 `Clone`**——需要副本时必须逐字段重新构造（防止解密后条目被意外复制导致多份明文残留）；仅 Rust 端内部使用，不跨 IPC |
| `CipherAlgorithm` | 枚举：Aes256Gcm(0) / ChaCha20Poly1305(1)，Default = Aes256Gcm；显式 discriminant 保证密文格式跨版本兼容；`#[repr(u8)]` + `serde` rename_all="camelCase"；`encrypt_field` 输出首字节 `algo_byte: u8` = discriminant |
| `VaultError` | 枚举：Locked / InvalidMasterPassword / EntryNotFound / RateLimited / CryptoError / DbError / IoError |

### 3.4 assistant-providers 变更

- 迁出 `crypto.rs`（当前 54 行，含 `get_machine_id` / `encrypt_api_key` / `decrypt_api_key` / `encrypt_api_key_b64` / `decrypt_api_key_b64`）→ `assistant-security::crypto::api_key`
- `assistant-providers` 内部改为 re-export：`pub use assistant_security::crypto::api_key::{encrypt_api_key, decrypt_api_key, encrypt_api_key_b64, decrypt_api_key_b64, get_machine_id};`，保持对下游零破坏
- `assistant-providers/Cargo.toml` 新增依赖 `assistant-security`
- `assistant-providers` 原有 `aes-gcm` / `base64` / `rand` 依赖保留（其他模块仍使用），`sha2` / `machine-uid` 仅 `crypto.rs` 使用，迁出后可从 `assistant-providers/Cargo.toml` 移除（改由 `assistant-security` 间接提供）

### 3.5 assistant-storage 变更

- workspace `Cargo.toml` 中 `rusqlite` feature 从 `bundled` 改为 `bundled-sqlcipher`（影响所有使用 rusqlite 的 crate，但 SQLCipher 不设 `PRAGMA key` 时行为等同普通 SQLite，因此 `assistant.db` 零改动）
- CRUD 逻辑在 `SecurityDbState`（`assistant-security/src/security_db/mod.rs`）中实现（见 §5 偏差 #4）
- `assistant-storage/Cargo.toml` 新增依赖 `assistant-security`（保留，后续同步阶段仍需）

### 3.6 workspace Cargo.toml 变更

- members 新增 `"crates/assistant-security"`
- workspace.dependencies 新增 `assistant-security = { path = "crates/assistant-security" }`
- P1-A 新增依赖：`argon2` / `hkdf` / `zeroize` / `subtle` / `chacha20poly1305` / `thiserror`（VaultError 枚举需要 `#[derive(Error)]`） / `hex`（sqlcipher_key 十六进制编码用于 PRAGMA key）
- P1-D 新增依赖：`totp-rs` / `sha1` / `zxcvbn`（不在 P1-A 阶段引入）
- `rusqlite` feature 变更为 `bundled-sqlcipher`（见 §3.5）

### 3.7 安全数据库 schema

```sql
CREATE TABLE IF NOT EXISTS vault_entries (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  username TEXT,
  url TEXT,
  category TEXT NOT NULL DEFAULT 'login',
  encrypted_password TEXT NOT NULL,
  encrypted_notes TEXT,
  encrypted_custom_fields TEXT,
  encrypted_totp_secret TEXT,
  totp_digits INTEGER NOT NULL DEFAULT 6,
  totp_period INTEGER NOT NULL DEFAULT 30,
  tags TEXT NOT NULL DEFAULT '[]',
  is_favorite INTEGER NOT NULL DEFAULT 0,
  revision INTEGER NOT NULL DEFAULT 1,
  deleted_at INTEGER,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS vault_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS security_audit_log (
  id TEXT PRIMARY KEY,
  event_type TEXT NOT NULL,
  detail TEXT,
  timestamp INTEGER NOT NULL
);
-- event_type 枚举:
--   vault_unlock / vault_unlock_failed / vault_lock
--   hibp_audit / hibp_leak_found
--   master_password_change
--   recovery_key_generated
--   biometric_enrolled
--   ssh_key_loaded / ssh_key_removed
-- 说明:
--   安全审计日志本身是敏感信息（泄露用户安全习惯），存入加密的安全数据库
--   vault_lock 的 detail 含锁定原因：timeout / manual / screen_lock / app_exit

CREATE INDEX IF NOT EXISTS idx_vault_entries_category
  ON vault_entries(category);
CREATE INDEX IF NOT EXISTS idx_vault_entries_updated
  ON vault_entries(updated_at);
CREATE INDEX IF NOT EXISTS idx_vault_entries_deleted
  ON vault_entries(deleted_at);
```

### 3.8 参考项目

| 参考点 | 参考项目 | 关键文件 |
|--------|---------|---------|
| Argon2id 密钥派生 + 常量时间比较 | Vaultwarden | `src/crypto.rs`（PBKDF2 `hash_password` / `verify_password_hash`，Rust 实现；我们改用 Argon2id，但常量时间比较模式相同：`subtle::ConstantTimeEq`） |
| Argon2id 参数 + 版本号 | KeePassXC | `src/crypto/kdf/Argon2Kdf.h`（`ARGON2_DEFAULT_VERSION = 0x13`，`ARGON2_DEFAULT_MEMORY = 1<<16`，`ARGON2_DEFAULT_PARALLELISM = 4`，`ARGON2_DEFAULT_ROUNDS = 10`）；KeePassXC 默认 t=10，我们选 t=3（与 Vaultwarden PBKDF2 600K 迭代安全等级对齐，Argon2id 内存硬度为主防线） |
| SQLCipher 数据库管理 | Vaultwarden | `src/db/`（Diesel + SQLite，连接管理） |
| 安全类型封装（Plaintext / Ciphertext + zeroize on drop） | prs | `lib/src/types.rs`（`Plaintext(SecVec<u8>)` / `Ciphertext(SecVec<u8>)`，`SecVec` 内存保护 + zeroize on drop；我们用 `secrecy::Secret` 替代已停维的 `secstr`，API 等价） |
| 生物识别快速解锁（Windows Hello / Touch ID 门控密钥释放） | KeePassXC | `src/quickunlock/QuickUnlockInterface.h`（`setKey` / `getKey` / `reset` trait）、`src/quickunlock/WindowsHello.cpp`（TPM `KeyCredentialManager` + `RequestSignAsync` → SHA256(signature) 派生加密密钥）、`src/quickunlock/TouchID.mm`（Keychain + `kSecAccessControlBiometryCurrentSet`） |
| 屏幕锁监听（三平台） | KeePassXC | `src/gui/osutils/winutils/ScreenLockListenerWin.cpp`（`WTSRegisterSessionNotification` + `RegisterPowerSettingNotification`） |
| Git 同步模式（prepare→操作→finalize） | prs | `lib/src/sync.rs`（`Sync` struct + `prepare` / `finalize` / `readyness`）、`lib/src/git.rs`（git CLI 封装：`git_init` / `git_pull` / `git_push` / `git_commit` / `git_fetch`）、`lib/src/store.rs`（`Store` struct + `sync()` helper） |

### 3.9 验收标准

- [ ] `cargo check` 通过（含 assistant-security + 修改后的 assistant-providers/assistant-storage）
- [ ] `cargo test -p assistant-security` 通过
- [ ] `assistant-providers` 的 `encrypt_api_key` / `decrypt_api_key` 仍正常工作（迁出后零功能回归）
- [ ] `security.db` 解锁后 SQL 操作正常，锁定后 `conn` 置 None
- [ ] `MasterKey` Drop 自动 zeroize
- [ ] **安全不变式**（对应 §4.4）：
  - [ ] master_key 不持久化到磁盘（代码审查：无 `fs::write` 含 master_key）
  - [ ] 密码不进 JS 堆（代码审查：`vault_copy_password` 返回 void）
  - [ ] `VaultPlaintext` Drop 后内存清零（单元测试：构造 → drop → 验证原地址全零）
  - [ ] `CipherAlgorithm` discriminant 与 `algo_byte` 一致（单元测试：加密 → 解密 → 验证首字节）

---

## 4. 跨阶段约束

### 4.1 与设计文档一致性

本规格文档为设计文档 [VAULT-DESIGN.md](../../VAULT-DESIGN.md) 的 P1-A~P1-F 实施约束。以下为关键对齐点：

| 维度 | 设计文档 | 本 spec | 对齐状态 |
|------|---------|---------|---------|
| 密钥层级 | §2.2 Argon2id → HKDF → entry_key / sqlcipher_key | §3.3 VaultCrypto 方法表 | ✅ 一致 |
| Argon2id 版本号 | §3.1 显式标注 V0x13 | §3.3 `derive_master_key` 标注 V0x13 | ✅ 一致（已补全） |
| 双 salt 区分 | §2.2 `master_key_salt` / `hash_salt` 两个独立 salt | §3.5 函数名 `get_master_key_salt` / `get_hash_salt` | ✅ 一致（已补全） |
| Copy 类型 zeroize | §3.1 `[u8; 32]` 是 Copy 类型，必须 `mut derived; derived.zeroize()` | §3.3 `verify_master_password` 标注 ⚠️ | ✅ 一致（已补全） |
| HKDF salt=None | §3.1 显式标注 `salt=None` | §3.3 `derive_entry_key` / `derive_sqlcipher_key` 标注 | ✅ 一致（已补全） |
| 数据模型 | §2.3 vault_entries + vault_meta + security_audit_log + 索引 + event_type 枚举 | §3.7 schema DDL + 索引 + event_type 枚举 | ✅ 一致 |
| 安全类型 | §2.2 VaultPlaintext / MasterKey | §3.3 安全类型表 | ✅ 一致 |
| VaultEntryDecrypted 不实现 Clone | §2.3 显式标注 | §3.3 安全类型表 | ✅ 一致（已补全） |
| Tauri Commands | §5 完整命令表（含 P2 同步/SSH/紧急访问） | spec-2 §4 仅 P1-B 范围 | ✅ spec 为子集 |
| 生物识别 | §3.5.2 平台实现表 | spec-3 §8.3 平台实现表 | ✅ 一致 |
| vaultStore | §6.2 含 syncStatus/conflicts/hibpResults | spec-2 §5.4 含 hibpResults（syncStatus/conflicts 属 P2） | ✅ spec 为子集 |
| 工具注册 | §6.1 defineTool | spec-2 §5.2 属性表 | ✅ 一致（移除 createdAt 避免硬编码日期） |

### 4.2 类型安全

- `as unknown as` 仅允许 2 处（`confUtils.ts` + `i18n/index.ts`），vault 代码禁止
- 新增设置项必须同步更新 `AppConf` interface + Rust struct（见 spec-2 §5.6）

### 4.3 Tauri Command 错误映射

所有 `vault_*` command 返回 `Result<T, String>`，Rust 侧 `VaultError` 枚举序列化为可读字符串：

| VaultError 变体 | 前端处理 |
|----------------|---------|
| `Locked` | 提示解锁 / 自动跳转解锁界面 |
| `InvalidMasterPassword` | 输入框标红 + "主密码错误" |
| `EntryNotFound(id)` | 刷新列表 / 提示条目已删除 |
| `RateLimited` | 显示"操作过于频繁，请稍后再试" + 倒计时 |
| `CryptoError(msg)` | 通用错误弹窗 + msg |
| `DbError(msg)` | 通用错误弹窗 + msg |
| `IoError(msg)` | 通用错误弹窗 + msg |

### 4.4 安全不变式

| 不变式 | 验证方式 |
|--------|---------|
| master_key 不持久化到磁盘 | 代码审查：无 fs::write 含 master_key |
| 密码不进 JS 堆（除 reveal） | 代码审查：vault_copy_password 不返回 String |
| reveal 速率限制 | 单元测试：4 次调用返回 RateLimited |
| MasterKey Drop zeroize | 单元测试：drop 后内存全零 |
| CSP 生效 | 集成测试：eval() 抛错 |

### 4.5 向后兼容

- `assistant.db` 零改动（SQLCipher 不设 key 时等同普通 SQLite）
- `assistant-providers` 的 `encrypt_api_key` / `decrypt_api_key` 迁出后功能不变（re-export 保持公开 API）
- 现有工具不受影响（密码箱为新增工具）

### 4.6 条件编译策略

三平台代码（`screen_lock/` 和 `biometric/` 的 windows.rs / macos.rs / linux.rs）均编写完整，`#[cfg(target_os)]` 条件编译正常实现。macOS / Linux 平台的验收标准暂缓（当前仅 Windows 平台可测试），待具备对应平台环境后再行验证。

---

## 5. 实施偏差记录

> 记录 P1-A 实施过程中与原始规格的偏差、原因及安全等价性论证。

### 5.1 `VaultPlaintext`：`secrecy::Secret<Vec<u8>>` → 纯 `Vec<u8>` + `zeroize::Zeroize`

**偏差**：规格 §3.3 要求 `VaultPlaintext` 内部使用 `secrecy::Secret<Vec<u8>>`，实际实现改为纯 `Vec<u8>` + 手动 `Zeroize` + `Drop`。

**原因**：
1. `secrecy 0.8`（USTC 镜像最高可用版本）的 `Secret<S>` 要求 `S: Zeroize`，而 `Vec<u8>: Zeroize` 需要 `zeroize/alloc` feature——`secrecy 0.8` 内部依赖 `zeroize` 时未启用 `alloc`，导致 trait bound 链断裂（编译错误）
2. `secrecy 0.10+` 可解决此问题，但 USTC/清华镜像均未同步该版本，官方 crates.io 在国内网络不可达
3. 内联 secrecy 源码（~200 行）虽可行，但引入 mlock 相关 unsafe（Windows 上 `VirtualLock` 效果有限），维护负担大于收益

**安全属性等价性**：

| 安全属性 | `secrecy::Secret<Vec<u8>>` | 纯 `Vec<u8>` + zeroize | 等价 |
|---------|--------------------------|----------------------|------|
| Drop 自动 zeroize | ✅ `#[zeroize(drop)]` | ✅ `Drop::drop { self.inner.zeroize() }` | ✅ |
| 受控只读访问 | ✅ `expose_secret() -> &S` | ✅ `expose() -> &[u8]` | ✅ |
| 不实现 Clone | ✅ | ✅ 未 derive Clone | ✅ |
| 不实现 Debug（真实内容） | ✅ | ✅ 自定义 Debug 显示 REDACTED | ✅ |
| 不实现 Serialize | ✅ | ✅ 未 derive Serialize | ✅ |
| From 清零原始 buffer | ✅ move 语义 | ✅ `other.zeroize()` 显式清零 | ✅ |
| mlock 内存页锁定 | ✅ | ❌ | ⚠️ 深度防御，非文档必需 |

**结论**：所有文档 §3.3 / §4.4 要求的安全属性均已满足。`mlock` 为深度防御（且 Windows 上效果有限），设计文档未将其列为必需属性。

**影响范围**：
- `vault_crypto/types.rs`：`VaultPlaintext` 实现
- workspace `Cargo.toml`：移除 `secrecy` 依赖
- `assistant-security/Cargo.toml`：移除 `secrecy` 依赖
- `vault_crypto/mod.rs`：`expose()` 替代 `expose_secret()`，API 语义等价

### 5.2 SQLCipher 加密时机：init 即设 PRAGMA key（非 unlock 后生效）

**偏差**：原规格 §3.3 `init()` 描述为"不设 PRAGMA key，空库无需加密"，`unlock()` 后 SQLCipher 加密生效。实际实现改为 `init()` 即设 PRAGMA key，数据库从创建起即为 SQLCipher 加密。

**原因**：
1. SQLCipher 的 `PRAGMA key` 必须在打开 Connection 后、执行任何 DDL/写入前设置，否则写入的数据为明文
2. 若 `init()` 不设 key，写入的 `vault_meta`（含 master_key_hash）为明文；`unlock()` 时设 key 后无法读取明文 meta（SQLCipher 会尝试解密明文数据，报错）
3. 空库"无需加密"的前提是 init 后立即 lock 再 unlock，但用户可能忘记 lock，期间明文数据库含 master_key_hash（安全风险）

**安全属性等价性**：

| 安全属性 | 原规格（init 不加密） | 实际（init 即加密） | 等价 |
|---------|---------------------|-------------------|------|
| 解锁后数据库加密 | ✅ unlock 设 PRAGMA key | ✅ init 已设 PRAGMA key | ✅ |
| 空库期间无敏感数据泄露 | ⚠️ 明文 meta 含 hash | ✅ 全库加密 | ✅ 实际更安全 |
| assistant.db 零影响 | ✅ 不设 key = 普通 SQLite | ✅ 不设 key = 普通 SQLite | ✅ |

**结论**：实际实现比原规格更安全（消除了 init→unlock 之间的明文窗口），功能等价。

### 5.3 两阶段解锁（Two-Phase Unlock）：消除外部 salt 文件

**偏差**：原规格 §3.5 要求双 salt（master_key_salt / hash_salt）存储在 `vault_meta` 表中。早期实现曾改为外部 `security.salt.json` 文件（解决鸡生蛋问题），但外部 salt 文件是单点故障——文件被删/损坏，整个密码箱永久不可恢复。

**最终设计**：两阶段解锁，salt 全部存储在加密数据库内部，无外部依赖：

```
Phase 1（bootstrap）：
  bootstrap_key = Argon2id(password, BOOTSTRAP_SALT)  // 编译期固定常量
  sqlcipher_key = HKDF(bootstrap_key, info="sqlcipher")
  PRAGMA key = hex(sqlcipher_key)
  → 打开加密数据库

Phase 2（full verification）：
  master_key_salt = vault_meta["master_key_salt"]  // 从加密数据库内部读取
  hash_salt = vault_meta["hash_salt"]
  master_key = Argon2id(password, master_key_salt)  // 用随机 salt 派生真正的 master_key
  stored_hash = vault_meta["master_key_hash"]
  verify(master_key_hash, stored_hash)  // 常量时间比较
```

**安全属性分析**：

| 安全属性 | 说明 |
|---------|------|
| BOOTSTRAP_SALT 固定 → 预计算风险？ | bootstrap_key 仅用于打开 SQLCipher（数据库文件本地存储，攻击者需先获取文件）；真正的 entry-level 加密由 master_key（随机 salt）保护 |
| master_key_salt 存储在加密 DB 内 | ✅ 与数据库文件同生共死，无单点故障 |
| hash_salt 存储在加密 DB 内 | ✅ 同上 |
| backup 只需复制单个 .db 文件 | ✅ 无需额外同步 salt 文件 |
| is_initialized() 只检查 .db 文件存在 | ✅ 简化，无外部依赖 |

**与业界对比**：
- KeePassXC：KDBX 格式有自定义文件头部（非加密区）存储 KDF 参数（含 salt），依赖文件格式特性
- 1Password：OPVault 格式类似，头部含非加密的 KDF 参数
- 本方案：利用 SQLCipher 的 PRAGMA key 机制 + 固定 bootstrap salt 实现等价功能，无需自定义文件格式

### 5.4 CRUD 架构：SecurityDbState 内联（非 assistant-storage 模块）

**偏差**：原规格 §3.5 要求在 `assistant-storage` 中新增 `vault/mod.rs` + `vault_meta/mod.rs` 模块，实际 CRUD 逻辑直接在 `SecurityDbState`（`assistant-security/src/security_db/mod.rs`）中实现。

**原因**：
1. vault CRUD 需要同时访问 `Connection` + `MasterKey`（加解密），两者均在 `SecurityDbState` 中管理
2. 若拆到 `assistant-storage`，需跨 crate 传递 `MasterKey` 引用（增加 unsafe 暴露面）
3. `assistant-storage` 的 `assistant.db` 与 `security.db` 是不同数据库，混用同一 crate 管理 vault 会模糊职责边界

**结论**：功能等价，架构更内聚。后续 P2A 同步阶段可能需要 `assistant-storage` 引用 `assistant-security` 的 vault 模块（待 P2A 规格确定）。
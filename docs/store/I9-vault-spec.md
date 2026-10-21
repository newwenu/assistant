# I9 — 密码管理箱（Password Vault）规格说明

## 1. 交付目标

在现有工具箱体系中新增 **密码管理箱** 工具，提供本地加密存储 + 云同步能力。以独立 `assistant-security` crate 集中安全逻辑，独立 `security.db`（SQLCipher）隔离敏感数据，最小暴露 IPC API 防止 XSS 窃密。

> **前置依赖**：I8 ✅（声明式设置系统）、I9 ✅（Provider 多 API 适配）
> **设计文档**：[VAULT-DESIGN.md](../VAULT-DESIGN.md)
> **参考项目**：prs（Git 同步）、KeePassXC（屏幕锁/生物识别/TOTP/HIBP）、Vaultwarden（条目级加密/紧急访问）

---

## 2. 实施路线

### 2.1 分阶段交付

| 阶段 | 交付物 | 核心能力 | 依赖 |
|------|--------|---------|------|
| **P1-A** | assistant-security crate 骨架 + security.db | crypto 迁出 + SQLCipher 安全数据库 + vault 生命周期 | 无 |
| **P1-B** | vault CRUD + 最小暴露 API | 条目增删改查 + 复制走 Rust + reveal 速率限制 | P1-A |
| **P1-C** | 前端 UI + 工具注册 | VaultUnlock + VaultList + VaultEntryForm + VaultEntryDetail | P1-B |
| **P1-D** | TOTP + HIBP + 密码强度 | 两步验证 + 泄露检测 + zxcvbn + Diceware | P1-C |
| **P1-E** | 自动锁定 + 剪贴板守护 + CSP | 屏幕锁联动 + Rust 剪贴板线程 + 严格 CSP + DevTools 禁用 | P1-C |
| **P1-F** | 生物识别快速解锁 | Windows Hello / Touch ID / Polkit 门控密钥释放 | P1-E |
| **P2A** | Git 同步 + assistant-sync crate | prepare→操作→finalize + .enc 双写 + 冲突解决 | P1-F |
| **P2A'** | SSH Agent 集成 | 解锁自动加载 + 锁定自动移除 | P2A |
| **P2A''** | 恢复密钥 + 紧急访问 | 恢复密钥生成/使用 + 灾难恢复验证 | P2A |
| **P2B** | WebDAV/S3 同步 + AutoType | SyncBackend trait + enigo 自动填充 | P2A |
| **P3** | KDBX4 导入导出 + Passkey | rust-keepass 集成 + WebAuthn | P2B |

### 2.2 初始阶段范围（P1-A ~ P1-F）

本规格文档覆盖 **P1-A 至 P1-F**，即本地密码箱的完整可用版本。P2A/P2B/P3 在后续规格文档中展开。

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
│   ├── clipboard_guard.rs        ← P1-E 填充：剪贴板守护线程（~80 行）
│   ├── screen_lock/              ← P1-E 填充：OS 屏幕锁监听（条件编译，~120 行）
│   │   ├── mod.rs                ← ScreenLockListener trait + spawn_listener 工厂
│   │   ├── windows.rs            ← WTS_SESSION_LOCK + PBT_APMSUSPEND（~50 行）
│   │   ├── macos.rs              ← com.apple.screenIsLocked Darwin 通知（~40 行）
│   │   └── linux.rs              ← logind PrepareForSleep + GNOME Session（~50 行）
│   ├── biometric/                ← P1-F 填充：生物识别门控密钥释放（条件编译，~200 行）
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

**P1-A 阶段代码量估算**：~735 行（crypto 55 + vault_crypto 270 + security_db 310 + types 120 - 20 lib.rs pub mod）

**各阶段累计代码量**：

| 阶段 | 新增行数 | 累计 | 主要新增 |
|------|---------|------|---------|
| P1-A | ~735 | ~735 | crypto 迁出 + vault_crypto + security_db + types |
| P1-B | ~350 | ~1085 | audit + Tauri commands（vault_*) + RevealGuard + CRUD |
| P1-C | ~600 | ~1685 | 前端 6 组件 + vaultStore + vault.service + 工具注册 + i18n |
| P1-D | ~200 | ~1885 | TOTP + HIBP + password_strength + Diceware |
| P1-E | ~280 | ~2165 | clipboard_guard + screen_lock + CSP + 独立解锁窗口 |
| P1-F | ~250 | ~2415 | biometric 三平台实现 |

### 3.3 关键接口

> 以下为接口行为说明，具体实现签名在设计文档 §2.2 / §3.1 中给出。spec 层仅约束输入/输出/副作用，不绑定内部字段布局。

**SecurityDbState**（`security_db/mod.rs`）：

| 方法 | 输入 | 输出 | 副作用 / 约束 |
|------|------|------|---------------|
| `locked()` | — | `Self` | 构造锁定状态：conn=None, master_key=None |
| `init(db_path)` | 数据库路径 | `Result<()>` | 创建 security/ 目录 + 新(不设 PRAGMA key) + 执行 schema DDL |
| `unlock(master_password, db_path)` | 主密码 + 路径 | `Result<()>` | 读 salt → Argon2id 派生 master_key → 常量时间验证 hash → HKDF 派生 sqlcipher_key → 打开 Connection + `PRAGMA key` → 验证可读 vault_meta → 存入 conn + master_key |
| `lock()` | — | `Result<()>` | catch_unwind 包裹 → zeroize master_key → close conn（置 None）→ security.db 文件回退为 SQLCipher 密文 |
| `is_locked()` | — | `bool` | 检查 conn 是否为 None |
| `get_conn()` | — | `Result<Connection>` | 锁定时返回 VaultError::Locked |
| `get_master_key()` | — | `Result<MasterKey>` | 锁定时返回 VaultError::Locked；MasterKey 实现 Zeroize + Drop 自动清零 |
| `backup(db_path)` | 数据库路径 | `Result<()>` | 复制 security.db 到 backups/ 目录，保留最近 5 份 |

**VaultCrypto**（`vault_crypto/mod.rs`）：

| 方法 | 输入 | 输出 | 副作用 / 约束 |
|------|------|------|---------------|
| `derive_master_key(password, salt)` | 密码 + 32B salt | `Result<[u8; 32]>` | Argon2id(m=65536, t=3, p=4) |
| `verify_master_password(password, stored_hash, hash_salt)` | 密码 + 哈希 + salt | `Result<bool>` | 派生后用 subtle::ConstantTimeEq 比较，立即 zeroize 派生结果 |
| `derive_entry_key(master_key, entry_id)` | 主密钥 + 条目 ID | `[u8; 32]` | HKDF-SHA256(info=entry_id) |
| `derive_sqlcipher_key(master_key)` | 主密钥 | `[u8; 32]` | HKDF-SHA256(info="sqlcipher") |
| `encrypt_field(plaintext, entry_key, algo)` | VaultPlaintext + 密钥 + 算法 | `Result<String>` | nonce(12B random) + AEAD 加密 → base64(algo_byte‖nonce‖ciphertext‖tag) |
| `decrypt_field(encrypted_b64, entry_key)` | base64 密文 + 密钥 | `Result<VaultPlaintext>` | base64 decode → 首字节识别算法 → AEAD 解密 → VaultPlaintext（Drop 自动 zeroize） |

**安全类型**（`vault_crypto/types.rs` + `types/mod.rs`）：

| 类型 | 约束 |
|------|------|
| `VaultPlaintext` | 内部 `secrecy::Secret<Vec<u8>>`，访问必须通过 `expose()`；From 转换自动 zeroize 原始 buffer；`#[zeroize(drop)]` |
| `MasterKey` | 包装 `[u8; 32]`，实现 Zeroize + Drop 自动清零；不实现 Clone/Debug（防止意外复制/打印） |
| `PerEntryKey` | 包装 `[u8; 32]`，条目删除后不可复用（无持久化路径） |
| `CipherAlgorithm` | 枚举：Aes256Gcm(0) / ChaCha20Poly1305(1)，Default = Aes256Gcm |
| `VaultError` | 枚举：Locked / InvalidMasterPassword / EntryNotFound / RateLimited / CryptoError / DbError / IoError |

### 3.4 assistant-providers 变更

- 迁出 `crypto.rs`（当前 54 行，含 `get_machine_id` / `encrypt_api_key` / `decrypt_api_key` / `encrypt_api_key_b64` / `decrypt_api_key_b64`）→ `assistant-security::crypto::api_key`
- `assistant-providers` 内部改为 re-export：`pub use assistant_security::crypto::api_key::{encrypt_api_key, decrypt_api_key, encrypt_api_key_b64, decrypt_api_key_b64, get_machine_id};`，保持对下游零破坏
- `assistant-providers/Cargo.toml` 新增依赖 `assistant-security`
- `assistant-providers` 原有 `aes-gcm` / `sha2` / `base64` / `machine-uid` / `rand` 依赖保留（其他模块仍使用），但 `crypto.rs` 本体删除

### 3.5 assistant-storage 变更

- workspace `Cargo.toml` 中 `rusqlite` feature 从 `bundled` 改为 `bundled-sqlcipher`（影响所有使用 rusqlite 的 crate，但 SQLCipher 不设 `PRAGMA key` 时行为等同普通 SQLite，因此 `assistant.db` 零改动）
- `assistant-storage/Cargo.toml` 新增依赖 `assistant-security`
- 新增 `vault/mod.rs` 模块：vault_entries CRUD 函数签名（create / read / update / delete / list），调用 `assistant_security::vault_crypto` 加解密
- 新增 `vault_meta/mod.rs` 模块：KV 读写函数（get_master_key_hash / set_master_key_hash / get_salt / set_salt 等），委托 `assistant_security::security_db::meta`

### 3.6 workspace Cargo.toml 变更

- members 新增 `"crates/assistant-security"`
- workspace.dependencies 新增 `assistant-security = { path = "crates/assistant-security" }`
- P1-A 新增依赖：`argon2` / `hkdf` / `zeroize` / `secrecy` / `subtle` / `chacha20poly1305`
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
```

### 3.8 验收标准

- [ ] `cargo check` 通过（含 assistant-security + 修改后的 assistant-providers/assistant-storage）
- [ ] `cargo test -p assistant-security` 通过
- [ ] `assistant-providers` 的 `encrypt_api_key` / `decrypt_api_key` 仍正常工作（迁出后零功能回归）
- [ ] `security.db` 解锁后 SQL 操作正常，锁定后 `conn` 置 None
- [ ] `MasterKey` Drop 自动 zeroize

---

## 4. P1-B — vault CRUD + 最小暴露 API

### 4.1 交付目标

实现 vault 条目的增删改查，通过最小暴露 IPC API 确保密码不进入 JS 堆（除 `vault_reveal_password` 受速率限制）。

### 4.2 Tauri Commands

| Command | 签名 | 密码进 JS | 说明 |
|---------|------|:---------:|------|
| `vault_init` | `(masterPassword) → void` | ❌ | 初始化密码箱 |
| `vault_unlock` | `(masterPassword) → bool` | ❌ | 验证主密码并解锁 |
| `vault_is_locked` | `() → bool` | ❌ | 查询锁定状态 |
| `vault_lock` | `() → void` | ❌ | 锁定（zeroize + close conn） |
| `vault_list_entries` | `(category?, search?, tag?) → VaultEntrySummary[]` | ❌ | 列表（不解密） |
| `vault_copy_password` | `(id) → void` | ❌ | Rust 端解密 + 写剪贴板 |
| `vault_copy_field` | `(id, fieldName) → void` | ❌ | 复制 customField |
| `vault_reveal_password` | `(id) → { value, expiresAt }` | ✅ | 速率限制 3/分钟 + 审计 |
| `vault_reveal_notes` | `(id) → { value, expiresAt }` | ✅ | 同上 |
| `vault_create_entry` | `(entry) → id` | ❌ | 新增 |
| `vault_update_entry` | `(id, entry) → void` | ❌ | 更新 |
| `vault_delete_entry` | `(id) → void` | ❌ | 软删除 |
| `vault_change_master_password` | `(current, new) → void` | ❌ | 原子变更 + 全量重加密 |

### 4.3 RevealGuard 速率限制

RevealGuard 为 Tauri State 管理的运行时守卫，约束 `vault_reveal_password` / `vault_reveal_notes` 的调用频率：

| 属性 | 值 |
|------|---|
| 速率上限 | 3 次/分钟（滑动窗口） |
| 超限行为 | 返回 `VaultError::RateLimited`，前端显示"操作过于频繁，请稍后再试" |
| 审计 | 每次 reveal（无论成功或超限）写入 `security_audit_log`，event_type = `vault_reveal` / `vault_reveal_rate_limited` |
| 计数重置 | 滑动窗口：记录最近 3 次 reveal 的时间戳，若最早一次距今 > 60s 则允许新请求 |
| 线程安全 | 内部使用 `Mutex<Vec<Instant>>` 保护时间戳列表 |

### 4.4 主密码变更原子流程

1. 验证当前主密码
2. WAL checkpoint(FULL)
3. 备份 security.db
4. 派生新 master_key + 新 sqlcipher_key
5. `PRAGMA rekey = 新 sqlcipher_key`（页面级原子重加密）
6. 逐条重加密 .enc 文件
7. 更新 vault_meta + .vault-meta
8. 验证新主密码可解密
9. 失败回滚：从 backup 恢复

### 4.5 invoke_handler 注册

`src-tauri/src/lib.rs` 的 `invoke_handler` 宏新增所有 `cmd::vault_*` 条目。

### 4.6 验收标准

- [ ] `vault_init` → `vault_unlock` → `vault_list_entries` → `vault_lock` 完整生命周期
- [ ] `vault_copy_password` 密码不进 JS 堆（Rust 端直接写剪贴板）
- [ ] `vault_reveal_password` 速率限制生效（第 4 次返回 RateLimited）
- [ ] `vault_change_master_password` 原子性（中途崩溃可从 backup 恢复）

---

## 5. P1-C — 前端 UI + 工具注册

### 5.1 交付目标

实现密码箱完整前端 UI，注册为工具箱 `security` 分类工具。

### 5.2 工具注册

注册为工具箱 `security` 分类工具，遵循项目现有 `defineTool()` 模式（参考 `password-generator/index.tsx`、`fs-link/index.tsx`）：

| 属性 | 值 |
|------|---|
| id | `"password-vault"` |
| name | `"tools.password_vault.name"`（i18n key） |
| description | `"tools.password_vault.description"`（i18n key） |
| icon | `SafetyCertificateOutlined`（Ant Design Icons） |
| keywords | `["vault", "password", "manager", "safe", "credential", "locker", "keepass"]` |
| component | `lazyWithRecovery(() => import("./password-vault"))` |
| widthMode | `"wide"` |

### 5.3 前端文件结构

```
src/tools/password-vault/
├── index.tsx              ← defineTool 入口
├── VaultApp.tsx           ← 主界面路由（解锁/列表/编辑）
├── VaultUnlock.tsx        ← 主密码解锁
├── VaultList.tsx          ← 条目列表 + 搜索/分类/标签筛选
├── VaultEntryForm.tsx     ← 新增/编辑条目
├── VaultEntryDetail.tsx   ← 条目详情（复制/显示密码）
├── VaultSettings.tsx      ← 密码箱设置（自动锁定时间/剪贴板清零/加密算法）
├── vaultStore.ts          ← Zustand 状态
├── vault.service.ts       ← 前端服务层（invoke 封装）
└── password-vault.module.css
```

### 5.4 vaultStore 状态

与设计文档 §6.2 对齐。Zustand store，以下为状态字段 + actions 的完整清单：

**状态字段**：

| 字段 | 类型 | 说明 |
|------|------|------|
| `isLocked` | `boolean` | 密码箱是否锁定 |
| `isInitialized` | `boolean` | 是否已初始化（有主密码） |
| `loading` | `boolean` | 全局加载中（unlock/createEntry 等异步操作） |
| `error` | `string \| null` | 最近一次操作错误（显示后自动清零） |
| `entries` | `VaultEntrySummary[]` | 条目列表（不解密，仅元数据） |
| `searchQuery` | `string` | 搜索关键词 |
| `categoryFilter` | `VaultCategory \| null` | 分类筛选 |
| `tagFilter` | `string \| null` | 标签筛选 |
| `revealedField` | `{ value: string; expiresAt: number } \| null` | 当前 reveal 的字段值 + 过期时间（30s 后自动清零） |
| `hibpResults` | `Map<string, number>` | 条目 ID → HIBP 泄露次数缓存 |
| `biometricAvailable` | `boolean` | 平台是否支持生物识别 |

**核心 actions**：

| Action | 签名 | 说明 |
|--------|------|------|
| `init` | `(masterPassword) → Promise<void>` | 初始化密码箱 |
| `unlock` | `(masterPassword) → Promise<boolean>` | 主密码解锁 |
| `lock` | `() → Promise<void>` | 锁定 |
| `loadEntries` | `() → Promise<void>` | 刷新条目列表 |
| `createEntry` | `(entry) → Promise<string>` | 新增条目 |
| `updateEntry` | `(id, entry) → Promise<void>` | 更新条目 |
| `deleteEntry` | `(id) → Promise<void>` | 软删除条目 |
| `copyPassword` | `(id) → Promise<void>` | Rust 端复制（不进 JS 堆） |
| `revealPassword` | `(id) → Promise<void>` | reveal 到 `revealedField`（受 RevealGuard 限制） |
| `resetAutoLockTimer` | `() → void` | 重置自动锁定计时器 |

### 5.5 UI 组件规格

| 组件 | 核心功能 |
|------|---------|
| VaultUnlock | 主密码输入 + 生物识别按钮 + 创建密码箱入口 + 恢复密钥入口 |
| VaultList | 搜索框 + 分类筛选 + 标签筛选 + 新增按钮 + 条目卡片列表 + 泄露标记 |
| VaultEntryForm | 分类选择 + 标题/用户名/密码/TOTP/标签/备注 + 密码生成器集成 + 自定义字段 |
| VaultEntryDetail | 元信息展示 + 复制密码/用户名/URL + TOTP 倒计时环 + reveal 30s 超时清零 |

### 5.6 设置项扩展

AppConf 新增字段（同步更新 Rust struct + `#[serde(default)]`）：

| 字段 | 类型 | 默认值 |
|------|------|--------|
| `vaultAutoLockMinutes` | `number` | 5 |
| `vaultLockOnScreenLock` | `boolean` | true |
| `vaultClipboardClearSeconds` | `number` | 30 |
| `vaultCipherAlgorithm` | `"aes256Gcm" \| "chacha20Poly1305"` | `aes256Gcm` |
| `vaultBiometricEnabled` | `boolean` | false |

### 5.7 i18n

`zh-CN.json` / `en-US.json` 的 `tools.password_vault` 命名空间下新增约 50 个词条。

### 5.8 验收标准

- [ ] 工具箱 security 分类显示密码箱工具
- [ ] 完整 CRUD 流程：创建 → 列表 → 编辑 → 删除
- [ ] 复制密码走 Rust 端（不进 JS 堆）
- [ ] reveal 密码 30s 后自动清零
- [ ] 自动锁定计时器生效

---

## 6. P1-D — TOTP + HIBP + 密码强度

### 6.1 交付目标

实现 TOTP 两步验证生成、HIBP k-匿名泄露检测、主密码强度校验、Diceware 密码短语生成。

### 6.2 TOTP

| 接口 | 说明 |
|------|------|
| `vault_totp_generate(id) → string` | Rust 端解密 totpSecret + 生成 6/8 位验证码，totpSecret 不出 Rust |
| `vault_totp_time_remaining(period) → u32` | 剩余有效秒数 |

前端：圆形倒计时环 + 6 位数字 + 一键复制，每秒刷新。

### 6.3 HIBP

| 接口 | 说明 |
|------|------|
| `vault_hibp_check(id) → u32` | 单条 k-匿名校验 |
| `vault_hibp_audit() → Vec<(id, count)>` | 全库审计（逐条解密→检测→立即 zeroize） |

k-匿名流程：`SHA1(password) → 前5位hex → GET /range/{prefix} → 本地匹配后缀`

### 6.4 主密码强度

- 初始化/变更时用 `zxcvbn` 评估，拒绝评分低于 "fair" 的主密码
- 前端实时显示强度条 + 破解时间估算

### 6.5 Diceware 密码短语

扩展现有 `password-generator.service.ts`：新增 `generatePassphrase(wordCount, separator, capitalize)`，从 EFF 大词表（7776 词）随机取词拼接。纯前端实现。

### 6.6 验收标准

- [ ] TOTP 生成正确（与 Google Authenticator 对比）
- [ ] HIBP k-匿名：只发 SHA-1 前 5 位，完整哈希不离境
- [ ] 弱主密码被拒绝（zxcvbn 评分 < fair）
- [ ] Diceware 生成 4 词密码短语

---

## 7. P1-E — 自动锁定 + 剪贴板守护 + CSP

### 7.1 交付目标

实现多层自动锁定、Rust 剪贴板守护线程、严格 CSP 配置、release DevTools 禁用、独立解锁窗口。

### 7.2 自动锁定（三层触发）

| 优先级 | 触发 | 实现 |
|--------|------|------|
| 1 | OS 屏幕锁/睡眠/用户切换 | Rust 端监听系统事件 → `vault_lock()` |
| 2 | 前端计时器 | `setTimeout`（默认 5 分钟）+ `visibilitychange` |
| 3 | 手动锁定 | 用户点击锁定按钮 |

屏幕锁监听（参考 KeePassXC `ScreenLockListenerWin.cpp`）：
- Windows: `WTS_SESSION_LOCK` + `PBT_APMSUSPEND`
- macOS: `com.apple.screenIsLocked` Darwin 通知
- Linux: logind `PrepareForSleep` + GNOME Session `StatusChanged`

### 7.3 剪贴板 Rust 线程守护

ClipboardGuard 为 Tauri State 管理的运行时守卫，确保密码复制后剪贴板自动恢复：

| 行为 | 说明 |
|------|------|
| `copy_and_guard(password, timeout)` | 保存当前剪贴板内容 → 写入密码 → spawn 守护线程 → sleep timeout → 校验剪贴板仍为密码 → 恢复原内容 |
| 条件恢复 | 仅当剪贴板内容仍为密码时恢复，不覆盖用户新复制的内容 |
| `force_clear()` | 立即恢复原剪贴板内容（不等待 timeout） |
| 退出钩子 | 应用关闭时调用 `force_clear()`，确保剪贴板不留密码 |
| 默认超时 | 30s（由 `vaultClipboardClearSeconds` 设置项控制） |
| 线程安全 | 内部使用 `Mutex<Option<String>>` 保护原剪贴板内容 + `Option<JoinHandle<()>>` 管理守护线程 |

### 7.4 CSP 配置

```
default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' asset: data: blob:; font-src 'self' data:; connect-src 'self' https://ipinfo.io; object-src 'none'; base-uri 'self'; form-action 'self'
```

> **`style-src 'unsafe-inline'` 说明**：Ant Design 组件库运行时注入 `<style>` 标签实现主题定制，`unsafe-inline` 为此必需。`unsafe-inline` 仅影响样式注入，不构成 XSS 向量（XSS 需要 `script-src unsafe-inline`，此处 `script-src` 严格为 `'self'`）。

### 7.5 DevTools 禁用

release 构建时禁用 DevTools（`cfg(not(debug_assertions))` 条件编译），debug 构建保留以便开发调试。

### 7.6 独立解锁窗口

独立 WebviewWindow 用于主密码输入，与主窗口 V8 isolate 隔离：

| 属性 | 值 |
|------|---|
| 窗口标签 | `"vault-unlock"` |
| 尺寸 | 400×200，居中 |
| 可调整大小 | 否 |
| DevTools | 禁用 |
| CSP | 同 §7.4 严格策略 |
| JS 代码量 | <50 行，无第三方依赖 |
| 生命周期 | 已存在时 focus 而非新建；解锁成功后 `window.close()` |
| 与主窗口通信 | 解锁成功后通过 Tauri event `vault:unlocked` 通知主窗口刷新状态 |

### 7.7 验收标准

- [ ] OS 屏幕锁触发 vault 自动锁定
- [ ] 5 分钟无操作触发自动锁定
- [ ] 复制密码 30s 后剪贴板恢复原内容
- [ ] 应用关闭时剪贴板被恢复
- [ ] CSP 生效（`script-src 'self'`，无 `unsafe-eval`）
- [ ] release 构建无法打开 DevTools
- [ ] 独立解锁窗口与主窗口 V8 isolate 隔离

---

## 8. P1-F — 生物识别快速解锁

### 8.1 交付目标

利用 Windows Hello / macOS Touch ID / Linux Polkit 实现快速解锁，**门控密钥释放**而非仅 `authenticate() → bool`。

### 8.2 BiometricKeyStore trait

```rust
pub trait BiometricKeyStore: Send + Sync {
    fn is_available(&self) -> Result<bool>;
    fn set_key(&self, key_id: &str, data: &[u8]) -> Result<()>;   // OS 弹生物识别确认
    fn get_key(&self, key_id: &str) -> Result<Vec<u8>>;           // OS 弹生物识别提示 → 释放密钥
    fn delete_key(&self, key_id: &str) -> Result<()>;
}
```

### 8.3 平台实现

| 平台 | 密钥存储 | 门控方式 | Rust 依赖 |
|------|---------|---------|-----------|
| macOS | Keychain `kSecClassGenericPassword` | `kSecAccessControlBiometryCurrentSet` | `core-foundation-rs` + `security-framework` |
| Windows | TPM `KeyCredentialManager` | `RequestSignAsync(challenge)` → SHA256(sig) | `windows` crate (WinRT) |
| Linux | `libsecret` | Polkit（无门控，降级为系统密码） | `libsecret` |

### 8.4 解锁流程

1. 检测 `biometric_key_enc` 是否存在于 `vault_meta`
2. 存在 → 调用平台 `getKey()`（OS 弹生物识别提示）
3. 通过 → 释放随机密钥 → 解密 `biometric_key_enc` → 恢复 master_key → 解锁
4. 失败/取消 → 回退到主密码输入

### 8.5 Tauri Commands

| Command | 签名 | 说明 |
|---------|------|------|
| `vault_unlock_biometric` | `() → Result<bool>` | 调用平台 getKey() |
| `vault_enroll_biometric` | `(masterPassword) → void` | 启用生物识别 |

### 8.6 验收标准

- [ ] Windows Hello 弹窗 → 通过 → 解锁成功
- [ ] Touch ID 弹窗 → 通过 → 解锁成功
- [ ] 生物识别不可用 → 回退到主密码输入
- [ ] 生物识别解锁后 master_key 不经 IPC

---

## 9. 跨阶段约束

### 9.1 类型安全

- `as unknown as` 仅允许 2 处（`confUtils.ts` + `i18n/index.ts`），vault 代码禁止
- 新增设置项必须同步更新 `AppConf` interface + Rust struct

### 9.2 安全不变式

| 不变式 | 验证方式 |
|--------|---------|
| master_key 不持久化到磁盘 | 代码审查：无 fs::write 含 master_key |
| 密码不进 JS 堆（除 reveal） | 代码审查：vault_copy_password 不返回 String |
| reveal 速率限制 | 单元测试：4 次调用返回 RateLimited |
| MasterKey Drop zeroize | 单元测试：drop 后内存全零 |
| CSP 生效 | 集成测试：eval() 抛错 |

### 9.3 向后兼容

- `assistant.db` 零改动（SQLCipher 不设 key 时等同普通 SQLite）
- `assistant-providers` 的 `encrypt_api_key` / `decrypt_api_key` 迁出后功能不变
- 现有工具不受影响（密码箱为新增工具）
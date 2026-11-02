# I9 — 密码管理箱（Password Vault）设计文档

## 1. 概述

在现有工具箱体系中新增 **密码管理箱** 工具，提供本地加密存储 + 云同步能力。复用项目已有的 AES-256-GCM 加密、SQLite 存储、工具注册体系，新增 Argon2id 主密码派生 + Git 云同步（参考 prs）。

### 1.1 目标

| 目标 | 说明 |
|------|------|
| 安全存储 | 主密码 + Argon2id 派生密钥，AES-256-GCM / ChaCha20-Poly1305 条目级加密，内存 zeroize |
| 云同步 | **Git 同步**（参考 prs 的 prepare→操作→finalize 模式） |
| 渐进增强 | Phase 1 本地密码箱 + TOTP + HIBP + 生物识别 → Phase 2A Git 同步 + SSH Agent + 紧急访问 |
| 工具箱集成 | `defineTool()` 注册，复用密码生成器，归属 `security` 分类 |

### 1.2 参考项目

| 项目 | Stars | 参考点 | 关键文件 |
|------|-------|--------|---------|
| **prs** | — | 🥇 **Git 同步模式** — prepare→操作→finalize 自动同步、Readyness 状态机、智能推送优化、**整体 GPG 加密**（每条目一个 `.gpg` 文件，非逐字段 JSON 加密） | `lib/src/sync.rs`（Sync struct + prepare/finalize）、`lib/src/git.rs`（git CLI 封装）、`lib/src/types.rs`（Plaintext/Ciphertext 安全封装）、`cli/src/action/sync/`（init/commit/pull/push/status） |
| **Vaultwarden** | 69K | Rust 零知识加密 + 条目级同步 API + WebSocket 推送 + 紧急访问 + Send 一次性分享 | `src/crypto.rs`（PBKDF2）、`src/api/core/ciphers.rs`（cipher CRUD）、`src/db/models/cipher.rs`（数据模型）、`src/api/notifications.rs`（WS 实时推送）、`src/api/emergency_access.rs`（紧急访问）、`src/db/models/send.rs`（一次性分享） |
| **KeePassXC** | 29K | 文件级同步 + 合并策略 + TOTP + HIBP 泄露检测 + 生物识别快速解锁 + SSH Agent + AutoType + Passkey | `src/core/Merger.cpp`（合并算法）、`src/core/Totp.cpp`、`src/core/PasswordHealth.cpp`、`src/networking/HibpDownloader.cpp`（k-匿名泄露检测）、`src/quickunlock/`（Windows Hello/TouchID）、`src/sshagent/SSHAgent.cpp`（SSH Agent 集成）、`src/gui/entry/AutoTypeAssociationsModel.cpp` |
| **rust-keepass** | 137 | ~~纯 Rust KDBX4 读写~~（已砍掉，CSV/JSON 导入已覆盖迁移需求） | `src/kpdb/crypter.rs`（AES/ChaCha20 加解密 + SecureString 内存保护）、`src/kpdb/parser.rs` |

### 1.3 同步方案决策

| 维度 | Git 同步（prs 模式）✅ |
|------|----------------------|
| 实现复杂度 | 🥇 极低 — 调用 git CLI，prs 仅 ~670 行 |
| 冲突解决 | 🥇 Git 原生三路合并 + rebase，40 年工业验证 |
| 历史追踪 | 🥇 Git 完整历史，可 `git log` / `git revert` 回滚 |
| 多设备 | 🥇 Git 天然分布式，多分支多设备 |
| 离线支持 | 🥇 Git 离线可操作，上线后自动同步 |
| 依赖 | 需系统安装 `git`（可 Tauri bundle） |
| 国内可用性 | GitHub/GitLab 可能慢，可配自建 Gitea |
| 文件粒度 | 🥇 每条目一个加密文件（整体打包），diff 友好 |
| pass 生态兼容 | 🥇 完全兼容 `pass` 标准 |

**决策**：仅实现 Git 同步（实现量小、冲突解决免费、历史追踪免费）。~~WebDAV/S3 同步~~已砍掉（Git 同步已够用，WebDAV/S3 面向非技术用户，个人项目不需要）。

---

## 2. 架构设计

### 2.1 整体架构

```
┌─────────────────────────────────────────────────────────────────┐
│  前端 (React + Zustand)                                         │
│  src/tools/password-vault/                                      │
│  ├── index.tsx              ← defineTool 入口                   │
│  ├── VaultApp.tsx           ← 主界面路由（解锁/列表/编辑）       │
│  ├── VaultUnlock.tsx        ← 主密码解锁                        │
│  ├── VaultList.tsx          ← 条目列表 + 搜索/分类/标签筛选      │
│  ├── VaultEntryForm.tsx     ← 新增/编辑条目                     │
│  ├── VaultEntryDetail.tsx   ← 条目详情（复制/显示密码）          │
│  ├── VaultSyncSettings.tsx  ← 同步配置（Git）                   │
│  ├── VaultConflictResolve.tsx ← 同步冲突解决 UI（Git 模式）     │
│  ├── vaultStore.ts          ← Zustand 状态（解锁/条目/同步）     │
│  └── vault.service.ts       ← 前端服务层（invoke 封装）         │
├─────────────────────────────────────────────────────────────────┤
│  后端 (Rust)                                                     │
│  assistant-security/          ← 新 crate：安全相关全部集中       │
│  ├── crypto/                 ← AES-256-GCM / ChaCha20-Poly1305  │
│  ├── vault_crypto/           ← Argon2id + HKDF + vault 加解密   │
│  ├── security_db/            ← security.db 初始化/解锁/锁定/备份│
│  ├── biometric/              ← 生物识别门控密钥释放              │
│  │   ├── mod.rs              ← 统一 trait BiometricKeyStore      │
│  │   ├── macos.rs            ← Keychain + kSecAccessControl      │
│  │   ├── windows.rs          ← TPM KeyCredentialManager          │
│  │   └── linux.rs            ← libsecret + Polkit               │
│  ├── audit/                  ← security_audit_log 写入           │
│  ├── password_strength/      ← zxcvbn 主密码强度评估            │
│  └── types/                  ← MasterKey / PerEntryKey 等安全类型│
│  assistant-storage/src/                                         │
│  ├── vault/                  ← vault_entries CRUD                │
│  ├── vault_meta/             ← vault_meta KV（主密码哈希/同步态）│
│  assistant-providers/src/                                       │
│  └── (crypto 已迁出)         ← API Key 加解密改调 assistant-security │
│  assistant-sync/             ← 新 crate：Git 同步引擎            │
│  ├── git_sync/               ← Git 同步（参考 prs）             │
│  ├── sync_engine/            ← 同步引擎（pull/push/sync）       │
│  └── vault_file/             ← vault 文件序列化/反序列化        │
│  src-tauri/src/cmd/                                             │
│  └── vault/                  ← Tauri commands                    │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 密钥层级（参考 Vaultwarden + KeePassXC）

```
用户输入 master_password
  │
  ├─▶ Argon2id(m=65536, t=3, p=4, salt=random_32B)
  │     → master_key (32 bytes)        ← 安全数据库主密钥（原 vault_key，语义扩展）
  │
  ├─▶ Argon2id(m=65536, t=3, p=4, salt=random_32B, different_salt)
  │     → master_key_hash (32 bytes)   ← 存储于 vault_meta，用于验证主密码
  │
  ├─▶ HKDF-SHA256(master_key, info="sqlcipher", 32)
  │     → sqlcipher_key (32 bytes)     ← security.db 的 SQLCipher 页面加密密钥
  │
  └─▶ HKDF-SHA256(master_key, info=entry_id, 32)
        → per_entry_key (32 bytes)     ← 每条目独立密钥，条目删除后密钥不可复用

条目加密:
  plaintext → AES-256-GCM(key=per_entry_key, nonce=random_12B) → ciphertext
  存储: base64(nonce || ciphertext || tag)

条目解密:
  验证 tag → AES-256-GCM 解密 → plaintext → 使用后 zeroize

安全数据库加密:
  security.db 整库使用 SQLCipher 透明加密（PRAGMA key = sqlcipher_key）
  → 磁盘上整个 DB 文件为密文，解锁后 SQL 操作完全正常（LIKE/INDEX/JOIN 不受影响）
  → 锁定时关闭连接 → 文件回退为密文
```

> **设计决策 — 独立安全数据库**：安全数据库（`security.db`）独立于主应用数据库（`assistant.db`），使用 SQLCipher 全库加密。
> - **主库** `assistant.db`：存对话/设置等非敏感数据，不加密，应用启动时打开
> - **安全库** `security.db`：存密码条目/审计日志/SSH 凭据等安全相关数据，SQLCipher 加密，解锁时打开
> - SQLCipher 不设 `PRAGMA key` 时行为等同普通 SQLite，因此主库零改动
> - 未来所有安全相关表（加密笔记、客户端证书、端到端加密密钥等）均存入安全数据库
> - **备份天然加密**：锁定后 `security.db` 文件为密文，直接复制即加密备份

**Plaintext / Ciphertext 类型安全封装（参考 prs `types.rs`）**：

prs 用 `secstr::SecVec<u8>` 封装明文/密文，从类型系统层面防止泄露。`secstr` 已停维，我们采用 `secrecy`（活跃维护，API 等价）：

**VaultPlaintext 安全类型**（`vault_crypto/types.rs`）：

- 内部 `secrecy::Secret<Vec<u8>>`，访问必须通过 `expose_secret()`（`secrecy::ExposeSecret` trait），不实现 `Clone`/`Debug`（防止意外复制/打印）
- `From<Vec<u8>>`：move 所有权到 `Secret::new(other)`，原始 buffer 随即 zeroize + drop
- `#[zeroize(drop)]`：Drop 时自动清零，编译器保证调用
- `Secret` 底层用 `mlock()` 锁定内存页，防止被 swap 到磁盘
- **加密/解密 API 统一使用 `VaultPlaintext`**：`encrypt_field(plaintext: &VaultPlaintext, ...) → String` → `decrypt_field(...) → VaultPlaintext`，从类型系统层面防止明文泄露

**与已有 crypto 的关系**：已有 `encrypt_api_key` / `decrypt_api_key` 使用 `machine_uid` 派生密钥，vault 新增 `vault_crypto` 使用 `Argon2id(master_password)` 派生密钥。两者共享 `aes-gcm` + `base64` + `rand` 依赖，新增 `argon2` + `hkdf` + `zeroize`。

### 2.3 数据模型

#### 数据库架构

```
app_data_dir/
├── assistant.db              ← 主库（非敏感：providers, conversations, messages...）
│                              不加密，WAL 模式，应用启动时打开
├── security/
│   ├── security.db           ← 安全库（SQLCipher 全库加密，WAL 模式，解锁时打开）
│   ├── backups/              ← 自动备份（保留最近 5 份，加密副本）
│   │   ├── security-20261021-143000.db.bak
│   │   └── ...
│   │   触发时机：vault 锁定时 / 主密码变更前 / 应用退出时
│   │   恢复方式：复制 .db.bak → security.db → 用主密码解锁
│   ├── .git/                 ← Git 同步仓库（vault 条目）
│   ├── .vault-meta           ← 密钥元数据
│   └── entries/              ← .enc 加密文件
```

> **Rust 状态管理**：`SecurityDbState` 持有 `conn: Mutex<Option<Connection>>` + `master_key: Mutex<Option<MasterKey>>`。`None` = 锁定，`Some` = 已解锁。`MasterKey` 实现 `Zeroize` + `Drop` 自动清零，不实现 `Clone`/`Debug`。`rusqlite::Connection` 在 `bundled-sqlcipher` feature 下实现 `Send`，满足 Tauri `State<T>: Send + Sync` 约束。锁定时 `conn` 置 None → 关闭连接 → `security.db` 文件回退为 SQLCipher 密文 → 主库 `assistant.db` 不受影响。

#### SQLite 表（均位于 security.db）

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
  is_favorite INTEGER NOT NULL DEFAULT 0,  -- 0=false, 1=true
  revision INTEGER NOT NULL DEFAULT 1,
  deleted_at INTEGER,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS vault_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
-- 存储:
--   master_key_hash    Argon2id 哈希的主密码（用于验证）
--   master_key_salt    master_key 的 Argon2id salt（base64）
--   hash_salt          master_key_hash 的 Argon2id salt（base64）
--   sync_remote_etag   远程文件 etag
--   sync_last_pull_at  上次拉取时间
--   sync_config        同步配置 JSON（backend_type/url/auth 等）
--   biometric_key_enc  生物识别保护的加密 master_key（base64）
--   emergency_token    紧急访问恢复令牌（base64）
--   emergency_contact  紧急访问联系人信息 JSON
--   hibp:{entry_id}    条目级 HIBP 泄露次数（key 前缀 hibp:，value = 数字字符串）

-- 说明:
--   hibp_count 从 vault_entries 移出至 vault_meta，理由：
--   1. HIBP 审计结果是外部查询结果，非条目固有属性
--   2. 每次审计需 UPDATE，放在核心表会频繁触发行级锁
--   3. 用 hibp:{entry_id} 前缀模式，删除条目时同步删除对应 meta 即可

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
```

CREATE INDEX IF NOT EXISTS idx_vault_entries_category
  ON vault_entries(category);
CREATE INDEX IF NOT EXISTS idx_vault_entries_updated
  ON vault_entries(updated_at);
```

#### Rust 数据结构

**VaultEntry**（加密存储，`serde rename_all = "camelCase"`）：

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | `String` | UUID |
| `title` | `String` | 标题（明文，搜索必需） |
| `username` | `Option<String>` | 用户名（明文，搜索必需） |
| `url` | `Option<String>` | 网址（明文，搜索/AutoType 匹配必需） |
| `category` | `VaultCategory` | 枚举：login / card / identity / note / sshKey（`serde rename_all = "lowercase"`） |
| `encrypted_password` | `String` | base64 密文 |
| `encrypted_notes` | `Option<String>` | base64 密文 |
| `encrypted_custom_fields` | `Option<String>` | base64 密文（整体加密的 JSON 数组） |
| `encrypted_totp_secret` | `Option<String>` | base64 密文 |
| `totp_digits` | `u32` | 默认 6 |
| `totp_period` | `u32` | 默认 30 |
| `tags` | `Vec<String>` | JSON 数组字符串 |
| `is_favorite` | `bool` | 收藏标记 |
| `revision` | `u64` | 同步用单调递增版本号 |
| `deleted_at` | `Option<i64>` | 软删除时间戳，None = 未删除 |
| `created_at` | `i64` | 创建时间戳 |
| `updated_at` | `i64` | 更新时间戳 |

**VaultEntryDecrypted**（解密后，仅 Rust 端内部使用，不跨 IPC）：

与 `VaultEntry` 字段一一对应，但 `encrypted_*` 替换为 `VaultPlaintext` 明文类型。`custom_fields` 为 `Vec<CustomField>`，其中 `CustomField { name: String, value: VaultPlaintext, is_secret: bool }`。**注意**：`VaultPlaintext` 不实现 `Clone`，`VaultEntryDecrypted` 也不实现 `Clone`——需要副本时必须逐字段重新构造。

#### 前端类型

```typescript
interface VaultEntrySummary {
  id: string;
  title: string;
  username: string | null;
  url: string | null;
  category: "login" | "card" | "identity" | "note" | "sshKey";
  tags: string[];
  isFavorite: boolean;
  updatedAt: number;
}

interface VaultEntryDetail extends VaultEntrySummary {
  password: string;
  notes: string | null;
  customFields: CustomField[] | null;
  totpSecret: string | null;
  totpDigits: number;
  totpPeriod: number;
  revision: number;
  createdAt: number;
}
```

---

## 3. 加密方案

### 3.1 vault_crypto 核心 API

错误处理风格与项目一致：`Result<T, String>` + `.map_err(|e| format!(...))`。

**CipherAlgorithm** 枚举（`serde rename_all = "camelCase"`）：

| 变体 | discriminant | 说明 |
|------|-------------|------|
| `Aes256Gcm` | 0 | 默认算法 |
| `ChaCha20Poly1305` | 1 | 可选，无侧信道风险 |

显式标注 discriminant（`Aes256Gcm = 0, ChaCha20Poly1305 = 1`），保证密文格式跨版本兼容。

| 方法 | 签名 | 行为 |
|------|------|------|
| `derive_master_key` | `(password: &str, salt: &[u8; 32]) → Result<[u8; 32], String>` | Argon2id(m=65536, t=3, p=4, V0x13) 派生 32B master_key |
| `verify_master_password` | `(password: &str, stored_hash: &[u8], hash_salt: &[u8; 32]) → Result<bool, String>` | 派生后 `subtle::ConstantTimeEq` 常量时间比较，**立即 zeroize 派生结果**（`[u8; 32]` 是 Copy 类型，必须 `mut derived; derived.zeroize()` 而非重新绑定） |
| `derive_entry_key` | `(master_key: &[u8; 32], entry_id: &str) → [u8; 32]` | HKDF-SHA256(salt=None, info=entry_id) |
| `derive_sqlcipher_key` | `(master_key: &[u8; 32]) → [u8; 32]` | HKDF-SHA256(salt=None, info="sqlcipher") |
| `encrypt_field` | `(plaintext: &VaultPlaintext, entry_key: &[u8; 32], algo: CipherAlgorithm) → Result<String, String>` | 12B 随机 nonce → AEAD 加密 → base64(algo_byte ‖ nonce ‖ ciphertext ‖ tag) |
| `decrypt_field` | `(encrypted_b64: &str, entry_key: &[u8; 32]) → Result<VaultPlaintext, String>` | base64 decode → 首字节识别算法 → AEAD 解密 → `VaultPlaintext::from(plaintext)`（Drop 自动 zeroize） |

> **类型安全闭环**：`encrypt_field(&VaultPlaintext, ...) → base64 String` → `decrypt_field(...) → VaultPlaintext`。调用方无法持有裸明文——必须通过 `VaultPlaintext::expose_secret()` 访问，且 Drop 自动 zeroize。

### 3.2 安全要点

| 要点 | 实现方式 |
|------|---------|
| 主密码不存储明文 | 只存 Argon2id 哈希，验证通过后内存中持有 master_key |
| 常量时间比较 | `subtle::ConstantTimeEq` 防时序攻击 |
| 条目级密钥隔离 | HKDF-SHA256(master_key, entry_id) → per_entry_key，条目删除后密钥不可复用 |
| 内存零化 | `zeroize::Zeroize` trait 标记敏感数据，Drop 时自动清零 |
| 自动锁定 | 前端计时器（默认 5 分钟）+ 窗口失焦时清零 master_key + 关闭 security.db 连接 |
| 剪贴板清零 | 复制密码后 30s 自动清空剪贴板 |
| 暴力破解防护 | Argon2id 高参数（m=64MB, t=3, p=4）+ 解锁失败 3s 延迟递增 |
| 主密码强度校验 | 初始化/变更时用 zxcvbn（Rust: `zxcvbn-rs`）评估，拒绝评分低于 "fair" 的主密码（参考 KeePassXC `PasswordHealth.cpp`） |
| 双算法支持 | AES-256-GCM（默认）+ ChaCha20-Poly1305（可选，无侧信道风险），密文首字节标识算法 |

---

## 3.3 TOTP 两步验证（Phase 1，参考 KeePassXC `Totp.cpp`）

每个条目可选关联 TOTP 密钥，生成 6/8 位时间步验证码。

### 3.3.1 vault_totp.rs 核心 API

依赖 `totp-rs` crate。关键接口：

| 方法 | 签名 | 说明 |
|------|------|------|
| `new_from_base32` | `(secret_b32: &str, digits: u32, period: u32) → Result<Totp>` | 从 Base32 密钥创建实例 |
| `generate_current` | `(totp: &Totp) → Result<String>` | 生成当前 6/8 位验证码 |
| `time_remaining` | `(period: u32) → u32` | 剩余有效秒数（UI 倒计时环） |
| `from_otpauth_uri` | `(uri: &str) → Result<(secret, digits, period)>` | 解析 `otpauth://` URI |

### 3.3.2 前端 TOTP 显示

条目详情中 TOTP 区域：圆形倒计时环 + 6 位数字 + 一键复制。倒计时每秒刷新，到期自动重新生成。

---

## 3.4 HIBP 泄露检测（Phase 1，参考 KeePassXC `HibpDownloader.cpp`）

调用 [Have I Been Pwned](https://haveibeenpwned.com/) API 进行 k-匿名查询——只发送密码 SHA-1 哈希的前 5 位，完整哈希永不离开本地。

### 3.4.1 vault_hibp.rs 核心 API

依赖 `sha1` + `reqwest`。关键接口：

| 方法 | 签名 | 说明 |
|------|------|------|
| `check` | `async (password: &str) → Result<u32>` | k-匿名单条检测，返回泄露次数（0=安全） |
| `audit_vault` | `async (conn, master_key) → Result<Vec<(id, count)>>` | 批量检测：逐条解密→检测→立即 zeroize |

k-匿名检测流程：`SHA1(password) → 取前5位hex → GET /range/{prefix} → 本地匹配后缀 → 返回泄露次数`

### 3.4.2 安全要点

| 要点 | 说明 |
|------|------|
| k-匿名 | 只发 SHA-1 前 5 位（~20 bit），HIBP 返回该前缀下所有后缀，本地匹配 |
| 完整哈希不离境 | 后 35 位仅在本地比较，HIBP 服务器无法知道具体查了哪个密码 |
| Add-Padding | 请求头 `Add-Padding: true`，HIBP 返回随机填充行，防流量分析 |
| 逐条解密 | 审计时同一时刻内存中只有一个明文密码，用完立即 zeroize |

---

## 3.5 生物识别快速解锁（Phase 1，参考 KeePassXC `quickunlock/`）

利用 Windows Hello / macOS Touch ID / Linux Polkit 实现快速解锁，无需每次输入主密码。

### 3.5.1 方案：生物识别门控密钥释放（参考 KeePassXC TouchID.mm / WindowsHello.cpp）

> ⚠️ **关键设计约束**：生物识别**不能**只返回 bool。如果 `authenticate() → true` 后用无保护的密钥去解密 `biometric_key_enc`，任何进程都能绕过生物识别直接读取密钥。正确做法是：**OS 密钥管理器中的密钥受生物识别保护，读取时 OS 自动弹出生物识别提示，通过后才释放密钥**。

KeePassXC 的实际实现（`quickunlock/TouchID.mm`）：

```
setKey(dbUuid, passwordKey):
  1. 生成随机 AES-256 密钥 randomKey + IV
  2. 用 randomKey 加密 passwordKey → encryptedMasterKey（存内存）
  3. 将 randomKey+IV 存入 macOS Keychain，设置:
     - kSecAttrAccessControl = kSecAccessControlBiometryCurrentSet  ← Touch ID 保护
     - kSecAttrAccessible = kSecAttrAccessibleWhenUnlockedThisDeviceOnly  ← 仅本设备
  4. 清零 randomKey/IV

getKey(dbUuid, passwordKey):
  1. 调用 SecItemCopyMatching() 读取 Keychain
     → OS 自动弹出 Touch ID 提示
     → 通过后返回 randomKey+IV
     → 用户取消或失败 → 返回错误
  2. 用 randomKey 解密 encryptedMasterKey → passwordKey
  3. 清零 randomKey/IV
```

KeePassXC 的 Windows Hello 实现（`quickunlock/WindowsHello.cpp`）：

```
deriveEncryptionKey(challenge, key):
  1. KeyCredentialManager::OpenAsync("keepassxc_winhello") → 获取 TPM 密钥对
  2. credential.RequestSignAsync(challenge) → Windows Hello 提示
     → 通过后返回签名
     → 用户取消 → 返回错误
  3. key = SHA256(签名结果)  ← 加密密钥由 TPM 签名派生

setKey(dbUuid, data):
  1. challenge = random(12B)  ← 同时用作 IV
  2. deriveEncryptionKey(challenge, key)  ← 弹出 Windows Hello
  3. AES-256-GCM(key, challenge) 加密 data → encrypted
  4. 存储 challenge + encrypted（存内存）

getKey(dbUuid, data):
  1. 读取 challenge + encrypted
  2. deriveEncryptionKey(challenge, key)  ← 再次弹出 Windows Hello
  3. AES-256-GCM 解密 → data
```

**核心区别**：
- macOS：Keychain 条目受 `kSecAccessControlBiometryCurrentSet` 保护，`SecItemCopyMatching()` 读取时 OS 弹 Touch ID
- Windows：TPM 密钥对受 Windows Hello 保护，`RequestSignAsync()` 签名时 OS 弹 Windows Hello，签名结果派生加密密钥
- **两者都不是 `authenticate() → bool`，而是"受保护的操作 → 失败/密钥"**

### 3.5.2 我们的实现方案

| 平台 | 密钥存储 | 生物识别门控方式 | Rust 依赖 |
|------|---------|----------------|-----------|
| macOS | Keychain `kSecClassGenericPassword` | `kSecAccessControlBiometryCurrentSet` + `kSecAttrAccessibleWhenUnlockedThisDeviceOnly` | `core-foundation-rs` + `security-framework` |
| Windows | TPM 密钥对（`KeyCredentialManager`） | `RequestSignAsync(challenge)` → SHA256(signature) = 加密密钥 | `windows` crate (WinRT) |
| Linux | `libsecret`（**无生物识别门控**） | Polkit 对话框（用户密码） | `libsecret` crate |

**流程**：

| 操作 | 流程 |
|------|------|
| 启用 | 输入主密码 → 派生 master_key → 调用平台 `setKey()`（OS 弹生物识别确认）→ 随机密钥加密 master_key → 加密结果存入 `vault_meta.biometric_key_enc` |
| 解锁 | 调用平台 `getKey()`（OS 弹生物识别提示）→ 通过 → 释放随机密钥 → 解密 `biometric_key_enc` → 恢复 master_key |
| 回退 | 生物识别不可用/失败 → 回退到主密码输入 |
| 禁用 | 删除平台密钥存储中的条目 + 删除 `vault_meta.biometric_key_enc` |

> **Linux 特殊情况**：`libsecret` 不支持生物识别门控，Linux 上生物识别快速解锁降级为"系统密码解锁"（Polkit 弹框输入用户密码）。若用户需要生物识别，需自行配置 `pam-fprint-gtk`。

### 3.5.3 解锁流程

```
用户打开密码箱
  │
  ├─▶ 检测 biometric_key_enc 是否存在
  │     ├─ 存在 → 调用平台 getKey()
  │     │         → OS 弹出生物识别提示（Touch ID / Windows Hello / Polkit）
  │     │         ├─ 通过 → OS 释放随机密钥 → 解密 biometric_key_enc → 恢复 master_key → 解锁
  │     │         └─ 失败/取消 → 回退到主密码输入
  │     └─ 不存在 → 直接要求主密码输入
  │
  └─▶ 解锁后：master_key 在内存中，自动锁定时清零 + 关闭 security.db 连接
```

> **注意**：`plugin-biometric` 的 `authenticate()` 仅用于**检测生物识别是否可用**（`isAvailable()`），不用于实际解锁。解锁必须通过平台原生 API 的受保护操作，确保密钥释放受生物识别门控。

---

## 3.6 SSH Agent 集成（Phase 2A，参考 KeePassXC `sshagent/`）

将 SSH 私钥存入密码箱，解锁后自动加载到系统 ssh-agent，锁定时自动移除。

### 3.6.1 核心 API

| 方法 | 说明 |
|------|------|
| `add_to_agent(entry_id, conn, master_key)` | 解密条目中 SSH 私钥 → 加载到系统 ssh-agent（Unix: `SSH_AUTH_SOCK` 协议；Windows: `ssh-add` 命令） |
| `remove_from_agent(entry_id)` | 从 ssh-agent 移除指定密钥 |
| `remove_all_from_agent()` | vault 锁定时移除所有由 vault 加载的密钥 |
| `list_agent_identities()` | 列出当前 ssh-agent 中的密钥 |

### 3.6.2 生命周期

| 事件 | SSH Agent 行为 |
|------|---------------|
| vault 解锁 | 自动加载标记为 `sshAutoAdd: true` 的 SSH Key 条目 |
| vault 锁定 | 自动移除所有由 vault 加载的密钥 |
| 条目删除 | 从 ssh-agent 移除对应密钥 |
| 条目编辑 | 移除旧密钥 → 加载新密钥 |

> ⚠️ **安全警告**（UI 中必须展示）：加载到系统 ssh-agent 后，**本机任何进程均可使用该密钥**（ssh-agent 无访问控制）。建议仅加载需要频繁使用的密钥，长期密钥使用时再手动加载。

---

## 3.7 紧急访问（Phase 2A，参考 Vaultwarden `emergency_access`）

指定信任联系人，在用户失联后可申请获取密码库访问权。本地版简化为**恢复密钥**机制。

### 3.7.1 方案

本地版简化为**恢复密钥**机制，替代 Vaultwarden 的多人紧急访问流程。

| 操作 | 流程 |
|------|------|
| 生成 | 随机 32 字符恢复密钥 → Argon2id 派生加密密钥 → 用该密钥加密 master_key → 存入 `vault_meta.emergency_token` |
| 解锁 | 输入恢复密钥 → Argon2id 派生 → 解密 `emergency_token` → 恢复 master_key |

恢复密钥由用户打印或保存到安全位置，**不存储明文**。使用后可选择重新生成（旧密钥失效）。

> ⚠️ **安全注意**：恢复密钥派生应使用**随机 salt**（存入 vault_meta），而非硬编码 salt，以防止预计算攻击。

### 3.7.2 灾难恢复：仅凭恢复密钥 + Git 仓库还原全部密码

**问题**：`emergency_token`（加密后的 master_key）和恢复密钥的 Argon2id salt 仅存储于本地 SQLite `vault_meta` 表。若软件损坏/丢失、数据库文件不可用，仅凭恢复密钥 + Git 仓库**无法还原密码**——两处断链：

1. 缺少 `emergency.salt` → 无法从恢复密钥派生解密密钥
2. 缺少 `emergency.token` → 无法解密得到 master_key

同理，即使记得主密码，缺少 `master_key_salt` 也无法派生 master_key。

**修复**：将这些**非秘密的密钥元数据**同步写入 Git 仓库的 `.vault-meta` 文件（详见 4.2.1）。Salt 是公开随机数，emergency.token 是加密后的密文——即使攻击者获取整个仓库，没有恢复密钥或主密码仍无法解密任何条目。安全性完全依赖 Argon2id 计算难度。

**修复后的还原流程**：

```
灾难场景：软件丢失/损坏，仅剩 Git 仓库 + 恢复密钥

1. git clone 仓库
2. 读取 .vault-meta → 获取 emergency.salt + emergency.token + crypto.argon2Params
3. Argon2id(recovery_key, emergency.salt, argon2Params) → 解密密钥
4. 解密 emergency.token → master_key
5. 遍历 entries/*.enc：
     - entry_id = 文件名
     - per_entry_key = HKDF-SHA256(master_key, entry_id)
     - 解密 .enc 文件 → 明文
6. ✅ 全部密码还原完成
```

> **安全提示**：恢复密钥 + Git 仓库 = 完整密码库，两者**必须分开存放**。

---

> **已砍掉**：~~§3.8 AutoType 自动填充~~（浏览器扩展填充体验远优于模拟键盘，跨平台键盘模拟坑多，已过时）、~~§3.10 KDBX4 导入导出 + Passkey~~（CSV/JSON 导入已覆盖迁移需求，Passkey 与硬件绑定安全模型矛盾）。

---

## 3.9 Diceware 密码短语生成（Phase 1）

使用 EFF 大词表生成易记忆的密码短语，补充现有密码生成器的纯随机字符模式。EFF 词表来自 [https://www.eff.org/deeplinks/2016/07/new-wordlists-random-passphrases](https://www.eff.org/deeplinks/2016/07/new-wordlists-random-passphrases)，含 7776 个常见英文单词。

### 3.9.1 方案：扩展现有前端密码生成器

现有 `password-generator.service.ts` 已实现 `generatePassword()`（随机字符）+ `getCrackTimeEstimation()`（熵值估算），**直接扩展即可**，无需 Rust 后端参与：

新增 `generatePassphrase(wordCount=4, separator="-", capitalize=false) → string`，从 EFF 大词表（7776 词，`eff-large-wordlist.ts` ~15KB gzip）中随机取词拼接。

**不选 Rust 后端的理由**：现有生成器是纯前端（`crypto.getRandomValues`），Diceware 也是纯计算，无必须走 IPC 的理由；未来如需中文/日文词表，可按 locale 懒加载。

---

## 4. 云同步设计

### 4.1 方案概览

| 方案 | 优先级 | 实现量 | 冲突解决 | 历史追踪 |
|------|--------|--------|---------|---------|
| **4.2 Git 同步** | ✅ 唯一方案（Phase 2A） | ~670 行（参考 prs） | Git 原生三路合并 | ✅ 完整 Git 历史 |

---

### 4.2 Git 同步（首选，参考 prs）

prs 的同步方案极其简洁优雅——**直接用 Git 作为同步后端**，每个密码条目导出为一个整体加密文件（prs 用 GPG，本设计用 AES-256-GCM），整个密码库就是一个 Git 仓库。Git 提供免费的冲突解决（三路合并）、完整历史追踪、离线支持和多设备同步。

#### 4.2.1 Git 仓库结构

```
~/.assistant/vault/                ← Git 仓库
├── .git/                          ← Git 元数据
├── .vault-meta                    ← vault 元信息（含密钥元数据，见下方）
├── entries/
│   ├── a1b2c3d4.enc              ← 单条目加密文件（文件名 = entry_id）
│   ├── e5f6g7h8.enc
│   └── ...
```

> 早期设计的 `deletions/deleted.json` 墓碑索引已彻底移除（不写入、不读取）。删除状态由 `vault_entries.deleted_at`（本地）与 Git 自身的删除记录（远端）承载。

#### .vault-meta 文件

`.vault-meta` 不仅存储版本号，还包含**非秘密的密钥元数据**（salt 是公开随机数，加密后的 master_key 是密文），确保灾难恢复时仅凭 Git 仓库 + 恢复密钥/主密码即可还原全部密码：

```json
{
  "version": 1,
  "exportedAt": 1729555200,
  "crypto": {
    "masterKeySalt": "base64(32字节 Argon2id salt，用于派生 master_key)",
    "hashSalt": "base64(32字节 Argon2id salt，用于派生 master_key_hash)",
    "argon2Params": { "m": 65536, "t": 3, "p": 4 },
    "defaultCipher": "aes256Gcm"
  },
  "emergency": {
    "token": "base64(加密后的 master_key)",
    "salt": "base64(恢复密钥的 Argon2id salt)"
  },
  "integrity": "base64(HMAC-SHA256(master_key, version+crypto+emergency))"
}
```

> **安全性**：即使攻击者获取整个 Git 仓库（含 `.vault-meta`），没有主密码或恢复密钥仍无法派生 master_key。Salt 是公开随机数，Argon2id 安全性不依赖 salt 保密；emergency.token 是加密后的密文，解密需要恢复密钥。

> **时间戳单位（契约，勿改）**：`exportedAt` 一律为 **unix 秒**（10 位，如 `1729555200`），与本项目内部（DB 时间列、IPC 返回值、前端 `Date.now()`）统一使用的 **unix 毫秒**不同。`.vault-meta` 是跨设备 / 跨版本的对外文件格式，单位变更会让新旧设备写出相差 1000 倍的同名字段且不产生任何报错，属于最难排查的隐性不兼容。若要变更，必须升 `version` 并在读取时按版本归一化。

> **完整性保护**：`integrity` 字段使用 HMAC-SHA256(master_key, 其余字段内容) 签名。Git pull 后验证签名——若 `.vault-meta` 被篡改（如修改 masterKeySalt 导致拒绝服务），签名验证失败，拒绝应用变更。签名密钥为 master_key，攻击者无法伪造。

> **不含 biometric.keyEnc**：生物识别的 `keyEnc` 由 OS 密钥管理器（Windows Credential Manager / macOS Keychain）中的 device_key 加密，该 key 绑定在特定设备上。灾难恢复场景（换机器/重装系统）中 device_key 不存在，`keyEnc` 无法解密，存入仓库无意义。`keyEnc` 仅存于本地 SQLite `vault_meta` 表，供同设备生物识别解锁使用。

> **绝不存入仓库的字段**：
>
> | vault_meta key | 排除理由 |
> |---|---|
> | `master_key_hash` | 🔴 **绝对禁止** — 主密码的 Argon2id 哈希，若与 `hashSalt` 同时泄露，攻击者可离线暴力破解主密码 |
> | `sync_config` | 🔴 含 Git 认证凭据，推入仓库即泄露 |
> | `sync_remote_etag` / `sync_last_pull_at` | 设备绑定的同步状态，跨设备无意义 |
> | `biometric_key_enc` | 设备绑定，灾难恢复不可用（见上） |
> | `hibp:{entry_id}` | 外部审计缓存，可重新查询 |
> | `emergency_contact` | ⚠️ 可选 — 紧急联系人信息非解密必需，且可能泄露社交关系；默认不存，用户可显式开启 |

> **双写一致性**：`vault_meta` 表变更时同步更新 `.vault-meta` 文件，与条目双写逻辑一致（步骤 2-5 在同一 DB 事务中）。

#### .enc 文件格式

每个 `.enc` 文件采用**整体打包加密**（与 prs 整体 GPG 加密的模式一致），将所有敏感字段序列化为一个 JSON blob 后一次加密：

```json
{
  "id": "a1b2c3d4-...",
  "category": "login",
  "revision": 3,
  "encryptedBlob": "base64(algo_byte‖nonce‖AES-256-GCM(per_entry_key, blob_json)‖tag)"
}
```

`encryptedBlob` 解密后的 `blob_json` 内容：

```json
{
  "title": "GitHub",
  "username": "user@example.com",
  "url": "https://github.com",
  "password": "s3cret!",
  "notes": "Personal account",
  "customFields": [{"name": "API Key", "value": "xxx", "isSecret": true}],
  "totpSecret": "JBSWY3DPEHPK3PXP",
  "totpDigits": 6,
  "totpPeriod": 30,
  "tags": ["dev", "work"],
  "isFavorite": true,
  "createdAt": 1729468800,
  "updatedAt": 1729555200
}
```

> **仓库整体打包 vs SQLite 逐字段——两层各取最优策略**：
>
> | 层 | 策略 | 理由 |
> |---|---|---|
> | **SQLite** | 逐字段加密 | 部分解密（HIBP 只解密 password、TOTP 只解密 totpSecret）、SQL LIKE 搜索（title/username/url 明文）、内存最小暴露 |
> | **Git .enc** | 整体打包加密 | 与 prs 模式一致、nonce/tag 开销仅 28B（vs 逐字段 196B）、灾难恢复一次解密即得全部数据、prepare/finalize 已避免冲突 |
>
> prs 实际采用整体 GPG 加密每个 `.gpg` 文件（文件内第一行密码，后续行 `key: value` 属性），并非逐字段 JSON 加密。prs 的冲突避免依赖 `prepare → 操作 → finalize` 串行化模式，而非 JSON 字段级合并。本设计采用同样策略（prepare/finalize 模式），因此整体打包在仓库层面是可行且更优的选择。
>
> 仅 `id`/`category`/`revision` 为明文。即使仓库推送到公开 GitHub，攻击者也无法从 Git 历史中获知用户在哪些网站有账号。`category` 保留明文是因为其值域极小（5 种），信息熵可忽略。
>
> **.enc 明文字段的元数据泄露权衡**：
>
> | 字段 | 当前状态 | 泄露风险 | 建议 |
> |------|---------|---------|------|
> | `id` | 明文 | 无 — UUID 无语义 | 保留明文 |
> | `category` | 明文 | 极低 — 5 种值，熵≈0 | 保留明文 |
> | `revision` | 明文 | 无 — 单调递增整数 | 保留明文（同步必需） |
> | `isFavorite` | **移入 blob** | 中 — 暴露用户偏好模式（哪些账号最常用） | 🔒 加密 |
> | `createdAt` | **移入 blob** | 中 — 暴露账号创建时间模式 | 🔒 加密 |
> | `updatedAt` | **移入 blob** | 中 — 暴露密码更新频率（安全习惯侧面信息） | 🔒 加密 |
>
> `isFavorite`/`createdAt`/`updatedAt` 移入 `encryptedBlob` 后，.enc 文件仅剩 `id` + `category` + `revision` + `encryptedBlob` 四个字段，元数据泄露面最小化。代价是 Git log 中无法直接看到条目的更新时间和收藏状态，但此信息在解密后仍可获取。

#### 4.2.2 prepare → 操作 → finalize 模式（prs 核心设计）

prs 最精妙的设计是**将同步自动嵌入每次密码操作**，而非手动触发：

```rust
// 每次操作前
sync.prepare()?;   // → 如果有 remote → git pull（拉取远程变更到最新）

// ... 执行密码操作（create/update/delete）...
// ... 操作会修改 entries/ 下的 .enc 文件 ...

// 每次操作后
sync.finalize("added: GitHub")?;  // → git add . + git commit + git push
```

这确保了**每次操作都在最新状态上执行，操作后立即同步**，极大减少冲突概率。

#### 4.2.3 DB 与文件双写一致性

数据库写入和 .enc 文件导出必须保证一致性：

```
操作流程（以 create 为例）：
  1. sync.prepare()?;                          // git pull
  2. DB 事务开始
  3. vault::create_entry(conn, entry, key)?;   // 写入 vault_entries
  4. vault_file::export_entry(...)?;           // 导出 .enc 文件
  5. DB 事务提交
  6. sync.finalize("added: ...")?;             // git commit + push
```

- 步骤 2-5 在同一个 DB 事务中：如果 .enc 导出失败，DB 写入也回滚
- 步骤 6 在事务外：finalize 失败不影响本地数据，下次操作时会重试

#### 4.2.4 软删除与 Git 同步的交互

`vault_entries.deleted_at` 软删除由「本地 DB 墓碑 + Git 自身的删除记录」承载，不再有 `deletions/deleted.json`：

| 事件 | DB 行为 | Git 行为 |
|------|---------|---------|
| 删除条目 | `UPDATE vault_entries SET deleted_at = now() WHERE id = ?` | 下次导出时删除对应 `.enc`，`git add -A` 记录删除 |
| 拉取时发现远程删除 | **不自动改库**；仅在同步结果中上报 `removedIds`，由用户决定是否删除 | 该 `.enc` 已被 Git 移除 |
| 拉取时远程有但本地已删除 | 跳过导入（本地删除优先） | `.enc` 仍存在，下次 push 时删除 |
| 恢复条目 | `UPDATE vault_entries SET deleted_at = NULL` | 下次导出重建 `.enc` |
| 永久清理（用户手动） | `DELETE FROM vault_entries WHERE deleted_at IS NOT NULL` | 对应 `.enc` 已不存在，无额外操作 |

> 软删除条目在前端列表中默认隐藏，可通过"回收站"查看和恢复。

#### 4.2.5 GitSync 核心 API

`assistant-sync/src/git_sync.rs`，错误处理风格与项目一致。

**Readyness 就绪状态**（参考 prs）：

| 变体 | 说明 |
|------|------|
| `NoSync` | 未初始化 git |
| `RepoState(RepositoryState)` | Git 异常状态（Merge / Rebase / RebaseInteractive / CherryPick / Unknown） |
| `Dirty` | 有未提交变更 |
| `Ready` | 就绪，可操作 |

**核心方法**：

| 方法 | 行为 |
|------|------|
| `init()` | 若已初始化则跳过；`git init` + 空提交 |
| `clone(url)` | `git clone url repo_path`，检查退出码 |
| `prepare()` | 若已初始化且有 remote → `git pull -q` |
| `finalize(msg)` | 若 dirty → `git add .` + `git commit -q -m msg`；若有 remote 且 `need_to_push()` → `git push -q` |
| `readyness()` | 返回当前 Readyness 状态 |
| `need_to_push()` | 智能推送优化：30s 内 pull 过且本地/远程 hash 相同 → 跳过 push。elapsed 不可得时保守返回 true（推送） |

> **关键**：所有 `git` 子命令调用统一通过 `git(args) → Result<Output>` 辅助方法，检查 `output.status.success()`，失败时返回 `format!("git {} failed: {}", args, stderr)`。**禁止**直接 `.status()?` 忽略退出码。

#### 4.2.6 Vault 与 GitSync 集成

每次密码操作自动同步（以 create 为例）：

1. `sync.prepare()?` — 拉取远程变更
2. DB 事务内：写入 `vault_entries` + 导出 `.enc` 文件（步骤 2-5 原子性，任一失败则回滚）
3. `sync.finalize("vault: added {title}")?` — 提交并推送

#### vault_file 导出/导入

**导出**（SQLite 逐字段 → .enc 整体打包）：

1. `derive_entry_key(master_key, entry.id)` 派生条目级密钥
2. 将 `VaultEntryDecrypted` 各字段通过 `expose_secret()` 读取，构造 `VaultBlob` 明文结构体
3. `serde_json::to_vec(&blob)` 序列化 → `VaultPlaintext::from(json_bytes)` → `encrypt_field(&plaintext, &entry_key, Aes256Gcm)` → base64 密文
4. 写入 `{id}.enc`：`{ id, category, revision, encrypted_blob }`

**导入**（.enc 整体打包 → SQLite 逐字段）：

1. 读取 `.enc` 文件 → 反序列化 `EncFile`
2. `derive_entry_key(master_key, enc_file.id)` → `decrypt_field(&enc_file.encrypted_blob, &entry_key)` → `VaultPlaintext`
3. `serde_json::from_slice(plaintext.expose_secret())` → `VaultBlob`
4. 逐字段写入 SQLite（各自独立加密）

#### 4.2.7 Git 冲突处理

Git 同步的冲突由 Git 本身处理。由于 .enc 文件采用整体打包加密（`encryptedBlob` 为单字段），同一条目的任何修改都会导致 blob 变化：

| 场景 | Git 行为 | 对用户的影响 |
|------|---------|-------------|
| 不同条目修改 | 自动合并（不同文件） | 无感知 |
| 同一条目，任一字段修改 | CONFLICT（blob 整体变化） | 需在 VaultConflictResolve 中选择 |
| 远程删除 + 本地修改 | CONFLICT | 需选择保留或删除 |

> **与逐字段 JSON 加密的区别**：逐字段加密时，同一条目不同字段的修改可由 Git 三路合并自动解决（不同 JSON key）。整体打包后此能力丧失，但 `prepare → finalize` 串行化模式已将同条目并发修改概率降至极低（与 prs 一致）。prs 同样采用整体加密，实际使用中冲突极少发生。

对于 Git 无法自动合并的冲突，前端检测 `Readyness::RepoState(Merge)` 状态，弹出冲突解决 UI（解密本地/远程两个版本，显示 diff，让用户选择）。

#### 4.2.8 支持的 Git 远程

| 服务 | URL 格式 | 特点 |
|------|---------|------|
| **GitHub** | `https://github.com/user/vault.git` | 最流行，免费私有仓库 |
| **GitLab** | `https://gitlab.com/user/vault.git` | 免费私有仓库，可自建 |
| **Gitea / Forgejo** | `https://gitea.example.com/user/vault.git` | 轻量自建，国内友好 |
| **Gogs** | `https://gogs.example.com/user/vault.git` | 极轻量自建 |
| **Coding** | `https://e.coding.net/user/vault.git` | 国内服务，速度快 |

> SSH 协议同样支持：`git@gitea.example.com:user/vault.git`，可复用用户已有 SSH key。

#### 4.2.9 SSH 连接复用（参考 prs `util/git.rs`）

每次 `prepare → pull` + `finalize → push` 涉及两次 SSH 握手（~4s）。prs 通过 SSH ControlMaster 持久连接将后续操作降至 ~0.1s：

- 设置 `GIT_SSH_COMMAND` 环境变量：`ssh -o 'ControlMaster auto' -o 'ControlPath /tmp/.prs-session--%r@%h:%p' -o 'ControlPersist 1h' -o 'ConnectTimeout 10'`
- 白名单机制：只对 `github.com` / `gitlab.com` 启用，避免不兼容服务出错
- 进程级缓存：`std::sync::LazyLock<Mutex<HashMap<PathBuf, bool>>>`，同一进程只判断一次
- Windows 上跳过（`cfg!(unix)` guard），仅 Unix 生效

---

> **已砍掉**：~~§4.3 WebDAV/S3 同步~~（Git 同步已够用，WebDAV/S3 面向非技术用户，个人项目不需要）、~~SyncBackend trait / WebDAV 实现 / 合并算法 / WebDAV 兼容云服务~~ 一并移除。

---

## 5. Tauri Commands

所有 command 在 `src-tauri/src/cmd/vault.rs` 中定义，统一通过 `State<SecurityDbState>` 获取安全数据库连接，需要解密的操作通过 `assistant_security::vault_crypto::get_master_key()` 获取内存中的 master_key。

| 分类 | Command | 签名 | 说明 |
|------|---------|------|------|
| 生命周期 | `vault_init` | `(master_password) → ()` | 初始化密码箱（Argon2id 哈希 + salt） |
| | `vault_unlock` | `(master_password) → bool` | 验证主密码并解锁 |
| | `vault_is_locked` | `() → bool` | 查询锁定状态 |
| | `vault_lock` | `() → ()` | 锁定（清零 master_key + 关闭 security.db 连接） |
| CRUD | `vault_list_entries` | `(category?, search?, tag?) → Vec<Summary>` | 列表（不解密，返回元数据） |
| | `vault_copy_password` | `(id) → ()` | Rust 端解密 + 写剪贴板，密码不进 JS 堆 |
| | `vault_copy_field` | `(id, fieldName) → ()` | Rust 端解密 + 写剪贴板（customField） |
| | `vault_reveal_password` | `(id) → { value, expiresAt }` | 返回密码明文（速率限制 3/分钟 + 审计 + UI 通知） |
| | `vault_reveal_notes` | `(id) → { value, expiresAt }` | 返回笔记明文（同上） |
| | `vault_create_entry` | `(entry) → id` | 新增 |
| | `vault_update_entry` | `(id, entry) → ()` | 更新 |
| | `vault_delete_entry` | `(id) → ()` | 软删除 |
| | `vault_change_master_password` | `(current, new) → ()` | 变更主密码（触发全量重加密） |
| 导出 | `vault_export` | `(format: "csv" \| "json") → Path` | 导出全部条目（解密后写入文件，用完 zeroize） |
| 导入 | `vault_import` | `(path, format) → count` | 从 CSV/JSON 导入条目 |
| 同步 | `vault_sync_push/pull/now` | `() → SyncResult` | 推/拉/双向同步（追加提交，不覆盖远程） |
| | `vault_sync_force_push` | `() → SyncResult` | 强制推送（需用户二次确认） |
| | `vault_get/set_sync_config` | `() → SyncConfig` | 同步配置读写 |
| | `vault_resolve_conflict` | `(entry_id, resolution) → ()` | 冲突解决 |
| Git | `vault_git_init/clone` | `(url?) → ()` | 初始化/克隆 |
| | `vault_git_add/set_remote` | `(name, url) → ()` | 远程仓库管理 |
| | `vault_git_readyness` | `() → Readyness` | 就绪状态 |
| | `vault_git_pull/push` | `() → ()` | 手动拉/推 |
| TOTP | `vault_totp_generate` | `(id) → String` | 生成当前验证码 |
| | `vault_totp_time_remaining` | `(period) → u32` | 倒计时 |
| HIBP | `vault_hibp_check` | `(id) → u32` | 单条泄露检测 |
| | `vault_hibp_audit` | `() → Vec<(id, count)>` | 全库审计 |
| 生物识别 | `vault_unlock_biometric` | `() → Result<(), String>` | 调用平台 getKey()（OS 弹生物识别）→ 成功则恢复 master_key 并打开 security.db，失败返回 Err |
| | `vault_enroll_biometric` | `(master_password) → ()` | 启用生物识别 |
| SSH | `vault_ssh_add/remove_from_agent` | `(id) → ()` | SSH Agent 管理 |
| 紧急访问 | `vault_generate_recovery_key` | `() → String` | 生成恢复密钥 |
| | `vault_unlock_with_recovery_key` | `(recovery_key) → bool` | 恢复密钥解锁 |

#### 主密码变更原子流程（参考 KeePassXC Database::save）

`vault_change_master_password` 必须保证原子性，防止中途崩溃导致数据库永久损坏：

```
1. 验证当前主密码
2. WAL checkpoint(FULL) → 确保 security.db 无未提交 WAL
3. 备份 security.db → backups/security-{timestamp}.db.bak
4. 派生新 master_key + 新 sqlcipher_key
5. PRAGMA rekey = 新 sqlcipher_key
   → SQLCipher 原子重加密所有数据库页面
   → 失败则整个 rekey 回滚，数据库保持旧密钥
6. 逐条重加密 .enc 文件（entry_key = HKDF(新 master_key, entry_id)）
   → 每个文件独立：失败跳过并记录，不阻塞其他文件
   → 成功后 git add + commit
7. 更新 vault_meta（master_key_hash, master_key_salt, hash_salt）
8. 更新 .vault-meta（masterKeySalt, emergency.token）
9. 更新 biometric_key_enc（若已启用）
10. 验证：用新主密码解锁 → 读取一条 → 确认可解密
11. 失败回滚：从 backup 恢复 security.db + git reset
```

> **关键**：SQLCipher `PRAGMA rekey` 是**页面级原子操作**——要么所有页面用新密钥重加密成功，要么保持旧密钥不变。这比 prs 的逐文件 recrypt 安全得多（prs 单文件失败不影响其他文件，但也没有整体原子性）。.enc 文件是逐文件操作，单个失败不影响数据库一致性。

### invoke_handler 注册

在 `src-tauri/src/lib.rs` 的 `invoke_handler` 宏中新增上述所有 `cmd::vault_*` 条目。

---

## 6. 前端设计

### 6.1 工具注册

```typescript
// src/tools/registry.ts 新增
const passwordVault = defineTool({
  id: "password-vault",
  name: "tools.password_vault.name",
  description: "tools.password_vault.description",
  icon: SafetyCertificateOutlined,
  keywords: ["vault", "password", "manager", "safe", "credential", "locker", "keepass"],
  component: lazyWithRecovery(() => import("./password-vault")),
  widthMode: "wide",
  createdAt: new Date("2026-10-20"),
});
```

### 6.2 vaultStore（Zustand）

状态字段：`isLocked` / `isInitialized` / `entries` / `searchQuery` / `categoryFilter` / `tagFilter` / `syncStatus`（idle|syncing|conflict|error）/ `conflicts` / `hibpResults` / `biometricAvailable`

核心 action：`unlock` / `unlockBiometric` / `lock` / `init` / `loadEntries` / `createEntry` / `updateEntry` / `deleteEntry` / `getEntryDetail` / `syncNow` / `resolveConflict` / `generateTotp` / `hibpCheckEntry` / `hibpAuditAll` / `sshAddToAgent` / `sshRemoveFromAgent` / `generateRecoveryKey`

### 6.3 UI 组件

#### VaultUnlock — 主密码解锁

```
┌─────────────────────────────────────┐
│         🔒 密码管理箱               │
│                                     │
│    ┌───────────────────────────┐    │
│    │  请输入主密码              │    │
│    └───────────────────────────┘    │
│                                     │
│    [ 🖐 生物识别解锁 ]  (可选)      │
│         [ 解  锁 ]                  │
│                                     │
│    首次使用？[创建密码箱]            │
│    忘记密码？[使用恢复密钥]          │
└─────────────────────────────────────┘
```

#### VaultList — 条目列表

```
┌──────────────────────────────────────────────────────┐
│  🔍 搜索...    [全部▾] [标签▾]    [+ 新增] [↻ 同步]  │
├──────────────────────────────────────────────────────┤
│  ⭐ GitHub            user@github.com     login       │
│  ⭐ AWS Console       admin@aws.amazon.com  login    │
│    Bank of China      ****6789            card       │
│    SSH Key - Prod     —                  sshKey  🔑  │
│    WiFi Password      —                  note        │
│    🔴 Leaked Site     pwned@x.com        login  ⚠   │
│    ...                                               │
├──────────────────────────────────────────────────────┤
│  42 条目 · 上次同步 5 分钟前 · ⚠ 1 泄露              │
└──────────────────────────────────────────────────────┘
```

- 🔑 = SSH Key 已加载到 agent
- ⚠ = HIBP 检测到泄露（hibp_count > 0）

#### VaultEntryForm — 新增/编辑

```
┌──────────────────────────────────────────────────────┐
│  新增条目                                    [保存]   │
├──────────────────────────────────────────────────────┤
│  分类:  [Login ▾]                                    │
│  标题:  [________________]                            │
│  用户名: [________________]                           │
│  密码:  [****************] [👁] [🔄 生成]            │
│    生成模式: [随机字符 ▾] / [密码短语 ▾]              │
│    密码短语: 4词 · 分隔符[- ▾] · ☐ 首字母大写        │
│  网址:  [________________]                            │
│  TOTP:  [________________] [📷 扫码] [⏱ 预览]       │
│  标签:  [dev] [work] [+ 添加]                        │
│  备注:  [________________]                            │
│  自定义字段:                                         │
│    [字段名] [值] [🔒密] [×]                          │
│    [+ 添加字段]                                      │
└──────────────────────────────────────────────────────┘
```

> 密码生成按钮 `[🔄 生成]` 支持两种模式：
> - **随机字符**：调用已有 `password-generator.service.ts` 的 `generatePassword()`
> - **密码短语**：调用同文件的 `generatePassphrase()`，生成 Diceware 短语如 `correct-horse-battery-staple`
>
> TOTP 字段：支持手动输入 Base32 密钥、扫描二维码（`otpauth://` URI 解析）、实时预览 6 位验证码

#### VaultConflictResolve — 冲突解决

```
┌──────────────────────────────────────────────────────────┐
│  ⚠️ 同步冲突                                            │
│                                                          │
│  ┌────────────┐    ┌────────────┐    ┌────────────┐     │
│  │  本地版本   │    │  远程版本   │    │   建议     │     │
│  │ GitHub     │    │ GitHub     │    │ GitHub     │     │
│  │ user:old   │    │ user:new   │    │ user:new   │     │
│  │ pwd:•••    │    │ pwd:•••    │    │ pwd:•••    │     │
│  └────────────┘    └────────────┘    └────────────┘     │
│                                                          │
│  [使用本地]  [使用远程]  [合并保留两者]  [跳过]           │
└──────────────────────────────────────────────────────────┘
```

### 6.4 自动锁定

多层锁定触发（优先级从高到低）：

1. **OS 屏幕锁/睡眠/用户切换**（`vaultLockOnScreenLock: true`）：Rust 端监听系统事件 → 立即 `vault_lock()`
   - Windows: `WTS_SESSION_LOCK` + `PBT_APMSUSPEND` + `GUID_LIDSWITCH_STATE_CHANGE`（参考 KeePassXC `ScreenLockListenerWin.cpp`）
   - macOS: `com.apple.screenIsLocked` Darwin 通知（参考 `ScreenLockListenerMac.cpp`）
   - Linux: logind `PrepareForSleep` + GNOME Session `StatusChanged`（参考 `ScreenLockListenerDBus.cpp`）
2. **前端计时器**：`setTimeout`（默认 5 分钟）+ `visibilitychange` 窗口失焦 → 调用 `vaultStore.lock()`
3. **手动锁定**：用户点击锁定按钮

锁定动作：清零 `master_key`（MasterKey Drop 自动 zeroize）+ 关闭 `security.db` 连接（文件回退为 SQLCipher 密文）→ 主库 `assistant.db` 不受影响

### 6.5 剪贴板安全（参考 prs `clipboard.rs` + `clip_revert.rs`）

复制密码后 30 秒自动**恢复剪贴板原内容**（而非简单清空，参考 prs）。超时前校验当前剪贴板内容是否仍为密码，防止覆盖用户后续复制的新内容。

**Rust 线程守护**（替代前端 setTimeout，参考 prs `TempClip` 子进程模式）：

`ClipboardGuard`（`assistant-security/src/clipboard_guard.rs`）：

- `thread: Mutex<Option<JoinHandle<()>>>` — 守护线程句柄（Mutex 包裹，满足 `&self` 方法内赋值）
- `original: Mutex<Option<String>>` — 剪贴板原始内容
- `copy_and_guard(&self, password: &str, timeout: Duration) → Result<(), String>`：读取当前剪贴板 → 写入密码 → 启动守护线程（sleep timeout → 校验剪贴板仍为密码 → 恢复原始内容）
- `force_clear(&self)`：退出钩子，立即恢复原始内容

| 场景 | 守护线程 | 退出钩子（`force_clear`） | prs 子进程 |
|------|:-------:|:------------------------:|:---------:|
| 正常超时 | ✅ | - | ✅ |
| 用户关闭应用 | - | ✅ | ✅ |
| 应用 panic | ✅（线程存活） | - | ✅ |
| `kill -9` | ❌ | ❌ | ✅ |

> `kill -9` 场景下攻击者已有系统级权限（可直接读 `/proc/pid/mem`），剪贴板安全已无意义。守护线程 + 退出钩子覆盖 99.99% 场景，无需 prs 的子进程方案（需额外 helper binary）。

### 6.6 搜索策略

- **普通搜索**：SQLCipher 解密页面后，SQLite `LIKE` 查询 title/username/url/tags，O(1) 索引加速
- **正则搜索**（`/pattern/` 语法）：解密后匹配加密字段（notes/customFields），O(n)，仅在用户显式使用正则时触发
- **SQLCipher 优势**：全库加密后搜索仍可用（页面级透明加解密），无需像逐字段加密方案那样牺牲搜索性能

### 6.7 IPC 安全与前端防线

> **核心问题**：Tauri 架构下 JS 堆与 Rust 之间有 IPC 通道（`invoke()`），敏感数据可能经过 JS 堆。XSS 可读取 JS 堆、截获 invoke 参数、调用任意 command。所有加密设计建立在"攻击者无法进入 JS 堆"的前提下，必须纵深防御保证此前提。

#### 6.7.1 三层安全模型

| 层 | 目标 | DAG：每层独立有效 |
|---|------|------------------|
| **防入侵** | 让 XSS 尽难发生 | CSP + DevTools 禁用 + 独立解锁窗口 + 依赖审计 |
| **减暴露** | 即使 XSS 发生，可窃取数据最少 | 生物识别优先 + 复制/TOTP 走 Rust + reveal 速率限制 |
| **限时间** | 即使数据泄露，窗口最短 | 超时清零 + 屏幕锁联动 + 崩溃安全剪贴板 + catch_unwind |

#### 6.7.2 防入侵层

**CSP 配置**：`tauri.conf.json` 中 `security.csp` 设为严格策略，关键约束：

| 指令 | 约束 | 理由 |
|------|------|------|
| `script-src` | `'self'`（禁止 `unsafe-eval` / `unsafe-inline`） | **最关键防线**：Vite 构建后所有 JS 来自 bundle，内联 `<script>` 无法执行 |
| `style-src` | `'self' 'unsafe-inline'` | Ant Design v6 动态主题运行时注入 `<style>`，仅允许内联 style，不影响 JS 执行 |
| `connect-src` | `'self'` + 项目所需外部域名 | Tauri IPC（`'self'` 覆盖 invoke）+ 外部 API（如 ipinfo.io、HIBP 等，按项目实际配置） |
| `object-src` | `'none'` | 禁止 `<object>`/`<embed>` 插件攻击面 |

> ⚠️ CSP 具体值随项目依赖变化，不在此硬编码示例。以 `tauri.conf.json` 实际配置为准。

**DevTools 禁用**（release 构建）：`src-tauri/src/window.rs` 中 `#[cfg(not(debug_assertions))]` 条件下 `builder.devtools(false)`。

**独立解锁窗口**（隔离 V8 isolate）：

通过 `WebviewWindowBuilder::new(app, "vault-unlock", ...)` 创建，配置 `inner_size(400, 200)` / `center(true)` / `resizable(false)` / `decorations(true)` / `devtools(false)`（永远禁用）。

解锁窗口是独立 WebView 上下文（Tauri 2 每个窗口独立 V8 isolate），有自己的严格 CSP，JS 代码极简（<50 行），无第三方依赖。主窗口的 XSS **无法读取解锁窗口的 JS 堆**。

解锁页面（`vault-unlock.html`）：`<input type="password">` + 解锁按钮，点击后读取密码 → 立即清空 DOM → `invoke('vault_unlock', { masterPassword })` → 成功则 `window.close()`。

#### 6.7.3 减暴露层

**生物识别优先**（master_key 不经 IPC）：

| 解锁方式 | master_key 是否经 IPC | 安全等级 |
|---------|:-------------------:|---------|
| 生物识别 | ❌ OS 原生门控释放密钥 | 🟢 最优 |
| 独立解锁窗口 | ✅ 但在隔离 V8 isolate | 🟡 次优 |
| 主窗口内输入 | ✅ 与主 UI 共享 V8 | 🟠 回退 |

**最小暴露 API**（替代原 `vault_get_entry` 返回全部明文）：

| Command | 返回值 | 密码进 JS | 用途 |
|---------|--------|:---------:|------|
| `vault_list_entries(query)` | `VaultEntrySummary[]` | ❌ | 列表/搜索 |
| `vault_copy_password(id)` | `void` | ❌ | Rust 端解密 + 写剪贴板 |
| `vault_totp_generate(id)` | `string`（6位） | ❌ | Rust 端生成，totpSecret 不出 Rust |
| `vault_copy_field(id, name)` | `void` | ❌ | 复制 customField |
| `vault_reveal_password(id)` | `{ value, expiresAt }` | ✅ | **唯一**返回密码明文的命令 |
| `vault_reveal_notes(id)` | `{ value, expiresAt }` | ✅ | 显示笔记 |

**`vault_reveal_*` 防批量泄露**：

| 层 | 机制 | 效果 |
|---|------|------|
| 速率限制 | 3 次/分钟，超限返回 `RateLimited` | 500 条目需 2.8 小时批量获取 |
| 审计日志 | 写入 `security_audit_log`（加密） | 事后可检测异常 |
| UI 通知 | 每次 reveal 显示 toast | 用户可感知异常 reveal |
| 单条限制 | 同一时间只有一个 reveal 有效 | 防并行批量请求 |

`RevealGuard`（`assistant-security/src/reveal_guard.rs`）：

- `last_reveal: Mutex<Instant>` — 上次 reveal 时间
- `reveal_count: AtomicU32` — 计数器，`Ordering::SeqCst`（速率限制需跨线程一致）
- `vault_reveal_password(id, guard) → Result<RevealResult, String>`：
  1. 若距上次 reveal < 20s：`reveal_count.fetch_add(1, SeqCst)`，若 `> 3` → 审计日志 + 返回 `Err("Rate limited")`
  2. 否则：`reveal_count.store(1, SeqCst)` 重置计数
  3. 更新 `last_reveal` → 审计日志 → 解密密码 → 返回 `{ value: VaultPlaintext, expires_at: now + 30s }`

#### 6.7.4 限时间层

- **reveal 超时清零**：`vault_reveal_password` 返回 `{ value, expiresAt }`，前端 30s 后清零（仅缩短窗口，V8 GC 不保证覆写）
- **屏幕锁联动**：见 §6.4，最高优先级触发锁定
- **崩溃安全剪贴板**：见 §6.5，Rust 线程守护 + 退出钩子
- **catch_unwind**：`SecurityDbState::lock()` 用 `catch_unwind` 包裹，确保 panic 时 master_key 仍被 zeroize

#### 6.7.5 与参考项目对比

| 防线 | KeePassXC (Qt C++) | Bitwarden (Electron) | 我们 (Tauri) |
|------|:------------------:|:--------------------:|:------------:|
| 防入侵 | 不需要（无 JS 堆） | CSP + contextIsolation | CSP + DevTools 禁用 + 独立解锁窗口 |
| 减暴露 | 不需要（同进程） | ❌ 密码返回 renderer | ✅ 复制走 Rust + reveal 速率限制 |
| 限时间 | QSecureZeroMemory | setTimeout（不可靠） | Rust 线程守护 + 退出钩子 + 屏幕锁 |

Tauri 天然优势：无 `nodeIntegration`、无 `remote` 模块、IPC 需显式注册 command、WebView2 比 Chromium 攻击面小。

---

## 7. 设置项扩展

按项目规则，新增设置项必须同步更新接口：

### 7.1 AppConf 新增字段

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `vaultAutoLockMinutes` | `number` | 5 | 自动锁定时间 |
| `vaultLockOnScreenLock` | `boolean` | true | OS 屏幕锁/睡眠/用户切换时自动锁定（参考 KeePassXC ScreenLockListener） |
| `vaultClipboardClearSeconds` | `number` | 30 | 剪贴板清零时间 |
| `vaultCipherAlgorithm` | `"aes256Gcm" \| "chacha20Poly1305"` | `aes256Gcm` | 加密算法 |
| `vaultHibpAutoAudit` | `boolean` | true | 自动 HIBP 审计 |
| `vaultSshAutoAdd` | `boolean` | true | 解锁后自动加载 SSH 密钥 |

Rust `AppConf` struct 同步添加对应字段，均标注 `#[serde(default = ...)]`。

---

## 8. Rust 依赖新增

### 8.1 新 crate: assistant-security

**职责**：所有安全相关逻辑集中——加密/解密、密钥派生、安全数据库管理、生物识别、安全审计。从 `assistant-providers` 中迁出 `crypto.rs`，新增 vault 专用模块。

> **设计原则**：安全逻辑不应散落在业务 crate 中。`assistant-providers` 负责 AI 提供商通信，不应包含加密实现；`assistant-storage` 负责数据持久化，不应包含加密逻辑。`assistant-security` 是唯一知道"如何加密"的 crate，其他 crate 只调用其公开 API。

```
assistant-security/
├── src/
│   ├── lib.rs
│   ├── crypto/               ← AES-256-GCM / ChaCha20-Poly1305 加解密（从 providers 迁出）
│   ├── vault_crypto/         ← Argon2id 主密码派生 + HKDF 条目级密钥 + master_key 管理
│   ├── security_db/          ← security.db 初始化/解锁/锁定/备份（SQLCipher）
│   ├── biometric/            ← 生物识别门控密钥释放
│   │   ├── mod.rs            ← BiometricKeyStore trait
│   │   ├── macos.rs          ← Keychain + kSecAccessControlBiometryCurrentSet
│   │   ├── windows.rs        ← TPM KeyCredentialManager + RequestSignAsync
│   │   └── linux.rs          ← libsecret + Polkit
│   ├── audit/                ← security_audit_log 写入
│   ├── password_strength/    ← zxcvbn 主密码强度评估
│   └── types/                ← MasterKey / PerEntryKey / VaultPlaintext 等安全类型
├── Cargo.toml
└── tests/
```

| 依赖 | 版本 | 用途 |
|------|------|------|
| `aes-gcm` | 0.11 | AES-256-GCM 加密 |
| `chacha20poly1305` | 0.11 | ChaCha20-Poly1305 加密 |
| `argon2` | 0.6 | 主密码派生 |
| `hkdf` | 0.13 | 条目级密钥 / sqlcipher_key 派生 |
| `zeroize` | 1.9 (derive) | 敏感内存清零 |
| `secrecy` | 0.10 | 明文安全封装 (Zeroize+Debug 遮蔽) |
| `subtle` | 2.6 | 常量时间比较 |
| `zxcvbn` | 3 | 主密码强度评估 |
| `totp-rs` | 6 | TOTP 生成 |
| `sha1` | 0.11 | HIBP SHA-1 哈希 |
| `hex` | 0.4 | 十六进制编解码 |
| `rusqlite` | bundled-sqlcipher | security.db SQLCipher 加密 |
| `base64` | 0.22 | 编解码 |
| `rand` | 0.9 | 随机 nonce/salt 生成 |
| `serde` / `serde_json` | * | 类型序列化 |
| `chrono` | 0.4 | 时间戳 |
| `uuid` | * | 条目 ID |

平台特定依赖（条件编译）：
- `[target.'cfg(target_os = "macos")'.dependencies]`: `core-foundation-rs`, `security-framework`
- `[target.'cfg(target_os = "windows")'.dependencies]`: `windows` (WinRT KeyCredentialManager)
- `[target.'cfg(target_os = "linux")'.dependencies]`: `libsecret`

### 8.2 assistant-providers 变更

- **迁出** `crypto.rs` → `assistant-security::crypto`
- API Key 加解密改调 `assistant_security::crypto::encrypt_api_key()` / `decrypt_api_key()`
- 新增依赖 `assistant-security`

### 8.3 assistant-storage 变更

`rusqlite` feature 从 `bundled` 改为 `bundled-sqlcipher`（编译时内嵌 SQLCipher C 源码 + OpenSSL，~2-3MB 体积增加）。

> **兼容性**：SQLCipher 不设 `PRAGMA key` 时行为等同普通 SQLite。主库 `assistant.db` 打开时不设 key → 零改动；安全库 `security.db` 打开时设 key → 透明加密。

新增依赖 `assistant-security`（安全数据库操作需要调用 `security_db` 模块）。

### 8.4 新 crate: assistant-sync

workspace members 新增 `"crates/assistant-sync"`。依赖 `serde` / `serde_json` / `tokio` / `base64` / `assistant-storage` / `assistant-security`。

仅 Git 同步模式，无 WebDAV/S3 features。

### 8.3 安全相关 Crate 维护状态审计（2026-10-21）

| crate | 仓库 | archived | 最后 push | ⭐ | 状态 |
|-------|------|----------|-----------|-----|------|
| `argon2` | RustCrypto/password-hashes | ❌ | 2026-10-18 | 915 | ✅ 活跃 |
| `hkdf` | RustCrypto/KDFs | ❌ | 2026-09-21 | 110 | ✅ 活跃 |
| `aes-gcm` | RustCrypto/AEADs | ❌ | 2026-10-13 | 968 | ✅ 活跃 |
| `chacha20poly1305` | RustCrypto/AEADs | ❌ | 2026-10-13 | 968 | ✅ 活跃 |
| `sha1` | RustCrypto/hashes | ❌ | 2026-10-20 | 2276 | ✅ 活跃 |
| `zeroize` | RustCrypto/utils | ❌ | 2026-10-19 | 684 | ✅ 活跃 |
| `secrecy` | iqlusioninc/crates | ❌ | 2026-10-17 | 581 | ✅ 活跃 |
| `totp-rs` | constantoine/totp-rs | ❌ | 2026-10-16 | 277 | ✅ 活跃 |
| `subtle` | dalek-cryptography/subtle | ❌ | 2024-08-03 | 311 | ⚠️ 低活跃（2年+未push，但issue讨论活跃，v3.0规划中；功能极稳定，RustCrypto全生态间接依赖） |
| ~~`secstr`~~ | valpackett/secstr (Codeberg) | ❌ | 2024-12-03 | 3 | 🔴 事实停维（3星，2 open issue无响应，Unlicense许可）→ **已替换为 `secrecy`** |

---

## 9. i18n

`zh-CN.json` 的 `tools.password_vault` 命名空间下新增约 50 个词条，覆盖：解锁/创建/锁定、条目 CRUD、分类名（登录/银行卡/身份/安全笔记/SSH密钥）、同步状态、Git 操作、冲突解决、密码生成/Diceware、TOTP、HIBP、生物识别、SSH Agent、紧急访问、加密算法名。

---

## 10. 安全审计清单

| # | 检查项 | 状态 |
|---|--------|------|
| 1 | 主密码不以明文存储 | ✅ Argon2id 哈希 |
| 2 | master_key 不持久化到磁盘 | ✅ 仅内存持有（MasterKey: zeroize + Drop 自动清零） |
| 3 | 常量时间比较防时序攻击 | ✅ subtle::ConstantTimeEq |
| 4 | 条目级密钥隔离（HKDF） | ✅ per-entry key |
| 5 | 敏感内存保护 | ✅ secrecy::Secret (Zeroize+Debug 遮蔽) + zeroize (Drop 清零) + expose_secret() 访问控制 |
| 6 | AES-256-GCM / ChaCha20-Poly1305 双算法认证加密 | ✅ AEAD 内置 tag，密文首字节标识算法 |
| 7 | 随机 nonce 不重复 | ✅ 每次 12B random |
| 8 | Argon2id 高参数防暴力 | ✅ m=64MB, t=3, p=4 |
| 9 | 解锁失败延迟递增 | ✅ 3s 基础 × 失败次数 |
| 10 | 剪贴板安全 | ✅ 超时后恢复原内容（非清空），防覆盖后续复制 |
| 11 | 云端只存密文 | ✅ Git: .enc 整体打包加密（encryptedBlob） |
| 12 | 同步冲突不静默丢数据 | ✅ Git: 原生 CONFLICT 标记 |
| 13 | Git 仓库不泄露明文 | ✅ .enc 仅含 encryptedBlob + id/category/revision（非敏感），.vault-meta 仅含 salt/密文，.gitignore 排除 security.db |
| 14 | Git remote 传输加密 | ✅ HTTPS/SSH 协议层加密 |
| 15 | Git 凭证安全 | ✅ 使用系统 credential helper / SSH key，不存储密码 |
| 16 | TOTP 密钥加密存储 | ✅ encrypted_totp_secret 条目级加密，与密码同级保护 |
| 17 | HIBP k-匿名查询 | ✅ 只发 SHA-1 前 5 位，完整哈希不离境，Add-Padding 防流量分析 |
| 18 | HIBP 审计逐条解密 | ✅ 同一时刻内存中只有一个明文密码，用完立即 zeroize |
| 19 | 生物识别门控密钥释放 | ✅ macOS: Keychain kSecAccessControlBiometryCurrentSet；Windows: TPM RequestSignAsync → SHA256(sig) 派生加密密钥；Linux: libsecret（无门控，降级为 Polkit）；非 authenticate()→bool 模式 |
| 20 | SSH Agent 生命周期管理 | ✅ vault 锁定时自动移除所有已加载密钥，防止锁定后密钥残留 |
| 21 | 恢复密钥不存明文 | ✅ 仅存加密后的 master_key，恢复密钥由用户安全保管 |
| 22 | 灾难恢复完整性 | ✅ `.vault-meta` 含 salt + emergency.token，恢复密钥 + Git 仓库可独立还原 |
| 23 | 安全数据库独立加密 | ✅ security.db 使用 SQLCipher 全库加密（PRAGMA key = sqlcipher_key），独立于 assistant.db，锁定时关闭连接回退为密文 |
| 24 | .vault-meta 完整性保护 | ✅ HMAC-SHA256(master_key, content) 签名，Git pull 后验证，篡改拒绝应用 |
| 25 | 安全数据库本地备份 | ✅ 每次锁定时自动备份至 security/backups/，保留最近 5 份，备份文件为加密副本 |
| 26 | 安全审计日志加密存储 | ✅ security_audit_log 存入 security.db（SQLCipher 加密），泄露用户安全习惯的日志不暴露于明文 |
| 27 | 主密码强度校验 | ✅ 初始化/变更时 zxcvbn 评估，拒绝弱主密码（参考 KeePassXC PasswordHealth） |
| 28 | 主密码变更原子性 | ✅ 备份 → PRAGMA rekey（页面级原子重加密）→ 逐条重加密 .enc → 验证 → 失败回滚 |
| 29 | OS 屏幕锁联动 | ✅ vaultLockOnScreenLock 设置，Rust 端监听 WTS_SESSION_LOCK / Darwin notification / DBus（参考 KeePassXC ScreenLockListener） |
| 30 | SSH Agent 安全警告 | ✅ UI 展示警告：加载后本机任何进程可使用密钥 |
| 31 | 数据导出/导入 | ✅ vault_export（CSV/JSON）+ vault_import，解密后写入用完 zeroize |
| 32 | CSP 严格配置 | ✅ script-src 'self'（无 unsafe-eval/unsafe-inline），style-src 'unsafe-inline' 仅限 Ant Design 动态主题，object-src 'none' |
| 33 | DevTools 生产禁用 | ✅ `#[cfg(not(debug_assertions))] builder.devtools(false)`，release 构建无法打开 DevTools |
| 34 | 独立解锁窗口 | ✅ 隔离 V8 isolate，极简 JS（<50行），无第三方依赖，严格 CSP，主窗口 XSS 无法读取解锁窗口堆 |
| 35 | 生物识别优先解锁 | ✅ master_key 不经 IPC（OS 原生门控释放密钥），主密码输入为回退 |
| 36 | 最小暴露 API | ✅ vault_copy_password/vault_totp_generate 密码不进 JS 堆；vault_reveal_password 速率限制 3/分钟 + 审计 + UI 通知 |
| 37 | 剪贴板 Rust 线程守护 | ✅ ClipboardGuard 守护线程 + force_clear 退出钩子，应用崩溃/关闭时恢复剪贴板 |
| 38 | panic 时 master_key zeroize | ✅ SecurityDbState::lock() 用 catch_unwind 包裹，确保 panic unwind 前先 zeroize |
| 39 | 密码历史追踪 | ✅ vault_entries 增加 encrypted_password_history，更新时自动追加旧密码+时间戳（参考 Vaultwarden cipher.password_history） |
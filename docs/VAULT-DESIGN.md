# I9 — 密码管理箱（Password Vault）设计文档

## 1. 概述

在现有工具箱体系中新增 **密码管理箱** 工具，提供本地加密存储 + 云同步能力。复用项目已有的 AES-256-GCM 加密、SQLite 存储、工具注册体系，新增 Argon2id 主密码派生 + Git 云同步（参考 prs）。

### 1.1 目标

| 目标 | 说明 |
|------|------|
| 安全存储 | 主密码 + Argon2id 派生密钥，AES-256-GCM / ChaCha20-Poly1305 条目级加密，内存 zeroize |
| 云同步 | **Git 同步优先**（参考 prs 的 prepare→操作→finalize 模式），WebDAV/S3 备选 |
| 渐进增强 | Phase 1 本地密码箱 + TOTP + HIBP + 生物识别 → Phase 2A Git 同步 + SSH Agent + 紧急访问 → Phase 2B WebDAV/S3 + AutoType → Phase 3 KDBX4 + Passkey |
| 工具箱集成 | `defineTool()` 注册，复用密码生成器，归属 `security` 分类 |

### 1.2 参考项目

| 项目 | Stars | 参考点 | 关键文件 |
|------|-------|--------|---------|
| **prs** | — | 🥇 **Git 同步模式** — prepare→操作→finalize 自动同步、Readyness 状态机、智能推送优化 | `lib/src/sync.rs`（Sync struct + prepare/finalize）、`lib/src/git.rs`（git CLI 封装）、`cli/src/action/sync/`（init/commit/pull/push/status） |
| **Vaultwarden** | 69K | Rust 零知识加密 + 条目级同步 API + WebSocket 推送 + 紧急访问 + Send 一次性分享 | `src/crypto.rs`（PBKDF2）、`src/api/core/ciphers.rs`（cipher CRUD）、`src/db/models/cipher.rs`（数据模型）、`src/api/notifications.rs`（WS 实时推送）、`src/api/emergency_access.rs`（紧急访问）、`src/db/models/send.rs`（一次性分享） |
| **KeePassXC** | 29K | 文件级同步 + 合并策略 + TOTP + HIBP 泄露检测 + 生物识别快速解锁 + SSH Agent + AutoType + Passkey | `src/core/Merger.cpp`（合并算法）、`src/core/Totp.cpp`、`src/core/PasswordHealth.cpp`、`src/networking/HibpDownloader.cpp`（k-匿名泄露检测）、`src/quickunlock/`（Windows Hello/TouchID）、`src/sshagent/SSHAgent.cpp`（SSH Agent 集成）、`src/gui/entry/AutoTypeAssociationsModel.cpp` |
| **rust-keepass** | 137 | 纯 Rust KDBX4 读写 | `src/kpdb/crypter.rs`（AES/ChaCha20 加解密 + SecureString 内存保护）、`src/kpdb/parser.rs` |

### 1.3 同步方案决策

| 维度 | Git 同步（prs 模式）✅ 首选 | WebDAV/S3 同步 备选 |
|------|---------------------------|---------------------|
| 实现复杂度 | 🥇 极低 — 调用 git CLI，prs 仅 ~670 行 | 中 — WebDAV PROPFIND/PUT/GET + etag 冲突 ~800+ 行 |
| 冲突解决 | 🥇 Git 原生三路合并 + rebase，40 年工业验证 | 自定义合并算法（KeePassXC Merger） |
| 历史追踪 | 🥇 Git 完整历史，可 `git log` / `git revert` 回滚 | 无历史，只有当前状态 |
| 多设备 | 🥇 Git 天然分布式，多分支多设备 | etag 冲突检测，需手动解决 |
| 离线支持 | 🥇 Git 离线可操作，上线后自动同步 | 需本地缓存 + 合并逻辑 |
| 依赖 | 需系统安装 `git`（可 Tauri bundle） | 仅需 `reqwest`（项目已有） |
| 国内可用性 | GitHub/GitLab 可能慢，可配自建 Gitea | 坚果云 WebDAV 国内最快 |
| 文件粒度 | 🥇 每条目一个加密文件，diff 友好 | 整个 vault.enc 一个文件 |
| pass 生态兼容 | 🥇 完全兼容 `pass` 标准 | 不兼容 |

**决策**：Phase 2A 先实现 Git 同步（实现量小、冲突解决免费、历史追踪免费），Phase 2B 可选 WebDAV/S3（面向国内坚果云用户）。

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
│  ├── VaultSyncSettings.tsx  ← 同步配置（Git/WebDAV/S3）         │
│  ├── VaultConflictResolve.tsx ← 同步冲突解决 UI（WebDAV 模式）  │
│  ├── vaultStore.ts          ← Zustand 状态（解锁/条目/同步）     │
│  └── vault.service.ts       ← 前端服务层（invoke 封装）         │
├─────────────────────────────────────────────────────────────────┤
│  后端 (Rust)                                                     │
│  assistant-storage/src/                                         │
│  ├── vault.rs               ← vault_entries CRUD                │
│  ├── vault_meta.rs          ← vault_meta KV（主密码哈希/同步态） │
│  assistant-providers/src/                                       │
│  ├── crypto.rs              ← 已有 AES-256-GCM（复用）          │
│  ├── vault_crypto.rs        ← Argon2id + HKDF + vault 加解密    │
│  assistant-sync/            ← 新 crate：云同步引擎               │
│  ├── src/lib.rs                                             │
│  ├── src/git_sync.rs       ← Git 同步（优先，参考 prs）        │
│  ├── src/backend.rs        ← SyncBackend trait（WebDAV/S3 备选）│
│  ├── src/webdav.rs         ← WebDAV 实现                       │
│  ├── src/s3.rs             ← S3/R2 实现                        │
│  ├── src/merge.rs          ← 合并算法（WebDAV 模式，参考 Merger）│
│  ├── src/sync_engine.rs    ← 同步引擎（pull/push/sync）        │
│  ├── src/vault_file.rs     ← vault 文件序列化/反序列化         │
│  src-tauri/src/cmd/                                             │
│  └── vault.rs               ← Tauri commands                    │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 密钥层级（参考 Vaultwarden + KeePassXC）

```
用户输入 master_password
  │
  ├─▶ Argon2id(m=65536, t=3, p=4, salt=random_32B)
  │     → vault_key (32 bytes)        ← 对称加密主密钥
  │
  ├─▶ Argon2id(m=65536, t=3, p=4, salt=random_32B, different_salt)
  │     → master_key_hash (32 bytes)  ← 存储于 vault_meta，用于验证主密码
  │
  └─▶ HKDF-SHA256(vault_key, info=entry_id, 32)
        → per_entry_key (32 bytes)    ← 每条目独立密钥，条目删除后密钥不可复用

条目加密:
  plaintext → AES-256-GCM(key=per_entry_key, nonce=random_12B) → ciphertext
  存储: base64(nonce || ciphertext || tag)

条目解密:
  验证 tag → AES-256-GCM 解密 → plaintext → 使用后 zeroize
```

**Plaintext / Ciphertext 类型安全封装（参考 prs `types.rs`）**：

prs 用 `secstr::SecVec<u8>` 封装明文/密文，从类型系统层面防止泄露。我们采用同样模式：

```rust
use secstr::SecVec;
use zeroize::Zeroize;

/// 明文安全封装 — 内存保护，访问必须通过 unsecure_ref()
#[derive(Zeroize)]
#[zeroize(drop)]
pub struct VaultPlaintext(SecVec<u8>);

impl From<Vec<u8>> for VaultPlaintext {
    fn from(mut other: Vec<u8>) -> VaultPlaintext {
        let into = VaultPlaintext(other.to_vec().into());
        other.zeroize();                  // 原始 buffer 立即清零
        into
    }
}

impl VaultPlaintext {
    pub fn unsecure_ref(&self) -> &[u8] { self.0.unsecure() }
    pub fn is_empty(&self) -> bool { self.unsecure_ref().is_empty() }
    pub fn from_str(s: &str) -> Self { s.as_bytes().to_vec().into() }
}
```

- `unsecure_ref()` 命名自带警告，开发者不会无意中泄露
- `From` 转换时**自动 zeroize 原始 buffer**，消除"忘记清零"的隐患
- `SecVec` 底层用 `mlock()` 锁定内存页，防止被 swap 到磁盘
- `#[zeroize(drop)]` 确保 Drop 时自动清零
- **加密/解密 API 统一使用 `VaultPlaintext`**：`encrypt_field(plaintext: &VaultPlaintext, ...)` → `decrypt_field(...) → VaultPlaintext`，从类型系统层面防止明文泄露

**与已有 crypto.rs 的关系**：已有 `encrypt_api_key` / `decrypt_api_key` 使用 `machine_uid` 派生密钥，vault 新增 `vault_crypto.rs` 使用 `Argon2id(master_password)` 派生密钥。两者共享 `aes-gcm` + `base64` + `rand` 依赖，新增 `argon2` + `hkdf` + `zeroize`。

### 2.3 数据模型

#### SQLite 表

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
-- 存储:
--   master_key_hash    Argon2id 哈希的主密码（用于验证）
--   vault_key_salt     vault_key 的 Argon2id salt（base64）
--   hash_salt          master_key_hash 的 Argon2id salt（base64）
--   sync_remote_etag   远程文件 etag
--   sync_last_pull_at  上次拉取时间
--   sync_config        同步配置 JSON（backend_type/url/auth 等）
--   biometric_key_enc  生物识别保护的加密 vault_key（base64）
--   emergency_token    紧急访问恢复令牌（base64）
--   emergency_contact  紧急访问联系人信息 JSON
--   hibp:{entry_id}    条目级 HIBP 泄露次数（key 前缀 hibp:，value = 数字字符串）

-- 说明:
--   hibp_count 从 vault_entries 移出至 vault_meta，理由：
--   1. HIBP 审计结果是外部查询结果，非条目固有属性
--   2. 每次审计需 UPDATE，放在核心表会频繁触发行级锁
--   3. 用 hibp:{entry_id} 前缀模式，删除条目时同步删除对应 meta 即可

CREATE INDEX IF NOT EXISTS idx_vault_entries_category
  ON vault_entries(category);
CREATE INDEX IF NOT EXISTS idx_vault_entries_updated
  ON vault_entries(updated_at);
```

#### Rust 数据结构

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultEntry {
    pub id: String,
    pub title: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub category: VaultCategory,
    pub encrypted_password: String,
    pub encrypted_notes: Option<String>,
    pub encrypted_custom_fields: Option<String>,
    pub encrypted_totp_secret: Option<String>,
    pub totp_digits: u32,
    pub totp_period: u32,
    pub tags: Vec<String>,
    pub is_favorite: bool,
    pub revision: u64,
    pub deleted_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VaultCategory {
    Login,
    Card,
    Identity,
    Note,
    SshKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultEntryDecrypted {
    pub id: String,
    pub title: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub category: VaultCategory,
    pub password: VaultPlaintext,
    pub notes: Option<VaultPlaintext>,
    pub custom_fields: Option<Vec<CustomField>>,
    pub totp_secret: Option<VaultPlaintext>,
    pub totp_digits: Option<u32>,
    pub totp_period: Option<u32>,
    pub hibp_count: Option<i32>,
    pub tags: Vec<String>,
    pub is_favorite: bool,
    pub revision: u64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomField {
    pub name: String,
    pub value: VaultPlaintext,
    pub is_secret: bool,
}
```

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

### 3.1 vault_crypto.rs 核心 API

```rust
use argon2::{Argon2, Algorithm, Version, Params};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use chacha20poly1305::{ChaCha20Poly1305, Key as ChaChaKey};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroize;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CipherAlgorithm {
    Aes256Gcm,
    ChaCha20Poly1305,
}

impl Default for CipherAlgorithm {
    fn default() -> Self { CipherAlgorithm::Aes256Gcm }
}

pub struct VaultCrypto;

impl VaultCrypto {
    /// Argon2id 派生 vault_key
    pub fn derive_vault_key(password: &str, salt: &[u8; 32]) -> Result<[u8; 32], String> {
        let params = Params::new(65536, 3, 4, Some(32))
            .map_err(|e| format!("Argon2 params error: {}", e))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut key = [0u8; 32];
        argon2.hash_password_into(password.as_bytes(), salt, &mut key)
            .map_err(|e| format!("Argon2 derivation failed: {}", e))?;
        Ok(key)
    }

    /// 验证主密码
    pub fn verify_master_password(
        password: &str,
        stored_hash: &[u8],
        hash_salt: &[u8; 32],
    ) -> Result<bool, String> {
        let derived = Self::derive_vault_key(password, hash_salt)?;
        let valid = subtle::ConstantTimeEq::ct_eq(&derived, stored_hash).into();
        // zeroize derived immediately
        let mut d = derived;
        d.zeroize();
        Ok(valid)
    }

    /// HKDF 派生条目级密钥
    pub fn derive_entry_key(vault_key: &[u8; 32], entry_id: &str) -> [u8; 32] {
        let hkdf = Hkdf::<Sha256>::new(None, vault_key);
        let mut key = [0u8; 32];
        hkdf.expand(entry_id.as_bytes(), &mut key).expect("HKDF expand failed");
        key
    }

    /// 加密条目字段（支持 AES-256-GCM / ChaCha20-Poly1305）
    pub fn encrypt_field(plaintext: &VaultPlaintext, entry_key: &[u8; 32], algo: CipherAlgorithm) -> Result<String, String> {
        let nonce_bytes: [u8; 12] = rand::thread_rng().gen();
        let ciphertext = match algo {
            CipherAlgorithm::Aes256Gcm => {
                let cipher = Aes256Gcm::new_from_slice(entry_key)
                    .map_err(|e| format!("Invalid key: {}", e))?;
                let nonce = Nonce::from_slice(&nonce_bytes);
                cipher.encrypt(nonce, plaintext.unsecure_ref())
                    .map_err(|e| format!("Encryption failed: {}", e))?
            }
            CipherAlgorithm::ChaCha20Poly1305 => {
                let cipher = ChaCha20Poly1305::new_from_slice(entry_key)
                    .map_err(|e| format!("Invalid key: {}", e))?;
                let nonce = chacha20poly1305::Nonce::from_slice(&nonce_bytes);
                cipher.encrypt(nonce, plaintext.unsecure_ref())
                    .map_err(|e| format!("Encryption failed: {}", e))?
            }
        };
        let mut result = Vec::with_capacity(1 + 12 + ciphertext.len());
        result.push(algo as u8);
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        Ok(base64::engine::general_purpose::STANDARD.encode(&result))
    }

    /// 解密条目字段（自动识别算法）→ 返回 VaultPlaintext
    pub fn decrypt_field(encrypted_b64: &str, entry_key: &[u8; 32]) -> Result<VaultPlaintext, String> {
        let data = base64::engine::general_purpose::STANDARD
            .decode(encrypted_b64)
            .map_err(|e| format!("Base64 decode failed: {}", e))?;
        if data.len() < 13 { return Err("Data too short".into()); }
        let algo = match data[0] {
            0 => CipherAlgorithm::Aes256Gcm,
            1 => CipherAlgorithm::ChaCha20Poly1305,
            _ => return Err("Unknown cipher algorithm".into()),
        };
        let nonce_bytes = &data[1..13];
        let plaintext = match algo {
            CipherAlgorithm::Aes256Gcm => {
                let cipher = Aes256Gcm::new_from_slice(entry_key)
                    .map_err(|e| format!("Invalid key: {}", e))?;
                let nonce = Nonce::from_slice(nonce_bytes);
                cipher.decrypt(nonce, &data[13..])
                    .map_err(|e| format!("Decryption failed: {}", e))?
            }
            CipherAlgorithm::ChaCha20Poly1305 => {
                let cipher = ChaCha20Poly1305::new_from_slice(entry_key)
                    .map_err(|e| format!("Invalid key: {}", e))?;
                let nonce = chacha20poly1305::Nonce::from_slice(nonce_bytes);
                cipher.decrypt(nonce, &data[13..])
                    .map_err(|e| format!("Decryption failed: {}", e))?
            }
        };
        Ok(VaultPlaintext::from(plaintext))
    }
}
```

> **类型安全闭环**：`encrypt_field(&VaultPlaintext, ...) → base64 String` → `decrypt_field(...) → VaultPlaintext`。
> 调用方无法持有裸 `String` 明文——必须通过 `VaultPlaintext::unsecure_ref()` 访问，且 Drop 自动 zeroize。

### 3.2 安全要点

| 要点 | 实现方式 |
|------|---------|
| 主密码不存储明文 | 只存 Argon2id 哈希，验证通过后内存中持有 vault_key |
| 常量时间比较 | `subtle::ConstantTimeEq` 防时序攻击 |
| 条目级密钥隔离 | HKDF-SHA256(vault_key, entry_id) → per_entry_key，条目删除后密钥不可复用 |
| 内存零化 | `zeroize::Zeroize` trait 标记敏感数据，Drop 时自动清零 |
| 自动锁定 | 前端计时器（默认 5 分钟）+ 窗口失焦时清零 vault_key |
| 剪贴板清零 | 复制密码后 30s 自动清空剪贴板 |
| 暴力破解防护 | Argon2id 高参数（m=64MB, t=3, p=4）+ 解锁失败 3s 延迟递增 |
| 双算法支持 | AES-256-GCM（默认）+ ChaCha20-Poly1305（可选，无侧信道风险），密文首字节标识算法 |

---

## 3.3 TOTP 两步验证（Phase 1，参考 KeePassXC `Totp.cpp`）

每个条目可选关联 TOTP 密钥，生成 6/8 位时间步验证码。

### 3.3.1 vault_totp.rs 核心 API

```rust
use totp_rs::{Totp, Algorithm, Secret};

pub struct VaultTotp;

impl VaultTotp {
    /// 从 Base32 密钥创建 TOTP 实例
    pub fn new_from_base32(
        secret_b32: &str,
        digits: u32,
        period: u32,
    ) -> Result<Totp, String> {
        let secret = Secret::Encoded(secret_b32.to_string());
        let totp = Totp::new(Algorithm::SHA1, digits, period, secret.to_bytes()
            .map_err(|e| format!("Base32 decode failed: {}", e))?)?;
        Ok(totp)
    }

    /// 生成当前 TOTP 码
    pub fn generate_current(totp: &Totp) -> Result<String, String> {
        let token = totp.generate_current()
            .map_err(|e| format!("TOTP generate failed: {}", e))?;
        Ok(token)
    }

    /// 获取剩余有效秒数（用于 UI 倒计时环）
    pub fn time_remaining(period: u32) -> u32 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        period - (now as u32 % period)
    }

    /// 从 otpauth:// URI 解析 TOTP 参数
    pub fn from_otpauth_uri(uri: &str) -> Result<(String, u32, u32), String> {
        let totp = Totp::from_url(uri)
            .map_err(|e| format!("OTPAuth URI parse failed: {}", e))?;
        let secret = totp.secret_base32();
        Ok((secret, totp.digits, totp.step))
    }
}
```

### 3.3.2 前端 TOTP 显示

条目详情中 TOTP 区域：圆形倒计时环 + 6 位数字 + 一键复制。倒计时每秒刷新，到期自动重新生成。

---

## 3.4 HIBP 泄露检测（Phase 1，参考 KeePassXC `HibpDownloader.cpp`）

调用 [Have I Been Pwned](https://haveibeenpwned.com/) API 进行 k-匿名查询——只发送密码 SHA-1 哈希的前 5 位，完整哈希永不离开本地。

### 3.4.1 vault_hibp.rs 核心 API

```rust
use sha1::{Sha1, Digest};
use reqwest::Client;

pub struct VaultHibp {
    client: Client,
}

impl VaultHibp {
    pub fn new() -> Self {
        Self { client: Client::new() }
    }

    /// k-匿名泄露检测
    ///
    /// 1. SHA1(password) → 取前 5 位 hex 作为 range 前缀
    /// 2. GET https://api.pwnedpasswords.com/range/{prefix}
    /// 3. 在返回的哈希后缀列表中匹配
    /// 4. 返回泄露次数（0 = 未泄露）
    pub async fn check(&self, password: &str) -> Result<u32, String> {
        let mut hasher = Sha1::new();
        hasher.update(password.as_bytes());
        let hash_hex = hex::encode(hasher.finalize());
        let (prefix, suffix) = (&hash_hex[..5], &hash_hex[5..]);

        let url = format!("https://api.pwnedpasswords.com/range/{}", prefix);
        let resp = self.client.get(&url)
            .header("Add-Padding", "true")
            .send().await
            .map_err(|e| format!("HIBP request failed: {}", e))?;

        let body = resp.text().await
            .map_err(|e| format!("HIBP read failed: {}", e))?;

        for line in body.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() == 2 && parts[0].eq_ignore_ascii_case(suffix) {
                return Ok(parts[1].parse().unwrap_or(0));
            }
        }
        Ok(0)
    }

    /// 批量检测（逐条解密 → 检测 → 立即 zeroize，同一时刻只有一个明文）
    pub async fn audit_vault(
        &self,
        conn: &Connection,
        vault_key: &[u8; 32],
    ) -> Result<Vec<(String, u32)>, String> {
        let entries = vault::list_all(conn)?;
        let mut results = Vec::new();
        for entry in &entries {
            let per_entry_key = VaultCrypto::derive_entry_key(vault_key, &entry.id);
            let plaintext = VaultCrypto::decrypt_field(&entry.encrypted_password, &per_entry_key)?;
            let count = self.check(plaintext.as_str()).await?;
            drop(plaintext);
            vault::update_hibp_count(conn, &entry.id, count as i32)?;
            results.push((entry.id.clone(), count));
        }
        Ok(results)
    }
}
```

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

### 3.5.1 方案

使用 Tauri 2 官方 `plugin-biometric`。核心流程：**生物识别只做身份验证（bool），vault_key 的加解密由系统密钥管理器完成**。

```rust
use tauri_plugin_biometric::Biometric;

/// 生物识别解锁流程
pub async fn vault_unlock_biometric(state: &State) -> Result<bool, String> {
    let biometric = Biometric::new(state.app_handle());

    if !biometric.is_available() {
        return Err("Biometric not available".into());
    }

    let conn = get_db_conn(state)?;
    let biometric_key_enc = vault_meta::get(&conn, "biometric_key_enc")?;

    match biometric_key_enc {
        None => Err("Biometric not enrolled".into()),
        Some(enc) => {
            // authenticate() 只返回 bool（是否通过），不返回密钥
            let authenticated = biometric.authenticate(
                "解锁密码管理箱",
                "验证您的身份以解锁密码箱",
            ).await?;

            if authenticated {
                // 通过后，用系统密钥管理器解密 biometric_key_enc → vault_key
                let vault_key = Self::decrypt_with_system_key(&enc)?;
                vault_crypto::set_vault_key(&vault_key);
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }
}

/// 启用生物识别：用系统密钥保护 vault_key
pub async fn vault_enroll_biometric(master_password: &str, state: &State) -> Result<(), String> {
    let conn = get_db_conn(state)?;
    // 从主密码派生 vault_key（读取 vault_key_salt → Argon2id）
    let salt_b64 = vault_meta::get(&conn, "vault_key_salt")?
        .ok_or("Vault not initialized")?;
    let salt = base64_decode(&salt_b64)?;
    let vault_key = VaultCrypto::derive_vault_key(master_password, &salt)?;

    // 用系统密钥管理器加密 vault_key（平台相关）
    // Windows: DPAPI / CredProtect
    // macOS: Keychain Secure Enclave
    // Linux: libsecret
    let enc = Self::encrypt_with_system_key(&vault_key)?;
    vault_meta::set(&conn, "biometric_key_enc", &enc)?;
    Ok(())
}

/// 用平台系统密钥管理器加密（生物识别通过后可解密）
fn encrypt_with_system_key(vault_key: &[u8; 32]) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    { dpapi_encrypt(vault_key) }
    #[cfg(target_os = "macos")]
    { keychain_encrypt(vault_key) }
    #[cfg(target_os = "linux")]
    { libsecret_encrypt(vault_key) }
}

/// 用平台系统密钥管理器解密
fn decrypt_with_system_key(enc: &str) -> Result<[u8; 32], String> {
    #[cfg(target_os = "windows")]
    { dpapi_decrypt(enc) }
    #[cfg(target_os = "macos")]
    { keychain_decrypt(enc) }
    #[cfg(target_os = "linux")]
    { libsecret_decrypt(enc) }
}
```

### 3.5.2 解锁流程

```
用户打开密码箱
  │
  ├─▶ 检测 biometric_key_enc 是否存在
  │     ├─ 存在 → 弹出生物识别提示（Windows Hello / Touch ID）
  │     │         ├─ 成功 → 解密 vault_key → 解锁
  │     │         └─ 失败 → 回退到主密码输入
  │     └─ 不存在 → 直接要求主密码输入
  │
  └─▶ 解锁后：vault_key 在内存中，5 分钟无操作自动锁定
```

---

## 3.6 SSH Agent 集成（Phase 2A，参考 KeePassXC `sshagent/`）

将 SSH 私钥存入密码箱，解锁后自动加载到系统 ssh-agent，锁定时自动移除。

### 3.6.1 vault_ssh.rs 核心 API

```rust
pub struct VaultSsh;

impl VaultSsh {
    /// 将条目中的 SSH 私钥加载到 ssh-agent
    pub fn add_to_agent(entry_id: &str, conn: &Connection, vault_key: &[u8; 32]) -> Result<(), String> {
        let entry = vault::get_entry_decrypted(conn, entry_id, vault_key)?;
        let private_key = entry.password;
        let comment = format!("assistant-vault:{}", entry.title);

        #[cfg(unix)]
        {
            let mut agent = ssh2::Session::new()?;
            agent.connect_unix()?;
            agent.userauth_agent(None)?;
            agent.add_identity(private_key.as_bytes(), comment.as_bytes())?;
        }
        #[cfg(windows)]
        {
            let _ = Command::new("ssh-add").stdin(Stdio::piped()).spawn()?;
        }
        Ok(())
    }

    /// 从 ssh-agent 移除指定密钥
    pub fn remove_from_agent(entry_id: &str) -> Result<(), String> { ... }

    /// vault 锁定时移除所有已加载的 SSH 密钥
    pub fn remove_all_from_agent() -> Result<(), String> { ... }

    /// 列出当前 ssh-agent 中的密钥
    pub fn list_agent_identities() -> Result<Vec<String>, String> { ... }
}
```

### 3.6.2 生命周期

| 事件 | SSH Agent 行为 |
|------|---------------|
| vault 解锁 | 自动加载标记为 `sshAutoAdd: true` 的 SSH Key 条目 |
| vault 锁定 | 自动移除所有由 vault 加载的密钥 |
| 条目删除 | 从 ssh-agent 移除对应密钥 |
| 条目编辑 | 移除旧密钥 → 加载新密钥 |

---

## 3.7 紧急访问（Phase 2A，参考 Vaultwarden `emergency_access`）

指定信任联系人，在用户失联后可申请获取密码库访问权。本地版简化为**恢复密钥**机制。

### 3.7.1 方案

```rust
pub struct EmergencyAccess;

impl EmergencyAccess {
    /// 从恢复密钥派生加密密钥（Argon2id，与主密码同级安全）
    fn derive_recovery_key(recovery_key: &str) -> Result<[u8; 32], String> {
        let salt = b"assistant-vault-emergency-salt";
        let params = Params::new(65536, 3, 4, Some(32))
            .map_err(|e| format!("Argon2 params error: {}", e))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut key = [0u8; 32];
        argon2.hash_password_into(recovery_key.as_bytes(), salt, &mut key)
            .map_err(|e| format!("Argon2 derivation failed: {}", e))?;
        Ok(key)
    }

    /// 生成恢复密钥（一次性，打印/保存到安全位置）
    pub fn generate_recovery_key(conn: &Connection, vault_key: &[u8; 32]) -> Result<String, String> {
        let recovery_key: String = rand::thread_rng()
            .sample_iter(&Alphanumeric).take(32).collect();
        let recovery_derived = Self::derive_recovery_key(&recovery_key)?;
        let vault_key_plaintext = VaultPlaintext::from_str(&hex::encode(vault_key));
        let encrypted_vault_key = VaultCrypto::encrypt_field(
            &vault_key_plaintext,
            &recovery_derived,
            CipherAlgorithm::Aes256Gcm,
        )?;
        vault_meta::set(conn, "emergency_token", &encrypted_vault_key)?;
        Ok(recovery_key)
    }

    /// 使用恢复密钥解锁（替代主密码）
    pub fn unlock_with_recovery_key(recovery_key: &str, conn: &Connection) -> Result<bool, String> {
        let encrypted = vault_meta::get(conn, "emergency_token")?
            .ok_or("No recovery key enrolled")?;
        let key = Self::derive_recovery_key(recovery_key)?;
        let vault_key_hex = VaultCrypto::decrypt_field(&encrypted, &key)?;
        let vault_key_bytes = hex::decode(vault_key_hex.unsecure_ref())
            .map_err(|e| format!("Hex decode failed: {}", e))?;
        let vault_key: [u8; 32] = vault_key_bytes.try_into()
            .map_err(|_| "Invalid vault_key length")?;
        vault_crypto::set_vault_key(&vault_key);
        Ok(true)
    }
}
```

恢复密钥由用户打印或保存到安全位置，**不存储明文**。使用后可选择重新生成（旧密钥失效）。

---

## 3.8 AutoType 自动填充（Phase 2B，参考 KeePassXC `AutoType`）

全局快捷键触发，根据当前活动窗口标题自动匹配条目，模拟键盘输入用户名 + Tab + 密码。

### 3.8.1 方案

```rust
pub struct AutoType;

impl AutoType {
    /// 根据窗口标题匹配条目
    pub fn match_entries(window_title: &str, conn: &Connection) -> Result<Vec<VaultEntrySummary>, String> {
        let entries = vault::list_entries(conn, None, None, None)?;
        let matched: Vec<_> = entries.iter()
            .filter(|e| {
                if let Some(url) = &e.url {
                    Self::url_matches_title(url, window_title)
                } else {
                    false
                }
            })
            .cloned()
            .collect();
        Ok(matched)
    }

    /// 执行自动输入序列
    pub fn type_sequence(username: &str, password: &str) -> Result<(), String> {
        enigo::Key::Unicode(username)?;  // 输入用户名
        enigo::Key::Tab?;                // Tab 切换到密码框
        enigo::Key::Unicode(password)?;  // 输入密码
        enigo::Key::Return?;             // 回车提交
        Ok(())
    }
}
```

依赖 `enigo` crate 跨平台模拟键盘输入。全局快捷键由 Tauri `GlobalShortcut` plugin 注册。

---

## 3.9 Diceware 密码短语生成（Phase 1）

使用 EFF 大词表生成易记忆的密码短语，补充现有密码生成器的纯随机字符模式。EFF 词表来自 [https://www.eff.org/deeplinks/2016/07/new-wordlists-random-passphrases](https://www.eff.org/deeplinks/2016/07/new-wordlists-random-passphrases)，含 7776 个常见英文单词。

### 3.9.1 方案：扩展现有前端密码生成器

现有 `password-generator.service.ts` 已实现 `generatePassword()`（随机字符）+ `getCrackTimeEstimation()`（熵值估算），**直接扩展即可**，无需 Rust 后端参与：

```typescript
// src/tools/password-generator/password-generator.service.ts 新增

import effLargeWordlist from "./eff-large-wordlist";

export function generatePassphrase(
  wordCount: number = 4,
  separator: string = "-",
  capitalize: boolean = false,
): string {
  const words: string[] = [];
  for (let i = 0; i < wordCount; i++) {
    const idx = secureRandomIndex(effLargeWordlist.length);
    let word = effLargeWordlist[idx];
    if (capitalize) word = word.charAt(0).toUpperCase() + word.slice(1);
    words.push(word);
  }
  return words.join(separator);
}
```

词表文件 `eff-large-wordlist.ts` 为 7776 个单词的 `readonly string[]` 常量数组（~50KB gzip 后 ~15KB），直接打包进前端 bundle。

**不选 Rust 后端的理由**：
- 现有生成器是纯前端（`crypto.getRandomValues`），Diceware 也是纯计算，无必须走 IPC 的理由
- 避免与现有前端生成器割裂，用户在 vault 表单点"生成"体验一致
- 未来如需中文/日文词表，可按 locale 懒加载对应词表文件

---

## 4. 云同步设计

### 4.1 方案概览

| 方案 | 优先级 | 实现量 | 冲突解决 | 历史追踪 |
|------|--------|--------|---------|---------|
| **4.2 Git 同步** | ✅ 首选（Phase 2A） | ~670 行（参考 prs） | Git 原生三路合并 | ✅ 完整 Git 历史 |
| **4.3 WebDAV/S3 同步** | 备选（Phase 2B） | ~800+ 行 | 自定义合并算法 | ❌ 无历史 |

---

### 4.2 Git 同步（首选，参考 prs）

prs 的同步方案极其简洁优雅——**直接用 Git 作为同步后端**，每个密码条目导出为一个 AES-256-GCM 加密文件，整个密码库就是一个 Git 仓库。Git 提供免费的冲突解决（三路合并）、完整历史追踪、离线支持和多设备同步。

#### 4.2.1 Git 仓库结构

```
~/.assistant/vault/                ← Git 仓库
├── .git/                          ← Git 元数据
├── .vault-meta                    ← vault 元信息（版本号、导出时间）
├── entries/
│   ├── a1b2c3d4.enc              ← 单条目加密文件（文件名 = entry_id）
│   ├── e5f6g7h8.enc
│   └── ...
└── deletions/
    └── deleted.json              ← 已删除条目 ID 列表 + 删除时间
```

每个 `.enc` 文件内容：

```json
{
  "id": "a1b2c3d4-...",
  "encryptedTitle": "base64(AES-256-GCM(per_entry_key, title))",
  "encryptedUsername": "base64(...)",
  "encryptedUrl": "base64(...)",
  "category": "login",
  "encryptedPassword": "base64(AES-256-GCM(per_entry_key, password))",
  "encryptedNotes": "base64(...)",
  "encryptedCustomFields": "base64(...)",
  "encryptedTags": "base64(...)",
  "isFavorite": true,
  "revision": 3,
  "createdAt": 1729468800,
  "updatedAt": 1729555200
}
```

> **所有可识别个人身份的字段均加密**（title/username/url/tags），仅 `id`/`category`/`isFavorite`/`revision`/时间戳为明文。
> 这样即使仓库推送到公开 GitHub，攻击者也无法从 Git 历史中获知用户在哪些网站有账号。
> `category` 保留明文是因为其值域极小（5 种），信息熵可忽略。

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

`vault_entries.deleted_at` 软删除与 `deletions/deleted.json` 的同步规则：

| 事件 | DB 行为 | Git 行为 |
|------|---------|---------|
| 删除条目 | `UPDATE vault_entries SET deleted_at = now() WHERE id = ?` | `deleted.json` 追加 `{id, deleted_at}`；.enc 文件**保留**（Git 需要历史） |
| 拉取时发现远程删除 | `UPDATE vault_entries SET deleted_at = ...` | 从 `deleted.json` 读取 |
| 拉取时远程有但本地已删除 | 跳过（本地删除优先） | — |
| 永久清理（用户手动） | `DELETE FROM vault_entries WHERE deleted_at IS NOT NULL` | 删除 .enc 文件 + 从 `deleted.json` 移除 |

> 软删除条目在前端列表中默认隐藏，可通过"回收站"查看和恢复。

#### 4.2.5 GitSync 核心 API

```rust
// assistant-sync/src/git_sync.rs

pub struct GitSync {
    repo_path: PathBuf,
}

/// Git 同步就绪状态（参考 prs Readyness）
pub enum Readyness {
    NoSync,                           // 未初始化 git
    RepoState(RepositoryState),       // Git 异常状态（merge/rebase 中）
    Dirty,                            // 有未提交变更
    Ready,                            // 就绪，可操作
}

pub enum RepositoryState {
    Merge,
    Rebase,
    RebaseInteractive,
    CherryPick,
    Unknown,
}

impl GitSync {
    pub fn new(repo_path: PathBuf) -> Self {
        Self { repo_path }
    }

    /// 初始化 Git 仓库
    pub fn init(&self) -> Result<()> {
        if self.is_init() { return Ok(()); }
        self.git(&["init"])?;
        self.git(&["commit", "-q", "--allow-empty", "-m", "Initialize password vault"])?;
        Ok(())
    }

    /// 从远程克隆
    pub fn clone(&self, url: &str) -> Result<()> {
        let parent = self.repo_path.parent().ok_or("No parent")?;
        Command::new("git").args(["clone", url, &self.repo_path.to_string_lossy()])
            .current_dir(parent).status()?;
        Ok(())
    }

    /// 操作前：拉取远程变更
    pub fn prepare(&self) -> Result<()> {
        if !self.is_init() { return Ok(()); }
        if !self.has_remote()? { return Ok(()); }
        self.pull()?;
        Ok(())
    }

    /// 操作后：提交并推送
    pub fn finalize<M: AsRef<str>>(&self, msg: M) -> Result<()> {
        if !self.is_init() { return Ok(()); }
        if self.is_dirty()? {
            self.commit_all(msg)?;
        }
        if self.has_remote()? && self.need_to_push()? {
            self.push()?;
        }
        Ok(())
    }

    /// 拉取远程变更
    pub fn pull(&self) -> Result<()> {
        Command::new("git").args(["pull", "-q"])
            .current_dir(&self.repo_path).status()?;
        Ok(())
    }

    /// 推送本地变更
    pub fn push(&self) -> Result<()> {
        Command::new("git").args(["push", "-q"])
            .current_dir(&self.repo_path).status()?;
        Ok(())
    }

    /// 提交所有变更
    pub fn commit_all<M: AsRef<str>>(&self, msg: M) -> Result<()> {
        Command::new("git").args(["add", "."])
            .current_dir(&self.repo_path).status()?;
        Command::new("git").args(["commit", "-q", "-m", msg.as_ref()])
            .current_dir(&self.repo_path).status()?;
        Ok(())
    }

    /// 检查就绪状态
    pub fn readyness(&self) -> Result<Readyness> {
        if !self.is_init() { return Ok(Readyness::NoSync); }
        if self.is_dirty()? { return Ok(Readyness::Dirty); }
        if let Some(state) = self.repo_state()? {
            return Ok(Readyness::RepoState(state));
        }
        Ok(Readyness::Ready)
    }

    /// 智能推送优化（参考 prs）
    /// 30 秒内 pull 过，且本地和远程 hash 相同 → 跳过 push
    fn need_to_push(&self) -> Result<bool> {
        let last_pulled = self.git_last_pull_time()?;
        if last_pulled.elapsed().unwrap_or_default() > Duration::from_secs(30) {
            return Ok(true);
        }
        let branch = self.git_current_branch()?;
        let upstream = self.git_branch_upstream(&branch)?;
        Ok(self.git_ref_hash(&branch)? != self.git_ref_hash(&upstream)?)
    }

    pub fn is_init(&self) -> bool {
        self.repo_path.join(".git").exists()
    }

    pub fn has_remote(&self) -> Result<bool> { ... }
    pub fn add_remote_url(&self, remote: &str, url: &str) -> Result<()> { ... }
    pub fn set_remote_url(&self, remote: &str, url: &str) -> Result<()> { ... }

    fn git(&self, args: &[&str]) -> Result<Output> {
        let output = Command::new("git").args(args)
            .current_dir(&self.repo_path).output()?;
        if !output.status.success() {
            return Err(format!("git {} failed: {}", args.join(" "),
                String::from_utf8_lossy(&output.stderr)).into());
        }
        Ok(output)
    }
}
```

#### 4.2.6 Vault 与 GitSync 集成

```rust
// 每次密码操作自动同步
pub fn vault_create_entry(entry: &VaultEntryCreate, conn: &Connection, vault_key: &[u8; 32]) -> Result<String> {
    // 1. 同步准备
    if let Some(sync) = get_git_sync() {
        sync.prepare()?;
    }

    // 2. 写入数据库
    let id = vault::create_entry(conn, entry, vault_key)?;

    // 3. 导出条目到 .enc 文件
    let entry_data = vault::get_entry(conn, &id, vault_key)?;
    vault_file::export_entry(&entry_data, &get_vault_dir()?.join("entries"))?;

    // 4. 同步完成
    if let Some(sync) = get_git_sync() {
        sync.finalize(format!("vault: added {}", entry.title))?;
    }

    Ok(id)
}
```

#### 4.2.7 Git 冲突处理

Git 同步的冲突由 Git 本身处理：

| 场景 | Git 行为 | 对用户的影响 |
|------|---------|-------------|
| 不同条目修改 | 自动合并（不同文件） | 无感知 |
| 同一条目，不同字段 | 可能自动合并（JSON 内） | 需检查 |
| 同一条目，同字段 | CONFLICT 标记 | 需在 VaultConflictResolve 中选择 |
| 远程删除 + 本地修改 | CONFLICT | 需选择保留或删除 |

对于 Git 无法自动合并的冲突，前端检测 `Readyness::RepoState(Merge)` 状态，弹出冲突解决 UI（显示 Git diff，让用户选择本地/远程版本）。

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

```rust
const SSH_PERSIST_CMD: &str =
    "ssh -o 'ControlMaster auto' \
        -o 'ControlPath /tmp/.prs-session--%r@%h:%p' \
        -o 'ControlPersist 1h' \
        -o 'ConnectTimeout 10'";

const SSH_PERSIST_HOST_WHITELIST: [&str; 2] = ["github.com", "gitlab.com"];

fn cmd_git(args: &[&str], dir: &Path, connects_remote: bool) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(dir).current_dir(dir);
    if connects_remote && guess_ssh_persist_support(dir) {
        cmd.env("GIT_SSH_COMMAND", SSH_PERSIST_CMD);
    }
    cmd.args(args);
    cmd
}
```

- 白名单机制：只对 `github.com` / `gitlab.com` 启用，避免不兼容服务出错
- 进程级缓存：`lazy_static!` + `Mutex<HashMap<PathBuf, bool>>`，同一进程只判断一次
- Windows 上跳过（`cfg!(unix)` guard），仅 Unix 生效

---

### 4.3 WebDAV/S3 同步（备选，Phase 2B）

面向国内坚果云用户或无 Git 环境的场景。采用文件级同步：整个 vault 导出为一个加密文件，通过 WebDAV/S3 同步。

#### 4.3.1 同步流程

```
本地 vault.db
  │
  ├─▶ 导出: vault_entries → 序列化 → AES-256-GCM(vault_key) → vault.enc 文件
  │
  ├─▶ 上传: PUT vault.enc 到远程（If-Match: etag 防覆盖）
  │
  ├─▶ 下载: GET vault.enc → 解密 → 与本地 merge → 写入 vault.db
  │
  └─▶ 冲突: etag 不匹配 → 下载远程 → 两方 merge → 冲突条目标记供用户决策
```

#### 4.3.2 vault.enc 文件格式

```json
{
  "version": 1,
  "entries": [
    {
      "id": "uuid",
      "title": "GitHub",
      "username": "user@example.com",
      "url": "https://github.com",
      "category": "login",
      "encryptedPassword": "base64...",
      "encryptedNotes": "base64...",
      "encryptedCustomFields": "base64...",
      "tags": ["dev", "work"],
      "isFavorite": true,
      "revision": 3,
      "createdAt": 1729468800,
      "updatedAt": 1729555200
    }
  ],
  "deletedIds": ["uuid-of-deleted-entry"],
  "exportedAt": 1729555200
}
```

> 注意：vault.enc 中的 `encryptedPassword` 等字段是**二次加密**的——条目级密钥加密后，导出时再用 vault_key 做整体加密。这样即使 vault.enc 文件泄露，没有 master_password 也无法解密任何条目。

#### 4.3.3 SyncBackend trait

```rust
#[async_trait]
pub trait SyncBackend: Send + Sync {
    async fn get(&self, path: &str) -> Result<(Vec<u8>, Etag), SyncError>;
    async fn put(&self, path: &str, data: &[u8], if_match: Option<&Etag>) -> Result<Etag, SyncError>;
    async fn delete(&self, path: &str) -> Result<(), SyncError>;
    async fn test_connection(&self) -> Result<(), SyncError>;
}

pub struct Etag(pub String);

pub enum SyncError {
    Network(String),
    Auth(String),
    Conflict,
    NotFound,
    Io(String),
}
```

### 4.3.4 WebDAV 实现

```rust
pub struct WebDavBackend {
    client: reqwest::Client,
    base_url: Url,
    username: String,
    password: String,
}

impl WebDavBackend {
    pub fn new(base_url: &str, username: &str, password: &str) -> Result<Self, SyncError> {
        Ok(Self {
            client: reqwest::Client::new(),
            base_url: Url::parse(base_url)?,
            username: username.to_string(),
            password: password.to_string(),
        })
    }
}

#[async_trait]
impl SyncBackend for WebDavBackend {
    async fn get(&self, path: &str) -> Result<(Vec<u8>, Etag), SyncError> {
        let url = self.base_url.join(path)?;
        let resp = self.client.get(url)
            .basic_auth(&self.username, Some(&self.password))
            .send().await?;
        let etag = resp.headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|s| Etag(s.to_string()))
            .unwrap_or(Etag(String::new()));
        let data = resp.bytes().await?.to_vec();
        Ok((data, etag))
    }

    async fn put(&self, path: &str, data: &[u8], if_match: Option<&Etag>) -> Result<Etag, SyncError> {
        let url = self.base_url.join(path)?;
        let mut req = self.client.put(url)
            .basic_auth(&self.username, Some(&self.password))
            .body(data.to_vec());
        if let Some(etag) = if_match {
            req = req.header("If-Match", &etag.0);
        }
        let resp = req.send().await?;
        if resp.status() == reqwest::StatusCode::PRECONDITION_FAILED {
            return Err(SyncError::Conflict);
        }
        let etag = resp.headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|s| Etag(s.to_string()))
            .unwrap_or(Etag(String::new()));
        Ok(etag)
    }
    // ...
}
```

### 4.3.5 合并算法（WebDAV 模式，参考 KeePassXC Merger.cpp）

KeePassXC Merger 的核心逻辑（从参考源码提炼）：

1. **条目合并**：按 UUID 匹配，不存在则 Added，存在则比较 `lastModificationTime` 取更新者
2. **位置合并**：`locationChanged` 时间戳比较，取更新位置
3. **删除合并**：合并双方删除记录，已删除条目从目标中移除
4. **元数据合并**：取更新时间戳更大者的元数据字段

```rust
pub enum MergeAction {
    KeepLocal,
    KeepRemote,
    MergeAuto(MergedEntry),
    Conflict(ConflictEntry),
}

pub struct MergedEntry {
    pub id: String,
    pub source: MergeSource,
}

pub struct ConflictEntry {
    pub id: String,
    pub local: VaultEntry,
    pub remote: VaultEntry,
}

pub enum MergeSource {
    LocalNewer,
    RemoteNewer,
}

pub fn merge_entries(
    local: &[VaultEntry],
    remote: &[VaultEntry],
    local_deletions: &[String],
    remote_deletions: &[String],
) -> Vec<MergeAction> {
    let remote_map: HashMap<&str, &VaultEntry> = remote.iter().map(|e| (e.id.as_str(), e)).collect();
    let mut actions = Vec::new();

    for local_entry in local {
        if remote_deletions.contains(&local_entry.id) {
            continue;
        }
        match remote_map.get(local_entry.id.as_str()) {
            None => actions.push(MergeAction::KeepLocal),
            Some(remote_entry) => {
                if local_entry.updated_at > remote_entry.updated_at {
                    actions.push(MergeAction::MergeAuto(MergedEntry {
                        id: local_entry.id.clone(),
                        source: MergeSource::LocalNewer,
                    }));
                } else if remote_entry.updated_at > local_entry.updated_at {
                    actions.push(MergeAction::MergeAuto(MergedEntry {
                        id: local_entry.id.clone(),
                        source: MergeSource::RemoteNewer,
                    }));
                } else if entries_differ(local_entry, remote_entry) {
                    actions.push(MergeAction::Conflict(ConflictEntry {
                        id: local_entry.id.clone(),
                        local: local_entry.clone(),
                        remote: (*remote_entry).clone(),
                    }));
                }
            }
        }
    }

    for remote_entry in remote {
        if !local_deletions.contains(&remote_entry.id)
            && !local.iter().any(|l| l.id == remote_entry.id)
        {
            actions.push(MergeAction::KeepRemote);
        }
    }

    actions
}
```

### 4.3.6 同步引擎（WebDAV 模式）

```rust
pub struct SyncEngine<B: SyncBackend> {
    backend: B,
    db: Arc<Mutex<Connection>>,
    crypto: VaultCrypto,
    config: SyncConfig,
}

pub struct SyncConfig {
    pub remote_path: String,
    pub auto_sync_interval_secs: u64,
    pub on_conflict: ConflictStrategy,
}

pub enum ConflictStrategy {
    PreferLocal,
    PreferRemote,
    AskUser,
}

impl<B: SyncBackend> SyncEngine<B> {
    pub async fn push(&self, vault_key: &[u8; 32]) -> Result<SyncResult, SyncError> {
        let entries = vault::list_all(&self.db)?;
        let vault_file = VaultFile::from_entries(&entries, vault_key)?;
        let data = serde_json::to_vec(&vault_file)?;
        let etag = self.backend.put(&self.config.remote_path, &data, None).await?;
        vault_meta::set(&self.db, "sync_remote_etag", &etag.0)?;
        Ok(SyncResult::Pushed)
    }

    pub async fn pull(&self, vault_key: &[u8; 32]) -> Result<SyncResult, SyncError> {
        let (data, etag) = self.backend.get(&self.config.remote_path).await?;
        let vault_file: VaultFile = serde_json::from_slice(&data)?;
        let remote_entries = vault_file.to_entries(vault_key)?;
        let local_entries = vault::list_all(&self.db)?;
        let actions = merge::merge_entries(&local_entries, &remote_entries, &[], &[]);
        self.apply_merge_actions(actions, &remote_entries)?;
        vault_meta::set(&self.db, "sync_remote_etag", &etag.0)?;
        Ok(SyncResult::Pulled)
    }

    pub async fn sync(&self, vault_key: &[u8; 32]) -> Result<SyncResult, SyncError> {
        let local_etag = vault_meta::get(&self.db, "sync_remote_etag")?;
        let (data, remote_etag) = self.backend.get(&self.config.remote_path).await?;
        if remote_etag.0 == local_etag.unwrap_or_default() {
            return self.push(vault_key).await;
        }
        let vault_file: VaultFile = serde_json::from_slice(&data)?;
        let remote_entries = vault_file.to_entries(vault_key)?;
        let local_entries = vault::list_all(&self.db)?;
        let actions = merge::merge_entries(&local_entries, &remote_entries, &[], &[]);
        let conflicts = self.apply_merge_actions(actions, &remote_entries)?;
        if conflicts.is_empty() {
            self.push(vault_key).await?;
            Ok(SyncResult::Synced)
        } else {
            Ok(SyncResult::Conflict(conflicts))
        }
    }
}
```

### 4.3.7 WebDAV 兼容云服务

| 服务 | WebDAV 地址 | 特点 |
|------|-----------|------|
| **坚果云** | `https://dav.jianguoyun.com/dav/` | 国内最稳定，免费 1GB |
| **Nextcloud** | 自部署 | 开源私有云，完全可控 |
| **群晖 NAS** | `https://nas:port/webdav/` | 群晖自带 WebDAV |
| **Koofr** | `https://app.koofr.net/dav/` | 欧洲，免费 10GB |

---

## 5. Tauri Commands

```rust
// src-tauri/src/cmd/vault.rs

#[tauri::command]
pub fn vault_init(master_password: String, state: State<DbState>) -> Result<(), String> {
    let conn = get_db_conn(&state)?;
    vault_crypto::init_vault(&conn, &master_password)
}

#[tauri::command]
pub fn vault_unlock(master_password: String, state: State<DbState>) -> Result<bool, String> {
    let conn = get_db_conn(&state)?;
    vault_crypto::verify_and_unlock(&conn, &master_password)
}

#[tauri::command]
pub fn vault_is_locked(state: State<DbState>) -> Result<bool, String> {
    Ok(vault_crypto::is_vault_locked())
}

#[tauri::command]
pub fn vault_lock() -> Result<(), String> {
    vault_crypto::lock_vault()
}

#[tauri::command]
pub fn vault_list_entries(
    category: Option<String>,
    search: Option<String>,
    tag: Option<String>,
    state: State<DbState>,
) -> Result<Vec<VaultEntrySummary>, String> {
    let conn = get_db_conn(&state)?;
    vault::list_entries(&conn, category, search, tag)
}

#[tauri::command]
pub fn vault_get_entry(id: String, state: State<DbState>) -> Result<VaultEntryDetail, String> {
    let conn = get_db_conn(&state)?;
    let vault_key = vault_crypto::get_vault_key()?;
    vault::get_entry_decrypted(&conn, &id, &vault_key)
}

#[tauri::command]
pub fn vault_create_entry(entry: VaultEntryCreate, state: State<DbState>) -> Result<String, String> {
    let conn = get_db_conn(&state)?;
    let vault_key = vault_crypto::get_vault_key()?;
    vault::create_entry(&conn, &entry, &vault_key)
}

#[tauri::command]
pub fn vault_update_entry(id: String, entry: VaultEntryUpdate, state: State<DbState>) -> Result<(), String> {
    let conn = get_db_conn(&state)?;
    let vault_key = vault_crypto::get_vault_key()?;
    vault::update_entry(&conn, &id, &entry, &vault_key)
}

#[tauri::command]
pub fn vault_delete_entry(id: String, state: State<DbState>) -> Result<(), String> {
    let conn = get_db_conn(&state)?;
    vault::soft_delete_entry(&conn, &id)
}

#[tauri::command]
pub fn vault_change_master_password(
    current: String,
    new: String,
    state: State<DbState>,
) -> Result<(), String> {
    let conn = get_db_conn(&state)?;
    vault_crypto::change_master_password(&conn, &current, &new)
}

#[tauri::command]
pub async fn vault_sync_push(state: State<DbState>) -> Result<(), String> { ... }

#[tauri::command]
pub async fn vault_sync_pull(state: State<DbState>) -> Result<SyncResult, String> { ... }

#[tauri::command]
pub async fn vault_sync_now(state: State<DbState>) -> Result<SyncResult, String> { ... }

#[tauri::command]
pub fn vault_get_sync_config(state: State<DbState>) -> Result<Option<SyncConfig>, String> { ... }

#[tauri::command]
pub fn vault_set_sync_config(config: SyncConfig, state: State<DbState>) -> Result<(), String> { ... }

#[tauri::command]
pub fn vault_resolve_conflict(
    entry_id: String,
    resolution: ConflictResolution,
    state: State<DbState>,
) -> Result<(), String> { ... }

#[tauri::command]
pub fn vault_git_init(state: State<DbState>) -> Result<(), String> { ... }

#[tauri::command]
pub fn vault_git_clone(url: String, state: State<DbState>) -> Result<(), String> { ... }

#[tauri::command]
pub fn vault_git_add_remote(name: String, url: String, state: State<DbState>) -> Result<(), String> { ... }

#[tauri::command]
pub fn vault_git_set_remote(name: String, url: String, state: State<DbState>) -> Result<(), String> { ... }

#[tauri::command]
pub fn vault_git_readyness(state: State<DbState>) -> Result<Readyness, String> { ... }

#[tauri::command]
pub fn vault_git_pull(state: State<DbState>) -> Result<(), String> { ... }

#[tauri::command]
pub fn vault_git_push(state: State<DbState>) -> Result<(), String> { ... }

#[tauri::command]
pub fn vault_totp_generate(id: String, state: State<DbState>) -> Result<String, String> {
    let conn = get_db_conn(&state)?;
    let vault_key = vault_crypto::get_vault_key()?;
    vault_totp::generate_current(&conn, &id, &vault_key)
}

#[tauri::command]
pub fn vault_totp_time_remaining(period: u32) -> Result<u32, String> {
    Ok(vault_totp::time_remaining(period))
}

#[tauri::command]
pub async fn vault_hibp_check(id: String, state: State<DbState>) -> Result<u32, String> {
    let conn = get_db_conn(&state)?;
    let vault_key = vault_crypto::get_vault_key()?;
    let hibp = VaultHibp::new();
    let entry = vault::get_entry_decrypted(&conn, &id, &vault_key)?;
    let count = hibp.check(&entry.password).await?;
    vault::update_hibp_count(&conn, &id, count as i32)?;
    Ok(count)
}

#[tauri::command]
pub async fn vault_hibp_audit(state: State<DbState>) -> Result<Vec<(String, u32)>, String> {
    let conn = get_db_conn(&state)?;
    let vault_key = vault_crypto::get_vault_key()?;
    let hibp = VaultHibp::new();
    hibp.audit_vault(&conn, &vault_key).await
}

#[tauri::command]
pub async fn vault_unlock_biometric(state: State<DbState>) -> Result<bool, String> {
    vault_crypto::vault_unlock_biometric(&state).await
}

#[tauri::command]
pub async fn vault_enroll_biometric(master_password: String, state: State<DbState>) -> Result<(), String> {
    vault_crypto::vault_enroll_biometric(&master_password, &state).await
}

#[tauri::command]
pub fn vault_ssh_add_to_agent(id: String, state: State<DbState>) -> Result<(), String> {
    let conn = get_db_conn(&state)?;
    let vault_key = vault_crypto::get_vault_key()?;
    VaultSsh::add_to_agent(&id, &conn, &vault_key)
}

#[tauri::command]
pub fn vault_ssh_remove_from_agent(id: String) -> Result<(), String> {
    VaultSsh::remove_from_agent(&id)
}

#[tauri::command]
pub fn vault_generate_recovery_key(state: State<DbState>) -> Result<String, String> {
    let conn = get_db_conn(&state)?;
    let vault_key = vault_crypto::get_vault_key()?;
    EmergencyAccess::generate_recovery_key(&conn, &vault_key)
}

#[tauri::command]
pub fn vault_unlock_with_recovery_key(recovery_key: String, state: State<DbState>) -> Result<bool, String> {
    let conn = get_db_conn(&state)?;
    EmergencyAccess::unlock_with_recovery_key(&recovery_key, &conn)
}
```

### invoke_handler 注册

在 `src-tauri/src/lib.rs` 的 `invoke_handler` 宏中新增：

```rust
cmd::vault_init,
cmd::vault_unlock,
cmd::vault_is_locked,
cmd::vault_lock,
cmd::vault_list_entries,
cmd::vault_get_entry,
cmd::vault_create_entry,
cmd::vault_update_entry,
cmd::vault_delete_entry,
cmd::vault_change_master_password,
cmd::vault_sync_push,
cmd::vault_sync_pull,
cmd::vault_sync_now,
cmd::vault_get_sync_config,
cmd::vault_set_sync_config,
cmd::vault_resolve_conflict,
cmd::vault_git_init,
cmd::vault_git_clone,
cmd::vault_git_add_remote,
cmd::vault_git_set_remote,
cmd::vault_git_readyness,
cmd::vault_git_pull,
cmd::vault_git_push,
cmd::vault_totp_generate,
cmd::vault_totp_time_remaining,
cmd::vault_hibp_check,
cmd::vault_hibp_audit,
cmd::vault_unlock_biometric,
cmd::vault_enroll_biometric,
cmd::vault_ssh_add_to_agent,
cmd::vault_ssh_remove_from_agent,
cmd::vault_generate_recovery_key,
cmd::vault_unlock_with_recovery_key,
```

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

```typescript
interface VaultState {
  isLocked: boolean;
  isInitialized: boolean;
  entries: VaultEntrySummary[];
  searchQuery: string;
  categoryFilter: string | null;
  tagFilter: string | null;
  syncStatus: "idle" | "syncing" | "conflict" | "error";
  conflicts: ConflictEntry[];
  hibpResults: Map<string, number>;
  biometricAvailable: boolean;

  unlock: (masterPassword: string) => Promise<void>;
  unlockBiometric: () => Promise<void>;
  lock: () => Promise<void>;
  init: (masterPassword: string) => Promise<void>;
  loadEntries: () => Promise<void>;
  createEntry: (entry: VaultEntryCreate) => Promise<string>;
  updateEntry: (id: string, entry: VaultEntryUpdate) => Promise<void>;
  deleteEntry: (id: string) => Promise<void>;
  getEntryDetail: (id: string) => Promise<VaultEntryDetail>;
  syncNow: () => Promise<void>;
  resolveConflict: (entryId: string, resolution: "local" | "remote" | "merge") => Promise<void>;
  generateTotp: (id: string) => Promise<string>;
  hibpCheckEntry: (id: string) => Promise<number>;
  hibpAuditAll: () => Promise<void>;
  sshAddToAgent: (id: string) => Promise<void>;
  sshRemoveFromAgent: (id: string) => Promise<void>;
  generateRecoveryKey: () => Promise<string>;
}
```

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

```typescript
// vaultStore.ts
const LOCK_TIMEOUT_MS = 5 * 60 * 1000; // 5 分钟
let lockTimer: ReturnType<typeof setTimeout>;

function resetLockTimer() {
  clearTimeout(lockTimer);
  lockTimer = setTimeout(() => {
    vaultStore.getState().lock();
  }, LOCK_TIMEOUT_MS);
}

// 窗口失焦时锁定
document.addEventListener("visibilitychange", () => {
  if (document.hidden && vaultStore.getState().isLocked === false) {
    resetLockTimer();
  }
});
```

### 6.5 剪贴板安全（参考 prs `clipboard.rs` + `clip_revert.rs`）

prs 不是简单"清空剪贴板"，而是**恢复剪贴板原来的内容**（用户之前复制的东西不会丢）。我们采用同样模式：

```typescript
async function copyPassword(password: string) {
  const oldContent = await navigator.clipboard.readText();
  await navigator.clipboard.writeText(password);
  message.success("已复制，30 秒后自动恢复");
  setTimeout(() => {
    navigator.clipboard.readText().then((current) => {
      if (current === password) {
        navigator.clipboard.writeText(oldContent);
      }
    });
  }, 30_000);
}
```

- 超时后**恢复原内容**而非清空，用户之前复制的东西不丢失
- 校验 `current === password` 防止覆盖用户后续复制的新内容

### 6.6 解密后正则搜索（参考 prs `grep.rs`）

SQLite 全文搜索仅覆盖明文字段（`title`/`username`/`url`/`tags`）。加密字段（`notes`/`customFields`）需解密后搜索：

```typescript
async function searchVault(query: string, category?: string): Promise<VaultEntrySummary[]> {
  const isRegex = query.startsWith("/") && query.endsWith("/");
  const pattern = isRegex ? new RegExp(query.slice(1, -1), "i") : null;

  let results = await vaultService.listEntries(category);

  if (pattern) {
    const decrypted = await Promise.all(
      results.map((e) => vaultService.getEntry(e.id))
    );
    results = decrypted
      .filter((e) => {
        const searchable = [e.notes, ...Object.values(e.customFields ?? {})].join("\n");
        return pattern.test(searchable);
      })
      .map((e) => toSummary(e));
  }

  return results;
}
```

- 普通搜索：SQLite `LIKE` 查询明文字段，O(1)
- 正则搜索（`/pattern/`）：解密后匹配加密字段，O(n) 但仅在用户显式使用正则时触发

---

## 7. 设置项扩展

按项目规则，新增设置项必须同步更新接口：

### 7.1 AppConf 新增字段

```typescript
// src/settings/core/types/confTypes.ts
export interface AppConf {
  // ... 已有字段
  vaultAutoLockMinutes: number;      // 自动锁定时间，默认 5
  vaultClipboardClearSeconds: number; // 剪贴板清零时间，默认 30
  vaultAutoSyncIntervalMinutes: number; // 自动同步间隔，默认 5，0=禁用
  vaultCipherAlgorithm: "aes256Gcm" | "chacha20Poly1305"; // 加密算法，默认 aes256Gcm
  vaultBiometricEnabled: boolean;    // 生物识别快速解锁，默认 false
  vaultHibpAutoAudit: boolean;       // 自动 HIBP 审计，默认 true
  vaultSshAutoAdd: boolean;          // 解锁后自动加载 SSH 密钥，默认 true
  vaultAutoTypeEnabled: boolean;     // AutoType 自动填充，默认 false
  vaultAutoTypeShortcut: string;     // AutoType 全局快捷键，默认 "Ctrl+Shift+V"
}
```

### 7.2 Rust AppConf 对应

```rust
// assistant-config/src/app_conf.rs
pub struct AppConf {
    // ... 已有字段
    #[serde(default = "default_vault_auto_lock")]
    pub vault_auto_lock_minutes: u32,
    #[serde(default = "default_vault_clipboard_clear")]
    pub vault_clipboard_clear_seconds: u32,
    #[serde(default = "default_vault_auto_sync")]
    pub vault_auto_sync_interval_minutes: u32,
    #[serde(default)]
    pub vault_cipher_algorithm: VaultCipherAlgorithm,
    #[serde(default)]
    pub vault_biometric_enabled: bool,
    #[serde(default = "default_true")]
    pub vault_hibp_auto_audit: bool,
    #[serde(default = "default_true")]
    pub vault_ssh_auto_add: bool,
    #[serde(default)]
    pub vault_auto_type_enabled: bool,
    #[serde(default = "default_auto_type_shortcut")]
    pub vault_auto_type_shortcut: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum VaultCipherAlgorithm {
    #[default]
    Aes256Gcm,
    ChaCha20Poly1305,
}

fn default_vault_auto_lock() -> u32 { 5 }
fn default_vault_clipboard_clear() -> u32 { 30 }
fn default_vault_auto_sync() -> u32 { 5 }
fn default_true() -> bool { true }
fn default_auto_type_shortcut() -> String { "Ctrl+Shift+V".into() }
```

---

## 8. Rust 依赖新增

### 8.1 assistant-providers/Cargo.toml

```toml
argon2 = "0.5"
hkdf = "0.12"
zeroize = { version = "1", features = ["derive"] }
secstr = "0.5"
subtle = "2"
aes-gcm = "0.10"
chacha20poly1305 = "0.10"
totp-rs = "5"
sha1 = "0.10"
hex = "0.4"
reqwest = { workspace = true }
enigo = "0.3"
```

### 8.2 新 crate: assistant-sync/Cargo.toml

```toml
[package]
name = "assistant-sync"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
log = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
base64 = { workspace = true }
assistant-storage = { workspace = true }
assistant-providers = { workspace = true }

# Git 同步（首选）— 调用系统 git CLI，无需额外 crate
# 若需纯 Rust git（无系统依赖），可启用 git2 feature:
# git2 = "0.19"

# WebDAV/S3 同步（备选）— 需启用 webdav feature:
# reqwest = { workspace = true }
# url = "2"
# aws-sdk-s3 = "1"

[features]
default = ["git"]
git = []
webdav = ["reqwest", "url"]
s3 = ["reqwest", "url", "aws-sdk-s3"]
```

### 8.3 workspace Cargo.toml 新增

```toml
[workspace]
members = [
    # ... 已有
    "crates/assistant-sync",
]

[workspace.dependencies]
argon2 = "0.5"
hkdf = "0.12"
zeroize = { version = "1", features = ["derive"] }
secstr = "0.5"
subtle = "2"
aes-gcm = "0.10"
chacha20poly1305 = "0.10"
totp-rs = "5"
sha1 = "0.10"
hex = "0.4"
enigo = "0.3"
assistant-sync = { path = "crates/assistant-sync" }
```

---

## 9. i18n

### zh-CN.json 新增

```json
{
  "tools": {
    "password_vault": {
      "name": "密码管理箱",
      "description": "安全存储和管理您的密码、凭证与密钥",
      "unlock_title": "解锁密码箱",
      "unlock_placeholder": "请输入主密码",
      "unlock_button": "解锁",
      "init_title": "创建密码箱",
      "init_description": "设置主密码以加密您的密码库",
      "init_button": "创建",
      "lock_button": "锁定",
      "add_entry": "新增条目",
      "edit_entry": "编辑条目",
      "delete_entry": "删除条目",
      "category_login": "登录",
      "category_card": "银行卡",
      "category_identity": "身份",
      "category_note": "安全笔记",
      "category_ssh_key": "SSH 密钥",
      "sync_now": "立即同步",
      "sync_push": "上传",
      "sync_pull": "下载",
      "sync_idle": "未同步",
      "sync_syncing": "同步中...",
      "sync_conflict": "同步冲突",
      "sync_error": "同步失败",
      "sync_mode_git": "Git 同步",
      "sync_mode_webdav": "WebDAV 同步",
      "sync_mode_s3": "S3 同步",
      "git_init": "初始化 Git 仓库",
      "git_clone": "从远程克隆",
      "git_add_remote": "添加远程仓库",
      "git_set_remote": "修改远程仓库",
      "git_pull": "拉取",
      "git_push": "推送",
      "git_readyness_no_sync": "未初始化",
      "git_readyness_dirty": "有未提交变更",
      "git_readyness_ready": "就绪",
      "git_readyness_merge": "合并中，需解决冲突",
      "git_readyness_rebase": "变基中，需解决冲突",
      "conflict_title": "同步冲突",
      "conflict_use_local": "使用本地",
      "conflict_use_remote": "使用远程",
      "conflict_skip": "跳过",
      "copied": "已复制，{seconds} 秒后自动清除",
      "password_generate": "生成密码",
      "password_diceware": "密码短语",
      "password_diceware_words": "词数",
      "password_diceware_separator": "分隔符",
      "password_diceware_capitalize": "首字母大写",
      "totp_title": "两步验证 (TOTP)",
      "totp_generate": "生成验证码",
      "totp_copy": "复制验证码",
      "totp_setup": "设置 TOTP",
      "totp_scan_qr": "扫描二维码",
      "totp_preview": "预览",
      "hibp_title": "泄露检测",
      "hibp_check": "检测泄露",
      "hibp_audit_all": "全库审计",
      "hibp_safe": "未发现泄露",
      "hibp_leaked": "该密码已在 {count} 次数据泄露中出现",
      "hibp_auditing": "正在审计...",
      "hibp_audit_done": "审计完成，{leaked} 个密码存在泄露风险",
      "biometric_unlock": "生物识别解锁",
      "biometric_enroll": "启用生物识别",
      "biometric_not_available": "生物识别不可用",
      "biometric_prompt": "验证您的身份以解锁密码箱",
      "ssh_add_to_agent": "加载到 SSH Agent",
      "ssh_remove_from_agent": "从 SSH Agent 移除",
      "ssh_in_agent": "已加载",
      "ssh_auto_add": "解锁后自动加载",
      "emergency_generate": "生成恢复密钥",
      "emergency_unlock": "使用恢复密钥",
      "emergency_hint": "请将恢复密钥保存到安全位置，切勿与主密码存放在一起",
      "auto_type_title": "自动填充",
      "auto_type_trigger": "触发自动填充",
      "auto_type_no_match": "未找到匹配的条目",
      "master_password_wrong": "主密码错误",
      "vault_locked": "密码箱已锁定",
      "cipher_aes256gcm": "AES-256-GCM",
      "cipher_chacha20": "ChaCha20-Poly1305"
    }
  }
}
```

---

## 10. 安全审计清单

| # | 检查项 | 状态 |
|---|--------|------|
| 1 | 主密码不以明文存储 | ✅ Argon2id 哈希 |
| 2 | vault_key 不持久化到磁盘 | ✅ 仅内存持有 |
| 3 | 常量时间比较防时序攻击 | ✅ subtle::ConstantTimeEq |
| 4 | 条目级密钥隔离（HKDF） | ✅ per-entry key |
| 5 | 敏感内存保护 | ✅ secstr::SecVec (mlock 防 swap) + zeroize (Drop 清零) + unsecure_ref 访问控制 |
| 6 | AES-256-GCM / ChaCha20-Poly1305 双算法认证加密 | ✅ AEAD 内置 tag，密文首字节标识算法 |
| 7 | 随机 nonce 不重复 | ✅ 每次 12B random |
| 8 | Argon2id 高参数防暴力 | ✅ m=64MB, t=3, p=4 |
| 9 | 解锁失败延迟递增 | ✅ 3s 基础 × 失败次数 |
| 10 | 剪贴板安全 | ✅ 超时后恢复原内容（非清空），防覆盖后续复制 |
| 11 | 云端只存密文 | ✅ Git: .enc 文件条目级加密；WebDAV: vault.enc 整体加密 |
| 12 | 同步冲突不静默丢数据 | ✅ Git: 原生 CONFLICT 标记；WebDAV: 冲突条目标记供用户决策 |
| 13 | Git 仓库不泄露明文 | ✅ .enc 文件仅含加密字段，.gitignore 排除 vault.db |
| 14 | Git remote 传输加密 | ✅ HTTPS/SSH 协议层加密 |
| 15 | Git 凭证安全 | ✅ 使用系统 credential helper / SSH key，不存储密码 |
| 16 | TOTP 密钥加密存储 | ✅ encrypted_totp_secret 条目级加密，与密码同级保护 |
| 17 | HIBP k-匿名查询 | ✅ 只发 SHA-1 前 5 位，完整哈希不离境，Add-Padding 防流量分析 |
| 18 | HIBP 审计逐条解密 | ✅ 同一时刻内存中只有一个明文密码，用完立即 zeroize |
| 19 | 生物识别 vault_key 保护 | ✅ 系统密钥加密 vault_key 存入 vault_meta，生物识别失败回退主密码 |
| 20 | SSH Agent 生命周期管理 | ✅ vault 锁定时自动移除所有已加载密钥，防止锁定后密钥残留 |
| 21 | 恢复密钥不存明文 | ✅ 仅存加密后的 vault_key，恢复密钥由用户安全保管 |
| 22 | AutoType 不缓存密码 | ✅ 每次触发重新解密，输入完成后立即 zeroize |
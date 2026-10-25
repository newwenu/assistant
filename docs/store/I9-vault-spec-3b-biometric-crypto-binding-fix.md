# I9-3b — 生物识别密码学绑定修复设计

> 本文档为 I9 密码管理箱规格的 **第 3b 部分**，修复生物识别实现的密码学绑定缺陷。
> 前置文档：[I9-vault-spec-3a-biometric-platform.md](./I9-vault-spec-3a-biometric-platform.md)（平台 API 实现）
> 参考项目：[KeePassXC quickunlock/](../../references/keepassxc/src/quickunlock/)、[watchkey-win/](../../references/watchkey-win/)
> 修复范围：`biometric_ops.rs` + 三平台 `BiometricKeyStore` impl

---

## 1. 问题诊断

### 1.1 当前实现的安全缺陷

#### 缺陷 A：`wrapping_key` 明文暴露（🔴 致命）

[biometric_ops.rs:40-60](../../src-tauri/crates/assistant-security/src/security_db/biometric_ops.rs#L40-L60) 的 `enroll_biometric`：

```rust
let mut wrapping_key = [0u8; 32];
rand::Rng::fill(&mut rand::thread_rng(), &mut wrapping_key);

let encrypted_keys = VaultCrypto::encrypt_field(
    &VaultPlaintext::from(plaintext),
    &wrapping_key,
    CipherAlgorithm::Aes256Gcm,
);

let mut blob = Vec::with_capacity(32 + biometric_key_enc.len());
blob.extend_from_slice(&wrapping_key);          // ← 明文密钥！
blob.extend_from_slice(biometric_key_enc.as_bytes());  // ← 密文
key_store.set_key("vault-master-key", &blob)?;
```

**问题**：AES-256-GCM 解密密钥 `wrapping_key` 以明文存储在 blob 前 32 字节，紧邻密文。任何能读取 keyring/keychain 的进程可直接提取并解密得到 `bootstrap_key || master_key`。

**影响**：加密层形同虚设，等同于将主密钥明文存入 OS 凭据存储。

#### 缺陷 B：Windows TPM 签名结果被丢弃（🔴 致命）

[windows.rs:155-195](../../src-tauri/crates/assistant-security/src/biometric/windows.rs#L155-L195) 的 `get_key`：

```rust
let sign_result = credential.RequestSignAsync(&challenge_buffer)...;

match get_operation_status(&sign_result)? {
    KeyCredentialStatus::Success => {}  // ← 仅判断成功/失败
    ...
}

let entry = keyring_entry(key_id)?;
let encoded = entry.get_password()?;     // ← 直接读取，无需密码学证明
```

**问题**：TPM 签名输出未参与任何密钥推导，仅作为布尔门禁使用。签名结果被完全丢弃后，通过 `keyring` crate 直接读取凭据即可获取密钥材料。

**影响**：Windows Hello 认证可被绕过——同用户下任何进程都能调用 `keyring::Entry::get_password()` 获取密钥。

### 1.2 与参考项目对比

| 维度 | 本项目 (当前) | KeePassXC Win | watchkey-win | KeePassXC Mac |
|------|-------------|---------------|--------------|---------------|
| 签名参与密钥推导 | ❌ 丢弃 | ✅ SHA256(sig) | ✅ HKDF(sig) | N/A (OS 门控) |
| AES 密钥存储 | 🔴 明文拼接 | ✅ 不存储 | ✅ 不存储 | ⚠️ Keychain(受保护) |
| 密文存储 | keyring | 内存 | 磁盘 JSON | 内存 |
| 密钥/密文分离 | ❌ 同一 blob | ✅ 分离 | ✅ 分离 | ✅ 分离 |
| 持久化 | ✅ | ❌ 进程内 | ✅ | ❌ 进程内 |

---

## 2. 修复方案总览

### 2.1 核心原则

1. **绝不把解密密钥和密文拼接存储**
2. **Windows 平台：TPM 签名结果必须参与密钥推导**（参考 watchkey-win）
3. **macOS 平台：保持 SecAccessControl OS 门控**（参考 KeePassXC TouchID）
4. **Linux 平台：分离存储，接受平台限制**（参考 KeePassXC Polkit）

### 2.2 各平台策略

| 平台 | 策略 | 密码学绑定强度 | 持久化 |
|------|------|--------------|--------|
| **Windows** | HKDF(TPM_Sign(fixed_challenge)) → AES 密钥，密文存 keyring | ✅✅ 强 | ✅ |
| **macOS** | 随机 AES 密钥存 Keychain(SecAccessControl)，密文存 keyring | ✅ 中（OS 级） | ✅ |
| **Linux** | 随机 AES 密钥存 libsecret，密文存 libsecret（不同条目） | ⚠️ 弱（OS DAC） | ✅ |

---

## 3. Windows 平台修复设计

### 3.1 方案：HKDF 签名派生 + Keyring 持久化

**参考**：[watchkey-win/crypto.rs](../../references/watchkey-win/src/crypto.rs) + [watchkey-win/auth.rs](../../references/watchkey-win/src/auth.rs)

#### 3.1.1 数据流

```
enroll_biometric:
  ┌─────────────────────────────────────────────────────────────┐
  │ 输入: bootstrap_key(32B) || master_key(32B) = 64B plaintext │
  ├─────────────────────────────────────────────────────────────┤
  │ 1. ensure_credential()                                      │
  │    ├── OpenAsync(CREDENTIAL_NAME)                           │
  │    └── 若 NotFound → RequestCreateAsync(FailIfExists)       │
  │        → 弹 Windows Hello → 创建 TPM 密钥对                 │
  │                                                             │
  │ 2. challenge = SHA256("assistant-vault-biometric-v1")      │
  │    → 固定 challenge，确保每次签名一致（TPM 私钥不变时）    │
  │                                                             │
  │ 3. signature = RequestSignAsync(challenge)                   │
  │    → 弹 Windows Hello → 返回 TPM 签名结果                    │
  │                                                             │
  │ 4. wrapping_key = HKDF-SHA256(                              │
  │        salt = b"assistant-vault-v1",                        │
  │        ikm  = signature,                                    │
  │        info = b"aes-256-gcm-key"                            │
  │    ) → 32 字节 AES 密钥                                     │
  │                                                             │
  │ 5. nonce = random(12B)                                      │
  │ 6. ciphertext = AES-256-GCM-Encrypt(                        │
  │        key = wrapping_key,                                  │
  │        nonce = nonce,                                       │
  │        plaintext = bootstrap_key || master_key              │
  │    )                                                        │
  │                                                             │
  │ 7. blob = base64(nonce || ciphertext)                       │
  │    → 只存密文！不含 wrapping_key！                          │
  │                                                             │
  │ 8. key_store.set_key("vault-master-key", &blob)             │
  │    → keyring crate → Windows Credential Manager 通用凭据    │
  │                                                             │
  │ 9. 清零: wrapping_key, signature, plaintext                 │
  └─────────────────────────────────────────────────────────────┘

unlock_biometric:
  ┌─────────────────────────────────────────────────────────────┐
  │ 1. blob = key_store.get_key("vault-master-key")             │
  │    → keyring crate → Windows Credential Manager 读取密文     │
  │                                                             │
  │ 2. 解码: nonce(12B) || ciphertext                            │
  │                                                             │
  │ 3. challenge = SHA256("assistant-vault-biometric-v1")      │
  │    → 与 enroll 时相同的固定 challenge                       │
  │                                                             │
  │ 4. signature = RequestSignAsync(challenge)                   │
  │    → 弹 Windows Hello → 返回 TPM 签名结果                    │
  │                                                             │
  │ 5. wrapping_key = HKDF-SHA256(                              │
  │        salt = b"assistant-vault-v1",                        │
  │        ikm  = signature,                                    │
  │        info = b"aes-256-gcm-key"                            │
  │    ) → 必须与 enroll 时相同！                               │
  │                                                             │
  │ 6. plaintext = AES-256-GCM-Decrypt(                         │
  │        key = wrapping_key,                                  │
  │        nonce = nonce,                                       │
  │        ciphertext = ciphertext                              │
  │    ) → bootstrap_key(32B) || master_key(32B)               │
  │                                                             │
  │ 7. 清零: wrapping_key, signature, plaintext                 │
  └─────────────────────────────────────────────────────────────┘
```

#### 3.1.2 安全保证

| 攻击场景 | 修复前 | 修复后 |
|---------|--------|--------|
| 同用户进程读取 keyring | 直接提取 wrapping_key → 解密 | 获取密文但无 wrapping_key（需 TPM 签名） |
| 导出 Credential Manager 文件 | 包含明文 wrapping_key | 仅含密文，无解密密钥 |
| TPM 私钥被重置 | N/A | 签名变化 → HKDF 结果变化 → 解密失败 → master_key 丢失（安全降级） |
| 重放攻击 | N/A | 固定 challenge + TPM 签名确定性 → 无新风险 |

#### 3.1.3 新增辅助函数

```rust
/// 固定 challenge，确保每次签名结果一致（TPM 私钥不变时）
fn biometric_challenge() -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(b"assistant-vault-biometric-v1");
    hasher.finalize().to_vec()
}

/// 从 TPM 签名派生 AES-256-GCM 密钥
fn derive_wrapping_key(signature: &[u8]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(b"assistant-vault-v1"), signature);
    let mut key = [0u8; 32];
    hk.expand(b"aes-256-gcm-key", &mut key)
        .expect("32 bytes is a valid HKDF output length");
    key
}

/// 计算 signature hash 用于检测 TPM 重置
fn compute_signature_hash(signature: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(signature);
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}
```

#### 3.1.4 BiometricKeyStore trait 变更（Windows）

**`set_key(key_id, data)`**：

```
输入: data = base64(nonce || ciphertext) — 仅密文，不含明文密钥
操作:
  1. ensure_credential() — 创建/确认 TPM 密钥对
  2. keyring::Entry::set_password(base64(data)) — 存入 Windows Credential Manager
  3. 返回 Ok(())
```

**变更点**：
- ❌ 移除：`RequestSignAsync(challenge)` 签名步骤（签名由调用方 `biometric_ops.rs` 执行）
- ❌ 移除：`wrapping_key` 生成和拼接
- ✅ 保留：TPM 密钥对管理（ensure_credential）
- ✅ 保留：keyring crate 存储（Windows Credential Manager 通用凭据）

**`get_key(key_id)`**：

```
操作:
  1. keyring::Entry::get_password() — 从 Windows Credential Manager 读取
  2. 返回 Ok(data) — 原样返回 base64(nonce || ciphertext)，不解密
```

**变更点**：
- ❌ 移除：`RequestSignAsync(challenge)` 签名步骤（签名上移到 `biometric_ops.rs`）
- ❌ 移除：base64 decode（由调用方负责）
- ✅ 保留：keyring crate 读取（Windows Credential Manager 通用凭据）

**核心转变**：Windows 平台的 `BiometricKeyStore` 退化为**纯存储层**（仍通过 keyring crate → Windows Credential Manager），TPM 签名和 HKDF 密钥派生上移到 `biometric_ops.rs`。存储位置不变，变化的是存储内容（移除明文 wrapping_key）和解密密钥来源（从 blob 提取 → HKDF(TPM_Signature) 运行时派生）。

---

## 4. macOS 平台修复设计

### 4.1 方案：SecAccessControl 门控 + 分离存储

**参考**：[KeePassXC TouchID.mm](../../references/keepassxc/src/quickunlock/TouchID.mm)

#### 4.1.1 数据流

```
enroll_biometric:
  ┌─────────────────────────────────────────────────────────────┐
  │ 输入: bootstrap_key(32B) || master_key(32B) = 64B plaintext │
  ├─────────────────────────────────────────────────────────────┤
  │ 1. random_key = random(32B), random_iv = random(12B)       │
  │                                                             │
  │ 2. ciphertext = AES-256-GCM-Encrypt(                        │
  │        key = random_key,                                    │
  │        iv = random_iv,                                      │
  │        plaintext = bootstrap_key || master_key              │
  │    )                                                        │
  │                                                             │
  │ 3. key_store.set_key("vault-master-aes-key",                │
  │        (random_key || random_iv).to_hex())                  │
  │    → 存入 Keychain，受 SecAccessControl 保护               │
  │    → 读取时弹 Touch ID                                      │
  │                                                             │
  │ 4. key_store.set_key("vault-master-ciphertext",             │
  │        base64(ciphertext))                                  │
  │    → 存入 Keychain 或本地文件（不受生物识别保护）          │
  │                                                             │
  │ 5. 清零: random_key, random_iv, plaintext                  │
  └─────────────────────────────────────────────────────────────┘

unlock_biometric:
  ┌─────────────────────────────────────────────────────────────┐
  │ 1. aes_key_material = key_store.get_key("vault-master-aes-key")
  │    → 触发 Touch ID → 返回 hex(random_key || random_iv)     │
  │                                                             │
  │ 2. 解析: random_key(32B) || random_iv(12B)                 │
  │                                                             │
  │ 3. ciphertext_b64 = key_store.get_key("vault-master-ciphertext")
  │    → 返回 base64(ciphertext)                                │
  │                                                             │
  │ 4. plaintext = AES-256-GCM-Decrypt(                         │
  │        key = random_key,                                    │
  │        iv = random_iv,                                      │
  │        ciphertext = base64_decode(ciphertext_b64)           │
  │    ) → bootstrap_key(32B) || master_key(32B)               │
  │                                                             │
  │ 5. 清零: random_key, random_iv, plaintext                  │
  └─────────────────────────────────────────────────────────────┘
```

#### 4.1.2 安全保证

| 攻击场景 | 修复前 | 修复后 |
|---------|--------|--------|
| 导出 Keychain | 包含完整密钥材料 | AES 密钥受 Touch ID 保护，导出需系统授权 |
| 绕过 Touch ID 读取 | 直接获取完整数据 | `vault-master-aes-key` 条目读取强制 Touch ID |
| Keychain 备份文件被窃 | 含明文 wrapping_key | 含加密的 AES 密钥（Touch ID 保护） |
| 内存转储 | N/A | 密钥用后立即清零（zeroize） |

#### 4.1.3 BiometricKeyStore trait 变更（macOS）

**保持现有实现不变**，仅需调整 `biometric_ops.rs` 的调用方式：

- `set_key("vault-master-aes-key", hex(random_key || random_iv))` — 受 SecAccessControl 保护
- `set_key("vault-master-ciphertext", base64(ciphertext))` — 普通存储

---

## 5. Linux 平台修复设计

### 5.1 方案：libsecret 分离存储

**参考**：[KeePassXC Polkit.cpp](../../references/keepassxc/src/quickunlock/Polkit.cpp)

#### 5.1.1 数据流

与 macOS 相同的分离存储模式：

```
enroll_biometric:
  1. random_key = random(32B), random_iv = random(12B)
  2. ciphertext = AES-256-GCM-Encrypt(random_key, random_iv, plaintext)
  3. key_store.set_key("vault-master-aes-key", hex(random_key || random_iv))
  4. key_store.set_key("vault-master-ciphertext", base64(ciphertext))
  5. 清零敏感数据

unlock_biometric:
  1. aes_key_material = key_store.get_key("vault-master-aes-key")
  2. ciphertext_b64 = key_store.get_key("vault-master-ciphertext")
  3. plaintext = AES-256-GCM-Decrypt(random_key, random_iv, ciphertext)
  4. 清零敏感数据
```

#### 5.1.2 平台限制说明

Linux 平台的生物识别安全强度受限于 OS 基础设施：

| 限制 | 说明 | 缓解措施 |
|------|------|---------|
| libsecret 无生物识别门控 | 读取不强制认证 | 文件权限限制（0600） |
| Polkit 与 keyring 独立 | 认证是布尔门禁 | 接受此限制，在 UI 层提示用户 |
| Process keyring 不持久 | 进程退出丢失 | 改用 SECRET_COLLECTION_DEFAULT（用户会话级） |

**重要**：即使 Linux 平台无法达到 Windows/macOS 的安全等级，修复后的实现仍优于当前：
- ✅ 移除了 `wrapping_key` 明文拼接
- ✅ 密钥和密文分离存储
- ⚠️ 但仍依赖 OS DAC 保护（非密码学绑定）

---

## 6. `biometric_ops.rs` 重构设计

### 6.1 架构变更

```
修复前:
  biometric_ops.rs:
    enroll_biometric:
      生成 wrapping_key → 加密 → [wrapping_key || 密文] → key_store.set_key()
    
    unlock_biometric:
      key_store.get_key() → [wrapping_key || 密文] → 用 wrapping_key 解密

修复后 (平台相关逻辑上移到 biometric_ops.rs):
  biometric_ops.rs:
    enroll_biometric:
      #[cfg(windows)]
        TPM 签名 → HKDF 派生 wrapping_key → 加密 → [nonce || 密文] → key_store.set_key()
      
      #[cfg(macos)]
        生成随机 wrapping_key → 加密 → 
          key_store.set_key("aes-key", hex(wrapping_key || iv))
          key_store.set_key("ciphertext", base64(ciphertext))
      
      #[cfg(linux)]
        同 macOS
    
    unlock_biometric:
      #[cfg(windows)]
        key_store.get_key() → [nonce || 密文]
        TPM 签名 → HKDF 派生 wrapping_key → 解密
      
      #[cfg(macos)]
        key_store.get_key("aes-key") → 触发 Touch ID → wrapping_key || iv
        key_store.get_key("ciphertext") → ciphertext
        解密
      
      #[cfg(linux)]
        同 macOS
```

### 6.2 伪代码

```rust
impl SecurityDbState {
    pub fn enroll_biometric(&self, key_store: &dyn BiometricKeyStore) -> Result<(), VaultError> {
        if self.is_locked() { return Err(VaultError::Locked); }
        if !key_store.is_available()? { return Err(VaultError::BiometricUnavailable); }
        if self.is_biometric_enrolled(key_store)? { self.disenroll_biometric(key_store)?; }

        let mk_guard = self.master_key.lock().unwrap();
        let master_key = mk_guard.as_ref().ok_or(VaultError::Locked)?;
        let bk_guard = self.bootstrap_key.lock().unwrap();
        let bootstrap_key = bk_guard.as_ref().ok_or(VaultError::Locked)?;

        let mut plaintext = Vec::with_capacity(64);
        plaintext.extend_from_slice(bootstrap_key.expose());
        plaintext.extend_from_slice(master_key.expose());

        #[cfg(target_os = "windows")]
        {
            let (nonce, ciphertext) = self.enroll_windows(key_store, &plaintext)?;
            let mut combined = Vec::with_capacity(12 + ciphertext.len());
            combined.extend_from_slice(&nonce);
            combined.extend_from_slice(&ciphertext);
            let encoded = base64::engine::general_purpose::STANDARD.encode(&combined);
            key_store.set_key("vault-master-key", encoded.as_bytes())?;
        }

        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            let mut wrapping_key = [0u8; 32];
            rand::Rng::fill(&mut rand::thread_rng(), &mut wrapping_key);
            let mut iv = [0u8; 12];
            rand::Rng::fill(&mut rand::thread_rng(), &mut iv);

            let encrypted = VaultCrypto::encrypt_field(
                &VaultPlaintext::from(plaintext.clone()),
                &wrapping_key,
                CipherAlgorithm::Aes256Gcm,
            ).map_err(VaultError::Crypto)?;

            let aes_key_hex = {
                let mut combined = wrapping_key.to_vec();
                combined.extend_from_slice(&iv);
                hex::encode(&combined)
            };

            key_store.set_key("vault-master-aes-key", aes_key_hex.as_bytes())?;
            key_store.set_key("vault-master-ciphertext", encrypted.as_bytes())?;

            wrapping_key.zeroize();
            iv.zeroize();
        }

        plaintext.zeroize();
        log::info!("Biometric enrolled successfully");
        Ok(())
    }

    pub fn unlock_biometric(&self, key_store: &dyn BiometricKeyStore) -> Result<(), VaultError> {
        if !self.is_locked() { return Err(VaultError::AlreadyUnlocked); }
        if !key_store.is_available()? { return Err(VaultError::BiometricUnavailable); }

        #[cfg(target_os = "windows")]
        {
            let blob = key_store.get_key("vault-master-key")?;
            let decoded = base64::engine::general_purpose::STANDARD.decode(&blob)
                .map_err(|e| VaultError::Crypto(format!("base64 decode: {e}")))?;

            if decoded.len() < 13 {
                return Err(VaultError::Crypto("Invalid biometric blob".into()));
            }
            let nonce: [u8; 12] = decoded[..12].try_into()
                .map_err(|_| VaultError::Crypto("Invalid nonce".into()))?;
            let ciphertext = &decoded[12..];

            let signature = self.request_tpm_signature(key_store)?;
            let wrapping_key = derive_wrapping_key(&signature);

            let keys_plaintext = VaultCrypto::decrypt_field(
                &base64::engine::general_purpose::STANDARD.encode(ciphertext),
                &wrapping_key,
            ).map_err(|e| VaultError::Crypto(format!("Decrypt failed: {e}")))?;

            if keys_plaintext.expose().len() != 64 {
                return Err(VaultError::Crypto("Invalid key length".into()));
            }

            self.restore_keys_from_plaintext(&keys_plaintext)?;
            wrapping_key.zeroize();
        }

        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            let aes_key_hex = String::from_utf8_lossy(
                &key_store.get_key("vault-master-aes-key")?
            ).to_string();

            let ciphertext_b64 = String::from_utf8_lossy(
                &key_store.get_key("vault-master-ciphertext")?
            ).to_string();

            let aes_key_bytes = hex::decode(&aes_key_hex)
                .map_err(|e| VaultError::Crypto(format!("hex decode: {e}")))?;

            if aes_key_bytes.len() != 44 {
                return Err(VaultError::Crypto("Invalid AES key material length".into()));
            }

            let mut wrapping_key = [0u8; 32];
            wrapping_key.copy_from_slice(&aes_key_bytes[..32]);
            let mut iv = [0u8; 12];
            iv.copy_from_slice(&aes_key_bytes[32..44]);

            let keys_plaintext = VaultCrypto::decrypt_field(
                &ciphertext_b64, &wrapping_key,
            ).map_err(|e| VaultError::Crypto(format!("Decrypt failed: {e}")))?;

            if keys_plaintext.expose().len() != 64 {
                return Err(VaultError::Crypto("Invalid key length".into()));
            }

            self.restore_keys_from_plaintext(&keys_plaintext)?;
            wrapping_key.zeroize();
            iv.zeroize();
        }

        log::info!("Vault unlocked via biometric");
        Ok(())
    }
}
```

### 6.3 Windows 平台辅助方法

```rust
#[cfg(target_os = "windows")]
impl SecurityDbState {
    fn request_tpm_signature(
        &self,
        key_store: &dyn BiometricKeyStore,
    ) -> Result<Vec<u8>, VaultError> {
        // 调用 WindowsHelloKeyStore 的新方法获取 TPM 签名
        // 或者直接在这里调用 TPM API（需要决定职责边界）
        todo!("见 §7 职责边界决策")
    }

    fn enroll_windows(
        &self,
        key_store: &dyn BiometricKeyStore,
        plaintext: &[u8],
    ) -> Result<([u8; 12], Vec<u8>), VaultError> {
        let signature = self.request_tpm_signature(key_store)?;
        let wrapping_key = derive_wrapping_key(&signature);

        let mut nonce = [0u8; 12];
        rand::Rng::fill(&mut rand::thread_rng(), &mut nonce);

        let encrypted = VaultCrypto::encrypt_field(
            &VaultPlaintext::from(plaintext.to_vec()),
            &wrapping_key,
            CipherAlgorithm::Aes256Gcm,
        ).map_err(VaultError::Crypto)?;

        let ciphertext = base64::engine::general_purpose::STANDARD
            .decode(&encrypted)
            .map_err(|e| VaultError::Crypto(format!("base64: {e}")))?;

        wrapping_key.zeroize();
        Ok((nonce, ciphertext))
    }

    fn restore_keys_from_plaintext(
        &self,
        keys_plaintext: &VaultPlaintext,
    ) -> Result<(), VaultError> {
        let mut bootstrap_arr = [0u8; 32];
        let mut master_arr = [0u8; 32];
        bootstrap_arr.copy_from_slice(&keys_plaintext.expose()[..32]);
        master_arr.copy_from_slice(&keys_plaintext.expose()[32..]);
        
        let bootstrap_key = MasterKey::new(bootstrap_arr);
        let master_key = MasterKey::new(master_arr);

        let sqlcipher_key = VaultCrypto::derive_sqlcipher_key(&bootstrap_key);
        let sqlcipher_key_hex = hex::encode(sqlcipher_key);

        let path = std::path::Path::new(&self.db_path);
        let conn = Connection::open(path).map_err(|e| VaultError::Database(e.to_string()))?;
        conn.execute_batch(&format!("PRAGMA key = \"x'{}'\"", sqlcipher_key_hex))
            .map_err(|e| VaultError::Database(format!("PRAGMA key: {e}")))?;
        conn.execute_batch(SQLCIPHER_PRAGMAS)
            .map_err(|e| VaultError::Database(format!("PRAGMA cipher: {e}")))?;

        {
            let mut sk = sqlcipher_key;
            sk.zeroize();
        }

        *self.master_key.lock().unwrap() = Some(master_key);
        *self.bootstrap_key.lock().unwrap() = Some(bootstrap_key);
        *self.conn.lock().unwrap() = Some(conn);

        self.check_biometric_consistency(key_store)?;

        Ok(())
    }
}
```

---

## 7. 职责边界决策

### 7.1 方案 A：TPM 操作留在 `windows.rs`（推荐）

**思路**：`BiometricKeyStore` trait 新增平台特定方法：

```rust
#[cfg(target_os = "windows")]
pub trait BiometricKeyStoreExt: BiometricKeyStore {
    fn request_signature(&self, key_id: &str) -> Result<Vec<u8>, VaultError>;
    fn ensure_credential(&self, key_id: &str) -> Result<(), VaultError>;
}
```

**优点**：
- 平台特定代码封装在平台文件中
- `biometric_ops.rs` 通过 trait 方法调用，无需 `#[cfg]`
- 符合现有架构分层

**缺点**：
- trait 需要扩展（但可通过 platform-specific trait 实现）

### 7.2 方案 B：TPM 操作上移到 `biometric_ops.rs`

**思路**：`windows.rs` 退化为纯存储，`biometric_ops.rs` 直接调用 Windows API。

**优点**：
- `BiometricKeyStore` trait 不变
- 密钥派生逻辑集中

**缺点**：
- `biometric_ops.rs` 需要 `#[cfg(target_os = "windows")]` 大段代码
- 违反平台抽象层的设计初衷
- 需要在 `biometric_ops.rs` 中引入 `windows` crate

**决策：采用方案 A**

理由：
1. 保持 `BiometricKeyStore` 作为统一抽象层
2. 平台特定细节封装在平台文件中
3. `biometric_ops.rs` 的条件编译最小化

---

## 8. 迁移兼容性

### 8.1 已注册生物识别的用户

**问题**：修复后，旧格式 `[wrapping_key(32B) || base64(nonce||ciphertext)]` 无法解析。

**解决方案**：

```rust
fn unlock_biometric(&self, key_store: &dyn BiometricKeyStore) -> Result<(), VaultError> {
    let blob = key_store.get_key("vault-master-key")?;

    // 尝试新格式
    if let Ok(result) = self.try_unlock_new_format(&blob, key_store) {
        return result;
    }

    // 回退到旧格式（一次性迁移）
    if let Ok(result) = self.try_unlock_legacy_format(&blob, key_store) {
        return result;
    }

    Err(VaultError::BiometricFailed("Invalid biometric data format".into()))
}

fn try_unlock_legacy_format(
    &self,
    blob: &[u8],
    key_store: &dyn BiometricKeyStore,
) -> Result<Result<(), VaultError>, ()> {
    // 旧格式: [wrapping_key(32B)] || [base64(nonce||ciphertext)]
    if blob.len() < 33 { return Err(()); }

    let wrapping_key = &blob[..32];
    let biometric_key_enc = String::from_utf8_lossy(&blob[32..]).to_string();

    let combined = base64::decode(&biometric_key_enc).ok_or(())?;
    if combined.len() < 12 { return Err(()); }

    let encrypted_keys_str = base64::encode(&combined[12..]);
    let keys_plaintext = VaultCrypto::decrypt_field(&encrypted_keys_str, wrapping_key).ok_or(())?;

    // 解密成功 → 重新以新格式注册
    self.restore_keys_from_plaintext(&keys_plaintext)?;
    self.disenroll_biometric(key_store)?;  // 删除旧格式
    self.enroll_biometric(key_store)?;     // 以新格式重新注册

    log::info!("Migrated legacy biometric format to new crypto-bound format");
    Ok(Ok(()))
}
```

### 8.2 迁移流程

```
用户点击生物识别解锁:
  1. 尝试新格式解析 → 失败
  2. 尝试旧格式解析 → 成功
  3. 用主密码解锁（旧格式需要 wrapping_key 明文，已可用）
  4. 自动 disenroll（删除旧格式）
  5. 自动 enroll（以新格式注册）
  6. 提示用户："生物识别安全格式已升级"
```

**注意**：旧格式迁移需要一次主密码输入（因为旧格式已经不安全，必须替换）。

---

## 9. 任务清单

### Phase 1：Windows 平台修复（优先级 P0）

- [ ] 9.1 新增 `derive_wrapping_key()`、`biometric_challenge()`、`compute_signature_hash()` 辅助函数到 `vault_crypto/mod.rs` 或新建 `biometric_crypto.rs`
- [ ] 9.2 `BiometricKeyStore` trait 新增 `request_signature()` 和 `ensure_credential()` 方法（通过 platform-specific trait 扩展）
- [ ] 9.3 重构 `windows.rs`：
  - [ ] 9.3.1 `set_key()` 移除 TPM 签名和 wrapping_key 生成，变为纯存储
  - [ ] 9.3.2 `get_key()` 移除 TPM 签名，变为纯读取
  - [ ] 9.3.3 新增 `request_signature()` 实现：固定 challenge → TPM 签名 → 返回签名结果
  - [ ] 9.3.4 新增 `ensure_credential()` 实现：OpenAsync / RequestCreateAsync
- [ ] 9.4 重构 `biometric_ops.rs`：
  - [ ] 9.4.1 `enroll_biometric()` Windows 分支：调用 `ensure_credential()` → `request_signature()` → `derive_wrapping_key()` → 加密 → 存储密文
  - [ ] 9.4.2 `unlock_biometric()` Windows 分支：读取密文 → `request_signature()` → `derive_wrapping_key()` → 解密
  - [ ] 9.4.3 新增 `try_unlock_legacy_format()` 兼容旧格式
- [ ] 9.5 编译验证：`cargo check -p assistant-security --target x86_64-pc-windows-msvc`
- [ ] 9.6 功能验证：Windows Hello 注册/解锁/迁移流程

### Phase 2：macOS 平台修复（优先级 P1）

- [ ] 10.1 重构 `macos.rs`：
  - [ ] 10.1.1 保持现有 `SecAccessControl` 实现不变
  - [ ] 10.1.2 确认 `set_key()` / `get_key()` 可处理任意 key_id
- [ ] 10.2 重构 `biometric_ops.rs`：
  - [ ] 10.2.1 `enroll_biometric()` macOS 分支：生成随机 wrapping_key+iv → 加密 → 分别存储
  - [ ] 10.2.2 `unlock_biometric()` macOS 分支：分别读取 → 解密
- [ ] 10.3 编译验证：`cargo check -p assistant-security --target x86_64-apple-darwin`
- [ ] 10.4 功能验证：Touch ID 注册/解锁流程

### Phase 3：Linux 平台修复（优先级 P1）

- [ ] 11.1 重构 `linux.rs`：
  - [ ] 11.1.1 确认 `set_key()` / `get_key()` 可处理任意 key_id
- [ ] 11.2 重构 `biometric_ops.rs`：
  - [ ] 11.2.1 `enroll_biometric()` Linux 分支：同 macOS 分离存储模式
  - [ ] 11.2.2 `unlock_biometric()` Linux 分支：同 macOS 分离存储模式
- [ ] 11.3 编译验证：`cargo check -p assistant-security --target x86_64-unknown-linux-gnu`
- [ ] 11.4 功能验证：libsecret 注册/解锁流程

### Phase 4：集成测试与清理（优先级 P2）

- [ ] 12.1 旧格式迁移端到端测试
- [ ] 12.2 三平台编译矩阵验证
- [ ] 12.3 安全审计：确认无明文密钥残留
- [ ] 12.4 更新设计文档：I9-vault-spec-3a-biometric-platform.md 标记过时部分
- [ ] 12.5 更新 VAULT-DESIGN.md §3.5 反映新架构

---

## 10. 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|---------|
| TPM 签名非确定性 | 不同次签名结果不同 → HKDF 密钥不同 → 无法解密 | 低 | 使用固定 challenge；参考 watchkey-win 验证 |
| 旧格式迁移失败 | 用户无法生物识别解锁 | 中 | 提供清晰的错误提示 + 主密码回退路径 |
| macOS Keychain 权限问题 | 新增条目可能被拒绝 | 低 | 复用现有 schema/service name |
| Linux 性能回归 | 多次 libsecret 调用 | 低 | libsecret 有缓存，影响可忽略 |
| 条件编译复杂度增加 | 代码维护困难 | 中 | 抽取公共逻辑到非条件编译函数 |

---

## 11. 验收标准

### 11.1 安全验收

- [ ] Windows: keyring 中存储的数据不含任何明文密钥
- [ ] Windows: 解密密钥 100% 来自 `HKDF(TPM_Signature)`
- [ ] macOS: AES 密钥仅在 Touch ID 通过后释放
- [ ] Linux: AES 密钥和密文分离存储（不同 key_id）
- [ ] 所有平台: 敏感数据使用后立即 zeroize

### 11.2 功能验收

- [ ] 新用户注册生物识别 → 正常工作
- [ ] 旧格式用户首次解锁 → 自动迁移到新格式
- [ ] 生物识别解锁 → 成功恢复 master_key
- [ ] 主密码变更 → 自动 disenroll 生物识别
- [ ] 设备更换/TPM 重置 → 安全失败（非静默返回错误密钥）

### 11.3 兼容性验收

- [ ] `BiometricKeyStore` trait 公共接口不变（仅新增平台扩展方法）
- [ ] 上层 Tauri Commands 零修改
- [ ] 前端代码零修改
- [ ] 现有数据库 schema 不变

---

## 附录 A：参考项目关键代码索引

| 项目 | 文件 | 关键函数/行号 | 用途 |
|------|------|---------------|------|
| watchkey-win | [crypto.rs:12-17](../../references/watchkey-win/src/crypto.rs#L12-L17) | `derive_key()` | HKDF 签名派生 |
| watchkey-win | [auth.rs:161-164](../../references/watchkey-win/src/auth.rs#L161-L164) | `fixed_challenge()` | 固定 challenge |
| watchkey-win | [auth.rs:272-276](../../references/watchkey-win/src/auth.rs#L272-L276) | `signature_hash()` | TPM 重置检测 |
| watchkey-win | [main.rs:87-125](../../references/watchkey-win/src/main.rs#L87-L125) | `obtain_master_key()` | 完整注册/解锁流程 |
| KeePassXC | [WindowsHello.cpp:51-69](../../references/keepassxc/src/quickunlock/WindowsHello.cpp#L51-L69) | `deriveEncryptionKey()` | SHA256 签名派生 |
| KeePassXC | [WindowsHello.cpp:106-128](../../references/keepassxc/src/quickunlock/WindowsHello.cpp#L106-L128) | `setKey()` | Windows 注册流程 |
| KeePassXC | [TouchID.mm:130-180](../../references/keepassxc/src/quickunlock/TouchID.mm#L130-L180) | `setKey()` | macOS 注册流程 |
| KeePassXC | [TouchID.mm:230-280](../../references/keepassxc/src/quickunlock/TouchID.mm#L230-L280) | `getKey()` | macOS 解锁流程 |
| KeePassXC | [Polkit.cpp:103-155](../../references/keepassxc/src/quickunlock/Polkit.cpp#L103-L155) | `setKey()` | Linux 注册流程 |
# I9-3a — 密码管理箱：生物识别平台真实实现

> 本文档为 I9 密码管理箱规格的 **第 3a 部分**，覆盖 P1-F 遗留项——三平台生物识别 stub 替换为真实 OS API 调用。
> 前置文档：[I9-vault-spec-3-security-enhance.md](./I9-vault-spec-3-security-enhance.md)（P1-D/E/F，上层逻辑已实现）
> 设计文档：[VAULT-DESIGN.md](../../VAULT-DESIGN.md) §3.4
> 任务清单：[I9-vault-tasklist-biometric.md](./I9-vault-tasklist-biometric.md)
>
> **当前状态**：上层逻辑（enroll/unlock/disenroll/自愈/Tauri Commands/前端）全部已实现，仅 3 个平台文件为 stub。

---

## 8a. P1-F 遗留 — 生物识别平台真实实现

### 8a.1 问题现状

| 文件 | 当前实现 | 效果 |
|------|---------|------|
| `biometric/windows.rs` | `is_available → Ok(false)` / `set_key,get_key → BiometricUnavailable` | Windows Hello 永远不可用 |
| `biometric/macos.rs` | 同上 | Touch ID 永远不可用 |
| `biometric/linux.rs` | 同上 | libsecret/Polkit 永远不可用 |

**影响**：用户无法使用生物识别解锁，P1-F 整体不可用。上层逻辑已完整实现，仅被 stub 阻断。

### 8a.2 BiometricKeyStore trait 回顾

```rust
pub trait BiometricKeyStore: Send + Sync {
    fn is_available(&self) -> Result<bool, VaultError>;
    fn set_key(&self, key_id: &str, data: &[u8]) -> Result<(), VaultError>;
    fn get_key(&self, key_id: &str) -> Result<Vec<u8>, VaultError>;
    fn delete_key(&self, key_id: &str) -> Result<(), VaultError>;
    fn has_key(&self, key_id: &str) -> Result<bool, VaultError>;
}
```

> **关键语义**：`set_key` / `get_key` 必须触发 OS 生物识别认证；`has_key` 仅检查条目存在性，**不触发认证**（用于自愈检测）。

---

### 8a.3 Windows Hello 实现

#### 8a.3.1 方案：KeyCredentialManager + TPM + DPAPI

Windows Hello 通过 TPM 生成非对称密钥对，私钥不离开 TPM。KeyCredentialManager 只支持签名操作，不支持存取任意数据。因此采用 **TPM 门控 + DPAPI 加密存储** 双层方案：

```
enroll (set_key):
  1. KeyCredentialManager::RequestCreateAsync(key_id)
     → TPM 生成密钥对 + 弹 Windows Hello 确认
  2. CryptProtectData(data) → DPAPI 加密
     → 写入 %LOCALAPPDATA%\assistant\biometric\{key_id}.bin

unlock (get_key):
  1. KeyCredentialManager::OpenAsync(key_id)
     → 获取密钥对句柄
  2. key_credential.RequestSignAsync(challenge)
     → TPM 签名 + 弹 Windows Hello 确认
  3. 签名成功 → 读取 .bin 文件 → CryptUnprotectData → 返回 data
```

> **为什么需要 TPM 签名步骤？** DPAPI 加密文件绑定当前 Windows 用户，同一用户下任何进程都能调用 `CryptUnprotectData` 解密。TPM 签名操作强制触发 Windows Hello 认证，形成门控：即使攻击者获取了 DPAPI 加密文件，也无法绕过 Windows Hello。

#### 8a.3.2 实现细节

**`is_available()`**：

```rust
fn is_available(&self) -> Result<bool, VaultError> {
    let result = unsafe {
        KeyCredentialManager::IsSupportedAsync()?.get()?
    };
    Ok(result)
}
```

- 返回 true 表示设备配置了 Windows Hello（PIN/指纹/虹膜至少一种）

**`set_key(key_id, data)`**：

1. `KeyCredentialManager::OpenAsync(key_id)` — 先检查密钥是否已存在
   - `KeyCredentialStatus::Success` → 密钥已存在，跳过创建
   - `KeyCredentialStatus::NotFound` → 继续创建
2. 若需创建：`KeyCredentialManager::RequestCreateAsync(key_id, KeyCredentialCreationOption::FailIfExists)` — **OS 弹 Windows Hello 确认**
   - `KeyCredentialStatus::Success` → 继续
   - `KeyCredentialStatus::CredentialAlreadyExists` → 静默继续（密钥已存在）
   - 用户取消/其他错误 → 返回 `VaultError::BiometricCancelled`
3. DPAPI 加密：`encrypted = DataProtectionProvider("LOCAL=user").ProtectAsync(data)` — WinRT DPAPI，绑定当前用户凭据
4. 写入文件：`%LOCALAPPDATA%\assistant\biometric\{key_id}.bin`
5. 返回 `Ok(())`

> **与原始方案的差异**：原始方案先 `RequestCreateAsync` 遇 `CredentialAlreadyExists` 再 `DeleteAsync` + 重试。实际实现改为先 `OpenAsync` 检查存在性，仅在 `NotFound` 时创建，避免不必要的删除-重建循环（更安全、更少弹窗）。

**`get_key(key_id)`**：

1. `KeyCredentialManager::OpenAsync(key_id)` — 获取已有密钥对
   - 不存在 → 返回 `VaultError::BiometricUnavailable`
2. 生成随机 challenge：`challenge = rand::[u8; 32]`
3. `key_credential.RequestSignAsync(challenge)` — **OS 弹 Windows Hello 确认**
   - 签名成功 → 继续
   - 用户取消 → 返回 `VaultError::BiometricCancelled`
   - 签名失败 → 返回 `VaultError::BiometricFailed`
4. 读取 DPAPI 加密文件：`%LOCALAPPDATA%\assistant\biometric\{key_id}.bin`
   - 文件不存在 → 返回 `VaultError::BiometricUnavailable`
5. `data = DataProtectionProvider.UnprotectAsync(encrypted)` — WinRT DPAPI 解密
6. 返回 `Ok(data)`

**`delete_key(key_id)`**：

1. `KeyCredentialManager::DeleteAsync(key_id)` — 删除 TPM 密钥对（幂等）
2. 删除 DPAPI 加密文件（若存在）
3. 返回 `Ok(())`

**`has_key(key_id)`**：

1. `KeyCredentialManager::OpenAsync(key_id)` — 尝试打开（**不触发认证**）
   - 成功 → `Ok(true)`
   - `NotFound` → `Ok(false)`

#### 8a.3.3 依赖

```toml
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.58", features = [
    "Foundation",
    "Security_Credentials",
    "Security_Cryptography",
    "Security_Cryptography_DataProtection",
    "Storage_Streams",
    "Win32_Foundation",
    "Win32_System_WinRT",
] }
```

> **注意**：`Security_Credentials`（非 `Security_Credentials_UI`）包含 `KeyCredentialManager`；DPAPI 使用 WinRT `DataProtectionProvider`（feature `Security_Cryptography_DataProtection`），而非 Win32 `CryptProtectData`；`Win32_System_WinRT` 提供 `RoInitialize`/`RoUninitialize`。

#### 8a.3.4 异步转同步

WinRT API 返回 `IAsyncOperation<T>` 或 `IAsyncAction`（无返回值），Rust 侧需同步等待：

```rust
fn wait_for_async<T>(async_op: IAsyncOperation<T>) -> Result<T, VaultError>
where T: windows::core::RuntimeType,
{
    async_op.get().map_err(map_win_error)
}

fn wait_for_action(async_op: IAsyncAction) -> Result<(), VaultError> {
    async_op.get().map_err(map_win_error)
}
```

> Tauri Commands 在 tokio 线程池中执行，阻塞等待 WinRT 异步操作是安全的（不阻塞 UI 线程）。

#### 8a.3.5 WinRT 生命周期

每个公开方法入口需 `RoInitialize(RO_INIT_MULTITHREADED)`，退出时 `RoUninitialize`。使用 RAII guard 确保配对：

```rust
struct WinRtGuard;
impl WinRtGuard {
    fn new() -> Result<Self, VaultError> {
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
            .map_err(|e| VaultError::BiometricFailed(format!("RoInitialize: {e}")))?;
        Ok(Self)
    }
}
impl Drop for WinRtGuard {
    fn drop(&mut self) { unsafe { RoUninitialize() }; }
}
```

#### 8a.3.6 DPAPI 实现选择

文档 §8a.3.1 描述使用 Win32 `CryptProtectData`/`CryptUnprotectData`。实际实现采用 **WinRT `DataProtectionProvider`** 方案，原因：

| 方案 | 优点 | 缺点 |
|------|------|------|
| Win32 `CryptProtectData` | 文档原始方案 | 手动内存管理（`LocalFree`）、需 `CRYPT_INTEGER_BLOB`、unsafe 代码多 |
| WinRT `DataProtectionProvider` | 自动内存管理、API 简洁、与 `windows` crate 一致 | 需 `Security_Cryptography_DataProtection` feature |

两者安全等价：均绑定当前用户凭据，均不支持 `CRYPTPROTECT_LOCAL_MACHINE`。WinRT 方案为参考项目 watchkey-win 验证过的实现。

---

### 8a.4 macOS Touch ID 实现

#### 8a.4.1 方案：Keychain Services + SecAccessControl

macOS Keychain 原生支持生物识别门控：创建条目时指定 `kSecAccessControlBiometryCurrentSet`，读取时 OS 自动弹 Touch ID。

```
enroll (set_key):
  1. SecAccessControlCreateWithFlags(
       kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
       kSecAccessControlBiometryCurrentSet)
  2. SecItemAdd({kSecClass: GenericPassword, kSecAttrAccount: key_id,
       kSecValueData: data, kSecAttrAccessControl: access_control})
     → OS 弹 Touch ID 确认

unlock (get_key):
  1. SecItemCopyMatching({kSecClass: GenericPassword, kSecAttrAccount: key_id,
       kSecReturnData: true})
     → OS 弹 Touch ID 确认 → 返回 data
```

#### 8a.4.2 实现细节

**`is_available()`**：

```rust
fn is_available(&self) -> Result<bool, VaultError> {
    // 尝试创建 SecAccessControl — 失败即设备不支持 Touch ID
    let access_control = unsafe {
        SecAccessControlCreateWithFlags(
            kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
            kSecAccessControlBiometryCurrentSet,
            std::ptr::null_mut(),
        )
    };
    Ok(!access_control.is_null())
}
```

> Mac 无 Touch ID（如 Mac Pro）→ `SecAccessControlCreateWithFlags` 返回 null → `is_available → Ok(false)`

**`set_key(key_id, data)`**：

1. 创建 AccessControl：`SecAccessControlCreateWithFlags(kSecAttrAccessibleWhenUnlockedThisDeviceOnly, kSecAccessControlBiometryCurrentSet, nil)`
   - 失败 → 返回 `VaultError::BiometricUnavailable`
2. 构造查询字典：
   ```
   kSecClass              → kSecClassGenericPassword
   kSecAttrAccount        → key_id
   kSecValueData          → data
   kSecAttrAccessControl  → access_control
   kSecAttrSynchronizable → false  (不同步到 iCloud)
   ```
3. `SecItemAdd(query, nil)` — **OS 弹 Touch ID 确认**
   - `errSecSuccess` → `Ok(())`
   - `errSecUserCanceled` → `VaultError::BiometricCancelled`
   - `errSecDuplicateItem` → 先 `SecItemDelete` 再重试
   - 其他 → `VaultError::BiometricFailed`

**`get_key(key_id)`**：

1. 构造查询字典：
   ```
   kSecClass       → kSecClassGenericPassword
   kSecAttrAccount → key_id
   kSecReturnData  → true
   kSecMatchLimit  → kSecMatchLimitOne
   ```
2. `SecItemCopyMatching(query, &mut result)` — **OS 弹 Touch ID 确认**
   - `errSecSuccess` → `Ok(result.into_bytes())`
   - `errSecUserCanceled` → `VaultError::BiometricCancelled`
   - `errSecItemNotFound` → `VaultError::BiometricUnavailable`
   - `errSecAuthFailed` → `VaultError::BiometricFailed`

**`delete_key(key_id)`**：

1. `SecItemDelete({ kSecClass: GenericPassword, kSecAttrAccount: key_id })`
   - `errSecItemNotFound` → 静默返回 `Ok(())`（幂等）

**`has_key(key_id)`**：

1. `SecItemCopyMatching({ kSecClass: GenericPassword, kSecAttrAccount: key_id, kSecReturnData: false })` — **不触发 Touch ID**
   - `errSecSuccess` → `Ok(true)`
   - `errSecItemNotFound` → `Ok(false)`

#### 8a.4.3 依赖

```toml
[target.'cfg(target_os = "macos")'.dependencies]
core-foundation = "0.10"
security-framework = "3.0"
```

> `security-framework` crate 封装了 Keychain Services API（`SecItemAdd` / `SecItemCopyMatching` / `SecItemDelete`），可直接使用。`SecAccessControlCreateWithFlags` 需通过 `core-foundation` FFI 调用。

---

### 8a.5 Linux libsecret 实现

#### 8a.5.1 方案：libsecret + Secret Service API + Polkit

Linux 无统一生物识别 API。`libsecret` 通过 D-Bus 与密钥存储守护进程（gnome-keyring / KDE Wallet）通信。守护进程解锁时通过 **Polkit** 弹框认证（密码/PAM/指纹，取决于 PAM 配置）。

```
enroll (set_key):
  1. secret_password_store_sync(schema, attributes, label, data,
       SECRET_COLLECTION_DEFAULT, cancellable)
     → 若 keyring 已锁定 → Polkit 弹框 → 存入 gnome-keyring

unlock (get_key):
  1. secret_password_lookup_sync(schema, attributes, cancellable)
     → 若 keyring 已锁定 → Polkit 弹框 → 返回 data
```

> **libsecret 的"生物识别"语义**：
> - 若 PAM 配置了指纹识别（fprintd），Polkit 弹框可能支持指纹
> - 默认行为是输入登录密码
> - 本质是"系统认证解锁"，而非严格意义上的"生物识别"
> - 在 Linux 生态中这是最接近的方案

#### 8a.5.2 实现细节

**`is_available()`**：

```rust
fn is_available(&self) -> Result<bool, VaultError> {
    // 检查 Secret Service D-Bus 是否可用
    let service = unsafe {
        secret_service_get_sync(SECRET_SERVICE_NONE, None)
    };
    match service {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}
```

**`set_key(key_id, data)`**：

1. 构造 schema 和 attributes：
   ```rust
   let schema = secret_schema_new(
       "com.assistant.vault.biometric",
       SECRET_SCHEMA_DONT_MATCH_NAME,
       [("account", SECRET_SCHEMA_ATTRIBUTE_STRING)],
   );
   let attributes = [("account", key_id)];
   ```
2. `secret_password_store_sync(schema, &attributes, key_id, data, SECRET_COLLECTION_DEFAULT, None)`
   - 成功 → `Ok(())`
   - 失败 → `VaultError::BiometricFailed`

**`get_key(key_id)`**：

1. `secret_password_lookup_sync(schema, &attributes, None)`
   - 成功 → `Ok(data.into_bytes())`
   - 未找到 → `VaultError::BiometricUnavailable`
   - keyring 锁定 → OS 弹 Polkit 认证框 → 认证后返回

**`delete_key(key_id)`**：

1. `secret_password_clear_sync(schema, &attributes, None)`
   - 未找到 → 静默返回 `Ok(())`

**`has_key(key_id)`**：

1. `secret_password_lookup_sync(schema, &attributes, None)` — 读取但不触发额外认证
   - 找到 → `Ok(true)`
   - 未找到 → `Ok(false)`

#### 8a.5.3 依赖

```toml
[target.'cfg(target_os = "linux")'.dependencies]
libsecret = "0.1"
```

#### 8a.5.4 手写 FFI 绑定备选方案

若 `libsecret` crate 不可用或质量不佳，手写 FFI：

```rust
#[link(name = "secret-1")]
extern "C" {
    fn secret_password_store_sync(
        schema: *const SecretSchema,
        collection: *const c_char,
        label: *const c_char,
        password: *const c_char,
        cancellable: *const c_void,
        error: *mut *mut GError,
        ...
    ) -> gboolean;

    fn secret_password_lookup_sync(
        schema: *const SecretSchema,
        cancellable: *const c_void,
        error: *mut *mut GError,
        ...
    ) -> *mut c_char;

    fn secret_password_clear_sync(
        schema: *const SecretSchema,
        cancellable: *const c_void,
        error: *mut *mut GError,
        ...
    ) -> gboolean;
}
```

> libsecret 使用可变参数（`...`）传递 attributes 键值对，FFI 绑定需特殊处理。建议优先使用 `libsecret` crate。

---

### 8a.6 错误映射

各平台 OS 错误码统一映射到 `VaultError`：

| OS 错误 | 平台 | 映射 |
|---------|------|------|
| `KeyCredentialStatus::CredentialAlreadyExists` | Windows | 先 delete 再 set（内部重试） |
| 用户取消 Windows Hello | Windows | `VaultError::BiometricCancelled` |
| TPM 签名失败 | Windows | `VaultError::BiometricFailed` |
| DPAPI 解密失败 | Windows | `VaultError::BiometricFailed` |
| `errSecUserCanceled` | macOS | `VaultError::BiometricCancelled` |
| `errSecAuthFailed` | macOS | `VaultError::BiometricFailed` |
| `errSecItemNotFound` | macOS | `VaultError::BiometricUnavailable` |
| D-Bus 不可用 | Linux | `VaultError::BiometricUnavailable` |
| Polkit 认证取消 | Linux | `VaultError::BiometricCancelled` |
| keyring 查找失败 | Linux | `VaultError::BiometricUnavailable` |

---

### 8a.7 安全不变式

| 不变式 | 验证方式 |
|--------|---------|
| Windows: 私钥不离开 TPM | 代码审查：仅使用 KeyCredentialManager API，不导出私钥 |
| Windows: DPAPI 加密文件绑定当前用户 | 代码审查：使用 WinRT DataProtectionProvider("LOCAL=user")，无 LOCAL_MACHINE 标志 |
| Windows: get_key 必须经过 TPM 签名门控 | 代码审查：get_key 先 RequestSignAsync 再读文件 |
| macOS: Keychain 条目受 kSecAccessControlBiometryCurrentSet 保护 | 代码审查：set_key 必须指定 AccessControl |
| macOS: 条目不同步到 iCloud | 代码审查：kSecAttrSynchronizable = false |
| Linux: 数据存入默认 keyring（用户会话级） | 代码审查：SECRET_COLLECTION_DEFAULT |
| 所有平台: get_key 触发 OS 认证 | 代码审查：get_key 调用 OS API（非文件直读） |
| 所有平台: has_key 不触发 OS 认证 | 代码审查：has_key 使用存在性检查（非数据读取） |
| 所有平台: master_key 不经 IPC | 已验证（上层逻辑不变） |

---

### 8a.8 跨阶段约束

| 约束 | 说明 |
|------|------|
| 条件编译 | 3 个平台文件通过 `#[cfg(target_os = "...")]` 隔离，Cargo.toml 依赖通过 `[target.'cfg(...)'.dependencies]` 条件引入 |
| trait 不变 | `BiometricKeyStore` trait 签名不变，仅替换 impl |
| 上层逻辑不变 | enroll/unlock/disenroll/自愈/Tauri Commands/前端代码零修改 |
| 编译隔离 | Windows 上不编译 macos/linux 代码，反之亦然 |
| 新增依赖仅平台生效 | `windows` / `core-foundation`+`security-framework` / `libsecret` 仅对应平台编译时引入 |
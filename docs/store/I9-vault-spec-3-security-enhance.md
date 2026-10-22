# I9-3 — 密码管理箱：安全增强与运行时守护

> 本文档为 I9 密码管理箱规格的 **第 3 部分**，覆盖 P1-D（TOTP + HIBP + 密码强度）、P1-E（自动锁定 + 剪贴板守护 + CSP）、P1-F（生物识别快速解锁）。
> 前置文档：[I9-vault-spec-1-foundation.md](./I9-vault-spec-1-foundation.md)（P1-A 安全基础）、[I9-vault-spec-2-crud-and-ui.md](./I9-vault-spec-2-crud-and-ui.md)（P1-B/C 数据交互与前端）
> 设计文档：[VAULT-DESIGN.md](../VAULT-DESIGN.md)
> 任务清单：[I9-vault-tasklist.md](./I9-vault-tasklist.md)

---

## 6. P1-D — TOTP + HIBP + 密码强度

### 6.1 交付目标

实现 TOTP 两步验证生成、HIBP k-匿名泄露检测、主密码强度校验、Diceware 密码短语生成。

### 6.2 TOTP

| 接口 | 说明 |
|------|------|
| `vault_totp_generate(id) → string` | Rust 端解密 totpSecret + 生成 6/8 位验证码，totpSecret 不出 Rust |
| `vault_totp_time_remaining(period) → u32` | 剩余有效秒数 |
| `vault_totp_parse_uri(uri) → { secret, digits, period, algorithm }` | 解析 `otpauth://` URI，提取密钥和参数 |

TOTP 算法支持：SHA1（默认，兼容性最广）/ SHA256 / SHA512（参考 KeePassXC `Totp.h` `Algorithm` 枚举）。6/8 位数字 + 自定义步长（默认 30s）+ Steam Guard 5 位编码。

前端：圆形倒计时环 + 6 位数字 + 一键复制，每秒刷新。

**参考实现**：KeePassXC `src/core/Totp.cpp` + `src/core/Totp.h` — `Settings` struct 含 `key` / `digits` / `step` / `algorithm` / `encoder`；`DEFAULT_STEP = 30` / `DEFAULT_DIGITS = 6` / `STEAM_DIGITS = 5`；支持 `otpauth://` URI 解析（`fromOtpauthUri`）和 KeePass2 格式（`fromKeePass2Totp`）。

### 6.3 HIBP

| 接口 | 说明 |
|------|------|
| `vault_hibp_check(id) → u32` | 单条 k-匿名校验 |
| `vault_hibp_audit() → Vec<(id, count)>` | 全库审计（逐条解密→检测→立即 zeroize） |

k-匿名流程：`SHA1(password) → 前5位hex → GET /range/{prefix} → 本地匹配后缀`

安全要点：
- **k-匿名**：只发 SHA-1 前 5 位（~20 bit），HIBP 返回该前缀下所有后缀，本地匹配
- **完整哈希不离境**：后 35 位仅在本地比较，HIBP 服务器无法知道具体查了哪个密码
- **Add-Padding**：请求头 `Add-Padding: true`，HIBP 返回随机填充行，防流量分析（参考 KeePassXC 实现）
- **User-Agent**：HIBP API 要求设置 User-Agent（KeePassXC 用 `"KeePassXC"`），我们用 `"Assistant/Vault"` 格式
- **逐条解密**：审计时同一时刻内存中只有一个明文密码，用完立即 zeroize

**参考实现**：KeePassXC `src/networking/HibpDownloader.cpp` — `add(password)` 添加待检密码 → `validate()` 逐条 SHA1 → k-匿名范围查询 `https://api.pwnedpasswords.com/range/{prefix}` → `fetchReadyRead` / `fetchFinished` 处理响应 → `pwnCount()` 本地匹配后缀提取泄露次数。

### 6.4 主密码强度

- 初始化/变更时用 `zxcvbn-rs` crate 评估，拒绝评分低于 "fair" 的主密码
- 前端实时显示强度条 + 破解时间估算
- 参考 KeePassXC `src/core/PasswordHealth.h` — `PasswordHealth(entropy)` 构造 → `score()` 返回评分 → `scoreReason()` 返回文字描述 → `adjustScore(amount)` 调整评分

### 6.5 Diceware 密码短语

扩展现有 `password-generator.service.ts`：新增 `generatePassphrase(wordCount, separator, capitalize)`，从 EFF 大词表（[efflargewordlist.txt](https://www.eff.org/deeplinks/2016/07/new-wordlists-random-passphrases)，7776 词，每个词提供 ~12.9 bit 熵）随机取词拼接。纯前端实现。词表内嵌为 JSON 常量（~50KB gzip 后 ~15KB），不运行时下载。

### 6.6 验收标准

- [ ] TOTP 生成正确（与 Google Authenticator 对比）
- [ ] TOTP `otpauth://` URI 解析正确
- [ ] HIBP k-匿名：只发 SHA-1 前 5 位，完整哈希不离境
- [ ] HIBP Add-Padding header 发送
- [ ] 弱主密码被拒绝（zxcvbn-rs 评分 < fair）
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

### 7.3 屏幕锁监听

**ScreenLockListener trait**（`screen_lock/mod.rs`）：

```rust
pub trait ScreenLockListener: Send + Sync {
    fn spawn(&self, callback: Box<dyn Fn() + Send + Sync>) -> Result<()>;
    fn stop(&self) -> Result<()>;
}

pub fn spawn_listener(callback: Box<dyn Fn() + Send + Sync>) -> Result<()> {
    #[cfg(target_os = "windows")]
    return windows::spawn(callback);
    #[cfg(target_os = "macos")]
    return macos::spawn(callback);
    #[cfg(target_os = "linux")]
    return linux::spawn(callback);
}
```

> `stop()` 用于取消注册系统通知（Windows: `WTSUnRegisterSessionNotification` + `UnregisterPowerSettingNotification`；macOS: 移除 Darwin 通知回调；Linux: 关闭 DBus 监听），参考 KeePassXC `ScreenLockListenerWin::~ScreenLockListenerWin()` 析构函数。
>
> **验收暂缓**：三平台代码及 `#[cfg(target_os)]` 条件编译均正常实现，macOS / Linux 验收暂缓（当前仅 Windows 可测试）。

**平台实现**：

| 平台 | 监听机制 | 关键 API |
|------|---------|---------|
| Windows | `WTSRegisterSessionNotification` + `RegisterPowerSettingNotification` | `WM_WTSSESSION_CHANGE`（`WTS_SESSION_LOCK`）+ `WM_POWERBROADCAST`（`PBT_APMSUSPEND`）+ `GUID_LIDSWITCH_STATE_CHANGE` |
| macOS | Darwin 通知 | `com.apple.screenIsLocked` / `com.apple.screenIsUnlocked`（`notify_dispatch`） |
| Linux | DBus | logind `PrepareForSleep`（`org.freedesktop.login1.Manager`）+ GNOME Session `StatusChanged` |

**参考实现**：KeePassXC `src/gui/osutils/` —
- Windows: `winutils/ScreenLockListenerWin.cpp` — `WTSRegisterSessionNotification` + `RegisterPowerSettingNotification`，`nativeEventFilter` 处理 `WM_WTSSESSION_CHANGE` / `WM_POWERBROADCAST`
- macOS: `macutils/ScreenLockListenerMac.cpp` — Darwin 通知中心 `com.apple.screenIsLocked`
- Linux: `nixutils/ScreenLockListenerDBus.cpp` — DBus `org.freedesktop.login1.Manager` `PrepareForSleep`

### 7.4 剪贴板 Rust 线程守护

ClipboardGuard 为 Tauri State 管理的运行时守卫，确保密码复制后剪贴板自动恢复：

| 行为 | 说明 |
|------|------|
| `copy_and_guard(password, timeout)` | 保存当前剪贴板内容 → 写入密码 → spawn 守护线程 → sleep timeout → 校验剪贴板仍为密码 → 恢复原内容 |
| 条件恢复 | 仅当剪贴板内容仍为密码时恢复，不覆盖用户新复制的内容 |
| `force_clear()` | 立即恢复原剪贴板内容（不等待 timeout） |
| 退出钩子 | 应用关闭时调用 `force_clear()`，确保剪贴板不留密码 |
| 默认超时 | 30s（由 `vaultClipboardClearSeconds` 设置项控制） |
| 线程安全 | 内部使用 `Mutex<Option<String>>` 保护原剪贴板内容 + `Option<JoinHandle<()>>` 管理守护线程 |

**剪贴板恢复场景**（与设计文档 §6.5 一致，参考 prs `clipboard.rs` + `clip_revert.rs`）：

| 场景 | 守护线程 | 退出钩子（`force_clear`） | prs 子进程 |
|------|:-------:|:------------------------:|:---------:|
| 正常超时 | ✅ | - | ✅ |
| 用户关闭应用 | - | ✅ | ✅ |
| 应用 panic | ✅（线程存活） | - | ✅ |
| `kill -9` | ❌ | ❌ | ✅ |

> `kill -9` 场景下攻击者已有系统级权限（可直接读 `/proc/pid/mem`），剪贴板安全已无意义。守护线程 + 退出钩子覆盖 99.99% 场景，无需 prs 的子进程方案（需额外 helper binary）。

### 7.5 CSP 配置

```
default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' asset: data: blob:; font-src 'self' data:; connect-src 'self' https://ipinfo.io https://api.pwnedpasswords.com; object-src 'none'; base-uri 'self'; form-action 'self'
```

> **`style-src 'unsafe-inline'` 说明**：Ant Design 组件库运行时注入 `<style>` 标签实现主题定制，`unsafe-inline` 为此必需。`unsafe-inline` 仅影响样式注入，不构成 XSS 向量（XSS 需要 `script-src unsafe-inline`，此处 `script-src` 严格为 `'self'`）。
>
> **`connect-src` 说明**：`https://ipinfo.io` 用于 IP 查询工具；`https://api.pwnedpasswords.com` 用于 HIBP k-匿名泄露检测（§6.3）。Tauri IPC 由 `'self'` 覆盖。
>
> ⚠️ 项目当前 `tauri.conf.json` 中 `"csp": null`（未启用），需在 P1-E 阶段设置为此严格策略。

### 7.6 DevTools 禁用

release 构建时禁用 DevTools（`cfg(not(debug_assertions))` 条件编译），debug 构建保留以便开发调试。

### 7.7 独立解锁窗口

独立 WebviewWindow 用于主密码输入，与主窗口 V8 isolate 隔离：

| 属性 | 值 |
|------|---|
| 窗口标签 | `"vault-unlock"` |
| 尺寸 | 400×200，居中 |
| 可调整大小 | 否 |
| DevTools | 禁用 |
| CSP | 同 §7.5 严格策略 |
| JS 代码量 | <50 行，无第三方依赖 |
| 生命周期 | 已存在时 focus 而非新建；解锁成功后 `window.close()` |
| 与主窗口通信 | 解锁成功后通过 Tauri event `vault:unlocked` 通知主窗口刷新状态 |

### 7.8 验收标准

- [ ] Windows 屏幕锁/睡眠触发 vault 自动锁定
- [ ] ~~macOS 屏幕锁触发 vault 自动锁定~~（验收暂缓）
- [ ] ~~Linux 屏幕锁触发 vault 自动锁定~~（验收暂缓）
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

> **核心安全约束**（与设计文档 §3.5.1 一致）：生物识别**不能**只返回 bool。如果 `authenticate() → true` 后用无保护的密钥去解密 `biometric_key_enc`，任何进程都能绕过生物识别直接读取密钥。正确做法：**OS 密钥管理器中的密钥受生物识别保护，读取时 OS 自动弹出提示，通过后才释放密钥**。任何进程无法绕过生物识别直接读取受保护数据。

### 8.2 BiometricKeyStore trait

**`biometric/mod.rs`**：

```rust
use crate::types::VaultError;

/// 平台抽象接口：生物识别门控密钥释放。
///
/// 约束 `Send + Sync`：Tauri State 跨线程要求。
/// `key_id` 为逻辑标识（如 "vault-master-key"），各平台实现自行映射到
/// OS 密钥管理器的条目名/属性。
pub trait BiometricKeyStore: Send + Sync {
    /// 检测平台是否支持生物识别。
    ///
    /// Windows: `KeyCredentialManager::IsSupportedAsync()`
    /// macOS:   检查 `kSecAccessControlBiometryCurrentSet` 可用性
    /// Linux:   始终返回 `true`（libsecret 可用即视为支持，降级为系统密码）
    fn is_available(&self) -> Result<bool, VaultError>;

    /// 将数据存入 OS 密钥管理器，受生物识别保护。
    ///
    /// 调用时 OS 弹出生物识别确认（Windows Hello / Touch ID / Polkit），
    /// 用户通过后才写入。若用户取消或识别失败，返回 `VaultError::BiometricCancelled`。
    fn set_key(&self, key_id: &str, data: &[u8]) -> Result<(), VaultError>;

    /// 从 OS 密钥管理器读取受保护的数据。
    ///
    /// 调用时 OS 弹出生物识别提示，用户通过后才释放数据。
    /// 若用户取消或识别失败，返回 `VaultError::BiometricCancelled`。
    /// 若条目不存在，返回 `VaultError::NotFound`。
    fn get_key(&self, key_id: &str) -> Result<Vec<u8>, VaultError>;

    /// 从 OS 密钥管理器删除条目。
    ///
    /// 幂等操作：条目不存在时返回 `Ok(())`。
    fn delete_key(&self, key_id: &str) -> Result<(), VaultError>;

    /// 检查 OS 密钥管理器中是否存在指定条目。
    ///
    /// **不触发**生物识别提示（仅检查存在性，不读取数据）。
    /// 用于自愈机制的一致性检测。
    fn has_key(&self, key_id: &str) -> Result<bool, VaultError>;
}

/// 工厂函数：根据编译目标平台返回对应实现。
pub fn create_key_store() -> Box<dyn BiometricKeyStore> {
    #[cfg(target_os = "windows")]
    return Box::new(windows::WindowsHelloKeyStore);
    #[cfg(target_os = "macos")]
    return Box::new(macos::TouchIdKeyStore);
    #[cfg(target_os = "linux")]
    return Box::new(linux::LibsecretKeyStore);
}
```

> **VaultError 新增变体**：`BiometricUnavailable` / `BiometricCancelled` / `BiometricFailed(String)`，用于区分"平台不支持"、"用户取消"、"识别失败"三种情况，前端据此决定是否回退主密码。

### 8.3 平台实现

> **存储架构分两层**：
> - **第 1 层（OS 密钥管理器）**：受生物识别保护的密钥，读取时 OS 自动弹出提示
> - **第 2 层（文件系统 `biometric-key.enc`）**：AES-256-GCM 加密后的 master_key 密文，存于 `{app_data_dir}/security/biometric-key.enc`（与 `security.db` 同目录）
>
> `biometric-key.enc` **必须存文件系统**而非 `vault_meta`：生物识别解锁时数据库仍锁定，无法读取 `vault_meta`，存在循环依赖。文件系统存储在数据库打开前即可读取。
>
> **macOS 例外**：Keychain + `kSecAccessControlBiometryCurrentSet` 可直接保护 master_key，macOS 采用**单层方案**，无需 `biometric-key.enc` 文件。

#### 8.3.1 Windows Hello（`biometric/windows.rs`）

**两层方案**：TPM 密钥对（第 1 层）+ `biometric-key.enc`（第 2 层）。

**密钥存储**：TPM `KeyCredentialManager` — TPM 生成非对称密钥对，私钥永不离开 TPM。

**门控方式**：`RequestSignAsync(challenge)` — 签名操作受 Windows Hello 保护，OS 弹 PIN/指纹/面部识别提示。

**`set_key(key_id, data)` 流程**：

1. `KeyCredentialManager::IsSupportedAsync()` — 检查 TPM + Windows Hello 可用
2. 生成随机 12B `challenge`（同时用作 AES-256-GCM nonce）
3. `KeyCredentialManager::OpenAsync(key_id)` — 打开已有 TPM 密钥对
   - 若不存在 → `RequestCreateAsync(key_id, FailIfExists)` 创建新密钥对 → OS 弹 Windows Hello 确认
4. `credential.RequestSignAsync(challenge)` — OS 弹 Windows Hello → 通过后返回 `signature`
5. `encryption_key = SHA256(signature)` — 从 TPM 签名派生对称加密密钥
6. `encrypted = AES-256-GCM(key=encryption_key, nonce=challenge, plaintext=data)`
7. 存储 `(challenge, encrypted)` 到 `vault_meta`（challenge 非秘密，仅为下次签名输入；此处 `data` 为 `wrapping_key`，加密后的 master_key 另存 `biometric-key.enc`）
8. **zeroize** `encryption_key`

**`get_key(key_id)` 流程**：

1. 读取存储的 `(challenge, encrypted)`（来自 `vault_meta`，此时数据库已通过 bootstrap_key 打开）
2. `KeyCredentialManager::OpenAsync(key_id)` → `credential.RequestSignAsync(challenge)` — OS 弹 Windows Hello
3. `encryption_key = SHA256(signature)` — 派生同一加密密钥
4. `data = AES-256-GCM-Decrypt(key=encryption_key, nonce=challenge, ciphertext=encrypted)`
5. **zeroize** `encryption_key`
6. 返回 `data`

> **关键**：加密密钥由 TPM 签名派生，签名操作受 Windows Hello 门控。没有 Windows Hello 通过 → 无法获取签名 → 无法派生加密密钥 → 无法解密 `encrypted`。任何进程无法绕过 Windows Hello 直接获取 `encryption_key`。

**Rust 依赖**：`windows` crate（WinRT `Windows::Security::Credentials::UI` + `Windows::Storage::Streams`）

**参考实现**：KeePassXC `src/quickunlock/WindowsHello.cpp` — `deriveEncryptionKey()` + `setKey()` + `getKey()` + `queueSecurityPromptFocus()`（`FindWindowA("Credential Dialog Xaml Host")` + `SetForegroundWindow` 自动聚焦 Windows Hello 弹窗）

#### 8.3.2 macOS Touch ID（`biometric/macos.rs`）

**单层方案**：master_key 直接存 Keychain，**无需 `biometric-key.enc` 文件**。

**密钥存储**：Keychain `kSecClassGenericPassword` 条目。

**门控方式**：`kSecAccessControlBiometryCurrentSet` — 条目创建/读取时 OS 弹 Touch ID 提示。

**`set_key(key_id, data)` 流程**：

1. 构造 `SecAccessControl`：`SecAccessControlCreateWithFlags(kSecAttrAccessibleWhenUnlockedThisDeviceOnly, kSecAccessControlBiometryCurrentSet)`
   - `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`：仅设备解锁后可访问，不迁移到新设备
   - `kSecAccessControlBiometryCurrentSet`：当前已注册的生物识别（指纹/面部）变更后条目失效
2. 构造 `SecItemAdd` 属性字典：
   - `kSecClass`: `kSecClassGenericPassword`
   - `kSecAttrService`: `"assistant-vault"`
   - `kSecAttrAccount`: `key_id`
   - `kSecValueData`: `data`（**macOS 上 `data` 即为 master_key 本身**，非 wrapping_key）
   - `kSecAttrAccessControl`: 上一步的 access control
3. `SecItemAdd(attrs)` — OS 弹 Touch ID 确认 → 通过后写入 Keychain
4. 若条目已存在（`errSecDuplicateItem`）→ `SecItemUpdate` 更新

**`get_key(key_id)` 流程**：

1. 构造 `SecItemCopyMatching` 查询字典：
   - `kSecClass`: `kSecClassGenericPassword`
   - `kSecAttrService`: `"assistant-vault"`
   - `kSecAttrAccount`: `key_id`
   - `kSecReturnData`: `true`
   - `kSecUseOperationPrompt`: `"解锁密码管理箱"`（Touch ID 提示文案）
2. `SecItemCopyMatching(query)` — OS 弹 Touch ID → 通过后返回 `data`（**即 master_key**）
3. 用户取消 → `errSecUserCanceled` → 返回 `VaultError::BiometricCancelled`

> **macOS 单层优势**：
> - **无 `biometric-key.enc` 文件** → 无两层不一致问题，无需自愈机制
> - **无 wrapping_key 中间层** → 减少一次 AES-GCM 加解密，流程更简洁
> - Keychain 条目本身受 `kSecAccessControlBiometryCurrentSet` 保护，`SecItemCopyMatching()` 读取时 OS 自动弹 Touch ID，通过后直接返回明文数据。任何进程无法绕过 Touch ID 直接读取该条目。
>
> **行业实践**：KeePassXC macOS 版同样采用单层方案，master_key 直接存 Keychain。

**Rust 依赖**：`core-foundation-rs`（`CFDictionary` / `CFString` / `CFData`）+ `security-framework`（`SecItemAdd` / `SecItemCopyMatching` / `SecItemDelete` / `SecAccessControl`）

**参考实现**：KeePassXC `src/quickunlock/TouchID.mm` — `isAvailable()` 检查 `kSecAccessControlBiometryCurrentSet` + `canEvaluatePolicy(error:)`；`setKey()` 用 `SecItemAdd` + `kSecAttrAccessControl`；`getKey()` 用 `SecItemCopyMatching` + `kSecUseOperationPrompt`

#### 8.3.3 Linux Polkit（`biometric/linux.rs`）

**两层方案**：libsecret（第 1 层）+ `biometric-key.enc`（第 2 层）。

**密钥存储**：`libsecret`（Secret Service API / D-Bus）。

**门控方式**：**无 OS 级生物识别门控**。`libsecret` 存取密钥时不弹生物识别提示，安全性依赖用户登录会话（解锁 keyring 时已输入用户密码）。

**`set_key(key_id, data)` 流程**：

1. `secret_password_store_sync(schema, attributes, label, password, collection)` — 存入 Secret Service
   - `schema`: 自定义 `assistant_vault` schema
   - `attributes`: `{ "key-id": key_id }`
   - `collection`: `SECRET_COLLECTION_DEFAULT`（默认 keyring）
2. 首次存入时 Secret Service 可能弹 Polkit 对话框要求授权（取决于桌面环境配置）

**`get_key(key_id)` 流程**：

1. `secret_password_lookup_sync(schema, attributes)` — 从 Secret Service 读取
2. 若 keyring 已解锁（用户登录时自动解锁）→ 直接返回，**无额外提示**
3. 若 keyring 锁定 → Polkit 弹框要求输入用户密码解锁

> **Linux 特殊情况**：`libsecret` 不支持生物识别门控，Linux 上生物识别快速解锁降级为"系统密码解锁"（Polkit 弹框输入用户密码）。若用户需要生物识别，需自行配置 `pam-fprint-gtk`（PAM 指纹模块），使系统登录/Polkit 认证使用指纹。`set_key()` / `get_key()` 在 Linux 上使用 `libsecret` 存取密钥，但**无 OS 级生物识别保护**——安全性依赖用户登录会话。
>
> **可用性检测**：`is_available()` 检查 Secret Service daemon 是否运行（`libsecret::secret_service_connect_sync` 是否成功）。无 daemon 时返回 `false`，前端隐藏生物识别按钮。

**Rust 依赖**：`libsecret` crate（`secret_password_store_sync` / `secret_password_lookup_sync` / `secret_password_clear_sync`）

> **验收暂缓**：三平台代码及 `#[cfg(target_os)]` 条件编译均正常实现，macOS / Linux 验收暂缓（当前仅 Windows 可测试）。

#### 8.3.4 平台方案汇总

| 平台 | 方案 | 第 1 层（OS 凭证） | 第 2 层（文件系统） | 自愈需求 |
|------|------|-------------------|-------------------|:---:|
| **Windows** | 两层 | TPM 密钥对 → 签名 → `wrapping_key` | `biometric-key.enc` = AES-GCM(`wrapping_key`, `master_key`) | ✅ |
| **macOS** | 单层 | Keychain → `master_key`（受 Touch ID 保护） | 无 | ❌ |
| **Linux** | 两层 | libsecret → `wrapping_key` | `biometric-key.enc` = AES-GCM(`wrapping_key`, `master_key`) | ✅ |

#### 8.3.5 自愈机制（Windows / Linux）

两层方案存在 OS 凭证与 `biometric-key.enc` 不一致的风险（系统重装、OS 凭证被清理、用户手动删除文件等）。**每次 vault 成功解锁后**执行一致性检测并自动修复：

```rust
fn check_biometric_consistency(&self) -> Result<()> {
    // macOS 单层方案无需自愈
    #[cfg(target_os = "macos")]
    return Ok(());

    let has_os_key = self.biometric_store.has_key(KEY_ID)?;
    let has_enc_file = self.biometric_key_path.exists();

    match (has_os_key, has_enc_file) {
        (true, true)  => {} // 正常，无需操作
        (true, false) => {
            // OS 凭证在但密文文件丢失 → 用当前 master_key 重新生成
            self.write_biometric_key_enc(&self.master_key)?;
        }
        (false, true) => {
            // 密文文件在但 OS 凭证丢失 → 删密文，标记未注册
            fs::remove_file(&self.biometric_key_path)?;
            self.set_biometric_enrolled(false)?;
        }
        (false, false) => {} // 未注册，无需操作
    }
    Ok(())
}
```

> **安全保证**：`check_biometric_consistency` 仅在 vault 已解锁（`master_key` 可用）时调用。`has_key()` 不触发生物识别提示（仅检查条目存在性，不读取数据）。

### 8.4 Enrollment 流程（启用生物识别）

用户在 VaultSettings 中开启 `vaultBiometricEnabled` 或首次解锁后点击"启用生物识别"时触发：

**前置检查**（所有平台通用）：
1. vault 已解锁（`is_locked() == false`），否则返回 `VaultError::Locked`
2. `BiometricKeyStore::is_available() == true`，否则返回 `VaultError::BiometricUnavailable`
3. 未已启用（已启用则先 disenroll 再重新 enroll）

#### 8.4.1 macOS 单层 Enrollment

1. `BiometricKeyStore::set_key("vault-master-key", master_key_bytes)` — **OS 弹 Touch ID 确认**
   - master_key 直接存入 Keychain，受 `kSecAccessControlBiometryCurrentSet` 保护
2. 用户通过 → 继续；用户取消 → 返回 `VaultError::BiometricCancelled`
3. **审计日志**：`audit_log(conn, AuditEvent::BiometricEnrolled, None)`
4. **无需 `biometric-key.enc` 文件**，无需 zeroize wrapping_key

#### 8.4.2 Windows / Linux 两层 Enrollment

1. **生成随机包装密钥**：`wrapping_key = rand::[u8; 32]`（32 字节随机密钥，用于加密 master_key）
2. **加密 master_key**：
   - `nonce = rand::[u8; 12]`
   - `encrypted_master_key = AES-256-GCM(key=wrapping_key, nonce=nonce, plaintext=master_key_bytes)`
   - `biometric_key_enc = base64(nonce || encrypted_master_key)`
3. **存储 wrapping_key 到 OS 密钥管理器**：
   - `BiometricKeyStore::set_key("vault-master-key", wrapping_key)` — **OS 弹生物识别确认**
   - 用户通过 → 继续；用户取消 → **回滚**：zeroize `wrapping_key`，不写入文件，返回 `VaultError::BiometricCancelled`
4. **写入 `biometric-key.enc` 文件**：将 `biometric_key_enc` 写入 `{app_data_dir}/security/biometric-key.enc`
5. **审计日志**：`audit_log(conn, AuditEvent::BiometricEnrolled, None)`
6. **zeroize** `wrapping_key` / `nonce` / `master_key_bytes` 副本

> **安全要点**：
> - `wrapping_key` 仅存在于 OS 密钥管理器中（受生物识别保护），不写入任何文件
> - `biometric-key.enc` 是密文，即使被提取也无法解密（缺少 `wrapping_key`）
> - master_key 本身不直接存入 OS 密钥管理器——中间层 `wrapping_key` 确保：即使 OS 密钥管理器被攻破（如 root 访问 Keychain），攻击者获取 `wrapping_key` 后仍需从文件系统读取 `biometric-key.enc` 才能解密 master_key
> - 启用后 **不改变** 主密码解锁路径——主密码输入仍可正常解锁
> - `biometric-key.enc` 存文件系统而非 `vault_meta`：enrollment 时 vault 已解锁可写 `vault_meta`，但解锁时 vault 锁定无法读 `vault_meta` → 循环依赖。文件系统存储在数据库打开前即可读取

### 8.5 解锁流程（生物识别快速解锁）

用户在 VaultUnlock 中点击生物识别按钮时触发：

**前置检查**（所有平台通用）：
1. vault 已锁定（`is_locked() == true`）
2. `BiometricKeyStore::is_available() == true`

#### 8.5.1 macOS 单层解锁

1. `master_key_bytes = BiometricKeyStore::get_key("vault-master-key")` — **OS 弹 Touch ID 提示**
   - 用户通过 → 继续；用户取消 → 返回 `VaultError::BiometricCancelled`
2. **恢复 vault 状态**：
   - `master_key = MasterKey::new(master_key_bytes)`
   - 用 `master_key` 派生 `sqlcipher_key` → 打开 `security.db` 连接
   - 存入 `SecurityDbState` 的 `conn` + `master_key`
3. **审计日志**：`audit_log(conn, AuditEvent::VaultUnlock, Some("biometric"))`
4. **zeroize** `master_key_bytes` 副本

#### 8.5.2 Windows / Linux 两层解锁

1. **前置检查**：`biometric-key.enc` 文件存在（已 enroll）
2. **释放 wrapping_key**：
   - `wrapping_key = BiometricKeyStore::get_key("vault-master-key")` — **OS 弹生物识别提示**
   - 用户通过 → 继续；用户取消 → 返回 `VaultError::BiometricCancelled`，前端回退主密码输入
3. **解密 master_key**：
   - 读取 `{app_data_dir}/security/biometric-key.enc` 文件内容
   - base64 decode → 提取 `nonce`（前 12B）+ `encrypted_master_key`
   - `master_key_bytes = AES-256-GCM-Decrypt(key=wrapping_key, nonce=nonce, ciphertext=encrypted_master_key)`
   - 解密失败（AEAD tag 不匹配）→ 返回 `VaultError::Crypto("biometric key decryption failed")`，可能原因：`wrapping_key` 被篡改 / `biometric-key.enc` 被篡改 / enroll 后 master_key 变更（主密码变更）但未重新 enroll
4. **恢复 vault 状态**：
   - `master_key = MasterKey::new(master_key_bytes)`
   - 用 `master_key` 派生 `sqlcipher_key` → 打开 `security.db` 连接
   - 存入 `SecurityDbState` 的 `conn` + `master_key`
5. **自愈检测**：`check_biometric_consistency()` — 修复 OS 凭证与 `biometric-key.enc` 不一致
6. **审计日志**：`audit_log(conn, AuditEvent::VaultUnlock, Some("biometric"))`
7. **zeroize** `wrapping_key` / `master_key_bytes` 副本

> **master_key 不经 IPC**：整个流程在 Rust 端完成，`vault_unlock_biometric` 返回 `Result<bool>`，master_key 仅存于 `SecurityDbState` 内存中，不返回给前端 JS 堆。
>
> **主密码变更后失效**：`vault_change_master_password` 会 zeroize 旧 master_key 并生成新 master_key，但 `biometric-key.enc` 仍加密旧 master_key。因此主密码变更后必须自动调用 `vault_disenroll_biometric()` 清除旧条目，并在前端提示用户重新启用生物识别。变更流程新增步骤：验证新密码 → WAL checkpoint → 备份 → PRAGMA rekey → 更新 vault_meta → **disenroll_biometric()** → 验证 → 失败回滚。

### 8.6 Disenrollment 流程（禁用生物识别）

用户在 VaultSettings 中关闭 `vaultBiometricEnabled` 或主密码变更后自动触发：

1. **删除 OS 密钥管理器条目**：`BiometricKeyStore::delete_key("vault-master-key")` — 幂等，条目不存在也返回 `Ok(())`
2. **删除 `biometric-key.enc` 文件**（Windows / Linux）：`fs::remove_file("{app_data_dir}/security/biometric-key.enc")` — macOS 无此文件，跳过
3. **审计日志**：`audit_log(conn, AuditEvent::VaultLock, Some("biometric_disenrolled"))`
4. **结果**：OS 凭证已删除 + `biometric-key.enc` 已删除 → 下次解锁直接要求主密码输入

> **无需 zeroize**：`biometric-key.enc` 是密文（加密后的 master_key），删除即可。OS 密钥管理器中的密钥由 `delete_key` 删除，OS 负责清理。

### 8.7 Tauri Commands

| Command | 签名 | 说明 | 审计事件 |
|---------|------|------|---------|
| `vault_is_biometric_available` | `() → Result<bool>` | 检测平台是否支持生物识别（前端据此显示/隐藏生物识别按钮） | — |
| `vault_is_biometric_enrolled` | `() → Result<bool>` | 检测是否已启用生物识别（OS 凭证存在 + `biometric-key.enc` 文件存在 / macOS 仅检查 Keychain 条目） | — |
| `vault_unlock_biometric` | `() → Result<bool>` | 生物识别解锁（Rust 端完成全部流程，master_key 不经 IPC） | `VaultUnlock("biometric")` / `VaultUnlockFailed` |
| `vault_enroll_biometric` | `(masterPassword: String) → Result<()>` | 启用生物识别：验证主密码 → 派生 master_key → §8.4 enrollment 流程 | `BiometricEnrolled` |
| `vault_disenroll_biometric` | `() → Result<()>` | 禁用生物识别：§8.6 disenrollment 流程 | `VaultLock("biometric_disenrolled")` |

**错误码**：

| 错误 | 触发场景 | 前端行为 |
|------|---------|---------|
| `VaultError::Locked` | vault 未解锁时调用 enroll | 提示先解锁 |
| `VaultError::BiometricUnavailable` | 平台不支持生物识别 | 隐藏生物识别按钮 |
| `VaultError::BiometricCancelled` | 用户取消 OS 生物识别提示 | 回退主密码输入，不显示错误 |
| `VaultError::BiometricFailed(msg)` | 生物识别失败（多次尝试未通过等） | 显示错误 + 回退主密码输入 |
| `VaultError::NotFound` | `get_key()` 时条目不存在（已 disenroll 或 OS 条目被外部删除） | 自动 disenroll + 回退主密码输入 |
| `VaultError::Crypto(msg)` | `biometric-key.enc` 解密失败（主密码变更后未重新 enroll） | 自动 disenroll + 提示重新启用 + 回退主密码输入 |

### 8.8 前端集成

#### 8.8.1 vaultStore 扩展

**新增状态字段**：

| 字段 | 类型 | 说明 |
|------|------|------|
| `biometricAvailable` | `boolean` | 平台是否支持生物识别（启动时检测一次） |
| `biometricEnrolled` | `boolean` | 是否已启用生物识别（解锁后检测 OS 凭证存在性 / `biometric-key.enc` 文件存在性） |

**新增 actions**：

| Action | 签名 | 说明 |
|--------|------|------|
| `checkBiometricAvailable` | `() → Promise<void>` | 调用 `vault_is_biometric_available` → 更新 `biometricAvailable` |
| `unlockBiometric` | `() → Promise<boolean>` | 调用 `vault_unlock_biometric` → 成功则 `isLocked = false`；`BiometricCancelled` 不设 error |
| `enrollBiometric` | `(masterPassword: string) → Promise<void>` | 调用 `vault_enroll_biometric` → 成功则 `biometricEnrolled = true` |
| `disenrollBiometric` | `() → Promise<void>` | 调用 `vault_disenroll_biometric` → 成功则 `biometricEnrolled = false` |

**初始化时机**：`VaultApp` mount 时调用 `checkBiometricAvailable()`；解锁成功后调用 `vault_is_biometric_enrolled`（Rust 端检测 OS 凭证存在性 + `biometric-key.enc` 文件存在性）→ 设置 `biometricEnrolled`。

#### 8.8.2 VaultUnlock 扩展

在主密码输入框下方新增生物识别按钮（条件显示）：

```
┌──────────────────────────┐
│      🔒 密码管理箱        │
│                          │
│  [  主密码输入框  ]       │
│                          │
│  [  解锁  ]              │  ← 主密码解锁
│                          │
│  ─── 或 ───              │  ← 仅 biometricAvailable && biometricEnrolled 时显示
│  [  🛂 生物识别解锁  ]   │  ← 调用 unlockBiometric()
│                          │
└──────────────────────────┘
```

- **显示条件**：`biometricAvailable === true && biometricEnrolled === true`
- **点击行为**：调用 `unlockBiometric()`
- **成功**：`isLocked = false`，VaultApp 切换到 VaultList
- **`BiometricCancelled`**：不显示错误，用户可再次点击或输入主密码
- **`BiometricFailed`**：显示错误提示 + "回退主密码输入"
- **`NotFound` / `Crypto`**：自动 disenroll + 提示"生物识别已失效，请重新启用" + 回退主密码

#### 8.8.3 VaultSettings 扩展

现有 `vaultBiometricEnabled` 开关（[VaultSettings.tsx](../../src/tools/password-vault/VaultSettings.tsx)）的语义细化：

| 操作 | 行为 |
|------|------|
| 开启 | 若 `biometricAvailable === false` → 提示"当前平台不支持生物识别"，开关保持关闭；否则 → 弹窗要求输入主密码 → `enrollBiometric(password)` → 成功则开关开启 |
| 关闭 | `disenrollBiometric()` → 成功则开关关闭 |

> **开关 ↔ enroll 状态同步**：`vaultBiometricEnabled`（设置项）与 `biometricEnrolled`（运行时状态）应保持一致。若检测到不一致（如 OS 条目被外部删除导致 `biometricEnrolled = false` 但设置项仍为 true），自动将 `vaultBiometricEnabled` 设为 false。

#### 8.8.4 vault.service.ts 扩展

```typescript
isBiometricAvailable: (): Promise<boolean> =>
  invoke("vault_is_biometric_available"),

unlockBiometric: (): Promise<boolean> =>
  invoke("vault_unlock_biometric"),

enrollBiometric: (masterPassword: string): Promise<void> =>
  invoke("vault_enroll_biometric", { masterPassword }),

disenrollBiometric: (): Promise<void> =>
  invoke("vault_disenroll_biometric"),
```

### 8.9 条件编译与依赖

**模块结构**（`assistant-security/src/biometric/`）：

```
biometric/
├── mod.rs        ← BiometricKeyStore trait + create_key_store() 工厂
├── windows.rs    ← #[cfg(target_os = "windows")] WindowsHelloKeyStore
├── macos.rs      ← #[cfg(target_os = "macos")] TouchIdKeyStore
└── linux.rs      ← #[cfg(target_os = "linux")] LibsecretKeyStore
```

**`lib.rs` 新增**：`pub mod biometric;`

**`mod.rs` 条件编译**：

```rust
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod linux;
```

**Cargo.toml 平台特定依赖**（`assistant-security/Cargo.toml`）：

```toml
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.58", features = ["Security_Credentials_UI", "Storage_Streams"] }

[target.'cfg(target_os = "macos")'.dependencies]
core-foundation = "0.10"
security-framework = "3.0"

[target.'cfg(target_os = "linux")'.dependencies]
libsecret = "0.1"
```

> **`sha2` 依赖复用**：Windows Hello 的 `SHA256(signature)` 使用 `sha2` crate，已在 P1-A 阶段引入（`vault_crypto` 使用），无需新增。

### 8.10 验收标准

- [ ] Windows Hello 弹窗 → 通过 → 解锁成功
- [ ] ~~Touch ID 弹窗 → 通过 → 解锁成功~~（验收暂缓）
- [ ] ~~Linux Polkit → 通过 → 解锁成功~~（验收暂缓）
- [ ] 生物识别不可用 → 前端隐藏生物识别按钮
- [ ] 生物识别解锁后 master_key 不经 IPC（`vault_unlock_biometric` 返回 `bool`，不返回密钥）
- [ ] `vault_enroll_biometric` 后 OS 密钥管理器有条目 + `biometric-key.enc` 文件存在（Windows/Linux）/ Keychain 条目存在（macOS）
- [ ] `vault_disenroll_biometric` 后生物识别条目已删除 + `biometric-key.enc` 文件已删除 + 无法再生物识别解锁
- [ ] 用户取消生物识别 → 回退主密码输入，不显示错误
- [ ] 主密码变更后自动 disenroll → 前端提示重新启用
- [ ] `biometric-key.enc` 解密失败（主密码变更后未重新 enroll）→ 自动 disenroll + 回退主密码
- [ ] macOS 单层：enroll 后无 `biometric-key.enc` 文件，解锁直接从 Keychain 获取 master_key
- [ ] 自愈：OS 凭证丢失 + `biometric-key.enc` 存在 → 解锁后自动 disenroll
- [ ] 自愈：OS 凭证存在 + `biometric-key.enc` 丢失 → 解锁后自动重建 `biometric-key.enc`
- [ ] `cargo check` 通过（三平台 `#[cfg(target_os)]` 条件编译）
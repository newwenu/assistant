# I9 — 密码管理箱（Password Vault）P1-F 遗留任务清单

> 规格：[I9-vault-spec-3a-biometric-platform.md](./I9-vault-spec-3a-biometric-platform.md)
> 设计：[VAULT-DESIGN.md](../../VAULT-DESIGN.md) §3.4
> 前置：P1-A ~ P1-E 完成，P1-F 上层逻辑已实现（仅平台文件为 stub）

---

## 阶段 F' — P1-F 遗留：生物识别平台真实实现

### F'1. Cargo.toml 平台特定依赖

**文件**：`src-tauri/crates/assistant-security/Cargo.toml`

- [x] F'1.1 新增 `[target.'cfg(target_os = "windows")'.dependencies]`：`windows = { version = "0.58", features = ["Foundation", "Security_Credentials", "Security_Cryptography", "Security_Cryptography_DataProtection", "Storage_Streams", "Win32_Foundation", "Win32_System_WinRT"] }`（已修正：`Security_Credentials` 非 `Security_Credentials_UI`，DPAPI 用 WinRT `DataProtectionProvider` 非 Win32 `CryptProtectData`）
- [x] F'1.2 新增 `[target.'cfg(target_os = "macos")'.dependencies]`：`core-foundation = "0.10"` + `security-framework = "3.0"`
- [x] F'1.3 新增 `[target.'cfg(target_os = "linux")'.dependencies]`：`libsecret = "0.1"`（手写 FFI 绑定实现）
- [x] F'1.4 workspace `Cargo.toml` 对应新增 workspace.dependencies（`windows` crate 已添加）

**依赖**：无

---

### F'2. Windows Hello 真实实现

**文件**：`src-tauri/crates/assistant-security/src/biometric/windows.rs`

- [x] F'2.1 `is_available()` — `KeyCredentialManager::IsSupportedAsync()?.get()` → `Ok(bool)`
- [x] F'2.2 `set_key(key_id, data)` — 先 `OpenAsync` 检查存在性，仅 `NotFound` 时 `RequestCreateAsync(FailIfExists)` + 弹 Windows Hello → `DataProtectionProvider("LOCAL=user").ProtectAsync(data)` → 写入 `%LOCALAPPDATA%\assistant\biometric\{key_id}.bin`
- [x] F'2.3 `set_key` 重复处理 — 先 Open 检查，已存在则跳过创建（避免不必要的 Delete+重建）
- [x] F'2.4 `get_key(key_id)` — `OpenAsync(key_id)` → `RequestSignAsync(challenge)` + 弹 Windows Hello → 读 .bin 文件 → `DataProtectionProvider.UnprotectAsync` → 返回 data
- [x] F'2.5 `delete_key(key_id)` — `DeleteAsync(key_id)` + 删除 .bin 文件（幂等）
- [x] F'2.6 `has_key(key_id)` — `OpenAsync(key_id)` 不触发认证 → `Ok(true/false)`
- [x] F'2.7 异步转同步辅助 — `wait_for_async<T>(op: IAsyncOperation<T>)` + `wait_for_action(op: IAsyncAction)`（DeleteAsync 返回 IAsyncAction）
- [x] F'2.8 错误映射 — 用户取消 → `BiometricCancelled` / 签名失败 → `BiometricFailed` / DPAPI 失败 → `BiometricFailed`
- [x] F'2.9 DPAPI 辅助 — WinRT `DataProtectionProvider("LOCAL=user").ProtectAsync` / `UnprotectAsync`（替代 Win32 `CryptProtectData`，自动内存管理）
- [x] F'2.10 WinRT 生命周期 — `WinRtGuard` RAII：`RoInitialize(RO_INIT_MULTITHREADED)` / `RoUninitialize`

**依赖**：F'1

---

### F'3. macOS Touch ID 真实实现

**文件**：`src-tauri/crates/assistant-security/src/biometric/macos.rs`

- [x] F'3.1 `is_available()` — `SecAccessControlCreateWithFlags(kSecAttrAccessibleWhenUnlockedThisDeviceOnly, kSecAccessControlBiometryCurrentSet)` → 非空即支持
- [x] F'3.2 `set_key(key_id, data)` — 创建 AccessControl → 构造 Keychain 查询字典（kSecClassGenericPassword + kSecAttrAccessControl + kSecAttrSynchronizable=false）→ `SecItemAdd` + 弹 Touch ID
- [x] F'3.3 `set_key` 重复处理 — `errSecDuplicateItem` → `SecItemDelete` → 重试 `SecItemAdd`
- [x] F'3.4 `get_key(key_id)` — `SecItemCopyMatching(kSecReturnData=true)` + 弹 Touch ID → 返回 data
- [x] F'3.5 `delete_key(key_id)` — `SecItemDelete`（`errSecItemNotFound` 静默返回 Ok）
- [x] F'3.6 `has_key(key_id)` — `SecItemCopyMatching(kSecReturnData=false)` 不触发 Touch ID → `Ok(true/false)`
- [x] F'3.7 错误映射 — `errSecUserCanceled` → `BiometricCancelled` / `errSecAuthFailed` → `BiometricFailed` / `errSecItemNotFound` → `BiometricUnavailable`
- [x] F'3.8 Core Foundation 类型辅助 — `CFDictionary` 构造 / `CFStringRef` 转换 / `CFDataRef` → `Vec<u8>` 提取

**依赖**：F'1

---

### F'4. Linux libsecret 真实实现

**文件**：`src-tauri/crates/assistant-security/src/biometric/linux.rs`

- [x] F'4.1 `is_available()` — `secret_service_get_sync()` 检查 D-Bus 可用性 → `Ok(bool)`
- [x] F'4.2 `set_key(key_id, data)` — 构造 schema（`com.assistant.vault.biometric`）+ attributes → `secret_password_store_sync` → `SECRET_COLLECTION_DEFAULT`
- [x] F'4.3 `get_key(key_id)` — `secret_password_lookup_sync` → 若 keyring 锁定则 Polkit 弹框 → 返回 data
- [x] F'4.4 `delete_key(key_id)` — `secret_password_clear_sync`（未找到静默返回 Ok）
- [x] F'4.5 `has_key(key_id)` — `secret_password_lookup_sync` → 找到 `Ok(true)` / 未找到 `Ok(false)`
- [x] F'4.6 错误映射 — D-Bus 不可用 → `BiometricUnavailable` / 查找失败 → `BiometricUnavailable`
- [x] F'4.7 手写 FFI 绑定 — 使用 `#[link(name = "secret-1")]` 手写 FFI（`secret_password_store_sync` / `secret_password_lookup_sync` / `secret_password_clear_sync` + `GError` + `SecretSchema`），可变参数通过 `...` 传递

**依赖**：F'1

---

### F'5. 编译验证

- [x] F'5.1 `cargo check -p assistant-security --target x86_64-pc-windows-msvc` 通过（Windows 目标）— 当前平台已验证
- [ ] F'5.2 `cargo check -p assistant-security --target x86_64-apple-darwin` 通过（macOS 目标）— 需 macOS 环境验证
- [ ] F'5.3 `cargo check -p assistant-security --target x86_64-unknown-linux-gnu` 通过（Linux 目标）— 需 Linux 环境验证
- [x] F'5.4 `cargo check` 通过（当前平台 Windows）✅
- [ ] F'5.5 `cargo clippy -p assistant-security` 无新增 warning

**依赖**：F'2, F'3, F'4

---

### F'6. 功能验证

> 以下测试需在对应平台物理设备/虚拟机上执行

- [ ] F'6.1 **Windows**：`is_available()` 在配置了 Windows Hello 的设备上返回 true
- [ ] F'6.2 **Windows**：`set_key` 弹出 Windows Hello 确认 → 用户通过 → 成功
- [ ] F'6.3 **Windows**：`set_key` 用户取消 → 返回 `BiometricCancelled`
- [ ] F'6.4 **Windows**：`get_key` 弹出 Windows Hello 确认 → 返回与 set_key 相同的 data
- [ ] F'6.5 **Windows**：`has_key` 不弹窗 → 返回 true（set_key 后）
- [ ] F'6.6 **Windows**：`delete_key` 后 `has_key` 返回 false
- [ ] F'6.7 **Windows**：未配置 Windows Hello 时 `is_available()` 返回 false
- [ ] F'6.8 **macOS**：`is_available()` 在有 Touch ID 的 Mac 上返回 true
- [ ] F'6.9 **macOS**：`set_key` 弹出 Touch ID 确认 → 用户通过 → 成功
- [ ] F'6.10 **macOS**：`set_key` 用户取消 → 返回 `BiometricCancelled`
- [ ] F'6.11 **macOS**：`get_key` 弹出 Touch ID 确认 → 返回与 set_key 相同的 data
- [ ] F'6.12 **macOS**：`has_key` 不弹窗 → 返回 true（set_key 后）
- [ ] F'6.13 **macOS**：`delete_key` 后 `has_key` 返回 false
- [ ] F'6.14 **macOS**：无 Touch ID 的 Mac 上 `is_available()` 返回 false
- [ ] F'6.15 **Linux**：`is_available()` 在 gnome-keyring 运行时返回 true
- [ ] F'6.16 **Linux**：`set_key` 成功存入 gnome-keyring
- [ ] F'6.17 **Linux**：`get_key` 返回与 set_key 相同的 data
- [ ] F'6.18 **Linux**：`has_key` 返回 true（set_key 后）
- [ ] F'6.19 **Linux**：`delete_key` 后 `has_key` 返回 false
- [ ] F'6.20 **Linux**：gnome-keyring 未运行时 `is_available()` 返回 false

**依赖**：F'5

---

### F'7. 集成验证（上层逻辑不变性）

- [ ] F'7.1 enroll_biometric 成功 → `is_biometric_enrolled` 返回 true
- [ ] F'7.2 unlock_biometric 成功 → vault 解锁（条目可访问）
- [ ] F'7.3 unlock_biometric 用户取消 → vault 仍锁定，前端回退主密码输入
- [ ] F'7.4 disenroll_biometric → `is_biometric_enrolled` 返回 false
- [ ] F'7.5 vault 锁定后生物识别解锁 → check_biometric_consistency 自愈正常
- [ ] F'7.6 主密码变更 → 自动 disenroll → 前端提示重新启用
- [ ] F'7.7 前端 VaultUnlock 生物识别按钮正确显示（available && enrolled）
- [ ] F'7.8 前端 VaultSettings 开关 ↔ enroll 状态同步
- [ ] F'7.9 master_key 不经 IPC（代码审查：unlock_biometric 返回 bool，master_key 仅存 Rust 内存）

**依赖**：F'6

---

### F'8. 安全不变式验证

- [ ] F'8.1 Windows: ~~私钥不离开 TPM（代码审查：仅 KeyCredentialManager API，无私钥导出）~~ — **口径需修正**：`KeyCredentialManager` 只提供 `RequestSignAsync` 签名能力，**不导出任何密钥材料**；vault 密钥密文另存于 `keyring`。代码审查项改为「无密钥导出接口调用、无明文落盘」，不再以"私钥在 TPM 内"表述（TPM 为可选后端，非唯一实现）
- [ ] F'8.2 Windows: ~~DPAPI 加密文件绑定当前用户（代码审查：CryptProtectData 无 LOCAL_MACHINE 标志）~~ — **实现方式已变更**：`windows.rs` 无任何 `CryptProtectData` 调用、无自定义加密文件，改用 `keyring::Entry(KEYRING_SERVICE, key_id)` 存 `biometric_key_enc` / `sig_hash`（Windows 后端为凭据管理器，底层由 DPAPI 按当前用户保护）。审查项改为「无自定义 DPAPI 文件、密钥不落明文盘」
- [ ] F'8.3 Windows: ~~get_key 必须经过 TPM 签名门控（代码审查：先 RequestSignAsync 再读文件）~~ — **门控位置已上移**：签名门控不在 `get_key` 内，而在 `biometric_ops::unlock_windows` —— 先 `request_signature`（触发 Windows Hello）→ 比对 `sig_hash`（不匹配即 `BiometricFailed`，要求重新登记）→ 才解密 keyring 中的密文。审查项改为「解密前必须完成签名比对」
- [ ] F'8.4 macOS: Keychain 条目受 kSecAccessControlBiometryCurrentSet 保护（代码审查：set_key 指定 AccessControl）
- [ ] F'8.5 macOS: 条目不同步到 iCloud（代码审查：kSecAttrSynchronizable = false）
- [ ] F'8.6 Linux: 数据存入默认 keyring（代码审查：SECRET_COLLECTION_DEFAULT）
- [ ] F'8.7 所有平台: get_key 触发 OS 认证（代码审查）
- [ ] F'8.8 所有平台: has_key 不触发 OS 认证（代码审查）

**依赖**：F'7
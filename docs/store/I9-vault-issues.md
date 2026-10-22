# Vault 代码审查问题清单

> 审查日期：2026-10-22
> 审查方法：从 10 条关键流程全链路追踪，逐层检查边界条件与异常路径
> 涉及 crate：assistant-security、assistant-sync、src-tauri/src/cmd/vault.rs
> 涉及前端：vaultStore.ts、VaultApp.tsx、vault.service.ts

---

## 问题总览

| # | 问题 | 严重度 | 类型 | 修复难度 | 关联 spec |
|---|------|--------|------|---------|----------|
| 1 | vault_lock 不移除 SSH Agent 密钥 | 🔴 严重 | 安全 | 低 | spec-5 §3.1 |
| 2 | vault_unlock 不自动添加 SSH 密钥 | 🟡 中 | 功能缺失 | 低 | spec-5 §3.2 |
| 3 | decrypt_* 返回 String 不 zeroize | 🟡 中 | 安全隐患 | 中 | spec-1 §4.2 |
| 4 | HIBP Audit 串行请求极慢 | 🟡 中 | 性能 | 低 | spec-3 §5.3 |
| 5 | 改密后 emergency data 未清理 | 🟡 中 | 安全隐患 | 低 | spec-5 §6.2 |
| 6 | Windows 屏幕锁可能收不到事件 | 🟡 中 | 可靠性 | 中 | spec-3 §4.1 |
| 7 | ClipboardGuard 守护线程持锁做 IO | 🟡 低 | 死锁风险 | 低 | spec-3 §3.2 |
| 8 | vault:unlocked 事件 loading 残留 | 🟡 低 | UI 状态 | 低 | spec-2 §2.3 |
| 9 | vault_export 明文文件无保护 | 🟡 中 | 安全隐患 | 中 | spec-2 §3.4 |
| 10 | RevealResult 明文进 JS 堆 | ⚪ 已知 | 设计权衡 | — | spec-3 §2.1 |
| 11 | 灾难恢复完全不可用（4 个关键缺失） | 🔴 严重 | 功能缺失 | 高 | spec-5 §13.5 |
| 12 | .vault-meta 缺少 masterKeySalt/argon2Params 字段 | 🟡 中 | 数据缺失 | 中 | spec-5 §13.5 |
| 13 | VaultMetaFile.emergency 仅存 token+salt，灾难恢复无法重建 security.db | 🔴 严重 | 设计缺陷 | 高 | spec-5 §13.2 |

---

## 🔴 问题 1：vault_lock 不移除 SSH Agent 中的密钥

### 严重度：严重

### 关联流程

锁定 vault → SSH 密钥残留在 Agent 中 → 任何进程可通过 `$SSH_AUTH_SOCK` 使用密钥

### 全链路

```
vault_lock (vault.rs:101)
  → state.lock()                        // 只清 master_key + conn
  → emit(VAULT_LOCKED)                  // 事件广播
  → ❌ 没有遍历 ssh_state.added_fingerprints 逐个 bridge.remove_key()
```

### 代码位置

- `src-tauri/src/cmd/vault.rs` 第 101–109 行：`vault_lock` command
- `src-tauri/src/setup.rs` 第 103 行：ScreenLockMonitor 回调也调用 `state.lock()`

### 违反的 spec

spec-5 §3.1 明确要求："vault 锁定时自动从 SSH agent 移除所有由 vault 添加的密钥"

### 后果

锁定 vault 后，SSH Agent 中仍有私钥可用。任何进程都能通过 `$SSH_AUTH_SOCK` 使用这些密钥认证，**绕过 vault 的访问控制**。屏幕锁触发自动锁定时同样存在此问题。

### 采纳修复方案

提取 `ssh_remove_all_on_lock` 辅助函数，在 `vault_lock` 和 `ScreenLockMonitor` 回调中统一调用。

**改动文件**：
- `src-tauri/src/cmd/vault.rs` — 新增 `pub(crate) fn ssh_remove_all_on_lock`，`vault_lock` 中调用
- `src-tauri/src/setup.rs` — ScreenLockMonitor 回调中调用

**步骤 1**：在 `vault.rs` 中 `vault_ssh_auto_add_all` 附近新增辅助函数：

```rust
pub(crate) fn ssh_remove_all_on_lock(app: &AppHandle) {
    let ssh_state = app.state::<crate::setup::SshAgentState>();
    let fingerprints: Vec<String> = {
        let mut fps = ssh_state.added_fingerprints.lock().unwrap();
        fps.drain().collect()
    };
    if fingerprints.is_empty() {
        return;
    }
    let bridge = ssh_bridge_from_conf();
    for fp in &fingerprints {
        if let Err(e) = bridge.remove_key(fp) {
            log::warn!("Failed to remove SSH key {} from agent: {}", fp, e);
        }
    }
    log::info!("Removed {} SSH keys from agent on vault lock", fingerprints.len());
}
```

> 关键：使用 `drain().collect()` 一步完成"取出 + 清空"，避免二次加锁。

**步骤 2**：修改 `vault_lock`：

```rust
#[command]
pub fn vault_lock(app: AppHandle) -> Result<(), String> {
    ssh_remove_all_on_lock(&app);
    let state = vault_state(&app)?;
    state.lock().map_err(|e| e.to_string())?;
    crate::protocol::events::emit_broadcast(
        &app,
        crate::protocol::events::event_name::VAULT_LOCKED,
        (),
    );
    Ok(())
}
```

**步骤 3**：修改 `setup.rs` ScreenLockMonitor 回调：

```rust
monitor.spawn_listener(move || {
    crate::cmd::ssh_remove_all_on_lock(&app_handle);
    let state = app_handle.state::<SecurityDbState>();
    if let Err(e) = state.lock() {
        log::error!("Screen lock triggered vault_lock but failed: {}", e);
    } else {
        log::info!("Screen lock triggered vault_lock");
    }
    let _ = emit_broadcast(&app_handle, event_name::VAULT_LOCKED, ());
    if let Err(e) = crate::cmd::vault_show_unlock_window(app_handle.clone()) {
        log::warn!("Screen lock show_unlock_window failed: {}", e);
    }
});
```

**风险评估**：低。`vault_ssh_remove_from_agent` 已实现，`SshAgentState.added_fingerprints` 已存在，只差调用。`drain()` 保证即使部分 remove 失败，fingerprint 列表也会清空（下次 lock 不会重复尝试）。

---

## 🟡 问题 2：vault_unlock 后不自动添加 SSH 密钥到 Agent

### 严重度：中

### 关联流程

解锁 vault → SSH 密钥不会自动加载 → 用户必须手动添加

### 全链路

```
vault_unlock (vault.rs:48)
  → state.unlock()
  → emit(VAULT_UNLOCKED)
  → ❌ 没有调用 ssh_auto_add_all()
```

### 代码位置

- `src-tauri/src/cmd/vault.rs` 第 48–60 行：`vault_unlock` command
- `src-tauri/src/cmd/vault.rs` 第 734 行：`vault_ssh_auto_add_all` 已实现但未被 unlock 流程调用
- `AppConf.vault_ssh_auto_add` 配置项存在但未被消费

### 违反的 spec

spec-5 §3.2 要求："vault 解锁时若 `vault_ssh_auto_add=true`，自动将所有 SSH 类别条目添加到 agent"

### 后果

用户每次解锁后必须手动点击"添加到 Agent"，或手动调用 `sshAutoAddAll`。配置项 `vault_ssh_auto_add` 形同虚设。

### 采纳修复方案

提取 `ssh_auto_add_on_unlock` 辅助函数，在三种 unlock 路径成功后统一调用。

**改动文件**：
- `src-tauri/src/cmd/vault.rs` — 新增 `fn ssh_auto_add_on_unlock`，修改 `vault_unlock` / `vault_unlock_biometric` / `vault_unlock_with_recovery_key`

**步骤 1**：新增辅助函数：

```rust
fn ssh_auto_add_on_unlock(app: &AppHandle) {
    use assistant_config::app_conf::AppConf;
    let conf = match AppConf::load() {
        Ok(c) => c,
        Err(_) => return,
    };
    if !conf.vault_ssh_auto_add {
        return;
    }
    if let Err(e) = vault_ssh_auto_add_all(app.clone()) {
        log::warn!("SSH auto-add on unlock failed: {}", e);
    }
}
```

**步骤 2**：修改 `vault_unlock`：

```rust
pub fn vault_unlock(app: AppHandle, master_password: String) -> Result<bool, String> {
    let state = vault_state(&app)?;
    match state.unlock(&master_password) {
        Ok(()) => {
            ssh_auto_add_on_unlock(&app);
            crate::protocol::events::emit_broadcast(
                &app,
                crate::protocol::events::event_name::VAULT_UNLOCKED,
                (),
            );
            Ok(true)
        }
        Err(VaultError::InvalidPassword) => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}
```

**步骤 3**：修改 `vault_unlock_biometric`（同理，在 `Ok(())` 分支 emit 前加 `ssh_auto_add_on_unlock(&app);`）

**步骤 4**：修改 `vault_unlock_with_recovery_key`（同理，在 `if result {` 块内 emit 前加 `ssh_auto_add_on_unlock(&app);`）

**风险评估**：低。`vault_ssh_auto_add_all` 已有完整实现（检查配置 → 遍历 SSH 条目 → validate + add + 记录 fingerprint），只是未被 unlock 路径调用。配置项 `vault_ssh_auto_add` 默认 `false`，不影响现有用户。

---

## 🟡 问题 3：decrypt_* 返回 String 而非 VaultPlaintext — 密码明文不 zeroize

### 严重度：中

### 关联流程

复制密码 / HIBP 检查 / 导出 / Reveal — 所有需要解密的流程

### 全链路

```
security_db::decrypt_password (mod.rs:650)
  → VaultCrypto::decrypt_field() → VaultPlaintext    // 安全类型
  → plaintext.to_str() → String::from()               // ❌ 转为普通 String
  → VaultPlaintext Drop → zeroize                     // 原 VaultPlaintext 清零
  → 返回的 String 在堆上，Drop 时 ❌ 不 zeroize
```

### 代码位置

- `assistant-security/src/security_db/mod.rs` 第 650 行：`decrypt_password` → 返回 `String`
- `assistant-security/src/security_db/mod.rs` 第 672 行：`decrypt_notes` → 返回 `String`
- `assistant-security/src/security_db/mod.rs` 第 694 行：`decrypt_totp_secret` → 返回 `String`
- `assistant-security/src/security_db/mod.rs` 第 716 行：`decrypt_field` → 返回 `String`
- `assistant-security/src/security_db/mod.rs` 第 758 行：`decrypt_ssh_private_key` → 返回 `VaultPlaintext`（✅ 已安全，无需修改）

### 违反的 spec

spec-1 §4.2 要求："解密后的敏感数据必须通过安全类型系统管理，确保内存清零"

### 后果

密码明文以普通 `String` 存在于堆上，Rust Drop 只释放内存不覆写内容。**可能被内存扫描、core dump、或 /proc/pid/mem 捕获**。`VaultPlaintext` 的 zeroize 保护在 `to_str()` 转换时失效。

### 采纳修复方案

**方案 A（采纳）**：定义 `SecureString` 类型，4 个 `decrypt_*` 返回类型改为 `SecureString`，调用方按需 `.expose_str()` 或 `.into_inner()`。

**改动文件**：
- `assistant-security/src/vault_crypto/types.rs` — 新增 `SecureString`
- `assistant-security/src/security_db/mod.rs` — 4 个 `decrypt_*` 返回类型改为 `SecureString`
- `src-tauri/src/cmd/vault.rs` — 8 个调用方适配

**步骤 1**：在 `types.rs` 中新增 `SecureString`（与 `VaultPlaintext` / `SafePassword` 并列）：

```rust
pub struct SecureString {
    inner: String,
}

impl SecureString {
    pub fn new(s: String) -> Self {
        Self { inner: s }
    }

    pub fn expose_str(&self) -> &str {
        &self.inner
    }

    /// 消费 SecureString 返回内部 String，调用方负责后续 zeroize。
    /// 仅用于必须传 String 的场景（如 IPC 返回值）。
    pub fn into_inner(mut s: Self) -> String {
        let inner = std::mem::take(&mut s.inner);
        std::mem::forget(s);
        inner
    }
}

impl Drop for SecureString {
    fn drop(&mut self) {
        let bytes = unsafe { self.inner.as_bytes_mut() };
        bytes.zeroize();
    }
}

impl fmt::Debug for SecureString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecureString([REDACTED; {} chars])", self.inner.len())
    }
}
```

**步骤 2**：修改 4 个 `decrypt_*` 返回类型（`decrypt_ssh_private_key` 不改，已返回 `VaultPlaintext`）：

```rust
pub fn decrypt_password(&self, id: &str) -> Result<SecureString, VaultError> {
    self.with_conn_and_key(|conn, mk| {
        // ... 查询 + 解密 ...
        let s = plaintext.to_str().map(String::from).map_err(|_| ...)?;
        Ok(SecureString::new(s))
    })
}
// decrypt_notes, decrypt_totp_secret, decrypt_field 同理
```

**步骤 3**：调用方适配（vault.rs 8 处）：

| 调用方 | 场景 | 适配方式 |
|--------|------|----------|
| `vault_copy_password` | 借用 &str 写剪贴板 | `password.expose_str()`，SecureString 作用域结束自动 zeroize |
| `vault_copy_field` | 同上 | `value.expose_str()` |
| `vault_reveal_password` | IPC 返回 String | `SecureString::into_inner(password)` — 密码最终进 JS 堆，与问题 10 同理 |
| `vault_reveal_notes` | 同上 | `SecureString::into_inner(notes)` |
| `vault_generate_totp` | TOTP 计算 | `totp_secret.expose_str()` |
| `vault_copy_totp` | 同上 | `totp_secret.expose_str()` |
| `vault_hibp_audit` | HIBP 检查 | `SafePassword::new(SecureString::into_inner(password))` — SafePassword 自身有 zeroize |
| `export_entries` | 导出 | 在 `VaultEntryExport` 构造时用 `expose_str()` 借用 |

**风险评估**：中等。改动面涉及 4 个函数签名 + 8 个调用方，但逻辑不变，只是类型包装。`into_inner` 用于必须传 String 的场景（reveal/IPC），`expose_str` 用于临时借用场景（copy/hibp/totp）。

---

## 🟡 问题 4：HIBP Audit 串行请求 — 大量条目时极慢

### 严重度：中

### 关联流程

HIBP 全量审计 → 逐条串行 HTTP 请求 → 超长等待

### 全链路

```
vault_hibp_audit (vault.rs:432)
  → for entry in &entries {
      client.check_password(&password).await    // ❌ 串行，每次一个 HTTP round-trip
    }
```

### 代码位置

- `src-tauri/src/cmd/vault.rs` 第 432–469 行：`vault_hibp_audit`

### 后果

500 个条目 × ~200ms/请求 = **~100 秒**。前端无进度反馈，用户可能以为卡死。

### 采纳修复方案

使用 `futures::stream::iter` + `buffer_unordered` 做受控并发，加 1500ms 间隔信号量避免触发 HIBP API 429 速率限制。

**改动文件**：
- `src-tauri/src/cmd/vault.rs` — 重写 `vault_hibp_audit`
- `src-tauri/Cargo.toml` — 确认 `futures` 依赖

**步骤 1**：重写 `vault_hibp_audit`：

```rust
use futures::stream::{self, StreamExt};
use std::sync::Arc;
use tokio::time::{Duration, sleep};

const HIBP_CONCURRENCY: usize = 2;
const HIBP_MIN_INTERVAL_MS: u64 = 1500;

#[command]
pub async fn vault_hibp_audit(app: AppHandle) -> Result<Vec<(String, u32)>, String> {
    let state = vault_state(&app)?;
    let entries = state
        .list_entries(None, None, None)
        .map_err(|e| e.to_string())?;

    let client = Arc::new(HibpClient::new());
    let interval = Arc::new(tokio::sync::Semaphore::new(1));

    let checks: Vec<(String, String)> = entries.iter()
        .filter_map(|entry| {
            state.decrypt_password(&entry.id).ok().map(|p| {
                (entry.id.clone(), SecureString::into_inner(p))
            })
        })
        .collect();

    let results: Vec<(String, u32)> = stream::iter(checks)
        .map(|(id, password)| {
            let client = client.clone();
            let interval = interval.clone();
            async move {
                let _permit = interval.acquire().await.unwrap();
                let safe = SafePassword::new(password);
                let count = client.check_password(&safe).await.unwrap_or(0);
                sleep(Duration::from_millis(HIBP_MIN_INTERVAL_MS)).await;
                drop(_permit);
                (id, count)
            }
        })
        .buffer_unordered(HIBP_CONCURRENCY)
        .filter(|(_, count)| futures::future::ready(*count > 0))
        .collect()
        .await;

    {
        let guard = state.get_conn().map_err(|e| e.to_string())?;
        if let Some(conn) = guard.as_ref() {
            let _ = audit_log(conn, AuditEvent::HibpAudit, None);
        }
    }

    Ok(results)
}
```

**设计要点**：
- `Semaphore(1)` 作为速率限制器：每次 acquire 后 sleep 1500ms 才释放，确保请求间隔 ≥1500ms
- `buffer_unordered(2)`：最多 2 个并发请求（一个在等响应，一个在等间隔）
- `SafePassword` 保证每个密码用完 zeroize
- 500 条目：约 500 × 1500ms / 2 并发 ≈ **6 分钟**（比串行 100 秒慢，但不会触发 429）

> **权衡**：HIBP API v3 range endpoint 速率限制为 1500ms/请求。串行无间隔可能频繁 429 导致漏检；受控并发 + 间隔保证不触发 429，总时间略长但结果可靠。如需更快，可考虑 `buffer_unordered(4)` + `Semaphore(2)` 分组限速。

**风险评估**：低。逻辑不变，只是从串行改为受控并发。`Semaphore` 确保不违反 HIBP 速率限制。

---

## 🟡 问题 5：change_master_password 后恢复密钥 emergency data 未清理

### 严重度：中

### 关联流程

修改主密码 → 旧恢复密钥失效但 emergency data 文件仍在 → 用户使用恢复密钥得到不友好错误

### 全链路

```
change_master_password (mod.rs:780)
  → 重新加密所有条目
  → PRAGMA rekey（改变 SQLCipher 密钥）
  → disenroll_biometric()                // ✅ 生物识别已清理
  → ❌ 没有删除 emergency data 文件
```

### 代码位置

- `assistant-security/src/security_db/mod.rs` 第 780–1024 行：`change_master_password`
- 第 1019 行：调用了 `disenroll_biometric` 但没有清理 emergency data

### 后果

1. 旧恢复密钥解出旧 master_key，无法打开 rekey 后的 DB → `InvalidRecoveryKey` 错误
2. emergency data 文件仍存在于 Git 仓库中，包含用旧恢复密钥加密的旧 master_key
3. 如果攻击者同时获得旧恢复密钥和旧 DB 备份，可以解锁旧备份
4. 用户尝试使用恢复密钥得到不友好的 `InvalidRecoveryKey` 错误，不知道原因

### 采纳修复方案

在 `vault_change_master_password` command 成功后，清理 emergency data 并记录审计日志。

**改动文件**：
- `src-tauri/src/cmd/vault.rs` — `vault_change_master_password` 中添加 emergency 清理
- `assistant-security/src/audit/mod.rs` — `AuditEvent` 枚举新增 `RecoveryKeyInvalidated`
- `assistant-sync/src/vault_file/mod.rs` — 新增 `clear_emergency_data` 方法
- `assistant-sync/src/sync_engine/mod.rs` — 新增 `clear_emergency_data` 透传方法

**步骤 1**：`AuditEvent` 新增枚举值：

```rust
pub enum AuditEvent {
    // ... existing ...
    RecoveryKeyInvalidated,
}
```

对应 `Display` impl 添加 `"RecoveryKeyInvalidated"` 分支。

**步骤 2**：`vault_file/mod.rs` 新增 `clear_emergency_data`：

```rust
pub fn clear_emergency_data(&self) -> Result<(), SyncError> {
    let path = self.vault_dir.join(VAULT_META_FILE);
    if !path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(&path)?;
    let mut meta: VaultMetaFile = serde_json::from_str(&content)
        .map_err(|e| SyncError::Other(e.to_string()))?;
    meta.emergency = None;
    let json = serde_json::to_string_pretty(&meta)
        .map_err(|e| SyncError::Other(e.to_string()))?;
    fs::write(&path, json)?;
    Ok(())
}
```

**步骤 3**：`sync_engine/mod.rs` 透传：

```rust
pub fn clear_emergency_data(&self) -> Result<(), SyncError> {
    self.vault_file.clear_emergency_data()
}
```

**步骤 4**：修改 `vault_change_master_password` command：

```rust
#[command]
pub fn vault_change_master_password(
    app: AppHandle,
    current: String,
    new: String,
) -> Result<(), String> {
    let state = vault_state(&app)?;
    state
        .change_master_password(&current, &new, cipher_algo())
        .map_err(|e| e.to_string())?;

    // 清理 emergency data（旧恢复密钥已失效）
    {
        let engine = sync_state(&app)?;
        if let Err(e) = engine.clear_emergency_data() {
            log::warn!("Failed to clear emergency data after master password change: {}", e);
        }
    }

    // 审计日志
    {
        let guard = state.get_conn().map_err(|e| e.to_string())?;
        if let Some(conn) = guard.as_ref() {
            let _ = audit_log(conn, AuditEvent::RecoveryKeyInvalidated, None);
        }
    }

    Ok(())
}
```

**风险评估**：低。`change_master_password` 本身已正确 `disenroll_biometric`，添加 emergency 清理是对称操作。`clear_emergency_data` 只修改 `.vault-meta` JSON 的 `emergency` 字段为 `null`，不影响其他字段。前端应在改密成功后提示用户"恢复密钥已失效，请重新生成"。

---

## 🟡 问题 6：ScreenLockMonitor Windows 实现使用 GetForegroundWindow — 可能无法接收锁屏消息

### 严重度：中

### 关联流程

屏幕锁监听 → 获取窗口句柄 → 注册通知 → 可能收不到事件

### 全链路

```
windows_listen (screen_lock/mod.rs:52)
  → hwnd = GetForegroundWindow()            // ❌ 可能是其他进程的窗口
  → WTSRegisterSessionNotification(hwnd)    // 消息发到 hwnd，但 hwnd 不属于本进程
  → GetMessageW(ptr::null_mut(), ...)       // 收不到 WM_WTSSESSION_CHANGE
```

### 代码位置

- `assistant-security/src/screen_lock/mod.rs` 第 52–99 行：`windows_listen`

### 问题分析

1. `GetForegroundWindow()` 返回的是**调用时刻**的前台窗口，可能是其他应用的
2. `WTSRegisterSessionNotification` 要求 hwnd 属于当前进程，否则消息无法送达
3. 如果应用启动时最小化到托盘，没有前台窗口，`hwnd` 为 null → 直接 return
4. `GetMessageW` 使用 `ptr::null_mut()`（线程所有消息），但 `WM_WTSSESSION_CHANGE` 是发给特定窗口的消息

### 后果

屏幕锁事件可能**完全无法检测**，自动锁定功能在 Windows 上失效。

### 修复方案

创建一个隐藏的 Message-Only 窗口（`HWND_MESSAGE`），用它接收 `WM_WTSSESSION_CHANGE`：

```rust
fn create_message_window() -> Hwnd {
    extern "system" {
        fn CreateWindowExW(...) -> Hwnd;
        fn RegisterClassW(...) -> u16;
    }
    // 注册一个空窗口类
    // 创建 HWND_MESSAGE 类型的 message-only 窗口
    // 该窗口不可见，但能接收 WM_WTSSESSION_CHANGE
}
```

或使用 Tauri 的 `WebviewWindow::hwnd()` 获取 Tauri 窗口句柄（保证属于本进程）。

---

## 🟡 问题 7：ClipboardGuard 守护线程中恢复原内容时持锁做 IO — 死锁风险

### 严重度：低

### 关联流程

守护线程超时 → 恢复原内容 → 持有 Mutex 锁执行剪贴板 IO

### 全链路

```
guard thread (clipboard_guard.rs:65)
  → let should_clear = { current_password.lock() ... };   // ✅ 释放
  → if should_clear {
      let orig = original.lock().unwrap();                // ❌ 持有 original 锁
      write_clipboard_internal(original_text);            //    同时执行 IO
      // original 锁直到 } 才释放
    }
```

### 代码位置

- `assistant-security/src/clipboard_guard.rs` 第 65–82 行：守护线程逻辑

### 后果

`write_clipboard_internal` 涉及 `arboard::Clipboard::new()` + `set_text()`，在 Windows 上是 OLE 剪贴板操作，可能阻塞。如果此时主线程调用 `copy_and_guard` 或 `force_clear` 也需要 `original.lock()`，会长时间阻塞（非死锁，但主线程卡住直到 OLE 完成）。

实际风险中等：`write_clipboard_internal` 通常 <50ms，但如果其他进程锁定剪贴板（如大型复制操作），可能阻塞数秒。

### 采纳修复方案

参照同文件 `force_clear` 的模式，先 `take()` 释放锁，再写剪贴板。

**改动文件**：
- `assistant-security/src/clipboard_guard.rs` — 守护线程中 `should_clear` 块

**步骤**：修改守护线程中的恢复逻辑：

```rust
// 修改前（持锁做 IO）：
if should_clear {
    let orig = original.lock().unwrap();
    if let Some(ref original_text) = *orig {
        let _ = write_clipboard_internal(original_text);
    } else {
        let _ = clear_clipboard_internal();
    }
    let mut cur = current_password.lock().unwrap();
    *cur = None;
}

// 修改后（先 take 释放锁，再 IO）：
if should_clear {
    let original_content = {
        let mut orig = original.lock().unwrap();
        orig.take()
    };
    if let Some(ref text) = original_content {
        let _ = write_clipboard_internal(text);
    } else {
        let _ = clear_clipboard_internal();
    }
    {
        let mut cur = current_password.lock().unwrap();
        *cur = None;
    }
}
```

**设计要点**：
- `take()` 将 `Option<String>` 取出设为 `None`，锁立即释放
- 与 `force_clear` 完全相同的模式，保持一致性
- `original_content` 在块结束时 Drop，不影响功能（恢复原内容只需写一次）

**风险评估**：极低。逻辑等价，只是减少了持锁时间。`force_clear` 已使用此模式，经过验证。

---

## 🟡 问题 8：vault:unlocked 事件绕过 store action — loading 状态可能残留

### 严重度：低

### 关联流程

生物识别/恢复密钥解锁 → Rust emit 事件 → 前端直接 setState → loading 残留

### 全链路

```
Rust: vault_unlock_biometric → emit(VAULT_UNLOCKED)
前端: listen("vault:unlocked", () => {
  useVaultStore.setState({ isLocked: false });    // ❌ 没有设置 loading: false
  resetAutoLockTimer();
})
```

对比正常密码解锁走 `unlock` action：`set({ loading: true })` → `set({ isLocked: false, loading: false })`。

### 代码位置

- `src/tools/password-vault/VaultApp.tsx` 第 31–34 行：`vault:unlocked` 事件监听

### 后果

如果 `loading` 之前为 `true`（比如用户点了解锁按钮后切换到生物识别），生物识别成功后 `loading` 残留为 `true`，UI 可能显示 loading 状态。

### 采纳修复方案

在 `vault:unlocked` 事件监听器中添加 `loading: false` + `error: null`，与 vaultStore 中 unlock 方法的成功路径保持一致。

**改动文件**：
- `src/tools/password-vault/VaultApp.tsx` — 事件监听器

**步骤**：

```tsx
// 修改前：
void listen("vault:unlocked", () => {
  useVaultStore.setState({ isLocked: false });
  useVaultStore.getState().resetAutoLockTimer();
}).then((fn) => {

// 修改后：
void listen("vault:unlocked", () => {
  useVaultStore.setState({ isLocked: false, loading: false, error: null });
  useVaultStore.getState().resetAutoLockTimer();
}).then((fn) => {
```

**设计要点**：
- `loading: false`：修复独立解锁窗口路径下 loading 残留的 bug
- `error: null`：清除可能残留的错误信息（如之前输入错误密码的提示）
- 与 vaultStore 中 `unlock()`/`unlockBiometric()`/`unlockWithRecoveryKey()` 成功时的 setState 保持一致

**风险评估**：极低。一行改动，仅影响事件监听路径。

---

## 🟡 问题 9：vault_export 明文文件无加密保护

### 严重度：中

### 关联流程

导出 → 解密所有条目 → 明文 JSON/CSV 写入磁盘 → 无自动清理

### 全链路

```
vault_export (vault.rs:305)
  → state.export_entries()                      // 解密所有条目为明文
  → std::fs::write(file_path, &content)         // ❌ 明文写入磁盘
  → 文件路径: app_data_dir/security/exports/vault_export_20260101_120000.json
```

### 代码位置

- `src-tauri/src/cmd/vault.rs` 第 305–330 行：`vault_export`
- `assistant-security/src/security_db/mod.rs` 第 1024–1102 行：`export_entries`

### 后果

明文密码以 JSON/CSV 形式存在于磁盘上，任何能读取 app data 目录的进程都能获取所有密码。文件不会自动清理。

### 采纳修复方案

采用 **方案 B + C 组合**：TTL 自动清理 + UI 确认警告。方案 A（加密导出）留作后续迭代。

**改动文件**：
- `src-tauri/src/cmd/vault.rs` — `vault_export` 添加 TTL 清理线程
- `src/tools/password-vault/VaultSettings.tsx` — 导出按钮添加确认弹窗
- `src/tools/password-vault/locales/` — i18n 新增导出警告词条

**步骤 1**：`vault_export` 添加 5 分钟 TTL 自动清理：

```rust
pub fn vault_export(app: AppHandle, format: String) -> Result<String, String> {
    // ... 现有解密 + 写文件逻辑 ...

    let file_path = export_dir.join(&file_name);
    std::fs::write(&file_path, &content).map_err(|e| e.to_string())?;

    // 5 分钟后自动清理导出文件
    let cleanup_path = file_path.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(300));
        if let Err(e) = std::fs::remove_file(&cleanup_path) {
            log::warn!("Failed to auto-clean export file: {}", e);
        } else {
            log::info!("Auto-cleaned export file: {:?}", cleanup_path);
        }
    });

    Ok(file_path.to_str().unwrap_or(&file_name).to_string())
}
```

**步骤 2**：VaultSettings.tsx 导出按钮添加确认弹窗：

```tsx
const handleExport = async (format: 'json' | 'csv') => {
  Modal.confirm({
    title: t('vault:exportWarningTitle'),
    content: t('vault:exportWarningContent'),
    // "导出文件包含所有密码的明文，将在 5 分钟后自动删除。
    //  请确保在安全环境中操作，并在用后手动删除文件。"
    okText: t('vault:exportConfirm'),
    okType: 'danger',
    cancelText: t('common:cancel'),
    onOk: async () => {
      const path = await invoke<string>('vault_export', { format });
      message.info(t('vault:exportSuccess', { path }));
    },
  });
};
```

**步骤 3**：i18n 词条（`tools.password_vault` 命名空间）：

```json
{
  "exportWarningTitle": "确认导出密码",
  "exportWarningContent": "导出文件包含所有密码的明文，将在 5 分钟后自动删除。请确保在安全环境中操作，并在用后手动删除文件。",
  "exportConfirm": "我了解风险，继续导出",
  "exportSuccess": "已导出到：{{path}}"
}
```

**设计要点**：
- TTL 清理用独立线程，不阻塞主流程，不持有任何锁
- 清理线程 `detach`（无需 JoinHandle），进程退出时自动终止
- UI 确认弹窗使用 `danger` 样式按钮，明确告知风险
- 未来可升级为方案 A（加密 ZIP 导出），当前方案向后兼容

**风险评估**：低。TTL 清理是纯新增逻辑，不影响现有导出功能。确认弹窗是纯 UI 改动。

---

## ⚪ 问题 10：RevealResult.value 密码明文通过 IPC 传到前端 JS

### 严重度：已知设计权衡

### 关联流程

reveal → 密码明文进入浏览器内存 → JS GC 回收不 zeroize

### 全链路

```
vault_reveal_password (vault.rs:179)
  → decrypt_password(&id) → String             // Rust 堆上明文
  → RevealResult { value: password, .. }        // 通过 Tauri IPC 传到前端
  → 前端 vaultStore.revealedField.value         // 浏览器 JS 堆上明文
  → 30s 后 clearRevealedField()                 // JS GC 回收（不 zeroize）
```

### 代码位置

- `src-tauri/src/cmd/vault.rs` 第 179–215 行：`vault_reveal_password`
- `src/tools/password-vault/vaultStore.ts` 第 297–316 行：`revealPassword` action

### 分析

这是 reveal 功能的设计意图（用户就是要看到密码），但需要权衡：

- ✅ DevTools 在 release 中已禁用（`window.rs` 第 120 行 `devtools(false)`）
- ⚠️ 恶意浏览器扩展仍可能读取 JS 堆
- ⚠️ JS GC 不 zeroize，密码可能残留在 V8 堆中直到被覆写

### 缓解措施

- 已有：RevealGuard 速率限制（3 次/分钟）+ 30s TTL 自动隐藏
- 可选：在 `clearRevealedField` 时用空字符串覆写 value 再置 null（减少 V8 堆残留窗口）

---

## 修复优先级建议

### P0 — 立即修复

| # | 问题 | 原因 |
|---|------|------|
| 1 | vault_lock 不移除 SSH Agent 密钥 | 直接违反 spec 安全要求，锁定后密钥仍可用 |

### P1 — 近期修复

| # | 问题 | 原因 |
|---|------|------|
| 2 | vault_unlock 不自动添加 SSH 密钥 | spec 功能缺失，配置项形同虚设 |
| 5 | 改密后 emergency data 未清理 | 恢复密钥静默失效，用户体验差 |
| 6 | Windows 屏幕锁可能收不到事件 | 自动锁定功能可能完全失效 |
| 8 | vault:unlocked 事件 loading 残留 | UI 状态 bug，容易修复 |

### P2 — 计划修复

| # | 问题 | 原因 |
|---|------|------|
| 3 | decrypt_* 返回 String 不 zeroize | 安全隐患，改动面较大 |
| 4 | HIBP Audit 串行请求 | 性能问题，500 条时 ~100s |
| 9 | vault_export 明文文件无保护 | 需要设计决策（加密 vs 警告） |

### P3 — 记录已知

| # | 问题 | 原因 |
|---|------|------|
| 10 | RevealResult 明文进 JS 堆 | 设计权衡，非 bug |

---

## 任务清单未完成项（从 I9-vault-tasklist.md 汇总）

> 以下为逐条验证后确认未实现或部分缺失的任务项，按阶段分组

### 阶段 C — 前端 UI + 工具注册

| 任务ID | 描述 | 缺失内容 |
|--------|------|----------|
| C4.2 | VaultList 条目卡片列表 | 泄露标记（⚠ hibp_count > 0）未渲染；标题/用户名/分类图标/收藏标记已实现 |
| C4.4 | VaultList 底部状态栏 | 条目数 + 泄露数均未实现；syncBar 区域仅有同步状态 |

### 阶段 D — TOTP + HIBP + 密码强度

| 任务ID | 描述 | 缺失内容 |
|--------|------|----------|
| D1.1 | totp.rs Steam Guard 模式 | `generate_totp` / `time_remaining` / `from_otpauth_uri` + SHA-1/256/512 + 6/8位均已实现，缺 Steam Guard 特殊编码模式 |
| D1.4 | 前端 TOTP 圆形倒计时环 | VaultEntryDetail 仅显示 "TOTP configured" 文本，缺圆形倒计时环 + 6位验证码 + 一键复制 |
| D2.4 | 前端条目泄露标记 | VaultList 中无泄露标记渲染；VaultSettings 中全库审计按钮已有；vaultStore 中 hibpLeaks 状态已有 |

### 阶段 E — 自动锁定 + 剪贴板守护 + CSP

| 任务ID | 描述 | 缺失内容 |
|--------|------|----------|
| E4.2 | vault-unlock.html 极简页面 | 独立解锁窗口的 HTML 页面不存在（<50行 JS + password input + unlock button + 立即清除 DOM） |
| E4.3 | 独立窗口 event 通知 | VaultApp.tsx 已有 `vault:unlocked`/`vault:locked` 事件监听，但缺独立窗口 HTML 中解锁成功后 `window.close()` 逻辑 |

### 阶段 A/B/C/D/E/F/G — 验证/测试任务（均未执行）

以下为需要实际编译/运行/测试才能确认的验证任务，暂未勾选：

| 阶段 | 任务ID | 描述 |
|------|--------|------|
| A | A8.1–A8.12 | cargo check / cargo test / MasterKey Drop zeroize / VaultPlaintext Drop zeroize / CipherAlgorithm discriminant / VaultEntryDecrypted 未 Clone / VaultPlaintext 未 Serialize / master_key 不持久化 / 密码不进 JS 堆 |
| B | B7.1–B7.10 | cargo check / 完整生命周期 / copy_password 不返回字符串 / reveal 速率限制 / 改密后可解锁 / 改密原子性 / export 后 zeroize / getEntry 不解密 / RevealGuard 审计日志 / VaultError 序列化 |
| C | C8.1–C8.12 | tsc --noEmit / 工具箱显示 / 完整 CRUD / 复制走 Rust / reveal 精确超时 / reveal 状态独立 / 自动锁定 / visibilitychange / 设置项同步 / 改密崩溃恢复 / VaultError 前端处理 / Detail 布尔标记 |
| D | D4.1–D4.7 | TOTP 对比验证 / otpauth URI 解析 / HIBP 前5位 / HIBP 完整哈希不离境 / Add-Padding header / 弱密码拒绝 / Diceware EFF 词表 |
| E | E5.1–E5.8 | 屏幕锁自动锁定 / 5分钟无操作 / 剪贴板30s恢复 / 应用关闭恢复 / CSP 生效 / DevTools 禁用 / V8 isolate / 窗口 focus |
| F | F3.1–F3.7 | Windows Hello / Touch ID / 回退主密码 / master_key 不经 IPC / disenroll 清理 / 改密后 disenroll / cargo check 三平台 |
| G | G1.1–G4.6 | 编译验证 / 安全不变式 / 功能验证 / 跨阶段约束验证 |

---

## 🔴 问题 11：灾难恢复完全不可用（4 个关键缺失）

### 严重度：严重

### 关联 spec

spec-5 §13.5 灾难恢复流程

### 设计要求的流程

1. 克隆 Git 仓库 → 获取 `.enc` 文件 + `.vault-meta`
2. 恢复密钥解锁 → 恢复 master_key
3. 验证 `.vault-meta` 的 HMAC 签名
4. 导入全部条目 → 遍历 `entries/*.enc` → 逐条 `import_entry` → 写入 SQLite
5. 导入删除记录 → 读取 `deletions/deleted.json` → 标记已删除条目
6. 完成 → 密码箱在新设备上完全可用

### 逐项检查

| 步骤 | 状态 | 缺失内容 |
|------|------|----------|
| 1. 克隆 Git 仓库 | ✅ | `vault_git_clone` 已实现 |
| 2. 恢复密钥解锁 | ❌ | `unlock_with_recovery_key` 要求 `security.db` 已存在，新设备没有则返回 `NotInitialized` |
| 3. 验证完整性 | ✅ | `verify_vault_meta_integrity` 已实现 |
| 4. 导入全部条目 | ❌ | `import_all` 方法不存在，仅有单条 `import_entry`；缺少 `SecurityDbState::create_entry_from_blob()` 桥接 |
| 5. 导入删除记录 | ❌ | 无对应方法读取 `deleted.json` 并标记删除 |
| 6. 前端入口 | ❌ | VaultUnlock 中恢复密钥按钮仅在 `isInitialized=true` 时显示，新设备无法进入 |

### 关键代码位置

- **后端阻塞点**：`recovery_key/mod.rs:133-135` — `if !path.exists() { return Err(VaultError::NotInitialized); }`
- **前端阻塞点**：`VaultUnlock.tsx:232` — `{isInitialized && recoveryKeySet && !showRecoveryKey && (`
- **缺失方法**：`sync_engine/mod.rs` — 无 `import_all_entries()` / `import_deleted_records()`
- **缺失 command**：`cmd/vault.rs` — 无 `vault_disaster_recovery`

### 修复方案

1. `recovery_key/mod.rs` — 新增 `disaster_recovery_unlock()` 分支：当 `security.db` 不存在时，从 `.vault-meta` 解密 master_key 后**新建** security.db（init schema + 写入 salt/hash）
2. `sync_engine/mod.rs` — 新增 `import_all_entries()`：`list_entry_files()` → 逐条 `import_entry()` → `SecurityDbState::create_entry_from_blob()` 写入 SQLite
3. `sync_engine/mod.rs` — 新增 `import_deleted_records()`：读取 `deleted.json` → 标记删除
4. `sync_engine/mod.rs` — 新增 `disaster_recovery(git_url, recovery_key, db_path)` 一体化函数
5. `VaultUnlock.tsx` — 新设备时显示"灾难恢复"入口（Git URL + 恢复密钥输入）
6. `cmd/vault.rs` — 新增 `vault_disaster_recovery` Tauri command

---

## 🟡 问题 12：.vault-meta 缺少 masterKeySalt / argon2Params 字段

### 严重度：中

### 关联 spec

spec-5 §13.5 灾难恢复流程

### 问题描述

`VaultMetaFile` 结构体（`vault_file/types.rs:24-31`）当前字段：

```rust
pub struct VaultMetaFile {
    pub version: u32,
    pub cipher_algorithm: String,
    pub entry_count: u32,
    pub last_sync_at: Option<i64>,
    pub emergency: Option<EmergencyDataFile>,
    pub hmac: String,
}
```

灾难恢复在新设备上需要从 `.vault-meta` 重建 `security.db`，这需要：
- `masterKeySalt`：master_key 的 Argon2id 盐值（用于验证主密码）
- `argon2Params`：Argon2id 参数（m/t/p），不同设备可能使用不同参数

当前 `.vault-meta` 不包含这些信息，灾难恢复即使能解密 master_key，也无法重建 `security.db` 的 `master_key_hash` 验证字段。

### 修复方案

`VaultMetaFile` 新增字段（`#[serde(default)]` 兼容旧文件）：

```rust
pub struct VaultMetaFile {
    // ...existing fields...
    #[serde(default)]
    pub master_key_salt: Option<String>,   // base64(argon2id_salt)
    #[serde(default)]
    pub argon2_params: Option<Argon2ParamsFile>,
}

pub struct Argon2ParamsFile {
    pub memory_cost: u32,   // m
    pub time_cost: u32,     // t
    pub parallelism: u32,   // p
}
```

`write_vault_meta()` 时从 `SecurityDbState` 读取 salt 和参数一并写入。

---

## 🔴 问题 13：VaultMetaFile.emergency 仅存 token+salt，灾难恢复无法重建 security.db

### 严重度：严重

### 关联 spec

spec-5 §13.2 恢复密钥方案

### 问题描述

`EmergencyDataFile`（`vault_file/types.rs:35-38`）当前仅包含：

```rust
pub struct EmergencyDataFile {
    pub token: String,   // base64(nonce‖encrypted_master_key‖tag)
    pub salt: String,    // base64(argon2id_salt)
}
```

恢复密钥解锁流程（`recovery_key/mod.rs`）：
1. 从 `emergency.token` + `emergency.salt` 解密得到 `master_key` ✅
2. 从 `master_key` 派生 `sqlcipher_key` ✅
3. 用 `sqlcipher_key` 打开 `security.db` ✅（**但新设备没有 security.db**）

### 修复方案（已更新）

采用"只读解密 + JSON 导出"方案，不重建 `security.db`。

灾难恢复定位为**独立的只读解密工具**：从 Git 仓库 + 恢复密钥解密全部条目，导出为 JSON 文件（与 `vault_export` 格式一致）。用户自行创建新密码库 + `vault_import` 导入。

详见 spec-5 §13.5（2026-10-22 更新）。
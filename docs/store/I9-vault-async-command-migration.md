# Vault Tauri 命令同步→异步迁移

## 背景

Tauri v2 的同步 `#[command]` 在主线程（IPC 线程）上执行。当命令内部包含阻塞式系统调用（网络 I/O、磁盘 I/O、平台生物识别弹窗）时，会卡住 webview 渲染线程，导致：

1. 前端 `loading` 状态变更后 React 无法 re-render，用户看不到 spinner
2. 主窗口白屏/无响应，直到命令返回
3. 连续操作时 UI 完全冻结

## 解决方案

将耗时命令改为 `async`，用 `tauri::async_runtime::spawn_blocking` 将阻塞操作卸到 Tokio 线程池：

```rust
#[command]
pub async fn vault_xxx(app: AppHandle) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<SomeState>();
        // 阻塞操作...
    })
    .await
    .map_err(|e| e.to_string())?
}
```

### 关键约束

- `AppHandle` 是 `Clone + Send + Sync + 'static`，可 move 进 `spawn_blocking` 闭包
- `tauri::State<'_, T>` 带生命周期不可跨线程，需在闭包内通过 `app.state::<T>()` 重新获取
- `SecurityDbState` / `SyncEngine` 内部用 `Mutex` 保护，是 `Send + Sync`
- 生物识别平台弹窗不依赖调用线程的事件泵（Windows: MTA + `AllowSetForegroundWindow`；macOS: Security Server 进程管理）

## 修改清单

### biometric.rs — 生物识别（系统弹窗阻塞）

| 命令 | 改前 | 改后 | 耗时原因 |
|------|------|------|----------|
| `vault_unlock_biometric` | `fn` | `async fn` + `spawn_blocking` | Windows Hello / Touch ID 系统弹窗 |
| `vault_enroll_biometric` | `fn` | `async fn` + `spawn_blocking` | 同上 + 可能需要 unlock |
| `vault_disenroll_biometric` | `fn` | `async fn` + `spawn_blocking` | 删除平台密钥 |

未改（无需）：

| 命令 | 原因 |
|------|------|
| `vault_is_biometric_available` | 纯内存检查，微秒级 |
| `vault_is_biometric_enrolled` | 纯内存检查，微秒级 |

### sync.rs — Git 同步（网络 I/O 阻塞）

| 命令 | 改前 | 改后 | 耗时原因 |
|------|------|------|----------|
| `vault_sync_pull` | `fn` | `async fn` + `spawn_blocking` | Git pull，网络 I/O |
| `vault_sync_push` | `fn` | `async fn` + `spawn_blocking` | Git push，网络 I/O |
| `vault_sync_now` | `fn` | `async fn` + `spawn_blocking` | pull + push |
| `vault_sync_force_push` | `fn` | `async fn` + `spawn_blocking` | Git force push |
| `vault_git_clone` | `fn` | `async fn` + `spawn_blocking` | Git clone，可能数十秒 |

未改（无需或极轻量）：

| 命令 | 原因 |
|------|------|
| `vault_get_sync_config` | 读 Mutex 内字段，微秒级 |
| `vault_set_sync_config` | 写 Mutex 内字段，微秒级 |
| `vault_resolve_conflict` | 单文件操作，毫秒级 |
| `vault_git_init` | `git init`，毫秒级 |
| `vault_git_add_remote` | 配置写入，毫秒级 |
| `vault_git_set_remote` | 配置写入，毫秒级 |
| `vault_sync_readyness` | 状态检查，微秒级 |
| `vault_sync_prepare` | 文件暂存，毫秒级 |
| `vault_sync_finalize` | commit，毫秒级 |

### entries.rs — Vault 条目操作（磁盘/重加密阻塞）

| 命令 | 改前 | 改后 | 耗时原因 |
|------|------|------|----------|
| `vault_change_master_password` | `fn` | `async fn` + `spawn_blocking` | 重加密全库 + rewrite vault-meta |
| `vault_export_to_file` | `fn` | `async fn` + `spawn_blocking` | 序列化 + `fs::write` |
| `vault_import` | `fn` | `async fn` + `spawn_blocking` | `fs::read` + 反序列化 + 批量写入 DB |

未改（无需）：

| 命令 | 原因 |
|------|------|
| `vault_list_entries` | DB 查询，毫秒级 |
| `vault_get_entry` | DB 查询，微秒级 |
| `vault_create_entry` | 单条写入，毫秒级 |
| `vault_update_entry` | 单条写入，毫秒级 |
| `vault_delete_entry` | 单条删除，毫秒级 |

### recovery.rs — 恢复密钥（网络 + 磁盘阻塞）

| 命令 | 改前 | 改后 | 耗时原因 |
|------|------|------|----------|
| `vault_disaster_recovery_export` | `fn` | `async fn` + `spawn_blocking` | git clone/pull + 解密 + 序列化 + 写文件 |

未改（无需）：

| 命令 | 原因 |
|------|------|
| `vault_generate_recovery_key` | 内存加密 + 单次文件写入，毫秒级 |
| `vault_unlock_with_recovery_key` | 内存解密，毫秒级 |
| `vault_has_recovery_key` | 文件存在检查，微秒级 |
| `vault_verify_vault_meta_integrity` | HMAC 校验，毫秒级 |

### audit.rs — HIBP 审计

已为 `async`，无需修改：

| 命令 | 状态 |
|------|------|
| `vault_hibp_check` | 已 `async` |
| `vault_hibp_audit` | 已 `async` |

## 前端 UI 反馈现状

所有耗时操作的前端已有 loading 状态，之前因主线程阻塞无法 re-render，现在异步化后可正常工作：

| 操作 | UI 组件 | loading 来源 |
|------|---------|-------------|
| sync pull/push/now/forcePush | `VaultSyncSettings` | `syncing` + `Spin` + 按钮 `loading` |
| git clone | `VaultSyncSettings` | `loading` 状态 |
| changeMasterPassword | `ChangePasswordTab` | `loading` + 按钮 `loading` |
| exportVault | `ImportExportTab` | `loading` prop + 按钮 `loading` |
| importVault | `ImportExportTab` | `loading` prop + 按钮 `loading` |
| biometric enroll | `ChangePasswordTab` Modal | `reEnrollLoading` + 按钮 `loading` |
| biometric unlock | `BiometricButton` | `unlockMethod === "biometric"` loading |
| disaster recovery export | `DisasterRecovery` | `loading` prop |

## 涉及文件

| 文件 | 变更类型 |
|------|----------|
| `src-tauri/src/cmd/vault/biometric.rs` | 3 个命令改为 async |
| `src-tauri/src/cmd/vault/sync.rs` | 5 个命令改为 async |
| `src-tauri/src/cmd/vault/entries.rs` | 3 个命令改为 async |
| `src-tauri/src/cmd/vault/recovery.rs` | 1 个命令改为 async |

## 后续研究方向

1. **进度事件**：长时间操作（git clone、changeMasterPassword）可通过 Tauri event 向前端发送进度百分比
2. **取消机制**：`spawn_blocking` 返回 `JoinHandle`，可研究 `abort` 或用 `CancellationToken` 支持用户取消
3. **并发控制**：多个 sync 操作并发时可能产生 Git 锁冲突，需在前端或后端加互斥
4. **超时保护**：网络操作应设超时，避免无限等待
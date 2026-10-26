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

1. ~~**进度事件**：长时间操作（git clone、changeMasterPassword）可通过 Tauri event 向前端发送进度百分比~~ → 见下方详细方案
2. **取消机制**：`spawn_blocking` 返回 `JoinHandle`，可研究 `abort` 或用 `CancellationToken` 支持用户取消
3. **并发控制**：多个 sync 操作并发时可能产生 Git 锁冲突，需在前端或后端加互斥
4. ~~**超时保护**：网络操作应设超时，避免无限等待~~ → 见下方详细方案

---

## 超时保护方案

> **与 I20 依赖关系**：`vault_sync(action)` 命令合并（I20 简化 7）与超时/进度改造强耦合 — 同一个命令的签名和执行模式需同时确定。建议 `vault_sync` 合并命令直接用 `tokio::process::Command` + timeout + kill + 流式进度实现，避免先合再用 `spawn_blocking` 再重构。底层改造（`git.rs` 超时配置、`ssh.rs` SSH keepalive）独立，不依赖 I20。

### 现状

- ~~`spawn_blocking`/`.await` 无超时包裹，网络挂起时永远阻塞~~ → Phase 2 待实施
- ~~`cmd_git()` 只设 `GIT_TERMINAL_PROMPT=0`，无 `http.timeout` 等 Git 配置~~ → ✅ Phase 1 已实施
- ~~SSH `ConnectTimeout=10` 仅 Unix 生效，Windows 缺失；SSH 数据传输阶段无超时~~ → ✅ Phase 1 已实施

日常 Git 的"超时"实为下层被动失败（OS TCP SYN 重试 ~21s/130s、远端断连、NAT 过期、DNS 上限），**关键盲区是"慢但活着"的连接** — 无任何层会超时。

### 两层超时策略

终止一个操作只有两种方式：让它自己退出（优雅），或从外部杀掉它（强制）。不需要更多层。

#### 第一层：让 Git 自己退出

通过 Git 配置让所有协议的操作都能自超时，子进程正常退出，无需外部清理：

| 协议 | 配置 | 覆盖范围 |
|------|------|---------|
| HTTP | `http.timeout=30` + `http.lowSpeedLimit=1000` + `http.lowSpeedTime=60` | 连接 + 传输 |
| SSH | `ConnectTimeout=10` + `ServerAliveInterval=15` + `ServerAliveCountMax=4` | 连接 + 传输 |

SSH `ServerAliveInterval/CountMax`：每 15s 发 keepalive 探测，连续 4 次无响应（60s）后断开，Git 收到连接关闭后退出。填补了 SSH 数据传输盲区。

Windows 补 `ConnectTimeout` + `ServerAliveInterval`（通过 `core.sshCommand` 或 `GIT_SSH_COMMAND` 环境变量）。

#### 第二层：退不了就强杀

用 `tokio::process::Command` 替代 `spawn_blocking`，超时后 `child.kill()` 强制终止子进程：

- 覆盖 Git 自身 bug、配置遗漏等不可预见情况
- 真正清理资源（对比 `tokio::time::timeout + spawn_blocking`，后者只放弃 `.await`，不终止线程和子进程，会泄漏）
- CPU 操作（重加密、生物识别）无需超时 — 纯计算必定完成，加超时反而引入线程泄漏和操作冲突风险

### 超时阈值

基于仓库大小预估（典型 200 条目 ~500 KB，重度 500 条目 ~1.2 MB）：

| 操作 | 超时 | 依据 |
|------|------|------|
| `pull/push/sync_now/force_push` | 60 s | 一般网络 ~8s，差网络 ~60s |
| `git_clone` | 180 s | 大仓库慢网络 2~5 min |
| `disaster_recovery` | 300 s | 含 clone + 解密 + 写文件 |

### 涉及文件

| 文件 | 变更 | I20 联动 |
|------|------|----------|
| `git_sync/git.rs` | `cmd_git()` 加 HTTP 超时配置；网络操作改用 `tokio::process::Command` + `timeout` + `child.kill()` | — |
| `git_sync/ssh.rs` | 补 SSH `ServerAliveInterval/CountMax`（含 Windows） | — |
| `cmd/vault/sync.rs` | `vault_sync(action)` 直接用异步子进程 + 超时 + 进度（**与 I20 简化 7 同步实现**） | ✅ |
| `cmd/vault/recovery.rs` | 1 命令改用异步子进程 + 超时 | — |

---

## 进度事件方案

### 现状

- Git 进度走 stderr，当前 `.status()` 全部丢弃
- `git_clone()` 加了 `--progress` 但最终走 `.status()` 无效
- 纯计算操作（重加密、导入/导出）无进度反馈

### Git 进度机制

加 `--progress` + stderr 非 tty 时，Git 输出机器可读进度行（`\n` 分隔）：
`Receiving objects:  47% (123/256), 1.2 MiB | 2.3 MiB/s`

### 各操作进度来源

| 操作 | Git 原生 | 自造 | 参数 |
|------|---------|------|------|
| clone/pull/push/fetch | ✅ Receiving/Resolving/Writing objects | — | `--progress` |
| change_master_password | — | ✅ "Re-encrypting 47/200" | callback |
| import/export | — | ✅ "Importing 12/50" | callback |
| disaster_recovery | ✅ clone 阶段 | ✅ 解密/写文件阶段 | 两者结合 |

### 事件协议

事件名 `vault:sync-progress`，payload：

| 字段 | 类型 | 含义 |
|------|------|------|
| `operation` | string | "clone" / "pull" / "push" / "changeMasterPassword" / ... |
| `phase` | string | "receiving-objects" / "resolving-deltas" / "re-encrypting" / ... |
| `current` | u64 | 已完成数 |
| `total` | u64 | 总数 |
| `bytesTransferred` | u64? | 已传输字节数 |

前端 `listen("vault:sync-progress", ...)` → `percent = current/total * 100` 更新进度条。

### 实现要点

- **Git 网络操作**：`std::process::Command` → `tokio::process::Command`，`.stderr(Stdio::piped())` + `BufReader::lines()` 流式读取 → 正则解析进度行 → `app.emit()`
- **纯计算操作**：`SecurityDbState` 重加密/导入接口加 `Fn(usize)` callback，循环中 emit
- **架构影响**：网络操作须从 `spawn_blocking` 迁移到纯 `async fn`（`spawn_blocking` 内无法做 async I/O）；与超时方案对齐，共用 `tokio::process::Command`

### 涉及文件

| 文件 | 变更 | I20 联动 |
|------|------|----------|
| `git_sync/git.rs` | 网络操作改用 `tokio::process::Command` + 流式读 stderr | — |
| `git_sync/progress.rs` | 新增：Git 进度行正则解析 | — |
| `types/mod.rs` | 新增 `SyncProgress` 类型 | — |
| `cmd/vault/sync.rs` | `vault_sync(action)` emit 进度事件（**与 I20 简化 7 同步实现**） | ✅ |
| `cmd/vault/entries.rs` | changeMasterPassword/import/export 加进度回调 | — |
| `cmd/vault/recovery.rs` | disaster_recovery_export 加进度事件 | — |
| `security_db/rekey.rs` | `change_master_password` 接口加 callback | — |
| `security_db/import_export.rs` | `import_entries` 接口加 callback | — |
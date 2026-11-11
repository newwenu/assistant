# I9 密码箱 — 运行时密钥与会话驻留规格（S4）

> 状态：规格（2026-11-11）
> 上游：[I9-vault-key-hierarchy-design.md](./I9-vault-key-hierarchy-design.md)、[I9-vault-key-hierarchy-spec.md](./I9-vault-key-hierarchy-spec.md)
> 同层：[I9-vault-entry-cipher-spec.md](./I9-vault-entry-cipher-spec.md)（条目字节）、[I9-vault-plaintext-egress-spec.md](./I9-vault-plaintext-egress-spec.md)（明文出口）
> 生物识别 payload、keystore 与重包细节见 [I9-vault-biometric-spec.md](./I9-vault-biometric-spec.md)；本规格只负责它们在会话中的清理与锁定。
> 本规格只定义**解锁到锁定之间，密钥与明文在进程/前端/系统里的驻留与清理**。
> 路径约定：全部为仓库相对路径。

---

## 0. 范围

| 驻留点 | 文件 | 现状 |
|---|---|---|
| 后端会话状态 | `security_db/mod.rs` | `conn` / `master_key` / `bootstrap_key` 三个 `Mutex<Option<_>>` |
| 解锁/锁定入口 | `cmd/vault/lifecycle.rs`、`cmd/vault/mod.rs` | 口令、生物识别、原生窗口、屏幕锁、退出 |
| 自动锁定 | `src/vault/core/slices/vaultLifecycleSlice.ts`、`useVaultAutoLock.ts` | 前端计时器 |
| 前端明文 | `vaultEntriesSlice.ts`、`vaultRevealSlice.ts` | 条目摘要/详情/揭示值 |
| 剪贴板 | `assistant-security/src/clipboard_guard.rs` | 定时清除 |
| 揭示 | `assistant-security/src/reveal_guard.rs`、`cmd/vault/reveal.rs` | 速率限制 + TTL |
| 备份 | `security_db/mod.rs`、`security_db/rekey.rs` | 保留 5 份 |
| 原生解锁窗口 | `assistant-security/src/unlock_dialog/*` | 口令明文驻留 |
| SSH agent | `cmd/vault/mod.rs`、`cmd/vault/ssh.rs` | 锁定时移除 |
| 进程退出 | `cmd/vault/mod.rs`、`vault_plugin.rs` | 仅清剪贴板 |

不覆盖：磁盘上的密文格式（S2/S3）、导出文件（S5）、操作系统层面的内存转储/交换分区（见 §5 的边界声明）。

---

## 1. 现状

### 1.1 会话状态

```rust
pub struct SecurityDbState {
    conn: Mutex<Option<Connection>>,
    master_key: Mutex<Option<MasterKey>>,
    bootstrap_key: Mutex<Option<MasterKey>>,
    db_path: String,
}
```

- `MasterKey` 实现 `Zeroize + Drop`，锁定时置 `None` 触发清零。
- `Connection` 由 SQLCipher 持有页缓存；未设置 `cipher_memory_security`。
- `with_unlocked`：

```rust
pub(crate) fn with_unlocked<T>(app: &AppHandle, f: impl FnOnce(&SecurityDbState) -> Result<T, VaultErrorDto>) -> Result<T, VaultErrorDto> {
    let state = vault_state(app)?;
    drop(state.get_conn().map_err(VaultErrorDto::from)?);   // 先确认已解锁，再释放守卫
    f(state.inner())
}
```

### 1.2 锁定触发点

| 触发 | 入口 | 清理内容 |
|---|---|---|
| 用户点击锁定 | `vault_lock` → `state.lock()` → `on_locked()` | 密钥、连接、剪贴板、SSH、揭示限速、广播事件 |
| 自动锁定 | 前端 `setTimeout` → `vault_lock` | 同上 |
| 系统锁屏 | `ScreenLockMonitor` → `state.lock()` → `on_locked()` | 同上，并按需弹出解锁窗口 |
| 应用退出 | `vault_cleanup_on_exit` | **仅剪贴板** |
| 崩溃 | 无 | 无 |

### 1.3 缺陷

| 编号 | 问题 | 后果 | 证据 |
|---|---|---|---|
| **F8** | 剪贴板守卫把明文密码与原文长期保存在 `Arc<Mutex<String>>`，无 `zeroize` | 锁定后内存中仍可能残留上次复制的密码 | `clipboard_guard.rs:8-117` |
| **F9** | 揭示明文存于前端 store，TTL 固定 30s，锁定外无主动清零 | 用户切走页面但未锁定时，明文长期在 JS 堆中 | `vaultRevealSlice.ts:41-67`、`cmd/vault/reveal.rs:7` |
| **F10** | SQLCipher 未设置 `cipher_memory_security` / `temp_store` | 页缓存与临时表可能落盘或驻留 | `security_db/mod.rs:27-28` |
| **F13** | 改密前的 `.bak` 保留旧密钥代际 | 旧口令可打开旧备份并读出全部条目 | `rekey.rs:44`、`mod.rs:248-330` |
| **F15** | `SecureString::into_inner` 用 `mem::forget` 交出未清零 `String` | 后续副本不再受 Drop 保护 | `vault_crypto/types.rs:88-93` |
| **F17** | `with_unlocked` 先释放连接守卫再执行业务 | 锁定与业务之间存在竞态窗口 | `cmd/vault/mod.rs:168-175` |
| **F19** | 审计只在 `security.db` 内，未解锁/恢复态不可写 | 锁定期间的敏感动作不可审计 | `audit/mod.rs:61-76` |
| **F27** | 退出只清剪贴板，不清密钥/SSH/前端状态 | 进程退出前仍有明文驻留；SSH agent 可能保留密钥 | `cmd/vault/mod.rs:310-313` |
| **F28** | 原生解锁窗口的 `SecureString` 生命周期未在规格中约定 | 口令可能被平台窗口缓冲区保留 | `assistant-security/src/unlock_dialog/*` |

---

## 2. 目标状态机

### 2.1 会话状态

```
Locked ──unlock(口令/生物识别)──▶ Unlocked ──lock(用户/自动/锁屏/退出)──▶ Locked
   ▲                                      │
   └──────────── 失败（不改变状态）────────┘
```

规则：

1. `Unlocked` 状态**有且只有一份** `dek` / `dbKey` / `conn`，均由 `SecurityDbState` 独占持有。
2. 解锁失败（口令错、生物识别失败、探针失败）**不得**写入任何部分状态；要么全部成功，要么保持 `Locked`。
3. `Locked` 状态不得残留任何明文快照、剪贴板内容、SSH 私钥、前端条目缓存。

### 2.2 `SecurityDbState` 目标结构

```rust
pub struct SecurityDbState {
    session: Mutex<Option<VaultSession>>,
    db_path: String,
    epoch: AtomicU64,          // 每次 lock +1；用于拒绝过期会话
}

struct VaultSession {
    conn: Connection,
    dek: Dek,
    db_key: DbKey,
    epoch: u64,
}
```

好处：

- 三个 `Mutex` 合并为一个，`Locked` 时不可能出现「有 conn 无 key」或「有 key 无 conn」的半状态。
- `epoch` 让 `with_unlocked` 可以检测「业务执行期间被锁定」，避免 F17 的竞态。
- `Dek` / `DbKey` 不实现 `Clone`，会话内只有一份。

### 2.3 `with_unlocked` 目标语义

```rust
pub(crate) fn with_unlocked<T>(
    app: &AppHandle,
    f: impl FnOnce(&VaultSession) -> Result<T, VaultErrorDto>,
) -> Result<T, VaultErrorDto> {
    let state = vault_state(app)?;
    state.with_session(|session| f(session))
}
```

`with_session` 必须：

1. 在同一把锁内取得 `&VaultSession` 并调用 `f`；
2. 若 `f` 执行期间 `epoch` 变化（锁定发生），返回 `VaultError::Locked`，不得返回部分结果；
3. 禁止把 `&Connection` 或 `&Dek` 泄露到闭包外。

---

## 3. 锁定与清理

### 3.1 唯一锁定路径

所有锁定必须走：

```rust
fn lock_session(&self, reason: LockReason) -> Result<(), VaultError>;
```

`LockReason` 取值：`User` / `AutoLock` / `ScreenLock` / `AppExit` / `Reset` / `Panic`。

`lock_session` 的执行顺序（不可调换）：

```
1. epoch += 1（先让所有在途业务失效）
2. 取走 session（dek / dbKey / conn 同时 drop）
3. conn.close()（SQLCipher 关闭，释放页缓存）
4. 清剪贴板（ClipboardGuard::force_clear）
5. 移除 SSH agent 中的全部本应用注入的 key
6. 清揭示限速与前端可见状态（通过事件通知）
7. 审计 VaultLock{reason}
8. 广播 VAULT_LOCKED
```

约束：

- 第 1 步必须最先，避免第 4–5 步执行期间还有业务线程持旧会话。
- 第 4/5 步失败只 `warn`，不回滚锁定。
- `AppExit` 与 `Panic` 也必须走同一函数；`vault_cleanup_on_exit` 改为调用 `lock_session(AppExit)`。

### 3.2 退出与崩溃

| 场景 | 要求 |
|---|---|
| 正常退出 | `vault_plugin::on_exit` → `lock_session(AppExit)`；然后清剪贴板兜底 |
| 窗口关闭但进程存活 | 不自动锁定（保持现状），由自动锁定兜底 |
| 进程崩溃 | 无可靠清理；依赖 OS 进程内存回收。**必须**保证「密钥不落盘、不写日志、不写崩溃报告」 |
| 系统休眠 | 不额外处理；恢复后由自动锁定/锁屏事件处理 |

崩溃相关的禁止项：

1. 任何 `Panic` 信息不得包含 `dek` / `dbKey` / 口令 / 明文字段；
2. 禁止在 `unwrap` / `expect` 的消息里拼接密钥材料；
3. 禁止把 `VaultSession` 放进会实现 `Debug` 的结构体。

### 3.3 自动锁定

1. 计时器只在前端；后端不重复计时。
2. `vault_auto_lock_minutes` 为 `0` 时表示「不自动锁定」，必须显式支持，不能当作 5 分钟。
3. 前端 `lock()` 必须清空：`entries`、`selectedEntry`、`revealedField`、`hibpLeaks`、`searchQuery`、`sync` 预览、恢复表单。
4. 锁定事件必须通过 `VAULT_LOCKED` 广播给所有窗口；多窗口场景下每个窗口都要清空。

### 3.4 屏幕锁

1. `vault_lock_on_screen_lock = true` 时，锁屏即 `lock_session(ScreenLock)`。
2. 锁屏触发的解锁窗口必须走原生窗口，不把口令留在 WebView。
3. 解锁窗口取消/失败后保持锁定。

---

## 4. 内存清理

### 4.1 后端

| 对象 | 要求 |
|---|---|
| `Dek` / `DbKey` / `Kek` / `MetaKey` | `Zeroize + Drop`，不实现 `Clone` |
| `MasterKey`（v1 兼容期） | 同上 |
| `SecureString` | 保留；新增 `into_zeroizing() -> Zeroizing<String>`；`into_inner()` 标记为 `#[deprecated]`，仅允许在立即消费的场景使用 |
| `VaultPlaintext` | 保持 `Drop` 清零；禁止把它转成 `String` 后再长期持有 |
| `Connection` | `lock_session` 时显式 `close()`；禁止把 `Connection` 放到 `Arc` 里跨线程共享 |
| SQLCipher | 启用 `PRAGMA cipher_memory_security = ON`（若当前 SQLCipher 版本支持）；`PRAGMA temp_store = MEMORY` |
| 剪贴板守卫 | 内部明文改为 `Zeroizing<String>`；`Drop` 时 `force_clear` |
| SSH agent | 锁定时移除；注入失败时不得保留私钥副本 |

### 4.2 前端

| 位置 | 要求 |
|---|---|
| `revealedField.value` | TTL 由 `VaultConf` 提供（默认 30s）；锁定/隐藏/切换条目时立即置 `null` |
| `selectedEntry`（`VaultEntryDetail`） | 锁定、切换条目、关闭抽屉时清空。注意：当前 DTO 只含 `has_password` / `has_notes` / `has_totp` 等**标志位与元数据**，不含密码/备注正文；正文只经 `revealedField` 出现 |
| 列表摘要 `entries` / `deletedEntries` | 锁定、筛选重置时清空（含 title/username/url/tags 元数据） |
| `vaultBiometricSlice` | `BiometricReset` 必须清空 `biometricEnrolled`；当前代码把 `crypto` / `notFound` 当重置信号，需改为只认 `biometricReset` |
| `console.log` / 错误上报 | 禁止输出任何字段值；错误只上报 code + message |
| React DevTools / 状态快照 | 不额外处理；文档声明「明文在解锁期间存在于 WebView 内存」 |
| 复制到剪贴板 | 走 `vault_copy_*`，不把明文写入前端 store |

### 4.3 备份

改密/轮换产生的 `security/backups/*.bak` 是旧密钥代际的完整数据库。要求：

1. **改密成功后**，必须处理旧备份，二选一（由实现固定其一，不得留给用户）：
   - 方案 A：删除改密前的全部备份；
   - 方案 B：用新密钥对备份重新 `PRAGMA rekey`（备份本身是 SQLCipher 库）。
2. 推荐方案 A：备份只是「可重建缓存」的兜底，条目真身在 `entries/*.enc`；删除旧代备份的代价小于保留旧口令可用面。
3. 备份目录必须在仓库白名单之外（sync-crypto-spec §2.1）。
4. 备份文件权限 0600（平台允许时）。

### 4.4 原生解锁窗口

1. 口令在窗口内以 `SecureString` 持有，窗口销毁时清零。
2. 禁止把口令写入窗口标题、日志、剪贴板。
3. 失败重试不缓存上一次口令。
4. 窗口关闭（取消）必须返回 `Cancelled`，不得返回空口令继续尝试。

---

## 5. 边界声明（必须写入用户可见文案/开发者文档）

以下内容**不在**本规格保护范围内，不得宣传为已防护：

1. 同用户权限的进程可读取本进程内存；
2. 操作系统交换分区、休眠文件、崩溃转储；
3. 调试器、内存扫描器；
4. 屏幕截图/录屏；
5. 解锁期间 WebView 中存在的明文（前端无法保证零化）；
6. SSH agent 自身的密钥缓存策略（超出本应用控制）。

---

## 6. 文件改动清单

| 文件 | 改动 |
|---|---|
| `src-tauri/crates/assistant-vault/src/security_db/mod.rs` | `SecurityDbState` 合并为 `session: Mutex<Option<VaultSession>>` + `epoch`；`lock` 改为 `lock_session(LockReason)`；SQLCipher PRAGMA 加固 |
| `src-tauri/crates/assistant-vault/src/security_db/rekey.rs` | 改密成功后处理旧备份（§4.3） |
| `src-tauri/crates/assistant-crypto/src/vault_crypto/types.rs` | `SecureString::into_zeroizing`；`into_inner` 标记弃用；新增 `Dek` / `DbKey` |
| `src-tauri/crates/assistant-security/src/clipboard_guard.rs` | 内部 `Zeroizing<String>`；`Drop` 清理；不再无限期保存原文 |
| `src-tauri/crates/assistant-security/src/unlock_dialog/*` | 口令生命周期与取消语义（§4.4） |
| `src-tauri/src/cmd/vault/mod.rs` | `with_unlocked` 改为 `with_session`；`on_locked` 统一走 `lock_session`；退出调用 `lock_session(AppExit)` |
| `src-tauri/src/cmd/vault/lifecycle.rs` | 锁定/解锁错误路径不变状态；失败不半解锁 |
| `src-tauri/src/vault_plugin.rs` | 退出清理顺序：先 lock 再清剪贴板 |
| `src/vault/core/slices/vaultLifecycleSlice.ts` | `lock()` 清空全部明文切片；`autoLockMinutes=0` 支持 |
| `src/vault/core/slices/vaultRevealSlice.ts` | TTL 来自配置；切换/隐藏/锁定即清空 |
| `src/vault/core/slices/vaultEntriesSlice.ts` | 锁定时清空摘要与详情 |
| `src/vault/core/useVaultEvents.ts` | 监听 `VAULT_LOCKED`，多窗口同步清空 |

---

## 7. 测试

| 编号 | 用例 | 断言 |
|---|---|---|
| T-R1 | 解锁 → 锁定 | `session=None`；`conn` 已关闭；`dek`/`dbKey` Drop |
| T-R2 | 锁定后调用任意业务命令 | `Locked`，不返回部分结果 |
| T-R3 | 业务执行中触发锁定 | 业务返回 `Locked`（epoch 失配），不写入 |
| T-R4 | 解锁失败（口令错/生物识别失败） | 状态仍为 `Locked`；无 `conn`/`dek` 残留 |
| T-R5 | 剪贴板复制 → 锁定 | 剪贴板被清空；内部 `Zeroizing` 已清零 |
| T-R6 | 揭示 → 锁定 | 前端 `revealedField=null` |
| T-R7 | 自动锁定 `0` 分钟 | 不触发自动锁定 |
| T-R8 | 屏幕锁事件 | 走 `lock_session(ScreenLock)`；审计存在 |
| T-R9 | 退出 | `lock_session(AppExit)` 被调用；SSH 清空；剪贴板清空 |
| T-R10 | 改密后旧备份 | 旧 `.bak` 不存在（方案 A）或旧口令不可打开（方案 B） |
| T-R11 | 日志扫描 | 所有日志/错误消息不含 `dek`/`dbKey`/口令/明文字段 |
| T-R12 | 多窗口 | 一个窗口锁定后，另一个窗口的条目/揭示被清空 |

测试落点：`src-tauri/crates/assistant-vault/tests/security_db.rs`（会话状态、探针读、备份代际）；前端 `src/vault/core/__tests__/vaultStore.test.ts` 扩展「锁定清空」用例。

---

## 8. 禁止事项

1. **禁止**在 `Locked` 状态下保留任何 `Connection` / `Dek` / `DbKey`。
2. **禁止**把 `Dek` / `DbKey` 实现 `Clone` / `Debug` / `Serialize`。
3. **禁止**在日志、错误、panic、崩溃报告、审计 detail 中出现口令或明文字段值。
4. **禁止**用「先检查已解锁、再执行业务」的两段式代替真正的会话锁。
5. **禁止**在改密后保留旧口令可打开的数据库备份。
6. **禁止**把 `SecureString::into_inner` 的结果长期持有。
7. **禁止**在关闭/退出路径只清剪贴板而不清会话。

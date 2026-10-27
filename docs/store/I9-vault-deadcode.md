# Vault 死代码问题与修复方案

> 审查日期：2026-10-22
> 审查方法：对 vault 全部 Rust + TypeScript 代码做定义-引用交叉比对，识别定义了但未被使用/未接入的代码；再逐条与规格文档核实，区分"偏差残留""设计意图未落地""规格未完成""超规格添加""代码自创冗余"
> 涉及 crate：assistant-security、assistant-storage、assistant-sync、src-tauri/src/cmd/vault.rs
> 涉及前端：vaultStore.ts、vault.service.ts、VaultEntryForm.tsx、VaultSettings.tsx
> 关联文档：I9-vault-spec-1~5、I9-vault-issues.md
> 第一批修复完成日期：2026-10-22（问题 1 + 问题 8 + 附带修复 `VaultCategory::from_str` → `parse`）
> 复核日期：2026-10-27（逐条比对当前代码，勾选已落地验收项，汇总残留项；残留汇总见文末）
> 第二批修复完成日期：2026-10-27（问题 9 大小写 bug + 问题 7 屏幕锁联动与死窗口清理 + 新发现 12 假解锁事件）

---

## 问题总览

| # | 死代码 | 严重度 | 根因 | 文档依据 | 处理 | 状态 |
|---|--------|--------|------|---------|------|------|
| 1 | `assistant-storage::vault_meta` + `vault` | 🔴 高 | 偏差残留 | spec-1 §5.4 偏差 #4 | 删除 | ✅ 已修复 |
| 2 | `SafePassword` + `sha1_hex` | 🔴 高 | 设计意图未落地 | spec 偏差 #25 "更优" | 重构使用 | ✅ 已修复 |
| 3 | `VaultEntryDecrypted` | 🟡 中 | 设计意图未落地 | spec-1 §3.3 + DESIGN §2.3 + spec-4 §11 | 重构导出/同步使用 | 🟡 部分完成（导出已接，同步未接） |
| 4 | `vault_totp_parse_uri` IPC 漏接 | 🟡 中 | 规格未完成 | spec-3 §6.2 | 补全 IPC 三层 | ✅ 已修复 |
| 5 | `audit::query_log` 无 IPC | 🟢 低 | 超规格添加 | 无 | 保留，补 IPC 为增强 | 🟡 部分完成（无 UI 消费方） |
| 6 | `backup()` 内联重复 | 🔴 高 | 实现偏差 | spec-1 §3.3 + 偏差 #5 | 重构调用 | 🟡 部分完成（无 IPC / 无轮转） |
| 7 | `vault_show_unlock_window` 前端未接 | 🟡 中 | 规格未完成 | spec-3 §7.7 + tasklist E4 | 补全前端 | ✅ 已修复（原生对话框 + 屏幕锁联动） |
| 8 | `LAST_REVEAL_EXPIRY` 只写不读 | 🟢 低 | 代码自创冗余 | 无 | 删除 | ✅ 已修复 |
| 9 | ChaCha20 硬编码 Aes | 🟡 中 | 实现偏差 | DESIGN §3.2 + spec-2 §5.6 | 修复读取设置 | ✅ 已修复（同步侧旁支另列 P4） |
| 10 | `generatePassphrase` 未集成 | 🟡 中 | 规格未完成 | spec-3 §6.5 + tasklist D3.4 | UI 集成 | ✅ 已修复 |

---

## 🔴 问题 1：`assistant-storage::vault_meta` + `vault` — 偏差残留整模块死代码

### 严重度：高

### 根因

项目早期 vault 存储逻辑放在 `assistant-storage` crate（普通 SQLite），安全重构后全部迁到 `assistant-security` crate（SQLCipher + 加密），但旧代码未清理。

### 文档依据

- [spec-1 §5.4 偏差 #4](./I9-vault-spec-1-foundation.md)：**"原规格 §3.5 要求在 `assistant-storage` 中新增 `vault/mod.rs` + `vault_meta/mod.rs` 模块，实际 CRUD 逻辑直接在 `SecurityDbState` 内联"**，结论：功能等价，架构更内聚。
- [spec 偏差表 #4](./I9-vault-spec.md)：`assistant-storage::vault` 模块 → `SecurityDbState` 内联，标注"等价"。

### 重复对比

| `assistant-storage::vault_meta` | `assistant-security::security_db::meta` |
|---|---|
| `get_meta(conn, key)` | `get_meta(conn, key)` — 完全相同 |
| `set_meta(conn, key, value)` | `set_meta(conn, key, value)` — 完全相同 |
| `is_vault_initialized(conn)` | `is_vault_initialized(conn)` — 完全相同 |
| — | `get_salt` / `set_salt` / `get_master_key_hash` / `set_master_key_hash` / `delete_meta` / `ensure_schema_version` — 仅 security 有 |

### 引用情况

- `assistant-storage::vault_meta` — **零引用**
- `assistant-storage::vault::count_entries` — **零引用**
- `assistant-storage::vault::list_entry_summaries` — 被 `assistant-sync::sync_engine::mod.rs:158` 引用 1 次

### 修复方案

1. 迁移 `assistant-sync` 的唯一调用方，改用 `assistant-security::security_db::SecurityDbState::list_entries`
2. 删除 `assistant-storage/src/vault_meta.rs`
3. 删除 `assistant-storage/src/vault.rs`
4. 从 `assistant-storage/src/lib.rs` 移除 `pub mod vault_meta;` 和 `pub mod vault;`

### 验收

- [x] `cargo check -p assistant-storage` 通过（无 vault 相关模块）
- [x] `cargo check -p assistant-sync` 通过（改用 SecurityDbState）
- [x] `cargo check -p assistant-app` 通过
- [x] `assistant-storage/Cargo.toml` 移除 `assistant-security` 依赖
- [x] `assistant-sync/Cargo.toml` 移除 `assistant-storage` 依赖

### 实施记录（2026-10-22）

1. 迁移 `assistant-sync::sync_engine::mod.rs:158`：`assistant_storage::vault::list_entry_summaries(conn)` → `security_db.list_entries(None, None, None)`，同时移除手动 `get_conn()` 调用（`list_entries` 内部管理连接锁）
2. 删除 `assistant-storage/src/vault_meta.rs`、`assistant-storage/src/vault.rs`
3. 从 `assistant-storage/src/lib.rs` 移除 `pub mod vault_meta;` + `pub mod vault;`
4. 从 `assistant-storage/Cargo.toml` 移除 `assistant-security` 依赖（仅被 vault.rs 使用）
5. 从 `assistant-sync/Cargo.toml` 移除 `assistant-storage` 依赖（仅被 vault 引用）
6. 附带修复 `vault.rs:118`：`VaultCategory::from_str(&c)` → `VaultCategory::parse(&c)`，对齐 security crate 内部风格，消除 `use std::str::FromStr` import 需求

---

## 🔴 问题 2：`SafePassword` + `sha1_hex` — HIBP 安全包装未使用

### 严重度：高

### 根因

`SafePassword` 设计用于 HIBP 检查时安全持有密码（Drop zeroize），但 `check_password()` 直接接收 `&str`，在函数体内自己算 SHA1，`SafePassword` 变成了"先设计后忘记使用"。

`sha1_hex()` 同理——`check_password` 内联了 SHA1 计算，`sha1_hex` 只被测试调用。

### 文档依据

- [spec 偏差表 #25](./I9-vault-spec.md)：`HIBP SafePassword` | 无特殊类型 → `SafePassword` + Drop zeroize | **更优**

文档明确标注 `SafePassword` 比无特殊类型"更优"，是**有意设计**，但代码未落地。

### 代码位置

- `src-tauri/crates/assistant-security/src/vault_crypto/hibp.rs:86` — `SafePassword` 定义
- `src-tauri/crates/assistant-security/src/vault_crypto/hibp.rs:79` — `sha1_hex` 定义
- `src-tauri/crates/assistant-security/src/vault_crypto/hibp.rs:22` — `check_password` 未使用两者

### 修复方案

重构 `check_password` 使用 `SafePassword` + `sha1_hex`：

```rust
pub async fn check_password(&self, password: &SafePassword) -> Result<u32, String> {
    let hash_hex = sha1_hex(password.as_str());
    let prefix = &hash_hex[..5];
    let suffix = &hash_hex[5..];
    // ... k-匿名查询逻辑不变
}
```

调用方（`vault_hibp_check` / `vault_hibp_audit`）改为：

```rust
let safe = SafePassword::new(password_string);
let count = hibp.check_password(&safe).await?;
// safe 在此 drop，自动 zeroize
```

`sha1_hex` 改为 `pub(crate)` 限制可见性。

### 验收

- [x] `check_password` 接收 `&SafePassword` 而非 `&str`
- [x] `sha1_hex` 可见性为 `pub(crate)`
- [x] HIBP 审计后密码明文已 zeroize（`SafePassword` drop 验证）

### 复核结论（2026-10-27）

全部落地：`hibp.rs:21` 签名为 `check_password(&self, password: &SafePassword)`；`sha1_hex` 为 `pub(crate)`；`SafePassword` 实现 `Drop` → `zeroize`；调用方 `cmd/vault/audit.rs:125,189` 均以 `SafePassword::new(...)` 包裹。

---

## 🟡 问题 3：`VaultEntryDecrypted` — 全条目解密类型未使用

### 严重度：中

### 根因

两种解密策略的冲突：
- **策略 A**（`VaultEntryDecrypted`）：一次性解密整个条目所有字段，返回完整结构体。适合导出、同步。
- **策略 B**（实际采用）：`reveal_password` / `reveal_notes` / `copy_password` 等逐字段解密。更安全（最小暴露面）。

实际选择了策略 B，但 `export_entries` 绕过 `VaultEntryDecrypted` 自己内联了解密+组装逻辑。

### 文档依据

- [spec-1 §3.3 安全类型表](./I9-vault-spec-1-foundation.md)：`VaultEntryDecrypted` | 与 `VaultEntry` 字段一一对应，`encrypted_*` 替换为 `VaultPlaintext` 明文类型；**不实现 `Clone`**；仅 Rust 端内部使用，不跨 IPC
- [DESIGN §2.3](../VAULT-DESIGN.md)：`VaultEntryDecrypted`（解密后，仅 Rust 端内部使用，不跨 IPC）
- [spec-4 §11 同步流程](./I9-vault-spec-4-git-sync.md)：将 `VaultEntryDecrypted` 各字段通过 `expose()` 读取，构造 `VaultBlob` 明文结构体
- [tasklist A2.1](./I9-vault-tasklist.md)：`VaultEntryDecrypted` **不实现 Clone**（防止解密后多份明文残留）

文档在**三处**定义了 `VaultEntryDecrypted` 的用途：导出、同步、安全约束。

### 代码位置

- `src-tauri/crates/assistant-security/src/types/mod.rs:172` — `VaultEntryDecrypted` 定义
- `src-tauri/crates/assistant-security/src/security_db/mod.rs:1024` — `export_entries` 内联解密，未使用 `VaultEntryDecrypted`

### 修复方案

新增 `decrypt_entry` 方法，让 `VaultEntryDecrypted` 成为导出/同步的统一解密入口：

```rust
impl SecurityDbState {
    pub fn decrypt_entry(&self, id: &str) -> Result<VaultEntryDecrypted, VaultError> {
        self.with_conn_and_key(|conn, mk| {
            // 查询 + 逐字段解密 → 构造 VaultEntryDecrypted
        })
    }

    pub fn export_entries(&self) -> Result<Vec<VaultEntryExport>, VaultError> {
        // 遍历 → decrypt_entry → 转 VaultEntryExport
    }
}
```

同步引擎（`assistant-sync`）的 vault_file 序列化也改用 `decrypt_entry`。

### 验收

- [x] `export_entries` 通过 `decrypt_entry` 实现
- [x] `VaultEntryDecrypted` 不实现 `Clone`（编译验证）
- [ ] `cargo test -p assistant-security` 通过（本次复核未执行测试）
- [ ] 同步引擎（`assistant-sync`）的 vault_file 序列化改用 `decrypt_entry` — **未完成**

### 复核结论（2026-10-27）

- ✅ `crypto_ops.rs:140` 已实现 `decrypt_entry`，`import_export.rs:25` 的 `export_entries` 已改为调用它；`types/mod.rs:174` 的 `VaultEntryDecrypted` 无 `Clone` 实现。
- ❌ 同步侧未接入：`assistant-sync/src/sync_engine/mod.rs:197` 的 `export_all_entries` 仍用 `list_entries` 摘要，且把 `encrypted_password/notes/totp/custom_fields` 全部传 `None`。
- ❌ 更严重：`export_all_entries` **零调用者**，`prepare/pull/push/sync_now` 都不会写 `.enc` 文件，条目实际未进入 Git 仓库。该缺口归属 [tasklist-p2](./I9-vault-tasklist-p2.md) H5.2 / H7.1（仍未勾选），本文件只做指向，不重复跟踪。

---

## 🟡 问题 4：`vault_totp_parse_uri` — Rust 已实现但 IPC 三层漏接

### 严重度：中

### 根因

`from_otpauth_uri()` 在 `totp.rs` 已完整实现，但缺少三层桥接：Tauri command → vault.service.ts → vaultStore.ts。

### 文档依据

- [spec-3 §6.2 TOTP 接口表](./I9-vault-spec-3-security-enhance.md)：`vault_totp_parse_uri(uri) → { secret, digits, period, algorithm }` | 解析 `otpauth://` URI，提取密钥和参数
- [spec-3 §6.6 验收标准](./I9-vault-spec-3-security-enhance.md)：TOTP `otpauth://` URI 解析正确

### 代码位置

- `src-tauri/crates/assistant-security/src/vault_crypto/totp.rs:79` — `from_otpauth_uri` 已实现
- `src-tauri/src/cmd/vault.rs` — **缺少** `vault_totp_parse_uri` command
- `src/tools/password-vault/vault.service.ts` — **缺少** `totpParseUri`
- `src/tools/password-vault/vaultStore.ts` — **缺少** 调用

### 修复方案

补全三层：

1. `vault.rs` 添加 command：
   ```rust
   #[command]
   pub fn vault_totp_parse_uri(uri: String) -> Result<ParsedOtpAuth, String> {
       totp::from_otpauth_uri(&uri)
   }
   ```
2. `lib.rs` 注册 `cmd::vault_totp_parse_uri`
3. `vault.service.ts` 添加：
   ```typescript
   totpParseUri: (uri: string): Promise<ParsedOtpAuth> =>
     invoke("vault_totp_parse_uri", { uri }),
   ```
4. `vaultStore.ts` 添加 `parseOtpUri` action + 状态

### 验收

- [x] 前端可通过 `vault_totp_parse_uri` 解析 `otpauth://totp/` URI
- [x] 解析结果包含 secret / digits / period / algorithm / issuer / account

### 复核结论（2026-10-27）

三层全部接通：`cmd/vault/audit.rs:103` 定义 command → `lib.rs:119` 注册 → `vault.service.ts:117` `totpParseUri` → `vaultAuditSlice.ts:99` → `VaultEntryDrawer.tsx:137` 实际调用。

---

## 🟢 问题 5：`audit::query_log` + `AuditLogEntry` — 只写不读

### 严重度：低

### 根因

`audit::log()` 被广泛使用，但 `query_log()` 没有暴露 IPC command，前端无法查看审计日志。

### 文档依据

- 文档定义了 `security_audit_log` 表和 `audit::log()` 写入（spec-1 §3.7 + spec-2 §4.3）
- **没有**文档提到 `query_log` 或前端审计面板
- 审计日志用途是"事后可检测异常"（spec-2 §4.3），但文档没说"前端可查看"

`query_log` 是**超规格添加**的便利方法。

### 代码位置

- `src-tauri/crates/assistant-security/src/audit/mod.rs:66` — `query_log` 定义
- `src-tauri/crates/assistant-security/src/audit/mod.rs:97` — `AuditLogEntry` 定义
- `src-tauri/src/cmd/vault.rs` — **缺少** `vault_query_audit_log` command

### 修复方案（增强，非规格要求）

1. `vault.rs` 添加 command：
   ```rust
   #[command]
   pub fn vault_query_audit_log(app: AppHandle, limit: Option<i64>) -> Result<Vec<AuditLogEntry>, String> {
       let state = vault_state(&app)?;
       let conn_guard = state.conn.lock().unwrap();
       let conn = conn_guard.as_ref().ok_or("Vault locked".to_string())?;
       audit::query_log(conn, limit.unwrap_or(100)).map_err(|e| e.to_string())
   }
   ```
2. `vault.service.ts` 添加 `queryAuditLog`
3. `vaultStore.ts` 添加审计日志状态 + 加载方法
4. `VaultSettings.tsx` 添加审计日志面板

### 验收

- [x] `vault_query_audit_log` command 已实现并注册
- [x] `vault.service.ts` / `vaultAuditSlice.ts` 已接入
- [ ] 前端可在设置页查看最近 N 条审计日志 — **未完成，无任何 UI 消费方**
- [ ] 日志条目含 event_type / detail / timestamp — 类型已定义，但无展示

### 复核结论（2026-10-27）

- ✅ IPC 与前端服务层已通：`cmd/vault/audit.rs:108` command → `lib.rs:120` 注册 → `vault.service.ts:120` → `vaultAuditSlice.ts:103`。
- ❌ `queryAuditLog` 全仓只出现在 service 与 slice，**没有任何组件调用**；i18n 也无审计日志词条（`zh-CN.json` 仅 `hibp_auto_audit` / `audit_all`）。属于"能调但看不到"的悬空接口。

---

## 🔴 问题 6：`backup()` 内联重复 — `change_master_password` 绕过 `backup()` 自行 VACUUM INTO

### 严重度：高

### 根因

`SecurityDbState::backup()` 已实现 `VACUUM INTO`，但 `change_master_password` 自己内联了相同的 VACUUM INTO 逻辑，而非调用 `self.backup()`。这是典型的重复实现。

### 文档依据

- [spec-1 §3.3 SecurityDbState 方法表](./I9-vault-spec-1-foundation.md)：`backup(db_path)` | 复制 security.db 到 backups/ 目录，保留最近 5 份；**触发时机**：vault 锁定时 / 主密码变更前 / 应用退出时
- [spec 偏差表 #5](./I9-vault-spec.md)：backup 实现 | `fs::copy` → `VACUUM INTO` | 更优：紧凑无碎片

文档明确要求 `backup()` 在"主密码变更前"触发，且 `change_master_password` 应调用 `backup()`。

### 代码位置

- `src-tauri/crates/assistant-security/src/security_db/mod.rs:239` — `backup()` 方法定义
- `src-tauri/crates/assistant-security/src/security_db/mod.rs:810` — `change_master_password` 内联 VACUUM INTO（重复）

### 修复方案

1. 重构 `change_master_password` 调用 `self.backup(&backup_path)?` 替代内联 VACUUM INTO
2. 补充 IPC command `vault_backup`（增强，文档未要求但符合 backup 触发时机设计）：
   ```rust
   #[command]
   pub fn vault_backup(app: AppHandle) -> Result<String, String> {
       let state = vault_state(&app)?;
       let backup_path = state.backup().map_err(|e| e.to_string())?;
       Ok(backup_path.to_string_lossy().to_string())
   }
   ```
3. `vault.service.ts` 添加 `backup` 方法

### 验收

- [x] `change_master_password` 不再内联 VACUUM INTO，与 `backup()` 共用 `backup_conn`
- [ ] `vault_backup` command 可手动触发备份 — **未完成，command 不存在**
- [x] 备份文件存在于 `backups/` 目录（主密码变更路径已写入 `.bak`）
- [ ] 备份保留最近 5 份的轮转清理 — **未实现**

### 复核结论（2026-10-27）

- ✅ 去重目标达成：`security_db/mod.rs:242` 的 `backup()` 与 `rekey.rs:51` 的 `change_master_password` 共用私有 `backup_conn()`。
- ❌ `backup()` 除测试（`tests/security_db.rs:122`）外无调用者；无 `vault_backup` command；无 `backups/` 保留 5 份的清理逻辑（spec-1 §3.3 要求）。

---

## 🟡 问题 7：`vault_show_unlock_window` — Rust command 完整但前端未接入

### 严重度：中

### 根因

command 已实现（创建 `vault-unlock` 窗口），但前端缺少 service 方法、路由页面、screen_lock 联动。

### 文档依据

- [spec-3 §7.7 独立解锁窗口](./I9-vault-spec-3-security-enhance.md)：完整规格（窗口标签/尺寸/JS代码量/生命周期/与主窗口通信）
- [spec-3 §7.8 验收标准](./I9-vault-spec-3-security-enhance.md)：独立解锁窗口与主窗口 V8 isolate 隔离
- [tasklist E4](./I9-vault-tasklist.md)：E4.1 command + E4.2 页面 + E4.3 事件通信

### 代码位置

- `src-tauri/src/cmd/vault.rs:467` — `vault_show_unlock_window` command 已实现
- `src/tools/password-vault/vault.service.ts` — **缺少** `showUnlockWindow`
- `src/` — **缺少** `/vault-unlock` 路由页面
- `screen_lock` 回调 — **缺少** 调用 `vault_show_unlock_window`

### 修复方案

1. `vault.service.ts` 添加：
   ```typescript
   showUnlockWindow: (): Promise<void> =>
     invoke("vault_show_unlock_window"),
   ```
2. 创建 `VaultUnlockWindow.tsx` 路由组件（极简，<50 行 JS，无第三方依赖）：
   - `<input type="password">` + 解锁按钮
   - 点击后 `invoke('vault_unlock', { masterPassword })` → 成功则 `window.close()`
   - 立即清空 DOM 中的密码值
3. 注册 `/vault-unlock` 路由
4. `screen_lock` 回调中：锁定后调用 `vault_show_unlock_window` 弹出解锁窗口
5. 主窗口监听 `vault:unlocked` 事件刷新 vaultStore

### 验收

- [x] 屏幕锁后弹出解锁入口 — 2026-10-27 已接：`setup.rs` 的 `on_lock` 在 `lock()` 成功后，若 `is_initialized()` 为真则 spawn `vault_show_unlock_window`
- [x] 解锁入口与主窗口 V8 isolate 隔离 — 已改用原生 Win32 对话框（无 JS 运行时），优于原 webview 方案
- [x] 解锁成功后自动关闭，主窗口刷新状态 — `lifecycle.rs:69` 成功后 emit `VAULT_UNLOCKED`，前端 `useVaultEvents.ts` 监听刷新
- [x] 窗口已存在时 focus 而非新建 — `state.is_locked()` 为 false 时直接返回 `Ok(true)`，不重复创建
- [x] 死窗口清理 — `src/layouts/VaultUnlockWindow.tsx` 已删除，`WINDOW_VAULT_UNLOCK` 常量与 `App.tsx` viewMap 映射同步移除

### 复核结论（2026-10-27）

**方案已变更**：[I9-vault-spec-3c-native-unlock-dialog.md](./I9-vault-spec-3c-native-unlock-dialog.md) 将"Tauri webview 解锁窗口"改为"原生 Win32 解锁对话框"，`vault_show_unlock_window` 已走 `unlock_dialog`。

- ✅ 更安全，安全目标（isolate 隔离）达成且更强。
- ✅ 屏幕锁联动已补齐（本节原方案第 4 步）。
- ✅ 遗留死代码已清理：`VaultUnlockWindow.tsx` / `WINDOW_VAULT_UNLOCK` / viewMap 映射全部删除，全仓零匹配（仅历史文档仍提及旧方案）。
- 现状唯一调用方：`settings/shared/renderers/BiometricSwitchRender.tsx:46`。

### 实施记录（2026-10-27）

1. `setup.rs` `on_lock`：`state.lock()` 成功后判断 `state.is_initialized()`，为真则 `tauri::async_runtime::spawn` 调用 `crate::cmd::vault_show_unlock_window(app_for_dialog)`，失败仅 `log::warn!`，不影响锁屏主流程
2. `setup.rs` `on_unlock`：移除 `emit_broadcast(VAULT_UNLOCKED)`，仅记录日志（详见新发现 12）
3. 删除 `src/layouts/VaultUnlockWindow.tsx`
4. `src/config/constants.ts` 移除 `WINDOW_VAULT_UNLOCK`
5. `src/App.tsx` 移除 `VaultUnlockWindow` import 与 viewMap 映射

---

## 🟢 问题 8：`LAST_REVEAL_EXPIRY` — 只写不读的 AtomicU64（服务端 expiresAt 的影子副本）

### 严重度：低

### 根因

文档设计了**两层过期机制**：

| 层 | 位置 | 机制 | 文档依据 |
|---|---|---|---|
| 服务端 | Rust `vault_reveal_password` | 生成 `expiresAt = now + 30s`，通过 `RevealResult` 返回给前端 | DESIGN §6.7.3: "返回 `{ value: VaultPlaintext, expires_at: now + 30s }`" |
| 客户端 | 前端 `vaultStore.ts` | `delay = max(expiresAt - Date.now(), 0)` → setTimeout 清零 | spec-2 §4.1: "超时清零基于服务端返回的 `expiresAt` 精确计算" |

`LAST_REVEAL_EXPIRY` 是服务端 `expiresAt` 的**影子副本**——每次 reveal 时 `.store(expires_at)`，但没有任何代码 `.load()` 读取它。它不是 `expiresAt` 的生成逻辑（生成逻辑在 `let expires_at = chrono::Utc::now().timestamp() + REVEAL_TTL_SECS as i64;`），只是一个从未被消费的跟踪变量。

### 文档依据

- 文档**大量提及** `expiresAt` 的生成和前端消费（DESIGN §6.7.3/6.7.4、spec-2 §4.1、tasklist C2.1/C2.2/C5.2/C8.5/G3.11）
- 但 `LAST_REVEAL_EXPIRY` 这个变量名在所有规格文档中**零匹配**——它是实现时自行添加的影子副本，文档没有设计依据
- 前端基于服务端 `expiresAt` 精确超时清零的机制**完全依赖 `RevealResult.expiresAt`**，与 `LAST_REVEAL_EXPIRY` 无关

### 代码位置

- `src-tauri/src/cmd/vault.rs:21` — `LAST_REVEAL_EXPIRY` 定义
- `src-tauri/src/cmd/vault.rs` — `vault_reveal_password` / `vault_reveal_notes` 中 `.store()` 写入
- **没有任何代码** `.load()` 读取

### 修复方案

**删除** `LAST_REVEAL_EXPIRY` 及所有 `.store()` 调用。`RevealResult.expiresAt` 的生成和返回不受影响——前端基于服务端 `expiresAt` 精确超时清零的机制（spec-2 §4.1、tasklist C8.5）完全不变。

### 验收

- [x] `LAST_REVEAL_EXPIRY` 及相关 `.store()` 全部移除
- [x] `RevealResult { value, expires_at }` 仍正常返回（`expires_at = now + 30s`）
- [x] 前端 `vaultStore.ts` 基于 `expiresAt` 精确超时清零行为不变
- [x] `cargo check -p assistant-app` 通过

### 实施记录（2026-10-22）

1. 从 `vault.rs` 移除 `use std::sync::atomic::{AtomicU64, Ordering}`（仅被 `LAST_REVEAL_EXPIRY` 使用）
2. 删除 `static LAST_REVEAL_EXPIRY: AtomicU64 = AtomicU64::new(0);`
3. 删除 `vault_reveal_password` 中 `LAST_REVEAL_EXPIRY.store(expires_at as u64, Ordering::SeqCst);`
4. 删除 `vault_reveal_notes` 中 `LAST_REVEAL_EXPIRY.store(expires_at as u64, Ordering::SeqCst);`
5. `RevealResult { value, expires_at }` 的生成逻辑（`let expires_at = chrono::Utc::now().timestamp() + REVEAL_TTL_SECS as i64;`）和返回逻辑完全保留不变

---

## 🟡 问题 9：ChaCha20-Poly1305 — 双算法支持但加密硬编码 Aes256Gcm

### 严重度：中

### 根因

`encrypt_field` / `decrypt_field` 都支持 ChaCha20Poly1305 分支，但所有 `encrypt_field` 调用都硬编码 `CipherAlgorithm::Aes256Gcm`，设置项 `vaultCipherAlgorithm` 已定义但加密时未读取。

### 文档依据

- [DESIGN §3.2](../VAULT-DESIGN.md)：`ChaCha20Poly1305` | 1 | 可选，无侧信道风险
- [spec-2 §5.6 设置项](./I9-vault-spec-2-crud-and-ui.md)：`vaultCipherAlgorithm` | `vault_cipher_algorithm` | `"aes256Gcm" | "chacha20Poly1305"` | `aes256Gcm`

文档明确要求双算法支持 + 用户可切换。

### 代码位置

- `src-tauri/crates/assistant-security/src/security_db/mod.rs` — 所有 `encrypt_field` 调用硬编码 `CipherAlgorithm::Aes256Gcm`
- `src-tauri/crates/assistant-security/src/vault_crypto/types.rs` — `CipherAlgorithm` 枚举已定义 ChaCha20Poly1305

### 修复方案

1. `SecurityDbState` 新增 `cipher_algorithm` 字段，从 `AppConf` 读取 `vault_cipher_algorithm`
2. 所有 `encrypt_field` 调用改为 `self.cipher_algorithm` 而非硬编码
3. `decrypt_field` 保持双分支（兼容旧数据）
4. 前端 `VaultSettings.tsx` 暴露算法选择下拉

### 验收

- [x] 设置 `vaultCipherAlgorithm = "chacha20Poly1305"` 后新条目用 ChaCha20 加密 — 2026-10-27 已修大小写 bug
- [x] 旧 Aes256Gcm 条目仍可正常解密（`decrypt_field` 按密文首字节选算法）
- [x] 密文首字节 `algo_byte` 与实际算法一致（`encrypt_field` 写入 `algo.discriminant()`）
- [x] 新增单测 `cipher_algorithm_from_conf_is_case_insensitive`（`tests/security_db.rs`），覆盖 `chacha20Poly1305` / `CHACHA20POLY1305` / `aes256Gcm` / 空串

### 复核结论（2026-10-27）

- ✅ 链路已接通：`cmd/vault/mod.rs:87` 的 `cipher_algo()` 读取 `AppConf.vault_cipher_algorithm`，`create/update/import/change_password` 均已传入；设置项与下拉已存在（`fieldDefinitions.ts:387`）。
- ✅ **大小写 bug 已修**：`SecurityDbState::cipher_algorithm_from_conf`（`security_db/mod.rs:266-271`）改为 `match conf_val.to_ascii_lowercase().as_str()`，前端 `"chacha20Poly1305"` 现可正确命中 `ChaCha20Poly1305`。选择全小写或全大写同样兼容。
- 🟡 旁支不一致（未修，见残留 P4）：`assistant-sync/src/vault_file/mod.rs:103` 的 `export_entry` 用 `Default::default()`（AES），`write_vault_meta` 硬编码 `"aes256Gcm"`，同步侧不跟随用户算法设置。

---

## 🟡 问题 10：`generatePassphrase` — 已实现但未集成到 VaultEntryForm

### 严重度：中

### 根因

`password-generator.service.ts` 中有完整的 Diceware 实现，但 vault 的条目创建表单没有"生成密码短语"按钮。

### 文档依据

- [spec-3 §6.5 Diceware 密码短语](./I9-vault-spec-3-security-enhance.md)：扩展现有 `password-generator.service.ts`：新增 `generatePassphrase(wordCount, separator, capitalize)`
- [tasklist D3.4](./I9-vault-tasklist.md)：VaultEntryForm：密码生成模式切换（随机字符 / 密码短语）
- [spec-3 §6.6 验收标准](./I9-vault-spec-3-security-enhance.md)：Diceware 生成 4 词密码短语

### 代码位置

- `src/tools/password-generator/password-generator.service.ts:2164` — `generatePassphrase` 已实现
- `src/tools/password-vault/VaultEntryForm.tsx` — **缺少** 密码短语模式

### 修复方案

1. `VaultEntryForm.tsx` 添加密码生成模式切换（随机字符 / 密码短语）
2. 密码短语模式：词数（默认 4）+ 分隔符（默认 `-`）+ 首字母大写开关
3. 调用 `generatePassphrase(wordCount, separator, capitalize)` 生成

### 验收

- [x] VaultEntryForm 可切换"随机字符"和"密码短语"两种生成模式
- [x] 密码短语生成 4 词（EFF 大词表 7776 词）
- [x] 生成的密码短语填入密码字段

### 复核结论（2026-10-27）

已落地：`forms/LoginFields.tsx:97-101` 提供随机/短语模式切换，`:147` 调用 `generatePassphrase(...)`；`forms/CardFields.tsx:112` 同样已接。

---

## 修复优先级建议

### 第一批：可直接删除（零风险）— ✅ 已完成

| # | 操作 | 风险 | 实施日期 |
|---|------|------|---------|
| 8 | 删除 `LAST_REVEAL_EXPIRY` | 零 — 前端已有过期管理 | 2026-10-22 |
| 1 | 删除 `assistant-storage::vault_meta` + `vault` | 低 — 需迁移 1 处 sync 调用方 | 2026-10-22 |

附带修复：`VaultCategory::from_str` → `VaultCategory::parse`（对齐 security crate 风格，消除 `FromStr` trait import 需求）

### 第二批：重构消除重复（低风险，高收益）

| # | 操作 | 风险 | 状态 |
|---|------|------|------|
| 6 | `change_master_password` 改调 `backup()` | 低 — 功能等价 | ✅ 已共用 `backup_conn` |
| 2 | `check_password` 改用 `SafePassword` | 低 — 接口变更需更新调用方 | ✅ 已完成 |

### 第三批：补全规格未完成项（中等风险）

| # | 操作 | 风险 | 状态 |
|---|------|------|------|
| 4 | 补全 `vault_totp_parse_uri` IPC 三层 | 低 — 纯新增 | ✅ 已完成 |
| 7 | 补全独立解锁窗口前端 | 中 — 需新建路由页面 | ✅ 已完成（原生对话框 + 屏幕锁联动 + 死窗口清理） |
| 10 | VaultEntryForm 集成 Diceware | 低 — 纯 UI 新增 | ✅ 已完成 |
| 9 | 修复 ChaCha20 算法选择 | 中 — 需测试双算法兼容 | ✅ 已完成（大小写归一化 + 单测；同步侧旁支见 P4） |

### 第四批：重构架构（较高风险，延后）

| # | 操作 | 风险 | 状态 |
|---|------|------|------|
| 3 | `export_entries` 改用 `VaultEntryDecrypted` | 中 — 需重构导出+同步 | 🟡 导出侧完成，同步侧未接 |
| 5 | 补全审计日志查询 IPC | 低 — 纯新增（增强） | 🟡 IPC 完成，UI 未做 |

---

## 残留汇总（2026-10-27 复核 + 第二批修复后）

### 已完成（无需再动）

| # | 项 | 验证点 |
|---|----|--------|
| 1 | `assistant-storage::vault_meta` + `vault` 删除 | 目录下已无 `vault.rs`/`vault_meta.rs`，`Cargo.toml` 已移除 `assistant-security` 依赖 |
| 2 | `SafePassword` + `sha1_hex` | `check_password(&SafePassword)`、`sha1_hex` 为 `pub(crate)`、`Drop` zeroize |
| 4 | `vault_totp_parse_uri` 三层 | command / `lib.rs` / service / slice / `VaultEntryDrawer` 全通 |
| 7 | 屏幕锁解锁联动 + 死窗口清理 | `setup.rs` `on_lock` 已 spawn `vault_show_unlock_window`；`VaultUnlockWindow.tsx` / `WINDOW_VAULT_UNLOCK` 全仓零匹配 |
| 8 | `LAST_REVEAL_EXPIRY` 删除 | 全仓零匹配 |
| 9 | ChaCha20 算法选择 | `to_ascii_lowercase()` 归一化 + 单测通过 |
| 10 | Diceware 集成 | `LoginFields` / `CardFields` 均有模式切换 |
| 12 | 屏幕解锁假解锁事件 | `setup.rs` `on_unlock` 不再 emit `VAULT_UNLOCKED`，仅记日志 |

### 残留项（按建议优先级）

| 优先级 | 项 | 具体残留 | 建议动作 |
|--------|----|---------|---------|
| P1 | 3 / 11 | `export_all_entries` 零调用、字段全 `None`，同步不写 `.enc`，条目未进 Git 仓库；连带灾难恢复导出必然 0 条 | 接 `decrypt_entry` + 在 `prepare` 流程中调用；归口 [tasklist-p2](./I9-vault-tasklist-p2.md) H5.2 / H7.1 |
| P3 | 6 | 无 `vault_backup` command；`backup()` 仅测试调用；无保留 5 份轮转 | 补 command + service + 轮转清理 |
| P3 | 5 | `queryAuditLog` 无 UI 消费方，i18n 无词条 | 设置页加审计日志面板（增强项，非规格要求） |
| P4 | 9 旁支 | 同步侧 `export_entry` 用 `Default::default()`、`write_vault_meta` 硬编码 `"aes256Gcm"` | 跟随用户算法设置 |
| P4 | 13 | `VaultFileManager` 5 个零外部调用方法 | 随 P1 同步链路接通后自然消解，或按需删除 |

### 核实记录（2026-10-27 逐项对照代码，全部确认存在）

| 项 | 证据 |
|----|------|
| P1 / 3·11 | `sync_engine/mod.rs:197` 定义，全仓无调用；`prepare()`（同文件 :36）只调 git prepare，`sync_now` = prepare→pull→finalize，均不导出条目；`export_entry` 唯一调用点 :211 位于 `export_all_entries` 内部；:218-227 密码/备注/TOTP/自定义字段全传 `None`。`tasklist-p2` H5.2 / H7.1 仍为 `[ ]` |
| P3 / 6 | `backup()`（`mod.rs:242`）仅 `tests/security_db.rs:122` 调用；`rekey.rs:51` 用的是 `backup_conn`；全仓无 `vault_backup` command；`rekey.rs:50` 只生成 `security-{timestamp}.db.bak`，无保留 5 份清理 |
| P3 / 5 | `queryAuditLog` 仅出现在 `vault.service.ts:120` 与 `vaultAuditSlice.ts:28,103`，无组件调用；`zh-CN.json` 仅 :373 `hibp_auto_audit`、:926 `audit_all` 两条无关词条 |
| P4 / 9 旁支 | `vault_file/mod.rs:51`、`:285` 硬编码 `"aes256Gcm"`；`:105` 用 `Default::default()` |
| P4 / 13 | `remove_entry_file`(:146)、`read_vault_meta`(:205)、`add_deleted_entry`(:378)、`remove_deleted_entry`(:387)、`SyncEngine::vault_file_manager()`(:193) 全仓仅定义处匹配 |

### 已修复（2026-10-27 第二批，原核实记录归档）

| 原项 | 原证据 | 修复后 |
|------|--------|--------|
| P0 / 9 | `security_db/mod.rs:266-271` 仅 match `"chacha20poly1305"`，无 `to_ascii_lowercase`；`fieldDefinitions.ts:395` 选项值为 `"chacha20Poly1305"` | 已改为 `conf_val.to_ascii_lowercase().as_str()`；`cipher_algorithm_from_conf_is_case_insensitive` 单测通过 |
| P2 / 7 | `setup.rs:127-153` 屏幕锁仅 `lock()` + emit `VAULT_LOCKED`；`VaultUnlockWindow.tsx` 仅被 `App.tsx:19` 引用，无任何创建者 | `on_lock` 已 spawn 原生解锁对话框；三处前端死代码已删除 |
| 新 12 | `setup.rs:155-163` 的 `on_unlock` emit `VAULT_UNLOCKED`，前端据此置 `isLocked: false`，Rust 侧仍锁定 | `on_unlock` 仅记日志，不再发事件 |

### 本轮新发现（原文档未覆盖）

| 新# | 内容 | 严重度 | 证据 |
|---|------|--------|------|
| 11 | P1 不只是死代码，而是**用户可见流程失效**：`vault_disaster_recovery_export`（`recovery.rs:105`）→ `disaster_recovery_export`（`sync_engine/mod.rs:279`）读取 `.enc` 文件，但 `.enc` 从未生成 → 灾难恢复导出必然返回 0 条；前端 `VaultUnlock.tsx:101` → `unlock/DisasterRecovery.tsx` 已完整接线 | 🔴 高（未修，归 P1） | 见上 P1 证据 + `export_entry` 无其它调用者 |
| 12 | 屏幕解锁误发 `VAULT_UNLOCKED`：`setup.rs:159` 在 `on_unlock` 中 emit `VAULT_UNLOCKED`，前端 `useVaultEvents.ts:10` 据此置 `isLocked: false`，而 Rust `SecurityDbState` 仍处于锁定 → UI 误判已解锁，后续命令报 "Vault locked" | ✅ 已修复（2026-10-27） | `setup.rs` `on_unlock` 现仅记录 `state.is_locked()` |
| 13 | `VaultFileManager` 另有 5 个无外部调用者：`remove_entry_file`(:146)、`read_vault_meta`(:205)、`add_deleted_entry`(:378)、`remove_deleted_entry`(:387)、`SyncEngine::vault_file_manager()`(:193) | 🟢 低（未修，归 P4） | 全仓仅定义处匹配 |

### 过时项（原文需更正，勿按旧路径操作）

| 位置 | 过时内容 | 现状 |
|------|---------|------|
| 问题 4/5/6/7/9/10 的「代码位置」 | `src-tauri/src/cmd/vault.rs` | 已拆分为 `src-tauri/src/cmd/vault/*.rs`（audit.rs / lifecycle.rs / entries.rs / recovery.rs / sync.rs / mod.rs） |
| 问题 4/5/7 的「代码位置」 | `src/tools/password-vault/vault.service.ts`、`VaultEntryForm.tsx` | 已迁移至 `src/vault/core/vault.service.ts`、`src/vault/forms/LoginFields.tsx` / `CardFields.tsx` |
| 问题 5 的「代码位置」 | "缺少 `vault_query_audit_log` command" | 已存在：`cmd/vault/audit.rs:108`，`lib.rs:120` 注册 |
| 问题 7 的「修复方案」 | Tauri webview 解锁窗口（`vault-unlock` 窗口 + `/vault-unlock` 路由） | 已被 [spec-3c 原生对话框](./I9-vault-spec-3c-native-unlock-dialog.md) 取代；方案作废，但"屏幕锁联动"目标仍有效 |
| 问题 10 的「代码位置」 | `src/tools/password-generator/password-generator.service.ts:2164` | 路径仍有效 |
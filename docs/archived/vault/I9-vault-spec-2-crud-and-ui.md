# I9-2 — 密码管理箱：数据交互与前端

> 本文档为 I9 密码管理箱规格的 **第 2 部分**，覆盖 P1-B（vault CRUD + 最小暴露 API）和 P1-C（前端 UI + 工具注册）。
> 前置文档：[I9-vault-spec-1-foundation.md](./I9-vault-spec-1-foundation.md)（P1-A 安全基础 crate + 跨阶段约束）
> 后续文档：[I9-vault-spec-3-security-enhance.md](./I9-vault-spec-3-security-enhance.md)（P1-D/E/F 安全增强）
> 设计文档：[VAULT-DESIGN.md](../../VAULT-DESIGN.md)
> 任务清单：[I9-vault-tasklist.md](./I9-vault-tasklist.md)

---

## 4. P1-B — vault CRUD + 最小暴露 API

### 4.1 交付目标

实现 vault 条目的增删改查，通过最小暴露 IPC API 确保密码不进入 JS 堆（除 `vault_reveal_password` 受速率限制）。

### 4.2 Tauri Commands

| Command | 签名 | 密码进 JS | 说明 |
|---------|------|:---------:|------|
| `vault_init` | `(masterPassword) → void` | ❌ | 初始化密码箱 |
| `vault_unlock` | `(masterPassword) → bool` | ❌ | 验证主密码并解锁 |
| `vault_is_locked` | `() → bool` | ❌ | 查询锁定状态 |
| `vault_lock` | `() → void` | ❌ | 锁定（zeroize + close conn） |
| `vault_list_entries` | `(category?, search?, tag?) → VaultEntrySummary[]` | ❌ | 列表（不解密） |
| `vault_get_entry` | `(id) → VaultEntry` | ❌ | 单条详情（不解密，返回加密字段 + 元数据） |
| `vault_copy_password` | `(id) → void` | ❌ | Rust 端解密 + 写剪贴板 |
| `vault_copy_field` | `(id, fieldName) → void` | ❌ | 复制 customField |
| `vault_reveal_password` | `(id) → { value, expiresAt }` | ✅ | 速率限制 3/分钟 + 审计 |
| `vault_reveal_notes` | `(id) → { value, expiresAt }` | ✅ | 同上 |
| `vault_create_entry` | `(entry) → id` | ❌ | 新增 |
| `vault_update_entry` | `(id, entry) → void` | ❌ | 更新 |
| `vault_delete_entry` | `(id) → void` | ❌ | 软删除 |
| `vault_change_master_password` | `(current, new) → void` | ❌ | 原子变更 + 全量重加密 |
| `vault_export` | `(format: "csv" \| "json") → Path` | ❌ | 导出全部条目（解密后写入文件，用完 zeroize） |
| `vault_import` | `(path, format) → count` | ❌ | 从 CSV/JSON 导入条目 |

### 4.3 RevealGuard 速率限制

RevealGuard 为 Tauri State 管理的运行时守卫，约束 `vault_reveal_password` / `vault_reveal_notes` 的调用频率：

**`vault_reveal_*` 防批量泄露**（与设计文档 §6.7.3 一致）：

| 层 | 机制 | 效果 |
|---|------|------|
| 速率限制 | 3 次/分钟，超限返回 `RateLimited` | 500 条目需 2.8 小时批量获取 |
| 审计日志 | 写入 `security_audit_log`（加密） | 事后可检测异常 |
| UI 通知 | 每次 reveal 显示 toast | 用户可感知异常 reveal |
| 单条限制 | 同一时间只有一个 reveal 有效 | 防并行批量请求 |

**RevealGuard 内部实现**（`assistant-security/src/reveal_guard.rs`）：

| 属性 | 值 |
|------|---|
| 速率上限 | 3 次/分钟 |
| 超限行为 | 返回 `VaultError::RateLimited`，前端显示"操作过于频繁，请稍后再试" |
| 审计 | 每次 reveal（无论成功或超限）写入 `security_audit_log`，event_type = `vault_reveal` / `vault_reveal_rate_limited` |
| 线程安全 | `last_reveal: Mutex<Instant>` + `reveal_count: AtomicU32`（`Ordering::SeqCst`，速率限制需跨线程一致） |

### 4.4 主密码变更原子流程

1. 验证当前主密码
2. WAL checkpoint(FULL)
3. 备份 security.db
4. 派生新 master_key + 新 sqlcipher_key
5. `PRAGMA rekey = 新 sqlcipher_key`（页面级原子重加密）
6. 逐条重加密 .enc 文件
7. 更新 vault_meta + .vault-meta
8. **zeroize 旧 master_key**（防止旧密钥残留内存）
9. 验证新主密码可解密
10. 失败回滚：从 backup 恢复

### 4.5 invoke_handler 注册

`src-tauri/src/lib.rs` 的 `invoke_handler` 宏新增所有 `cmd::vault_*` 条目。

### 4.6 参考项目

| 参考点 | 参考项目 | 关键文件 |
|--------|---------|---------|
| Cipher CRUD API（条目级加密 + 逐字段加解密） | Vaultwarden | `src/api/core/ciphers.rs`（cipher CRUD 路由：`post_ciphers` / `put_cipher` / `delete_cipher` / `post_ciphers_import`）、`src/db/models/cipher.rs`（数据模型：`Cipher` struct 含 `uuid` / `atype` / `name` / `notes` / `fields` / `data` / `deleted_at`，软删除用 `deleted_at: Option<NaiveDateTime>`） |
| 安全类型封装（Plaintext / Ciphertext） | prs | `lib/src/types.rs`（`Plaintext(SecVec<u8>)` / `Ciphertext(SecVec<u8>)` 安全封装，zeroize on drop） |
| 审计日志 | Vaultwarden | `src/db/models/event.rs`（`Event` struct 含 `event_type: i32` / `cipher_uuid` / `event_date`，`EventType` 枚举从 1000 起编） |
| 主密码变更原子性 | KeePassXC | `src/core/Database.cpp`（`changeKdf` + `changeKdfParallelism` + re-encrypt 流程） |

### 4.7 验收标准

- [ ] `vault_init` → `vault_unlock` → `vault_list_entries` → `vault_get_entry` → `vault_lock` 完整生命周期
- [ ] `vault_copy_password` 密码不进 JS 堆（Rust 端直接写剪贴板）
- [ ] `vault_reveal_password` 速率限制生效（第 4 次返回 RateLimited）
- [ ] `vault_change_master_password` 原子性（中途崩溃可从 backup 恢复）
- [ ] `vault_export` 导出后内存中明文已 zeroize

---

## 5. P1-C — 前端 UI + 工具注册

### 5.1 交付目标

实现密码箱完整前端 UI，注册为工具箱 `security` 分类工具。

### 5.2 工具注册

注册为工具箱 `security` 分类工具，遵循项目现有 `defineTool()` 模式（参考 `password-generator/index.tsx`、`fs-link/index.tsx`）：

| 属性 | 值 |
|------|---|
| id | `"password-vault"` |
| name | `"tools.password_vault.name"`（i18n key） |
| description | `"tools.password_vault.description"`（i18n key） |
| icon | `SafetyCertificateOutlined`（Ant Design Icons） |
| keywords | `["vault", "password", "manager", "safe", "credential", "locker", "keepass"]` |
| component | `lazyWithRecovery(() => import("./password-vault"))` |
| widthMode | `"wide"` |
| createdAt | **不设置**（项目 `defineTool` 中 `createdAt` 为可选字段，省略则 `isNew = false`，避免硬编码日期） |

### 5.3 前端文件结构

```
src/tools/password-vault/
├── index.tsx              ← defineTool 入口
├── VaultApp.tsx           ← 主界面路由（解锁/列表/编辑）
├── VaultUnlock.tsx        ← 主密码解锁
├── VaultList.tsx          ← 条目列表 + 搜索/分类/标签筛选
├── VaultEntryForm.tsx     ← 新增/编辑条目
├── VaultEntryDetail.tsx   ← 条目详情（复制/显示密码）
├── VaultSettings.tsx      ← 密码箱设置（自动锁定时间/剪贴板清零/加密算法）
├── vaultStore.ts          ← Zustand 状态
├── vault.service.ts       ← 前端服务层（invoke 封装）
└── password-vault.module.css
```

### 5.4 vaultStore 状态

与设计文档 §6.2 对齐。Zustand store，以下为状态字段 + actions 的完整清单：

**状态字段**：

| 字段 | 类型 | 说明 |
|------|------|------|
| `isLocked` | `boolean` | 密码箱是否锁定 |
| `isInitialized` | `boolean` | 是否已初始化（有主密码） |
| `loading` | `boolean` | 全局加载中（unlock/createEntry 等异步操作） |
| `error` | `string \| null` | 最近一次操作错误（显示后自动清零） |
| `entries` | `VaultEntrySummary[]` | 条目列表（不解密，仅元数据） |
| `searchQuery` | `string` | 搜索关键词 |
| `categoryFilter` | `VaultCategory \| null` | 分类筛选 |
| `tagFilter` | `string \| null` | 标签筛选 |
| `revealedField` | `{ value: string; expiresAt: number; kind: "password" \| "notes" } \| null` | 当前 reveal 的字段值 + 过期时间 + 类型标识（区分密码/笔记 reveal，防止互相覆盖）；超时清零基于服务端返回的 `expiresAt` 精确计算（`delay = max(expiresAt - Date.now(), 0)`），超时回调仅清零对应 `kind` 的状态 |
| `hibpResults` | `Map<string, number>` | 条目 ID → HIBP 泄露次数缓存 |
| `biometricAvailable` | `boolean` | 平台是否支持生物识别 |

**核心 actions**：

| Action | 签名 | 说明 |
|--------|------|------|
| `init` | `(masterPassword) → Promise<void>` | 初始化密码箱 |
| `unlock` | `(masterPassword) → Promise<boolean>` | 主密码解锁 |
| `unlockBiometric` | `() → Promise<boolean>` | 生物识别解锁（调用平台 `getKey()` → OS 弹提示 → 释放密钥 → 恢复 master_key） |
| `lock` | `() → Promise<void>` | 锁定 |
| `loadEntries` | `() → Promise<void>` | 刷新条目列表 |
| `createEntry` | `(entry) → Promise<string>` | 新增条目 |
| `updateEntry` | `(id, entry) → Promise<void>` | 更新条目 |
| `deleteEntry` | `(id) → Promise<void>` | 软删除条目 |
| `copyPassword` | `(id) → Promise<void>` | Rust 端复制（不进 JS 堆） |
| `copyField` | `(id, fieldName) → Promise<void>` | Rust 端复制自定义字段（不进 JS 堆） |
| `revealPassword` | `(id) → Promise<void>` | reveal 到 `revealedField`（kind="password"），受 RevealGuard 限制；超时清零基于服务端 `expiresAt` |
| `revealNotes` | `(id) → Promise<void>` | reveal 到 `revealedField`（kind="notes"），受 RevealGuard 限制；超时清零基于服务端 `expiresAt` |
| `resetAutoLockTimer` | `() → void` | 重置自动锁定计时器 |

### 5.5 UI 组件规格

| 组件 | 核心功能 |
|------|---------|
| VaultApp | 主界面路由（解锁/列表/编辑）+ 自动锁定计时器（mousemove / keydown / **visibilitychange** 监听）；标签页切换回前台时重置自动锁定计时器（`document.visibilityState === "visible"` → `resetAutoLockTimer()`） |
| VaultUnlock | 主密码输入 + 生物识别按钮 + 创建密码箱入口 + 恢复密钥入口 |
| VaultList | 搜索框 + 分类筛选 + 标签筛选 + 新增按钮 + 条目卡片列表 + 泄露标记 |
| VaultEntryForm | 分类选择 + 标题/用户名/密码/TOTP/标签/备注 + 密码生成器集成 + 自定义字段 |
| VaultEntryDetail | 元信息展示 + 复制密码/用户名/URL + TOTP 倒计时环 + reveal 密码/笔记独立状态（`revealedField?.kind === "password"` / `"notes"`）+ 基于 `expiresAt` 精确超时清零 |

### 5.6 设置项扩展

AppConf 新增字段，**必须同步更新前端 interface + Rust struct + `#[serde(default)]`**（项目规则要求）：

| 前端字段 (AppConf) | Rust 字段 (app_conf.rs) | 类型 | 默认值 |
|---------------------|------------------------|------|--------|
| `vaultAutoLockMinutes` | `vault_auto_lock_minutes` | `u16` / `number` | 5 |
| `vaultLockOnScreenLock` | `vault_lock_on_screen_lock` | `bool` / `boolean` | true |
| `vaultClipboardClearSeconds` | `vault_clipboard_clear_seconds` | `u16` / `number` | 30 |
| `vaultCipherAlgorithm` | `vault_cipher_algorithm` | `CipherAlgorithm` / `"aes256Gcm" \| "chacha20Poly1305"` | `aes256Gcm` |
| `vaultBiometricEnabled` | `vault_biometric_enabled` | `bool` / `boolean` | false |
| `vaultHibpAutoAudit` | `vault_hibp_auto_audit` | `bool` / `boolean` | true |

Rust struct 示例（`assistant-config/src/app_conf.rs`）：
- 每个字段添加 `#[serde(default)]`，确保旧配置文件缺失新字段时不报错
- `CipherAlgorithm` 实现 `Serialize`/`Deserialize`，序列化为 `"aes256Gcm"` / `"chacha20Poly1305"`
- ⚠️ 项目现有 `AppConfDto` 使用 `#[serde(rename_all = "camelCase", deny_unknown_fields)]`（见 `src-tauri/src/protocol/dto/config.rs`），新增字段必须同时更新 `AppConfDto` struct + `From<AppConf>` impl

### 5.7 i18n

`zh-CN.json` / `en-US.json` 的 `tools.password_vault` 命名空间下新增约 50 个词条。

### 5.8 参考项目

| 参考点 | 参考项目 | 关键文件 |
|--------|---------|---------|
| 条目列表 UI + 分类/搜索/标签筛选 | KeePassXC | `src/gui/entry/EntryModel.h`（`ModelColumn` 枚举：Title/Username/Password/Url/Notes/Totp/PasswordStrength 等 17 列）、`src/gui/entry/EntryView.cpp`（列表视图） |
| 密码生成器集成 | KeePassXC | `src/gui/entry/EntryWidget.cpp`（编辑表单 + 生成器弹窗） |
| defineTool 工具注册模式 | 本项目 | `src/tools/registry.ts`（`defineTool()` 函数 + `lazyWithRecovery` + `classifyKeywords` 自动分类）、`src/tools/password-generator/index.tsx`（现有 security 工具示例） |
| Cipher 数据模型（前端 TypeScript 类型） | Vaultwarden | `src/db/models/cipher.rs`（`Cipher` struct：`uuid` / `atype`(1=Login,2=SecureNote,3=Card,4=Identity,5=SSHKey) / `name` / `notes` / `fields` / `data` / `deleted_at`，软删除模式） |

### 5.9 验收标准

- [ ] 工具箱 security 分类显示密码箱工具
- [ ] 完整 CRUD 流程：创建 → 列表 → 编辑 → 删除
- [ ] 复制密码走 Rust 端（不进 JS 堆）
- [ ] reveal 密码/笔记基于服务端 `expiresAt` 精确超时清零（非固定 30s）
- [ ] reveal 密码和笔记状态独立（`kind` 区分，不互相覆盖）
- [ ] 自动锁定计时器生效（mousemove / keydown / visibilitychange 三事件监听）
- [ ] 标签页切换回前台时重置自动锁定计时器
- [ ] 设置项同步：前端 `AppConf` interface + Rust `AppConf` struct + `AppConfDto` struct + `From<AppConf>` impl 四处一致
- [ ] `vault_change_master_password` 中途崩溃可从 backup 恢复
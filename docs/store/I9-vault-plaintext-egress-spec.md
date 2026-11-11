# I9 密码箱 — 明文出口与辅助密钥规格（S5）

> 状态：规格（2026-11-11）
> 上游：[I9-vault-key-hierarchy-design.md](./I9-vault-key-hierarchy-design.md)、[I9-vault-key-hierarchy-spec.md](./I9-vault-key-hierarchy-spec.md)
> 同层：[I9-vault-runtime-session-spec.md](./I9-vault-runtime-session-spec.md)（内存驻留）、[I9-vault-sync-crypto-spec.md](./I9-vault-sync-crypto-spec.md)（仓库）
> 本规格只定义**明文以什么形态、经什么通道、离开应用边界时必须有怎样的约束**。
> 路径约定：全部为仓库相对路径。

---

## 0. 出口分级

所有明文出口必须归入以下三档之一。新增出口先定档，再实现。

| 档 | 定义 | 允许条件 |
|---|---|---|
| **E1 允许** | 用户显式请求、单条目、有 TTL/审计 | 限速 + 审计 + TTL |
| **E2 有条件** | 批量/文件/网络，泄露面大 | 二次确认 + 审计 + 权限/路径约束 + 清理 |
| **E3 禁止** | 无用户请求、静默、不可控 | 不得实现 |

### 0.1 现有出口定档

| 出口 | 通道 | 档 | 现状 | 缺口 |
|---|---|---|---|---|
| 复制密码 | 剪贴板 | E1 | 定时清除、审计 | 内存残留（F8） |
| 复制自定义字段 | 剪贴板 | E1 | 同上 | 同上 |
| 复制 TOTP 码 | 剪贴板 | E1 | 同上 | 同上 |
| 揭示密码/备注 | IPC → 前端 | E1 | 30s TTL、限速、审计 | 前端驻留（F9） |
| 历史/冲突预览 | IPC → 前端 | E1/E2 | 部分脱敏 | **notes 未脱敏**（F7） |
| 冲突字段揭示 | IPC → 前端 | E1 | **无限速、无审计** | F7 |
| 导出 JSON/CSV | 文件 | E2 | 无确认、无审计、无权限 | F5 |
| 灾难恢复导出 | 文件 + git clone | E2 | 有路径参数、无清理 | F12 |
| 恢复码展示 | 屏幕/剪贴板 | E2 | 有警告 | 无审计（见 key-hierarchy-spec） |
| HIBP 检查 | 网络 | E2 | k-anonymity | 无出口审计约定 |
| SSH 私钥注入 | 本机 agent | E2 | 有审计（手动） | 自动路径无审计（F14） |
| provider API key | 本机库 + 网络 | E2 | machine-id 包裹 | **不是保密**（F11） |
| 审计日志查询 | IPC → 前端 | E1 | 无明文 | 无 |
| 日志/错误 | 文件/控制台 | E3 | 未见明文 | 需扫描约束 |

---

## 1. 剪贴板与揭示

### 1.1 剪贴板

目标：

```rust
pub fn copy_and_guard(&self, secret: &str, ttl_secs: u64) -> Result<(), VaultError>;
```

规则：

1. 明文在守卫内以 `Zeroizing<String>` 持有；到期或 `force_clear` 时清零。
2. **不长期保存原剪贴板内容**：原内容用于「恢复用户剪贴板」，最多保存到 TTL；TTL 到期后连同明文一起清空。
3. 若原剪贴板内容本身可能是敏感信息（用户先前复制过密码），恢复动作等于把它写回剪贴板；这是用户预期行为，但必须在文档说明。
4. `ttl_secs` 来自 `VaultConf::vault_clipboard_clear_seconds`；`0` 表示不自动清除（需二次确认）。
5. 锁定、退出、崩溃兜底（下次启动）都必须清空剪贴板。
6. 每次复制写审计：`VaultCopyPassword` / `VaultCopyField`（detail 为条目 ID，不含值）。

### 1.2 揭示

规则：

1. `vault_reveal_password` / `vault_reveal_notes` 必须经过 `RevealGuard`（3 次/60s）。
2. TTL 由 `VaultConf` 提供，默认 30s；`RevealedField.expiresAt` 必须由后端返回，前端不得自定。
3. 前端在以下任一时刻立即清空 `revealedField`：
   - TTL 到期；
   - 用户点击隐藏；
   - 切换条目；
   - 锁定 / 收到 `VAULT_LOCKED`；
   - 组件卸载。
4. 揭示动作写审计 `VaultReveal`（detail 为条目 ID）。
5. 禁止把揭示值写入 `console`、错误上报、URL、`localStorage` / `sessionStorage`。

### 1.3 冲突/历史预览

当前 `mask_entry_preview` 只脱敏 `password`、`totp_secret`、`is_secret` 自定义字段，**`notes` 明文返回**（`sync_engine/preview.rs:9-47`）。

目标：

| 字段 | 预览中 | 说明 |
|---|---|---|
| `title` / `username` / `url` / `tags` | 显示 | 列表也需要，且属于「整库保护」范围 |
| `password` | 固定 `•••` | 需揭示 |
| `notes` | **固定 `•••`** | 需揭示 |
| `totp_secret` | 固定 `•••` | 需揭示 |
| 受保护自定义字段 | 固定 `•••` | 需揭示 |
| 未受保护自定义字段 | 显示 | 与条目语义一致 |

`vault_conflict_reveal` 必须：

1. 经 `RevealGuard`；
2. 写审计 `VaultReveal`，detail = `conflict:{entryId}:{side}:{field}`；
3. 返回 TTL；
4. 仅允许 `password` / `notes` / `totpSecret` / 自定义字段名，白名单之外返回 `Validation`。

---

## 2. 批量导出（E2）

### 2.1 `vault_export_to_file`

当前：`entries.rs:302-352` 直接把 `VaultEntryExport` 序列化为 JSON/CSV 写入用户路径。

目标流程：

```
1. 前端二次确认：「导出为明文，包含密码、TOTP、SSH 私钥；请妥善保管」
2. 命令接收 confirm_token（固定字面量 "EXPORT-PLAINTEXT"）
3. 校验 path：必须是用户通过保存对话框选择的绝对路径；禁止目录、禁止已存在文件覆盖（或二次确认）
4. 写文件：权限 0600（平台允许时）；UTF-8
5. 审计 VaultEntriesExported { format, count, pathHash }   // 只记 path 的 SHA-256，不记明文路径
6. 返回 { path, count }
```

约束：

1. **禁止**默认导出路径；未提供路径 → `Validation`。
2. **禁止**导出到仓库目录、`security/` 目录、`security/recovery/`。
3. **禁止**在导出文件中包含内部字段（`id`、`revision`、`dekVersion`）；当前 `VaultEntryExport` 不含这些，保持。
4. JSON 使用 `to_string_pretty`；CSV 的 tags 用逗号分隔（现状保持）。
5. 导出成功/失败都写审计；失败不留下半截文件（先写临时文件再 rename）。

### 2.2 `vault_import`

1. 读取路径必须来自打开对话框；禁止目录。
2. 导入前展示「将导入 N 条，明文文件将被读取」。
3. 导入后必须写 `entries/*.enc` 并 `finalize`（sync-crypto-spec §2.3 / entry-cipher-spec §5.2）。
4. 审计 `VaultEntriesImported { format, count, pathHash }`。
5. 导入失败不得留下 DB 与仓库不一致的状态。

### 2.3 灾难恢复导出

当前：`vault_disaster_recovery_export` 可传 `git_url` + `commit`，clone 到 `security/recovery/`，导出全条目 JSON。

目标：

1. `export_path` 同 §2.1 的路径与权限要求。
2. 导出结束（成功/失败）删除 `security/recovery/`。
3. 审计 `RecoveryExportUsed { entryCount, at }` 写 JSONL（库不可用）。
4. 若 `integrity_warning=true`，导出文件仍生成，但 UI 必须强提示。
5. 禁止把恢复码写入导出文件、日志、审计。

---

## 3. 恢复码展示（E2）

> 纸面字节格式、编解码与 `RecoverySession` 见 [I9-vault-recovery-channel-spec.md](./I9-vault-recovery-channel-spec.md)；本节只定「展示/传递通道」。

1. 恢复码只在生成时展示一次；再次查看等同重新生成（recovery-channel-spec §1）。
2. 展示时必须：
   - 5 组分组显示；
   - 强警告「这张纸就是全部数据」；
   - 勾选「我已抄写」后才能关闭；
   - 提供打印，不默认复制到剪贴板。
3. 写审计 `RecoveryKeyExported`。
4. 禁止把恢复码写入剪贴板历史（平台允许时排除）、截图分享提示、日志。

---

## 4. 辅助密钥

### 4.1 provider API key（`machine_uid`）

当前：`assistant-crypto/src/api_key.rs` 用 `SHA-256(machine_uid)` 作为 AES-256-GCM 密钥。

事实认定：

- `machine_uid` 可由同机任意进程读取，因此该密钥**不是秘密**；
- 该方案是**混淆/防误读**，不是「只有本机用户能解」的保密；
- 任何同机进程只要读到 provider 库文件，就能解密 API key。

规格要求：

1. 在 `api_key.rs` 顶部注释明确写出上述事实，**禁止**在 UI/文档中宣称「API key 已加密保护」。
2. 升级路径（不在本期实现，但必须登记）：改用 OS keystore（DPAPI / Keychain / libsecret）或用户口令派生密钥。
3. 日志、错误、审计不得输出 API key 明文或密文。
4. 与保险箱密钥体系**完全隔离**：不得复用 `dek` / `master_key` / 生物识别 wrapping key。

### 4.2 SSH 私钥

1. 私钥只在「注入 agent」或「解析元数据」时解密，用完立即 Drop。
2. `vault_ssh_auto_add_all` 与 `auto_add_entry` / `auto_remove_fingerprint` 必须写审计（detail 为指纹，不含私钥）。
3. 锁定/退出必须移除本应用注入的 key（runtime-session-spec §3.1）。
4. `vault_ssh_parse_key` 接收前端传入的私钥明文；必须：
   - 不写日志；
   - 不回显；
   - 解析后立即丢弃。
5. agent 的 `lifetime` 参数必须生效；未指定时由 `VaultConf` 决定，不得无限期。

### 4.3 TOTP

1. TOTP secret 只在生成码时解密；码本身是短周期明文，允许展示/复制。
2. 复制 TOTP 码写审计 `VaultCopyField`（detail `totp:{id}`）。
3. 禁止在日志中输出 secret 或完整 `otpauth://` URI。

### 4.4 HIBP

1. 只允许 k-anonymity 前缀请求；**禁止**发送完整密码或完整 SHA-1。
2. 请求失败不得把密码写入错误消息。
3. `vault_hibp_audit` 批量检查时必须限速（现状 1.5s/次、并发 2）。
4. 审计只记条目 ID 与命中次数，不记密码。

---

## 5. 日志与错误

### 5.1 禁止出现在日志/错误/审计中的内容

- 口令、`dek`、`dbKey`、`KEK`、`metaKey`、恢复码；
- 条目明文字段（password / notes / totp secret / 受保护自定义字段 / SSH 私钥）；
- provider API key；
- 未脱敏的远端 URL（含 `user:token@`）；
- 剪贴板内容。

### 5.2 允许出现的内容

- 条目 ID、事件类型、条目数量、格式、状态码；
- 路径的 SHA-256（导出/导入审计）；
- 脱敏后的远端 `scheme://host/path`。

### 5.3 错误消息

1. `VaultError::Crypto(String)` 的消息不得包含明文或密钥；只允许算法名与失败阶段。
2. `EntryCorrupted` 只带 `id`。
3. `VaultErrorDto.message` 面向用户，必须是可读原因，不得是底层错误字符串拼接。

---

## 6. 文件权限与路径

1. 所有导出的明文文件：创建时 `0600`（Unix）；Windows 使用当前用户 ACL。
2. 禁止写入系统临时目录（`%TEMP%`、`/tmp`）作为中间落点；导出用同目录 `.tmp` + rename。
3. `resolve_external_input` 必须同时限制：
   - 不进入应用数据目录（`security/`）；
   - 不进入仓库目录；
   - 不覆盖已存在文件，除非二次确认。
4. 灾难恢复 clone 目录固定 `security/recovery/`，使用后删除，禁止保留。

---

## 7. 文件改动清单

| 文件 | 改动 |
|---|---|
| `src-tauri/src/cmd/vault/entries.rs` | 导出/导入：confirm_token、路径校验、0600、审计、临时文件 |
| `src-tauri/src/cmd/vault/recovery.rs` | 灾难恢复：路径/权限/审计/清理 `security/recovery/` |
| `src-tauri/src/cmd/vault/reveal.rs` | TTL 来自配置；揭示审计；禁止明文日志 |
| `src-tauri/src/cmd/vault/audit.rs` | HIBP 限速与审计；TOTP 不记 secret |
| `src-tauri/src/cmd/vault/ssh.rs` | 自动路径补审计；lifetime 生效；私钥不落日志 |
| `src-tauri/crates/assistant-vault/src/sync_engine/preview.rs` | notes 脱敏 |
| `src-tauri/crates/assistant-vault/src/sync_engine/history.rs` | 冲突揭示限速/审计/TTL |
| `src-tauri/crates/assistant-vault/src/audit/mod.rs` | 新增 `VaultEntriesExported` / `VaultEntriesImported` / `RecoveryExportUsed` |
| `src-tauri/crates/assistant-crypto/src/api_key.rs` | 注释澄清「非保密」；登记升级路径 |
| `src-tauri/crates/assistant-security/src/clipboard_guard.rs` | `Zeroizing`、TTL 后清空原文 |
| `src/vault/settings/ImportExportTab.tsx` | 二次确认、明文警告、无默认路径 |
| `src/vault/settings/RecoveryTab.tsx` | 恢复码强警告、勾选抄写、默认不复制 |
| `src/vault/detail/RevealFieldButton.tsx` | TTL 展示、锁定即清空 |
| `src/i18n/locales/*/vault.json` | 全部明文警告文案 |

---

## 8. 测试

| 编号 | 用例 | 断言 |
|---|---|---|
| T-P1 | 导出未传 confirm_token | `Validation`，不写文件 |
| T-P2 | 导出成功后检查文件权限 | Unix 0600 / Windows 当前用户 ACL |
| T-P3 | 导出路径指向 `security/` 或仓库 | `Validation` |
| T-P4 | 导出中途失败 | 无半截目标文件；`.tmp` 被清理 |
| T-P5 | 导入后立即检查 `.enc` 与 git | 每条都有文件且已提交 |
| T-P6 | 冲突预览含 notes | notes 为 `•••` |
| T-P7 | 冲突揭示 4 次 | 第 4 次 `RateLimited`；每次成功有审计 |
| T-P8 | 揭示后锁定 | 前端 `revealedField=null` |
| T-P9 | 恢复码展示 | 审计 `RecoveryKeyExported`；剪贴板未被自动写入 |
| T-P10 | 灾难恢复成功 | `security/recovery/` 不存在；JSONL 有 `RecoveryExportUsed` |
| T-P11 | SSH 自动注入 | 审计存在；detail 为指纹 |
| T-P12 | 日志扫描 | 无口令/密钥/明文字段/未脱敏 URL |
| T-P13 | API key 文件被同机进程读取 | 能解密；文档与 UI 不得声称保密 |

测试落点：新增 `src-tauri/crates/assistant-vault/tests/egress_guard.rs`（confirm_token、路径、权限、审计）；前端二次确认放入 `src/vault/settings/__tests__/ImportExportTab.test.tsx`（新增）。

---

## 9. 禁止事项

1. **禁止**任何明文出口不经用户显式请求。
2. **禁止**批量导出无二次确认、无审计。
3. **禁止**把 `notes` 明文放进冲突/历史预览。
4. **禁止**在日志、错误、审计 detail 中出现明文/密钥/恢复码/API key。
5. **禁止**把恢复码自动写入剪贴板或文件。
6. **禁止**把明文导出到仓库、`security/`、系统临时目录。
7. **禁止**宣称 `machine_uid` 方案为「保密加密」。
8. **禁止**在解锁状态下无限期保留剪贴板原文与前端揭示值。

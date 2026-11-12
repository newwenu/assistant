# I9 密码箱 — 能力、配置与版本门禁规格（S6）

> 状态：规格（2026-11-11）
> 上游：[I9-vault-key-hierarchy-design.md](./I9-vault-key-hierarchy-design.md)、[I9-vault-key-hierarchy-spec.md](./I9-vault-key-hierarchy-spec.md)
> 同层：[I9-vault-runtime-session-spec.md](./I9-vault-runtime-session-spec.md)（会话）、[I9-vault-sync-crypto-spec.md](./I9-vault-sync-crypto-spec.md)（仓库）
> 本规格定义**什么情况下保险箱能力存在、用什么算法、认哪个版本、平台缺能力时怎么退化**。
> 路径约定：全部为仓库相对路径。

---

## 0. 范围

| 门禁 | 位置 | 说明 |
|---|---|---|
| 编译期能力 | `src-tauri/Cargo.toml`、`src-tauri/src/vault_plugin.rs` | `feature = "vault"` |
| 命令注册 | `vault_plugin.rs` 的 `generate_handler!` | 命令缺失/多出 |
| 前端能力 | `src/features/vault/module.ts`、`featureIpc.ts` | 前端模块开关 |
| 运行时配置 | `assistant-config/src/vault_conf.rs` | 自动锁、剪贴板、算法、同步、SSH |
| 库结构版本 | `security_db/schema.rs`、`security_db/meta.rs` | `SCHEMA_VERSION` |
| 仓库格式版本 | `.vault-meta.version`、`EntryFile.version` | v1 / v2 判定 |
| 平台能力 | `assistant-vault/src/biometric/*`、`assistant-security/*` | Hello/TouchID/libsecret、锁屏、剪贴板、原生窗口 |
| 启动顺序 | `vault_plugin.rs`、`cmd/vault/mod.rs` | keyring 预热、状态装配、锁屏监听 |

---

## 1. 编译期能力开关

### 1.1 规则

1. `feature = "vault"` 关闭时：
   - 不注册任何 `vault_*` 命令；
   - 不创建 `SecurityDbState` / `VaultRepo` / `SyncEngine` / `RevealGuard` / `ClipboardGuard` / `SshAgentState`；
   - 不监听屏幕锁；
   - 不读取 `vault-config.json`；
   - 前端不挂载 vault 路由/模块。
2. `feature = "vault"` 打开时：
   - `vault_plugin::mounted::init()` 注册全部命令；
   - `on_setup` 加载配置；
   - `on_exit` 执行 `lock_session(AppExit)` + 剪贴板兜底。

### 1.2 命令清单一致性

`vault_plugin.rs` 的 `generate_handler!` 是**唯一注册点**。要求：

1. 新增/删除命令必须同时改：
   - `src-tauri/src/cmd/vault/*.rs`；
   - `vault_plugin.rs`；
   - 前端 `vault.service.ts`；
   - `src/vault/core/contract/vault.contract.ts`（如涉及 DTO）。
2. 增加一条测试：从 `vault_plugin.rs` 提取 `vault_*` 命令名，与 `vault.service.ts` 中 `invoke("vault_*")` 的集合做双向包含比对。
3. 禁止在 `feature = "vault"` 为 false 时保留任何 `vault_*` 命令的占位实现。

### 1.3 前端能力

1. `src/features/vault/module.ts` 必须声明能力依赖；`featureIpc` 在能力关闭时直接抛 `FeatureUnavailable`，不得静默返回空。
2. 能力关闭时，任何 vault 入口（路由、菜单、快捷键）都不得渲染。
3. 禁止用 `window.__TAURI__` 直接探测命令是否存在。

---

## 2. 配置契约

### 2.1 字段表

`assistant-config/src/vault_conf.rs`：

| 字段 | 类型 | 默认 | 语义 | 安全影响 |
|---|---|---|---|---|
| `vault_auto_lock_minutes` | `u16` | `5` | 自动锁定分钟；`0` = 不自动锁定 | 越大明文驻留越久 |
| `vault_lock_on_screen_lock` | `bool` | `true` | 系统锁屏时锁定 | 关闭会扩大暴露窗口 |
| `vault_clipboard_clear_seconds` | `u16` | `30` | 剪贴板清除秒数；`0` = 不自动清除 | 越大泄露窗口越大 |
| `vault_cipher_algorithm` | 枚举 | `Aes256Gcm` | 新写入使用的算法 | 只影响新密文 |
| `vault_hibp_auto_audit` | `bool` | `true` | 解锁后是否自动跑 HIBP | 影响网络出口 |
| `vault_sync_remote_url` | `Option<String>` | `None` | 远端 URL（可能含凭据） | 需脱敏 |
| `vault_ssh_auto_add` | `bool` | `false` | 解锁后自动注入 SSH key | 扩大 agent 驻留 |
| `vault_ssh_use_pageant` | `bool` | `true` | 使用 Pageant；关闭时用 OpenSSH agent | 平台相关 |
| `vault_ssh_use_openssh_agent` | `bool` | `true` | 保留字段；**当前无消费点**，桥接仅由 `vault_ssh_use_pageant` 单向决定 | 平台相关 |
| `vault_ssh_auth_sock_override` | `String` | 空 | 覆盖 `SSH_AUTH_SOCK` | 可指向不可信 agent |
| `vault_sync_last_at` | `Option<i64>` | `None` | 上次同步时间（毫秒） | 无 |
| `vault_sync_ssh_hosts` | `Vec<String>` | 空 | 持久化 SSH host key 的主机 | 影响信任范围 |

### 2.2 校验

1. 反序列化即校验（封闭枚举拒绝非法值）。
2. 额外校验：
   - `vault_auto_lock_minutes <= 24 * 60`；
   - `vault_clipboard_clear_seconds <= 600`；
   - `vault_ssh_auth_sock_override` 非空时必须是绝对路径；
   - `vault_sync_remote_url` 必须是 `http(s)://`、`ssh://` 或 `git@host:path` 形态。
3. 校验失败 → `vault_get_conf` 返回 `Validation`，不得静默回退默认值（除字段缺失）。
4. 配置迁移：`migrate_from_app_conf` 只做一次性搬迁；未知字段忽略；不得把非法值当默认值。

### 2.3 算法切换语义

`vault_cipher_algorithm` **只影响新写入**：

| 场景 | 行为 |
|---|---|
| 已有条目 | 保持原算法；解密时按密文首字节（v1）或 `EntryFile.cipher`（v2）识别 |
| 新建/更新条目 | 使用当前配置算法 |
| 改密（v1.5） | 不重加密条目；算法不变 |
| `rotate_dek` | 全量重加密；使用当前配置算法 |
| 同步 | 每文件自带算法；允许仓库内混合 |

要求：

1. 禁止「切换配置后全量重加密」的隐式行为。
2. 禁止在解密时信任配置算法；必须按密文自身标识。
3. UI 必须说明「切换仅对新条目/轮换生效」。

---

## 3. 版本门禁

### 3.1 三个独立版本号（不得混用）

| 版本 | 位置 | 取值 | 用途 |
|---|---|---|---|
| `SCHEMA_VERSION` | `security_db/schema.rs` | 当前 `2` | 本机库 DDL |
| `.vault-meta.version` | 仓库 | `1`（旧）/ `2`（v1.5） | 密钥体系代际 |
| `i9:v1:` 前缀 | HKDF info / AAD | 字面量 | 密钥派生域标签，**不是**文件版本 |

规则：

1. `.vault-meta.version = 1` 或缺失且存在 `crypto.masterKeySalt` / `emergency` → `UnsupportedVaultVersion`。
2. `.vault-meta.version = 2` 才进入 v1.5 解析。
3. `EntryFile.version` 独立于 `.vault-meta.version`；两者不一致时以 manifest 校验为准。
4. **禁止**把 `i9:v1:` 改成 `i9:v2:` 来「配合」v2 文件版本。
5. `SCHEMA_VERSION` 变更必须提供迁移或明确拒绝；不得静默继续。

### 3.2 旧库处置

与 key-hierarchy-spec §5 一致：

```
1. 检测到 v1 → UnsupportedVaultVersion
2. UI：提示「此库由旧版本创建，本版本不支持升级」+「重置仓库」入口
   同屏提示先备份仓库目录（重置不可逆，旧库不会被自动保留）
3. 重置：二次确认 + confirm_token("RESET-VAULT")
4. 重置流程：锁库 → 清 keystore → 备份 `<security>/vault-meta.bak`（**仓库之外**，每次重置覆盖）→ 删 .vault-meta / security.db / entries/ / audit /
5. 审计 VaultRepositoryReset（JSONL）
6. 重置成功后回到初始化页（未初始化态），由用户主动执行初始化；不自动创建新库
```

禁止：迁移器、staging 目录、`journal.json`、`.vault-legacy/` 归档、字段映射。

补充约束：

- **检测阶段只读**：报 `UnsupportedVaultVersion` 时不得创建、改名或删除仓库内任何文件（与 key-hierarchy-spec §11.2 I10 一致）。
- **备份提示不等于备份**：UI 只提示用户自行备份，应用不得代为复制或归档旧库（§9.4）。
- **重置后不自动重建**：第 6 步只切到初始化页；是否新建库由用户点击初始化决定，避免「确认重置即静默建库」。

### 3.3 仓库格式版本

| 文件 | 版本 | 不匹配处理 |
|---|---|---|
| `.vault-meta` | 2 | v1 → `UnsupportedVaultVersion`；其他 → `VaultCorrupted` |
| `entries/*.enc` | 2 | v1 → `UnsupportedEntryFileVersion`；其他 → `VaultCorrupted` |
| `EntryBlobV2.schema` | 1 | 未知 → `EntryCorrupted` |

---

## 4. 平台能力矩阵

> 生物识别的 payload、key id、ACL 判定与删除语义见 [I9-vault-biometric-spec.md](./I9-vault-biometric-spec.md)；本表只定「能力是否存在、缺失时如何退化」。

| 能力 | Windows | macOS | Linux | 缺失时行为 |
|---|---|---|---|---|
| 生物识别 | Windows Hello（`KeyCredentialManager`） | Touch ID（`kSecAccessControlUserPresence`） | libsecret（需绑定会话锁） | `BiometricUnavailable`；**禁止静默降级** |
| 剪贴板排除监控 | `exclude_from_monitoring` | `exclude_from_history` | 无 | 仅 TTL 清除；UI 提示 |
| 系统锁屏监听 | `screen_lock/windows.rs` | `screen_lock/macos.rs` | `screen_lock/linux.rs` | 依赖 `vault_lock_on_screen_lock`；不可用时文档声明 |
| 原生解锁窗口 | `unlock_dialog/windows/*` | `unlock_dialog/macos.rs` | `unlock_dialog/linux.rs` | 回退到 WebView 表单；口令不得写入日志 |
| OS keyring | Windows Credential Manager | Keychain | libsecret | 生物识别不可用 |

规则：

1. 平台能力必须在注册时**探测一次**并缓存；探测失败只影响该能力，不影响口令路径。
2. 生物识别的 ACL 不满足时（macOS 无 `UserPresence`、Linux 未绑定会话锁）→ 直接禁用，不得 `set_key` 后使用。
3. `delete_key` 必须区分 not-found 与真实失败（key-hierarchy-spec §5.8 A-1）。
4. 平台能力差异必须在 UI 中显式说明，不得让用户误以为「已开启但没生效」。

---

## 5. 启动与退出顺序

### 5.1 启动

```
1. Tauri 插件 init
2. Windows: warm_up_keyring_backend()（失败只 warn，不阻断）
3. vault_setup()
   ├─ AppDirs::resolve()
   ├─ SecurityDbState::new(security/security.db)
   ├─ manage RevealGuard / ClipboardGuard / SshAgentState
   ├─ manage VaultRepo(security/vault)          # 仓库文件层，独立持有
   ├─ manage SyncEngine(共享同一 repo)          # 仅 git 同步
   └─ spawn_screen_lock_watcher()
4. 前端挂载 vault 模块（若 feature 开启）
```

约束：

1. `vault_setup` 失败必须让插件 setup 失败，不得带着半初始化状态继续。
2. keyring 预热失败不得阻断口令路径。
3. `AppDirs::resolve` 失败必须报错，不得回退到当前工作目录。
4. 生物识别、恢复、改密、轮换只依赖 `VaultRepo`，不得经 `SyncEngine` 取 repo；`remote_url = None`（从未配置同步）时这些功能必须照常可用（key §9.3）。

### 5.2 退出

```
1. vault_plugin::on_exit
2. lock_session(AppExit)
3. clipboard_force_clear()
```

禁止只清剪贴板。

### 5.3 目录布局

| 路径 | 内容 | 进仓库 | 备份 |
|---|---|---|---|
| `<data>/security/security.db` | SQLCipher 本机库 | 否 | 是（`backups/`） |
| `<data>/security/vault/` | git 仓库（`.vault-meta` + `entries/`） | 是（仅这两类） | 用户自行 |
| `<data>/security/backups/` | `security-*.db.bak` | 否 | 否 |
| `<data>/security/recovery/` | 灾难恢复临时 clone | 否 | 否（用后删） |
| `<data>/audit/` | JSONL 审计（v1.5） | 否 | 否 |
| `<conf>/vault-config.json` | 配置（含可能带凭据的 URL） | 否 | 否 |

---

## 6. 失败模式

| 失败 | 行为 |
|---|---|
| `feature` 关闭 | 所有 vault 命令不存在；前端入口不渲染 |
| 配置损坏 | `vault_get_conf` 返回 `Validation`；不自动重置配置 |
| keyring 不可用 | 生物识别不可用；口令路径正常 |
| SQLCipher 打开失败 | `LocalDbKeyMismatch` 或 `VaultCorrupted`（按 key-hierarchy-spec 区分） |
| `.vault-meta` 版本不符 | `UnsupportedVaultVersion` / `VaultCorrupted` |
| `entries/*.enc` 版本不符 | `UnsupportedEntryFileVersion` |
| 仓库出现禁止文件 | `RepositoryContainsForbiddenFile` |
| 同步远端不可达 | `GitUnavailable`（可重试） |
| 屏幕锁监听不可用 | 记录 warn；自动锁定仍生效 |

---

## 7. 文件改动清单

| 文件 | 改动 |
|---|---|
| `src-tauri/src/vault_plugin.rs` | 命令注册一致性；退出顺序；feature-off 分支 |
| `src-tauri/src/cmd/vault/mod.rs` | `vault_setup` 失败即失败；启动清理 `.tmp`；`.gitignore` 最小化 |
| `src-tauri/crates/assistant-config/src/vault_conf.rs` | 校验（§2.2）；`autoLock=0` 语义；URL 形态 |
| `src-tauri/crates/assistant-config/tests/vault_conf.rs` | 新增边界值/非法值用例 |
| `src-tauri/crates/assistant-vault/src/security_db/schema.rs` | 版本注释；不改 DDL |
| `src-tauri/crates/assistant-vault/src/security_db/meta.rs` | `ensure_schema_version` 拒绝未知版本 |
| `src-tauri/crates/assistant-vault/src/vault_file/mod.rs` | `.vault-meta` 严格版本判定（见 sync-crypto-spec） |
| `src-tauri/crates/assistant-vault/src/security_db/biometric_ops.rs` | 平台能力探测与 ACL 判定 |
| `src/vault/core/vaultError.ts` | 新错误码 |
| `src/features/vault/module.ts` | 能力探测与关闭时的 `FeatureUnavailable` |
| `src-tauri/crates/assistant-vault/tests/command_registry.rs` | 已存在；扩展为「声明 ↔ `generate_handler!` ↔ 前端 service」三方一致性 |
| `src-tauri/crates/assistant-vault/tests/command_gate.rs` | 已存在；补「关闭 vault 时无命令」断言 |

---

## 8. 测试

| 编号 | 用例 | 断言 |
|---|---|---|
| T-C1 | `--no-default-features`（关闭 vault）构建 | 无 `vault_*` 命令；无状态装配 |
| T-C2 | 开启 vault 构建 | 命令集合与前端 `vault.service.ts` 双向一致 |
| T-C3 | `vault_auto_lock_minutes=0` | 不自动锁定 |
| T-C4 | `vault_auto_lock_minutes=99999` | `Validation` |
| T-C5 | `vault_clipboard_clear_seconds=0` | 不自动清除；UI 有二次确认 |
| T-C6 | `vault_cipher_algorithm="des"` | 反序列化失败/`Validation` |
| T-C7 | 切换算法后旧条目 | 仍可解密；新条目用新算法 |
| T-C8 | `.vault-meta.version=1` | `UnsupportedVaultVersion`；不修改文件 |
| T-C9 | `EntryFile.version=1` | `UnsupportedEntryFileVersion` |
| T-C10 | `SCHEMA_VERSION` 不匹配 | 明确错误，不静默继续 |
| T-C11 | keyring 不可用 | 生物识别 `BiometricUnavailable`；口令路径正常 |
| T-C12 | 配置损坏 | `Validation`；不覆盖用户配置 |
| T-C13 | 仓库出现 `security.db` | `RepositoryContainsForbiddenFile` |
| T-C14 | 退出 | `lock_session(AppExit)` 被调用 |

手工验收（M2.16，无自动化替代）：

| 编号 | 步骤 | 断言 |
|---|---|---|
| T-C8a | 放入 v1 库（`version=1`）→ 打开应用 | 解锁页显示「不支持升级」+ 重置入口 + 备份提示；仓库内文件 mtime 与内容 hash 均不变 |
| T-C8b | 点重置 → 输入 `RESET-VAULT` → 确认 | 审计落一条 `VaultRepositoryReset`；`<security>/vault-meta.bak` 存在且**仓库根目录无该文件**；`.vault-meta` / `security.db` / `entries/` / `audit/` 均不存在；界面回到初始化页且**未自动建库** |
| T-C8c | 重置确认前关闭对话框 | 磁盘无任何变化（无 `vault-meta.bak`、无删除） |
| T-C8d | 重置后重新初始化并新建条目 | 不再出现 `RepositoryContainsForbiddenFile`；`.vault-meta` 与 `entries/*.enc` 正常写入 |

---

## 9. 禁止事项

1. **禁止**在 `feature` 关闭时保留可调用的 `vault_*` 命令。
2. **禁止**静默回退配置默认值来掩盖非法配置。
3. **禁止**混用 `SCHEMA_VERSION` / `.vault-meta.version` / `i9:v1:` 三个版本概念。
4. **禁止**为 v1 库写迁移器或归档目录。
5. **禁止**在平台能力不满足时静默降级并声称已开启。
6. **禁止**只清剪贴板而不锁会话地退出。
7. **禁止**把 `security.db`、备份、审计、配置写入仓库。
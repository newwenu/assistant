# I9-5 — 密码管理箱：SSH Agent + 恢复密钥

> 本文档为 I9 密码管理箱规格的 **第 5 部分**，覆盖 P2A'（SSH Agent 集成）和 P2A''（恢复密钥 + 紧急访问）。
> 前置文档：[I9-vault-spec-4-git-sync.md](./I9-vault-spec-4-git-sync.md)（P2A Git 同步）
> 设计文档：[VAULT-DESIGN.md](../VAULT-DESIGN.md)
> 任务清单：[I9-vault-tasklist-p2.md](./I9-vault-tasklist-p2.md)

---

## 12. P2A' — SSH Agent 集成

### 12.1 交付目标

实现 SSH Agent 集成，将 vault 中存储的 SSH 私钥加载到系统 SSH Agent（ssh-agent / Pageant），实现 Git 操作和远程服务器连接的无密码认证。

> **前置依赖**：P1-A ~ P1-F + P2A 完成
> **参考项目**：KeePassXC（`src/ssh/AgentSettings.cpp` + `ssh-agent.cpp`）、1Password（SSH Agent 服务）

### 12.2 SSH Agent 架构

```
┌─────────────────────────────────────────────────────┐
│  vault_entries (SQLite)                             │
│  ┌─────────────────────────────────────────────┐    │
│  │ category = "ssh"                            │    │
│  │ encrypted: { privateKey, publicKey, ... }   │    │
│  └─────────────────────────────────────────────┘    │
│         │ decrypt                                   │
│         ▼                                           │
│  ┌─────────────────────────────────────────────┐    │
│  │ SSH Agent Bridge (Rust)                     │    │
│  │  - add_to_agent(key)                        │    │
│  │  - remove_from_agent(fingerprint)           │    │
│  │  - list_agent_keys()                        │    │
│  └─────────────────────────────────────────────┘    │
│         │                                           │
│         ▼                                           │
│  ┌─────────────────────────────────────────────┐    │
│  │ System SSH Agent                            │    │
│  │  Windows: Pageant / OpenSSH Agent           │    │
│  │  macOS:   ssh-agent (via SSH_AUTH_SOCK)     │    │
│  │  Linux:   ssh-agent (via SSH_AUTH_SOCK)     │    │
│  └─────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────┘
```

### 12.3 SSH Agent Bridge trait

**`ssh_agent/mod.rs`**：

```rust
pub trait SshAgentBridge: Send + Sync {
    fn add_key(&self, private_key: &[u8], fingerprint: &str, lifetime: Option<u32>) -> Result<()>;
    fn remove_key(&self, fingerprint: &str) -> Result<()>;
    fn list_keys(&self) -> Result<Vec<SshKeyInfo>>;
    fn is_agent_running(&self) -> Result<bool>;
}

pub struct SshKeyInfo {
    pub fingerprint: String,
    pub key_type: String,
    pub comment: String,
    pub bits: u32,
}

pub fn create_agent_bridge() -> Box<dyn SshAgentBridge> {
    #[cfg(target_os = "windows")]
    {
        if ssh_agent_config().use_pageant() {
            return Box::new(windows::PageantBridge);
        }
        return Box::new(windows::OpenSshAgentBridge);
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    return Box::new(unix::SshAgentBridge);
}
```

### 12.4 平台实现

| 平台 | Agent | 通信方式 | 添加密钥 | 移除密钥 |
|------|-------|---------|---------|---------|
| **Windows** | Pageant / OpenSSH Agent | WM_COPYDATA 消息 / 命名管道 (`\\.\pipe\openssh-ssh-agent`) | `ssh-add` | `ssh-add -d` |
| **macOS** | ssh-agent | Unix socket (`SSH_AUTH_SOCK`) | `ssh-add`（直接 socket 通信） | `ssh-add -d` |
| **Linux** | ssh-agent | Unix socket (`SSH_AUTH_SOCK`) | `ssh-add` | `ssh-add -d` |

> **实现策略**：使用 `ssh-add` 命令行工具作为统一接口，避免直接实现 SSH Agent 协议（复杂且各平台差异大）。`ssh-add` 在所有平台均可用（Windows 通过 Git for Windows 自带的 `ssh-add.exe`）。

> **Linux `SSH_AUTH_SOCK` 说明**：Linux 桌面环境下 `SSH_AUTH_SOCK` 可由 `gnome-keyring`、`kwallet` 或 OpenSSH `ssh-agent` 提供，三者均实现相同的 SSH Agent 协议，对调用方完全透明，无需区分。Windows 上 Pageant（WM_COPYDATA）与 OpenSSH Agent（命名管道）通信协议不同，需用户选择或同时支持（参照 KeePassXC `usePageant` / `useOpenSSH` 设置）。

### 12.5 SSH 密钥生命周期

| 事件 | 行为 |
|------|------|
| vault 解锁 | 若 `vaultSshAutoAdd = true` → 自动添加所有 `category = "ssh"` 条目到 Agent |
| vault 锁定 | 从 Agent 移除所有由 vault 添加的密钥（追踪 fingerprint 列表） |
| 添加 SSH 条目 | 若 vault 已解锁且 Agent 运行 → 自动添加到 Agent |
| 删除 SSH 条目 | 从 Agent 移除对应密钥 |
| vault 关闭 | 等同锁定 → 移除所有 |

**追踪机制**：`SshAgentState`（Tauri State）维护 `added_fingerprints: Mutex<HashSet<String>>`，记录由 vault 添加到 Agent 的密钥指纹。锁定时仅移除这些密钥，不影响用户手动添加的其他密钥。

### 12.6 SSH 密钥安全约束

| 约束 | 说明 |
|------|------|
| 私钥仅在内存中短暂存在 | 解密 → 添加到 Agent → zeroize 明文副本 |
| Agent 添加需确认 | 首次添加时弹窗确认（显示 fingerprint + comment） |
| 密钥格式校验 | 添加前校验私钥格式（OpenSSH / PEM / PuTTY `.ppk`），格式错误返回 `VaultError::InvalidKeyFormat` |
| passphrase 支持 | 若私钥有 passphrase → 解密 vault 条目后得到 passphrase → 用 passphrase 解密私钥 → 添加到 Agent |
| 自动添加可关闭 | `vaultSshAutoAdd` 设置项控制，默认 `false`（安全优先） |

### 12.7 SSH 条目数据模型

`category = "ssh"` 的 VaultEntry，`encryptedBlob` 解密后包含：

```json
{
  "title": "GitHub SSH Key",
  "privateKey": "-----BEGIN OPENSSH PRIVATE KEY-----\n...",
  "publicKey": "ssh-ed25519 AAAA... user@host",
  "fingerprint": "SHA256:abc123...",
  "passphrase": "",
  "keyType": "ed25519",
  "keyBits": 256,
  "comment": "user@host",
  "tags": ["work", "github"],
  "notes": "Generated 2024-01"
}
```

> `fingerprint` / `keyType` / `keyBits` / `comment` 为元数据，由 Rust 端从公钥解析，不依赖用户手动输入。

### 12.8 Tauri Commands

| Command | 签名 | 说明 |
|---------|------|------|
| `vault_ssh_add_to_agent` | `(entryId: String) → void` | 解密 SSH 私钥 → 添加到系统 Agent → zeroize 明文 |
| `vault_ssh_remove_from_agent` | `(entryId: String) → void` | 从 Agent 移除指定条目的密钥 |
| `vault_ssh_list_agent` | `() → Vec<SshKeyInfo>` | 列出 Agent 中所有密钥（含 vault 添加标记） |
| `vault_ssh_auto_add_all` | `() → u32` | 自动添加所有 SSH 条目到 Agent，返回添加数量 |

### 12.9 前端集成

#### 12.9.1 vaultStore 新增

| 字段 | 类型 | 说明 |
|------|------|------|
| `sshAgentKeys` | `SshKeyInfo[]` | Agent 中当前密钥列表 |
| `sshAutoAddEnabled` | `boolean` | 是否自动添加 |

**新增 actions**：

| Action | 签名 | 说明 |
|--------|------|------|
| `addSshToAgent` | `(entryId) → Promise<void>` | 添加到 Agent |
| `removeSshFromAgent` | `(entryId) → Promise<void>` | 从 Agent 移除 |
| `loadSshAgentKeys` | `() → Promise<void>` | 刷新 Agent 密钥列表 |

#### 12.9.2 VaultEntryDetail 扩展

`category === "ssh"` 时显示：
- 密钥类型 + 位数（`ed25519 256-bit`）
- 指纹（`SHA256:abc123...`）
- 公钥（可复制）
- [添加到 Agent] / [从 Agent 移除] 按钮

#### 12.9.3 VaultSettings 扩展

新增 SSH Agent 设置区：
- 自动添加开关（`vaultSshAutoAdd`）
- Agent 状态指示（运行/未运行）
- 当前 Agent 密钥列表
- Windows: Pageant / OpenSSH Agent 选择（`vaultSshUsePageant` / `vaultSshUseOpenSshAgent`，可同时启用）
- Linux/macOS: `SSH_AUTH_SOCK` 路径覆盖（`vaultSshAuthSockOverride`，为空时使用环境变量）

### 12.10 设置项扩展

| 前端字段 (AppConf) | Rust 字段 (app_conf.rs) | 类型 | 默认值 | 说明 |
|---------------------|------------------------|------|--------|------|
| `vaultSshAutoAdd` | `vault_ssh_auto_add` | `bool` / `boolean` | false | 自动添加 SSH 密钥到 Agent |
| `vaultSshUsePageant` | `vault_ssh_use_pageant` | `bool` / `boolean` | true | Windows: 使用 Pageant（WM_COPYDATA） |
| `vaultSshUseOpenSshAgent` | `vault_ssh_use_openssh_agent` | `bool` / `boolean` | true | Windows: 使用 OpenSSH Agent（命名管道） |
| `vaultSshAuthSockOverride` | `vault_ssh_auth_sock_override` | `string` / `String` | `""` | Linux/macOS: 自定义 `SSH_AUTH_SOCK` 路径 |

### 12.11 i18n

`tools.password_vault` 命名空间下新增约 20 个词条，覆盖：SSH Agent 状态、添加/移除操作、密钥类型、指纹、自动添加设置、确认对话框、Windows Agent 类型选择、`SSH_AUTH_SOCK` 路径覆盖。

---

## 13. P2A'' — 恢复密钥 + 紧急访问

### 13.1 交付目标

实现恢复密钥（Recovery Key）机制，允许用户在忘记主密码时通过恢复密钥解锁密码箱，并保护 `.vault-meta` 完整性防止灾难恢复时元数据被篡改。

> **前置依赖**：P1-A ~ P1-F + P2A 完成
> **参考项目**：Bitwarden（恢复代码）、1Password（紧急访问）

### 13.2 恢复密钥方案

恢复密钥为 20 字符的 Base32 编码随机字符串（160 bit 熵），生成后仅显示一次，用户需安全保存。

**恢复密钥格式**：`XXXXX-XXXXX-XXXXX-XXXXX`（4 组 5 字符，`-` 分隔，便于抄写和朗读）

**生成流程**（`vault_generate_recovery_key`）：

1. 生成 20 字节随机数据：`raw_key = rand::[u8; 20]`
2. Base32 编码（RFC 4648，无 padding）：`recovery_key = base32(raw_key)`
3. 插入分隔符：`formatted = XXXXX-XXXXX-XXXXX-XXXXX`
4. Argon2id 派生加密密钥：`recovery_derived_key = argon2id(password=recovery_key, salt=random_16B, m=65536, t=3, p=4)`
5. 加密 master_key：`encrypted_master_key = AES-256-GCM(key=recovery_derived_key, nonce=random_12B, plaintext=master_key_bytes)`
6. 存入 `.vault-meta` 的 `emergency` 字段：
   ```json
   {
     "token": "base64(nonce‖encrypted_master_key‖tag)",
     "salt": "base64(argon2id_salt)"
   }
   ```
7. **zeroize** `raw_key` / `recovery_derived_key` / `master_key_bytes` 副本
8. 返回 `formatted`（**仅此一次**，后续无法再次查看）

> **安全要点**：
> - 恢复密钥**不等同于**主密码——它是对 master_key 的加密包装，不是新的认证凭据
> - 恢复密钥生成后**不存储明文**——仅存加密后的 master_key（`emergency.token`）和派生盐（`emergency.salt`）
> - 恢复密钥的熵（160 bit）与主密码相当（Argon2id 派生后为 256 bit 密钥空间）
> - 每次生成新恢复密钥会**覆盖**旧的

### 13.3 恢复密钥解锁流程

**`vault_unlock_with_recovery_key(recoveryKey)`**：

1. 格式校验：移除 `-` 分隔符 → 校验 Base32 字符集 → 长度 = 20
2. 读取 `.vault-meta` 的 `emergency.token` + `emergency.salt`
   - 若不存在 → 返回 `VaultError::RecoveryKeyNotSet`
3. Argon2id 派生：`recovery_derived_key = argon2id(password=recoveryKey, salt=emergency.salt, ...)`
4. 解密 master_key：
   - base64 decode `emergency.token` → 提取 `nonce` + `encrypted_master_key` + `tag`
   - `master_key_bytes = AES-256-GCM-Decrypt(key=recovery_derived_key, nonce, ciphertext)`
   - 解密失败 → 返回 `VaultError::InvalidRecoveryKey`
5. 验证 master_key：用 `master_key_bytes` 派生 `sqlcipher_key` → 尝试打开 `security.db`
   - 打开失败 → 返回 `VaultError::InvalidRecoveryKey`（恢复密钥正确但数据库损坏）
6. 恢复 vault 状态：存入 `SecurityDbState` 的 `conn` + `master_key`
7. **审计日志**：`audit_log(conn, AuditEvent::VaultUnlock, Some("recovery_key"))`
8. **zeroize** `recovery_derived_key` / `master_key_bytes` 副本
9. 返回 `Ok(true)`

> **master_key 不经 IPC**：与生物识别解锁一致，整个流程在 Rust 端完成。

### 13.4 .vault-meta 完整性保护

`.vault-meta` 的 `integrity` 字段使用 HMAC-SHA256 签名，防止灾难恢复时元数据被篡改：

**签名计算**：

```rust
fn compute_vault_meta_hmac(master_key: &[u8], meta: &VaultMetaContent) -> [u8; 32] {
    let content = serde_json::to_vec(meta)?;  // 序列化除 integrity 外的所有字段
    hmac_sha256(master_key, &content)
}
```

**验证时机**：

| 时机 | 行为 |
|------|------|
| Git pull 后 | 验证拉取的 `.vault-meta` 签名 → 失败则拒绝应用变更 + 提示用户 |
| 恢复密钥解锁 | 验证本地 `.vault-meta` 签名 → 失败则警告"元数据可能被篡改" + 允许继续（用户确认） |
| vault 初始化 | 生成初始 `.vault-meta` + 计算签名 |
| vault_meta 变更 | 重新计算签名 + 写入 `.vault-meta` |

> **安全保证**：HMAC 密钥为 master_key，攻击者无法伪造签名（除非已获取 master_key，此时密码箱已被完全攻破，签名保护已无意义）。

### 13.5 灾难恢复流程

灾难恢复是一个**独立的只读解密工具**，不修改任何本地数据库状态，仅从 Git 仓库解密数据并导出为 JSON 文件。用户可自行导入到任意密码库（本应用或其他密码管理器）。

**设计原则**：
- **数据到手即成功**：灾难恢复的核心目标是把密码还给用户，不需要重建数据库
- **只读无副作用**：不创建/修改/删除 `security.db`，不改变任何本地状态
- **通用格式导出**：输出标准 JSON，与现有 `vault_export` 格式一致，可直接用 `vault_import` 导入
- **不依赖本地状态**：无论 `security.db` 是否存在、是否损坏，都能工作

**触发场景**（不仅限于新设备初始化）：

| 场景 | 本地状态 | 说明 |
|------|---------|------|
| 新设备，什么都没有 | `!isInitialized` | 最常见的灾难恢复场景 |
| 忘记主密码 | `isInitialized && isLocked` | 无法用主密码解锁 |
| DB 文件损坏/误删 | `isInitialized` 但解锁报错 | 本地数据不可信 |
| DB 存在但 SQLCipher 打不开 | 解锁返回 Database 错误 | DB 结构损坏 |
| 改密中途崩溃 | 解锁失败 | DB 状态不一致 |

**流程**（`vault_disaster_recovery_export`）：

1. **Git 仓库准备**：
   - 若提供 `gitUrl` → `clone_repo(url)` 克隆远程仓库
   - 若未提供 → 使用本地已有仓库（`prepare()` + `pull()`）
2. **读取 emergency data**：从 `.vault-meta` 读取 `emergency.token` + `emergency.salt`
   - 若不存在 → 返回 `VaultError::RecoveryKeyNotSet`
3. **恢复密钥解密**：
   - Argon2id 派生 → AES-256-GCM 解密 → 得到 `master_key` + `bootstrap_key`
   - 解密失败 → 返回 `VaultError::InvalidRecoveryKey`
4. **验证完整性**：用 `master_key` 验证 `.vault-meta` 的 HMAC 签名
   - 失败 → 警告但不阻止（记录日志）
5. **解密全部条目**：遍历 `entries/*.enc` → `per_entry_key = HKDF(master_key, entry_id)` → AES-256-GCM 解密 → 组装为 `VaultEntryExport` 列表
6. **导出 JSON**：序列化为 JSON 文件，写入 `exports/` 目录，5 分钟后自动清理
7. **zeroize** `master_key` / `bootstrap_key` 副本
8. **返回**：导出文件路径 + 条目数量

**导出 JSON 格式**（与 `vault_export` 完全一致）：

```json
[
  {
    "category": "login",
    "title": "Example",
    "username": "user@example.com",
    "url": "https://example.com",
    "password": "decrypted_password",
    "notes": "decrypted_notes",
    "totpSecret": "base32_totp_secret",
    "tags": ["tag1"],
    "customFields": [],
    "isFavorite": false,
    "createdAt": 1697980800,
    "updatedAt": 1697980800
  }
]
```

**用户后续操作**：

1. 创建新密码库（`vault_init` + 新主密码）
2. 导入 JSON（`vault_import`，路径为灾难恢复返回的文件路径）
3. 灾难恢复完成 — 密码库在新设备上可用

> **与原设计的区别**：原设计试图在灾难恢复流程中自动重建 `security.db`，需要扩展 `EmergencyData` 存储密码验证数据、实现密钥切换导入等复杂逻辑。新设计将灾难恢复定位为"只读解密 + 导出"工具，复用现有 `vault_export`/`vault_import` 格式，大幅降低实现复杂度，且用户可自由选择导入目标。

### 13.6 恢复密钥轮换

用户可随时生成新的恢复密钥（旧密钥立即失效）：

1. 验证 vault 已解锁
2. 执行 §13.2 生成流程（覆盖 `emergency` 字段）
3. 重新计算 `.vault-meta` 的 HMAC 签名
4. 若 Git 同步已启用 → `sync.prepare()` + `sync.finalize("vault: recovery key rotated")`

### 13.7 Tauri Commands

| Command | 签名 | 说明 |
|---------|------|------|
| `vault_generate_recovery_key` | `() → String` | 生成恢复密钥（仅返回一次） |
| `vault_unlock_with_recovery_key` | `(recoveryKey: String) → bool` | 恢复密钥解锁（需 `security.db` 存在） |
| `vault_has_recovery_key` | `() → bool` | 检查是否已设置恢复密钥 |
| `vault_verify_vault_meta_integrity` | `() → IntegrityResult` | 验证 .vault-meta 完整性 |
| `vault_disaster_recovery_export` | `(gitUrl: Option\<String\>, recoveryKey: String) → DisasterRecoveryResult` | 灾难恢复：从 Git 仓库解密并导出 JSON |

**DisasterRecoveryResult**：

| 字段 | 类型 | 说明 |
|------|------|------|
| `filePath` | `String` | 导出 JSON 文件路径 |
| `entryCount` | `u32` | 解密导出的条目数量 |
| `integrityWarning` | `bool` | HMAC 验证是否失败（警告） |

**IntegrityResult**：

| 字段 | 类型 | 说明 |
|------|------|------|
| `valid` | `bool` | 签名是否有效 |
| `message` | `String` | 验证结果描述 |

### 13.8 前端集成

#### 13.8.1 vaultStore 新增

| 字段 | 类型 | 说明 |
|------|------|------|
| `hasRecoveryKey` | `boolean` | 是否已设置恢复密钥 |
| `disasterRecoveryStep` | `1 \| 2 \| 3 \| null` | 灾难恢复当前步骤（null = 不在流程中） |
| `disasterRecoveryResult` | `DisasterRecoveryResult \| null` | 灾难恢复导出结果 |

**新增 actions**：

| Action | 签名 | 说明 |
|--------|------|------|
| `generateRecoveryKey` | `() → Promise<string>` | 生成恢复密钥（返回明文，仅一次） |
| `unlockWithRecoveryKey` | `(key) → Promise<boolean>` | 恢复密钥解锁 |
| `disasterRecoveryExport` | `(gitUrl?, recoveryKey) → Promise<DisasterRecoveryResult>` | 灾难恢复导出 |
| `checkRecoveryKey` | `() → Promise<void>` | 检查是否已设置恢复密钥 |

#### 13.8.2 VaultUnlock 扩展

在生物识别按钮下方新增恢复密钥入口 + **始终显示**灾难恢复入口：

```
┌──────────────────────────────────┐
│      🔒 密码管理箱                │
│                                  │
│  [  主密码输入框  ]               │
│  [  解锁 / 创建密码箱  ]          │
│                                  │
│  ─── 或 ───                      │
│  [  🛂 生物识别解锁  ]           │  ← biometricAvailable && biometricEnrolled
│  [  🔑 恢复密钥解锁  ]           │  ← hasRecoveryKey && isInitialized
│  [  🔄 从云端恢复   ]            │  ← 始终显示
│                                  │
└──────────────────────────────────┘
```

点击"恢复密钥解锁" → 弹窗输入恢复密钥 → `unlockWithRecoveryKey(key)`

点击"从云端恢复" → 在同一页面内切换为步骤式 UI：

**步骤 1：连接 Git 仓库**
- Git 仓库地址输入框（留空使用本地已有仓库）
- "下一步"按钮

**步骤 2：输入恢复密钥**
- 恢复密钥输入框
- "下一步"按钮

**步骤 3：导出完成**
- 显示导出结果：条目数量 + 文件路径
- "打开文件位置"按钮
- 提示用户：创建新密码库 → 导入 JSON 文件
- "完成"按钮

#### 13.8.3 VaultSettings 扩展

新增恢复密钥设置区：
- [生成恢复密钥] 按钮（点击后弹窗显示密钥，提示抄写保存，关闭后无法再次查看）
- [轮换恢复密钥] 按钮（需输入主密码确认）
- 恢复密钥状态：已设置 / 未设置

#### 13.8.4 恢复密钥显示弹窗

```
┌──────────────────────────────────────────────────────┐
│  ⚠️ 恢复密钥                                        │
│                                                      │
│  请将以下恢复密钥安全保存。关闭此窗口后将无法再次查看。│
│                                                      │
│  ┌──────────────────────────────────────────────┐    │
│  │  ABCDE-FGHIJ-KLMNO-PQRST                   │    │
│  └──────────────────────────────────────────────┘    │
│                                                      │
│  [复制]  [我已安全保存]                              │
│                                                      │
│  ⚠️ 建议保存到：物理保险箱、密码管理器备份、纸质备份  │
└──────────────────────────────────────────────────────┘
```

#### 13.8.5 vault.service.ts 扩展

```typescript
generateRecoveryKey: (): Promise<string> => invoke("vault_generate_recovery_key"),
unlockWithRecoveryKey: (recoveryKey: string): Promise<boolean> =>
  invoke("vault_unlock_with_recovery_key", { recoveryKey }),
hasRecoveryKey: (): Promise<boolean> => invoke("vault_has_recovery_key"),
verifyVaultMetaIntegrity: (): Promise<IntegrityResult> =>
  invoke("vault_verify_vault_meta_integrity"),
disasterRecoveryExport: (gitUrl?: string, recoveryKey: string): Promise<DisasterRecoveryResult> =>
  invoke("vault_disaster_recovery_export", { gitUrl: gitUrl ?? null, recoveryKey }),
```

### 13.9 i18n

`tools.password_vault` 命名空间下新增约 15 个词条，覆盖：恢复密钥生成/解锁/轮换、完整性验证、灾难恢复提示、弹窗文案。

### 13.10 跨阶段约束

| 维度 | 设计文档 | 本 spec | 对齐状态 |
|------|---------|---------|---------|
| SSH Agent 集成 | §3.6 | §12 | ✅ 一致 |
| 恢复密钥方案 | §3.7 | §13.2-13.3 | ✅ 一致 |
| .vault-meta 完整性 | §3.7.1 | §13.4 | ✅ 一致 |
| 灾难恢复流程 | §3.7.2 | §13.5 | ✅ 一致 |
| SSH Commands | §5 SSH 部分 | §12.8 | ✅ 一致 |
| 恢复密钥 Commands | §5 紧急访问部分 | §13.7 | ✅ 一致 |

**安全不变式**：

| 不变式 | 验证方式 |
|--------|---------|
| SSH 私钥明文不进 JS 堆 | 代码审查：add_to_agent 在 Rust 端完成 |
| SSH 私钥添加后 zeroize | 代码审查：add 后立即 zeroize |
| 恢复密钥仅返回一次 | 代码审查：generate 后不存明文 |
| .vault-meta HMAC 签名有效 | 单元测试：篡改后验证失败 |
| 恢复密钥解锁后 master_key 不经 IPC | 代码审查：与生物识别解锁一致 |
# I9-4 — 密码管理箱：Git 同步

> 本文档为 I9 密码管理箱规格的 **第 4 部分**，覆盖 P2A（Git 同步 + assistant-sync crate）。
> 前置文档：[I9-vault-spec-1-foundation.md](./I9-vault-spec-1-foundation.md)（P1-A 安全基础）、[I9-vault-spec-2-crud-and-ui.md](./I9-vault-spec-2-crud-and-ui.md)（P1-B/C）、[I9-vault-spec-3-security-enhance.md](./I9-vault-spec-3-security-enhance.md)（P1-D/E/F）
> 后续文档：[I9-vault-spec-5-ssh-and-recovery.md](./I9-vault-spec-5-ssh-and-recovery.md)（P2A' SSH Agent + P2A'' 恢复密钥）
> 设计文档：[VAULT-DESIGN.md](../VAULT-DESIGN.md)
> 任务清单：[I9-vault-tasklist-p2.md](./I9-vault-tasklist-p2.md)

---

## 11. P2A — Git 同步 + assistant-sync crate

### 11.1 交付目标

新建 `assistant-sync` crate，实现 Git 同步引擎（参考 prs `lib/src/sync.rs` + `lib/src/git.rs`），将密码条目导出为 `.enc` 加密文件存入 Git 仓库，通过 `prepare → 操作 → finalize` 模式自动嵌入每次密码操作，实现多设备同步、冲突解决、离线支持。

> **前置依赖**：P1-A ~ P1-F 全部完成
> **参考项目**：prs（Git 同步模式 — prepare/finalize、Readyness 状态机、智能推送优化）

### 11.2 assistant-sync crate 结构

```
assistant-sync/
├── src/
│   ├── lib.rs                    ← crate 入口，pub mod 声明
│   ├── git_sync/                 ← Git 同步引擎
│   │   ├── mod.rs                ← GitSync struct + init/clone/prepare/finalize/readyness/need_to_push（~200 行）
│   │   ├── git.rs                ← git CLI 封装（git(args) → Result<Output>）（~80 行）
│   │   └── ssh.rs                ← SSH ControlMaster 复用（Unix only，~60 行）
│   ├── sync_engine/              ← 同步引擎（pull/push/sync）
│   │   └── mod.rs                ← SyncEngine struct + pull/push/sync_now/force_push（~120 行）
│   ├── vault_file/               ← vault 文件序列化/反序列化
│   │   ├── mod.rs                ← export_entry / import_entry / export_all / import_all（~150 行）
│   │   └── types.rs              ← EncFile / VaultBlob / VaultMeta / DeletedJson（~60 行）
│   └── types/                    ← 公共同步类型
│       └── mod.rs                ← Readyness / SyncResult / SyncConfig / RepositoryState（~50 行）
├── Cargo.toml
└── tests/
    ├── git_sync.rs               ← GitSync 单元测试（~80 行）
    └── vault_file.rs             ← vault_file 单元测试（~60 行）
```

**P2A 阶段代码量估算**：~800 行

### 11.3 GitSync 核心 API

**Readyness 就绪状态**（参考 prs `lib/src/sync.rs`）：

| 变体 | 说明 |
|------|------|
| `NoSync` | 未初始化 git |
| `RepoState(RepositoryState)` | Git 异常状态（Merge / Rebase / RebaseInteractive / CherryPick / Unknown） |
| `Dirty` | 有未提交变更 |
| `Ready` | 就绪，可操作 |

**RepositoryState 枚举**：

| 变体 | 说明 |
|------|------|
| `Merge` | 合并冲突中 |
| `Rebase` | rebase 进行中 |
| `RebaseInteractive` | 交互式 rebase |
| `CherryPick` | cherry-pick 进行中 |
| `Unknown` | 其他异常状态 |

**GitSync struct**（`git_sync/mod.rs`）：

| 方法 | 输入 | 输出 | 行为 |
|------|------|------|------|
| `init(repo_path)` | 仓库路径 | `Result<()>` | 若已初始化则跳过；`git init` + 空提交 |
| `clone(url, repo_path)` | 远程 URL + 路径 | `Result<()>` | `git clone url repo_path`，检查退出码 |
| `prepare(repo_path)` | 仓库路径 | `Result<()>` | 若已初始化且有 remote → `git pull -q`；SSH 连接复用（§11.9） |
| `finalize(repo_path, msg)` | 路径 + 提交消息 | `Result<()>` | 若 dirty → `git add .` + `git commit -q -m msg`；若有 remote 且 `need_to_push()` → `git push -q` |
| `readyness(repo_path)` | 仓库路径 | `Result<Readyness>` | 返回当前 Readyness 状态 |
| `need_to_push(repo_path)` | 仓库路径 | `Result<bool>` | 智能推送优化：30s 内 pull 过且本地/远程 hash 相同 → 跳过 push。elapsed 不可得时保守返回 true |
| `add_remote(repo_path, name, url)` | 路径 + 名称 + URL | `Result<()>` | `git remote add name url` |
| `set_remote(repo_path, name, url)` | 路径 + 名称 + URL | `Result<()>` | `git remote set-url name url` |
| `pull(repo_path)` | 仓库路径 | `Result<()>` | `git pull -q` |
| `push(repo_path)` | 仓库路径 | `Result<()>` | `git push -q` |

> **关键约束**：所有 `git` 子命令调用统一通过 `git(args) → Result<Output>` 辅助方法（`git_sync/git.rs`），检查 `output.status.success()`，失败时返回 `format!("git {} failed: {}", args, stderr)`。**禁止**直接 `.status()?` 忽略退出码。

### 11.4 git CLI 封装

**`git_sync/git.rs`**：

```rust
fn git(args: &[&str], repo_path: &Path) -> Result<Output> {
    Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .env("GIT_TERMINAL_PROMPT", "0")  // 禁用交互式认证提示
        .output()
        .map_err(|e| format!("git spawn failed: {}", e))
        .and_then(|o| {
            if o.status.success() { Ok(o) }
            else { Err(format!("git {} failed: {}", args.join(" "), String::from_utf8_lossy(&o.stderr))) }
        })
}
```

- `GIT_TERMINAL_PROMPT=0`：禁止 git 弹出交互式密码提示，认证失败直接返回错误
- 所有 git 调用均通过此函数，统一错误处理和日志

### 11.5 Git 仓库结构

```
{app_data_dir}/security/vault/     ← Git 仓库
├── .git/                          ← Git 元数据
├── .vault-meta                    ← vault 元信息（含密钥元数据）
├── entries/
│   ├── a1b2c3d4.enc              ← 单条目加密文件（文件名 = entry_id）
│   ├── e5f6g7h8.enc
│   └── ...
```

> 早期设计的 `deletions/` 目录与 `deleted.json` 墓碑索引**已彻底移除**：既不写入、也不读取，代码中不再保留任何相关常量、结构体或函数。删除状态由 `vault_entries.deleted_at`（本地）与 Git 自身的删除记录（远端）承载。

### 11.6 .vault-meta 文件

`.vault-meta` 存储版本号 + **非秘密的密钥元数据**（salt 是公开随机数，加密后的 master_key 是密文），确保灾难恢复时仅凭 Git 仓库 + 恢复密钥/主密码即可还原全部密码：

```json
{
  "version": 1,
  "exportedAt": 1729555200,
  "crypto": {
    "masterKeySalt": "base64(32字节 Argon2id salt，用于派生 master_key)",
    "hashSalt": "base64(32字节 Argon2id salt，用于派生 master_key_hash)",
    "argon2Params": { "m": 65536, "t": 3, "p": 4 },
    "defaultCipher": "aes256Gcm"
  },
  "emergency": {
    "token": "base64(加密后的 master_key)",
    "salt": "base64(恢复密钥的 Argon2id salt)"
  },
  "integrity": "base64(HMAC-SHA256(master_key, version+crypto+emergency))"
}
```

**完整性保护**：`integrity` 字段使用 HMAC-SHA256(master_key, 其余字段内容) 签名。签名密钥为 master_key，攻击者无法伪造。

**校验归属同步职责**：`.vault-meta` 完整性校验是**同步流程的固有步骤**，不是恢复流程的附加动作。`SyncEngine::pull()` 在 `git pull` 成功之后、`import_all` 之前必须完成校验；校验不通过则中止本次同步——**不写库、不 finalize、不 push**，返回 `VaultError::VaultMetaIntegrityFailed`。恢复流程（灾难恢复 / 恢复密钥解锁）只消费同一校验函数的结果，不自行定义策略。

| 校验结果 | 同步行为 |
|---|---|
| 签名有效 | 继续 `import_all` |
| 签名不匹配 | 中止同步，不执行任何 DB 写入；前端提示「元数据可能被篡改」 |
| `integrity` 为空 / `.vault-meta` 缺失 | 视为**未签名**，同样中止自动导入；需用户确认后由本地 master_key 重新签名（`export_vault_meta`）方可继续 |
| 仅本地 CRUD（无 pull） | 校验失败不阻断本地操作，仅在同步时阻断 |

**禁止产生未签名元数据**：首次初始化必须由 `export_vault_meta(master_key)` 生成带签名的 `.vault-meta`；禁止把 `integrity: ""` 的空壳 meta 推送到仓库，否则所有设备 pull 时都会判定未签名而中止同步。

**绝不存入仓库的字段**：

| vault_meta key | 排除理由 |
|---|---|
| `master_key_hash` | 🔴 **绝对禁止** — 主密码的 Argon2id 哈希，若与 `hashSalt` 同时泄露，攻击者可离线暴力破解主密码 |
| `sync_config` | 🔴 含 Git 认证凭据，推入仓库即泄露 |
| `sync_remote_etag` / `sync_last_pull_at` | 设备绑定的同步状态，跨设备无意义 |
| `biometric_key_enc` | 设备绑定，灾难恢复不可用 |
| `hibp:{entry_id}` | 外部审计缓存，可重新查询 |

**双写一致性**：`vault_meta` 表变更时同步更新 `.vault-meta` 文件，与条目双写逻辑一致（步骤 2-5 在同一 DB 事务中）。

### 11.7 .enc 文件格式

每个 `.enc` 文件采用**整体打包加密**（与 prs 整体 GPG 加密的模式一致），将所有敏感字段序列化为一个 JSON blob 后一次加密：

```json
{
  "id": "a1b2c3d4-...",
  "category": "login",
  "revision": 3,
  "encryptedBlob": "base64(algo_byte‖nonce‖AES-256-GCM(per_entry_key, blob_json)‖tag)"
}
```

`encryptedBlob` 解密后的 `blob_json` 内容：

```json
{
  "title": "GitHub",
  "username": "user@example.com",
  "url": "https://github.com",
  "password": "s3cret!",
  "notes": "Personal account",
  "customFields": [{"name": "API Key", "value": "xxx", "isSecret": true}],
  "totpSecret": "JBSWY3DPEHPK3PXP",
  "totpDigits": 6,
  "totpPeriod": 30,
  "tags": ["dev", "work"],
  "isFavorite": true,
  "createdAt": 1729468800,
  "updatedAt": 1729555200
}
```

> **仓库整体打包 vs SQLite 逐字段——两层各取最优策略**：
>
> | 层 | 策略 | 理由 |
> |---|---|---|
> | **SQLite** | 逐字段加密 | 部分解密（HIBP 只解密 password、TOTP 只解密 totpSecret）、SQL LIKE 搜索（title/username/url 明文）、内存最小暴露 |
> | **Git .enc** | 整体打包加密 | 与 prs 模式一致、nonce/tag 开销仅 28B（vs 逐字段 196B）、灾难恢复一次解密即得全部数据、prepare/finalize 已避免冲突 |

仅 `id` / `category` / `revision` 为明文。`isFavorite` / `createdAt` / `updatedAt` 积入 `encryptedBlob`，元数据泄露面最小化。

### 11.8 vault_file 导出/导入

**导出**（SQLite 逐字段 → .enc 整体打包）：

1. `derive_entry_key(master_key, entry.id)` 派生条目级密钥
2. 将 `VaultEntryDecrypted` 各字段通过 `expose()` 读取，构造 `VaultBlob` 明文结构体
3. `serde_json::to_vec(&blob)` 序列化 → `VaultPlaintext::from(json_bytes)` → `encrypt_field(&plaintext, &entry_key, Aes256Gcm)` → base64 密文
4. 写入 `{id}.enc`：`{ id, category, revision, encrypted_blob }`

**导入**（.enc 整体打包 → SQLite 逐字段）：

1. 读取 `.enc` 文件 → 反序列化 `EncFile`
2. `derive_entry_key(master_key, enc_file.id)` → `decrypt_field(&enc_file.encrypted_blob, &entry_key)` → `VaultPlaintext`
3. `serde_json::from_slice(plaintext.expose())` → `VaultBlob`
4. 逐字段写入 SQLite（各自独立加密）

**导出全部**（`export_all`）：遍历 `vault_entries` 表所有**未删除**条目 → 逐条 `export_entry` 写入 `entries/`；随后删除 `deleted_at IS NOT NULL` 条目对应的 `.enc` 文件（让 Git 提交记录承载删除事实）

**导入全部**（`import_all`）：只把存在的 `.enc` 内容写入 SQLite，**不改动本地删除状态**：

1. 记录导入前 `entries/*.enc` 的文件集合 `before`（pull 前快照）
2. 遍历当前 `entries/*.enc` → 逐条 `import_entry` 写入 SQLite（新增或按 `revision` / `updatedAt` 覆盖）；**不读写 `deleted_at`**
3. 计算 `removed = before - current`，仅作为 `SyncResult.removedIds` 上报给前端提示「远程已删除 N 条」，**不自动标记 `deleted_at`**

> **本地数据库自主原则**：`vault_entries` 的增删改只由本地用户操作（`vault_delete_entry` / 恢复操作）驱动；同步流程只做条目内容的导入导出，绝不擅自修改本地删除状态。
>
> 防误删护栏（必须实现）：
> - 文件集合为空而 DB 非空 → 中止导入并报错，不执行任何写入
> - 仓库处于 merge/rebase 等冲突状态时跳过导入

### 11.8.1 同步流程中的完整性前置校验

`pull()` 的步骤顺序固定，不可调换：

```
1. git pull（或 clone）
2. read .vault-meta → verify_vault_meta_integrity(master_key)      ← 同步职责
3. 校验失败 → 中止：不 import_all、不写 DB、不 finalize、不 push，返回 VaultMetaIntegrityFailed
4. 校验通过 → import_all（§11.8 导入全部）
5. 返回 SyncResult（含 imported / removedIds / integrityWarning）
```

- 校验必须固定在 `import_all` **之前**：一旦先写库再校验，被篡改的 `.vault-meta` 已经影响本地数据，无法回退
- `sync_now` = `pull` + `push`，同样受此约束：pull 阶段校验失败则不进入 push
- 灾难恢复（clone 场景）复用同一校验函数：校验失败时仍允许导出，但不自动入库，并在结果中标记 `integrityWarning = true`
- 恢复密钥解锁场景：本机库已存在，签名失败仅告警并允许继续（风险限于元数据展示）

### 11.9 DB 与文件双写一致性

数据库写入和 .enc 文件导出必须保证一致性：

```
操作流程（以 create 为例）：
  1. sync.prepare()?;                          // git pull + .vault-meta 完整性校验（失败即中止）
  2. DB 事务开始
  3. vault::create_entry(conn, entry, key)?;   // 写入 vault_entries
  4. vault_file::export_entry(...)?;           // 导出 .enc 文件
  5. DB 事务提交
  6. sync.finalize("vault: added {title}")?;   // git commit + push
```

- 步骤 2-5 在同一个 DB 事务中：如果 .enc 导出失败，DB 写入也回滚
- 步骤 6 在事务外：finalize 失败不影响本地数据，下次操作时会重试

**delete 的差异**：步骤 3 为 `soft_delete_entry`（仅置 `deleted_at`），步骤 4 改为删除对应 `.enc` 文件；DB 行保留，供本地恢复使用。

**双写适用操作**：create / update / delete / change_master_password

### 11.10 软删除与 Git 同步交互

删除语义由「本地 DB `deleted_at` 墓碑 + Git 自身的删除记录」承载，`deletions/deleted.json` 设计**已彻底移除**（不写入、不读取、不保留兼容代码）。`vault_entries.deleted_at` 列**保留不删**，schema 版本不变。

**设计理由**：Git 回滚只解决「恢复」，不解决「传播」。文件消失这一信号本身是二义的——既可能是「对方删除」，也可能是「我还没推」。因此：

- **传播**：push 时删除已删条目的 `.enc`，删除动作进入 Git 提交历史；pull 时文件集合对比得出被移除的文件，作为提示信息上报（**不自动改库**）
- **恢复**：`deleted_at = NULL` 后下次导出重建 `.enc`，Git 自动恢复文件；用户无需掌握 `git checkout`
- **追溯**：谁在何时删除由 Git commit 记录承担（`git log --diff-filter=D -- entries/`），比单一 JSON 文件更完整
- **避免冲突**：按条目分文件的 `.enc` 天然规避单一共享文件必然冲突的问题

| 事件 | DB 行为 | Git 行为 |
|------|---------|---------|
| 删除条目（本地用户操作） | `UPDATE vault_entries SET deleted_at = now() WHERE id = ?` | 下次导出时删除对应 `.enc`，`git add -A` 记录删除 |
| 拉取时发现远程删除 | **不自动改库**；仅在同步结果中上报 `removedIds`，由用户决定是否删除 | 该 `.enc` 已被 Git 移除 |
| 拉取时远程有但本地已删除 | 跳过导入该条目（本地删除优先） | `.enc` 仍存在，下次 push 时删除 |
| 恢复条目（本地用户操作） | `UPDATE vault_entries SET deleted_at = NULL` | 下次导出重建 `.enc` |
| 永久清理（本地用户操作） | `DELETE FROM vault_entries WHERE deleted_at IS NOT NULL` | 对应 `.enc` 已不存在，无额外操作 |

> 软删除条目在前端列表中默认隐藏，可通过"回收站"查看和恢复。

### 11.11 Git 冲突处理

Git 同步的冲突由 Git 本身处理。由于 .enc 文件采用整体打包加密（`encryptedBlob` 为单字段），同一条目的任何修改都会导致 blob 变化：

| 场景 | Git 行为 | 对用户的影响 |
|------|---------|-------------|
| 不同条目修改 | 自动合并（不同文件） | 无感知 |
| 同一条目，任一字段修改 | CONFLICT（blob 整体变化） | 需在 VaultConflictResolve 中选择 |
| 远程删除 + 本地修改 | CONFLICT | 需选择保留或删除 |

对于 Git 无法自动合并的冲突，前端检测 `Readyness::RepoState(Merge)` 状态，弹出冲突解决 UI（解密本地/远程两个版本，显示 diff，让用户选择）。

**冲突解决选项**：

| 选项 | 行为 |
|------|------|
| 使用本地 | `git checkout --ours <file>` → `git add` → 继续 |
| 使用远程 | `git checkout --theirs <file>` → `git add` → 继续 |
| 合并保留两者 | 保留本地版本 + 将远程版本作为新条目导入（新 UUID） |
| 跳过 | 暂不解决，标记为待处理冲突 |

### 11.12 SSH 连接复用（参考 prs `util/git.rs`）

每次 `prepare → pull` + `finalize → push` 涉及两次 SSH 握手（~4s）。prs 通过 SSH ControlMaster 持久连接将后续操作降至 ~0.1s：

- 设置 `GIT_SSH_COMMAND` 环境变量：`ssh -o 'ControlMaster auto' -o 'ControlPath /tmp/.assistant-vault--%r@%h:%p' -o 'ControlPersist 1h' -o 'ConnectTimeout 10'`
- 白名单机制：只对 `github.com` / `gitlab.com` 启用，避免不兼容服务出错
- 进程级缓存：`std::sync::LazyLock<Mutex<HashMap<PathBuf, bool>>>`，同一进程只判断一次
- Windows 上跳过（`cfg!(unix)` guard），仅 Unix 生效

### 11.13 SyncEngine

**SyncEngine struct**（`sync_engine/mod.rs`）：

| 方法 | 输入 | 输出 | 行为 |
|------|------|------|------|
| `pull(repo_path, conn, master_key)` | 路径 + 连接 + 密钥 | `Result<SyncResult>` | 记录文件快照 → `git pull` → `verify_vault_meta_integrity` → 遍历 `entries/*.enc` → 逐条 `import_entry` → 上报被移除文件 |
| `push(repo_path)` | 仓库路径 | `Result<SyncResult>` | `git push` |
| `sync_now(repo_path, conn, master_key)` | 路径 + 连接 + 密钥 | `Result<SyncResult>` | `pull` + `push` |
| `force_push(repo_path)` | 仓库路径 | `Result<SyncResult>` | `git push --force`（需用户二次确认） |

**SyncResult**：

| 字段 | 类型 | 说明 |
|------|------|------|
| `pulled` | `u32` | 拉取的条目数 |
| `pushed` | `u32` | 推送的条目数 |
| `removedIds` | `Vec<String>` | 远端已移除的条目 ID（仅上报，供用户确认，不自动改库） |
| `conflicts` | `Vec<Conflict>` | 冲突列表 |
| `readyness` | `Readyness` | 当前就绪状态 |

**SyncConfig**：

| 字段 | 类型 | 说明 |
|------|------|------|
| `remote_url` | `Option<String>` | 远程仓库 URL |
| `remote_name` | `String` | 远程名称（默认 `"origin"`） |

### 11.14 支持的 Git 远程

| 服务 | URL 格式 | 特点 |
|------|---------|------|
| **GitHub** | `https://github.com/user/vault.git` | 最流行，免费私有仓库 |
| **GitLab** | `https://gitlab.com/user/vault.git` | 免费私有仓库，可自建 |
| **Gitea / Forgejo** | `https://gitea.example.com/user/vault.git` | 轻量自建，国内友好 |
| **Gogs** | `https://gogs.example.com/user/vault.git` | 极轻量自建 |
| **Coding** | `https://e.coding.net/user/vault.git` | 国内服务，速度快 |

> SSH 协议同样支持：`git@gitea.example.com:user/vault.git`，可复用用户已有 SSH key。

### 11.15 Tauri Commands

| Command | 签名 | 说明 |
|---------|------|------|
| `vault_sync_pull` | `() → SyncResult` | 拉取远程变更 → 导入 .enc → 标记删除 |
| `vault_sync_push` | `() → SyncResult` | 推送本地变更 |
| `vault_sync_now` | `() → SyncResult` | 双向同步（pull + push） |
| `vault_sync_force_push` | `() → SyncResult` | 强制推送（需前端二次确认） |
| `vault_get_sync_config` | `() → SyncConfig` | 读取同步配置 |
| `vault_set_sync_config` | `(config: SyncConfig) → void` | 写入同步配置 |
| `vault_resolve_conflict` | `(entryId: String, resolution: ConflictResolution) → void` | 解决单条冲突 |
| `vault_git_init` | `() → void` | 初始化本地 Git 仓库 |
| `vault_git_clone` | `(url: String) → void` | 克隆远程仓库 |
| `vault_git_add_remote` | `(name: String, url: String) → void` | 添加远程仓库 |
| `vault_git_set_remote` | `(name: String, url: String) → void` | 修改远程仓库 URL |
| `vault_git_readyness` | `() → Readyness` | 查询就绪状态 |
| `vault_git_pull` | `() → void` | 手动拉取 |
| `vault_git_push` | `() → void` | 手动推送 |

**ConflictResolution 枚举**：`UseLocal` / `UseRemote` / `MergeBoth` / `Skip`

### 11.16 前端扩展

#### 11.16.1 vaultStore 新增状态

| 字段 | 类型 | 说明 |
|------|------|------|
| `syncStatus` | `"idle" \| "syncing" \| "conflict" \| "error"` | 同步状态 |
| `conflicts` | `Conflict[]` | 待解决冲突列表 |
| `syncConfig` | `SyncConfig \| null` | 同步配置 |
| `readyness` | `Readyness \| null` | Git 就绪状态 |

**新增 actions**：

| Action | 签名 | 说明 |
|--------|------|------|
| `syncNow` | `() → Promise<SyncResult>` | 双向同步 |
| `syncPull` | `() → Promise<SyncResult>` | 拉取 |
| `syncPush` | `() → Promise<SyncResult>` | 推送 |
| `resolveConflict` | `(entryId, resolution) → Promise<void>` | 解决冲突 |
| `loadSyncConfig` | `() → Promise<void>` | 加载同步配置 |
| `saveSyncConfig` | `(config) → Promise<void>` | 保存同步配置 |
| `initGitRepo` | `() → Promise<void>` | 初始化 Git 仓库 |
| `cloneGitRepo` | `(url) → Promise<void>` | 克隆远程仓库 |

#### 11.16.2 VaultSyncSettings 组件

```
┌──────────────────────────────────────────────────────┐
│  同步设置                                            │
├──────────────────────────────────────────────────────┤
│  远程仓库:  [https://github.com/user/vault.git    ]  │
│                                                      │
│  状态:  ● 就绪  |  上次同步: 2 分钟前                 │
│                                                      │
│  [初始化仓库]  [克隆仓库]  [立即同步]  [强制推送]     │
└──────────────────────────────────────────────────────┘
```

#### 11.16.3 VaultConflictResolve 组件

```
┌──────────────────────────────────────────────────────────┐
│  ⚠️ 同步冲突                                            │
│                                                          │
│  ┌────────────┐    ┌────────────┐    ┌────────────┐     │
│  │  本地版本   │    │  远程版本   │    │   建议     │     │
│  │ GitHub     │    │ GitHub     │    │ GitHub     │     │
│  │ user:old   │    │ user:new   │    │ user:new   │     │
│  │ pwd:•••    │    │ pwd:•••    │    │ pwd:•••    │     │
│  └────────────┘    └────────────┘    └────────────┘     │
│                                                          │
│  [使用本地]  [使用远程]  [合并保留两者]  [跳过]           │
└──────────────────────────────────────────────────────────┘
```

- 解密本地/远程两个版本（Rust 端完成，明文不经 IPC）
- 显示 diff（仅非敏感字段明文显示，敏感字段用 `•••` 遮蔽）
- 用户选择后调用 `vault_resolve_conflict`

#### 11.16.4 VaultList 扩展

- 底部状态栏新增同步状态：`42 条目 · 上次同步 5 分钟前 · ⚠ 1 泄露`
- 新增同步按钮（`↻`），点击调用 `syncNow()`
- `syncStatus === "conflict"` 时显示冲突提示，点击打开 VaultConflictResolve

#### 11.16.5 vault.service.ts 扩展

```typescript
syncPull: (): Promise<SyncResult> => invoke("vault_sync_pull"),
syncPush: (): Promise<SyncResult> => invoke("vault_sync_push"),
syncNow: (): Promise<SyncResult> => invoke("vault_sync_now"),
syncForcePush: (): Promise<SyncResult> => invoke("vault_sync_force_push"),
getSyncConfig: (): Promise<SyncConfig> => invoke("vault_get_sync_config"),
setSyncConfig: (config: SyncConfig): Promise<void> => invoke("vault_set_sync_config", { config }),
resolveConflict: (entryId: string, resolution: ConflictResolution): Promise<void> =>
  invoke("vault_resolve_conflict", { entryId, resolution }),
gitInit: (): Promise<void> => invoke("vault_git_init"),
gitClone: (url: string): Promise<void> => invoke("vault_git_clone", { url }),
gitAddRemote: (name: string, url: string): Promise<void> => invoke("vault_git_add_remote", { name, url }),
gitSetRemote: (name: string, url: string): Promise<void> => invoke("vault_git_set_remote", { name, url }),
gitReadyness: (): Promise<Readyness> => invoke("vault_git_readyness"),
gitPull: (): Promise<void> => invoke("vault_git_pull"),
gitPush: (): Promise<void> => invoke("vault_git_push"),
```

### 11.17 设置项扩展

AppConf 新增字段：

（同步相关设置项已移除自动同步间隔，同步采用事件驱动模式：每次密码操作自动 prepare→finalize，用户也可手动点击"立即同步"按钮。）

Rust struct 添加 `#[serde(default)]`，`AppConfDto` 同步更新。

### 11.18 i18n

`zh-CN.json` / `en-US.json` 的 `tools.password_vault` 命名空间下新增约 30 个词条，覆盖：同步状态（idle/syncing/conflict/error）、Git 操作（init/clone/pull/push/force_push/add_remote/set_remote）、冲突解决（use_local/use_remote/merge_both/skip）、同步设置、Git 远程服务名。

### 11.19 workspace Cargo.toml 变更

- members 新增 `"crates/assistant-sync"`
- workspace.dependencies 新增 `assistant-sync = { path = "crates/assistant-sync" }`

### 11.20 assistant-sync 依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| `serde` / `serde_json` | * | 类型序列化 |
| `tokio` | 1 | 异步运行时（git 子进程） |
| `base64` | 0.22 | 编解码 |
| `assistant-security` | * | vault_crypto 加解密 |
| `assistant-storage` | * | vault_entries CRUD |

### 11.21 跨阶段约束

| 维度 | 设计文档 | 本 spec | 对齐状态 |
|------|---------|---------|---------|
| Git 仓库结构 | §4.2.1 | §11.5-11.7 | ✅ 一致 |
| prepare→finalize 模式 | §4.2.2 | §11.9 | ✅ 一致 |
| 双写一致性 | §4.2.3 | §11.9 | ✅ 一致 |
| 软删除交互 | §4.2.4 | §11.10 | ⚠️ 已调整：`deleted.json` 设计彻底移除；DB `deleted_at` 保留，同步不擅自改库 |
| GitSync API | §4.2.5 | §11.3 | ✅ 一致 |
| vault_file 导出/导入 | §4.2.6 | §11.8 | ✅ 一致 |
| 冲突处理 | §4.2.7 | §11.11 | ✅ 一致 |
| SSH 连接复用 | §4.2.9 | §11.12 | ✅ 一致 |
| Tauri Commands | §5 同步/Git 部分 | §11.15 | ✅ 一致 |
| vaultStore 扩展 | §6.2 syncStatus/conflicts | §11.16.1 | ✅ 一致 |

**类型安全**：新增 SyncConfig / SyncResult / Readyness / ConflictResolution 类型，前端 TypeScript 类型与 Rust serde 序列化对齐。

**安全不变式**：

| 不变式 | 验证方式 |
|--------|---------|
| Git 仓库不泄露明文 | .enc 仅含 encryptedBlob + id/category/revision |
| .vault-meta 不含 master_key_hash | 代码审查：export 流程排除禁止字段 |
| .vault-meta 完整性保护 | 单元测试：篡改后签名验证失败；pull 时校验失败 → 不执行 import_all、不写库 |
| 未签名 meta 不被静默接受 | 单元测试：`integrity` 为空时 pull 中止并返回 `VaultMetaIntegrityFailed` |
| Git 凭证不存入仓库 | 代码审查：sync_config 仅存 vault_meta |
| 双写一致性 | 集成测试：.enc 导出失败时 DB 事务回滚 |
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

> 软删除条目在前端列表中默认隐藏，可通过"回收站"查看和恢复。回收站与永久清理的完整设计见 §11.22.4（P2B 补充）。

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

> P2B 新增的运维类命令（`vault_git_remove_remote` / `vault_git_test_connection` / `vault_sync_abort` / `vault_sync_discard` / `vault_purge_deleted` / 回收站 / 历史还原）见 §11.22.3。

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
│                                                      │
│  ── 更多操作 ─────────────────────────────────────    │
│  [测试连接]  [移除远程仓库]                           │
│  ── 危险操作 ─────────────────────────────────────    │
│  [中止同步]  [放弃本地改动]                           │
└──────────────────────────────────────────────────────┘
```

> P2B 补充：首次接入区需同时渲染 URL 输入与「初始化 / 克隆」按钮（当前实现两者互斥，见 §11.22.1）；`{ repoState }` 时显示「中止同步」，`reset --hard` 需二次确认（见 §11.22.2）。

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
| 同步运维 / 回收站 / 历史还原 | §4.2.8 恢复与追溯 | §11.22 | 🆕 P2B 补充：单条目还原、禁止整仓回滚、还原提升 revision |

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

### 11.22 同步运维与历史还原（P2B）

> 补充设计。来源：P2A 落地后对账，发现「远程配置无写入通路」「冲突态无入口」「无恢复手段」「无回收站」「无历史还原」五类缺口。
> 任务拆分见 `I9-vault-tasklist-p2.md` 阶段 L。

#### 11.22.1 远程仓库配置闭环

**问题**：`SyncConfig.remote_url` 与 Git 真实配置是两份数据，且前者不具备权威性。

| 层 | 行为 | 权威性 |
|----|------|--------|
| `SyncEngine.config`（内存 `Mutex<SyncConfig>`） | `set_config` 仅替换内存值 | ❌ 不参与 push/pull 决策 |
| Git 仓库 `remote.origin.url` | `has_remote()` / `git push` 实际读取 | ✅ 唯一权威来源 |

因此 `vault_set_sync_config` **不能**作为配置 remote 的手段。当前后果：`push` / `forcePush` 返回 `No remote configured`，`pull` 直接返回 `UpToDate` 空转（无 remote 时按「无事可做」处理）。正确流程：

| 场景 | 前置 | 操作序列 |
|------|------|---------|
| 本机已有保险库，首次启用同步 | 本地条目任意 | `git_init` → `git_add_remote` → `saveSyncConfig`（缓存）→ `sync_now` |
| 新设备，远程已有仓库 | **本地条目数 = 0** | `git_clone(url)` → `saveSyncConfig`（缓存）→ `sync_now` |
| 更换仓库 | 已有 remote | `git_remove_remote` → `git_add_remote(new)` → `saveSyncConfig` |
| 彻底断开同步 | 已有 remote | `git_remove_remote` → `saveSyncConfig`（`remote_url = null`） |

**克隆前置约束**：`GitSync::clone` 拒绝非空目标目录（`Clone target is not empty`），而本地任一条目都会在 vault 目录生成 `entries/*.enc`。因此克隆**仅适用于全新设备**；UI 需按 `entries.length > 0` 禁用并提示，或引导走灾难恢复路径（`clone_recovery_repo` 克隆到临时目录再合并）。

**首次接入区 UI**（`isNoSync` 时同时渲染，不再与 URL 输入框互斥）：

```
┌──────────────────────────────────────────────────────┐
│  远程仓库:  [https://github.com/user/vault.git    ]  │
│  [测试连接]                                          │
│                                                      │
│  [初始化仓库]  [克隆仓库]                             │
│   ⓘ 本地已有条目时不可克隆                            │
└──────────────────────────────────────────────────────┘
```

#### 11.22.2 冲突态生命周期

`Readyness` 只有 `ready` / `dirty` / `noSync` / `{ repoState }` 四支，而前端此前只处理前三支，导致冲突态「看得见状态、点不到入口」。

```
ready ──pull 冲突──▶ repoState(Merge)
                        │
          ┌─────────────┴─────────────┐
          ▼                           ▼
   resolve_conflict + commit      abort_repo_state
   （逐条解决后收尾）              （git merge --abort）
          │                           │
          └──────────▶ ready ◀────────┘
```

| 操作 | 前置 | git 行为 | 命令 |
|------|------|---------|------|
| 打开冲突面板 | `repoState = merge/rebase/cherryPick` | 无 | 前端读取 `detect_conflicts` |
| 解决单条 | 同上 | `checkout --ours/--theirs` + `add` | `vault_resolve_conflict(path, resolution)` |
| 冲突收尾 | 全部冲突已解决 | `commit`（或交由 `sync_now` 的 `finalize`） | `vault_sync(action: "sync")` |
| 中止同步 | 同上 | `merge --abort` / `rebase --abort` / `cherry-pick --abort` | `vault_sync_abort` |
| 放弃本地改动 | 任意 | `add -A` + `reset --hard` | `vault_sync_discard`（二次确认） |

#### 11.22.3 新增 Tauri Commands（运维类）

| Command | 签名 | 说明 |
|---------|------|------|
| `vault_git_remove_remote` | `(remote: String) → void` | 移除远程仓库（解绑 / 迁移） |
| `vault_git_test_connection` | `(url?: String) → ConnectionTest` | `git ls-remote`（`GIT_TERMINAL_PROMPT=0`），返回 `ok` / `stderr` 摘要 |
| `vault_sync_abort` | `() → void` | 中止 merge / rebase / cherry-pick |
| `vault_sync_discard` | `() → void` | 放弃本地改动（`reset --hard`） |
| `vault_purge_deleted` | `() → u32` | 永久清理软删除条目 |
| `vault_list_deleted` | `() → VaultEntrySummary[]` | 回收站列表 |
| `vault_restore_deleted` | `(id: String) → void` | 从回收站恢复 |
| `vault_history_list` | `(entryId?: String, limit?: u32) → HistoryCommit[]` | 历史版本列表 |
| `vault_history_preview` | `(commit: String, entryId: String) → EntryPreview` | 历史版本预览（敏感字段遮蔽） |
| `vault_history_restore` | `(commit: String, entryId: String) → void` | 单条目还原 |

**HistoryCommit**：`hash` / `timestamp` / `message` / `entryId`（可为 `null`，表示跨条目提交）

**EntryPreview**：`title` / `category` / `fields: { key, value, masked }[]` / `revision` / `updatedAt`

#### 11.22.4 回收站与永久清理

软删除（`deleted_at` 墓碑）与永久清理（`DELETE`）必须分开，因为两者在 Git 侧后果不同：

| 操作 | DB | Git | 可恢复性 |
|------|----|-----|---------|
| 软删除 | `deleted_at = now()` | 下次导出移除 `.enc`，删除进入提交历史 | ✅ 回收站恢复 / Git 历史还原 |
| 恢复 | `deleted_at = NULL`，`revision + 1` | 下次导出重建 `.enc` | — |
| 永久清理 | `DELETE FROM vault_entries WHERE deleted_at IS NOT NULL` | 无额外操作（`.enc` 已不存在） | ❌ 墓碑消失后无法再定位历史版本 |

**约束**：永久清理必须二次确认，且文案需明确「清理后无法再从 Git 历史还原」。同时存在「历史还原」能力时，purge 是唯一不可逆操作，需置于 danger zone。

#### 11.22.5 历史浏览与单条目还原

**能力现状**：`git.rs` 仅有 `init/clone/add_all/commit/reset_hard/status/push/pull/fetch/remote*/checkout_ours|theirs/ref_hash/unmerged_paths/conflict_stage_hashes/last_pull_time`，**无任何历史读取函数**，亦无 `abort` / `ls-remote` 封装。`git log` / `git show` / `git cat-file` 需先补齐。`GitSync` 层同样缺 `remove_remote` / `test_connection` / `abort_repo_state`，需包一层（底层 `git_remote_remove` 已存在）。

**为什么按条目而非整仓**：

- `.enc` 按条目分文件、逐条目独立密钥（`derive_entry_key(master_key, entry_id)`），单条目还原在密码学上天然可行
- 整仓 `reset --hard <commit>` 会绕过 revision 语义、把所有条目一起倒退，且误操作代价极高
- 删除操作的传播已由 Git 提交历史承担（`git log --diff-filter=D -- entries/`），定位能力已具备

**还原数据流**（严格对齐双写模式 §11.9）：

```
history_restore(commit, entry_id)
  ├─ 0. 校验 readyness ≠ repoState            → 否则拒绝（中间态会污染冲突流程）
  ├─ 1. git show <commit>:entries/<id>.enc    → 取旧 EntryFile
  ├─ 2. 当前 master_key 解密 → EntryBlob
  │      └─ 失败 → 返回「该版本由旧主密码加密，无法还原」，不写任何数据
  ├─ 3. 事务内 restore_entry_tx
  │      ├─ revision   = 本地 revision + 1     ← 关键
  │      ├─ updated_at = now
  │      ├─ deleted_at = NULL
  │      └─ export_entry（用当前密钥重新加密落盘）
  ├─ 4. finalize("vault: restore {id}")        → commit + push
  └─ 5. 审计日志（记录来源 commit）
```

**两条硬约束**：

1. **禁止直接复制历史密文字节**。`upsert_imported_in` 的 revision 门禁是：

   ```rust
   if entry.revision < revision as u64
       || (entry.revision == revision as u64 && entry.updated_at <= updated_at)
   {
       return Ok(ImportOutcome::SkippedStale);
   }
   ```

   历史版本必然 revision 更低 → 走导入通道会被静默跳过；紧接着 `export_all_entries` 用 DB 当前版本覆盖工作区，还原被抹掉，而 `SyncResult` 仍返回 `Success`。因此还原必须走独立事务并提升 revision，其他设备看到 `N+1` 才会正确收敛到旧内容。

2. **墓碑会挡住还原**。`Some((Some(_), _, _)) => Ok(ImportOutcome::SkippedLocallyDeleted)` 意味着本地 `deleted_at` 非空时任何导入都被跳过。还原已删除条目必须同事务完成「清墓碑 + 写内容 + 提升 revision」，不能依赖导入通道。

**旧主密码边界**：改主密码（§11 全量重加密）后，旧 commit 的 `.enc` 由旧主密钥派生，当前 `master_key` 必然解不开。这是设计使然而非缺陷；UI 需给出明确文案，且**不触碰 `.vault-meta`**（旧 meta 的 HMAC 同样是旧 key 签名）。

**预览接口复用**：`history_preview` 与冲突 diff（H15.2）本质相同——「取某个版本的 `EntryFile` → 解密 → 遮蔽敏感字段」。统一实现一份 `decrypt_entry_preview`，两处共用，避免重复造解密路径。

#### 11.22.6 提交信息隐私

现状 `finalize_sync(&engine, &format!("vault: create {}", snapshot.title))` 与 `vault: update {title}` 把**条目标题以明文写入 Git 提交信息**（`vault: delete {id}` 已是 id）。托管平台与任何有读权限者都能看到，历史面板还会成列展示。

修正为只写 id：`vault: create {id}` / `vault: update {id}` / `vault: delete {id}`（不变）/ `vault: restore {id}`。追溯能力不受影响（id 稳定且与 `.enc` 文件名一致）。

#### 11.22.7 P2B 约束与验证

| 约束 | 说明 |
|------|------|
| 权威来源唯一 | remote 配置以 Git 配置为准，`SyncConfig` 仅为内存缓存 |
| 克隆前置 | 目标目录必须为空，本地有条目时禁用克隆 |
| 冲突可退出 | 任何 `repoState` 都必须有 `abort` 路径，不允许「只能命令行解决」 |
| 还原提升 revision | 单条目还原 `revision + 1`，禁止复制历史密文 |
| 还原拒绝中间态 | `repoState` 时拒绝还原 |
| purge 不可逆 | 二次确认 + 明确「无法从历史还原」 |
| 提交信息不含标题 | 仅 id / 操作类型 |

**验证要点**：

- 远程配置：新设备克隆后可同步；本地有条目时克隆被拦截
- 冲突：双端改同一条目 → 提示条出现 → 解决后脱离 Merge 态
- 恢复：`abort` 可退出 Merge；`discard` 有二次确认
- 回收站：删除 → 恢复 → `.enc` 重建 → 另一设备可见
- 历史还原：还原后 revision 递增，其他设备收敛（不触发 `SkippedStale`）
- 历史还原边界：旧主密码 → 明确错误且不写库；`repoState` → 拒绝
- 隐私：`git log` 抽查无标题明文

#### 11.22.8 UI 参考（P2B 界面优化）

P2B 的界面改动集中在「首次接入 / 冲突 / 历史 / 回收站 / 状态提示」五处。参考项目目录中已有可对照的实现，**无需新增仓库**。

参考分两层：**A. Git 界面首选**（同栈 React/TS，可直接搬布局）；**B. 密码库语义**（哪些信息该展示、哪些操作不该给）。

**A. Git 界面首选 —— `references/1-ai-agent/OpenBitFun`（React + TS，与本项目同栈）**

| 界面 | 参考文件 | 可借鉴要点 |
|------|---------|-----------|
| 首次接入空态（L1.1） | `src/web-ui/src/app/scenes/git/GitScene.tsx`（`!isRepository` 分支） | 居中卡片：图标 + 标题 + 一句原因 + 单个主按钮 + 底部灰色 hint；装饰性连线表达「尚未建仓」。比纯按钮更清楚地说明「为什么现在是这个状态」 |
| Git 场景信息架构 | `src/web-ui/src/app/scenes/git/GitNav.tsx` | 左栏 = 标题 + 仓库状态（分支 / ahead / behind）+ 导航项（变更 / 分支 / 图）；状态与导航同处一栏，不必另开面板 |
| 设置页布局（L1 / L3） | `src/web-ui/src/tools/git/components/GitSettingsView/GitSettingsView.tsx` | 三 Tab（用户 / 仓库 / 高级）+ `section` 分块 + 顶部可关闭 status banner（error/success 各一条），保存按钮常驻 header |
| 提交历史列表（L5.6） | `src/web-ui/src/tools/git/components/GitBranchHistoryView/GitBranchHistoryView.tsx` | 相对时间函数、作者下拉筛选、关键字搜索（消息/作者/hash）、多选集合、行内展开详情、点击复制 hash |
| 分支图 / 提交图（L5） | `src/web-ui/src/tools/git/components/GitGraphView/GitGraphView.tsx` | 图形化提交关系 + 搜索命中定位（上一个/下一个） |
| 差异视图（L5 预览） | `src/web-ui/src/tools/git/components/GitDiffView/`、`GitDiffEditor/` | 折叠/展开全部；空态区分「工作区无变更」「两提交间无差异」；接受/拒绝全部前有明确二次确认文案 |
| 检出受阻对话框（L2 参考） | `src/web-ui/src/tools/git/components/BranchQuickSwitch.tsx` | 冲突态不弹裸错误：列出被覆盖文件 + 给出「提交并切换」这一条出路，失败后可重试 |
| i18n 词条库（L7） | `src/web-ui/src/locales/zh-CN/panels/git.json` | 现成的中文文案：`relativeTime.*`、`confirm.*`（含「此操作不可恢复」句式）、`notifications.*`、`status.*`、`trust.*` |

> **不可照抄**：`GitSettingsView` 在 `config.remote` 为空时**整段不渲染**（没有「添加远程」入口），与本项目 L1.1 同病；`GitService` 也没有 `clone` / `addRemote` / `testConnection`。L1 的「首次接入三分支」仍需自行设计。
> **明确不采纳**：OpenBitFun 提供 `resetToCommit`（soft/mixed/hard），本项目按硬约束 1 禁止整仓回滚，不复用该入口。

**B. 密码库语义 —— `references/3-vault-sync/keepassxc`、`references/4-vault-security/ripasso`**

| 界面 | 参考文件 | 可借鉴要点 |
|------|---------|-----------|
| 历史版本面板（L5.6） | `keepassxc/src/gui/entry/EditEntryWidgetHistory.ui` | `QTreeView` 4 列：绝对时间 / 相对时间（`Current (5 分钟前)`）/ 修改说明 / 操作者；底部 `[Show] [Restore] [Delete] [Delete all]`，选中前全部 disabled |
| 历史列表列语义 | `keepassxc/src/gui/entry/EntryHistoryModel.cpp` | 相对时间用 `humanReadableTimeDifference`；当前版本行标记为 `Current`，不与历史混排 |
| 冲突解决（L2 / H15） | `keepassxc/src/gui/MergeDialog.ui` | 以表格逐行列出变更（而非整屏 diff），底部确认按钮；适合「多条冲突一次过」 |
| 首次接入语义（L1） | `keepassxc/src/gui/DatabaseOpenWidget.ui`、`MainWindow.ui`（新建/打开/合并/导入四动作） | 「新建 / 打开 / 导入」并列呈现而非互斥隐藏，正是 L1.1 的方向；`actionDatabaseMerge` 与 `actionDatabaseImport` 分离，对应「克隆」与「导入」应分开 |
| 状态提示条（H14.2） | `keepassxc/src/gui/MessageWidget.*`、`KMessageWidget.*` | 顶部可关闭的 inline 提示条，按 info/warning/error 分级，替代整块 `Alert` |
| 维护 / 危险操作区（L4） | `keepassxc/src/gui/dbsettings/DatabaseSettingsWidgetMaintenance.ui` | 「维护」独立分组，破坏性操作与常规设置视觉分离 |
| 回收站语义（L4） | `keepassxc/src/core/Group.cpp`、`Metadata.*`（`RecycleBin`） | 回收站是「特殊分组」而非独立表：删除即移入、恢复即移出，与本项目 `deleted_at` 语义可对齐 |
| 条目版本信息 | `ripasso/crates/ripasso/src/git.rs`（`read_git_meta_data`）、`ripasso-gtk/src/window/mod.rs` | 用 `blame` 取「最后改动该文件的提交」→ 展示 `committed-by` 与 `updated` 标签，成本低且无需遍历全部历史 |

> `prs` 是最贴近的 Git 同步实现，但为纯 CLI（`cli/src/action/sync/*`），只参考命令语义与 `Readyness` 状态机，不含界面。
> 其余参考组与本任务无关：`vaultwarden` 为服务端、`rust-keepass` 无历史概念、`keyring-*` / `watchkey-win` 为系统凭据层、`2-desktop-orb/*` 与 `5-design-style/*` 无 Git 界面。
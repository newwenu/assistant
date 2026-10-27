# I9 — 密码管理箱（Password Vault）P2 任务清单

> 规格：[I9-vault-spec-4-git-sync.md](./I9-vault-spec-4-git-sync.md) + [I9-vault-spec-5-ssh-and-recovery.md](./I9-vault-spec-5-ssh-and-recovery.md)
> 设计：[VAULT-DESIGN.md](../VAULT-DESIGN.md)
> 前置：P1-A ~ P1-F 全部完成（阶段 A ~ G ✅ + F' 生物识别平台真实实现 ✅）

---

## 阶段 H — P2A：assistant-sync crate + Git 同步引擎

### H1. 新建 assistant-sync crate

**文件**：`src-tauri/crates/assistant-sync/`

- [x] H1.1 创建 `Cargo.toml`：package 元数据 + 依赖（`serde` / `serde_json` / `tokio` / `base64` / `assistant-security` / `assistant-storage`）
- [x] H1.2 创建 `src/lib.rs`：模块声明（git_sync / sync_engine / vault_file / types）
- [x] H1.3 workspace `Cargo.toml` members 新增 `"crates/assistant-sync"`，workspace.dependencies 新增 `assistant-sync = { path = "crates/assistant-sync" }`

**依赖**：G4

---

### H2. 同步类型定义

**文件**：`assistant-sync/src/types/`

- [x] H2.1 `mod.rs`：`Readyness` 枚举（NoSync / RepoState(RepositoryState) / Dirty / Ready）+ `RepositoryState` 枚举（Merge / Rebase / RebaseInteractive / CherryPick / Unknown）+ `SyncResult` struct（pulled / pushed / conflicts / readyness / imported / removedIds / integrityWarning）+ `SyncConfig` struct（remote_url / remote_name）+ `Conflict` struct（entry_id / local_revision / remote_revision）+ `ConflictResolution` 枚举（UseLocal / UseRemote / MergeBoth / Skip）+ `IntegrityResult` struct（valid / message）
- [x] H2.2 `SyncConfig` 实现 `Serialize`/`Deserialize`，`remote_url` 为 `Option<String>`（未配置时为 None）
- [ ] H2.3 `Readyness` / `RepositoryState` 实现 `Display` trait（错误消息友好显示）

**依赖**：H1

---

### H3. git CLI 封装

**文件**：`assistant-sync/src/git_sync/git.rs`

- [x] H3.1 `git(args, repo_path) → Result<Output>`：`Command::new("git")` + `.args(args)` + `.current_dir(repo_path)` + `.env("GIT_TERMINAL_PROMPT", "0")` + 检查 `output.status.success()` + 失败时返回 `format!("git {} failed: {}", args, stderr)`
- [x] H3.2 `git_with_ssh(args, repo_path, ssh_env) → Result<Output>`：额外注入 `GIT_SSH_COMMAND` 环境变量（SSH ControlMaster，Unix only）

**依赖**：H2

---

### H4. GitSync 核心实现

**文件**：`assistant-sync/src/git_sync/mod.rs`

- [x] H4.1 `GitSync::init(repo_path)` — 若 `.git` 已存在则跳过；`git init` + 空提交（`git commit --allow-empty -m "vault init"`）
- [x] H4.2 `GitSync::clone(url, repo_path)` — `git clone url repo_path`，检查退出码
- [x] H4.3 `GitSync::prepare(repo_path)` — 若有 remote → `git pull -q`（SSH 连接复用 §H9）
- [x] H4.4 `GitSync::finalize(repo_path, msg)` — 若 dirty → `git add .` + `git commit -q -m msg`；若有 remote 且 `need_to_push()` → `git push -q`
- [x] H4.5 `GitSync::readyness(repo_path)` — 解析 `git status --porcelain=v2` + `git rev-parse --git-dir` + 检测 merge/rebase/cherry-pick 状态 → 返回 Readyness
- [x] H4.6 `GitSync::need_to_push(repo_path)` — 智能推送优化：30s 内 pull 过且本地/远程 hash 相同 → 跳过 push；elapsed 不可得时保守返回 true
- [x] H4.7 `GitSync::add_remote(repo_path, name, url)` — `git remote add name url`
- [x] H4.8 `GitSync::set_remote(repo_path, name, url)` — `git remote set-url name url`
- [x] H4.9 `GitSync::pull(repo_path)` — `git pull -q`
- [x] H4.10 `GitSync::push(repo_path)` — `git push -q`

**依赖**：H3

---

### H5. vault_file 导出/导入

**文件**：`assistant-sync/src/vault_file/`

- [ ] H5.1 `types.rs`：`EncFile` struct（id / category / revision / encrypted_blob）+ `VaultBlob` struct（title / username / url / password / notes / custom_fields / totp_secret / totp_digits / totp_period / tags / is_favorite / created_at / updated_at）+ `VaultMetaFile` struct（version / exported_at / crypto / emergency / integrity）
- [ ] H5.2 `mod.rs`：`export_entry(conn, entry_id, master_key, repo_path) → Result<()>` — SQLite 逐字段读取 → 构造 VaultBlob → 整体打包加密 → 写入 `{id}.enc`
- [ ] H5.3 `mod.rs`：`import_entry(enc_path, master_key, conn) → Result<()>` — 读取 .enc → 解密 VaultBlob → 逐字段写入 SQLite
- [ ] H5.4 `mod.rs`：`export_all(conn, master_key, repo_path) → Result<u32>` — 遍历所有未删除条目 → 逐条 export_entry → 随后删除 `deleted_at IS NOT NULL` 条目对应的 `.enc`（让 Git 提交记录承载删除）→ 返回导出数量
- [ ] H5.5 `mod.rs`：`import_all(repo_path, master_key, conn) → Result<(u32, u32)>` — 记录 pull 前 `entries/*.enc` 文件快照 → 遍历当前文件逐条 import_entry（新增或按 revision/updatedAt 覆盖）→ 文件集合对比得出被移除项，仅作为 `removedIds` 上报 → 返回 (导入数, 上报移除数)；**不读写 `deleted_at`，不擅自改库**；需含防误删护栏（空集合中止 / 冲突状态跳过导入）
- [ ] H5.6 `mod.rs`：`export_vault_meta(conn, master_key, repo_path) → Result<()>` — 从 vault_meta 读取非秘密字段 → 构造 VaultMetaFile → 计算 HMAC 签名 → 写入 `.vault-meta`；首次初始化必须走此函数，**禁止写入 `integrity: ""` 的空壳 meta**（会导致所有设备 pull 时判定未签名而中止）
- [ ] H5.7 `mod.rs`：`verify_vault_meta_integrity(master_key, repo_path) → Result<IntegrityResult>` — 读取 `.vault-meta` → 验证 HMAC 签名；**唯一调用点为同步 pull（H6.1）**，恢复流程只消费结果、不另行定义策略；`integrity` 为空/文件缺失视为未签名（`valid = false`）

**依赖**：H4

---

### H6. SyncEngine 实现

**文件**：`assistant-sync/src/sync_engine/mod.rs`

- [ ] H6.1 `SyncEngine::pull(repo_path, conn, master_key) → Result<SyncResult>` — `git pull` → **`verify_vault_meta_integrity(master_key)`（同步职责；失败则中止：不 `import_all`、不写库、不 finalize、不 push，返回 `VaultError::VaultMetaIntegrityFailed`）** → `import_all` → 返回 SyncResult
- [x] H6.2 `SyncEngine::push(repo_path) → Result<SyncResult>` — `git push` → 返回 SyncResult
- [ ] H6.3 `SyncEngine::sync_now(repo_path, conn, master_key) → Result<SyncResult>` — `pull` + `push`；pull 阶段完整性校验失败则不进入 push
- [x] H6.4 `SyncEngine::force_push(repo_path) → Result<SyncResult>` — `git push --force`

**依赖**：H5

---

### H7. DB 与文件双写集成

**文件**：`src-tauri/src/cmd/vault.rs`

- [ ] H7.1 修改 `vault_create_entry`：在现有 CRUD 逻辑外层包裹 `sync.prepare()` + DB 事务（含 export_entry）+ `sync.finalize("vault: added {title}")`
- [ ] H7.2 修改 `vault_update_entry`：同上模式
- [ ] H7.3 修改 `vault_delete_entry`：DB 软删除（`deleted_at = now()`，该列**保留不删**）+ 删除对应 `.enc` 文件 + `sync.finalize("vault: deleted {id}")`
- [ ] H7.4 修改 `vault_change_master_password`：全量重加密后 `export_all` + `sync.finalize("vault: master password changed")`
- [ ] H7.5 双写一致性保证：export_entry 失败时 DB 事务回滚（步骤 2-5 在同一事务中）

**依赖**：H6

---

### H8. 软删除与 Git 同步交互

**文件**：`assistant-sync/src/vault_file/mod.rs`

> 前提：DB `vault_entries.deleted_at` 列**保留不删**，schema 不做迁移（`SCHEMA_VERSION` 保持 2）。删除状态由 DB 墓碑 + Git 删除记录承载，`deletions/deleted.json` 设计**已彻底移除**（不写入、不读取、不保留兼容代码）。同步流程**不擅自修改本地数据库**。

- [ ] H8.1 ~~`read_deleted_index(repo_path)`~~ — **不实现**，`deletions/deleted.json` 读写能力已从代码中完全删除
- [ ] H8.2 ~~`collect_legacy_deletions(repo_path)`~~ — **不实现**，不做旧仓库兼容解析
- [ ] H8.3 `purge_deleted(conn, repo_path) → Result<u32>` — 永久清理（仅用户手动触发）：`DELETE FROM vault_entries WHERE deleted_at IS NOT NULL` + 确保对应 `.enc` 已不存在
- [ ] H8.4 ~~`update_deleted_json`~~ — **不实现**，删除传播改由 §H5.4 / §H5.5 的文件集合对比完成

**依赖**：H5

---

### H9. SSH 连接复用

**文件**：`assistant-sync/src/git_sync/ssh.rs`

- [x] H9.1 `build_ssh_command() → String`：构造 `GIT_SSH_COMMAND` 值（`ssh -o 'ControlMaster auto' -o 'ControlPath /tmp/.assistant-vault--%r@%h:%p' -o 'ControlPersist 1h' -o 'ConnectTimeout 10'`）
- [x] H9.2 `should_use_control_master(url) → bool`：白名单判断（`github.com` / `gitlab.com`），进程级缓存（`LazyLock<Mutex<HashMap<PathBuf, bool>>>`）
- [x] H9.3 `#[cfg(unix)]` 条件编译：Windows 上跳过，仅 Unix 生效

**依赖**：H3

---

### H10. Git 冲突处理

**文件**：`assistant-sync/src/sync_engine/mod.rs` + `src-tauri/src/cmd/vault.rs`

- [ ] H10.1 `detect_conflicts(repo_path) → Result<Vec<Conflict>>`：检测 `Readyness::RepoState(Merge)` → `git diff --name-only --diff-filter=U` 获取冲突文件列表
- [ ] H10.2 `resolve_conflict(repo_path, entry_id, resolution, conn, master_key) → Result<()>`：根据 ConflictResolution 执行对应 git 操作（UseLocal: `git checkout --ours` / UseRemote: `git checkout --theirs` / MergeBoth: 保留本地 + 远程作为新条目 / Skip: 标记待处理）
- [ ] H10.3 `vault_resolve_conflict(entry_id, resolution)` Tauri command

**依赖**：H6

---

### H11. 同步 Tauri Commands

**文件**：`src-tauri/src/cmd/vault.rs`

- [ ] H11.1 `vault_sync_pull() → SyncResult` — 调用 SyncEngine::pull
- [ ] H11.2 `vault_sync_push() → SyncResult` — 调用 SyncEngine::push
- [ ] H11.3 `vault_sync_now() → SyncResult` — 调用 SyncEngine::sync_now
- [ ] H11.4 `vault_sync_force_push() → SyncResult` — 需前端二次确认
- [ ] H11.5 `vault_get_sync_config() → SyncConfig` — 从 vault_meta 读取
- [ ] H11.6 `vault_set_sync_config(config) → void` — 写入 vault_meta
- [ ] H11.7 `vault_git_init() → void` — 调用 GitSync::init
- [ ] H11.8 `vault_git_clone(url) → void` — 调用 GitSync::clone
- [ ] H11.9 `vault_git_add_remote(name, url) → void` — 调用 GitSync::add_remote
- [ ] H11.10 `vault_git_set_remote(name, url) → void` — 调用 GitSync::set_remote
- [x] H11.11 `vault_git_readyness() → Readyness` — 调用 GitSync::readyness
- [ ] H11.12 `vault_git_pull() → void` — 调用 GitSync::pull
- [ ] H11.13 `vault_git_push() → void` — 调用 GitSync::push

**依赖**：H7, H10

---

### H12. invoke_handler 注册

**文件**：`src-tauri/src/lib.rs` + `src-tauri/src/setup.rs`

- [ ] H12.1 `setup.rs` 新增 `GitSync` + `SyncEngine` State 管理
- [x] H12.2 `lib.rs` invoke_handler 新增所有 `cmd::vault_sync_*` + `cmd::vault_git_*` 条目

**依赖**：H11

---

### H13. 前端：vaultStore 同步扩展

**文件**：`src/tools/password-vault/vaultStore.ts`

- [ ] H13.1 新增状态字段：`syncStatus: "idle" | "syncing" | "conflict" | "error"` / `conflicts: Conflict[]` / `syncConfig: SyncConfig | null` / `readyness: Readyness | null`
- [x] H13.2 新增 actions：`syncNow` / `syncPull` / `syncPush` / `resolveConflict` / `loadSyncConfig` / `saveSyncConfig` / `initGitRepo` / `cloneGitRepo`
- [x] ~~H13.3 自动同步计时器~~：已移除定时轮询，改为事件驱动（每次密码操作自动 prepare→finalize）+ 手动"立即同步"按钮

**依赖**：H12

---

### H14. 前端：VaultSyncSettings 组件

**文件**：`src/tools/password-vault/VaultSyncSettings.tsx`

- [x] H14.1 远程仓库 URL 输入（已移除自动同步间隔选择）
- [ ] H14.2 状态指示：就绪/同步中/冲突/错误 + 上次同步时间
- [ ] H14.3 操作按钮：初始化仓库 / 克隆仓库 / 立即同步 / 强制推送
- [x] H14.4 `VaultSettings.tsx` 集成：新增"同步"标签页

**依赖**：H13

---

### H15. 前端：VaultConflictResolve 组件

**文件**：`src/tools/password-vault/VaultConflictResolve.tsx`

- [ ] H15.1 冲突列表显示：条目标题 + 本地/远程修改时间
- [ ] H15.2 解密 diff 显示（Rust 端解密，仅非敏感字段明文，敏感字段 `•••` 遮蔽）
- [ ] H15.3 解决选项按钮：使用本地 / 使用远程 / 合并保留两者 / 跳过
- [ ] H15.4 VaultList 集成：`syncStatus === "conflict"` 时显示冲突提示 + 打开 VaultConflictResolve

**依赖**：H13

---

### H16. 前端：VaultList 同步状态栏

**文件**：`src/tools/password-vault/VaultList.tsx`

- [ ] H16.1 底部状态栏新增同步状态：条目数 · 上次同步时间 · 泄露数
- [x] H16.2 新增同步按钮（`↻`），点击调用 `syncNow()`
- [x] H16.3 `syncStatus === "syncing"` 时按钮旋转动画

**依赖**：H13

---

### H17. 前端：vault.service.ts 同步扩展

**文件**：`src/tools/password-vault/vault.service.ts`

- [x] H17.1 新增 invoke 封装：`syncPull` / `syncPush` / `syncNow` / `syncForcePush` / `getSyncConfig` / `setSyncConfig` / `resolveConflict` / `gitInit` / `gitClone` / `gitAddRemote` / `gitSetRemote` / `gitReadyness` / `gitPull` / `gitPush`

**依赖**：H12

---

### H18. 设置项 + i18n

**文件**：`src-tauri/crates/assistant-config/src/app_conf.rs` + `src/tools/password-vault/vault.service.ts` + `src-tauri/crates/assistant-i18n/locales/`

- [x] ~~H18.1 Rust `AppConf` struct 新增 `vault_auto_sync_interval_minutes: u32`~~：已移除，不再需要自动同步间隔字段
- [x] ~~H18.2 前端 `AppConf` interface 新增 `vaultAutoSyncIntervalMinutes: number`~~：已移除
- [x] H18.3 `AppConfDto` + `From<AppConf>` impl 同步更新
- [ ] H18.4 `zh-CN.json` + `en-US.json` 新增同步相关约 30 词条

**依赖**：H12

---

### H19. 验证

- [ ] H19.1 `cargo check` 通过
- [ ] H19.2 `cargo test -p assistant-sync` 通过
- [ ] H19.3 `tsc --noEmit` 通过
- [ ] H19.4 GitSync::init → GitSync::readyness 返回 Ready
- [ ] H19.5 GitSync::clone 从远程仓库克隆成功
- [ ] H19.6 prepare → 本地修改 → finalize：变更已 commit + push
- [ ] H19.7 need_to_push 智能优化：30s 内 pull 过且 hash 相同 → 返回 false
- [ ] H19.8 vault_file export → import 往返一致（逐字段加密 → 整体打包 → 解密 → 逐字段还原）
- [ ] H19.9 双写一致性：export_entry 失败时 DB 事务回滚
- [ ] H19.10 软删除：本地删除 → DB `deleted_at` 标记 + `.enc` 移除；pull 时被移除文件仅上报 `removedIds`，不自动改库
- [ ] H19.11 冲突检测：Readyness::RepoState(Merge) 正确识别
- [ ] H19.12 冲突解决：UseLocal / UseRemote / MergeBoth 各选项行为正确
- [ ] H19.13 .vault-meta HMAC 签名验证：篡改后验证失败
- [ ] H19.14 .vault-meta 不含 master_key_hash（代码审查）
- [ ] H19.15 SSH ControlMaster 复用（Unix only）：连续 push/pull 仅一次握手
- [ ] H19.16 `GIT_TERMINAL_PROMPT=0` 生效：认证失败返回错误而非挂起
- [ ] H19.17 前端同步状态正确显示（idle/syncing/conflict/error）
- [x] ~~H19.18 自动同步计时器生效（间隔可配置）~~：已移除定时轮询机制
- [ ] H19.19 设置项四处同步：Rust AppConf + 前端 AppConf + AppConfDto + From<AppConf>
- [ ] H19.20 pull 完整性闸门：签名篡改 / `integrity` 为空时 `pull` 中止，且未执行任何 DB 写入

**依赖**：H14, H15, H16, H17, H18

---

## 阶段 I — P2A'：SSH Agent 集成

### I1. SshAgentBridge trait + 平台实现

**文件**：`assistant-security/src/ssh_agent/`

- [x] I1.1 `mod.rs`：`SshAgentBridge` trait（add_key / remove_key / list_keys / is_agent_running）+ `SshKeyInfo` struct（fingerprint / key_type / comment / bits）+ 工厂函数 `create_agent_bridge() → Box<dyn SshAgentBridge>`
- [x] I1.2 `unix.rs`：Unix socket 通信（`SSH_AUTH_SOCK`）+ `ssh-add` 命令封装
- [x] I1.3 `windows.rs`：Pageant 通信（命名管道兼容层）+ `ssh-add` 命令封装（Git for Windows 自带）
- [x] I1.4 `Cargo.toml` 条件编译依赖（无额外 crate，使用 `std::process::Command` 调用 `ssh-add`）

**依赖**：H19

---

### I2. SSH Agent 生命周期管理

**文件**：`src-tauri/src/cmd/vault.rs` + `src-tauri/src/setup.rs`

- [ ] I2.1 `SshAgentState` struct：`added_fingerprints: Mutex<HashSet<String>>`（追踪由 vault 添加的密钥）+ `bridge: Box<dyn SshAgentBridge>`
- [x] I2.2 vault 解锁时：若 `vaultSshAutoAdd = true` → 遍历 `category = "ssh"` 条目 → 逐条 add_to_agent → 追踪 fingerprint
- [x] I2.3 vault 锁定时：遍历 `added_fingerprints` → 逐条 remove_from_agent → 清空 set
- [ ] I2.4 添加 SSH 条目时：若 vault 已解锁且 Agent 运行 → 自动添加
- [ ] I2.5 删除 SSH 条目时：从 Agent 移除对应密钥

**依赖**：I1

---

### I3. SSH 密钥格式校验

**文件**：`assistant-security/src/ssh_agent/validate.rs`

- [x] I3.1 `validate_private_key(key_data: &[u8]) → Result<SshKeyMeta>` — 校验格式（OpenSSH / PEM / PuTTY .ppk）+ 提取元数据（key_type / bits / fingerprint / comment）
- [x] I3.2 `SshKeyMeta` struct：key_type / bits / fingerprint / comment / public_key
- [x] I3.3 `VaultError` 新增 `InvalidKeyFormat(String)` 变体

**依赖**：I1

---

### I4. SSH Agent Tauri Commands

**文件**：`src-tauri/src/cmd/vault.rs`

- [x] I4.1 `vault_ssh_add_to_agent(entry_id) → void` — 解密 SSH 私钥 → validate → add_to_agent → zeroize 明文 → 追踪 fingerprint
- [x] I4.2 `vault_ssh_remove_from_agent(entry_id) → void` — 从 Agent 移除 + 从 added_fingerprints 移除
- [x] I4.3 `vault_ssh_list_agent() → Vec<SshKeyInfo>` — 列出 Agent 中所有密钥（含 vault 添加标记）
- [x] I4.4 `vault_ssh_auto_add_all() → u32` — 自动添加所有 SSH 条目

**依赖**：I2, I3

---

### I5. 前端：SSH Agent 集成

**文件**：`src/tools/password-vault/`

- [ ] I5.1 vaultStore 新增：`sshAgentKeys: SshKeyInfo[]` / `sshAutoAddEnabled: boolean` + actions：`addSshToAgent` / `removeSshFromAgent` / `loadSshAgentKeys`
- [ ] I5.2 VaultEntryDetail 扩展：`category === "ssh"` 时显示密钥类型 + 位数 + 指纹 + 公钥 + [添加到 Agent] / [从 Agent 移除] 按钮
- [ ] I5.3 VaultSettings 扩展：SSH Agent 设置区（自动添加开关 + Agent 状态 + 密钥列表）
- [x] I5.4 vault.service.ts 新增：`sshAddToAgent` / `sshRemoveFromAgent` / `sshListAgent` / `sshAutoAddAll`

**依赖**：I4

---

### I6. 设置项 + i18n

**文件**：`src-tauri/crates/assistant-config/src/app_conf.rs` + `src-tauri/crates/assistant-i18n/locales/`

- [x] I6.1 Rust `AppConf` struct 新增 `vault_ssh_auto_add: bool` + `#[serde(default)]`
- [x] I6.2 前端 `AppConf` interface 新增 `vaultSshAutoAdd: boolean`
- [x] I6.3 `AppConfDto` + `From<AppConf>` impl 同步更新
- [ ] I6.4 `zh-CN.json` + `en-US.json` 新增 SSH Agent 相关约 15 词条

**依赖**：I4

---

### I7. 验证

- [ ] I7.1 `cargo check` 通过
- [ ] I7.2 `ssh-add -l` 列出 Agent 中的密钥（添加后验证）
- [ ] I7.3 vault 锁定后 Agent 中 vault 密钥已移除
- [ ] I7.4 vault 解锁 + `vaultSshAutoAdd = true` → SSH 密钥自动添加
- [ ] I7.5 SSH 私钥明文添加后 zeroize（代码审查：add_to_agent 后无明文残留）
- [ ] I7.6 SSH 私钥不进 JS 堆（代码审查：add_to_agent 在 Rust 端完成）
- [ ] I7.7 密钥格式校验：OpenSSH / PEM / PuTTY .ppk 均可识别
- [ ] I7.8 无效密钥格式返回 `VaultError::InvalidKeyFormat`
- [ ] I7.9 added_fingerprints 追踪正确：仅移除由 vault 添加的密钥，不影响用户手动添加的
- [ ] I7.10 前端 SSH 条目详情正确显示密钥元信息
- [ ] I7.11 设置项四处同步

**依赖**：I5, I6

---

## 阶段 J — P2A''：恢复密钥 + 紧急访问

### J1. 恢复密钥生成

**文件**：`assistant-security/src/recovery_key/`

- [x] J1.1 `mod.rs`：`generate_recovery_key(master_key) → (String, EmergencyData)` — 20 字节随机 → Base32 编码 → 插入分隔符 → Argon2id 派生 → AES-256-GCM 加密 master_key → 构造 EmergencyData → zeroize 中间变量
- [x] J1.2 `EmergencyData` struct：token（base64 编码的 nonce‖encrypted_master_key‖tag）+ salt（Argon2id salt）
- [x] J1.3 `format_recovery_key(raw: &[u8]) → String` — Base32 编码 + XXXXX-XXXXX-XXXXX-XXXXX 分隔
- [x] J1.4 `parse_recovery_key(formatted: &str) → Result<Vec<u8>>` — 移除分隔符 + Base32 解码 + 长度校验

**依赖**：I7

---

### J2. 恢复密钥解锁

**文件**：`assistant-security/src/recovery_key/`

- [x] J2.1 `unlock_with_recovery_key(recovery_key, emergency_data, db_path) → Result<SecurityDbState>` — parse → Argon2id 派生 → AES-256-GCM 解密 master_key → 派生 sqlcipher_key → 打开 security.db → 存入 SecurityDbState → zeroize 中间变量
- [x] J2.2 解密失败返回 `VaultError::InvalidRecoveryKey`
- [x] J2.3 `VaultError` 新增 `InvalidRecoveryKey` / `RecoveryKeyNotSet` 变体

**依赖**：J1

---

### J3. .vault-meta 完整性保护

**文件**：`assistant-sync/src/vault_file/mod.rs`

- [x] J3.1 `compute_vault_meta_hmac(master_key, meta_content) → [u8; 32]` — HMAC-SHA256(master_key, 序列化后的 meta 内容)
- [x] J3.2 `export_vault_meta` 更新：写入前计算 HMAC 签名 → 存入 integrity 字段
- [ ] J3.3 ~~Git pull 后验证~~ — **已归入同步职责**：实现位置为 H6.1（`pull` 内调用 H5.7 `verify_vault_meta_integrity`），J 侧不再单独实现
- [ ] J3.4 恢复密钥解锁时验证：签名失败 → 警告"元数据可能被篡改" + 允许继续（用户确认）；复用 H5.7 校验函数，仅告警不阻断（本机库已存在，风险限于元数据展示）

**依赖**：H5, J2

---

### J4. 恢复密钥 Tauri Commands

**文件**：`src-tauri/src/cmd/vault.rs`

- [x] J4.1 `vault_generate_recovery_key() → String` — 生成恢复密钥 + 存入 .vault-meta emergency 字段 + 重新计算 HMAC 签名 + 审计日志（recovery_key_generated）+ **仅返回一次**
- [x] J4.2 `vault_unlock_with_recovery_key(recovery_key) → bool` — 恢复密钥解锁 + 审计日志（VaultUnlock("recovery_key")）+ master_key 不经 IPC
- [x] J4.3 `vault_has_recovery_key() → bool` — 检查 .vault-meta emergency 字段是否存在
- [x] J4.4 `vault_verify_vault_meta_integrity() → IntegrityResult` — 验证 .vault-meta HMAC 签名

**依赖**：J3

---

### J5. 灾难恢复流程

**文件**：`src-tauri/src/cmd/vault/recovery.rs`（命令与紧急仓库隔离）、`assistant-sync/src/sync_engine/mod.rs`（`clone_recovery_repo`）、`assistant-sync/src/git_sync/mod.rs`（clone 目标校验） — clone Git 仓库 → 恢复密钥解锁 → 验证 .vault-meta 完整性（复用 H5.7，失败标记 `integrityWarning` 但不阻断导出）→ import_all → 完成
  - ✅ 已完成：**紧急仓库隔离** — 克隆目标改为 `<app_data>/security/recovery`，不再复用全局 `SyncEngine`（正式仓库 `<app_data>/security/vault`）；克隆前清空紧急仓库，克隆后校验 `.vault-meta` 存在，失败即清理并显式返回错误；`GitSync::clone` 拒绝非空目标目录；移除 `let _ = engine.pull()` 静默吞错；无 `gitUrl` 时只读本地仓库，不再 `prepare()` / `pull()`
  - 未完成：导出仍依赖 `entries/*.enc`，而 H5.4 `export_all` 零调用 → 导出恒为 0 条
  - ⚠️ **缺陷 #12**：`.vault-meta` 缺少 `masterKeySalt` / `argon2Params` 字段，无法重建 security.db
  - ⚠️ **缺陷 #13**：`EmergencyDataFile` 仅存 `token+salt`，缺少 `encrypted_bootstrap` / `master_key_hash` / `master_key_salt`
- [ ] J5.2 前端 VaultUnlock：新设备"灾难恢复"入口 → 输入 Git URL + 恢复密钥 → 调用 disaster_recovery
  - ⚠️ **缺陷 #11**：恢复密钥按钮仅在 `isInitialized=true` 时显示，新设备无法进入

**依赖**：J4

---

### J6. 前端：恢复密钥集成

**文件**：`src/tools/password-vault/`

- [ ] J6.1 vaultStore 新增：`hasRecoveryKey: boolean` + actions：`generateRecoveryKey` / `unlockWithRecoveryKey` / `checkRecoveryKey`
- [ ] J6.2 VaultUnlock 扩展：恢复密钥解锁入口（输入框 + 解锁按钮）
- [ ] J6.3 VaultSettings 扩展：恢复密钥设置区（生成按钮 + 轮换按钮 + 状态显示）
- [ ] J6.4 恢复密钥显示弹窗：密钥明文 + 复制按钮 + "我已安全保存"按钮 + 安全提示
- [x] J6.5 vault.service.ts 新增：`generateRecoveryKey` / `unlockWithRecoveryKey` / `hasRecoveryKey` / `verifyVaultMetaIntegrity`

**依赖**：J4

---

### J7. i18n

**文件**：`src-tauri/crates/assistant-i18n/locales/`

- [ ] J7.1 `zh-CN.json` + `en-US.json` 新增恢复密钥相关约 15 词条

**依赖**：J4

---

### J8. 验证

- [ ] J8.1 `cargo check` 通过
- [ ] J8.2 恢复密钥生成：20 字符 Base32 + XXXXX-XXXXX-XXXXX-XXXXX 格式
- [ ] J8.3 恢复密钥解锁成功：生成 → 锁定 → 用恢复密钥解锁 → 条目可访问
- [ ] J8.4 无效恢复密钥返回 `VaultError::InvalidRecoveryKey`
- [ ] J8.5 恢复密钥仅返回一次（generate 后 .vault-meta 仅存加密数据，无明文）
- [ ] J8.6 恢复密钥解锁后 master_key 不经 IPC（代码审查）
- [ ] J8.7 .vault-meta HMAC 签名验证：正常时 valid=true，篡改后 valid=false
- [ ] J8.8 .vault-meta 不含 master_key_hash（代码审查）
- [ ] J8.9 Git pull 后 .vault-meta 签名验证（同步职责，见 H6.1）：篡改时中止同步且不写库
- [ ] J8.10 灾难恢复流程：新设备 clone + 恢复密钥解锁 → 全部条目可访问
- [ ] J8.11 恢复密钥轮换：旧密钥失效，新密钥可解锁
- [ ] J8.12 审计日志：recovery_key_generated / VaultUnlock("recovery_key") 正确写入

**依赖**：J5, J6, J7

---

## 阶段 K — P2 验收

### K1. 编译验证

- [ ] K1.1 `cargo check` 通过
- [ ] K1.2 `cargo clippy` 无新增 warning
- [ ] K1.3 `cargo test` 通过（含 assistant-sync）
- [ ] K1.4 `tsc --noEmit` 通过

**依赖**：J8

---

### K2. 安全不变式验证

- [ ] K2.1 Git 仓库不泄露明文（.enc 仅含 encryptedBlob + id/category/revision）
- [ ] K2.2 .vault-meta 不含 master_key_hash（代码审查）
- [ ] K2.3 .vault-meta HMAC 签名有效（单元测试：篡改后验证失败）
- [ ] K2.4 Git 凭证不存入仓库（sync_config 仅存 vault_meta）
- [ ] K2.5 双写一致性（集成测试：.enc 导出失败时 DB 事务回滚）
- [ ] K2.6 SSH 私钥不进 JS 堆（代码审查：add_to_agent 在 Rust 端完成）
- [ ] K2.7 SSH 私钥添加后 zeroize（代码审查）
- [ ] K2.8 恢复密钥仅返回一次（代码审查：generate 后不存明文）
- [ ] K2.9 恢复密钥解锁后 master_key 不经 IPC（代码审查）
- [ ] K2.10 `GIT_TERMINAL_PROMPT=0` 生效（认证失败返回错误而非挂起）

**依赖**：K1

---

### K3. 功能验证

- [ ] K3.1 Git 同步完整流程：init → add_remote → create_entry → sync_now → 远程仓库有 .enc 文件
- [ ] K3.2 双向同步：设备 A 修改 → push → 设备 B pull → 条目一致
- [ ] K3.3 冲突解决：同一条目双端修改 → pull 冲突 → UseLocal/UseRemote/MergeBoth 各选项正确
- [ ] K3.4 智能推送优化：30s 内 pull 过 → push 跳过
- [ ] K3.5 软删除同步：A 删除 → push → B pull → B 本地标记删除
- [ ] K3.6 vault_file 往返一致：export → import → 逐字段对比
- [ ] K3.7 SSH Agent 添加/移除：添加后 `ssh-add -l` 可见，移除后不可见
- [ ] K3.8 vault 锁定后 SSH Agent 密钥已移除
- [ ] K3.9 恢复密钥生成 → 锁定 → 恢复密钥解锁 → 条目可访问
- [ ] K3.10 灾难恢复：新设备 clone + 恢复密钥解锁 → 全部条目可访问
- [ ] K3.11 .vault-meta 完整性：正常操作后签名有效
- [x] ~~K3.12 自动同步计时器生效（间隔可配置）~~：已移除定时轮询机制
- [ ] K3.13 前端同步状态正确显示（idle/syncing/conflict/error）
- [ ] K3.14 VaultConflictResolve 组件正确显示 diff + 解决冲突

**依赖**：K2

---

### K4. 跨阶段约束验证

- [ ] K4.1 类型安全：新增 SyncConfig / SyncResult / Readyness / ConflictResolution / SshKeyInfo / IntegrityResult 类型，前端 TypeScript 与 Rust serde 对齐
- [ ] K4.2 Tauri Command 错误映射：VaultError 新增变体（InvalidKeyFormat / InvalidRecoveryKey / RecoveryKeyNotSet / VaultMetaIntegrityFailed）均序列化为可读字符串
- [ ] K4.3 安全不变式 10 项完整验证（对应 K2.1-K2.10）
- [ ] K4.4 向后兼容：P1 功能不受影响（同步为可选功能，未配置时行为不变）
- [ ] K4.5 条件编译策略：SSH ControlMaster 仅 Unix 生效（`#[cfg(unix)]`），Windows 跳过
- [ ] K4.6 设置项同步：Rust AppConf + 前端 AppConf + AppConfDto + From<AppConf> 四处字段一致（含 P2 新增 2 字段）

**依赖**：K3
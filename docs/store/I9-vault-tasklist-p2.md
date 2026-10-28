# I9 — 密码管理箱（Password Vault）P2 任务清单

> 规格：[I9-vault-spec-4-git-sync.md](./I9-vault-spec-4-git-sync.md) + [I9-vault-spec-5-ssh-and-recovery.md](./I9-vault-spec-5-ssh-and-recovery.md)
> 设计：[VAULT-DESIGN.md](../VAULT-DESIGN.md)
> 前置：P1-A ~ P1-F 全部完成（阶段 A ~ G ✅ + F' 生物识别平台真实实现 ✅）
> 阶段 L 为 P2B 补全（同步运维闭环 + 回收站 + Git 历史还原），对应 spec §11.22

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
- [x] H4.11 ~~待修项~~：`git_ref_hash` 硬编码 40 字符长度校验 —— 已改为「长度 40/64 + 全 ASCII 十六进制」双重校验，SHA-1 / SHA-256 仓库均可通过；新增用例 `ref_hash_accepts_sha256_repository`（`git init --object-format=sha256` 建库，断言 64 位哈希，旧版 git 不支持时跳过）

**依赖**：H3

---

### H5. vault_file 导出/导入

**文件**：`assistant-sync/src/vault_file/`

- [x] H5.1 `types.rs`：`EntryFile`（id / category / revision / encrypted_blob）+ `EntryBlob`（title / username / url / password / notes / custom_fields / totp_secret / totp_digits / totp_period / tags / is_favorite / created_at / updated_at）+ `VaultMetaFile`（version / exported_at / crypto / emergency / integrity）
  - 实际命名与规格（`EncFile` / `VaultBlob`）不同，统一使用 `EntryFile` / `EntryBlob`，语义一致
- [x] H5.2 `mod.rs`：`export_entry(master_key, snapshot) → Result<()>` — 由调用方提供 `VaultEntrySnapshot`（不再内部查库）→ 构造 `EntryBlob` → 按条目派生密钥整体加密 → 原子写入 `{id}.enc`
- [x] H5.3 `mod.rs`：`import_entry(master_key, entry_id) → Result<ImportedEntry>` — 读取 .enc → 解密 `EntryBlob` → 返回 `ImportedEntry`；**不直接写库**，落库由 `SecurityDb::upsert_imported_entries` 统一负责
- [x] H5.4 `export_all` — 实现在 `SyncEngine::export_all_entries`（不在 `vault_file`）：遍历 `export_all_snapshots()` → 逐条 `export_entry` → 删除 `deleted_at IS NOT NULL` 条目对应 `.enc` → 重签 `.vault-meta`
- [x] H5.5 `import_all` — 主体实现在 `SyncEngine::import_from_repo` + `SecurityDb::upsert_imported_entries`：pull 前记录 `entries/*.enc` 快照 → `read_all_entries` → 逐条 upsert（Inserted / Updated / SkippedStale / SkippedLocallyDeleted）；已含防误删护栏（pull 后条目为空而本地非空 → 直接报错中止）。**已实现**：`removedIds` 上报——pull 后重新列举 `.enc`，与 pull 前快照比对，仅存在于快照中的 ID 作为 `removed_ids` 随 `ImportReport` 返回
- [x] H5.6 `mod.rs`：`export_vault_meta(master_key, crypto, emergency) → Result<()>` — 构造 `VaultMetaFile` → `compute_vault_meta_hmac` 签名 → 原子写入 `.vault-meta`；`integrity` 恒由签名填充，不存在空壳 meta 写入路径
- [x] H5.7 `mod.rs`：`verify_vault_meta_integrity(master_key) → Result<IntegrityResult>` — 文件缺失 / `integrity` 为空 → `valid = false`；否则重算 HMAC 比对
  - 实际调用点：`SyncEngine::import_from_repo`（pull 路径，失败即 `VaultMetaIntegrityFailed` 中止，不写库）+ 灾难恢复导出（失败仅置 `integrityWarning`，不阻断导出）
- [x] ~~H5.8 **待定项（未纳入设计）**：Git 完整性边界~~ — **已定论并迁移**：Git 哈希链只提供无密钥完整性、不提供认证，不引入仓库级 HMAC；结论登记于 P3 阶段 P1，P2 侧不再作为待办
- [x] ~~H5.9 **待定项（未纳入设计）**：本地持久化 `last_good_commit`~~ — **未纳入规格并迁移**：回滚 / 历史重写检测需本地 commit 基准，受多设备并发、首次同步无基准、force push 语义约束；登记于 P3 阶段 P2

**依赖**：H4

---

### H6. SyncEngine 实现

**文件**：`assistant-sync/src/sync_engine/mod.rs`

- [x] H6.1 `SyncEngine::pull(repo_path, conn, master_key) → Result<SyncResult>` — `git pull` → `import_from_repo`（内部先 `verify_vault_meta_integrity`，失败返回 `VaultError::VaultMetaIntegrityFailed`，不写库、不 finalize/push）→ 返回 SyncResult
  - 完整性校验落在 `import_from_repo` 的写库前一刻，效果等同规格要求的“pull 中止”
- [x] H6.2 `SyncEngine::push(repo_path) → Result<SyncResult>` — `git push` → 返回 SyncResult
- [x] H6.3 `SyncEngine::sync_now(repo_path, conn, master_key) → Result<SyncResult>` — `prepare` → `import_from_repo`（含完整性校验，失败即返回错误，不进入 export/finalize）→ `export_all_entries` → `finalize`
- [x] H6.4 `SyncEngine::force_push(repo_path) → Result<SyncResult>` — `git push --force`

**依赖**：H5

---

### H7. DB 与文件双写集成

**文件**：`src-tauri/src/cmd/vault.rs`

- [x] H7.1 修改 `vault_create_entry`：在现有 CRUD 逻辑外层包裹 `sync.prepare()` + DB 事务（含 export_entry）+ `sync.finalize("vault: added {title}")` — 实现在 `src-tauri/src/cmd/vault/entries.rs`，经 `create_entry_tx` 事务内导出
- [x] H7.2 修改 `vault_update_entry`：同上模式 — `update_entry_tx` 事务内导出
- [x] H7.3 修改 `vault_delete_entry`：DB 软删除（`deleted_at = now()`，该列**保留不删**）+ 删除对应 `.enc` 文件 + `sync.finalize("vault: deleted {id}")` — `delete_entry_tx` 事务内删除 `.enc`
- [x] H7.4 修改 `vault_change_master_password`：全量重加密后 `export_all` + `sync.finalize("vault: master password changed")`
- [x] H7.5 双写一致性保证：export_entry 失败时 DB 事务回滚（步骤 2-5 在同一事务中）

**依赖**：H6

---

### H8. 软删除与 Git 同步交互

**文件**：`assistant-sync/src/vault_file/mod.rs`

> 前提：DB `vault_entries.deleted_at` 列**保留不删**，schema 不做迁移（`SCHEMA_VERSION` 保持 2）。删除状态由 DB 墓碑 + Git 删除记录承载，`deletions/deleted.json` 设计**已彻底移除**（不写入、不读取、不保留兼容代码）。同步流程**不擅自修改本地数据库**。

- [x] H8.1 ~~`read_deleted_index(repo_path)`~~ — **不实现**，`deletions/deleted.json` 读写能力已从代码中完全删除
- [x] H8.2 ~~`collect_legacy_deletions(repo_path)`~~ — **不实现**，不做旧仓库兼容解析
- [ ] H8.3 `purge_deleted(conn, repo_path) → Result<u32>` — 永久清理（仅用户手动触发）：`DELETE FROM vault_entries WHERE deleted_at IS NOT NULL` + 确保对应 `.enc` 已不存在 — **部分完成**：`SecurityDb::purge_deleted` 仅执行 DB 删除（含 `VaultEntryPurged` 审计），**不触碰 `.enc`**；`.enc` 的移除发生在软删除阶段（`delete_entry_tx` 的 export 回调），故"purge 后 `.enc` 不存在"是间接结论、代码中无显式校验。UI 侧 `TrashDrawer` 有 `modal.confirm` 二次确认 + 恢复入口，命令 `vault_purge_deleted` 已注册
- [x] H8.4 ~~`update_deleted_json`~~ — **不实现**，删除传播改由 §H5.4 / §H5.5 的文件集合对比完成

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

- [x] H10.1 `detect_conflicts(repo_path) → Result<Vec<Conflict>>`：检测 `Readyness::RepoState(Merge)` → `git diff --name-only --diff-filter=U` 获取冲突文件列表
  - 实现位置：`GitSync::detect_conflicts` + `git::git_unmerged_paths`（`diff --name-only --diff-filter=U`）+ `git::git_conflict_stage_hashes`（`ls-files -u` 解析 stage 2 / stage 3 对象哈希）
  - `pull` / `sync_now` 捕获 `GitStatus` 后改为返回真实冲突列表（此前恒为 `Vec::new()`）；枚举失败时降级为空列表并 `log::warn`，不掩盖「确有冲突」的事实
  - 测试：`detect_conflicts_reports_both_sides`（双 clone 改同一文件制造真实合并冲突，断言 path / 两侧哈希非空且不等）
- [ ] H10.2 `resolve_conflict(repo_path, entry_id, resolution, conn, master_key) → Result<()>`：根据 ConflictResolution 执行对应 git 操作（UseLocal: `git checkout --ours` / UseRemote: `git checkout --theirs` / MergeBoth: 保留本地 + 远程作为新条目 / Skip: 标记待处理）— 现状：UseLocal / UseRemote / Skip 已实现；**MergeBoth 仅退化为 UseLocal**，未把远程版本另存为新条目（补全归入 P3 阶段 N1）；~~且解决后只 `checkout + add`，不自动完成 merge commit（仓库停留在 Merge 状态，需后续 `sync_now` / `finalize` 收尾）~~ — 该备注已失效：L2.3 已实现无剩余冲突时 `commit_all`；另冲突解决结果不落库属 P3 阶段 M1 缺陷
- [x] H10.3 `vault_resolve_conflict(entry_id, resolution)` Tauri command — 现签名为 `vault_resolve_conflict(path, resolution)`，按冲突文件路径而非条目 ID 解决

**依赖**：H6

---

### H11. 同步 Tauri Commands

**文件**：`src-tauri/src/cmd/vault.rs`

- [x] H11.1 `vault_sync_pull() → SyncResult` — 调用 SyncEngine::pull — 已按现架构合并为单命令 `vault_sync(action: "pull")`
- [x] H11.2 `vault_sync_push() → SyncResult` — 调用 SyncEngine::push — 合并为 `vault_sync(action: "push")`
- [x] H11.3 `vault_sync_now() → SyncResult` — 调用 SyncEngine::sync_now — 合并为 `vault_sync(action: "sync")`
- [x] H11.4 `vault_sync_force_push() → SyncResult` — 需前端二次确认 — 合并为 `vault_sync(action: "forcePush")`，前端已有确认弹窗
- [x] H11.5 `vault_get_sync_config() → SyncConfig` — 从 vault_meta 读取
- [x] H11.6 `vault_set_sync_config(config) → void` — 写入 vault_meta
- [x] H11.7 `vault_git_init() → void` — 调用 GitSync::init — `vault_git_init(url: Option<String>)`，`None` 时 init
- [x] H11.8 `vault_git_clone(url) → void` — 调用 GitSync::clone — 同一命令 `vault_git_init(Some(url))` 分支
- [x] H11.9 `vault_git_add_remote(name, url) → void` — 合并为 `vault_git_remote(remote, url, mode: Add)`
- [x] H11.10 `vault_git_set_remote(name, url) → void` — 合并为 `vault_git_remote(remote, url, mode: Set)`
- [x] H11.11 `vault_git_readyness() → Readyness` — 调用 GitSync::readyness
- [x] H11.12 `vault_git_pull() → void` — 调用 GitSync::pull — 通过 `vault_sync(action: "pull")`
- [x] H11.13 `vault_git_push() → void` — 调用 GitSync::push — 通过 `vault_sync(action: "push")`

**依赖**：H7, H10

---

### H12. invoke_handler 注册

**文件**：`src-tauri/src/lib.rs` + `src-tauri/src/setup.rs`

- [x] H12.1 `setup.rs` 新增 `GitSync` + `SyncEngine` State 管理 — `SyncEngine` 内含 `Mutex<GitSync>`，由 `setup.rs` 统一 `manage`
- [x] H12.2 `lib.rs` invoke_handler 新增所有 `cmd::vault_sync_*` + `cmd::vault_git_*` 条目

**依赖**：H11

---

### H13. 前端：vaultStore 同步扩展

**文件**：`src/vault/core/slices/vaultSyncSlice.ts`（规格写作 `src/tools/password-vault/vaultStore.ts`，实际已拆分到 `src/vault/` 下的 slice 架构）

- [x] H13.1 新增状态字段：`syncStatus: "idle" | "syncing" | "conflict" | "error"` / `conflicts: Conflict[]` / `syncConfig: SyncConfig | null` / `readyness: Readyness | null` — 已按现架构落地为 `syncing: boolean` / `syncResult: SyncResult | null` / `syncConfig` / `syncReadyness`；未保留独立 `conflicts` 数组（冲突以 `Readyness::RepoState(Merge)` 呈现）
- [x] H13.2 新增 actions：`syncNow` / `syncPull` / `syncPush` / `resolveConflict` / `loadSyncConfig` / `saveSyncConfig` / `initGitRepo` / `cloneGitRepo` — 实际命名 `gitInit` / `gitClone` / `gitAddRemote` / `gitSetRemote`，已全部实现
- [x] ~~H13.3 自动同步计时器~~：已移除定时轮询，改为事件驱动（每次密码操作自动 prepare→finalize）+ 手动"立即同步"按钮

**依赖**：H12

---

### H14. 前端：VaultSyncSettings 组件

**文件**：`src/vault/settings/VaultSyncSettings.tsx`（规格写作 `src/tools/password-vault/VaultSyncSettings.tsx`，实际路径已迁移）

- [x] H14.1 远程仓库 URL 输入（已移除自动同步间隔选择）— 原缺口已修复：URL 输入框在 `VaultSyncSettings.tsx` 的 `<Form layout="vertical">` 内**无条件渲染**（不受 `isNoSync` 限制）；`handleInit` 调 `gitInit()` 后，若 URL 非空则 `saveSyncConfig(buildConfig())` → `vault_set_sync_config` → `engine.ensure_remote(...)`，确实写入 remote；保存按钮同样走该路径
- [ ] H14.2 状态指示：就绪/同步中/冲突/错误 + 上次同步时间 — 就绪/同步中/结果提示已实现；**上次同步时间**与**冲突态**未实现（`readyness` 为 `{ repoState }` 时一律落到 `status_unknown`）— 补全登记于 P3 阶段 N2 / N4
- [x] H14.3 操作按钮：初始化仓库 / 克隆仓库 / 立即同步 / 强制推送 — 四项均已实现：初始化 `handleInit`（`gitInit`）、克隆 `handleClone`（`gitClone`，本地已有条目时禁用并提示）、强制推送 `syncForcePush`（`forcePushOpen` 二次确认）；另补齐「测试连接」（`gitTestConnection`）与「移除远程」（`gitRemoveRemote`）
- [x] H14.4 `VaultSettings.tsx` 集成：新增"同步"标签页

**依赖**：H13

---

### H15. 前端：VaultConflictResolve 组件

**文件**：`src/vault/settings/VaultConflictResolve.tsx`（规格写作 `src/tools/password-vault/VaultConflictResolve.tsx`）

- [x] H15.1 冲突列表显示：条目标题 + 本地/远程修改时间 — 落地为 `VaultConflictResolve` 内左右两栏 `Descriptions`，标题 / 分类 / 时间均来自解密预览；后端新增 `ConflictPreview`（`entryId` / `local` / `remote` / `error`），无需扩展 `ConflictFile`
- [x] H15.2 解密 diff 显示（Rust 端解密，仅非敏感字段明文，敏感字段 `•••` 遮蔽）— 后端新增 `SyncEngine::conflict_preview(db, path)`：`git show :2:<path>` / `:3:<path>` 取 ours/theirs 两个 stage，复用 `decode_entry_file` + `mask_entry_preview`（与 L5.3 历史预览同一遮蔽逻辑）；命令 `vault_conflict_preview(path)`；前端 `vaultService.conflictPreview`，组件加载即拉取，解密失败降级为 `Alert` 提示且不阻塞解决操作
- [x] H15.3 解决选项按钮：使用本地 / 使用远程 / 合并保留两者 / 跳过 — 四选项 UI 与命令均已实现（`MergeBoth` 后端目前退化为使用本地）
- [x] H15.4 VaultList 集成：`syncStatus === "conflict"` 时显示冲突提示 + 打开 VaultConflictResolve — 已闭环（见 L2.1~L2.3）：`repoState` 分支渲染 `Alert` 提示条，冲突文件选择后打开 `VaultConflictResolve`

**依赖**：H13

---

### H16. 前端：VaultList 同步状态栏

**文件**：`src/vault/VaultList.tsx`（规格写作 `src/tools/password-vault/VaultList.tsx`）

- [ ] H16.1 底部状态栏新增同步状态：条目数 · 上次同步时间 · 泄露数 — 条目数与泄露数已实现（`VaultList.tsx` 的 `entries.length` / `leakCount`）；**上次同步时间**未实现（无对应数据源，`vaultStore` 无 `lastSyncAt`，后端也未提供）— 补全登记于 P3 阶段 N2
- [x] H16.2 新增同步按钮（`↻`），点击调用 `syncNow()`
- [x] H16.3 `syncStatus === "syncing"` 时按钮旋转动画

**依赖**：H13

---

### H17. 前端：vault.service.ts 同步扩展

**文件**：`src/vault/core/vault.service.ts`（规格写作 `src/tools/password-vault/vault.service.ts`）

- [x] H17.1 新增 invoke 封装：`syncPull` / `syncPush` / `syncNow` / `syncForcePush` / `getSyncConfig` / `setSyncConfig` / `resolveConflict` / `gitInit` / `gitClone` / `gitAddRemote` / `gitSetRemote` / `gitReadyness` / `gitPull` / `gitPush` — 实际按合并后的命令落地为 `vault_sync(action)` 与 `vault_git_init(url?)` 等，`resolveConflict` 签名为 `(path, resolution)`

**依赖**：H12

---

### H18. 设置项 + i18n

**文件**：`src-tauri/crates/assistant-config/src/app_conf.rs` + `src/vault/core/vault.service.ts` + `src/i18n/locales/`（规格写作 `src-tauri/crates/assistant-i18n/locales/`，前端词条实际在 `src/i18n/locales/`，该 crate 的 locales 内无 `vault` 命名空间）

- [x] ~~H18.1 Rust `AppConf` struct 新增 `vault_auto_sync_interval_minutes: u32`~~：已移除，不再需要自动同步间隔字段
- [x] ~~H18.2 前端 `AppConf` interface 新增 `vaultAutoSyncIntervalMinutes: number`~~：已移除
- [x] H18.3 `AppConfDto` + `From<AppConf>` impl 同步更新
- [x] H18.4 `zh-CN.json` + `en-US.json` 新增同步相关约 30 词条 — 实际落地为 `vault.sync.*` / `vault.conflict.*` / `vault.disaster_recovery.*` 精简词条集；`VaultConflictResolve` 引用的 8 个 `vault.conflict.*` 键均已存在（无需补词条，缺的是组件挂载与数据）

**依赖**：H12

---

### H19. 验证

- [x] H19.1 `cargo check` 通过
- [x] H19.2 `cargo test -p assistant-sync` 通过 — 已建 `tests/{git_sync,vault_file,sync_engine}.rs`，21 项全部通过（git_sync 7 / sync_engine 5 / vault_file 9）
- [x] H19.3 `tsc --noEmit` 通过
- [x] H19.4 GitSync::init → GitSync::readyness 返回 Ready — `readyness_is_no_sync_before_init_and_ready_after`
- [x] H19.5 GitSync::clone 从远程仓库克隆成功 — `push_then_clone_roundtrip`
- [x] H19.6 prepare → 本地修改 → finalize：变更已 commit + push — `finalize_without_remote_only_commits`、`dirty_after_change_and_ready_after_commit`
- [x] H19.7 need_to_push 智能优化：30s 内 pull 过且 hash 相同 → 返回 false — `git_sync::safe_need_to_push` 已实现（`FETCH_HEAD` mtime + 本地/上游 hash 比对）；缺单元测试，测试缺口登记于 P3 阶段 O1
- [x] H19.8 vault_file export → import 往返一致（逐字段加密 → 整体打包 → 解密 → 逐字段还原） — `export_import_roundtrip_preserves_all_fields`
- [x] H19.9 双写一致性：export_entry 失败时 DB 事务回滚 — `create_tx_rolls_back_when_export_fails`、`delete_tx_rolls_back_when_export_fails`
- [x] H19.10 软删除：本地删除 → DB `deleted_at` 标记 + `.enc` 移除；pull 时被移除文件仅上报 `removedIds`，不自动改库 — 软删除、`.enc` 移除与 `removedIds` 上报均已实现；回收站恢复 / 永久清理见 L4
- [x] H19.11 冲突检测：Readyness::RepoState(Merge) 正确识别 — `detect_conflicts_reports_both_sides`（冲突文件枚举 + 两侧哈希）；`Readyness::RepoState` 的识别由 `readyness_is_no_sync_before_init_and_ready_after` 间接覆盖，未单独构造 rebase/cherry-pick 用例
- [ ] H19.12 冲突解决：UseLocal / UseRemote / MergeBoth 各选项行为正确 — 复验前置条件：M1（解决结果落库）、N1（MergeBoth 真实现）、N5（解决后 push）修复后再验收
- [x] H19.13 .vault-meta HMAC 签名验证：篡改后验证失败 — `tampered_vault_meta_fails_verification`、`unsigned_vault_meta_fails_verification`、`missing_vault_meta_fails_verification`、`vault_meta_signed_by_other_key_fails_verification`
- [x] H19.14 .vault-meta 不含 master_key_hash（代码审查） — `VaultCryptoMeta` 仅含 `master_key_salt` / `hash_salt` / argon2 参数 / 默认算法
- [x] ~~H19.15 SSH ControlMaster 复用（Unix only）：连续 push/pull 仅一次握手~~ — **本条关闭（非验证通过）**：`ssh.rs` 仅在 Unix 注入 `ControlMaster` 且白名单限 `github.com` / `gitlab.com`，Windows 无复用；"仅一次握手"的手工验证与白名单是否扩展（自建 GitLab / Gitea / 内网）登记于 P3 阶段 P3，此处不再重复验收
- [ ] H19.16 `GIT_TERMINAL_PROMPT=0` 生效：认证失败返回错误而非挂起 — 代码已注入（`git.rs` 的 `cmd.env("GIT_TERMINAL_PROMPT", "0")`），但**无带超时断言的测试**，未验证"不挂起"，测试缺口登记于 P3 阶段 O1
- [x] ~~H19.17 前端同步状态正确显示（idle/syncing/conflict/error）~~ — **本条作废（模型变更）**：前端不存在 `syncStatus` 四态字段；实际表达为 `syncing`（loading）+ `syncResult`（`SyncResult` 变体，冲突判断走 `"conflict" in result`）+ `readyness`（`ready` / `dirty` / `noSync` / `{ repoState }`）；`repoState` 文案缺陷见 P3 阶段 N4
- [x] ~~H19.18 自动同步计时器生效（间隔可配置）~~：已移除定时轮询机制
- [ ] H19.19 设置项四处同步：Rust AppConf + 前端 AppConf + AppConfDto + From<AppConf>
- [x] H19.20 pull 完整性闸门：签名篡改 / `integrity` 为空时 `pull` 中止，且未执行任何 DB 写入 — `import_aborts_when_vault_meta_tampered`

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

- [x] I2.1 `SshAgentState` struct：`added_fingerprints: Mutex<HashSet<String>>`（追踪由 vault 添加的密钥）+ `bridge: Box<dyn SshAgentBridge>`
- [x] I2.2 vault 解锁时：若 `vaultSshAutoAdd = true` → 遍历 `category = "ssh"` 条目 → 逐条 add_to_agent → 追踪 fingerprint
- [x] I2.3 vault 锁定时：遍历 `added_fingerprints` → 逐条 remove_from_agent → 清空 set
- [x] I2.4 添加 SSH 条目时：若 vault 已解锁且 Agent 运行 → 自动添加
- [x] I2.5 删除 SSH 条目时：从 Agent 移除对应密钥

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

- [x] I5.1 vaultStore 新增：`sshAgentKeys: SshKeyInfo[]` / `sshAutoAddEnabled: boolean` + actions：`addSshToAgent` / `removeSshFromAgent` / `loadSshAgentKeys`
- [x] I5.2 VaultEntryDetail 扩展：`category === "ssh"` 时显示密钥类型 + 位数 + 指纹 + 公钥 + [添加到 Agent] / [从 Agent 移除] 按钮
- [x] I5.3 VaultSettings 扩展：SSH Agent 设置区（自动添加开关 + Agent 状态 + 密钥列表）
- [x] I5.4 vault.service.ts 新增：`sshAddToAgent` / `sshRemoveFromAgent` / `sshListAgent` / `sshAutoAddAll`

**依赖**：I4

---

### I6. 设置项 + i18n

**文件**：`src-tauri/crates/assistant-config/src/app_conf.rs` + `src-tauri/crates/assistant-i18n/locales/`

- [x] I6.1 Rust `AppConf` struct 新增 `vault_ssh_auto_add: bool` + `#[serde(default)]`
- [x] I6.2 前端 `AppConf` interface 新增 `vaultSshAutoAdd: boolean`
- [x] I6.3 `AppConfDto` + `From<AppConf>` impl 同步更新
- [x] I6.4 `zh-CN.json` + `en-US.json` 新增 SSH Agent 相关约 15 词条

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

**文件**：`src-tauri/src/cmd/vault/recovery.rs`（命令与紧急仓库隔离）、`assistant-sync/src/sync_engine/mod.rs`（`clone_recovery_repo`）、`assistant-sync/src/git_sync/mod.rs`（clone 目标校验）

- [ ] J5.1 `disaster_recovery(git_url, recovery_key, db_path) → Result<()>` — clone Git 仓库 → 恢复密钥解锁 → 验证 .vault-meta 完整性（复用 H5.7，失败标记 `integrityWarning` 但不阻断导出）→ import_all → 完成
  - ✅ 已完成：**紧急仓库隔离** — 克隆目标改为 `<app_data>/security/recovery`，不再复用全局 `SyncEngine`（正式仓库 `<app_data>/security/vault`）；克隆前清空紧急仓库，克隆后校验 `.vault-meta` 存在，失败即清理并显式返回错误；`GitSync::clone` 拒绝非空目标目录；移除 `let _ = engine.pull()` 静默吞错；无 `gitUrl` 时只读本地仓库，不再 `prepare()` / `pull()`
  - 导出语义：`disaster_recovery_export` 直接读取紧急仓库的 `entries/*.enc`；仓库侧清单由 `sync_now` 中的 `export_all_entries` 写入，恢复时以克隆得到的仓库内容为准，不触碰本地正式仓库（原“`export_all` 零调用 → 导出恒为 0 条”的备注已失效）
  - 已按规格 §13.5 收敛：**缺陷 #12 失效** — `.vault-meta` 的 `crypto` 已包含 `masterKeySalt` / `argon2Params`，且新设计明确恢复流程不重建 `security.db`，定位为只读解密 + JSON 导出
  - **缺陷 #13 失效**：`EmergencyDataFile` 的 `token + salt` 已满足 §13.5 解密 master_key 所需，`bootstrap_key` 解密后仅用于 zeroize
  - **验收缺口**：`vault_disaster_recovery` 未在 `src-tauri/src/lib.rs` 的 `invoke_handler` 注册（当前仅注册 `vault_disaster_recovery_export`），故本条命令对前端不可达；本条应保持未完成，直至注册并接通前端
- [ ] J5.2 前端 VaultUnlock：新设备"灾难恢复"入口 → 输入 Git URL + 恢复密钥 → 调用 disaster_recovery — **部分完成**：入口已落地且不受 `isInitialized` 限制（~~缺陷 #11~~ 已修），`VaultUnlock.tsx` 无条件渲染「灾难恢复」按钮 → `unlock/DisasterRecovery.tsx` 三步流程（Git URL → 恢复密钥 → 结果）；但**终点是 `vault_disaster_recovery_export`（克隆到紧急仓库 + 解密 + 导出 JSON 文件），未调用 `vault_disaster_recovery`**——该命令亦未在 `lib.rs` 注册，前端无法触达"直接恢复进本地库"路径

**依赖**：J4

---

### J6. 前端：恢复密钥集成

**文件**：`src/tools/password-vault/`

- [x] J6.1 vaultStore 新增：`hasRecoveryKey: boolean` + actions：`generateRecoveryKey` / `unlockWithRecoveryKey` / `checkRecoveryKey` — 已落地，命名有差异：状态字段为 `recoveryKeySet`，检查 action 为 `checkRecoveryKeySet`；`generateRecoveryKey` / `unlockWithRecoveryKey` 均在 `core/slices/vaultRecoverySlice.ts`，`vaultStore.test.ts` 有对应用例
- [x] J6.2 VaultUnlock 扩展：恢复密钥解锁入口（输入框 + 解锁按钮） — `unlock/RecoveryKeyForm.tsx`（输入框 + 解锁按钮），在 `VaultUnlock.tsx` 中按 `isInitialized && recoveryKeySet` 条件渲染（未初始化或无恢复密钥时不显示，属合理约束）
- [ ] J6.3 VaultSettings 扩展：恢复密钥设置区（生成按钮 + 轮换按钮 + 状态显示） — **部分完成**：`settings/RecoveryTab.tsx` 有生成按钮（已设置时文案切换为 `key_regenerate`，重新生成即覆盖旧密钥，等价轮换）+ 完整性校验按钮 + `recoveryKeySet` 状态文案；但**无独立"轮换"按钮与轮换说明**，轮换语义隐含在"重新生成"中，未显式表达
- [ ] J6.4 恢复密钥显示弹窗：密钥明文 + 复制按钮 + "我已安全保存"按钮 + 安全提示 — **未完成**：`RecoveryTab.tsx` 的 `Modal` 仅有 `Input.TextArea`（readOnly 展示密钥）+ 安全提示文案 + 单个「关闭」按钮；**缺复制按钮**，**缺"我已安全保存"确认按钮**（关闭即丢弃，无确认闭环）
- [x] J6.5 vault.service.ts 新增：`generateRecoveryKey` / `unlockWithRecoveryKey` / `hasRecoveryKey` / `verifyVaultMetaIntegrity`

**依赖**：J4

---

### J7. i18n

**文件**：`src-tauri/crates/assistant-i18n/locales/`

- [x] J7.1 `zh-CN.json` + `en-US.json` 新增恢复密钥相关约 15 词条 — 已落地，**实际路径为前端 `src/i18n/locales/`**（规格写的 `assistant-i18n` crate 不含 vault 词条）；覆盖 `vault.unlock.recovery_key_*`（解锁入口）与 `vault.recovery.*` / `vault.settings.recovery.*`（设置区、弹窗、完整性校验）两组

**依赖**：J4

---

### J8. 验证

- [x] J8.1 `cargo check` 通过
  - 必须在 `src-tauri/` 目录下执行，否则 `src-tauri/.cargo/config.toml` 的 `[env] OPENSSL_DIR` 不会加载，`libsqlite3-sys`（bundled-sqlcipher）构建会因缺少 OpenSSL 而失败
  - 已执行：`cargo check --workspace`（1m17s，exit 0），仅剩 incremental 目录权限警告
- [ ] J8.2 恢复密钥生成：20 字符 Base32 + XXXXX-XXXXX-XXXXX-XXXXX 格式
- [ ] J8.3 恢复密钥解锁成功：生成 → 锁定 → 用恢复密钥解锁 → 条目可访问
- [ ] J8.4 无效恢复密钥返回 `VaultError::InvalidRecoveryKey`
- [ ] J8.5 恢复密钥仅返回一次（generate 后 .vault-meta 仅存加密数据，无明文）
- [ ] J8.6 恢复密钥解锁后 master_key 不经 IPC（代码审查）
- [ ] J8.7 .vault-meta HMAC 签名验证：正常时 valid=true，篡改后 valid=false
- [ ] J8.8 .vault-meta 不含 master_key_hash（代码审查）
- [x] J8.9 Git pull 后 .vault-meta 签名验证（同步职责，见 H6.1）：篡改时中止同步且不写库 — `import_aborts_when_vault_meta_tampered`（`import_from_repo` 校验失败即 `VaultMetaIntegrityFailed`，DB 无写入）
- [ ] J8.10 灾难恢复流程：新设备 clone + 恢复密钥解锁 → 全部条目可访问
- [ ] J8.11 恢复密钥轮换：旧密钥失效，新密钥可解锁
- [ ] J8.12 审计日志：recovery_key_generated / VaultUnlock("recovery_key") 正确写入

**依赖**：J5, J6, J7

---

## 阶段 K — P2 验收

### K1. 编译验证

- [x] K1.1 `cargo check` 通过
  - 需在 `src-tauri/` 目录下执行（加载 `.cargo/config.toml` 的 `OPENSSL_DIR`），`cargo check --workspace` 已通过
- [ ] K1.2 `cargo clippy` 无新增 warning
- [ ] K1.3 `cargo test` 通过（含 assistant-sync）
- [ ] K1.4 `tsc --noEmit` 通过

**依赖**：J8

---

### K2. 安全不变式验证

- [x] K2.1 Git 仓库不泄露明文（.enc 仅含 encryptedBlob + id/category/revision） — `entry_file_keeps_only_id_category_revision_in_plaintext`
- [x] K2.2 .vault-meta 不含 master_key_hash（代码审查）
- [x] K2.3 .vault-meta HMAC 签名有效（单元测试：篡改后验证失败） — `vault_file.rs` 4 项验证用例
- [ ] K2.4 Git 凭证不存入仓库（sync_config 仅存 vault_meta）
- [x] K2.5 双写一致性（集成测试：.enc 导出失败时 DB 事务回滚） — `create_tx_rolls_back_when_export_fails`、`delete_tx_rolls_back_when_export_fails`
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
- [ ] K3.3 冲突解决：同一条目双端修改 → pull 冲突 → UseLocal/UseRemote/MergeBoth 各选项正确 — 复验前置：M1 / N1 / N5 修复（当前「使用远程」会被本地覆盖、MergeBoth 退化为 UseLocal）
- [ ] K3.4 智能推送优化：30s 内 pull 过 → push 跳过 — 代码已实现（`safe_need_to_push`），但导出非幂等（M3）会导致本地 hash 恒变、优化失效，需 M3 修复后复验
- [ ] K3.5 软删除同步：A 删除 → push → B pull → B 本地标记删除 — 当前 B 侧仅上报 `removed_ids` 不落库（M2），需修复后复验
- [ ] K3.6 vault_file 往返一致：export → import → 逐字段对比
- [ ] K3.7 SSH Agent 添加/移除：添加后 `ssh-add -l` 可见，移除后不可见
- [ ] K3.8 vault 锁定后 SSH Agent 密钥已移除
- [ ] K3.9 恢复密钥生成 → 锁定 → 恢复密钥解锁 → 条目可访问
- [ ] K3.10 灾难恢复：新设备 clone + 恢复密钥解锁 → 全部条目可访问
- [x] K3.11 .vault-meta 完整性：正常操作后签名有效 — `tampered_vault_meta_fails_verification` 等用例已覆盖签名有效/失效路径
- [x] ~~K3.12 自动同步计时器生效（间隔可配置）~~：已移除定时轮询机制
- [x] ~~K3.13 前端同步状态正确显示（idle/syncing/conflict/error）~~ — **本条作废（模型变更）**：同 H19.17，前端无 `syncStatus` 四态字段；状态由 `syncing` + `syncResult` + `readyness` 表达，`repoState` 文案缺陷见 P3 阶段 N4
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

---

## 阶段 L — P2B：同步运维补全（H8/H14/H15 缺口闭环 + Git 历史还原）

> 来源：P2A 落地后的缺口复盘（`VaultSyncSettings` / `VaultConflictResolve` / `VaultList` 与后端能力对账）。H8.3、H14.1、H14.3、H15.2、H15.4 的遗留项统一在本阶段闭环，并新增「回收站」与「历史浏览 / 单条目还原」。
> 设计详见 `I9-vault-spec-4-git-sync.md` §11.22；界面参照项目见 §11.22.8（`keepassxc` 为主，`ripasso-gtk` 为辅，均在现有 `references/` 内，无需新增仓库）。
>
> **两条硬约束**：
> 1. **只做单条目还原，不做整仓库 `reset --hard <commit>`**——整仓回滚会绕开 revision 语义、全量倒退、极易误操作。
> 2. **还原必须提升 revision**（`本地 revision + 1`），用当前密钥重新加密写入；**禁止直接复制历史密文字节**——否则 `upsert_imported_in` 会按 `SkippedStale` 静默丢弃，且 `export_all_entries` 会立刻用 DB 当前版本覆盖回去，还原看似成功实则无效。

### L1. 远程仓库配置闭环

**文件**：`src/vault/settings/VaultSyncSettings.tsx` + `src-tauri/src/cmd/vault/sync.rs` + `assistant-sync/src/sync_engine/mod.rs`

- [x] L1.1 首次接入区合并渲染：`isNoSync` 时**同时**渲染「远程仓库地址」输入框 + `[初始化仓库]` / `[克隆仓库]` 两个按钮
  - 落地：URL 输入框改为始终渲染（与「测试连接」同排 `Space.Compact`）；`isNoSync` 分支追加 `first_setup_hint` 说明与克隆前置警告；`handleInit` 在 URL 非空时继续调用 `saveSyncConfig` 完成绑定，`gitAddRemote` 分支可达
- [x] L1.2 保存真正写入 Git remote：`handleSaveConfig` 在 `saveSyncConfig` 之外，按「是否已有 remote」分派 `gitAddRemote` / `gitSetRemote`
  - 落地：绑定逻辑收敛到后端 `vault_set_sync_config`（已有 `engine.ensure_remote()`：无则 add、有则 set），前端保存后重新 `loadSyncConfig`，`has_remote()` 读到的真实 git 配置与 SyncConfig 保持一致
- [x] L1.3 克隆按钮 UI 入口：调用 `vaultStore.gitClone(url)`（store action / service 方法 / 后端 `vault_git_init(url: Some)` 分支均已就绪）
- [x] L1.4 克隆前置校验：本地条目数 > 0 时禁用克隆并提示原因
  - 落地：`cloneBlocked = entries.length > 0`，禁用按钮并展示 `clone_blocked_local_entries` 警告条
- [x] L1.5 新增 `GitSync::remove_remote` + 命令 `vault_git_remove_remote(remote)`：`GitSync` 无此方法，但底层 `git::git_remote_remove` 已存在，仅需包一层；用于解绑 / 迁移仓库（当前无任何撤销 remote 的路径）
  - 落地：`GitSync::remove_remote` → `SyncEngine::remove_remote` → 命令层同步清空 `AppConf.vault_sync_remote_url` 与 `SyncConfig.remote_url`；设置页 `[解绑远程仓库]` 二次确认
- [x] L1.6 新增 `git::git_ls_remote(url)` + `GitSync::test_connection` + 命令 `vault_git_test_connection(url?)`：`git ls-remote`（`GIT_TERMINAL_PROMPT=0`），保存前验证 URL / SSH 可达性，失败返回 stderr 摘要
  - 落地：`git::git_ls_remote` 失败时返回 stderr 摘要（空则回退退出码）；`SyncEngine::test_connection` 支持显式 URL 或回退 `SyncConfig.remote_url`；命令返回远程分支数，前端「测试连接」按钮展示 `test_ok`

**依赖**：H14

---

### L2. 冲突入口闭环

**文件**：`src/vault/VaultList.tsx` + `src/vault/settings/VaultConflictResolve.tsx` + `assistant-sync/src/sync_engine/mod.rs`

- [x] L2.1 `VaultList` 处理 `Readyness = { repoState }` 分支：`merge` / `rebase` / `cherryPick` 时显示冲突提示条（当前仅处理 `ready` / `dirty` / `noSync`，冲突态无任何入口）
  - 提示条参照 `references/3-vault-sync/keepassxc/src/gui/MessageWidget.*`；「受阻 + 一条出路」对话框参照 `references/1-ai-agent/OpenBitFun/src/web-ui/src/tools/git/components/BranchQuickSwitch.tsx`
  - 落地：`repoState` 非空时渲染 `Alert` 提示条，文案含仓库状态与冲突文件数；`loadSyncReadyness` 在冲突态自动拉取 `syncConflicts`
- [x] L2.2 挂载 `VaultConflictResolve`（当前组件未被任何组件 import，属孤立组件），由提示条打开
  - 落地：提示条「处理冲突」打开冲突文件选择 `Modal`，选定文件后打开 `VaultConflictResolve`；后端新增 `vault_sync_conflicts` 命令（`SyncEngine::conflicts`）
- [x] L2.3 冲突解决后收尾：`resolve_conflict` 完成后补 `commit`（或显式提示用户点「立即同步」），避免仓库长期停留在 Merge 状态
  - 落地：`SyncEngine::resolve_conflict` 在无剩余冲突时自动 `commit_all("vault: resolve conflict")`；前端解决成功后提示「冲突已解决」并刷新 readyness
- [x] L2.4 冲突双版本预览：`SyncEngine::conflict_preview`（`git show :2:` / `:3:` + 遮蔽）+ `vault_conflict_preview` 命令 + `VaultConflictResolve` 左右两栏展示（解除 H15.1 / H15.2 阻塞）
  - 落地：集成测试 `conflict_preview_decrypts_both_sides_with_masking` 构造真实双端冲突，验证两侧标题解密与密码遮蔽

**依赖**：H15, L5.3（diff 预览接口）

---

### L3. 同步恢复操作

**文件**：`assistant-sync/src/sync_engine/mod.rs` + `assistant-sync/src/git_sync/git.rs` + `src-tauri/src/cmd/vault/sync.rs` + `src/vault/settings/VaultSyncSettings.tsx`

- [x] L3.1 新增 `git::git_abort(repo, state)`（`merge --abort` / `rebase --abort` / `cherry-pick --abort`，`git.rs` 目前无 abort 封装）+ `SyncEngine::abort_repo_state()` + 命令 `vault_sync_abort`
  - 落地：`git::git_abort` 按 `RepositoryState` 映射到 `merge` / `rebase` / `cherry-pick` / `revert` / `am` 的 `--abort`，不可中止态返回 `GitCli` 错误；`GitSync::abort_state` 先读状态再中止；前端 `repoState` 提示条内联 `[中止同步]`
- [x] L3.2 `SyncEngine::discard_local_changes()` → `GitSync::reset_hard_all()`（已存在，`git add -A` + `git reset --hard`）；新增命令 `vault_sync_discard`，danger zone + 二次确认
- [x] L3.3 `SyncEngine::remove_remote(name)` → `GitSync::remove_remote`（配合 L1.5）
- [x] L3.4 前端：`{ repoState }` 时显示 `[中止同步]`；danger zone 放 `[放弃本地改动]`（二次确认）+ `[移除远程仓库]`
- [x] L3.5 按钮语义收敛：以「立即同步」为主按钮，`pull` / `push` 收进「更多操作」，避免与 `sync_now` 语义重叠
  - 落地：主按钮 `[同步]` + `Dropdown`「更多操作」内 `拉取` / `推送`

**依赖**：H10, L1

---

### L4. 软删除运维（回收站 + 永久清理）

**文件**：`assistant-security/src/security_db/crud.rs` + `src-tauri/src/cmd/vault/entries.rs` + `src/vault/`

- [x] L4.1 `SecurityDb::list_deleted_entries()` → 复用 `list_deleted_entry_ids` + 读墓碑快照（允许 `deleted_at IS NOT NULL`），返回 `VaultEntrySummary[]`
- [x] L4.2 `restore_entry_tx(conn, master_key, id, export)` → 事务内 `deleted_at = NULL` + `revision = revision + 1` + `updated_at = now` + 事务内 `export_entry` 重建 `.enc`
  - 动机：当前无任何 undelete 能力，软删除后导入一律命中 `ImportOutcome::SkippedLocallyDeleted`
- [x] L4.3 `purge_deleted(conn, repo_path)`（H8.3）→ `DELETE FROM vault_entries WHERE deleted_at IS NOT NULL` + 确认对应 `.enc` 已不存在；新增命令 `vault_purge_deleted`
  - 落地：`SecurityDb::purge_deleted()` 事务内删除墓碑并写审计 `vault_entry_purged`；命令层先取墓碑 ID 列表，清库后逐个 `remove_entry_file` 兜底
- [x] L4.4 前端回收站：列表 + `[恢复]` + `[永久清理]`；永久清理二次确认并提示「清理后无法再从 Git 历史还原」
  - 落地：新增 `src/vault/TrashDrawer.tsx`，由 `VaultList` 工具栏「回收站」按钮打开；恢复与清空均二次确认
- [x] L4.5 恢复后 `finalize("vault: restore {id}")` 触发 commit + push

**依赖**：H8, L5（共用解密 / 审计基础设施）

---

### L5. Git 历史浏览与单条目还原

**文件**：`assistant-sync/src/git_sync/git.rs` + `assistant-sync/src/sync_engine/mod.rs` + `src-tauri/src/cmd/vault/history.rs`

- [x] L5.1 git 基础封装补全：`git_log` / `git_show_file(commit, path)`（`git_log` 已带 `--name-only`，可定位条目路径）
  - 落地：`git_log(repo, path: Option<&str>, limit)` 以 `%H%x1f%ct%x1f%s` 单行头 + `--name-only` 解析为 `RawLogEntry{hash,timestamp,message,paths}`；空仓库返回空列表而非报错
- [x] L5.2 `SyncEngine::history_list(entry_id: Option<&str>, limit) -> Vec<HistoryCommit>`（`hash` / `timestamp` / `message` / `entry_id`）
  - 落地：路径固定 `entries/<id>.enc`；未指定条目时从提交路径反解 id
- [x] L5.3 `SyncEngine::history_preview(commit, entry_id) -> EntryPreview`：`git show` 取旧 `EntryFile` → 当前 master key 解密 → 敏感字段遮蔽（`password` / `totp_secret` / `is_secret` 的 `custom_fields` → `•••`）
  - 与 H15.2 冲突 diff **共用同一预览接口**，一次性解除「后端命令缺失」阻塞
  - 落地：解密失败统一映射 `SyncError::HistoryDecryptFailed`，前端据此提示「旧主密码加密」
- [x] L5.4 `SyncEngine::history_restore(commit, entry_id, db)`：
  1. `readyness` 为 `RepoState` 时拒绝执行（中间态 checkout 会污染冲突流程）
  2. `git show <commit>:entries/<id>.enc` 取旧 `EntryFile`
  3. 当前 master key 解密 → `EntryBlob`；失败返回明确错误「该版本由旧主密码加密，无法还原」，**不写任何数据**
  4. 事务内 `restore_entry_tx`：`revision = 本地 revision + 1`、`updated_at = now`、`deleted_at = NULL`，并用当前密钥重新加密落盘
  5. `finalize("vault: restore {id}")` → commit + push
  6. 审计日志记录来源 commit
  - 落地：新增 `SecurityDb::restore_from_history_tx(entry, source, export)`，存在则 `revision+1` 并清墓碑；不存在则按「全库最大 revision + 1」插入，避免 `SkippedStale`
- [x] L5.5 新增命令：`vault_history_list(entryId?, limit)` / `vault_history_preview(commit, entryId)` / `vault_history_restore(commit, entryId)`
- [x] L5.6 前端历史面板：条目详情「历史」入口 → 版本列表（时间 + 提交信息 + 操作类型）→ 预览 → 还原（二次确认）
  - 落地：`src/vault/detail/VaultEntryHistory.tsx`，左列提交（时间 + 操作标签 + `Current` 标记 + 短 hash），右列遮蔽预览；还原按钮在预览失败时禁用
- [x] L5.7 明确不做：整仓库 `git reset --hard <commit>`（见本阶段开头硬约束 1）

**依赖**：H4, L4

---

### L6. 提交信息隐私修正

**文件**：`src-tauri/src/cmd/vault/entries.rs`

- [x] L6.1 `finalize` 提交信息去标题化：`vault: create {title}` / `vault: update {title}` → 改为 `{id}`（`delete` 已是 id，无需改；新增 `restore` 同样用 id）
  - 原因：条目标题目前以明文写入 Git 提交信息，托管平台与有读权限者可直接看到；L5 历史面板会把标题成列展示，问题更显眼
  - 落地：`create` / `update` / `restore` 三处改为 `snapshot.id`

**依赖**：无

---

### L7. i18n + 验证

- [x] L7.1 `zh-CN.json` + `en-US.json` 新增：
  - `vault.sync.*`：`clone` / `clone_confirm` / `clone_err_not_empty` / `abort` / `discard` / `discard_confirm` / `remove_remote` / `test_connection` / `test_ok` / `test_fail`
  - `vault.trash.*`：回收站 / 恢复 / 永久清理 / 清理警告
  - `vault.history.*`：历史 / 预览 / 还原 / 还原确认 / `err_old_key`
  - 落地：`vault.history.*` 共 19 条（含 `op_create` / `op_update` / `op_delete` / `op_restore` / `op_other` / `current` / `masked_hint`）
- [x] L7.2 `cargo check` / `cargo test -p assistant-sync` / `tsc --noEmit` 通过
- [ ] L7.3 远程配置闭环：新设备克隆 → 同步成功；本地已有条目时克隆被拦截并提示
- [ ] L7.4 冲突闭环：双端改同一条目 → 冲突提示条出现 → 解决 → 仓库脱离 Merge 态
- [ ] L7.5 中止 / 放弃本地改动：Merge 态可退出；`reset --hard` 前二次确认生效
- [ ] L7.6 回收站：删除 → 回收站可见 → 恢复后 `.enc` 重建 → push 后另一设备可见
- [ ] L7.7 purge：永久清理后 DB 无墓碑行，对应 `.enc` 不存在
- [ ] L7.8 历史还原：改条目 → 还原旧版本 → revision 递增且其他设备收敛到旧内容（验证 `SkippedStale` 不触发）
  - 落地：`history_list_and_restore_revert_entry_to_old_commit` 集成测试已覆盖「列表 → 预览遮蔽 → 还原 → revision 递增」；跨设备收敛需 e2e 手工验证
- [x] L7.9 历史还原边界：改主密码后还原旧 commit → 返回「旧主密码加密」错误且不写库；`RepoState` 时还原被拒绝
  - 落地：`history_restore_rejects_version_encrypted_with_other_master_key`（解密失败返回 `HistoryDecryptFailed` 且 DB 无新增）与 `history_restore_refused_while_repository_in_conflict_state`（Merge 态前置拒绝）两个集成测试覆盖
- [x] L7.10 提交信息不含标题（代码审查 + `git log` 抽查）
  - 落地：全仓库 `vault:` 提交信息仅含 id 与条目数量，无标题

**依赖**：L1 ~ L6
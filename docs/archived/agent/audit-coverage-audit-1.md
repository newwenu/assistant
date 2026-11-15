# 运行时审计覆盖审计（第 1 轮）

| 项 | 值 |
|---|---|
| 状态 | 现行（审计基线，随代码复核更新） |
| 审计日期 | 2026-11-14 |
| 审计对象 | 两条运行时审计链路：① vault 审计（`security_audit_log` + JSONL）；② Agent 工具权限审计（`permission_audit`） |
| 审计范围 | `src-tauri/crates/assistant-vault/src/audit/**`、`src-tauri/crates/assistant-vault/src/security_db/**`、`src-tauri/crates/assistant-vault/src/lifecycle.rs`、`src-tauri/crates/assistant-tools/src/gateway.rs`、`src-tauri/crates/assistant-tools/src/pipeline.rs`、`src-tauri/crates/assistant-tools/src/trait.rs`、`src-tauri/crates/assistant-tools/src/os/**`、`src-tauri/src/cmd/**`、`src-tauri/src/adapters/**`、`src-tauri/src/tool_setup.rs`、`src-tauri/crates/assistant-storage/src/db.rs`、`src/vault/core/**`、`src/vault/*.tsx` |
| 审计方法 | `AuditEvent` 31 个变体逐个反查生产调用点；网关决策分支逐个核对 `audit()` 调用；`#[command]` 全量扫描按「是否写审计」分类；`permission_intents` 实现逐个比对工具注册表；前端 `queryAuditLog` 消费点反查；对照 I9-vault-audit-spec（已归档）§3 落点表逐行验证 |
| 证据基准 | 只采当前工作区源码。I9-vault-audit-spec（已归档）只作为「**应当**覆盖什么」的对照基线，不作为现状依据 |
| 引用口径 | 定位优先用 `文件::符号` 锚点（如 `assistant-vault/src/audit/mod.rs::AuditEvent`）；行号会随迭代漂移，仅作本次审计时的参考 |
| 修订记录 | 2026-11-14 首轮：建立覆盖矩阵，登记缺口 G1–G9、观察项 O1–O5、有意例外 E1–E4 |
| 复核记录 | 2026-11-14 复核（逐条回查代码）：新增 G10（仅 JSONL 事件永不入库）、O6（查询能力受限）、O7（JSONL 并发追加无锁）；修正 G4 的误判（`vault_resolve_conflict` 实际经条目层写 `VaultEntryUpdated` / `VaultEntryCreated`，缺口实际在 `upsert_imported_in` 的 `Updated` 分支与远端配置变更）；补 G1 遗漏的 3 处 `InvalidPassword` 分支、G5 遗漏的 `ssh_remove_all_on_lock`；§2.1「三个写入原语」更正为「四个」 |

---

## 0 结论摘要

1. **审计不是一个功能，是两条互不相通的链路**：vault 审计写 `security.db` 加密表 `security_audit_log`（未解锁/恢复态另写明文 JSONL）；Agent 工具权限审计写主库 `permission_audit`。两条链路的存储、事件词表、失败降级策略都不同，不能按同一口径评估。
2. vault 侧 `AuditEvent` 共 **31 个变体，其中 29 个有生产调用点**；未使用的是 `VaultUnlockFailed` 与 `LocalDbRepaired`（G1 / G2）。
3. 工具权限侧网关的 **8 种 `AuditOutcome` 决策分支全部有审计**，覆盖完整；真正的缺口不在分支，而在**分支之前**——`permission_intents` 为空时网关在策略求值前就返回 `Allowed`，且不写审计（G6）。
4. 10 项缺口按严重度：**高 3 项（G1 / G4 / G6）**、中 5 项（G3 / G5 / G7 / G9 / G10）、低 2 项（G2 / G8）。
5. 最值得先修的三条：**解锁失败无审计**（口令爆破在审计里完全不可见）、**只读工具零审计**（Agent 读了什么文件无痕，`hard_deny_rules` 里的敏感路径 `read` 规则形同虚设）、**仅 JSONL 的事件永不入库**（G10：`BACKFILL_EVENTS` 白名单漏了 `vault_unsupported_version` / `biometric_failed`，也是 G1 补审计能否被看见的前提）。
6. 有一条结构性事实影响所有「补审计」的判断：`audit::log(conn, ...)` 是 `log_dual(Some(conn), None, ...)` 的别名，**只写 DB 不写 JSONL**。条目类事件（created / updated / deleted …）全部走这条，属于「解锁态 DB-only」的**有意设计**，不是遗漏（见 E2）。

---

## 1 判定口径

一个运行时动作算「已纳入审计」需**同时**满足三条：

| 维度 | 要求 | 反例 |
|---|---|---|
| 载体 | 有稳定事件标识：vault 侧是 `AuditEvent` 变体（`Display` 输出蛇形字符串），工具侧是 `AuditEntry{tool,action,resource,outcome}` | 只在 `log::warn!` 里写自由文本 |
| 调用点 | 在生产代码路径上（非 `#[cfg(test)]`、非测试文件）有实际调用 | 变体定义了但零调用点（死事件） |
| 可检索 | 落库/落文件后能被事后查询 | 写了但无查询入口（见 G9）；写了 JSONL 但不在补录白名单，永远进不了库（见 G10） |

补充口径：

- **JSONL 不是「备份」，是「库不可用时的唯一落点」**。凡在「库必然可用」的路径上只写 DB（`audit::log`），不算缺口；凡在「库不可用」的路径上没写 JSONL，才算缺口。
- **审计失败不阻断业务**是既定策略（`gateway.rs::audit` 失败只 `warn`，`jsonl.rs::log_dual` 的 JSONL 失败只 `warn`）。本审计不把「审计失败被吞」计为缺口，只记为观察项（O4）。
- **审计 detail 禁止承载敏感值**：导出/导入路径用 `path_digest`（SHA-256 小写 hex），口令、明文路径、密钥材料一律不入 detail。本审计不重复验证脱敏（由 `assistant-vault/tests/audit.rs::t_a9_no_secrets_in_audit`、`t_a10_export_detail_uses_path_digest` 守住）。

---

## 2 两条链路的现状

### 2.1 vault 审计

**四个写入原语**（`assistant-vault/src/security_db/mod.rs`、`assistant-vault/src/audit/mod.rs`）：

| 原语 | 语义 | 使用场景 |
|---|---|---|
| `audit::log(conn, event, detail)` | `log_dual(Some(conn), None, ...)` → **仅 DB** | 解锁态下的条目/明文出口事件 |
| `SecurityDbState::audit_dual(event, detail)` | 有 session 就 DB+JSONL，无 session 就仅 JSONL | 生命周期、生物识别 |
| `SecurityDbState::audit_jsonl(event, detail)` | 仅 JSONL | 恢复通道（无 conn）、重置后 |
| `audit::log_dual(conn, writer, ...)` | 两路都试，任一路成功即 `Ok` | 需要显式指定连接的分支（unlock / lock / 锚点回滚） |

**存储**：`security_audit_log(id, event_type, detail, timestamp)` 在加密库内；JSONL 落在 `security/audit/audit-YYYYMMDD[-n].jsonl`，单文件 8 MiB 顺延，`detail` 恒在（缺失为 `null`）。

**补录**：`SecurityDbState::unlock` 成功后读 JSONL 中 `at > audit_jsonl_synced_at` 的记录，只补 `BACKFILL_EVENTS` 白名单（`recovery_export_used` / `dek_rotated` / `local_db_rebuilt` / `vault_repository_reset`），去重键 `jsonl-{at}-{event}`，失败只 `warn` 不阻断解锁。

白名单口径本身有偏差：`dek_rotated` / `local_db_rebuilt` 本就是**双写**（DB 侧已有行，补录只会被 `(event, timestamp)` 去重挡掉，属冗余项），而真正**只写 JSONL** 的 `vault_unsupported_version`（`cmd/vault/lifecycle.rs::vault_unlock` 版本拒绝分支）与 `biometric_failed`（`security_db/biometric_ops.rs::audit_failure_jsonl` 恒传 `conn=None`）**不在白名单**，因此永不入库 → G10。

### 2.2 工具权限审计

`permission_audit(id, tool, action, resource, outcome, at_millis)` 在主库，`DbPermissionAuditStore::log_decision` 只追加不更新、无 TTL（`src-tauri/src/adapters/permission_store.rs`）。

网关 `ToolGateway::evaluate` 的决策分支与审计的对应关系（`assistant-tools/src/gateway.rs`）：

| 决策 | outcome | 是否写审计 |
|---|---|---|
| tier 硬上限拒绝 | `Denied`（`action="*"`，`resource="tier:{tier}"`） | 是 |
| `permission_intents` 为空 → 直接 `Allowed` | — | **否（G6）** |
| 命中已授权 | `GrantHit` | 是 |
| 策略 `Allow` | `Allowed` | 是 |
| 策略 `Deny` | `Denied` | 是 |
| 审批通过（单次 / 会话 / 永久） | `Approved` / `SessionApproved` / `AlwaysApproved` | 是 |
| 用户拒绝 | `Rejected` | 是 |
| 审批超时 | `TimedOut` | 是 |

另有**不经网关**的一条写入：`src-tauri/src/cmd/conversation.rs` 切换会话工作区时直插 `permission_audit`（`tool="workspace"`、`action="switch"`、`resource` 为工作区或 `"none"`、`outcome="allowed"`）。

---

## 3 已覆盖矩阵

### 3.1 vault 域（29 / 31）

| 域 | 事件 | 落点（`文件::符号`） | 写入方式 |
|---|---|---|---|
| 会话 | `VaultUnlock` | `security_db/mod.rs::SecurityDbState::unlock` | 双写 |
| 会话 | `VaultLock` | `security_db/mod.rs::SecurityDbState::lock_session`（detail = `LockReason::as_str`） | 双写 |
| 会话 | `AnchorRollbackRejected` | `security_db/mod.rs::SecurityDbState::unlock` 的锚点比对失败分支 | 双写 |
| 会话 | `VaultUnsupportedVersion` | `cmd/vault/lifecycle.rs::vault_unlock` 版本拒绝分支 | 仅 JSONL |
| 条目 | `VaultEntryCreated` | `security_db/entries/write.rs::create_entry_in`、`entries/import.rs` | 仅 DB |
| 条目 | `VaultEntryUpdated` | `entries/write.rs::update_entry_in` / `set_favorite_in`、`entries/import.rs` | 仅 DB |
| 条目 | `VaultEntryDeleted` | `entries/write.rs::delete_entry_in` / `delete_entries_tx` | 仅 DB |
| 条目 | `VaultEntryRestored` | `entries/deleted.rs::restore_*` | 仅 DB |
| 条目 | `VaultEntryPurged` | `entries/deleted.rs::purge_*` | 仅 DB |
| 明文出口 | `VaultReveal` / `VaultRevealRateLimited` | `cmd/vault/reveal.rs`、`cmd/vault/sync.rs`（冲突字段） | 仅 DB |
| 明文出口 | `VaultCopyPassword` / `VaultCopyField` | `cmd/vault/reveal.rs`、`cmd/vault/audit.rs::vault_copy_totp` | 仅 DB |
| 明文出口 | `VaultEntriesExported` / `VaultEntriesImported` | `cmd/vault/entries.rs`（detail `{format}:{count}:{pathHash}`） | 仅 DB |
| HIBP | `HibpAudit` / `HibpLeakFound` | `cmd/vault/audit.rs::vault_hibp_audit` / `vault_hibp_check` | 仅 DB |
| 密钥 | `MasterPasswordChange` | `security_db/rekey.rs::change_master_password` | 仅 DB |
| 密钥 | `DekRotated` | `lifecycle.rs::rotate_dek` | 双写 |
| 密钥 | `RecoveryKeyExported` | `lifecycle.rs::export_recovery_key` | 双写 |
| 密钥 | `RecoveryExportUsed` | `cmd/vault/recovery.rs` | 仅 JSONL |
| 密钥 | `LocalDbRebuilt` | `security_db/rekey.rs::rebuild_local_db` | 双写 |
| 密钥 | `VaultRepositoryReset` | `lifecycle.rs::reset_repository`（删除磁盘后写） | 仅 JSONL |
| 生物识别 | `BiometricEnrolled` / `BiometricUnlocked` / `BiometricRevoked` | `security_db/biometric_ops.rs` | 双写 |
| 生物识别 | `BiometricFailed` | `security_db/biometric_ops.rs::audit_failure_jsonl`（detail 取封闭集合 reason） | **仅 JSONL**（恒传 `conn=None`，非「视 conn 可用性」）→ G10 |
| SSH | `SshKeyLoaded` / `SshKeyRemoved` | `cmd/vault/ssh.rs::vault_ssh_add_to_agent` / `vault_ssh_remove_from_agent`（**仅手动路径**，自动路径见 G5） | 仅 DB |

**未使用变体**：`VaultUnlockFailed`（G1）、`LocalDbRepaired`（G2）。

### 3.2 工具权限域

| 覆盖对象 | 现状 |
|---|---|
| 有 `permission_intents` 的 6 个工具：`link` / `write` / `edit` / `mkdir` / `move` / `delete` | 全路径审计（8 种 outcome） |
| 工作区切换 | `conversation.rs` 直插一行 |
| 无 `permission_intents` 的 5 个工具：`read` / `ls` / `glob` / `grep` / `ask_user` | **零审计**（G6）；另有 `link` 的 `action="check"` 返回空 intent，同样零审计 |
| 工具执行阶段（Execute）的结果 | 不写 `permission_audit`（结果落 `messages.tool_calls_json` + `role='tool'` 行，属消息侧持久化，不在本审计口径内） |

---

## 4 缺口清单

编号沿用 `G`（gap），优先级为**建议**排序依据。

### G1 解锁失败完全无审计 —— 高

**现象**：`AuditEvent::VaultUnlockFailed` 有定义、有 `Display`（`vault_unlock_failed`），但全仓库**零调用点**。口令错误时 `cmd/vault/lifecycle.rs::vault_unlock` 走 `Err(VaultError::InvalidPassword)` 分支，只有 `log::warn!("Vault unlock failed: invalid password")`。

**证据**：口令错误分支共 **3 处**，全部只有 `log::warn!`、无审计：

| 位置 | 日志 |
|---|---|
| `cmd/vault/lifecycle.rs::vault_unlock` | `Vault unlock failed: invalid password` |
| `cmd/vault/lifecycle.rs::vault_show_unlock_window`（原生解锁窗） | `Native unlock dialog: invalid password` |
| `cmd/vault/lifecycle.rs::vault_rebuild_local_db` | `Local db rebuild failed: invalid password` |

这三处都是 `Ok` / `InvalidPassword` / 其他 三分支结构，只有「其他」分支在版本拒绝时写 JSONL。`SecurityDbState::unlock` 的 `probe_read` 失败、`anchor.ok_or(VaultRollbackDetected)` 失败分支同样直接返回（前者 conn 已打开，可写 DB；后者只写 `AnchorRollbackRejected`，无法区分「锚点缺失」与「锚点不符」）。

**影响**：口令爆破、撞库、设备被反复试探在审计里**完全不可见**。这是审计最该有的一条，也是唯一「攻击行为发生但没有痕迹」的场景。

**建议修法**：三层各补一处，`AuditEvent` 已有变体、不需要改枚举：

1. 命令层 3 个 `InvalidPassword` 分支：`state.audit_jsonl(VaultUnlockFailed, Some("invalid_password"))`（此时库未打开，只能落 JSONL）。
2. `SecurityDbState::unlock` 的锚点缺失/比对失败分支（conn 已打开）：改用 `log_dual` 写 `VaultUnlockFailed`，detail 区分 `anchor_missing` / `meta_or_dek_version`（与既有 `AnchorRollbackRejected` 并存，语义不同：后者是回滚拒绝，前者是失败归因）；`probe_read` 失败同样补 `probe_failed`。
3. **配套必做**：把 `vault_unlock_failed` 加入 `BACKFILL_EVENTS`。否则按第 1 条补的 JSONL 行永远进不了库，`vault_query_audit_log` 看不到——「补了等于没补」（见 G10）。

**注意**：detail 必须是封闭集合，禁止写入口令片段、口令长度等可辅助猜测的信息；建议把 reason 收敛为 `invalid_password` / `probe_failed` / `anchor_missing` / `meta_or_dek_version`。

### G2 `LocalDbRepaired` 是死事件 —— 低

**现象**：`AuditEvent::LocalDbRepaired`（`local_db_repaired`）零调用点，实际只有 `LocalDbRebuilt`（`security_db/rekey.rs::rebuild_local_db`）。I9-vault-audit-spec（已归档）§3.2 把「修复」与「重建」并列为 `rekey.rs` 的落点。

**影响**：规格要求的「修复」语义无法与「重建」区分；同时违反同规格 §6 第 6 条「禁止新增事件只改 `Display` 而不加 `AuditEvent` 变体」的反向情形——**变体存在但永不产生**，会误导后续读者以为该路径已覆盖。

**建议修法**：二选一，不要留悬空变体。① 若 `rekey.rs` 确实存在「保留数据修复」与「清库重建」两条路径，各写各的事件；② 若只有重建一条路径，删除 `LocalDbRepaired` 变体与 `Display` 分支，并同步从规格落点表中移除。

### G3 初始化无任何审计事件 —— 中

**现象**：`vault_init` 全链路（`cmd/vault/lifecycle.rs::vault_init` → `lifecycle.rs::initialize` → `SecurityDbState::init`）**不写任何审计**；`AuditEvent` 也没有 `VaultInitialized` 之类的变体。

**证据**：`lifecycle.rs::initialize` 与 `SecurityDbState::init` 内无 `audit_*` 调用。`SecurityDbState::init` 成功后会立即建立 session（`session.kek = kek` 后写入 `self.session`），因此**初始化完成时库是可用的**，可直接 `audit_dual`。

**影响**：「保险库何时、由谁首次创建」无痕；与 `VaultRepositoryReset`（有审计）不对称——重置有记录、创建没有。

**建议修法**：新增 `VaultInitialized` 变体（蛇形 `vault_initialized`），在 `initialize` 末尾 `audit_dual`，detail 可带 `dek_version`。**不要**复用 `VaultUnlock`：语义不同，且会污染「解锁次数」类统计。

### G4 同步域整体无审计 —— 高

**现象**：同步命令层**不写任何 vault 审计**；其中 Pull 覆盖已有条目时，条目层也没有留痕。

| 命令 | 现状 |
|---|---|
| `vault_sync`（Pull / Push / Sync / ForcePush） | 失败只 `log::warn!`；成功不写审计。**Pull 覆盖已有条目**走 `entries/import.rs::upsert_imported_in` 的 `Updated` 分支，该分支**不写 `VaultEntryUpdated`**（只有 `Inserted` 分支写 `VaultEntryCreated`）→ 本条是缺口里最隐蔽的一处 |
| `vault_set_sync_config` | 无审计（含远端 URL 变更，且 `persist_sync_remote_url` 会把 URL 落到 conf） |
| `vault_git_init` / `vault_git_remote` / `vault_git_remove_remote` | 无审计（clone 会覆盖本地仓库、add/set 改数据流向、remove 删远端） |
| `vault_resolve_conflict` | **已覆盖，不计缺口**：UseRemote → `sync_engine/history.rs::import_remote_entry` → `entries/import.rs::force_import_entry_tx` 写 `VaultEntryUpdated`；MergeBoth → `create_remote_copy_tx` 写 `VaultEntryCreated`；UseLocal 只导出 git、不改库。缺的只是 detail 里没有 `conflict:{side}` 标记（见 O5） |
| `vault_sync_confirm_removals` | **已覆盖**：经 `delete_entries_tx` 写 `VaultEntryDeleted`（detail=`remote`） |
| `vault_sync_pending_removals` / `vault_sync_discard` | 纯查询 / 只 `git reset --hard` 不动库，不计缺口 |

**证据**：`cmd/vault/sync.rs` 内对 `audit_log` 的使用仅 2 处，且都在「冲突字段被 reveal」路径上（`VaultReveal` / `VaultRevealRateLimited`），**不是同步行为本身的审计**。另需注意「命令层不写 ≠ 全无痕」：条目层的写入原语（`delete_entries_tx` / `force_import_entry_tx` / `create_remote_copy_tx`）会自动留痕，因此本缺口的真实范围是「同步动作本身 + 远端配置变更 + Pull 覆盖已有条目」这三块，而不是「所有同步命令」。

**影响**：`ForcePush` 是覆盖远端的破坏性操作，属于最需要留痕的一类，却完全无痕；**Pull 覆盖本地已有条目同样无痕**，且它看起来像「同步成功」，事后无法区分「这条记录是用户改的」还是「远端推来的」；远端 URL 变更是潜在的「数据流向被改」信号，也无痕。I9-vault-audit-spec（已归档）§3.4 要求的「冲突解决复用现有事件 + detail 带 `conflict`」「`VaultManifestMismatch`（可选）」均未落地。

**建议修法**（按最小改动排序）：

1. `vault_sync`：新增 `VaultSyncCompleted` / `VaultSyncFailed` 两个变体，detail 用封闭集合 `{action}:{result}`（如 `force_push:success`、`pull:auth_failed`），**禁止**把远端 URL 或 commit hash 明文入 detail（URL 走 `path_digest` 同款摘要）。
2. 远端配置类命令（`vault_set_sync_config` / `vault_git_remote` / `vault_git_remove_remote` / `vault_git_init`）：统一新增 `VaultSyncConfigChanged`，detail 只记动作 + 远端名，不记 URL。
3. `entries/import.rs::upsert_imported_in` 的 `Updated` 分支补 `VaultEntryUpdated`（与 `force_import_entry_tx` 对齐），detail 带 `sync:` 前缀，以便把「同步覆盖」与「用户编辑」区分开。
4. 冲突解决若要进一步区分，复用条目侧既有事件并在 detail 追加 `conflict:{side}`（与 O5 一并修）。
5. 可选：pull 失败分支补 `VaultManifestMismatch`。

### G5 SSH 自动注入 / 自动移除无审计 —— 中

**现象**：只有**手动** add/remove 写 `SshKeyLoaded` / `SshKeyRemoved`。自动路径全无审计：

| 函数 | 触发时机 |
|---|---|
| `cmd/vault/ssh.rs::vault_ssh_auto_add_all` | 用户显式点「全部加入 agent」 |
| `cmd/vault/ssh.rs::auto_add_entry` | 新建/更新 SSH 密钥条目后自动注入 |
| `cmd/vault/ssh.rs::auto_remove_fingerprint` | 删除/轮换密钥后自动移除 |
| `cmd/vault/mod.rs::ssh_remove_all_on_lock` | 锁库时（`on_vault_locked` 调用）批量移除 |

另：`ssh_auto_add_on_unlock`（`cmd/vault/mod.rs`）被 **4 处**解锁成功分支调用——`vault_unlock`、`vault_show_unlock_window`、`vault_rebuild_local_db`、`vault_unlock_biometric`，即**每次解锁都可能批量注入全部 SSH 私钥**，且无审计。

**影响**：解锁后自动把全部 SSH 私钥灌进 ssh-agent，是「批量密钥暴露」行为，却是审计盲区；事后无法回答「这把 key 是什么时候被 agent 加载的」。

**建议修法**：四处均补 `SshKeyLoaded` / `SshKeyRemoved`，detail 用 `auto:{entryId}` 前缀与手动路径区分（既有手动路径 detail 是裸 `id` / `fingerprint`）。若担心事件量，可在 `auto_add_entry` 与 `auto_remove_fingerprint` 用同一事件 + `auto:` 前缀，批量路径汇总为一行 `auto_all:{count}`。**注意锁库路径**：`ssh_remove_all_on_lock` 执行时 session 已关闭、conn 不可用，只能写 JSONL；而 `SshKeyRemoved` 不在 `BACKFILL_EVENTS`，因此该行会永久不可查（与 G10 同源，需一并加白名单）。

### G6 只读工具与 `ask_user` 无 `permission_intents` → 网关短路且不写审计 —— 高

**现象**：`ToolGateway::evaluate` 在 `let intents = tool.permission_intents(...)` 后紧接 `if intents.is_empty() { return Ok(GatewayDecision::Allowed); }`。该分支**既不求值策略、也不写审计**。

**证据**：全仓库 `fn permission_intents` 仅 6 个实现（`fs_link` / `agent_write` / `agent_edit` / `agent_mkdir` / `agent_move` / `agent_delete`）；注册的 11 个工具中 `read` / `ls` / `glob` / `grep` / `ask_user` 均未实现（`assistant-tools/src/os/mod.rs::register_readonly_tools` / `register_interactive_tools`）。另有第 6 条零审计路径：`fs_link` 的 `action="check"` 在 `permission_intents` 里直接 `return Ok(vec![])`（`os/fs_link/mod.rs::permission_intents`），命中同一短路——它本质也是只读探测。

**影响**：三重后果，都是安全向的：

1. **读操作零痕迹**：Agent 读过哪些文件、哪些目录被枚举、grep 过什么关键字，审计里都没有。
2. **`hard_deny_rules` 的 `read` 规则不生效**：`policy.rs::hard_deny_rules()` 里针对系统目录的 `read` 拒绝规则依赖 `policy.evaluate(action, resource)` 求值，而空 intent 在求值前就返回，规则**永远不会被求值**。当前 `read` 类硬拒绝只有 2 条且都是 Unix 路径（`/etc/shadow`、`/etc/ssh/**`）；Windows 侧只有 `edit` 类（`C:/Windows/**`、`C:/Program Files/**`），**没有任何 `read` 硬拒绝**——即修好 G6 也只能让这 2 条 Unix 规则生效，Windows 的「读取系统目录」仍无兜底规则，属规格层面的空白，需单独评估。
3. **tier 硬上限仍生效**：注意 `evaluate_tier_ceiling` 在 intent 判定**之前**执行，所以只读工具的 tier 越界仍会被拦并写 `Denied`——这条不是缺口（避免误判）。

**建议修法**：给 4 个只读工具实现 `permission_intents`，返回**声明式 intent**（`read` / `list` / `glob` / `grep` 各一），并让策略对这些 action 默认 `Allow`（保持「只读不打扰」的既有体验）。这样网关会走完整求值 → 命中 `Allow` → 写 `Allowed` 审计，同时 `hard_deny_rules` 生效。`ask_user` 建议单独评估：它不是权限动作，若要审计应新增事件语义（如「提问次数」），不要塞进权限 outcome。

**注意**：此改动会让只读工具调用**每调用一行** `permission_audit`，需要评估表增长速率（见 O3）。若量级不可接受，替代方案是「只读工具只在 `Deny` / `hard_deny` 命中时写审计」，但那要求网关在空 intent 时也求值策略——即把短路改成「无 intent 也走一次策略求值」，实现更小、审计更省。

### G7 Validate 阶段拒绝无审计 —— 中

**现象**：`ToolExecutionPipeline` 的顺序是 `find → validate → gateway → execute`。`validate_input` 失败时直接构造 `ToolOutcome`（`PipelineStage::Validate`）返回，**不经过网关、不写审计**。只有 `GatewayDecision::Denied` 分支才有审计。

**证据**：`assistant-tools/src/pipeline.rs` 的 `!validation.ok` 分支直接 `return outcome`；`Denied` 分支的审计由网关侧写入，pipeline 本身不写。

**证据补充**：`agent_delete.rs::AgentDeleteTool::validate_input` 内部调用 `DeleteRequest::parse` + `resolve_target`，而 `resolve_target` 会经 `os/path.rs::reject_system_dirs` 拒绝系统目录、`.ssh`、`.aws/credentials`、keychain 等目标。也就是说**最敏感的越界拒绝恰好发生在无审计的 Validate 阶段**。

**同源的第二条无审计分支**：`ToolGateway::evaluate` 里 `tool.permission_intents(input, context)?` 的 `?` 会把 intent 计算失败直接冒泡成 `Err`，pipeline 侧归为 `PipelineStage::Gateway` 的 `Internal` 错误——**这条路径也没有任何审计**（`evaluate` 在 `intents` 拿到之前就返回了）。`delete` / `write` 等工具的 `permission_intents` 内部同样做 `resolve_target`，因此与 Validate 阶段的失败是同一类「路径解析被拒」。

**影响**：被输入校验挡下的调用（典型如 `delete` 对系统目录的拒绝、`write` 对路径越界的拒绝）不落 `permission_audit`。这类拒绝恰恰是「越界尝试」的信号，属于最该审计的一类。

**建议修法**：两处各补一次 `PermissionAuditStore::log_decision`：

1. pipeline 的 `!validation.ok` 分支：`outcome` 复用 `Denied`（或新增 `ValidateRejected`），`action` 取工具 id，`resource` 取校验错误码（封闭集合）。
2. gateway 的 `permission_intents` 失败分支：同样记 `Denied`，`resource` 用固定串 `intent_error`（错误文本可能含路径，**不得**入 detail）。

实现上建议在网关暴露一个「代写审计」的方法（如 `ToolGateway::audit_decision`）供 pipeline 调用，避免把 `audit_store` 再注入 pipeline（当前 pipeline 只持有 `gateway`）。

### G8 ToolBox 路径绕过网关 —— 低

**现象**：`cmd/os_tool_exec.rs::os_tool_exec` 用 `ToolUseContext::for_toolbox(None)` 直接 `invoke(...)`，**不经过 `ToolGateway`、不写 `permission_audit`**。

**证据**：`os_tool_exec` 只做 `find_for_toolbox` + `invoke`，无 `evaluate` 调用。

**影响**：UI / 用户直接触发的 OS 工具操作是审计盲区。设计上 ToolBox「无会话、无基准、无边界」（I38h-workspace-spec（已归档）§4.2），因此没有权限决策可审计——但「用户通过 UI 执行了 OS 工具」这一事实本身仍值得留痕（属于用户操作审计，不是权限审计）。

**建议修法**：先确认产品意图。若 ToolBox 明确不在审计口径内，在本文 E 类登记为有意例外；若要覆盖，建议新增独立审计语义（`tool="toolbox"`、`action` = 工具名、`outcome="invoked"`），**不要**伪造权限 outcome，避免污染授权命中率统计。

### G9 审计只写不读 —— 中

**现象**：

| 侧 | 后端 | 前端 |
|---|---|---|
| vault | `vault_query_audit_log` 存在（`cmd/vault/audit.rs::vault_query_audit_log`） | `queryAuditLog` 只到 service / slice（`src/vault/core/vault.service.ts`、`src/vault/core/slices/vaultAuditSlice.ts`），**无任何组件消费**（`src/vault` 下 `.tsx` 中出现的 `audit` 全部是 HIBP 审计，与审计日志无关） |
| 工具权限 | **无查询命令**（`src-tauri/src/cmd/permission.rs` 只有 grants 的 list / revoke / set_scope / clear） | 无 |

**影响**：审计只写不读，等于「证据无法被举证」。用户与开发者都无法回答「上次改密是什么时候」「这个工具被拒绝过几次」。

**查询能力本身也受限**（即使补了前端面板）：`vault_query_audit_log(app, limit)` 只接受 `limit`（默认 100），无 `since` / `event_type` 过滤、无分页，且经 `with_unlocked` 包装（锁定态直接报 `NotInitialized`）；JSONL 侧完全没有读取命令。表上已有 `idx_audit_log_timestamp` / `idx_audit_log_event_type`，加过滤参数的成本很低（见 O6）。

**建议修法**：

1. 前端补一个审计日志面板（vault 设置页下，消费既有 `queryAuditLog`，无需新命令）。
2. 工具权限侧新增 `list_permission_audit(limit, since)` 命令 + DTO，遵循 DTO 契约流程（`npm run contract:gen`）。注意 `permission_audit` 表无 TTL 会持续增长（O3），查询必须带 `limit`。

### G10 仅 JSONL 的事件永不入库 → 永不可查 —— 中

**现象**：`BACKFILL_EVENTS` 白名单只有 4 项，且组成失衡：

| 事件 | 实际写入方式 | 是否在白名单 | 结果 |
|---|---|---|---|
| `recovery_export_used` | 仅 JSONL | 是 | 解锁后补录入库，可查 |
| `vault_repository_reset` | 仅 JSONL | 是 | 解锁后补录入库，可查 |
| `dek_rotated` / `local_db_rebuilt` | **双写**（DB 已有行） | 是 | 冗余：补录必被 `(event, timestamp)` 去重挡掉 |
| `vault_unsupported_version` | 仅 JSONL | **否** | 永不入库，永不可查 |
| `biometric_failed` | 仅 JSONL（`audit_failure_jsonl` 恒传 `conn=None`） | **否** | 永不入库，永不可查 |

**证据**：`security_db/mod.rs::BACKFILL_EVENTS` 常量仅含前 4 项；补录逻辑 `try_backfill_jsonl_audit` 用 `record.event.as_str()` 做白名单过滤，未命中即跳过。唯一查询入口 `cmd/vault/audit.rs::vault_query_audit_log` → `audit/mod.rs::query_log` 只查 `security_audit_log`，JSONL 无任何读取命令。

**影响**：

1. **两类安全事件永久不可查**：`vault_unsupported_version`（用异常版本客户端试探）与 `biometric_failed`（`payload_len` / `stale_payload` / `probe_failed` / `delete_failed`，凭据异常信号）写进了文件，却没有任何 API / UI 能读到。
2. **与 G1 修法直接冲突**：按 G1 建议在 `vault_unlock` 的 `InvalidPassword` 分支用 `audit_jsonl` 写 `vault_unlock_failed`，若不加入白名单，补的审计同样永远查不到——「补了等于没补」。同理 G5 的锁库移除路径。
3. 本条是 §8 自检清单第 ③ 条的**存量违规**：「若产生于『库不可用』路径 → 必须写 JSONL **且加入 `BACKFILL_EVENTS`**」，现有代码只做到了前半句。

**建议修法**：

1. 把 `vault_unsupported_version`、`biometric_failed` 加入 `BACKFILL_EVENTS`；`dek_rotated` / `local_db_rebuilt` 可保留（兼容旧版本仅写 JSONL 的历史数据），但需在注释里说明「这两项是双写冗余，保留只为向后兼容」。
2. 把「仅 JSONL ⇒ 必须入白名单」写进 §8 自检清单并作为评审硬条件（已有 ③ 条，需真正执行）。
3. 若产品要求「锁定态也能查审计」，需新增 JSONL 读取命令，与 G9 一并做。

---

## 5 观察项（不计为缺口，但需知情）

| 编号 | 观察 | 说明 |
|---|---|---|
| O1 | **重置仓库会连带销毁历史审计** | `cmd/vault/lifecycle.rs::vault_reset_repository` 的删除目标包含 `security.db` 与 `security/audit` 目录；`lifecycle.rs::reset_repository` 在删除**之后**才写 `VaultRepositoryReset`，因此只剩「重置」这一条记录，此前全部审计（DB 与 JSONL）不可恢复。若产品要求「审计不可被一次操作抹除」，需把审计目录移出删除目标或在重置前外置归档 |
| O2 | **JSONL 明文在 Windows 无 ACL 收紧** | `audit/jsonl.rs::restrict_permissions` 仅在 `#[cfg(unix)]` 下设 0600，非 unix 为空实现。当前依赖 detail 不含敏感值（有测试守）+ 文件位于用户私有目录，风险可接受；若日后 detail 扩展，需先补 Windows ACL |
| O3 | **`permission_audit` 只增不减** | 表无 TTL、无归档、无索引（`assistant-storage/src/db.rs` 建表段只有主键）。G6 修好后只读工具会显著增加写入量，需先评估增长率，再决定是否加 `at_millis` 索引 / 保留期 |
| O4 | **审计失败静默** | 网关 `audit()` 失败只 `warn`；JSONL 失败只 `warn`。策略本身正确（审计是观测不是事实源），但没有任何计数或告警，审计链路整体断裂时无人感知。建议至少加「连续失败计数」并在设置页暴露 |
| O5 | **冲突 reveal 的 detail 用条目路径而非 entryId** | `cmd/vault/sync.rs::vault_conflict_reveal` 写 `VaultReveal` 时 detail 为 `conflict:{path}:{side}:{field}`，而 I9-vault-audit-spec（已归档）§3.5 规定 `conflict:{entryId}:{side}:{field}`。语义上「按路径查条目」比「按 id」更贴近冲突场景（冲突上下文只有 path），但两处口径不一致会让按 entryId 聚合的查询漏掉冲突行。修法：统一为 `conflict:{entryId}:{side}:{field}`，或在规格侧承认 path 口径 |
| O6 | **vault 审计查询只有 `limit` 一个参数** | `audit/mod.rs::query_log(conn, limit)` 固定 `ORDER BY timestamp DESC LIMIT ?1`，无 `since` / `event_type` 过滤、无游标分页；`vault_query_audit_log` 默认 `limit=100`。数据量大时只能「拿最近 N 条」再在内存里筛，且锁定态完全查不到（`with_unlocked`）。表上已有 `idx_audit_log_timestamp` / `idx_audit_log_event_type`，补 `since` + `event_type` 参数即可用上索引，成本低 |
| O7 | **JSONL 并发追加无锁、轮转判断有竞态** | `audit/jsonl.rs::AuditJsonlWriter` 只持目录路径，每次 `append_at` 都 `create_dir_all` + `OpenOptions::append` 打开；`active_file_path` 按 `metadata.len() < 8 MiB` 选文件。多个 `spawn_blocking` 线程（解锁 / 锁库 / 生物识别路径）并发写时，轮转判断与写入不是原子组合，极端情况下同刻并发可能写入同一文件或触发一次打开失败（失败只 `warn`）。后果是**丢行而非错行**（每行单次 `write_all`），风险低；若审计量提升，需加进程内 `Mutex` 或改为单写线程 |

---

## 6 有意例外（明确不列为缺口）

| 编号 | 例外 | 理由 |
|---|---|---|
| E1 | 工具**执行结果**不写 `permission_audit` | 结果落 `messages.tool_calls_json` + 独立 `role='tool'` 行，属于消息侧持久化；权限审计只记「决策」，不记「产出」 |
| E2 | 条目类事件只写 DB（`audit::log`） | 这些事件只可能发生在解锁态（库必然可用），JSONL 的存在意义是「库不可用时的落点」，故 DB-only 是设计而非遗漏 |
| E3 | 审批超时（`TimedOut`）不视为拒绝 | 网关按 `TimedOut` 单独记 outcome，语义上与 `Rejected` 区分，属正确设计 |
| E4 | `VaultUnsupportedVersion` 只写 JSONL | 版本拒绝时库尚未打开，无 conn 可用；JSONL 是唯一落点，符合 I9-vault-audit-spec（已归档）§3.2 |

---

## 7 建议落地顺序

| 批次 | 内容 | 理由 |
|---|---|---|
| P0 | G1（解锁失败）+ G6（只读工具 intent 或「空 intent 也求值」）+ G10（白名单补 2 项） | G1 / G6 是「攻击行为无痕」的缺口；G10 只是常量里加两行，却是 G1 / G5 补的 JSONL 审计能被查到的前提；G6 同时修复 `hard_deny_rules` 失效 |
| P1 | G4（同步域）+ G3（初始化） | 破坏性操作（ForcePush）与仓库创建无痕；改动集中在命令层与 `upsert_imported_in`，风险低 |
| P2 | G7（Validate 拒绝）+ G5（SSH 自动）+ G9（查询入口）+ O6（查询参数） | G9 是前几项的前提：不补查询入口，补的审计无人可见；O6 与面板一起做才不用二次改接口 |
| P3 | G2（死事件收敛）+ G8（ToolBox 口径确认）+ O1–O7 评估 | 语义收敛与风险知情，不影响功能 |

---

## 8 复核方法

本节给出**可执行**的复核命令，避免下一轮复核时先怀疑文档本身。命令在仓库根目录执行，Windows PowerShell 下用 `rg`；若本机没有 `rg`，把 `rg -n "<pat>" <path>` 换成 `Get-ChildItem <path> -Recurse -Include *.rs | Select-String -Pattern "<pat>"`，结论口径不变。

1. **事件变体总数与死事件**：对比 `AuditEvent` 变体清单与生产调用点。

```powershell
rg -n "enum AuditEvent" -A 40 src-tauri/crates/assistant-vault/src/audit/mod.rs
rg -n "AuditEvent::" src-tauri --glob "!*tests*" --glob "!*/tests/*"
```

判定：`rg` 结果中**不出现**的变体即为死事件（当前应为 `VaultUnlockFailed`、`LocalDbRepaired`）。

2. **网关审计分支完整性**：确认 `evaluate` 的每个 `return Ok(GatewayDecision::...)` 前都有 `self.audit(...)`。

```powershell
rg -n "GatewayDecision::" -B 6 src-tauri/crates/assistant-tools/src/gateway.rs
```

3. **intent 覆盖率**：对比工具注册表与 `permission_intents` 实现。

```powershell
rg -n "registry.register" src-tauri/crates/assistant-tools/src/os/mod.rs
rg -n "fn permission_intents" src-tauri/crates/assistant-tools/src
```

判定：注册的工具数 > `permission_intents` 实现数时，差额即为 G6 的覆盖范围（当前 11 vs 6）。

4. **命令层审计覆盖率**：`cmd/**` 下每个 `#[command]` 是否写审计。

```powershell
rg -n "#\[command\]" -A 2 src-tauri/src/cmd
rg -n "audit_log|audit_dual|audit_jsonl|permission_audit" src-tauri/src/cmd
```

5. **前端消费点**：确认 `queryAuditLog` 是否有组件调用。

```powershell
rg -n "queryAuditLog" src
rg -n "[Aa]udit" src/vault --glob "*.tsx"
```

6. **补录白名单**：确认 `BACKFILL_EVENTS` 是否覆盖「库不可用时产生」的事件——**双向核对**（先列出仅 JSONL 的事件，再逐个看是否在白名单）。

```powershell
rg -n "BACKFILL_EVENTS" -A 8 src-tauri/crates/assistant-vault/src/security_db/mod.rs
rg -n "audit_jsonl\(|log_dual\(" -A 3 src-tauri/crates/assistant-vault/src src-tauri/src --glob "!*tests*"
```

判定：第二个命令里凡 `log_dual` 的 conn 实参为 `None`（或走 `audit_jsonl`）的事件，其蛇形名必须出现在白名单中；漏一个即为 G10 复发（当前漏 `vault_unsupported_version`、`biometric_failed`）。

7. **「命令层无审计」不等于「全无痕」**：判断某个命令是否真的无痕，要连带它调用的 `security_db` 写入原语一起看。

```powershell
rg -n "fn upsert_imported_in|fn upsert_imported_entries|fn force_import_entry_tx|fn create_remote_copy_tx|fn restore_from_history_tx|fn delete_entries_tx" -A 50 src-tauri/crates/assistant-vault/src/security_db/entries
rg -n "audit_log" src-tauri/crates/assistant-vault/src/security_db/entries
```

判定：每个写入原语的所有分支（尤其 `Updated` / `Inserted` / `Skipped*`）是否各有 `audit_log`；当前 `upsert_imported_in` 的 `Updated` 分支缺失（G4）。

> 新增事件时的自检清单（建议写进 PR 模板，评审按条打勾，不做「事后补」）：① `AuditEvent` 变体 + `Display` 蛇形字符串；② 生产调用点（不是测试）；③ 若产生于「库不可用」路径 → 必须写 JSONL **且加入 `BACKFILL_EVENTS`**（只做前半句即为 G10）；④ detail 用封闭集合或摘要，禁敏感值；⑤ 是否需要同步更新本文 §3 与 §4；⑥ 若在命令层新增写审计，确认不会被条目层的同类事件重复记（避免一动作两行）。

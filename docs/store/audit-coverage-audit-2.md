# 前端审计覆盖审计（第 2 轮）

| 项 | 值 |
|---|---|
| 状态 | 现行（审计基线，随代码复核更新） |
| 审计日期 | 2026-11-14 |
| 审计对象 | 两条运行时审计链路在**前端**的读写情况：① vault 审计查询链；② Agent 工具权限审计（`permission_audit`）的展示与触发面 |
| 审计范围 | `src/vault/**`（`core/vault.service.ts`、`core/slices/vaultAuditSlice.ts`、`core/vaultStore.ts`、`settings/*.tsx`、`VaultUnlock.tsx`、`VaultToolbar.tsx`）、`src/services/permissionService.ts`、`src/services/chatService.ts`、`src/settings/shared/sections/PermissionGrantsSection.tsx`、`src/settings/pages/AgentModeSettings.tsx`、`src/stores/chatConversationSlice.ts`、`src/chat/tool/ApprovalCard.tsx`、`src/chat/core/toolCallView.ts`、`src/tools/os.ts`、`src/i18n/locales/*.json`、`src/test-utils/rustCommands.ts`、`src/services/__tests__/commandNames.test.ts` |
| 审计方法 | ① 前端命令字面量全量扫描，按「命令 → 后端 `#[command]` → 是否写审计」三跳对齐；② 每个审计查询能力反查 `.tsx` 消费点；③ 前端可触发的权限/密钥动作逐个核对后端落点；④ 对照 `I50-runtime-audit-spec.md` §2–§4 与 `I50-runtime-audit-tasklist.md` 的 V3 / T4 条目核对现状 |
| 证据基准 | 只采当前工作区源码。第 1 轮审计（`audit-coverage-audit-1.md`）只作为后端侧结论的对照基线，不作为前端现状依据 |
| 引用口径 | 定位优先用 `文件::符号`；行号仅作本次审计时的参考，会随迭代漂移 |
| 修订记录 | 2026-11-14 首轮前端审计：建立「前端入口 → 后端审计」映射表，登记缺口 F1–F8、观察项 OF1–OF5、有意例外 EF1–EF3 |

---

## 0 结论摘要

1. **两条审计链路在前端都处于「只写不读」的完全空白状态**。vault 侧查询能力只到 service / slice（`vault.service.ts::queryAuditLog`、`vaultAuditSlice.ts::queryAuditLog`），`src/vault` 下 `.tsx` 中 `queryAuditLog` **零命中**；工具权限侧前端连查询接口都没有（`permissionService` 只有 grants 的 list / revoke / clear / setScope）。
2. **前端唯一与「权限」相关的 UI 是 `PermissionGrantsSection`，它展示的是「已授权操作」，不是审计日志**。用户能看到「谁被放行了」，看不到「谁被拒绝、谁超时、谁越界被拦」。
3. 前端可触发的动作里，**后端已覆盖的审计占多数**（解锁成功、锁定、reveal / copy、导出导入、改密、DEK 轮换、审批四决策、工作区切换、历史恢复、冲突解决均已有落点）；缺口集中在前端**独有的三个触发面**：
   - **授权回收**（撤销 / 全部清除 / 改作用域）—— 后端 4 个命令零审计（F3）；
   - **ToolBox 直调 OS 工具**（`src/tools/os.ts` → `os_tool_exec`）—— 绕过网关（F4 对应后端 G8）；
   - **自动锁定**（前端 `setTimeout` → `vault_lock`）—— 审计 detail 恒为 `user`，无法与手动锁定区分（F5）。
4. 前端要落地可用面板，还缺三块「地基」：**查询参数与字段**（`query_log` 固定 `ORDER BY timestamp DESC LIMIT ?1`，`AuditLogEntry` 无 entryId）、**`permission_audit` 无任何索引**、**前端零事件词表映射与 i18n 键**（31 个 vault 事件、8 个 outcome 在 `src/i18n` 中无任何条目）。
5. 有一条**前端红线**必须写进实现约束：**前端不得本地生成、缓存或推断审计记录**。审计是后端事实源，前端只做「读 + 展示 + 映射」，否则会出现「UI 有记录、库里没有」的举证不一致。

---

## 1 判定口径（前端）

后端三要素（事件标识 / 生产调用点 / 可检索落点）不变；前端另加两条：

| 维度 | 要求 | 反例 |
|---|---|---|
| 可读 | 存在前端调用链（service → store/slice → 组件）且**有 `.tsx` 消费** | 只定义 `queryAuditLog` 无人调用（现状） |
| 可解释 | 事件串 / outcome 串有封闭映射（i18n 文案 + `detail` 文法解析），未映射值有兜底原文显示 | 直接把 `vault_entry_purged` 蛇形串丢进表格 |

补充口径：

- **前端不参与审计写入判定**。前端动作触发的审计由后端命令/网关负责；前端「补审计」的唯一合法形态是「补一条能触发后端既有写入路径的调用」，不是自己发事件。
- **前端展示口径必须与后端统计口径一致**：拒绝率、超时率一律以 `permission_audit.outcome` 为准，不得用 `toolCallView.status = error` 反推（见 OF2）。
- **锁定态不可查是设计约束**：`vault_query_audit_log` 经 `with_unlocked` 包装（`cmd/vault/mod.rs::with_unlocked` → `SecurityDbState::get_conn`），锁定态直接返回 `VaultError::Locked`（前端错误码 `locked`），前端必须在锁定态禁用面板而不是报错（见 OF3）。

---

## 2 前端现状盘点

### 2.1 vault 审计查询链（有接口、无消费）

| 层 | 位置 | 现状 |
|---|---|---|
| 契约 | `src/vault/core/contract/vault.contract.ts::AuditLogEntry` | `{ id: string, eventType: string, detail: string \| null, timestamp: number }`，无 `entryId`、无 `eventType` 枚举、无 `since` |
| service | `src/vault/core/vault.service.ts::queryAuditLog` | `invoke("vault_query_audit_log", { limit })`，仅 `limit` 一个入参 |
| slice | `src/vault/core/slices/vaultAuditSlice.ts::queryAuditLog` | 纯透传，**不落 state**、无 loading / error 分支 |
| store | `src/vault/core/vaultStore.ts` | `auditSlice` 已装配（第 45 / 53 行） |
| 组件 | — | **零消费**：`src/vault` 下 `.tsx` 中 `queryAuditLog` 无命中 |
| 设置页 | `src/vault/settings/VaultSettings.tsx` | 6 个 tab：`sync` / `ssh_agent` / `recovery` / `change_password` / `import_export` / `danger_zone`，**无 audit tab** |
| i18n | `src/i18n/locales/zh-CN.json::vault.tab` | 8 个键（`vault` / `manage` / `sync` / `ssh_agent` / `recovery` / `change_password` / `import_export` / `danger_zone`），**无 audit 键** |

注：`src/vault` 下所有 `audit` 字样均属 **HIBP 泄露检测**（`VaultToolbar.tsx::vault.hibp.audit_all`、`vault.service.ts::hibpAudit` / `hibpAuditCancel`），与审计日志无关（见 F8）。

### 2.2 工具权限审计（前端只有「授权面」）

| 层 | 位置 | 现状 |
|---|---|---|
| service | `src/services/permissionService.ts` | 4 个方法：`list` / `revoke` / `clear` / `setScope`，**无审计查询** |
| 契约 | `src/types/protocol.contract.ts::PermissionGrantDto` | 只有授权行字段，无 `permission_audit` 对应 DTO |
| 组件 | `src/settings/shared/sections/PermissionGrantsSection.tsx` | 「已授权操作」列表：撤销、改作用域、全部清除；**无审计日志区** |
| 挂载点 | `src/settings/pages/AgentModeSettings.tsx`（第 479 行） | 设置页内联 section，非独立 tab |
| 后端 | `src-tauri/src/cmd/permission.rs` | 只有 `list_permission_grants` / `revoke_permission_grant` / `set_permission_grant_scope` / `clear_permission_grants`，**零审计写入** |

`permission_audit` 表结构（`assistant-storage/src/db.rs`）：`(id INTEGER PK AUTOINCREMENT, tool, action, resource, outcome, at_millis)`，**全表无任何 `CREATE INDEX`**（`db.rs` 中 6 处索引均属 `messages` / `attachment_refs` / `token_usage`），且注释明示「只追加不更新、不设 TTL」。

### 2.3 前端入口 → 后端审计落点

#### vault 侧

| 前端入口（`src/vault`） | 命令 | 后端审计 | 状态 |
|---|---|---|---|
| `vaultService.unlock` | `vault_unlock` | 成功 `VaultUnlock`（双写） | 有 |
| 同上（口令错误） | 同上 | 仅 `log::warn!` | **无 → G1** |
| `unlockBiometric` | `vault_unlock_biometric` | `BiometricUnlocked` / `BiometricFailed`（仅 JSONL） | 有写入，**白名单缺失 → G10** |
| `lock`（手动按钮 + `vaultLifecycleSlice` 自动锁定定时器） | `vault_lock` | `VaultLock`，detail 恒 `user` | 有，**归因失真 → F5** |
| `vault_init` | `vault_init` | 无 | **无 → G3** |
| `resetRepository` | `vault_reset_repository` | `VaultRepositoryReset`（仅 JSONL） | 有 |
| `rebuildLocalDb` | `vault_rebuild_local_db` | `LocalDbRebuilt`（双写） | 有；口令错误分支**无 → G1** |
| `copyPassword` / `copyField` / `totpCopy` | `vault_copy_password` / `vault_copy_field` / `vault_copy_totp` | `VaultCopyPassword` / `VaultCopyField` | 有 |
| `revealPassword` / `revealNotes` / `conflictReveal` | `vault_reveal_password` / `vault_reveal_notes` / `vault_conflict_reveal` | `VaultReveal` / `VaultRevealRateLimited` | 有 |
| `exportToFile` / `import` | `vault_export_to_file` / `vault_import` | `VaultEntriesExported` / `VaultEntriesImported`（detail `{format}:{count}:{pathHash}`） | 有 |
| `hibpAudit` / `hibpCheck` | `vault_hibp_audit` / `vault_hibp_check` | `HibpAudit` / `HibpLeakFound` | 有 |
| `changeMasterPassword` | `vault_change_master_password` | `MasterPasswordChange` | 有 |
| `exportRecoveryKey` / `rotateDek` | `vault_export_recovery_key` / `vault_rotate_dek` | `RecoveryKeyExported` / `DekRotated` | 有 |
| `recoverFromPaper` | `vault_recover_from_paper` | `RecoveryExportUsed`（仅 JSONL） | 有 |
| `syncPull` / `syncPush` / `syncSync` / `syncForcePush` | `vault_sync` | 无 | **无 → G4** |
| `syncDiscard` | `vault_sync_discard` | 无需（只 `git reset --hard` 不动库） | 有意例外 |
| `resolveConflict` | `vault_resolve_conflict` | 条目层 `VaultEntryUpdated` / `VaultEntryCreated` | 有（缺 `conflict:{side}` 标记 → O5） |
| `historyRestore` | `vault_history_restore` | `VaultEntryRestored`，detail `{entryId}@{commit}` | 有 |
| `sshAddToAgent` / `sshRemoveFromAgent` | `vault_ssh_add_to_agent` / `vault_ssh_remove_from_agent` | `SshKeyLoaded` / `SshKeyRemoved` | 有（手动路径） |
| `sshAutoAddAll` | `vault_ssh_auto_add_all` | 无 | **无 → G5** |

#### 工具权限侧

| 前端入口 | 命令 | 后端审计 | 状态 |
|---|---|---|---|
| 审批卡片 4 决策（`ApprovalCard::decide`） | `resolve_agent_tool_call` | `Approved` / `SessionApproved` / `AlwaysApproved` / `Rejected`（网关 `audit()`） | 有 |
| 审批卡片超时（前端仅禁用按钮） | — | 后端 `ApprovalTimedOut` → `TimedOut` | 有（见 OF1） |
| 授权撤销 / 全部清除 / 改作用域 | `revoke_permission_grant` / `clear_permission_grants` / `set_permission_grant_scope` | 无 | **无 → F3** |
| 会话工作区切换（`setConversationWorkspace` → `patch_conversation`） | `patch_conversation` | 直插 `permission_audit`（`tool=workspace` / `action=switch` / `outcome=allowed`） | 有 |
| ToolBox `fs_link` create / check（`src/tools/os.ts::callOsTool`） | `os_tool_exec` | 无（`ToolUseContext::for_toolbox` 不经网关） | **无 → G8 / F4** |
| 工具档位与审批级别变更（`toolConfig`） | `patch_conversation` | 无 | 配置写入，非权限决策（有意例外 EF2） |

---

## 3 缺口清单

编号 `F`（frontend），与第 1 轮 `G` 编号互不冲突；「对应」列指向后端缺口。

### F1 前端无任何审计展示面 —— 高（对应 G9 前端半边）

**现象**：vault 侧查询能力齐全到 slice 层却**零组件消费**；vault 设置页 6 个 tab 无审计入口；工具权限侧连查询接口都没有。

**影响**：审计「只写不读」在前端彻底闭环不了。用户无法回答「上次改密是什么时候」「这个工具被拒绝过几次」「SSH 私钥是什么时候被灌进 agent 的」。

**建议修法**：vault 设置页新增 `audit` tab，消费既有 `queryAuditLog`（后端无需新命令，V3.1 / V3.2 仅补参数与字段）；工具权限侧待 `list_permission_audit` 落地后再加面板（见 F2）。面板必须：默认 `limit` 有上限、按 `timestamp DESC`、锁定态禁用（OF3）。

### F2 工具权限侧无查询命令 / 无 DTO / 无 service —— 高（对应 G9 工具半边）

**现象**：`src-tauri/src/cmd/permission.rs` 4 个命令全是授权管理，无一条读 `permission_audit`；`protocol.contract.ts` 无对应 DTO；`permissionService` 无方法。

**影响**：即使前端想做面板也无数据源。这是「前端缺口」里唯一**必须先后端**的一项。

**建议修法**：新增 `list_permission_audit(limit, since)` + 输出 DTO（`Option` 不加 `skip_serializing_if`，前端声明 `T | null`），走 `npm run contract:gen`；查询必须带 `limit` 且按 `at_millis DESC`。**同时必须补索引**（F6），否则无 TTL 全表扫描。

### F3 授权回收动作零审计 —— 高

**现象**：`PermissionGrantsSection` 是前端**唯一能改变权限状态的入口**（撤销、全部清除、会话级 ⇄ 全局互转），对应后端 3 个写命令 `revoke_permission_grant` / `clear_permission_grants` / `set_permission_grant_scope` **全部零审计**（`permission.rs` 内无任何 `permission_audit` 写入）。

**证据**：全仓库 `permission_audit` 写入只有两处——`adapters/permission_store.rs::log_decision`（网关决策）与 `cmd/conversation.rs`（工作区切换直插）。

**影响**：与「放行」方向相反的「回收」方向完全无痕。事后无法回答「这条全局永久授权是谁在什么时候撤销的」——而撤销恰恰是安全事件响应时最常被追问的动作。与 G8（ToolBox）相比，这条更该补：ToolBox 至少没有权限状态变更，授权回收是**持久权限的真实变更**。

**建议修法**：3 个命令各补一次 `permission_audit` 写入，沿用既有词表与语义边界：`tool` 取授权行的 `tool`、`action` 取 `grant:revoke` / `grant:clear` / `grant:scope`、`outcome` 记 `allowed`（这是「管理动作被允许执行」，不是权限决策，**禁止**伪造 `denied` / `bypassed`）。`resource` 记 `grantKey` 或 `none`（全部清除）。detail 不得落 `key_json` 明文之外的敏感内容。注意 `permission.rs` 走的是 `DbState` 主库连接，不是网关的 `PermissionAuditStore`，实现上需复用 `outcome_str` 的串面而不是新增一套。

### F4 ToolBox 直调无审计 —— 中（对应 G8）

**现象**：前端 `src/tools/os.ts::callOsTool` 是唯一 OS 工具入口，实际只暴露 `fs_link` 的 `create` / `check`（`src/tools/fs-link/index.tsx` 两处调用），统一走 `os_tool_exec`；该命令用 `ToolUseContext::for_toolbox(None)` 直接 `invoke`，不经 `ToolGateway`。

**影响**：用户从 UI 主动执行的 OS 工具操作是审计盲区；`fs_link check` 更是纯读探测，连「被调用过」都没有记录。

**建议修法**：与 G8 / T4.1 合并处理。若按 T4.1 的 `bypassed` 语义落地，前端无需改动（命令层前置写审计即可）；**不要**在前端补任何本地记录。

### F5 自动锁定与手动锁定不可区分 —— 中

**现象**：前端自动锁定由 `src/vault/core/slices/vaultLifecycleSlice.ts::resetAutoLockTimer` 的 `setTimeout` 触发，最终调用同一个 `vaultService.lock()` → `vault_lock`；而 `cmd/vault/lifecycle.rs::vault_lock` 恒传 `LockReason::User`。`LockReason::AutoLock` 在 `assistant-vault` 内**生产零调用**（仅 `tests/session_lock.rs` 使用），`as_str` 的 `"auto_lock"` 永远不会出现在审计里。

**影响**：`VaultLock` 的 detail 无法区分「用户主动锁」与「闲置超时锁」。这两者在安全复盘里含义完全不同（前者是用户行为，后者是「无人值守暴露时长」证据），而审计现在把它们合并成 `user`。

**建议修法**：二选一，不要保留悬空枚举值。① `vault_lock` 增加可选 reason 入参，前端手动按钮不传（默认 `user`），自动锁定定时器传 `autoLock`，与 `LockReason::as_str()` 对齐；② 若不愿改命令签名，则删除 `LockReason::AutoLock` 并在规格落点表登记「自动锁定并入 user」。推荐 ①，因为审计价值明确且改动局限于一个命令 + 一处前端调用。

### F6 查询能力与索引不足以支撑面板 —— 中（对应 O6 / O3）

**现象**：

| 项 | 现状 |
|---|---|
| `query_log` SQL | `SELECT id, event_type, detail, timestamp FROM security_audit_log ORDER BY timestamp DESC LIMIT ?1`——**固定排序、无 `since`、无 `event_type` 过滤、无分页** |
| `AuditLogEntry` | `id` 是自增整数串（非稳定业务 ID），无 `entryId` 语义字段 |
| 前端入参 | `queryAuditLog(limit?)` 只有一个 `limit` |
| `permission_audit` | 表无任何索引；无 TTL；前端若拉列表只能全表扫描 |

**影响**：面板只能做「最近 N 条」这一种视图，无法按事件类型筛选（用户最常问的就是「只看解锁失败」「只看导出」），也无法翻页；数据量增长后单次查询会退化。

**建议修法**：`query_log` 增加 `since` / `event_types` 可选参数（表上已有 `idx_audit_log_timestamp` / `idx_audit_log_event_type`，成本低）；`AuditLogEntry` 视需要补 `entryId`；`permission_audit` 补 `(at_millis)` 与 `(tool, at_millis)` 索引。均为后端改动，前端只透传。

### F7 前端零事件词表映射与 i18n —— 中

**现象**：`src/i18n/locales/zh-CN.json` / `en-US.json` 中**无任何审计事件文案**（`audit` 命中仅 `vault.hibp.audit_all` 与 `danger_zone.confirm1` 里的说明文字）；`vault.tab` 无 `audit` 键；`AuditEvent` 31 个变体、`AuditOutcome` 8 个变体在前端无任何映射。

**影响**：面板一旦落地，表格只能显示蛇形串（`vault_entry_purged` / `alwaysApproved`），用户不可读；`detail` 的封闭文法（`{format}:{count}:{pathHash}`、`{entryId}@{commit}`、`auto:{entryId}`、`{side}:{field}`）无解析规则，只能整串展示。

**建议修法**：新增两层映射（纯展示、不参与写入）：① `event_type` / `outcome` → i18n key 的封闭字典；② `detail` 按 `event_type` 分支渲染（仅解析已知文法，**解析失败必须原样兜底显示**，不得吞掉——否则新事件上线后前端会「静默丢行」）。zh-CN 与 en-US 同步补齐。

### F8 HIBP「审计」与安全审计日志命名冲突 —— 低

**现象**：前端已有 `hibpAudit` / `hibpAuditCancel` / `vault.hibp.audit_all` 一套「审计」术语，指**泄露检测**；新增的安全审计日志若也用 `audit` 命名，UI 与 i18n 会撞名。

**建议修法**：新面板命名统一用 `securityLog` / `security_log`（后端事件串保持 `vault_*` 不变），i18n 键放 `vault.tab.security_log` 与 `vault.security_log.*`，与 `vault.hibp.*` 明确区分。

---

## 4 观察项

- **OF1 审批超时是双轨**：`ApprovalCard` 的 `expired` 只做本地禁用（`locked = expired || submitting !== null`），**不向后端发任何请求**；真正的 `timedOut` 由后端 `ApprovalTimedOut` 写。因此前端时钟偏差只影响按钮可点性，不影响审计口径——审计一律以后端为准，前端不得自记超时。
- **OF2 拒绝的两种「可见性」不同源**：`toolCallView.ts` 的 `status = error` + `errorKind` 是**消息侧事实**（落 `messages.tool_calls_json`），与 `permission_audit.outcome` 不是同一口径。面板统计拒绝率必须读 `permission_audit`，不得用 `error` 状态反推（两者会因 `validateRejected` / `intentError` 等新增 outcome 而分叉）。
- **OF3 锁定态不可查**：`vault_query_audit_log` 经 `with_unlocked` 包装，锁定态直接返回 `VaultError::Locked`（前端错误码 `locked`）。面板必须「锁定态禁用 + 解锁后拉取」，不得把该错误当异常弹出。
- **OF4 JSONL 侧无读取命令**：`vault_unlock_failed`、`ssh_key_removed`（锁库批量）等只在 JSONL 的事件，前端在解锁前完全看不到，只能等补录进库后才出现在面板。**补录白名单漏一项，UI 就永久少一类记录**（与 G10 强联动，属「补了等于没补」的前端可见版本）。
- **OF5 面板必须强制分页上限**：`permission_audit` 无 TTL、`security_audit_log` 随使用线性增长，前端不得提供「加载全部」入口。

---

## 5 有意例外

- **EF1 前端不缓存审计**：不在 store 里做本地副本、不做增量合并、不做乐观插入。审计是后端事实源，前端只读。
- **EF2 配置类变更不入审计**：`toolConfig`（审批档位、工具开关）、`vaultAutoLockMinutes`、`vaultCipherAlgorithm` 等设置写入不产生审计行——它们改变的是「未来策略参数」，不是「一次权限决策」。当前无对应事件，**不登记为缺口**。
- **EF3 条目层事件锁定态不可见属设计**：条目类事件（created / updated / deleted / restored / purged）只写 DB（`audit::log`），前端在锁定态查不到，这是加密库的既定边界，不是缺口。

---

## 6 与 I50 规格 / 任务清单的对应

下表「规格 / 清单」列为本轮审计提出后**已更新**的落点（规格与清单同日修订）：F1 / F2 / F3 / F5 / F7 / F8 原先无规格条款或无清单条目，现已分别补入 §2.6 / §3.7 / §3.6 / §2.3 / §6 与 P3.1 / P4.1 / P2.1 / P1.1 / P5.1–P5.3 批次。

| 本审计条目 | I50 规格（更新后） | 任务清单（更新后） |
|---|---|---|
| F1 vault 面板 | §2.6「前端必须提供面板」 | P3.1 |
| F2 工具权限查询 | §3.7 查询面 | P4.1 / P5.3 |
| F3 授权回收审计 | §3.6 非网关审计行 | P2.1 |
| F4 ToolBox | §3.5 / G8 | T4.1 |
| F5 锁定原因 | §2.3「`vault_lock` 的归因」 | P1.1 |
| F6 查询参数 / 索引 | §2.6 / §3.7 / §5 | V3.1 / V3.2 / T5.2 / P4.1 |
| F7 词表映射 / i18n | §6「词表必须封闭映射」 | P5.1 / P5.2 |
| F8 命名冲突 | §6「命名不得与 HIBP 审计撞车」 | P3.1 / P5.3 |

---

## 7 复核命令

前端侧（本审计涉及的既有守卫）：

```
npm run typecheck
npx vitest run src/services/__tests__/commandNames.test.ts
```

`commandNames.test.ts` 的 `invoke` 字面量集合由 Rust `generate_handler![` 解析而来（`src/test-utils/rustCommands.ts`），因此**新增 `list_permission_audit` 后前端必须同时出现同名字面量调用**，否则双向差集断言失败——这是 F2 落地时的硬约束。
# I9 密码箱 P3 任务清单（P2 残留闭环）

> 来源：对 `I9-vault-tasklist-p2.md` 未勾选项与当前代码的逐项对账（2026-10-28）。
> 对账基准：`assistant-sync` / `assistant-security` / `src-tauri/src/cmd/vault` / `src/vault` 现有实现。
> 结论：P2 规格中的**功能点已全部落地**；残留集中在 **同步正确性缺陷（P0）**、**少量功能补全（P1）**、**验证缺口（P2）**、**设计待决（P3）** 四类。
>
> 状态图例：`[ ]` 未开始 / `[~]` 部分完成 / `[x]` 已完成或已定 / `[!]` 缺陷待修
>
> **决策已全部拍板**：19 项见 §0.1，无待定项。
>
> **实施进度（2026-10-28）**：B1 ~ B7 已完成并验证通过（B5 含 P3 / P4，B6 含 N3 / N4，B7 含 M7 前端提示）；剩余 O1 ~ O3（验证补齐）。详见 §0.5。

---

## 0. 残留项总览

| 编号 | 残留内容 | 类型 | 优先级 | 代码现状 | 状态 |
|------|----------|------|--------|----------|------|
| M1 | 冲突解决结果不落库，下次同步被本地覆盖 | 缺陷 | P0 | `resolve_conflict` 已强制落库 + 自动 push | `[x]` |
| M2 | 删除不传播：远端删除被本地复活，且与墓碑乒乓 | 缺陷 | P0 | 待确认态（内存）+ 确认后批量软删除 | `[x]` |
| M3 | 导出不幂等：每次同步全量重写 → 必然 commit + push | 缺陷 | P0 | `export_entry` 按 revision 跳过，meta 内容未变不重写 | `[x]` |
| M4 | 防误删护栏误伤"对端全量删除" | 缺陷 | — | **并入 M2**（D3），阈值降级为文案强度 | `[x]` |
| M5 | 单独 pull / push 后前端不刷新 | 缺陷 | P1 | `syncPull` / `syncPush` 已补 `loadEntries` + `loadSyncReadyness` | `[x]` |
| M6 | `sync_now` 提交信息风格不一致 | 清理 | P2 | 已统一为 `vault: sync`（D10） | `[x]` |
| M7 | 无法解密的条目被静默跳过，且可能被本地导出覆盖 | 缺陷 | P0 | `SyncResult::UnreadableEntries { ids, reason }` + 归因 + 导出跳过 + 前端两条出路按钮 | `[x]` |
| N1 | `MergeBoth` 退化为 `UseLocal`（H10.2） | 补全 | P1 | 已实现为新条目（`Uuid::new_v4`） | `[x]` |
| N2 | 上次同步时间（H14.2 / H16.1） | 补全 | P1 | `vault_sync_last_time` 命令 + `AppConf.vault_sync_last_at` 写入 + 两处 UI 展示 | `[x]` |
| N3 | 恢复密钥解锁未做 meta 完整性告警（J3.4） | 补全 | P1 | 仅灾难恢复导出做了 | `[x]` |
| N4 | `repoState` 文案落到 `status_unknown`（H14.2） | 补全 | P2 | `VaultSyncSettings.tsx` | `[x]` |
| N5 | 冲突解决后不 push | 补全 | P2 | 已随 M1 一并 `finalize("vault: resolve conflict")` | `[x]` |
| N6 | 冲突解决界面信息不足，用户只能盲选 | 补全 | P1 | 条目名 + 按需显示 + 差异高亮 + 分侧降级（D17） | `[x]` |
| O1 | 单元/集成测试缺口 | 验证 | P1 | 本轮新增 5 项 `sync_engine` 用例；`ssh_validate` 4 项 / `recovery_key` 9 项为存量用例，核对后确认已覆盖对应缺口；剩 `need_to_push` / `GIT_TERMINAL_PROMPT` / D19 历史版本导出 3 项，见 §O1 | `[~]` |
| O2 | 手工验证清单（I7 / J8 / K / L7 残留） | 验证 | P2 | 见 §O2 | `[ ]` |
| O3 | `clippy` / 全量 `cargo test` / `tsc` 复核 | 验证 | P1 | 已跑 `assistant-sync`（40 项）+ `assistant-security`（全绿）+ `fmt` / `check` + `lint` / `typecheck` / `vitest`；`clippy --workspace` 未重跑 | `[~]` |
| P1 | Git 完整性边界（H5.8） | 已定 | — | 不引入仓库级 HMAC（D11） | `[x]` |
| P2 | `last_good_commit` 回滚检测（H5.9） | 已定 | — | 不纳入 P3（D12） | `[x]` |
| P3 | SSH ControlMaster 复用验证 + 白名单（H19.15） | 补全 | P2 | `AppConf.vault_sync_ssh_hosts` 可配置 + 留空回退默认 | `[x]` |
| P4 | `SyncConfig.branch` 死字段 | 清理 | P2 | 已从 struct / interface / `buildConfig()` / mock 四处删除 | `[x]` |

---

## 0.1 决策记录（已拍板）

> 拍板日期：2026-10-28。以下结论已确定，实施时不再讨论；未列入者见 §0.2。

| 编号 | 决策点 | 结论 | 实施要点 |
|------|--------|------|----------|
| D1 | 远端删除如何落库（H8「同步不擅自修改本地数据库」） | **保留 H8 原则不变**。`sync_now` 的 pull 阶段**不写 DB**，将"仓库已消失但本地仍存活"的条目作为待确认列表返回；**用户确认后**才软删除（写 `deleted_at`，`revision` 不变）。禁止硬删除、禁止覆盖条目内容 | 新增 `SyncResult::PendingRemovals { entries: [{ id, title }], totalLocal }`；未确认前**禁止 push**（否则条目被复活形成乒乓） |
| D15 | 「待确认删除」态存哪 | **内存态**（`SyncEngine` 的 `Mutex` 字段，不落盘、不加 `AppConf` 字段） | 重启即丢失，但下次 `sync_now` 的 pull 会重新检测到同一批 `removed_ids`，**幂等自愈**；无需持久化，也无迁移成本 |
| D16 | 取消确认后如何告知用户「尚未同步到远程」 | **UI 常驻提醒**：库处于待确认态时，`VaultSyncSettings` 状态行 + `VaultList` 顶部横幅显示「有 N 条远端删除待确认，当前未同步到远程」，并提供「查看并确认」入口 | 提醒必须**显式**（不能只禁用 push 按钮而不解释），否则用户会误以为同步已完成；文案走 i18n；确认或取消后横幅消失 |
| D2 | 远端删除是否在 DB 中区分来源 | **不新增列**（`vault_entries` 保持现结构，零迁移）；来源写入审计日志 `VaultEntryDeleted` 的 `detail`（`local` / `remote`） | 回收站 UI 暂不区分来源，将来要加列时审计数据仍在 |
| D3 | 删除确认的阈值与交互形态 | **取消阈值自动判断**：**所有**远端删除一律进入确认流程（与 D1 的 `PendingRemovals` 合并，不再有独立变体）。阈值降级为**文案强度分级**：`removed == local_count` 时用强警告文案（"对端删除了全部 N 条"），否则普通文案 | M4 并入 M2，不再单独实现护栏；确认框列出条目标题，一次性确认；需 i18n 两套文案 |
| D4 | `UseRemote` 落库的 revision 取值 | **`max(local, remote) + 1`** | 保证单调递增，其他设备 pull 时正常接受；新增的 `force_import_entry` 仅 `pub(crate)`，只允许 `resolve_conflict` 调用 |
| D5 | M3 幂等判据 | **只比 `EntryFile.revision`**（零解密）；`.vault-meta` 内容未变则**整文件不重写**，`exported_at` 语义变为「上次内容变更时间」 | 外部工具手改 `.enc` 但 revision 未变 → 不重写，该场景不在威胁模型内 |
| D7 | 上次同步时间的存储位置 | **`AppConf.vault_sync_last_at`**（i64 毫秒），库锁定时仍可读 | 需 Rust `AppConf` struct + 前端 interface + DTO + `From` 四处同步（见项目规则） |
| D8 | 恢复密钥解锁完整性告警的传递方式 | **保持 `bool` 返回值不变**，额外广播 `VAULT_META_INTEGRITY_WARNING` 事件 | 零前端契约破坏；`vaultStore` 监听后弹 Modal，仅告警不阻断 |
| D9 | 冲突解决后是否自动 push | **自动**：`resolve_conflict` 收尾直接 `finalize("vault: resolve conflict")` | 与 N5 合并实施；前端 `resolveConflict` 成功后补 `loadEntries()` |
| D10 | `sync_now` 的提交信息 | 统一为 **`vault: sync`**，去掉 `Sync at {rfc3339}` | 与 L6 的 id 化提交风格一致，避免历史面板混入英文时间戳条目 |
| D11 | Git 完整性边界（P1 / H5.8） | **不引入仓库级 HMAC**；`.vault-meta` 的 `integrity` 维持仅覆盖元数据自身 | 有仓库写权限者超出威胁模型，任何仓库内校验都会被一并伪造 |
| D12 | `last_good_commit` 回滚检测（P2 / H5.9） | **不纳入 P3**，仅登记 | 基准不可存入仓库 + 多设备并发 + force push 语义，成本远超收益 |
| D17 | 冲突解决界面是否允许显示密码明文 | **允许，但按需触发**：默认全掩码，用户点击「显示」才解密该侧该字段，复用详情页 reveal 语义（30s 后前端自动清除）。**不做**「只显示密码是否相同」的保守方案 | 新增 `vault_conflict_reveal(path, side, field)` 命令；与 `revealPassword` 同一套安全模型（用户主动触发、临时内存、不写日志）；冲突未解决前不落库 |
| D18 | 已无法解密的条目/仓库文件的处理策略 | **显式识别 + 不覆盖 + 明确归因**：①`read_all_entries` 不再静默跳过，返回失败 id 列表；②`ImportReport` 增加 `unreadable_ids`；③导出阶段**跳过**这些 id，禁止用本地旧密钥密文覆盖对端文件；④按 `.vault-meta.crypto.masterKeySalt` 与本地 `master_key_salt` 比对归因——不同则报「对端使用另一套主密钥」，相同则报「文件损坏」 | 归因是本次新增的关键能力：现状一律落到 `HMAC signature mismatch — vault meta may be tampered`，用户会误判为被篡改。出路见 D19，**不新增**任何"输入另一主密码接管仓库"的流程 |
| D19 | 密钥不匹配时用户的出路 | **复用现有灾难恢复导出**（`vault_disaster_recovery_export`：clone 仓库 + 恢复密钥 → 明文 JSON），仅增加可选 `commit` 参数以支持**导出历史版本**；不做「输入对端主密码接管仓库」的新流程 | 改主密码会 `clear_emergency_data` 使 HEAD 的 `.vault-meta.emergency` 为空，恢复密钥在 HEAD 必然失败；但**密码修改之前的历史提交仍带 emergency blob**，因此"选历史版本导出"是必需能力而非可选优化。若用户从未生成恢复密钥，则唯一出路是保留本机数据或覆盖对端，必须显式警告 |
| D14 | `SyncConfig.branch` 死字段（P4） | **删除该字段** | 已确认无持久化：仅 `assistant-sync` 的 `SyncConfig` struct + `Default`、`src/vault/core/vault.types.ts`、`buildConfig()`、`vaultStore.test.ts` 四处；`AppConf` 无对应项，无需迁移 |
| D6 | `MergeBoth` 实现还是从 UI 移除 | **实现**：取 stage 3（theirs）解密，**创建为新条目**（`Uuid::new_v4()`，`revision = 全库 max + 1`），本地保留原 id | 新条目 id 不得用 `{id}-remote-{ts}`，避免污染 id 语义；需 `parse_entry_id` 兼容性检查 + 导出 `.enc` |
| D13 | SSH ControlMaster 白名单范围 | **用户可配置**：在同步设置页与远程地址一并配置（新增 `AppConf.vault_sync_ssh_hosts: Vec<String>`），留空时回退默认 `github.com` / `gitlab.com` | 与 D7 的 `AppConf` 字段同批实现，四处同步；`ssh.rs` 从常量白名单改为读配置；前端 `VaultSyncSettings` 增加 host 输入（逗号分隔） |

---

## 0.2 待拍板项（已清空）

无。§0.1 已覆盖全部决策点（D6、D13 已补充拍板），可直接进入实施。

---

## 0.3 实施顺序

按依赖关系分 7 批，**同批内可并行**（不同文件），跨批必须顺序：

| 批次 | 内容 | 说明 | 状态 |
|------|------|------|------|
| **B1** | M3 + M7（文件层 + 历史版本导出） | 其余所有同步修复的前置：不幂等则 M1/M2 的验证都会被全量重写掩盖；M7 的"不覆盖对端密文"直接复用 M3 的跳过逻辑，同批改 `vault_file` 与 `export_all_entries`；D19 的历史 commit 导出复用同一套 `read_all_entries` 返回值 | `[x]` |
| **B2** | M1 + N1 + N5 + N6（冲突组） | 同改 `resolve_conflict` 与冲突 UI，一次改完；含 D4 / D6 / D9 / D17 | `[x]` |
| **B3** | M2 + M4（删除确认组） | 同改 `import_from_repo` 与 `sync_now` 返回类型；含 D1 / D2 / D3 / D15 / D16，并消费 B1 的 `unreadable_ids`（D18） | `[x]` |
| **B4** | M5 + M6 | 前端 action 补刷新 + 提交信息统一，无依赖 | `[x]` |
| **B5** | N2 + D13（`AppConf` 配置组） | 两个新增字段同批做，共用「四处同步」流程；含 P3 / P4 | `[x]` |
| **B6** | N3 + N4 | 独立小改：事件广播、文案（P4 已并入 B5 完成） | `[x]` |
| **B7** | M7 前端提示（D18-⑤） | B1 遗留：`SyncResult` 需携带 `unreadable_ids` + 归因，UI 显示「N 条条目无法解密」与两条出路按钮 | `[x]` |

最后执行 O3 全量复核，再补 O1 测试与 O2 手工验证。

---

## 0.4 契约变更清单（实施时对照）

> 状态：B1 ~ B7 对应的契约变更**已全部落地**（2026-10-28）。B7 新增 `SyncResult::UnreadableEntries { ids, reason }`，见 §M7 实施状态。

**Rust 侧**

| 变更 | 位置 | 批次 |
|------|------|------|
| `SyncResult::PendingRemovals { entries, totalLocal }` | `assistant-sync` types | B3 |
| `SyncEngine.pending_removals: Mutex<Option<PendingRemovals>>`（内存态，不落盘） | `sync_engine/mod.rs` | B3 |
| 新增命令 `vault_sync_pending_removals()`（供 UI 恢复横幅，可选） | `cmd/vault/sync.rs` | B3 |
| 删除 `import_from_repo` 的 `Refusing to import` 分支 | `sync_engine/mod.rs` | B3 |
| `SyncEngine::resolve_conflict` 增加 `&SecurityDbState` 参数 | `sync_engine/mod.rs` | B2 |
| `SecurityDb::force_import_entry`（`pub(crate)`） | `security_db/crud.rs` | B2 |
| `export_entry` 按 revision 跳过重写 | `vault_file/mod.rs` | B1 |
| `export_entry` / `export_all_entries` 支持 `force`（改主密码后强制重写） | `vault_file/mod.rs` + `sync_engine/mod.rs` | B1 |
| `read_all_entries` 返回 `(readable, unreadable_ids)` | `vault_file/mod.rs` | B1 |
| `ImportReport.unreadable_ids` | `assistant-sync` types | B1 |
| `export_all_entries` 跳过 `unreadable_ids`（D18 不覆盖） | `sync_engine/mod.rs` | B1 |
| `SyncError::VaultKeyMismatch`（对端主密钥不同） | `assistant-sync` types | B1 |
| `.vault-meta.crypto.master_key_salt` 与本地比对归因 | `sync_engine/mod.rs` | B1 |
| `vault_disaster_recovery_export` 增加可选 `commit`（历史版本导出，D19） | `cmd/vault/recovery.rs` | B1 |
| `GitSync::checkout_detached(commit)` + `SyncEngine::checkout_recovery_repo` | `git_sync/mod.rs` + `sync_engine/mod.rs` | B1 |
| `DisasterRecoveryResult.skipped_ids`（导出时被跳过的条目） | `cmd/vault/recovery.rs` | B1 |
| `ConflictFile.title`（供列表展示条目名） | `assistant-sync` types | B2 |
| `ConflictPreview` 拆分 `localError` / `remoteError` | `assistant-sync` types | B2 |
| `EntryPreview` 补两侧 commit 作者与提交时间 | `assistant-sync` types | B2 |
| 新增命令 `vault_conflict_reveal(path, side, field)` | `cmd/vault/sync.rs` | B2 |
| 新增命令 `vault_sync_confirm_removals(app, ids)` | `cmd/vault/sync.rs` | B3 |
| 新增命令 `vault_sync_last_time()` | `cmd/vault/sync.rs` | B5 |
| `AppConf.vault_sync_last_at` + `AppConf.vault_sync_ssh_hosts` | `assistant-config` | B5 |
| `should_use_control_master` 改读配置 | `git_sync/ssh.rs` | B5 |
| 删除 `SyncConfig.branch` | `assistant-sync` types + 3 处构造点 | B5 |
| 广播 `VAULT_META_INTEGRITY_WARNING` | `cmd/vault/recovery.rs` | B6 |
| `SyncResult::UnreadableEntries { ids, reason }` + `UnreadableReason`（`KeyMismatch` / `Corrupt`） | `assistant-sync` types | B7 |
| `pull` / `sync_now` 在 `unreadable_ids` 非空时返回 `UnreadableEntries`（导出后） | `sync_engine/mod.rs` | B7 |
| `vault_sync` 对 `UnreadableEntries` 也写入上次同步时间 | `cmd/vault/sync.rs` | B7 |

**前端侧**

| 变更 | 位置 | 批次 |
|------|------|------|
| `SyncResult` union 增加 `pendingRemovals` | `vault.types.ts` | B3 |
| `syncNow` 处理 `pendingRemovals` → 弹确认框 | `vaultSyncSlice.ts` | B3 |
| 新增 `confirmRemovals(ids)` action | `vaultSyncSlice.ts` | B3 |
| 新增 `pendingRemovals` state + `loadPendingRemovals()`（D16 横幅数据源） | `vaultSyncSlice.ts` | B3 |
| 删除确认 Modal（列出标题 + 强警告分支；实现在 `VaultList` 内，未新建组件） | `VaultList.tsx` | B3 |
| `VaultList` 顶部「未同步到远程」横幅（D16） | `src/vault/` 组件 | B3 |
| `syncPull` / `syncPush` 补 `loadEntries` + `loadSyncReadyness` | `vaultSyncSlice.ts` | B4 |
| 「上次同步」展示 + 「SSH 复用主机」输入 | `VaultSyncSettings.tsx` | B5 |
| `repoState` 文案改用 `repo_state_blocked` | `VaultSyncSettings.tsx` | B6 |
| 删除 `buildConfig()` 的 `branch` | `VaultSyncSettings.tsx` | B5 |
| 监听 `vault:meta-integrity-warning` → `notifyWarning` | `vault/core/useVaultEvents.ts` | B6 |
| `getSyncResultInfo` 增加 `pendingRemovals` 分支 | `vault.types.ts` 或工具 | B3 |
| `ConflictFile.title` + `ConflictPreview.localError/remoteError` 类型 | `vault.types.ts` | B2 |
| 冲突列表显示条目名（替代裸路径） | `VaultList.tsx` | B2 |
| 冲突双栏差异高亮 + 相同字段折叠 | `VaultConflictResolve.tsx` | B2 |
| 冲突页「显示」按钮（复用 `RevealFieldButton` 语义） | `VaultConflictResolve.tsx` | B2 |
| 冲突选项后果说明 + `useLocal`/`useRemote` 二次确认 | `VaultConflictResolve.tsx` | B2 |
| 单侧解密失败按侧降级（不再整体替换双栏） | `VaultConflictResolve.tsx` | B2 |
| 无法解密条目提示（归因文案 + 两条出路按钮） | `VaultSyncSettings.tsx` | B7 |
| `DisasterRecovery` 第 1 步增加可选「历史版本（commit）」输入 + 跳过条目提示 | `src/vault/unlock/DisasterRecovery.tsx` | B1 |
| `disasterRecoveryExport` 透传 `commit` 参数 | `vault.service.ts` / `vaultRecoverySlice.ts` | B1 |
| i18n：删除确认普通/强警告两套文案 + 远端副本标题后缀 | i18n 资源 | B2 / B3 |

> 项目规则提醒：`AppConf` 新增字段必须同时更新 Rust struct（含 `#[serde(default)]`）、`settingsStore.ts` 的 interface、DTO、`From` 实现，禁止用 `Record<string, unknown>` 传递。

---

## 0.5 实施进度（2026-10-28）

### 已完成批次

**B1（M3 + M7 后端 + D19）**

| 交付 | 位置 |
|------|------|
| `export_entry` 按 `revision` 跳过重写；`export_vault_meta` 内容未变不重写（D5） | `vault_file/mod.rs` |
| `read_all_entries` 返回 `(Vec<ImportedEntry>, Vec<String>)`，失败 id 不再静默丢弃 | `vault_file/mod.rs` |
| `ImportReport.unreadable_ids` + `export_all_entries` 跳过这些 id（D18-③） | `assistant-sync` types + `sync_engine/mod.rs` |
| `SyncError::VaultKeyMismatch` + `remote_uses_other_master_key()` salt 比对归因（D18-④） | `sync_engine/mod.rs` |
| `vault_disaster_recovery_export` 可选 `commit`；`checkout_recovery_repo` / `checkout_detached`（D19） | `cmd/vault/recovery.rs` + `git_sync/mod.rs` |
| `DisasterRecoveryResult.skipped_ids` + 前端历史版本输入与跳过提示 | `recovery.rs` + `DisasterRecovery.tsx` / `vaultRecoverySlice.ts` |

**B2（M1 + N1 + N5 + N6）**

| 交付 | 位置 |
|------|------|
| `resolve_conflict` 增加 `&SecurityDbState`，`UseRemote` 走 `force_import_entry_tx`（`revision = max+1`，D4） | `sync_engine/mod.rs` + `security_db/crud.rs` |
| `MergeBoth` 真实现：`Uuid::new_v4()` 新建远端副本条目（D6） | `sync_engine/mod.rs` |
| 收尾自动 `finalize("vault: resolve conflict")`（D9 / N5） | `sync_engine/mod.rs` |
| `ConflictFile.title`、`ConflictPreview` 分侧 `localError` / `remoteError`、`EntryPreview` 补两侧 commit 作者与时间 | `assistant-sync` types |
| `vault_conflict_reveal(path, side, field)`（D17） | `cmd/vault/sync.rs` |
| 冲突列表显示条目名、差异行高亮 + 相同字段折叠、按需显示明文（30s 自动清除）、单侧降级、`useLocal` / `useRemote` 二次确认 | `VaultConflictResolve.tsx` / `VaultList.tsx` |

**B3（M2 + M4，D1 / D2 / D3 / D15 / D16）**

| 交付 | 位置 |
|------|------|
| `PendingRemovalEntry` / `PendingRemovals` + `SyncResult::PendingRemovals` | `assistant-sync` types |
| `pending_removals: Mutex<Option<PendingRemovals>>` 内存态（D15，不落盘） | `sync_engine/mod.rs` |
| `pull` / `sync_now` 检测到 `removed_ids` 时不写 DB、不 commit、不 push，直接返回待确认态 | `sync_engine/mod.rs` |
| `push` 在待确认态下直接返回 `PendingRemovals`（D16 后端阻断乒乓） | `sync_engine/mod.rs` |
| `delete_entries_tx(ids, detail)` 事务内批量软删除 + 审计 `detail = "remote"`（D2） | `security_db/crud.rs` |
| 删除原 `Refusing to import` 护栏分支（M4 并入） | `sync_engine/mod.rs` |
| 命令 `vault_sync_pending_removals` / `vault_sync_confirm_removals` | `cmd/vault/sync.rs` + `lib.rs` |
| 前端 `pendingRemovals` state + `loadPendingRemovals` / `confirmRemovals` + `applySyncResult` 归一化 | `vaultSyncSlice.ts` |
| `VaultList` 顶部常驻横幅 + 确认框（全量删除强警告、默认焦点在「取消」） | `VaultList.tsx` |
| `VaultSyncSettings` 横幅 + 待确认态禁用「推送」 | `VaultSyncSettings.tsx` |
| i18n 中英 11 条文案 | `zh-CN.json` / `en-US.json` |

**B4（M5 + M6）**

- `syncPull` / `syncPush` 成功后补 `loadEntries()` + `loadSyncReadyness()`（M5）
- `sync_now` 提交信息统一为 `vault: sync`（M6 / D10）

**B5（N2 + P3 + P4，D7 / D13 / D14）**

| 交付 | 位置 |
|------|------|
| `AppConf.vault_sync_last_at` / `AppConf.vault_sync_ssh_hosts`（均 `#[serde(default)]`，`new()` 给默认值） | `assistant-config` `app_conf.rs` |
| DTO 字段 + `From` 映射 | `protocol/dto/config.rs` |
| 前端 `AppConf` interface 两字段 | `settings/core/types/confTypes.ts` |
| `persist_sync_last_at()`（成功 / 待确认删除生效后写入毫秒时间戳） | `cmd/vault/mod.rs` + `sync.rs` |
| 命令 `vault_sync_last_time()` 注册 | `cmd/vault/sync.rs` + `lib.rs` |
| `apply_ssh_persist_hosts()` + `APP_SIDE_EFFECTS` 侧效 | `cmd/vault/mod.rs` + `cmd/config.rs` |
| 启动时加载 SSH 主机配置 | `setup.rs` |
| `set_ssh_persist_hosts()` / `resolve_persist_hosts()` + 缓存失效 | `git_sync/ssh.rs` |
| `vaultService.syncLastTime()` + `syncLastAt` / `loadSyncLastTime()`（四个同步动作后刷新） | `vault.service.ts` + `vaultSyncSlice.ts` |
| 底部状态栏「上次同步」+ 同步设置状态行 | `VaultList.tsx` / `VaultSyncSettings.tsx` |
| 「SSH 复用主机」输入（Windows 隐藏）+ 保存时 `dispatchConfPatch` | `VaultSyncSettings.tsx` |
| i18n 中英 4 条文案（`ssh_hosts` / `ssh_hosts_hint` / `last_sync` / `last_sync_never`） | `zh-CN.json` / `en-US.json` |
| 删除 `SyncConfig.branch`（struct + `Default` + interface + `buildConfig()` + mock） | P4 |
| `assistant-config` 补新字段 roundtrip + 「字段缺失取默认」断言 | `tests/app_conf.rs` |

**B6（N3 + N4）**

| 交付 | 位置 |
|------|------|
| `event_name::VAULT_META_INTEGRITY_WARNING` + `VaultMetaIntegrityWarningPayload { message }` | `protocol/events.rs` |
| 恢复密钥解锁成功后校验 `.vault-meta`，`valid == false` 或 `Err` 时广播事件（返回值仍为 `bool`，仅告警不阻断） | `cmd/vault/recovery.rs` |
| 监听 `vault:meta-integrity-warning` → `notifyWarning`（走 `useErrorNotifications` 全局 notification） | `vault/core/useVaultEvents.ts` |
| i18n 中英 2 条文案（`vault.meta.integrity_warning_title` / `_desc`） | `zh-CN.json` / `en-US.json` |
| `readynessText()` 在 `repoState` 时返回 `repo_state_blocked`（带 `{ state }`），与下方提示条一致 | `VaultSyncSettings.tsx` |

**B7（M7 前端提示，D18-⑤）**

| 交付 | 位置 |
|------|------|
| `UnreadableReason`（`KeyMismatch` / `Corrupt`）+ `SyncResult::UnreadableEntries { ids, reason }` | `assistant-sync` types |
| `pull` 在 `unreadable_ids` 非空时返回 `UnreadableEntries`（先 `import_from_repo`，故其余条目已正常导入） | `sync_engine/mod.rs` |
| `sync_now` 在 `export_all_entries`（已跳过 `unreadable_ids`）与 `finalize` 之后返回 `UnreadableEntries`，保证对端密文未被覆盖 | `sync_engine/mod.rs` |
| `unreadable_reason()` 复用 `remote_uses_other_master_key()` 做 salt 比对归因 | `sync_engine/mod.rs` |
| `vault_sync` 对 `UnreadableEntries` 也写入 `vault_sync_last_at` | `cmd/vault/sync.rs` |
| `SyncResult` union + `UnreadableReason` / `UnreadableEntries` 类型 | `vault.types.ts` |
| `getSyncResultInfo` 增加 `unreadableEntries` 分支（`result_unreadable` 文案） | `VaultSyncSettings.tsx` |
| 不可解密告警块：条数 + 归因文案 + 被跳过的 id 列表 + 「用恢复密钥导出数据」/「用本机数据覆盖对端」按钮 | `VaultSyncSettings.tsx` |
| 「用恢复密钥导出数据」→ 切到恢复密钥页签并提示选历史提交；「覆盖对端」→ 复用 `DangerZoneConfirm` 强制推送二次确认 | `VaultSettings.tsx` / `VaultSyncSettings.tsx` |
| `VaultKeyMismatch` 错误不再与告警块重复渲染（同一归因只出现一次） | `VaultSyncSettings.tsx` |
| i18n 中英 7 条文案（`result_unreadable` / `unreadable_title` / 两条归因 / `unreadable_export` / `unreadable_export_hint` / `unreadable_overwrite`） | `zh-CN.json` / `en-US.json` |
| 补 3 项测试：`pull_reports_unreadable_entries_with_corrupt_reason`、`pull_reports_key_mismatch_and_keeps_remote_ciphertext`、`sync_now_skips_export_of_unreadable_entries` | `tests/sync_engine.rs` |

### 验证结果

| 检查 | 结果 |
|------|------|
| `cargo test -p assistant-sync --features assistant-sync/git-tests` | 40 passed（`git_sync` 9 / `sync_engine` 18 / `vault_file` 13），O1 新增 5 项 |
| `cargo test -p assistant-security --features assistant-security/slow-tests` | 全绿（`recovery_key` 9 / `vault_crud` 13 / `vault_crypto` 17 / `vault_sync_db` 7 / `security_db` 10 / `ssh_validate` 4 / `totp` 14 / `password_strength` 12 / `hibp` 5 / `api_key` 7 / `reveal_guard` 3） |
| `cargo test -p assistant-config` | 55 passed（含 B5 新增的两处新字段断言） |
| `cargo check -p assistant-app` / `cargo check --all-targets` | 通过（仅增量编译目录权限警告，与代码无关）；B5 / B6 / B7 / O1 后各复跑通过 |
| `cargo fmt -p assistant-sync -p assistant-security -- --check` | 通过 |
| `npm run lint` / `npm run typecheck` | 通过（B5 / B6 / B7 后各复跑通过） |
| `npm run test`（vitest） | 10 文件 / 181 用例全通过；B6 / B7 后复跑通过 |

### 随 O1 一并修正的两处实现问题

| 问题 | 根因 | 处理 |
|------|------|------|
| `change_master_password` 在测试进程内触发 `STATUS_ACCESS_VIOLATION (0xc0000005)` | `security_db::rekey.rs` 改密末尾直接 `create_key_store()` 走 Windows Hello WinRT（`RoInitialize` + `windows-future` 的 `join()` 在测试多线程下存在竞态），DB 层反向依赖 OS 凭据库 | 从 `rekey.rs` 移除生物识别调用，改由命令层 `vault_change_master_password` 在改密成功后按需 `is_biometric_enrolled` → `disenroll_biometric`（`biometric::get_key_store` 提升为 `pub(super)`） |
| Windows 上 `.enc` / `.vault-meta` 被 Git 自动转换行尾，工作区长期 dirty 并制造伪冲突 | 系统级 `core.autocrlf` 默认开启，而 `vault_file` 始终写 LF | `git_init` / `git_clone` 统一写入 `core.autocrlf=false` / `core.safecrlf=false`，clone 另加 `-c` 前置参数避免检出阶段被转换 |

### 剩余工作

| 批次 | 内容 | 阻塞项 |
|------|------|--------|
| **B5** | N2 上次同步时间 + D13 SSH ControlMaster 主机配置（`AppConf` 四处同步）+ P3 / P4 | **已完成**（2026-10-28） |
| **B6** | N3 `VAULT_META_INTEGRITY_WARNING` 事件 + N4 `repoState` 文案（P4 已并入 B5） | **已完成**（2026-10-28） |
| **B7** | M7 前端提示（D18-⑤）：`SyncResult` 携带 `unreadable_ids` + 归因，UI 显示「N 条条目无法解密」+ 「用恢复密钥导出数据」/「用本机数据覆盖对端」两个出路 | **已完成**（2026-10-28） |
| **O1 ~ O3** | 补齐 §O1 用例、§O2 手工验证、`cargo clippy --workspace` | **O1 已补 12 项并全绿、O3 已执行**（2026-10-28）；剩 §O1 的 `need_to_push` / `GIT_TERMINAL_PROMPT` / D19 三项与 O2 手工验证 |

---

## 阶段 M — 同步正确性修复（P0/P1）

### M1 `[x]` 冲突解决结果不落库，下一次同步被本地覆盖

**现象**

[resolve_conflict](../../src-tauri/crates/assistant-sync/src/sync_engine/mod.rs#L317-L335) 只做 `git checkout --ours/--theirs` + `git add`，无剩余冲突时 `commit_all`，**全程不触碰 DB**。

若用户选「使用远程」：
1. 仓库文件 = 远端版本（revision `R`），DB 仍是本地版本（revision `N`）；
2. 下一次 `sync_now` → `import_from_repo` → [upsert_imported_in](../../src-tauri/crates/assistant-security/src/security_db/crud.rs#L688-L720) 在 `R <= N` 时返回 `SkippedStale`；
3. 紧接 `export_all_entries` 把 DB 的本地版本重新写回 `.enc` 并 commit/push。

结果：**「使用远程」被静默还原为本地版本**。双端从同一基线各自修改时 `N == R`，必然命中。

**修复方案**

> 已拍板：**D4**（`revision = max(local, remote) + 1`，`force_import_entry` 限 `pub(crate)`）、**D9**（收尾自动 push）。

- `SyncEngine::resolve_conflict` 增加 `&SecurityDbState` 参数（命令层 `vault_resolve_conflict` 已有 `app`，可 `app.state()` 取）；
- `UseRemote`：取 stage 3 内容（`git show :3:<path>` 或 checkout 后读文件）→ `decode_entry_file` → 走**强制落库**路径（新增 `SecurityDb::force_import_entry`，绕过 revision 比较，写入 `revision = max(local, remote) + 1`）→ 事务内 `export_entry` 重建 `.enc`，确保后续 `import` 不会回退；
- `UseLocal`：DB 已是本地版本，但需补一次 `export_entry`，使磁盘密文与 DB 同步（避免 checkout 出来的旧密文与 DB 记录不一致）；
- 收尾：`commit_all` 后调用 `finalize("vault: resolve conflict")`（见 N5）。

**验收**：双端同基线改同一条目 → 冲突 → 选「使用远程」→ 条目详情为远程内容 → 再点「立即同步」内容不变 → 另一设备 pull 后一致。

**参考**：`tests/sync_engine.rs` 现有 `conflict_preview_decrypts_both_sides_with_masking` 已能构造真实双端冲突，可复用其建库流程。

**实施状态（2026-10-28，`[x]` 已完成）**：`resolve_conflict` 已接收 `&SecurityDbState`；`UseRemote` 走 `force_import_entry_tx`（`revision = max+1`）；`UseLocal` 补一次 `export_entry`；收尾 `finalize("vault: resolve conflict")` 自动 commit + push。

---

### M2 `[x]` 删除不传播：远端删除被本地复活，且与墓碑形成乒乓

**现象**

- [import_from_repo](../../src-tauri/crates/assistant-sync/src/sync_engine/mod.rs#L206-L223) 对 `removed_ids` 仅 `log::warn` + 随 `ImportReport` 返回，**不修改 DB**（H8 的"同步流程不擅自修改本地数据库"）；
- [export_all_entries](../../src-tauri/crates/assistant-sync/src/sync_engine/mod.rs#L602-L635) 只删除 `deleted_at IS NOT NULL` 的 id 对应文件，对"远端已删但本地仍存活"的条目会**重新导出**。

于是形成循环：A 删除并 push → B pull（文件消失）→ B 的 DB 仍存活 → B 导出重建文件并 push → A pull 到文件但本地是墓碑 → `SkippedLocallyDeleted` → A 导出再删 → push → B 再复活……**每次同步都产生一次删除/恢复的乒乓提交**。

**修复方案（已拍板 D1 / D2 / D3 / D15 / D16：用户确认后软删除）**

1. [import_from_repo](../../src-tauri/crates/assistant-sync/src/sync_engine/mod.rs#L206-L223) 检测到 `removed_ids` 非空时**不写 DB、不 commit、不 push**，直接返回 `SyncResult::PendingRemovals { entries: [{ id, title }], totalLocal }`（`title` 供确认框展示，从 DB 读取）；
2. **待确认态存内存**（D15）：`SyncEngine` 增加 `pending_removals: Mutex<Option<PendingRemovals>>` 字段，不落盘、不加 `AppConf` 字段。重启后丢失属预期行为——下次 `sync_now` 的 pull 会重新检测到同一批 `removed_ids`，**幂等自愈**；
3. 前端 `vaultSyncSlice.syncNow` 收到该变体 → 弹确认框列出条目标题 → 用户确认后调新增命令 `vault_sync_confirm_removals(app, ids)`：
   - 事务内软删除（`deleted_at = now()`，`revision` 不变）+ 审计日志 `VaultEntryDeleted` 的 `detail = "remote"`（**不新增 DB 列**，D2）；
   - 随后 `export_all_entries`（墓碑 → 删文件）→ `finalize("vault: sync")`；
   - 清空内存态；
4. 用户取消 → 不改动任何数据，**内存态保留**并**禁用 push**（否则本地导出会把条目复活，形成乒乓）；下次 `sync_now` 再次提示（幂等）；
5. **「尚未同步到远程」必须显式提醒**（D16）：待确认态存在时，`VaultSyncSettings` 状态行 + `VaultList` 顶部横幅常驻显示「有 N 条远端删除待确认，当前未同步到远程」，并带「查看并确认」入口。**不能只禁用 push 按钮而不解释**——否则用户会误以为同步已完成。确认或取消后横幅消失；
6. 安全性：`before` 集合只包含 pull 前仓库中已存在的文件，因此不会误删"本设备离线期间新建、尚未 push"的条目（它们不在 `before` 内）；
7. `export_all_entries` 现状已满足"墓碑 → 删文件"。

> H8 原则**无需修订**：同步流程本身仍不修改数据库，落库动作发生在用户明确确认之后。

**不采用方案 B**：`deletions/deleted.json` 在 P2 已被彻底移除，不回头。

**验收**
- A 删除 → push → B pull → B 弹出确认框列出条目名 → 确认后进回收站 → B 不再复活 → 连续两次同步无新增提交；
- 取消 → B 保持原样、push 被禁用、**顶部横幅显示「当前未同步到远程」**；
- 取消后重启应用 → 横幅消失（内存态丢失），但下次 `sync_now` 重新弹出确认框（自愈）。

**实施状态（2026-10-28，`[x]` 已完成）**

| 方案项 | 状态 | 位置 |
|--------|------|------|
| ① `import_from_repo` 检测到 `removed_ids` 即返回 `PendingRemovals`（不写 DB / 不 commit / 不 push） | 已完成 | `sync_engine/mod.rs` |
| ② `pending_removals: Mutex<Option<PendingRemovals>>` 内存态（D15） | 已完成 | `sync_engine/mod.rs` |
| ③ `vault_sync_confirm_removals` → `delete_entries_tx` 事务批量软删除 + 审计 `detail = "remote"`（D2） | 已完成 | `cmd/vault/sync.rs` / `security_db/crud.rs` |
| ④ 取消后保留内存态并拒绝 `push`（`push` 入口直接返回 `PendingRemovals`） | 已完成 | `sync_engine/mod.rs` |
| ⑤ 常驻横幅 + 「查看并确认」入口（D16） | 已完成 | `VaultList.tsx` / `VaultSyncSettings.tsx` |
| ⑥ `before` 集合仅含 pull 前已存在的文件，不误删离线新建条目 | 已完成 | `sync_engine/mod.rs` |
| ⑦ 墓碑 → 删文件（复用 `export_all_entries`） | 已完成 | `sync_engine/mod.rs` |

> 注：`SyncEngine` 另暴露 `pending_removals()` 供 `vault_sync_pending_removals` 命令读取，用于前端刷新（重启后为空，符合 D15）。

---

### M3 `[x]` 导出不幂等：每次同步全量重写 → 必然 commit + push

**现象**

- `export_all_entries` 无条件对每个 snapshot 调 [export_entry](../../src-tauri/crates/assistant-sync/src/vault_file/mod.rs#L53-L115)；
- `export_entry` 每次 `VaultCrypto::encrypt_field` 生成**新随机 nonce**（[vault_crypto/mod.rs](../../src-tauri/crates/assistant-security/src/vault_crypto/mod.rs#L81-L87)），文件字节必然变化；
- `export_vault_meta` 每次写 `exported_at = now`（[vault_file/mod.rs](../../src-tauri/crates/assistant-sync/src/vault_file/mod.rs#L184-L209)）。

因此零变更时 `sync_now` 也一定产生"全库重写"提交，[need_to_push](../../src-tauri/crates/assistant-sync/src/git_sync/mod.rs#L290-L303) 的 30s/hash 优化失效（本地 hash 恒 != upstream），永远 push。

**影响**：Git 历史被无意义提交淹没（直接损害 L5 历史还原面板可用性）、每次同步全量上传、revision 语义未被利用。

**修复方案（已拍板 D5）**

1. `export_entry` 增加"内容未变则跳过"：`EntryFile` 已含 `revision`，读取现有文件头部 `revision`，若 `== snapshot.revision` → **不重写**（**只比 revision，零解密**，不比对 blob 摘要）；
2. `.vault-meta`：写前比对旧文件，除 `exported_at` / `integrity` 外内容一致则**整文件不重写**（`exported_at` 语义变为「上次内容变更时间」）；
3. 幂等后 `finalize` 依赖的 `git_has_changes` 自然为 false，不再产生提交。

**必须同时处理的例外（否则幂等会直接破坏改主密码流程）**

改主密码时 [change_master_password](../../src-tauri/crates/assistant-security/src/security_db/rekey.rs#L15-L80) 会重新加密 DB 中所有条目，**但 `revision` 不变**；随后 [vault_change_master_password](../../src-tauri/src/cmd/vault/entries.rs#L173-L213) 调 `export_all_entries` 重写仓库文件。若按 revision 跳过，密文将保持旧密钥 → 对端（用新密码）无法解密，且 `.vault-meta` 已用新密钥签名 → 同步直接卡在完整性校验。

因此：`export_entry` / `export_all_entries` 增加 `force: bool` 参数，`vault_change_master_password` 传 `true`（其余调用点传 `false`）。`force` 的判据来源是 `.vault-meta.crypto.master_key_salt` 与本地 `master_key_salt` 不同——即"密钥已轮换"，此时**必须**全量重写。

**验收**：连续两次 `sync_now` 无条目变更 → 第二次 `git log` 无新增提交，`readyness = Ready`；改主密码后 → 仓库所有 `.enc` 密文更新、对端用新密码可解密。

**实施状态（2026-10-28，`[x]` 已完成）**：`export_entry` 读取现有文件 `revision` 相同即跳过；`export_vault_meta` 除 `exported_at` / `integrity` 外内容一致不重写；`force` 参数已贯通（改主密码路径传 `true`）。测试见 `vault_file.rs` 的 3 个幂等用例。

---

### M4 `[x]` 防误删护栏误伤"对端全量删除" — **已拍板 D3：并入 M2**

**现象**

[import_from_repo](../../src-tauri/crates/assistant-sync/src/sync_engine/mod.rs#L176-L183) 在 `before` 非空、`imported` 为空、本地条目数 > 0 时直接返回 `Err("Refusing to import: repository entries vanished...")`。当对端**合法地**删光所有条目并推送时，本设备 pull 永久失败，只能手工处理。

**结论**：不再需要独立护栏。M2 的 `PendingRemovals` 确认流程已覆盖所有删除场景，原护栏的 `Err` 分支**直接删除**，改为：

- `removed_ids` 非空 → 一律进入 `PendingRemovals`（D3 取消阈值判断）；
- `removed == totalLocal`（对端删光）→ 确认框使用**强警告文案**（"对端删除了全部 N 条条目"），并默认焦点落在「取消」；
- 其余情况 → 普通文案，列出被删条目标题。

**验收**：对端删除全部条目 → 本设备 pull 弹出强警告确认 → 确认后进入回收站；取消后保持本地不变、顶部横幅提示「当前未同步到远程」（D16），可再次同步。

---

### M5 `[x]` 单独 pull / push 后前端不刷新（P1）

**现象**：[syncPull](../../src/vault/core/slices/vaultSyncSlice.ts#L74-L84) / [syncPush](../../src/vault/core/slices/vaultSyncSlice.ts#L85-L95) 只 `set({ syncResult, syncing: false })`，不刷新条目与 readyness；仅 [syncNow](../../src/vault/core/slices/vaultSyncSlice.ts#L96-L109) 会 `loadEntries()` + `loadSyncReadyness()`。

**修复方案**：两个 action 末尾同样 `await get().loadEntries(); await get().loadSyncReadyness();`（pull 后条目可能被更新 / 软删除，见 M2）。

**验收**：`VaultSyncSettings` 点「拉取」→ 列表条目即时反映远端变更，冲突态即时出现。

**实施状态（2026-10-28，`[x]` 已完成）**：`syncPull` / `syncPush` 末尾均已 `await get().loadEntries()` + `await get().loadSyncReadyness()`。

---

### M6 `[x]` `sync_now` 提交信息统一（D10，P2）

**现状**：`sync_now` 提交信息为 `format!("Sync at {}", rfc3339)`，与 L6 去标题化后的 `vault: create {id}` 风格不一致，历史还原面板会混入英文时间戳条目。

**修复方案（已拍板 D10）**：统一为 `vault: sync`。

**实施状态（2026-10-28，`[x]` 已完成）**：`sync_now` 与 `confirm_removals` 的 `finalize` 均使用 `"vault: sync"`。

---

### M7 `[~]` 无法解密的条目被静默跳过，且可能被本地导出覆盖（D18 / D19，P0）

**现状：三个层次都缺信息**

1. **读取层静默丢弃**：[read_all_entries](../../src-tauri/crates/assistant-sync/src/vault_file/mod.rs#L147-L165) 对 `import_entry` 的失败只 `log::warn!("Skipping unreadable vault entry file ...")`，**不向上返回**。调用方无法知道"有文件读不了"，只看到"导入 N 条"。
2. **归因错误**：主密码不同的仓库，`.vault-meta` 的 HMAC 是用**对端**主密钥签的，本机校验必然失败 → [import_from_repo](../../src-tauri/crates/assistant-sync/src/sync_engine/mod.rs#L164-L168) 返回 `VaultMetaIntegrityFailed("HMAC signature mismatch — vault meta may be tampered")`。用户看到的是**「可能被篡改」**，真实原因是**密钥不匹配**。
3. **可能反向覆盖**：`.vault-meta` 校验通过但个别条目解不开时（例如部分文件来自旧密钥、导出中途失败留下的混合状态），`sync_now` 会接着执行 [export_all_entries](../../src-tauri/crates/assistant-sync/src/sync_engine/mod.rs#L602-L635)，用**本地密钥**重新加密并覆盖对端文件 → 对端密文被永久替换。B1 的 D5 幂等在 revision 相同时能挡住一部分，但只要本机该条目 revision 更高就会覆盖。

**当前用户能不能解密？——能，但走的是灾难恢复导出，且需要指定历史版本（D19）**

- 条目密钥 = `HKDF(master_key, entry_id)`（[derive_entry_key](../../src-tauri/crates/assistant-security/src/vault_crypto/mod.rs#L65-L70)），`master_key` = `Argon2(主密码, master_key_salt)`；
- 对端 `master_key_salt` 在仓库里（`.vault-meta.crypto.masterKeySalt`），但应用**没有**「输入另一主密码 + 外部 salt 派生密钥」的入口——这条不做（D19）；
- **但已有可用路径**：[vault_disaster_recovery_export](../../src-tauri/src/cmd/vault/recovery.rs#L180-L235) 会把仓库 clone 到临时目录，读 `.vault-meta.emergency`，用**恢复密钥**解出该版本的主密钥，导出明文 JSON。全过程不需要知道主密码；
- **限制**：`vault_change_master_password` 会调 [clear_emergency_data](../../src-tauri/src/cmd/vault/entries.rs#L189-L195)（审计 `RecoveryKeyInvalidated`），因此改密码之后 HEAD 的 `.vault-meta.emergency` 为 `None` → 恢复密钥在 HEAD 必然失败。而**改密码之前的历史提交仍带 emergency blob**，所以「选历史版本导出」是**必需能力**，不是可选优化；
- 若用户从未生成过恢复密钥，则确实无路可走，只能保留本机数据或覆盖对端——这种情况必须显式警告。

**修复方案（已拍板 D18 / D19）**

1. `read_all_entries` 改为返回 `(Vec<ImportedEntry>, Vec<String>)`，失败 id 一并返回，**不再静默**；
2. `ImportReport` 增加 `unreadable_ids`，`sync_now` 的 `SyncResult` 携带（或复用 `SyncResult::Error` 附结构化原因）；`disaster_recovery_export` 同样返回被跳过的 id，避免"导出成功但少了几条"；
3. **归因**：读 `.vault-meta.crypto.master_key_salt` 与本地 `master_key_salt` 比对——
   - 不同 → `SyncError::VaultKeyMismatch`，文案「对端仓库使用另一套主密钥（可能在其他设备改过主密码）。本机无法解密」；
   - 相同 → `EntryDecryptFailed`，文案「条目文件损坏或格式不兼容」，并列出 id；
4. **禁止覆盖**：`export_all_entries` 跳过 `unreadable_ids` 中的 id（该条目本轮不写盘），保证对端密文不被本地旧密钥覆盖；
5. **UI 出路（只引导，不新建流程）**：同步结果区显示「N 条条目无法解密」+ 归因 + 两个动作——
   - 「用恢复密钥导出数据」→ 跳转现有 `DisasterRecovery` 面板，并提示选择**改密码之前的历史提交**；
   - 「用本机数据覆盖对端」→ 二次确认（明确告知对端密文将被替换）；
6. `vault_disaster_recovery_export` 增加可选 `commit: Option<String>`：clone 后 `checkout` 该提交再导出；前端 `DisasterRecovery` 第 1 步增加可选「历史版本」输入（留空 = HEAD）；
7. `.vault-meta` 完整性失败时，先做 salt 比对再决定文案，避免把密钥不匹配报成"被篡改"。

**不做**：新增「输入对端主密码接管/重新克隆仓库」的流程——灾难恢复导出已覆盖该需求，且少一个需要长期维护的密钥输入界面（D19）。

**验收**
- B 改主密码并 push → A 用旧密码 pull → 提示「对端使用另一套主密钥」而非「可能被篡改」，且 A 的仓库文件**未被覆盖**；
- A 在灾难恢复面板填入 git URL + 恢复密钥 + 改密码前的 commit → 导出成功，条目数与历史版本一致；
- 恢复密钥在 HEAD（改密码后）导出 → 明确报「该版本未设置恢复密钥」，而不是含糊的 clone 失败；
- 人为破坏单个 `.enc` → 提示「条目文件损坏」并列出 id，其余条目正常同步；
- 上述两种情况下，连续同步不产生覆盖对端密文的提交。

**实施状态（2026-10-28，`[x]` 已完成）**

| 方案项 | 状态 | 位置 |
|--------|------|------|
| ① `read_all_entries` 返回 `(readable, unreadable_ids)` | 已完成 | `vault_file/mod.rs` |
| ② `ImportReport.unreadable_ids` + `disaster_recovery_export` 返回 `skipped_ids` | 已完成 | `assistant-sync` types / `cmd/vault/recovery.rs` |
| ③ 导出跳过 `unreadable_ids`（禁止覆盖对端密文） | 已完成 | `sync_engine/mod.rs` `export_all_entries` |
| ④ salt 比对归因（`VaultKeyMismatch` vs 文件损坏） | 已完成 | `sync_engine/mod.rs` `remote_uses_other_master_key` / `attributed_integrity_error` / `unreadable_reason` |
| ⑤ UI 提示与出路按钮 | 已完成（B7） | `VaultSyncSettings.tsx` / `VaultSettings.tsx` |
| ⑥ `vault_disaster_recovery_export` 可选 `commit` + 前端历史版本输入 | 已完成 | `cmd/vault/recovery.rs` / `DisasterRecovery.tsx` |
| ⑦ meta 完整性失败先 salt 比对再定文案 | 已完成 | `sync_engine/mod.rs` |

**⑤ 落地细节（B7）**：新增 `SyncResult::UnreadableEntries { ids, reason }`（`reason` 为 `keyMismatch` / `corrupt`），`pull` 与 `sync_now` 在 `unreadable_ids` 非空时返回；前端同步设置页渲染红色告警块，展示条数、归因文案与被跳过的 id，并提供两个按钮——「用恢复密钥导出数据」切到恢复密钥页签并提示选择改密码之前的历史提交，「用本机数据覆盖对端」复用强制推送的二次确认。`VaultKeyMismatch` 异常（整库都解不开的场景）与告警块同源，故只渲染一处，避免重复提示。

---

## 阶段 N — 功能补全（P1/P2）

### N1 `[x]` `MergeBoth` 真实现（H10.2）— **已拍板 D6：实现，创建为新条目**

**现状**：[resolve_conflict](../../src-tauri/crates/assistant-sync/src/sync_engine/mod.rs#L326-L329) 的 `MergeBoth` 直接 `git.resolve_conflict_local(path)`，与 `UseLocal` **完全等价**；而 `VaultConflictResolve.tsx` 提供了该选项（`merge_both` 词条已存在）。当前属于**欺骗性 UI**。

**方案（已拍板）**：取 stage 3（theirs）解密 → **创建为新条目**：
- 新 id 用 `Uuid::new_v4()`，**不用** `{id}-remote-{ts}`（避免污染 id 语义与 `parse_entry_id` 约束）；
- `revision = 全库 max + 1`（与 D4 同规则，保证其他设备正常接受）；
- 标题加后缀区分来源（如 `{title} (远端)`，词条走 i18n）；
- 插入 DB 并导出 `.enc`；随后 `resolve_conflict_local` 保留本地为原 id。

**验收**：冲突时选「合并保留两者」→ 列表出现两条（本地 + 远端副本）→ 另一设备 pull 后同样两条。

> 说明：P2 文档 H10.2 中"解决后不自动完成 merge commit"的备注已**失效**——L2.3 已实现无剩余冲突时 `commit_all`。

**实施状态（2026-10-28，`[x]` 已完成）**：`MergeBoth` 取 stage 3 解密 → `Uuid::new_v4()` 新建条目（`revision = 全库 max + 1`），本地保留原 id；新条目随后 `export_entry` 写入 `.enc`。

---

### N2 `[x]` 上次同步时间（H14.2 / H16.1）

**现状**：前端无数据源。后端已有 [git_last_pull_time](../../src-tauri/crates/assistant-sync/src/git_sync/git.rs#L261-L266)（读 `FETCH_HEAD` mtime），但仅被 `need_to_push` 使用，无命令暴露；`SyncConfig` / `SyncResult` 也不携带时间。

**方案（已拍板 D7）**：`AppConf` 新增 `vault_sync_last_at: Option<i64>`（毫秒），每次成功 pull/push 后写入（库锁定时仍可读，`FETCH_HEAD` mtime 作回退）；新增命令 `vault_sync_last_time() -> Option<i64>`；`VaultSyncSettings` 状态行 + `VaultList` 底部状态栏展示 `formatRelativeTime`。按项目规则需同步 Rust struct + 前端 interface + DTO + `From` 四处。

**验收**：同步成功后 UI 显示"上次同步：x 分钟前"；重启应用后仍在。

**实施状态（2026-10-28，`[x]` 已完成）**：`AppConf.vault_sync_last_at` + DTO + 前端 interface 四处同步；`persist_sync_last_at()` 在 `vault_sync` 的 `Success` / `UpToDate` 分支与 `vault_sync_confirm_removals`（`count > 0`）后写入；`vault_sync_last_time()` 已注册；`vaultSyncSlice` 新增 `syncLastAt` 与 `loadSyncLastTime()`（四个同步动作成功后刷新）；`VaultSyncSettings` 状态行与 `VaultList` 底部状态栏均用 `formatRelativeTime` 展示。

---

### N3 `[x]` 恢复密钥解锁未做 meta 完整性告警（J3.4）

**现状**：[vault_unlock_with_recovery_key](../../src-tauri/src/cmd/vault/recovery.rs#L34-L75) 直接解密 master key，**未调用** `verify_vault_meta_integrity`；只有 `disaster_recovery_export`（同文件 `:143-175`）设置了 `integrity_warning`。

**方案（已拍板 D8：发事件，不改返回类型）**：`vault_unlock_with_recovery_key` 返回值**保持 `bool`**；解锁成功后校验 `.vault-meta`，失败则广播 `VAULT_META_INTEGRITY_WARNING` 事件（复用现有 `emit_broadcast` 机制），前端 store 监听后弹 Modal 提示"元数据可能被篡改"，**仅告警不阻断**。

**验收**：手工篡改 `.vault-meta` 后恢复密钥解锁 → 出现告警且可继续。

**实施状态（2026-10-28，`[x]` 已完成）**：`event_name::VAULT_META_INTEGRITY_WARNING` + `VaultMetaIntegrityWarningPayload { message }` 已加入 [events.rs](../../src-tauri/src/protocol/events.rs)；`vault_unlock_with_recovery_key` 在 `result == true` 后取 master key 调 `verify_vault_meta_integrity`，`valid == false` 或 `Err` 均广播事件（仅告警，返回值仍为 `bool`）；前端 [useVaultEvents.ts](../../src/vault/core/useVaultEvents.ts) 监听后经 `notifyWarning` 走 `useErrorNotifications` 的全局 notification，新增词条 `vault.meta.integrity_warning_title` / `vault.meta.integrity_warning_desc`（中英）。

---

### N4 `[x]` `repoState` 文案落到 `status_unknown`（H14.2，P2）

**现状**：[readynessText](../../src/vault/settings/VaultSyncSettings.tsx#L195-L200) 对 `{ repoState }` 一律返回 `vault.sync.status_unknown`，而实际语义是"仓库处于 merge/rebase/cherry-pick 中间态"，且同页下方已有 `repo_state_blocked` 提示条，两处文案不一致。

**方案**：`repoState` 时返回 `vault.sync.repo_state_blocked`（词条已存在），或按 `merge` / `rebase` / `cherryPick` 细分。

**实施状态（2026-10-28，`[x]` 已完成）**：`readynessText()` 在 `isReady` 之后判断 `repoState`，命中时返回 `vault.sync.repo_state_blocked` 并传入 `{ state: repoState }`，顶部 Alert 与下方提示条文案一致；`status_unknown` 仅作为兜底保留。

---

### N5 `[x]` 冲突解决后不 push（P2）— **已拍板 D9，并入 M1**

**现状**：`resolve_conflict` 只 `commit_all`，不 push，需用户再点「立即同步」。

**方案（已拍板）**：与 M1 一并处理——解决且无剩余冲突后直接 `finalize("vault: resolve conflict")`（commit + push）；前端 `resolveConflict` 成功后补 `loadEntries()`。实施时随 M1 一起完成，不单独排期。

**实施状态（2026-10-28，`[x]` 已完成）**：已随 M1 一并落地，`VaultConflictResolve` 关闭后列表即时刷新。

---

### N6 `[x]` 冲突解决界面信息不足，用户只能盲选（D17，P1）

**现状（6 处缺口，均已在代码中确认）**

| # | 缺口 | 代码事实 | 用户处境 |
|---|------|----------|----------|
| 1 | 冲突列表只有文件路径 | [ConflictFile](../../src-tauri/crates/assistant-sync/src/types/mod.rs#L86-L90) 仅 `path` / `local_hash` / `remote_hash`，[VaultList.tsx](../../src/vault/VaultList.tsx#L347-L352) 直接渲染 `file.path` | 看到 `entries/<uuid>.enc`，**不知道是哪个账号**；两个 hash 未渲染，等于白传 |
| 2 | 密码与受保护字段全掩码 | [mask_entry_preview](../../src-tauri/crates/assistant-sync/src/sync_engine/mod.rs#L27-L61) 把 `password` / `totp_secret` / `is_secret` 自定义字段全替换为 `MASKED` | 密码管理器里最关键的字段看不见，**无法判断哪边是新密码** |
| 3 | 无差异高亮 | [VaultConflictResolve.tsx](../../src/vault/settings/VaultConflictResolve.tsx#L79-L148) 两列 `Descriptions` 平铺 15 个字段 | 差异全靠肉眼逐个比对 |
| 4 | 缺可信的"哪边更新"依据 | `EntryPreview.commit` 被硬编码为字符串 `"conflict"`（[mod.rs:512](../../src-tauri/crates/assistant-sync/src/sync_engine/mod.rs#L512)），只有条目自带 `updatedAt` 与 `revision` | 设备时钟不准时 `updatedAt` 不可信；git 侧的作者/提交时间才是硬证据，但未取出 |
| 5 | 单侧解密失败会吞掉另一侧 | `preview.error` 为单值，渲染时 `preview?.error` 分支**整体替换**双栏（[VaultConflictResolve.tsx:186-193](../../src/vault/settings/VaultConflictResolve.tsx#L186-L193)） | 远端解不开时，本地可读版本也一起消失 |
| 6 | 选项后果无说明 | 四个 Radio 只有标签，无二次确认 | 不知道「保留两者」会新建 UUID 条目（D6）、「使用远程」会丢弃本地修改 |

**修复方案（已拍板 D17）**

1. `ConflictFile` 增加 `title: Option<String>`：`conflicts()` 按 `parse_entry_id` 从 DB 查标题，查不到再回退路径；
2. 新增命令 `vault_conflict_reveal(path, side, field)`：默认掩码，点击才解密该侧该字段，30s 后前端清除——与 [RevealFieldButton](../../src/vault/detail/RevealFieldButton.tsx) + `revealPassword` 完全同一套安全模型（用户主动触发、临时内存、不落库、不写日志）；
3. 前端逐字段对比：不同字段加底色 + `Tag`「仅本地」/「仅远端」，相同字段折叠；
4. 补可信时间：用 `git log -1 --format=%an|%aI` 取两侧 commit 作者与提交时间，替换硬编码 `"conflict"`；UI 显示「本地：张三 10:32 / 远端：李四 09:15」；
5. `ConflictPreview` 拆成 `local_error` / `remote_error`，**按侧降级**：哪侧解不开只在该侧显示「此版本无法用当前密钥解密」，另一侧照常展示；
6. 每个 Radio 下补一行后果说明；`useLocal` / `useRemote` 走二次确认（提示「将丢弃另一侧的修改」）。

**不做**：批量解决（N 个冲突一次处理）——与 D3 的一次性确认语义不同，冲突必须逐条看内容，批量等于盲选。

**验收**：冲突列表显示条目名；点击密码旁「显示」→ 仅该侧该字段出明文、30s 后自动隐藏；两侧差异行高亮、相同字段折叠；一侧解密失败时另一侧仍完整展示；选「使用本地/远程」弹二次确认说明丢弃内容。

**实施状态（2026-10-28，`[x]` 已完成）**

| # | 方案项 | 状态 | 位置 |
|---|--------|------|------|
| 1 | `ConflictFile.title` + 列表显示条目名（回退路径） | 已完成 | `assistant-sync` types / `VaultList.tsx` |
| 2 | `vault_conflict_reveal(path, side, field)` 按需显示 + 30s 自动清除 | 已完成 | `cmd/vault/sync.rs` / `VaultConflictResolve.tsx` |
| 3 | 逐字段差异高亮 + `diffFields` + 相同字段折叠 | 已完成 | `ConflictPreview.diff_fields` / `VaultConflictResolve.tsx` |
| 4 | 两侧 commit 作者与提交时间（替换硬编码 `"conflict"`） | 已完成 | `EntryPreview.commit_author` / `commit_time` |
| 5 | `localError` / `remoteError` 分侧降级 | 已完成 | `ConflictPreview` / `VaultConflictResolve.tsx` |
| 6 | 选项后果说明 + `useLocal` / `useRemote` 二次确认 | 已完成 | `VaultConflictResolve.tsx` |

---

## 阶段 O — 验证补全

### O1 `[~]` 单元 / 集成测试缺口（P1）

现有用例：`tests/git_sync.rs` 9 项、`tests/sync_engine.rs` 18 项、`tests/vault_file.rs` 13 项，合计 **40 项**（P2 文档记录的 21 项已过期）。B1 ~ B4 已补 5 项：`export_entry_skips_rewrite_when_revision_unchanged`、`export_entry_rewrites_when_revision_changes`、`export_vault_meta_skips_rewrite_when_content_unchanged`、`read_all_entries_reports_unreadable_ids`、`pull_stages_pending_removals_and_confirm_soft_deletes`。

O1 收尾又补 5 项（`tests/sync_engine.rs`）：`resolved_use_remote_does_not_regress_on_next_sync`、`resolved_merge_both_creates_remote_copy_entry`、`remote_deleting_every_entry_stages_all_removals_and_survives_cancel`、`conflict_preview_reports_only_the_undecryptable_side`、`force_export_rewrites_entries_after_master_password_change`；同时核对存量用例 `tests/ssh_validate.rs` 4 项与 `tests/recovery_key.rs` 9 项已覆盖下表的 SSH / 恢复密钥缺口。

测试结构：`assistant-sync/tests/common/mod.rs` 统一公共辅助（常量、模板库、快照构造、仓库构造、`serial_guard()` 串行锁），`Cargo.toml` 新增独立门控 `git-tests`，`sync_engine.rs` / `git_sync.rs` 头部加 `#![cfg(feature = "git-tests")]`；`assistant-security/tests/common/mod.rs` 提供 `init_vault` / `temp_db_path` / `cleanup`。

| 目标 | 对应 P2 项 | 现状 | 建议用例 | 状态 |
|------|-----------|------|----------|------|
| `need_to_push` 30s/hash 优化 | H19.7 | 代码已实现，**无测试** | 构造本地与 upstream 同 hash + 新鲜 `FETCH_HEAD`，断言 `false` | `[ ]` |
| `GIT_TERMINAL_PROMPT=0` | H19.16 / K2.10 | 代码已注入环境变量，**无测试** | `ls_remote` 指向需认证 URL，断言快速失败不挂起（带超时断言） | `[ ]` |
| SSH 密钥格式校验 | I7.7 / I7.8 | 已补 `tests/ssh_validate.rs` 4 项 | OpenSSH / PEM / PPK / 垃圾字节四组 | `[x]` |
| 恢复密钥格式与错误密钥 | J8.2 / J8.4 | 已补 `tests/recovery_key.rs`（格式 4 组 8 位、长度 / Base32 拒绝、错误密钥 `InvalidRecoveryKey`、改密码后失效） | Base32 长度 + 分隔符格式；错误密钥返回 `InvalidRecoveryKey` | `[x]` |
| 永久清理 | L7.7 | `purge_deleted_removes_only_soft_deleted_entries` 覆盖 DB 侧（墓碑清空、正常条目保留）；`.enc` 文件删除未断言 | 删除 → purge → DB 无墓碑行、`.enc` 不存在 | `[~]` |
| 冲突落库（M1） | H19.12 | 已补 `resolved_use_remote_does_not_regress_on_next_sync` | 双端冲突 → `useRemote` → 再 `sync_now` 内容不回退 | `[x]` |
| 删除待确认（M2/D1） | K3.5 | `pull_stages_pending_removals_and_confirm_soft_deletes` | A 删 → B pull → 返回 `PendingRemovals` 且 DB 未变 → 确认 → 进回收站、无乒乓提交 | `[x]` |
| 删除取消（M2/D1） | K3.5 | `remote_deleting_every_entry_stages_all_removals_and_survives_cancel` 覆盖取消后 DB 不变、`pending_removals` 内存态保留；push 被拒与前端横幅仍属 O2 手工项 | 取消 → DB 不变、push 被拒、内存态保留（D15）、横幅可见（D16） | `[~]` |
| 对端全删（M4/D3） | K3.5 | 已补 `remote_deleting_every_entry_stages_all_removals_and_survives_cancel`（`pending.entries == 2`、`total_local == 2`） | A 删光 → B pull → `PendingRemovals` 且 `removed == totalLocal` | `[x]` |
| MergeBoth（N1） | H19.12 | 已补 `resolved_merge_both_creates_remote_copy_entry` | 冲突 → mergeBoth → 两条条目、新 id 为 UUID、标题带远端后缀 | `[x]` |
| 导出幂等（M3） | — | 3 项已补 | 连续两次 `sync_now` 无新增提交 | `[x]` |
| SSH host 配置（D13） | H19.15 | 已补 2 项（`assistant-sync` `ssh.rs`：归一化 / 留空回退）+ `assistant-config` 的 roundtrip 与「字段缺失取默认」两处断言 | 配置自定义 host → 归一化后命中；留空 → 回退默认两项；旧配置缺字段可正常加载 | `[x]` |
| 密钥不匹配归因（M7/D18） | — | 已补 `pull_reports_key_mismatch_and_keeps_remote_ciphertext` | 用另一套 master key 加密的仓库 → 返回 `VaultKeyMismatch`，且导出**未覆盖**对端文件 | `[x]` |
| 历史版本灾难恢复导出（M7/D19） | — | 无测试 | 改密码前后各一 commit：`commit` 指向改密码前 → 恢复密钥导出成功；指向 HEAD → 报「该版本未设置恢复密钥」 | `[ ]` |
| 损坏文件不覆盖（M7/D18） | — | 已补 `pull_reports_unreadable_entries_with_corrupt_reason` + `sync_now_skips_export_of_unreadable_entries` | 破坏单个 `.enc` → `unreadable_ids` 含该 id、其余条目正常导入、导出跳过该 id | `[x]` |
| 改密码后强制重写（M3/force） | — | 已补 `force_export_rewrites_entries_after_master_password_change` | 改主密码 → 仓库所有 `.enc` 密文变化（`revision` 不变也必须重写） | `[x]` |
| 冲突预览分侧错误（N6/D17） | — | 已补 `conflict_preview_reports_only_the_undecryptable_side` | 仅远端不可解密 → `remoteError` 有值、`localError` 为空 | `[x]` |
| 冲突明文按需显示（N6/D17） | — | `tests/reveal_guard.rs` 3 项覆盖掩码与限流，冲突上下文按需揭示未断言 | 默认返回 `MASKED`；调用 `vault_conflict_reveal` 才返回明文；库锁定时返回 `VaultLocked` | `[~]` |

---

### O2 `[ ]` 手工验证清单（P2 文档未勾项）

- **I7.2 ~ I7.11**：`ssh-add -l` 可见性、锁定后移除、解锁自动添加、zeroize / 不进 JS 堆（代码审查）、三种密钥格式、无效格式错误、`added_fingerprints` 仅移除自己添加的、前端元信息展示、设置项四处同步
- **J8.3 / J8.5 ~ J8.12**：生成→锁定→恢复密钥解锁、仅返回一次、master key 不经 IPC、HMAC 校验、无 `master_key_hash`、pull 闸门、灾难恢复端到端、轮换、审计日志
- **K1.2 ~ K1.4**：`cargo clippy` 无新增 warning、全量 `cargo test`、`tsc --noEmit`
- **K2.4 / K2.6 ~ K2.10**：凭证不入库、SSH 明文生命周期、恢复密钥不外泄
- **K3.1 ~ K3.11 / K3.13 / K3.14**：完整同步流程、双向同步、冲突各选项、智能推送、软删除同步（M2 的确认流程：确认 / 取消两条路径）、往返一致、SSH Agent、恢复密钥、灾难恢复、meta 完整性、前端状态、冲突 diff 组件
- **K4.1 ~ K4.6**：类型对齐、错误映射、安全不变式、P1 兼容、条件编译、设置项四处一致
- **L7.3 ~ L7.8**：远程配置闭环、冲突闭环、中止/放弃、回收站、purge、历史还原跨设备收敛
- **M7 / D18 / D19**：B 改主密码 push → A 用旧密码 pull → 提示「对端使用另一套主密钥」而非「可能被篡改」，且 A 未覆盖对端密文；A 用恢复密钥 + 改密码前的 commit 导出成功；人为破坏单个 `.enc` → 仅该条目报错、其余正常
- **N6 / D17**：冲突列表显示条目名；密码默认掩码、点击才出明文且 30s 自动隐藏；差异行高亮；单侧解密失败时另一侧仍完整展示

> 说明：P2 文档中 **J5.2 / J6.1 ~ J6.4 / J7.1 / H14.1 / H14.3 / L1.x / L2.x / L3.x** 实际已在代码落地（`RecoveryTab.tsx` / `RecoveryKeyForm.tsx` / `DisasterRecovery.tsx` / `vaultRecoverySlice.ts`；`VaultUnlock.tsx` 的灾难恢复入口已不再受 `isInitialized` 限制，**缺陷 #11 已修**），应回填勾选而非重复开发。

> 状态（2026-10-28）：**K3.1 ~ K3.11 / K3.13 / K3.14**（含 M2 确认 / 取消两条路径）、**N6 / D17**、**M7 / D19 的灾难恢复历史版本导出**、**M7 / D18 的 UI 提示（B7）**对应实现已就绪，可开始手工验证。

---

### O3 `[x]` 全量复核命令（已执行，全部通过）

```powershell
# 必须在 src-tauri 目录下执行（加载 .cargo/config.toml 的 OPENSSL_DIR）
cargo clippy --workspace --all-targets
cargo test --workspace
# 前端
npx tsc --noEmit
```

**已执行（2026-10-28）**

| 命令 | 结果 |
|------|------|
| `cargo test -p assistant-sync --features assistant-sync/git-tests` | 40 passed / 0 failed（`git_sync` 9、`sync_engine` 18、`vault_file` 13） |
| `cargo test -p assistant-security --features assistant-security/slow-tests` | 全绿：`recovery_key` 9、`vault_crud` 13、`vault_crypto` 17、`vault_sync_db` 7、`security_db` 10、`ssh_validate` 4、`totp` 14、`password_strength` 12、`hibp` 5、`api_key` 7、`reveal_guard` 3 |
| `cargo check -p assistant-app` | 通过 |
| `cargo fmt -p assistant-sync -p assistant-security -- --check` | 通过 |
| `cargo check --all-targets` | 通过 |
| `npm run lint` | 通过 |
| `npm run typecheck` | 通过 |
| `npm run test` | 10 文件 / 181 用例通过 |
| `cargo clippy --workspace --all-targets` | 通过（exit 0）；仅存量 warning：`assistant-sync` 3 条（`types/mod.rs` `ptr_arg`、`ssh.rs` `needless_splitn` ×2，均为 P2 之前代码）、其余 crate 的 `Default` / `needless_borrow` 等；B5 / B6 / B7 未新增 |
| `cargo test --workspace` | 通过（`--no-fail-fast` 全量执行，0 failed）。期间修正两处**存量过期断言**：`assistant-app` `protocol_contract.rs` 把 `cacheReadTokens` / `cacheWriteTokens` 断言为 `Some(0)`（DTO 实为 `Option<u32>` → `None`）、`assistant-storage` `conversations.rs` 断言标题含「副本」（`4ad84ab` 已将后缀改为 `(copy)`）；均只改测试，未动业务代码 |

---

## 阶段 P — 设计结论与待决项

### P1 `[x]` Git 完整性边界（H5.8）— **已拍板 D11：不做仓库级 HMAC**

Git 哈希链只提供**无密钥完整性**（能检出意外损坏与非自洽篡改），不提供**认证**：有仓库写权限者可重写历史、回滚到旧提交或替换对象并保持链自洽。因此不引入 `entriesDigest` 等仓库级 HMAC（任何仓库内校验都会被一并伪造），`.vault-meta` 的 `integrity` 维持仅覆盖元数据自身。具备仓库写权限者视为超出威胁模型。

### P2 `[x]` `last_good_commit` 回滚检测（H5.9）— **已拍板 D12：不纳入**

若要检出**回滚 / 历史重写**，需在本地 DB 持久化上次成功同步的 commit hash，pull 前校验其为远端 HEAD 的祖先。约束：基准不得存入仓库（否则可被同时篡改）；需处理多设备并发、首次同步无基准、force push 语义。**结论：不纳入 P3，仅保留本节记录。**

### P3 `[x]` SSH ControlMaster 复用验证与白名单（H19.15）— **已拍板 D13：用户可配置**

[ssh.rs](../../src-tauri/crates/assistant-sync/src/git_sync/ssh.rs) 仅在 Unix 启用 `ControlMaster`，且白名单限 `github.com` / `gitlab.com`；Windows 仅注入超时参数、无复用。

**结论（已拍板）**：白名单改为**用户可配置**，与远程地址在同步设置页一并配置：

- 新增 `AppConf.vault_sync_ssh_hosts: Vec<String>`（`#[serde(default)]`），与 D7 的 `vault_sync_last_at` 同批实现（四处同步）；
- 留空时回退默认 `["github.com", "gitlab.com"]`，保持向后兼容；
- `ssh.rs` 的 `should_use_control_master(host)` 改为从配置读取；host 取 remote URL 的 authority 部分；
- 前端 `VaultSyncSettings` 增加「SSH 复用主机」输入（逗号分隔），与 URL 输入同区域；
- 安全性：`ControlPath` 落在 `/tmp`，多用户机器需 `0600` 权限；Windows 仍不复用（仅注入超时参数）。

**实施状态（2026-10-28，`[x]` 已完成）**：`ssh.rs` 新增 `set_ssh_persist_hosts()` / `resolve_persist_hosts()`，`guess_ssh_persist_support()` 改为读配置并在配置变更时清空缓存（`SSH_PERSIST_GUESS_CACHE`）；`setup.rs` 启动时加载；`APP_SIDE_EFFECTS` 注册 `vault_sync_ssh_hosts` 侧效；前端同步设置页新增 host 输入（Windows 隐藏，与后端 `#[cfg]` 语义一致）；补 2 项单测（归一化 / 留空回退）。

### P4 `[x]` `SyncConfig.branch` 字段语义 — **已拍板 D14：删除该字段**

前端 `buildConfig()` 硬编码 `branch: "main"`，后端 `push` / `pull` / `prepare` 实际使用**当前分支**（`git_current_branch`），该字段全程未被消费。

**清理范围**（已确认无持久化，`AppConf` 无对应项，无需迁移）：
- `assistant-sync` 的 `SyncConfig` struct 字段 + `Default` 实现
- `src/vault/core/vault.types.ts` 的 `SyncConfig` interface
- `src/vault/settings/VaultSyncSettings.tsx` 的 `buildConfig()`
- `src/vault/core/vaultStore.test.ts` 的 mock 数据
- 后端构造点：`setup.rs`、`cmd/vault/recovery.rs`、`cmd/vault/sync.rs`

**实施状态（2026-10-28，`[x]` 已完成）**：上述五处全部删除（后端构造点用 `..Default::default()`，无需改动），`cargo check --all-targets` 与 `tsc --noEmit` 均通过。

---

## 附录 A：P2 文档需回填的勾选状态

| P2 项 | 实际状态 | 依据 |
|-------|----------|------|
| H10.2 "不自动完成 merge commit" | 备注失效 | `resolve_conflict` 无剩余冲突时 `commit_all` |
| H14.1 / H14.3 / L1.1 ~ L1.6 | 已完成 | `VaultSyncSettings.tsx` 的 URL 输入常驻、克隆/测试连接/解绑按钮 |
| H15.3（MergeBoth） | 已完成 | 后端已实现为新条目（`Uuid::new_v4()` + `revision = max+1`），见 N1 |
| H19.7 | 已实现、缺测试 | `need_to_push` + `safe_need_to_push` |
| H19.20 / J8.9 / K3.11 | 已完成 | `import_aborts_when_vault_meta_tampered` |
| J3.3 | 已归并 | 校验点在 `import_from_repo` |
| J5.2 / J6.1 ~ J6.4 / J7.1 | 已完成 | `RecoveryTab.tsx` / `RecoveryKeyForm.tsx` / `DisasterRecovery.tsx` / `vaultRecoverySlice.ts` |
| K3.12 / H13.3 / H18.1 ~ H18.2 | 已作废 | 定时轮询与自动同步间隔字段均已移除 |
| K3.5（软删除同步） | 已完成 | M2：`PendingRemovals` 待确认 + `vault_sync_confirm_removals` 批量软删除（D1 / D15 / D16） |
| H19.12（冲突解决落库） | 已完成 | M1：`force_import_entry_tx` + 收尾自动 `finalize`（D4 / D9） |
| H14.2 / H16.1（上次同步时间） | 已完成 | B5 / N2（D7）：`vault_sync_last_time` + `AppConf.vault_sync_last_at` |
| H19.15（SSH 复用白名单） | 已完成 | B5 / P3（D13）：`AppConf.vault_sync_ssh_hosts` 可配置 |
| J3.4（恢复密钥 meta 完整性告警） | 已完成 | B6 / N3（D8）：`VAULT_META_INTEGRITY_WARNING` 事件 + 前端 `notifyWarning` |
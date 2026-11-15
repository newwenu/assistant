# I50 — 运行时审计规格

| 项 | 值 |
| --- | --- |
| 状态 | 现行规格（约束 vault 审计与工具权限审计两条链路） |
| 日期 | 2026-11-14 |
| 依据 | 后端现状盘点（缺口 G1–G10 / 观察项 O1–O7）见 `audit-coverage-audit-1.md`，`G11`–`G14` 为本规格编写时新增的发现；前端现状盘点（缺口 F1–F8 / 观察项 OF1–OF5 / 有意例外 EF1–EF3）见 `audit-coverage-audit-2.md`。本规格只写「应当如何」，不复述盘点内容 |
| 替代 | I9-vault-audit-spec（已归档）§1–§6 |
| 落地对象 | 后端：`src-tauri/crates/assistant-vault/src/audit/`、`src-tauri/crates/assistant-vault/src/security_db/`、`src-tauri/crates/assistant-tools/src/gateway.rs`、`src-tauri/crates/assistant-tools/src/pipeline.rs`、`src-tauri/src/adapters/permission_store.rs`、`src-tauri/src/cmd/vault/`、`src-tauri/src/cmd/permission.rs`；前端：`src/vault/settings/`、`src/vault/core/`、`src/services/permissionService.ts`、`src/i18n/locales/` |
| 关联 | I38g-agent-approval-advisor-spec.md（审批卡片与 `explain`）、I27-tool-data-store-spec.md（工具落盘，与审计无关） |

本规格中的「必须 / 禁止」为可验收约束；「应当」为默认做法，偏离需在代码注释里写明理由。

---

## 1 范围与判定口径

### 1.1 两条链路

| 维度 | vault 审计 | 工具权限审计 |
| --- | --- | --- |
| 承载数据 | 保险库内的 `security_audit_log`（加密库） + 库外 JSONL 明文兜底 | 主库 `permission_audit` 表（明文） |
| 生命周期 | 随 `security.db`；库销毁即消失 | 随主库；不随会话删除，按 `audit_retention_days` 裁剪（§5） |
| 事件词表 | `AuditEvent` 枚举（31 个变体，见 §2.3） | `AuditOutcome` 枚举（现状 8 个、本规格扩展为 11 个，见 §3.1） |
| 写入时机 | 业务动作成功后 / 拒绝时 | 网关做出 `GatewayDecision` 时；管理动作（§3.6）在授权变更事务内 |
| 失败语义 | 不阻断业务，仅 `warn` | 不阻断业务，仅 `warn` |
| 可查询性 | 后端 `query_log` 有接口但未参数化（§2.6），前端无消费方（F1） | 无查询接口（F2） |

两条链路的存储、事件词表、降级策略、查询面**互不通用**：不得把工具权限事件写进 `security_audit_log`，也不得把 vault 事件写进 `permission_audit`。

### 1.2 三要素判定

一个安全动作被视为「已审计覆盖」，必须同时满足：

1. **稳定事件标识**：`AuditEvent`（vault）或 `AuditOutcome`（工具）中存在对应变体，字符串名固定；
2. **生产调用点**：真实业务路径上存在对该变体的写入调用（不是仅在测试里出现）；
3. **可检索落点**：写入后能从落点按稳定字段检索到（vault：`event_type` / `timestamp` / `detail` 中的条目 ID；工具：`tool` / `action` / `resource` / `outcome`）。

只满足其中 1–2 项即视为缺口，不是「已覆盖」。

### 1.3 非目标

- 不做跨链路的统一审计视图；
- 不做防篡改（哈希链、签名）——`security_audit_log` 依赖库本身的加密与完整性，JSONL 依赖文件系统权限；
- 不把审计当作事实源：审计用于观测与追责，业务状态以业务表为准。

---

## 2 vault 审计

### 2.1 存储

**加密库表**（`security_db/schema.rs`，`SCHEMA_VERSION = 2`）：

```sql
CREATE TABLE security_audit_log (
    id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    detail TEXT,
    timestamp INTEGER NOT NULL
);
CREATE INDEX idx_audit_log_timestamp ON security_audit_log(timestamp);
CREATE INDEX idx_audit_log_event_type ON security_audit_log(event_type);
```

- `timestamp` 为 Unix 毫秒；
- `detail` 允许 `NULL`（表示「无附加信息」，不是「缺失」）；
- `id` 只作主键占位，业务语义由 `(event_type, timestamp, detail)` 承载。双写行的 `id` 与 JSONL 侧无对应关系。

**JSONL 兜底**（`audit/jsonl.rs`）：

- 目录：`<data>/security/audit/`，与 `security.db` 同级；
- 文件名：`audit-YYYYMMDD.jsonl`（本地日期），当日文件超过 8 MiB 顺延为 `audit-YYYYMMDD-{n}.jsonl`；
- 单行结构：`{"at": <毫秒>, "event": "<snake_case 事件名>", "detail": <string|null>}`；
- 权限：Unix 下文件创建后置 0600（`restrict_permissions`），Windows 为空实现；
- 写入器只持目录路径，不持密钥、不持连接——因此锁库、无 `security.db`、恢复通道都能写。

### 2.2 写入原语

| 原语 | 位置 | 语义 |
| --- | --- | --- |
| `log(conn, event, detail)` | `audit/mod.rs` | `log_dual(Some(conn), None, ..)` 的别名，**仅 DB** |
| `log_dual(conn, writer, event, detail)` | `audit/jsonl.rs` | DB 与 JSONL 同时尝试；**任一路成功即 `Ok`**，失败侧仅 `warn` |
| `audit_dual(conn, writer, ..)` / `audit_log(conn, ..)` | `security_db/mod.rs` | `SecurityDb` 上的状态方法：`audit_dual` 取当前 conn + writer，`audit_log` 只写 DB |
| `AuditJsonlWriter::append_at(event, detail, at)` | `audit/jsonl.rs` | 指定时间戳追加；双写时必须与 DB 行共用同一 `at` |

**通道选择规则（必须遵守）**：

- **双写（B）**：解锁 / 锁库 / 主密码变更 / 恢复密钥导出 / DEK 轮换 / 重建 / 回滚拒绝 / 生物识别——即「锁库后仍需可追溯」的会话级事件；
- **仅 DB（D）**：解锁态事务内的条目级明细（reveal、create/update/delete/restore/purge、copy、HIBP、SSH 挂载）；
- **仅 JSONL（J）**：写入时拿不到已解锁 `conn` 的事件（失败类、未解锁路径）。**J 通道事件必须同时进补录白名单**（§2.5）。

双写时间戳共用是硬约束：`log_dual` 内部只取一次 `now_millis`，DB 行与 JSONL 行必须同值，否则 §2.5 的去重会失效。

### 2.3 事件词表

字符串名为 `AuditEvent::Display` 输出（snake_case）。通道：D = 仅 DB，B = 双写，J = 仅 JSONL。

| 事件 | 通道 | 生产落点 | detail 文法 |
| --- | --- | --- | --- |
| `vault_unlock` | B | `security_db::unlock_session` | `NULL` |
| `vault_unlock_failed` | J | 待补（G1，`cmd/vault/lifecycle.rs` 3 处 `InvalidPassword` 分支，均为 `wrong_password`） | 封闭标签，现状只用到 `wrong_password`；`recovery_wrong_key` 为恢复通道保留值，仅在该通道真正出现口令校验失败时启用（生物识别失败已有独立事件 `biometric_failed`，不复用本标签） |
| `vault_lock` | B | `security_db::lock_session` | `LockReason::as_str()` |
| `vault_reveal` | D | `cmd/vault/reveal.rs`（`{entryId}`）、`cmd/vault/sync.rs`（冲突揭示） | 条目文法（§2.4）；冲突揭示路径**目标**为 `conflict:{entryId}:{side}:{field}:{digest}`（现状 `conflict:{path}:{side}:{field}`，O5） |
| `vault_reveal_rate_limited` | D | `cmd/vault/reveal.rs`（`{entryId}`）、`cmd/vault/sync.rs`（冲突揭示） | 条目文法；与 `vault_reveal` **同一事件两条路径的 detail 语义必须同形**——冲突揭示路径现状写路径原文（O5），目标同 `conflict:*` 5 段 |
| `vault_entry_created` | D | `security_db/entries/write.rs`、`entries/import.rs` | 条目文法 |
| `vault_entry_updated` | D | `entries/write.rs`、`entries/import.rs` | 条目文法（同步覆盖路径待补，G3） |
| `vault_entry_deleted` | D | `entries/write.rs`（单条 / 批量） | 条目文法；批量路径**必须**为 `{entryId}@{source}`（现状写调用方标签 `remote`，丢 entryId，R15） |
| `vault_entry_restored` | D | `entries/deleted.rs` | `{entryId}` 或 `{entryId}@{source}` |
| `vault_entry_purged` | D | `entries/deleted.rs` | `{count}` |
| `vault_copy_password` | D | `cmd/vault/reveal.rs` | 条目文法 |
| `vault_copy_field` | D | `cmd/vault/reveal.rs`、`cmd/vault/audit.rs` | `{entryId}` 或 `totp:{entryId}`；**语义**为「字段明文被读出」，含 `vault_totp_generate`（生成亦属读出，R16），前端文案不得一律写「复制」 |
| `hibp_audit` | D | `cmd/vault/audit.rs` | `NULL` |
| `hibp_leak_found` | D | `cmd/vault/audit.rs` | `{entryId}:{count}` |
| `master_password_change` | B | `security_db/rekey.rs` | `NULL` |
| `recovery_key_exported` | B | `assistant-vault/src/lifecycle.rs::export_recovery_key` | `{dekVersion}` |
| `recovery_export_used` | J | 恢复通道 | `{count}` |
| `dek_rotated` | B | `assistant-vault/src/lifecycle.rs::rotate_dek` | `{old}:{new}:{count}`（现状为 `{old}->{new} entries {n}` 自由文本，R14） |
| `local_db_repaired` | — | 无调用点（G2：删除该变体） | — |
| `local_db_rebuilt` | B | `security_db/rekey.rs` | `{count}` |
| `anchor_rollback_rejected` | B | `security_db/mod.rs` | `meta_or_dek_version` |
| `biometric_enrolled` | B | `security_db/biometric_ops.rs` | `NULL` |
| `biometric_unlocked` | B | 同上 | `NULL` |
| `biometric_failed` | J | 同上（`conn` 恒为 `None`） | 封闭标签（平台错误码映射后的固定串） |
| `biometric_revoked` | B | 同上 | `NULL` |
| `ssh_key_loaded` | D | `cmd/vault/ssh.rs` | `{entryId}`（SSH 密钥条目 id） |
| `ssh_key_removed` | **D**（单条现状）/ **J**（锁库批量**目标**） | `cmd/vault/ssh.rs`（单条，`audit::log` 仅 DB）、`cmd/vault/mod.rs::ssh_remove_all_on_lock`（锁库批量**现状零写入**，属 G5；该点在 `lock_session` 之后、DB 已关闭，补录只能走 JSONL） | `{fingerprint}`（现状表曾标 B，与实际不符，R17） |
| `vault_repository_reset` | J | `assistant-vault/src/lifecycle.rs::reset_repository` | `NULL` |
| `vault_unsupported_version` | J | `cmd/vault/lifecycle.rs` | `unsupported_version` / `version_too_new` |
| `vault_entries_exported` | D | `cmd/vault/entries.rs` | `{format}:{count}:{pathDigest}`，失败追加 `:failed` |
| `vault_entries_imported` | D | `cmd/vault/entries.rs` | `{format}:{count}:{pathDigest}` |

新增 vault 事件的准入条件：① 有真实生产调用点；② 通道归属明确；③ detail 文法落在 §2.4 的封闭形态内；④ 若为 J 通道，同一次提交必须加入 `BACKFILL_EVENTS`。

**`vault_lock` 的归因（F5）**：`LockReason` 有 6 个变体（`user` / `auto_lock` / `screen_lock` / `app_exit` / `reset` / `panic`），现状 `auto_lock` 生产零调用——前端自动锁定定时器最终调用同一个 `vault_lock` 命令，而该命令硬编码 `LockReason::User`，detail 恒为 `user`。规格要求：

- `vault_lock` 必须接受可选 `reason`（默认 `user`），前端自动锁定传 `autoLock`，后端映射 `LockReason::AutoLock`；
- `reason` 为封闭枚举 DTO（camelCase wire 名，需 `npm run contract:gen`），detail 仍用 `LockReason::as_str()` 的 snake 串；
- 禁止在前端本地记录或推断锁定原因——归因只由后端落 detail。

### 2.4 detail 文法

**禁止**把自由文本、用户输入、完整路径、密码/TOTP/密钥材料写进 `detail`。

条目级事件的 `detail` 必须属于以下五种形态之一：

| 形态 | 说明 | 检索方式 |
| --- | --- | --- |
| `{entryId}` | 单条动作 | `detail = ?1` |
| `{entryId}@{suffix}` | 带来源后缀（如恢复来源、同步远端删除） | `detail LIKE ?1 || '@%'` |
| `{tag}:{entryId}` | 带封闭标签前缀（`totp`） | `detail LIKE '%:' || ?1` |
| `{entryId}:{count}` | 条目级 + 计数（仅 `hibp_leak_found`） | `detail LIKE ?1 || ':%'` |
| `conflict:{entryId}:{side}:{field}:{digest}` | 冲突字段揭示（固定 5 段，见下） | `detail LIKE 'conflict:' || ?1 || ':%'` |

因此**条目 ID 必须落在 detail 的可定位固定段**——首段、末段，或 `conflict:` 前缀后的第 2 段；这是可检索性的实现前提（`LIKE` 前缀/后缀匹配只对这几种位置有效）。

`unknown` 是上述形态中 ID 段的封闭哨兵值（来源见本节「入参校验前置」）：它**满足形态**（段数不变、仍可按位置检索），但不对应任何真实条目，因此不会与真实 `entryId` 的查询结果混淆；前端按未知值渲染（§6）。

**`:` 与 `@` 分隔的消歧规则（R13）**：`{tag}:{entryId}` 与 `{entryId}:{count}` 分隔符相同、位置语义相反，且与三元组、`conflict` 形态共用同一分隔符，因此解析必须**按段数 + 首段封闭标签有序匹配**，不得只按「第二段是否数字」单条判断：

1. 首段 = `conflict` 且恰为 5 段 → `conflict:{entryId}:{side}:{field}:{digest}`；首段 = `conflict` 但段数 ≠ 5 → 原样兜底（不得退化为第 4 条，避免把畸形冲突行渲染成 `{entryId}:{count}`）；
2. 首段 ∈ 导出格式封闭集合（`json` / `csv`）→ 4 段 `{format}:{count}:{digest}:failed`（失败）或 3 段 `{format}:{count}:{digest}`（成功）；首段命中格式但段数 ∉ {3, 4} → 原样兜底；
3. 首段 = `totp` 且 2 段 → `{tag}:{entryId}`；
4. 串中含 `@` 且首段不在上述任一封闭标签内 → `{entryId}@{suffix}`，**以首个 `@` 切分**，entryId 取首段、其后整段为 suffix（不得要求「恰好 2 段」）；
5. 2 段且首段不在上述任一封闭标签内、第二段为纯数字 → `{entryId}:{count}`（`hibp_leak_found`）；
6. 其余 → 原样兜底，不猜测。

**第 4 条必须按首个 `@` 切分的原因**：`{entryId}` 是 UUID（只含十六进制与 `-`），永不含 `@`；但 suffix 在历史恢复路径上是 git 引用，而 `HEAD@{1}` 这类 reflog 语法本身含 `@`，`git show` 能成功、审计照常写入，因此「`@` 段数」不可作为判定依据。

**该规则成立的前提（必须写进测试）**：条目 ID 由 `uuid::Uuid::new_v4()` 生成（`security_db/entries/write.rs`），**永不为纯数字**，因此「第二段纯数字」不会与 `{tag}:{entryId}` 冲突。若将来改用数字 ID，本规则必须同步改为按首段封闭标签判断。

**首段封闭标签集合**固定为：`totp`（`{tag}:{entryId}`）、`conflict`（冲突揭示前缀）、`json` / `csv`（导出格式）；三个子集互斥，新增任一成员必须同时更新 §2.4、后端唯一解析入口 `audit::parse_detail`（`detail_entry_id` 亦经它取 ID）、前端解析器与文法测试。首段不在该集合内的 2 段串才走第 5 条 `{entryId}:{count}`。

**入参校验前置（R13 的前置条件）**：`detail` 中的 ID 类字段必须在写入前校验形态——条目 ID 必须是 UUID 形态（或来自已验证的库内行），冲突路径必须能被 `parse_entry_id` 解析，`side` / `field` 必须在封闭集合内；任一不满足则写 `unknown`，不得写原串。理由：`cmd/vault/reveal.rs::vault_reveal_password` 与 `cmd/vault/sync.rs::vault_conflict_reveal` 的**限流分支都在任何库内校验之前**就写审计，`id` / `path` / `side` / `field` 全是未校验的前端入参；不校验就会让任意文本（含 `:`、空格、换行）进入 `detail`，既违反「禁自由文本」，也直接破坏上面 `:` / `@` 消歧规则所依赖的段数前提。

`conflict` 揭示事件的形态固定为：

```
conflict:{entryId}:{side}:{field}:{pathDigest}
```

- `side` ∈ `local` / `remote`；
- `field` ∈ 封闭集合（如 `password` / `notes` / `totp` / `customFields`）；
- `pathDigest` 为路径的 SHA-256 摘要（`audit::path_digest`），用于定位但不泄露路径原文；
- `{entryId}` / `side` / `field` 三者任一无法解析或不在封闭集合内时，一律写封闭哨兵值 `unknown`，**禁止**退化为原串（理由见上条「入参校验前置」）。

现状为 `conflict:{path}:{side}:{field}`（`cmd/vault/sync.rs::vault_conflict_reveal`），**三处不合规**：段数 4 而非 5、首段后是路径原文、路径串未校验即入库（见上条「入参校验前置」）。必须改为上式，路径一律先转摘要。

**实现位置约束**：`path` 形如 `entries/{uuid}.enc`，entryId 只能由 `sync_engine::preview::parse_entry_id` 得到，而它是 `pub(super)`（仅 `sync_engine` 内可见）。因此 detail 的拼装必须在 `assistant-vault` 内完成（新增公开辅助，如 `sync_engine` 暴露 `entry_id_from_path(path) -> Option<String>`，或直接在 `sync_engine::reveal_conflict_field` 里返回 detail 所需的 `(entryId, digest)`），**禁止**在 `cmd/vault/sync.rs` 里复制 `strip_prefix("entries/") + strip_suffix(".enc")` 的解析逻辑——否则同一语义出现第二个实现入口，违反 §2.7.2。

批量条目动作（同步远端删除）**必须**为 `{entryId}@{source}`，`source` 为封闭标签（现状仅 `remote`）；现状只写 `remote` 丢掉了 entryId（R15），违反本节「条目 ID 必须落在可定位固定段」。

历史恢复的 `{suffix}` 例外：`vault_entry_restored` 的 `detail` 为 `{entryId}@{commit}`（`security_db/entries/deleted.rs::restore_from_history_tx`，`commit` 来自 `cmd/vault/sync.rs::vault_history_restore` 的前端入参）。它不是封闭标签，但**由 `git show` 成功隐式校验**——`git_show_file` 拼成 `{commit}:{path}`，commit 含 `:` 或空格时解析必然失败、走不到审计写入。因此本节要求：`{suffix}` 要么是封闭标签（删除的 `source`），要么是「已被产生它的解析动作证明合法」的值（恢复的 commit）；两者都**不得**是未经任何校验就原样透传的前端字符串。

**该「隐式校验」不构成安全保证**：它只解释「为什么恢复路径的 `detail` 现状是收敛的」，不能推广为「前端入参可直通 git」。事实上 `sync_engine/history.rs::read_historical_entry` 把 `commit` 与 `entry_id` 直接拼成 `git show {commit}:entries/{entry_id}.enc`，`commit` 首字符为 `-` 时会被 git 当作选项（如 `--output=<file>`），`entry_id` 也未校验 UUID 形态即进入路径。这属于恢复路径的入参白名单问题（拒绝前导 `-`、限制 git 引用字符集、`entry_id` 校验 UUID），**不在本规格范围内**，需单独立项；本规格只要求审计侧在拿不到合法值时写 `unknown`。

非条目事件的 `detail` 必须满足其一：

- `NULL`；
- 单个整数计数；
- `{format}:{count}:{digest}` 三元组（导出导入）；
- `{old}:{new}:{count}` 三元组（`dek_rotated`，R14）；
- 封闭标签（`LockReason`、`meta_or_dek_version`、`wrong_password` 等）。

**禁止**任何含空格、`->`、自然语言片段的 `detail`：这类串无法被前端文法解析，也会让「原样展示」退化为自由文本泄露口。

### 2.5 JSONL 补录

触发点：`unlock_session` 成功写 `vault_unlock` 之后立即调用 `try_backfill_jsonl_audit`。

规则：

1. **游标**：`vault_meta.audit_jsonl_synced_at`（毫秒；代码常量为 `JSONL_SYNC_KEY`），`read_since` 现状为严格大于该值。补录成功后**必须**把游标推进到「本次已处理记录的最大 `at`」。
   现状写的是 `now`（G11），时钟回拨会漏补——必须修正。
   **游标精度（R2）**：毫秒时间戳不足以唯一标识进度。改成「max(at)」后，若严格用 `>` 过滤，与游标同毫秒的其余记录在下一次补录中被排除，**永久漏补**。因此：
   - 短期修法：`read_since` 改为 `>=`，由去重键（第 3 条）挡住重复；代价是同毫秒记录每次解锁都会重读一遍；
   - 正解：以第 3 条引入的 `seq` / `(文件名, 行号)` 作为游标，时间戳只用于展示排序。
   两种修法都必须与第 3 条同批落地，单独改游标会引入新漏补。
   **游标落在未来必须自愈（G11）**：`stored > now` 属时钟回拨或元数据损坏，本次按 `since = 0` 全量扫描一次（去重键挡重复），补录结束后游标被改写成实际 `max(at)`，下一轮自动回到增量路径。不得直接信任未来游标——否则「回拨期间写入」的记录因早于游标而永久漏补。
   无记录时不推进游标（保持原值），不得写成 `now`。
2. **白名单**：`BACKFILL_EVENTS` 必须**覆盖**「所有可能只写 JSONL 的事件全集」（即上表 J 通道事件集合）。当前 J 集合为
   `vault_unlock_failed`、`recovery_export_used`、`vault_repository_reset`、`vault_unsupported_version`、`biometric_failed`、`ssh_key_removed`（锁库批量移除路径，**G5 补齐后才成为 J**）（现状白名单只含 `recovery_export_used` / `dek_rotated` / `local_db_rebuilt` / `vault_repository_reset`，缺 4 项，G4）。
   双写事件也可以留在白名单里（去重会挡掉，无副作用），但白名单不得**缺少**任何 J 事件——这是唯一自洽的判定方向。
3. **去重键必须能区分同一毫秒的多条记录（R3）**：现状 `INSERT ... SELECT ... WHERE NOT EXISTS (event_type = ? AND timestamp = ?)` 以 `(event_type, timestamp)` 去重、`id` 取 `jsonl-{at}-{event}` 作 PK，同一毫秒产生多条同事件时会被**必然合并**——白名单扩容后（G4）`ssh_key_removed` 锁库批量移除 N 个密钥（`cmd/vault/mod.rs::ssh_remove_all_on_lock` 为循环）与 `biometric_failed` 连续重试都命中该路径，属确定丢行而非概率事件。
   允许的实现路径，二者择一，**禁止**继续用 `(event_type, timestamp)`：
   - (a) JSONL 行加单调 `seq`（写入侧生成，同 `at` 递增），去重键 `(event_type, timestamp, seq)`；
   - (b) 以记录的物理位置 `(文件名, 行号)` 作身份，去重键 `(event_type, source)`。

   选 (b) 时须注意：一旦将来引入文件重写/压缩，行号会漂移，必须在同一变更里补「重写后去重键失效」的处理或明确禁止重写。
4. **一致性断言（必须加测试）**：维护一份「J 通道事件清单」，断言其每个元素都在 `BACKFILL_EVENTS` 中。新增 J 事件而未改白名单必须让测试失败。
5. **单行解析失败**：`warn` 后跳过，不整体失败（现状已满足）。
6. **单文件不可读必须降级为跳过（R4）**：`read_since` 现状用 `File::open(&path)?`（`audit/jsonl.rs`），一个文件权限/占用异常就让整次补录返回 `Err`，已读入内存的记录全部丢弃、游标不推进；该文件若永久不可读，补录**永久失效**且每次解锁只留一条 `warn`（静默失效）。必须改为逐文件 `warn + continue`。
7. **补录必须包事务、并避免全量重扫（R6）**：`since` 缺失或损坏时现状 `unwrap_or(0)` 即全量扫描；`rebuild_local_db` 重建库只写 `vault_id`、不写游标（`security_db/rekey.rs`），因此**重建后首次解锁必然全量补录**。要求：
   - 补录循环包一层事务，禁止逐条隐式提交（N 条 = N 次 fsync，解锁耗时随日志线性增长）；
   - `rebuild_local_db` 完成后把游标写成重建时刻；
   - `read_since` 按文件名日期裁剪候选文件（文件名用本地日期、记录用 UTC 毫秒，**必须留 1 天余量**），禁止无条件遍历全部日文件。
8. **补录行的可信度与原生行不同（R1）**：JSONL 是库外明文，无 HMAC、无链式哈希、不绑定 `vault_id`，任何进程都能改写或删行；补录把其原样插入加密库，等于让可篡改数据获得与加密库同等的可信地位（删掉自己的 `vault_unlock_failed`、或伪造 `recovery_export_used` 都会被"洗白"入库）。
   要求：补录行的 `id` 前缀 `jsonl-` 必须保留，展示层必须可辨识来源（§6）；**不得**把补录行表述为不可否认证据，也不得在面板中与原生行做同质化措辞。
9. **已知取舍：库回滚与跨库污染（R11）**
   - 恢复旧备份后库时间线回退、JSONL 仍前进，补录会把「恢复点之后」的事件注入当前库，审计时间线与数据状态不一致；这是可接受的取舍，但恢复流程应在恢复完成后把游标对齐到备份点，否则补录语义会长期错位。
   - JSONL 目录位于 `<security>/audit/`、与仓库目录分层，记录中不含 `vault_id`；手工复制目录会把旧库事件灌进新库。reset 流程会删除 audit 目录故安全，rebuild 不删（同库，合理）。该风险属已知取舍，若要加固需在 JSONL 行增加 `vault_id` 并跳过不匹配行。
10. **自回响必须显式排除（R12）**：只有「**既双写、又在 `BACKFILL_EVENTS` 白名单内**」的事件才存在自回响——当前正是 `dek_rotated`（`lifecycle.rs::rotate_dek`）与 `local_db_rebuilt`（`security_db/rekey.rs::rebuild_local_db`）：两者都用 `log_dual` 在**同一个 `at`** 写 DB 与 JSONL，因此 JSONL 那行必然被本批读到。
   `vault_unlock` / `vault_lock` 虽同样双写，但**不在白名单**，被过滤挡住，不构成自回响（不得把它们写成本条的触发事件）。
   现状挡重复靠两重机制：`(event_type, timestamp)` 的 `NOT EXISTS`，以及 PK `jsonl-{at}-{event}` 上的 `INSERT OR IGNORE`。第 3 条一旦为区分同毫秒多行而把 `seq` 并入 id，PK 这一重即失效；若同时改去重键，原生行没有新键字段 → **每次解锁都会给 `dek_rotated` / `local_db_rebuilt` 多插一条 `jsonl-` 假补录行**（改用 `>=` 游标后会反复读到这些行，故是每次解锁都多一条，而非只多一条）。
   要求：补录循环先排除「原生行已存在」的记录——**按 `(event_type, timestamp)` 单独查一次原生行**即可（该键与原生行天然一致，与第 3 条的新去重键互不干扰）；或让去重条件同时对照原生行键。两种做法都要写进注释说明取舍。
   判定标准：DEK 轮换后 `dek_rotated` 在 `security_audit_log` 中**恰好 1 行**；重建库后 `local_db_rebuilt` **恰好 1 行**；连续解锁 3 次后 `vault_unlock` **恰好 3 行**。
   该条是第 3 条的前置条件——**先落地本条，再改去重键**。

### 2.6 查询面

- 后端：`query_log(conn, limit)` 固定 `ORDER BY timestamp DESC LIMIT ?1`，无 `since`、无 `event_type` 过滤。
- 前端：`src/vault/core/slices/vaultAuditSlice.ts` 的 `queryAuditLog(limit?)` 无任何组件消费。
- 规格要求：
  - 查询接口必须支持 `since`（毫秒）与 `eventTypes`（集合）两个可选参数，`limit` 必须带上限（默认 100、最大 500）；
  - **排序必须补稳定次级键（R20）**：`ORDER BY timestamp DESC, id DESC`；同毫秒多条时若只按时间排序，面板翻页会出现重复 / 漏项；
  - 返回项必须包含 `eventType`、`detail`、`timestamp`，条目级事件额外返回解析出的 `entryId`；
  - **前端必须提供面板（F1）**：vault 设置页新增独立 tab 消费 `queryAuditLog`，支持按 `eventType` 与时间范围筛选（对应后端两个参数），默认只拉最近 100 条；
  - **锁定态禁用（OF3）**：`vault_query_audit_log` 经 `with_unlocked` 包装，锁定态返回 `VaultError::Locked`（前端错误码 `locked`）；面板在锁定态必须禁用并提示解锁，不得把该错误当异常弹出；
  - **禁止本地缓存（EF1）**：前端不得在 store 里保存审计副本、做增量合并或乐观插入；审计事实源始终在后端，前端只做「读 + 展示 + 映射」；
  - **禁止「加载全部」（OF5）**：`security_audit_log` 无 TTL，前端不得提供无上限的加载入口；
  - **补录行必须可辨识（R1）**：补录行 `timestamp` 是历史时间、可能落在当前查询窗口之外，且来源为库外明文（§2.5 第 8 条）；`AuditLogEntry.id` 已返回，前端按 `jsonl-` 前缀区分「原生行 / 补录行」并在展示上标注来源；禁止按时间做增量追加（增量追加会让补录行永远进不了窗口，与 EF1「禁止本地缓存」同源）；
  - 未接入 UI 前不得对外宣称「用户可查看审计日志」。

### 2.7 风险—收益与实现整齐性

本节回答两件事：补录 / 审计的每一处加固**值不值得做**，以及 vault 审计实现内部**是否自洽**。编号 `R*` 与 §2.5、§2.4 的条目一一对应，用于 §7 差异表与任务清单索引。

#### 2.7.1 风险—收益

| 编号 | 风险 | 触发概率 | 影响 | 修复成本 | 收益 | 结论 |
| --- | --- | --- | --- | --- | --- | --- |
| R1 | 补录行源自库外可篡改明文 | 高（任何本地进程可写） | 审计可信度被稀释，伪造行被「洗白」 | 低（只加来源标注与措辞约束） | 明确可信边界 | 必做（约束层） |
| R2 | 毫秒游标同刻漏补 | 中（同毫秒多事件） | 永久漏事件 | 中（`>=` 或换序号游标） | 消除漏补 | 必做，与 R3 同批 |
| R3 | 去重键合并同刻事件 | 高（白名单扩容后**必然**） | 确定丢行 | 中（改键 + 兼容历史行） | 不丢行 | 必做 |
| R4 | 单文件不可读导致整批失败 | 低 | 补录永久静默失效 | 极低（`warn + continue`） | 单点故障不再放大 | 必做 |
| R6 | 无事务 + 重建后全量重扫 | 高（每次 `rebuild` 后首次解锁） | 解锁耗时线性增长、写放大 | 低（包事务 + 写游标 + 按日裁剪） | 耗时从线性变有界 | 必做 |
| R11 | 恢复备份后时间线错位 | 低 | 审计与数据状态不一致 | 中（恢复流程对齐游标） | 语义自洽 | 取舍：可只做文档声明 |
| R12 | 自回响：DEK 轮换 / 重建库时同 `at` 双写的行被当补录回插 | **改 R3 后必现**（当前 `dek_rotated`、`local_db_rebuilt` 在白名单内且双写） | 每次解锁多一条假「补录」行 | 低（补录前按 `(event_type, timestamp)` 排除原生行） | 去重语义正确 | 必做（R3 的前置条件） |
| R13 | `:` / `@` 文法多义（`{tag}:{entryId}` / `{entryId}:{count}` / `{format}:{count}:{digest}` / `conflict:*` / `{entryId}@{suffix}` 共用分隔符） | 中（`hibp_leak_found` 与 `totp` 并存；未校验的 `path` 可含 `:`；git reflog 引用可含 `@`） | 前端解析错位、展示错字段 | 低（一条有序消歧规则 + 路径摘要化） | 可解析 | 必做 |
| R14 | `dek_rotated` 用自由文本 | 100%（每次 DEK 轮换） | 前端只能原样展示，且违反「禁自由文本」 | 极低（改三元组） | 文法自洽 | 必做 |
| R15 | 批量删除丢 `entryId` | 100%（每次远端同步删除） | 无法按条目追溯 | 极低（改 `{entryId}@{source}`） | 可追溯 | 必做 |
| R16 | `vault_copy_field` 语义宽化（含生成） | 100% | 前端文案误导（写「复制」） | 极低（文档 + 文案约束） | 语义准确 | 必做（约束层） |
| R17 | `ssh_key_removed` 通道与实现不符 | — | 规格不可信 | 极低（改表） | 规格自洽 | 必做 |
| R18 | 审计写入返回 `Result` 但生产调用点一律 `let _ =`（工具侧例外：`cmd/conversation.rs` 内联 INSERT 用 `?` 传播） | 中（已出现 1 处实例） | 审计失败可阻断业务 | 中（vault 原语返回 `()`；工具侧内联 INSERT 改 `warn`） | 类型上不可能阻断 | 建议（与 R19 同批） |
| R19 | 失败级别 `error!` / `warn` 不统一 | 中 | 告警噪音、与「审计是观测」定位冲突 | 极低 | 两条链路一致 | 建议 |
| R20 | 查询排序无稳定次级键 | 中（同毫秒多条） | 面板顺序抖动、翻页重复/漏项 | 极低（补次级排序键） | 稳定展示 | 必做（与查询参数化同批） |
| R21 | 时间源入口分散 | 低 | 未来改精度时易漏改 | 极低（统一入口） | 单一时间源 | 建议 |
| R22 | 限流分支在任何校验之前就把前端入参写进 `detail`（`reveal.rs` 两处 + `sync.rs` 一处） | 100%（每次限流触发） | 任意文本（含 `:`、`@`、空格、换行）进入审计，破坏 `:` / `@` 段数前提与「禁自由文本」 | 极低（写入前校验形态，非法写 `unknown`） | detail 对任意入参保持封闭文法 | 必做（R13 的前置条件） |

**编号说明**：`R5`、`R7`–`R10` 为 §2.5 起草时的预留位，现状中已满足（如坏行解析失败只 `warn` 跳过）或已并入相邻条目，不单列；引用时以本表为准。

**收益上限的诚实说明**：上述加固全部落地后，vault 审计能达到的是「**可追溯、可解释、失败不放大**」，**不是**「不可篡改、不可否认」——后者需要签名链与外部锚点，属本规格非目标（§1.3）。因此 R1 的结论只能是「标注来源」，不能是「验证来源」。

#### 2.7.2 实现整齐性

判定「整齐」的标准：**同一语义只允许一个实现入口、一个失败级别、一个文法形态**。逐项核对结果：

| 维度 | 现状 | 要求 |
| --- | --- | --- |
| 写入入口 | `audit::log`（D）、`log_dual`（B/J 底层）、`SecurityDb::audit_dual`（B）、`SecurityDb::audit_jsonl`（J）四层并存 | 保留四层，但**通道归属必须由 §2.3 表决定**，调用点不得自行选择（R17 即此违反） |
| 失败级别 | `write_db_at` 用 `error!`；JSONL 侧 `warn`；`permission_audit` 侧 `warn` | 统一 `warn`（R19），并保持 §4「不阻断业务」 |
| 返回类型（vault 侧） | 四个原语全部返回 `Result`，生产调用点一律 `let _ =` 丢弃 | 返回 `()`，从类型上消除「阻断业务」的可能（R18）；若保留 `Result`，则必须在 §4 明文禁止 `?` 传播 |
| 写入入口（工具侧） | `PermissionAuditStore::log_decision`（网关决策行唯一入口，错误被 `audit()` 吞成 `warn`）与 `cmd/conversation.rs` 的内联 INSERT（管理行，错误用 `?` 传播、回滚工作区切换）并存 | 两类行**语义不同，各自只允许一个入口**：网关决策行 → `log_decision`；管理行 → 与业务**同事务**的内联写入（§3.6 已明确禁止复用 `log_decision`，它另锁 `DbState`、另取连接，无法共用事务）。两个入口的失败级别统一为 `warn`；现状管理行用 `?` 传播，属 §3.6 已登记的偏差（R18 同源） |
| 时间源 | `log_dual` 用 `chrono::Utc::now().timestamp_millis()`；补录用 `types::now_millis()` | 统一走 `types::now_millis()`（自带 JSON safe 范围断言），R21 |
| `detail` 文法 | §2.4 声明五形态 + 两组三元组，但 `dek_rotated` / 批量删除 / 冲突揭示 / `hibp_leak_found` 实际未对齐 | 按 R13–R15 收敛；新增形态必须同批更新 §2.4、后端 `audit::parse_detail` 与前端解析器 |
| 事件语义 | `vault_copy_field` 同时承载「复制」与「生成」；`vault_reveal_rate_limited` 在两条路径的 detail 语义不同（`entryId` vs 路径原文） | 语义宽化必须在 §2.3 写明（R16）；同一事件的 detail 语义**必须唯一**，路径原文按 O5 改摘要，未校验入参按 R22 写 `unknown` |
| 查询 | `ORDER BY timestamp DESC` 无次级键；`limit` 无夹取 | 补稳定次级键（R20）与 `limit` 夹取（§2.6） |
| 一致性测试 | 白名单、词表、文法均无自动化断言 | 三张表（`BACKFILL_EVENTS`、事件词表、detail 文法）各配一个枚举一致性测试，新增项漏改必须失败 |

**整齐性的实现前提**：`AuditEvent` 的 `Display` 串是 wire 契约（落库 `event_type`、JSONL `event`、前端字典 key 三处共用），必须有测试断言「全部变体（删除死事件 `local_db_repaired` 后为 30 个）的 `Display` 串两两唯一且与前端字典全集一致」，否则重命名会静默切断历史数据与前端映射。

---

## 3 工具权限审计

### 3.1 存储与词表

表结构（`assistant-storage/src/db.rs`）：

```sql
CREATE TABLE permission_audit (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    tool            TEXT NOT NULL,
    action          TEXT NOT NULL,
    resource        TEXT NOT NULL,
    outcome         TEXT NOT NULL,
    at_millis       INTEGER NOT NULL,
    conversation_id TEXT,
    turn_id         TEXT,
    iteration       INTEGER
);
```

**归属坐标**（`conversation_id` / `turn_id` / `iteration`）：三列可空，写入时从 `ToolUseContext` 直接取，与决策行同时落库。

- 目的：审计必须能回答「哪次对话、哪一轮、第几次工具调用」，这是审批追溯的最小坐标；靠 `at_millis` 事后关联不可行（时间戳既不唯一也不单调，同毫秒多行只能靠自增 `id` 排序）。
- **存量行不回填**：迁移只 `ADD COLUMN`，历史行三列为 `NULL`。缺失就是事实，禁止用空串或占位 id 伪造归属（同 `messages.turn_index` 的降级口径）。
- 管理行（§3.6）与 ToolBox 直调行（§3.5）没有轮次归属，三列恒为 `NULL`。

**`resource` 摘要规则**：绝对路径只保留末段，形如 `…/<basename>`；非绝对路径原样写入。

- 理由：`permission_audit` 在主库明文 SQLite 里（只有 vault 的 `security.db` 走 SQLCipher），落原文等于明文持久化用户目录结构；vault 侧 `security_audit_log` 的 `detail` 同样只存 `path_digest` / 末段，两侧口径必须一致。
- 实现入口唯一：`assistant-tools/src/gateway.rs::redact_resource`。网关决策行、管理行、ToolBox 直调行都过它，禁止各调用点自行拼接。
- 绝对路径判定覆盖 Unix 根、UNC（`\\\\`）、盘符（`X:\` / `X:/`）；末尾分隔符不产生空段。

`outcome` 为封闭集合，字符串名由 `outcome_str` 映射（camelCase）：

| 值 | 含义 | 现状 |
| --- | --- | --- |
| `allowed` | 策略允许（无 grant 命中） | 有 |
| `grantHit` | 持久授权命中 | 有 |
| `denied` | 策略拒绝 | 有 |
| `approved` | 本次审批通过 | 有 |
| `sessionApproved` | 本会话授权通过 | 有 |
| `alwaysApproved` | 永久授权通过 | 有 |
| `rejected` | 用户拒绝 | 有 |
| `timedOut` | 审批超时 | 有 |
| `validateRejected` | 输入校验失败，未进入策略求值 | 待增（G7） |
| `intentError` | 权限意图计算失败 | 待增（G7） |
| `bypassed` | 空 intent 短路，未经策略求值 | 待增（G6） |

新增 outcome 时必须同步：`outcome_str` 分支、前端展示映射、统计口径说明。禁止复用 `denied` 表示「输入非法」或「意图计算失败」——三者语义不同，混用会污染拒绝率。

`allowed` 同时承载 §3.6 的管理行（表示「管理动作被允许执行」），两者只能靠 `tool` 维度区分，统计时必须按 §6 隔离。

### 3.2 决策点覆盖

`ToolGateway::evaluate` 的判定顺序固定为：

1. `evaluate_tier_ceiling(tier)` —— 只看约束层；
2. 若 intent 列表为空 → 直接 `Allowed`；
3. 逐条 `evaluate(action, resource)`，取最严格结论；
4. 需要审批 → 走审批，按结果落 `approved` / `sessionApproved` / `alwaysApproved` / `rejected` / `timedOut`。

**每个返回 `GatewayDecision` 的分支必须对应一次 `audit()` 调用**，一一对应、无遗漏、无重复。当前 8 个 outcome 分支均已落地，短路分支待补（见下）。

流水线层（`assistant-tools/src/pipeline.rs`）另有两条必须审计的提前返回：

| 位置 | 条件 | 规格 outcome | action | resource |
| --- | --- | --- | --- | --- |
| Validate 阶段 | `!validation.ok` | `validateRejected` | `none` | `validate:failed` |
| intent 计算 | `permission_intents` 返回 `Err` | `intentError` | `none` | `intent:error` |

约束：这两处**只记固定串，不记错误文本**——错误文本可能夹带路径、参数、密钥片段。

### 3.3 空 intent 短路

`permission_intents` 返回空列表时，网关不求值策略、不做审批，直接 `Allowed`。此路径必须记为 `bypassed`（`action = none`，`resource = intent:none`），否则 `ask_user` 这类调用在审计里完全不可见。

`bypassed` 的语义是「未经权限策略求值即执行」，只允许两个触发点：空 intent 短路、ToolBox 直调（§3.5）。不得用它表示「策略允许」（那是 `allowed`）。

### 3.4 intent 声明规则

- **写类工具必须声明 intent**：`edit` / `delete` / `purge` 等，`resource` 为解析后的绝对路径；移动类工具必须声明两条（源与目标）。
- **只读工具必须声明 `read` intent**：`read`、`ls`、`glob`、`grep` 当前均未声明（G6）。统一使用 `action = "read"`，`resource` 为解析后的路径（`glob` / `grep` 用 pattern）。
  理由：基础规则集里已有 `("read", "*") → Allow`（`dangerous_only` 档位下即放行；`all_os_tools` 档位会把 `read` 转 `Ask`，那是该档位的既有语义，不是本规格引入的），声明 `read` 不新增规则、但让读取行为可审计、可命中硬保护 Deny（`/etc/shadow`、`/etc/ssh/**`）。禁止新造 `list` / `glob` / `grep` 等 action 名——它们未在基础规则里，默认落到 `("*","*") → Ask`，会凭空增加审批。
- **`fs_link action=check`** 应当声明 `read`（现状返回 `vec![]`）。注意 `fs_link` 是 `TOOLBOX_ONLY`，当前不经过网关，此改动不影响现有审计；目的是保持 intent 声明一致，避免将来放开 audience 时退化成空 intent 短路。
- **`ask_user`** 不属于权限动作，不声明 intent；其可见性由 §3.3 的 `bypassed` 保证。
- **Tier 不是 intent**：`ToolTier` 只走 `evaluate_tier_ceiling`，不得混入 `(action, resource)` 评估。

### 3.5 明确不审计

- 工具执行结果（成功 / 失败 / 输出内容）：属于业务事实，由工具返回值与消息侧持久化承载（E1）；
- 审批卡片的 UI 展示次数。

**ToolBox 不得例外**：用户从 UI 主动触发的 OS 工具绕过了网关（G8）。它虽无策略求值，仍是安全相关动作，必须在 ToolBox 执行入口补一行审计：`tool` = 实际 OS 工具名，`action = none`，`resource = toolbox`，`outcome = bypassed`。禁止因「用户主动」而省略审计——否则「用户删了一个目录」在审计里完全不存在。

### 3.6 非网关审计行（管理动作）

有两类写入**不来自网关决策**，但必须落在同一张 `permission_audit` 表里：

| 行类型 | 触发 | `tool` | `action` | `resource` | `outcome` |
| --- | --- | --- | --- | --- | --- |
| 工作区切换（现状已有） | `cmd/conversation.rs` 改 `workspace_root` | `workspace` | `switch` | 脱敏后的工作区摘要或固定串 `none` | `allowed` |
| 授权管理（F3） | `revoke_permission_grant` / `clear_permission_grants` / `set_permission_grant_scope` | `grant` | `grant:revoke` / `grant:clear` / `grant:scope` | 作用域摘要或固定串 `none` | `allowed` |

约束：

- 管理行的 `outcome` 只能是 `allowed`（表示「该管理动作被允许执行」）。**禁止**用 `denied` / `bypassed` 表示管理动作：前者是策略拒绝、后者是「未经策略求值即执行」，均与「管理操作成功」语义不符。
- 管理行不得进入拒绝率 / 审批率统计，统计口径见 §6。
- 授权管理行的 `resource` 是**作用域摘要**（`audit_scope_summary`：路径取 `redact_resource` 末段、命令取 `[tier] cwd · argv`、MCP 取短哈希），批量动作用固定串 `none`。**禁止**写 `grantKey` 原文、`key_json`（含 `canonical` 绝对路径）、入参原文。
- 主键解析失败（`decode_grant_key` 出的 JSON 反序列化失败）时落固定串 `unparsed` 并 `warn`，**禁止**回退写原文键——回退等于把整段键 JSON 写进明文库。
- 工作区切换行的 `resource` 过同一个 `redact_resource`（§3.1），落库即已是摘要，展示层不再做二次截断。
- 审计行必须与授权变更在同一事务内提交；受影响行数为 0（目标已不存在）时**不写**审计——没有发生变更。
- **与 §4 的边界**：审计 INSERT 失败时 `warn` 后继续提交事务，**不得**因审计失败回滚授权变更（单语句失败不会自动回滚 SQLite 事务）。若遇到会触发 SQLite 自动回滚的错误（如磁盘写满），业务本身也无法提交，属不可避免的例外，不得为此改写审计语义。
- **要求**：`cmd/conversation.rs` 的工作区切换行与授权管理行同口径——审计 INSERT 失败只 `warn` 后继续提交，**禁止**用 `map_err(..)?` 把审计失败变成切换失败（与 §4「不阻断业务」冲突）。
- 命令层**禁止**复用 `PermissionAuditStore::log_decision` 写管理行：该实现会重新锁 `DbState` 并另取连接，无法与授权变更共用事务。

### 3.7 查询面（F2）

`permission_audit` 现状**无任何查询入口**：`cmd/permission.rs` 只有 grants 的 list / revoke / set_scope / clear，四个命令零审计写入、也无读取。规格要求：

- 新增 `list_permission_audit(limit, since, until, offset)` 命令：`limit` 默认 100、上限 500、非正值回落默认值；`since` / `until` 为毫秒闭区间、可选；`offset` 为页内起点、非正值回落 0；固定 `ORDER BY at_millis DESC, id DESC`；
- 返回 `PermissionAuditPageDto { entries, hasMore }`：`hasMore` 必须由后端「多取一行再截断」判定，禁止让前端用「本页条数 == limit」推断（恰好取完与还有更早记录同形）；翻页时前端以首页最新一行的 `atMillis` 作为后续页的 `until`，冻结窗口，避免新写入挤偏分页；
- 输出 DTO 必须包含 `tool` / `action` / `resource` / `outcome` / `atMillis` 与归属坐标 `conversationId` / `turnId` / `iteration`（后三者可空，§3.1）；输出 DTO 的 `Option` 不加 `skip_serializing_if`（键恒在、值 `null`）；
- 查询前必须已有 `at_millis` 索引（§5），否则按时间范围查询会全表扫描；
- 前端 `permissionService` 必须新增对应方法，并以**字面量** `invoke("list_permission_audit", ..)` 调用：`commandNames.test.ts` 的断言是「前端字面量 ⊆ `generate_handler!` 声明清单」单向（仅 vault 组双向），声明清单由 `generate_handler!` 自动派生，禁止手写清单；
- 前端必须提供追溯面板，展示口径遵循 §6；未接入 UI 前不得宣称「可查看权限审计」。

---

## 4 降级与失败

统一原则：**审计是观测，不是事实源。任何审计写入失败不得阻断主业务。**

| 场景 | 要求 |
| --- | --- |
| DB 写失败（vault） | `warn`，继续业务；若 JSONL 成功则仍算写入成功。现状 `write_db_at` 用 `error!`，与 JSONL 侧及 `permission_audit` 侧不一致，按 R19 统一为 `warn` |
| JSONL 写失败（vault） | `warn`，继续业务；若 DB 成功则仍算写入成功 |
| 双写两侧均失败 | `warn`，继续业务（事件丢失，但不回滚） |
| 补录失败 | `warn`，不阻断解锁；游标不推进，下次重试 |
| 审计目录不可创建 | `warn`，仅 DB 通道生效 |
| `permission_audit` 写失败 | `warn`，工具照常执行 |
| 管理行写失败（§3.6） | `warn`，授权变更照常提交，不回滚、不报错 |
| 审计 writer 未注入（`None`） | 跳过该通道，不报错 |

禁止行为：把审计失败包装成业务错误；在审计失败时回滚业务事务；把审计失败静默吞掉而不留 `warn`。

---

## 5 保留与销毁

| 通道 | 保留策略 | 销毁 |
| --- | --- | --- |
| `security_audit_log` | 无 TTL，随库存在 | 库重建 / reset 时一并消失 |
| JSONL | 按日分文件、单文件 8 MiB 顺延，按 `audit_retention_days` 裁剪已补录日文件 | 不随锁库删除；仅随 `<data>/security/audit/` 目录被清理 |
| `permission_audit` | 按 `audit_retention_days` 裁剪（`0` = 不清理，与 JSONL / 工具结果同一口径） | 随主库删除 |

已知取舍：

- JSONL 无总量上限会持续占用磁盘（G12）。若将来加上限，必须满足「只删已成功补录入加密库、且早于游标的文件」，禁止删未补录文件。
- `permission_audit` 是只追加表，若无保留策略会随使用时间单调增长。裁剪在**启动时**执行一次（`setup.rs` 调 `assistant-storage::db::cleanup_expired_audit`），判据 `at_millis < now - retention_days`，边界行（恰好 `retention_days` 天前）保留。不做运行期定时器：审计量级远低于消息表，启动裁剪已足够，且避免在工具执行热路径上加锁。
- 配置读取失败时按 `0`（不清理）处理：宁可保留历史决策，也不因配置损坏误删审计。
- 裁剪本身不留审计行：审计表不能为自己写审计（会形成自增长），裁剪事实只进日志（`log::info`）。需要更强可追溯性时，应在外部日志侧补，不在本表内补。
- 表已有 `at_millis` 索引（T5.2），按时间范围查询不会全表扫描；统计仍须按 `tool` / `action` 维度隔离口径（§6）。

---

## 6 呈现口径

- vault 审计面向「本机用户自查」：按时间倒序，条目级事件可点进对应条目；
- 工具权限审计面向「审批追溯」：必须能回答「这次为什么问我 / 为什么放行」，依据 `policy::explain` 的命中规则；
- 两类审计只允许展示条目 ID、计数、路径摘要、封闭标签；禁止展示路径原文、密码 / TOTP / 密钥材料、工具入参原文。
- **补录行不得与原生行同质化**：vault 面板必须按 `id` 前缀 `jsonl-` 标注「来自库外日志补录」，措辞不得暗示其具备与加密库行同等的防篡改性（§2.5 第 8 条）；工作区切换行的 `resource` 是路径，展示时必须摘要，不得原样铺开（§3.6）。

**前端实现约束（F7 / F8 / OF2）**：

- **词表必须封闭映射**：`event_type`（现状 31 个，删除死事件 `local_db_repaired` 后为 30 个）与 `outcome`（11 个）必须各有一份前端字典映射到 i18n key，zh-CN 与 en-US 同步补齐；缺任一侧视为未完成。
- **未知值必须原样兜底**：字典未命中时必须显示原始蛇形串（`outcome` 显示原串），**禁止**过滤掉该行、禁止显示空白。理由：后端新增事件先于前端字典更新是常态，兜底缺失会让 UI 静默少一类记录。
- **`detail` 按事件分支渲染**：仅对 §2.4 已列出的封闭文法做解析（`{entryId}`、`{entryId}@{suffix}`、`{tag}:{entryId}`、`{entryId}:{count}`、`conflict:{entryId}:{side}:{field}:{digest}`、`{format}:{count}:{digest}`、`{format}:{count}:{digest}:failed`、`{old}:{new}:{count}`、纯计数、封闭标签）；解析失败一律原样展示整串。
- **`:` / `@` 分隔的解析顺序以 §2.4 为唯一来源**：前端解析器必须逐条实现 §2.4 的「首段封闭标签 + 段数」有序匹配，含 `@` 形态按**首个 `@`** 切分，**不得**在本节另立一套规则或按段数猜测（两处规则一旦分叉，同一 `detail` 会在不同页面渲染成不同字段）。
- **条目身份只取 DTO 的 `entryId`**：条目 ID 的解析已在后端 `query_log` 内完成（§2.6），前端**不得**再从 `detail` 推断条目 ID，`detail` 解析只用于展示计数 / 标签 / 摘要等附加字段；同一语义因此只有一个权威解析入口。
- **`unknown` 是哨兵值而非条目 ID**：`detail` 中任一段为 `unknown`（§2.4「入参校验前置」的兜底值）时，前端必须按未知值渲染（占位文案 + 不可点进条目），**禁止**拿它去查条目、也禁止当成真实 ID 展示为可点击链接。
- **`vault_copy_field` 文案不得写死「复制」**：该事件同时覆盖 `vault_totp_generate`（R16），文案用「读取 / 查看」一类中性措辞。
- **统计口径只取网关决策行**：拒绝率 / 审批率必须排除 `tool ∈ {workspace, grant}` 的管理行（§3.6）与 `resource = toolbox` 的 ToolBox 行（§3.5）；不得用消息侧 `toolCallView.status = error` 反推拒绝（OF2：`error` 是消息事实，`outcome` 是权限事实，两者会因 `validateRejected` / `intentError` 分叉）。
- **必须展示归属坐标**：面板需呈现 `conversationId` / `turnId` / `iteration`（§3.1），否则同一工具的多条决策行无法区分到具体轮次。会话侧优先显示会话标题（查不到时回落 `conversationId`），`iteration` 以「迭代 N」呈现；三者全空（管理行、ToolBox 行、迁移前存量行）显示固定占位符 `—`，**禁止**渲染空串或伪造归属。
- **命名不得与 HIBP「审计」撞车（F8）**：新面板的 i18n 键统一用 `security_log`（如 `vault.tab.security_log` / `vault.security_log.*`），与既有 `vault.hibp.audit_all`（泄露检测）区分；后端事件串保持 `vault_*` 不变。

---

## 7 规格条款 → 现状差异

下表只列与本规格条款直接相关的条目；完整缺口与观察项清单见 `audit-coverage-audit-1.md`（后端 G1–G10 / O1–O7）与 `audit-coverage-audit-2.md`（前端 F1–F8 / OF1–OF5）。`G11`–`G14` 是本规格编写时新增的发现，`F*` 行来自前端盘点。

> 表内「现状」是**编写规格时的基线**，用于说明差异从何而来；落地状态由 [任务清单](./I50-runtime-audit-tasklist.md) 唯一跟踪，本表不随实现回填，避免同一事实两处维护。

| 条款 | 现状 | 差异 |
| --- | --- | --- |
| §2.3 `vault_unlock_failed` 有生产调用点 | 变体存在、零调用点 | G1 |
| §2.3 `local_db_repaired` 有生产调用点 | 死事件 | G2 |
| §2.3 `vault_entry_updated` 覆盖同步覆盖路径 | `upsert_imported_in` 的 `Updated` 分支无审计 | G3 |
| §2.5 白名单覆盖全部 J 事件 | 缺 `vault_unlock_failed`、`vault_unsupported_version`、`biometric_failed`、`ssh_key_removed`（多余的双写项无害） | G4 |
| §2.3 `ssh_key_removed` 覆盖锁库批量移除 | `ssh_remove_all_on_lock` 无审计 | G5 |
| §3.4 只读工具声明 `read`、`fs_link check` 声明 `read` | 4 个只读工具无 intent；`check` 返回空 | G6 |
| §3.2 Validate / intent 失败可审计 | 直接返回，无审计 | G7 |
| §3.3 短路分支记 `bypassed` | 不写审计 | G6 |
| §3.5 ToolBox 补审计 | 直调绕过网关，无任何记录 | G8 |
| §2.5 游标推进到已处理最大 `at` | 推进到 `now` | G11（本轮新增） |
| §5 JSONL 有总量上限 | 无上限 | G12（本轮新增） |
| §2.4 条目 detail 五形态、conflict 用摘要 | conflict 用路径原文且路径未校验（限流分支在校验前就写入前端传入的 `path`）；`sync.rs` 的限流揭示同源 | O5 |
| （范围外）恢复路径入参白名单 | `history_restore` 的前端 `commit` / `entry_id` 未经校验即拼进 `git show` 参数（前导 `-` 会被当选项）；本规格只要求审计侧写 `unknown`，白名单单独立项 | G14 附注 |
| §2.3 `vault_lock` 归因可区分自动锁定 | `vault_lock` 硬编码 `LockReason::User`；`LockReason::AutoLock` 生产零调用 | F5 |
| §2.6 前端面板消费 `queryAuditLog` | 无任何 `.tsx` 组件消费，slice / service 仅透传 | F1 |
| §2.6 查询参数化（`since` / `eventTypes` / `entryId`） | `query_log` 固定 `LIMIT ?1`，DTO 无 `entryId` | F6 |
| §3.6 授权管理动作审计 | 3 个 grants 写命令零审计写入 | F3 |
| §3.7 `list_permission_audit` 查询命令 | 无命令、无 DTO、无 service 方法 | F2 |
| §5 `permission_audit` 时间索引 | 无索引 | O3 / F6 |
| §6 词表封闭映射与未知值兜底 | 前端对 31 个事件与 11 个 outcome 零映射、零 i18n 键 | F7 |
| §2.5 游标精度、去重键、自回响 | 游标为毫秒且严格 `>`；去重键 `(event_type, timestamp)`；解锁先写行后补录 | R2 / R3 / R12 |
| §2.5 读取容错与补录事务 | `File::open(..)?` 整批失败；补录逐条隐式提交；`rebuild_local_db` 不写游标 | R4 / R6 |
| §2.4 `:` 文法消歧、`dek_rotated` 与批量删除文法 | `hibp_leak_found` 与 `totp:` 同分隔符无消歧规则；`dek_rotated` 为自由文本；批量删除只写 `remote` | R13 / R14 / R15 |
| §2.4 入参校验前置 | 限流分支在库内校验之前写未校验前端入参（`reveal.rs` 两处 + `sync.rs` 一处） | G14（本轮新增）/ R22 |
| §2.3 `vault_copy_field` 语义、`ssh_key_removed` 通道 | `vault_totp_generate` 复用 copy 事件；`ssh_key_removed` 单条实为 D，表标 B | R16 / R17 |
| §4 / §2.2 失败级别、原语返回类型、时间源 | `write_db_at` 用 `error!`；原语返回 `Result` 但全仓 `let _ =`；两个时间源入口 | R18 / R19 / R21 |
| §2.6 查询排序稳定性 | `ORDER BY timestamp DESC` 无次级键，同毫秒顺序不定 | R20 |

---

## 8 验收

**测试（vault）**：

1. 「J 通道事件清单 ⊆ `BACKFILL_EVENTS`」的枚举一致性测试；
2. 双写行（原生行 + 同次写入的 JSONL 行）补录后**不重复**：断言以 R3 的新去重键为前提，不得再以 `(event_type, timestamp)` 为断言前提；`dek_rotated` 与 `local_db_rebuilt` 各**恰好 1 行**（R12 自回响，这两个是白名单内的双写事件），连续解锁 3 次后 `vault_unlock` 恰好 3 行；
3. 游标回拨测试：把游标设为未来值后，模拟时间回退，断言不漏补（修正后必须通过）；
4. 同毫秒写入 2 条同事件（`ssh_key_removed` / `biometric_failed`）→ 补录后 2 行（R3）；
5. 单文件不可读 → 其余文件记录仍被补录且结果为 `Ok`（R4）；
6. `rebuild_local_db` 后首次解锁不重扫全部历史文件（R6，可用「未读取文件数」断言）；
7. 解锁态写 `vault_lock` 后检查 JSONL 与 DB：`detail` 为 `LockReason::as_str()` 的 snake 串（F5 回归）；
8. `detail` 文法全量校验：对每个事件的样本 `detail` 断言落在 §2.4 的形态集合内，`dek_rotated` 为 `{old}:{new}:{count}`、批量删除为 `{entryId}@remote`、冲突揭示为 5 段且**前端传入含 `:` / 空格 / 换行的 `path` 时 detail 仍为 5 段且不含原文**、`hibp_leak_found` 不被解析成 `tag:entryId`、`{uuid}@HEAD@{1}` 按首个 `@` 切分后 suffix 完整保留（R13–R15）；
9. `AuditEvent` 的 `Display` 串两两唯一（§2.7.2 末条 / 词表契约前提）；
10. `query_log` 同毫秒多条时顺序稳定（R20）；
11. JSONL 目录不可写时，DB 通道仍成功且返回 `Ok`；
12. 限流分支写未校验入参（R22 / G14）：`vault_reveal_rate_limited` 传入非 UUID 的 `id`（如 `a:b`、`123`、含换行串）→ `detail` 为 `unknown` 而非原串；冲突揭示限流分支传入畸形 `path` / 非法 `side` / 非法 `field` → `detail` 为 5 段且对应段为 `unknown`。

**测试（工具）**：

1. 每个 `GatewayDecision` 分支各有一条 `permission_audit` 行（表驱动）；
2. 空 intent（`ask_user`、`fs_link check`）产生 `bypassed`；
3. Validate 失败产生 `validateRejected`，`permission_intents` 报错产生 `intentError`；
4. `PermissionAuditStore::log_decision` 注入失败时工具仍返回正常结果（仅 `warn`）；
5. 授权管理三命令各产生 1 条管理行（`tool = grant`），受影响行数为 0 时不产生。

**测试（前端）**：

1. `npm run typecheck` 通过；
2. `npx vitest run src/services/__tests__/commandNames.test.ts`：前端字面量命令必须 ⊆ `generate_handler!` 声明清单，且 `list_permission_audit` 必须出现在前端字面量集合中（该组为单向断言，vault 组才是双向）；
3. 事件 / outcome 字典完整性：枚举全集逐项都有 i18n key（zh-CN 与 en-US 各一份），未知值兜底返回原串；
4. 面板在锁定态禁用且不弹错误（组件测试；若项目已有组件测试基线）。

**命令**（cwd = `src-tauri`）：

```
cargo test -p assistant-vault audit
cargo test -p assistant-tools gateway
cargo test -p assistant-storage
cargo test --test assembly_contract
```

**命令**（仓库根）：

```
npm run contract:gen
npm run typecheck
npx vitest run src/services/__tests__/commandNames.test.ts
```

---

## 9 新增事件的提交清单

新增 vault 事件：

- [ ] `AuditEvent` 变体 + `Display` 字符串
- [ ] 生产调用点（真实业务路径，非测试）
- [ ] 通道归属（D / B / J），B 通道确认 `log_dual` 只取一次时间戳
- [ ] `detail` 文法落在 §2.4 封闭形态内
- [ ] J 通道 → 同步更新 `BACKFILL_EVENTS` + 一致性测试
- [ ] 若涉及条目，`detail` 满足「ID 落在可定位固定段」（首段 / 末段 / `conflict:` 后第 2 段，§2.4）
- [ ] ID 类字段在写入前已校验（UUID 形态或来自已验证的库内行）；限流 / 提前返回分支同样不得写未校验入参（§2.4「入参校验前置」）

新增工具 outcome：

- [ ] `AuditOutcome` 变体 + `outcome_str`
- [ ] 与 `GatewayDecision` 分支一一对应的 `audit()` 调用
- [ ] 前端展示映射与统计口径说明
- [ ] 表驱动测试覆盖

新增前端审计展示：

- [ ] 事件 / outcome 字典 + i18n（zh-CN + en-US，键名统一 `security_log`，不与 `vault.hibp.audit_all` 撞车）
- [ ] `detail` 文法解析 + 解析失败原样兜底 + 未知值兜底（禁止过滤行）
- [ ] 锁定态禁用、`limit` 上限、无「加载全部」入口
- [ ] 新增命令必须同步前端 `invoke` 字面量（`commandNames.test.ts` 双向差集）
- [ ] 统计口径排除管理行（`tool ∈ {workspace, grant}`）与 ToolBox 行（`resource = toolbox`）
- [ ] 前端不得缓存 / 推断 / 乐观插入审计记录（EF1）
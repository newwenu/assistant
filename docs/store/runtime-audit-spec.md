# 运行时审计规格

| 项 | 值 |
| --- | --- |
| 状态 | 现行规格（约束 vault 审计与工具权限审计两条链路） |
| 日期 | 2026-11-14 |
| 依据 | 现状盘点（缺口编号 G1–G10 / 观察项 O1–O7）见同目录 `audit-coverage-audit-1.md`；本规格只写「应当如何」，不复述盘点内容 |
| 替代 | I9-vault-audit-spec（已归档）§1–§6 |
| 落地对象 | `src-tauri/crates/assistant-vault/src/audit/`、`src-tauri/crates/assistant-vault/src/security_db/`、`src-tauri/crates/assistant-tools/src/gateway.rs`、`src-tauri/crates/assistant-tools/src/pipeline.rs`、`src-tauri/src/adapters/permission_store.rs`、`src-tauri/src/cmd/vault/` |
| 关联 | I38g-agent-approval-advisor-spec.md（审批卡片与 `explain`）、I27-tool-data-store-spec.md（工具落盘，与审计无关） |

本规格中的「必须 / 禁止」为可验收约束；「应当」为默认做法，偏离需在代码注释里写明理由。

---

## 1 范围与判定口径

### 1.1 两条链路

| 维度 | vault 审计 | 工具权限审计 |
| --- | --- | --- |
| 承载数据 | 保险库内的 `security_audit_log`（加密库） + 库外 JSONL 明文兜底 | 主库 `permission_audit` 表（明文） |
| 生命周期 | 随 `security.db`；库销毁即消失 | 随主库；不随会话删除 |
| 事件词表 | `AuditEvent` 枚举（31 个变体，见 §2.3） | `AuditOutcome` 枚举（现状 8 个、本规格扩展为 11 个，见 §3.1） |
| 写入时机 | 业务动作成功后 / 拒绝时 | 网关做出 `GatewayDecision` 时 |
| 失败语义 | 不阻断业务，仅 `warn` | 不阻断业务，仅 `warn` |
| 可查询性 | 后端 `query_log` 有接口、前端无消费方 | 无查询接口 |

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
| `vault_unlock_failed` | J | 待补（G1，`cmd/vault/lifecycle.rs` 3 处 `InvalidPassword` 分支） | 封闭标签：`wrong_password` / `recovery_wrong_key` / `biometric_mismatch` |
| `vault_lock` | B | `security_db::lock_session` | `LockReason::as_str()` |
| `vault_reveal` | D | `cmd/vault/reveal.rs`（`{entryId}`）、`cmd/vault/sync.rs`（冲突揭示，O5） | 条目文法（§2.4） |
| `vault_reveal_rate_limited` | D | `cmd/vault/reveal.rs`（`{entryId}`）、`cmd/vault/sync.rs`（路径原文，O5） | 条目文法 |
| `vault_entry_created` | D | `security_db/entries/write.rs`、`entries/import.rs` | 条目文法 |
| `vault_entry_updated` | D | `entries/write.rs`、`entries/import.rs` | 条目文法（同步覆盖路径待补，G3） |
| `vault_entry_deleted` | D | `entries/write.rs`（单条 / 批量） | 条目文法；批量路径用调用方给定的封闭标签（如 `remote`） |
| `vault_entry_restored` | D | `entries/deleted.rs` | `{entryId}` 或 `{entryId}@{source}` |
| `vault_entry_purged` | D | `entries/deleted.rs` | `{count}` |
| `vault_copy_password` | D | `cmd/vault/reveal.rs` | 条目文法 |
| `vault_copy_field` | D | `cmd/vault/reveal.rs`、`cmd/vault/audit.rs` | `{entryId}` 或 `totp:{entryId}` |
| `hibp_audit` | D | `cmd/vault/audit.rs` | `NULL` |
| `hibp_leak_found` | D | `cmd/vault/audit.rs` | `{entryId}:{count}` |
| `master_password_change` | B | `security_db/rekey.rs` | `NULL` |
| `recovery_key_exported` | B | `assistant-vault/lifecycle.rs` | `{dekVersion}` |
| `recovery_export_used` | J | 恢复通道 | `{count}` |
| `dek_rotated` | B | `cmd/vault/lifecycle.rs` | `{old}->{new} entries {n}` |
| `local_db_repaired` | — | 无调用点（G2：删除该变体） | — |
| `local_db_rebuilt` | B | `security_db/rekey.rs` | `{count}` |
| `anchor_rollback_rejected` | B | `security_db/mod.rs` | `meta_or_dek_version` |
| `biometric_enrolled` | B | `security_db/biometric_ops.rs` | `NULL` |
| `biometric_unlocked` | B | 同上 | `NULL` |
| `biometric_failed` | J | 同上（`conn` 恒为 `None`） | 封闭标签（平台错误码映射后的固定串） |
| `biometric_revoked` | B | 同上 | `NULL` |
| `ssh_key_loaded` | D | `cmd/vault/ssh.rs` | 条目文法 |
| `ssh_key_removed` | D | `cmd/vault/ssh.rs` | `{fingerprint}`（锁库批量路径待补，G5） |
| `vault_repository_reset` | J | `cmd/vault/lifecycle.rs`（reset） | `NULL` |
| `vault_unsupported_version` | J | `cmd/vault/lifecycle.rs` | `unsupported_version` / `version_too_new` |
| `vault_entries_exported` | D | `cmd/vault/entries.rs` | `{format}:{count}:{pathDigest}`，失败追加 `:failed` |
| `vault_entries_imported` | D | `cmd/vault/entries.rs` | `{format}:{count}:{pathDigest}` |

新增 vault 事件的准入条件：① 有真实生产调用点；② 通道归属明确；③ detail 文法落在 §2.4 的封闭形态内；④ 若为 J 通道，同一次提交必须加入 `BACKFILL_EVENTS`。

### 2.4 detail 文法

**禁止**把自由文本、用户输入、完整路径、密码/TOTP/密钥材料写进 `detail`。

条目级事件的 `detail` 必须属于以下三种形态之一：

| 形态 | 说明 | 检索方式 |
| --- | --- | --- |
| `{entryId}` | 单条动作 | `detail = ?1` |
| `{entryId}@{suffix}` | 带来源后缀（如恢复来源） | `detail LIKE ?1 || '@%'` |
| `{tag}:{entryId}` | 带封闭标签前缀（`totp`、`conflict`） | `detail LIKE '%:' || ?1` |

因此**条目 ID 必须落在 detail 的首段或末段**，这是可检索性的实现前提。

`conflict` 揭示事件的形态固定为：

```
conflict:{entryId}:{side}:{field}:{pathDigest}
```

- `side` ∈ `local` / `remote`；
- `field` ∈ 封闭集合（如 `password` / `notes` / `totp` / `customFields`）；
- `pathDigest` 为路径的 SHA-256 摘要（`audit::path_digest`），用于定位但不泄露路径原文。

现状用路径原文（O5），必须改为上式。

非条目事件的 `detail` 必须满足其一：

- `NULL`；
- 单个整数计数；
- `{format}:{count}:{digest}` 三元组（导出导入）；
- 封闭标签（`LockReason`、`meta_or_dek_version`、`wrong_password` 等）。

### 2.5 JSONL 补录

触发点：`unlock_session` 成功写 `vault_unlock` 之后立即调用 `try_backfill_jsonl_audit`。

规则：

1. **游标**：`vault_meta.jsonl_sync_at`（毫秒），`read_since` 严格大于该值。补录成功后**必须**把游标推进到「本次已处理记录的最大 `at`」。
   现状写的是 `now`（G11，本规格新增发现），时钟回拨会漏补——必须修正。
2. **白名单**：`BACKFILL_EVENTS` 必须**覆盖**「所有可能只写 JSONL 的事件全集」（即上表 J 通道事件集合）。当前 J 集合为
   `recovery_export_used`、`vault_repository_reset`、`vault_unsupported_version`、`biometric_failed`（现状漏后两项，G4）。
   双写事件也可以留在白名单里（去重会挡掉，无副作用），但白名单不得**缺少**任何 J 事件——这是唯一自洽的判定方向。
3. **去重**：`INSERT ... SELECT ... WHERE NOT EXISTS (event_type = ? AND timestamp = ?)`，即去重键为 `(event_type, timestamp)`；`id` 固定为 `jsonl-{at}-{event}` 仅作 PK 占位。
4. **一致性断言（必须加测试）**：维护一份「J 通道事件清单」，断言其每个元素都在 `BACKFILL_EVENTS` 中。新增 J 事件而未改白名单必须让测试失败。
5. 单行解析失败：`warn` 后跳过，不整体失败。

### 2.6 查询面

- 后端：`query_log(conn, limit)` 固定 `ORDER BY timestamp DESC LIMIT ?1`，无 `since`、无 `event_type` 过滤。
- 前端：`src/vault/core/slices/vaultAuditSlice.ts` 的 `queryAuditLog(limit?)` 无任何组件消费。
- 规格要求：
  - 查询接口必须支持 `since`（毫秒）与 `eventTypes`（集合）两个可选参数，`limit` 必须带上限（默认 100、最大 500）；
  - 返回项必须包含 `eventType`、`detail`、`timestamp`，条目级事件额外返回解析出的 `entryId`；
  - 未接入 UI 前不得对外宣称「用户可查看审计日志」。

---

## 3 工具权限审计

### 3.1 存储与词表

表结构（`assistant-storage/src/db.rs`）：

```sql
CREATE TABLE permission_audit (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    tool      TEXT NOT NULL,
    action    TEXT NOT NULL,
    resource  TEXT NOT NULL,
    outcome   TEXT NOT NULL,
    at_millis INTEGER NOT NULL
);
```

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

`permission_intents` 返回空列表时，网关不求值策略、不做审批，直接 `Allowed`。此路径必须记为 `bypassed`（`action = none`，`resource = intent:none`），否则 `ask_user` 与 `fs_link action=check` 这类调用在审计里完全不可见。

`bypassed` 的语义是「未经权限策略求值即执行」，只允许两个触发点：空 intent 短路、ToolBox 直调（§3.5）。不得用它表示「策略允许」（那是 `allowed`）。

### 3.4 intent 声明规则

- **写类工具必须声明 intent**：`edit` / `delete` / `purge` 等，`resource` 为解析后的绝对路径；移动类工具必须声明两条（源与目标）。
- **只读工具必须声明 `read` intent**：`read`、`ls`、`glob`、`grep` 当前均未声明（G6）。统一使用 `action = "read"`，`resource` 为解析后的路径（`glob` / `grep` 用 pattern）。
  理由：基础规则集里已有 `("read", "*") → Allow`，声明 `read` 不引入新审批，但让读取行为可审计、可命中硬保护 Deny（`/etc/shadow`、`/etc/ssh/**`）。禁止新造 `list` / `glob` / `grep` 等 action 名——它们未在基础规则里，默认落到 `("*","*") → Ask`，会凭空增加审批。
- **`fs_link action=check`** 必须改为声明 `read`（现状返回 `vec![]`，直接落进短路分支）。
- **`ask_user`** 不属于权限动作，不声明 intent；其可见性由 §3.3 的 `bypassed` 保证。
- **Tier 不是 intent**：`ToolTier` 只走 `evaluate_tier_ceiling`，不得混入 `(action, resource)` 评估。

### 3.5 明确不审计

- 工具执行结果（成功 / 失败 / 输出内容）：属于业务事实，由工具返回值与消息侧持久化承载（E1）；
- 审批卡片的 UI 展示次数。

**ToolBox 不得例外**：用户从 UI 主动触发的 OS 工具绕过了网关（G8）。它虽无策略求值，仍是安全相关动作，必须在 ToolBox 执行入口补一行审计：`tool` = 实际 OS 工具名，`action = none`，`resource = toolbox`，`outcome = bypassed`。禁止因「用户主动」而省略审计——否则「用户删了一个目录」在审计里完全不存在。

---

## 4 降级与失败

统一原则：**审计是观测，不是事实源。任何审计写入失败不得阻断主业务。**

| 场景 | 要求 |
| --- | --- |
| DB 写失败（vault） | `warn`，继续业务；若 JSONL 成功则仍算写入成功（现状：JSONL 成功时不打 `warn`，属补充观察，不阻断业务） |
| JSONL 写失败（vault） | `warn`，继续业务；若 DB 成功则仍算写入成功 |
| 双写两侧均失败 | `warn`，继续业务（事件丢失，但不回滚） |
| 补录失败 | `warn`，不阻断解锁；游标不推进，下次重试 |
| 审计目录不可创建 | `warn`，仅 DB 通道生效 |
| `permission_audit` 写失败 | `warn`，工具照常执行 |
| 审计 writer 未注入（`None`） | 跳过该通道，不报错 |

禁止行为：把审计失败包装成业务错误；在审计失败时回滚业务事务；把审计失败静默吞掉而不留 `warn`。

---

## 5 保留与销毁

| 通道 | 保留策略 | 销毁 |
| --- | --- | --- |
| `security_audit_log` | 无 TTL，随库存在 | 库重建 / reset 时一并消失 |
| JSONL | 按日分文件、单文件 8 MiB 顺延，**当前无总量上限** | 不随锁库删除；仅随 `<data>/security/audit/` 目录被清理 |
| `permission_audit` | 无 TTL、无索引（仅自增主键） | 随主库删除 |

已知取舍：

- JSONL 无上限会持续占用磁盘（G12）。若将来加上限，必须满足「只删已成功补录入加密库、且早于游标的文件」，禁止删未补录文件。
- `permission_audit` 无时间索引，按时间范围查询会全表扫描；若开启查询面，必须先加 `at_millis` 索引并评估增长率（O3）。

---

## 6 呈现口径

- vault 审计面向「本机用户自查」：按时间倒序，条目级事件可点进对应条目；
- 工具权限审计面向「审批追溯」：必须能回答「这次为什么问我 / 为什么放行」，依据 `policy::explain` 的命中规则；
- 两类审计只允许展示条目 ID、计数、路径摘要、封闭标签；禁止展示路径原文、密码 / TOTP / 密钥材料、工具入参原文。

---

## 7 规格条款 → 现状差异

下表只列与本规格条款直接相关的条目；完整缺口与观察项清单见 `audit-coverage-audit-1.md`。`G11`–`G13` 是本规格编写时新增的发现。

| 条款 | 现状 | 差异 |
| --- | --- | --- |
| §2.3 `vault_unlock_failed` 有生产调用点 | 变体存在、零调用点 | G1 |
| §2.3 `local_db_repaired` 有生产调用点 | 死事件 | G2 |
| §2.3 `vault_entry_updated` 覆盖同步覆盖路径 | `upsert_imported_in` 的 `Updated` 分支无审计 | G3 |
| §2.5 白名单覆盖全部 J 事件 | 缺 `vault_unsupported_version`、`biometric_failed`（多余的双写项无害） | G4 |
| §2.3 `ssh_key_removed` 覆盖锁库批量移除 | `ssh_remove_all_on_lock` 无审计 | G5 |
| §3.4 只读工具声明 `read`、`fs_link check` 声明 `read` | 4 个只读工具无 intent；`check` 返回空 | G6 |
| §3.2 Validate / intent 失败可审计 | 直接返回，无审计 | G7 |
| §3.3 短路分支记 `bypassed` | 不写审计 | G6 |
| §3.5 ToolBox 补审计 | 直调绕过网关，无任何记录 | G8 |
| §2.5 游标推进到已处理最大 `at` | 推进到 `now` | G11（本轮新增） |
| §5 JSONL 有总量上限 | 无上限 | G12（本轮新增） |
| §5 `permission_audit` 查询前加索引 | 无索引 | O3 |
| §2.4 条目 detail 三形态、conflict 用摘要 | conflict 用路径原文；`sync.rs` 的限流揭示也写路径原文 | O5 |

---

## 8 验收

**测试（vault）**：

1. 「J 通道事件清单 ⊆ `BACKFILL_EVENTS`」的枚举一致性测试；
2. 双写行 `(event_type, timestamp)` 相同、补录后不重复；
3. 游标回拨测试：把 `jsonl_sync_at` 设为未来值后，模拟时间回退，断言不漏补（修正后必须通过）；
4. JSONL 目录不可写时，DB 通道仍成功且返回 `Ok`。

**测试（工具）**：

1. 每个 `GatewayDecision` 分支各有一条 `permission_audit` 行（表驱动）；
2. 空 intent（`ask_user`、`fs_link check`）产生 `bypassed`；
3. Validate 失败产生 `validateRejected`，`permission_intents` 报错产生 `intentError`；
4. `PermissionAuditStore::log_decision` 注入失败时工具仍返回正常结果（仅 `warn`）。

**命令**（cwd = `src-tauri`）：

```
cargo test -p assistant-vault audit
cargo test -p assistant-tools gateway
cargo test --test assembly_contract
```

---

## 9 新增事件的提交清单

新增 vault 事件：

- [ ] `AuditEvent` 变体 + `Display` 字符串
- [ ] 生产调用点（真实业务路径，非测试）
- [ ] 通道归属（D / B / J），B 通道确认 `log_dual` 只取一次时间戳
- [ ] `detail` 文法落在 §2.4 封闭形态内
- [ ] J 通道 → 同步更新 `BACKFILL_EVENTS` + 一致性测试
- [ ] 若涉及条目，`detail` 满足「ID 在首段或末段」

新增工具 outcome：

- [ ] `AuditOutcome` 变体 + `outcome_str`
- [ ] 与 `GatewayDecision` 分支一一对应的 `audit()` 调用
- [ ] 前端展示映射与统计口径说明
- [ ] 表驱动测试覆盖

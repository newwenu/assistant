# Vault / Crypto DTO 边界分析与直出类型清单

> **本文已被 [vault-ipc-contract.md](./vault-ipc-contract.md) 取代**，仅作历史记录保留。新文档把问题边界从「子 crate 类型直出」扩为「四类跨边界值形态」，并修正了本文的 P2 / P3 结论（不搬迁至 `protocol`、不等待归属决定）。
>
> 审查日期：2026-10-29
> 审查范围：`src-tauri/crates/assistant-vault` 内部类型分层、`assistant-crypto` 的线格式归属、`src-tauri/src/cmd/vault/*` 的跨 IPC 返回类型
> 审查方法：全量 `serde` 派生检索 + 每个 `#[command]` 签名逐个核对 + 前端 `vault.types.ts` 字段对照
> 与主文档关系：本文是 [crate-architecture-analysis.md](./crate-architecture-analysis.md) §3.1.1 的细化与事实修正，结论冲突时以本文实测为准

## 0. 结论摘要

1. **`assistant-crypto` 不提供 DTO**。它是加密原语叶 crate，职责是「算」，不是「形」；但 `TotpResult` / `ParsedOtpAuth` 目前确实带着 camelCase serde 直出前端，属**待收敛项**，不是设计意图。
2. **DTO 归领域 crate**（`assistant-vault::dto`），不归原语 crate。主文档 §7.1.1 已写明 crypto「不含 VaultEntry* DTO」，同一原则适用于 TOTP。
3. **持久化格式不进 DTO**。`.vault` 文件格式（`vault_file/types.rs`）与导出格式（`VaultEntryExport` / `VaultEntryCsvRow`）必须留在 domain 层，否则一旦按 dto 做编译裁剪，落盘兼容性与导出功能会一起断。
4. **主文档 §3.1.1 的「5 个直出类型」严重低估**：实测为 **14 处签名 / 13 个去重类型**，且漏记 crypto 的 2 个。
5. **已修复一处真实契约 bug**：`AuditLogEntry` 缺 `rename_all = "camelCase"`，导致前端 `eventType` 恒为 `undefined`。

---

## 1. Vault 内部三层边界

### 1.1 目录结构与实测行数

> 行数口径：`crates/assistant-vault/src/**/*.rs` 物理行数；本节均为小文件，按精确行数记录（主文档 §0 的百行精度约定适用于 crate 级统计）。

| 路径 | 行数 | 层 | 内容 |
|---|---|---|---|
| `src/shared/mod.rs` | 51 | 共享词表 | `VaultCategory`（Display / FromStr / parse）、`CustomFieldInput` |
| `src/dto/mod.rs` | 70 | 线格式 | `VaultEntryCreate` / `Update` / `Summary` / `Detail` / `RevealResult` |
| `src/dto/audit.rs` | 31 | 线格式 | `StrengthResult` / `StrengthWarning` / `AuditLogEntry` |
| `src/dto/integrity.rs` | 7 | 线格式 | `IntegrityResult` |
| `src/types/mod.rs` | 189 | domain + 持久化 | `VaultEntrySnapshot` / `Import` / `ImportOutcome` / `VaultCryptoInfo` / `VaultEntryDecrypted` / `DecryptedCustomField` / `VaultEntryExport` / `VaultEntryCsvRow` |
| `src/vault_file/types.rs` | 114 | 持久化（文件格式） | `EntryFile` / `EntryBlob` / `VaultMetaFile` / `VaultCryptoMeta` / `Argon2Params` / `EmergencyDataFile` 等 |

### 1.2 依赖方向（单向，无环）

```
shared  ←── dto          dto 只依赖 serde + shared，零 domain 依赖
   ↑
 types  ←── dto          types 通过 pub use 兼容旧路径，domain 本身不引用 dto 实现
```

约束：

- **`dto` 不得依赖 `types`**：DTO 是前端契约，一旦引用 domain 结构，domain 字段改动会静默改契约。
- **`shared` 不得依赖 `dto` / `types`**：它是两者的公共底座，被 domain 与 dto 同时引用，反向引用即成环。
- **转换实现留在 vault 侧**（`impl From<Domain> for Dto`）。孤儿规则允许该实现与 DTO 同 crate，因此将来把 DTO 整体搬去统一 protocol crate 时，转换实现仍合法、无需改位置。

### 1.3 收录判定标准（三问，任一为「是」则**不进** dto）

| # | 问题 | 是 → 归属 |
|---|---|---|
| ① | 字段名会出现在 `.vault` 文件 / CSV / DB 列里？ | 持久化层（`types` 或 `vault_file/types`） |
| ② | 只在 crate 内部流转、无 `#[command]` 返回它？ | domain 层（`types`） |
| ③ | 承载加密语义（`MasterKey` / `VaultPlaintext` / `SecureString`）？ | 留在 `assistant-crypto` |

三问全部为「否」→ 进 `dto`，加 `#[serde(rename_all = "camelCase")]`。

> 该标准替代了原先模糊的「看起来像 DTO 就放 dto」，避免持久化格式被误收进线格式层。

---

## 2. 当前 DTO 清单（`assistant-vault::dto`）

| 类型 | 文件 | 前端消费点 | 备注 |
|---|---|---|---|
| `VaultEntryCreate` | `dto/mod.rs` | `vault_create_entry` 入参 | 含 `customFields` |
| `VaultEntryUpdate` | `dto/mod.rs` | `vault_update_entry` 入参 | 字段与 Create 同构 |
| `VaultEntrySummary` | `dto/mod.rs` | `vault_list_entries` 返回 | 列表页 |
| `VaultEntryDetail` | `dto/mod.rs` | `vault_get_entry` 返回 | `hasPassword` 等布尔替代明文 |
| `RevealResult` | `dto/mod.rs` | `vault_reveal_field` 返回 | `value` + `expiresAt` |
| `StrengthResult` | `dto/audit.rs` | `vault_check_master_password_strength` | 原在 `password_strength/`，已迁 |
| `StrengthWarning` | `dto/audit.rs` | 同上（嵌于 `warnings`） | 原在 `password_strength/`，已迁 |
| `AuditLogEntry` | `dto/audit.rs` | `vault_query_audit_log` | 原在 `audit/`，已迁；**本次补 camelCase** |
| `IntegrityResult` | `dto/integrity.rs` | `vault_verify_integrity` | 原在 `vault_file/mod.rs`，已迁 |

业务模块一律 `pub use crate::dto::…` 兼容旧路径，故主 app 调用点零改动。

---

## 3. 故意排除清单（不得迁入 dto）

| 位置 | 类型 | 排除理由 |
|---|---|---|
| `vault_file/types.rs` | `EntryFile` / `EntryBlob` / `VaultMetaFile` / `VaultCryptoMeta` / `Argon2Params` / `EmergencyDataFile` | `.vault` 文件格式，改字段 = 破坏已落盘文件兼容 |
| `types/mod.rs` | `VaultEntryExport` | JSON 导出文件格式（用户可见、可迁移） |
| `types/mod.rs` | `VaultEntryCsvRow` + `serialize_tags_csv` / `deserialize_tags_csv` | CSV 导出格式（tags 需扁平化为逗号串） |
| `types/mod.rs` | `VaultEntrySnapshot` / `VaultEntryImport` / `ImportOutcome` | 同步中间态，无前端消费 |
| `types/mod.rs` | `VaultCryptoInfo` | DB 元信息（salt / argon2 参数） |
| `types/mod.rs` | `VaultEntryDecrypted` / `DecryptedCustomField` | 解密中间态，含 `VaultPlaintext` |
| `recovery_key/mod.rs` | `EmergencyData` | 写进 vault 文件 |
| `audit/mod.rs` | `AuditEvent` | 内部枚举，`to_string()` 落 DB；线格式里仅是 `eventType: string` |

判定要点：**导出格式不是前端契约**。`VaultEntryExport` / `VaultEntryCsvRow` 与 `VaultEntrySummary` 字段相似，容易被误判为「重复 DTO」而合并——但前者是**落盘契约**（向后兼容优先级最高），后者是**线契约**（可随前端演进），生命周期不同，必须分开。

---

## 4. 直出类型实测（修正主文档 §3.1.1 的「5 个」）

**实测：14 处 `#[command]` 签名 / 13 个去重类型**（主文档记为 5 个，严重低估）。

| crate | 类型 | 签名位置 | 有 DTO？ |
|---|---|---|---|
| `assistant_sync` | `SyncResult` | `src-tauri/src/cmd/vault/sync.rs:26` | ❌ |
| `assistant_sync` | `SyncConfig` | `src-tauri/src/cmd/vault/sync.rs:70` | ❌ |
| `assistant_sync` | `PendingRemovals` | `src-tauri/src/cmd/vault/sync.rs:120`（`Option<…>`） | ❌ |
| `assistant_sync` | `Readyness` | `src-tauri/src/cmd/vault/sync.rs:255` | ❌ |
| `assistant_sync` | `ConflictFile` | `src-tauri/src/cmd/vault/sync.rs:261`（`Vec<…>`） | ❌ |
| `assistant_sync` | `HistoryCommit` | `src-tauri/src/cmd/vault/sync.rs:272`（`Vec<…>`） | ❌ |
| `assistant_sync` | `EntryPreview` | `src-tauri/src/cmd/vault/sync.rs:284` | ❌ |
| `assistant_sync` | `ConflictPreview` | `src-tauri/src/cmd/vault/sync.rs:327` | ❌ |
| `assistant_security` | `SshKeyInfo` | `src-tauri/src/cmd/vault/ssh.rs:80`（`Vec<…>`） | ❌ |
| `assistant_security` | `SshKeyMeta` | `src-tauri/src/cmd/vault/ssh.rs:155`、`:164` | ❌ |
| `assistant_vault` | `IntegrityResult` | `src-tauri/src/cmd/vault/recovery.rs:116` | ✅ 本次已收敛（路径仍经 re-export 兼容） |
| `assistant_crypto` | `TotpResult` | `src-tauri/src/cmd/vault/audit.rs:16` | ❌ |
| `assistant_crypto` | `ParsedOtpAuth` | `src-tauri/src/cmd/vault/audit.rs:103` | ❌ |

补充事实：

- 这 13 个类型的 serde 配置**当前都是正确的 camelCase**（`TotpResult` / `ParsedOtpAuth` 在 `assistant-crypto/src/vault_crypto/totp.rs:41,68` 已带 `rename_all = "camelCase"`），所以**契约本身没坏**，问题在归属而非格式。
- 前端侧全部有手写镜像（`src/vault/core/vault.types.ts`），且契约测试只校验 TS 自身字段、不校验 Rust 侧 → 手写漂移无任何保护。
- 主文档 §3.1.1 的 5 个类型表漏记 `PendingRemovals` / `ConflictFile` / `HistoryCommit` / `EntryPreview` / `ConflictPreview` / `SshKeyMeta` / `TotpResult` / `ParsedOtpAuth`，共 8 个。

---

## 5. `assistant-crypto` 是否应直接提供 DTO

### 5.1 结论：不应

三条理由：

1. **职责错位**。crypto 是加密原语叶 crate（`vault_crypto/{mod,types,totp}` + `api_key` + `VaultError`），职责是「算」；DTO 是序列化适配层，职责是「形」。主文档 §3.1.1 已明确「DTO 不是共享契约层，而是主 app 内部的序列化适配层」。
2. **内部类型与线格式被绑死**。`TotpResult` 现在既是计算结果又是 wire 契约，改内部字段名等于静默改前端契约，编译器不报错——与 [I16-protocol-type-misplacement-audit.md](./I16-protocol-type-misplacement-audit.md) 记录的「子 crate 改枚举变体名，前端不会编译报错」同类问题。
3. **主文档已排除同类**。§7.1.1 写明 crypto「不含 VaultEntry* DTO（属 vault 领域）」；TOTP 是同一性质（值是原语产物，形由调用方决定）。

### 5.2 但也不能硬塞进 `vault::dto`

`TotpResult` / `ParsedOtpAuth` **不含 Vault 领域语义**（无条目、分类、标签），塞进 `vault::dto` 会把「vault 领域 DTO」这个边界搞脏。正确落点是「**适配层定义 DTO，`From` 桥接留在调用侧**」。

### 5.3 三个可选方案

| 方案 | 做法 | 成本 | 评价 |
|---|---|---|---|
| **A（推荐）** | crypto 保持纯值类型；`TotpResultDto` / `ParsedOtpAuthDto` 放 `vault::dto`（vault 已依赖 crypto，不新增边）；`From` 实现留 vault；cmd 返回 DTO | ~40 行 + 2 处签名 | 与 §3.1.1 终局一致；将来搬 protocol 时 `From` 仍合法（孤儿规则） |
| B（最省） | 现状不动，仅在本文登记为「已知例外：crypto 自带 camelCase 线格式」 | 0 | 违反归属原则，但契约本身没坏（字段名已正确） |
| C | DTO 放主 app `protocol/dto`，crypto 只给值类型 | 中 | 终局最正，但 protocol 当前仍依赖 storage/providers/config，先做会加深主文档 §4.1 的混乱 |

### 5.4 附：`CipherAlgorithm` 的 serde 是空转，无需处理

`CipherAlgorithm` 带 `rename_all = "camelCase"`，但不出现在任何 IPC 返回类型里：

- 配置侧是 `String`（`crates/assistant-config/src/app_conf.rs:59-60` 的 `vault_cipher_algorithm`）
- vault 文件侧是 `String`（`crates/assistant-vault/src/vault_file/types.rs` 的 `VaultCryptoMeta.default_cipher`）

故该 serde 派生当前无消费者，**不属于本次收敛范围**，保留即可。

---

## 6. 已修复：`AuditLogEntry` 的 camelCase 缺失

**问题**：迁入 `dto/audit.rs` 前，`AuditLogEntry`（原 `audit/mod.rs`）只有 `#[derive(Serialize)]`，无 `rename_all`，序列化输出 `event_type`。

**前端契约**（`src/vault/core/vault.types.ts` 的 `AuditLogEntry`）要求：

```ts
export interface AuditLogEntry {
  id: string;
  eventType: string;
  detail: string | null;
  timestamp: number;
}
```

**后果**：审计日志页面的「事件类型」列恒为 `undefined`，且因 TS 手写镜像、契约测试不校验 Rust，该 bug 无任何自动发现机制。

**修复**：`dto/audit.rs` 补 `#[serde(rename_all = "camelCase")]`。Rust 侧字段名仍为 `event_type`，DB 列映射不受影响（`audit/mod.rs` 的 `query_map` 用 `row.get("event_type")` 字符串键，与结构体字段名解耦）。

**同类排查**：`StrengthResult` / `StrengthWarning` / `IntegrityResult` / `VaultEntryCreate` / `Update` / `Summary` / `Detail` / `RevealResult` 均已带 camelCase，无第二例。

---

## 7. 待办与优先级

| 优先级 | 事项 | 影响面 | 前置 |
|---|---|---|---|
| P1 | 方案 A：crypto 去 DTO 化，`vault::dto` 补 `TotpResultDto` / `ParsedOtpAuthDto` + `From` | 2 处 cmd 签名 + 前端类型不变 | 无 |
| P2 | `assistant_sync` 8 个直出类型建 DTO（`SyncResultDto` / `SyncConfigDto` / `PendingRemovalsDto` / `ReadinessDto` / `ConflictFileDto` / `HistoryCommitDto` / `EntryPreviewDto` / `ConflictPreviewDto`） | `src-tauri/src/cmd/vault/sync.rs` 13 处引用 + `recovery.rs` / `mod.rs` 各 1 处 `SyncConfig` | 需先确定 sync 的 DTO 归 vault 还是主 app `protocol/dto` |
| P2 | `assistant_security` 的 `SshKeyInfo` / `SshKeyMeta` 建 DTO | `src-tauri/src/cmd/vault/ssh.rs` 3 处 | 同上 |
| P3 | 引入 `ts-rs` / `typeshare` 从 Rust 生成 TS，替代手写镜像 | 全量 `src/**/*.types.ts` | 需先完成 P1/P2，否则生成的仍是半截契约 |
| P3 | 统一 DTO 搬迁至 `protocol` crate | 全量 dto | 主文档 §4.1 的 4 处类型引用须先断开 |

> P2 的关键分歧点：sync / security 的 DTO 放 vault 还是主 app。放 vault 会让 vault 承担 `SyncResult` 等领域语义（可接受，`sync_engine` 已在 vault）；放主 app `protocol/dto` 则更接近终局但会加深 protocol 的现有依赖问题。**建议随 P3 统一搬迁一次性决定**，避免两次搬运。

---

## 8. 对主文档的修正登记

| 主文档位置 | 原记录 | 实测修正 |
|---|---|---|
| §3.1.1「5 个类型直出前端」 | 5 个 | **13 个去重类型 / 14 处签名**，详见本文 §4 |
| §3.1.1 表格 | 未记 crypto | 补 `TotpResult` / `ParsedOtpAuth` |
| §7.1.1 vault 形态 | `types(VaultEntry* DTO)` | 已拆为 `dto/ + shared/ + types/`，详见本文 §1 |
| §6.1 归属判定 4 | `types/` 拆两半（VaultError → crypto，其余 → vault） | 再进一步：vault 侧三拆（`shared` / `dto` / `types`），且持久化格式不进 dto |
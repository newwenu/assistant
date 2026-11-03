# DTO 前后端同步审计（收口前基线）

> 审计日期：2026-10-30
> 复核日期：2026-11-02（按当前源码逐条复核，变更见 §1）
> 审计范围：`src-tauri/src/protocol/dto/**`、`src-tauri/crates/assistant-vault/src/dto/**`、`src/**` 中消费这些 DTO 的 service / store / 类型文件
> 审计方法：全量 `#[serde]` / `#[ts]` 派生检索 + `#[command]` 签名核对 + 前端类型逐字段对照 + 契约测试有效性检查 + 数值域推导
> 证据基准：**只采当前仓库源码**。`docs/achievement*/` 下的存档文档不作为依据——其中的分层/收口方案在代码中并未执行（例如 `ProviderInputDto.models` 至今仍以 JSON 文本上线，见 D7），引用它们会把「计划」误当成「现状」
> 结论性质：除标注「推算」外，均为源码实测

---

## 0. 结论摘要

1. **同步机制有两套，只有一套是真同步**。vault 域是 ts-rs 生成式（单一真源）；chat / settings / config / storage / orb / branch 域是「Rust 手写 DTO + TS 手写 interface」两份副本，靠测试兜底，而该测试是自证的（D6）。
2. **无现网可达的功能缺陷**。审计时唯一看似 P0 的 `MessageDto.tool_calls_json`，已在 I30 阶段一（提交 `6ce5dbf`）随 tool call 一并从线格式删除，D1 关闭；现网契约风险以 D2 为首。
3. **1 类系统性契约谎言（P1）**：`Option` 的缺省语义在 vault（`null`）与 chat（键缺失）两域相反，而前端按 `| null` 声明 → 运行时可能是 `undefined`（D2）。
4. **数值类型结论**：wire 层只能用 `number` 或 `string`；用 `number` 必须由值域证明背书，不可证明则改 `string`。`BigInt` 不能作为 DTO 类型，只允许在前端内部做精确运算（§4）。
5. **值域不能靠两端各写一个宽松类型来「对齐」**：生成器只同步形状，不同步值域；值域必须由**更窄的一端定义**，并由**值域测试**守住。
6. **根本病灶**：只要 chat / settings 仍是两份手写副本，D2 / D4 / D5 会反复回归；扩大生成式契约（阶段 4）不是「更规范」，而是唯一能让「形状」维度自动同步的手段。

---

## 1. 复核变更（2026-11-02）

| 项 | 审计时（2026-10-30） | 复核时（2026-11-02） | 处理 |
|---|---|---|---|
| D1 `MessageDto.tool_calls_json` | 线格式存在、前端不消费、不可达 | 已由 I30 阶段一从线格式与前端一并删除；DB 列保留为预留 | D1 改写为「已关闭」，仅留收尾动作 |
| `protocol/dto/**` 类型数 | 43 | **52** | 更新 §2.1、阶段 4 |
| vault 导出类型数 | 36 | **38**（新增 `RemoteStatusDto` / `RemoteStatusErrorKindDto`） | 更新 §2.1、§7 |
| `deny_unknown_fields` 处数 | 42 | **48**（分布在 7 个文件） | 更新 D4 |
| `security_db/crud.rs` | 单文件，行号 135 / 413 / 889-890 | 已拆分为 `security_db/entries/*` | 更新 D3 行号 |
| 前端 `vaultCipherAlgorithm` 联合 | `settings/core/types/confTypes.ts` | 迁至 `src/features/vault/conf.ts:7` | 更新 D5 |
| 前端工具调用代码 | `MessageBubble` / `chatStorePorts` 有 `ToolCallBlock`、`appendToolCallDelta` | 全仓 0 处 | D1 不再引用 |
| `SendChatMessageArgsDto` | 未单独检查 | **唯一缺 `deny_unknown_fields` 的输入 DTO** | 新增 D4 例外说明 |

行号漂移是后续功能迭代造成的，不影响结论；本文所有引用已按复核时的源码校正。

---

## 2. 同步机制现状

### 2.1 两套机制对照

| 域 | Rust 定义 | TS 定义 | 生成方式 | 同步强度 |
|---|---|---|---|---|
| vault | `src-tauri/crates/assistant-vault/src/dto/**`（38 个导出类型） | `src/vault/core/contract/vault.contract.ts` | ts-rs `#[ts(export)]`，`npm run contract:gen` | **形状强同步**（结构、可选性、枚举联合都由生成器保证）；值域无保证 |
| chat | `src-tauri/src/protocol/dto/chat.rs`（含 `MessageDto` / `ConversationDto` 等） | `src/chat/core/chatTypes.ts` | 手写两份 | 弱：仅靠测试断言键名 |
| branch | `src-tauri/src/protocol/dto/branch.rs` | `src/chat/core/chatTypes.ts`（`SiblingInfo` / `ChatMessagesChangedPayload`） | 手写两份 | 弱 |
| settings | `src-tauri/src/protocol/dto/settings.rs` | `src/settings/provider/types.ts`、`src/types/events.ts` | 手写两份 | 弱 |
| config | `src-tauri/src/protocol/dto/config.rs` | `src/settings/core/types/confTypes.ts`、`src/features/vault/conf.ts` | 手写两份 | 弱 |
| storage | `src-tauri/src/protocol/dto/storage.rs` | 手写（消费点分散） | 手写两份 | 弱 |
| orb | `src-tauri/src/protocol/dto/orb.rs` | 手写 | 手写两份 | 弱 |

`src-tauri/src/protocol/dto/**` 共 52 个 `pub struct` / `pub enum`，全部**没有** `TS` 派生；ts-rs 依赖只出现在 `src-tauri/crates/assistant-vault/Cargo.toml`。chat / branch 中与 tool call 相关的线格式字段已由 I30 阶段一删除，当前线格式不含任何 tool 字段（见 D1）。

### 2.2 三层传递链

```
SQLite 列类型 ──► Rust domain struct ──► Rust DTO ──► serde_json ──► WebView JSON.parse ──► TS interface
   (i64)              (i64/u64)          (i64/u64)      (精确整数)      (f64 双精度)        (number)
```

关键事实：**精度丢失发生在第 5 步**（`JSON.parse` 只能产出 f64），而不是 serde 序列化。因此「后端用 u64、前端用 number」这套约定，其安全性完全取决于业务值是否 ≤ 2^53-1，而这一点**当前没有任何一处代码或测试表达**。

### 2.3 已验证事实（避免后人重复踩坑）

| 事实 | 验证方式 | 结论 |
|---|---|---|
| serde 的 `Option<T>` 字段缺失时，**不加** `#[serde(default)]` 也能反序列化为 `None` | 阅读 serde 1.0.229 源码 `serde/src/private/de.rs:24-57`：`MissingFieldDeserializer` 的 `deserialize_option` 返回 `visit_none()` | vault 的 `#[ts(optional = nullable)]` + `Option<T>`（无 `default`）两端一致，**不要误加 `default`** |
| ts-rs 把 `i64` / `u64` 一律映射为 `number` | `src/vault/core/contract/vault.contract.ts` 全量字段对照 | 生成器不表达值域 |
| 前端 `invoke<T>` 是纯泛型透传，无运行时校验 | `src/services/tauriInvoke.ts:5-9` | TS 类型是对后端 JSON 的**无检查断言** |
| chat / branch 线格式已无 tool call 字段 | 全仓检索 `tool_calls_json` / `toolCalls`：DTO 与前端命中 0 处，仅 storage 行形状保留 | I30 阶段一（`6ce5dbf`）剥离，见 D1 |
| `MessageDto` 的 `Option` 字段仍大量 `skip_serializing_if` | `protocol/dto/chat.rs:85-105`（`tokens` 在 `:91`） | 输出 DTO 键会缺失，见 D2 |

---

## 3. 缺陷清单

> 编号 `D1..D8`，P0 = 现网可达的功能缺陷，P1 = 契约谎言（类型与实际不符），P2 = 一致性/可维护性，P3 = 风格。**本次 P0 为空**；D1 已关闭，章节保留作为决策记录。所有「怎么办」统一收敛到 §5，本节只陈述事实与后果。

### D1（已关闭）`tool_calls_json` 已由 I30 从线格式剥离

**定性**：审计时这是「契约提前落地、运行时尚未接通」的预留字段。提交 `6ce5dbf`（I30 阶段一）已把 tool call 从 chat 构建路径整体删除，当前**线格式与前端类型中都不存在 tool call 字段**，风险性质从「将来引爆的雷」变为「待 agentloop Step 7 重新定契约」。

**现状（源码实测）**

| 层 | 现状 |
|---|---|
| 线格式 | `MessageDto` 已无 `tool_calls_json`（`protocol/dto/chat.rs:85-105`）；`ChatDeltaPayloadDto` 已无 `delta_tool_call_*`（`protocol/dto/chat.rs:373-392`） |
| 前端 | `Message` 无 `toolCalls`（`src/chat/core/chatTypes.ts:3-16`）；`ToolCallBlock` / `appendToolCallDelta` 全仓 0 处 |
| 落库写入 | `update_message_content` 已去掉该参数（`assistant-storage/src/conversations/msg_ops.rs:44-55`）；`cmd/chat/insert.rs` 两处 `tool_calls_json: None`（插入即 NULL） |
| DB 列 | `messages.tool_calls_json TEXT` 保留（`assistant-storage/src/db.rs:55`），属落盘格式，作为 agentloop 的预留列 |

**原审计结论（存档，仍成立）**：可达性链条在第一跳即断——provider 请求体不声明 `tools`（`openai/types.rs:5-15`、`gemini/request.rs:41`），`ToolRegistry` 只被 `os_tool_exec.rs` 使用（`src/setup.rs:93`），故该列恒 `NULL`。字段已被先行删除，因此不再是线格式问题。

**遗留风险**：`db.rs:55` 建表语句旁没有「预留：agentloop 落库用」注释（SQLite 无列注释，只能写在代码里），后人可能把该列当废弃列 `DROP`。处置见 §5 阶段 1。

---

### D2（P1）`Option` 缺省语义两域相反，前端类型说谎

**现象**

| 域 | 约定 | 例 |
|---|---|---|
| vault | `Option<T>` **不** skip → 键存在，值为 `null` | `VaultEntryDetail.username: string \| null`（`src/vault/core/contract/vault.contract.ts:69`） |
| chat / branch / settings | 大量 `skip_serializing_if = "Option::is_none"` → 键**缺失** | `MessageDto.tokens`（`protocol/dto/chat.rs:90-91`）、`active_branch_id`（`protocol/dto/branch.rs:30`）、`retry_after_ms`（`protocol/dto/chat.rs:346-347`） |

前端对应声明却是「必填可空」：

- `Message.tokens: number | null`（`src/chat/core/chatTypes.ts:9`）
- `Conversation.activeBranchId: string | null`（`src/chat/core/chatTypes.ts:33`）
- `ChatMessagesChangedPayload.activeBranchId: string | null`（`src/chat/core/chatTypes.ts:188`）
- `ChatError.retryAfterMs: number | null`（`src/chat/core/chatErrors.ts:34-44`）

于是类型说「一定是 number 或 null」，运行时却可能是 `undefined`。当前只是靠松散判断侥幸没出错（`src/history/detail/HistoryMessageItem.tsx:110` 用了 `!= null`，同时覆盖 null 与 undefined）。

**附带发现**：同一个 `Message` interface 内部风格也不统一——`tokens` 是「必填 + 可空」，而 `parentId?: string | null` 是「可选 + 可空」（`src/chat/core/chatTypes.ts:3-16`）。这说明前端从未真正确定过「缺字段」代表什么。

**后果**：`Object.keys` 级断言、`=== null` 判断、`JSON.stringify` 去重都会在两域得到不同行为；新增字段时只能靠记忆沿用所在文件的局部惯例。

**处置**：统一为「输出 DTO 一律不 skip、`Option` 输出 `null`、前端 `T | null` 必填」，`skip_serializing_if` 只保留在输入 DTO。见 §5 阶段 2、§6 第 3 条。

---

### D3（P1）数值域单侧放宽

**现象**：Rust 侧用语义上更宽的类型，TS 侧照抄成 `number`，中间没有任何值域表达或校验。

| 字段 | Rust 类型 | 实际值域 | 风险 |
|---|---|---|---|
| `StrengthResult.crack_time_seconds` | `u64`（`assistant-vault/src/dto/audit.rs:10`） | 无上界（zxcvbn 离线慢哈希估算可到 1e17 秒量级，**推算**） | **可超 2^53**；前端只消费 `crackTimeDisplay`（`src/vault/unlock/PasswordForm.tsx:106`、`src/vault/settings/ChangePasswordTab.tsx:167`），`crackTimeSeconds` 仍 0 处消费 |
| `VaultEntryDetail.revision` | `u64`（`assistant-vault/src/dto/mod.rs:100`） | 单调递增小整数 | 三层域不一致：SQLite `INTEGER`(i64，`security_db/schema.rs:23`) → `i64 as u64` → DTO `u64` → TS `number` |
| `EntryPreviewDto.revision` | `u64`（`assistant-vault/src/dto/sync.rs:152`） | 同上 | 同上 |
| `StorageBreakdownDto.total_bytes / total_count` | `u64`（`protocol/dto/storage.rs:25-26`） | 受附件目录配额约束（256MB 级） | 实际安全，但类型不表达上限 |
| `AttachmentDeleteResultDto.removed_count / removed_bytes` | `u64`（`protocol/dto/storage.rs:34-35`） | 同上 | 同上 |
| `RetryNotificationDto.delay_ms` | `u64`（`protocol/dto/chat.rs:323`） | 秒级 | 实际安全 |
| 时间戳 `created_at / updated_at / timestamp / expiresAt / commitTime` | `i64` 毫秒 | 2^53 ms ≈ 公元 287396 年 | 当前安全；**仅靠文档约定单位**，改 µs 剩 285 年，改 ns 直接爆 |
| `DisasterRecoveryResultDto.entry_count` | `u32`（`assistant-vault/src/dto/recovery.rs:9`） | 有界 | 正确示范 |
| `max_tokens` / `presentation_rank` / `context_window` / `width` / `height` | `u32` | 有界 | 正确示范 |
| `compact_threshold_percent` / `opacity_percent` / `score` | `u8` | 有界 | 正确示范 |
| `compact_summary_max_tokens` / `scale_percent` / `vault_auto_lock_minutes` | `u16` | 有界 | 正确示范 |

**Rust 侧镜像放宽**：`i64 ↔ u64` 全部用裸 `as`，越界时**静默回绕**。审计时实测 30 处，其中包含往返转换；`security_db/crud.rs` 现已拆分为 `security_db/entries/*`，对应位置如下：

- `assistant-vault/src/security_db/entries/query.rs:146` —— `row.get::<_, i64>("revision")? as u64`
- `assistant-vault/src/security_db/entries/import.rs:63` —— `max_revision.max(entry.revision as i64) as u64 + 1`（u64 → i64 → u64）
- `assistant-vault/src/security_db/entries/import.rs:211-212` —— `entry.revision < revision as u64`

**后果**：值域只在开发者脑内；一旦某字段的真实值超界，前端静默得到错误数字（而非报错），后端也不会 panic，问题会在很久以后以「数据对不上」的形式出现。

**处置**：见 §5 阶段 3。

---

### D4（P2）`deny_unknown_fields` 两域不一致

- `src-tauri/src/protocol/dto/**`：**48 处**（`chat.rs` 23 / `settings.rs` 10 / `storage.rs` 4 / `config.rs` 4 / `branch.rs` 3 / `orb.rs` 3 / `os_tool.rs` 1）。
- `src-tauri/crates/assistant-vault/src/dto/**`：**0 处**。

**例外**：`protocol` 侧并非「全部严格」——`SendChatMessageArgsDto`（`protocol/dto/chat.rs:164-175`）是纯输入 DTO，只有 `rename_all = "camelCase"`，没有 `deny_unknown_fields`。这是全目录唯一一个不带该属性的 `#[derive(Deserialize)]` 输入 DTO。

**后果**：前端多传一个字段时，chat / settings 命令直接反序列化失败（用户看到错误），vault 命令静默忽略（同样的问题表现为「改了没生效」）。同一类错误在两域得到两种用户体验，且 vault 侧拼错字段名不会报错。

**处置**：见 §5 阶段 2。

---

### D5（P2）前端窄化、后端宽化（反向的单侧放宽）

| 字段 | Rust | TS | 后果 |
|---|---|---|---|
| `VaultConfDto.vault_cipher_algorithm` | `String`（`protocol/dto/config.rs:43`） | `"aes256Gcm" \| "chacha20Poly1305"`（`src/features/vault/conf.ts:7`） | 后端若给出第三种值，TS 静默把它当联合成员 |
| `ChatMessagesChangedPayloadDto.change_kind` | `String`（`protocol/dto/branch.rs:27`） | `MessageChangeKind` 联合（`src/chat/core/chatTypes.ts:176`） | 同上；前端 `switch` 无 `default` 时会走空分支 |

这与 D3 是同一问题的两个方向：**谁更宽谁就吞掉错误**。类型窄的一侧必须要么有运行时校验，要么由宽的一侧（后端）用 enum 保证。当前两者都没有。

**处置**：后端改用 `enum`（Rust 强类型 + serde 生成联合），前端联合随之自动正确；手写域至少加 `assertNever` 兜底。见 §5 阶段 2。

---

### D6（P2）契约测试是自证的，不构成同步保证

- 前端：`src/chat/core/__tests__/protocolContract.test.ts:54-220` 构造一个字面量、声明为 `Message`，然后断言它有自己刚写进去的键 —— **恒真**，与后端无关。
- 后端：`src-tauri/tests/protocol_contract.rs:139-186` 只断言 `contains_key("createdAt")` 与「无 snake_case」；`chat_delta_payload_dto_round_trip`（`:187`）只验证字段名往返。
- 缺失：**真实后端 JSON → 前端类型**的单向校验；数值域边界用例；生成文件的 CI 一致性校验。

**后果**：后端把 `u32` 改成 `u64`、新增字段、改可空性、改单位，全部测试保持绿色。测试提供的是「我写的和我想的一致」，而不是「两端一致」。

**处置**：见 §5 阶段 5。

---

### D7（P2）同一概念双向形状不同

`Provider` 在线上有两种形状：

| 方向 | 后端 | 前端 | 序列化职责 |
|---|---|---|---|
| 输出 | `ProviderListDto.models: Vec<ModelConfigDto>`（`protocol/dto/settings.rs:131`） | `Provider.models: ModelConfig[]`（`src/settings/provider/types.ts:25`） | 结构化，正常 |
| 输入 | `ProviderInputDto.models: String`（JSON 文本，`protocol/dto/settings.rs:118`） | `ProviderInput.models: string`（`src/settings/provider/types.ts:37`） | 由 UI 层手写 `JSON.stringify(models)`（`src/settings/provider/ConnectDialog.tsx:213`） |

不是错误，但把「序列化」这件事交给了最不该负责的 UI 组件；且同名概念两种形状，容易在新增调用点漏掉 stringify。

**处置**：输入也改为结构化数组，由 serde 负责序列化；若因历史原因保留，至少在 `providerService.saveProvider` 集中做 stringify，禁止 UI 层直接拼。见 §5 阶段 2。

---

### D8（P3）命名与 barrel 别名

- vault 的生成文件保留 `*Dto` 后缀，barrel 再重命名为无后缀（`src/vault/core/vault.types.ts:20-45`，如 `ConflictFileDto as ConflictFile`）。
- chat 域 Rust 有 `Dto` 后缀，前端却从未出现 `Dto`。
- 结果：两域的心智模型不同，新人无法从名字判断「这个类型是否来自后端」。

**处置**：要么全仓保留 `Dto` 后缀（生成式域不动 barrel），要么全仓在 barrel 统一去后缀并注明「`*Dto` 为线格式、无后缀为领域类型」。当前 vault 已有明确注释（`src/vault/core/vault.types.ts:1-6`），把同样注释复制到手写域即可。见 §5 阶段 4。

---

## 4. 数值类型决策：`BigInt` vs `number`

### 4.1 为什么 `BigInt` 不能作为 DTO 类型

| 约束 | 事实 |
|---|---|
| 序列化 | `JSON.stringify(1n)` 直接抛 `TypeError`；Tauri v2 命令的返回值与参数都经 `serde_json` → JSON 文本 |
| 反序列化 | `JSON.parse` **永远**只产出 `number`，不会产出 `BigInt` |
| 生态 | `1n + 1` 抛错、`Math.max` 不接受、`1n === 1` 为 `false`、`InputNumber` / 图表库需改写 |
| 性能 | 堆分配，比 `number` 慢一个量级；而本项目 99% 的字段是计数、字节数、毫秒时间戳 |

因此**能进 wire 的只有 `number` 或 `string`**。`BigInt` 只允许出现在前端内部（例如位运算），且必须在解析 `string` 之后。

项目内唯一正确用例：`src/tools/ip-info/ip-info.service.ts` —— 纯前端内部做无符号位运算，不进 IPC，属正例。

### 4.2 判定规则（新增/修改字段时逐条执行）

> 问：这个字段的**所有写入路径**，能否论证 `|v| ≤ 2^53-1`？

| 答案 | 采用 | 落地写法 |
|---|---|---|
| 能，且无位运算需求 | `number` | Rust 用能表达上限的窄类型（`u8/u16/u32/i32`），或 `i64` + 构造处断言 |
| 能，但类型必须宽（如 `i64` 时间戳） | `number` | 类型别名焊单位 + `debug_assert` 值域 |
| **不能** | `string` | Rust `#[serde(with = "u64_as_string")]` + `#[ts(type = "string")]`；前端按需 `BigInt(str)` |
| 需要精确单调比较（revision） | **收窄**而不是升 BigInt | 统一 `i64`，与 SQLite 列一致 |

安全边界数值：`2^53-1 = 9_007_199_254_740_991`（约 9.0e15）。

### 4.3 安全性排序

```
string on wire + 按需 BigInt  >  number + 已证明值域  >  number + 未证明值域  >  BigInt on wire（不可实现）
```

---

## 5. 收口方案（分阶段）

### 阶段 0：基线（不改代码）— 已完成

- [x] 完成本审计，缺陷编号与证据固化
- [x] 确认 D1 当前不可达（chat 请求不带 `tools`，`tool_calls_json` 恒 `NULL`）
- [x] 2026-11-02 复核：D1 已由 I30 阶段一删除，其余结论复验成立

### 阶段 1：D1 收尾（I30 之后，agent 运行时设计之前）

> 定位：字段已删除，此阶段只负责「别让后人误删预留列」和「重新引入时别再犯错」。

- [ ] `assistant-storage/src/db.rs:55` 建表语句补注释：该列由 agentloop 写入，**禁止 `DROP`**
- [ ] agentloop Step 7 设计评审时把线格式定死：结构化 `tool_calls: Vec<ToolCallDto>`，禁止把 JSON 文本搬回 DTO
- [ ] 前端恢复 `Message.toolCalls` 时与后端同名同形状，不留映射代码
- [ ] `src-tauri/tests/protocol_contract.rs` 增加结构断言（不是仅键名）

### 阶段 2：统一契约语义（P1 / P2）

- [ ] 输出 DTO 移除全部 `skip_serializing_if`（vault 已是 `null` 语义，chat / branch / settings 向其对齐）
- [ ] 前端 `tokens` / `activeBranchId` / `retryAfterMs` 等改为 `T | null` 且**必填**
- [ ] `deny_unknown_fields` 补到 vault 输入 DTO（`VaultEntryCreate` / `VaultEntryUpdate` / `CustomFieldInput` / `SyncConfigDto`）；输出 DTO 可选择性补
- [ ] `SendChatMessageArgsDto` 补 `deny_unknown_fields`（D4 例外）
- [ ] `vault_cipher_algorithm` / `change_kind` 后端改 `enum`；手写域加 `assertNever`
- [ ] `ProviderInputDto.models` 改结构化数组，或把 `JSON.stringify` 收进 `providerService.saveProvider`
- [ ] 规范条目写入 `docs/store/` 与 `.trae/rules/project_rules.md`（类型安全一节）

### 阶段 3：数值域（P1）

- [ ] 删除或 string 化 `crack_time_seconds`（前端已只用 `crackTimeDisplay`）
- [ ] `revision` 全链路统一 `i64`；`as` 改 `try_from`
- [ ] 计数 / 字节类 `u64` 收窄 `u32`
- [ ] 时间戳加单位别名 + `debug_assert` 值域

### 阶段 4：扩大生成式契约（P2）

- [ ] 给 `src-tauri/src/protocol/dto/**` 加 `ts-rs`（当前 52 个类型），生成 `src/types/protocol.contract.ts`
- [ ] `chatTypes.ts` / `confTypes.ts` / `types/events.ts` 改为从生成文件 re-export
- [ ] `npm run contract:gen` 覆盖全域；CI 加 `git diff --exit-code` 校验
- [ ] 统一 `Dto` 后缀策略，并在手写域 barrel 补「线格式 / 领域类型」注释（D8）

> 这是本审计最根本的一条：**只要 chat / settings 仍是两份手写副本，D2 / D4 / D5 会反复回归**。生成式契约不是「更规范」，而是唯一能让「形状」这一维度自动保持同步的手段。

### 阶段 5：测试补强（P2）

- [ ] 后端导出 JSON fixture，前端消费
- [ ] 数值边界用例（`2^53-1` / `2^53` / `u64::MAX`）
- [ ] 删除或重写自证式断言（`src/chat/core/__tests__/protocolContract.test.ts:54-220`）

---

## 6. 新增 / 修改 DTO 的检查清单

1. **归属**：字段名会出现在 `.vault` / CSV / DB 列里？→ 不进 DTO。历史反例 `MessageDto.tool_calls_json`（DTO 字段名与 SQLite 列名同形、值是列格式的 JSON 文本）已由 I30 删除；同类现存活反例见 D7（`ProviderInputDto.models`）。
2. **命名**：`#[serde(rename_all = "camelCase")]`；输入 DTO 加 `deny_unknown_fields`。
3. **可选性**：输出用 `T | null`（不 skip）；输入用 `?: T | null`（`#[ts(optional = nullable)]`，无需 `default`）。
4. **数值**：按 §4.2 判定；`u64` 必须能说明「为什么不是 `u32`」。
5. **单位**：时间字段名带单位或类型别名；禁止同一字段在不同域混用秒 / 毫秒。
6. **枚举**：能在 Rust 用 `enum` 就不要用 `String`（否则前端联合是凭空猜的）。
7. **生成**：vault 改完必须 `npm run contract:gen` 并提交生成文件。
8. **测试**：至少一条「后端真实 JSON → 前端类型」的断言，而不是自构造对象。

---

## 7. 未决 / 待实测

| # | 项 | 需要的动作 |
|---|---|---|
| 1 | agentloop Step 7 的 `tool_calls` 线格式 | 设计评审时定死为结构化 `Vec<ToolCallDto>`（见阶段 1） |
| 2 | `crack_time_seconds` 的实际最大值 | 用 zxcvbn 对 20 位随机串取 `guesses_log10`，换算秒数确认是否 > 2^53 |
| 3 | 是否还有其它前端未消费的 DTO 字段 | 对 `vault.contract.ts` 的 38 个类型做一次「字段级消费率」统计（本次只查了 `crackTimeSeconds`、`toolCallsJson`） |
| 4 | `ToolCall` 是否需要保留原始 JSON 字符串 | 若前端有「展示原始参数」需求，`ToolCallDto.arguments` 保持 `String` 即可，不必二次解析 |
| 5 | ts-rs 全域化的成本 | 估算 `protocol` crate 加 `ts-rs` 后生成的类型数量（当前 52 个）与 barrel 重写工作量 |
| 6 | `SendChatMessageArgsDto` 是否该严格 | 该 DTO 走 `#[tauri::command]` 参数反序列化，补 `deny_unknown_fields` 会不会影响前端既有调用 |
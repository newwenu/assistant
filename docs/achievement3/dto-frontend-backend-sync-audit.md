# DTO 前后端同步审计（收口前基线）

> 审计日期：2026-10-30
> 复核日期：2026-11-02
> 修订日期：2026-11-03（基线提交 `527e4dc`）
> 审计范围：`src-tauri/src/protocol/dto/**`、`src-tauri/crates/assistant-vault/src/dto/**`、`src/**` 中消费这些 DTO 的 service / store / 类型文件
> 审计方法：全量 `#[serde]` / `#[ts]` 派生检索 + `#[command]` 签名核对 + 前端类型逐字段对照 + 契约测试有效性检查 + 数值域推导
> 证据基准：**只采当前仓库源码**。`docs/achievement*/` 下的存档文档只用于追溯设计意图，不作为现状依据——其中的分层/收口方案在代码中并未执行（例如 `ProviderInputDto.models` 至今仍以 JSON 文本上线，见 D7），引用它们会把「计划」误当成「现状」
> 引用口径：行号以基线提交 `527e4dc` 为准。行号会随迭代漂移，定位时优先使用「类型名::字段名」锚点
> 结论性质：除标注「推算」外，均为源码实测

---

## 0. 结论摘要

1. **同步机制有两套，只有一套是真同步**。vault 域是 ts-rs 生成式（单一真源）；chat / settings / config / storage / orb / branch 域是「Rust 手写 DTO + TS 手写 interface」两份副本，靠测试兜底，而该测试是自证的（D6）。
2. **无现网可达的 UI 功能缺陷，但存在 1 处静默失效**：`chat-messages-changed` 的 `_seq` 防乱序守卫因两端键名不一致从未生效（D9）。它不报错、不破坏类型检查，属于最需要优先修的一类。
3. **契约风险与类型谎言**：`Option` 的缺省语义在 vault（`null`）与 chat（键缺失）两域相反，而前端按 `| null` 声明 → 运行时可能是 `undefined`（D2）；此外还有两个此前未审计的维度——错误通道结构不对称（D10）、wire 类型与前端 view model 混用（D11）。
4. **数值类型结论**：wire 层只能用 `number` 或 `string`；用 `number` 必须由值域证明背书，不可证明则改 `string`。`BigInt` 不能作为 DTO 类型，只允许在前端内部做精确运算（§4）。
5. **值域不能靠两端各写一个宽松类型来「对齐」**：生成器只同步形状，不同步值域；值域必须由**更窄的一端定义**，并由**值域测试**守住。
6. **根本病灶**：只要 chat / settings 仍是两份手写副本，D2 / D4 / D5 会反复回归；扩大生成式契约是唯一能让「形状」维度自动同步的手段。
7. **实施顺序已重排**：生成式先行（阶段 3），语义统一与数值域在其后（阶段 4 / 5）。按旧顺序先手改 60+ 处 `skip_serializing_if`、再上生成器，等于把同一批字段改两遍。

---

## 1. 复核变更

### 1.1 2026-11-02 复核

| 项 | 审计时（2026-10-30） | 复核时（2026-11-02） | 处理 |
|---|---|---|---|
| D1 `MessageDto.tool_calls_json` | 线格式存在、前端不消费、不可达 | 已由 I30 阶段一从线格式与前端一并删除；DB 列保留为预留 | D1 改写为「已关闭」，仅留收尾动作 |
| `protocol/dto/**` 类型数 | 43 | **52** | 更新 §2.1、阶段 3 |
| vault 导出类型数 | 36 | 生成物 39 / barrel 38（详见 §1.2） | 更新 §2.1、D8 |
| `deny_unknown_fields` 处数 | 42 | **48**（分布在 7 个文件） | 更新 D4 |
| `security_db/crud.rs` | 单文件，行号 135 / 413 / 889-890 | 已拆分为 `security_db/entries/*` | 更新 D3 |
| 前端 `vaultCipherAlgorithm` 联合 | `settings/core/types/confTypes.ts` | 迁至 `src/features/vault/conf.ts:7` | 更新 D5 |
| 前端工具调用代码 | `MessageBubble` / `chatStorePorts` 有 `ToolCallBlock`、`appendToolCallDelta` | 全仓 0 处 | D1 不再引用 |
| `SendChatMessageArgsDto` | 未单独检查 | 全目录唯一缺 `deny_unknown_fields` 的输入 DTO | 见 D4 例外说明 |

### 1.2 2026-11-03 修订（基线 `527e4dc`）

| 项 | 修订内容 | 处理 |
|---|---|---|
| vault 类型计数 | 生成物 `vault.contract.ts` 实为 **39** 个导出；`vault.types.ts` barrel 手工列了 **38** 个，漏掉 `StrengthWarning` | §2.1 写明两个口径；D8 增加 barrel 漂移 |
| `_seq` 防乱序 | 后端 `_seq` 序列化为 `seq`、命令参数也按 `seq` 取；前端仍用 `_seq`，守卫从未生效 | 新增 D9（P1，静默失效）；阶段 2 修复 |
| 错误通道 | chat 命令 `Result<_, String>`，vault 命令 `Result<_, VaultErrorDto>`；`MaxSiblingsExceededErrorDto` 无任何使用 | 新增 §2.4、D10 |
| wire / view model | `Message.error` 等字段由前端本地写入，不在 DTO；生成式落地前必须拆分 | 新增 §2.5、D11 |
| D2 例子校正 | `ChatError.retryAfterMs` 经 `toChatError` 归一化后恒有值，不是活的谎言；真正的违例是 `Message.tokens` / `activeBranchId` | 更新 D2 |
| D3 存储上限 | 未找到「附件目录 256MB 配额」常量；只有单文件 8MB / 16MB 上限（`assistant-agent/src/multimodal.rs:18-19`） | 更新 D3 |
| `crack_time_seconds` | 前端仅生成类型引用、0 处消费（实测） | §7 第 2 项关闭，阶段 5 直接删除 |
| `SendChatMessageArgsDto` | 仓库已有测试明确断言「未知可选键被忽略」（`protocol/dto/mod.rs:419-426`），补 `deny_unknown_fields` 不是纯增量 | 更新 D4、§7 第 6 项 |
| 生成式漂移检查 | 仓库无 CI（无 `.github/workflows`），且 I23 已定案「不设漂移检查脚本」 | 阶段 3 改为本地 `contract:check` |
| 裸 `as` 统计 | `assistant-vault/src` 现有 **25 处** `as u64` / `as i64`，分布在 5 个文件 | 更新 D3、阶段 5 |

行号漂移是后续功能迭代造成的，不影响结论；本文所有引用已按基线源码校正。

---

## 2. 同步机制现状

### 2.1 两套机制对照

| 域 | Rust 定义 | TS 定义 | 生成方式 | 同步强度 |
|---|---|---|---|---|
| vault | `src-tauri/crates/assistant-vault/src/dto/**`（生成物 39 个类型；barrel 暴露 38 个） | `src/vault/core/contract/vault.contract.ts` | ts-rs `#[ts(export)]`，`npm run contract:gen` | **形状强同步**（结构、可选性、枚举联合都由生成器保证）；值域无保证；barrel 仍有手工漂移 |
| chat | `src-tauri/src/protocol/dto/chat.rs`（含 `MessageDto` / `ConversationDto` 等） | `src/chat/core/chatTypes.ts` | 手写两份 | 弱：仅靠测试断言键名 |
| branch | `src-tauri/src/protocol/dto/branch.rs` | `src/chat/core/chatTypes.ts`（`SiblingInfo` / `ChatMessagesChangedPayload`） | 手写两份 | 弱 |
| settings | `src-tauri/src/protocol/dto/settings.rs` | `src/settings/provider/types.ts`、`src/types/events.ts` | 手写两份 | 弱 |
| config | `src-tauri/src/protocol/dto/config.rs` | `src/settings/core/types/confTypes.ts`、`src/features/vault/conf.ts` | 手写两份 | 弱 |
| storage | `src-tauri/src/protocol/dto/storage.rs` | 手写（消费点分散） | 手写两份 | 弱 |
| orb | `src-tauri/src/protocol/dto/orb.rs` | 手写 | 手写两份 | 弱 |

`src-tauri/src/protocol/dto/**` 共 52 个 `pub struct` / `pub enum`，全部**没有** `TS` 派生；ts-rs 依赖只出现在 `src-tauri/crates/assistant-vault/Cargo.toml:33`。chat / branch 中与 tool call 相关的线格式字段已由 I30 阶段一删除，当前线格式不含任何 tool 字段（见 D1）。

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
| `MessageDto` 的 `Option` 字段仍大量 `skip_serializing_if` | `protocol/dto/chat.rs:83-106`（`tokens` 在 `:90-91`） | 输出 DTO 键会缺失，见 D2 |
| serde `rename_all = "camelCase"` 会丢弃字段名**前导下划线**：`_seq` → `seq` | 阅读 `serde_derive-1.0.229/src/internals/case.rs:82-104`：`PascalCase` 把 `_` 当分隔符，`CamelCase` 再小写首字母 | D9 的键名不一致由此产生 |
| Tauri 命令参数同样做 `to_lower_camel_case`，且 `Option` 缺键返回 `None` | `tauri-macros-2.7.4/src/command/wrapper.rs:499-505`；`tauri-2.12.5/src/ipc/command.rs:146-156` 的 `deserialize_option` → `visit_none()` | 前端传 `_seq` 不会绑定到 Rust 的 `_seq`，见 D9 |
| `crack_time_seconds` 前端 0 处消费 | 全仓检索 `crackTimeSeconds`：仅命中生成文件 | 可直接删除，见 D3 |
| `SendChatMessageArgsDto` 的宽松行为已被测试固化 | `protocol/dto/mod.rs:419-426` 断言未知可选键被忽略 | 补 `deny_unknown_fields` 需先改语义与测试，见 D4 |

### 2.4 错误通道（此前未审计）

| 域 | 命令返回 | 前端消费方式 |
|---|---|---|
| vault | `Result<T, VaultErrorDto>`（如 `cmd/vault/audit.rs:49-54`） | `vaultErrorCode` / `vaultErrorMessage` 按 `code` 分支 |
| chat | `Result<T, String>`（如 `cmd/chat/mod.rs:30-35`） | `toChatError` 把 string / object 归一到 `ChatError`（`src/chat/core/chatErrors.ts:107-146`） |
| conversation / provider / storage / orb | `Result<T, String>` 为主 | 各调用点自行 `String(e)` 或 `notifyError` |

后果：同类失败在两域得到不同结构；chat 侧无法枚举错误码、无法稳定本地化、无法对错误形状写契约测试。`MaxSiblingsExceededErrorDto`（`protocol/dto/branch.rs:35-42`）定义后 Rust 侧 0 处构造/使用，是典型死 DTO。

### 2.5 wire 类型与前端 view model

后端 DTO 之外，前端在部分「wire 类型」上直接挂了本地状态：

| 前端类型 | 本地字段 / 来源 | 说明 |
|---|---|---|
| `Message` | `error?: ChatError \| null`，由 `chatStorePorts.ts:168-183` 写入 | 后端 `MessageDto` 无此字段 |
| `ContextUsage` / `ChatErrorCategory` / `RevealedField` | 纯前端派生类型 | 与生成类型的边界未声明 |

后果：生成式改造时不能直接把 `chatTypes.ts` 换成 re-export，必须先定义 `MessageWire` 与 `Message = MessageWire & { error?: ChatError }` 这类叠加类型（见 D11）。

---

## 3. 缺陷清单

> 编号 `D1..D11`，P0 = 现网可达的功能缺陷，P1 = 契约谎言 / 静默失效，P2 = 一致性、可维护性，P3 = 风格。**本次 P0 为空**；D1 已关闭，章节保留作为决策记录。所有「怎么办」统一收敛到 §5，本节只陈述事实与后果。

### D1（已关闭）`tool_calls_json` 已由 I30 从线格式剥离

**定性**：审计时这是「契约提前落地、运行时尚未接通」的预留字段。提交 `6ce5dbf`（I30 阶段一）已把 tool call 从 chat 构建路径整体删除，当前**线格式与前端类型中都不存在 tool call 字段**，风险性质从「将来引爆的雷」变为「待 agentloop Step 7 重新定契约」。

**现状（源码实测）**

| 层 | 现状 |
|---|---|
| 线格式 | `MessageDto` 已无 `tool_calls_json`（`protocol/dto/chat.rs:83-106`）；`ChatDeltaPayloadDto` 已无 `delta_tool_call_*`（`protocol/dto/chat.rs:371-390`） |
| 前端 | `Message` 无 `toolCalls`（`src/chat/core/chatTypes.ts:3-18`）；`ToolCallBlock` / `appendToolCallDelta` 全仓 0 处 |
| 落库写入 | `update_message_content` 已去掉该参数（`assistant-storage/src/conversations/msg_ops.rs:44-55`）；`cmd/chat/insert.rs` 两处 `tool_calls_json: None`（插入即 NULL） |
| DB 列 | `messages.tool_calls_json TEXT` 保留（`assistant-storage/src/db.rs:55`），属落盘格式，作为 agentloop 的预留列 |

**原审计结论（存档，仍成立）**：可达性链条在第一跳即断——provider 请求体不声明 `tools`（`openai/types.rs:5-15`、`gemini/request.rs:41`），`ToolRegistry` 只被 `os_tool_exec.rs` 使用（`src/setup.rs:93`），故该列恒 `NULL`。字段已被先行删除，因此不再是线格式问题。

**遗留风险**：`db.rs:55` 建表语句旁没有「预留：agentloop 落库用」注释（SQLite 无列注释，只能写在代码里），后人可能把该列当废弃列 `DROP`。处置见 §5 阶段 1。

---

### D2（P1）`Option` 缺省语义两域相反，前端类型说谎

**现象**

| 域 | 约定 | 例 |
|---|---|---|
| vault | `Option<T>` **不** skip → 键存在，值为 `null` | `VaultEntryDetail.username: string \| null`（`src/vault/core/contract/vault.contract.ts:71`） |
| chat / branch / settings | 大量 `skip_serializing_if = "Option::is_none"` → 键**缺失** | `MessageDto.tokens`（`protocol/dto/chat.rs:90-91`）、`active_branch_id`（`protocol/dto/branch.rs:29-30`）、`retry_after_ms`（`protocol/dto/chat.rs:346-347`） |

前端对应声明却是「必填可空」：

- `Message.tokens: number | null`（`src/chat/core/chatTypes.ts:9`）
- `Conversation.activeBranchId: string | null`（`src/chat/core/chatTypes.ts:33`）
- `ChatMessagesChangedPayload.activeBranchId: string | null`（`src/chat/core/chatTypes.ts:184-190`）

于是类型说「一定是 number 或 null」，运行时却可能是 `undefined`。当前只是靠松散判断侥幸没出错（`src/history/detail/HistoryMessageItem.tsx:110` 用了 `!= null`，同时覆盖 null 与 undefined）。

**校正（2026-11-03）**：审计时把 `ChatError.retryAfterMs` 也列为违例，但它经 `toChatError` 的 `value.retryAfterMs ?? null` 归一化后恒有值（`src/chat/core/chatErrors.ts:107-125`），不是活的谎言。真正的违例集中在后端 `skip_serializing_if` 且前端声明为必填的字段：`Message.tokens`、`Conversation.activeBranchId`、`ChatMessagesChangedPayload.activeBranchId`，以及 `Message.siblingCount` 这类「可选 + 可空」与「可选」混用的风格问题。

**附带发现**：同一个 `Message` interface 内部风格也不统一——`tokens` 是「必填 + 可空」，而 `parentId?: string | null` 是「可选 + 可空」（`src/chat/core/chatTypes.ts:3-18`）。这说明前端从未真正确定过「缺字段」代表什么。

**后果**：`Object.keys` 级断言、`=== null` 判断、`JSON.stringify` 去重都会在两域得到不同行为；新增字段时只能靠记忆沿用所在文件的局部惯例。

**处置**：统一为「输出 DTO 一律不 skip、`Option` 输出 `null`、前端 `T | null` 必填」，`skip_serializing_if` 只保留在输入 DTO。见 §5 阶段 4、§6 第 3 条。

---

### D3（P1）数值域单侧放宽

**现象**：Rust 侧用语义上更宽的类型，TS 侧照抄成 `number`，中间没有任何值域表达或校验。

| 字段 | Rust 类型 | 实际值域 | 风险 |
|---|---|---|---|
| `StrengthResult.crack_time_seconds` | `u64`（`assistant-vault/src/dto/audit.rs:4-13`） | 无上界（zxcvbn 离线慢哈希估算可到 1e17 秒量级，**推算**） | **可超 2^53**；前端 0 处消费（`crackTimeDisplay` 之外无引用），**建议直接删除** |
| `VaultEntryDetail.revision` | `u64`（`assistant-vault/src/dto/mod.rs:103`） | 单调递增小整数 | 三层域不一致：SQLite `INTEGER`(i64) → `i64 as u64` → DTO `u64` → TS `number` |
| `EntryPreviewDto.revision` | `u64`（`assistant-vault/src/dto/sync.rs:152`） | 同上 | 同上 |
| `StorageBreakdownDto.total_bytes / total_count` | `u64`（`protocol/dto/storage.rs:22-29`） | **未找到目录总量配额常量**；单文件上限为 8MB / 16MB（`assistant-agent/src/multimodal.rs:18-19`） | 类型不表达上限；不能再用「256MB 配额」作为安全论据 |
| `AttachmentDeleteResultDto.removed_count / removed_bytes` | `u64`（`protocol/dto/storage.rs:31-36`） | 同上 | 同上 |
| `RetryNotificationDto.delay_ms` | `u64`（`protocol/dto/chat.rs:318-327`） | 秒级 | 实际安全 |
| 时间戳 `created_at / updated_at / timestamp / expiresAt / commitTime` | `i64` 毫秒 | 2^53 ms ≈ 公元 287396 年 | 当前安全；**仅靠文档约定单位**，改 µs 剩 285 年，改 ns 直接爆 |
| `DisasterRecoveryResultDto.entry_count` | `u32`（`assistant-vault/src/dto/recovery.rs:9`） | 有界 | 正确示范 |
| `max_tokens` / `presentation_rank` / `context_window` / `width` / `height` | `u32` | 有界 | 正确示范 |
| `compact_threshold_percent` / `opacity_percent` / `score` | `u8` | 有界 | 正确示范 |
| `compact_summary_max_tokens` / `scale_percent` / `vault_auto_lock_minutes` | `u16` | 有界 | 正确示范 |

**Rust 侧镜像放宽**：`i64 ↔ u64` 全部用裸 `as`，越界时**静默回绕**。`assistant-vault/src` 现有 **25 处** `as u64` / `as i64`，分布 5 个文件；主要位置：

- `assistant-vault/src/security_db/entries/snapshot.rs:86` —— `revision: revision as u64`
- `assistant-vault/src/security_db/entries/import.rs:34` —— `(local as u64).max(entry.revision) + 1`
- `assistant-vault/src/security_db/entries/import.rs:63` —— `max_revision.max(entry.revision as i64) as u64 + 1`（u64 → i64 → u64）
- `assistant-vault/src/security_db/entries/import.rs:207-208` —— `entry.revision < revision as u64`
- `assistant-vault/src/security_db/entries/import.rs:139` / `:169` / `:230` / `:270` —— `revision as i64` 写库

**后果**：值域只在开发者脑内；一旦某字段的真实值超界，前端静默得到错误数字（而非报错），后端也不会 panic，问题会在很久以后以「数据对不上」的形式出现。

**处置**：见 §5 阶段 5。

---

### D4（P2）`deny_unknown_fields` 两域不一致，且方向未区分

- `src-tauri/src/protocol/dto/**`：**48 处**（`chat.rs` 23 / `settings.rs` 10 / `storage.rs` 4 / `config.rs` 4 / `branch.rs` 3 / `orb.rs` 3 / `os_tool.rs` 1）。
- `src-tauri/crates/assistant-vault/src/dto/**`：**0 处**。

**方向区分（2026-11-03 补充）**：

- **输入 DTO** 加 `deny_unknown_fields` 是对前端的校验契约：多传字段会立即失败，能暴露拼写错误。
- **纯输出 DTO** 加该属性只影响 Rust 侧反序列化（往返测试），前端永远不会触发它；它不构成「前后端同步」的一部分，不应计入契约一致性指标。
- 因此「48 处 vs 0 处」不是有效对比；有效指标应是「输入 DTO 是否严格」。

**例外**：`protocol` 侧并非「全部严格」——`SendChatMessageArgsDto`（`protocol/dto/chat.rs:164-173`）是纯输入 DTO，只有 `rename_all = "camelCase"`，没有 `deny_unknown_fields`。这是全目录唯一一个不带该属性的 `#[derive(Deserialize)]` 输入 DTO。但仓库已有测试明确锁定其宽松行为（`protocol/dto/mod.rs:419-426`：未知可选键被忽略），所以补属性需要同步改语义与测试，不是纯增量。

另：`ProviderErrorCodeDto` 这类纯 unit 变体枚举上的 `deny_unknown_fields` 没有实际作用（没有字段可拒绝），属于计数噪声。

**后果**：前端多传一个字段时，chat / settings 命令直接反序列化失败（用户看到错误），vault 命令静默忽略（同样的问题表现为「改了没生效」）。同一类错误在两域得到两种用户体验，且 vault 侧拼错字段名不会报错。

**处置**：见 §5 阶段 4。

---

### D5（P2）前端窄化、后端宽化（反向的单侧放宽）

| 字段 | Rust | TS | 后果 |
|---|---|---|---|
| `VaultConfDto.vault_cipher_algorithm` | `String`（`protocol/dto/config.rs:43`） | `"aes256Gcm" \| "chacha20Poly1305"`（`src/features/vault/conf.ts:7`） | 后端若给出第三种值，TS 静默把它当联合成员 |
| `ChatMessagesChangedPayloadDto.change_kind` | `String`（`protocol/dto/branch.rs:27`） | `MessageChangeKind` 联合（`src/chat/core/chatTypes.ts:176-182`） | 同上；前端 `switch` 无 `default` 时会走空分支 |
| `ConversationListChangedPayloadDto.change_kind` | `String`（`protocol/dto/chat.rs:432`） | `ConversationListChangeKind` 联合（`src/chat/core/chatTypes.ts:195-205`） | 同上 |
| `AgentModeDto.kind` / `AgentModeInputDto.kind` | `String`（`protocol/dto/settings.rs:11` / `:58`） | `"default" \| "quick" \| "deep" \| "custom"`（`src/settings/agentMode/types.ts:5`） | 封闭集合，后端改 `enum` 后前端联合自动正确 |
| `MessageDto.role` | `String`（`protocol/dto/chat.rs:88`） | `string` | 应用内只写 `user` / `assistant`（`cmd/chat/insert.rs:220/259`），但 DB 列为 TEXT；改 enum 前需核对存量数据 |
| `ConversationDto.title_source` | `String`（`protocol/dto/chat.rs:49`） | `string` | 应用内写入 `placeholder` / `user` / `copy`（`cmd/conversation.rs:52`、`:229`、`assistant-storage/src/conversations/ops.rs:10`）；同样是 DB TEXT，需兼容存量 |
| `ChatDeltaPayloadDto.finish_reason` | `String`（`protocol/dto/chat.rs:382-383`） | `string` | **开放集合**（由 provider 决定），前端只透传；需兜底而非枚举 |
| `ConversationQueryOptsDto.mode` | `String`（`protocol/dto/chat.rs:152-153`） | 无类型约束，调用点直接传字面量 `"paginated"` / `"all"`（`src/services/chatService.ts:33/41`） | 输入侧封闭集合；缺省 `""` 也会绕过校验 |
| `SettingProviderChangedPayloadDto.action` | `String`（`protocol/dto/settings.rs:138-139`） | `SettingAction` 枚举（`src/types/events.ts:15`） | 手写枚举与后端字符串无关联，新增值需人工同步 |

**归类规则（建议写入 §6）**：

- **封闭集合**（值集合由应用固定）：必须用 Rust `enum`，由 serde 产出联合，前端不再手写。
- **开放集合**（值随 provider / 插件扩展）：Rust 保持 `String`，前端不做窄化；若已窄化，必须加 `assertNever` 兜底与运行时校验，不得静默吞值。
- **存储边界例外**：值虽为封闭语义，但已以 TEXT 落库（`role` / `title_source`）时，改 enum 前必须确认存量数据只含预期值，否则先做数据迁移或保留 `String` + 校验函数。

开放集合示例：`ProviderListDto.type_`、`AgentModeDto.reasoning_effort`、`ChatDeltaPayloadDto.finish_reason`、`AppConfDto.theme / language`。

这与 D3 是同一问题的两个方向：**谁更宽谁就吞掉错误**。类型窄的一侧必须要么有运行时校验，要么由宽的一侧（后端）用 enum 保证。当前两者都没有。

**处置**：后端封闭集合改 `enum`；开放集合补 `assertNever` 与边界校验。见 §5 阶段 4。

---

### D6（P2）契约测试是自证的，不构成同步保证

- 前端：`src/chat/core/__tests__/protocolContract.test.ts:54-220` 构造一个字面量、声明为 `Message`，然后断言它有自己刚写进去的键 —— **恒真**，与后端无关。
- 后端：`src-tauri/tests/protocol_contract.rs` 只断言 `contains_key("createdAt")` 与「无 snake_case」；`chat_delta_payload_dto_round_trip` 只验证字段名往返。
- 缺失：**真实后端 JSON → 前端类型**的单向校验；数值域边界用例；生成文件的本地一致性校验。

**新增证据（D9）**：`_seq` 的键名不一致同时逃过了两端测试——`src-tauri/tests/protocol_contract.rs` 与 `src/chat/core/__tests__/protocolContract.test.ts` 均 **0 处**覆盖 `ChatMessagesChangedPayload`。这证明「两端各自写断言」不仅无法发现跨端键名不一致，连覆盖都没有。

**后果**：后端把 `u32` 改成 `u64`、新增字段、改可空性、改单位、改键名，全部测试保持绿色。测试提供的是「我写的和我想的一致」，而不是「两端一致」。

**处置**：见 §5 阶段 6。最小可行方案：后端测试输出真实 JSON fixture，前端测试 `JSON.parse` 后做键集合 / 类型断言 + `expectTypeOf`，并为 `_seq` 这类关键字段补精确键名断言。

---

### D7（P2）同一概念双向形状不同

`Provider` 在线上有两种形状：

| 方向 | 后端 | 前端 | 序列化职责 |
|---|---|---|---|
| 输出 | `ProviderListDto.models: Vec<ModelConfigDto>`（`protocol/dto/settings.rs:122-134`） | `Provider.models: ModelConfig[]`（`src/settings/provider/types.ts:25`） | 结构化，正常 |
| 输入 | `ProviderInputDto.models: String`（JSON 文本，`protocol/dto/settings.rs:107-120`） | `ProviderInput.models: string`（`src/settings/provider/types.ts:37`） | 由 UI 层手写 `JSON.stringify(models)`（`src/settings/provider/ConnectDialog.tsx:213`） |

不是错误，但把「序列化」这件事交给了最不该负责的 UI 组件；且同名概念两种形状，容易在新增调用点漏掉 stringify。

**补充（2026-11-03）**：改结构化输入不是单点改动——`cmd/provider.rs:64-75` 的 `save_provider` 直接把 `provider.models` 当 `models_json` 使用，并用 `provider.models.is_empty()` 判空。若要结构化，需在命令层完成 `Vec<ModelConfigDto> → serde_json::to_string`，并明确空数组语义（当前空字符串会被存成 `NULL`/空）。

**处置**：输入也改为结构化数组，由 serde 负责序列化；若因历史原因保留，至少在 `providerService.saveProvider` 集中做 stringify，禁止 UI 层直接拼。见 §5 阶段 4。

---

### D8（P3）命名、barrel 别名与生成物漂移

- vault 的生成文件保留 `*Dto` 后缀，barrel 再重命名为无后缀（`src/vault/core/vault.types.ts:23-48`，如 `ConflictFileDto as ConflictFile`）。
- chat 域 Rust 有 `Dto` 后缀，前端却从未出现 `Dto`。
- **新发现**：生成物 `vault.contract.ts` 有 **39** 个导出，barrel 手工列了 **38** 个，`StrengthWarning` 未被 re-export；后续新增类型会继续漏。
- 结果：两域的心智模型不同，新人无法从名字判断「这个类型是否来自后端」，且 generated → barrel 的覆盖不可自动验证。

**处置**：要么全仓保留 `Dto` 后缀（生成式域不动 barrel），要么全仓在 barrel 统一去后缀并注明「`*Dto` 为线格式、无后缀为领域类型」。barrel 改为 `export * from`（或脚本生成），不再手工枚举。见 §5 阶段 3、§6 第 7 条。

---

### D9（P1，静默失效）`_seq` 两端键名不一致，防乱序守卫从未生效

**现象链（源码实测）**

| 环节 | 事实 |
|---|---|
| Rust 字段 | `ChatMessagesChangedPayloadDto._seq: Option<u32>`，带 `rename_all = "camelCase"` + `skip_serializing_if`（`protocol/dto/branch.rs:31-32`） |
| serde 序列化 | `camelCase` 把前导 `_` 当分隔符丢弃 → 线格式键为 **`seq`**（`serde_derive/src/internals/case.rs:82-104`） |
| 命令参数 | `create_branch` / `switch_branch` 的参数名 `_seq` 经 `to_lower_camel_case` → **`seq`**（`tauri-macros/src/command/wrapper.rs:499-505`）；前端传 `_seq` 不绑定 |
| 缺键行为 | `Option` 参数缺键时 `deserialize_option` → `visit_none()`（`tauri/src/ipc/command.rs:146-156`）→ 后端恒 `None` |
| 前端类型 | `ChatMessagesChangedPayload._seq?: number`（`src/chat/core/chatTypes.ts:189`） |
| 前端调用 | `chatService.createBranch/switchBranch` 传 `_seq`（`src/services/chatService.ts:115-136`） |
| 前端消费 | 监听读 `payload._seq` 并维护 `lastProcessedSeq`（`src/services/chatLifecycleEvents.ts:134-148`） |
| 设计意图 | `docs/achievement1/I17b-conversation-branch-spec.md:1123-1150` 明确要求「后端回传 `_seq`，前端验证顺序」 |

**结论**：请求侧参数永远绑不到 `_seq`；响应侧键名是 `seq`，前端读 `_seq` 永远 `undefined`。因此 `lastProcessedSeq` 从不推进，乱序守卫形同虚设。

**为什么没被发现**：`_seq` 在两侧都是可选字段（`Option` / `?:`），类型检查通过；两端契约测试都未覆盖 `ChatMessagesChangedPayload`（D6），因此键名不一致没有任何断言兜底。

**处置**（阶段 2，成本低，优先修）

- 字段与命令参数统一改名 `seq`（`ChatMessagesChangedPayloadDto`、`create_branch`、`switch_branch`），前端类型 / 调用 / 监听同步改。
- 补测试：后端断言 `to_value(...)["seq"] == 1`；前端 fixture 测试断言 `payload.seq` 可读。
- 若不打算保留乱序保护（分支命令本身是串行 await 的），应连同前端 `lastProcessedSeq` 一起删除，而不是留一个永不生效的守卫。

---

### D10（P2）错误通道结构不对称，且存在死 DTO

- chat / conversation / provider / storage / orb 命令返回 `Result<T, String>`；vault 返回 `Result<T, VaultErrorDto>`（见 §2.4）。
- 前端 chat 侧用 `toChatError` 把 string / object 归一到 `ChatError`（`src/chat/core/chatErrors.ts:107-146`），vault 侧用 `vaultErrorCode` / `vaultErrorMessage`。两套错误模型互不复用。
- `MaxSiblingsExceededErrorDto`（`protocol/dto/branch.rs:35-42`）在 Rust 侧无任何构造/使用；前端也无对应消费。它是「设计过但没接线」的死类型。

**后果**：chat 域的错误无法枚举、无法稳定本地化、无法做类型化处理；新增错误码只能改字符串；死 DTO 会误导后人以为该错误契约已生效。

**处置**：阶段 2 定案 chat 错误是否改为 typed DTO（如 `ChatErrorDto`）；无论是否改，先删除或接线 `MaxSiblingsExceededErrorDto`，并把错误通道纳入 §6 检查清单。

---

### D11（P2）wire 类型与前端 view model 混用

- `Message.error?: ChatError | null` 由前端本地写入（`src/stores/chatStorePorts.ts:168-183`），后端 `MessageDto` 无此字段。
- `ContextUsage`、`ChatErrorCategory`、`RevealedField` 等纯前端类型与生成类型混在同一 barrel / types 文件。
- 前端类型同时承担「线格式镜像」和「UI 视图状态」两种职责。

**后果**：阶段 3 生成式落地时，不能直接把 `chatTypes.ts` / `vault.types.ts` 改成对生成文件的 re-export；必须先把本地字段剥离，否则会丢字段或产生交叉类型错误。这是生成式改造的**前置条件**，不是收尾工作。

**处置**：阶段 3 前置拆分：`MessageWire`（生成）+ `Message = MessageWire & { error?: ChatError }`（本地）；barrel 只做 re-export，本地类型单独文件。见 §5 阶段 3。

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
| 能，但类型必须宽（如 `i64` 时间戳） | `number` | 类型别名焊单位 + 构造处校验 |
| **不能** | `string` | Rust `#[serde(with = "u64_as_string")]` + `#[ts(type = "string")]`；前端按需 `BigInt(str)` |
| 需要精确单调比较（revision） | **收窄**而不是升 BigInt | 统一 `i64`，与 SQLite 列一致 |

安全边界数值：`2^53-1 = 9_007_199_254_740_991`（约 9.0e15）。

### 4.3 安全性排序

```
string on wire + 按需 BigInt  >  number + 已证明值域  >  number + 未证明值域  >  BigInt on wire（不可实现）
```

### 4.4 把规则变成可执行机制（2026-11-03 补充）

单纯「类型别名 + `debug_assert`」比文档约定强不了多少：别名不阻止赋值，`debug_assert` 在 release 下消失。建议至少选一项：

- **newtype**：`Revision(i64)` / `Millis(i64)`，只暴露 `TryFrom` 与具名构造函数，DTO 用 `#[serde(transparent)]`；`i64 ↔ u64` 的裸 `as` 全部改为 `try_from`。
- **clippy 门禁**：对 `assistant-vault` / `assistant-storage` 开启 `clippy::cast_sign_loss`、`clippy::cast_possible_truncation`，把现有 25 处裸 `as` 收敛到白名单并逐条给出值域说明。
- 时间字段名带单位（`_ms` / `_sec`），并在构造处断言范围。

---

## 5. 收口方案（分阶段）

> 实施顺序已重排：先修静默失效与低成本项，再做生成式契约，最后做语义统一与数值域。旧版把生成式放在阶段 4，会导致语义修改做两遍。
>
> **执行状态（2026-11-03）**：阶段 1、2、3、4 已完成并验证。协议契约 57 个类型已生成，前端 `chatTypes` / `provider` / `confTypes` / `events` 已切换到生成类型，vault barrel 改 `export *`；`cargo` 契约测试、315 项前端测试、`tsc`、Prettier、ESLint、`cargo fmt` 均通过。

### 阶段 0：基线（不改代码）— 已完成

- [x] 完成本审计，缺陷编号与证据固化
- [x] 确认 D1 当前不可达（chat 请求不带 `tools`，`tool_calls_json` 恒 `NULL`）
- [x] 2026-11-02 复核：D1 已由 I30 阶段一删除，其余结论复验成立
- [x] 2026-11-03 修订：重算计数、补 D9–D11、重排阶段

### 阶段 1：D1 收尾（I30 之后，agent 运行时设计之前）

> 定位：字段已删除，此阶段只负责「别让后人误删预留列」和「重新引入时别再犯错」。

- [x] `assistant-storage/src/db.rs:55` 建表语句补注释：该列由 agentloop 写入，**禁止 `DROP`**
- [ ] agentloop Step 7 设计评审时把线格式定死：结构化 `tool_calls: Vec<ToolCallDto>`，禁止把 JSON 文本搬回 DTO
- [ ] 前端恢复 `Message.toolCalls` 时与后端同名同形状，不留映射代码
- [ ] `src-tauri/tests/protocol_contract.rs` 增加结构断言（不是仅键名）

### 阶段 2：低成本、与生成解耦的修复

- [x] **D9**：`_seq` 全链路改名 `seq`（`branch.rs` 字段、`create_branch` / `switch_branch` 参数、`chatTypes.ts`、`chatService.ts`、`chatLifecycleEvents.ts`）
- [x] D9 测试：后端 `chat_messages_changed_payload_uses_seq_key` 断言线格式键为 `seq`；前端 `protocolContract.test.ts` 增加读取断言
- [x] **D10** 删除侧：移除未接线的 `MaxSiblingsExceededErrorDto`
- [ ] **D10** 定案侧：确认 chat 错误通道是否改为 typed DTO（至少记录决策）
- [x] **D11 预研**：本地扩展字段已在 §2.5 / D11 列明（`Message.error`、`ContextUsage`、`RevealedField` 等），命名采用 `*Wire` + 交叉类型
- [x] **D7 止血**：`ProviderInput.models` 改结构化数组，`JSON.stringify` 收敛到 `providerService.saveProvider`

**验收**：`_seq` 改为 `seq` 后，前端能读到事件序号；`rg -n '_seq' src/ src-tauri/src` 无残留；上述死 DTO 引用为 0 或有使用点。

### 阶段 3：生成式契约（原阶段 4 前置）

> 这是本审计最根本的一条：**只要 chat / settings 仍是两份手写副本，D2 / D4 / D5 会反复回归**。生成式契约不是「更规范」，而是唯一能让「形状」这一维度自动保持同步的手段。

**3.1 spike（前置，必须先做）**

- [x] 可行性验证（ts-rs 12.0.1）：`serde-compat` 默认开启，可读 `serde(tag / rename)`；已在 `src-tauri/Cargo.toml` 启用 `serde-json-impl`（处理 `serde_json::Value`）与 `no-serde-warnings`
- [x] 确认内部标签枚举产出：`ContentBlockDto`（`#[serde(tag = "type")]` + 字段 `rename = "tool_use_id"`）生成 `{ "type": "text", ... } | { "type": "image", ... }` 联合，与手写类型一致
- [x] 确认依赖形态：`#[derive(TS)]` 写在非测试代码中，ts-rs 已作为 `assistant-app` 的普通依赖
- [x] 确认导出路径与生成方式：`TS_RS_EXPORT_DIR` 指向 vault 契约目录，协议契约改用**集中式生成测试**（`src-tauri/tests/protocol_bindings.rs`）直接写 `src/types/protocol.contract.ts`；命令为 `cargo test -p assistant-app --test protocol_bindings`
- [x] 确认 serde 属性对生成形状的影响：`deny_unknown_fields` 被忽略且无警告；`#[serde(default)] + skip_serializing_if` 映射为可选键 `?:`

**3.2 拆分 view model（D11）**

- [x] 生成类型直接作为 wire 类型（`MessageDto` 等）；本地扩展改为交叉类型（`Message = MessageDto & { error?: ChatError | null }`）
- [x] barrel 只做 re-export（vault `export *`），本地类型单独声明并注明「非线格式」

**3.3 生成与接线**

- [x] 给 `src-tauri/src/protocol/dto/**` 加 `ts-rs`（阶段 4 后实为 57 个类型），生成 `src/types/protocol.contract.ts`
- [x] `chatTypes.ts` / `provider/types.ts` / `confTypes.ts` / `types/events.ts` 改为从生成文件 re-export（阶段 4 完成）
- [x] `npm run contract:gen` 覆盖 vault + protocol 两个 crate
- [x] vault barrel 改 `export * from "./contract/vault.contract"`，消除 38/39 漂移

**3.4 本地一致性检查（替代 CI）**

- [x] 新增 `npm run contract:check`：生成后 `git diff --exit-code` 校验工作区；仓库无 CI，不设流水线门禁
- [ ] 把该命令写入提交前约定（是否挂 hook 由维护者决定），与 `docs/achievement2/I23-vault-ipc-contract.md` 的 D8 结论保持一致

**验收（阶段 3 完成时）**：`npm run contract:gen` 成功且生成两个 contract 文件；`git diff --exit-code` 为空；前端 `tsc` 通过；barrel 无手工枚举。**当前状态（2026-11-03）**：已完成——57 个协议类型生成、前端全部接线、barrel 改 `export *`。

### 阶段 4：统一契约语义（原阶段 2，生成后执行）

- [x] 输出 DTO 移除全部 `skip_serializing_if`（共 63 处；vault 已是 `null` 语义，chat / branch / settings / os_tool 已对齐）
- [x] 前端 `tokens` / `activeBranchId` / `Message.reasoning` 等改为 `T | null` 且**必填**；前端已切换到生成契约
- [x] `deny_unknown_fields` 只补到输入 DTO（`VaultEntryCreate` / `VaultEntryUpdate` / `CustomFieldInput` / `SyncConfigDto`）；前端类型与测试已验证无多余键
- [x] `SendChatMessageArgsDto` 补 `deny_unknown_fields`，原「忽略未知键」测试改为拒绝断言
- [x] 封闭集合改 `enum`：`vault_cipher_algorithm`（config + DTO 双层）、两个 `change_kind`、`AgentMode.kind`、`ConversationQueryOptsDto.mode`、`SettingProviderChangedPayloadDto.action`
- [x] `role` / `title_source`：本轮保留 `String`（DB 列为 TEXT，避免迁移风险），按 D5 存储边界例外处理；存量核对留待后续迁移
- [x] `finish_reason` 属开放集合：保持 `String`，前端仅透传，不做窄化
- [x] `ProviderInputDto.models` 改结构化数组，`save_provider` 内部 `serde_json::to_string`；前端不再 stringify
- [x] 规范条目写入 `.trae/rules/project_rules.md`（类型安全一节新增「DTO 契约」）

**执行状态（2026-11-03）**：阶段 4 已完成。`chatTypes.ts` / `provider/types.ts` / `confTypes.ts` / `events.ts` 已改为从生成契约 re-export；`vault.types.ts` barrel 改 `export *`；本地状态用交叉类型（如 `Message = MessageDto & { error?: ChatError | null }`）。前端 `tsc`、315 项测试、Prettier、ESLint 全部通过。

**验收**：`rg -n 'skip_serializing_if' src-tauri/src/protocol/dto` 已为 0；所有封闭集合字段无 `String`；`deny_unknown_fields` 输入覆盖清单为 0 缺口。

### 阶段 5：数值域（P1）

- [x] 删除 `crack_time_seconds`（前端 0 处消费，已实测）
- [x] `revision` 全链路统一 `i64`；`as` 改 `try_from`（先处理 `snapshot.rs:86`、`import.rs:34/63/139/169/207-208/230/270`）
- [x] 计数 / 字节类 `u64` 收窄 `u32`（`StorageBreakdownDto` / `AttachmentDeleteResultDto`），并说明上限来源
- [x] 时间戳加单位别名；构造处校验值域
- [x] 对 `assistant-vault` 开 `clippy::cast_sign_loss` / `cast_possible_truncation`（采用 §4.4 的 clippy 门禁方案）；`assistant-storage` 的 21 处跨界转换尚未逐条论证值域，延后

**执行状态（2026-11-03）**：阶段 5 已完成。

- `crack_time_seconds` 已从 `StrengthResult` 与构造处删除，`vault.contract.ts` 重新生成后仅剩 `crackTimeDisplay`。
- `revision` 在 DTO（`VaultEntryDetail` / `EntryPreviewDto`）、域类型（`VaultEntrySnapshot` / `VaultEntryImport` / `VaultEntryDecrypted`）、同步模型（`EntryPreview`）与落盘文件（`EntryFile` / `ImportedEntry`）统一为 `i64`；`decode_entry_file` 对负值返回错误，保留「落盘 revision 非负」约束；`snapshot.rs` / `crypto_ops.rs` / `import.rs` / `deleted.rs` 的裸 `as` 全部移除，`bool → i64` 改用 `i64::from`。
- 计数：`StorageBreakdownDto.total_count` / `AttachmentDeleteResultDto.removed_count` 改 `u32`，命令层用 `u32::try_from` 返回错误；`total_bytes` / `removed_bytes` / `AttachmentSummary.bytes` 保留 `u64`，因未找到目录配额常量，无法论证更窄上限。
- 时间戳：`assistant-vault::types` 新增 `Millis` 别名、`MAX_SAFE_JSON_MILLIS` 与 `now_millis()`（构造处 `debug_assert` 值域），所有写入路径改用 `now_millis()`；`protocol/dto` 新增 `Millis` 别名与单位说明。后续待办：把 `protocol/dto` 的既有时间戳字段逐个改用该别名（避免本轮大范围改动）。
- 门禁：`assistant-vault/src/lib.rs` 增加 `#![deny(clippy::cast_sign_loss, clippy::cast_possible_truncation)]`，`cargo clippy -p assistant-vault --lib` 通过；`biometric/windows.rs` 的 HRESULT 位模式转换加 `#[allow]` 与注释。
- 验证：`cargo test -p assistant-vault`（43 + 64 集成测试）、`cargo test -p assistant-app --test protocol_contract`（10）、`cargo fmt --check`、`tsc`、`vitest`（315）、Prettier、ESLint 全部通过。

**验收**：`assistant-vault/src` 裸 `as u64` / `as i64` 为 0（白名单除外且有值域注释）；`cargo clippy` 无新增告警；`crack_time_seconds` 无引用。

### 阶段 6：测试补强（P2）

- [ ] 后端导出 JSON fixture，前端 `JSON.parse` 后消费（不再自构造对象）
- [ ] 数值边界用例（`2^53-1` / `2^53` / `u64::MAX`）
- [ ] 删除或重写自证式断言（`src/chat/core/__tests__/protocolContract.test.ts:54-220`）
- [ ] `seq` 键名回归测试（与阶段 2 合并，不重复建设）
- [ ] 错误通道测试：至少覆盖一条命令的错误形状

---

## 6. 新增 / 修改 DTO 的检查清单

1. **归属**：字段名会出现在 `.vault` / CSV / DB 列里？→ 不进 DTO。历史反例 `MessageDto.tool_calls_json`（DTO 字段名与 SQLite 列名同形、值是列格式的 JSON 文本）已由 I30 删除；同类现存活反例见 D7（`ProviderInputDto.models`）。
2. **命名**：`#[serde(rename_all = "camelCase")]`；**禁止以下划线开头的 wire 字段**（`_seq` 会被 camelCase 吞掉前导下划线，见 D9）。
3. **可选性**：输出用 `T | null`（不 skip）；输入用 `?: T | null`（`#[ts(optional = nullable)]`，无需 `default`）。同一 DTO 内不得混用「可选」与「可选 + 可空」两种风格。
4. **数值**：按 §4.2 判定；`u64` 必须能说明「为什么不是 `u32`」，跨域转换禁止裸 `as`。
5. **单位**：时间字段名带单位或类型别名；禁止同一字段在不同域混用秒 / 毫秒。
6. **枚举**：封闭集合在 Rust 用 `enum`；开放集合保留 `String` 但前端必须 `assertNever` 兜底并校验边界。
7. **生成**：vault / protocol 改完必须 `npm run contract:gen` 并提交生成文件；barrel 用 `export *`，禁止手工枚举。
8. **方向**：输入 DTO 加 `deny_unknown_fields`；纯输出 DTO 不把该属性当作契约保证。
9. **错误通道**：命令错误要么返回 typed DTO，要么在文档中明确为 `String` 并保证前端能稳定解析；禁止定义未接线的错误 DTO。
10. **视图模型**：wire 类型不得承载本地 UI 状态（如 `Message.error`）；本地扩展用交叉类型单独声明。
11. **测试**：至少一条「后端真实 JSON → 前端类型」的断言，而不是自构造对象；关键可选字段要有键名断言。

---

## 7. 未决 / 待实测

| # | 项 | 需要的动作 |
|---|---|---|
| 1 | agentloop Step 7 的 `tool_calls` 线格式 | 设计评审时定死为结构化 `Vec<ToolCallDto>`（见阶段 1） |
| 2 | ~~`crack_time_seconds` 的实际最大值~~ | **已关闭**：前端 0 处消费，阶段 5 直接删除，无需再测上限 |
| 3 | `_seq` 防乱序是否仍需保留 | 若分支命令已串行 await，可删除整套守卫；否则按阶段 2 改名并补测试（先做决策，再改代码） |
| 4 | chat 错误通道是否 typed | 定案 `ChatErrorDto` 或明确保留 `String`；两者都要有测试 |
| 5 | ~~`ProviderInputDto.models` 结构化成本~~ | **已完成**：DTO 改 `Vec<ModelConfigDto>`，命令层 `serde_json::to_string`，前端不再 stringify |
| 6 | 是否还有其它前端未消费的 DTO 字段 | 对 `vault.contract.ts` 的 39 个类型做一次「字段级消费率」统计（本次只查了 `crackTimeSeconds`、`toolCallsJson`） |
| 7 | `ToolCall` 是否需要保留原始 JSON 字符串 | 若前端有「展示原始参数」需求，`ToolCallDto.arguments` 保持 `String` 即可，不必二次解析 |
| 8 | ts-rs 全域化的实际产出 | **已完成**：57 个类型生成成功，`ContentBlockDto` 联合与手写形状一致；前端已全部接线 |
| 9 | ~~`SendChatMessageArgsDto` 是否该严格~~ | **已完成**：已补 `deny_unknown_fields`，测试改为拒绝未知字段 |

---

## 8. 复核命令（可重跑）

```bash
rg -n "pub struct|pub enum" src-tauri/src/protocol/dto
rg -n "deny_unknown_fields" src-tauri/src/protocol/dto
rg -n "skip_serializing_if" src-tauri/src/protocol/dto
rg -n "ts\(export" src-tauri/crates/assistant-vault/src
rg -n "^export (type|interface|enum)" src/vault/core/contract/vault.contract.ts
rg -n "_seq" src/services src/chat src-tauri/src/protocol/dto src-tauri/src/cmd
rg -n "as u64|as i64" src-tauri/crates/assistant-vault/src
```

预期：协议 DTO 52 个；`deny_unknown_fields` 48 处；`skip_serializing_if` 63 处属性（chat 46 / settings 13 / branch 2 / os_tool 2）；vault `#[ts(export)]` 39 个；contract 导出 39 行；`_seq` 命中应只剩待修复项；vault 裸 `as` 25 处。

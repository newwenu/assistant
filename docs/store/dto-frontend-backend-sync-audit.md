# DTO 前后端同步审计（收口前基线）

> 审计日期：2026-10-30
> 审计范围：`src-tauri/src/protocol/dto/**`、`src-tauri/crates/assistant-vault/src/dto/**`、`src/**` 中消费这些 DTO 的 service / store / 类型文件
> 审计方法：全量 `#[serde]` / `#[ts]` 派生检索 + `#[command]` 签名核对 + 前端类型逐字段对照 + 契约测试有效性检查 + 数值域推导
> 证据基准：**只采当前仓库源码**。`docs/achievement*/` 下的存档文档不作为依据——其中的分层/收口方案在代码中并未执行（例如「持久化格式不进 DTO」这条在 `MessageDto.tool_calls_json` 上就没落实），引用它们会把「计划」误当成「现状」
> 结论性质：除标注「推算」外，均为源码实测

---

## 0. 结论摘要

1. **同步机制有两套，只有一套是真同步**。vault 域是 ts-rs 生成式（单一真源）；chat / settings / config / storage / orb / branch 域是「Rust 手写 DTO + TS 手写 interface」两份副本，靠测试兜底，而该测试是自证的。
2. **本次未发现现网可达的功能缺陷**。唯一看起来像 P0 的 `MessageDto.tool_calls_json`，经可达性追踪确认是**未接通的预留字段**（agent 运行时尚未设计，chat 请求体不带 `tools`，故该字段恒为 `NULL`），现网不可达，详见 D1。
3. **发现 1 类系统性契约谎言（P1）**：`Option` 的缺省语义在 vault（`null`）与 chat（键缺失）两域相反，而前端按 `| null` 声明 → 运行时可能是 `undefined`。
4. **数值类型结论**：wire 层用 `number`，但**必须由值域证明背书**；不可证明的字段改 `string` 编码。`BigInt` 不能作为 DTO 类型（JSON 无法承载），只允许在前端内部做精确运算。
5. **值域不能靠两端各写一个宽松类型来"对齐"**：生成器只同步形状，不同步值域；值域必须由**更窄的一端定义**，并由**值域测试**守住。

---

## 1. 同步机制现状

### 1.1 两套机制对照

| 域 | Rust 定义 | TS 定义 | 生成方式 | 同步强度 |
|---|---|---|---|---|
| vault | `src-tauri/crates/assistant-vault/src/dto/**`（36 个导出类型） | `src/vault/core/contract/vault.contract.ts` | ts-rs `#[ts(export)]`，`npm run contract:gen` | **形状强同步**（结构、可选性、枚举联合都由生成器保证）；值域无保证 |
| chat | `src-tauri/src/protocol/dto/chat.rs`（含 `MessageDto` / `ConversationDto` 等） | `src/chat/core/chatTypes.ts` | 手写两份 | 弱：仅靠测试断言键名 |
| branch | `src-tauri/src/protocol/dto/branch.rs` | `src/chat/core/chatTypes.ts`（`SiblingInfo` / `ChatMessagesChangedPayload`） | 手写两份 | 弱 |
| settings | `src-tauri/src/protocol/dto/settings.rs` | `src/settings/provider/types.ts`、`src/types/events.ts` | 手写两份 | 弱 |
| config | `src-tauri/src/protocol/dto/config.rs` | `src/settings/core/types/confTypes.ts` | 手写两份 | 弱 |
| storage | `src-tauri/src/protocol/dto/storage.rs` | 手写（消费点分散） | 手写两份 | 弱 |
| orb | `src-tauri/src/protocol/dto/orb.rs` | 手写 | 手写两份 | 弱 |

`src-tauri/src/protocol/dto/**` 共 43 个 `pub struct` / `pub enum`，全部**没有** `TS` 派生；ts-rs 依赖只出现在 `src-tauri/crates/assistant-vault/Cargo.toml`。

### 1.2 三层传递链

```
SQLite 列类型 ──► Rust domain struct ──► Rust DTO ──► serde_json ──► WebView JSON.parse ──► TS interface
   (i64)              (i64/u64)          (i64/u64)      (精确整数)      (f64 双精度)        (number)
```

关键事实：**精度丢失发生在第 5 步**（`JSON.parse` 只能产出 f64），而不是 serde 序列化。因此「后端用 u64、前端用 number」这套约定，其安全性完全取决于业务值是否 ≤ 2^53-1，而这一点**当前没有任何一处代码或测试表达**。

### 1.3 已验证事实（避免后人重复踩坑）

| 事实 | 验证方式 | 结论 |
|---|---|---|
| serde 的 `Option<T>` 字段缺失时，**不加** `#[serde(default)]` 也能反序列化为 `None` | 阅读 serde 1.0.229 源码 `serde/src/private/de.rs:24-57`：`MissingFieldDeserializer` 的 `deserialize_option` 返回 `visit_none()` | vault 的 `#[ts(optional = nullable)]` + `Option<T>`（无 `default`）两端一致，**不要误加 `default`** |
| ts-rs 把 `i64` / `u64` 一律映射为 `number` | `src/vault/core/contract/vault.contract.ts` 全量字段对照 | 生成器不表达值域 |
| 前端 `invoke<T>` 是纯泛型透传，无运行时校验 | `src/services/tauriInvoke.ts:5-9` | TS 类型是对后端 JSON 的**无检查断言** |

---

## 2. 缺陷清单

> 编号 `D1..D8`，P0 = 现网可达的功能缺陷，P1 = 契约谎言（类型与实际不符），P2 = 一致性/可维护性，P3 = 风格。**本次审计 P0 为空**，D1 虽严重但当前不可达，单列为「未接通」。

### D1（未接通 / 预留）`tool_calls_json` 在线格式上存在，前端完全不消费

**定性**：这不是现网缺陷，而是**契约提前落地、运行时尚未接通**。当前没有任何数据路径能写入该字段，因此用户可见行为无异常；但它是一颗**会在 agent 运行时落地当天引爆的雷**，也是修复成本最低的时刻。

**现象**

- 后端 `MessageDto` 字段为 `tool_calls_json: Option<String>`（`src-tauri/src/protocol/dto/chat.rs:102`），序列化名 `toolCallsJson`，值是**JSON 文本**。
- 写入路径：`src-tauri/src/cmd/chat/mod.rs:222-226` 把 `outcome.tool_calls` 序列化成字符串落库。
- 前端 `Message` 只有 `toolCalls?: ToolCall[]`（`src/chat/core/chatTypes.ts:9`），**没有** `toolCallsJson`。
- 全仓检索 `toolCallsJson` / `tool_calls_json`：前端命中 **0 处**；`JSON.parse` 仅出现在 `src/stores/toolStore.ts:67`（与本字段无关）。

**可达性判定（为什么当前是死字段）**

沿「谁能让 `outcome.tool_calls` 非空」逐跳回溯，链条在第一跳就断了：

| 跳 | 位置 | 事实 |
|---|---|---|
| ① 模型只能对**已声明**的工具发起调用 | `src-tauri/crates/assistant-providers/src/openai/types.rs:5-15` | `ChatRequest` 只有 `model` / `messages` / `stream` / `temperature` / `max_tokens` / `stream_options`，**没有 `tools` 字段**，请求体从不声明工具 |
| ② Gemini 请求同样不带工具 | `src-tauri/crates/assistant-providers/src/gemini/request.rs:41` | `tools: None` 写死 |
| ③ 工具注册表未接入对话链路 | `src-tauri/src/setup.rs:93` | `ToolRegistry` 只被 `src-tauri/src/cmd/os_tool_exec.rs:18` 使用（前端主动执行工具），chat 链路从未读取它 |
| ④ 因此 `outcome.tool_calls` 恒为 `None` | `src-tauri/src/cmd/chat/mod.rs:222-226` | `.filter(|tc| !tc.is_empty())` → `tool_calls_json` 恒 `None` → DB 列恒 `NULL` |
| ⑤ 前端增量逻辑同样永不触发 | `src/stores/chatStorePorts.ts:142-154` | `appendToolCallDelta` 依赖流内 `tool_call` 增量，链路 ①–④ 已断，事件不会出现 |

结论：前端 `Message.toolCalls` 与 `MessageBubble` 工具卡片（`src/chat/message/MessageBubble.tsx:58`、`:212`）是**已就绪但永不显示**的一侧；后端是**已定义但永不填充**的一侧。两端都停在「等 agent 运行时」。

**会在何时变成真实缺陷**

agent 运行时一旦接入（请求开始声明 `tools`，模型开始返回 `tool_calls`），`tool_calls_json` 就会真正落库并出现在 `get_conversation` / `chat-messages-changed` 的返回里。此时前端类型里没有对应字段 → 切换会话或重进历史后，已落库的工具调用不会出现在 `message.toolCalls`，工具卡片消失；且因为该字段是 JSON 文本，前端即便想补救也得先手写一次 `JSON.parse` + 结构校验。

**根因**

DTO 直接暴露了**数据库列形状**——`tool_calls_json` 既是列名又是列格式，前端拿到的是「待二次解析的字符串」而不是数据。按本文 §5 第 1 条（字段名会进 DB 列 / 落盘格式的一律不进 DTO），它本就不该出现在线格式上。DB 列保留 `tool_calls_json TEXT` 没问题（那是落盘格式），但**线格式**应当结构化。

**建议（趁 agent 运行时尚未设计，现在改成本最低）**

后端改为结构化字段，让 DTO 承担反序列化职责：

```rust
// protocol/dto/chat.rs
pub struct ToolCallDto { pub id: String, pub name: String, pub arguments: String }
pub tool_calls: Vec<ToolCallDto>,   // serde(default)
```

`From<MessageRow>` 内 `serde_json::from_str(&json).unwrap_or_default()`（沿用现有 `unwrap_or_default` 容错风格）。前端 `Message.toolCalls` 与后端同名同形状，**不再需要任何映射代码**，且 agent 运行时接入时无需再动契约。

备选方案（不动后端）：在 `chatService.getConversation*` 出口加一层 `MessageDto → Message` 适配器。缺点是「手写映射」会把同一份契约再复制一次，长期看比改后端更贵。

---

### D2（P1）`Option` 缺省语义两域相反，前端类型说谎

**现象**

| 域 | 约定 | 例 |
|---|---|---|
| vault | `Option<T>` **不** skip → 键存在，值为 `null` | `VaultEntryDetail.username: string \| null`（`vault.contract.ts:65`） |
| chat / branch / settings | 大量 `skip_serializing_if = "Option::is_none"` → 键**缺失** | `MessageDto.tokens`（`chat.rs:93-94`）、`active_branch_id`（`branch.rs:30`）、`retry_after_ms`（`chat.rs:246-247`） |

前端对应声明却是「必填可空」：

- `Message.tokens: number | null`（`src/chat/core/chatTypes.ts:10`）
- `ChatMessagesChangedPayload.activeBranchId: string | null`（`src/chat/core/chatTypes.ts:152-158`）
- `ChatError.retryAfterMs: number | null`（`src/chat/core/chatErrors.ts:32-42`）

于是类型说「一定是 number 或 null」，运行时却可能是 `undefined`。当前只是靠松散判断侥幸没出错（`src/history/detail/HistoryMessageItem.tsx:110` 用了 `!= null`，同时覆盖 null 与 undefined）。

**建议**（二选一，写进规范，不允许两域各自为政）

- **方案 A（推荐）**：所有**输出** DTO 一律不 skip，`Option` 输出 `null`。前端统一用 `T | null`。理由：键永远存在 → 前端可做 `Object.keys` 级断言，也让「缺字段」与「空值」两种语义不会混。
- **方案 B**：保留 skip，前端统一用 `?: T`（即 `T | undefined`）。缺点：无法区分「后端没实现」与「值为空」。

`skip_serializing_if` 只应保留在**输入** DTO（减小请求体），不应出现在输出 DTO。

---

### D3（P1）数值域单侧放宽

**现象**：Rust 侧用语义上更宽的类型，TS 侧照抄成 `number`，中间没有任何值域表达或校验。

| 字段 | Rust 类型 | 实际值域 | 风险 |
|---|---|---|---|
| `StrengthResult.crack_time_seconds` | `u64`（`crates/assistant-vault/src/dto/audit.rs:10`） | 无上界（zxcvbn 离线慢哈希估算可到 1e17 秒量级，**推算**） | **可超 2^53**，当前仅因前端 0 处消费而未爆 |
| `VaultEntryDetail.revision` | `u64`（`dto/mod.rs:100`） | 单调递增小整数 | 三层域不一致：SQLite `INTEGER`(i64，`security_db/schema.rs:23`) → `i64 as u64` → DTO `u64` → TS `number` |
| `EntryPreviewDto.revision` | `u64`（`dto/sync.rs:151`） | 同上 | 同上 |
| `StorageBreakdownDto.total_bytes / total_count` | `u64`（`protocol/dto/storage.rs:25-26`） | 受附件目录配额约束（256MB 级） | 实际安全，但类型不表达上限 |
| `AttachmentDeleteResultDto.removed_count / removed_bytes` | `u64` | 同上 | 同上 |
| `RetryNotificationDto.delay_ms` | `u64`（`chat.rs:223`） | 秒级 | 实际安全 |
| 时间戳 `created_at / updated_at / timestamp / expiresAt / commitTime` | `i64` 毫秒 | 2^53 ms ≈ 公元 287396 年 | 当前安全；**仅靠文档约定单位**，改 µs 剩 285 年，改 ns 直接爆 |
| `DisasterRecoveryResultDto.entry_count` | `u32`（`dto/recovery.rs:9`） | 有界 | ✅ 正确示范 |
| `max_tokens` / `presentation_rank` / `context_window` / `width` / `height` | `u32` | 有界 | ✅ |
| `compact_threshold_percent` / `opacity_percent` / `score` | `u8` | 有界 | ✅ |
| `compact_summary_max_tokens` / `scale_percent` / `vault_auto_lock_minutes` | `u16` | 有界 | ✅ |

**Rust 侧镜像放宽**：`i64 ↔ u64` 全部用裸 `as`，越界时**静默回绕**。实测 30 处，其中包含往返转换：

- `crates/assistant-vault/src/security_db/crud.rs:135` `row.get::<_, i64>("revision")? as u64`
- `crates/assistant-vault/src/security_db/crud.rs:413` `max_revision.max(entry.revision as i64) as u64 + 1`（u64 → i64 → u64）
- `crates/assistant-vault/src/security_db/crud.rs:889-890` `entry.revision < revision as u64`

**建议**

1. 计数/字节/延迟类 `u64` 收窄为 `u32`（`recovery.rs` 已是范例），让类型自己表达上限。
2. `revision` 全链路统一 `i64`（与 SQLite 列一致），去掉两次域转换；转换处用 `i64::try_from` 而非 `as`。
3. `crack_time_seconds`：前端 0 消费 → **直接删除**该字段，或改 `string` 编码（见 §3.2）。
4. 时间戳用类型别名把单位焊进名字（`Millis`），DTO 构造处加 `debug_assert!(ts.abs() <= (1i64 << 53) - 1)`。

---

### D4（P2）`deny_unknown_fields` 两域不一致

- `src-tauri/src/protocol/dto/**`：42 处，**全部输入/输出 DTO 都严格**。
- `crates/assistant-vault/src/dto/**`：**0 处**。

后果：前端多传一个字段时，chat/settings 命令直接反序列化失败（用户看到错误），vault 命令静默忽略（同样的问题表现为「改了没生效」）。同一类错误在两域得到两种用户体验，且 vault 侧拼错字段名不会报错。

**建议**：vault 的**输入** DTO（`VaultEntryCreate` / `VaultEntryUpdate` / `CustomFieldInput` / `SyncConfigDto`）补 `deny_unknown_fields`；输出 DTO 可选择性补（加了也无害，且能防止生成器把内部字段意外导出后前端误用）。

---

### D5（P2）前端窄化、后端宽化（反向的单侧放宽）

| 字段 | Rust | TS | 后果 |
|---|---|---|---|
| `VaultConfDto.vault_cipher_algorithm` | `String`（`protocol/dto/config.rs:43`） | `"aes256Gcm" \| "chacha20Poly1305"`（`src/settings/core/types/confTypes.ts:16`） | 后端若给出第三种值，TS 静默把它当联合成员 |
| `ChatMessagesChangedPayloadDto.change_kind` | `String`（`protocol/dto/branch.rs:27`） | `MessageChangeKind` 联合（`chatTypes.ts:154`） | 同上；前端 `switch` 无 `default` 时会走空分支 |

这与 D3 是同一问题的两个方向：**谁更宽谁就吞掉错误**。类型窄的一侧必须要么有运行时校验，要么由宽的一侧（后端）用 enum 保证。

**建议**：后端把这两个字段改成 enum（Rust 侧强类型 + ts-rs/serde 生成联合），前端联合随之自动正确；手写域至少加 `assertNever` 兜底。

---

### D6（P2）契约测试是自证的，不构成同步保证

- 前端：`src/chat/core/__tests__/protocolContract.test.ts:49-83` 构造一个字面量、声明为 `Message`，然后断言它有自己刚写进去的键 —— **恒真**，与后端无关。
- 后端：`src-tauri/tests/protocol_contract.rs:119-160` 只断言 `contains_key("createdAt")` 与「无 snake_case」，`chat_delta_payload_dto_round_trip` 只验证字段名往返。
- 缺失：**真实后端 JSON → 前端类型**的单向校验；数值域边界用例。

后果：后端把 `u32` 改成 `u64`、新增字段、改可空性、改单位，全部测试保持绿色。

**建议**

1. 后端用 `serde_json::to_value(&dto)` 生成 JSON fixture（提交进仓库），前端用该 fixture 做类型断言 + 关键字段运行时校验。
2. 补数值边界用例：`2^53-1` 正常、`2^53` 触发断言或被编码为字符串、`u64::MAX` 不 panic。
3. 生成式契约加 CI 校验：`npm run contract:gen` 后 `git diff --exit-code src/vault/core/contract/`，防止手改生成文件。

---

### D7（P2）同一概念双向形状不同

`Provider` 在线上有两种形状：

| 方向 | 后端 | 前端 | 序列化职责 |
|---|---|---|---|
| 输出 | `ProviderListDto.models: Vec<ModelConfigDto>`（`settings.rs:116`） | `Provider.models: ModelConfig[]` | 结构化，正常 |
| 输入 | `ProviderInputDto.models: String`（JSON 文本，`settings.rs:103`） | `ProviderInput.models: string` | 由 UI 层手写 `JSON.stringify(models)`（`src/settings/provider/ConnectDialog.tsx:213`） |

不是错误，但把「序列化」这件事交给了最不该负责的 UI 组件；且同名概念两种形状，容易在新增调用点漏掉 stringify。

**建议**：输入也改为结构化数组，由 serde 负责序列化；若因历史原因保留，至少在 `providerService.saveProvider` 集中做 stringify，禁止 UI 层直接拼。

---

### D8（P3）命名与 barrel 别名

- vault 的生成文件保留 `*Dto` 后缀，barrel 再重命名为无后缀（`src/vault/core/vault.types.ts:20-45`：`ConflictFileDto as ConflictFile`）。
- chat 域 Rust 有 `Dto` 后缀，前端却从未出现 `Dto`。
- 结果：两域的心智模型不同，新人无法从名字判断「这个类型是否来自后端」。

**建议**：要么全仓保留 `Dto` 后缀（生成式域不动 barrel），要么全仓在 barrel 统一去后缀并注明「`*Dto` 为线格式、无后缀为领域类型」。当前 vault 已有明确注释（`vault.types.ts:1-6`），建议把同样注释复制到手写域。

---

## 3. 数值类型决策：`BigInt` vs `number`

### 3.1 为什么 `BigInt` 不能作为 DTO 类型

| 约束 | 事实 |
|---|---|
| 序列化 | `JSON.stringify(1n)` 直接抛 `TypeError`；Tauri v2 命令的返回值与参数都经 `serde_json` → JSON 文本 |
| 反序列化 | `JSON.parse` **永远**只产出 `number`，不会产出 `BigInt` |
| 生态 | `1n + 1` 抛错、`Math.max` 不接受、`1n === 1` 为 `false`、`InputNumber` / 图表库需改写 |
| 性能 | 堆分配，比 `number` 慢一个量级；而本项目 99% 的字段是计数、字节数、毫秒时间戳 |

因此**能进 wire 的只有 `number` 或 `string`**。`BigInt` 只允许出现在前端内部（例如位运算），且必须在解析 `string` 之后。

项目内唯一正确用例：`src/tools/ip-info/ip-info.service.ts` —— 纯前端内部做无符号位运算，不进 IPC，属正例。

### 3.2 判定规则（新增/修改字段时逐条执行）

> 问：这个字段的**所有写入路径**，能否论证 `|v| ≤ 2^53-1`？

| 答案 | 采用 | 落地写法 |
|---|---|---|
| 能，且无位运算需求 | `number` | Rust 用能表达上限的窄类型（`u8/u16/u32/i32`），或 `i64` + 构造处断言 |
| 能，但类型必须宽（如 `i64` 时间戳） | `number` | 类型别名焊单位 + `debug_assert` 值域 |
| **不能** | `string` | Rust `#[serde(with = "u64_as_string")]` + `#[ts(type = "string")]`；前端按需 `BigInt(str)` |
| 需要精确单调比较（revision） | **收窄**而不是升 BigInt | 统一 `i64`，与 SQLite 列一致 |

安全边界数值：`2^53-1 = 9_007_199_254_740_991`（约 9.0e15）。

### 3.3 安全性排序

```
string on wire + 按需 BigInt  >  number + 已证明值域  >  number + 未证明值域  >  BigInt on wire（不可实现）
```

---

## 4. 收口方案（分阶段）

### 阶段 0：基线（不改代码）

- [x] 完成本审计，缺陷编号与证据固化
- [x] 确认 D1 当前不可达（chat 请求不带 `tools`，`tool_calls_json` 恒 `NULL`），无需实机复现

### 阶段 1：D1 契约前置修正（建议在 agent 运行时设计**之前**完成）

> 定位说明：这不是修 bug，而是**趁运行时还没接通，把线格式改对**。越晚改，越可能要同时处理已落库的 JSON 文本与前端映射。

- [ ] `MessageDto` 用结构化 `tool_calls: Vec<ToolCallDto>` 替代 `tool_calls_json: String`（DB 列 `tool_calls_json TEXT` 保持不变）
- [ ] 前端 `Message.toolCalls` 与后端同名同形状，不留任何映射代码
- [ ] `src-tauri/tests/protocol_contract.rs` 增加 `toolCalls` 结构断言（不是仅键名）
- [ ] 若 agent 运行时设计晚于本阶段，可暂缓但**必须在设计评审时把该字段形状定死**

### 阶段 2：统一 Option 语义（P1）

- [ ] 输出 DTO 移除全部 `skip_serializing_if`（vault 已是 `null` 语义，chat 向其对齐）
- [ ] 前端 `tokens` / `activeBranchId` / `retryAfterMs` 等改为 `T | null` 且**必填**
- [ ] `deny_unknown_fields` 补到 vault 输入 DTO
- [ ] 规范条目写入 `docs/store/` 与 `.trae/rules/project_rules.md`（类型安全一节）

### 阶段 3：数值域（P1）

- [ ] 删除或 string 化 `crack_time_seconds`
- [ ] `revision` 全链路统一 `i64`；`as` 改 `try_from`
- [ ] 计数/字节类 `u64` 收窄 `u32`
- [ ] 时间戳加单位别名 + `debug_assert` 值域

### 阶段 4：扩大生成式契约（P2）

- [ ] 给 `src-tauri/src/protocol/dto/**` 加 `ts-rs`（43 个类型），生成 `src/types/protocol.contract.ts`
- [ ] `chatTypes.ts` / `confTypes.ts` / `types/events.ts` 改为从生成文件 re-export
- [ ] `npm run contract:gen` 覆盖全域；CI 加 `git diff --exit-code` 校验

> 这是本审计最根本的一条：**只要 chat/settings 仍是两份手写副本，D2/D4/D5 会反复回归**。生成式契约不是「更规范」，而是唯一能让「形状」这一维度自动保持同步的手段。

### 阶段 5：测试补强（P2）

- [ ] 后端导出 JSON fixture，前端消费
- [ ] 数值边界用例（`2^53-1` / `2^53` / `u64::MAX`）
- [ ] 删除或重写自证式断言（`protocolContract.test.ts:49-83`）

---

## 5. 新增 / 修改 DTO 的检查清单

1. **归属**：字段名会出现在 `.vault` / CSV / DB 列里？→ 不进 DTO。这条标准由本次实测归纳（`MessageDto.tool_calls_json` 是设计层面的反例：DTO 字段名与 SQLite 列名同形，且值是列格式的 JSON 文本；虽然当前因运行时未接通而不可达，但形状本身就是错的）。
2. **命名**：`#[serde(rename_all = "camelCase")]`；输入 DTO 加 `deny_unknown_fields`。
3. **可选性**：输出用 `T | null`（不 skip）；输入用 `?: T | null`（`#[ts(optional = nullable)]`，无需 `default`）。
4. **数值**：按 §3.2 判定；`u64` 必须能说明「为什么不是 `u32`」。
5. **单位**：时间字段名带单位或类型别名；禁止同一字段在不同域混用秒/毫秒。
6. **枚举**：能在 Rust 用 `enum` 就不要用 `String`（否则前端联合是凭空猜的）。
7. **生成**：vault 改完必须 `npm run contract:gen` 并提交生成文件。
8. **测试**：至少一条「后端真实 JSON → 前端类型」的断言，而不是自构造对象。

---

## 6. 未决 / 待实测

| # | 项 | 需要的动作 |
|---|---|---|
| 1 | ~~D1 是否真实导致工具卡片丢失~~ → 已澄清：agent 运行时未设计，`tool_calls_json` 当前无写入路径 | 无（改为在 agent 运行时设计评审时定死 `tool_calls` 线格式，见阶段 1） |
| 2 | `crack_time_seconds` 的实际最大值 | 用 zxcvbn 对 20 位随机串取 `guesses_log10`，换算秒数确认是否 > 2^53 |
| 3 | 是否还有其它前端未消费的 DTO 字段 | 对 `vault.contract.ts` 36 个类型做一次「字段级消费率」统计（本次只查了 `crackTimeSeconds`、`toolCallsJson`） |
| 4 | `ToolCall` 是否需要保留原始 JSON 字符串 | 若前端有「展示原始参数」需求，`ToolCallDto.arguments` 保持 `String` 即可，不必二次解析 |
| 5 | ts-rs 全域化的成本 | 估算 `protocol` crate 加 `ts-rs` 后生成的类型数量与 barrel 重写工作量 |
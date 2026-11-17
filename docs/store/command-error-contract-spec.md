# 命令错误契约规格（C6 立项文档）

| 项 | 值 |
|---|---|
| 状态 | 首批已落地（B0–B2 完成，B3 人工回归待执行；C6 仍开放：未迁域仍返回字符串错误） |
| 日期 | 2026-11-17 |
| 来源 | compat-tech-debt-audit-1（已归档） §3 C6（优先级 P3，结构性） |
| 影响范围 | `src-tauri/src/protocol/dto/`（新增 `error.rs`）、`src-tauri/src/cmd/chat/`、`src-tauri/src/cmd/agent/`、`src-tauri/src/cmd/mod.rs`、`src/chat/core/`、`src/i18n/locales/`、`src-tauri/crates/assistant-i18n/locales/` |
| 验证 | `npm run contract:gen` → `cargo test -p assistant-app --test protocol_dto_roundtrip` → `npm run contract:check` → `npm run typecheck` → `npm run lint` → `npm run test` |

> 一句话：把 chat / agent 域命令的失败通道从「纯字符串」升级为「带码、可重试标记、重试倒计时的结构化错误」，与流内 `ProviderErrorPayloadDto` 共用一套错误码枚举，并用编译期穷举表把前后端同步从「靠人记」变成「漏了编译不过」。

本文中的「必须 / 禁止」为可验收约束；「应当」为默认做法，偏离需在代码注释里写明理由。

---

## 0 遵循的原则来源

仓库**没有独立的 DTO 原则文档**，DTO 相关约束分散在四处，本文逐条对齐；执行时以这四处为准，本文只做 DTO 场景的落地展开，不复述原文。

| 来源 | 约束力 | 与本规格直接相关的条款 |
|---|---|---|
| `.trae/rules/project_rules.md`「类型安全规范」 | 强制（工作区规则） | 输出 DTO 的 `Option` 不加 `skip_serializing_if`（键恒在、值 `null`，前端声明 `T \| null` 必填）；输入 DTO 的 `Option` 加 `#[ts(optional = nullable)]` 且加 `deny_unknown_fields`；改 DTO 后必须 `npm run contract:gen` 并提交生成物；禁止手改生成类型；封闭集合用 `enum`；禁止下划线开头的 wire 字段 |
| `docs/principle/CODE-REVIEW-PRINCIPLE.md` **D2 类型与契约** | 评审判据（强证据） | 契约单一生成来源、生成物禁止手改；可选/未知字段与开放/封闭集合策略两端一致；跨进程调用必须类型化返回；**传输类型不得承载本地 UI 状态**；事件载荷结构化，**枚举字面量双份维护即为缺陷** |
| `docs/principle/CODE-REVIEW-PRINCIPLE.md` **D3 错误处理与可观测性** | 评审判据（强证据） | 失败必须归入「可恢复 / 需用户决策 / 应上报 / 可降级」之一；**在必然失败的类别上提供「重试」即为缺陷**；后端错误映射必须保留可判定信息；日志必须带上下文 |
| `docs/CODE-STRUCTURE.md` §4.3 协议层 / §契约命令 | 事实性基准 | `protocol/dto/` 按域拆分、`dto/mod.rs` 汇总导出并承载 ts-rs 导出；`npm run contract:gen` / `contract:check` 的生成物路径与校验范围 |

对应的落地位置：D2「枚举双份维护」→ §1.1 E-N5 与 §2.4；D2「不承载 UI 状态」→ §1.3 `details` 偏离记档；D3「必然失败不得给重试」→ §4.2 `Storage` 独立类别；D3「保留可判定信息」→ §2.3 禁止 `From<String>`、§4.1 未知码保留 `message`；project_rules「输出 Option 键恒在」→ §2.1 `retry_after_ms`。

---

## 1 现状与非目标

### 1.1 现状（以代码为准）

| # | 事实 | 落点 |
|---|---|---|
| E-N1 | 命令级错误只有文案，无码、无重试语义 | `src-tauri/src/cmd/chat/mod.rs::send_chat_message`、`cmd/agent/mod.rs::send_agent_message` 均返回 `Result<(), String>` |
| E-N2 | 流内错误已有完整结构：码 + 可重试 + 重试倒计时 + HTTP 元信息 | `src-tauri/src/protocol/dto/chat/event.rs::ProviderErrorPayloadDto` |
| E-N3 | 前端 `toChatError` 已同时接受 `string` 与结构化对象，字符串一律归一成 `InternalError` | `src/chat/core/chatErrors.ts::toChatError` |
| E-N4 | 字符串错误的来源集中且已知：`translate_agent_error` / `translate_storage_error` / `get_db_conn` / 各命令内联 `Err("…".to_string())` | `src-tauri/src/cmd/mod.rs` |
| E-N5 | 错误码枚举有两份手写副本：Rust 侧 `ProviderErrorCodeDto` 与前端 `ChatErrorCode`，靠 `satisfies Record<…>` 在**前端内部**兜底，但后端新增码时前端不会报错 | `event.rs`、`chatErrors.ts` |
| E-N6 | 存在未本地化的硬编码英文错误文案（如 `"Conversation is full, cannot send"`、`"no pending approval for call '…'"`） | `cmd/chat/insert.rs`、`cmd/agent/mod.rs` |

E-N5 是 C6 的隐性半场：只把命令错误结构化、不动枚举单源，等于给下一轮留下「后端加码、前端静默回落 `InternalError`」的新债。

### 1.2 目标

1. 定义通用 `CommandErrorDto`（码 / 文案 / 可重试 / 重试倒计时），作为**新命令与已迁命令**的唯一错误结构（未迁域仍返回字符串，见 §6.1 关闭判据）；
2. 错误码单源：Rust 侧一个枚举，前端从生成契约取类型，**删除前端手写枚举**；
3. 流内错误码成为命令错误码的子集（同一枚举），前端只需要一张分类表；
4. 首批迁移 chat / agent 六个命令及其内部 helper，其余域保持字符串不变（`toChatError` 兼容）；
5. 消除 E-N6 中落在首批范围内的硬编码英文文案；
6. 同步机制落到编译期：新增码时前端漏映射即 `npm run typecheck` 失败。

### 1.3 非目标

- 不迁移 `conversation` / `provider` / `storage` / `config` / `tool` / `mcp` / `orb` 等域命令（第二批及以后，见 §6）；
- 不重构 `assistant-storage` / `assistant-agent` 的错误类型：`StorageError` / `AgentError` 仍是既有定义，只在命令边界做结构化映射；
- 不做错误码到 UI 文案的后端本地化改造：`CommandErrorDto.message` 仍是后端已本地化的原文（沿用 `assistant_i18n::t`），前端只负责在码上做分类；
- **偏离原始修法（须记档）**：不引入 `details` 字段。原 C6 修法写的是 `CommandErrorDto`（`code` / `message` / `retryable` / `retryAfterMs` / `details`），本文去掉 `details`——依据 [CODE-REVIEW-PRINCIPLE](../principle/CODE-REVIEW-PRINCIPLE.md) D2「传输类型不得承载本地 UI 状态，本地扩展必须用交叉类型叠加」。前端 `ChatError.details` 由 `toChatError(extra)` 本地注入（`command` / `conversationId`）。偏离理由已在此写明，后续复核不要当作漏项补回；
- 不合并 `VaultErrorDto`：vault 的 36 个码是保险库域专用，与通用命令错误同形但码域不同，共用会让一个域的新码污染另一域的分类表。**这也是范围收缩**：vault 域不在 C6 的关闭判据内（见 §6.1）。

---

## 2 DTO 定义

### 2.1 位置与命名

新建 `src-tauri/src/protocol/dto/error.rs`，在 `protocol/dto/mod.rs` 注册 `pub mod error;` 并 `pub use error::*;`。同步更新 [CODE-STRUCTURE](../CODE-STRUCTURE.md) §4.3 的 `dto/` 目录清单（当前列了 `agent` `branch` `chat` `clock` `config` `mcp` `orb` `os_tool` `settings` `storage` `workspace`，需补 `error`）。

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum CommandErrorCodeDto {
    // —— provider 域（原 ProviderErrorCodeDto 全量，顺序与语义不变）——
    ApiKeyMissing,
    AuthFailed,
    RateLimited,
    ModelNotFound,
    ContextOverflow,
    NetworkError,
    Timeout,
    ServerError,
    RequestRejected,
    ParseError,
    Cancelled,
    StreamError,
    InternalError,
    // —— 命令域新增 ——
    SessionBusy,
    ConversationNotFound,
    ProviderNotFound,
    Validation,
    StorageFailure,
    NoPendingInteraction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct CommandErrorDto {
    pub code: CommandErrorCodeDto,
    pub message: String,
    pub retryable: bool,
    pub retry_after_ms: Option<u64>,
}
```

约束：

- **必须**保留原 `ProviderErrorCodeDto` 的 13 个变体名与拼写（wire 上是字符串，改名即破坏前端既有分类）；
- `retry_after_ms` **禁止**加 `skip_serializing_if`：输出侧键恒在、值 `null`（对齐 D2 结论与项目规则），前端声明 `number | null`；
- **禁止**加 `details` / `httpStatus` / `provider`：命令级失败发生在 provider 调用之前（`send_chat_message` 只做入参校验、建消息、`begin`、`spawn`），不存在 HTTP 元信息；
- `deny_unknown_fields` 只对输入有意义；本 DTO 是输出侧结构，**不加**（对齐 `VaultErrorDto`）。

### 2.2 可重试推导

`retryable` **必须**由码推导，**禁止**由调用点自由传入——否则同一个码在不同命令上语义不一致。

```rust
impl CommandErrorCodeDto {
    pub fn is_retryable(self) -> bool {
        matches!(
            self,
            Self::RateLimited
                | Self::NetworkError
                | Self::Timeout
                | Self::ServerError
                | Self::StreamError
                | Self::SessionBusy
        )
    }
}
```

`Cancelled` **禁止**标为可重试（用户主动停止）；`Validation` / `ConversationNotFound` / `ProviderNotFound` / `NoPendingInteraction` / `StorageFailure` 为不可重试（重试必然再次失败）。

### 2.3 构造与转换

```rust
impl CommandErrorDto {
    pub fn new(code: CommandErrorCodeDto, message: impl Into<String>) -> Self;
    pub fn internal(message: impl Into<String>) -> Self;          // code = InternalError
    pub fn with_retry_after_ms(mut self, ms: Option<u64>) -> Self;
}

impl From<ProviderError> for CommandErrorDto;   // code / retry_after_ms 取自 ProviderError
impl From<AgentError> for CommandErrorDto;      // 附件校验 → Validation；Other(_) → InternalError
impl From<StorageError> for CommandErrorDto;    // ConversationNotFound → 同名码；模式类 → Validation；Other(_) → StorageFailure
```

约束：

- **禁止**提供 `From<String> for CommandErrorDto`：隐式把任意字符串吞成 `InternalError` 会让「本该分类的失败」静默退化，也让调用点看不出自己在降级。需要降级处**必须**显式写 `CommandErrorDto::internal(msg)`；
- `From<AgentError>` **必须**承担原 `translate_agent_error` 的本地化职责（`chat.attachments_not_allowed` / `chat.too_many_images` / `chat.too_many_files` / `chat.image_too_large` / `chat.file_too_large`），文案与参数拼接逻辑不变；
- `From<StorageError>` **必须**承担原 `translate_storage_error` 的本地化职责（`conversation.not_found` / `agent_mode.builtin_cannot_delete` / `agent_mode.not_builtin_cannot_reset`）；
- 原 `translate_agent_error` / `translate_storage_error` **保留**（`conversation` 等域仍在用 `Result<_, String>`），**新增** `command_error_from_agent` / `command_error_from_storage` 供首批命令使用。禁止为省事把两个函数改成返回 `CommandErrorDto` —— 那会连锁改动 40+ 处未迁移命令。

### 2.4 流内错误复用同一枚举

`src-tauri/src/protocol/dto/chat/event.rs`：

- **删除** `ProviderErrorCodeDto` 枚举与 `From<ProviderErrorCode> for ProviderErrorCodeDto`；
- `ProviderErrorPayloadDto.code` 改为 `CommandErrorCodeDto`；
- `RetryNotificationDto.error_code` 改为 `Option<CommandErrorCodeDto>`；
- 新增 `From<ProviderErrorCode> for CommandErrorCodeDto`，13 个分支穷尽 `match`——`assistant-providers` 新增码时此 `match` 编译失败，不会静默漏映射。

这是「子集共享一个超集枚举」的单源做法：流内事件实际只产生 13 个 provider 码，类型上放宽到 19 个，前端本来就要兜底未知码，无行为损失。

---

## 3 首批迁移范围

### 3.1 纳入（六个命令 + 三个 helper）

| 命令 / helper | 当前位置 | 迁移后签名 | 失败点映射 |
|---|---|---|---|
| `send_chat_message` | `cmd/chat/mod.rs` | `Result<(), CommandErrorDto>` | 透传 `start_turn` |
| `cancel_chat` | `cmd/chat/mod.rs` | `Result<(), CommandErrorDto>` | 恒 `Ok` |
| `send_agent_message` | `cmd/agent/mod.rs` | `Result<(), CommandErrorDto>` | 透传 `start_turn` |
| `cancel_agent` | `cmd/agent/mod.rs` | `Result<(), CommandErrorDto>` | 恒 `Ok` |
| `resolve_agent_tool_call` | `cmd/agent/mod.rs` | `Result<(), CommandErrorDto>` | 无挂起点 → `NoPendingInteraction` |
| `resolve_agent_question` | `cmd/agent/mod.rs` | `Result<(), CommandErrorDto>` | 无挂起点 → `NoPendingInteraction` |
| `start_turn` | `cmd/agent/mod.rs`（`pub(crate)`） | `Result<(), CommandErrorDto>` | 见下表 |
| `validate_and_preprocess` | `cmd/agent/mod.rs` | `Result<String, CommandErrorDto>` | 空内容 → `Validation`；附件 → `From<AgentError>` |
| `insert_chat_messages` | `cmd/chat/insert.rs`（`pub(crate)`） | `Result<InsertedMessages, CommandErrorDto>` | 见 §3.3 |

`start_turn` 内失败点映射（**必须**逐点显式，禁止 `?` 隐式吞）：

| 失败点 | 码 | 文案来源 |
|---|---|---|
| `tasks.begin` 返回忙 | `SessionBusy` | `assistant_i18n::t("chat.session_busy")`（已有 key） |
| `get_db_conn` 锁失败 | `StorageFailure` | `assistant_i18n::t_with_args("error.db_lock", …)`（已有 key） |
| `validate_and_preprocess` 空内容 | `Validation` | `chat.empty_content`（已有 key） |
| `insert_chat_messages` 失败 | 透传 | 见 §3.3 |

### 3.2 排除（明确保留字符串，不做隐式兼容层）

- `conversation` / `provider` / `storage` / `tool` / `mcp` / `config` / `workspace` / `attachment` / `screenshot` / `orb` / `clock` / `compaction` / `permission` 全部命令；
- `import_attachment` 及其 `mapImportError` 的 `"file_too_large"` / `"too_large"` / `"invalid_file_type"` 字符串协议（前端按字面量判别，迁移需同时改前端判别方式，留给第二批）；
- `isCompactCancelled`（`src/services/chat/chatService.ts`）依赖 `"Compaction cancelled"` 字面量，属 `compaction` 域，不动。

理由：前端 `toChatError` 已兼容 `string`，未迁移命令的行为**完全不变**；一次性全量迁移会把 55 处签名、40+ 处 helper、前端 19 处字符串判别绑成一个大改动，回滚成本高。

### 3.3 `insert_chat_messages` 的处理

该函数是首批唯一的内部 helper 迁移点，内部 `?` 来源混杂（`StorageError` / `Result<_, String>` / 事务提交）。处理原则：

- 函数签名改 `Result<InsertedMessages, CommandErrorDto>`；
- 已知语义的失败点**必须**给码：会话已满 → `Validation`（文案新增 `chat.conversation_full`，替换硬编码 `"Conversation is full, cannot send"`）；会话/Provider 不存在 → `ConversationNotFound` / `ProviderNotFound`；
- 无法判别的透传（`get_conversation_detail` 的 `Result<_, String>`、事务提交 `e.to_string()`）→ 显式 `CommandErrorDto::internal(msg)`；
- **禁止**给 `CommandErrorDto` 加 `From<String>` 让 `?` 自动工作（理由见 §2.3）。

**可选优化（不纳入本次验收）**：把 `is_conversation_full` / `get_conversation_detail` / `get_provider_by_id` 三个纯判定前置到 `start_turn` 的 `begin` 之前——先校验再占 turn 槽，失败时不会产生「turn 已占、消息未建」的中间态。本次只做错误结构化，不做顺序调整，避免把行为变更混进契约改造。

---

## 4 前端改造

### 4.1 错误码改生成类型（删除手写枚举）

`src/chat/core/chatErrors.ts`：

```ts
import type { CommandErrorCodeDto } from "../../types/protocol.contract";

// 值必须等于键：mapped type 让「值写错」和「漏键/多键」同时编译失败
type SameKeys = { [K in CommandErrorCodeDto]: K };

export const ChatErrorCode: SameKeys = {
  ApiKeyMissing: "ApiKeyMissing",
  // …19 项
};

export type ChatErrorCode = CommandErrorCodeDto;
```

- `ChatErrorCode.X` 的既有引用点（`chatErrorRecovery.ts` 的 `ERROR_ICONS`、`turnOrchestrator.ts` 的 `ChatErrorCode` 导入）**无需改写**——`const` 对象保留了属性访问形状；
- `Object.values(ChatErrorCode).includes(...)` 改为 `Object.prototype.hasOwnProperty.call(ChatErrorCode, obj.code)`，避免依赖值集合；
- `CODE_TO_CATEGORY` 改为 `as const satisfies Record<CommandErrorCodeDto, ChatErrorCategory>`，键用 `[ChatErrorCode.X]`；后端新增码时前端漏映射 → `npm run typecheck` 失败。
- `isChatError` 对**未知码**必须宽松接受：只校验 `typeof obj.code === "string"` + `message` / `retryable` 的形状，不再要求码在枚举内；`inferCategory` 对未知码回落 `Unknown`，但 **`message` 必须原样保留**。现状是未知码直接判 `false`，落到 `toChatError` 最后一个分支换成 `translateKey("chat.error.unknown_error")`，把后端文案丢掉——违反 D3「后端错误映射必须保留可判定信息」。本仓前后端同版本打包，正常不会出现未知码；此条是为契约生成漏跑 / 手工改生成物时保留可诊断性。

### 4.2 新增分类 `Busy` / `Storage`

`ChatErrorCategory` 从 11 项增至 **13 项**，新增 `Busy = "busy"` 与 `Storage = "storage"`。码到类别映射（末列对齐 [CODE-REVIEW-PRINCIPLE](../principle/CODE-REVIEW-PRINCIPLE.md) D3 的四类语义）：

| 新码 | 类别 | D3 语义类 | 理由 |
|---|---|---|---|
| `SessionBusy` | `Busy`（新） | 可恢复 | 本地会话忙，等待即可，与 provider 侧限流语义不同，不该复用 `RateLimit` |
| `ConversationNotFound` | `InvalidRequest` | 需用户决策 | 请求指向的目标不存在，用户需新建对话或切回有效会话 |
| `ProviderNotFound` | `InvalidRequest` | 需用户决策 | 同上，指向 Provider 配置 |
| `Validation` | `InvalidRequest` | 需用户决策 | 入参/附件校验失败 |
| `StorageFailure` | `Storage`（新） | 应上报 | 本地存储故障，重试必然再次失败，用户唯一可做的是复制诊断上报 |
| `NoPendingInteraction` | `Cancelled` | 需用户决策 | 已决策/已超时，本次提交无效 |

`StorageFailure` **禁止**映射到 `Unknown`：`DEFAULT_ACTIONS[Unknown]` 含 `retry`，而 `DEFAULT_ACTIONS` 只按类别取动作、不读 `error.retryable`，映射过去会给不可重试的失败配「重试」按钮——直接违反 D3「在必然失败的类别上提供『重试』即为缺陷」。

`src/chat/core/chatErrorRecovery.ts` 三张穷举表**必须**同步补齐（`ERROR_ICONS` / `DEFAULT_ACTIONS` / `CATEGORY_INFO`），三处都是 `Record<ChatErrorCode, …>` / `Record<ChatErrorCategory, …>`，漏项即编译失败：

- `Busy` 的 `DEFAULT_ACTIONS`：`["wait_and_retry", "copy_diagnostics"]`；
- `Busy` 的 `CATEGORY_INFO`：`titleKey = "chat.error.SessionBusy"`、`messageKey = "chat.error.category_busy_message"`、`severity = "warning"`；
- `Storage` 的 `DEFAULT_ACTIONS`：`["copy_diagnostics"]`（**禁止**含 `retry` / `wait_and_retry`）；
- `Storage` 的 `CATEGORY_INFO`：`titleKey = "chat.error.StorageFailure"`、`messageKey = "chat.error.category_storage_message"`、`severity = "error"`。

### 4.3 `toChatError` 不动

`toChatError` 的双分支已经正确：`isChatError` 命中后读 `code` / `retryable` / `retryAfterMs`，命令错误与流内错误走同一条路径，`retryAfterMs` 直接进入 `formatRetryAfter` 的倒计时文案。**禁止**为本次改造在 `toChatError` 里新增「命令错误专用分支」——那会把两条通道重新拆开。

### 4.4 调用点回归排查（迁移任何命令前必做）

命令从 `Result<_, String>` 改为结构化后，所有 `typeof e === "string"` 的判别会从「命中」变成「不命中」。已知判别点：

| 位置 | 依赖的命令 | 是否受本次影响 |
|---|---|---|
| `src/stores/chat/chatConversationSlice.ts`（`setConversationMode` / `setConversationWorkspace`） | `patch_conversation` | 否（conversation 域未迁移） |
| `src/chat/attachment/factory.ts::mapImportError` | `import_attachment` | 否（未迁移） |
| `src/services/chat/chatService.ts::isCompactCancelled` | `compact_*` | 否（未迁移） |
| `src/screenshot/useScreenshotSession.ts` | `get_screenshot_session` | 否（未迁移） |
| `src/services/stream/turnOrchestrator.ts` | `send_chat_message` / `send_agent_message` | **是**——已走 `toChatError`，无需改，但**必须**回归验证错误卡片能显示码与倒计时 |

**必须**写进后续批次的实施检查单：迁移某命令前，先检索该命令调用点是否存在 `typeof e === "string"` / `String(e)` 判别，有则同步改为 `isChatError` 或 `toChatError`。

---

## 5 文案与 i18n

### 5.1 前端（`src/i18n/locales/zh-CN.json` / `en-US.json`）

新增 `chat.error` 下键：

| key | 用途 |
|---|---|
| `SessionBusy` | `Busy` 类别标题 |
| `category_busy_message` | `Busy` 类别建议文案 |
| `ConversationNotFound` | 会话不存在标题 |
| `ProviderNotFound` | Provider 不存在标题 |
| `Validation` | 校验失败标题 |
| `StorageFailure` | 存储故障标题 |
| `category_storage_message` | `Storage` 类别建议文案（只提示复制诊断，不提示重试） |
| `NoPendingInteraction` | 提交无效标题 |

### 5.2 后端（`src-tauri/crates/assistant-i18n/locales/zh-CN.json` / `en-US.json`）

新增：

| key | 用途 |
|---|---|
| `chat.conversation_full` | 替换 `insert_chat_messages` 的硬编码 `"Conversation is full, cannot send"` |
| `agent.no_pending_interaction` | 替换 `resolve_agent_tool_call` 的 `"no pending approval for call '…'"` 与 `resolve_agent_question` 的透传英文文案 |

约束：

- 两个语言文件**必须**同时补齐，缺键会导致运行期回落 key 字符串；
- `CommandErrorDto.message` 是「后端已本地化原文」，前端 `ErrorBlock` 直接展示，**禁止**前端再按码二次翻译 `message`（码只用于分类与动作）。

---

## 6 迁移批次

| 批次 | 内容 | 前置 | 验收 |
|---|---|---|---|
| B0 | 新增 `CommandErrorDto` / `CommandErrorCodeDto`，删除 `ProviderErrorCodeDto`，`event.rs` 换枚举，注册生成 | — | `cargo test -p assistant-app --test protocol_dto_roundtrip`、`npm run contract:check` |
| B1 | 前端 `ChatErrorCode` 改生成类型 + `Busy` / `Storage` 类别 + 三张穷举表 + i18n | B0 | `npm run typecheck`、`npm run test` |
| B2 | chat / agent 六个命令 + `start_turn` / `validate_and_preprocess` / `insert_chat_messages` 改结构化 | B0 | `cargo test -p assistant-app`、`npm run typecheck` |
| B3 | 人工回归：断网 / 触发会话忙 / 提交过期审批卡片 | B2 | 见 §7 |
| 第二批（另立项） | `conversation` 域（`patch_conversation` 等）| B2 完成 | 迁移前先按 §4.4 排查调用点 |
| 第三批（另立项） | `provider` / `storage` / `mcp` / `attachment` | 第二批完成 | 同上 |

B0 与 B1 必须同一次提交：B0 删除 `ProviderErrorCodeDto` 后前端生成契约里该名字消失，前端 `ChatErrorCode` 若不改会立刻编译失败——这正是期望的 fail-loud。

### 6.1 关闭判据（首批完成 ≠ C6 关闭）

原 C6 的债是「chat / conversation / provider / storage / config 域命令**全部**是纯字符串错误」。本文首批只覆盖 chat / agent 的六个命令，**其余域一条未动**，因此：

- **B0–B3 完成只能把 C6 标记为「部分关闭 / 首批已迁」**，不得在 compat-tech-debt-audit-1（已归档） 中把 C6 划入「已关闭」；
- C6 **完全关闭**的判据是：`src-tauri/src/cmd` 下 `Result<[^>]*, String>` 的匹配数降为 **0**（vault 域已是 `VaultErrorDto`，不计入）。复核命令（在 `src-tauri` 目录执行）：

```
rg -c --stats "Result<[^>]*, String>" src/cmd
```

- 每批迁移完成后**必须**把该命令的实测匹配数记入本文件（口径与审计文档 §1 一致：按匹配数计，不用 `grep -c` 按行数比对），否则「减少」无法证伪；
- **首批（B2）实测记录**：迁移前 `src/cmd` 共 **103** 匹配（28 个文件），其中 `agent/mod.rs` 6、`chat/insert.rs` 3、`chat/mod.rs` 2；迁移后 **92** 匹配（25 个文件），上述三个文件降为 0，降幅 11 与逐文件计数一致。剩余 92 全部位于未迁域（conversation / provider / storage / config / tool / mcp / …）；
- 未迁域在关闭判据达成前，**禁止**把 `CommandErrorDto` 描述成「所有命令级失败的唯一结构」对外表述——当前它只是「新命令与已迁命令的唯一结构」，§1.2 第 1 条的目标措辞按此理解。

### 6.2 原始债的继承对照

| 原 C6 债项 | 本文处理 | 是否在首批内 |
|---|---|---|
| chat 域命令字符串错误 | 六个命令改结构化 | 是 |
| agent 域命令字符串错误 | 同上（原 C6 标题只写 chat，实际 `cmd/agent` 同样全部是字符串，一并纳入） | 是 |
| conversation 域命令字符串错误 | 第二批 | 否 |
| provider 域命令字符串错误 | 第三批 | 否 |
| storage / config / tool / mcp / orb / attachment / screenshot / clock / compaction / permission / workspace 域 | 第三批及以后 | 否 |
| 流内 `ProviderErrorCodeDto` 与前端 `ChatErrorCode` 双份副本 | 删 Rust 侧枚举，前端改生成类型 | 是（B0 + B1） |
| `VaultErrorDto` 与通用结构并存 | 有意保留，理由见 §1.3 | 否（不列为债） |

---

## 7 验证清单

在仓库根目录执行（cargo 相关在 `src-tauri` 目录执行）：

1. `npm run contract:gen` —— 生成 `src/types/protocol.contract.ts`，确认 `CommandErrorDto` / `CommandErrorCodeDto` 出现、`ProviderErrorCodeDto` 消失；
2. `cargo test -p assistant-app --test protocol_dto_roundtrip` —— 新增用例：
   - `CommandErrorDto` 往返（含 `retry_after_ms: None`）；
   - 输出侧 `"retryAfterMs":null` 键恒在（对齐 `output_optional_fields_are_present_as_null`）；
   - `is_retryable` 逐码断言（可重试 6 个、不可重试 13 个）；
   - `From<ProviderError>` 的 `RateLimited { retry_after_ms: Some(n) }` 保留倒计时；
3. `cargo test -p assistant-app --test protocol_bindings` —— 声明数组注册新 DTO、删除 `ProviderErrorCodeDto`，`missing` / `stale` 断言为空调；
4. `npm run contract:check` —— 生成契约与源码一致、无遗漏提交；
5. `npm run typecheck` —— 前端 `ChatErrorCode` / `CODE_TO_CATEGORY` / `ERROR_ICONS` / `DEFAULT_ACTIONS` / `CATEGORY_INFO` 全部穷举通过；
6. `npm run lint`；
7. `npm run test` —— 既有 326 项不回归；
8. 人工回归：
   - 断网后发送 → 错误卡片显示 `NetworkError` 且「重试」按钮可用；
   - 生成中再次发送 → 显示「会话正在处理中」+「等待并重试」，不再是泛化的内部错误；
   - 点击已过期的审批卡片 → 显示「提交无效」而非英文原文；
   - 空内容发送 → 显示「内容为空」且归类为请求无效（无重试按钮）；
   - 制造本地存储失败（如临时锁住库文件）→ 显示「存储故障」，动作只有「复制诊断」，**不出现重试按钮**（验证 `Storage` 类别未复用 `Unknown` 的动作表）。
9. **日志可观测性**（D3）：确认命令失败时 `tauriInvoke` 的既有日志仍带 `command` 与参数键名，且结构化错误不改变日志形态；禁止为本次改造新增裸 `console.*`。
10. **债量复核**（§6.1）：在 `src-tauri` 下跑 `rg -c --stats "Result<[^>]*, String>" src/cmd`，把匹配数与迁移前对比并记入 §6.1；首批完成后匹配数**必须下降但不为 0**——若为 0，说明批次范围写错（未迁域被误改）；若未下降，说明改的是 helper 而非 `#[command]` 签名。

---

## 8 风险与约束

| # | 风险 | 处置 |
|---|---|---|
| R1 | 命令签名变更后前端某处 `typeof e === "string"` 判别失效，错误详情丢失 | §4.4 检查单；首批仅 6 个命令且调用点已走 `toChatError` |
| R2 | 删除 `ProviderErrorCodeDto` 破坏既有生成契约名 | 一次提交内完成 B0+B1；`protocol_bindings.rs` 的 `missing`/`stale` 断言会直接报出漏改 |
| R3 | `SessionBusy` 可重试但与 `RateLimited` 语义混淆 | 独立 `Busy` 类别，标题/文案独立，不复用 `RateLimited` 的 key |
| R4 | 未迁移域仍返回字符串，两条通道并存 | 有意保留（§3.2）；`toChatError` 是唯一归一入口，并存不产生分支逻辑 |
| R5 | `CommandErrorDto` 加 `details` 会承载 UI 状态 | 明文禁止（§1.3），诊断信息继续由 `toChatError(extra)` 注入 |
| R6 | 护栏靠人执行（仓库无 CI） | 新增用例写进 `cargo test` / `vitest`；`contract:check` 与其余命令列入提交前动作，理由同 compat-tech-debt-audit-1（已归档） §5.1 |
| R7 | `StorageFailure` 若映射到 `Unknown`，会因 `DEFAULT_ACTIONS[Unknown]` 含 `retry` 给不可重试失败配重试按钮（违反 D3） | 独立 `Storage` 类别，动作仅 `copy_diagnostics`（§4.2）；§7 第 8 条加人工回归 |

---

## 9 定位锚点

行号会漂移，优先按以下锚点检索：

| 主题 | 检索词 |
|---|---|
| DTO 定义 | `CommandErrorDto`、`CommandErrorCodeDto`、`is_retryable`、`with_retry_after_ms` |
| 流内复用 | `ProviderErrorPayloadDto`、`RetryNotificationDto`、`From<ProviderErrorCode> for CommandErrorCodeDto` |
| 命令迁移 | `send_chat_message`、`send_agent_message`、`start_turn`、`validate_and_preprocess`、`insert_chat_messages` |
| 前端单源 | `SameKeys`、`ChatErrorCode`、`CODE_TO_CATEGORY`、`ChatErrorCategory::Busy`、`ChatErrorCategory::Storage` |
| 映射函数 | `command_error_from_agent`、`command_error_from_storage`、`translate_agent_error`、`translate_storage_error` |
| 验证 | `protocol_dto_roundtrip`、`protocol_bindings`、`contract:check` |
| 关闭判据 | `Result<[^>]*, String>`、`rg -c --stats`、§6.1、§6.2 |
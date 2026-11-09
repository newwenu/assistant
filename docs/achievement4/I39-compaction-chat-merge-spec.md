# I39 压缩与 chat 的共享边界规格

| 项 | 值 |
|---|---|
| 状态 | 定稿（2026-11-05 修订：取消「完全复用」方针） |
| 日期 | 2026-11-05 |
| 归属阶段 | P3 收尾（运行时内核等价性） |
| 关联 | [I36 运行时规格](./I36-agent-runtime-spec.md) Step 6 / Step 7、[I36a 内核基线](../store/I36a-agent-kernel-baseline.md)、[I38 阶段总纲](../store/I38-agent-roadmap.md) R12 |
| 影响面 | `assistant-agent` 内核（`compaction.rs` / `kernel::request`）、`src-tauri` 压缩执行体与事件适配器、前端 chat store 与事件订阅 |

## 0 定位

本文**不是**第三份压缩规格，只回答一个此前无人负责的横切问题：**压缩与 chat 应该共享到什么程度**。

**结论（修订后）**：压缩**不**收编为 `TurnRunner` 的第三种 `TurnSpec`，也**不**追求与 chat 逐字节等价复用。只共享「已经证明同构且共享有收益」的积木，**保留压缩自己的编排、落库口径与超时口径**。

| 主题 | 归属 |
|---|---|
| 压缩生命周期（事件、hook、trigger 的语义） | I36 spec Step 6（本文不改） |
| 「终止式 → 续跑式」的切换点 | P4（I38b，本文不改） |
| 压缩与 chat 共享哪些代码、保留哪些差异 | **本文**（roadmap R12） |

> 修订说明：初稿把压缩写成「第三种 `TurnSpec`，差异仅由 `output` / `profile` / `commit_summary` 表达」。落地时确认该方案要为 3 个新维度付出改造成本，而收益集中在「删除 `TERMINATING` 静态表」「统一刷新点」「统一失败通道」三项。因此改为**分层共享**：能直接删重复代码的就共享，需要新增抽象层的就不做。

---

## 1 改造前盘点

> 本节记录**改造前**的两条路径形态，用于说明「哪些差异是冗余的」；改造后的目标形态见 §2 与 §6。

### 1.1 差异判定

| 层 | chat 主流程 | 压缩流程 | 判定 |
|---|---|---|---|
| 互斥源 | `TurnTasks::begin(Replace)` | `begin(Replace)` **+ 额外静态表 `compaction::TERMINATING`** | **冗余**（两个互斥源，且与 `TurnTasks` 判定不一致） |
| 流消费 | `stream::consume` + `StreamError` | 同一函数 | **已共享**，无需改动 |
| 事件发射 | `TurnEmitter`（seq 单调 + 代际校验） | 同一类型 | **已共享**，无需改动 |
| usage 构造 | `PersistCtx::usage_records` | 同一函数 | **已共享**，无需改动 |
| 错误载荷 | `ProviderErrorPayload` | 同一类型 | **已共享**，无需改动 |
| 编排 | `TurnRunner::run`（drive / 迭代 / 阶梯 / 状态机） | `execute_compaction` 手写单次调用 | **必要差异**：压缩无迭代、无工具、无状态机快照 |
| 超时 | provider 层 ttft/idle/total | 硬编码 `Duration::from_secs(30)` | **必要差异**：摘要生成口径固定，不引入配置面 |
| 投影 | `project_chat_messages`（附件投影 + 超预算降级） | `prompt_builder::prepare_messages`（**无投影、无降级**） | **冗余（且是缺陷）**：历史含附件且超窗时旧路径会把超限请求直接发给 provider |
| 成功刷新 | `emit_stream_finished` → `chat-messages-changed(stream_finished)` | `emit_stream_finished` **再手写一遍** `chat-messages-changed(messages_compacted)` | **冗余**：同一次成功发两次刷新，前端 token 用量被刷新两遍 |
| 失败通知 | `chat-delta.error` + `session-state-changed(Error)` | 同一对事件 **+ 额外 `conversation-compact-failed`** | **冗余**：失败态两套通道 |
| 落库 | `TurnPersister`（内容 / 状态 / usage） | `save_compaction_partial` + 手写事务 | **必要差异**：摘要行 `tokens` 必须为 `NULL`、成功收尾要改会话级状态 |
| 成功收尾 | `finish_turn` → `emitter.terminated` | 手写事务（`is_full` / `active_branch_id` / `compression_state`） | **必要差异**：会话级状态是压缩独有 |
| 前端状态 | `chatStreamStore` + `chatConversationSlice` | `chatCompactSlice` 的 `compactError` / `compactSummary` | **冗余**：失败已由消息 `error` 承载，`compactSummary` 无任何消费方 |
| 前端渲染 | `MessageBubble` + `ErrorBlock` | `CompactErrorBanner` 独立横幅 | **冗余**：摘要消息同样渲染 `ErrorBlock` |

### 1.2 必须保留的差异（不合并）

| 差异 | 理由 |
|---|---|
| 独立编排 `execute_compaction` | 压缩是后台单次调用，不需要迭代、工具轮、phase 快照与 `OnBusy` 状态机；强行塞进 `TurnRunner` 需新增 `output` / `profile` / `commit_summary` 三个维度，成本高于收益 |
| 单次调用 30s 上限 | 摘要生成是「短任务」，口径固定；不新增 `AppConf` 字段，避免 `contract:gen` 与配置迁移 |
| 摘要行 `tokens = NULL` | 摘要的 completion tokens 不是上下文占用，历史详情页显示会误导 |
| 摘要截断（`MAX_SUMMARY_OUTPUT_TOKENS * 4` + 截断文案） | 压缩独有的输出长度约束 |
| `phase = Compacting` | 语义明确，前端暂未消费，保留可读性 |
| 成功收尾单事务（`is_full` / `active_branch_id` / `compression_state`） | 会话级状态是压缩独有 |
| `conversation-terminated` 事件（`isFull`） | 压缩独有状态位通知，与 `chat-messages-changed` 的职责不同 |
| 消息 `role = compact_summary` 与 `MessageBubble` 只读渲染分支 | 摘要是独立语义的行，不能伪装成 assistant 消息 |
| `OnBusy::Replace` 的既有语义 | 压缩与 chat 共用「后发顶替」语义，压缩期间用户发送消息按「发送优先」处理 |

---

## 2 共享边界

### 2.1 共享（改动即删重复代码）

| # | 共享内容 | 收益 |
|---|---|---|
| S1 | `TurnTasks` 作为**唯一**互斥源 | 删 `compaction::TERMINATING` 与 `is_terminating` / `set_terminating`，消除两套并发判定 |
| S2 | `stream::consume` / `TurnEmitter` / `PersistCtx::usage_records` / `ProviderErrorPayload` | 现状已共享，明确为契约，禁止在压缩侧另起实现 |
| S3 | `request::project_chat_messages`（附件投影 + 超预算降级） | 修复压缩请求不做投影的缺陷，与 chat 同源 |
| S4 | `TauriEventSink::emit_stream_finished`（参数化 `change_kind`） | 删压缩成功后的手写刷新，一次成功只发一次刷新 |
| S5 | `chat-delta.error` + `session-state-changed(Error)` 失败通道 | 删 `conversation-compact-failed` 与前端 `compactError` |

### 2.2 保留独立（不做统一抽象）

见 §1.2。核心原则：**压缩保留自己的编排函数**，只把其中重复的积木替换为共享版本。

### 2.3 明确不做

初稿曾计划的以下改造**全部撤销**：

| 撤销项 | 原因 |
|---|---|
| `TurnOutput` 枚举 + `TurnSpec.output` 字段 | 只服务压缩，chat / agent 恒为 `Assistant`，属纯新增维度 |
| `TurnSpec::compaction()` 构造器 | 压缩不进 `TurnRunner` |
| `TurnLimits::compaction()` | 同上；30s 常量直接用于 `stream::consume` |
| `RequestProfile` + `TurnInput.profile` | 压缩请求形态只有一个调用点，`project_chat_messages` 增加一个 `append_user_prompt` 参数即可覆盖 |
| `CompactionSink::commit_summary` | 摘要提交留在 `execute_compaction` 成功分支，不需要端口回调；宿主侧可拆出本地辅助函数 `commit_summary`，但不构成内核端口 |
| `CreateAssistantReq.role` | 压缩摘要行由宿主直接 `insert_message`；且 `create_assistant_message` 当前无生产调用者，不应为它扩字段 |
| `AppConf.compact_summary_timeout_secs` | 不引入配置面，避免 `assistant-config` + DTO + `contract:gen` 连锁改动 |

---

## 3 后端规格

### 3.1 互斥源统一（S1）

`src-tauri/crates/assistant-agent/src/compaction.rs`：

- 删除 `TERMINATING` 静态表、`is_terminating`、`set_terminating`。
- 保留 `default_compact_summary_prompt` / `COMPACTION_SAFETY_TOKENS` / `CompactDecision` / `evaluate_compaction`。

`src-tauri/src/cmd/agent/mod.rs`：`validate_and_preprocess` 中删掉 `compaction::is_terminating(conversation_id)` 检查。准入判定改由 `TurnTasks` 承担：

- chat / agent：`begin(Replace)` —— 后发顶替，压缩期间用户发送即取消压缩。
- 压缩：`manual_compact_conversation` 用 `tasks.current(&conversation_id).is_some()` 拒绝（现有行为，保留）。

`src-tauri/src/cmd/compaction.rs`：删掉全部 `set_terminating` 调用点（入口处 1 处置 `true`、异步块 1 处、`execute_compaction` 各终态分支 4 处），并删除对 `is_terminating` / `set_terminating` 的 import。

### 3.2 投影复用（S3）

`src-tauri/crates/assistant-agent/src/kernel/request.rs`：

```rust
pub async fn project_chat_messages(
    detail: &ConversationDetail,
    system_prompt: &str,
    provider_row: &ProviderRow,
    model_id: &str,
    attachment_store: &AttachmentStore,
    offload_budget: Option<usize>,
    purpose: ProjectionPurpose,
    append_user_prompt: Option<&str>,
) -> Vec<types::ChatMessage>
```

新增参数透传给 `prompt_builder::prepare_messages(detail, system_prompt, append_user_prompt)`；`build` 与 `estimate_branch_tokens` 传 `None`，行为不变。

`execute_compaction` 改为：

```rust
let context_window_tokens =
    types::resolve_context_window_from_stored(&model_id, &provider_row.models_json);
let offload_budget = request::offload_budget_of(context_window_tokens, max_tokens);
let chat_messages = request::project_chat_messages(
    detail,
    &system_prompt,
    provider_row,
    &model,
    &attachment_store,
    offload_budget,
    ProjectionPurpose::Send,
    Some(summary_prompt),
)
.await;
```

**唯一有意变更的行为**：压缩请求开始做附件投影与超预算降级。旧路径用 `prepare_messages`，历史含图片/文件且超窗时会把超限请求原样发出；新路径与 chat 一致，属修正而非漂移。

### 3.3 成功刷新复用（S4）

`src-tauri/src/adapters/event_sink.rs`：

```rust
impl TauriEventSink {
    /// 终态后的列表刷新；`change_kind` 由调用方给出。
    pub(crate) fn emit_stream_finished(&self, change_kind: MessageChangeKindDto) { ... }
}
```

- `TurnEvent::Terminated` 分支传 `MessageChangeKindDto::StreamFinished`。
- `execute_compaction` 成功分支传 `MessageChangeKindDto::MessagesCompacted`，并**删除**其后手写的 `chat-messages-changed` 块。

理由：前端对 `stream_finished` 跳过 token 用量刷新，对 `messages_compacted` 会刷新（`chatLifecycleEvents.ts`）。压缩消耗真实 token，必须走后者；且同一次成功只允许一次刷新。

### 3.4 失败通道统一（S5）

`execute_compaction` 失败分支（`Cancelled` / `Timeout` / `Provider` / 成功事务提交失败）：

1. 保存部分结果与 usage（`save_compaction_partial`，保留）。
2. `emitter.terminated(status, error)` —— 已自动发 `chat-delta.error` 与 `session-state-changed(Error)`。
3. **删除** `conversation-compact-failed` 广播。

事务提交失败（`commit_summary` 返回 `Err`）无 provider 错误可复用，终态错误载荷取 `ProviderError::RequestFailed("compaction commit failed")`，其余分支保持 provider 原生错误。

`emitter.terminated` 必须在 `tasks.finish` 之前调用（现有序，不变），否则事件被代际守卫丢弃。

### 3.5 保留的独立实现

以下保持现状，**不得**为了统一而改写：

| 项 | 保留实现 |
|---|---|
| 编排 | `execute_compaction` 自己 `begin` → 建摘要行 → `stream_start(Compacting)` → `stream::consume(30s)` → 终态分支 → `terminated` → `tasks.finish` |
| 超时 | `Some(limits::DEFAULT_COMPACTION_MODEL_CALL_TIMEOUT)`（30s，取值不变；禁止散落字面量） |
| 摘要行 | `role = compact_summary`、`tokens = NULL`、`parent_id = 最后一条消息`、`sibling_index = 0` |
| 部分结果 | `save_compaction_partial`：usage 照写，`has_effective_output == false` 时不写内容 |
| 成功收尾 | 单事务：`update_message_content`（含截断）→ `set_conversation_is_full(true)` → `update_active_branch_id` → `upsert_compression_state` |
| 成功事件 | `conversation-terminated { conversationId, isFull }` |

### 3.6 删除清单

| 位置 | 删除内容 |
|---|---|
| `crates/assistant-agent/src/compaction.rs` | `TERMINATING` / `is_terminating` / `set_terminating` |
| `src-tauri/src/cmd/compaction.rs` | `set_terminating` 调用；成功后手写 `chat-messages-changed` 块；`conversation-compact-failed` 广播 |
| `src-tauri/src/cmd/agent/mod.rs` | `validate_and_preprocess` 内的 `is_terminating` 检查 |
| `src-tauri/src/adapters/compaction_sink.rs` | `compaction::set_terminating` 调用与失败分支的 `CONVERSATION_COMPACT_FAILED` 广播 |
| `src-tauri/src/protocol/dto/chat.rs` | `ConversationCompactFailedPayloadDto` |
| `src-tauri/src/protocol/events.rs` | `CONVERSATION_COMPACT_FAILED` |
| `src/chat/compact/CompactErrorBanner.tsx` + `.module.css` | 整个文件 |

### 3.7 已落地改动的回退清单

初稿方案在 `assistant-agent` 内核中已落地以下改动，**必须回退**（保留的部分已在 §3.2 注明）：

| 文件 | 回退内容 | 保留 |
|---|---|---|
| `kernel/mod.rs` | `TurnOutput` 枚举、`TurnSpec.output` 字段、`TurnSpec::compaction()`、`RequestProfile` 再导出 | — |
| `kernel/limits.rs` | `TurnLimits::compaction()` | `DEFAULT_COMPACTION_MODEL_CALL_TIMEOUT` 常量（供 §3.5 使用） |
| `kernel/ports.rs` | `RequestProfile` 结构、`TurnInput.profile` 字段、`CompactionSink::commit_summary`、`ProjectionPurpose` import | — |
| `kernel/request.rs` | `build` 内对 `profile` 的消费（system prompt / max_tokens / temperature / purpose 覆盖） | `project_chat_messages` 的 `append_user_prompt` 参数（§3.2） |
| `kernel/runner.rs` | `commit_summary` 调用点与 `commit_summary` 方法、`TurnOutput` import | — |

---

## 4 前端规格

### 4.1 保留

| 项 | 说明 |
|---|---|
| `chatCompactSlice.manualCompact` | 乐观插入临时 `compact_summary` 消息 + `beginStream(tempId)`；`invoke` 抛错（已满 / 正忙）时回滚临时消息并 `notifyError`（准入失败，turn 未开始） |
| `chatCompactSlice.isFull` / `unlockConversation` | 不变 |
| `CompactBanner` | `isFull` 提示与「解压」入口，不变；「新建对话」恒传 `builtin-default`（不继承当前会话 `modeId`），见 §4.3 |
| `MessageBubble` 的 `compact_summary` 只读分支 | 不变（含 `compactSummaryHeader`）；`ErrorBlock` 分支在它之前，失败时照常渲染 |
| `chatStreamStore` / `chatStorePorts` | 不改 |

### 4.2 退役

| 位置 | 变更 |
|---|---|
| `src/stores/chatCompactSlice.ts` | 删 `compactSummary` / `compactError` / `clearCompactError` |
| `src/stores/chatConversationSlice.ts` | 删接口字段 `compactSummary` / `compactError` 与切会话重置项 |
| `src/services/chatLifecycleEvents.ts` | 删 `ConversationCompactFailed` 监听；`ConversationTerminated` 只写 `isFull`；`ConversationUnlocked` 只清 `isFull` |
| `src/chat/compact/CompactErrorBanner.tsx` + `.module.css` | 删除文件 |
| `src/chat/ChatView.tsx` | 删 `CompactErrorBanner` 的 import 与渲染 |

失败呈现路径统一为：`chat-delta.error` → 摘要消息 `error` 字段 → `MessageBubble` 的 `ErrorBlock`。

### 4.3 压缩不继承会话模式

压缩请求与「压缩横幅里的新建对话」是两件不同的事，二者都不继承会话 `modeId`：

| 项 | 判据 | 理由 |
|---|---|---|
| 压缩请求本身 | `execute_compaction` 直接构造 `ChatOptions { tools: None, temperature: 0.3 }` 调 provider 适配器 | 压缩是单次摘要调用，无迭代、无工具轮、无 `TurnSpec`（§1.2）；会话模式对摘要生成没有任何影响面，继承只会把 `toolConfig` 等无关状态拖进后台任务 |
| 横幅的「新建对话」 | 恒传 `builtin-default` | 该按钮不是用户显式选模式，只是一个空白新会话；按 I38d §4.8「新建会话 `modeId = builtin-default`」处理。切到 agent 由共同视图的 `ModeSelector` 承担（I38d §4.10），不需要在压缩入口隐式带过链路 |

---

## 5 事件与 DTO

### 5.1 事件表

| 事件 | 变更 |
|---|---|
| `chat-stream-start` | 不变（压缩传 `phase = compacting`） |
| `chat-delta` | 不变（压缩失败走 `error` 字段） |
| `chat-user-message-confirmed` | 压缩不发送（无 user 消息） |
| `session-state-changed` | 不变 |
| `chat-messages-changed` | 压缩成功改为**只发一次**，`change_kind = messages_compacted`（§3.3） |
| `conversation-list-changed` | 不变 |
| `conversation-terminated` | 载荷去掉 `summary`，仅 `{ conversationId, isFull }` |
| `conversation-compact-failed` | **删除** |
| `conversation-unlocked` | 不变 |

### 5.2 DTO 变更

```rust
pub struct ConversationTerminatedPayloadDto {
    pub conversation_id: String,
    pub is_full: bool,
}
// 删除 ConversationCompactFailedPayloadDto
```

变更后执行 `npm run contract:gen`，提交 `src/types/protocol.contract.ts`。

### 5.3 成功路径顺序契约

```
stream::consume 成功
  → 单事务提交（截断 → update_message_content → is_full → active_branch_id → compression_state）
    → emitter.terminated(Completed)（内部：chat-messages-changed(messages_compacted) + conversation-list-changed）
      → conversation-terminated(is_full)
        → tasks.finish
```

约束：

1. 事务提交必须在 `emitter.terminated` 之前，否则前端刷新拿到未提交的 `active_branch_id`。
2. `conversation-terminated` 在 `chat-messages-changed` 之后发，避免横幅先于摘要出现。
3. 事务失败 → 不置 `is_full`、不发 `conversation-terminated`，走 §3.4 失败通道。
4. `tasks.finish` 必须在所有事件之后，否则代际守卫会丢弃事件。

---

## 6 语义对齐表

| 维度 | chat（现状） | 压缩（本规格后） | 关系 |
|---|---|---|---|
| 互斥源 | `TurnTasks` | `TurnTasks`（`TERMINATING` 已删） | 共享 |
| 流消费 / 超时判定 | `stream::consume` | 同 | 共享 |
| 事件发射 | `TurnEmitter` | 同 | 共享 |
| usage 构造 | `PersistCtx::usage_records` | 同 | 共享 |
| 错误载荷 | `ProviderErrorPayload` | 同 | 共享 |
| 编排 | `TurnRunner::run` | `execute_compaction` | **独立** |
| 迭代 / 工具 / phase 快照 | 1 / 无 / 有 | 单次 / 无 / 仅 `Compacting` | **独立** |
| 超时 | provider ttft/idle/total | 单调用 30s | **独立** |
| 投影 | `project_chat_messages` | 同（新增 `append_user_prompt`） | 共享 |
| 落库 | `TurnPersister` | `save_compaction_partial` + 单事务 | **独立**（`tokens = NULL`） |
| 成功刷新 | `emit_stream_finished(StreamFinished)` | `emit_stream_finished(MessagesCompacted)` | 共享（参数化） |
| 失败通道 | `chat-delta.error` + session-state | 同 | 共享 |
| 前端失败呈现 | `ErrorBlock` | `ErrorBlock` | 共享 |

---

## 7 实施步骤

| 步骤 | 内容 | 验证 |
|---|---|---|
| T0 | 按 §3.7 回退内核初稿改动，`cargo test -p assistant-agent` 恢复全绿 | 工作区无 `TurnOutput` / `RequestProfile` / `commit_summary` 残留 |
| T1 | §3.1 互斥源统一：删 `TERMINATING`，删 `validate_and_preprocess` 检查，删 `execute_compaction` 三处 `set_terminating` | `rg "is_terminating\|set_terminating\|TERMINATING"` 无命中 |
| T2 | §3.2 投影复用：`project_chat_messages` 增参，`execute_compaction` 改调；`build` / `estimate_branch_tokens` 传 `None` | `cargo test -p assistant-agent`（`attachment_budget` 全绿） |
| T3 | §3.3 刷新复用：`emit_stream_finished` 参数化，删压缩手写刷新块 | 手动：压缩成功仅一次 `chat-messages-changed` |
| T4 | §3.4 失败通道：删 `conversation-compact-failed` 广播；DTO / 事件常量删除；`npm run contract:gen` | 手动：超时 / 取消 / provider 错误三条路径只有 `chat-delta.error` |
| T5 | §3.5 常量替换（`DEFAULT_COMPACTION_MODEL_CALL_TIMEOUT`），`conversation-terminated` 去 `summary` | `cargo clippy`；`cargo test -p assistant-app`（`assembly_contract` / `protocol_contract`） |
| T6 | §4 前端收敛：删 `compactSummary` / `compactError` / `clearCompactError` / `CompactErrorBanner` / 监听 | `npm run lint`、`npm run typecheck`、前端单测 |
| T7 | 文档：I36 spec Step 6 补一句「共享边界见 I39」；I38 R12 与文档地图同步修订 | — |

---

## 8 验收标准

1. 全仓 `rg "is_terminating|set_terminating|TERMINATING|COMPACT_FAILED|CompactErrorBanner|compactError|compactSummary"` 无业务命中（允许的存量同名：配置键 `compactSummaryPrompt` / `compactSummaryMaxTokens`、样式类 `compactSummaryHeader`）。
2. 压缩成功：`is_full = true`、`active_branch_id` 指向摘要、`compression_count + 1`；前端只收到**一次** `chat-messages-changed`，且 `change_kind = messages_compacted`。
3. 压缩失败（超时 / provider 错误 / 事务提交失败）：只收到 `chat-delta.error` + `session-state-changed(Error)`；摘要消息带 `error`，`ErrorBlock` 可见；`is_full` 不变。（空响应不属失败，见 D4）
4. 压缩取消：摘要消息 `interrupted`、`is_full` 不变、无失败横幅。
5. 压缩期间用户发送消息：chat `begin(Replace)` 顶替压缩，压缩终态事件被代际守卫丢弃，不留 `streaming` 孤儿行。
6. 摘要消息 `tokens` 恒为 `NULL`；超长摘要被截断并追加 `chat.compaction.summary_truncated`。
7. 压缩请求含附件且超窗时，走降级路径而非超限直发（与 chat 一致）。
8. chat / agent 链路零差异：`cargo test -p assistant-agent` 与 `cargo test -p assistant-app` 全绿。

---

## 9 风险与对策

| # | 风险 | 对策 |
|---|---|---|
| P-1 | 压缩接入 `project_chat_messages` 后摘要内容可能变化（附件以投影文本出现） | 属有意修正（§3.2）；若需回退，`append_user_prompt` 之外再传 `offload_budget = None` 即可关闭降级 |
| P-2 | 删除 `TERMINATING` 后，`TurnTasks` 的 `Replace` 语义让「压缩期间发送」变成顶替 | 属预期语义（验收 5）；`manual_compact_conversation` 仍拒绝在流进行中启动压缩 |
| P-3 | 失败横幅删除后，摘要消息不在当前视口时用户看不到失败 | 消息 `error` 是唯一真值；`session-state-changed(Error)` 仍可用于全局提示，后续如需再补，应加在 `session-state` 而非新开事件 |
| P-4 | `conversation-terminated` 去掉 `summary` 后若有外部消费方 | 前端 `compactSummary` 无消费方，已一并删除；`summary` 正文以消息为唯一真值 |

---

## 10 决策记录

| # | 决策 | 结论 |
|---|---|---|
| D1 | 压缩是否收编为 `TurnRunner` 的第三种 `TurnSpec` | **否（修订）**。保留 `execute_compaction` 独立编排；初稿「完全复用」方针作废 |
| D2 | 压缩超时口径 | 单次调用 30s 常量，不引入 `AppConf` 字段 |
| D3 | 取消是否算失败 | 保留现状：`Cancelled` → 摘要消息 `interrupted`，同时 `emitter.terminated` 发取消事件 |
| D4 | 空摘要是否置 `is_full` | 保留现状：`has_effective_output == false` 时 `save_compaction_partial` 不写内容；`Ok` 分支仍按成功处理（与 chat 的「空响应失败」不同，属保留差异） |
| D5 | 摘要正文是否随事件下发 | **否**。删 `conversation-terminated.summary`，正文以消息为唯一真值 |
| D6 | 失败呈现 | 复用消息 `error` 块；删 `conversation-compact-failed`、`compactError`、`CompactErrorBanner` |
| D7 | 互斥源 | 只保留 `TurnTasks`，删 `TERMINATING` |
| D8 | 请求投影是否复用 | **是**。`project_chat_messages` 增 `append_user_prompt` 参数，修复无投影 / 无降级缺陷 |
| D9 | 成功刷新是否复用 | **是**。`emit_stream_finished` 参数化 `change_kind`，一次成功一次刷新 |
| D10 | 落库 role 如何表达 | 不扩 `CreateAssistantReq`；摘要行由宿主 `insert_message` 直接写 `compact_summary` |
| D11 | 压缩准入策略 | 压缩启动时 `TurnTasks::current` 非空即拒绝；与 chat 的 `Replace` 共用同一互斥源 |
| D12 | 摘要提交是否需要端口回调 | **否**。留在 `execute_compaction` 成功分支，单事务完成 |
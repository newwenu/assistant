# I15 — Chat Runtime 提取规格说明

## 1. 交付目标

将根包 `cmd/chat.rs` 和 `cmd/conversation.rs` 中的流式消费循环、消息构建、取消生命周期等 chat 运行时逻辑提取到 `assistant-agent` crate，使该 crate 从"仅含 compaction 碎片"升级为"普通 chat 运行时"，为后续 tool-call 执行循环（I16）奠定基础。

**设计原则**：
- **纯逻辑进 crate，Tauri 绑定留 cmd**：提取后的运行时函数不依赖 `AppHandle` / `Emitter`，可单元测试
- **消除重复**：`send_chat_message` 和 `execute_compaction` 中的同构流式消费循环统一为 `consume_stream`
- **保持行为不变**：提取为纯重构，不改变任何外部可观测行为（事件 payload、DB 写入、前端交互均不变）
- **为 tool-call 循环留口**：`consume_stream` 返回 `StreamOutcome`，其中 `finish_reason` 和 `tool_calls` 是执行循环的注入点
- **为 agent 类型分化奠基**：提示词构建提取为 `prompt_builder`，按 `AgentModeRow.kind` 分派策略，为 I16 不同 agent 类型的 tool-call 可配置性做准备
- **分层拆分**：流式消费拆为"累积状态"（`StreamAccumulator`）+ "主循环"（`consume_stream`）两层，对齐参考项目 OpenBitFun `agent-stream` 的 `StreamContext` + `StreamProcessor` 模式

> **前置依赖**：I7 ✅（compaction 已完成，`assistant-agent` crate 已存在）
> **核心参考**：OpenBitFun `agent-stream` crate（`StreamContext` + `StreamProcessor` + `process_stream_with_options` + `tool_call_accumulator`）
> **辅助参考**：deepseek-harness-orb `agent-loop`（settle/abandon 双路径 + max-tokens 截断 + tool 调度器）、airi `core-agent`（response categoriser + transient retry）

---

## 2. 当前问题分析

### 2.1 代码重复

`cmd/chat.rs` 的 `send_chat_message` 和 `cmd/conversation.rs` 的 `execute_compaction` 各含一个流式消费循环，结构同构：创建 stream → while stream.next() → 累积 content → emit delta → 检查 finish_reason → DB 写入 → 完成/错误处理。

两处循环共 ~300 行，逻辑差异仅：
- compaction 多 30s 超时
- compaction 多摘要截断
- chat 多 tool-call delta emit
- chat 多 compaction 触发

### 2.2 `assistant-agent` crate 空转

当前 crate 仅含 `compaction.rs`（3 个函数 + 3 个常量：`is_terminating` / `set_terminating` / `should_terminate` + `DEFAULT_COMPACTION_THRESHOLD_PERCENT` / `MAX_SUMMARY_REQUEST_TOKENS` / `DEFAULT_COMPACT_SUMMARY_PROMPT`），而 Agent 运行时的核心职责（消息构建、流式消费、取消管理）全在根包 `cmd/` 中。

### 2.3 不可测试

流式消费循环嵌在 Tauri command 函数内，依赖 `AppHandle` / `Emitter` / DB 连接，无法单元测试。提取后 `consume_stream` 为纯 async 函数，可用 mock stream 测试。

### 2.4 巨型函数

`send_chat_message` ~357 行（第 20–376 行），混合 IO / 逻辑 / 运行时 / 策略，认知负担高。

### 2.5 提示词构建硬编码

当前 `send_chat_message` 和 `execute_compaction` 中的消息构建逻辑存在重要差异，但被内联散落在两处：

| 差异点 | `send_chat_message` | `execute_compaction` |
|--------|---------------------|---------------------|
| system prompt 来源 | `mode.prompt`（AgentModeRow） | `detail.conversation.system_prompt`（对话设置） |
| 末尾追加 | 无 | 追加 `summary_prompt`（摘要请求 user 消息） |
| temperature | `mode.temperature` 或 0.7 | 固定 0.3 |
| max_tokens | `mode.max_tokens` 或 4096 | `app_conf.compact_summary_max_tokens`（u16→u32） |
| context_window_tokens | 从 model config 计算 | 0 |
| reasoning | `mode.reasoning_enabled` / `reasoning_effort` | None |
| StreamExt 来源 | `futures::StreamExt` | `tokio_stream::StreamExt as TokioStreamExt` |

`AgentModeRow` 已有 `kind` 字段（`"default"` / `"quick"` / `"deep"` / `"custom"`）和 `tools_json` 字段，但构建逻辑未利用 `kind` 做策略分派，`tools_json` 始终为 `"[]"`（死数据）。

### 2.6 reasoning 和 tool_calls 不持久化

当前 DB `messages` 表无 `reasoning` 和 `tool_calls_json` 列。前端 `chatStore` 在内存中累积 `reasoning` 和 `toolCalls`，刷新页面后丢失。对"深度"模式（`builtin-deep`，reasoning 是核心价值）和未来的 tool-call agent，这是严重体验缺陷。

---

## 3. 提取方案

### 3.1 新增模块

| 文件 | 来源 | 职责 |
|------|------|------|
| `chat_runtime.rs` | 新增 | `StreamAccumulator` + `consume_stream` + `StreamOutcome` + `StreamError` + `ToolCallAccumulator` |
| `prompt_builder.rs` | 新增 | `prepare_messages` + `build_chat_options` + `AgentKind` 分派 |
| `chat_tasks.rs` | 从根包搬入 | `ChatTasks` + `CancellationToken` 管理 |
| `compaction.rs` | 已有（不变） | should_terminate + terminating 标记 + 常量 |

### 3.2 `chat_runtime.rs` — 流式消费核心

#### 3.2.1 `StreamAccumulator` — 累积状态层

独立 struct，持有流式消费过程中所有中间累积状态：`full_content`、`full_reasoning`、`total_tokens`、`prompt_tokens`、`finish_reason`、`ToolCallAccumulator`。

**核心方法**：
- `new()` — 初始化空状态
- `apply_delta(&mut self, delta: &ChatDelta) -> ApplyDeltaOutcome` — 处理单个 delta，修改累积状态，返回结构化信号（见下文）
- `into_outcome(self) -> StreamOutcome` — **消费 self**，构造 `StreamOutcome`（含 `ToolCallAccumulator.finish()` 的 JSON 校验）。消费语义保证构造 outcome 后原 accumulator 不可再用，避免半构造状态误用。对齐 OpenBitFun `StreamContext::into_result(self)` (lib.rs:398)

**`ApplyDeltaOutcome` — 结构化返回信号**：

对齐 OpenBitFun `ToolCallDeltaOutcome` (tool_call_accumulator.rs:134)，`apply_delta` 不再返回 void，而是返回结构化信号，让调用方（`consume_stream` 主循环）可以精确响应不同事件类型：

```rust
pub struct ApplyDeltaOutcome {
    /// 前一个 tool call 因新 id 到达而自动 finalize（I16 使用，I15 恒 None）
    pub finalized_previous: Option<ToolCall>,
    /// tool call 参数片段正在流式传输（I16 用于前端实时渲染，I15 恒 None）
    pub params_partial: Option<ToolCallParamsChunk>,
}
```

I15 阶段 `ToolCallAccumulator::apply_delta` 内部逻辑已实现 finalized_previous 检测，但 `consume_stream` 暂不消费此信号（仅通过 `on_delta` 回调透传原始 delta）。I16 的执行循环将直接消费 `finalized_previous` 来触发工具执行。

**设计理由**（对齐 OpenBitFun `StreamContext`，lib.rs:335）：
1. **独立可测试**：构造 `StreamAccumulator` → apply 任意 delta 序列 → 断言 outcome，不需要 mock stream
2. **I16 graceful shutdown 扩展**：`StreamOutcome.partial_recovery_reason` 字段已预留，I16 在 cancel/timeout/error 路径中设置此字段即可返回部分结果，**无需新增 `into_partial_outcome()` 方法**，不改 `into_outcome` 签名。对齐 OpenBitFun `StreamContext.can_recover_as_partial_result()` (lib.rs:452) + `StreamResult.partial_recovery_reason` (lib.rs:279)
3. **职责分离**：累积逻辑（"每个 delta 做什么"）与循环逻辑（"怎么迭代"）解耦
4. **`has_effective_output` 标记**：仅收到用户可见输出（text / tool call / tool params）时设 true，thinking/reasoning 不算。I16 用于决定取消/错误时是否可 partial recovery。对齐 OpenBitFun `StreamContext.has_effective_output` (lib.rs:345)

#### 3.2.2 `StreamOutcome` — 单轮消费结果

| 字段 | 类型 | 说明 |
|------|------|------|
| `full_content` | `String` | 累积的 assistant 文本内容 |
| `full_reasoning` | `String` | 累积的 reasoning/thinking 内容（持久化来源） |
| `total_tokens` | `Option<u32>` | 仅来自 Provider usage 报告，None 表示未报告。token 估算由调用方在写 DB 时自行决定 |
| `prompt_tokens` | `Option<u32>` | 同上 |
| `finish_reason` | `Option<String>` | Provider 报告的结束原因 |
| `tool_calls` | `Option<Vec<ToolCall>>` | I16 预留。当前普通 chat 不使用 |
| `has_effective_output` | `bool` | 是否产出了用户可见输出（text / tool call / tool params）。I16 用于 partial recovery 决策。对齐 `StreamResult.has_effective_output` (lib.rs:271) |
| `partial_recovery_reason` | `Option<String>` | 非空表示结果为部分恢复（取消/超时/错误但已有有效输出）。I16 使用。对齐 `StreamResult.partial_recovery_reason` (lib.rs:279) |

类型选择：`total_tokens` / `prompt_tokens` 用 `u32` 与 `TokenUsage` 对齐，调用方写 DB 时自行转为 `Option<i64>`。

`has_effective_output` 和 `partial_recovery_reason` 为零成本预留字段，I15 正常完成时分别为 `false` 和 `None`，I16 扩展时无需改签名。

#### 3.2.3 `consume_stream` — 主循环层

纯 async 函数，只做"循环 + 取消 + 超时 + 委托"，自身无业务逻辑：

1. 构造 `StreamAccumulator`
2. `loop { tokio::select! { cancel / next_item } }`，其中 `next_item` 使用 `TimedStreamItem` 枚举统一处理流结束和超时（见下文）
3. 每个 delta：`accum.apply_delta(&delta)` + `on_delta(seq, &delta)`
4. 流结束：`accum.into_outcome()` → 返回 `StreamOutcome`

**`TimedStreamItem` 枚举** — 对齐 OpenBitFun `TimedStreamItem` (lib.rs:522)：

把"流结束"和"超时"统一为枚举变体，主循环 match 处理，比嵌套 `match tokio::time::timeout(...).await` 更清晰：

```rust
enum TimedStreamItem<T> { Item(T), End, TimedOut }

async fn next_item<S: Stream + Unpin>(
    stream: &mut S, timeout: Option<Duration>
) -> TimedStreamItem<S::Item> {
    match timeout {
        Some(d) => match tokio::time::timeout(d, stream.next()).await {
            Ok(Some(item)) => TimedStreamItem::Item(item),
            Ok(None) => TimedStreamItem::End,
            Err(_) => TimedStreamItem::TimedOut,
        },
        None => match stream.next().await {
            Some(item) => TimedStreamItem::Item(item),
            None => TimedStreamItem::End,
        },
    }
}
```

I15 用总超时（compaction 30s），I16 可升级为 watchdog 空闲超时（连续 N 秒无新数据则超时，长输出不被误杀），只需改 `next_item` 内部逻辑，主循环不变。

**参数**：
- `stream`：`futures::Stream<Item = Result<ChatDelta, ProviderError>>`（统一使用 `futures::StreamExt`，`execute_compaction` 需从 `tokio_stream` 迁移）
- `cancel_token`：`CancellationToken`，由外部 `ChatTasks` 管理
- `on_delta`：回调 `Fn(u32, &ChatDelta)`，由调用方决定 emit 逻辑
- `timeout`：`Option<Duration>`，chat 不设超时（None），compaction 设 30s

**关键设计决策**：

| 决策 | 理由 |
|------|------|
| `on_delta` 为回调而非直接 emit | 解耦 Tauri `AppHandle`，使函数可测试 |
| `timeout` 为 `Option<Duration>` | chat 不设超时（None），compaction 设 30s |
| 返回 `StreamOutcome` 而非在函数内写 DB | DB 写入是 IO 侧职责，由调用方处理 |
| `cancel_token` 由外部传入 | 生命周期由 `ChatTasks` 管理，运行时不拥有 |
| 累积逻辑在 `StreamAccumulator` 而非循环体内 | 对齐 OpenBitFun `StreamContext`，独立可测试，I16 扩展容易 |
| `delta_retry` 透传不消费 | `consume_stream` 不消费 retry 通知，仅通过 `on_delta` 回调传递 |
| `cancel_chat` 不清理 `is_terminating` | 取消信号与 compaction 状态解耦；compaction 检测到 `StreamError::Cancelled` 后在自己的错误处理路径中 `set_terminating(false)` |
| compaction 取消时保留 summary 消息 | `role=compact_summary` 不参与对话（`prepare_messages` 过滤），前端可渲染取消状态；删除需额外 DB 操作且无用户价值 |
| `tool_calls_json` → `toolCalls` 前端映射 | tool_calls 当前未实际接入项目可用状态；I16 设计统一前后端消息协议时一并解决字段映射，I15 不做临时 JSON.parse 补丁 |
| DB 写入/finish 通知 channel 保留在 cmd 层 | channel 是 Tauri command 层的 IO 解耦机制（`db_rx` 在独立 spawn 中写 DB 避免阻塞流式循环），不属于运行时逻辑 |

**`on_delta` 调用约定**：

回调接收 `(seq: u32, delta: &ChatDelta)`，调用方在回调内自行决定 emit 哪些字段。两种调用方的 payload 差异是正常的：
- `send_chat_message` 的回调 emit 全字段（`delta_text` / `delta_reasoning` / `delta_tool_call_*` / `usage` / `finish_reason` / `delta_retry`）
- `execute_compaction` 的回调仅 emit `delta_text` + `finish_reason`（compaction 不需要 reasoning / tool-call / retry 字段）

这保证了 `consume_stream` 本身不感知 Tauri 事件协议。

#### 3.2.4 `StreamError`

三种变体：`Provider(ProviderError)` / `Cancelled` / `Timeout`。

每个变体携带 `PartialOutcome`（含 `full_content`、`full_reasoning`、`total_tokens`、`prompt_tokens`、`has_effective_output`），表示错误发生时的部分累积结果。调用方据此决定 DB 写入策略：有输出则保存部分内容，无输出则不写。对齐 OpenBitFun `StreamProcessError.has_effective_output` (lib.rs:286)。

**设计演进**：初始设计为 `bool`（仅 `has_effective_output`），实现中发现取消/错误路径需要将 `full_content` 写入 DB（部分输出持久化），因此升级为 `PartialOutcome` 携带完整部分结果。`StreamAccumulator::into_partial(self)` 消费 accumulator 构造 `PartialOutcome`，与 `into_outcome(self)` 对称。

```rust
pub struct PartialOutcome {
    pub full_content: String,
    pub full_reasoning: String,
    pub total_tokens: Option<u32>,
    pub prompt_tokens: Option<u32>,
    pub has_effective_output: bool,
}

pub enum StreamError {
    Provider(ProviderError, PartialOutcome),
    Cancelled(PartialOutcome),
    Timeout(PartialOutcome),
}
```

#### 3.2.5 `ToolCallAccumulator`

OpenAI 的 tool call 流式传输是**增量拼接**模式，需要跨多个 delta 累积。

**结构**：`Vec<PartialToolCall>`，每个 `PartialToolCall` 含 `id` / `name` / `arguments_buf`。

**核心方法**：
- `new()` — 初始化
- `apply_delta(&mut self, delta: &ChatDelta) -> ToolCallDeltaOutcome` — 增量累积，返回结构化信号（对齐 OpenBitFun `PendingToolCalls::apply_delta` tool_call_accumulator.rs:449）
- `finish(self) -> Result<Vec<ToolCall>, ToolCallAccumulateError>` — 校验 JSON + 返回完整 `Vec<ToolCall>`

**`ToolCallDeltaOutcome`** — 对齐 OpenBitFun `ToolCallDeltaOutcome` (tool_call_accumulator.rs:134)：

```rust
pub struct ToolCallDeltaOutcome {
    /// 前一个 tool call 因新 id 到达而自动 finalize
    pub finalized_previous: Option<ToolCall>,
    /// 参数片段正在流式传输
    pub params_partial: Option<ToolCallParamsChunk>,
}
```

I15 简化版不含 `early_detected`（OpenBitFun 有，用于前端提前显示"正在调用 tool_x"），I16 按需添加。

**`ToolCallBoundary` 概念预留**：OpenBitFun 区分 5 种 finalize 边界 (tool_call_accumulator.rs:7)：`NewTool` / `FinishReason` / `StreamEnd` / `GracefulShutdown` / `EndOfAggregation`。I15 的 `finish(self)` 对应 `StreamEnd` 边界。I16 添加 graceful shutdown 时只需引入此枚举，不改 `finish` 签名。

**累积规则**（与前端 `chatStore.appendToolCallDelta` 对齐）：
- `delta_tool_call_id` 非空 → push 新 `PartialToolCall`（新 tool call 开始）
- 否则 → 追加到最后一个 `PartialToolCall` 的 `name` / `arguments_buf`
- `finish()` 时校验每个 `arguments_buf` 为合法 JSON

**max-tokens 截断策略**（对齐 deepseek-harness-orb `BlockAssembler.assembled()`）：

当 `finish_reason == "max_tokens"` 时，arguments 不完整的 tool call 应**丢弃**而非修复。理由：截断的 tool call arguments 不完整，无法安全执行；强行 JSON 修复可能产生语义错误。deepseek-harness-orb 在 `BlockAssembler.assembled()` 中对 `max-tokens` 场景执行 `all.filter(block => block.type !== 'tool-call')` 裁剪，replay metadata 也同步裁剪。

I15 的 `ToolCallAccumulator::finish()` 应在 `finish_reason == "max_tokens"` 时，对 arguments 不完整的 tool call 返回 `ToolCallAccumulateError::Truncated` 而非尝试 JSON 修复。这与 OpenBitFun 的 `repair_truncated_json`（tool_call_accumulator.rs:168）策略不同——OpenBitFun 总是尝试修复，但 deepseek-harness-orb 的丢弃策略在生产中更安全。**I15 采用丢弃策略**，I16 可按 agent 类型配置修复/丢弃策略。

**设计选择**：使用 `Vec` + 隐式 index（新 id = 新 tool call，否则追加到最后一个），而非 `HashMap<u32, ...>` + 显式 index。理由：与前端逻辑一致，且当前 `ChatDelta` 无 `delta_tool_call_index` 字段（`UnifiedToolCall` 有 `tool_call_index: Option<usize>`，但转换到 `ChatDelta` 时未传递）。如需支持并发多 tool call，应先在 `assistant-providers` 的 `ChatDelta` 中补上 `delta_tool_call_index` 字段，届时可升级为 HashMap 方案。

### 3.3 `prompt_builder.rs` — 提示词构建 + Agent 类型分派

#### 3.3.1 `prepare_messages`

从 `send_chat_message` 和 `execute_compaction` 中提取的消息列表构建逻辑。

**参数**：`detail: &ConversationDetail`、`system_prompt: &str`（由调用方决定来源）、`append_user_prompt: Option<&str>`（compaction 传入 summary_prompt，chat 传入 None）。

**逻辑**：过滤 `compact_summary` 消息 → `MessageRow` → `ChatMessage` 映射 → system prompt 插入首位 → 可选追加 user 消息。

`ChatOptions` 的构建（temperature / max_tokens / reasoning）**不在 `prepare_messages` 范围内**，由 `build_chat_options` 或调用方自行构造。

#### 3.3.2 `build_chat_options`

按 `AgentKind` 分派 ChatOptions 构建策略，替代当前 `send_chat_message` 中的内联 fallback 链。

**参数**：`kind: &AgentKind`、`mode: &AgentModeRow`、`context_window_tokens: u32`。

#### 3.3.3 `AgentKind` 枚举

四种变体：`Default` / `Quick` / `Deep` / `Custom`，从 `AgentModeRow.kind` 字段映射。

**策略方法**：
- `from_mode_kind(s: &str)` — 字符串 → 枚举，未知值 fallback 到 Default
- `supports_tool_call(&self)` — 当前仅 Custom 返回 true；I16 新增 Code/Research 等
- `persists_reasoning(&self)` — 仅 Deep 返回 true（深度模式 reasoning 有核心价值，需持久化）

I15 使用 enum + match 的最简分派，不引入 trait。I16 当需要运行时多态（动态注册 agent 类型）时再升级为 `PromptStrategy` trait。

#### 3.3.4 `tools_json` 的激活

当前 `AgentModeRow.tools_json` 全为 `"[]"`。`build_chat_options` 读取 `tools_json`，非空时解析为 `Vec<ToolDefinition>` 并设置到 `ChatOptions.tools`（I16 字段，I15 预留读取逻辑但 `ChatOptions.tools` 尚不存在，暂跳过）。这使 `tools_json` 从死数据变为运行时配置的入口。

### 3.4 `chat_tasks.rs` — 取消/完成生命周期 + 流尝试身份

从根包 `src/chat_tasks.rs` 整体搬入 `assistant-agent`。内部从 `Mutex<HashMap<String, CancellationToken>>` 扩展为 `Mutex<HashMap<String, ChatTaskEntry>>`，其中：

```rust
pub struct ChatTaskEntry {
    pub cancel_token: CancellationToken,
    pub attempt_counter: u32,
}
```

**公开 API 变更**：`start()` 返回值从 `CancellationToken` 扩展为 `(CancellationToken, String)`，第二个元素为 `attempt_id`（格式 `${conversation_id}:${attempt_counter}`）。

**`attempt_id` 语义**：流尝试身份标识，对齐 deepseek-harness-orb `LlmAttemptId`（`${sessionId}:${attempt}`）和 OpenBitFun `attempt_id`（`${round_id}:attempt:${attempt_number}`）。与 `message_id`（消息持久化身份）分离，解决 I16 流事件归属判定问题。

**`start()` 行为**：
1. 新对话：创建 `ChatTaskEntry { cancel_token, attempt_counter: 1 }`，返回 `(token, "${conversation_id}:1")`
2. 同对话重入（editMessage 重发 / regenerate）：递增 `attempt_counter`，取消旧 token，返回 `(new_token, "${conversation_id}:${new_counter}")`

**`chat-stream-start` 事件协议**：`start()` 返回 `attempt_id` 后，调用方（`cmd/chat.rs`）在流开始前发射 `chat-stream-start` 事件，携带 `conversation_id` / `attempt_id` / `message_id`。前端收到后记录 `activeAttemptId`，后续 `chat-delta` 事件按 `attempt_id` 过滤。对齐 deepseek-harness-orb `start` 帧协议。

**`chat-delta` 事件新增 `attempt_id` 字段**：所有 `chat-delta` emit 携带 `attempt_id`，前端 `handleDeltaEvent` 统一按 `attempt_id` 过滤所有事件类型（error / finish_reason / reasoning / tool_call / usage / retry）。

**与参考项目的差异说明**：OpenBitFun 使用 `DashMap<String, ChatTaskEntry>`（无锁并发 map），我们保持 `Mutex<HashMap>`。理由：
1. `ChatTasks` 操作频率低（每次对话 start/cancel/finish 各一次），`Mutex` 性能足够
2. 引入 `dashmap` 需新增 crate 依赖，当前场景不需要
3. OpenBitFun 使用 `DashMap` 是因为其 `agent-runtime` 支持多会话并发高频操作

搬入后根包 `src/chat_tasks.rs` 删除，全局替换 `use crate::chat_tasks::ChatTasks` → `use assistant_agent::chat_tasks::ChatTasks`（涉及 `setup.rs`、`cmd/chat.rs`）。

### 3.5 `cmd/chat.rs` 瘦身

提取后的 `send_chat_message` 变为：参数校验 + 守卫 → DB 读取 → insert 消息 → `prepare_messages`（调用 crate）→ `build_chat_options`（调用 crate）→ `adapter.chat_completion_stream` → spawn: `consume_stream` + on_delta(emit chat-delta) + DB 写入 + compaction 触发。

流式消费循环从 ~120 行降为 ~30 行（构造回调 + 处理 outcome）。

**DB 写入和 finish 通知的 channel 保留在 cmd 层**。`consume_stream` 返回 `StreamOutcome` 后，调用方通过 `db_tx.send()` 写入 DB、通过 `finish_tx.send()` 通知 `ChatTasks::finish()`。这两个 unbounded channel（`db_tx`/`db_rx`、`finish_tx`/`finish_rx`）是 Tauri command 层的 IO 解耦机制，不属于运行时逻辑。

**token 估算在 cmd 层执行**：当 `outcome.total_tokens.is_none()` 且 `!outcome.full_content.is_empty()` 时，调用方使用 `(content.len() as i64 + 3) / 4 + 4` 估算。这是 IO 侧策略（决定 DB 写入值），不是运行时逻辑。

#### 3.5.1 cmd 层辅助函数拆分

`send_chat_message` 提取 crate 调用后仍保留 ~200 行 IO 胶水代码。为进一步降低认知负担，将重复的 IO 编排逻辑提取为模块级辅助函数（纯代码组织，不改变行为）：

| 辅助函数 | 职责 | 消除的重复 |
|----------|------|-----------|
| `resolve_context_window(model, provider_row)` | 从 model config 计算 context_window_tokens | 内联 13 行 model_list 查找 |
| `estimate_tokens(content)` | `(len+3)/4+4` 估算 | 3 处内联估算 |
| `resolve_final_tokens(outcome)` | 从 StreamOutcome 决定最终 token 值（usage 或估算） | Ok 分支内联 15 行 |
| `resolve_partial_tokens(partial)` | 从 PartialOutcome 决定部分结果 token 值 | 3 个 Err 分支各 ~5 行重复 |
| `emit_delta(ectx, seq, delta)` | emit chat-delta 全字段 | on_delta 闭包内联 30 行 |
| `emit_stream_error(ectx, error)` | emit chat-delta error 字段 | 4 处内联 emit error |
| `save_partial_on_error(db_tx, msg_id, partial)` | 错误路径 DB 写入（partial content + 估算 token） | 3 个 Err 分支各 ~5 行重复 |
| `spawn_db_writer(app)` | 创建 DB 写入 channel + spawn 消费者 | 内联 15 行 channel 样板 |
| `spawn_finish_listener(app)` | 创建 finish 通知 channel + spawn 消费者 | 内联 8 行 channel 样板 |
| `spawn_compaction_listener(app)` | 创建 compaction 触发 channel + spawn 消费者 | 内联 25 行 compaction spawn |

**`StreamEmitCtx` 结构体**：统一 emit 上下文（`orb_window` + `conv_id` + `attempt_id` + `msg_id`），避免每个 emit 调用重复传递 4 个参数。生命周期 `'a` 借用 `stream_future` 闭包内的变量，零分配。

**compaction channel 解耦**：`spawn_compaction_listener` 返回 `UnboundedSender<(conv_id, detail, provider_row)>`。`stream_future` 闭包内仅需 `compact_tx.send(...)` 一行即可触发 compaction，无需捕获 `app`/`detail`/`provider_row` 等大对象。compaction 的 `set_terminating(true)` + `execute_compaction` + 错误处理全部在 listener 的 spawn 中执行。

**`AgentModeRow::default()`**：为 `AgentModeRow` 派生 `Default`，消除手动构造 15 行默认值。`build_chat_options(kind, mode.as_ref().unwrap_or(&AgentModeRow::default()), ctx)` 一行替代。

**`AgentKind::default()`**：为 `AgentKind` 派生 `Default`（`Default` 变体），`unwrap_or_default()` 替代 `unwrap_or(AgentKind::Default)`。

**效果**：`send_chat_message` 从 ~375 行降至 ~198 行（-47%），`stream_future` 闭包从 ~220 行降至 ~78 行（-64%）。

### 3.6 `cmd/conversation.rs` 的 `execute_compaction` 复用

提取后：`prepare_messages`（append_user_prompt = Some(summary_prompt)）→ `adapter.chat_completion_stream` → `consume_stream`(timeout = 30s, on_delta 仅 emit delta_text + finish_reason) → 摘要截断 → 事务写入 DB + emit 事件。

删除重复的流式消费代码，从 ~221 行降为 ~80 行。

### 3.7 DB schema 扩展 — reasoning + tool_calls 持久化

`messages` 表增加 `reasoning TEXT` 列 + `tool_calls_json TEXT` 列。`MessageRow` struct 同步新增对应字段。`update_message_content` 签名扩展为接受 `reasoning` 和 `tool_calls_json` 参数。

**Migration 落地约束**：当前 `migrations::run_migrations()` 已禁用，migration 仅手动执行。双路径处理：
- **新安装**：修改 `CREATE TABLE messages` 语句直接加两列
- **已有数据库升级**：手动执行 `ALTER TABLE messages ADD COLUMN reasoning TEXT` / `ALTER TABLE messages ADD COLUMN tool_calls_json TEXT`

**列读取连锁修改**：`MSG_SELECT` 常量（conversations.rs:6）和 `read_msg_row` 函数（conversations.rs:59）硬编码列名和位置偏移，新增列后两处必须同步修改，否则运行时 panic。

**签名扩展全调用方更新**：`update_message_content` 签名扩展后，除 `chat.rs` 的 `db_rx` 外，`conversation.rs` 的 compaction 写入也调用此函数，所有调用方必须同步传参。

**写入时机**：`consume_stream` 返回 `StreamOutcome` 后，调用方在 `db_tx.send()` 中一并写入：
- `reasoning`：`outcome.full_reasoning`（非空时写入）
- `tool_calls_json`：`outcome.tool_calls` 序列化（I16 有 tool call 时写入）

**读取时机**：`get_conversation_detail` / `get_messages_paginated` 返回的 `MessageRow` 携带 `reasoning` 和 `tool_calls_json`，前端 `chatTypes.ts` 的 `Message` 接口已有 `reasoning?: string` 和 `toolCalls?: ToolCall[]` 字段。`reasoning` 字段名一致直接透传；`tool_calls_json` → `toolCalls` 映射 I16 设计统一前后端消息协议时一并解决，I15 不做临时补丁。

---

## 4. 模块归属（提取后）

| 模块 | 归属 | 职责 |
|------|------|------|
| `compaction.rs` | assistant-agent | should_terminate + terminating 标记 + 常量（不变） |
| `chat_runtime.rs` | assistant-agent | StreamAccumulator + consume_stream + StreamOutcome + StreamError + ToolCallAccumulator |
| `prompt_builder.rs` | assistant-agent | prepare_messages + build_chat_options + AgentKind 分派 |
| `chat_tasks.rs` | assistant-agent | ChatTasks + CancellationToken 生命周期管理 |
| `cmd/chat.rs` | assistant-app | Tauri command 入口 + DB 读写 + emit 事件 + compaction 触发 + token 估算 + 辅助函数（`emit_delta` / `emit_stream_error` / `spawn_db_writer` / `spawn_finish_listener` / `spawn_compaction_listener` / `resolve_context_window` / `estimate_tokens` / `resolve_final_tokens` / `resolve_partial_tokens` / `save_partial_on_error`） |
| `cmd/conversation.rs` | assistant-app | execute_compaction 前置/后置 + manual_compact + unlock command |

---

## 5. 依赖关系

### 5.1 crate 依赖变更

`assistant-agent/Cargo.toml` 新增：`assistant-providers`（ChatMessage, ChatDelta, ChatOptions, ProviderError, ToolCall）、`assistant-storage`（ConversationDetail, MessageRow, AgentModeRow）、`tokio-util`（CancellationToken）、`futures`（Stream trait）。

### 5.2 根包依赖变更

`assistant-app/Cargo.toml`：删除 `tokio-util` 直接依赖（改为通过 `assistant-agent` 间接使用）；删除 `tokio-stream` 直接依赖（仅 `conversation.rs` 使用，提取后不再需要）。注意：`assistant-providers` crate 的 `base.rs` 也 `use tokio_stream::Stream`，`tokio-stream` 是 providers 的直接依赖，根包移除不影响 providers 编译。

### 5.3 依赖方向

assistant-app → assistant-agent → assistant-providers / assistant-storage。不引入循环依赖。`assistant-agent` 不依赖 `assistant-app`（不依赖 AppHandle）。

---

## 6. 行为不变保证

| 行为 | 提取前 | 提取后 | 验证方式 |
|------|--------|--------|---------|
| 流式 delta 事件 | `orb_window.emit("chat-delta", ...)` | `on_delta(seq, &delta)` 回调内 emit | 前端 chat-delta 事件不变 |
| chat-delta 携带 attempt_id | 无 attempt_id 字段 | 所有 chat-delta emit 携带 `attempt_id` | 前端按 attempt_id 过滤 |
| chat-stream-start 事件 | 无 | 流开始前 emit `chat-stream-start`（含 `conversation_id` / `attempt_id` / `message_id`） | 前端收到后记录 activeAttemptId |
| attempt_id 格式 | 无 | `${conversation_id}:${attempt_counter}`，后端生成 | 对齐 deepseek-harness-orb `LlmAttemptId` |
| attempt_id 过滤 | 无（旧 message_id 过滤失效） | 前端 `handleDeltaEvent` 统一按 `attempt_id` 过滤所有事件类型 | 旧请求事件自动忽略，新请求事件正常处理 |
| streamingMessageId 对齐 | 前端 `temp-assistant-xxx` ≠ 后端 UUID | `chat-stream-start` 到达时前端替换为后端真实 `message_id` | 消息 ID 一致性 |
| tool-call delta 透传 | emit 全字段 | `on_delta` 回调内 emit 同字段 | 前端 `appendToolCallDelta` 行为不变 |
| reasoning delta 透传 | emit 全字段 | `on_delta` 回调内 emit 同字段 | 前端 `appendReasoningDelta` 行为不变 |
| 取消 | `cancel_token.is_cancelled()` → emit error | 同上 | cancel_chat 行为不变 |
| 取消时部分内容持久化 | 取消时 `full_content` 写入 DB | `StreamError::Cancelled(PartialOutcome)` → `partial.full_content` 写入 DB | 取消后刷新页面仍可见已输出部分 |
| Provider 错误时部分内容持久化 | 错误时 `full_content` 写入 DB | `StreamError::Provider(_, PartialOutcome)` → `partial.full_content` 写入 DB | 错误前已输出内容不丢失 |
| DB 写入 | `db_tx.send((msg_id, content, tokens))` | consume_stream 返回 outcome 后调用方写 DB | 消息内容/token 不变 |
| compaction 触发 | `should_terminate` + spawn | 同上 | 自动终止行为不变 |
| compaction 超时 | `tokio::time::timeout(30s)` | `consume_stream(..., Some(30s))` | 超时行为不变 |
| 摘要截断 | `summary.truncate(max_chars)` | 调用方在 outcome 后截断 | 截断行为不变 |
| reasoning 持久化 | 无（刷新丢失） | `outcome.full_reasoning` 写入 DB `messages.reasoning` | 历史回看含 reasoning |
| tool_calls 持久化 | 无（刷新丢失） | `outcome.tool_calls` 序列化写入 DB `messages.tool_calls_json` | 历史回看含 tool_calls |
| regenerate 路径 | `regenerate_message` 删消息 → 前端重触发 `send_chat_message` | 同上 | regenerate → resend 副作用序列不变 |
| compaction 可取消 | compaction 无 cancel_token，用户无法中止 | compaction 启动时调 `chat_tasks.start()` 获取 cancel_token，传入 `consume_stream`；用户调 `cancel_chat` 可取消 compaction | 模型循环/卡死时用户可中止；chat 和 compaction 不会同时运行，共享 entry 语义正确 |
| editMessage 时序窗口 | cancel 后清 streamingMessageId → 事件丢失 | cancel 后保留旧 ID，chat-stream-start 到达时原子替换 | 无事件丢失窗口 |

---

## 7. 为 I16（tool-call 执行循环）预留的接口

### 7.1 `StreamOutcome.tool_calls`

当前 `consume_stream` 在收到 `finish_reason: Some("tool_calls")` 时，将累积的 `tool_calls` 填入 `StreamOutcome.tool_calls`。普通 chat 模式下调用方忽略此字段直接结束；I16 的执行循环读取此字段决定是否继续。

### 7.2 `StreamOutcome.finish_reason`

I16 执行循环的判断条件：`finish_reason == "tool_calls"` → 执行工具 → 拼结果消息 → 再次调用 Provider；否则结束。

### 7.3 多轮 token 累积

I16 执行循环中，多轮 `consume_stream` 的 token 需要累积。`StreamOutcome` 的 `total_tokens` / `prompt_tokens` 是单轮值，I16 的循环层需要自行累加（类似 OpenBitFun 的 `TurnTokenUsage::merge_round`）。本次不实现 `TurnTokenUsage`，但 `StreamOutcome` 的字段设计不阻碍后续添加。

### 7.4 `ChatOptions.tools`

当前 `ChatOptions` 不含 `tools` 字段。I16 需要新增，本次不新增（避免提前引入未使用的字段），但 `consume_stream` 的设计不阻碍后续添加。`prompt_builder.build_chat_options` 已预留 `tools_json` 读取逻辑。

### 7.5 `AgentKind` 扩展

I16 新增 agent 类型时只需：在 enum 添加变体 → 在 `from_mode_kind` 添加匹配 → 在 `supports_tool_call` / `persists_reasoning` 设置策略 → 可选升级为 `PromptStrategy` trait。

### 7.6 前端协同

前端 `chatOrchestrator` 的 `ChatPhase` 已预留 `"tool_executing"` 状态。I15 不改变前端行为，但 I16 的执行循环需要：Rust 端 emit `chat-tool-execute` 事件 → 前端设 `phase = "tool_executing"` → Rust 端 emit `chat-delta`（含 tool result）→ 前端恢复 `phase = "streaming"`。

### 7.7 超时增强

当前 `consume_stream` 接受 `Option<Duration>`（仅 total timeout）。`assistant-providers` 已定义 `StreamTimeoutConfig`（含 `first_token_timeout` / `idle_timeout` / `total_timeout`），I16 或后续迭代可升级为三阶段超时，本次不引入。

### 7.8 `StreamAccumulator` 扩展 — partial recovery

I16 graceful shutdown 时，`consume_stream` 在 cancel/timeout/error 路径中检查 `accum.has_effective_output`，若为 true 则设置 `outcome.partial_recovery_reason = Some("...")` 后返回 `Ok(outcome)`，而非返回 `Err`。调用方通过 `partial_recovery_reason.is_some()` 判断结果是否为部分恢复。

**无需新增 `into_partial_outcome()` 方法**——`into_outcome(self)` 已包含 `partial_recovery_reason` 字段，正常完成时为 None，部分恢复时为 Some。对齐 OpenBitFun `StreamContext.can_recover_as_partial_result()` (lib.rs:452) + `StreamResult.partial_recovery_reason` (lib.rs:279)。

I16 另需 `force_finish_pending_tool_calls()` 方法，把未完成的 tool call 强制 finalize（标记 `Interrupted`），确保流中断后 tool call 状态干净。对齐 OpenBitFun `StreamContext.force_finish_pending_tool_calls()` (lib.rs:506)。

### 7.9 settle/abandon 双路径（I16+ 预留，对齐 deepseek-harness-orb）

deepseek-harness-orb 的 `AssistantStreamAttempt` 区分两种终态：
- **`settle(eventType, append)`**：DB 写入成功 → 发布 `committed` 终态帧，流结果持久化
- **`abandon()`**：DB 写入失败 → 发布 `abandoned` 终态帧，流结果丢弃

**解决的问题**：如果 `consume_stream` 成功返回 `StreamOutcome` 但后续 DB 写入失败，前端可能永远卡在 streaming 状态（已收到 delta 但未收到 end 信号）。settle/abandon 保证前端始终收到终态通知。

**I15 不实现**（DB 写入失败直接 panic 是当前行为），I16 在 `consume_stream` 返回后引入 settle/abandon 语义：DB 写入成功 → committed，失败 → abandoned + emit error 事件。

参考：deepseek-harness-orb `AssistantStreamAttempt.settle()` / `.abandon()` (agent-loop/src/assistant-stream.ts:97/113)

### 7.10 编排运行时（I16+ 预留，对齐 airi）

airi 的 `ChatOrchestratorRuntime` 提供了比简单 `consume_stream` 更完整的编排层：
- **FIFO 发送队列**：用户连续发送时排队，前一条完成后才处理下一条
- **会话 generation 守卫**：同一会话同时只有一个活跃 generation，新请求取消旧的
- **生命周期钩子**：`onGeneratedTurn` / `onInterruptedTurn` 回调，在 generation 完成后统一处理持久化和 UI 更新

**I15 不实现**（当前 `ChatTasks` 的 `start/cancel/finish` 已提供基本的互斥），I16 当需要多轮 tool-call 循环时，可参考 airi 的 `ChatOrchestratorRuntime` 升级 `ChatTasks` 为编排运行时。

参考：airi `ChatOrchestratorRuntime` (core-agent/src/runtime/chat-orchestrator-runtime.ts)

### 7.11 工具输出截断（I16+ 预留，对齐 deepseek-harness-orb）

deepseek-harness-orb 的 `dsh-output-retention` 包提供结构化的工具输出截断：
- **`TextRetainer`**：支持 `head` / `tail` / `headTail` 三种字节窗口策略
- **UTF-8 安全截断**：不会在多字节字符中间截断
- **标准化省略脚注**：截断时插入 `[...N bytes omitted...]` 标记

**解决的问题**：工具输出（如 shell 命令结果、文件内容）可能非常长，直接拼入 context 会浪费 token。截断保留首尾关键信息，中间省略。

**I15 不实现**，I16 工具执行循环返回结果时按需截断。参考：deepseek-harness-orb `dsh-output-retention` (packages/util/output-retention/src/index.ts)

### 7.12 FinishReason 结构化升级路径（I16+ 预留，对齐 deepseek-harness-orb）

当前 `StreamOutcome.finish_reason` 为 `Option<String>`，调用方做字符串比较。deepseek-harness-orb 的 `FinishReasonMap` 将 finish_reason 映射为结构化枚举：

```rust
// I16+ 升级方向
pub enum FinishReason {
    Stop,                    // "stop"
    ToolCalls,               // "tool_calls"
    MaxTokens,               // "max_tokens" — 触发 §3.2.5 截断策略
    Aborted(Failure),        // 中断（取消/错误）
    Error(Failure),          // Provider 错误
}
```

**好处**：
1. `MaxTokens` 变体可精确路由到截断策略（§3.2.5），无需字符串匹配
2. `Aborted` / `Error` 携带结构化 `Failure`，调用方无需再解析 error 字符串
3. 编译器保证穷举所有 finish_reason，不会遗漏新增值

**I15 保持 `Option<String>`**（最小变更），I16 升级为枚举。参考：deepseek-harness-orb `FinishReasonMap` (llm/src/types.ts)

### 7.13 瞬态重试（I16+ 预留，对齐 airi + OpenBitFun）

| 项目 | 重试策略 |
|------|---------|
| **OpenBitFun** | `should_retry` + `delay_ms` — 分类重试（Auth 永不重试 / Network 必重试）+ 指数退避 + Provider Hint 优先 |
| **deepseek-harness-orb** | `agent/request-error` waterfall — listener 返回 `{ kind: 'retry' }` 触发重试，否则终态失败 |
| **airi** | `transientRetryDelayMs` — 408/429/5xx 指数退避 + `Retry-After` 尊重 + 30s 上限 |

**airi 的 `Retry-After` 尊重 + 上限设计更实用**：超过 30s 的 `Retry-After` 直接失败让用户决定，而非盲目等待。OpenBitFun 的分类重试更精细但需要更多配置。

**I15 不实现重试**（当前行为：Provider 错误直接返回），I16 参考 airi 的 `transientRetryDelayMs` 模式在 `consume_stream` 外层包装重试逻辑。

---

## 8. 参考映射

| 功能点 | 参考项目 | 参考文件 + 行号 | 借鉴内容 | 我们的对等物 |
|--------|---------|----------------|---------|------------|
| 流式消费主循环 | OpenBitFun | agent-stream/lib.rs:1046 `process_stream_with_options` | 主循环极简，委托给 `handle_*` 方法；`tokio::select!` 同时监听 cancel + next_item | `consume_stream` |
| 累积状态 struct | OpenBitFun | agent-stream/lib.rs:335 `StreamContext` | struct 持有所有中间状态，`into_result(self)` 消费语义，独立于循环逻辑 | `StreamAccumulator` |
| 流结果 | OpenBitFun | agent-stream/lib.rs:250 `StreamResult` | `has_effective_output` + `partial_recovery_reason` + `first_chunk_ms` 等字段 | `StreamOutcome` |
| 流错误 | OpenBitFun | agent-stream/lib.rs:284 `StreamProcessError` | 携带 `has_effective_output: bool`，调用方据此决定 DB 写入策略 | `StreamError` |
| 超时包装枚举 | OpenBitFun | agent-stream/lib.rs:522 `TimedStreamItem` + `next_stream_item` | Item/End/TimedOut 三变体统一处理，主循环 match 分派 | `TimedStreamItem` + `next_item` |
| 取消检测 | OpenBitFun | agent-stream/lib.rs:627 `check_cancellation` | 每个 chunk 处理后也检查取消；我们用 `tokio::select!` 同时监听，等效 | `tokio::select!` cancel 分支 |
| partial recovery | OpenBitFun | agent-stream/lib.rs:452 `can_recover_as_partial_result` + lib.rs:1105 | `has_effective_output` 为 true 时返回部分结果而非 Err | `StreamOutcome.partial_recovery_reason` |
| force finish | OpenBitFun | agent-stream/lib.rs:506 `force_finish_pending_tool_calls` | 取消/错误时强制 finalize 未完成 tool call，标记 `Interrupted` | I16 预留 |
| Tool call 累积 | OpenBitFun | agent-stream/tool_call_accumulator.rs:141 `PendingToolCalls` | `BTreeMap<ToolCallStreamKey, PendingToolCall>` + `apply_delta` 返回 `ToolCallDeltaOutcome` | `ToolCallAccumulator`（Vec + 隐式 index 简化版） |
| Tool call delta 信号 | OpenBitFun | agent-stream/tool_call_accumulator.rs:134 `ToolCallDeltaOutcome` | `finalized_previous` + `early_detected` + `params_partial` 三信号 | `ToolCallDeltaOutcome`（含 `finalized_previous` + `params_partial`，I15 简化版） |
| Tool call 边界 | OpenBitFun | agent-stream/tool_call_accumulator.rs:7 `ToolCallBoundary` | 5 种 finalize 边界，不同边界 JSON 修复策略不同 | I16 预留概念 |
| Tool call 索引 | OpenBitFun | agent-stream/tool_call_accumulator.rs:81 `ToolCallStreamKey` | `Indexed(usize)` / `Unindexed` 双模式，支持并发多 tool call 交错 delta | Vec + 隐式 index（与前端对齐，I16 按需升级） |
| JSON 截断修复 | OpenBitFun | agent-stream/tool_call_accumulator.rs:168 `repair_truncated_json` | 关闭未闭合字符串 + 按栈序关闭括号，拒绝不安全截断 | I16 预留 |
| 取消生命周期 | OpenBitFun | agent-runtime/turn_cancellation.rs | `CancellationToken` 管理 + 按对话隔离 | `ChatTasks`（保持 `Mutex<HashMap>` 而非 `DashMap`，理由见 §3.4） |
| 流尝试身份 | deepseek-harness-orb | agent-loop/src/assistant-stream.ts `LlmAttemptId` | `${sessionId}:${attempt}` branded type，start/chunk/end 帧携带 | `attempt_id`（`${conversation_id}:${attempt_counter}`，普通 string） |
| 流尝试身份 | OpenBitFun | contracts/events/src/agentic.rs `attempt_id` | `${round_id}:attempt:${attempt_number}`，每事件携带 | `attempt_id`（同上） |
| start 帧协议 | deepseek-harness-orb | agent-loop/src/assistant-stream.ts `start` 帧 | 含 `attemptId` / `turn` / `step`，前端记录 activeAttempt | `chat-stream-start` 事件（含 `attempt_id` / `message_id`） |
| attempt 过滤 | deepseek-harness-orb | api/session-controller/src/client/sessions/assistant-stream.ts:120 | `frame.attemptId !== activeAttempt.attemptId` → 忽略 | `handleDeltaEvent` 统一 `attempt_id` 过滤 |
| 对话轮次统计 | OpenBitFun | agent-runtime/dialog_turn.rs | `TurnStats` / `TurnTokenUsage::merge_round` | I16 多轮循环自行累加 |
| 权限规划 | OpenBitFun | agent-runtime/permission.rs | `PermissionIntentPlan` 三态 | I16 工具执行时参考 |
| Agent 类型注册 | OpenBitFun | agent-runtime/agents.rs | `BuiltinAgentDefinitionSpec` 工厂 | `AgentKind` enum 最简分派，I16 再升级为 trait |
| 提示词渲染 | OpenBitFun | agent-runtime/prompt.rs | `render_prompt_environment_info()` + `render_direct_tool_listing_body()` | `prompt_builder` |
| 上下文策略 | OpenBitFun | agent-runtime/context_profile.rs | `ContextProfile` + `ModelCapabilityProfile` | 后续迭代参考 |
| 预取压缩 | OpenBitFun | agent-runtime/compression_prefetch.rs | `CompressionPrefetch` | 本次不实现，`on_delta` 回调机制预留可能 |
| 重试策略 | OpenBitFun | agent-stream/retry.rs:7 `should_retry` + retry.rs:25 `delay_ms` | 分类重试（Auth 永不重试 / Network 必重试）+ 指数退避 + Provider Hint 优先 | I16 预留骨架 |
| SSE 诊断日志 | OpenBitFun | agent-stream/lib.rs:131 `SseLogCollector` | 正常不输出，错误时 flush 全部 SSE 历史 + 智能截断 | I16 按需加入 |
| 事件发射解耦 | OpenBitFun | agent-stream/lib.rs:80 `StreamEventSink` trait | `async fn enqueue(&self, event, priority)` — 测试时注入 `RecordingEventSink` | `on_delta` 回调（同步，更轻量）；验证时用 `Arc<Mutex<Vec>>` 收集 |
| settle/abandon 双路径 | deepseek-harness-orb | agent-loop/src/assistant-stream.ts:97/113 `settle()`/`abandon()` | DB 写入成功 → committed，失败 → abandoned，前端始终收到终态 | I16 预留（§7.9） |
| 流累积 + 组装二层 | deepseek-harness-orb | llm/src/assembler.ts `AssistantStreamAccumulator` + `BlockAssembler` | `Accumulator` 做持久化压缩（去重/合并），`Assembler` 做语义组装（content block 分类） | I16+ 迭代方向：拆分 `StreamAccumulator` 为持久化压缩层 + 语义组装层 |
| max-tokens 截断丢弃 | deepseek-harness-orb | llm/src/assembler.ts `BlockAssembler.assembled()` | `finish_reason === 'max-tokens'` 时丢弃不完整 tool call | `ToolCallAccumulator::finish()` 截断策略（§3.2.5） |
| interrupted 前缀保留 | deepseek-harness-orb | agent-loop/src/assistant-stream.ts:133 `interruptedBlocks()` | 取消时保留已送达前缀，写入 `interrupted: true` | I16 partial recovery（§7.8） |
| tool 调度器 | deepseek-harness-orb | agent-loop/src/tool-calls.ts `executeToolCalls` | 排他屏障 + 有界并行池 + abort drain + 失败静默 | I16 tool 执行循环 |
| FinishReason 结构化 | deepseek-harness-orb | llm/src/types.ts `FinishReasonMap` | `stop`/`tool_calls`/`max_tokens`/`aborted`/`error` 枚举 | I16 升级 `Option<String>` → 枚举（§7.12） |
| 工具输出截断 | deepseek-harness-orb | util/output-retention/src/index.ts `TextRetainer` | head/tail/headTail 字节窗口 + UTF-8 安全截断 + 省略脚注 | I16 工具执行结果截断（§7.11） |
| 编排运行时 | airi | core-agent/src/runtime/chat-orchestrator-runtime.ts | FIFO 发送队列 + generation 守卫 + 生命周期钩子 | I16 升级 `ChatTasks` 为编排运行时（§7.10） |
| response categoriser | airi | core-agent/src/runtime/response-categoriser.ts `createStreamingCategorizer` | rehype 动态提取任意 XML 标签作为 reasoning，适配不合作的 Provider | I16 adapter 层转换（当前 DeepSeek adapter 已处理 `thinking` 标签） |
| 瞬态重试 | airi | core-agent/src/runtime/generation.ts `transientRetryDelayMs` | 408/429/5xx 指数退避 + `Retry-After` 尊重 + 30s 上限 | I16 重试逻辑（§7.13） |
| request-error waterfall | deepseek-harness-orb | agent-loop/src/agent.ts:460 `agent/request-error` | listener 返回 `{ kind: 'retry' }` 触发重试，否则终态失败 | I16 重试逻辑（§7.13） |

---

## 9. 验收标准

1. `consume_stream` + `StreamAccumulator` 统一 `send_chat_message` 和 `execute_compaction` 的流式消费逻辑，无重复代码
2. `StreamAccumulator` 独立可测试：构造 → apply delta 序列 → 断言 outcome
3. `StreamAccumulator::apply_delta` 返回 `ApplyDeltaOutcome`（含 `finalized_previous` + `params_partial`），I15 暂不消费信号但结构已就位
4. `StreamOutcome` 含 `full_reasoning` + `tool_calls` + `has_effective_output` + `partial_recovery_reason` 字段，为 I16 预留
5. `StreamError` 每个变体携带 `has_effective_output: bool`，调用方可据此决定 DB 写入策略
6. `prepare_messages` 统一消息列表构建逻辑（含 `append_user_prompt` 参数）
7. `build_chat_options` 按 `AgentKind` 分派 ChatOptions 构建，替代内联 fallback 链
8. `ChatTasks` 从根包搬入 `assistant-agent`，根包 `chat_tasks.rs` 删除
9. `ChatTasks.start()` 返回 `(CancellationToken, attempt_id)`，`attempt_id` 格式 `${conversation_id}:${attempt_counter}`
10. `ChatTaskEntry` 含 `cancel_token` + `attempt_counter`，同对话重入时递增 counter 并取消旧 token
11. `chat-stream-start` 事件在流开始前发射，携带 `conversation_id` / `attempt_id` / `message_id`
12. 所有 `chat-delta` 事件携带 `attempt_id` 字段
13. 前端 `handleDeltaEvent` 统一按 `attempt_id` 过滤所有事件类型（error / finish_reason / reasoning / tool_call / usage / retry）
14. 前端 `chat-stream-start` 监听器设置 `activeAttemptId` 并替换 `streamingMessageId` 为后端真实 ID
15. `editMessage` / `regenerateMessage` cancel 后不再清 `streamingMessageId` / `activeAttemptId`，消除时序窗口
16. `assistant-agent` crate 依赖 `assistant-providers` + `assistant-storage`，不依赖 `assistant-app`
17. `consume_stream` 不依赖 `AppHandle` / `Emitter`，可单元测试
18. 所有外部可观测行为不变：chat-delta 事件、DB 写入、compaction 触发、取消、超时
19. DB `messages` 表新增 `reasoning` + `tool_calls_json` 列，reasoning 持久化生效
20. reasoning 持久化条件：`outcome.full_reasoning 非空 && kind.persists_reasoning()`（仅 Deep 模式持久化）
21. `cmd/chat.rs` 的 `send_chat_message` 从 ~357 行降至 ~150 行
22. `cmd/conversation.rs` 的 `execute_compaction` 从 ~221 行降至 ~80 行
23. cargo check + clippy + test 通过
24. 前端行为无变化；`reasoning` 字段名一致直接透传；`tool_calls_json` → `toolCalls` 映射 I16 设计统一前后端消息协议时一并解决，I15 不做临时补丁
25. `compaction.rs` 模块代码不变（仅 `lib.rs` 新增模块声明）
26. `assistant-agent` crate 的公开 API 在 I15 完成后冻结，I16 只能**新增** API，不能**修改**已有签名（向后兼容的扩展除外）
27. `ToolCallAccumulator::finish()` 在 `finish_reason == "max_tokens"` 时对不完整 tool call 返回 `Truncated` 错误而非尝试 JSON 修复（对齐 deepseek-harness-orb 丢弃策略，§3.2.5）
28. `MSG_SELECT` + `read_msg_row` 与新增列同步修改，列偏移正确
29. `update_message_content` 签名扩展后所有调用方同步传参
30. compaction 调用 `consume_stream` 时传入 `chat_tasks.start()` 返回的 cancel_token，用户可通过 `cancel_chat` 取消 compaction
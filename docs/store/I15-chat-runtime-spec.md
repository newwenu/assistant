# I15 — Chat Runtime 提取规格说明

## 1. 交付目标

将根包 `cmd/chat.rs` 和 `cmd/conversation.rs` 中的流式消费循环、消息构建、取消生命周期等 chat 运行时逻辑提取到 `assistant-agent` crate，使该 crate 从"仅含 compaction 碎片"升级为"普通 chat 运行时"，为后续 tool-call 执行循环（I16）奠定基础。

**设计原则**：
- **纯逻辑进 crate，Tauri 绑定留 cmd**：提取后的运行时函数不依赖 `AppHandle` / `Emitter`，可单元测试
- **消除重复**：`send_chat_message` 和 `execute_compaction` 中的同构流式消费循环统一为 `consume_stream`
- **保持行为不变**：提取为纯重构，不改变任何外部可观测行为（事件 payload、DB 写入、前端交互均不变）
- **为 tool-call 循环留口**：`consume_stream` 返回 `StreamOutcome`，其中 `finish_reason` 和 `tool_calls` 是执行循环的注入点

> **前置依赖**：I7 ✅（compaction 已完成，`assistant-agent` crate 已存在）
> **核心参考**：OpenBitFun `agent-runtime` crate 的 `dialog_turn` + `scheduler` + `turn_cancellation` 模块

---

## 2. 当前问题分析

### 2.1 代码重复

`cmd/chat.rs` 的 `send_chat_message` 和 `cmd/conversation.rs` 的 `execute_compaction` 各含一个流式消费循环，结构同构：

```
创建 stream → while stream.next().await → {
    累积 content → emit delta → 检查 finish_reason → DB 写入
} → 完成/错误处理
```

两处循环共 ~300 行，逻辑差异仅：
- compaction 多 30s 超时
- compaction 多摘要截断
- chat 多 tool-call delta emit
- chat 多 compaction 触发

### 2.2 `assistant-agent` crate 空转

当前 crate 仅含 `compaction.rs`（3 个函数 + 2 个常量 + 1 个 prompt），而 Agent 运行时的核心职责（消息构建、流式消费、取消管理）全在根包 `cmd/` 中。

### 2.3 不可测试

流式消费循环嵌在 Tauri command 函数内，依赖 `AppHandle` / `Emitter` / DB 连接，无法单元测试。提取后 `consume_stream` 为纯 async 函数，可用 mock stream 测试。

### 2.4 巨型函数

`send_chat_message` ~280 行，混合 IO / 逻辑 / 运行时 / 策略，认知负担高。

---

## 3. 提取方案

### 3.1 新增模块

```
assistant-agent/
  src/
    lib.rs
    compaction.rs          ← 已有（不变）
    chat_runtime.rs        ← 新增：consume_stream + prepare_messages + StreamOutcome
    chat_tasks.rs          ← 从根包搬入（ChatTasks + CancellationToken 管理）
```

### 3.2 `chat_runtime.rs` — 流式消费核心

#### 3.2.1 `StreamOutcome`

```rust
pub struct StreamOutcome {
    pub full_content: String,
    pub total_tokens: Option<i64>,
    pub prompt_tokens: Option<u32>,
    pub finish_reason: Option<String>,
    pub tool_calls: Option<Vec<assistant_providers::base::ToolCall>>,
}
```

- `tool_calls` 字段为 tool-call 执行循环（I16）预留
- 当前 `send_chat_message` 不使用此字段（普通 chat 模式下 `finish_reason: "tool_calls"` 等同于结束）

#### 3.2.2 `consume_stream`

```rust
pub async fn consume_stream<S, F>(
    stream: S,
    cancel_token: tokio_util::sync::CancellationToken,
    on_delta: F,
    timeout: Option<std::time::Duration>,
) -> Result<StreamOutcome, StreamError>
where
    S: futures::Stream<Item = Result<ChatDelta, ProviderError>> + Unpin,
    F: Fn(u32, &ChatDelta) + Send,
{
    // 统一的流式消费逻辑：
    // 1. while let Some(result) = stream.next().await
    // 2. 检查 cancel_token.is_cancelled()
    // 3. 累积 content / tokens / tool_calls
    // 4. 调用 on_delta(seq, &delta) — 由调用方决定 emit 逻辑
    // 5. 检查 finish_reason → 返回 StreamOutcome
    // 6. 超时包装（compaction 需要 30s）
}
```

**关键设计决策**：

| 决策 | 理由 |
|------|------|
| `on_delta` 为回调而非直接 emit | 解耦 Tauri `AppHandle`，使函数可测试 |
| `timeout` 为 `Option<Duration>` | chat 不设超时（None），compaction 设 30s |
| 返回 `StreamOutcome` 而非在函数内写 DB | DB 写入是 IO 侧职责，由调用方处理 |
| `cancel_token` 由外部传入 | 生命周期由 `ChatTasks` 管理，运行时不拥有 |

#### 3.2.3 `StreamError`

```rust
pub enum StreamError {
    Provider(ProviderError),
    Cancelled,
    Timeout,
}
```

#### 3.2.4 `prepare_messages`

```rust
pub fn prepare_messages(
    detail: &ConversationDetail,
    system_prompt: &str,
) -> Vec<ChatMessage>
```

从 `send_chat_message` 和 `execute_compaction` 中提取的消息构建逻辑：
- 过滤 `compact_summary` 消息
- `MessageRow` → `ChatMessage` 映射
- system prompt 插入首位

### 3.3 `chat_tasks.rs` — 取消/完成生命周期

从根包 `src/chat_tasks.rs` 整体搬入 `assistant-agent`，公开 API 不变：

```rust
pub struct ChatTasks(pub Mutex<HashMap<String, CancellationToken>>);

impl ChatTasks {
    pub fn new() -> Self;
    pub fn start(&self, conversation_id: String) -> CancellationToken;
    pub fn cancel(&self, conversation_id: &str);
    pub fn finish(&self, conversation_id: &str);
    pub fn is_running(&self, conversation_id: &str) -> bool;
}
```

搬入后根包 `src/chat_tasks.rs` 删除，`setup.rs` 改为 `use assistant_agent::ChatTasks`。

### 3.4 `cmd/chat.rs` 瘦身

提取后的 `send_chat_message` 变为：

```rust
#[command]
pub fn send_chat_message(app: AppHandle, conversation_id: String, content: String, mode_id: Option<String>) -> Result<(), String> {
    // 1. 参数校验 + is_full / terminating 守卫
    // 2. DB 读取（detail / provider / mode）
    // 3. insert user + assistant message
    // 4. prepare_messages（调用 crate）
    // 5. build ChatOptions
    // 6. adapter.chat_completion_stream(...)
    // 7. spawn: consume_stream + on_delta(emit) + DB 写入 + compaction 触发
    Ok(())
}
```

流式消费循环从 ~120 行降为 ~30 行（构造回调 + 处理 outcome）。

### 3.5 `cmd/conversation.rs` 的 `execute_compaction` 复用

```rust
pub(crate) async fn execute_compaction(...) -> Result<(), String> {
    // 1. prepare_messages（调用 crate）
    // 2. adapter.chat_completion_stream(...)
    // 3. consume_stream(stream, cancel_token, on_delta, Some(30s))  ← 复用
    // 4. 摘要截断
    // 5. 事务写入 DB + emit 事件
}
```

删除重复的流式消费代码，从 ~175 行降为 ~80 行。

---

## 4. 模块归属（提取后）

| 模块 | 归属 | 职责 |
|------|------|------|
| `compaction.rs` | assistant-agent | should_terminate + terminating 标记 + DEFAULT_COMPACT_SUMMARY_PROMPT（不变） |
| `chat_runtime.rs` | assistant-agent | consume_stream + prepare_messages + StreamOutcome + StreamError |
| `chat_tasks.rs` | assistant-agent | ChatTasks + CancellationToken 生命周期管理 |
| `cmd/chat.rs` | assistant-app | Tauri command 入口 + DB 读写 + emit 事件 + compaction 触发 |
| `cmd/conversation.rs` | assistant-app | execute_compaction 前置/后置 + manual_compact + unlock command |

---

## 5. 依赖关系

### 5.1 crate 依赖变更

`assistant-agent/Cargo.toml` 新增：

```toml
[dependencies]
assistant-providers = { path = "../assistant-providers" }  # ChatMessage, ChatDelta, ChatOptions, ProviderError
assistant-storage = { path = "../assistant-storage" }      # ConversationDetail, MessageRow（prepare_messages 需要）
tokio-util = { workspace = true }                           # CancellationToken
futures = { workspace = true }                              # Stream trait
```

### 5.2 根包依赖变更

`assistant-app/Cargo.toml`：
- 删除 `tokio-util` 直接依赖（改为通过 `assistant-agent` 间接使用）
- `chat_tasks.rs` 删除，改为 `use assistant_agent::ChatTasks`

### 5.3 依赖方向

```
assistant-app → assistant-agent → assistant-providers
                                → assistant-storage
```

不引入循环依赖。`assistant-agent` 不依赖 `assistant-app`（不依赖 AppHandle）。

---

## 6. 行为不变保证

| 行为 | 提取前 | 提取后 | 验证方式 |
|------|--------|--------|---------|
| 流式 delta 事件 | `orb_window.emit("chat-delta", ...)` | `on_delta(seq, &delta)` 回调内 emit | 前端 chat-delta 事件不变 |
| 取消 | `cancel_token.is_cancelled()` → emit error | 同上 | cancel_chat 行为不变 |
| DB 写入 | `db_tx.send((msg_id, content, tokens))` | consume_stream 返回 outcome 后调用方写 DB | 消息内容/token 不变 |
| compaction 触发 | `should_terminate` + spawn | 同上 | 自动终止行为不变 |
| compaction 超时 | `tokio::time::timeout(30s)` | `consume_stream(..., Some(30s))` | 超时行为不变 |
| 摘要截断 | `summary.truncate(max_chars)` | 调用方在 outcome 后截断 | 截断行为不变 |

---

## 7. 为 I16（tool-call 执行循环）预留的接口

### 7.1 `StreamOutcome.tool_calls`

当前 `consume_stream` 在收到 `finish_reason: Some("tool_calls")` 时，将累积的 `tool_calls` 填入 `StreamOutcome.tool_calls`。普通 chat 模式下调用方忽略此字段直接结束；I16 的执行循环读取此字段决定是否继续。

### 7.2 `StreamOutcome.finish_reason`

I16 执行循环的判断条件：

```rust
loop {
    let outcome = consume_stream(stream, ...).await?;
    match outcome.finish_reason.as_deref() {
        Some("tool_calls") => {
            // I16: 执行工具 → 拼结果消息 → 再次调用 Provider
            break; // 当前：普通 chat 直接结束
        }
        _ => break,
    }
}
```

### 7.3 `ChatOptions.tools`

当前 `ChatOptions` 不含 `tools` 字段。I16 需要新增 `tools: Option<Vec<ToolDefinition>>`，本次不新增（避免提前引入未使用的字段），但 `consume_stream` 的设计不阻碍后续添加。

---

## 8. 参考映射

| 功能点 | 参考项目 | 参考文件 | 借鉴内容 |
|--------|---------|---------|---------|
| 流式消费循环 | OpenBitFun | agent-runtime/sdk.rs | `StreamOutcome` 模式：消费一次流返回结构化结果，循环策略由调用方决定 |
| 取消生命周期 | OpenBitFun | agent-runtime/turn_cancellation.rs | `CancellationToken` 管理 + `ChatTasks` 按对话隔离 |
| 对话轮次统计 | OpenBitFun | agent-runtime/dialog_turn.rs | `TurnStats` / `TurnTokenUsage` — 本次不实现，但 `StreamOutcome` 的 token 字段为其预留 |
| 权限规划 | OpenBitFun | agent-runtime/permission.rs | `PermissionIntentPlan` 三态（Allowed/Denied/RequiresApproval）— I16 工具执行时参考 |

---

## 9. 验收标准

1. `consume_stream` 统一 `send_chat_message` 和 `execute_compaction` 的流式消费逻辑，无重复代码
2. `prepare_messages` 统一消息构建逻辑
3. `ChatTasks` 从根包搬入 `assistant-agent`，根包 `chat_tasks.rs` 删除
4. `assistant-agent` crate 依赖 `assistant-providers` + `assistant-storage`，不依赖 `assistant-app`
5. `consume_stream` 不依赖 `AppHandle` / `Emitter`，可单元测试
6. 所有外部可观测行为不变：chat-delta 事件、DB 写入、compaction 触发、取消、超时
7. `StreamOutcome` 含 `tool_calls` 字段，为 I16 预留
8. `cmd/chat.rs` 的 `send_chat_message` 从 ~280 行降至 ~150 行
9. `cmd/conversation.rs` 的 `execute_compaction` 从 ~175 行降至 ~80 行
10. cargo check + clippy + test 通过
11. 前端行为无变化（无前端改动）
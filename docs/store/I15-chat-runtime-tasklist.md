# I15 — Chat Runtime 提取任务清单

> 规格：[I15-chat-runtime-spec.md](./I15-chat-runtime-spec.md)
> 前置：I7 ✅

---

## 验证原则

**每一步提取紧跟一步等价性验证，从调用方视角证明"换完和没换一样"。**

调用方是 `send_chat_message` 和 `execute_compaction`。验证问题是：**调用方看到的副作用序列是否与提取前完全一致？**

6 类副作用（每步提取后必须确认未破坏）：

| 副作用 | 出现时机 |
|--------|---------|
| emit("chat-delta", delta_payload) | 每个 delta |
| db_tx.send((msg_id, content, tokens)) | 取消/完成/错误时 |
| finish_tx.send(conv_id) | 取消/完成/错误时 |
| compaction::should_terminate + spawn | finish_reason + prompt_tokens 超阈值 |
| emit("chat-delta", error_payload) | 取消/Provider error 时 |
| token 估算 (len+3)/4+4 | Provider 未报 usage |

---

## 阶段 A — crate 依赖 + 模块骨架

- [x] A1. `assistant-agent/Cargo.toml` 新增依赖：`assistant-providers` + `assistant-storage` + `tokio-util` + `futures`
- [x] A2. `assistant-agent/src/lib.rs` 新增 `pub mod chat_runtime;` + `pub mod prompt_builder;` + `pub mod chat_tasks;`
- [x] A3. 新建 `chat_runtime.rs`：定义 `StreamAccumulator` + `StreamOutcome`（含 `full_reasoning`）+ `StreamError` + `consume_stream` 函数签名
- [x] A4. 新建 `prompt_builder.rs`：定义 `AgentKind` enum + `prepare_messages` 签名 + `build_chat_options` 签名
- [x] A5. 新建 `chat_tasks.rs`：从根包 `src/chat_tasks.rs` 搬入 `ChatTasks`，扩展内部结构为 `HashMap<String, ChatTaskEntry>`（`ChatTaskEntry { cancel_token, attempt_counter }`），`start()` 返回值从 `CancellationToken` 扩展为 `(CancellationToken, attempt_id)`（格式 `${conversation_id}:${attempt_counter}`），同对话重入时递增 counter 并取消旧 token。搬入 `lock_map` poison recovery 辅助方法
- [x] **A6. ✅ 验证**：cargo check 通过（骨架无逻辑变更，编译通过即等价）

---

## 阶段 B — `StreamAccumulator` + `ToolCallAccumulator` 实现 + 验证

- [x] B1. 实现 `StreamAccumulator`：`new()` + `apply_delta(&mut self, delta: &ChatDelta) -> ApplyDeltaOutcome`（content/reasoning/tokens/tool_calls 各自累积，返回结构化信号）+ `into_outcome(self) -> StreamOutcome`（**消费 self**，构造后原 accumulator 不可再用。对齐 OpenBitFun `StreamContext::into_result(self)` lib.rs:398）
- [x] B2. 定义 `ApplyDeltaOutcome` struct：`finalized_previous: Option<ToolCall>` + `params_partial: Option<ToolCallParamsChunk>`（对齐 OpenBitFun `ToolCallDeltaOutcome` tool_call_accumulator.rs:134，I15 简化版不含 `early_detected`）
- [x] B3. 定义 `ToolCallAccumulator` struct：`Vec<PartialToolCall>` + 隐式 index 策略（与前端 `chatStore.appendToolCallDelta` 对齐）
- [x] B4. 实现 `ToolCallAccumulator::apply_delta(&mut self, delta: &ChatDelta) -> ToolCallDeltaOutcome`：`delta_tool_call_id` 非空 → push 新 PartialToolCall（若前一个未 finalize 则先 finalize 置入 `finalized_previous`）；否则追加到最后一个的 name/arguments_buf（对齐 OpenBitFun `PendingToolCalls::apply_delta` tool_call_accumulator.rs:449）
- [x] B5. 实现 `ToolCallAccumulator::finish(self) -> Result<Vec<ToolCall>, ToolCallAccumulateError>`：校验 JSON + 返回完整 tool calls（对应 OpenBitFun `ToolCallBoundary::StreamEnd` 边界，tool_call_accumulator.rs:7）。**max-tokens 截断策略**：当 `finish_reason == "max_tokens"` 时，arguments 不完整的 tool call 返回 `ToolCallAccumulateError::Truncated` 而非尝试 JSON 修复（对齐 deepseek-harness-orb `BlockAssembler.assembled()` 的丢弃策略，spec §3.2.5）
- [x] B6. `StreamAccumulator` 内维护 `has_effective_output: bool`：收到 `delta_text` 非空或 tool call 相关 delta 时设 true（thinking/reasoning 不算。对齐 OpenBitFun `StreamContext.has_effective_output` lib.rs:345）
- [x] **B7. ✅ 验证 StreamAccumulator 等价性**：构造与 `chat.rs` 内联循环相同的 delta 序列，断言 StreamAccumulator 累积结果与内联逻辑的 `full_content` / `last_tokens` / `last_prompt_tokens` 完全一致
  - 正常完成：3 个 text delta + finish_reason="stop"
  - reasoning 累积：2 个 reasoning delta + 1 个 text delta
  - token 累积：delta 带 usage{prompt=100,completion=50}
  - tool_calls 单个：id + name + 2 个 arguments delta
  - tool_calls 多个：2 组 id/name/arguments 交替
  - 空 stream：0 个 delta
  - finish_reason="tool_calls"
  - `has_effective_output`：仅 text/tool_call delta 触发，reasoning delta 不触发
  - **max-tokens 截断**：finish_reason="max_tokens" + tool call arguments 不完整 → `finish()` 返回 `Truncated` 错误
- [x] **B8. ✅ 验证 ToolCallAccumulator 等价性**：断言累积结果与前端 `appendToolCallDelta` 行为一致
  - 新 id → 新 tool call
  - 无 id + 有 name → 追加到最后一个 name
  - 无 id + 有 arguments → 追加到最后一个 arguments_buf
  - 非法 JSON → Error
  - 空 arguments → 合法
  - `apply_delta` 返回 `ToolCallDeltaOutcome`：新 id 到达时 `finalized_previous` 非空

---

## 阶段 C — `consume_stream` 实现 + 验证

- [x] C1. 定义 `TimedStreamItem<T>` 枚举：`Item(T)` / `End` / `TimedOut`，实现 `next_item` async 函数（对齐 OpenBitFun `TimedStreamItem` lib.rs:522 + `next_stream_item` lib.rs:528）
- [x] C2. 实现 `consume_stream` 主循环：构造 StreamAccumulator → loop { tokio::select! { cancel → StreamError::Cancelled(accum.has_effective_output) / next_item → match TimedStreamItem } } → accum.apply_delta + on_delta → 流结束调 accum.into_outcome
- [x] C3. 实现 `finish_reason` 检测 + `StreamOutcome` 构造（含 `has_effective_output` + `partial_recovery_reason: None`）
- [x] C4. 实现 timeout 包装：`next_item(stream, timeout)` 返回 `TimedStreamItem::TimedOut` → `StreamError::Timeout(accum.has_effective_output)`（I15 用总超时，I16 可升级为 watchdog 空闲超时，只需改 `next_item` 内部逻辑）
- [x] C5. 实现 `StreamError` 转换：`Provider(err, accum.has_effective_output)` / `Cancelled(accum.has_effective_output)` / `Timeout(accum.has_effective_output)`（对齐 OpenBitFun `StreamProcessError.has_effective_output` lib.rs:286）
- [x] C6. `delta_reasoning` 累积到 `StreamAccumulator.full_reasoning`（不阻断流，仅累积供 DB 持久化）
- [x] C7. `delta_retry` 透传：`consume_stream` 不消费 retry 通知，仅通过 `on_delta` 回调传递
- [x] **C8. ✅ 验证 consume_stream 等价性**：用 mock stream（`futures::stream::iter`）构造与 `chat.rs` 内联循环相同的 delta 序列，断言 consume_stream 的 StreamOutcome 与内联循环的最终状态一致
  - 正常完成：mock 3 个 delta → StreamOutcome { full_content, finish_reason, has_effective_output=true, partial_recovery_reason=None }
  - 取消：mock 中途 cancel_token.cancel() → StreamError::Cancelled(has_effective_output)
  - 超时：mock stream + timeout=1ms → StreamError::Timeout(has_effective_output)
  - Provider error：mock 含 Err(ProviderError) → StreamError::Provider(_, has_effective_output)
  - reasoning 累积：mock 含 reasoning delta → StreamOutcome.full_reasoning 非空，has_effective_output 不因 reasoning 而设 true
  - 空 stream：mock 0 个 delta → StreamOutcome { full_content="", has_effective_output=false }
- [x] **C9. ✅ 验证 on_delta 回调等价性**：收集回调参数到 `Arc<Mutex<Vec<serde_json::Value>>>`（对齐 OpenBitFun `RecordingEventSink` 模式），断言 seq 序列 + delta 内容与 `chat.rs` 内联循环的 emit 序列一致

---

## 阶段 D — `prompt_builder` 实现 + 验证

- [x] D1. 实现 `AgentKind` enum：`from_mode_kind` + `supports_tool_call` + `persists_reasoning`
- [x] D2. 实现 `prepare_messages`：过滤 `compact_summary` + `MessageRow` → `ChatMessage` 映射 + system prompt 插入 + `append_user_prompt` 可选追加
- [x] D3. 实现 `build_chat_options`：按 `AgentKind` 分派 temperature / max_tokens / reasoning 配置，替代内联 fallback 链
- [x] **D4. ✅ 验证 prepare_messages 等价性**：用 `chat.rs` 当前内联消息构建的输入数据调用 `prepare_messages`，断言输出 Vec<ChatMessage> 与内联构建结果完全一致
  - 过滤 compact_summary
  - system prompt 插入
  - append_user_prompt=None（chat 场景）
  - append_user_prompt=Some(...)（compaction 场景）
- [x] **D5. ✅ 验证 build_chat_options 等价性**：用 `chat.rs` 当前内联 fallback 链的输入数据调用 `build_chat_options`，断言输出 ChatOptions 与内联 fallback 链结果完全一致
  - Default + mode.temperature=Some / None
  - Quick / Deep / Custom
- [x] **D6. ✅ 验证 AgentKind**：`from_mode_kind` + `supports_tool_call` + `persists_reasoning` 对 4 种 kind 值的映射

---

## 阶段 E — DB schema 扩展 + 验证

- [x] E1. `CREATE TABLE messages` 新增 `reasoning TEXT` + `tool_calls_json TEXT` 列（新安装路径）
- [x] E2. 已有数据库自动 ALTER TABLE（沿用 pragma_table_info 模式，init_db 时检测列是否存在）
- [x] E3. `MessageRow` struct 新增 `reasoning: Option<String>` + `tool_calls_json: Option<String>` 字段
- [x] E4. `update_message_content` 签名扩展：接受 `reasoning` + `tool_calls_json` 参数（全调用方已同步更新）
- [x] E5. `get_conversation_detail` / `get_messages_paginated` 补充 `reasoning` / `tool_calls_json` 列（`MSG_SELECT` + `read_msg_row` + `insert_message` 已同步改）
- [x] **E6. ✅ 验证 DB 向后兼容**：cargo test -p assistant-storage 33 tests passed
- [x] **E7. ✅ 验证前端加载映射**：`reasoning` 直接透传（后端字段名 `reasoning` = 前端字段名 `reasoning`）；`tool_calls_json` → `toolCalls` 映射 I16 设计统一前后端消息协议时一并解决，I15 不做临时 JSON.parse 补丁

---

## 阶段 F — 根包瘦身：`send_chat_message` 逐项替换 + 逐项验证

> 核心原则：每次只替换一个内联片段，替换后立即验证调用方副作用不变。

- [x] F1. 替换内联消息构建为 `prepare_messages` 调用（`append_user_prompt = None`）
- [x] **F2. ✅ 验证**：cargo test + 手动发一条消息，确认 chat-delta 事件序列不变（消息内容、顺序、system prompt 位置）
- [x] F3. 替换内联 ChatOptions 构建为 `build_chat_options` 调用
- [x] **F4. ✅ 验证**：cargo test + 手动发一条消息，确认 temperature / reasoning 配置行为不变
- [x] F5. `ChatTaskEntry` 扩展：`{ cancel_token, attempt_counter }`，`start()` 返回 `(CancellationToken, attempt_id)`，`attempt_id` 格式 `${conversation_id}:${attempt_counter}`，同对话重入时递增 counter + 取消旧 token
- [x] **F6. ✅ 验证**：cargo test -p assistant-agent，确认 start/cancel/finish/attempt_counter 行为正确
- [x] F7. `chat-stream-start` 事件：流开始前 emit `chat-stream-start`（含 `conversation_id` / `attempt_id` / `message_id`）
- [x] F8. 所有 `chat-delta` 事件携带 `attempt_id` 字段
- [x] F9. 替换内联流式循环为 `consume_stream` 调用 + on_delta 回调（emit chat-delta 全字段 + attempt_id）
- [x] **F10. ✅ 验证 emit 序列快照**：c9_on_delta_callback_sequence 验证 seq + delta 类型/内容等价性（crate 层）；完整 payload 字段验证留 I16 集成测试
  - 正常完成：3 个 text delta + finish → 全字段 + attempt_id
  - 取消：中途取消 → error 字段（ProviderError::Cancelled）+ attempt_id
  - Provider error → error 字段 + attempt_id
- [x] F11. DB 写入从 outcome 取值：`content` → `outcome.full_content`，`tokens` → `outcome.total_tokens`（None 时估算），`reasoning` → `outcome.full_reasoning`（**仅当 `kind.persists_reasoning()` 为 true 时写入**，即仅 Deep 模式持久化 reasoning），`tool_calls_json` → `outcome.tool_calls` 序列化（I16）
- [x] **F12. ✅ 验证**：手动发消息 → 查 DB，断言 content / tokens / reasoning 值与提取前一致（最新 assistant 消息：content=368字符, tokens=172, reasoning=空(非Deep模式)）
- [x] F13. token 估算保留在 cmd 层（`outcome.total_tokens.is_none()` 时用 `(len+3)/4+4`）
- [x] **F14. ✅ 验证**：Provider 未报 usage 时，断言 DB 中 tokens 为估算值（日志确认：`no_usage=>estimated=812 (content_len=3231)`，公式 `(3231+3)/4+4=812` 正确）
- [x] F15. `StreamError` 变体从 `bool` 升级为 `PartialOutcome`：取消/错误路径携带 `full_content`/`total_tokens` 等部分累积结果，调用方可将部分内容写入 DB
- [x] F16. `StreamAccumulator::into_partial(self)` 方法：消费 accumulator 构造 `PartialOutcome`，与 `into_outcome(self)` 对称
- [x] F17. cmd 层辅助函数拆分（spec §3.5.1）：`emit_delta` / `emit_stream_error` / `save_partial_on_error` / `spawn_db_writer` / `spawn_finish_listener` / `spawn_compaction_listener` / `resolve_context_window` / `estimate_tokens` / `resolve_final_tokens` / `resolve_partial_tokens`
- [x] F18. `StreamEmitCtx` 结构体：统一 emit 上下文（`orb_window` + `conv_id` + `attempt_id` + `msg_id`），零分配借用
- [x] F19. compaction channel 解耦：`spawn_compaction_listener` 返回 `UnboundedSender`，闭包内仅需 `compact_tx.send(...)` 一行
- [x] F20. `AgentModeRow::default()` + `AgentKind::default()`：消除手动构造默认值

---

## 阶段 F2 — 前端 `attempt_id` 协议适配

> Spec §3.4 + §6 行为不变表：attempt_id 过滤、streamingMessageId 对齐、editMessage 时序窗口

- [x] F2-1. 前端 `chatStore` 新增 `activeAttemptId: string | null` 状态
- [x] F2-2. 前端监听 `chat-stream-start` 事件：设置 `activeAttemptId` + 替换 `streamingMessageId` 为后端真实 `message_id`
- [x] F2-3. 前端 `handleDeltaEvent` 统一按 `attempt_id` 过滤所有事件类型（error / finish_reason / reasoning / tool_call / usage / retry）：`delta.attempt_id !== activeAttemptId` → 忽略
- [x] F2-4. `cancelStreaming()` 不再清 `streamingMessageId` / `activeAttemptId`（消除 editMessage 时序窗口：cancel 后保留旧 ID，chat-stream-start 到达时原子替换）
- [x] **F2-5. ✅ 验证**：快速连续发消息 → 旧请求事件自动忽略，新请求事件正常处理；editMessage 重发 → 无事件丢失

---

## 阶段 G — 根包瘦身：`execute_compaction` 逐项替换 + 逐项验证

> **设计改进**：compaction 可被用户取消。compaction 启动时调用 `chat_tasks.start()` 获取 cancel_token，传入 `consume_stream`。用户调 `cancel_chat` 即可同时取消 compaction。chat 和 compaction 对同一对话不会同时运行（`manual_compact` 已守卫 `is_running`，自动 compaction 在 chat finish 后触发），共享 entry 语义正确。

- [x] G1. 替换内联消息构建为 `prepare_messages` 调用（`append_user_prompt = Some(summary_prompt)`）
- [x] **G2. ✅ 验证**：触发 compaction，确认摘要内容不变
- [x] G3. 替换内联流式循环为 `consume_stream` 调用（timeout = 30s，cancel_token 从 `chat_tasks.start()` 获取，on_delta 仅 emit `delta_text` + `finish_reason`），迁移 `tokio_stream::StreamExt` → `futures::StreamExt`
- [x] **G4. ✅ 验证 emit 序列快照**：断言 compaction 的 emit payload 仅含 delta_text + finish_reason（无 reasoning / tool_call / retry 字段）
- [x] G5. 摘要截断 + 事务写入从 outcome 取值
- [x] **G6. ✅ 验证**：触发 compaction → 查 DB，断言摘要内容与提取前一致
- [x] G7. compaction 取消路径：用户调 `cancel_chat` → compaction 中断 → `set_terminating(false)` + emit `conversation_compact_failed`（error: Cancelled）。已插入的空 summary 消息保留（role=compact_summary 不参与对话，前端可渲染取消状态）
- [x] **G8. ✅ 验证 compaction 取消**：compaction 运行中调 cancel_chat → compaction 停止 → 对话状态恢复（is_full 不变，terminating 清除）

---

## 阶段 H — 收尾清理 + 验证

- [x] H1. 删除根包 `src/chat_tasks.rs`，全局替换 `use crate::chat_tasks::ChatTasks` → `use assistant_agent::chat_tasks::ChatTasks`（涉及 `setup.rs`、`cmd/chat.rs`）
- [x] H2. 根包 `Cargo.toml` 清理：移除 `tokio-util` 直接依赖（如无其他引用）；移除 `tokio-stream` 直接依赖（仅 `conversation.rs` 使用，提取后不再需要）
- [x] **H3. ✅ 验证**：cargo check + clippy + test 通过
- [x] **H4. ✅ 验证 ChatTasks 等价性**：start / cancel / finish / is_running 行为不变

---

## 阶段 I — 手动 E2E 验证（最终兜底）

> 前面每步已验证过等价性，此阶段是端到端场景的最终确认。

- [ ] I1. 正常发送：发消息 → 等流式完成 → 前端显示完整回答 → DB 有消息
- [ ] I2. 取消：发消息 → 中途点取消 → 前端显示已输出部分 → DB 有部分内容
- [ ] I3. 深度模式 reasoning 持久化：选深度模式 → 发消息 → 前端显示 reasoning + 回答 → 刷新页面 → 历史消息仍含 reasoning
- [ ] I4. 自动终止：长对话触发 compaction → 自动终止行为不变
- [ ] I5. 手动终止：点手动终止 → 终止 + 解锁行为不变
- [ ] I6. Provider 错误：模拟 Provider 返回 error → 前端显示错误提示
- [ ] I7. compaction 超时：compaction 超过 30s → 超时行为不变
- [ ] I8. compaction 取消：compaction 运行中点取消 → compaction 停止 → 对话状态恢复（terminating 清除，空 summary 消息保留）

---

## 阶段 J — 最终验收

- [x] J1. cargo check + clippy + test 通过
- [x] J2. 验收标准 spec §9 全部通过

---

## I16+ 预留说明

> 以下为跨参考项目分析后识别的 I16+ 升级方向，I15 不实现但需知晓设计预留。

| 方向 | 参考项目 | 关键参考代码 | spec 章节 |
|------|---------|-------------|----------|
| settle/abandon 双路径 | deepseek-harness-orb | `AssistantStreamAttempt.settle()/abandon()` (agent-loop/src/assistant-stream.ts:97/113) | §7.9 |
| 编排运行时 | airi | `ChatOrchestratorRuntime` (core-agent/src/runtime/chat-orchestrator-runtime.ts) | §7.10 |
| 工具输出截断 | deepseek-harness-orb | `TextRetainer` (util/output-retention/src/index.ts) | §7.11 |
| FinishReason 结构化 | deepseek-harness-orb | `FinishReasonMap` (llm/src/types.ts) | §7.12 |
| 瞬态重试 | airi + OpenBitFun | `transientRetryDelayMs` (core-agent/src/runtime/generation.ts) + `should_retry` (agent-stream/retry.rs:7) | §7.13 |
| 流累积拆分为压缩层 + 组装层 | deepseek-harness-orb | `AssistantStreamAccumulator` + `BlockAssembler` (llm/src/assembler.ts) | §3.2.1 |
| tool 调度器 | deepseek-harness-orb | `executeToolCalls` (agent-loop/src/tool-calls.ts) | §7.1 |
| response categoriser | airi | `createStreamingCategorizer` (core-agent/src/runtime/response-categoriser.ts) | — |

---

## 验证策略总览

核心原则：**逐步提取，逐步验证**。每一步替换后立即从调用方视角确认副作用不变，而非最后统一验证。

```
提取流程（每步紧跟验证）：

A 骨架 ──→ A6 ✅编译
                │
B StreamAccumulator ──→ B7 ✅累积等价 / B8 ✅ToolCall等价
                │
C consume_stream ──→ C8 ✅循环等价 / C9 ✅回调等价
                │
D prompt_builder ──→ D4 ✅消息等价 / D5 ✅选项等价 / D6 ✅AgentKind
                │
E DB扩展 ──→ E6 ✅向后兼容
                │
F chat瘦身 ──→ F2 ✅消息 / F4 ✅选项 / F6 ✅ChatTaskEntry / F10 ✅emit快照 / F12 ✅DB写入 / F14 ✅token估算
                │
F2 前端attempt_id ──→ F2-5 ✅过滤+时序窗口
                │
G compaction瘦身 ──→ G2 ✅消息 / G4 ✅emit快照 / G6 ✅DB写入 / G8 ✅取消
                │
H 收尾 ──→ H3 ✅编译 / H4 ✅ChatTasks
                │
I 手动E2E ──→ 7个场景兜底
                │
J 最终验收
```

如果任何一步验证失败，**只需回退该步替换**，不影响前面已验证通过的步骤。
# I15 — Chat Runtime 提取任务清单

> 规格：[I15-chat-runtime-spec.md](./I15-chat-runtime-spec.md)
> 前置：I7 ✅

---

## 阶段 A — crate 依赖 + 模块骨架

- [ ] A1. `assistant-agent/Cargo.toml` 新增依赖：`assistant-providers` + `assistant-storage` + `tokio-util` + `futures`
- [ ] A2. `assistant-agent/src/lib.rs` 新增 `pub mod chat_runtime;` + `pub mod chat_tasks;`
- [ ] A3. 新建 `chat_runtime.rs`：定义 `StreamOutcome` + `StreamError` + `consume_stream` 函数签名
- [ ] A4. 新建 `chat_tasks.rs`：从根包 `src/chat_tasks.rs` 搬入 `ChatTasks` struct + impl

---

## 阶段 B — `consume_stream` 实现

- [ ] B1. 实现 `consume_stream` 核心逻辑：stream 消费 + cancel 检查 + content/tokens 累积 + on_delta 回调
- [ ] B2. 实现 `finish_reason` 检测 + `StreamOutcome` 构造
- [ ] B3. 实现 `tool_calls` 累积（从 `ChatDelta` 的 `delta_tool_call_*` 字段收集，使用 `ToolCallAccumulator`）
- [ ] B4. 实现 timeout 包装（`Option<Duration>`，None 不设超时，Some 设 `tokio::time::timeout`）
- [ ] B5. 实现 `StreamError` 转换：Provider error / Cancelled / Timeout

---

## 阶段 C — `prepare_messages` 实现

- [ ] C1. 实现 `prepare_messages`：过滤 `compact_summary` + `MessageRow` → `ChatMessage` 映射 + system prompt 插入
- [ ] C2. 验证 `prepare_messages` 输出与当前 `send_chat_message` 内联逻辑一致

---

## 阶段 D — 根包瘦身

- [ ] D1. `cmd/chat.rs` 的 `send_chat_message`：替换内联消息构建为 `prepare_messages` 调用
- [ ] D2. `cmd/chat.rs` 的 `send_chat_message`：替换内联流式循环为 `consume_stream` 调用 + on_delta 回调（emit chat-delta）
- [ ] D3. `cmd/chat.rs` 的 `send_chat_message`：DB 写入从 outcome 取值
- [ ] D4. `cmd/conversation.rs` 的 `execute_compaction`：替换内联消息构建为 `prepare_messages` 调用
- [ ] D5. `cmd/conversation.rs` 的 `execute_compaction`：替换内联流式循环为 `consume_stream` 调用（timeout = 30s）
- [ ] D6. `cmd/conversation.rs` 的 `execute_compaction`：摘要截断 + 事务写入从 outcome 取值
- [ ] D7. 删除根包 `src/chat_tasks.rs`，`setup.rs` 改为 `use assistant_agent::ChatTasks`
- [ ] D8. 根包 `Cargo.toml` 清理：移除 `tokio-util` 直接依赖（如无其他引用）

---

## 阶段 E — 测试

- [ ] E1. `consume_stream` 单元测试：正常完成（finish_reason = "stop"）
- [ ] E2. `consume_stream` 单元测试：取消（cancel_token 取消）
- [ ] E3. `consume_stream` 单元测试：超时
- [ ] E4. `consume_stream` 单元测试：Provider error
- [ ] E5. `consume_stream` 单元测试：tool_calls 累积
- [ ] E6. `prepare_messages` 单元测试：过滤 compact_summary + system prompt 插入
- [ ] E7. `ChatTasks` 测试：start / cancel / finish / is_running

---

## 阶段 F — 集成验证

- [ ] F1. cargo check + clippy + test 通过
- [ ] F2. 前端行为无变化（手动验证：发送消息 + 取消 + 自动终止 + 手动终止 + 解锁）
- [ ] F3. 验收标准 spec §9 全部通过
# I36 — P3 运行时内核任务清单

> 规格：[I36-agent-runtime-spec.md](./I36-agent-runtime-spec.md) Step 0-3、7-9 与 §10 验收标准
> 阶段：[I38-agent-roadmap.md](./I38-agent-roadmap.md) P3
> 决策：[I36-agent-runtime-design.md](./I36-agent-runtime-design.md)
> 顺序：C0 → C1 → C2 → C3 → C4 → C5 → C6 → C7 → C8 → C9 → C11 → C12（C10 为 P4 交接，不在交付范围）
> 勾选条件：代码已改 **且** 对应验收已跑。
> **状态唯一位置**：本清单是 P3 任务级进度的唯一来源；阶段级结论只在 [roadmap](./I38-agent-roadmap.md) §2 写一次，spec / design / 基线 / 验证文档都不记录进度。本清单**不管理提交**（不写提交拆分 / 提交信息 / 回滚批次，见 roadmap §7 第 2 条）。
> 范围：`assistant-agent/src/kernel/` 共用内核、`src-tauri/src/adapters/` 端口实现、`src-tauri/src/cmd/agent/` 宿主命令、chat 等价迁移、事件 DTO、`messages.status`、轮级观测。
> 不包含：RoundExecutor 多迭代循环（P4 / I38b）、`agent/` 下的停止钩子 / 中间件链 / 阶梯 ③-⑤ / 摘要执行体（见 C10 交接；审批门本体已在 P2 `assistant-tools/src/gateway.rs`，P4 只做宿主侧装配）、任何工具实现（P5 / I38c）、前端 UI（P6 / I38d）。
> 测试位置：新增单测一律放 `crates/assistant-agent/tests/<模块>.rs` 集成测试；端口替身放 `kernel/ports/test_doubles.rs` 且仅 `#[cfg(test)]`。
> 硬边界：`kernel/` 下不得出现 `tauri` / `AppHandle` / `Window` / `tauri::State`，不得 `use` 任何 `src-tauri` 类型。

---

## 验收映射

| 验收号 | 内容 | 清单项 |
|---|---|---|
| 1 | `cargo tree` 无 tauri；kernel grep 无 tauri 类型 | C9.4、C9.5 |
| 2 | 仅用替身跑完一整轮 | C2.19 |
| 3 | `EventSink` 失败不中止 turn | C2.17 |
| 4 | `MessageStore` 失败 → `status='failed'` | C2.18 |
| 5 | `TurnSpec::chat()/agent()` 字段值；无 `is_agent/is_chat` | C1.17、C1.21、C4.7 |
| 6 | chat 四组输入事件序列 / seq / 落库 / 文案逐条一致 | C5.30 |
| 7 | agent 入口 `Busy` 不取消；chat `Replace` 顶替 | C5.23、C4.11 |
| 8 | 取消后落库 + `status='interrupted'` + 回 `Idle` | C7.11、C4.4 |
| 9 | 旧 turn 迟到完成不写库 / 不改状态 / 不发事件 | C3.9、C4.13 |
| 10 | `turn_wall_clock` 与 `model_call` 可分别触发超时 | C4.14、C5.3 |
| 11 | 有推理无正文重试 1 次仍空 → `Failed` 且 iteration 不递增 | C5.17 |
| 12 | 无 orb 窗口仍可跑完并落库 | C5.28、C5.29 |
| 21 | 事件恒带 `turn_id`；同 turn `seq` 严格单调 | C6.9 |
| 22 | 流式中强制退出重启后 `status='interrupted'` | C7.9 |
| 23 | `contract:gen` 后前端可编译，`assembly_contract` 通过 | C6.8、C6.11、C9.7 |
| 13-20 | 阶梯 / 钩子 / 压缩生命周期 | 交接 C10（随 P4 验收） |
| 24 | 旧模块退役：`chat_runtime` / `chat_tasks` / `chat_request` 删除且无残留引用 | C12.12 |
| 24a | 无 orb 窗口时压缩仍完成并落库 `kind="compaction"` usage | C12.13 |
| 25 | 内核测试以 `--features test-doubles` 跑，五套用例数均 > 0 | C12.14 |

---

## C0 口径与前置确认

- [ ] C0.1 确认 P3 只改 `assistant-agent`、新增 `src-tauri/src/adapters/` 与 `src-tauri/src/cmd/agent/`、`protocol` DTO、`assistant-storage` 迁移、`setup.rs` / `ipc.rs` 注册；不动 `assistant-tools` 与前端组件
- [ ] C0.2 确认 `kernel/` 零 tauri：不出现 `tauri` / `AppHandle` / `Window` / `tauri::State`，不 `use` `src-tauri` 类型
- [ ] C0.3 确认内核访问 DB / 事件 / 输入 / 时钟只经 `kernel/ports.rs` 四个端口
- [ ] C0.4 确认 Step 0-2 不改变 chat 可观测行为；chat 行为只在 Step 3 迁移
- [ ] C0.5 确认 P3 的 `TurnRunner` 内跑单迭代过渡循环，`round_executor = None`（roadmap R2）
- [ ] C0.6 确认 `messages` 迁移只做 ADD COLUMN，不改写存量数据
- [ ] C0.7 确认 P3 不新增工具、不改 `Tool` trait / `ToolRegistry`
- [ ] C0.8 确认旧实现删除时机：对比测试全绿后才删 `chat_runtime` / `chat_tasks` / `chat_request`；`prompt_builder` / `chat_options` 为共享层，保留（spec §3.5.1）
- [ ] C0.9 确认线字段仍叫 `attempt_id`，`TurnId` 只作为内核别名，不改前端解析

---

## C1 Step 0 — 值类型

实现：`src-tauri/crates/assistant-agent/src/kernel/{ids,phase,outcome,limits,mod}.rs`、`src-tauri/crates/assistant-agent/src/lib.rs`、`src-tauri/crates/assistant-agent/Cargo.toml`
测试：`src-tauri/crates/assistant-agent/tests/kernel_values.rs`

- [x] C1.1 `Cargo.toml` 增加 `async-trait = { workspace = true }`（原生 async fn in trait 不兼容 `dyn`）
- [x] C1.2 `lib.rs` 增加 `pub mod kernel;`
- [x] C1.3 `ids.rs`：`pub type TurnId = String`；`TurnContext { conversation_id, turn_id, iteration, provider_attempt, started_at }`
- [x] C1.4 `ids.rs` 注释写明 `iteration` / `provider_attempt` 不落库、不进 `MessageDto`
- [x] C1.5 `phase.rs`：`TurnPhase` 七个值（`Starting` / `Compacting` / `Thinking` / `Streaming` / `ToolCalling` / `AwaitingApproval` / `Finalizing`）+ `serde(rename_all = "camelCase")`
- [x] C1.6 `phase.rs`：`SessionState::{Idle, Processing{turn_id, phase}, Error{message, recoverable}}`
- [x] C1.7 `outcome.rs`：`TurnStatus::{Completed, Cancelled, Failed, Interrupted}`
- [x] C1.8 `outcome.rs`：`TurnOutcome` 十一字段（status / text / iterations / model_calls / tool_calls / prompt_tokens / completion_tokens / cache_read_tokens / hit_cap / wrap_up_injected / error）
- [x] C1.9 `outcome.rs`：`TurnError`，含 `MiddlewareStop { name, reason }`（Step 4 预留）
- [x] C1.10 `outcome.rs`：`hit_cap` 与 `wrap_up_injected` 独立表达，不合并
- [x] C1.11 `limits.rs`：`TurnLimits` 六字段 + `Default`（12 / 派生 / 30min / 10min / 1 / 6）
- [x] C1.12 `limits.rs`：`effective_tool_call_limit(inner, outer) = outer.map_or(inner, |o| inner.min(o))`
- [x] C1.13 `mod.rs`：`OnBusy` / `EventRoute` / `LadderPolicy` 三个枚举
- [x] C1.14 `mod.rs`：`TurnSpec` 七字段 + `TurnSpec::chat()` / `TurnSpec::agent()`（C5.13 追加 `kind` / `limits`，现共九字段，见 spec §0B.2.1 偏差记录）
- [x] C1.15 `chat()` 字段值：`Replace` / 1 迭代 / `tools = None` / `ProjectionOnly`（含终止式压缩）/ orb 路由 / `wrap_up = false`
- [x] C1.16 `agent()` 字段值：`Reject` / 12 迭代 / `tools = Some` / `Full` / broadcast 路由 / `wrap_up = true`
- [x] C1.17 单测：`chat()` 与 `agent()` 逐字段断言（验收 5）
- [x] C1.18 单测：`TurnLimits::default()` 与 `max_tool_calls` 派生值（最小 8）
- [x] C1.19 单测：`effective_tool_call_limit` 只收紧不放宽
- [x] C1.20 单测：`TurnPhase` / `SessionState` / `TurnStatus` 序列化为 camelCase
- [x] C1.21 断言：`kernel/` 内 grep 无 `is_agent` / `is_chat`（写成 `tests/kernel_values.rs` 的文件扫描用例）

---

## C2 Step 0B — 端口与共用内核骨架

实现：`src-tauri/crates/assistant-agent/src/kernel/ports.rs`、`kernel/ports/test_doubles.rs`、`src-tauri/src/adapters/{mod,event_sink,message_store,input_source,system_clock}.rs`、`src-tauri/src/lib.rs`
测试：`src-tauri/crates/assistant-agent/tests/kernel_ports.rs`

- [x] C2.1 `ports.rs`：`TurnEvent` 九个变体（`StreamStart` / `Delta` / `PhaseChanged` / `ToolStarted` / `ToolFinished` / `CompactionStarted` / `CompactionFinished` / `Terminated` / `UserMessageConfirmed`）
- [x] C2.2 `ports.rs`：`EventSink::emit` 返回 `()`，失败只记日志
- [x] C2.3 `ports.rs`：`MessageStore` 四方法（`create_assistant_message` / `save_content` / `finalize` / `record_usage`）+ `CreateAssistantReq` / `StoreError`
- [x] C2.4 `ports.rs`：`TurnInputSource` 三方法（`conversation_detail` / `provider` / `attachments`）+ `TurnInput` / `InputError` / `ProviderCredential`
- [x] C2.5 `ports.rs`：`Clock::now()`；`MessageStatus` 四值
- [x] C2.6 `ports.rs` 不 `use` 任何 `src-tauri` 类型；`attachments` 复用 `assistant-storage::AttachmentStore`
- [x] C2.7 `test_doubles.rs`：`RecordingSink`（`Vec<TurnEvent>` 逐条可比对）
- [x] C2.8 `test_doubles.rs`：`InMemoryStore`（记录 `create` / `save` / `finalize` / `usage` 调用序列，可注入失败）
- [x] C2.9 `test_doubles.rs`：`FixtureSource`（固定会话详情与凭据）
- [x] C2.10 `test_doubles.rs` 仅 `#[cfg(test)]`，不进生产依赖
- [x] C2.11 `adapters/mod.rs` 新建并在 `src-tauri/src/lib.rs` 注册 `mod adapters;`
- [x] C2.12 `adapters/event_sink.rs`：`TurnEvent` → `chat-delta` / `chat-stream-start` / `session-state-changed` / 终止类映射，内核不写事件名字面量
- [x] C2.13 `adapters/event_sink.rs`：`EventRoute::Orb` 窗口缺失直接 return，`Broadcast` 走 `app.emit`，**禁止**返回 `Err`
- [x] C2.14 `adapters/message_store.rs`：基于 `DbState` 调 `conversations` / `token_usage` 现有函数
- [x] C2.15 `adapters/input_source.rs`：`AppHandle` state + `crypto::get_machine_id` 解密 provider key
- [x] C2.16 `adapters/system_clock.rs`：`SystemClock` 实现 `Clock`
- [ ] C2.17 单测：`RecordingSink` 抛错时 turn 不中止、日志有记录（验收 3）
- [ ] C2.18 单测：`InMemoryStore` 失败时 turn 失败且 `finalize(failed)` 被调用（验收 4）
- [ ] C2.19 验收 2：仅用 `RecordingSink` / `InMemoryStore` / `FixtureSource` / 暂停时钟跑完一整轮，不启动 Tauri

---

## C3 Step 1 — 会话状态机

实现：`src-tauri/crates/assistant-agent/src/kernel/state.rs`
测试：`src-tauri/crates/assistant-agent/tests/kernel_state.rs`

- [x] C3.1 `SessionStates(Arc<Mutex<HashMap<String, SessionState>>>)` 结构
- [x] C3.2 实现 `ensure_idle` / `get` / `can_start_new_turn` / `begin_turn` / `set_phase` / `finish_turn` / `remove` 七方法
- [x] C3.3 `can_start_new_turn` 判据为 `Idle | Error{recoverable:true}`
- [x] C3.4 `begin_turn` 在不可开始时返回 `StateError::Busy`
- [x] C3.5 `set_phase` 校验 `turn_id` 是否当前代际，不匹配静默丢弃 + `log::warn!`
- [x] C3.6 `finish_turn` 是 `Processing` 的唯一出口，且必然写入 `Idle` 或 `Error`
- [x] C3.7 mutex poison 时恢复（对齐现有 `ChatTasks::lock_map` 写法）
- [x] C3.8 单测：spec §1.2 迁移表十二行逐条覆盖
- [x] C3.9 单测：过期 `turn_id` 的 `set_phase` / `finish_turn` 无效（验收 9 的状态侧）
- [x] C3.10 单测：`Processing` 下 `begin_turn` 返回 `Busy`；`Error{recoverable:true}` 下允许
- [x] C3.11 单测：`remove` 后回到「未记录」状态且 `can_start_new_turn` 为真

---

## C4 Step 2 — 取消与代际守卫

实现：`src-tauri/crates/assistant-agent/src/kernel/tasks.rs`（`select!` 包装作为 `runner.rs` 内联辅助，不新增模块）
测试：`src-tauri/crates/assistant-agent/tests/kernel_tasks.rs`

- [x] C4.1 `TurnTasks(Mutex<HashMap<String, TurnEntry>>)` + `TurnEntry { token, counter, turn_id }`
- [x] C4.2 `begin(conv, OnBusy::Replace)`：取消旧令牌、`counter += 1`、返回新 `turn_id`
- [x] C4.3 `begin(conv, OnBusy::Reject)`：返回 `Busy`，不动旧令牌
- [x] C4.4 `cancel(conv) -> bool`：只置位令牌，不 `JoinHandle::abort()`
- [x] C4.5 `finish(conv, turn_id)`：恒校验代际，非当前代际不生效
- [x] C4.6 `current(conv) -> Option<TurnId>`
- [x] C4.7 `turn_id` 格式固定为 `{conversation_id}:{counter}`，计数只增（验收 5）
- [x] C4.8 子令牌层级：model stream / tool call / approval wait 三个 `child_token`
- [x] C4.9 所有长 await 用 `tokio::select! { biased; _ = child.cancelled() => ..., r = fut => r }`（统一走 `runner::cancellable()`）
- [x] C4.10 代际三写侧：事件发射 / 终态落库 / 状态迁移，缺一即漏洞
- [x] C4.11 单测：`Replace` 顶替旧 turn、`Reject` 拒绝且旧 turn 继续跑（验收 7）
- [x] C4.12 单测：`finish` 传旧 `turn_id` 不移除新 turn
- [x] C4.13 单测：旧 token 迟到完成 → 不写库、不改状态、不发终止事件（验收 8 / 9）
- [x] C4.14 单测：`turn_wall_clock` 与 `model_call` 分别触发超时；工具执行不受 `model_call` 约束（验收 10）

---

## C5 Step 3 — TurnRunner 与 chat 等价迁移

实现：`src-tauri/crates/assistant-agent/src/kernel/{stream,request,persist,emit,runner}.rs`、`src-tauri/src/cmd/agent/mod.rs`、`src-tauri/src/cmd/chat/{mod,stream}.rs`、`src-tauri/src/cmd/mod.rs`、`src-tauri/src/ipc.rs`、`src-tauri/src/setup.rs`
测试：`src-tauri/crates/assistant-agent/tests/kernel_stream.rs`、`tests/kernel_runner.rs`、`tests/chat_equivalence.rs`

**流式消费**

- [x] C5.1 `stream.rs`：内核自有 `StreamOutcome` / `PartialOutcome` / `StreamError::{Cancelled, Timeout, Provider}`，不借用 `chat_runtime` 同名类型
- [x] C5.2 `consume(stream, token, timeout, on_delta)` 实现，`seq` 起点 0、单调 +1
- [x] C5.3 单次超时取 `min(model_call, turn 剩余)`（验收 10）
- [x] C5.4 取消返回 `Cancelled(PartialOutcome)`，已累积内容不丢
- [x] C5.5 不做 provider 重试、不落库、不发事件、不判空响应
- [x] C5.6 单测：`seq` 起点与单调性对齐现状（验收 6 的 seq 维度）
- [x] C5.7 单测：取消 / 超时 / provider 错误三种分支均返回部分结果

**请求构建与落库**

- [x] C5.8 `request.rs`：`build(spec, input, iteration) -> ChatRequest`，顺序为投影 → 多模态 → 提示词 → tools
- [x] C5.9 `request.rs` 不读 DB，输入全由端口给
- [x] C5.10 `persist.rs`：部分结果 / 终态 / usage 三类写入，失败即 turn 失败
- [x] C5.11 `emit.rs`：`TurnEvent` 生成 + `seq` 单调分配
- [x] C5.12 `emit.rs` 不映射线事件名（由 `adapters/event_sink.rs` 负责）

**Runner**

- [x] C5.13 `runner.rs`：`TurnRunner` 八字段，`round_executor: Option<Arc<dyn RoundExecutor>> = None`（`limits` 收进 `TurnSpec`，见 §0B.2.1 偏差记录）
- [x] C5.14 定义最小 `RoundExecutor` trait 占位（只有 `run_iterations` 签名），P4 填实现；内核不得 `use` 工具类型。**实际落点 `kernel/mod.rs`**（P3 无 `agent/` 目录，见 spec §0B.2.1 偏差记录）；P4 迁至 `agent/round_executor.rs`（I38b §2.2）
- [x] C5.15 `run(ctx, spec) -> TurnOutcome` 为唯一入口
- [x] C5.16 主流程七步：刹车 → 阶梯 → 单次调用 → 空响应重试 → 无工具结束 → 收尾 → 工具执行
- [x] C5.17 空响应重试 `continue` **不**递增 `iteration`；重试一次仍空 → `Failed`（验收 11）
- [ ] C5.18 收尾调用占用第 `max_iterations` 个名额，不额外加轮
- [x] C5.19 `persist_terminal` 先于 `states.finish_turn`（先落库、后改状态）
- [x] C5.20 单测：正常完成 / 取消 / provider 报错 / 空响应四组 `TurnOutcome` 断言

**宿主命令与 chat 迁移**

- [x] C5.21 新建 `src-tauri/src/cmd/agent/mod.rs`，`cmd/mod.rs` 注册 `mod agent;` 与 `pub use agent::*;`
- [x] C5.22 `send_agent_message`：只做入参校验 → `TurnTasks::begin` → `spawn(runner.run)` → `TurnOutcome` 映射
  - 落地：命令只设 `spec.tools = Some(RegistryToolCatalog)`，其余与 `send_chat_message` 共用 `start_turn`；命令本身不做选路
- [x] C5.23 `send_agent_message` 在 `Processing` 下返回 `Busy` 且不取消正在跑的 turn（验收 7）
- [x] C5.24 `ipc.rs` 注册新命令（`send_agent_message` / `cancel_agent`）
- [x] C5.25 `cmd/chat/mod.rs` 的 `send_chat_message` 改调 `TurnRunner::run(ctx, TurnSpec::chat())`，宿主只保留校验 / begin / spawn / 返回值映射
- [x] C5.26 `spawn_chat_stream` 的编排职责迁入 `kernel/persist.rs` + `kernel/emit.rs` + `adapters/`
- [x] C5.27 `cmd/chat/stream.rs` 只保留 `StreamEmitCtx` → `EventSink` 适配，或整体删除
  - 落地：整体删除；`chat-messages-changed` / `conversation-list-changed` 广播移入 `src-tauri/src/adapters/event_sink.rs` 的 `emit_stream_finished`
- [x] C5.28 去掉发送路径对 orb 窗口的存在性要求，`error.orb_window_not_found` 不再出现在发送链路（验收 12）
  - 落地：`TauriEventSink` 统一走 `emit_orb_only`，窗口缺失只 `log::error!`，不阻断 turn
- [ ] C5.29 单测：关闭 / 从未创建 orb 窗口时 agent turn 仍跑完并落库（验收 12）
- [ ] C5.30 对比测试：正常完成 / 中途取消 / provider 报错 / 空响应四组，`TurnEvent` 序列、`seq`、落库内容、错误文案逐条一致（验收 6）
- [x] C5.31 对比测试全绿后删除 `chat_runtime.rs` / `chat_tasks.rs` / `chat_request.rs`，并从 `lib.rs` 移除 `pub mod`（`prompt_builder.rs` / `chat_options.rs` 保留，见 C12.11）
  - 偏差：转 C12 执行（spec §3.5）。`src-tauri/src/cmd/compaction.rs` 仍依赖 `chat_runtime` 的 `build_usage_records` / `consume_stream` / `PartialOutcome`，`src-tauri/src/cmd/conversation.rs` 仍依赖 `chat_request`；须先迁调用点再删
  - 落地：C12.1–C12.10 完成后三模块已删除，`lib.rs` 仅保留共享层 `prompt_builder` / `chat_options`
- [x] C5.32 `setup.rs` 的 `manage(ChatTasks::new())` 换成 `manage(TurnTasks::new())`
  - 落地：同时 `manage(SessionStates::new())`，二者均为 `Arc<TurnTasks>` / `Arc<SessionStates>` 单例
- [x] C5.33 `cancel_chat` 改调 `TurnTasks::cancel`
- [ ] C5.34 前端 `chat_streamCharacterization` / `chatOrchestrator` 两套测试保持全绿
- [x] C5.35 确认没有运行时开关保留双路径（禁止 `if use_kernel { ... } else { ... }`）

---

## C6 Step 7 — 事件与 DTO

实现：`src-tauri/src/protocol/dto/chat.rs`、`src-tauri/src/protocol/events.rs`、`src/types/protocol.contract.ts`
测试：`src-tauri/crates/assistant-agent/tests/kernel_emit.rs`、`src-tauri/tests/protocol_dto_roundtrip.rs`

- [x] C6.1 `ChatDeltaPayloadDto` 增加 `iteration: u32`（输出 DTO，非 `Option`，默认 0）——`protocol_contract.rs::chat_delta_payload_dto_round_trip`
- [x] C6.2 `ChatStreamStartPayloadDto` 增加 `phase: TurnPhaseDto`
- [x] C6.3 新增 `SessionStateChangedPayloadDto`（`conversation_id` / `turn_id` / `state`）
- [x] C6.4 新增 `SessionStateDto` 封闭枚举：`idle` / `processing{turnId, phase}` / `error{message, recoverable}`
- [x] C6.5 `TurnPhaseDto` 与 Rust `TurnPhase` 同源，不手写第二份——`From<TurnPhase>` 用穷尽 `match`；`protocol_contract.rs::turn_phase_dto_serializes_identically_to_kernel_turn_phase`
- [x] C6.6 输出 DTO 的 `Option` 不加 `skip_serializing_if`（键恒在、值 `null`）
- [x] C6.7 `events.rs` 新增 `SESSION_STATE_CHANGED = "session-state-changed"`
- [x] C6.8 `npm run contract:gen` 并提交 `src/types/protocol.contract.ts`
- [x] C6.9 单测：每条事件都带 `turn_id`；同 turn 内 `seq` 严格单调，跨 turn 归零（验收 21）——`kernel_emit.rs` 四条用例
- [x] C6.10 `session-state-changed` 不带 `seq`（它是状态快照）——`protocol_contract.rs::session_state_changed_payload_wire_shape`
- [x] C6.11 前端按 `turn_id` 过滤、按 `seq` 排序的类型可编译（验收 23）——`chatTypes.ts` 导出 `TurnPhase`/`SessionState`/`SessionStateChangedPayload` 与 `ChatEvent.SessionStateChanged`，`chatStreamEvents.ts` 订阅；`npm run typecheck` 通过

---

## C7 Step 8 — 终态持久化与启动清扫

实现：`src-tauri/crates/assistant-storage/src/db.rs`、`src-tauri/crates/assistant-storage/src/conversations/msg_ops.rs`、`src-tauri/src/adapters/message_store.rs`、`src-tauri/src/cmd/chat/insert.rs`、`src-tauri/src/setup.rs`
测试：`src-tauri/crates/assistant-storage/tests/{db,conversations}.rs`、`src-tauri/crates/assistant-agent/tests/kernel_runner.rs`

- [x] C7.1 `CREATE TABLE messages` 增加 `status TEXT NOT NULL DEFAULT 'completed'`
- [x] C7.2 按现有 pragma 模板增加 `ALTER TABLE messages ADD COLUMN status TEXT NOT NULL DEFAULT 'completed'`
- [x] C7.3 `MessageStatus` 四值与 SQL 取值一致：`streaming` / `completed` / `interrupted` / `failed`
- [x] C7.4 创建助手消息时写 `streaming`；用户消息恒 `completed`
- [x] C7.5 终态映射：正常结束 → `completed`；取消 → `interrupted`；出错有部分内容 → `failed`；触顶未收尾 → `completed` + `hit_cap` 事件
- [x] C7.6 `setup.rs` 在 DB 可用之后、主窗口创建之前执行 `UPDATE messages SET status='interrupted' WHERE status='streaming'`
- [x] C7.7 清扫失败只 `log::warn!`，不重试、不阻塞启动
- [x] C7.8 不做「推断停在第几轮」、不续跑、不删除消息
- [x] C7.9 单测：流式中强制退出后重启，该消息 `status='interrupted'`（验收 22）——`restart_sweep_marks_streaming_message_interrupted`
- [x] C7.10 迁移测试：旧库（无 status 列）升级后列存在且存量行值为 `completed`——`init_db_adds_status_column_to_legacy_messages_table`
- [x] C7.11 单测：取消后消息内容已落库且 `status='interrupted'`，状态回到 `Idle`（验收 8）——`cancel_mid_stream_saves_partial_then_finalizes_interrupted`（含 Save 早于 Finalize 的顺序断言）

---

## C8 Step 9 — 观测

实现：`src-tauri/crates/assistant-agent/src/kernel/{stream,outcome,runner}.rs`
测试：`src-tauri/crates/assistant-agent/tests/kernel_outcome.rs`

- [x] C8.1 `TurnOutcome` 增加 `ttft_ms: Option<u64>`——`outcome.rs`；`StreamOutcome` / `PartialOutcome` 同步透传
- [x] C8.2 `stream::consume` 首次回调处记录 `now - request_started` 写入 `TurnOutcome`——`StreamAccumulator::record_ttft`，`consume` 增 `clock` / `request_started` 参数（时间单源，不读 `Instant::now()`）
- [x] C8.3 `finish_turn` 输出一条 `info` 级固定字段日志：`conv / turn / status / iterations / model_calls / tool_calls / prompt / completion / cache_read / ttft_ms / elapsed_ms / hit_cap`——`TurnRunner::log_turn_summary`（TTFT 缺省记 `-`）
- [x] C8.4 不引入 TTFM（design §3.19）
- [x] C8.5 上下文占用沿用 provider usage 的 `context_window_tokens`，不做三段拆分
- [x] C8.6 单测：无 delta 时 `ttft_ms = None`；有 delta 时非零——`kernel_outcome.rs` 两条用例（`ttft_is_none_when_stream_emits_no_delta` / `ttft_records_first_delta_only`）

---

## C9 校验与文档收尾

- [x] C9.1 `cargo test -p assistant-agent` 全绿（含新增四套集成测试）
- [x] C9.2 `cargo clippy -p assistant-agent --all-targets -- -D warnings` 无告警
- [x] C9.3 先执行格式化修复，再 `cargo fmt --all --check` 通过
- [x] C9.4 `cargo tree -p assistant-agent` 无 `tauri`（验收 1）——仅 `assistant-providers` / `assistant-storage` 依赖
- [x] C9.5 grep 确认 `assistant-agent/src/kernel/` 无 `AppHandle` / `Window` / `tauri::State`（验收 1）——命中仅注释与 `test_doubles.rs` 的测试替身
- [x] C9.6 `cargo test --workspace --no-run` 全量编译通过
- [x] C9.7 `cargo test --test assembly_contract` 通过（验收 23）——8 passed
- [x] C9.8 `npm run contract:gen` 幂等（以生成前后契约文件哈希比对验证）——`protocol.contract.ts` / `vault.contract.ts` 哈希不变
- [x] C9.9 前端 `npm test` 中 chat 相关两套测试全绿——`src/chat` + `src/services` 11 文件 137 用例通过
- [x] C9.10 更新 I36 spec「模块落点索引」（原「实施状态总览」）中的模块落点与归属阶段
- [x] C9.11 更新 I38 roadmap §2 P3 状态与 §4 文档地图

---

## C10 交接给 P4（不在本清单交付范围）

以下模块在 spec 中位于 `agent/`，依赖工具执行闭环与 RoundExecutor，**不随 P3 落地**，P4（I38b）开工时按 spec 对应章节实现：

| spec 章节 | 落点 | 依赖 | 对应验收 |
|---|---|---|---|
| Step 4 中间件链与阶梯 ③④⑤ | `assistant-agent/src/agent/middleware.rs`、`assistant-agent/src/agent/ladder.rs` | P4 工具执行 | 13-16 |
| Step 4 阶梯 ②（摘要执行体） | `assistant-agent/src/agent/compaction_exec.rs` | **不随 P4**，随续跑式压缩同批（I38b D13） | 移出 P4 验收 |
| Step 5 停止钩子 | `assistant-agent/src/agent/stop.rs` | P4 迭代循环 | 17-18 |
| Step 6 压缩生命周期 | 手动终止式留在 `src-tauri/src/cmd/compaction.rs`（P3 已迁移内核调用）；`agent/compaction_exec.rs` 随续跑式批次 | **轮内续跑不随 P4**（I38b D13） | 19-20 归续跑式批次 |
| 审批门（端口与网关） | `assistant-tools/src/gateway.rs` | 已在 P2 落地 | 不适用 |
| 审批门生产装配（`TimingApprovalPrompter`、两表、`Deny` 默认实现） | `src-tauri/src/agent/`、`src-tauri/src/adapters/` | P4（**不新建 `agent/approval.rs`**） | I38b C4.14 / C4.18 |

P3 交付的内核已为这些模块预留接入点：`TurnSpec.stop_hooks` / `TurnSpec.ladder` / `TurnSpec.tools` / `TurnError::MiddlewareStop` / `TurnOutcome.hit_cap` / `wrap_up_injected`。

> 预留接入点≠已接线：P3 内 `ladder` / `wrap_up` / `stop_hooks` 无读取点，按 I36b V28–V30 记为「未接线」，不得记「已实现」。

---

## C11 一致性收口（I36b H1–H9）

> 判定口径与证据见 [I36b-agent-kernel-conformance-spec.md](./I36b-agent-kernel-conformance-spec.md) §4。C11.1–C11.7 属 P3 收口；C11.8 是 P4 开工阻断项。

- [x] C11.1（H1）模型调用可注入：`TurnInput` 携带 `adapter`，`kernel/request.rs` 不再调用 `create_provider_adapter`；L1 用例零监听端口
- [x] C11.2（H2）时间单源：`TurnContext` 起点由 `Clock` 注入，`src/kernel/` 内无 `Instant::now()`
- [x] C11.3（H3）落库时间戳可注入：`PersistCtx` 带 `created_at_ms`，`persist` 内无 `Utc::now()`
- [x] C11.4（H4）时钟可推进：`ManualClock` + `advance`；超时用例改由虚拟时间触发，用例内无 `sleep`
- [x] C11.5（H6）单次调用超时口径定稿并回写 I36 spec §3.2 与验收 10，补「整段封顶」用例
- [x] C11.6（H7）未接线字段清单断言：`kernel_values` 显式列出 `ladder` / `wrap_up` / `stop_hooks` 为 P4 接线
- [x] C11.7（H9）`TurnEvent` 可稳定序列化（或 `TurnTrace` 投影），黄金轨迹两遍一致
- [x] C11.8（H1 / H8，P4 前置）L1 全部改脚本化适配器，真实 TCP 用例降级到 `slow-tests` 特性

---

## C12 旧实现退役与压缩链路迁移（spec §3.5）

> 目的：清掉「内核已就位、旧调用点未迁」的遗漏代码，让 C5.31 的删除真正可执行。
> 范围：`cmd/compaction.rs` 迁移、`cmd/conversation.rs` 占用查询迁移、测试引用迁移、三模块退役。
> 不含：`kernel/runner.rs` 的 P3 过渡单迭代循环（P4 / I38b 删除）、`agent/` 四模块（`stop` / `middleware` / `ladder` / `compaction_exec`，C10 交接）、阶梯 ③-⑤ 与续跑语义（P4）、阶梯 ② 与摘要执行体（续跑式压缩批次，I38b D13）。
> 现状：`CompactionSink` 端口与 `adapters/compaction_sink.rs` 已存在且由 `runner.rs` 接线，**不新建 adapter、不改触发时序**；只替换 `execute_compaction` 内部依赖。

实现：`src-tauri/src/cmd/compaction.rs`、`src-tauri/src/cmd/conversation.rs`、`src-tauri/crates/assistant-agent/src/lib.rs`
测试：`src-tauri/crates/assistant-agent/tests/{kernel_stream,kernel_persist,kernel_tasks,attachment_budget,prompt_options_spec,token_estimate}.rs`、`cargo test -p assistant-agent --features test-doubles`、`cargo test --workspace --no-run`

**压缩链路迁移**

- [x] C12.1 `cmd/compaction.rs` 流消费改 `kernel::stream::consume`，注入 `adapters::SystemClock::new()` 与 `request_started = clock.now()`，超时保持 `Some(Duration::from_secs(30))`；移除 `chat_runtime::{consume_stream, StreamError, PartialOutcome}` 引用
- [x] C12.2 `cmd/compaction.rs` usage 记录改 `kernel::persist::build_usage_records` + `PersistCtx{kind:"compaction", branch_leaf_id: active_branch_id, created_at_ms: clock.now_millis()}`；移除 `chat_runtime::{build_usage_records, UsageRecordCtx}`
- [x] C12.3 `cmd/compaction.rs` 的 stream-start / delta 改经 `kernel::emit::TurnEmitter`（`turn_id = attempt_id`，`stream_start(TurnPhase::Compacting)` + `delta`），删除手写 `emit_orb_only` 调用与 `AtomicU32` seq 自增；增量事件必须在 `tasks.finish` 之前发射（代际校验）
  - 落地：终态改经 `TurnEmitter::terminated(TurnStatus, Option<ProviderErrorPayload>)`，在 `tasks.finish` 之前发射；三条失败分支的 `emit_orb_only` 与 `seq_val` 手写块全部删除
- [x] C12.4 `cmd/compaction.rs` 删除 orb 窗口硬依赖（`ok_or_else(|| "Orb window not found")`），窗口缺失只 `log::error!`；压缩同样受「后台可运行」约束（验收 12）
- [x] C12.5 `cmd/compaction.rs` 三条失败分支（取消 / 超时 / provider 错误）的 usage 记录与终态事件口径与迁移前逐条一致
  - 落地：三条分支仍先 `usage_ctx.usage_records(&partial.attempt_usages)` 落库再 `save_compaction_partial`，错误文案（`Compaction cancelled` / `Summary generation timed out (30s)` / provider 原文）与错误载荷不变

**占用查询迁移**

- [x] C12.6 `cmd/conversation.rs` 的 `estimate_branch_tokens` 改调 `kernel::request::estimate_branch_tokens`，移除 `assistant_agent::chat_request` 引用

**测试引用迁移**

- [x] C12.7 `tests/attachment_budget.rs` 改 `kernel::request::{project_chat_messages, estimate_branch_tokens}`
- [x] C12.8 `tests/usage_records.rs` 改 `kernel::persist::{build_usage_records, PersistCtx}` + `kernel::stream::UsageSnapshot`，迁入新建 `tests/kernel_persist.rs`（`created_at_ms` 用固定值，保持断言可复现）
  - 落地：原 7 例全量迁入，另补 1 例断言 `created_at_ms` 注入生效（共 8 例）
- [x] C12.9 `tests/chat_runtime_spec.rs` 中 b7 `StreamAccumulator` 7 个用例迁到 `tests/kernel_stream.rs`（改 `kernel::stream::StreamAccumulator`）；内核现无 accumulator 单测，不迁则 usage 合并覆盖随旧模块一起丢失
- [x] C12.9a c8 / c9 的 9 个 `consume_stream` 用例与 `tests/kernel_stream.rs` 逐条对照：已覆盖者删除，未覆盖者补进 `kernel_stream.rs`
  - 落地：逐条对照后 `kernel_stream.rs` 已覆盖全部 9 例，另补 `empty_stream_completes_without_output` 一例
- [x] C12.9b d4 `prepare_messages` 5 个、d5 `build_chat_options` 12 个、d6 `agent_kind` 2 个用例改 use 到 `prompt_builder` / `chat_options`，拆到独立测试文件
  - 落地：新建 `tests/prompt_options_spec.rs`，共 20 例（5 + 12 + 2 + 1 拆分）
- [x] C12.9c g4 / g6 / g7 的 5 个压缩用例改 `kernel::stream::consume` 重写，保留 partial outcome 与「summary = full_content」断言
- [x] C12.9d `chat_tasks` 3 个用例与 `tests/kernel_tasks.rs` 对照（已覆盖可删，未覆盖补入）
  - 落地：`kernel_tasks.rs` 10 例已覆盖 attempt_id 格式 / 会话独立 / cancel+finish 三条语义
- [x] C12.9e `estimate_text_tokens` 3 个用例改 `token_estimate::estimate_text_tokens`
  - 落地：新建 `tests/token_estimate.rs`

**退役与断言**

- [x] C12.10 删除 `chat_runtime.rs` / `chat_tasks.rs` / `chat_request.rs`，`lib.rs` 移除对应 `pub mod`
- [x] C12.11 `prompt_builder.rs` / `chat_options.rs` 保留，`lib.rs` 注释标注「共享层，非 chat 独占」，并确认 `kernel/request.rs` 仍引用
- [x] C12.12 grep 断言：`assistant-agent/src`、`assistant-agent/tests`、`src-tauri/src` 内无 `chat_runtime` / `chat_tasks` / `chat_request` 残留（验收 24）
  - 证据：grep 三处目录零命中（含注释与文档字符串）
- [x] C12.13 补测试或手工验收：无 orb 窗口时手动压缩仍完成、`kind="compaction"` usage 落库、事件序列与有窗口时一致（验收 24a；与 C5.29 同口径，覆盖压缩链路）
  - 若无法在测试环境构造无窗口场景，改为手工验收并在本条记录证据（操作步骤 + 日志 / 落库查询结果）
  - 证据（代码级）：`execute_compaction` 已无 `get_webview_window` 调用；`protocol::events::emit_orb_only` 在窗口缺失时静默跳过（不阻断、不改事件形状）；usage 落库走 `PersistCtx{kind:"compaction"}` 与窗口无关
  - 待运行时复验：启动应用（不创建 orb 窗口）手动触发压缩，核对 `token_usage.kind='compaction'` 落库与事件序列
- [x] C12.14 `cargo test -p assistant-agent --features test-doubles` 全绿，且 `kernel_emit` / `kernel_outcome` / `kernel_request_tools` / `kernel_runner` / `kernel_stream` 用例数均 > 0（验收 25）
  - 实测：`kernel_emit` 5 / `kernel_outcome` 2 / `kernel_request_tools` 4 / `kernel_runner` 12 / `kernel_stream` 22；`kernel_persist` 8 / `kernel_tasks` 10 / `prompt_options_spec` 20 / `token_estimate` 3
- [x] C12.15 `cargo test --workspace --no-run`、`cargo clippy -p assistant-agent --all-targets -- -D warnings`、`cargo fmt --all --check` 通过
# I38b — P4 Agent Loop 闭环任务清单

> 规格：[I38b-agent-loop-spec.md](./I38b-agent-loop-spec.md) §7 实施步骤、§8 验收标准、§9.1 决策定稿
> 阶段：[I38-agent-roadmap.md](./I38-agent-roadmap.md) P4
> 顺序：C0 → C1 → C2 → C3 → C4 → C5 → C6 → C7 → C8 → C9
> 勾选条件：代码已改 **且** 对应验收已跑。
> **状态唯一位置**：本清单是 P4 任务级进度的唯一来源；阶段级结论只在 [roadmap](./I38-agent-roadmap.md) §2 写一次，spec / design / 基线 / 验证文档都不记录进度。本清单**不管理提交**（不写提交拆分 / 提交信息 / 回滚批次，见 roadmap §7 第 2 条）。
> 范围：只交付 RoundExecutor 多迭代循环、tool call 累积器、`ChatOptions.tools` 注入、P2 遗留生产端口绑定、事件 DTO 与 tool 落库；**阶梯只做 ③④⑤（② 摘要压缩随续跑式压缩批次，D13）**；不新增 OS 工具实现、不做前端审批 UI、不做 MCP、不做压缩续跑式切换、不做子代理。
> 测试位置：`assistant-agent` 单测一律放 `crates/assistant-agent/tests/<模块>.rs` 集成测试，不写 `#[cfg(test)]` 内联模块；`src-tauri` 侧端到端测试放 `src-tauri/tests/`。
> 决策基线：D1-D26 已全部定稿（§3.2 / §9.1），开工后不再新增决策；遇到未覆盖问题先回写规格，不在代码里临时拍板。

> **前置清理（已完成，2026-11-05）**：`src-tauri/tests/protocol_bindings.rs` 原 `assert_eq!(output.matches("export type ").count(), 63, ...)` 计数断言已删除，替换为「`src/protocol/dto` 中 `#[derive(...TS)]` 类型集合 ↔ `declarations` 类型集合」双向差集断言（另加一条 `declarations` 重复注册检查）。原因：计数断言在「新增 DTO 但漏加 `declarations`」时仍然通过（输出数不变），只在「加了条目却忘改数字」时失败，属于魔数提醒器而非校验器；同仓 `assistant-vault` 的 `export_bindings` 无对应计数，说明原写法是孤例。清理后 P4 新增两个 DTO 只需往 `declarations` 加两条，**不再需要改任何数字**。规格侧痕迹见 I38b §6.1.1 / A10 / §9 风险表，历史侧痕迹见 I38a 任务清单 C3.11 注记。

---

## C0 口径与前置确认

- [x] C0.1 确认 P3 已通过验收：`kernel/` 端口、状态机、取消代际、双层超时、chat 等价迁移可用（P4 只替换迭代循环体）
- [x] C0.2 确认 P2 已通过验收：`ToolPipeline`（`find → validate → gateway → execute → persist`）、`partition_tool_batches`、`select_persist_targets` 均可用
- [x] C0.3 确认 P4 **不改** `assistant-tools` 任何代码：分批、落盘、结果事件全在 Pipeline 内部完成，executor 不重复实现（§4.5 职责切分）
- [x] C0.4 确认 P4 不实现 OS 工具（P5）、前端五态与审批 UI（P6）、MCP（P7）、压缩续跑式（D13）、子代理（P8）
- [x] C0.5 确认 D1-D26 与 I36 D10 的边界（「模型可见历史落库、运行时簿记不落库」）已理解一致（§6.2）
- [x] C0.6 确认协议改动一律走 `npm run contract:gen`，禁止手改 `src/types/protocol.contract.ts`
- [x] C0.7 确认 `MessageDto` 本阶段不动（工具卡片字段归 P6）

---

## C1 累积器迁移与 provider 快照语义（P4-C1，D1-D9）

实现：`src-tauri/crates/assistant-agent/src/agent/tool_call.rs`、`src-tauri/crates/assistant-providers/src/**`
测试：`src-tauri/crates/assistant-agent/tests/tool_call.rs`

- [x] C1.1 新建 `assistant-agent/src/agent/tool_call.rs`，`agent/mod.rs` 增加 `pub mod tool_call;`
- [x] C1.2 定义 `ToolCallStreamKey { Indexed(usize), Unindexed }`（D1），Anthropic 有 index 用 `Indexed`、否则 `Unindexed`
- [x] C1.3 定义 `ToolCallBoundary { NewTool, FinishReason, StreamEnd }`（D2），保留 `finalized_previous` 语义
- [x] C1.4 实现 `apply_delta`（增量入口）与 `apply_unified_response`（`UnifiedResponse` 入口，D4）
- [x] C1.5 实现跨 chunk 累积：`Indexed` 按 index 分组、`Unindexed` 单槽累积
- [x] C1.6 实现 `arguments_is_snapshot` 分支：`true` 用快照替换、`false` 追加（D8）
- [x] C1.7 截断处理按 D3：先 `repair_truncated_json`，修复可用则降级（`is_error = false` + 修复提示），失败才报 `Truncated`
- [x] C1.8 非法 JSON 不 panic，产出可回灌的失败结果
- [x] C1.9 `ChatDelta` 新增 `delta_tool_call_arguments_is_snapshot: bool`（D9，provider 内部流类型，非 wire）
- [x] C1.10 OpenAI adapter 填 `arguments_is_snapshot = false`
- [x] C1.11 Anthropic adapter 填 `false`，有 index 时带 `Indexed` 键
- [x] C1.12 Gemini adapter 只透传快照片段并标 `true`，累积统一交给 `agent/tool_call.rs`（D8，修 F15/F16）
- [x] C1.13 删除 `assistant-providers/src/tool_call_accumulator.rs` 及其 `lib.rs` 导出（A7）
- [x] C1.13a `kernel/stream.rs` 的 `StreamAccumulator` 删除 `tool_calls` 字段与 `apply_tool_call`，只保留正文 / 推理 / usage / `ttft_ms` / `finish_reason` / `has_effective_output`（§3.1）
- [x] C1.13b `StreamOutcome` / `PartialOutcome` 删除 `tool_calls` 字段；工具片段改由 `consume` 的 `on_delta` 回调原样交给调用方累积
- [x] C1.13c 删除后 `kernel/runner.rs` 的过渡循环暂不可编译 → **C1 与 C5 必须同批交付**（见 C5.2）
- [x] C1.14 全仓 grep 确认无第二份累积器实现：`assistant-providers` 无 `ToolCallAccumulator`，`assistant-agent/src/kernel/stream.rs` 无 `ToolCallAccum`（A7 口径见 spec §3.1）
- [x] C1.15 `tool_call.rs` 不出现 `tauri` / `AppHandle` / `assistant_tools` 引用
- [x] C1.16 测试：跨 chunk 累积（`Unindexed` 与 `Indexed` 双键）
- [x] C1.17 测试：Gemini 快照替换语义
- [x] C1.18 测试：三种 `ToolCallBoundary` 的终结行为
- [x] C1.19 测试：截断 JSON 修复成功 / 失败两条路径
- [x] C1.20 测试：非法 JSON 产出失败结果不 panic
- [x] C1.21 测试：三 adapter 对同一逻辑调用产出等价的 `FinalizedToolCall`（A6 前半）

---

## C2 tools 通道与三 adapter 注入（P4-C2，D10、§4.1-§4.3）

实现：`assistant-providers/src/types/tool.rs`、`types/config.rs`、`openai/**`、`anthropic/**`、`gemini/**`、`assistant-agent/src/kernel/ports.rs`、`src-tauri/src/adapters/**`
测试：`src-tauri/crates/assistant-providers/tests/`、`src-tauri/src/agent/` 装配测试

- [x] C2.1 定义 `ToolSpec { name, description, input_schema }`，`serde(rename_all = "camelCase")`（§4.1）
- [x] C2.2 `ChatOptions` 新增 `tools: Option<Vec<ToolSpec>>`，`#[serde(default, skip_serializing_if = "Option::is_none")]`（§4.2）
- [x] C2.3 `build_chat_options` 新增 `tools` 参数并填入返回结构（不新增第二个构造函数）
- [x] C2.4 同步两处 `ChatOptions` 完整字面量构造：`chat_options.rs`、`cmd/compaction.rs`（压缩摘要显式 `tools: None`，不得用 `..Default::default()`）
- [x] C2.5 `kernel/ports.rs` 声明 `ToolCatalog` async trait（`agent_tools() -> Vec<ToolSpec>`，D16）；**同时删除 `kernel/mod.rs` 的同步占位 `ToolCatalog` / `EmptyToolCatalog` 及其导出**，全仓只留一套
- [x] C2.5a `TurnSpec.tools: Option<Arc<dyn ToolCatalog>>` 与 `runner.rs` 的 `spec.tools.is_none()` 判定改到新签名（P3 过渡循环随 C5 删除）
- [x] C2.6 `src-tauri` 实现 `ToolCatalog`：调 `ToolRegistry::tool_definitions_for_tier(ToolTier::Os)`，逐条转 `ToolSpec`
- [x] C2.7 `max_tier` 常量放适配器内，`External` 排除（D15）；`fs_link` 为 `TOOLBOX_ONLY` 不会出现
- [x] C2.8 注入开关：`TurnSpec.tools` 有值（仅 agent 链路传入）且 `AgentKind::supports_tool_call()` 为真（现状 `custom`）才取工具；`default` / `quick` / `deep` 恒 `None`（A9）
- [x] C2.9 `ToolCatalog` 返回空 `Vec` 时 `tools = None`，循环退化为单次调用（A9）
- [x] C2.10 OpenAI `ChatRequest` 新增 `tools: Option<Vec<Value>>` + `skip_serializing_if`，构造点 `openai/chat.rs` 填值
- [x] C2.11 `openai/message_converter.rs` 新增 `ToolSpec` → `{ type: "function", function: { name, description, parameters } }` 转换
- [x] C2.12 Anthropic `MessagesRequest` 新增 `tools` 字段，构造点 `anthropic/request.rs` 填值
- [x] C2.13 `anthropic/message_converter.rs` 新增 `ToolSpec` → `{ name, description, input_schema }` 转换（无 `parameters` 包壳）
- [x] C2.14 Gemini `convert_tools` 签名由 `&[(String, String, String)]` 改为 `&[ToolSpec]`
- [x] C2.15 去掉 `convert_tools` / `sanitize_schema` / `strip_unsupported_schema_fields` 上的 `#[allow(dead_code)]`（转正）
- [x] C2.16 Gemini `request.rs` 构造点填 `convert_tools(...)`；`code_assist.rs` 保持 `None`（不改 Code Assist 行为）
- [x] C2.17 Anthropic 回填 `tool_use.input` 从字符串改为 JSON 对象，解析失败回退 `{}`（§4.3 必须修正项）
- [x] C2.18 Gemini 回填 `functionCall.args` 从字符串改为 JSON 对象，解析失败回退 `{}`（§4.3 必须修正项）
- [x] C2.19 Gemini `functionResponse` 必须带工具名：确认 `convert_tool_result_parts` 在 `name` 非空时产出 parts
- [x] C2.20 测试：三 adapter `ToolSpec` 注入形状快照（A6 后半）
- [x] C2.21 测试：带工具历史的第二轮请求三方言均合法（A19）
- [x] C2.22 测试：`custom` 注入、`default` / `quick` / `deep` 不注入（A9）

---

## C3 RoundExecutor 契约与端口（P4-C3，§2.3、§2.4、§4.5、§6.1）

实现：`assistant-agent/src/agent/round_executor.rs`、`agent/tool_event.rs`、`kernel/ports.rs`、`kernel/ports/test_doubles.rs`

- [x] C3.1 定义 `RoundExecutor` trait（`run_iterations(ctx) -> RoundOutcome`，`#[async_trait]`）
- [x] C3.2 定义 `RoundOutcome`，含 `confirmation_wait_ms`（D12）与 `hit_cap` / `wrap_up_injected` 分离
- [x] C3.3 定义 `TurnObserver { set_phase, emit_tool_event }`（现状实现；`delta` 入口见 C3.24）
- [x] C3.4 定义 `TurnRunCtx`：`turn_id` / `conversation_id` / `iteration` / `remaining_turn_budget` / `child_token` / `ToolCatalog` / `ToolExecutionPort` / `EventSink` / `MessageStore` / `TurnInputSource` / `Clock` / `TurnLimits` / `TurnSpec`（现状；`initial_message_id` / `turn_deadline` 补入见 C3.22，`EventSink` 移除见 C3.23）
- [x] C3.5 `TurnRunCtx` 新增委派预留字段 `agent_id` / `parent_turn_id` / `delegation_depth`，P4 恒为根 agent 值（§2.4）
- [x] C3.6 定义 `ToolExecutionPort` / `ToolExecCtx` / `ToolExecResult` / `ToolStage` / `ToolErrorClass`，全部声明在 `round_executor.rs`（§4.5）
- [x] C3.7 `ToolExecCtx` 不含权限端口、不含 `TurnObserver`（§4.5）
- [x] C3.8 `ToolExecResult` 字段与 `ToolOutcome` 的映射表逐项对齐（`content` 取 `assistant_text`，不取 `result_for_assistant`）
- [x] C3.9 `ToolStage` / `ToolErrorClass` 为本地镜像枚举，不 import `assistant-tools` 类型
- [x] C3.10 新建 `agent/tool_event.rs`：`ToolEvent`（上下文）+ `ToolEventPayload::{Started, ArgsDelta{delta}, Finished{result, confirmation_wait_ms}, Failed{error, error_kind, confirmation_wait_ms}}`（§6.1）
- [x] C3.11 `kernel/ports.rs` 的 `TurnEvent::ToolStarted` / `ToolFinished` 收敛为 `TurnEvent::Tool(ToolEvent)`
- [x] C3.11a 删除 `kernel/emit.rs` 的 `TurnEmitter::tool_started` / `tool_finished` 两个方法，改为单一 `tool_event(&self, event: ToolEvent)`（P3 已实现但无调用方，§6.1 现状核对）
- [x] C3.12 `event_sink.rs` 的 `ToolStarted` / `ToolFinished` 两个 `match` 分支（P3 期只打 debug 日志）合并为 `TurnEvent::Tool(_)` 分支，线事件映射在 C6.7 接入
- [x] C3.13 全仓 grep 确认 `ToolStarted` / `ToolFinished` 变体名已消失
- [x] C3.14 `test_doubles.rs` 的 `MessageStore` 替身补 `save_tool_calls` / `append_tool_message`，支持「写入 → 读回 → 重建 wire 形状」
- [x] C3.15 `test_doubles.rs` 新增 `ToolCatalog` / `ToolExecutionPort` 替身（可脚本化返回工具集与结果）
- [x] C3.16 `round_executor.rs` / `tool_event.rs` 不出现 `tauri` / `AppHandle` / `assistant_tools`（A8）
- [x] C3.17 测试：`ToolStage` / `ToolErrorClass` 与 `PipelineStage` / `ToolErrorKind` 一一对应（A18 前半）
- [x] C3.18 `TurnRunCtx` 补 `observer: &'a dyn TurnObserver`（C3.4 落地时漏项，§2.3）：executor 切 phase 与发 `ToolEvent` 的唯一入口；不得由实现层另持 `EventSink` 绕开（runner 侧 `RunnerObserver` 已接线）
- [x] C3.19 `kernel/ports.rs` 的 `StopHook` 由「只有 `name()`」补齐为 **I36 Step 5.1 的同步契约**：`check(&TurnState<'_>) -> StopDecision`（`Continue` / `Stop { reason }`）+ `TurnState<'a>`（`iteration` / `max_iterations` / `tool_calls` / `max_tool_calls` / `prompt_tokens` / `completion_tokens` / `elapsed` / `turn_deadline` / `model: &'a str`）；**不得改成 async**，钩子禁止 IO / await（§2.3、D18 细节）
- [x] C3.20 `RoundOutcome` 补齐 `reasoning` / `total_tokens` / `attempt_usages` / `ttft_ms` 四字段，与 `TurnOutcome` 同名同义（§2.3）；C5 切换后 `token_usage` 逐条落库与 TTFT 观测不得静默归零
- [x] C3.21 测试：`TurnState` 三维字段各自触发 `Stop`、以及全 `Continue` 的组合判定（放 `assistant-agent/tests/stop.rs`，与 C4.0a 的实现同批；11 条用例）
- [x] C3.22 `RoundOutcome` 再补三字段：`final_message_id`（`Option<String>`，本 turn 最后一条被写入并 `finalize` 的 assistant 行 id；仅「从未写入任何 assistant 行」时为 `None`）/ `context_window_tokens`（`Option<u32>`，最后一次请求投影值）/ `reserved_output_tokens`（`u32`，最后一次请求的 `max_tokens`）；`TurnOutcome` 同步带出 `final_message_id`（§2.3 / §5.1）
- [x] C3.23 `TurnRunCtx` 移除 `EventSink` 字段，`TurnRunner` 侧传参一并删除；全仓确认 delta 无第二条发射路径（§2.3）
- [x] C3.24 `TurnObserver` 补 `delta(&self, delta: &ProviderDelta) -> u32`（正文 / 推理增量的**唯一入口**，返回本次 `seq`）；`RunnerObserver` 实现委派 `TurnEmitter::delta`（seq 单调、代际校验、`iteration` 标记都在 `TurnEmitter`），executor 不得直接持有 `EventSink`（§2.3 / A23）
- [x] C3.25 `TurnRunCtx` 补 `initial_message_id: &'a str`、`turn_started_at: Instant` 两字段（§2.3 / §5.1），`TurnRunner` 侧传参同步；`remaining_turn_budget.turn_deadline: Option<Instant>` 保持既有定义（绝对截止时刻）
- [x] C3.26 `RoundOutcome` 的装配收敛为**单一构造器 `finish(status, text, reasoning, hit_cap, wrap_up_injected, error)`**，所有 return 点（含 `stopped` / `halted_by_budget` / `halted_with_summary` / 错误路径）只调用它，禁止各点各拼字段（§5「RoundOutcome 装配」）
- [x] C3.27 `TurnError` 补齐 P4 出口变体（§2.3）：`Timeout` / `EmptyResponse` / `NoProgress { reason }` / `Persist(String)` / `Provider(String)`，保留既有 `MiddlewareStop` / `Other`；**不参与 wire**（前端只看 `status` + `text`）

---

## C4 执行器实现（P4-C4）

实现：`assistant-agent/src/agent/{stop,middleware,ladder}.rs`（库侧纯逻辑）、`src-tauri/src/agent/round_executor.rs`（`ToolExecutionPort` 适配器、`TimingApprovalPrompter`）、`src-tauri/src/agent/**` 装配
测试：`assistant-agent/tests/{stop,middleware,ladder}.rs`、`src-tauri/tests/agent_loop.rs`

- [x] C4.0a 实现 `assistant-agent/src/agent/stop.rs`：`IterationLimitHook` / `ToolCallLimitHook` / `TurnWallClockHook` 三个 `StopHook` 恒安装（I36 spec §5.2 前三个），供 C4.1 组合；`TurnTokenBudgetHook` 仅在有配置时安装；只读 `TurnState`，不读时钟、不做 IO
- [x] C4.0b 实现 `assistant-agent/src/agent/middleware.rs`：`before_model` / `after_tool` / `after_iteration` 固定顺序链，**P4 只注册 1、3、4、5、6、7、8 七项**（② 摘要不随 P4，D13），顺序由常量数组决定，不做运行时排序、不留空位与 TODO 桩
- [x] C4.0c 实现 `assistant-agent/src/agent/ladder.rs`：阶梯 ③ 微压缩、④ 收尾还原、⑤ 硬裁剪；**② 摘要不随 P4**（D13），`ladder.rs` 不得留摘要执行分支或 TODO 桩
- [x] C4.0d 库侧三模块不出现 `tauri` / `AppHandle` / `assistant_tools`，只依赖 `kernel/ports`（A8 口径扩到 `agent/`）
- [x] C4.1 实现 `stop_hooks_all_continue`：`max_iterations` / `max_tool_calls` / turn 墙钟三维独立判定（D18；返回首个 `Stop` 的钩子名与原因，`None` 表示全部 `Continue`）
- [x] C4.2 两个 deadline 不混用（§2.2）：`TurnState.turn_deadline = limits.turn_wall_clock_ms.map(Duration::from_millis)`（总时长上限，供钩子 `elapsed >= limit` 判定）；裁剪用的剩余墙钟由 `ctx.remaining_turn_budget.turn_deadline`（`Option<Instant>`，绝对截止时刻）与 `clock.now()` 做差；`elapsed = clock.now().saturating_duration_since(ctx.turn_started_at)`，executor 不重算 turn 起点
- [x] C4.3 `before_model` 阶梯 + 中间件按 I36 Step 4.2 固定顺序执行（P4 为 1、3、4、5、6、7、8 七项，② 不注册）
- [x] C4.4 nudge 注入排在第 2.5 步（阶梯之后、模型调用之前），只进本次请求的 `Vec<ChatMessage>`（D19）
- [x] C4.5 模型调用走 `provider.chat_completion_stream` + `kernel::stream::consume`，`model_calls += 1`；每次 attempt 结束后 `token_stats.merge(&r)` 并 `attempt_usages.extend(r.attempt_usages)`（`StreamOutcome` 已按 attempt 逐条收集，重试不覆盖）
- [x] C4.6 空响应重试：`empty_response_retries = 1`，重发走**内层 `loop`**（不得用外层 `for` 的 `continue`，否则 `iteration` 递增，违反 D22 / A3），`model_calls` 递增，重发前 `accumulator.reset()`；「空」= `!has_effective_output && !accumulator.has_pending()`（**纯工具调用不触发重试**）；重试用尽仍空 → `Failed`（`iteration = 1` 复用占位行以空内容 `finalize(Failed)`；`iteration > 1` **不新增行**；`final_message_id` 保持上一次写入的值，仅「从未写过任何 assistant 行」时才是 `None`）
- [x] C4.6b 输出上限截断不重试（D22）：新增 `is_output_limit_reached(finish_reason)`（`length` / `max_tokens` / `MAX_TOKENS` / `max_output_tokens` → `true`），`finish_reason` 从 `StreamOutcome` 逐 attempt 取（`Ok(o) => finish_reason = o.finish_reason.clone()`）；命中时跳过内层重发，直接走 4.1 的 `Failed(EmptyResponse)`，`model_calls` 只加 1（A3）
- [x] C4.6a `agent/tool_call.rs` 的累积器补 `reset()`（只清空本次 attempt 的累积状态，不重置 `iteration`、不影响已发 `ArgsDelta`）；`has_pending()` 语义对齐「本 attempt 有工具调用片段」（D22 空响应判据）
- [x] C4.7 累积器 `finalize` 取本轮 `tool_calls`，无工具则 `Completed`
- [x] C4.8 收尾分支：`iteration == max_iterations` 时 `wrap_up_restore` + 撤回 tools + 收尾指令（D23）
- [x] C4.9 `split_by_budget`：按原始顺序取前 `remaining_tool_calls` 个，其余进 `skipped`；墙钟为 0 时整批并入（§5.3）
- [x] C4.10 `skipped` 结果形状：`is_error = true`、`stage = Gateway`、`error_kind = Denied`、content 写「未执行：…」
- [x] C4.11 `skipped` 不参与 `after_tool` 中间件、不进 D19 计数、不进 D25 分类预算，但计入 `tool_calls`
- [x] C4.12 实现 `ToolExecutionPort` 适配器：包 `Arc<assistant_tools::ToolPipeline>`，只做类型转换 + `confirmation_wait_ms` 注入 + 字段搬运
- [x] C4.13 适配器不得调用 `partition_tool_batches` / `select_persist_targets`（§4.5）
- [x] C4.14 实现 `TimingApprovalPrompter` 装饰器 + `WaitGuard`（`Drop` 记录 `Instant::elapsed`），按 `tool_call_id` 映射等待时长（§6.4）
- [x] C4.15 适配器在 `pipeline.execute` 返回后按 `tool_call_id` 取出并 `remove`，避免无界增长
- [x] C4.16 未走审批（`Allow` 直通）时 `confirmation_wait_ms = 0`
- [x] C4.17 `RoundOutcome.confirmation_wait_ms` 汇总本 turn 全部等待
- [x] C4.18 承接 P2 遗留生产端口（现状：`src-tauri` 只有 `ToolRegistry`，其余全部未装配，见 spec §6.4）
- [x] C4.18a 新建 `src-tauri/src/adapters/` 下的 `PermissionGrantStore` / `PermissionAuditStore` 实现（`Mutex<Connection>` + 同步 rusqlite，沿用 `DbMessageStore::with_conn` 模式）
- [x] C4.18b `permission_grants` / `permission_audit` 两表（`CREATE TABLE IF NOT EXISTS`，D14）随本批先建（C6.13 复用同一段建表代码，不重复）
- [x] C4.18c `ApprovalPrompter` 生产默认实现「Ask 即 Deny + 写审计」（`reason = "approval channel not available in P4"`，fail-closed；P6 替换内层，装饰器结构不变）；取舍理由见 spec §6.4（codex / OpenBitFun 同向，OpenHuman 的 fail-open 为反例）
- [x] C4.18d 装配 `ToolPipeline` / `ToolGateway` / `ToolResultStorage` / `PipelineEventSink` 并在 `setup.rs` 与 `TurnRunner` 之间接线
- [x] C4.18e `src-tauri` 全仓 grep 确认无遗留未绑定端口（`PermissionGrantStore` / `PermissionAuditStore` / `ApprovalPrompter` / `PipelineEventSink` / `ToolResultStorage` 均有生产实例）
- [x] C4.19 承接取消打标：turn 取消时整批结果标 `Cancelled`，content 写「已取消」（§4.4）
- [x] C4.20 `append_tool_results`：回填请求历史 + 写 `role='tool'` 行，两者文本逐字节一致（D21 / D11）
- [x] C4.20a 行级落库 `ensure_row()` 幂等：`iteration = 1` 复用 `ctx.initial_message_id`；`iteration > 1` 在模型返回后（成功 / 失败 / 取消）建一次，`parent_id = last_row_id`，每迭代重置 `row_created`（§5.1）
- [x] C4.20b 错误 / 取消 / 超时路径先 `ensure_row()` 再 `persist_row(..., 消息状态)`：取消 → `Interrupted`、超时 / provider 错误 → `Failed`（`finish` 的第 1 参是 `TurnStatus`，取消为 `Cancelled`，映射沿用 `message_status_of`）；**不得复用上一迭代 `msg_id`**（会覆盖已 `finalize` 的行、破坏行链），并置 `final_message_id = Some(msg_id)`（A22）
- [x] C4.20c 每迭代落库顺序固定为 `create → save_content → record_usage → save_tool_calls → finalize`；usage 由 executor 逐调用写（`message_id` = 本次行），`TurnPersister` 不再写 usage（§5.1）
- [x] C4.20d 请求历史**只在第 0 步投影一次**，后续迭代在内存 `Vec<ChatMessage>` 上追加，禁止每迭代重读 DB（§5.1）
- [x] C4.21 `after_tool` 阶梯顺序固定为 D26（豁免）→ D25（分类预算）→ D19（通用 2/3 阶梯）；`after_tool` 的 `StopTurn` **不立即返回**，先记 `halt_reason`、落完本迭代全部 tool 行后再 `halted_with_summary`（否则下一轮 wire 缺配对结果）
- [x] C4.21a `StopTurn` 路由：`before_model` → `stopped(reason, previous_content)`（`Completed`，文本兜底用上一迭代已落库正文）；`after_tool` / `after_iteration` → `halted_with_summary(reason)`（`Failed`）。不得混用（A21 直接断言两档 `status`）；`previous_content` 在每迭代 4.5 落库后更新
- [x] C4.21b nudge 传递：`pending_nudges` 由 `MiddlewareChain::take_pending_nudges()` 在每次迭代末取出并暂存，下一轮 2.5 注入；不进 `messages` 表（D19）
- [x] C4.21c `MiddlewareOutcome::StopTurn` 改为结构化 `{ middleware: &'static str, reason: String }`（`middleware.rs` 现状 `format!("{}: {}", mw.name(), reason)` 必须去掉）：`middleware` 只进 `log::warn!` 与遥测，`reason` 原样作为用户可见文本来源；`stopped` / `halted_with_summary` 只读 `reason`，`progress_summary_reason` 不再拼次数（§5.1 / §5.2）
- [x] C4.22 `halted_with_summary`：用根因摘要覆盖最终文本，`status = Failed`（仅「无进展」出口）；摘要经 `persist_final_text(summary, Failed)` 落成一条 assistant 行（`row_finalized == false` 时写本迭代行 / 第 1 迭代即占位行，`true` 时新建一条，`parent = last_row_id`；`final_message_id` 指向它，content 与 `text` 逐字节一致）（§5.2）
- [x] C4.22a `halted_by_budget()`：预算耗尽走 `spec.wrap_up = true` 时撤回 tools + 收尾指令再调一次模型（`call_model_without_tools` 计入 `model_calls` 与 token 统计、更新 `last_request`），结果经 `persist_final_text(text, Completed)` 落成一条独立 assistant 行；否则用根因摘要产出文本并同样 `persist_final_text(summary, Completed)`；两条路径都是 `Completed` + `hit_cap = true`、文本非空（A21）
- [x] C4.22b 收尾调用失败 / 超时 → `Failed`，`text` 退回根因摘要并 `persist_final_text(summary, Failed)` 落库（收尾调用已建行则沿用该行 `finalize(Failed)`，不重复建），不吞错误（§5.2）
- [x] C4.22c `persist_final_text` 写库失败（DB 错误）不得吞掉：`status` 按原终态返回、`error = Some(TurnError::Persist(...))`（§5.2）
- [x] C4.22d `wrap_up = false` 的最后一轮仍有工具调用 → 循环自然走完，`content` 为空时必须用根因摘要 `budget_summary_reason("iteration_limit")` + `persist_final_text(summary, Completed)` + `hit_cap = true`，不得返回空文本（§5 硬约束 / A21）
- [x] C4.23 所有长 await 包 `tokio::select! { biased; _ = token.cancelled() => ... }`
- [x] C4.24 测试：3 迭代闭环（A1）
- [x] C4.25 测试：`max_iterations` 收尾（A2）
- [x] C4.26 测试：空响应重试（含「`finish_reason = length` 且无正文/无工具片段时不重发、`model_calls` 只加 1」用例）（A3 / A15）
- [x] C4.27 测试：取消路径（A4）
- [x] C4.28 测试：工具失败回填后继续、`Denied` 不重试（A5）
- [x] C4.29 测试：三维预算各自单独触发（A12）
- [x] C4.30 测试：预算内裁剪与 wire 合法性（A12b）
- [x] C4.31 测试：重复失败 2/3 阶梯与 nudge 不落库（A13）
- [x] C4.32 测试：并行批次墙钟与顺序（A14）
- [x] C4.33 测试：失败分类阶梯（A17）
- [x] C4.34 测试：审批计时装饰器（sleep 20ms 替身，断言 `>= 20`）
- [x] C4.35 测试：行链与 `active_branch_id`（三迭代后 `parent_id` 依次相连、无挂在 `user` 下的兄弟分支、活跃叶为最后一条 assistant 行）（A20）
- [x] C4.36 测试：双 Halt 出口（预算耗尽 `completed` + `hit_cap` + 文本非空；无进展 `failed` + 根因摘要）；落库 `text` 与 `RoundOutcome.text` 不含中间件内部标识、摘要不重复拼次数（A21 / A21b）
- [x] C4.37 测试：第 2 迭代取消 / 超时 / provider 错误时新建行并保留第 1 迭代行不变（A22）
- [x] C4.38 测试：delta 只经 `TurnObserver::delta`，`seq` 严格单调、`iteration` 标记正确、旧代际丢弃（A23）
- [x] C4.39 适配器每次 `execute` 现构造 `ToolUseContext`（`workspace_root` 来自会话绑定、`session_id = conversation_id`、`tool_call_id` 逐调用填）与 `ResolvedPermissionPolicy::default_global()`；`ToolResultStorage` 的 `root` 用稳定的 `app_data_dir()/tool-results/`（不得用临时目录）（§4.5）
- [x] C4.40 落库辅助函数按 §5.1 的表实现：`ensure_row` / `persist_row` / `persist_row_as_new` / `persist_final_text` / `append_tool_results` / `not_executed` / `split_by_budget` / `stopped` / `halted_by_budget` / `halted_with_summary` / `budget_summary_reason` / `progress_summary_reason`；**不得出现第三条写库路径**（assistant 行只经 `persist_row` / `persist_row_as_new`，tool 行只经 `append_tool_results`；`persist_final_text` 只在两者之上择行）
- [x] C4.40a 每迭代重置 `row_created` / `row_finalized` / `retries_used` 并 `accumulator.reset()`；`row_finalized` 只用于 `persist_final_text` 择行，不参与 wire、不落库（§5.1）
- [x] C4.40b 错误 / 取消 / 超时路径的 `persist_row` 传 `tool_calls = []`（部分累积的调用不完整，落库会让 wire 出现无配对结果的 `tool_call`）（§5.1 / A22）
- [x] C4.40c usage 只写一次：`persist_row_as_new` 仅当该 attempt 的 usage 尚未落库时才 `record_usage`；`stopped` / `halted_with_summary` 无新 attempt 时不写 usage，避免同一 attempt 写两条 `token_usage`（§5.2）

---

## C5 TurnRunner 切换（P4-C5）

实现：`assistant-agent/src/kernel/runner.rs`

- [x] C5.1 `TurnRunner` 切到 `executor.run_iterations(ctx)`（`round_executor: Arc<dyn RoundExecutor>` 字段非 `Option`，`new()` 直接装 `LoopRoundExecutor`，无注入方法）
- [x] C5.2 删除 P3 的过渡单迭代循环，全仓无第二套循环体；**本项与 C1.13a–C1.13c 同批交付**（C1 删 `StreamOutcome.tool_calls` 后过渡循环即不可编译）
- [x] C5.3 一次性替换，**不保留**运行时双路径开关
- [x] C5.4 测试：chat 路径事件序列与 P3 对比一致（A9）
- [x] C5.5 测试：chat 路径无 tool 消息、无 `tools` 注入（A9）

---

## C6 事件 DTO 与 tool 落库（P4-C6，D5 / D11 / D14、§6.1.1 / §6.2 / §6.3）

实现：`src-tauri/src/protocol/dto/agent.rs`、`protocol/events.rs`、`adapters/event_sink.rs`、`assistant-storage/src/**`、`assistant-agent/src/prompt_builder.rs`、`src/adapters/message_store.rs`
测试：`src-tauri/tests/protocol_dto_roundtrip.rs`、`src-tauri/tests/protocol_bindings.rs`、`assistant-storage` 集成测试

- [x] C6.1 新建 `src-tauri/src/protocol/dto/agent.rs`：`AgentToolCallDto`（`deny_unknown_fields`）+ `AgentToolCallPayloadDto`（`#[serde(tag = "phase", rename_all = "camelCase")]`，四变体）
- [x] C6.2 变体内多词字段用显式 `#[serde(rename = "confirmationWaitMs")]` / `"errorKind"`，不用 `rename_all_fields`
- [x] C6.3 `Failed` 变体复用 `ToolErrorKindDto`，不新建第三个错误类别枚举
- [x] C6.4 `Option` 字段不加 `skip_serializing_if`（键恒在、值 `null`）
- [x] C6.5 `protocol/dto/mod.rs` 加 `pub mod agent;` 与 `pub use agent::*;`
- [x] C6.6 `protocol/events.rs` 加 `AGENT_TOOL_CALL = "agent-tool-call"` 常量
- [x] C6.7 `event_sink.rs` 实现 `TurnEvent::Tool(e)` → `AgentToolCallDto` 映射并 `emit(AGENT_TOOL_CALL, ...)`，只做字段搬运
- [x] C6.8 `protocol_bindings.rs` 的 `use` 与 `declarations` 加两类型（计数断言已清理，无需改数字）；跑 `cargo test --test protocol_bindings`，集合断言须通过
- [x] C6.9 `protocol_dto_roundtrip.rs` 加 `agent_tool_call_dto_roundtrip`，四变体各一条，`confirmation_wait_ms` 取 `Some` / `None`
- [x] C6.10 `npm run contract:gen` 生成 `protocol.contract.ts`，确认四变体为 discriminated union（A10）
- [x] C6.11 `assistant-storage/src/db.rs`：`CREATE TABLE messages` 加 `tool_call_id TEXT` 与 `is_error INTEGER NOT NULL DEFAULT 0`
- [x] C6.12 `db.rs` 既有库分支追加两段 `pragma_table_info` 守卫 + `ALTER TABLE messages ADD COLUMN ...`，放在 `model_id` 守卫之后
- [x] C6.13 `db.rs` 建表段新增 `permission_grants` / `permission_audit` 两表（`CREATE TABLE IF NOT EXISTS`，幂等，不做 ALTER 迁移，D14）；若 C4.18b 已提前落地，此处只核对不重复建
- [x] C6.14 `conversations/mod.rs`：`MSG_SELECT` 追加两列（13 → 15）；`MessageRow` 加 `tool_call_id: Option<String>` / `is_error: bool`
- [x] C6.15 `read_msg_row` 用具名列同步（`row.get("tool_call_id")` / `row.get::<_, i64>("is_error")? != 0`）
- [x] C6.16 补齐 `MessageRow` 结构体字面量的两字段：`src/cmd/compaction.rs`、`src/cmd/chat/insert.rs`（两处）、`src/adapters/message_store.rs`；不得改成 `..Default::default()`
- [x] C6.17 `msg_ops.rs` / `branch.rs` 新增带 `tool_call_id` / `is_error` 的插入路径（供 `append_tool_message`），既有 INSERT 不改（新列靠 `DEFAULT` / `NULL`）
- [x] C6.18 `DbMessageStore` 用 `with_conn` 模式实现 `save_tool_calls` / `append_tool_message`，不引入 `spawn_blocking` 或连接池
- [x] C6.19 `append_tool_message` 的 `parent_id` 指向 assistant 行、`sibling_index = 0`；不新增「按 role 分桶的 sibling_index」
- [x] C6.20 `prompt_builder::prepare_messages` 从 `tool_calls_json` / `tool_call_id` / `is_error` 重建 `ChatMessage`（替换现有硬编码 `None` / `false`）
- [x] C6.20a 投影期配对修复（D27）：重建后做一次只读配平 —— 正向孤儿（`tool_calls_json` 的 `call_id` 无对应 tool 行）按 `tool_calls` 顺序合成 `is_error = true`、`content = "未执行：本轮中断"` 的 tool 消息；反向孤儿（tool 行的 `tool_call_id` 在同分支前序 assistant 行里找不到）投影丢弃；空 `content` 的 tool 行投影为 `"(空结果)"`；配对只在同一 `parent_id` 分支内匹配；**不写库**、纯函数幂等、命中即 `log::warn!`
- [x] C6.21 `prompt_builder` 建 `call_id → tool_name` 映射，给 `role='tool'` 行填 `name`（Gemini 必需）
- [x] C6.22 纯工具调用的 assistant 行 `content` 写空串（`NOT NULL` 约束），不写 `NULL`
- [x] C6.23 迁移只 `ADD COLUMN`，无 `DROP` / `RENAME` / 存量行重写
- [x] C6.24 测试：tool 落库往返（写入 → 读回 → 重建 wire 形状，顺序一致）（A11）
- [x] C6.25 测试：旧库迁移后 `tool_call_id` 为 `NULL`、`is_error` 为 `0`
- [x] C6.26 测试：`append_tool_message` 不参与分支兄弟计数
- [x] C6.27 测试：投影期配对修复（A11b）—— 构造「assistant 有 `tool_calls_json` 但缺 tool 行」的历史，断言 `prepare_messages` 产出配平 wire（合成 `is_error = true` 的「未执行：本轮中断」）且 `messages` 表无新增行；构造反向孤儿断言投影丢弃；同一输入重复调用结果一致（幂等）

---

## C7 端到端测试（P4-C7）

测试：`src-tauri/tests/agent_loop_e2e.rs`、`src-tauri/tests/agent_loop_dialect.rs`

> **本批整体跳过（2026-11-06，用户决策）**：两个端到端测试文件不编写，C7.1–C7.17 全部不计入完成项，复选框保持未勾选以保留原始验收口径。跳过原因：本批需要跨 `assistant-app` 装配真实 `ToolPipeline` + provider adapter 才能成立，成本高于收益；组件级等价覆盖已存在（见下），端到端链路本身无自动化覆盖，作为已知缺口登记，不视为已验收。
>
> 现有等价覆盖（不代表 C7 已验收）：
> - 循环 / 三维预算 / 阶梯 ③④⑤ / 行链 / 双 Halt：`crates/assistant-agent/tests/loop_executor.rs`（26 条，A1–A23 大部分口径）
> - turn 生命周期 / 事件序列 / 落库序列 / 取消 / 超时 / 空响应重试：`crates/assistant-agent/tests/kernel_runner.rs`（13 条）
> - 端口镜像不漂移：`src-tauri/tests/agent_tool_mirror.rs`
> - 回填方言（Anthropic `input` / Gemini `args` / `functionResponse` / 非法参数回退）：`assistant-providers` 各 `message_converter` 单测
>
> 需要恢复本批时，把下列 `- [ ]` 条目改回待办即可，无需改规格。

- [ ] C7.1 3 迭代闭环 + tool 落库往返联合用例（A1 / A11）（跳过）
- [ ] C7.2 收尾调用不含 `tools`、`hit_cap` / `wrap_up_injected` 断言（A2）（跳过）
- [ ] C7.3 空响应重试后仍空 → `Failed`，`messages` 无空白行（A3 / A15）（跳过）
- [ ] C7.4 取消：部分内容落库、`status = interrupted`、结果标 `Cancelled`、状态回 `Idle`（A4）（跳过）
- [ ] C7.5 工具失败回填后继续；`Denied` 不重试（A5）（跳过）
- [ ] C7.6 三维预算与预算内裁剪（A12 / A12b）（跳过）
- [ ] C7.7 nudge / halt 与 `messages` 无新增行（A13）（跳过）
- [ ] C7.8 并行批次：墙钟小于串行和、结果顺序与 `tool_calls` 一致（A14）（跳过）
- [ ] C7.9 收尾还原：微压缩清空结果后结论调用可见最近结果，后续轮不保留（A16）；**摘要计数断言不在本批**，随续跑式压缩批次验收（spec §8 的 A16 归属说明）（跳过）
- [ ] C7.10 失败分类阶梯与成功清零（A17）（跳过）
- [ ] C7.11 端口镜像不漂移 + `assistant-agent` 源码无 `assistant_tools` 引用（A18）（跳过；镜像契约已由 `src-tauri/tests/agent_tool_mirror.rs` 覆盖，源码无引用由 C8.10 覆盖）
- [ ] C7.12 回填方言：Anthropic `input` 对象、Gemini `args` 对象、`functionResponse` 带工具名、非法 arguments 回退 `{}` 不 400（A19）（跳过；已由 `message_converter` 单测覆盖）
- [ ] C7.13 累积器唯一实现（A7）（跳过）
- [ ] C7.14 行链与 `active_branch_id` 端到端：三迭代后行序、`parent_id` 链、活跃叶、无假分支（A20）（跳过；行链已由 `loop_executor.rs` 单测覆盖）
- [ ] C7.15 双 Halt 出口端到端：预算耗尽 `completed` + `hit_cap` + 文本非空；无进展 `failed` + 根因摘要（A21）（跳过；已由 `loop_executor.rs` / `kernel_runner.rs` 覆盖）
- [ ] C7.16 错误路径建行端到端：第 2 迭代取消 / 超时 / provider 错误新增行且第 1 迭代行不变（A22）（跳过）
- [ ] C7.17 delta `seq` 单调与旧代际丢弃端到端（A23）（跳过）

---

## C8 验收命令

`cwd = src-tauri`

- [x] C8.1 `cargo test -p assistant-agent` 全绿（2026-11-06：默认 feature 与 `--all-features` 均全绿，含 `kernel_runner` 13 项）
- [x] C8.2 `cargo test -p assistant-providers` 全绿
- [x] C8.3 `cargo test -p assistant-storage` 全绿
- [x] C8.4 `cargo test -p assistant-app --test protocol_dto_roundtrip` 通过（A10）
- [x] C8.5 `cargo test --test assembly_contract` 通过（A10）
- [ ] C8.6 `cargo test -p assistant-app --test agent_loop_e2e` 与 `--test agent_loop_dialect` 通过（**跳过**：C7 整体跳过，两个测试文件不存在，本项不成立；原勾选为误记，已纠正为未完成）
- [x] C8.7 `cargo clippy -p assistant-agent -p assistant-providers -p assistant-storage --all-targets -- -D warnings` 无告警（2026-11-06：exit 0，无 clippy 告警；仅有 cargo 的增量编译目录清理 warning，与 lint 无关）
- [x] C8.8 先执行格式化修复，再 `cargo fmt --all --check` 通过（2026-11-06：`cargo fmt --all --check` exit 0，无需修复）
- [x] C8.9 `cargo tree -p assistant-agent` 中无 `tauri`、无 `assistant-tools`（A8）（2026-11-06：481 行依赖树，`tauri v` / `assistant-tools v` 均无匹配；路径中的 `src-tauri` 已排除）
- [x] C8.10 grep 确认 `agent/round_executor.rs` / `agent/tool_event.rs` / `agent/tool_call.rs` / `agent/stop.rs` / `agent/middleware.rs` / `agent/ladder.rs` 无 `AppHandle` / `Window` / `assistant_tools`（2026-11-06：`crates/assistant-agent/src/agent` 全目录 `AppHandle|tauri::|assistant_tools` 零匹配）
- [x] C8.11 `npm run contract:check` 幂等（无 diff）
- [x] C8.12 `cargo test --workspace --no-run` 确认全量编译通过
- [x] C8.13 确认未改 `assistant-tools` 任何文件（`git status` 路径核对）（2026-11-06：全量 `git status --short` 中无 `crates/assistant-tools` 条目）

---

## C9 文档

- [x] C9.1 更新 [roadmap](./I38-agent-roadmap.md) §2：P4 标记已完成并记录日期（阶段状态唯一台账）（2026-11-06：P4 置 ✅ 已完成，并注明 C7 端到端批次跳过）
- [x] C9.2 在本清单勾选 C1–C8 完成项（任务级进度只在本清单维护）（2026-11-06：C1–C6 全勾；C7.1–C7.17 跳过不计完成；C8 除 C8.6（跳过）外全勾）
- [x] C9.3 确认 I38b 规格与 I36 spec 无进度表述（spec 只写「怎么做」）（2026-11-06：核对时发现 §7 实施步骤表仍带 `状态` 列且含 `进行中` / `待办` 陈旧值，已删除该列并改为指向 tasklist / roadmap 的指针，使规格只留步骤与内容）

---

## 实施偏差记录

> 记录代码与规格字面不同的落地差异。**积极优化** = 比规格最低要求做得更好；**真实偏差** = 代码与规格不一致（已处置或待处置）。新增偏差必须先回写规格再登记，不在代码里临时拍板。

### 积极优化（强于规格最低要求）

| 编号 | 原条目 | 差异 | 处置 |
|---|---|---|---|
| O1 | C4.23 | 第 0 步投影读库同样受取消约束 | 已包 `cancellable`；取消时返回 `Cancelled`（`projection_exit`），不落任何行 |
| O2 | C4.40c | 规格表述为「该 attempt 的 usage 尚未落库才写」 | 实现用 `RoundState.persisted_usage_count` 游标只写增量（比逐 attempt 打标记更简单，语义等价；收尾调用新增的 attempt 照常补写） |

### 真实偏差（代码与规格字面不同）

| 编号 | 条目 | 差异 | 处置 |
|---|---|---|---|
| V1 | C3.26 | 带 `RoundState` 的全部出口都走 `RoundState::finish`；唯一例外是第 0 步投影阶段（`RoundState` 尚未建立，无法调用 `finish`） | 收敛为单个 helper `projection_exit(status, error, provider_error)`，不做第二个字段拼装点；只填 `status` / `error` / `provider_error`，其余计数恒 0 |
| V2 | C4.11 / C4.19 | 取消路径原先只累加 `to_run.len()`，漏计 `skipped` | 改为 `n = to_run.len() + skipped.len()`，与正常路径 `calls.len()` 口径一致 |
| V3 | C4.16 | `Allow` 直通时端口返回 `confirmation_wait_ms = None`（而非 `Some(0)`），`RoundOutcome` 汇总侧按 0 累加 | 语义等价（直通不计等待）；字段保持 `Option<u64>`，`None` 表示「未发生等待」 |
| V4 | C4.19 | 规格只规定 `error_kind = Cancelled` + content「已取消」，未规定 `stage` | 沿用 `ToolStage::Gateway`（与 `skipped` 的 `Denied` 同档，表示未进入 Execute 阶段） |
| V5 | C4.22d | 清单写 `budget_summary_reason("iteration_limit")`（下划线），实现用 `"iteration-limit"`（连字符） | 以 `stop.rs` 钩子 `name()` 为准（连字符）；清单文本为笔误，代码内部自洽 |
| V6 | C5.4 | 「与 P3 对比」已无 P3 循环可对照 | 以 P3 期写入、P4 切换后**未改动仍全绿**的 `kernel_runner` 断言为基线（事件序列 / `StoreCall` 序列 / 空响应重试 / 取消 / 超时） |
| V7 | §2.1 | 规格记载 RoundExecutor 实现在 `src-tauri/src/agent/round_executor.rs`；实现层无 `tauri` / `AppHandle` / `assistant-tools` 依赖，实际位于 `crates/assistant-agent/src/agent/loop_executor.rs` | **已处置（2026-11-06）**：spec §2.1 / §2.2 / 影响范围三处 `src-tauri/src/agent/` 路径已同步为实际代码位置。理由：`LoopRoundExecutor` 通过 `ToolExecutionPort` trait 抽象后无需 `tauri` / `assistant-tools` 依赖，自然归属 `assistant-agent` crate；适配器与装配在 `src-tauri/src/adapters/` + `setup.rs`。这是架构优化结果，非回归。 |
| V8 | §2.3 `RoundOutcome` | 规格原字段表无 `provider_error`，而 P4 迁移丢失了 `TurnEvent::Terminated.error` 的原始载荷 | **已处置（2026-11-06）**：spec §2.3 补 `provider_error: Option<ProviderErrorPayload>`（标注「不进 DTO、不参与 wire」）。执行器在全部失败出口填值，`TurnRunner::drive` 用 `if terminal_error.is_none()` 回填（取数前置路径优先级更高）。理由见下方缺陷 5。 |

**核对时修复的实际缺陷（非偏差，属回归）**：

1. 行链回归：`append_tool_results` 末尾曾把 `last_row_id` 覆盖回 assistant 行，使下一迭代新行与 tool 行成为兄弟分支（假分支）。已删除该覆盖，`last_row_id` 严格等于最后一条 tool 行 id。
2. 取消路径行 id 误用：`ensure_row` / `persist_row_as_new` / `append_tool_results` 曾用 `cancellable(...).unwrap_or(Ok(旧 id))`，取消时把父行 id 当成本次要写的行 id。现三处统一返回 `Option<T>`，取消即跳过写入，不伪造 id。
3. 过时断言：`kernel_values.rs` 的 `agent_spec_field_values` 曾断言 `spec.stop_hooks.is_empty()`，与 P4 恒装三个钩子冲突；改为断言钩子名列表。
4. C4.19 取消批次形状与 C4.40c usage 单次落库：核对时补实现（前者原为 `Denied` + 「未执行：本轮已取消」，后者原为每次 `persist_row` 全量重写 `attempt_usages`）。
5. 终态错误载荷丢失（P3 → P4 迁移回归）：P3 的 `TurnRunner` 在墙钟超时 / 流超时 / provider 失败 / 空响应 / 取消五个分支均写 `terminal_error`，迁入执行器后只剩取数前置路径赋值，`run_iterations` 返回后未再写。后果：`event_sink` 的 `Terminated.error` 恒为 `None`，前端 `SessionState::Error` 降级为字面量 `"turn failed"`，`code` / `retryable` / `retryAfterMs` 全丢。已修：执行器新增 `RoundOutcome.provider_error`，在 `projection_exit`、`stream()` 建流失败、`stream_failure`（Timeout / Provider / Cancelled）、空响应、收尾调用三失败分支逐一填值；`TurnRunner::drive` 回填 `terminal_error`。`kernel_runner` 的 `provider_error_marks_turn_failed_with_error_text` 已加强为断言 `code == AuthFailed`，锁住不退化。

**落地状态（2026-11-06）**：P4 已完成。C7（端到端）与 C8.6 按用户决策整体跳过，不计完成（见 C7 段说明）；其余 C1–C6、C8.1–C8.5 / C8.7–C8.13、C9 全部完成。

**P5 将修改的交集**（2026-11-06 记录，非 P4 缺陷）：

P5 将在白名单工具解析落地时对 P4 的以下交付做类型收窄与清理（规格见 [I38c §3.1](./I38c-os-tools-spec.md)）：
- `ToolCatalog` async trait 删除（`kernel/ports.rs`）
- `RegistryToolCatalog` 删除（`src-tauri/src/adapters/tool_catalog.rs`，含 `MAX_TIER`）
- `TurnSpec.tools` 类型由 `Option<Arc<dyn ToolCatalog>>` 收窄为 `Option<Vec<ToolSpec>>`
- 连带同步点（实测清单，缺一处即编译失败）：`kernel/mod.rs`（字段 + `chat()` / `agent()` 默认值）、`kernel/runner.rs`（`TurnRunCtx.tools`）、`kernel/request.rs`（`build()` 签名）、`agent/round_executor.rs`（`TurnRunCtx.tools` / `ToolExecCtx.tools` 字段）、`agent/loop_executor.rs`（`request::build` 传参 + `ToolExecCtx` 构造）、`tests/loop_executor.rs`、`tests/kernel_request_tools.rs`
- `test_doubles.rs` 的 `StaticToolCatalog` 替身删除
- 以上变更不影响 P4 的循环机制（RoundExecutor / 停止钩子 / 中间件 / 阶梯 / 落库），仅影响工具注入通道的签名

**P4 遗留缺陷（P5 顺手清理）**：`ToolExecCtx.tools` 是死字段——生产适配器 `PipelineToolExecution`（`src-tauri/src/adapters/tool_exec.rs`）只读 `observer` / `turn_id` / `iteration` / `conversation_id`，从不读 `tools`；P4 为对称性保留了该字段但无消费者。P5 收窄 `TurnSpec.tools` 时一并删除，避免留下 `Option<&[ToolSpec]>` 的新死字段（I38c tasklist C6b.21）。

C4.24–C4.38 循环单测已落地于 `assistant-agent/tests/loop_executor.rs`（26 条，A1–A23 全覆盖，`cargo test -p assistant-agent --features test-doubles --test loop_executor` 全绿）；C4.34 审批计时装饰器单测位于 `src-tauri/tests/approval_timing.rs`（3 条，`cargo test --test approval_timing` 全绿）。

---

## 验收映射

| 规格验收 | 对应任务 |
|---|---|
| A1 三迭代闭环 | C4.24、C7.1 |
| A2 收尾调用 | C4.8、C4.25、C7.2 |
| A3 空响应重试 | C4.6、C4.6b、C4.26、C7.3 |
| A4 取消 | C4.19、C4.27、C7.4 |
| A5 工具失败 / `Denied` | C4.21、C4.28、C7.5 |
| A6 三 adapter 注入形状 + Gemini 快照语义 | C1.21、C2.20 |
| A7 累积器唯一实现 | C1.13、C1.14、C7.13 |
| A8 无 tauri / 无 assistant-tools 依赖 | C3.16、C8.9、C8.10 |
| A9 chat 路径行为不变 | C2.8、C2.9、C2.22、C5.4、C5.5 |
| A10 契约生成与声明集合一致 | C6.8、C6.9、C6.10、C8.4、C8.5 |
| A11 tool 落库往返 | C6.14 – C6.21、C6.24、C7.1 |
| A11b 投影期配对修复 | C6.20a、C6.27、C7.1 |
| A12 三维预算 | C4.1、C4.29、C7.6 |
| A12b 预算内裁剪 | C4.9 – C4.11、C4.30、C7.6 |
| A13 重复失败阶梯 | C4.4、C4.21、C4.31、C7.7 |
| A14 工具执行策略 | C4.12、C4.32、C7.8 |
| A15 空响应不落库 | C4.6、C4.26、C7.3 |
| A16 收尾还原（压缩计数断言移出本批，随续跑式压缩批次） | C4.8、C7.9 |
| A17 失败分类阶梯 | C4.21、C4.33、C7.10 |
| A18 端口镜像 | C3.9、C3.17、C7.11 |
| A19 回填方言 | C2.17 – C2.19、C2.21、C7.12 |
| A20 行链与 `active_branch_id` | C4.20a、C4.35、C7.14 |
| A21 双 Halt 出口 | C4.22、C4.22a、C4.36、C7.15 |
| A21b Halt 文本可读（无内部标识） | C4.21c、C4.22、C4.36 |
| A22 错误路径建行 | C4.20b、C4.37、C7.16 |
| A23 delta 唯一路径与 seq 单调 | C3.3、C3.23、C4.38、C7.17 |
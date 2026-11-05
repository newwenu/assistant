# I38b — P4 Agent Loop 闭环任务清单

> 规格：[I38b-agent-loop-spec.md](./I38b-agent-loop-spec.md) §7 实施步骤、§8 验收标准、§9.1 决策定稿
> 阶段：[I38-agent-roadmap.md](./I38-agent-roadmap.md) P4
> 顺序：C0 → C1 → C2 → C3 → C4 → C5 → C6 → C7 → C8 → C9
> 勾选条件：代码已改 **且** 对应验收已跑。
> **状态唯一位置**：本清单是 P4 任务级进度的唯一来源；阶段级结论只在 [roadmap](./I38-agent-roadmap.md) §2 写一次，spec / design / 基线 / 验证文档都不记录进度。本清单**不管理提交**（不写提交拆分 / 提交信息 / 回滚批次，见 roadmap §7 第 2 条）。
> 范围：只交付 RoundExecutor 多迭代循环、tool call 累积器、`ChatOptions.tools` 注入、P2 遗留生产端口绑定、事件 DTO 与 tool 落库；不新增 OS 工具实现、不做前端审批 UI、不做 MCP、不做压缩续跑式切换、不做子代理。
> 测试位置：`assistant-agent` 单测一律放 `crates/assistant-agent/tests/<模块>.rs` 集成测试，不写 `#[cfg(test)]` 内联模块；`src-tauri` 侧端到端测试放 `src-tauri/tests/`。
> 决策基线：D1-D26 已全部定稿（§3.2 / §9.1），开工后不再新增决策；遇到未覆盖问题先回写规格，不在代码里临时拍板。

> **前置清理（已完成，2026-11-05）**：`src-tauri/tests/protocol_bindings.rs` 原 `assert_eq!(output.matches("export type ").count(), 63, ...)` 计数断言已删除，替换为「`src/protocol/dto` 中 `#[derive(...TS)]` 类型集合 ↔ `declarations` 类型集合」双向差集断言（另加一条 `declarations` 重复注册检查）。原因：计数断言在「新增 DTO 但漏加 `declarations`」时仍然通过（输出数不变），只在「加了条目却忘改数字」时失败，属于魔数提醒器而非校验器；同仓 `assistant-vault` 的 `export_bindings` 无对应计数，说明原写法是孤例。清理后 P4 新增两个 DTO 只需往 `declarations` 加两条，**不再需要改任何数字**。规格侧痕迹见 I38b §6.1.1 / A10 / §9 风险表，历史侧痕迹见 I38a 任务清单 C3.11 注记。

---

## C0 口径与前置确认

- [ ] C0.1 确认 P3 已通过验收：`kernel/` 端口、状态机、取消代际、双层超时、chat 等价迁移可用（P4 只替换迭代循环体）
- [ ] C0.2 确认 P2 已通过验收：`ToolPipeline`（`find → validate → gateway → execute → persist`）、`partition_tool_batches`、`select_persist_targets` 均可用
- [ ] C0.3 确认 P4 **不改** `assistant-tools` 任何代码：分批、落盘、结果事件全在 Pipeline 内部完成，executor 不重复实现（§4.5 职责切分）
- [ ] C0.4 确认 P4 不实现 OS 工具（P5）、前端五态与审批 UI（P6）、MCP（P7）、压缩续跑式（D13）、子代理（P8）
- [ ] C0.5 确认 D1-D26 与 I36 D10 的边界（「模型可见历史落库、运行时簿记不落库」）已理解一致（§6.2）
- [ ] C0.6 确认协议改动一律走 `npm run contract:gen`，禁止手改 `src/types/protocol.contract.ts`
- [ ] C0.7 确认 `MessageDto` 本阶段不动（工具卡片字段归 P6）

---

## C1 累积器迁移与 provider 快照语义（P4-C1，D1-D9）

实现：`src-tauri/crates/assistant-agent/src/agent/tool_call.rs`、`src-tauri/crates/assistant-providers/src/**`
测试：`src-tauri/crates/assistant-agent/tests/tool_call.rs`

- [ ] C1.1 新建 `assistant-agent/src/agent/tool_call.rs`，`agent/mod.rs` 增加 `pub mod tool_call;`
- [ ] C1.2 定义 `ToolCallStreamKey { Indexed(usize), Unindexed }`（D1），Anthropic 有 index 用 `Indexed`、否则 `Unindexed`
- [ ] C1.3 定义 `ToolCallBoundary { NewTool, FinishReason, StreamEnd }`（D2），保留 `finalized_previous` 语义
- [ ] C1.4 实现 `apply_delta`（增量入口）与 `apply_unified_response`（`UnifiedResponse` 入口，D4）
- [ ] C1.5 实现跨 chunk 累积：`Indexed` 按 index 分组、`Unindexed` 单槽累积
- [ ] C1.6 实现 `arguments_is_snapshot` 分支：`true` 用快照替换、`false` 追加（D8）
- [ ] C1.7 截断处理按 D3：先 `repair_truncated_json`，修复可用则降级（`is_error = false` + 修复提示），失败才报 `Truncated`
- [ ] C1.8 非法 JSON 不 panic，产出可回灌的失败结果
- [ ] C1.9 `ChatDelta` 新增 `delta_tool_call_arguments_is_snapshot: bool`（D9，provider 内部流类型，非 wire）
- [ ] C1.10 OpenAI adapter 填 `arguments_is_snapshot = false`
- [ ] C1.11 Anthropic adapter 填 `false`，有 index 时带 `Indexed` 键
- [ ] C1.12 Gemini adapter 只透传快照片段并标 `true`，累积统一交给 `agent/tool_call.rs`（D8，修 F15/F16）
- [ ] C1.13 删除 `assistant-providers/src/tool_call_accumulator.rs` 及其 `lib.rs` 导出（A7）
- [ ] C1.13a `kernel/stream.rs` 的 `StreamAccumulator` 删除 `tool_calls` 字段与 `apply_tool_call`，只保留正文 / 推理 / usage / `ttft_ms` / `finish_reason` / `has_effective_output`（§3.1）
- [ ] C1.13b `StreamOutcome` / `PartialOutcome` 删除 `tool_calls` 字段；工具片段改由 `consume` 的 `on_delta` 回调原样交给调用方累积
- [ ] C1.13c 删除后 `kernel/runner.rs` 的过渡循环暂不可编译 → **C1 与 C5 必须同批交付**（见 C5.2）
- [ ] C1.14 全仓 grep 确认无第二份累积器实现：`assistant-providers` 无 `ToolCallAccumulator`，`assistant-agent/src/kernel/stream.rs` 无 `ToolCallAccum`（A7 口径见 spec §3.1）
- [ ] C1.15 `tool_call.rs` 不出现 `tauri` / `AppHandle` / `assistant_tools` 引用
- [ ] C1.16 测试：跨 chunk 累积（`Unindexed` 与 `Indexed` 双键）
- [ ] C1.17 测试：Gemini 快照替换语义
- [ ] C1.18 测试：三种 `ToolCallBoundary` 的终结行为
- [ ] C1.19 测试：截断 JSON 修复成功 / 失败两条路径
- [ ] C1.20 测试：非法 JSON 产出失败结果不 panic
- [ ] C1.21 测试：三 adapter 对同一逻辑调用产出等价的 `FinalizedToolCall`（A6 前半）

---

## C2 tools 通道与三 adapter 注入（P4-C2，D10、§4.1-§4.3）

实现：`assistant-providers/src/types/tool.rs`、`types/config.rs`、`openai/**`、`anthropic/**`、`gemini/**`、`assistant-agent/src/kernel/ports.rs`、`src-tauri/src/adapters/**`
测试：`src-tauri/crates/assistant-providers/tests/`、`src-tauri/src/agent/` 装配测试

- [ ] C2.1 定义 `ToolSpec { name, description, input_schema }`，`serde(rename_all = "camelCase")`（§4.1）
- [ ] C2.2 `ChatOptions` 新增 `tools: Option<Vec<ToolSpec>>`，`#[serde(default, skip_serializing_if = "Option::is_none")]`（§4.2）
- [ ] C2.3 `build_chat_options` 新增 `tools` 参数并填入返回结构（不新增第二个构造函数）
- [ ] C2.4 同步两处 `ChatOptions` 完整字面量构造：`chat_options.rs`、`cmd/compaction.rs`（压缩摘要显式 `tools: None`，不得用 `..Default::default()`）
- [ ] C2.5 `kernel/ports.rs` 声明 `ToolCatalog` async trait（`agent_tools() -> Vec<ToolSpec>`，D16）；**同时删除 `kernel/mod.rs` 的同步占位 `ToolCatalog` / `EmptyToolCatalog` 及其导出**，全仓只留一套
- [ ] C2.5a `TurnSpec.tools: Option<Arc<dyn ToolCatalog>>` 与 `runner.rs` 的 `spec.tools.is_none()` 判定改到新签名（P3 过渡循环随 C5 删除）
- [ ] C2.6 `src-tauri` 实现 `ToolCatalog`：调 `ToolRegistry::tool_definitions_for_tier(ToolTier::Os)`，逐条转 `ToolSpec`
- [ ] C2.7 `max_tier` 常量放适配器内，`External` 排除（D15）；`fs_link` 为 `TOOLBOX_ONLY` 不会出现
- [ ] C2.8 注入开关：仅 `AgentKind::supports_tool_call()` 为真（现状 `custom`）取工具，`default` / `quick` / `deep` 恒 `None`（A9）
- [ ] C2.9 `ToolCatalog` 返回空 `Vec` 时 `tools = None`，循环退化为单次调用（A9）
- [ ] C2.10 OpenAI `ChatRequest` 新增 `tools: Option<Vec<Value>>` + `skip_serializing_if`，构造点 `openai/chat.rs` 填值
- [ ] C2.11 `openai/message_converter.rs` 新增 `ToolSpec` → `{ type: "function", function: { name, description, parameters } }` 转换
- [ ] C2.12 Anthropic `MessagesRequest` 新增 `tools` 字段，构造点 `anthropic/request.rs` 填值
- [ ] C2.13 `anthropic/message_converter.rs` 新增 `ToolSpec` → `{ name, description, input_schema }` 转换（无 `parameters` 包壳）
- [ ] C2.14 Gemini `convert_tools` 签名由 `&[(String, String, String)]` 改为 `&[ToolSpec]`
- [ ] C2.15 去掉 `convert_tools` / `sanitize_schema` / `strip_unsupported_schema_fields` 上的 `#[allow(dead_code)]`（转正）
- [ ] C2.16 Gemini `request.rs` 构造点填 `convert_tools(...)`；`code_assist.rs` 保持 `None`（不改 Code Assist 行为）
- [ ] C2.17 Anthropic 回填 `tool_use.input` 从字符串改为 JSON 对象，解析失败回退 `{}`（§4.3 必须修正项）
- [ ] C2.18 Gemini 回填 `functionCall.args` 从字符串改为 JSON 对象，解析失败回退 `{}`（§4.3 必须修正项）
- [ ] C2.19 Gemini `functionResponse` 必须带工具名：确认 `convert_tool_result_parts` 在 `name` 非空时产出 parts
- [ ] C2.20 测试：三 adapter `ToolSpec` 注入形状快照（A6 后半）
- [ ] C2.21 测试：带工具历史的第二轮请求三方言均合法（A19）
- [ ] C2.22 测试：`custom` 注入、`default` / `quick` / `deep` 不注入（A9）

---

## C3 RoundExecutor 契约与端口（P4-C3，§2.3、§2.4、§4.5、§6.1）

实现：`assistant-agent/src/agent/round_executor.rs`、`agent/tool_event.rs`、`kernel/ports.rs`、`kernel/ports/test_doubles.rs`

- [ ] C3.1 定义 `RoundExecutor` trait（`run_iterations(ctx) -> RoundOutcome`，`#[async_trait]`）
- [ ] C3.2 定义 `RoundOutcome`，含 `confirmation_wait_ms`（D12）与 `hit_cap` / `wrap_up_injected` 分离
- [ ] C3.3 定义 `TurnObserver { set_phase, emit_tool_event }`
- [ ] C3.4 定义 `TurnRunCtx`：`turn_id` / `conversation_id` / `iteration` / `remaining_turn_budget` / `child_token` / `ToolCatalog` / `ToolExecutionPort` / `EventSink` / `MessageStore` / `TurnInputSource` / `Clock` / `TurnLimits` / `TurnSpec`
- [ ] C3.5 `TurnRunCtx` 新增委派预留字段 `agent_id` / `parent_turn_id` / `delegation_depth`，P4 恒为根 agent 值（§2.4）
- [ ] C3.6 定义 `ToolExecutionPort` / `ToolExecCtx` / `ToolExecResult` / `ToolStage` / `ToolErrorClass`，全部声明在 `round_executor.rs`（§4.5）
- [ ] C3.7 `ToolExecCtx` 不含权限端口、不含 `TurnObserver`（§4.5）
- [ ] C3.8 `ToolExecResult` 字段与 `ToolOutcome` 的映射表逐项对齐（`content` 取 `assistant_text`，不取 `result_for_assistant`）
- [ ] C3.9 `ToolStage` / `ToolErrorClass` 为本地镜像枚举，不 import `assistant-tools` 类型
- [ ] C3.10 新建 `agent/tool_event.rs`：`ToolEvent`（上下文）+ `ToolEventPayload::{Started, ArgsDelta{delta}, Finished{result, confirmation_wait_ms}, Failed{error, error_kind, confirmation_wait_ms}}`（§6.1）
- [ ] C3.11 `kernel/ports.rs` 的 `TurnEvent::ToolStarted` / `ToolFinished` 收敛为 `TurnEvent::Tool(ToolEvent)`
- [ ] C3.11a 删除 `kernel/emit.rs` 的 `TurnEmitter::tool_started` / `tool_finished` 两个方法，改为单一 `tool_event(&self, event: ToolEvent)`（P3 已实现但无调用方，§6.1 现状核对）
- [ ] C3.12 `event_sink.rs` 的 `ToolStarted` / `ToolFinished` 两个 `match` 分支（P3 期只打 debug 日志）合并为 `TurnEvent::Tool(_)` 分支，线事件映射在 C6.7 接入
- [ ] C3.13 全仓 grep 确认 `ToolStarted` / `ToolFinished` 变体名已消失
- [ ] C3.14 `test_doubles.rs` 的 `MessageStore` 替身补 `save_tool_calls` / `append_tool_message`，支持「写入 → 读回 → 重建 wire 形状」
- [ ] C3.15 `test_doubles.rs` 新增 `ToolCatalog` / `ToolExecutionPort` 替身（可脚本化返回工具集与结果）
- [ ] C3.16 `round_executor.rs` / `tool_event.rs` 不出现 `tauri` / `AppHandle` / `assistant_tools`（A8）
- [ ] C3.17 测试：`ToolStage` / `ToolErrorClass` 与 `PipelineStage` / `ToolErrorKind` 一一对应（A18 前半）

---

## C4 执行器实现（P4-C4）

实现：`src-tauri/src/agent/round_executor.rs`（`ToolExecutionPort` 适配器、`TimingApprovalPrompter`）、`src-tauri/src/agent/**` 装配
测试：`src-tauri/tests/agent_loop.rs`

- [ ] C4.1 实现 `stop_hooks_all_continue`：`max_iterations` / `max_tool_calls` / turn 墙钟三维独立判定（D18）
- [ ] C4.2 `turn_deadline` 只读 TurnRunner 算好的值，executor 不重复计算
- [ ] C4.3 `before_model` 阶梯 + 中间件按 I36 Step 4.2 固定顺序执行
- [ ] C4.4 nudge 注入排在第 2.5 步（阶梯之后、模型调用之前），只进本次请求的 `Vec<ChatMessage>`（D19）
- [ ] C4.5 模型调用走 `provider.chat_completion_stream` + `kernel::stream::consume`，`model_calls += 1`
- [ ] C4.6 空响应重试：`empty_response_retries = 1`，不递增 `iteration`、递增 `model_calls`，重发前丢弃空白 assistant 行（D22）
- [ ] C4.7 累积器 `finalize` 取本轮 `tool_calls`，无工具则 `Completed`
- [ ] C4.8 收尾分支：`iteration == max_iterations` 时 `wrap_up_restore` + 撤回 tools + 收尾指令（D23）
- [ ] C4.9 `split_by_budget`：按原始顺序取前 `remaining_tool_calls` 个，其余进 `skipped`；墙钟为 0 时整批并入（§5.1）
- [ ] C4.10 `skipped` 结果形状：`is_error = true`、`stage = Gateway`、`error_kind = Denied`、content 写「未执行：…」
- [ ] C4.11 `skipped` 不参与 `after_tool` 中间件、不进 D19 计数、不进 D25 分类预算，但计入 `tool_calls`
- [ ] C4.12 实现 `ToolExecutionPort` 适配器：包 `Arc<assistant_tools::ToolPipeline>`，只做类型转换 + `confirmation_wait_ms` 注入 + 字段搬运
- [ ] C4.13 适配器不得调用 `partition_tool_batches` / `select_persist_targets`（§4.5）
- [ ] C4.14 实现 `TimingApprovalPrompter` 装饰器 + `WaitGuard`（`Drop` 记录 `Instant::elapsed`），按 `tool_call_id` 映射等待时长（§6.4）
- [ ] C4.15 适配器在 `pipeline.execute` 返回后按 `tool_call_id` 取出并 `remove`，避免无界增长
- [ ] C4.16 未走审批（`Allow` 直通）时 `confirmation_wait_ms = 0`
- [ ] C4.17 `RoundOutcome.confirmation_wait_ms` 汇总本 turn 全部等待
- [ ] C4.18 承接 P2 遗留生产端口（现状：`src-tauri` 只有 `ToolRegistry`，其余全部未装配，见 spec §6.4）
- [ ] C4.18a 新建 `src-tauri/src/adapters/` 下的 `PermissionGrantStore` / `PermissionAuditStore` 实现（`Mutex<Connection>` + 同步 rusqlite，沿用 `DbMessageStore::with_conn` 模式）
- [ ] C4.18b `permission_grants` / `permission_audit` 两表（`CREATE TABLE IF NOT EXISTS`，D14）随本批先建（C6.13 复用同一段建表代码，不重复）
- [ ] C4.18c `ApprovalPrompter` 生产默认实现「Ask 即 Deny + 写审计」（`reason = "approval channel not available in P4"`，fail-closed；P6 替换内层，装饰器结构不变）
- [ ] C4.18d 装配 `ToolPipeline` / `ToolGateway` / `ToolResultStorage` / `PipelineEventSink` 并在 `setup.rs` 与 `TurnRunner` 之间接线
- [ ] C4.18e `src-tauri` 全仓 grep 确认无遗留未绑定端口（`PermissionGrantStore` / `PermissionAuditStore` / `ApprovalPrompter` / `PipelineEventSink` / `ToolResultStorage` 均有生产实例）
- [ ] C4.19 承接取消打标：turn 取消时整批结果标 `Cancelled`，content 写「已取消」（§4.4）
- [ ] C4.20 `append_tool_results`：回填请求历史 + 写 `role='tool'` 行，两者文本逐字节一致（D21 / D11）
- [ ] C4.21 `after_tool` 阶梯顺序固定为 D26（豁免）→ D25（分类预算）→ D19（通用 2/3 阶梯）
- [ ] C4.22 `halted_with_summary`：用根因摘要覆盖最终文本，`status = Failed`
- [ ] C4.23 所有长 await 包 `tokio::select! { biased; _ = token.cancelled() => ... }`
- [ ] C4.24 测试：3 迭代闭环（A1）
- [ ] C4.25 测试：`max_iterations` 收尾（A2）
- [ ] C4.26 测试：空响应重试（A3 / A15）
- [ ] C4.27 测试：取消路径（A4）
- [ ] C4.28 测试：工具失败回填后继续、`Denied` 不重试（A5）
- [ ] C4.29 测试：三维预算各自单独触发（A12）
- [ ] C4.30 测试：预算内裁剪与 wire 合法性（A12b）
- [ ] C4.31 测试：重复失败 2/3 阶梯与 nudge 不落库（A13）
- [ ] C4.32 测试：并行批次墙钟与顺序（A14）
- [ ] C4.33 测试：失败分类阶梯（A17）
- [ ] C4.34 测试：审批计时装饰器（sleep 20ms 替身，断言 `>= 20`）

---

## C5 TurnRunner 切换（P4-C5）

实现：`assistant-agent/src/kernel/runner.rs`

- [ ] C5.1 `TurnRunner` 切到 `executor.run_iterations(ctx)`
- [ ] C5.2 删除 P3 的过渡单迭代循环，全仓无第二套循环体；**本项与 C1.13a–C1.13c 同批交付**（C1 删 `StreamOutcome.tool_calls` 后过渡循环即不可编译）
- [ ] C5.3 一次性替换，**不保留**运行时双路径开关
- [ ] C5.4 测试：chat 路径事件序列与 P3 对比一致（A9）
- [ ] C5.5 测试：chat 路径无 tool 消息、无 `tools` 注入（A9）

---

## C6 事件 DTO 与 tool 落库（P4-C6，D5 / D11 / D14、§6.1.1 / §6.2 / §6.3）

实现：`src-tauri/src/protocol/dto/agent.rs`、`protocol/events.rs`、`adapters/event_sink.rs`、`assistant-storage/src/**`、`assistant-agent/src/prompt_builder.rs`、`src/adapters/message_store.rs`
测试：`src-tauri/tests/protocol_dto_roundtrip.rs`、`src-tauri/tests/protocol_bindings.rs`、`assistant-storage` 集成测试

- [ ] C6.1 新建 `src-tauri/src/protocol/dto/agent.rs`：`AgentToolCallDto`（`deny_unknown_fields`）+ `AgentToolCallPayloadDto`（`#[serde(tag = "phase", rename_all = "camelCase")]`，四变体）
- [ ] C6.2 变体内多词字段用显式 `#[serde(rename = "confirmationWaitMs")]` / `"errorKind"`，不用 `rename_all_fields`
- [ ] C6.3 `Failed` 变体复用 `ToolErrorKindDto`，不新建第三个错误类别枚举
- [ ] C6.4 `Option` 字段不加 `skip_serializing_if`（键恒在、值 `null`）
- [ ] C6.5 `protocol/dto/mod.rs` 加 `pub mod agent;` 与 `pub use agent::*;`
- [ ] C6.6 `protocol/events.rs` 加 `AGENT_TOOL_CALL = "agent-tool-call"` 常量
- [ ] C6.7 `event_sink.rs` 实现 `TurnEvent::Tool(e)` → `AgentToolCallDto` 映射并 `emit(AGENT_TOOL_CALL, ...)`，只做字段搬运
- [ ] C6.8 `protocol_bindings.rs` 的 `use` 与 `declarations` 加两类型（计数断言已清理，无需改数字）；跑 `cargo test --test protocol_bindings`，集合断言须通过
- [ ] C6.9 `protocol_dto_roundtrip.rs` 加 `agent_tool_call_dto_roundtrip`，四变体各一条，`confirmation_wait_ms` 取 `Some` / `None`
- [ ] C6.10 `npm run contract:gen` 生成 `protocol.contract.ts`，确认四变体为 discriminated union（A10）
- [ ] C6.11 `assistant-storage/src/db.rs`：`CREATE TABLE messages` 加 `tool_call_id TEXT` 与 `is_error INTEGER NOT NULL DEFAULT 0`
- [ ] C6.12 `db.rs` 既有库分支追加两段 `pragma_table_info` 守卫 + `ALTER TABLE messages ADD COLUMN ...`，放在 `model_id` 守卫之后
- [ ] C6.13 `db.rs` 建表段新增 `permission_grants` / `permission_audit` 两表（`CREATE TABLE IF NOT EXISTS`，幂等，不做 ALTER 迁移，D14）；若 C4.18b 已提前落地，此处只核对不重复建
- [ ] C6.14 `conversations/mod.rs`：`MSG_SELECT` 追加两列（13 → 15）；`MessageRow` 加 `tool_call_id: Option<String>` / `is_error: bool`
- [ ] C6.15 `read_msg_row` 用具名列同步（`row.get("tool_call_id")` / `row.get::<_, i64>("is_error")? != 0`）
- [ ] C6.16 补齐 `MessageRow` 结构体字面量的两字段：`src/cmd/compaction.rs`、`src/cmd/chat/insert.rs`（两处）、`src/adapters/message_store.rs`；不得改成 `..Default::default()`
- [ ] C6.17 `msg_ops.rs` / `branch.rs` 新增带 `tool_call_id` / `is_error` 的插入路径（供 `append_tool_message`），既有 INSERT 不改（新列靠 `DEFAULT` / `NULL`）
- [ ] C6.18 `DbMessageStore` 用 `with_conn` 模式实现 `save_tool_calls` / `append_tool_message`，不引入 `spawn_blocking` 或连接池
- [ ] C6.19 `append_tool_message` 的 `parent_id` 指向 assistant 行、`sibling_index = 0`；不新增「按 role 分桶的 sibling_index」
- [ ] C6.20 `prompt_builder::prepare_messages` 从 `tool_calls_json` / `tool_call_id` / `is_error` 重建 `ChatMessage`（替换现有硬编码 `None` / `false`）
- [ ] C6.21 `prompt_builder` 建 `call_id → tool_name` 映射，给 `role='tool'` 行填 `name`（Gemini 必需）
- [ ] C6.22 纯工具调用的 assistant 行 `content` 写空串（`NOT NULL` 约束），不写 `NULL`
- [ ] C6.23 迁移只 `ADD COLUMN`，无 `DROP` / `RENAME` / 存量行重写
- [ ] C6.24 测试：tool 落库往返（写入 → 读回 → 重建 wire 形状，顺序一致）（A11）
- [ ] C6.25 测试：旧库迁移后 `tool_call_id` 为 `NULL`、`is_error` 为 `0`
- [ ] C6.26 测试：`append_tool_message` 不参与分支兄弟计数

---

## C7 端到端测试（P4-C7）

测试：`src-tauri/tests/agent_loop_e2e.rs`、`src-tauri/tests/agent_loop_dialect.rs`

- [ ] C7.1 3 迭代闭环 + tool 落库往返联合用例（A1 / A11）
- [ ] C7.2 收尾调用不含 `tools`、`hit_cap` / `wrap_up_injected` 断言（A2）
- [ ] C7.3 空响应重试后仍空 → `Failed`，`messages` 无空白行（A3 / A15）
- [ ] C7.4 取消：部分内容落库、`status = interrupted`、结果标 `Cancelled`、状态回 `Idle`（A4）
- [ ] C7.5 工具失败回填后继续；`Denied` 不重试（A5）
- [ ] C7.6 三维预算与预算内裁剪（A12 / A12b）
- [ ] C7.7 nudge / halt 与 `messages` 无新增行（A13）
- [ ] C7.8 并行批次：墙钟小于串行和、结果顺序与 `tool_calls` 一致（A14）
- [ ] C7.9 收尾还原：微压缩清空结果后结论调用可见最近结果，后续轮不保留；摘要调用不推进 `iteration` 但计入 `model_calls`（A16）
- [ ] C7.10 失败分类阶梯与成功清零（A17）
- [ ] C7.11 端口镜像不漂移 + `assistant-agent` 源码无 `assistant_tools` 引用（A18）
- [ ] C7.12 回填方言：Anthropic `input` 对象、Gemini `args` 对象、`functionResponse` 带工具名、非法 arguments 回退 `{}` 不 400（A19）
- [ ] C7.13 累积器唯一实现（A7）

---

## C8 验收命令

`cwd = src-tauri`

- [ ] C8.1 `cargo test -p assistant-agent` 全绿
- [ ] C8.2 `cargo test -p assistant-providers` 全绿
- [ ] C8.3 `cargo test -p assistant-storage` 全绿
- [ ] C8.4 `cargo test -p assistant-app --test protocol_dto_roundtrip` 通过（A10）
- [ ] C8.5 `cargo test --test assembly_contract` 通过（A10）
- [ ] C8.6 `cargo test -p assistant-app --test agent_loop_e2e` 与 `--test agent_loop_dialect` 通过
- [ ] C8.7 `cargo clippy -p assistant-agent -p assistant-providers -p assistant-storage --all-targets -- -D warnings` 无告警
- [ ] C8.8 先执行格式化修复，再 `cargo fmt --all --check` 通过
- [ ] C8.9 `cargo tree -p assistant-agent` 中无 `tauri`、无 `assistant-tools`（A8）
- [ ] C8.10 grep 确认 `agent/round_executor.rs` / `agent/tool_event.rs` / `agent/tool_call.rs` 无 `AppHandle` / `Window` / `assistant_tools`
- [ ] C8.11 `npm run contract:check` 幂等（无 diff）
- [ ] C8.12 `cargo test --workspace --no-run` 确认全量编译通过
- [ ] C8.13 确认未改 `assistant-tools` 任何文件（`git status` 路径核对）

---

## C9 文档

- [ ] C9.1 更新 [roadmap](./I38-agent-roadmap.md) §2：P4 标记已完成并记录日期（阶段状态唯一台账）
- [ ] C9.2 在本清单勾选 C1–C8 完成项（任务级进度只在本清单维护）
- [ ] C9.3 确认 I38b 规格与 I36 spec 无进度表述（spec 只写「怎么做」）

---

## 验收映射

| 规格验收 | 对应任务 |
|---|---|
| A1 三迭代闭环 | C4.24、C7.1 |
| A2 收尾调用 | C4.8、C4.25、C7.2 |
| A3 空响应重试 | C4.6、C4.26、C7.3 |
| A4 取消 | C4.19、C4.27、C7.4 |
| A5 工具失败 / `Denied` | C4.21、C4.28、C7.5 |
| A6 三 adapter 注入形状 + Gemini 快照语义 | C1.21、C2.20 |
| A7 累积器唯一实现 | C1.13、C1.14、C7.13 |
| A8 无 tauri / 无 assistant-tools 依赖 | C3.16、C8.9、C8.10 |
| A9 chat 路径行为不变 | C2.8、C2.9、C2.22、C5.4、C5.5 |
| A10 契约生成与声明集合一致 | C6.8、C6.9、C6.10、C8.4、C8.5 |
| A11 tool 落库往返 | C6.14 – C6.21、C6.24、C7.1 |
| A12 三维预算 | C4.1、C4.29、C7.6 |
| A12b 预算内裁剪 | C4.9 – C4.11、C4.30、C7.6 |
| A13 重复失败阶梯 | C4.4、C4.21、C4.31、C7.7 |
| A14 工具执行策略 | C4.12、C4.32、C7.8 |
| A15 空响应不落库 | C4.6、C4.26、C7.3 |
| A16 收尾还原与压缩计数 | C4.8、C7.9 |
| A17 失败分类阶梯 | C4.21、C4.33、C7.10 |
| A18 端口镜像 | C3.9、C3.17、C7.11 |
| A19 回填方言 | C2.17 – C2.19、C2.21、C7.12 |
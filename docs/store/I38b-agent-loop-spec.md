# I38b — P4：Agent Loop 闭环规格

| 项 | 值 |
|---|---|
| 状态 | Draft（未开始） |
| 日期 | 2026-11-04 |
| 阶段 | P4（见 [I38 roadmap](./I38-agent-roadmap.md)） |
| 关联 | [I36 spec](./I36-agent-runtime-spec.md)（turn 生命周期与端口）、[I38a](./I38a-tool-control-layer-spec.md)（ToolPipeline）、[I30](./I30-phase1-toolcall-separation-spec.md) §6（B 段决策）、[I18 design](./I18-agent-tool-architecture-design.md) §3.4 / §3.15 |
| 影响范围 | `assistant-agent`（`agent/` 累积器与 RoundExecutor trait）、`assistant-providers`（`ChatOptions.tools` 与三 adapter 注入）、`src-tauri/src/agent/`（RoundExecutor 实现与装配）、`src-tauri/src/protocol`（工具事件 DTO） |

> 本文是 P4 的唯一权威规格。I18 spec 的 Step 7 与 I30 §6 的 B 段内容已迁入本文。
> **前置未完成时不得开工**：P2（ToolPipeline）与 P3（TurnRunner 单迭代形态）必须先通过各自验收。

---

## 1 目标与非目标

### 1.1 目标

| # | 交付 |
|---|---|
| 1 | 多迭代闭环：模型调用 → 提取 tool_calls → 执行 → 回填 → 继续生成 |
| 2 | 唯一 tool call 流式累积器（迁入 `assistant-agent/src/agent/tool_call.rs`） |
| 3 | `ChatOptions.tools` 通道与 OpenAI / Anthropic / Gemini 三 adapter 注入 |
| 4 | 收尾调用、空响应重试、迭代上限、取消与部分结果的完整闭环 |
| 5 | 工具事件（开始 / 参数增量 / 完成 / 失败）与落库（`tool_calls_json`） |
| 6 | RoundExecutor 接管迭代循环，TurnRunner 只保留 turn 生命周期 |

### 1.2 非目标

- 具体 OS 工具实现（P5）。
- 前端五态卡片与审批 UI（P6）；本阶段只发射事件与 DTO。
- MCP（P7）。
- 压缩从终止式切续跑式：切换点在 P4 完成**之后**单独提交（见 I36 design D9），本文只要求 RoundExecutor 就位，不要求同批切换。
- 子代理、排队输入、USD 成本上限（I36 design §3.19）；子代理本阶段只做契约预留（§2.4），不实现 spawn。

---

## 2 职责边界（关键决策）

### 2.1 三层职责

| 层 | 归属 | 职责 |
|---|---|---|
| TurnRunner | `assistant-agent/src/kernel/runner.rs` | turn 开始/结束、状态机、双层超时、取消代际、终态落库、事件主干、phase 投影 |
| **RoundExecutor** | trait 在 `assistant-agent/src/agent/round_executor.rs`；实现在 `src-tauri/src/agent/round_executor.rs` | 迭代循环：停止钩子 → 阶梯/`before_model` → 模型调用 → 工具执行 → `after_tool` / `after_iteration` → 下一迭代；收尾调用；空响应重试 |
| Provider 流式 | `assistant-providers` | 单次请求：投影、流式片段、usage、单次超时 |

**决策**：P3 的 TurnRunner 内部循环是**过渡形态**（单迭代）。P4 起：

```
TurnRunner::run(ctx, spec):
  begin / 状态机 / 预算
  outcome = executor.run_iterations(TurnRunCtx { ports, limits, spec, token, observer })
  persist_terminal / finish_turn / emit_terminal / 清理
  return outcome
```

- TurnRunner **不再**直接消费 model stream；它只通过 `RoundOutcome` 得到文本、usage、计数。
- 每迭代的 phase 变化由 executor 通过 `TurnObserver`（由 TurnRunner 传入）上报；`SessionStates` 仍是唯一写方。
- 停止钩子、上下文阶梯、收尾调用属于**迭代语义**，随循环迁入 executor，不留在 TurnRunner。
- 双层超时的**计算**在 TurnRunner（turn deadline），**执行**在 executor（每迭代传入剩余预算）。

### 2.2 依赖方向（修正 I18 spec Step 7 的落点）

| 组件 | 位置 | 可依赖 |
|---|---|---|
| `agent/tool_call.rs` | `assistant-agent` | `assistant-providers`（流类型）；**不得**依赖 `assistant-tools` |
| `agent/round_executor.rs`（trait + `RoundOutcome`） | `assistant-agent` | 仅依赖 `kernel/ports` 与 provider 流类型；**不得**依赖 `assistant-tools` |
| RoundExecutor 实现 | `src-tauri/src/agent/round_executor.rs` | `assistant-agent` + `assistant-tools`（ToolPipeline / ToolRegistry） |
| `kernel/request.rs` | `assistant-agent` | 通过 `ToolCatalog` 端口取得 provider 中立工具定义；不直接 import `assistant-tools` |

**理由**：I36 design §4 规定 `kernel` / `agent` → `assistant-tools` 禁止，只依赖抽象。实现层放在 `src-tauri` 才能同时持有 Pipeline 与运行时，不破坏 crate 依赖方向。

### 2.3 接口契约

```rust
// assistant-agent/src/agent/round_executor.rs
#[async_trait]
pub trait RoundExecutor: Send + Sync {
    async fn run_iterations(&self, ctx: TurnRunCtx<'_>) -> RoundOutcome;
}

pub struct RoundOutcome {
    pub status: TurnStatus,
    pub text: String,
    pub iterations: u32,
    pub model_calls: u32,
    pub tool_calls: u32,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub cache_read_tokens: Option<u32>,
    pub hit_cap: bool,
    pub wrap_up_injected: bool,
    pub error: Option<TurnError>,
}

pub trait TurnObserver: Send + Sync {
    fn set_phase(&self, phase: TurnPhase);
    fn emit_tool_event(&self, event: ToolEvent);
}
```

`TurnRunCtx` 由 TurnRunner 构造，携带：`turn_id` / `conversation_id` / `iteration` 起点 / `remaining_turn_budget` / `child_token` / `ToolCatalog` / `EventSink` / `MessageStore` / `TurnInputSource` / `Clock` / `TurnLimits` / `TurnSpec`，以及 §2.4 的委派预留字段（P4 恒为根 agent 值）。

**硬约束**：`RoundExecutor` trait 与 `RoundOutcome` 不得出现 `tauri`、`AppHandle`、`assistant_tools` 类型。

### 2.4 委派预留（P8 前置，零行为变化）

P4 **不实现**子代理，但契约必须可扩展，否则 P8 要重改 trait、事件与落库格式。

| 预留点 | P4 落地 | P8 扩展 |
|---|---|---|
| `TurnRunCtx` | 新增可选 `agent_id` / `parent_turn_id` / `delegation_depth`，P4 恒为根 agent 值 | 子 agent turn 填父 turn 与深度 |
| `ToolCatalog` | 支持按 agent 作用域收窄，P4 只有全集 | 子 agent 只拿到父授予的工具子集 |
| 工具入口 | 子代理作为 `spawn_agent` 工具，经 ToolPipeline | P8.1 实现 `spawn_agent` |
| 取消 | `child_token` 由父令牌派生 | 取消树向下传播（I36 D18） |
| 事件 | `agent-tool-call` / turn 事件可选携带 `agentId` / `parentTurnId` | 前端按 agent 过滤与投影 |

**硬约束**：

1. 新增字段必须有默认值，`RoundOutcome` 的现有字段与语义不变。
2. 不得在 `RoundExecutor` 内写 `if is_subagent` 分支；委派只能作为工具进入。
3. `assistant-agent` 不得因此依赖 `assistant-tools`（沿用 §2.2 的依赖方向）。
4. 上述字段在 P4 的对比测试中必须恒为根 agent 值，不得改变 chat 行为（A9）。

---

## 3 Tool call 累积器（I30 B 段决策落地）

### 3.1 唯一实现

- 迁入 `assistant-agent/src/agent/tool_call.rs`。
- 删除 `assistant-providers/src/tool_call_accumulator.rs`（D8 完成后）。
- 输入是 provider 流类型（`UnifiedResponse` / `ChatDelta` 片段），输出是完整的 `FinalizedToolCall`。
- **不得**再出现第三套累积器；chat 路径已无累积器（P0 已剥离）。

### 3.2 D1-D10 决策表（P4 开工前定稿）

| # | 决策点 | 结论 | 说明 |
|---|---|---|---|
| D1 | 流式键策略 | `ToolCallStreamKey::{Indexed(usize), Unindexed}` | 保留 providers 现状；Anthropic 若提供 index 则用 Indexed，否则回落 Unindexed |
| D2 | 边界枚举 | 保留 `ToolCallBoundary::{NewTool, FinishReason, StreamEnd}` | `NewTool` 用于流式提前终结上一个工具；删除会让 `finalized_previous` 语义失效 |
| D3 | 截断语义 | 先 `repair_truncated_json`，修复可用则降级（`is_error=false` + 修复提示）；修复失败再报 `Truncated` | 两者并存，不二选一 |
| D4 | 入口形态 | 保留 `apply_unified_response`，底层 `apply_delta` 同时存在 | agentloop 的输入天然是 `UnifiedResponse` |
| D5 | 事件字段 | `ToolEvent { turn_id, iteration, call_id, name, phase, args_delta?, result? }` | 供 P6 五态渲染；`phase ∈ Started / ArgsDelta / Finished / Failed` |
| D6 | 模块落点 | `assistant-agent/src/agent/tool_call.rs` | 见 §2.2 依赖方向 |
| D7 | 测试口径 | 跨 chunk 累积 / 快照模式 / 双键 / 边界 / 截断 / 非法 JSON 六类 | 与实现同批交付 |
| D8 | Gemini 累积上移 | **上移**：provider 只透传快照片段并标 `arguments_is_snapshot = true`；累积统一由 `agent/tool_call.rs` 完成 | 根治 F15/F16 |
| D9 | 快照标记承载 | `ChatDelta` 新增 `delta_tool_call_arguments_is_snapshot: bool`（provider 内部流类型，非 wire） | 不新增独立流出口，避免第三套格式 |
| D10 | tools 注入 | P4 统一新增 `ChatOptions.tools`，三 adapter 同批实现 | Step 7 真正发起工具调用的前置 |

> D1/D2/D3 的取值以 `assistant-providers` 现实现为基线；D8 上移后 provider 回归无状态。

### 3.3 快照与增量语义

| 来源 | `arguments_is_snapshot` | 累积器行为 |
|---|---|---|
| OpenAI | `false` | 追加 JSON 片段 |
| Anthropic | `false` | 追加 JSON 片段（若提供 index，按 index 分组） |
| Gemini | `true` | 用快照替换当前工具参数，不追加 |

**验收**：三个 adapter 对同一逻辑工具调用产出的 `FinalizedToolCall` 等价；Gemini 的 `arguments_is_snapshot` 与实际语义一致（修 F15）。

---

## 4 tools 通道

### 4.1 provider 中立类型（`assistant-providers/src/types/tool.rs`）

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}
```

- `ToolSpec` 是 provider 输入形状，不携带 `tier` / `audience` / `behavior_hints`。
- `ToolDefinition`（`assistant-tools`）→ `ToolSpec` 的转换在 `src-tauri` 适配层完成，经 `ToolCatalog` 端口进入 `kernel/request.rs`。

### 4.2 ChatOptions

```rust
pub struct ChatOptions {
    // ...现有字段...
    #[serde(default)]
    pub tools: Option<Vec<ToolSpec>>,
}
```

- `None` / 空数组 = 不声明工具（chat 路径行为不变）。
- 现有 `ChatMessage.tool_calls` / `tool_call_id` 已存在（F12），本阶段开始被填充。

### 4.3 三 adapter 注入

| Provider | wire 形状 | 注意 |
|---|---|---|
| OpenAI | `tools: [{ type: "function", function: { name, description, parameters } }]` | `parameters` 直接使用 `input_schema` |
| Anthropic | `tools: [{ name, description, input_schema }]` | 不支持 `parameters` 包壳 |
| Gemini | `tools: [{ functionDeclarations: [{ name, description, parameters }] }]` | 需 schema 方言转换；复用现有 `gemini/message_converter.rs::convert_tools`，删除其死代码状态 |

**约束**：转换只在 adapter 内进行；`ToolSpec` 不得直接序列化成任一 provider 的 wire 形状。

### 4.4 回填消息形状

```
assistant message: { role: "assistant", content: Option<String>, tool_calls: Vec<ToolCall> }
tool message:      { role: "tool", tool_call_id: <id>, content: <result_for_assistant> }
```

- 一个 tool_call 对应一条 tool message；顺序与 `tool_calls` 一致。
- `result_for_assistant` 缺省时用 `data` 的紧凑序列化。
- `error_kind = Cancelled` 时 content 写「已取消」，不当作失败重试。

---

## 5 循环流程

```
run_iterations(ctx) -> RoundOutcome:

  for iteration in 1..=max_iterations:
    // 1) 刹车先于花钱
    if !stop_hooks_all_continue(TurnState { iteration, tool_calls, tokens, elapsed }):
        return stopped(reason)
    if turn_deadline_reached(): return timeout("turn wall clock")

    // 2) before_model：阶梯 + 中间件（顺序固定，见 I36 spec Step 4.2）
    observer.set_phase(Thinking)
    for mw in before_model: if StopTurn -> return stopped

    // 3) 单次模型调用
    observer.set_phase(Streaming)
    let stream = provider.chat_completion_stream(request_with_tools, model_call_budget, child_token)
    let res = kernel::stream::consume(stream, child_token, model_call_budget, on_delta)
    model_calls += 1
    match res:
      Cancelled(partial) -> persist_partial; return Cancelled
      Timeout(partial)   -> persist_partial; return Failed
      Provider(e, partial) -> persist_partial; return Failed(e)

    // 4) 空响应重试（不递增 iteration）
    if !has_effective_output && retries_used < empty_response_retries:
        retries_used += 1; continue

    // 5) 无工具 → 结束
    let Some(tool_calls) = accumulator.finalize(iteration) else { return Completed }

    // 6) 最后一轮：收尾（撤回 tools + 追加收尾指令）
    if iteration == max_iterations:
        let wrapped = call_model_without_tools(wrap_up_instruction)
        return Completed { hit_cap: true, wrap_up_injected: true, text: wrapped.text }

    // 7) 工具执行（ToolPipeline）
    observer.set_phase(ToolCalling)
    let results = pipeline.execute(tool_calls, ctx).await
    for r in results: after_tool_middlewares(&mut r)
    append_tool_results(results)
    tool_calls_count += n

    // 8) 迭代后钩子
    for mw in after_iteration: if StopTurn -> return stopped

  return Completed { hit_cap: true }   // 仅当收尾被禁用
```

**硬约束**：

- 工具执行受**自身超时**与 turn 剩余约束，**不继承** `model_call` 上限（I36 design D4）。
- 收尾调用占第 `max_iterations` 个名额，不额外加一轮；收尾调用不含 tools，但不得绕过审批（它不执行工具）。
- `empty_response_retries` 的 `continue` 不递增 `iteration`。
- 工具执行被取消时返回 `Cancelled` 的结果回填给模型，不当作失败。
- 所有长 await 必须包在 `tokio::select! { biased; _ = token.cancelled() => ... }`。

---

## 6 事件与落库

### 6.1 事件

| 事件 | 时机 | 载荷 |
|---|---|---|
| `agent-tool-call` | 工具开始 / 参数增量 / 完成 / 失败 | `turn_id` / `iteration` / `call_id` / `name` / `phase` / `argsDelta` / `result` |
| `chat-delta` | 文本 / 推理增量 | 追加 `iteration`（P3 已加） |
| `session-state-changed` | phase 变化 | 由 TurnObserver → TurnRunner → EventSink 发射 |

### 6.2 落库

- 助手消息的 `tool_calls_json` 复用预留列（I30 §5.3），写入完整 `FinalizedToolCall` 列表。
- 工具结果不单独建表：回填内容随下一轮 assistant 消息历史进入请求；如需审计，用 P2 的 `PermissionAuditStore` 端口。
- 若 P4 决定新建 `tool_calls` 表，必须先更新 I36 spec 的 D10 与迁移脚本，不在本文暗改。

---

## 7 实施步骤

| 步骤 | 内容 | 依赖 |
|---|---|---|
| P4-C1 | 迁移累积器到 `agent/tool_call.rs`；落地 D1-D9；删除 providers 侧累积器；三 adapter 快照语义修正 | P3 |
| P4-C2 | `ToolSpec` + `ChatOptions.tools` + 三 adapter 注入；`convert_tools` 转正 | P4-C1 |
| P4-C3 | RoundExecutor trait / `RoundOutcome` / `TurnObserver`；`TurnRunCtx` | P3 |
| P4-C4 | `src-tauri/src/agent/round_executor.rs` 实现：stop hooks、阶梯、模型调用、Pipeline 执行、回填、收尾 | P2、P4-C1/C2/C3 |
| P4-C5 | TurnRunner 切到 `executor.run_iterations`；删除 P3 的过渡循环 | P4-C4 |
| P4-C6 | 事件与落库：`agent-tool-call` DTO、`tool_calls_json` 写入、契约生成 | P4-C4 |
| P4-C7 | 端到端测试：3 迭代闭环、收尾、空响应、取消、工具失败 | P4-C5/C6 |

**切换约束**：P4-C5 之前，`TurnRunner` 不得同时保留旧循环与 executor 两条路径；用一次性替换，不做运行时开关。

---

## 8 验收标准

| # | 标准 |
|---|---|
| A1 | 三迭代闭环：模型请求工具 → 执行 → 回填 → 再次请求工具 → 最终文本，消息序列与 tool 消息数量正确 |
| A2 | `max_iterations` 触发收尾：最后一次请求不含 `tools`，`hit_cap = true`、`wrap_up_injected = true` |
| A3 | 空响应（有推理无正文）重试 1 次后仍空 → `Failed`，`iteration` 不因重试递增 |
| A4 | 取消：部分内容落库、`status = interrupted`、工具结果标 `Cancelled`、状态回 `Idle` |
| A5 | 工具失败（`error_kind = Business`）回填给模型后可继续下一迭代；`Denied` 不重试 |
| A6 | 三个 adapter 的 `ToolSpec` 注入形状有快照测试；Gemini 快照语义与 `arguments_is_snapshot` 一致 |
| A7 | 累积器只有一处实现：`assistant-providers/src/tool_call_accumulator.rs` 已删除，全仓无第二份 |
| A8 | `cargo tree -p assistant-agent` 无 `tauri`、无 `assistant-tools`；`agent/round_executor.rs` 仅含 trait 与值类型 |
| A9 | chat 路径行为不变：无 `tools`、无 tool 消息、事件序列与 P3 对比测试一致 |
| A10 | 事件 DTO 经 `npm run contract:gen` 生成，`cargo test --test assembly_contract` 通过 |

---

## 9 风险与开放问题

| 风险 | 缓解 |
|---|---|
| 三 adapter 快照语义再次分叉 | D9 单字段 + A6 快照测试；Gemini 累积上移（D8） |
| 工具结果过长挤爆上下文 | P2 的三层预算与落盘在 executor 内必经 |
| 收尾调用被误当成新迭代 | 明确「收尾占第 N 个名额」；`hit_cap` 与 `wrap_up_injected` 分开 |
| 依赖方向被破坏 | A8 以 `cargo tree` 断言；RoundExecutor 实现只允许在 `src-tauri` |
| P4-C5 切换导致 chat/agent 双路径漂移 | 一次性替换，不做运行时开关；A9 对比测试 |

**开放问题**：

1. 工具结果的 `tool` 消息是否复用现有 `MessageRow` 的 `role = tool`，还是只存在请求历史中（不落库）？当前倾向：落库（便于重启后恢复），但需与 I36 spec D10「turn 结构不落库」协调。
2. 审批等待期是否继续计入 turn 墙钟？I36 spec §11 开放问题 1；P4 与 P6 联调时定稿。
3. 压缩续跑式切换是否与 P4 同批？建议独立提交，先保持终止式（I36 design D9）。

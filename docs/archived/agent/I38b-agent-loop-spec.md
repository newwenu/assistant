# I38b — P4：Agent Loop 闭环规格

| 项 | 值 |
|---|---|
| 状态 | 现行（P4 规格，决策已全部拍板；进度见 [roadmap](../../store/I38-agent-roadmap.md) §2） |
| 日期 | 2026-11-05 |
| 阶段 | P4（见 [I38 roadmap](../../store/I38-agent-roadmap.md)） |
| 关联 | [I36 spec](./I36-agent-runtime-spec.md)（turn 生命周期与端口）、[I38a](./I38a-tool-control-layer-spec.md)（ToolPipeline）、[I30](./I30-phase1-toolcall-separation-spec.md) §6（B 段决策）、[I18 design](../../store/I18-agent-tool-architecture-design.md) §3.4 / §3.15 |
| 影响范围 | `assistant-agent`（`agent/` 累积器与 RoundExecutor trait）、`assistant-providers`（`ChatOptions.tools` 与三 adapter 注入）、`assistant-agent/src/agent/loop_executor.rs`（RoundExecutor 实现）、`src-tauri/src/adapters/` + `src-tauri/src/tool_setup.rs`（`ToolExecutionPort` 适配器、`TimingApprovalPrompter`、装配）、`src-tauri/src/protocol/dto/agent.rs`（新增 agent 事件 DTO 模块，§6.1.1）、`assistant-storage`（`messages` 新增 `tool_call_id` / `is_error` 两列（D11）+ `permission_grants` / `permission_audit` 两表（D14）） |

> 本文是 P4 的唯一权威规格。I18 spec 的 Step 7 与 I30 §6 的 B 段内容已迁入本文。
> **前置未完成时不得开工**：P2（ToolPipeline）与 P3（TurnRunner 单迭代形态）必须先通过各自验收。
> 执行清单：[I38b-agent-loop-tasklist.md](./I38b-agent-loop-tasklist.md)（C0–C9 逐项勾选与验收映射）。
> 实施指南：[I38b-agent-loop-impl-guide.md](./I38b-agent-loop-impl-guide.md)（未完成步骤细化、§5 算法讲解提取）。

---

## 1 目标与非目标

### 1.1 目标

| # | 交付 |
|---|---|
| 1 | 多迭代闭环：模型调用 → 提取 tool_calls → 执行 → 回填 → 继续生成 |
| 2 | 唯一 tool call 流式累积器（迁入 `assistant-agent/src/agent/tool_call.rs`） |
| 3 | `ChatOptions.tools` 通道与 OpenAI / Anthropic / Gemini 三 adapter 注入 |
| 4 | 收尾调用、空响应重试、迭代上限、取消与部分结果的完整闭环 |
| 5 | 工具事件（开始 / 参数增量 / 完成 / 失败）与落库：`tool_calls_json` + 独立 `role='tool'` 结果行（D11） |
| 6 | RoundExecutor 接管迭代循环，TurnRunner 只保留 turn 生命周期 |
| 7 | 上下文阶梯 ③④⑤（微压缩 / 收尾还原 / 硬裁剪）与固定顺序中间件链（①③④⑤⑥⑦⑧，② 不在本阶段） |

### 1.2 非目标

- 具体 OS 工具实现（P5）。
- 前端五态卡片与审批 UI（P6）；本阶段只发射事件与 DTO。
- MCP（P7）。
- 压缩从终止式切续跑式**以及阶梯 ② 摘要压缩**：切换点在 P4 完成**之后**单独提交（见 I36 design D9、D13），本文只要求 RoundExecutor 就位，不要求同批切换，也不实现摘要执行体。
- 子代理、排队输入、USD 成本上限（I36 design §3.19）；子代理本阶段只做契约预留（§2.4），不实现 spawn。

---

## 2 职责边界（关键决策）

### 2.1 三层职责

| 层 | 归属 | 职责 |
|---|---|---|
| TurnRunner | `assistant-agent/src/kernel/runner.rs` | turn 开始/结束、状态机、双层超时、取消代际、终态落库、事件主干、phase 投影 |
| **RoundExecutor** | trait 在 `assistant-agent/src/agent/round_executor.rs`；实现在 `assistant-agent/src/agent/loop_executor.rs` | 迭代循环：停止钩子 → 阶梯/`before_model` → 模型调用 → 工具执行 → `after_tool` / `after_iteration` → 下一迭代；收尾调用；空响应重试 |
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
| `agent/tool_event.rs` | `assistant-agent` | std / serde + 同 crate 的 `agent/round_executor.rs`（取 `ToolErrorClass` 值类型）；定义 `ToolEvent` / `ToolEventPayload`（阶段枚举）；**不得**依赖 `assistant-tools` |
| `kernel/ports.rs`（新增 `ToolCatalog` trait） | `assistant-agent` | 与 `MessageStore` / `TurnEvent` 同文件；只声明 trait，不 import `assistant-tools` |
| `agent/round_executor.rs`（trait + `RoundOutcome`） | `assistant-agent` | 仅依赖 `kernel/ports` 与 provider 流类型；**不得**依赖 `assistant-tools` |
| `agent/stop.rs` / `agent/middleware.rs` / `agent/ladder.rs`（③④⑤） | `assistant-agent` | 仅依赖 `kernel/ports`（`StopHook` / `TurnLimits`）与 provider 流类型；**不得**依赖 `assistant-tools` |
| RoundExecutor 实现（`LoopRoundExecutor`） | `assistant-agent/src/agent/loop_executor.rs` | `kernel/ports`（`ToolExecutionPort` trait）、provider 流类型；**不得**依赖 `assistant-tools` |
| `ToolExecutionPort` 适配器 + `TimingApprovalPrompter` + 端口装配 | `src-tauri/src/adapters/` + `src-tauri/src/tool_setup.rs` | `assistant-agent` + `assistant-tools`（ToolPipeline / ToolRegistry / ToolGateway） |
| `kernel/request.rs` | `assistant-agent` | 通过 `ToolCatalog` 端口取得 provider 中立工具定义；不直接 import `assistant-tools` |

**理由**：I36 design §4 规定 `kernel` / `agent` → `assistant-tools` 禁止，只依赖抽象。`LoopRoundExecutor` 通过 `ToolExecutionPort` trait 消费工具执行能力，自身无需 `assistant-tools`，故归属 `assistant-agent`；`ToolExecutionPort` 的生产适配器需同时持有 Pipeline 与运行时，放在 `src-tauri` 不破坏 crate 依赖方向。

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
    /// 推理正文，与 `text` 分离（`TurnOutcome.reasoning` 落库口径）。
    pub reasoning: String,
    pub iterations: u32,
    pub model_calls: u32,
    pub tool_calls: u32,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub cache_read_tokens: Option<u32>,
    /// provider 上报 total；未上报时 `None`（与 `TurnOutcome.total_tokens` 同义）。
    pub total_tokens: Option<u32>,
    /// 每个带 usage 的 attempt 一条快照（`kernel::stream::UsageSnapshot`），供终态逐条写 `token_usage`。
    pub attempt_usages: Vec<UsageSnapshot>,
    /// 首个 delta 距请求发起毫秒数；全程无 delta 时 `None`。
    pub ttft_ms: Option<u64>,
    /// 本 turn 审批等待累计（D12）；已计入 turn 墙钟，单独暴露用于诊断
    pub confirmation_wait_ms: u64,
    pub hit_cap: bool,
    pub wrap_up_injected: bool,
    /// 本 turn **最后一条被写入并 `finalize` 的 assistant 行** id（多迭代下每迭代一条，见 §5.1）。
    /// 只有「从未写入任何 assistant 行」时为 `None`（例如 `request::build` 失败 / 首迭代建行前取消）；
    /// 空响应路径在首迭代写占位行、后续迭代沿用上一迭代已写的行，因此均非 `None`（A15）；
    /// chat 单迭代写的是 `TurnRunCtx.initial_message_id` 那条占位行，因此为 `Some`。
    pub final_message_id: Option<String>,
    /// 最后一次请求的 `context_window_tokens`（`kernel::request::ChatRequest` 的投影值）。
    /// 压缩判定仍留在 TurnRunner（§2.1），该值随 outcome 回传，避免 TurnRunner 重算投影。
    pub context_window_tokens: Option<u32>,
    /// 最后一次请求的 `ChatOptions.max_tokens`（`evaluate_compaction` 的 reserved 输入）。
    pub reserved_output_tokens: u32,
    /// 终态错误的具名出口（内核内部口径，**不参与 wire**，供 `TurnRunner` 判定与日志）。
    pub error: Option<TurnError>,
    /// provider 失败时的**原始**错误载荷（`code` / `retryable` / `retry_after_ms` /
    /// `http_status` / `provider_code`），仅供 `TurnRunner` 还原
    /// `TurnEvent::Terminated.error`，**不进 DTO、不参与 wire**（2026-11-06 回写，见 V8）。
    ///
    /// 为什么单列而不是从 `TurnError::Provider(String)` 反推：`TurnError` 只留文案，
    /// 反推会伪造错误码（`AuthFailed` 被当 `RequestRejected`、`RateLimited` 的
    /// `retry_after_ms` 直接丢失），比不传更糟。`assistant-agent` 本就依赖
    /// `assistant-providers`，该字段不破坏 §2.2 的 kernel 端口隔离。
    pub provider_error: Option<ProviderErrorPayload>,
}

pub trait TurnObserver: Send + Sync {
    fn set_phase(&self, phase: TurnPhase);
    fn emit_tool_event(&self, event: ToolEvent);
    /// 正文 / 推理增量：返回本次事件的 `seq`。executor **只能**经这里发 delta，
    /// 不得直接持有 `EventSink`（seq 单调、代际校验、`iteration` 标记都在 `TurnEmitter`）。
    fn delta(&self, delta: &ProviderDelta) -> u32;
}
```

`TurnRunCtx` 由 TurnRunner 构造，字段（伪码中所有 `ctx.*` 的取值来源，未列出的符号不得出现）：

| 字段 | 类型 / 来源 | 用途 |
|---|---|---|
| `turn_id` / `conversation_id` | `&'a str` | 事件上下文、落库外键 |
| `initial_message_id` | `&'a str`（命令层预建占位行） | 第 1 迭代复用的行，`last_row_id` 初值 |
| `start_iteration` | `u32`，P4 恒为 `1` | `for iteration in start_iteration..=max_iterations`，取代 P3 的「起点」含糊说法 |
| `max_iterations` | `u32`（= `ctx.limits.max_iterations`，单列只为伪码可读） | 循环上界 |
| `limits` | `&'a TurnLimits`（`max_iterations` / `max_tool_calls` / `turn_wall_clock: Option<Duration>` / `model_call: Option<Duration>` / `empty_response_retries` / `microcompact_keep_recent`） | 三维预算与单次调用预算 |
| `model` | `&'a str` | `TurnState.model`、`TurnCtx.model` |
| `spec` | `&'a TurnSpec`（`wrap_up` / `ladder` / `stop_hooks` / `limits`；工具注入开关用 `ctx.tools.is_some()`） | 收尾开关、阶梯档位、预算钩子 |
| `input` | `&'a TurnInput`（`TurnRunner` 已取好的 `conversation_detail()` 产物，**不是** `TurnInputSource`） | 第 0 步 `request::build` 的唯一输入 |
| `tools` | `Option<&'a Arc<dyn ToolCatalog>>`（与 `request::build` 第 3 参同型，即 `TurnSpec.tools` 的借用） | `request::build` 的 tools 来源（`None` = chat） |
| `remaining_turn_budget` | `TurnBudget { remaining_tool_calls: u32, turn_deadline: Option<Instant> }` | 裁剪时的剩余额度 / 剩余墙钟 |
| `turn_started_at` | `Instant`（TurnRunner 侧 `started_at` 原值） | `elapsed` 的唯一算法来源 |
| `child_token` | `&'a CancellationToken` | 所有长 await 的取消 |
| `provider_id` / `model_id` | `Option<&'a str>`（与 `CreateAssistantReq` 同型；`model_id` 与 `ctx.model` 同一来源） | `create_assistant_message` 的行元数据 |
| `tool_exec` | `Option<&'a dyn ToolExecutionPort>`（`None` = 无工具执行实现，仅 chat / 测试） | 工具执行端口 |
| `store` | `&'a dyn MessageStore` | 行级落库端口 |
| `clock` | `&'a dyn Clock` | 只读当前时间 |
| `generation` | `&'a dyn TurnGeneration`（`fn is_current(&self) -> bool`） | 落库前的代际校验（§5.1） |
| `observer` | `&'a dyn TurnObserver` | phase / delta / tool 事件的唯一出口 |
| `agent_id` / `parent_turn_id` / `delegation_depth` | 可选，P4 恒为根 agent 值 | §2.4 的委派预留 |

> **`ctx.input` 是 `&TurnInput` 而非端口（2026-11-05 回写）**：`kernel::request::build(input: &TurnInput, iteration: u32, catalog: Option<&Arc<dyn ToolCatalog>>)` 只消费已取好的快照（`kernel/request.rs` 现有签名）。`conversation_detail()` 的取数与失败处理（`NotFound` / `Failed` → 终态）留在 `TurnRunner`（P3 的 `cancellable` 包装也在那一层），因此 `run_iterations` 内**没有**「取历史失败」这条路径；`TurnRunCtx` 不再持有 `TurnInputSource`。

> **`TurnInput.prompt_facts`（新增，2026-11-09）**：环境事实（当前仅 `working_directory`）随输入快照进入内核，**只承载取值、不承载渲染**：`Working directory: …` 这一行的措辞由 `prompt_builder::compose_system_prompt` 单点产出，`request::build` 只在 `AgentKind::supports_tool_call()` 为真时把它并入系统消息（[I38d §4.9.1](./I38d-agent-frontend-modes-spec.md)）。取值由宿主调 `tool_setup::workspace_root_from_conf()` 给出，与工具执行、审批 `extensions` 同源；`assistant-agent` 仍不依赖 `assistant-config`。

> **`ctx` 不含 `provider`（2026-11-05 回写）**：P3 的模型调用是 `ChatRequest::stream(cancel_token)`（`kernel/request.rs` 的 `impl ChatRequest`），adapter / api_key / model / options / `context_window_tokens` 全部收敛在 `ChatRequest` 内，`kernel/runner.rs` 的调用点即样板。executor 拿 `request_with_tools.stream(...)` 直接调、收尾调用拿 `without_tools(request).stream(...)`；ctx 里再放一个 `ChatProvider` 会造出第二条调用路径（且拿不到 `context_window_tokens`）。

> **`TurnGeneration` 端口（新增，2026-11-05）**：§5.1 要求「每次落库前走 `TurnTasks::is_current`」，但 `MessageStore` 不承载代际、`TurnRunCtx` 原先也没有对应字段，实现方无法校验。补一个最小端口（放 `kernel/ports.rs`，与 `Clock` / `CompactionSink` 同层，`assistant-agent` 不感知 `TurnTasks`）：
>
> ```rust
> pub trait TurnGeneration: Send + Sync {
>     /// 本 turn 是否仍是该对话的当前代际；旧代际一律不得落库 / 发事件。
>     fn is_current(&self) -> bool;
> }
> ```
>
> `TurnRunner` 用 `struct TurnGenerationGuard { tasks: Arc<TurnTasks>, conversation_id: String, turn_id: TurnId }` 实现（内部只调 `tasks.is_current(&conversation_id, &turn_id)`，与 `kernel/persist.rs:116` 的 `TurnPersister::is_current` 同判据），turn 开始构造一次随 ctx 下发。executor 在 `persist_row` / `persist_row_as_new` / `append_tool_results` 入口先判 `ctx.generation.is_current()`，为假则 `log::warn!` 并跳过写入（不落库、不发事件、不改 `last_row_id`），**不中断循环**（取消令牌已由 `TurnTasks::begin` 在换代时取消，正常路径会在第 1 步刹车退出）。

> **ctx 不含 `EventSink`（2026-11-05 回写）**：C3 定稿时把 `EventSink` 放进了 ctx，导致正文增量出现两条可写路径 —— 裸 `EventSink` 拿不到 `seq` / `iteration`，`TurnObserver` 背后的 `TurnEmitter` 才有 seq 单调分配与代际校验。P4 收敛为**唯一路径**：delta 走 `TurnObserver::delta`、Tool 事件走 `emit_tool_event`、phase 走 `set_phase`；`EventSink` 从 `TurnRunCtx` 移除（C4 同批删字段，TurnRunner 侧的传参一并删除）。这样 I36 验收 21「同 turn 内 `seq` 严格单调」在多迭代下才成立。

> **`turn_deadline` 有两种类型，不得混用（P3 已定，P4 照抄）**：
> - `TurnRunCtx.remaining_turn_budget.turn_deadline: Option<Instant>` —— **绝对截止时刻**（`started_at + turn 预算`），TurnRunner 算好下发；用于裁剪时算「还剩多少」（`deadline.saturating_duration_since(clock.now())`）。
> - `TurnState.turn_deadline: Option<Duration>` —— **总时长上限**（`limits.turn_wall_clock_ms`），钩子按 `elapsed >= limit` 判定；用于三维预算的墙钟维度。
>
> executor 因此还需要 `turn_started_at`：P4 给 `TurnRunCtx` 增加 `turn_started_at: Instant`（TurnRunner 侧已有 `started_at`，直接传入），`elapsed = clock.now().saturating_duration_since(ctx.turn_started_at)`，`TurnState.turn_deadline = ctx.limits.turn_wall_clock_ms.map(Duration::from_millis)`。审批等待已计入墙钟（D12），两个值都天然含它，不需要再补偿。

> **`observer` 必须在 ctx 内**：executor 是 phase 切换与 `ToolEvent` 的唯一产生者，`TurnObserver` 已定义却不能从 ctx 取得时，只能由实现层自行持有 `EventSink` 并绕开 `TurnObserver`，导致「谁写 phase」出现两个写方。`SessionStates` 仍是唯一写方，`TurnObserver` 只是投影入口。

> **`StopHook` 按 I36 Step 5.1 定型，不在本文重定义**：P3 的 `kernel/ports.rs` 只留了 `name()` 占位，P4 必须补齐 I36 Step 5.1 的**同步**契约 `check(&TurnState<'_>) -> StopDecision`（`Continue` / `Stop { reason }`）与 `TurnState<'a>`（`iteration` / `max_iterations` / `tool_calls` / `max_tool_calls` / `prompt_tokens` / `completion_tokens` / `elapsed` / `turn_deadline` / `model: &'a str`）。**不改成 async**：`check` 同步、禁止 IO 与 await（I36 design §3.5），与 OpenHuman 的 async 版不同，取 goose / design 的同步语义。三维预算所需字段（`iteration` / `tool_calls` / `elapsed` + `turn_deadline`）已在 I36 契约内，不新增字段；`turn_deadline` 由 TurnRunner 算好下发，钩子只读。`Continue` / `Stop { reason }` 与 D19 的 `Nudge` / `Halt` 分开：`StopDecision` 只表达循环是否继续，nudge 属中间件。

> **不携带权限端口**：审批与授权全在 `ToolPipeline` → `ToolGateway` 内部（P2 已有 `ApprovalPrompter` / `PermissionGrantStore` / `PermissionAuditStore`），由 `src-tauri` 装配时注入（D12 的计时装饰器同层）。executor 看不到也不需要任何权限类型，这是 §2.2 依赖方向能守住的前提。

> **`TurnError` 需扩到 P4 的出口集合**：P3 的 `kernel/outcome.rs` 只有 `MiddlewareStop { name, reason }` 与 `Other(String)`，而 P4 的伪码要用到具名变体。P4 新增（保留原两变体，`#[non_exhaustive]` 或直接穷举均可，但 **wire 层不新增字段**，只落在 `RoundOutcome.error` 供 TurnRunner 判定）：`Timeout`（单次调用 / turn 墙钟超时）、`EmptyResponse`（空响应重试耗尽）、`NoProgress { reason }`（无进展阶梯耗尽）、`Persist(String)`（终态文本落库失败）、`Provider(String)`（provider 错误，`String` 而非 provider 类型，守住依赖方向）。`MiddlewareStop` 继续用于 `before_model` 的 `stopped` 出口。**所有变体都不参与 wire**：前端只看 `status` 与 `text`（I36 契约不变）。

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
- 累积器对外只有四个方法：`apply(&UnifiedResponse)` / `finalize(iteration) -> Vec<FinalizedToolCall>` / `reset()` / `has_pending() -> bool`。`reset()` 只清空本次 attempt 的累积状态（D22 空响应重发前调用，避免两次采样叠加），**不重置 `iteration`**、不影响已发射的 `ArgsDelta` 事件（前端按 `call_id` 复用或丢弃）；`has_pending()` 只回答「本 attempt 是否收到过工具调用片段」，供 D22 空响应判据使用（**不**参与落库判断）。

**P4-C1 的必做清理（现状核对 2026-11-05）**：`assistant-agent/src/kernel/stream.rs` 的 `StreamAccumulator` 里已有第二份工具片段累积（`tool_calls: Vec<ToolCallAccum>` + `apply_tool_call`，含 id 匹配与追加，`StreamOutcome.tool_calls` / `PartialOutcome.tool_calls` 对外暴露）。这不是「已剥离」，C1 必须一并处理：

- `StreamAccumulator` 只保留 `full_content` / `full_reasoning` / usage / `ttft_ms` / `finish_reason` / `has_effective_output`，**删除 `tool_calls` 字段与 `apply_tool_call`**，`StreamOutcome` / `PartialOutcome` 同步删除 `tool_calls`。
- 工具片段改由 `stream::consume` 的 `on_delta` 回调原样交给调用方，由 executor 持有的 `agent/tool_call.rs` 累积器消费（`consume` 的职责变成「正文/推理/usage/seq 累积 + 逐条回调」）。
- 连带删除：`kernel/runner.rs` 的 P3 过渡循环里 `stream_outcome.tool_calls` 的三处引用随 C5 一起消失；C1 与 C5 之间该循环暂不可编译，因此**C1 与 C5 必须同批交付**（任务清单已据此标注）。
- 验收 A7 的 grep 口径扩为「`assistant-providers` 无累积器 **且** `assistant-agent/src/kernel/stream.rs` 无 `ToolCallAccum`」。

### 3.2 D1-D10 决策表（P4 开工前定稿）

| # | 决策点 | 结论 | 说明 |
|---|---|---|---|
| D1 | 流式键策略 | `ToolCallStreamKey::{Indexed(usize), Unindexed}` | 保留 providers 现状；Anthropic 若提供 index 则用 Indexed，否则回落 Unindexed |
| D2 | 边界枚举 | 保留 `ToolCallBoundary::{NewTool, FinishReason, StreamEnd}` | `NewTool` 用于流式提前终结上一个工具；删除会让 `finalized_previous` 语义失效 |
| D3 | 截断语义 | 先 `repair_truncated_json`，修复可用则降级（`is_error=false` + 修复提示）；修复失败再报 `Truncated` | 两者并存，不二选一 |
| D4 | 入口形态 | 保留 `apply_unified_response`，底层 `apply_delta` 同时存在 | agentloop 的输入天然是 `UnifiedResponse` |
| D5 | 事件字段 | 上下文结构体 `ToolEvent { turn_id, iteration, call_id, name, payload }` + 阶段枚举 `ToolEventPayload::{Started, ArgsDelta{delta}, Finished{result, confirmation_wait_ms?}, Failed{error, error_kind, confirmation_wait_ms?}}`；wire 侧 `AgentToolCallDto` + `AgentToolCallPayloadDto`（`#[serde(tag = "phase")]`），完整定义见 §6.1 / §6.1.1 | 供 P6 五态渲染；阶段用枚举变体而非 `phase` 字段 + `Option` 堆叠，与 OpenBitFun `ToolEventData` / Codex `EventMsg` / OpenHuman `AgentProgress` 一致；`confirmation_wait_ms` 按 D12；`error_kind` 是 `ToolErrorClass` 镜像，wire 侧复用 `ToolErrorKindDto` |
| D6 | 模块落点 | `assistant-agent/src/agent/tool_call.rs` | 见 §2.2 依赖方向 |
| D7 | 测试口径 | 跨 chunk 累积 / 快照模式 / 双键 / 边界 / 截断 / 非法 JSON 六类 | 与实现同批交付 |
| D8 | Gemini 累积上移 | **上移**：provider 只透传快照片段并标 `arguments_is_snapshot = true`；累积统一由 `agent/tool_call.rs` 完成 | 根治 F15/F16 |
| D9 | 快照标记承载 | `ChatDelta` 新增 `delta_tool_call_arguments_is_snapshot: bool`（provider 内部流类型，非 wire） | 不新增独立流出口，避免第三套格式 |
| D10 | tools 注入 | P4 统一新增 `ChatOptions.tools`，三 adapter 同批实现 | Step 7 真正发起工具调用的前置 |

> D1/D2/D3 的取值以 `assistant-providers` 现实现为基线；D8 上移后 provider 回归无状态。
> D11–D27（落库形态、审批计时、压缩切换、存储形态、真实工具集、`ToolCatalog` 端口 async 性、落库判据、三维预算、重复失败阶梯、工具执行策略、结果体积、空响应重试、收尾还原、压缩计数、失败分类预算、重复调用豁免、投影期配对修复）见 §9.1。

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
- 与 `assistant_tools::ToolDefinition` **字段完全同形**（`name` / `description` / `input_schema`，均 `camelCase`），因此转换是纯字段搬运，不加业务逻辑。
- `ToolDefinition`（`assistant-tools`）→ `ToolSpec` 的转换在 `src-tauri` 适配层完成，经 `ToolCatalog` 端口进入 `kernel/request.rs`。

**`ToolCatalog` 端口**（`assistant-agent/src/kernel/ports.rs`，与 `MessageStore` / `TurnEvent` 同文件，async，D16）：

> **现状核对（2026-11-05）**：P3 已在 `kernel/mod.rs` 留了同步占位 `pub trait ToolCatalog: Send + Sync {}` 与 `EmptyToolCatalog`，`TurnSpec.tools: Option<Arc<dyn ToolCatalog>>` 指向的就是它。C2 **不是新增**，而是：① 在 `ports.rs` 声明下面的 async trait；② 删除 `mod.rs` 的同步占位与 `EmptyToolCatalog`（连带删掉其导出）；③ 把 `TurnSpec.tools` 与 `runner.rs` 的 `spec.tools.is_none()` 判定改到新签名。全仓不得留两套 `ToolCatalog`。

```rust
#[async_trait]
pub trait ToolCatalog: Send + Sync {
    /// P4 只返回全集（audience 过滤由适配器调 `tool_definitions_for_tier` 完成）
    async fn agent_tools(&self) -> Vec<ToolSpec>;
}
```

- 实现放 `src-tauri`：内部调 `ToolRegistry::tool_definitions_for_tier(max_tier)`（`registry.rs:35`，已异步且已按 `audience().agent` 过滤、按名排序），逐条转 `ToolSpec`。
- 返回空 `Vec` 合法：此时 `ChatOptions.tools = None`，循环退化为「单次调用 + 无工具」，chat 等价行为不变（A9）。
- P8 按 agent 收窄时给该方法加作用域参数即可，调用点不变（D16 的理由）。

### 4.2 ChatOptions

落点：`assistant-providers/src/types/config.rs:5`（现有 `temperature` / `max_tokens` / `stream` / `context_window_tokens` / `reasoning_*`）。

```rust
pub struct ChatOptions {
    // ...现有字段...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolSpec>>,
}
```

- `None` / 空数组 = 不声明工具（chat 路径行为不变）。
- **注入开关以 `TurnSpec.tools` 为准**：`TurnSpec::chat()` 恒为 `None`，`TurnSpec::agent()` 为 `Some(catalog)`。`AgentKind::supports_tool_call()`（P6 起改为「`kind = agent` 为真」，旧值 `custom` 等一律为假，见 I38d §4.11）只用于 `request.rs` 的二次门控，**不作为链路判据**；链路由前端按会话 `modeId` 选路（chat 会话 / agent 会话，见 I38d §4.8）。这样「chat 路径行为不变」（A9）由 `TurnSpec` 保证，不依赖注册表是否为空。
- **赋值点**：`build_chat_options`（`chat_options.rs:39`）新增 `tools: Option<Vec<ToolSpec>>` 参数并在返回结构里填入；调用点从 `chat_request.rs` 迁到 `kernel/request.rs` 时，agent 链路（`TurnSpec::agent()`）传 `Some(...)`、chat 链路（`TurnSpec::chat()`）传 `None`。不新增第二个构造函数，避免两套 options 组装逻辑。
- **`ChatOptions` 是内部结构，不是 wire DTO**（`cmd/compaction.rs:137` 与 `chat_options.rs:54` 用完整字面量构造），加字段会让这两处编译失败：`chat_options.rs` 随本次改动同步；`cmd/compaction.rs` 的压缩摘要调用显式写 `tools: None`（摘要不带工具），**不得**改成 `..Default::default()` 掩盖语义。
- 现有 `ChatMessage.tool_calls` / `tool_call_id` 已存在（F12），本阶段开始被填充。

### 4.3 三 adapter 注入

| Provider | wire 形状 | 注意 |
|---|---|---|
| OpenAI | `tools: [{ type: "function", function: { name, description, parameters } }]` | `parameters` 直接使用 `input_schema`；无现成转换函数，P4 在 `openai/message_converter.rs` 新增 |
| Anthropic | `tools: [{ name, description, input_schema }]` | 不支持 `parameters` 包壳；无现成转换函数，P4 在 `anthropic/message_converter.rs` 新增 |
| Gemini | `tools: [{ functionDeclarations: [{ name, description, parameters }] }]` | 需 schema 方言转换；复用 `gemini/message_converter.rs:229` 的 `convert_tools`，签名从 `&[(String, String, String)]` 改为 `&[ToolSpec]`，并去掉该函数及 `sanitize_schema` / `strip_unsupported_schema_fields` 上的 `#[allow(dead_code)]`（转正） |

**约束**：转换只在 adapter 内进行；`ToolSpec` 不得直接序列化成任一 provider 的 wire 形状。

**请求结构体现状（决定改动落点）**：

| Provider | 请求 struct | 现状 | P4 改动 |
|---|---|---|---|
| OpenAI | `openai/types.rs:5` `ChatRequest` | **无 `tools` 字段** | 加 `#[serde(skip_serializing_if = "Option::is_none")] pub tools: Option<Vec<Value>>`；在 `openai/chat.rs:41` 构造点填值 |
| Anthropic | `anthropic/types.rs:4` `MessagesRequest` | **无 `tools` 字段** | 同上；构造点 `anthropic/request.rs:44` |
| Gemini | `gemini/types.rs:6` `GenerateContentRequest` | **已有 `pub tools: Option<Vec<Value>>`**（`types.rs:13`），但两处构造点（`gemini/request.rs:41`、`gemini/code_assist.rs:52`）都硬编码 `None` | 只改构造点：`request.rs` 填 `convert_tools(...)`；`code_assist.rs` 保持 `None`（该路径不注入工具，P4 不改 Code Assist 行为） |

- 三处都用 `Value` 而非强类型：adapter 内部转换，不把 provider 方言泄漏到 `assistant-providers` 的公共类型（`ToolSpec` 只在 `ChatOptions` 层）。

**回填消息的方言差异（必须处理，否则请求被 provider 拒绝）**：`ToolCall.arguments` 是 **JSON 文本**（`assistant-providers/src/types/tool.rs`），三个 adapter 对它的期望不同：

| Provider | 现状 | P4 必须做的 |
|---|---|---|
| OpenAI | `"arguments": tc.arguments`（字符串，正确） | 不改 |
| Anthropic | `"input": tc.arguments`（`message_converter.rs:152`）—— **错**，Anthropic 要求 `input` 是对象 | 解析成 `serde_json::Value`；解析失败回退 `{}`（正常路径已由 D3 的 `repair_truncated_json` 兜底） |
| Gemini | `"args": tc.arguments`（`message_converter.rs:147`）—— **错**，Gemini 要求 `args` 是对象 | 同 Anthropic：解析成 `Value`，失败回退 `{}` |

**Gemini 的 tool 结果还要工具名**：`convert_tool_result_parts`（`message_converter.rs:168`）在 `msg.name` 为空时直接返回空 parts，导致 `functionResponse` 缺失、请求被拒。因此 `role='tool'` 的 `ChatMessage.name` **必须**填工具名 —— 由 `prompt_builder` 从父 assistant 行的 `tool_calls_json` 按 `tool_call_id` 反查（不新增列，见 §6.3）。OpenAI / Anthropic 的 `convert_messages` 不读 `name`，填了不产生任何 wire 字段，也无副作用。

> 这三处是 P4 的**必须修正项**，不是可选优化：`convert_tools` 是标了 `#[allow(dead_code)]` 的死代码，tool 回填分支虽有 `match` 但从未被真实数据触发（历史里没有 `role='tool'` 行），所以缺陷一直没暴露；一旦 P4 打开 `tools` 通道，第一个带工具历史的重放请求就会 400。

### 4.4 回填消息形状

```
assistant message: { role: "assistant", content: String（纯工具调用时为空串）, tool_calls: Vec<ToolCall> }
tool message:      { role: "tool", tool_call_id: <id>, name: <tool_name>, content: <result_for_assistant>, is_error: bool }
```

> **`name` 必须填**：Gemini 的 `functionResponse` 只认工具名（`convert_tool_result_parts` 在 `name` 为空时返回空 parts）；OpenAI / Anthropic 不读该字段。值由 `prompt_builder` 从父 assistant 行的 `tool_calls_json` 按 `tool_call_id` 反查，不落新列（§6.3）。

> **`content` 是 `String`，不是 `Option<String>`**（`assistant-providers/src/types/message.rs` 现状）。空串由 adapter 各自映射成 wire 的空值：OpenAI `message_converter.rs` 已写 `content: Null`；Anthropic 空串不产出 text block；Gemini 空串且有 tool_calls 时只发 `functionCall` part。**不改 `ChatMessage.content` 类型**，否则三 adapter 与全部历史消息都要动。

- 一个 tool_call 对应一条 tool message；顺序与 `tool_calls` 一致。
- `result_for_assistant` 缺省时用 `data` 的紧凑序列化；**tool 行 content 不得为空串**（Anthropic 对空 content 的 `tool_result` 有 `"Tool execution failed"` 兜底，会掩盖真实失败原因）。
- `error_kind = Cancelled` 时 content 写「已取消」，不当作失败重试。
- 落库与 wire 同形：assistant 行 → `tool_calls_json`，tool message → `role='tool'` 行（`tool_call_id` + `content` + `is_error`），详见 §6.2；`prompt_builder` 从这两类行重建上述形状。
- **一条 tool_call 一条 tool message，不在 `prompt_builder` 里合并**：Anthropic 要求 `tool_result` 位于 user 消息内，`anthropic/message_converter.rs::merge_consecutive_messages` 已负责把连续 user 消息合并为多 block 消息；Gemini 由 `convert_tool_result_parts` 逐条转 part。合并是 adapter 职责，落库形态保持一 call 一行。
- **重放时允许出现「合成结果」**：历史行链若因跨进程中断缺了某条 tool 行，`prompt_builder` 按 D27 在投影期合成一条 `is_error = true`、`content = "未执行：本轮中断"` 的 tool message（不写库），使 wire 仍然配平；正常路径的行链不会出现合成行，A11 的往返断言以正常路径为准。

### 4.5 工具执行（D20 / D21）

```rust
// assistant-agent/src/agent/ports.rs —— 端口，不是 assistant-tools 的 ToolPipeline 结构体
#[async_trait]
pub trait ToolExecutionPort: Send + Sync {
    async fn execute(&self, calls: Vec<FinalizedToolCall>, ctx: &ToolExecCtx<'_>) -> Vec<ToolExecResult>;
}
```

> **命名决策**：端口叫 `ToolExecutionPort`，**不**叫 `ToolPipeline` —— 后者已是 `assistant-tools::pipeline::ToolPipeline` 的具名结构体（`find → validate → gateway → execute → persist`）。端口与实现同名会让「哪一层负责分批/落盘」读不清；`src-tauri` 的适配器持有 `Arc<assistant_tools::ToolPipeline>` 并实现本端口。

`ToolExecCtx` 携带 `turn_id` / `conversation_id` / `iteration` / `child_token` / `ToolCatalog` / `Clock` / `TurnObserver`。**不含权限端口**：审批在 `ToolPipeline` → `ToolGateway` 内完成，适配器只在装配时把 `ApprovalPrompter`（含 §6.4 的计时装饰器）交给 gateway。

> **`ToolExecCtx` 含 `observer`（2026-11-06 回写，修正上一段早期的「不含 observer」口径）**：§6.1 要求 `Started` / `Finished` / `Failed` 由 `src-tauri` 的 `PipelineEventSink` 适配器「补 `turn_id` / `iteration` / `confirmation_wait_ms` 后交给 `TurnObserver::emit_tool_event`」，而该 sink 只能在每次 `execute` 时按本次 turn 的 `turn_id` / `iteration` 现构造 —— 这两个值只存在于 ctx，`TurnObserver` 亦只能从 ctx 取得。若 ctx 不带 observer，适配器只能自行持有 `EventSink`，反而出现第二条事件通道（正是 §6.1「不得双发」要禁止的）。因此 `ToolExecCtx.observer` 是**只读转发口**：适配器只把 Pipeline 的三种终态事件转成 `ToolEvent` 交给它，**不得**在此发 `ArgsDelta`（`ArgsDelta` 属 executor 的累积器）。


**职责切分（关键，避免重复实现）**：`find → validate → gateway → execute → persist` 与**分批并行、结果落盘、结果事件**全部在 P2 的 `ToolPipeline::execute` 内部完成（`pipeline.rs:176` 起：`partition_tool_batches` → 并行/串行 → `select_persist_targets` → `persist` → `report`），并保证返回的 `Vec<ToolOutcome>` **与入参顺序一致**。executor 侧的 `ToolExecutionPort` 适配器只做三件事：类型转换、`confirmation_wait_ms` 注入、`FinalizedToolCall` → `ToolCall` 的字段搬运。**executor 不得再调 `partition_tool_batches` / `select_persist_targets`**，否则会出现二次分批与二次落盘。

因此执行语义由 P2 固定，P4 只需按下列行为对齐：

1. **解析**：`find` 未命中 → Pipeline 返回 `stage = Find`、`is_error = true`，content 为 `unknown tool 'X'`，回灌模型自纠，不中断 turn。
2. **校验**：`validate_input` 失败 → `stage = Validate`、`is_error = true`，content 带校验错误与期望形状，同样回灌自纠。
3. **审批**：调用前走 P2 的 `ToolGateway`（`ApprovalPrompter` 在装配时注入，含 §6.4 的计时装饰器）；等待计入 turn 墙钟并单记 `confirmation_wait_ms`（D12）；超时即 `Deny`。
4. **分批**：`partition_tool_batches` 按 `is_concurrency_safe` 合批 —— 连续安全调用并行，其余单独成批串行；未注册按不安全（fail-closed）。executor 只消费有序结果。
5. **收口**：`select_persist_targets` 是**单轮级**决策（需先拿到全部结果按大小降序判断），Pipeline 在全部批次结束后统一执行，并把超阈值结果改写为 `persisted_reference`（引用 + 预览）；executor 拿到的 `content` 已是回填文本，落库与回填同一份文本（D21）。
6. **隔离**：单个工具失败/超时只影响自身结果（`is_error = true`），不取消同批其他工具；turn 取消时整批返回 `Cancelled`，按 §4.4 写「已取消」。
7. **事件**：`Started` / `Finished` / `Failed` 由 Pipeline 经 `PipelineEventSink` 发出（§6.1）；`ArgsDelta` 由 executor 的累积器发出。

**硬约束**：`ToolExecutionPort` / `ToolExecCtx` / `ToolExecResult` / `ToolStage` / `ToolErrorClass` 全部声明在 `assistant-agent/src/agent/round_executor.rs`（与 `RoundExecutor` trait、`TurnObserver` 同文件，避免模块间循环引用），实现放 `src-tauri`；trait 与 ctx 不得出现 `tauri` / `AppHandle` / `assistant_tools` 类型（§2.2）。`ToolExecResult` 是 `assistant-agent` 的本地值类型，由适配器从 `assistant_tools::ToolOutcome` 转换，字段映射如下（全部可在现有类型上直接取到，不新增 P2 字段）：

| `ToolExecResult` 字段 | 来源（`assistant_tools::ToolOutcome`） |
|---|---|
| `call_id` | `tool_call_id` |
| `tool_name` | `tool_name` |
| `content` | `assistant_text`（**不是** `result.result_for_assistant`；落盘后这里已是「引用 + 预览」） |
| `is_error` | `!result.ok` |
| `stage` | `stage`（`Find/Validate/Gateway/Execute/Persist` → 本地 `ToolStage`） |
| `error_kind` | `result.error_kind`（→ 本地 `ToolErrorClass`） |
| `confirmation_wait_ms` | §6.4 装饰器按 `tool_call_id` 提供（`ToolOutcome` 无此字段） |

`ToolStage` 与 `ToolErrorClass` 是本地镜像枚举（`Find/Validate/Gateway/Execute/Persist` 与 `Business/Timeout/Cancelled/Internal/Denied`），端口不泄漏 `ToolOutcome` / `ToolErrorKind`；镜像由 `contract` 侧测试锁定一一对应，`assistant-tools` 新增变体时该测试失败，避免静默漂移。

> `ToolOutcome` 里还有 `blocks_input_rewrite`（校验阶段的「不可改写绕过」标记）。P4 **不在 executor 里使用**它：D25 的分类只看 `(stage, error_kind)`，绕过语义仍由 Pipeline 内部负责（`blocks_input_rewrite` 决定是否允许模型改输入重试，属 P2 已实现行为）。

**适配器构造参数的来源（2026-11-05 回写）**：`ToolPipeline::execute` 必须同时拿到 `&ToolUseContext` 与 `&ResolvedPermissionPolicy`，两者都由适配器在每次 `execute` 时现构造，**不放进 `ToolExecCtx`**（它们是 `assistant-tools` 类型，进 ctx 会破坏 §2.2 的依赖方向）：

| 参数 | P4 取值 | 来源 |
|---|---|---|
| `ToolUseContext.workspace_root` | 会话绑定的工作区；无绑定则 `None` | `src-tauri` 从会话行 / 全局配置读取，P4 只读不写 |
| `ToolUseContext.session_id` | `Some(conversation_id)` | `TurnRunCtx.conversation_id` |
| `ToolUseContext.tool_call_id` | `Some(FinalizedToolCall.id)` | 每次调用现填（逐调用构造，不共享一份） |
| `ResolvedPermissionPolicy` | `ResolvedPermissionPolicy::default_global()` | 模块常量；配置化随 P6（与 `TurnLimits` 同批） |
| `ToolResultStorage` 的 `root` | `app_data_dir()/tool-results/` | `tool_setup.rs` 构造一次并注入适配器；**不得用临时目录** —— 该路径会写进 `messages.content` 的 `<persisted-output path="…">`，A11 的跨进程重放一致性依赖它稳定 |

`default_global()` 判为 `Ask` 的调用会进入 §6.4 的 fail-closed 审批默认（立即 `Deny` + 审计），因此 P4 的工具有效集合是「策略判 `Allow` 的只读类工具」+「测试替身工具」；这不影响 A14 的分批断言与 A11 的往返断言。

---

## 5 循环流程

> 循环伪码全文、符号表、辅助函数表、双 Halt 出口定义、预算内裁剪逻辑已提取至 [I38b 实施指南 §附录A](./I38b-agent-loop-impl-guide.md#附录-a循环伪码来自主规格-5完整定义)。本文保留精简摘要，编码以指南附录为准。

**核心流程**：① `request::build` 一次性投影历史 → ② 迭代循环（三维刹车 → 模型调用（内层空响应重试）→ 落库 → 无工具则结束 → 最后一轮收尾/裁剪 → 工具执行 → after_tool/after_iteration 中间件 → nudge 传递）→ ③ 循环结束兜底。

**双 Halt 出口**：`halted_by_budget()`（三维预算耗尽，Completed + hit_cap，wrap_up=true 时走收尾调用）→ `halted_with_summary()`（无进展阶梯耗尽，Failed + 根因摘要）。

**行级落库**：每迭代一行 assistant，工具结果独立 tool 行。辅助函数 `ensure_row`（幂等建行）/ `persist_row`（内容+usage+finalize）/ `persist_row_as_new`（无条件建新行）/ `persist_final_text`（择行）/ `append_tool_results`（内存+DB）为唯二写路径，禁止出现第三条写库路径。

**预算内裁剪**：按原始顺序取剩余额度内的调用执行，超出部分写 `is_error = true` 配对结果保持 wire 合法，skipped 不进失败统计。三维预算独立判定，任一触发即 halt。

**硬约束**：空响应重试用内层 loop（外层 for 的 continue 会递增 iteration）；pending_nudges 只进本次请求不落库（D19）；收尾调用不计入迭代名额但计入 model_calls；persist_final_text 失败不吞 DB 错误。

---











## 6 事件与落库

### 6.1 事件

| 事件 | 时机 | 载荷 |
|---|---|---|
| `agent-tool-call` | 工具开始 / 参数增量 / 完成 / 失败 | 上下文（`conversationId` / `turnId` / `iteration` / `callId` / `name`）+ 嵌套阶段载荷（§6.1.1） |
| `chat-delta` | 文本 / 推理增量 | 追加 `iteration`（P3 已加） |
| `session-state-changed` | phase 变化 | 由 TurnObserver → TurnRunner → EventSink 发射 |

**事件单一来源（避免双发）**：

| 阶段 | 产生方 | 说明 |
|---|---|---|
| `ArgsDelta` | executor 的累积器（`agent/tool_call.rs`） | 流式阶段产生，此时工具尚未执行，与 Pipeline 无关 |
| `Started` / `Finished` / `Failed` | `src-tauri` 的 `PipelineEventSink` 适配器 | P2 的 `PipelineEvent` 已覆盖三种终态；适配器转成 `ToolEvent` 并补 `turn_id` / `iteration` / `confirmation_wait_ms` 后交给 `TurnObserver::emit_tool_event` |

- **不得**在 executor 里再发一遍 Started/Finished/Failed：Pipeline 已发，重复会污染前端五态卡片。
- `confirmation_wait_ms` 由 `src-tauri` 的计时装饰器产生（见 §6.4），Pipeline 本身不带该字段。
- **取消归入 `Failed`**：P2 对取消路径返回 `is_error = true` + `error_kind = Cancelled`，因此不单设 `Cancelled` 阶段（wire 侧 `errorKind = "cancelled"` 已能区分），避免多一个阶段却没有独立的渲染分支。

**内核事件形状（阶段用枚举变体，不用 `phase` 字段 + 一堆 `Option`）**

三家参考项目一致采用「变体承载阶段、变体只带该阶段有意义的字段」：

| 项目 | 出处 | 形状 |
|---|---|---|
| OpenBitFun | `references/1-ai-agent/OpenBitFun/src/crates/contracts/events/src/agentic.rs:367` | `AgenticEvent::ToolEvent { session_id, turn_id, round_id, ..., tool_event: ToolEventData }`，`ToolEventData`（同文件 `:502`，`#[serde(tag = "event_type")]`）按 `ParamsPartial` / `Started` / `Completed` / `Failed` / `Cancelled` / `Queued` / `Waiting` / `Progress` / `ConfirmationNeeded` / `Confirmed` / `Rejected` 分变体；共享标识用 `ToolEventIdentity`（`:461`）以 `#[serde(flatten)]` 复用 |
| Codex | `references/1-ai-agent/codex/codex-rs/protocol/src/protocol.rs:1663` | `EventMsg::ItemStarted(ItemStartedEvent)` / `ItemCompleted(ItemCompletedEvent)`，阶段由变体名承担 |
| OpenHuman | `references/1-ai-agent/openhuman/crates/openhuman-core/src/agent/progress.rs:18` | `AgentProgress::ToolCallStarted { .. }` / `ToolCallCompleted { .. }` / `ToolCallArgsDelta { .. }`，变体直接带字段 |

因此 P4 的 `ToolEvent` 定为：

```rust
// assistant-agent/src/agent/tool_event.rs
pub struct ToolEvent {
    pub turn_id: String,
    pub iteration: u32,
    pub call_id: String,
    pub name: String,
    pub payload: ToolEventPayload,
}

pub enum ToolEventPayload {
    Started,
    ArgsDelta { delta: String },
    Finished {
        result: String,                        // 回填文本（已按 D21 落盘改写）
        confirmation_wait_ms: Option<u64>,
    },
    Failed {
        error: String,
        error_kind: ToolErrorClass,            // §4.5 的本地镜像
        confirmation_wait_ms: Option<u64>,
    },
}
```

- `TurnEvent` 侧：`kernel::ports::TurnEvent` 现有的 `ToolStarted` / `ToolFinished` 两个变体收敛为单一 `TurnEvent::Tool(ToolEvent)`。**现状核对（2026-11-05）**：P3 已完成，`kernel/emit.rs:96/104` 已有 `TurnEmitter::tool_started` / `tool_finished` 两个发射方法，但**无调用方**（生产者为空，不是方法不存在）。因此 C3 的动作是：① 删掉这两个 `TurnEmitter` 方法，改为单一 `tool_event(&self, event: ToolEvent)`；② `event_sink.rs` 的 `ToolStarted` / `ToolFinished` 两个 `match` 分支（只打 debug 日志）合并为 `TurnEvent::Tool(_)` 分支并接 C6 的线事件映射；③ 替换后全仓 grep 不得再有这两个变体名与这两个方法名。除这三处外无其他调用点，替换不产生迁移债。
- `error_kind` 用本地镜像 `ToolErrorClass`（§4.5），**不**在事件里泄漏 `assistant_tools::ToolErrorKind`：P2 的 `PipelineEvent::Failed` 已带 `error_kind`，适配器必须把它搬过来，否则 P6 只能靠文本猜错误类别。`ToolErrorClass` 由 §4.5 的 contract 测试锁定与 `ToolErrorKind` 一一对应，新增变体会让该测试失败。
- `TurnRunner` 把 `TurnEvent::Tool(e)` 映射为线事件 `agent-tool-call`，字段 camelCase（`confirmationWaitMs` 等），映射逻辑在 `src-tauri/src/adapters/event_sink.rs`；映射只做字段搬运（枚举变体 → 枚举变体），不做语义重组。
- **扩展方式固定**：P6 新增审批阶段（`ConfirmationNeeded` / `Confirmed` / `Rejected`）时，加 `ToolEventPayload` 变体与对应 wire 变体，**不**在既有变体上加 `Option` 字段。这与 OpenBitFun 加六个阶段时的方式一致。
- **`Started` 不带参数字段**：参数在流式阶段已由 `ArgsDelta` 逐片发出，`Started` 只承担「开始」语义；OpenBitFun 的 `Started` 带 `params` 是因为它的参数可能一次性到达，我们的累积器统一处理增量，不在 `Started` 重复携带完整参数（否则前端要处理「先有全量再有增量」两种来源）。

### 6.1.1 wire DTO 落点与契约改动（D5 的落地）

**落点：新增 `src-tauri/src/protocol/dto/agent.rs`**，理由：

1. `chat.rs` 承载文本流（`ChatDeltaPayloadDto` / `MessageDto`），工具生命周期是另一条事件族；OpenBitFun 有独立 `agentic.rs`、OpenHuman 有独立 `progress.rs`，两家都不把 tool 事件混进 chat 模块。
2. `os_tool.rs` 承载**同步命令**的 request/result（`invoke` 直调，`OsToolExecRequestDto` / `OsToolResultDto`），`agent-tool-call` 是**异步事件**。混在一起会让「用户直调工具」与「agent 循环内工具」两条路径的 DTO 语义混淆，且字段集本就不同（`iteration` / `confirmationWaitMs` 只属后者）。

```rust
// src-tauri/src/protocol/dto/agent.rs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentToolCallDto {
    pub conversation_id: String,
    pub turn_id: String,
    pub iteration: u32,
    pub call_id: String,
    pub name: String,
    pub payload: AgentToolCallPayloadDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub enum AgentToolCallPayloadDto {
    Started,
    ArgsDelta {
        delta: String,
    },
    Finished {
        result: String,
        #[serde(rename = "confirmationWaitMs")]
        confirmation_wait_ms: Option<u64>,
    },
    Failed {
        error: String,
        #[serde(rename = "errorKind")]
        error_kind: ToolErrorKindDto,
        #[serde(rename = "confirmationWaitMs")]
        confirmation_wait_ms: Option<u64>,
    },
}
```

- **多词字段用显式 `#[serde(rename = ...)]`，不靠 `rename_all_fields`**：`rename_all` 在 enum 上只作用于变体名，不作用于变体内字段；`rename_all_fields` 在既有 DTO 中无先例（`ContentBlockDto` 的多词字段也是显式 `#[serde(rename = "tool_use_id")]`），且 ts-rs 12 对该属性的处理未经验证。显式 rename 由 serde 与 ts-rs 双方保证。
- **复用 `ToolErrorKindDto`**（`dto/os_tool.rs:19`），不新建第三个错误类别枚举 —— 已有 `assistant_tools::ToolErrorKind` → `ToolErrorKindDto` 的 `From` 实现，agent 事件跨模块 use 即可。
- **不引入 `payload: Value`**：OpenBitFun 的 `FrontendEventNotification` 用弱类型是因为它跨多宿主（Web / RPC / Tauri），本仓只有 Tauri 一个宿主，强类型能换来前端 `switch (payload.phase)` 的完整收窄。
- **tagged enum 上不加 `deny_unknown_fields`**（与既有 `ContentBlockDto` 一致，enum 上该属性不适用）；`AgentToolCallDto` 外层保留 `deny_unknown_fields`。
- **`Option` 字段不加 `skip_serializing_if`**：按项目 DTO 契约，输出 DTO 的键恒在、值为 `null`，前端声明 `T | null` 为必填。
- ts-rs 12 已能生成 discriminated union，既有 `ContentBlockDto`（`protocol.contract.ts:37`）就是先例，无需改生成机制。

**契约改动清单（机械项，缺一即测试失败）**：

| 文件 | 改动 |
|---|---|
| `src-tauri/src/protocol/dto/agent.rs` | 新建，含上述两个类型 |
| `src-tauri/src/protocol/dto/mod.rs` | 加 `pub mod agent;` 与 `pub use agent::*;`（现有 8 个模块，新增为第 9 个） |
| `src-tauri/src/protocol/events.rs` | 加 `pub const AGENT_TOOL_CALL: &str = "agent-tool-call";`（现有常量按域分组，插在 chat 组之后） |
| `src-tauri/tests/protocol_bindings.rs` | `use` 加两类型；`declarations` 数组加两条。**无计数魔数需改**：原 `assert_eq!(output.matches("export type ").count(), 63, ...)` 已于 2026-11-05 替换为「`src/protocol/dto` 中 `#[derive(...TS)]` 类型集合 ↔ `declarations` 类型集合」双向差集断言，漏注册会直接报出类型名 |
| `src-tauri/tests/protocol_dto_roundtrip.rs` | 加 `agent_tool_call_dto_roundtrip`，覆盖四个变体各一条（`Finished` / `Failed` 的 `confirmation_wait_ms` 分别取 `Some` 与 `None`） |
| `src/types/protocol.contract.ts` | 由 `npm run contract:gen` 生成，**不手改** |
| `src-tauri/src/adapters/event_sink.rs` | `TurnEvent::Tool(e)` → `AgentToolCallDto` 映射并 `emit(AGENT_TOOL_CALL, ...)` |

- 本阶段**不动 `MessageDto`**：其 `deny_unknown_fields` 决定前端契约，工具卡片渲染所需字段（如 `toolCalls` / `toolCallId`）归 P6 一起扩，避免 P4 改了前端类型却没人消费。
- `MessageRow` 是内部读模型（§6.3），扩展它不触发 `contract:gen`，与本节无关。

### 6.2 落库（D11）

参照成熟项目的既定做法，结论是：**tool 调用与 tool 结果都落库，但都不建独立 tool 表**。

| 项目 | 会话载体 | tool call | tool 结果 | 失败标记 |
|---|---|---|---|---|
| Codex | SQLite `thread_items.item_json` | item JSON 内 | 独立 item 行（`FunctionCallOutput`） | `thread_turns.status` |
| OpenHuman | JSONL 每行一条消息 | assistant 行 `tool_calls[]` | 独立 `role:"tool"` 行 | 行级 `failure: true` |
| OpenBitFun | 每轮 JSON 文件 | item 内 `tool_call` | item 内 `tool_result` | `success` + `error` |

出处：`references/1-ai-agent/codex/codex-rs/thread-store/src/local/thread_history.rs`（`INSERT INTO thread_items (..., item_json)`）、`references/1-ai-agent/openhuman/tests/fixtures/session_compat/native_tools.jsonl`（assistant 行 `tool_calls` + 独立 `role:"tool"` 行）、`references/1-ai-agent/OpenBitFun/src/crates/services/services-core/src/session/types.rs`（`ToolItemData` / `ToolResultData`）。

三家都把结果落库、都不建 tool 表，差别只在「结果是一条独立行」还是「item 内的一个字段」。本仓 `messages` 已是「一行一条消息」，取 OpenHuman 形态最贴合：

- **assistant 行**：`tool_calls_json` 复用预留列（I30 §5.3），写入 `[{ id, name, arguments }]`，`arguments` 保留原始 JSON 文本。
- **纯工具调用的 assistant 行 `content` 写空串 `''`**：`messages.content` 是 `TEXT NOT NULL`（db.rs:50），不得写 `NULL`。空串即「无正文」，adapter 已按 provider 方言各自省略（§4.4），**不需要**在 `prompt_builder` 里转成 `Option`（`ChatMessage.content` 是 `String`）。
- **tool 结果行**：写独立 `role = 'tool'` 行（`role` 列无 CHECK 约束，无需迁移即可承载），`content` 存**与回填完全相同的那份文本**（即 `ToolOutcome.assistant_text`：未落盘时是 `result_for_assistant` 或 `data` 的紧凑序列化，已落盘时是 `<persisted-output path="…">` 引用 + 预览）；两份文本必须逐字节一致，否则重启重放与在线行为不同（A11）。
- **结果行的 `id` 用新 UUID，不借用 call_id**：本仓 `messages.id` 是主键且参与分支（`parent_id` / `sibling_index`）与前端标识，call_id 在同一对话内可能因重试重复，借用会污染主键语义。
- **新增列 `tool_call_id TEXT`**（旧行 `NULL`）承载 call_id，使 `prompt_builder` 能重建 provider wire 形状。
- **新增列 `is_error INTEGER NOT NULL DEFAULT 0`** 承载行级失败标记：Anthropic 的 `tool_result` 需要显式 `is_error`，不能靠 content 猜。
- 结果行 `parent_id` 指向**上一条写入行**（同批首条 tool 行指向本迭代 assistant 行，后续逐条链式追加），`sibling_index = 0`；一个 `tool_calls_json` 条目对应一条 tool 行，顺序一致，重放按行序而非时间戳匹配。链式追加是硬约束：所有 tool 行都挂 assistant 行会让 `active_branch_id` 停在第 1 条 tool 行，导致后续行与终态文本不可见（详见 [I38b 行链修复规格](./I38b-agent-loop-row-chain-fix-spec.md)）。
- **不落库**：`turn_id` / `iteration` / `provider_attempt`（守 I36 D10）。
- **不建 `tool_calls` 表**；迁移只允许 `ADD COLUMN`，禁止 `DROP` / `RENAME` / 存量行重写。
- `MessageDto` 逐字段不变（其 `deny_unknown_fields` 决定前端契约）；工具卡片渲染与 DTO 扩展归 P6。
- 审批审计走 P2 的 `PermissionAuditStore` 端口，不复用消息行。
- **`prompt_builder::prepare_messages` 必须改**：当前它把 `tool_calls` / `tool_call_id` / `is_error` 硬编码为 `None` / `false`，落库后要从三列取值重建，否则 tool 行虽已落库但模型下一轮看不到（A11 的核心断言）。

#### 6.2.1 投影期配对修复（D27）

重建 wire 形状时，行链可能因**跨进程中断**而不完整：assistant 行已带 `tool_calls_json`，但对应的 `role='tool'` 行没写成（进程被杀、DB 写失败、`append_tool_results` 中途失败）。三家 provider 都强制「每个 `tool_call` 必须有配对结果」，缺一条就会让该会话**之后每一轮请求 400**，且无法自愈。因此在 `prepare_messages` 重建阶段加一层**只读修复**（不写库、不改行链、不改 D11 落库形态）：

| 情形 | 判定 | 处理 |
|---|---|---|
| 正向孤儿 | 某 assistant 行的 `tool_calls_json` 中 `call_id` 在后续 `role='tool'` 行里找不到 | 按 `tool_calls` 顺序为该 `call_id` 合成一条 `role='tool'` 消息：`tool_call_id = call_id`、`name` 取父行 `tool_calls_json` 的 `name`、`content = "未执行：本轮中断"`、`is_error = true`（与 §5.3 的 `skipped` 同形，仅 `content` 文案不同）；`log::warn!` 记录 `conversation_id` / `call_id` |
| 反向孤儿 | 某 `role='tool'` 行的 `tool_call_id` 在**同一条分支**的前序 assistant 行里找不到 | 投影时丢弃该消息（不合成、不写库）；`log::warn!` 记录 `conversation_id` / `message_id` / `tool_call_id` |
| 空结果体 | `role='tool'` 行 `content` 为空串 | 投影时替换为 `"(空结果)"`；落库文本不改。理由：`anthropic/message_converter.rs::convert_tool_result_message` 在 `is_error` 且 `content` 为空时会填 `"Tool execution failed"`，把真实失败原因替换成通用文案 |

- **判定范围按分支**：配对只在同一 `parent_id` 链（即同一条活跃分支）内匹配，不跨兄弟分支，避免把另一分支的结果误配到当前分支的调用。
- **修复结果不落库**：合成行只存在于本次请求的 `Vec<ChatMessage>`；行链保持 append-only，A11 的「落库与 wire 同形」仍以正常路径为断言对象。
- **幂等**：修复是纯函数（输入行链 → 输出消息序列），重复调用结果一致；不得依赖请求次数或时间。
- **与 §5.3 的分界**：§5.3 的 `skipped` 是**在线路径**主动写库的「未执行」结果行（D18 预算裁剪）；D27 是**重放路径**对历史缺口的兜底合成（不写库）。两者文案可不同，但 `is_error = true` + `tool_call_id` 配对语义必须一致。
- **为什么补配对而不是删调用**：goose 的 `fix_tool_calling` 选择「删掉孤儿 `tool_request`」，本仓选择「补配对结果」——因为行链是 append-only 的模型可见历史，删调用会让模型看到「它从没发起过这个调用」的错误记忆，且与 §5.3 的 `skipped` 形状分叉；补结果则让「调用发生过但没执行」这个事实被如实表达。
- **不在 adapter 层修**：修复必须发生在 `prompt_builder`（唯一重建点），否则三 adapter 各写一份，且 `ChatMessage` 形状会被 provider 方言污染（§4.3 的「转换只在 adapter 内进行」约束）。

**与 I36 D10「turn 结构不落库」的边界**：

| 事实 | 是否落库 | 理由 |
|---|---|---|
| `messages.status` | 是（D10 唯一新列） | 跨进程恢复「这条消息没写完」 |
| `messages.tool_calls_json` | 是（预留列，P4 首次写入） | 模型可见历史，重启后必须能重放 |
| `messages.tool_call_id` / `messages.is_error` | 是（P4 新增列） | 同属模型可见历史；`is_error` 是 Anthropic wire 必需 |
| `turn_id` / `iteration` / `provider_attempt` | 否 | D10：turn 结构无消费者，落库会与 `conversations.generation` 产生重复版本语义 |

**判据**：一条事实若模型下一轮请求必须看到，则落库；若只是运行时簿记，则不落库。tool 结果属于前者，turn 结构属于后者。

### 6.3 端口扩展（D11 的落地前提）

I36 §2 定义的 `MessageStore` 只有 `create_assistant_message` / `save_content` / `finalize` / `record_usage`，不足以承载 D11，P4 需扩展两个方法（仍为 `#[async_trait]`，不得为此引入 `tauri`）：

```rust
#[async_trait]
pub trait MessageStore: Send + Sync {
    // ...P3 既有四方法...
    async fn save_tool_calls(&self, msg_id: &str, tool_calls_json: &str) -> Result<(), StoreError>;
    async fn append_tool_message(&self, req: AppendToolMessageReq) -> Result<String, StoreError>;
}

pub struct AppendToolMessageReq {
    pub conversation_id: String,
    pub parent_id: String,
    pub tool_call_id: String,
    pub content: String,
    pub is_error: bool,
    pub timestamp: i64,
}
```

- `save_tool_calls` 在 assistant 消息 `finalize` 之前调用，避免终态消息被二次改写。
- `append_tool_message` 一次调用只写一条结果行，顺序由调用方保证（与 `tool_calls` 一致）。
- 测试替身必须实现这两个方法，且能回放「写入 → 读回 → 重建 wire 形状」（A11）。
- **`append_tool_message` 的 `parent_id` 指向链尾行（首条指向本迭代 assistant 行，后续逐条链式追加），`sibling_index` 恒为 `0`**：tool 行不参与分支兄弟计数。链式追加后同父兄弟重复（原「同一 assistant 行下多个 tool 行 `sibling_index` 重复」）自然消失；分支视图按 `role` 过滤 tool 行（`role='tool'` 不渲染为独立消息），`sibling_index` 只在前端分支切换时使用，tool 行不参与。**不新增「按 role 分桶的 sibling_index」**，那会改 `get_sibling_count` 语义并波及现有分支逻辑。链式追加是硬约束：全部挂 assistant 行会让 `active_branch_id` 停在第 1 条 tool 行（见 [行链修复规格](./I38b-agent-loop-row-chain-fix-spec.md)）。
- **实现沿用 `DbMessageStore::with_conn`**（`src/adapters/message_store.rs:18`，`Mutex<Connection>` + 同步 rusqlite）：新增两个方法照抄该模式即可，不为 P4 引入 `spawn_blocking` 或连接池 —— 这是 P3 既定的落库方式，单次工具结果写入量与普通消息同量级；若后续成为热点（大量并行工具），在 P8 统一改造，P4 不做双轨。

**存储侧同步改动**（缺一不可，否则 `prompt_builder` 读不到）：

| 位置 | 改动 |
|---|---|
| `assistant-storage/src/db.rs` | 按既有「ADD COLUMN」两步法（`db.rs:150-175` 的 `provider_id` / `model_id` 就是现成样板，模板注释在 `db.rs:188-217`）：① 在 `CREATE TABLE messages`（`db.rs:46-60`）里加 `tool_call_id TEXT` 与 `is_error INTEGER NOT NULL DEFAULT 0`（新库直接建全）；② 在 `init_db` 的「既有库」`else` 分支（`db.rs:149` 起）追加两段 `pragma_table_info` 守卫 + `ALTER TABLE messages ADD COLUMN ...`（旧库补齐），放在 `model_id` 守卫之后 |
| `assistant-storage/src/conversations/mod.rs` | `MSG_SELECT` 追加两列（现为 13 列，新增为第 14 / 15 列）；`MessageRow` 加 `pub tool_call_id: Option<String>`、`pub is_error: bool`；`read_msg_row` 用具名列同步（`row.get("tool_call_id")` / `row.get::<_, i64>("is_error")? != 0`，与既有 `is_pinned` 写法一致） |
| `assistant-storage/src/conversations/msg_ops.rs` / `branch.rs` | 读行走 `MSG_SELECT` + `read_msg_row`，随上一条自动同步；**既有 INSERT 不用改**（`branch.rs:185` 等语句显式列名，新列靠 `DEFAULT` / `NULL` 兜底），只需新增一条带 `tool_call_id` / `is_error` 的插入路径（供 `append_tool_message` 使用），分支语义（`parent_id` / `sibling_index`）与普通消息一致 |
| `assistant-agent/src/prompt_builder.rs` | 从 `tool_calls_json` / `tool_call_id` / `is_error` 重建 `ChatMessage`（`content` 原样取行内容，空串保留）；同时建 `call_id → tool_name` 映射（来自前面 assistant 行的 `tool_calls_json`），给 `role='tool'` 行填 `name`（Gemini 必需，§4.4） |

- 迁移**只允许** `ADD COLUMN`；禁止 `DROP` / `RENAME` / 存量行重写（含禁止把 `role='tool'` 行批量回填）。
- `is_error` 用 `INTEGER NOT NULL DEFAULT 0`：旧行自动获得 `0`（非错误），语义正确，无需数据回填。
- `tool_call_id` 允许 `NULL`：旧行与非 tool 行都没有 call_id；`prompt_builder` 见到 `NULL` 就不填该字段。
- `MessageRow` 是内部读模型，**不是** protocol DTO；`From<MessageRow> for MessageDto`（`protocol/dto/chat.rs:104`）只挑字段、不 include `tool_calls_json`，因此扩展 `MessageRow` 不触发 `contract:gen`，前端契约在 P6 才动。
- `MessageRow` 新增字段会让现有**结构体字面量**编译失败，必须一并补 `tool_call_id: None, is_error: false`：`src/cmd/compaction.rs:167`、`src/cmd/chat/insert.rs:217` 与 `:256`、`src/adapters/message_store.rs:46`。这是纯机械补齐，不得借机改成 `..Default::default()`（`MessageRow` 也无 `Default`）。

### 6.4 审批计时（D12 的落地方式）

`assistant_tools::ApprovalRequest` 已带 `tool_call_id` 与 `timeout`，因此不需要改 P2 类型：在 `src-tauri` 用装饰器包住 `ApprovalPrompter` 实现。

```rust
struct TimingApprovalPrompter {
    inner: Arc<dyn ApprovalPrompter>,
    waits: Arc<Mutex<HashMap<String, u64>>>,   // call_id -> ms
}

#[async_trait]
impl ApprovalPrompter for TimingApprovalPrompter {
    async fn request(&self, req: ApprovalRequest) -> ApprovalDecision {
        // 守卫在 future 被 drop 时也会记录 —— 超时由 gateway 外层的
        // request_with_timeout 触发，会把本 future drop 掉，
        // 若用「请求前后取差值」的写法，超时路径永远记不下来。
        let _guard = WaitGuard {
            call_id: req.tool_call_id.clone(),
            started: Instant::now(),
            waits: self.waits.clone(),
        };
        self.inner.request(req).await
    }
}

struct WaitGuard { call_id: String, started: Instant, waits: Arc<Mutex<HashMap<String, u64>>> }

impl Drop for WaitGuard {
    fn drop(&mut self) {
        let elapsed_ms = self.started.elapsed().as_millis() as u64;
        if let Ok(mut waits) = self.waits.lock() {
            waits.insert(self.call_id.clone(), elapsed_ms);
        }
    }
}
```

> **用 `Instant::now()` 而不是注入的 `Clock`**：`Clock::now()` 的测试替身是 `FixedClock`（`kernel/ports/test_doubles.rs:228`，返回常量），用它算差值恒为 0，无法断言；且装饰器位于 `src-tauri`，多注入一个端口只为计时并不划算。直接取真实单调时钟，测试用一个会 sleep 约 20ms 的 `ApprovalPrompter` 替身并断言 `>= 20`（真实时间，不用精确值）。

- **不改 P2 的超时归属**：`ToolGateway::authorize` 已在 `gateway.rs:259` 用 `request_with_timeout(self.prompter, request)` 统一加超时；装饰器作为 `prompter` 被传入，因此**超时路径下本 future 会被 drop**。用 `Drop` 守卫记录，两种路径（正常审批 / 超时）都能拿到等待时长，无需把超时搬到装饰器内部、也不改 `assistant-tools` 代码。
- 超时路径记录的实际等待 ≈ `timeout`，超时决策仍是 `Deny`（I38a §2.8，不得放行）。
- `ToolExecutionPort` 适配器在 `pipeline.execute` 返回后，按 `tool_call_id` 取出该值填入 `ToolExecResult.confirmation_wait_ms`，随后 `remove`，避免无界增长。
- 未走审批（`Allow` 直通，未进入 `request()`）时值为 `0`，不区分「未审批」与「瞬时审批」——两者对墙钟影响相同。
- 该值只进 `ToolEvent` 与 `RoundOutcome.confirmation_wait_ms` 汇总，**不落库**（D12）。

**P2 遗留端口的生产绑定（C4.18 的现状与口径，2026-11-05）**：`src-tauri` 目前**只有** `ToolRegistry` 被初始化（`setup.rs:104` 的 `os::register_os_tools`），`ToolPipeline` / `ToolGateway` / `PermissionGrantStore` / `PermissionAuditStore` / `ApprovalPrompter` / `ToolResultStorage` / `PipelineEventSink` 一个都没有装配，grep 全仓无引用。C4.18 因此是「新建装配」而非「接线」，且必须先定两件事：

1. **`PermissionGrantStore` / `PermissionAuditStore` 的生产实现**：新建 `permission_grants` / `permission_audit` 两表（D14，`CREATE TABLE IF NOT EXISTS`），实现放 `src-tauri/src/adapters/`，沿用 `DbMessageStore::with_conn` 的 `Mutex<Connection>` + 同步 rusqlite 模式（§6.3 末尾的既定做法），不引入连接池或 `spawn_blocking`。
2. **`ApprovalPrompter` 的生产默认**：真实审批 UI 归 P6，P4 必须给一个语义明确的默认实现 —— **「Ask 即 Deny 并写审计」**：`request()` 立即返回 `Deny`（带 `reason = "approval channel not available in P4"`），审计由 `PermissionAuditStore` 记录该决策。这符合 I38a §2.8「超时视为 `Deny`、不得默认放行」，也保证 `ResolvedPermissionPolicy` 判为 `Ask` 的工具在 P4 是 fail-closed 而非静默放行。P6 用「发 `ConfirmationNeeded` 事件 + 命令回填」的实现替换该默认值，**装饰器结构不变**（`TimingApprovalPrompter` 包住新的内层即可，`confirmation_wait_ms` 仍是 `0`）。

> **fail-closed 的取舍理由（2026-11-05 参考项目核对）**：参考项目的主流做法是「审批通道不可用即拒绝」——`references/1-ai-agent/codex/codex-rs/core/src/session/mod.rs` 用 `rx_approve.await.unwrap_or(ReviewDecision::Abort)`，通道关闭回退 `Abort`；`references/1-ai-agent/OpenBitFun/src/crates/execution/agent-runtime/src/permission.rs` 把通道断开映射为 `PermissionWaitOutcome::Cancelled` 而非放行。**反例是 OpenHuman**：`references/1-ai-agent/openhuman/crates/openhuman-core/src/agent/tinyagents/middleware/approval.rs` 在 gate 不可用时直接 `Allow`，非聊天渠道更是「silently allow」，其前提是桌面端已有审批 UI、放行只为不锁死功能。P4 **尚无审批 UI**，采用 fail-open 等于让有外部副作用的工具在无人值守下静默执行，因此本阶段取 codex / OpenBitFun 的语义；P6 接入真实通道后仍保留「通道关闭 / 超时 → `Deny`」，不因有 UI 而放宽。

---

## 7 实施步骤

| 步骤 | 内容 |
|---|---|
| P4-C1 | 累积器迁移与 provider 快照语义（D1-D9） |
| P4-C2 | tools 通道与三 adapter 注入（§4.1-§4.3） |
| P4-C3 | RoundExecutor 契约与端口（C3.1-C3.21） |
| P4-C4 | 执行器实现（库侧 stop/middleware/ladder + 宿主侧 round_executor 实现） |
| P4-C5 | TurnRunner 切换到 RoundExecutor |
| P4-C6 | 事件 DTO 与 tool 落库（D5/D11/D14/D27） |
| P4-C7 | 端到端测试 |

> 本表只列步骤与内容，**不记录进度**；步骤级完成情况见 [I38b 任务清单](./I38b-agent-loop-tasklist.md)，阶段级结论见 [roadmap](../../store/I38-agent-roadmap.md) §2。
> 未完成步骤的细化展开见 [I38b 实施指南](./I38b-agent-loop-impl-guide.md)（含 §5 算法讲解提取与逐项实施说明）。
> **切换约束**：P4-C5 之前，`TurnRunner` 不得同时保留旧循环与 executor 两条路径；用一次性替换，不做运行时开关。C1 删 `StreamOutcome.tool_calls` 后过渡循环即不可编译，因此 C1 与 C5 同批交付。

---

## 8 验收标准

| # | 标准 |
|---|---|
| A1 | 三迭代闭环：模型请求工具 → 执行 → 回填 → 再次请求工具 → 最终文本，消息序列与 tool 消息数量正确 |
| A2 | `max_iterations` 触发收尾：最后一次请求不含 `tools`，`hit_cap = true`、`wrap_up_injected = true` |
| A3 | 空响应（有推理无正文、且无工具调用片段）重试 1 次后仍空 → `Failed`，`iteration` 不因重试递增；**纯工具调用（无正文但有 `tool_calls`）不触发重试**，正常进入工具执行；**被输出上限截断（`finish_reason` 为 `length` / `max_tokens` / `MAX_TOKENS`）不重试**，`model_calls` 只加 1，直接 `Failed` |
| A4 | 取消：部分内容落库、`status = interrupted`、工具结果标 `Cancelled`、状态回 `Idle` |
| A5 | 工具失败（`error_kind = Business`）回填给模型后可继续下一迭代；`Denied` 不重试 |
| A6 | 三个 adapter 的 `ToolSpec` 注入形状有快照测试；Gemini 快照语义与 `arguments_is_snapshot` 一致 |
| A7 | 累积器只有一处实现：`assistant-providers/src/tool_call_accumulator.rs` 已删除，`assistant-agent/src/kernel/stream.rs` 无 `ToolCallAccum`（`StreamOutcome` / `PartialOutcome` 无 `tool_calls` 字段），全仓无第二份（§3.1） |
| A8 | `cargo tree -p assistant-agent` 无 `tauri`、无 `assistant-tools`；`agent/round_executor.rs` 仅含 trait 与值类型 |
| A9 | chat 路径行为不变：`default` / `quick` / `deep` / `custom` 等旧值模式（P6 起按 chat 链路处理）`tools = None`（即使注册表有工具）、无 tool 消息、事件序列与 P3 对比测试一致；仅 `kind = agent` 的模式注入工具 |
| A10 | 事件 DTO 经 `npm run contract:gen` 生成（`AgentToolCallDto` / `AgentToolCallPayloadDto` 进 `protocol.contract.ts`，四变体为 discriminated union）；`cargo test --test assembly_contract` 与 `cargo test --test protocol_dto_roundtrip` 通过；`protocol_bindings.rs` 的「源码 TS 派生类型 ↔ `declarations`」集合断言通过（2026-11-05 起取代原计数断言，新增 DTO 只加 `declarations` 条目即可，无魔数要改） |
| A11 | tool 落库往返一致：assistant 行 `tool_calls_json` + `role='tool'` 行（`tool_call_id` / `is_error`）写入 → 读回 → `prompt_builder` 重建出 §4.4 的 wire 形状，顺序与 `tool_calls` 一致；迁移只做 `ADD COLUMN`，旧行 `NULL` / `0` 语义正确 |
| A11b | 投影期配对修复（D27）：构造「assistant 行有 `tool_calls_json` 但无 tool 行」的历史 → `prepare_messages` 产出配平的 wire（合成 `is_error = true` 的「未执行：本轮中断」结果），且 `messages` 表**无新增行**；构造「tool 行无父调用」→ 投影丢弃该消息、请求仍合法；修复对同一输入幂等 |
| A12 | 三维预算各自独立生效：`max_iterations` / `max_tool_calls` / turn 墙钟单独触发都能走 §5.2 的 `halted_by_budget()`（`Completed` + `hit_cap` + 文本非空）；`max_tool_calls` 按发起数计（拒绝/失败/取消都算） |
| A12b | 预算内裁剪：单轮请求 5 个调用、`max_tool_calls` 只剩 2 时，只执行前 2 个，后 3 个写「未执行」`role='tool'` 行（`is_error = true`）；下一轮请求 wire 合法（每个 `tool_call` 都有配对结果）；`skipped` 不计入重复失败与分类预算 |
| A13 | 重复失败阶梯：同工具同参连续失败 2 次 → 下一请求出现 nudge 且 `messages` 表**无**新增行；连续 3 次无进展 → Halt，`text` 为根因摘要而非最后一条工具输出；分类命中时按 D25 预算提前 Halt（A17） |
| A14 | 工具执行策略：安全工具批内并行（墙钟明显小于串行和）、不安全工具串行；未知工具与 schema 校验失败返回 `is_error = true` 且 turn 继续；结果顺序与 `tool_calls` 严格一致 |
| A15 | 空响应重试：`model_calls` 递增而 `iteration` 不变；重发前空白 assistant 行不落库，`messages` 表无该行；纯工具调用不触发重试（与 A3 同口径） |
| A16 | 收尾还原：微压缩清空过结果体的 capped turn，结论调用能看到最近结果（受 `restore_allowance`），后续轮次不再保留还原（**摘要相关断言见下方归属说明**） |
| A17 | 失败分类阶梯：`Gateway/Denied` 首次即 Halt；`Validate/Business` 首次 nudge、第 2 次 Halt；`Execute/Internal` 允许 2 次重试后 Halt；同一 `(class, tool, scope)` 成功一次即清零；分类命中时不触发 D19 通用计数 |
| A18 | 端口镜像不漂移：`ToolStage` / `ToolErrorClass` 与 `assistant_tools::PipelineStage` / `ToolErrorKind` 的一一对应由 contract 测试锁定；`assistant-agent` 源码 grep 无 `assistant_tools` 引用 |
| A19 | 回填方言正确：Anthropic `tool_use.input` 与 Gemini `functionCall.args` 是 **JSON 对象**（非字符串）；Gemini `functionResponse` 带工具名；非法 arguments 回退 `{}` 且请求不 400。快照测试覆盖三 adapter 的「带工具历史的第二轮请求」 |
| A20 | 行链正确：三迭代后 `messages` 为 `user → assistant₁(tool_calls) → tool×n → assistant₂(tool_calls) → tool×m → assistant₃(text)`，`parent_id` 依次指向前一条；`conversations.active_branch_id` 最终指向 `assistant₃`；**不存在**挂在 `user` 下的兄弟分支（前端不出现假分支）；**不存在**未 `finalize` 的悬空行（第 1 迭代任何提前返回路径都收尾占位行） |
| A21 | 双 Halt 出口：`max_tool_calls` 耗尽 → `status = completed`、`hit_cap = true`、`text` 非空（`wrap_up = true` 时 `wrap_up_injected = true` 且多一次不含 `tools` 的调用）；无进展阶梯耗尽 → `status = failed`、`text` 为根因摘要。两档都不出现空文本（含 `wrap_up = false` 且最后一轮仍有工具调用、循环自然走完的路径 —— 该路径同样产摘要文本 + `hit_cap = true`），且**摘要/结论都落成一条 assistant 行**（`RoundOutcome.final_message_id` 指向它、`active_branch_id` 指向它，`content` 与 `text` 一致） |
| A21b | Halt 文本可读：两档出口落库的 `text` 与 `RoundOutcome.text` 都是中文完整句，**不含**中间件内部标识（无 `repeated-failure:` / `tool-output-cap:` 之类前缀）；`StopTurn.middleware` 只进 `log::warn!` 与遥测；摘要文本不重复拼接次数（§5.1 / §5.2） |
| A22 | 错误路径建行：第 2 迭代取消 / 超时 / provider 错误 → 新增一条 assistant 行（`parent_id` = 上一条 tool 行）并写入已累积部分内容与已上报 usage；**不得**复用第 1 迭代的 `msg_id`（该行内容与 `status` 保持不变） |
| A23 | delta 唯一路径：正文 / 推理增量只经 `TurnObserver::delta` 发射，同 turn 内 `seq` 严格单调、`iteration` 与实际迭代一致；旧代际 turn 的 delta 被丢弃（无重复事件、无 `seq` 回退） |

> **A16 摘要断言归属（2026-11-05 调整）**：原 A16 中的「摘要调用不推进 `iteration`、但计入 `model_calls`」依赖阶梯 ② 的摘要调用存在，而 ② 随续跑式压缩同批落地（D13），**P4 不实现**。该断言移入续跑式压缩批次的验收项；P4 的 A16 只覆盖 ④ 收尾还原，避免出现「P4 验收不可达」的条目。同理，C7.9 只验还原与不落库，不验摘要计数。

---

## 9 风险与决策定稿

| 风险 | 缓解 |
|---|---|
| 三 adapter 快照语义再次分叉 | D9 单字段 + A6 快照测试；Gemini 累积上移（D8） |
| 工具结果过长挤爆上下文 | P2 的三层预算与落盘在 executor 内必经 |
| 收尾调用被误当成新迭代 | 明确「收尾占第 N 个名额」；`hit_cap` 与 `wrap_up_injected` 分开 |
| 依赖方向被破坏 | A8 以 `cargo tree` 断言；RoundExecutor 实现只允许在 `src-tauri` |
| P4-C5 切换导致 chat/agent 双路径漂移 | 一次性替换，不做运行时开关；A9 对比测试 |
| tool 结果行数翻倍推高上下文占用 | 结果行只存回填文本（已落盘时为引用 + 预览，不存全文）；阶梯 ① 投影按行序截断 |
| `role='tool'` 行进入分支视图 | 结果行链式挂在前一行（首条挂本迭代 assistant 行）、`sibling_index = 0`，不产生新分支；链式追加同时保证 `active_branch_id` 推进到最后一行 |
| 只卡迭代次数导致单轮工具风暴 | D18 的 `max_tool_calls` 与墙钟独立计数；A12 |
| nudge 写库污染后续轮 | D19 只进本次请求；A13 断言 `messages` 无新增行 |
| 并行执行打乱结果顺序 | 批内并行、按 `tool_calls` 索引回填与落库；A14 |
| 空响应重试留下空白 assistant 行 | 行在第 4.5 步有效输出后才建，无有效输出不建行；D22 重发前 `accumulator.reset()`；A15 |
| 输出上限截断被误判成空响应，白花一次调用且必然再次截断 | D22 的 `is_output_limit_reached(finish_reason)` 直接跳过重发；A3 断言 `model_calls` 只加 1 |
| 历史行链缺 tool 结果行（跨进程中断）导致会话之后每轮 400 且无法自愈 | D27 投影期配对修复（只读、幂等、不写库）；A11b 覆盖正向 / 反向孤儿 |
| 打开 `tools` 通道后回填消息被 provider 拒绝（`input` / `args` 是字符串、Gemini 缺工具名） | §4.3 的三处方言修正 + A19 快照测试；这是死代码转生产时必踩的第一个坑 |
| 新增 DTO 漏改契约机械项（`declarations` / roundtrip） | §6.1.1 的改动清单逐项核对；A10 同时跑 `assembly_contract` 与 `protocol_dto_roundtrip`。`declarations` 漏注册已由集合断言直接报出类型名（2026-11-05 起取代计数断言），不再靠魔数兜底 |
| 事件形状用 `phase` 字段 + `Option` 堆叠，P6 加审批阶段时被迫堆字段 | D5 定为阶段枚举变体；扩展只加变体（与 OpenBitFun `ToolEventData` 一致） |

### 9.1 决策定稿（P4 开工前生效）

原开放问题 1–3、I36 §11 问题 1–3 及 D17–D27 新增开放点在此定稿；取值一律以参考项目已落地的做法为准，不另造方案。

> 编号说明：D11–D27 沿用 §3.2 的 D 编号序列（本文体系）；引用 I36 design 的决策时一律带 `I36` 前缀（如 `I36 D10`），两套编号不互通。

| # | 决策点 | 结论 | 依据 |
|---|---|---|---|
| D11 | tool 结果是否落库 | **落库**，写独立 `role = 'tool'` 行 + 新增 `tool_call_id` / `is_error` 两列，不建 tool 表（详见 §6.2） | OpenHuman / Codex / OpenBitFun 三家均落库且均不建 tool 表 |
| D12 | 审批等待是否计入 turn 墙钟 | **计入**，同时单独记录 `confirmation_wait_ms`，与工具自身 `duration_ms` 分开 | OpenBitFun `ToolItemData.confirmation_wait_ms` 与 `duration_ms` 分离；审批等待是用户侧延迟，不能算进工具执行耗时 |
| D13 | 阶梯 ② 摘要压缩与压缩续跑式切换是否随 P4 同批 | **不同批**：P4 不实现阶梯 ②（`SummarizeMiddleware` / `agent/compaction_exec.rs` 均不落地），压缩保持终止式，续跑式切换与 ② 同批独立提交 | I36 design D9 第二步；Codex 用 `CompactTrigger` 统一事件 |
| D14 | 存储形态 | 消息侧只 `ADD COLUMN`；授权侧新建 `permission_grants` / `permission_audit` 两表（`CREATE TABLE IF NOT EXISTS`，不做 ALTER 迁移） | 主库迁移约束；P2 端口需生产实现（I38a §11 问题 2 留给 P4 决定，此处定稿） |
| D15 | P4 的真实工具集 | `ToolCatalog` 走 `ToolRegistry::tool_definitions_for_tier`，只取 `audience().agent == true`；闭环验收用替身工具，工具集为空时仍须通过 | I18 design 工具受众分层；真实 OS 工具归 P5 |
| D16 | `ToolCatalog` 端口同步还是 async | **async**（`#[async_trait]`），P4 实现可纯内存读取 | OpenHuman `memory/conversations/blocking.rs` 的教训：同步 store 调用会 park tokio worker；P8 按 agent 收窄时不必改签名 |
| D17 | 落库判据 | 模型下一轮请求必须看到 → 落库；纯运行时簿记 → 不落库 | 见 §6.2 边界表；与 I36 D10「turn 结构不落库」划清 |
| D18 | 循环预算维度 | **三维独立**：`max_iterations` / `max_tool_calls` / turn 墙钟；任一触发走 §5.2 的 `halted_by_budget()`（文本非空、`hit_cap = true`），不静默截断 | OpenHuman `turn_runner.rs` 同时设 `with_max_model_calls` + `with_max_tool_calls`，`turn_policy.rs` 另设 `max_wall_clock_ms` 与 `model_call_wall_clock_ms`；只卡迭代会让单轮多工具调用失控 |
| D19 | 重复失败与无进展 | 三级阶梯 `Continue / Nudge / Halt`：nudge 只进本次请求不落库；**Halt 只用于「无进展」**（同参失败达阈值），用根因摘要覆盖最终文本并置 `Failed`；预算耗尽属另一出口，走 §5.2 的 `halted_by_budget()`（`Completed` + `hit_cap`，文本非空） | OpenHuman `RepeatedToolFailureMiddleware`：第 2 次同参失败发 Redirect(nudge)、第 3 次才 pause(halt)，成功即重置计数；预算出口与无进展出口必须分开，否则与 I36 验收 17 冲突 |
| D20 | 工具执行策略 | 按 `is_concurrency_safe` 分批次：连续安全调用合批并行、其余单独成批串行（未注册按不安全 fail-closed）；未知工具与 schema 校验失败返回可修正错误（`is_error = true`），不中断 turn | 复用 `assistant-tools::pipeline::partition_tool_batches` 现状；Codex `tools/parallel.rs` 用 `supports_parallel_tool_calls` 做读写闸门；OpenHuman 未知工具/参数错误回灌模型自纠 |
| D21 | 工具结果体积 | 沿用 P2 的 `select_persist_targets` 预算，超限落盘并回填 `<persisted-output path="…">预览` | `assistant-tools/src/storage.rs` 已实现单条/单轮双阈值与降序选择；避免结果行把上下文挤爆 |
| D22 | 空响应重试与落库 | 「空」= 无正文**且**无工具调用片段（`!has_effective_output && !accumulator.has_pending()`），纯工具调用是有效输出、不重试。**被输出上限截断（`finish_reason` 为 `length` / `max_tokens` / `MAX_TOKENS` / `max_output_tokens`）时不重试**：同一 `max_tokens` 重发必然同样截断。重试不递增 `iteration` 但递增 `model_calls`（内层 `loop` 实现，见 §5）；重发前 `accumulator.reset()`；**整轮无有效输出就不建行**（行在第 4.5 步才建），因此不存在「空白 assistant 行」需要丢弃；`iteration = 1` 的占位行以空内容 `finalize(Failed)`，`iteration > 1` 不新增行 | 空白行落库会在重启/重放时被当成真实助手消息；行级落库把建行推迟到有效输出之后，从源头消除该问题。P3 的 `has_effective_output` 只统计正文，多迭代下若不并入工具片段会把每次纯工具调用都判成空响应。输出上限判据对齐 goose `inference.rs` 的 `output_token_limit_reached`：截断不是「模型没说话」，重发无意义 |
| D23 | 收尾调用是否还原被微压缩清空的工具结果 | **还原**，但只限本次结论调用、受 `restore_allowance` 约束（与内容清单共用一份 allowance） | OpenHuman `harness_context_ladder.rs`：阶梯 ④ `FinalCallWrapUpMiddleware` 撤销 ③，`split_input_allowance` 拆分额度；详见 I36 §11 问题 2 |
| D24 | 压缩是否重置迭代计数 | **不重置**；摘要调用不计 `max_iterations`，但计入 `model_calls` 与 token 统计 | OpenHuman `max_model_calls` 为 harness 级上限，压缩是 `before_model` 中间件不改计数器；详见 I36 §11 问题 3。**该口径在阶梯 ② 落地后生效**，P4 无轮内压缩，验收断言移出本批 |
| D25 | 失败是否按类别分预算 | **分**：先按 `error_kind` 走分类阶梯（validation / not_found / permission / transient / uncertain_side_effect），未分类的才落到 D19 的通用 2/3 阶梯；分类命中即由分类预算独占，不再叠加通用计数 | OpenHuman `recovery_policy()` + `ClassifiedFailureTracker`：`validation` / `not_found` / `transient` 预算 1–2 次，`permission` / `policy` 零重试，`uncertain_side_effect` 先核对再重试；分类预算耗尽即 halt |
| D26 | 重复调用豁免 | P4 豁免集为空；P8 引入 `wait_subagent` 时加入白名单（轮询类工具的相同参数重复调用属于合法进展，不计入 D19/D25） | OpenHuman `is_repeat_call_exempt()`：仅 `wait_subagent` 豁免，避免长任务在第三次轮询就被熔断 |
| D27 | 投影期配对修复 | **`prompt_builder` 重建 wire 时做一次只读配对修复**：正向孤儿（assistant 行 `tool_calls_json` 有调用、无对应 `role='tool'` 行）合成 `is_error = true` 的「未执行：本轮中断」结果行（只进投影、不写库）；反向孤儿（`role='tool'` 行找不到父调用）投影时丢弃并 `log::warn!`。规则见 §6.2 | goose `conversation.rs::fix_messages` 每次发请求前跑 `fix_tool_calling`（删孤儿 tool_request / 孤儿 tool_response）与 `fix_empty_tool_results`（补占位）；OpenAI / Anthropic / Gemini 三家都强制 function_call ↔ 结果配对，缺配对整条会话之后每轮 400、且无法自愈 |

**D12 细节**：超时沿用 I38a §2.8 —— 超时即 `Deny`，不得放行、不得无限挂起；默认 **120s**（P6 定稿，见 [I38d §3.3](./I38d-agent-frontend-modes-spec.md)，原「5 min」作废），取值由审批配置 `approvalTimeoutSecs` 提供。P4 只在 `ToolEvent` 与内存统计里带该值，不落库；是否落库与展示由 P6 定。

**D14 细节**：两张新表按 `CREATE TABLE IF NOT EXISTS` 建在 `db.rs` 的建表段（幂等，老库首次启动即补齐，不需要 ALTER 迁移），字段直接对齐 P2 端口入参，不做额外索引或历史表：

```sql
CREATE TABLE IF NOT EXISTS permission_grants (
    grant_key TEXT PRIMARY KEY,   -- ApprovalKey 的规范 JSON（tool + scope，scope 用 tag="kind"）
    tool      TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS permission_audit (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    tool      TEXT NOT NULL,
    action    TEXT NOT NULL,
    resource  TEXT NOT NULL,
    outcome   TEXT NOT NULL,      -- AuditOutcome 的 camelCase 值
    at_millis INTEGER NOT NULL
);
```

- `grant_key` 存 `serde_json` 序列化后的 `ApprovalKey`（`ApprovalKey` 是 `Hash + Eq` 且字段顺序固定，序列化结果可作稳定键）；`tool` 冗余一列仅为按工具排查。
- 审计只追加不更新；不设 TTL、不做自动清理（清理策略随 P6/P7 定）。
- **消息侧不动结构**：`messages` 只加 `tool_call_id` / `is_error` 两列（§6.3），不新建 `tool_calls` 表。

**D15 细节**：`fs_link` 为 `TOOLBOX_ONLY`，不会出现在 agent 工具集；某环境工具集为空时，P4 的闭环测试改用替身工具，不阻塞验收。`max_tier` 取 `ToolTier::Os`（`Sandbox` / `Os` 全收、`External` 排除）—— `External` 只承载 MCP 工具（P7），P4 不应提前把它们暴露给模型；该值作为常量放在适配器内，随 P7 引入 MCP 时改为按「已连接 server」动态计算。

**D18 细节**：三个上限都来自 `TurnLimits`，默认值随配置给出（配置项随 P6 落地）；`max_tool_calls` 按**发起数**计，被拒绝/失败/取消的调用同样计数（它们已花了预算）。turn 墙钟沿用 I36 的 `remaining_turn_budget`，审批等待计入其中（D12）。三个维度由 `assistant-agent/src/agent/stop.rs` 的三个 `StopHook` 分别读取 `TurnState` 的 `iteration` / `tool_calls` / `elapsed + turn_deadline` 独立判定，任一返回 `Stop` 即走 Halt，不做优先级合并；钩子按 I36 Step 5.1 的同步契约实现，只读快照、不读时钟、不做 IO（`turn_deadline` 由 TurnRunner 算好传入）。

**D19 细节**：

| 档位 | 触发 | 动作 |
|---|---|---|
| `Continue` | 有进展 | 照常进入下一迭代 |
| `Nudge` | 同一工具同一参数连续失败 2 次 | 下一迭代 §5 的 2.5 注入一条纠正提示（「你刚才重复了同一调用，请换一种方式」），**只进本次请求** |
| `Halt`（预算） | 任一预算耗尽（`max_iterations` / `max_tool_calls` / turn 墙钟） | 走 §5.2 的 `halted_by_budget()`：`spec.wrap_up = true` 时再调一次模型产出结论（`Completed` + `hit_cap`），否则用根因摘要产出文本（`Completed` + `hit_cap`）。**不是 `Failed`** |
| `Halt`（无进展） | 连续 3 次「同一调用同参失败」且已 nudge 过 | 走 §5.2 的 `halted_with_summary()`：不再调模型，`RoundOutcome.text` 用中间件给出的根因摘要（含工具名 / 次数 / 末次原因，已是中文完整句）覆盖，`status = Failed` |

**与 D22 的分界**：空响应（无正文且无工具调用）**只走 D22 重试**，不计入本阶梯的「无进展」；重试仍空直接 `Failed`。因此本阶梯的计数只统计**有工具调用但结果无进展**（同参失败、重复调用同一工具拿到同样结果）的情形，两者互不叠加。

nudge 与 halt 的阈值先定为模块常量（nudge = 2、halt = 3），与 OpenHuman 的 `RepeatedToolFailureMiddleware` 一致（其测试 `repeated_tool_failure_pauses_only_after_the_threshold` 即「第 2 次 nudge、第 3 次 pause」）；P4 不进配置，P6 若需可调再进 `TurnLimits` 并同步 `assistant-config` 与 protocol DTO。`Nudge` 不额外消耗预算；**成功一次即重置计数**（与 OpenHuman `repeated_tool_failure_resets_on_a_success` 一致）。

**D20 细节**：并发安全判定直接用 `Tool::is_concurrency_safe(Some(&arguments))`（参数敏感，如 `read` 的多路径），`partition_tool_batches` 由 `ToolPipeline::execute` 内部调用（P2 现状），**executor 不调用、不重写**（§4.5 职责切分）。未注册工具按不安全处理（fail-closed）。并行只缩短墙钟，**不改变结果顺序与落库顺序**：`execute` 保证返回顺序与入参一致（内部 `slots: Vec<Option<ToolOutcome>>` 按原索引回填）。未知工具与校验失败是**可修正错误**：模型看得到错误文本并能在下一轮改参数重试，因此不计入 D19 的通用「无进展」计数，而按 D25 归入 `validation` 分类（给 1 次纠正机会）。

**D21 细节**：预算与落盘直接复用 `assistant-tools/src/storage.rs` 的 `select_persist_targets`（单条 `PER_TOOL_LIMIT_CHARS` / 单轮 `PER_ROUND_LIMIT_CHARS`，按大小降序选择）与 `persisted_reference`（`<persisted-output path="…">` + `PREVIEW_CHARS` 预览）；调用点在 `ToolPipeline::execute` 收尾，executor 不重复调用。落库与回填必须是**同一份** `ToolOutcome.assistant_text`（落盘后已改写为引用 + 预览），否则重启后重放与在线行为不一致（A11 覆盖）。

**D22 细节**：判定「空响应」沿用 A3 口径 —— 有推理但无正文且无工具调用；重试请求沿用同一 `messages` 与同一 `tools`，仅靠 provider 重新采样。`empty_response_retries = 1`（模块常量，与 OpenHuman 单次重发一致；不进配置）。重试仍空则 `Failed`，`RoundOutcome.text` 写「模型未返回有效内容」。**行处理**（§5.1 的「返回后才建行」）：`iteration = 1` 复用命令层预建的占位行，把空内容 `finalize(Failed)`（该行本来就在，不清扫）；`iteration > 1` 因为还没建行，**不新增任何行**（A15）。

**D25 细节**：分类输入用 `ToolExecResult` 的 `(stage, error_kind)`，**不扫错误文本**（OpenHuman 的文本标记匹配是为兼容历史错误串，我们没有历史包袱；`stage` 由 Pipeline 精确给出，比文本更可靠）。现有 `ToolErrorKind` 只有 `Business/Timeout/Cancelled/Internal/Denied`，因此分类必须结合 stage —— 未知工具在 Pipeline 里标的是 `Internal`，只看 kind 会误判成可重试的瞬时错误。

| `stage` | `error_kind` | 分类 | 允许重试次数（nudge 之后） | nudge 文案要点 |
|---|---|---|---|---|
| `Find` | `Internal` | `validation` | 1 | 「工具名不存在，改用已提供的工具」 |
| `Validate` | `Business` | `validation` | 1 | 「上次调用未通过校验，修正参数后重试一次」 |
| `Gateway` | `Denied` | `permission` | 0（首次即 halt） | 权限拒绝不会因重试自愈，直接结束并用根因摘要覆盖 |
| `Gateway` | `Internal` | `transient` | 2 | 网关内部故障，可重试 |
| `Execute` | `Timeout` | `uncertain_side_effect` | 1 | 「命令超时可能已部分生效，先核对结果再以更小粒度重试」 |
| `Execute` | `Business` | `validation` | 1 | 「业务校验失败，修正参数后重试一次」 |
| `Execute` | `Internal` | `transient` | 2 | 不 nudge，仅计数 |
| 任意 | `Cancelled` | 不计 | — | 取消不是失败（D20），直接回填 |
| `Persist` 及其余 | 任意 | 不分类 | — | 落到 D19 通用阶梯（同参失败 2 次 nudge、3 次 halt） |

组合规则：分类命中即**独占**——同一 `(class, tool, scope)` 的计数只由分类预算管，不再进 D19 的通用计数；任何成功观测清空该 scope 的全部分类计数（OpenHuman 只在「同工具同 scope 成功」时清，避免无关成功掩盖阻塞）。`scope` 取工具的稳定目标标识（路径 / URL 去 query），P4 的替身工具用 `tool_name` 兜底；`scope` 与 D19 的「同参指纹」都由 executor 从 `FinalizedToolCall.arguments` 现算，**不由端口回传**（端口结果里没有参数，OpenHuman 也要靠 `before_tool` 另存 `call_id → arg_fp`）。分类阶梯同样受 D18 三维预算约束，预算耗尽优先于分类预算。

**D26 细节**：豁免判定放在 `after_tool` 最前面——豁免工具即使同参失败也不进任何阶梯；P4 无此类工具，代码保留空表 + 单元测试，P8 只需加一行。
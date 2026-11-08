# I38b Agent Loop 实施指南

| 项 | 值 |
|---|---|
| 状态 | 现行（P4 实施中） |
| 日期 | 2026-11-06 |
| 阶段 | P4（见 [I38 roadmap](./I38-agent-roadmap.md)） |
| 关联 | [I38b 主规格](./I38b-agent-loop-spec.md)（算法权威定义）、[I38b 任务清单](./I38b-agent-loop-tasklist.md)（进度唯一来源） |

> 本文从主规格 §5（循环伪码/落库/双Halt/裁剪）与 §7（实施步骤）中提取未完成步骤，细化为可直接编码的实施条目。主规格 §5 是唯一权威算法定义，本文仅做展开讲解与步骤串联，不得与本规格冲突。

---

## 1 内容索引

> **进度唯一来源是 [任务清单](./I38b-agent-loop-tasklist.md)**（勾选 + V 表偏差 + 验收映射），本文不维护进度，只做步骤展开讲解。

| 批 | 步骤 | 本文章节 |
|---|---|---|
| C3 契约 | C3.22-C3.27 | §2 |
| C4 循环 | C4.2-C4.40c | §3 |
| C5 切换 | C5.1-C5.5 | §4 |
| C7 测试 | C7.1-C7.17 | §5 |
| C8 验收 | C8.1-C8.13 | §6 |
| C9 文档 | C9.1-C9.3 | §7 |

C1 / C2 / C3（C3.1-C3.21）/ C4（C4.0a-0d/1/12-15/18/21c）/ C6 不在本文档范围，进度见任务清单。

---

## 2 C3 契约补齐（C3.22–C3.27）

### C3.22 RoundOutcome 补三字段

当前 `RoundOutcome` 已有 `reasoning` / `total_tokens` / `attempt_usages` / `ttft_ms`（C3.20 补齐），需再加：

```rust
pub struct RoundOutcome {
    // ... 既有字段 ...
    pub final_message_id: Option<String>,    // 本 turn 最后一条已 finalize 的 assistant 行 id
    pub context_window_tokens: Option<u32>,  // 最后一次请求的投影值
    pub reserved_output_tokens: u32,         // 最后一次请求的 max_tokens
}
```

- `final_message_id`：`None` 仅当「本 turn 从未写入任何 assistant 行」（如 `request::build` 失败、首迭代建行前取消）。其余路径由 `persist_row` / `persist_row_as_new` / `persist_final_text` 成功时置 `Some`。D22 空响应在 `iteration = 1` 写占位行、`iteration > 1` 沿用上一迭代已写的行，都不为 `None`。
- `context_window_tokens`：取 `last_request.context_window_tokens`。
- `reserved_output_tokens`：取 `last_request.max_tokens`。
- 同步 `TurnOutcome` 带出 `final_message_id`。

### C3.23 TurnRunCtx 移除 EventSink

`TurnRunCtx` 当前有 `EventSink` 字段，必须移除。TurnRunner 侧传参一并删除。全仓确认 delta 无第二条发射路径（唯一发射口是 `TurnObserver::delta`）。

### C3.24 TurnObserver 补 delta 方法

```rust
pub trait TurnObserver: Send + Sync {
    // ... 既有方法 ...
    fn delta(&self, delta: &ProviderDelta) -> u32;  // 正文/推理增量的唯一入口，返回本次 seq
}
```

`RunnerObserver` 实现委派 `TurnEmitter::delta`（seq 单调、代际校验、`iteration` 标记都在 `TurnEmitter`）。executor 不得直接持有 `EventSink`。

### C3.25 TurnRunCtx 补两字段

```rust
pub struct TurnRunCtx<'a> {
    // ... 既有字段 ...
    pub initial_message_id: &'a str,  // 命令层预建的占位行 id（第 1 迭代复用）
    pub turn_started_at: Instant,     // turn 起点时刻（算 elapsed 用）
}
```

- `remaining_turn_budget.turn_deadline: Option<Instant>` 保持既有定义（绝对截止时刻），不重复。
- TurnRunner 侧传参同步这两个字段。

### C3.26 RoundOutcome 单一构造器

所有 return 点统一走一个 `finish()` 构造器，禁止各点各拼字段：

```rust
fn finish(status: TurnStatus, text: String, reasoning: String,
          hit_cap: bool, wrap_up_injected: bool,
          error: Option<TurnError>) -> RoundOutcome {
    RoundOutcome {
        status, text, reasoning, hit_cap, wrap_up_injected, error,
        iterations: iteration,
        model_calls, tool_calls,
        prompt_tokens: token_stats.prompt,
        completion_tokens: token_stats.completion,
        cache_read_tokens: token_stats.cache_read,
        total_tokens: token_stats.total,
        attempt_usages, ttft_ms: token_stats.ttft_ms,
        confirmation_wait_ms,
        final_message_id,
        context_window_tokens: last_request.context_window_tokens,
        reserved_output_tokens: last_request.max_tokens,
    }
}
```

### C3.27 TurnError 补齐 P4 出口变体

```rust
pub enum TurnError {
    Timeout,
    EmptyResponse,
    NoProgress { reason: String },
    Persist(String),
    Provider(String),
    MiddlewareStop { name: String, reason: String },
    Other(String),
}
```

保留既有 `MiddlewareStop` / `Other`，新增 `Timeout` / `EmptyResponse` / `NoProgress` / `Persist` / `Provider`。不参与 wire（前端只看 `status` + `text`）。

---

## 3 C4 核心循环实现

> 算法权威定义见 [I38b 主规格 §5](./I38b-agent-loop-spec.md)，本节按实施顺序展开细化。以下所有函数均为 `run_iterations` 内部的闭包或 `RoundState` 私有方法。

### 3.1 RoundState 结构体（实现推荐）

把循环内所有可变量收进一个结构体，避免「两处各写一半」：

```rust
struct RoundState {
    // 第 0 步产出
    request: ChatRequest,
    last_request: ChatRequest,
    // 行级落库
    msg_id: String,
    row_created: bool,
    row_finalized: bool,
    last_row_id: String,
    // 累积器
    accumulator: ToolCallAccumulator,
    // 中间件链（单例，跨迭代复用）
    chain: MiddlewareChain,
    // 迭代内累积
    pending_nudges: Vec<String>,
    previous_content: String,
    // 计数器
    model_calls: u32,
    tool_calls: u32,
    // token 统计
    token_stats: TokenStats,
    attempt_usages: Vec<UsageSnapshot>,
    // 审批等待
    confirmation_wait_ms: u64,
    // final_message_id
    final_message_id: Option<String>,
    // 循环变量
    iteration: u32,
    content: String,
    reasoning: String,
}
```

### 3.2 C4.2 两个 deadline 不混用

| 用途 | 来源 | 类型 |
|---|---|---|
| `elapsed` 与 `TurnState.turn_deadline` 比较（钩子判定） | `TurnState.turn_deadline = limits.turn_wall_clock_ms.map(Duration::from_millis)` | `Option<Duration>`（总时长上限） |
| 裁剪用的剩余墙钟 | `ctx.remaining_turn_budget.turn_deadline` | `Option<Instant>`（绝对截止时刻） |

```rust
// 钩子判定用：
let elapsed = ctx.clock.now().saturating_duration_since(ctx.turn_started_at);
// 裁剪用（剩余时长 = 截止 - 现在）：
let remaining_wall = ctx.remaining_turn_budget.turn_deadline
    .map(|d| d.saturating_duration_since(ctx.clock.now()));
```

### 3.3 C4.3-C4.5 中间件链 + 模型调用

```
// 2) before_model 阶梯 + 中间件
ctx.observer.set_phase(Thinking);
{
    let mut view = ModelRequestView {
        messages: &mut request.chat_messages,
        context_window_tokens: request.context_window_tokens,
        max_tokens: request.options.max_tokens,
        is_wrap_up_call: false,
    };
    if let StopTurn { reason } = chain.before_model(&mut view, &turn_ctx).await {
        return stopped(reason, previous_content);  // Completed，兜底用上一迭代已落库正文
    }
    request.context_window_tokens = view.context_window_tokens;
    request.options.max_tokens = view.max_tokens;
}  // view 借用到此结束

// 2.5) nudge 注入
for nudge in pending_nudges.drain(..) {
    request.chat_messages.push(nudge_message(nudge));
}

// 2.6) 本次请求快照
let request_with_tools = request.clone();

// 3) 模型调用（内层 loop 实现空响应重试）
ctx.observer.set_phase(Streaming);
last_request = request_with_tools.clone();
let stream = request_with_tools.stream(ctx.child_token.clone())?;
let r = consume(stream, ctx.child_token.clone(), model_call_budget, ctx.clock, request_started,
    |_seq, d| { accumulator.apply(d); ctx.observer.delta(d); }
).await;
model_calls += 1;
// 合并 token / usage（Ok/Err 两条路径分别处理）
```

### 3.4 C4.6 空响应重试

**判据**：`!has_effective_output && !accumulator.has_pending() && !is_output_limit_reached(finish_reason)`

**实现要点**：
- 用内层 `loop`（禁止用外层 `for` 的 `continue`，会递增 `iteration`）
- 重试不递增 `iteration`，但递增 `model_calls`
- 重发前 `accumulator.reset()`
- 首迭代：复用具名行以空内容 `finalize(Failed)`（不新增行）
- `iteration > 1`：不新增任何行，`final_message_id` 保持上一次写入值

**纯工具调用不触发重试**：无正文但有 `tool_calls` 是有效输出。

### 3.5 C4.6b 输出上限截断不重试

```rust
fn is_output_limit_reached(finish_reason: Option<&str>) -> bool {
    matches!(finish_reason, Some("length" | "max_tokens" | "MAX_TOKENS" | "max_output_tokens"))
}
```

截断命中时直接走 `Failed(EmptyResponse)`，`model_calls` 只加 1（A3）。

### 3.6 C4.6a 累积器补 reset/has_pending

在 `agent/tool_call.rs` 的累积器上补两个方法：

```rust
impl ToolCallAccumulator {
    pub fn reset(&mut self) { /* 清空本次 attempt 的累积状态，不重置 iteration */ }
    pub fn has_pending(&self) -> bool { /* 本 attempt 有工具调用片段（D22 空响应判据） */ }
}
```

### 3.7 C4.7-C4.8 收尾分支

**C4.7**：累积器 `finalize` 取本轮 `tool_calls`，无工具则 `Completed`。

**C4.8**：`iteration == max_iterations && spec.wrap_up` 时触发收尾：
1. 本轮 calls 一律不执行，但必须写配对结果行（`not_executed(calls, "已达迭代上限")`）
2. 撤 tools + 追加收尾指令（「预算已用尽，请基于已有结果给出结论，不要再调用工具」）
3. 收尾调用由 `call_model_without_tools` 执行（`is_wrap_up_call = true` 触发 ④ 还原）

### 3.8 C4.9-C4.11 预算内裁剪

**C4.9** `split_by_budget`：

```rust
fn split_by_budget(calls: Vec<FinalizedToolCall>, remaining_calls: u32,
                   remaining_clock: Option<Duration>) -> (Vec<FinalizedToolCall>, Vec<FinalizedToolCall>) {
    let to_run_count = calls.len().min(remaining_calls as usize);
    let to_run = calls[..to_run_count].to_vec();
    let skipped = calls[to_run_count..].to_vec();
    (to_run, skipped)
}
```

- 墙钟为 0 时整批并入 `skipped`
- 按原始顺序取前 `remaining_tool_calls` 个，其余进 `skipped`

**C4.10** `skipped` 结果形状：

```rust
ToolExecResult {
    is_error: true,
    stage: ToolStage::Gateway,
    error_kind: ToolErrorClass::Denied,
    content: "未执行：本轮工具调用预算已用尽（max_tool_calls = N）".to_string(),
    // ... 其他字段按正常结果填
}
```

**C4.11**：`skipped` 不参与 `after_tool` 中间件、不进 D19 计数、不进 D25 分类预算，但计入 `tool_calls`（D18 按发起数计）。

### 3.9 C4.16-C4.17 审批计时

`TimingApprovalPrompter` 装饰器已实现（C4.14），适配器（C4.15）在 `pipeline.execute` 返回后按 `tool_call_id` 取出等待时长并 `remove`。

**C4.16**：未走审批（`Allow` 直通）时 `confirmation_wait_ms` 为 `None`（`Option<u64>`），executor 累加时过滤：

```rust
confirmation_wait_ms += results.iter()
    .filter_map(|r| r.confirmation_wait_ms)
    .sum::<u64>();
```

**C4.17**：`RoundOutcome.confirmation_wait_ms` 直接取上述累加值（已在 `finish()` 中装配）。

### 3.10 C4.19 取消打标

turn 取消时整批结果标 `Cancelled`，content 写「已取消」。Pipeline 侧取消路径已返回 `is_error = true` + `error_kind = Cancelled`，executor 侧在取消分支处理。

### 3.11 C4.20-C4.20d 行级落库

#### C4.20 `append_tool_results`

对每个结果：内存 `request.messages` 追加一条 `role='tool'`（`tool_call_id` / `name` / `is_error` / content）+ `store.append_tool_message(parent = last_row_id)`，每写成功一条即回写 `last_row_id`（首条挂本迭代 assistant 行，后续逐条链式追加）。两份文本逐字节一致（D21/D11）。

返回最后一条 tool 行 id；函数内已同步回写 `last_row_id`，调用方不再传入 `parent_id`。

#### C4.20a `ensure_row` 幂等

```rust
fn ensure_row() -> String {
    if iteration > start_iteration && !row_created {
        msg_id = ctx.store.create_assistant_message(CreateAssistantReq {
            conversation_id: ctx.conversation_id.to_string(),
            parent_id: Some(last_row_id.to_string()),
            provider_id: ctx.provider_id.map(str::to_string),
            model_id: ctx.model_id.map(str::to_string),
        }).await?;
        last_row_id = msg_id.clone();
        row_created = true;
    }
    msg_id
}
```

- `iteration == start_iteration`（P4 恒为 1）复用 `ctx.initial_message_id`
- 每迭代重置 `row_created = false`
- 建行成功后必须回写 `last_row_id = msg_id`：否则下一迭代的 tool 行仍挂上一迭代末行，链式追加会断在「迭代 2 首个 tool 行」

#### C4.20b 错误/取消/超时路径

先 `ensure_row()` 再 `persist_row(msg_id, partial_content, [], 消息状态)`：
- 取消 → `Interrupted`
- 超时/Provider 错误 → `Failed`
- **不得**复用上一迭代 `msg_id`（会覆盖已 finalize 的行）
- `final_message_id = Some(msg_id)`

#### C4.20c 落库顺序

每迭代固定顺序：`create → save_content → record_usage → save_tool_calls → finalize`

usage 由 executor 逐调用写（`message_id` = 本次行），`TurnPersister` 不再写 usage。

#### C4.20d 请求历史只投影一次

第 0 步 `request::build` 只投影一次 DB → `Vec<ChatMessage>`。后续迭代只在内存追加，禁止每迭代重读 DB。

### 3.12 C4.21-C4.21b after_tool 阶梯

**C4.21** 阶梯顺序固定：D26（豁免）→ D25（分类预算）→ D19（通用 2/3 阶梯）。

`after_tool` 的 `StopTurn` **不立即返回**：先记 `halt_reason`，落完本迭代全部 tool 行后再 `halted_with_summary`（否则下一轮 wire 缺配对结果）。

```rust
let mut halt_reason = None;
for r in results.iter_mut() {
    let m = meta.get(r.call_id.as_str()).unwrap();
    let mut view = ToolResultView {
        result: r,
        arguments: m.arguments.as_str(),
        argument_fingerprint: m.fingerprint.as_str(),
        scope: m.scope.as_str(),
    };
    if let StopTurn { reason } = chain.after_tool(&mut view, &turn_ctx).await {
        halt_reason = halt_reason.or(Some(reason));
    }
}
append_tool_results(results ++ skipped);   // 内部逐条推进 last_row_id，无需接收 msg_id
tool_calls += n;
```

**C4.21a** StopTurn 路由：

| 触发阶段 | 出口函数 | TurnStatus |
|---|---|---|
| `before_model` | `stopped(reason, previous_content)` | `Completed` |
| `after_tool` / `after_iteration` | `halted_with_summary(reason)` | `Failed` |

**C4.21b** nudge 传递：

`pending_nudges` 由 `chain.take_pending_nudges()` 在每次迭代末取出并暂存，下一轮 2.5 注入。不进 `messages` 表（D19）。

### 3.13 C4.22-C4.22d 双 Halt 出口

#### C4.22 `halted_with_summary`

由 `after_tool` / `after_iteration` 的 `StopTurn` 触发：

```rust
fn halted_with_summary(reason: String) -> RoundOutcome {
    let summary = progress_summary_reason(&reason);
    // reason 已是中间件给出的中文完整句（含工具名/次数/末次原因），原样使用
    persist_final_text(summary.clone(), Failed);
    finish(Failed, summary, String::new(), false, false,
           Some(TurnError::NoProgress { reason }))
}
```

#### C4.22a `halted_by_budget`

由第 1 步三维刹车触发：

```rust
fn halted_by_budget(hook_name: &str, reason: String) -> RoundOutcome {
    if spec.wrap_up {
        let wrapped = call_model_without_tools(request, wrap_up_instruction);
        match wrapped {
            Ok(w) => {
                persist_final_text(w.text.clone(), Completed);
                finish(Completed, w.text, w.reasoning, true, true, None)
            }
            Err(e) => {
                let summary = budget_summary_reason(hook_name);
                persist_final_text(summary.clone(), Failed);
                finish(Failed, summary, String::new(), true, true, Some(e))
            }
        }
    } else {
        let summary = budget_summary_reason(hook_name);
        persist_final_text(summary.clone(), Completed);
        finish(Completed, summary, String::new(), true, false, None)
    }
}
```

#### C4.22b 收尾调用失败

收尾调用失败/超时 → `Failed`，`text` 退回根因摘要 `persist_final_text(summary, Failed)`。收尾调用若已建行则沿用该行 `finalize(Failed)`，不重复建。

#### C4.22c `persist_final_text` 写库失败

不得吞掉 DB 错误：`status` 按原终态返回，`error = Some(TurnError::Persist(...))`。

#### C4.22d 循环自然走完的兜底

`wrap_up = false` 且最后一轮仍有工具调用时 `content` 可能为空。必须用 `budget_summary_reason("iteration_limit")` + `persist_final_text(summary, Completed)` + `hit_cap = true`，不得返回空文本。

### 3.14 C4.23 取消令牌包裹

所有长 await 必须包在 `tokio::select!` 中：

```rust
tokio::select! {
    biased;
    result = long_async_operation() => { /* 处理结果 */ }
    _ = token.cancelled() => { /* 取消处理 */ }
}
```

### 3.15 C4.39 适配器上下文构造

每次 `execute` 现构造 `ToolUseContext` 与 `ResolvedPermissionPolicy`：

| 参数 | P4 取值 | 来源 |
|---|---|---|
| `workspace_root` | 会话绑定的工作区 | `src-tauri` 从会话行读取 |
| `session_id` | `Some(conversation_id)` | `TurnRunCtx.conversation_id` |
| `tool_call_id` | 逐调用 `FinalizedToolCall.id` | 每次调用现填 |
| `ResolvedPermissionPolicy` | `default_global()` | 模块常量 |
| `ToolResultStorage.root` | `app_data_dir()/tool-results/` | setup.rs 构造一次 |

### 3.16 C4.40-C4.40c 落库辅助函数

#### C4.40 函数清单（不得出现第三条写库路径）

| 函数 | 动作 | 不做什么 |
|---|---|---|
| `ensure_row()` | 返回本迭代 assistant 行 id（第 1 迭代复用占位行，其余建新行 `parent = last_row_id`） | 不写内容/usage/finalize |
| `persist_row(msg_id, content, tool_calls, status)` | `save_content` → `record_usage` → `save_tool_calls`（非空时）→ `finalize`；置 `final_message_id` 与 `row_finalized` | 不建行 |
| `persist_row_as_new(content, status)` | 无条件新建 assistant 行（`parent = last_row_id`），再按上述顺序落库 | 不复用当前 `msg_id` |
| `persist_final_text(text, status)` | 择行：`row_finalized == false` → `persist_row(ensure_row(), text, status)`；`row_finalized == true` → `persist_row_as_new(text, status)` | 不产生两条行 |
| `append_tool_results(results)` | 对每个结果追加内存消息 + `store.append_tool_message(parent = last_row_id)`（逐条推进 `last_row_id`）；返回最后一条 tool 行 id | 不发事件 |
| `not_executed(calls, reason)` | 构造 skipped 形状（`is_error = true` / `Gateway` / `Denied`） | 不执行 |
| `split_by_budget(calls, remaining_calls, remaining_clock)` | 按原始顺序切分 `to_run` / `skipped` | 不改顺序 |

#### C4.40a 每迭代重置

```rust
row_created = false;
row_finalized = false;
retries_used = 0;
accumulator.reset();
```

#### C4.40b 错误路径 tool_calls = []

错误/取消/超时路径的 `persist_row` 传 `tool_calls = []`（部分累积的调用不完整，落库会让 wire 出现无配对结果的 `tool_call`）。

#### C4.40c usage 只写一次

`persist_row_as_new` 仅当该 attempt 的 usage 尚未落库时才 `record_usage`。`stopped` / `halted_with_summary` 无新 attempt 时不写 usage。

---

## 4 C5 TurnRunner 切换（C5.1-C5.5）

### C5.1 切到 executor

`TurnRunner` 的 `run_turn` 方法中，把 P3 的单迭代循环替换为：

```rust
let outcome = executor.run_iterations(ctx).await;
```

### C5.2 删除过渡循环

删除 `kernel/runner.rs` 中 P3 的过渡单迭代循环体。**本项与 C1.13a-C1.13c 同批交付**（C1 删 `StreamOutcome.tool_calls` 后过渡循环即不可编译）。

### C5.3 一次性替换

不保留运行时双路径开关（不由 feature flag / config 控制用旧循环还是新 executor）。

### C5.4-C5.5 测试

- chat 路径事件序列与 P3 对比一致（A9）
- chat 路径无 tool 消息、无 `tools` 注入（A9）

---

## 5 C7 端到端测试（C7.1-C7.17）

测试文件：`src-tauri/tests/agent_loop_e2e.rs`、`src-tauri/tests/agent_loop_dialect.rs`

| 任务 | 验收 |
|---|---|
| C7.1 3 迭代闭环 + tool 落库往返 | A1 / A11 |
| C7.2 收尾调用含 `hit_cap` / `wrap_up_injected` | A2 |
| C7.3 空响应重试仍空 → Failed，messages 无空白行 | A3 / A15 |
| C7.4 取消路径 | A4 |
| C7.5 工具失败回填后继续；Denied 不重试 | A5 |
| C7.6 三维预算与预算内裁剪 | A12 / A12b |
| C7.7 nudge/halt 与 messages 无新增行 | A13 |
| C7.8 并行批次墙钟与顺序 | A14 |
| C7.9 收尾还原（摘要计数断言不在本批） | A16 |
| C7.10 失败分类阶梯与成功清零 | A17 |
| C7.11 端口镜像不漂移 | A18 |
| C7.12 回填方言 | A19 |
| C7.13 累积器唯一实现 | A7 |
| C7.14 行链与 active_branch_id | A20 |
| C7.15 双 Halt 出口 | A21 |
| C7.16 错误路径建行 | A22 |
| C7.17 delta seq 单调与旧代际丢弃 | A23 |

---

## 6 C8 验收命令（C8.1-C8.13）

`cwd = src-tauri`

| 任务 | 命令 |
|---|---|
| C8.1 | `cargo test -p assistant-agent` 全绿 |
| C8.2 | `cargo test -p assistant-providers` 全绿 |
| C8.3 | `cargo test -p assistant-storage` 全绿 |
| C8.4 | `cargo test -p assistant-app --test protocol_dto_roundtrip` 通过 |
| C8.5 | `cargo test --test assembly_contract` 通过 |
| C8.6 | `cargo test -p assistant-app --test agent_loop_e2e` 与 `--test agent_loop_dialect` 通过 |
| C8.7 | `cargo clippy -p assistant-agent -p assistant-providers -p assistant-storage --all-targets -- -D warnings` 无告警 |
| C8.8 | 先执行格式化修复，再 `cargo fmt --all --check` 通过 |
| C8.9 | `cargo tree -p assistant-agent` 无 `tauri`、无 `assistant-tools` |
| C8.10 | grep 确认 `agent/` 六文件无 `AppHandle` / `Window` / `assistant_tools` |
| C8.11 | `npm run contract:check` 幂等（无 diff） |
| C8.12 | `cargo test --workspace --no-run` 全量编译通过 |
| C8.13 | 确认未改 `assistant-tools` 任何文件 |

---

## 7 C9 文档（C9.1-C9.3）

- C9.1 更新 roadmap §2：P4 标记已完成并记录日期
- C9.2 在 tasklist 勾选 C1-C8 完成项
- C9.3 确认 I38b 规格与 I36 spec 无进度表述

---

## 附录 A：循环伪码（来自主规格 §5，完整定义）

```
run_iterations(ctx) -> RoundOutcome:

  // 0) 请求历史只投影一次（后续迭代在内存上追加，见 §5.1）
  //    `request::build` 内部按 `AgentKind::supports_tool_call()` + `ctx.tools` 把工具注入 `options.tools`，
  //    且 `catalog.agent_tools()` 只在 build 内取一次；executor **不得**再取第二份工具全集（会与 build 的注入分叉）
  let start_iteration = ctx.iteration          // 起点迭代号（P4 恒 1），见符号表
  let mut request = match request::build(ctx.input, start_iteration, ctx.tools).await {
      Ok(r) => r,
      Err(e) => return finish(Failed, "", "", false, false, Some(TurnError::Other(e))),   // 投影 / 提示词构建失败：无行可写，final_message_id = None
  }
  let mut last_request = request.clone()      // 最后一次真正发给 provider 的请求（装配 outcome 用）
  let mut msg_id = ctx.initial_message_id     // 第 1 迭代复用命令层预建的占位行
  let mut row_created = false                 // ensure_row 的幂等标记（每迭代重置）
  let mut row_finalized = false               // 本迭代是否已写入并 finalize assistant 行（persist_final_text 择行用）
  let mut last_row_id = msg_id                // 下一迭代新行的 parent（= 上一条 tool 行）
  let mut accumulator = ToolCallAccumulator::new()   // agent/tool_call.rs，跨迭代复用；每次 attempt 前 reset()
  let mut chain = MiddlewareChain::p4(ctx.limits.microcompact_keep_recent)   // 中间件链的唯一实例（顺序固定）
  let mut pending_nudges: Vec<String> = vec![]       // 只进下一次请求，不落库（D19）
  let mut previous_content = String::new()    // 上一迭代已落库的正文，仅 `stopped` 兜底用
  let mut model_calls = 0u32                  // 每次 provider 调用 +1（含空响应重试与收尾调用）
  let mut tool_calls = 0u32                    // 已发起的工具调用总数（D18），含 skipped；`TurnState.tool_calls` 同源
  let mut token_stats = TokenStats::default()  // prompt / completion / cache_read / total / ttft
  let mut attempt_usages: Vec<UsageSnapshot> = vec![]   // 每 attempt 一条，供 `RoundOutcome.attempt_usages`
  let mut final_message_id: Option<String> = None
  let mut confirmation_wait_ms = 0u64         // 逐次 `ToolExecResult.confirmation_wait_ms` 累加（D12）
  let mut iteration = start_iteration         // 最后一次实际进入的迭代号（`finish` 的 `iterations` 来源）
  let mut content = String::new()             // 最后一次有效输出的正文（循环外「迭代上限出口」要读）
  let mut reasoning = String::new()           // 同上，推理正文
  let hooks = ctx.spec.stop_hooks.as_slice()   // 三维预算钩子集合（`TurnSpec::agent()` 装配，`chat()` 为空）

  // ensure_row：本迭代的行，幂等（同一 iteration 只建一次）
  //   `iteration == start_iteration`（P4 恒为 1）复用命令层预建占位行；
  //   其余迭代在模型返回后（成功或失败）才建
  fn ensure_row() -> msg_id:
      if iteration > start_iteration && !row_created:
          msg_id = ctx.store.create_assistant_message(CreateAssistantReq {
              conversation_id: ctx.conversation_id.to_string(),
              parent_id: Some(last_row_id.to_string()),
              provider_id: ctx.provider_id.map(str::to_string),   // `Option<&str>` → `Option<String>`
              model_id: ctx.model_id.map(str::to_string),
          }).await?                    // 实现内同步 conversations.active_branch_id（§6.2）
          last_row_id = msg_id.clone() // 关键：不写回会让下一迭代 tool 行挂错父节点
          row_created = true
      return msg_id

  for it in start_iteration..=ctx.limits.max_iterations:
    iteration = it                              // 循环变量写入外层可变量，供循环外 `finish` 装配
    // 1) 刹车先于花钱（D18：三维预算在此一并判）
    //    注意两个 deadline 的类型差异（§2.2）：`TurnState.turn_deadline` 是**总时长上限**（Duration），
    //    `ctx.remaining_turn_budget.turn_deadline` 是**绝对截止时刻**（Instant）
    let elapsed = ctx.clock.now().saturating_duration_since(ctx.turn_started_at)
    if let Some((hook_name, reason)) = stop_hooks_all_continue(&hooks, TurnState {
        iteration, max_iterations: ctx.limits.max_iterations, tool_calls,
        max_tool_calls: ctx.limits.max_tool_calls,
        prompt_tokens: token_stats.prompt.unwrap_or(0),
        completion_tokens: token_stats.completion.unwrap_or(0),
        elapsed,
        turn_deadline: ctx.limits.turn_wall_clock,
        model: &ctx.model,
    }):
        // `stop_hooks_all_continue` 返回 `Option<(&'static str, String)>`（`stop.rs` 现有签名）：
        // 第 1 项是钩子名（`iteration-limit` / `tool-call-limit` / `turn-wall-clock` / `token-budget`），
        // 第 2 项是钩子自带的原因文本
        return halted_by_budget(hook_name, reason)   // 预算档 Halt：wrap_up 为真走收尾调用（§5.2），否则根因摘要文本（Completed + hit_cap）

    // 1.5) 本迭代 assistant 行在模型返回后才建（第 4.5 步或错误路径）：
    //      调用前建行会让空响应重试留下空白行（D22 / A15）
    row_created = false
    row_finalized = false
    retries_used = 0                       // 重试预算按迭代计（D22）
    accumulator.reset()
    let turn_ctx = TurnCtx { turn_id: ctx.turn_id, conversation_id: ctx.conversation_id,
                             iteration, max_iterations: ctx.limits.max_iterations, model: ctx.model }

    // 2) before_model：阶梯 + 中间件（顺序固定，见 I36 spec Step 4.2）
    //    `chain` 由 `MiddlewareChain::p4(ctx.limits.microcompact_keep_recent)` 构造，七项顺序由常量数组固定；
    //    `spec.ladder = ProjectionOnly`（chat）时 ③④⑤ 因无工具结果自然 no-op（chat 不注入 tools），executor 不做分支
    //    `stopped()` 的 text 兜底用 `previous_content`（上一迭代的正文，第 1 迭代为空串）
    //    阶梯只改「本次请求」的消息副本（`ModelRequestView.messages` 借用 `request.chat_messages`），不写回历史
    ctx.observer.set_phase(Thinking)
    let (mut window, mut max_tokens);
    {
        let mut view = ModelRequestView {
            messages: &mut request.chat_messages,          // ①③④⑤ 在此原地改写本次请求
            context_window_tokens: request.context_window_tokens,
            max_tokens: request.options.max_tokens,
            is_wrap_up_call: false,
        };
        if let StopTurn { reason } = chain.before_model(&mut view, &turn_ctx).await:
            return stopped(reason, previous_content)
        window = view.context_window_tokens;               // 阶梯可下调有效窗口 / 输出预留（硬裁剪后），必须写回
        max_tokens = view.max_tokens;
    }                                                      // view 借用到此结束，才能再动 request
    request.context_window_tokens = window
    request.options.max_tokens = max_tokens

    // 2.5) nudge 注入（D19，必须排在阶梯之后）
    for nudge in pending_nudges.drain(..):
        request.chat_messages.push(nudge_message(nudge))   // 只进本次请求，不落库

    // 2.6) 本次请求快照（`request_with_tools` 的唯一赋值点）：
    //      tools 已在第 0 步由 `request::build` 写进 `options.tools`，此处**只 clone**；
    //      不得再拼 tools（重复注入会让 chat 意外带工具，A9 失败），也不得改 `options.tools` 之外的字段
    let request_with_tools = request.clone()

    // 3) 单次模型调用（重试用内层 loop：外层 for 的 continue 会递增 iteration，违反 D22 / A3）
    ctx.observer.set_phase(Streaming)
    let mut has_effective_output = false
    let mut finish_reason: Option<String> = None    // 末次 attempt 的结束原因（D22 输出上限判据）
    content.clear()                      // 正文（末次 attempt 的值；重试前 reset 已丢弃旧 attempt）
    reasoning.clear()                    // 推理正文
    loop {
        last_request = request_with_tools.clone()    // 装配 outcome 的 context_window / reserved 用最后一次真实请求
        let request_started = ctx.clock.now()        // 单次调用的超时预算与 TTFT 都从这一刻起算
        let model_call_budget = effective_model_call_budget(ctx, request_started)   // `limits.model_call` 与 turn 剩余取小
        let stream = match request_with_tools.stream(ctx.child_token.clone()) {     // `ChatRequest::stream`，不直接用 adapter
            Ok(s) => s,
            Err(e) => {                              // 连接建立即失败：无任何内容，仍需收尾本迭代行
                persist_row(ensure_row(), "", [], Failed)
                return finish(Failed, "", "", false, false, Some(TurnError::Provider(e.to_string())))
            }
        }
        let r = kernel::stream::consume(
            stream, ctx.child_token.clone(), model_call_budget, ctx.clock, request_started,
            |_seq, d| { accumulator.apply(d); ctx.observer.delta(d); },   // 回调签名 `FnMut(u32, &ChatDelta)`
        ).await
        model_calls += 1
        // `StreamOutcome` 才有分项 token；`PartialOutcome` 只有 `attempt_usages` / `ttft_ms`，两条路径合并方式不同
        match &r {
            Ok(o) => { token_stats.merge(o); attempt_usages.extend(o.attempt_usages.clone()) }
            Err(partial) => { token_stats.merge_usage(partial.last_usage());      // 失败路径已上报的 usage 也计入
                              token_stats.ttft_ms = token_stats.ttft_ms.or(partial.ttft_ms);
                              attempt_usages.extend(partial.attempt_usages.clone()) }
        }
        // 注意两种 status：`persist_row` 的第 4 参是**消息落库状态**（`MessageStatus`），
        // `finish` 的第 1 参是**turn 状态**（`TurnStatus`）。映射沿用 P3 `message_status_of`：
        // Cancelled → Interrupted；Timeout / Provider → Failed。两者不可直接互抄。
        match r:
          Err(Cancelled(partial))   -> { persist_row(ensure_row(), partial.full_content, [], Interrupted)
                                    return finish(Cancelled, partial.full_content, partial.full_reasoning, false, false, None) }
          Err(Timeout(partial))     -> { persist_row(ensure_row(), partial.full_content, [], Failed)
                                    return finish(Failed, partial.full_content, partial.full_reasoning, false, false, Some(TurnError::Timeout)) }
          Err(Provider(e, partial)) -> { persist_row(ensure_row(), partial.full_content, [], Failed)
                                    return finish(Failed, partial.full_content, partial.full_reasoning, false, false, Some(TurnError::Provider(e.to_string()))) }
          Ok(o) -> { has_effective_output = o.has_effective_output; content = o.full_content; reasoning = o.full_reasoning;
                     finish_reason = o.finish_reason.clone() }

        // 4) 空响应重试（不递增 iteration、递增 model_calls；不建行、不写内容，D22）
        //    "空" = 无正文（has_effective_output = false）且本 attempt 无工具调用片段（accumulator.has_pending() = false）。
        //    纯工具调用（无正文、有 tool_calls）是**有效输出**，不得触发重试 —— 否则 agent 循环在每次工具调用前就被判失败。
        //    推理单独出现（有 reasoning 无正文）仍算空（A3）。
        //    被输出上限截断（`is_output_limit_reached(finish_reason)`）**不重试**：同一 max_tokens 重发必然同样截断，
        //    只会白花一次调用（D22 第 2 条）；直接落入 4.1 的 `Failed(EmptyResponse)`。
        if !has_effective_output && !accumulator.has_pending()
           && !is_output_limit_reached(finish_reason.as_deref())
           && retries_used < ctx.limits.empty_response_retries:
            retries_used += 1
            accumulator.reset()        // 丢弃本次 attempt 的增量，避免与重发结果叠加
            continue                   // 内层重发，iteration 不变
        break
    }

    let calls = accumulator.finalize(iteration)   // 空 = 纯文本结束

    // 4.1) 重试用尽仍空 → Failed（D22：不新增行；首迭代复用占位行并以空内容 finalize）
    //     判据：`!has_effective_output && calls.is_empty()`（`finalize` 之后以 calls 为准）
    if !has_effective_output && calls.is_empty():
        if iteration == start_iteration:
            persist_row(ctx.initial_message_id, "", [], Failed)   // 占位行本就在，不能删，只能收尾
        // final_message_id：首迭代即占位行 id；后续迭代保持上一次写入值
        // （此前从未写过任何 assistant 行才为 None，A15）
        return finish(Failed, "", "", false, false, Some(TurnError::EmptyResponse))

    // 4.5) 落库本次调用（唯一写方，见 §5.1）：建行 → 正文/推理 → usage → tool_calls → finalize
    let msg_id = ensure_row()
    persist_row(msg_id, content, calls, Completed)   // 展开见 §5.1 的表；content 含推理，usage 逐调用一条
    previous_content = content.clone()               // 供下一迭代 `before_model` 的 `stopped()` 兜底

    // 5) 无工具 → 结束（chat 单迭代即在此返回，等价 P3）
    if calls.is_empty(): return finish(Completed, content, reasoning, false, false, None)

    let n = calls.len() as u32      // 按发起数计（D18）

    // 6) 最后一轮：收尾（仅 spec.wrap_up；撤回 tools + 追加收尾指令 + 还原被微压缩清空的结果，D23）
    if ctx.spec.wrap_up && iteration == ctx.limits.max_iterations:
        // 本轮 calls 一律不执行，但必须写配对结果行，否则收尾请求 wire 非法（§5.3 同形）
        tool_calls += n                                   // 未执行也算「发起」（D18 按发起数计）
        append_tool_results(not_executed(calls, "已达迭代上限"))
        // 收尾调用的 ④ 还原由 `is_wrap_up_call = true` 触发（见 §5.2 的 `call_model_without_tools`），
        // executor 不再单独算 `restore_allowance`，也不在调用点拼 `wrap_up_restore`
        let wrapped = call_model_without_tools(request, wrap_up_instruction)
        if wrapped.is_ok:
            persist_final_text(wrapped.text, Completed)   // 本迭代行已 finalize → 收尾结果独立一条 assistant 行（parent = last_row_id）
            return finish(Completed, wrapped.text, wrapped.reasoning, true, true, None)
        else:
            let summary = budget_summary_reason("iteration-limit")   // 收尾失败/超时：退回根因摘要，不吞错误（§5.2）
            persist_final_text(summary, Failed)
            return finish(Failed, summary, "", true, true, Some(wrapped.error))

    // 6.5) 预算内裁剪（§5.3）：超预算/超时的调用不执行，但仍写 tool 行
    //      额度在此现算（此时 tool_calls 尚未计入本次 n，故 remaining 含本次全部发起数）
    let remaining_tool_calls = ctx.limits.max_tool_calls.saturating_sub(tool_calls)
    let remaining_wall_clock = ctx.remaining_turn_budget
        .turn_deadline.map(|d| d.saturating_duration_since(ctx.clock.now()))   // 绝对截止时刻 → 剩余
    let (to_run, skipped) = split_by_budget(calls, remaining_tool_calls, remaining_wall_clock)

    // 7) 工具执行（ToolExecutionPort）
    ctx.observer.set_phase(ToolCalling)
    let tool_exec = ctx.tool_exec.expect("calls 非空时必有 ToolExecutionPort（TurnSpec 装配保证）")
    let exec_ctx = ToolExecCtx {                       // 每次 execute 现构造；不含权限类型（§4.5）
        turn_id: ctx.turn_id, conversation_id: ctx.conversation_id,
        iteration, child_token: ctx.child_token, tools: ctx.tools, clock: ctx.clock,
    }
    let mut results = tool_exec.execute(to_run, &exec_ctx).await
    confirmation_wait_ms += results.iter().filter_map(|r| r.confirmation_wait_ms).sum()   // Option 求和；Allow 直通为 None
    // 参数与指纹由 executor 从本迭代的 FinalizedToolCall 现算并按 call_id 关联（端口结果不携带参数，D25）；
    // 只对**实际执行**的 results 走 after_tool，skipped 不进任何阶梯（§5.3）
    let meta: HashMap<&str, ToolCallMeta> = calls.iter()
        .map(|c| (c.tool_id.as_str(), ToolCallMeta::from_finalized(c)))
        .collect()
    let mut halt_reason = None
    for r in results.iter_mut():
        let m = meta.get(r.call_id.as_str()).expect("端口结果必来自本迭代的 calls")
        let mut view = ToolResultView {
            result: r,
            arguments: m.arguments.as_str(),                    // 原始 arguments 文本（非法时 `{}`）
            argument_fingerprint: m.fingerprint.as_str(),        // 同参失败判定（D19）
            scope: m.scope.as_str(),                             // 分类预算键 (class, tool, scope)（D25）
        };
        // after_tool 的 StopTurn **不立即返回**：本迭代已执行的工具必须先落 tool 行，
        // 否则下一轮 wire 缺配对结果会 400（§5.3）。先记原因，全部落库后再走无进展 Halt。
        if let StopTurn { reason } = chain.after_tool(&mut view, &turn_ctx).await:
            halt_reason = halt_reason.or(Some(reason))
    append_tool_results(results ++ skipped)   // 回填内存历史 + 写 role='tool' 行（D11），内部逐条推进 last_row_id
    tool_calls += n                                                // 按发起数计（D18），含 skipped

    // 8) 迭代后钩子（D26 → D25 → D19 的顺序由中间件链内部固定，executor 不重排）
    pending_nudges.extend(chain.take_pending_nudges())   // nudge 只进下一轮 2.5，不落库
    if halt_reason.is_none():
        if let StopTurn { reason } = chain.after_iteration(&turn_ctx).await: halt_reason = Some(reason)
    if let Some(reason) = halt_reason:
        return halted_with_summary(reason)   // 无进展 Halt → Failed（A21），不复用 stopped

  // 循环正常走完（`wrap_up = false` 且最后一轮仍发起了工具调用）：等价预算档的迭代上限出口
  if content.is_empty():
      let summary = budget_summary_reason("iteration-limit")   // 不得返回空文本（A21）
      persist_final_text(summary, Completed)
      return finish(Completed, summary, "", true, false, None)
  return finish(Completed, content, reasoning, true, false, None)
```

**伪码符号说明（避免实现时自行猜测取值来源）**：

| 符号 | 定义 |
|---|---|
| `request` | 第 0 步 `request::build` 的产物 `ChatRequest`（`chat_messages: Vec<ChatMessage>` / `options: ChatOptions`（含 `max_tokens`、`tools`）/ `model` / `context_window_tokens`）；后续迭代只在内存追加 |
| `start_iteration` | `let start_iteration = ctx.iteration`（`TurnRunCtx.iteration` 是**起点迭代号**，P4 恒为 `1`）；伪码用独立名字只为与循环变量 `iteration` 区分 |
| `request_with_tools` | 第 2.6 步 `request.clone()` 的快照：tools 已由 `request::build` 写进 `options.tools`，此处**不再拼 tools**；收尾调用由 `call_model_without_tools` 自行置 `options.tools = None` |
| `effective_model_call_budget(ctx, request_started)` | 单次模型调用预算：`limits.model_call` 与「turn 绝对截止时刻 - `request_started`」取小，两者都 `None` 时 `None`；不继承 turn 预算的语义（I36 design D4） |
| `clock` | `ctx.clock`（`kernel::ports::Clock` 注入端口），executor 只读当前时间，不持有计时器 |
| `exec_ctx` | `ToolExecCtx`（§4.5）：`turn_id` / `conversation_id` / `iteration` / `child_token` / `tools: Option<&'a Arc<dyn ToolCatalog>>`（与 `ctx.tools` 同型，直接透传）/ `clock`；**不含** `ToolUseContext` / `ResolvedPermissionPolicy`（§4.2）、也不含 `TurnObserver` |
| `token_stats` | P4 新增的内部簿记（不落库、不进 wire）：`prompt` / `completion` / `cache_read` / `total`: `Option<u32>`，`ttft_ms: Option<u64>`；`merge(&StreamOutcome)` / `merge_usage(UsageSnapshot)` 只累加、不覆盖缺失值。写 `TurnState` 的 `u32` 字段时 `unwrap_or(0)` |
| `last_request` | 最后一次真正发给 provider 的请求（含收尾调用）；用于装配 `context_window_tokens` / `reserved_output_tokens` |
| `ctx.turn_started_at` | turn 起点的 `Instant`（TurnRunner 侧的 `started_at` 原值下发），只用于算 `elapsed`（§2.2） |
| `ctx.model` | 本次 turn 使用的模型名（`TurnState.model` 的唯一来源）；executor 不自行从 provider 行推断 |
| `previous_content` | 上一迭代**已落库**的正文；仅 `stopped()` 的文本兜底用（第 1 迭代为空串） |
| `stop_hooks_all_continue(hooks, state)` | `agent/stop.rs` 现有函数，签名 `(&[Arc<dyn StopHook>], &TurnState) -> Option<(&'static str, String)>`；返回**钩子名 + 原因**，`None` 表示全部 `Continue`。`hooks` 取 `ctx.spec.stop_hooks`（见下条） |
| `budget_summary_reason(hook_name)` | 把**钩子名**映射成用户可读的根因摘要（`iteration-limit` → 「已达迭代上限」/ `tool-call-limit` → 「已达工具调用上限」/ `turn-wall-clock` → 「本轮已超时」/ `token-budget` → 「本轮 token 预算已用尽」；未知名回落通用文本），文本必须非空 |
| `progress_summary_reason(reason)` | 无进展出口的**用户可见**文本：`reason` 已是中间件给出的完整句（D19/D25 的 `halt_summary` 含工具名、连续失败次数与末次原因）时**原样使用**，不再拼接次数、不加重试提示；`reason` 为空时回落固定文本「本轮已停止：工具调用未取得进展」。文本必须非空，且**不得含中间件内部标识**（见 `chain` 条） |
| `hooks` | `ctx.spec.stop_hooks`（`TurnSpec::agent()` 按 `limits` 装配 `IterationLimitHook` / `ToolCallLimitHook` / `TurnWallClockHook`，配了 token 预算才加 `TurnTokenBudgetHook`；`TurnSpec::chat()` 为空 `Vec`，单迭代由 `max_iterations = 1` 保证）；executor 只读，不重建 |
| `retries_used` | 本迭代已用掉的空响应重发次数，每迭代开头清零（D22 的 `empty_response_retries` 按迭代计） |
| `chain` | `MiddlewareChain`（`agent/middleware.rs`）：`MiddlewareChain::p4(ctx.limits.microcompact_keep_recent)` 构造（七项顺序由 `P4_MIDDLEWARE_ORDER` 常量固定）；`before_model(&mut ModelRequestView, &TurnCtx)` / `after_tool(&mut ToolResultView, &TurnCtx)` / `after_iteration(&TurnCtx)` 三者均 `async` 且返回 `MiddlewareOutcome`，`take_pending_nudges()` 同步；executor 不重排、不自行构造第二份。**`StopTurn` 必须返回结构化 `{ middleware: &'static str, reason: String }`**（现状 `format!("{}: {}", mw.name(), reason)` 会把内部标识拼进用户可见文本）：`middleware` 只进 `log::warn!` 与遥测，`reason` 才是落库正文的来源 |
| `turn_ctx` | `TurnCtx { turn_id, conversation_id, iteration, max_iterations, model }`（字段均为借用 `&'a str` / `u32`，与 `middleware.rs` 定义 1:1）；每迭代开头构造一次，三阶段共用 |
| `view`（`before_model`） | `ModelRequestView { messages: &mut request.chat_messages, context_window_tokens, max_tokens, is_wrap_up_call }`；阶梯只改这个视图，`view` 借用的作用域结束后必须把 `context_window_tokens` / `max_tokens` 写回 `request` |
| `view`（`after_tool`） | `ToolResultView { result: &mut ToolExecResult, arguments, argument_fingerprint, scope }`；三字段由 `meta` 表按 `call_id` 查得，端口结果本身不携带 |
| `meta: HashMap<&str, ToolCallMeta>` | 由本迭代 `calls`（`Vec<FinalizedToolCall>`）按 `tool_id` 建的查找表；`ToolCallMeta` 提供 `arguments`（原始 JSON 文本，解析失败时 `{}`）/ `fingerprint`（参数指纹，用于「同参失败」判定 D19）/ `scope`（D25 分类预算的 `(class, tool, scope)` 键的 scope 段） |
| `nudge_message(text)` | 构造一条仅存在于内存请求的 `ChatMessage`（不写库、不带 `tool_call_id`）；收尾指令同用此函数 |
| `is_output_limit_reached(finish_reason: Option<&str>)` | 判定 `finish_reason` 是否为输出上限截断：`"length"` / `"max_tokens"` / `"MAX_TOKENS"` / `"max_output_tokens"` → `true`，其余（含 `None`）→ `false`。对应 goose 的 `metadata.output_token_limit_reached`；截断不是「模型没说话」，重发无意义（D22） |
| `call_model_without_tools(request, instruction)` | 收尾调用：`request.options.tools = None` → 追加 `instruction`（`nudge_message`）→ 以 `is_wrap_up_call = true` 跑一次 `chain.before_model`（触发 ④ 还原）→ `stream` + `consume` → 更新 `last_request` / `model_calls` / `token_stats` / `attempt_usages`；返回 `Result<{ text, reasoning }, TurnError>`。签名与行为见 §5.2 |
| `wrap_up_instruction` | 收尾指令文本（「预算已用尽，请基于已有结果给出结论，不要再调用工具」），仅追加到本次请求 |
| `not_executed(calls, reason)` | 为未执行的调用构造 `skipped` 形状的结果（§5.3），`is_error = true` |

**`RoundOutcome` 装配（所有 return 点统一走一个构造器，禁止各点各拼字段）**：

```
finish(status, text, reasoning, hit_cap, wrap_up_injected, error) -> RoundOutcome:
    RoundOutcome {
        status, text, reasoning, hit_cap, wrap_up_injected, error,
        iterations: iteration,                  // 最后一次实际进入的迭代号
        model_calls, tool_calls,                // 累计（含空响应重试与 skipped）
        prompt_tokens: token_stats.prompt, completion_tokens: token_stats.completion,
        cache_read_tokens: token_stats.cache_read, total_tokens: token_stats.total,
        attempt_usages, ttft_ms: token_stats.ttft_ms,
        confirmation_wait_ms: confirmation_wait_ms,   // 逐次 `ToolExecResult.confirmation_wait_ms` 累加（含收尾路径的 0）
        final_message_id,                       // persist_* 成功即置 Some（§5.1）
        context_window_tokens: last_request.context_window_tokens,   // 最后一次实际发出的请求
        reserved_output_tokens: last_request.max_tokens,
    }
```

- `iterations` 取**最后一次进入的迭代号**（不是 `max_iterations`）：第 1 步 `halted_by_budget()` 发生在迭代开头时，`iteration` 仍是当次值，符合「实际用了几个迭代」的语义。
- `last_request` 指**最后一次真正发给 provider 的请求**（含收尾调用，收尾调用的 `max_tokens` 与 `context_window` 才是压缩判定的输入）；`halted_by_budget()` 的非 `wrap_up` 路径没有新请求时沿用上一次。
- `confirmation_wait_ms` 由 executor 累加每次 `ToolExecResult.confirmation_wait_ms`（该值由 §6.4 的计时装饰器产生），executor 不自行计时；`skipped` 结果为 0。

**硬约束**：

- 工具执行受**自身超时**与 turn 剩余约束，**不继承** `model_call` 上限（I36 design D4）。
- 收尾调用占第 `max_iterations` 个名额，不额外加一轮；收尾调用不含 tools，但不得绕过审批（它不执行工具）。
- **收尾与 `spec.wrap_up` 绑定**：`wrap_up = false`（chat / `quick` / `deep`）时第 6 步不执行 —— 否则 chat 的 `max_iterations = 1` 会凭空多一次模型调用，A9 的「事件序列与 P3 一致」必然失败。无工具时按第 5 步结束；最后一轮仍有工具时按循环末尾的「迭代上限出口」产出摘要文本（见下条）。
- **阶梯与 `spec.ladder` 绑定**：`ProjectionOnly` 只做 ① 投影（+ 终止式压缩判定），不注册 ③④⑤；`Full` 才注册。③④⑤ 的实现可以常驻，但注册与否由 `spec.ladder` 决定，不由 executor 硬编码。
- `empty_response_retries` 的重发走第 3 步的**内层 `loop`**（外层 `for` 的 `continue` 会递增 `iteration`，禁止使用）；重发不递增 `iteration`，但**递增 `model_calls`**（它确实花了一次模型调用，预算与统计都要算）。
- **`wrap_up = false` 的最后一轮不得返回空文本**：最后一轮仍有工具调用时循环会自然走完，此时 `content` 可能为空（模型只发了 `tool_calls`）。必须走根因摘要（`budget_summary_reason("iteration_limit")`）+ `persist_final_text(summary, Completed)` + `hit_cap = true`，与预算档出口同形（A21 的「两档都不出现空文本」覆盖到该路径）。
- **三维预算独立**：`max_iterations`（有效迭代数）、`max_tool_calls`（工具调用总数，D18）、turn 墙钟。任一触发都走 §5.2 的 `halted_by_budget()`（文本非空、`hit_cap = true`），不得静默截断。
- 工具执行被取消时返回 `Cancelled` 的结果回填给模型，不当作失败。
- 所有长 await 必须包在 `tokio::select! { biased; _ = token.cancelled() => ... }`。
- **`pending_nudges` 只进本次请求的 `Vec<ChatMessage>`，绝不写 `messages` 表**（D19）：写进去会在后续每一轮重放成陈旧指令。
- **收尾还原只作用于本次结论调用**（D23）：不写回历史、不改变落库行；`restore_allowance` 与内容清单共用一份额度，`context_window` 缺失时为 0。
- **压缩不重置 `iteration`**（D24）：摘要调用不占迭代名额，但 `model_calls += 1` 且计入 token 统计。
- **`after_tool` 阶梯顺序固定**（D26 → D25 → D19）：先判豁免、再判分类预算、最后才进通用 2/3 阶梯；顺序颠倒会让可分类的失败被通用计数提前熔断。

### A.1 行级落库与请求历史

P3 的 `TurnRunner` 是「一个 turn 一行消息」的写方（`TurnContext.message_id` 建一次、`TurnPersister` 绑死、终态 `persist_terminal` 写这一行）。多迭代后这个假设不再成立，P4 定稿如下：

**行链**：一个 turn 的模型可见历史是一串行，不是一行。

```
[命令层预建 assistant 占位行]  ← TurnRunCtx.initial_message_id（第 1 迭代复用）
  ├─ tool 行（第 1 迭代每个 call 一条，parent = 占位行）
  ├─ assistant 行（第 2 迭代，parent = 上一条 tool 行，sibling_index 由 get_sibling_count 现算）
  │    └─ tool 行（第 2 迭代）
  ├─ …
  └─ 最后一条 assistant 行（纯文本或收尾结果）← RoundOutcome.final_message_id
```

- **每迭代一条 assistant 行**：`iteration > 1` 的行在第 4.5 步创建，`parent_id = last_row_id`（上一条 tool 行）。**不得挂到 user 消息下** —— 那会变成第 1 迭代的兄弟分支，前端会显示一条假分支。
- **`active_branch_id` 更新**：`MessageStore::create_assistant_message` 的 `src-tauri` 实现在插入后调用 `conversations::update_active_branch_id`（与 `cmd/chat/insert.rs:84` 同一做法），使新行成为活跃叶。P3 的实现没做这一步（命令层先建行后自行更新），P4 由 store 补齐，chat 路径因只有一行而无行为变化。
- **`final_message_id`**：`RoundOutcome` 回传本 turn **最后一条被写入并 `finalize` 的 assistant 行** id（含 chat 的占位行、含收尾/摘要行）；`TurnOutcome` 同步带出，供 `TurnRunner` 更新 `active_branch_id` 兜底、终态事件与日志。`None` 只出现在「本 turn 从未写入任何 assistant 行」（如 `request::build` 失败、首迭代建行前取消）；D22 空响应在 `iteration = 1` 写占位行、`iteration > 1` 沿用上一迭代已写的行，都不是 `None`。`persist_row` / `persist_row_as_new` / `persist_final_text` 成功即置 `Some`。
- **usage 写方**：**谁花掉模型调用，谁写 usage**。executor 在每次调用落库时写 `record_usage`（`message_id` = 本次 `msg_id`），一次调用一条，不覆盖。`TurnPersister::persist_terminal` / `persist_partial` 不再写 usage（P3 的 `usage_recorded` 标记随之删除）；`RoundOutcome.attempt_usages` 仍回传给 `TurnOutcome`，只用于汇总与日志。**没有行可写时**（D22 空响应且 `iteration > 1`）usage 只进 `attempt_usages` 汇总，不落库 —— 该场景不产生消息行。
- **代际校验不豁免**：executor 每次落库前必须走 `TurnTasks::is_current`（沿用 `TurnPersister::is_current` 的判定），旧代际 turn 的写入一律丢弃并 `log::warn!`。
- **取消 / 超时 / 错误路径**：先 `ensure_row()` 再 `persist_row(msg_id, partial_content, [], 消息状态)` = `save_content(已累积内容)` + `record_usage(已上报的 usage)` + `finalize`。消息状态按 `message_status_of(turn_status)`：取消 → `Interrupted`、超时 / provider 错误 → `Failed`（`RoundOutcome.status` 分别为 `Cancelled` / `Failed`，两套枚举不可互抄）。**错误路径不写 `tool_calls`**（部分累积的调用不完整，落库会让 wire 出现无配对结果的 `tool_call`），`tool_calls` 参数恒为 `[]`。`iteration = 1` 的 `ensure_row` 复用占位行；`iteration > 1` 时该行**尚不存在**（第 4.5 步才建），必须在此刻建 `parent_id = last_row_id` 的新行 —— **不得拿上一迭代的 `msg_id` 落库**，那会覆盖已 `finalize` 的行、破坏行链。错误路径同样把 `final_message_id = Some(msg_id)`，部分内容即本 turn 的可见叶。空响应重试路径**不建行**（A15）。
- **请求历史不重读 DB**：第 0 步 `request::build` 只投影一次；后续迭代只在内存 `Vec<ChatMessage>` 上追加 `assistant(tool_calls)` + 逐条 `role='tool'`。重启后由 `prompt_builder` 从行链重建，两者必须产出等价形状（A11）。**禁止每迭代重读 DB 重新投影**：那会让 §5.3 的 `skipped` 行与内存历史分叉，且 ③ 微压缩的「本轮已清空」状态无法保持。

**辅助函数的落库职责**（§5 只写调用点，实现按此展开，避免同一动作在两处各写一半）：

| 函数 | 动作 | 不做什么 |
|---|---|---|
| `ensure_row()` | 返回本迭代的 assistant 行 id：`iteration == ctx.start_iteration` 返回 `ctx.initial_message_id`；否则 `row_created == false` 时 `create_assistant_message(parent = last_row_id)` 并置 `row_created = true`；已建则直接返回 | 不写内容、不写 usage、不 finalize |
| `persist_row(msg_id, content, tool_calls, status)` | `save_content` → `record_usage`（本次 attempt 的 usage，按 `msg_id` 一条）→ `tool_calls` 非空时 `save_tool_calls` → `finalize(msg_id, status)`；并置 `final_message_id = Some(msg_id)`、`row_finalized = true` | 不建行（行由 `ensure_row` 提供） |
| `persist_row_as_new(content, status)` | **无条件**新建一条 assistant 行（`parent = last_row_id`，即上一条 tool 行 / 上一迭代末行），再按 `persist_row` 的同样顺序落库（同样置 `final_message_id = Some(new_id)`、`row_finalized = true`）；用于收尾调用结果与「本迭代行已 finalize 后的终态文本」，**不复用本迭代的 `msg_id`** | 不改动上一条 assistant / tool 行；无新 attempt 时不写 usage |
| `persist_final_text(text, status)` | 终态文本（`stopped` / `halted_by_budget` 非收尾路径 / `halted_with_summary` / 收尾调用失败）的择行写入：`row_finalized == false` 时走 `persist_row(ensure_row(), text, status)`（第 1 迭代即占位行，**绝不悬空**）；`row_finalized == true` 时走 `persist_row_as_new(text, status)`（本迭代行已 `finalize`，只能新建） | 不产生两条 assistant 行、不覆盖已 `finalize` 的行 |
| `append_tool_results(results)` | 对每个结果：内存 `request.messages` 追加一条 `role='tool'`（`tool_call_id` / `name` / `is_error` / 同一份 `content`）+ `store.append_tool_message(parent = last_row_id)`，每写成功一条即回写 `last_row_id`（首条挂本迭代 assistant 行，后续逐条链式追加）；返回**最后一条 tool 行 id** | 不发事件（`Started/Finished/Failed` 由 Pipeline sink 发）、不改 assistant 行 |
| `not_executed(calls, reason)` | 按 §5.3 的 `skipped` 形状构造结果（`is_error = true` / `Gateway` / `Denied` / 指定 `content`） | 不执行、不进失败统计 |
| `split_by_budget(calls, remaining_calls, remaining_clock)` | 按原始顺序切分 `to_run` / `skipped`（§5.3） | 不改顺序、不重新排序 |
| `stopped(reason, previous_content)` | 由 **`before_model`** 中间件的 `StopTurn` 触发（P4 无实现，保留通用出口）：把上一迭代已落库正文（为空时用根因摘要）经 `persist_final_text(text, Completed)` 落库并返回 `Completed`；`reason` 取 `StopTurn.reason`（**不含** `middleware` 名） | 不再调模型 |
| `halted_with_summary(reason)` | 由 **`after_tool` / `after_iteration`** 的 `StopTurn` 触发（P4 只有 D19/D25 阶梯会产生）：`text = progress_summary_reason(reason)`（根因摘要，`reason` 取 `StopTurn.reason`，原样使用、不再拼次数）经 `persist_final_text(text, Failed)` 落库并返回 `Failed` | 不再调模型、不改已落库的 assistant / tool 行 |

> **StopTurn 的路由规则（必须按阶段区分）**：`before_model` 的 `StopTurn` → `stopped()`（`Completed`）；`after_tool` / `after_iteration` 的 `StopTurn` → `halted_with_summary()`（`Failed`）。若把后两者也送进 `stopped()`，A21 的「无进展阶梯耗尽 → `status = failed`」直接失败。`after_tool` 的 `StopTurn` 还要**先落完本迭代全部 tool 行**再返回（否则下一轮 wire 缺配对结果）。

> **`StopTurn` 的两个字段不得混用**：`middleware`（`&'static str`，中间件名）只用于 `log::warn!` 与遥测；`reason`（`String`）是用户可见文本，落库前**不得**被 `format!("{}: {}", ...)` 之类拼入内部标识 —— 否则 UI 会出现 `repeated-failure: 工具 …` 这种英文内部键。中间件产出 `reason` 时按中文完整句给出（如 `halt_summary` 的「工具 X 连续失败 N 次，已停止本轮：…」），executor 只搬运不加工。

> 上表之外**不得**出现第三条写库路径：所有 assistant 行写入都经 `persist_row` / `persist_row_as_new`（`persist_final_text` 只在这两者之上做择行，不新增写路径），所有 tool 行写入都经 `append_tool_results`。

> **`row_finalized` 标记**：每迭代重置为 `false`；`persist_row` / `persist_row_as_new` 成功后置 `true`。它只用于 `persist_final_text` 择行与「本迭代是否已有可见行」的判断，不参与 wire、不落库。

### A.2 两种 Halt（对齐 I36 验收 17）

D19 的 `Halt` 档原本只有一个出口（「根因摘要覆盖文本 + `Failed`」），与 I36 验收 17「`max_tool_calls` 触发时回填上限提示**且本轮仍产出文本**」直接冲突。P4 拆成两个出口：

| 出口 | 触发 | 动作 | 终态 |
|---|---|---|---|
| `halted_by_budget()` | `max_iterations` / `max_tool_calls` / turn 墙钟耗尽（§5 第 1 步） | `spec.wrap_up = true`：撤回 tools、追加「预算已用尽，请给出结论」收尾指令，**再调一次模型**；`wrap_up = false`：不调模型，`text` 用根因摘要 | `wrap_up` 路径 = `Completed` + `hit_cap = true` + `wrap_up_injected = true`；非 `wrap_up` 路径 = `Completed` + `hit_cap = true`（I36 §8.2 表：迭代上限且未收尾也是 `completed`，不是失败） |
| `halted_with_summary(summary)` | D19 通用 2/3 阶梯、D25 分类预算耗尽（§5 第 8 步） | 不调模型，`text` = 中间件给出的根因摘要（已是中文完整句，含工具名 / 次数 / 末次原因） | `Failed` |

- 两档**都不再写「静默截断」**：文本一定非空，前端永远有内容可渲染。
- **两档的文本都必须落库**：`halted_by_budget()` 的非 `wrap_up` 路径与 `halted_with_summary()` 都不产生模型输出，但 `RoundOutcome.text` 是用户可见的最终结论，必须经 `persist_final_text(text, status)` 落库（`final_message_id` 指向该行，`status` 分别为 `Completed` / `Failed`）。否则重启后活跃叶停在 tool 行、失败原因丢失，A20 的「活跃叶为最后一条 assistant 行」也不成立。`content` 与 `RoundOutcome.text` 逐字节一致（与 D11 的回填一致性同口径）。`persist_final_text` 的择行保证**第 1 迭代的占位行不会被悬空**：尚未落库时直接写占位行，已落库才新建。
- `halted_by_budget()` 的收尾调用**也走行级落库**（`persist_final_text`：本迭代行已 `finalize` → 新建一条 assistant 行，`parent = last_row_id`），`final_message_id` 指向它。
- 预算耗尽时若收尾调用本身失败/超时 → 按 `Failed` 返回，`text` 退回根因摘要，不吞错误；该摘要同样经 `persist_final_text(summary, Failed)` 落库（收尾调用若已建行则沿用该行 `finalize(Failed)`，不重复建）。
- I36 验收 17 的「回填给模型的最后一条消息是『已达工具调用上限』」由 §5.3 的 `skipped` 行满足（`content = 未执行：本轮工具调用预算已用尽（max_tool_calls = N）`），与收尾指令是两件事，不互相替代。

**三个终态出口的伪码展开**（与辅助函数表配套，不得在别处另写一份）：

```
halted_by_budget(hook_name, reason) -> RoundOutcome:
    if spec.wrap_up:
        // 收尾调用：撤 tools + 追加收尾指令 + 还原 ③ 清空的结果（D23，由 is_wrap_up_call 触发 ④）
        let wrapped = call_model_without_tools(request, wrap_up_instruction)
        if wrapped.is_ok:
            persist_final_text(wrapped.text, Completed)
            return finish(Completed, wrapped.text, wrapped.reasoning, true, true, None)
        else:
            // 收尾失败/超时：退回根因摘要，不吞错误
            let summary = budget_summary_reason(hook_name)
            persist_final_text(summary, Failed)
            return finish(Failed, summary, "", true, true, Some(wrapped.error))
    else:
        let summary = budget_summary_reason(hook_name)   // 按触发的钩子名给出「已达迭代上限 / 工具调用上限 / 本轮超时」
        persist_final_text(summary, Completed)        // 非 wrap_up 也是 completed + hit_cap（I36 §8.2）
        return finish(Completed, summary, "", true, false, None)

halted_with_summary(reason) -> RoundOutcome:
    let summary = progress_summary_reason(reason)  // reason 已是中间件给出的中文完整句，原样使用（不再拼次数）
    persist_final_text(summary, Failed)
    return finish(Failed, summary, "", false, false, Some(TurnError::NoProgress { reason: summary }))

stopped(reason, previous_content) -> RoundOutcome:   // before_model 的 StopTurn（P4 无实现，仅保留出口）
    let text = if previous_content.is_empty() { progress_summary_reason(reason) } else { previous_content }
    persist_final_text(text, Completed)
    return finish(Completed, text, "", false, false, None)

call_model_without_tools(request, instruction) -> Result<WrapUpOutcome { text, reasoning }, TurnError>:
    // 与主循环第 2 / 2.6 / 3 步同形，差别只有三点：撤 tools、加收尾指令、`is_wrap_up_call = true`
    let mut req = request.clone()
    req.options.tools = None                                  // 收尾调用强制不带 tools（`ChatOptions.tools`）
    req.chat_messages.push(nudge_message(instruction))        // 只进本次请求，不落库
    let (mut window, mut max_tokens);
    {
        let mut view = ModelRequestView {
            messages: &mut req.chat_messages,
            context_window_tokens: req.context_window_tokens,
            max_tokens: req.options.max_tokens,
            is_wrap_up_call: true,                            // ④ 收尾还原的唯一开关
        };
        if let StopTurn { reason } = chain.before_model(&mut view, &turn_ctx).await:
            return Err(TurnError::MiddlewareStop { name: "wrap-up".into(), reason })
        window = view.context_window_tokens;
        max_tokens = view.max_tokens;
    }
    req.context_window_tokens = window
    req.options.max_tokens = max_tokens
    last_request = req.clone()
    let request_started = ctx.clock.now()
    let model_call_budget = effective_model_call_budget(ctx, request_started)
    let stream = req.stream(ctx.child_token.clone()).map_err(|e| TurnError::Provider(e.to_string()))?
    let r = kernel::stream::consume(stream, ctx.child_token.clone(), model_call_budget, ctx.clock, request_started,
                                    |_seq, d| { ctx.observer.delta(d); })
    model_calls += 1                                          // 收尾调用计入 model_calls 与 token 统计
    match &r {
        Ok(o) => { token_stats.merge(o); attempt_usages.extend(o.attempt_usages.clone());
                   Ok(WrapUpOutcome { text: o.full_content.clone(), reasoning: o.full_reasoning.clone() }) }
        Err(StreamError::Timeout(p))     => { merge_partial(p); Err(TurnError::Timeout) }
        Err(StreamError::Cancelled(p))   => { merge_partial(p); Err(TurnError::MiddlewareStop { name: "wrap-up".into(), reason: "cancelled".into() }) }
        Err(StreamError::Provider(e, p)) => { merge_partial(p); Err(TurnError::Provider(e.to_string())) }
    }

merge_partial(p) = { token_stats.merge_usage(p.last_usage()); token_stats.ttft_ms = token_stats.ttft_ms.or(p.ttft_ms);
                     attempt_usages.extend(p.attempt_usages.clone()) }
```

> **收尾调用为什么不复用主循环的调用代码**：主循环那次调用处在「本轮有工具结果、要落 tool 行」的上下文里，收尾调用没有工具结果、且必须先跑 `is_wrap_up_call = true` 的 ④ 还原；复用同一段会让 `row_finalized` / `last_row_id` 两个状态被两次调用共享，产生重复建行。两段代码共享的只有 `consume` 调用形状（6 参）与 `merge_partial`。

> **三个出口与 `call_model_without_tools` 都定义在 `run_iterations` 内部**（闭包或私有 fn 带 `&mut` 状态）：它们要读写 `request` / `last_request` / `row_finalized` / `model_calls` / `token_stats` / `attempt_usages` / `confirmation_wait_ms` 等循环作用域状态。实现时推荐把它们作为 `impl RoundExecutor` 的私有方法 + 一个 `RoundState` 结构体（把所有可变量收进去），行为与伪码一一对应。

- **收尾调用计入 `model_calls` 与 token 统计**：它真实消费了一次 provider 调用；但**不占迭代名额**（已是第 `max_iterations` 个迭代内）。
- **收尾调用的 usage 只写一次**：`persist_row_as_new` 写 usage 的前提是「该 attempt 的 usage 尚未落库」——收尾调用是新 attempt，写在新行；`stopped` / `halted_with_summary` 没有新 attempt，**不写 usage**。
- **`persist_final_text` 失败（DB 写失败）**：不得吞掉，`status` 按原终态返回但 `error = Some(TurnError::Persist(...))`，避免「文本已生成却静默丢失」。

### A.3 预算内裁剪（D18 的批次内落地）

三维刹车在迭代开头判（`stop_hooks_all_continue`），但一次模型响应可能一次要求 N 个调用，若直接整批执行就会越过 `max_tool_calls` / turn 墙钟。因此**执行前必须按剩余额度裁剪**：

```rust
let remaining_tool_calls = ctx.limits.max_tool_calls.saturating_sub(tool_calls);
let remaining_wall_clock = ctx.remaining_turn_budget
    .turn_deadline
    .map(|d| d.saturating_duration_since(ctx.clock.now()));
```

- **切分规则**：按模型给出的原始顺序，取前 `remaining_tool_calls` 个进 `to_run`，其余进 `skipped`。顺序优先于并发安全分组。
- **墙钟二判**：`remaining_wall_clock` 已为 0（或 Pipeline 返回前已耗尽）时，把尚未执行的批次整体并入 `skipped`。
- **`skipped` 的回填形状**：与正常结果同形，`is_error = true`，`stage = Gateway`，`error_kind = Denied`，`content` 为 `未执行：本轮工具调用预算已用尽（max_tool_calls = N）` 或 `未执行：本轮已超时`。必须写 `role='tool'` 行 —— OpenAI / Anthropic 的 wire 要求每个 `tool_call` 都有配对结果，缺行会让下一轮请求直接 400。
- **`skipped` 不参与失败统计**：不进 `after_tool` 中间件、不计入 D19/D25 阶梯。
- **`skipped` 计入 `tool_calls`**：模型确实发起了这些调用（D18 按发起数计）。

**为什么不做「删掉 assistant 行里的多余 tool_calls」**：那会让落库的 assistant 行与模型实际输出不一致，历史重放时模型看到的是被篡改的自我输出；而写「未执行」结果既保持 wire 合法，又给模型一个明确的可读原因。

---

## 附录 B：D 决策速查（实现相关）

| # | 决策 | 要点 |
|---|---|---|
| D11 | tool 结果落库 | `role='tool'` 行 + `tool_call_id` / `is_error` 列 |
| D12 | 审批计时 | `confirmation_wait_ms` 单独记录，不落库 |
| D13 | 阶梯 ② 摘要压缩 | P4 不实现，随续跑式压缩批次 |
| D14 | 存储形态 | 消息侧 ADD COLUMN；授权侧 CREATE TABLE IF NOT EXISTS |
| D15 | 工具集 | `ToolCatalog` 走 `tool_definitions_for_tier(Os)` |
| D16 | ToolCatalog async | async trait，P4 可纯内存读取 |
| D17 | 落库判据 | 模型下一轮必须看到 → 落库；纯运行时簿记 → 不落库 |
| D18 | 三维预算 | max_iterations / max_tool_calls / turn 墙钟 独立判定 |
| D19 | 重复失败阶梯 | Continue/Nudge/Halt 三级；nudge 不落库 |
| D20 | 工具执行策略 | 并发安全合批并行，其余串行 |
| D21 | 结果体积 | 复用 P2 的 select_persist_targets 落盘 |
| D22 | 空响应重试 | 内层 loop，不递增 iteration；输出上限截断不重试 |
| D23 | 收尾还原 | 仅本次结论调用，受 restore_allowance 约束 |
| D25 | 失败分类预算 | 按 (class, tool, scope) 分预算，成功即清零 |
| D27 | 投影期配对修复 | prompt_builder 只读配平，不写库 |
# I38f — P8.1：子代理规格（`spawn_agent`）

| 项 | 值 |
|---|---|
| 状态 | 草稿（可选备选，暂缓接入；未进入阶段表 / 门禁） |
| 日期 | 2026-11-09 |
| 阶段 | P8.1（可选候选；见 [roadmap](./I38-agent-roadmap.md) §2.1） |
| 关联 | [I18 design §3.16](./I18-agent-tool-architecture-design.md)（子 Agent 权限继承）、I36 design D16–D18（已归档）、[I36a §3.3/§3.4/§5](./I36a-agent-kernel-baseline.md)、I38a（已归档）（Policy / Gateway / Pipeline / `ApprovalKey`）、I38b（已归档）§2.4 / §4.5（委派预留与工具执行端口）、I38c（已归档）（工具实现范式）、I38d §4.2 / §4.6 / §4.7（已归档）（审批级别、策略装配、工具面板） |
| 影响范围 | `assistant-tools`（`ToolUseContext` / `ApprovalKey` / `ToolPipeline::with_storage`）、`assistant-agent`（`TurnSpec` / `TurnRunCtx` / `ToolExecCtx` / `TurnTasks` / `TurnLimits`）、`src-tauri`（`spawn_agent` 工具与子代理 runner 装配）、相关测试；**不改 wire DTO** |

> **备选状态（2026-11-09）**：子代理暂缓接入，本文留档为可选规格；不进入 roadmap §2 阶段表与门禁。
> 恢复实施前必须按 roadmap §7 第 5 条重新立项（先改 §2/§3，再启用本规格）。
> 本文是 P8.1（备选）的唯一权威规格。P8.1 只做「父 turn 内的一次性只读委派」；可继续子会话、roster / 任务板、后台活动投影归 P8.2，跨会话 Router 归 P8.3，重启恢复归 P9。
> **门禁**（恢复实施时）：P4 与 P5 完成（见 roadmap §3 的 P4 / P5 依赖）。P7 不是前置；即使 P7 已落地，External / MCP 工具也不进入 P8.1 子工具集（§3.4）。

---

## 1 目标与非目标

### 1.1 目标

| # | 交付 |
|---|---|
| 1 | `spawn_agent` 作为 Agent 工具接入 `ToolPipeline`，不新增旁路执行通道 |
| 2 | 父 turn 内启动**一个**子 turn，复用 `TurnRunner` / `RoundExecutor` 内核 |
| 3 | 子工具集 = 父工具集的**只读子集**（且 `tier <= Os`）；默认取父的只读工具，显式传入时必须是子集 |
| 4 | `max_depth = 1` 硬上限；子代理不得再委派；同父 turn 同时最多一个子代理 |
| 5 | 权限作用域：父批准不自动放行子代理；`ApprovalKey` 预留 `agent_id` 维度 |
| 6 | 取消树：父 turn 取消向下传播；子取消不反向取消父；父 turn 结束不留孤儿任务 |
| 7 | 结果回传：只把子 turn 终态文本 + 统计摘要回填父模型，不内联子 transcript |
| 8 | 记账隔离：子 turn 不写 `messages`，usage 以 `kind = subagent` 落在父会话名下 |

### 1.2 非目标

- 可继续子会话、子会话历史、roster / 任务板、steer / cancel / resume、后台活动投影（P8.2）。
- Router / Worker 跨会话调度、跨会话记忆（P8.3）。
- 跨重启的子代理存活、Watchdog、事件唤醒（P9，需单独 ADR）。
- 子代理写操作。P8.1 的子工具集**硬限制为只读**；写类子代理需要子审批通道与清理策略，归 P8.2。
- 子代理专用 UI。P8.1 不新增前端组件、不新增 wire 字段；子代理活动不可见（只通过父工具卡片体现）。
- 子代理使用 External / MCP 工具。P8.1 子工具集只允许 `tier <= Os`；External 工具的权限意图会触发 `Ask`，而 P8.1 没有子审批通道，归 P8.2。
- 子代理独立模型配置。P8.1 继承父 provider / model；独立 model 配置归 P8.2 或独立批次。
- 子代理附件传递。任务上下文由父以文本传入；P8.1 不把父历史或附件转交给子代理。

### 1.3 前置

1. P4 完成：`RoundExecutor::run_iterations`、工具执行端口、三维预算与 stop hooks 可用。
2. P5 完成：至少只读组工具已注册（`read` / `ls` / `glob` / `grep`），`resolve_available_tools` 的 deny-by-default 生效。
3. P2 完成：`ToolPipeline` 的 `find → validate → gateway → execute → persist` 顺序稳定，`ApprovalKey` / `PermissionGrantStore` 可用。
4. **不依赖 P6 的新交互**：`spawn_agent` 的审批复用现有 `ApprovalPrompter`（P6 已上线时走审批卡片；未上线时按 fail-closed 处理）。

---

## 2 架构边界

```
父 Agent Loop（RoundExecutor）
   │  ToolExecutionPort
   ▼
ToolPipeline（P2：find → validate → gateway → execute → persist）
   │  tool name = spawn_agent
   ▼
SpawnAgentTool（src-tauri，宿主工具）
   │  一次性委派 + 深度/并发/只读子集校验
   ▼
SubagentRunner（src-tauri 适配器）
   ├── 子 TurnRunner（assistant-agent，同一内核）
   ├── 子 TurnInputSource（合成 ConversationDetail；父 provider / model / adapter）
   ├── 子 MessageStore（内存 transcript；usage 转写父会话）
   ├── 子 EventSink（丢弃；P8.1 不广播子事件）
   ├── 子 TurnTasks（父令牌派生；取消树）
   ├── 子 SessionStates（独立实例，不污染父 phase）
   └── 子 ToolExecutionPort（只读子集 + 执行期白名单）
```

**依赖方向**（不得破坏）：

| 组件 | 位置 | 约束 |
|---|---|---|
| `SpawnAgentTool` | `src-tauri` | 实现 `assistant_tools::Tool`；依赖 `assistant-agent` 与 `assistant_tools`；**不得**放进 `assistant-tools` crate |
| `SubagentRunner` | `src-tauri` | 只做装配与端口适配；循环语义全部在内核 |
| 内核扩展 | `assistant-agent` | 不依赖 `tauri` / `assistant_tools`（沿用 I38b §2.2） |
| 权限字段 | `assistant-tools` | 不依赖 `assistant-agent` / `tauri` |

**硬约束**：

1. 委派只能作为工具进入，**禁止**在 `RoundExecutor` 内写 `if is_subagent` 分支（I36 D17 / roadmap R10）。
2. 子 turn 必须与父 turn 同进程、同生命周期；`spawn_agent` 不得把子 turn `spawn` 成游离后台任务。
3. 子 turn 的事件、消息、usage 三条写路径必须各自隔离；不得借父 `TauriEventSink` / `DbMessageStore` 落库。

---

## 3 工具契约（`spawn_agent`）

### 3.1 身份与 Tool trait 映射

| `Tool` 方法 | 取值 |
|---|---|
| `id()` / `name()` | `spawn_agent` |
| `description()` | 说明「把一个自包含的只读子任务委派给子代理，返回其结论；子代理看不到父对话历史，任务必须自包含」 |
| `input_schema()` | 见 §3.2 |
| `tier()` | `ToolTier::Sandbox`：工具本身不触碰 OS / 外部世界；子工具各自按父工具集的 tier 受控 |
| `audience()` | `AGENT_ONLY`：不进 ToolBox 透传入口 |
| `behavior_hints()` | `read_only = false`、`destructive = false`、`open_world = false` |
| `is_concurrency_safe()` | `false`：同一迭代内多个 `spawn_agent` 串行成批 |
| `manages_own_execution_timeout()` | `true`：子 turn 可能超过 `exec::invoke` 的 30s 统一 deadline，超时由工具自身按 §4.5 收口 |
| `permission_intents()` | `[("agent", "spawn")]`：受审批级别与项目规则约束，见 §5.1 |
| `validate_input()` | 见 §3.3（纯检查、无 IO） |
| `preview_for_approval()` | 见 §5.2 |
| `execute()` | 见 §4 |

`behavior_hints.read_only = false` 是有意的：委派会启动模型调用与工具执行，不能被静态标注成只读；但子代理的**工具面**仍被硬限制为只读（§3.4）。

### 3.2 `input_schema`

```json
{
  "type": "object",
  "properties": {
    "task": {
      "type": "string",
      "minLength": 1,
      "maxLength": 8000,
      "description": "Self-contained task for the sub-agent. Include all context it needs; it cannot see the parent conversation."
    },
    "tools": {
      "type": "array",
      "items": { "type": "string" },
      "maxItems": 20,
      "description": "Optional tool ids. Must be a read-only subset of the parent's current tool set. Omit to use all read-only tools of the parent."
    },
    "maxIterations": {
      "type": "integer",
      "minimum": 1,
      "maximum": 8,
      "default": 4,
      "description": "Upper bound of model iterations for the sub-agent."
    }
  },
  "required": ["task"],
  "additionalProperties": false
}
```

| 字段 | 约束 |
|---|---|
| `task` | 必填，去空白后 1–8000 字符；空串或超长 → `ValidationResult::invalid` |
| `tools` | 可选，最多 20 项；每项必须是父当前工具集 id、`read_only == true`、且不是 `spawn_agent` / `ask_user` |
| `maxIterations` | 可选，1–8，缺省 4；超过上限按 8 截断（不拒绝，避免模型因小误差整轮失败） |

### 3.3 `validate_input`（纯检查）

顺序固定：

1. `spawn_agent` 必须出现在 `context.allowed_tools` 中；否则 `ValidationResult::blocked("spawn_agent is not enabled for this turn")`。这堵住「模型幻觉调用未装配工具」的旁路。
2. `context.delegation_depth >= 1` → `blocked("sub-agent cannot delegate again")`（`max_depth = 1`）。
3. 解析 schema；`task` / `tools` / `maxIterations` 按 §3.2 校验。
4. 显式 `tools` 时逐项检查：不在 `context.allowed_tools`、非只读、或命中排除名单 → `invalid`，错误文本列出违规项；**不静默丢弃**。
5. 未显式子工具集时，默认集合 = `context.allowed_tools` ∩ 只读 ∩ 排除名单；空集合合法（子代理只做推理）。

`validate_input` 不得读 DB / 写文件 / 发事件；`ToolRegistry` 与 `Tool` 元数据是内存只读，允许访问。

### 3.4 子工具集解析

记父当前有效工具集为 `P`（`context.allowed_tools`，由宿主按 `resolve_available_tools` 落定），排除名单 `X = { spawn_agent, ask_user }`。

| 场景 | 子工具集 |
|---|---|
| 未传 `tools` | `P ∩ { t | t.behavior_hints().read_only ∧ t.tier() <= Os } \ X` |
| 传 `tools = R` | `R`，前提是 `R ⊆ P` 且 `∀t∈R: read_only(t) ∧ t.tier() <= Os ∧ t∉X`；否则验证失败 |
| 结果为空 | 合法；子 turn `TurnSpec.tools = None`，循环退化为纯推理 |

**执行期白名单**（与注入期白名单不同）：子 `ToolExecutionPort` 在进入 `ToolPipeline` 前按子工具集过滤。工具集以**内部 id** 保存；过滤时先用 `ToolRegistry::find_by_name` 把模型回传的 `tool_name` 解析为 `registered.id`，再与集合比较。任何不在集合内的调用（无论模型如何产生）直接返回：

```
ToolExecResult { is_error: true, stage: Gateway, error_kind: Denied,
                 content: "tool '<name>' is not available to this sub-agent" }
```

实现方式：`SubagentRunner` 持有 `Arc<ToolRegistry>` 与按子策略 / 子 id 集派生的 `PipelineToolExecution`；把放行调用交给派生 executor，拒绝项自造结果，最后按 `call_id` 合并回原始顺序。不得改动 `ToolPipeline` 的 `find → validate → gateway → execute → persist` 顺序。

理由：`ToolPipeline::find_by_name` 能解析注册表里的**任何**工具，注入期白名单不构成执行期授权；子代理必须用独立的执行期闸门，不能依赖「模型没看到」。

### 3.5 结果映射

| 子 `TurnOutcome.status` | `ToolResult` |
|---|---|
| `Completed` | `ok = true`；`data` = §3.5.1；`result_for_assistant` = 摘要文本 |
| `Failed` | `ok = false`、`error_kind = Business`；错误文本取 `TurnError` 的稳定文案（禁止 `Debug`）；子 `TurnOutcome` 不含 provider 原始载荷，故不附 `code` |
| `Cancelled` | `ok = false`、`error_kind = Cancelled` |
| `Interrupted` | `ok = false`、`error_kind = Cancelled`（父侧按「被中断」呈现） |
| 外层超时（§4.5） | `ok = false`、`error_kind = Timeout` |
| 装配失败（DB / provider / 槽位） | `ok = false`、`error_kind = Business` 或 `Internal`；文案说明原因 |

#### 3.5.1 `data` 形状（内部工具结果，非 wire）

```json
{
  "agentId": "conv-1:sub:1",
  "parentTurnId": "conv-1:3",
  "delegationDepth": 1,
  "status": "completed",
  "iterations": 3,
  "modelCalls": 3,
  "toolCalls": 4,
  "totalTokens": 1234,
  "text": "子代理最终结论"
}
```

`result_for_assistant` 是父模型唯一可见的子代理输出：

```
[subagent conv-1:sub:1 status=completed iterations=3 tool_calls=4]
<子代理最终文本>
```

子代理最终文本本身应是结论式摘要（§6.1 的系统提示要求）；不做二次摘要。文本过大时由 P2 的 `select_persist_targets` / `ToolResultStorage` 统一落盘并改写为引用，`spawn_agent` 不单独截断。

---

## 4 运行与生命周期

### 4.1 子代理身份

| 字段 | 取值 |
|---|---|
| `agent_id` | `format!("{}:sub:{}", parent_agent_id, seq)`；`parent_agent_id = context.agent_id.unwrap_or(context.session_id)`，`seq` 是父 turn 内从 1 递增 |
| `parent_turn_id` | 父 `TurnRunCtx.turn_id` |
| `delegation_depth` | 父深度 + 1（根为 0，故子为 1） |
| 子 `TurnId` | 子 `TurnTasks` 生成 `{child_scope}:{counter}`；`child_scope = agent_id` |
| 子 `conversation_id` | 合成作用域 `agent_id`（仅内核内部使用，不落 `conversations` 表） |

根 agent 的 `agent_id` 取 `conversation_id`（I36 D16）；P8.1 把 `TurnRunner` 里原先硬编码的 `"root"` 改为 `conversation_id`，但不改变任何 wire 行为。

### 4.2 子 `TurnSpec`

| 字段 | 取值 |
|---|---|
| `on_busy` | `Reject`（子作用域唯一，不会有并发 turn） |
| `max_iterations` | 输入 `maxIterations`，硬上限 8 |
| `tools` | §3.4 解析后的 `Vec<ToolSpec>`；空集归一为 `None`。`ToolSpec` 必须按 id 从 `ToolRegistry` **重新取**（`description` / `input_schema` 以注册表当前值为准），不得复用父 turn 的注入快照 |
| `ladder` | `Full`（微压缩 / 收尾还原 / 确定性裁剪对子 turn 同样有效） |
| `stop_hooks` | 与 agent 链路一致的三维预算钩子 |
| `wrap_up` | `true` |
| `route` | `Broadcast`（子 `EventSink` 丢弃事件，route 不影响 wire） |
| `kind` | `"subagent"`（usage 分类） |
| `limits` | `TurnLimits::subagent(max_iterations)`，见 §4.3 |
| `identity` | §4.1 的三个身份字段（新增结构，见 §8-C1） |

新增内核身份结构（内部类型，不参与 wire）：

```rust
#[derive(Debug, Clone, Default)]
pub struct TurnIdentity {
    /// `None` = 根 agent（运行时取 `conversation_id`）。
    pub agent_id: Option<String>,
    /// `None` = 根 turn。
    pub parent_turn_id: Option<String>,
    /// 根为 0，子为 1。
    pub delegation_depth: u32,
}
```

### 4.3 子预算

新增 `TurnLimits::subagent(max_iterations)`：

| 字段 | 取值 | 理由 |
|---|---|---|
| `max_iterations` | 入参（1–8） | 父可收窄，不能放大 |
| `max_tool_calls` | `derived_tool_call_limit(max_iterations)` | 沿用 P4 的派生公式 |
| `turn_wall_clock` | `min(120s, 父 turn 剩余墙钟)`；父剩余未知时 120s | 防止单个委派吞掉父 turn 预算 |
| `model_call` | `min(120s, child_wall)` | 单次调用不得超过子墙钟 |
| `empty_response_retries` | 1 | 与 agent 一致 |
| `microcompact_keep_recent` | 6 | 与 agent 一致 |

父 turn 剩余墙钟来自 `ToolUseContext.remaining_turn_ms`（§8-C1 新增字段）；`None` 表示父不设限，子仍用 120s。剩余为 0 时直接拒绝（`Timeout` 文案），不启动子 turn。

### 4.4 子输入（`SubagentInputSource`）

子 turn **不继承父消息历史**，只拿：

1. 父 provider 行 + 已解密 api key（复用 `TauriInputSource::conversation_detail` 的加载路径）。
2. 父 provider adapter（`Arc<dyn BaseProviderAdapter>` clone；适配器无每轮可变状态）。
3. 父 `AttachmentStore` 句柄（P8.1 不投递附件，仅保持端口形状）。
4. 一个合成的 `ConversationDetail`：父 provider / model，`active_branch_id = None`，`mode_id = None`，`tool_config_json = None`；消息列表只有一条 `user` 行，内容为 `task`。
5. 一个合成的 `AgentModeRow`：`kind = "agent"`（启用工具调用），`prompt = SUBAGENT_SYSTEM_PROMPT + "\n\n" + 父模式 prompt`，其余模型参数继承父模式；缺父模式时用 `AgentModeRow { kind: "agent", ..Default }`。

`SUBAGENT_SYSTEM_PROMPT`（固定文案）：

> You are a read-only sub-agent. Complete the delegated task using only the tools provided.
> Do not ask the user. Do not spawn other agents. Return a concise result with findings,
> file paths, and unresolved questions. If the task requires write access, report that
> clearly instead of attempting it.

**DB 锁纪律**：父会话 / provider / mode 的读取必须在启动子 turn 前完成并释放 `DbState` 锁；不得在子 turn 的整个运行期间持有 DB 锁（子 `MessageStore` 的 usage 写入按记录短锁获取）。

### 4.5 子运行、超时与取消

```
parent_token = TurnTasks::cancel_token(parent_conversation_id)
child_token  = parent_token.child_token()
child_tasks  = TurnTasks::new(); begin_child(child_scope, &child_token)
outer_deadline = child_wall_clock + 5s grace
```

| 行为 | 规则 |
|---|---|
| 运行方式 | `SpawnAgentTool::execute` **内联 await** 子 `TurnRunner::run`；不得 `spawn` 后不等待 |
| 父取消 | 父令牌取消 → 子令牌随取消树失效 → 子 `TurnRunner` 的长 await 走 `cancellable` 返回 `Cancelled`；父 `ToolPipeline` 外层的取消仍可丢弃整个 future |
| 子取消 | 子令牌取消**不**影响父令牌（`CancellationToken::child_token` 单向） |
| 外层超时 | `tokio::time::timeout(outer_deadline, child.run())`；超时后 `child_tasks.cancel(child_scope)` 并返回 `Timeout`，不把子 turn 转成游离任务 |
| 孤儿防止 | 子 runner 不启动后台 task；父 turn 结束（完成 / 失败 / 取消）时子 future 必然已结束或被丢弃 |
| 槽位 | 同一 `(conversation_id, parent_turn_id)` 只允许一个在跑子代理；第二次直接 `Business` 拒绝，不排队（I18 §3.16）。槽位用 RAII 守卫释放，父 turn 取消 / future 丢弃时不得残留 |
| 并发 | `is_concurrency_safe = false`，同迭代多个 `spawn_agent` 由 Pipeline 串行成批 |

### 4.6 子实例与事件

| 端口 | 子代理取值 |
|---|---|
| `EventSink` | `DiscardingEventSink`：吞掉全部 `TurnEvent`；P8.1 不向任何窗口发射子事件 |
| `MessageStore` | `SubagentMessageStore`：内存 transcript；usage 转写父会话（§4.7） |
| `SessionStates` | 独立 `SessionStates::new()`：子 phase 不进入父状态机 |
| `TurnTasks` | 独立实例 + `begin_child`；不写父 `TurnTasks` 条目 |
| `Clock` | 生产 `SystemClock`（与父同源） |
| `CompactionConfig` | `None`：子 turn 不做终止式压缩；上下文压力由阶梯 ③④⑤ 处理 |
| `ToolResultStorage` | 子作用域目录：`app_data_dir()/tool-results/subagents/<slug>/`；子 turn 结束后删除该目录。`slug` 由 `agent_id` 过滤为 `[A-Za-z0-9_-]`。删除失败只 `log::warn!`，不影响结果回传 |

**子结果落盘清理**：子工具结果超过 P2 阈值时会落盘；若沿用父共享目录，文件会在子 transcript 丢弃后成为孤儿。因此子 `PipelineToolExecution` 必须派生一个带子作用域 `ToolResultStorage` 的 `ToolPipeline`（`ToolPipeline::with_storage`），并在子 turn 收尾（成功 / 失败 / 取消 / 超时）后统一删除子目录。清理必须用 RAII 守卫（`Drop` 内做 best-effort `remove_dir_all`），否则父 turn 取消导致 `spawn_agent` future 被丢弃时清理代码不会执行；删除失败只 `log::warn!`。

**子事件不可见的取舍**：P8.1 没有 roster / 投影 UI，若把子事件广播出去，前端会收到无法归属的 `turn_id` / 工具卡片。P8.2 引入 `subagentProjection` 后再消费这些事件；届时只需替换 `EventSink` 与前端投影，不改内核。

### 4.7 子 usage 记账

子 `MessageStore` 只做一件事与持久化相关：转写 `record_usage`。

| 字段 | 取值 |
|---|---|
| `conversation_id` | 父会话 id（子 turn 的合成 id 不落库） |
| `message_id` | `None`（子消息不在 `messages` 表，不制造悬空引用） |
| `attempt_id` | 子 `turn_id` |
| `branch_leaf_id` | `None`（父分支叶子不可见于工具上下文；累计口径仍含子代理） |
| `kind` | `"subagent"` |
| `provider_id` / `model_id` | 继承子 `TurnInput`（即父 provider / model） |

写入失败只 `log::warn!`，不得让子 turn 失败（与父链路 usage 写入失败的处理一致）。占用统计查询只认 `kind = 'chat'`，子代理不会污染父会话的上下文占用。

---

## 5 权限与审批

### 5.1 `spawn_agent` 自身审批

| 环节 | 规则 |
|---|---|
| intents | `[PermissionIntent::new("agent", "spawn")]` |
| 审批级别 `none` | `policy_from_agent_mode` 的基座规则 `("*","*",Allow)` → `Allow`，不弹卡片 |
| 审批级别 `dangerous_only`（内置 agent 初始值） | 无专门规则 → 基座 `("*","*",Ask)` → `Ask` |
| 审批级别 `all_os_tools` | 同上 → `Ask` |
| 项目规则 | `.assistant/permissions.json` 可追加 `("agent","spawn",Deny/Ask)`，只能收紧 |
| 审批键 | `action = "agent"` 走 `ApprovalScope::Opaque { fingerprint: "spawn" }`；`AllowSession` / `AllowAlways` 语义为「允许委派」，不是「允许某个具体任务」 |
| 审计 | 走 P2 的 `PermissionAuditStore`，不新建表 |

`AllowAlways` 覆盖全部委派的取舍：P8.1 子代理只读且深度 1，委派授权面有限；任务内容在审批卡片可见。若 P8.2 允许写类子代理，审批键必须改为「任务摘要 + 子工具集」并重新定稿，不得沿用本键。

### 5.2 `preview_for_approval`

返回 `ApprovalPreview { summary, truncated, estimated: false }`：

- `summary = "委派子代理：<task 前 80 字>（工具 N 个，最多 M 轮）"`；
- `task` 被截断时 `truncated = true`；
- 不读文件、不调用模型、不改决策。

### 5.3 子作用域审批键

`ApprovalKey` 新增可选 `agent_id` 维度（§8-C2）：

| 场景 | `agent_id` | 序列化 |
|---|---|---|
| 根 agent（P4–P8.1 现状） | `None` | `agent_id` 字段**省略**，历史 `permission_grants` 的 `grantKey` 逐字节不变 |
| 子代理 | `Some(child agent_id)` | 键含 `agentId`；父键与子键不互相命中 |

规则：

1. 子代理**不复用**父代理的授权；「只读父作用域」仅指实现可查询父库做审计与解释，不作为放行依据（比 I18 §3.16 的「授权读取」更严格，避免父批准被委派放大）。
2. 子代理新增批准只写子作用域；P8.1 子工具全只读且子策略不走 `Ask`（§5.4），因此不产生子批准；该字段在此阶段只做契约与单测覆盖，P8.2 的子审批通道直接复用。
3. 父 turn 结束不清理子批准（P8.1 无子批准）；清理策略归 P8.2。

### 5.4 子策略（固定、只读）

子 `ToolExecutionPort` 使用：

```
child_policy = ResolvedPermissionPolicy::default_global()
             + PermissionRuntimeCeiling::try_new(deny_non_read_rules()).constraint_layer()
             + PermissionConstraintLayer::new([("*", "tier:external", Deny)])
```

`deny_non_read_rules` 对 `edit` / `write` / `delete` / `move` / `copy` / `execute` / `run` / `shell` / `agent` / `network` / `fetch` / `http` 逐条 `Deny`（不得出现 `Allow`，`try_new` 会拒绝）。

- 不用 `("*","*",Deny)`：`evaluate_tier_ceiling` 会以 `action = "*"` 求值，宽泛 Deny 会把只读工具的 tier 也拒掉。
- 子代理不继承父的 `approval_level`：父的 `all_os_tools` 不会让子代理的只读调用逐次弹审批；委派本身的审批（§5.1）覆盖该授权。此取舍登记在 §10 OQ4。
- 只读工具当前 `permission_intents` 为空，`ToolGateway` 短路为 `Allowed`；路径安全仍由 `validate_input` 复用 `os::path` 兜底。

---

## 6 上下文与结果

### 6.1 上下文隔离

- 子 turn 只看到 §4.4 的系统提示 + `task`；**看不到父消息树、父工具结果、父附件**。
- 父若需要子代理知道先前结论，必须在 `task` 文本中显式给出；不得由 `spawn_agent` 隐式内联父 transcript。
- 子工具结果只进入子 transcript（内存），不写父 `messages`；只有子最终文本作为 `ToolResult` 回父。

### 6.2 深度与放大器防护

| 防护 | 机制 |
|---|---|
| 深度 | `context.delegation_depth >= 1` 时 `spawn_agent` 直接拒绝；子工具集同时排除 `spawn_agent` |
| 工具面 | 注入期只读子集 + 执行期白名单（§3.4） |
| 并发 | 每父 turn 一个槽位；`is_concurrency_safe = false` |
| 预算 | 子墙钟 ≤ 120s 且 ≤ 父剩余；迭代 ≤ 8；工具次数按迭代派生 |
| 结果体积 | P2 单轮结果预算与落盘统一处理；子代理不额外放大 |

---

## 7 注册与可用性

| 层 | 规则 |
|---|---|
| `tool_setup.rs` | 注册 `spawn_agent`（`AGENT_ONLY`）；与 OS 工具共用同一 `ToolRegistry` / `ToolPipeline` |
| `list_available_tools` | 返回 `spawn_agent`（tier `sandbox`），前端工具面板可勾选 |
| 可见性 | 必须同时满足：`maxToolTier >= sandbox`（默认即满足） **且** `enabledTools` 显式包含 `spawn_agent` |
| 默认值 | 内置 `builtin-agent` 的 `enabledTools` 为空 → 默认看不到 `spawn_agent`；启用是显式动作 |
| ToolBox | `AGENT_ONLY` → `os_tool_exec` 不可达 |
| MCP | P7 的 External 工具**不进入 P8.1 子工具集**（`tier <= Os` 过滤）；子代理使用 External 工具归 P8.2（需要子审批通道） |

---

## 8 实施步骤

> 顺序按依赖固定；每步完成需可编译。任务级勾选在实施时另建 tasklist（本文不记录进度）。

| 步骤 | 内容 | 依赖 |
|---|---|---|
| **P8.1-C1** | 内核身份与上下文：`TurnSpec.identity`（`agent_id` / `parent_turn_id` / `delegation_depth`）；`TurnRunner` 根身份由 `"root"` 改为 `conversation_id`；`TurnRunCtx` / `ToolExecCtx` 透传身份与 `remaining_turn_ms`；`ToolUseContext` 新增 `agent_id` / `delegation_depth` / `allowed_tools` / `remaining_turn_ms`（全部可选且有默认值）；`TurnLimits::subagent(max_iterations)`；`TurnTasks::begin_child(scope, parent_token)` | P4 内核 |
| **P8.1-C2** | 审批键作用域：`ApprovalKey` 新增可选 `agent_id`，根键序列化保持不变；`ApprovalKey::from_intent` 从 `ToolUseContext.agent_id` 取值；补「父键不命中子键」单测 | C1 |
| **P8.1-C3** | `spawn_agent` 工具：身份、schema、`validate_input`（含 `allowed_tools` / 深度 / 只读子集）、`preview_for_approval`、`permission_intents`、`manages_own_execution_timeout`、执行期槽位与结果映射 | C1、C2 |
| **P8.1-C4** | `SubagentRunner`：合成 `TurnInputSource`、`SubagentMessageStore`（usage 转写）、`DiscardingEventSink`、独立 `SessionStates` / `TurnTasks`、子 `PipelineToolExecution`（子策略 + 执行期白名单 + 子作用域 `ToolResultStorage`）、外层超时与取消树、子结果目录清理 | C1、C3 |
| **P8.1-C5** | 宿主装配：`tool_setup.rs` 注册 `spawn_agent`；`send_agent_message` / `policy_executor` 下发父有效工具 id 集（`PipelineToolExecution::with_allowed_tools`）；`tool_resolver` 提取共享过滤函数并新增 id 版解析 | C3、C4 |
| **P8.1-C6** | 测试与验收：§9 A1–A12；重点覆盖只读子集、执行期白名单、深度/槽位、取消树、超时、usage 隔离、根键兼容 | C5 |
| **P8.1-C7** | 文档复核：roadmap §2.1 / §3 / §4 与本文引用一致；不改 wire DTO（无需 `contract:gen`） | C6 |

**跨阶段契约变更登记**：

- `ToolUseContext` 字段新增属 `assistant-tools` 内部类型，不是 wire；但所有使用结构体字面量的测试需补 `..Default::default()`。
- `ApprovalKey` 的 `agent_id` 必须 `#[serde(default, skip_serializing_if = "Option::is_none")]`，否则历史授权键失效。
- `TurnSpec` 仅内核可见；`TurnRunCtx` / `ToolExecCtx` 字段新增会被 `assistant-agent` 测试替身引用，需同批更新。

---

## 9 验收标准

| # | 标准 |
|---|---|
| A1 | `spawn_agent` 的 `audience = AGENT_ONLY`：`list_available_tools` 可见，`os_tool_exec` 不可达 |
| A2 | 未在 `allowed_tools` 中时 `validate_input` 拒绝；深度 ≥ 1 时拒绝；两种拒绝都不进入审批 |
| A3 | 默认子工具集 = 父只读工具集去 `spawn_agent` / `ask_user`；显式 `tools` 含非子集或非只读项时返回可纠正错误并列出违规项 |
| A4 | 子工具集为空时子 turn 仍可纯推理完成，`tools = None` |
| A5 | 子代理回传伪造/未列入的工具调用时，执行期白名单返回 `Denied`，不进入 `ToolPipeline` |
| A6 | 子代理无法再次委派（工具不在子集 **且** 深度自检拒绝）；同父 turn 第二个并发委派被 `Business` 拒绝 |
| A7 | 父 turn 取消 → 子令牌取消、子 `TurnRunner` 收敛为 `Cancelled`；子取消不取消父；无后台 task 残留 |
| A8 | 子 run 超过子墙钟返回 `Timeout`，且子任务已取消；子 `Failed` / `Cancelled` / `Interrupted` 的 `error_kind` 与文案符合 §3.5 |
| A9 | 子成功回传：`data` 字段齐全，`result_for_assistant` 含状态头与子文本；父模型可读 |
| A10 | 子 `messages` 不入库：父会话 `messages` 数量与内容不变；`token_usage_records` 出现 `kind = 'subagent'` 行，`conversation_id` 为父会话，占用查询（`kind = 'chat'`）不受影响 |
| A11 | `ApprovalKey` 根键 JSON 与改造前逐字节一致；子键含 `agentId` 且不命中父键；`permission_grants` 历史行仍可命中 |
| A12 | `cargo test -p assistant-tools`、`cargo test -p assistant-agent`、`cargo test --test assembly_contract` 通过；`assistant-agent` 无 tauri 依赖；`cargo clippy --workspace --all-targets -- -D warnings` 与 `cargo fmt --check` 通过；`npm run contract:check` 幂等（P8.1 不改 DTO） |

---

## 10 风险与开放问题

| 风险 | 缓解 |
|---|---|
| 子代理消耗大量 token / 墙钟 | 子墙钟 ≤ 120s、迭代 ≤ 8、每父 turn 一个槽位；usage 记账可观测 |
| 父上下文无法被继承导致任务失败 | 系统提示要求任务自包含；错误回填给父模型，父可补充上下文重试 |
| 子代理绕过工具白名单 | 执行期白名单（§3.4），不依赖注入期可见性 |
| 父批准被委派放大 | `ApprovalKey.agent_id` 作用域；子键与父键不互命中 |
| `spawn_agent` 未装配却被模型调用 | `validate_input` 检查 `allowed_tools`，未装配即 blocked |
| 历史授权键失效 | `agent_id` 省略序列化；A11 锁定 |
| 子事件污染前端 | P8.1 丢弃子事件；P8.2 再引入投影 |

**开放问题**（P8.1 按下列默认落地；后续批次可改）：

1. **子代理模型**：P8.1 继承父 provider / model；是否需要独立便宜模型？归 P8.2。
2. **父级执行期白名单**：现状 `PipelineToolExecution` 只做注入期白名单，父 agent 幻觉调用未装配工具仍可能进入 Pipeline（只受策略约束）。P8.1 只对 `spawn_agent` 做自检、对子代理做执行期白名单；是否把执行期白名单提升为 P2 的通用能力，需单独评估（会改 P2 契约）。
3. **`AllowAlways` 语义**：当前覆盖全部委派而非单个任务；若未来引入写类子代理，必须改成任务摘要 + 子工具集并重新定稿。
4. **子代理不继承父 `approval_level`**：只读子调用不逐次弹审批，委派审批覆盖；若用户要求 `all_os_tools` 对子读也生效，需要在子策略中注入父审批级别，归 P8.2。
5. **交互工具排除名单**：P8.1 硬编码排除 `ask_user`。新增交互类工具时需要 `BehaviorHints` 增加「需要用户在场」维度，或维护统一排除表。
6. **子代理结果体积**：依赖 P2 的落盘阈值；若父模型仍频繁遇到超大子结果，再考虑专用摘要器（I36 design §3.19 的解冻条件）。

---

## 11 参考

- I18 design §3.16：子 Agent 权限继承与会话作用域（授权读取 / 新增批准 / ceiling / 数量与深度）。
- I36 design D16–D18：身份字段、委派边界、取消树与结果回收。
- I36a §3.3 / §3.4 / §5：DSH 能力接缝、Codex 控制面 / 执行面分离、扩展点。
- I38a §2.6 / §2.9 / §4.5：`ApprovalKey`、授权存储端口、Pipeline 执行语义。
- I38b §2.4 / §4.5：委派预留、`ToolExecutionPort` 职责切分。
- I38d §4.2 / §4.6 / §4.7：审批级别、每轮策略装配、工具面板。
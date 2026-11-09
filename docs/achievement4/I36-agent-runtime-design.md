# I36 — Agent 运行时架构决策记录（ADR）

> 本文记录运行时**决策**：为什么这样分层、参考了什么、取舍是什么。**不记录进度**。
> 阶段、状态与门禁见 [I38-agent-roadmap.md](../store/I38-agent-roadmap.md)（唯一阶段入口）。
> 实施规格见 [I36-agent-runtime-spec.md](./I36-agent-runtime-spec.md)；Agent Loop 见 [I38b](./I38b-agent-loop-spec.md)；
> 内核能力基线与参考实现对照见 [I36a-agent-kernel-baseline.md](../store/I36a-agent-kernel-baseline.md)；
> 内核一致性判定（确定性要求 / 验证矩阵 / 实检缺口）见 [I36b-agent-kernel-conformance-spec.md](../store/I36b-agent-kernel-conformance-spec.md)。
> 工具框架见 [I18-agent-tool-architecture-design.md](../store/I18-agent-tool-architecture-design.md)；
> ToolBox 桥接见 [I18b-tool-unified-bridge-spec.md](../store/I18b-tool-unified-bridge-spec.md)；
> chat 构建剥离见 [I30-phase1-toolcall-separation-spec.md](./I30-phase1-toolcall-separation-spec.md)。

---

## 1 问题域

### 1.1 现状：一次请求就是一次对话

当前链路是 `send_chat_message` → `adapter.chat_completion_stream` → `chat_runtime::consume_stream` → `cmd/chat` 落库并发事件。一次用户输入对应**一次** provider 请求，请求结束即对话结束。这套链路没有「轮」的概念，因此工具闭环（模型 → 工具 → 回填 → 继续生成）无处安放。

**chat 链路的模块最终迁入共用内核**（§1.4 / §1.5）。下表左侧的 chat 资产不是「参照后自建」，而是**迁移来源**：能力进 `kernel/`，chat 与 agent 都从内核调用：

| chat 资产 | 位置 | 迁移去向 |
|---|---|---|
| 取消令牌 + 代际计数 | `chat_tasks.rs` | `kernel/tasks.rs`（`OnBusy::Replace` 保持 chat 语义） |
| 流式累积与部分结果 | `chat_runtime.rs` | `kernel/stream.rs`（tool 通道移到 `agent/`） |
| 命令层编排（落库 / 发事件 / spawn） | `src-tauri/src/cmd/chat` | `kernel/persist.rs` + `kernel/emit.rs`（经端口）；命令层只留适配 |
| 请求构建 / 提示词 | `chat_request.rs`、`prompt_builder.rs` | `kernel/request.rs` |
| 选项 / 归一化 | `chat_options.rs`、`normalization.rs` | 保持为无状态函数，`kernel/request.rs` 直接调用 |

可直接使用的只有**无状态纯函数**（无会话、无流、无 chat 假设）：

| 纯函数资产 | 位置 | 提供什么 |
|---|---|---|
| 请求预算卸载 | `request_projection.rs` | 图片 → Tool 内容 → 历史用户文本，逐级卸载 |
| 压缩判定 | `compaction.rs` | `NotNeeded / Terminate / Undetermined` |
| 多模态投影 | `multimodal.rs` | 文件/图片按预算投影 |
| token 估算与截断 | `token_estimate.rs` | 本地估算、按 token 截断 |
| PDF 文本抽取 | `pdf_text.rs` | 附件预处理 |

### 1.2 缺口清单

| 缺口 | 后果 | 归属层 |
|---|---|---|
| 无 turn 概念 | 一次输入只能触发一次模型调用，工具结果无法回填 | 运行时 |
| 无状态机 | chat 链路只有一个布尔 `is_running`，前端分不清「思考 / 流式 / 调工具 / 等审批」 | 运行时 |
| 无轮内循环 | LLM 返回 tool_call 后到此为止（P4 / I38b 未落地） | RoundExecutor |
| 无停止钩子 | 只能在事后取消，不能在两次调用之间刹车 | 运行时 |
| 只有一层上下文降级 | 压缩是「终止本轮」，不是「轮内续跑」 | 运行时 |
| 无终态持久化 | 进程崩溃后消息仍是 `streaming` 形状，UI 无从判断 | 运行时 |
| 无轮级观测 | 无 TTFT、无轮级 token / 迭代汇总 | 运行时 |

### 1.3 三层边界

| 层 | 职责 | 权威文档 |
|---|---|---|
| **AgentRuntime**（turn 编排） | turn 生命周期、会话状态机、预算、停止钩子、事件投影 | 本文 |
| **RoundExecutor**（iteration 循环） | 模型调用 → 工具执行 → 回填 → 下一迭代 | I38b（P4） |
| **Provider 流式**（单次请求） | 请求投影、流式累积、部分结果、单次超时 | 本文 Step 3 |

**分层判据（唯一）**：

> 谁拥有「跨迭代」的决策，谁就在 AgentRuntime。

单次请求内部的关注点（投影、累积、单次超时）不下沉到 runtime；跨迭代的计数与刹车（预算、迭代上限、重复失败）不下放到单次请求。

### 1.4 运行时与 chat 链路的边界

**决策：runtime 必须能在没有窗口、没有前端的情况下跑完一整轮。**

这条不是「顺手支持」，而是判据：**任何一处 `AppHandle` 出现在 turn 的关键路径上，后台运行就不成立**。现状有三处硬依赖：

| 现状 | 位置 | 后果 |
|---|---|---|
| 发送前查窗口 | `cmd/chat/mod.rs` `app.get_webview_window(WINDOW_ORB)`，缺失即返回 `error.orb_window_not_found` | 窗口不存在时**发不出消息**，与后台运行直接冲突 |
| 事件只发 orb 窗口 | `protocol/events.rs` `emit_orb_only` | 窗口销毁后 delta 无处可去 |
| 编排在前端 | `src/services/chatOrchestrator.ts` | 见 §1.5 |

**前端编排的实证**（`chatOrchestrator.ts`）：

- `ensureIdle()`：切会话/发送前先 `cancelChat` 再**轮询 5 s** 等后端变 idle，超时强制置 idle。
- 切换会话时前端主动 `cancelChat`（`chatConversationSlice.ts`）。
- 前端自己维护 `phase: idle | sending | error`，并做终态事件去重（`handledTerminalEvents`）。
- 前端拼装乐观消息、分支点解析、标题兜底、错误映射。

这些全是**跨轮的决策**，按 §1.3 的判据应该在后端。前端做这些的直接后果是：窗口一关，轮的状态机就断在半路；两个窗口同时打开时各自算一套 phase。

**决策内容**：

1. `runtime` 与 turn 内核**不依赖 `tauri`**，落在 `assistant-agent`（该 crate 当前已无 tauri 依赖，只需新增内核模块）。
2. turn 关键路径只通过**端口**（§1.6）访问外部世界：事件、消息持久化、输入源、时钟。
3. 前端从「编排者」降级为「订阅者」：只渲染后端给出的 `phase` 与消息，不做 cancel 调度、不做 idle 轮询、不推导 phase。
4. 窗口关闭不等于 turn 结束。终止条件只有四个：显式取消、`turn_wall_clock` 到期、`model_call` 到期、应用退出。

### 1.5 chat 与 agent 的共用内核

> **本版修订**：上一版结论是「runtime 自建最小重复实现」。本版改为**抽出共用内核，chat 与 agent 都成为内核的 client**。理由：两者在 turn 生命周期、流式消费、部分结果、压缩、事件、落库上做的事**几乎完全相同**，差异只在「迭代次数」「有没有工具」「忙时是顶替还是拒绝」。重复实现会把这三处差异扩散成两套并行演化的代码。

**内核（`assistant-agent/src/kernel/`）承担**：

| 能力 | chat 用法 | agent 用法 |
|---|---|---|
| turn 身份与令牌表 | 顶替（`OnBusy::Replace`） | 拒绝（`OnBusy::Reject`） |
| 流式消费（delta 累积 / 部分结果 / 单次超时 / 取消） | 1 次调用 | 每次迭代 1 次调用 |
| 请求构建（投影 / 多模态 / 附件 / 提示词） | 无工具 | 带 tools |
| 压缩判定与执行 | 终止式 | 阶梯第 ② 步，续跑式（D9 第二步后） |
| 部分结果与终态落库 | 一致 | 一致 |
| 事件发射（带 `turn_id` + `seq`） | 一致 | 一致 |
| 用量记录 | 一致 | 一致 |

**两条链路各自的差异（不进内核）**：

| 维度 | chat | agent |
|---|---|---|
| 迭代次数 | 固定 1 | `max_iterations` |
| 工具 | 无 | `RoundExecutor` |
| 审批 | 无 | `ApprovalGate` |
| 状态机 phase | 只用到 `Thinking / Streaming / Finalizing` | 全部 phase |
| 上下文阶梯 | 只有 ①（投影）+ ②（终止式压缩） | ①–⑤ 全阶梯 |
| 停止钩子 / 收尾调用 | 无 | 有 |
| 事件路由 | orb 窗口 | 广播 |

**选路不进内核**：哪个会话走 chat、哪个走 agent，由前端按**会话当前模式**的 `kind` 决定——`kind = agent` → `send_agent_message`，其余（`chat` 与旧值 `default` / `quick` / `deep` / `custom`）→ `send_chat_message`（旧值仅作兼容标签、不决定分支，见 I38d §4.11）；每次发送按会话当前 `modeId` 选路，同一 `conversationId` 内可切换（turn 进行中禁止切换）。规则见 I38d §4.8。内核只提供 `TurnSpec::chat()` / `TurnSpec::agent()` 两个取值，不做任何选路判断，也不读取模式表。

**参数化而不是分叉**：上述差异全部表达为内核的入参（`TurnSpec`），不写 `if is_agent { ... }`：

```rust
pub struct TurnSpec {
    pub on_busy: OnBusy,                  // Replace | Reject
    pub max_iterations: u32,              // chat = 1
    pub tools: Option<ToolCatalog>,       // chat = None
    pub ladder: LadderPolicy,             // chat = ProjectionOnly | TerminatingCompact
    pub stop_hooks: Vec<Arc<dyn StopHook>>, // chat = []
    pub wrap_up: bool,                    // chat = false
    pub route: EventRoute,                // Orb | Broadcast
    pub kind: &'static str,               // usage 统计口径：chat | agent（与 route 解耦）
    pub limits: TurnLimits,               // 墙钟 / 单调用超时 / 空响应重试 / 微压缩
}
```

**偏差记录（C5 落地时）**：`TurnLimits` 原先作为 `TurnRunner` 字段传入，导致「chat 行为差异」同时藏在 runner 配置与 spec 两处，违反「差异只能来自 `TurnSpec`」。现收进 `TurnSpec`，由 `TurnSpec::chat()` / `TurnSpec::agent()` 绑定各自的 limits；`TurnRunner` 因此为八字段。`TurnLimits::max_iterations` 仅用于派生 `max_tool_calls`，迭代上限的权威值仍是 `TurnSpec::max_iterations`。

**判据**：新需求若只影响 chat 或只影响 agent，用 `TurnSpec` 字段表达；若字段无法表达，才允许分叉。禁止在内核里写 `if conversation.is_agent`。

### 1.6 端口（ports）

内核通过 4 个端口访问外部世界，端口定义在 `assistant-agent`，实现放在 `src-tauri`：

| 端口 | 职责 | 生产实现 | 测试实现 |
|---|---|---|---|
| `EventSink` | 接收 `TurnEvent`（delta / phase / 工具 / 压缩 / 终止） | 映射为 Tauri 事件（`chat-*` / `session-state-changed`），按 `EventRoute` 选 orb 或广播 | 收集到 `Vec<TurnEvent>` |
| `MessageStore` | 建助手消息、写内容、写终态、读会话输入、记录 usage | `DbState` 上的 rusqlite 实现 | 内存实现 |
| `TurnInputSource` | 会话详情、provider 行 + 明文 key、`AttachmentStore`、模式配置 | `AppHandle` 上的 state 与 crypto | 固定 fixture |
| `Clock` | `now()`、TTFT 起点 | 系统时钟 | `tokio::time::pause` 可控时钟 |

**端口的硬约束**：

1. 端口方法是 `async` 或纯同步，**不得**出现 `AppHandle`、`Window`、`tauri::State` 类型。
2. 端口实现失败必须**可降级**：`EventSink` 失败只记日志（事件是加速器，不是事实源）；`MessageStore` 失败必须让 turn 失败（消息是事实源）。
3. `TurnInputSource` 是**只读**端口：内核不改会话元数据（标题、分支）以外的任何东西；标题兜底这类写操作留在宿主命令层。

### 1.7 事件与事实的分工

| | 事实源 | 作用 |
|---|---|---|
| 消息内容与终态 | DB（`messages`） | 重连、重启、多窗口对齐的唯一依据 |
| `phase` | 后端 `SessionStates` | 输入框与工具卡片状态 |
| delta 流 | `EventSink` | 只用于「更快的观感」，丢失可由消息查询补齐 |

**因此**：窗口不存在时，`EventSink` 是 no-op 而不是错误；前端重新挂载后靠「查消息 + 查 phase」对齐，不靠补发历史 delta。

---

## 2 参考项目关键设计提取

### 2.1 OpenBitFun — 会话状态与取消

`src/crates/execution/agent-runtime/src/`：

```rust
pub enum SessionState {
    Idle,
    Processing { current_turn_id: String, phase: ProcessingPhase },
    Error { error: String, recoverable: bool },
}

pub enum ProcessingPhase {
    Starting, Compacting, Thinking, Streaming, ToolCalling, ToolConfirming,
}
```

关键洞察三条：

1. **状态机判据是行为而非展示**：`can_start_new_turn = Idle | Error{recoverable:true}`。`Error` 携带 `recoverable` 而不是简单复位为 `Idle`，因为「用户可重试的失败」与「干净空闲」对输入框的约束不同。
2. **`Processing` 携带 `current_turn_id`**：状态与代际绑定，避免「旧轮的迟到完成把新轮状态改成 Idle」。
3. **取消令牌按 turn 存**（`turn_cancellation.rs`）：`get_or_insert_new` 对同一 turn 复用同一令牌，`cancel()` 幂等。

`compression_prefetch.rs` 提供投机压缩的三态认领：

```rust
pub enum PrefetchClaim<T, E> { Discarded, Ready(T), Waiting(CompressionPrefetch<T, E>) }
```

关键洞察：**认领前观测到的失败直接丢弃，认领后的失败返回给调用方**。这保证了投机任务失败不会污染主流程的压缩状态。

### 2.2 Codex — 压缩的生命周期语义

- 压缩被建模为 **turn item**：`emit_turn_item_started(ContextCompactionItem)` → `start_new_context_window` → `emit_turn_item_completed`。压缩因此和工具调用一样，在事件流里是可观测的一段，而不是隐式副作用。
- **pre/post compact hook 可以中止整轮**：`PreCompactHookOutcome::Stopped → Err(CodexErr::TurnAborted)`。压缩不是「一定成功」的步骤。
- `CompactionTrigger { Manual, Auto }` 区分手动与自动触发，但走同一条生命周期——避免「手动压缩」与「自动压缩」分裂成两套实现。
- `compact_token_budget`：token 预算型压缩**不做模型摘要**，直接安装一个新的上下文窗口，但仍然建模为压缩（让 hook 与 UI 观察到同一生命周期）。
- `compact_retained_tool_outputs`：显式标记的「无对应调用的命名输出」与普通历史**共享同一预算**，要么整条保留，要么整条丢弃——不允许半截保留。
- `turn_timing`：TTFT（首个 token）与 TTFM（首个模型项）按轮记录。

### 2.3 OpenHuman — 循环策略、中间件链与上下文阶梯

`crates/openhuman-core/src/agent/tinyagents/`：

**运行策略**（`turn_policy.rs`）：

| 限制 | 值 | 说明 |
|---|---|---|
| `max_model_calls` | = `max_iterations` | 必须显式设置，否则落到 SDK 默认 25 |
| `max_tool_calls` | `max_iterations * 8`（最小 8） | 嵌套作用域取 `min`，**委托只能收紧** |
| `max_wall_clock_ms` | 3600 s | 整轮墙钟上限 |
| `max_model_call_ms` | 900 s | 单次模型调用上限，每次重试重新计时 |
| `empty_response_retries` | 1 | 「有推理无正文」的空响应重试一次 |

两条写进注释的教训值得直接吸收：

1. **拿 turn 剩余预算当单次调用上限会杀死合法长任务**：某轮已合法花掉 543 s，下一次调用只剩 56 s 就被判死。因此拆成「整轮上限」+「单次调用上限」，单次取 `min(单次上限, 轮剩余)`。
2. **重试必须只存在于一层**：历史上 provider 层与 harness 层各有一份重试，两者叠加导致退避时间与错误分类都对不上。最终把重试收归 harness 单层。

**停止钩子**（`stop_hooks.rs`）：

```rust
pub trait StopHook: Send + Sync {
    fn name(&self) -> &'static str;
    fn check(&self, state: &TurnState<'_>) -> StopDecision;
}
pub enum StopDecision { Continue, Stop { reason: String } }
```

`TurnState` 借出循环局部变量（`iteration` / `max_iterations` / `tool_calls` / `max_tool_calls` / `prompt_tokens` / `completion_tokens` / `elapsed` / `turn_deadline` / `model`），钩子零分配；`check` 是**同步**的（§3.5），**在每次模型调用之前触发**——刹车必须发生在花钱之前。

**中间件链**（`middleware.rs`）：`before_model` / `after_tool` 两类钩子，把「微压缩」「工具输出封顶 + 语义摘要」「工具策略」「循环守卫」「重复失败熔断」全部表达为中间件。注释里写明动机：这些关注点在旧引擎里写死在工具调用路径上，换 harness 后**全部变成死代码**——只有挂到主循环的钩子上才随循环生效。

**上下文阶梯**（`harness_context_ladder.rs`）：`before_model` 按注册顺序执行，注册顺序即语义顺序：

```
① 摘要压缩（LLM 摘要，折叠较旧切片）
② 微压缩（清空较旧工具结果体，保留最近 N）
③ 收尾还原（capped turn 的结论调用恢复被 ② 清空的工具结果）
④ 硬裁剪（整条淘汰最旧消息，system 消息永不丢）
```

两条踩坑记录直接决定我们的顺序设计：

- 顺序曾是 `② → ① → ④`：摘要器拿到的是已被清空的工具结果，而它的提示词又在要求「关键结果/输出」——**摘要里全是占位符，且无法恢复**。
- 微压缩曾**未受预算门控**：一过 `keep_recent` 就无条件清空，导致「窗口明明放得下 10 次工具调用」的轮只保留 5 条结果。

**轮结果**（`turn_outcome.rs`）：除文本外还携带 `model_calls` / `tool_calls` / `input_tokens` / `cached_input_tokens` / `charged_amount_usd` / `hit_cap` / `wrap_up_injected` / `early_exit_tool` / `breaker_halt` / `tool_outcomes`。其中 `hit_cap` 与 `wrap_up_injected` **分开表达**，因为调用方动作不同：后者已无需再向模型索要结论，前者仍需走带外收尾。

**启动清扫**（`reaper.rs`）：启动时把上个进程遗留的非终态 run 收敛为 `Cancelled`，尽力而为、绝不阻塞启动。

### 2.4 提取汇总

| 要素 | 来源 | 结论 |
|---|---|---|
| 会话状态机 + phase + `recoverable` | OpenBitFun | 采纳 |
| 状态与 turn_id 绑定 | OpenBitFun | 采纳 |
| turn 级取消令牌 store | OpenBitFun | 采纳（内核 `turn_tasks`，格式与 `attempt_id` 兼容） |
| 投机预取压缩 + 三态认领 | OpenBitFun | 暂不采纳（§3.19） |
| 压缩作为 turn item + pre/post hook | Codex | 采纳 |
| `CompactionTrigger{Manual,Auto}` | Codex | 采纳 |
| 保留输出整条保留或整条丢弃 | Codex | 采纳 |
| TTFT / TTFM | Codex | 采纳（仅 TTFT，TTFM 暂不做） |
| 双层墙钟上限 | OpenHuman | 采纳 |
| 停止钩子在迭代之间 | OpenHuman | 采纳 |
| 中间件链 | OpenHuman | 采纳 |
| 上下文阶梯顺序 + 预算门控 | OpenHuman | 采纳 |
| `max_tool_calls` 嵌套取 min | OpenHuman | 采纳 |
| 重试只在一层 | OpenHuman | 采纳（保持在 provider 层） |
| `hit_cap` / `wrap_up` 分开 | OpenHuman | 采纳 |
| 排队输入 | OpenHuman | 暂不采纳（§3.19） |
| 持久事件 journal | OpenHuman / OpenBitFun | 暂不采纳（§3.19） |
| 子代理 / `max_depth` | OpenHuman / OpenBitFun | 暂不采纳（§3.19）；P4 只预留契约（D16-D18） |

---

## 3 决策

### 3.1 D1 — turn / iteration / attempt 三层标识

**决策**：内核的 `TurnTasks` 生成 `turn_id = {conversation_id}:{counter}`，**格式与线字段 `attempt_id` 完全一致**，因此 `attempt_id` 的语义升级为「turn 代际 id」；新增两个只存在于内存与事件的字段：`iteration`（轮内迭代序号，从 1 起）、`provider_attempt`（provider 层重试次数，只读，由 adapter 上报）。

**理由**：前端已按 `attempt_id` 丢弃过期事件（I16 stream-attempt-identity），改字段名要同时动 DTO 契约与前端；而它实际的用途就是「这是哪一次生成」，与 turn 代际语义完全一致。命名偏差的代价远小于一次跨层重命名。chat 的「顶替」不是另一个实现，而是内核 `OnBusy::Replace` 参数（§1.5）。

**取舍**：`attempt_id` 这个名字现在是「turn」，读代码的人会误解。以文档 + 类型别名 `TurnId = attempt_id` 收敛；若将来重命名，一次 DTO 迁移完成，不在本轮做。

### 3.2 D2 — 会话状态机是后端唯一事实源

**决策**：新增 `SessionStates`（按 `conversation_id`）：

```rust
pub enum SessionState {
    Idle,
    Processing { turn_id: String, phase: TurnPhase },
    Error { message: String, recoverable: bool },
}
```

`phase ∈ Starting | Compacting | Thinking | Streaming | ToolCalling | AwaitingApproval | Finalizing`。前端只做投影，**不自行推导**任何 phase。

**理由**：多窗口共享同一会话（I20 跨窗口同步）。若每个窗口各自从 delta 流推导 phase，必然漂移出「A 窗口显示正在调工具、B 窗口显示正在思考」。且 phase 是工具卡片五态（I38d）的上游，必须有唯一来源。

**取舍**：新增一个低频离散事件 `session-state-changed`，不塞进 `chat-delta`——delta 是高频流式语义，phase 是低频状态语义，混在一起会让前端要做「delta 里哪些是状态」的分流。同时 `chat-stream-start` 与终止事件**冗余携带**当前 phase，保证晚订阅窗口能立刻对齐而不必等下一次状态变化。

**附带决策**：`Processing` 期间的新 turn 按 `OnBusy` 策略处理——agent 用 `Reject`（返回 `Busy`，由宿主命令转成用户可见错误），chat 用 `Replace`（保持现有「静默顶替」）。这是 `TurnSpec` 参数，不是两套实现（§1.5）。

### 3.3 D3 — 取消是协作式、代际化的

**决策**：turn 级 `CancellationToken` 为父令牌，模型流与每次工具执行各取 `child_token()`。取消只置位，**不 `abort()` task**。写终态前校验 `turn_id` 是否为当前代际：非当前代际的完成结果**丢弃**（不写库、不发终止事件、不改状态）。

**理由**：
- `abort()` 会跳过部分结果落库，用户已经看到的内容会凭空消失；协作式取消能让 `kernel::stream::consume` 正常返回 `StreamError::Cancelled(partial)` 并落库。
- 代际校验解决「旧 turn 的迟到完成把新 turn 覆盖成 Idle」——这是现有 `attempt_id` 已经在做的事，运行时必须继续遵守，不能因为引入状态机而绕过。

**取舍**：协作式取消要求每个 await 点都响应令牌，靠「自觉检查」不可靠；因此规定所有长 await（模型流、工具执行、审批等待）必须包在 `tokio::select! { biased; _ = token.cancelled() => ..., r = fut => r }` 里，写进规格的验收项。

### 3.4 D4 — 双层时间上限，工具调用不继承单次模型调用上限

**决策**：

| 上限 | 默认 | 作用域 |
|---|---|---|
| `turn_wall_clock` | 60 min | 整个 turn |
| `model_call` | 15 min | 单次模型调用（每次 provider 重试重新计时） |
| 工具调用 | 自身超时（I18 §8.2）+ turn 剩余 | 不叠加 `model_call` |

**理由**：OpenHuman 的教训是「用 turn 剩余量当单次上限会杀死合法长任务」；反过来，没有单次上限时，一个卡死的流会让整轮挂死且**永远等不到终止事件**。两者必须同时存在。

**取舍**：两个默认值都是「防挂死」而非「体验指标」，所以远大于用户可感知延迟；都可配置，`0` 表示关闭。工具调用不叠加 `model_call`，因为工具可能合法地跑满 30 s 甚至数分钟（`shell`、未来的 MCP）。

### 3.5 D5 — 停止钩子在迭代之间，不在事后

**决策**：`StopHook { name, check(&TurnState) -> Continue | Stop{reason} }`（同步，见 §3.5），在**每次模型调用之前**触发。内置三个恒装：迭代上限、工具调用数上限、turn 墙钟（P4 三维预算，I38b D18）；轮级 token 预算为附加维度，未配置时不安装。重复失败熔断不在此处，归 `after_iteration` 中间件（D19 的 nudge/halt 分级）。

**理由**：事后检查意味着钱已经花了。OpenHuman 明确把 stop hooks 放在 iteration 边界，且 `TurnState` 借出循环局部变量以避免热路径分配。

**取舍**：钩子必须是**同步轻量检查**，不做 IO。需要 IO 的输入（价格目录、配额）在 turn 开始时算好放进 `TurnState`，钩子只做比较。

### 3.6 D6 — 最后一次模型调用做收尾，而不是直接截断

**决策**：当本轮将因 `max_iterations` 结束、且最后一次响应仍请求工具时，**撤回 tools 并追加收尾指令**，让模型基于已有结果给出结论；`TurnOutcome.wrap_up_injected = true`。

**理由**：直接截断会把「工具都做完了但没总结」的半成品交给用户。收尾调用仍受 `model_call` 上限约束，不额外放宽。

**取舍**：`max_iterations` 实际会多花一次请求。规格里明确「第 N 次是收尾调用，不是新迭代」，并在 UI 上归入同一个 `Thinking` phase，不额外显示一轮。

### 3.7 D7 — 中间件链顺序固定，跨切面不进循环体

**决策**：`TurnMiddleware` 提供 `before_model` / `after_tool` / `after_iteration` 三类钩子，**按注册顺序执行，注册顺序即语义顺序**。循环体只写控制流（判断是否继续、回填结果、更新计数）。

**理由**：OpenHuman 的注释直指问题本质——同一批关注点写在工具调用路径上时，换执行引擎后全部变成死代码；只有挂到主循环的钩子上才随循环生效。chat 的 `cmd/chat` 已经在长成「一个大函数里塞满关注点」的形状，这正是 runtime 另起独立路径、而不是继续在 chat 上叠加的原因（§1.4）。

**取舍**：固定顺序意味着新增中间件必须声明插入位置（不是「随便 append」），并写进规格的顺序表。这比隐式依赖强。

### 3.8 D8 — 上下文阶梯：顺序即语义

**决策**：`before_model` 的降级步骤按「最便宜且足够」排序：

| 序 | 步骤 | 触发 | 成本 | 失败行为 |
|---|---|---|---|---|
| ① | 请求投影卸载（已有） | 估算 > budget | 零 | 原样继续 |
| ② | 摘要压缩（**不随 P4**，与续跑式同批，I38b D13） | 估算 ≥ budget × 阈值 | 一次模型调用 | 降级到 ③，不中止 |
| ③ | 微压缩（清空较旧工具结果体） | 仍 > budget（P4 无 ②，直接在此起效） | 零 | 原样继续 |
| ④ | 收尾还原 | 仅 capped turn 的结论调用 | 零 | 原样继续 |
| ⑤ | 硬裁剪（整条淘汰最旧消息） | ③/④ 后仍 > budget | 零 | 原样继续 |

**硬约束**：②③ **必须受预算门控**，不得无条件执行。

**理由**：OpenHuman 的两条踩坑（摘要器读到被清空的占位符、微压缩未受门控导致「放得下也清」）都是顺序/门控问题，不是算法问题。顺序写死在文档里，才有可评审的对象。

**取舍**：摘要压缩要额外一次模型调用，因此阈值默认沿用现有配置项 `compact_threshold_percent_of_budget`（默认 80，**占可用预算**而不是占完整窗口，分母已扣除输出预留与 `COMPACTION_SAFETY_TOKENS`）。窗口未知时（现有 `CompactDecision::Undetermined`）**不压缩**，只做 ①③⑤ 并记日志——保持现有语义，不把「不可判定」当成「不超限」。

### 3.9 D9 — 压缩是 turn 内的一个阶段，不是 turn 的终点

**目标形态**：`Compacting` phase 内完成摘要，然后**继续本 turn**。

**迁移分两步**：

1. 先把压缩抽成显式 turn item（`started` / `completed` 事件 + pre/post hook 可中止），但**保持现有终止式语义**（`CompactDecision::Terminate` → 压缩后另起）；
2. 待 RoundExecutor 就位后，把「终止式」替换为「续跑式」。**该步不随 P4 同批**：RoundExecutor 在 P4 落地，但续跑式切换与阶梯 ② 作为 P4 之后的独立提交（I38b D13），P4 期间轮内自动压缩不接线。

**理由**：终止式的用户体感是「对话被打断、要重新发一次」，而压缩本应无感。但直接改成续跑需要一个能继续的地方（RoundExecutor），否则没有落点。

**取舍**：过渡期会短期存在两套压缩触发（手动命令 + 轮内自动）。用同一个 `CompactTrigger { Manual, Auto }` 统一事件与审计，避免分裂成两套实现——这是 Codex 的做法。

### 3.10 D10 — 终态持久化只加一列

**决策**：`messages.status ∈ streaming | completed | interrupted | failed`，默认 `completed`。`turn_id` / `iteration` / `provider_attempt` **不落库**。

**理由**：进程崩溃后唯一需要跨进程恢复的事实是「这条消息没写完」。把 turn 结构落库会引入与 `conversations.generation` 重复的版本语义，且没有消费者。

**取舍**：牺牲「崩溃后精确知道停在第几轮、第几个工具」。当前需求里没有消费者（UI 只需区分「写完了 / 被打断」）。若将来要做断点续跑，再引入 turn 表，届时是一次完整迁移。

### 3.11 D11 — 启动清扫

**决策**：启动时把 `status = 'streaming'` 的消息改写为 `interrupted`，并清理内存状态机残留；**尽力而为，失败不阻塞启动**。

**理由**：对齐 OpenHuman `reap_orphaned_runs`。上一个进程留下的非终态必须被收敛，否则 UI 永远显示「正在生成」，且用户无法发送新消息。

**取舍**：不做重试循环；DB 锁不可用时记日志跳过，下次启动再扫。

### 3.12 D12 — 事件是单一有序流

**决策**：每个 turn 一条单调递增 `seq`，所有运行时事件（phase 变化、工具开始/结束、压缩、终止）都带 `turn_id + seq`；前端按 `seq` 排序、按 `turn_id` 过滤。

**理由**：多窗口 + 重连需要可排序；现有 `ChatDeltaPayloadDto` 已有 `seq`，扩展比新建通道成本低。

**取舍**：**不**引入独立的事件 journal（OpenHuman 用 JSONL 持久化事件流，OpenBitFun 有 `session_event_journal`）。我们已有消息表作为持久事实，事件只是 UI 投影；再持久化事件会造出第二事实源，且要额外解决「事件与消息不一致时谁赢」。

### 3.13 D13 — 后台可运行：turn 的存活与窗口无关

**决策**：turn 由后端持有，**窗口只是观察者**。`send` 不要求任何窗口存在；`cancel` 由后端状态机裁决，前端可发起但不是唯一发起方。

**理由**：
- 现有 `send_chat_message` 在 orb 窗口缺失时直接报错，等于「关掉窗口就不能用 agent」。agent 要跑几十次模型调用、几十次工具执行，中途用户关窗是常态。
- 前端 `ensureIdle()` 的 5 s 轮询本质是**前端在替后端做生命周期等待**。后端有了状态机（D2），这个等待应该由 `begin_turn` 的返回值表达，而不是前端定时器。

**取舍（D13a，现行）**：不做「应用退出后继续跑」。进程退出即 turn 中断，启动清扫（D11）负责把残留 `Processing` 收敛为终态。**不**引入独立后台进程或系统服务——收益（跨进程存活）与成本（IPC、密钥、单实例、升级）不成比例。

**边界（D13b，P8+ 候选）**：上述取舍是**当前实现边界**，不是对「持久化运行」的永久否决。Super-agent 的 persistence（重启恢复、Watchdog、事件唤醒）若进入范围，必须单独出 ADR，并满足 D16-D18 的会话谱系与委派契约；不得在 D13a 下直接叠加后台进程。前置条件：会话谱系可落库、事件可重放、授权与取消树可按 agent 作用域裁决。

### 3.14 D14 — 抽共用内核，而不是两套最小实现

**决策**：`assistant-agent/src/kernel/` 是 chat 与 agent 共用的 turn 内核；差异用 `TurnSpec` 参数化（§1.5）。

**理由**：
1. **两份实现必然漂移**。`chat_tasks` 的「顶替」与 `turn_tasks` 的「拒绝」如果各写一遍，后续任何一处修 bug（比如代际校验漏了某个分支）都要改两遍，且只改一边不会被发现。
2. **落库与事件格式是最容易漂的地方**。`seq` 语义、`turn_id` 格式、部分结果的写入时机，一旦两套实现有细微差异，前端就要写两套解析（这正是上一版列为「硬约束」的东西，参数化能直接消灭它）。
3. **chat 迟早要变成 agent 的退化实例**。早抽比晚抽便宜：现在抽，chat 只是换一个调用入口；等 agent 长到 3 倍体量再抽，chat 会被迫对齐一堆 agent 专属概念。

**取舍**：抽取期间 chat 的行为**必须逐字节不变**（事件名、`seq` 起点、落库时机、错误文案）。因此迁移方式是「**先加内核、chat 改调内核、再删旧实现**」三步，中间允许短暂双路径，但**同一时刻只有一条路径被编译进 `send_chat_message`**。

### 3.15 D15 — 前端降级为订阅者

**决策**：以下编排从前端移到后端，前端只保留渲染与输入：

| 前端现状 | 移到哪里 |
|---|---|
| `ensureIdle()` 轮询 + 5 s 超时 | `begin_turn` 返回 `Busy` / `Accepted`，前端不做等待 |
| 切会话时主动 `cancelChat` | 后端在切换会话时按策略处理（见下） |
| 自维护 `phase` + 终态去重 | 订阅 `session-state-changed`，按 `turn_id` 过滤 |
| 乐观消息拼装 | 后端先落库并发 `chat-user-message-confirmed`，前端只做「占位到真实 id 的替换」 |
| 标题兜底 | 宿主命令层（`cmd/agent` / `cmd/chat`），不进内核（§1.6 第 3 条） |

**「切会话时是否取消」是策略，不是规则**：默认**不取消**（后台继续跑完，消息落库后通过 `chat-messages-changed` 补齐）。用户显式点停止才取消。这与现状相反，但正是「后台运行」的定义——如果切走就停，就不叫后台。

**取舍**：不取消意味着切走后输入框可能显示「另一会话正在运行」，需要 UI 明确指示（`session-state-changed` 已足够）。同时要保证**同一会话同一时刻只有一个 turn**（否则后台跑两个 turn 会互相覆盖写库）。

### 3.16 D16 — 会话谱系与 agent 身份（P8 前置）

**决策**：turn 的内存结构与事件载荷新增三个**可选、默认根 agent** 的身份字段：`agent_id`、`parent_turn_id`、`delegation_depth`。根 agent 取 `agent_id = conversation_id`、`parent_turn_id = None`、`delegation_depth = 0`。

**语义**：

| 字段 | 根 turn | 子 agent turn |
|---|---|---|
| `agent_id` | conversation 的根 agent | 子会话/子 agent 的稳定 id |
| `parent_turn_id` | `None` | 发起委派的父 turn |
| `delegation_depth` | `0` | 父深度 + 1，受上限约束 |

**理由**：D12 的 `seq` 以 turn 为序，现有事件只靠 `turn_id` 区分来源；一旦出现 worker，前端无法从扁平事件流还原「哪个任务属于哪个 agent」。这三个字段在 P4 恒为根 agent，**零行为变化**；若等到 P8 再加，事件 DTO、前端过滤与落库投影都要重做。

**取舍**：不落库完整会话树（沿用 D10），只保证身份可表达、可过滤。父子关系以 `parent_turn_id` 表达，不新增 turn 表；需要跨重启恢复时由 P9 单独迁移。

**参考**：DSH `subagentCatalog` 的 child id/创建时间/模式；OpenBitFun `sessionLineage` / `subagentProjection`；Codex `sender_thread_id` / `receiver_thread_ids`。

### 3.17 D17 — 委派边界与权限继承（P8 前置）

**决策**：子代理**作为工具**（`spawn_agent`）经 `ToolPipeline` 进入，不在 `RoundExecutor` 内部写 `if is_subagent` 分叉。默认约束：

| 约束 | 默认 | 说明 |
|---|---|---|
| `max_depth` | 1 | 父可显式放宽，但不得超过全局上限 |
| 并发子 agent | 可配置，必须有显式上限 | 与 turn 的并发上限同源，不允许无界 spawn |
| 工具集 | 父显式收窄 | 子 agent 只能看到父授予的子集，不可扩大 |
| 权限 | 读父 `GrantStore`，新批准写子作用域 | `PermissionRuntimeCeiling` 只紧不松 |
| 回传 | 摘要或引用 | 不把子 agent 完整 transcript 内联回父上下文 |

**理由**：参考项目的共同做法是「委派是工具/能力 seam，不是循环分叉」——DSH 的 `ctx.subagents`、OpenHuman 的 `delegate_*`、Codex 的 collaboration tools 都如此。权限上若允许子 agent 扩大父授权，审批键与 ceiling 会失效；上下文上若内联完整 transcript，token 成本随并发线性膨胀。

**取舍**：P4 只定义契约与可选字段，**不实现** spawn；子 agent 的 persona、output schema、工具过滤等高级能力留到 P8 按消费方逐个加，避免一次性设计过多无人使用的选项。

**参考**：DSH `SubagentCapabilities`（`depthLimit` / `toolFilter` / `persona` / `outputSchema`）；OpenHuman orchestrator 的 `subagents` allowlist 与 `delegate_*` 工具；Goose `max_concurrency`；Codex `fork_turns`。

### 3.18 D18 — 取消树与结果回收（P8 前置）

**决策**：取消令牌按父子关系组成**取消树**：父 turn 取消时向下传播到子 agent；子 agent 取消**不**反向取消父 turn。子结果通过 `MessageStore` 或摘要引用回父，父 turn 写终态前仍按 D3 校验 `turn_id` 代际，非当前代际的完成结果丢弃。

**理由**：D3 的协作式取消与 D12 的单向有序流在单 agent 下成立；多 agent 下若没有树形传播，父 turn 结束后子 agent 会变成孤儿任务，继续消耗模型调用并写库。反向取消则会让「子任务失败」误伤父任务。

**取舍**：P8 之前不实现后台 worker 的持久化存活；子 agent 生命周期与父 turn 同进程。跨重启的孤儿收敛归 P9（见 D13b）。

**参考**：DSH continuable subagent 的 activation/`interrupt` 与 child-first disposal；Goose `SubagentOutcome::Cancelled`；OpenBitFun 后台子代理活动投影。

### 3.19 暂不采纳清单

| 要素 | 来源 | 不采纳理由 | 解冻条件 |
|---|---|---|---|
| 投机预取压缩 | OpenBitFun | 当前轮规模下收益不明显，且要求压缩任务可取消、可丢弃的完整生命周期 | 单轮出现 ≥2 次压缩 |
| 排队输入（turn 进行中入队） | OpenHuman | 当前 UI 流式期间禁用发送，无消费方 | 前端支持「流式期间继续输入」 |
| 子代理 / `max_depth` | DeepSeek Harness / OpenHuman / OpenBitFun | 当前无消费方；P4 只预留身份与委派契约（D16-D18），不实现 spawn | 单轮出现可并行的独立子任务，或用户同时推进多个任务 |
| Router / Worker 跨会话编排（Super-agent） | DeepSeek Harness | 当前只有 conversation 级 turn，无会话之上的调度与跨会话记忆 | P8 立项，且 D16-D18 已落地 |
| skill / 按需工具暴露 | DeepSeek Harness / OpenHuman | 当前工具数量少，静态工具 schema 成本可控 | 每轮工具 schema 超过预算阈值，或引入 skill 目录 |
| 持久化运行 / Watchdog | DeepSeek Harness | 见 D13b，需要单独 ADR | P9 立项 |
| USD 成本上限 | OpenHuman | 无价格目录 | 引入模型价格表 |
| 持久事件 journal | OpenHuman / OpenBitFun | 见 D12 | 需要离线回放轮时间线 |
| 工具结果语义摘要（payload summarizer） | OpenHuman | 先用确定性清空（③）拿到稳定收益 | ③ 仍不够用 |
| 会话级 goal / light checkpoint | OpenBitFun | 无工作区/分支目标概念 | 引入工作区任务模型 |
| TTFM（首个模型项） | Codex | 仅 TTFT 已能定位首包延迟 | 需要分项延迟归因 |

---

## 4 模块落点

```
src-tauri/crates/assistant-agent/src/
├── kernel/               // 共用内核：chat 与 agent 都是它的 client（§1.5）
│   ├── mod.rs            // 对外导出 TurnRunner / TurnSpec / TurnContext / ports
│   ├── ids.rs            // TurnId（= attempt_id 值）、TurnContext
│   ├── phase.rs          // TurnPhase + 状态投影
│   ├── state.rs          // SessionState / SessionStates（状态机唯一事实源）
│   ├── tasks.rs          // TurnTasks：令牌 + 代际计数，OnBusy::{Replace, Reject}
│   ├── stream.rs         // 流式消费：delta 累积 / 部分结果 / 单次超时 / 取消
│   ├── request.rs        // turn 级请求构建（吸收 chat_request / prompt_builder）
│   ├── persist.rs        // 部分结果与终态落库（经 MessageStore 端口）
│   ├── emit.rs           // TurnEvent 生成 + seq 分配（经 EventSink 端口）
│   ├── limits.rs         // TurnLimits + 默认值 + 与 AppConf 的映射
│   ├── outcome.rs        // TurnOutcome / TurnStatus / 计数
│   ├── ports.rs          // EventSink / MessageStore / TurnInputSource / Clock
│   └── runner.rs         // 外层循环：iteration / 阶梯 / 钩子 / 收尾 / 终态
├── agent/                // agent 专属：内核之上的多迭代能力
│   ├── mod.rs
│   ├── stop.rs           // StopHook / StopDecision / 内置钩子
│   ├── middleware.rs     // TurnMiddleware trait + 注册表 + 顺序表
│   ├── ladder.rs         // 阶梯 ③④⑤（① 复用 request_projection；② 随续跑式压缩同批）
│   └── compaction_exec.rs// 摘要执行（② 的执行体，随续跑式压缩同批，不在 P4）
├── chat_runtime.rs       // 退场中：迁入 kernel/stream.rs 后删除
├── chat_tasks.rs         // 退场中：迁入 kernel/tasks.rs（OnBusy::Replace）后删除
├── chat_request.rs       // 退场中：迁入 kernel/request.rs 后删除
├── chat_options.rs       // 保留：推理档位映射（无状态）
├── prompt_builder.rs     // 退场中：迁入 kernel/request.rs 后删除
├── normalization.rs      // 保留：文本归一化（无状态）
├── request_projection.rs // 共享纯函数：阶梯第 ① 步
├── compaction.rs         // 共享纯函数：判定（不含执行）
└── token_estimate.rs     // 共享纯函数：估算与截断
```

```
src-tauri/src/
├── cmd/
│   ├── chat/             // 保留：命令适配（校验 + 调内核 + 标题兜底）
│   └── agent/            // 新增：send_agent_message / cancel_agent / 状态查询
├── adapters/             // 新增：端口的生产实现
│   ├── event_sink.rs     // TurnEvent → chat-* / session-state-changed，按 EventRoute 选路由
│   ├── message_store.rs  // DbState 上的消息与 usage 读写
│   └── input_source.rs   // 会话详情 / provider 行 / 附件 / crypto
```

**依赖方向（单向）**：

- `kernel` → `request_projection` / `compaction`（判定）/ `token_estimate` / `multimodal` / `pdf_text` / `chat_options` / `normalization`：允许（无状态纯函数）。
- `kernel` / `agent` → `tauri` / `AppHandle` / `Window` / `cmd/*`：**禁止**（§1.4、§1.6）。
- `agent` → `assistant-tools`：**禁止**。只依赖 `RoundExecutor` 抽象（P4 / I38b 落地时由宿主注入）。
- 审批门不在 `agent/`：`ApprovalPrompter` / 授权 / 审计已在 P2 落在 `assistant-tools/src/gateway.rs`；P4 只在宿主侧（`src-tauri/src/agent/`、`src-tauri/src/adapters/`）加 `TimingApprovalPrompter` 装饰器与两表生产实现，**不新建 `assistant-agent/src/agent/approval.rs`**。
- `chat_runtime` / `chat_tasks` / `chat_request` / `prompt_builder`：**迁出后删除**，不作为长期依赖。

**命名**：目录名用 `kernel/`（而非 `turn/`）。理由有三：避免 `turn::TurnRunner` 这类「目录名与类型前缀重复」；与本文「共用内核」同词，路径即术语；把边界写在路径上——`kernel/` 只有 turn 生命周期与端口，策略（停止钩子 / 阶梯 / 审批 / 多迭代循环）归 `agent/`，即 `agent/` 是 `kernel/` 的 client。

---

## 5 与既有文档的关系与迁移顺序

| 文档 | 关系 |
|---|---|
| I18 design / spec | 工具框架与工具生命周期。RoundExecutor 循环契约见 I38b；本文只定义 turn 生命周期与端口 |
| I18b | ToolBox 双入口。ToolBox **不进入**运行时（无 turn、无状态机、无审批），运行时只服务 Agent 入口 |
| I30 | tool call 从 chat 构建剥离。本文的内核抽取**吸收**了 I30 的剥离目标：`kernel/stream.rs` 只留文本/推理/usage，tool 通道归 `agent/` |
| I15 / I16 | 流式累积与 attempt 身份。D1 升级 `attempt_id` 语义，需与 I16 的过期事件丢弃规则保持一致 |
| I20 | 跨窗口事件同步。D2 的状态事件必须走同一广播通道 |
| I17 | 重试与恢复。D4/D5 不得与既有重试叠加（重试仍在 provider 层） |

**迁移顺序**（先抽内核，再长能力；细粒度步骤编号见 spec 的 Step 0B / 1–9）：

1. **端口与值类型**（`kernel/ports.rs`、`ids`、`phase`、`state`、`outcome`）——无行为变化，只新增类型与空实现。
2. **内核原语**（`kernel/tasks.rs`、`kernel/stream.rs`、`kernel/request.rs`、`kernel/persist.rs`、`kernel/emit.rs`）——此时内核无调用方，可单测验证。
3. **chat 改调内核**（`OnBusy::Replace`、`max_iterations = 1`、`tools = None`、`route = Orb`）：`send_chat_message` 内部换成内核调用，**外部行为逐字节不变**（事件名 / `seq` 起点 / 落库时机 / 错误文案）。
4. **删除 chat 旧实现**（`chat_runtime` / `chat_tasks` / `chat_request` / `prompt_builder`），确认无引用后删除。
5. **取消代际化 + 双层超时**（`stream` 接上 `model_call`）。
6. **agent 宿主**（`cmd/agent` + `adapters/*`）：`OnBusy::Reject`、`route = Broadcast`，单迭代形态即可上线，用于验证「无窗口可跑」。
7. **中间件链 + 上下文阶梯**（P4 挂 ①③④⑤，② 与第 10 步同批接入）。
8. **停止钩子 + 收尾调用**。
9. **RoundExecutor 接入，多迭代闭环**（P4 / I38b）：TurnRunner 的单迭代循环替换为 `RoundExecutor::run_iterations`。
10. **压缩从「终止式」切到「续跑式」，并接入阶梯 ②**（D9 第二步，P4 完成后单独提交；含摘要执行体下沉到 `agent/compaction_exec.rs`，I38b D13）。

第 3 步是最大风险点：chat 的对外行为必须不变。因此第 3 步的验收方式是**对比测试**——同一组请求在旧路径与新路径下，事件序列、`seq`、落库内容、错误文案完全一致（用 `EventSink` 测试实现收集后逐条比对）。

**前端编排上移**（与后端步骤解耦，可并行）：

- 先加 `session-state-changed` 订阅，前端**只读**，不删 `ensureIdle()`（双保险）。
- 后端 `begin_turn` 返回 `Busy` 稳定后，再删 `ensureIdle()` 轮询与 `cancelChat` 调度。
- 最后删前端 `phase` 自维护与 `handledTerminalEvents` 去重。

**回退点**：第 3 步前，内核无调用方，回退 = 不合并；第 3 步后若 chat 行为有偏差，回退 = 恢复旧调用路径（旧实现尚未删除）。第 4 步删除后不再有单点回退，需靠对比测试守住。

---

## 6 运行时安全原则（补充 I18 §8）

1. **刹车先于花钱**：预算与上限检查在模型调用之前，不在之后。
2. **取消不 abort**：协作式取消必须让部分结果落库，不允许「点停止 → 内容消失」。
3. **代际不覆盖**：非当前 `turn_id` 的完成结果一律丢弃，不改状态、不写库。
4. **状态单源**：phase 只有后端能写，前端推导视为 bug。
5. **降级不中止**：上下文阶梯的每一步失败都降级到下一步，只有「预算为 0」才终止本轮。
6. **不可判定不等于不超限**：窗口未知时记日志并按保守路径处理，禁止静默跳过。
7. **收尾不越权**：收尾调用不得绕过审批（它不执行工具），也不得放宽单次调用上限。
8. **清扫不阻塞**：启动收敛是尽力而为，失败只记日志。
9. **链路不双写**：同一会话同一时刻只有一个 turn。令牌表在内核里是**唯一**的，chat 与 agent 共用同一张表，靠 `OnBusy` 策略决定顶替或拒绝；不存在「两条链路各自持令牌」的形态。
10. **后台不依赖窗口**：turn 的关键路径不得出现 `AppHandle` / `Window` / 窗口查找（§1.4、§1.6）。新增代码若需要窗口，说明它属于适配层，不属于内核。
11. **事件可丢，事实不可丢**：`EventSink` 失败只记日志并继续；`MessageStore` 失败必须让 turn 失败。任何「因为发不出事件就中止 turn」的写法都是错的。

---

## 7 参考来源

| 设计要素 | 参考项目 | 参考文件 | 借鉴内容 |
|---|---|---|---|
| 会话状态机（§3.2） | OpenBitFun | `execution/agent-runtime/src/session_state.rs`、`session_state_manager.rs` | `Idle / Processing{turn_id,phase} / Error{recoverable}` + `can_start_new_turn` |
| ProcessingPhase（§3.2） | OpenBitFun | 同上 | `Starting/Compacting/Thinking/Streaming/ToolCalling/ToolConfirming` |
| turn 级取消令牌（§3.3） | OpenBitFun | `execution/agent-runtime/src/turn_cancellation.rs` | 按 turn 存令牌、`get_or_insert_new` 复用、`cancel` 幂等 |
| 投机预取与三态认领（§3.13） | OpenBitFun | `execution/agent-runtime/src/compression_prefetch.rs` | `PrefetchClaim{Discarded,Ready,Waiting}`、认领前失败丢弃 |
| 压缩作为 turn item（§3.9） | Codex | `core/src/compact_token_budget.rs` | started/completed 事件 + pre/post hook 可中止 |
| `CompactionTrigger`（§3.9） | Codex | 同上 | `Manual / Auto` 统一生命周期 |
| 保留输出整条保留/丢弃 | Codex | `core/src/compact_retained_tool_outputs.rs` | 与普通历史共享预算，不做半截保留 |
| TTFT 归因（§3.13 暂不做 TTFM） | Codex | `core/src/turn_timing.rs` | 首 token 计时 |
| 双层墙钟上限（§3.4） | OpenHuman | `agent/tinyagents/turn_policy.rs` | turn 3600 s + 单次 900 s，单次取 `min(单次, 轮剩余)` |
| 工具调用上限嵌套取 min（§3.5） | OpenHuman | `agent/stop_hooks.rs` | `tool_call_limit = max_iterations * 8`，嵌套 `min` 收紧 |
| 停止钩子（§3.5） | OpenHuman | `agent/stop_hooks.rs` | `StopHook{name,check(TurnState)}` + 迭代间触发 + 零分配 |
| 中间件链（§3.7） | OpenHuman | `agent/tinyagents/middleware.rs` | `before_model` / `after_tool` 挂载，避免关注点成为死代码 |
| 上下文阶梯顺序（§3.8） | OpenHuman | `agent/tinyagents/harness_context_ladder.rs` | 压缩 → 微压缩 → 收尾还原 → 硬裁剪；顺序踩坑记录 |
| 微压缩预算门控（§3.8） | OpenHuman | 同上 | 不得越过 `keep_recent` 无条件清空 |
| 轮结果计数与 `hit_cap`（§3.6） | OpenHuman | `agent/tinyagents/turn_outcome.rs` | `hit_cap` / `wrap_up_injected` / `breaker_halt` 分开表达 |
| 收尾调用（§3.6） | OpenHuman | 同上 + `FinalCallWrapUpMiddleware` | 最后一次调用撤回工具并追加收尾指令 |
| 启动清扫（§3.11） | OpenHuman | `agent/tinyagents/reaper.rs` | 非终态 run 收敛为 `Cancelled`，尽力而为 |
| 排队输入（§3.13 暂不采纳） | OpenHuman | `agent/queued_turn.rs` | 入队 payload 与 lane 分离、预览截断 |
| 上下文占用分片（§3.13） | OpenHuman | `agent/context_breakdown.rs` | system / tools / history 三段给 UI |
| 空响应重试一次（§3.5） | OpenHuman | `turn_policy.rs` | `empty_response_retries = 1`，与错误重试分离 |
# I36 — Agent 运行时落地规格

| 项 | 值 |
|---|---|
| 状态 | Draft |
| 日期 | 2026-11-03 |
| 关联 | [I36-agent-runtime-design.md](./I36-agent-runtime-design.md)（决策依据）、[I18-agent-tool-architecture-spec.md](./I18-agent-tool-architecture-spec.md) Step 7、[I30-phase1-toolcall-separation-spec.md](./I30-phase1-toolcall-separation-spec.md)、[I18b-tool-unified-bridge-spec.md](./I18b-tool-unified-bridge-spec.md) |
| 影响范围 | `assistant-agent`（新增 `turn/` 共用内核、`agent/` 多迭代能力）、`assistant-config`、`assistant-storage`、`src-tauri/src/cmd/agent`（新增）、`src-tauri/src/adapters`（新增，端口实现）、`src-tauri/src/protocol`、前端 chat 域。**chat 链路对外行为不变**：`cmd/chat`、`chat_runtime`、`chat_tasks`、`chat_request`、`prompt_builder` 在 Step 3 内改调内核，Step 3.4 删除旧实现 |

本文只写**怎么做**。为什么这样分层、参考了什么、取舍是什么，见 design 文档，本文不重复论证。

---

## 实施状态总览

| 模块 | 落点 | 状态 | 说明 |
|---|---|---|---|
| 请求预算卸载 | `assistant-agent/src/request_projection.rs` | 已实现 | 纯函数，阶梯第 ① 步；内核与两条链路共用 |
| 压缩判定 | `assistant-agent/src/compaction.rs` | 已实现 | 纯函数 `evaluate_compaction`；不含摘要执行 |
| 多模态投影 / token 估算 | `multimodal.rs` / `token_estimate.rs` | 已实现 | 纯函数 |
| chat 链路（迁移来源） | `chat_tasks.rs` / `chat_runtime.rs` / `chat_request.rs` / `chat_options.rs` / `prompt_builder.rs` / `normalization.rs` / `cmd/chat` | 已实现 | Step 3 改调内核，Step 3.4 删除 `chat_runtime` / `chat_tasks` / `chat_request` / `prompt_builder`；`chat_options` / `normalization` 保留为无状态函数 |
| **端口定义** | `turn/ports.rs` | 未实现 | `EventSink` / `MessageStore` / `TurnInputSource` / `Clock`，**不得出现 tauri 类型** |
| **端口生产实现** | `src-tauri/src/adapters/` | 未实现 | `event_sink.rs` / `message_store.rs` / `input_source.rs` |
| **TurnSpec（参数化）** | `turn/mod.rs` | 未实现 | `OnBusy` / `max_iterations` / `tools` / `ladder` / `stop_hooks` / `wrap_up` / `route` |
| turn 令牌表 | `turn/tasks.rs` | 未实现 | `OnBusy::{Replace, Reject}`；`turn_id = conv:counter` |
| 流式消费 | `turn/stream.rs` | 未实现 | delta 累积 / 部分结果 / 单次超时 / 取消；不含 tool 通道 |
| turn 级请求构建 | `turn/request.rs` | 未实现 | 吸收 `chat_request` / `prompt_builder` |
| 部分结果与终态落库 | `turn/persist.rs` | 未实现 | 经 `MessageStore` 端口 |
| 事件生成与 seq | `turn/emit.rs` | 未实现 | 经 `EventSink` 端口 |
| 会话状态机 | `turn/state.rs` | 未实现 | chat 链路目前只有布尔 `is_running` |
| turn 值类型 | `turn/ids.rs`、`phase.rs`、`outcome.rs` | 未实现 | — |
| 运行时上限 | `turn/limits.rs` | 未实现 | 双层墙钟尚未接线 |
| TurnRunner | `turn/turn_runner.rs` | 未实现 | 唯一入口；内核无 tauri 依赖 |
| 停止钩子 | `agent/stop.rs` | 未实现 | agent 专属 |
| 中间件链 | `agent/middleware.rs` | 未实现 | agent 专属 |
| 上下文阶梯 ②③④⑤ | `agent/ladder.rs` | 未实现 | ② 的摘要执行体在 `agent/compaction_exec.rs` |
| 摘要执行 | `agent/compaction_exec.rs` | 未实现 | 不调用 `cmd/compaction.rs` |
| 审批门 | `agent/approval.rs` | 未实现 | — |
| 宿主命令 | `src-tauri/src/cmd/agent/` | 未实现 | 只做「校验入参 → begin_turn → spawn → 映射返回值」 |
| 消息终态列 | `assistant-storage` | 未实现 | `messages` 无 status 列 |
| 启动清扫 | `src-tauri/src/setup.rs` 启动序列 | 未实现 | — |
| 轮级观测（TTFT / 汇总） | `turn/outcome.rs` + 事件 | 未实现 | 现有只有 provider usage |

---

## 1 范围与前置

**在范围内**：turn 生命周期、会话状态机、取消与代际、双层超时、停止钩子、中间件链、上下文阶梯、压缩生命周期、终态持久化与启动清扫、轮级观测、**端口化与共用内核抽取**（design §1.5 / §1.6）、**后台可运行**（design §3.13）。

**不在范围内**：工具生命周期（I18 Step 2-6）、RoundExecutor 的循环体（I18 Step 7）、ToolBox 桥接（I18b）、前端 UI 组件（I18 Step 9）、**进程退出后继续运行**（design D13 取舍）。

**前置条件**：

1. `messages` 表可加列（迁移脚本随本规格一起提交）。
2. 摘要执行体在 `agent/compaction_exec.rs` 新建，不依赖 `AppHandle`，也不调用 `cmd/compaction.rs`。
3. **内核零 tauri**：`assistant-agent/src/turn/` 下不得出现 `tauri`、`AppHandle`、`Window`、`tauri::State`，不得 `use` 任何 `src-tauri` 类型；可用 `cargo tree -p assistant-agent` 与 `grep` 双重验证。
4. **端口是唯一出口**：内核访问 DB / 事件 / 输入 / 时钟只经 `turn/ports.rs` 的四个端口（design §1.6）。
5. Step 0B 与 Step 0-2 **不得**改变 chat 链路的可观测行为；Step 3 是 chat 行为等价迁移，Step 4 才删除旧实现。

---

## Step 0 — 值类型

### 0.1 标识

```rust
/// turn 代际 id，由 runtime 自建的 TurnTasks 生成，格式与线字段 attempt_id 一致
/// （`{conversation_id}:{counter}`）。线字段仍叫 `attempt_id`，见 design §3.1。
pub type TurnId = String;

#[derive(Debug, Clone)]
pub struct TurnContext {
    pub conversation_id: String,
    pub turn_id: TurnId,
    /// 轮内迭代序号，从 1 开始；provider 重试不递增。
    pub iteration: u32,
    /// 本次模型调用的 provider 重试次数，由 adapter 上报，只读。
    pub provider_attempt: u32,
    pub started_at: Instant,
}
```

**约束**：`iteration` 与 `provider_attempt` **不落库、不进 `MessageDto`**，只出现在事件与日志里。理由见 design §3.1 / §3.10。

### 0.2 阶段与状态

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TurnPhase {
    Starting,
    Compacting,
    Thinking,
    Streaming,
    ToolCalling,
    AwaitingApproval,
    Finalizing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Idle,
    Processing { turn_id: TurnId, phase: TurnPhase },
    Error { message: String, recoverable: bool },
}
```

`TurnPhase` 是**封闭集合**，用 `enum`；新增 phase 必须同步前端生成类型（`npm run contract:gen`）。

### 0.3 轮结果

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnStatus {
    Completed,
    Cancelled,
    Failed,
    Interrupted,
}

#[derive(Debug, Clone)]
pub struct TurnOutcome {
    pub status: TurnStatus,
    pub text: String,
    pub iterations: u32,
    pub model_calls: u32,
    pub tool_calls: u32,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub cache_read_tokens: Option<u32>,
    /// 因迭代上限结束（最后一次响应仍在请求工具）。
    pub hit_cap: bool,
    /// 最后一次调用已注入收尾指令，`text` 即结论。
    pub wrap_up_injected: bool,
    pub error: Option<TurnError>,
}
```

`hit_cap` 与 `wrap_up_injected` **分开表达**，不合并（design §2.3 / §3.6）：前者表示「还没总结」，后者表示「已经总结完」。

### 0.4 上限与默认值

```rust
#[derive(Debug, Clone)]
pub struct TurnLimits {
    pub max_iterations: u32,
    pub max_tool_calls: u32,
    pub turn_wall_clock: Option<Duration>,
    pub model_call: Option<Duration>,
    pub empty_response_retries: u32,
    pub microcompact_keep_recent: usize,
}
```

| 字段 | 默认 | 来源 | 备注 |
|---|---|---|---|
| `max_iterations` | 12 | 常量 | 工具密集任务常见 3-8 轮 |
| `max_tool_calls` | `max_iterations * 8`（最小 8） | 派生 | 嵌套作用域取 `min`，只收紧 |
| `turn_wall_clock` | 30 min | 常量，`0` 关闭 | 防挂死，非体验指标 |
| `model_call` | 10 min | 常量，`0` 关闭 | 每次 provider 重试重新计时 |
| `empty_response_retries` | 1 | 常量 | 「有推理无正文」重试一次 |
| `microcompact_keep_recent` | 6 | 常量，`0` 关闭 | 保留最近 N 条工具结果体 |

**重试归属**：provider 层重试保持现状（`RetryConfig.max_attempts = 3`，退避由 `retry_after` 决定）。运行时**不得**再实现一层重试，否则退避与错误分类叠加（design §2.3 教训）。

---

## Step 0B — 端口与共用内核骨架

> 这一步先于状态机，因为状态机与 turn 内核都要依赖端口；先有端口，内核才能无 tauri 编译。

### 0B.1 端口契约（`turn/ports.rs`）

```rust
use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;

/// 运行时事件。适配层负责映射为线事件名，内核不写字符串字面量。
pub enum TurnEvent {
    StreamStart { turn_id: String, phase: TurnPhase },
    Delta { turn_id: String, seq: u32, delta: ProviderDelta },
    PhaseChanged { turn_id: String, phase: TurnPhase },
    ToolStarted { turn_id: String, call_id: String, name: String },
    ToolFinished { turn_id: String, call_id: String, ok: bool },
    CompactionStarted { turn_id: String },
    CompactionFinished { turn_id: String, ok: bool },
    Terminated { turn_id: String, status: TurnStatus, error: Option<ChatError> },
    UserMessageConfirmed { conversation_id: String, user_msg_id: String },
}

#[async_trait]
pub trait EventSink: Send + Sync {
    /// 失败只记日志，不得让 turn 失败（design §1.7）。
    async fn emit(&self, event: TurnEvent);
}

#[async_trait]
pub trait MessageStore: Send + Sync {
    async fn create_assistant_message(&self, req: CreateAssistantReq) -> Result<String, StoreError>;
    async fn save_content(&self, msg_id: &str, content: &str, tokens: Option<u32>, reasoning: Option<&str>) -> Result<(), StoreError>;
    async fn finalize(&self, msg_id: &str, status: MessageStatus) -> Result<(), StoreError>;
    async fn record_usage(&self, records: &[UsageRecord]) -> Result<(), StoreError>;
}

#[async_trait]
pub trait TurnInputSource: Send + Sync {
    async fn conversation_detail(&self, conversation_id: &str) -> Result<TurnInput, InputError>;
    /// 只读；标题兜底等写操作留在宿主命令层（design §1.6 第 3 条）。
    async fn provider(&self, provider_id: &str) -> Result<ProviderCredential, InputError>;
    fn attachments(&self) -> Arc<AttachmentStore>;
}

pub trait Clock: Send + Sync {
    fn now(&self) -> std::time::Instant;
}
```

> `assistant-agent/Cargo.toml` 需新增 `async-trait`（workspace 已有，原生 `async fn in trait` 不兼容 `dyn`，见 `assistant-tools/src/trait.rs` 注释）。**不得**为此引入 `tauri`。

**实现位置**：

| 端口 | 生产实现（`src-tauri/src/adapters/`） | 测试实现（`turn/ports/test_doubles.rs`，`#[cfg(test)]`） |
|---|---|---|
| `EventSink` | `event_sink.rs`：`TurnEvent` → `chat-*` / `session-state-changed`，按 `EventRoute` 选 orb / 广播；窗口不存在时静默跳过 | `RecordingSink`（`Vec<TurnEvent>`，供对比测试逐条比对） |
| `MessageStore` | `message_store.rs`：`DbState` 上调用 `conversations::*` / `token_usage::*` | `InMemoryStore` |
| `TurnInputSource` | `input_source.rs`：`AppHandle` state + `crypto::get_machine_id` 解密 key | `FixtureSource` |
| `Clock` | `SystemClock` | `tokio::time::pause` + 手动推进 |

**`EventSink` 的窗口语义**：`route = Orb` 时，窗口不存在 → 直接 return（**不报错**）；`route = Broadcast` 时走 `app.emit`，无订阅者也是 no-op。**禁止**在 sink 里返回 `Err` 让内核中止 turn。

### 0B.2 TurnSpec（`turn/mod.rs`）

```rust
pub enum OnBusy { Replace, Reject }
pub enum EventRoute { Orb, Broadcast }
pub enum LadderPolicy { ProjectionOnly, TerminatingCompact, Full }

pub struct TurnSpec {
    pub on_busy: OnBusy,
    pub max_iterations: u32,
    pub tools: Option<Arc<dyn ToolCatalog>>,
    pub ladder: LadderPolicy,
    pub stop_hooks: Vec<Arc<dyn StopHook>>,
    pub wrap_up: bool,
    pub route: EventRoute,
}

impl TurnSpec {
    /// chat 链路：顶替、1 迭代、无工具、仅投影 + 终止式压缩、orb 路由
    pub fn chat() -> Self;
    /// agent 链路：拒绝、12 迭代、带工具、全阶梯、广播路由
    pub fn agent() -> Self;
}
```

**硬约束**：内核内**不得**出现 `if is_agent` / `if is_chat`。差异只能来自 `TurnSpec` 字段。

### 0B.3 内核模块骨架

```rust
// turn/mod.rs
pub mod ids; pub mod phase; pub mod state; pub mod tasks; pub mod stream;
pub mod request; pub mod persist; pub mod emit; pub mod limits; pub mod outcome;
pub mod ports; pub mod turn_runner;

pub use turn_runner::{TurnRunner, TurnOutcome};
```

| 模块 | 职责 | 不做 |
|---|---|---|
| `turn/tasks.rs` | `begin(conv, spec) -> Result<TurnGuard, StateError>`；`Replace` 取消旧令牌并递增，`Reject` 返回 `Busy` | 不 spawn、不发事件 |
| `turn/stream.rs` | `consume(stream, sink, seq) -> Result<StreamOutcome, StreamError>`；累积文本/推理/usage，单次超时，取消返回 `Cancelled(partial)` | 不建请求、不重试、不落库、不判空响应 |
| `turn/request.rs` | `build(spec, input, iteration) -> ChatRequest`；投影 → 多模态 → 提示词 → tools（`spec.tools`） | 不读 DB（输入由端口给） |
| `turn/persist.rs` | 部分结果 / 终态 / usage 写入；失败即 turn 失败 | 不决定何时写（由 runner 决定） |
| `turn/emit.rs` | `TurnEvent` 生成 + `seq` 单调分配 | 不映射线事件名（适配层做） |
| `turn/state.rs` | 状态机（Step 1） | 不发事件（`emit.rs` 发） |
| `turn/turn_runner.rs` | 唯一入口 `run(ctx) -> TurnOutcome` | 不碰 tauri |

### 0B.4 chat 行为等价契约（Step 3 的验收依据）

迁移后 chat 路径必须满足：

| 维度 | 要求 |
|---|---|
| 事件名 | `chat-delta` / `chat-stream-start` / `chat-messages-changed` / `conversation-list-changed` 不变 |
| `seq` | 起点 0、单调 +1、与现状一致 |
| `attempt_id` | 仍是 `{conv}:{counter}`，计数只增 |
| 落库时机 | 与现状一致：流式结束写一次 + 部分结果在取消/出错时写 |
| 错误文案 | 与现状一致（含 `chat.empty_content`、`error.orb_window_not_found` 的去留需在 Step 3 单独确认：后台运行要求**去掉**该错误，故 chat 命令层改为「窗口缺失也照常发送」） |
| 取消语义 | `OnBusy::Replace`，新消息顶替旧 turn |

**对比测试**：用 `RecordingSink` 跑同一组输入（正常完成 / 中途取消 / provider 报错 / 空响应），逐条比对 `TurnEvent` 序列与 `InMemoryStore` 的写入序列。

---

## Step 1 — 会话状态机

### 1.1 结构与契约

```rust
pub struct SessionStates(Arc<Mutex<HashMap<String, SessionState>>>);

impl SessionStates {
    pub fn ensure_idle(&self, conversation_id: &str);
    pub fn get(&self, conversation_id: &str) -> Option<SessionState>;
    pub fn can_start_new_turn(&self, conversation_id: &str) -> bool;
    pub fn begin_turn(&self, conversation_id: &str, turn_id: &TurnId) -> Result<(), StateError>;
    pub fn set_phase(&self, conversation_id: &str, turn_id: &TurnId, phase: TurnPhase);
    pub fn finish_turn(&self, conversation_id: &str, turn_id: &TurnId, outcome: &TurnOutcome);
    pub fn remove(&self, conversation_id: &str);
}
```

**判据**：

- `can_start_new_turn = Idle | Error { recoverable: true }`（对齐 OpenBitFun）。
- `begin_turn` 在非 `can_start_new_turn` 时返回 `StateError::Busy`，由 runtime 宿主命令转成用户可见错误。chat 链路的「静默顶替旧令牌」语义**保持不动**（只在 chat 内生效，不进入 runtime）。
- `set_phase` / `finish_turn` 校验 `turn_id` 是否为当前代际：不匹配时**静默丢弃**并 `log::warn!`（design §3.3）。
- `finish_turn` 是唯一能把状态从 `Processing` 迁出的入口，且必然写入终态（`Idle` 或 `Error`）。

### 1.2 phase 迁移表

| 从 | 事件 | 到 |
|---|---|---|
| — | `begin_turn` | `Processing{Starting}` |
| `Starting` | 开始构建请求 | `Processing{Thinking}` |
| `Thinking` | 收到首个 delta | `Processing{Streaming}` |
| `Streaming` | 流结束且有 tool_call | `Processing{ToolCalling}` |
| `ToolCalling` | 工具需要审批 | `Processing{AwaitingApproval}` |
| `AwaitingApproval` | 审批通过/拒绝 | `Processing{ToolCalling}` |
| 任意 | 触发压缩 | `Processing{Compacting}` |
| `Compacting` | 压缩完成 | 回到压缩前 phase |
| 任意 | 无 tool_call 且流结束 | `Processing{Finalizing}` |
| `Finalizing` | 落库完成 | `Idle` |
| 任意 | 失败 | `Error{recoverable}` |
| 任意 | 取消 | `Idle` |

### 1.3 与 chat 链路的关系（共用内核，策略不同）

**硬约束**：`TurnTasks` 是**唯一**的令牌表，chat 与 agent 共用（design §1.5）。差异只体现在 `OnBusy` 参数：

```rust
pub struct TurnTasks(Mutex<HashMap<String, TurnEntry>>);

pub struct TurnEntry {
    pub token: CancellationToken,
    pub counter: u32,
    pub turn_id: TurnId,
}

impl TurnTasks {
    /// `Replace`：取消旧令牌、计数 +1、返回新 id（chat 现状）
    /// `Reject`：返回 `Busy`，不动旧令牌（agent）
    pub fn begin(&self, conversation_id: &str, on_busy: OnBusy)
        -> Result<(TurnId, CancellationToken), Busy>;
    pub fn cancel(&self, conversation_id: &str) -> bool;
    /// 恒校验 turn_id，非当前代际不生效（两条链路一致）
    pub fn finish(&self, conversation_id: &str, turn_id: &TurnId);
    pub fn current(&self, conversation_id: &str) -> Option<TurnId>;
}
```

两种策略的语义差异（同一实现，两个参数值）：

| 维度 | chat（`OnBusy::Replace`） | agent（`OnBusy::Reject`） |
|---|---|---|
| 会话已在跑时 | 取消旧令牌、计数 +1、返回新 id | 返回 `Busy` |
| 对应现状 | `ChatTasks::start` 的「顶替」 | 新增行为 |
| `finish` | 校验 `turn_id` 后移除 | 同左 |

**唯一必须一致的东西**：`turn_id` 格式 `{conv}:{counter}`。前端按它解析并丢弃过期事件（I16），格式漂移会让前端要写两套解析。

`SessionStates` 与 `TurnTasks` 的关系：`TurnTasks` 只管「谁在跑」，`SessionStates` 只管「处于哪个阶段」。`begin_turn` 必须先 `TurnTasks::begin` 成功、再 `SessionStates::begin_turn`；后者失败时立即 `TurnTasks::finish` 回滚。

---

## Step 2 — 取消与代际守卫

### 2.1 令牌层级

```
turn token（TurnTasks 持有）
├── model stream child token   → turn::stream::consume
├── tool call child token      → ToolPipeline / exec::invoke
└── approval wait child token  → ApprovalChannel::wait
```

规则：

1. 所有长 await 必须写成 `tokio::select! { biased; _ = child.cancelled() => ..., r = fut => r }`。
2. 取消**只置位**，禁止 `JoinHandle::abort()`。
3. 模型流被取消时，`turn/stream.rs` 的 `consume` 返回 `StreamError::Cancelled(PartialOutcome)`，运行时必须落库部分内容（语义对齐 chat 链路已有的 `save_partial_on_error`，实现统一在 `turn/persist.rs`）。
4. 工具被取消时，工具返回 `ToolErrorKind::Cancelled`（I18 §3.12），回填给模型时写成「已取消」，不当作失败。

### 2.2 代际守卫落点

代际校验必须出现在**三个**写侧，缺一即漏洞：

| 写侧 | 校验 | 失败行为 |
|---|---|---|
| 事件发射 | `payload.turn_id` 与当前代际一致 | 不发射（`log::debug!`） |
| 终态落库 | `SessionStates` 当前 `turn_id` 一致 | 不写库（`log::warn!`） |
| 状态迁移 | 同上 | 静默丢弃 |

**反例**：只在事件层校验、不校验落库 → 旧 turn 的迟到 `completed` 会把新 turn 的消息内容覆盖成旧文本。

---

## Step 3 — TurnRunner（单迭代可用形态）

> 本步骤完成时，`turn/turn_runner.rs` 的外部行为与 chat 链路**等价**：一次用户输入 → 一次模型调用 → 结束。chat 链路在 Step 3 内改调内核，Step 4 才删除旧实现，故本步骤结束后两条链路共用同一内核。

### 3.1 结构

```rust
pub struct TurnRunner {
    states: Arc<SessionStates>,
    tasks: Arc<TurnTasks>,
    limits: TurnLimits,
    sink: Arc<dyn EventSink>,
    store: Arc<dyn MessageStore>,
    input: Arc<dyn TurnInputSource>,
    clock: Arc<dyn Clock>,
    middlewares: MiddlewareChain,
    stop_hooks: Vec<Arc<dyn StopHook>>,
    // RoundExecutor 在 I18 Step 7 就位后注入，此处为 Option
    round_executor: Option<Arc<dyn RoundExecutor>>,
}
```

`TurnRunner::run(ctx, spec) -> TurnOutcome` 是唯一入口。新增宿主 `src-tauri/src/cmd/agent/` 只负责：校验入参 → `TurnTasks::begin` → `spawn(runner.run(...))` → 把 `TurnOutcome` 映射成返回值。**落库与事件发射在内核侧完成**（经端口），宿主不碰。这与 `cmd/chat` 的现状相反，是刻意的：chat 把编排写在命令层，内核不重复这个错误。

### 3.2 主流程

```
run(ctx):
  states.begin_turn(conv, turn_id)?                     // 非 can_start → Busy
  let deadline = now + turn_wall_clock
  let mut outcome = TurnOutcome::new(turn_id)

  loop iteration in 1..=max_iterations:
    // 1) 刹车先于花钱
    if !stop_hooks_all_continue(TurnState{ iteration, tool_calls, tokens, elapsed }) {
        outcome = stopped(reason); break
    }
    if now >= deadline { outcome = timeout("turn wall clock"); break }

    // 2) 上下文阶梯（before_model）
    set_phase(Thinking)
    for mw in middlewares.before_model { if mw stops → outcome = stopped; break outer }

    // 3) 单次模型调用（受 model_call + turn 剩余约束）
    set_phase(Streaming)
    let stream = adapter.chat_completion_stream(..., model_call_budget(deadline), child_token)?;
    let res = turn::stream::consume(stream, child_token, model_call_budget(deadline), |seq, d| emit_delta(seq, d)).await;
    outcome.model_calls += 1
    match res {
        Ok(o)  => accumulate(o),
        Err(StreamError::Cancelled(p)) => { persist_partial(p); outcome = Cancelled; break }
        Err(StreamError::Timeout(p))   => { persist_partial(p); outcome = Failed; break }
        Err(StreamError::Provider(e,p))=> { persist_partial(p); outcome = Failed(e); break }
    }
    if !has_effective_output && empty_response_retries_used < limit {
        empty_response_retries_used += 1; continue   // 不递增 iteration
    }

    // 4) 无工具 → 结束
    let Some(tool_calls) = res.tool_calls else { outcome = Completed; break }

    // 5) 最后一轮：收尾
    if iteration == max_iterations {
        let wrapped = run_wrap_up(tool_calls_withdrawn = true, append_wrap_up_instruction = true)
        outcome.hit_cap = true; outcome.wrap_up_injected = true
        outcome.text = wrapped.text; outcome = Completed; break
    }

    // 6) 工具执行（RoundExecutor / ToolPipeline）
    set_phase(ToolCalling)
    let results = execute(tool_calls, child_token)
    for r in results { middlewares.after_tool(&mut r) }
    append_tool_results(results)
    outcome.tool_calls += n

    // 7) 迭代后钩子
    for mw in middlewares.after_iteration { ... }
  else:
    // 循环耗尽且未收尾 → hit_cap 但无结论（仅当收尾被禁用）
    outcome.hit_cap = true

  set_phase(Finalizing)
  persist_terminal(turn_id, outcome)     // 代际校验后写 messages.status
  states.finish_turn(conv, turn_id, &outcome)
  emit_turn_terminal(outcome)
  return outcome
```

**硬约束**：

- `empty_response_retries` 的 `continue` **不**递增 `iteration`（它不是新迭代，只是同一次调用的重发）。
- 收尾调用占用第 `max_iterations` 个名额，不额外加一轮。
- `persist_terminal` 与 `states.finish_turn` 的先后顺序固定：先落库、后改状态——避免 UI 先看到 Idle 但内容还没写完。
- `adapter.chat_completion_stream` 是 **provider 适配层**接口（`assistant-provider`），与 chat 命令层无关；名字里的 `chat` 是历史命名，不在本轮改。

### 3.3 `turn/stream.rs` 契约

```rust
pub enum StreamError {
    Cancelled(PartialOutcome),
    Timeout(PartialOutcome),
    Provider(ProviderError, PartialOutcome),
}

pub async fn consume<S>(
    stream: S,
    token: CancellationToken,
    timeout: Option<Duration>,
    on_delta: impl FnMut(u32, &ChatDelta) + Send,
) -> Result<StreamOutcome, StreamError>;
```

职责边界（**不做的**事比做的更重要）：

| 做 | 不做 |
|---|---|
| delta 累积成 `StreamOutcome`（正文 / 推理 / usage；tool 通道由 `agent/` 扩展） | 不构建请求（`turn/request.rs` 负责） |
| 单次超时（`model_call` 或 turn 剩余，取小） | 不做 provider 重试（重试在 adapter 层，只一层） |
| 取消响应，返回 `PartialOutcome` | 不落库、不发事件（由 `TurnRunner` + `turn/persist.rs` / `turn/emit.rs` 负责） |
| 单调 `seq` 递增并交给回调 | 不判断「是否空响应」（由 `TurnRunner` 判断） |

`StreamOutcome` / `PartialOutcome` 是**内核自己的类型**，不再借用 `chat_runtime` 的同名类型（否则 chat 删除时会连带删除内核类型）。字段集合对齐现状：`full_content` / `reasoning` / `tool_calls`（仅 agent）/ `usage`。

**chat 迁移时的等价性**：`turn/stream.rs` 必须能复现 `chat_runtime::consume_stream` 的全部可观测行为（`seq` 起点、`on_delta` 调用时机、部分结果内容）。这是 Step 3 对比测试的核心断言。

### 3.4 chat 改调内核并删除旧实现

**顺序**（不可颠倒）：

1. `cmd/chat/mod.rs` 的 `send_chat_message` 内部换成 `TurnRunner::run(ctx, TurnSpec::chat())`，宿主只保留：入参校验 → `begin` → `spawn` → 返回值映射。
2. `spawn_chat_stream` / `spawn_db_writer` / `spawn_finish_listener` / `spawn_compaction_listener` 的职责迁到 `turn/persist.rs` + `turn/emit.rs` + `adapters/`。`cmd/chat/stream.rs` 只剩 `StreamEmitCtx` 到 `EventSink` 的适配（或直接删除）。
3. 对比测试全绿后，删除 `chat_runtime.rs` / `chat_tasks.rs` / `chat_request.rs` / `prompt_builder.rs`，并从 `assistant-agent/src/lib.rs` 移除 `pub mod`。
4. `setup.rs` 的 `manage(ChatTasks::new())` 换成 `manage(TurnTasks::new())`；`cancel_chat` 改调 `TurnTasks::cancel`。

**禁止**：

- 用运行时开关（`if use_kernel { ... } else { ... }`）保留两条路径——双路径会长期存活且行为漂移。
- 删除前不跑对比测试——`chat_streamCharacterization.test.ts` 与 `chatOrchestrator.test.ts` 是前端侧的护栏，后端需自己补 `RecordingSink` 对比测试。

**窗口缺失的行为变更**：`send_chat_message` 不再要求 orb 窗口存在（`error.orb_window_not_found` 从发送路径移除，仅保留给真正需要窗口的命令）。这是**有意的行为变更**，属于「后台可运行」的一部分（design §3.13），需在验收 12 中覆盖。

---

## Step 4 — 中间件链与上下文阶梯

### 4.1 trait 与注册

```rust
#[async_trait]
pub trait TurnMiddleware: Send + Sync {
    fn name(&self) -> &str;
    async fn before_model(&self, req: &mut ModelRequestView<'_>, ctx: &TurnCtx<'_>) -> MiddlewareOutcome;
    async fn after_tool(&self, res: &mut ToolResultView<'_>, ctx: &TurnCtx<'_>) -> MiddlewareOutcome;
    async fn after_iteration(&self, ctx: &TurnCtx<'_>) {}
}

pub enum MiddlewareOutcome { Continue, StopTurn { reason: String } }
```

`MiddlewareChain` 按注册顺序执行；任一 `StopTurn` 立即中止该阶段并结束本轮（`TurnStatus::Failed`，`error = TurnError::MiddlewareStop{name,reason}`）。

### 4.2 注册顺序（固定）

| 序 | 中间件 | 钩子 | 职责 |
|---|---|---|---|
| 1 | `ProjectionMiddleware` | `before_model` | 阶梯 ①：调用现有 `offload_if_over_budget` |
| 2 | `SummarizeMiddleware` | `before_model` | 阶梯 ②：摘要压缩 |
| 3 | `MicrocompactMiddleware` | `before_model` | 阶梯 ③：清空较旧工具结果体 |
| 4 | `WrapUpRestoreMiddleware` | `before_model` | 阶梯 ④：capped turn 结论调用还原 |
| 5 | `HardTrimMiddleware` | `before_model` | 阶梯 ⑤：整条淘汰最旧消息 |
| 6 | `ToolOutputCapMiddleware` | `after_tool` | 单条工具结果字节上限（I18 §3.13） |
| 7 | `ToolOutcomeCaptureMiddleware` | `after_tool` | 捕获 `success` / `call_id` / 原始参数，供 `TurnOutcome` 汇总 |
| 8 | `RepeatedFailureMiddleware` | `after_iteration` | 连续失败熔断（同工具同参数失败 N 次 → StopTurn） |

**顺序是语义**，不是实现细节：3 必须在 2 之后（否则摘要器读到的是占位符），4 必须在 3 之后（否则还原的是空内容）。

### 4.3 阶梯门控公式

```
budget = context_window - reserved_output - COMPACTION_SAFETY_TOKENS(1000)
// 窗口未知：不压缩，只跑 ①⑤，log::warn! 一次
if context_window.is_none() { run(Projection, HardTrim); return }

// ② 摘要压缩
if estimated >= budget * COMPACTION_THRESHOLD_PERCENT / 100 {
    run(Summarize)                       // 失败 → 降级到 ③，不中止
}

// ③ 微压缩：仅在 ② 已执行或不可用，且估算仍超预算时
if estimated > budget && microcompact_keep_recent > 0 {
    blank_older_tool_bodies(keep_recent = N)
}

// ⑤ 硬裁剪：整条淘汰最旧消息，system 消息永不淘汰，图片按固定成本计价
while estimated > budget && evictable_oldest_exists() { evict() }
```

**微压缩占位符**：`[tool result cleared: {tool_name}, {tokens} tokens]`——保留工具名与规模，让模型知道「这里发生过什么」，避免凭空丢上下文。

**禁止**：微压缩在未超预算时执行；摘要器读取已被清空的结果体。

### 4.4 与现有 `compaction.rs` 的对接

现有 `evaluate_compaction(prompt_tokens, window, auto_compact, threshold_percent_of_budget, reserved_output) -> CompactDecision` 继续作为**唯一判定函数**，运行时不得另写一套阈值逻辑：

| 判定 | 运行时动作 |
|---|---|
| `NotNeeded` | 跳过 ② |
| `Terminate` | 执行 ②（当前终止式：压缩后另起；见 Step 6） |
| `Undetermined` | 跳过 ②，`log::warn!`，继续 ①③⑤ |

---

## Step 5 — 停止钩子与预算

### 5.1 契约

```rust
pub trait StopHook: Send + Sync {
    fn name(&self) -> &'static str;
    fn check(&self, state: &TurnState<'_>) -> StopDecision;
}

pub struct TurnState<'a> {
    pub iteration: u32,
    pub max_iterations: u32,
    pub tool_calls: u32,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub elapsed: Duration,
    pub model: &'a str,
}
```

- `check` 是**同步**的，禁止 IO、禁止 await（design §3.5）。
- `TurnState` 借出循环局部变量，钩子不得持有引用跨迭代。

### 5.2 内置钩子

| 钩子 | 触发条件 | Stop 原因 |
|---|---|---|
| `IterationLimitHook` | `iteration > max_iterations` | 冗余保险（主循环已判） |
| `ToolCallLimitHook` | `tool_calls >= max_tool_calls` | `tool-call-limit` |
| `TurnTokenBudgetHook` | `prompt_tokens + completion_tokens >= turn_token_budget` | `token-budget` |

`turn_token_budget` 从配置读取，未配置时不安装该钩子（不设默认值，避免误杀）。

### 5.3 嵌套收紧

`max_tool_calls` 在嵌套作用域（未来子代理）中取 `min(内层, 外层)`，对齐 OpenHuman：

```rust
pub fn effective_tool_call_limit(inner: u32, outer: Option<u32>) -> u32 {
    outer.map_or(inner, |o| inner.min(o))
}
```

当前无子代理，`outer` 恒为 `None`；函数先落地以保证未来不会写出「内层放宽」的实现。

---

## Step 6 — 压缩生命周期

### 6.1 触发枚举

```rust
pub enum CompactTrigger { Manual, Auto }
```

手动（`manual_compact_conversation`）与自动（轮内阶梯 ②）**共用**同一生命周期与同一套事件，只在 `trigger` 字段上区分。

### 6.2 生命周期

```
set_phase(Compacting)
emit compaction-started { turn_id, trigger }
pre_compact_hook()            // 返回 Stop → TurnStatus::Failed，本轮中止
run_summarize()               // agent/compaction_exec.rs，独立实现（不调用 cmd/compaction.rs）
emit compaction-completed { turn_id, trigger, before_tokens, after_tokens }
post_compact_hook()           // 返回 Stop → TurnStatus::Failed
restore_phase(previous)
```

**失败降级**：`run_summarize` 失败时**不中止本轮**，`log::warn!` 后降级到阶梯 ③⑤（对齐 OpenHuman 的 `FaultTolerantCachingSummarizer` 行为）。只有 hook 主动 Stop 才中止。

### 6.3 过渡期语义

本规格落地时保持**终止式**（`CompactDecision::Terminate` → 压缩完成后另起），轮内续跑等 I18 Step 7 就位后再切（design §3.9 第二步）。切换时只改 `run_summarize` 之后的分支，不改生命周期与事件。

---

## Step 7 — 事件与 DTO

### 7.1 事件清单

| 事件名 | 时机 | 载荷要点 |
|---|---|---|
| `chat-stream-start`（已有名） | `begin_turn` 后 | `conversation_id` / `attempt_id`(=turn_id) / `message_id` / **新增 `phase`** |
| `session-state-changed`（新增） | 任意 phase 或 Idle/Error 迁移 | `conversation_id` / `turn_id` / `state` |
| `chat-delta`（已有名） | 流式增量 | 追加 `iteration`（u32） |
| `agent-tool-call`（I18 Step 9 定义） | 工具开始/结束/等审批 | `turn_id` / `iteration` / `call_id` |
| `compaction-started` / `compaction-completed`（新增） | 压缩 | `turn_id` / `trigger` / 前后 token |
| `chat-stream-end` / 终止类（已有名） | 终态 | `turn_id` / `status` / `outcome 摘要` |

> **内核只产出 `TurnEvent`，不写事件名字面量**（design §1.6）。映射在 `src-tauri/src/adapters/event_sink.rs`：`TurnEvent::Delta` → `chat-delta`、`TurnEvent::StreamStart` → `chat-stream-start`、`TurnEvent::PhaseChanged` → `session-state-changed`，等等。沿用 `chat-*` 名字是为了前端 store 与订阅逻辑共享同一套通道与类型；这是**线契约兼容**，不是代码复用。

**路由**：`TurnSpec.route` 决定 orb 还是广播。`route = Orb` 且窗口不存在时**静默跳过**（design §3.13）；`route = Broadcast` 走 `app.emit`，无订阅者是 no-op。

### 7.2 排序与过滤

- 每条事件带 `turn_id`（线字段 `attempt_id`）与 `seq`。
- `seq` 在同一 turn 内**单调递增**，跨 turn 重新从 0 开始。
- 前端按 `turn_id` 过滤、按 `seq` 排序；`session-state-changed` 不带 `seq`（它是状态快照，不是流片段）。

### 7.3 DTO 变更要求

1. `ChatDeltaPayloadDto` 增加 `iteration: u32`（输出 DTO，非 `Option`，默认 0 表示非 agent 轮）。
2. `ChatStreamStartPayloadDto` 增加 `phase: TurnPhaseDto`。
3. 新增 `SessionStateChangedPayloadDto`（`conversation_id` / `turn_id` / `state: SessionStateDto`）。
4. `SessionStateDto` 是**封闭枚举**（`idle` / `processing` / `error`），`processing` 内含 `turnId` + `phase`；`error` 内含 `message` + `recoverable`。
5. 改完必须执行 `npm run contract:gen`，并提交 `src/types/protocol.contract.ts`。
6. 输出 DTO 的 `Option` 字段**不加** `skip_serializing_if`（键恒在、值 `null`）；输入 DTO 的 `Option` 加 `#[ts(optional = nullable)]`。
7. `TurnPhase` 是封闭集合 → Rust `enum` + `serde(rename_all = "camelCase")`；前端不得兜底窄化，未知值直接视为 bug 并记日志。

---

## Step 8 — 终态持久化与启动清扫

### 8.1 迁移

```sql
ALTER TABLE messages ADD COLUMN status TEXT NOT NULL DEFAULT 'completed';
```

- 取值：`streaming` / `completed` / `interrupted` / `failed`。
- 新建助手消息时写 `streaming`；`persist_terminal` 改写为终态。
- 用户消息恒为 `completed`。

### 8.2 写入时机

| 时机 | 内容 | status |
|---|---|---|
| 创建助手消息 | 空内容 | `streaming` |
| 流式出错但有部分内容 | `PartialOutcome` 内容 | `failed`（附 `error` 事件） |
| 正常结束 | `TurnOutcome.text` | `completed` |
| 用户取消 | 已累积内容 | `interrupted` |
| 迭代上限且未收尾 | 已有内容 | `completed` + `hit_cap` 事件（不是失败） |

### 8.3 启动清扫

```
启动序列（setup.rs，DB 可用之后、主窗口创建之前）:
  UPDATE messages SET status='interrupted' WHERE status='streaming'
  清理 SessionStates 残留（进程内新实例，天然为空）
  失败 → log::warn!，继续启动（不重试、不阻塞）
```

**不做**的事：不推断「停在第几轮」、不尝试续跑、不删除任何消息。

---

## Step 9 — 观测

### 9.1 轮级汇总

每次 `finish_turn` 输出一条结构化日志（`info` 级），字段固定：

```
[turn] conv=.. turn=.. status=.. iterations=.. model_calls=.. tool_calls=..
       prompt=.. completion=.. cache_read=.. ttft_ms=.. elapsed_ms=.. hit_cap=..
```

### 9.2 TTFT

在 `turn/stream.rs` 的 `consume` 首次回调处记录 `now - request_started`，写入 `TurnOutcome` 与上述日志。**不**引入 TTFM（design §3.16）。

### 9.3 上下文占用

沿用 provider usage（`UsageSnapshot.context_window_tokens`，内核侧同名独立类型），不做 system/tools/history 三段拆分（design §3.16 暂不采纳）。

---

## 10 验收标准

按 Step 顺序，每条可独立验证。

**Step 0B（端口与内核）**

1. `cargo tree -p assistant-agent` 中**无** `tauri`；`assistant-agent/src/turn/` 下 grep 无 `AppHandle` / `Window` / `tauri::State`。
2. 内核可只用测试替身（`RecordingSink` / `InMemoryStore` / `FixtureSource` / 暂停时钟）跑完一整轮，不启动 Tauri。
3. `EventSink` 实现抛错/窗口缺失时，turn **不中止**，日志有记录（design §1.7）。
4. `MessageStore` 失败时 turn 失败并写 `status='failed'`（事实源不可丢）。
5. `TurnSpec::chat()` / `TurnSpec::agent()` 的字段值符合 §0B.2；内核代码中 grep 无 `is_agent` / `is_chat`。

**Step 1-3（单迭代可用 + chat 等价迁移）**

6. chat 对比测试：同一组输入（正常完成 / 中途取消 / provider 报错 / 空响应）在旧路径与新内核路径下，`TurnEvent` 序列、`seq`、落库内容、错误文案逐条一致。
7. 新入口（`cmd/agent` 的发送命令）在 `Processing` 状态下返回 `Busy`，且不取消正在跑的 turn；`OnBusy::Replace` 下（chat）仍是顶替语义。
8. 取消后消息内容落库、`status='interrupted'`、状态回到 `Idle`。
9. 旧 turn 的迟到完成：手动构造（旧 token 完成晚于新 turn 开始）→ 不写库、不改状态、不发终止事件。
10. `turn_wall_clock` 与 `model_call` 可分别触发超时；工具执行不受 `model_call` 限制。
11. 「有推理无正文」响应重试 1 次后仍为空 → `Failed`，且 `iteration` 不因重试递增。
12. **后台运行**：关闭 orb 窗口（或从未创建）后调用 agent 发送命令，turn 正常跑完并落库；全程无「窗口未找到」错误。

**Step 4（阶梯）**

13. 构造「窗口放得下全部工具结果」的轮：微压缩**不**触发（回归 OpenHuman 的门控教训）。
14. 构造「摘要 + 最近窗口仍溢出」：按 ②→③→⑤ 顺序各触发一次，最终请求不超预算。
15. 摘要器失败：本轮不中止，降级到 ③⑤，日志含降级原因。
16. 窗口未知：不调用摘要器，日志出现一次 `Undetermined` 告警。

**Step 5-6**

17. `max_tool_calls` 触发时，回填给模型的最后一条消息是「已达工具调用上限」，且本轮仍产出文本。
18. 迭代上限触发收尾：最后一次请求不含 `tools`，且 `wrap_up_injected = true`、`hit_cap = true`。
19. 手动压缩与自动压缩产生相同的 `compaction-started/completed` 事件序列，仅 `trigger` 不同。
20. pre-compact hook 返回 Stop → 本轮 `Failed`，不进入摘要执行。

**Step 7-8**

21. 每条事件都带 `turn_id`；同 turn 内 `seq` 严格单调。
22. 强制结束进程（流式中）后重启：该消息 `status='interrupted'`，UI 不再显示「正在生成」。
23. `npm run contract:gen` 后前端类型可编译，`cargo test --test assembly_contract` 通过。

---

## 11 风险与开放问题

| 风险 | 影响 | 缓解 |
|---|---|---|
| `attempt_id` 语义升级为 turn 代际 | 命名与语义偏差，读代码易误解 | 文档 + 类型别名；重命名留作独立迁移 |
| **chat 迁移期行为漂移** | chat 事件序列 / `seq` / 落库时机任一变化，前端与既有测试全崩 | 对比测试（验收 6）；Step 3 保留旧路径可回退，Step 4 前必须全绿 |
| **迁移期双路径并存** | 同一时刻两条路径都可能被调用，行为不一致 | `send_chat_message` 内**只允许一条路径被编译**（cfg 或直接替换），不做运行时开关 |
| **端口抽象泄漏** | 有人在 `turn/` 里塞 `AppHandle`，后台运行悄悄失效 | 前置条件 3 的 grep / `cargo tree` 断言进 CI；code review 检查 `turn/` 的 import |
| `EventSink` 失败被当成致命错误 | 窗口关闭导致 turn 中止 | design §1.7 写死「事件可丢」；验收 3 |
| 前端仍保留 `ensureIdle()` 轮询 | 双保险期与后端 `Busy` 语义叠加，可能出现「前端等 5 s 后端已拒绝」 | 分三步删（design §5 前端编排上移），每步观察一轮 |
| 收尾调用多花一次请求 | 成本与延迟增加 | 仅在真正触顶时发生；`hit_cap` 事件让用户可见 |
| 压缩从终止式切续跑式 | 切换期两套语义 | 生命周期与事件先统一，切换只改一个分支 |
| `messages.status` 新增列 | 旧库迁移 | `NOT NULL DEFAULT 'completed'`，旧行语义正确 |
| 微压缩清空工具结果 | 模型后续可能引用已清空内容 | 占位符保留工具名与规模；收尾调用做还原（④） |

**开放问题**（待 RoundExecutor 就位后定）：

1. 工具审批等待期间，`turn_wall_clock` 是否继续计时？当前倾向「继续计时」，因为用户可能长时间不响应；需与审批超时（I18 §3.9）合并考虑。
2. 收尾调用是否需要读取被微压缩清空的工具结果？当前倾向「需要」（阶梯 ④），但会推高收尾调用的输入量。
3. 压缩完成后是否重置 `iteration` 计数？当前倾向「不重置」，压缩不改变「这一轮已经用掉几次模型调用」的事实。
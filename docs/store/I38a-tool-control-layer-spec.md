# I38a — P2：工具控制层规格

| 项 | 值 |
|---|---|
| 状态 | 现行（P2 工具控制层规格；进度见 [roadmap](./I38-agent-roadmap.md) §2） |
| 日期 | 2026-11-04 |
| 阶段 | P2（见 [I38 roadmap](./I38-agent-roadmap.md)） |
| 关联 | [I18 design](./I18-agent-tool-architecture-design.md) §3.5 / §3.9 / §3.12-§3.15、[I18 spec](./I18-agent-tool-architecture-spec.md) Step 4-6（已迁移至本文）、[I31](./I31-phase1-tool-framework-spec.md) §5（`os/path.rs`） |
| 影响范围 | `assistant-tools` crate（新增 `policy.rs` / `gateway.rs` / `pipeline.rs` / `storage.rs`）；`trait.rs` / `result.rs` 增量扩展。**无 tauri 依赖** |

> 本文是 P2 的唯一权威规格。I18 spec 的 Step 4-6 已迁入本文并在原文件删除，避免双份定义。
> 决策依据（为什么取最严、为什么先校验后审批）见 I18 design §3.5 / §3.9 / §3.14 / §3.15，本文不重复论证。

> 执行清单：[I38a-tool-control-layer-tasklist.md](./I38a-tool-control-layer-tasklist.md)（C0–C8 逐项勾选与验收映射）。

---

## 1 目标与非目标

### 1.1 目标

| # | 交付 |
|---|---|
| 1 | 纯决策的权限引擎：`Allow / Ask / Deny` + 多层取最严 |
| 2 | 权限网关：意图收集 → 记忆授权 → 策略评估 → 审批 → 审计 |
| 3 | 工具流水线：查找 → 校验 → 网关 → 执行 → 持久化 → 事件 |
| 4 | 审批键规范化，防「批准 A 放行 B」 |
| 5 | 结果双通道与错误分类（`result_for_assistant` / `error_kind`） |
| 6 | 并发批次编排、单工具/单轮输出预算与落盘 |

### 1.2 非目标

- RoundExecutor、多迭代循环、turn 生命周期（P4 / P3）。
- 任何具体 OS 工具实现（P5）；本阶段用测试替身工具。
- Tauri 事件、前端审批 UI（P6）。网关只依赖审批端口，不感知窗口。
- MCP（P7）。
- 新 DB 表。审计存储只定义端口，生产实现随 P4 落库方案确定。

### 1.3 前置条件

1. P1 完成：`assistant-tools/src/os/path.rs` 可用，`fs_link` 不再持有私有路径实现。
2. `Tool` trait 的现有实现可继续工作：本阶段新增方法必须有默认实现。
3. 现有 `exec::invoke` 保留为**唯一执行入口**；Pipeline 复用 `invoke`，不直接调 `execute()`。

---

## 2 类型契约

### 2.1 PermissionEffect（`policy.rs`）

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionEffect {
    Allow,
    Ask,
    Deny,
}

impl PermissionEffect {
    pub const fn most_restrictive(self, other: Self) -> Self {
        match (self, other) {
            (Self::Deny, _) | (_, Self::Deny) => Self::Deny,
            (Self::Ask, _) | (_, Self::Ask) => Self::Ask,
            (Self::Allow, Self::Allow) => Self::Allow,
        }
    }
}
```

### 2.2 PermissionRule / 约束层

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRule {
    pub action: String,   // read / edit / delete / execute / network / external / *
    pub resource: String, // 路径 glob / 命令名 / URL pattern / tier:<tier> / *
    pub effect: PermissionEffect,
}

pub struct PermissionConstraintLayer {
    pub rules: Vec<PermissionRule>,
}
```

**匹配语义**：

1. 同一规则集内 **last-match-wins**：宽泛规则在前，具体规则在后覆盖。
2. 不同规则集之间取 `most_restrictive`。
3. 未命中任何规则时默认 `Allow`（单层内），但默认全局主规则集的首条是 `("*", "*", Ask)`，因此整体仍是「默认需审批」。

### 2.3 ResolvedPermissionPolicy

```rust
#[derive(Debug, Clone, Default)]
pub struct ResolvedPermissionPolicy {
    rules: Vec<PermissionRule>,
    constraint_layers: Vec<PermissionConstraintLayer>,
}

impl ResolvedPermissionPolicy {
    pub fn evaluate(&self, action: &str, resource: &str) -> PermissionEffect;

    /// P6 新增：返回命中原因，供审批卡片回答「为什么问我」。
    /// 不改变 evaluate 的结果，仅旁路记录；实现必须与 evaluate 共用同一匹配逻辑。
    pub fn explain(&self, action: &str, resource: &str) -> Vec<ApprovalReason>;
}
```

评估算法：

```
result = eval_rules(self.rules, action, resource)        // last-match-wins
for layer in constraint_layers:
    result = result.most_restrictive(eval_rules(layer.rules, action, resource))
return result
```

`explain` 的产出规则：

- 逐层记录**实际改变结果**的那条规则（last-match-wins 的获胜规则），不记录所有被覆盖的规则。
- `result == Ask` 且无任何显式规则命中 → 返回一条 `DefaultPolicy { action, resource }`（默认策略即「未知一律问」）。
- `explain` **不得**有副作用、不得影响 `evaluate` 的返回值；两者必须由同一 `eval_rules` 实现驱动，禁止各写一份匹配逻辑（否则会出现「卡片说 A、实际按 B」）。

### 2.4 PermissionRuntimeCeiling

```rust
pub struct PermissionRuntimeCeiling {
    rules: Vec<PermissionRule>,
}

impl PermissionRuntimeCeiling {
    pub fn try_new(rules: Vec<PermissionRule>) -> Result<Self, CeilingValidationError>;
}
```

**硬约束**：`try_new` 发现任何 `effect == Allow` 的规则即返回 `Err`。委托只能收紧，不能放宽。

### 2.5 默认规则集

```rust
fn default_global_rules() -> Vec<PermissionRule> {
    vec![
        PermissionRule::new("*", "*", PermissionEffect::Ask),
        PermissionRule::new("read", "*", PermissionEffect::Allow),
        PermissionRule::new("read", "/etc/shadow", PermissionEffect::Deny),
        PermissionRule::new("read", "/etc/ssh/**", PermissionEffect::Deny),
        PermissionRule::new("edit", "/etc/**", PermissionEffect::Deny),
        PermissionRule::new("edit", "C:/Windows/**", PermissionEffect::Deny),
        PermissionRule::new("edit", "C:/Program Files/**", PermissionEffect::Deny),
    ]
}
```

项目级配置 `.assistant/permissions.json` 的结构与加载优先级（应用默认 → 项目 → Agent 模式）保持 I18 design §3.11 的定义；解析产物是一个 `PermissionConstraintLayer`，不在此处重复定义格式。

### 2.6 ApprovalKey（`gateway.rs`）

```rust
pub struct ApprovalKey {
    pub tool: String,
    pub scope: ApprovalScope,
}

pub enum ApprovalScope {
    Path { canonical: String },
    Command { argv: Vec<String>, cwd: String, sandbox_tier: ToolTier },
    Endpoint { host: String, method: String },
    Opaque { fingerprint: String },
}
```

| 规则 | 要求 |
|---|---|
| 路径归一 | `canonicalize` 后比较；符号链接必须解析到真实目标 |
| 命令归一 | 分词为完整 argv，`cwd` 与沙箱档位入键；**禁止默认前缀匹配** |
| 空键不命中 | 无法生成键时直接走审批，不查缓存 |
| 生命周期 | 会话级；跨会话记忆走 `AlwaysApprove` 持久化 |
| 委托 | 子 Agent 可读父 `GrantStore`，新增批准不写父库 |

### 2.7 Gateway 决策

```rust
pub enum GatewayDecision {
    Allowed,
    Denied { action: String, resource: String },
    RejectedByUser { reason: Option<String> },
}

pub enum ApprovalDecision {
    Approve,
    Deny { reason: Option<String> },
    AlwaysApprove,
}
```

### 2.8 审批端口（**本文新增决策**）

`assistant-tools` 不得依赖 tauri。审批等待抽象为端口：

```rust
#[async_trait]
pub trait ApprovalPrompter: Send + Sync {
    async fn request(&self, request: ApprovalRequest) -> ApprovalDecision;
}

pub struct ApprovalRequest {
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub intents: Vec<PermissionIntent>,
    pub timeout: Duration,
    // P6 修订（跨阶段契约变更，见 §2.8.1）
    pub tier: ToolTier,
    pub behavior: BehaviorHints,
    pub reasons: Vec<ApprovalReason>,
    pub preview: Option<ApprovalPreview>,
}
```

- 生产实现（P6）：发 `agent-tool-awaiting-approval` 事件 → 前端审批 → Tauri command 回填决策。
- 测试实现：脚本化返回 `Approve / Deny / AlwaysApprove / 超时`。
- **超时语义**：超时视为 `Deny`，不得默认放行，也不得无限挂起。
- **预留扩展（不实现）**：AI 审批顾问（`ApprovalAdvisor`）不在 P2 范围，P2–P7 也**不预留空 trait / 空字段**。插入点（策略之后、人审批之前）、方向性约束（`fuse` 纯函数）、输入分层与启用门禁见 [I18 design §3.17](./I18-agent-tool-architecture-design.md)；本节 `ApprovalPrompter` / `ApprovalDecision` / `GatewayDecision` 契约不因此改变。

#### 2.8.1 P6 修订：审批载荷携带用户判断信息

**动机**：P2 的 `ApprovalRequest` 只携带「要做什么」（`arguments` + `intents`），无法回答「为什么问我」「有多严重」「能不能撤销」。L0（`tier` / `behavior`）在注册期已知却未随载荷传出，属于零成本信息遗漏；L2（预演）则因为审批发生在 `execute()` 之前，无法复用执行结果。

```rust
pub struct ApprovalReason {
    pub kind: ApprovalReasonKind,
}

pub enum ApprovalReasonKind {
    /// 命中策略规则（含默认的 `("*","*", Ask)`）。
    DefaultPolicy { action: String, resource: String },
    /// 命中 AgentMode 的 approval_level 收紧规则。
    ApprovalLevel { level: String },
    /// 命中项目级 `.assistant/permissions.json` 规则。
    ProjectRule { action: String, resource: String },
    /// 被 tier ceiling 收紧。
    TierCeiling { tier: ToolTier },
    /// AI 顾问加码（P10 才产生）。
    Advisor { rationale: String },
}

pub struct ApprovalPreview {
    pub summary: String,
    pub details: Vec<ApprovalPreviewItem>,
    pub truncated: bool,
    pub estimated: bool,
}

pub struct ApprovalPreviewItem {
    pub label: String,
    pub value: String,
}
```

**装配规则**：

| 字段 | 来源 | 缺失语义 |
|---|---|---|
| `tier` / `behavior` | `tool.tier()` / `tool.behavior_hints()` | 不可缺失 |
| `reasons` | `ResolvedPermissionPolicy::explain(action, resource)`（新增，见 §2.3） | 恒非空，至少一条 `DefaultPolicy` |
| `preview` | `tool.preview_for_approval(input, ctx)`（新增，见 §3.3） | `None` = 信息未知，**不等于零** |

**约束**：

- `preview_for_approval` **只在网关判定为 `Ask` 之后调用**；`Allow` / `Deny` 路径不调用（避免给全量工具调用增加 IO）。
- 预演超时（建议 2s）或 `Err` → `preview = None`，审批继续。**信息缺失可以，决策缺失不行**。
- 预演**不得**改变决策、不得写 grant、不得读取文件全文。
- 预演耗时单记，不得混入 `TimingApprovalPrompter` 的 `confirmation_wait_ms`。
- 存量影响：`DenyAllApprovalPrompter` 与 `tests/gateway.rs` 的 `ApprovalRequest` 构造点需同步补字段；因新字段全部由 Gateway 填充，`ApprovalPrompter` 实现方无需改动逻辑。

### 2.9 存储端口

```rust
#[async_trait]
pub trait PermissionGrantStore: Send + Sync {
    async fn is_granted(&self, key: &ApprovalKey) -> bool;
    async fn save_grant(&self, key: &ApprovalKey) -> Result<(), String>;
}

#[async_trait]
pub trait PermissionAuditStore: Send + Sync {
    async fn log_decision(&self, entry: AuditEntry) -> Result<(), String>;
}

#[async_trait]
pub trait ToolResultStorage: Send + Sync {
    async fn persist(&self, req: PersistToolResult) -> Result<PersistedRef, String>;
}
```

三者均为端口：P2 提供内存实现与文件实现（`ToolResultStorage` 的根目录由构造参数注入），生产 DB 绑定随 P4 落地。

---

## 3 Tool trait / ToolResult 扩展

### 3.1 新增方法（带默认实现，存量工具零改动）

```rust
pub struct ValidationResult {
    pub ok: bool,
    pub message: Option<String>,
    pub error_code: Option<String>,
    pub blocks_input_rewrite: bool,
}

fn validate_input(&self, _input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
    ValidationResult::ok()
}

fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
    self.is_readonly()
}

/// P6 新增：审批预演。仅在网关判定为 Ask 后调用。
/// 返回 None 表示「不提供预演信息」，不是「零」。
async fn preview_for_approval(
    &self,
    _input: &Value,
    _ctx: &ToolUseContext,
) -> Option<ApprovalPreview> {
    None
}
```

| 方法 | 调用时机 | 约束 |
|---|---|---|
| `validate_input` | Pipeline 的 validate 阶段 | 纯检查、无 IO；路径检查必须复用 `os/path.rs` |
| `is_concurrency_safe` | Pipeline 分批 | 默认等于 `read_only`；工具可按 input 覆写 |
| `preview_for_approval` | **仅** Gateway 判定 `Ask` 后、人工审批前 | 可做只读 IO；2s 上限；失败/超时返回 `None`；不得写 grant、不得读全文、不得改变决策 |

`preview_for_approval` 的默认实现返回 `None`，存量工具零改动；P5 的 `delete` / `write` / `edit` 各自覆写（[I38c §4.1](./I38c-os-tools-spec.md)）。

### 3.2 ToolResult 双通道

```rust
pub struct ToolResult {
    pub ok: bool,
    pub data: Option<Value>,                  // UI 渲染
    pub result_for_assistant: Option<String>, // 回填模型的精简文本
    pub error: Option<String>,
    pub error_kind: Option<ToolErrorKind>,
}

pub enum ToolErrorKind { Business, Timeout, Cancelled, Internal, Denied }
```

- `result_for_assistant` 缺省时由 `data` 的紧凑序列化兜底，存量工具不强制实现。
- `error_kind` 由 `invoke` 与 Pipeline 打标：超时 → `Timeout`，取消 → `Cancelled`，工具 `Err` → `Internal`，网关拒绝 → `Denied`，业务失败 → `Business`。
- **wire 约束**：`OsToolResultDto` 同步新增 `resultForAssistant` / `errorKind` 字段；输出 DTO 的 `Option` 不加 `skip_serializing_if`（键恒在、值 `null`），前端声明 `T | null`。改 DTO 后执行 `npm run contract:gen`。

---

## 4 实现步骤

### P2-C1 — Policy Engine

1. 新建 `assistant-tools/src/policy.rs`：`PermissionEffect` / `PermissionRule` / `PermissionConstraintLayer` / `ResolvedPermissionPolicy` / `PermissionRuntimeCeiling` / 默认规则集。
2. 单测：`most_restrictive` 九种组合；last-match-wins；多层取最严；Ceiling 拒绝 Allow；默认规则对 `read` / `edit /etc/**` 的判定。

### P2-C2 — 审批与存储端口

1. 新建 `gateway.rs` 的类型部分：`ApprovalKey` / `ApprovalScope` / `GatewayDecision` / `ApprovalDecision` / `ApprovalPrompter` / 三个 Store trait。
2. 内存实现（测试用）与文件实现（结果落盘用）。
3. 单测：键归一（路径 canonical、命令 argv、空键）、超时 → Deny、`AlwaysApprove` 写 GrantStore。

### P2-C3 — Tool trait / ToolResult 扩展

1. `trait.rs` 增加 `validate_input` / `is_concurrency_safe` 默认实现。
2. `result.rs` 增加双通道字段与 `ToolErrorKind`；`from_exec_error` 支持指定 kind。
3. `exec::invoke` 打标：超时 / `Err` 归一化时写 `error_kind`。
4. 更新 `OsToolResultDto` 与契约生成；存量工具测试保持通过。

### P2-C4 — Gateway

```
evaluate(tool, input, context, policy) -> GatewayDecision:

  1. intents = tool.permission_intents(input, context)?
  2. for intent in intents:
       for resource in intent.resources:
         key = ApprovalKey::from(tool, intent, input, context)
         if grant_store.is_granted(&key): audit(GrantHit); continue
         effect = policy.evaluate(intent.action, resource)
         match effect:
           Allow -> audit(Allow); continue
           Deny  -> audit(Deny); return Denied
           Ask   -> collect need_approval { key, intent, resource }
  3. if need_approval 非空:
       reasons = policy.explain(action, resource)              // P6 新增
       preview = timeout(2s, tool.preview_for_approval(input, context)).ok()  // P6 新增，仅此处
       request = ApprovalRequest { ..., tier, behavior, reasons, preview }
       decision = approval_prompter.request(request).await
       Approve       -> audit(Approved); continue
       AlwaysApprove -> grant_store.save_grant; audit(AlwaysApproved); continue
       Deny          -> audit(Rejected); return RejectedByUser
  4. return Allowed
```

**约束**：

- 意图为空（只读工具）时直接 `Allowed`，不触发审批。
- 同一工具多意图时，任一 `Deny` 立即返回；`Ask` 合并为一次审批请求。
- **每一条放行路径都必须留审计**，包括审批缓存命中。`grant_store` 命中记 `GrantHit`，与策略 `Allow` 区分：前者是「用户此前授权」，后者是「策略静态允许」，两者的事后追溯意义不同。
- 审计失败只记日志，不改变决策（审计不是事实源）。
- `AlwaysApprove` 只保存当前规范化键，不扩大到工具名或前缀。
- **P6 修订**：`explain` 与 `preview_for_approval` **只允许**在 `need_approval` 非空时调用。`explain` 对多个 `(action, resource)` 各取一次并合并去重；`preview` 每次审批只调一次（取首个 `need_approval` 的 `input`）。两者都不得影响 `GatewayDecision`——`preview` 超时/失败即 `None`，`explain` 异常即退化为一条 `DefaultPolicy`。
- 空键（`ApprovalKey` 为 `None`）不查缓存、不落 grant，但必须写审计：它每次都会走审批，审计是唯一能证明「确实问过人」的记录。

### P2-C5 — Pipeline

```rust
pub struct ToolPipeline {
    registry: Arc<ToolRegistry>,
    gateway: Arc<ToolGateway>,
    storage: Arc<dyn ToolResultStorage>,
    default_timeout: Duration,
}
```

执行流程固定为：

```
execute(tool_calls, context) -> Vec<ToolOutcome>:

  1. 查找：registry.find(name)；未找到 → Internal「Unknown tool」
  2. 校验：tool.validate_input(...)
       blocks_input_rewrite = false → 返回 Business，允许模型修正后重试
       blocks_input_rewrite = true  → 返回 Business，禁止任何改写绕过
  3. 网关：gateway.evaluate(...)；Denied / RejectedByUser → error_kind = Denied
  4. 执行：exec::invoke(tool, input, context, default_timeout)
  5. 持久化：storage.persist(...)（大结果落盘，小结果内联）
  6. 事件：通过 `PipelineEventSink` 端口上报 Started / Finished / Failed（P2 只定义 trait 与内存实现，Tauri 适配在 P4/P6）
  7. 回填：给模型取 result_for_assistant（缺省用 data 紧凑序列化）
```

**顺序不可交换**：`find → validate → gateway → execute → persist`。校验先于审批，避免为非法输入弹框。

**并发编排**：

```
partition_tool_batches(tool_calls):
  连续 is_concurrency_safe(true) 的工具合并为一个 batch（join_all 并发）
  遇到非并发安全工具即切分，单独串行
结果按原始顺序收集；持久化与事件顺序与原始顺序一致
```

示例：`[read_1, read_2, write_1, grep_1, grep_2, bash_1]` → 4 个 batch（2 并发 + 2 串行）。

### P2-C6 — 结果持久化

| 参数 | 取值 | 作用 |
|---|---|---|
| `per_tool_limit_chars` | 50K | 单结果超过即落盘 |
| `per_round_limit_chars` | 200K | 单轮总预算 |
| `preview_chars` | 2K | 落盘后回填预览 |

**选择算法**：按序列化字符数**降序**排列，依次累加；累计超过单轮预算的结果才落盘。落盘后消息历史只保留 `<persisted-output path=…>` 引用 + 前 `preview_chars` 字符。

**跨轮去重**（同会话）：同一路径、同一行范围、文件修订号未变的重复读取 → 返回「已在上文返回」，不回传内容；修订号变化则失效重读。该能力属于 `read`（P5），P2 只定义 `ReadReceiptStore` 端口。

**键结构（P5 修订）**：P2 的 `ReadReceiptKey` 只有 `path` / `start_line` / `end_line`，无法表达「同会话」语义。P5 按 [I38c-read §3.1](./I38c-read-spec.md) 给键补 `session_id: Option<String>`（取自 `ToolUseContext.session_id`；agent 链路由 `PipelineToolExecution` 填 `conversation_id`，ToolBox 链路为 `None` 并归入同一桶）。这是跨阶段契约变更，P5 落地时必须与本段同步。

**失效端口（P5 新增）**：修订号是「长度 + mtime」，删除后重建若长度与时间戳恰好相同（保时间戳的复制 / 解包）会误判「已读过」。P5 给 `ReadReceiptStore` 增加 `invalidate_path(&self, path: &str)`，按**组件级前缀**失效路径及其子树（`/a/b` 不得匹配 `/a/bc`）；`delete` 成功与 `move` 的源路径均调用（见 [I38c-delete §3.6](./I38c-delete-spec.md) / [I38c-move §3](./I38c-move-spec.md)）。同属跨阶段契约变更。

---

## 5 验收标准

> 本节是 P2 一次性验收记录（2026-11-05），只说明「当时如何判定通过」，不是阶段状态台账；阶段结论见 [roadmap](./I38-agent-roadmap.md) §2。

| # | 标准 | 验收记录 |
|---|---|---|
| A1 | `cargo test -p assistant-tools` 全绿；`cargo clippy -p assistant-tools -- -D warnings` 无告警 | ✅ 57+27+15+21+6 passed；clippy `--all-targets` 无告警 |
| A2 | `assistant-tools` 的 `cargo tree` 中无 `tauri`；`policy.rs` / `gateway.rs` / `pipeline.rs` 无 `AppHandle` / `Window` | ✅ `cargo tree` 仅自身条目；grep 无命中（`pipeline_source_has_no_tauri_dependency` 单测守住） |
| A3 | `most_restrictive` 九种组合、last-match-wins、多层取最严、Ceiling 拒绝 Allow 均有单测 | ✅ `tests/policy.rs` 21 项 |
| A4 | Gateway：Allow 直接放行；Ask 触发一次审批；Deny 立即拒绝；`AlwaysApprove` 只对当前键生效，相似键不命中 | ✅ `tests/gateway.rs` |
| A5 | 审批超时返回 `RejectedByUser`（非 Allowed）；取消返回 `Cancelled` | ⚠️ 超时已实现并单测；取消需 `ToolUseContext` 带取消信号，P2 无该字段，顺延到 P4（见 tasklist C4.13） |
| A6 | Pipeline 顺序与并发 batch 数量符合 §4-C5 示例；结果顺序与原始 tool_calls 一致 | ✅ `tests/pipeline.rs` |
| A7 | `validate_input` 命中 `blocks_input_rewrite` 时，Pipeline 不进入 Gateway，也不得被任何改写机制绕过 | ✅ 阶段顺序与 `blocks_input_rewrite` 用例 |
| A8 | 结果持久化：小结果内联；超预算按大小降序落盘；落盘引用与预览长度符合上限 | ✅ `tests/storage.rs` 6 项 + `tests/pipeline.rs` 单轮预算用例 |
| A9 | `ToolResult` 新增字段后，存量工具（`fs_link`）与 `os_tool_exec` 行为不变；契约生成通过 | ✅ `fs_link` 单测全绿；`contract:gen` 幂等（契约文件哈希前后一致） |
| A10 | 无新增 DB 表；GrantStore / AuditStore / Storage 均以端口注入 | ✅ `assistant-storage` 零改动；存储根目录由构造参数注入 |

---

## 6 风险与开放问题

| 风险 | 缓解 |
|---|---|
| 审批端口实现被塞入 tauri 类型 | A2 以 `cargo tree` + grep 双重校验；适配层只能在 `src-tauri` |
| 路径归一化不一致导致越权命中 | 复用 `os/path.rs` 的 canonicalize；单测覆盖符号链接 |
| 前缀匹配被当作默认策略 | 只有 `ApprovalScope::Opaque` 可由工具显式声明；review 检查默认路径 |
| 审计写入失败影响主流程 | 审计只记日志；只有策略/审批决策影响结果 |
| 结果落盘破坏可读性 | 只对超过单轮预算的最大结果落盘；预览保留 2K |
| 审批键被 lossy 归一导致越权命中 | `ApprovalKey` 的路径键必须是合法 UTF-8；无法无损转换时返回空键走审批，禁止 `to_string_lossy` |

**已知限制（P2 明确不支持，不在本阶段解决）**：

1. **不支持 >260 字符的长路径**。`canonicalize_lenient` 统一剥离 `\\?\` 前缀（I31 §5.2），全链路只有「展示形态」一种形态。Rust std 在 Windows 上对绝对路径的文件操作会内部转 verbatim，因此**工具自身的文件读写不受 260 限制**；受限的是外部进程：`mklink` 既不接受 `\\?\` 也受 260 约束，`git` 需 `core.longpaths=true` 才能处理长路径。
2. 若后续确认需要长路径，须按 `docs/analysis/path-handling-inconsistency-analysis.md` §9-B 改为双形态模型（`to_io_path()` / `to_display_path()`），并同步修订 I31 §5.2 的「剥离 `\\?\`」结论。该改造会触及 `canonicalize_lenient` 的返回契约与 `ensure_within_root` 的比较方式，与「存量工具零改动」冲突，故不在 P2 实施。
3. `dunce::simplified` 对不安全 verbatim 路径（尾随点/空格、保留名）会拒绝简化，此时 `simplify_verbatim` 仍返回 `\\?\` 形态，`ensure_within_root` 可能误拒。影响面限于此类非常规路径，P2 不处理。

**开放问题**（1–3 已由 P4 定稿，取值见 [I38b §9.1](./I38b-agent-loop-spec.md)）：

1. ~~`ApprovalRequest` 是否需要携带 `iteration` 与 `turn_id`？~~ **已定稿：不加**。P4 用 `TimingApprovalPrompter` 装饰器按 `tool_call_id` 记录等待时长，`turn_id` / `iteration` 由 `src-tauri` 的 `PipelineEventSink` 适配器在发事件时补齐（I38b §6.1 / §6.4）；P2 的 `ApprovalRequest` 字段保持原样。
2. ~~审计生产存储是否复用 `messages.tool_calls_json`，还是新建 `tool_calls` 表？~~ **已定稿：都不复用，新建 `permission_grants` / `permission_audit` 两表**（I38b D14）；tool 调用与结果走 `messages` 的 `tool_calls_json` + 独立 `role='tool'` 行，不建 `tool_calls` 表（I38b D11 / §6.2）。
3. ~~`ToolOutcome` 的事件载荷是否直接复用 DTO？~~ **已定稿：不复用**。`assistant-agent` 侧定义 `ToolEvent`（`assistant-agent` 本地类型），`src-tauri` 的适配器把它映射成 protocol DTO；`assistant-tools` 的 `PipelineEvent` 只喂适配器（I38b §6.1）。

---

## 7 参考

| 设计要素 | 参考项目 | 参考文件 |
|---|---|---|
| PermissionEffect / most_restrictive | OpenBitFun | `contracts/product-domains/src/tool_permissions.rs` |
| 主规则 + 约束层 | OpenBitFun | 同上 |
| PermissionRuntimeCeiling | OpenBitFun | 同上 |
| 审批缓存与键规范化 | codex | `core/src/tools/approvals.rs`、`core/src/tools/runtimes/` |
| ToolPipeline 生命周期 | OpenBitFun | `agentic/tools/pipeline/tool_pipeline.rs` |
| validate_input / 不变量不可改写 | OpenBitFun | `execution/tool-contracts/src/framework.rs` |
| 并发分批 | OpenBitFun | `partition_tool_batches()` |
| 结果三层上限与降序选择 | OpenBitFun | `execution/tool-contracts/src/tool_result_storage.rs` |
| 错误分类 | codex | `tools/src/tool_executor.rs` |
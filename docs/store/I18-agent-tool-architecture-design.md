# I18 — Agent Tool 架构设计规格

> 统一 ToolCall 执行 + OS 工具能力 + MCP 外部工具的分层架构设计
> 参考：OpenBitFun（Tool trait + Permission Engine + RoundExecutor）、OpenHuman（ToolPolicy trait + ToolDispatcher）

---

## 1. 问题域

当前项目存在两套完全独立的"工具"体系，且 Agent Loop 缺失：

| 体系 | 位置 | 能力 | 问题 |
|------|------|------|------|
| 前端 ToolBox | `src/tools/` | 21 个纯 UI 计算工具，浏览器沙箱内执行 | 无法被 LLM 调用，与 Agent 无关 |
| LLM ToolCall | `assistant-providers/src/types/tool.rs` | 流式累积 + JSON 修复 + 前端透传显示 | **无执行**，tool_call 到此为止，不回填不继续 |

**核心缺口**：LLM 返回 `tool_call` 后，应用只显示不执行 → 无法形成"调用→执行→回填→继续"的 Agent Loop。

**目标**：设计统一分层架构，让 LLM 能调用工具（包括 OS 工具），且安全可控。

---

## 2. 参考项目关键设计提取

### 2.1 OpenBitFun — 工业级完整实现

**Tool trait**（`agentic/tools/framework.rs`）：

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    async fn description(&self) -> Result<String>;
    fn input_schema(&self) -> Value;
    fn is_readonly(&self) -> bool;
    fn is_concurrency_safe(&self, input: Option<&Value>) -> bool;
    fn permission_intents(&self, input: &Value, context: &ToolUseContext) -> Result<Vec<PermissionIntent>>;
    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult>;
}
```

关键洞察：**工具自描述权限需求，策略层决策是否放行**——关注点分离。

**Permission Engine**（`contracts/product-domains/src/tool_permissions.rs`）：

```rust
pub enum PermissionEffect { Allow, Ask, Deny }

impl PermissionEffect {
    pub const fn most_restrictive(self, other: Self) -> Self {
        // Deny > Ask > Allow
    }
}

pub struct PermissionRule {
    pub action: String,      // "read" / "edit" / "delete" / "execute"
    pub resource: String,    // 路径 glob / 命令名
    pub effect: PermissionEffect,
}

pub struct ResolvedPermissionPolicy {
    rules: PermissionRuleset,                        // 主规则（last-match-wins）
    constraint_layers: Vec<PermissionConstraintLayer>, // 独立约束层
}
```

关键洞察：**多层策略叠加取最严**，`PermissionRuntimeCeiling` 禁止携带 `Allow` 规则——委托只能收紧不能放宽。

**RoundExecutor**（`agentic/execution/round_executor.rs`）：

```rust
pub struct RoundExecutor {
    stream_processor: Arc<StreamProcessor>,
    tool_pipeline: Option<Arc<ToolPipeline>>,
    event_queue: Arc<EventQueue>,
    cancellation_tokens: DialogTurnCancellationTokenStore,
}
```

关键洞察：RoundExecutor 持有 ToolPipeline，流式消费 LLM 输出 → 提取 tool_calls → ToolPipeline 执行 → 回填 → 继续循环。

**ToolPipeline**（`agentic/tools/pipeline/tool_pipeline.rs`）：

完整生命周期：查找工具 → 收集 permission_intents → 策略评估 → 用户审批（Ask） → 执行（带超时+取消） → 结果存储 → 回填。

**内置 OS 工具清单**：

| 工具 | action | readonly | destructive | open_world | 特点 |
|------|--------|----------|-------------|------------|------|
| Read | read | ✅ | ❌ | ❌ | 行范围、tail、文档转换 |
| Write | edit | ❌ | ❌ | ❌ | 路径解析、写前检查、plan artifact 诊断 |
| Delete | delete | ❌ | ✅ | ❌ | 删前 checkpoint |
| LS | read | ✅ | ❌ | ❌ | 递归列目录，限制条目数 |
| Glob | read | ✅ | ❌ | ❌ | ripgrep 后端 |
| Bash | execute | ❌ | ✅ | ✅ | 本地/远程 Shell、edit_constraint_guard、后台进程 |
| AskUser | — | ✅ | ❌ | ❌ | AI 主动问用户，暂停等待回答 |

### 2.2 OpenHuman — 轻量级策略抽象

**ToolPolicy trait**（`agent/tool_policy.rs`）：

```rust
pub enum PolicyDecision { Allow, Deny, RequireApproval }

#[async_trait]
pub trait ToolPolicy: Send + Sync {
    fn name(&self) -> &str;
    async fn check(&self, request: &ToolPolicyRequest) -> PolicyDecision;
}

pub struct AllowAllToolPolicy;  // 默认全放行
```

关键洞察：策略是可插拔的中间件，默认 `AllowAll`，调用方可安装更严策略。`ToolPolicyRequest` 携带 `session_id / channel / agent_definition_id / call_id / iteration` 上下文。

**ToolDispatcher**（`config/ops/tool_dispatcher.rs`）：

支持多种工具调用格式：`native`（结构化 function_call）、`xml`（XML-in-tag）、`pformat` / `python` / `typescript`（代码块格式）。

---

## 3. 架构设计

### 3.1 总览

```
┌─────────────────────────────────────────────────────────────────┐
│                        LLM Provider                             │
│                   返回 tool_call / text                         │
└──────────────────────────┬──────────────────────────────────────┘
                           │
                  ┌────────▼────────┐
                  │   RoundExecutor  │  Agent Loop 核心
                  │  (消费→执行→回填→继续) │
                  └────────┬────────┘
                           │
            ┌──────────────┼──────────────────────┐
            │              │                      │
     ┌──────▼──────┐ ┌─────▼──────┐ ┌─────────────▼────────────┐
     │  L0: 沙箱工具 │ │ L1: OS 工具 │ │  L2: MCP/外部工具         │
     │  (前端执行)   │ │ (Rust 执行) │ │  (进程通信)               │
     └──────┬──────┘ └─────┬──────┘ └─────────────┬────────────┘
            │              │                      │
     ┌──────▼──────────────▼──────────────────────▼────────────┐
     │              Tool Gateway (权限网关)                      │
     │  ┌──────────────────────────────────────────────────┐   │
     │  │  Permission Policy Engine                        │   │
     │  │  Rule: (action, resource) → Allow/Ask/Deny      │   │
     │  │  多层叠加: global + project + agent_mode          │   │
     │  │  most_restrictive 组合                           │   │
     │  └──────────────────────────────────────────────────┘   │
     │  ┌──────────────────────────────────────────────────┐   │
     │  │  User Approval Channel                          │   │
     │  │  Ask → emit awaiting_user → 前端审批 UI          │   │
     │  │  超时自动 Deny                                   │   │
     │  └──────────────────────────────────────────────────┘   │
     └─────────────────────────────────────────────────────────┘
```

### 3.2 统一 Tool Trait

三层工具统一到一个 trait，这是分层设计的核心：

```rust
/// 工具能力层级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolTier {
    /// L0: 前端沙箱，无 OS 访问（现有 ToolBox 工具）
    Sandbox,
    /// L1: Rust 后端，有 OS 访问（内置文件/Shell 工具）
    Os,
    /// L2: 外部进程通信（MCP server 工具）
    External,
}

/// 三维安全标记
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BehaviorHints {
    pub read_only: bool,
    pub destructive: bool,
    pub open_world: bool,
}

/// 工具执行结果
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub output: String,
    pub is_error: bool,
    pub artifacts: Vec<ToolArtifact>,
}

#[derive(Debug, Clone)]
pub struct ToolArtifact {
    pub kind: String,       // "file" / "image" / "diff"
    pub path: Option<String>,
    pub content: String,
}

/// 工具执行上下文
pub struct ToolUseContext {
    pub session_id: String,
    pub conversation_id: String,
    pub workspace_root: Option<PathBuf>,
    pub cancellation_token: CancellationToken,
    pub iteration: u32,
    pub source: ToolCallSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallSource {
    Session,
    Bus,
    Channel,
    Cron,
    Webhook,
}

/// 统一工具 trait
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    async fn description(&self) -> Result<String>;
    fn input_schema(&self) -> Value;
    fn tier(&self) -> ToolTier;
    fn behavior_hints(&self) -> BehaviorHints;
    fn is_readonly(&self) -> bool { self.behavior_hints().read_only }
    fn is_concurrency_safe(&self) -> bool { self.behavior_hints().read_only }

    /// 声明权限需求（不执行，只描述）
    fn permission_intents(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> Result<Vec<PermissionIntent>>;

    /// 执行工具
    async fn execute(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> Result<ToolResult>;
}
```

**`ToolTier` 的 `PartialOrd` 语义**：`Sandbox < Os < External`。Agent 模式声明 `max_tier` 即可限制可用工具层级。

### 3.3 Permission Policy Engine

#### 3.3.1 核心类型

```rust
/// 权限效果：Allow / Ask(需审批) / Deny(硬拒绝)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionEffect {
    Allow,
    Ask,
    Deny,
}

impl PermissionEffect {
    /// 组合两个独立策略决策，取更严格者
    pub const fn most_restrictive(self, other: Self) -> Self {
        match (self, other) {
            (Self::Deny, _) | (_, Self::Deny) => Self::Deny,
            (Self::Ask, _) | (_, Self::Ask) => Self::Ask,
            (Self::Allow, Self::Allow) => Self::Allow,
        }
    }
}
```

#### 3.3.2 权限意图

```rust
/// 工具声明的单个权限需求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionIntent {
    pub action: String,
    pub resources: Vec<String>,
    pub save_resources: Vec<String>,
    pub display_metadata: HashMap<String, Value>,
}

impl PermissionIntent {
    pub fn new(action: impl Into<String>, resource: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            resources: vec![resource.into()],
            save_resources: vec![],
            display_metadata: HashMap::new(),
        }
    }

    pub fn with_save_resources(mut self, resources: Vec<String>) -> Self {
        self.save_resources = resources;
        self
    }

    pub fn with_display_metadata(mut self, key: &str, value: Value) -> Self {
        self.display_metadata.insert(key.to_string(), value);
        self
    }
}
```

#### 3.3.3 权限规则

```rust
/// 单条权限规则
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRule {
    pub action: String,       // "read" / "edit" / "delete" / "execute" / "network"
    pub resource: String,     // 路径 glob / 命令名 / URL pattern / "*"
    pub effect: PermissionEffect,
}
```

规则匹配语义：
- 同一规则集内，**后匹配优先**（last-match-wins）——允许前面的宽泛规则被后面的具体规则覆盖
- 不同规则集之间，取 `most_restrictive`

#### 3.3.4 多层策略

```rust
/// 独立约束层（项目级、Agent 模式级等）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionConstraintLayer {
    rules: Vec<PermissionRule>,
}

/// 已解析的完整策略 = 主规则 + 独立约束层
#[derive(Debug, Clone, Default)]
pub struct ResolvedPermissionPolicy {
    rules: Vec<PermissionRule>,                          // 应用级主规则
    constraint_layers: Vec<PermissionConstraintLayer>,    // 项目级 + Agent 模式级
}

impl ResolvedPermissionPolicy {
    /// 评估一个权限请求的最终效果
    pub fn evaluate(&self, action: &str, resource: &str) -> PermissionEffect {
        let mut result = PermissionEffect::Allow;

        // 主规则（last-match-wins）
        if let Some(e) = self.eval_rules(&self.rules, action, resource) {
            result = result.most_restrictive(e);
        }

        // 各约束层独立评估，取 most_restrictive
        for layer in &self.constraint_layers {
            if let Some(e) = self.eval_rules(layer.rules(), action, resource) {
                result = result.most_restrictive(e);
            }
        }

        result
    }

    fn eval_rules(&self, rules: &[PermissionRule], action: &str, resource: &str) -> Option<PermissionEffect> {
        let mut last: Option<PermissionEffect> = None;
        for rule in rules {
            if self.matches_action(&rule.action, action) && self.matches_resource(&rule.resource, resource) {
                last = Some(rule.effect);
            }
        }
        last
    }
}
```

#### 3.3.5 运行时天花板（委托约束）

```rust
/// 子 Agent / 委托执行的权限天花板
/// 禁止携带 Allow 规则——委托只能收紧不能放宽
pub struct PermissionRuntimeCeiling {
    rules: Vec<PermissionRule>,
}

impl PermissionRuntimeCeiling {
    pub fn try_new(rules: Vec<PermissionRule>) -> Result<Self, CeilingValidationError> {
        if let Some(rule) = rules.iter().find(|r| r.effect == PermissionEffect::Allow) {
            return Err(CeilingValidationError { rule: rule.clone() });
        }
        Ok(Self { rules })
    }
}
```

#### 3.3.6 默认规则集

应用级默认策略（最宽松基线）：

```rust
fn default_global_rules() -> Vec<PermissionRule> {
    vec![
        // 默认全部需审批（安全基线）
        PermissionRule::new("*", "*", PermissionEffect::Ask),
        // 文件读取：显式 Allow（最常见的只读操作）
        PermissionRule::new("read", "*", PermissionEffect::Allow),
        // 系统敏感路径读取：Deny 覆盖上面的 Allow（last-match-wins）
        PermissionRule::new("read",  "/etc/shadow",       PermissionEffect::Deny),
        PermissionRule::new("read",  "/etc/ssh/**",       PermissionEffect::Deny),
        // 系统敏感路径编辑：硬拒绝
        PermissionRule::new("edit",  "/etc/**",           PermissionEffect::Deny),
        PermissionRule::new("edit",  "C:/Windows/**",     PermissionEffect::Deny),
        PermissionRule::new("edit",  "C:/Program Files/**", PermissionEffect::Deny),
    ]
}
```

> 默认 Ask + 显式 Allow 模式：新增 action 类型默认需审批，避免意外放行。与 OpenBitFun 的 Ask 预设策略一致。

### 3.4 Tool Gateway

Tool Gateway 是 RoundExecutor 和工具执行之间的安全层：

```rust
pub struct ToolGateway {
    policy_engine: Arc<PolicyEngine>,
    approval_channel: ApprovalChannel,
    grant_store: Arc<dyn PermissionGrantStore>,
    audit_store: Arc<dyn PermissionAuditStore>,
    event_queue: Arc<EventQueue>,
}

#[async_trait]
pub trait PermissionGrantStore: Send + Sync {
    async fn is_granted(&self, action: &str, resource: &str) -> bool;
    async fn save_grant(&self, action: &str, resource: &str) -> Result<()>;
}

#[async_trait]
pub trait PermissionAuditStore: Send + Sync {
    async fn log_decision(&self, entry: AuditEntry) -> Result<()>;
}

pub struct AuditEntry {
    pub tool_name: String,
    pub action: String,
    pub resource: String,
    pub effect: PermissionEffect,
    pub granted_by: Option<String>,
    pub timestamp: Instant,
}

pub struct ApprovalChannel {
    pending: TokioMutex<HashMap<String, ApprovalWaiter>>,
    timeout: Duration,
}

pub struct ApprovalWaiter {
    tx: tokio::sync::oneshot::Sender<ApprovalDecision>,
    created_at: Instant,
}

#[derive(Debug, Clone)]
pub enum ApprovalDecision {
    Approve,
    Deny { reason: Option<String> },
    AlwaysApprove,
}

impl ToolGateway {
    /// 评估工具调用的权限，返回是否允许执行
    pub async fn evaluate(
        &self,
        tool: &dyn Tool,
        input: &Value,
        context: &ToolUseContext,
        policy: &ResolvedPermissionPolicy,
    ) -> Result<GatewayDecision> {
        // 1. 收集权限意图
        let intents = tool.permission_intents(input, context)?;

        // 2. 逐条评估
        let mut need_approval = Vec::new();
        for intent in &intents {
            for resource in &intent.resources {
                // 2a. 查询 GrantStore（记忆授权）
                if self.grant_store.is_granted(&intent.action, resource).await {
                    self.audit_store.log_decision(AuditEntry {
                        tool_name: tool.name().to_string(),
                        action: intent.action.clone(),
                        resource: resource.clone(),
                        effect: PermissionEffect::Allow,
                        granted_by: Some("grant_store".to_string()),
                        timestamp: Instant::now(),
                    }).await.ok();
                    continue;
                }
                // 2b. 策略评估
                match policy.evaluate(&intent.action, resource) {
                    PermissionEffect::Allow => {
                        self.audit_store.log_decision(AuditEntry {
                            tool_name: tool.name().to_string(),
                            action: intent.action.clone(),
                            resource: resource.clone(),
                            effect: PermissionEffect::Allow,
                            granted_by: Some("policy".to_string()),
                            timestamp: Instant::now(),
                        }).await.ok();
                    },
                    PermissionEffect::Ask => {
                        need_approval.push((intent.action.clone(), resource.clone()));
                    },
                    PermissionEffect::Deny => {
                        self.audit_store.log_decision(AuditEntry {
                            tool_name: tool.name().to_string(),
                            action: intent.action.clone(),
                            resource: resource.clone(),
                            effect: PermissionEffect::Deny,
                            granted_by: None,
                            timestamp: Instant::now(),
                        }).await.ok();
                        return Ok(GatewayDecision::Denied {
                            action: intent.action.clone(),
                            resource: resource.clone(),
                        });
                    },
                }
            }
        }

        // 3. 需要审批的意图 → 请求用户
        if !need_approval.is_empty() {
            self.event_queue.emit(AgenticEvent::ToolAwaitingApproval {
                tool_name: tool.name().to_string(),
                arguments: input.clone(),
                intents: need_approval.clone(),
            });

            let decision = self.approval_channel.wait_for_approval(
                &tool.name(),
                &need_approval,
            ).await?;

            match decision {
                ApprovalDecision::Deny { reason } => {
                    for (action, resource) in &need_approval {
                        self.audit_store.log_decision(AuditEntry {
                            tool_name: tool.name().to_string(),
                            action: action.clone(),
                            resource: resource.clone(),
                            effect: PermissionEffect::Deny,
                            granted_by: Some("user".to_string()),
                            timestamp: Instant::now(),
                        }).await.ok();
                    }
                    return Ok(GatewayDecision::RejectedByUser { reason });
                },
                ApprovalDecision::AlwaysApprove => {
                    for (action, resource) in &need_approval {
                        self.grant_store.save_grant(action, resource).await?;
                        self.audit_store.log_decision(AuditEntry {
                            tool_name: tool.name().to_string(),
                            action: action.clone(),
                            resource: resource.clone(),
                            effect: PermissionEffect::Allow,
                            granted_by: Some("user:always".to_string()),
                            timestamp: Instant::now(),
                        }).await.ok();
                    }
                },
                ApprovalDecision::Approve => {
                    for (action, resource) in &need_approval {
                        self.audit_store.log_decision(AuditEntry {
                            tool_name: tool.name().to_string(),
                            action: action.clone(),
                            resource: resource.clone(),
                            effect: PermissionEffect::Allow,
                            granted_by: Some("user:once".to_string()),
                            timestamp: Instant::now(),
                        }).await.ok();
                    }
                },
            }
        }

        Ok(GatewayDecision::Allowed)
    }
}

pub enum GatewayDecision {
    Allowed,
    Denied { action: String, resource: String },
    RejectedByUser { reason: Option<String> },
}
```

### 3.5 Tool Pipeline

Tool Pipeline 编排完整的工具执行生命周期：

```rust
pub struct ToolPipeline {
    registry: Arc<ToolRegistry>,
    gateway: Arc<ToolGateway>,
    result_storage: Arc<ToolResultStorage>,
    default_timeout: Duration,
}

pub struct ToolRegistry {
    tools: DashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn register(&self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn find(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).map(|t| Arc::clone(&*t))
    }

    /// 获取指定 tier 及以下的全部工具定义（供 LLM function calling）
    pub fn tool_definitions_for_tier(&self, max_tier: ToolTier) -> Vec<ToolDefinition> {
        self.tools.iter()
            .filter(|entry| entry.value().tier() <= max_tier)
            .map(|entry| {
                let tool = entry.value();
                ToolDefinition {
                    r#type: "function".to_string(),
                    function: FunctionDefinition {
                        name: tool.name().to_string(),
                        description: /* ... */,
                        parameters: tool.input_schema(),
                    },
                }
            })
            .collect()
    }
}

impl ToolPipeline {
    pub async fn execute(
        &self,
        tool_calls: &[ToolCall],
        context: &ToolExecutionContext,
    ) -> Result<Vec<ToolResult>> {
        let mut results = Vec::new();

        for tool_call in tool_calls {
            let start = Instant::now();

            // 1. 查找工具
            let tool = match self.registry.find(&tool_call.name) {
                Some(t) => t,
                None => {
                    results.push(ToolResult::error(
                        format!("Unknown tool: {}", tool_call.name)
                    ));
                    continue;
                }
            };

            // 2. 权限网关评估
            match self.gateway.evaluate(
                tool.as_ref(),
                &tool_call.arguments,
                &context.use_context,
                &context.policy,
            ).await? {
                GatewayDecision::Allowed => {},
                GatewayDecision::Denied { action, resource } => {
                    results.push(ToolResult::error(
                        format!("Permission denied: {} on {}", action, resource)
                    ));
                    continue;
                },
                GatewayDecision::RejectedByUser { reason } => {
                    results.push(ToolResult::error(
                        format!("User rejected: {}", reason.unwrap_or_default())
                    ));
                    continue;
                },
            }

            // 3. 执行（带超时 + 取消）
            let result = match timeout(
                self.default_timeout,
                tool.execute(&tool_call.arguments, &context.use_context),
            ).await {
                Ok(Ok(r)) => r,
                Ok(Err(e)) => ToolResult::error(format!("{e}")),
                Err(_) => ToolResult::error("Tool execution timed out"),
            };

            // 4. 持久化
            self.result_storage.persist(
                &tool_call.id,
                &context.use_context.conversation_id,
                &tool_call.name,
                &tool_call.arguments,
                &result,
                start.elapsed(),
            ).await?;

            // 5. 发射事件
            self.gateway.event_queue.emit(AgenticEvent::ToolCompleted {
                tool_call_id: tool_call.id.clone(),
                tool_name: tool_call.name().to_string(),
                result: result.clone(),
                elapsed_ms: start.elapsed().as_millis() as u64,
            });

            results.push(result);
        }

        Ok(results)
    }
}
```

### 3.6 RoundExecutor（Agent Loop）

```rust
pub struct RoundExecutor {
    ai_client: Arc<AIClient>,
    tool_pipeline: Arc<ToolPipeline>,
    stream_processor: Arc<StreamProcessor>,
    event_queue: Arc<EventQueue>,
    max_iterations: u32,
}

pub struct RoundContext {
    pub messages: Vec<Message>,
    pub tool_context: ToolExecutionContext,
    pub agent_mode: AgentModeConfig,
}

pub struct AgentModeConfig {
    pub max_tier: ToolTier,
    pub policy: ResolvedPermissionPolicy,
    pub tool_timeout: Duration,
    pub max_iterations: u32,
}

impl RoundExecutor {
    pub async fn execute_round(&self, context: &mut RoundContext) -> Result<RoundOutcome> {
        for iteration in 0..context.agent_mode.max_iterations {
            // 1. 构建请求（注入可用工具定义）
            let tool_defs = self.tool_pipeline.registry()
                .tool_definitions_for_tier(context.agent_mode.max_tier);

            let request = ModelRequest {
                messages: context.messages.clone(),
                tools: tool_defs,
                ..Default::default()
            };

            // 2. 调用 LLM（流式）
            let stream_outcome = self.stream_processor
                .process_stream(request, &self.ai_client)
                .await?;

            // 3. 提取 assistant message（含 text + tool_calls）
            context.messages.push(stream_outcome.assistant_message.clone());

            // 4. 无 tool_call → 对话结束
            let tool_calls = match stream_outcome.tool_calls {
                calls if !calls.is_empty() => calls,
                _ => return Ok(RoundOutcome::Finished(stream_outcome)),
            };

            // 5. 发射 tool_call running 事件
            for tc in &tool_calls {
                self.event_queue.emit(AgenticEvent::ToolCallStarted {
                    tool_call_id: tc.id.clone(),
                    tool_name: tc.name.clone(),
                    arguments: tc.arguments.clone(),
                });
            }

            // 6. 执行工具
            let tool_results = self.tool_pipeline
                .execute(&tool_calls, &context.tool_context)
                .await?;

            // 7. 回填 tool_results 到消息历史
            let results_message = Message::tool_results_message(
                tool_calls.iter().zip(tool_results.iter())
                    .map(|(tc, tr)| (tc.id.clone(), tr.output.clone(), tr.is_error))
                    .collect(),
            );
            context.messages.push(results_message);

            // 8. 继续循环（LLM 基于工具结果继续生成）
        }

        Ok(RoundOutcome::MaxIterationsReached)
    }
}
```

---

## 4. 三层工具详细设计

### 4.1 L0: 沙箱工具（Sandbox Tools）

**对应现有 `src/tools/` 的 21 个前端工具**。

关键设计：沙箱工具**双入口**——
- 主窗口独立使用（现有行为，不变）
- Agent Loop 调用（新增，LLM 返回 `tool_call: {name: "json-viewer"}` → 前端执行 → 返回结果）

```rust
pub struct SandboxTool {
    pub id: String,
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub category: String,
}

impl Tool for SandboxTool {
    fn tier(&self) -> ToolTier { ToolTier::Sandbox }
    fn behavior_hints(&self) -> BehaviorHints {
        BehaviorHints { read_only: true, destructive: false, open_world: false }
    }
    fn permission_intents(&self, ..) -> Result<Vec<PermissionIntent>> {
        Ok(vec![])  // 沙箱工具无需权限
    }
    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult> {
        // 通过 Tauri event 发到前端执行，等待前端返回结果
        let response = context.frontend_channel
            .invoke("sandbox_tool_execute", json!({
                "tool_id": self.id,
                "input": input,
            }))
            .await?;
        Ok(ToolResult { output: response, is_error: false, artifacts: vec![] })
    }
}
```

### 4.2 L1: OS 工具

#### 4.2.1 工具清单与安全标记

| 工具名 | action | read_only | destructive | open_world | 说明 |
|--------|--------|-----------|-------------|------------|------|
| `fs_read` | read | ✅ | ❌ | ❌ | 读文件，支持行范围/tail |
| `fs_write` | edit | ❌ | ❌ | ❌ | 写文件，路径白名单约束 |
| `fs_delete` | delete | ❌ | ✅ | ❌ | 删除文件/目录，需审批 |
| `fs_list` | read | ✅ | ❌ | ❌ | 列目录，限制深度和条目数 |
| `fs_glob` | read | ✅ | ❌ | ❌ | glob 模式搜索文件 |
| `fs_grep` | read | ✅ | ❌ | ❌ | ripgrep 内容搜索 |
| `fs_mkdir` | edit | ❌ | ❌ | ❌ | 创建目录 |
| `fs_move` | edit | ❌ | ❌ | ❌ | 移动/重命名文件 |
| `shell_exec` | execute | ❌ | ✅ | ✅ | Shell 命令执行，需审批+超时 |
| `ask_user` | — | ✅ | ❌ | ❌ | AI 主动问用户，暂停等待回答 |

#### 4.2.2 fs_read 实现

```rust
pub struct FsReadTool;

#[derive(Deserialize)]
struct FsReadInput {
    path: String,
    offset: Option<u32>,
    limit: Option<u32>,
    tail: Option<u32>,
}

impl Tool for FsReadTool {
    fn name(&self) -> &str { "fs_read" }
    async fn description(&self) -> Result<String> {
        Ok("Read file contents. Supports line range (offset+limit) and tail.".to_string())
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path (absolute or relative to workspace)" },
                "offset": { "type": "integer", "description": "Start line (1-based)" },
                "limit": { "type": "integer", "description": "Max lines to read" },
                "tail": { "type": "integer", "description": "Read last N lines" }
            },
            "required": ["path"]
        })
    }
    fn tier(&self) -> ToolTier { ToolTier::Os }
    fn behavior_hints(&self) -> BehaviorHints {
        BehaviorHints { read_only: true, destructive: false, open_world: false }
    }
    fn permission_intents(&self, input: &Value, ctx: &ToolUseContext) -> Result<Vec<PermissionIntent>> {
        let path = input["path"].as_str().unwrap_or("");
        let resolved = resolve_path(path, ctx.workspace_root.as_deref())?;
        Ok(vec![PermissionIntent::new("read", resolved.to_string_lossy())])
    }
    async fn execute(&self, input: &Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let args: FsReadInput = serde_json::from_value(input.clone())?;
        let path = resolve_path(&args.path, ctx.workspace_root.as_deref())?;

        // 限制读取大小（最大 1MB）
        let metadata = tokio::fs::metadata(&path).await?;
        if metadata.len() > 1_048_576 {
            return Ok(ToolResult::error("File too large (max 1MB)"));
        }

        let content = tokio::fs::read_to_string(&path).await?;
        let lines: Vec<&str> = content.lines().collect();

        let selected = match (args.offset, args.limit, args.tail) {
            (_, _, Some(tail)) => lines.iter().rev().take(tail as usize).rev(),
            (Some(off), Some(lim), _) => lines.iter().skip(off as usize - 1).take(lim as usize),
            (Some(off), None, _) => lines.iter().skip(off as usize - 1),
            _ => lines.iter(),
        };

        Ok(ToolResult {
            output: selected.cloned().collect::<Vec<_>>().join("\n"),
            is_error: false,
            artifacts: vec![],
        })
    }
}
```

#### 4.2.3 fs_write 实现

```rust
pub struct FsWriteTool;

#[derive(Deserialize)]
struct FsWriteInput {
    path: String,
    content: String,
    append: Option<bool>,
}

impl Tool for FsWriteTool {
    fn name(&self) -> &str { "fs_write" }
    fn tier(&self) -> ToolTier { ToolTier::Os }
    fn behavior_hints(&self) -> BehaviorHints {
        BehaviorHints { read_only: false, destructive: false, open_world: false }
    }
    fn permission_intents(&self, input: &Value, ctx: &ToolUseContext) -> Result<Vec<PermissionIntent>> {
        let path = input["path"].as_str().unwrap_or("");
        let resolved = resolve_path(path, ctx.workspace_root.as_deref())?;
        Ok(vec![PermissionIntent::new("edit", resolved.to_string_lossy())])
    }
    async fn execute(&self, input: &Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let args: FsWriteInput = serde_json::from_value(input.clone())?;
        let path = resolve_path(&args.path, ctx.workspace_root.as_deref())?;

        // 确保父目录存在
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        if args.append.unwrap_or(false) {
            tokio::fs::OpenOptions::new()
                .append(true).open(&path)
                .await?
                .write_all(args.content.as_bytes()).await?;
        } else {
            tokio::fs::write(&path, &args.content).await?;
        }

        Ok(ToolResult {
            output: format!("Written {} bytes to {}", args.content.len(), path.display()),
            is_error: false,
            artifacts: vec![ToolArtifact {
                kind: "file".to_string(),
                path: Some(path.to_string_lossy().to_string()),
                content: args.content.clone(),
            }],
        })
    }
}
```

#### 4.2.4 shell_exec 实现

```rust
pub struct ShellExecTool;

#[derive(Deserialize)]
struct ShellExecInput {
    command: String,
    cwd: Option<String>,
    timeout_ms: Option<u64>,
}

impl Tool for ShellExecTool {
    fn name(&self) -> &str { "shell_exec" }
    fn tier(&self) -> ToolTier { ToolTier::Os }
    fn behavior_hints(&self) -> BehaviorHints {
        BehaviorHints { read_only: false, destructive: true, open_world: true }
    }
    fn permission_intents(&self, input: &Value, _ctx: &ToolUseContext) -> Result<Vec<PermissionIntent>> {
        let command = input["command"].as_str().unwrap_or("");
        Ok(vec![
            PermissionIntent::new("execute", command.to_string()),
            PermissionIntent::new("network", "*"),
        ])
    }
    async fn execute(&self, input: &Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let args: ShellExecInput = serde_json::from_value(input.clone())?;

        let cwd = args.cwd
            .map(|c| resolve_path(&c, ctx.workspace_root.as_deref()))
            .transpose()?
            .or_else(|| ctx.workspace_root.clone());

        let timeout_duration = Duration::from_millis(
            args.timeout_ms.unwrap_or(30_000)
        );

        let output = timeout(timeout_duration, async {
            if has_hidden_execution(&args.command) {
                return Err(ShellSecurityError::HiddenExecution.into());
            }

            let mut cmd = tokio::process::Command::new("cmd");
            cmd.args(["/C", &args.command])
                .env_clear()
                .envs(safe_env_vars())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());

            if let Some(ref dir) = cwd {
                cmd.current_dir(dir);
            }

            cmd.output().await
        }).await??;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let is_error = !output.status.success();

        let mut result_output = stdout;
        if !stderr.is_empty() {
            result_output.push_str("\n--- stderr ---\n");
            result_output.push_str(&stderr);
        }

        Ok(ToolResult {
            output: result_output,
            is_error,
            artifacts: vec![],
        })
    }
}
```

#### 4.2.5 Shell 安全守卫

Shell 执行使用 `cmd /C` / `bash -c`（保留管道、重定向等 shell 能力），配合纵深防御组合：

```rust
fn has_hidden_execution(command: &str) -> bool {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let chars: Vec<char> = command.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\'' if !in_double_quote => in_single_quote = !in_single_quote,
            '"' if !in_single_quote => in_double_quote = !in_double_quote,
            '$' if !in_single_quote && i + 1 < chars.len() && chars[i + 1] == '(' => return true,
            '`' if !in_single_quote => return true,
            '&' if !in_single_quote && !in_double_quote => return true,
            _ => {}
        }
        i += 1;
    }
    false
}

fn is_command_executor(program: &str) -> bool {
    matches!(program,
        "bash" | "sh" | "zsh" | "dash" | "cmd" | "powershell" | "python" | "python3" | "node" | "ruby" | "perl"
    )
}

fn strip_quoted_heredoc_body(command: &str) -> String {
    // 移除单引号/双引号包裹的 heredoc body
    // heredoc 内容是数据不是 shell，避免 prose 中的危险命令名误触发
    // 实现略
    command.to_string()
}

fn safe_env_vars() -> HashMap<&'static str, String> {
    let mut vars = HashMap::new();
    for key in &[
        "PATH", "HOME", "LANG", "LC_ALL", "LC_CTYPE",
        "TERM", "USER", "SHELL", "TMPDIR",
        // Windows 必需
        "SystemRoot", "WINDIR", "COMSPEC", "PATHEXT",
        "TEMP", "TMP", "USERPROFILE", "APPDATA",
        "LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)", "ProgramW6432",
    ] {
        if let Ok(val) = std::env::var(key) {
            vars.insert(key, val);
        }
    }
    vars
}

enum CommandRiskLevel { Low, Medium, High }

fn command_risk_level(command: &str) -> CommandRiskLevel {
    let command = strip_quoted_heredoc_body(command);
    for segment in split_unquoted_segments(&command) {
        let words: Vec<&str> = segment.split_whitespace().collect();
        let Some(base) = words.first() else { continue };
        let base = normalized_command_name(base);
        if is_command_executor(base) && words.len() > 1 {
            // bash -c "rm -rf /" — 嵌套执行器中的隐藏命令
            // 标记为 High 风险，由 SafetyBelt 决定是否拒绝
        }
        if matches!(base.as_str(),
            "mkfs" | "dd" | "shutdown" | "reboot" | "halt" | "poweroff"
            | "sudo" | "su" | "mount" | "umount"
            | "iptables" | "ufw" | "firewall-cmd"
            | "useradd" | "userdel" | "usermod" | "passwd"
            | "format"
        ) {
            return CommandRiskLevel::High;
        }
    }
    CommandRiskLevel::Medium
}
```

> 与 OSCALLDESIGN.md §3.2 的 BareExec 执行模型对齐：shell 执行 + 纵深防御组合。`has_hidden_execution()` + `is_command_executor()` 对应结构守卫；`safe_env_vars()` 对应环境隔离；`command_risk_level()` 对应命令分类；`strip_quoted_heredoc_body()` 对应 heredoc 处理。

#### 4.2.6 ask_user 实现

```rust
pub struct AskUserTool;

#[derive(Deserialize)]
struct AskUserInput {
    question: String,
    options: Option<Vec<String>>,
}

impl Tool for AskUserTool {
    fn name(&self) -> &str { "ask_user" }
    fn tier(&self) -> ToolTier { ToolTier::Os }
    fn behavior_hints(&self) -> BehaviorHints {
        BehaviorHints { read_only: true, destructive: false, open_world: false }
    }
    fn permission_intents(&self, ..) -> Result<Vec<PermissionIntent>> {
        Ok(vec![])  // 问用户不需要权限审批
    }
    async fn execute(&self, input: &Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let args: AskUserInput = serde_json::from_value(input.clone())?;

        // 通过 Tauri event 请求前端展示问题 UI
        let answer = ctx.frontend_channel
            .invoke("ask_user_question", json!({
                "question": args.question,
                "options": args.options,
            }))
            .await?;

        Ok(ToolResult {
            output: answer,
            is_error: false,
            artifacts: vec![],
        })
    }
}
```

### 4.3 L2: MCP/外部工具

```rust
pub struct McpTool {
    pub bridge_name: String,        // "mcp__{server}__{tool}" 命名规范
    pub server_id: String,
    pub tool_name: String,
    pub description: String,
    pub input_schema: Value,
    pub behavior_hints: BehaviorHints,
    pub client: Arc<McpClient>,
}

impl Tool for McpTool {
    fn name(&self) -> &str { &self.bridge_name }
    fn tier(&self) -> ToolTier { ToolTier::External }
    fn behavior_hints(&self) -> BehaviorHints { self.behavior_hints }
    fn permission_intents(&self, input: &Value, _ctx: &ToolUseContext) -> Result<Vec<PermissionIntent>> {
        let mut intents = vec![PermissionIntent::new("external", self.server_id.clone())];
        if self.behavior_hints.open_world {
            intents.push(PermissionIntent::new("network", "*"));
        }
        Ok(intents)
    }
    async fn execute(&self, input: &Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        let response = self.client
            .call_tool(&self.tool_name, input)
            .await?;
        Ok(ToolResult {
            output: response.content,
            is_error: response.is_error,
            artifacts: vec![],
        })
    }
}
```

MCP Server 进程管理：

```rust
pub struct McpServerManager {
    servers: DashMap<String, McpServerProcess>,
}

pub struct McpServerProcess {
    pub id: String,
    pub config: McpServerConfig,
    pub client: Arc<McpClient>,
    pub process: Option<tokio::process::Child>,
}

#[derive(Deserialize)]
pub struct McpServerConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: Option<String>,
}

impl McpServerManager {
    /// 启动 MCP server 并注册其工具到 ToolRegistry
    pub async fn start_server(
        &self,
        config: McpServerConfig,
        registry: &ToolRegistry,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();

        // 启动子进程
        let mut child = tokio::process::Command::new(&config.command)
            .args(&config.args)
            .envs(&config.env)
            .stdout(std::process::Stdio::piped())
            .stdin(std::process::Stdio::piped())
            .spawn()?;

        // 建立 MCP 连接
        let client = McpClient::connect(/* stdio transport */).await?;

        // 发现工具
        let tools = client.list_tools().await?;

        // 注册到 ToolRegistry
        for tool_def in tools {
            let bridge_name = format!("mcp__{}__{}", &id, &tool_def.name);
            let mcp_tool = McpTool {
                bridge_name,
                server_id: id.clone(),
                tool_name: tool_def.name,
                description: tool_def.description,
                input_schema: tool_def.input_schema,
                behavior_hints: infer_behavior_hints(&tool_def),
                client: client.clone(),
            };
            registry.register(Arc::new(mcp_tool));
        }

        self.servers.insert(id.clone(), McpServerProcess {
            id: id.clone(),
            config,
            client,
            process: Some(child),
        });

        Ok(id)
    }
}

/// 从 MCP 工具元数据推断三维安全标记
/// 保守默认：无 annotation 时 destructive=true, open_world=true
/// MCP 自报只能升级（更严），不能降级（更宽松）
/// read_only 声明可采纳（它是降风险声明，但 read_only 工具仍经策略评估，不自动 Allow）
fn infer_behavior_hints(tool_def: &McpToolDefinition) -> BehaviorHints {
    let declared_read_only = tool_def.annotations.as_ref()
        .and_then(|a| a.read_only_hint)
        .unwrap_or(false);
    let declared_destructive = tool_def.annotations.as_ref()
        .and_then(|a| a.destructive_hint)
        .unwrap_or(false);
    let declared_open_world = tool_def.annotations.as_ref()
        .and_then(|a| a.open_world_hint)
        .unwrap_or(false);

    BehaviorHints {
        // read_only: 采纳 MCP 声明（降风险，但仍经策略评估）
        read_only: declared_read_only,
        // destructive: 无声明时默认 true；有声明时取 max(declared, !read_only)
        //   - MCP 声明 destructive=true → true
        //   - MCP 声明 destructive=false + read_only=true → false（可信）
        //   - MCP 声明 destructive=false + read_only=false → true（不可信）
        destructive: declared_destructive || !declared_read_only,
        // open_world: 同 destructive 逻辑
        open_world: declared_open_world || !declared_read_only,
    }
}
```

---

## 5. 前端集成

### 5.1 流式事件扩展

```typescript
interface ChatDeltaPayload {
  conversation_id: string
  message_id: string
  delta_text?: string
  delta_reasoning?: string
  delta_tool_call_id?: string
  delta_tool_call_name?: string
  delta_tool_call_arguments?: string
  tool_call_status?: ToolCallStatus
  tool_call_result?: string
  tool_call_started_at?: number
  tool_call_finished_at?: number
  tool_call_awaiting_approval?: ToolApprovalRequest
  finish_reason?: string
  usage?: TokenUsage
  error?: string
}

type ToolCallStatus = 'running' | 'success' | 'error' | 'awaiting_user' | 'cancelled'

interface ToolApprovalRequest {
  tool_call_id: string
  tool_name: string
  arguments: Record<string, unknown>
  intents: PermissionIntent[]
}
```

### 5.2 ToolCallBubble 五态渲染

```
┌─ running ──────────────┐   ┌─ success ──────────────┐
│ 🔄 fs_read             │   │ ✅ fs_read  (120ms)    │
│ /src/main.ts           │   │ /src/main.ts           │
└────────────────────────┘   │ ▼ [45 lines]           │
                              └────────────────────────┘
┌─ error ────────────────┐   ┌─ awaiting_user ────────┐
│ ❌ shell_exec           │   │ ⚠️ shell_exec          │
│ Permission denied:     │   │ rm -rf /tmp/cache      │
│ execute on system dir  │   │ [✓ Approve] [✗ Deny]  │
└────────────────────────┘   └────────────────────────┘
┌─ cancelled ────────────┐
│ ⊘ fs_delete            │
│ User rejected          │
└────────────────────────┘
```

### 5.3 审批交互流

```
RoundExecutor → ToolGateway evaluate → Ask
  → emit AgenticEvent::ToolAwaitingApproval
  → 前端 ChatDeltaPayload.tool_call_awaiting_approval
  → ToolCallBubble 渲染 [Approve] [Deny] 按钮
  → 用户点击 → Tauri command: approve_tool_call / deny_tool_call
  → Rust ApprovalChannel 收到决策
  → 继续执行或拒绝
```

---

## 6. Agent 模式集成

### 6.1 AgentMode 扩展

```typescript
interface AgentMode {
  id: string
  name: string
  description: string
  kind: 'default' | 'quick' | 'deep' | 'custom'
  prompt: string
  model: string | null
  temperature: number | null
  max_tokens: number | null
  tools: string[]                    // 可用工具白名单（空=全部）
  max_tool_tier: ToolTier           // 允许的最高工具层级
  approval_level: ApprovalLevel     // 审批级别
  presentation_rank: number
  is_builtin: boolean
  created_at: number
  updated_at: number
}

type ToolTier = 'sandbox' | 'os' | 'external'

type ApprovalLevel =
  | 'none'              // 全部自动执行
  | 'dangerous_only'    // 仅 destructive/open_world 需审批
  | 'all_os_tools'      // 所有非 Sandbox 工具需审批
```

### 6.2 内置预设更新

| 模式 | max_tool_tier | approval_level | tools |
|------|---------------|----------------|-------|
| 默认 | `sandbox` | `none` | [] |
| 快速 | `sandbox` | `none` | [] |
| 深度 | `os` | `dangerous_only` | ["fs_read","fs_list","fs_glob","fs_grep","fs_write","shell_exec","ask_user"] |
| 翻译助手 | `sandbox` | `none` | [] |

### 6.3 Agent 模式 → Policy 映射

```rust
fn policy_from_agent_mode(mode: &AgentModeConfig) -> ResolvedPermissionPolicy {
    let mut global_rules = default_global_rules();

    // approval_level 调整默认规则
    match mode.approval_level {
        ApprovalLevel::None => {
            // 所有 Ask → Allow
            global_rules.insert(0, PermissionRule::new("*", "*", PermissionEffect::Allow));
        },
        ApprovalLevel::DangerousOnly => {
            // 保持默认（read=Allow, edit/execute=Ask）
        },
        ApprovalLevel::AllOsTools => {
            // 所有非 Sandbox 操作需审批
            global_rules.push(PermissionRule::new("read", "*", PermissionEffect::Ask));
        },
    }

    // max_tier 限制：超过层级的工具直接 Deny
    let tier_rules = match mode.max_tool_tier {
        ToolTier::Sandbox => vec![
            PermissionRule::new("*", "tier:os", PermissionEffect::Deny),
            PermissionRule::new("*", "tier:external", PermissionEffect::Deny),
        ],
        ToolTier::Os => vec![
            PermissionRule::new("*", "tier:external", PermissionEffect::Deny),
        ],
        ToolTier::External => vec![],
    };

    ResolvedPermissionPolicy::new(global_rules, vec![
        PermissionConstraintLayer::new(tier_rules),
    ])
}
```

---

## 7. 项目级权限配置

用户可在项目根目录创建 `.assistant/permissions.json` 声明项目级规则：

```json
{
  "rules": [
    { "action": "read",  "resource": "src/**",    "effect": "allow" },
    { "action": "edit",  "resource": "src/**",    "effect": "allow" },
    { "action": "edit",  "resource": "*.lock",    "effect": "deny"  },
    { "action": "edit",  "resource": ".env",      "effect": "deny"  },
    { "action": "edit",  "resource": ".env.*",    "effect": "deny"  },
    { "action": "execute", "resource": "npm run *",  "effect": "allow" },
    { "action": "execute", "resource": "git *",      "effect": "allow" },
    { "action": "execute", "resource": "*",          "effect": "ask"   }
  ]
}
```

加载优先级：应用默认 → 项目配置 → Agent 模式配置，取 `most_restrictive`。

---

## 8. Crate 组织

```
src-tauri/
├── crates/
│   ├── assistant-agent/           # 现有：chat_runtime, chat_tasks
│   │   └── src/
│   │       ├── chat_runtime.rs    # 现有：StreamAccumulator
│   │       ├── chat_tasks.rs      # 现有：任务管理+取消
│   │       ├── round_executor.rs  # 新增：Agent Loop
│   │       └── ...
│   ├── assistant-tools/           # 新增 crate
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── trait.rs           # Tool trait + ToolTier + BehaviorHints
│   │       ├── registry.rs        # ToolRegistry
│   │       ├── pipeline.rs        # ToolPipeline (执行编排)
│   │       ├── gateway.rs         # ToolGateway (权限网关)
│   │       ├── policy.rs          # PermissionEffect + PermissionRule + ResolvedPermissionPolicy
│   │       ├── result.rs          # ToolResult + ToolArtifact
│   │       ├── context.rs         # ToolUseContext
│   │       ├── sandbox/           # L0 沙箱工具桥接
│   │       │   └── mod.rs
│   │       ├── os/                # L1 内置 OS 工具
│   │       │   ├── mod.rs
│   │       │   ├── fs_read.rs
│   │       │   ├── fs_write.rs
│   │       │   ├── fs_delete.rs
│   │       │   ├── fs_list.rs
│   │       │   ├── fs_glob.rs
│   │       │   ├── fs_grep.rs
│   │       │   ├── fs_mkdir.rs
│   │       │   ├── fs_move.rs
│   │       │   ├── shell_exec.rs
│   │       │   └── ask_user.rs
│   │       └── mcp/              # L2 MCP 工具
│   │           ├── mod.rs
│   │           ├── bridge.rs      # McpTool + 命名规范
│   │           ├── client.rs      # MCP 协议客户端
│   │           └── server_manager.rs  # 进程管理
│   └── assistant-providers/       # 现有：provider 适配+流式
│       └── src/
│           ├── tool_call_accumulator.rs  # 现有：流式累积
│           └── types/tool.rs            # 现有：ToolCall 数据结构
```

---

## 9. 实施路径

### Phase 1: Agent Loop 基础

> 无 OS 工具，先让 tool_call 能执行+回填

- `assistant-tools` crate 骨架：Tool trait + ToolRegistry + ToolResult
- `assistant-tools/sandbox/`：L0 沙箱工具桥接（注册现有 21 个前端工具）
- `round_executor.rs`：消费 StreamOutcome.tool_calls → 执行 → 回填 → 继续对话
- I2b 五态追踪：tool_calls 表 + 流式事件 + ToolCallBubble
- Agent 模式 `max_tool_tier: 'sandbox'` 生效

### Phase 2: Permission Policy Engine

> 安全网关先行，OS 工具还没加但框架就位

- `policy.rs`：PermissionEffect + PermissionRule + ResolvedPermissionPolicy + most_restrictive
- `gateway.rs`：ToolGateway + ApprovalChannel
- `pipeline.rs`：ToolPipeline (权限评估 → 用户审批 → 执行编排)
- Agent 模式 `approval_level` 生效
- 项目级 `.assistant/permissions.json` 加载

### Phase 3: L1 OS 工具

> 安全网关已就位，可以安全地加 OS 能力

- `os/` 模块：fs_read / fs_list / fs_glob / fs_grep (ReadOnly)
- fs_write / fs_mkdir / fs_move (ConstrainedWrite + 路径白名单)
- fs_delete (destructive + 审批)
- shell_exec (destructive + open_world + 审批 + 超时)
- ask_user (AI 主动问用户)
- Agent 模式 `max_tool_tier: 'os'` + `tools` 白名单生效

### Phase 4: L2 MCP 客户端

> 最外层，依赖前三层

- `mcp/` 模块：McpTool + bridge 命名规范 + 三维安全标记推断
- `mcp/client.rs`：MCP 协议客户端（stdio transport）
- `mcp/server_manager.rs`：进程管理（启动/连接/重连/关闭）
- MCP 工具动态注册到 ToolRegistry
- Agent 模式 `max_tool_tier: 'external'` 生效

---

## 10. 安全原则

1. **工具自描述权限，策略层决策**：工具通过 `permission_intents()` 声明"我需要什么"，ToolGateway 决定"允不允许"——关注点分离
2. **多层策略取最严**：应用默认 + 项目配置 + Agent 模式三层叠加，`most_restrictive` 组合
3. **委托只能收紧不能放宽**：`PermissionRuntimeCeiling` 禁止携带 `Allow` 规则
4. **审批流统一**：`Ask` 效果 → `awaiting_user` 状态 → 前端审批 UI → approve/reject
5. **先建框架再加能力**：Phase 1 无 OS 工具只建执行框架，Phase 2 安全网关先行，Phase 3 才加 OS 能力
6. **最小权限**：工具默认声明最高限制，Agent 模式按需放宽
7. **超时隔离**：所有工具执行带超时，Shell 执行默认 30s，防止命令挂起
8. **审计日志**：所有工具调用记录到 `tool_calls` 表，含完整参数和结果
9. **路径约束**：文件操作限制在工作区或用户授权路径内，系统敏感路径硬拒绝
10. **L0 双入口不破坏现有体验**：前端 ToolBox 独立使用不变，Agent Loop 调用是新增能力

---

## 11. 参考来源

| 设计要素 | 参考项目 | 参考文件 | 借鉴内容 |
|---------|---------|---------|---------|
| Tool trait | OpenBitFun | `agentic/tools/framework.rs` | `name/description/input_schema/is_readonly/permission_intents/execute` 统一接口 |
| PermissionEffect | OpenBitFun | `contracts/product-domains/src/tool_permissions.rs` | `Allow/Ask/Deny` + `most_restrictive` 组合 |
| ResolvedPermissionPolicy | OpenBitFun | 同上 | 主规则 + constraint_layers 多层叠加 |
| PermissionRuntimeCeiling | OpenBitFun | 同上 | 委托只能收紧不能放宽 |
| ToolPipeline | OpenBitFun | `agentic/tools/pipeline/tool_pipeline.rs` | 完整生命周期编排 |
| RoundExecutor | OpenBitFun | `agentic/execution/round_executor.rs` | Agent Loop 核心循环 |
| 内置 OS 工具 | OpenBitFun | `agentic/tools/implementations/` | Read/Write/Delete/LS/Glob/Bash/AskUser 实现 |
| file_permissions | OpenBitFun | `agentic/tools/file_permissions.rs` | 文件路径权限意图生成 |
| mcp_tool_bridge | OpenBitFun | `execution/tool-contracts/src/mcp_tool_bridge.rs` | MCP 工具命名规范 + 三维安全标记 |
| ToolPolicy trait | OpenHuman | `agent/tool_policy.rs` | 可插拔策略中间件 + `AllowAllToolPolicy` 默认 |
| ToolDispatcher | OpenHuman | `config/ops/tool_dispatcher.rs` | 多格式工具调用分发 |
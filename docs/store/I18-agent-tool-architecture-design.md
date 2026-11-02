# I18 — Agent Tool 架构决策记录（ADR）

> 本文档记录架构**决策**：为什么这样设计、参考了什么、做了什么取舍。
> **实施规格**见 [I18-agent-tool-architecture-spec.md](./I18-agent-tool-architecture-spec.md)（Step 0-10）。
> **Shell 安全子域**见 [OSCALLDESIGN.md](../designed/OSCALLDESIGN.md)。
> **ToolBox 双入口桥接**见 [I18b-tool-unified-bridge-spec.md](./I18b-tool-unified-bridge-spec.md)。

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

## 3. 架构决策

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

### 3.2 决策：三层信任模型（ToolTier）

**决策**：用 `ToolTier` 枚举（`Sandbox < Os < External`）显式分层，`PartialOrd` 语义让 `max_tool_tier` 一字段即可限制可用层级。

**取舍**：
- OpenBitFun 用 `ToolExposure`（Direct/Deferred）+ `is_readonly()` + `is_concurrency_safe(input)` 三元组实现等效分层，更灵活但更复杂
- OpenHuman 用纯 `ToolPolicy` trait 的 `check()` 方法做工具级准入，无层级枚举，更简单但缺乏结构化分层
- **选择 ToolTier**：显式枚举比隐式组合更直观，`PartialOrd` 让 Agent 模式配置只需一个字段

**类型签名**（完整定义见 I18-spec Step 0.1）：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolTier { Sandbox, Os, External }
```

### 3.3 决策：三维安全标记（BehaviorHints）

**决策**：用 `BehaviorHints { read_only, destructive, open_world }` 静态描述工具对系统的潜在影响。

**取舍**：
- OpenBitFun 用 `is_readonly()` + `is_concurrency_safe(input)` 两个方法，`is_concurrency_safe` 默认等于 `is_readonly` 但可按 input 覆盖
- **选择静态 BehaviorHints**：简化但够用——当前所有工具的安全标记不依赖 input。若未来需要动态判断（如 shell_exec 某些只读命令可并发），可扩展为 `fn behavior_hints(&self, input: &Value) -> BehaviorHints`

**推导规则**（上层消费）：
- `read_only = true` → Pipeline 可并发执行
- `destructive = true` → Gateway 触发 Ask 审批
- `open_world = true` → Gateway 触发 Ask 审批

**类型签名**（完整定义见 I18-spec Step 0.2）：

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BehaviorHints { pub read_only: bool, pub destructive: bool, pub open_world: bool }
```

### 3.4 决策：统一 Tool Trait

**决策**：三层工具统一到一个 trait，工具自描述权限需求（`permission_intents`），策略层决策是否放行——关注点分离。

**取舍**：
- OpenBitFun 的 Tool trait 含 `is_concurrency_safe(input)` 动态方法，我们简化为从 `behavior_hints().read_only` 推导
- OpenHuman 无统一 Tool trait，策略是独立中间件
- **选择统一 trait + 静态 hints**：实现一次两个入口可用（Agent / ToolBox），简化但保留核心关注点分离

**类型签名**（完整定义见 I18-spec Step 1）：

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    async fn description(&self) -> Result<String>;
    fn input_schema(&self) -> Value;
    fn tier(&self) -> ToolTier;
    fn behavior_hints(&self) -> BehaviorHints;
    fn permission_intents(&self, input: &Value, context: &ToolUseContext) -> Result<Vec<PermissionIntent>>;
    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult>;
}
```

### 3.5 决策：多层策略取最严

**决策**：`PermissionEffect { Allow, Ask, Deny }` + `most_restrictive` 组合。多层策略（应用默认 + 项目配置 + Agent 模式）叠加取最严。

**取舍**：
- OpenBitFun 同构：`PermissionEffect` + `most_restrictive` + `PermissionConstraintLayer`
- OpenHuman 的 `PolicyDecision` 是 `Allow / Deny / RequireApproval` 三值，同构
- **选择 last-match-wins 主规则 + constraint_layers 独立约束层**：与 OpenBitFun 一致，项目级和 Agent 模式级作为独立约束层，层间取 most_restrictive

**关键不变量**：`PermissionRuntimeCeiling` 禁止携带 Allow 规则——委托只能收紧不能放宽，编译期可检查。

> 完整类型定义、评估算法、默认规则集见 I18-spec Step 4。

### 3.6 决策：双入口分流

**决策**：Tool trait 实现共享，执行路径分流——Agent 经过 ToolPipeline 完整安全基础设施，ToolBox 直达执行。

**理由**：

| ToolPipeline 基础设施 | Agent 场景（LLM 自主调用） | ToolBox 场景（用户主动调用） |
|---|---|---|
| ToolGateway 审批流 | LLM 自主决策 → 需要人类审批 | 用户自己点的 → 审批自己无意义 |
| Permission Policy 评估 | 防止 LLM 越权 | 用户就是权限主体 |
| 审计日志 | 追踪 AI 自主行为 | 用户操作，不是 AI 行为 |
| 超时保护 | 防止 LLM 发起的命令挂起 | 由 Rust `invoke()` 统一兜底（I31 §4.3，已取代「UI 层自行处理」） |
| 结果回填消息历史 | Agent Loop 需要继续对话 | 结果直接返回组件渲染 |

**安全约束不缺失**：路径校验等在 `Tool.execute()` 内部，两个入口均受保护。

> ToolBox 桥接完整规格见 [I18b-tool-unified-bridge-spec.md](./I18b-tool-unified-bridge-spec.md)。

### 3.6.1 决策：受众维度与双入口分离

**决策**：`Tool` trait 增加 `audience() -> ToolAudience { agent, toolbox }`，默认 `BOTH`；两个维度各自在查询侧过滤，而不是拆成两个 registry。

**理由**：

| 备选方案 | 问题 |
|---|---|
| 拆「ToolBox registry」与「Agent registry」 | 同一实现要注册两次，`setup.rs` 维护两份清单；共享 trait 的好处被容器拆分抵消 |
| 单一 `exposed_to_agent: bool` | 只能表达「给不给 LLM」，无法表达「只给 Agent、不给前端透传入口」；将来仍要改契约 |
| **双布尔受众（采纳）** | 两个入口各有明确执行点（`tool_definitions_for_tier` / `find_for_toolbox`），一次表达完整；默认 `BOTH` 使存量工具零改动 |

**取舍**：

- 受众决定「能否被某个入口看到」，**不决定「是否启用」**。启用仍由 `conversation.tool_config.tools` 白名单（§3.8 deny-by-default）控制，两者正交。
- `fs_link` 是第一个非 `BOTH` 工具，声明 `TOOLBOX_ONLY`。它存在的意义是验证桥接链路，不是给模型的能力；受众维度让这个定位第一次可以被代码表达，而不是靠文档口头约定。
- 默认值 `BOTH` 是**迁移便利**，不是推荐写法：新增工具必须显式声明受众（见 I18 spec Step 2 硬约束）。

> 规格落地见 [I31-phase1-tool-framework-spec.md](./I31-phase1-tool-framework-spec.md) §4.2.1。

### 3.7 决策：Shell 执行层洋葱模型

**决策**：Shell 命令经 SafetyBelt → AllowlistGate → ApprovalGate → BareExec 四层洋葱栈，层间契约保证单调性和短路语义。

**取舍**：
- 纯 argv 数组执行（`execvp` 语义）可提供更强确定性保证，但丧失管道、重定向、变量展开等 shell 能力，对开发工作流影响过大
- **选择 shell 执行 + 纵深防御组合**：结构守卫 + 环境隔离 + 命令分类 + heredoc 处理，与 OpenHuman 的生产验证策略一致

> 洋葱模型完整设计、层间契约、BareExec 纵深防御见 [OSCALLDESIGN.md](../designed/OSCALLDESIGN.md)。

### 3.8 决策：MCP 工具安全标记推断

**决策**：MCP 工具的三维安全标记从其 annotations 推断，保守默认——无 annotation 时 `destructive=true, open_world=true`。

**原则**：MCP 自报只能升级（更严），不能降级（更宽松）。`read_only` 声明可采纳（降风险），但 `read_only` 工具仍经策略评估，不自动 Allow。

**推断逻辑**：
- `read_only`：采纳 MCP 声明
- `destructive`：`declared_destructive || !declared_read_only`（声明非破坏性但非只读 → 不可信 → 视为破坏性）
- `open_world`：同 destructive 逻辑

### 3.9 决策：审批超时语义

**决策**：审批请求带超时，超时后**自动 Deny**——用户未响应视为拒绝，而非无限挂起或默认放行。最安全的选择。

### 3.10 决策：先安全框架再加能力

**决策**：实施顺序为 Phase 1（Agent Loop 基础，无 OS 工具）→ Phase 2（Permission Policy Engine）→ Phase 3（L1 OS 工具）→ Phase 4（L2 MCP）。

**理由**：如果先加 OS 工具再建安全框架，LLM 一旦调 `shell_exec("rm -rf /")` 就直接执行——没有任何拦截。

### 3.11 决策：两级可用性配置 + deny-by-default

**决策**：工具可用性由两级配置叠加，且默认拒绝（deny-by-default）。

**两级层次**：

| 层级 | 何时生效 | 持久化 | 典型场景 |
|------|---------|--------|---------|
| **AgentMode 预设** | 选择 Agent 模式时 | `agent_modes` 表 | 深度模式预设 fs_read+shell_exec |
| **对话级配置** | 对话创建时继承 AgentMode 预设，之后可随时修改 | `conversations` 表 | 对话中关掉 shell_exec，只留只读工具 |

**取舍**：不设轮次级临时开关——对话级配置已支持随时修改，"临时启用一次"是 UI 交互便利性（前端可提供"临时启用"开关 + 自动还原计时器），不需要数据模型独立承载。

**deny-by-default 语义**：`tools: string[]` 是**白名单**——空数组 = 无可用工具，非空数组 = 仅这些工具可用。所有内置 Agent 模式默认 `tools: []`，用户必须显式启用工具。

**取舍**：
- "空=全部"（allow-by-default）更方便但违反最小权限——新工具自动对所有模式可用，可能意外暴露危险能力
- **选择 deny-by-default**：新增工具不会自动对任何模式可用，必须显式添加到 `tools` 列表。与安全原则"最小权限"一致

**AgentMode 预设是预填充模板**：对话创建时从 AgentMode 深拷贝到 `conversation.tool_config`，之后独立演化。AgentMode 修改不回溯影响已创建的对话。

**可用性层与权限层正交**：

| 层 | 问题 | 机制 | 时机 |
|----|------|------|------|
| **可用性层** | LLM **能否看到**这个工具？ | `resolveAvailableTools()` | 构建 LLM 请求时 |
| **权限层** | LLM **能否执行**这个操作？ | `ToolGateway.evaluate()` | 执行 tool_call 时 |

可用性层先过滤（不可用的工具不出现在 `tools` 参数中），权限层后评估（即使可用，操作仍可能被 Deny/Ask）。两者不冲突。

---

## 4. 三层工具设计决策

### 4.1 L0: 沙箱工具

**决策**：沙箱工具双入口——主窗口独立使用（现有行为不变）+ Agent Loop 调用（新增）。`tier() = Sandbox`，`permission_intents() = []`（无需权限），`execute()` 通过 Tauri event 发到前端执行。

### 4.2 L1: OS 工具

**决策**：11 个内置 OS 工具，从安全到危险排序实现。共享 `resolve_path()` + `validate_path()` 基础设施。

> 工具清单、安全标记、每个工具的 JSON Schema 规格和实现见 I18-spec Step 2。
> Shell 安全守卫（SafetyBelt / AllowlistGate / ApprovalGate / BareExec）见 [OSCALLDESIGN.md](../designed/OSCALLDESIGN.md)。

### 4.3 L2: MCP/外部工具

**决策**：MCP 工具通过 `McpTool` 桥接实现 Tool trait，命名规范 `mcp__{server}__{tool}`。`McpServerManager` 管理子进程生命周期，发现工具后动态注册到 ToolRegistry。

**隔离边界声明**：MCP server 以独立子进程运行，但当前**不提供 cgroup / seccomp / namespace 级别的 OS 隔离**。权限策略对 External 层的作用是逻辑层控制（决定是否调用），不是执行层隔离（限制调用后的行为）。严格策略禁用某 MCP server 时，必须**拒绝调用**而非宣称"沙箱内安全执行"。

> 详见 [OSCALLDESIGN.md](../designed/OSCALLDESIGN.md) §1.2。

---

## 5. 前端集成与 Agent 模式

> ChatDelta 扩展、ToolCallBubble 五态渲染、审批交互流、AgentMode 扩展字段、内置预设、Policy 映射的完整规格见 I18-spec Step 9-10。

**关键决策摘要**：
- `ToolCallStatus` 五态：`running | success | error | awaiting_user | cancelled`
- 审批交互：Ask → emit `AgenticEvent::ToolAwaitingApproval` → 前端审批 UI → approve/deny Tauri command → ApprovalChannel 收到决策
- Agent 模式 `max_tool_tier` + `approval_level` 两字段控制工具能力和审批级别
- 项目级 `.assistant/permissions.json` 声明规则，加载优先级：应用默认 → 项目配置 → Agent 模式配置，取 `most_restrictive`

---

## 6. Crate 组织

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

## 7. 实施路径

### Phase 1: Agent Loop 基础

> 无 OS 工具，先让 tool_call 能执行+回填

- `assistant-tools` crate 骨架：Tool trait + ToolRegistry + ToolResult
- `assistant-tools/sandbox/`：L0 沙箱工具桥接（注册现有 21 个前端工具）
- `round_executor.rs`：消费 StreamOutcome.tool_calls → 执行 → 回填 → 继续对话
- Tool Call 五态追踪：tool_calls_json 扩展 + 流式事件 + ToolCallBubble（详见 I18-spec Step 9）
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

## 8. 安全原则

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

## 9. 参考来源

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
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

#### 3.2.1 决策：当前不引入多表面暴露（Deferred）

**决策**：只保留 `ToolAudience { agent, toolbox }` 双布尔受众，不引入 Direct / Deferred / CodeMode 多表面暴露；满足下列任一触发条件时再升级为 bitflags。

| 触发条件 | 门槛取值 |
|---|---|
| 单模式可用工具数 | > 30 |
| `tools` 定义序列化后占上下文预算 | > 25% |
| 引入 code mode / 脚本化嵌套调用 | 一旦引入即需要 |

**理由**：Deferred 不是加一个枚举值，而是引入一套配套机制——工具搜索元数据、两段式调用（先取定义再调用）、缓存失效与权限校验时序。当前工具数不足 10，收益为负、复杂度为正。

**参考**：codex `ToolExposures` bitflags（`NONE / DIRECT / DEFERRED / CODE_MODE / ALL`）与 `ToolExposure` 六值枚举（含 `DeferredModelOnly` / `DirectModelOnly` / `CodeModeOnly`）——同一工具可在不同表面独立暴露，这是工具规模上去后的必然形态，但不宜提前引入。

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

#### 3.3.1 决策：并发安全判定保留可覆写点

**决策**：并发安全以 `behavior_hints().read_only` 为默认判定，同时保留可覆写点，供工具按 input 动态判定；默认实现集中在 trait 层，不分散到各工具。

**理由**：静态标记对 `shell_exec` 判不准——`ls` 可并发、`rm` 不可；文件读取在写操作之后也不能并发。但把判定完全下沉到每个工具，会让「没覆写的工具」失去统一默认值。

**取舍**：不整体改为 OpenBitFun 的纯动态 `is_concurrency_safe(input: Option<&Value>)`——无 input 时语义不明，且默认值散落。采用「默认推导 + 可选覆写」：未覆写者行为与现状一致，覆写者按 input 判定。

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

**安全约束不缺失**：路径校验等在 `Tool.execute()` 内部，两个入口均受保护；`shell_exec` 的执行层守卫同样在 `execute()` 内（见 §3.7.1），只有 AllowlistGate / ApprovalGate 这层审批留在 Pipeline。

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

**决策**：Shell 命令经 SafetyBelt → AllowlistGate → ApprovalGate → BareExec 四层洋葱栈，层间契约保证单调性和短路语义。各层归属按「审批 / 校验 / 执行」划分（见 §3.7.1），并非整体挂在 Pipeline 上。

**取舍**：
- 纯 argv 数组执行（`execvp` 语义）可提供更强确定性保证，但丧失管道、重定向、变量展开等 shell 能力，对开发工作流影响过大
- **选择 shell 执行 + 纵深防御组合**：结构守卫 + 环境隔离 + 命令分类 + heredoc 处理，与 OpenHuman 的生产验证策略一致

> 洋葱模型完整设计、层间契约、BareExec 纵深防御见 [OSCALLDESIGN.md](../designed/OSCALLDESIGN.md)。

#### 3.7.1 决策：Shell 守卫按「审批 / 校验 / 执行」三层归属

**决策**：洋葱栈不整体挂在 Pipeline 上，而是按「谁来回答这个问题」拆到三层：

| 层 | 守卫 | 归属判据 |
|---|---|---|
| 执行层不变量 | SafetyBelt、结构守卫、环境隔离、命令分类、heredoc 剥离 | 「无论谁调用都不该发生」——与调用者身份无关 |
| 校验层 | `validate_input` 的形状检查 + 转译 SafetyBelt 结论 | 「形状合法吗、能否自动修正」——仅 Agent 入口 |
| 审批层 | PolicyEngine、AllowlistGate、ApprovalGate | 「现在允许吗、谁来决定」——依赖人类意图，仅 Agent 入口 |

**理由**：ToolBox 不经过 Pipeline（见 §3.6 与 [I18b](I18b-tool-unified-bridge-spec.md) §2.2）。若 SafetyBelt 留在 Pipeline，ToolBox 调 `shell_exec` 将**完全无守卫**——比「需要审批」严重得多。反之，若把 AllowlistGate 下沉到 `execute()`，用户的免审批偏好会被硬编码进工具实现，且 ToolBox 入口无人可配置。

**`validate_input` 复用 SafetyBelt 而非另写一套**：两套命令判定必然漂移，出现「校验层放行、执行层拒绝」或更糟的反向情况。唯一判定源在 SafetyBelt，校验层只把结论翻译成 `ValidationResult`，命中时 `blocks_input_rewrite = true`。

**对 `shell_exec` 的约束**：受众仍为 `BOTH`（ToolBox 未来提供 shell 组件时可直接复用），但 `execute()` 内部**必须**包含执行层守卫——这是该工具的实现不变量，不能假设「反正 Pipeline 会拦」。

> 完整分层表、调用链与反例见 [OSCALLDESIGN.md](../designed/OSCALLDESIGN.md) §3.3–§3.4。

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

### 3.12 决策：结果双通道 + 错误语义分类

**决策**：`ToolResult` 除 `ok` / `data` / `error` 外增加两条独立通道：

- `result_for_assistant: Option<String>`——回填给模型的精简文本，与 `data` 并存
- `error_kind: Option<ToolErrorKind>`——错误类别，供模型判断「该改参数重试」还是「环境故障不要重试」

```rust
pub enum ToolErrorKind { Business, Timeout, Cancelled, Internal, Denied }
```

**理由**：UI 与模型对结果的需求不同。UI 要完整结构（行号、覆盖率、诊断、diff），模型只需要可纠正的短文本。共用同一份 JSON 的代价是双向浪费：UI 缺字段时只能重新拼装，模型吃进大量渲染用字段。错误分类同理——只有 `ok: false` 一个布尔时，模型无法区分「路径写错了」和「后端崩了」，只能盲目重试。

| error_kind | 产生位置 | 模型应有的反应 |
|---|---|---|
| `Business` | 工具内部业务失败（路径不存在、参数缺失、未知 action） | 可改参数重试 |
| `Timeout` | `exec::invoke` 超时兜底 | 可重试，但应缩小范围或显式提高 timeout |
| `Cancelled` | 用户取消 / 代际切换 | 不重试 |
| `Internal` | 工具返回 `Err` 后被 `invoke` 归一化 | 不重试，向用户报告 |
| `Denied` | Gateway 拒绝或审批拒绝 | 不重试，换方案 |

**取舍**：
- OpenBitFun 用 `ToolResult` 枚举（`Result` / `Progress` / `StreamChunk`）同时承载双通道与流式；我们用扁平结构 + 可选字段——枚举会改变序列化形态与前端匹配逻辑，而流式当前不采纳
- 不采纳 `image_attachments`（多模态工具结果）：当前无工具需要回传图像
- 不采纳 `Progress` / `StreamChunk` 流式输出：需要事件通道 + 前端增量渲染；长任务先用超时兜底 + 最终结果，出现构建/测试类长任务后再补（与 I18-spec Step 9.1 的 ChatDelta 扩展对齐）

**wire 约束**：`ToolResult` 经 `protocol/dto` 转出时，新增 `Option` 字段按 DTO 规则处理——键恒在、值为 `null`，前端声明为 `T | null`；不得在 DTO 层静默丢弃。

### 3.13 决策：工具输出预算——选择算法与跨轮复用

**决策**：大结果持久化不是「超阈值就全落盘」，而是三层上限 + 按大小降序选择 + 跨轮去重。

| 参数 | 取值 | 作用 |
|---|---|---|
| `per_tool_limit_chars` | 50K | 单个工具结果超过即持久化 |
| `per_round_limit_chars` | 200K | 单轮所有工具结果的总预算 |
| `preview_chars` | 2K | 持久化后回填的预览长度 |

**选择算法**：按序列化字符数降序排列，依次累加；累计超过 `per_round_limit_chars` 的结果才落盘。落盘后消息历史只保留 `<persisted-output path=…>` 引用 + 前 `preview_chars` 字符预览。

**跨轮复用**：同一会话内对同一路径、同一行范围且文件修订号未变化的重复读取，返回「已在上文返回」语义——不回传内容，只回一句提示复用上文；文件修订号变化则复用失效，必须重读。

**取舍**：
- 不做全局摘要压缩：不可逆且丢信息
- 不做「所有结果一律落盘」：IO 成本高且破坏可读性
- 不做「小结果也落盘」：落盘的价值在节省上下文，小结果没有这个收益

**参考**：OpenBitFun `ToolResultStoragePolicy` + `select_tool_result_indices_for_persistence()`（按大小降序选择）；`file_read_tool` 的 `already_served` + `file_revision` 读取回执。

### 3.14 决策：审批键规范化与会话级复用

**决策**：审批缓存以「规范化操作指纹」为键，不以工具名为键。

| 工具类型 | 指纹组成 |
|---|---|
| 文件操作 | `tool` + 规范化绝对路径（canonicalize + 分隔符/大小写归一） |
| 命令执行 | `tool` + 分词后的完整 argv + cwd + 沙箱权限档位 |
| 网络/外部 | `tool` + 规范化 host / URL pattern + 方法 |

**语义**：同一会话内命中已批准的指纹直接放行；未命中才发起审批。一次批准只对同一指纹生效，对「相似但不相同」的操作不生效。

**取舍**：
- 规范化不足 → 批准 A 路径命中 B 路径（越权）
- 规范化过度（如按命令前缀匹配）→ 批准 `cargo test` 顺带放行 `cargo publish`
- **选择全 argv 精确 + cwd 作为默认**：前缀匹配只能由工具显式声明，不得作为默认策略

**委托**：子 Agent 可读父 `GrantStore` 的记忆，但其新增批准不写入父库——与 [OSCALLDESIGN.md](../designed/OSCALLDESIGN.md) §4.3 一致。

**参考**：codex `with_cached_approval(keys, fetch)`（空 keys 直接走审批，防误命中）+ `ApplyPatchApprovalKey { environment_id, path }` + `UnifiedExecApprovalKey { environment_id, executable, command, cwd, tty, sandbox_permissions }`。

### 3.15 决策：执行前校验阶段与「不可改写不变量」

**决策**：Pipeline 在「查找工具」与「权限评估」之间插入 **validate** 阶段，并把校验失败分两类：

| 类别 | 例子 | 是否允许上游改写后重试 |
|---|---|---|
| 可修正输入错误 | schema 不匹配、枚举值非法、必填缺失 | 允许 |
| 不可改写不变量 | 路径越出工作区、目标为受保护目录、tier 越界 | **禁止**——直接拒绝 |

**理由**：把安全不变量与参数格式错误混在 `execute()` 内部，两类失败对模型呈现同一形态；一旦引入任何「自动修正输入」的机制（钩子、模型重试、UI 改写），安全不变量就可能被「修正」绕过。因此不变量违规必须在结构上打标，而非依赖调用方自觉。

**参考**：OpenBitFun `ValidationResult { result, message, error_code, meta }` + `blocks_input_rewrite()`——不变量违规打标后，PreToolUse 钩子不得改写输入。

**顺序**：`find → validate → gateway → execute → persist`。校验先于权限评估，避免为明显非法的输入弹审批框。

**只作用于 Agent 入口**：ToolBox 是用户主动调用，不经过 Pipeline，因此没有 validate 阶段，也不存在审批。它的不变量保护在 `execute()` 内部（`validate_path()` 等）。两个入口必须复用同一套判定函数，否则同一条路径可能在 Agent 侧被拒、在 ToolBox 侧放行。详见 [I18b](I18b-tool-unified-bridge-spec.md) §2.2。

---

## 4. 三层工具设计决策

### 4.1 L0: 沙箱工具

**决策**：沙箱工具双入口——主窗口独立使用（现有行为不变）+ Agent Loop 调用（新增）。`tier() = Sandbox`，`permission_intents() = []`（无需权限），`execute()` 通过 Tauri event 发到前端执行。

### 4.2 L1: OS 工具

**决策**：12 个内置 OS 工具，从安全到危险排序实现。共享 `resolve_path()` + `validate_path()` 基础设施。

#### 4.2.1 决策：编辑语义拆分为 fs_write 与 fs_edit

**决策**：文件编辑拆成两个工具——`fs_write` 负责整文件新建/覆盖，`fs_edit` 负责 `old_string` / `new_string` 唯一匹配替换。

**理由**：整文件覆盖要求模型重述全文——长文件易截断、易误删未读部分，且审批卡片无法展示「改了什么」。唯一匹配替换只要求最小上下文，天然产出 diff 供审批。

**约束**：
- `old_string` 在文件中必须唯一匹配；0 匹配或多匹配均失败，错误回传匹配次数与首个匹配位置
- `fs_edit` 标记 `read_only = false, destructive = false`；审批卡片必须展示 diff
- 不采纳 codex `apply_patch` 的自定义 patch 语法——需额外解析器与格式教学成本，收益仅在批量多文件编辑场景

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

> 本节表达**目标态**的模块边界与归属，不是进度表；各模块实际落地情况见 [I18-spec](I18-agent-tool-architecture-spec.md) 的「实施状态总览」。

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
│   │       │   ├── path.rs        # 共享：resolve_path + validate_path
│   │       │   ├── fs_read.rs
│   │       │   ├── fs_write.rs
│   │       │   ├── fs_edit.rs
│   │       │   ├── fs_delete.rs
│   │       │   ├── fs_list.rs
│   │       │   ├── fs_glob.rs
│   │       │   ├── fs_grep.rs
│   │       │   ├── fs_mkdir.rs
│   │       │   ├── fs_move.rs
│   │       │   ├── shell_exec.rs
│   │       │   ├── shell/         # 执行层守卫（execute() 内部调用，两入口共享）
│   │       │   │   ├── safety_belt.rs      # 致命命令硬拒绝（唯一命令判定源）
│   │       │   │   ├── structural_guard.rs # 隐藏执行构造拦截
│   │       │   │   ├── env_guard.rs        # env_clear + SAFE_ENV_VARS
│   │       │   │   ├── classify.rs         # 命令风险分级
│   │       │   │   └── bare_exec.rs         # tokio::process::Command
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

> 本节只记录**阶段划分与顺序决策**（为什么这个先建），不记录完成情况；进度见 [I18-spec](I18-agent-tool-architecture-spec.md) 的「实施状态总览」。

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
- fs_write / fs_edit / fs_mkdir / fs_move (ConstrainedWrite + 路径白名单)
- fs_delete (destructive + 审批)
- `os/shell/` 执行层守卫（SafetyBelt / 结构守卫 / 环境隔离 / 命令分类 / BareExec），先于 `shell_exec.rs` 落地——守卫必须在 `execute()` 内部，不能等 Pipeline 兜底（见 §3.7.1）
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
11. **审批按操作指纹而非工具名**：批准一次只对同一规范化操作生效，不因同类工具自动继承（见 §3.14）
12. **安全不变量不可被改写绕过**：路径越界等不变量违规必须在结构上打标，禁止任何输入修正机制绕过（见 §3.15）
13. **结果分通道**：UI 消费 `data`，模型消费 `result_for_assistant`，错误按 `error_kind` 分类而非单一布尔（见 §3.12）

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
| 结果双通道（§3.12） | OpenBitFun | `execution/tool-contracts/src/framework.rs` | `data` / `result_for_assistant` / `image_attachments` 三通道分离 |
| 错误语义分类（§3.12） | codex | `tools/src/tool_executor.rs` | `FunctionCallError::RespondToModel` 与取消回传「abort」的区分 |
| 输出预算与持久化（§3.13） | OpenBitFun | `execution/tool-contracts/src/tool_result_storage.rs` | 单工具/单轮/预览三层上限 + 按大小降序选择落盘 |
| 读取去重回执（§3.13） | OpenBitFun | `agentic/tools/implementations/file_read_tool.rs` | `already_served` + 文件修订号失效 |
| 审批缓存与键规范化（§3.14） | codex | `core/src/tools/approvals.rs`、`core/src/tools/runtimes/` | `with_cached_approval` + `ApplyPatchApprovalKey` / `UnifiedExecApprovalKey` |
| 执行前校验（§3.15） | OpenBitFun | `execution/tool-contracts/src/framework.rs` | `ValidationResult` + `blocks_input_rewrite()` 不变量不可改写 |
| 多表面暴露（§3.2.1，暂不采纳） | codex | `tools/src/tool_executor.rs` | `ToolExposures` bitflags 与六值 `ToolExposure` |
| 动态并发安全（§3.3.1） | OpenBitFun | `agentic/tools/framework.rs` | `is_concurrency_safe(input)` 按 input 判定 |
| 编辑语义（§4.2.1） | codex / OpenBitFun | `core/src/tools/runtimes/apply_patch.rs`；`implementations/file_edit` | `old_string`/`new_string` 唯一匹配 + diff 审批 |
# OS Call 设计文档

> **本文档职责**：Shell 执行层安全架构的**权威源**——信任模型、洋葱模型、层间契约、委托约束。
> **架构决策记录**见 [I18-agent-tool-architecture-design.md](../store/I18-agent-tool-architecture-design.md)（§3.7 洋葱模型决策、§3.8 MCP 隔离边界）。
> **实施规格**见 [I18-agent-tool-architecture-spec.md](../store/I18-agent-tool-architecture-spec.md)（Step 2 shell_exec 规格、Step 4 Policy Engine、Step 5 Gateway）。
> **ToolBox 桥接**见 [I18b-tool-unified-bridge-spec.md](../store/I18b-tool-unified-bridge-spec.md)。
> **Tool Call 状态追踪**见 I18-spec Step 9。

---

## 1. 信任模型

### 1.1 三级分层

| 层级 | 含义 | 信任模型 | 隔离保证 | 详细设计 |
|------|------|---------|---------|---------|
| Sandbox | 前端沙箱（现有 ToolBox 工具） | 完全信任，浏览器环境 | 浏览器沙箱，无 OS 访问 | I18-design §4.1, I18-spec Step 1 |
| Os | Rust 后端 OS 工具 | 信任实现，需输入校验与安全控制 | 同进程，依赖洋葱模型拦截 | I18-design §4.2, I18-spec Step 2 |
| External | MCP 外部工具 | 不信任外部进程，需权限策略 | **无 OS 级隔离**（见 §1.2） | I18-design §4.3, I18-spec Step 2 |

### 1.2 External 层隔离边界声明

MCP server 以独立子进程运行，但当前**不提供 cgroup / seccomp / namespace 级别的 OS 隔离**。因此：

- MCP 进程的文件系统、网络、子进程创建等副作用**不在洋葱模型的拦截范围内**
- 权限策略对 External 层的作用是**逻辑层控制**（决定是否调用），不是**执行层隔离**（限制调用后的行为）
- 当严格策略禁用某 MCP server 时，必须**拒绝调用**而非宣称"沙箱内安全执行"

> 未来若引入 OS 级隔离（如容器化 MCP），需更新本声明并重新评估 External 层的信任模型。

### 1.3 安全模型

所有层工具通过 `permission_intents()` 声明权限需求，ToolPipeline 统一收集并交由 PolicyEngine 决策。详见 I18-spec Step 4-5。

---

## 2. 工具调用安全路径

ToolPipeline 是安全决策与工具执行的编排层，所有工具调用必须经过此路径：

```
入口（ToolBox / Agent tool_call）
  → ToolPipeline.execute()
    → 收集 permission_intents()
    → PolicyEngine 评估 → Allow / Ask / Deny
      → Allow → 执行工具（Os 层经 Shell 洋葱栈，见 §3）
      → Ask  → 审批通道（见 §4）→ 执行工具
      → Deny → 直接拒绝
```

> RoundExecutor 完整循环、代际守卫、并发编排等实现规格见 I18-spec Step 6-7。

---

## 3. Shell 执行层

Shell 命令的 `open_world` 属性使其可做任何事，是 Os 层最复杂的安全问题。

### 3.1 洋葱模型

多层安全检查逐层包裹，每层独立负责一个安全维度：

```
┌─ ApprovalGate ──────────────────────────────┐
│  未白名单的命令需用户确认                      │
│  ┌─ AllowlistGate ────────────────────────┐  │
│  │  常用命令免审批直接放行                    │  │
│  │  ┌─ SafetyBelt ──────────────────────┐  │  │
│  │  │  致命命令硬拒绝，不可配置不可绕过     │  │  │
│  │  │  ┌─ BareExec ──────────────────┐  │  │  │
│  │  │  │  tokio::process::Command    │  │  │  │
│  │  │  └────────────────────────────┘  │  │  │
│  │  └─────────────────────────────────┘  │  │
│  └──────────────────────────────────────┘  │
└───────────────────────────────────────────┘
```

| 层 | 职责 | 可配置性 | 失败行为 |
|----|------|---------|---------|
| SafetyBelt | 致命命令硬拒绝 | 编译期固定 | 直接 Deny |
| AllowlistGate | 常用命令免审批 | 用户可配置 | 放行 → 下一层 |
| ApprovalGate | 未白名单命令需用户确认 | 用户可配置审批级别 | Approve/Deny |
| BareExec | 实际执行 | 无 | 执行结果 |

> Shell 分层栈完整实现规格见 I18-spec Step 2 (shell_exec)。

### 3.2 层间契约

洋葱模型的安全保证依赖以下层间交互规则：

**单调性**：SafetyBelt 的 Deny 是**最终决定**，不可被外层（AllowlistGate、ApprovalGate）覆盖。一旦 SafetyBelt 拒绝，该命令不会到达 BareExec。

**短路语义**：各层按由内到外的顺序求值，任一层 Deny 即短路返回，不继续求值外层：

```
SafetyBelt  → Deny  → 直接返回（AllowlistGate、ApprovalGate 不执行）
            → Pass  → AllowlistGate
AllowlistGate → Pass  → ApprovalGate
              → Pass  → ApprovalGate（AllowlistGate 放行不跳过审批）
ApprovalGate  → Approve → BareExec
              → Deny   → 直接返回
```

> AllowlistGate 放行不跳过 ApprovalGate——审批层始终有机会检查。AllowlistGate 的作用是**免审批**（ApprovalGate 自动 Approve），不是**跳过审批**。

**BareExec 执行模型**：BareExec 使用 shell 执行（`cmd /C` / `bash -c`），配合以下纵深防御组合保证安全：

| 防御层 | 作用 | 详见 |
|--------|------|------|
| 结构守卫 | 阻止隐藏执行构造（`$(...)` / `` `...` `` / `&` / `bash -c "..."`），防止审批时用户看到的命令与实际执行的不一致 | I18-spec Step 2 (shell_exec) |
| 环境隔离 | `env_clear()` + `SAFE_ENV_VARS` 白名单，防止 API key / secret 泄漏到子进程 | I18-spec Step 2 (shell_exec) |
| 命令分类 | 分词 + 基名提取 → 风险分级（High/Medium/Low），High 风险硬拒绝 | I18-spec Step 2 (shell_exec) |
| heredoc 处理 | 剥离引号包裹的 heredoc body，避免 prose 中的危险命令名误触发 | I18-spec Step 2 (shell_exec) |

> 纯 argv 数组执行（`execvp` 语义）可提供更强的确定性保证，但会丧失管道、重定向、变量展开等 shell 能力，对开发工作流影响过大。当前选择 shell 执行 + 纵深防御组合，与 openhuman 的生产验证策略一致。若未来引入 OS 级沙箱（如 dsh 的 Landlock 方案），可重新评估此选择。

### 3.3 与 ToolPipeline 的关系

Shell 洋葱栈是 Shell 工具的**独立安全层**，由 ToolPipeline 在权限评估通过后调用。调用链：

```
ToolPipeline.execute()
  → PolicyEngine 评估 → Allow/Ask
    → Shell 洋葱栈（SafetyBelt → AllowlistGate → ApprovalGate → BareExec）
```

PolicyEngine 与洋葱栈是**两级安全**：PolicyEngine 做工具级权限决策（"shell_exec 是否允许"），洋葱栈做命令级安全检查（"具体命令是否安全"）。两者独立，PolicyEngine Allow 不跳过洋葱栈。

---

## 4. 审批通道

ApprovalGate 通过审批通道与用户交互，其架构属性如下：

### 4.1 审批决策

| 属性 | 设计 |
|------|------|
| 决策类型 | Approve（本次允许）/ Deny（本次拒绝）/ AlwaysApprove（记忆授权） |
| 记忆授权 | AlwaysApprove 持久化到 GrantStore，后续同 (action, resource) 自动 Allow，无需再审批 |
| 审计记录 | 所有审批决策（含 Allow 自动放行）写入 AuditStore，只追加不可修改 |

> GrantStore / AuditStore 接口规格见 I18-spec Step 5。

### 4.2 超时语义

审批请求带超时，超时后**自动 Deny**。这是最安全的选择：用户未响应视为拒绝，而非无限挂起或默认放行。

### 4.3 委托下的审批

子 Agent 的审批通道继承父 Agent 的 GrantStore 记忆，但子 Agent 的 AlwaysApprove 决策**不写入父 Agent 的 GrantStore**——委托获得的授权不应反向传播。

---

## 5. 委托约束

当 Agent 委托子 Agent 执行时，安全策略必须满足以下不变量：

### 5.1 单向收紧

子 Agent 的有效权限是父 Agent 权限与子 Agent 自身策略的**交集**，取最严。子 Agent 不可放宽父 Agent 的 Deny 规则。

### 5.2 运行时天花板

委托时构造 `PermissionRuntimeCeiling`：仅含 Deny / Ask 规则，**禁止携带 Allow 规则**。含 Allow 规则的天花板构造失败——委托只能收紧不能放宽是编译期可检查的不变量。

### 5.3 层级继承

子 Agent 的 `max_tool_tier` 不可超过父 Agent。若父 Agent 为 Os 层，子 Agent 不可升级到 External 层。

> 委托策略的完整解析与评估规格见 I18-spec Step 4。
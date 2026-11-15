# OS Call 设计文档

> **状态（2026-11-06）**：本文的 Shell 执行层设计**暂不实施**——`shell` 工具已改为暂不提供，理由与恢复条件见 I38c §13（已归档）。本文保留为评估留档，不构成实施要求。

> **本文档职责**：Shell 执行层安全架构的**权威源**——信任模型、洋葱模型、层间契约、委托约束。
> **架构决策记录**见 I18-agent-tool-architecture-design.md（已归档）（§3.7 洋葱模型决策、§3.7.1 守卫分层归属、§3.8 MCP 隔离边界）。
> **实施规格**见 I38c §13（已归档）（`shell` 暂不提供）、I38a（已归档）（Policy / Gateway）。
> **ToolBox 桥接**见 I18b-tool-unified-bridge-spec（已归档）。
> **Tool Call 状态追踪**见 I38d（已归档）。
> **阶段与门禁**见 [I38-roadmap](../store/I38-agent-roadmap.md)。

---

## 1. 信任模型

### 1.1 三级分层

| 层级 | 含义 | 信任模型 | 隔离保证 | 详细设计 |
|------|------|---------|---------|---------|
| Sandbox | 前端沙箱（现有 ToolBox 工具） | 完全信任，浏览器环境 | 浏览器沙箱，无 OS 访问 | I18-design §4.1, I18 spec Step 1 |
| Os | Rust 后端 OS 工具 | 信任实现，需输入校验与安全控制 | 同进程，依赖执行层守卫拦截（见 §3.4） | I18-design §4.2, I38c-shell |
| External | MCP 外部工具 | 不信任外部进程，需权限策略 | **无 OS 级隔离**（见 §1.2） | I18-design §4.3, I38e |

### 1.2 External 层隔离边界声明

MCP server 以独立子进程运行，但当前**不提供 cgroup / seccomp / namespace 级别的 OS 隔离**。因此：

- MCP 进程的文件系统、网络、子进程创建等副作用**不在洋葱模型的拦截范围内**
- 权限策略对 External 层的作用是**逻辑层控制**（决定是否调用），不是**执行层隔离**（限制调用后的行为）
- 当严格策略禁用某 MCP server 时，必须**拒绝调用**而非宣称"沙箱内安全执行"

> 未来若引入 OS 级隔离（如容器化 MCP），需更新本声明并重新评估 External 层的信任模型。

### 1.3 安全模型

所有层工具通过 `permission_intents()` 声明权限需求，Agent 入口由 ToolPipeline 统一收集并交由 PolicyEngine 决策（ToolBox 入口无审批，见 §2）。详见 I38a（已归档）§2 / §4。

---

## 2. 工具调用安全路径

两个入口、两条路径。安全能力按「是否依赖人类意图」归属，而不是全部堆在 Pipeline 上：

| 入口 | 路径 | 审批 | 执行层不变量 |
|---|---|---|---|
| Agent tool_call | RoundExecutor → ToolPipeline → `execute()` | 需要（LLM 自主发起） | 受保护 |
| ToolBox | `os_tool_exec` → `execute()` | 不需要（用户点击即授权） | 受保护 |

```
Agent tool_call
  → ToolPipeline.execute()
    → validate_input()      （可修正错误 / 不可改写不变量分类）
    → 收集 permission_intents()
    → PolicyEngine 评估 → Allow / Ask / Deny
      → Ask  → 审批通道（见 §4）
      → Allow → execute()
        → 执行层不变量（SafetyBelt + 结构守卫 + 环境隔离 + 命令分类）
        → BareExec

ToolBox（os_tool_exec）
  → registry.find_for_toolbox()
  → execute()
    → 执行层不变量（同一套 SafetyBelt + 守卫）
    → BareExec
```

**分界线**：凡是「无论谁调用都不该发生」的约束，属于**执行层不变量**，必须在 `execute()` 内部、两个入口共享；凡是「需要某个人类决定」的约束，属于**审批层**，只存在于 Agent 入口。

> 这条分界线决定了洋葱栈不能整体留在 Pipeline：若整体留在 Pipeline，ToolBox 调用 shell 就会完全绕过 SafetyBelt。守卫分层归属见 §3.4。

> RoundExecutor 完整循环、代际守卫、并发编排等实现规格见 I38b（已归档）与 I38a（已归档）§4-C5。

---

## 3. Shell 执行层

Shell 命令的 `open_world` 属性使其可做任何事，是 Os 层最复杂的安全问题。

### 3.1 洋葱模型

多层安全检查逐层包裹，每层独立负责一个安全维度。图中**双线框**为执行层（两入口共享），**单线框**为审批层（仅 Agent 入口）：

```
╔═ 执行层（execute() 内部，两入口共享）══════════════════╗
║  ┌─ SafetyBelt ──────────────────────────────┐        ║
║  │  致命命令硬拒绝，不可配置不可绕过              │        ║
║  │  ┌─ 结构守卫 / 环境隔离 / 命令分类 ────────┐  │        ║
║  │  │  防隐藏执行构造、清空环境变量、风险分级      │  │        ║
║  │  │  ┌─ BareExec ──────────────────────┐  │  │        ║
║  │  │  │  tokio::process::Command        │  │  │        ║
║  │  │  └─────────────────────────────────┘  │  │        ║
║  │  └───────────────────────────────────────┘  │        ║
║  └───────────────────────────────────────────┘        ║
╚════════════════════════════════════════════════════════╝

  ┌─ 审批层（仅 Agent 入口，Pipeline 内）──────────────┐
  │  ApprovalGate  未白名单的命令需用户确认              │
  │  AllowlistGate 常用命令免审批（自动 Approve，不跳过） │
  └──────────────────────────────────────────────────┘
```

| 层 | 归属 | 职责 | 可配置性 | 失败行为 |
|----|------|------|---------|---------|
| SafetyBelt | 执行层 | 致命命令硬拒绝 | 编译期固定 | 直接 Deny |
| 结构守卫 / 环境隔离 / 命令分类 | 执行层 | 防隐藏执行构造、清空环境变量、风险分级 | 编译期固定 | 直接 Deny |
| AllowlistGate | 审批层 | 常用命令免审批 | 用户可配置 | 放行 → 下一层 |
| ApprovalGate | 审批层 | 未白名单命令需用户确认 | 用户可配置审批级别 | Approve/Deny |
| BareExec | 执行层 | 实际执行 | 无 | 执行结果 |

> Shell 分层栈**暂不实施**（`shell` 暂不提供，见 I38c §13（已归档））；分层归属理由见 §3.4，保留作将来恢复时的参考。

### 3.2 层间契约

洋葱模型的安全保证依赖以下层间交互规则：

**单调性**：SafetyBelt 的 Deny 是**最终决定**，不可被任何外层（结构守卫、AllowlistGate、ApprovalGate）覆盖。一旦 SafetyBelt 拒绝，该命令不会到达 BareExec。

**短路语义**：执行层由内到外求值，任一层 Deny 即短路返回；审批层位于执行层之外，只在 Agent 入口求值：

```
【执行层｜两入口共享】
SafetyBelt      → Deny  → 直接返回（后续层不执行）
                → Pass  → 结构守卫 / 环境隔离 / 命令分类
结构守卫等      → Deny  → 直接返回
                → Pass  → BareExec

【审批层｜仅 Agent 入口，位于执行层之前】
AllowlistGate   → 命中白名单 → ApprovalGate 自动 Approve（不跳过审批层）
                → 未命中     → ApprovalGate 请求用户确认
ApprovalGate    → Approve    → 进入执行层
                → Deny       → 直接返回
```

> AllowlistGate 放行不跳过 ApprovalGate——审批层始终有机会检查。AllowlistGate 的作用是**免审批**（ApprovalGate 自动 Approve），不是**跳过审批**。
>
> 审批层先于执行层求值：用户不会为一个必然被 SafetyBelt 拒绝的命令做决策。但**执行层不因审批通过而豁免**——批准 `rm -rf` 不会让 SafetyBelt 放行。

**BareExec 执行模型**：BareExec 使用 shell 执行（`cmd /C` / `bash -c`），配合以下纵深防御组合保证安全：

| 防御层 | 归属 | 作用 | 详见 |
|--------|------|------|------|
| 结构守卫 | 执行层 | 阻止隐藏执行构造（`$(...)` / `` `...` `` / `&` / `bash -c "..."`），防止审批时用户看到的命令与实际执行的不一致 | I38c-shell |
| 环境隔离 | 执行层 | `env_clear()` + `SAFE_ENV_VARS` 白名单，防止 API key / secret 泄漏到子进程 | I38c-shell |
| 命令分类 | 执行层 | 分词 + 基名提取 → 风险分级（High/Medium/Low），High 风险硬拒绝 | I38c-shell |
| heredoc 处理 | 执行层 | 剥离引号包裹的 heredoc body，避免 prose 中的危险命令名误触发 | I38c-shell |

> 纯 argv 数组执行（`execvp` 语义）可提供更强的确定性保证，但会丧失管道、重定向、变量展开等 shell 能力，对开发工作流影响过大。当前选择 shell 执行 + 纵深防御组合，与 openhuman 的生产验证策略一致。若未来引入 OS 级沙箱（如 dsh 的 Landlock 方案），可重新评估此选择。

### 3.3 与 ToolPipeline / 入口的关系

Shell 守卫不是一个整体挂在 Pipeline 上，而是**按归属拆到两层**：

| 层 | 位置 | 求值方 |
|---|---|---|
| 执行层（SafetyBelt + 结构守卫 + 环境隔离 + 命令分类 + BareExec） | `ShellExecTool::execute()` 内部 | 两个入口都调用 `execute()`，因此都受保护 |
| 审批层（AllowlistGate + ApprovalGate） | `ToolPipeline` 内 | 仅 Agent 入口 |

Agent 入口调用链：

```
ToolPipeline.execute()
  → validate_input()  → 复用 SafetyBelt 的判定结果（不重复实现）
  → PolicyEngine 评估 → Allow/Ask
    → AllowlistGate → ApprovalGate
      → ShellExecTool::execute()（内含 SafetyBelt → 守卫 → BareExec）
```

ToolBox 入口调用链：

```
os_tool_exec
  → registry.find_for_toolbox()
    → ShellExecTool::execute()（同一套 SafetyBelt → 守卫 → BareExec）
```

**为什么 `validate_input` 要复用 SafetyBelt**：若校验层另写一套命令判定，两处规则迟早漂移——出现「校验层放行、执行层拒绝」或更糟的反向情况。唯一判定源在 SafetyBelt，`validate_input` 只把它的结论翻译成 `ValidationResult`（命中时 `blocks_input_rewrite = true`，因为它是安全不变量）。

### 3.4 守卫分层归属：审批 / 校验 / 执行

一个约束该放哪一层，取决于它回答的是哪个问题：

| 问题 | 归属 | 判据 |
|---|---|---|
| 「这个操作**永远**不该发生吗？」 | 执行层不变量（`execute()` 内部） | 与调用者身份无关，两入口都拒绝 |
| 「这个操作**形状**合法吗？能否自动修正？」 | 校验层（`validate_input`，仅 Agent） | 可分类为「可修正」或「不可改写」 |
| 「这个操作**现在**允许吗？谁来决定？」 | 审批层（PolicyEngine + AllowlistGate + ApprovalGate，仅 Agent） | 依赖人类意图与当前策略 |

对照到具体守卫：

| 守卫 | 层 | 理由 |
|---|---|---|
| SafetyBelt（致命命令） | 执行层 | 任何人都不该执行 `rm -rf /`，与是否点击、是否批准无关 |
| 结构守卫（隐藏执行构造） | 执行层 | 防止「所见非所执行」，无论审批与否都要拦 |
| 环境隔离（`env_clear`） | 执行层 | 密钥泄漏与调用者身份无关 |
| 命令分类（风险分级） | 执行层 | High 风险硬拒绝是能力边界，不是策略选择 |
| heredoc 剥离 | 执行层 | 属命令解析正确性，非权限 |
| `validate_input` | 校验层 | 只做形状检查 + 把 SafetyBelt 结论转成 `ValidationResult` |
| AllowlistGate | 审批层 | 「免不免审批」是人类偏好 |
| ApprovalGate | 审批层 | 唯一真正需要人类决策的环节 |

**反例（不该这么做）**：把 SafetyBelt 放在 Pipeline 里 → ToolBox 调 shell 完全无守卫；把 AllowlistGate 放进 `execute()` → ToolBox 调用时无人可配置、且审批层的用户偏好被硬编码进工具实现。

PolicyEngine 与执行层守卫是**两级安全**：PolicyEngine 做工具级权限决策（"shell 是否允许"），执行层守卫做命令级安全检查（"具体命令是否安全"）。两者独立，PolicyEngine Allow 不跳过执行层守卫。

---

## 4. 审批通道

ApprovalGate 通过审批通道与用户交互，其架构属性如下：

### 4.1 审批决策

| 属性 | 设计 |
|------|------|
| 决策类型 | Approve（本次允许）/ Deny（本次拒绝）/ AlwaysApprove（记忆授权） |
| 记忆授权 | AlwaysApprove 持久化到 GrantStore，后续同**规范化操作指纹**（`ApprovalKey`）自动 Allow，无需再审批；指纹不以工具名为键 |
| 审批键 | 文件操作 = 规范化路径（`canonicalize` 解析符号链接）；命令 = 完整 argv + cwd + 沙箱档位；**不做命令前缀匹配**（批准 `cargo test` 不得命中 `cargo publish`） |
| 空键不命中 | 无法生成指纹（如命令解析失败）时直接走审批，不允许命中缓存 |
| 审计记录 | 所有审批决策（含 Allow 自动放行）写入 AuditStore，只追加不可修改 |

> GrantStore / AuditStore 接口规格与 `ApprovalKey` 归一规则见 I38a（已归档）§2.6 / §2.9；设计理由见 I18 design（已归档） §3.14。

### 4.2 超时语义

审批请求带超时，超时后**自动 Deny**。这是最安全的选择：用户未响应视为拒绝，而非无限挂起或默认放行。

### 4.3 委托下的审批

子 Agent 的审批通道继承父 Agent 的 GrantStore 记忆，但子 Agent 的 AlwaysApprove 决策**不写入父 Agent 的 GrantStore**——委托获得的授权不应反向传播。

继承是**按指纹继承**而非按工具名继承：子 Agent 的 cwd / argv / 路径若与父 Agent 不同，指纹即不同，不会命中父库中的批准。因此「子 Agent 换个目录执行同样的命令」仍需重新审批。

---

## 5. 委托约束

当 Agent 委托子 Agent 执行时，安全策略必须满足以下不变量：

### 5.1 单向收紧

子 Agent 的有效权限是父 Agent 权限与子 Agent 自身策略的**交集**，取最严。子 Agent 不可放宽父 Agent 的 Deny 规则。

### 5.2 运行时天花板

委托时构造 `PermissionRuntimeCeiling`：仅含 Deny / Ask 规则，**禁止携带 Allow 规则**。含 Allow 规则的天花板构造失败——委托只能收紧不能放宽是编译期可检查的不变量。

### 5.3 层级继承

子 Agent 的 `max_tool_tier` 不可超过父 Agent。若父 Agent 为 Os 层，子 Agent 不可升级到 External 层。

> 委托策略的完整解析与评估规格见 I38a（已归档）§2.3 / §2.4。
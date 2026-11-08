# I38d — P6：前端集成与 Agent 模式规格

| 项 | 值 |
|---|---|
| 状态 | 现行（P6 规格；任务级进度见 [I38d tasklist](./I38d-agent-frontend-modes-tasklist.md)，阶段级结论见 [roadmap](./I38-agent-roadmap.md) §2） |
| 日期 | 2026-11-04 |
| 阶段 | P6（见 [I38 roadmap](./I38-agent-roadmap.md)） |
| 关联 | [I18 spec](./I18-agent-tool-architecture-spec.md) Step 9-10（已迁入本文）、[I36 spec](./I36-agent-runtime-spec.md) Step 7（事件与 DTO）、[I38a](./I38a-tool-control-layer-spec.md)（审批端口）、[I38b](./I38b-agent-loop-spec.md)（工具事件）、[I18b](./I18b-tool-unified-bridge-spec.md)（ToolBox 桥接） |
| 影响范围 | `src-tauri/src/protocol/dto`、`src/types`（契约生成）、`src-tauri/src/cmd/conversation.rs`（模式可改）、前端 chat 域（`ConversationView` + `ChatLayout` / `AgentLayout`）、设置页 Agent 模式、对话工具栏 |

> **重要修正**：P0 已从 `ChatDeltaPayloadDto` 删除 `delta_tool_call_id/name/arguments`。P6 **不得**把这些字段加回 chat delta；工具调用一律走独立的 `agent-tool-call` 事件。
> **命名规则**：wire DTO 与前端 interface 一律 camelCase；Rust 存储 struct 与 DB 列一律 snake_case，转换只在 DTO 层。

---

## 1 目标与非目标

### 1.1 目标

| # | 交付 |
|---|---|
| 1 | 工具调用五态渲染（running / success / error / awaiting_user / cancelled） |
| 2 | 审批交互闭环（允许 / 拒绝 / 本会话允许 / 始终允许 / 超时自动 Deny） |
| 3 | 前端只订阅后端 phase，不自行推导、不轮询 idle |
| 4 | 两级工具装配：模式级 `toolConfig`（新会话初始值）+ 会话级 `conversations.tool_config_json`（布局内调整后持久化）；装配 UI 位于 `AgentLayout`（§4.4 / §4.7） |
| 5 | `resolveAvailableTools` deny-by-default + 层级硬上限 |
| 6 | AgentMode → PermissionPolicy 映射；项目级权限加载入口 |
| 7 | chat / agent **同会话内可切换**：每次发送按会话当前 `mode.kind` 选路，默认 chat；turn 进行中禁止切换（§4.8） |
| 8 | `ask_user` 提问交互（P6-C8；与 C1–C7 解耦，**不属 P6 主体退出条件**，见 §2.4 / §3.6） |
| 9 | **共同视图 + 双布局**：`ConversationView` 为唯一入口，`ChatLayout` / `AgentLayout` 复用共享组件，布局由链路决定（§4.10） |
| 10 | 模式模型收敛：内置模式为 `builtin-default`(chat) / `builtin-translate`(chat) / `builtin-agent`(agent)，`builtin-quick` / `builtin-deep` 下线；自定义模式创建时可选 chat 或 agent；旧 `kind` 值（`default` / `quick` / `deep` / `custom`）退化为纯兼容标签、不决定分支（§4.3 / §4.11） |

### 1.2 非目标

- 后端工具实现（P5）；MCP UI（P7）。
- 子代理、排队输入（I36 design §3.19）；子代理的会话树 / roster 投影归 P8.2。
- 重写历史消息渲染；历史 `tool_calls_json` 的展示仅按 §3.4 处理。
- 不做 chat / agent 两套独立视图或独立窗口：布局差异只体现在工具区与工具栏，消息渲染、输入、附件、上下文栏共用（§4.10）。

### 1.3 前置

1. P4 已发射 `agent-tool-call` / `session-state-changed`，且 DTO 可生成。
2. P5 至少只读组工具可用；危险组上线需审批 UI 同期（P5 §7.3）。
3. I36 spec 的 phase 是唯一事实源；前端不得自行推导。

---

## 2 事件与 DTO

### 2.1 工具事件

完整声明见设计来源 [I38b §6.1.1](./I38b-agent-loop-spec.md)（落点 `src-tauri/src/protocol/dto/agent.rs`），生成到 `src/types/protocol.contract.ts`。本节只写 P6 增量与前端消费约束，不复述字段。

| 类型 | P6 前 | P6 后 |
|---|---|---|
| `AgentToolCallDto` | `conversationId` / `turnId` / `iteration` / `callId` / `name` / `payload` | **不变** |
| `AgentToolCallPayloadDto` | `tagged enum`（`phase`）：`started` / `argsDelta{delta}` / `finished{result,confirmationWaitMs}` / `failed{error,errorKind,confirmationWaitMs}` | **追加** `awaitingApproval{approval: ToolApprovalRequestDto}`，既有四变体不动 |

即 `{ "phase": "awaitingApproval", "approval": {...} }`。既有四个变体已被 P4 消费，P6 不得修改其字段名或语义。

**约束**：

- 输出 DTO 的 `Option` 字段**不加** `skip_serializing_if`：键恒在、值 `null`，前端声明 `T | null`。
- 事件名 `agent-tool-call`；`payload.phase` 是封闭枚举，前端不得兜底窄化。
- 工具调用结果同时进入消息历史；事件只负责实时渲染，不作为事实源。
- `payload` 是 **tagged enum，不是「扁平结构 + 独立 phase 枚举」**：`phase` 与同变体的其他字段永远同时存在，扁平化会让 `argsDelta` 携带空的 `result`、`finished` 携带空的 `delta`，前端必须自己判断「哪个字段在本 phase 有意义」。

### 2.2 审批请求与命令

审批载荷复用 P2 的 `ApprovalRequest`，经 DTO 转出。**形状与 [I38a §2.8.1](./I38a-tool-control-layer-spec.md) 一一对应**（`Xxx` → `XxxDto`），本节不复述字段声明；完整类型由 `npm run contract:gen` 生成到 `src/types/protocol.contract.ts`。

统一 derive：`Debug, Clone, Serialize, Deserialize, PartialEq, TS`；枚举另加 `Eq`，输入 DTO 另加 `deny_unknown_fields`，输入 `Option` 字段加 `#[ts(optional = nullable)]`。

`rename_all` **只约束 wire DTO**（`protocol/dto/*.rs`），不要套到 `assistant-tools` 的内部类型上——内部类型不参与 wire，序列化成什么形状完全由 DTO 侧的 `From` 决定。DTO 侧分两类：

| 位置 | 规则 | 先例 |
|---|---|---|
| DTO 结构体字段 | `camelCase` | `AgentToolCallDto` / `OsToolResultDto` |
| DTO 枚举变体 | 以该类型 spec 为准：`ToolTierDto` / `ApprovalLevelDto` 用 `snake_case`（与既有 `ConversationListChangeKindDto` 一致）；tagged 枚举（`ApprovalReasonDto`）用 `tag = "kind"` + `camelCase` | `TurnPhaseDto`（camelCase）/ `ConversationListChangeKindDto`（snake_case） |

**内部类型的 serde 属性与 wire 无关**：`assistant-tools` 里 `ToolTier` / `PermissionIntent` 恰好也 derive 了 `Serialize` 并带 `rename_all = "camelCase"`（供内部序列化用），但那是内部约定，**不得**当成 wire 契约的兜底——`ToolTierDto` / `PermissionIntentDto` 必须自己声明 `rename_all` 并做 `From` 映射，否则内部类型一改，wire 就静默漂移。

**既有 DTO 复用**：`ToolTierDto` / `ApprovalLevelDto` 由 P5（[I38c §3.1](./I38c-os-tools-spec.md)）新建；`PermissionIntentDto` 由 **P6 新建**——P2 只有 `assistant-tools::PermissionIntent`（无 `TS` 派生，`src-tauri` 中无对应 DTO），因此 P6 需在 `protocol/dto/agent.rs` 补一个同形 `PermissionIntentDto` + `From` 转换。本节其余 DTO 均为 P6 新建。

**生成方式**：Rust DTO 手写一次（唯一源），前端类型由 `npm run contract:gen` 自动产出，**前端与文档都不手写**。三个 ts-rs 相关的落点：

| 情形 | 写法 |
|---|---|
| `Duration` | ts-rs 无该 impl，wire 用 `timeout_ms: u64`（前端 `timeoutMs: number`） |
| `serde_json::Value` | `#[ts(type = "unknown")]`，否则不生成 |
| tagged 枚举 DTO | `#[serde(tag = "kind", rename_all = "camelCase")]` 可直接生成判别联合，先例见 `ContentBlockDto`（`tag = "type"`） |

| DTO | 字段 | 约束 |
|---|---|---|
| `ToolApprovalRequestDto` | `conversationId` / `turnId` / `callId` / `toolName` / `arguments` / `intents` / `timeoutMs` / `context: ApprovalContextDto` | 顶层**不再**加审批信息字段 |
| `ApprovalContextDto` | `tier: ToolTierDto` / `behavior: ToolBehaviorDto` / `reasons: ApprovalReasonDto[]` / `preview: ApprovalPreviewDto \| null` / `extensions: ApprovalExtensionDto[]` | 恒存在，不可为 `null` |
| `ApprovalReasonDto` | 封闭枚举 `defaultPolicy{rule}` / `approvalLevel{level}` / `projectRule{action,resource}` / `tierCeiling{tier}` / `advisor{rationale}` | `tag = "kind"`；`advisor` P6 恒不出现 |
| `ApprovalPreviewDto` | `summary` / `truncated` / `estimated` | `summary` 已由后端格式化；整体 `null` = 未知 |
| `ApprovalExtensionDto` | `kind: String` / `label` / `value` / `severity: ApprovalExtensionSeverityDto` | `kind` 开放；`label` / `value` 后端已格式化 |
| `ApprovalExtensionSeverityDto` | `info` / `caution` / `danger` | 封闭三档，仅用于着色 |
| `ToolBehaviorDto` | `readOnly` / `destructive` / `openWorld` | 三个 `bool` |
| `ToolApprovalDecisionDto` | `conversationId` / `callId` / `decision: ToolApprovalKindDto` / `reason?: string \| null` | 输入 DTO；`deny_unknown_fields` |
| `PermissionGrantDto` | `grantKey` / `tool` / `scope: GrantScopeDto` / `conversationId: string \| null` / `scopeSummary` / `createdAt` | 输出 DTO；`grantKey` 不透明，`scope` / `conversationId` 由后端解析前缀得出 |
| `GrantScopeDto` | `session` / `global` | 封闭两档，与卡片按钮一一对应 |
| `SetGrantScopeArgsDto` | `grantKey` / `scope: GrantScopeDto` / `conversationId?: string \| null` | 输入 DTO；`scope = session` 时 `conversationId` 必填 |

`ToolApprovalKindDto` 为 `approve` / `deny` / `allowSession` / `allowAlways`（`rename_all = "camelCase"`）。`allowSession` 落会话级授权、`allowAlways` 落全局授权（§3.3.1），旧 `alwaysApprove` 已移除。

新增类型必须登记到 `src-tauri/tests/protocol_bindings.rs` 的 `declarations` 数组——该测试扫描 `protocol/dto/*.rs` 中带 `TS` 派生的类型，漏登记即失败。

**DTO 约束**：

- `context` **恒存在且不可为 `null`**：它是审批信息的唯一入口，前端不得对 `context` 做空值兜底。
- `context` 内的 `Option` 字段不加 `skip_serializing_if`：`preview` 键恒在、值 `null`，前端声明 `ApprovalPreviewDto | null`。
- `reasons` 恒非空（至少一条 `defaultPolicy`），前端不得对空数组兜底成「原因未知」。
- `extensions` 恒存在（可为空数组）；`kind` 是开放 `String`，前端**不得**白名单过滤，未知 `kind` 必须按 `severity` 排序后原样渲染 `label` / `value`。
- `ApprovalReasonDto` 是封闭枚举（`tag = "kind"`），前端不得窄化；`advisor` 分支 P6 恒不出现，前端须能渲染未知分支的通用降级（P10 才填充）。
- **新增审批信息只能加进 `context`**（或 `extensions`），不得新增 `ToolApprovalRequestDto` 顶层字段——否则前端又要按 `tool_name` 分支判断哪些字段有意义。
- **`arguments` 是已解析的 JSON 对象，不是字符串**：后端从 `ToolCall.arguments`（`Value`，已在 `agent/tool_call.rs` 完成一次解析 + 截断修复）直接 clone 进审批请求，前端拿到即对象，**不得**再 `JSON.parse`。也**不得**复用消息历史里 `ContentBlockDto::ToolUse.input`（那是 `String`，双重编码）——两者可能因截断修复而不同，审批必须用 `arguments`，才能保证「审批看到的」与「实际执行的」是同一份。
- 命令：`approve_agent_tool_call` / `deny_agent_tool_call`（或统一 `resolve_agent_tool_approval`，只保留一个入口）。
- 前端倒计时基于 `timeoutMs`；超时后后端自动 Deny，前端同时置灰按钮，避免重复提交。
- **预留扩展（P10 不落地）**：`ApprovalReasonDto::Advisor` 与 `ApprovalPreviewDto` 的顾问分支由 [I18 design §3.17](./I18-agent-tool-architecture-design.md) 定义；P6 只渲染 `preview` 与 L0/L1 原因，**不得**调用顾问、不得假定 `advisor` 一定存在。

### 2.3 状态事件

- `session-state-changed` 由 I36 spec Step 7 定义；前端订阅并映射到 UI phase。
- `Processing` 携带 `turnId` + `phase`；旧 `turn_id` 的迟到事件必须丢弃（前端按 `turnId` 过滤）。
- 前端**不得**维护第二套 `phase` 推导，也不得用 `ensureIdle()` 轮询。

### 2.4 提问请求（`ask_user`，P6-C8）

工具形态与 schema 见 [I38c §9](./I38c-os-tools-spec.md)，超时方案见 [I38c §11 OQ5](./I38c-os-tools-spec.md)（已定稿）。本节只定义 wire 与前端契约。

`AgentToolCallPayloadDto` 追加第二个变体 `awaitingAnswer{question: AgentQuestionRequestDto}`，与 `awaitingApproval` 并列；两者都映射到 `awaiting_user` 态，区别只在卡片形态。

| DTO | 字段 | 约束 |
|---|---|---|
| `AgentQuestionRequestDto` | `conversationId` / `turnId` / `callId` / `questions: AgentQuestionDto[]` / `timeoutMs` | `questions` 恒 1–4 条；`timeoutMs` 来自 `userQuestionTimeoutSecs`（默认 180000，`0` → 不设超时，前端不显示倒计时） |
| `AgentQuestionDto` | `question` / `header` / `options: AgentQuestionOptionDto[]` / `multiSelect` | `options` 恒 2–10 条；`header` ≤20 字符 |
| `AgentQuestionOptionDto` | `label` / `description` | 后端已校验，前端原样渲染 |
| `AgentQuestionAnswerDto` | `conversationId` / `callId` / `answers: AgentQuestionAnswerItemDto[]` | 输入 DTO，`deny_unknown_fields`；`answers` 长度必须等于 `questions` |
| `AgentQuestionAnswerItemDto` | `selected: string[]` / `other: string \| null` | 单选也走数组（恒 0–1 项）；自由输入写 `other`，不塞进 `selected` |

**不变量**：

- 「其他」选项由前端自动追加，**不进 schema、不进 `options`**（避免模型自己造 `Other` 与前端重复）；用户选它时展开文本框，内容写 `other`。
- 超时与取消都由后端收口：前端倒计时结束只置灰（与审批一致），后端超时后工具返回 `Cancelled`，turn 继续，模型看到「用户未回答」。
- 命令：`resolve_agent_question`（单入口，与 `resolve_agent_tool_approval` 对称）。
- `ask_user` 的等待**不计入** `confirmationWaitMs`（该字段只度量审批等待）；本批不新增独立计时字段。
- `awaitingAnswer` 只在实时流出现；历史消息里 `ask_user` 只按终态渲染（§3.6）。

---

## 3 前端集成

### 3.1 消息模型

```ts
// 与 wire 对齐：camelCase
interface ToolCallView {
  callId: string
  name: string
  status: 'running' | 'success' | 'error' | 'awaiting_user' | 'cancelled'
  arguments? : Record<string, unknown>
  resultText?: string | null
  startedAt?: number | null
  finishedAt?: number | null
  approval?: ToolApprovalRequestDto | null
}
```

- 流式期间：`AgentToolCallPayloadDto.phase` 映射到 `ToolCallView.status`。
  - `started` / `argsDelta` → `running`
  - `finished` → `success`
  - `failed` → `error`
  - `awaitingApproval` → `awaiting_user`
  - `awaitingAnswer` → `awaiting_user`（卡片形态见 §3.6，与审批卡片并列但不同组件）
- 取消（turn 终止且未完成）→ `cancelled`。
- **`arguments` 的来源分两条**：`running` 态由前端累积 `argsDelta` 字符串后 `JSON.parse`（增量 JSON 不完整时保留上一份可解析快照，解析失败只影响预览）；`awaitingApproval` 态**直接取 `approval.arguments`**（后端已解析的完整对象），不再解析。审批判据一律用后者。
- **不**从 `ChatDeltaPayloadDto` 读取任何工具字段（P0 已移除）。

### 3.2 ToolCallBubble 五态

```
┌ running ─────────────┐  ┌ success ─────────────┐
│ .. read             │  │ ok read  (120ms)     │
│ /src/main.ts        │  │ /src/main.ts         │
└─────────────────────┘  │ ▾ [45 lines]         │
                         └──────────────────────┘
┌ error ──────────────┐  ┌ awaiting_user ───────┐
│ err shell           │  │ ! shell              │
│ Permission denied   │  │ rm -rf /tmp/cache    │
└─────────────────────┘  │ [Approve] [Deny]     │
                         └──────────────────────┘
┌ cancelled ──────────┐
│ ⊘ delete            │
│ User rejected       │
└─────────────────────┘
```

- 审批卡片必须展示：工具名、规范化参数（命令完整 argv / 文件路径 + diff）、权限意图。
- 审批卡片动作固定四个：`[允许]`（仅本次）、`[拒绝]`、`[本会话允许]`、`[始终允许]`。后两者是授权动作，分别落 `session` / `global` 作用域（§3.3.1）；`[始终允许]` 需二次确认。卡片不提供下拉或隐藏菜单，四个动作平铺，用户无需展开即可区分作用域。
- 授权文案（`[本会话允许]`）：本会话内同类操作不再询问；`[始终允许]`：所有会话的同类操作都不再询问。两者都注明可在设置页撤销或修改作用域。
- 线格式枚举同步：`approve` / `deny` / `allowSession` / `allowAlways`，取代旧 `alwaysApprove`（语义收窄为会话级后名称自解释，避免「always」与实际作用域不符）。
- **AI 顾问启用后（P10，非 P6）**：AI 免审不得静默，卡片须标注「AI 判定只读，本次免审」并提供「撤销该免审」；AI 加码时标注「AI 标记为高风险」并展示 `rationale`。免审不写 grant，只对本次调用生效（[I18 design §3.17.6](./I18-agent-tool-architecture-design.md)）。
- 大结果默认折叠，展示行数 / 字节数与「展开」。

### 3.2.1 审批信息分级（用户判断依据）

用户批准一个操作前需要回答四个问题：**要我做什么、为什么问我、有多严重、能不能撤销**。信息按获取成本分级，**高一级不得依赖低一级不存在**；L2 有两个出口（预演 `preview` 与后端检测 `extensions`），因为二者共用同一个「仅 `Ask` 路径、2s 预算」的执行窗口：

| 级别 | 内容 | 来源 | 成本 | P6 是否必做 | 缺失时 |
|---|---|---|---|---|---|
| **L0 固有** | `context.tier`、`context.behavior`（`readOnly` / `destructive` / `openWorld`）、`tool_name` | `Tool` trait 注册期元数据 | 零 | 必做 | 不可能缺失 |
| **L1 参数推导** | `intents`（action + 规范化 resources）、路径分类（工作区内/外、符号链接、受保护目录）、`context.reasons`（命中哪条策略） | `permission_intents` + `os/path.rs` + `Policy::explain` | 零（纯函数，无 IO） | 必做 | 不可能缺失 |
| **L2 预演** | `context.preview`：一句话 `summary`（后端已格式化，如「删除 128 个条目（3.4 MB）」）+ `truncated` / `estimated` 两个不确定性标记 | `Tool::preview_for_approval` | 一次 IO，**仅 `Ask` 时执行** | 必做（可为 `null`） | 卡片只显示 L0+L1 |
| **L2 扩展** | `context.extensions[]`：后端检测结论（`kind` + `label` + `value` + `severity`） | 各检测方（路径守卫、MCP 来源等，见 I38a §2.8.1） | 零或一次 IO，与预演共用 2s 预算 | 必做（可为空数组） | 不渲染扩展区 |
| **L3 顾问** | AI 结论 + `rationale` | `ApprovalAdvisor`（[I18 design §3.17](./I18-agent-tool-architecture-design.md)） | 模型调用 | **P6 不做**（归 P10） | 不显示 |

**所有审批信息都在 `context` 一个包裹内**，顶层不再加字段。理由：这四个问题（在哪执行 / 能不能撤销 / 为什么问我 / 有多大）之外，将来还会出现新问题（P7 的 MCP 来源可信度、P10 的顾问证据），每加一个就改一次顶层 wire 并让前端按 `tool_name` 分支判断「哪些字段有意义」，成本会随工具数增长。包裹后，新增信息只动 `context` 内部，`ApprovalRequest` 与 `ToolApprovalRequestDto` 的签名稳定。

**`preview` 是一句话摘要，不是结构化量级表**：各工具量纲不同（条目数 / 字节数 / 行数 / 替换次数），固定的 `size: u64` 会迫使后端塞进一个含义模糊的数字。`summary` 由后端按工具语义格式化，前端只渲染一行、不解析。结构化量级若将来真的需要，作为 `extensions` 条目加入，**不扩 `preview`**——否则 `preview` 会退化成第二个万能字段。

**`extensions` 是「后端检测结论」的唯一开放出口**：用 `Vec<{kind, label, value, severity}>` 而非 `Map<String, Value>`，是因为 map 的值是任意 JSON，前端不认 `kind` 就只能丢弃或显示 `[object Object]`。`label` + `value` 让前端在完全不认识 `kind` 时也能原样渲染。`severity` 是封闭三档（`info` / `caution` / `danger`），前端按档位统一着色，不需要理解语义。

**L0 信息是零成本却最容易漏的**：`behavior.destructive` 已在 `Tool` trait 上，但 P2 的 `ApprovalRequest` 没携带，导致卡片无法回答「能不能撤销」。这是本节的主要修复目标。

**L2 的必要性来自一个规格缺口**：`delete` / `write` / `edit` 分册都要求「审批卡片展示量级」，但预扫描发生在 `execute()` 内——而审批在 `execute()` **之前**。因此预演必须在网关判定为 `Ask` 之后、人工审批之前单独执行，不能复用执行结果里的 `data`。

### 3.2.2 卡片布局与降级

```
┌ ! delete ──────────────────────────────── 8s ┐
│ 将删除 128 个条目（3.4 MB）            ← L2   │
│ 路径  D:\proj\tmp\cache                       │
│       └ 工作区内 · 非符号链接          ← L1   │
│ 影响  不可逆 · 破坏性                  ← L0   │
│ ⚠ 检测  目标被 3 个进程占用，删除将失败 ← L2 扩展 │
│ 原因  项目规则 *.tmp → 需审批          ← L1   │
│ ⚠ 预估基于参数，执行前可能变化         ← 标注   │
│ [允许] [拒绝] [本会话允许] [始终允许]     │
└──────────────────────────────────────────────┘
```

| 规则 | 要求 |
|---|---|
| `preview` 为 `null` | 只渲染 L0+L1；**不得**把缺失渲染成「0 个条目」——「未知」与「零」必须可区分 |
| `preview.truncated = true` | 显示「≥ N（已达扫描上限）」，不得显示精确值 |
| `preview.estimated` | 恒 `true`；卡片必须展示「预估基于参数」提示（审批在 `execute` 之前，只能按参数推算） |
| `reasons` 多条 | 全部展示（例如同时命中 `approvalLevel` 与 `projectRule`），不得只取第一条 |
| `behavior.readOnly = true` | 出现审批卡片本身即异常（只读工具不应 `Ask`）；显示「只读操作被要求确认」以暴露配置问题 |
| `behavior.openWorld = true` | 必须显示「会访问工作区外部」，与 `destructive` 分开标注 |
| `context.extensions` 含未知 `kind` | **不得丢弃**；按 `severity` 排序（`danger` → `caution` → `info`），原样显示 `label` / `value`，不解析 `value` 内部结构 |
| `context.extensions` 为空 | 不渲染扩展区（不显示「无扩展信息」），避免把「无」渲染成噪声 |
| `context.extensions[].value` | 视为**纯文本**：不得当 Markdown / HTML 渲染。P7 的 MCP 来源字段可能含外部可控文本，渲染即等于给注入开通道 |
| 参数脱敏 | 审批载荷携带原始参数（用户要据此判断），**不得**在 L0–L2 内做脱敏；脱敏只用于 L3 顾问输入（I18 design §3.17.4） |

**预演的安全约束**（后端实现约束，前端只消费）：

1. **只在 `Ask` 路径执行**：`Allow` / `Deny` 不调 `preview_for_approval`，否则等于给所有工具调用加一次 IO。
2. **失败不影响决策**：预演超时（建议 2s）或 `Err` → `preview = null`，审批照常进行。信息缺失可以，决策缺失不行。
3. **必须复用 `resolve()`**：预演与 `execute()` 用同一套路径解析，禁止「展示 A、执行 B」。
4. **不读文件全文**：`write` 只统计现有行数/字节数（与 [I38c-write](./I38c-write-spec.md) 的「不读全文」一致），`edit` 的 diff 按参数估算并标注。
5. **不写 grant**：预演是只读观察，不产生任何授权副作用。
6. **TOCTOU 声明**：预演与实际执行之间目标可能变化，**以执行返回为准**；预演数字只用于审批文案（[I38c-delete §3.3](./I38c-delete-spec.md)）。

### 3.3 审批交互流

```
RoundExecutor → ToolPipeline → Gateway: Ask
  → Policy::explain 产出 reasons（L1）
  → Tool::preview_for_approval（仅 Ask；超时/失败 → preview = null）（L2）
  → 各检测方填充 extensions（与预演共用 2s 预算；无结论则空数组）（L2 扩展）
  → 组装为单个 ApprovalContext
  → ApprovalPrompter（src-tauri 实现），载荷含 `context`（tier / behavior / reasons / preview / extensions）
  → emit agent-tool-call { phase: awaitingApproval, approval: ToolApprovalRequestDto }
  → ToolCallBubble 渲染 [允许] [拒绝] [本会话允许] [始终允许]
  → 用户点击 → resolve_agent_tool_call { decision }
  → ApprovalPrompter 收到决策 → Gateway 继续或拒绝
```

**超时**：前端倒计时结束只置灰，不自行发送 Deny；以后端超时为准（避免双决策竞态）。

**预演耗时不计入审批等待**：`preview_for_approval` 的耗时单独统计，不得混入 `confirmation_wait_ms`（后者只度量「等人」的时间）；预演超时上限 2s，超时即 `preview = null` 继续走审批。

**审批超时值**：新增配置 `AppConf.approval_timeout_secs: Option<u32>`，`None` → 120s（与 P4 装配的 `APPROVAL_TIMEOUT` 一致），`Some(0)` 不允许（审批必须有时限，超时即 `Deny`）。[I38b D12 细节](./I38b-agent-loop-spec.md)提到的「默认 5 min」作废，以本节为准。

**授权撤销入口**：设置页「已授权操作」列出 `permission_grants` 的行（`tool` + 作用域标签 + 会话归属 + 规范化 scope 摘要），提供单条撤销、**作用域修改**与全部清除。授权默认是**会话级**，可显式升级为全局（§3.3.1）；全局授权等于永久放行，因此该入口与审批卡片同期上线（P6-C4）。

### 3.3.1 授权作用域（会话级默认，可升级为全局）

授权的作用域与审批键是**两个正交维度**：`ApprovalKey` 只回答「批准了什么操作」（`tool` + 规范化 `scope`），作用域回答「这份批准对谁有效」。因此 `ApprovalKey` 不引入会话字段。

| 作用域 | 生效范围 | 写入方式 | 生命周期 |
|---|---|---|---|
| `session`（默认） | 仅 `conversationId` 相同的会话 | 卡片「本会话允许」；或设置页从 `global` 降级 | 会话删除时级联清理 |
| `global` | 所有会话 | 卡片「始终允许」（需二次确认）；或设置页把某条会话级授权升级 | 直到用户撤销 |

**命中判定**（网关每次 `Ask` 前执行）：

1. 会话级命中 = 存在 `(session, 本会话 id, key)` 行；
2. 全局命中 = 存在 `(global, key)` 行；
3. 全局优先短路，两种命中都留 `grantHit` 审计。

**存储表示**：`permission_grants` 主键仍是 `grant_key`，键字符串前置作用域前缀：

| 作用域 | `grant_key` 形态 |
|---|---|
| 全局 | `g|<ApprovalKey 的规范 JSON>` |
| 会话 | `s:<conversationId>|<ApprovalKey 的规范 JSON>` |
| 历史遗留（无前缀） | 读取时按 `global` 兼容，**不迁移、不改写** |

不用「新增 `scope` 列 + 复合主键」的原因：本项目迁移只允许 `ADD COLUMN`（`assistant-storage/src/db.rs` 注释），改主键必须重建表并搬数据，超出允许范围；前缀方案让「同键不同作用域」各自成行，多会话并行授权互不覆盖。

**对前端的约束**：`grantKey` 恒为不透明字符串（撤销时原样回传），前端**不得**解析前缀或 JSON 内部结构；作用域由后端解析后以独立字段下发（`PermissionGrantDto.scope`）。

**作用域修改（两个入口）**：

| 入口 | 动作 | 语义 |
|---|---|---|
| 卡片 | `[本会话允许]` / `[始终允许]` | 首次落授权即定作用域，不产生中间态 |
| 设置页 | 会话级行「设为全局」；全局行「改为仅某会话」 | 事后调整已有授权的作用域 |

修改规则（`set_permission_grant_scope`，单事务内 delete + insert）：

1. **升级** `session → global`：写 `g|<key>`，并删除该键**所有** `s:*|<key>` 行——全局已覆盖它们，保留只会让设置页出现同键多行。
2. **降级** `global → session`（必须指定目标会话）：删除 `g|<key>`，写 `s:<conversationId>|<key>`；其它会话立即回到需要审批。设置页必须在二次确认里明示「其它会话将重新需要审批」。
3. 目标行已存在时视为幂等成功（不报错、不重复插入）。
4. 源行不存在（已被撤销 / 已超时清理）返回 `false`，前端重新拉列表对齐，不假装成功。

`created_at` 随行迁移（保留原始授权时间），不重置为当前时间——设置页按时间排序时才不会因改作用域而跳动。

**卡片不提供降级**：审批卡片只有「更宽松」的两个授权动作，降级属于事后管理，放在设置页以免把审批决策复杂化。

### 3.4 历史消息

- 历史消息从 `tool_calls_json` 恢复为只读的 `ToolCallView`（`success/error` 终态）。
- 历史卡片不显示审批按钮；`awaiting_user` 不得出现在历史。
- `tool_calls_json` 为空 / 非法 → 不渲染卡片，不阻塞消息。

### 3.5 前端编排上移（三步）

| 步 | 动作 |
|---|---|
| 1 | 增加 `session-state-changed` 订阅，前端只读；保留 `ensureIdle()` 作为双保险 |
| 2 | 后端 `begin_turn` 返回 `Busy` 稳定后，删除 `ensureIdle()` 轮询与切会话时的 `cancelChat` 调度 |
| 3 | 删除前端自维护 `phase` 与 `handledTerminalEvents` 去重，改用 `turnId` 过滤 |

规则：切换会话**不取消**后台 turn（I36 design D15）；只有用户显式点停止才取消。

### 3.6 提问交互（P6-C8）

```
┌ 需要你确认 ─────────────────────────────┐
│ 目标目录是哪个？               [1/2]     │
│ ( ) src/    ( ) tests/    ( ) 其他…      │
│                        [提交]  剩余 2:58 │
└─────────────────────────────────────────┘
```

- 多问按 `questions` 顺序逐条展示（`header` 作标签、`question` 作正文），**全部作答后**才可提交；`multiSelect = true` 用多选框，否则单选。
- 「其他」恒由前端追加在选项末尾，选中后展开文本框，内容写 `answers[].other`，不写 `selected`。
- 超时置灰、不重复提交（与审批一致）；超时后卡片转为 `cancelled` 并标注「未回答」。
- 提问卡片只在 agent 会话渲染；历史里 `ask_user` 只显示终态（`success` 显示问答摘要，`error` / `cancelled` 显示原因）。
- 与审批卡片的区别：审批卡片有 `Always`、一次只有一个；提问卡片可以有多条、没有 `Always`。

---

## 4 Agent 模式与两级可用性

### 4.1 数据模型

| 层 | 变更 | 命名 |
|---|---|---|
| SQLite `agent_modes` | `kind` 取值改为 `chat` / `agent`；旧值**不重映射**，读取时兜底（§4.11） | snake_case |
| SQLite `agent_modes` | `ADD COLUMN tool_config_json TEXT`：模式级工具装配（新会话的初始值）；旧 `tools` / `max_tool_tier` / `approval_level` **手动删除**（§4.11） | snake_case |
| SQLite `conversations` | `ADD COLUMN tool_config_json TEXT`：会话级工具装配（布局内调整后持久化，可空；空 = 尚未装配，取模式初始值） | snake_case |
| Rust `AgentModeRow` | 新增 `tool_config_json: Option<String>`；`kind: String` 语义改为 `chat` / `agent` | snake_case |
| Rust `ConversationRow` | 新增 `tool_config_json: Option<String>` | snake_case |
| DTO `AgentModeDto` | `kind: AgentModeKindDto`（`chat` / `agent`）；新增 `toolConfig: ConversationToolConfigDto`（输出恒在，由 `tool_config_json` 解析，缺省为 `sandbox` / `none` / 空名单） | camelCase，`contract:gen` 生成 |
| DTO `AgentModeInputDto` | 同上；`toolConfig` 为输入（缺省 `sandbox` / `none` / 空名单） | camelCase，输入 DTO |
| DTO `ConversationDto` | 新增 `toolConfig: ConversationToolConfigDto \| null`（键恒在；`null` = 该会话尚未在 agent 布局装配） | camelCase |
| DTO `ConversationPatchDto` | 新增 `toolConfig: ConversationToolConfigDto \| null`（缺省 / `null` = 不改；用于布局内装配落库） | camelCase，输入 DTO |
| DTO `SendChatMessageArgsDto.toolConfig` | 保留：每轮下发的**有效**配置（agent 链路取会话持久值 ?? 模式初始值；chat 链路恒 `null`） | camelCase |

**两级存储的语义**：

- **模式级**（`agent_modes.tool_config_json`）是**初始值**：新会话首次进入 agent 布局时用它播种。
- **会话级**（`conversations.tool_config_json`）是**实际值**：布局内每次装配变更立即 `patch_conversation { toolConfig }` 落库；重启、换窗口、换设备打开同一会话都恢复。
- 会话级为 `null` 时按模式级解析；会话级一旦写入就不再回退（模式后续修改不回溯影响已有会话）。

**迁移纪律**：只允许 `ADD COLUMN`、内置行补种、按 `id` 的内置行 `kind` 归一与下线删除，以及**手动删除**已下线的旧工具三列（`tools` / `max_tool_tier` / `approval_level`，一次性、不可逆，见 §4.11）；禁止 RENAME 业务列 / 全表数据重写。旧 `kind` 取值**不做数据映射**，由读取侧兜底（§4.11）。

### 4.2 类型

```ts
// ToolTierDto / ApprovalLevelDto / ConversationToolConfigDto 由 `npm run contract:gen`
// 从 P5 的 protocol/dto 生成（I38c §3.1），前端**不手写**定义，本节只声明前端侧新增字段。
type AgentModeKind = "chat" | "agent"

interface AgentMode {
  // ...现有字段（camelCase）...
  kind: AgentModeKind
  toolConfig: ConversationToolConfigDto  // 模式级初始装配（新会话播种值）
}

interface Conversation {
  // ...现有字段（camelCase）...
  modeId: string | null
  toolConfig: ConversationToolConfigDto | null  // 会话级装配；null = 尚未装配
}
```

- `AgentMode` 顶层不再有 `tools` / `maxToolTier` / `approvalLevel`，三者收敛进 `toolConfig`（旧 DB 三列已手动删除，既不读取、也不再出现在 DTO 顶层）。
- `src/settings/agentMode/types.ts` 的手写镜像**必须与 `AgentModeDto` 同时改**，漏一处即漂移。
- `ConversationToolConfigDto` 的字段（`maxToolTier` / `approvalLevel` / `enabledTools` / `disabledTools`）以生成类型为准；**P6 起落库**（`agent_modes.tool_config_json` / `conversations.tool_config_json`），后端只存 JSON 文本，不建列。

### 4.3 内置预设

**内置模式收敛为「两个 chat / 一个 agent」**（`builtin-quick` / `builtin-deep` 下线，见 §4.11）：

| 模式 id | 名称 | 链路（`kind`） | toolConfig | 说明 |
|---|---|---|---|---|
| `builtin-default` | 默认 | `chat` | —（不生效） | 唯一内置 chat 通用模式；新建会话默认值；不渲染工具 UI |
| `builtin-translate` | 翻译助手 | `chat` | —（不生效） | 纯 persona 的 chat 模式，不参与工具装配 |
| `builtin-agent` | 智能体 | `agent` | `sandbox` / `dangerous_only` / `[]` | 唯一内置 agent 模式；工具在 `AgentLayout` 内装配 |

**deny-by-default**：白名单为空 = 无工具；新增工具不会自动对所有模式可用（I18 design §3.11）。`builtin-agent` 虽然链路是 agent，但初始 `enabledTools = []`，必须在布局内显式勾选工具后才可用。

**自定义模式（`isBuiltin = false`）保留，且支持两种形态**：创建时 `kind` 二选一（`chat` / `agent`），**创建后锁定**（编辑不可改）。

| 自定义 `kind` | 消息路径 | 布局 | 工具装配 |
|---|---|---|---|
| `chat` | `send_chat_message` + `TurnSpec::chat()` | `ChatLayout` | 无入口、`toolConfig` 不生效 |
| `agent` | `send_agent_message` + `TurnSpec::agent()` | `AgentLayout` | `ToolConfigPanel` 可装配，落库会话级（§4.4） |

锁定 `kind` 的理由：`kind` 决定已有会话的发送路径与布局，允许编辑会让同一会话在无感知的情况下换链路。需要换类型时新建模式。后端 `update_agent_mode` 收到与既有行不同的 `kind` 时返回 `error.agent_mode_kind_locked`；前端编辑态把类型控件置为只读。

**旧值（`default` / `quick` / `deep` / `custom`）退化为纯兼容标签**：库里原样保留、不重映射，**不参与分支判定**——只有 `kind = agent` 进 agent 组与 agent 链路，其余（含上述旧值）在模式列表归入 chat 组、按 chat 链路处理。`custom` 不再映射为 `agent`，其原有的工具能力在 P6 后不再生效（§4.11 / A25）。注意：P6 语境下的「自定义模式」指 `isBuiltin = false` 的行（可按需创建 `chat` 或 `agent`），与旧 `kind = 'custom'` 值无关。

### 4.4 工具配置的两级存储与初始化

**有效配置的解析顺序**（单一实现，前端 `resolveEffectiveToolConfig`）：

```
effective = conversation.toolConfig              // 会话级持久值（conversations.tool_config_json）
         ?? mode.toolConfig                        // 模式级初始值（agent_modes.tool_config_json）
         ?? { sandbox, none, [], [] }              // 兜底
```

```
mode.toolConfig = {
  maxToolTier:   mode.tool_config_json.maxToolTier,
  approvalLevel: mode.tool_config_json.approvalLevel,
  enabledTools:  [...mode.tool_config_json.enabledTools],
  disabledTools: [...mode.tool_config_json.disabledTools],
}
```

- **模式级是初始值**：`AgentMode` 的 `toolConfig` 只在新会话首次进入 agent 布局时用于播种，不在每次发送时实时读取。
- **会话级是实际值**：首次进入 agent 布局且 `conversation.toolConfig = null` 时，前端把模式级播种值通过 `patch_conversation { toolConfig }` **立即落库**；之后布局内每次装配变更同样落库。
- **重启 / 换窗口**：会话级持久值直接恢复，不需要重新播种，也不会因模式后续修改而漂移（双向下沉）。
- **会话内切换模式不销毁会话级配置**：切到 chat 时 `toolConfig` 不发送（`null`），但 `conversations.tool_config_json` 保留；切回 agent 复用，**不重新从模式播种**（§4.8）。
- 每次发送消息把**有效配置**随请求传给后端（`SendChatMessageArgsDto.toolConfig`）；后端不缓存该状态。
- 装配写入失败（`patch_conversation` 报错）时：内存态回滚到写入前的值，并提示「保存失败」，不得让 UI 显示一个实际未落库的配置。

### 4.5 resolveAvailableTools

> **后端实现已迁至 P5**（见 [I38c §3.1](./I38c-os-tools-spec.md)）。P6 只做前端调用与 UI 绑定：从会话有效配置构造 `ConversationToolConfigDto`，随 `send_agent_message` 请求传给后端；后端 `resolve_available_tools` 完成过滤并返回 `Vec<ToolSpec>`。以下为前端视角的接口契约，实际过滤逻辑以 I38c 为准。

```rust
pub async fn resolve_available_tools(
    registry: &ToolRegistry,
    enabled_tools: &[String],
    disabled_tools: &[String],
    max_tier: ToolTier,
) -> Vec<ToolSpec> {
    // 1. 层级硬上限 + 受众过滤（复用 tool_definitions_for_tier(max_tier)，返回 Vec<ToolDefinition>）
    // 2. deny-by-default：enabled_tools 为空 → 空集
    // 3. 白名单过滤
    // 4. disabled_tools 优先于 enabled_tools
    // 5. ToolDefinition → ToolSpec（返回 Vec<ToolSpec>）
}
```

空集不在此处转 `None`：`kernel/request.rs` 的 `resolve_tools` 负责把空集归一为 `None`（循环退化为单次调用）。

**不变量**：

- `config` 来自请求携带的**有效配置**（会话级持久值 ?? 模式级初始值，§4.4）。**agent 链路恒下发**，chat 链路恒 `null`（不构造）；后端对 `null` 的兜底语义是 deny-by-default（空工具集），见 [I38c §3.1](./I38c-os-tools-spec.md)。
- `maxToolTier` 是硬上限；`enabledTools` 包含更高层级工具也不生效。
- 同一工具同时出现在 `enabledTools` / `disabledTools` → 禁用。
- 工具 `name` 是稳定标识，不随 i18n 变化。
- 可用性层（LLM 能否看到）与权限层（能否执行）正交：可用性过滤发生在构建 `tools` 参数时；权限评估发生在执行 tool_call 时。

### 4.6 AgentMode → Policy 映射

```rust
fn policy_from_agent_mode(mode: &AgentModeConfig) -> ResolvedPermissionPolicy {
    let mut global_rules = default_global_rules();

    match mode.approval_level {
        ApprovalLevel::None => {
            global_rules.insert(0, PermissionRule::new("*", "*", PermissionEffect::Allow));
        }
        ApprovalLevel::DangerousOnly => {
            // 保持默认（read=Allow, edit/execute=Ask）
        }
        ApprovalLevel::AllOsTools => {
            global_rules.push(PermissionRule::new("read", "*", PermissionEffect::Ask));
        }
    }

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

**审批级别语义**：

| 级别 | 语义 |
|---|---|
| `none` | 全部自动执行（仍受 tier 与工具白名单约束） |
| `dangerous_only` | 仅 `destructive` / `open_world` 需审批 |
| `all_os_tools` | 所有非 Sandbox 工具需审批 |

### 4.7 UI

| 交互点 | 位置 | 功能 |
|---|---|---|
| AgentMode 编辑表单 | 设置页 `/settings/agents` | 名称 / 描述 / 类型（创建时）/ prompt / 模型 / 温度 / maxTokens / 推理；**不含**层级上限、审批级别、工具多选（已移出，见下） |
| 默认工具装配摘要 | 设置页模式行 | 只读展示「`sandbox` · `none` · 3 个工具」，附「在对话中调整」提示 |
| 模式切换 | 共同视图输入区工具栏（**两个布局都有**，§4.10） | `ModeSelector` 单控件：下拉按 `kind` 分组列出对话组 / Agent 组，选中项直接决定链路；turn 进行中置灰（§4.8） |
| 工具装配面板 `ToolConfigPanel` | `AgentLayout` 输入区上方（**仅 agent 链路**） | 层级上限、审批级别、按 tier 分组的工具多选；默认全不勾选；变更即 `patch_conversation { toolConfig }` 落库（§4.4） |
| 工具概览条 `ToolToolbar` | `AgentLayout` 输入区上方 | 当前会话已启用工具概览，点击打开装配面板 |
| 工具可用性变更提示 | 聊天流 | 配置变更时系统消息提示「已启用 shell」等 |

装配面板（`ToolConfigPanel`，位于布局内）：

```
┌─ 工具装配 ────────────────────────────────────────┐
│  层级上限:  [sandbox ▼]   审批级别: [dangerous_only ▼] │
│  ── Sandbox 工具 ──  ☐ json-viewer ☐ color-picker  │
│  ── OS 工具 ──       ☐ read ☐ write ...            │
│  ── External ──       (无 MCP server 时显示暂无)    │
│                              [设为该模式默认] [完成] │
└─────────────────────────────────────────────────────┘
```

- **设置页只保留 persona 配置**：层级上限 / 审批级别 / 工具多选从设置页移除，避免「模式默认值」与「会话实际值」两处编辑产生歧义。
- 「设为该模式默认」把当前装配写回 `agent_modes.tool_config_json`，只影响**未来**新建会话，不改已有会话；自定义 agent 模式与 `builtin-agent` 可用，其余内置模式隐藏该按钮。
- 层级上限切换时，下方分组自动展开 / 折叠；超出层级的已选工具显示为不可用并提示将被忽略。
- 内置模式（`isBuiltin: true`）在设置页不可修改名称 / `kind`；chat 模式（`builtin-default` / `builtin-translate`）不出现工具装配 UI 与「设为该模式默认」。

### 4.8 chat / agent 会话内切换

**两套模式集合**（同一张 `agent_modes` 表，按 `kind` 字段区分）：

| 集合 | 判定 | 可选值 | 工具能力 |
|---|---|---|---|
| chat 模式 | `kind = chat`（含旧值兼容行） | 内置「对话」+ 内置「翻译助手」+ 自定义 chat 模式 + 旧值行（`default` / `quick` / `deep` / `custom`） | **无**，`toolConfig` 不生效、不渲染工具 UI |
| agent 模式 | `kind = agent` | 内置「智能体」+ 自定义 agent 模式 | **有**，按 §4.5 解析 |

**切换是会话级、可随时往返的**（2026-11-07 定稿，取代早期「会话内锁定」）：

- 入口：共同视图（§4.10）的 `ModeSelector` 单个控件——下拉按 `kind` 分组列出全部模式（对话组 / Agent 组），选中项直接决定链路；跨组切换与组内切换是同一个动作，不另设分段切换控件。
- 持久化：`ConversationDto.modeId` 是唯一持久化的链路判据；切换即 `patch_conversation { modeId }`，会话已有的 `tool_config` 持久值不受影响（§4.4）。
- **链路按 turn 决定，不按会话决定**：每次发送时用会话当前 `modeId` 解析 `mode.kind` → 链路 → 发送命令与 `TurnSpec`（`resolveConversationRoute`，单一实现）。切换只影响**下一次发送**，不回溯已有消息，也不改变进行中的 turn。
- **默认 chat**：新建会话 `modeId = builtin-default`（`kind = chat` → `ChatLayout`）；新建时显式传入 agent 模式则按传入值。
- **两种形态的自定义模式**：`kind = chat` 的自定义模式与内置「对话」走完全相同的 chat 路径（`send_chat_message` / `TurnSpec::chat()` / `ChatLayout`），其 `toolConfig` 不生效也不渲染；`kind = agent` 的自定义模式走 agent 路径（`send_agent_message` / `TurnSpec::agent()` / `AgentLayout`）。两条路径互不复用分支，也不存在「半 chat 半 agent」的第三种形态。
- **无组内记忆**：分组只是下拉的展示层次，用户每次切换都显式选中目标模式，因此不再需要按会话记忆各组上次模式（早期 `lastChatModeId` / `lastAgentModeId` 内存态已随控件合并下线）。
- **turn 进行中禁止切换**：`session-state-changed` 的 phase 非 `idle` / `error`（含 `awaitingApproval` / `awaitingAnswer`）时切换控件置灰；后端 `patch_conversation` 在会话存在进行中 turn 时返回 `error.conversation_busy`（`TurnTasks::current` 判定），前端原样展示。
  - 理由：审批 / 提问卡片只存在于 `AgentLayout`。中途切到 chat 布局会把待审批请求藏起来，用户既看不到也无法处理，只能等超时 `Deny`——这是行为错误，不是视觉问题。
  - 双保险的必要性：多窗口同一会话时，窗口 B 的切换必须被后端拒绝，否则窗口 A 的取消会按新链路分派到错误的命令（`cancel_chat` / `cancel_agent`）。
- **取消与发送同判据**：`cancelTurn` 同样用会话当前 `modeId` 解析链路；切换在 turn 期间被两端禁止，判据不会漂移。

**后端改动**（`src-tauri/src/cmd/conversation.rs` 的 `patch_conversation`）：

| 项 | 现状 | P6 后 |
|---|---|---|
| `mode_id` 守卫 | 有消息即返回 `error.conversation_mode_locked` | 删除；改为「有进行中 turn → `error.conversation_busy`」 |
| i18n | `error.conversation_mode_locked` | 下线该键；新增 `error.conversation_busy` |
| 存在性校验 | 无 | 不做（查不到 mode 行时前端与后端解析均按 `builtin-default`（chat 链路）兜底） |
| 事件 | `ModeUpdated` | 不变（用于多窗口同步会话列表） |

**历史消息**：

- 历史渲染按**消息自身数据**（`tool_calls_json` 有无）决定是否出现工具卡片，不由当前模式决定（§3.4）。
- 因此同一会话「前半段 chat、后半段 agent」是合法状态：chat 段无工具卡片，agent 段有只读终态卡片。
- 早期「同一会话混用两套 turn 语义无法回溯解释」的顾虑，已由「逐消息按自身数据渲染 + 每 turn 的 `TurnSpec` 由发送时模式决定」覆盖，不再作为禁止切换的理由。

**命令映射**（链路差异只由 `TurnSpec` 承载，见 I36 spec §0B.2）：

| 会话链路 | 判定来源 | 发送命令 | TurnSpec |
|---|---|---|---|
| chat | 会话 `modeId` → `mode.kind ≠ agent`（含 `chat` 与旧值） | `send_chat_message` | `TurnSpec::chat()` |
| agent | 会话 `modeId` → `mode.kind = agent` | `send_agent_message` | `TurnSpec::agent()` |

**不变量**：

- **链路由 `mode.kind` 唯一决定**，前端不得根据会话内工具配置反推链路，也不得在 chat 链路上调用 `send_agent_message`。
- **判据取自会话自身**：`ConversationDto.modeId` → `agent_modes` 行 → `kind`。前端**不得**用全局选中态（`selectedModeId`）或发送请求里的 `mode_id` 作为链路判据；后者只作回显，不参与判定。
- `modeId = null`、或 `modeId` 查不到对应 mode 行时，按 `builtin-default` 处理（chat 链路）。
- chat 链路恒 `toolConfig = null`：即便该 mode 记录的 `toolConfig` 非空，也不注入（`TurnSpec::chat()` 已固定，且 chat 模式按定义无工具能力）。
- agent 链路但 `enabledTools` 为空时，仍是 agent 链路（`TurnSpec::agent()`），由后端 `resolve_tools` 把空集归一为 `None`、退化为单次调用；不因「没有工具」而改走 chat 链路。
- `ConversationToolConfig` 只对 agent 链路下发；chat 链路传 `null`，且不渲染工具 UI（§4.10）。
- 工具配置持久值跨切换保留：切到 chat 不发送、不销毁，切回 agent 复用（§4.4）。
- 工具相关 UI（`ToolToolbar` / `ToolConfigPanel` / `ToolCallBubble` / 审批 / 提问卡片）**只在 `AgentLayout` 渲染**。
- 两条链路共用同一会话状态机与 phase 语义（I36 spec Step 1）；`session-state-changed` 的消费方式一致。
- 老会话 `kind` 缺失或为旧值时的读取兜底（**不落库重映射**，§4.11）：只有 `kind = agent` 走 agent 链路，**其余一律 chat 链路**（含旧值 `default` / `quick` / `deep` / `custom`、`null`、未知值、mode 行缺失）。旧值在库里原样保留，**仅作兼容标签，不参与分支判定**，`custom` 不做任何映射。兜底只在 `AgentKind::from_mode_kind` 与 `resolveConversationRoute` 各实现一次。

### 4.9 工作区（`workspace_root`）

**定稿（2026-11-07）：P6 用全局配置，会话级绑定推迟。**

| 项 | 定稿 |
|---|---|
| 来源 | 新增设置项 `AppConf.workspace_root: Option<String>`（`None` = 未配置） |
| 选择方式 | 设置页提供文件夹选择；`tauri-plugin-dialog` 已在依赖中，不新增依赖 |
| 生效时机 | 每次工具执行读取当前配置（改设置后下一轮生效），不在启动时缓存 |
| 非法值 | 不存在 / 非绝对路径 → 按 `None` 处理，工具层报「未配置工作区」（[I38c glob / grep](./I38c-glob-spec.md) 的既有语义），**不静默退回当前目录** |
| 传递路径 | `setup.rs` 的 `PipelineToolExecution` 读配置 → `ToolUseContext.workspace_root`（[I38b §4.5](./I38b-agent-loop-spec.md)） |
| 审批一致性 | `context.extensions` 的「工作区内/外」与 `ensure_within_root` 用**同一来源**，不得出现两套口径 |
| 推迟项 | `conversations.workspace_root` 列 + 会话级选择器归 P8.2（多会话并行不同工作区才有收益） |

**理由**：P6 已含审批闭环 + 五态 + 模式配置 + 链路切换；会话级绑定要加表列、加选择器、加迁移，收益只在「同一应用内并行多个工作区」时体现。全局配置先让所有 OS 工具的相对路径与缺省 `path` 可用，且边界只有一个，审批与执行不会分叉。

### 4.10 共同视图与双布局

**形态：一个视图 + 两个布局。** `ConversationView` 是 chat 域的**唯一视图入口**（Orb 与主窗口都挂它），持有会话级状态与命令；`ChatLayout` / `AgentLayout` 是纯展示布局，只接收 props，不持有状态机、不订阅事件。

```
src/chat/
  ConversationView.tsx        // 唯一入口：状态、命令、模式切换、布局选择
  layouts/
    ChatLayout.tsx            // 对话布局
    AgentLayout.tsx           // Agent 布局
  tool/
    ToolCallBubble.tsx        // 五态气泡（§3.2）
    ApprovalCard.tsx          // 审批卡片（§3.2.2）
    QuestionCard.tsx          // 提问卡片（§3.6）
    ToolToolbar.tsx           // 会话工具概览条（§4.7）
    ToolConfigPanel.tsx       // 会话工具装配面板（§4.4 / §4.7）
```

**职责边界**：

| 层 | 负责 | 禁止 |
|---|---|---|
| `ConversationView` | 会话 / 消息 / 流状态读取；send / cancel / retry / regenerate / branch / fork / compact / 附件；模式切换；`route` 解析；布局选择；`toolConfig` 读写与落库（§4.4） | 直接渲染工具气泡、审批 / 提问卡片 |
| `ChatLayout` | 对话链路的 chrome、消息区、输入区、上下文栏组合 | 引用任何工具 / 审批 / 提问组件；构造 `toolConfig` |
| `AgentLayout` | agent 链路的 chrome、消息区（含工具气泡 / 审批 / 提问卡片）、`ToolToolbar` + `ToolConfigPanel`、输入区 | 复制 `MessageList` / `MessageInput` / 附件 / 上下文栏；自行订阅事件 |

**共享组件清单（复用，不复制）**：`MessageList`、`MessageBubble`、`MessageContent`、`MessageInput`、`AttachmentPreview`、`ContextBar`、`CompactBanner`、`ModelSelector`、`ProviderSelector`、`ReasoningSelector`、`ModeSelector`。

**布局选择**：只由 `resolveConversationRoute(modeId, modes).link` 决定（与发送同一判据）；**不得**按「消息里有无 `tool_calls`」「全局选中态」「上次渲染的布局」决定。模式切换后布局整体替换，`ConversationView` 不重建。

**布局差异**：

| 区域 | `ChatLayout` | `AgentLayout` |
|---|---|---|
| 顶部 chrome（provider / model / 新建） | 有 | 有（同组件） |
| 消息区 | `MessageList`（无工具卡片） | `MessageList` + 工具气泡 / 审批 / 提问卡片 |
| 输入区上方工具栏 | 模式切换、推理选择、压缩、附件 | 同上 + `ToolToolbar`（点击展开 `ToolConfigPanel`） |
| 输入区 | `MessageInput` | `MessageInput`（同组件） |
| 底部 | `ContextBar` | `ContextBar` |

**工具装配的布局内约束**：

- `ToolConfigPanel` 只存在于 `AgentLayout`；`ChatLayout` 无对应入口，也不提供「临时启用工具」的后门。
- 装配变更**只通过 `ConversationView` 的单一写入函数**（`updateToolConfig`）→ 内存态更新 → `patch_conversation { toolConfig }` 落库；`ToolConfigPanel` 自身不调用 Tauri 命令。
- 首次进入 agent 布局且会话级配置为 `null` 时，由 `ConversationView` 播种（模式级 → 会话级，§4.4），面板首帧即可显示有效值，不出现空态闪烁。
- turn 进行中面板可打开但控件只读（与模式切换同样理由：避免配置与实际执行不一致）；`awaitingApproval` 期间允许查看，不允许改。

**迁移与替换**：

- 现有 `ChatView` 的会话级逻辑迁入 `ConversationView`；`ChatView` 下线，`OrbPanel` 与主窗口改挂 `ConversationView`（保留 `modelOpenRef` prop）。
- 拆分是**纯搬移**，不得在搬移中改写语义（[CODE-SPLIT-PRINCIPLE](../principle/CODE-SPLIT-PRINCIPLE.md) §7）。
- `AgentLayout` 不得写成第二份 `ChatView` 拷贝：重复片段下沉为共享组件，差异用 props / slot 表达。
- 输入草稿：`MessageInput` 的本地草稿提升为 `ConversationView` 的受控值（按 `conversationId` 索引），布局替换不丢草稿（保持 I35 F38「只隐藏不卸载、草稿不丢」的不变量）。
- 布局替换只换展示层：消息数据、分页游标、分支状态、输入草稿（按 `conversationId` 存内存）均不得重置。

### 4.11 手动数据库迁移

`migrations::run_migrations()` 已全局禁用，schema 变更走 `db.rs` 的 `pragma_table_info` 存在性检测 + `ALTER TABLE ADD COLUMN`。本次包含四类：**加列（代码内幂等）**、**内置行补种（代码 `seed`，`INSERT OR IGNORE`）**、**内置行收敛（手动一次性 SQL）**、**旧工具三列删除（手动一次性 SQL，不可逆）**。

| 变更 | 方式 | 幂等 |
|---|---|---|
| `agent_modes.tool_config_json` 加列 | `db.rs` 检测 + `ALTER` | 是 |
| `conversations.tool_config_json` 加列 | `db.rs` 检测 + `ALTER` | 是 |
| 内置行 `kind` 归一（`builtin-default` / `builtin-translate` → `chat`，`builtin-agent` → `agent`） | 手动 SQL | 是（按 `id` 精确更新） |
| 下线 `builtin-quick` / `builtin-deep` | 手动 SQL | 是（`DELETE ... WHERE id IN (...)`） |
| 补种 `builtin-agent` | 代码 `seed_builtin_modes`（`INSERT OR IGNORE`） | 是 |
| 删除旧工具三列 `tools` / `max_tool_tier` / `approval_level` | 手动 SQL `DROP COLUMN` | 否（不可逆，只能靠备份还原） |

**不做数据映射**（明确排除）：

- **旧 `kind` 取值不重映射、不参与分支判定**：用户自建行的 `default` / `quick` / `deep` / `custom` 原样保留在库里，**仅作兼容标签**（`custom` 也不映射为 `agent`）。`AgentKind::from_mode_kind` 与 `resolveConversationRoute` 只认 `agent`：`kind = agent` → agent 链路，其余（旧值 / `null` / 未知）→ chat 链路。新写入一律只写 `chat` / `agent`。
- **旧工具三列手动删除**：`tools` / `max_tool_tier` / `approval_level` 直接 `DROP COLUMN`（一次性、不可逆），代码侧不再创建、不再读取、不再写入。`tool_config_json` 为 `NULL` 时按缺省 `sandbox` / `none` / `[]` / `[]`（与旧内置默认值等价，deny-by-default）。

**手动执行顺序**（应用退出后执行，先备份数据库文件）：

```sql
-- 1) 加列（db.rs 启动时也会幂等执行，此处可跳过）
ALTER TABLE agent_modes ADD COLUMN tool_config_json TEXT;
ALTER TABLE conversations ADD COLUMN tool_config_json TEXT;

-- 2) 内置行 kind 归一（只按 id 精确更新，不碰用户自建行）
UPDATE agent_modes SET kind = 'chat'  WHERE id IN ('builtin-default', 'builtin-translate');
UPDATE agent_modes SET kind = 'agent' WHERE id = 'builtin-agent';

-- 3) 指向被删模式的会话改指内置对话
UPDATE conversations SET mode_id = 'builtin-default'
WHERE mode_id IN ('builtin-quick', 'builtin-deep');

-- 4) 删除下线内置模式
DELETE FROM agent_modes WHERE id IN ('builtin-quick', 'builtin-deep');

-- 5) 删除旧工具三列（不可逆；需 SQLite >= 3.44.4 原生 DROP COLUMN）
ALTER TABLE agent_modes DROP COLUMN tools;
ALTER TABLE agent_modes DROP COLUMN max_tool_tier;
ALTER TABLE agent_modes DROP COLUMN approval_level;
```

**校验**：

```sql
SELECT id, kind, is_builtin FROM agent_modes ORDER BY presentation_rank;  -- 内置三行：default/chat、translate/chat、agent/agent
SELECT COUNT(*) FROM agent_modes WHERE id IN ('builtin-quick','builtin-deep');           -- 0
SELECT COUNT(*) FROM conversations WHERE mode_id IN ('builtin-quick','builtin-deep');    -- 0
SELECT COUNT(*) FROM agent_modes WHERE is_builtin = 0 AND kind NOT IN ('chat','agent','default','quick','deep','custom'); -- 0
SELECT COUNT(*) FROM pragma_table_info('agent_modes') WHERE name IN ('tools','max_tool_tier','approval_level'); -- 0（旧三列已删除）
```

**回滚**：迁移前的数据库备份文件即回滚点。本迁移做「加列 + 按 id 改内置行 + 删除两个内置行 + 删除旧工具三列」；用户自建行的其余字段不被改写，但 `DROP COLUMN` 不可逆，回滚只能靠还原备份。

**注意**：步骤 2 不是行为关键（旧值一律走 chat 链路，`builtin-translate` 的 `custom` 也不会进 agent 链路），但仍应执行，让内置行数据与新模型一致（`builtin-translate` 的标签由 `custom` 归一为 `chat`）。`builtin-default` 旧值 `default` 与 `chat` 同链路，归一只是标签整理。

---

## 5 实施步骤

| 批次 | 范围 |
|---|---|
| P6-C1 | DTO 契约 |
| P6-C1a | 模式模型收敛 + 手动迁移（§4.1 / §4.3 / §4.11） |
| P6-C1b | 链路选路（§4.8） |
| P6-C1c | 审批信息装配（§3.2.1） |
| P6-C1d | 共同视图 + 双布局 + 工具装配面板（§4.4 / §4.8 / §4.10） |
| P6-C2 | 前端事件接入 |
| P6-C3 | `ToolCallBubble` 五态 |
| P6-C4 | 审批 UI（§3.2.2 / §3.3） |
| P6-C5 | 前端编排上移（§3.5） |
| P6-C6 | AgentMode UI（§4.7） |
| P6-C7 | 模式到策略映射（§4.6） |
| P6-C8 | `ask_user`（§2.4 / §3.6，**独立批次，不属主体退出条件**） |

每批的具体任务、依赖与完成状态见 [I38d-agent-frontend-modes-tasklist.md](./I38d-agent-frontend-modes-tasklist.md)。本规格只定义「做什么」与「验收什么」，不记录任务级进度。

---

## 6 验收标准

| # | 标准 |
|---|---|
| A1 | 五态渲染与事件映射正确；历史消息只出现 `success/error` 终态 |
| A2 | 审批：`允许` 继续（仅本次）；`拒绝` 拒绝；`本会话允许` 仅本会话同键不再询问；`始终允许` 全局同键不再询问且需二次确认；超时自动 Deny 且前端不重复提交 |
| A3 | 多窗口同一会话 phase 一致；旧 `turnId` 的迟到事件被丢弃 |
| A4 | 删除 `ensureIdle()` 后：切会话不取消后台 turn；显式停止可取消 |
| A5 | `ChatDeltaPayloadDto` 不含任何工具字段（P0 的决定不被回退） |
| A6 | deny-by-default：`enabledTools = []` 时 `tools` 参数为空；`disabledTools` 优先 |
| A7 | `maxToolTier` 硬上限：低层级模式下 External 工具不出现在列表 |
| A8 | 设置页 AgentMode 表单**不含**层级上限 / 审批级别 / 工具多选，仅只读展示默认装配摘要；`ToolConfigPanel` 只在 `AgentLayout` 出现且默认全不勾选；内置模式名称与 `kind` 不可改 |
| A9 | `npm run contract:gen` 后 `src/types/protocol.contract.ts` 可编译；`cargo test --test assembly_contract` 通过 |
| A10 | 审批级别三档的 Policy 映射有单测；tier 规则拒绝超层级工具 |
| A11 | 链路选路与会话内切换：每次发送按会话当前 `modeId` 的 `kind` 分派（`chat` → `send_chat_message` + `TurnSpec::chat()`，`agent` → `send_agent_message` + `TurnSpec::agent()`），**自定义模式同样适用**（自定义 chat 与内置对话路径完全一致）；切换只影响下一次发送；同一 `conversationId` 空闲时可往返切换，turn 进行中前端置灰且后端返回 `conversation_busy`；chat 链路不渲染工具 UI、agent 链路渲染；历史按消息自身 `tool_calls_json` 渲染 |
| A12 | 审批载荷为**单个** `context` 包裹：`tier` / `behavior` / `reasons`（非空）/ `preview`（可为 `null`）/ `extensions`（可为空数组）；`reasons` 至少含一条 `defaultPolicy`；`ToolApprovalRequestDto` 顶层无 `tier` / `behavior` / `reasons` / `preview` 字段 |
| A12b | `extensions` 含未知 `kind` 时卡片仍渲染该条（`label` / `value` 原样显示、按 `severity` 排序）；空数组时不出现扩展区；`value` 按纯文本渲染 |
| A13 | `preview = null` 时卡片不显示任何「0 个条目」类文案；`truncated = true` 显示「≥ N」；`estimated` 提示恒显示 |
| A14 | `preview_for_approval` 仅在 `Ask` 路径被调用（`Allow` / `Deny` 路径的调用计数为 0），且超时/失败不影响审批结果 |
| A15 | `Policy::explain` 与 `evaluate` 结果一致：对同一 `(action, resource)`，`explain` 报出的获胜规则与 `evaluate` 的 effect 相符（表驱动单测） |
| A16 | 预演与 `execute` 共用同一扫描函数：`delete` 同一目标、无并发变更时，`preview.summary` 中的条目数与 `execute` 的 `data.entries` 相等（单测直接比对共用函数的返回值，不解析 `summary` 字符串） |
| A17 | 工作区（§4.9）：`workspaceRoot` 未配置时相对路径与缺省 `path` 报「未配置工作区」而非退回当前目录；配置后 `ensureWithinRoot` 与审批 `extensions` 的「工作区内/外」结论一致（同一来源，单测比对）；改配置后下一轮生效 |
| A18 | 审批超时读 `approvalTimeoutSecs`（`null` → 120s）；`Some(0)` 被拒绝；设置页「已授权操作」可列出（含作用域标签与会话归属）、撤销、**修改作用域**（会话级⇄全局，降级需指定会话）并全部清除 `permission_grants` 行；修改后同键不留重复行 |
| A19 | （P6-C8）`ask_user` 不被 30s 统一超时打断：`managesOwnExecutionTimeout() = true` 的工具在 `exec::invoke` 不套统一 deadline；`userQuestionTimeoutSecs` 生效（`null` → 180s、`0` → 不限时）；turn 取消仍立即终止等待 |
| A20 | （P6-C8）提问卡片：1–4 问逐条渲染、`multiSelect` 生效、「其他」由前端追加且回答写 `other`、全部作答才可提交、超时置灰不重复提交、历史只显示终态 |
| A21 | 视图 / 布局：`ConversationView` 为 chat 域唯一入口，`ChatView` 无残留引用；`ChatLayout` / `AgentLayout` 共用共享组件（不复制消息列表、输入区、附件、上下文栏）；布局只由 `resolveConversationRoute(...).link` 决定，不按消息内容或全局选中态 |
| A22 | 默认 chat 与模式切换：新建会话 `modeId = builtin-default`（`kind = chat` → `ChatLayout`）；`ModeSelector` 单控件下拉按 `kind` 分组列出全部模式，选中项直接决定链路、无组内记忆；`builtin-agent` 恒存在，Agent 组永不禁用 |
| A23 | 会话级工具配置持久化：布局内装配变更立即 `patch_conversation { toolConfig }` 落库；切到 chat 后 `toolConfig` 不下发但不丢；切回 agent 复用原值；重启 / 换窗口打开同一会话恢复该值，不重新从模式播种 |
| A24 | 模式模型收敛：内置模式为 `builtin-default`(chat) / `builtin-translate`(chat) / `builtin-agent`(agent)；`builtin-quick` / `builtin-deep` 不存在；自定义模式 `kind` 创建后不可改，两种形态分别走 chat / agent 路径 |
| A25 | 手动迁移（§4.11）：迁移后内置三行 `kind` 正确、无会话指向 `builtin-quick` / `builtin-deep`；**用户自建行的旧 `kind` 值不被改写、不参与分支判定**（`agent` → agent 链路，其余含 `custom` → chat 链路）；旧工具三列不被回填、`tool_config_json` 为 `NULL` 时取缺省值；`pragma_table_info` 检测 + `ALTER` 与迁移 SQL 在已迁移库上重复执行无副作用 |

---

## 7 风险与开放问题

| 风险 | 缓解 |
|---|---|
| 前端恢复工具 delta 到 ChatDelta | A5 明确禁止；工具事件独立通道 |
| 双决策竞态（前端超时 + 后端超时） | 前端只置灰，不发送 Deny；后端超时为准 |
| phase 漂移 | 后端唯一事实源；前端只订阅 |
| deny-by-default 导致「工具看不见」被误报为 bug | 设置页与对话工具栏明确显示「未启用」状态 |
| 迁移期双保险叠加 | 三步删除，每步观察一轮（I36 design §5） |
| 用户把预演数字当成承诺（预演 3 项、实际删 300 项） | `estimated` 恒 `true` 且卡片必须展示「预估基于参数」；`truncated` 显示「≥ N」；`execute` 结果覆盖卡片数字，并在结果里回显实际量级 |
| 预演拖慢审批（大目录扫描） | 2s 上限 + `DELETE_SCAN_LIMIT`；超时即 `null`，不阻塞审批 |
| 卡片信息与实际判定不一致 | `Policy::explain` 与 `evaluate` 共用 `eval_rules`（A15 表驱动单测）；预演与 `execute` 共用统计函数（A16） |
| 全局授权永久放行，用户无法回收 | 默认只授会话级，全局需卡片内显式升级 + 二次确认；设置页可事后降级为指定会话；键精确到 `(tool, 规范化 scope)`，不按前缀匹配；审计留痕；设置页「已授权操作」按作用域列出并提供单条撤销、作用域修改与全部清除（§3.3.1 / A18） |
| 用户误以为会话级授权全局生效（换会话又被问） | 卡片文案明示「本会话允许」；升级入口是并列的「始终允许」按钮（不藏在下拉里）；设置页每行带作用域标签与会话归属，并提供「设为全局」 |
| 改作用域后同键残留重复行 | `set_permission_grant_scope` 单事务 delete + insert；升级时删除该键所有会话级行；`created_at` 随行迁移（§3.3.1） |
| 全局 `workspaceRoot` 限制「多目录并行」 | 相对路径与缺省 `path` 先可用；会话级绑定登记为 P8.2（§4.9），不在 P6 内加表列 |
| 中途切换把待审批卡片藏起来 | turn 进行中禁止切换（前端置灰 + 后端 `conversation_busy`），避免审批只能等超时 `Deny`（§4.8） |
| 多窗口切换使取消判据漂移 | 后端拒绝 busy 会话的模式变更；`cancel` 与 send 共用 `resolveConversationRoute`（§4.8） |
| 双布局复制成两份 `ChatView` | 共享组件清单 + 布局差异表 + A21；拆分按 [CODE-SPLIT-PRINCIPLE](../principle/CODE-SPLIT-PRINCIPLE.md) §7 纯搬移（§4.10） |
| 两级存储漂移（模式改了、会话没变 / 会话改了、模式没变） | 语义显式：模式 = 初始值、会话 = 实际值，会话一旦写入即不再读模式（§4.4）；设置页只读摘要 + A23 |
| 布局内装配隐式污染其他会话 | 常规装配只写会话级；模式级仅由「设为该模式默认」显式写回（§4.7 / 开放问题 8） |
| `kind` 归一不可逆（内置行） | 手动迁移前强制备份；迁移 SQL 只按 `id` 精确更新内置行且幂等，带校验查询（§4.11 / A25） |
| 既有 `custom` / `deep` 模式的工具能力退化 | **已知代价、显式接受**：旧值仅作兼容标签、不决定分支，一律走 chat 链路（`custom` 不映射为 `agent`）。需要 agent 能力的用户新建 `kind = agent` 模式（§4.3 / §4.8 / §4.11） |
| 旧 `kind` 值长期留在库里 | 读取兜底是唯一入口（`from_mode_kind` / `resolveConversationRoute`），任何写入路径只产 `chat` / `agent`；用户编辑任一模式即被归一（§4.11） |

**开放问题**：

1. ~~`alwaysApprove` 的持久化范围：本会话还是跨会话？~~ **已定稿（2026-11-08 修订）：默认会话级，可显式升级为全局**。取代 2026-11-07「一律跨会话持久」的定稿。作用域与审批键正交：`ApprovalKey` 仍为 `tool + 规范化 scope`，作用域由 `permission_grants` 的 `grant_key` 前缀承载（`g|` / `s:<conversationId>|`），历史无前缀行按 `global` 兼容读取。不采用「加 `scope` 列 + 复合主键」：本项目迁移只允许 `ADD COLUMN`，改主键需重建表并搬数据。卡片按钮文案与作用域一一对应：`本会话允许` → `session`、`始终允许` → `global`（后者二次确认）；设置页可事后修改作用域（会话级「设为全局」/ 全局「改为仅某会话」），修改走单事务 `set_permission_grant_scope`；会话级授权随会话删除级联清理（§3.3.1 / A18）。
2. 工具装配的编辑权限与存储位置。**已定稿（2026-11-07 修订）：两级存储 + 布局内编辑**。装配 UI 从设置页移到 `AgentLayout` 的 `ToolConfigPanel`；模式级 `agent_modes.tool_config_json` 只作新会话初始值，会话级 `conversations.tool_config_json` 为实际值并即时落库（§4.1 / §4.4）。设置页只读展示默认装配摘要。早期「不引入对话级持久层、仅内存态」的决定作废。
3. ~~审批等待是否计入 turn 墙钟：与 P4 开放问题 2 同一决策，联调时定稿。~~ **已定稿（2026-11-07）：计入**，取 P4 D12——审批等待计入 turn 墙钟，同时单独记录 `confirmationWaitMs`（只进事件与内存汇总，不落库），与工具自身 `durationMs` 分开。
4. ~~`workspace_root` 的来源：全局配置还是会话级绑定？~~ **已定稿（2026-11-07）：P6 用全局配置 `AppConf.workspace_root`**，会话级绑定推迟到 P8.2（§4.9）。
5. ~~`ask_user` 归 P5 还是 P6，是否阻塞 P6？~~ **已定稿（2026-11-07）：归 P6，但作为 P6-C8 单独批次，不属 P6 主体退出条件**。工具实现、超时能力（`managesOwnExecutionTimeout`）与前端提问卡片同期落地，与 C1–C7 解耦验收（§2.4 / §3.6 / §5）。
6. ~~审批超时默认值：P4 代码 120s 与 I38b D12 细节「默认 5 min」冲突。~~ **已定稿（2026-11-07）：默认 120s**，由 `approvalTimeoutSecs` 配置提供（`null` → 120s，`Some(0)` 拒绝）；I38b D12 细节的 5 min 作废（§3.3）。
7. ~~会话内是否允许 chat ⇄ agent 切换？~~ **已定稿（2026-11-07）：允许**，取代早期「会话内锁定」。切换在 turn 进行中禁止（前端置灰 + 后端 `conversation_busy`）；前端形态为共同视图 + 双布局（`ConversationView` + `ChatLayout` / `AgentLayout`），默认 chat（§4.8 / §4.10）。
8. 模式级默认装配的回写边界。**已定稿（2026-11-07）：只在用户显式点「设为该模式默认」时写回 `agent_modes.tool_config_json`**，且只影响未来新建会话；布局内的常规装配变更只写会话级，不隐式修改模式。理由：隐式回写会让「在 A 会话调工具」污染「B 会话的初始配置」，且用户无感知。
9. 旧 `quick` / `deep` 模式是否需要保留为自定义模式？**已定稿（2026-11-07）：不保留**，直接删除行（其 persona 与默认值均为空壳，参数可在新建模式时自选）；引用它们的会话改指 `builtin-default`（§4.11）。

---

## 8 参考

| 要素 | 参考项目 | 参考文件 |
|---|---|---|
| 五态 ToolCard | OpenBitFun | `ToolCardStatus`、`toolInteractionModel` |
| 审批交互 | OpenBitFun | `PermissionRequestManager`、`PermissionReplySource` |
| 工具组件拆分 | openhuman | `tool-call.tsx` / `tool-error.tsx` / `tool-fallback.tsx` |
| Agent 模式暴露覆盖 | OpenBitFun | `agents.md` + `ToolExposure` override |
| 可插拔策略 | openhuman | `ToolPolicy::check()` |
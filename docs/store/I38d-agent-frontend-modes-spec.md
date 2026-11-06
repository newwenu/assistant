# I38d — P6：前端集成与 Agent 模式规格

| 项 | 值 |
|---|---|
| 状态 | 现行（P6 规格；进度见 [roadmap](./I38-agent-roadmap.md) §2） |
| 日期 | 2026-11-04 |
| 阶段 | P6（见 [I38 roadmap](./I38-agent-roadmap.md)） |
| 关联 | [I18 spec](./I18-agent-tool-architecture-spec.md) Step 9-10（已迁入本文）、[I36 spec](./I36-agent-runtime-spec.md) Step 7（事件与 DTO）、[I38a](./I38a-tool-control-layer-spec.md)（审批端口）、[I38b](./I38b-agent-loop-spec.md)（工具事件）、[I18b](./I18b-tool-unified-bridge-spec.md)（ToolBox 桥接） |
| 影响范围 | `src-tauri/src/protocol/dto`、`src/types`（契约生成）、前端 chat 域、设置页 Agent 模式、对话工具栏 |

> **重要修正**：P0 已从 `ChatDeltaPayloadDto` 删除 `delta_tool_call_id/name/arguments`。P6 **不得**把这些字段加回 chat delta；工具调用一律走独立的 `agent-tool-call` 事件。
> **命名规则**：wire DTO 与前端 interface 一律 camelCase；Rust 存储 struct 与 DB 列一律 snake_case，转换只在 DTO 层。

---

## 1 目标与非目标

### 1.1 目标

| # | 交付 |
|---|---|
| 1 | 工具调用五态渲染（running / success / error / awaiting_user / cancelled） |
| 2 | 审批交互闭环（Approve / Deny / AlwaysApprove / 超时自动 Deny） |
| 3 | 前端只订阅后端 phase，不自行推导、不轮询 idle |
| 4 | AgentMode 扩展字段（`maxToolTier` / `approvalLevel` / `tools`）与对话级 `ConversationToolConfig`（仅内存，不落库） |
| 5 | `resolveAvailableTools` deny-by-default + 层级硬上限 |
| 6 | AgentMode → PermissionPolicy 映射；项目级权限加载入口 |
| 7 | chat / agent 链路显式切换：新建会话按 `mode.kind` 选路，会话内锁定（§4.8） |

### 1.2 非目标

- 后端工具实现（P5）；MCP UI（P7）。
- 子代理、排队输入（I36 design §3.19）；子代理的会话树 / roster 投影归 P8.2。
- 重写历史消息渲染；历史 `tool_calls_json` 的展示仅按 §3.4 处理。

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

`ToolApprovalKindDto` 为 `approve` / `deny` / `alwaysApprove`（`rename_all = "camelCase"`）。

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
- `alwaysApprove` 仅在审批卡片内提供，文案说明「本会话内同一操作不再询问」。
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
│ [Approve] [Deny] [AlwaysApprove]              │
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
  → ToolCallBubble 渲染 [Approve] [Deny] [Always]
  → 用户点击 → approve_agent_tool_call / deny_agent_tool_call
  → ApprovalPrompter 收到决策 → Gateway 继续或拒绝
```

**超时**：前端倒计时结束只置灰，不自行发送 Deny；以后端超时为准（避免双决策竞态）。

**预演耗时不计入审批等待**：`preview_for_approval` 的耗时单独统计，不得混入 `confirmation_wait_ms`（后者只度量「等人」的时间）；预演超时上限 2s，超时即 `preview = null` 继续走审批。

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

---

## 4 Agent 模式与两级可用性

### 4.1 数据模型

| 层 | 变更 | 命名 |
|---|---|---|
| SQLite `agent_modes` | `ADD COLUMN max_tool_tier TEXT NOT NULL DEFAULT 'sandbox'` | snake_case |
| SQLite `agent_modes` | `ADD COLUMN approval_level TEXT NOT NULL DEFAULT 'none'` | snake_case |
| Rust `AgentModeRow` | 新增 `max_tool_tier: String` / `approval_level: String` | snake_case |
| DTO `AgentModeDto` | 新增 `maxToolTier: ToolTierDto` / `approvalLevel: ApprovalLevelDto`（`tools` 已有） | camelCase，`contract:gen` 生成 |
| 前端 `AgentMode`（`src/settings/agentMode/types.ts`） | 同步加 `maxToolTier` / `approvalLevel` | 手写镜像，**必须与 `AgentModeDto` 同时改**，漏一处即漂移 |
| DTO `SendChatMessageArgsDto` | 新增 `toolConfig: ConversationToolConfigDto \| null` | camelCase |
| DTO `ConversationDto` | 新增 `modeId: string \| null`（链路判据来源，§4.8） | camelCase |
| DTO `ConversationToolConfigDto` | `maxToolTier` / `approvalLevel` / `enabledTools` / `disabledTools` | camelCase，`contract:gen` 生成，**仅内存不落库** |

**不引入对话级持久层**：`conversations` 表**不加** `tool_config_json`，`ConversationRow` 也不加字段。对话级配置只存在于前端内存（按 `conversationId` 索引），随每次发送请求传给后端；后端不持有该状态，也不落库。

**迁移纪律**：只允许 `ADD COLUMN`，禁止 DROP / RENAME / 数据重写（与主库规则一致）。

### 4.2 类型

```ts
// ToolTierDto / ApprovalLevelDto / ConversationToolConfigDto 由 `npm run contract:gen`
// 从 P5 的 protocol/dto 生成（I38c §3.1），前端**不手写**定义，本节只声明前端侧新增字段。
interface AgentMode {
  // ...现有字段（camelCase）...
  tools: string[]          // 白名单；空数组 = 无可用工具
  maxToolTier: ToolTierDto
  approvalLevel: ApprovalLevelDto
}
```

`ConversationToolConfigDto` 的字段（`maxToolTier` / `approvalLevel` / `enabledTools` / `disabledTools`）同样以生成类型为准；它**仅存在于内存**、不落库，因此后端不建表，也不在前端额外定义一份。

### 4.3 内置预设

| 模式 | maxToolTier | approvalLevel | tools | 说明 |
|---|---|---|---|---|
| 默认 | `sandbox` | `none` | `[]` | 无可用工具，按需启用 |
| 快速 | `sandbox` | `none` | `[]` | 同上 |
| 深度 | `os` | `dangerous_only` | `[]` | 层级开放到 OS，工具仍需显式启用 |
| 翻译助手 | `sandbox` | `none` | `[]` | 同上 |

**deny-by-default**：白名单为空 = 无工具；新增工具不会自动对所有模式可用（I18 design §3.11）。

### 4.4 对话级临时配置与初始化

对话创建时（前端新建会话）从 AgentMode 初始化**内存态**：

```
conversation.toolConfig = {
  maxToolTier:   mode.maxToolTier,
  approvalLevel: mode.approvalLevel,
  enabledTools:  [...mode.tools],
  disabledTools: [],
}
```

- 该对象只存在于前端内存，按 `conversationId` 索引，**不落库**（§4.1）。
- 会话内修改只影响当前对话；AgentMode 后续修改也不回溯影响已创建对话（双向下沉）。
- **新建对话 → 重新从 AgentMode 初始化，即自动还原**，不依赖计时器（I18 design §3.11）。
- 应用重启 → 内存态丢失，重新打开的会话按 AgentMode 重新初始化。
- 每次发送消息把当前 `toolConfig` 随请求传给后端（`SendChatMessageArgsDto.toolConfig`）；后端不缓存该状态。

### 4.5 resolveAvailableTools

> **后端实现已迁至 P5**（见 [I38c §3.1](./I38c-os-tools-spec.md)）。P6 只做前端调用与 UI 绑定：从会话内存态构造 `ConversationToolConfigDto`，随 `send_agent_message` 请求传给后端；后端 `resolve_available_tools` 完成过滤并返回 `Vec<ToolSpec>`。以下为前端视角的接口契约，实际过滤逻辑以 I38c 为准。

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

- `config` 来自请求携带的会话内存态（`SendChatMessageArgsDto.toolConfig`）。**P6 前端恒下发**（新建会话即从 AgentMode 初始化），因此后端不会收到 `null`；后端对 `null` 的兜底语义是 deny-by-default（空工具集），见 [I38c §3.1](./I38c-os-tools-spec.md)。
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
| AgentMode 编辑表单 | 设置页 `/settings/agents` | 层级上限、审批级别、按 tier 分组的工具多选；默认全不勾选 |
| 对话工具栏 | 聊天输入区（**仅 agent 会话**，见 §4.8） | 当前会话可用工具概览，点击增删；变更写入**会话内存态**（不落库） |
| 工具可用性变更提示 | 聊天流 | 配置变更时系统消息提示「已启用 shell」等 |

编辑表单：

```
┌─ 工具配置 ────────────────────────────────────────┐
│  层级上限:  [sandbox ▼]   审批级别: [dangerous_only ▼] │
│  ── Sandbox 工具 ──  ☐ json-viewer ☐ color-picker  │
│  ── OS 工具 ──       ☐ read ☐ write ...            │
│  ── External ──       (无 MCP server 时显示暂无)    │
└─────────────────────────────────────────────────────┘
```

- 内置模式（`isBuiltin: true`）可查看但不可修改；用户需创建自定义模式。
- 层级上限切换时，下方分组自动展开 / 折叠。

### 4.8 chat / agent 显式切换

**两套模式集合**（同一张 `agent_modes` 表，按 `kind` 字段区分）：

| 集合 | 判定 | 可选值 | 工具能力 |
|---|---|---|---|
| chat 模式 | `kind ∈ {default, quick, deep}` | 默认 / 快速 / 深度 | **无**，`tools` 恒为 `[]` |
| agent 模式 | `kind = custom` | 用户自定义模式 | **有**，按 §4.5 解析 |

**切换是显式的，且只发生在新建会话时**：

- 入口：新建会话时选择模式；选择器按上表两组分栏展示，跨组不可混选。
- 链路一旦创建即固定，**不支持在同一会话内从 chat 切到 agent**（或反向）。需要换链路 = 新建会话，**不复用 `conversationId`**。
- 理由：同一会话内混用两套 turn 语义，会让历史消息里前半段无 `tool_calls`、后半段有，无法回溯解释；同时 `TurnSpec` 差异（`on_busy`、`max_iterations`、审批）在会话中途变更会让进行中的 turn 行为不可预期。

**命令映射**（链路差异只由 `TurnSpec` 承载，见 I36 spec §0B.2）：

| 会话链路 | 判定来源 | 发送命令 | TurnSpec |
|---|---|---|---|
| chat | 会话所属 mode 的 `kind ∈ {default, quick, deep}` | `send_chat_message` | `TurnSpec::chat()` |
| agent | 会话所属 mode 的 `kind = custom` | `send_agent_message` | `TurnSpec::agent()` |

**不变量**：

- **链路由 `mode.kind` 唯一决定**，前端不得根据会话内工具配置反推链路，也不得在 chat 会话上调用 `send_agent_message`。
- **判据取自会话自身**：`ConversationDto.modeId` → `agent_modes` 行 → `kind`。前端**不得**用全局选中态（`selectedModeId`）或发送请求的 `mode_id` 作为链路判据；否则会话中途换模式会直接换链路。
- `modeId = null`、或 `modeId` 查不到对应 mode 行时，按 `default` 处理（chat 链路）。
- chat 会话恒 `tools = None`：即便该 mode 记录的 `tools` 非空，也不注入（`TurnSpec::chat()` 已固定）。
- `ConversationToolConfig` 只对 agent 会话存在；chat 会话传 `null`，且不渲染对话工具栏。
- 工具相关 UI（对话工具栏、`ToolCallBubble`、审批面板）**只在 agent 会话渲染**。
- 两条链路共用同一会话状态机与 phase 语义（I36 spec Step 1）；`session-state-changed` 的消费方式一致。
- 老会话无 `kind` 或 `kind` 非法时按 `default` 归入 chat 链路（`AgentKind::from_mode_kind` 的兜底语义）。

---

## 5 实施步骤

| 步骤 | 内容 | 依赖 |
|---|---|---|
| P6-C1 | DTO：`AgentToolCallPayloadDto` / 审批请求与决策（含 §2.2 的单个 `ApprovalContextDto` 包裹：tier / behavior / reasons / preview / extensions）/ `AgentMode` 扩展 / `ConversationDto.modeId`（§4.8 判据）/ `ConversationToolConfig`（仅内存，随请求传递）；`npm run contract:gen` | P4 |
| P6-C1b | 链路选路（§4.8）：新建会话按 `mode.kind` 分流到 `send_chat_message` / `send_agent_message`；发送时以会话 `modeId` 判定链路（**不取全局选中态**）；会话内锁定链路；chat 会话不渲染工具 UI | P6-C1 |
| P6-C1c | 审批信息装配（§3.2.1）：`ApprovalRequest` 增加单个 `context: ApprovalContext`（tier / behavior / reasons / preview / extensions）；`Policy::explain` 产出命中规则；`Tool::preview_for_approval` 端口（默认 `None`）与 2s 超时包装；`extensions` 默认空数组，检测方按需填充；**仅在 `Ask` 路径调用** | P6-C1 |
| P6-C2 | 前端事件接入：订阅 `agent-tool-call` 与 `session-state-changed`；按 `turnId` 过滤 | P6-C1 |
| P6-C3 | `ToolCallBubble` 五态 + 大结果折叠 + 历史恢复 | P6-C2 |
| P6-C4 | 审批 UI 与命令；超时置灰；AlwaysApprove 文案；L0/L1/L2 分区渲染与 `null` / `truncated` 降级（§3.2.2）；`extensions` 按 `severity` 通用渲染（未知 `kind` 不得丢弃） | P6-C3、P6-C1c |
| P6-C5 | 删除 `ensureIdle()` / 切会话取消 / 前端 phase 自维护（三步） | P4 稳定 |
| P6-C6 | AgentMode 表单 + 对话工具栏 + `resolveAvailableTools` 接入 | P6-C1 |
| P6-C7 | `policy_from_agent_mode` + 项目级 `.assistant/permissions.json` 加载 | P2、P6-C6 |

---

## 6 验收标准

| # | 标准 |
|---|---|
| A1 | 五态渲染与事件映射正确；历史消息只出现 `success/error` 终态 |
| A2 | 审批：Approve 继续；Deny 拒绝；AlwaysApprove 本会话同键不再询问；超时自动 Deny 且前端不重复提交 |
| A3 | 多窗口同一会话 phase 一致；旧 `turnId` 的迟到事件被丢弃 |
| A4 | 删除 `ensureIdle()` 后：切会话不取消后台 turn；显式停止可取消 |
| A5 | `ChatDeltaPayloadDto` 不含任何工具字段（P0 的决定不被回退） |
| A6 | deny-by-default：`enabledTools = []` 时 `tools` 参数为空；`disabledTools` 优先 |
| A7 | `maxToolTier` 硬上限：低层级模式下 External 工具不出现在列表 |
| A8 | AgentMode 表单默认全不勾选；内置模式不可修改 |
| A9 | `npm run contract:gen` 后 `src/types/protocol.contract.ts` 可编译；`cargo test --test assembly_contract` 通过 |
| A10 | 审批级别三档的 Policy 映射有单测；tier 规则拒绝超层级工具 |
| A11 | 链路选路：`kind ∈ {default, quick, deep}` 的会话只调 `send_chat_message` 且不渲染工具 UI；`kind = custom` 的会话只调 `send_agent_message`；同一 `conversationId` 上无法切换链路 |
| A12 | 审批载荷为**单个** `context` 包裹：`tier` / `behavior` / `reasons`（非空）/ `preview`（可为 `null`）/ `extensions`（可为空数组）；`reasons` 至少含一条 `defaultPolicy`；`ToolApprovalRequestDto` 顶层无 `tier` / `behavior` / `reasons` / `preview` 字段 |
| A12b | `extensions` 含未知 `kind` 时卡片仍渲染该条（`label` / `value` 原样显示、按 `severity` 排序）；空数组时不出现扩展区；`value` 按纯文本渲染 |
| A13 | `preview = null` 时卡片不显示任何「0 个条目」类文案；`truncated = true` 显示「≥ N」；`estimated` 提示恒显示 |
| A14 | `preview_for_approval` 仅在 `Ask` 路径被调用（`Allow` / `Deny` 路径的调用计数为 0），且超时/失败不影响审批结果 |
| A15 | `Policy::explain` 与 `evaluate` 结果一致：对同一 `(action, resource)`，`explain` 报出的获胜规则与 `evaluate` 的 effect 相符（表驱动单测） |
| A16 | 预演与 `execute` 共用同一扫描函数：`delete` 同一目标、无并发变更时，`preview.summary` 中的条目数与 `execute` 的 `data.entries` 相等（单测直接比对共用函数的返回值，不解析 `summary` 字符串） |

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

**开放问题**：

1. `alwaysApprove` 的持久化范围：本会话还是跨会话？P2 当前为会话级；跨会话需要新的持久化策略，P7 前不定。
2. 对话工具栏与 AgentMode 表单的编辑权限：内置模式只读，自定义模式可改。**已定论**：不引入对话级持久层——工具栏变更只写入会话内存态，不落库、不设还原计时器；新建对话从 AgentMode 预设重新初始化，即自动还原（I18 design §3.11）。
3. 审批等待是否计入 turn 墙钟：与 P4 开放问题 2 同一决策，联调时定稿。

---

## 8 参考

| 要素 | 参考项目 | 参考文件 |
|---|---|---|
| 五态 ToolCard | OpenBitFun | `ToolCardStatus`、`toolInteractionModel` |
| 审批交互 | OpenBitFun | `PermissionRequestManager`、`PermissionReplySource` |
| 工具组件拆分 | openhuman | `tool-call.tsx` / `tool-error.tsx` / `tool-fallback.tsx` |
| Agent 模式暴露覆盖 | OpenBitFun | `agents.md` + `ToolExposure` override |
| 可插拔策略 | openhuman | `ToolPolicy::check()` |
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

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentToolCallPayloadDto {
    pub conversation_id: String,
    pub turn_id: String,
    pub iteration: u32,
    pub call_id: String,
    pub name: String,
    pub phase: AgentToolCallPhaseDto,
    pub arguments: Option<serde_json::Value>,
    pub args_delta: Option<String>,
    pub result: Option<String>,
    pub error: Option<String>,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub enum AgentToolCallPhaseDto {
    Started,
    ArgsDelta,
    Finished,
    Failed,
    AwaitingApproval,
}
```

**约束**：

- 输出 DTO 的 `Option` 字段**不加** `skip_serializing_if`：键恒在、值 `null`，前端声明 `T | null`。
- 事件名 `agent-tool-call`；`phase` 是封闭枚举，前端不得兜底窄化。
- 工具调用结果同时进入消息历史；事件只负责实时渲染，不作为事实源。

### 2.2 审批请求与命令

审批载荷复用 P2 的 `ApprovalRequest`，经 DTO 转出：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToolApprovalRequestDto {
    pub conversation_id: String,
    pub turn_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub intents: Vec<PermissionIntentDto>,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolApprovalDecisionDto {
    pub conversation_id: String,
    pub call_id: String,
    pub decision: ToolApprovalKindDto,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub enum ToolApprovalKindDto {
    Approve,
    Deny,
    AlwaysApprove,
}
```

- 命令：`approve_agent_tool_call` / `deny_agent_tool_call`（或统一 `resolve_agent_tool_approval`，只保留一个入口）。
- 前端倒计时基于 `timeoutMs`；超时后后端自动 Deny，前端同时置灰按钮，避免重复提交。

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

- 流式期间：`AgentToolCallPhaseDto` 映射到 `ToolCallView.status`。
  - `started` / `argsDelta` → `running`
  - `finished` → `success`
  - `failed` → `error`
  - `awaitingApproval` → `awaiting_user`
- 取消（turn 终止且未完成）→ `cancelled`。
- **不**从 `ChatDeltaPayloadDto` 读取任何工具字段（P0 已移除）。

### 3.2 ToolCallBubble 五态

```
┌ running ─────────────┐  ┌ success ─────────────┐
│ .. fs_read           │  │ ok fs_read  (120ms)  │
│ /src/main.ts        │  │ /src/main.ts         │
└─────────────────────┘  │ ▾ [45 lines]         │
                         └──────────────────────┘
┌ error ──────────────┐  ┌ awaiting_user ───────┐
│ err shell_exec      │  │ ! shell_exec         │
│ Permission denied   │  │ rm -rf /tmp/cache    │
└─────────────────────┘  │ [Approve] [Deny]     │
                         └──────────────────────┘
┌ cancelled ──────────┐
│ ⊘ fs_delete         │
│ User rejected       │
└─────────────────────┘
```

- 审批卡片必须展示：工具名、规范化参数（命令完整 argv / 文件路径 + diff）、权限意图。
- `alwaysApprove` 仅在审批卡片内提供，文案说明「本会话内同一操作不再询问」。
- 大结果默认折叠，展示行数 / 字节数与「展开」。

### 3.3 审批交互流

```
RoundExecutor → ToolPipeline → Gateway: Ask
  → ApprovalPrompter（src-tauri 实现）
  → emit agent-tool-call { phase: awaitingApproval, approval: ToolApprovalRequestDto }
  → ToolCallBubble 渲染 [Approve] [Deny] [Always]
  → 用户点击 → approve_agent_tool_call / deny_agent_tool_call
  → ApprovalPrompter 收到决策 → Gateway 继续或拒绝
```

**超时**：前端倒计时结束只置灰，不自行发送 Deny；以后端超时为准（避免双决策竞态）。

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
| DTO / 前端 `AgentMode` | `maxToolTier` / `approvalLevel` / `tools` | camelCase |
| DTO `SendChatMessageArgsDto` | 新增 `toolConfig: ConversationToolConfig \| null` | camelCase |
| DTO `ConversationDto` | 新增 `modeId: string \| null`（链路判据来源，§4.8） | camelCase |
| 前端 `ConversationToolConfig` | `maxToolTier` / `approvalLevel` / `enabledTools` / `disabledTools` | camelCase，**仅内存** |

**不引入对话级持久层**：`conversations` 表**不加** `tool_config_json`，`ConversationRow` 也不加字段。对话级配置只存在于前端内存（按 `conversationId` 索引），随每次发送请求传给后端；后端不持有该状态，也不落库。

**迁移纪律**：只允许 `ADD COLUMN`，禁止 DROP / RENAME / 数据重写（与主库规则一致）。

### 4.2 类型

```ts
type ToolTier = 'sandbox' | 'os' | 'external'
type ApprovalLevel = 'none' | 'dangerous_only' | 'all_os_tools'

interface AgentMode {
  // ...现有字段（camelCase）...
  tools: string[]          // 白名单；空数组 = 无可用工具
  maxToolTier: ToolTier
  approvalLevel: ApprovalLevel
}

interface ConversationToolConfig {
  maxToolTier: ToolTier
  approvalLevel: ApprovalLevel
  enabledTools: string[]
  disabledTools: string[]
}
```

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

```rust
fn resolve_available_tools(
    registry: &ToolRegistry,
    config: &ConversationToolConfig,
    max_tier: ToolTier,
) -> Vec<Arc<dyn Tool>> {
    // 1. 层级硬上限（取 AgentMode / 对话配置中的更严者）
    // 2. 只保留 audience().agent == true
    // 3. deny-by-default：enabled_tools 为空 → 空集
    // 4. 白名单过滤
    // 5. disabled_tools 优先于 enabled_tools
}
```

**不变量**：

- `config` 来自请求携带的会话内存态（`SendChatMessageArgsDto.toolConfig`）；为 `null` 时按该会话所属 AgentMode 预设解析，**不视为"无工具"**。
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
| 工具可用性变更提示 | 聊天流 | 配置变更时系统消息提示「已启用 shell_exec」等 |

编辑表单：

```
┌─ 工具配置 ────────────────────────────────────────┐
│  层级上限:  [sandbox ▼]   审批级别: [dangerous_only ▼] │
│  ── Sandbox 工具 ──  ☐ json-viewer ☐ color-picker  │
│  ── OS 工具 ──       ☐ fs_read ☐ fs_write ...      │
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
| P6-C1 | DTO：`AgentToolCallPayloadDto` / 审批请求与决策 / `AgentMode` 扩展 / `ConversationDto.modeId`（§4.8 判据）/ `ConversationToolConfig`（仅内存，随请求传递）；`npm run contract:gen` | P4 |
| P6-C1b | 链路选路（§4.8）：新建会话按 `mode.kind` 分流到 `send_chat_message` / `send_agent_message`；发送时以会话 `modeId` 判定链路（**不取全局选中态**）；会话内锁定链路；chat 会话不渲染工具 UI | P6-C1 |
| P6-C2 | 前端事件接入：订阅 `agent-tool-call` 与 `session-state-changed`；按 `turnId` 过滤 | P6-C1 |
| P6-C3 | `ToolCallBubble` 五态 + 大结果折叠 + 历史恢复 | P6-C2 |
| P6-C4 | 审批 UI 与命令；超时置灰；AlwaysApprove 文案 | P6-C3 |
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

---

## 7 风险与开放问题

| 风险 | 缓解 |
|---|---|
| 前端恢复工具 delta 到 ChatDelta | A5 明确禁止；工具事件独立通道 |
| 双决策竞态（前端超时 + 后端超时） | 前端只置灰，不发送 Deny；后端超时为准 |
| phase 漂移 | 后端唯一事实源；前端只订阅 |
| deny-by-default 导致「工具看不见」被误报为 bug | 设置页与对话工具栏明确显示「未启用」状态 |
| 迁移期双保险叠加 | 三步删除，每步观察一轮（I36 design §5） |

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
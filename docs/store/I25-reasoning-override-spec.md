# I25 — 思考深度工具栏覆盖（Agent 模式作为默认）

> 阶段号：I25（I24 之后的下一个活跃阶段；子阶段 S1 / S2 / S3）
> 状态：设计稿 v1，未实施
> 范围：`src/chat/**`、`src/stores/**`、`src/services/**`、`src/i18n/**`、`src-tauri/src/cmd/chat/**`、`src-tauri/crates/assistant-agent/**`、`src-tauri/crates/assistant-storage/**`
> 基线口径：本文所有结论以**当前仓库代码**为准，行号为写作时实测值，实施时以符号名为准
> 一句话：把「思考深度」从「只在设置页改 Agent 模式」变成「聊天工具栏可即时改，Agent 模式值作为默认」

---

## 0. 结论速览

| # | 结论 | 依据 |
|---|---|---|
| 1 | 参数链路已经打通，缺的只有「一处覆盖入口」：`AgentModeRow.reasoning_enabled/effort` → `build_chat_options` → `ChatOptions` → 各 Provider adapter | `prompt_builder.rs` 的 `build_chat_options` 是唯一合并点（见 F5） |
| 2 | 覆盖必须用 `Option<T>` 三态语义：`None` = 不覆盖（继承模式），不能拿空串或 `false` 冒充继承 | 后端 `Option<bool>` / `Option<String>` 已是这个语义（见 F3） |
| 3 | 覆盖状态属于**会话域**（`chatStore`），不属于 `agentModeStore`。后者是「默认配置」，把临时覆盖塞进去会让设置页与聊天互相污染 | `agentModeStore` 只持有 `selectedModeId`（见 F2） |
| 4 | 合并只在后端做一次，前端不重复实现合并公式，避免双源 | 前端只下发 `(enabled, effort)` 对，后端 `or` 模式默认（见 3.1） |
| 5 | `reasoning_enabled` 与 `reasoning_effort` 必须**成对下发**：只下发 effort、enabled 为 `None` 时，OpenAI 路径视为开启、Anthropic 路径视为关闭 | `openai/chat.rs` 用 `unwrap_or(true)`，`anthropic/request.rs` 用 `unwrap_or(false)`（见 F8） |
| 6 | `conversations.mode_id` 是**死列**：只写不读，`CONV_SELECT` 里没有它 | 见 F6。S2 不复用它，另开两列 |
| 7 | 设置页的布尔开关**无法表达 `null`**，一旦用户拨动过开关，模式的「跟随 Provider 默认」就永久丢失 | `BooleanField` 只有 `checked={!!value}`（见 F9） |
| 8 | `ChatView` 同时服务主窗口与 Orb 面板，控件只需加一处，但 Orb 窄布局有溢出风险 | `orb/OrbPanel.tsx` 复用 `ChatView`（见 F2） |
| 9 | 模型是否支持推理的能力判定已存在，但没暴露给前端，S3 才需要 | `catalog_reasoning()` 当前仅测试使用（见 F10） |

---

## 1. 现状事实（代码证据）

### F1 模式级已具备两项参数

| 位置 | 事实 |
|---|---|
| `src/settings/agentMode/types.ts` | `AgentMode` 含 `reasoningEnabled: boolean \| null`、`reasoningEffort: string \| null` |
| `src/settings/core/agentKind/agentModeDefinitions.ts` | 内置三模式：`builtin-default` = `null/null`、`builtin-quick` = `false/null`、`builtin-deep` = `true/"high"` |
| `src-tauri/crates/assistant-storage/src/agent_modes.rs` | `AgentModeRow` 同名字段 + `#[derive(Default)]`；种子数据与前端定义一致 |
| `src-tauri/crates/assistant-storage/src/db.rs` | `agent_modes` 表含 `reasoning_enabled INTEGER`、`reasoning_effort TEXT` |

### F2 前端只有「模式」这一个入口，且模式是全局的

| # | 事实 | 位置 |
|---|---|---|
| F2.1 | 工具栏只有 `ModeSelector` + 摘要 + 附件三件 | `src/chat/ChatView.tsx` 的 `chat-tool-bar`（L447–L478） |
| F2.2 | `ModeSelector` 只读写全局 `selectedModeId`，无 per-conversation 概念 | `src/chat/selectors/ModeSelector.tsx` |
| F2.3 | `handleModeChange` 只做「切换模式 + 可选切换模型」，不重置任何临时状态 | `src/chat/ChatView.tsx` L261–L289 |
| F2.4 | `ChatView` 同时被主窗口与 Orb 面板复用 | `src/orb/OrbPanel.tsx` L294 |
| F2.5 | 工具栏 chip 样式复用 `orb-mode-chip`（`max-width: 100px`、`height: 28px`） | `src/assets/styles/global.css` L465–L498 |

F2.5 的 `max-width: 100px` 是新增第四个 chip 的直接约束：主窗口尚可，Orb 面板（`chat-view-compact`）需要实测是否换行或挤压。

### F3 后端协议已经是 `Option` 三态

```rust
pub struct ChatOptions {
    pub reasoning_enabled: Option<bool>,
    pub reasoning_effort: Option<String>,
    // ...
}
```

位置：`src-tauri/crates/assistant-providers/src/types/config.rs`。语义即「`None` = 不干预，交给 Provider/模型默认」。

### F4 发送链路（唯一出口）

```
ChatView.handleSend                     (src/chat/ChatView.tsx L187)
  → chatOrchestrator.send()             (src/services/chatOrchestrator.ts L150)
  → deps.env.getSelectedModeId()        (L201 / L299)
  → chatService.sendChatMessage(...)    (L310)
  → Rust send_chat_message(mode_id)     (src-tauri/src/cmd/chat/mod.rs L31)
  → insert.rs: agent_modes::get_by_id() (src-tauri/src/cmd/chat/insert.rs L36)
  → build_chat_request(detail, mode…)   (src-tauri/crates/assistant-agent/src/chat_request.rs L23)
  → build_chat_options(kind, mode, …)   (src-tauri/crates/assistant-agent/src/prompt_builder.rs L89)
```

关键点：`chatOrchestrator` 是所有发送（含重新生成 / 重试 / 编辑重发）的唯一出口，覆盖参数只需在这一处取值，不需要逐调用点改造。

### F5 合并点唯一

`build_chat_options` 当前是纯透传：

```rust
reasoning_enabled: mode.reasoning_enabled,
reasoning_effort: mode.reasoning_effort.clone(),
```

`chat_request.rs` 在 `mode = None` 时回落到 `AgentModeRow::default()`，即全 `None`。所以「加一个覆盖参数 + 一次 `or`」就是全部后端逻辑。

### F6 `conversations.mode_id` 是死列

| # | 事实 | 位置 |
|---|---|---|
| F6.1 | 表结构有 `mode_id TEXT` | `src-tauri/crates/assistant-storage/src/db.rs` L37 |
| F6.2 | 只有 `create_conversation` 写入它 | `src-tauri/src/cmd/conversation.rs` L65–L68 |
| F6.3 | `ConversationRow` 与 `CONV_SELECT` 都没有它，读路径完全不感知 | `src-tauri/crates/assistant-storage/src/conversations/mod.rs` L11–L33 |
| F6.4 | 前端 `createConversation(providerId, modelId)` 不传 `modeId`，手工新建的会话该列恒为 `NULL` | `src/services/chatService.ts`；`chatOrchestrator` 仅在自动建会话时传 |

结论：**不要复用 `mode_id` 存思考覆盖**。它是「模式」语义，且当前连模式回读都没实现；把思考覆盖塞进去会让两个未完成的语义纠缠。S2 另开两列。

### F7 Provider 侧已做 effort 归一化，无需改适配层

`src-tauri/crates/assistant-providers/src/client/quirks.rs`：

| 模型族 | 归一化行为 |
|---|---|
| DeepSeek（`deepseek-reasoner`） | `low/medium → high`，仅 `low` 保留 |
| DeepSeek flash | 仅认 `low` |
| GLM | `low/medium → high`，`max/xhigh → max` |
| 通用 | `low/medium/high` 原样 |

这意味着工具栏暴露的选项应与设置页保持一致（`low/medium/high`），不要一上来暴露 `max`：用户选「中」在 GLM 上仍会变成 `high`，是既有行为而非本阶段 bug。

### F8 「只下发 effort」在 Provider 间语义不一致

| Provider 路径 | `enabled = None` 时的默认 | `effort = Some` 时 |
|---|---|---|
| `openai/chat.rs` | `unwrap_or(true)` → 视为开启 | 生效 |
| `anthropic/request.rs` | `unwrap_or(false)` → 视为关闭 | 被 `if !enabled { remove("reasoning_effort") }` 删除 |
| `gemini/request.rs`、`code_assist/request.rs` | `unwrap_or(false)` | 视具体实现 |

因此协议层必须保证 **`effort` 只与 `enabled = Some(true)` 一起出现**。这是 2.3 的不变式之一。

### F9 设置页无法表达 `null`

```tsx
<Switch checked={!!value} onChange={(checked) => onChange(checked)} />
```

位置：`src/settings/shared/renderers/SettingField.tsx` 的 `BooleanField`。

后果：`builtin-default` 的 `reasoningEnabled = null` 一旦被用户拨动，就变成 `false`/`true`，再也回不到「跟随 Provider 默认」。这不是本阶段必须修的，但决定了「继承」的语义必须由**工具栏**承担，而不能指望模式配置表达三态。

### F10 模型能力判定存在但未暴露

| 位置 | 事实 |
|---|---|
| `src-tauri/crates/assistant-providers/src/model_catalog.rs` | `catalog_reasoning(model_id)` 已实现，当前仅测试引用 |
| `src-tauri/src/protocol/dto/settings.rs` 的 `ModelConfigDto` | 无 `reasoning` 字段 |
| `src/settings/provider/types.ts` 的 `ModelConfig` | 无 `reasoning` 字段 |

### F11 i18n 键位已占用

`src/i18n/locales/zh-CN.json` 的 `chat.reasoning.*` 已被 `ReasoningBlock` 用于「展开思考 / 收起思考 / 思考中」。新增键必须另起命名空间（见 3.2.5）。

### F12 契约测试的改动面

| 位置 | 影响 |
|---|---|
| `src-tauri/crates/assistant-agent/tests/chat_runtime_spec.rs` | 直接调用 `build_chat_options`，签名变更需同步 |
| `src/services/__tests__/chatOrchestrator.test.ts` | `ChatEnvPort` 字面量对象需补新方法 |
| `src-tauri/tests/protocol_contract.rs` | 仅在 S2 改 DTO 时需补 camelCase round-trip |

---

## 2. 语义与不变式

### 2.1 优先级链

```
工具栏即时覆盖  >  （S2）会话已保存覆盖  >  Agent 模式默认  >  Provider / 模型默认
```

### 2.2 覆盖三态

| 工具栏选项 | 下发 `reasoning_enabled` | 下发 `reasoning_effort` | 含义 |
|---|---|---|---|
| 跟随模式 | `null` | `null` | 不覆盖，完全用 Agent 模式值 |
| 关闭 | `false` | `null` | 强制关闭 |
| 低 | `true` | `"low"` | 强制开启 + 低 |
| 中 | `true` | `"medium"` | 强制开启 + 中 |
| 高 | `true` | `"high"` | 强制开启 + 高 |

「跟随模式」在 UI 上应显示**当前生效值**（如「跟随模式 · 深度/高」），否则用户无法判断此刻到底开没开。

### 2.3 不变式

| # | 不变式 | 强制位置 |
|---|---|---|
| I1 | `enabled = Some(false)` ⇒ `effort = None`（后端已兜底 remove，但前端不应制造脏组合） | 前端 `resolveReasoning` |
| I2 | `effort = Some(_)` ⇒ `enabled = Some(true)` | 前端 `resolveReasoning` |
| I3 | 切换 Agent 模式时，覆盖重置为「跟随模式」 | `ChatView.handleModeChange` |
| I4 | S1 阶段切换会话时覆盖重置为「跟随模式」；S2 阶段改为从会话记录恢复 | `chatConversationSlice.switchConversation` / 新建会话 |
| I5 | 合并公式只存在于后端一处；前端不实现「覆盖 + 模式」合并，只实现「选项 → 参数对」映射 | 见 3.1 |
| I6 | `null` 不得被序列化成空字符串或 `0` | 前端 service / Rust `Option` |

I3 的理由：用户从「深度/高」切到「快速」时，若旧覆盖仍在，会表现为「模式配置没生效」，这是最常见的用户困惑。

---

## 3. 设计

### 3.1 分层与职责

| 层 | 职责 | 不做 |
|---|---|---|
| `chatStore` | 持有 `reasoningChoice`（会话域临时状态） | 不合并模式值 |
| `chatOrchestrator` | 从 env port 取 choice，映射成参数对，随每次发送下发 | 不做持久化 |
| `chatService` | 透传参数到 IPC | 不做校验 |
| `send_chat_message` | 接收 `Option` 参数并透传 | 不做合并 |
| `build_chat_options` | **唯一合并点**：`override.or(mode)` | 不关心来源 |

这样「Agent 模式作为默认」天然成立：覆盖为 `None` 时走 `or(mode)`，等价于今天的透传。

### 3.2 S1：会话内覆盖 + 工具栏控件（先落地，不动库）

#### 3.2.1 纯映射函数

新文件 `src/chat/core/reasoning.ts`：

```ts
export type ReasoningChoice = "inherit" | "off" | "low" | "medium" | "high";

export const DEFAULT_REASONING_CHOICE: ReasoningChoice = "inherit";

export interface ReasoningPayload {
  enabled: boolean | null;
  effort: string | null;
}

export function toReasoningPayload(choice: ReasoningChoice): ReasoningPayload {
  switch (choice) {
    case "inherit":
      return { enabled: null, effort: null };
    case "off":
      return { enabled: false, effort: null };
    default:
      return { enabled: true, effort: choice };
  }
}

export function effectiveReasoning(
  choice: ReasoningChoice,
  mode: { reasoningEnabled: boolean | null; reasoningEffort: string | null } | undefined,
): { enabled: boolean; effort: string | null } {
  const payload = toReasoningPayload(choice);
  const enabled = payload.enabled ?? mode?.reasoningEnabled ?? null;
  const effort = payload.enabled === false ? null : payload.effort ?? mode?.reasoningEffort ?? null;
  return { enabled: enabled ?? false, effort };
}
```

`effectiveReasoning` **只用于 UI 展示**（「跟随模式」时显示模式值），不参与发送。发送一律用 `toReasoningPayload`，保证 I5。

> `effectiveReasoning` 里 `enabled ?? false` 的兜底是展示口径，与 Provider 真实默认（OpenAI 视为 true）不同。UI 文案用「跟随模式」而不是「开启/关闭」，正是为了不承诺具体默认。

#### 3.2.2 chatStore 增加状态

`src/stores/chatStore.ts`：

```ts
reasoningChoice: ReasoningChoice;
setReasoningChoice: (c: ReasoningChoice) => void;
```

初始值 `DEFAULT_REASONING_CHOICE`。`setReasoningChoice` 内部顺带触发 S2 的持久化（若已实施），S1 阶段为空实现。

#### 3.2.3 env port 暴露读取

`src/services/chatOrchestrator.ts` 的 `ChatEnvPort` 增加：

```ts
getReasoningChoice: () => ReasoningChoice;
```

`src/stores/chatStorePorts.ts`：

- `EnvPortReadable` 增加 `reasoningChoice: ReasoningChoice`
- `createEnvPort` 增加 `getReasoningChoice: () => get().reasoningChoice`

选择读 `chatStore` 而非 `agentModeStore`，是为了让「覆盖」与「会话」同生命周期；`getSelectedModeId` 仍留在 `agentModeStore`。

#### 3.2.4 新组件 `ReasoningSelector`

新文件 `src/chat/selectors/ReasoningSelector.tsx`，形态对齐 `ModeSelector`（`variant="chip"`、`className="orb-mode-chip"` 或其通用化版本）：

| 项 | 取值 |
|---|---|
| 选项 | `inherit / off / low / medium / high` |
| 文案 | `chat.reasoning_depth.*` |
| 图标 | `BrainOutlined`（`@ant-design/icons` 已在依赖中） |
| Tooltip | 继承时显示「当前继承：{{mode}} / {{effort}}」；覆盖时显示「本次会话生效，切换模式后重置」 |
| 隐藏条件 | S3 前恒显示；S3 后按当前模型 `reasoning` 能力显示 |

样式：`src/assets/styles/global.css` 增加 `.orb-reasoning-chip`，与 `.orb-mode-chip` 同规则；建议顺手把两者用 `:is()` 合并，减少重复块（可选，不改变行为）。

#### 3.2.5 ChatView 接入

`src/chat/ChatView.tsx`：

- `chat-tool-bar` 内 `ModeSelector` 之后插入 `<ReasoningSelector />`
- `handleModeChange` 末尾追加 `chat.setReasoningChoice("inherit")`（I3）

i18n 新增命名空间（`chat.reasoning` 已被 `ReasoningBlock` 占用，见 F11）：

```json
"chat": {
  "reasoning_depth": {
    "label": "思考深度",
    "inherit": "跟随模式",
    "off": "关闭",
    "low": "低",
    "medium": "中",
    "high": "高",
    "inheriting": "当前继承：{{value}}",
    "overridden": "本次会话生效"
  }
}
```

`zh-CN.json` 与 `en-US.json` 两处都要加；`i18next.d.ts` 的键空间由 `typeof zhCN` 派生，无需手改类型。

#### 3.2.6 协议与后端合并

**前端 service**（`src/services/chatService.ts`）：

```ts
sendChatMessage(
  conversationId: string,
  content: string,
  modeId: string | null | undefined,
  branchPointMessageId: string | null | undefined,
  providerId: string,
  modelId: string,
  reasoningEnabled?: boolean | null,
  reasoningEffort?: string | null,
): Promise<void>
```

新增两个参数放在末尾并保持可选，现有 6 个实参的调用点无需改动。

**编排器**（`src/services/chatOrchestrator.ts` L299–L316）：

```ts
const reasoning = toReasoningPayload(deps.env.getReasoningChoice());
await chatService.sendChatMessage(
  convId,
  content,
  modeId,
  pendingBranchPoint,
  selectedProviderId,
  selectedModelId,
  reasoning.enabled,
  reasoning.effort,
);
```

**命令**（`src-tauri/src/cmd/chat/mod.rs`）：

```rust
#[command]
pub async fn send_chat_message(
    // 现有参数不变
    mode_id: Option<String>,
    reasoning_enabled: Option<bool>,
    reasoning_effort: Option<String>,
) -> Result<(), String>
```

前端 `reasoningEnabled` / `reasoningEffort` 对应 Rust `reasoning_enabled` / `reasoning_effort`（Tauri 默认 camelCase↔snake_case 约定，与现有 `conversationId` / `modeId` 一致）。未传即 `None`，向后兼容。

**合并**（`src-tauri/crates/assistant-agent/src/prompt_builder.rs`）：

```rust
pub struct ReasoningOverride {
    pub enabled: Option<bool>,
    pub effort: Option<String>,
}

pub fn build_chat_options(
    kind: &AgentKind,
    mode: &AgentModeRow,
    context_window_tokens: u32,
    override_: &ReasoningOverride,
) -> ChatOptions {
    let enabled = override_.enabled.or(mode.reasoning_enabled);
    let effort = if enabled == Some(false) {
        None
    } else {
        override_.effort.clone().or_else(|| mode.reasoning_effort.clone())
    };
    // ...
}
```

`chat_request.rs` 把 override 从 `send_chat_message` 一路传到 `build_chat_request` → `build_chat_options`。

> 参数名不要用 `override`（Rust 关键字），用 `override_` 或 `reasoning`。

### 3.3 S2：会话级持久化（可选）

目标：切换对话 / 重启后仍保留覆盖。

#### 3.3.1 数据库

`src-tauri/crates/assistant-storage/src/db.rs` 的 `conversations` 建表语句增加：

```sql
reasoning_enabled INTEGER,
reasoning_effort TEXT
```

存量库按现有 `pragma_table_info` 模式补 `ALTER TABLE`（`db.rs` 中 messages 加列已有范例，照抄该模式）。

#### 3.3.2 存储层

| 文件 | 改动 |
|---|---|
| `conversations/mod.rs` | `CONV_SELECT` 追加两列；`ConversationRow` 加两个 `Option` 字段；`read_conv_row` 加两个索引（`Option<i64>` → `map(|v| v != 0)`） |
| `conversations/crud.rs` | `insert` / `update` 的 SQL 与 params 同步 |
| `conversations/ops.rs` | 两处 `ConversationRow { .. }` 构造点补字段 |
| `src/cmd/conversation.rs` | `ConversationRow { .. }` 构造点补字段 |
| `src-tauri/tests/protocol_contract.rs` | `sample_conversation_row()` 补字段 |
| `assistant-storage/tests/common/mod.rs` | `conv()` helper 补字段 |
| `assistant-agent/tests/chat_runtime_spec.rs` | 内联 `ConversationRow { .. }` 补字段 |

`ConversationRow` 没有 `Default`，`..Default::default()` 不可用，因此**所有构造点必须逐一补齐**（实测 7 处）。这是 S2 的主要成本，也是把 S2 独立成阶段的原因。

#### 3.3.3 DTO 与前端类型

- `src-tauri/src/protocol/dto/chat.rs` 的 `ConversationDto` 增加 `reasoning_enabled` / `reasoning_effort`，写法对齐现有 `active_branch_id`：

  ```rust
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub reasoning_enabled: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub reasoning_effort: Option<String>,
  ```

- 同文件的 `impl From<ConversationRow> for ConversationDto` 同步补两行
- `src/chat/core/chatTypes.ts` 的 `Conversation` 同步
- `ConversationDto` 带 `deny_unknown_fields`（实测），前端构造对象时不可多传字段

#### 3.3.4 独立命令而非 `patch_conversation`

```rust
#[command]
pub fn set_conversation_reasoning(
    app: AppHandle,
    id: String,
    reasoning_enabled: Option<bool>,
    reasoning_effort: Option<String>,
) -> Result<(), String>
```

理由：`Option<Option<T>>` 无法用普通 `Option` 表达「把已有覆盖清空」。独立命令的三个状态天然可表达：

| 入参 | 语义 |
|---|---|
| 两参都 `None` | 清空覆盖 → 回到「跟随模式」 |
| `enabled = Some(false)` | 关闭 |
| `enabled = Some(true)` + `effort = Some(_)` | 开启 + 强度 |

若并入 `patch_conversation`，前端「不改该字段」与「清空该字段」会退化成同一个 `null`，必须再引入哨兵值，违背 I6。

#### 3.3.5 前端恢复与写入

- `chatConversationSlice.switchConversation`：读取 detail 后按两字段反推 `reasoningChoice`（`enabled=false → off`、`enabled=true → effort`、两者皆空 → `inherit`）
- `setReasoningChoice`：先 `set` 本地状态，再 `void chatService.setConversationReasoning(convId, enabled, effort)`；失败仅打日志，不阻塞发送
- 发送时**仍显式下发参数**，不依赖刚写入的库值，避免 IPC 异步竞态导致「本次不生效」

### 3.4 S3（可选）：按模型能力显隐

1. `ModelConfigDto` / `ModelConfig` 增加 `reasoning: bool`（`#[serde(default)]`，兼容存量 `models_json`）
2. `list_providers` 反序列化存量时保留默认 `false`；`list_models_cmd` / `preview_models_cmd` 用 `catalog_reasoning(&m.id).unwrap_or(false)` 填充
3. `ChatView` 按当前模型决定 `ReasoningSelector` 的显隐或禁用

注意：`catalog_reasoning` 只覆盖目录内模型；自定义模型名会返回 `None`。策略建议 **`None` 时显示控件**（宁可让用户选，也不要误判隐藏），仅明确 `false` 时隐藏。

---

## 4. 影响面清单

### S1

| 文件 | 改动 | 风险 |
|---|---|---|
| `src/chat/core/reasoning.ts` | 新建：类型 + 两个纯函数 | 低 |
| `src/chat/selectors/ReasoningSelector.tsx` | 新建：chip 选择器 | 低 |
| `src/stores/chatStore.ts` | 加 2 个字段 | 低 |
| `src/stores/chatStorePorts.ts` | `EnvPortReadable` + `createEnvPort` 各加一项 | 低 |
| `src/services/chatOrchestrator.ts` | `ChatEnvPort` 加方法；`send` 映射参数 | 中（唯一出口，需跑测试） |
| `src/services/chatService.ts` | `sendChatMessage` 加 2 个可选参 | 低 |
| `src/chat/ChatView.tsx` | 插入控件 + 模式切换重置 | 低 |
| `src/assets/styles/global.css` | `.orb-reasoning-chip` | 低（Orb 窄布局需实测） |
| `src/i18n/locales/{zh-CN,en-US}.json` | 新增 `chat.reasoning_depth` | 低 |
| `src-tauri/src/cmd/chat/mod.rs` | 命令加 2 个 `Option` 参 | 低 |
| `src-tauri/crates/assistant-agent/src/prompt_builder.rs` | 新增 `ReasoningOverride` + 合并 | 中（纯函数，易测） |
| `src-tauri/crates/assistant-agent/src/chat_request.rs` | 透传 override | 低 |
| `src/services/__tests__/chatOrchestrator.test.ts` | env 字面量补方法 | 低 |
| `src-tauri/crates/assistant-agent/tests/chat_runtime_spec.rs` | 调用点补参数 | 低 |

### S2

| 文件 | 改动 | 风险 |
|---|---|---|
| `db.rs` | 建表 + 存量迁移 | **高**（迁移必须幂等） |
| `conversations/{mod,crud,ops}.rs` | SELECT / Row / 写路径 | 中（索引错位易错） |
| `cmd/conversation.rs` | 新命令 + 构造点 | 中 |
| `protocol/dto/chat.rs` | DTO 字段 | 低 |
| `chatTypes.ts` | 前端类型 | 低 |
| `chatConversationSlice.ts` | 恢复逻辑 | 中 |
| 4 处测试 helper | 补字段 | 低 |

---

## 5. 验收标准

### S1

| # | 验收 | 判定 |
|---|---|---|
| A1 | 默认 `inherit` 时，请求参数与实施前**逐字段相同** | 抓包 / 单测断言 `ChatOptions` 全等 |
| A2 | 选「关闭」后，请求不含 `reasoning_effort`，且 `thinking.type = disabled`（OpenAI 兼容路径） | 单测 |
| A3 | 选「高」后，请求 `reasoning_effort = "high"`、`thinking.type = enabled` | 单测 |
| A4 | 模式为 `deep`（`true/high`）、覆盖为 `inherit` → 等价 `true/high` | 单测 |
| A5 | 模式为 `deep`、覆盖为「关闭」→ 等价 `false/None`（覆盖优先） | 单测 |
| A6 | 从「深度」切到「快速」后，覆盖自动回到 `inherit` | 手测 + 组件测试 |
| A7 | 重新生成 / 重试 / 编辑重发 均带当前覆盖 | 手测（覆盖参数在编排器唯一出口） |
| A8 | Orb 面板工具栏不溢出、不换行错位 | 手测（窄宽度） |
| A9 | `npm run lint` / `npm run typecheck` / `npm run test` 通过 | CI |
| A10 | `cargo test -p assistant-agent` 通过 | CI |

### S2

| # | 验收 | 判定 |
|---|---|---|
| B1 | 存量库启动自动加列，无数据丢失 | 用旧库副本启动 |
| B2 | 设「高」→ 切走 → 切回，控件仍为「高」 | 手测 |
| B3 | 设「跟随模式」→ 切走 → 切回，控件为「跟随模式」且两列为 `NULL` | 手测 + SQL 查询 |
| B4 | 重启应用后覆盖仍在 | 手测 |
| B5 | `ConversationDto` camelCase round-trip | `protocol_contract.rs` |

---

## 6. 测试计划

| 层 | 用例 |
|---|---|
| 前端纯函数 | `toReasoningPayload` 五个取值；`effectiveReasoning` 在 `inherit` + 各模式组合下的展示值 |
| 前端编排 | `send` 在 `inherit` 时不改变既有调用形态（参数为 `null`）；在 `high` 时传 `true/"high"` |
| 后端纯函数 | `build_chat_options`：override 全 `None` = 现状；`Some(false)` 清空 effort；`Some(true)+Some("low")` 覆盖 `high`；`effort` 单独存在时被 `enabled=false` 吞掉 |
| 契约 | S2 时补 `ConversationDto` round-trip；确认 `send_chat_message` 不传新参仍可用 |

---

## 7. 风险与取舍

| # | 风险 / 取舍 | 处理 |
|---|---|---|
| R1 | 覆盖状态放 `agentModeStore` 会污染设置页 | 明确放 `chatStore`（见 3.2.2） |
| R2 | 前端与后端各算一次合并 → 双源漂移 | 前端只做「选项 → 参数对」，合并仅后端（I5） |
| R3 | 只下发 effort 造成 Provider 语义分裂 | 成对下发（I2 / F8） |
| R4 | `enabled=false` 但仍带 effort | 前端 `toReasoningPayload` 与后端 `enabled == Some(false) ⇒ None` 双重兜底 |
| R5 | GLM/DeepSeek 归一化让「中」看起来像「高」 | 工具栏只暴露 `low/medium/high`，与设置页一致；不在 UI 承诺精确强度 |
| R6 | S1 切换会话覆盖丢失，用户以为坏了 | Tooltip 明示「本次会话生效」；S2 落地后改为持久化 |
| R7 | Orb 窄布局第四个 chip 挤压 | 复用 `orb-mode-chip` 的 `max-width: 100px`；实测后可把 label 缩写（「跟随」） |
| R8 | S2 迁移非幂等 → 重复加列报错 | 沿用 `pragma_table_info` 判存在再 `ALTER` 的既有模式 |
| R9 | `ConversationRow` 无 `Default`，漏改编译期即报错 | 这是优点：编译器强制覆盖全部构造点 |

---

## 8. 本阶段不做

| # | 不做项 | 理由 |
|---|---|---|
| N1 | 修设置页布尔开关无法表达 `null`（F9） | 属模式配置三态化，独立议题；工具栏已能表达继承 |
| N2 | `mode_id` 回读 / 按会话恢复模式（F6） | 与思考覆盖无关，但同属「会话级参数」，建议作为后续阶段一并处理 |
| N3 | 暴露 `max` / `xhigh` 等扩展档位 | 与 Provider 归一化冲突（F7），先保持三档 |
| N4 | 覆盖影响历史消息或触发重新生成 | 覆盖只作用于「下一次请求」，不改已生成内容 |
| N5 | 把思考深度纳入压缩 / 摘要等内部调用 | 内部调用走独立路径，不读会话覆盖 |
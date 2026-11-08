# I25 — 思考深度工具栏覆盖（Agent 模式作为默认）

> 阶段号：I25（I24 之后的下一个活跃阶段；子阶段 S1 / S2 / S3）
> 状态：S1、S3 已实施并验证（lint / typecheck / vitest / cargo test 全通过）；S3.5 档位单通道重构已落地；**A6–A8、C3–C7 手测项已于 2026-11-01 真机实测通过**；S2（会话级落盘）**明确不做**。唯一残留为组件级测试（见 §9）
> 范围：`src/chat/**`、`src/stores/**`、`src/services/**`、`src/i18n/**`、`src-tauri/src/cmd/chat/**`、`src-tauri/crates/assistant-agent/**`、`src-tauri/crates/assistant-storage/**`
> 基线口径：本文所有结论以**当前仓库代码**为准，行号为写作时实测值，实施时以符号名为准
> 一句话：把「思考深度」从「只在设置页改 Agent 模式」变成「聊天工具栏可即时改，Agent 模式值作为默认」

---

## 0. 结论速览

| # | 结论 | 依据 |
|---|---|---|
| 1 | 参数链路已打通，覆盖入口收敛到一处：`AgentModeRow.reasoning_enabled/effort` 由前端先算成档位 → `reasoning_level` → `resolve_reasoning_level()` → `ChatOptions` → 各 Provider adapter | `resolveReasoningChoice()` 决定档位，`chat_options.rs` 的 `resolve_reasoning_level()` 是唯一折算点（见 F5） |
| 2 | 覆盖在**协议层只传一个档位字符串** `reasoning_level`，`default` 是前后端共认的内置档位，不是「缺省」的替身。后端只认档位值，不认 `inherit` | `ReasoningOverride { level }` + `resolve_reasoning_level()`（见 F5、3.2.6） |
| 3 | 覆盖状态属于**会话域**（`chatStore`），不属于 `agentModeStore`。后者是「默认配置」，把临时覆盖塞进去会让设置页与聊天互相污染 | `agentModeStore` 只持有 `selectedModeId`（见 F2） |
| 4 | `inherit` 只存在于前端初始态，是内部「未覆盖」标记，**不占下拉选项位**；下发前必被 `resolveReasoningChoice()` 解析成具体档位 | `reasoning.ts` + `chatStorePorts.ts`（见 3.2.1、3.2.3） |
| 5 | 后端由档位值**单向推导** `(enabled, effort)` 对：`default → (None, None)`、`off → (Some(false), None)`、`on → (Some(true), None)`、其余 → `(Some(true), Some(v))`。不存在「只下发 effort」的脏组合 | `resolve_reasoning_level()` 是唯一推导点（见 3.2.6、F8） |
| 6 | `conversations.mode_id` 是**死列**：只写不读，`CONV_SELECT` 里没有它。本阶段不复用，也不新增任何会话级列 | 见 F6。S2 明确不做 |
| 7 | 设置页的布尔开关**无法表达 `null`**，一旦用户拨动过开关，模式的「跟随 Provider 默认」就永久丢失 | `BooleanField` 只有 `checked={!!value}`（见 F9） |
| 8 | `ChatView` 同时服务主窗口与 Orb 面板，控件只需加一处，但 Orb 窄布局有溢出风险 | `orb/OrbPanel.tsx` 复用 `ChatView`（见 F2） |
| 9 | 模型是否支持推理的能力判定已存在，并已通过 `reasoningLevels` 暴露给前端，控件据此裁剪选项 | `catalog_reasoning_levels()` + `ModelConfigDto.reasoningLevels`（见 F10、F13） |
| 10 | 仓库内置的 models.dev 快照**已经带 per-model 的 `reasoning_options`**（toggle / effort 取值表 / budget_tokens），这是档位的权威来源，不该硬编码 | 快照实测 1426 处 `toggle`、608 处 `budget_tokens`、4475 处 `effort`（见 F13） |
| 11 | 档位链路已闭环：`catalog_reasoning_levels()` → `ModelConfigDto.reasoningLevels` → `ReasoningSelector` 按模型生成选项，固定五档已替换 | `model_catalog.rs` + `settings.rs` / `provider.rs` + `ReasoningSelector.tsx`（见 F13、3.4） |
| 12 | 送往前端的 DTO 改动必须四处同批：`ModelConfigDto` 带 `#[serde(default)]`、两处构造点填充、前端类型同步、契约测试补字段。只改前端会让 `deny_unknown_fields` 反序列化失败，`unwrap_or_default()` 把模型列表吞成空数组 | `settings.rs` DTO 定义 + `provider.rs` 存量读取（见 3.4.5、R10） |
| 13 | 档位覆盖**不做会话级落盘**：切换会话即回到「模式推导值」。这是刻意的取舍——覆盖是「这一次对话想调深一点」的临时意图，持久化会与模式配置形成双源 | 见 3.3 |

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

F2.5 的 `max-width: 100px` 是新增第四个 chip 的直接约束：主窗口尚可，Orb 面板（`chat-view-compact`）已手测确认不换行、不挤压（2026-11-01，对应 A8）。

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
  → build_chat_request(detail, mode…)   (src-tauri/crates/assistant-agent/src/chat_request.rs)
  → build_chat_options(kind, mode, …)   (src-tauri/crates/assistant-agent/src/chat_options.rs)
```

关键点：`chatOrchestrator` 是所有发送（含重新生成 / 重试 / 编辑重发）的唯一出口，覆盖参数只需在这一处取值，不需要逐调用点改造。

### F5 档位解析点唯一，后端不再合并模式值

`build_chat_options` 原本是纯透传（`mode.reasoning_enabled` / `mode.reasoning_effort` 直接赋给 `ChatOptions`），S1 时期曾在此处加过 `override.or(mode)` 的合并。S3.5 重构后，这一层**合并被移除**：模式值由前端 `resolveReasoningChoice()` 先算成档位，后端只做「档位 → `(enabled, effort)`」的纯解析。

```rust
pub struct ReasoningOverride {
    pub level: Option<String>,   // 前端下发的档位值，None 等价于 default
}
```

`chat_request.rs` 在 `mode = None` 时仍回落到 `AgentModeRow::default()`（供 temperature / max_tokens 使用），但推理参数不再读它。这样「用户显式选了 `default`」与「用户没选、跟随模式」在后端看来是同一个输入，区分责任完全前置到前端——这是 2.2 里 `inherit` 只在初始态存在的前提。

### F6 `conversations.mode_id` 是死列

| # | 事实 | 位置 |
|---|---|---|
| F6.1 | 表结构有 `mode_id TEXT` | `src-tauri/crates/assistant-storage/src/db.rs` L37 |
| F6.2 | 只有 `create_conversation` 写入它 | `src-tauri/src/cmd/conversation.rs` L65–L68 |
| F6.3 | `ConversationRow` 与 `CONV_SELECT` 都没有它，读路径完全不感知 | `src-tauri/crates/assistant-storage/src/conversations/mod.rs` L11–L33 |
| F6.4 | 前端 `createConversation(providerId, modelId)` 不传 `modeId`，手工新建的会话该列恒为 `NULL` | `src/services/chatService.ts`；`chatOrchestrator` 仅在自动建会话时传 |

结论：**不复用 `mode_id` 存思考覆盖**。它是「模式」语义，且当前连模式回读都没实现；把思考覆盖塞进去会让两个未完成的语义纠缠。本阶段不新增任何会话级列（S2 明确不做）。

### F7 Provider 侧已做 effort 归一化，无需改适配层

`src-tauri/crates/assistant-providers/src/client/quirks.rs`：

| 模型族 | 归一化行为 |
|---|---|
| DeepSeek `deepseek-v4-flash` | `low → low`、`high → high`、`max → max`、`medium → high` |
| DeepSeek `deepseek-v4-pro` | `low/medium → high`、`high → high`、`max/xhigh → max` |
| GLM `glm-5.2` / `glm-5-2` / `glm-5p2` | `low/medium/high → high`、`max/xhigh → max` |
| 通用（OpenAI / Gemini / Anthropic） | `low/medium/high` 原样；`xhigh` / `max` / `minimal` / `none` 一律丢弃 |

各函数另有 `"" | "none" | "minimal" => None` 的兜底，即「关闭」不会被当成一个档位传下去。

这说明两件事：

1. 固定暴露 `low/medium/high` 三档是当前唯一能跨全部 Provider 生效的集合；
2. 一旦把 `xhigh` / `max` 也摆出来，在通用路径会被**静默丢弃**（用户以为选了最高、实际什么都没发）。因此档位不能全局固定，必须按模型能力生成——这正是 F13 与 3.4 解决的问题，S3.4 已落地。

### F8 「只下发 effort」在 Provider 间语义不一致

| Provider 路径 | `enabled = None` 时的默认 | `effort = Some` 时 |
|---|---|---|
| `openai/chat.rs` | `unwrap_or(true)` → 视为开启 | 生效 |
| `anthropic/request.rs` | `unwrap_or(false)` → 视为关闭 | 被 `if !enabled { remove("reasoning_effort") }` 删除 |
| `gemini/request.rs`、`code_assist/request.rs` | `unwrap_or(false)` | 视具体实现 |

因此协议层必须保证 **`effort` 只与 `enabled = Some(true)` 一起出现**。S3.5 之后这条由后端 `resolve_reasoning_level()` 单向推导保证：档位值是唯一输入，`(enabled, effort)` 是它的函数值，前端无法制造脏组合。

### F9 设置页无法表达 `null`

```tsx
<Switch checked={!!value} onChange={(checked) => onChange(checked)} />
```

位置：`src/settings/shared/renderers/SettingField.tsx` 的 `BooleanField`。

后果：`builtin-default` 的 `reasoningEnabled = null` 一旦被用户拨动，就变成 `false`/`true`，再也回不到「跟随 Provider 默认」。这不是本阶段必须修的，但决定了「继承」的语义必须由**工具栏**承担，而不能指望模式配置表达三态。

### F10 模型能力判定已暴露给前端

| 位置 | 事实 |
|---|---|
| `src-tauri/crates/assistant-providers/src/model_catalog.rs` | `catalog_reasoning(model_id)` 判定是否支持推理；`catalog_reasoning_levels(model_id)` 给出档位集合 |
| `src-tauri/src/protocol/dto/settings.rs` 的 `ModelConfigDto` | 含 `reasoning_levels: Option<ReasoningLevelsDto>` |
| `src/settings/provider/types.ts` 的 `ModelConfig` | 含 `reasoningLevels?: { toggle: boolean; efforts: string[] }` |

### F11 i18n 键位已占用

`src/i18n/locales/zh-CN.json` 的 `chat.reasoning.*` 已被 `ReasoningBlock` 用于「展开思考 / 收起思考 / 思考中」。新增键必须另起命名空间（见 3.2.5）。

### F12 契约测试的改动面

| 位置 | 影响 |
|---|---|
| `src-tauri/crates/assistant-agent/tests/chat_runtime_spec.rs` | 直接调用 `build_chat_options`，`ReasoningOverride` 字段变更已同步（6 条推理用例） |
| `src/services/__tests__/chatOrchestrator.test.ts` | `ChatEnvPort` 字面量对象已补 `getReasoningChoice` |
| `src-tauri/tests/protocol_contract.rs` | 本阶段不动 `ConversationDto`，无需改动（S2 不做） |

### F13 快照已带档位事实，解析层已补齐（S3.1 / S3.2 完成）

`src-tauri/crates/assistant-providers/assets/models-dev.json` 的每个模型节点同时含 `reasoning: bool` 与 `reasoning_options`：

```json
"reasoning_options": [
  { "type": "effort", "values": ["low", "medium", "high", "xhigh", "max"] }
]
```

```json
"reasoning_options": [
  { "type": "toggle" },
  { "type": "budget_tokens", "min": 1024, "max": 63999 }
]
```

快照里三种 action 的出现次数（实测）：`toggle` 1426、`budget_tokens` 608、`effort` 4475；另有 1153 处 `"reasoning_options": []`，表示**权威声明该模型没有可选档位**。

S1 时期 `ModelFacts` 只有 `reasoning: bool`，档位信息在解析阶段就被丢弃——这是当时只能固定三档的根因。现已补上：

| 新增 | 内容 |
|---|---|
| `ReasoningLevels { toggle, efforts }` | `toggle` 表示「有开关」（目录给了 `toggle` action，或 effort 值里出现 `none`/`off`）；`efforts` 保留目录顺序 |
| `KNOWN_EFFORTS` | `minimal / low / medium / high / xhigh / max`，其余值丢弃（与我们 `ChatOptions.effort` 能表达的范围对齐） |
| `parse_reasoning_levels()` | 字段缺失 → `None`；`[]` → `None`；只声明 `budget_tokens` → `None`（我们协议无该通道） |
| `ModelFacts.reasoning_levels` | 新字段，由 `MODEL_CATALOG` 初始化时填充 |
| `catalog_reasoning_levels()` | 读取函数，供上层投影使用 |

`None` 与 `Some` 的区分是有意的：`None` 统一表示「目录没有我们能用的信息」，上层按「未知模型」走保守集合；`Some` 才是权威声明。因此 `[]` 与「只有 budget_tokens」都被折成 `None`，而不是伪造一个空档位集合。

实测样本（已固化为单测）：

| 模型 | 目录声明 | 解析结果 |
|---|---|---|
| `claude-fable-5` | `effort` | `toggle=false`，`efforts=[low,medium,high,xhigh,max]` |
| `claude-haiku-5-5` | `toggle` + `effort` | `toggle=true`，`efforts=[low,medium,high,xhigh,max]` |
| `glm-4.5` | `toggle` | `toggle=true`，`efforts=[]` |
| `qwen3-next-80b-a3b-thinking` | 仅 `budget_tokens` | `None` |
| 目录外模型 | — | `None` |

两步收尾已完成（S3.3 / S3.4）：`ModelConfigDto.reasoningLevels` 把档位送到前端，`ReasoningSelector` 用它替代固定五档。

参考实现 `references/1-ai-agent/OpenBitFun/src/crates/adapters/ai-adapters/src/models_dev.rs` 的做法值得对齐：

| 层 | 做法 |
|---|---|
| 解析 | `reasoning_options` 三种 action 全部保留；`None` = 字段缺失，`Some([])` = 明确无档位 |
| 投影 | 按 `models_dev` → `adapter_fallback` → `generic` 三层生成档位描述符，每层都产出「档位 id + 请求动作」 |
| 兜底 | `adapter_fallback` 的模型表刻意保守，只列已测模型；未知模型走 generic 的 `toggle + low/medium/high` |
| UI | 档位来自投影结果，`off/on/minimal/low/medium/high/xhigh/max` 都只是语义 key，由 i18n 出文案；文案缺失时回落原值 |

我们的 `ChatOptions` 只有 `enabled` + `effort` 两个自由度，没有 `budget_tokens` 通道。所以对齐时 `budget_tokens` 只能**折算**成档位（或先忽略），不能原样下发——这是与我们现有协议的真实差距，不要照搬。

---

## 2. 语义与不变式

### 2.1 优先级链

推理参数只有一个输入：**档位值**。它的来源分两段，前端负责合并，后端只负责折算：

```
已覆盖  → 用户在下拉里选的档位（default / off / on / 各 effort）
未覆盖  → 前端 resolveReasoningChoice() 用当前 Agent 模式推导：
            ├ 模式有声明 → 模式推导出的档位
            └ 模式也没声明 → default
default → 后端解析为 (None, None)，即交给 Provider / 模型默认
```

因此「Agent 模式作为默认」体现为：未覆盖时显示值与下发值都来自模式；用户一旦选了具体档位，模式值即被完全接管（包括选 `default` —— 那是显式的「不要模式值，用 Provider 默认」）。

### 2.2 档位语义表

| 档位值 | 下发 `(enabled, effort)` | 含义 | 是否占选项位 |
|---|---|---|---|
| `inherit` | —（下发前必被解析） | 内部「未覆盖」标记，只在初始态存在 | 否 |
| `default` | `(None, None)` | 不指定强度，交给 Provider / 模型默认 | 是（恒在首位） |
| `off` | `(Some(false), None)` | 强制关闭 | 是（模型声明 toggle 时） |
| `on` | `(Some(true), None)` | 开启但不指定强度 | 是（模型只声明 toggle 时） |
| `low` / `medium` / `high` / `xhigh` / `max` | `(Some(true), Some(该值))` | 开启 + 指定强度 | 是（模型目录声明时） |

`inherit` 与 `default` 的区别是本阶段的关键：前者是「我没有意见」，后者是「我明确要求忽略模式」。`inherit` 不会出现在协议层——它会先被模式值填充（可能变成 `high`），只有模式也没声明时才落到 `default` 并映射为 `(None, None)`；后端不认识 `inherit`，也不为它做特殊兜底。

### 2.3 不变式

| # | 不变式 | 强制位置 |
|---|---|---|
| I1 | `enabled = Some(false)` ⇒ `effort = None` | 后端 `resolve_reasoning_level()` 的 `off` 分支 |
| I2 | `effort = Some(_)` ⇒ `enabled = Some(true)` | 同上，`effort` 分支恒返回 `Some(true)` |
| I3 | 切换 Agent 模式时，覆盖重置为 `inherit`（重新跟随新模式） | `ChatView.handleModeChange` |
| I4 | 切换 / 新建会话时，覆盖重置为 `inherit`；**不做会话级落盘** | `chatConversationSlice` |
| I5 | 模式值 → 档位的解析只在前端一处（`resolveReasoningChoice`）；后端不再 `or` 模式默认 | 见 3.1、F5 |
| I6 | `null` 不得被序列化成空字符串或 `0` | 前端 service / Rust `Option` |
| I7 | 后端不认识 `inherit`，也不做特殊兜底：`inherit` 若漏到后端会按「其余」分支当作 effort 下发 | 由前端 `toReasoningLevel()` 保证不下发（见 3.2.6 第 3 条） |

I1 / I2 由「档位值单向推导」结构性保证，而不是靠调用方自觉：`(enabled, effort)` 是档位值的函数值，不存在中间态。

I3 的理由：用户从「深度/高」切到「快速」时，若旧覆盖仍在，会表现为「模式配置没生效」，这是最常见的用户困惑。

I4 的理由见 3.3：覆盖是「这一次对话想调深一点」的临时意图，落盘会与模式配置形成双源。

---

## 3. 设计

### 3.1 分层与职责

| 层 | 职责 | 不做 |
|---|---|---|
| `chatStore` | 持有 `reasoningChoice`（可能是 `inherit`，会话域临时状态） | 不认识模式值 |
| `chatStorePorts.getReasoningChoice` | **唯一合并点**：`resolveReasoningChoice(choice, 当前模式)` → 具体档位 | 不落盘 |
| `chatOrchestrator` | 取档位值，经 `toReasoningLevel()` 兜底后随每次发送下发 | 不解析模式 |
| `chatService` | 透传 `reasoningLevel` 到 IPC | 不做校验 |
| `send_chat_message` | 接收 `Option<String>` 并构造 `ReasoningOverride` | 不做合并 |
| `build_chat_options` | 调 `resolve_reasoning_level()`，档位 → `(enabled, effort)` | 不读模式值 |

关键点：合并（模式 → 档位）只在前端一处，折算（档位 → Provider 参数）只在后端一处，两段互不重叠。这样「Agent 模式作为默认」在 UI 与请求两条路径上都不会漂移——它们消费的是同一个 `resolveReasoningChoice` 结果。

### 3.2 S1：会话内覆盖 + 工具栏控件（先落地，不动库）

#### 3.2.1 纯映射函数

新文件 `src/chat/core/reasoning.ts`（已落地，S3.5 重构后为以下实际内容）：

```ts
export const REASONING_EFFORTS = ["low", "medium", "high"] as const;

export const REASONING_DEFAULT = "default";   // 内置档位，前后端共认
export const REASONING_INHERIT = "inherit";   // 内部「未覆盖」态，不占选项位
export const REASONING_OFF = "off";
export const REASONING_ON = "on";

export type ReasoningChoice = string;         // 取值来自目录投影，无法穷举

export const DEFAULT_REASONING_CHOICE: ReasoningChoice = REASONING_INHERIT;

export function reasoningChoices(levels?: ReasoningLevelsLike): readonly ReasoningChoice[] {
  // levels 缺失 → 保守集合 default + off + low/medium/high
  // toggle + efforts → default + off + 每个 effort
  // toggle 无 efforts → default + off + on
  // 无 toggle 有 efforts → default + 每个 effort
  // 两者皆无 → 仅 default
}

export function modeReasoningChoice(mode?: ReasoningModeDefaults): ReasoningChoice | null {
  if (!mode) return null;
  if (mode.reasoningEnabled === false) return REASONING_OFF;
  if (mode.reasoningEffort) return mode.reasoningEffort;
  if (mode.reasoningEnabled === true) return REASONING_ON;
  return null;
}

export function resolveReasoningChoice(
  choice: ReasoningChoice,
  mode?: ReasoningModeDefaults,
): ReasoningChoice {
  if (choice !== REASONING_INHERIT) return choice;
  return modeReasoningChoice(mode) ?? REASONING_DEFAULT;
}

export function toReasoningLevel(choice: ReasoningChoice): ReasoningChoice {
  return choice === REASONING_INHERIT ? REASONING_DEFAULT : choice;
}
```

三个函数的分工是刻意拆开的：

| 函数 | 用途 | 是否可能返回 `inherit` |
|---|---|---|
| `modeReasoningChoice` | 模式配置 → 档位；两者都没声明时返回 `null` | 否 |
| `resolveReasoningChoice` | **显示值 + 下发值**统一收口：覆盖优先，否则回落模式，最后兜底 `default` | 否 |
| `toReasoningLevel` | 发送前兜底，把残留的 `inherit` 折成 `default` | 否 |

与 S1 初稿的差异（都是实施时修正的）：

1. 删掉了 `toReasoningPayload` 与 `effectiveReasoning`。前者把档位映射成 `(enabled, effort)` 对，后者再做一次模式回落——这导致「合并」在前端出现两套实现，且 `effectiveReasoning` 的返回值仍可空，UI 还得再判一次。现在合并只在 `resolveReasoningChoice` 一处，折算完全交给后端。
2. `REASONING_CHOICES` 从「固定五档」降级为「未知模型的保守集合」，实际选项由 `reasoningChoices()` 按模型投影组合。
3. `inherit` 从 `REASONING_CHOICES` 移除。它仍是一个合法常量，但不再作为可选项——用户无法「选跟随模式」，只能通过切换模式 / 切会话自动回到该状态。
4. `resolveReasoningChoice` 永不返回 `null`：UI 与请求都不需要再处理空值分支，`default` 承担了这个兜底角色。

`reasoningChoices()` 的五种组合（已单测）：

| `levels` | 选项 |
|---|---|
| `undefined`（未知模型） | `default` + `off` + `low/medium/high` |
| `toggle=true, efforts=[low,high,xhigh]` | `default` + `off` + `low/high/xhigh` |
| `toggle=true, efforts=[]` | `default` + `off` + `on` |
| `toggle=false, efforts=[low,medium]` | `default` + `low/medium` |
| `toggle=false, efforts=[]` | 仅 `default` |

`default` 恒在首位且永不缺席：即便模型声明「没有任何可选档位」，用户仍能显式要求「用 Provider 默认」，这与「模型不支持推理」是两件事。

#### 3.2.2 chatStore 增加状态

`src/stores/chatStore.ts`：

```ts
reasoningChoice: ReasoningChoice;
setReasoningChoice: (c: ReasoningChoice) => void;
```

初始值 `DEFAULT_REASONING_CHOICE`（即 `inherit`）。`setReasoningChoice` 只改本地状态，**不触发任何持久化**——覆盖不落盘是本阶段的明确取舍（见 3.3）。

#### 3.2.3 env port 暴露读取

`src/services/chatOrchestrator.ts` 的 `ChatEnvPort` 增加：

```ts
getReasoningChoice: () => ReasoningChoice;
```

`src/stores/chatStorePorts.ts`：

- `EnvPortReadable` 增加 `reasoningChoice: ReasoningChoice`
- `createEnvPort` 的 `getReasoningChoice` 做解析，而不是直接返回原始 choice：

```ts
getReasoningChoice: () => {
  const { modes, selectedModeId } = useAgentModeStore.getState();
  return resolveReasoningChoice(
    get().reasoningChoice,
    modes.find((m) => m.id === selectedModeId),
  );
},
```

把 `resolveReasoningChoice` 放在 port 而不是组件里，是为了让**显示与下发共用同一个结果**：控件读它、编排器也读它，不可能出现「界面显示高、实际发的是别的」。

选择读 `chatStore` 而非 `agentModeStore`，是为了让「覆盖」与「会话」同生命周期；`getSelectedModeId` 与模式列表仍来自 `agentModeStore`。

#### 3.2.4 新组件 `ReasoningSelector`

新文件 `src/chat/selectors/ReasoningSelector.tsx`，形态对齐 `ModeSelector`（`variant="chip"` + 独立 `className`）：

| 项 | 实际取值 |
|---|---|
| 选项 | 由 `reasoningChoices(当前模型的 reasoningLevels)` 生成；`default` 恒在首位，`inherit` 不出现 |
| 文案 | `chat.reasoning_depth.*` |
| 图标 | 无。chip 只显示当前档位文本，靠 Tooltip 补充语义（与 `ModeSelector` 一致，避免四个 chip 挤在 Orb 窄栏） |
| 选中值显示 | `resolveReasoningChoice()` 的结果，即**当前实际生效档位**。未覆盖时显示模式推导值（如「高」），模式也没声明时显示「默认」 |
| Tooltip | 未覆盖 → `from_mode`（「当前来自模式：{{value}}」）；已覆盖 → `overridden`（「本次会话已覆盖」） |
| 样式类 | `.orb-reasoning-chip`，与 `.orb-mode-chip` 并列复用同一组规则 |
| 关闭菜单 | 监听 `orb-close-menus` 事件，与 `ModelSelector` / `ProviderSelector` 一致 |
| 隐藏条件 | 恒显示。未知模型走保守集合，隐藏反而让用户以为不支持 |

两处实施时补的防御：

**① 越界回落。** `resolveReasoningChoice` 可能返回一个当前模型集合里不存在的值（模式设了 `max`，但当前模型目录只到 `high`）。此时控件显示回落到 `default`，而不是让 Antd 把裸字符串当 label 显示：

```ts
const resolved = resolveReasoningChoice(reasoningChoice, mode);
const displayChoice = choices.includes(resolved) ? resolved : REASONING_DEFAULT;
```

这是「模式配置与模型能力错配」的唯一处理点。不擅自降级到最接近的档位——显示 `default` 是最诚实的表达，真正发不发得出去由后端与 Provider 归一化收口。

**② 残留值重置。** 用户选了 `xhigh`，随后切到一个只到 `high` 的模型：

```ts
useEffect(() => {
  if (reasoningChoice !== REASONING_INHERIT && !choices.includes(reasoningChoice)) {
    setReasoningChoice(REASONING_INHERIT);
  }
}, [choices, reasoningChoice, setReasoningChoice]);
```

重置为 `inherit` 而不是 `default`，因为用户此时并未主动表达「要 Provider 默认」，只是原来的选择失效了；状态回到「未覆盖」，显示随之回落到新模式推导出的档位。

没有 `allowClear`、没有 `placeholder`：显示值恒为具体档位，不存在「空」状态。用户想回到模式推导值，只能通过切换模式或切会话（I3 / I4）。

样式：`src/assets/styles/global.css` 的 `.orb-reasoning-chip` 与 `.orb-mode-chip` 用**并列选择器**复用同一组规则（`max-width: 100px`、`height: 28px`、`border-radius: 999px`），没有改用 `:is()`——并列写法与文件里已有的 `.orb-model-chip` / `.orb-provider-chip` 块保持一致，可读性更好。

#### 3.2.5 ChatView 接入

`src/chat/ChatView.tsx`：

- `chat-tool-bar` 内 `ModeSelector` 之后插入 `<ReasoningSelector />`
- `handleModeChange` 末尾追加 `chat.setReasoningChoice("inherit")`（I3）

i18n 新增命名空间（`chat.reasoning` 已被 `ReasoningBlock` 占用，见 F11）：

```json
"chat": {
  "reasoning_depth": {
    "label": "思考深度",
    "default": "默认",
    "off": "关闭",
    "on": "开启",
    "low": "低",
    "medium": "中",
    "high": "高",
    "minimal": "极低",
    "xhigh": "极高",
    "max": "最高",
    "from_mode": "当前来自模式：{{value}}",
    "overridden": "本次会话已覆盖"
  }
}
```

`zh-CN.json` 与 `en-US.json` 两处都要加；`i18next.d.ts` 的键空间由 `typeof zhCN` 派生，无需手改类型。

相对 S1 初稿的键位变化：删掉了 `inherit`（「跟随模式」不再是选项）、`inheriting`（改为 `from_mode`）、`provider_default`（`default` 档位本身就是它，不需要第二个说法）。`minimal` / `xhigh` / `max` 是 S3 放开按模型档位时补的，用于 `EFFORT_LABEL_KEYS` 查表；查不到的 effort 值回落原字符串。

#### 3.2.6 协议与后端解析

**前端 service**（`src/services/chatService.ts`）：

```ts
sendChatMessage(
  conversationId: string,
  content: string,
  modeId: string | null | undefined,
  branchPointMessageId: string | null | undefined,
  providerId: string,
  modelId: string,
  reasoningLevel: string | null = null,
): Promise<void>
```

只加一个参数，放在末尾并带默认值，现有 6 个实参的调用点无需改动。

**编排器**（`src/services/chatOrchestrator.ts`）：

```ts
const reasoningLevel = toReasoningLevel(deps.env.getReasoningChoice());
await chatService.sendChatMessage(
  convId,
  content,
  modeId,
  pendingBranchPoint,
  selectedProviderId,
  selectedModelId,
  reasoningLevel,
);
```

`deps.env.getReasoningChoice()` 返回的已是解析后的档位（见 3.2.3），`toReasoningLevel` 只是防 `inherit` 漏网的兜底，正常路径下是恒等变换。

**命令**（`src-tauri/src/cmd/chat/mod.rs`）：

```rust
#[command]
pub async fn send_chat_message(
    // 现有参数不变
    mode_id: Option<String>,
    reasoning_level: Option<String>,
) -> Result<(), String>
```

前端 `reasoningLevel` 对应 Rust `reasoning_level`（Tauri 默认 camelCase↔snake_case 约定，与现有 `conversationId` / `modeId` 一致）。未传即 `None`，等价于 `default`。

**解析**（`src-tauri/crates/assistant-agent/src/chat_options.rs`）：

```rust
pub const REASONING_LEVEL_DEFAULT: &str = "default";
pub const REASONING_LEVEL_OFF: &str = "off";
pub const REASONING_LEVEL_ON: &str = "on";

#[derive(Debug, Clone, Default)]
pub struct ReasoningOverride {
    pub level: Option<String>,
}

fn resolve_reasoning_level(level: Option<&str>) -> (Option<bool>, Option<String>) {
    match level {
        None | Some(REASONING_LEVEL_DEFAULT) => (None, None),
        Some(REASONING_LEVEL_OFF) => (Some(false), None),
        Some(REASONING_LEVEL_ON) => (Some(true), None),
        Some(effort) => (Some(true), Some(effort.to_string())),
    }
}
```

`chat_request.rs` 把 override 从 `send_chat_message` 一路传到 `build_chat_request` → `build_chat_options`。

三个实施决定：

1. 参数名用 `reasoning`，不用 `override_`——`override` 是 Rust 关键字，加下划线的写法虽然合法但读起来别扭，而 `reasoning` 在这个函数里没有歧义。
2. `resolve_reasoning_level` 是**纯函数且无模式依赖**，所以 I1 / I2 不靠调用方自觉：`off` 分支结构上就不可能返回 effort，`effort` 分支结构上就不可能返回 `Some(false)`。
3. 未知档位值（包括 `inherit` 这种不该出现的字符串）走最后一个分支，即「当作开启 + 该 effort」；如果将来发现残留 `inherit` 被当成 effort 下发，`toReasoningLevel` 的兜底就是第一道防线。

`ReasoningOverride` 派生了 `Default`，所以 `chat_runtime_spec.rs` 里的调用点可以传 `&ReasoningOverride::default()`，等价于「档位缺失」，即 `default` 语义。

**与 S1 旧实现的差异（这是本轮的核心变更）**：旧版是 `reasoning.enabled.or(mode.reasoning_enabled)` 的双参合并，问题在于 `(None, None)` 同时承载了「跟随模式」和「Provider 默认」两种语义，后端无法区分。新版把模式回退整个删掉，模式值改由前端先算成档位——协议层只剩一种输入，一种语义。

### 3.3 S2：会话级持久化（明确不做）

结论：**不落盘**。`reasoningChoice` 只活在 `chatStore` 内存里，切换会话 / 重启应用即回到 `inherit`，显示上等价于「模式推导值」（模式也没声明时为 `default`）。

四条理由：

1. 覆盖是「这一次对话想调深一点」的临时意图，不是会话的稳定属性。用户调整思考深度通常是为了解决眼前这个问题，而不是给整个会话定一个人设。
2. 落盘会形成双源：模式配置已经能表达「这个 Agent 该用什么深度」，再给会话存一份，两个值冲突时谁优先需要额外规则，而这个规则用户看不见。
3. 成本远高于收益：`ConversationRow` 无 `Default`，实测 7 处构造点必须逐一补齐；`db.rs` 要加幂等迁移；`ConversationDto` 带 `deny_unknown_fields`，DTO 改动必须四处同批（与 R10 同类风险）；还要新增独立命令处理「清空覆盖」的三态，因为 `Option<Option<T>>` 表达不了。
4. 不落盘时用户仍有两个稳定表达渠道：模式配置（持久、跨会话）与工具栏（临时、本会话）。职责边界清晰，不需要额外解释。

若将来重启这个议题，以下要点已提前记录，避免重新踩坑：

| # | 注意点 |
|---|---|
| P1 | 存储用**单列 `reasoning_level TEXT`**，不要拆成 `enabled` + `effort` 两列。`"default"` 本身就能表达「显式用 Provider 默认」，`NULL` 表达「未覆盖」，三态一列搞定，与 3.2.6 的单通道协议天然对齐 |
| P2 | 不复用 `conversations.mode_id`（F6）：它是模式语义的死列，回读都还没实现 |
| P3 | 清空覆盖需要独立命令，不能并入 `patch_conversation`——`Option<Option<T>>` 表达不了「把已有值清空」，会退化成哨兵值，违背 I6 |
| P4 | `ConversationRow` 无 `Default`，7 处构造点必须逐一补齐；编译器会强制报错，不会漏 |
| P5 | `ConversationDto` 带 `deny_unknown_fields`，DTO / 构造点 / 前端类型 / 契约测试必须同批 |
| P6 | 发送时仍显式下发参数，不依赖刚写入的库值，避免 IPC 异步竞态导致「本次不生效」 |
| P7 | 恢复时反推：`"default" → default`、`"off" → off`、其余值 → 该值、`NULL → inherit` |

### 3.4 S3（可选）：按模型提供档位

S3 的目标从「显隐控件」升级为「档位跟随模型」。理由见 F13：快照里每个模型的 `reasoning_options` 已经写明它到底支持 `toggle`、哪些 `effort` 值、还是 `budget_tokens` 区间。解析层已补齐（S3.1/S3.2），剩余是把结果送到前端并替换固定五档。固定三档会带来两个问题——支持 `xhigh`/`max` 的模型拿不到高档，目录只声明 `toggle` 的模型却被摆出强度档位。

#### 3.4.1 分层落地

| 步骤 | 改动 | 状态 |
|---|---|---|
| S3.1 | `ModelFacts` 增加 `reasoning_levels: Option<ReasoningLevels>`，解析 `reasoning_options` | **已完成**：`toggle` + `efforts` 两字段；`None` 表示「目录没有可用信息」 |
| S3.2 | 读取函数 `catalog_reasoning_levels()` | **已完成** |
| S3.3 | `ModelConfigDto` / `ModelConfig` 增加 `reasoningLevels` 投影结果 | **已完成**：DTO / 两处构造 / 前端类型 / 契约测试同批落地，`#[serde(default)]` 兼容存量（见 3.4.5、3.4.6） |
| S3.4 | `ReasoningSelector` 选项改为「`default` + 投影档位」 | **已完成**：`reasoningChoices()` 组合选项；当前 choice 不在集合内时回落 `inherit` 并重置 |
| S3.5 | 档位单通道重构：`inherit` 退出选项位，`default` 成为内置档位，后端删掉模式回退 | **已完成**：`reasoning.ts` 重写 + `resolve_reasoning_level()` + IPC 双参收成 `reasoning_level`；前端 31 条 / 后端 46 条用例通过（见 3.2.1、3.2.6） |

#### 3.4.2 三种 action 到我们协议的映射（已落地）

`ChatOptions` 只有 `enabled` + `effort`，没有 budget 通道，因此解析层按下面的口径收：

| 快照 action | 解析结果 |
|---|---|
| `toggle` | `toggle = true`（前端可渲染 `off` / `on`） |
| `effort { values }` | 只保留 `KNOWN_EFFORTS`（`minimal/low/medium/high/xhigh/max`），其余丢弃；值里的 `none`/`off` 折算成 `toggle = true` 而不是一个档位 |
| `budget_tokens { min, max }` | **忽略**。我们没有对应字段；单独出现时整体判为 `None`（走未知模型策略） |
| `reasoning_options: []` | 判为 `None`（没有可用信息），不做「权威无档位」的强断言 |
| 字段缺失（`None`） | 判为 `None`，保守集合 `off + low/medium/high`，与 S1 现状一致 |

S3.4 渲染时再由 `toggle` / `efforts` 组合出选项：

| `levels` | 选项 |
|---|---|
| `toggle=true`、`efforts` 非空 | `default` + `off` + 每个 effort |
| `toggle=true`、`efforts` 空 | `default` + `off` + `on` |
| `toggle=false`、`efforts` 非空 | `default` + 每个 effort（不提供独立 `off`，关闭由模式或 `default` 控制） |
| `None` | `default` + `off` + `low/medium/high`（保守集合） |

档位到协议参数的折算由后端 `resolve_reasoning_level()` 一处负责：`default` → `(None, None)`；`off` → `(Some(false), None)`；`on` → `(Some(true), None)`；其余值 → `(Some(true), Some(值本身))`。这样放开 `xhigh` / `max` 只是多了一个字符串，不再需要改映射分支。前端只负责把「未覆盖」解析成具体档位（`resolveReasoningChoice`），不碰 `(enabled, effort)` 这一对。

#### 3.4.3 与设置页的关系

设置页的档位选项由 `REASONING_EFFORTS` 生成（S1 清理后已从 `reasoning.ts` 导入，不再本地定义）。S3.4 只改了工具栏，设置页**保持三档**：选模式时模型可能还没定（`mode.model` 可为 `null`），无法按模型投影，给出「并集」会重演通用路径静默丢弃的问题（F7）。因此设置页只做「保守交集」，具体档位是否可用在发送时由后端收口；这也是 S3.4 与设置页的分工边界。

#### 3.4.4 未知模型策略

`catalog_reasoning_levels` 只覆盖目录内模型，自定义模型名、`reasoning_options: []`、只声明 `budget_tokens` 三种情况都返回 `None`。策略：**`None` 时显示保守集合**（宁可让用户选，也不要误判隐藏）。`catalog_reasoning()` 返回 `Some(false)` 的模型可以整体禁用控件，但那是能力判定，与档位集合无关。

#### 3.4.5 S3.3 为什么不能单独改前端类型

`ModelConfigDto` 是 `deny_unknown_fields` 的（`src-tauri/src/protocol/dto/settings.rs`），而 `list_providers` 用 `serde_json::from_str(&r.models_json).unwrap_or_default()` 读存量（`src-tauri/src/cmd/provider.rs`）。存量 `models_json` 由前端 `JSON.stringify(models)` 原样写入，后端 `save_provider` 不解析。

于是只改前端 `ModelConfig` 加字段、Rust DTO 没同步，会走成这个链路：

1. 前端保存时把 `reasoningLevels` 写进 `models_json`
2. 后端下次 `list_providers` 反序列化，`deny_unknown_fields` 报「未知字段」
3. `unwrap_or_default()` 吞掉错误 → **模型列表变成空数组**
4. 用户所有模型凭空消失，界面无任何提示

这不是理论风险，是这条链路的必然结果。所以 S3.3 的四个改动点必须**同一批次**落地，不能拆：

| 文件 | 改动 |
|---|---|
| `src-tauri/src/protocol/dto/settings.rs` | `ModelConfigDto` 加 `#[serde(default)] pub reasoning_levels: Option<ReasoningLevelsDto>` |
| `src-tauri/src/cmd/provider.rs` | `list_models_cmd` / `preview_models_cmd` 两处构造点各补一行 |
| `src/settings/provider/types.ts` | `ModelConfig` 加 `reasoningLevels?: { toggle: boolean; efforts: string[] }` |
| `src-tauri/src/protocol/dto/mod.rs` | `test_provider_list_dto_roundtrip` 构造点补字段 |

`#[serde(default)]` 是必须的，否则存量 `models_json` 缺这个键同样会失败——和 `inputModalities` 当初的处理一致。

#### 3.4.6 档位存库还是现查

`reasoning_levels` 是**本地目录快照的静态事实**，不是 Provider API 返回的。两种口径：

| 方案 | 取舍 |
|---|---|
| A. 前端回传时一起写进 `models_json` | 目录快照更新后库里的档位变**陈旧**；还要多一次迁移 |
| B. 每次构造 DTO 时现查 `catalog_reasoning_levels(model_id)` | 永远跟随最新快照，零迁移；`models_json` 里即使有历史副本也被现查值覆盖 |

倾向 B。`list_providers` 因此不再纯粹读库，但要引入的只是一次 `HashMap::get`，零成本。

### 3.5 S1 遗留的代码风格问题（已清理）

三处不一致已在本轮处理完：

| # | 问题 | 处理结果 |
|---|---|---|
| C1 | `REASONING_CHOICES` 导出后无人使用 | 先由 `ReasoningSelector` 直接消费；S3.5 后降级为「未知模型的保守集合」，实际选项统一走 `reasoningChoices(levels)` |
| C2 | 档位枚举重复定义 | `reasoning.ts` 新增 `REASONING_EFFORTS` 常量，设置页与保守集合都由它派生；S3.4 后 `ReasoningChoice` 放宽为 `string`（取值来自目录投影，不再可能枚举穷举） |
| C3 | 设置页档位 label 未走 i18n | 新增 `settings.agent_mode.field.reasoning_effort.options.{low,medium,high}` 双语键 |

`REASONING_EFFORTS` 是「后端 `KNOWN_EFFORTS` 的子集」这一事实需要保持：设置页只暴露三档是**刻意的保守交集**，因为 `normalize_generic_reasoning_effort` 只认 `low/medium/high`，其余值在通用路径会被静默丢弃。工具栏的放开已在 S3.4 完成：档位由目录投影决定，目录没有信息时才回落到这三档。

---

## 4. 影响面清单

### S1（已落地）

| 文件 | 改动 | 风险 |
|---|---|---|
| `src/chat/core/reasoning.ts` | 新建：常量 + `reasoningChoices()` + `modeReasoningChoice()` + `resolveReasoningChoice()` + `toReasoningLevel()`；`REASONING_EFFORTS` 为保守集合唯一来源 | 低 |
| `src/chat/core/__tests__/reasoning.test.ts` | 新建：4 组共 17 条纯函数单测 | 低 |
| `src/chat/selectors/ReasoningSelector.tsx` | 新建：chip 选择器；显示值走 `resolveReasoningChoice`，越界回落 `default` | 低 |
| `src/stores/chatStore.ts` | 加 2 个字段 | 低 |
| `src/stores/chatStorePorts.ts` | `EnvPortReadable` + `createEnvPort` 各加一项；`getReasoningChoice` 内做 `resolveReasoningChoice` 解析 | 中（显示与下发的唯一合并点） |
| `src/stores/chatConversationSlice.ts` | 切换 / 新建会话重置为 `inherit`（I4） | 低 |
| `src/services/chatOrchestrator.ts` | `ChatEnvPort` 加方法；`send` 取档位值并 `toReasoningLevel` 兜底 | 中（唯一出口，需跑测试） |
| `src/services/chatService.ts` | `sendChatMessage` 加 1 个 `reasoningLevel` 参 | 低 |
| `src/chat/ChatView.tsx` | 插入控件 + 模式切换重置 | 低 |
| `src/assets/styles/global.css` | `.orb-reasoning-chip` | 低（Orb 窄布局已手测通过，2026-11-01） |
| `src/i18n/locales/{zh-CN,en-US}.json` | 新增 `chat.reasoning_depth`；新增 `settings.agent_mode.field.reasoning_effort.options.*` | 低 |
| `src/settings/pages/AgentModeSettings.tsx` | 档位选项改由 `REASONING_EFFORTS` 生成，label 走 i18n | 低 |
| `src-tauri/src/cmd/chat/mod.rs` | 命令加 1 个 `reasoning_level: Option<String>` 参 | 低 |
| `src-tauri/crates/assistant-agent/src/chat_options.rs` | 新建：`ReasoningOverride { level }` + `resolve_reasoning_level()` + `build_chat_options()`（纯函数，无模式依赖） | 中（纯函数，易测） |
| `src-tauri/crates/assistant-agent/src/chat_request.rs` | 透传 override | 低 |
| `src/services/__tests__/chatOrchestrator.test.ts` | env 字面量补方法 + 断言 | 低 |
| `src-tauri/crates/assistant-agent/tests/chat_runtime_spec.rs` | 调用点补参数 + 6 条档位用例 | 低 |

### S2（明确不做）

不落盘，因此**没有任何文件改动**。理由与未来重启时的注意事项见 3.3。

### S3（已落地）

| 文件 | 改动 | 状态 |
|---|---|---|
| `src-tauri/crates/assistant-providers/src/model_catalog.rs` | `ReasoningLevels` + `KNOWN_EFFORTS` + `parse_reasoning_levels` + `ModelFacts.reasoning_levels` + `catalog_reasoning_levels()` + 4 条单测 | 已完成 |
| `src-tauri/src/protocol/dto/settings.rs` | 新增 `ReasoningLevelsDto`；`ModelConfigDto` 加 `#[serde(default, skip_serializing_if = "Option::is_none")] reasoning_levels` | 已完成 |
| `src-tauri/src/cmd/provider.rs` | 新增 `model_config_dto()`，`list_models_cmd` / `preview_models_cmd` 两处构造点统一现查 `catalog_reasoning_levels` 填充 | 已完成 |
| `src-tauri/src/protocol/dto/mod.rs` | `test_provider_list_dto_roundtrip` 构造点补字段；新增存量 `models_json` 兼容用例 | 已完成 |
| `src/settings/provider/types.ts` | `ModelConfig` 加 `reasoningLevels` | 已完成 |
| `src/chat/core/reasoning.ts` | 新增 `reasoningChoices()`、`ReasoningLevelsLike`、`on` 档位；`ReasoningChoice` 放宽为 `string` | 已完成 |
| `src/chat/selectors/ReasoningSelector.tsx` | 选项改由当前模型的 `reasoningLevels` 生成；不在集合内时状态回落 `inherit`、显示回落 `default` | 已完成 |
| `src/i18n/locales/*.json` | `chat.reasoning_depth` 补 `minimal` / `xhigh` / `max` | 已完成 |

---

## 5. 验收标准

### S1

| # | 验收 | 判定 |
|---|---|---|
| A1 | 未覆盖 + 模式无声明 → 下发 `default` → `ChatOptions` 两字段均为 `None`（等价于实施前的透传默认） | 单测断言 `ChatOptions` 全等 |
| A2 | 选「关闭」后，请求不含 `reasoning_effort`，且 `thinking.type = disabled`（OpenAI 兼容路径） | 单测 |
| A3 | 选「高」后，请求 `reasoning_effort = "high"`、`thinking.type = enabled` | 单测 |
| A4 | 模式为 `deep`（`true/high`）、未覆盖 → 前端解析为 `high` → 等价 `true/high` | 单测（前端解析 + 后端折算两段） |
| A5 | 模式为 `deep`、覆盖为「关闭」→ 等价 `false/None`（覆盖优先） | 单测 |
| A6 | 从「深度」切到「快速」后，覆盖自动回到 `inherit` | 手测 + 组件测试 |
| A7 | 重新生成 / 重试 / 编辑重发 均带当前档位 | 手测（档位取值在编排器唯一出口） |
| A8 | Orb 面板工具栏不溢出、不换行错位 | 手测（窄宽度） |
| A9 | `npm run lint` / `npm run typecheck` / `npm run test` 通过 | CI |
| A10 | `cargo test -p assistant-agent` 通过 | CI |
| A11 | 模式为 `deep`、覆盖为「默认」→ 等价 `(None, None)`，即**忽略模式值**（这是「显式要 Provider 默认」） | 单测 |
| A12 | 后端收到 `inherit` 字符串（异常残留）→ 不 panic、不当作 effort 下发 | 单测 |

S1 实际验证结果（2026-10-31 首轮，2026-11-01 S3.5 重构后复验）：

| 项 | 结果 |
|---|---|
| A1 / A4 / A5 / A11 | 由 `chat_runtime_spec.rs` 的 6 条档位用例覆盖（`default` 忽略模式、缺省等价 `default`、`off` 优先、`effort` 覆盖、`on` 无 effort），通过 |
| A12 | 后端把未知档位值走 effort 分支，不 panic；`toReasoningLevel` 在前端先行兜底 |
| A2 / A3 | 由 `reasoning.test.ts` + `chatOrchestrator.test.ts` 覆盖到「档位值」层，Provider 报文层未单独断言（OpenAI/Anthropic 报文构造属既有测试面，未新增） |
| A9 | 通过（前端定向 31 条：reasoning 17 + orchestrator 14） |
| A10 | 通过（assistant-agent `chat_runtime_spec` 46 条） |
| A6 / A7 / A8 | **已手测通过**（2026-11-01 真机）：模式切换后覆盖回到 `inherit`、重新生成 / 重试 / 编辑重发均带当前档位、Orb 窄栏工具栏不溢出。A6 的「组件测试」部分仍未补，见 §9 |

### S2（明确不做）

无验收项。**不落盘的验收反而是有的**，并入 I4 的检查：切换会话后控件回到模式推导值，重启应用后同样回到模式推导值。

### S3

| # | 验收 | 判定 |
|---|---|---|
| C1 | `catalog_reasoning_levels` 对四类目录声明给出预期结果 | 单测（已通过，S3.1/S3.2） |
| C2 | `ModelConfigDto` round-trip 含 `reasoningLevels`，camelCase 正确 | 单测（已通过，`test_provider_list_dto_roundtrip`） |
| C3 | 存量 `models_json`（无 `reasoningLevels` 键）反序列化成功，模型数量不变 | 单测已通过（`test_model_config_dto_accepts_legacy_models_json`）；**旧库启动已手测通过**（2026-11-01） |
| C4 | 支持 `xhigh`/`max` 的模型在工具栏能选到对应档位 | **已手测通过**（2026-11-01；`reasoningChoices` 组合已单测） |
| C5 | 目录只声明 `toggle` 的模型不出现强度档位，只显示开关 | **已手测通过**（2026-11-01；`toggle=true` / `efforts=[]` → `default/off/on` 已单测） |
| C6 | 目录外模型显示保守集合 `default + off + low/medium/high` | **已手测通过**（2026-11-01；`levels=undefined` → 保守集合已单测） |
| C7 | 已选档位不在新模型集合内时，状态回落 `inherit`、显示回落 `default`，不残留旧值 | **已手测通过**（2026-11-01）；组件内 `useEffect` 重置 + 单测覆盖集合判定，**组件测试仍未补**（见 §9） |
| C8 | `cargo test -p assistant-providers --features internal-tests` / `npm run test` 通过 | CI |

---

## 6. 测试计划

| 层 | 用例 |
|---|---|
| 前端纯函数 | `toReasoningLevel`：具体档位原样、`inherit` 折成 `default`（2 条）；`reasoningChoices`：五种 `levels` 组合 + 首项恒为 `default` + 永不含 `inherit`（6 条）；`modeReasoningChoice`：`off` / effort / `on` / effort-only / 全空（5 条）；`resolveReasoningChoice`：覆盖原样 / 回落模式 / 兜底 `default` / 永不返回 `inherit` 或 `null`（4 条）。共 17 条 |
| 前端编排 | `send` 在 `inherit` 时下发 `default`；在 `high` 时下发 `"high"`；断言实参形态 |
| 后端纯函数 | `resolve_reasoning_level`：`None` 与 `"default"` 等价 `(None, None)`；`"off"` → `(Some(false), None)`；`"on"` → `(Some(true), None)`；effort → `(Some(true), Some(v))`。`chat_runtime_spec` 内 6 条用例覆盖（含「default 忽略模式值」） |
| 后端目录解析 | `catalog_reasoning_levels`：`effort` 单声明、`toggle+effort`、`toggle` 单声明、仅 `budget_tokens`、目录外模型（4 条，已通过） |
| 契约 | 确认 `send_chat_message` 不传 `reasoning_level` 仍可用（等价 `default`） |
| S3.3 契约 | `ModelConfigDto` round-trip 含 `reasoningLevels`；**存量 `models_json`（无该键）反序列化成功且模型不丢**（对应 R10） |

---

## 7. 风险与取舍

| # | 风险 / 取舍 | 处理 |
|---|---|---|
| R1 | 覆盖状态放 `agentModeStore` 会污染设置页 | 明确放 `chatStore`（见 3.2.2） |
| R2 | 前端与后端各算一次合并 → 双源漂移 | 切成两段互不重叠的纯函数：前端只做「未覆盖 → 具体档位」，后端只做「档位 → `(enabled, effort)`」（见 3.2.1 / 3.2.6） |
| R3 | 只下发 effort 造成 Provider 语义分裂 | 成对下发（I2 / F8）；单通道后，成对折算只发生在 `resolve_reasoning_level()` 一处 |
| R4 | `enabled=false` 但仍带 effort | `resolve_reasoning_level()` 的 `off` 分支结构上只产出 `(Some(false), None)`，不可能同时带 effort |
| R5 | GLM/DeepSeek 归一化让「中」看起来像「高」 | S1 只暴露 `low/medium/high`，与设置页一致；不在 UI 承诺精确强度。S3 放开 `xhigh/max` 后，归一化差异改为由「档位集合按模型生成」吸收（见 3.4） |
| R6 | 切换会话覆盖丢失，用户以为坏了 | S2 明确不做落盘；Tooltip 明示「本次会话已覆盖」，切会话即回到模式推导值（这是设计而非缺陷） |
| R7 | Orb 窄布局第四个 chip 挤压 | 新增 `.orb-reasoning-chip` 与 `.orb-mode-chip` 共用同一组尺寸规则（`max-width: 100px`）；**已手测（2026-11-01）确认不溢出、不换行**，无需缩写 label |
| R8 | 未来若重启 S2，迁移非幂等 → 重复加列报错 | 沿用 `pragma_table_info` 判存在再 `ALTER` 的既有模式（当前不涉及） |
| R9 | 未来若重启 S2，`ConversationRow` 无 `Default`，漏改编译期即报错 | 这是优点：编译器强制覆盖全部构造点（当前不涉及） |
| R10 | S3.3 只改前端类型 → `models_json` 带新字段，`deny_unknown_fields` + `unwrap_or_default()` 让模型列表变空且无提示 | 四个改动点（DTO / 两处构造 / 前端类型 / 契约测试）同批落地；`reasoning_levels` 必须带 `#[serde(default)]`（见 3.4.5） |
| R11 | 档位写进 `models_json` 后随目录快照更新而陈旧 | 采用现查口径：构造 DTO 时 `catalog_reasoning_levels(model_id)` 覆盖（见 3.4.6） |

---

## 8. 本阶段不做

| # | 不做项 | 理由 |
|---|---|---|
| N1 | 修设置页布尔开关无法表达 `null`（F9） | 属模式配置三态化，独立议题；工具栏已不再暴露「未覆盖」这一档，两者无耦合 |
| N2 | `mode_id` 回读 / 按会话恢复模式（F6） | 与思考覆盖无关，但同属「会话级参数」，建议作为后续阶段一并处理 |
| N3 | S1 阶段暴露 `max` / `xhigh` 等扩展档位 | 与 Provider 归一化冲突（F7）：通用路径会静默丢弃。S3 改为按模型 `reasoning_options` 生成档位后才放开（见 3.4） |
| N4 | 覆盖影响历史消息或触发重新生成 | 覆盖只作用于「下一次请求」，不改已生成内容 |
| N5 | 把思考深度纳入压缩 / 摘要等内部调用 | 内部调用走独立路径，不读会话覆盖 |
| N6 | **S2 会话级落盘（`conversations` 加列）** | **用户明确不做**。档位是「当前会话的显示态」，切会话 / 重启后回到模式推导值；不落盘也避免了迁移与 `ConversationRow` 全构造点改动的风险（R8 / R9） |

---

## 9. 已知残留（归档时状态）

| # | 项 | 现状 | 处置 |
|---|---|---|---|
| U1 | `ReasoningSelector` 组件级测试缺失 | `src/chat/selectors/` 下无 `*.test.tsx`；组件行为（选项裁剪渲染、覆盖写入 `chatStore`、切模型时的 `useEffect` 重置）仅由纯函数单测 + 手测覆盖 | 记为技术债，需要时补 `ReasoningSelector.test.tsx`；A6 / C7 的「组件测试」判定因此降级为「手测通过」 |
| U2 | Provider 报文层未单独断言 | A2 / A3 只断言到「档位值」层，OpenAI / Anthropic 的报文构造属既有测试面 | 记录项，不新增断言 |
| U3 | 手测项无自动化回归 | A6–A8、C3–C7 的结论来自 2026-11-01 真机手测，后续改动无 CI 拦截 | 由 U1 的组件测试部分覆盖；其余属交互 / 布局面，依赖手测 |

验证口径说明：上表 U1 是本阶段**唯一**的自动化缺口；其余验收项（A1–A5、A9–A12、C1、C2、C8）均为 CI 单测，S3.5 重构后已复验通过。
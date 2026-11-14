# I28 — `send_chat_message` 参数收敛设计

| 项 | 值 |
|---|---|
| 状态 | Draft（待评审） |
| 日期 | 2026-11-01 |
| 修订 | v2：修正 serde `Option` 缺省行为、补 A2 混合方案、补 DTO 反序列化测试项、对齐 I20 既有结论 |
| 前置 | I21（`send_chat_message` Pipeline 重构，已完成）、I22（`chat_request` / `utils` 下放 `assistant-agent`，已完成）、I20（命令简化规范，已确认命令数量不再变） |
| 范围 | `send_chat_message` 的 IPC 参数面 + 其下游两个内部函数的参数面 |
| 影响范围 | `src/chat/core/chatTypes.ts`、`src/services/chatService.ts`、`src/services/chatOrchestrator.ts`、`src-tauri/src/protocol/dto/{chat,mod}.rs`、`src-tauri/src/cmd/chat/{mod,insert}.rs` |
| 不在范围 | 流式协议、事件载荷、消息树语义、`stream.rs` 后台任务、压缩逻辑、命令数量增减 |

本文档只讨论**参数形状**，不重新讨论 I21 已经解决的职责拆分。I21 把 430 行的上帝函数拆成了 4 段管道，但每一段的入参仍是位置参数列表，本文档处理的是这一层遗留问题。

---

## 1. 概述

`send_chat_message` 目前是 **1 个框架注入参数 + 7 个前端字段**的位置参数列表，下游 `insert_chat_messages`（7 参数）、`spawn_chat_stream`（8 参数）同样如此。结果是：**新增或调整一个发送字段，要同步改 4 个函数签名 + 2 处调用点 + 1 个 TS 接口**；且相邻参数中有 4 个都是 `String` / `Option<String>`，错序不会报编译错误。

仓库内已有三个结构体入参先例（`os_tool_exec` 的 `request`、`get_conversation` 的 `opts`、`patch_conversation` 的 `patch`）。本文档把 `send_chat_message` 收敛到同一形态，并顺带收敛内部两层，使「新增字段」的成本降为**两处字段声明**（Rust struct + TS 接口）。

---

## 2. 目标

- 新增/删除/调整发送字段时，**不需要改任何函数签名**，只改结构体字段。
- 消除 `spawn_chat_stream` 的 `#[allow(clippy::too_many_arguments)]`。
- 与既有结构体入参先例保持同一风格（`rename_all = "camelCase"` + 嵌套对象 + `*Dto` 命名）。
- 用一条反序列化单测把「哪些字段可省略」固化为可执行契约。

## 3. 非目标

- 不改命令名、不改返回值 `Result<(), String>`、不改事件名与载荷。
- **不改变命令数量**：仍是一个 `send_chat_message`。I20 §3 把该命令归类为「参数多但不可拆」，本文档不推翻该结论——I20 说的是"不拆成多个命令"，本文档只打包参数（详见 §5.5）。
- 不引入 Codex 式 `Op` 枚举 / channel 提交（I21 §9 已判定为过度设计）。
- 不把 `build_chat_request` 的参数面一起改（该函数已在 `assistant-agent` 层，签名语义清晰，列为后续可选项，见 §10）。
- 不重构前端 `ChatEnvPort`（5 个 getter 的拆分有其正当用途，见 §5.4）。
- 不改 `create_branch` 等其它命令，尽管它们同样可以收敛（见 §10）。

---

## 4. 既有事实

全部经代码核对（行号以当前工作区为准）。

### 4.1 前端调用面

| 事实 | 位置 |
|---|---|
| `sendChatMessage` 是 7 个位置参数的箭头函数，内部转成 7 个 camelCase key 的**扁平**对象 | `src/services/chatService.ts` L81–98 |
| 唯一调用方是 `chatOrchestrator.send()`，逐项从 `deps.env` 取 5 个值再拼 `toReasoningLevel(...)` | `src/services/chatOrchestrator.ts` L313–321 |
| 7 个值来自 5 个 env getter + 2 个入参（`convId`、`content`） | `src/services/chatOrchestrator.ts` L55–59 |
| 参数类型写成 `string \| null \| undefined`，且 `reasoningLevel` 带默认值 `= null` | `src/services/chatService.ts` L84–88 |
| 服务层类型统一从 `src/chat/core/chatTypes.ts` 导入并 re-export | `src/services/chatService.ts` L2–9 |
| `tauriInvoke` 只透传 `args`，日志只记顶层 key 名，不记值 | `src/services/tauriInvoke.ts` L5–18 |

### 4.2 后端命令面

| 事实 | 位置 |
|---|---|
| 命令签名 8 个参数：`app` + 7 个前端字段 | `src-tauri/src/cmd/chat/mod.rs` L28–37 |
| 无任何 `serde` 属性，靠 Tauri 逐参数名反序列化 | 同上 |
| 命令体 4 步编排（校验 L38 / 落库 L44–55 / 建请求 L62–72 / 建流 L74–84），每步都重拼一次参数 | `src-tauri/src/cmd/chat/mod.rs` L38–84 |
| `reasoning_level` 在 L60–61 被 move 进 `ReasoningOverride`，其余字段靠 `as_deref()` 借用 | 同上 |

### 4.3 下游内部函数

| 函数 | 位置 | 参数数 | 备注 |
|---|---|---|---|
| `validate_and_preprocess` | `cmd/chat/mod.rs` L90–109 | 3 | 无需改动 |
| `insert::insert_chat_messages` | `cmd/chat/insert.rs` L19–27 | 7（`conn` + 6） | 与命令字段高度重合 |
| `spawn_chat_stream` | `cmd/chat/mod.rs` L112–121 | 8 | 带 `#[allow(clippy::too_many_arguments)]`（L111） |
| `insert::insert_on_branch` | `cmd/chat/insert.rs` L95 | 7 | 私有子函数 |
| `insert::insert_normal_append` | `cmd/chat/insert.rs` L158 | 7 | 私有子函数 |
| `insert::insert_user_message_with_attachments` | `cmd/chat/insert.rs` L193 | 6 | 私有子函数 |
| `insert::insert_placeholder_assistant_message` | `cmd/chat/insert.rs` L231–238 | 7 | 私有子函数；`provider_id`/`model_id` 为 `Option<&str>` |
| `build_chat_request` | `crates/assistant-agent/src/chat_request.rs` L24–32 | 7 | 已下放 agent 层 |

### 4.4 仓库内已有的结构体入参先例

| 先例 | 位置 | 前端传法 |
|---|---|---|
| `os_tool_exec(registry, request: OsToolExecRequest)`，struct 带 `#[serde(rename_all = "camelCase")]`，**无** `deny_unknown_fields` | `src-tauri/src/cmd/os_tool_exec.rs` L7–14（命令 L16–20） | `invoke("os_tool_exec", { request: { tool, action, params } })`（`src/tools/os.ts` L27–29） |
| `get_conversation(app, id, opts: Option<ConversationQueryOptsDto>)` | `src-tauri/src/cmd/conversation.rs` L89–93 | `invoke("get_conversation", { id, opts: {...} })` |
| `patch_conversation(app, id, patch: ConversationPatchDto)` | `src-tauri/src/cmd/conversation.rs` L217–221 | `invoke("patch_conversation", { id, patch: {...} })` |

**注意先例并不统一**：`ConversationPatchDto`（`protocol/dto/chat.rs` L142）与 `ConversationQueryOptsDto`（L155）带 `deny_unknown_fields`，而 `OsToolExecRequest` 不带。因此"既有先例都带 `deny_unknown_fields`"是错误概括，本文档 §7.2 按各自语义分别决策。

**关键框架事实**：Tauri v2 命令宏按**参数名**逐个从 JSON 对象取 key，`#[serde(flatten)]` 无法改变这一映射，所以"单 struct 参数"在 JSON 里必然是嵌套对象（`{ args: {...} }`）。想让 `conversationId` 留在顶层，只能把它写成独立参数（§6 方案 A2）。

### 4.5 serde 对 `Option` 字段的缺省行为

serde 派生对 `Option<T>` 字段**缺省即为 `None`，不需要 `#[serde(default)]`**。本仓库有两处直接证据：

| 证据 | 说明 |
|---|---|
| `ConversationPatchDto` 的 4 个 `Option<String>` 字段全部没有 `#[serde(default)]`（`protocol/dto/chat.rs` L143–152） | 前端 `renameConversation` 只传 `{ title, titleSource }`，`modelId` / `providerId` 两个 key 缺失，功能正常 |
| `ConversationQueryOptsDto.before_timestamp: Option<i64>` 无 `#[serde(default)]`，而同结构体里非 `Option` 的 `mode` / `limit` 才加（`protocol/dto/chat.rs` L156–164） | 说明本仓库约定是「`Option` 不加、非 `Option` 才加」 |

这条事实修正了本文档 v1 的一个错误判断（详见 §5.3）。

### 4.6 测试面

| 事实 | 位置 |
|---|---|
| `chatService.sendChatMessage` 在两处被 `vi.fn()` mock | `src/services/__tests__/chatOrchestrator.test.ts` L28、`src/stores/__tests__/chatStreamCharacterization.test.ts` L13 |
| 对它的断言只有调用次数，不断言参数形状 | `src/services/__tests__/chatOrchestrator.test.ts` L110 |
| Rust 侧 `insert_chat_messages` / `spawn_chat_stream` 无任何测试引用 | 全仓 grep 仅命中定义与调用 |
| `protocol/dto/mod.rs` 已有 `mod tests`（L16）与 `roundtrip` 辅助（L20），但要求 `Serialize + PartialEq` | `src-tauri/src/protocol/dto/mod.rs` L16–30 |

---

## 5. 问题分析

### 5.1 变更放大：加一个字段要动 6 处

以「新增 `thinkingBudget` 字段」为例，当前必须改动：

| # | 位置 | 改动性质 |
|---|---|---|
| 1 | `chatService.sendChatMessage` 参数列表 + 对象字面量 | 位置参数 → 调用点错序风险 |
| 2 | `chatOrchestrator.send()` 调用点 | 位置参数 → 漏传/错序 |
| 3 | `send_chat_message` 签名 | 编译期强制 |
| 4 | `insert_chat_messages` 签名 + 调用点 | 仅透传，但同样要改 |
| 5 | `spawn_chat_stream` 签名 + 调用点 | 仅透传，但同样要改 |
| 6 | 若字段参与请求构建，还要改 `build_chat_request` | 真正使用处 |

其中 2、4、5 是**纯透传**，它们的存在只是为了把值搬过去，却每次都要跟着改签名。这也是 §4.2 里命令体"每步重拼一次参数"的直接后果。

### 5.2 位置参数的错序风险是真实存在的

`send_chat_message` 的 7 个字段里有 4 个是 `String` / `Option<String>`：`conversation_id`、`content`、`provider_id`、`model_id`。把 `provider_id` 与 `model_id` 写反，类型检查完全通过，运行期表现是「用 A 供应商的密钥调 B 模型」——报错会指向 provider 的 404/401，与真实原因相隔很远。

结构体入参把这类错误从「静默错序」降级为「编译期字段缺失/多余」。

### 5.3 可选性只写在 TS 签名里（不是 bug，是可读性问题）

**先修正一个常见误解**：`undefined` 在 JSON 序列化时被丢弃，但后端 `Option` 字段缺 key 时得到 `None`（§4.5），所以"前端省略 key"不会导致后端报错。v1 文档把它写成"隐式契约会导致解析失败"，是错误判断。

真正成立的问题更轻：`chatService` 把三个可空字段声明为 `string | null | undefined`（L84–88），而调用点恒传 `null`（`chatOrchestrator.ts` L301–312）。读代码的人无法从签名判断"到底哪些字段可以省略、省略后行为如何"。改成结构体后，可选性由 Rust `Option` + serde 隐式可选统一表达，TS 侧用 `?:` 表达，两侧语义一致。

**结论**：这是收益最小的一条，不构成独立动机，不应在评审里被当作主要理由。

### 5.4 前端 5 个 getter 不构成问题

`ChatEnvPort` 提供 `getSelectedModeId` / `getSelectedProviderId` / `getSelectedModelId` / `getReasoningChoice` / `getPendingBranchPoint` 五个独立 getter，看起来像"应该合并成一个对象"，但：

- `getPendingBranchPoint()` 在发送前被消费两次：一次用于乐观消息分支判断（`chatOrchestrator.ts` L223–255），一次用于传给后端（L317）。合并成对象会强迫调用方提前取快照，反而容易读到过期值。
- `getIsStreaming` / `getActiveConversationId` 在 `send()` 中还有独立用途。

**结论**：env port 保持原样，收敛只发生在「调用后端那一刻」的组装处。

### 5.5 与 I20 结论的关系

I20 §3 的"不简化清单"里写着：`Chat` / `send_chat_message` / `cancel_chat` —「核心流式命令，参数多但不可拆」。评审时容易误读为"参数面也不许动"。需要在文档里明确二者边界：

| 维度 | I20 的"不可拆" | 本文档 |
|---|---|---|
| 命令数量 | 不把 1 个命令拆成多个 | **同样不变**，仍是 1 个 |
| 参数形状 | 未涉及 | 打包成 struct |

即：I28 与 I20 是**正交**的，不构成结论反转。

---

## 6. 方案对比

| 方案 | 做法 | 新增字段成本 | 缺点 | 结论 |
|---|---|---|---|---|
| **A1. 全量单 struct** | `send_chat_message(app, args: SendChatMessageArgsDto)`，前端传 `{ args: {...} }` | 2 处 | `tauriInvoke` 日志只剩 `args` 一个 key，连会话 id 都看不到 | 备选 |
| **A2. 混合（推荐）** | `send_chat_message(app, conversation_id: String, args: SendChatMessageArgsDto)`，前端传 `{ conversationId, args: {...} }` | 2 处 | 比 A1 多一个参数（3 个，仍远低于 8） | **推荐** |
| B. 保留扁平参数 | 维持现状 | 6 处 | 问题全部保留 | 否决 |
| C. 保留扁平但内部收敛 | IPC 不变，只把 `insert` / `spawn` 收成结构体 | IPC 层仍 6 处中的 3 处 | 只解决一半，两层风格不一致 | 仅作过渡（§9 Step 1） |
| D. `Op` 枚举 / channel | Codex 式提交 | — | 无多 agent 需求，I21 §9 已否决 | 否决 |

**推荐 A2 的三条理由**：

1. **与最接近的两个先例一致**：`get_conversation(app, id, opts)`、`patch_conversation(app, id, patch)` 都是"会话 id 在顶层 + 其余打包"。`send_chat_message` 同样是会话域命令。
2. **保住日志可观测性**：`tauriInvoke` 只记顶层 key 名（L5–18）。A1 会让日志变成 `argsKeys: ["args"]`，排查时无法区分是哪个会话；A2 仍是 `["conversationId", "args"]`。
3. **会话 id 是必填标量**，放进 struct 只是把它藏起来，没有任何类型安全收益。

A1 的唯一优势是"命令签名最短"，但 3 个参数不触发 clippy 阈值，收益不成立。

---

## 7. 推荐方案（A2）

### 7.1 IPC 层：新增 `SendChatMessageArgsDto`

放在 `src-tauri/src/protocol/dto/chat.rs`，与 `ConversationPatchDto` / `ConversationQueryOptsDto` 同址（该文件已是"前端 ↔ 后端命令契约"的既定落点）；`protocol/dto/mod.rs` L10 的 `pub use chat::*;` 会自动导出，无需改 mod.rs。

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendChatMessageArgsDto {
    pub content: String,
    pub mode_id: Option<String>,
    pub branch_point_message_id: Option<String>,
    pub provider_id: String,
    pub model_id: String,
    pub reasoning_level: Option<String>,
}
```

命令改为：

```rust
#[command]
pub async fn send_chat_message(
    app: AppHandle,
    conversation_id: String,
    args: SendChatMessageArgsDto,
) -> Result<(), String> {
    let SendChatMessageArgsDto {
        content,
        mode_id,
        branch_point_message_id,
        provider_id,
        model_id,
        reasoning_level,
    } = args;
    // L38 起的编排不变；conversation_id 仍是本地变量
}
```

一次解构后，`insert` / `spawn` 的调用不再需要重新拼装 7 个实参。

### 7.2 serde 属性决策

| 属性 | 决定 | 理由 |
|---|---|---|
| `rename_all = "camelCase"` | **用** | 与三个既有先例一致；前端 key 无需改名 |
| `#[serde(default)]` on 3 个 `Option` 字段 | **不用** | serde 对 `Option<T>` 缺省即为 `None`（§4.5），加与不加行为相同；本仓库既有 DTO 均不加，保持一致 |
| `deny_unknown_fields` | **不用** | 见下 |
| `skip_serializing_if` | 不需要 | 本结构体只做反序列化，无序列化路径 |

`deny_unknown_fields` 的取舍需要单独说明，因为它是**宽松策略**，代价可测量：

- `ConversationPatchDto` 用它是为了挡住拼写错误的 patch key（精确补丁语义）。
- 发送命令是**长期演进的入参**：加上它会让"前端先上线新字段、后端后上线"直接报错。
- 宽松的代价：**可选字段拼错会被静默吞掉**（例如前端写成 `branchPointMsgId`，后端得到 `None`，静默退化为普通追加）。这个风险由 §7.6 的单测 + §11 的风险项显式承担。

### 7.3 前端层

TS 接口放在 `src/chat/core/chatTypes.ts`（服务层类型统一来源，`chatService.ts` L2–9 已在此导入并 re-export），不在 `chatService.ts` 里就地定义：

```ts
export interface SendChatMessageArgs {
  content: string;
  modeId?: string | null;
  branchPointMessageId?: string | null;
  providerId: string;
  modelId: string;
  reasoningLevel?: string | null;
}
```

`src/services/chatService.ts` L81–98 改为：

```ts
sendChatMessage: (
  conversationId: string,
  args: SendChatMessageArgs,
): Promise<void> =>
  invoke("send_chat_message", { conversationId, args }),
```

`src/services/chatOrchestrator.ts` L313–321 改为：

```ts
await chatService.sendChatMessage(convId, {
  content,
  modeId,
  branchPointMessageId: pendingBranchPoint,
  providerId: selectedProviderId,
  modelId: selectedModelId,
  reasoningLevel: toReasoningLevel(deps.env.getReasoningChoice()),
});
```

**关于日志补偿**：A2 保住了 `conversationId` 这个顶层 key，`args` 内部的字段名不再出现在日志里。若认为这仍不够，可在 `sendChatMessage` 内 `catch` 时补一行 `log.error` 记录 `Object.keys(args)`——但这需要给 `chatService.ts` 增加 `createLogger` 导入（当前该文件只有 L1–2 两行 import）。建议**先不做**，等实际排查受阻再加，避免为日志改动服务层。

### 7.4 内部层：两个结构体，去掉 clippy allow

**`insert.rs`**：

```rust
pub(crate) struct InsertChatInput<'a> {
    pub conversation_id: &'a str,
    pub content: &'a str,
    pub mode_id: Option<&'a str>,
    pub branch_point_message_id: Option<&'a str>,
    pub provider_id: &'a str,
    pub model_id: &'a str,
}

pub(crate) fn insert_chat_messages(
    conn: &rusqlite::Connection,
    input: InsertChatInput<'_>,
) -> Result<InsertedMessages, String>
```

7 参数 → 2 参数。注意 `conversation_id` 仍出现在这里：它是 DB 主键，虽然 IPC 层在顶层，内部仍需要。

**`mod.rs`**：`spawn_chat_stream` 收成单参数结构体，删掉 L111 的 `#[allow(clippy::too_many_arguments)]`。

```rust
struct ChatStreamSpawn<'a> {
    app: &'a AppHandle,
    conversation_id: &'a str,
    assistant_msg_id: &'a str,
    user_msg_id: Option<&'a str>,
    content: &'a str,
    request: ChatRequest,
    detail: ConversationDetail,
    provider_row: ProviderRow,
}

fn spawn_chat_stream(spawn: ChatStreamSpawn<'_>) -> Result<(), String>
```

`request` / `detail` / `provider_row` 是所有权转移字段（`ChatRequest` 内含 `Box<dyn BaseProviderAdapter>`，无法借用），结构体因此混合借用与所有权。`spawn_chat_stream` 是**同步函数**，结构体在函数入口即被解构、不跨 `await`，所以不存在生命周期风险（v1 文档的"需注意不跨 async move"属于过度担心，见 §11）。

### 7.5 `insert.rs` 的 4 个私有子函数保持原样

`insert_on_branch` / `insert_normal_append` / `insert_user_message_with_attachments` / `insert_placeholder_assistant_message` 仍是 6–7 个位置参数，**本次不改**，理由：

- 它们的入参不是"请求字段"，而是**已解析的局部值**：`now`（时间戳）、`sibling_index`、`parent_id`（`Option<String>` 所有权）、`detail_before`。这些不在 `InsertChatInput` 的语义范围内，硬塞进去会让结构体变成杂物袋。
- 它们是模块私有函数，`clippy::too_many_arguments` 阈值（7）恰好未触发，无编译告警。

唯一需要显式说明的转换点：`InsertChatInput.provider_id: &str`（必填）→ `insert_placeholder_assistant_message(provider_id: Option<&str>)`（可空），内部需包一层 `Some(...)`。当前代码用 `msg_provider_id: &Option<String>` + `.as_deref()` 实现，改造后保持不变即可。

### 7.6 新增 DTO 反序列化单测

`protocol/dto/mod.rs` 的 `roundtrip` 辅助要求 `Serialize + PartialEq`，本结构体只有 `Deserialize`，因此**不能复用**，需用 `serde_json::from_str` 直接断言。放在 `protocol/dto/mod.rs` 的 `mod tests` 内：

| 用例 | 输入 | 期望 |
|---|---|---|
| 1. 只传必填字段 | `{"content":"x","providerId":"p","modelId":"m"}` | `Ok`，3 个可选字段为 `None` |
| 2. 缺必填字段 | 去掉 `providerId` | `Err`，错误含 `missing field` 与 `providerId` |
| 3. 拼错可选字段 | 传 `{"branchPointMsgId":"b", ...}` | `Ok`（宽松策略生效），`branch_point_message_id` 为 `None`——**这是 §7.2 宽松策略的代价，用测试把行为钉死** |

这条测试是本文档唯一新增的测试资产，也是"可选字段可省略"这一契约的可执行表达。用例 3 的意义在于：把"静默退化"从隐患变成**已知并受控**的行为。

### 7.7 收敛后的形状

```
send_chat_message(app, conversation_id, args)          ← 8 → 3 参数
├── validate_and_preprocess(&app, &args.content, &conversation_id)   ← 保持 3 参数
├── insert_chat_messages(&conn, InsertChatInput { .. })              ← 7 → 2 参数
├── build_chat_request(..)                                          ← 保持 7 参数（agent 层，见 §10）
└── spawn_chat_stream(ChatStreamSpawn { .. })                       ← 8 → 1 参数
```

| 函数 | 改前 | 改后 |
|---|---|---|
| `send_chat_message` | 8 | 3 |
| `insert_chat_messages` | 7 | 2 |
| `spawn_chat_stream` | 8（含 clippy allow） | 1（无 allow） |
| 新增字段需改动的文件数 | 6 处 | 2 处（Rust struct + TS interface） |

---

## 8. 变更清单

| # | 文件 | 位置 | 改动 | 契约 |
|---|---|---|---|---|
| 1 | `src-tauri/src/protocol/dto/chat.rs` | 追加于 L164 后 | 新增 `SendChatMessageArgsDto` | — |
| 2 | `src-tauri/src/protocol/dto/mod.rs` | L16 起 `mod tests` | 新增 3 个反序列化用例（§7.6） | — |
| 3 | `src-tauri/src/cmd/chat/mod.rs` | L28–37 | 命令签名 → `(app, conversation_id, args)` + 入口解构 | 需同步 |
| 4 | `src-tauri/src/cmd/chat/mod.rs` | L44–55 | `insert_chat_messages` 调用改 `InsertChatInput` | 内部 |
| 5 | `src-tauri/src/cmd/chat/mod.rs` | L111–121 | `spawn_chat_stream` 改单参数 + 删 `allow` | 内部 |
| 6 | `src-tauri/src/cmd/chat/mod.rs` | L74–84 | `spawn_chat_stream` 调用点改结构体字面量 | 内部 |
| 7 | `src-tauri/src/cmd/chat/insert.rs` | L19–27 | `insert_chat_messages` 改 `(conn, input)` + 内部改用 `input.*` | 内部 |
| 8 | `src/chat/core/chatTypes.ts` | 追加 | 新增 `SendChatMessageArgs` 接口 | — |
| 9 | `src/services/chatService.ts` | L81–98 | 7 位置参数 → `(conversationId, args)` + `{ conversationId, args }` | 需同步 |
| 10 | `src/services/chatOrchestrator.ts` | L313–321 | 调用点改对象字面量 | 需同步 |
| 11 | `src/services/__tests__/chatOrchestrator.test.ts` | L28 / L110 | 无需改：mock 是 `vi.fn()`，断言只看调用次数 | — |
| 12 | `src/stores/__tests__/chatStreamCharacterization.test.ts` | L13 | 无需改（同上） | — |

**前后端必须同一提交**：IPC 契约是硬断点，不存在灰度。仓库内只有 1 个调用点，切换成本低。`ipc.rs` 的 `generate_handler!` 列表**不变**（命令名未改）。

---

## 9. 实施顺序

分两步提交，每步都可独立通过编译与测试：

**Step 1（纯内部，零 IPC 变更）**
1. `insert.rs` 引入 `InsertChatInput`，改 `insert_chat_messages` 签名与唯一调用点。
2. `mod.rs` 引入 `ChatStreamSpawn`，改 `spawn_chat_stream` 签名与唯一调用点，删除 `#[allow(clippy::too_many_arguments)]`。
3. 在 `src-tauri` 目录跑 `cargo check` / `cargo test` / `cargo clippy --all-targets -- -D warnings`。

**Step 2（IPC 收敛）**
1. 新增 `SendChatMessageArgsDto` + §7.6 的 3 个单测。
2. 改命令签名 + 前端 `chatTypes` / `chatService` / `chatOrchestrator`。
3. 在仓库根目录跑 `npm run typecheck` / `npm run lint` / `npm test`；在 `src-tauri` 目录跑 `cargo check` / `cargo test` / `cargo clippy --all-targets -- -D warnings`。
4. 手测：正常发送、分支重生成（`branchPointMessageId` 非空）、无 mode（`modeId` 为 null）、reasoning 覆盖、**前端只传必填 4 字段（省略 3 个可选 key）仍能发送**、取消流、provider 缺失报错。

---

## 10. 后续可选项（不在本次范围）

| 项 | 现状 | 收益 | 建议 |
|---|---|---|---|
| `build_chat_request` 参数面 | 7 个位置参数（`chat_request.rs` L24–32） | 与 `ChatRequest` 同址，语义清晰 | 暂不动；若后续继续加字段（如新的 capability 开关），再收成 `ChatRequestInput` |
| `create_branch` | 6 个参数（`cmd/conversation.rs` L394–401） | 同样可收敛 | 与本文档同批次可选，单独提交 |
| `createConversation` | 3 个位置参数（`chatService.ts` L17–21） | 小 | 收益低，不做 |
| `ChatEnvPort` 合并 | 5 个 getter | 无（§5.4） | 不做 |
| `ConversationPatchDto` 的 `deny_unknown_fields` | 已存在 | 与发送命令的宽松策略不一致 | 保持现状：patch 是精确补丁语义，宽松会掩盖拼写错误 |

---

## 11. 风险与缓解

| 风险 | 影响 | 缓解 |
|---|---|---|
| 前后端未同批上线导致 IPC 契约不匹配 | 发送功能整体不可用，报 invalid args | 单提交切换；仅 1 个调用点；手测清单（§9 Step 2.4） |
| 可选字段拼错被静默吞掉（无 `deny_unknown_fields`） | 例如 `branchPointMsgId` 拼错 → 静默退化为普通追加，不报错 | §7.6 用例 3 固化该行为；TS 接口 + 唯一组装点 `chatService` 双重约束；评审时检查拼写 |
| 参数错误信息变模糊 | 从"缺少 `provider_id`"变成"缺少 `args`" | 顶层缺 `args` 时提示本身可读；`args` 内字段缺失仍由 serde 报 `missing field providerId` |
| 命令签名变更影响其它调用方 | — | 全仓 grep 确认仅 `chatService.ts` 一处 invoke |
| `ChatStreamSpawn` 混合借用与所有权 | 编译期借用检查报错 | `spawn_chat_stream` 是同步函数，结构体入口即解构、不跨 await；风险仅限实现细节，不影响设计 |
| `tauriInvoke` 日志丢失 `args` 内字段名 | 排查变慢 | A2 保留 `conversationId` 顶层 key；必要时按 §7.3 加 catch 日志（需新增 logger 导入，默认不做） |

---

## 12. 验收

- `send_chat_message` 签名为 3 个参数（`app` + `conversation_id` + `args`）。
- 新增一个发送字段时，只需改 `SendChatMessageArgsDto` 与 `SendChatMessageArgs` 两处。
- `spawn_chat_stream` 无 `#[allow(clippy::too_many_arguments)]`，参数数为 1。
- `insert_chat_messages` 参数数为 2。
- `protocol/dto/mod.rs` 的 3 个反序列化用例通过。
- 命令数量不变：`ipc.rs` 的 `generate_handler!` 列表无增删。
- `src-tauri` 目录执行通过：

```
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
```

- 仓库根目录执行通过：

```
npm run typecheck
npm run lint
npm test
```

- 手测覆盖：正常发送 / 分支点重生成 / 无 mode / reasoning 覆盖 / 省略可选字段 / 取消流 / provider 缺失报错。

---

## 13. 待定问题

| # | 问题 | 倾向 |
|---|---|---|
| 1 | A1（全量 struct）还是 A2（保留 `conversationId` 顶层） | **A2**：与 `get_conversation` / `patch_conversation` 一致，且保住日志 key（§6） |
| 2 | struct 名用 `SendChatMessageArgsDto` 还是 `SendChatMessageRequestDto` | `*Dto` 是 `protocol/dto` 的既有后缀（`ConversationPatchDto` / `ConversationQueryOptsDto`），`Args` + `Dto` 语义略重叠。倾向 `SendChatMessageArgsDto`：前端也叫 `args`，两侧同名更好检索 |
| 3 | 是否给 `InsertChatInput` 加 `#[non_exhaustive]` | 不加：内部结构体，加属性只会让构造点变啰嗦 |
| 4 | 是否连 `create_branch` 一起收 | 不一起：`create_branch` 无对应 DTO，改动面独立，单独提交更易回滚 |
| 5 | `docs/store/REFACTOR-SPLIT-PLAN.md` 是否需要登记 | 不需要：该文件记录"文件拆分"，I28 是参数收敛，不产生新文件 |
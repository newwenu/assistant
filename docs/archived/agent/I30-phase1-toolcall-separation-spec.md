# I30 — 阶段一：ToolCall 从 Chat 构建剥离（已归档）

| 项 | 值 |
|---|---|
| 状态 | 已归档（A 段规格；B 段已迁出至 I38b，进度见 [roadmap](../../store/I38-agent-roadmap.md) §2） |
| 日期 | 2026-11-02（2026-11-04 归档修订） |
| 关联 | [I38 roadmap](../../store/I38-agent-roadmap.md)（P0/P4）、[I38b](./I38b-agent-loop-spec.md)（B 段去向）、[I18 spec](./I18-agent-tool-architecture-spec.md)、[I18b](./I18b-tool-unified-bridge-spec.md)、[I31](./I31-phase1-tool-framework-spec.md) |
| 影响范围 | `assistant-agent`、`assistant-providers`、`assistant-storage`、`src-tauri/src/cmd/chat`、`src-tauri/src/protocol`、前端 chat 域 |

本文拆为两段，**执行节奏不同**：

| 段 | 内容 | 处置 | 可否独立执行 |
|---|---|---|---|
| **A 剥离** | 把 tool call 从 chat 构建路径删干净（§5） | 已归档（本规格不再维护其状态） | 是，零行为变化 |
| **B 设计** | 新建 `agent/` 落点与累积器契约 | → [I38b](./I38b-agent-loop-spec.md) | P4 开工时执行 |

**B 段去向**：原 §6 的 D1-D10 决策表已迁入 [I38b §3.2](./I38b-agent-loop-spec.md)，落点统一为 `assistant-agent/src/agent/tool_call.rs`。B 段推迟的原始理由（无调用方、契约未定型、删除后仍可从 git 历史取回）保留在 git 历史中，不再作为实施依据。

本文**不实施**控制层（I38a），**不注入**工具给 LLM；Agent Loop 的落点与契约见 I38b（B 段）。

---

## 1 本文职责

1. **判定边界**：哪些代码属于 chat 构建，哪些属于 agentloop。
2. **剥离清单**：逐文件、逐符号，标注「删除 / 保留」。
3. **设计待办**：B 段只登记待定决策与解冻条件，不写代码形态。

不在本文范围：

- 控制层实现（Permission Policy / Gateway / Pipeline）→ I38a（P2）
- RoundExecutor 与多迭代闭环 → I38b（P4）
- OS 工具逐个实现与共享路径层 → I38c / I31
- ToolBox 桥接补全（`workspace_root` 来源、超时、错误通道）→ I18b
- 前端 ToolCallBubble 五态与审批 UI → I38d（P6）

---

## 2 术语

| 术语 | 定义 |
|---|---|
| chat 路径 | `send_chat_message` → provider adapter 流 → `chat_runtime::consume_stream` → `cmd/chat` 落库与发事件 → 前端 `MessageBubble` 渲染 |
| agentloop 路径 | 「LLM 返回 tool_call → 执行 → 回填 → 继续生成」的闭环，即 I38b（P4）的 RoundExecutor |
| tool call 四件套 | 产生（adapter 解析）、累积（流式拼装）、落库（消息持久化）、渲染（前端卡片） |
| chat 线格式 | `ChatDeltaPayloadDto` / `MessageDto` 等跨进程传输的 DTO |
| 落盘格式 | `MessageRow` 与 `messages` 表列形状（storage 层） |

---

## 3 剥离判据

只有一条规则：

> **产出物是否会进入 chat 消息的落盘格式或线格式？**

- 会 → 属于 chat 构建。但 chat 消息只允许承载 `content` / `reasoning` / `usage` / `finish_reason`。
- 不会 → 属于 agentloop，必须移出 chat 构建。

按此规则，tool call 四件套**全部**不属于 chat：chat 请求体从不声明 `tools`，模型不会返回 tool_call，落盘列恒为空，前端卡片永不显示。

**判据的反向用法**：将来若有人想在 chat 路径里「顺便支持一下工具调用」，本条直接否决——需要工具调用就走 agentloop 入口，不走 chat 入口。

**判据的边界**：判据只约束「chat 构建」。provider 层的**解析能力**（`UnifiedResponse.tool_call`）、请求侧消息形状（`ChatMessage.tool_calls`）、DB 列形状（`MessageRow.tool_calls_json`）都不属于 chat 构建，见 §5.6 白名单。

### 3.1 判据必须按「层级」施加（关键修正）

同一个字段名，落在不同层级，性质完全不同。初稿把判据施加在**类型名**上而不是**层级**上，导致误删底层能力：

| 类型 | 所在层级 | 是否跨进程 | 判据结论 |
|---|---|---|---|
| `ChatDelta` | **provider 内部流类型**（`ChatStream` 的 Item） | 否（先由 `emit_delta` 转 DTO） | **不属于 chat 构建 → 不删** |
| `ChatDeltaPayloadDto` | chat 线格式 | 是 | 属于 chat → 删 |
| `MessageDto.tool_calls_json` | chat 线格式 | 是 | 属于 chat → 删 |
| `MessageRow.tool_calls_json` | 落盘格式 | 否 | 不属于 chat → 不删 |
| agent 侧 `ToolCallAccumulator` | chat 构建内的有状态累积 | 否 | 与 providers 重复 → 删 |
| providers 侧 `ToolCallAccumulator` | provider 内部（越界但唯一实现） | 否 | 唯一正确实现 → **暂留**（P5） |

**推论**：`ChatDelta` 的 tool 三字段是 provider 表达「看到了 tool 片段」的唯一通道，删掉等于切断 agentloop 的输入源。**「能力」与「污染」的区分标准是：删掉之后，P4 是否需要重写底层**——需要，就说明删的是能力。

### 3.2 provider 层职责边界

| 职责 | 归属 provider | 说明 |
|---|---|---|
| 协议适配（JSON/SSE → `UnifiedResponse`） | 是 | 无状态 |
| 请求构造（`ChatMessage` → wire format） | 是 | 无状态 |
| 语义归一化（`finish_reason` → `ToolCallCompletion`、usage 换算） | 是 | 无状态 |
| 流式事件化（chunk → 语义单元） | 是 | 无状态 |
| 传输韧性（重试 / 超时 / 取消 / 错误分类） | 是 | 属传输层，非语义 |
| **跨 chunk 累积 tool call** | **否** | 需要边界状态，属 agentloop |
| **工具执行 / 多轮编排 / 持久化** | **否** | 属 agentloop |

判据：**provider 输出「片段」，agentloop 输出「完整对象」**。谁产出完整 tool call 对象，谁就越界。

当前越界实况（阶段一不修，登记待 B 段处理）：

| # | 越界 | 位置 |
|---|---|---|
| F15 | Gemini adapter 内持有 `ToolCallAccumulator`，并把累积结果重写回 `unified.tool_call`，使 `arguments_is_snapshot: false` 与「实际是快照」自相矛盾 | `src-tauri/crates/assistant-providers/src/gemini/request.rs` |
| F16 | 三个 adapter 的 tool 参数语义不一致：OpenAI / Anthropic 透传增量片段，Gemini 输出累积快照 | 同上 + `openai/stream.rs`、`anthropic/stream.rs` |
| F17 | `ChatDelta` 缺少快照标记字段，下游无法区分「增量」与「快照」 | `src-tauri/crates/assistant-providers/src/types/chat_delta.rs` |
| F18 | 工具声明通道未建：`ChatOptions` 无 `tools` 字段；`gemini::convert_tools` 零调用方；openai/anthropic 无 tools 注入 | `types/config.rs`、`gemini/message_converter.rs` |

---

## 4 现状事实

以下均为当前源码实测，符号名为准（行号仅供定位，会随改动漂移）。

### 4.1 两套累积器

| 维度 | agent 侧（`chat_runtime.rs`） | providers 侧（`tool_call_accumulator.rs`） |
|---|---|---|
| 存储 | `Vec<PartialToolCall>`，只保留末尾一个 | `BTreeMap<ToolCallStreamKey, PendingToolCall>` |
| 键 | 无 index，以 `delta_tool_call_id` 为边界 | `ToolCallStreamKey::{Indexed(usize), Unindexed}` |
| 边界枚举 | 无（隐式「新 id 即新工具」） | `ToolCallBoundary::{NewTool, FinishReason, StreamEnd}` |
| 输出 | `Vec<ToolCall>`（3 字段） | `FinalizedToolCall`（6 字段，含 `arguments: Value` / `is_error` / `parse_error`） |
| 快照语义 | 无（一律 append） | 支持 `arguments_is_snapshot`（Gemini 用） |
| 非法 JSON | `ToolCallAccumulateError::InvalidJson` 整批失败 | 先 `repair_truncated_json`，失败则 `is_error = true` 单条降级 |
| 截断 | `Truncated` 错误 | 无显式截断错误 |
| 流式回调 | `apply_delta` 产出 `params_partial` | `apply_delta` 产出 `finalized_previous` + `params_partial` |

### 4.2 其他事实

| # | 事实 | 位置 |
|---|---|---|
| F1 | 两套 `ToolCallAccumulator` 并存，语义不同（见 4.1） | `src-tauri/crates/assistant-agent/src/chat_runtime.rs`、`src-tauri/crates/assistant-providers/src/tool_call_accumulator.rs` |
| F2 | chat 流协议预埋了 tool call 通道：`unified_responses_into_chat_deltas` 把 `UnifiedResponse.tool_call` 转成带 `delta_tool_call_*` 的 `ChatDelta` | `src-tauri/crates/assistant-providers/src/types/chat_delta.rs` |
| F3 | Gemini 是唯一真实发射方：adapter 内累积 index 型 tool call，`finalize_all` 后转 `ChatDelta` 产出 | `src-tauri/crates/assistant-providers/src/gemini/request.rs` |
| F4 | 落库路径已接通：`outcome.tool_calls` 非空时序列化成 `tool_calls_json`，随 `DbWriteMsg` 写入 `update_message_content` | `src-tauri/src/cmd/chat/mod.rs`、`src-tauri/src/cmd/chat/stream.rs`、`src-tauri/crates/assistant-storage/src/conversations/msg_ops.rs` |
| F5 | 线格式泄漏 DB 列形状：`MessageDto.tool_calls_json` 是「待二次解析的字符串」；`ChatDeltaPayloadDto` 有 `delta_tool_call_*` 三字段 | `src-tauri/src/protocol/dto/chat.rs` |
| F6 | 前端只有一条写入路径：`appendToolCallDelta` 从流式 delta 累积；`MessageDto → Message` 映射**不**读 `toolCallsJson`，故历史消息的 `toolCalls` 恒为空 | `src/stores/chatStorePorts.ts`、`src/chat/core/chatTypes.ts` |
| F7 | 全链路不可达：chat 请求体不带 `tools`（`ChatRequest` 无该字段），`outcome.tool_calls` 恒 `None`，DB 列恒 `NULL`；已由 DTO 审计确认现网无缺陷 | `docs/archived/infra/dto-frontend-backend-sync-audit.md` D1 |
| F8 | agent 侧 `StreamAccumulator::apply_delta` 的返回值在 `consume_stream` 里被丢弃（`let _outcome`），`ApplyDeltaOutcome` 的唯一消费方是测试 | `chat_runtime.rs` `consume_stream` |
| F9 | agent 侧 `finish_with_truncation` 零调用方（现存死代码） | `chat_runtime.rs` |
| F10 | `update_message_content` 有 **3 个**调用点：`cmd/chat/stream.rs`、`cmd/compaction.rs` 两处 | `src-tauri/src` |
| F11 | `MessageRow.tool_calls_json` 在 storage 层有 **7 处**形状引用（建表 / 列清单 / 行读取 / 插入 / 分支 / 复制对话两处），均属落盘格式 | `src-tauri/crates/assistant-storage/src` |
| F12 | 请求侧 `ChatMessage.tool_calls` / `tool_call_id` 已存在，但当前无人填充（属 agentloop 回填用的形状，不是 chat 通道） | `src-tauri/crates/assistant-providers/src/types/message.rs` |
| F13 | `gemini/message_converter.rs::convert_tools` 零调用方（现存死代码，不在本文范围） | `src-tauri/crates/assistant-providers/src/gemini/message_converter.rs` |
| F14 | mascot 的 `hadToolCallInRecentTurn` 依赖 `m.toolCalls`；删掉后只剩 `m.role === "tool"`，而 chat 路径从不产生 `tool` 角色消息，该判定将恒为 false，`expressionPolicy` 中两条 `requiresToolCall` 规则转为休眠 | `src/mascot/useMascotChatSync.ts`、`src/mascot/core/expressionPolicy.ts` |

**结论**：删除是**零行为变化**的（整条链路是 dead path），但结构本身是错的——它把半套 agent 能力焊在了 chat 上，并且已经产生了重复实现（F1）。这不会随时间自愈，只会随 P4 落地变成第三套累积器。

---

## 5 A 段：剥离清单

### 5.1 Rust — `assistant-agent`

文件：`src-tauri/crates/assistant-agent/src/chat_runtime.rs`

| 符号 | 动作 |
|---|---|
| `ToolCallAccumulator`（struct + `impl` + `Default`） | 删除 |
| `PartialToolCall` | 删除 |
| `ToolCallParamsChunk` | 删除 |
| `ToolCallDeltaOutcome` | 删除 |
| `ToolCallAccumulateError` | 删除 |
| `ApplyDeltaOutcome`（**整个类型**，不只是两个字段） | 删除；`StreamAccumulator::apply_delta` 返回值改为 `()` |
| `StreamAccumulator::tool_calls` 字段及其初始化 | 删除 |
| `StreamAccumulator::apply_delta` 内的 tool call 分支（含 `has_effective_output` 的 tool 判定） | 删除，只保留文本/推理/usage/finish 累积 |
| `StreamOutcome::tool_calls` | 删除 |
| `StreamAccumulator::into_outcome` 内 tool call 结果处理 | 删除 |
| `ToolCallAccumulator::finish_with_truncation` | 删除（零调用方，F9） |
| `self_mark_effective` 私有 helper | 删除 |
| `use assistant_providers::base::ToolCall` | 删除（保留 `ChatDelta` / `ProviderError` / `TokenUsage`） |

保留：`StreamOutcome` 的 usage 七字段、`finish_reason`、`attempt_usages`、`has_effective_output`、`partial_recovery_reason`、`resolve_final_tokens` / `resolve_partial_tokens`。

> 说明：`ApplyDeltaOutcome` 之所以整类型删除，是因为 `consume_stream` 已经丢弃它的返回值（F8），保留空壳只会成为下一次「顺手用一下」的入口。

### 5.2 Rust — `assistant-providers`：**完全不动**

**结论：本阶段对 `assistant-providers` 零改动。**

初稿曾计划删除 `ChatDelta` tool 三字段、Gemini 累积逻辑与 `tool_call_accumulator.rs`。经 §3.1 层级判据复核，该方案错误——它删的是**能力**而非**污染**：

1. `ChatDelta` 是 provider **内部流类型**，不跨进程，不属于 chat 线格式；
2. providers 侧累积器是**唯一正确实现**（index 键 + 快照语义 + `repair_truncated_json`），删掉后 P4 必然重写；
3. 删掉后 `ChatStream` 无任何 tool 表达力，agentloop 拿不到输入，必须改 trait——那是 B 段的大改。

因此 §5.2 由「删除清单」改为「预留清单」：

| # | 预留项 | 现状 | 阶段一处置 |
|---|---|---|---|
| P1 | `UnifiedResponse.tool_call` / `UnifiedToolCall` / `tool_call_index` | 有 | 保留；`gemini_response_to_unified` 的 `pub(crate)` 可见性待 B 段评估是否放开 |
| P2 | `arguments_is_snapshot` | 有，Gemini 用法有误（F15） | 保留；语义修正移入 B 段（D8） |
| P3 | `ChatDelta.delta_tool_call_id` / `_name` / `_arguments` | 有 | **保留**（provider 内部流类型） |
| P4 | `ChatDelta` 缺快照标记（F17） | 缺 | 阶段一不补，登记 B 段（D9） |
| P5 | `providers::tool_call_accumulator`（`ToolCallAccumulator` / `ToolCallBoundary` / `ToolCallStreamKey` / `FinalizedToolCall` / `repair_truncated_json`） | 有，Gemini 用 | **保留**；上移 agentloop 移入 B 段（D8） |
| P6 | `ChatOptions.tools`（F18） | 缺 | 阶段一不建，B 段新增 |
| P7 | `ChatMessage.tool_calls` / `tool_call_id` | 有 | 保留（回填用） |
| P8 | `ToolCallCompletion` / `finish_reason` 映射 | 有 | 保留 |

**副作用登记**：由于 providers 仍会产出 tool `ChatDelta`，但阶段一删除了 chat 落库与前端渲染，这些 delta 会被 `consume_stream` 忽略（agent 侧累积器已删）。当前请求不带 `tools`，Gemini 也不会返回 functionCall，故无现网影响（R3）。

> 这条修正是本文最重要的变更：它把「剥离」从「切能力」收窄为「删重复 + 删污染」。

### 5.3 Rust — `src-tauri/src/cmd/chat` + `assistant-storage`

| 文件 | 符号 | 动作 |
|---|---|---|
| `cmd/chat/stream.rs` | `DbWriteMsg.tool_calls_json` 字段 | 删除 |
| `cmd/chat/stream.rs` | `save_partial_on_error` 中该字段赋值、`spawn_db_writer` 中该字段传参 | 删除 |
| `cmd/chat/mod.rs` | `let tool_calls_json = outcome.tool_calls...` 整段构造、`DbWriteMsg { tool_calls_json, .. }` 传参 | 删除 |
| `cmd/chat/insert.rs` | 两处 `tool_calls_json: None` | **保留**（`MessageRow` 字段保留，见下） |
| `cmd/compaction.rs` | `update_message_content` 两处调用点的 `None` 实参 | 删除（随签名收窄） |

`assistant-storage` 侧：

| 文件 | 符号 | 动作 |
|---|---|---|
| `conversations/msg_ops.rs` | `update_message_content` 的 `tool_calls_json: Option<&str>` 参数与 SQL 赋值 | 删除参数，SQL 不再写该列 |
| `conversations/msg_ops.rs` | `insert_message` 中 `row.tool_calls_json` 传参 | **保留**（DB 行形状） |

**数据库列 `tool_calls_json TEXT` 保留不动**。理由：主库迁移规则只允许 `ADD COLUMN`，禁止 `DROP` / `RENAME` / 数据重写。该列转为**预留列**，由 P4 的 agentloop 落库时复用（见 R2）。

> R2 的执行方式修正：SQLite 没有列注释，无法通过 `ALTER` 加注释。预留说明只能写在 `assistant-storage/src/db.rs` 建表语句旁的代码注释中，并在本文登记。

**修正点**：初稿写「`insert.rs` 两处 `tool_calls_json: None` 随字段移除」，与 §5.4「`MessageRow.tool_calls_json` 保留」矛盾——字段保留就必须保留字面量赋值。

### 5.4 Rust — `src-tauri/src/protocol`

| 文件 | 符号 | 动作 |
|---|---|---|
| `protocol/dto/chat.rs` | `MessageDto.tool_calls_json` 字段、`From<MessageRow> for MessageDto` 中该字段赋值 | 删除 |
| `protocol/dto/chat.rs` | `ChatDeltaPayloadDto.delta_tool_call_id` / `_name` / `_arguments` | 删除 |
| `protocol/dto/mod.rs` | 行内单测中 `tool_calls_json: None` 与 `delta_tool_call_*: None` 的构造点（初稿遗漏） | 删除 |
| `src-tauri/tests/protocol_contract.rs` | `sample_message_row` 的 `tool_calls_json: None`、`sample_chat_delta_dto` 的三字段（两处构造） | 删除 |

`MessageRow.tool_calls_json` **保留**：它是 DB 行形状（storage 层类型），不是线格式。线格式与落盘格式的边界按 DTO 审计 §5 执行。

### 5.5 前端

| 文件 | 符号 | 动作 |
|---|---|---|
| `src/chat/core/chatTypes.ts` | `Message.toolCalls?`、`ToolCall` interface、`ChatDeltaPayload.deltaToolCallId/Name/Arguments` | 删除 |
| `src/stores/chatStorePorts.ts` | `appendToolCallDelta` 整个方法 | 删除 |
| `src/services/chatStreamEvents.ts` | 三字段透传、「非文本 payload」判定条件中的三字段 | 删除 |
| `src/services/chatOrchestrator.ts` | deps 类型声明中的 `appendToolCallDelta`、`handleDeltaEvent` 内的三字段透传块 | 删除 |
| `src/chat/message/MessageBubble.tsx` | `ToolCall` 类型 import、`ToolCallBlock` import、`hasToolCalls`、渲染块、memo 比较器中的 `prev/next.message.toolCalls` 段 | 删除 |
| `src/chat/blocks/ToolCallBlock.tsx` + `.module.css` | 整个组件 | 删除（唯一使用方是 `MessageBubble`） |
| `src/mascot/useMascotChatSync.ts` | `m.toolCalls?.length` 判定条件 | 删除该子条件，仅保留 `m.role === "tool"` 分支（F14：该分支转为休眠，不删函数） |
| `src/chat/core/__tests__/protocolContract.test.ts` | `toolCalls: undefined` 样本字段与 `toHaveProperty("toolCalls")` 断言 | 删除 |
| `src/services/__tests__/chatOrchestrator.test.ts` | `appendToolCallDelta: vi.fn()` mock（初稿遗漏） | 删除 |

> F14 的处理选择：保留 `hadToolCallInRecentTurn` 与 `requiresToolCall` 规则，只是其输入恒为 false。这样 P6 恢复 tool 消息时无需重建策略；代价是两条表情规则在 P0 到 P6 之间不会触发。若选择连规则一起删，需同步修改 `src/mascot/core/types.ts` 与 `expressionPolicy.ts`，属扩大改动面，不推荐。

### 5.6 必须保留的白名单

A2 的检索会命中以下符号，它们**不在删除范围**：

| 保留项 | 位置 | 理由 |
|---|---|---|
| `UnifiedResponse.tool_call` / `UnifiedToolCall` / `tool_call_index` / `arguments_is_snapshot` | `assistant-providers/src/types/unified.rs` | provider 解析能力，agentloop 输入源 |
| `ChatDelta.delta_tool_call_id` / `_name` / `_arguments` 及 `unified_responses_into_chat_deltas` 的 tool 分支 | `assistant-providers/src/types/chat_delta.rs` | **provider 内部流类型**，非线格式（§3.1，P3） |
| `providers::tool_call_accumulator` 全部符号 | `assistant-providers/src/tool_call_accumulator.rs` | 唯一正确实现，暂留待 B 段上移（P5，D8） |
| `ToolCallCompletion` 及 `"tool_calls"` finish_reason 映射 | `assistant-providers/src/types/unified.rs`、`openai/stream.rs`、`gemini/stream.rs` | 属于 `finish_reason` 语义，不是 tool 通道 |
| `ChatMessage.tool_calls` / `tool_call_id` | `assistant-providers/src/types/message.rs` | 请求侧消息形状，回填用 |
| `openai/message_converter.rs` 的 `tool_calls` 序列化 | `assistant-providers/src/openai/message_converter.rs` | 请求侧消息转换 |
| `openai/types.rs` 的 `ChatToolCallDelta` | `assistant-providers/src/openai/types.rs` | provider SSE 解析形状 |
| `MessageRow.tool_calls_json` + `messages` 表列 + `MSG_SELECT` / `read_msg_row` / `insert_message` / `branch.rs` / `ops.rs` 两处 / `db.rs` 建表 | `assistant-storage/src` | 落盘格式（F11，共 7 处） |
| `assistant-providers/src/types/tool.rs::ToolCall` | `assistant-providers/src/types/tool.rs` | 请求侧通用工具调用值类型 |

范围外（不在本文处理，仅登记）：`gemini/message_converter.rs::convert_tools` 为零调用方死代码（F13）；Gemini 累积越界与 adapter 语义不一致（F15/F16）；`ChatDelta` 缺快照标记（F17）；`ChatOptions` 无 `tools`（F18）。

### 5.7 实施顺序

| 阶段 | 内容 | 依赖 |
|---|---|---|
| T1 | 剥离 `assistant-agent`（`StreamAccumulator` / `StreamOutcome` / agent 侧累积器） | 无 |
| T2 | 剥离 `cmd/chat` + `assistant-storage` 参数 + `protocol/dto` | T1 |
| T3 | 剥离前端（types / store / services / MessageBubble / ToolCallBlock / 测试） | T2 |
| T4 | 全量验证：A1-A8 | T3 |

`assistant-providers` **不在实施顺序内**（§5.2 零改动）。

---

## 6 B 段：已迁出

B 段（累积器落点与 D1-D10 决策）已迁移到 [I38b §3](./I38b-agent-loop-spec.md)，作为 P4 的权威规格：

- 落点：`assistant-agent/src/agent/tool_call.rs`（唯一累积器）。
- 决策：D1-D10 已在 I38b §3.2 定稿；provider 侧 `tool_call_accumulator.rs` 在 D8 完成后删除。
- 落库：复用预留列 `tool_calls_json`；事件走 `agent-tool-call`（I38d）。
- 解冻条件已由 P4 阶段门禁替代（见 [I38 roadmap](../../store/I38-agent-roadmap.md) §6）。

本节不再维护决策表；原文保存在 git 历史中。

---

## 7 验收标准（A 段）

| # | 标准 |
|---|---|
| A1 | `cargo test` 全绿；`StreamOutcome` 不含任何 tool 相关字段 |
| A2 | 白名单（§5.6）之外，全仓检索 `toolCallsJson` / `ToolCallBlock` / `appendToolCallDelta` / `ToolCallDeltaOutcome` / `PartialToolCall` / `ToolCallParamsChunk` / `ToolCallAccumulateError` / `ApplyDeltaOutcome` 命中为 0 |
| A2b | `assistant-providers` 的 git diff 为空（§5.2 零改动） |
| A3 | agent 侧 `ToolCallAccumulator` 命中为 0；providers 侧实现保留且编译通过 |
| A4 | 前端 `npm run typecheck` 通过；`MessageBubble` 不再引用 `ToolCall` 类型 |
| A5 | chat 端到端行为不变：文本流、推理流、usage 七字段、重试、取消、部分保存 |
| A6 | DB 列 `tool_calls_json` 仍存在；除 `insert_message` 写入 `NULL` 外，无任何**非 NULL** 写入路径 |
| A7 | 未新增 `agent_loop` 模块、未新增任何无调用方的空壳代码 |
| A8 | 契约测试（Rust + 前端）随字段删除同步收敛，不保留「只为占位」的断言 |

> A2 已去掉 `delta_tool_call`（providers 内部流类型保留）；新增 A2b 以机器可验的方式约束「providers 零改动」。初稿 A3 写「`ToolCallAccumulator` 全仓为 0」会误杀 providers 侧唯一实现，已收窄为 agent 侧。

---

## 8 与 I18 的关系

阶段一 P0 = I18 Step 0-3 + Step 8，外加本文 A 段定义的「剥离」这一**前置动作**。

B 段去向：`RoundExecutor` 与累积器在 P4（I38b）落地，直接复用预留列 `tool_calls_json` 与 `agent-tool-call` 事件，不再另起第三套。

---

## 9 非目标

- 不给 agent 注入工具（阶段一只到「能力就位、无调用方」）
- 不改 DB schema（只停写，不删列）
- **不动 `assistant-providers` 任何一行**（§5.2 零改动，含 `ChatDelta` tool 三字段、Gemini 累积、`tool_call_accumulator.rs`）
- 不动 `UnifiedResponse.tool_call` / `UnifiedToolCall`（provider 解析能力保留）
- 不新建 `agent/` 模块、不实现 `run_agent_loop()` 入口（B 段已移至 I38b，P4 执行）
- 不做前端 ToolCallBubble、五态、审批 UI（I38d / P6）
- 不修 provider 越界（F15/F16）、不补快照标记（F17）、不建 tools 通道（F18）——全部移入 I38b（P4）
- 不处理 `convert_tools` 死代码（F13，另记）

---

## 10 风险

| # | 风险 | 说明 | 缓解 |
|---|---|---|---|
| R1 | 误删 provider 解析能力 | `UnifiedResponse.tool_call` / `ChatDelta` tool 三字段 / providers 累积器是 agentloop 未来的输入与参考实现 | §5.2 改为零改动；§3.1 层级判据；§5.6 白名单；A2b 用 git diff 机器验证 |
| R2 | DB 列成为孤儿 | `tool_calls_json` 列保留但无人写非 NULL | 在 `db.rs` 建表语句旁加代码注释「预留：agentloop 落库用」（SQLite 无列注释）；不删列以避免迁移违规 |
| R3 | tool delta 无人消费 | 删掉 agent 侧累积器后，providers 仍可能产出 tool `ChatDelta`，`consume_stream` 会忽略 | 属预期。当前请求不带 `tools`，模型不会返回 tool call，无现网影响 |
| R4 | 前端历史消息 | `MessageDto → Message` 本就不映射 `toolCallsJson` | 删除后历史渲染与现状完全一致 |
| R5 | 未来误加回 chat | 有人为了「顺手支持工具」又把字段塞回 chat 路径 | §3 判据写入本文；code review 以此为准 |
| R6 | P4 重造累积器 | D1-D10 被忽略，直接照抄旧实现 | I38b §3.2 决策表 + P4 门禁；providers 侧实现保留在原处可直接参考 |
| R7 | mascot 表情规则休眠 | F14：两条 `requiresToolCall` 规则在 P0 到 P6 间不触发 | 明确记录为预期行为，P6（I38d）恢复 |
| R8 | 快照/增量语义仍混乱 | F17：`ChatDelta` 无快照标记，Gemini 又用错 `arguments_is_snapshot` | 阶段一不修（零调用方，无现网影响）；D8/D9 已在 I38b §3.2 定稿 |

---

## 11 修订记录

| # | 修订 |
|---|---|
| 1 | 拆为 A 剥离（可执行）/ B 设计（推迟），并给出推迟依据与解冻条件 |
| 2 | 修正 `insert.rs` 两处 `tool_calls_json: None`：由「删除」改为「保留」 |
| 3 | 补齐 `update_message_content` 的第三个调用点（`cmd/compaction.rs` 两处） |
| 4 | 修正 A2 / A6（原 A7）：白名单口径、非 NULL 写入口径 |
| 5 | 补齐测试删除项：`chat_runtime_spec.rs`、`chat_delta.rs` 行内单测、`protocol_contract.rs`、`protocol/dto/mod.rs` 行内单测、`chatOrchestrator.test.ts` |
| 6 | 新增 §5.6 白名单（7 类保留项） |
| 7 | 新增 F8-F14（返回值丢弃、死代码、调用点数、落盘引用数、请求侧形状、mascot 依赖） |
| 8 | 删除初稿 §6.2 的具体 Rust 契约，改为 D1-D7 决策表 |
| 9 | 实施顺序调整：取消「先建新家」，T1/T2 可并行 |
| 10 | **§5.2 由「删除清单」改为「零改动 + 预留清单」**：`ChatDelta` tool 三字段、Gemini 累积、`tool_call_accumulator.rs` 全部保留 |
| 11 | 新增 §3.1 层级判据与 §3.2 provider 职责边界（能力 vs 污染的区分标准） |
| 12 | 新增 F15-F18（Gemini 越界、adapter 语义不一致、缺快照标记、tools 通道未建） |
| 13 | 新增 P1-P8 预留清单；新增 D8/D9/D10；A2 去掉 `delta_tool_call`、A3 收窄到 agent 侧、新增 A2b |
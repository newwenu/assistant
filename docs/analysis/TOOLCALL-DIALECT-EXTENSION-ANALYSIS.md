# 工具调用方言扩展性分析

> 日期：2026-11-08
> 关联：`docs/ARCHITECTURE.md` · `docs/analysis/MODEL-CAPABILITY-EVOLUTION-ANALYSIS.md` · I38d-agent-frontend-modes-spec（已归档）
> 范围：provider 流协议 → 统一中间表示 → 累积终结 → 落库 → DTO 投影 → 前端视图
> 性质：**前瞻分析**。讨论 tool call 表达形态的结构边界与扩展成本，不描述实施方案，不跟踪代码进度。
> 阅读约定：结论部分（§0、§3–§6）讨论的是结构，不随代码演进失效；基线快照（§2）是某一时刻的结构证据，代码演进后**不再更新**，不构成待办清单。

---

## 0 结论

1. 当前统一中间表示只覆盖**原生结构化**这一种方言。它的形状隐含了七个假设（§3），其中四个是**语法型**（击穿结构），三个是**词汇型**（加字段即可）。
2. 四个语法型假设中，最关键的**不是**「参数不是 JSON」，而是「**参数形态没有方言标记**」。原始文本在落库时被完整保留，却没有任何字段说明它是 JSON、位置式还是代码式，导致下游只能靠「试着解析成 JSON」来猜。
3. 真正的断点不在 provider 层，而在**四个「raw 原文 → Value」的重新解析边界**：DTO 投影、Gemini 出站重放、Anthropic 出站重放、前端预览。四处都是静默兜底（`Value::Null` / `json!({})` / `undefined`），**不报错、不记日志、不区分「空参数」与「参数无法解析」**——这是当前链路唯一无法靠日志发现的失败模式，且其中一处已被写成测试期望行为。
4. 新增方言的成本分布极不均衡：提示层、请求层、流解析层全部是**加实现**；只有统一中间层、落库约定、DTO 契约、前端视图需要**改结构**。这决定了推进顺序——先做能完整落在「加实现」里的方言。
5. 参考项目里唯一把「多方言」做成体系的是 openhuman（6 种方言、单一 parser 顺序回退、`auto` 降级链）。它的关键经验是**方言要随转录持久化**，否则会话恢复时无法重放。
6. 取舍：**先做 XML-in-tag（不动结构），再评估代码式（动结构）**。语法约束 freeform 与外挂解释器不建议引入——前者绑定特定 provider 的解码能力，后者每次调用多一次推理。

---

## 1 核心命题：方言差异的三种性质

「支持更多工具调用格式」这句话本身是含混的。实际存在三个互不相同的层次，扩展成本差一个数量级。

| 性质 | 含义 | 典型差异 | 扩展成本 |
| --- | --- | --- | --- |
| **词汇型** | 同一个槽位，取值集合变多 | 新增 `index` 语义、新增 `name` 命名空间、新增快照/增量标记 | 加字段 / 加枚举 |
| **语法型** | 槽位的**语义**改变，不再能装下新内容 | 参数不是 JSON、调用没有 id、解析失败不等于没有调用 | 改结构 |
| **形态型** | 调用不再出现在协议字段里，而是出现在**正文文本**里 | XML-in-tag、P-Format、代码式、DSL | 改结构 + 加解析器 |

形态型是语法型的子集但更极端：它不只改变槽位语义，还改变了「调用从哪里来」。协议层看到的不再是结构化字段，而是一段需要事后解析的自然语言。

**判断标准**：如果新方言能被现有结构「装下」且不丢失信息，是词汇型；如果装得下但语义已偏（比如 `String` 里塞非 JSON），是语法型伪装成词汇型——**这是最危险的一类**，因为它不会报错。

---

## 2 基线快照（2026-11-08）

> 本节记录某一时刻的结构证据。**不跟踪修复进度**；代码演进后以当时代码为准。

### 2.1 链路

```
provider 流协议（tool_use / functionCall / function_call）
   ▼
UnifiedToolCall { tool_call_index, id, name, arguments: Option<String>, arguments_is_snapshot }
ChatDelta { delta_tool_call_id, delta_tool_call_name, delta_tool_call_arguments,
            delta_tool_call_index, delta_tool_call_arguments_is_snapshot }
   ▼
PendingToolCall 累积（index 分组；无 index 视作单槽）
   ▼
FinalizedToolCall { tool_id, tool_name, raw_arguments: String, arguments: Value,
                    is_error, parse_error }
   ▼
tool_calls_json（Vec<ToolCall>，arguments 存 raw_arguments 原文）
   ▼
MessageToolCallDto { call_id, name, arguments: serde_json::Value, status, result }
   ▼
前端 ToolCallView { callId, name, status, arguments, resultText }
```

### 2.2 三种 provider 的承载位置

同样是「JSON 参数」，三个 provider 的序列化位置完全不同，这是方言差异的源头。

| provider | 承载位置 | 参数形态 | 流式语义 |
| --- | --- | --- | --- |
| Anthropic | `content_block` 中的 `tool_use` 块 | `input` 对象；流内为 `input_json_delta` 的 `partial_json` 片段 | 增量拼接 |
| Gemini | `parts` 中的 `functionCall` | `args` 是**已解析的 JSON 对象** | **完整快照**（每帧全量） |
| OpenAI | `function_call` / `tool_calls[]` | `arguments` 是 JSON **字符串** | 增量拼接 |

Gemini 的快照语义已被 `arguments_is_snapshot` 吸收——这是唯一一处「方言差异被单点结构吸收」的成功先例，也是 §3.2 反模式的起因。

### 2.3 已吸收的差异

| 差异 | 吸收方式 | 位置 |
| --- | --- | --- |
| 增量 vs 快照 | `arguments_is_snapshot` 布尔，累积器分流 `append` / `replace` | `src-tauri/crates/assistant-providers/src/types/chat_delta.rs`、`src-tauri/crates/assistant-agent/src/agent/tool_call.rs` |
| 有 index vs 无 index | `ToolCallStreamKey::Indexed / Unindexed` | `src-tauri/crates/assistant-agent/src/agent/tool_call.rs` |
| JSON 截断 | `repair_truncated_json` 补全未闭合字符串与括号 | `src-tauri/crates/assistant-agent/src/agent/tool_call.rs` |

### 2.4 关键结构事实：raw 原文保到了 DB，却在读取与重放处静默降级

这是本文最重要的发现，需要单独列出。

**好的部分**：`FinalizedToolCall` 同时持有 `raw_arguments: String` 与 `arguments: Value`，落库时 `tool_calls_json` 写入的是 **raw 原文**（`src-tauri/crates/assistant-agent/src/agent/loop_executor/budget.rs` 的 `tool_calls_json`）。也就是说，**数据库里存的是模型真正写出的字节**，没有经过 JSON 归一化。

**断掉的部分**：raw 原文在**四个边界**上被重新解析成 `Value`，且每一处都是静默兜底：

| # | 边界 | 位置 | 非 JSON 原文的结果 |
| --- | --- | --- | --- |
| 1 | 读取投影 | `src-tauri/src/cmd/conversation_utils.rs` 的 `message_tool_calls` | `unwrap_or(Value::Null)` |
| 2 | 出站重放（Gemini） | `src-tauri/crates/assistant-providers/src/gemini/message_converter.rs` 的 `parse_function_args` | `json!({})` |
| 3 | 出站重放（Anthropic） | `src-tauri/crates/assistant-providers/src/anthropic/message_converter.rs` 的 `parse_tool_input` | `json!({})` |
| 4 | 前端预览 | `src/chat/core/toolCallView.ts` 的 `parseArgs` | `undefined` |

三处后端解析的形状完全一致：

```rust
match serde_json::from_str::<Value>(arguments) {
    Ok(Value::Object(map)) => Value::Object(map),
    _ => json!({}),
}
```

共同特征：**不记日志、不返回错误、不区分「空参数」与「参数无法解析」**。最终表现是「这次调用没有参数」，而不是「这次调用的参数无法展示」。

更值得注意的是 Anthropic 的测试里已有 `arguments: "{not json"` 的用例（`message_converter.rs`），即**「解析失败 → 空对象」被写成了期望行为**。这说明静默兜底不是疏漏，而是被固化的约定——引入文本方言时必须同时推翻这些测试断言，成本比想象中高。

**推论**：如果现在引入任一种非 JSON 方言，**写入侧会成功、读取侧与重放侧会三重静默丢失**。这比「完全不支持」更糟——因为它通过了写入测试，故障只在重启会话后才暴露，且重放给模型的历史会被改写成空参数，导致模型基于错误历史继续推理。

**另一个结构性依赖**：工具结果配对依赖 `call_id` 唯一性（`results.get(&call.id)`）。无 id 的方言必须先合成稳定 id，否则结果无法与调用对齐，状态会全部落到 `Cancelled`。

---

## 3 结构命题：统一层的隐含假设

`UnifiedToolCall` 只承载四个字段，却隐含了七个前提。逐一检验哪些会被新方言打破。

| # | 隐含假设 | 打破者 | 性质 | 现有结构能否吸收 |
| --- | --- | --- | --- | --- |
| A1 | 每次调用有唯一 `id` | P-Format、`$ command`、代码式 | **语法型** | 否。需合成 id，且要保证跨重启稳定 |
| A2 | `name` 是扁平字符串 | 代码式的命名空间（`developer__shell`）、子代理嵌套 | 词汇型 | 勉强。需约定拼接/转义规则 |
| A3 | `arguments` 是 **JSON 对象**的序列化 | P-Format 位置数组、`apply_patch` 的 DSL 文本、代码式文本 | **语法型** | 形式上能装（`String`），语义上不能 |
| A4 | 参数是**增量拼接**的片段 | Gemini 快照语义 | 词汇型 | 是（已由 `arguments_is_snapshot` 吸收） |
| A5 | 一次响应内多调用按 `index` 分组 | code mode 的并发/嵌套调用 | 词汇型 | 部分。扁平序号无法表达父子关系 |
| A6 | 解析失败 = 没有调用 | 文本方言（部分解析成功、解释文字被误判） | **语法型** | 否。缺「部分有效」的表达 |
| A7 | 参数形态**不需要被记录** | 所有非 JSON 方言 | **语法型** | 否。**无方言标记字段** |

### 3.1 A3 与 A7 的组合才是真问题

单看 A3，可以辩称「`String` 装得下任何文本，问题不大」。但 A7 让 A3 无法补救：

- 没有方言标记，下游无法知道 `"read_file[0|src/main.rs|1|20]"` 该按什么规则解析
- 只能靠「试着按 JSON 解析，失败就当空对象」——正是 §2.4 的静默降级
- 会话恢复（重放历史）时同样无法判断，因为 DB 里只有裸文本

openhuman 对此的处理是显式的：`agent.tool_dispatcher` 作为配置项被**持久化并在会话恢复时复用**（见 `references/1-ai-agent/openhuman/gitbooks/developing/architecture/agent-harness.md` 的 "Tool dispatch and tool-call dialects" 一节，明确写 "Persisted session histories can contain suffixes in any of these shapes, so the session shell keeps the dispatcher around to parse and replay them faithfully when a transcript is resumed"）。它把「方言」当成**会话级上下文**，而不是一次性解析参数。

### 3.2 布尔累积反模式

`arguments_is_snapshot` 的存在证明「用布尔吸收方言差异」可行，但也划出了危险线：

- 每遇到一种新差异就加一个布尔（`is_snapshot`、`is_positional`、`has_no_id`…），最终得到**布尔矩阵**：组合数爆炸且互斥关系无法表达
- 布尔无法承载「互斥的形态」——JSON 与 P-Format 不会同时成立，但两个 `bool` 允许同时为 `true`
- 布尔无法携带「解析依据」——`true` 只说明「是快照」，不说明「按什么语法解析」

**替代方向**：把「参数形态」做成**枚举**（`ArgumentsShape::{Json, Positional, Code, Freeform}`），布尔退化为枚举的一个变体。这样：
- 互斥性由类型系统保证
- 未知形态可以显式表达（避免 A6/A7 的静默）
- 新增方言是加变体，不是加布尔

代价是这个枚举要穿透 wire、落库、DTO、前端四层，属于**不可逆决策**（§5.2）。

---

## 4 方言能力矩阵

按**表达形态**分类，provider / 项目名只作为实例出现在证据里。

### 4.1 原生结构化

协议层有独立的调用字段，参数是 JSON。

| 实例 | 形态 |
| --- | --- |
| Anthropic | `content_block` type=`tool_use`，`input` 对象，流内 `input_json_delta` |
| Gemini | `functionCall` + `args` 对象（快照） |
| OpenAI Chat | `tool_calls[]` + `arguments` 字符串（增量） |
| OpenAI Responses | `function_call` item + `call_id` |

**特征**：id 天然存在、name 扁平、参数 JSON、增量或快照二选一。**当前结构完整覆盖这一格**。

### 4.2 文本内 JSON（XML-in-tag）

调用写在正文里，用标签包裹，标签内是 JSON。

```text
<tool_call>{"name":"read_file","arguments":{"path":"src/main.rs"}}</tool_call>
```

| 证据 | 位置 |
| --- | --- |
| openhuman `xml` 方言 | `references/1-ai-agent/openhuman/crates/openhuman-core/src/config/ops/tool_dispatcher.rs` |
| goose 剥壳解析 | `references/1-ai-agent/goose/crates/goose-provider-types/src/json.rs` 的 `parse_tool_arguments` |

**成本评估**：这是**最划算的一格**。参数仍是 JSON，A3 不被打破；缺的只是 A1（无 id）与 A7（形态标记）。若接受「文本方言一律合成 id + 约定成 JSON」，可**不动结构**落地。

goose 在此格还暴露了两个真实世界的容错需求：
- `unwrap_double_encoded_object`：模型把 JSON 再序列化成字符串（双重编码）
- `strip_json_wrapper`：模型多包了一层标签

这两条说明**解析层必须容忍「多包一层」**。而当前 `serde_json::from_str` 严格解析、`repair_truncated_json` 只补括号，都无法处理双重编码。

### 4.3 位置式（P-Format）

参数用位置值而非键值，极省 token。

```text
<tool_call>read_file[0|src/main.rs|1|20]</tool_call>
```

prompt 侧给签名 `read_file[0|<path>|1|<limit>]`，模型只填值。

| 证据 | 位置 |
| --- | --- |
| openhuman `pformat` 方言，明确标注 opt-in | 同上 `tool_dispatcher.rs`；语法见 `agent-harness.md` |

**成本评估**：**打破 A3 与 A7**。参数不是 JSON，落库后无法用 `from_str::<Value>` 还原。优势是 token 极省；劣势是参数顺序敏感、可读性差、模型易错位。

### 4.4 代码式

工具目录渲染成函数签名，模型写函数调用。

```python
read_file(path="src/main.rs", limit=20)
```

```typescript
read_file({path: "src/main.rs", limit: 20});
```

| 证据 | 位置 |
| --- | --- |
| openhuman `python` / `typescript`，且明说 `python` 是默认代码方言 | `tool_dispatcher.rs`、`agent-harness.md` |
| codex code mode：工具目录渲染为 TS 声明 | `references/1-ai-agent/codex/codex-rs/tools/src/code_mode_tests.rs` |

**成本评估**：同 P-Format 打破 A3/A7，但**模型先验更强**——代码训练充分的模型写过大量函数调用，误判率低于位置式。openhuman 文档直说 "Compact like P-Format but a syntax small code-trained models already write"。

额外优势：代码式天然支持**多步编排**（循环、`Promise.all`），A5 的嵌套/并发需求在此格才真正出现。

额外风险：代码式文本**与普通正文难以区分**。模型解释「可以调用 `read_file(path=...)`」时会被误判为真实调用。openhuman 承认 "mis-parse on some models"。

### 4.5 语法约束 freeform（DSL）

不靠事后解析，靠**解码期语法约束**。

```json
{ "name": "apply_patch", "type": "grammar", "syntax": "lark", "definition": "start: \"patch\"" }
```

模型输出受 lark 语法约束的自由文本：

```text
*** Begin Patch
*** Update File: path/to/file.py
@@ def example():
-    pass
+    return 123
*** End Patch
```

| 证据 | 位置 |
| --- | --- |
| codex `FreeformToolFormat { type: "grammar", syntax: "lark" }` | `references/1-ai-agent/codex/codex-rs/tools/src/tool_spec_tests.rs` |
| `apply_patch` 指令文本 | `references/1-ai-agent/codex/codex-rs/protocol/src/prompts/base_instructions/default.md` |

**成本评估**：本质是「结构化输出」而非「工具调用」。它把参数变成 DSL，好处是 diff 这类天然非 JSON 的内容不用嵌套转义。但：
- 绑定 provider 的 grammar 解码能力，非该 provider 无法复现
- 参数不是键值对，工具侧需要专用解析器
- 与我们「provider 无关的统一中间层」目标冲突

**不建议引入**，除非将来直接对接支持 grammar 的端点并愿意接受绑定。

### 4.6 特殊 token

模型训练时就有的特殊标记，不是普通文本。

```text
functions.shell:0 <|tool_call_argument_begin|> {"command":"ls"}
```

| 证据 | 位置 |
| --- | --- |
| goose 双格式回退：有标记 / 无标记（`name {json}`） | `references/1-ai-agent/goose/crates/goose/src/providers/toolshim.rs` |

**成本评估**：仅特定模型族（DeepSeek 系）原生。参数仍是 JSON，但**解析必须按 token 切分**，与普通文本解析器不同路径。

### 4.7 外挂解释器（tool shim）

起**第二个模型**把主模型输出的任意文本转成结构化调用。

| 证据 | 位置 |
| --- | --- |
| goose tool shim（默认 Ollama `mistral-nemo`） | `references/1-ai-agent/goose/documentation/docs/guides/tool-shim.md` |

**成本评估**：完全不挑模型，但每次工具调用多一次推理。延迟与成本翻倍，且失败模式变成「解释器幻觉」——它可能编造一个模型没写的调用。

**不建议引入**，除非目标模型完全不可控且必须支持。

### 4.8 三态能力矩阵

沿用 MODEL-CAPABILITY-EVOLUTION-ANALYSIS 的判断：「看起来支持、实际半路丢弃」比「完全没有」更危险。因此矩阵必须有**第三态**。

| 方言 | 请求层可关闭原生 tools | 提示层可渲染签名 | 流解析可识别 | 落库可存原文 | **读取/重放可还原** | 前端可展示 |
| --- | --- | --- | --- | --- | --- | --- |
| 原生结构化 | 已实现 | 已实现 | 已实现 | 已实现 | 已实现 | 已实现 |
| XML-in-tag | 已实现（开关缺） | 未实现 | 未实现 | 已实现 | **静默丢失** | 未实现 |
| P-Format | 已实现（开关缺） | 未实现 | 未实现 | 已实现 | **静默丢失** | 未实现 |
| 代码式 | 已实现（开关缺） | 未实现 | 未实现 | 已实现 | **静默丢失** | 未实现 |
| 语法约束 freeform | 不可（provider 绑定） | 不可 | 未实现 | 已实现 | **静默丢失** | 未实现 |
| 特殊 token | 已实现 | 未实现 | 未实现 | 已实现 | 取决于是否 JSON | 未实现 |
| 外挂解释器 | 已实现 | 不适用 | 不适用 | 已实现 | 已实现（输出是 JSON） | 已实现 |

**读法**：前三行不是「未支持」，而是「已解析但未透传」——落库成功、DTO 丢失。这正是 §2.4 的结构后果。

---

## 5 扩展成本分层

### 5.1 加实现 vs 改结构

| 层 | 位置 | 性质 | 说明 |
| --- | --- | --- | --- |
| 提示层 | `src-tauri/crates/assistant-agent/src/prompt_builder.rs` | **加实现** | 多渲染一种 schema 说明（JSON schema / 位置签名 / 函数签名 / DSL 规则） |
| 请求层 | 各 provider `request.rs` | **加实现** | 开关「是否发送原生 `tools` 字段」 |
| 流解析层 | 各 provider `stream.rs` + 共用 parser | **加实现** | 新增 parser；文本方言可共用同一套顺序回退 |
| 出站重放层 | 各 provider `message_converter.rs`（`parse_tool_input` / `parse_function_args`） | **改实现** | 当前一律 `from_str → json!({})`；需按形态还原 |
| 统一中间层 | `UnifiedToolCall` / `ChatDelta` | **改结构** | 仅当需要携带「参数形态」时 |
| 累积终结层 | `FinalizedToolCall` | **加实现** | 已保留 `raw_arguments`，可承载任意文本 |
| 持久层 | `messages.tool_calls_json` | **改约定** | 当前已存 raw 原文，**无需改表**；但缺方言标记 |
| DTO 层 | `MessageToolCallDto.arguments: Value` | **改结构** | 非 JSON 原文当前静默变 `null` |
| 前端视图层 | `src/chat/core/toolCallView.ts` 的 `parseArgs` | **改实现** | 严格 `JSON.parse` 需放宽为按形态解析 |

**结论**：9 层里只有 3 层需要**改结构/改约定**（统一中间层、DTO 契约、落库约定），其余 6 层都是**加实现**或**改实现**。而持久层**意外地已经就绪**——因为落库存的是 raw 原文。这是当前设计最值得保留的一点。

### 5.2 不可逆决策点

**落库形态是唯一真正不可逆的**。一旦写入用户数据库，历史会话就永久依赖当时的编码约定。

三个候选：

| 方案 | 形态 | 可逆性 | 风险 |
| --- | --- | --- | --- |
| 现状 | `arguments` 存 raw 字符串 | **最好**——raw 可重解释 | 缺方言标记，只能猜 |
| 加形态枚举 | `arguments` 存 raw + 平级 `shape` 字段 | 好 | 需补列 + 迁移 |
| 结构化为对象 | `arguments` 存 `{shape, raw}` | 一般 | 历史数据需迁移，且违反「不做无谓嵌套」 |

推荐方向是**第二格**：保持 raw 原文不动，只加一个平级标记。它与现有 `tool_calls_json` 完全兼容（`Vec<ToolCall>` 加可选字段即可，`#[serde(default)]` 吸收历史数据）。

**明确不建议**：把参数在落库时归一化成 JSON 对象。这会**永久丢弃**模型真正写出的字节，一旦解析器有 bug 就无法回溯——而 raw 原文正是现在唯一能救回来的证据。

---

## 6 取舍与风险

### 6.1 建议先做

**XML-in-tag，全链路**。理由：

1. 参数仍是 JSON，A3 不被打破——不需要改统一中间层与 DTO 语义
2. 只缺 id 与形态标记：id 可合成（`call_{index}` 风格，openhuman 用同样做法）
3. 收益明确：直接打开本地 Ollama / llama.cpp 场景
4. 可完整落在「加实现」里，除了四处静默兜底需要改成保留原文 / 可观测失败

这一格做完后，`auto` 降级链就有了第一个真实分支，后续方言可以按同一套 parser 骨架追加。

### 6.2 建议占位（只做结构预留，不做实现）

**参数形态标记**。只在落库与 DTO 契约里加一个可选字段，不实现任何新方言。理由：

- 它是 A7 的唯一解法，且是**不可逆决策**
- 越晚加，历史数据越多，迁移越贵
- 提前占位后，将来加方言不需要再改契约

占位内容建议最小化：一个可选枚举 + `serde(default)`，默认值代表「JSON」。

### 6.3 建议明确不做

| 方言 | 理由 |
| --- | --- |
| 语法约束 freeform | 绑定 provider 解码能力，与 provider 无关的统一层目标冲突 |
| 外挂解释器（tool shim） | 每次调用多一次推理；失败模式是「解释器幻觉」，比不解析更危险 |
| P-Format | token 省得有限，参数顺序敏感导致误判率高；openhuman 自己标 opt-in |

### 6.4 静默丢失（唯一无法靠日志发现的失败）

这是本文最应被记住的一条。

文本模拟工具的失败有三种：
1. **报错**：能发现，能重试
2. **误判**：把解释文字当调用——能被观察到（出现了不该有的调用）
3. **静默丢失**：调用的参数丢了，但流程继续——**无法发现**

第 3 种的机理就是 §2.4 的四处静默兜底。设计上必须让「解析失败」变成一个**可观测的第三态**，而不是伪装成「没有参数」。

参照 goose 的 `parse_tool_arguments` 返回 `Option`、agent 层 `FinalizedToolCall` 带 `is_error` + `parse_error`——**后端已有表达失败的能力，是四处重新解析边界把它抹平了**。修复点分散但性质一致，其中出站重放两处的优先级最高：它们会让模型看到被篡改的历史。

### 6.5 误判（文本模拟的固有代价）

goose 在 `references/1-ai-agent/goose/crates/goose-local-inference/src/tool_emulation.rs` 的文件头注释里老实写了已知误判：「`$` 出现在行首的说明文字会被当成命令」。这是**文本模拟的固有代价**，不是实现 bug。

缓解手段（按可靠性排序）：
1. **解码期约束**（grammar）——最可靠，但绑定 provider
2. **标签包裹 + 完整匹配**（openhuman 的 `<tool_call>`）——较可靠，但要求模型遵守
3. **行首标记**（goose 的 `$`）——不可靠，说明文字会误触发
4. **事后启发式**——最不可靠

结论：**只要模型不用原生结构化，就一定存在误判**。因此 `auto` 降级链的正确顺序是「原生优先」，文本方言只能是兜底，不能成为默认。

---

## 7 证据采集清单

写实施方案前需确认这几处，作为设计输入。

| 要确认的 | 位置 |
| --- | --- |
| 请求层是否已有「不发原生 tools」的开关 | 各 provider `request.rs` 的 `tools` 字段构造 |
| 工具目录渲染能否按形态切换 | `src-tauri/crates/assistant-agent/src/prompt_builder.rs` 的工具目录段 |
| 历史重建是否会丢 tool call 信息 | `prompt_builder.rs` 的 `rebuild_rows` / 工具配对修复 |
| 参数校验在哪一层做（决定放宽点） | `src-tauri/crates/assistant-tools/src/pipeline.rs` 的 `validate_input` |
| 出站重放是否有共用解析入口 | 各 provider `message_converter.rs` 的 `parse_tool_input` / `parse_function_args` |
| 结果配对是否强依赖 call_id | `src-tauri/src/cmd/conversation_utils.rs` 的 `collect_tool_results` |
| 落库形态能否加平级字段 | `src-tauri/crates/assistant-storage/src/conversations/msg_ops.rs` 的 `tool_calls_json` |
| 前端预览的容错边界 | `src/chat/core/toolCallView.ts` 的 `parseArgs` |

---

## 8 下游建议

若决定推进，建议按 §5.1 的成本分层拆成两批：

- **批一（不改结构）**：XML-in-tag 全链路 + `auto` 降级链 + **四处静默兜底改为可观测失败**（DTO 投影、Gemini/Anthropic 出站重放、前端预览）。可独立落地，不需要先改 spec。注意批一必须先修兜底、再接方言——否则新方言会被四处静默吃掉，无法验证是否真的可用。
- **批二（改结构）**：参数形态枚举（§6.2 的占位）+ 代码式解析。**必须先改 spec**，因为它同时影响落库约定、DTO 契约、前端视图三处，且属不可逆决策。

批一做完就能验证「文本方言可用」这个前提是否成立；若批一在真实模型上误判率不可接受，批二无需开始——这样避免了为一个未验证的前提做不可逆的契约变更。
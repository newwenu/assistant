# Token Usage 语义分析

> 只回答三件事：现状哪里失真、为什么失真、代码落在哪。
> 术语定义与计算式不在本文展开，一律引用 `docs/store/I29-token-usage-semantics-spec.md`；契约与展示见 `docs/store/I29-token-usage-ui-spec.md`。
> 不含实施方案、改造步骤与优先级排期。

---

## 0 一句话结论

本仓库里「token 数」至少有 9 个互不相同的量（6 个独立口径 + 3 个从属子集），而 UI 只用 `contextUsage.usedTokens` 一个字段、一根进度条承载了其中 2 个：流式期间它是「本次请求的输入 token」，切换会话后它变成「整段会话的累计 token」。两者量级差一个数量级，语义不可互换。

真正决定「上下文还能装多少」的是最近一次请求的 `prompt_tokens`——因为服务端每次都会把整段历史重新计一遍。「累计 token」天然随轮次呈 O(n²) 增长，把它当占用用必然虚高。

---

## 1 术语：九个量的索引

完整定义、四维坐标与反义见 `I29-token-usage-semantics-spec.md` §1。本节只保留与本仓库代码落点的对应关系，避免两份文档重复定义。

前六个是彼此独立的口径，后三个是「已包含在别的量里」的子集量，不能与前者相加。

| 术语 | 精确定义 | 谁产生 | 量级特征 | 代码落点 |
| --- | --- | --- | --- | --- |
| **上下文窗口** `contextWindow` | 模型单次请求可容纳的输入+输出上限，是能力常量，与已用量无关 | 模型配置（`models_json` 的 `contextWindow`）→ 本地注入 | 常量（128k / 32k 等） | `types::resolve_context_window_from_stored`、`chat_options.rs`、`chatUtils.ts` |
| **上下文占用** `contextTokens` | 下一次请求将发送的输入 token 数，最可靠的近似是上次请求的 `prompt_tokens` | 只能由 provider 上报推断 | 随轮次线性增长 | 未建模（现被 `usedTokens` 混用） |
| **单次请求输入** `promptTokens` | 本次请求实际发送给 provider 的输入 token | provider 上报 | 每轮不同 | `TokenUsage.prompt_tokens` |
| **单次请求输出** `completionTokens` | 本次请求生成的 token（含推理） | provider 上报 | 每轮不同 | `TokenUsage.completion_tokens` |
| **会话累计** `cumulativeTokens` | 所有轮次 `prompt + completion` 之和，即计费口径总消耗 | 本地聚合 | O(n²)，远大于占用 | `agent_modes.rs` 的 `SUM(tokens)` |
| **占比** `percentage` | 占用 / 窗口 | 本地计算 | 0 ~ >1 | 三处各自计算 |
| **缓存命中** `cacheRead` | 输入中被 prompt cache 命中的部分，已含在 `prompt` 内 | provider 上报（各系字段不同） | ⊆ prompt | `TokenUsage.cache_read_tokens` |
| **缓存写入** `cacheWrite` | 输入中写入 prompt cache 的部分，已含在 `prompt` 内 | 仅 Anthropic 系上报 | ⊆ prompt | `TokenUsage.cache_write_tokens` |
| **推理** `reasoning` | 思考 token，已含在 `completion` 内（Gemini 归一化后） | Gemini 上报；OpenAI / Anthropic 未解析 | ⊆ completion | `TokenUsage.reasoning_tokens` |

### 1.1 三个口径的区分与本仓库现状

窗口（分母，模型属性）、占用（分子，能否继续塞）、累计（账单，与窗口无关）三者必须在 UI 分开呈现。定义见 `I29-token-usage-semantics-spec.md` §1 的坐标表，各量「不是什么」见该文档 §1.1。

本仓库现状是三者只落在一个字段：`contextUsage.usedTokens` 同时承载「本次 prompt」与「会话累计」（见 §5 A1）。

### 1.2 中文短语的三重歧义

「上下文窗口总 token 花费」在不同语境下指窗口 / 占用 / 累计三者之一，量级相差一个数量级。判别方式与数值对照见 `I29-token-usage-semantics-spec.md` §1.2。

对代码的直接后果：凡以该短语命名的字段或文案，都无法从名字判断它属于哪个口径，必须回查写入点。

### 1.3 `context_window_tokens` 不是 provider 返回值

`TokenUsage.context_window_tokens` 由本地 `ChatOptions.context_window_tokens` 注入（`build_chat_request` 从模型配置解析），provider 响应里没有该字段。它是本地账务字段，缺省值 `128_000` 在前端 6 处、后端 3 处（另 1 处仅在测试夹具内）各自独立硬编码：

| 侧 | 位置 |
| --- | --- |
| 前端 6 处 | `stores/chatStorePorts.ts`（3 处）、`stores/chatStore.ts`、`stores/chatConversationSlice.ts`、`services/chatOrchestrator.ts` |
| 后端 3 处 | `cmd/conversation_utils.rs`、`assistant-storage/src/agent_modes.rs`、`assistant-providers/src/model_catalog.rs` |
| 后端 1 处（测试内） | `assistant-providers/src/types/config.rs` 的取值位于 `#[cfg(test)]` 模块内，不进运行时 |

（`settings/pages/AgentModeSettings.tsx` 的 `max: 128_000` 是滑块上限，非窗口缺省；`protocol/dto/mod.rs` 与各 `tests/` 中的取值是测试夹具，均不计入。）

### 1.4 两种度量方式被混进同一个标量

- **上报值**：provider 返回的 `prompt_tokens` / `completion_tokens`，精度高，含真实分词细节。
- **估算值**：`(len + 3) / 4 + 4`（`estimate_tokens`）与 `len / 4`（`request_projection`），按字节长度折算。

现状 `SUM` 把两者相加：assistant 消息贡献「上报的每轮总量」，user 消息贡献「字符估算」。同一个聚合数里混了两种量纲，既不是占用也不是精确账单。

---

## 2 为什么「累计」天然是 O(n²)

每轮请求都要重发整段历史，因此 `第 n 轮 prompt ≈ 前 n-1 轮的全部内容`，于是 `Σ(prompt) ≈ n²/2 × 单轮增量`。

以每轮「用户 1k + 助手 1k」为例（忽略 system prompt）：

| 轮次 | 本轮 prompt | 本轮 total（prompt+completion） |
| --- | --- | --- |
| 1 | 1,000 | 2,000 |
| 2 | 3,000 | 4,000 |
| 3 | 5,000 | 6,000 |
| 4 | 7,000 | 8,000 |
| **Σ** | **16,000** | **20,000** |

128k 窗口下：真实占用 7,000（5.5%），累计 20,000。轮次越多差距越大（平方级），`A / O ≈ n/2`。

注意末列「累计 / 窗口 = 15.6%」是**错误用法会看到的数字**，不是任何合法口径——`A` 与 `W` 之间没有可解释的比值。完整推导与 1/5/10/20 轮的对照表见 `I29-token-usage-semantics-spec.md` §3.2。

**结论**：`SUM` 作为计费累计是正确的，作为上下文占用是错的。这不是聚合函数写错，而是口径被换掉了。

---

## 3 对话里哪些 token 进入上下文

本节说明构成，公式与估算误差的量化见 `I29-token-usage-semantics-spec.md` §3.1、§3.5。

### 3.1 组成公式

公式与各项的语义定义见 `I29-token-usage-semantics-spec.md` §3.1。本仓库的差异只有一处：`provider 自身开销`（role 包装、分隔符）完全无法本地复现，因此本地任何求和都必然低于真实 `P`，这也是「占用只能取上报值」的代码依据。逐项的实际进出情况见 §3.2。

### 3.2 逐项清单

| 项 | 是否进入上下文 | 依据 |
| --- | --- | --- |
| Agent 模式的 `prompt` | 是，作为 system 消息插在最前 | `prompt_builder::prepare_messages` |
| `conversations.system_prompt` | 否 | 建会话时恒为 `None`，无写入路径；`chat_request.rs` 只读 `mode.prompt` |
| 活跃分支的 user / assistant `content` | 是，沿父链 leaf→root 后逆序 | `trace_active_branch` + `prepare_messages` |
| `compact_summary` 角色消息 | 否 | `filter(|m| m.role != "compact_summary")` |
| 历史消息的 `reasoning` | 否 | `ChatMessage` 结构无该字段，历史思考不回放 |
| 历史消息的 `tool_calls` / `tool_call_id` | 否 | 组装时显式置 `None` |
| Agent 模式的 `tools_json` | 否 | 从未进入请求体（`ChatRequest` 无 tools 字段） |
| 图片附件（vision 模型） | 是，rehydrate 为 image block | `multimodal::rehydrate_images` |
| 图片附件（非 vision 模型） | 是，降级为占位文本 | `project_images_for_text_model` |
| 文件附件（支持 PDF） | 是，rehydrate 为 file block | `rehydrate_files` + `support_pdf` |
| 文件附件（不支持） | 是，降级为占位文本 | `project_files` |
| 本轮新用户消息 | 是（落库后组装） | `prepare_messages` |
| 兄弟分支消息 | 否 | 只走活跃分支链 |
| 会话标题 / 时间戳 / `is_pinned` | 否 | 元数据，不进请求体 |

### 3.3 附件估算与降级

- 单图固定估算 **765** tokens（`IMAGE_TOKEN_ESTIMATE`，按 768×768 / 0.75 折算）。
- 文本 / 文件 / 思考 / 工具块统一按 `len / 4`。
- `offload_if_over_budget` 在估算总量 > 窗口时，从最后一条用户消息起逆序把图片换成占位文本。
- 降级预算用的是完整窗口，未预留输出侧 `max_tokens`（默认 4096），输入可用预算被高估约 4096。
- 降级只改出站请求、不改持久化消息 → 库内 `content` 与真实发送内容可能不一致，按库内容估算会偏高。

### 3.4 压缩之后占用并不下降

自动压缩判定是 `prompt_tokens ≥ 窗口 × 80%`，且只在 provider 上报了 `prompt_tokens` 时才会判定（不报 usage 的 provider 永远不会触发自动压缩）。

压缩完成后的实际状态：

| 环节 | 现状行为 |
| --- | --- |
| 摘要落库 | 作为 `compact_summary` 角色消息插入分支 |
| 分支指针 | 指向摘要节点，`trace_active_branch` 仍沿父链回溯全量历史 |
| 出站请求 | `compact_summary` 被过滤，其余历史照旧全部发送 |
| 会话状态 | `is_full = 1`，输入框禁用 |
| 摘要复用 | 不进入任何后续 prompt；新建会话也不携带 |

因此压缩前的出站构成 ≈ 压缩后的出站构成：摘要消息虽进入分支，但在组装时被过滤，出站集合与压缩前完全相同，`O` 不变。压缩的实际效果是「停机 + 存档」——唯一变化的 `is_full = 1` 是停机标记，不是容量变化。`compact_summary` 的用途是存档与展示（`MessageBubble` 渲染成交接摘要样式），不参与出站组装，详见 §3.5。

### 3.5 「停机 + 存档」是当前已接受的行为

I7 的原始设计是「停机 + 存档 + 续接」：`ARCHITECTURE.md` §I7 与 `I7-spec.md` §7.1 都写明「[继续对话] 自动新建对话并将摘要注入首条 user 消息」。**当前交付范围只到「停机 + 存档」**，续接不纳入计划。代码现状：

| 环节 | 设计意图 | 现状 | 位置 |
| --- | --- | --- | --- |
| 横幅按钮 | 新建会话 + 注入摘要 | 只建**空会话**，摘要不带 | `chat/compact/CompactBanner.tsx` 的 `handleNewConversation` → `createConversation(providerId, modelId)` |
| 后端入口 | 建会话时接受初始消息 | `create_conversation` 只有 `provider_id` / `model_id` / `mode_id`；`fork_to_new_conversation` 只复制分支、无注入 | `cmd/conversation.rs` |
| 摘要消费 | 新会话首条 user 消息 | 只用于**存档与展示**：`MessageBubble` 以 `role === "compact_summary"` 渲染成交接摘要样式，符合 I7-spec「摘要可见」 | `chat/message/MessageBubble.tsx`、`services/chatLifecycleEvents.ts` |
| 文案键 | `chat.compact.continue_conversation` | 该键不存在，只有 `chat.compact.new_conversation`（「新建对话」） | `i18n/locales/{zh-CN,en-US}.json` |

**`prompt_builder` 过滤 `compact_summary` 是长期设计，不计划改动**：`compact_summary` 不是合法的 provider role，原会话又已 `is_full` 锁定、本就不该再发送，过滤是必要的防御。摘要的正确消费路径是「新会话首条 `user` 消息」，与 `prepare_messages` 的 role 过滤无关——`prepare_messages` 只做三件事（过滤 `compact_summary`、插 system、可选追加 user），对 user 消息一视同仁，**因此即使将来实现续接也无需修改该文件**。

两个语境的差别：

- **原会话**：已锁定、不再发送，`O` 停在压缩前的值。压缩在这里确实不缩减任何东西——这是当前已接受的行为。
- **续接会话**：出站是「摘要 + 新消息」，历史被摘要替换，`O` 应从续接会话首次请求的 `P` 重新锚定。当前未实现。

在当前范围内只存在第一种语境，`O` 不变成立。

---

## 4 Provider 归一化差异如何影响上述语义

各 provider 对「`P` 是否含缓存、`C` 是否含推理、`total` 该取哪个」的语义结论见 `I29-token-usage-semantics-spec.md` §4。本节只记本仓库适配层**实际解析了哪些字段**，作为该文档结论的代码依据。

| Provider | 适配层写入 `cache_read_tokens` 的字段 | 适配层写入 `cache_write_tokens` 的字段 | 推理字段 | 适配层 `total_token_count` 来源 |
| --- | --- | --- | --- | --- |
| Anthropic | `cache_read_input_tokens`；`P` 由 `input + cache_read + cache_creation` 本地求和 | `cache_creation_input_tokens` | 无（thinking 已在 `output_tokens` 内） | 本地求和 |
| OpenAI Chat Completions | `prompt_cache_hit_tokens`，缺失时回退 `prompt_tokens_details.cached_tokens` | 恒 `None` | `completion_tokens_details` 未建模，恒 `None` | `usage.total_tokens` |
| OpenAI Responses | `input_tokens_details.cached_tokens` | `input_tokens_details.cache_write_tokens` | 未提取，恒 `None` | `usage.total_tokens` |
| Gemini | `cached_content_token_count` | `cache_creation_token_count`（无 provider 写入路径，恒 `None`） | `thoughts_token_count` 已并入 `candidates_token_count`，单列值上报 | `usage.total_token_count` |

跨 provider 统一不变量（`cacheRead ⊆ prompt`、`cacheWrite ⊆ prompt`、`reasoning ⊆ completion`、`uncached = prompt - cacheRead - cacheWrite`、`total = prompt + completion`）与推导见 `I29-token-usage-semantics-spec.md` §2.1、§4。

上表的 `total_token_count` 是**适配层**产出的值，到不了持久化层：`chat_runtime` 的累加器自己算 `total = prompt + completion`，provider 返回的 `total_tokens` 被丢弃；`StreamOutcome` 只保留 `total_tokens` 与 `prompt_tokens`，`UnifiedTokenUsage` 的其余字段在运行时就已丢失。

---

## 5 现状失真清单

计算层的错误算法另见 `I29-token-usage-semantics-spec.md` §6（M1–M16），本节只记现象与代码根因。编号与 ui-spec 的 P/W/Q/F 映射见 `I29-token-usage-semantics-spec.md` §8.1。

| 编号 | 现象 | 根因 | 影响 |
| --- | --- | --- | --- |
| A1 | `usedTokens` 流式期间是「本次 prompt」，切会话后是「会话累计」 | 两个来源写入同一字段：`finishStream` 用 `promptTokens`，`fetchTokenUsage` 用 `summary.totalTokens` | 同字段两种语义，UI 无法自洽 |
| A2 | 切换会话后进度条常打满 / 泛红 | `agent_modes.rs` 用 `SUM(tokens)` 当占用 | 用户误判「马上爆上下文」，实际可能只有 5% |
| A3 | 聚合数里混了三种度量 | 有 usage 的 assistant 行贡献整轮 total；无 usage 的 assistant 行贡献「仅回复文本」的估算（`resolve_final_tokens` 只拿 `full_content` 估算，不含 prompt）；user 行贡献 `(len+3)/4+4` 估算 | 数值既不是占用也不是精确账单 |
| A4 | 历史消息上的 `tokens` 被当作「这条消息自己的 token」 | 有 usage 时该值是整轮 prompt+completion；无 usage 时只是回复文本的估算 | 同一列在两种情况下含义不同，长回复的 token 数看起来远超其文本长度 |
| A5 | provider 的 `total_tokens` 被忽略 | 累加器用 `prompt + completion` 覆盖 | 与官方账单可能有小数级差异 |
| A6 | 窗口缺省值 `128_000` 多处硬编码 | 前后端各自 fallback | 未知模型永远显示 128k，压缩阈值也按 128k 算 |
| A7 | 发送瞬间进度条先归零再跳升 | `beginSend` 重置 `usedTokens: 0` | 视觉抖动，且与「已占用」事实矛盾 |
| A8 | 切会话时 `maxTokens` 先写 128k 再被覆盖 | `switchConversation` 重置 + 异步 `fetchTokenUsage` | 短暂显示错误分母 |
| A9 | 压缩不改变原会话的出站构成，且续接未实现 | 见 §3.4、§3.5 | 压缩按钮 / 自动压缩给了「能省上下文」的错误预期；摘要仅用于存档与展示 |
| A10 | 7 个 usage 字段只有 3 个被消费 | `ContextBar` 只读 `usedTokens/maxTokens/percentage`；前端实际取值仅 `promptTokens` / `completionTokens` / `contextWindowTokens`，`cacheReadTokens` / `cacheWriteTokens` / `reasoningTokens` 在 `src/` 内无任何读取点（仅类型声明），`TokenUsage.totalTokens` 亦无消费方 | 缓存命中、缓存写入、推理 token 与 provider total 全部不可见 |
| A11 | 多次 attempt 的 usage 被覆盖而非累加 | 累加器每次 `usage` 直接覆盖 | 失败重试的消耗不计入任何统计 |
| A12 | 压缩请求自身的消耗完全不记录 | `cmd/compaction.rs` 丢弃 `outcome.total_tokens`，摘要消息写入 `tokens: None` | 一次压缩可能花掉整个窗口的输入 token，账面上为零 |
| A13 | 窗口未知在类型上无法表达 | `TokenUsage.context_window_tokens` 与 `TokenUsageDto` 同名字段均为 `u32`（`#[serde(default)]` 且无 `skip_serializing_if`），未知只能传 `0` | 「未知」与「窗口是 0」被压成同一个值，违反 `I29-token-usage-semantics-spec.md` §1.3；即使上层已改成 `Option`，序列化边界仍会退回 `0` |
| A14 | 占用锚点不区分分支 | 消息层已按 `active_branch_id` 追踪（`get_conversation_detail` → `trace_active_branch`），但占用锚点取「该会话最新一条 chat 请求」，不区分该请求属于哪条分支 | 切到旧分支后，进度条仍显示被弃用分支的 `P`，与即将发送的历史不一致。（累计值不按分支过滤是正确的：花掉的钱不分分支） |
| A15 | OpenAI 的推理与缓存写入口径恒为「未上报」 | `openai/types.rs` 的 `ChatUsage` 未建模 `completion_tokens_details`，`openai/stream.rs` 的 `From<ChatUsage>` 里 `reasoning_token_count: None`、`cache_creation_token_count: None` 为硬编码；`prompt_cache_miss_tokens` 虽已反序列化但标 `#[allow(dead_code)]` 未使用 | 推理口径对 OpenAI 永远缺失，Tooltip 恒显示「未上报」；这与「provider 确实没报」无法区分 |
| A16 | 窗口未知时压缩与降级判定静默失效 | `compaction::should_terminate` 在 `context_window_tokens == 0` 时直接返回 `false`；offload 预算同样依赖该窗口 | 未声明窗口的模型既不自动压缩也不降级，直到 provider 报超窗错误为止——「不判定」被当成「不超限」 |
| A17 | 超窗错误没有处理路径 | `context_length_exceeded` 仅出现在 `client/error.rs` 的测试夹具里，没有匹配分支与降级动作 | 一旦超窗直接失败，既不提示「先压缩」也不自动重试，用户只能手动操作 |
| A18 | `is_full` 名为「窗口已满」，实为「已压缩锁定」 | 只由压缩成功路径置 `true`（`cmd/compaction.rs`）、由 `unlock_conversation` 置 `false`，与窗口占用无关 | 字段名易被读成容量状态，而它表达的是「该会话已存档、不再发送」 |
| A19 | 记录排序无二级键 | 记录表 `created_at` 为毫秒时间戳，占用查询 `ORDER BY created_at DESC LIMIT 1` 在同毫秒内无确定次序（重试的两次请求可能同毫秒） | 「最新一条」可能取到同毫秒内的前一次请求，占用分子抖动 |
| A20 | provider 不报 usage 时，有内容的会话永远看不到占用 | 记录表只接受 provider 上报值，`flush_attempt` 在 `UsageSnapshot::is_empty()` 时直接返回、不入队（`chat_runtime.rs`），因此该会话零记录；占用查询无结果 → `hasUsage = false` → 显示「—」 | 「分支无内容」与「有内容但无上报」被渲染成同一个「—」，用户无法判断距窗口还有多远；压缩判定同样失明 |

---

## 6 估算公式的偏差

本仓库三个估算式的定义见 `I29-token-usage-semantics-spec.md` §3.4（E1/E2/E3），误差方向与量级见 §3.5。本节只讨论各参考项目对这条偏差的处理策略差异。

`len / 4` 隐含「4 字节 ≈ 1 token」，对英文近似成立，对中文明显偏低：

- 中文 UTF-8 每字 3 字节 → `3 / 4 = 0.75` token/字
- 实际分词通常 1 ~ 1.5 token/字

纯中文会话的估算值会系统性低估约 30% ~ 50%，用于 offload 预算与「零记录时的占用展示兜底」都偏乐观。

> T5.3 已把本仓库的估算口径换成 E1 字符加权（CJK 记 1 token/字），偏差从「低估 30%~50%」收窄到「低估至多约 33%」，且估算已退出记账侧；下文对旧口径的描述保留为演进记录。

各项目采用的估算密度与分母单位汇总在 `I29-token-usage-semantics-spec.md` §3.5 的表；本节只记「策略取向」的差异——修正、规避、还是让偏差进入用户可见的分子。

- **OpenBitFun**（修正）：`util/token_counter.rs` 按字符类别给权重，ASCII 字符 `+0.3` token、非 ASCII 字符 `+0.6` token（英文约 3.3 字符/token、中文约 1.7 字符/token），再加结构开销（每条消息 `+4`、每个 tool_call `+10`、消息数组 `+3`、每个工具定义 `+15`，工具非空再 `+10`）。这是唯一主动给 CJK 加权的实现。
- **deepseek-harness-orb**（规避）：`token-meter/estimate.ts` 固定 `CHARS_PER_TOKEN = 4`，`projection.ts` 的注释明确承认「the estimator systematically underprices CJK text and JSON schemas」，因此把估算值限定在 `contextBreakdown`（只回答「由什么构成」），占用一律锚定 provider 上报的 `projectedTokens`，两者**不允许相加**。
- **openhuman**（隔离）：`prompt_size.rs` 以字节为记账单位，`EST_BYTES_PER_TOKEN = 4` 只用于渲染人类可读的 `~N tok` 列，源码注释直言「is a divisor, not a tokenizer」。估算值不进入任何判定。
- **codex**（不用估算）：不做字符估算，改为用 `BASELINE_TOKENS = 12_000` 直接修正窗口，把 tokenizer 差异转成固定余量。

本仓库原本不属于以上四种：既不修正权重，又不隔离，还把估算值直接混进占用与聚合（见 A3），于是偏差进入了用户可见的分子。T5.3 修正了权重（对齐第一种做法），并停止把估算写入记账；P8 让估算只在「零记录时的展示兜底」出现，且强制标 `~`（对齐第二种的隔离取向，但保留请求发出前的决策能力）。

---

## 7 四个参考项目怎么建模同一件事

四个项目形态不同：OpenBitFun 是 Rust 桌面端，deepseek-harness-orb 是 TS 事件溯源内核，codex 是 Rust CLI，openhuman 是 Rust RPC 服务。它们回答的是同一组问题，但切分方式各不相同。以下逐项目列出真实结构与字段，再与 `I29-token-usage-semantics-spec.md` §1 的九个量对齐。

### 7.1 OpenBitFun：账单与占用是两套持久化

这是与本仓库架构最接近的项目（同为 Rust + Tauri 形态），它把两件事拆成两套独立存储：

**账单侧 —— `TokenUsageRecord`**（`services-core/src/token_usage/types.rs`）

```
model_config_id / effective_model_name / session_id / turn_id / timestamp
input_tokens / output_tokens / cached_tokens / cache_write_tokens / total_tokens
cached_tokens_available: bool   ← 显式区分「未上报」与「上报 0」
token_details: Option<Value>    ← cache_write 从 cacheCreationTokenCount 提取（仅 Anthropic）
is_subagent: bool
```

三个细节直接对应本仓库的失真点：

1. `cached_tokens_available` 是独立布尔，不是用 `0` 冒充「未上报」。本仓库 `TokenUsage.cache_read_tokens` 也用 `Option` 表达未上报，流式 delta 确实把它送到了前端，但落库路径上的 `StreamAccumulator` 只保留 `prompt_tokens` 与 `total_tokens`（`chat_runtime.rs`），因此持久化侧永远只有「未上报」；前端则只消费 `promptTokens`（A10）。同一字段在传输层存在、在存储与展示层消失。
2. `is_subagent` 让子代理请求可被默认排除（`TokenUsageQuery.include_subagent` 默认 `false`）。本仓库没有子代理概念，所有轮次都进同一个 `SUM`。
3. 每条记录带 `turn_id`，因此「一轮内多次请求」天然可分辨。本仓库的重试 attempt 直接覆盖（A11）。

**聚合侧 —— 两级独立统计**

| 结构 | 关键字段 | 与命中率的关系 |
| --- | --- | --- |
| `ModelTokenStats` | `total_input` / `total_output` / `total_cached` / `total_cache_write` / `cache_reported_input_tokens` / `request_count` / `session_ids: HashSet` / `first_used` / `last_used` | `cache_hit_ratio() = total_cached / cache_reported_input_tokens`，**分母只累计「provider 明确上报过缓存」的请求**；分母为 0 返回 `None` 而非 `0` |
| `SessionTokenStats` | 同上（无 `session_ids`） | 同一套命中率口径 |

此外 `estimated_cache_savings_ratio() = cache_hit_ratio() * 0.90`，按「cache-read 约等于全价的 10%」折算节省比——即它区分「token 命中率」与「成本节省率」两个量，后者是估算。本仓库两个量都没有。

落盘方式是 `records/<date>.json` 按日分片 + `model_stats.json` 汇总，写入由 `AgenticEvent::TokenUsageUpdated` 订阅者触发，一次请求一次 `record_usage`。

**占用侧 —— `SessionContextUsage`**（`agentic/session/context_usage.rs`）

```
turn_id / input_tokens / output_tokens / total_tokens / timestamp
source: ModelRequest | ContextCompression
```

关键在 `source` 枚举：占用有两个合法来源，`ModelRequest` 是 provider 上报的请求用量，`ContextCompression` 是压缩后的 `tokens_after`。也就是说**压缩结果会作为新的占用锚点写入同一投影**，而账单侧完全不受影响。本仓库的压缩既不更新占用（§3.4、§3.5），又把压缩请求的 usage 丢弃（A12）——两侧都没落。

**压力预算 —— `compression_trigger_budget`**（`agentic/execution/execution_engine.rs`）

```
output_reserve = configured max_tokens ?? automatic_max_output_tokens(context_window)
safety_reserve = 10_000
input_limit    = context_window - output_reserve - safety_reserve
usage_ratio    = total_tokens / context_window        ← 分母是完整窗口
硬触发         = total_tokens >= input_limit
预取窗口       = input_limit - 10_000 <= total_tokens < input_limit
```

触发是**两段式**：进入预取窗口（`compression_prefetch.rs` 的 `PREFETCH_LEAD_TOKENS = 10_000`）先发起一次推测性的压缩作业，真正越过 `input_limit` 才把结果应用。这样压缩的等待时间被前置消化，不会卡在「已经塞不下」的那一刻。

与本仓库对比：本仓库是 `prompt_tokens >= context_window * 80%`（`compaction.rs`），**不扣输出预留、不扣安全裕度、没有预取**，且 `prompt_tokens == 0` 时直接短路（不报 usage 的 provider 永不触发）。OpenBitFun 的 `input_limit` 在 128k 窗口、`max_tokens` 未设时会预留 32k 输出 + 10k 安全，实际输入预算约 86k；本仓库的 80% 是 102.4k，两者相差约 16k，恰好是本仓库 §3.3 中 offload 未扣 `max_tokens` 的那部分。

**增量估算 —— `TokenAnchor`**（`agentic/session/token_anchor.rs`）

```
anchor_id / session_id / turn_id / round_id / model_id
prefix_message_count / prefix_last_message_id / prefix_digest(SHA256)
input_tokens                          ← provider 上报值
system_tokens_at_anchor / tool_tokens_at_anchor / prepended_reminder_tokens_at_anchor
```

锚点把「上一次 provider 上报的输入」固定在前缀位置上，下一次压力估算只做增量：

```
total = input_tokens
      + (current_system - system_at_anchor)
      + (current_tools - tool_at_anchor)
      + (current_reminder - reminder_at_anchor)
      + estimate_tail(messages[prefix_message_count..])
```

锚点失效有三种判定：前缀长度变短/变长、`prefix_last_message_id` 与当前第 `count-1` 条不符、前缀内容 `digest` 不符。最近 64 条锚点保留。这样每轮只估「尾部新增」，避免 O(n) 全量重算，同时保持与 provider 值锚定。本仓库没有任何锚点：要么用 provider 上报的 `prompt_tokens`，要么 `len / 4` 全量估算，没有中间态。

### 7.2 deepseek-harness-orb：三个投影，明确禁止互相替代

它以「session projection」形式定义三份**独立**视图（`packages/llm/token-meter/src/projection.ts`）：

| 投影 | 字段 | 语义边界（源码注释原文要点） |
| --- | --- | --- |
| `TokenUsageProjection` | `uncachedInputTokens` / `outputTokens` / `cacheReadTokens` / `cacheWriteTokens` | 四桶**互不相交**；「reasoning 已包含在 outputTokens 中，不再重复累加」 |
| `ContextPressureProjection` | `pressureTokens?` / `projectedTokens?` / `contextWindow?` | 「**刻意不是一次原子请求观测**：每个字段是不同时刻的 last-wins 记录」；换模型会出现「新容量配旧压力」的短暂错配，这是「面向用户的参考值，不是计费或准入输入」的取舍 |
| `ContextBreakdownProjection` | `systemTokens` / `toolsTokens` / `messageTokens` | 全部用固定启发式，「**与 provider 锚定的 `projectedTokens` 不会相等**，因为估算器系统性低估 CJK 与 JSON schema」，因此「只作构成近似展示，绝不当作总量」 |

三个字段的定义精确到口径：

- `pressureTokens` = `inputTokens + cacheReadTokens + cacheWriteTokens`，**排除 output**，所以「本轮流式过程中它不会增长」。
- `projectedTokens` = `pressureTokens + surfaceTokens - sampledSurfaceTokens`，即「下一次请求的 prompt 会是多少」。
- 二者缺一不可的原因写在注释里：只有 `pressureTokens` 看不到压缩（压缩本身不上报 usage），所以 fold 额外维护 running `surfaceTokens`，压缩 shadow 掉一段后 `projectedTokens` 立刻下降。

**重试处理**：`llm/retry-started` 事件把 `last` 槽清空，`addReplacing` 实现「先减 previous 再加 next」，因此重试 attempt 的 usage 是**替换**而非叠加，且不会重复计入 totals。本仓库是每次收到 usage 直接覆盖（A11），语义相反：重复消耗被丢弃而非替换。

**严格校验**：`turn-usage.ts` 的 `deriveTurnTokenUsage` 规定——任何 attempt 缺生命周期边界、usage 不完整、计数非安全整数、或 `total - output < knownPrompt`、`exactPrompt !== knownPrompt` 等矛盾，整体返回 `undefined`（宁可不显示，不给错数）。`cacheReadTokens` / `cacheWriteTokens` / `reasoningTokens` **只在每个 attempt 都上报时才出现**，且 `reasoning <= output` 是硬校验。本仓库的做法是「有值就用、无值估算」，正好相反。

### 7.3 codex：用两个字段承载两种口径

`TokenUsageInfo`（`protocol/src/protocol.rs`）只有三个字段，但切分得很干净：

```
total_token_usage: TokenUsage    ← 会话累计
last_token_usage:  TokenUsage    ← 最近一次请求
model_context_window: Option<i64>
```

`append_last_usage` 的实现是 `total.add_assign(last); self.last_token_usage = last.clone()`——**同一份 usage 同时进累计与覆盖最近值**，两者从不混用。`TokenUsage.tokens_in_context_window()` 的注释写得很直白：

> 对 `last_token_usage` 是「最新活跃上下文大小」，对 `total_token_usage` 是「会话累计」。

UI 侧的选择也一致：`status/card.rs` 取 `info.last_token_usage` 作进度条分子，`total_token_usage` 只出现在账单展示。这正是本仓库 A1 缺失的那层区分——本仓库把这两者写进同一个 `usedTokens`。

另外两个细节：

1. **有效窗口**：`BASELINE_TOKENS = 12000`，`percent_of_context_window_remaining` 用 `effective_window = context_window - 12000`、`used = total_tokens - 12000`。它承认固定开销占掉一截，与 OpenBitFun 的 `safety_reserve` 是同一思路的两种表达。本仓库分母直接用完整窗口，没有基线扣除。
2. **展示口径**：`non_cached_input = max(input - cached, 0)`，`blended_total = non_cached_input + output`——展示时把缓存命中从输入里剔掉，避免「用了 100k 但其中 90k 命中」被读成「烧了 100k」。本仓库不区分。
3. **三层记录**：`TokenUsageRecord { thread_id, turn_id, session_id, root_turn_id, response_id, usage, turn_token_usage, thread_token_usage }`，请求级 / 轮级 / 线程级同时落盘，而非事后用 SUM 反推。

### 7.4 openhuman：把「上一轮输入」单列为仪表分子

`ThreadTokenUsageResponse`（`threads/ops/usage.rs`）字段划分：

| 字段 | 用途 |
| --- | --- |
| `input_tokens` / `output_tokens` / `cached_input_tokens` / `cost_usd` / `turn_count` | 累计账单（orchestrator + 全部子代理） |
| `last_turn_input_tokens` / `last_turn_output_tokens` | **上下文仪表分子** |
| `context_window` | 分母，`0` 表示未知 |
| `has_usage: bool` | 是否有持久化记录 |
| `subagents: Vec<SubagentUsageDto>` | 按子代理拆分的消耗 |

两处注释点出了本仓库同样会遇到的问题：

1. `last_turn_input_tokens` 明确「**只算 orchestrator 自己，排除子代理**，因为每个子代理运行在自己的上下文窗口里，折叠进来会让仪表超过 100%」。本仓库若将来引入子代理，`SUM` 口径必然重犯这个错误。
2. `has_usage` 明确「transcripts 存在但没记录任何 spend 时必须返回 `false`，否则 UI 会用 0 覆盖正在进行中的 live 桶」。本仓库 `fetchTokenUsage` 拿到 `totalTokens = 0` 时无条件覆盖 `usedTokens`，正是它要规避的场景。

`agent/context_breakdown.rs` 则提供**构成拆解**而非总量：`sections = system prompt 的各 section + 汇总 tools + history`，其中 `history.est_tokens = last_turn_input_tokens`（用上一轮输入反推，不是重新估算全部历史），`est_tokens = bytes / EST_BYTES_PER_TOKEN`，并返回 `context_window`（未知记 `0`）。它把「构成」与「总量」分开成两个 RPC，避免用构成之和冒充总量。

成本侧还有一条：`cost_usd` 按**当前价格重新审计**，不信任 transcript 里写入时的 `charged_amount_usd`。本仓库只有 token 数，没有计价（§8）。

### 7.5 汇总对照

| 维度 | OpenBitFun | deepseek-harness-orb | codex | openhuman | 本仓库 |
| --- | --- | --- | --- | --- | --- |
| 请求级持久化 | `TokenUsageRecord`，含 `turn_id` / `is_subagent` | 事件日志即事实源，投影由事件折叠 | `TokenUsageRecord` 三层同时落 | transcript 逐条 `usage` | 仅 `messages.tokens` 一个标量 |
| 占用与累计的隔离 | 两套存储：`TokenUsageRecord` + `SessionContextUsage` | 两个投影：`tokenUsage` / `contextPressure` | 两个字段：`total` / `last` | 两组字段：累计 + `last_turn_input_tokens` | 同一字段 `usedTokens`，两种来源 |
| 「未上报」表达 | `cached_tokens_available: bool` | 字段 `undefined` + 严格校验整体失效 | `Option<i64>` | `context_window: 0` 表未知 | `Option` 存在但运行时被丢弃；`context_window_tokens` 为 `u32`（A13） |
| 缓存命中率分母 | `cache_reported_input_tokens`（仅已上报请求） | 四桶互斥，天然可算 | `non_cached_input` 展示时剔除 | 单列 `cached_input_tokens` | 无命中率 |
| 上下文压力 | 全量估算 + `TokenAnchor` 增量 | `pressureTokens` + `projectedTokens` | `last_token_usage` | `last_turn_input_tokens` | `usedTokens`（可能是 SUM） |
| 压缩对占用的影响 | 压缩后 `tokens_after` 写入占用投影（`source = ContextCompression`） | `projectedTokens` 经 surface delta 立即下降 | 本次核对的 `token_usage.rs` / `protocol.rs` 中无压缩与占用的联动 | `context_breakdown` 只做构成拆解，不涉及压缩 | 占用不变，压缩 usage 丢弃 |
| 压缩触发预算 | `window - output_reserve - 10k` | `token-meter` 只产出投影，不含触发阈值 | 本次核对范围未涉及 | 本次核对范围未涉及 | `window * 80%`，无预留 |
| 估算权重 | ASCII 0.3 / 非 ASCII 0.6 + 结构开销 | 固定 4 字符/token + 结构开销，且只用于构成 | 无本地估算 | `bytes / EST_BYTES_PER_TOKEN` | 统一 `len / 4` |
| 重试 attempt | 每条记录带 `turn_id`，可分辨 | `retry-started` 清槽 + `addReplacing` 替换 | `last` 覆盖、`total` 累加 | transcript 逐条累积 | 直接覆盖，重复消耗丢失 |
| 子代理 | `is_subagent` 默认排除 | 由 route/attempt 归属 | 无 | `subagents` 拆分，且不进入仪表 | 无概念 |

差异的实质可以归结为一句话：**四个参考项目都把「请求级事实」当作唯一可信源，再用多个互不替代的视图回答不同问题；本仓库反过来，用一个聚合标量同时回答「占了多少」和「花了多少」，于是两个问题都答不准。**

四者还有一个共同的取舍值得记录：占用值一律锚定 provider 上报，本地估算只允许出现在「构成拆解」里，且明示为近似（openhuman 的 `est_tokens`、deepseek-harness-orb 的 `contextBreakdown`）。本仓库是目前唯一把估算值直接当作占用分子展示的（A3、§6）。

### 7.6 窗口未知时怎么处理

`W` 未知（模型配置没填、provider 未声明）时，四家分成两派：**让未知保持未知**（codex、orb）与**必须有个分母**（openhuman、OpenBitFun）。语义分类见 `I29-token-usage-semantics-spec.md` §1.3、取舍理由见 §5.1，本节只记代码路径。

| 项目 | 关键代码 | 未知时的实际行为 |
| --- | --- | --- |
| codex | `codex-rs/tui/src/token_usage.rs` 的 `model_context_window: Option<i64>`；`chatwidget.rs` 的 `context_remaining_percent` / `context_used_tokens` | 两级解析（本次上报 → 配置），都没有即 `None`；`context_used_tokens` 在「百分比已知」时返回 `None`，因此**百分比与绝对数互斥**，窗口未知就只显示绝对 token 数 |
| codex（不一致处） | `chatwidget/status_controls.rs` | 状态栏走另一条路：窗口未知时 `return Some(100)`，即「剩余 100%」。同一产品内两处口径对不上 |
| deepseek-harness-orb | `packages/llm/token-meter/src/usage-projection.ts` 的 `contextPressure` fold | `contextWindow` 声明为 `optional().positive()`，`0` 在类型上非法；线上**省略字段**而非写 `null`；新事件不带窗口时**主动删除旧值**，并有测试 `removes an older capacity when the newest route advertises none` 锁定该行为 |
| openhuman（后端） | `crates/openhuman-core/src/threads/ops/usage.rs`、`inference/model_context.rs`、`web_chat/channel_event.rs` | 解析链末级 `unwrap_or(0)`；`context_window_for_model` 的文档注释写明「未知就跳过预算裁剪，不要猜」，但 DTO 边界把它压成 `0`，注释里直接承认「UI falls back to a default」 |
| openhuman（前端） | `app/src/store/chatRuntimeSlice.ts`、`app/src/features/conversations/aui/ContextUsage.tsx` | 仅在 `ctxWindow > 0` 时覆盖，于是**粘住上一个已知窗口**；再取不到时用 `DEFAULT_CONTEXT_WINDOW = 200_000` |
| OpenBitFun | `contracts/config-contracts/src/normalization.rs`、`conversion.rs`、`types.rs`、`agentic/execution/execution_engine.rs` | 配置里的 `0` 归一化为 `None`（方向正确），但转换期 `unwrap_or(DEFAULT_MODEL_CONTEXT_WINDOW_TOKENS)` 填成 `128_128`，于是运行时 `context_window == 0` 的 `ratio = 0` 分支成为**死代码** |

取舍：

- **采纳** codex 与 orb：未知保持 `NULL` / `undefined`，不画比例，只显示绝对量；并吸收 orb 的「未知覆盖已知」——新上下文未声明窗口时清空旧值，不沿用。
- **不采纳** openhuman 的 `0` 哨兵与前端 200k 兜底、OpenBitFun 的 `128_128` 填充：两者都把「猜一个分母」的责任从配置层转移到展示层，且假分母会同时污染压缩阈值与 offload 预算，不只是多画一根条。
- **注意** OpenBitFun 有一处比本仓库更准，值得单独吸收：它的压缩预算是 `W - output_reserve - safety_reserve`，而非 `W * 80%`。这正好对应本仓库 §3.3「降级预算用完整窗口、未预留输出侧 `max_tokens`」那条失真。

---

## 8 未被度量的部分

- system prompt 的单独占用（无法从 prompt 中拆分，只能靠上报总量推断）
- 单轮 `uncached` 输入与缓存命中的成本差异（只有 token 数，没有计价）
- 失败 attempt 的消耗（见 A11）
- 工具定义 token（当前未发送，无需度量；若未来启用需纳入 `I29-token-usage-semantics-spec.md` §3.1 的构成公式）
- 输出侧预留（`max_tokens`）与窗口余量关系（offload 未扣除）
- 窗口的第三方来源：provider `/models` 响应带的 `context_length` / `context_window` / `max_input_tokens` / `limit.context` / `top_provider.context_length`（OpenAI 兼容）、`context_window` / `max_input_tokens`（Anthropic）、`inputTokenLimit`（Gemini）已由发现层解析（T5.4），随 `models_json` 持久化；只有三个 provider 都未声明时才为 `NULL`，此时「未知窗口」只能靠本地配置补齐（见 §7.6）

---

## 附录 A 参考项目文件索引

| 参考项目 | 关键文件 | 内容 |
| --- | --- | --- |
| OpenBitFun | `adapters/ai-adapters/src/stream/types/openai.rs` | OpenAI usage 提取 + DeepSeek cache 优先策略 |
| OpenBitFun | `adapters/ai-adapters/src/stream/types/anthropic.rs` | Anthropic 三桶求和 + cache read / write 分离 |
| OpenBitFun | `adapters/ai-adapters/src/stream/types/gemini.rs` | Gemini usage + `thoughts_token_count` 修正 |
| OpenBitFun | `adapters/ai-adapters/src/stream/types/responses.rs` | Responses API usage + `cache_write_tokens` |
| OpenBitFun | `execution/agent-stream/src/unified.rs` | `UnifiedTokenUsage` 定义 |
| OpenBitFun | `services/services-core/src/token_usage/types.rs` | `TokenUsageRecord` / `ModelTokenStats` / `SessionTokenStats` |
| deepseek-harness-orb | `packages/llm/token-meter/src/types.ts` | `TokenMeasurement` / `TokenMeasurementBaseline` / `TokenSurfaceNode` |
| deepseek-harness-orb | `packages/llm/token-meter/src/projection.ts` | 三投影定义：`TokenUsageProjection` / `ContextPressureProjection` / `ContextBreakdownProjection` |
| deepseek-harness-orb | `packages/llm/token-meter/src/usage-projection.ts` | `tokenUsage` / `contextPressure` 的 fold 实现、`addReplacing`、surface delta |
| deepseek-harness-orb | `packages/llm/token-meter/src/breakdown-projection.ts` | `contextBreakdown` fold：system / tools / message 构成 |
| deepseek-harness-orb | `packages/llm/token-meter/src/turn-usage.ts` | `TurnTokenUsage` 跨 attempt 聚合与严格校验 |
| deepseek-harness-orb | `packages/llm/token-meter/src/estimate.ts` | 固定 4 字符/token + 结构开销（`BLOCK_OVERHEAD` / `ROLE_OVERHEAD`） |
| codex | `codex-rs/tui/src/token_usage.rs` | `TokenUsageInfo` 双字段 + `BASELINE_TOKENS = 12000` 有效窗口 |
| codex | `codex-rs/protocol/src/protocol.rs` | `append_last_usage`、`TokenUsageRecord` 三层记录 |
| openhuman | `crates/openhuman-core/src/threads/ops/usage.rs` | `ThreadTokenUsageResponse`：累计 + `last_turn_input_tokens` + `has_usage` |
| openhuman | `crates/openhuman-core/src/agent/context_breakdown.rs` | system / tools / history 构成拆解 RPC |
| OpenBitFun | `agentic/session/token_anchor.rs` | `TokenAnchor` 结构与前缀失效判定 |
| OpenBitFun | `agentic/session/context_usage.rs` | `SessionContextUsage` + `source: ModelRequest / ContextCompression` |
| OpenBitFun | `execution/agent-runtime/src/compression_prefetch.rs` | `PREFETCH_LEAD_TOKENS = 10_000` 两段式触发 |
| OpenBitFun | `assembly/core/src/service/token_usage/subscriber.rs` | `TokenUsageUpdated` 事件 → `record_usage` |
| OpenBitFun | `assembly/core/src/util/token_counter.rs` | ASCII 0.3 / 非 ASCII 0.6 + 消息级结构开销 |

## 附录 B 本应用文件索引

| 层 | 文件 | 职责 |
| --- | --- | --- |
| Provider 映射 | `src-tauri/crates/assistant-providers/src/types/unified.rs` | `UnifiedTokenUsage` 定义（6 字段） |
| | `src-tauri/crates/assistant-providers/src/types/chat_delta.rs` | `TokenUsage`（7 字段）+ unified→token 转换 |
| | `src-tauri/crates/assistant-providers/src/openai/stream.rs` | OpenAI ChatUsage → `UnifiedTokenUsage` |
| | `src-tauri/crates/assistant-providers/src/openai/types.rs` | `ChatUsage` 反序列化字段 |
| | `src-tauri/crates/assistant-providers/src/anthropic/stream.rs` | AnthropicUsage → `UnifiedTokenUsage` |
| | `src-tauri/crates/assistant-providers/src/gemini/stream.rs` | GeminiUsageMetadata → `UnifiedTokenUsage` |
| | `src-tauri/crates/assistant-providers/src/openai/responses_types.rs` | ResponsesUsage → `UnifiedTokenUsage` |
| | `src-tauri/crates/assistant-providers/src/stream/stats.rs` | `StreamStats`（仅日志，无持久化） |
| 请求组装 | `src-tauri/crates/assistant-agent/src/chat_request.rs` | system prompt 来源、附件投影、offload 调用 |
| | `src-tauri/crates/assistant-agent/src/prompt_builder.rs` | 历史消息过滤与组装 |
| | `src-tauri/crates/assistant-agent/src/request_projection.rs` | 估算常量、`estimate_total_tokens`（E2 投影估算）、超预算降级 |
| | `src-tauri/crates/assistant-agent/src/chat_options.rs` | 窗口注入、`max_tokens` 缺省 4096 |
| 运行时 | `src-tauri/crates/assistant-agent/src/chat_runtime.rs` | `StreamAccumulator` / `StreamOutcome` / `PartialOutcome` |
| | `src-tauri/crates/assistant-agent/src/compaction.rs` | 80% 阈值、摘要提示词、窗口解析 |
| 存储 | `src-tauri/crates/assistant-storage/src/agent_modes.rs` | `SUM(tokens)` 聚合、`TokenUsageSummary` |
| | `src-tauri/crates/assistant-storage/src/conversations/msg_ops.rs` | 消息读写、`tokens` 落点 |
| | `src-tauri/crates/assistant-storage/src/db.rs` | `messages` 表结构 |
| 命令层 | `src-tauri/src/protocol/dto/chat.rs` | `TokenUsageDto` / `TokenUsageSummaryDto` |
| | `src-tauri/src/cmd/conversation_utils.rs` | 窗口解析与缺省值 |
| | `src-tauri/src/cmd/compaction.rs` | 压缩命令入口 |
| 前端 | `src/services/chatOrchestrator.ts` | 流式 usage → `ContextUsage` |
| | `src/stores/chatStorePorts.ts` | `beginSend` / `finishStream` 的 usage 处理 |
| | `src/stores/chatConversationSlice.ts` | 切会话时重置与拉取 |
| | `src/chat/core/chatUtils.ts` | `fetchTokenUsage`、`resolveContextWindowTokens` |
| | `src/chat/core/chatTypes.ts` | `ContextUsage` / `TokenUsage` 类型 |
| | `src/chat/ChatView.tsx` | `ContextBar` 展示 |
| | `src/config/constants.ts` | 70% / 90% 阈值、80% 压缩阈值 |
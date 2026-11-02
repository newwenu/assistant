# Token Usage 改造 — 任务清单

> 语义：[I29-token-usage-semantics-spec.md](./I29-token-usage-semantics-spec.md)（唯一事实源）
> 现状：[I29-token-usage-analysis.md](./I29-token-usage-analysis.md)（A1–A20 现象）
> 契约：[I29-token-usage-ui-spec.md](./I29-token-usage-ui-spec.md)（P/W/Q/F 事实与 §7 验收）
> 子阶段：P1–P8（P = 子阶段，T = 任务，段号一一对应；本文即 I29 的任务清单）
> 顺序：P1 → P2 → P3 → P4 → P5；P6 可并行，但 P6 的 T5.2 / T5.5 依赖 P1 的 T0.3；P8 在 P7 之后，修复「provider 不报 usage 时有内容的会话看不到占用」（M16 / A20）；deferred 段的 T7 不参与本顺序
> 勾选条件：代码已改 **且** 判据已跑。条目末尾括号内为可回溯编号（`A` = 诊断 §5，`M` = 语义 §6）

## 子阶段划分

| 子阶段 | 内容 | 任务段 | 前置 |
| --- | --- | --- | --- |
| P1 | 类型与常量收敛 | T0 | — |
| P2 | 采集 | T1 | P1 |
| P3 | 持久化 | T2 | P2 |
| P4 | 聚合与 IPC | T3 | P3 |
| P5 | 前端展示 | T4 | P1、P4 |
| P6 | 阈值与估算 | T5 | P1（可与 P2–P5 并行） |
| P7 | 校验 | T6 | P1–P6 |
| P8 | 占用展示兜底 | T8 | P7（修复 M16 / A20） |

每阶段收口即跑该阶段判据；P7 只做汇总回归与契约 §7 逐条走查，不引入新实现。

---

## P1 / T0 类型与常量收敛

`src/chat/core/chatTypes.ts`、`src-tauri/crates/assistant-providers/src/types/{chat_delta.rs,config.rs}`、`src-tauri/crates/assistant-providers/src/model_catalog.rs`、`src-tauri/crates/assistant-providers/src/{base.rs,openai/{chat.rs,responses_stream.rs},anthropic/request.rs,gemini/{request.rs,code_assist.rs}}`、`src-tauri/crates/assistant-agent/src/{chat_options.rs,chat_request.rs,compaction.rs}`、`src-tauri/src/protocol/dto/{chat.rs,mod.rs}`、`src-tauri/src/cmd/{conversation.rs,conversation_utils.rs,compaction.rs,chat/{mod.rs,stream.rs}}`、`src-tauri/crates/assistant-storage/src/agent_modes.rs`、`src/stores/{chatStorePorts.ts,chatStore.ts,chatConversationSlice.ts}`、`src/services/chatOrchestrator.ts`

- [x] T0.1 `ContextUsage.maxTokens` / `percentage` 改 `number | undefined`，新增 `hasUsage: boolean`（无记录时占位「—」），`tsc` 通过（A6 A8 M9 契约 §4.3）
- [x] T0.2 `context_window_tokens` 改 `Option<u32>` + `skip_serializing_if`，未知序列化为缺省字段而非 `0`（A13 M4）
  - 同批透传：`ChatOptions` / `ChatUsage` / `unified_responses_into_chat_deltas` / 五个 provider 请求构造点 / `should_terminate` 签名（未知分支先返回「不可判定」，语义在 T5.5 收口）
  - 与 T0.1 同批：单改一侧会把 `undefined` 变回 `0`
- [x] T0.3 后端删「未知即 128k」兜底（3 处运行时：`conversation_utils` / `agent_modes` / `model_catalog`），未知返回 `None`；`types/config.rs` 中的同值断言属测试夹具，改为断言 `None`（A6）
- [x] T0.4 前端删窗口兜底（6 处），`128_000` 仅留设置页滑块上限；窗口改由后端随查询 / usage 返回，前端不再自行推导（A6 A8 契约 §4.4）
- [x] T0.5 未知覆盖已知：切会话 / 切模型未声明窗口时清空 `maxTokens`，不沿用旧值（A8 M11）

## P2 / T1 采集

`src-tauri/crates/assistant-agent/src/chat_runtime.rs`、`src-tauri/src/cmd/compaction.rs`

- [x] T1.1 `StreamAccumulator` 扩为全字段，`total` 优先取 provider 的 `total_tokens`；`total < P + C` 时回退 `P + C` 并置 `estimated`（A5 M12）
- [x] T1.2 `StreamOutcome` / `PartialOutcome` 携带 `P/C/CR/CW/RT/total/estimated` 七个字段 + 窗口（A10）
- [x] T1.3 流中断路径同样产出全字段 usage（A10）
- [x] T1.4 重试 attempt 各产一条记录，不覆盖（A11 M7）
- [x] T1.5 压缩请求 usage 不被丢弃，摘要消息 `tokens` 保持 `NULL`（成功与中断两条路径都不得写估算值，`save_compaction_partial` 同步改）（A12 M10）
- [x] T1.6 provider 适配层补齐可解析字段：`ChatUsage` 建模 `completion_tokens_details`，映射 `reasoning_token_count`，不再硬编码 `None`；同时声明 `cache_capable`（无缓存 / 读缓存 / 读写缓存），随记录落库（A15 M14）

## P3 / T2 持久化

`src-tauri/crates/assistant-storage/src/db.rs`（建表挂批处理；`migrations.rs` 是空壳，不可依赖）

- [x] T2.1 建 `token_usage_records` 表，字段按契约 §3.2（含 `cache_capable` / `estimated`），`IF NOT EXISTS` 幂等
- [x] T2.2 索引 `(conversation_id, branch_leaf_id, created_at DESC, id DESC)`
- [x] T2.3 三条派生查询：occupancy / cumulative / cacheHit；占用 = 最新 `kind = chat` 的 `prompt_tokens`，命中率分母 = `cache_capable = 1` 且任一缓存字段非 `NULL`（A2 A3 M5 M14）
- [x] T2.4 四个写入点：成功、中断、压缩、每次 attempt（A11 A12）
- [x] T2.5 存量无记录显示「—」，不回填估算（M3）

  口径修订（T8）：不回填记录表不变；展示层改为「分支有消息则按投影估算显示 `~N`」，空分支窗口已知显示 `0 / 窗口`、窗口未知显示「—」。见契约 §3.4 第 4–5 级与 §6。
- [x] T2.6 占用查询按分支过滤（`branch_leaf_id`）、补 `id DESC` 二级键、优先非估算记录；累计不按分支过滤（A14 A19 M3）

## P4 / T3 聚合与 IPC

`src-tauri/src/protocol/dto/chat.rs`、`src-tauri/crates/assistant-storage/src/agent_modes.rs`、`src/chat/core/{chatTypes.ts,chatUtils.ts}`

- [x] T3.1 `TokenUsageSummaryDto` 拆 `occupancy` / `cumulative` / `cache` 三组，前后端同批（A1）
- [x] T3.2 占用改查记录表，删 `SUM(COALESCE(tokens, ...))`（A2 A3 M1）
- [x] T3.3 `messages.tokens` 收敛为「整轮 total」，占用逻辑不再读该列（A4）
- [x] T3.4 累计 = `Σ(P + C)`，含重试与压缩请求（M7 M10）

## P5 / T4 前端展示

`src/chat/core/chatUtils.ts`、`src/stores/{chatStorePorts.ts,chatConversationSlice.ts}`、`src/chat/ChatView.tsx`、i18n 资源

- [x] T4.1 `fetchTokenUsage` 改为从 `occupancy` / `cumulative` / `cache` 三组写入 `usedTokens`、`cumulativeTokens` 与 breakdown，与流式写入同源（A1 F3 F5）
- [x] T4.2 `beginSend` 不清零 `usedTokens`（A7）
- [x] T4.3 切会话不先写窗口缺省值（A8）
- [x] T4.4 ContextBar 三段进度条（命中 / 写入 / 未缓存）+ Tooltip 六项（A10 F1 F6）
- [x] T4.5 窗口未知只显示绝对 token 数，百分比与绝对数互斥（契约 §4.4 §5.1）
- [x] T4.6 文案替换，全仓无语义 §7 禁令写法（A9）
- [x] T4.7 `is_full` 的文案与命名不出现容量措辞（「窗口已满」），统一为「已存档 / 已终止」（A18）

## P6 / T5 阈值与估算

`src-tauri/crates/assistant-agent/src/{compaction.rs,request_projection.rs}`、估算（E1/E2）

- [x] T5.1 压缩阈值改 `P >= W - max_tokens - safety`，同步改名或改写 `threshold_percent` 语义（M8 M15）
- [x] T5.2 offload 预算改 `W - max_tokens`，不用完整窗口；`W` 未知时预算不可判定（依赖 T0.3）（诊断 §3.3）
- [x] T5.3 估算给 CJK 加权并强制标 `estimated`；估算值不得覆盖上报值，仅在分支上无任何非估算记录时才可兜底进入 `O`（M2 M3）

  落地：`assistant-agent::token_estimate::estimate_text_tokens`（ASCII 1/4、CJK 1、其余 1/2，`div_ceil(4)`）为唯一口径，`chat_runtime` 以 `pub use` 转发同名符号，`request_projection` 全部调用点直接复用该模块；`cmd::token_usage_record::normalize_record_tokens` 在 `prompt_tokens` 缺失时按 `total - completion` 回退并强制 `estimated = true`。单测：`chat_runtime_spec` 3 例（估算加权）+ `token_usage_record` 4 例（回退与标记）。
- [x] T5.4 解析 provider `/models` 的窗口字段，拿不到仍为 `NULL`

  落地：三个适配器的发现层各自建模并解析，统一经 `resolve_context_window(model_id, stored, api_context)` 合并：
  - OpenAI（`src-tauri/crates/assistant-providers/src/openai/types.rs`）：`context_window` → `context_length` → `max_input_tokens` → `limit.context` → `top_provider.context_length`
  - Anthropic（`src-tauri/crates/assistant-providers/src/anthropic/types.rs`）：`context_window` → `max_input_tokens` → `limit.context`
  - Gemini（`src-tauri/crates/assistant-providers/src/gemini/types.rs`）：`inputTokenLimit`（camelCase，与 Google 响应同形）

  三者均 `find(|&v| v > 0)`，`0` 与缺失等同视为未声明 → `None` → 序列化为缺省字段（T0.2 契约），不回落硬编码常量。单测：`openai/types.rs` 5 例、`anthropic/types.rs` 4 例、`gemini/discovery.rs` 2 例，覆盖字段优先级、`0` 归一、缺失保持 `None`。
- [x] T5.5 窗口未知时压缩与降级判定返回「不可判定」，不得当成「不超限」静默跳过（A16 M13）
- [x] T5.6 超窗错误（`context_length_exceeded` 等）给出可操作提示，不静默失败（A17）

## P7 / T6 校验

- [x] T6.1 单测：累加器全字段、`total` 优先 provider、重试两条记录、压缩一条记录

  落地：`crates/assistant-agent/tests/chat_runtime_spec.rs` 新增 7 例（`b7_accumulator_*` / `estimate_*`）；`src/cmd/token_usage_record.rs` 7 例覆盖全字段透传、压缩记录、重试不覆盖。
- [x] T6.2 单测：命中率分母只含「`cache_capable = 1` 且任一缓存字段非 `NULL`」的请求（M5 M14）

  落地：`crates/assistant-storage/tests/token_usage.rs::cache_denominator_requires_capable_and_reported_field`。
- [x] T6.3 前端测试：切会话前后占用一致、发送不归零、未知窗口显示绝对数、无记录显示「—」（`hasUsage = false`）

  口径修订（T8）：无记录但分支有消息的场景改为断言 `~N` 且 `estimated = true`；仅空分支断言 `hasUsage = false`。

  落地：`src/chat/core/__tests__/tokenUsage.test.ts` 9 例（映射与 fetch 逻辑）+ `src/stores/__tests__/chatTokenUsage.test.ts` 7 例（切会话、切回、切分支后重查占用、未知窗口、无记录、`finishStream` 有/无 usage），全部通过。
- [x] T6.4 逐条走查契约 §7 的验收判据（当时 15 条；P8 新增的 16–18 由 T8.7 回归）

  走查结论：14 条直接满足；第 7 条走查中发现 `resolve_final_tokens` / `resolve_partial_tokens` 在无 usage 时把估算值写进 `messages.tokens`，与「无 usage 时该列为 `NULL`」冲突，已改为只接受 provider 上报的 `total_tokens`（估算仅用于展示兜底，不再冒充账单口径）。第 8 条当时按「一律显示「—」」走查，P8 将其收窄为「仅空分支显示「—」」（M16 / A20）。

  | §7 | 判据 | 证据 |
  | --- | --- | --- |
  | 1 | 流结束与切走切回同源 | `chatStorePorts.finishStream` 用 `promptTokens`；切会话 `fetchTokenUsage` → `occupancy.usedTokens`（同一条 chat 记录的 `prompt_tokens`）；`chatTokenUsage.test.ts` 两例 |
  | 2 | 10 轮 `O = 20k` / `A = 120k` | `token_usage.rs::ten_round_scenario_keeps_occupancy_and_cumulative_apart` |
  | 3 | 缓存 / 推理未上报为 `undefined` | `TokenUsage` 可选字段直接透传，无 `?? 0` 兜底；`tokenUsage.test.ts` 映射用例 |
  | 4 | 命中率分母谓词 | `token_usage.rs::cache_denominator_requires_capable_and_reported_field` |
  | 5 | 1 次聊天 / 重试 2 条 / 压缩 1 条 | `token_usage_record.rs` 记录构建 7 例 + `chat_runtime_spec.rs` `attempt_usages` |
  | 6 | 压缩计入累计不计占用 | `token_usage.rs::compaction_record_excluded_from_occupancy_but_counted_in_cumulative` |
  | 7 | `messages.tokens` 与记录 total 一致，无 usage 为 `NULL` | 本次修正 `resolve_final_tokens` / `resolve_partial_tokens` 不再写估算；`Accumulator.apply` 保证有 P/C 时 `total` 必为 `Some`；`protocolContract.test.ts` 断言 `null` |
  | 8 | 存量库升级：有消息显示 `~N`、空会话窗口已知显示 `0 / 窗口`、窗口未知显示「—」 | `db.rs` 建表 `IF NOT EXISTS`；`token_usage.rs::summary_without_records_has_no_occupancy` 覆盖空分支；有消息分支由 T8.2 / T8.3 的投影估算兜底（`estimated = true`）；`tokenUsage.test.ts` 与 `chatTokenUsage.test.ts` 覆盖空分支保留窗口 |
  | 9 | 无缓存字段退化为单色 | `ContextBar` 分段宽度 `hit/write` 取 `?? 0`，`uncached = used`，三段布局不变 |
  | 10 | 占用下降不残留旧值 | 新增 `refreshTokenUsage`，`MessagesChanged`（非 `stream_finished`）触发重查；`chatTokenUsage.test.ts::refreshes occupancy after a branch switch` |
  | 11 | 窗口未知只报绝对数、不沿用旧窗口 | `contextUsageFromSummary` 直取后端 `contextWindowTokens`；`updateModel` 重置 `contextUsage` 时丢弃 `maxTokens` / `percentage`；`tokenUsage.test.ts` 未知窗口用例 |
  | 12 | 切分支占用跟随、累计不降 | `token_usage.rs::occupancy_follows_branch`；`get_cumulative` 仅按 `conversation_id` 聚合，无分支过滤 |
  | 13 | 同毫秒按 `id` 取后者 | `token_usage.rs::same_millisecond_breaks_tie_by_id` |
  | 14 | 未知窗口不触发压缩、超窗给提示 | `compaction.rs` 返回 `CompactDecision::Undetermined`（不当作「不超限」）；`ProviderError::ContextOverflow` → `ChatErrorCategory.ContextOverflow` → `chat.error.category_context_overflow_message` 给出「压缩 / 新建对话 / 换更大窗口模型」 |
  | 15 | `threshold_percent` 迁移或明确报错 | `app_conf.rs` 旧键 `compact_threshold_percent` 迁移到 `compact_threshold_percent_of_budget` 并告警，显式新值优先；`app_conf.rs` 测试两例 |
- [x] T6.5 单测：切分支后占用跟随分支、同毫秒两条记录按 `id` 取后者、估算记录不覆盖上报值（A14 A19 M3）

  落地：`crates/assistant-storage/tests/token_usage.rs` 12 例，含分支跟随、跨会话隔离、同毫秒按 `id` 取后者、估算不覆盖上报、估算兜底、压缩排除占用但计入累计、重试累计、未知窗口只报绝对数、10 轮场景 `O`/`A` 分离。

---

## P8 / T8 占用展示兜底（修复「不展示上下文长度」）

`src-tauri/crates/assistant-agent/src/{request_projection.rs,chat_request.rs}`、`src-tauri/src/cmd/conversation.rs`、`src-tauri/src/protocol/dto/chat.rs`、`src/chat/core/{chatTypes.ts,chatUtils.ts}`、`src/chat/context/ContextBar.tsx`、i18n 资源

背景：T2.5 与契约旧版 §6 把「无记录」一律渲染为「—」，导致 provider 不报 usage 的会话**永远看不到占用**（M16 / A20）。本次改为四级来源：上报非估算 → 上报估算 → 分支投影估算 → 无内容。估算只进展示，**不进记录表、不进 `messages.tokens`、不进累计 `A`**（T5.3 的落库口径不变）。

- [x] T8.1 `estimate_total_tokens` 提升为 `pub`（`request_projection.rs`），`offload_if_over_budget` 改为调用同一符号，保证全仓只有一套投影口径（E1/E2）
- [x] T8.2 新增 `estimate_branch_tokens(detail, provider_row, attachment_store)`：复用 `project_chat_messages` 的消息投影，只取 `chat_messages` 求和，不构造 adapter、不发起请求
- [x] T8.3 `get_conversation_token_usage` 增加第三级兜底：`get_summary` 的 occupancy 为空时调用 T8.2，返回 `occupancy.used_tokens` + `estimated = true` + `estimatedSource = projection`；**不写记录表**
- [x] T8.4 DTO：`occupancy` 增加 `estimated: bool` + `estimatedSource: 'record' | 'projection' | null`；`cumulative` 在该会话零记录时返回缺省字段而非 `0`（M4 / M16）
- [x] T8.5 `ContextUsage` 增加 `estimated: boolean` 与 `estimatedSource?: 'record' | 'projection'`，`hasUsage` 语义收紧为「占用可展示」；不变式 `estimated = true` ⇒ `hasUsage = true` 且 `estimatedSource` 有值
- [x] T8.6 `ContextBar` 估算值加 `~` 前缀；Tooltip 来源行按 `estimated` / `estimatedSource` 四选一（「provider 上报」/「本地估算（provider 未上报输入 token）」/「本地估算（provider 未上报 usage）」/「尚无消息」）；零记录时累计显示「—」；空分支窗口已知时显示 `0 / 窗口（0%）`、窗口未知才显示「—」（契约 §5.1 §6）
- [x] T8.7 单测：无记录但有消息 → `~N` 且不落库；上报值存在时估算不覆盖；空分支窗口已知 → `0 / 窗口`、窗口未知 → 「—」；同一分支新增消息后估算值随之变化；`messages.tokens` 仍为 `NULL`（契约 §7 判据 16–18）

---

## 暂不实施（deferred）

### T7 续接（不纳入当前范围）

`src/chat/compact/CompactBanner.tsx`、`src/stores/chatCompactSlice.ts`、`src/services/chatLifecycleEvents.ts`、`src-tauri/src/cmd/conversation.rs`、`src/i18n/locales/{zh-CN,en-US}.json`

I7 原始设计是「停机 + 存档 + 续接」，当前只交付到「停机 + 存档」（诊断 §3.5）。以下条目不参与 P1–P6 的顺序与勾选，且**不是任何任务的前置**：T1.5（记录压缩请求 usage）与 T2.4（四个写入点）都不依赖续接。

- [ ] T7.1 建会话时接受初始消息：`create_conversation` 增加首条 user 内容入参（或复用 `fork_to_new_conversation` 的复制语义）
- [ ] T7.2 「继续对话」按钮改为「新建会话 + 首条 user 注入摘要」，替换现存的空建会话行为
- [ ] T7.3 摘要注入：`compactSummary` 当前只用于存档与展示（`MessageBubble` 渲染），若续接则需注入新会话首条 user 消息
- [ ] T7.4 补 `chat.compact.continue_conversation` 键并与 `chat.compact.new_conversation` 区分语义（zh-CN / en-US 同步）
- [ ] T7.5 续接会话首次请求前占用显示「—」，首次请求后按该请求 `prompt_tokens` 锚定，不沿用原会话的 `O`（M6）
- [ ] T7.6 若续接上线，文案按「原会话 / 续接会话」两语境选行（契约 §5.4 的暂不生效块）

> `prompt_builder.rs` 不在 T7 范围内，也不需要改：摘要作为新会话首条 `user` 消息落库后，`prepare_messages` 会自然带上，与它的 `compact_summary` 过滤无关（诊断 §3.5）。
# Token Usage 改造 — 任务清单

> 语义：[I29-token-usage-semantics-spec.md](./I29-token-usage-semantics-spec.md)（唯一事实源）
> 现状：[I29-token-usage-analysis.md](./I29-token-usage-analysis.md)（A1–A19 现象）
> 契约：[I29-token-usage-ui-spec.md](./I29-token-usage-ui-spec.md)（P/W/Q/F 事实与 §7 验收）
> 子阶段：P1–P7（P = 子阶段，T = 任务，段号一一对应；本文即 I29 的任务清单）
> 顺序：P1 → P2 → P3 → P4 → P5；P6 可并行，但 P6 的 T5.2 / T5.5 依赖 P1 的 T0.3。deferred 段的 T7 不参与本顺序
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

每阶段收口即跑该阶段判据；P7 只做汇总回归与契约 §7 逐条走查，不引入新实现。

---

## P1 / T0 类型与常量收敛

`src/chat/core/chatTypes.ts`、`src-tauri/crates/assistant-providers/src/types/{chat_delta.rs,config.rs}`、`src-tauri/crates/assistant-providers/src/model_catalog.rs`、`src-tauri/crates/assistant-providers/src/{base.rs,openai/{chat.rs,responses_stream.rs},anthropic/request.rs,gemini/{request.rs,code_assist.rs}}`、`src-tauri/crates/assistant-agent/src/{chat_options.rs,chat_request.rs,compaction.rs}`、`src-tauri/src/protocol/dto/{chat.rs,mod.rs}`、`src-tauri/src/cmd/{conversation.rs,conversation_utils.rs,compaction.rs,chat/{mod.rs,stream.rs}}`、`src-tauri/crates/assistant-storage/src/agent_modes.rs`、`src/stores/{chatStorePorts.ts,chatStore.ts,chatConversationSlice.ts}`、`src/services/chatOrchestrator.ts`

- [ ] T0.1 `ContextUsage.maxTokens` / `percentage` 改 `number | undefined`，新增 `hasUsage: boolean`（无记录时占位「—」），`tsc` 通过（A6 A8 M9 契约 §4.3）
- [ ] T0.2 `context_window_tokens` 改 `Option<u32>` + `skip_serializing_if`，未知序列化为缺省字段而非 `0`（A13 M4）
  - 同批透传：`ChatOptions` / `ChatUsage` / `unified_responses_into_chat_deltas` / 五个 provider 请求构造点 / `should_terminate` 签名（未知分支先返回「不可判定」，语义在 T5.5 收口）
  - 与 T0.1 同批：单改一侧会把 `undefined` 变回 `0`
- [ ] T0.3 后端删「未知即 128k」兜底（3 处运行时：`conversation_utils` / `agent_modes` / `model_catalog`），未知返回 `None`；`types/config.rs` 中的同值断言属测试夹具，改为断言 `None`（A6）
- [ ] T0.4 前端删窗口兜底（6 处），`128_000` 仅留设置页滑块上限；窗口改由后端随查询 / usage 返回，前端不再自行推导（A6 A8 契约 §4.4）
- [ ] T0.5 未知覆盖已知：切会话 / 切模型未声明窗口时清空 `maxTokens`，不沿用旧值（A8 M11）

## P2 / T1 采集

`src-tauri/crates/assistant-agent/src/chat_runtime.rs`、`src-tauri/src/cmd/compaction.rs`

- [ ] T1.1 `StreamAccumulator` 扩为全字段，`total` 优先取 provider 的 `total_tokens`；`total < P + C` 时回退 `P + C` 并置 `estimated`（A5 M12）
- [ ] T1.2 `StreamOutcome` / `PartialOutcome` 携带 `P/C/CR/CW/RT/total/estimated` 七个字段 + 窗口（A10）
- [ ] T1.3 流中断路径同样产出全字段 usage（A10）
- [ ] T1.4 重试 attempt 各产一条记录，不覆盖（A11 M7）
- [ ] T1.5 压缩请求 usage 不被丢弃，摘要消息 `tokens` 保持 `NULL`（成功与中断两条路径都不得写估算值，`save_compaction_partial` 同步改）（A12 M10）
- [ ] T1.6 provider 适配层补齐可解析字段：`ChatUsage` 建模 `completion_tokens_details`，映射 `reasoning_token_count`，不再硬编码 `None`；同时声明 `cache_capable`（无缓存 / 读缓存 / 读写缓存），随记录落库（A15 M14）

## P3 / T2 持久化

`src-tauri/crates/assistant-storage/src/db.rs`（建表挂批处理；`migrations.rs` 是空壳，不可依赖）

- [ ] T2.1 建 `token_usage_records` 表，字段按契约 §3.2（含 `cache_capable` / `estimated`），`IF NOT EXISTS` 幂等
- [ ] T2.2 索引 `(conversation_id, branch_leaf_id, created_at DESC, id DESC)`
- [ ] T2.3 三条派生查询：occupancy / cumulative / cacheHit；占用 = 最新 `kind = chat` 的 `prompt_tokens`，命中率分母 = `cache_capable = 1` 且任一缓存字段非 `NULL`（A2 A3 M5 M14）
- [ ] T2.4 四个写入点：成功、中断、压缩、每次 attempt（A11 A12）
- [ ] T2.5 存量无记录显示「—」，不回填估算（M3）
- [ ] T2.6 占用查询按分支过滤（`branch_leaf_id`）、补 `id DESC` 二级键、优先非估算记录；累计不按分支过滤（A14 A19 M3）

## P4 / T3 聚合与 IPC

`src-tauri/src/protocol/dto/chat.rs`、`src-tauri/crates/assistant-storage/src/agent_modes.rs`、`src/chat/core/{chatTypes.ts,chatUtils.ts}`

- [ ] T3.1 `TokenUsageSummaryDto` 拆 `occupancy` / `cumulative` / `cache` 三组，前后端同批（A1）
- [ ] T3.2 占用改查记录表，删 `SUM(COALESCE(tokens, ...))`（A2 A3 M1）
- [ ] T3.3 `messages.tokens` 收敛为「整轮 total」，占用逻辑不再读该列（A4）
- [ ] T3.4 累计 = `Σ(P + C)`，含重试与压缩请求（M7 M10）

## P5 / T4 前端展示

`src/chat/core/chatUtils.ts`、`src/stores/{chatStorePorts.ts,chatConversationSlice.ts}`、`src/chat/ChatView.tsx`、i18n 资源

- [ ] T4.1 `fetchTokenUsage` 改为从 `occupancy` / `cumulative` / `cache` 三组写入 `usedTokens`、`cumulativeTokens` 与 breakdown，与流式写入同源（A1 F3 F5）
- [ ] T4.2 `beginSend` 不清零 `usedTokens`（A7）
- [ ] T4.3 切会话不先写窗口缺省值（A8）
- [ ] T4.4 ContextBar 三段进度条（命中 / 写入 / 未缓存）+ Tooltip 六项（A10 F1 F6）
- [ ] T4.5 窗口未知只显示绝对 token 数，百分比与绝对数互斥（契约 §4.4 §5.1）
- [ ] T4.6 文案替换，全仓无语义 §7 禁令写法（A9）
- [ ] T4.7 `is_full` 的文案与命名不出现容量措辞（「窗口已满」），统一为「已存档 / 已终止」（A18）

## P6 / T5 阈值与估算

`src-tauri/crates/assistant-agent/src/{compaction.rs,request_projection.rs}`、估算三处（E1/E2/E3）

- [ ] T5.1 压缩阈值改 `P >= W - max_tokens - safety`，同步改名或改写 `threshold_percent` 语义（M8 M15）
- [ ] T5.2 offload 预算改 `W - max_tokens`，不用完整窗口；`W` 未知时预算不可判定（依赖 T0.3）（诊断 §3.3）
- [ ] T5.3 估算给 CJK 加权并强制标 `estimated`；估算值不得覆盖上报值，仅在分支上无任何非估算记录时才可兜底进入 `O`（M2 M3）
- [ ] T5.4 （可选）解析 provider `/models` 的 `context_length`，拿不到仍为 `NULL`
- [ ] T5.5 窗口未知时压缩与降级判定返回「不可判定」，不得当成「不超限」静默跳过（A16 M13）
- [ ] T5.6 超窗错误（`context_length_exceeded` 等）给出可操作提示，不静默失败（A17）

## P7 / T6 校验

- [ ] T6.1 单测：累加器全字段、`total` 优先 provider、重试两条记录、压缩一条记录
- [ ] T6.2 单测：命中率分母只含「`cache_capable = 1` 且任一缓存字段非 `NULL`」的请求（M5 M14）
- [ ] T6.3 前端测试：切会话前后占用一致、发送不归零、未知窗口显示绝对数、无记录显示「—」（`hasUsage = false`）
- [ ] T6.4 逐条走查契约 §7 的 15 条验收判据
- [ ] T6.5 单测：切分支后占用跟随分支、同毫秒两条记录按 `id` 取后者、估算记录不覆盖上报值（A14 A19 M3）

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
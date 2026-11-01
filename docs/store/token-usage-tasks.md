# Token Usage 改造 — 任务清单

> 语义：[token-usage-semantics.md](./token-usage-semantics.md)（唯一事实源）
> 现状：[token-usage-analysis.md](./token-usage-analysis.md)（A1–A13 现象）
> 契约：[token-usage-ui-spec.md](./token-usage-ui-spec.md)（P/W/Q/F 事实与 §7 验收）
> 顺序：T0 → T1 → T2 → T3 → T4；T5 可并行，但 T5.1 依赖 T0.3
> 勾选条件：代码已改 **且** 判据已跑。条目末尾括号内为可回溯编号（`A` = 诊断 §5，`M` = 语义 §6）

---

## T0 类型与常量收敛

`src/chat/core/chatTypes.ts`、`src-tauri/crates/assistant-providers/src/types/{chat_delta.rs,config.rs}`、`src-tauri/crates/assistant-providers/src/model_catalog.rs`、`src-tauri/src/protocol/dto/chat.rs`、`src-tauri/src/cmd/conversation_utils.rs`、`src-tauri/crates/assistant-storage/src/agent_modes.rs`、`src/stores/{chatStorePorts.ts,chatStore.ts,chatConversationSlice.ts}`、`src/services/chatOrchestrator.ts`

- [ ] T0.1 `ContextUsage.maxTokens` / `percentage` 改 `number | undefined`，`tsc` 通过（A6 A8 M9）
- [ ] T0.2 `context_window_tokens` 改 `Option<u32>` + `skip_serializing_if`，未知序列化为缺省字段而非 `0`（A13 M4）
  - 与 T0.1 同批：单改一侧会把 `undefined` 变回 `0`
- [ ] T0.3 后端删「未知即 128k」兜底（4 处），未知返回 `None`（A6）
- [ ] T0.4 前端删窗口兜底（6 处），`128_000` 仅留设置页滑块上限（A6 A8）
- [ ] T0.5 未知覆盖已知：切会话 / 切模型未声明窗口时清空 `maxTokens`，不沿用旧值（A8 M11）

## T1 采集

`src-tauri/crates/assistant-agent/src/chat_runtime.rs`、`src-tauri/src/cmd/compaction.rs`

- [ ] T1.1 `StreamAccumulator` 扩为全字段，`total` 优先取 provider 的 `total_tokens`（A5）
- [ ] T1.2 `StreamOutcome` / `PartialOutcome` 携带 5 个 usage 字段 + 窗口（A10）
- [ ] T1.3 流中断路径同样产出全字段 usage（A10）
- [ ] T1.4 重试 attempt 各产一条记录，不覆盖（A11 M7）
- [ ] T1.5 压缩请求 usage 不被丢弃，摘要消息 `tokens` 保持 `NULL`（A12 M10）

## T2 持久化

`src-tauri/crates/assistant-storage/src/db.rs`（建表挂批处理；`migrations.rs` 是空壳，不可依赖）

- [ ] T2.1 建 `token_usage_records` 表，字段按契约 §3.2，`IF NOT EXISTS` 幂等
- [ ] T2.2 索引 `(conversation_id, created_at DESC)`
- [ ] T2.3 三条派生查询：occupancy / cumulative / cacheHit；占用 = 最新 `kind = chat` 的 `prompt_tokens`（A2 A3 M5）
- [ ] T2.4 四个写入点：成功、中断、压缩、每次 attempt（A11 A12）
- [ ] T2.5 存量无记录显示「—」，不回填估算（M3）

## T3 聚合与 IPC

`src-tauri/src/protocol/dto/chat.rs`、`src-tauri/crates/assistant-storage/src/agent_modes.rs`、`src/chat/core/{chatTypes.ts,chatUtils.ts}`

- [ ] T3.1 `TokenUsageSummaryDto` 拆 `occupancy` / `cumulative` / `cache` 三组，前后端同批（A1）
- [ ] T3.2 占用改查记录表，删 `SUM(COALESCE(tokens, ...))`（A2 A3 M1）
- [ ] T3.3 `messages.tokens` 收敛为「整轮 total」，占用逻辑不再读该列（A4）
- [ ] T3.4 累计 = `Σ(P + C)`，含重试与压缩请求（M7 M10）

## T4 前端展示

`src/chat/core/chatUtils.ts`、`src/stores/{chatStorePorts.ts,chatConversationSlice.ts}`、`src/chat/ChatView.tsx`、i18n 资源

- [ ] T4.1 `fetchTokenUsage` 只写 occupancy + cumulative，不再写 `usedTokens`（A1 F3 F5）
- [ ] T4.2 `beginSend` 不清零 `usedTokens`（A7）
- [ ] T4.3 切会话不先写窗口缺省值（A8）
- [ ] T4.4 ContextBar 三段进度条（命中 / 写入 / 未缓存）+ Tooltip 六项（A10 F1 F6）
- [ ] T4.5 窗口未知只显示绝对 token 数，百分比与绝对数互斥（契约 §4.4 §5.1）
- [ ] T4.6 文案替换，全仓无语义 §7 禁令写法（A9）

## T5 阈值与估算

`src-tauri/crates/assistant-agent/src/{compaction.rs,request_projection.rs}`、估算三处（E1/E2/E3）

- [ ] T5.1 压缩阈值改 `P >= W - max_tokens - safety`，同步改写 `threshold_percent` 语义（M8）
- [ ] T5.2 offload 预算改 `W - max_tokens`，不用完整窗口（诊断 §3.3）
- [ ] T5.3 估算要么给 CJK 加权，要么强制标 `estimated` 且永不进入占用分子（M3）
- [ ] T5.4 （可选）解析 provider `/models` 的 `context_length`，拿不到仍为 `NULL`

## T6 校验

- [ ] T6.1 单测：累加器全字段、`total` 优先 provider、重试两条记录、压缩一条记录
- [ ] T6.2 单测：命中率分母只含「任一缓存字段非 `NULL`」的请求（M5）
- [ ] T6.3 前端测试：切会话前后占用一致、发送不归零、未知窗口显示绝对数
- [ ] T6.4 逐条走查契约 §7 的 11 条验收判据
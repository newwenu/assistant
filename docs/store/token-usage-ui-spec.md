# Token Usage 持久化与 UI 优化 — 完整 Specification

> 阶段：独立（依赖 Phase 1/2 已完成）
> 状态：设计稿 v2，未实施
> 前置文档：`docs/store/token-usage-analysis.md`
> 基线口径：本文以当前仓库代码为准

---

## 0. 结论速览

| # | 结论 |
|---|---|
| 1 | `messages` 表只有 `tokens`（INTEGER，单个 total），`prompt_tokens` / `completion_tokens` / `cache_read_tokens` / `cache_write_tokens` / `reasoning_tokens` 全部丢失 |
| 2 | `StreamOutcome` 含 `total_tokens` + `prompt_tokens`，但 `completion_tokens` / cache / reasoning 不在其中——这些字段仅在流式 `TokenUsage`（chat_delta）里出现，流结束后丢弃 |
| 3 | 前端 `ContextUsage` 只有 3 个字段（usedTokens / maxTokens / percentage），`ContextBar` 仅展示总量 + 进度条 |
| 4 | 本文同时覆盖持久化（messages 表扩展 + TokenUsage 累计 → StreamOutcome → DB）和 UI（ContextBar 分段着色 + 会话切换展示 breakdown） |

---

## 1. 现状事实

### 1.1 持久化层：只存了 1 个数字

| # | 事实 | 位置 |
|---|---|---|
| P1 | `messages` 表 `tokens` 列存的是 `total_tokens`（或字符估算回退值） | `db.rs` L49 |
| P2 | `MessageRow.tokens: Option<i64>` — 仅一个字段 | `conversations/mod.rs` L38 |
| P3 | `update_message_content()` 也只能更新 1 个 `tokens` | `msg_ops.rs` L53 |
| P4 | `MSG_SELECT` 仅含 `tokens`，无 breakdown 列 | `conversations/mod.rs` L12–13 |

### 1.2 写入路径：prompt_tokens 被丢弃

| # | 事实 | 位置 |
|---|---|---|
| W1 | `StreamOutcome` 同时持有 `total_tokens: Option<u32>` 和 `prompt_tokens: Option<u32>` | `chat_runtime.rs` L13–14 |
| W2 | `resolve_final_tokens()` 只取 `total_tokens` | `chat_runtime.rs` L396–413 |
| W3 | `DbWriteMsg` 只传 `tokens: Option<i64>`（即 total） | `stream.rs` L78 |
| W4 | `prompt_tokens` 仅用于 compaction 判断，判断完丢弃 | `mod.rs` L234 |
| W5 | `completion_tokens` / cache / reasoning **不在 StreamOutcome 中**——这些字段仅出现在流式 delta 的 `TokenUsage` 里，流结束后无存储 | `chat_runtime.rs` L10–19, `chat_delta.rs` L103–114 |
| W6 | `PartialOutcome`（流中断）也无这些字段 | `chat_runtime.rs` L21–28 |

### 1.3 聚合查询：粗暴 SUM

| # | 事实 | 位置 |
|---|---|---|
| Q1 | `TokenUsageSummary` 仅 3 字段：`total_tokens` / `context_window_tokens` / `percentage` | `agent_modes.rs` L26–30 |
| Q2 | SQL：`SUM(COALESCE(tokens, (LENGTH(content)+3)/4+4))` | `agent_modes.rs` L282 |
| Q3 | `TokenUsageSummaryDto` 仅 3 字段 | `protocol/dto/chat.rs` L202–206 |

### 1.4 前端：全数据已到但只用 1 个字段

| # | 事实 | 位置 |
|---|---|---|
| F1 | `TokenUsageDto` 含全部 6 字段（prompt/completion/total/cache_read/cache_write/reasoning） | `protocol/dto/chat.rs` L172–184 |
| F2 | 流式 delta 携带完整 `usage: Option<TokenUsageDto>` | `ChatDeltaPayloadDto.usage` |
| F3 | 前端 `chatOrchestrator.ts` 只取 `promptTokens` → `ContextUsage.usedTokens` | `chatOrchestrator.ts` L403–414 |
| F4 | `ContextUsage` 仅 3 字段 | `chatTypes.ts` |
| F5 | `fetchTokenUsage()` 将 `TokenUsageSummary`（3 字段）映射到 `ContextUsage` | `chatUtils.ts` L38–43 |
| F6 | `ContextBar` 只显示总量 + 进度条 + 百分比 | `ChatView.tsx` L40–89 |

---

## 2. 数据流全貌（改后目标）

```
Provider API
  │
  ▼
UnifiedTokenUsage (6 fields)
  │
  ├─▶ TokenUsage (chat_delta, 7 fields) ──▶ streaming delta ──▶ ContextUsage (7+ fields) ──▶ ContextBar
  │
  └─▶ StreamOutcome (8 fields) ──▶ DbWriteMsg ──▶ messages table (6 token columns)
                                                         │
                                                         ▼
                                              TokenUsageSummary (6+ fields) ──▶ fetchTokenUsage() ──▶ ContextUsage
```

### 2.1 各层结构（改后）

| 层 | 结构 | 新增字段 |
|----|------|----------|
| `StreamOutcome` | 原 8 字段 | `completion_tokens: Option<u32>` / `cache_read_tokens: Option<u32>` / `cache_write_tokens: Option<u32>` / `reasoning_tokens: Option<u32>` |
| `DbWriteMsg` | 原 5 字段 | 同上 4 个 + `prompt_tokens: Option<i64>` |
| `MessageRow` | 原 13 字段 | `prompt_tokens: Option<i64>` / `completion_tokens: Option<i64>` / `cache_read_tokens: Option<i64>` / `cache_write_tokens: Option<i64>` / `reasoning_tokens: Option<i64>` |
| `messages` 表 | 原 14 列 | 新增 5 列：`prompt_tokens` / `completion_tokens` / `cache_read_tokens` / `cache_write_tokens` / `reasoning_tokens` (均为 INTEGER) |
| `TokenUsageSummary` | 原 3 字段 | `prompt_tokens: i64` / `completion_tokens: i64` / `cache_read_tokens: i64` / `cache_write_tokens: i64` / `reasoning_tokens: i64` |
| `TokenUsageSummaryDto` | 原 3 字段 | 同上 5 个 |
| `ContextUsage` | 原 3 字段 | `promptTokens: number` / `completionTokens: number` / `cacheReadTokens: number` / `cacheWriteTokens: number` / `reasoningTokens: number` |
| `ContextBar` | 原 3 props | 接收完整 `ContextUsage`，进度条内分段着色 |

---

## 3. 数据库变更

### 3.1 messages 表新增列

```sql
ALTER TABLE messages ADD COLUMN prompt_tokens INTEGER;
ALTER TABLE messages ADD COLUMN completion_tokens INTEGER;
ALTER TABLE messages ADD COLUMN cache_read_tokens INTEGER;
ALTER TABLE messages ADD COLUMN cache_write_tokens INTEGER;
ALTER TABLE messages ADD COLUMN reasoning_tokens INTEGER;
```

**迁移策略**：在 `db.rs` 的 `init_schema` 中执行 `ALTER TABLE ... ADD COLUMN`（SQLite 支持 `IF NOT EXISTS` 的 `ALTER` 仅限 `RENAME`，但重复 `ADD COLUMN` 会报错——用 `PRAGMA table_info(messages)` 检查列是否存在后再执行）。

或者在 `init_schema` 中用 `CREATE TABLE IF NOT EXISTS` 升级 `messages` 的 schema（新安装的 DB 直接含新列，存量 DB 用 migration）。

### 3.2 MessageRow 扩展

`conversations/mod.rs` 的 `MessageRow` struct、`read_msg_row()`、`MSG_SELECT` 常量都需新增 5 个字段。

### 3.3 update_message_content 扩展

`msg_ops.rs` 的 `update_message_content()` 函数签名和 SQL 需增加 5 个新参数。

### 3.4 insert_message 扩展

`msg_ops.rs` 的 `insert_message()` 的 `INSERT INTO messages` 语句需包含新列。

### 3.5 TokenUsageSummary 聚合查询

新 SQL 按 conversation 聚合所有 breakdown 字段：

```sql
SELECT
  COALESCE(SUM(COALESCE(tokens, (LENGTH(content)+3)/4+4)), 0) as total_tokens,
  COALESCE(SUM(COALESCE(prompt_tokens, 0)), 0) as prompt_tokens,
  COALESCE(SUM(COALESCE(completion_tokens, 0)), 0) as completion_tokens,
  COALESCE(SUM(COALESCE(cache_read_tokens, 0)), 0) as cache_read_tokens,
  COALESCE(SUM(COALESCE(cache_write_tokens, 0)), 0) as cache_write_tokens,
  COALESCE(SUM(COALESCE(reasoning_tokens, 0)), 0) as reasoning_tokens
FROM messages WHERE conversation_id = ?1
```

---

## 4. Rust 层变更

### 4.1 StreamOutcome 扩展

`chat_runtime.rs` 新增 4 字段：

| 字段 | 类型 | 来源 |
|------|------|------|
| `completion_tokens` | `Option<u32>` | 流式累加器从最后一个 TokenUsage 的 `completion_tokens` 捕获 |
| `cache_read_tokens` | `Option<u32>` | 同上 |
| `cache_write_tokens` | `Option<u32>` | 同上 |
| `reasoning_tokens` | `Option<u32>` | 同上 |

**捕获时机**：在流的 accumulator（当前为 `StreamAccumulator` 或等价结构）中，每当收到 `ChatDelta::usage` 时更新这 4 个字段，`into_outcome()` 时一并传入。

**completion_tokens 的计算**：若 provider 未直接报告 `completion_tokens`，可用 `total_tokens - prompt_tokens` 回退。但当前 `TokenUsage`（chat_delta）已有 `completion_tokens` 字段（来自 `UnifiedTokenUsage.candidates_token_count`），直接取即可。

### 4.2 PartialOutcome 同理扩展

`chat_runtime.rs` 的 `PartialOutcome` 同样新增这 4 个字段（流中断时仍需完整记录已产出 token）。

### 4.3 resolve_final_tokens 扩展

`resolve_final_tokens()` 函数保留（返回 `Option<i64>` 用于 messages.tokens 兼容），新增独立函数或改为同时返回 breakdown 的 tuple/struct。

或更简洁：**不再通过 `resolve_final_tokens` 做字段裁剪**，改为在 `StreamOutcome` 上直接暴露所有 token 字段，由调用方组装 `DbWriteMsg`。

### 4.4 DbWriteMsg 扩展

`stream.rs` 的 `DbWriteMsg` 新增字段：

```
pub prompt_tokens: Option<i64>,
pub completion_tokens: Option<i64>,
pub cache_read_tokens: Option<i64>,
pub cache_write_tokens: Option<i64>,
pub reasoning_tokens: Option<i64>,
```

`save_partial_on_error()` 同步扩展。

### 4.5 TokenUsageSummary / TokenUsageSummaryDto 扩展

| 位置 | 新增 |
|------|------|
| `agent_modes.rs` `TokenUsageSummary` | `prompt_tokens` / `completion_tokens` / `cache_read_tokens` / `cache_write_tokens` / `reasoning_tokens`（均为 `i64`） |
| `agent_modes.rs` `get_conversation_token_usage()` | SQL 查询加入 5 个新列的聚合 |
| `protocol/dto/chat.rs` `TokenUsageSummaryDto` | 同 5 字段 |
| `From<TokenUsageSummary>` | 透传 |

---

## 5. TypeScript 层变更

### 5.1 ContextUsage 扩展

`src/chat/core/chatTypes.ts`：

| 字段 | 类型 | 说明 |
|------|------|------|
| `usedTokens` | `number` | 保留（兼容现有，流式更新用 = promptTokens） |
| `maxTokens` | `number` | 保留 |
| `percentage` | `number` | 保留 |
| `promptTokens` | `number` | 输入 token（provider 精确值） |
| `completionTokens` | `number` | 输出 token |
| `cacheReadTokens` | `number` | 缓存命中 token |
| `cacheWriteTokens` | `number` | 缓存写入 token |
| `reasoningTokens` | `number` | 推理 token |

> `usedTokens` 语义保持为"当前输入 token 数"，流式过程中等于 `promptTokens`。会话切换时从 `TokenUsageSummary` 取 `totalTokens` 作为总量（现有行为不变）。

### 5.2 store 初始值

`src/stores/chatStore.ts` L68 和 `chatConversationSlice.ts` L156：

```typescript
contextUsage: {
  usedTokens: 0,
  maxTokens: 128_000,
  percentage: 0,
  promptTokens: 0,
  completionTokens: 0,
  cacheReadTokens: 0,
  cacheWriteTokens: 0,
  reasoningTokens: 0,
}
```

> 注意：key 字符串中 `Tokens` 的 `T` 大写 = camelCase → Rust DTO 侧 `#[serde(rename_all = "camelCase")]` 对应的是小写开头（`promptTokens` → `prompt_tokens`），当前 DTO 使用的是 snake_case 序列化。这里保持一致用 camelCase field names。

### 5.3 chatOrchestrator.ts 透传

`src/services/chatOrchestrator.ts` L403–414 的 `if (payload.usage)` 块：

```typescript
if (payload.usage) {
  const usage = payload.usage;
  const ctx = usage.contextWindowTokens && usage.contextWindowTokens > 0
    ? usage.contextWindowTokens : 128_000;
  deps.state.updateContextUsage({
    usedTokens: usage.promptTokens,
    maxTokens: ctx,
    percentage: usage.promptTokens / ctx,
    promptTokens: usage.promptTokens,
    completionTokens: usage.completionTokens,
    cacheReadTokens: usage.cacheReadTokens ?? 0,
    cacheWriteTokens: usage.cacheWriteTokens ?? 0,
    reasoningTokens: usage.reasoningTokens ?? 0,
  });
}
```

### 5.4 fetchTokenUsage 扩展

`src/chat/core/chatUtils.ts` L38–43：

```typescript
deps.setContextUsage({
  usedTokens: summary.totalTokens,
  maxTokens: summary.contextWindowTokens,
  percentage: summary.percentage,
  promptTokens: summary.promptTokens ?? 0,
  completionTokens: summary.completionTokens ?? 0,
  cacheReadTokens: summary.cacheReadTokens ?? 0,
  cacheWriteTokens: summary.cacheWriteTokens ?? 0,
  reasoningTokens: summary.reasoningTokens ?? 0,
});
```

---

## 6. UI 设计

### 6.1 ContextBar 改造

**保留**：进度条 + 百分比文本 + 三级阈值颜色。

**进度条分段着色**：

```
[maxTokens 完整宽度]
[cache_read] [prompt (non-cache)] | [completion (non-reasoning)] [reasoning]
   绿色             蓝色              灰色                      紫色
```

| 段 | 宽度计算公式 | 颜色 | 条件 |
|----|-------------|------|------|
| cache | `cacheReadTokens / maxTokens` | `#52c41a`（绿） | `cacheReadTokens > 0` |
| prompt | `(promptTokens - cacheReadTokens) / maxTokens` | `#1677ff`（蓝） | 始终 |
| completion | `(completionTokens - reasoningTokens) / maxTokens` | `#d9d9d9`（灰） | `completionTokens > 0` |
| reasoning | `reasoningTokens / maxTokens` | `#722ed1`（紫） | `reasoningTokens > 0` |

实现：使用 `antd Progress` 的 `success` prop 或自定义分段。或改用纯 CSS inline 分段（更灵活）。

### 6.2 Tooltip 增强

hover 时显示详细行：

```
Input: 1,234 tokens (cache hit: 456)
Output: 567 tokens (reasoning: 89)
Total: 1,801 / 128,000 (1.4%)
```

仅当对应值 > 0 时才显示子行。

### 6.3 降级行为

| 场景 | 行为 |
|------|------|
| 所有新字段 = 0（存量会话） | 退化为现有单色进度条（prompt 段 = 100% 蓝色） |
| `cacheReadTokens = 0` | 不显示绿色段 |
| `reasoningTokens = 0` | 不显示紫色段 |
| `completionTokens = 0`（流式刚开始） | 不显示灰色段 |

---

## 7. 实施清单

### Phase S1：数据库 + storage crate

| # | 任务 | 文件 |
|---|------|------|
| S1.1 | messages 表新增 5 列（migration） | `assistant-storage/src/db.rs` |
| S1.2 | `MessageRow` 新增 5 字段 | `assistant-storage/src/conversations/mod.rs` |
| S1.3 | `MSG_SELECT` 新增 5 列 | 同上 |
| S1.4 | `read_msg_row()` 读取新列 | 同上 |
| S1.5 | `insert_message()` 写入新列 | `conversations/msg_ops.rs` |
| S1.6 | `update_message_content()` 扩展签名 + SQL | 同上 |
| S1.7 | `TokenUsageSummary` 新增 5 字段 | `agent_modes.rs` |
| S1.8 | `get_conversation_token_usage()` 聚合新列 | 同上 |

### Phase S2：agent crate（StreamOutcome + DbWriteMsg）

| # | 任务 | 文件 |
|---|------|------|
| S2.1 | `StreamOutcome` 新增 `completion_tokens` / `cache_read_tokens` / `cache_write_tokens` / `reasoning_tokens` | `chat_runtime.rs` |
| S2.2 | `PartialOutcome` 同步新增 | 同上 |
| S2.3 | 流式累加器在收到 `ChatDelta::usage` 时更新这 4 个字段（最后一次 delta 的 usage 即为最终值） | `chat_runtime.rs` 或 stream accumulator |
| S2.4 | `DbWriteMsg` 新增 `prompt_tokens` + 上 4 个字段 | `stream.rs` |
| S2.5 | `save_partial_on_error()` 扩展 | `stream.rs` |
| S2.6 | `send_chat_message` 组装 `DbWriteMsg` 时传入全部字段 | `cmd/chat/mod.rs` L227–233 |

### Phase S3：DTO + 命令层

| # | 任务 | 文件 |
|---|------|------|
| S3.1 | `TokenUsageSummaryDto` 新增 5 字段 | `protocol/dto/chat.rs` |
| S3.2 | `From<TokenUsageSummary>` 透传 | 同上 |

### Phase F1：前端类型 + store

| # | 任务 | 文件 |
|---|------|------|
| F1.1 | `ContextUsage` 新增 `promptTokens` / `completionTokens` / `cacheReadTokens` / `cacheWriteTokens` / `reasoningTokens` | `src/chat/core/chatTypes.ts` |
| F1.2 | store 初始值同步扩展 | `src/stores/chatStore.ts` |
| F1.3 | 会话切换时新字段 reset 为 0 | `src/stores/chatConversationSlice.ts` |

### Phase F2：前端数据流

| # | 任务 | 文件 |
|---|------|------|
| F2.1 | `chatOrchestrator.ts` 透传全部 usage 字段到 `ContextUsage` | `src/services/chatOrchestrator.ts` |
| F2.2 | `fetchTokenUsage()` 透传 `TokenUsageSummary` 新字段 | `src/chat/core/chatUtils.ts` |

### Phase F3：UI

| # | 任务 | 文件 |
|---|------|------|
| F3.1 | `ContextBar` 接收完整 `ContextUsage`，实现分段着色进度条 | `src/chat/ChatView.tsx` |
| F3.2 | Tooltip 扩展为多行详细信息 | 同上 |
| F3.3 | 降级逻辑（全部为 0 时退化为现有样式） | 同上 |

### Phase F4：验证

| # | 检查项 |
|---|--------|
| V1 | 新安装 DB 的 messages 表含 5 个新列（`PRAGMA table_info(messages)`） |
| V2 | 存量 DB 执行 migration 后含新列、存量数据新列为 NULL、聚合查询正常工作 |
| V3 | 流式聊天过程中进度条分段着色正确（cache 绿 / prompt 蓝 / completion 灰 / reasoning 紫） |
| V4 | 流结束后 `messages` 表 6 个 token 列有正确值 |
| V5 | 会话切换后从 DB 读取 breakdown 并正确展示 |
| V6 | 存量会话（新列为 NULL）退化为单色进度条 |
| V7 | 无 breakdown 数据时 UI 与改动前完全一致 |
| V8 | TypeScript 编译无新增错误 |
| V9 | Rust 编译无新增错误 |

---

## 8. 注意事项

### 8.1 completion_tokens 默认值语义

`StreamOutcome.completion_tokens` 和 `DbWriteMsg.completion_tokens` 使用 `Option<i64>`：
- `Some(0)` → 明确报告 0 输出
- `None` → provider 未报告（存量数据或用 character estimate 回退的场景）

但在 `ContextUsage` 中，所有新字段使用 `number`（默认 0），因为 TS 侧不需要区分"未报告"和"0"（UI 展示效果相同：不显示对应段）。

### 8.2 存量数据兼容

`get_conversation_token_usage` 的 SQL 聚合使用 `COALESCE(new_column, 0)`，确保存量 `NULL` 值不破坏查询。

### 8.3 依赖关系

- Phase S1–S3 必须先于 F1–F3（后端 schema 就绪后前端才能用）
- S2 依赖 S1（MessageRow 扩展后才能写 DbWriteMsg）
- F2 依赖 F1（ContextUsage 扩展后才能透传）

### 8.4 不做什么

- 不改 `TokenUsageDto`（已有 6 字段，无需改）
- 不新增独立 TokenDetail 面板（属于 P2，不在本 spec 范围）
- 不实现 cost 估算
- 不实现按 model 聚合统计
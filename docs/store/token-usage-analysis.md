# Token Usage 获取机制分析报告

> 对比本应用与参考项目（OpenBitFun、deepseek-harness-orb）的 token usage 获取、归一化、持久化与展示机制，明确当前实现的差距与优先级。
>
> **更新日期**: 2026-10-31（标记 Phase 1/2 完成状态，移除内联代码以降低时效性风险）

## 1 数据流架构

所有项目均遵循 4 层转换架构：

```
Provider API 原始字段 → Provider Usage 结构 → UnifiedTokenUsage → 前端 TokenUsage → UI 展示
```

| 层次 | 本应用 | OpenBitFun | deepseek-harness-orb |
|------|--------|------------|---------------------|
| Provider 解析 | `assistant-providers/src/{openai,anthropic,gemini}/` 对应文件 | 同结构，同路径 | N/A（TypeScript SDK） |
| 归一化 | `assistant-providers/src/types/unified.rs` `UnifiedTokenUsage` | `agent-stream/unified.rs`（re-export） | `TokenUsage` interface |
| 前端传输 | `TokenUsage`（chat_delta.rs）→ `TokenUsageDto`（protocol/dto/chat.rs） | 直接序列化 `UnifiedTokenUsage` | `TurnTokenUsage` 跨 attempt 聚合 |
| UI 展示 | `ChatView.tsx` ContextBar（仅 used/max/percentage） | 桌面端 token 面板 | `ContextPressureProjection` |

## 2 UnifiedTokenUsage 结构对比

本应用与 OpenBitFun 的 `UnifiedTokenUsage` **完全对齐**，均包含以下 6 个字段（详见 `assistant-providers/src/types/unified.rs`）：

| 字段 | 类型 | 语义 |
|------|------|------|
| `prompt_token_count` | `u32` | 输入 token 总数 |
| `candidates_token_count` | `u32` | 输出 token 总数 |
| `total_token_count` | `u32` | prompt + candidates |
| `reasoning_token_count` | `Option<u32>` | 思维链/thinking token（仅 Gemini 报告） |
| `cached_content_token_count` | `Option<u32>` | Cache READ tokens，hit rate 的分子，`prompt_token_count` 为分母 |
| `cache_creation_token_count` | `Option<u32>` | Cache WRITE tokens（仅 Anthropic / OpenAI Responses 报告），与 read 互斥分离，**不计入** hit rate 分子 |

## 3 各 Provider 字段映射

### 3.1 OpenAI 兼容系（含 DeepSeek/Qwen/MiniMax）

| 字段 | API 原始 | OpenBitFun | 本应用 | 差距 |
|------|----------|------------|--------|------|
| prompt | `prompt_tokens` | ✅ | ✅ | 无 |
| completion | `completion_tokens` | ✅ | ✅ | 无 |
| total | `total_tokens` | ✅ | ✅ | 无 |
| cache_read | `prompt_cache_hit_tokens` ∨ `prompt_tokens_details.cached_tokens` | ✅ DeepSeek 优先 | ✅ 同策略 | 无 |
| cache_write | — | ❌ 不支持 | ❌ 不支持 | 无（OpenAI 不报告） |
| reasoning | — | ❌ 不提取 | ❌ 不提取 | ⚠️ OpenAI 不报告，但 `reasoning_token_count` 字段已预留 |
| `prompt_cache_miss_tokens` | DeepSeek 扩展 | ✅ 反序列化（`#[allow(dead_code)]`） | ✅ 已声明 | 无 |

**核心策略一致**：`prompt_cache_hit_tokens`（DeepSeek 原生）优先于 `prompt_tokens_details.cached_tokens`（OpenAI 标准），两者都存在时取 DeepSeek 值。

### 3.2 Anthropic

| 字段 | API 原始 | OpenBitFun | 本应用 | 差距 |
|------|----------|------------|--------|------|
| prompt | `input_tokens + cache_read + cache_creation` | ✅ 三者求和 | ✅ 同策略 | 无 |
| completion | `output_tokens` | ✅ | ✅ | 无 |
| total | `prompt + completion`（自行计算） | ✅ | ✅ | 无 |
| cache_read | `cache_read_input_tokens` | ✅ | ✅ | 无 |
| cache_write | `cache_creation_input_tokens` | ✅ | ✅ | 无 |

**两者实现完全一致**。Anthropic 的 `input_tokens` 不含 cache token，需三者求和才是完整 prompt 占用。

### 3.3 Gemini

| 字段 | API 原始 | OpenBitFun | 本应用 | 差距 |
|------|----------|------------|--------|------|
| prompt | `prompt_token_count` | ✅ | ✅ | 无 |
| completion | `candidates_token_count + thoughts_token_count` | ✅ | ✅ | 无 |
| total | `total_token_count` | ✅ | ✅ | 无 |
| cache_read | `cached_content_token_count` | ✅ | ✅ | 无 |
| reasoning | `thoughts_token_count` | ✅ | ✅ | 无 |

**两者实现完全一致**。Gemini 的 `candidates_token_count` 不含 thinking tokens，需加回。

### 3.4 OpenAI Responses API

| 字段 | API 原始 | OpenBitFun | 本应用 | 差距 |
|------|----------|------------|--------|------|
| prompt | `input_tokens` | ✅ | ✅ | 无 |
| completion | `output_tokens` | ✅ | ✅ | 无 |
| total | `total_tokens` | ✅ | ✅ | 无 |
| cache_read | `input_tokens_details.cached_tokens` | ✅ | ✅ | 无 |
| cache_write | `input_tokens_details.cache_write_tokens` | ✅ | ✅ | 无 |

**两者实现完全一致**。

### 3.5 Provider 映射总结

**本应用的 Provider → UnifiedTokenUsage 转换逻辑已与 OpenBitFun 完全对齐**，无功能性差距。

## 4 前端 TokenUsage 结构对比

### 4.1 本应用当前结构

三层 TokenUsage 已对齐（详见对应源文件）：

- **Rust 中间层** `assistant-providers/src/types/chat_delta.rs`：`TokenUsage` 含 `prompt_tokens` / `completion_tokens` / `total_tokens` / `cache_read_tokens` / `cache_write_tokens` / `reasoning_tokens` / `context_window_tokens`，其中 cache 和 reasoning 均为 `Option<u32>`
- **Rust DTO 层** `src-tauri/src/protocol/dto/chat.rs`：`TokenUsageDto` 字段与中间层一一对应
- **TypeScript 层** `src/chat/core/chatTypes.ts`：`TokenUsage` 接口含 `promptTokens` / `completionTokens` / `totalTokens` / `cacheReadTokens?` / `cacheWriteTokens?` / `reasoningTokens?` / `contextWindowTokens?`

转换函数 `unified_usage_to_token_usage()` 将 `UnifiedTokenUsage` 的 Option 字段**直接透传**（不再 `unwrap_or(0)`），`reasoning_token_count` 也一并传递。

### 4.2 已解决的差距

以下 3 项在 Phase 1/2 中已完成（参见 §8）：

| 原差距 | 当前状态 |
|--------|----------|
| ~~缺少 `reasoning_tokens`~~ | ✅ 三层均已补齐 |
| ~~cache 字段 `u32` vs `Option<u32>`~~ | ✅ 已改为 `Option<u32>`，直接透传 |
| ~~DTO 缺少 `reasoning_tokens`~~ | ✅ `TokenUsageDto` 已包含 |

### 4.3 OpenBitFun 的做法

OpenBitFun 直接序列化 `UnifiedTokenUsage`（所有字段均为 `Option` 或 `u32`），不经过中间扁平化。前端按需读取 `reasoning_token_count`、`cached_content_token_count`、`cache_creation_token_count`。

### 4.4 deepseek-harness-orb 的做法

`TurnTokenUsage` 采用**保守聚合策略**：`cacheReadTokens` / `cacheWriteTokens` / `reasoningTokens` 仅在**所有 attempt 都报告了该字段**时才存在，否则为 `undefined`。这避免了部分 attempt 无 cache 报告时的统计失真。

## 5 Token 估算与持久化

### 5.1 本应用当前实现

**估算与聚合**（`assistant-storage/src/agent_modes.rs`）：

- 每条消息有 `tokens` 字段（来自 provider 报告或估算）
- 若 `tokens` 为 NULL，回退到字符级估算：`(LENGTH(content) + 3) / 4 + 4`
- 按 conversation 聚合所有消息的 tokens 总和，与 `context_window_tokens` 比较得出百分比

**持久化**：仅 `messages.tokens`（INTEGER，单值），无按 model/session 的聚合统计。无 per-request 的 prompt/completion/cache/reasoning 拆分记录。

### 5.2 OpenBitFun 的 Token 持久化统计

**三层结构**：

| 结构 | 用途 | 关键字段 |
|------|------|----------|
| `TokenUsageRecord` | 单次 API 调用记录 | `model_config_id`, `session_id`, `turn_id`, `input_tokens`, `output_tokens`, `cached_tokens`, `cache_write_tokens`, `cached_tokens_available` |
| `ModelTokenStats` | 按 model 聚合 | `total_input`, `total_output`, `total_cached`, `total_cache_write`, `cache_reported_input_tokens`, `session_count`, `request_count` |
| `SessionTokenStats` | 按 session 聚合 | 同 ModelTokenStats 字段 + `session_id` |

**关键设计决策**：

1. **`cached_tokens_available: bool`** — 标记 provider 是否明确报告了 cache 信息
2. **`cache_reported_input_tokens`** — 仅在 provider 明确报告 cache 时才计入 hit ratio 分母
3. **`cache_hit_ratio() = total_cached / cache_reported_input_tokens`** — 避免无 cache 报告的 provider 稀释命中率
4. **`estimated_cache_savings_ratio() = cache_hit_ratio * 0.90`** — 基于 cache read 约为全价 10% 的定价模型

### 5.3 deepseek-harness-orb 的 Token 估算

**字符级双系数估算**（`estimate.ts`）：
- ASCII 字符：1 token ≈ 4 字符
- 非 ASCII 字符（CJK 等）：1 token ≈ 1.5 字符
- 消息级结构开销：每消息 +4、tool_call +10、消息数组 +3

**Context Pressure 投影**（`projection.ts`）：

核心思想：compaction 不产生 usage 事件，但 `projectedTokens` 通过 **provider 锚点 + heuristic delta** 重定价，使 compaction 后的 context 占用显示不会断层。结构含 `pressureTokens`（最近一次 provider 报告值）、`projectedTokens`（锚点 + surface delta）、`contextWindow`（当前模型窗口）。

**Token 四桶分离**（`TokenUsageProjection`）：

四个桶互斥分离：`uncachedInputTokens`（未命中 cache）、`outputTokens`（输出，含 reasoning）、`cacheReadTokens`（cache 命中）、`cacheWriteTokens`（cache 写入），reasoning 已包含在 output 中不重复计算。

## 6 前端展示对比

### 6.1 本应用

`ChatView.tsx` 的 `ContextBar` 组件：

- 仅展示 `usedTokens / maxTokens` 加进度条和百分比
- 有三级阈值颜色：正常（<68%）、警告黄色（68%-90%）、危险红色（>90%）
- **不展示** cache hit、cache write、reasoning tokens 等细分信息

### 6.2 OpenBitFun

桌面端 token 面板展示：
- 总 token 用量（input / output / total）
- Cache hit ratio（`cached / prompt`）
- Estimated cache savings ratio（`hit_ratio * 0.90`）
- 按 model 分组的聚合统计

### 6.3 deepseek-harness-orb

- `ContextPressureProjection`：pressureTokens / projectedTokens / contextWindow
- `ContextBreakdownProjection`：systemTokens / toolsTokens / messageTokens（组成分解）
- 四桶分离展示：uncachedInput / output / cacheRead / cacheWrite

## 7 缺失项与优先级

> ✅ = 已完成  ❌ = 待实施

| 优先级 | 缺失项 | 来源参考 | 影响 | 建议方案 | 状态 |
|--------|--------|----------|------|----------|------|
| **P0** | `TokenUsage` / `TokenUsageDto` 缺少 `reasoning_tokens` | OpenBitFun `UnifiedTokenUsage.reasoning_token_count` | 前端无法显示 reasoning token 占比；Gemini thinking token、DeepSeek reasoning token 被吞掉 | 在 `TokenUsage` / `TokenUsageDto` / `chatTypes.ts` 中添加 `reasoning_tokens: Option<u32>` / `reasoningTokens?: number` | ✅ |
| **P1** | cache 字段 `u32` vs `Option<u32>` | deepseek-harness-orb `TurnTokenUsage` 保守聚合 | 无法区分"provider 不报告"和"cache = 0"，cache hit ratio 分母失真 | `TokenUsage` 中 `cache_read_tokens` / `cache_write_tokens` 改为 `Option<u32>`，前端对应 `number \| undefined` | ✅ |
| **P1** | 缺少 `TokenUsageRecord` / `ModelTokenStats` 持久化 | OpenBitFun `services-core/token_usage` | 无法按 model/session 聚合 token 用量，无法计算 cache hit ratio | 新增 `token_usage` 表 + `ModelTokenStats` / `SessionTokenStats` 聚合查询 | ❌ |
| **P2** | 缺少 `ContextPressureProjection` | deepseek-harness-orb `token-meter/projection` | compaction 后 context 占用显示断层 | 实现 provider 锚点 + heuristic delta 重定价 | ❌ |
| **P2** | 缺少跨 attempt 的 `TurnTokenUsage` 聚合 | deepseek-harness-orb `token-meter/turn-usage` | 重试/continuation 场景下 token 统计不完整 | 在 `StreamStats` 中累积多 attempt usage | ❌ |
| **P2** | 前端不展示 cache / reasoning 信息 | OpenBitFun 桌面端 token 面板 | 用户无法感知 cache 命中率和 reasoning 开销 | ContextBar 扩展或新增 TokenDetail 面板 | ❌ |
| **P3** | OpenAI 缺少 `prompt_cache_miss_tokens` 反序列化 | OpenBitFun `openai.rs` | 含此字段的 DeepSeek payload 可能导致 serde 拒绝 | 在 `ChatUsage` 中添加 `#[serde(default)] prompt_cache_miss_tokens: Option<u32>` | ✅ |
| **P3** | Token 估算公式粗糙 | deepseek-harness-orb `estimate.ts` | `(LENGTH + 3) / 4 + 4` 对 CJK 文本严重低估 | 实现 ASCII/非ASCII 双系数估算 | ❌ |

## 8 实施路线建议

### Phase 1：补齐字段（P0） ✅ 已完成

1. `TokenUsage` 添加 `reasoning_tokens: Option<u32>` ✅
2. `TokenUsageDto` 添加 `reasoning_tokens: Option<u32>` ✅
3. `chatTypes.ts` 添加 `reasoningTokens?: number` ✅
4. `unified_usage_to_token_usage` 传递 `reasoning_token_count` ✅
5. `ChatUsage` 添加 `prompt_cache_miss_tokens`（P3 顺手做） ✅

### Phase 2：cache 语义精确化（P1） ✅ 已完成

1. `TokenUsage` 的 `cache_read_tokens` / `cache_write_tokens` 改为 `Option<u32>` ✅
2. `TokenUsageDto` 同步修改 ✅
3. `chatTypes.ts` 已是 `number | undefined`，无需改 ✅
4. `unified_usage_to_token_usage` 直接传递而非 `unwrap_or(0)` ✅

### Phase 3：持久化统计（P1）

1. 新增 `token_usage_records` SQLite 表
2. 每次 API 调用完成后写入 `TokenUsageRecord`
3. 实现 `ModelTokenStats` / `SessionTokenStats` 聚合查询
4. 实现 `cache_hit_ratio()` / `estimated_cache_savings_ratio()`

### Phase 4：Context Pressure 投影（P2）

1. 在 `StreamStats` 中记录最后一次 provider 报告的 usage 作为锚点
2. 实现 `ContextPressureProjection`：锚点 + surface delta
3. compaction 后用投影值替代断层值

### Phase 5：前端展示增强（P2）

1. ContextBar 扩展：显示 cache hit ratio、reasoning tokens
2. 新增 TokenDetail 面板：四桶分离展示
3. 按 model 分组的聚合统计面板

## 附录 A：参考项目文件索引

| 参考项目 | 关键文件 | 内容 |
|----------|----------|------|
| OpenBitFun | `adapters/ai-adapters/src/stream/types/openai.rs` | OpenAI usage 提取 + DeepSeek cache 优先策略 |
| OpenBitFun | `adapters/ai-adapters/src/stream/types/anthropic.rs` | Anthropic 三桶求和 + cache read/write 分离 |
| OpenBitFun | `adapters/ai-adapters/src/stream/types/gemini.rs` | Gemini usage + thoughts_token_count 修正 |
| OpenBitFun | `adapters/ai-adapters/src/stream/types/responses.rs` | Responses API usage + cache_write_tokens |
| OpenBitFun | `execution/agent-stream/src/unified.rs` | UnifiedTokenUsage 定义 |
| OpenBitFun | `services/services-core/src/token_usage/types.rs` | TokenUsageRecord / ModelTokenStats / SessionTokenStats |
| deepseek-harness-orb | `packages/llm/token-meter/src/types.ts` | TokenMeasurement / TokenSurfaceNode |
| deepseek-harness-orb | `packages/llm/token-meter/src/turn-usage.ts` | TurnTokenUsage 跨 attempt 聚合 |
| deepseek-harness-orb | `packages/llm/token-meter/src/projection.ts` | ContextPressureProjection / TokenUsageProjection |
| deepseek-harness-orb | `packages/llm/token-meter/src/estimate.ts` | ASCII/非ASCII 双系数估算 |

## 附录 B：本应用文件索引

| 文件 | 内容 |
|------|------|
| `src-tauri/crates/assistant-providers/src/types/unified.rs` | UnifiedTokenUsage 定义 |
| `src-tauri/crates/assistant-providers/src/types/chat_delta.rs` | TokenUsage 定义 + unified→token 转换 |
| `src-tauri/crates/assistant-providers/src/openai/stream.rs` | OpenAI ChatUsage → UnifiedTokenUsage |
| `src-tauri/crates/assistant-providers/src/anthropic/stream.rs` | AnthropicUsage → UnifiedTokenUsage |
| `src-tauri/crates/assistant-providers/src/gemini/stream.rs` | GeminiUsageMetadata → UnifiedTokenUsage |
| `src-tauri/crates/assistant-providers/src/openai/responses_types.rs` | ResponsesUsage → UnifiedTokenUsage |
| `src-tauri/crates/assistant-providers/src/stream/stats.rs` | StreamStats（仅日志，无持久化） |
| `src-tauri/src/protocol/dto/chat.rs` | TokenUsageDto（前端协议层） |
| `src-tauri/crates/assistant-storage/src/agent_modes.rs` | TokenUsageSummary + get_conversation_token_usage |
| `src/chat/core/chatTypes.ts` | 前端 TokenUsage interface |
| `src/chat/ChatView.tsx` | ContextBar 展示组件 |
| `src/services/chatOrchestrator.ts` | usage → contextUsage 转换 |

## 附录 C：相关 spec

| 文档 | 说明 |
|------|------|
| `token-usage-ui-spec.md` | Token Usage 持久化与 UI 优化完整 spec（DB messages 表扩展 + StreamOutcome → DbWriteMsg → 分段着色 UI） |
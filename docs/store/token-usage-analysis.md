# Token Usage 获取机制分析报告

> 对比本应用与参考项目（OpenBitFun、deepseek-harness-orb）的 token usage 获取、归一化、持久化与展示机制，明确当前实现的差距与优先级。

## 1 数据流架构

所有项目均遵循 4 层转换架构：

```
Provider API 原始字段 → Provider Usage 结构 → UnifiedTokenUsage → 前端 TokenUsage → UI 展示
```

| 层次 | 本应用 | OpenBitFun | deepseek-harness-orb |
|------|--------|------------|---------------------|
| Provider 解析 | `openai/types.rs` `anthropic/types.rs` `gemini/types.rs` | 同结构，同路径 | N/A（TypeScript SDK） |
| 归一化 | `types/unified.rs` `UnifiedTokenUsage` | `agent-stream/unified.rs`（re-export） | `TokenUsage` interface |
| 前端传输 | `types/chat_delta.rs` `TokenUsage` → `protocol/dto/chat.rs` `TokenUsageDto` | 直接序列化 `UnifiedTokenUsage` | `TurnTokenUsage` 跨 attempt 聚合 |
| UI 展示 | `ChatView.tsx` ContextBar（仅 used/max/percentage） | 桌面端 token 面板 | `ContextPressureProjection` |

## 2 UnifiedTokenUsage 结构对比

本应用与 OpenBitFun 的 `UnifiedTokenUsage` **完全对齐**：

```rust
pub struct UnifiedTokenUsage {
    pub prompt_token_count: u32,
    pub candidates_token_count: u32,
    pub total_token_count: u32,
    pub reasoning_token_count: Option<u32>,       // ✅ 对齐
    pub cached_content_token_count: Option<u32>,   // ✅ 对齐，仅含 cache read
    pub cache_creation_token_count: Option<u32>,   // ✅ 对齐，仅含 cache write
}
```

OpenBitFun 的字段语义注释（本应用缺少但语义一致）：
- `cached_content_token_count`：Cache READ tokens（从缓存命中），hit rate 的分子，`prompt_token_count` 为分母
- `cache_creation_token_count`：Cache WRITE tokens（仅 Anthropic 报告），与 read 互斥分离，**不计入** hit rate 分子

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
| `prompt_cache_miss_tokens` | DeepSeek 扩展 | ✅ 反序列化（`#[allow(dead_code)]`） | ❌ 未声明 | ⚠️ 含此字段的 payload 可能导致 serde 拒绝 |

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

**本应用的 Provider → UnifiedTokenUsage 转换逻辑已与 OpenBitFun 完全对齐**，无功能性差距。唯一微小差异是 OpenAI 的 `prompt_cache_miss_tokens` 字段未声明。

## 4 前端 TokenUsage 结构对比

### 4.1 本应用当前结构

**Rust 层** (`chat_delta.rs`):

```rust
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub cache_read_tokens: u32,       // 非 Option，默认 0
    pub cache_write_tokens: u32,      // 非 Option，默认 0
    pub context_window_tokens: u32,
}
```

**TypeScript 层** (`chatTypes.ts`):

```typescript
export interface TokenUsage {
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
  cacheReadTokens?: number;      // optional，但 Rust 侧始终为 0
  cacheWriteTokens?: number;     // optional，但 Rust 侧始终为 0
  contextWindowTokens?: number;
}
```

**转换函数** (`chat_delta.rs`):

```rust
fn unified_usage_to_token_usage(usage: &UnifiedTokenUsage, context_window_tokens: u32) -> TokenUsage {
    TokenUsage {
        prompt_tokens: usage.prompt_token_count,
        completion_tokens: usage.candidates_token_count,
        total_tokens: usage.total_token_count,
        cache_read_tokens: usage.cached_content_token_count.unwrap_or(0),   // Option → u32
        cache_write_tokens: usage.cache_creation_token_count.unwrap_or(0),   // Option → u32
        context_window_tokens,
    }
}
```

### 4.2 关键差距

| 差距 | 说明 | 影响 |
|------|------|------|
| ❌ 缺少 `reasoning_tokens` | `UnifiedTokenUsage.reasoning_token_count` 在转换时被丢弃 | 前端无法显示 reasoning token 占比；Gemini 的 thinking token、DeepSeek 的 reasoning token 信息丢失 |
| ⚠️ cache 字段 `u32` vs `Option<u32>` | `unwrap_or(0)` 将"provider 不报告"和"cache = 0"混为一谈 | 无法区分两种语义，cache hit ratio 计算分母失真 |
| ❌ 缺少 `reasoning_tokens` 在 DTO | `TokenUsageDto` 也无此字段 | 前端协议层无法传递 |

### 4.3 OpenBitFun 的做法

OpenBitFun 直接序列化 `UnifiedTokenUsage`（所有字段均为 `Option` 或 `u32`），不经过中间扁平化。前端按需读取 `reasoning_token_count`、`cached_content_token_count`、`cache_creation_token_count`。

### 4.4 deepseek-harness-orb 的做法

`TurnTokenUsage` 采用**保守聚合策略**：

```typescript
interface TurnTokenUsage {
  uncachedInputTokens: number;
  outputTokens: number;
  totalTokens: number;
  cacheReadTokens?: number;    // 仅当所有 attempt 都报告时才为 Some
  cacheWriteTokens?: number;   // 仅当所有 attempt 都报告时才为 Some
  reasoningTokens?: number;    // 仅当所有 attempt 都报告时才为 Some
}
```

`cacheReadTokens` / `cacheWriteTokens` / `reasoningTokens` 仅在**所有 attempt 都报告了该字段**时才存在，否则为 `undefined`。这避免了部分 attempt 无 cache 报告时的统计失真。

## 5 Token 估算与持久化

### 5.1 本应用当前实现

**估算公式**（`agent_modes.rs`）：

```sql
COALESCE(SUM(COALESCE(tokens, (LENGTH(content) + 3) / 4 + 4)), 0)
```

- 每条消息有 `tokens` 字段（来自 provider 报告或估算）
- 若 `tokens` 为 NULL，回退到字符级估算：`(LENGTH(content) + 3) / 4 + 4`
- 按 conversation 聚合，与 `context_window_tokens` 比较

**持久化**：仅 `messages.tokens`（INTEGER，单值），无按 model/session 的聚合统计。

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

```typescript
interface ContextPressureProjection {
  pressureTokens?: number;    // 最近一次请求的 provider 报告值（锚点）
  projectedTokens?: number;   // pressureTokens + 自上次报告以来的 surface delta
  contextWindow?: number;     // 当前模型的上下文窗口
}
```

**核心思想**：compaction 不产生 usage 事件，但 `projectedTokens` 通过 **provider 锚点 + heuristic delta** 重定价，使 compaction 后的 context 占用显示不会断层。

**Token 四桶分离**（`TokenUsageProjection`）：

```typescript
interface TokenUsageProjection {
  uncachedInputTokens: number;   // 未命中 cache 的输入
  outputTokens: number;          // 输出（含 reasoning）
  cacheReadTokens: number;       // cache 命中
  cacheWriteTokens: number;      // cache 写入
}
```

四个桶互斥分离，reasoning 已包含在 `outputTokens` 中不重复计算。

## 6 前端展示对比

### 6.1 本应用

`ChatView.tsx` 的 `ContextBar` 组件：

```tsx
<ContextBar usage={chat.contextUsage} />
// contextUsage = { usedTokens, maxTokens, percentage }
// 显示：usedTokens / maxTokens + Progress Bar
```

- 仅展示 `usedTokens / maxTokens` 和进度条
- `usedTokens` 来自 `usage.promptTokens`（流式实时更新）
- **不展示** cache hit、cache write、reasoning tokens

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

| 优先级 | 缺失项 | 来源参考 | 影响 | 建议方案 |
|--------|--------|----------|------|----------|
| **P0** | `TokenUsage` / `TokenUsageDto` 缺少 `reasoning_tokens` | OpenBitFun `UnifiedTokenUsage.reasoning_token_count` | 前端无法显示 reasoning token 占比；Gemini thinking token、DeepSeek reasoning token 被吞掉 | 在 `TokenUsage` / `TokenUsageDto` / `chatTypes.ts` 中添加 `reasoning_tokens: Option<u32>` / `reasoningTokens?: number` |
| **P1** | cache 字段 `u32` vs `Option<u32>` | deepseek-harness-orb `TurnTokenUsage` 保守聚合 | 无法区分"provider 不报告"和"cache = 0"，cache hit ratio 分母失真 | `TokenUsage` 中 `cache_read_tokens` / `cache_write_tokens` 改为 `Option<u32>`，前端对应 `number \| undefined` |
| **P1** | 缺少 `TokenUsageRecord` / `ModelTokenStats` 持久化 | OpenBitFun `services-core/token_usage` | 无法按 model/session 聚合 token 用量，无法计算 cache hit ratio | 新增 `token_usage` 表 + `ModelTokenStats` / `SessionTokenStats` 聚合查询 |
| **P2** | 缺少 `ContextPressureProjection` | deepseek-harness-orb `token-meter/projection` | compaction 后 context 占用显示断层 | 实现 provider 锚点 + heuristic delta 重定价 |
| **P2** | 缺少跨 attempt 的 `TurnTokenUsage` 聚合 | deepseek-harness-orb `token-meter/turn-usage` | 重试/continuation 场景下 token 统计不完整 | 在 `StreamStats` 中累积多 attempt usage |
| **P2** | 前端不展示 cache / reasoning 信息 | OpenBitFun 桌面端 token 面板 | 用户无法感知 cache 命中率和 reasoning 开销 | ContextBar 扩展或新增 TokenDetail 面板 |
| **P3** | OpenAI 缺少 `prompt_cache_miss_tokens` 反序列化 | OpenBitFun `openai.rs` | 含此字段的 DeepSeek payload 可能导致 serde 拒绝 | 在 `ChatUsage` 中添加 `#[serde(default)] prompt_cache_miss_tokens: Option<u32>` |
| **P3** | Token 估算公式粗糙 | deepseek-harness-orb `estimate.ts` | `(LENGTH + 3) / 4 + 4` 对 CJK 文本严重低估 | 实现 ASCII/非ASCII 双系数估算 |

## 8 实施路线建议

### Phase 1：补齐字段（P0）

1. `TokenUsage` 添加 `reasoning_tokens: Option<u32>`
2. `TokenUsageDto` 添加 `reasoning_tokens: Option<u32>`
3. `chatTypes.ts` 添加 `reasoningTokens?: number`
4. `unified_usage_to_token_usage` 传递 `reasoning_token_count`
5. `ChatUsage` 添加 `prompt_cache_miss_tokens`（P3 顺手做）

### Phase 2：cache 语义精确化（P1）

1. `TokenUsage` 的 `cache_read_tokens` / `cache_write_tokens` 改为 `Option<u32>`
2. `TokenUsageDto` 同步修改
3. `chatTypes.ts` 已是 `number | undefined`，无需改
4. `unified_usage_to_token_usage` 直接传递而非 `unwrap_or(0)`

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
# I9 — Provider 多 API 格式适配 规格说明

## 1. 交付目标

将当前 `assistant-providers` crate 从「仅支持 OpenAI Chat Completions 格式」升级为「支持 OpenAI / Anthropic / Gemini 三种原生 API 格式 + OpenAI 兼容格式」，同时引入统一响应类型、Provider 特异行为处理、结构化流处理等基础设施。

**核心原则**：增量改进，每一步向后兼容，不中断现有功能。

> **前置依赖**：I8 ✅（声明式设置系统）
> **参考**：OpenBitFun（Rust + reqwest + SSE，分层设计、UnifiedResponse、quirks 处理模式）

---

## 2. 当前问题

| # | 问题 | 影响 |
|---|------|------|
| P1 | 单文件 617 行，所有逻辑混在一起 | 难以维护、测试、扩展 |
| P2 | `ChatDelta` 扁平结构，一个 delta 只能带一种内容 | 多 tool_calls 场景丢失信息 |
| P3 | 手动 `buffer.find('\n')` SSE 解析 | 容易出错，不处理 `event:`/`id:`/`retry:` 字段 |
| P4 | 无 Provider 特异行为处理 | DeepSeek reasoning、MiniMax reasoning_details、Qwen reasoning 不兼容 |
| P5 | 无 SSE 流内错误检测 | 部分 provider 在流中返回错误，当前无法捕获 |
| P6 | `create_provider_adapter` 始终返回 `OpenAIAdapter` | Anthropic/Gemini ProviderKind 已定义但未实现 |
| P7 | 无测试 | 流解析、错误分类、重试逻辑无覆盖 |
| P8 | 超时逻辑散落在 stream 循环内 | 不可复用、不可独立测试 |

### 2.1 当前文件结构

```
assistant-providers/src/
├── base.rs        (347行) — trait 定义、错误类型、配置类型
├── openai.rs      (617行) — 全部 OpenAI 逻辑：请求/响应/SSE/重试/超时/错误分类
├── crypto.rs      — API 密钥加密
├── sse_log.rs     — SSE 日志收集
└── lib.rs         — 模块声明
```

### 2.2 参考项目对比

| 设计点 | 当前项目 | OpenBitFun (Rust) | openhuman (Rust) | airi (TypeScript) |
|--------|---------|-------------------|------------------|-------------------|
| 文件组织 | 单文件 617 行 | 按职责 6+ 文件 | 工厂 10+ 子模块 | 按功能分层 |
| SSE 解析 | 手动 buffer + `\n` | eventsource_stream crate | 委托 tinyinference | @xsai/stream-text SDK |
| 统一响应 | ChatDelta 扁平 | UnifiedResponse + into_unified_responses() | ChatModel trait | StreamEvent 联合类型 |
| 错误分类 | HTTP 状态码 | HTTP + SSE 流内错误 | BackendErrorCode + Sentry | APICallError + statusCode |
| Provider 特异 | 无 | quirks.rs + normalizer | 委托 tinyinference | contentArrayCompatibility |
| Reasoning | reasoning_content 字段 | ReasoningPreset 系统 | 委托 tinyinference | thinking mode |
| 多 Provider | 始终 OpenAIAdapter | 3 独立模块 | 工厂 + provider string | 30+ defineProvider |

**结论**：以 **OpenBitFun** 为主要参考（同为 Rust + reqwest + SSE），借鉴其分层设计、UnifiedResponse、quirks 处理模式。

---

## 3. 目标架构

### 3.1 新文件结构

```
assistant-providers/src/
├── base.rs                    — trait 定义、错误类型、配置类型（保留，小幅调整）
├── client/                    — HTTP 客户端基础设施（从各 adapter 共享提取）
│   ├── mod.rs                 — AIClient struct + 配置
│   ├── http.rs                — 请求构建 + 发送 + 重试
│   ├── sse.rs                 — execute_sse_request（SSE 请求执行入口）
│   ├── format.rs              — API 格式检测（OpenAI/Anthropic/Gemini）
│   ├── quirks.rs              — Provider 特异行为（URL 检测、reasoning 兼容、header 差异）
│   ├── healthcheck.rs         — 健康检查
│   ├── request_capacity.rs    — 请求容量/限流
│   ├── response_aggregator.rs — 非流式响应聚合
│   └── utils.rs               — 工具函数（elapsed_ms 等）
├── types/
│   ├── mod.rs                 — 统一类型导出
│   ├── unified.rs             — UnifiedResponse + UnifiedToolCall + UnifiedTokenUsage
│   ├── chat_delta.rs          — ChatDelta（从 base.rs 积入，桥接 UnifiedResponse）
│   ├── message.rs             — 共享消息类型（ChatMessage/MessageRole/ContentBlock）
│   ├── config.rs              — ProviderConfig/RetryConfig/StreamTimeoutConfig
│   └── tool.rs                — ToolDefinition/ToolCall 类型
├── openai/
│   ├── mod.rs                 — OpenAIAdapter struct + new/with_* builders
│   ├── chat.rs                — chat_completion_stream 请求构建 + 发送
│   ├── types.rs               — ChatRequest/ChatResponseChunk/ChatUsage 等请求/响应类型
│   ├── stream.rs              — SSE 解析 + into_unified_responses 转换
│   ├── error.rs               — classify_http_error + parse_retry_after_secs
│   ├── message_converter.rs   — OpenAI 消息格式转换
│   └── discovery.rs           — list_models 实现（模型发现）
├── anthropic/
│   ├── mod.rs                 — AnthropicAdapter struct
│   ├── request.rs             — 请求构建（Messages API）+ 认证头
│   ├── types.rs               — Anthropic 请求/响应类型
│   ├── stream.rs              — SSE 解析（event_type 分发）
│   ├── message_converter.rs   — 消息格式转换（system 分离、content blocks、merge、trim）
│   └── discovery.rs           — list_models 实现（模型发现）
├── gemini/
│   ├── mod.rs                 — GeminiAdapter struct
│   ├── request.rs             — 请求构建（streamGenerateContent）+ 认证
│   ├── types.rs               — Gemini 请求/响应类型
│   ├── stream.rs              — SSE 解析（candidates 结构）+ tool call 状态管理
│   ├── message_converter.rs   — 消息格式转换（system→systemInstruction、role 映射、tool_result）
│   └── discovery.rs           — list_models 实现（模型发现）
├── stream/
│   ├── mod.rs                 — 流处理共享导出
│   ├── timeout.rs             — StreamTimeoutController
│   ├── stats.rs               — StreamStats 结构化统计
│   ├── sse.rs                 — SSE 通用解析（eventsource_stream 封装）
│   └── inline_think.rs        — inline thinking 解析（DeepSeek 等将 thinking 嵌入 text）
├── tool_call_accumulator.rs   — 流式 tool_call 片段拼装（id/name/arguments 增量合并）
├── trace.rs                   — 请求/响应追踪（debug 模式输出完整信息，脱敏 API Key）
├── diagnostics.rs             — 诊断信息（敏感信息控制）
├── crypto.rs                  — API 密钥加密（保留）
├── sse_log.rs                 — SSE 日志收集（保留）
└── lib.rs                     — 模块声明 + create_provider_adapter 工厂
```

### 3.2 核心类型关系

```
                    ┌─────────────────┐
                    │ BaseProviderAdapter │  (trait, 不变)
                    └────────┬────────┘
                             │ chat_completion_stream()
                             │ 返回 ChatStream = Stream<ChatDelta>
                             │
              ┌──────────────┼──────────────┐
              │              │              │
     ┌────────┴────┐  ┌─────┴─────┐  ┌─────┴─────┐
     │OpenAIAdapter │  │AnthropicAdapter│  │GeminiAdapter│
     └──────┬──────┘  └─────┬─────┘  └─────┬─────┘
            │               │               │
            │ 各自的 SSE 流 │               │
            ▼               ▼               ▼
     OpenAISSEData   AnthropicSSEEvent   GeminiSSEChunk
            │               │               │
            ▼               ▼               ▼
     into_unified_responses()  ←── 统一转换接口
            │               │               │
            └───────┬───────┘───────────────┘
                    ▼
            Vec<UnifiedResponse>
                    │
                    ▼  (桥接层)
              ChatDelta  ←── 前端消费的类型（保持不变）
```

---

## 4. 各 Phase 规格

### Phase 1: 基础设施 — UnifiedResponse + 文件拆分 + 测试

**目标**：引入 `UnifiedResponse`，将 `openai.rs` 拆分为子模块，添加核心测试。不改变外部接口。

#### 4.1.1 UnifiedResponse 类型

位于 `src/types/unified.rs`，是整个适配层的核心中间类型。每个 SSE chunk 经 `into_unified_responses()` 拆分为一个或多个 `UnifiedResponse`，再经 `into_chat_deltas()` 桥接为前端不变的 `ChatDelta`。

| 字段 | 类型 | 说明 |
|------|------|------|
| `text` | `Option<String>` | 文本内容 |
| `reasoning_content` | `Option<String>` | 推理/思考内容 |
| `reasoning_content_kind` | `Option<ReasoningContentKind>` | 区分 Thinking（Anthropic thinking block）vs Reasoning（DeepSeek reasoning_content），前端可差异化渲染 |
| `tool_call` | `Option<UnifiedToolCall>` | 工具调用片段（index/id/name/arguments） |
| `tool_call_completion` | `Option<ToolCallCompletion>` | 终止原因：NormalNoToolUse / NormalToolUse / OutputLimit / ContentFiltered / Failed / Unknown |
| `finish_reason` | `Option<String>` | 原始 finish_reason 字符串 |
| `usage` | `Option<UnifiedTokenUsage>` | Token 用量统计 |
| `thinking_signature` | `Option<String>` | Anthropic extended thinking 返回签名，后续轮次必须回传才能继续 thinking |
| `provider_metadata` | `Option<Value>` | 各 provider 特有元数据透传（如 Anthropic cache 信息、Gemini safety ratings） |

#### 4.1.2 UnifiedTokenUsage 类型

| 字段 | 类型 | 说明 |
|------|------|------|
| `prompt_token_count` | `u32` | 输入 token |
| `candidates_token_count` | `u32` | 输出 token |
| `total_token_count` | `u32` | 总 token |
| `cached_content_token_count` | `Option<u32>` | 缓存读取 token（DeepSeek prompt_cache_hit_tokens / Anthropic cache_read） |
| `cache_creation_token_count` | `Option<u32>` | 缓存写入 token（Anthropic 专有，与读取是两个独立指标，用于准确计算缓存成本） |
| `reasoning_token_count` | `Option<u32>` | 推理 token |

**设计要点**：
- 一个 SSE chunk 可拆分为多个 `UnifiedResponse`（text + tool_call + finish_reason 分离）
- 多 tool_calls 场景下每个 tool_call 独立一个事件
- `ToolCallCompletion::Failed` 对应 Anthropic 的 `refusal` / Gemini 的 `SAFETY` 等拒绝场景

#### 4.1.3 OpenAI SSE 类型兼容字段

在 `openai/types.rs` 的 `Delta` 结构中增加：

- `reasoning_content: Option<String>` — DeepSeek/Qwen 标准推理字段
- `reasoning: Option<String>` — Qwen 备选字段
- `reasoning_details: Option<Vec<ReasoningDetail>>` — MiniMax 推理详情（含 type + text）
- `tool_calls: Option<Vec<OpenAIToolCall>>` — 工具调用

在 `OpenAIUsage` 中增加：
- `prompt_cache_hit_tokens: Option<u32>` — DeepSeek 扩展缓存命中 token
- `prompt_tokens_details.cached_tokens` — OpenAI 官方缓存字段

#### 4.1.4 `into_unified_responses()` 转换逻辑

位于 `openai/stream.rs`，核心转换流程：

1. **reasoning 兼容归一**：优先级 `reasoning_content` > `reasoning` > `reasoning_details`（MiniMax 取 type="reasoning.text" 的 text 拼接）
2. **text + reasoning 事件**：同一 chunk 中 text 和 reasoning_content 合入一个 UnifiedResponse
3. **tool_call 拆分**：每个 tool_call 独立一个 UnifiedResponse，首个携带 usage
4. **finish_reason 事件**：合并到最后一个响应，或独立一个新响应
5. **usage 兜底**：无任何内容时仍输出 usage 事件

#### 4.1.5 桥接层：UnifiedResponse → ChatDelta

位于 `types/chat_delta.rs`，`into_chat_deltas()` 方法将 UnifiedResponse 拆分为 ChatDelta 序列：

- text/reasoning_content → 一个 ChatDelta（delta_text + delta_reasoning）
- tool_call → 一个 ChatDelta（delta_tool_call_id/name/arguments）
- finish_reason/usage → 一个 ChatDelta（finish_reason + TokenUsage 映射）
- TokenUsage 映射：`cached_content_token_count` → `cache_read_tokens`

#### 4.1.6 文件拆分清单

| 原位置 | 新位置 | 内容 |
|--------|--------|------|
| `openai.rs` L1-50 | `openai/mod.rs` | `OpenAIAdapter` struct + builders |
| `openai.rs` L51-170 | `openai/types.rs` | ChatRequest/ChatResponseChunk/Delta/Usage 等类型 |
| `openai.rs` L171-230 | `openai/error.rs` | `classify_http_error` + `parse_retry_after_secs` |
| `openai.rs` L231-580 | `openai/stream.rs` | SSE 解析 + `into_unified_responses` + 桥接 |
| `openai.rs` L581-617 | `openai/discovery.rs` | `list_models` 实现 |
| `base.rs` ChatDelta 相关 | `types/chat_delta.rs` | ChatDelta 定义（从 base.rs 积出） |
| 新增 | `types/unified.rs` | UnifiedResponse 全套类型 |
| 新增 | `types/message.rs` | 共享消息类型 |
| 新增 | `types/config.rs` | 配置类型 |
| 新增 | `types/tool.rs` | 工具定义类型 |
| 新增 | `stream/timeout.rs` | StreamTimeoutController |
| 新增 | `stream/stats.rs` | StreamStats |
| 新增 | `stream/inline_think.rs` | inline thinking 解析 |
| 新增 | `tool_call_accumulator.rs` | 流式 tool_call 拼装 |
| 新增 | `trace.rs` | 请求/响应追踪 |
| 新增 | `diagnostics.rs` | 诊断信息 |

---

### Phase 2: 流处理基础设施 — StreamTimeoutController + StreamStats + eventsource_stream + ToolCallAccumulator + InlineThink

**目标**：提取超时控制器、统计器、tool_call 拼装器、inline thinking 解析器为独立结构，引入 `eventsource_stream` crate 替代手动 SSE 解析。

#### 4.2.1 StreamTimeoutController

位于 `stream/timeout.rs`，管理两阶段超时：

- **TTFT 超时**（首 token 超时）：从流开始到收到首个内容 token 的最大等待时间
- **Idle 超时**（空闲超时）：收到首 token 后，两个 chunk 之间的最大间隔

接口：`new(ttft_timeout, idle_timeout)` → `check()` 返回 `Option<StreamTimeoutStage>` → `observe_content(has_content)` 更新状态

#### 4.2.2 StreamStats

位于 `stream/stats.rs`，结构化统计每个流的事件分布：

- 计数器：sse_chunk_count / text_event_count / reasoning_event_count / tool_call_event_count / usage_event_count / finish_event_count / error_count
- 自定义计数器：`HashMap<&str, u32>` 用于 provider 特有事件（如 skip:unknown_event）
- `log_summary(reason)` 输出结构化 debug 日志

#### 4.2.3 ToolCallAccumulator

位于 `tool_call_accumulator.rs`，解决流式 tool_call 的片段拼装问题：

- 同一个 tool_call 的 id、name、arguments 可能跨多个 SSE chunk 到达
- Accumulator 按 `tool_call_index` 维护状态，增量合并 id（首次出现时设置）、name（首次出现时设置）、arguments（字符串追加）
- 当 `finish_reason` 到达时输出完整的 tool_call

#### 4.2.4 InlineThink 解析器

位于 `stream/inline_think.rs`，处理某些 provider 将 thinking 内容嵌入普通 text 的场景：

- DeepSeek R1 等模型在 `content` 字段中输出 `<think>...</think>` 标签包裹的推理内容
- 解析器检测 `<think>` 开始标签和 `</think>` 结束标签，将内容分离为 `reasoning_content`
- 状态机设计：OutsideThink → InsideThink → OutsideThink，跨 chunk 保持状态

#### 4.2.5 引入 eventsource_stream

**Cargo.toml 新增依赖**：`eventsource-stream = "0.2"`

替换手动 buffer + `find('\n')` 解析为 `response.bytes_stream().eventsource()`，直接获取解析好的 `data` 和 `event` 字段。

**好处**：
- 自动处理 SSE 协议细节（`event:` 字段、`id:` 字段、`retry:` 字段）
- 自动处理 `\r\n` 和 `\n` 两种行尾
- 减少 ~50 行手动解析代码
- Anthropic SSE 依赖 `event:` 字段区分事件类型，手动解析容易遗漏

---

### Phase 3: Provider 特异行为 — quirks 模块

**目标**：集中处理各 OpenAI 兼容 provider 的非标准行为。位于 `client/quirks.rs`。

#### 4.3.1 URL 检测函数

| 函数 | 匹配规则 |
|------|---------|
| `is_deepseek_url` | `api.deepseek.com` |
| `is_zhipuai_url` | `bigmodel.cn` / `api.z.ai` |
| `is_dashscope_url` | `dashscope.aliyuncs.com` / `dashscope-intl.aliyuncs.com` |
| `is_siliconflow_url` | `api.siliconflow.cn` / `api.siliconflow.com` |
| `is_minimax_url` | `api.minimax.chat` / `minimax` |
| `is_glm_url` | `bigmodel.cn` 且模型名含 `glm` |

#### 4.3.2 特异行为处理

| 函数 | 行为 |
|------|------|
| `should_append_tool_stream(url, model)` | 智谱/DashScope 在 tool_calls 后需追加空 tool_call chunk，否则流异常终止；DashScope 仅 GLM-4+/GLM-5+ 需要 |
| `apply_reasoning_toggle(body, enabled, url)` | DeepSeek: `thinking: {type: "enabled"/"disabled"}`；ZhipuAI: `thinking: {type: "enabled"/"disabled"}`；DashScope/SiliconFlow: `enable_thinking: true/false`；其他 URL 返回 false 由调用方 fallback 注入 `thinking` |
| `is_deepseek_reasoning_effort_model(model)` | DeepSeek V4 Flash/Pro 支持 reasoning_effort 参数 |
| `normalize_deepseek_reasoning_effort(effort)` | 将 low/medium/high/max/xhigh 映射到 DeepSeek 实际支持的值 |
| `is_glm_reasoning_effort_model(model)` | GLM-5.2+ 支持 reasoning_effort 参数 |
| `normalize_glm_reasoning_effort(effort)` | GLM reasoning effort 归一化 |
| `normalize_generic_reasoning_effort(effort)` | 通用 reasoning effort 归一化（low/medium/high），用于 OpenAI/Anthropic/Gemini 等标准 API |
| `wants_bearer_auth(url)` | 某些 Anthropic 兼容端点（Nous Portal、Xiaomi MiMo、Moonshot）使用 Bearer 认证而非 x-api-key |
| `is_minimax_final_chunk(json)` | MiniMax 终帧用 `chat.completion`（非 `chat.completion.chunk`），choice 用 `message` 而非 `delta` |
| `extract_sse_api_error(json)` | 检测 SSE 流内错误，识别 `context_length_exceeded` 等错误码 |
| `parse_glm_major_minor(model)` | GLM 版本号解析，用于 `should_append_tool_stream` 的精确判断 |

#### 4.3.3 在 OpenAI stream 中使用 quirks

- 每个 SSE chunk 先调用 `extract_sse_api_error` 检测流内错误
- 非标准 chunk 用 `is_minimax_final_chunk` 宽松兼容
- tool_call 流结束时按 `should_append_tool_stream` 决定是否追加空 chunk
- reasoning 开关通过 `apply_reasoning_toggle` 注入请求体

---

### Phase 3.5: Reasoning Effort 端到端打通

**目标**：让 `reasoning_enabled` / `reasoning_effort` 从前端 UI 传递到后端 quirks，实现推理强度的用户可配置化。

**参考**：OpenBitFun `ReasoningPreset` + `ReasoningPresetAction` + `compile_chat_reasoning_action`

#### 4.3.5.1 数据流全链路

```
前端 UI (AgentMode 表单)
  ↓ reasoning_enabled: bool, reasoning_effort: "low"|"medium"|"high"
前端 Store (agentModeStore)
  ↓ invoke("save_agent_mode", { ... })
Rust cmd (cmd/agent_mode.rs)
  ↓ AgentModeRow 写入 SQLite
Rust cmd (cmd/chat.rs)
  ↓ 从 mode 读取 reasoning_enabled / reasoning_effort → ChatOptions
assistant-providers (openai/chat.rs)
  ↓ ChatOptions.reasoning_enabled → apply_reasoning_toggle
  ↓ ChatOptions.reasoning_effort → normalize_*_reasoning_effort → request body
Provider API (DeepSeek / GLM / 通用)
```

#### 4.3.5.2 存储层扩展

`AgentModeRow` 新增字段：

| 字段 | SQLite 类型 | Rust 类型 | 说明 |
|------|------------|----------|------|
| `reasoning_enabled` | `INTEGER` (nullable) | `Option<bool>` | `None` = 跟随模型默认；`Some(true)` = 强制开启；`Some(false)` = 强制关闭 |
| `reasoning_effort` | `TEXT` (nullable) | `Option<String>` | `None` = 不设置；`Some("low"/"medium"/"high")` = 推理强度 |

SQLite 迁移：`ALTER TABLE agent_modes ADD COLUMN reasoning_enabled INTEGER; ALTER TABLE agent_modes ADD COLUMN reasoning_effort TEXT;`

内置模式默认值：

| mode | reasoning_enabled | reasoning_effort |
|------|-------------------|-----------------|
| builtin-default | `None` | `None` |
| builtin-quick | `None` | `None` |
| builtin-deep | `Some(true)` | `Some("high")` |
| builtin-translate | `None` | `None` |

#### 4.3.5.3 Effort 归一化

与参考项目 OpenBitFun 一致，effort 值按 provider 分三路归一化：

| 路径 | 函数 | 输入 → 输出 |
|------|------|------------|
| DeepSeek V4 | `normalize_deepseek_reasoning_effort(model, effort)` | low(flash only)→low, medium/low→high, high→high, max→max, xhigh→max |
| GLM-5.2 | `normalize_glm_reasoning_effort(effort)` | low/medium/high→high, max/xhigh→max |
| 通用 (OpenAI/Anthropic/Gemini) | `normalize_generic_reasoning_effort(effort)` | low→low, medium→medium, high→high |

前端 UI 选项仅暴露通用三档 `low / medium / high`，DeepSeek/GLM 的 `max` 由各自 normalize 函数内部映射。

#### 4.3.5.4 前端接口扩展

`AgentMode` interface 新增：

```typescript
reasoning_enabled: boolean | null;  // null = 跟随默认
reasoning_effort: string | null;    // null = 不设置, "low"|"medium"|"high"
```

UI 控件：
- **Reasoning 开关**：`Switch`，`checkedChildren="开"` / `unCheckedChildren="关"`，关闭时自动清空 `reasoning_effort`
- **Reasoning Effort 下拉**：`Select`，选项 `[{low, medium, high}]`，仅当 `reasoning_enabled === true` 时显示

#### 4.3.5.5 与参考项目的差异

| 方面 | 当前项目 | OpenBitFun | 原因 |
|------|---------|-----------|------|
| 数据模型 | `AgentModeRow` 直接存两个字段 | `ReasoningConfig` + `ReasoningPreset[]` + `ReasoningPresetAction[]` | AgentMode 已有 kind 分类，effort 直接绑定 mode |
| 前端 UI | 表单内嵌 Switch + Select | 独立 ReasoningConfigPanel + ReasoningPresetEditor | 设置页更简洁 |
| effort 值域 | `low/medium/high` 三档 | 同 + `max/xhigh` (DeepSeek/GLM) | normalize 内部映射，前端无需暴露 |
| 作用范围 | per-AgentMode | per-model-config | model 由 conversation 绑定，mode 控制推理行为更自然 |

---

### Phase 4: Anthropic 原生 Provider

**目标**：实现 `AnthropicAdapter`，支持 Anthropic Messages API 原生格式。

#### 4.4.1 Anthropic API 特点

| 特点 | 与 OpenAI 的差异 |
|------|-----------------|
| 端点 | `POST /v1/messages`（非 `/v1/chat/completions`） |
| 认证 | `x-api-key: <key>` + `anthropic-version: 2023-06-01` |
| System 消息 | 不在 messages 数组内，单独的 `system` 顶层字段 |
| Content 格式 | Content blocks 数组：`[{type: "text", text: "..."}, {type: "thinking", thinking: "...", signature: "..."}]` |
| Tool use | `tool_use` content block + `tool_result` 消息（content blocks 内） |
| SSE 事件类型 | `message_start` / `content_block_start` / `content_block_delta` / `content_block_stop` / `message_delta` / `message_stop` |
| Thinking | `thinking` content block + `signature`（用于后续 replay，必须回传） |
| Max tokens | 必填字段（OpenAI 可选） |
| Streaming | `stream: true` 请求参数 |
| Cache | 支持 `cache_control` 标记，响应中返回 `cache_creation_input_tokens` 和 `cache_read_input_tokens` |

#### 4.4.2 AnthropicAdapter

位于 `anthropic/mod.rs`，结构与 OpenAIAdapter 对称：`new(config)` / `with_retry()` / `with_timeout()` / `default_base_url()` 返回 `"https://api.anthropic.com/v1"`。

#### 4.4.3 Anthropic 消息转换

位于 `anthropic/message_converter.rs`，`convert_messages(messages) → (system, anthropic_messages)`。

**转换规则**（比 OpenAI 复杂得多，参考 OpenBitFun 实现）：

1. **system 分离**：System 角色消息提取为顶层 `system` 字段，不进入 messages 数组
2. **content blocks 封装**：每条消息的 content 包装为 `[{type: "text", text: "..."}]` 数组
3. **连续同 role 合并**（`merge_consecutive_messages`）：Anthropic API 不允许连续两条相同 role 的消息，需将相邻同 role 消息的 content blocks 合并
4. **尾部空白裁剪**（`trim_final_assistant_trailing_whitespace`）：Anthropic 拒绝 assistant prefill 以空白结尾的文本，需裁剪
5. **tool_result 消息转换**：Tool 角色消息转为 `{role: "user", content: [{type: "tool_result", tool_use_id, content}]}` 格式
6. **image content blocks**：图片消息转为 `{type: "image", source: {type: "base64", media_type, data}}` 格式
7. **thinking content blocks**：历史 thinking 消息转为 `{type: "thinking", thinking, signature}` 格式，signature 必须回传

#### 4.4.4 Anthropic SSE 流处理

位于 `anthropic/stream.rs`，基于 eventsource_stream 的 `event` 字段分发：

| 事件类型 | 处理 |
|---------|------|
| `message_start` | 提取 usage、model 信息 |
| `content_block_start` | Thinking block → `{reasoning_content, reasoning_content_kind: Thinking}`；ToolUse block → `{tool_call}` |
| `content_block_delta` | text_delta → `{text}`；thinking_delta → `{reasoning_content}`；input_json_delta → `{tool_call.arguments}` |
| `content_block_stop` | 当前 block 结束，thinking block 时提取 signature 到 `{thinking_signature}` |
| `message_delta` | stop_reason → `{finish_reason, tool_call_completion}`；usage → `{usage, cache_creation_token_count}` |
| `message_stop` | 流结束 |
| `error` | SSE 流内错误，转为 ProviderError |

#### 4.4.5 Anthropic 兼容端点认证

位于 `anthropic/request.rs`，`apply_headers()` 根据URL选择认证方式：

- **标准 Anthropic**（api.anthropic.com）：`x-api-key` + `anthropic-version`
- **Bearer 认证端点**（bigmodel.cn / api.z.ai / api.kimi.com/coding / api.moonshot.{cn,ai}/anthropic / inference-api.nousresearch.com / api.xiaomimimo.com/anthropic）：`Authorization: Bearer` + `anthropic-version`
- 通过 `quirks::wants_bearer_auth(url)` 统一判断

---

### Phase 5: Gemini 原生 Provider

**目标**：实现 `GeminiAdapter`，支持 Gemini `streamGenerateContent` API。

#### 4.5.1 Gemini API 特点

| 特点 | 与 OpenAI 的差异 |
|------|-----------------|
| 端点 | `POST /v1beta/models/{model}:streamGenerateContent?alt=sse` |
| 认证 | `x-goog-api-key: <key>` 或 URL 参数 `?key=<key>` |
| System 消息 | 顶层 `systemInstruction` 字段 |
| Role 映射 | `user` → `user`，`assistant` → `model` |
| Content 格式 | `parts` 数组：`[{text: "..."}, {functionCall: {name, args}}]` |
| Tool use | `functionCall` part + `functionResponse` part |
| SSE 格式 | 无 `data:` 前缀，直接 JSON 行（`alt=sse` 时加 `data:` 前缀） |
| Thinking | `thinkingConfig: { includeThoughts: true, thinkingBudget: N }`，thought 在 part 中 |
| 无 `messages` 字段 | 用 `contents` 数组 |
| Safety | 响应含 `safetyRatings`，透传到 `provider_metadata` |

#### 4.5.2 GeminiAdapter

位于 `gemini/mod.rs`，结构与 OpenAIAdapter 对称：`new(config)` / `with_retry()` / `with_timeout()` / `default_base_url()` 返回 `"https://generativelanguage.googleapis.com"`。

#### 4.5.3 Gemini 消息转换

位于 `gemini/message_converter.rs`，`convert_messages(messages) → (system_instruction, contents)`。

**转换规则**（参考 OpenBitFun 实现）：

1. **system → systemInstruction**：System 角色消息转为顶层 `{parts: [{text}]}` 格式
2. **role 映射**：`assistant` → `model`
3. **parts 封装**：每条消息的 content 包装为 `{parts: [{text}]}` 格式
4. **tool_result / functionResponse**：Tool 角色消息转为 `{role: "user", parts: [{functionResponse: {name, response}}]}` 格式
5. **image parts**：图片消息转为 `{inline_data: {mime_type, data}}` 格式
6. **thoughtSignature**：历史 thinking 消息需回传 thought signature（类似 Anthropic）

#### 4.5.4 Gemini SSE 流处理

位于 `gemini/stream.rs`，解析 `GeminiStreamResponse { candidates, usageMetadata }` 结构：

- **candidates[].content.parts[]** 遍历：
  - `text` → `{text}`
  - `thought` → `{reasoning_content, reasoning_content_kind: Thinking}`
  - `functionCall` → `{tool_call}`（需 ToolCallAccumulator 拼装，因为 name 和 args 可能分次到达）
- **candidates[].finishReason** → `{finish_reason}`
- **usageMetadata** → `{usage}`（含 cachedContentTokenCount）

**tool call 状态管理**：Gemini 的 functionCall 在 `content_block_start` 时给出 name，arguments 在后续 delta 中增量到达，需用 ToolCallAccumulator 拼装。

---

### Phase 6: Provider 工厂 + 配置扩展

**目标**：更新 `create_provider_adapter` 工厂函数，根据 `ProviderKind` 返回对应 adapter。

#### 4.6.1 工厂函数

位于 `lib.rs`，`create_provider_adapter(config) → Box<dyn BaseProviderAdapter>`：

- `ProviderKind::OpenAI` / `ProviderKind::Custom` → `OpenAIAdapter::new(config)`
- `ProviderKind::Anthropic` → `AnthropicAdapter::new(config)`
- `ProviderKind::Gemini` → `GeminiAdapter::new(config)`

#### 4.6.2 ProviderKind 扩展

`ProviderKind` 枚举：`OpenAI` / `Anthropic` / `Gemini` / `Custom { base_url }`

新增方法 `use_native_api() → bool`：Anthropic/Gemini 返回 true（使用原生 API），其余返回 false（使用 OpenAI 兼容格式）。

#### 4.6.3 前端 Provider 类型配置

当前前端 `provider.type` 字段值为 `"OpenAI"` / `"Anthropic"` / `"Gemini"` / 自定义。
**无需修改前端**，`provider_kind_from_str` 已支持映射。

---

### Phase 7: OpenAI Responses API + Gemini Code Assist ✅

#### OpenAI Responses API ✅

- [x] `openai/responses_types.rs` — ResponsesStreamEvent / ResponsesUsage / replay capture 类型定义 + 解析函数 + 8 个单元测试
- [x] `openai/responses_stream.rs` — SSE 流处理（事件分发 + InProgressToolCall 跟踪 + arguments 尾部补齐 + replay capture + response.failed/incomplete 处理）
- [x] `ProviderKind::OpenAIResponses` 枚举变体 + `provider_kind_from_str` 映射
- [x] `UnifiedToolCall.arguments_is_snapshot: bool` 字段（G5 差距关闭）
- [x] `UnifiedResponse.model_response_replay: Option<ModelResponseReplayCapture>` 字段（G6 差距关闭）
- [x] `ReasoningContentKind::Summary` 变体（reasoning_summary_text.delta 支持）
- [x] `ModelResponseReplayCapture` / `ModelResponseReplayItem` / `ReasoningSummaryPart` 类型

#### Gemini Code Assist ✅

- [x] `gemini/code_assist.rs` — Code Assist 流处理（v1internal:streamGenerateContent 端点 + thinkingConfig + 默认模型列表）
- [x] `ProviderKind::GeminiCodeAssist` 枚举变体 + `provider_kind_from_str` 映射
- [x] Code Assist 专用 base URL（`https://cloudcode-pa.googleapis.com`）

---

## 5. 实施顺序与依赖关系

```
Phase 1: UnifiedResponse + 文件拆分 + 测试
    │
    ▼
Phase 2: StreamTimeoutController + StreamStats + eventsource_stream
         + ToolCallAccumulator + InlineThink
    │
    ▼
Phase 3: quirks 模块（Provider 特异行为）
    │
    ▼
Phase 3.5: Reasoning Effort 端到端打通
    │
    ├──────────────────────┐
    ▼                      ▼
Phase 4: Anthropic    Phase 5: Gemini
    │                      │
    └──────────┬───────────┘
               ▼
Phase 6: Provider 工厂 + 配置扩展
```

**每个 Phase 完成后**：
1. 运行 `cargo test` 确保所有测试通过
2. 运行 `cargo clippy` 确保无 lint 警告
3. 手动测试：用现有 OpenAI provider 发送对话，确认流式输出正常
4. git commit

---

## 6. 新增依赖

### 6.1 上游开源 crate

| crate | 版本 | 用途 | Phase | 优先级 | 说明 |
|-------|------|------|-------|--------|------|
| `eventsource-stream` | 0.2.3 | SSE 协议解析 | B3 | 🔴 P0 | 替换手动 buffer+find('\n')，4 个 Adapter 复用；OpenBitFun 已验证 |
| `jsonrepair` | 待定 | 流式 tool_call arguments JSON 修复 | B2+ | 🟡 P1 | OpenBitFun fork 自 `jsonrepair-rs` 0.2.1，修复 LLM 输出的截断/不完整 JSON；Phase 2 先用简单拼接，后续引入 |

### 6.2 从参考项目移植的模块（零外部依赖）

| 模块 | 来源 | 行数 | Phase | 优先级 | 说明 |
|------|------|------|-------|--------|------|
| `HiddenTextStreamParser` | OpenBitFun `agent-stream/hidden_text.rs` | ~120 | B1 | 🟢 P2 | 通用标签状态机，支持多标签跨 chunk 解析；InlineThink 是其特化版本（仅 `<tool_call>` 标签） |
| `StreamStats` | OpenBitFun `stream/stream_handler/stream_stats.rs` | ~70 | B1 | 🟢 P2 | 7 计数器 + HashMap 自定义 + wall clock + log_summary |
| `StreamTimeoutController` | OpenBitFun `stream/stream_handler/timeout.rs` | ~70 | B1 | 🟢 P2 | TTFT + Idle 两阶段超时，从 chat.rs 内联逻辑抽取 |
| `ToolCallAccumulator` | OpenBitFun `agent-stream/tool_call_accumulator.rs` | ~300 | B2 | 🔵 P3 | BTreeMap 按 index 排序，跨 chunk 拼装；需适配 UnifiedResponse 输入 + ChatDelta 输出 |
| `quirks` | OpenBitFun `client/quirks.rs` | ~150 | C1-C2 | 🔵 P3 | URL 检测 + reasoning toggle + reasoning_effort normalize + Bearer auth 检测 |

### 6.3 不建议引入的上游 SDK

| crate | 原因 |
|-------|------|
| `anthropic-sdk` | 过重，我们只需 messages API + SSE，自行实现更轻量可控 |
| `google-gemini-client` | 同上，Gemini REST API 简单直接 |
| `async-openai` | 绑定 OpenAI 特定格式，不支持 Anthropic/Gemini 原生 |
| `llm-client` | 抽象层过厚，与 UnifiedResponse 体系不兼容 |

### 6.4 Cargo.toml 变更

```toml
# assistant-providers/Cargo.toml
[dependencies]
eventsource-stream = "0.2.3"  # SSE 协议解析（P0，Phase B3）
# jsonrepair = "0.2"          # JSON 修复（P1，Phase B2+，暂缓引入）
```

---

## 7. Phase 1 实施差距（与参考项目 OpenBitFun 对比）

> Phase 1（A1-A4）已完成，以下为与参考项目成熟实现的差距，需后续 Phase 增量补齐。

### 7.1 into_unified_responses 差距

| # | 差距 | 影响 | 补补 Phase | 参考 OpenBitFun 实现 |
|---|------|------|-----------|---------------------|
| G1 | MiniMax `chat.completion` 非流式帧兼容 | ✅ 已完成 (C1) | C1 | `is_valid_chat_completion_chunk_weak()` 宽松校验 |
| G2 | `object` 字段校验 | ✅ 已完成 (C1) | C1 | 校验 `object` 字段过滤非标准事件 |
| G3 | SSE 流内 error 提取 | ✅ 已完成 (C1) | C1 | `extract_sse_api_error()` 提取流内错误 |
| G4 | `received_finish_reason` 兜底 | ✅ 已完成 (B3) | B3 | eventsource_stream 流结束检测 + finish_reason 兜底 |

### 7.2 UnifiedResponse 类型差距

| # | 缺失字段 | 用途 | 补补 Phase | 状态 |
|---|---------|------|-----------|------|
| G5 | `arguments_is_snapshot: bool` | OpenAI Responses API 的 tool_call arguments 是完整快照而非增量，影响 ToolCallAccumulator 拼装策略 | Phase 7 | ✅ 已完成 |
| G6 | `model_response_replay: Option<ModelResponseReplayCapture>` | Anthropic/Gemini thinking signature 回传；✅ D2/D3 已实现 Anthropic thinking_signature 透传，Gemini 侧待 E2 | D2/E2 | ✅ 已完成（Responses API replay capture + Anthropic thinking_signature） |

### 7.3 流处理基础设施差距

| # | 差距 | 当前状态 | 补补 Phase |
|---|------|---------|-----------|
| G7 | StreamTimeoutController | ✅ 已完成 (B1) | B1 |
| G8 | StreamStats | ✅ 已完成 (B1) | B1 |
| G9 | InlineThink 状态机 | ✅ 已完成 (B1) | B1 |
| G10 | ToolCallAccumulator | ✅ 已完成 (B2) | B2 |
| G11 | eventsource_stream | ✅ 已完成 (B3) | B3 |

### 7.4 quirks 差距

| # | 差距 | 当前状态 | 补补 Phase |
|---|------|---------|-----------|
| G12 | URL 检测（DeepSeek/ZhipuAI/DashScope/SiliconFlow/MiniMax/GLM） | ✅ 已完成 (C1) | C1 |
| G13 | reasoning toggle（OpenAI 兼容 provider 的 reasoning 开关注入） | ✅ 已完成 (C2+C'4) | C2 |
| G14 | reasoning_effort normalize（DeepSeek/GLM 的 reasoning_effort 值归一化） | ✅ 已完成 (C2) | C2 |
| G15 | Bearer auth 检测（Anthropic 兼容端点认证方式选择） | ✅ 已完成 (C1) | C1 |
| G16 | generic reasoning_effort normalize（OpenAI/Anthropic/Gemini 通用 effort 归一化） | ✅ 已完成 (C'4) | C'4 |
| G17 | reasoning effort 端到端（前端 UI → SQLite → ChatOptions → request body） | ✅ 已完成 (C'1-C'5) | C' |
| G18 | ZhipuAI GLM reasoning toggle（智谱 GLM-5.2 的 thinking 开关） | ✅ 已完成 (C'4) | C'4 |
| G19 | generic reasoning toggle fallback（非特异 URL 的 thinking 开关兜底） | ✅ 已完成 (C'4) | C'4 |

---

## 8. 风险与缓解

| 风险 | 缓解措施 |
|------|---------|
| 文件拆分引入编译错误 | 拆分后立即 `cargo check`，逐步迁移 |
| eventsource_stream crate 行为差异 | 先在测试中验证 SSE 解析结果与手动解析一致 |
| Anthropic/Gemini SSE 格式差异大 | 每个 Adapter 各自独立 stream.rs，互不影响 |
| MiniMax 等非标准 provider 终帧格式 | quirks 模块集中处理，`is_minimax_final_chunk` 宽松检测 |
| 前端 ChatDelta 接口变化 | ChatDelta 结构不变，桥接层 `into_chat_deltas()` 保证兼容 |
| 重试逻辑在流中断时行为 | StreamTimeoutController 统一管理，超时后走重试逻辑 |
| Anthropic 连续同 role 消息 | message_converter 中 merge_consecutive_messages 处理 |
| Anthropic assistant prefill 尾部空白 | trim_final_assistant_trailing_whitespace 裁剪 |
| 流式 tool_call 片段拼装 | ToolCallAccumulator 按 index 增量合并 id/name/arguments |
| DeepSeek inline thinking 混入 text | InlineThink 解析器用状态机分离 `<think>` 标签 |
| API Key 日志泄露 | trace.rs 中脱敏处理，diagnostics.rs 控制敏感信息输出 |

---

## 8. Phase 7 验证清单

### H1 编译验证 ✅

- [x] `cargo check -p assistant-providers` 通过
- [x] `cargo clippy -p assistant-providers` 无实质 warning
- [x] `cargo test -p assistant-providers` — 171 个单元测试全部通过

### H2 功能验证（运行时）

| # | 验证项 | 状态 |
|---|--------|------|
| 1 | OpenAI Responses API 流式对话正常（文本 + reasoning + reasoning_summary + tool_call） | ⬜ |
| 2 | Responses API `output_item.done` arguments 尾部补齐正确 | ⬜ |
| 3 | Responses API `response.failed` / `response.incomplete` 事件正确处理 | ⬜ |
| 4 | Responses API `model_response_replay` capture 正确构建（含 opaque reasoning） | ⬜ |
| 5 | Gemini Code Assist 流式对话正常（文本 + thinking + function_call） | ⬜ |
| 6 | `ProviderKind::OpenAIResponses` 工厂分发到 responses_stream | ⬜ |
| 7 | `ProviderKind::GeminiCodeAssist` 工厂分发到 code_assist_stream | ⬜ |
| 8 | ChatDelta 接口不变，前端无需修改 | ⬜ |
# I9 — Provider 多 API 格式适配 任务清单

> 规格：[I9-provider-multi-api-spec.md](./I9-provider-multi-api-spec.md)
> 前置：I8 ✅（声明式设置系统）

---

## 阶段 A — Phase 1：UnifiedResponse + 类型系统 + openai.rs 拆分

- [x] A1. 新建 `types/` 模块：unified.rs（UnifiedResponse/UnifiedToolCall/UnifiedTokenUsage/ReasoningContentKind/ToolCallCompletion）+ chat_delta.rs（从 base.rs 迁出 ChatDelta + into_chat_deltas 桥接）+ message.rs + config.rs + tool.rs + mod.rs 统一导出
- [x] A2. `openai.rs` 拆分为 `openai/` 子模块：mod.rs（adapter + builders）+ types.rs（请求/响应类型 + reasoning_content/reasoning/reasoning_details/tool_calls 兼容字段 + prompt_cache_hit_tokens）+ error.rs + stream.rs + chat.rs + message_converter.rs + discovery.rs；删除原单文件，更新 lib.rs 模块声明
- [x] A3. `openai/stream.rs` 实现 `into_unified_responses()` 转换：reasoning 兼容归一（优先级 reasoning_content > reasoning > reasoning_details）+ text/reasoning 合并 + tool_call 拆分 + finish_reason 合并 + usage 兜底
- [x] A4. `types/chat_delta.rs` 实现 `into_chat_deltas()` 桥接：text/reasoning/tool_call/finish_reason/usage → ChatDelta 映射 + TokenUsage 映射（cached_content_token_count → cache_read_tokens）

---

## 阶段 B — Phase 2：流处理基础设施

- [x] B1. 新建 `stream/` 模块：timeout.rs（StreamTimeoutController，TTFT + Idle 两阶段超时）+ stats.rs（StreamStats，7 计数器 + HashMap 自定义 + log_summary）+ sse.rs（eventsource_stream 封装）+ inline_think.rs（InlineThink 状态机，OutsideThink → InsideThink → OutsideThink，跨 chunk 保持状态）
- [x] B2. 新建 `tool_call_accumulator.rs`：ToolCallAccumulator（按 tool_call_index 增量合并 id/name/arguments）
- [x] B3. `Cargo.toml` 新增 `eventsource-stream = "0.2"`；`openai/stream.rs` 替换手动 buffer + find('\n') 为 eventsource_stream

---

## 阶段 C — Phase 3：Provider 特异行为 quirks 模块

- [x] C1. 新建 `client/mod.rs`（AIClient struct + 配置）+ `client/quirks.rs`：URL 检测（is_deepseek_url / is_zhipuai_url / is_dashscope_url / is_siliconflow_url / is_minimax_url / is_glm_url）+ 特异行为函数（should_append_tool_stream / apply_reasoning_toggle / normalize_deepseek_reasoning_effort / normalize_glm_reasoning_effort / parse_glm_major_minor / wants_bearer_auth / is_minimax_final_chunk / extract_sse_api_error）
- [x] C2. OpenAI 集成 quirks：stream.rs 每个 chunk 调用 extract_sse_api_error + is_minimax_final_chunk + should_append_tool_stream；chat.rs 通过 apply_reasoning_toggle 注入 reasoning 开关；ChatOptions 补充 reasoning_enabled / reasoning_effort 字段；SSE error 消息附加原始 data 提升诊断性

---

## 阶段 C' — Phase 3.5：Reasoning Effort 端到端打通

> 目标：让 `reasoning_enabled` / `reasoning_effort` 从前端 UI 传递到后端 quirks，实现推理强度的用户可配置化。
> 参考：OpenBitFun `ReasoningPreset` + `ReasoningPresetAction` + `compile_chat_reasoning_action`

### 数据流全链路

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

### 任务

- [x] C'1. **Rust 存储层**：`AgentModeRow` 新增 `reasoning_enabled: Option<bool>` + `reasoning_effort: Option<String>` 字段；SQLite ALTER TABLE 迁移（`reasoning_enabled INTEGER` + `reasoning_effort TEXT`）；`read_mode_row` / `insert` / `update` 适配
- [x] C'2. **Rust 命令层**：`cmd/chat.rs` 从 `mode.reasoning_enabled` / `mode.reasoning_effort` 传入 `ChatOptions`（替换当前 `None` 硬编码）
- [x] C'3. **前端类型层**：`AgentMode` interface 新增 `reasoning_enabled: boolean | null` + `reasoning_effort: string | null`；`saveMode` payload 同步
- [x] C'4. **前端 UI 层**：AgentMode 编辑表单新增「Reasoning」Switch + 「Reasoning Effort」Select（low / medium / high），关闭时清空 effort；`normalize_generic_reasoning_effort` 从参考项目移植到 quirks.rs；chat.rs else 分支调用归一化
- [x] C'5. **Rust 命令层 save**：`cmd/agent_mode.rs` 解析 `reasoning_enabled` / `reasoning_effort` 写入 `AgentModeRow`

### 设计说明

| 方面 | 当前项目方案 | 参考项目 (OpenBitFun) 方案 | 差异原因 |
|------|-------------|--------------------------|---------|
| 数据模型 | `AgentModeRow` 直接存 `reasoning_enabled` + `reasoning_effort` 两个简单字段 | `ReasoningConfig` + `ReasoningPreset[]` + `ReasoningPresetAction[]` 完整预设系统 | 当前项目 AgentMode 已有 kind（default/quick/deep/custom），reasoning_effort 可直接绑定到 mode，无需额外预设层 |
| 前端 UI | mode 编辑表单内嵌 toggle + select | 独立 ReasoningConfigPanel + ReasoningPresetEditor 页面 | 当前项目设置页更简洁，内嵌即可 |
| effort 值域 | `"low"` / `"medium"` / `"high"` 三档（与参考项目 generic 一致） | 同 + `"max"` / `"xhigh"` (DeepSeek/GLM 特有) | `normalize_generic_reasoning_effort` 仅 low/medium/high；DeepSeek/GLM 的 normalize 函数内部映射到 high/max，前端选项无需暴露 |
| 作用范围 | per-AgentMode（每个模式独立设置） | per-model-config（模型配置级别） | 当前项目 model 由 conversation 绑定，mode 控制推理行为更自然 |

---

## 阶段 D — Phase 4：Anthropic 原生 Provider

- [x] D1. 新建 `anthropic/` 模块：mod.rs（AnthropicAdapter + new/with_retry/with_timeout/default_base_url）+ types.rs（MessagesRequest/MessagesResponse/ContentBlock/ToolUseBlock/ThinkingBlock）+ request.rs（Messages API 端点 + apply_headers 认证 + Bearer 端点支持 via quirks::wants_bearer_auth）+ discovery.rs
- [x] D2. 新建 `anthropic/message_converter.rs`：convert_messages — system 分离 + content blocks 封装 + merge_consecutive_messages + trim_final_assistant_trailing_whitespace + tool_result 转换 + image content blocks + thinking content blocks（signature 回传）
- [x] D3. 新建 `anthropic/stream.rs`：event_type 分发（message_start / content_block_start / content_block_delta / content_block_stop / message_delta / message_stop / error）+ thinking_signature 透传 + cache_creation_token_count 填充

---

## 阶段 E — Phase 5：Gemini 原生 Provider

- [x] E1. 新建 `gemini/` 模块：mod.rs（GeminiAdapter + new/with_retry/with_timeout/default_base_url）+ types.rs（GenerateContentRequest/GeminiStreamResponse/Candidate/Part/FunctionCall/UsageMetadata）+ request.rs（streamGenerateContent 端点 + x-goog-api-key 认证 + alt=sse）+ discovery.rs
- [x] E2. 新建 `gemini/message_converter.rs`：convert_messages — system→systemInstruction + role 映射（assistant→model）+ parts 封装 + tool_result→functionResponse + image parts + thoughtSignature 回传
- [x] E3. 新建 `gemini/stream.rs`：candidates/parts 遍历（text / thought / functionCall）+ ToolCallAccumulator 拼装 + usageMetadata 映射（含 cachedContentTokenCount）+ safetyRatings 透传到 provider_metadata

---

## 阶段 F — Phase 6：Provider 工厂 + 配置 + 追踪诊断

- [x] F1. `lib.rs` 更新 `create_provider_adapter` 工厂：ProviderKind::Anthropic → AnthropicAdapter，ProviderKind::Gemini → GeminiAdapter；`ProviderKind` 新增 `use_native_api() → bool`；确认前端无需修改
- [x] F2. 新建 `trace.rs`（请求/响应追踪，debug 模式输出完整信息，API Key 脱敏）+ `diagnostics.rs`（敏感信息输出级别控制）

---

## 阶段 G — 核心测试

- [x] G1. UnifiedResponse 转换测试：text_and_reasoning_in_same_chunk + multiple_tool_calls_split + minimax_reasoning_details_fallback + qwen_reasoning_field_fallback + deepseek_cache_hit_tokens + thinking_signature_passthrough + cache_creation_token_count
- [x] G2. 流处理测试：tool_call_accumulator_merge + inline_think_extraction + stream_timeout_controller（TTFT + Idle）+ stream_stats_counters
- [x] G3. 错误/重试测试：classify_http_error_all_codes + parse_retry_after_secs_formats + retry_config_exponential_backoff

---

## 阶段 H — 验收

### H1. 编译验证

- [x] `cargo check` 通过
- [x] `cargo clippy` 无新增 warning
- [x] `cargo test` 通过

**依赖**：A1~A4, B1~B3, C1~C2, D1~D3, E1~E3, F1~F2, G1~G3

---

### H2. 功能验证

- [ ] OpenAI provider 流式对话正常（文本 + reasoning + tool_call）
- [ ] DeepSeek / MiniMax / Qwen reasoning 字段正确提取
- [ ] Reasoning effort 端到端：前端设置 reasoning_effort=low → ChatOptions 传入 → quirks normalize → 请求体包含正确字段
- [ ] Anthropic Messages API 流式对话正常 + thinking signature 回传 + 连续同 role 合并
- [ ] Gemini streamGenerateContent 流式对话正常 + thought 分离 + functionCall 拼装
- [ ] Provider 工厂根据 ProviderKind 返回正确 adapter
- [ ] ChatDelta 接口不变，前端无需修改
- [ ] API Key 在日志中脱敏

**依赖**：H1

---

## 上游依赖引入计划

### 🔴 P0 — 必须引入的上游 crate

| crate | 版本 | Phase | 用途 | 行动 |
|-------|------|-------|------|------|
| `eventsource-stream` | 0.2.3 | B3 | SSE 协议解析，替换手动 buffer+find('\n') | 添加到 workspace Cargo.toml + assistant-providers/Cargo.toml |

### 🟡 P1 — 后续引入的上游 crate

| crate | 版本 | Phase | 用途 | 行动 |
|-------|------|-------|------|------|
| `jsonrepair` | 待定 | B2+ | 流式 tool_call arguments JSON 修复 | Phase 2 先用简单拼接，后续评估引入 `jsonrepair-rs` 或 fork OpenBitFun 的 `tool-call-jsonrepair` |

### 🟢 P2 — 从参考项目移植（零外部依赖）

| 模块 | 来源路径 | 行数 | Phase | 移植要点 |
|------|---------|------|-------|---------|
| `HiddenTextStreamParser` | `agent-stream/hidden_text.rs` | ~120 | B1 | 通用标签状态机；InlineThink = `new(vec![think_tag])` 特化 |
| `StreamStats` | `stream/stream_handler/stream_stats.rs` | ~70 | B1 | 7 计数器 + HashMap + wall clock + log_summary |
| `StreamTimeoutController` | `stream/stream_handler/timeout.rs` | ~70 | B1 | 从 chat.rs 内联逻辑抽取为独立结构体 |

### 🔵 P3 — 参考项目适配移植

| 模块 | 来源路径 | 行数 | Phase | 适配要点 |
|------|---------|------|-------|---------|
| `ToolCallAccumulator` | `agent-stream/tool_call_accumulator.rs` | ~300 | B2 | 输入改 UnifiedResponse，输出改 ChatDelta；JSON 修复部分先跳过 |
| `quirks` | `client/quirks.rs` | ~150 | C1-C2 | URL 检测 + reasoning toggle + reasoning_effort normalize + Bearer auth |

### ❌ 不建议引入

| crate | 原因 |
|-------|------|
| `anthropic-sdk` | 过重，只需 messages API + SSE |
| `google-gemini-client` | Gemini REST API 简单直接 |
| `async-openai` | 绑定 OpenAI 格式，不支持 Anthropic/Gemini 原生 |
| `llm-client` | 抽象层过厚，与 UnifiedResponse 体系不兼容 |

---

## Phase 1 实施差距（G1-G19）

> Phase 1（A1-A4）已完成 ✅，以下差距已由后续 Phase 增量补齐。

### into_unified_responses 差距

| # | 差距 | 状态 | 补补 Phase |
|---|------|------|-----------|
| G1 | MiniMax `chat.completion` 非流式帧兼容 | ✅ | C1 |
| G2 | `object` 字段校验过滤非标准事件 | ✅ | C1 |
| G3 | SSE 流内 `{"error":{...}}` 提取 | ✅ | C1 |
| G4 | 流提前关闭时 `received_finish_reason` 兜底 | ✅ | B3 |

### UnifiedResponse 类型差距

| # | 缺失字段 | 状态 | 补补 Phase |
|---|---------|------|-----------|
| G5 | `arguments_is_snapshot: bool` | ⬜ | Phase 7 候选 |
| G6 | `model_response_replay` / thinking_signature | ✅ D2/D3 已实现 thinking_signature 透传，Gemini 侧待 E2 | D2/E2 |

### 流处理基础设施差距

| # | 差距 | 状态 | 补补 Phase |
|---|------|------|-----------|
| G7 | StreamTimeoutController 独立化 | ✅ | B1 |
| G8 | StreamStats 结构化统计 | ✅ | B1 |
| G9 | InlineThink 状态机 | ✅ | B1 |
| G10 | ToolCallAccumulator 跨 chunk 拼装 | ✅ | B2 |
| G11 | eventsource_stream 替换手动解析 | ✅ | B3 |

### quirks 差距

| # | 差距 | 状态 | 补补 Phase |
|---|------|------|-----------|
| G12 | URL 检测 | ✅ | C1 |
| G13 | reasoning toggle | ✅ | C2 |
| G14 | reasoning_effort normalize | ✅ | C2 |
| G15 | Bearer auth 检测 | ✅ | C1 |
| G16 | generic reasoning_effort normalize | ✅ | C'4 |
| G17 | reasoning effort 端到端打通 | ✅ | C'1-C'5 |
| G18 | ZhipuAI GLM reasoning toggle | ✅ | C'4 |
| G19 | generic reasoning toggle fallback | ✅ | C'4 |
# I9 — Provider 多 API 格式适配 任务清单

> 规格：[I9-provider-multi-api-spec.md](./I9-provider-multi-api-spec.md)
> 前置：I8 ✅（声明式设置系统）

---

## 阶段 A — Phase 1：UnifiedResponse + 类型系统 + openai.rs 拆分

- [ ] A1. 新建 `types/` 模块：unified.rs（UnifiedResponse/UnifiedToolCall/UnifiedTokenUsage/ReasoningContentKind/ToolCallCompletion）+ chat_delta.rs（从 base.rs 迁出 ChatDelta + into_chat_deltas 桥接）+ message.rs + config.rs + tool.rs + mod.rs 统一导出
- [ ] A2. `openai.rs` 拆分为 `openai/` 子模块：mod.rs（adapter + builders）+ types.rs（请求/响应类型 + reasoning_content/reasoning/reasoning_details/tool_calls 兼容字段 + prompt_cache_hit_tokens）+ error.rs + stream.rs + chat.rs + message_converter.rs + discovery.rs；删除原单文件，更新 lib.rs 模块声明
- [ ] A3. `openai/stream.rs` 实现 `into_unified_responses()` 转换：reasoning 兼容归一（优先级 reasoning_content > reasoning > reasoning_details）+ text/reasoning 合并 + tool_call 拆分 + finish_reason 合并 + usage 兜底
- [ ] A4. `types/chat_delta.rs` 实现 `into_chat_deltas()` 桥接：text/reasoning/tool_call/finish_reason/usage → ChatDelta 映射 + TokenUsage 映射（cached_content_token_count → cache_read_tokens）

---

## 阶段 B — Phase 2：流处理基础设施

- [ ] B1. 新建 `stream/` 模块：timeout.rs（StreamTimeoutController，TTFT + Idle 两阶段超时）+ stats.rs（StreamStats，7 计数器 + HashMap 自定义 + log_summary）+ sse.rs（eventsource_stream 封装）+ inline_think.rs（InlineThink 状态机，OutsideThink → InsideThink → OutsideThink，跨 chunk 保持状态）
- [ ] B2. 新建 `tool_call_accumulator.rs`：ToolCallAccumulator（按 tool_call_index 增量合并 id/name/arguments）
- [ ] B3. `Cargo.toml` 新增 `eventsource-stream = "0.2"`；`openai/stream.rs` 替换手动 buffer + find('\n') 为 eventsource_stream

---

## 阶段 C — Phase 3：Provider 特异行为 quirks 模块

- [ ] C1. 新建 `client/mod.rs`（AIClient struct + 配置）+ `client/quirks.rs`：URL 检测（is_deepseek_url / is_zhipuai_url / is_dashscope_url / is_siliconflow_url / is_minimax_url / is_glm_url）+ 特异行为函数（should_append_tool_stream / apply_reasoning_toggle / normalize_deepseek_reasoning_effort / normalize_glm_reasoning_effort / parse_glm_major_minor / wants_bearer_auth / is_minimax_final_chunk / extract_sse_api_error）
- [ ] C2. OpenAI 集成 quirks：stream.rs 每个 chunk 调用 extract_sse_api_error + is_minimax_final_chunk + should_append_tool_stream；chat.rs 通过 apply_reasoning_toggle 注入 reasoning 开关

---

## 阶段 D — Phase 4：Anthropic 原生 Provider

- [ ] D1. 新建 `anthropic/` 模块：mod.rs（AnthropicAdapter + new/with_retry/with_timeout/default_base_url）+ types.rs（MessagesRequest/MessagesResponse/ContentBlock/ToolUseBlock/ThinkingBlock）+ request.rs（Messages API 端点 + apply_headers 认证 + Bearer 端点支持 via quirks::wants_bearer_auth）+ discovery.rs
- [ ] D2. 新建 `anthropic/message_converter.rs`：convert_messages — system 分离 + content blocks 封装 + merge_consecutive_messages + trim_final_assistant_trailing_whitespace + tool_result 转换 + image content blocks + thinking content blocks（signature 回传）
- [ ] D3. 新建 `anthropic/stream.rs`：event_type 分发（message_start / content_block_start / content_block_delta / content_block_stop / message_delta / message_stop / error）+ thinking_signature 透传 + cache_creation_token_count 填充

---

## 阶段 E — Phase 5：Gemini 原生 Provider

- [ ] E1. 新建 `gemini/` 模块：mod.rs（GeminiAdapter + new/with_retry/with_timeout/default_base_url）+ types.rs（GenerateContentRequest/GeminiStreamResponse/Candidate/Part/FunctionCall/UsageMetadata）+ request.rs（streamGenerateContent 端点 + x-goog-api-key 认证 + alt=sse）+ discovery.rs
- [ ] E2. 新建 `gemini/message_converter.rs`：convert_messages — system→systemInstruction + role 映射（assistant→model）+ parts 封装 + tool_result→functionResponse + image parts + thoughtSignature 回传
- [ ] E3. 新建 `gemini/stream.rs`：candidates/parts 遍历（text / thought / functionCall）+ ToolCallAccumulator 拼装 + usageMetadata 映射（含 cachedContentTokenCount）+ safetyRatings 透传到 provider_metadata

---

## 阶段 F — Phase 6：Provider 工厂 + 配置 + 追踪诊断

- [ ] F1. `lib.rs` 更新 `create_provider_adapter` 工厂：ProviderKind::Anthropic → AnthropicAdapter，ProviderKind::Gemini → GeminiAdapter；`ProviderKind` 新增 `use_native_api() → bool`；确认前端无需修改
- [ ] F2. 新建 `trace.rs`（请求/响应追踪，debug 模式输出完整信息，API Key 脱敏）+ `diagnostics.rs`（敏感信息输出级别控制）

---

## 阶段 G — 核心测试

- [ ] G1. UnifiedResponse 转换测试：text_and_reasoning_in_same_chunk + multiple_tool_calls_split + minimax_reasoning_details_fallback + qwen_reasoning_field_fallback + deepseek_cache_hit_tokens + thinking_signature_passthrough + cache_creation_token_count
- [ ] G2. 流处理测试：tool_call_accumulator_merge + inline_think_extraction + stream_timeout_controller（TTFT + Idle）+ stream_stats_counters
- [ ] G3. 错误/重试测试：classify_http_error_all_codes + parse_retry_after_secs_formats + retry_config_exponential_backoff

---

## 阶段 H — 验收

### H1. 编译验证

- [ ] `cargo check` 通过
- [ ] `cargo clippy` 无新增 warning
- [ ] `cargo test` 通过

**依赖**：A1~A4, B1~B3, C1~C2, D1~D3, E1~E3, F1~F2, G1~G3

---

### H2. 功能验证

- [ ] OpenAI provider 流式对话正常（文本 + reasoning + tool_call）
- [ ] DeepSeek / MiniMax / Qwen reasoning 字段正确提取
- [ ] Anthropic Messages API 流式对话正常 + thinking signature 回传 + 连续同 role 合并
- [ ] Gemini streamGenerateContent 流式对话正常 + thought 分离 + functionCall 拼装
- [ ] Provider 工厂根据 ProviderKind 返回正确 adapter
- [ ] ChatDelta 接口不变，前端无需修改
- [ ] API Key 在日志中脱敏

**依赖**：H1
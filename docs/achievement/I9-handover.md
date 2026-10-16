# I9 交接 — Phase 1-6 + 核心测试 + H1 验收 已完成

## 状态

- ✅ A1-A4 全部完成，cargo check/test/clippy 通过（34 tests）
- ✅ B1-B3 全部完成，cargo check/test/clippy 通过（51 tests）
- ✅ C1-C2 全部完成，cargo check/test/clippy 通过（98 tests）
- ✅ C'1-C'5 全部完成，Reasoning Effort 端到端打通
- ✅ D1-D3 全部完成，Anthropic 原生 Provider 实现（127 tests）
- ✅ E1-E3 全部完成，Gemini 原生 Provider 实现
- ✅ F1-F2 全部完成，Provider 工厂 + trace/diagnostics
- ✅ G1-G3 全部完成，核心测试（174 tests）
- ✅ H1 编译验证通过（cargo check + clippy + test）
- ⬜ H2 功能验证待运行时测试

## 已完成的文件变更

### Phase 1 (A1-A4)
```
删除: src/openai.rs (617行单文件)
新增: src/types/{mod,unified,chat_delta,message,config,tool}.rs
新增: src/openai/{mod,types,error,stream,chat,message_converter,discovery}.rs
修改: src/lib.rs (加 types 模块)
```

### Phase 2 (B1-B3)
```
新增: src/stream/{mod,timeout,stats,inline_think,sse}.rs
新增: src/tool_call_accumulator.rs
修改: src/lib.rs (加 stream + tool_call_accumulator 模块)
修改: src/openai/chat.rs (eventsource-stream 替换手动 SSE 解析)
修改: Cargo.toml (加 eventsource-stream 0.2, bytes)
修改: workspace Cargo.toml (加 bytes)
```

### Phase 3 (C1-C2)
```
新增: src/client/{mod,quirks}.rs (URL 检测 + 特异行为函数 + 27 测试)
修改: src/lib.rs (加 client 模块)
修改: src/openai/types.rs (ChatResponseChunk.object + ChatChoice.message MiniMax 兼容)
修改: src/openai/stream.rs (delta.or(message) 回退 + 测试更新)
修改: src/openai/chat.rs (集成 quirks: SSE error + object 校验 + reasoning toggle/effort + tool_stream + 诊断性 data 附加)
修改: src/base.rs (ChatOptions 新增 reasoning_enabled / reasoning_effort 字段)
修改: src-tauri/src/cmd/chat.rs (ChatOptions 构造适配新字段)
```

### Phase 3.5 (C'1-C'5)
```
修改: src-tauri/crates/assistant-storage/src/agent_modes.rs (AgentModeRow 新增 reasoning_enabled/reasoning_effort + ALTER TABLE 迁移 + read/insert/update/reset 适配 + builtin-deep 默认 reasoning_enabled=true, reasoning_effort=high)
修改: src-tauri/src/cmd/chat.rs (从 mode 读取 reasoning_enabled/reasoning_effort 传入 ChatOptions)
修改: src-tauri/src/cmd/agent_mode.rs (save_agent_mode 解析 reasoning_enabled/reasoning_effort)
修改: src-tauri/crates/assistant-providers/src/client/quirks.rs (新增 normalize_generic_reasoning_effort)
修改: src-tauri/crates/assistant-providers/src/openai/chat.rs (else 分支调用 normalize_generic_reasoning_effort 归一化)
修改: src/stores/agentModeStore.ts (AgentMode interface 新增字段 + saveMode payload 同步)
修改: src/settings/AgentModeManager.tsx (表单新增 Reasoning Switch + Reasoning Effort Select)
```

### Phase 4 (D1-D3)
```
新增: src/anthropic/{mod,types,error,request,stream,message_converter,discovery}.rs
修改: src/lib.rs (加 anthropic 模块)
修改: src/base.rs (create_provider_adapter 工厂 + ChatMessage 扩展 Tool 角色/tool_calls/tool_call_id/name/is_error/thinking_signature + ToolCall 结构体 + message_role_from_str 支持 tool)
修改: src/openai/chat.rs (clippy: collapsible_else_if 修复)
修改: src/openai/message_converter.rs (适配 ChatMessage 新字段 + tool/tool_calls 序列化)
修改: src/openai/types.rs (ChatRequest.messages 改为 Vec<Value> 支持多态消息)
修改: src/anthropic/types.rs (MessagesRequest.system 改为 Option<Value> 支持 system content blocks)
修改: src/anthropic/message_converter.rs (补齐 tool_result/tool_use/thinking signature 回传/image base64)
修改: tests/base.rs (message_role_from_str("tool") → Tool)
修改: cmd/chat.rs (ChatMessage 构造适配新字段)
```

### Phase 5 (E1-E3)
```
新增: src/gemini/{mod,types,error,request,stream,message_converter,discovery}.rs
修改: src/lib.rs (加 gemini 模块)
```

### Phase 6 (F1-F2)
```
新增: src/trace.rs (API Key 脱敏 + 请求/响应追踪 + 5 测试)
新增: src/diagnostics.rs (DiagnosticLevel 三级控制 + 5 测试)
修改: src/lib.rs (加 trace + diagnostics 模块)
修改: src/base.rs (create_provider_adapter 工厂加 Gemini 分支 + ProviderKind::use_native_api() + RetryConfig/context_window 测试)
修改: src/gemini/request.rs (接入 trace::log_request/log_response_chunk/log_error)
```

## 关键架构

```
SSE chunk → EventSource → into_unified_responses() → Vec<UnifiedResponse>
                                                          ↓
              unified_responses_into_chat_deltas() → Vec<ChatDelta>  (前端不变)
```

- `EventSource`：eventsource-stream 0.2.3 自动解析 SSE event/data 行
- `UnifiedResponse`：text / reasoning_content / tool_call / finish_reason / usage 各自独立字段
- `into_unified_responses()`：reasoning 降级链 reasoning_content > reasoning > reasoning_details；多 tool_calls 拆分为多个 UnifiedResponse
- 桥接层：`types/chat_delta.rs` 将 UnifiedResponse 映射回现有 ChatDelta
- `StreamTimeoutController`：TTFT + Idle 两阶段超时控制
- `StreamStats`：结构化流统计与日志
- `InlineThinkParser`：`<think>` 标签跨 chunk 解析器
- `ToolCallAccumulator`：流式 tool_call 片段拼装 + JSON 截断修复
- `AnthropicAdapter`：Anthropic Messages API 原生格式支持
  - 认证：x-api-key / anthropic-version + Bearer 兼容端点（bigmodel.cn / api.z.ai / api.kimi.com/coding / api.moonshot.{cn,ai}/anthropic / inference-api.nousresearch.com / api.xiaomimimo.com/anthropic）
  - 消息转换：system 分离 + content blocks + 连续同 role 合并 + 尾部空白裁剪
  - SSE 流处理：event_type 分发（message_start/content_block_start/content_block_delta/message_delta/message_stop/error）
  - thinking_signature 透传 + cache_creation_token_count 填充
  - reasoning toggle：thinking.enabled/disabled + budget_tokens 自动计算
  - ChatMessage 扩展：Tool 角色 + tool_calls/tool_call_id/name/is_error/thinking_signature 字段
  - message_converter 补齐：tool_result 转换 + tool_use content blocks + thinking signature 回传 + image base64 解析
- `GeminiAdapter`：Gemini streamGenerateContent API 原生格式支持
  - 认证：x-goog-api-key header + alt=sse 查询参数
  - 消息转换：system→systemInstruction + role 映射 (assistant→model) + parts 封装 + tool_result→functionResponse + thinking_signature 透传 + 连续同 role 合并
  - SSE 流处理：candidates/parts 遍历 (text/thought/functionCall/executableCode/codeExecutionResult) + ToolCallAccumulator 拼装 + usageMetadata 映射 (含 cachedContentTokenCount) + safetyRatings/groundingMetadata 透传到 provider_metadata
  - reasoning toggle：thinkingConfig.includeThoughts + thinkingLevel + thinkingBudget
  - 请求构建：GenerateContentRequest typed struct → to_value() → patch（与 Anthropic 模式一致）
- `trace.rs`：请求/响应追踪 + API Key 脱敏 (URL key= 参数 + JSON apiKey/api_key 字段)
- `diagnostics.rs`：DiagnosticLevel 三级控制 (Normal/Verbose/Full) + 全局原子变量
- `ProviderKind::use_native_api()`：Anthropic/Gemini 返回 true，OpenAI/Custom 返回 false

## 下一步：H2 功能验证（运行时测试）

| 验证项 | 方式 |
|--------|------|
| OpenAI 流式对话 | 配置 OpenAI API Key，发送对话请求 |
| Anthropic Messages API | 配置 Anthropic API Key，验证 thinking signature 回传 |
| Gemini streamGenerateContent | 配置 Gemini API Key，验证 thought 分离 + functionCall 拼装 |
| Provider 工厂 | 单元测试已覆盖 (provider_kind_use_native_api) |
| API Key 脱敏 | 单元测试已覆盖 (trace 模块 5 测试) |

## 差距 G1-G19（补在 spec §7 + tasklist 末尾）

- G1-G4: ✅ into_unified_responses MiniMax 兼容/object 校验/SSE error/finish_reason 兜底 已完成
- G5: `arguments_is_snapshot` → Phase 7 候选（OpenAI Responses API）
- G6: `model_response_replay` / thinking_signature → ✅ D2/D3/E2 已实现 thinking_signature 透传（Anthropic + Gemini）
- G7-G11: ✅ 流基础设施 TimeoutController/Stats/InlineThink/Accumulator/eventsource 已完成
- G12-G15: ✅ quirks URL 检测/reasoning toggle/effort normalize/Bearer auth 已完成
- G16-G19: ✅ generic reasoning normalize / 端到端打通 / GLM toggle / generic toggle fallback 已完成

## 文档位置

- 规格：`docs/store/I9-provider-multi-api-spec.md`（§6 依赖 + §7 差距已更新）
- 任务：`docs/store/I9-provider-multi-api-tasklist.md`（A1-A4 + B1-B3 + D1-D3 已勾选 + 依赖/差距已追加）
- 参考：`references/OpenBitFun/src/crates/`
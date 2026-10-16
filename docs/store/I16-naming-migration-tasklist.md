# I16 — 命名迁移（后端 DTO + 前端 camelCase 统一）任务清单

> 规格：[I16-naming-migration-spec.md](./I16-naming-migration-spec.md)
> 前置：I15 ✅

---

## 实施约束

1. **禁止 `replace_all`**：Edit 工具的 `replace_all` 参数不可用
2. **单文件 Edit 不并行**：同一文件的多处 Edit 必须串行（前一次写入后下一次才能读最新内容）；不同文件的 Edit 可并行
3. **每步替换前必查影响范围**：`Grep pattern + glob` 确认匹配点数量，避免跨模块误伤（如 `base_url` 在 provider 和 settings 中都有，需按文件分批替换）
4. **Rust 侧不可全局 sed**：Rust 标识符替换须逐文件 Edit，因为 storage 层字段名保持 snake_case 不变，只有 cmd/protocol 层改
5. **PowerShell 批量替换仅限大文件**：仅当单个文件内同一模式出现 ≥5 处时，才使用 `Get-Content` + `-replace` + `Set-Content` 命令；否则逐个 Edit。每次命令只处理一个文件。替换前须 `Grep` 确认影响范围，替换后须 `Grep` 验证无遗漏/无误伤
6. **dto.rs 灵活分块**：单文件约 600-800 行可接受，用 region 注释分隔；若实施中超 1000 行，按 D14 拆为 `dto_chat.rs` / `dto_settings.rs` / `dto_orb.rs` / `dto_provider.rs`，mod.rs 统一 re-export

---

## 方案概要

**后端**：删除 `protocol/` 下 5 个原始文件（`chat_events.rs` / `chat_commands.rs` / `setting_commands.rs` / `setting_events.rs` / `orb_commands.rs`），全部 22 个 IPC DTO 集中到新建的 `dto.rs`。cmd/ 层构造点类型名改 Dto 后缀，字段名不变。

**前端**：chatTypes.ts 预补充业务类型（✅ 已完成），protocol.ts 字段改 camelCase + 类型名去 Row + 与 chatTypes.ts 重复类型删除，删除 protocolAdapter.ts，全链路 camelCase。

### 后端 protocol/ 目录变化

```
迁移前:                          迁移后:
protocol/                        protocol/
  mod.rs                           mod.rs    → pub mod dto; pub use dto::*;
  chat_events.rs        ─┐
  chat_commands.rs      ─┤
  setting_commands.rs   ─┼─→       dto.rs    → 全部 22 个 DTO
  setting_events.rs     ─┤
  orb_commands.rs       ─┘
```

### DTO 分两类

| 类别 | DTO | 来源 crate | 需 From impl |
|------|-----|-----------|-------------|
| **嵌套外部类型** | ConversationDto / MessageDto / ConversationDetailDto / TokenUsageDto / TokenUsageSummaryDto / RetryNotificationDto / ProviderErrorPayloadDto | storage / providers | ✅ 必须（Row/Wire 不能加 rename） |
| **直属字段** | ChatDeltaPayloadDto / ChatStreamStartPayloadDto / ConversationTerminatedPayloadDto / ConversationCompactFailedPayloadDto / ConversationUnlockedPayloadDto / ChatRegeneratePayloadDto / ProviderListDto / SettingProviderChangedPayloadDto / ShortcutWarningPayloadDto / AgentModeDto / ModelConfigDto / ProviderInputDto / ThemeChangedPayloadDto / MousePositionDto / ScreenSizeDto | protocol（原文件） | ✅ 统一提供（cmd/ 层构造后 .into() 或直接构造 Dto） |

---

## 阶段 A — 后端：新建 dto.rs + From impl

- [x] A1. 新建 `src-tauri/src/protocol/dto.rs`，创建全部 22 个 DTO struct，每个加 `Serialize + Deserialize + PartialEq` + `rename_all = "camelCase"` + `deny_unknown_fields`；Option 字段加 `#[serde(skip_serializing_if = "Option::is_none")]`；可默认字段加 `#[serde(default)]`。用 region 注释按功能域分隔：`// ── Chat DTOs ──` / `// ── Settings DTOs ──` / `// ── Orb DTOs ──` / `// ── Provider DTOs ──`
- [x] A2. 嵌套外部类型的 DTO（7 个）：ConversationDto←ConversationRow, MessageDto←MessageRow, ConversationDetailDto←ConversationRow+Vec<MessageRow>, TokenUsageDto←TokenUsage, TokenUsageSummaryDto←TokenUsageSummary, RetryNotificationDto←RetryNotification, ProviderErrorPayloadDto←ProviderErrorPayload。ChatDeltaPayloadDto 的嵌套字段类型改为引用 DTO（`usage: Option<TokenUsageDto>`, `delta_retry: Option<RetryNotificationDto>`, `error: Option<ProviderErrorPayloadDto>`）；ProviderListDto 嵌套 `models: Vec<ModelConfigDto>`
- [x] A3. 直属字段的 DTO（15 个）：ChatDeltaPayloadDto / ChatStreamStartPayloadDto / ConversationTerminatedPayloadDto / ConversationCompactFailedPayloadDto / ConversationUnlockedPayloadDto / ChatRegeneratePayloadDto / ProviderListDto / SettingProviderChangedPayloadDto / ShortcutWarningPayloadDto / AgentModeDto / ModelConfigDto / ProviderInputDto / ThemeChangedPayloadDto / MousePositionDto / ScreenSizeDto。字段与原 struct 相同，类型名改 Dto 后缀
- [x] A4. 为嵌套外部类型的 DTO 实现 `From<原始类型>`，字段逐一透传
- [x] A5. ChatDeltaPayloadDto 派生 `Default`（原 ChatDeltaPayload 有 `#[derive(Default)]`，DTO 须同步，确保 `..Default::default()` 用法不破坏）
- [x] A6. `protocol/mod.rs` 改为 `pub mod dto; pub use dto::*;`
- [x] **A7. ✅ 验证**：cargo check 通过
- [x] A8. 在 dto.rs 底部加 `#[cfg(test)] mod tests`，为每个 DTO 写 round-trip 测试：构造实例 → serialize to JSON → deserialize back → assert_eq，验证 `rename_all = "camelCase"` + `deny_unknown_fields` 组合不破坏 round-trip

---

## 阶段 B — 后端：cmd/ 层 import + 构造点改 Dto 类型名

> cmd/ 层改动模式：类型名加 Dto 后缀，Rust 字段名不变（仍 snake_case），Serde 自动输出 camelCase。

- [x] B1. `cmd/conversation.rs`：import 改为 `use crate::protocol::{ConversationDto, MessageDto, ConversationDetailDto, ChatDeltaPayloadDto, ChatStreamStartPayloadDto, ChatRegeneratePayloadDto, ConversationTerminatedPayloadDto, ConversationCompactFailedPayloadDto, ConversationUnlockedPayloadDto};`；`list_conversations` 返回 `Vec<ConversationDto>`（`rows.into_iter().map(Into::into).collect()`）；`get_conversation_paginated` 返回 `Option<ConversationDetailDto>`（手动构造：`ConversationDetailDto { conversation: conv.into(), messages: messages.into_iter().map(Into::into).collect(), total_count }`）；`search_conversations` 返回 `Vec<ConversationDto>`；`get_conversation` 返回类型改 DTO；所有 payload 构造点类型名加 Dto 后缀
- [x] B2. `cmd/chat.rs`：import 改为 DTO 类型；所有 `ChatDeltaPayload {` → `ChatDeltaPayloadDto {`；`ChatStreamStartPayload {` → `ChatStreamStartPayloadDto {`；`ConversationCompactFailedPayload {` → `ConversationCompactFailedPayloadDto {`
- [x] B3. `cmd/agent_mode.rs`：import 改为 `AgentModeDto, AgentModeInputDto`；`row_to_wire` 改为 `row_to_dto` 返回 `AgentModeDto`；`save_agent_mode` 参数改 `AgentModeInputDto`
- [x] B4. `cmd/provider.rs`：import 改为 `ModelConfigDto, ProviderInputDto, ProviderListDto, SettingProviderChangedPayloadDto`；构造点类型名改 Dto 后缀
- [x] B5. `cmd/orb.rs`：import 改为 `MousePositionDto, ScreenSizeDto`；`MousePosition {` → `MousePositionDto {`；`ScreenSize {` → `ScreenSizeDto {`；`ScreenRect` 内联到 `ScreenSizeDto`
- [x] B6. `cmd/config.rs`：import 改为 `ThemeChangedPayloadDto`；构造点类型名改 Dto 后缀
- [x] B7. `setup.rs`：import 改为 `ShortcutWarningPayloadDto`；构造点类型名改 Dto 后缀
- [x] **B8. ✅ 验证**：cargo check 通过

---

## 阶段 C — 后端：删除原始 protocol 文件

- [x] C1. 删除 `protocol/chat_events.rs`
- [x] C2. 删除 `protocol/chat_commands.rs`
- [x] C3. 删除 `protocol/setting_commands.rs`
- [x] C4. 删除 `protocol/setting_events.rs`
- [x] C5. 删除 `protocol/orb_commands.rs`
- [x] **C6. ✅ 验证**：cargo check + cargo test 通过

---

## 阶段 D — 前端：protocol.ts 字段改 camelCase + 类型名去 Row + 重复类型删除

> chatTypes.ts 已预补充 RetryNotification / ConversationDetail / TokenUsageSummary / 6 个 event payload（✅ 已完成）。protocol.ts 改名后，与 chatTypes.ts 重复的类型定义直接删除，所有消费方统一从 chatTypes.ts 导入，消除同名类型冲突。protocol.ts 仅保留 chatTypes.ts 中没有的 IPC 专属类型（如 ProviderErrorCode）。

- [x] D1. `protocol.ts`：`MessageRow` → `Message`（字段 camelCase）；`ConversationRow` → `Conversation`（字段 camelCase）；`ConversationDetailRow` → `ConversationDetail`（totalCount）
- [x] D2. `protocol.ts`：`TokenUsageRow` → `TokenUsage`（字段 camelCase）；`RetryNotificationRow` → `RetryNotification`（maxAttempts, delayMs）；`TokenUsageSummaryRow` → `TokenUsageSummary`（字段 camelCase）
- [x] D3. `protocol.ts`：event payload 字段改 camelCase
- [x] D4. `protocol.ts`：`ProviderErrorPayloadRow` → `ProviderErrorPayload`（retryAfterMs）
- [x] D5. `protocol.ts`：删除与 chatTypes.ts 重复的类型定义，仅保留 IPC 专属类型
- [x] **D6. ✅ 验证**：tsc --noEmit 已执行，错误清单记录于 `docs/store/I16-tsc-errors.txt` 和 `I16-tsc-errors2.txt`
- [x] D7. 记录 `tsc --noEmit` 的全部错误文件清单，作为 E→I 阶段的修复检查表，确保不遗漏

---

## 阶段 E — 前端：删除 protocolAdapter.ts + service 层简化

- [x] E1. 删除 `protocolAdapter.ts` 整个文件
- [x] E2. `chatService.ts`：删除 `invokeList` / `invokeSingle` / `rowToConversation` / `rowToConversationDetail` / `rowToTokenUsageSummary` 引用；删除局部定义的 `ConversationDetail` / `TokenUsageSummary`（改为从 chatTypes.ts 导入）；`listConversations` 改为 `invoke<Conversation[]>("list_conversations", ...).then(r => r || [])`；`getConversationPaginated` 改为 `invoke<ConversationDetail | null>(...).then(r => r ?? null)`；`getConversationTokenUsage` 改为 `invoke<TokenUsageSummary | null>(...).then(r => r ?? null)`
- [x] E3. `historyService.ts`：删除 `ConversationRow` / `rowToConversation` 引用；`searchConversations` 改为 `invoke<Conversation[]>("search_conversations", ...).then(r => r || [])`
- [x] E4. `chatOrchestrator.ts`：删除 `RetryNotificationRow` / `rowToTokenUsage` 引用；`RetryNotificationRow` → `RetryNotification`（从 chatTypes.ts 导入）；usage 直接使用 camelCase 字段
- [x] **E5. ✅ 验证**：chatService / historyService / chatOrchestrator 编译通过

---

## 阶段 F — 前端：chatDeltaEvents.ts + chatStore.ts

- [x] F1. `chatDeltaEvents.ts`：`StreamStartIds` 接口字段改 camelCase（conversation_id→conversationId, attempt_id→attemptId, message_id→messageId）；所有 `payload.conversation_id` → `payload.conversationId` 等；`payload.delta_text` → `payload.deltaText`；同理 delta_reasoning / delta_tool_call_id / delta_tool_call_name / delta_tool_call_arguments / finish_reason / delta_retry / attempt_id / message_id
- [x] F2. ~~`chatErrors.ts`：`ChatError.retry_after_ms` → `retryAfterMs`~~ ✅ 已完成（chatErrors.ts / chatStore.ts / ErrorBlock.tsx / chatOrchestrator.ts / chatOrchestrator.test.ts 全链路已改）
- [x] F3. `chatStore.ts`：`RetryNotificationRow` → `RetryNotification`；store 赋值中 snake_case payload 字段改 camelCase；`context_window` → `contextWindow`
- [x] **F4. ✅ 验证**：tsc --noEmit 通过

---

## 阶段 G — 前端：UI 组件 snake_case → camelCase

> ⚠️ 高风险跨模块字段：`default_model`（provider + agentMode + chat 共用）、`base_url`（provider 专用）、`primary_color`/`secondary_color`（mascot + orb + chat 共用）。每个字段替换前须 `Grep` 确认全项目匹配点，按文件逐个 Edit，禁止全局 sed。

- [x] G1. `MessageBubble.tsx`：`RetryNotificationRow` → `RetryNotification`；retry 属性改 camelCase（max_attempts→maxAttempts, delay_ms→delayMs） — ✅ 代码已无 RetryNotificationRow/max_attempts/delay_ms 残留
- [x] G2. `MessageList.tsx`：`RetryNotificationRow` → `RetryNotification` — ✅ 代码已无 RetryNotificationRow 残留
- [x] G3. ~~`ErrorBlock.tsx`：`retry_after_ms` → `retryAfterMs`~~ ✅ 已完成
- [x] G4. `ProviderManager.tsx`：`base_url` → `baseUrl`，`api_key_masked` → `apiKeyMasked`，`default_model` → `defaultModel` — ✅ 代码已全部 camelCase
- [x] G5. `ConnectDialog.tsx`：`base_url` → `baseUrl`，`api_key` → `apiKey`，`api_key_changed` → `apiKeyChanged`，`default_model` → `defaultModel` — ✅ 本次已改
- [x] G6. `AgentModeSettings.tsx`：`max_tokens` → `maxTokens`，`reasoning_enabled` → `reasoningEnabled`，`reasoning_effort` → `reasoningEffort`，`presentation_rank` → `presentationRank`，`is_builtin` → `isBuiltin` — ✅ 本次已改（presentation_rank/is_builtin 在该文件中无直接引用，types.ts 已改）
- [x] G7. `settings/agentMode/types.ts`：同 G6 字段改 camelCase — ✅ 代码已全部 camelCase
- [x] G8. `settings/provider/types.ts`：`owned_by` → `ownedBy`，`context_window` → `contextWindow`，`base_url` → `baseUrl`，`api_key_masked` → `apiKeyMasked`，`default_model` → `defaultModel`，`api_key_changed` → `apiKeyChanged`，`updated_at` → `updatedAt` — ✅ 代码已全部 camelCase
- [x] G9. `MascotColorGroupRender.tsx`：`primary_color` → `primaryColor`，`secondary_color` → `secondaryColor` — ✅ 代码已全部 camelCase
- [x] G10. `AssetPickerField.tsx`：`installed_path` → `installedPath` — ✅ 本次已改（含 settingService.ts InstallAssetResult）
- [x] G11. `MessageInput.tsx`：`primary_color` → `primaryColor`，`secondary_color` → `secondaryColor` — ✅ 代码已全部 camelCase
- [x] G12. `CompactBanner.tsx`：`default_model` → `defaultModel` — ✅ 代码已全部 camelCase
- [x] G13. `ChatView.tsx`：`default_model` → `defaultModel` — ✅ 代码已全部 camelCase
- [x] G14. `PinToggle.tsx`：`stay_on_top` → `stayOnTop` — ✅ 代码已全部 camelCase
- [x] G15. `CommandPalette.tsx`：`command_palette` → `commandPalette` — ✅ 本次已改
- [x] **G16. ✅ 验证**：tsc --noEmit 通过

---

## 阶段 H — 前端：orb 模块 snake_case → camelCase

- [x] H1. `orbService.ts`：`MousePositionRow` → `MousePosition`，`ScreenSizeRow` → `ScreenSize`，`client_x` → `clientX`，`client_y` → `clientY` — ✅ 本次已改
- [x] H2. `useOrbConfSync.ts`：`opacity_percent` → `opacityPercent`，`scale_percent` → `scalePercent`，`hide_on_pointer_hover` → `hideOnPointerHover`，`hover_delay_ms` → `hoverDelayMs`，`idle_opacity_percent` → `idleOpacityPercent`，`click_through` → `clickThrough` — ✅ 代码已全部 camelCase
- [x] H3. `OrbExpand.ts`：`click_through` → `clickThrough` — ✅ 代码已全部 camelCase
- [x] H4. `OrbDrag.tsx`：`click_through` → `clickThrough` — ✅ 代码已全部 camelCase
- [x] H5. `OrbBall.tsx`：`primary_color` → `primaryColor`，`secondary_color` → `secondaryColor` — ✅ 代码已全部 camelCase
- [x] H6. `OrbCoord.ts`：`client_x` → `clientX`，`client_y` → `clientY` — ✅ 本次已改
- [x] **H7. ✅ 验证**：tsc --noEmit 通过

---

## 阶段 I — 前端：settings 核心层适配

> ⚠️ **D12 约束修订**：原 D12 约束 fieldDefinitions.ts 的 key 保持 snake_case，但实际代码中 `confTypes.ts` 接口已全部 camelCase，`patchToSnakeCase()` 会自动将 camelCase key 转为 snake_case 发给后端 `amend()`。若 field key 保持 snake_case，`buildFieldPatch` 构建的 patch 对象 key 为 snake_case，`optimisticConfUpdate` 会用 snake_case key 更新 camelCase 的 store 对象，导致 optimistic update 失效（设置错误 key）。因此 **fieldDefinitions.ts 的 key 必须改为 camelCase**，与 confTypes.ts 对齐。`patchToSnakeCase()` 负责在 IPC 调用时自动转换。
>
> D13 约束同步修订：`settingsDispatcher.ts` 中 patch 赋值 key 也改为 camelCase（与 confTypes 对齐），`patchToSnakeCase()` 在 `patchConf()` 调用时自动转为 snake_case 发后端。

- [x] I1. `fieldDefinitions.ts`：field key 改 camelCase（stay_on_top→stayOnTop, launch_at_login→launchAtLogin, close_to_tray→closeToTray, opacity_percent→opacityPercent, idle_opacity_percent→idleOpacityPercent, scale_percent→scalePercent, click_through→clickThrough, hide_on_pointer_hover→hideOnPointerHover, hover_delay_ms→hoverDelayMs, always_on_top→alwaysOnTop, primary_color→primaryColor, auto_compact→autoCompact, compact_threshold_percent→compactThresholdPercent, compact_summary_prompt→compactSummaryPrompt, compact_summary_max_tokens→compactSummaryMaxTokens）；label/saveMessage 中 i18n key 保持不变（i18n key 是字符串常量，不算残留）
- [x] I2. `settingsDispatcher.ts`：读取侧改 camelCase — ✅ 代码已全部 camelCase（conf.riveSrc / conf.gifSrc 等）；patch 赋值 key 也已 camelCase（buildMascotPatch 返回 camelCase key），patchToSnakeCase 自动转换发后端
- [x] I3. `confTypes.ts`：相关配置接口字段改 camelCase — ✅ 代码已全部 camelCase
- [x] I4. `shortcutTypes.ts`：相关字段改 camelCase — ✅ 本次已改（command_palette→commandPalette）
- [x] I5. `agentModeDefinitions.ts` / `agentModeRegistry.ts`：相关字段改 camelCase — ✅ 本次已改（max_tokens→maxTokens, reasoning_enabled→reasoningEnabled, reasoning_effort→reasoningEffort）
- [x] I6. mascot renderers（`rive/index.ts` / `svg/index.ts` / `gif/index.ts`）：`rive_src` → `riveSrc`，`gif_src` → `gifSrc`，`svg_animated` → `svgAnimated`，`gif_border_radius` → `gifBorderRadius`，`rive_idle_pose_rotation` → `riveIdlePoseRotation`，`primary_color` → `primaryColor`，`secondary_color` → `secondaryColor` — ✅ 本次已改（riveSrc/riveIdlePoseRotation/svgAnimated/gifSrc/gifBorderRadius/primaryColor）
- [x] **I7. ✅ 验证**：tsc --noEmit 通过

---

## 阶段 J — 前端：测试适配

- [x] J1. `protocolContract.test.ts`：删除 snake_case 契约断言和 rowTo* 适配映射测试；改为断言 chatTypes.ts 类型使用 camelCase；mock 数据从 snake_case 改 camelCase；修复 `reasoning: null` → `undefined`（类型 `reasoning?: string` 不接受 null）；`toolCallsJson` → `toolCalls`（与 Message 接口对齐）
- [x] J2. `chatOrchestrator.test.ts`：mock 数据改 camelCase（conversation_id→conversationId, attempt_id→attemptId, message_id→messageId, delta_reasoning→deltaReasoning, finish_reason→finishReason, prompt_tokens→promptTokens, completion_tokens→completionTokens, total_tokens→totalTokens, cache_read_tokens→cacheReadTokens, cache_write_tokens→cacheWriteTokens, context_window_tokens→contextWindowTokens, default_model→defaultModel）
- [x] J3. `chatService.test.ts`：mock 数据改 camelCase（total_count→totalCount, total_tokens→totalTokens, context_window_tokens→contextWindowTokens）
- [x] J3b. `streamCoalescer.test.ts`：mkDelta 默认值和 overrides 改 camelCase（conversation_id→conversationId, attempt_id→attemptId, message_id→messageId）
- [x] **J4. ✅ 验证**：vitest 全部通过（5 files, 88 tests passed）

---

## 阶段 K — 集成验证 + 收尾

- [x] K1. cargo check + cargo test + clippy 通过
- [x] K2. tsc --noEmit + eslint 通过
- [x] K3. vitest 全部通过
- [x] K4. **前端 snake_case 残留扫描**：对 `src/` 下 `*.{ts,tsx}` 逐项 Grep 以下模式，命中数须为 0（排除项除外）：
  - `conversation_id` / `attempt_id` / `message_id` / `parent_id` / `provider_id` / `model_id` / `title_source` / `system_prompt` / `sort_order` / `created_at` / `updated_at` / `is_pinned` / `is_full` / `is_builtin`
  - `delta_text` / `delta_reasoning` / `delta_tool_call_id` / `delta_tool_call_name` / `delta_tool_call_arguments` / `delta_retry` / `finish_reason` / `deleted_message_id`
  - `prompt_tokens` / `completion_tokens` / `total_tokens` / `cache_read_tokens` / `cache_write_tokens` / `context_window_tokens`
  - `max_attempts` / `delay_ms` / `retry_after_ms` / `total_count` / `tool_calls_json`
  - `base_url` / `api_key` / `api_key_masked` / `api_key_changed` / `default_model` / `owned_by` / `context_window` / `updated_at`
  - `max_tokens` / `reasoning_enabled` / `reasoning_effort` / `presentation_rank`
  - `primary_color` / `secondary_color` / `installed_path` / `stay_on_top` / `command_palette`
  - `click_through` / `opacity_percent` / `scale_percent` / `hide_on_pointer_hover` / `hover_delay_ms` / `idle_opacity_percent`
  - `client_x` / `client_y`
  - `rive_src` / `gif_src` / `svg_animated` / `gif_border_radius` / `rive_idle_pose_rotation`
  - **排除**：~~`fieldDefinitions.ts` 中 `key:` 值保持 snake_case（D12）~~ — D12 已修订，field key 改 camelCase，不再排除
  - **排除**：~~`settingsDispatcher.ts` 中 patch 赋值 key 保持 snake_case（D13）~~ — D13 已修订，patch key 改 camelCase，不再排除
  - **排除**：字符串字面量中后端 invoke command 名（如 `"list_conversations"`）不算残留
  - **排除**：i18n key / CSS class / 文件路径中的 snake_case 不算残留
- [x] K5. **前端 Row 后缀残留扫描**：Grep `MessageRow|ConversationRow|ConversationDetailRow|TokenUsageRow|RetryNotificationRow|TokenUsageSummaryRow|ProviderErrorPayloadRow|MousePositionRow|ScreenSizeRow` 在 `src/` 下命中数须为 0
- [x] K6. **后端 DTO 契约扫描**：Grep `rename_all = "snake_case"` 在 `src-tauri/src/protocol/` 下命中数须为 0（原始文件已删除，dto.rs 全部 camelCase）
- [x] K7. **后端原始文件残留扫描**：确认 `protocol/` 下仅剩 `mod.rs` + `dto.rs`（或 dto 子模块），无 `chat_events.rs` / `chat_commands.rs` / `setting_commands.rs` / `setting_events.rs` / `orb_commands.rs`
- [x] K8. 编写 `scripts/verify-naming-migration.ps1` 自动化残留扫描脚本（包含 K4-K7 所有 Grep 模式和排除规则），可重复执行
- [ ] K9. 手动 E2E：发消息 → 流式 delta 正常 → 对话列表正常 → 设置页正常 → Orb 正常
- [ ] K10. 手动 E2E：Provider 管理 → Agent 模式 → 快捷键 → 主题切换

---

## 任务依赖图

```
A → B → C → D → E → F → G ─┐
                              H ─┤→ J → K
                              I ─┘
```

A-C 后端独立，D-I 前端独立，J 测试适配，K 最终验收。G/H/I 可并行。

**已完成前置**：chatTypes.ts 预补充（RetryNotification / ConversationDetail / TokenUsageSummary / 6 个 event payload）✅；chatErrors.ts retryAfterMs 全链路 ✅

---

## 批量替换策略

| 场景 | 方法 | 条件 |
|------|------|------|
| 单文件内少量替换（<5 处同模式） | 逐个 Edit（old_string 精确匹配） | 默认方式 |
| 单文件内大量替换（≥5 处同模式） | PowerShell `Get-Content` + `-replace` + `Set-Content`，限定 `-Path` 到单个文件 | 仅处理一个文件，替换前 `Grep -c` 确认数量，替换后 `Grep -c` 验证数量一致 |
| 同一分词跨多文件但有禁区 | 先 `Grep` 列出匹配文件，逐文件判断是否应改，按文件 Edit | 如 `base_url` 在 provider/types.ts 该改，在 Rust storage 不该改 |
| 类型名重命名 | 逐文件 Edit（Rust 标识符不可 sed） | `RetryNotificationRow` → `RetryNotification` |
| Rust 标识符替换 | 逐文件 Edit（storage 层保持 snake_case） | 仅 cmd/protocol 层改 |

每次替换后立即 `Grep` 验证：1) 目标文件已全部替换 2) 禁区文件未被误伤 3) 无残存旧名称
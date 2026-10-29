# I16 命名迁移规格 — 后端 DTO + 前端 camelCase 统一

## 1. 目标

消除前端 snake_case 字段泄漏到 UI 层的问题。后端 IPC 边界统一输出 camelCase，前端全链路使用 camelCase，删除前端 `rowTo*` 适配层。

## 2. 方案

### 2.1 核心思路

后端在 IPC 边界引入 DTO 层，通过 `#[serde(rename_all = "camelCase")]` 让 Tauri command 返回值和 event payload 序列化为 camelCase。前端直接消费 camelCase，无需适配层做字段转换。

### 2.2 分层架构

```
后端：
┌─────────────────────────────────────────────┐
│  cmd/ 层 — 返回 DTO，emit DTO               │
├─────────────────────────────────────────────┤
│  protocol/dto.rs — 全部 IPC DTO 集中         │
│  #[serde(rename_all = "camelCase")]         │
├─────────────────────────────────────────────┤
│  storage/ + providers/ — 内部层，snake_case  │
│  不变，仅通过 From trait → DTO 输出          │
└─────────────────────────────────────────────┘

前端：
┌─────────────────────────────────────────────┐
│  业务层 (Store / Component / Hook)           │
│  camelCase: message.toolCalls               │
├─────────────────────────────────────────────┤
│  protocol.ts + chatTypes.ts — 全部 camelCase│
├─────────────────────────────────────────────┤
│  Tauri 边界 (invoke / listen) — camelCase   │
└─────────────────────────────────────────────┘
```

### 2.3 设计决定

| ID | 决定 | 理由 |
|----|------|------|
| D1 | storage 层保持 snake_case | 磁盘格式不变，零迁移风险 |
| D2 | IPC 边界输出 camelCase | JS/TS 生态原生 camelCase，UI 层零适配 |
| D3 | event payload 也走独立 DTO | 与 command DTO 模式统一；原 struct 保持 snake_case 不动，emit 时转 DTO 输出 |
| D4 | storage Row struct 不加 rename | 加 rename 会破坏磁盘反序列化 |
| D5 | 前端删除整个 `protocolAdapter.ts` | 后端已出 camelCase，所有 rowTo* 映射变为恒等；invokeList/invokeSingle 逻辑太简单，内联到调用处 |
| D6 | 前端 protocol.ts 字段名改为 camelCase | 与后端 IPC 输出对齐 |
| D7 | chatTypes.ts 为前端业务类型单一真相源 | 已是 camelCase；迁移前预补充 protocol.ts 中缺失的业务类型（RetryNotification / ConversationDetail / TokenUsageSummary / 6 个 event payload），消除迁移后同名类型冲突 |
| D8 | DTO 加 `deny_unknown_fields` | 前端反序列化时多余/缺少字段立即报错，前后端契约保障 |
| D9 | Option 字段加 `skip_serializing_if` | 省略 None 字段，高频 event payload 传输体积减少约 67% |
| D10 | 核心 DTO 派生 Serialize + Deserialize | 支持 round-trip 测试；deny_unknown_fields 依赖 Deserialize |
| D11 | 新增/可选字段加 `#[serde(default)]` | 前向兼容：后端新增字段时旧版前端不崩溃 |
| D12 | Config patch field key 改 camelCase | ~~原约束：fieldDefinitions.ts 的 key 保持 snake_case~~ → 修订：field key 改 camelCase，与 confTypes.ts 对齐；`patchToSnakeCase()` 在 IPC 调用时自动转为 snake_case 发后端 amend()。原约束导致 `optimisticConfUpdate` 用 snake_case key 更新 camelCase store 对象，optimistic update 失效 |
| D13 | settingsDispatcher.ts patch 赋值 key 改 camelCase | ~~原约束：patch 赋值 key 保持 snake_case~~ → 修订：与 D12 联动，patch key 改 camelCase，`patchToSnakeCase()` 自动转换 |
| D14 | dto.rs 过长时可按功能域拆为子模块 | 单文件约 600-800 行可接受，用 region 注释分隔；若超 1000 行则拆为 `dto_chat.rs` / `dto_settings.rs` / `dto_orb.rs` / `dto_provider.rs`，mod.rs 统一 re-export |

## 3. 后端要求

### 3.1 新建 dto.rs

在 `src-tauri/src/protocol/` 下新建 `dto.rs`，**所有 IPC DTO 集中于此文件**（chat、settings、orb 不拆分），与参考项目 OpenBitFun 的 `api/dto.rs` 模式一致。DTO 都是同一种东西（IPC 边界翻译），按功能域拆分反而增加认知成本。

**文件组织**：用 region 注释按功能域分隔，便于 800+ 行文件中快速导航：

```rust
// ── Chat DTOs ──────────────────────────────
// ── Settings DTOs ───────────────────────────
// ── Orb DTOs ────────────────────────────────
// ── Provider DTOs ───────────────────────────
```

若实施中发现 dto.rs 超 1000 行，按 D14 拆为子模块（`dto_chat.rs` / `dto_settings.rs` / `dto_orb.rs` / `dto_provider.rs`），`mod.rs` 统一 `pub mod` + `pub use`。

**DTO 注解规范**：

- 所有 DTO：`Serialize + Deserialize + PartialEq` + `rename_all = "camelCase"` + `deny_unknown_fields`
- 所有 `Option<T>` 字段：`#[serde(skip_serializing_if = "Option::is_none")]`
- 可默认/新增字段：`#[serde(default)]`
- 每个 DTO 提供 `From<原始类型>` impl，字段逐一透传

**需要创建的 DTO**：

| DTO | 来源 | 说明 |
|-----|------|------|
| ConversationDto | ConversationRow | conversation 列表/详情返回 |
| MessageDto | MessageRow | message 列表返回 |
| ConversationDetailDto | ConversationRow + Vec<MessageRow> | 替代 ConversationDetailResponse |
| TokenUsageDto | TokenUsage (providers) | delta event 中 usage 字段 |
| TokenUsageSummaryDto | TokenUsageSummary (storage) | token 用量查询返回 |
| RetryNotificationDto | RetryNotification (providers) | delta event 中 deltaRetry 字段 |
| ProviderErrorPayloadDto | ProviderErrorPayload (providers) | delta event 中 error 字段 |
| ChatDeltaPayloadDto | ChatDeltaPayload | 流式 delta event |
| ChatStreamStartPayloadDto | ChatStreamStartPayload | 流式开始 event |
| ConversationTerminatedPayloadDto | ConversationTerminatedPayload | 对话终止 event |
| ConversationCompactFailedPayloadDto | ConversationCompactFailedPayload | 压缩失败 event |
| ConversationUnlockedPayloadDto | ConversationUnlockedPayload | 解锁 event |
| ChatRegeneratePayloadDto | ChatRegeneratePayload | 重新生成 event |
| ProviderListDto | ProviderListRow | provider 列表 event |
| SettingProviderChangedPayloadDto | SettingProviderChangedPayload | provider 变更 event |
| ShortcutWarningPayloadDto | ShortcutWarningPayload | 快捷键冲突 event |
| AgentModeDto | AgentModeWire | agent mode 设置 |
| ModelConfigDto | ModelConfigWire | model 配置 |
| ProviderInputDto | ProviderInputWire | provider 输入 |
| ThemeChangedPayloadDto | ThemeChangedPayload | 主题变更 event |
| MousePositionDto | MousePosition | orb 鼠标位置 |
| ScreenSizeDto | ScreenSize | orb 屏幕尺寸 |

### 3.2 command 返回类型改 DTO

所有 Tauri command 的返回类型从原始 Row/内部类型改为对应 DTO，返回时通过 `into()` / `into_iter().map(Into::into).collect()` 转换。

### 3.3 删除原始 protocol 文件

原始 5 个 protocol 文件（`chat_events.rs` / `chat_commands.rs` / `setting_commands.rs` / `setting_events.rs` / `orb_commands.rs`）中的 struct 全部被 dto.rs 替代，**直接删除这 5 个文件**。cmd/ 层构造点类型名加 Dto 后缀（如 `ChatDeltaPayload {` → `ChatDeltaPayloadDto {`），Rust 字段名不变。

### 3.4 不受影响

- `assistant-storage/` 全部：磁盘层保持 snake_case
- `assistant-providers/` 内部逻辑：原始类型内部使用不变
- 配置文件磁盘格式不变

## 4. 前端要求

### 4.1 protocol.ts — 字段名全部改 camelCase，类型名去 Row 后缀

所有 snake_case 字段名改为 camelCase。所有 `*Row` 类型名删除 Row 后缀。

**类型归属策略（D7）**：迁移前先将 protocol.ts 中 chatTypes.ts 缺失的业务类型预补充到 chatTypes.ts（已完成：RetryNotification / ConversationDetail / TokenUsageSummary / ChatDeltaPayload / ChatStreamStartPayload / ConversationTerminatedPayload / ConversationCompactFailedPayload / ConversationUnlockedPayload / ChatRegeneratePayload）。迁移后 protocol.ts 中这些重复类型定义删除，所有消费方统一从 chatTypes.ts 导入，消除同名类型冲突。protocol.ts 仅保留 chatTypes.ts 中确实没有的 IPC 专属类型（如 `ProviderErrorCode`）。

chatService.ts 中局部定义的 `ConversationDetail` / `TokenUsageSummary` 同步删除，改为从 chatTypes.ts 导入。

### 4.2 删除整个 protocolAdapter.ts

该文件所有内容删除：`rowTo*` 映射函数、`messageToRow`/`conversationToRow` 反向映射、`parseToolCallsJson`、`invokeList`/`invokeSingle`。后端已出 camelCase，映射变为恒等；`invokeList`/`invokeSingle` 的 null-safety 逻辑（`r || []` / `r ?? null`）太简单，直接内联到各 service 调用处。

### 4.3 service 层简化 invoke 调用

所有 service 中 invoke 调用删除 rowTo* 适配参数，null-safety 逻辑内联（如 `.then(r => r || [])`）。

### 4.4 chatDeltaEvents.ts — event payload 字段名改 camelCase

所有 `payload.conversation_id` → `payload.conversationId`，`payload.delta_text` → `payload.deltaText` 等。StreamStartIds 接口字段同步改。

### 4.5 chatErrors.ts — ChatError 修正遗漏的 snake_case ✅ 已完成

`retry_after_ms` → `retryAfterMs`。同步修改 `toChatError()` 构造、`isChatError()` 判断、所有 `error.retry_after_ms` 引用点（chatStore.ts / ErrorBlock.tsx / chatOrchestrator.ts / chatOrchestrator.test.ts）。

### 4.6 UI 组件 — RetryNotificationRow 改名 + 属性改 camelCase

`RetryNotificationRow` → `RetryNotification`，`max_attempts` → `maxAttempts`，`delay_ms` → `delayMs`。

### 4.7 chatStore.ts — 类型名改

`RetryNotificationRow` → `RetryNotification`，所有 store 赋值中 snake_case payload 字段改 camelCase。

## 5. Phase 5 扩展：非 Chat 模块

settings 和 orb 模块的 DTO 已纳入 `dto.rs`（见 3.1 DTO 表），无需额外拆分。前端类型定义和 UI 属性访问同步改 camelCase。

Config patch 适配（D12 修订）：`fieldDefinitions.ts` 的 `key` 改为 camelCase，与 `confTypes.ts` 对齐。`patchToSnakeCase()` 在 `patchConf()` 调用时自动将 camelCase key 转为 snake_case 发后端 `amend()`。UI 通过 field key 访问配置值，key 与 store 对象属性名一致，optimistic update 正确生效。

## 6. 测试要求

- 删除 `protocolContract.test.ts` 中的 DTO snake_case 契约断言和适配映射测试
- 改为断言前端 chatTypes.ts 类型使用 camelCase
- 所有 mock 数据从 snake_case 改为 camelCase
- **后端 round-trip 测试**：对每个 DTO 构造实例 → serialize to JSON → deserialize back → assert_eq，验证 `rename_all = "camelCase"` + `deny_unknown_fields` 组合不破坏 round-trip。可在 `src-tauri/tests/` 下新增 `dto_roundtrip.rs` 或在 dto.rs 同目录 `#[cfg(test)] mod tests`

## 7. 风险

| 风险 | 缓解 |
|------|------|
| DTO From impl 漏字段 | Rust struct 字段必须显式赋值，漏字段编译失败 |
| 后端输出与前端类型不匹配 | `deny_unknown_fields` 前端反序列化时立即报错 |
| event 字段名变更致前端旧版崩溃 | Tauri 桌面应用强制更新，无灰度需求 |
| ChatError.retry_after_ms 改名 | ChatError 不持久化，仅运行时使用 ✅ 已完成 |
| event 原始 struct 误被直接 emit | 原始文件已删除，不存在此风险 |
| DTO 新增字段后旧版前端反序列化失败 | `#[serde(default)]` 自动填充默认值 |
| skip_serializing_if 导致前端收不到 null 字段 | 前端 chatTypes.ts 已用 `?:` 可选属性，undefined 与字段缺失行为一致 |
| ChatDeltaPayloadDto 缺 Default | 原 ChatDeltaPayload 有 `#[derive(Default)]`，DTO 须同步派生 Default 或确保所有 Option 字段默认 None |
| settingsDispatcher.ts patch key 误改 camelCase | D13 修订：patch 赋值 key 改 camelCase（与 confTypes 对齐），`patchToSnakeCase()` 自动转换发后端 |

## 8. 可能改动的文件

> ⚠️ 以下列表可能不完整，实施时以实际代码搜索为准，不要仅依赖此列表。

### 后端

- `src-tauri/src/protocol/dto.rs` — 新建（所有 22 个 IPC DTO 集中）
- `src-tauri/src/protocol/mod.rs` — 精简为 `pub mod dto; pub use dto::*;`
- `src-tauri/src/protocol/chat_events.rs` — **删除**
- `src-tauri/src/protocol/chat_commands.rs` — **删除**
- `src-tauri/src/protocol/setting_commands.rs` — **删除**
- `src-tauri/src/protocol/setting_events.rs` — **删除**
- `src-tauri/src/protocol/orb_commands.rs` — **删除**
- `src-tauri/src/cmd/conversation.rs` — import + 返回类型改 DTO
- `src-tauri/src/cmd/chat.rs` — import + emit 构造点类型名改 Dto 后缀
- `src-tauri/src/cmd/agent_mode.rs` — import + 返回类型改 DTO
- `src-tauri/src/cmd/provider.rs` — import + 返回类型改 DTO
- `src-tauri/src/cmd/orb.rs` — import + 返回类型改 DTO
- `src-tauri/src/cmd/config.rs` — import + emit 构造点类型名改 Dto 后缀
- `src-tauri/src/setup.rs` — import + emit 构造点类型名改 Dto 后缀

### 前端

- `src/chat/core/chatTypes.ts` — 预补充 RetryNotification / ConversationDetail / TokenUsageSummary / 6 个 event payload（✅ 已完成）
- `src/chat/core/protocol.ts` — 字段改 camelCase，类型名去 Row；与 chatTypes.ts 重复的类型定义删除
- `src/chat/core/protocolAdapter.ts` — 整个文件删除
- `src/chat/core/chatErrors.ts` — retry_after_ms 改 retryAfterMs（✅ 已完成）
- `src/services/chatService.ts` — 简化 invoke
- `src/services/chatOrchestrator.ts` — 删 rowToTokenUsage 调用
- `src/services/chatDeltaEvents.ts` — payload 字段改 camelCase
- `src/services/historyService.ts` — 简化 invoke
- `src/stores/chatStore.ts` — 类型名改，store 赋值改 camelCase
- `src/chat/message/MessageBubble.tsx` — RetryNotification 属性改 camelCase
- `src/chat/message/MessageList.tsx` — RetryNotificationRow 改名
- `src/settings/provider/ProviderManager.tsx` — base_url/api_key_masked/default_model 改 camelCase
- `src/settings/provider/ConnectDialog.tsx` — base_url/api_key/api_key_changed/default_model 改 camelCase
- `src/settings/pages/AgentModeSettings.tsx` — max_tokens/reasoning_enabled/reasoning_effort/presentation_rank/is_builtin 改 camelCase
- `src/settings/agentMode/types.ts` — 同上
- `src/settings/core/field/fieldDefinitions.ts` — field key 保持 snake_case（D12），label/saveMessage 中 snake_case 引用改 camelCase
- `src/settings/core/settingsDispatcher.ts` — 读取侧改 camelCase（conf.rive_src→conf.riveSrc 等）；patch 赋值 key 保持 snake_case（D13）
- `src/settings/shared/renderers/MascotColorGroupRender.tsx` — primary_color/secondary_color 改 camelCase
- `src/settings/shared/renderers/AssetPickerField.tsx` — installed_path 改 camelCase
- `src/services/settingService.ts` — installed_path 改 camelCase
- `src/orb/useOrbConfSync.ts` — opacity_percent/scale_percent/hide_on_pointer_hover/hover_delay_ms/idle_opacity_percent/click_through 改 camelCase
- `src/orb/OrbExpand.ts` — click_through 改 camelCase
- `src/orb/OrbDrag.tsx` — click_through 改 camelCase
- `src/orb/OrbCoord.ts` — client_x/client_y 改 camelCase
- `src/orb/OrbBall.tsx` — primary_color/secondary_color 改 camelCase
- `src/services/orbService.ts` — MousePositionRow/ScreenSizeRow 类型名改，字段改 camelCase
- `src/components/PinToggle.tsx` — stay_on_top 改 camelCase
- `src/chat/message/MessageInput.tsx` — primary_color/secondary_color 改 camelCase
- `src/chat/compact/CompactBanner.tsx` — default_model 改 camelCase
- `src/chat/ChatView.tsx` — default_model 改 camelCase
- `src/components/commandPalette/CommandPalette.tsx` — command_palette 改 camelCase

### 测试

- `protocolContract.test.ts` — 删旧断言，改 camelCase 契约断言
- `chatOrchestrator.test.ts` — mock 数据改 camelCase
- `chatService.test.ts` — mock 数据改 camelCase
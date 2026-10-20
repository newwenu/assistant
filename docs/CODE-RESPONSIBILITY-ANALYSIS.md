# 代码文件职能检查与功能提取分析

> 生成日期：2026-10-19
> 范围：`src/`（前端 TypeScript）+ `src-tauri/src/`（后端 Rust）

---

## 1 模块总览

### 1.1 前端 (`src/`)

| 目录 | 文件数 | 核心职责 | 评价 |
|------|--------|---------|------|
| `services/` | 17 | Tauri invoke 封装 + 事件订阅 + 全局错误处理 | ⚠️ 有重叠 |
| `stores/` | 11 | Zustand 状态管理 | ⚠️ 部分过重 |
| `chat/core/` | 7 | 对话领域类型 / 错误 / 流式 / 动画 | ⚠️ 边界模糊 |
| `orb/` | 7 | 悬浮球 UI + 坐标 + 拖拽 + 展开 | ✅ 职责清晰 |
| `mascot/` | 10+ | 形象渲染 + 表情联动 | ✅ 职责清晰 |
| `settings/` | 15+ | 声明式设置系统 | ⚠️ Dispatcher 过重 |
| `tools/` | 20+ | 工具箱 | ✅ 职责清晰 |
| `i18n/` | 3 | 国际化 | ✅ 职责清晰 |
| `history/` | 5 | 历史记录视图 | ✅ 职责清晰 |

### 1.2 后端 (`src-tauri/src/`)

| 目录 | 文件数 | 核心职责 | 评价 |
|------|--------|---------|------|
| `cmd/` | 10 | Tauri command 处理函数 | ⚠️ chat.rs 过重 |
| `protocol/` | 3 | DTO + 事件名 + emit 工具 | ✅ 职责清晰 |
| 根目录 | 5 | lib.rs / setup / tray / window / crash_dump | ✅ 职责清晰 |

---

## 2 前端问题详列

### 2.1 🔴 P0 — 功能重叠 / 重复代码

#### 2.1.1 `historyService.ts` 与 `chatService.ts` 职能重叠

**文件**：
- `src/services/historyService.ts`（44 行）
- `src/services/chatService.ts`（142 行）

**事实**：

`historyService` 中以下方法直接委托 `chatService` 同名方法，零逻辑增量：

```ts
getConversationPaginated: chatService.getConversationPaginated,
getConversationAllMessages: chatService.getConversationAllMessages,
```

以下方法在两个 service 中各自独立调用 `invoke`，做完全相同的事：

| 方法 | historyService | chatService |
|------|---------------|------------|
| `deleteConversation` | `invoke("delete_conversation", { id })` | `invoke("delete_conversation", { id })` |
| `renameConversation` | `invoke("rename_conversation", ...)` | `invoke("rename_conversation", ...)` |
| `toggleConversationPin` | `invoke("toggle_conversation_pin", { id })` | `invoke("toggle_conversation_pin", { id })` |
| `duplicateConversation` | `invoke("duplicate_conversation", { id })` | `invoke("duplicate_conversation", { id })` |
| `getMessageSubtreeSize` | `invoke("get_message_subtree_size", ...)` | `invoke("get_message_subtree_size", ...)` |
| `deleteMessagePersist` | `invoke("delete_message_persist", ...)` | `invoke("delete_message_persist", ...)` |

`historyService` 独有的方法仅 3 个：`searchConversations`、`exportConversation`、`exportConversationToFile`。

**影响**：6 个方法重复定义，修改一处容易遗漏另一处。

**建议**：
- `historyService` 删除 6 个重复/委托方法，仅保留 3 个独有方法
- `historyService` 需要调用已删除方法时，直接引用 `chatService`

**✅ 已完成**：8 个方法全部改为委托 `chatService`（含原有的 2 个委托 + 6 个重复 invoke），`historyService` 仅保留 3 个独有方法（`searchConversations`、`exportConversation`、`exportConversationToFile`）。`renameConversation` 的 `titleSource` 参数统一为必选 `string`，所有调用方均兼容。

---

### 2.2 🟡 P1 — 职责过重 / 需拆分

#### 2.2.1 `settingsDispatcher.ts` 承担了过多职责

**文件**：`src/settings/core/settingsDispatcher.ts`（169 行）

**事实**：该文件包含以下功能模块：

| 功能 | 行数 | 性质 | 当前归属 |
|------|------|------|---------|
| `dispatchConfPatch` — 配置 patch 分发 | ~15 | 核心分发 | ✅ 留在此 |
| `dispatchProviderSaved` / `dispatchProviderDeleted` | ~15 | 核心分发 | ✅ 留在此 |
| `dispatchThemeChanged` — 主题变更分发 | ~15 | 核心分发 | ✅ 留在此 |
| `emitSettingEventToPeers` — 跨窗口事件发射 | ~12 | 基础设施 | 可提取 |
| `buildRendererConfig` / `buildMascotPatch` | ~35 | Mascot 配置构建 | 可提取 |
| `optimisticConfUpdate` | ~8 | 乐观更新 | 可提取 |
| `toConfRecord` | ~4 | 类型转换 | 可提取 |

**建议**：
- `emitSettingEventToPeers` → 提取为 `services/crossWindowEvent.ts`（跨窗口通信是通用基础设施）
- `buildRendererConfig` / `buildMascotPatch` → 提取为 `mascot/core/mascotConfig.ts`（Mascot 领域逻辑）
- `optimisticConfUpdate` → 提取为 `settings/core/optimisticUpdate.ts`
- `toConfRecord` → 提取为 `settings/core/types/confUtils.ts`
- Dispatcher 只保留 3 个 `dispatch*` 函数

---

#### 2.2.2 `providerStore.ts` 混合了静态注册表和动态状态

**文件**：`src/stores/providerStore.ts`（133 行）

**事实**：模块顶层构建了大量静态映射（约 60 行），这些数据在应用生命周期内不变：

```ts
export const API_FORMATS: ApiFormatInfo[] = ...      // 静态数据
export const API_FORMAT_MAP = new Map<...>(...)      // 静态映射
export const PROVIDER_PRESETS: ProviderPreset[] = ... // 静态数据
export const PRESET_MAP = new Map<...>(...)          // 静态映射
export function getPresetByType(...)                 // 静态查询
export function getApiFormatByType(...)              // 静态查询
export function getFormatInfo(...)                   // 静态查询
export function formatNeedsApiKey(...)               // 静态查询
```

Zustand store 部分仅占约 30 行。

**问题**：静态注册表查询与 Zustand 动态状态混在同一文件，违反单一职责。静态数据不需要 store 的响应式能力。

**建议**：
- 提取静态部分 → `settings/core/provider/providerLookup.ts`
- `providerStore.ts` 只保留 `{ providers, loading, loadProviders, saveProvider, deleteProvider, fetchModels }`

---

#### 2.2.3 `chatStorePorts.ts` 过于厚重

**文件**：`src/stores/chatStorePorts.ts`（317 行）

**事实**：文件混合了纯函数和适配器创建：

| 内容 | 行数 | 性质 |
|------|------|------|
| `resolveContextWindowTokens` | ~8 | 纯函数，仅查 providerStore |
| `fetchTokenUsage` | ~12 | 纯函数，调用 chatService |
| `mergeStreamingContent` | ~6 | 纯函数，消息合并 |
| `createStatePort` | ~120 | 适配器创建 |
| `createEnvPort` | ~15 | 适配器创建 |
| `createOrchestrator` | ~8 | 工厂函数 |

**问题**：3 个纯函数与适配器耦合在同一文件，纯函数无法独立测试。

**建议**：
- 提取 `resolveContextWindowTokens`、`fetchTokenUsage`、`mergeStreamingContent` → `chat/core/chatUtils.ts`
- `chatStorePorts.ts` 只保留适配器创建

---

#### 2.2.4 `App.tsx` 主窗口布局逻辑过重

**文件**：`src/App.tsx`（约 300 行）

**事实**：`MainLayout` 组件（约 200 行）包含：
- 侧边栏 + 菜单 + 路由
- 错误通知消费（`useErrorNotifyStore` + `notification.useNotification`）
- 设置加载（5 个 `useEffect` 加载不同配置）
- 快捷键警告监听
- 导航事件监听

**建议**：
- 提取 `MainLayout` → `components/MainLayout.tsx`
- 提取错误通知消费 → `hooks/useErrorNotifications.ts`
- `App.tsx` 只保留窗口分流（`MainLayout` / `SettingsWindow` / `OrbWindow`）+ 路由定义

---

#### 2.2.5 `chatStore.ts` 过大

**文件**：`src/stores/chatStore.ts`（536 行）

**事实**：包含对话 CRUD、消息操作、分支操作、compact 操作等所有逻辑，接口定义 30+ 个方法。

**建议**：使用 Zustand slice pattern 拆分：
- `chatConversationSlice.ts` — 对话 CRUD（loadConversations、createConversation、switchConversation、deleteConversation、renameConversation、updateModel、togglePin、duplicateConversation、batchDelete、purgeAll）
- `chatMessageSlice.ts` — 消息操作（sendMessage、resendFrom、cancelStream、deleteMessage、editMessage、regenerateMessage、retryFromError、loadMoreMessages）
- `chatCompactSlice.ts` — compact 操作（manualCompact、unlockConversation、clearCompactError）
- `chatStore.ts` — 组合 slices

---

### 2.3 🟢 P2 — 代码卫生 / 小改进

#### 2.3.1 `tauriInvoke.ts` 未被使用

**文件**：`src/services/tauriInvoke.ts`（20 行）

**事实**：提供了带日志的 `invoke` 封装，但所有 service 都直接用 `@tauri-apps/api/core` 的 `invoke`。

**建议**：统一使用 `tauriInvoke`（获得自动错误日志），或删除此文件。

**✅ 已完成**：8 个文件（chatService、historyService、orbService、settingService、providerService、agentModeService、toolService、tools/os）的 `import { invoke } from "@tauri-apps/api/core"` 统一替换为 `import { tauriInvoke as invoke } from "./tauriInvoke"`，所有 `invoke(...)` 调用无需逐行修改。测试文件 mock 路径同步更新，6 个测试全部通过。

---

#### 2.3.2 `protocol.ts` 空文件

**文件**：`src/chat/core/protocol.ts`（0 行）

**事实**：空文件，无任何内容。

**建议**：删除，或添加注释说明规划用途。

---

#### 2.3.3 `chatErrors.ts` 与 `chatErrorRecovery.ts` 边界模糊

**文件**：
- `src/chat/core/chatErrors.ts`（182 行）
- `src/chat/core/chatErrorRecovery.ts`（200+ 行）

**事实**：
- `chatErrors.ts` 包含：类型定义（`ChatError`、`ChatErrorCode`、`ChatErrorCategory`）+ 分类逻辑（`inferCategory`）+ 转换逻辑（`toChatError`）+ **UI 展示逻辑**（`getErrorDisplay`、`formatRetryAfter`、`ERROR_ICONS`）
- `chatErrorRecovery.ts` 包含：恢复策略（`getErrorPresentation`、`getRecoveryActions`、`executeRecoveryAction`）

`getErrorDisplay`（chatErrors）和 `getErrorPresentation`（chatErrorRecovery）功能重叠——都提供错误的可展示信息。

**建议**：
- `chatErrors.ts` 只保留类型 + 分类 + 转换（`ChatError`、`ChatErrorCode`、`ChatErrorCategory`、`inferCategory`、`toChatError`、`isChatError`）
- 提取 `getErrorDisplay`、`formatRetryAfter`、`ERROR_ICONS` → `chat/core/chatErrorDisplay.ts`
- `chatErrorRecovery.ts` 保持不变

---

#### 2.3.4 `OrbDrag.tsx` 平台逻辑与 Hook 混合

**文件**：`src/orb/OrbDrag.tsx`

**事实**：`IS_WINDOWS` / `IS_MACOS` 检测和平台特定拖拽策略与 React hook 混合在同一文件。

**建议**：提取平台策略 → `orb/platformDrag.ts`，`OrbDrag.tsx` 只保留 hook 接口。

---

#### 2.3.5 `main.tsx` 循环依赖风险

**文件**：`src/main.tsx`（97 行）

**事实**：`applyLocaleChange()` 在 `main.tsx` 中定义，但被以下模块通过 `await import("@/main")` 动态导入：
- `settingsStore.ts`
- `settingEvents.ts`
- `initSettingSystem.ts`

形成隐式循环依赖：`main → settingsStore → main`。

**建议**：提取 `applyLocaleChange` → `i18n/localeApplier.ts`，消除对 `main.tsx` 的反向依赖。

---

#### 2.3.6 `errorNotifier.ts` 目录归属

**文件**：`src/services/errorNotifier.ts`

**事实**：用 Zustand store 实现通知队列，放在 `services/` 目录下。虽然 Zustand 作为事件总线是合理模式，但目录语义上 `services/` 是 Tauri invoke 封装层。

**建议**：移到 `stores/` 下以保持目录语义一致，或保留但添加注释说明设计意图。

---

#### 2.3.7 `historyEvents.ts` 中 3 个 debounce 实例可合并

**文件**：`src/services/historyEvents.ts`

**事实**：`debouncedRefreshList` / `debouncedRefreshListForTerminated` / `debouncedRefreshListForUnlocked` 做同一件事（`refreshConversationList`），分开实例可能在短时间内连续触发 3 次刷新。

**背景**：`chatLifecycleEvents.ts` 与 `historyEvents.ts` 监听相同事件不是问题——前者在 Orb 窗口运行，后者在主窗口运行，是跨窗口数据同步的有意设计。

**建议**：合并为 1 个共享 debounce 实例。

---

## 3 后端问题详列

### 3.1 🟡 P1 — 职责过重 / 需拆分

#### 3.1.1 `cmd/chat.rs` 过重（609 行）

**文件**：`src-tauri/src/cmd/chat.rs`

**事实**：该文件包含：

| 内容 | 行数 | 性质 |
|------|------|------|
| `send_chat_message` command | ~200 | command 处理 |
| `StreamEmitCtx` + `emit_delta` + `emit_stream_error` | ~60 | 流式发射辅助 |
| `resolve_context_window` + `estimate_tokens` + `resolve_final_tokens` + `resolve_partial_tokens` | ~40 | 纯函数 |
| `DbWriteMsg` + `spawn_db_writer` | ~30 | DB 异步写入 |
| `spawn_finish_listener` | ~50 | 流完成监听 |
| `spawn_compaction_listener` | ~30 | compact 监听 |
| `save_partial_on_error` | ~10 | 错误保存 |

**问题**：
1. `send_chat_message` 是整个应用最长的函数（~200 行），包含消息插入、分支处理、provider 解析、流启动、结果处理等全部逻辑
2. 流式辅助函数（emit_delta、emit_stream_error）与 command 处理耦合
3. 三个 `spawn_*` 函数是独立的生命周期管理，与 command 逻辑无关

**建议**：
- 提取流式辅助 → `cmd/chat_stream.rs`（StreamEmitCtx、emit_delta、emit_stream_error、spawn_db_writer、spawn_finish_listener、spawn_compaction_listener）
- 提取纯函数 → `cmd/chat_utils.rs`（resolve_context_window、estimate_tokens、resolve_*_tokens、save_partial_on_error）
- `chat.rs` 只保留 `send_chat_message` 和 `cancel_chat` 两个 command

---

#### 3.1.2 `cmd/conversation.rs` 与 `cmd/chat.rs` 边界模糊

**文件**：
- `src-tauri/src/cmd/conversation.rs`（~400 行）
- `src-tauri/src/cmd/chat.rs`（609 行）

**事实**：
- `conversation.rs` 包含：create、list、get、delete、rename、updateModel、updateProvider、getPaginated、getAllMessages、deleteMessage、search、export、togglePin、duplicate、batchDelete、purge、getTokenUsage、createBranch、switchBranch、getMessageSiblings、forkToNewConversation、incrementGeneration、executeCompaction、messagesWithSiblingCounts
- `chat.rs` 包含：sendChatMessage、cancelChat + 流式辅助

`conversation.rs` 实际上承载了所有对话 CRUD + 分支操作 + compact 执行，而 `chat.rs` 承载了流式发送。命名上 "conversation" vs "chat" 的分界不清晰——分支操作（createBranch、switchBranch）和 compact（executeCompaction）在语义上更接近 chat 流程。

**建议**：
- 保持当前拆分（conversation = CRUD，chat = 流式），但将 `executeCompaction` 移到 `chat.rs` 或独立的 `cmd/compaction.rs`
- `messagesWithSiblingCounts` 是纯辅助函数，提取到 `cmd/conversation_utils.rs`

---

#### 3.1.3 `cmd/config.rs` 混合了配置管理和 Mascot 资产安装

**文件**：`src-tauri/src/cmd/config.rs`（341 行）

**事实**：该文件包含：

| 内容 | 行数 | 性质 |
|------|------|------|
| Side-effect registry（APP_SIDE_EFFECTS / ORB_SIDE_EFFECTS） | ~60 | 配置变更副作用 |
| `patch_conf` / `get_*_conf` / `set_theme` / `set_locale` | ~60 | 配置 command |
| `install_mascot_asset` / `uninstall_mascot_asset` + 辅助 | ~140 | Mascot 资产管理 |
| `apply_autostart` | ~8 | 自启动辅助 |

**问题**：Mascot 资产安装/卸载是独立的文件管理功能，与配置 patch 无关。

**建议**：
- 提取 Mascot 资产管理 → `cmd/mascot_asset.rs`（install_mascot_asset、uninstall_mascot_asset、AssetSpec、validate_mime、mascot_assets_dir、get_asset_spec）
- `config.rs` 只保留配置 command + side-effect registry

---

### 3.2 🟢 P2 — 代码卫生

#### 3.2.1 `tray.rs` 的 `rebuild_tray` 与 `create_tray` 大量重复

**文件**：`src-tauri/src/tray.rs`

**事实**：`rebuild_tray` 几乎完整复制了 `create_tray` 的菜单构建代码（约 60 行重复），仅为了更新 i18n 文本。

**建议**：提取 `build_tray_menu(app) -> Menu` 公共函数，`create_tray` 和 `rebuild_tray` 共用。

---

#### 3.2.2 `protocol/dto.rs` 单文件过大（~600 行）

**文件**：`src-tauri/src/protocol/dto.rs`

**事实**：包含 Chat DTOs、Settings DTOs、Config DTOs、Orb DTOs、Branch DTOs 等所有数据传输对象。

**建议**：按领域拆分为 `dto/chat.rs`、`dto/config.rs`、`dto/orb.rs`，`dto/mod.rs` 重新导出。

---

#### 3.2.3 `lib.rs` 的 `invoke_handler` 注册列表过长

**文件**：`src-tauri/src/lib.rs`（151 行）

**事实**：`invoke_handler` 宏中列出了 50+ 个 command 函数。

**建议**：可保持现状（Tauri 框架限制），或按模块分组注释提高可读性。

---

## 4 跨前后端一致性问题

### 4.1 事件名命名不一致

**事实**：后端 `event_name` 中：

```rust
pub const CONVERSATION_TERMINATED: &str = "conversation_terminated";  // snake_case
pub const CONVERSATION_UNLOCKED: &str = "conversation_unlocked";      // snake_case
pub const CONVERSATION_LIST_CHANGED: &str = "conversation-list-changed"; // kebab-case
pub const CHAT_MESSAGES_CHANGED: &str = "chat-messages-changed";       // kebab-case
pub const CHAT_DELTA: &str = "chat-delta";                             // kebab-case
```

`conversation_terminated` 和 `conversation_unlocked` 使用 snake_case，其余使用 kebab-case。

**建议**：统一为 kebab-case：`conversation-terminated`、`conversation-unlocked`。需同步修改前端监听。

---

### 4.2 DTO 字段 `active_branch_id` 前后端处理差异

**事实**：
- 后端 `ConversationDto.active_branch_id` 使用 `#[serde(default, skip_serializing_if = "Option::is_none")]`
- 前端 `Conversation.activeBranchId` 在 `chatStore.ts` 中用 `?? null` 处理

两者语义一致，但后端 `ConversationRow` 的 `active_branch_id` 是 `Option<String>`，而前端有时用 `undefined` 有时用 `null`。

**建议**：前端统一使用 `string | null`（与后端 `Option<String>` 对应），避免 `undefined` / `null` 混用。

---

## 5 提取优先级总表

| 优先级 | 编号 | 问题 | 影响范围 | 工作量 | 风险 |
|--------|------|------|---------|--------|------|
| 🔴 P0 | 2.1.1 | historyService ↔ chatService 方法重复 | 消除 6 个重复方法 | 小 | 低 | ✅ 已完成 |
| 🟢 P2 | 2.3.7 | historyEvents 3 个 debounce 合并为 1 个 | 减少重复刷新 | 极小 | 无 |
| 🟡 P1 | 2.2.1 | settingsDispatcher 拆分 | 降低单文件复杂度 | 中 | 低 |
| 🟡 P1 | 2.2.2 | providerStore 静态数据提取 | 职责分离 | 小 | 低 |
| 🟡 P1 | 2.2.3 | chatStorePorts 纯函数提取 | 可测试性提升 | 小 | 低 |
| 🟡 P1 | 2.2.4 | App.tsx MainLayout 提取 | 可维护性 | 中 | 低 |
| 🟡 P1 | 2.2.5 | chatStore slice 拆分 | 降低单文件复杂度 | 大 | 中 |
| 🟡 P1 | 3.1.1 | cmd/chat.rs 拆分 | 降低单文件复杂度 | 中 | 低 |
| 🟡 P1 | 3.1.2 | conversation.rs vs chat.rs 边界 | 职责清晰 | 小 | 低 |
| 🟡 P1 | 3.1.3 | cmd/config.rs mascot 资产提取 | 职责分离 | 小 | 低 |
| 🟢 P2 | 2.3.1 | tauriInvoke 统一/删除 | 代码卫生 | 小 | 低 | ✅ 已完成 |
| 🟢 P2 | 2.3.2 | protocol.ts 空文件 | 代码卫生 | 极小 | 无 |
| 🟢 P2 | 2.3.3 | chatError 展示逻辑分离 | 职责清晰 | 小 | 低 |
| 🟢 P2 | 2.3.4 | OrbDrag 平台策略提取 | 可测试性 | 小 | 低 |
| 🟢 P2 | 2.3.5 | applyLocaleChange 循环依赖 | 架构健康 | 小 | 低 |
| 🟢 P2 | 2.3.6 | errorNotifier 目录归属 | 语义一致 | 极小 | 无 |
| 🟢 P2 | 3.2.1 | tray.rs 重复代码 | 消除重复 | 小 | 低 |
| 🟢 P2 | 3.2.2 | dto.rs 按领域拆分 | 可维护性 | 中 | 低 |
| 🟢 P2 | 4.1 | 事件名命名不一致 | 一致性 | 小 | 中 |
| 🟢 P2 | 4.2 | activeBranchId null/undefined 混用 | 类型安全 | 小 | 低 |

---

## 6 架构亮点（值得保持的设计）

| 设计 | 位置 | 说明 |
|------|------|------|
| chatOrchestrator 的 Port/Adapter 模式 | `chatOrchestrator.ts` + `chatStorePorts.ts` | `ChatStatePort` / `ChatEnvPort` 接口解耦了 orchestrator 与 store，orchestrator 可独立测试 |
| streamCoalescer 的调度器抽象 | `streamCoalescer.ts` | `FlushScheduler` 接口使 rAF/timer 可替换，测试友好 |
| mascot 的 ExpressionPolicy | `mascot/core/expressionPolicy.ts` | 表情策略可配置，`AckRule` 优先级排序设计优雅 |
| settingsDispatcher 统一链路 | `settingsDispatcher.ts` | `dispatchConfPatch → invoke → store.load → emitSettingEventToPeers` 保证了跨窗口一致性 |
| OrbCoord 的坐标系统一 | `orb/OrbCoord.ts` | `physicalToLogical` / `logicalToPhysical` 避免了前后端像素混淆 |
| lazyWithRecovery 的模块恢复 | `utils/lazyWithRecovery.tsx` | 懒加载 + 错误恢复 + 订阅恢复机制，健壮性好 |
| Rust side-effect registry | `cmd/config.rs` | `APP_SIDE_EFFECTS` / `ORB_SIDE_EFFECTS` 静态数组实现配置变更副作用，声明式且可扩展 |
| Rust DTO round-trip test | `protocol/dto.rs` | `#[cfg(test)] mod tests` 验证 serde camelCase 序列化往返，保证前后端协议一致 |

---

## 7 建议执行顺序

1. **第一批（P0，消除重叠）**：~~2.1.1~~ ✅ 已完成
2. **第二批（P1 前端，降低复杂度）**：2.2.2 → 2.2.3 → 2.2.1 → 2.2.4 → 2.2.5
3. **第三批（P1 后端，降低复杂度）**：3.1.3 → 3.1.2 → 3.1.1
4. **第四批（P2，代码卫生）**：2.3.7 → 2.3.1 → 2.3.2 → 2.3.5 → 2.3.3 → 2.3.4 → 2.3.6 → 3.2.1 → 3.2.2 → 4.1 → 4.2
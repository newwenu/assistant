# I17 — Chat 重发/重试恢复体系重构规格说明

## 1. 交付目标

将当前 4 条行为不一致的重发/重试路径统一为 **共享的「截断+重发」原语**，并引入**错误类别感知的恢复动作映射**，使前端能根据错误类型提供精确恢复建议（而非一刀切"重试"按钮）。

**设计原则**：
- **统一原语**：所有重试/重发/编辑重发操作收敛到共享的 `resendFrom(pivotTimestamp, content)` 原语，消除 `handleRetry` / `regenerateMessage` / `editMessage` / `deltaRetry` 四条路径的行为分歧；`retryFromError(messageId)` 和 `editMessage(messageId, newContent)` 均基于此原语构建
- **错误类别感知**：后端 `ProviderErrorPayload` 透传 `httpStatus` + `provider`，前端 `normalizeChatError()` 做跨 provider 归一化分类，`getRecoveryActions()` 按 category 映射恢复动作
- **精确关联**：重试操作必须关联到出错的 assistant 消息 ID，禁止"找最后一条 user 消息"的模糊定位
- **幂等安全**：错误事件处理增加 terminal dedup，防止重复事件导致重复状态变更
- **可取消重试**：后端 `'retry` 循环的 sleep 改为 `tokio::select!` + cancel_token，用户可中断等待中的重试
- **渐进实施**：每个 Step 独立可测，不破坏现有行为

> **前置依赖**：I16 ✅（`attemptId` 过滤已实现，流事件归属正确）
> **核心参考**：OpenBitFun `aiErrorPresenter.ts`（`normalizeCategory` + `DEFAULT_ACTIONS` + `getAiErrorPresentation`）、`interruptedTurnRecovery.ts`（恢复候选选择 + 门控）

---

## 2. 当前问题全景

### 2.1 四条重发/重试路径行为分歧

| # | 路径 | 触发方式 | 签名 | 定位方式 | 清理失败消息 | 取消流式 | 竞态保护 | 幂等保护 | timestamp 一致 |
|---|------|---------|------|---------|-------------|---------|---------|---------|---------------|
| R1 | `handleRetry` | ErrorBlock 重试按钮 | `() => void` | ❌ 找最后一条 user 消息 | ❌ | ❌ | ❌ | ❌ | N/A |
| R2 | `regenerateMessage` | MessageActions 重新生成 | `(messageId: string) => Promise<void>` | ✅ 按 timestamp 找前一条 user 消息 | ✅ 删自身+后续 | ❌ | ❌ | ❌ | ✅ |
| R3 | `deltaRetry` (后端自动) | Rust 重试循环通知 | `RetryNotification` | 后端控制 | N/A | N/A | N/A | N/A | N/A |
| R4 | `editMessage` | 编辑 user 消息后重发 | `(messageId, newContent) => Promise<void>` | ✅ 精确 messageId | ✅ 删后续（保留自身） | ✅ | ❌ | ❌ | ❌ 后端改 timestamp 前端不改 |

**R1 根本问题**：`onRetry` 是无参回调 `() => void`，从 `ErrorBlock` → `MessageBubble` → `MessageList` → `ChatView` 一路传递，**丢失了出错消息的 ID 上下文**。无论哪条消息出错，都重发最后一条 user 消息。

**场景**：对话 3 轮问答，第 2 轮 assistant 报错。用户点击第 2 轮错误重试 → 实际重发第 3 轮 user 消息，完全错误。

**R4 与 R2 的关键差异**：`editMessage` 是唯一做了取消流式保护的路径，但它有独有的 timestamp 不一致 bug（见 2.7）。

### 2.2 错误分类过于粗糙

当前 `ChatError` 仅有 `retryable: boolean`，无语义类别、无严重度、无恢复动作映射：

```typescript
interface ChatError {
  code: ChatErrorCode;      // 13 枚举，1:1 映射 Rust 变体
  message?: string;
  retryAfterMs: number | null;
  retryable: boolean;       // ← 唯一恢复信号
}
```

对比参考项目 `AiErrorPresentation`：

```typescript
interface AiErrorPresentation {
  category: AiErrorCategory;           // 13 语义类别（跨 provider 归一化）
  titleKey: string;                    // i18n key
  messageKey: string;                  // 建议文案 i18n key
  severity: 'warning' | 'error';       // 严重度
  retryable: boolean;
  actions: AiErrorAction[];            // 恢复动作列表
  diagnostics: string;                 // 诊断信息
}
```

**具体差距**：

| 错误场景 | 我们 | 参考项目 |
|----------|------|---------|
| `ContextOverflow` | 给"重试"按钮 → 重试必然再次溢出 | 引导"压缩上下文"或"新建对话" |
| `AuthFailed` | 给"重试"按钮 → 重试必然再次认证失败 | 引导"打开模型设置" |
| `ProviderQuota` | 给"重试"按钮 → 重试必然再次额度不足 | 引导"充值/换模型" |
| `RateLimited` | 给"重试"按钮，无倒计时 | 引导"等待 N 秒后重试"或"换模型" |
| `ContentPolicy` | 可能给"重试"按钮 | 不提供重试（`retryable: false`），仅"复制诊断" |

### 2.3 后端错误信息丢失

`ProviderErrorPayload` 不传 `httpStatus` 和 `provider`，前端无法做二次推断：

```rust
pub struct ProviderErrorPayload {
    pub code: ProviderErrorCode,     // ← 枚举直传
    pub message: String,
    pub retry_after_ms: Option<u64>,
    pub retryable: bool,             // ← 仅布尔值
    // ❌ 缺少: http_status, provider, provider_code, provider_message
}
```

后端 `classify_http_error()` 已消费 HTTP 状态码做分类，但分类结果（`ProviderErrorCode`）丢失了原始状态码和 provider 信息，前端无法做跨 provider 归一化。

### 2.4 后端重试不可取消

每个 provider 的 `'retry` 循环中 `tokio::time::sleep` 不检查 cancel_token：

```rust
tokio::time::sleep(Duration::from_millis(delay)).await;  // ← sleep 期间不检查取消
attempt += 1;
continue 'retry;
```

用户点"停止生成"时，如果后端正在 sleep 等待重试，无法立即中断。

**影响范围**：5 个 provider 文件共 **20 处** `tokio::time::sleep`（每个文件 4 处，分别对应连接错误重试、HTTP 错误重试、首 token 超时重试、SSE 流错误重试）：

| 文件 | sleep 数量 |
|------|-----------|
| `openai/responses_stream.rs` | 4 |
| `openai/chat.rs` | 4 |
| `gemini/request.rs` | 4 |
| `gemini/code_assist.rs` | 4 |
| `anthropic/request.rs` | 4 |

### 2.5 错误事件无幂等保护

如果后端因网络抖动发送两次错误事件，前端会执行两次 `errorStream()`，设置两次 error 状态。

### 2.6 `chat-regenerate` 事件前端未监听

后端 `regenerate_message` 发出 `chat-regenerate` 事件，但前端 `chatDeltaEvents.ts` 和 `chatStore.ts` **均未注册该事件的 handler**。`ChatRegeneratePayload` 类型虽存在于 `chatTypes.ts`，但无任何消费逻辑。

当前 `chatStore.regenerateMessage` 自己做了前端 `filter` 删除 + 调后端 + `sendMessage`，碰巧与后端行为一致。但若后端删除逻辑变化（如改为软删除），前端将不同步。本质风险是**前端与后端的消息删除依赖隐式约定而非显式事件驱动**。

### 2.7 `editMessage` 前后端 timestamp 不一致

后端 `edit_message`（[conversations.rs:L454-482](file:///g:/codeversion/assistant/src-tauri/crates/assistant-storage/src/conversations.rs#L454-L482)）执行两步 SQL：

```sql
-- Step 1: 删除 timestamp >= 该消息且 id != 该消息 的所有消息（即删除后续）
DELETE FROM messages WHERE conversation_id = ?1 AND timestamp >= ?2 AND id != ?3

-- Step 2: 更新内容 + 重置 timestamp 为 now()
UPDATE messages SET content = ?2, timestamp = ?3 WHERE id = ?1
```

**Step 2 把 timestamp 改为 `now()`**，但前端 `editMessage` 只更新了 `content`，没有更新 `timestamp`：

```typescript
// 前端 chatStore.editMessage — 只改 content，不改 timestamp
.map((m) => (m.id === messageId ? { ...m, content: newContent } : m))
```

**后果**：
1. 前端消息的 `timestamp` 是旧值，后端是 `now()`，下次 `get_conversation_detail` 加载时排序位置可能变化
2. 如果用户快速连续编辑，timestamp 乱序导致消息显示顺序错误
3. 后续的 `filter(m => m.timestamp < msg.timestamp)` 依赖 timestamp 做截断，不一致的 timestamp 会导致截断位置错误

### 2.8 `editMessage` cancel→send 时序窗口

```typescript
// ① cancel
await chatService.cancelChat(state.currentConversationId).catch(() => {});
set({ isStreaming: false, streamingContent: "", isReasoningStreaming: false });

// ② filter 消息 + 后端 editMessage（异步）

// ③ sendMessage
void get().sendMessage(newContent);
```

在 ① cancel 完成后、③ sendMessage 开始前，`isStreaming: false` 但新 `send()` 还没开始。这段时间内：
- 如果 cancel 的 `Cancelled` error 事件延迟到达，会被当前 attemptId 处理，设置 `phase: "error"`
- 然后 ③ 的 `sendMessage` 开始时 `phase` 是 `"error"` 而非 `"idle"`，orchestrator 状态可能异常

**R2 `regenerateMessage` 也有此时序窗口**，但它没有 ① 的 cancel 步骤（假设调用方保证不在流式中调用），所以风险更低。R4 是唯一同时有 cancel + resend 的路径，时序窗口风险最高。

---

## 3. 参考项目做法

### 3.1 OpenBitFun — 错误类别感知恢复

**三层降级分类** `normalizeCategory()`：

```
Layer 1: 后端已分类 → 直接使用 category
Layer 2: HTTP 状态码推断 → 402→provider_quota, 401→auth, 429→rate_limit, 529→provider_unavailable
Layer 3: 消息文本模式匹配 → "insufficient_quota"→provider_quota, "context window"→context_overflow, ...
```

**恢复动作映射** `DEFAULT_ACTIONS`：

```typescript
const DEFAULT_ACTIONS: Record<AiErrorCategory, AiErrorActionCode[]> = {
  network:           ['retry', 'switch_model', 'copy_diagnostics'],
  auth:              ['open_model_settings', 'copy_diagnostics'],
  rate_limit:        ['wait_and_retry', 'switch_model', 'copy_diagnostics'],
  context_overflow:  ['compress_context', 'start_new_chat'],
  timeout:           ['retry', 'switch_model', 'copy_diagnostics'],
  provider_quota:    ['open_model_settings', 'switch_model', 'copy_diagnostics'],
  provider_billing:  ['open_model_settings', 'switch_model', 'copy_diagnostics'],
  provider_unavailable: ['wait_and_retry', 'switch_model', 'copy_diagnostics'],
  permission:        ['open_model_settings', 'copy_diagnostics'],
  invalid_request:   ['copy_diagnostics'],
  content_policy:    ['copy_diagnostics'],
  model_error:       ['retry', 'switch_model', 'copy_diagnostics'],
  unknown:           ['retry', 'switch_model', 'copy_diagnostics'],
};
```

**统一重试入口**：`ChatActions.retryMessage(conversationId: string)` — 精确关联到出错对话，不存在"找最后一条 user 消息"的歧义。

### 3.2 OpenBitFun — 中断恢复

`interruptedTurnRecovery.ts` 实现了完整的取消后恢复子系统：

- `isTurnAwaitingRecovery(turn)` — 判断 turn 是否处于可恢复状态
- `selectInterruptedTurnRecovery(session, composer)` — 选择恢复候选，含大量安全门控（模型变更、composer 草稿、远程连接等）
- 恢复按钮 UI — "继续生成"按钮

### 3.3 OpenBitFun — 幂等错误处理

```typescript
const terminalKey = `${sessionId}:${turnId}`;
if (context.handledTerminalTurnEvents.has(terminalKey)) {
  return;  // ← 防止重复处理
}
context.handledTerminalTurnEvents.add(terminalKey);
```

---

## 4. 重构方案

### 4.1 架构总览

```
┌──────────────────────────────────────────────────────────────────────┐
│  Layer 5: UI 组件                                                    │
│  ErrorBlock → 按恢复动作渲染多个按钮（retry/switch_model/...）         │
│  RetryBanner → 倒计时 + 取消按钮                                     │
│  MessageBubble → 编辑确认走 editMessage → resendFrom                  │
├──────────────────────────────────────────────────────────────────────┤
│  Layer 4: chatStore                                                  │
│  retryFromError(messageId) — 错误重试入口（替代 handleRetry + regenerate）│
│  editMessage(messageId, newContent) — 编辑重发入口（基于 resendFrom）   │
│  resendFrom(pivotTimestamp, content) — 共享截断+重发原语               │
│  executeRecoveryAction(actionCode, messageId) — 执行恢复动作          │
├──────────────────────────────────────────────────────────────────────┤
│  Layer 3: chatErrorRecovery.ts（新增）                                │
│  normalizeChatError(raw) → ChatErrorPresentation                     │
│  getRecoveryActions(category) → RecoveryAction[]                     │
│  executeAction(actionCode, ctx) → void                               │
├──────────────────────────────────────────────────────────────────────┤
│  Layer 2: chatOrchestrator.ts                                        │
│  ensureIdle() — 取消流式+等待取消完成+重置 phase（解决时序窗口）        │
│  handledTerminalEvents — 幂等 dedup                                  │
├──────────────────────────────────────────────────────────────────────┤
│  Layer 1: 后端 ProviderErrorPayload 扩展                              │
│  + http_status: Option<u16>                                          │
│  + provider: Option<String>                                          │
│  + provider_code: Option<String>                                     │
│  edit_message 不再改 timestamp（修复 2.7）                            │
└──────────────────────────────────────────────────────────────────────┘
```

**核心设计：`resendFrom` 原语**

四条路径的根本共性是「截断对话到某个时间点 → 重新发送一条消息」。提取为共享原语：

```typescript
// chatStore 内部方法（不暴露给外部）
async function resendFrom(
  pivotTimestamp: number,
  contentToSend: string,
): Promise<void> {
  // ① 确保不在流式中（取消+等待）
  await ensureIdle();

  // ② 截断 pivotTimestamp 之后的消息（保存快照用于回滚）
  const snapshot = get().messages;
  set(state => ({
    messages: state.messages.filter(m => m.timestamp < pivotTimestamp),
  }));

  // ③ 重新发送
  try {
    void get().sendMessage(contentToSend);
  } catch (e) {
    set({ messages: snapshot });  // 回滚
  }
}
```

> **I17b 扩展点**：I17b 时 `resendFrom` 签名扩展为 `resendFrom(pivotTimestamp, contentToSend, strategy?: TruncationStrategy)`，其中 `TruncationStrategy = { kind: "truncate" } | { kind: "branch"; branchPointMessageId: string }`。当 `strategy.kind === "branch"` 时，不删除后续消息，而是在 `branchPointMessageId` 处创建分支。详见 `I17b-conversation-branch-spec.md`。

各路径基于原语构建：

| 路径 | 原语调用方式 | 额外操作 |
|------|-------------|---------|
| `retryFromError(messageId)` | `resendFrom(failedMsg.timestamp, userMsg.content)` | 后端 `regenerateMessage` 删失败消息+后续 |
| `editMessage(messageId, newContent)` | `resendFrom(msg.timestamp, newContent)` | 后端 `editMessage` 改内容+删后续；前端更新 content |
| ~~`handleRetry`~~ | 废弃，由 `retryFromError` 替代 | — |

### 4.2 新增/修改文件清单

| 文件 | 操作 | 职责 |
|------|------|------|
| `src/chat/core/chatErrorRecovery.ts` | **新增** | 错误归一化 + 恢复动作映射 + 动作执行 |
| `src/chat/core/chatErrors.ts` | **修改** | `ChatError` 扩展字段 + `toChatError` 增强归一化 |
| `src/chat/blocks/ErrorBlock.tsx` | **修改** | 按恢复动作渲染多个按钮 |
| `src/chat/blocks/RetryBanner.tsx` | **新增** | 重试进度条 + 倒计时 + 取消按钮 |
| `src/chat/message/MessageBubble.tsx` | **修改** | `onRetry` 签名改为 `(messageId: string) => void`，使用新 RetryBanner |
| `src/chat/message/MessageList.tsx` | **修改** | `onRetryMessage` 签名改为 `(messageId: string) => void` |
| `src/chat/ChatView.tsx` | **修改** | `handleRetry` 改为 `retryFromError` 委托 |
| `src/stores/chatStore.ts` | **修改** | 新增 `retryFromError` + `resendFrom` 原语，重构 `editMessage` 基于原语 |
| `src/services/chatOrchestrator.ts` | **修改** | 新增 `ensureIdle()` + 错误事件幂等 dedup + `errorStream` 清理半截内容 |
| `crates/assistant-providers/src/base.rs` | **修改** | `ProviderErrorPayload` 扩展字段 |
| `crates/assistant-providers/src/client/error.rs` | **修改** | `classify_http_error` 返回 `ClassifiedError`，保留原始状态码 |
| `crates/assistant-providers/src/openai/responses_stream.rs` | **修改** | 4 处 sleep 改 `tokio::select!` + cancel；构造 `ProviderErrorPayload` 传入新字段 |
| `crates/assistant-providers/src/openai/chat.rs` | **修改** | 4 处 sleep 改 `tokio::select!` + cancel；构造 `ProviderErrorPayload` 传入新字段 |
| `crates/assistant-providers/src/gemini/request.rs` | **修改** | 4 处 sleep 改 `tokio::select!` + cancel；构造 `ProviderErrorPayload` 传入新字段 |
| `crates/assistant-providers/src/gemini/code_assist.rs` | **修改** | 4 处 sleep 改 `tokio::select!` + cancel；构造 `ProviderErrorPayload` 传入新字段 |
| `crates/assistant-providers/src/anthropic/request.rs` | **修改** | 4 处 sleep 改 `tokio::select!` + cancel；构造 `ProviderErrorPayload` 传入新字段 |
| `crates/assistant-providers/src/openai/discovery.rs` | **修改** | `classify_http_error` 调用点适配 `ClassifiedError` 返回值 |
| `crates/assistant-providers/src/gemini/discovery.rs` | **修改** | `classify_http_error` 调用点适配 `ClassifiedError` 返回值 |
| `crates/assistant-providers/src/anthropic/discovery.rs` | **修改** | `classify_http_error` 调用点适配 `ClassifiedError` 返回值 |
| `crates/assistant-storage/src/conversations.rs` | **修改** | `edit_message` 不再改 timestamp（修复 2.7） |
| `src-tauri/src/protocol/dto.rs` | **修改** | `ProviderErrorPayloadDto` 扩展字段 |

---

## 5. 详细设计

### 5.1 后端：`ProviderErrorPayload` 扩展

**`base.rs`**：

```rust
#[derive(Serialize, Debug, Clone)]
pub struct ProviderErrorPayload {
    pub code: ProviderErrorCode,
    pub message: String,
    pub retry_after_ms: Option<u64>,
    pub retryable: bool,
    pub http_status: Option<u16>,         // ← 新增
    pub provider: Option<String>,          // ← 新增
    pub provider_code: Option<String>,     // ← 新增
}
```

**`classify_http_error` 改为返回附加信息**：

```rust
pub(crate) struct ClassifiedError {
    pub error: ProviderError,
    pub http_status: u16,
    pub provider_code: Option<String>,
}

pub(crate) fn classify_http_error(status: reqwest::StatusCode, body: &str) -> ClassifiedError {
    let provider_code = extract_provider_code(body);  // ← 新增：提取 provider error code
    // ... 原有分类逻辑 ...
    ClassifiedError { error, http_status: status.as_u16(), provider_code }
}

fn extract_provider_code(body: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(body).ok()
        .and_then(|v| v.get("error").and_then(|e| e.get("code")).and_then(|c| c.as_str()).map(String::from))
}
```

**`ProviderErrorPayload::from` 扩展**：

```rust
impl ProviderErrorPayload {
    pub fn from_error(e: &ProviderError, http_status: Option<u16>, provider: Option<&str>, provider_code: Option<&str>) -> Self {
        Self {
            code: e.code(),
            message: e.to_string(),
            retry_after_ms: e.retry_after_ms(),
            retryable: e.is_retryable(),
            http_status,
            provider: provider.map(String::from),
            provider_code: provider_code.map(String::from),
        }
    }
}
```

**`dto.rs`** 同步扩展 `ProviderErrorPayloadDto`，新增 `http_status` / `provider` / `provider_code` 三个 `Option` 字段，`#[serde(default)]` 保证旧事件兼容。

### 5.2 后端：重试循环可取消

**所有 provider 的 `'retry` 循环**均需改造（5 个文件共 20 处）：

```rust
// 改前：
tokio::time::sleep(Duration::from_millis(delay)).await;

// 改后：
tokio::select! {
    _ = tokio::time::sleep(Duration::from_millis(delay)) => {},
    _ = cancel_token.cancelled() => {
        yield Err(ProviderError::Cancelled);
        return;
    }
};
```

**完整改造清单**：

| 文件 | 改造点 |
|------|--------|
| `openai/responses_stream.rs` | 连接错误重试(L218) + HTTP 错误重试(L235) + 首 token 超时重试(L273) + SSE 流错误重试(L301) |
| `openai/chat.rs` | 连接错误重试(L135) + HTTP 错误重试(L152) + 首 token 超时重试(L188) + SSE 流错误重试(L216) |
| `gemini/request.rs` | 连接错误重试(L138) + HTTP 错误重试(L156) + 首 token 超时重试(L190) + SSE 流错误重试(L218) |
| `gemini/code_assist.rs` | 连接错误重试(L148) + HTTP 错误重试(L165) + 首 token 超时重试(L196) + SSE 流错误重试(L224) |
| `anthropic/request.rs` | 连接错误重试(L134) + HTTP 错误重试(L151) + 首 token 超时重试(L185) + SSE 流错误重试(L213) |

**前提**：各 provider 的 stream 函数签名需新增 `cancel_token: CancellationToken` 参数，由 `chat_runtime::consume_stream` 传入（`consume_stream` 已持有 `cancel_token`，但当前未传递给底层 stream）。

同时 `RetryNotification` 新增 `error_code` 字段，让前端知道**为什么**在重试：

```rust
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct RetryNotification {
    pub attempt: u32,
    pub max_attempts: u32,
    pub delay_ms: u64,
    pub message: String,
    pub error_code: Option<ProviderErrorCode>,  // ← 新增
}
```

### 5.2b 后端：`edit_message` 不再改 timestamp

**问题**（2.7）：后端 `edit_message` 的 `UPDATE messages SET content = ?2, timestamp = ?3` 把 timestamp 改为 `now()`，但前端不更新 timestamp，导致前后端不一致。

**修复**：`edit_message` 的 UPDATE 不改 timestamp，仅改 content：

```rust
// 改前（conversations.rs）：
pub fn edit_message(conn: &Connection, message_id: &str, content: &str) -> Result<()> {
    let now = now_epoch_ms();
    conn.execute(
        "UPDATE messages SET content = ?1, timestamp = ?2 WHERE id = ?3",
        params![content, now, message_id],
    )?;
    // ...
}

// 改后：
pub fn edit_message(conn: &Connection, message_id: &str, content: &str) -> Result<()> {
    conn.execute(
        "UPDATE messages SET content = ?1 WHERE id = ?2",
        params![content, message_id],
    )?;
    // ...
}
```

**影响**：编辑消息后，该消息在对话中的排序位置不变（timestamp 不变），符合用户预期。如果需要记录编辑时间，可后续新增 `edited_at` 字段，但不在本次重构范围内。

### 5.3 前端：`ChatError` 扩展 + 归一化

**`chatErrors.ts`**：

```typescript
export interface ChatError {
  code: ChatErrorCode;
  message?: string;
  retryAfterMs: number | null;
  retryable: boolean;
  httpStatus?: number;          // ← 新增
  provider?: string;            // ← 新增
  providerCode?: string;        // ← 新增
  category?: ChatErrorCategory; // ← 新增：归一化语义类别
}

export enum ChatErrorCategory {
  Network = "network",
  Auth = "auth",
  RateLimit = "rate_limit",
  ContextOverflow = "context_overflow",
  Timeout = "timeout",
  ProviderQuota = "provider_quota",
  ProviderUnavailable = "provider_unavailable",
  InvalidRequest = "invalid_request",
  ContentPolicy = "content_policy",
  ModelError = "model_error",
  Unknown = "unknown",
}
```

**`toChatError` 增强**：当 `category` 未提供时，通过 `httpStatus` + `code` + `message` 做二次推断（对齐参考项目 Layer 2 + Layer 3 降级）。

### 5.4 前端：错误恢复动作映射

**`chatErrorRecovery.ts`（新增）**：

```typescript
export type RecoveryActionCode =
  | "retry"
  | "wait_and_retry"
  | "switch_model"
  | "compress_context"
  | "start_new_chat"
  | "open_model_settings"
  | "copy_diagnostics";

export interface RecoveryAction {
  code: RecoveryActionCode;
  labelKey: string;
  icon?: React.ComponentType;
}

export interface ChatErrorPresentation {
  category: ChatErrorCategory;
  titleKey: string;
  messageKey: string;
  severity: "warning" | "error";
  retryable: boolean;
  actions: RecoveryAction[];
  diagnostics: string;
}

const DEFAULT_ACTIONS: Record<ChatErrorCategory, RecoveryActionCode[]> = {
  [ChatErrorCategory.Network]:          ["retry", "switch_model", "copy_diagnostics"],
  [ChatErrorCategory.Auth]:            ["open_model_settings", "copy_diagnostics"],
  [ChatErrorCategory.RateLimit]:       ["wait_and_retry", "switch_model", "copy_diagnostics"],
  [ChatErrorCategory.ContextOverflow]: ["compress_context", "start_new_chat"],
  [ChatErrorCategory.Timeout]:         ["retry", "switch_model", "copy_diagnostics"],
  [ChatErrorCategory.ProviderQuota]:   ["open_model_settings", "switch_model", "copy_diagnostics"],
  [ChatErrorCategory.ProviderUnavailable]: ["wait_and_retry", "switch_model", "copy_diagnostics"],
  [ChatErrorCategory.InvalidRequest]:  ["copy_diagnostics"],
  [ChatErrorCategory.ContentPolicy]:   ["copy_diagnostics"],
  [ChatErrorCategory.ModelError]:      ["retry", "switch_model", "copy_diagnostics"],
  [ChatErrorCategory.Unknown]:         ["retry", "switch_model", "copy_diagnostics"],
};

export function getErrorPresentation(error: ChatError): ChatErrorPresentation;
export function getRecoveryActions(category: ChatErrorCategory): RecoveryAction[];
export function executeRecoveryAction(
  action: RecoveryActionCode,
  context: RecoveryContext,
): void | Promise<void>;
```

**`RecoveryContext`**：

```typescript
export interface RecoveryContext {
  conversationId: string;
  failedMessageId: string;
  error: ChatError;
  switchModel: (conversationId: string) => void;
  compressContext: (conversationId: string) => Promise<void>;
  openSettings: () => void;
  startNewChat: () => void;
  retryFromError: (messageId: string) => Promise<void>;
}
```

### 5.5 前端：共享原语 `resendFrom` + 统一重试入口 `retryFromError` + 重构 `editMessage`

**`chatOrchestrator.ts` — `ensureIdle()`**：

解决 R4 的 cancel→send 时序窗口问题（2.8）。将分散在 `editMessage` 中的取消逻辑收敛到 orchestrator：

```typescript
const ENSURE_IDLE_TIMEOUT_MS = 5000;  // 超时上限，防止 cancel 事件丢失导致无限等待

async function ensureIdle(): Promise<void> {
  if (!sendInProgress) return;

  // 取消当前流式
  if (deps.env.getCurrentConversationId()) {
    await deps.chatService.cancelChat(deps.env.getCurrentConversationId()!).catch(() => {});
  }

  // 等待 phase 回到 idle（cancel 事件的 finishReason 到达）
  const start = Date.now();
  await new Promise<void>((resolve) => {
    const check = () => {
      if (phase === "idle" || phase === "error") {
        // error 也算终止态，清掉 error phase
        if (phase === "error") phase = "idle";
        resolve();
      } else if (Date.now() - start > ENSURE_IDLE_TIMEOUT_MS) {
        // 超时强制重置（cancel 事件可能丢失：后端崩溃、网络断开）
        console.warn("[orchestrator] ensureIdle timeout, force resetting phase");
        phase = "idle";
        resolve();
      } else {
        setTimeout(check, 50);
      }
    };
    setTimeout(check, 50);
  });

  // 重置流式状态
  deps.state.setIsStreaming(false);
  deps.state.setStreamingContent("");
  deps.state.setIsReasoningStreaming(false);
}
```

> **设计决策**：超时上限 5s。正常 cancel 事件在 <1s 内到达；5s 覆盖极端网络延迟。超时后强制重置 phase 比"永远卡住"更安全——用户可以继续操作，最坏情况是残留的流式内容被清空。

**`chatStore.ts` — `resendFrom` 原语**：

```typescript
// I17 仅实现 truncate 策略；I17b 扩展为 TruncationStrategy 联合类型
// I17b 扩展点：type TruncationStrategy = { kind: "truncate" } | { kind: "branch"; branchPointMessageId: string };

// 内部方法，不暴露到 store 接口
async function resendFrom(
  pivotTimestamp: number,
  contentToSend: string,
): Promise<void> {
  // ① 确保不在流式中
  await orchestrator.ensureIdle();

  // ② 截断：保存快照用于回滚，然后删除 pivotTimestamp 之后的消息
  const snapshot = get().messages;
  set(state => ({
    messages: state.messages.filter(m => m.timestamp < pivotTimestamp),
  }));

  // ③ 重新发送（若后端持久化失败，回滚快照）
  try {
    void get().sendMessage(contentToSend);
  } catch (e) {
    console.error("[chatStore] resendFrom send failed, rolling back:", e);
    set({ messages: snapshot });
  }
}
```

> **设计决策**：I17 阶段 `resendFrom` **不包含** `TruncationStrategy` 参数，仅实现截断。I17b 时扩展签名增加 `strategy` 参数和 branch 分支逻辑。避免 I17 中出现 dead code，也防止误传 `{ kind: "branch" }` 时执行截断而非分支。

**`chatStore.ts` — `retryFromError`**：

```typescript
retryFromError: async (messageId: string) => {
  const state = get();
  const msgIndex = state.messages.findIndex((m) => m.id === messageId);
  if (msgIndex === -1 || state.messages[msgIndex].role !== "assistant") return;

  // 用数组索引回溯找前一条 user 消息（比 timestamp 比较更可靠）
  // 不用 timestamp 比较：避免连发 user 消息时取错、I17b 分支下 timestamp 非单调
  const msg = state.messages[msgIndex];
  const userMsg = [...state.messages]
    .slice(0, msgIndex)
    .reverse()
    .find((m) => m.role === "user");
  if (!userMsg) return;

  // 后端同步删除失败消息+后续
  try {
    await persistQueue.enqueue(async () => {
      await chatService.regenerateMessage(
        state.currentConversationId!,
        messageId,
      );
    });
  } catch (e) {
    console.error("[chatStore] retryFromError persist failed:", e);
  }

  // 基于原语重发
  await resendFrom(msg.timestamp, userMsg.content);
},
```

> **设计决策**：用 `slice(0, msgIndex).reverse().find(role === "user")` 而非 `filter(timestamp < msg.timestamp).pop()` 回溯 user 消息。原因：(1) timestamp 比较在连发 user 消息时可能取错；(2) I17b 分支模式下 timestamp 不保证全局单调；(3) 数组索引回溯在当前线性消息模型下语义更精确。I17b 分支模式下需改为沿 `parentId` 链回溯。

**`chatStore.ts` — 重构 `editMessage`**：

```typescript
editMessage: async (messageId: string, newContent: string) => {
  const state = get();
  const msg = state.messages.find((m) => m.id === messageId);
  if (!msg) return;

  // 后端持久化：改内容 + 删后续（不再改 timestamp，修复 2.7）
  await persistQueue.enqueue(async () => {
    await chatService.editMessage(messageId, newContent);
  });

  // 前端更新内容（timestamp 不变）
  set(state => ({
    messages: state.messages.map(m =>
      m.id === messageId ? { ...m, content: newContent } : m
    ),
  }));

  // 如果编辑的是 user 消息，基于原语重发
  if (msg.role === "user") {
    await resendFrom(msg.timestamp, newContent);
  }
},
```

**对比改前改后**：

| 维度 | 改前 `editMessage` | 改后 `editMessage` |
|------|-------------------|-------------------|
| 取消流式 | 自己 cancel + set | `resendFrom` → `ensureIdle()` |
| 前端截断 | 自己 filter | `resendFrom` 统一 filter |
| 后端 editMessage | 先调后端再截断 | 先调后端再 `resendFrom` |
| timestamp | 前端不改，后端改（不一致） | 前端不改，后端也不改（修复 2.7） |
| cancel→send 时序 | 有窗口（2.8） | `ensureIdle()` 等待 phase 回归 |

**`ChatView.tsx`**：

```typescript
// 改前：
const handleRetry = useCallback(() => {
  const lastUserMsg = [...chat.messages]
    .reverse()
    .find((m) => m.role === "user");
  if (lastUserMsg) {
    void chat.sendMessage(lastUserMsg.content);
  }
}, [chat]);

// 改后：
const handleRetry = useCallback((messageId: string) => {
  void chat.retryFromError(messageId);
}, [chat]);
```

### 5.6 前端：`ErrorBlock` 多动作渲染

**`ErrorBlock.tsx`**：

```typescript
interface ErrorBlockProps {
  error: ChatError;
  messageId: string;
  onAction?: (actionCode: RecoveryActionCode, messageId: string) => void;
}

export const ErrorBlock: React.FC<ErrorBlockProps> = ({ error, messageId, onAction }) => {
  const presentation = getErrorPresentation(error);

  return (
    <div className={css.root}>
      <div className={css.header}>
        <span className={css.icon}>{presentation.severity === "error" ? "🔴" : "🟡"}</span>
        <span className={css.title}>{t(presentation.titleKey)}</span>
      </div>
      <div className={css.suggestion}>{t(presentation.messageKey)}</div>
      {error.retryAfterMs && error.retryAfterMs > 0 && (
        <div className={css.retryHint}>{formatRetryAfter(error.retryAfterMs)}</div>
      )}
      <div className={css.actions}>
        {presentation.actions.map((action) => (
          <button
            key={action.code}
            className={css.actionBtn}
            onClick={() => onAction?.(action.code, messageId)}
            type="button"
          >
            {action.icon && <action.icon style={{ fontSize: 12, marginRight: 4 }} />}
            {t(action.labelKey)}
          </button>
        ))}
      </div>
    </div>
  );
};
```

### 5.7 前端：`RetryBanner` 倒计时 + 取消

**`RetryBanner.tsx`（新增）**：

```typescript
interface RetryBannerProps {
  retryStatus: RetryNotification;
  onCancel: () => void;
}

export const RetryBanner: React.FC<RetryBannerProps> = ({ retryStatus, onCancel }) => {
  const [countdown, setCountdown] = useState(Math.ceil(retryStatus.delayMs / 1000));

  useEffect(() => {
    const timer = setInterval(() => {
      setCountdown((c) => Math.max(0, c - 1));
    }, 1000);
    return () => clearInterval(timer);
  }, []);

  return (
    <div className={css.banner}>
      <ReloadOutlined spin style={{ fontSize: 12, marginRight: 6 }} />
      <span>
        {t("chat.retry.retrying", {
          attempt: retryStatus.attempt + 1,
          max: retryStatus.maxAttempts,
        })}
      </span>
      {countdown > 0 && (
        <span className={css.countdown}>
          {t("chat.retry.countdown", { seconds: countdown })}
        </span>
      )}
      <button className={css.cancelBtn} onClick={onCancel} type="button">
        {t("common.actions.cancel")}
      </button>
    </div>
  );
};
```

### 5.8 前端：错误事件幂等 dedup

**`chatOrchestrator.ts`**：

```typescript
export function createChatOrchestrator(deps: ChatOrchestratorDeps): ChatOrchestrator {
  let sendInProgress = false;
  let phase: ChatPhase = "idle";
  const handledTerminalEvents = new Set<string>();  // ← 新增

  function handleDeltaEvent(payload: ChatDeltaPayload) {
    const activeAttemptId = deps.env.getActiveAttemptId();
    if (activeAttemptId === null || payload.attemptId !== activeAttemptId) return;

    if (payload.error) {
      // 幂等：同一 attempt 的 error 只处理一次
      const terminalKey = `${payload.attemptId}:error`;
      if (handledTerminalEvents.has(terminalKey)) return;
      handledTerminalEvents.add(terminalKey);

      setPhase("error");
      deps.state.errorStream(toChatError(payload.error));
      return;
    }

    if (payload.finishReason) {
      // 幂等：同一 attempt 的 finish 只处理一次
      const terminalKey = `${payload.attemptId}:finish`;
      if (handledTerminalEvents.has(terminalKey)) return;
      handledTerminalEvents.add(terminalKey);

      setPhase("idle");
      deps.state.setRetryStatus(null);
      deps.state.finishStream(payload.usage ?? undefined);
      return;
    }

    // ... 其余事件处理不变 ...
  }

  // send() 开始时清空 dedup set
  async function send(content: string) {
    handledTerminalEvents.clear();  // ← 新增
    // ... 原有逻辑 ...
  }

  // ...
}
```

### 5.9 前端：`errorStream` 清理半截内容

当流式传输中途出错时，当前实现保留半截文本 + 错误标记。改为：

**`chatStore.ts` 的 `errorStream` action**：

```typescript
errorStream: (error: ChatError) => {
  set((state) => ({
    isStreaming: false,
    isReasoningStreaming: false,
    streamingContent: "",
    messages: state.messages.map((m) =>
      m.id === state.streamingMessageId
        ? { ...m, error, content: state.streamingContent || m.content }
        : m,
    ),
  }));
},
```

保留已流式内容（用户可能想看部分结果），但标记 error。

---

## 6. 实施步骤

### Step 1：后端 `ProviderErrorPayload` 扩展

**目标**：透传 `http_status` / `provider` / `provider_code` 到前端。

**变更**：
- `base.rs`：`ProviderErrorPayload` 新增 3 个 `Option` 字段
- `client/error.rs`：`classify_http_error` 返回 `ClassifiedError`，保留原始 `http_status` 和 `provider_code`
- **12 处 `classify_http_error` 调用点适配**：
  - `openai/responses_stream.rs`（1 处）
  - `openai/chat.rs`（1 处）
  - `openai/discovery.rs`（1 处）
  - `gemini/request.rs`（1 处）
  - `gemini/code_assist.rs`（1 处）
  - `gemini/discovery.rs`（1 处）
  - `anthropic/request.rs`（1 处）
  - `anthropic/discovery.rs`（1 处）
- **5 个 provider stream 文件**：构造 `ProviderErrorPayload` 时传入 `http_status` / `provider` / `provider_code`
- `dto.rs`：`ProviderErrorPayloadDto` 同步扩展，`#[serde(default)]` 兼容旧事件

**验证**：后端单元测试 `classify_http_error` 仍通过；前端收到新字段后 `toChatError` 正确提取。

### Step 2：后端重试循环可取消 + `edit_message` 不改 timestamp

**目标**：所有 provider 的 `'retry` 循环 sleep 改为 `tokio::select!` + cancel_token；`edit_message` 不再改 timestamp（修复 2.7）。

**变更**：
- **5 个 provider 文件共 20 处** `tokio::time::sleep` 改为 `tokio::select!`：
  - `openai/responses_stream.rs`（4 处）
  - `openai/chat.rs`（4 处）
  - `gemini/request.rs`（4 处）
  - `gemini/code_assist.rs`（4 处）
  - `anthropic/request.rs`（4 处）
- 各 provider stream 函数签名新增 `cancel_token: CancellationToken` 参数
- `chat_runtime.rs`：`consume_stream` 将 `cancel_token` 传递给底层 stream
- `chat_delta.rs`：`RetryNotification` 新增 `error_code: Option<ProviderErrorCode>`
- `conversations.rs`：`edit_message` 的 UPDATE 去掉 `timestamp = ?3`，只改 `content`

**验证**：重试期间点"停止生成"能立即中断；编辑消息后 reload 对话，消息排序不变。

### Step 3：前端 `ChatError` 扩展 + 归一化

**目标**：`ChatError` 新增 `httpStatus` / `provider` / `providerCode` / `category` 字段，`toChatError` 增强归一化。

**变更**：
- `chatErrors.ts`：新增 `ChatErrorCategory` 枚举，`ChatError` 扩展字段，`toChatError` 增加二次推断逻辑
- `chatTypes.ts`：`RetryNotification` 新增 `errorCode` 字段

**验证**：后端传来的新字段正确映射；旧事件（无新字段）退化为 `category: undefined`，不影响现有行为。

### Step 4：前端 `chatErrorRecovery.ts` 新增

**目标**：错误类别感知的恢复动作映射。

**变更**：
- 新增 `chatErrorRecovery.ts`：`getErrorPresentation` + `getRecoveryActions` + `executeRecoveryAction`
- 新增 i18n key（共 29 个）：

| 类别 | titleKey | messageKey | 合计 |
|------|----------|------------|------|
| Network | `chat.error.category.network.title` | `chat.error.category.network.message` | 2 |
| Auth | `chat.error.category.auth.title` | `chat.error.category.auth.message` | 2 |
| RateLimit | `chat.error.category.rate_limit.title` | `chat.error.category.rate_limit.message` | 2 |
| ContextOverflow | `chat.error.category.context_overflow.title` | `chat.error.category.context_overflow.message` | 2 |
| Timeout | `chat.error.category.timeout.title` | `chat.error.category.timeout.message` | 2 |
| ProviderQuota | `chat.error.category.provider_quota.title` | `chat.error.category.provider_quota.message` | 2 |
| ProviderUnavailable | `chat.error.category.provider_unavailable.title` | `chat.error.category.provider_unavailable.message` | 2 |
| InvalidRequest | `chat.error.category.invalid_request.title` | `chat.error.category.invalid_request.message` | 2 |
| ContentPolicy | `chat.error.category.content_policy.title` | `chat.error.category.content_policy.message` | 2 |
| ModelError | `chat.error.category.model_error.title` | `chat.error.category.model_error.message` | 2 |
| Unknown | `chat.error.category.unknown.title` | `chat.error.category.unknown.message` | 2 |
| **动作 labelKey** | | | |
| retry | `chat.error.action.retry` | | 1 |
| wait_and_retry | `chat.error.action.wait_and_retry` | | 1 |
| switch_model | `chat.error.action.switch_model` | | 1 |
| compress_context | `chat.error.action.compress_context` | | 1 |
| start_new_chat | `chat.error.action.start_new_chat` | | 1 |
| open_model_settings | `chat.error.action.open_model_settings` | | 1 |
| copy_diagnostics | `chat.error.action.copy_diagnostics` | | 1 |
| **合计** | | | **29** |

> **前置检查**：`compress_context` 和 `start_new_chat` 动作需确认当前代码库中是否有对应实现。若无实现，`ContextOverflow` 的恢复动作应降级为 `["copy_diagnostics"]`（比空操作更安全），并在注释中标记为 TODO。

### Step 5：前端 `ensureIdle` + `resendFrom` 原语 + 统一重试入口 `retryFromError`

**目标**：`handleRetry` 改为 `retryFromError(messageId)`，签名传递出错消息 ID；提取 `resendFrom` 共享原语。

**变更**：
- `chatOrchestrator.ts`：新增 `ensureIdle()` 方法
- `chatStore.ts`：新增 `resendFrom` 内部方法 + `retryFromError` action
- `ChatView.tsx`：`handleRetry` 改为 `(messageId: string) => void`
- `MessageList.tsx`：`onRetryMessage` 签名改为 `(messageId: string) => void`
- `MessageBubble.tsx`：`onRetry` 签名改为 `(messageId: string) => void`
- `ErrorBlock.tsx`：`onRetry` 签名改为 `(messageId: string) => void`

**验证**：点击任意消息的错误重试按钮，重发的是该消息对应的 user 消息，而非最后一条。

### Step 6：前端重构 `editMessage` 基于 `resendFrom`

**目标**：`editMessage` 的重发逻辑走 `resendFrom` 原语，消除独立的 cancel+filter+send 链路。

**变更**：
- `chatStore.ts`：`editMessage` 改为先调后端 `editMessage`（改内容+删后续），再 `resendFrom(msg.timestamp, newContent)`

**验证**：编辑 user 消息后重发，流式中编辑能正确取消+重发；前后端 timestamp 一致。

### Step 7：前端 `ErrorBlock` 多动作渲染

**目标**：`ErrorBlock` 按恢复动作渲染多个按钮。

**变更**：
- `ErrorBlock.tsx`：使用 `getErrorPresentation` 渲染动作列表
- `MessageBubble.tsx`：传递 `onAction` 回调
- `MessageList.tsx`：传递 `onAction` 回调
- `ChatView.tsx`：实现 `handleRecoveryAction`，按 `actionCode` 分派到不同操作

**验证**：`ContextOverflow` 错误显示"压缩上下文"+"新建对话"按钮；`AuthFailed` 错误显示"打开设置"按钮。

### Step 8：前端 `RetryBanner` 倒计时 + 取消

**目标**：重试进度条增加倒计时和取消按钮。

**变更**：
- 新增 `RetryBanner.tsx`
- `MessageBubble.tsx`：用 `RetryBanner` 替换内联 banner

**验证**：后端重试时，前端显示倒计时；点取消能中断重试。

### Step 9：前端错误事件幂等 dedup

**目标**：`handleDeltaEvent` 中 error 和 finish 事件去重。

**变更**：
- `chatOrchestrator.ts`：新增 `handledTerminalEvents` Set，error/finish 事件按 `attemptId` 去重

**验证**：后端发送重复 error 事件，前端只处理一次。

---

## 7. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 后端 DTO 扩展破坏旧版前端 | `#[serde(default)]` 保证旧字段缺失时退化为 `None`，前端 `toChatError` 对 `undefined` 做降级处理 |
| `retryFromError` 与 `editMessage` 并发 | `resendFrom` 开头调 `ensureIdle()`，orchestrator 层 `sendInProgress` 互斥 |
| `tokio::select!` 改变重试时序 | select 只在 sleep 期间检查取消，不影响正常重试延迟计算；单元测试验证 |
| 多动作按钮 UI 拥挤 | 动作列表最多 3 项，按优先级排序；小屏幕下折叠为下拉菜单 |
| `chat-regenerate` 事件未监听 | Step 5 中 `retryFromError` 不依赖后端 `chat-regenerate` 事件做前端删除（前端已自行 filter），后端事件仅做 DB 同步；后续可考虑监听该事件做显式同步 |
| `editMessage` 不改 timestamp 影响排序 | 编辑不应改变消息位置，这是正确行为；如需记录编辑时间可后续加 `edited_at` 字段 |
| `ensureIdle()` 轮询等待 phase | 50ms 轮询间隔 + **5s 超时上限**（cancel 事件丢失时强制重置）；后续可优化为 Promise 回调 |
| `resendFrom` 截断后后端异步失败 | 截断前保存 `messages` 快照，`sendMessage` 失败时回滚快照 + toast 提示 |
| `retryFromError` 找 user 消息逻辑 | 用数组索引回溯（`slice(0, msgIndex).reverse().find`）而非 timestamp 比较，避免连发 user 消息和 I17b 分支下的歧义 |
| `compress_context` / `start_new_chat` 无实现 | Step 4 前置检查：若无实现则降级恢复动作为 `["copy_diagnostics"]`，避免空操作按钮 |
| `classify_http_error` 返回值变更影响 12 处调用 | Step 1 逐文件适配，每处调用从 `classify_http_error(status, &text)` 改为 `classify_http_error(status, &text).error`；discovery 模块需同步 |
| `cancel_token` 未传递给底层 stream | Step 2 需同步修改 `chat_runtime::consume_stream` 及各 provider stream 函数签名，将 `cancel_token` 透传到底层 |
| `handledTerminalEvents` 内存泄漏 | 每次 `send()` 时 `clear()`；对话切换时也应清理；Set 内 key 为短字符串（`${attemptId}:error`），实际内存占用可忽略 |

---

## 8. 验收标准

- [x] **R1 消除**：不存在"找最后一条 user 消息"的重试逻辑，所有重试都通过 `retryFromError(messageId)` 精确关联
- [x] **R4 统一**：`editMessage` 的重发走 `resendFrom` 原语，不再有独立的 cancel+filter+send 链路
- [x] **timestamp 一致**：编辑消息后，前后端 timestamp 一致（后端不改 timestamp）
- [x] **cancel→send 时序安全**：`ensureIdle()` 等待 phase 回归后再 sendMessage，无时序窗口
- [x] **ensureIdle 超时安全**：cancel 事件丢失时 `ensureIdle()` 在 5s 内强制返回，不无限等待
- [x] **错误类别感知**：`ContextOverflow` 错误不显示"重试"按钮，显示"压缩上下文"+"新建对话"（或降级为"复制诊断"）
- [x] **后端信息透传**：前端 `ChatError.httpStatus` / `provider` / `providerCode` 非空（当后端提供时）
- [x] **重试可取消**：后端重试 sleep 期间点"停止生成"能立即中断（5 个 provider 共 20 处均已改造）
- [x] **幂等安全**：后端发送重复 error 事件，前端 `messages` 状态不变（不重复设置 error）
- [x] **RetryBanner 倒计时**：重试进度条显示秒级倒计时
- [x] **DTO 防御性反序列化**：`ProviderErrorPayloadDto` / `RetryNotificationDto` 新字段均标注 `#[serde(default)]`，缺失时退化为 `None`；前端 `isChatError` 对新字段宽松校验，`inferCategory` 降级为 `Unknown`（注：错误/重试数据不持久化，仅实时事件，此条为反序列化防御而非数据迁移）
- [x] **resendFrom 回滚**：`sendMessage` 失败时，截断前的消息快照恢复，用户不丢失消息
- [x] **retryFromError 精确定位**：用数组索引回溯找 user 消息，连发 user 消息场景下重试正确
- [x] **I17b 扩展点**：`resendFrom` 签名可扩展为接受 `TruncationStrategy` 参数（I17 中不包含该参数，避免 dead code）

---

## 9. I17b 扩展：对话分支（Conversation Branch）

I17 完成后，`resendFrom` 原语的 `TruncationStrategy` 策略接口为分支设计提供了扩展点。分支设计的完整规格见 **`I17b-conversation-branch-spec.md`**。

**I17 与 I17b 的边界**：

| 维度 | I17（本次） | I17b（后续） |
|------|------------|-------------|
| 核心目标 | 修复重发语义错误 + 错误类别感知 | 非破坏性重发 + 分支切换 |
| `resendFrom` 签名 | `(pivotTimestamp, contentToSend)` | `(pivotTimestamp, contentToSend, strategy?)` |
| `TruncationStrategy` | 不存在（I17 无 dead code） | `{ kind: "truncate" } \| { kind: "branch" }` |
| 消息数据模型 | 线性数组（不变） | 树结构（`parentId` + `siblingIndex`） |
| UI | 无分支选择器 | `BranchPicker` 组件 |
| 后端 | 不改消息表结构 | 消息表加 `sibling_index` + `active_branch_id` |
| 依赖关系 | 无 | 依赖 I17 完成 |
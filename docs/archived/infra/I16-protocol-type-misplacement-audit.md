# I16 协议类型错放审查

> **触发原因**：发送图片给模型时报 `[图片已省略，当前模型不支持图片]`，根因为 `ModelConfig` 的 serde 配置与数据库存储格式不一致，导致 `support_vision` 反序列化失败默认为 `false`
> **审查日期**：2026-10-24
> **依据规范**：[I16-frontend-backend-protocol-spec.md](../chat/I16-frontend-backend-protocol-spec.md)

---

## 1. 架构约定（I16）

> `src-tauri/src/protocol/` 模块是**统一协议定义点**，所有 event payload + command 返回值 struct 必须在此定义。

子 crate（`assistant-providers`、`assistant-storage`）只定义**内部业务类型**，不应直接参与前后端 JSON wire format。

---

## 2. 已发现的 bug 及修复

### 2.1 `ModelConfig` serde 不一致 → `support_vision` 反序列化失败

| 位置 | 类型 | `rename_all` | 实际 JSON 格式 |
|---|---|---|---|
| `assistant-providers/src/base.rs` | `ModelConfig` | 无（snake_case） | camelCase（来自前端） |
| `protocol/dto/settings.rs` | `ModelConfigDto` | `camelCase` | camelCase ✅ |

**问题**：`chat.rs:258` 用 `serde_json::from_str::<Vec<ModelConfig>>(&provider_row.models_json)` 反序列化数据库中前端存入的 camelCase JSON，`supportVision` 无法映射到 `support_vision`，走 `#[serde(default)]` 变为 `false`。

**影响**：所有 vision 模型被判定为非 vision，图片被 `project_images_for_text_model` 替换为占位文本。

**修复**：给 `ModelConfig` 加 `#[serde(rename_all = "camelCase")]`（已完成）。

### 2.2 `ImageThumbnail.tsx` 路径分隔符混用 → asset 协议 403

| 拼接方式 | Windows 结果 | scope 匹配 |
|---|---|---|
| `(await appDataDir()) + "attachments"` + `"/" + id` | `...\attachments/e9dd...jpg` | ❌ 混合分隔符 |
| `join(await appDataDir(), "attachments", id)` | `...\attachments\e9dd...jpg` | ✅ 统一反斜杠 |

**修复**：改用 `@tauri-apps/api/path` 的 `join()` 函数（已完成）。

---

## 3. 协议类型错放清单

### 🔴 P0 — 直接参与前后端 wire format，定义在子 crate 中

#### 3.1 `ModelConfig` — `assistant-providers/src/base.rs:168`

```
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]   ← 刚加的临时修复
pub struct ModelConfig { ... }
```

- **职责 1（内部业务）**：discovery 模块构造模型列表、vision 解析
- **职责 2（前后端通信）**：`models_json` 存入数据库、`chat.rs` 反序列化判断 `support_vision`
- **问题**：职责 2 应走 `ModelConfigDto`，但 `chat.rs` 和 `resolve_context_window_from_stored` 直接用 `ModelConfig`
- **涉及位置**：
  - `src/cmd/chat.rs:258` — `serde_json::from_str::<Vec<ModelConfig>>(&provider_row.models_json)`
  - `assistant-providers/src/types/config.rs:105` — `serde_json::from_str::<Vec<ModelConfig>>(models_json)`
- **正确做法**：`chat.rs` 和 `resolve_context_window_from_stored` 应改为用 `ModelConfigDto` 反序列化，`ModelConfig` 回退为内部类型（去掉 `rename_all`）

#### 3.2 `ContentBlock` / `ImageAttachmentRef` / `FileAttachmentRef` — `assistant-providers/src/types/message.rs:39-112`

```
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock { ... }

pub struct ImageAttachmentRef {
    #[serde(rename = "attachmentId")]
    pub attachment_id: String,
    ...
}
```

- **前端对应**：`contentBlockTypes.ts` 的 `ContentBlock` / `ImageAttachmentRef` / `FileAttachmentRef`
- **问题**：这些类型直接参与消息 `content` JSON 字段的序列化/反序列化（前端构造、后端解析），但定义在子 crate，`protocol/dto` 层无对应定义
- **涉及位置**：
  - `message.rs:parse_content()` — `serde_json::from_str::<Vec<ContentBlock>>(content)` 解析数据库中的消息内容
  - `multimodal.rs` — `rehydrate_images` / `project_images_for_text_model` 修改 ContentBlock
  - 前端 `contentBlockUtils.ts` — 构造 ContentBlock JSON
- **正确做法**：在 `protocol/dto` 层定义 `ContentBlockDto` / `ImageAttachmentRefDto`，子 crate 的 `ContentBlock` 只保留内部业务用途

#### 3.3 `ProviderErrorCode` — `assistant-providers/src/base.rs:18`

```
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ProviderErrorCode {
    ApiKeyMissing, AuthFailed, RateLimited, ...
}
```

- **前端对应**：`chatErrors.ts` 的 `ChatErrorCode`（值完全一致：`"ApiKeyMissing"`, `"RateLimited"` 等）
- **问题**：dto 层直接 `use assistant_providers::base::ProviderErrorCode`，没有自己的 DTO 版本。如果子 crate 改了枚举变体名，前端不会编译报错
- **涉及位置**：
  - `protocol/dto/chat.rs:1` — `use assistant_providers::base::ProviderErrorCode`
  - `ProviderErrorPayloadDto.code` 字段直接用此类型
- **正确做法**：在 `protocol/dto` 层定义 `ProviderErrorCodeDto`，与前端 `ChatErrorCode` 值一一对应，加 `deny_unknown_fields` 级保护

#### 3.4 `ProviderErrorPayload` — `assistant-providers/src/base.rs:108`

```
#[derive(Serialize, Debug, Clone)]   // 无 Deserialize
pub struct ProviderErrorPayload {
    pub code: ProviderErrorCode,
    pub message: String,
    pub retry_after_ms: Option<u64>,   // snake_case
    pub retryable: bool,
    pub http_status: Option<u16>,      // snake_case
    pub provider: Option<String>,
    pub provider_code: Option<String>, // snake_case
}
```

- **dto 层映射**：`ProviderErrorPayloadDto`（有 `rename_all="camelCase"` + `From<ProviderErrorPayload>`）
- **问题**：源类型 snake_case 序列化，dto 层 camelCase 序列化。如果有人直接序列化 `ProviderErrorPayload` 而非 `ProviderErrorPayloadDto`，前端收到 snake_case 字段无法解析
- **正确做法**：`ProviderErrorPayload` 只保留内部用途，所有前端通信必须走 `ProviderErrorPayloadDto`

### 🟡 P1 — dto 层已有映射，但源类型 serde 配置与 dto 不一致

#### 3.5 `TokenUsage` — `assistant-providers/src/types/chat_delta.rs:113`

| 属性 | `TokenUsage`（子 crate） | `TokenUsageDto`（dto 层） |
|---|---|---|
| `rename_all` | 无（snake_case） | `camelCase` |
| `deny_unknown_fields` | 无 | 有 |
| 映射 | — | `From<TokenUsage>` ✅ |

- **前端对应**：`chatTypes.ts` 的 `TokenUsage`（camelCase：`promptTokens`, `completionTokens` 等）
- **风险**：如果有人直接序列化 `TokenUsage` 而非 `TokenUsageDto`，前端收到 `prompt_tokens` 而非 `promptTokens`

#### 3.6 `RetryNotification` — `assistant-providers/src/types/chat_delta.rs:7`

| 属性 | `RetryNotification`（子 crate） | `RetryNotificationDto`（dto 层） |
|---|---|---|
| `rename_all` | 无（snake_case） | `camelCase` |
| `deny_unknown_fields` | 无 | 有 |
| 映射 | — | `From<RetryNotification>` ✅ |

- **前端对应**：`chatTypes.ts` 的 `RetryNotification`（camelCase：`maxAttempts`, `delayMs`, `errorCode`）
- **风险**：同 3.5

#### 3.7 `ChatDelta` — `assistant-providers/src/types/chat_delta.rs:20`

```
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ChatDelta {
    pub seq: Option<u32>,
    pub delta_text: Option<String>,      // snake_case
    pub delta_reasoning: Option<String>,  // snake_case
    pub usage: Option<TokenUsage>,       // 嵌套 snake_case
    pub delta_retry: Option<RetryNotification>,  // 嵌套 snake_case
    ...
}
```

- **dto 层映射**：`ChatDeltaPayloadDto`（有 `rename_all="camelCase"`）
- **风险**：`ChatDelta` 在流式处理内部使用，如果直接序列化给前端会出问题。当前通过 `ChatDeltaPayloadDto` 映射是安全的，但嵌套的 `TokenUsage` / `RetryNotification` 仍存在 3.5/3.6 的风险

---

## 4. 修复方案

### 4.1 紧急修复（✅ 已完成）

| 修复项 | 改动 |
|---|---|
| `ImageThumbnail.tsx` 路径拼接改用 `join()` | `chat/message/ImageThumbnail.tsx` |

### 4.2 架构修复（✅ 已完成）

按 I16 规范，所有参与前后端通信的类型应在 `protocol/dto` 层定义，子 crate 只保留内部业务类型。

#### Step 1：`ModelConfig` / `ModelConfigDto` 统一 ✅

- `chat.rs:258` 改为 `serde_json::from_str::<Vec<ModelConfigDto>>(&provider_row.models_json)`
- `chat_utils.rs` 和 `conversation_utils.rs` 改为用 `ModelConfigDto` 反序列化，调用 `base::resolve_context_window(model_id, stored, None)`
- `ModelConfig` 去掉 `rename_all="camelCase"`，回退为内部类型（snake_case）
- `resolve_context_window_from_stored` 不再从 `base.rs` re-export（仅子 crate 内部使用）

#### Step 2：`ContentBlock` / `ImageAttachmentRef` 积入 dto 层 ✅

- 在 `protocol/dto/chat.rs` 新增 `ContentBlockDto` / `ImageAttachmentRefDto` / `FileAttachmentRefDto`
- 添加 `From<ContentBlock>` / `From<ImageAttachmentRef>` / `From<FileAttachmentRef>` 映射
- `ContentBlockDto` 不含 `cached_data` / `file_path`（这些是内部字段，不应暴露给前端）
- 子 crate 的 `ContentBlock` 保留为内部类型，所有前端通信走 DTO

#### Step 3：`ProviderErrorCode` 积入 dto 层 ✅

- 在 `protocol/dto/chat.rs` 新增 `ProviderErrorCodeDto`（`rename_all="PascalCase"` + `deny_unknown_fields`）
- `ProviderErrorPayloadDto.code` 改为 `ProviderErrorCodeDto`
- `RetryNotificationDto.error_code` 改为 `Option<ProviderErrorCodeDto>`
- 添加 `From<ProviderErrorCode>` 映射
- 测试代码更新为使用 `ProviderErrorCodeDto`

#### Step 4：子 crate 类型保持 snake_case ✅

- `ModelConfig` 已回退为 snake_case（去掉 `rename_all="camelCase"`）
- `TokenUsage` / `RetryNotification` / `ChatDelta` 等内部类型保持 snake_case
- 所有前端通信统一走 dto 层的 camelCase 版本

---

## 5. 风险矩阵

| 类型 | 修复前风险 | 修复后状态 |
|---|---|---|
| `ModelConfig` | 🔴 已导致 bug（`support_vision` 反序列化失败） | ✅ 架构修复：`chat.rs` 用 `ModelConfigDto`，`ModelConfig` 回退为内部类型 |
| `ContentBlock` 系列 | 🟡 协议无编译时保护 | ✅ dto 层新增 `ContentBlockDto`，前端通信走 DTO |
| `ProviderErrorCode` | 🟡 前端硬编码字符串匹配 | ✅ dto 层新增 `ProviderErrorCodeDto`，`deny_unknown_fields` 保护 |
| `ProviderErrorPayload` | 🟡 可能直接序列化源类型 | ✅ dto 层 `ProviderErrorPayloadDto.code` 改用 `ProviderErrorCodeDto` |
| `TokenUsage` / `RetryNotification` | 🟡 同上 | ✅ 已有 `From` 映射，`RetryNotificationDto.error_code` 改用 `ProviderErrorCodeDto` |
| `ChatDelta` | 🟡 内部使用 | ✅ 通过 DTO 映射安全 |
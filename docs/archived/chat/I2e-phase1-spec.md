# I2e Phase 1 — 前端可用 + 后端 AttachmentStore 规格说明

> **来源规格**：[I2e-multimodal-attachments-spec.md](./I2e-multimodal-attachments-spec.md)
> **前置**：I2-History ✅（消息存储、分支、重发）
> **目标**：用户可粘贴/拖拽/选择图片 → 发送 → 消息中显示图片 → 历史消息加载后图片仍显示；旧纯文本消息正常渲染

---

## 1. Phase 1 范围

Phase 1 实现**图片附件的完整链路**，不含文件附件（Phase 2）和 offload 降级（Phase 3）。

| 能力 | Phase 1 | Phase 2 | Phase 3 |
|------|---------|---------|---------|
| 图片粘贴/拖拽/选择 | ✅ | — | — |
| 图片磁盘存储 | ✅ | — | — |
| ContentBlock[] JSON 存储 | ✅ | — | — |
| 旧数据降级渲染 | ✅ | — | — |
| Vision 模型 rehydrate | ✅ | — | — |
| 非 Vision 模型投影占位 | ✅ | — | — |
| 文件附件 | ❌ | ✅ | — |
| `inputModalities` 替代 `supportVision` | ❌ | ✅ | — |
| offload 降级 | ❌ | — | ✅ |
| 图片规范化 | ❌ | — | ✅ |

---

## 2. 涉及文件清单

### 2.1 前端新增文件

| 文件路径 | 用途 |
|----------|------|
| `src/chat/core/contentBlockTypes.ts` | ContentBlock / ImageAttachmentRef / FileAttachmentRef 类型定义 |
| `src/chat/core/contentBlockUtils.ts` | `buildContentBlocks` / `parseContent` / `serializeContent` 工具函数 |
| `src/chat/core/attachmentTypes.ts` | Attachment 类型 + MIME 白名单 + 大小/数量限制常量 |
| `src/chat/message/ImageThumbnail.tsx` | 图片缩略图渲染组件 |
| `src/chat/message/ImageThumbnail.module.css` | 图片缩略图样式 |
| `src/chat/message/FileCard.tsx` | 文件卡片渲染组件（Phase 1 仅骨架，Phase 2 启用） |
| `src/chat/message/FileCard.module.css` | 文件卡片样式 |
| `src/chat/message/AttachmentPreview.tsx` | 输入框附件预览条组件 |
| `src/chat/message/AttachmentPreview.module.css` | 预览条样式 |

### 2.2 前端变更文件

| 文件路径 | 当前状态 | 变更内容 |
|----------|----------|----------|
| `src/chat/core/chatTypes.ts` | `Message.content: string` | 不变（仍为 string，存 ContentBlock[] JSON） |
| `src/chat/message/MessageInput.tsx` | 纯文本输入（`onSend: (content: string) => void`） | 新增附件按钮 + 粘贴 + 拖拽 + 预览条 |
| `src/chat/message/MessageContent.tsx` | user→`<p>{content}</p>`，assistant→ReactMarkdown | 结构化渲染 + 旧数据降级 |
| `src/stores/chatMessageSlice.ts` | `sendMessage(content: string)` | 签名不变，content 为 JSON |
| `src/settings/provider/types.ts` | `ModelConfig` 无 vision 字段 | 新增 `supportVision?: boolean` |
| `src-tauri/tauri.conf.json` | `assetProtocol.scope.allow` 仅 mascot-assets | 新增 `$APPDATA/attachments/**` |

### 2.3 Rust 新增文件

| 文件路径 | 用途 |
|----------|------|
| `src-tauri/crates/assistant-storage/src/attachment_store.rs` | AttachmentStore：磁盘存储 + 内容寻址去重 |
| `src-tauri/crates/assistant-agent/src/multimodal.rs` | Ingress（admit_attachments）+ Dispatch（rehydrate / project） |

### 2.4 Rust 变更文件

| 文件路径 | 当前状态 | 变更内容 |
|----------|----------|----------|
| `src-tauri/crates/assistant-storage/src/lib.rs` | 7 个 pub mod | 新增 `pub mod attachment_store;` |
| `src-tauri/crates/assistant-agent/src/lib.rs` | 4 个 pub mod | 新增 `pub mod multimodal;` |
| `src-tauri/crates/assistant-agent/Cargo.toml` | 已有 tokio / assistant-storage | 新增 `sha2` |
| `src-tauri/crates/assistant-storage/Cargo.toml` | 无 tokio / sha2 | 新增 `sha2` / `tokio`（fs 异步 IO） |
| `src-tauri/src/setup.rs` | 初始化 DB + SecurityDbState + SyncEngine 等 | DB 初始化后新增 attachments 目录 + AttachmentStore 全局状态 |
| `src-tauri/src/cmd/chat.rs` | `send_chat_message`（L17-370） | 用户消息 insert 前调用 `admit_attachments`；`prepare_messages` 后、`chat_completion_stream` 前调用 rehydrate/project |
| `src-tauri/src/cmd/mod.rs` | `pub use chat::*` 等 | 新增 `pub use chat::*`（如 read_attachment 放在 chat.rs 中则自动导出） |
| `src-tauri/src/lib.rs` | `invoke_handler` 注册 ~90 个 command | 新增 `cmd::read_attachment` |
| `src-tauri/crates/assistant-agent/src/prompt_builder.rs` | `prepare_messages` 映射 MessageRow→ChatMessage | 或在此之后注入 rehydrate/project 调用 |
| `src-tauri/crates/assistant-providers/src/openai/message_converter.rs` | `_ => { "content": m.content }` 通配符 | User 消息拆出独立分支，遍历 ContentBlock[] 生成 parts |
| `src-tauri/crates/assistant-providers/src/anthropic/message_converter.rs` | `convert_user_message` 含 `tool_call_id` hack（L49-66） | 重写为 ContentBlock 遍历，删除 hack + `parse_image_source` |
| `src-tauri/crates/assistant-providers/src/gemini/message_converter.rs` | `convert_user_parts` 用 `try_parse_inline_image`（L47） | 先尝试 `parse_content`，失败再走原逻辑 |
| `src-tauri/crates/assistant-providers/src/types/message.rs` | `ContentBlock` 有 5 个字段 | 扩展 7 个新字段（attachment_id / media_type / bytes / width / height / name / cached_data）；`ContentBlockType` 新增 `File` |
| `src-tauri/crates/assistant-providers/src/base.rs` | `ModelConfig` 有 4 个字段 | 新增 `support_vision: bool` + `#[serde(default)]` |

### 2.5 遗漏文件补充（分析后新增）

以下文件在原始清单中遗漏，但经代码库分析确认**必须变更**，否则编译失败或功能缺失：

#### 🔴 编译阻断（不改则编译失败）

| 文件路径 | 遗漏原因 | 变更内容 |
|----------|----------|----------|
| `src-tauri/crates/assistant-providers/src/anthropic/discovery.rs` | `ModelConfig` 构造缺 `support_vision` 字段 | `ModelConfig { …, support_vision: true }`（Anthropic Claude 系列均支持 Vision） |
| `src-tauri/crates/assistant-providers/src/openai/discovery.rs` | 同上 | `ModelConfig { …, support_vision: false }`（从 model id 启发推断：含 `vision`/`4o`/`gpt-4-turbo` → true，其余 false） |
| `src-tauri/crates/assistant-providers/src/gemini/discovery.rs` | 同上 | `ModelConfig { …, support_vision: true }`（Gemini 系列均支持 Vision） |
| `src-tauri/src/protocol/dto/settings.rs` | `ModelConfigDto` 缺 `support_vision` 字段 | 新增 `support_vision: bool` + `#[serde(default)]`，`From<ModelConfig>` impl 同步 |
| `src-tauri/crates/assistant-providers/src/types/mod.rs` | `ContentBlock` 重构后 re-export 需更新 | `pub use message::{ContentBlock, ContentBlockType, …}` → 移除 `ContentBlockType`（或保留兼容），确保 enum re-export 正确 |

#### 🔴 运行时阻断（不改则功能缺失或报错）

| 文件路径 | 遗漏原因 | 变更内容 |
|----------|----------|----------|
| `src-tauri/crates/assistant-i18n/locales/zh-CN.json` | Rust 端 `t!("chat.too_many_images")` 等 key 不存在 | 新增 `chat.too_many_images` / `chat.image_too_large` / `chat.image_projected_placeholder` |
| `src-tauri/crates/assistant-i18n/locales/en-US.json` | 同上 | 同上，英文文案 |
| `src/i18n/locales/zh-CN.json` | 前端附件 UI 文案 key 不存在 | 新增"当前模型不支持图片"等文案 |
| `src/i18n/locales/en-US.json` | 同上 | 同上，英文文案 |

#### 🟡 功能完整性（不改不影响编译但影响质量）

| 文件路径 | 遗漏原因 | 变更内容 |
|----------|----------|----------|
| `src/hooks/useFileDrop.ts` | 已有完整拖拽 hook（accept 过滤、isDragging 状态、drag 事件处理），MessageInput 应复用而非从零实现 | MessageInput 拖拽逻辑改为调用 `useFileDrop`，仅加图片 MIME 校验 |
| `src/chat/message/MessageContent.module.css` | 结构化渲染需新样式（block 间距、图片容器等） | 新增 `.imageBlock` / `.fileBlock` 等样式规则 |
| `src/stores/providerStore.ts` | `ModelConfig` 类型扩展后，使用 `ModelConfig` 的组件需能读取 `supportVision` | 确认类型导出正确（通常自动生效，但需验证） |

---

## 3. 前端规格

### 3.1 ContentBlock 类型系统

**`src/chat/core/contentBlockTypes.ts`**（新建）：

```typescript
export type ContentBlock = TextBlock | ImageBlock | FileBlock

export interface TextBlock {
  type: 'text'
  text: string
}

export interface ImageBlock {
  type: 'image'
  attachment: ImageAttachmentRef
  offloaded?: true
}

export interface FileBlock {
  type: 'file'
  attachment: FileAttachmentRef
}

export interface ImageAttachmentRef {
  attachmentId: string          // sha256 hex + 扩展名（如 "abc...def.png"），同时作为磁盘文件名
  mediaType: ImageMediaType
  bytes: number
  width?: number                // 前端发送时可选；后端 save_image 不解码获取
  height?: number               // 前端发送时可选；后端 save_image 不解码获取
  name?: string
}

export interface FileAttachmentRef {
  attachmentId: string
  name: string
  bytes: number
}

export type ImageMediaType = 'image/png' | 'image/jpeg' | 'image/webp' | 'image/gif'
```

### 3.2 附件类型与约束

**`src/chat/core/attachmentTypes.ts`**（新建）：

```typescript
export const ALLOWED_IMAGE_MIME_TYPES = [
  'image/png', 'image/jpeg', 'image/webp', 'image/gif',
] as const
export type AllowedImageMimeType = (typeof ALLOWED_IMAGE_MIME_TYPES)[number]

export type AttachmentKind = 'image' | 'file'

export interface Attachment {
  id: string
  kind: AttachmentKind
  file: File
  previewUrl: string           // URL.createObjectURL(file) 做预览，浏览器管理 Blob 内存
  mimeType: AllowedImageMimeType
  originalSizeBytes: number
  width?: number
  height?: number
  attachmentId?: string        // 后端返回后填充
}

export const ATTACHMENT_MAX_IMAGES = 4
export const ATTACHMENT_MAX_FILES = 4
export const ATTACHMENT_MAX_IMAGE_SIZE_BYTES = 8 * 1024 * 1024   // 8 MB
export const ATTACHMENT_MAX_FILE_SIZE_BYTES = 16 * 1024 * 1024   // 16 MB
```

### 3.3 内容块工具函数

**`src/chat/core/contentBlockUtils.ts`**（新建）：

| 函数 | 签名 | 说明 |
|------|------|------|
| `buildContentBlocks` | `(text: string, attachments: Attachment[]) => ContentBlock[]` | 构造 ContentBlock[]：文本 → TextBlock，图片 → ImageBlock |
| `parseContent` | `(content: string) => ContentBlock[] \| null` | JSON.parse 成功且首元素有 type → 返回数组；失败 → null（旧数据降级） |
| `serializeContent` | `(blocks: ContentBlock[]) => string` | `JSON.stringify(blocks)` |
| `validateImageFile` | `(file: File) => { ok: boolean; error?: string }` | MIME 白名单 + 大小校验 |

**`parseContent` 是旧数据兼容的核心**：

```typescript
export function parseContent(content: string): ContentBlock[] | null {
  try {
    const parsed = JSON.parse(content)
    if (
      Array.isArray(parsed) &&
      parsed.length > 0 &&
      parsed.every((b): b is Record<string, unknown> =>
        typeof b === 'object' && b !== null && 'type' in b && typeof b.type === 'string'
      )
    ) {
      return parsed as ContentBlock[]
    }
    return null
  } catch {
    return null
  }
}
```

> **设计说明**：校验每个元素是否有 `type` 字符串字段，与 Rust 端 `serde(tag = "type")` 反序列化行为对齐——未知 `type` 值的 block 在渲染时走 `default: return null`，Rust 端 `serde` 反序列化失败返回 `None`，两端行为一致。不硬编码已知 type 值列表，避免新增 ContentBlock 变体时前端遗漏更新。`as ContentBlock[]` 断言在此处安全，因为上游 `every` 已完成结构校验。

### 3.4 MessageInput 变更

当前 `src/chat/message/MessageInput.tsx`（113 行）：
- Props: `{ onSend, onCancel, isStreaming, disabled, isFull? }`
- 内部状态: `value: string`
- 发送: `onSend(value.trim())` → `setValue("")`

变更后：

- **新增内部状态**: `attachments: Attachment[]`
- **附件按钮**（📎 图标，Input 左侧或右侧）：点击 → `<input type="file" accept="image/*">` → 校验 → 加入 `Attachment[]`
- **粘贴图片**：`onPaste` 事件 → 遍历 `clipboardData.items` → `item.type.startsWith("image")` → `item.getAsFile()` → 校验 → 加入
- **拖拽图片**：`onDragOver`（`e.preventDefault()` + 边框高亮视觉反馈）+ `onDrop` 事件 → `e.preventDefault()` → 遍历 `dataTransfer.files` → 校验 → 加入；拖拽区域为输入框区域
- **附件预览条**：`<AttachmentPreview attachments={attachments} onRemove={...} />` 渲染在 Input 上方
- **模型能力感知**：从 store 获取当前模型 `supportVision`，`false` 时隐藏附件按钮，粘贴/拖拽 toast 提示"当前模型不支持图片"
- **发送逻辑不变签名**: `onSend(serializeContent(buildContentBlocks(text, attachments)))`，发送后 `setAttachments([])` + `setValue("")`

### 3.5 MessageContent 变更

当前 `src/chat/message/MessageContent.tsx`（64 行）：
- user → `<p>{content}</p>`
- assistant → `<ReactMarkdown>{content}</ReactMarkdown>`

变更后：

```typescript
function MessageContent({ content, role }: { content: string; role: string }) {
  const blocks = useMemo(() => parseContent(content), [content])

  if (blocks) {
    return (
      <div className={css.root}>
        {blocks.map((block, i) => {
          switch (block.type) {
            case 'text':
              return role === 'user'
                ? <p key={i}>{block.text}</p>
                : <ReactMarkdown key={i} ...>{block.text}</ReactMarkdown>
            case 'image':
              return <ImageThumbnail key={i} attachment={block.attachment} />
            case 'file':
              return <FileCard key={i} attachment={block.attachment} />
            default:
              return null
          }
        })}
      </div>
    )
  }

  // 旧数据降级：parseContent 返回 null → 走原逻辑
  return (
    <div className={css.root}>
      {role === 'user'
        ? <p>{content}</p>
        : <ReactMarkdown ...>{content}</ReactMarkdown>}
    </div>
  )
}
```

### 3.6 ImageThumbnail 组件

**`src/chat/message/ImageThumbnail.tsx`**（新建）：

- Props: `attachment: ImageAttachmentRef`
- **渲染方式**：使用 Tauri `asset:` 协议直出，利用 WebView 浏览器缓存，不走 `invoke` IPC
  ```typescript
  import { convertFileSrc } from "@tauri-apps/api/core"
  import { appDataDir } from "@tauri-apps/api/path"

  // 模块级缓存：app 初始化时一次性解析，避免组件内重复 await
  let attachmentsDirCache: string | null = null
  async function getAttachmentsDir() {
    if (!attachmentsDirCache) {
      attachmentsDirCache = (await appDataDir()) + "attachments"
    }
    return attachmentsDirCache
  }

  // 组件内
  const assetUrl = useMemo(() => {
    // attachmentId 即为 sha256 hex + 扩展名（如 "abc...def.png"），同时是磁盘文件名
    return convertFileSrc(`${attachmentsDir}/${attachment.attachmentId}`)
    // → asset://localhost/<encoded-path>  浏览器自动缓存
  }, [attachment.attachmentId])
  ```
- `<img src={assetUrl} loading="lazy" ...>` — 原生懒加载 + 浏览器 HTTP 缓存
- **占位骨架屏**：用 `width/height`（如有）计算 `aspect-ratio`，防止 CLS；无尺寸时用固定高度骨架屏
- 样式：max-width/max-height 限制，圆角
- 点击放大：lightbox 或新窗口打开原图
- `offloaded` 标记：显示占位文本"图片已卸载"
- 加载中：skeleton（`<img>` 的 `onLoad` 事件切换）
- 加载失败：`onError` 事件 → 错误图标 + 重试按钮

> **为什么用 `asset:` 协议而非 `invoke read_attachment`**：
> - `invoke` 走 IPC，完全绕过 WebView HTTP 缓存，相同图片每次重新读取+解码
> - `asset:` 协议走 WebView 网络栈，WebView2/WebKit 均有磁盘缓存，相同 URL 零开销
> - `convertFileSrc` 已在项目 `AssetPickerField` 中验证可用
> - `<img loading="lazy">` 在 WebView2/WebKit 中原生支持，零 JS 开销
>
> **`read_attachment` command 保留**，但仅用于内部/高级场景（如 rehydrate、加密附件、offload 远程取回），不在前端渲染链路使用。

### 3.7 ModelConfig 变更

**前端** `src/settings/provider/types.ts`（L1-8）：

```typescript
// 当前
export interface ModelConfig {
  id: string;
  name: string;
  ownedBy: string;
  contextWindow?: number;
}
// 变更后
export interface ModelConfig {
  id: string;
  name: string;
  ownedBy: string;
  contextWindow?: number;
  supportVision?: boolean;   // 新增
}
```

**Rust** `src-tauri/crates/assistant-providers/src/base.rs`（L168-174）：

```rust
// 当前
pub struct ModelConfig {
    pub id: String,
    pub name: String,
    pub owned_by: String,
    #[serde(default)]
    pub context_window: u32,
}
// 变更后
pub struct ModelConfig {
    pub id: String,
    pub name: String,
    pub owned_by: String,
    #[serde(default)]
    pub context_window: u32,
    #[serde(default)]
    pub support_vision: bool,   // 新增
}
```

---

## 4. Rust 后端规格

### 4.0 关键架构背景（基于实际代码）

实际请求链路（从 `src-tauri/src/cmd/chat.rs` 读取）：

```
send_chat_message(convId, content, modeId, branchPointMsgId)   ← L30
  │
  ├─ L76-174: 插入 user message + 空 assistant message 到 SQLite
  │           （此时 content 直接存入，可能是纯文本或 ContentBlock[] JSON）
  │
  ├─ L195-208: 获取 provider / model 配置
  │
  ├─ L209: let chat_messages = prompt_builder::prepare_messages(&detail, system_prompt, None);
  │         ↑ 将 ConversationDetail.messages (Vec<MessageRow>) 映射为 Vec<ChatMessage>
  │         ↑ 每个 MessageRow.content → ChatMessage.content（string）
  │
  ├─ L210-257: 构建 ProviderConfig / adapter / options
  │
  ├─ L258: adapter.chat_completion_stream(&chat_messages, &model, &options, cancel_token)
  │         ↑ Provider 转换器在此将 ChatMessage → API 格式
  │
  └─ L287: chat_runtime::consume_stream(…) → 流式消费 → emit delta → DB 写入
```

**因此**：
- **Ingress（admit_attachments）**：应在 L76 插入 user message **之前**调用，将前端传来的含 base64 的 ContentBlock[] 转换为仅含 attachmentId 引用的版本
- **Dispatch（rehydrate / project）**：应在 L209 `prepare_messages` **之后**、L258 `chat_completion_stream` **之前**调用，对 `chat_messages: Vec<ChatMessage>` 做 in-place 变换
- **`chat_runtime.rs`** 是流消费者（`consume_stream`，L287），不参与消息格式转换，**无需变更**

### 4.1 AttachmentStore

**`assistant-storage/src/attachment_store.rs`**（新建）：

```rust
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use sha2::{Sha256, Digest};

/// 不可变配置，无需锁保护 IO 操作
pub struct AttachmentStoreConfig {
    pub dir: PathBuf,
}

pub struct AttachmentStore {
    config: Arc<AttachmentStoreConfig>,
}

/// 附件操作专用错误类型
#[derive(Debug, thiserror::Error)]
pub enum AttachmentError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid media type: {0}")]
    InvalidMediaType(String),
    #[error("Image too large: {size} bytes (max {max})")]
    TooLarge { size: usize, max: usize },
    #[error("Too many images: {count} (max {max})")]
    TooManyImages { count: usize, max: usize },
    #[error("Base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
}

impl AttachmentStore {
    pub fn new(dir: PathBuf) -> Self;

    /// 保存图片到磁盘，返回持久化引用
    /// 文件名格式：<sha256_hex_64>.<ext>（ext 从 media_type 推导，如 image/png → .png）
    pub async fn save_image(&self, data: &[u8], media_type: &str) -> Result<ImageAttachmentRef>;

    /// 按 ID 读取图片字节（ID 含扩展名，直接拼接路径）
    pub async fn read(&self, id: &str) -> Result<Vec<u8>>;

    /// 列出所有附件（供手动清理 UI 使用，Phase 2）
    pub async fn list_all(&self) -> Result<Vec<AttachmentEntry>>;

    /// 删除指定附件（供手动清理 UI 使用，Phase 2）
    pub async fn delete(&self, id: &str) -> Result<()>;
}
```

**常量**：

| 常量 | 值 | 说明 |
|------|-----|------|
| `ATTACHMENTS_DIR_NAME` | `"attachments"` | 子目录名 |

> **设计决策 — 全手动清理**：
>
> 不设 TTL 自动清扫、不设总量上限自动驱逐。附件只增不删，由用户在设置界面手动清理（Phase 2 提供 UI）。
>
> 原因：
> - 自动 TTL 可能删除 DB 中仍有引用的附件 → 图片丢失
> - 总量驱逐需跨 crate 查 DB 引用才能安全删除，复杂度高
> - 桌面应用磁盘空间通常充裕，附件对现代 SSD 无压力
> - 手动清理让用户有明确预期，不会出现"昨天发的图片今天消失"

**存储路径**：`<app_data_dir>/attachments/<sha256_hex>.<ext>`（ext 从 media_type 推导，确保 `asset:` 协议返回正确 MIME）

其中 `sha256_hex` 计算方式：
```rust
let hash = Sha256::digest(data);
let sha256_hex = format!("{:x}", hash);  // 完整 64 位 hex
let ext = media_type_to_ext(media_type); // "image/png" → "png"
let attachment_id = format!("{}.{}", sha256_hex, ext);  // 含扩展名，供 asset 协议 MIME 推导
```

```rust
fn media_type_to_ext(mt: &str) -> &'static str {
    match mt {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "bin",
    }
}
```

> **设计决策**：使用完整 64 位 hex（256 位）而非截断前缀。原因：
> - 截断前 16 位（64 位）在生日悖论下 ~2³² 个附件即 50% 碰撞，碰撞 = 不同图片写同一文件 = 静默数据损坏
> - 完整 64 位 hex 消除碰撞风险，且与 `attachmentId` 格式一致，`convertFileSrc` 映射天然成立
> - 现代文件系统对 64 字符文件名无压力

**写入方式**：原子写入 + 内容寻址去重
```rust
// 使用 create_new 原子创建：文件已存在 = 相同内容已存盘 = 去重命中，直接返回
// 避免 exists() + write 的 TOCTOU 竞态
match fs::OpenOptions::new().write(true).create_new(true).open(&path).await {
    Ok(file) => {
        // 新文件，写入数据
        let mut writer = tokio::io::BufWriter::new(file);
        writer.write_all(data).await?;
        writer.flush().await?;
        drop(writer);
    }
    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
        // 去重命中：相同 SHA-256 = 相同内容，跳过写入
    }
    Err(e) => return Err(e.into()),
}
```

> **并发安全**：`save_image` / `read` 仅需不可变 `config`（`Arc<AttachmentStoreConfig>`），无需锁。`delete` / `clean_tmp` 同理。

### 4.2 Ingress 管道

**`assistant-agent/src/multimodal.rs`**（新建）— `admit_attachments`：

```rust
use assistant_providers::types::message::ContentBlock;

/// 入站处理：接收前端 ContentBlock[] JSON，将 ImageBlock 的 base64 数据存盘，
/// 替换为 attachmentId 引用，返回新的 JSON 字符串。
///
/// 调用时机：send_chat_message 中插入 user message 到 SQLite 之前
///
/// 前端发送的 ImageBlock 格式（Ingress 前态）：
///   { "type": "image", "attachment_id": "", "cached_data": "base64...", ... }
/// attachment_id 为空字符串表示"尚未存盘"，cached_data 含 base64 原始数据。
/// Ingress 后替换为：
///   { "type": "image", "attachment_id": "sha256hex64.png", ... }
/// cached_data 被 #[serde(skip_serializing)] 不序列化，但可反序列化（Ingress 读取前端 base64）。
pub async fn admit_attachments(
    content: &str,
    store: &AttachmentStore,
) -> Result<String> {
    let mut blocks: Vec<ContentBlock> = serde_json::from_str(content)
        .map_err(|e| format!("Invalid ContentBlock JSON: {}", e))?;

    // 前置校验：先数图片数量，避免存了一半再报错产生孤立附件
    let image_count = blocks.iter()
        .filter(|b| matches!(b, ContentBlock::Image { attachment_id, .. } if attachment_id.is_empty()))
        .count();
    if image_count > 4 {
        return Err(t!("chat.too_many_images", max = 4).into());
    }

    for block in blocks.iter_mut() {
        let ContentBlock::Image { ref attachment_id, ref mut cached_data, media_type, width, height, name, .. } = block
            else { continue };

        // 跳过已存盘的图片（attachment_id 非空）
        if !attachment_id.is_empty() { continue; }

        // 从 cached_data 提取 base64 字节
        let data_b64 = cached_data.take()  // take 避免 clone
            .ok_or("ImageBlock missing cached_data (base64 payload)")?;
        let data = general_purpose::STANDARD.decode(&data_b64)
            .map_err(|e| format!("Invalid base64 in ImageBlock: {}", e))?;

        if data.len() > 8 * 1024 * 1024 {
            return Err(t!("chat.image_too_large", max_mb = 8).into());
        }

        let mt = media_type.clone().unwrap_or_else(|| "image/png".to_string());
        let ref_info = store.save_image(&data, &mt).await?;

        // 替换为持久化引用
        *block = ContentBlock::Image {
            attachment_id: ref_info.id,     // sha256 hex + 扩展名（如 "abc...def.png"）
            media_type: Some(ref_info.media_type),
            bytes: Some(ref_info.bytes),
            width: *width,
            height: *height,
            name: name.clone(),
            cached_data: None,              // 清除临时数据
        };
    }

    Ok(serde_json::to_string(&blocks)?)
}
```

> **设计决策**：
>
> 1. **使用 `Vec<ContentBlock>` 而非 `Vec<serde_json::Value>`**：
>    - `Value` 操作绕过类型系统，拼写错误/字段遗漏无法编译期捕获
>    - `ContentBlock` enum 保证非法状态不可表示（如 Image 含 text 字段）
>    - 与前端判别联合类型对齐，全链路类型闭合
>
> 2. **前端 Ingress 前态的 `attachment_id` 为空字符串**：
>    - enum 中 `attachment_id: String` 是必填字段（非 Option），前端发送时填空字符串
>    - Ingress 后替换为完整 sha256 hex，空字符串不再出现
>    - 这比 `Option<String>` 更简洁——空字符串是明确的"未存盘"标记，无需两态类型

### 4.3 Dispatch 管道

**`assistant-agent/src/multimodal.rs`**（同文件）— rehydrate / project：

```rust
use crate::prompt_builder;  // 或直接操作 Vec<ChatMessage>

/// Vision 模型：从磁盘读回图片字节，注入 ChatMessage.content_blocks
///
/// 采用方案 A：给 ChatMessage 新增 #[serde(skip)] content_blocks 字段，
/// rehydrate 填充此字段，Provider 转换器优先读 content_blocks，
/// fallback 到 parse_content(&msg.content)。
/// 不将 cached_data 写回 msg.content（skip_serializing 会跳过它，写入无意义）。
pub async fn rehydrate_images(
    messages: &mut [ChatMessage],
    store: &AttachmentStore,
) {
    for msg in messages.iter_mut() {
        if msg.role != MessageRole::User { continue; }
        let Some(mut blocks) = parse_content(&msg.content) else { continue };

        let mut has_image = false;
        for block in blocks.iter_mut() {
            let ContentBlock::Image { ref attachment_id, ref mut cached_data, .. } = block
                else { continue };

            match store.read(attachment_id).await {
                Ok(data) => {
                    *cached_data = Some(general_purpose::STANDARD.encode(&data));
                    has_image = true;
                }
                Err(e) => {
                    log::warn!("Failed to rehydrate image {}: {}", attachment_id, e);
                    // 保持无 cached_data 状态，转换器会跳过
                }
            }
        }

        // 仅当有图片被 rehydrate 时才填充 content_blocks
        // Provider 转换器优先读 content_blocks，fallback 到 parse_content(&msg.content)
        if has_image {
            msg.content_blocks = Some(blocks);
        }
    }
}

/// 非 Vision 模型：将 ImageBlock 替换为 TextBlock 占位文本
pub fn project_images_for_text_model(messages: &mut [ChatMessage]) {
    for msg in messages.iter_mut() {
        if msg.role != MessageRole::User { continue; }
        let Some(blocks) = parse_content(&msg.content) else { continue };

        let projected: Vec<ContentBlock> = blocks.into_iter().map(|block| {
            match block {
                ContentBlock::Image { .. } => ContentBlock::Text {
                    text: t!("chat.image_projected_placeholder").to_string(),
                },
                other => other,
            }
        }).collect();

        if let Ok(json) = serde_json::to_string(&projected) {
            msg.content = json;
        }
    }
}
```

> **关键设计决策 — `cached_data` 传递机制（方案 A 定案）**：
>
> `cached_data` 使用 `#[serde(skip_serializing)]`（跳过序列化，允许反序列化）。
>
> **Ingress 阶段**：前端 JSON 含 `cached_data` → `serde_json::from_str` 反序列化成功 → `admit_attachments` 读取 base64 → 存盘 → 清除 `cached_data` → 序列化写回 `msg.content`（`cached_data` 被跳过，不写入 DB）。
>
> **Dispatch 阶段**：rehydrate 从磁盘读回字节 → 填充 `cached_data` → **但 `serde_json::to_string` 时 `cached_data` 被跳过** → `msg.content` 中无 `cached_data`。
>
> 因此 Provider 转换器**不能**从 `msg.content` JSON 反序列化获取 `cached_data`，必须通过 `ChatMessage.content_blocks` 字段传递：
>
> **方案 A（定案）**：给 `ChatMessage` 新增 `#[serde(skip)] content_blocks: Option<Vec<ContentBlock>>` 扩展字段。
> - rehydrate 填充此字段
> - Provider 转换器优先读 `content_blocks`，fallback 到 `parse_content(&msg.content)`
> - 对现有 `convert_messages(&[ChatMessage])` 签名无侵入
> - `#[serde(skip)]` 确保此字段不参与序列化/反序列化（纯运行时传递）
>
> Provider 转换器中统一读取逻辑：
> ```rust
> let blocks = msg.content_blocks.as_deref()
>     .or_else(|| parse_content(&msg.content).as_ref());
> ```

### 4.4 Tauri Command: read_attachment

**`src-tauri/src/cmd/chat.rs`**（追加）：

> **注意**：此 command 仅用于内部/高级场景（如 rehydrate、加密附件、offload 远程取回）。
> 前端渲染使用 `asset:` 协议（`convertFileSrc`），不走此 command。

```rust
#[command]
pub async fn read_attachment(
    app: AppHandle,
    attachment_id: String,
) -> Result<Vec<u8>, String> {
    let store = app.state::<AttachmentStore>();
    store.read(&attachment_id).await
        .map_err(|e| format!("Failed to read attachment: {}", e))
}
```

注册点：
- `src-tauri/src/cmd/mod.rs`：已有 `pub use chat::*;`，新函数自动导出
- `src-tauri/src/lib.rs` L73：`invoke_handler` 数组中追加 `cmd::read_attachment,`

### 4.5 Provider 消息转换器变更

#### 4.5.0 公共前置：Rust 端 `parse_content` 工具函数

三个转换器共享同一个解析逻辑。建议放在 `assistant-providers/src/types/message.rs` 或公共模块：

```rust
/// 尝试将 content 解析为 ContentBlock[]。
/// 成功 → Some(vec)，失败（旧数据纯文本）→ None
pub fn parse_content(content: &str) -> Option<Vec<ContentBlock>> {
    serde_json::from_str::<Vec<ContentBlock>>(content).ok()
        .filter(|b| !b.is_empty())
}
```

> **注意**：`serde(tag = "type")` 内部已做判别反序列化——`type` 值不匹配已知变体时自动返回 `Err`，
> 因此 `parse_content` 对非 ContentBlock JSON（如纯文本、工具调用数组）天然返回 `None`，无需额外校验。

#### 4.5.1 OpenAI 转换器

**文件**：`src-tauri/crates/assistant-providers/src/openai/message_converter.rs`（59 行）

**当前代码**（L51-56）— `_ =>` 通配符分支：

```rust
_ => {
    json!({
        "role": role_str(&m.role),
        "content": m.content       // ← 整个 content 当纯文本 string
    })
}
```

此分支处理 System / User / Assistant（无 tool_calls）三种角色。`m.content` 直接作为 string 传入。

**改造方案**：

将 `_ =>` 拆分为 `MessageRole::System` + `MessageRole::User` + `MessageRole::Assistant`（无 tool_calls 时）。**仅 User 需要 ContentBlock 遍历**。

```rust
MessageRole::User => {
    // 优先读 content_blocks（rehydrate 填充，含 cached_data），fallback 到 parse_content
    let blocks = m.content_blocks.as_deref()
        .or_else(|| parse_content(&m.content).as_ref());
    if let Some(blocks) = blocks {
        let mut parts = Vec::new();
        for block in blocks {
            match block {
                ContentBlock::Text { text } => {
                    parts.push(json!({"type": "text", "text": text}));
                }
                ContentBlock::Image { cached_data: Some(ref data), media_type, .. } => {
                    let mt = media_type.as_deref().unwrap_or("image/png");
                    parts.push(json!({
                        "type": "image_url",
                        "image_url": { "url": format!("data:{};base64,{}", mt, data) }
                    }));
                }
                ContentBlock::Image { cached_data: None, .. } => {
                    // 未 rehydrate（不应出现在 Vision 模型路径），跳过
                }
                _ => {} // File / Thinking 等跳过
            }
        }
        json!({ "role": "user", "content": parts })
    } else {
        // 旧数据降级：纯文本
        json!({ "role": "user", "content": m.content })
    }
}
MessageRole::System => {
    json!({ "role": "system", "content": m.content })
}
MessageRole::Assistant => {
    // 无 tool_calls 的 assistant 消息（极少见但可能）
    json!({ "role": "assistant", "content": m.content })
}
```

**OpenAI API 格式参考**：

```json
{
  "role": "user",
  "content": [
    { "type": "text", "text": "描述这张图片" },
    { "type": "image_url", "image_url": { "url": "data:image/png;base64,iVBOR..." } }
  ]
}
```

**完全不变的分支**：`MessageRole::Tool`（L17-24）、`MessageRole::Assistant if m.tool_calls.is_some()`（L25-50）

---

#### 4.5.2 Anthropic 转换器

**文件**：`src-tauri/crates/assistant-providers/src/anthropic/message_converter.rs`（370 行）

**当前代码**（L43-67）— `convert_user_message`：

```rust
fn convert_user_message(msg: &ChatMessage) -> Value {
    let mut content_blocks: Vec<Value> = vec![json!({
        "type": "text",
        "text": msg.content       // ← 问题1: content 整段当 text（新数据是 JSON 数组）
    })];

    // ← 问题2: hack —— 滥用 tool_call_id 字段传图片 data URI
    if let Some(ref image_url) = msg.tool_call_id {
        if image_url.starts_with("data:") || image_url.starts_with("http") {
            if let Some((media_type, data)) = parse_image_source(image_url) {
                content_blocks.push(json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": media_type,
                        "data": data
                    }
                }));
            }
        }
    }

    json!({ "role": "user", "content": content_blocks })
}
```

**问题总结**：
1. `msg.content` 整段作为 text block — 新数据是 ContentBlock[] JSON 数组，不应直接当文本渲染
2. `tool_call_id` hack（L49-66）— 滥用 Tool 消息字段传图片，语义混乱
3. `parse_image_source`（L135-146）仅在此 hack 中使用，无其他调用方

**改造方案**：

```rust
fn convert_user_message(msg: &ChatMessage) -> Value {
    // 优先读 content_blocks（rehydrate 填充，含 cached_data），fallback 到 parse_content
    let blocks = msg.content_blocks.as_deref()
        .or_else(|| parse_content(&msg.content).as_ref());
    if let Some(blocks) = blocks {
        let mut content_blocks: Vec<Value> = Vec::new();
        for block in blocks {
            match block {
                ContentBlock::Text { text } => {
                    content_blocks.push(json!({ "type": "text", "text": text }));
                }
                ContentBlock::Image { cached_data: Some(ref data), media_type, .. } => {
                    let mt = media_type.as_deref().unwrap_or("image/png");
                    content_blocks.push(json!({
                        "type": "image",
                        "source": { "type": "base64", "media_type": mt, "data": data }
                    }));
                }
                ContentBlock::Image { cached_data: None, .. } => {
                    // 未 rehydrate，跳过
                }
                _ => {}
            }
        }
        json!({ "role": "user", "content": content_blocks })
    } else {
        // 旧数据降级：纯文本包裹为 content_blocks 格式
        json!({
            "role": "user",
            "content": [{ "type": "text", "text": msg.content }]
        })
    }
}
```

**删除内容**：
- `tool_call_id` hack 整段（L49-66 中 `if let Some(ref image_url) = msg.tool_call_id` 块）
- `parse_image_source` 函数（L135-147）— 无其他调用方，安全移除

> **存量数据兼容声明**：`tool_call_id` 图片 hack 从未在正式发布版本中启用（仅存在于开发分支），
> 无用户存量数据依赖此 hack。可安全删除，无需降级兼容。

**Anthropic API 格式参考**：

```json
{
  "role": "user",
  "content": [
    { "type": "text", "text": "描述这张图片" },
    { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": "iVBOR..." } }
  ]
}
```

**完全不变的函数**：
- `convert_assistant_message`（L69-103）
- `convert_tool_result_message`（L105-124）
- `merge_consecutive_messages`（L149-175）
- `trim_final_assistant_trailing_whitespace`（L177-201）
- 所有 unit tests（L207-370）

---

#### 4.5.3 Gemini 转换器

**文件**：`src-tauri/crates/assistant-providers/src/gemini/message_converter.rs`（421 行）

**当前代码**（L40-55）— `convert_user_parts`：

```rust
fn convert_user_parts(msg: &ChatMessage) -> Vec<Value> {
    let mut parts = Vec::new();
    if !msg.content.is_empty() {
        if let Some(image_part) = try_parse_inline_image(&msg.content) {
            // ← 问题: 假设整个 content 是一个 data URI
            parts.push(json!({ "text": "Image attached" }));
            parts.push(image_part);
        } else {
            parts.push(json!({ "text": msg.content }));
        }
    }
    parts
}
```

**问题**：
1. `try_parse_inline_image` 假设整个 `content` 是一个 `data:` URI — 新数据是 ContentBlock[] JSON 数组
2. 无法处理"文本 + 多张图片"的组合场景

**改造方案**：

```rust
fn convert_user_parts(msg: &ChatMessage) -> Vec<Value> {
    // 优先读 content_blocks（rehydrate 填充，含 cached_data），fallback 到 parse_content
    let blocks = msg.content_blocks.as_deref()
        .or_else(|| parse_content(&msg.content).as_ref());
    if let Some(blocks) = blocks {
        let mut parts = Vec::new();
        for block in blocks {
            match block {
                ContentBlock::Text { text } => {
                    parts.push(json!({ "text": text }));
                }
                ContentBlock::Image { cached_data: Some(ref data), media_type, .. } => {
                    let mt = media_type.as_deref().unwrap_or("image/png");
                    parts.push(json!({
                        "inlineData": { "mimeType": mt, "data": data }
                    }));
                }
                ContentBlock::Image { cached_data: None, .. } => {
                    // 未 rehydrate，跳过
                }
                _ => {}
            }
        }
        return parts;
    }

    // 旧数据降级：保留 try_parse_inline_image 兼容旧格式
    let mut parts = Vec::new();
    if !msg.content.is_empty() {
        if let Some(image_part) = try_parse_inline_image(&msg.content) {
            parts.push(json!({ "text": "Image attached" }));
            parts.push(image_part);
        } else {
            parts.push(json!({ "text": msg.content }));
        }
    }
    parts
}
```

**Gemini API 格式参考**：

```json
{
  "role": "user",
  "parts": [
    { "text": "描述这张图片" },
    { "inlineData": { "mimeType": "image/png", "data": "iVBOR..." } }
  ]
}
```

**完全不变的函数**：
- `convert_assistant_parts`（L57-98）
- `convert_tool_result_parts`（L100-123）
- `push_content`（L125-140）
- `convert_tools`（L142-158）
- `try_parse_inline_image`（L149-163）— 保留用于旧数据降级
- 所有 unit tests（L163-421）

---

#### 4.5.4 ContentBlock 类型重构：扁平 struct → 判别枚举

**文件**：`src-tauri/crates/assistant-providers/src/types/message.rs`（57 行）

**当前 `ContentBlock`**（5 个字段的扁平 struct）：

```rust
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub block_type: ContentBlockType,
    pub text: Option<String>,
    pub image_url: Option<String>,
    pub thinking: Option<String>,
    pub signature: Option<String>,
}
```

**重构后**（判别枚举，与前端判别联合对齐）：

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Image {
        attachment_id: String,              // sha256 hex + 扩展名（如 "abc...def.png"），同时作为磁盘文件名
        #[serde(skip_serializing_if = "Option::is_none")]
        media_type: Option<String>,         // "image/png" 等
        #[serde(skip_serializing_if = "Option::is_none")]
        bytes: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        width: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        height: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,              // 文件名
        #[serde(skip_serializing)]          // 允许反序列化（Ingress 读取前端 base64），禁止序列化（不污染 DB）
        cached_data: Option<String>,        // rehydrate 后的 base64 数据
    },
    File {                                  // Phase 2 启用
        attachment_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        bytes: Option<u64>,
    },
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    ToolUse {
        id: String,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        input: Option<String>,
    },
    ToolResult {
        tool_use_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
    },
}
```

**`ChatMessage` 新增 `content_blocks` 扩展字段**（方案 A 定案）：

```rust
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip)]  // 纯运行时传递，不参与序列化/反序列化
    pub content_blocks: Option<Vec<ContentBlock>>,  // rehydrate 填充，Provider 转换器优先读取
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<crate::types::tool::ToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub is_error: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_signature: Option<String>,
}
```

**`ContentBlockType` 枚举保留**（用于 `block_type` 字段迁移期兼容，可后续移除）：

```rust
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ContentBlockType {
    Text,
    Image,
    File,
    Thinking,
    ToolUse,
    ToolResult,
}
```

**关键设计决策**：

1. **判别枚举 vs 扁平 struct**：
   - 扁平 struct 的 12 个 Option 字段允许非法状态（如 `Image { text: Some("oops") }`）
   - 判别枚举通过 Rust 类型系统保证**非法状态不可表示**
   - `#[serde(tag = "type")]` 与前端 JSON 格式 `{"type": "image", ...}` 完全对齐
   - 全链路类型闭合：前端判别联合 ↔ Rust 判别枚举 ↔ JSON `type` 标签

2. **`cached_data` 用 `#[serde(skip_serializing)]` 而非 `#[serde(skip)]`**：
   - `#[serde(skip)]` = skip_serializing + skip_deserializing → Ingress 无法反序列化前端发来的 `cached_data` → **admit_attachments 永远失败**
   - `#[serde(skip_serializing)]` 仅跳过序列化，**允许反序列化** → Ingress 可读取前端 base64 ✓
   - 序列化时跳过 → rehydrate 写回 `msg.content` 时 `cached_data` 被排除 → Provider 转换器需从 `Vec<ContentBlock>` struct 直接读取
   - DB 写入时 `cached_data` 已被 Ingress 清除为 `None` → 不污染 ✓
   - 唯一风险：rehydrate 后如果 `ChatMessage` 被意外写回 DB（如编辑消息），`cached_data` 不会写入（`skip_serializing`），但 `msg.content` 中的 ContentBlock JSON 也不含 `cached_data`，所以安全

3. **`image_url` 字段移除**：
   - 旧 Anthropic `tool_call_id` hack 使用 `image_url`，hack 删除后此字段无调用方
   - 如需兼容旧数据中的 `image_url`，可在 `Deserialize` 中用 `#[serde(alias)]` 或自定义 deserializer

4. **迁移策略**：
   - 旧数据（扁平 struct JSON）与新数据（enum JSON）的 `type` 字段位置相同
   - `serde(tag = "type")` 的反序列化能同时处理两种格式（只要字段名匹配）
   - 不需要 DB 迁移

### 4.6 初始化

**文件**：`src-tauri/src/setup.rs`（327 行）

当前 `init()` 函数的关键初始化顺序（L50-100）：
1. AppConf / OrbConf 加载
2. i18n 初始化
3. **DB 初始化**（L63-72）：`db::init_db(&db_path)` → `handle.manage(DbState(...))`
4. ChatTasks 初始化
5. SecurityDbState / SyncEngine / ScreenLockMonitor / ToolRegistry
6. Tray / shortcuts

**变更**：在步骤 3 之后、步骤 4 之前，新增 attachments 目录和 AttachmentStore 初始化：

```rust
// 在 DB 初始化（handle.manage(DbState(...))）之后新增：
{
    let data_dir = handle.path().app_data_dir()
        .expect("Failed to resolve app data dir");
    let attachments_dir = data_dir.join("attachments");
    std::fs::create_dir_all(&attachments_dir)
        .expect("Failed to create attachments directory");

    let store = AttachmentStore::new(attachments_dir);
    handle.manage(store);
    log::info!("AttachmentStore initialized at {:?}", attachments_dir);
}
```

需要在 `src-tauri/src/setup.rs` 顶部新增：
```rust
use assistant_storage::attachment_store::AttachmentStore;
```

> **设计决策**：`AttachmentStore` 直接作为 Tauri managed state，无需额外 `Mutex` wrapper。
> `AttachmentStore` 内部仅含 `Arc<AttachmentStoreConfig>`（不可变配置），天然 `Send + Sync`。

---

## 5. 数据流（修正版）

```
┌─────────────────────────────────────────────────────────────────────┐
│                          前端                                      │
│                                                                     │
│  用户粘贴/拖拽/选择图片                                              │
│      ↓                                                              │
│  MessageInput: Attachment[] + text                                  │
│      ↓                                                              │
│  buildContentBlocks(text, attachments) → ContentBlock[]             │
│      │  TextBlock { type: "text", text: "描述这张图片" }           │
│      │  ImageBlock { type: "image", attachment: {                  │
│      │    dataUri: "data:image/png;base64,iVBOR..."  ← base64 在此 │
│  }}                                                                 │
│      ↓                                                              │
│  serializeContent(blocks) → JSON string                             │
│      ↓                                                              │
│  chatService.sendChatMessage(convId, content)                       │
│      ↓ Tauri invoke                                                 │
├─────────────────────────────────────────────────────────────────────┤
│                      Rust: cmd/chat.rs                              │
│                                                                     │
│  send_chat_message(convId, content, ...)                            │
│      ↓                                                              │
│  ┌─ Ingress ──────────────────────────────────────────────────────┐ │
│  │  admit_attachments(content, &store)                             │ │
│  │      │                                                          │ │
│  │      ├─ JSON.parse → Vec<ContentBlock>                         │ │
│  │      ├─ 对每个 ImageBlock:                                      │ │
│  │      │   ├─ 从 dataUri/base64 提取字节                           │ │
│  │      │   ├─ 校验数量 ≤4, 大小 ≤8MB                               │ │
│  │      │   ├─ store.save_image(data, mime) → 写磁盘               │ │
│  │      │   └─ 替换 ImageBlock.attachment 为:                      │ │
│  │      │       { attachmentId: "sha256abc...def.png",             │ │
│  │      │         mediaType: "image/png",                          │ │
│  │      │         bytes: 1234567, ... }                            │ │
│  │      │       （attachmentId 含扩展名，不再含 base64 数据）       │ │
│  │      └─ JSON.stringify → 新 content 字符串                       │ │
│  └─────────────────────────────────────────────────────────────────┘ │
│      ↓                                                              │
│  conversations::insert_message(user_msg)  ← content 存入 SQLite     │
│  conversations::insert_message(assistant_msg)                       │
│      ↓                                                              │
│  prompt_builder::prepare_messages(&detail, ...)                     │
│      │  遍历 detail.messages (Vec<MessageRow>)                      │
│      │  每个 → ChatMessage { role, content, ... }                  │
│      ↓                                                              │
│  ┌─ Dispatch ────────────────────────────────────────────────────┐  │
│  │  if model.support_vision {                                     │  │
│  │      rehydrate_images(&mut chat_messages, &store).await        │  │
│  │          │  对 User 消息的 ContentBlock[]:                     │  │
│  │          ├─ 按 attachment_id 从磁盘读取字节                    │  │
│  │          ├─ 注入 cached_data: base64 字符串                    │  │
│  │          └─ 填充 ChatMessage.content_blocks 字段              │  │
│  │             （cached_data 不写入 msg.content，因 skip_serializing）│  │
│  │  } else {                                                      │  │
│  │      project_images_for_text_model(&mut chat_messages)         │  │
│  │          │  ImageBlock → TextBlock { text: "[图片已省略...]" }│  │
│  │  }                                                             │  │
│  └────────────────────────────────────────────────────────────────┘  │
│      ↓                                                              │
│  adapter.chat_completion_stream(&chat_messages, &model, &options, ..)│
│      ↓                                                              │
│  ┌─ Provider 转换器 ──────────────────────────────────────────────┐  │
│  │  读取优先级：content_blocks > parse_content(&content)           │  │
│  │                                                                │  │
│  │  OpenAI:  User → 遍历 blocks                                   │  │
│  │            TextBlock → {"type":"text","text":...}              │  │
│  │            ImageBlock → {"type":"image_url","image_url":{..}}  │  │
│  │                                                                │  │
│  │  Anthropic: User → 遍历 blocks                                 │  │
│  │            TextBlock → {"type":"text","text":...}              │  │
│  │            ImageBlock → {"type":"image","source":{..}}         │  │
│  │            （tool_call_id hack 已删除）                         │  │
│  │                                                                │  │
│  │  Gemini:   User → 遍历 blocks                                  │  │
│  │            TextBlock → {"text":...}                            │  │
│  │            ImageBlock → {"inlineData":{..}}                    │  │
│  └────────────────────────────────────────────────────────────────┘  │
│      ↓                                                              │
│  流式消费 → emit delta → DB 写入 assistant message content          │
├─────────────────────────────────────────────────────────────────────┤
│                      前端渲染                                       │
│                                                                     │
│  MessageContent:                                                    │
│      ↓                                                              │
│  parseContent(content) → ContentBlock[] | null                      │
│      │                                                              │
│      ├─ Some(blocks):                                               │
│      │   TextBlock  → user:<p> / assistant:<ReactMarkdown>         │
│      │   ImageBlock → <ImageThumbnail attachmentId="sha256:...">   │
│      │                 → convertFileSrc(attachments_dir/id)        │
│      │                 → <img src={assetUrl} loading="lazy">       │
│      │                 （浏览器 HTTP 缓存，零 IPC 开销）            │
│      │   FileBlock  → <FileCard>                                   │
│      │                                                              │
│      └─ null (旧数据):                                              │
│          user:<p>{content}</p>                                      │
│          assistant:<ReactMarkdown>{content}</ReactMarkdown>         │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 6. 验收标准

| # | 标准 | 验证方式 |
|---|------|---------|
| V1 | 粘贴图片 → 输入框预览条显示缩略图 → 发送 → 消息中显示图片 | Ctrl+V 粘贴截图 |
| V2 | 拖拽图片文件到输入框 → 预览 → 发送 → 显示 | 拖拽 PNG 文件 |
| V3 | 点击附件按钮 → 文件选择器 → 选择图片 → 预览 → 发送 → 显示 | 点击 📎 按钮 |
| V4 | 多张图片（≤4）→ 全部显示 | 连续添加 4 张图片 |
| V5 | 超过 4 张图片 → 提示超出限制 | 添加第 5 张图片 |
| V6 | 图片超过 8MB → 提示超出大小限制 | 选择大图片 |
| V7 | 非 Vision 模型 → 附件按钮隐藏，粘贴/拖拽提示不支持 | 切换到非 Vision 模型 |
| V8 | Vision 模型 → 图片正确发送到 Provider，模型可"看到"图片 | 发送图片 + "描述这张图片" |
| V9 | SQLite `messages.content` 中 ImageBlock 仅含 attachmentId 引用（无 base64） | `SELECT content FROM messages WHERE role='user' LIMIT 1` |
| V10 | 磁盘 `<app_data_dir>/attachments/` 中有图片文件（文件名为 `<sha256_64hex>.<ext>`） | 检查文件系统 |
| V11 | 旧纯文本消息 → 正常 markdown 渲染，无报错 | 打开含旧消息的对话 |
| V12 | 历史消息加载后图片仍正确显示（`asset:` 协议 + 浏览器缓存） | 重启应用，打开有图片的对话，DevTools Network 确认缓存命中 |
| V13 | `parseContent` 对非 JSON content 返回 null，降级为 markdown | 构造纯文本 content，验证渲染正确 |
| V14 | `parseContent` 对合法 ContentBlock[] JSON 返回结构化数组 | 构造 JSON content，验证解析结果 |
| V15 | Anthropic `tool_call_id` hack 已移除，`parse_image_source` 已删除 | grep 确认无残留 |
| V16 | 非 Vision 模型发送图片 → Provider 收到 i18n 占位文本 | 切换到 gpt-3.5-turbo，发图片，查看 API 请求 |
| V17 | `<img loading="lazy">` 生效：视口外图片不加载，滚动进入视口才加载 | 长对话 + 多图片，DevTools Network 确认懒加载 |
| V18 | 图片加载前显示骨架屏占位（无 CLS），加载失败显示错误+重试 | 检查布局稳定性，模拟加载失败 |
| V19 | 并发安全：快速连续发送含相同图片的消息 → 磁盘仅一份文件，无写入冲突 | 连续发送同一图片 5 次 |
| V20 | `content_blocks` 传递验证：Vision 模型 → Provider 转换器读到的 `cached_data` 非空 → API 请求体含 base64 | 发送图片，查看 Provider 日志/请求 |
| V21 | `attachmentId` 含扩展名：磁盘文件名格式为 `<sha256_hex>.<ext>`，`convertFileSrc` 返回正确 MIME | 检查磁盘文件名 + DevTools Network Content-Type |

---

## 7. Phase 2/3 候选

| 候选项 | 说明 | 优先级 |
|--------|------|--------|
| 附件管理界面 | 用户查看当前附件用量、按对话筛选附件、手动删除（调用 `list_all` / `delete`） | Phase 2（**必须**，Phase 1 无清理入口） |
| FileBlock 支持 | `ContentBlock::File` 变体启用，支持 PDF/文档等非图片附件 | Phase 2 |
| 图片压缩 | 前端发送前自动压缩（复用 `image-compressor` 工具），减少磁盘占用和 API 传输量 | Phase 2 |
| 加密附件 | 附件磁盘加密，`read_attachment` command 鉴权 | Phase 3 |
| 远程 offload | 附件上传到对象存储，本地仅保留引用，按需取回 | Phase 3 |

---

## 8. 分支处理影响分析

Phase 1 将 `Message.content` 从纯文本变为 `ContentBlock[]` JSON，影响所有读取/写入 content 的分支操作链路。

### 8.1 分支操作影响矩阵

| 操作 | 前端入口 | content 传递方式 | 是否受影响 | 说明 |
|------|---------|-----------------|-----------|------|
| **editMessage** | `chatMessageSlice.editMessage(id, newContent)` | `newContent: string` → `resendFrom` → `send_chat_message` | 🔴 **需适配** | 用户编辑含图片消息时，需保留 ImageBlock、仅修改 TextBlock，重组 ContentBlock[] JSON |
| **regenerateMessage** | `chatMessageSlice.regenerateMessage(id)` | `parentMsg.content` → `resendFrom` | 🟢 **无需修改** | `parentMsg.content` 透传，JSON 格式可原样传递 |
| **retryFromError** | `chatMessageSlice.retryFromError(id)` | `userMsg.content` → `resendFrom` | 🟢 **无需修改** | 同上，透传即可 |
| **createBranch** | `chatService.createBranch(convId, bpId, role, newContent)` | `newContent: string` 直接传 Rust | 🟡 **参数语义变化** | 当前无前端直接调用（editMessage/regenerate 走 resendFrom），但 Tauri command 签名需文档标注 |
| **switchBranch** | `chatService.switchBranch(convId, targetMsgId)` | 不传 content | 🟢 **不受影响** | 仅修改 `active_branch_id` |
| **forkToNewConversation** | `chatService.forkToNewConversation(srcId, leafId)` | Rust 端 `content: msg.content.clone()` | 🟢 **不受影响** | JSON 字符串透传复制，新会话 `prompt_builder` 可正确解析 |

### 8.2 editMessage 适配方案

当前 `editMessage` 实现（`chatMessageSlice.ts` L97-108）：

```typescript
editMessage: async (messageId: string, newContent: string) => {
  // newContent 是用户编辑后的纯文本
  await get().resendFrom(msg.timestamp, newContent, { kind: "branch", ... });
}
```

Phase 1 后需改为：

```typescript
editMessage: async (messageId: string, newText: string) => {
  const msg = state.messages.find((m) => m.id === messageId);
  if (!msg) return;

  // 从原消息 content 中提取 ImageBlock，保留图片
  const blocks = parseContent(msg.content);
  let contentToSend: string;
  if (blocks) {
    // 重组：新文本 + 原有 ImageBlock
    const rebuilt = [{ type: 'text', text: newText }, ...blocks.filter(b => b.type === 'image')];
    contentToSend = serializeContent(rebuilt);
  } else {
    // 旧数据降级：纯文本直接使用
    contentToSend = newText;
  }

  await get().resendFrom(msg.timestamp, contentToSend, { kind: "branch", ... });
}
```

> **设计决策**：用户编辑消息时，**图片不可编辑、不可删除、不可重排序**。编辑仅修改 TextBlock 文本，ImageBlock 按原顺序保留。图片的增删通过附件预览条操作，不在编辑流程中。

### 8.3 content_preview 适配（分支选择器）

当前 `get_message_siblings`（`conversations.rs` L775-810）使用 `SUBSTR(content, 1, 100)` 生成分支预览文本。Phase 1 后 content 为 JSON，`SUBSTR` 会截断 JSON 字符串，导致分支选择器显示 `{"blocks":[{"type":"text"...` 而非可读预览。

**适配方案**：在 Rust 端 `get_message_siblings` 中，将 `SUBSTR(content, 1, 100)` 替换为调用 `content_text_preview(content, 100)` 辅助函数：

```rust
/// 从 content 中提取纯文本预览。
/// ContentBlock[] JSON → 取首个 TextBlock 的前 max_len 字符；
/// 纯文本 → 直接截断；解析失败 → 截断原始字符串
fn content_text_preview(content: &str, max_len: usize) -> String {
    if let Ok(blocks) = serde_json::from_str::<Vec<ContentBlock>>(content) {
        for block in &blocks {
            if let ContentBlock::Text { text } = block {
                return text.chars().take(max_len).collect();
            }
        }
        // 无 TextBlock（如纯图片消息）→ 返回占位
        return "🖼️".to_string();
    }
    // 旧数据降级：纯文本截断
    content.chars().take(max_len).collect()
}
```

SQL 改为在 Rust 端后处理（先查完整 content，再调用 `content_text_preview`），或在 SQL 中用 `SUBSTR` 粗取后 Rust 端二次处理。

### 8.4 其他 content 消费者影响

| 消费者 | 文件 | 当前行为 | Phase 1 影响 | 适配方案 |
|--------|------|---------|-------------|---------|
| **会话搜索** | `conversations.rs` L465 | `m.content LIKE %query%` | JSON 内容会匹配到结构字符 | 改为 Rust 端 `content_plain_text()` 过滤（先 title LIKE 粗筛，再 Rust 端文本匹配） |
| **导出 Markdown** | `conversations.rs` L475-493 | `format!("{}\n\n", msg.content)` | JSON 直接输出不可读 | 改为 `content_plain_text()` 提取纯文本 |
| **自动标题** | `chatOrchestrator.ts` L78-93 | `fallbackTitleFromUserMessage(content)` 对 content 做 `split("\n")` | JSON 不是纯文本 | 改为 `fallbackTitleFromUserMessage(extractPlainText(content))` |
| **Mascot 表情** | `useMascotChatSync.ts` L82 | `lastAssistantMsg.content` 传给 `pickAckFace` | assistant 消息 content 仍为纯文本（流式追加），**不受影响** | 无需修改 |
| **流式合并** | `chatUtils.ts` L57 | `mergeStreamingContent` 拼接 `streamingContent` | assistant 流式 content 仍为纯文本，**不受影响** | 无需修改 |
| **空值检查** | `chat.rs` L17 | `content.trim().is_empty()` | JSON 永远非空（即使只有图片） | 改为 `is_content_empty(content)` 语义判断 |

### 8.5 涉及文件汇总（分支 + content 消费者）

| # | 文件 | 优先级 | 变更内容 |
|---|------|--------|---------|
| 1 | `src/stores/chatMessageSlice.ts` | 🔴 | `editMessage` 保留 ImageBlock 重组 ContentBlock[] |
| 2 | `src-tauri/crates/assistant-storage/src/conversations.rs` | 🔴 | `get_message_siblings` 的 `SUBSTR(content,1,100)` → `content_text_preview()`；搜索 LIKE → Rust 端文本匹配；导出 markdown → `content_plain_text()` |
| 3 | `src-tauri/src/cmd/chat.rs` | 🟡 | `content.trim().is_empty()` → `is_content_empty(content)` |
| 4 | `src/services/chatOrchestrator.ts` | 🟡 | `fallbackTitleFromUserMessage(content)` → `fallbackTitleFromUserMessage(extractPlainText(content))` |

---

## 9. 抽象层设计建议

Phase 1 将 content 从纯文本变为 JSON，未来 Phase 2/3 还会加 audio/PDF block。为减少每次扩展的改动面，建议在三个**边界点**各放一层轻量抽象。

### 9.1 边界 1：Rust `ChatMessage.content` → 类型化 `Vec<ContentBlock>`

**当前**：`ChatMessage.content: String`，3 个 Provider 转换器各自解析 string。

**建议**：`ChatMessage.content` 保持 `String`（DB/序列化边界），但新增 `content_blocks: Option<Vec<ContentBlock>>`（规格已规划，方案 A 定案）。**解析集中点**放在 `prompt_builder.rs` 的 `prepare_messages` 中：

```rust
// prompt_builder.rs — 唯一的 String → Vec<ContentBlock> 解析点
let content_blocks = parse_content(&m.content);
ChatMessage {
    role: ...,
    content: m.content.clone(),
    content_blocks,  // Option<Vec<ContentBlock>>
    ...
}
```

3 个 Provider 转换器统一读取模式：
```rust
let blocks = msg.content_blocks.as_deref()
    .or_else(|| parse_content(&msg.content).as_ref());
```

**收益**：未来加 audio/PDF block 时，改动面从"N 个文件各自解析 string"收拢到"1 个 ContentBlock enum 定义 + N 个 converter match arm"。

### 9.2 边界 2：Rust `MessageRow` helper 方法

DB 层 `MessageRow.content` 保持 `String`（对应 SQLite TEXT 列），但加 helper 方法封装常见操作：

```rust
// 在 conversations.rs 或新建 content_codec.rs
impl MessageRow {
    /// 语义空判断：纯文本空 or ContentBlock[] 无有效 block
    pub fn is_content_empty(&self) -> bool {
        if self.content.trim().is_empty() { return true; }
        // ContentBlock[] JSON：解析后检查是否有 TextBlock 或 ImageBlock
        if let Ok(blocks) = serde_json::from_str::<Vec<ContentBlock>>(&self.content) {
            return blocks.is_empty()
                || blocks.iter().all(|b| matches!(b, ContentBlock::Text { text } if text.trim().is_empty()));
        }
        false
    }

    /// 提取纯文本预览（用于 sibling preview、搜索、导出 markdown）
    pub fn content_text_preview(&self, max_len: usize) -> String {
        content_text_preview(&self.content, max_len)
    }

    /// 提取全文纯文本（用于搜索、导出 markdown）
    pub fn content_plain_text(&self) -> String {
        if let Ok(blocks) = serde_json::from_str::<Vec<ContentBlock>>(&self.content) {
            blocks.iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            self.content.clone()
        }
    }
}
```

**替换散落代码**：

| 当前散落代码 | 替换为 |
|-------------|--------|
| `SUBSTR(content, 1, 100)` (2处) | `msg.content_text_preview(100)` |
| `content.trim().is_empty()` (chat.rs) | `msg.is_content_empty()` |
| `m.content LIKE ?1` (搜索) | Rust 端 `msg.content_plain_text().contains(query)` |
| `format!("{}\n\n", msg.content)` (导出 markdown) | `format!("{}\n\n", msg.content_plain_text())` |

### 9.3 边界 3：前端 `contentBlockUtils.ts` 全覆盖

规格已定义 `contentBlockUtils.ts`，需确保**所有前端 content 消费者**都走此模块：

| 消费者 | 当前 | 应改为 |
|--------|------|--------|
| `MessageContent.tsx` | 直接渲染 `content` | `parseContent(content)` → 遍历 blocks 渲染（规格已覆盖） |
| `editMessage` | 直接传 `newContent` string | `rebuildContent(original, newText)` 保留 ImageBlock（§8.2） |
| `fallbackTitleFromUserMessage(content)` | `content.split("\n")` | `extractPlainText(content).split("\n")` |
| `pickAckFace(fullText)` | `lastAssistantMsg.content` | assistant 流式仍为纯文本，**无需改** |
| `MessageBubble` memo | `content !== next.content` | JSON string 相等性比较仍有效，**无需改** |

需在 `contentBlockUtils.ts` 中新增：

```typescript
/** 从 content 中提取纯文本（用于标题、预览等非渲染场景） */
export function extractPlainText(content: string): string {
  const blocks = parseContent(content);
  if (blocks) {
    return blocks
      .filter((b): b is TextBlock => b.type === 'text')
      .map((b) => b.text)
      .join('\n');
  }
  return content; // 旧数据降级
}

/** 重组 content：替换 TextBlock 文本，保留 ImageBlock */
export function rebuildContent(originalContent: string, newText: string): string {
  const blocks = parseContent(originalContent);
  if (blocks) {
    const rebuilt: ContentBlock[] = [
      { type: 'text', text: newText },
      ...blocks.filter((b): b is ImageBlock => b.type === 'image'),
    ];
    return serializeContent(rebuilt);
  }
  return newText; // 旧数据降级
}
```

### 9.4 不建议抽象的位置

| 位置 | 原因 |
|------|------|
| `MessageDto.content` / `MessageRow.content` 保持 `String` | DTO 层是序列化边界，保持 string 最简单，前端自行解析 |
| `DbWriteMsg.content` 保持 `String` | assistant 流式写入永远是纯文本追加，不含 ImageBlock |
| `chatService.ts` 参数保持 `string` | Tauri invoke 走 JSON 序列化，string 透传即可 |
| `send_chat_message` 的 `content: String` 参数 | 同上，Tauri command 参数是 JSON string |

### 9.5 抽象投入 vs 收益总结

| 抽象层 | upfront 改动 | 未来每次加 block type 节省 | 推荐度 |
|--------|-------------|--------------------------|--------|
| `ChatMessage.content_blocks` + 集中解析 | ~30 行（改 struct + 1 处解析） | 省掉 3 个 converter 各自解析 string（~15 行 × 3） | ⭐⭐⭐ **必做**（规格已规划） |
| `MessageRow` helper 方法 | ~40 行（3 个方法） | 省掉每次改 `is_empty`/`preview`/`export` 逻辑（~5 处） | ⭐⭐ **强烈推荐** |
| 前端 `contentBlockUtils` 全覆盖 | ~20 行（补 `extractPlainText` + `rebuildContent`） | 省掉每个新 UI 消费者重复写解析 | ⭐⭐ **推荐** |
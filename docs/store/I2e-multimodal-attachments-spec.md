# I2e — 多模态附件规格说明

> **来源**：参考 OpenHuman 标记协议 + AttachmentStash 磁盘存储 + OpenBitFun ImageContext 结构化对象 + AIRI image-projection 描述降级
> **前置**：I2-History ✅ 已完成（消息存储、分支、重发）
> **核心参考**：OpenHuman `multimodal.rs` + `attachments.ts` + `attachment.tsx`；OpenBitFun `imagePayload.ts` + `imageUtils.ts` + `tool_image_attachment.rs`；AIRI `image-projection.ts`

## 1. 目标

实现对话中发送图片/文件附件的能力，让用户可以：
- **图片附件**：粘贴/拖拽/选择图片，发送给 Vision 模型
- **文件附件**：选择 PDF/文本文件，后端提取文本内容供模型阅读
- **消息渲染**：历史消息中正确展示图片缩略图和文件引用
- **模型能力感知**：仅 Vision 模型显示上传入口，非 Vision 模型隐藏

**设计原则**：
- **标记协议**（参考 OpenHuman）：附件编码为 `[IMAGE:…]` / `[FILE:…]` 标记嵌入 `content: String`，发送接口签名不变，全链路改动最小
- **磁盘存储**（参考 OpenHuman AttachmentStash）：入库前将 `data:` URI 替换为 `[Image: image #att:<sha256>]` 占位符，原始字节写磁盘，避免 SQLite 行膨胀
- **按需 Rehydrate**（参考 OpenHuman）：仅 Vision 模型 dispatch 时从磁盘读回图片数据，文本模型保持占位符
- **渐进交付**：Phase 1 前端可用（base64 直存）→ Phase 2 后端 stash 优化（磁盘存储 + TTL 清扫）

---

## 2. 参考项目做法

### 2.1 OpenHuman — 标记协议 + AttachmentStash

**前端**（`attachments.ts`）：

- 附件编码为文本内嵌标记：`buildMessageWithAttachments(text, attachments)` 将图片编码为 `[IMAGE:<data-uri>]`、文件编码为 `[FILE:<data-uri>]`、视频帧编码为多个 `[IMAGE:]` 标记
- 发送接口签名不变（`content: string`），但内容中携带标记
- `parseMessageImages(content)` 从存储消息中解析标记，提取 `dataUris` 用于渲染
- 附件校验：MIME 白名单 + 大小限制（图片 8MB / 文件 16MB / 视频 50MB）+ 数量限制（图片 4 / 文件 4）

**Rust 后端**（`multimodal.rs`）：

- **Ingress 管道** `stash_image_attachments()`：入库前解析 `[IMAGE:data:…]` 标记 → 计算 `sha256_prefix(data_uri)` 作为 ID → 将 `data:` URI 写入磁盘 `<workspace>/attachments/<id>` → 替换标记为 `[Image: image #att:<id>]` 占位符（~50 字节）
- **Dispatch 管道** `rehydrate_image_placeholders()`：发给 Vision 模型前，从磁盘读回 `data:` URI → 替换占位符为 `[IMAGE:<data-uri>]` 完整标记；非 Vision 模型保持占位符不动
- **AttachmentStash**：`ATTACHMENTS_MAX_BYTES = 256MB` 总量上限 + `ATTACHMENTS_TTL = 24h` 过期清扫 + 启动时 `sweep_stale_attachments()` + 写入后按 mtime 驱逐最旧文件至总量以下
- **文件处理** `inline_file_attachments()`：PDF/文本提取后内联为文本块，非提取型文件（DOCX/XLSX/ZIP）仅保留引用存根
- **安全**：不可信通道 `max_files: 0` 哨兵 + 路径遍历防护 + MIME 白名单 + 绝对路径拒绝

**存储位置**：`<workspace>/attachments/`，其中 workspace 为 `init_attachments_dir(cfg.workspace_dir.join("attachments"))`。回退目录为 `~/Library/Caches/openhuman-attachments`（macOS）或 `~/.cache/openhuman-attachments`（Linux）。

### 2.2 OpenBitFun — ImageContext 结构化对象

**前端**（`imagePayload.ts` + `imageUtils.ts`）：

- `ImageContext` 结构化对象携带 `id` / `name` / `dataUrl?` / `imagePath?` / `mimeType`
- `createImageContextFromFile()` 从文件选择器创建
- `createImageContextFromClipboard()` 从剪贴板粘贴创建
- `generateThumbnail()` Canvas 缩略图生成
- `validateImageFile()` MIME + 大小校验
- 发送时调用 Tauri command `upload_image_contexts`，后端返回 `{ id, image_path }`

**Rust 后端**（`host.rs`）：

- `is_local_image_path(path)` 校验：非空 + 不含 `://` + 扩展名合法（png/jpg/jpeg/gif/webp）
- `local_image_attachment(image_path, workspace_path)` 构建附件：相对路径解析为绝对路径 + MIME 推断
- `AgentInputAttachment::image_context()` 工厂方法

**存储位置**：本地文件路径引用（`image_path`），不复制到专用目录。图片可以是 workspace 内已有文件（如截图），也可以是上传后存入 `<workspace>/uploads/` 的文件。

### 2.3 AIRI — image-projection 描述降级

**前端**（`image-projection.ts`）：

- `describeChatImages()`：用户图片在发给 Provider 前由 Vision 模型生成文字描述，替换为 `[Image description, supplied as user content]\n{description}\n[End image description]` 文本块。持久化历史保留原始图片，仅 Provider prompt 中替换
- `replaceToolResultImages()`：工具结果中的图片替换为 `A tool image was left out of the history.` 文本注释，避免非 Vision 模型收到无法处理的图片

**启示**：对于非 Vision 模型，图片不应直接发送，应降级为描述或占位文本。OpenHuman 的 rehydrate 策略（仅 Vision 模型 rehydrate）已覆盖此场景。

---

## 3. 图片存在哪里

### 3.1 设计决策：磁盘存储（参考 OpenHuman AttachmentStash）

**不存 SQLite**。原因（OpenHuman 实测经验）：

| 方案 | SQLite 行体积 | 内存自动保存 | 跨线程 JSONL 索引 | Prompt 注入扫描 | 进程重启 |
|------|-------------|------------|------------------|----------------|---------|
| base64 直存 SQLite | 每图 ~1.3× 原始 MB | N chunks 嵌入膨胀 | 索引行膨胀 | 扫描 MB 级 base64 | ✅ |
| 磁盘 + 占位符 | 每图 ~50 字节 | 轻量 | 轻量 | 扫描 ~50 字节占位符 | ✅ |

### 3.2 存储路径

```
<app_data_dir>/attachments/<sha256_prefix>
```

其中：
- `<app_data_dir>` = Tauri `app.path().app_data_dir()`，当前项目已在 `setup.rs` 中使用此路径（数据库也在此目录）
- `<sha256_prefix>` = `sha256(data_uri_bytes)[..16]`（取 SHA-256 前 16 字符作为文件名，参考 OpenHuman `sha256_prefix`）

**完整路径示例**：
- Windows: `C:\Users\<user>\AppData\Roaming\com.assistant.app\attachments\a1b2c3d4e5f6g7h8`
- macOS: `~/Library/Application Support/com.assistant.app/attachments/a1b2c3d4e5f6g7h8`
- Linux: `~/.local/share/com.assistant.app/attachments/a1b2c3d4e5f6g7h8`

### 3.3 AttachmentStash 策略（参考 OpenHuman）

| 参数 | 值 | 说明 |
|------|-----|------|
| `ATTACHMENTS_MAX_BYTES` | 256 MB | 磁盘附件总量上限，超出按 mtime 驱逐最旧文件 |
| `ATTACHMENTS_TTL` | 24 h | 附件过期时间，启动时清扫 |
| `ATTACHMENTS_DIR` | `<app_data_dir>/attachments` | 专用目录，与 `assistant.db` 同级 |
| 文件名 | `sha256_prefix(data_uri)[..16]` | 原子去重：相同图片相同 ID，不重复写入 |
| 写入方式 | 原子写入（先写 `.tmp` 再 `rename`） | 避免并发读写半写文件 |

### 3.4 消息存储格式

**SQLite `messages.content` 列**存的是占位符，非 base64：

```
这是用户的消息文本
[Image: image #att:a1b2c3d4e5f6g7h8]
[File: document.pdf #att:b2c3d4e5f6g7h8i9]
```

- 图片占位符：`[Image: image #att:<id>]`（~40 字节）
- 文件占位符：`[File: <filename> #att:<id>]`（~50 字节）
- 前端渲染时通过 `parseMessageImages()` 解析占位符，从磁盘读取图片数据用于展示

---

## 4. 设计方案

### 4.1 架构总览

```
┌──────────────────────────────────────────────────────────────────────────┐
│  前端输入层                                                              │
│  MessageInput → 附件按钮 + 粘贴 + 拖拽 → Attachment[]                   │
│  buildMessageWithAttachments(text, attachments) → content 含标记         │
├──────────────────────────────────────────────────────────────────────────┤
│  前端发送层                                                              │
│  chatService.sendChatMessage(convId, content)  ← 签名不变               │
├──────────────────────────────────────────────────────────────────────────┤
│  Tauri Command 层                                                       │
│  send_chat_message(conv_id, content, ...)  ← 签名不变                   │
├──────────────────────────────────────────────────────────────────────────┤
│  Rust Ingress 管道                                                      │
│  stash_image_attachments(content) → 占位符 + 磁盘写入                   │
│  inline_file_attachments(content) → 文本提取 + 占位符                    │
├──────────────────────────────────────────────────────────────────────────┤
│  SQLite 存储                                                            │
│  messages.content = "用户文本\n[Image: image #att:a1b2c3d4...]"         │
├──────────────────────────────────────────────────────────────────────────┤
│  Rust Dispatch 管道（仅 Vision 模型）                                    │
│  rehydrate_image_placeholders(messages) → 从磁盘读回 data:URI            │
├──────────────────────────────────────────────────────────────────────────┤
│  Provider 消息转换                                                       │
│  OpenAI → content_parts [{type:"text"}, {type:"image_url"}]             │
│  Anthropic → content [{type:"text"}, {type:"image",source:{base64}}]    │
│  Gemini → parts [{text}, {inline_data}]                                 │
│  Ollama → images[] + prompt                                             │
├──────────────────────────────────────────────────────────────────────────┤
│  前端渲染层                                                              │
│  parseMessageImages(content) → {text, dataUris}                         │
│  MessageContent → <img> for dataUris + Markdown for text                │
└──────────────────────────────────────────────────────────────────────────┘
```

### 4.2 前端类型定义

#### 4.2.1 附件类型（新增 `src/chat/core/attachmentTypes.ts`）

```typescript
const ALLOWED_IMAGE_MIME_TYPES = [
  'image/png',
  'image/jpeg',
  'image/webp',
  'image/gif',
] as const;

export type AllowedImageMimeType = (typeof ALLOWED_IMAGE_MIME_TYPES)[number];

const ALLOWED_FILE_MIME_TYPES = [
  'application/pdf',
  'text/plain',
  'text/markdown',
  'text/csv',
] as const;

export type AllowedFileMimeType = (typeof ALLOWED_FILE_MIME_TYPES)[number];

export type AttachmentKind = 'image' | 'file';

export interface Attachment {
  id: string;
  kind: AttachmentKind;
  file: File;
  dataUri: string;
  previewUri?: string;
  mimeType: AllowedImageMimeType | AllowedFileMimeType;
  originalSizeBytes: number;
}

export const ATTACHMENT_MAX_IMAGES = 4;
export const ATTACHMENT_MAX_FILES = 4;
export const ATTACHMENT_MAX_IMAGE_SIZE_BYTES = 8 * 1024 * 1024;
export const ATTACHMENT_MAX_FILE_SIZE_BYTES = 16 * 1024 * 1024;
```

#### 4.2.2 消息类型（变更 `src/chat/core/chatTypes.ts`）

```typescript
export interface Message {
  // ... 现有字段不变 ...
  // content 仍为 string，但可包含 [IMAGE:] / [FILE:] 标记
  // 新增：前端解析后的附件信息（仅用于渲染，不持久化）
  imageUris?: string[];
  fileReferences?: FileReference[];
}

export interface FileReference {
  filename: string;
  attachmentId: string;
}
```

### 4.3 前端附件工具（新增 `src/chat/core/attachmentUtils.ts`）

参考 OpenHuman `attachments.ts`，核心函数：

```typescript
export function buildMessageWithAttachments(text: string, attachments: Attachment[]): string

export function parseMessageImages(content: string): { text: string; dataUris: string[] }

export function parseMessageFiles(content: string): { text: string; files: FileReference[] }

export async function validateAndReadFile(file: File): Promise<{ attachment: Attachment } | { error: AttachmentError }>

export function formatFileSize(bytes: number): string
```

**`buildMessageWithAttachments` 实现**（参考 OpenHuman）：

```typescript
export function buildMessageWithAttachments(text: string, attachments: Attachment[]): string {
  if (attachments.length === 0) return text;
  const markers = attachments
    .map(a => {
      if (a.kind === 'image') return `[IMAGE:${a.dataUri}]`;
      if (a.kind === 'file') return `[FILE:${a.dataUri}]`;
      return '';
    })
    .filter(marker => marker.length > 0)
    .join(' ');
  return text.trim() ? `${text.trim()} ${markers}` : markers;
}
```

**`parseMessageImages` 实现**（参考 OpenHuman）：

```typescript
export function parseMessageImages(content: string): { text: string; dataUris: string[] } {
  const dataUris: string[] = [];
  const text = content
    .replace(/\[IMAGE:([^\]]+)\]/g, (_match, uri: string) => {
      dataUris.push(uri);
      return '';
    })
    .replace(/\[FILE:([^\]]+)\]/g, '')
    .replace(/ {2,}/g, ' ')
    .trim();
  return { text, dataUris };
}
```

### 4.4 前端输入组件变更

**`MessageInput.tsx` 变更**：

- 新增附件按钮（📎 图标，参考 OpenHuman `ComposerPrimitive.AddAttachment`）
- 新增粘贴图片支持（`onPaste` 事件检测 `clipboardData.items` 中 image 类型）
- 新增拖拽附件支持（`onDrop` 事件检测 `dataTransfer.files`）
- 新增附件预览条（输入框上方，参考 OpenHuman `ComposerPrimitive.Attachments`）
- 附件预览条中每个附件显示缩略图/文件名 + 删除按钮
- `onSend` 签名不变：`onSend: (content: string) => void`，但 content 中包含标记

**模型能力感知**：

```typescript
interface ModelConfig {
  // ... 现有字段 ...
  supportVision: boolean;
}
```

- `supportVision = true`：显示附件按钮，允许图片附件
- `supportVision = false`：隐藏附件按钮，粘贴/拖拽图片时提示"当前模型不支持图片"

### 4.5 前端渲染组件变更

**`MessageContent.tsx` 变更**：

用户消息渲染逻辑变更：

```typescript
function UserMessageContent({ content }: { content: string }) {
  const { text, dataUris } = parseMessageImages(content);
  return (
    <div className="user-message-content">
      {dataUris.length > 0 && (
        <div className="attachment-grid">
          {dataUris.map((uri, i) => (
            <img key={i} src={uri} className="attachment-image" />
          ))}
        </div>
      )}
      {text && <p>{text}</p>}
    </div>
  );
}
```

助手消息渲染不变（助手不返回图片标记，仅文本 + 工具调用）。

### 4.6 Rust 后端变更

#### 4.6.1 AttachmentStash（新增 `assistant-storage/src/attachments.rs`）

参考 OpenHuman `AttachmentStash`，核心结构：

```rust
pub struct AttachmentStash {
    dir: PathBuf,
    max_bytes: u64,
    ttl: Duration,
}

impl AttachmentStash {
    pub fn new(dir: PathBuf, max_bytes: u64, ttl: Duration) -> Self;

    pub async fn write(&self, id: &str, data: &str) -> Result<PathBuf>;

    pub async fn read(&self, id: &str) -> Result<String>;

    pub fn build_index(&self) -> HashMap<String, PathBuf>;

    pub async fn sweep_stale(&self);

    fn evict_oldest(&self);
}
```

**常量**：

```rust
const ATTACHMENTS_MAX_BYTES: u64 = 256 * 1024 * 1024;
const ATTACHMENTS_TTL: Duration = Duration::from_secs(24 * 60 * 60);
```

#### 4.6.2 Ingress 管道（新增 `assistant-agent/src/multimodal.rs`）

```rust
pub async fn stash_image_attachments(
    message: &str,
    image_config: &MultimodalConfig,
) -> String;

pub fn inline_file_attachments(
    message: &str,
    file_config: &MultimodalFileConfig,
) -> Result<String>;
```

**`stash_image_attachments` 流程**（参考 OpenHuman）：

1. `parse_image_markers(message)` → `(cleaned_text, image_refs)`
2. 对每个 `image_ref`：
   - 校验数量 ≤ `max_images`（默认 4）
   - 校验大小 ≤ `max_image_bytes`（默认 8MB）
   - 计算 `id = sha256_prefix(data_uri_bytes)[..16]`
   - `stash().write(&id, &data_uri)` → 写入磁盘
   - 替换标记为 `[Image: image #att:{id}]` 占位符
3. 返回替换后的消息

#### 4.6.3 Dispatch 管道

```rust
pub fn rehydrate_image_placeholders(messages: &[ChatMessage]) -> Vec<ChatMessage>;
```

**流程**（参考 OpenHuman）：

1. 构建 `index = stash().build_index()`（一次磁盘扫描）
2. 对每条消息的 `content`，替换 `[Image: image #att:{id}]` 为 `[IMAGE:{data_uri}]`（从磁盘读回）
3. 占位符 ID 不存在（文件被驱逐/清扫）→ 保持占位符文本
4. **仅 Vision 模型调用此函数**，非 Vision 模型保持占位符

#### 4.6.4 初始化

在 `setup.rs` 的 `init()` 中，数据库初始化之后新增：

```rust
let attachments_dir = data_dir.join("attachments");
std::fs::create_dir_all(&attachments_dir)?;
assistant_agent::multimodal::init_attachments_dir(attachments_dir);
```

#### 4.6.5 Provider 消息转换器变更

**OpenAI**（`openai/message_converter.rs`）：

当前 `json!({"content": m.content})` → 改为检测 `[IMAGE:]` 标记，生成 `content_parts` 数组：

```rust
fn convert_user_message(msg: &ChatMessage) -> Value {
    let (text, image_refs) = parse_image_markers(&msg.content);
    if image_refs.is_empty() {
        return json!({"role": "user", "content": text});
    }
    let mut parts = vec![json!({"type": "text", "text": text})];
    for image_ref in &image_refs {
        if let Some(url) = extract_image_url(image_ref) {
            parts.push(json!({
                "type": "image_url",
                "image_url": { "url": url }
            }));
        }
    }
    json!({"role": "user", "content": parts})
}
```

**Anthropic**（`anthropic/message_converter.rs`）：

移除 `tool_call_id` hack，改为正规 `[IMAGE:]` 标记处理：

```rust
fn convert_user_message(msg: &ChatMessage) -> Value {
    let (text, image_refs) = parse_image_markers(&msg.content);
    let mut content_blocks = vec![json!({"type": "text", "text": text})];
    for image_ref in &image_refs {
        if let Some((media_type, data)) = parse_image_source(image_ref) {
            content_blocks.push(json!({
                "type": "image",
                "source": { "type": "base64", "media_type": media_type, "data": data }
            }));
        }
    }
    json!({"role": "user", "content": content_blocks})
}
```

**Gemini**（`gemini/message_converter.rs`）：

扩展 `try_parse_inline_image` 处理 `[IMAGE:]` 标记，生成 `inlineData` part。

#### 4.6.6 prompt_builder 修复

当前 `prompt_builder.rs` 从 `MessageRow` 构建 `ChatMessage` 时丢失 `tool_calls` 信息。修复：

```rust
.map(|m| ChatMessage {
    role: base::message_role_from_str(&m.role),
    content: m.content.clone(),
    tool_calls: serde_json::from_str(&m.tool_calls_json.clone().unwrap_or_default())
        .ok()
        .unwrap_or_default(),
    tool_call_id: None,
    // ...
})
```

### 4.7 MultimodalConfig

```rust
pub struct MultimodalConfig {
    pub max_images: usize,
    pub max_image_size_mb: u32,
    pub max_files: usize,
    pub max_file_size_mb: u32,
    pub allow_remote_fetch: bool,
}

impl Default for MultimodalConfig {
    fn default() -> Self {
        Self {
            max_images: 4,
            max_image_size_mb: 8,
            max_files: 4,
            max_file_size_mb: 16,
            allow_remote_fetch: false,
        }
    }
}
```

不可信通道配置（参考 OpenHuman `for_untrusted_channel_input`）：

```rust
impl MultimodalFileConfig {
    pub fn for_untrusted_channel_input() -> Self {
        Self {
            max_files: 0,  // 哨兵：禁止文件附件
            ..Default::default()
        }
    }
}
```

---

## 5. 实施步骤

### Phase 1：前端可用（base64 直存 SQLite）

> 最小可用版本，不依赖 Rust 后端 stash 改造。图片以 `[IMAGE:data:…]` 标记直接存入 SQLite `messages.content`。

| Step | 内容 | 涉及文件 |
|------|------|---------|
| **1a** | 新增 `attachmentTypes.ts` + `attachmentUtils.ts` | `src/chat/core/attachmentTypes.ts` (新), `src/chat/core/attachmentUtils.ts` (新) |
| **1b** | `MessageInput.tsx` 添加附件按钮 + 粘贴 + 拖拽 + 预览条 | `src/chat/message/MessageInput.tsx` |
| **1c** | `chatStore.sendMessage` 中调用 `buildMessageWithAttachments` | `src/stores/chatStore.ts` |
| **1d** | `MessageContent.tsx` 渲染 `[IMAGE:]` 为 `<img>` | `src/chat/message/MessageContent.tsx` |
| **1e** | `chatTypes.ts` 新增 `imageUris` / `fileReferences` 字段 | `src/chat/core/chatTypes.ts` |
| **1f** | 模型配置新增 `supportVision` 字段 | `src/stores/providerStore.ts`, Rust `ModelConfig` |

**Phase 1 验收**：用户可粘贴/拖拽/选择图片 → 发送 → 消息中显示图片 → 历史消息加载后图片仍显示

### Phase 2：后端 Stash 优化（磁盘存储 + TTL 清扫）

> Phase 1 可用后实施。将 SQLite 中的 base64 替换为磁盘占位符，解决行膨胀问题。

| Step | 内容 | 涉及文件 |
|------|------|---------|
| **2a** | 新增 `attachments.rs`（AttachmentStash） | `src-tauri/crates/assistant-storage/src/attachments.rs` (新) |
| **2b** | 新增 `multimodal.rs`（stash + rehydrate + inline_file） | `src-tauri/crates/assistant-agent/src/multimodal.rs` (新) |
| **2c** | `setup.rs` 初始化 attachments 目录 | `src-tauri/src/setup.rs` |
| **2d** | `send_chat_message` 中调用 `stash_image_attachments` | `src-tauri/src/cmd/chat.rs` |
| **2e** | dispatch 前调用 `rehydrate_image_placeholders`（仅 Vision 模型） | `src-tauri/crates/assistant-agent/src/chat_runner.rs` |
| **2f** | Provider 消息转换器正确处理 `[IMAGE:]` 标记 | 3 个 `message_converter.rs` |
| **2g** | `prompt_builder.rs` 修复 tool_calls 丢失 | `src-tauri/crates/assistant-agent/src/prompt_builder.rs` |
| **2h** | 前端 `parseMessageImages` 适配占位符格式 + Tauri command 读取附件 | `src/chat/core/attachmentUtils.ts`, `src-tauri/src/cmd/chat.rs` |

**Phase 2 验收**：发送图片后 SQLite `messages.content` 中为占位符（~50 字节）；磁盘 `<app_data_dir>/attachments/` 中有图片文件；Vision 模型 dispatch 时图片正确发送；24h 后启动清扫过期附件

### Phase 3：文件附件 + 安全加固

| Step | 内容 | 涉及文件 |
|------|------|---------|
| **3a** | `inline_file_attachments` 实现（PDF 提取 + 文本内联） | `src-tauri/crates/assistant-agent/src/multimodal.rs` |
| **3b** | 前端文件选择器（非图片文件） | `src/chat/message/MessageInput.tsx` |
| **3c** | 不可信通道 `max_files: 0` 哨兵 | `src-tauri/crates/assistant-agent/src/multimodal.rs` |
| **3d** | 路径遍历防护 + MIME 白名单严格校验 | `src-tauri/crates/assistant-agent/src/multimodal.rs` |

**Phase 3 验收**：可选择 PDF/文本文件 → 发送 → 模型可阅读文件内容；不可信通道拒绝文件附件

---

## 6. 与现有体系的关系

| 现有体系 | I2e 变更 | 兼容性 |
|---------|---------|--------|
| `ChatMessage.content: String` | 不变，标记协议保持 String 类型 | ✅ 完全兼容 |
| `MessageRow.content: TEXT` | 不变，存占位符或标记 | ✅ 完全兼容 |
| `send_chat_message(conv_id, content, ...)` | 签名不变，content 含标记 | ✅ 完全兼容 |
| `chatStore.sendMessage(content: string)` | 签名不变，content 含标记 | ✅ 完全兼容 |
| `ContentBlockType::Image` | 已有定义，Phase 2 启用 | ✅ 已预留 |
| `anthropic/message_converter.rs` tool_call_id hack | Phase 2 移除，改为标记处理 | 🔄 行为变更，更正确 |
| `gemini/message_converter.rs` try_parse_inline_image | Phase 2 扩展，处理标记 | 🔄 行为增强 |
| `prompt_builder.rs` | 修复 tool_calls 丢失 bug | 🔄 bug 修复 |
| `MessageInput.tsx` | 新增附件 UI | 🔄 UI 增强 |
| `MessageContent.tsx` | 新增图片渲染 | 🔄 UI 增强 |

---

## 7. 验收标准

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
| V9 | Phase 2：SQLite `messages.content` 中为 `[Image: image #att:…]` 占位符 | 查询数据库验证 |
| V10 | Phase 2：磁盘 `<app_data_dir>/attachments/` 中有图片文件 | 检查文件系统 |
| V11 | Phase 2：24h 后重启 → 过期附件被清扫 | 修改文件 mtime 为 25h 前，重启 |
| V12 | Phase 2：附件总量超 256MB → 最旧文件被驱逐 | 写入大量附件 |
| V13 | Phase 3：PDF 文件 → 模型可阅读 PDF 内容 | 发送 PDF + "总结这个文件" |
| V14 | Phase 3：不可信通道 → 文件附件被拒绝 | MCP/外部输入携带文件标记 |
| V15 | 历史消息加载后图片仍正确显示 | 重启应用，打开有图片的对话 |
| V16 | Anthropic 转换器不再使用 tool_call_id hack | 代码审查 + 发送图片到 Anthropic 模型 |
| V17 | prompt_builder 保留 tool_calls 信息 | 发送含工具调用的对话，重发时工具调用不丢失 |
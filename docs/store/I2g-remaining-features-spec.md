# I2g 剩余功能规格 — I2e Phase 2/3 + I2f 补丁

> **前置**：I2e Phase 1 ✅ + I2f ✅（图片附件完整链路 + 关联索引管理）
> **目标**：补全文件附件链路、模型能力声明升级、安全加固、图片规范化、offload 降级、小修补
> **排除**：`prompt_builder` tool_calls 修复（由后续文档引入，不属于本范围）

---

## 0.0 核心设计原则：`inputModalities` 驱动投影

**所有投影决策统一由 `inputModalities: Vec<String>` 驱动**，而非逐个布尔开关。

models-dev.json 中当前活跃模型的 `inputModalities` 组合：

| 组合 | Provider | 模型系列 |
|------|----------|---------|
| `text + image` | OpenAI | GPT-4o / GPT-4.1 |
| `text + image + pdf` | Anthropic | Claude 全系列 |
| `text + image + video + audio + pdf` | Google | Gemini 2.5/3.5 |
| `text + image + video` | MiniMax / GLM / Kimi | MiniMax-M3 / GLM-4.5V~5V / K2.5~K2.7 |
| `text + image + video + audio + pdf` | GLM | GLM-5V-Turbo |

**投影决策表**（统一公式）：

| ContentBlock | 模型能力匹配 | 投影结果 |
|-------------|-------------|---------|
| ImageBlock | `image ∈ modalities` | rehydrate → 原生发送 |
| ImageBlock | `image ∉ modalities` | → TextBlock（placeholder） |
| FileBlock(PDF) | `pdf ∈ modalities` | rehydrate → 原生发送 |
| FileBlock(PDF) | `pdf ∉ modalities` | → TextBlock（file_handle_text） |
| FileBlock(非PDF) | 任何 | → TextBlock（file_handle_text） |
| VideoBlock | `video ∈ modalities` | rehydrate → 原生发送 |
| VideoBlock | `video ∉ modalities` | → TextBlock（placeholder） |
| AudioBlock | `audio ∈ modalities` | rehydrate → 原生发送 |
| AudioBlock | `audio ∉ modalities` | → TextBlock（placeholder） |

> **I2g 范围**：实现 ImageBlock + FileBlock 投影。VideoBlock / AudioBlock 为**未来扩展点**，当前不实现，但架构必须不排斥——`inputModalities` 已包含 `video` / `audio` 值，`ContentBlock` enum 可扩展新变体，投影框架无需重构。

`support_vision` / `support_pdf` 等布尔值仅为**便捷派生**，不是决策源：

```rust
let support_vision = input_modalities.contains(&"image".to_string());
let support_pdf    = input_modalities.contains(&"pdf".to_string());
let support_video  = input_modalities.contains(&"video".to_string());
let support_audio  = input_modalities.contains(&"audio".to_string());
```

---

## 0. 遗留清单总览

| # | 来源 | 功能 | 优先级 | 工作量 | 依赖 |
|---|------|------|--------|--------|------|
| 1 | I2e-P1 | ImageThumbnail 加载失败重试按钮 | 低 | 0.5h | 无 |
| 2 | I2e-P2 | `project_files` FileBlock 能力感知投影 | **高** | 2h | 3,4 |
| 3 | I2e-P2 | 前端文件选择器（PDF/文本） | **高** | 3h | 无 |
| 4 | I2e-P2 | `inputModalities` 替代 `supportVision` | **高** | 2h | 无 |
| 5 | I2e-P2 | 不可信通道安全哨兵 | 中 | 1.5h | 2 |
| 6 | I2e-P3 | Offload 降级 | 低 | 3h | 无 |
| 7 | I2e-P3 | 图片规范化 | 中 | 3h | 无 |
| 8 | I2e-P3 | 降级文本函数抽取 | 低 | 1h | 2 |
| 9 | I2f | `created_at` 从磁盘 mtime 读取 | 低 | 0.5h | 无 |

**建议实施顺序**：4→3→2→5→1→9→7→6→8

> #4（inputModalities）是 #2（投影）的前置依赖——投影的 `support_pdf` / `support_video` / `support_audio` 全部从 `inputModalities` 派生。

---

## 1. ImageThumbnail 加载失败重试按钮（#1）

### 1.1 现状

[ImageThumbnail.tsx:117-128](../../src/chat/message/ImageThumbnail.tsx#L117-L128) — `error` 状态渲染与 `offloaded` 完全相同的占位 UI（图标 + 文案），无重试交互。

### 1.2 变更

**`src/chat/message/ImageThumbnail.tsx`**：

`error` 分支从复用 `.offloaded` 改为独立渲染，增加重试按钮：

```tsx
if (error) {
  return (
    <div className={css.error} style={aspectStyle}>
      <FileImageOutlined className={css.errorIcon} />
      <span className={css.errorText}>{t("chat.input.image_not_found")}</span>
      <button
        type="button"
        className={css.retryBtn}
        onClick={() => { setError(false); setLoaded(false); }}
      >
        <ReloadOutlined />
      </button>
    </div>
  );
}
```

重试逻辑：`setError(false)` + `setLoaded(false)` 触发 `<img>` 重新加载（`src` 不变，浏览器重新请求）。

**`src/chat/message/ImageThumbnail.module.css`**：

```css
.error {
  /* 同 .offloaded 布局，但增加按钮空间 */
  display: inline-flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 80px;
  min-width: 120px;
  padding: 12px;
  border-radius: var(--radius);
  background: var(--color-bg-layout);
  gap: 6px;
}

.errorIcon { /* 同 .offloadedIcon */ }
.errorText { /* 同 .offloadedText */ }

.retryBtn {
  margin-top: 4px;
  padding: 2px 8px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-secondary);
  cursor: pointer;
  font-size: 12px;
}
.retryBtn:hover {
  background: var(--color-bg-secondary);
}
```

**新增 import**：`ReloadOutlined` from `@ant-design/icons`。

---

## 2. `project_files_to_text` FileBlock 投影（#2）

### 2.1 现状

- multimodal.rs 仅有 `project_images_for_text_model`，无 FileBlock 投影
- 3× Provider 转换器中 `ContentBlock::File` 分支均走 `_ => json!({})` / `_ => None`，**静默丢弃 FileBlock**
- 前端 FileCard.tsx 已实现渲染骨架，但后端发送时 FileBlock 信息丢失

### 2.2 设计

**核心原则**：投影是**能力感知**的——根据模型 `inputModalities` 决定 FileBlock 如何送达 Provider。

**投影策略**（与 ImageBlock 完全对称）：

```
FileBlock 投影策略：
├─ 模型支持 pdf 且文件为 PDF → rehydrate PDF bytes，Provider 转换器生成原生格式
├─ 模型支持 pdf 但文件非 PDF → 投影为 TextBlock（file_handle_text）
└─ 模型不支持 pdf → 全部 FileBlock 投影为 TextBlock（file_handle_text）
```

**models-dev.json 中支持 `pdf` 输入的模型**（截至 2026-10）：

| Provider | 模型系列 |
|----------|---------|
| Anthropic | claude-sonnet-4-5、claude-haiku-4-5、claude-opus-4-1、claude-opus-5 等全部当前活跃 Claude |
| Google | gemini-2.5-pro、gemini-2.5-flash 等 Gemini 2.5 系列 |
| OpenAI | ❌ 不支持（GPT-4o 系列仅 `text + image`） |

> **注意**：`inputModalities`（#4）是投影能力感知的前置。在 #4 完成前，`support_pdf` 可从 `model_catalog` 独立查询，与 `support_vision` 同模式。

### 2.3 Rust 变更

**`src-tauri/crates/assistant-agent/src/multimodal.rs`** — 新增函数：

```rust
/// 能力感知的文件投影：
/// - support_pdf=true 且文件为 PDF → 不投影（后续 rehydrate_pdf 填充 bytes）
/// - 其余 FileBlock → 投影为 TextBlock（file_handle_text）
pub fn project_files(messages: &mut [ChatMessage], support_pdf: bool) {
    for msg in messages.iter_mut() {
        if msg.role != MessageRole::User {
            continue;
        }

        // 优先操作 content_blocks（rehydrate 后的实际数据源）
        // 兜底操作 content（content_blocks 为 None 时）
        let blocks = msg.content_blocks.take()
            .or_else(|| parse_content(&msg.content));

        let Some(blocks) = blocks else {
            continue;
        };

        let projected: Vec<ContentBlock> = blocks
            .into_iter()
            .map(|block| match block {
                ContentBlock::File { ref attachment } if !should_send_file_as_is(attachment, support_pdf) => {
                    ContentBlock::Text {
                        text: file_handle_text(&attachment),
                    }
                }
                other => other,
            })
            .collect();

        if let Ok(json) = serde_json::to_string(&projected) {
            msg.content = json;
        }
        msg.content_blocks = Some(projected);
    }
}

fn should_send_file_as_is(att: &FileAttachmentRef, support_pdf: bool) -> bool {
    if !support_pdf { return false; }
    // 仅 PDF 文件可原样发送；其余文件类型始终投影
    att.name.to_lowercase().ends_with(".pdf")
}
```

**PDF rehydrate**（当 `support_pdf=true` 时，为 PDF FileBlock 填充 bytes）：

```rust
pub async fn rehydrate_files(messages: &mut [ChatMessage], store: &AttachmentStore, support_pdf: bool) {
    if !support_pdf { return; }
    for msg in messages.iter_mut() {
        if msg.role != MessageRole::User { continue; }
        let Some(mut blocks) = parse_content(&msg.content) else { continue; };

        let mut changed = false;
        for block in blocks.iter_mut() {
            let ContentBlock::File { ref attachment, .. } = block else { continue; };
            if !should_send_file_as_is(attachment, true) { continue; }

            match store.read(&attachment.attachment_id).await {
                Ok(data) => {
                    // FileBlock 不像 ImageBlock 有 cached_data 字段
                    // 需要转为 Provider 可消费的形式——在 content_blocks 中标记
                    // 具体方式见 2.4 Provider 转换器
                    changed = true;
                }
                Err(e) => {
                    log::warn!("Failed to rehydrate file {}: {}", attachment.attachment_id, e);
                    *block = ContentBlock::Text {
                        text: assistant_i18n::t("chat.file_not_found_placeholder"),
                    };
                    changed = true;
                }
            }
        }

        if changed {
            if let Ok(json) = serde_json::to_string(&blocks) {
                msg.content = json;
            }
            msg.content_blocks = Some(blocks);
        }
    }
}
```

> **FileBlock rehydrate 的关键问题**：当前 `ContentBlock::File` 没有 `cached_data` 字段（不像 `ContentBlock::Image`）。Provider 转换器需要 PDF bytes 才能生成 API 格式。两种方案：
>
> **方案 A**：给 `ContentBlock::File` 新增 `cached_data: Option<String>` 字段（与 ImageBlock 对称），rehydrate 时填充 base64。**推荐**——与 ImageBlock 模式一致，Provider 转换器逻辑清晰。
>
> **方案 B**：Provider 转换器内部按 `attachment_id` 调用 `store.read()`。耦合 Store，不推荐。
>
> 采用**方案 A**。

**`ContentBlock::File` 扩展**（message.rs）：

```rust
File {
    #[serde(rename = "attachment")]
    attachment: FileAttachmentRef,
    #[serde(skip_serializing)]
    cached_data: Option<String>,   // 新增：rehydrate 后填充 base64
},
```

前端 `FileBlock` 类型同步扩展：

```typescript
export interface FileBlock {
  type: 'file'
  attachment: FileAttachmentRef
  cached_data?: string   // 新增（前端不使用，Rust serde 透传用）
}
```

**`src-tauri/src/cmd/chat.rs`** — Dispatch 管道重构：

```rust
// 现有
if support_vision {
    multimodal::rehydrate_images(&mut chat_messages, &attachment_store).await;
} else {
    multimodal::project_images_for_text_model(&mut chat_messages);
}

// 新增：文件 rehydrate + 投影
let support_pdf = /* 从 model_catalog 或 inputModalities 查询，见 #4 */;
multimodal::rehydrate_files(&mut chat_messages, &attachment_store, support_pdf).await;
multimodal::project_files(&mut chat_messages, support_pdf);
```

### 2.4 Provider 转换器 — FileBlock 原生支持

当 `support_pdf=true` 且 `cached_data` 有值时，Provider 转换器将 PDF 转为 API 原生格式：

**Anthropic**（message_converter.rs）— Claude 原生支持 PDF document：

```rust
ContentBlock::File { attachment, cached_data, .. } => {
    let data = cached_data.as_deref().unwrap_or("");
    if data.is_empty() {
        // 防御性兜底：无 cached_data 时投影为文本
        json!({ "type": "text", "text": file_handle_text(&attachment) })
    } else if attachment.name.to_lowercase().ends_with(".pdf") {
        json!({
            "type": "document",
            "source": {
                "type": "base64",
                "media_type": "application/pdf",
                "data": data
            }
        })
    } else {
        json!({ "type": "text", "text": file_handle_text(&attachment) })
    }
}
```

**Gemini**（message_converter.rs）— Gemini inlineData 支持 PDF：

```rust
ContentBlock::File { attachment, cached_data, .. } => {
    let data = cached_data.as_deref().unwrap_or("");
    if data.is_empty() || !attachment.name.to_lowercase().ends_with(".pdf") {
        Some(json!({ "text": file_handle_text(&attachment) }))
    } else {
        Some(json!({
            "inlineData": {
                "mimeType": "application/pdf",
                "data": data,
            }
        }))
    }
}
```

**OpenAI**（[message_converter.rs](../../src-tauri/crates/assistant-providers/src/openai/message_converter.rs)）— GPT 不支持 PDF，始终投影为文本：

```rust
ContentBlock::File { attachment, .. } => json!({
    "type": "text",
    "text": file_handle_text(&attachment)
}),
```

> **防御性兜底**：即使 `project_files` 理论上已先执行，Provider 转换器仍需处理 FileBlock（防止投影遗漏或未来新增文件类型时静默丢弃）。

---

## 3. 前端文件选择器（#3）

### 3.1 现状

- [useNativeFileAttach.ts:34](../../src/hooks/useNativeFileAttach.ts#L34) — `filters` 仅限图片：`{ name: "Images", extensions: ALLOWED_IMAGE_EXTENSIONS }`
- [attachmentUtils.ts](../../src/chat/core/attachmentUtils.ts) — `EXT_TO_MIME` 仅含图片 MIME，`createAttachmentFromFile` 仅处理 `kind: 'image'`
- [attachmentTypes.ts](../../src/chat/core/attachmentTypes.ts) — `ATTACHMENT_MAX_FILES` / `ATTACHMENT_MAX_FILE_SIZE_BYTES` 常量已定义但未使用

### 3.2 设计

附件按钮拆分为两个入口：
- **图片按钮**（现有）：仅选图片，受 `supportVision` 控制
- **文件按钮**（新增）：选 PDF/文本/Markdown/CSV 等，不受 `supportVision` 限制（文本模型也能读文件投影）

### 3.3 前端变更

#### 3.3.1 `src/chat/core/attachmentTypes.ts`

新增文件 MIME 白名单：

```typescript
export const ALLOWED_FILE_MIME_TYPES = [
  "application/pdf",
  "text/plain",
  "text/markdown",
  "text/csv",
  "text/html",
  "application/json",
  "text/x-python",
  "text/javascript",
  "text/typescript",
  "text/css",
  "text/yaml",
  "text/xml",
] as const;
export type AllowedFileMimeType = (typeof ALLOWED_FILE_MIME_TYPES)[number];

export const ALLOWED_FILE_EXTENSIONS = [
  "pdf", "txt", "md", "csv", "html", "json",
  "py", "js", "ts", "tsx", "css", "yaml", "yml", "xml",
] as const;
```

`Attachment` 接口扩展 `mimeType` 类型：

```typescript
export interface Attachment {
  id: string;
  kind: AttachmentKind;
  file: File;
  previewUrl: string;
  mimeType: AllowedImageMimeType | AllowedFileMimeType;
  originalSizeBytes: number;
  width?: number;
  height?: number;
  attachmentId?: string;
  filePath?: string;
}
```

#### 3.3.2 `src/chat/core/attachmentUtils.ts`

新增文件扩展名映射和创建函数：

```typescript
export const FILE_EXT_TO_MIME: Record<string, AllowedFileMimeType> = {
  pdf: "application/pdf",
  txt: "text/plain",
  md: "text/markdown",
  csv: "text/csv",
  html: "text/html",
  json: "application/json",
  py: "text/x-python",
  js: "text/javascript",
  ts: "text/typescript",
  tsx: "text/typescript",
  css: "text/css",
  yaml: "text/yaml",
  yml: "text/yaml",
  xml: "text/xml",
};

export function createFileAttachmentFromFile(file: File): Attachment | null {
  if (file.size > ATTACHMENT_MAX_FILE_SIZE_BYTES) return null;
  const ext = file.name.split(".").pop()?.toLowerCase() ?? "";
  const mimeType = FILE_EXT_TO_MIME[ext];
  if (!mimeType) return null;
  return {
    id: generateAttachmentId(),
    kind: "file",
    file,
    previewUrl: "",
    mimeType,
    originalSizeBytes: file.size,
  };
}

export function createFileAttachmentFromPath(filePath: string): Attachment | null {
  const ext = filePath.split(".").pop()?.toLowerCase() ?? "";
  const mimeType = FILE_EXT_TO_MIME[ext];
  if (!mimeType) return null;
  const name = filePath.split(/[\\/]/).pop() ?? filePath;
  return {
    id: generateAttachmentId(),
    kind: "file",
    file: new File([], name, { type: mimeType }),
    previewUrl: "",
    mimeType,
    originalSizeBytes: 0,
    filePath,
  };
}
```

#### 3.3.3 `src/chat/core/contentBlockUtils.ts`

`buildContentBlocksAsync` 新增 `kind: 'file'` 分支：

```typescript
for (const att of attachments) {
  if (att.kind === "image") {
    // ... 现有图片逻辑
  } else if (att.kind === "file") {
    const ref: FileAttachmentRef = {
      attachmentId: att.attachmentId ?? "",
      name: att.file.name,
      bytes: att.originalSizeBytes,
    };
    blocks.push({ type: "file", attachment: ref } satisfies FileBlock);
  }
}
```

#### 3.3.4 `src/hooks/useNativeFileAttach.ts`

新增 `handleFileSelect` 旁的 `handleDocSelect`：

```typescript
const handleDocSelect = useCallback(async () => {
  const selected = await open({
    multiple: true,
    filters: [{ name: "Documents", extensions: [...ALLOWED_FILE_EXTENSIONS] }],
  });
  if (!selected) return;
  const paths = Array.isArray(selected) ? selected : [selected];
  for (const p of paths) {
    addFileFromPath(p);
  }
}, [addFileFromPath]);
```

返回值扩展：`return { handleFileSelect, handleDocSelect };`

#### 3.3.5 `src/chat/message/MessageInput.tsx`

附件区域增加文件按钮图标（如 `PaperClipOutlined` 或 `FileTextOutlined`），点击调用 `handleDocSelect`。

#### 3.3.6 `src/chat/message/AttachmentPreview.tsx`

预览条中 `kind: 'file'` 附件渲染为文件名 + 大小标签（无缩略图）。

### 3.4 Rust 变更

#### 3.4.1 `src-tauri/crates/assistant-agent/src/multimodal.rs`

`admit_attachments` 新增 FileBlock 处理：

- FileBlock 的 `attachment_id` 为空时，从 `file_path` 或 `cached_data` 读取文件内容
- 调用 `store.save_image` 保存（复用内容寻址，PDF 等二进制同样适用）
- 填充 `attachment_id`

> **注意**：FileBlock 磁盘存储复用 `AttachmentStore::save_image`（内容寻址不限于图片）。后续如需区分可重命名为 `save_blob`，但当前无需。

#### 3.4.2 `src-tauri/crates/assistant-storage/src/attachment_store.rs`

`media_type_to_ext` 扩展文件 MIME：

```rust
fn media_type_to_ext(mt: &str) -> &'static str {
    match mt {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "application/pdf" => "pdf",
        "text/plain" => "txt",
        "text/markdown" => "md",
        "text/csv" => "csv",
        _ => "bin",
    }
}
```

---

## 4. `inputModalities` 替代 `supportVision`（#4）

### 4.1 现状

- Rust `ModelConfig.support_vision: bool`（[base.rs:184](../../src-tauri/crates/assistant-providers/src/base.rs#L184)）
- 前端 `ModelConfig.supportVision?: boolean`（[types.ts:7](../../src/settings/provider/types.ts#L7)）
- `resolve_support_vision` 从 `model_catalog` + API hint 解析（[vision.rs](../../src-tauri/crates/assistant-providers/src/vision.rs)）

### 4.2 设计

将 `support_vision: bool` 升级为 `input_modalities: Vec<String>`，向后兼容（`#[serde(default)]` + 迁移逻辑）。

**这是 #2（投影）的前置依赖**——投影的 `support_pdf` / `support_video` / `support_audio` 全部从 `inputModalities` 派生，不再单独硬编码布尔值。

### 4.3 Rust 变更

**`src-tauri/crates/assistant-providers/src/base.rs`**：

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModelConfig {
    pub id: String,
    pub name: String,
    pub owned_by: String,
    #[serde(default)]
    pub context_window: u32,
    #[serde(default)]
    pub support_vision: bool,           // 保留，从 input_modalities 推导
    #[serde(default = "default_input_modalities")]
    pub input_modalities: Vec<String>,  // 新增
}

fn default_input_modalities() -> Vec<String> {
    vec!["text".to_string()]
}
```

**`src-tauri/crates/assistant-providers/src/vision.rs`**：

`resolve_support_vision` 改为从 `input_modalities` 推导：

```rust
pub fn resolve_support_vision(model_id: &str, api_hint: Option<bool>, input_modalities: &[String]) -> bool {
    if input_modalities.iter().any(|m| m == "image") {
        return true;
    }
    let catalog_result = model_catalog::catalog_supports_vision(model_id);
    api_hint.or(catalog_result).unwrap_or(false)
}
```

**`src-tauri/crates/assistant-providers/src/model_catalog.rs`**：

`ModelFacts` 新增 `input_modalities: Vec<String>` 字段，从 `models-dev.json` 的 `modalities.input` 数组读取。

**Discovery 适配**（3× discovery.rs）：

构造 `ModelConfig` 时从 catalog 读取 `input_modalities`：

```rust
let facts = model_catalog::catalog_model_facts(&id);
let input_modalities = facts
    .map(|f| f.input_modalities.clone())
    .unwrap_or_else(|| vec!["text".to_string()]);
let support_vision = input_modalities.iter().any(|m| m == "image");

ModelConfig {
    id: id.clone(),
    name: ...,
    owned_by: ...,
    context_window: ...,
    support_vision,
    input_modalities,
}
```

**DTO 适配**：`src-tauri/src/protocol/dto/settings.rs` — `ModelConfigDto` 新增 `input_modalities: Vec<String>` + `From` impl 同步。

### 4.4 前端变更

**`src/settings/provider/types.ts`**：

```typescript
export interface ModelConfig {
  id: string;
  name: string;
  ownedBy: string;
  contextWindow?: number;
  supportVision?: boolean;
  inputModalities?: string[];   // 新增
}
```

`supportVision` 保留为便捷派生：`model.supportVision ?? model.inputModalities?.includes("image") ?? false`。

---

## 5. 不可信通道安全哨兵（#5）

### 5.1 设计

引入 `MultimodalConfig` 控制附件准入策略。可信通道（用户主动输入）宽松，不可信通道（API / MCP tool 返回）严格。

### 5.2 Rust 变更

**`src-tauri/crates/assistant-agent/src/multimodal.rs`** — 新增配置结构体：

```rust
#[derive(Debug, Clone)]
pub struct MultimodalConfig {
    pub max_images: usize,
    pub max_files: usize,
    pub max_image_bytes: u64,
    pub max_file_bytes: u64,
    pub allowed_image_media_types: Vec<String>,
    pub allowed_file_extensions: Vec<String>,
}

impl MultimodalConfig {
    pub fn trusted_channel_input() -> Self {
        Self {
            max_images: 4,
            max_files: 4,
            max_image_bytes: 8 * 1024 * 1024,
            max_file_bytes: 16 * 1024 * 1024,
            allowed_image_media_types: vec![
                "image/png".into(), "image/jpeg".into(),
                "image/gif".into(), "image/webp".into(),
            ],
            allowed_file_extensions: vec![
                "pdf".into(), "txt".into(), "md".into(), "csv".into(),
                "json".into(), "py".into(), "js".into(), "ts".into(),
            ],
        }
    }

    pub fn untrusted_channel_input() -> Self {
        Self {
            max_images: 0,   // 哨兵：不可信通道禁止附件
            max_files: 0,
            max_image_bytes: 0,
            max_file_bytes: 0,
            allowed_image_media_types: vec![],
            allowed_file_extensions: vec![],
        }
    }
}
```

**路径遍历防护**：

```rust
fn validate_path(path: &str) -> Result<(), String> {
    if path.contains("..") {
        return Err("path traversal detected".to_string());
    }
    if std::path::Path::new(path).is_absolute() {
        return Err("absolute path not allowed".to_string());
    }
    Ok(())
}
```

**`admit_attachments` 签名扩展**：

```rust
pub async fn admit_attachments(
    content: &str,
    store: &AttachmentStore,
    config: &MultimodalConfig,
) -> Result<String, String>
```

- 校验图片/文件数量不超过 `config.max_images` / `config.max_files`
- 校验 MIME 在 `allowed_image_media_types` 内
- 校验 `file_path` 通过 `validate_path`
- 校验大小不超过 `max_image_bytes` / `max_file_bytes`

**调用方适配**：[chat.rs](../../src-tauri/src/cmd/chat.rs) 中 `admit_attachments` 传入 `MultimodalConfig::trusted_channel_input()`。

---

## 6. Offload 降级（#6）

### 6.1 设计

请求预算超限时，将 ImageBlock 替换为 TextBlock 占位文本（含恢复路径），减少 token 消耗。

### 6.2 Rust 变更

**`src-tauri/crates/assistant-agent/src/request_projection.rs`**（新建）：

```rust
use assistant_providers::types::{ChatMessage, ContentBlock, MessageRole, parse_content};

const REQUEST_BUDGET_TOKENS: usize = 100_000;
const IMAGE_TOKEN_ESTIMATE: usize = 765;  // 768px × 768px / 0.75 ≈ 765 tokens（OpenAI 视觉 token 估算）

pub fn offload_if_over_budget(messages: &mut [ChatMessage], context_limit: u32) {
    let estimated_tokens = estimate_total_tokens(messages);
    let budget = context_limit as usize;

    if estimated_tokens <= budget {
        return;
    }

    // 从最后一条用户消息开始，逆序 offload ImageBlock
    for msg in messages.iter_mut().rev() {
        if msg.role != MessageRole::User {
            continue;
        }
        let Some(blocks) = parse_content(&msg.content) else {
            continue;
        };

        let projected: Vec<ContentBlock> = blocks
            .into_iter()
            .map(|block| match block {
                ContentBlock::Image { attachment, .. } => ContentBlock::Text {
                    text: offloaded_image_text(&attachment.attachment_id),
                },
                other => other,
            })
            .collect();

        if let Ok(json) = serde_json::to_string(&projected) {
            msg.content = json;
        }

        // 重新估算，若已低于预算则停止
        if estimate_total_tokens(messages) <= budget {
            return;
        }
    }
}

fn estimate_total_tokens(messages: &[ChatMessage]) -> usize {
    let mut total = 0;
    for msg in messages {
        if let Some(blocks) = parse_content(&msg.content) {
            for block in &blocks {
                match block {
                    ContentBlock::Text { text } => total += text.len() / 4,
                    ContentBlock::Image { .. } => total += IMAGE_TOKEN_ESTIMATE,
                    _ => total += 20,
                }
            }
        } else {
            total += msg.content.len() / 4;
        }
    }
    total
}

fn offloaded_image_text(attachment_id: &str) -> String {
    format!("[image offloaded: {}]", attachment_id)
}
```

**`src-tauri/crates/assistant-agent/src/lib.rs`** — 新增 `pub mod request_projection;`

**`src-tauri/src/cmd/chat.rs`** — Dispatch 管道末尾调用：

```rust
// 在 rehydrate/project/project_files_to_text 之后
let context_limit = models
    .iter()
    .find(|m| m.id == model_id)
    .and_then(|m| model_catalog::catalog_context_limit(&m.id))
    .unwrap_or(128_000);
request_projection::offload_if_over_budget(&mut chat_messages, context_limit);
```

---

## 7. 图片规范化（#7）

### 7.1 设计

入库前对图片做 resize / 重编码 / EXIF 旋转，确保：
- 大图缩放至合理尺寸（默认 2048px 长边）
- 统一编码为 JPEG（可配置 PNG）
- EXIF 旋转修正（手机拍照常见）

### 7.2 Rust 变更

**`src-tauri/crates/assistant-agent/src/normalization.rs`**（新建）：

```rust
use image::{DynamicImage, ImageFormat};

#[derive(Debug, Clone)]
pub struct NormalizeConfig {
    pub max_long_side: u32,       // 默认 2048
    pub target_format: ImageFormat, // 默认 JPEG
    pub jpeg_quality: u8,         // 默认 85
}

impl Default for NormalizeConfig {
    fn default() -> Self {
        Self {
            max_long_side: 2048,
            target_format: ImageFormat::Jpeg,
            jpeg_quality: 85,
        }
    }
}

pub fn normalize_image(data: &[u8], config: &NormalizeConfig) -> Result<Vec<u8>, String> {
    let mut img = image::load_from_memory(data)
        .map_err(|e| format!("Failed to decode image: {}", e))?;

    // EXIF 旋转修正（image crate 的 load_from_memory 已自动处理 Orientation）

    // Resize
    let (w, h) = (img.width(), img.height());
    let long_side = w.max(h);
    if long_side > config.max_long_side {
        let scale = config.max_long_side as f32 / long_side as f32;
        let new_w = (w as f32 * scale) as u32;
        let new_h = (h as f32 * scale) as u32;
        img = img.resize(new_w, new_h, image::imageops::FilterType::Lanczos3);
    }

    // 重编码
    let mut buf = std::io::Cursor::new(Vec::new());
    match config.target_format {
        ImageFormat::Jpeg => {
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, config.jpeg_quality);
            img.write_with_encoder(encoder);
        }
        ImageFormat::Png => {
            img.write_to(&mut buf, ImageFormat::Png).map_err(|e| e.to_string())?;
        }
        _ => {
            img.write_to(&mut buf, ImageFormat::Jpeg).map_err(|e| e.to_string())?;
        }
    }

    Ok(buf.into_inner())
}
```

**`src-tauri/crates/assistant-agent/Cargo.toml`** — 新增 `image = "0.25"` 依赖。

**`src-tauri/crates/assistant-agent/src/multimodal.rs`** — `admit_attachments` 中 `save_image` 前调用规范化：

```rust
let config = NormalizeConfig::default();
let normalized = normalization::normalize_image(&data, &config)?;
let ref_info = store.save_image(&normalized, &mt).await.map_err(|e| e.to_string())?;
```

> **注意**：规范化后 MIME 可能变化（如 PNG → JPEG），需同步更新 `mt`。

---

## 8. 降级文本函数抽取（#8）

### 8.1 现状

- `project_images_for_text_model` 内联 `t!("chat.image_projected_placeholder")`
- `offloaded_image_text` 内联 `format!("[image offloaded: {}]", attachment_id)`
- `file_handle_text` 内联 `format!("[file: {} ({})]", name, size_str)`

### 8.2 变更

抽取为独立函数，集中管理降级文本策略：

**`src-tauri/crates/assistant-agent/src/multimodal.rs`**：

```rust
pub fn text_only_image_text() -> String {
    assistant_i18n::t("chat.image_projected_placeholder")
}

pub fn offloaded_image_text(attachment_id: &str) -> String {
    assistant_i18n::t_with_args("chat.image_offloaded_placeholder", &{
        let mut args = HashMap::new();
        args.insert("id", attachment_id);
        args
    })
}

pub fn file_handle_text(att: &FileAttachmentRef) -> String {
    let size_str = format_size(att.bytes);
    format!("[file: {} ({})]", att.name, size_str)
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}
```

**i18n key 补充**：

- `chat.image_offloaded_placeholder` — `"图片已卸载 (ID: {{id}})"` / `"Image offloaded (ID: {{id}})"`

---

## 9. `created_at` 从磁盘 mtime 读取（#9）✅

### 9.1 现状

- ~~[storage.rs:58,75](../../src-tauri/src/cmd/storage.rs#L58) — `created_at: 0` 硬编码~~ → 已修复，填入 `entry.created_at as i64`
- ~~[attachment_store.rs:36](../../src-tauri/crates/assistant-storage/src/attachment_store.rs#L36) — `AttachmentEntry` 仅有 `id` + `bytes`，无 `created_at`~~ → 已添加 `created_at: u64`
- ~~`list_all()` 中 `metadata()` 已获取但仅用 `len()`，未读 `modified()` / `created()`~~ → 已读取 `modified()` 时间戳

### 9.2 变更

**`src-tauri/crates/assistant-storage/src/attachment_store.rs`**：

```rust
pub struct AttachmentEntry {
    pub id: String,
    pub bytes: u64,
    pub created_at: u64,   // 新增：Unix timestamp（秒）
}
```

`list_all()` 中读取 `modified()` 时间戳：

```rust
let metadata = entry.metadata().await?;
let created_at = metadata
    .modified()
    .ok()
    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
    .map(|d| d.as_secs())
    .unwrap_or(0);
entries.push(AttachmentEntry {
    id,
    bytes: metadata.len(),
    created_at,
});
```

**`src-tauri/src/cmd/storage.rs`** — `get_storage_breakdown` 中填入真实值：

```rust
AttachmentSummary {
    attachment_id: entry.id.clone(),
    media_type: media_type_from_id(&entry.id),
    bytes: entry.bytes,
    created_at: entry.created_at,   // 从 0 改为真实值
}
```

**`src-tauri/src/protocol/dto/storage.rs`** — 确认 `AttachmentSummary.created_at` 字段类型为 `i64`（当前已为 `i64`，`storage.rs` 中用 `entry.created_at as i64` 转换）。

---

## 9b. 存储总量告警提示 ✅

### 9b.1 需求

当附件总占用超过 500MB 时，在存储管理页面 summary-row 下方显示一行轻量警告，提示用户建议清理。

### 9b.2 设计决策

- **不使用 `Alert` 组件**：`Alert` 视觉冲击过强，适合阻断性警告（如灾难恢复），磁盘占用大只是建议性提示
- **使用 `WarningOutlined` + `Typography.Text type="warning"`**：与项目风格一致（VaultSyncSettings danger zone 同此模式），行内轻量，不破坏页面布局

### 9b.3 变更

**`src/settings/pages/StorageSettings.tsx`**：

- import 增加 `WarningOutlined`
- summary-row 下方条件渲染告警行：`breakdown.totalBytes > 500 * 1024 * 1024`

**`src/i18n/locales/zh-CN.json`** — 新增 key：

- `settings.storage.size_warning` — `"附件占用较大，建议清理以释放磁盘空间"`

**`src/i18n/locales/en-US.json`** — 新增 key：

- `settings.storage.size_warning` — `"Attachments are using significant disk space. Consider cleaning up to free space."`

---

## 10. 涉及文件清单汇总

### 10.1 新增文件

| 文件路径 | 用途 |
|----------|------|
| `src-tauri/crates/assistant-agent/src/request_projection.rs` | Offload 降级逻辑 |
| `src-tauri/crates/assistant-agent/src/normalization.rs` | 图片规范化逻辑 |

### 10.2 变更文件

| 文件路径 | 变更内容 | 对应 # |
|----------|----------|--------|
| `src/chat/message/ImageThumbnail.tsx` | error 分支增加重试按钮 | 1 |
| `src/chat/message/ImageThumbnail.module.css` | `.error` / `.retryBtn` 样式 | 1 |
| `src/chat/core/attachmentTypes.ts` | `ALLOWED_FILE_MIME_TYPES` / `ALLOWED_FILE_EXTENSIONS` / `AllowedFileMimeType` | 3 |
| `src/chat/core/attachmentUtils.ts` | `FILE_EXT_TO_MIME` / `createFileAttachmentFromFile` / `createFileAttachmentFromPath` | 3 |
| `src/chat/core/contentBlockUtils.ts` | `buildContentBlocksAsync` 增加 `kind: 'file'` 分支 | 3 |
| `src/chat/core/contentBlockTypes.ts` | `FileBlock` 新增 `cached_data` 字段 | 2 |
| `src/hooks/useNativeFileAttach.ts` | `handleDocSelect` + 文件选择 | 3 |
| `src/chat/message/MessageInput.tsx` | 文件按钮 | 3 |
| `src/chat/message/AttachmentPreview.tsx` | 文件附件预览 | 3 |
| `src-tauri/crates/assistant-agent/src/multimodal.rs` | `project_files` / `rehydrate_files` / `file_handle_text` / `admit_attachments` 扩展 / 降级文本抽取 / 规范化调用 | 2,3,5,7,8 |
| `src-tauri/crates/assistant-storage/src/attachment_store.rs` | `AttachmentEntry.created_at` / `media_type_to_ext` 扩展 / `list_all` mtime | 3,9 |
| `src-tauri/src/cmd/chat.rs` | Dispatch 管道增加 `rehydrate_files` / `project_files` / offload / 规范化 | 2,6,7 |
| `src-tauri/crates/assistant-providers/src/base.rs` | `ModelConfig.input_modalities` | 4 |
| `src-tauri/crates/assistant-providers/src/vision.rs` | `resolve_support_vision` 签名扩展 | 4 |
| `src-tauri/crates/assistant-providers/src/model_catalog.rs` | `ModelFacts.input_modalities` | 4 |
| `src-tauri/crates/assistant-providers/src/openai/discovery.rs` | `input_modalities` 构造 | 4 |
| `src-tauri/crates/assistant-providers/src/anthropic/discovery.rs` | 同上 | 4 |
| `src-tauri/crates/assistant-providers/src/gemini/discovery.rs` | 同上 | 4 |
| `src-tauri/src/protocol/dto/settings.rs` | `ModelConfigDto.input_modalities` | 4 |
| `src/settings/provider/types.ts` | `ModelConfig.inputModalities` | 4 |
| `src-tauri/crates/assistant-providers/src/openai/message_converter.rs` | `ContentBlock::File` → TextBlock 投影（GPT 不支持 PDF） | 2 |
| `src-tauri/crates/assistant-providers/src/anthropic/message_converter.rs` | `ContentBlock::File` → PDF document / TextBlock 投影 | 2 |
| `src-tauri/crates/assistant-providers/src/gemini/message_converter.rs` | `ContentBlock::File` → PDF inlineData / TextBlock 投影 | 2 |
| `src-tauri/crates/assistant-providers/src/types/message.rs` | `ContentBlock::File` 新增 `cached_data` 字段 | 2 |
| `src-tauri/src/cmd/storage.rs` | `created_at` 填入真实值 | 9 |
| `src-tauri/src/protocol/dto/storage.rs` | 确认 `created_at` 类型 | 9 |
| `src-tauri/crates/assistant-agent/src/lib.rs` | `pub mod request_projection;` / `pub mod normalization;` | 6,7 |
| `src-tauri/crates/assistant-agent/Cargo.toml` | `image = "0.25"` | 7 |
| `src-tauri/crates/assistant-i18n/locales/zh-CN.json` | `chat.image_offloaded_placeholder` | 8 |
| `src-tauri/crates/assistant-i18n/locales/en-US.json` | 同上 | 8 |
| `src/settings/pages/StorageSettings.tsx` | 总量告警提示（`WarningOutlined` + 条件渲染） | 9b |
| `src/i18n/locales/zh-CN.json` | `settings.storage.size_warning` | 9b |
| `src/i18n/locales/en-US.json` | 同上 | 9b |

---

## 11. 验收清单

### Phase 2 验收

| ID | 验收条件 | # |
|----|---------|---|
| V2-1 | ImageThumbnail 加载失败后显示重试按钮，点击后重新加载 | 1 |
| V2-2 | 前端文件选择器可选 PDF/文本文件，预览条显示文件名+大小 | 3 |
| V2-3 | 非 PDF 文件或不支持 PDF 的模型：FileBlock 投影为 TextBlock（`[file: name (size)]`） | 2 |
| V2-4 | 支持 PDF 的模型（Claude/Gemini）+ PDF 文件：Provider 收到原生 PDF 格式（Anthropic `document` / Gemini `inlineData`） | 2 |
| V2-5 | 不支持 PDF 的模型（GPT）+ PDF 文件：Provider 收到 TextBlock 投影 | 2 |
| V2-6 | `project_files` 同时操作 `content_blocks` 和 `content`，不因 `rehydrate_images` 设置 `content_blocks` 而失效 | 2 |
| V2-7 | `ContentBlock::File` 新增 `cached_data` 字段，rehydrate 时填充 base64 | 2 |
| V2-8 | `ModelConfig.input_modalities` 正确从 models-dev.json 读取 | 4 |
| V2-9 | `supportVision` 从 `inputModalities.includes("image")` 推导 | 4 |
| V2-10 | `MultimodalConfig::untrusted_channel_input()` max_files=0 拒绝附件 | 5 |
| V2-11 | 路径含 `..` 或绝对路径时 `admit_attachments` 返回错误 | 5 |

### Phase 3 验收

| ID | 验收条件 | # |
|----|---------|---|
| V3-1 | 请求 token 超限时 ImageBlock 被替换为 offload 占位文本 | 6 |
| V3-2 | 2048px 以上图片入库前被 resize 至 2048px 长边 | 7 |
| V3-3 | JPEG 质量默认 85，可配置 | 7 |
| V3-4 | `text_only_image_text()` / `offloaded_image_text()` / `file_handle_text()` 为独立 pub 函数 | 8 |

### I2f 补丁验收

| ID | 验收条件 | # | 状态 |
|----|---------|---|------|
| Vf-1 | `StorageSettings` 页面中附件 `created_at` 显示为真实时间（非 0） | 9 | ✅ |
| Vf-2 | 附件总占用 > 500MB 时，summary-row 下方显示 `WarningOutlined` + 警告文本 | 9b | ✅ |
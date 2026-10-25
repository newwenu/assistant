# I2e Phase 1 — 任务清单

> 规格：[I2e-phase1-spec.md](./I2e-phase1-spec.md)
> 主规格：[I2e-multimodal-attachments-spec.md](./I2e-multimodal-attachments-spec.md)
> 前置：I2-History ✅

---

## 阶段 A — 类型基础 + 存储基础设施 + 模型配置

### A1. ContentBlock 类型定义

`src/chat/core/contentBlockTypes.ts`（新建）

- [x] A1.1 `TextBlock` / `ImageBlock` / `FileBlock` 接口
- [x] A1.2 `ImageAttachmentRef`（`width?` / `height?` 可选）/ `FileAttachmentRef` 接口
- [x] A1.3 `ImageMediaType` 联合类型
- [x] A1.4 `ContentBlock` 联合类型（`TextBlock | ImageBlock | FileBlock`）

### A2. 附件类型与约束常量

`src/chat/core/attachmentTypes.ts`（新建）

- [x] A2.1 `ALLOWED_IMAGE_MIME_TYPES` + `AllowedImageMimeType`
- [x] A2.2 `AttachmentKind` / `Attachment` 接口
- [x] A2.3 `ATTACHMENT_MAX_IMAGES` / `ATTACHMENT_MAX_FILES` / `ATTACHMENT_MAX_IMAGE_SIZE_BYTES` / `ATTACHMENT_MAX_FILE_SIZE_BYTES`

### A3. ContentBlock 工具函数

`src/chat/core/contentBlockUtils.ts`（新建）

- [x] A3.1 `buildContentBlocks(text, attachments) → ContentBlock[]`
- [x] A3.2 `parseContent(content) → ContentBlock[] | null` — 全元素 `type` 校验（非仅首元素）
- [x] A3.3 `serializeContent(blocks) → string`
- [x] A3.4 `validateImageFile(file) → { ok, error? }`
- [x] A3.5 单元测试：纯文本→null / 合法JSON→数组 / 非ContentBlock JSON→null

依赖：A1, A2

### A4. ModelConfig + asset scope + discovery + DTO

`src/settings/provider/types.ts` + `base.rs` + `dto/settings.rs` + `tauri.conf.json` + 3× `discovery.rs`

- [x] A4.1 前端 `ModelConfig.supportVision?: boolean`
- [x] A4.2 Rust `ModelConfig.support_vision: bool` + `#[serde(default)]`
- [x] A4.3 `ModelConfigDto` 新增 `support_vision: bool` + `#[serde(default)]`，`From<ModelConfig>` impl 同步
- [x] A4.4 `tauri.conf.json` asset scope 新增 `"$APPDATA/attachments/**"`
- [x] A4.5 `anthropic/discovery.rs` — `ModelConfig` 构造加 `support_vision: true`（Claude 系列均支持 Vision）
- [x] A4.6 `openai/discovery.rs` — `ModelConfig` 构造加 `support_vision`（从 model id 启发推断：含 `vision`/`4o`/`gpt-4-turbo` → true，其余 false）
- [x] A4.7 `gemini/discovery.rs` — `ModelConfig` 构造加 `support_vision: true`（Gemini 系列均支持 Vision）

### A5. Rust ContentBlock → 判别枚举

`src-tauri/crates/assistant-providers/src/types/message.rs`

- [x] A5.1 重构为 `#[serde(tag = "type", rename_all = "snake_case")]` enum：`Text` / `Image` / `File` / `Thinking` / `ToolUse` / `ToolResult`
- [x] A5.2 `cached_data` 用 `#[serde(skip_serializing)]`（允许反序列化供 Ingress，禁止序列化防 DB 污染）
- [x] A5.3 `ChatMessage` 新增 `#[serde(skip)] content_blocks: Option<Vec<ContentBlock>>` 扩展字段（rehydrate 填充，Provider 转换器优先读取）
- [x] A5.4 `attachment_id` 为 `String`（非 Option），Ingress 前态填空字符串
- [x] A5.5 `width` / `height` → `Option<u32>`
- [x] A5.6 移除 `image_url` 字段
- [x] A5.7 `ContentBlockType` 补充 `File`，保留迁移期兼容

### A5b. types/mod.rs re-export 更新

`src-tauri/crates/assistant-providers/src/types/mod.rs`

- [x] A5b.1 `ContentBlock` 从 struct 变为 enum 后，re-export 需同步（移除或保留 `ContentBlockType`）

### A6. AttachmentStore 磁盘存储

`src-tauri/crates/assistant-storage/src/attachment_store.rs`（新建）+ `lib.rs` + `Cargo.toml`

- [x] A6.1 `Cargo.toml` 新增 `sha2` / `tokio`
- [x] A6.2 `lib.rs` 新增 `pub mod attachment_store;`
- [x] A6.3 `AttachmentStoreConfig { dir }`（不可变）+ `AttachmentStore { config: Arc<Config> }`
- [x] A6.4 `new(dir)` 构造
- [x] A6.5 `save_image` → 完整 sha256 64 位 hex + 扩展名（`<hex>.png`）作文件名 → 原子写入 → `ImageAttachmentRef`
- [x] A6.6 `read(id)` → 直接 `dir.join(id)` 拼接路径
- [x] A6.7 `list_all()` → 列出所有附件（Phase 2 手动清理 UI 用）
- [x] A6.8 `delete(id)` → 删除指定附件（Phase 2 手动清理 UI 用）
- [x] A6.9 `clean_tmp()` → 清理 `*.tmp` 残留

### A7. setup.rs 初始化

`src-tauri/src/setup.rs`

- [x] A7.1 DB 初始化后创建 `attachments` 目录
- [x] A7.2 `app.manage(store)`（直接 manage，无 Mutex wrapper）
- [x] A7.3 spawn `store.clean_tmp()`

依赖：A6

### A8. i18n key 补充

Rust 端 + 前端 i18n 文件

- [x] A8.1 Rust `zh-CN.json` — 新增 `chat.too_many_images` / `chat.image_too_large` / `chat.image_projected_placeholder`
- [x] A8.2 Rust `en-US.json` — 同上，英文文案
- [x] A8.3 前端 `zh-CN.json` — 新增"当前模型不支持图片"等附件 UI 文案
- [x] A8.4 前端 `en-US.json` — 同上，英文文案

依赖：无（可最先做）

### A9. MessageRow helper 方法（抽象层）

`src-tauri/crates/assistant-storage/src/conversations.rs`（或新建 `content_codec.rs`）

- [x] A9.1 `is_content_empty(&self) → bool` — 语义空判断（纯文本空 or ContentBlock[] 无有效 block）
- [x] A9.2 `content_text_preview(&self, max_len) → String` — 提取纯文本预览（用于 sibling preview）
- [x] A9.3 `content_plain_text(&self) → String` — 提取全文纯文本（用于搜索、导出 markdown）

依赖：A5（需要 ContentBlock enum 定义）

---

## 阶段 B — 后端管道（Ingress + Dispatch + Provider 转换）

### B1. multimodal.rs Ingress + Dispatch

`src-tauri/crates/assistant-agent/src/multimodal.rs`（新建）+ `lib.rs` + `Cargo.toml`

- [x] B1.1 `Cargo.toml` 新增 `assistant-storage` / `base64`
- [x] B1.2 `lib.rs` 新增 `pub mod multimodal;`
- [x] B1.3 `admit_attachments` — 反序列化 `Vec<ContentBlock>`（非 Value）→ 遍历 Image 变体 → 校验 → save_image → 替换 ref
- [x] B1.4 `rehydrate_images` — 磁盘读回 → 注入 `cached_data` → 填充 `msg.content_blocks`（Provider 转换器优先读此字段）
- [x] B1.5 `project_images_for_text_model` — `Image → Text { t!("chat.image_projected_placeholder") }`（i18n）
- [x] B1.6 错误 i18n：`t!("chat.too_many_images")` / `t!("chat.image_too_large")`
- [x] B1.7 附件不存在 → 占位文本，不报错

依赖：A5, A6

### B2. send_chat_message 集成 Ingress

`src-tauri/src/cmd/chat.rs`

- [x] B2.1 插入 DB 前调用 `admit_attachments(&content)`
- [x] B2.2 失败 → 返回错误

依赖：B1

### B3. Dispatch 集成 chat_runtime

`chat_runtime.rs`（或 chat_tasks.rs）

- [x] B3.1 dispatch 前按 `support_vision` 调用 rehydrate 或 project
- [x] B3.2 确认 `is_vision_model` 获取路径

依赖：B1, A4

### ~~~B4. Tauri Command: read_attachment~~~

~~~`src-tauri/src/cmd/chat.rs`~~~

~~- [ ] B4.1 `async fn read_attachment` — 仅内部/高级用途（前端渲染用 asset 协议）~~
~~- [ ] B4.2 注册 command~~

~~依赖：A6~~

### B5. OpenAI 转换器

`openai/message_converter.rs`

- [x] B5.1 导入 `parse_content`
- [x] B5.2 拆分 `MessageRole::User` 独立分支
- [x] B5.3 `msg.content_blocks` 优先 → enum match 遍历：`Text → {"type":"text"}`, `Image { cached_data: Some } → {"type":"image_url"}`；fallback `parse_content(&msg.content)`
- [x] B5.4 `parse_content` 失败 → 降级纯文本
- [x] B5.5 其他分支不变

依赖：A5

### B6. Anthropic 转换器

`anthropic/message_converter.rs`

- [x] B6.1 `convert_user_message` 重写：`msg.content_blocks` 优先 → enum match 遍历；fallback `parse_content`
- [x] B6.2 `Text → {"type":"text"}`, `Image { cached_data: Some } → {"type":"image","source":{"type":"base64"}}`
- [x] B6.3 降级：`json!({"role":"user","content":[{"type":"text","text":msg.content}]})`
- [x] B6.4 **删除** `tool_call_id` hack + `parse_image_source`
- [x] B6.5 其他函数不变

依赖：A5

### B7. Gemini 转换器

`gemini/message_converter.rs`

- [x] B7.1 `convert_user_parts` 重写：`parse_content` → enum match 遍历
- [x] B7.2 `Text → {"text":…}`, `Image { cached_data: Some } → {"inlineData":{…}}`
- [x] B7.3 降级：保留 `try_parse_inline_image` 兼容旧数据
- [x] B7.4 其他函数不变

依赖：A5

### B8. content 语义适配（与 B2/B3 同因果链，必须同步）

> **为什么必须与 B2/B3 同步**：B2 让 JSON content 写入 SQLite，此时以下 4 处现有功能**立即退化**——
> - 分支选择器：`SUBSTR(content,1,100)` 截断 JSON → 显示 `{"blocks":[{"type":"text"...`
> - 会话搜索：`content LIKE ?1` 匹配 JSON 结构字符 → 误命中
> - 导出 Markdown：`format!("{}", msg.content)` → 输出原始 JSON
> - 空值检查：`content.trim().is_empty()` 对纯图片消息语义错误
>
> 分开做意味着 B2→C7 之间有功能退化窗口。

`src-tauri/crates/assistant-storage/src/conversations.rs` + `src-tauri/src/cmd/chat.rs`

- [x] B8.1 `chat.rs` L17 — `content.trim().is_empty()` → `is_content_empty(content)` 语义判断
- [x] B8.2 `conversations.rs` L776/L795 — `SUBSTR(content, 1, 100)` → Rust 端调用 `content_text_preview(content, 100)`
- [x] B8.3 `conversations.rs` L465 — `m.content LIKE %query%` → Rust 端 `content_plain_text().contains(query)` 过滤
- [x] B8.4 `conversations.rs` L481-484 — `format!("{}\n\n", msg.content)` → `format!("{}\n\n", msg.content_plain_text())`
- [x] B8.5 确认 `SiblingInfoDto.contentPreview` 返回可读纯文本预览（非 JSON 截断）

依赖：A9, B2

---

## 阶段 C — 前端输入 + 渲染 + 端到端集成

### C1. 附件预览条

`src/chat/message/AttachmentPreview.tsx`（新建）+ CSS Module

- [x] C1.1 渲染 `Attachment[]`：缩略图/文件名+大小+删除按钮
- [x] C1.2 数量/大小超限提示

依赖：A2

### C2. MessageInput 集成附件

`src/chat/message/MessageInput.tsx`

- [x] C2.1 `attachments: Attachment[]` 状态
- [x] C2.2 附件按钮 → 文件选择器 → 校验 → 加入
- [x] C2.3 粘贴图片：`onPaste` → 校验 → 加入
- [x] C2.4 拖拽图片：复用 `useFileDrop` hook（已有完整拖拽实现），仅加图片 MIME 校验
- [x] C2.5 集成 `<AttachmentPreview>`
- [x] C2.6 发送：`onSend(serializeContent(buildContentBlocks(text, attachments)))`，清空 attachments
- [x] C2.7 非 Vision 模型：隐藏附件按钮，粘贴/拖拽提示不支持

依赖：A2, A3, C1

### C3. ImageThumbnail — asset 协议 + 懒渲染

`src/chat/message/ImageThumbnail.tsx`（新建）+ CSS Module

- [x] C3.1 `convertFileSrc(attachments_dir/attachmentId)` → `asset:` 协议 URL（浏览器缓存，零 IPC）
- [x] C3.2 `<img src={assetUrl} loading="lazy">` — 原生懒加载
- [x] C3.3 骨架屏占位（`width/height` 算 `aspect-ratio`，防 CLS）
- [x] C3.4 `onLoad` / `onError` 状态切换
- [x] C3.5 点击放大预览
- [x] C3.6 offloaded → 占位文本

依赖：A4（asset scope）

### C4. FileCard（Phase 1 骨架）

`src/chat/message/FileCard.tsx`（新建）+ CSS Module

- [x] C4.1 文件名 + 大小 + 图标（Phase 2 启用下载）

依赖：A1

### C5. MessageContent 结构化渲染

`src/chat/message/MessageContent.tsx`

- [x] C5.1 `parseContent(content)` → 遍历 ContentBlock[] 按 type 渲染
- [x] C5.2 `TextBlock` → user `<p>` / assistant `<ReactMarkdown>`
- [x] C5.3 `ImageBlock` → `<ImageThumbnail>`
- [x] C5.4 `FileBlock` → `<FileCard>`
- [x] C5.5 unknown type → `null`
- [x] C5.6 `parseContent` 返回 null → 旧数据降级

依赖：A3, C3, C4

### C5b. contentBlockUtils 补充函数（抽象层）

`src/chat/core/contentBlockUtils.ts`

- [x] C5b.1 `extractPlainText(content) → string` — 从 content 中提取纯文本（用于标题、预览等非渲染场景）
- [x] C5b.2 `rebuildContent(originalContent, newText) → string` — 重组 content：替换 TextBlock 文本，保留 ImageBlock（用于 editMessage）

依赖：A3

### C6. chatStore / chatOrchestrator 集成

- [x] C6.1 `sendMessage` 签名不变，content 已为 JSON
- [x] C6.2 确认全链路 content 为 JSON string
- [x] C6.3 确认乐观更新中 `Message.content` 正确存储 JSON

依赖：A3

---

## 阶段 D — 前端分支适配 + 样式补充

> **注意**：D2（分支选择器 SUBSTR 适配）和 D3.1-D3.3（搜索/导出/空值检查适配）已提前至 **B8**，
> 因为它们与 B2/B3 是同一因果链——B2 让 JSON 进入 DB，B8 修复 JSON 进入 DB 后的副作用。
> 此处仅保留前端侧适配。

### D1. editMessage 分支适配

`src/stores/chatMessageSlice.ts`

- [x] D1.1 `editMessage(id, newText)` — 从原消息 content 提取 ImageBlock，调用 `rebuildContent` 重组 ContentBlock[]，传给 `resendFrom`

依赖：C5b

### D2. chatOrchestrator 标题适配

`src/services/chatOrchestrator.ts`

- [x] D2.1 `fallbackTitleFromUserMessage(content)` → `fallbackTitleFromUserMessage(extractPlainText(content))`

依赖：C5b

### D3. MessageContent.module.css 样式补充

`src/chat/message/MessageContent.module.css`

- [x] D3.1 新增 `.imageBlock` / `.fileBlock` 等样式规则（block 间距、图片容器）

依赖：C5

---

## 阶段依赖关系

```
A1 ─┬─→ A3 ─┬─→ C2 ─→ C6
A2 ─┤       ├─→ C5
    │       ├─→ C5b ─→ D1, D2
    │       └─→ C6
A4 ─┼─→ B3 ──→ C3
A5 ─┼─→ B1 ─→ B2 ─→ B8
    │      └─→ B3
    ├─→ A5b
    └─→ A9 ─→ B8
A6 ─┼─→ B1
    │  └─→ B4
A7 ─┘
A8 ─→ B1（i18n key 供 t! 宏使用）

B5, B6, B7 独立于 B1-B4（仅依赖 A5）
B8 依赖 A9 + B2（与 B2 同因果链，必须同步）
C1 → C2
C3 → C5 → D3
C4 → C5

D1, D2 依赖 C5b
D3 依赖 C5
```

**推荐顺序**：A8（i18n）→ A 全部（1-2d）→ B 全部含 B8（2-3d）→ C 全部（1-2d）→ D 全部（0.5-1d）= **5-8 天**

> **关键约束**：B8 必须与 B2 在同一次提交中完成，否则 JSON content 进入 DB 后分支选择器/搜索/导出立即退化。

---

## 验收检查清单

- [ ] V1：粘贴图片 → 预览 → 发送 → 显示
- [ ] V2：拖拽图片 → 预览 → 发送 → 显示
- [ ] V3：附件按钮 → 选择 → 预览 → 发送 → 显示
- [ ] V4：≤4 张图片全部显示
- [ ] V5：>4 张 → 提示超限
- [ ] V6：>8MB → 提示超限
- [x] V7：非 Vision → 附件按钮隐藏
- [ ] V8：Vision → 图片正确发送到 Provider
- [x] V9：SQLite 中 ImageBlock 仅含 attachmentId（无 base64）
- [x] V10：磁盘文件名为 `<sha256_64hex>.<ext>`
- [x] V11：旧纯文本消息 → 正常渲染
- [ ] V12：历史图片 → asset 协议 + 浏览器缓存正确显示
- [x] V13：`parseContent` 非 JSON → null
- [x] V14：`parseContent` 合法 JSON → 数组
- [x] V15：`tool_call_id` hack + `parse_image_source` 已删除
- [x] V16：非 Vision → Provider 收到 i18n 占位文本
- [x] V17：`<img loading="lazy">` 懒加载生效
- [ ] V18：骨架屏占位防 CLS + 加载失败显示错误+重试
- [x] V19：编辑含图片消息 → 图片保留、仅文本被修改 → 分支正确创建
- [x] V20：分支选择器中含图片消息 → content_preview 显示可读纯文本（非 JSON 截断）【B8 验收】
- [x] V21：`content.trim().is_empty()` → `is_content_empty()` 语义判断正确（纯图片消息不误判为空）【B8 验收】
- [x] V22：搜索含图片消息的对话 → 搜索匹配纯文本内容（不匹配 JSON 结构字符）【B8 验收】
- [x] V23：导出含图片消息的对话为 Markdown → 输出纯文本（非 JSON）【B8 验收】
- [x] V24：3× discovery.rs 编译通过（`ModelConfig` 含 `support_vision` 字段）
- [x] V25：`ModelConfigDto.support_vision` 前端可读取
- [x] V26：i18n key 全部存在（Rust `t!` 和前端 `t()` 不回退到裸 key）
# I2g — 任务清单

> 规格：[I2g-remaining-features-spec.md](./I2g-remaining-features-spec.md)
> 前置：I2e Phase 1 ✅ + I2f ✅
> 说明：代码复核后对规格做了两处修正——#4 数据层已落地，仅剩运行时接线；#5 的绝对路径校验仅适用于不可信通道。

---

## P0 — ImageThumbnail 失败重试（#1）

`src/chat/message/ImageThumbnail.tsx` + `ImageThumbnail.module.css`

- [x] P0.1 `error` 分支与 `offloaded` 分离，独立 `.error` 容器
- [x] P0.2 增加 `ReloadOutlined` 重试按钮，点击重置 `error/loaded` 触发重新加载
- [x] P0.3 新增 `.error` / `.errorIcon` / `.errorText` / `.retryBtn` 样式

## P1 — 文件附件入口（#3 前端 + #2 准入）

### P1.1 前端类型与工具

`src/chat/core/attachmentTypes.ts`、`attachmentUtils.ts`、`contentBlockUtils.ts`

- [x] P1.1.1 `ALLOWED_FILE_MIME_TYPES` / `AllowedFileMimeType` / `ALLOWED_FILE_EXTENSIONS`
- [x] P1.1.2 `Attachment` 拆分为 `ImageAttachment` / `FileAttachment` 联合类型
- [x] P1.1.3 `FILE_EXT_TO_MIME` + `createFileAttachmentFromFile` / `createFileAttachmentFromPath`
- [x] P1.1.4 `buildContentBlocks` / `buildContentBlocksAsync` 增加 `kind: "file"` 分支
- [x] P1.1.5 `notifyAttachError` 增加文件数量/大小/类型文案

### P1.2 前端入口

`src/hooks/useNativeFileAttach.ts`、`src/chat/ChatView.tsx`、`AttachmentPreview.tsx`

- [x] P1.2.1 单入口附件对话框：图片与文档合并为 `Images & Documents` 单一筛选器，无需切换；非视觉模型只列文档扩展名
- [x] P1.2.2 拖拽按扩展名分流到图片/文件
- [x] P1.2.3 复用原回形针按钮，始终可见；图片仍校验模型视觉能力
- [x] P1.2.4 文件附件预览显示文件名 + 大小
- [x] P1.2.5 图片与文件分别按 `ATTACHMENT_MAX_IMAGES` / `ATTACHMENT_MAX_FILES` 计数

### P1.3 后端准入

`assistant-agent/src/multimodal.rs`、`assistant-storage/src/attachment_store.rs`

- [x] P1.3.1 `admit_attachments` 处理 `ContentBlock::File`（file_path / cached_data）
- [x] P1.3.2 文件数量上限 4、单文件上限 16MB、扩展名白名单
- [x] P1.3.3 `media_type_to_ext` 扩展 pdf / 文本类 MIME
- [x] P1.3.4 文件复用内容寻址存储（`save_image` 按 MIME 决定落盘扩展名）

## P2 — 能力感知投影（#2 + #8 + #4 运行时）

### P2.1 模型能力

`assistant-providers/src/types/config.rs`、`src/chat/ChatView.tsx`

- [x] P2.1.1 新增 `resolve_input_modalities_from_stored` + `resolve_support_pdf_from_stored`
- [x] P2.1.2 `build_chat_request` 按 `input_modalities` 派生 `support_vision` / `support_pdf`
- [x] P2.1.3 前端优先级改为 `inputModalities` 优先、`supportVision` 兜底

### P2.2 FileBlock 投影

`assistant-providers/src/types/message.rs`、`contentBlockTypes.ts`、`multimodal.rs`

- [x] P2.2.1 `ContentBlock::File` 增加 `cached_data`（`skip_serializing`）
- [x] P2.2.2 前端 `FileBlock` 增加可选 `cached_data`
- [x] P2.2.3 `rehydrate_files`（仅 `support_pdf` 且 PDF）
- [x] P2.2.4 `project_files`（非 PDF 或模型不支持 PDF → `file_handle_text`）
- [x] P2.2.5 投影优先读取 `content_blocks` 并同步 `content`
- [x] P2.2.6 降级文本抽取：`text_only_image_text` / `offloaded_image_text` / `file_handle_text`
- [x] P2.2.7 i18n 新增 `chat.image_offloaded_placeholder`

### P2.3 Provider 转换器

`openai` / `anthropic` / `gemini` 的 `message_converter.rs`

- [x] P2.3.1 Anthropic：PDF → `document` base64，其余 → 文本
- [x] P2.3.2 Gemini：PDF → `inlineData`，其余 → 文本
- [x] P2.3.3 OpenAI：FileBlock 始终 → 文本
- [x] P2.3.4 无 `cached_data` 时防御性降级为文本

## P3 — 不可信通道哨兵（#5）

`assistant-agent/src/multimodal.rs`、`src-tauri/src/cmd/chat/mod.rs`

- [x] P3.1 `MultimodalConfig`（trusted / untrusted）
- [x] P3.2 `admit_attachments` 增加 config 参数并校验数量/MIME/大小
- [x] P3.3 路径校验：仅不可信通道禁止绝对路径与 `..`
- [x] P3.4 可信通道传入 `trusted_channel_input()`

## P4 — 图片规范化（#7）

`assistant-agent/src/normalization.rs`（新建）、`Cargo.toml`、`multimodal.rs`

- [x] P4.1 长边超过 2048 时等比缩放
- [x] P4.2 读取 EXIF orientation 并 `apply_orientation`
- [x] P4.3 保留原格式：PNG/JPEG 重编码（JPEG 质量 85），GIF/WebP 原样返回
- [x] P4.4 回填 `width` / `height`
- [x] P4.5 `admit_attachments` 入库前调用规范化

## P5 — Offload 降级（#6）

`assistant-agent/src/request_projection.rs`（新建）、`chat_request.rs`

- [x] P5.1 按 `context_window_tokens` 估算 token
- [x] P5.2 超预算时从最后一条用户消息逆序卸载 ImageBlock
- [x] P5.3 卸载文本走 `offloaded_image_text`
- [x] P5.4 仅作用于出站请求，不修改持久化消息

## 待办（本轮未做）

- [x] 图片输入格式扩展（安全部分）：
  - [x] JFIF：扩展名映射为 `image/jpeg`，零依赖
  - [x] BMP / TIFF：开启 `image` crate 的 `bmp` / `tiff` feature（纯 Rust），准入时解码转 PNG
  - [x] 前端 MIME 白名单、扩展名映射、`ImageMediaType` 同步扩展
  - [x] 部分平台不返回 MIME 时回退按扩展名判定
- [ ] 图片输入格式扩展（重依赖部分）：
  - [ ] HEIC / HEIF：ISOBMFF 容器 + HEVC 帧，需 libheif（C/C++）或系统 API（Windows WIC / macOS ImageIO），涉及构建链、打包与专利成本
  - [ ] AVIF：ISOBMFF 容器 + AV1 帧，需额外 AV1 解码器，编译时间与二进制体积明显增加
- [ ] PDF 降级（不支持 PDF 的模型仍需读取 PDF 内容）：
  - [ ] 纯文本提取：引入 `lopdf` / `pdf-extract` crate，从 PDF 提取文本层，丢失排版但模型可读
  - [ ] 图片渲染：调用外部工具（`pdftoppm` / Windows WIC）将 PDF 页转为图片，需 vision 能力
  - [ ] 混合模式：文本层 + 关键页图片，最完整但复杂度高
  - [ ] OpenAI message_converter 补充 PDF cached_data 分支（当前忽略 cached_data，即使 rehydrate 成功也走 handle_text 降级）
- [ ] 长文本文件优化：
  - [ ] 智能截断：当前硬截断 512KB 字符，改为按 token 预算截断，保留文件头尾（头保留结构/导入，尾保留最新内容）
  - [ ] 分块摘要：超长文件先由小模型生成摘要，再注入上下文
  - [ ] 多文件总量控制：当前仅单文件限制，多文件合计可能超 context_window，需在 project 阶段统筹预算
  - [ ] 流式注入：超大文件不一次性注入消息，改为工具调用按需读取（类似 RAG）

## 已完成（复核确认）

- [x] #4 数据层：`ModelConfig.input_modalities` / `ModelConfigDto` / 三家 discovery / 前端类型
- [x] #9 `AttachmentEntry.created_at` 从磁盘 mtime 读取
- [x] #9b 存储总量 500MB 告警

## 质量审查修复

- [x] #2 `cached_data` 路径补扩展名白名单（原先只校验 `file_path`）
- [x] #3 `rebuildContent` 保留 `file` 块（原先只保留 `image`，编辑消息会丢文件）
- [x] #4 附件数量改为统计整条消息全部块（原先只统计新附件，可被历史附件绕过）
- [x] #5 错误消息上限改为从 `MultimodalConfig` 取值（原先硬编码 4 / 8 / 16）
- [x] #6 OpenAI / Anthropic / Gemini 在 `cached_data` 为空时降级为文本（原先 OpenAI 会发空 base64 URI，Gemini 静默丢图）
- [x] #7 FileCard 支持点击/回车用系统程序打开附件
- [x] #8 offload 改为增量更新 token 估算，不再每轮全量 clone
- [x] #9 不可信通道增加符号链接校验
- [x] #10 `save_image` 更名为 `save_attachment`
- [x] #11 `rehydrate_files` 补充“仅 PDF”设计说明
- [x] #12 `support_pdf` 下沉到 `rehydrate_files`，移除外层重复判断
- [x] #13 重试时对文件型 URL 追加时间戳绕开缓存；`data:` URL 无缓存语义，保持原样
- [x] 补充：`normalize_image` 用实际字节校验格式，防止伪造 MIME
- 结论：#1 `resize` 不成立——`DynamicImage::resize` 本身保持宽高比

## 验收

- [ ] V2-1 ~ V2-11（见规格第 11 节，需实际模型联调）
- [ ] V3-1 ~ V3-4（见规格第 11 节）
- [x] `cargo fmt` / `cargo check --all-targets`（agent / providers / storage 通过）
- [x] Rust 单测：`assistant-agent`、`assistant-storage` 全部通过；`assistant-providers` 1 个既有失败（`context_window_known_models`，与本次改动无关）
- [x] 前端 `tsc` / lint / 181 个单测通过
- 备注：`assistant-app` 的 `protocol_contract` 测试存在既有编译错误（`cache_read_tokens` / `cache_write_tokens` 期望 `Option<u32>`），与本功能无关
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
- **结构化内容块**（参考 DSH `ContentBlock[]`）：`content` 列存储 `ContentBlock[]` 的 JSON 序列化，图片/文件/文本各为独立类型块，消除正则解析，获得编译期类型安全
- **旧数据兼容**：前端 `JSON.parse(content)` 成功则结构化渲染；失败（存量纯文本消息）则降级为 ReactMarkdown 渲染——无需一次性 DB 迁移
- **磁盘存储**（参考 OpenHuman AttachmentStash + DSH AttachmentStore）：图片字节写磁盘，`ImageBlock` 仅存 `attachmentId` 引用，避免 SQLite 行膨胀
- **请求投影**（参考 DSH `content.ts`）：FileBlock 永远不发给 Provider，投影为确定性 handle 文本；非 Vision 模型 ImageBlock 投影为 `textOnlyImageText()` 占位文本
- **渐进交付**：Phase 1 前端可用 + 后端 AttachmentStore（ContentBlock[] JSON 存储 + 磁盘附件）→ Phase 2 请求投影 + 模型能力声明 → Phase 3 offload 降级 + 图片规范化

**旧数据兼容**：
- 标记协议从未引入代码，不存在标记格式的存量数据需要兼容
- 当前存量消息全是纯文本 `string`，前端 `JSON.parse(content)` 失败即走 ReactMarkdown 渲染——这是唯一的兼容逻辑
- 新消息一律构造 `ContentBlock[]` 并 JSON 序列化存入 `content` 列

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

### 2.4 DSH (deepseek-harness-orb) — ContentBlock[] 全链路结构化

**核心模型**（`dsh-llm/types.ts` + `dsh-llm/message.ts`）：

- `Message.content: readonly ContentBlock[]` — **不是 `string`**，是结构化内容块数组
- `ContentBlock = TextBlock | ReasoningBlock | ImageBlock | FileBlock | ToolCallBlock | ToolAdditionBlock | ToolRemovalBlock`
- `ImageBlock { type: 'image'; attachment: ImageAttachmentRef; offloaded?: true }` — 图片是独立类型块，不是字符串标记
- `FileBlock { type: 'file'; attachment: FileAttachmentRef }` — 文件是独立类型块

**持久化**（`session-persistence-jsonl`）：

- JSONL 格式：每行 `JSON.stringify(event)`，`ContentBlock[]` 直接序列化为 JSON 数组
- 无 String 载体，无标记协议，无正则解析
- 格式版本递增（当前 v4），历史格式通过迁移链转换

**附件服务**（`dsh-attachment/AttachmentStore`）：

- `ImageAttachmentRef { attachmentId: AttachmentId; mediaType; bytes; width; height; name? }` — `attachmentId` 是 `sha256:` 品牌类型，不是文件路径
- `FileAttachmentRef { attachmentId; name; bytes }` — 内容寻址，相同文件相同 ID
- `AttachmentStore.saveImages()` — 校验 + 规范化 + 持久化，返回 durable reference
- `AttachmentStore.admitPromptContent()` — 前端上传 → 服务端校验 → 替换为 durable ref

**请求投影**（`dsh-llm/content.ts`）：

- `projectFilesToText(messages)` — FileBlock 永远不发给 Provider，投影为 `fileHandleText()` 确定性文本（文件名 + 字节数 + 只读路径）
- `projectImagesForTextModel(messages)` — 非 Vision 模型，ImageBlock 投影为 `textOnlyImageText()` 占位文本
- `offloadedImageText(ref)` — 请求预算超限时，图片替换为 offload 占位文本（含恢复路径）

**与当前项目的关系**：

| 维度 | DSH | 当前项目 |
|------|-----|---------|
| 消息模型 | `content: ContentBlock[]` | `content: string` |
| 持久化 | JSONL（JSON 数组） | SQLite TEXT（纯字符串） |
| 图片引用 | `ImageBlock.attachment: ImageAttachmentRef` | 计划用 `[Image: image #att:id]` 占位符 |
| 文件投影 | `projectFilesToText()` → TextBlock | 无 |
| 降级 | `textOnlyImageText()` / `offloadedImageText()` | 占位符即降级 |
| 解析 | 无（类型级区分） | 需正则或 JSON.parse |

**启示**：DSH 的 `ContentBlock[]` 模型消除了标记协议的全部复杂度（语法/语义/生命周期/误渲染防护），是当前项目的终态目标。当前项目受 SQLite TEXT 列约束，但 TEXT 列同样可存储 JSON 字符串——**直接将 `ContentBlock[]` JSON 序列化存入 TEXT 列**，前端 `JSON.parse` 解析，旧数据降级为 markdown 渲染，即可在不改 DB schema 的前提下获得结构化存储的全部收益。

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

**SQLite `messages.content` 列**存 `ContentBlock[]` 的 JSON 序列化：

```json
[
  {"type": "text", "text": "请描述这两张图片"},
  {"type": "image", "attachment": {"attachmentId": "sha256:a1b2c3d4...", "mediaType": "image/png", "bytes": 12345, "width": 800, "height": 600}},
  {"type": "image", "attachment": {"attachmentId": "sha256:e5f6g7h8...", "mediaType": "image/jpeg", "bytes": 23456, "width": 1024, "height": 768}}
]
```

- 纯文本消息：`[{"type": "text", "text": "你好"}]`
- 图片消息：TextBlock + ImageBlock，ImageBlock 仅存 `attachmentId` 引用（~80 字节），图片字节在磁盘
- 文件消息：TextBlock + FileBlock，FileBlock 仅存 `attachmentId` + `name` + `bytes` 引用
- 旧数据（纯文本，非 JSON）：前端 `JSON.parse` 失败，降级为 ReactMarkdown 渲染

**存储体积对比**：

| 方案 | 单条图片消息 content 体积 | 说明 |
|------|--------------------------|------|
| ContentBlock[] JSON + 磁盘引用 | ~120 字节 | ImageBlock JSON ~80 字节 + TextBlock ~40 字节 |
| base64 直存 | ~1.3× 原始 MB | ❌ 不采用，SQLite 行膨胀 |
| 标记占位符 | ~50 字节 | `[Image: image #att:id]`（已弃用） |

---

## 3.5 标记协议规约（Marker Protocol Specification）— 已弃用

> ⚠️ **本节内容已弃用**。标记协议从未引入代码，实施路径已改为 `ContentBlock[]` JSON 存储（见 §3.4）。
> 本节保留作为设计参考：标记协议的 Ingress/Dispatch/Rehydrate 管道设计、AttachmentStash 磁盘存储策略、
> 以及占位符/Rehydrate 的思路仍可借鉴于 AttachmentStore 实现。

> 原始备注：本节将标记协议从"实现约定"提升为**协议规约**，定义语法、语义、生命周期和扩展点。
> 协议版本：**v1**。后续扩展通过版本号协商，不破坏 v1 消费者。

### 3.5.1 语法（Syntax）

```
marker       ::= image_marker | file_marker | placeholder_marker
image_marker ::= "[IMAGE:" data_or_url "]"
file_marker  ::= "[FILE:"  data_or_url "]"
placeholder  ::= "[Image: image #att:" hex_id "]"
              |  "[File:"  filename  " #att:" hex_id "]"
              |  "[File-EXTRACTED:" filename " #att:" hex_id "]"
              |  "[File-ATTACHED:"  filename " #att:" hex_id "]"

data_or_url  ::= data_uri | http_url
data_uri     ::= "data:" mime_type ";base64," base64_payload
http_url     ::= "https://" path   ; 仅当 allow_remote_fetch = true
hex_id       ::= [0-9a-f]{16}      ; sha256 前 16 hex 字符
filename     ::= [^\]]+             ; ⚠ 含 ] 的文件名需转义，见 3.5.4
mime_type    ::= image_mime | file_mime
image_mime   ::= "image/png" | "image/jpeg" | "image/webp" | "image/gif"
file_mime    ::= "application/pdf" | "text/plain" | "text/markdown" | "text/csv"
```

**标记在 `content: String` 中的位置**：标记可出现在 content 的任意位置，与正文以空格分隔。多个标记按出现顺序排列，对应附件的发送顺序。

**示例**：

```
请描述这两张图片 [IMAGE:data:image/png;base64,iVBOR...] [IMAGE:data:image/jpeg;base64,/9j/...]
```

```
请总结这个文件 [FILE:data:application/pdf;base64,JVBERi...]
```

### 3.5.2 语义（Semantics）

标记的语义因 **role** 和 **处理阶段** 而异：

| 标记 | role | 阶段 | 语义 |
|------|------|------|------|
| `[IMAGE:data:…]` | User | Ingress | 用户粘贴/拖拽的图片，待 stash 到磁盘 |
| `[IMAGE:data:…]` | User | Dispatch | 已 rehydrate 的图片，待 Provider 转换 |
| `[IMAGE:data:…]` | Tool | Ingress | 工具返回的图片结果，待合成 |
| `[IMAGE:data:…]` | Assistant | — | **v1 不产生**；v2 预留（模型原生输出图片） |
| `[Image: image #att:id]` | 任意 | 存储 | 磁盘占位符，Ingress 后、Dispatch 前 |
| `[FILE:data:…]` | User | Ingress | 用户选择的文件，待提取/stash |
| `[File-EXTRACTED:… #att:id]` | User | 存储 | 文件已提取文本内联，占位符仅作引用 |
| `[File-ATTACHED:… #att:id]` | User | 存储 | 文件无法提取，仅保留引用存根 |

**关键语义约束**：

1. **v1：`[IMAGE:]` 仅出现在 User 和 Tool 的 content 中**。Assistant 的 content 不含图片标记。
2. **占位符是存储态**：`[Image: image #att:id]` 仅存在于 SQLite `messages.content`，不发给 Provider。
3. **标记是线态**：`[IMAGE:data:…]` 存在于发给 Provider 的消息中，不存入 SQLite（Phase 2+）。

### 3.5.3 生命周期（Lifecycle）

```
                    ┌─────────────────────────────────────────────────┐
                    │              标记生命周期                        │
                    └─────────────────────────────────────────────────┘

  用户输入                Ingress               存储              Dispatch           Provider
  ────────              ────────              ──────            ─────────          ────────
  [IMAGE:data:…]  ──→  stash() 写磁盘   ──→  [Image: #att:id] ──→  rehydrate()  ──→  [IMAGE:data:…]
                       校验+sha256           SQLite 占位符        读回磁盘          Provider 格式
                                                                 (仅 Vision)

  [FILE:data:…]   ──→  inline_file()   ──→  [File-EXTRACTED:…]  ──→  直传文本     ──→  text block
                       提取文本              或 [File-ATTACHED:…]     (已内联)
```

**阶段说明**：

| 阶段 | 函数 | 输入 | 输出 | 条件 |
|------|------|------|------|------|
| Ingress | `stash_image_attachments()` | `[IMAGE:data:…]` | `[Image: image #att:id]` | 始终执行 |
| Ingress | `inline_file_attachments()` | `[FILE:data:…]` | `[File-EXTRACTED:… #att:id]` | 始终执行 |
| Dispatch | `rehydrate_image_placeholders()` | `[Image: image #att:id]` | `[IMAGE:data:…]` | 仅 Vision 模型 |
| Provider | `parse_image_markers()` | `[IMAGE:data:…]` | Provider 原生格式 | 按 Provider 分支 |

### 3.5.4 已知限制与缓解

| 限制 | 影响 | 缓解 |
|------|------|------|
| `]` 在标记 payload 中被视为终止符 | 文件名含 `]` 的 `[FILE:…]` 标记会被截断 | **v1**：禁止文件名含 `]`（前端校验替换为 `_`）；**v2**：改用 `[FILE:id\|filename]` 分隔格式 |
| base64 payload 中含 `]` | 理论上 base64 字符集含 `]`，但实际 base64 alphabet（A-Z, a-z, 0-9, +, /, =）**不含 `]`** | ✅ 安全，正则 `[IMAGE:([^\]]+)]` 不会误截断 |
| `[IMAGE:]` 嵌套 | 标记内含标记导致解析歧义 | Ingress 校验拒绝嵌套标记 |
| 大 payload 正则性能 | 单次 `replace` 扫描 MB 级 content | Phase 2 后 content 中只有 ~50 字节占位符，大 payload 仅在 Dispatch 后短暂存在 |

### 3.5.5 前瞻：协议扩展点

> 以下扩展点不在 v1 实施范围内，但协议设计已为其预留兼容路径。
> 扩展遵循**加法原则**：v1 消费者遇到未知标记时保持原样（pass-through），不报错。

#### E1. Tool Result 图片合成

**场景**：工具（如 `generate_image`、`screenshot`）返回图片，图片在 Tool role 的 content 中。

**v1 状态**：Tool result 的 `content` 为纯文本，无图片。

**扩展路径**：

```
Tool result content = "[IMAGE:data:image/png;base64,...]"
                         ↓
            合成为额外 User message:
            { role: "user", content: [{type: "image_url", ...}] }
                         ↓
            追加到 messages 列表末尾（在下一个 assistant 消息之前）
```

**协议变更**：`[IMAGE:]` 标记的合法 role 从 `{User}` 扩展为 `{User, Tool}`。
**兼容性**：v1 的 `parse_image_markers` 不区分 role，对 Tool content 中的标记同样解析。**无需修改解析逻辑**，只需在 dispatch 层增加 Tool → User 合成步骤。

**参考实现**：
- OpenBitFun `openai/message_converter.rs:192`：延迟合成 user message，在 tool batch 后追加
- OpenBitFun `gemini/message_converter.rs:126`：Gemini 3 直接在 `functionResponse.parts` 中放 `inlineData`
- AIRI `replaceToolResultImages()`：非 Vision 模型将 tool result 图片替换为文字注释

#### E2. Assistant 原生图片输出

**场景**：模型直接在 response 中返回图片（如 Gemini + Imagen、未来 GPT 多模态输出）。

**v1 状态**：Assistant content 不含 `[IMAGE:]` 标记。

**扩展路径**：

```
模型 response content = [{type: "text", text: "..."}, {type: "image", ...}]
                         ↓
            流式输出完成时，将 image block 编码为 [IMAGE:data:...] 标记
            存入 content: String（与 user 消息同格式）
                         ↓
            Ingress: stash_image_attachments() 正常处理（role-agnostic）
            Dispatch: rehydrate_image_placeholders() 正常处理
            Provider: convert_assistant_message() 新增 parse_image_markers() 调用
```

**协议变更**：`[IMAGE:]` 标记的合法 role 从 `{User, Tool}` 扩展为 `{User, Tool, Assistant}`。
**兼容性**：标记协议的 `content: String` 载体天然 role-agnostic——`parse_image_markers` 的正则不关心 role，统一解析。**v1 的存储、Ingress、Dispatch 逻辑无需修改**，仅需在 `convert_assistant_message()` 中增加标记解析调用。

**这是标记协议相对于类型级区分（`MessageContent` 枚举）的核心优势**：类型级区分需要在枚举定义中为 Assistant 变体添加 `images` 字段，改动所有 match 分支；标记协议只需在一个函数中加一行解析调用。

#### E3. 视频附件

**场景**：用户粘贴/拖拽视频文件，发送给支持视频的模型。

**v1 状态**：不支持。

**扩展路径**（参考 OpenHuman `attachments.ts`）：

```
[VIDEO:data:video/mp4;base64,...]        ; 整体标记（v2）
或
[IMAGE:data:image/jpeg;base64,frame1]    ; 帧采样（v1 兼容）
[IMAGE:data:image/jpeg;base64,frame2]
```

OpenHuman 实测：视频帧采样（`VIDEO_FRAME_COUNT = 2`）比整体视频标记更实用——大多数 Vision 模型不支持视频输入，但支持多张图片。帧采样将视频降级为多张图片，复用现有 `[IMAGE:]` 路径。

**协议变更**：新增 `[VIDEO:]` 标记类型，或复用 `[IMAGE:]` + 帧采样。
**兼容性**：帧采样方案**零协议变更**——v1 的 `[IMAGE:]` 标记和解析逻辑完全复用。

#### E4. 协议版本协商

**场景**：未来标记格式变更（如分隔符从 `]` 改为 `|`），需要新旧消费者共存。

**扩展路径**：

```
content 开头可选协议声明：
[MARKER-PROTOCOL:v2] 正文 [IMAGE:id|filename] ...
```

- v1 消费者：不识别 `[MARKER-PROTOCOL:v2]` 前缀，按 v1 规则解析 → 遇到 `|` 分隔的标记不匹配 `[IMAGE:([^\]]+)]` → 保持原样（pass-through）
- v2 消费者：识别前缀，按 v2 规则解析

**兼容性**：加法原则保证 v1 消费者不会崩溃，只是不渲染新格式标记。

#### E5. 终态替代：结构化内容块（ContentBlock[]）— 已直接采用

> ⚠️ 本扩展点原设计为"从标记协议迁移到 ContentBlock[]"，但标记协议从未引入代码。
> 实施路径已直接采用 `ContentBlock[]` JSON 存储（见 §3.4），无需迁移。

**当前状态**：`content` 列存储 `ContentBlock[]` JSON 序列化，图片/文件为独立类型块。

**ContentBlock 定义**（参考 DSH `dsh-llm/types.ts`）：

```typescript
type ContentBlock = TextBlock | ImageBlock | FileBlock | ToolCallBlock | ReasoningBlock

interface TextBlock      { type: 'text'; text: string }
interface ImageBlock     { type: 'image'; attachment: ImageAttachmentRef; offloaded?: true }
interface FileBlock      { type: 'file'; attachment: FileAttachmentRef }
interface ToolCallBlock  { type: 'tool-call'; id: string; name: string; arguments: string }
interface ReasoningBlock { type: 'reasoning'; text: string }
```

```rust
enum ContentBlock {
    Text { text: String },
    Image { attachment: ImageAttachmentRef, offloaded: Option<bool> },
    File { attachment: FileAttachmentRef },
    ToolCall { id: String, name: String, arguments: String },
    Reasoning { text: String },
}
```

**收益**：
- 编译期类型安全（`ImageBlock` vs 正则解析 `[IMAGE:...]`）
- 无标记协议的语法/语义/生命周期复杂性
- Offload 降级（请求预算管理，DSH 独有）
- `inputModalities: ('text'|'image')[]` 模型能力声明（比 `supportsVision: bool` 更具扩展性）
- `FileBlock` + `fileHandleText()` 文件投影（Provider 永远不收 file block，投影为确定性文本）
- 旧数据兼容零成本：`JSON.parse` 失败 → ReactMarkdown 渲染

### 3.5.6 协议设计原则总结

> ⚠️ 本节原为标记协议的设计原则，标记协议已弃用。以下原则中，仅"惰性 Rehydrate"和"请求投影"仍适用于 ContentBlock[] 方案。

| 原则 | 标记协议体现 | ContentBlock[] 对应 |
|------|-------------|-------------------|
| Role-agnostic 解析 | `parse_image_markers` 不区分 role | `ContentBlock[]` 按 `type` switch，天然 role-agnostic |
| 加法扩展 | 新标记类型不破坏 v1 消费者 | 新 `ContentBlock` type 加入联合类型，unknown type pass-through |
| String 载体 | `content: String` 不变 | `content: String`（JSON），全链路类型签名不变 |
| 存储与线态分离 | 占位符 vs 完整标记 | `ImageBlock.attachment`（引用）vs dispatch 时读回字节 |
| 惰性 Rehydrate | 仅 Vision 模型读回磁盘 | 仅 Vision 模型 dispatch 时从 AttachmentStore 读回图片字节 |
| 请求投影 | — | `projectFilesToText()` + `projectImagesForTextModel()` |

---

## 4. 设计方案

### 4.1 架构总览

```
┌──────────────────────────────────────────────────────────────────────────┐
│  前端输入层                                                              │
│  MessageInput → 附件按钮 + 粘贴 + 拖拽 → Attachment[]                   │
│  buildContentBlocks(text, attachments) → ContentBlock[]                  │
│  JSON.stringify(blocks) → content (JSON string)                         │
├──────────────────────────────────────────────────────────────────────────┤
│  前端发送层                                                              │
│  chatService.sendChatMessage(convId, content)  ← 签名不变               │
├──────────────────────────────────────────────────────────────────────────┤
│  Tauri Command 层                                                       │
│  send_chat_message(conv_id, content, ...)  ← 签名不变                   │
├──────────────────────────────────────────────────────────────────────────┤
│  Rust Ingress 管道                                                      │
│  admit_attachments(content) → ImageBlock 存磁盘 + 返回 ContentBlock[]   │
│  project_files(content) → FileBlock → TextBlock (文件投影)              │
├──────────────────────────────────────────────────────────────────────────┤
│  SQLite 存储                                                            │
│  messages.content = JSON.stringify(ContentBlock[])                      │
│  例: [{"type":"text","text":"描述图片"},{"type":"image","attachment":…}] │
├──────────────────────────────────────────────────────────────────────────┤
│  Rust Dispatch 管道                                                     │
│  rehydrate_images(messages) → Vision 模型: 从磁盘读回图片字节           │
│  project_images_for_text_model(messages) → 非 Vision: ImageBlock→Text   │
│  project_files_to_text(messages) → FileBlock→TextBlock (始终投影)       │
├──────────────────────────────────────────────────────────────────────────┤
│  Provider 消息转换（遍历 ContentBlock[]，按 type 分发）                 │
│  TextBlock  → text content part                                         │
│  ImageBlock → image content part (Vision) / textOnlyImageText (非Vision)│
│  FileBlock  → fileHandleText (始终投影，Provider 不收 FileBlock)        │
│  ── OpenAI    → content_parts [{type:"text"}, {type:"image_url"}]       │
│  ── Anthropic → content [{type:"text"}, {type:"image",source:{base64}}] │
│  ── Gemini    → parts [{text}, {inline_data}]                           │
│  ── Ollama    → images[] + prompt                                       │
├──────────────────────────────────────────────────────────────────────────┤
│  前端渲染层                                                              │
│  JSON.parse(content) 成功 → ContentBlock[] → 按 type 遍历渲染          │
│  JSON.parse(content) 失败 → 旧数据，ReactMarkdown 渲染                  │
│  TextBlock → ReactMarkdown   ImageBlock → <ImageThumbnail>             │
│  FileBlock → <FileCard>      ReasoningBlock → <ReasoningBlock>         │
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
  // content 仍为 string，但存储 ContentBlock[] JSON 序列化
  // 旧数据: content = "纯文本" → JSON.parse 失败 → 降级为 markdown 渲染
  // 新数据: content = '[{"type":"text","text":"..."},{"type":"image",...}]' → JSON.parse 成功 → 结构化渲染
}
```

#### 4.2.3 内容块类型（新增 `src/chat/core/contentBlockTypes.ts`）

参考 DSH `dsh-llm/types.ts`：

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
  attachmentId: string          // sha256: + 64 hex 字符
  mediaType: ImageMediaType
  bytes: number
  width: number
  height: number
  name?: string
}

export interface FileAttachmentRef {
  attachmentId: string
  name: string
  bytes: number
}

export type ImageMediaType = 'image/png' | 'image/jpeg' | 'image/webp' | 'image/gif'
```

### 4.3 前端内容块工具（新增 `src/chat/core/contentBlockUtils.ts`）

核心函数：

```typescript
export function buildContentBlocks(text: string, attachments: Attachment[]): ContentBlock[]

export function parseContent(content: string): ContentBlock[] | null

export function serializeContent(blocks: ContentBlock[]): string

export async function validateAndReadFile(file: File): Promise<{ attachment: Attachment } | { error: AttachmentError }>
```

**`buildContentBlocks` 实现**：

```typescript
export function buildContentBlocks(text: string, attachments: Attachment[]): ContentBlock[] {
  const blocks: ContentBlock[] = []
  if (text.trim()) blocks.push({ type: 'text', text: text.trim() })
  for (const a of attachments) {
    if (a.kind === 'image') {
      blocks.push({
        type: 'image',
        attachment: {
          attachmentId: a.attachmentId,
          mediaType: a.mimeType as ImageMediaType,
          bytes: a.originalSizeBytes,
          width: a.width!,
          height: a.height!,
        },
      })
    }
    if (a.kind === 'file') {
      blocks.push({
        type: 'file',
        attachment: {
          attachmentId: a.attachmentId,
          name: a.file.name,
          bytes: a.originalSizeBytes,
        },
      })
    }
  }
  return blocks
}
```

**`parseContent` 实现** — 旧数据兼容的核心：

```typescript
export function parseContent(content: string): ContentBlock[] | null {
  try {
    const parsed = JSON.parse(content)
    if (Array.isArray(parsed) && parsed.length > 0 && parsed[0]?.type) {
      return parsed as ContentBlock[]
    }
    return null
  } catch {
    return null
  }
}

export function serializeContent(blocks: ContentBlock[]): string {
  return JSON.stringify(blocks)
}
```

### 4.4 前端输入组件变更

**`MessageInput.tsx` 变更**：

- 新增附件按钮（📎 图标，参考 OpenHuman `ComposerPrimitive.AddAttachment`）
- 新增粘贴图片支持（`onPaste` 事件检测 `clipboardData.items` 中 image 类型）
- 新增拖拽附件支持（`onDrop` 事件检测 `dataTransfer.files`）
- 新增附件预览条（输入框上方，参考 OpenHuman `ComposerPrimitive.Attachments`）
- 附件预览条中每个附件显示缩略图/文件名 + 删除按钮
- `onSend` 签名不变：`onSend: (content: string) => void`，但 content 为 `JSON.stringify(ContentBlock[])`

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

**`MessageContent.tsx` 变更** — 结构化渲染 + 旧数据降级：

```typescript
function MessageContent({ content }: { content: string }) {
  const blocks = useMemo(() => parseContent(content), [content])

  if (blocks) {
    return (
      <div className="message-content-blocks">
        {blocks.map((block, i) => {
          switch (block.type) {
            case 'text':
              return <ReactMarkdown key={i}>{block.text}</ReactMarkdown>
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

  return <ReactMarkdown>{content}</ReactMarkdown>
}
```

- `parseContent()` 返回 `ContentBlock[]` → 结构化渲染：TextBlock 走 ReactMarkdown，ImageBlock 走缩略图，FileBlock 走文件卡片
- `parseContent()` 返回 `null` → 旧数据降级：整段 content 走 ReactMarkdown 渲染
- 无正则解析，无标记协议，无误渲染风险

### 4.6 Rust 后端变更

#### 4.6.1 AttachmentStore（新增 `assistant-storage/src/attachment_store.rs`）

参考 DSH `AttachmentStore` + OpenHuman `AttachmentStash`，核心结构：

```rust
pub struct AttachmentStore {
    dir: PathBuf,
    max_bytes: u64,
    ttl: Duration,
}

impl AttachmentStore {
    pub fn new(dir: PathBuf, max_bytes: u64, ttl: Duration) -> Self;

    pub async fn save_image(&self, data: &[u8], media_type: &str) -> Result<ImageAttachmentRef>;

    pub async fn save_file(&self, data: &[u8], name: &str) -> Result<FileAttachmentRef>;

    pub async fn read(&self, id: &str) -> Result<Vec<u8>>;

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
pub async fn admit_attachments(
    content: &str,
    config: &MultimodalConfig,
) -> Result<String>;

pub fn project_files_to_text(
    message: &str,
    file_config: &MultimodalFileConfig,
) -> Result<String>;
```

**`admit_attachments` 流程**（参考 DSH `AttachmentStore.admitPromptContent`）：

1. `parse_content(message)` → `ContentBlock[]`（JSON 反序列化）
2. 对每个 `ImageBlock`：
   - 校验数量 ≤ `max_images`（默认 4）
   - 校验大小 ≤ `max_image_bytes`（默认 8MB）
   - `attachment_store.save_image(data, media_type)` → 写入磁盘，返回 `ImageAttachmentRef`
   - 替换 ImageBlock 的 attachment 为持久化后的 ref
3. 返回更新后的 `JSON.stringify(ContentBlock[])`

#### 4.6.3 Dispatch 管道

```rust
pub fn rehydrate_images(messages: &[ChatMessage], is_vision_model: bool) -> Vec<ChatMessage>;

pub fn project_images_for_text_model(messages: &[ChatMessage]) -> Vec<ChatMessage>;

pub fn project_files_to_text(messages: &[ChatMessage]) -> Vec<ChatMessage>;
```

**`rehydrate_images` 流程**（参考 DSH）：

1. 仅 Vision 模型调用：遍历 `ContentBlock[]`，对 `ImageBlock` 从磁盘读回图片字节
2. 非 Vision 模型调用 `project_images_for_text_model`：`ImageBlock` → `TextBlock(textOnlyImageText())`
3. `project_files_to_text`：`FileBlock` → `TextBlock(fileHandleText())`（Provider 永远不收 FileBlock）
4. 占位符 ID 不存在（文件被驱逐/清扫）→ 保持 `textOnlyImageText()` 占位文本

#### 4.6.4 初始化

在 `setup.rs` 的 `init()` 中，数据库初始化之后新增：

```rust
let attachments_dir = data_dir.join("attachments");
std::fs::create_dir_all(&attachments_dir)?;
assistant_agent::multimodal::init_attachment_store(attachments_dir);
```

#### 4.6.5 Provider 消息转换器变更

**OpenAI**（`openai/message_converter.rs`）：

当前 `json!({"content": m.content})` → 改为遍历 `ContentBlock[]`，按 type 生成 content parts：

```rust
fn convert_user_message(msg: &ChatMessage) -> Value {
    let blocks = parse_content(&msg.content);
    if blocks.is_empty() {
        return json!({"role": "user", "content": msg.content});
    }
    let mut parts = Vec::new();
    for block in &blocks {
        match block {
            ContentBlock::Text(t) => parts.push(json!({"type": "text", "text": t.text})),
            ContentBlock::Image(img) => {
                if let Some(data) = img.cached_data() {
                    parts.push(json!({
                        "type": "image_url",
                        "image_url": { "url": format!("data:{};base64,{}", img.attachment.media_type, data) }
                    }));
                }
            }
            ContentBlock::File(_) => {} // FileBlock 已被 project_files_to_text 投影
        }
    }
    json!({"role": "user", "content": parts})
}
```

**Anthropic**（`anthropic/message_converter.rs`）：

```rust
fn convert_user_message(msg: &ChatMessage) -> Value {
    let blocks = parse_content(&msg.content);
    let mut content_blocks = Vec::new();
    for block in &blocks {
        match block {
            ContentBlock::Text(t) => content_blocks.push(json!({"type": "text", "text": t.text})),
            ContentBlock::Image(img) => {
                if let Some(data) = img.cached_data() {
                    content_blocks.push(json!({
                        "type": "image",
                        "source": { "type": "base64", "media_type": img.attachment.media_type, "data": data }
                    }));
                }
            }
            ContentBlock::File(_) => {}
        }
    }
    json!({"role": "user", "content": content_blocks})
}
```

**Gemini**（`gemini/message_converter.rs`）：

扩展 `try_parse_inline_image` 处理 ImageBlock，生成 `inlineData` part。

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

### Phase 1：前端可用 + 后端 AttachmentStore

> ContentBlock[] JSON 存储 + 磁盘附件存储。ImageBlock 从一开始就只存 attachmentId 引用，图片字节写磁盘。不经过 base64 直存 SQLite 的中间态。

| Step | 内容 | 涉及文件 |
|------|------|---------|
| **1a** | 新增 `contentBlockTypes.ts` + `contentBlockUtils.ts` | `src/chat/core/contentBlockTypes.ts` (新), `src/chat/core/contentBlockUtils.ts` (新) |
| **1b** | `MessageInput.tsx` 添加附件按钮 + 粘贴 + 拖拽 + 预览条 | `src/chat/message/MessageInput.tsx` |
| **1c** | `chatStore.sendMessage` 中调用 `buildContentBlocks` + `serializeContent` | `src/stores/chatStore.ts` |
| **1d** | `MessageContent.tsx` 结构化渲染：`parseContent` 成功遍历 blocks，失败降级 markdown | `src/chat/message/MessageContent.tsx` |
| **1e** | 新增 `ImageThumbnail` + `FileCard` 渲染组件 | `src/chat/message/ImageThumbnail.tsx` (新), `src/chat/message/FileCard.tsx` (新) |
| **1f** | 模型配置新增 `supportVision` 字段 | `src/stores/providerStore.ts`, Rust `ModelConfig` |
| **1g** | 新增 `attachment_store.rs`（AttachmentStore：磁盘存储 + TTL 清扫 + 驱逐） | `src-tauri/crates/assistant-storage/src/attachment_store.rs` (新) |
| **1h** | 新增 `multimodal.rs`（admit_attachments + rehydrate + project） | `src-tauri/crates/assistant-agent/src/multimodal.rs` (新) |
| **1i** | `setup.rs` 初始化 attachments 目录 | `src-tauri/src/setup.rs` |
| **1j** | `send_chat_message` 中调用 `admit_attachments`（图片存磁盘，content 存引用） | `src-tauri/src/cmd/chat.rs` |
| **1k** | dispatch 前调用 `rehydrate_images`（Vision）/ `project_images_for_text_model`（非 Vision） | `src-tauri/crates/assistant-agent/src/chat_runner.rs` |
| **1l** | Provider 消息转换器遍历 `ContentBlock[]` 按 type 分发 | 3 个 `message_converter.rs` |
| **1m** | 前端 `ImageThumbnail` 通过 Tauri command 从磁盘读取图片 | `src/chat/message/ImageThumbnail.tsx`, `src-tauri/src/cmd/chat.rs` |

**Phase 1 验收**：用户可粘贴/拖拽/选择图片 → 发送 → 消息中显示图片 → 历史消息加载后图片仍显示；旧纯文本消息正常渲染；SQLite `messages.content` 中 ImageBlock 仅含 attachmentId 引用（~80 字节）；磁盘 `<app_data_dir>/attachments/` 中有图片文件

### Phase 2：请求投影 + 模型能力声明

> Phase 1 可用后实施。完善 Provider 侧的投影逻辑和模型能力感知。

| Step | 内容 | 涉及文件 |
|------|------|---------|
| **2a** | `project_files_to_text` 实现（FileBlock → TextBlock，Provider 不收 FileBlock） | `src-tauri/crates/assistant-agent/src/multimodal.rs` |
| **2b** | 前端文件选择器（非图片文件） | `src/chat/message/MessageInput.tsx` |
| **2c** | `inputModalities: Vec<InputModality>` 替代 `supports_vision: bool` | `ModelConfig`, 前端设置页 |
| **2d** | `prompt_builder.rs` 修复 tool_calls 丢失 | `src-tauri/crates/assistant-agent/src/prompt_builder.rs` |
| **2e** | 不可信通道 `max_files: 0` 哨兵 + 路径遍历防护 + MIME 白名单严格校验 | `src-tauri/crates/assistant-agent/src/multimodal.rs` |

**Phase 2 验收**：可选择 PDF/文本文件 → 发送 → 模型可阅读文件内容；不可信通道拒绝文件附件；非 Vision 模型显示 `textOnlyImageText()` 占位符；24h 后启动清扫过期附件

### Phase 3：Offload 降级 + 图片规范化

| Step | 内容 | 涉及文件 |
|------|------|---------|
| **3a** | offload 降级（请求预算超限时 ImageBlock → TextBlock） | `src-tauri/crates/assistant-agent/src/request_projection.rs` (新) |
| **3b** | 图片规范化（resize/重编码/EXIF 旋转） | `src-tauri/crates/assistant-storage/src/normalization.rs` (新) |
| **3c** | `textOnlyImageText()` / `offloadedImageText()` 降级文本 | `src-tauri/crates/assistant-agent/src/content.ts` (新) |

**Phase 3 验收**：请求预算超限时图片降级为文本；图片自动规范化（大图缩放、EXIF 旋转）；非 Vision 模型显示可读的占位符文本

---

## 6. 与现有体系的关系

| 现有体系 | I2e 变更 | 兼容性 |
|---------|---------|--------|
| `ChatMessage.content: String` | 不变，存 `ContentBlock[]` JSON 序列化 | ✅ 类型签名不变 |
| `MessageRow.content: TEXT` | 不变，存 JSON 字符串 | ✅ DB schema 不变 |
| `send_chat_message(conv_id, content, ...)` | 签名不变，content 为 JSON | ✅ 完全兼容 |
| `chatStore.sendMessage(content: string)` | 签名不变，content 为 JSON | ✅ 完全兼容 |
| 旧纯文本消息 | `JSON.parse` 失败 → ReactMarkdown 渲染 | ✅ 旧数据兼容 |
| `ContentBlockType::Image` | 已有定义，Phase 2 启用 | ✅ 已预留 |
| `anthropic/message_converter.rs` tool_call_id hack | Phase 2 移除，改为 ContentBlock[] 遍历 | 🔄 行为变更，更正确 |
| `gemini/message_converter.rs` try_parse_inline_image | Phase 2 扩展，处理 ImageBlock | 🔄 行为增强 |
| `prompt_builder.rs` | 修复 tool_calls 丢失 bug | 🔄 bug 修复 |
| `MessageInput.tsx` | 新增附件 UI | 🔄 UI 增强 |
| `MessageContent.tsx` | 结构化渲染 + 旧数据降级 | 🔄 UI 增强 |

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
| V9 | Phase 1：SQLite `messages.content` 中 ImageBlock 仅含 attachmentId 引用 | 查询数据库验证 |
| V10 | Phase 1：磁盘 `<app_data_dir>/attachments/` 中有图片文件 | 检查文件系统 |
| V11 | Phase 1：24h 后重启 → 过期附件被清扫 | 修改文件 mtime 为 25h 前，重启 |
| V12 | 旧纯文本消息 → 正常 markdown 渲染，无报错 | 打开含旧消息的对话 |
| V13 | Phase 2：附件总量超 256MB → 最旧文件被驱逐 | 写入大量附件 |
| V14 | Phase 3：PDF 文件 → 模型可阅读 PDF 内容 | 发送 PDF + "总结这个文件" |
| V15 | Phase 3：不可信通道 → 文件附件被拒绝 | MCP/外部输入携带文件标记 |
| V16 | 历史消息加载后图片仍正确显示 | 重启应用，打开有图片的对话 |
| V17 | Anthropic 转换器不再使用 tool_call_id hack | 代码审查 + 发送图片到 Anthropic 模型 |
| V18 | prompt_builder 保留 tool_calls 信息 | 发送含工具调用的对话，重发时工具调用不丢失 |
| V19 | `parseContent` 对非 JSON content 返回 null，降级为 markdown | 构造纯文本 content，验证渲染正确 |
| V20 | `parseContent` 对合法 ContentBlock[] JSON 返回结构化数组 | 构造 JSON content，验证解析结果 |
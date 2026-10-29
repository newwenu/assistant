# 多模态附件能力引入分析

> 生成日期：2026-10-21
> 范围：附件处理全链路 — 前端输入/渲染 → Tauri command → Rust ingress/dispatch → Provider 转换 → SQLite 存储
> 性质：**纯分析文档**，描述当前代码结构、参考项目做法、影响评估与可能的演进方向，不作为执行计划
> 目标：评估当前 chat runtime 对多模态附件的支撑现状，分析标记协议/磁盘存储/rehydrate 策略的渗透范围与引入路径

---

## 1 现状：图片相关假设的渗透地图

当前项目对"消息仅含文本"的假设散落在前后端多处。以下按层级标注已存在的图片相关代码和隐式约束：

### 1.1 前端层

| 文件 | 行号 | 现状 | 语义 |
|------|------|------|------|
| `MessageInput.tsx` | L1-60 | 纯文本 `Input` 组件，`onSend: (content: string) => void` | 无附件入口、无粘贴/拖拽图片支持 |
| `MessageContent.tsx` | - | 渲染 `content: string` 为 Markdown | 无 `[IMAGE:]` 标记解析、无 `<img>` 渲染 |
| `chatTypes.ts` | - | `Message.content: string` | 类型上可承载标记，但无 `imageUris` / `fileReferences` 字段 |
| `chatStore.ts` | - | `sendMessage(content: string)` | 签名不变，但调用前需注入标记 |
| `useFileDrop.ts` | - | 存在文件拖拽 hook | 已有基础设施，未接入 MessageInput |
| `providerStore.ts` | - | 模型配置 | 无 `supportVision` 字段，无法按模型能力隐藏/显示附件入口 |

### 1.2 Tauri Command 层

| 文件 | 行号 | 现状 | 语义 |
|------|------|------|------|
| `chat.rs` | L88/L108/L138/L166/L186 | `tool_calls_json: None` | 发送消息时硬编码 None，新建消息不含 tool_calls |
| `chat.rs` | L308/L318 | 流式结束后写入 `tool_calls_json` | ✅ tool_calls 持久化路径存在 |
| `chat_stream.rs` | L80/L96 | `tool_calls_json: Option<String>` | DTO 已定义，流式消息可携带 |
| `chat_stream.rs` | L114 | 传递 `msg.tool_calls_json` | ✅ 前端可收到 tool_calls |

### 1.3 Rust Agent 层

| 文件 | 行号 | 现状 | 语义 |
|------|------|------|------|
| `prompt_builder.rs` | L46/L60/L74 | `tool_calls: None, tool_call_id: None` | **从 MessageRow 构建 ChatMessage 时丢弃 tool_calls** |
| `prompt_builder.rs` | L15-30 | `AgentKind` enum: Default/Quick/Deep/Custom | 无 Vision 能力声明，`supports_tool_call` 仅 Custom 返回 true |
| `chat_runtime.rs` | L139/L216 | 处理 `delta_tool_call_id` | ✅ 流消费引擎已支持 tool_call_id delta |
| `chat_runtime.rs` | - | `StreamAccumulator` / `ToolCallAccumulator` | ✅ 纯增量累积，与消息内容无关 |

### 1.4 Provider 转换层（关键：已有图片处理的 hack 和部分实现）

| 文件 | 行号 | 现状 | 语义 |
|------|------|------|------|
| `anthropic/message_converter.rs` | **L49-62** | **`tool_call_id` 被挪用为图片 data URI 载体** | 🔴 语义误用：当 `tool_call_id` 以 `data:` 或 `http` 开头时，解析为图片插入 content_blocks |
| `anthropic/message_converter.rs` | L112 | `tool_call_id` 用作 `tool_use_id` | 同一字段双重语义，图片场景下 tool_use_id 实际是图片数据 |
| `openai/message_converter.rs` | L22/L47/L54 | `content: m.content` | 用户消息直接传 content，无图片解析 |
| `gemini/message_converter.rs` | **L47/L149-165** | **`try_parse_inline_image()` 已存在** | 🟡 可解析 content 中的内联图片为 `inlineData` part，但解析逻辑待确认是否与标记协议兼容 |
| `gemini/message_converter.rs` | L277-L412 | 测试用例中 `tool_call_id: None` | 测试覆盖了无图片场景 |

### 1.5 存储层

| 文件 | 行号 | 现状 | 语义 |
|------|------|------|------|
| `db.rs` | L55 | `tool_calls_json TEXT` | ✅ 列已存在，可存储 tool_calls JSON |
| `conversations.rs` | L36 | `tool_calls_json: Option<String>` | ✅ MessageRow 类型已定义 |
| `conversations.rs` | L6 | SELECT 包含 `tool_calls_json` | ✅ 查询已包含该列 |
| `conversations.rs` | L876/L1127 | INSERT 包含 `tool_calls_json` | ✅ 写入路径完整 |
| `conversations.rs` | L232-236 | UPDATE 支持 `tool_calls_json` | ✅ 更新路径完整 |
| `conversations.rs` | - | `content: String` | 类型上可承载 `[IMAGE:]` / `[FILE:]` 标记或占位符 |

---

## 2 参考项目做法深度分析

### 2.1 OpenHuman — 标记协议 + AttachmentStash（核心参考）

OpenHuman 的多模态方案是最完整的参考，其设计决策背后的原因值得深入理解：

#### 2.1.1 标记协议的设计动机

```
[IMAGE:data:image/png;base64,iVBORw0KGgo...]
[FILE:data:application/pdf;base64,JVBERi0x...]
```

**为什么选文本内嵌标记而非结构化字段？**

OpenHuman 的 `TranscriptMessage.content` 是 `String` 类型，且此签名贯穿：
- 跨线程 JSONL 序列化（`serde` 自动处理 String）
- 内存自动保存（N chunks 嵌入向量）
- Prompt 注入扫描（扫描文本而非二进制）
- 数据库持久化（SQLite TEXT 列）

改为 `content: Vec<ContentPart>` 需要全链路改造。标记协议将结构化信息编码到文本中，**保持 `content: String` 不变**，全链路改动最小。

#### 2.1.2 AttachmentStash 的设计动机

OpenHuman 实测发现 base64 直存 SQLite 的问题：

| 问题 | 影响 |
|------|------|
| SQLite 行膨胀 | 每张图 ~1.3× 原始 MB，`SELECT *` 时内存拷贝膨胀 |
| 内存自动保存 | N chunks 嵌入时，base64 文本被当作普通文本向量化，浪费 embedding 配额 |
| 跨线程 JSONL | 索引行膨胀，序列化/反序列化成本高 |
| Prompt 注入扫描 | 扫描器需跳过 MB 级 base64 块，增加复杂度 |

**Stash 策略**：Ingress 时将 `[IMAGE:data:...]` 替换为 `[Image: image #att:<sha256>]`（~50 字节），原始字节写磁盘。Dispatch 时仅 Vision 模型才从磁盘读回。

#### 2.1.3 关键实现细节

**`stash_image_attachments` 流程**（OpenHuman `multimodal.rs:450-530`）：

```
输入: "用户消息\n[IMAGE:data:image/png;base64,abc123]"

1. parse_image_markers() → (cleaned_text="用户消息", image_refs=["data:image/png;base64,abc123"])
2. 对每个 image_ref:
   a. 校验数量 ≤ max_images（默认 4）
   b. 校验大小 ≤ max_image_bytes（默认 8MB）
   c. id = sha256_prefix("data:image/png;base64,abc123")[..16] → "a1b2c3d4e5f6g7h8"
   d. stash().write("a1b2c3d4e5f6g7h8", "data:image/png;base64,abc123") → 写入磁盘
   e. 占位符 = "[Image: image #att:a1b2c3d4e5f6g7h8]"
3. 输出: "用户消息\n[Image: image #att:a1b2c3d4e5f6g7h8]"
```

**`rehydrate_image_placeholders` 流程**（OpenHuman `multimodal.rs:540-580`）：

```
输入: ["用户消息\n[Image: image #att:a1b2c3d4e5f6g7h8]", ...]

1. index = stash().build_index() → {"a1b2c3d4e5f6g7h8": "/path/to/attachments/a1b2c3d4e5f6g7h8"}
2. 对每条消息:
   a. 检测 content 中是否有 [Image: ... #att:<id>]
   b. 若有，从 index 查找路径 → 读文件 → 得到 data_uri
   c. 替换占位符为 [IMAGE:<data_uri>]
3. 输出: ["用户消息\n[IMAGE:data:image/png;base64,abc123]", ...]
```

**AttachmentStash 关键参数**（OpenHuman `multimodal.rs:610-620`）：

```rust
const ATTACHMENTS_MAX_BYTES: u64 = 256 * 1024 * 1024;  // 256MB 总量上限
const ATTACHMENTS_TTL: Duration = Duration::from_secs(24 * 60 * 60);  // 24h 过期
```

**原子写入**：先写 `<id>.tmp` 再 `rename` 到 `<id>`，避免并发读取半写文件。

**容量管理**：每次写入后检查总量，超出则按 mtime 驱逐最旧文件。

#### 2.1.4 前端 attachments.ts 核心类型

```typescript
// OpenHuman attachments.ts:70-85
export interface Attachment {
  id: string;
  kind: 'image' | 'file' | 'video';
  file: File;
  dataUri: string;
  previewUri?: string;          // 缩略图（图片用原始图，视频用第一帧）
  mimeType: AllowedAttachmentMimeType;
  originalSizeBytes: number;
  payloadSizeBytes: number;     // gzip 压缩后大小（若压缩）
  compressed: boolean;
  frames?: string[];            // 视频帧（仅 kind='video'）
}
```

**视频处理**：OpenHuman 支持视频附件，通过 `<video>` 元素 seek 到 0.1/0.5/0.9 位置采样帧，转为 JPEG data URI，再走 `[IMAGE:]` 路径。这是可选增强，当前项目可暂不考虑。

**gzip 压缩**：`buildAttachmentDataUri()` 尝试 `CompressionStream('gzip')` 压缩，压缩后更小才使用。MIME 编码为 `application/gzip;original_mime=<原MIME>;name=<文件名>`。

### 2.2 OpenBitFun — ImageContext 结构化对象

OpenBitFun 选择了不同的路径：**不用文本标记，而是结构化对象**。

```typescript
// OpenBitFun context.ts:135-145
export interface ImageContext extends BaseContext {
  type: 'image';
  imagePath: string;           // 本地文件路径引用
  imageName: string;
  width?: number;              // 图片尺寸（渲染优化）
  height?: number;
  fileSize: number;
  mimeType: string;
  dataUrl?: string;            // 内联数据（小图或剪贴板）
  thumbnailUrl?: string;       // 缩略图独立 URL
  source: 'file' | 'clipboard' | 'url';  // 来源区分
  isLocal: boolean;            // 本地 vs 上传区分
}
```

**与 OpenHuman 的对比**：

| 维度 | OpenHuman（标记协议） | OpenBitFun（结构化对象） |
|------|---------------------|------------------------|
| 存储格式 | `content: String` 含 `[IMAGE:]` 标记 | `contentParts: [{type:'image', imagePath:...}]` |
| 全链路改动 | 小（保持 String） | 大（需改 DTO/DB/序列化） |
| 类型安全 | 弱（正则解析） | 强（TypeScript interface） |
| 图片来源追踪 | 无（只有 dataUri） | 有（source 字段） |
| 缩略图 | previewUri 可选 | thumbnailUrl 独立 |
| 适用场景 | 快速迭代、存量系统 | 新系统、强类型偏好 |

**对本项目的启示**：当前项目 `content: String` 已贯穿全链路，OpenHuman 的标记协议更适配。但 OpenBitFun 的 `source`/`isLocal`/`width`/`height` 字段值得借鉴，可在 Attachment 类型中补充。

### 2.3 AIRI — image-projection 描述降级

AIRI 的 `image-projection.ts` 解决了一个特定问题：**非 Vision 模型收到图片时的降级策略**。

```typescript
// AIRI image-projection.ts:15-35
export async function describeChatImages(conversation, describe, ...) {
  // 用户消息中的图片 → 调用 Vision 模型生成文字描述
  // 替换为: "[Image description, supplied as user content]\n{description}\n[End image description]"
}

export function replaceToolResultImages(conversation, note) {
  // 工具结果中的图片 → 替换为文本注释
  // "A tool image was left out of the history."
}
```

**关键设计决策**：
- **持久化历史保留原始图片**：降级仅在 Provider prompt 中生效，不影响存储
- **运行时上下文分离**：Runtime context 不进入 vision prompt

**与本项目的关联**：OpenHuman 的 rehydrate 策略（仅 Vision 模型 rehydrate，非 Vision 保持占位符）已覆盖此场景。AIRI 的描述降级是更激进的方案（用文字替代图片），可作为未来增强方向。

---

## 3 影响评估与分类

### 3.1 ✅ 可复用（与附件无关，无需改动）

| 模块 | 文件 | 原因 |
|------|------|------|
| 流消费引擎 | `chat_runtime.rs` | `consume_stream` 只消费 `ChatDelta` 流，不关心内容是否含图片 |
| ToolCall 累积器 | `chat_runtime.rs:230-290` | `apply_delta`/`finish` 处理 tool call 流式片段，与消息内容无关 |
| 流事件分发 | `chatStreamEvents.ts` | 监听 Tauri 事件、去重、合并 delta，不关心内容格式 |
| 流合并器 | `streamCoalescer.ts` | delta 合并、序列去重，纯数据管道 |
| 邻接表存储 | `conversations.rs` | `parent_id` 树结构、`sibling_index` 重排、`delete_subtree` 全部内容无关 |
| DB schema | `db.rs` | `content TEXT` 无长度限制（SQLite TEXT 最大 1GB），`tool_calls_json TEXT` 已存在 |
| MessageTree | `messageTree.ts` | 只建 `parentId→children` 映射，不关心 content 格式 |
| BranchPicker | `BranchPicker.tsx` | 纯 UI 组件，只看 `siblingIndex/siblingCount` |
| ToolCallBlock | `ToolCallBlock.tsx` | 渲染 tool call 信息，与用户消息内容无关 |
| Mascot 动画 | `useMascotChatSync.ts` | 已识别 `role === "tool"` 和 `toolCalls`，可扩展识别图片 |

### 3.2 🔧 需修改（有文本假设硬编码，但架构可扩展）

| # | 模块 | 文件 | 现状 | 修改方向 | 严重度 |
|---|------|------|------|---------|--------|
| 1 | **Anthropic tool_call_id hack** | `anthropic/message_converter.rs:49-62` | `tool_call_id` 被挪用为图片 data URI 载体 | 移除 hack，改为正规 `[IMAGE:]` 标记解析 | 🔴 必须修复 |
| 2 | **prompt_builder 丢失 tool_calls** | `prompt_builder.rs:46` | `tool_calls: None` 硬编码 | 从 `MessageRow.tool_calls_json` 反序列化 | 🔴 bug 修复 |
| 3 | **MessageInput 无附件入口** | `MessageInput.tsx:1-60` | 纯文本 Input | 新增附件按钮 + 粘贴/拖拽 + 预览条 | 🔴 功能缺失 |
| 4 | **MessageContent 无图片渲染** | `MessageContent.tsx` | Markdown 渲染 | 解析 `[IMAGE:]` 标记 → `<img>` 渲染 | 🔴 功能缺失 |
| 5 | **OpenAI converter 无图片** | `openai/message_converter.rs:22` | `content: m.content` 直传 | 检测 `[IMAGE:]` → 生成 `image_url` content_part | 🔴 功能缺失 |
| 6 | **Gemini converter 部分支持** | `gemini/message_converter.rs:47` | `try_parse_inline_image()` 存在 | 确认兼容性，可能需扩展 | 🟡 需验证 |
| 7 | **模型配置无 Vision 能力** | `providerStore.ts` | 无 `supportVision` | 新增字段，控制附件入口显隐 | 🟡 需新增 |
| 8 | **chatTypes 缺附件字段** | `chatTypes.ts` | 无 `imageUris`/`fileReferences` | 新增前端渲染用字段（不持久化） | 🟡 需新增 |
| 9 | **useFileDrop 未接入** | `useFileDrop.ts` | 存在但未连接 MessageInput | 接入粘贴/拖拽图片流程 | 🟡 需集成 |
| 10 | **AgentKind 无 Vision 声明** | `prompt_builder.rs:15-30` | 4 种 kind | 扩展能力声明或独立配置 | 🟢 可选 |

### 3.3 🆕 需新增（当前不存在，附件功能必需）

| # | 模块 | 说明 | 参考实现 |
|---|------|------|---------|
| 1 | **attachmentTypes.ts** | Attachment 接口、MIME 白名单、常量定义 | OpenHuman `attachments.ts:1-90` |
| 2 | **attachmentUtils.ts** | `buildMessageWithAttachments` / `parseMessageImages` / `validateAndReadFile` | OpenHuman `attachments.ts:200-400` |
| 3 | **attachments.rs (Rust)** | AttachmentStash：磁盘读写、容量管理、TTL 清扫 | OpenHuman `multimodal.rs:600-650` |
| 4 | **multimodal.rs (Rust)** | `stash_image_attachments` / `rehydrate_image_placeholders` / `inline_file_attachments` | OpenHuman `multimodal.rs:400-600` |
| 5 | **Markdown 导出适配** | `conversations.rs` 导出时处理 `[Image: #att:]` 占位符 | OpenHuman 有角色格式化逻辑 |

---

## 4 可能的演进方向

> 以下为基于参考项目分析的几种可能路径，非唯一方案，亦非执行计划。各路径可独立采用或组合采用。

### 4.1 路径 A：标记协议 + 磁盘 Stash（参考 OpenHuman）

**核心思路**：保持 `content: String` 不变，用 `[IMAGE:]`/`[FILE:]` 标记编码附件，Ingress 时 stash 到磁盘，Dispatch 时按需 rehydrate。

**优势**：
- 全链路改动最小（`content` 类型不变）
- SQLite 行体积可控（占位符 ~50 字节 vs base64 ~1.3MB）
- 已有 OpenHuman 完整实现可参考
- Gemini converter 已有 `try_parse_inline_image` 部分

**劣势**：
- 正则解析标记脆弱（`[^\]]+` 在特殊字符时可能出错）
- `content` 字段承载多重语义（文本 + 图片 + 文件），可读性下降
- 前端渲染需每次解析标记，无法利用结构化索引

**适用场景**：快速上线、存量系统改造、团队资源有限时。

### 4.2 路径 B：结构化 ContentPart（参考 OpenBitFun）

**核心思路**：将 `content: String` 升级为 `content: Vec<ContentPart>`，其中 `ContentPart = Text(String) | Image(ImageContext) | File(FileReference)`。

**优势**：
- 类型安全，编译期保证
- 无正则解析，性能更好
- 图片元数据（width/height/source/isLocal）自然携带
- 前端渲染可直接模式匹配，无需解析

**劣势**：
- 全链路改造大：DTO、DB schema、序列化、Prompt 构建全部需改
- SQLite 存储 `content` 列需改为 JSON 或新增 `content_parts` 表
- 与现有 `content: String` 的所有消费者（导出、搜索、嵌入）都需适配
- 迁移成本高，需处理历史数据

**适用场景**：新系统、强类型偏好、长期维护时。

### 4.3 路径 C：混合方案（推荐可能性较高）

**核心思路**：**存储和传输层用标记协议（路径 A），前端内部用结构化对象（路径 B）**。

```
前端内部:
  Attachment[] (结构化) → buildMessageWithAttachments() → content: String (含标记)

传输层 (Tauri Command):
  content: String (含 [IMAGE:] 标记)

Rust Ingress:
  content (含标记) → stash_image_attachments() → content (含占位符) → SQLite

Rust Dispatch (仅 Vision):
  content (含占位符) → rehydrate_image_placeholders() → content (含 [IMAGE:] 标记) → Provider

Provider 转换:
  content (含 [IMAGE:] 标记) → 解析 → 结构化 content_parts (image_url / inline_data / source)

前端渲染:
  content (含 [IMAGE:] 标记 或 [Image: #att:] 占位符) → parseMessageImages() → <img> 渲染
```

**优势**：
- 存储层改动最小（保持 `content: String`）
- 前端内部类型安全（Attachment 结构化对象）
- Provider 转换层获得结构化数据（从标记解析后）
- 渐进迁移可行：Phase 1 先直存 base64 标记，Phase 2 再加 stash

**劣势**：
- 需要维护"标记协议 ↔ 结构化对象"的双向转换
- 标记解析仍存在于传输边界

### 4.4 路径 D：分阶段渐进交付（参考 spec 的 Phase 策略）

无论选择哪条路径，都可按以下阶段渐进：

**Phase 1：前端可用（base64 直存）**
- 最小可用版本，不依赖 Rust 后端改造
- `[IMAGE:data:...]` 标记直接存入 SQLite
- 验证前端输入/渲染/发送全链路打通
- 风险：SQLite 行膨胀（但早期用户量和图片量可控）

**Phase 2：后端 Stash 优化**
- 新增 `attachments.rs` + `multimodal.rs`
- Ingress 时 stash 到磁盘，存占位符
- Dispatch 时 rehydrate
- 修复 Anthropic tool_call_id hack
- 修复 prompt_builder tool_calls 丢失

**Phase 3：文件附件 + 安全加固**
- PDF/文本提取内联
- 不可信通道 `max_files: 0` 哨兵
- 路径遍历防护
- MIME 白名单严格校验

---

## 5 关键技术决策点分析

### 5.1 标记协议的正则健壮性

当前 spec 中的正则：
```typescript
content.replace(/\[IMAGE:([^\]]+)\]/g, ...)
```

**潜在问题**：
- `[^\]]+` 在 data URI 包含 `]` 时提前截断（base64 字符集不含 `]`，安全）
- `[FILE:]` 标记中文件名可能含 `]`（如 `file[1].pdf`），会截断

**OpenHuman 的做法**：`tinyagents_harness::multimodal::markers` 模块用 Rust 实现，同样用 `[^]]+` 但有额外的 base64 校验。

**改进方向**：
- 对 `[FILE:]` 使用更安全的分隔符：`[FILE:<id>|<filename>]`
- 或限制文件名字符集（仅允许字母数字、`.`、`-`、`_`、空格）

### 5.2 Phase 1→Phase 2 迁移兼容性

Phase 1 存储的消息：`content = "...\n[IMAGE:data:image/png;base64,abc123]"`
Phase 2 存储的消息：`content = "...\n[Image: image #att:a1b2c3d4...]"`

**迁移策略选项**：

| 策略 | 描述 | 优劣 |
|------|------|------|
| **惰性迁移** | Phase 2 的 `parseMessageImages` 同时识别两种格式，旧消息在下次编辑时重写 | ✅ 无停机迁移；🟡 两种格式并存期间逻辑复杂 |
| **批量迁移脚本** | 启动时检测并转换旧格式消息 | ✅ 一次性清理；🟡 需停机或锁库 |
| **不迁移** | 旧消息保持 base64，仅新消息用占位符 | ✅ 零成本；🟡 旧消息行膨胀持续存在 |

**推荐**：惰性迁移 + 后台批量清理任务（低优先级异步执行）。

### 5.3 `tool_call_id` Hack 的移除时机

当前 Anthropic converter 的 hack：
```rust
if let Some(ref image_url) = msg.tool_call_id {
    if image_url.starts_with("data:") || image_url.starts_with("http") {
        // 把 tool_call_id 当作图片 data URI
    }
}
```

**移除条件**：
- 标记协议上线后，图片通过 `content` 中的 `[IMAGE:]` 标记传递
- `tool_call_id` 恢复其原有语义（tool result 关联 ID）

**风险**：
- 如果有外部依赖方依赖此 hack 行为（unlikely，因为这是内部实现细节）
- 测试用例需同步更新（`message_converter.rs` 的测试）

**建议**：Phase 2 一并移除，与 stash/rehydrate 同时实施。

### 5.4 缩略图生成策略

**需求**：附件预览条中显示缩略图，避免大图 DOM 渲染卡顿。

**OpenBitFun 的实现**（`imageUtils.ts:30-80`）：
```typescript
async function generateThumbnail(file: File, maxSize: number = 200): Promise<string> {
  const img = await createImageBitmap(file);
  const canvas = document.createElement('canvas');
  const scale = Math.min(maxSize / img.width, maxSize / img.height, 1);
  canvas.width = img.width * scale;
  canvas.height = img.height * scale;
  const ctx = canvas.getContext('2d')!;
  ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
  return canvas.toDataURL('image/jpeg', 0.7);
}
```

**决策点**：
- 是否需要独立缩略图（vs 直接用原图缩小显示）？
- 缩略图是否持久化（vs 每次从原图生成）？
- maxSize 选多大（OpenBitFun 用 200px，OpenHuman 用原图）？

**建议**：Phase 1 不生成独立缩略图，CSS `max-width/max-height` 控制显示尺寸。Phase 2 若预览条性能有问题再引入。

---

## 6 风险与约束

### 6.1 向后兼容

| 维度 | 兼容性 | 说明 |
|------|--------|------|
| 存储层 | ✅ | `content TEXT` 无格式约束，标记/占位符/纯文本均可存 |
| 旧对话 | ✅ | 无标记的消息正常显示，`parseMessageImages` 返回空数组 |
| 导出 | 🟡 需适配 | Markdown/JSON 导出需处理 `[Image: #att:]` 占位符（替换为图片引用或移除） |
| API 签名 | ✅ | `send_chat_message(conv_id, content, ...)` 不变 |
| 前端状态 | ✅ | `sendMessage(content: string)` 不变 |

### 6.2 性能

| 操作 | 复杂度 | 说明 |
|------|--------|------|
| 标记解析 | O(n) | 正则扫描 content，n 为文本长度 |
| Stash 写入 | O(1) | SHA-256 计算 + 文件写入（原子操作） |
| Stash 读取 | O(1) | 按 ID 直接读文件 |
| Rehydrate | O(m×k) | m 条消息 × k 个占位符，每次读磁盘 |
| Index 构建 | O(d) | d 为 attachments 目录文件数，一次扫描 |
| TTL 清扫 | O(d) | 启动时遍历目录，检查 mtime |

**潜在瓶颈**：
- Rehydrate 时若消息历史很长且有大量图片，磁盘 I/O 可能累积
- **缓解**：仅对最近 N 条消息 rehydrate（context window 限制天然截断）

### 6.3 安全

| 风险 | 等级 | 缓解措施 |
|------|------|---------|
| 路径遍历 | 🔴 高 | `AttachmentStash.write()` 校验 ID 格式（仅允许 hex 字符），拒绝 `../` |
| MIME 欺骗 | 🟡 中 | 前端 `validateAndReadFile` 校验 MIME 白名单；后端二次校验 |
| base64 注入 | 🟢 低 | 标记协议限定 `data:` URI 格式，解析时验证前缀 |
| 磁盘空间耗尽 | 🟡 中 | `ATTACHMENTS_MAX_BYTES = 256MB` 上限 + mtime 驱逐 |
| 不可信通道文件注入 | 🔴 高 | `max_files: 0` 哨兵拒绝所有 `[FILE:]` 标记 |
| SVG XSS | 🟡 中 | MIME 白名单排除 `image/svg+xml`（或 sanitize 后允许） |

### 6.4 `as unknown as` 断言约束

根据项目规则，`as unknown as` 双重断言仅允许 2 处：
- `settings/core/types/confUtils.ts` `toConfRecord()`
- `i18n/index.ts` `translateKey()`

附件相关的类型转换应使用：
- 泛型约束（`<T extends AttachmentKind>`）
- Discriminated union（`{ type: 'image', ... } | { type: 'file', ... }`）
- 类型守卫函数（`isImageAttachment(a): a is ImageAttachment`）

---

## 7 与现有架构的对齐

### 7.1 AgentMode 注册机制

当前 `agentModeRegistry.ts` 的 `defineAgentKind` 是开放注册机制：

```typescript
defineAgentKind({ id: "default", label: "...", color: "blue" });
defineAgentKind({ id: "quick", label: "...", color: "green" });
defineAgentKind({ id: "deep", label: "...", color: "purple" });
defineAgentKind({ id: "custom", label: "...", color: "orange" });
```

**扩展方向**：若需按 agent kind 区分附件行为（如 Deep 模式允许更多图片），可扩展：

```typescript
defineAgentKind({
  id: "vision",
  label: "settings.agent_mode.kind.vision",
  color: "cyan",
  supportsVision: true,
  maxImages: 8,  // 比 default 的 4 更宽松
});
```

### 7.2 Rust AgentKind 扩展

当前 `prompt_builder.rs`：

```rust
pub enum AgentKind { Default, Quick, Deep, Custom }
```

**可能扩展**：

```rust
pub enum AgentKind {
    Default,
    Quick,
    Deep,
    Custom,
    // 未来可能:
    // Vision,      // 支持图片输入
    // Autonomous,  // 自主 agent（见 AGENT-MODE-EVOLUTION 分析）
}

impl AgentKind {
    pub fn supports_vision(&self) -> bool {
        matches!(self, AgentKind::Default | AgentKind::Deep | AgentKind::Custom)
        // 或从配置读取
    }

    pub fn max_images(&self) -> usize {
        match self {
            AgentKind::Quick => 2,  // 快速模式限制更严
            _ => 4,
        }
    }
}
```

### 7.3 与 I2-History 的关系

I2-History 已完成消息存储、分支、重发。I2e 多模态附件建立在 I2-History 之上：

| I2-History 提供的能力 | I2e 使用方式 |
|----------------------|-------------|
| `messages` 表 `content TEXT` 列 | 存储标记/占位符 |
| `parent_id` 树结构 | 附件跟随消息的分支/重发/删除 |
| `tool_calls_json` 列 | I2e 修复 prompt_builder 的丢失 bug |
| `send_chat_message` Tauri command | 签名不变，content 含标记 |
| 消息流式输出 | 流式输出不含附件（附件在 user 消息中） |

**无冲突**：I2e 不改变 I2-History 的任何现有行为，仅扩展。

### 7.4 与 AGENT-MODE-EVOLUTION 分析的关系

AGENT-MODE-EVOLUTION 分析了 tool 角色引入对架构的影响。I2e 多模态附件是另一个独立维度：

| 维度 | AGENT-MODE-EVOLUTION | I2e 多模态附件 |
|------|---------------------|---------------|
| 核心变化 | 消息角色扩展（user/assistant/tool） | 消息内容扩展（文本→文本+图片+文件） |
| 影响范围 | Orchestrator/渲染/分支/prompt_builder | Input/converter/renderer/storage |
| 交互点 | tool 消息可能携带图片结果（如截图工具） | 用户消息携带图片输入 |
| 耦合度 | 低（可独立实施） | 低（可独立实施） |

**未来交汇点**：若 tool 角色引入后，工具返回的结果可能包含图片（如浏览器截图工具）。此时 I2e 的 `[IMAGE:]` 标记协议可复用于 tool result 中的图片，无需额外设计。

---

## 8 开放问题

以下问题在实施前需进一步明确，本分析不预设答案：

1. **视频支持**：OpenHuman 支持视频→帧采样→`[IMAGE:]` 标记，当前项目是否需要？优先级？
2. **远程图片**：OpenHuman `allow_remote_fetch` 配置允许 fetch URL 图片，当前项目是否需要？（涉及网络请求、CORS、缓存）
3. **图片编辑**：用户发送前是否允许裁剪/旋转/标注？还是仅支持原样发送？
4. **多模型对话中的图片传递**：用户发图片给 Vision 模型后，切换到非 Vision 模型，历史消息中的图片如何展示？（占位符？缩略图？"此消息包含图片"提示？）
5. **附件的生命周期管理**：用户是否可手动删除已发送的附件？删除后磁盘文件是否同步清理？
6. **跨设备同步**：若未来支持云同步，磁盘附件如何处理？（上传到云端？仅同步占位符？）
7. **PDF 提取的 crate 选择**：`pdf-extract`（纯 Rust，轻量） vs `lopd`（功能全，重量级） vs 调用系统工具（poppler）？
8. **`try_parse_inline_image` 的兼容性**：Gemini converter 已有的内联图片解析是否与 `[IMAGE:data:...]` 标记格式兼容？需验证。
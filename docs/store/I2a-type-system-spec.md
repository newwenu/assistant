# I2a — 类型系统增强规格说明

> **来源**：从 I2-History §2.1 / §2.2 / §2.3 暂缓内容提取
> **前置**：I2-History ✅ 已完成

## 1. 目标

为聊天消息系统引入编译期类型安全增强：
- **Branded Type ID**：消除 `string` ID 混淆风险，`MessageId` / `ConversationId` / `ToolCallId` / `ProviderId` 编译期不可互换
- **ContentBlock 多模态内容**：消息 `content` 从纯 `string` 升级为结构化 `ContentBlock[]`，支持文本/图片/工具调用/思考内嵌
- **MessageSource 来源追踪**：每条消息携带来源信息，支持 Provider 切换后重放和 UI 来源标识

---

## 2. Branded Type ID

### 2.1 问题

当前 `Message.id` / `Conversation.id` / `ToolCall.id` 均为 `string`，编译期无法区分，易混淆传参：

```typescript
// 以下代码编译通过，运行时错误
retryFromError(conversationId)  // 传了 conversationId，string 兼容
```

### 2.2 设计

```typescript
type MessageId = string & { readonly __brand: 'MessageId' }
type ConversationId = string & { readonly __brand: 'ConversationId' }
type ToolCallId = string & { readonly __brand: 'ToolCallId' }
type ProviderId = string & { readonly __brand: 'ProviderId' }

function asMessageId(id: string): MessageId { return id as MessageId }
function asConversationId(id: string): ConversationId { return id as ConversationId }
function asToolCallId(id: string): ToolCallId { return id as ToolCallId }
function asProviderId(id: string): ProviderId { return id as ProviderId }
```

**Rust 端**：无需 branded type（Rust 的 newtype 模式更重，且 Tauri command 序列化边界已做校验），仅前端 TypeScript 使用。

### 2.3 影响范围

- `chatTypes.ts`：所有 ID 字段改为 branded type
- `chatStore.ts`：所有接受 ID 参数的方法签名更新
- `chatService.ts`：所有 invoke 调用的 ID 参数
- `MessageBubble` / `MessageList` / `ChatView` 等组件的 props 类型
- Tauri event payload 中的 ID 字段（反序列化后需 `asXxxId()` 转换）

### 2.4 暂缓理由

当前项目规模小，string ID 混淆风险极低；TypeScript branded type 是编译期幻影类型，运行时零防护，实际收益有限。全量签名改动面大。

---

## 3. ContentBlock 多模态内容

### 3.1 问题

当前 `Message.content` 为纯 `string`，`MessageBubble` 直接用 `ReactMarkdown` 渲染。无法支持：
- 图片消息
- 工具调用/工具结果内嵌
- 思考内容内嵌

### 3.2 设计

```typescript
type ContentBlock =
  | { type: 'text'; text: string }
  | { type: 'image'; url: string; alt?: string; mime_type?: string }
  | { type: 'tool_call'; id: ToolCallId; name: string; arguments: string }
  | { type: 'tool_result'; id: ToolCallId; content: string; is_error?: boolean }
  | { type: 'thinking'; text: string }

interface Message {
  id: MessageId
  conversation_id: ConversationId
  role: 'system' | 'user' | 'assistant' | 'tool'
  content: ContentBlock[]
  source?: MessageSource
  tokens: number | null
  timestamp: number
  error?: ChatError | null
  parent_id?: MessageId | null
  children_ids?: MessageId[]
  feedback?: 'positive' | 'negative' | null
  is_pinned?: boolean
}
```

### 3.3 向后兼容

Rust 端存储仍为 `content TEXT`（JSON 序列化 `ContentBlock[]`），读取时若为纯字符串则自动包装为 `[{ type: 'text', text: content }]`。

### 3.4 影响范围

- Rust 端序列化/反序列化改造
- 前端所有读取 `message.content` 的地方重写
- `MessageBubble` 渲染逻辑重构
- `sendMessage` 构造消息逻辑
- 向后兼容逻辑

### 3.5 暂缓理由

改为 `ContentBlock[]` 需要 Rust 端序列化/反序列化改造 + 前端所有读取 `message.content` 的地方重写 + 向后兼容逻辑，是巨大重构。且当前 `content:string` + `toolCalls?: ToolCall[]` + `reasoning?: string` 的现有结构已够用。总架构声明 Function Calling 暂不实现（ARCHITECTURE.md I6），`tool_call`/`tool_result` ContentBlock 类型无消费方。等真正需要图片消息或 Agent Loop 时再改。

---

## 4. MessageSource 来源追踪

### 4.1 问题

当前 `Message` 接口里没有 `source` 字段，无法知道消息来自哪个 Provider/模型，不支持 Provider 切换后重放。

### 4.2 设计

```typescript
type MessageSource =
  | { kind: 'user' }
  | { kind: 'model'; provider: string; model: string; replay_state?: unknown }
  | { kind: 'tool'; call_id: ToolCallId }
  | { kind: 'system-prompt' }
```

- `{ kind: 'model' }` 的 `replay_state` 支持 Provider 切换后重放：切换 Provider 后，标记所有 `source.kind === 'model'` 的消息为"需要重放"，用户可选择从某条消息开始重放
- `{ kind: 'tool' }` 依赖工具执行（总架构声明 Function Calling 暂不实现）
- UI 来源标识：助手消息显示 Provider 图标 + 模型名称

### 4.3 影响范围

- `chatTypes.ts`：`Message` 新增 `source?: MessageSource`
- `chatStore.ts`：`sendMessage` 中构造 assistant 消息时设置 `source`
- Rust 端：`MessageRow` 新增 `source TEXT`（JSON 序列化），`dto.rs` 同步
- `MessageBubble`：显示 Provider/模型来源标识

### 4.4 暂缓理由

当前 `Message` 接口里完全没有 `source` 字段，也没有任何代码读取或使用来源信息。`{ kind: 'tool' }` 依赖工具执行（总架构声明 Function Calling 暂不实现），`{ kind: 'model' }` 的 `replay_state` 依赖 Provider 切换后重放功能（尚未实现）。加 `source` 字段增加 `sendMessage` 复杂度但无处消费，等有重放需求时再加。

---

## 5. 实施步骤

### Step 1：Branded Type ID

1. 新增 `src/types/branded.ts`：定义 4 个 branded type + 转换函数
2. `chatTypes.ts`：所有 ID 字段改为 branded type
3. `chatStore.ts`：方法签名更新
4. `chatService.ts`：invoke 调用适配
5. 组件 props 类型更新
6. Tauri event 监听中 ID 转换

**验证**：`retryFromError(conversationId)` 编译报错；`retryFromError(messageId)` 编译通过。

### Step 2：ContentBlock 多模态内容

1. 新增 `src/types/content.ts`：`ContentBlock` 联合类型
2. `chatTypes.ts`：`Message.content` 类型从 `string` 改为 `ContentBlock[]`
3. Rust 端：`MessageRow.content` 存储改为 JSON 序列化，读取时向后兼容（纯字符串 → `[{ type: 'text', text: content }]`）
4. `MessageBubble`：渲染逻辑重构（按 `ContentBlock.type` 分发渲染）
5. `sendMessage`：构造消息时包装为 `[{ type: 'text', text: content }]`

**验证**：旧数据（纯字符串 content）自动包装为 ContentBlock，渲染不变。

### Step 3：MessageSource 来源追踪

1. 新增 `src/types/message.ts`：`MessageSource` 联合类型
2. `chatTypes.ts`：`Message` 新增 `source?: MessageSource`
3. Rust 端：`MessageRow` 新增 `source TEXT`，`dto.rs` 同步
4. `chatStore.ts`：`sendMessage` / `finishStream` 中设置 `source`
5. `MessageBubble`：显示来源标识

**验证**：助手消息显示 Provider + 模型名称。

---

## 6. 依赖关系

```
Step 1 (Branded Type) ← 独立，可先行
Step 2 (ContentBlock) ← 可独立，但建议在 Step 1 之后（ID 类型先稳定）
Step 3 (MessageSource) ← 依赖 Step 1（ToolCallId），部分依赖 Step 2（tool_call/tool_result ContentBlock）
```

---

## 7. 验收标准

- [ ] **Branded Type**：`MessageId` / `ConversationId` 类型不匹配时编译报错
- [ ] **ContentBlock 向后兼容**：旧数据（纯字符串 content）自动包装，渲染不变
- [ ] **ContentBlock 图片**：图片 ContentBlock 正确渲染
- [ ] **MessageSource**：助手消息显示 Provider + 模型来源标识
- [ ] **MessageSource 重放**：切换 Provider 后，标记需要重放的消息（如已实现重放功能）
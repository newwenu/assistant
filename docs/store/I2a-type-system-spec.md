# I2a — 类型系统增强规格说明

> **来源**：从 I2-History §2.1 / §2.2 / §2.3 提取
> **前置**：I2-History ✅ 已完成

## 1. 目标

为聊天消息系统引入编译期类型安全增强：
- **Branded Type ID**：消除 `string` ID 混淆风险，`MessageId` / `ConversationId` / `ToolCallId` / `ProviderId` 编译期不可互换
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

---

## 3. MessageSource 来源追踪

### 3.1 问题

当前 `Message` 接口里没有 `source` 字段，无法知道消息来自哪个 Provider/模型，不支持 Provider 切换后重放。

### 3.2 设计

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

### 3.3 影响范围

- `chatTypes.ts`：`Message` 新增 `source?: MessageSource`
- `chatStore.ts`：`sendMessage` 中构造 assistant 消息时设置 `source`
- Rust 端：`MessageRow` 新增 `source TEXT`（JSON 序列化），`dto.rs` 同步
- `MessageBubble`：显示 Provider/模型来源标识

---

## 4. 实施步骤

### Step 1：Branded Type ID

1. 新增 `src/types/branded.ts`：定义 4 个 branded type + 转换函数
2. `chatTypes.ts`：所有 ID 字段改为 branded type
3. `chatStore.ts`：方法签名更新
4. `chatService.ts`：invoke 调用适配
5. 组件 props 类型更新
6. Tauri event 监听中 ID 转换

**验证**：`retryFromError(conversationId)` 编译报错；`retryFromError(messageId)` 编译通过。

### Step 2：MessageSource 来源追踪

1. 新增 `src/types/message.ts`：`MessageSource` 联合类型
2. `chatTypes.ts`：`Message` 新增 `source?: MessageSource`
3. Rust 端：`MessageRow` 新增 `source TEXT`，`dto.rs` 同步
4. `chatStore.ts`：`sendMessage` / `finishStream` 中设置 `source`
5. `MessageBubble`：显示来源标识

**验证**：助手消息显示 Provider + 模型名称。

---

## 5. 依赖关系

```
Step 1 (Branded Type) ← 独立，可先行
Step 2 (MessageSource) ← 依赖 Step 1（ToolCallId）
```

---

## 6. 验收标准

- [ ] **Branded Type**：`MessageId` / `ConversationId` 类型不匹配时编译报错
- [ ] **MessageSource**：助手消息显示 Provider + 模型来源标识
- [ ] **MessageSource 重放**：切换 Provider 后，标记需要重放的消息（如已实现重放功能）
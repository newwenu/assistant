# I2a — 消息来源追踪规格说明

> **来源**：从 I2-History §2.3 提取
> **前置**：I2-History ✅ 已完成（已封存）
> **状态**：✅ 已完成

## 1. 目标

assistant 消息携带 `providerId` / `modelId`，解决中途切模型后历史消息无法追溯来源的问题。

---

## 2. 问题

当前 `providerId` / `modelId` 仅存在于 `Conversation` 级别。用户中途切换模型后，历史 assistant 消息无法追溯实际由哪个 Provider/模型生成。

---

## 3. 设计

在 `Message` 上直接新增两个可选字段，与 `Conversation` 字段命名一致：

```typescript
// chatTypes.ts — Message 新增
providerId?: string | null;
modelId?: string;
```

```rust
// MessageRow 新增
pub provider_id: Option<String>,
pub model_id: Option<String>,
```

- 仅 assistant 消息需要写入；user / system / compact_summary 消息留空
- 写入时机：Rust 端创建 assistant 消息时，从所属 Conversation 读取 `provider_id` / `model_id` 写入
- UI 来源标识：⏸️暂缓（当前不显示悬浮窗，数据先落库备用）

---

## 4. 实施步骤

1. `chatTypes.ts`：`Message` 新增 `providerId?` / `modelId?`
2. Rust 端：`MessageRow` 新增 `provider_id` / `model_id` 列，`dto.rs` 同步
3. Rust 端：创建 assistant 消息时从 Conversation 写入 `provider_id` / `model_id`
4. `read_msg_row` 改为按列名取值（`row.get("provider_id")?`），避免下标后移风险
5. `db.rs`：CREATE TABLE 新增列 + `pragma_table_info` 迁移守卫（ADD COLUMN）
6. `create_branch`：assistant 分支也写入 `provider_id` / `model_id`
7. `duplicate_conversation` / `fork_to_new_conversation`：复制时保留原消息的 `provider_id` / `model_id`
8. 测试文件：所有 `MessageRow` 构造补全新字段

---

## 5. 验收标准

- [x] **数据落库**：assistant 消息的 `provider_id` / `model_id` 正确写入 SQLite
- [x] **协议传输**：`MessageDto` 序列化包含 `providerId` / `modelId`（camelCase）
- [x] **前端接收**：`Message.providerId` / `Message.modelId` 可读取

---

## 6. 变更清单

| 文件 | 变更 |
|------|------|
| `src/chat/core/chatTypes.ts` | `Message` 新增 `providerId?` / `modelId?` |
| `src-tauri/crates/assistant-storage/src/conversations.rs` | `MessageRow` 新增字段、`MSG_SELECT` 扩展、`read_msg_row` 改列名取值、`insert_message` / `create_branch` / `duplicate_conversation` / `fork_to_new_conversation` 同步 |
| `src-tauri/crates/assistant-storage/src/db.rs` | CREATE TABLE 新增列 + 迁移守卫 |
| `src-tauri/src/protocol/dto/chat.rs` | `MessageDto` 新增字段 + `From` impl |
| `src-tauri/src/cmd/chat.rs` | 创建 assistant 消息时写入 `provider_id` / `model_id` |
| `src-tauri/src/cmd/compaction.rs` | compact_summary 消息留空 |
| `src-tauri/tests/protocol_contract.rs` | `sample_message_row` / `sample_error_payload` / `sample_retry_notification` 补全字段 |
| `src-tauri/crates/assistant-storage/tests/conversations.rs` | 所有 `MessageRow` 构造补全新字段 |
| `src-tauri/crates/assistant-agent/tests/chat_runtime_spec.rs` | `make_message_row` 补全新字段 |
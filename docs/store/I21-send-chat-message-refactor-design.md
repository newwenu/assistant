# I21 — `send_chat_message` 上帝函数重构设计

> 生成日期: 2026-10-26
> 范围: `src-tauri/src/cmd/chat.rs` — `send_chat_message` 函数（~430 行）
> 关联: [REFACTOR-SPLIT-PLAN.md](./REFACTOR-SPLIT-PLAN.md)、[I20-command-simplification-spec.md](./I20-command-simplification-spec.md)
> 状态: **已完成**

---

## 1. 问题陈述

### 1.1 现状

`send_chat_message` 从第 28 行到第 460 行，约 **430 行**，承担 7 个不同层级的职责：

| # | 职责 | 行范围(约) | 行数 | 可独立测试 |
|---|------|-----------|------|-----------|
| 1 | 输入校验 | 30-35 | ~6 | ✅ |
| 2 | 附件处理 | 37-40 | ~4 | ✅ |
| 3 | **DB 写入 — 消息树插入** | 42-280 | **~240** | ❌ |
| 4 | Prompt 构建 + 多模态 | 280-315 | ~35 | ❌ |
| 5 | Provider 配置 + 模型选项 | 315-340 | ~25 | ❌ |
| 6 | 事件发射 + 任务管理 | 340-390 | ~50 | ❌ |
| 7 | Stream 生命周期编排 | 390-460 | ~70 | ❌ |

### 1.2 核心问题

**第 3 块（消息插入）最严重**，约 240 行，内含 3 条分支路径且大量重复代码：

```
branch_point_message_id
├── Some → branch_point.role == "user"
│   ├── insert user message + attachment refs
│   └── insert assistant message
├── Some → branch_point.role == "assistant"
│   └── insert assistant message only
└── None (normal append)
    ├── insert user message + attachment refs
    └── insert assistant message
```

- `insert_message` + `extract_attachment_ids` + `insert_attachment_refs` 在两条分支中几乎相同
- 全部内联在一个 `let (...) = { ... }` 块中，与 DB 连接生命周期耦合
- 无法独立单元测试

### 1.3 影响范围

- 修改消息字段（如新增 `metadata` 列）需在 3 处内联位置同步修改
- 修改分支逻辑（如新增 "system" 分支点）需在 240 行块中定位
- 阅读认知负荷极高：需同时理解消息树结构、DB 操作、provider 配置、stream 生命周期

---

## 2. 参考项目组织方式

### 2.1 Codex (OpenAI) — Channel + Op 枚举 + Trait 多态

| 层级 | 职责 | 函数体量 |
|------|------|---------|
| `Thread::submit()` | 投递 `Op::TurnInput` 到 channel | ~30 行 |
| `Session::submit_turn_input()` | 生成 submission ID，通过 channel 发送 | ~30 行 |
| `Turn::run_turn()` | 驱动采样循环 | ~200 行（职责单一） |
| `AgentControl` trait | `resolve()` / `spawn()` / `send()` | 每个方法 ~30-60 行 |
| `delivery.rs` | 消息格式转换 | ~45 行 |
| `execution.rs` | 并发容量控制 | ~92 行 |

**核心模式**：`submit()` 不直接执行逻辑，而是将 `Op::TurnInput` 发送到 channel，由 session loop 异步消费。`AgentControl` trait 将 spawn/send/resolve/interrupt 定义为接口方法。

### 2.2 OpenBitFun — Module 分片 + Driver 多态

| 层级 | 职责 |
|------|------|
| `useMessageSender` hook | 组装消息（附件、context、image），委托给 Manager |
| `FlowChatManager` | 单例协调器，委托给 Module |
| `MessageModule` | 编排逻辑：busy-gate → pending queue → conflict retry → surface recovery |
| `SessionDriver` | `driver.startTurn()` / `driver.steer()` — 按会话类型多态分派 |

**核心模式**：`flow-chat-manager/` 目录下按职责拆分为 10+ 个 Module，`sendMessage()` 通过 `driver.startTurn()` 委托实际传输。

### 2.3 OpenHuman — Pipeline 分层 + ops/ 拆分

| 层级 | 职责 | 函数体量 |
|------|------|---------|
| `start_chat()` | 校验 + 附件 + 安全扫描 + approval routing + queue dispatch + spawn turn | ~600 行（管道式） |
| `run_chat_task()` | session checkout + agent 构建 + progress bridge | ~100 行 |
| `deliver_response()` | 持久化 + 事件发射 | ~100 行 |
| `turn_guards.rs` | 超时 + 取消包装 | — |

**核心模式**：`start_chat()` 虽有 ~600 行，但每个阶段顺序执行且互不耦合（管道式），`run_chat_task()` 和 `deliver_response()` 完全独立可测试。

### 2.4 模式对比与选型

| 维度 | Codex | OpenBitFun | OpenHuman | **本项目** |
|------|-------|------------|-----------|-----------|
| 架构风格 | Channel + Op + Trait | Module + Driver | Pipeline + ops/ | **Pipeline**（最兼容） |
| 消息插入 | Session channel 异步 | driver 多态 | `run_chat_task` 委托 | **独立函数** |
| Stream 编排 | `run_turn` 循环 | driver + event handler | progress_bridge | **已有 chat_stream.rs** |
| 并发控制 | CancellationToken + Op channel | inFlightSubmissions | IN_FLIGHT + RunQueue | ChatTasks（已有） |
| 可测试性 | ✅ 各层独立 | ✅ Module 各有 .test.ts | ✅ ops/ 各有 _tests.rs | **重构目标** |

**选型决策**：采用 **OpenHuman 的 Pipeline 模式**（与现有架构最兼容），同时借鉴 Codex 的职责分离思想。不引入 Channel/Op 枚举（过度设计），不引入 Driver 多态（当前无多会话类型需求）。

---

## 3. 重构方案

### 3.1 目标架构

```
send_chat_message()                    ← 管道入口，~25 行
├── validate_and_preprocess()          ← 校验 + 附件处理
├── insert_chat_messages()             ← 消息树插入（独立函数，可测试）
│   ├── insert_user_message_with_attachments()  ← 消除重复
│   └── insert_placeholder_assistant_message()  ← 消除重复
├── build_chat_request()               ← prompt + multimodal + provider + options
└── spawn_chat_stream()                ← 委托给 chat_stream 模块
    ├── run_chat_task()                ← stream 消费 + 结果收集
    └── deliver_response()             ← DB 写入 + 事件发射（已有 chat_stream.rs）
```

### 3.2 文件组织

```
cmd/
├── chat.rs              ← send_chat_message + cancel_chat（~80 行）
├── chat_insert.rs       ← insert_chat_messages + 辅助函数（~180 行）  [新增]
├── chat_request.rs      ← build_chat_request（~60 行）               [新增]
├── chat_stream.rs       ← spawn/run/listener（已有，不变）
└── chat_utils.rs        ← resolve_context_window / resolve_final_tokens（已有，不变）
```

### 3.3 各函数签名与职责

#### 3.3.1 `validate_and_preprocess` — 校验 + 附件

| 属性 | 值 |
|------|-----|
| 所在文件 | `chat.rs` |
| 可见性 | `async fn`（模块私有） |
| 参数 | `content: &str`, `conversation_id: &str`, `attachment_store: &AttachmentStore` |
| 返回 | `Result<String, String>` — 处理后的 content |
| 错误 | 空内容 / compaction 终止中 / 附件处理失败 |
| 提取自 | chat.rs L30-40（`is_content_empty` + `is_terminating` + `admit_attachments`） |
| 行数 | ~10 行 |

---

#### 3.3.2 `insert_chat_messages` — 消息树插入

| 属性 | 值 |
|------|-----|
| 所在文件 | `chat_insert.rs` |
| 可见性 | `pub(crate) fn` |
| 参数 | `conn: &Connection`, `conversation_id: &str`, `content: &str`, `mode_id: Option<&str>`, `branch_point_message_id: Option<&str>`, `provider_id: &str`, `model_id: &str` |
| 返回 | `Result<InsertedMessages, String>` |
| 副作用 | DB 写入（insert_message + insert_attachment_refs + update_active_branch_id + update_conversation） |
| 提取自 | chat.rs L42-280（整个 `let (...) = { ... }` 块） |
| 行数 | ~120 行 |

**`InsertedMessages` struct**：

| 字段 | 类型 | 说明 |
|------|------|------|
| `mode` | `Option<AgentModeRow>` | 解析后的 agent mode |
| `assistant_msg_id` | `String` | 新插入的 assistant 占位消息 ID |
| `user_msg_id` | `Option<String>` | 新插入的 user 消息 ID（分支点为 assistant 时为 None） |
| `detail` | `ConversationDetail` | 插入后刷新的会话详情 |
| `provider_row` | `ProviderRow` | 验证存在性的 provider 行 |

**三条路径**：

| `branch_point_message_id` | `branch_point.role` | 操作 |
|---------------------------|---------------------|------|
| `Some(bp_id)` | `"user"` | 在 bp_parent 下插入 user + assistant |
| `Some(bp_id)` | `"assistant"` | 在 bp_parent 下仅插入 assistant |
| `None` | — | 在 last_message 下插入 user + assistant |

---

#### 3.3.3 `insert_user_message_with_attachments` — 消除重复

| 属性 | 值 |
|------|-----|
| 所在文件 | `chat_insert.rs` |
| 可见性 | `fn`（模块私有） |
| 参数 | `conn: &Connection`, `conversation_id: &str`, `content: &str`, `parent_id: Option<&str>`, `sibling_index: i64`, `now: i64` |
| 返回 | `Result<String, String>` — user message ID |
| 合并的重复逻辑 | `conversations::insert_message`(user role) + `extract_attachment_ids` + `attachment_store::insert_attachment_refs` |
| 行数 | ~25 行 |

---

#### 3.3.4 `insert_placeholder_assistant_message` — 消除重复

| 属性 | 值 |
|------|-----|
| 所在文件 | `chat_insert.rs` |
| 可见性 | `fn`（模块私有） |
| 参数 | `conn: &Connection`, `conversation_id: &str`, `parent_id: Option<&str>`, `sibling_index: i64`, `now: i64`, `provider_id: &str`, `model_id: &str` |
| 返回 | `Result<String, String>` — assistant message ID |
| 合并的重复逻辑 | `conversations::insert_message`(assistant role, empty content) — 三条路径均调用 |
| 行数 | ~20 行 |

---

#### 3.3.5 `build_chat_request` — Prompt + 多模态 + Provider + Options

| 属性 | 值 |
|------|-----|
| 所在文件 | `chat_request.rs` |
| 可见性 | `pub(crate) async fn` |
| 参数 | `detail: &ConversationDetail`, `mode: Option<&AgentModeRow>`, `provider_row: &ProviderRow`, `model_id: &str`, `attachment_store: &AttachmentStore` |
| 返回 | `Result<ChatRequest, String>` |
| 提取自 | chat.rs L280-340 |
| 行数 | ~60 行 |

**`ChatRequest` struct**：

| 字段 | 类型 | 说明 |
|------|------|------|
| `chat_messages` | `Vec<ChatMessage>` | prepare_messages + multimodal 处理后的消息 |
| `adapter` | `Box<dyn ProviderAdapter>` | create_provider_adapter 构造 |
| `model` | `String` | 模型 ID |
| `options` | `ChatOptions` | build_chat_options 构造 |
| `context_window_tokens` | `u32` | resolve_context_window 结果 |
| `kind` | `AgentKind` | from_mode_kind 解析 |

---

#### 3.3.6 `spawn_chat_stream` — Stream 生命周期编排

| 属性 | 值 |
|------|-----|
| 所在文件 | `chat.rs` |
| 可见性 | `fn`（模块私有） |
| 参数 | `app: &AppHandle`, `conversation_id: &str`, `assistant_msg_id: &str`, `user_msg_id: Option<&str>`, `content: &str`, `request: ChatRequest`, `detail: ConversationDetail`, `provider_row: ProviderRow` |
| 返回 | `Result<(), String>` |
| 提取自 | chat.rs L340-460 |
| 行数 | ~150 行 |

**内部流程**：

1. `ChatTasks::start` → 获取 cancel_token + attempt_id
2. emit `CHAT_USER_MESSAGE_CONFIRMED` + `CHAT_STREAM_START`
3. 加载 auto_compact 配置
4. `spawn_db_writer` + `spawn_finish_listener` + `spawn_compaction_listener`
5. 构建 `stream_future` 闭包：
   - `adapter.chat_completion_stream` → stream
   - `chat_runtime::consume_stream` → outcome
   - Ok → `db_tx.send` + compaction check + `finish_tx.send`
   - Err(Cancelled/Provider/Timeout) → `save_partial_on_error` + `emit_stream_error` + `finish_tx.send`
6. `tauri::async_runtime::spawn(Box::pin(stream_future))`

---

### 3.4 重构后 `send_chat_message` 结构

```
send_chat_message:
  1. validate_and_preprocess(content, conversation_id, attachment_store) → content
  2. { get_db_conn → insert_chat_messages(conn, ...) } → InsertedMessages
  3. build_chat_request(detail, mode, provider_row, model_id, attachment_store) → ChatRequest
  4. spawn_chat_stream(app, conversation_id, assistant_msg_id, user_msg_id, content, request, detail, provider_row)
```

**行数**：~25 行（纯编排，零业务逻辑）

---

## 4. 行数对比

| 文件 | 重构前 | 重构后 | 变化 |
|------|--------|--------|------|
| `chat.rs` | 479 行 | ~200 行 | **-279 行** |
| `chat_insert.rs` | (不存在) | ~180 行 | **新增** |
| `chat_request.rs` | (不存在) | ~60 行 | **新增** |
| `chat_stream.rs` | 227 行 | 227 行 | 不变 |
| `chat_utils.rs` | 47 行 | 47 行 | 不变 |
| **合计** | **753 行** | **714 行** | **-39 行** |

**`send_chat_message` 本体**：430 行 → **25 行**（-94%）

---

## 5. 可测试性提升

| 函数 | 重构前 | 重构后 |
|------|--------|--------|
| 消息树插入 | ❌ 无法独立测试 | ✅ `insert_chat_messages` 可 mock Connection |
| user 消息 + 附件插入 | ❌ 内联重复 | ✅ `insert_user_message_with_attachments` 独立测试 |
| assistant 占位消息插入 | ❌ 内联重复 | ✅ `insert_placeholder_assistant_message` 独立测试 |
| prompt + 多模态 + provider | ❌ 内联 | ✅ `build_chat_request` 独立测试 |
| 附件 ID 提取 | ❌ 私有 fn | ✅ `extract_attachment_ids` 独立测试 |
| stream 编排 | ❌ 闭包 | ✅ `spawn_chat_stream` 独立测试 |

---

## 6. 重复代码消除

重构前，user message 插入 + attachment ref 插入在两条分支中重复（~30 行 × 2 = ~60 行）。

重构后，提取为 `insert_user_message_with_attachments`（~25 行），两条分支各调用 1 行，**消除 ~35 行重复**。

assistant message 插入在三条路径中重复（~20 行 × 3 = ~60 行）。

重构后，提取为 `insert_placeholder_assistant_message`（~20 行），三条路径各调用 1 行，**消除 ~40 行重复**。

**总计消除 ~75 行重复代码**。

---

## 7. 实施步骤

### Step 1: 创建 `chat_insert.rs`

- 提取 `insert_chat_messages` + `InsertedMessages` struct
- 提取 `insert_user_message_with_attachments`
- 提取 `insert_placeholder_assistant_message`
- 提取 `update_conversation_provider_model`
- 提取 `insert_on_branch` + `insert_normal_append`
- 移入 `extract_attachment_ids` + `is_content_empty`
- 在 `cmd/mod.rs` 中 `mod chat_insert;`

### Step 2: 创建 `chat_request.rs`

- 提取 `build_chat_request` + `ChatRequest` struct
- 提取 `resolve_support_vision`
- 在 `cmd/mod.rs` 中 `mod chat_request;`

### Step 3: 重构 `chat.rs`

- 替换 `send_chat_message` 为管道式编排（~25 行）
- 提取 `validate_and_preprocess`
- 提取 `spawn_chat_stream`
- 删除已迁移的代码
- 保留 `cancel_chat`

### Step 4: 编译 + 测试

- `cargo check` 确保编译通过
- 运行现有测试
- 为 `chat_insert` / `chat_request` 补充单元测试

### Step 5: 更新 `REFACTOR-SPLIT-PLAN.md`

- 标记 `chat.rs` 重构完成
- 更新行数统计

---

## 8. 风险与缓解

| 风险 | 缓解 |
|------|------|
| DB 连接生命周期 | `insert_chat_messages` 仍接收 `&Connection`，与原代码生命周期一致 |
| `InsertedMessages` struct 携带 `detail` + `provider_row` | 仅在 `send_chat_message` 内使用，不泄漏到外部 |
| `ChatRequest` struct 携带 `adapter: Box<dyn ProviderAdapter>` | `adapter` 仍为局部构造，不改变所有权模型 |
| `spawn_chat_stream` 闭包捕获 | 与原 `stream_future` 闭包捕获一致，不新增捕获项 |
| `extract_attachment_ids` 从 `chat.rs` 移出 | 仅改变可见性 `fn` → `pub(crate) fn`，不改变逻辑 |

---

## 9. 不在本次重构范围

- **Channel / Op 枚举引入**：过度设计，当前无多 agent 协作需求
- **Driver 多态引入**：当前无多会话类型需求
- `chat_stream.rs` 拆分：已足够清晰（227 行），且 `spawn_*` 函数职责单一
- `chat_tasks.rs` 重构：`ChatTasks` 已足够简单（start/cancel/finish）
- 前端 `sendMessage` 拆分：前端不在本次范围
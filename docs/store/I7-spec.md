# I7 — Context Compaction 规格说明

## 1. 交付目标

上下文容量管理：当 token 用量达到阈值时，终止当前对话（只读），生成摘要存入 DB，前端显示"对话已达上限"横幅 + [新建对话] 按钮。新对话中模型可通过跨对话检索工具查询旧对话消息。

**设计原则**：
- **对话满即终止**：不压缩续接，不部分标记，整个对话标记为已满、只读
- **摘要即交接文档**：结构化摘要，用户复制粘贴到新对话后模型可直接继续工作
- **跨对话检索**（待 tool-call 就绪）：新对话中模型用 `search_history` 工具按关键词检索旧对话消息，补充摘要未覆盖的细节
- **零侵入消息流**：不插入摘要块、不修改消息、不加 `is_compressed` 列——对话满了就是满了

> **前置依赖**：I6 ✅（Agent 自定义模式 + token 追踪 + chatStore contextUsage 已就绪）
> **核心参考**：OpenBitFun `compression_prefetch.rs`（阈值判断）
> **注意**：`search_history` 工具依赖 tool-call 执行循环（当前未实现），I7 先实现终止 + 摘要 + 横幅，检索工具在 tool-call 就绪后接入

## 2. 功能规格

### 2.1 触发机制

```
COMPACTION_THRESHOLD_PERCENT = 80

should_terminate(prompt_tokens, context_window):
  prompt_tokens >= context_window * COMPACTION_THRESHOLD_PERCENT / 100
```

- **触发时机**：每次 `send_chat_message` 流式输出结束后（`finish_reason` 到达），检查 `usage.prompt_tokens` 是否超过阈值
- **执行方式**：在 `stream_future` 中异步执行摘要生成 + 标记对话已满，不阻塞当前流的输出

### 2.2 终止执行流程

```
1. generate_summary(provider, messages, model)
   ├─ 构造摘要请求：system prompt = SUMMARY_SYSTEM_PROMPT（结构化交接文档）
   ├─ 调用当前 Provider 的 chat_completion（非流式）
   └─ 返回摘要文本（结构化，可复制粘贴到新对话）

2. terminate_conversation(conn, conversation_id, summary_text)
   ├─ UPDATE conversations SET is_full = 1 WHERE id = ?1
   ├─ UPSERT compression_state (conversation_id, summary, last_compression_at, compression_count + 1)
   └─ emit conversation_terminated 事件 → 前端锁定对话 + 显示横幅（含摘要 + 复制按钮）
```

**关键**：对话标记 `is_full = 1` 后，`send_chat_message` 拒绝发送，输入区禁用。

### 2.3 跨对话检索工具（待 tool-call 就绪）

**`search_history(query, conversation_id?, limit=5)`** — 搜索历史对话消息，返回 `{id, conversation_id, conversation_title, role, content, timestamp}`。

- `query`：关键词匹配消息内容
- `conversation_id`：限定对话，不填搜全部

Tauri command：`search_history(app, query, conversation_id, limit) → Vec<HistoryMessageRow>`

SQL：`SELECT m.*, c.title FROM messages m JOIN conversations c ON m.conversation_id = c.id WHERE m.content LIKE ? ORDER BY m.timestamp DESC LIMIT ?`

> 此工具需要 tool-call 执行循环支持。I7 先实现 Tauri command + SQL，tool-call 循环就绪后注册为模型可用工具。

### 2.4 对话终止 UI

对话达到上限后：

- **横幅**（替代输入区位置）：显示摘要内容 + [复制摘要] 按钮 + [新建对话] 按钮
- **输入区禁用**：`send_chat_message` 返回错误，输入框 disabled
- **消息列表只读**：所有消息正常渲染，不可编辑/重新生成
- **侧边栏标记**：对话标题旁显示 `<LockOutlined />` 图标（`@ant-design/icons`），表示已满

**用户工作流**：
1. 对话满 → 横幅显示摘要
2. 点击 [复制摘要] → 摘要复制到剪贴板
3. 点击 [新建对话] → 新对话创建
4. 在新对话中粘贴摘要 → 模型从摘要恢复工作状态
5. 模型如需更多细节 → 调用 `search_history` 检索旧对话

### 2.5 摘要 System Prompt（交接文档）

摘要定位为**可复制的交接文档**：用户粘贴到新对话后，模型无需额外检索即可继续基本工作。

```
请生成本对话的交接摘要，格式如下：

## 对话摘要

**目标**：（用户的核心目标，1-2 句话）
**进展**：（已完成的关键步骤，每步一句）
**待办**：（未完成的事项）
**关键信息**：（后续需要引用的具体内容：文件路径、配置值、错误信息等）

要求：
- 摘要应自包含，粘贴到新对话后 AI 可直接继续工作
- 关键信息必须写具体值（文件路径、变量名、错误码），不要写"某个文件"
- 不记录每步操作细节，只记录关键节点
```

**摘要示例**：

```
## 对话摘要

**目标**：为 assistant 项目实现上下文压缩功能（I7）

**进展**：
- 完成规格文档 I7-spec.md 和任务清单 I7-tasklist.md
- 确定设计：对话满即终止 + 交接摘要 + 跨对话检索工具
- conversations 表新增 is_full 列

**待办**：
- 实现 should_terminate + generate_summary + terminate_conversation
- 实现 search_history Tauri command
- 前端横幅 + 复制摘要 + 新建对话按钮

**关键信息**：
- 规格文档：docs/store/I7-spec.md
- 任务清单：docs/store/I7-tasklist.md
- compression_state 表已有，需新增 summary 列
- AppConf 新增 auto_compact + compact_threshold_percent
```

### 2.6 摘要存储

摘要存入 `compression_state.summary` 列，前端通过 `conversation_terminated` 事件 payload 获取，也可通过 `get_compression_state` command 读取。

```sql
-- 已有表，新增 summary 列
CREATE TABLE IF NOT EXISTS compression_state (
    conversation_id TEXT PRIMARY KEY REFERENCES conversations(id) ON DELETE CASCADE,
    summary TEXT,
    last_compression_at INTEGER,
    compression_count INTEGER NOT NULL DEFAULT 0
);
```

### 2.7 conversations 表扩展

```sql
ALTER TABLE conversations ADD COLUMN is_full INTEGER NOT NULL DEFAULT 0;
```

**`is_full` 语义**：
- `0`：正常对话，可继续发送消息
- `1`：已满对话，只读，`send_chat_message` 拒绝

**ConversationRow 扩展**：新增 `is_full: bool` 字段

**send_chat_message 守卫**：

```rust
if conversation.is_full {
    return Err("此对话已达上下文上限，请新建对话".to_string());
}
```

### 2.8 AppConf 扩展

```rust
pub struct AppConf {
    // ... 现有字段
    #[serde(default = "default_true")]
    pub auto_compact: bool,           // 自动终止开关，默认 true
    #[serde(default)]
    pub compact_threshold_percent: u8, // 终止阈值百分比，默认 0（使用内置 80%）
}
```

前端 `settingsStore.ts` AppConf interface 同步新增 `auto_compact: boolean` + `compact_threshold_percent: number`。

### 2.9 声明式设置注册

```typescript
defineSetting({
  key: "auto_compact",
  group: "app",
  section: "general-main",
  type: "boolean",
  default: true,
  label: "上下文上限自动终止",
  extra: "对话达到上下文上限时自动终止并建议新建对话",
}),

defineSetting({
  key: "compact_threshold_percent",
  group: "app",
  section: "general-main",
  type: "slider",
  default: 0,
  label: "终止阈值",
  min: 0,
  max: 99,
  step: 5,
  marks: { 0: "默认(80%)", 50: "50%", 80: "80%", 99: "99%" },
  extra: "上下文用量达到此百分比时终止对话，0 表示使用默认 80%",
  visible: (conf) => conf.auto_compact === true,
}),
```

### 2.10 前端 UI

1. **对话满横幅**：替代输入区，显示摘要内容 + [复制摘要] 按钮 + [新建对话] 按钮
2. **输入区禁用**：`is_full` 对话的输入框 disabled
3. **侧边栏标记**：已满对话标题旁显示 `<LockOutlined />` 图标
4. **chat-delta 事件扩展**：新增 `conversation_terminated` 事件（payload 含 summary）

### 2.11 常量

```rust
// assistant-config/constant.rs
pub const COMPACTION_THRESHOLD_PERCENT: u8 = 80;
pub const MAX_SUMMARY_REQUEST_TOKENS: u32 = 4_096;
```

```typescript
// config/constants.ts
export const COMPACTION_THRESHOLD_PERCENT = 80;
```

## 3. 架构设计

### 3.1 模块归属

| 模块 | Crate | 职责 |
|------|-------|------|
| `compaction.rs` | assistant-agent | CompactionState + should_terminate + generate_summary |
| `compression_state.rs` | assistant-storage | compression_state 表 CRUD |
| `conversations.rs` | assistant-storage | search_history_messages 检索函数 |
| `cmd/chat.rs` | assistant-app | 流结束后触发终止 + is_full 守卫 |
| `cmd/conversation.rs` | assistant-app | search_history Tauri command |
| `ChatView.tsx` | 前端 | 对话满横幅 + 输入区禁用 |
| `chatStore.ts` | 前端 | isFull 状态 |
| `chatDeltaEvents.ts` | 前端 | conversation_terminated 事件消费 |

### 3.2 数据流

```
send_chat_message()
  ├─ 检查 conversation.is_full → true → 返回错误
  ├─ 准备消息、spawn(stream_future)
  └─ stream_future 异步执行：
       ├─ 流式输出逐字 emit chat-delta
       ├─ finish_reason 到达
       │    ├─ 保存最终内容到 DB（已有）
       │    ├─ 检查 should_terminate(prompt_tokens, context_window)
       │    │    ├─ true + auto_compact=true：
       │    │    │    ├─ emit conversation_terminated
       │    │    │    ├─ generate_summary → LLM 生成摘要（2-3 秒）
       │    │    │    ├─ terminate_conversation → 标记 is_full=1 + 存摘要
       │    │    │    └─ 前端锁定对话 + 显示横幅
       │    │    └─ false / auto_compact=false：跳过
       │    └─ finish_tx.send(conv_id)
       └─ 流结束
```

### 3.3 与 OpenBitFun 的对比

| | OpenBitFun | 本项目 |
|---|---|---|
| 策略 | 压缩续接：摘要替换早期消息，对话继续 | **终止对话**：对话满了就结束，开新对话 |
| 检索 | 无跨对话检索 | **跨对话检索工具**：新对话中模型搜旧对话 |
| 复杂度 | CompressionPrefetch + plan + apply + is_compressed | should_terminate + generate_summary + is_full |

## 4. 参考映射

| 功能点 | 参考项目 | 参考文件 | 借鉴内容 |
|--------|---------|---------|---------|
| 阈值判断 | OpenBitFun | compression_prefetch.rs L10-15 | `in_prefetch_window` 阈值公式（简化为百分比阈值） |
| 压缩状态 | OpenBitFun | session.rs L84-93 | `CompressionState { last_compression_at, compression_count }` |
| 会话配置 | OpenBitFun | session.rs L179,193 | `auto_compact: bool` 开关 |

## 5. 验收标准

1. 长对话 token 接近窗口 80% 时自动终止对话
2. 终止不中断当前流式输出（流结束后异步执行）
3. 摘要为结构化交接文档（目标/进展/待办/关键信息），可复制粘贴到新对话
4. 横幅显示摘要 + [复制摘要] 按钮 + [新建对话] 按钮
5. 终止后对话只读：输入区禁用、send_chat_message 拒绝
6. `search_history` Tauri command 可用（tool-call 注册待后续）
7. 侧边栏已满对话显示 `<LockOutlined />` 图标
8. 用户可在设置中关闭自动终止
9. `compression_state` 表正确持久化摘要和状态
10. cargo check + clippy + test 通过
# I2b — Tool Call 状态追踪规格说明

> **来源**：从 I2-History §2.6 / §3.3 / §4.5 / §4.6 / §5.2 / §5.3 / §6.2 暂缓内容提取
> **前置**：I2-History ✅ 已完成，**I6 Agent Loop**（ARCHITECTURE.md 声明暂不实现）
> **阻塞**：总架构声明 Function Calling 暂不实现，`send_chat_message` 无 agent loop，LLM 返回 tool_call 后应用只透传前端显示，不做"调用工具→回填结果→继续对话"

## 1. 目标

构建完整的 Tool Call 五态追踪系统：
- **五态模型**：`running` → `success` / `error` / `awaiting_user` / `cancelled`
- **流式事件扩展**：后端 emit `tool_call_status` / `tool_call_result` / `tool_call_started_at` / `tool_call_finished_at`
- **DB 持久化**：`tool_calls` 表存储完整生命周期
- **UI 渲染**：`ToolCallBubble` 三态渲染 + `ToolTimeline` 时间线视图
- **ChatError 扩展**：`ToolTimeout` / `SubagentFailed` 错误码

---

## 2. 数据模型

### 2.1 ToolCallStatus 五态

```typescript
type ToolCallStatus = 'running' | 'success' | 'error' | 'awaiting_user' | 'cancelled'
```

状态转换：

```
running ──→ success
        ──→ error
        ──→ awaiting_user ──→ running（用户批准后继续）
                           ──→ cancelled
        ──→ cancelled
```

### 2.2 ToolCallEntry

```typescript
interface ToolCallEntry {
  id: ToolCallId
  name: string
  arguments: string
  result?: string
  status: ToolCallStatus
  started_at: number
  finished_at: number | null
  is_error?: boolean
  subagent?: SubagentActivity
}
```

### 2.3 SubagentActivity

```typescript
interface SubagentActivity {
  task_id: string
  agent_id: string
  status: 'running' | 'awaiting_user' | 'completed' | 'failed'
  display_name?: string
  child_iteration?: number
  child_max_iterations?: number
  iterations?: number
  elapsed_ms?: number
  output?: string
  tool_calls: ToolCallEntry[]
}
```

> **注**：当前无 subagent 架构，`SubagentActivity` 为未来预留。

### 2.4 ChatError 扩展

```typescript
enum ChatErrorCode {
  // ... 现有 13 种 ...
  GuardrailBlocked = "GuardrailBlocked",    // ⏸️暂缓：当前无 guardrail
  BudgetExhausted = "BudgetExhausted",      // ⏸️暂缓：当前无 budget
  ToolTimeout = "ToolTimeout",              // ← 本规格：工具执行超时
  SubagentFailed = "SubagentFailed",        // ← 本规格：子 Agent 失败
}
```

---

## 3. DB Schema

### 3.1 tool_calls 表

```sql
CREATE TABLE IF NOT EXISTS tool_calls (
  id TEXT PRIMARY KEY,
  message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  arguments TEXT NOT NULL,
  result TEXT,
  status TEXT NOT NULL DEFAULT 'running',
  started_at INTEGER NOT NULL,
  finished_at INTEGER,
  is_error INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_tool_calls_message
  ON tool_calls(message_id);
```

---

## 4. Rust 后端

### 4.1 tool_calls.rs 新增模块

| 函数 | 签名 | 说明 |
|------|------|------|
| `insert_tool_call` | `(conn, entry) → Result<()>` | 插入工具调用 |
| `update_tool_call_status` | `(conn, id, status, result, finished_at) → Result<()>` | 更新状态 |
| `get_tool_calls` | `(conn, message_id) → Result<Vec<ToolCallRow>>` | 获取消息的工具调用 |

### 4.2 流式事件扩展

`send_chat_message` 扩展：流式消费中 emit 以下事件：

```rust
// ChatDeltaPayload 扩展
pub struct ChatDeltaPayload {
    // ... 现有字段 ...
    pub delta_tool_call_id: Option<String>,
    pub delta_tool_call_name: Option<String>,
    pub delta_tool_call_arguments: Option<String>,
    pub tool_call_status: Option<String>,        // ToolCallStatus
    pub tool_call_result: Option<String>,
    pub tool_call_started_at: Option<i64>,
    pub tool_call_finished_at: Option<i64>,
}
```

`chat_tasks.rs`：流式消费中插入 tool_calls 表记录。

### 4.3 Tauri Commands

无新增 command（tool_call 状态通过流式事件推送，不需要前端主动查询）。

---

## 5. 前端

### 5.1 chatStore 扩展

```typescript
// ChatDeltaPayload 扩展
interface ChatDeltaPayload {
  conversation_id: string
  message_id: string
  delta_text?: string
  delta_reasoning?: string
  delta_tool_call_id?: string
  delta_tool_call_name?: string
  delta_tool_call_arguments?: string
  tool_call_status?: ToolCallStatus
  tool_call_result?: string
  tool_call_started_at?: number
  tool_call_finished_at?: number
  finish_reason?: string
  usage?: TokenUsage
  error?: string
}
```

**chatStore 处理**：
- `delta_tool_call_id` + `tool_call_status = 'running'` → 创建新 `ToolCallEntry`
- `tool_call_status` 更新 → 更新对应 entry 的 status/result/finished_at
- `tool_call_result` → 更新对应 entry 的 result

**Message.toolCalls 类型**：从 `ToolCall[]` 改为 `ToolCallEntry[]`（含 status/result/started_at/finished_at）。

### 5.2 ToolCallBubble 组件

`src/chat/ToolCallBubble.tsx`：三态渲染

| 状态 | 渲染 |
|------|------|
| `running` | 旋转加载图标 + 工具名称 |
| `success` | ✓ + 工具名称 + 耗时 + 可折叠结果 |
| `error` | ✗ + 错误信息 |
| `awaiting_user` | ⚠ + 审批按钮 |
| `cancelled` | ⊘ 灰色 |

### 5.3 ToolTimeline 组件

`src/chat/ToolTimeline.tsx`：工具调用时间线视图

- 按执行顺序展示每个 `ToolCallEntry`
- 可折叠/展开
- 显示 started_at → finished_at 的完整生命周期

---

## 6. 实施步骤

### Step 1：Rust 后端 tool_calls 表 + CRUD

1. `db.rs`：`CREATE TABLE IF NOT EXISTS tool_calls`
2. 新增 `tool_calls.rs`：`ToolCallRow` + insert/update/get
3. `lib.rs`：`pub mod tool_calls`

**验证**：表创建成功，CRUD 单元测试通过。

### Step 2：Rust 后端流式事件扩展

1. `ChatDeltaPayload` 新增 tool_call 相关字段
2. `chat_tasks.rs`：流式消费中 emit tool_call 事件 + 插入 DB

**验证**：Agent Loop 执行工具时，前端收到 tool_call_status 事件。

### Step 3：前端 chatStore 适配

1. `chatTypes.ts`：`ToolCallEntry` 类型定义，`Message.toolCalls` 改为 `ToolCallEntry[]`
2. `initChatDeltaListener` 扩展：处理 tool_call_status 更新

**验证**：流式中 tool_call 状态正确更新。

### Step 4：前端 ToolCallBubble + ToolTimeline

1. 新增 `ToolCallBubble.tsx`：五态渲染
2. 新增 `ToolTimeline.tsx`：时间线视图
3. `MessageBubble.tsx`：集成 ToolCallBubble

**验证**：工具调用显示正确状态。

---

## 7. 验收标准

- [ ] **五态转换**：`running` → `success` / `error` / `awaiting_user` / `cancelled` 状态转换正确
- [ ] **DB 持久化**：tool_calls 表记录完整生命周期
- [ ] **流式事件**：前端收到 `tool_call_status` / `tool_call_result` 事件
- [ ] **ToolCallBubble 渲染**：五态显示正确
- [ ] **ToolTimeline**：时间线按执行顺序展示
- [ ] **ChatError 扩展**：`ToolTimeout` / `SubagentFailed` 错误码可用

---

## 8. 阻塞说明

本规格所有内容依赖 **I6 Agent Loop** 实现。当前 `send_chat_message` 无 agent loop：
1. `running→success/error` 状态转换依赖工具执行，无 agent loop 则状态永远无法推进
2. `tool_calls` 表的 `result`/`status`/`finished_at` 永远无值可写
3. `awaiting_user`/`cancelled` 态无触发场景

等 I6 实现 Agent Loop 后再回来实现完整五态模型。
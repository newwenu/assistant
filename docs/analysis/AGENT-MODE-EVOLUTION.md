# Agent 模式演进与 tool 角色引入分析

> 生成日期：2026-10-20
> 范围：chat runtime 全链路 — 前端 orchestrator → Tauri command → Rust chat_runtime → SQLite 存储
> 性质：**纯分析文档**，描述当前代码结构与可能的演进方向，不作为执行计划
> 目标：评估当前交替模式（user↔assistant）假设的渗透范围，分析 tool 角色及新 agent 模式引入的可能路径

---

## 1 现状：交替假设的渗透地图

当前 chat runtime 的核心假设是 **user 与 assistant 严格交替**：

```
null → user₁ → assistant₁ → user₂ → assistant₂ → ...
```

此假设散落在前后端多处，形成隐式耦合。以下按层级标注渗透点：

### 1.1 前端层

| 文件 | 行号 | 硬编码 | 语义 |
|------|------|--------|------|
| `chatOrchestrator.ts` | L249-257 | 固定创建 `user→assistant` 一对 | 发送时乐观插入的消息序列 |
| `chatOrchestrator.ts` | L239 | `branchPointMsg.role === "assistant"` | 仅 assistant 可作为分支点 |
| `chatMessageSlice.ts` | L125 | `parentMsg.role !== "user"` | regenerate 断言 assistant 的父必为 user |
| `chatMessageSlice.ts` | L140 | `role !== "assistant"` | 仅 assistant 可重试 |
| `MessageBubble.tsx` | L107-111 | 三分支 user/compactSummary/assistant | 样式选择无 tool 分支 |
| `MessageBubble.tsx` | L97-98 | `prevRole !== message.role` → 间距 | 相邻角色不同即加间距 |
| `MessageContent.tsx` | L20 | `isUser` 二分支 | 渲染策略无 tool |
| `MessageList.tsx` | L143-146 | `role === "assistant"` | 流式消息 ID 判定 |
| `MessageActions.tsx` | L36-37 | `isUser`/`isAssistant` | 操作按钮无 tool |
| `HistoryDetail.tsx` | L60-70 | 同 depth = 同角色 | 分支视图分组假设 |
| `HistoryDetail.tsx` | L93-94 | `role === "user"` / `role !== "user"` | 根节点二分 |

### 1.2 后端层

| 文件 | 行号 | 硬编码 | 语义 |
|------|------|--------|------|
| `chat.rs` | L281-430 | 固定插入 user+assistant 两条 | send_chat_message 的消息序列 |
| `prompt_builder.rs` | L43-60 | 线性过滤 compact_summary | prepare_messages 无 tool 角色映射 |
| `prompt_builder.rs` | L28-30 | `supports_tool_call` 仅 Custom | AgentKind 能力声明不完整 |

### 1.3 存储层

| 文件 | 行号 | 硬编码 | 语义 |
|------|------|--------|------|
| `conversations.rs` | L474-478 | export Markdown 角色匹配 | 无 tool 格式化 |
| `db.rs` | L47 | `role TEXT NOT NULL` | ✅ 无枚举约束，任意角色可存 |

---

## 2 引入 tool 角色后的树结构变化

### 2.1 当前树（交替模式）

```
null
 └─ U₁ (parentId: null)
     ├─ A₁ (parentId: U₁)              ← 第1次回复
     │   └─ U₂ (parentId: A₁)
     │       └─ A₂ (parentId: U₂)      ← 第2次回复
     └─ A₃ (parentId: U₁)              ← 重新生成分支
```

**不变量**：assistant.parentId → user，user.parentId → assistant | null

### 2.2 引入 tool 后的树（tool-loop 模式）

```
null
 └─ U₁ (parentId: null)
     └─ A₁ (parentId: U₁)              ← assistant 发起 tool call
         └─ T₁ (parentId: A₁)          ← tool 执行结果
             └─ A₂ (parentId: T₁)      ← assistant 根据 tool 结果继续
                 └─ T₂ (parentId: A₂)  ← 又一个 tool call
                     └─ A₃ (parentId: T₂)  ← 最终回复
```

**新不变量**：
- assistant.parentId → user | tool
- tool.parentId → assistant
- user.parentId → assistant | null
- **同角色可直连**：assistant → tool → assistant

### 2.3 可能的 autonomous 模式

```
null
 └─ A₁ (parentId: null)              ← assistant 自主发起（无 user 输入）
     └─ T₁ (parentId: A₁)
         └─ A₂ (parentId: T₁)
             └─ U₁ (parentId: A₂)    ← user 中途介入
                 └─ A₃ (parentId: U₁)
```

**进一步打破**：assistant 可作为根节点（parentId: null），user 不一定是第一条消息。

---

## 3 影响评估与分类

### 3.1 ✅ 可复用（角色无关，与 agent 模式解耦）

| 模块 | 文件 | 原因 |
|------|------|------|
| 流消费引擎 | `chat_runtime.rs` | `consume_stream` 只消费 `ChatDelta` 流，不关心角色交替。`StreamAccumulator`、`ToolCallAccumulator` 纯增量累积 |
| ToolCall 累积器 | `chat_runtime.rs:230-290` | `apply_delta`/`finish` 处理 tool call 流式片段，与消息角色无关 |
| 流事件分发 | `chatStreamEvents.ts` | 监听 Tauri 事件、去重、合并 delta，不关心角色 |
| 流合并器 | `streamCoalescer.ts` | delta 合并、序列去重，纯数据管道 |
| 邻接表存储 | `conversations.rs` | `parent_id` 树结构、`sibling_index` 重排、`delete_subtree` 全部角色无关 |
| DB schema | `db.rs` | `messages` 表 `role TEXT` 无枚举约束，任意角色都能存 |
| MessageTree | `messageTree.ts` | 只建 `parentId→children` 映射，不关心角色 |
| BranchPicker | `BranchPicker.tsx` | 纯 UI 组件，只看 `siblingIndex/siblingCount` |
| ToolCallBlock | `ToolCallBlock.tsx` | 渲染 tool call 信息，角色无关 |
| AgentMode 注册表 | `agentModeRegistry.ts` | `defineAgentKind`/`getAgentKind` 是开放注册机制 |
| Mascot 动画 | `useMascotChatSync.ts` | 已识别 `role === "tool"` 和 `toolCalls` |

### 3.2 🔧 需修改（有交替假设硬编码，但架构可扩展）

| # | 模块 | 文件 | 硬编码点 | 修改方案 | 严重度 |
|---|------|------|---------|---------|--------|
| 1 | Orchestrator 发送 | `chatOrchestrator.ts:249-257` | 固定创建 user→assistant 一对 | 根据 agent kind 调用不同 `createMessageSequence()` 策略 | 🔴 |
| 2 | Orchestrator 分支 | `chatOrchestrator.ts:239` | `branchPointMsg.role === "assistant"` | `isBranchableRole(branchPointMsg.role)` | 🟡 |
| 3 | Rust 发送 | `chat.rs:281-430` | 固定插入 user+assistant 两条 | 根据 `mode.kind` 选择插入策略 | 🔴 |
| 4 | prompt_builder | `prompt_builder.rs:43-60` | 无 tool 角色映射 | 增加 `message_role_from_str("tool")` + tool_call_id 关联 | 🔴 |
| 5 | regenerateMessage | `chatMessageSlice.ts:122-125` | `parentMsg.role !== "user"` | `isUserOrToolResult(parentMsg.role)` 或查找同轮最近 user | 🔴 |
| 6 | retryFromError | `chatMessageSlice.ts:140` | `role !== "assistant"` | `isRetryableRole(role)` | 🟡 |
| 7 | MessageBubble 样式 | `MessageBubble.tsx:107-111` | 三分支 | role → style 映射表，tool 用独立样式 | 🟡 |
| 8 | MessageContent 渲染 | `MessageContent.tsx:20` | isUser 二分支 | 增加 tool 渲染策略 | 🟡 |
| 9 | RoleContext 间距 | `MessageBubble.tsx:97-98` | 角色不同即加间距 | `isTurnBoundary(prevRole, currentRole)` | 🟡 |
| 10 | streamingMsgId | `MessageList.tsx:143-146` | `role === "assistant"` | `isStreamableRole(role)` 包含 tool | 🔴 |
| 11 | MessageActions | `MessageActions.tsx:36-37` | isUser/isAssistant | 增加 isTool 分支 | 🟢 |
| 12 | buildBranchView | `HistoryDetail.tsx:60-70` | 同 depth 同角色 | 按 (depth, roleCategory) 分组 | 🟡 |
| 13 | AgentKind 定义 | `prompt_builder.rs:15-30` | 4 种 kind | 扩展 kind 枚举 + 能力声明 | 🟡 |
| 14 | AgentMode type | `types.ts:7` | kind 联合类型 | 扩展联合类型 | 🟢 |
| 15 | Markdown 导出 | `conversations.rs:474-478` | 无 tool 格式 | 增加 tool 角色格式化 | 🟢 |

### 3.3 🔄 建议重新实现（当前设计与新 agent 模式根本冲突）

| 模块 | 当前设计 | 冲突原因 | 新设计方向 |
|------|---------|---------|-----------|
| 消息序列策略 | orchestrator + chat.rs 各自硬编码 user→assistant | 交替模式是一种策略，tool 循环是另一种，autonomous 又是一种。散落两端的硬编码无法统一扩展 | **Strategy 模式**：`MessageSequenceStrategy` 接口，每种 agent kind 注册一个策略 |
| 对话轮次模型 | 无，隐含"一对 user+assistant = 一轮" | tool 引入后一轮可能是 user→assistant→tool→assistant→tool→assistant | **Turn 模型**：`ChatTurn = { userMessage, responses: ResponseBranch[] }`，UI 渲染/分支/删除基于 Turn |
| 分支视图构建 | `buildBranchView` 按 depth 分组 | 同 depth 可能混合角色（depth 2 是 tool，depth 3 又是 assistant） | **基于 Turn 的分支视图**：先按 Turn 分组，Turn 内再按分支分组 |
| 删除策略 | 固定子树删除 | 不同 agent 模式删除语义不同 | **删除策略跟随 AgentKind**：`kind.deleteStrategy(message, tree) → DeleteAction` |

---

## 4 可能的架构演进方向

> 以下为一种可能的演进思路，非唯一方案，亦非执行计划。各方案可独立采用或组合采用。

### 4.1 一种可能的核心设计：MessageSequenceStrategy

```typescript
// 前端策略接口
interface MessageSequenceStrategy {
  // 发送时创建的乐观消息序列
  createOptimisticMessages(
    convId: string,
    content: string,
    parentMessage: Message | null,
  ): Message[];

  // 哪些角色可以作为分支点
  isBranchableRole(role: string): boolean;

  // 重新生成时如何查找父消息
  findRegenerateParent(message: Message, allMessages: Message[]): Message | null;

  // 删除策略
  getDeleteAction(message: Message, tree: MessageTree): DeleteAction;

  // 流式输出时哪些角色是流式目标
  isStreamableRole(role: string): boolean;
}

type DeleteAction =
  | { kind: "subtree" }                    // 删整棵子树
  | { kind: "single" }                     // 只删自己，子节点重挂
  | { kind: "single-if-leaf"; otherwise: "subtree" };  // 叶子单删，否则子树删
```

```rust
// 后端策略接口（Rust 侧）
trait MessageSequenceStrategy: Send + Sync {
    fn insert_messages(
        &self,
        conn: &Connection,
        conv_id: &str,
        content: &str,
        parent_id: Option<&str>,
        branch_point: Option<&MessageRow>,
    ) -> Result<(String, String), String>;  // (user_msg_id, assistant_msg_id)

    fn is_branchable_role(&self, role: &str) -> bool;
    fn supports_tool_loop(&self) -> bool;
}
```

### 4.2 三种可能的内置策略

#### AlternatingStrategy（当前行为的抽象）

```
发送：创建 [user, assistant] 一对
分支：仅 assistant 可分支
重新生成：assistant.parent 必须是 user
删除：user → 子树删除，assistant → 子树删除
流式：仅 assistant
```

#### ToolLoopStrategy（tool 角色引入的一种可能）

```
发送：创建 [user, assistant] 一对，tool 消息由后端在流结束后插入
分支：assistant 和 tool 可分支
重新生成：assistant.parent 可以是 user 或 tool
删除：user → 子树删除，assistant(有兄弟) → 单删，tool → 单删
流式：assistant 和 tool
```

#### AutonomousStrategy（自主 agent的一种可能）

```
发送：创建 [assistant] 单条（无 user 输入），或 [user, assistant] 一对
分支：任意角色可分支
重新生成：沿 parentId 回溯到最近 user 或根
删除：按角色和位置自适应
流式：assistant 和 tool
```

### 4.3 一种可能的 Turn 模型（UI 层概念）

```typescript
interface ChatTurn {
  // 本轮的 user 消息（autonomous 模式可能为 null）
  userMessage: Message | null;
  // 本轮的所有回复分支
  responses: ResponseBranch[];
  // 本轮在对话中的序号
  turnIndex: number;
}

interface ResponseBranch {
  // 从 user 之后到下一个 user 之前的所有消息
  // 交替模式：[assistant]
  // tool-loop 模式：[assistant, tool, assistant, tool, assistant]
  messages: Message[];
  // 分支的叶子节点 ID（用于 activeBranch 追踪）
  leafId: string;
}
```

**Turn 模型不持久化**，它是从邻接表树实时计算的视图。存储层仍然是 `parent_id` 邻接表。

### 4.4 Turn 计算的一种可能算法

```typescript
function computeTurns(messages: Message[], tree: MessageTree): ChatTurn[] {
  const turns: ChatTurn[] = [];
  let currentTurn: ChatTurn | null = null;

  // 按 BFS 顺序遍历树
  const ordered = tree.traverseBFS();

  for (const msg of ordered) {
    if (msg.role === "user" || (!currentTurn && msg.role === "assistant")) {
      // 新轮次开始
      currentTurn = {
        userMessage: msg.role === "user" ? msg : null,
        responses: [],
        turnIndex: turns.length,
      };
      turns.push(currentTurn);

      if (msg.role === "assistant") {
        // autonomous: assistant 是轮次的第一个消息
        currentTurn.responses.push({ messages: [msg], leafId: msg.id });
      }
    } else if (currentTurn) {
      // 追加到当前轮次的当前分支
      const branch = currentTurn.responses[currentTurn.responses.length - 1];
      branch.messages.push(msg);
      branch.leafId = msg.id;  // 更新叶子
    }
  }

  return turns;
}
```

### 4.5 演进后的分层架构示意

```
┌───────────────────────────────────────────────────────────────┐
│  UI 层                                                        │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────────┐ │
│  │ TurnView    │  │ BranchPicker │  │ MessageBubble        │ │
│  │ (按 Turn    │  │ (按分支选择) │  │ (role→style 映射)    │ │
│  │  渲染对话)  │  │              │  │                      │ │
│  └─────────────┘  └──────────────┘  └──────────────────────┘ │
├───────────────────────────────────────────────────────────────┤
│  策略层                                                       │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  MessageSequenceStrategy (接口)                         │ │
│  │  ├─ AlternatingStrategy  ← 当前行为                    │ │
│  │  ├─ ToolLoopStrategy     ← tool 角色引入               │ │
│  │  └─ AutonomousStrategy   ← 自主 agent                  │ │
│  └─────────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  DeleteStrategy (接口)                                  │ │
│  │  ├─ SubtreeDelete       ← 当前行为                     │ │
│  │  ├─ AdaptiveDelete      ← 按角色+位置自适应            │ │
│  │  └─ SoftDelete          ← 标记删除，可恢复             │ │
│  └─────────────────────────────────────────────────────────┘ │
├───────────────────────────────────────────────────────────────┤
│  Orchestrator 层                                              │
│  ┌────────────────────┐  ┌─────────────────────────────────┐ │
│  │ chatOrchestrator   │  │ chatStreamEvents               │ │
│  │ (调用策略创建消息) │  │ (✅ 可复用，不关心角色)       │ │
│  └────────────────────┘  └─────────────────────────────────┘ │
├───────────────────────────────────────────────────────────────┤
│  Runtime 层 (Rust)                                            │
│  ┌──────────────────┐  ┌──────────────┐  ┌────────────────┐ │
│  │ chat_runtime.rs  │  │prompt_builder│  │ chat.rs        │ │
│  │ (✅ 完全可复用)  │  │ (需加 tool   │  │ (需调用策略    │ │
│  │                  │  │  角色映射)   │  │  插入消息)     │ │
│  └──────────────────┘  └──────────────┘  └────────────────┘ │
├───────────────────────────────────────────────────────────────┤
│  存储层 (SQLite)                                              │
│  ┌──────────────────┐  ┌──────────────┐                      │
│  │ conversations.rs │  │ db.rs        │                      │
│  │ (✅ 邻接表角色   │  │ (✅ role TEXT│                      │
│  │  无关，可复用)   │  │  无枚举约束) │                      │
│  └──────────────────┘  └──────────────┘                      │
└───────────────────────────────────────────────────────────────┘
```

---

## 5 风险与约束

### 5.1 向后兼容

- **存储层**：邻接表 + `role TEXT` 无枚举约束，新角色直接存入，旧数据不受影响 ✅
- **旧对话**：交替模式的对话在 Turn 模型下退化为每轮一个 assistant，完全兼容 ✅
- **导出**：Markdown/JSON 导出需增加 tool 角色格式化，旧对话不受影响 ✅

### 5.2 性能

- **Turn 计算**：`computeTurns` 是 O(n) 遍历，与当前 `buildBranchView` 同阶，无性能风险
- **策略查找**：按 `agentKind` 查策略是 O(1) Map 查找，无性能风险
- **tool 消息插入**：后端在流结束后批量插入，不增加流式延迟

### 5.3 `as unknown as` 断言约束

根据项目规则，`as unknown as` 双重断言仅允许在 `settingsDispatcher.ts` 和 `i18n/index.ts` 两处使用。Strategy 模式的类型转换应使用泛型约束或 discriminated union，不得引入新的 double assertion。

---

## 6 与现有架构的对齐

### 6.1 AgentMode 注册机制

当前 `agentModeRegistry.ts` 的 `defineAgentKind` 已是开放注册机制：

```typescript
defineAgentKind({ id: "default", label: "...", color: "blue" });
defineAgentKind({ id: "quick",   label: "...", color: "green" });
defineAgentKind({ id: "deep",    label: "...", color: "purple" });
defineAgentKind({ id: "custom",  label: "...", color: "orange" });
```

若引入 Strategy 模式，可扩展为：

```typescript
defineAgentKind({
  id: "tool-loop",
  label: "settings.agent_mode.kind.tool_loop",
  color: "cyan",
  strategy: new ToolLoopStrategy(),   // ← 可能的新增
});
```

### 6.2 Rust AgentKind

当前 `prompt_builder.rs` 的 `AgentKind` enum：

```rust
pub enum AgentKind { Default, Quick, Deep, Custom }
```

若引入新 agent kind，可扩展为：

```rust
pub enum AgentKind { Default, Quick, Deep, Custom, ToolLoop, Autonomous }
```

`supports_tool_call()` 从 `matches!(self, Custom)` 可扩展为 `matches!(self, Custom | ToolLoop | Autonomous)`。

### 6.3 删除策略与现有 `delete_subtree` 的关系

当前 `delete_subtree` 是存储层的通用操作（递归 CTE 删除子树），**保留不动**。

若引入删除策略，可在其之上做业务层决策：

```typescript
// 策略决定动作，存储层执行
const action = strategy.getDeleteAction(message, tree);
switch (action.kind) {
  case "subtree":
    await chatService.deleteMessagePersist(message.id);  // 调用现有 delete_subtree
    break;
  case "single":
    await chatService.deleteSingleMessage(message.id);   // 可能的新增：单节点删除 + 重挂
    break;
}
```

若需单节点删除，Rust 侧需新增 `delete_single_message` 命令，实现单节点删除 + 子节点重挂 + `sibling_index` 重排。

---

## 7 总结

当前 chat runtime 以 user↔assistant 严格交替为核心假设，该假设渗透前后端 15+ 处。引入 tool 角色将打破交替不变量，引入 autonomous 模式将进一步打破"user 必为根"的假设。

可能的演进方向：

| 维度 | 现状 | + tool 角色 | + Strategy 模式 | + Turn 模型 | + Autonomous |
|------|------|------------|----------------|------------|-------------|
| 角色 | user, assistant | + tool | + tool | + tool | + tool |
| 树结构 | 严格交替 | 交替 + tool 链 | 交替 + tool 链 | 任意 | 任意 |
| 消息创建 | 硬编码 user→assistant | 硬编码 + 后端插 tool | Strategy 模式 | Strategy 模式 | Strategy 模式 |
| UI 渲染 | 按消息逐条 | 按消息 + tool 样式 | 按消息 + tool 样式 | 按 Turn | 按 Turn |
| 删除 | 子树删除 | 子树删除 | 子树删除 | 自适应删除 | 自适应删除 |
| 分支视图 | 按 depth 分组 | 按 depth 分组 | 按 depth 分组 | 按 Turn 分组 | 按 Turn 分组 |
| Agent 模式 | 4 种 kind | 4 种 kind | + tool-loop kind | + tool-loop kind | + autonomous kind |
| 存储改动 | — | 无 | 无 | 无 | 无 |
| chat_runtime 改动 | — | 无 | 无 | 无 | 无 |

**核心观察**：
1. Strategy 由 AgentKind 决定，不在 orchestrator 里 if-else
2. Turn 模型是 UI 层概念，存储层仍然是邻接表树
3. chat_runtime.rs 与 agent 模式正交，无需改动
4. 存储层邻接表 + sibling_index 对任意树形结构都适用，无需改动
5. 可复用模块（11 个）与需修改模块（15 处）边界清晰
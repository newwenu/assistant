# I6 — Agent 自定义模式 + Prompt 模板合并 规格说明

## 1. 交付目标

内置预设 + 用户自定义 Agent 模式 + Prompt 模板合并为统一体系 + token 追踪。

> **前置依赖**：I2 ✅（chatStore 流式输出 + ChatError + Prompt 模板 CRUD）+ I4 ✅（设置系统）
> **核心变更**：将 `prompt_templates` 表废弃，其能力（system prompt + 分类 + 内置预设）合并到 `agent_modes` 表。原 Prompt 模板的变量占位符 `{{variable}}` 交互已在 I2 中移除（DEVLOG 2026-10-05），不再保留。

### 当前缺口

| 缺口 | 说明 |
|------|------|
| 无 Agent 模式 | ARCHITECTURE.md §3.5 定义了 Quick/Standard/Deep 三级，但无数据层/UI/运行时集成 |
| temperature/max_tokens 硬编码 | `cmd.rs` L771-772 写死 `temperature: 0.7, max_tokens: 4096`，无法按模式调整 |
| Prompt 模板与 Agent 模式概念重叠 | 两者核心都是 system prompt + 分类 + 内置预设，维护两套 CRUD |
| Prompt 模板变量占位符已失效 | TemplateCards + 变量填写 Modal 已移除，`variables_json` 字段无消费端 |
| 无 token 追踪 | 对话无 token 用量展示 |

## 2. 功能规格

### 2.1 Agent 模式数据模型

统一 Agent 模式 + Prompt 模板为一个 `agent_modes` 表：

```sql
CREATE TABLE IF NOT EXISTS agent_modes (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  kind TEXT NOT NULL DEFAULT 'custom',      -- 'quick' | 'standard' | 'deep' | 'custom'
  prompt TEXT NOT NULL DEFAULT '',           -- system prompt（原 prompt_templates.content）
  model TEXT,                                -- 指定模型（null = 跟随对话模型）
  temperature REAL,                          -- null = 使用默认 0.7
  max_tokens INTEGER,                       -- null = 使用默认 4096
  tools TEXT NOT NULL DEFAULT '[]',          -- JSON: string[]，可用工具集
  presentation_rank INTEGER NOT NULL DEFAULT 99,  -- 排序权重
  is_builtin INTEGER NOT NULL DEFAULT 0,    -- 内置不可删除
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
```

**Rust 结构体**：

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentModeRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: String,           // default | quick | deep | custom
    pub prompt: String,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub tools_json: String,     // JSON array of tool IDs
    pub presentation_rank: u32,
    pub is_builtin: bool,
    pub created_at: i64,
    pub updated_at: i64,
}
```

**TypeScript 类型**：

```typescript
export interface AgentMode {
  id: string;
  name: string;
  description: string;
  kind: 'default' | 'quick' | 'deep' | 'custom';
  prompt: string;
  model: string | null;
  temperature: number | null;
  max_tokens: number | null;
  tools: string[];
  presentation_rank: number;
  is_builtin: boolean;
  created_at: number;
  updated_at: number;
}
```

### 2.2 内置预设（4 个）

3 个参数预设模式（无 system prompt）+ 1 个模板型（翻译助手），自动 seed：

| id | name | kind | prompt | model | temperature | max_tokens | tools | rank |
|----|------|------|--------|-------|-------------|------------|-------|------|
| `builtin-default` | 默认 | default | _(空)_ | null | 0.7 | 4096 | [] | 0 |
| `builtin-quick` | 快速 | quick | _(空)_ | null | 0.3 | 2048 | [] | 1 |
| `builtin-deep` | 深度 | deep | _(空)_ | null | 0.7 | 8192 | [] | 2 |
| `builtin-translate` | 翻译助手 | custom | `请将以下内容翻译为目标语言，保持原文的语气和格式。` | null | 0.3 | 4096 | [] | 10 |

**设计说明**：

- **默认**：无 system prompt，标准参数——最常用的纯对话模式，也是新建对话的默认选中
- **快速**：无 system prompt，低温度(0.3) + 短输出(2048)——适合简单问答，回答更简短确定
- **深度**：无 system prompt，长输出(8192)——适合需要长篇回答的场景
- **翻译助手**：唯一带 system prompt 的内置模式——翻译是高频且 prompt 稳定的场景，值得内置
- **kind='default'** 是新建对话的默认选中项，prompt 为空字符串时不注入 system message

> **tools 字段**：当前全部为 `[]`（Function Calling 未实现），后续迭代工具系统就绪后 Deep 可填入工具集。
> **原 Prompt 模板的代码解释/文本摘要/续写/头脑风暴不再内置**——这些场景的 prompt 因人而异，用户可自行创建自定义模式。

### 2.3 Agent 模式切换 UI（Orb 面板）

在 Orb 对话面板输入框上方新增模式选择器，作为独立的一行（mode bar），与顶部 chrome-bar 分离：

```
┌──────────────────────────────────────────────────────┐
│ [OpenAI ▾]  [gpt-4o ▾]  [＋]                        │  ← 连接层（chrome-bar）
├──────────────────────────────────────────────────────┤
│ 消息列表                                             │
├──────────────────────────────────────────────────────┤
│ [默认 ▾]                                             │  ← 行为层（mode bar）
│ [输入框]                                [MascotBtn]  │  ← 输入
├──────────────────────────────────────────────────────┤
│ [1.2k/128k]  [====]  12%                             │  ← token 用量
└──────────────────────────────────────────────────────┘
```

> **设计理由**：chrome-bar 是"连接层"（Provider + Model 决定连哪个后端），ModeSelector 是"行为层"（决定怎么对话），两者关注点不同。模式放在输入框正上方，用户打字前即可确认当前模式，且不挤占顶部空间。

> **Agent 模式与 Provider 无关**：模式列表是全局的，不随 Provider 切换而变化。Provider 仅决定可用模型列表，ModeSelector 始终展示全部 Agent 模式。

**模式选择器**：
- Ant Design `Select` 组件，`variant="chip"`，与 ProviderSelector / ModelSelector 风格一致
- 选项按 `presentation_rank` 排序
- 内置模式显示 kind 标签（默认/快速/深度），自定义模式显示 `custom` 标签
- 切换模式后：
  - 若新模式有 `model` 指定，自动切换模型选择器（可能跨 Provider）
  - 若新模式有 `temperature`/`max_tokens`，下次发送时生效
  - 模式的 `prompt` 作为 system_prompt 注入对话

### 2.4 Agent 模式管理页（主窗口设置）

替换原 `/settings/templates`，新路由 `/settings/agents`。

**列表视图**：
- 按分组展示：内置模式（不可删除）+ 自定义模式
- 每行：名称 + kind 标签 + 描述（ellipsis）+ 编辑/删除按钮
- 顶部：搜索框 + 新建按钮

**编辑/新建表单**（Drawer 或 Modal）：

| 字段 | 组件 | 说明 |
|------|------|------|
| name | Input | 模式名称 |
| description | TextArea | 描述 |
| prompt | TextArea (大) | system prompt |
| model | Select (可选) | 指定模型，null = 跟随对话 |
| temperature | Slider (0-2, step 0.1) | null = 默认 0.7 |
| max_tokens | InputNumber | null = 默认 4096 |
| tools | Checkbox.Group | 工具集选择（当前为空，预留） |

**内置模式**：仅 prompt / temperature / max_tokens 可编辑，name / kind / 不可改，不可删除。

### 2.5 chatStore 集成

**新增状态**：

```typescript
interface ChatState {
  // ... 现有字段
  selectedModeId: string | null;           // 当前选中的 Agent 模式 ID
  agentModes: AgentMode[];                 // 所有可用模式
  loadAgentModes: () => Promise<void>;
  selectMode: (modeId: string) => void;
}
```

**发送消息时注入模式配置**：

当前 `send_chat_message` 仅传 `conversationId` + `content`，需扩展：

```typescript
await invoke("send_chat_message", {
  conversationId: convId,
  content,
  modeId: get().selectedModeId,   // 新增
});
```

**Rust 端 `send_chat_message` 变更**：

```rust
pub async fn send_chat_message(
    app: AppHandle,
    conversation_id: String,
    content: String,
    mode_id: Option<String>,      // 新增
) -> Result<(), String> {
    // ... 现有逻辑

    // 读取模式配置
    let mode = mode_id
        .and_then(|id| agent_modes::get_by_id(&conn, &id).ok().flatten());

    let system_prompt = mode.as_ref()
        .map(|m| m.prompt.as_str())
        .unwrap_or("");  // 空字符串 = 无 system prompt（默认/快速/深度模式）

    let temperature = mode.as_ref()
        .and_then(|m| m.temperature)
        .unwrap_or(0.7);

    let max_tokens = mode.as_ref()
        .and_then(|m| m.max_tokens)
        .unwrap_or(4096);

    let options = ChatOptions {
        temperature,
        max_tokens,
        stream: true,
        context_window_tokens,
    };

    // system_prompt 注入 chat_messages 前方
    if !system_prompt.is_empty() {
        chat_messages.insert(0, ChatMessage {
            role: Role::System,
            content: system_prompt.to_string(),
        });
    }
    // ... 后续流式逻辑不变
}
```

### 2.6 token 追踪

**对话级 token 统计**：

在 Orb 面板底部 ContextBar 显示当前对话的 token 用量（已有 ContextBar 组件，接入数据源即可）：

```
[1.2k/128k]  [====]  12%
```

- `1.2k` = 当前对话已用 prompt_tokens + completion_tokens
- `128k` = 模型上下文窗口大小
- 百分比超 70% 黄色警告，超 90% 红色警告

**数据来源**：
- 流式输出结束时 `ChatDelta.usage` 已包含 `prompt_tokens` / `completion_tokens`
- `update_message_content` 已保存 `tokens` 字段
- 对话总 token = `SUM(messages.tokens)` WHERE conversation_id

**Tauri Command 新增**：

```rust
pub fn get_conversation_token_usage(
    app: AppHandle,
    conversation_id: String,
) -> Result<TokenUsageSummary, String>
```

```rust
pub struct TokenUsageSummary {
    pub total_tokens: i64,
    pub context_window_tokens: u32,
    pub percentage: f32,
}
```

### 2.7 Prompt 模板移除

**移除清单**：

| 项目 | 操作 | 说明 |
|------|------|------|
| `prompt_templates` 表 | DROP TABLE | 无外部消费者，直接删除 |
| `templates.rs` | 删除文件 | 整个模块移除 |
| `lib.rs` 中 `pub mod templates` | 删除 | 模块注册移除 |
| `list_prompt_templates` command | 删除 | 无调用方 |
| `save_prompt_template` command | 删除 | 无调用方 |
| `TemplateManager.tsx` | 删除 | 由 `AgentModeManager.tsx` 替代 |
| `/settings/templates` 路由 | 删除 | 不保留重定向 |
| 侧边栏"Prompt 模板"菜单项 | 改为"Agent 模式" | 路由指向 `/settings/agents` |

### 2.8 conversations 表扩展

新增 `mode_id` 列，记录对话创建时使用的 Agent 模式：

```sql
ALTER TABLE conversations ADD COLUMN mode_id TEXT;
```

- 新建对话时若选中了 Agent 模式，`mode_id` 写入模式 ID
- 后续切换模式不回写（对话的模式在创建时确定）
- `mode_id` 为 null 表示未使用模式（兼容旧对话）

## 3. Rust 后端规格

### 3.1 新增文件

| 文件 | 说明 |
|------|------|
| `assistant-storage/src/agent_modes.rs` | AgentModeRow CRUD + seed_builtin_modes |

### 3.2 agent_modes.rs 函数清单

| 函数 | 签名 | 说明 |
|------|------|------|
| `init_table` | `(conn: &Connection) → Result<()>` | CREATE TABLE IF NOT EXISTS |
| `insert_mode` | `(conn: &Connection, row: &AgentModeRow) → Result<()>` | 插入 |
| `get_modes` | `(conn: &Connection) → Result<Vec<AgentModeRow>>` | 查询全部，按 presentation_rank 排序 |
| `get_by_id` | `(conn: &Connection, id: &str) → Result<Option<AgentModeRow>>` | 按 ID 查询 |
| `update_mode` | `(conn: &Connection, row: &AgentModeRow) → Result<()>` | 更新 |
| `delete_mode` | `(conn: &Connection, id: &str) → Result<()>` | 删除（禁止删除 is_builtin） |
| `seed_builtin_modes` | `(conn: &Connection) → Result<()>` | 插入 4 个内置预设（INSERT OR IGNORE） |
| `get_conversation_token_usage` | `(conn: &Connection, conv_id: &str) → Result<TokenUsageSummary>` | 对话 token 统计 |

### 3.3 Tauri Commands 新增/修改

| command | 签名 | 说明 |
|---------|------|------|
| `list_agent_modes` | `(app: AppHandle) → Result<Value, String>` | 列出所有 Agent 模式 |
| `save_agent_mode` | `(app: AppHandle, mode: Value) → Result<(), String>` | 新增/更新模式 |
| `delete_agent_mode` | `(app: AppHandle, id: String) → Result<(), String>` | 删除模式（内置不可删） |
| `get_conversation_token_usage` | `(app: AppHandle, conversation_id: String) → Result<Value, String>` | 对话 token 统计 |
| `send_chat_message` | 新增 `mode_id: Option<String>` 参数 | 发送消息时注入模式配置 |

### 3.4 db.rs 变更

- `init_db` 新增 `agent_modes` 表 CREATE + `conversations.mode_id` 列 ALTER
- seed 调用 `agent_modes::seed_builtin_modes`

### 3.5 cmd.rs 变更

- `send_chat_message` 新增 `mode_id` 参数，读取模式配置注入 system_prompt / temperature / max_tokens
- 新增 `list_agent_modes` / `save_agent_mode` / `delete_agent_mode` / `get_conversation_token_usage`
- 删除 `list_prompt_templates` / `save_prompt_template` command

## 4. 前端规格

### 4.1 新增文件

| 文件 | 说明 |
|------|------|
| `src/stores/agentModeStore.ts` | Agent 模式 Zustand store |
| `src/settings/AgentModeManager.tsx` | Agent 模式管理页 |
| `src/chat/ModeSelector.tsx` | Orb 面板模式选择器 |
| `src/chat/TokenUsageBar.tsx` | token 用量指示条 |

### 4.2 agentModeStore.ts

```typescript
interface AgentModeState {
  modes: AgentMode[];
  selectedModeId: string | null;
  loading: boolean;
  loadModes: () => Promise<void>;
  selectMode: (modeId: string | null) => void;
  saveMode: (mode: Partial<AgentMode>) => Promise<void>;
  deleteMode: (id: string) => Promise<void>;
}
```

### 4.3 chatStore.ts 变更

- 新增 `selectedModeId` 字段，初始 null
- `sendMessage` 传递 `modeId: get().selectedModeId`
- 新增 `tokenUsage` 字段（TokenUsageSummary）
- 流式结束时更新 `tokenUsage`

### 4.4 路由变更

| 变更 | 说明 |
|------|------|
| 新增 `/settings/agents` | AgentModeManager |
| 删除 `/settings/templates` | 不保留重定向 |
| 侧边栏"Prompt 模板" → "Agent 模式" | 菜单项更新 |

### 4.5 TokenUsageBar 组件

```
[1.2k / 128k]  ← 正常（灰色）
[89k / 128k]   ← 警告（黄色，>70%）
[118k / 128k]  ← 危险（红色，>90%）
```

- 位置：ChatView 底部 ContextBar（已有组件，接入数据源）
- 数据：chatStore.tokenUsage
- 刷新时机：流式输出结束时

## 5. 参考项目

| 功能点 | 参考项目 | 参考文件 | 借鉴内容 |
|--------|---------|---------|---------|
| 内置 Agent 模式 | OpenBitFun | crates/.../agent-runtime/src/agents.rs | `mode_presentation_rank`：Standard=0, Cowork=1, DeepResearch=2, Ultimate=3, Creative=4，排序展示 |
| 自定义 Agent 定义 | OpenBitFun | crates/.../agent-runtime/src/custom_agent.rs | `CustomAgentDefinition { id, name, description, kind, tools, prompt, readonly, review, level, model }` |
| 前端 Agent 管理 | OpenBitFun | web-ui/src/app/scenes/agents/agentsStore.ts | `AgentFilterLevel`（all/builtin/user/project/external）+ 创建/编辑页面 |
| 模式配置 profile | OpenBitFun | agents.rs L22-27 | `STANDARD_HARNESS_CONFIG`：mode 可组合为 profile |
| token 追踪 | OpenBitFun | flow_chat 相关 | 流式输出 usage 字段消费 + 上下文百分比 |

## 6. 不做的事（I6 范围外）

- ❌ Function Calling / 内置工具 / MCP 客户端（后续迭代，参考 OpenBitFun `mcp_tool_bridge.rs`）
- ❌ Agent 模式 profile 组合 / 子代理（后续迭代）
- ❌ 变量占位符 `{{variable}}`（已移除，不再恢复）
- ❌ tools 字段实际消费（当前工具系统未实现，tools 仅存储）

## 7. 验收标准

1. `agent_modes` 表创建成功，4 个内置预设自动 seed
2. `conversations` 表新增 `mode_id` 列
3. 可列出所有 Agent 模式（内置 + 自定义），按 presentation_rank 排序
4. 可创建自定义 Agent 模式（name + prompt + temperature + max_tokens + model）
5. 可编辑内置模式的 prompt / temperature / max_tokens
6. 可删除自定义模式，内置模式不可删除
7. Orb 面板模式选择器可切换模式，默认选中"默认"模式
8. 选中"默认/快速/深度"模式时，不注入 system prompt，仅使用对应的 temperature / max_tokens
9. 选中"翻译助手"时，system prompt 注入对话
10. 不选模式时发送消息，使用默认值（temperature=0.7, max_tokens=4096, 无 system prompt）
11. `/settings/agents` 页面可管理所有模式
12. `/settings/templates` 路由已删除
13. 侧边栏显示"Agent 模式"而非"Prompt 模板"
14. token 用量在 Orb 面板底部 ContextBar 显示，超 70% 黄色警告，超 90% 红色警告
15. `prompt_templates` 表已 DROP，`templates.rs` 已删除
16. 编译通过：`cargo check` + `tsc --noEmit` + `npm run lint` 0 errors
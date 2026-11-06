# I38c — P5：OS 工具能力规格

| 项 | 值 |
|---|---|
| 状态 | 现行（P5 规格；进度见 [roadmap](./I38-agent-roadmap.md) §2） |
| 日期 | 2026-11-04 |
| 阶段 | P5（见 [I38 roadmap](./I38-agent-roadmap.md)） |
| 关联 | [I18 spec](./I18-agent-tool-architecture-spec.md) Step 2（已迁入本文）、[I31](./I31-phase1-tool-framework-spec.md) §5（共享路径）、[OSCALLDESIGN](../designed/OSCALLDESIGN.md)（Shell 洋葱）、[I38a](./I38a-tool-control-layer-spec.md)（Policy/Gateway/Pipeline）、[I18b](./I18b-tool-unified-bridge-spec.md)（ToolBox 桥接） |
| 影响范围 | `assistant-tools/src/os/*`；`src/tools/os.ts` 与 ToolBox 组件按 I18b 的 5 步流程 |

> 本文是 P5 的唯一权威规格。I18 spec 的 Step 2 工具清单与 Schema 已迁入本文。
> **门禁**：P1（`os/path.rs`）与 P2（Policy/Gateway）未完成前，不得新增任何破坏性工具，也不得把 `shell` / `delete` 加入生产注册表。

> 执行清单：[I38c-os-tools-tasklist.md](./I38c-os-tools-tasklist.md)（C0–C8 逐项勾选与验收映射）。任务级进度只在该清单维护。

---

## 1 目标与非目标

### 1.1 目标

| # | 交付 |
|---|---|
| 1 | 只读工具：`read` / `ls` / `glob` / `grep` |
| 2 | 修改工具：`write` / `edit` / `mkdir` / `move` |
| 3 | 破坏性工具：`delete` |
| 4 | 命令执行：`shell` + 执行层洋葱守卫 |
| 5 | 输出截断基础设施 `os/output.rs`（第一个需要截断的工具落地时建） |
| 6 | 每个工具显式声明 `audience()`，并复用 `os/path.rs` 与 `exec::invoke` |
| 7 | **白名单工具解析** `resolve_available_tools`：替换 P4 的 `RegistryToolCatalog`（tier + audience 自动全量注入），改为 deny-by-default + 显式白名单/黑名单 |

### 1.2 非目标

- 控制层（P2 范围）；RoundExecutor（P4 范围，规格见 [I38b](./I38b-agent-loop-spec.md)）。阶段状态见 [roadmap](./I38-agent-roadmap.md) §2。
- ToolBox 前端组件与审批 UI（P6）；本阶段只要求 Rust 侧可被 `os_tool_exec` 与 Pipeline 调用。
- `ask_user` 的完整交互：依赖 P6 输入框，见 §9。
- MCP（P7）。
- 文件内容语义摘要（I36 design §3.19 未采纳）。
- 白名单配置的前端 UI（AgentMode 编辑表单、对话工具栏、`ConversationToolConfig` 内存态）归 P6（见 [I38d §4](./I38d-agent-frontend-modes-spec.md)）；P5 只做后端 `resolve_available_tools` 与 `send_agent_message` 接入。

### 1.3 前置

1. `assistant-tools/src/os/path.rs` 已落地（I31 T3），所有路径工具只允许通过它解析。
2. `exec::invoke` 已统一超时与错误归一化（I31 T2）。
3. P2 的 `validate_input` / `BehaviorHints` / `permission_intents` 已可用。
4. 注册分组：`os/mod.rs` 提供只读 / 修改 / 危险三组；`setup.rs` 只注册已确认可暴露的分组（I31 §8）。

---

## 2 工具清单与安全标记

| 工具 | action | read_only | destructive | open_world | audience |
|---|---|---|---|---|---|
| `read` | read | 是 | 否 | 否 | BOTH |
| `ls` | read | 是 | 否 | 否 | BOTH |
| `glob` | read | 是 | 否 | 否 | BOTH |
| `grep` | read | 是 | 否 | 否 | BOTH |
| `write` | edit | 否 | 否 | 否 | BOTH |
| `edit` | edit | 否 | 否 | 否 | BOTH |
| `mkdir` | edit | 否 | 否 | 否 | BOTH |
| `move` | edit | 否 | 否 | 否 | BOTH |
| `delete` | delete | 否 | 是 | 否 | BOTH |
| `shell` | execute | 否 | 是 | 是 | BOTH（守卫齐备后；否则 AGENT_ONLY） |
| `ask_user` | — | 是 | 否 | 否 | AGENT_ONLY |
| `fs_link` | — | 实际 `read_only=false` | 否 | 是 | TOOLBOX_ONLY |

**命名规范**：文件名是代码维护名，`Tool::name()` 是注册名（模型与 `os_tool_exec` 看到的唯一标识），两者刻意分离。

`agent_` 前缀是**受众标记**，不是通用目录前缀：只有 `audience().agent == true` 的工具（`BOTH` / `AGENT_ONLY`）才用 `agent_`，一眼即可看出哪些文件会被注入模型；`TOOLBOX_ONLY` 工具与 Agent 无关，不套该前缀。注册名保持纯动作词，模型侧不出现领域前缀。

| 文件（`assistant-tools/src/os/`） | 注册名（`Tool::name()`） | 旧名 |
|---|---|---|
| `agent_read.rs` | `read` | `fs_read` |
| `agent_list.rs` | `ls` | `fs_list` |
| `agent_glob.rs` | `glob` | `fs_glob` |
| `agent_grep.rs` | `grep` | `fs_grep` |
| `agent_write.rs` | `write` | `fs_write` |
| `agent_edit.rs` | `edit` | `fs_edit` |
| `agent_mkdir.rs` | `mkdir` | `fs_mkdir` |
| `agent_move.rs` | `move` | `fs_move` |
| `agent_delete.rs` | `delete` | `fs_delete` |
| `agent_shell.rs` | `shell` | `shell_exec` |
| `agent_ask_user.rs` | `ask_user` | `ask_user` |

1. 注册名一律小写 snake_case、字母开头、≤64 字符，字符集 `[a-z0-9_]`；**禁用 kebab**（Gemini 方言不接受 `-`）。
2. 文件名与注册名解耦：改注册名只影响 `Tool::name()` 与调用方（前端 `os_tool_exec` 的 `tool` 参数），不影响文件组织。
3. `os_tool_exec` 的 `tool` 参数、AgentMode 的 `enabledTools` / `disabledTools` 白名单一律填**注册名**（`read`、`write` …），不填文件名。前端 `os.ts` 命名空间是 TS 本地写法：注册名单词时与注册名同名（`os.read`），多词时驼峰（`os.askUser`），不影响 wire（见 [I18b](./I18b-tool-unified-bridge-spec.md) §2）。
4. `fs_link` 是存量 `TOOLBOX_ONLY` 工具（`audience().agent == false`），**保留 `fs_link.rs` 文件名与 `fs_link` 注册名，不改名、不参与本表**——它不进入模型工具集，`agent_` 前缀对它没有意义。
5. 本文各工具小节标题一律写**注册名**；实现路径见 §3 与[执行清单](./I38c-os-tools-tasklist.md)。

**硬约束**：

1. 新增工具**必须显式声明受众**。默认值是 `NONE`（fail-closed），遗漏声明时工具不会出现在任何入口（I31 §4.2.1）。
2. 只读工具的 `permission_intents` 返回空 `Vec`；修改/破坏性工具按 §3 词汇表声明。
3. `fs_link` 的受众为 `TOOLBOX_ONLY`，不进入 LLM 工具列表；其 Path/permission 修正属于 P1（I31 T3）。
4. 任务级进度只在[执行清单](./I38c-os-tools-tasklist.md)勾选；阶段级结论只在 [roadmap](./I38-agent-roadmap.md) §2 写一次（roadmap §7）。

---

## 3 实现顺序

```
只读组：read → ls → glob → grep
修改组：write → edit → mkdir → move
危险组：delete → shell
特殊：  os/output.rs（第一个需要截断的工具 = read）
```

**注册分批**：

| 批次 | 可注册内容 | 条件 |
|---|---|---|
| P5-R1 只读组 | `read` / `ls` / `glob` / `grep` | P1 完成；P2 的 validate 已就绪 |
| P5-R2 修改组 | `write` / `edit` / `mkdir` / `move` | P2 的 Gateway 审批可用 |
| P5-R3 危险组 | `delete` / `shell` | P2 完整；前端组件同期上线（P6），不得先行注册 |

`os/mod.rs` 以 `register_readonly_tools` / `register_write_tools` / `register_dangerous_tools` 分组；`setup.rs` 只调用已确认的组。

### 3.1 白名单工具解析（接入层替换）

P4 的 `RegistryToolCatalog` 使用 `tool_definitions_for_tier(MAX_TIER)` 做自动全量注入——只要工具注册了、tier 不超上限、audience 是 agent，就全部暴露给模型。这套方案没有白名单/黑名单，也没有 deny-by-default。

P5 在工具实现完成后，新增 `resolve_available_tools` 替换 P4 的接入逻辑，同时删除 `RegistryToolCatalog`。

**函数签名**（落点 `src-tauri/src/adapters/tool_resolver.rs`）：

```rust
pub fn resolve_available_tools(
    registry: &ToolRegistry,
    enabled_tools: &[String],
    disabled_tools: &[String],
    max_tier: ToolTier,
) -> Vec<ToolSpec> {
    // 1. 层级硬上限：tier <= max_tier
    // 2. 只保留 audience().agent == true
    // 3. deny-by-default：enabled_tools 为空 → 空集
    // 4. 白名单过滤：tool.name() in enabled_tools
    // 5. disabled_tools 优先于 enabled_tools
}
```

**规则**：

| 步 | 规则 | 说明 |
|---|---|---|
| 1 | 层级硬上限 | `tool.tier() <= max_tier`；`External` 在 P5 恒不注入 |
| 2 | 受众过滤 | `tool.audience().agent == true`；`TOOLBOX_ONLY` 不出现在此 |
| 3 | deny-by-default | `enabled_tools` 为空数组 → 返回空 `Vec`，模型看不到任何工具 |
| 4 | 白名单匹配 | 仅 `enabled_tools` 中出现的注册名通过 |
| 5 | 黑名单覆盖 | 同一工具同时出现在两个列表 → 禁用；`disabled_tools` 是最终否决 |

**与 P4 的对比**：

| | P4 `RegistryToolCatalog` | P5 `resolve_available_tools` |
|---|---|---|
| 过滤方式 | `tier + audience` 自动 | `tier + audience + enabled_tools + disabled_tools` |
| 默认行为 | 注册了就可见 | deny-by-default，空白名单 = 无工具 |
| 配置来源 | 无（硬编码 `MAX_TIER = Os`） | `SendChatMessageArgsDto.tool_config`（前端内存态，每次请求携带） |
| `ToolCatalog` trait | 调用 `agent_tools()` | **不再使用 `ToolCatalog` trait**，改为同步函数直接调用 |

**`send_agent_message` 接入**：

```rust
// P4（删除）:
spec.tools = Some(Arc::new(RegistryToolCatalog::new(registry)));

// P5（替换为）:
let tool_config = args.tool_config.unwrap_or_default();
let tools = resolve_available_tools(
    &registry,
    &tool_config.enabled_tools,
    &tool_config.disabled_tools,
    tool_config.max_tool_tier,
);
spec.tools = if tools.is_empty() { None } else { Some(tools) };
```

**清理范围**（P5 完成时删除）：

| 删除项 | 位置 |
|---|---|
| `RegistryToolCatalog` 结构体与 impl | `src-tauri/src/adapters/tool_catalog.rs`（整文件） |
| `mod tool_catalog;` 导出 | `src-tauri/src/adapters/mod.rs` |
| `ToolCatalog` async trait（若无其他实现者） | `assistant-agent/src/kernel/ports.rs` |
| `TurnSpec.tools` 类型由 `Option<Arc<dyn ToolCatalog>>` 改为 `Option<Vec<ToolSpec>>` | `assistant-agent/src/kernel/mod.rs`、`runner.rs`、`request.rs` |
| P4 的 `test_doubles.rs` 中 `ToolCatalog` 替身 | `assistant-agent/src/kernel/ports/test_doubles.rs` |

**保留不动**：

- `ToolSpec` 类型（`name` / `description` / `input_schema`）——P4 C2.1 定义，白名单解析仍产出它
- 三个 provider adapter 的 `ToolSpec` → 方言转换（OpenAI / Anthropic / Gemini）——P4 C2.10–C2.16
- `ChatOptions.tools` 字段——P4 C2.2，仍由 `TurnSpec.tools` 填入
- RoundExecutor 的 `ToolExecutionPort`——P4 C3.6，与工具注入无关

**DTO 扩展**（P5 新增）：

`SendChatMessageArgsDto` 新增 `tool_config: Option<ConversationToolConfigDto>`：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationToolConfigDto {
    pub max_tool_tier: ToolTier,
    pub approval_level: ApprovalLevel,
    pub enabled_tools: Vec<String>,
    pub disabled_tools: Vec<String>,
}
```

- `Option` 输入字段加 `#[ts(optional = nullable)]` + `#[serde(default)]`。
- `ToolTier` / `ApprovalLevel` 复用 P6 的类型定义（P5 先定义，P6 不重复）。
- 执行 `npm run contract:gen` 生成前端类型。

**P6 的关系**：P5 只做后端解析与 `send_agent_message` 接线；前端 UI（AgentMode 编辑表单、对话工具栏、`ConversationToolConfig` 内存态初始化与传递）仍在 P6（见 [I38d §4](./I38d-agent-frontend-modes-spec.md)）。P6 的 `resolveAvailableTools`（I38d §4.5）后端部分已由 P5 完成，P6 只做前端调用与 UI 绑定。

---

## 4 每个工具的共同结构

```rust
pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str { "read" }
    async fn description(&self) -> Result<String, String>;
    fn input_schema(&self) -> Value;
    fn tier(&self) -> ToolTier { ToolTier::Os }
    fn audience(&self) -> ToolAudience { ToolAudience::BOTH }
    fn behavior_hints(&self) -> BehaviorHints;
    fn validate_input(&self, input: &Value, ctx: &ToolUseContext) -> ValidationResult;
    fn permission_intents(&self, input: &Value, ctx: &ToolUseContext) -> Result<Vec<PermissionIntent>, String>;
    async fn execute(&self, input: &Value, ctx: &ToolUseContext) -> Result<ToolResult, String>;
}
```

**共享基础设施**（定义与算法见 I31 §5，不在此重复）：

- `resolve()` / `resolve_leaf_preserving()` / `canonicalize_lenient()` / `reject_system_dirs()` / `ensure_within_root()`。
- `execute()` 内**只能**使用 `resolve()` 的返回值做 IO，禁止「校验 A、使用 B」。
- 所有 `Err` 先由 `exec::invoke` 归一化；工具内部只返回业务失败（`ToolResult::err`）。

**权限词汇表**（与 P2 一致）：

| action | 语义 | 资源 |
|---|---|---|
| `read` | 读取内容 | 目标路径 |
| `edit` | 创建 / 修改 / 移动 | 每个受影响路径各一条 |
| `delete` | 删除 | 目标路径 |
| `execute` | 执行命令 | 命令基名 |

---

## 5 只读工具

### 5.1 read

```json
{
  "type": "object",
  "properties": {
    "path": { "type": "string", "description": "File path to read" },
    "offset": { "type": "integer", "description": "Start line (1-based)" },
    "limit": { "type": "integer", "description": "Max lines to read" }
  },
  "required": ["path"]
}
```

**行为**：

- 默认读全部内容，最大 1MB / 2000 行。
- `offset` + `limit` 支持行范围读取；超过限制返回截断提示 + 实际行数。
- 二进制检测：前 8KB 含 NULL 字节 → 拒绝，提示用 hex 工具。
- **读取回执**：同一会话内对同一路径 + 同行范围 + 文件修订号未变 → 返回「已在上文返回」，不回传内容；修订号变化则重读（`ReadReceiptStore` 端口，P2 已定义）。

### 5.2 ls

```json
{
  "type": "object",
  "properties": {
    "path": { "type": "string", "description": "Directory path to list" },
    "depth": { "type": "integer", "description": "Max recursion depth (default 1)" },
    "all": { "type": "boolean", "description": "Include hidden entries" }
  },
  "required": ["path"]
}
```

默认 `depth = 1`，最大 10；每层最大 1000 条目。输出按名称排序。

### 5.3 glob

```json
{
  "type": "object",
  "properties": {
    "pattern": { "type": "string", "description": "Glob pattern (e.g. '**/*.ts')" },
    "path": { "type": "string", "description": "Base directory" },
    "exclude": { "type": "array", "items": { "type": "string" } }
  },
  "required": ["pattern"]
}
```

- 使用 `glob` crate（或 `wax` 若需要否定模式）；最大返回 1000 条。
- 结果只包含 workspace 内路径；越界由 `ensure_within_root` 统一拒绝。

### 5.4 grep

```json
{
  "type": "object",
  "properties": {
    "pattern": { "type": "string", "description": "Regex pattern" },
    "path": { "type": "string", "description": "Directory to search" },
    "include": { "type": "string", "description": "File glob filter" },
    "context": { "type": "integer", "description": "Lines of context (default 0)" }
  },
  "required": ["pattern"]
}
```

- 使用 ripgrep Rust API（`grep` crate）；最大返回 100 条匹配。
- 输出含文件路径、行号、匹配行；大结果按 P2 的三层预算落盘。

---

## 6 修改工具

### 6.1 write

```json
{
  "type": "object",
  "properties": {
    "path": { "type": "string" },
    "content": { "type": "string" },
    "append": { "type": "boolean", "description": "Append mode (default false)" }
  },
  "required": ["path", "content"]
}
```

- `append = false` 时覆盖已有文件；自动创建父目录。
- 最大写入 10MB；`permission_intents` → `edit` + 目标路径。
- 覆盖已有文件属于修改而非破坏：`destructive = false`，但审批卡片必须展示「覆盖」与目标路径。

### 6.2 edit

```json
{
  "type": "object",
  "properties": {
    "path": { "type": "string" },
    "old_string": { "type": "string", "description": "必须与文件中唯一一段内容完全匹配（含缩进）" },
    "new_string": { "type": "string", "description": "替换后的内容，可为空以删除该段" },
    "replace_all": { "type": "boolean", "description": "允许替换全部匹配（默认 false）" }
  },
  "required": ["path", "old_string", "new_string"]
}
```

| 情况 | 行为 |
|---|---|
| 0 匹配 | 失败，`error_kind = Business`，提示先 `read` 确认内容 |
| 1 匹配 | 替换并写回，返回起始行与 diff |
| 多匹配且 `replace_all = false` | 失败，回传匹配次数与首个匹配行号 |
| 多匹配且 `replace_all = true` | 全部替换，返回替换次数 |

**审批展示**：`edit` 的审批卡片必须展示 diff（新增 / 删除行数 + 片段），不能只展示路径。

### 6.3 mkdir

```json
{
  "type": "object",
  "properties": {
    "path": { "type": "string" },
    "parents": { "type": "boolean", "description": "Create parent directories (default true)" }
  },
  "required": ["path"]
}
```

### 6.4 move

```json
{
  "type": "object",
  "properties": {
    "source": { "type": "string" },
    "target": { "type": "string" },
    "overwrite": { "type": "boolean", "description": "Allow overwrite (default false)" }
  },
  "required": ["source", "target"]
}
```

- 两个路径各产生一条 `edit` intent。
- 目标已存在且 `overwrite = false` → 失败（`Business`）。

---

## 7 破坏性工具

### 7.1 delete

```json
{
  "type": "object",
  "properties": {
    "path": { "type": "string" },
    "recursive": { "type": "boolean", "description": "Delete directory recursively" }
  },
  "required": ["path"]
}
```

- `destructive = true`，必须经 Gateway 审批。
- 拒绝删除 `workspace_root` 本身；`recursive = true` 时删除目录及内容。
- 审批键为规范化绝对路径；批准一次只对该路径生效。

### 7.2 shell

```json
{
  "type": "object",
  "properties": {
    "command": { "type": "string", "description": "Shell command to execute" },
    "timeout": { "type": "integer", "description": "Timeout in seconds (default 30)" },
    "cwd": { "type": "string", "description": "Working directory" }
  },
  "required": ["command"]
}
```

**守卫分层**（归属见 OSCALLDESIGN §3.3-§3.4，本文只声明不变量）：

| 层 | 内容 | 归属 |
|---|---|---|
| 执行层（`execute()` 内部，两入口共享） | SafetyBelt、结构守卫、环境隔离、命令分类、heredoc 处理、BareExec | `assistant-tools` |
| 校验层 | `validate_input` 复用 SafetyBelt 判定，命中 `blocks_input_rewrite = true` | `assistant-tools` |
| 审批层 | AllowlistGate / ApprovalGate | P2 Pipeline（仅 Agent 入口） |

**实现不变量**：

1. `execute()` 必须包含 SafetyBelt 与全部纵深防御；**不得**假设「Pipeline 会拦」。
2. 子进程超时**不能**依赖 `tokio::time::timeout`；必须进程级超时 + kill（Windows 用 Job Object / `TerminateProcess`）。
3. 环境隔离：`env_clear()` + `SAFE_ENV_VARS` 白名单。
4. 结构守卫：阻止 `$()` / 反引号 / `&` / `bash -c "..."`（详见 OSCALLDESIGN）。
5. 默认超时 30s；用户可提高，但受 turn 剩余预算约束。

> 受众为 `BOTH` 的前提是上述执行层守卫全部在 `execute()` 内；任一缺失则改为 `AGENT_ONLY`（I18b §2.2）。

### 7.3 危险组注册门禁

`delete` / `shell` 满足以下全部条件前**不得**加入生产注册表：

1. P2 的 Policy / Gateway / 审批端口可用。
2. 前端审批卡片已上线（P6）。
3. `os/mod.rs` 的 `register_dangerous_tools` 由 `setup.rs` 显式调用，并有 code review 记录。

---

## 8 输出截断

`os/output.rs` 在第一个需要截断的工具（`read`）落地时创建：

```rust
pub struct OutputLimits {
    pub max_bytes: usize,
    pub max_lines: usize,
    pub preview_bytes: usize,
}

pub fn truncate_output(text: &str, limits: &OutputLimits) -> TruncatedOutput;
```

- 截断必须显式告知模型「已截断 + 实际行数 / 字节数」，不得静默丢弃。
- 大结果落盘由 P2 的 `ToolResultStorage` 负责，`os/output.rs` 只做单条上限。

---

## 9 ask_user（P6 联调）

```json
{
  "type": "object",
  "properties": {
    "question": { "type": "string", "description": "Question to ask the user" }
  },
  "required": ["question"]
}
```

- 语义：暂停 Agent Loop，等待前端回答，再继续。
- 依赖 P2 的审批端口与 P6 输入框；**不属于 P5 退出条件**。
- `audience = AGENT_ONLY`（不从前端透传入口暴露）。
- 超时与取消：超时按用户未回答处理（返回 `Cancelled` 或业务错误，不得默认继续）；取消走 turn 子令牌。

---

## 10 验收标准

| # | 标准 |
|---|---|
| A1 | 所有文件工具只通过 `os::path` 解析路径；crate 内 grep 无 `unwrap_or_else(\|_\| path.to_path_buf())` |
| A2 | 每个工具都有 `input_schema` 单测（合法 / 缺必填 / 类型错误） |
| A3 | 只读工具 `permission_intents` 为空、`is_concurrency_safe = true`；修改/删除工具产出规定的 action |
| A4 | `edit` 的 0 / 1 / 多匹配三分支行为符合 §6.2；审批载荷含 diff |
| A5 | `delete` 拒绝删除 workspace_root；批准只对规范化路径生效 |
| A6 | `shell` 在 `execute()` 内命中 SafetyBelt / 结构守卫 / 环境隔离；子进程超时能真正 kill |
| A7 | 分组注册：默认 `setup.rs` 只注册 P5-R1；危险组未调用时生产注册表不包含 `shell` / `delete` |
| A8 | `read` 的读取回执：修订号不变 → 复用提示；变更 → 重读 |
| A9 | `cargo test -p assistant-tools` 全绿；`cargo clippy -- -D warnings` 无告警 |

---

## 11 风险与开放问题

| 风险 | 缓解 |
|---|---|
| 工具绕过 `os/path.rs` 自行实现 | A1 grep；code review |
| 破坏性工具提前注册即前端可达 | 分组注册 + §7.3 门禁；I31 §8 |
| 同步 IO 超时无法中断 | `spawn_blocking`；shell 用进程级超时（I31 §4.3） |
| 大结果挤爆上下文 | P2 三层预算 + `os/output.rs` 单条上限 |
| Windows 路径形态（UNC / 盘符相对） | I31 §5.2.1 全覆盖；A1 只允许走共享函数 |

**开放问题**：

1. `grep` 是否引入 `ignore` crate 以遵守 `.gitignore`？当前倾向：引入，避免搜索结果被构建产物淹没。
2. `write` 的「覆盖」是否应标记 `destructive`？当前倾向：否，但审批卡片必须显示覆盖提示。
3. `shell` 的 Job Object 实现是否需要 `windows-sys` 新依赖？在 P5-C3 评估，避免仅为此引入。

---

## 12 参考

| 工具/要素 | 参考项目 | 参考实现 |
|---|---|---|
| Read / LS / Glob / Grep | OpenBitFun | `agentic/tools/implementations/` |
| Write / Edit / Delete | OpenBitFun / codex | `file_write` / `file_edit`；`apply_patch` |
| Bash / shell 守卫 | OpenBitFun | `BashTool` + `edit_constraint_guard` |
| AskUser | OpenBitFun | `AskUserQuestionTool` + `UserQuestionController` |
| 文档转换 / 行限制 | OpenBitFun | `FileReadTool` 三级限制 |
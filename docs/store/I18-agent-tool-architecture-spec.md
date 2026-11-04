# I18 — Agent Tool 架构实施规格

> **本文档职责**：从底层到顶层的**实施规格**——每步讲清：建什么、为什么先建、建完能干什么、类型契约、行为约束。是类型定义、流程算法、接口签名的**唯一权威源**。
> **架构决策记录**（为什么这样设计、参考了什么、做了什么取舍）见 [I18-agent-tool-architecture-design.md](./I18-agent-tool-architecture-design.md)。
> **Shell 执行层安全架构**（洋葱模型、层间契约、委托约束）见 [OSCALLDESIGN.md](../designed/OSCALLDESIGN.md)。
> **ToolBox 双入口桥接**（os_tool_exec、os.ts、新增工具 5 步流程）见 [I18b-tool-unified-bridge-spec.md](./I18b-tool-unified-bridge-spec.md)。

---

## 搭建总览

```
Step 0  值类型              ToolTier · BehaviorHints · ToolResult · PermissionIntent · ToolUseContext
Step 1  Tool Trait          统一工具抽象接口
Step 2  工具实现            L1 OS 工具 (fs_read → fs_list → fs_glob → fs_grep → fs_write → fs_mkdir → fs_move → fs_link → fs_delete → shell_exec → ask_user)  ⚠️ 阶段一暂缓，仅 fs_link 已实现
Step 3  ToolRegistry        注册 · 查找 · 层级过滤
Step 4  Permission Policy   PermissionEffect · PermissionRule · ResolvedPermissionPolicy · Ceiling
Step 5  Tool Gateway        策略评估 · 记忆授权 · 用户审批 · 审计日志
Step 6  Tool Pipeline       查找→评估→审批→执行→持久化→事件
Step 7  RoundExecutor       Agent Loop：消费→提取→执行→回填→继续
Step 8  Tauri 桥接          os_tool_exec · os.ts · 双入口分流
Step 9  前端集成            ChatDelta 扩展 · ToolCallBubble 五态 · 审批 UI
Step 10 Agent 模式集成      max_tool_tier · approval_level · Policy 映射
```

### 三层性质

| 性质 | Steps | 说明 |
|------|-------|------|
| **词汇层** | 0-1 | 定义"工具长什么样"、"结果长什么样"。零行为，纯类型。没有这些，上面写不出类型签名 |
| **执行层** | 2 | 真正干活的代码。每个 `Tool.execute()` 是唯一有 IO 副作用的地方。内含安全约束（validate_path / 大小限制等）。没有这些，上面全是空壳 |
| **控制层** | 3-7 | 不干活，但决定"谁可以干"、"按什么顺序干"。没有这些，工具只能被用户手动调（ToolBox 入口）；有了这些，工具能被 LLM 自主调（Agent 入口） |
| **连接层** | 8-10 | 把控制层接到前端、接到配置。没有这些，后端逻辑无法触达用户 |

### 搭建顺序原则：先安全框架再加能力

如果先加 OS 工具再建安全框架，LLM 一旦调 `shell_exec("rm -rf /")` 就直接执行——没有任何拦截。

所以 **Step 4-7（控制层）先于 Step 2 的后续扩展**：先用 fs_link 验证端到端，再建安全框架，再安全地加 fs_read/write/shell_exec。

---

## Step 0 — 值类型

> **建什么**：工具体系的基础词汇——层级标记、安全标记、结果格式、权限声明、执行上下文。
> **为什么先建**：上面每一层都引用这些类型。没有 ToolTier 就写不出 Tool Trait，没有 ToolResult 就写不出 execute() 签名。
> **建完能干什么**：可以写 Tool Trait 的签名了。

### 0.1 ToolTier

工具能力层级，**描述执行环境**，不决定安全边界（安全决策在 Step 4 Policy 层）。

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolTier {
    Sandbox,   // 前端沙箱，无 OS 访问，安全由浏览器保证
    Os,        // Rust 后端，有 OS 访问，安全需上层强制
    External,  // 外部进程（MCP），有 OS+网络访问，安全需上层强制
}
```

**PartialOrd 语义**：`Sandbox < Os < External`。Agent 模式声明 `max_tier` 即可限制可用层级——`tool_definitions_for_tier(Os)` 应返回 Sandbox + Os 工具。

**参考**：OpenBitFun 无显式 ToolTier 枚举，但通过 `ToolExposure`（Direct / Deferred）+ `is_readonly()` + `is_concurrency_safe()` 三元组实现等效分层。openhuman 通过 `ToolPolicy` trait 的 `check()` 方法做工具级准入，无层级枚举。我们的 ToolTier 是更简洁的显式分层。

**实施状态**：✅ 已实现，`assistant-tools/src/trait.rs`。

### 0.2 BehaviorHints

三维安全标记，**描述工具对系统的潜在影响**，不决定是否放行（放行在 Step 5 Gateway 层）。

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BehaviorHints {
    pub read_only: bool,      // 不修改任何状态 → 可并发安全执行
    pub destructive: bool,    // 删除/覆盖数据 → 需审批
    pub open_world: bool,     // 网络/不可预测副作用 → 需审批
}
```

**推导规则**（上层消费）：
- `read_only = true` → Step 6 Pipeline 可并发执行（OpenBitFun: `is_concurrency_safe()`）
- `destructive = true` → Step 5 Gateway 触发 Ask 审批
- `open_world = true` → Step 5 Gateway 触发 Ask 审批

**参考**：OpenBitFun 用 `is_readonly()` + `is_concurrency_safe(input)` 两个方法，`is_concurrency_safe` 默认等于 `is_readonly` 但可按 input 覆盖（如 Task 工具单实例时并发安全）。我们的 BehaviorHints 是静态的（不依赖 input），简化但够用。

**实施状态**：✅ 已实现，`assistant-tools/src/trait.rs`。

### 0.3 ToolResult

工具执行结果，Rust↔前端统一序列化格式。

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResult {
    pub ok: bool,
    pub data: Option<Value>,                 // 前端渲染用结构化数据，ok=true 时有值
    pub result_for_assistant: Option<String>,// 回填给模型的精简文本（可缺省，缺省时由 data 推导）
    pub error: Option<String>,               // ok=false 时有值
    pub error_kind: Option<ToolErrorKind>,   // ok=false 时的错误类别
}

pub enum ToolErrorKind { Business, Timeout, Cancelled, Internal, Denied }
```

**构造器契约**：
- `ToolResult::ok(data)` → `{ ok: true, data: Some(data), result_for_assistant: None, error: None, error_kind: None }`
- `ToolResult::ok_with_assistant(data, text)` → 同上，但 `result_for_assistant: Some(text)`
- `ToolResult::err(msg)` → `{ ok: false, data: None, result_for_assistant: None, error: Some(msg), error_kind: Some(Business) }`
- `ToolResult::err_kind(kind, msg)` → 指定错误类别，由 `exec::invoke` 在超时/取消/内部错误场景使用

**双通道语义**：`data` 给前端渲染（行号、覆盖率、诊断、diff 等完整结构），`result_for_assistant` 给模型（精简文本）。缺省时 Pipeline 用 `data` 的紧凑序列化兜底，因此存量工具无需改动即可工作。

**错误类别语义**：

| `error_kind` | 产生位置 | 模型应有的反应 |
|---|---|---|
| `Business` | 工具内部业务失败（路径不存在、参数缺失、未知 action） | 可改参数重试 |
| `Timeout` | `exec::invoke` 超时兜底 | 可重试，但缩小范围或显式提高 timeout |
| `Cancelled` | 用户取消 / 代际切换 | 不重试 |
| `Internal` | 工具返回 `Err` 后被 `invoke` 归一化 | 不重试，向用户报告 |
| `Denied` | Gateway 拒绝或审批拒绝 | 不重试，换方案 |

**OsToolResultDto**：Tauri command 返回类型，与 ToolResult 同构，`From<ToolResult>` 转换。存在原因：Tauri command 需要独立序列化类型。归属 `src-tauri/src/protocol/dto/os_tool.rs`——wire 类型必须落在 `protocol/dto`，`assistant-tools` 不持有 Tauri 契约类型。新增 `resultForAssistant` / `errorKind` 两个字段按 DTO 规则处理：键恒在、值为 `null`，前端声明为 `T | null`，不加 `skip_serializing_if`。

**参考**：OpenBitFun 的 `ToolResult` 更丰富——含 `output`(Value) + `is_error`(bool) + `artifacts`(Vec) + `presentation`(String)，且 `ToolResult::Result { data, result_for_assistant, image_attachments }` 显式分离「给 UI」与「给模型」两条通道。artifacts 支持持久化大结果到文件（>50KB 自动落盘，见 Step 6.4），presentation 控制前端渲染。我们不采纳 `image_attachments`（当前无多模态工具结果）与 `Progress`/`StreamChunk` 流式（见设计 §3.12 取舍）。

**实施状态**：⚠️ 部分实现——`assistant-tools/src/result.rs` 当前只有 `ok/data/error`；`result_for_assistant` 与 `error_kind` 随 Step 6 Pipeline 一并扩展，届时 `exec::invoke` 需按错误来源打标。

### 0.4 PermissionIntent

工具声明的单个权限需求。**纯声明**——工具说"我需要什么"，ToolGateway 决定"允不允许"。

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionIntent {
    pub action: String,          // "read" / "edit" / "delete" / "execute" / "network" / "external"
    pub resources: Vec<String>,  // 路径 glob / 命令名 / URL pattern / "*"
}
```

**参考**：OpenBitFun 的 `PermissionIntent` 含 `action`(String) + `resources`(Vec<String>)，与我们的同构。OpenBitFun 在 Pipeline 中调用 `plan_permission_intents()` 将多个工具的 intents 合并为 `PermissionIntentPlan`，再交 `PermissionRequestManager` 批量审批——支持同轮多工具的权限请求合并展示。我们的单工具审批简化版后续可扩展。

**实施状态**：✅ 已实现，`assistant-tools/src/trait.rs`。

### 0.5 ToolUseContext

工具执行上下文，携带调用方身份与运行时信息。

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUseContext {
    pub workspace_root: Option<String>,
    pub session_id: Option<String>,
    pub tool_call_id: Option<String>,
}
```

**工厂方法**：`ToolUseContext::for_toolbox(workspace_root)` — 构造 ToolBox 入口上下文（无 session_id / tool_call_id）。

**参考**：OpenBitFun 的 `ToolUseContext` 更丰富——含 `session_id` / `channel` / `agent_definition_id` / `custom_data`(HashMap) / `cancellation_token`。openhuman 的 `ToolCallContext` 含 `session_id` / `channel` / `agent_definition_id` / `call_id` / `iteration` / `source`。我们的简化版在 Step 7 RoundExecutor 时需扩展 `cancellation_token` 和 `iteration`。

**实施状态**：✅ 已实现，`assistant-tools/src/context.rs`。

---

## Step 1 — Tool Trait

> **建什么**：统一工具抽象接口，三层工具（Sandbox / OS / MCP）均实现此 trait。
> **为什么先建**：工具实现（Step 2）和 Registry（Step 3）都依赖此 trait。没有它写不出 `Arc<dyn Tool>`。
> **建完能干什么**：可以开始写具体的工具实现了。

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    async fn description(&self) -> Result<String, String>;
    fn input_schema(&self) -> Value;
    fn tier(&self) -> ToolTier;
    fn audience(&self) -> ToolAudience { ToolAudience::BOTH }
    fn behavior_hints(&self) -> BehaviorHints { BehaviorHints::default() }
    fn is_readonly(&self) -> bool { self.behavior_hints().read_only }
    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool { self.is_readonly() }
    fn validate_input(&self, _input: &Value, _context: &ToolUseContext) -> ValidationResult { ValidationResult::ok() }
    fn permission_intents(&self, _input: &Value, _context: &ToolUseContext) -> Result<Vec<PermissionIntent>, String> { Ok(vec![]) }
    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String>;
}
```

### 方法契约

| 方法 | 调用时机 | 副作用 | 说明 |
|------|---------|--------|------|
| `name()` | 注册/查找/日志 | 无 | 全局唯一，Rust snake_case（`fs_link`），前端 camelCase（`os.fsLink`） |
| `description()` | LLM function calling 构建 | 无 | 供 LLM 理解工具用途 |
| `input_schema()` | LLM function calling 构建 | 无 | JSON Schema |
| `tier()` | 层级过滤 | 无 | 返回 ToolTier |
| `behavior_hints()` | 安全评估 | 无 | 三维安全标记，默认全 false |
| `is_readonly()` | 并发决策 | 无 | 默认从 behavior_hints 推导 |
| `is_concurrency_safe(input)` | 并发分批 | 无 | 默认 `is_readonly()`；工具可按 input 覆写（见 6.3） |
| `validate_input(input, context)` | Pipeline 校验阶段 | 无 | 默认通过；不变量违规须标 `blocks_input_rewrite`（见 6.2.1） |
| `permission_intents()` | ToolGateway 评估 | 无 | 声明权限需求，**纯声明不执行 IO**；默认无需求 |
| `execute()` | Pipeline / os_tool_exec | **有** | 唯一有副作用的方法，内含路径校验等安全约束 |

### 关键约束

1. **`permission_intents()` 纯声明**：只描述"我需要什么权限"，不执行任何 IO，不修改任何状态
2. **`execute()` 内含安全约束**：路径校验（`validate_path()`）、大小限制、冲突检测等在 execute() 内部——两个入口（Agent / ToolBox）均受保护，不依赖外层 Pipeline
3. **async_trait 必要性**：Rust 原生 `async fn in trait` 不兼容 `dyn Trait`（无法构建 vtable）

### 参考

**OpenBitFun** 的 `Tool` trait 更丰富——含 `short_description()` / `default_exposure()` / `is_concurrency_safe(input)` / `validate_input()` / `call_impl()` + `call()`（call 包含 hook 拦截，call_impl 是纯业务逻辑）/ `render_result_for_assistant()` / `permission_intents()` 等。核心模式一致：`permission_intents()` 纯声明，`call_impl()` 有副作用。

**openhuman** 的 `ToolPolicy` trait 是独立的策略接口（`check(request) → PolicyDecision`），不混入 Tool trait。工具实现分散在各 domain module（`tools/impl/system/`、`tools/impl/network/`），通过 `orchestrator_tools.rs` 汇总注册。

**实施状态**：✅ 已实现，`assistant-tools/src/trait.rs`。

---

## Step 2 — 工具实现

> **建什么**：真正干活的代码——每个 Tool.execute() 是唯一有 IO 副作用的地方。
> **为什么现在建**：没有工具实现，Registry 是空壳，Pipeline 执行什么？RoundExecutor 驱动什么？
> **建完能干什么**：有了具体工具，可以注册到 Registry、通过 os_tool_exec 调用、验证端到端。

> **实施时序说明（2026-11-02）**：本节工具实现**暂缓**，不在阶段一落地。阶段一只交付工具**框架**（`ToolDefinition`、`invoke` 错误归一化、`os/path.rs` 共享路径安全），详见 `docs/store/I31-phase1-tool-framework-spec.md`。理由是：`ChatOptions` 尚无 `tools` 字段，工具实现后无任何消费方；且 Step 4/5 控制层未就绪前，破坏性工具不应存在。本节保留为 Step 2 的完整目标，等 Step 3-7 稳定后按下方顺序逐个补齐。

### 搭建顺序：从安全到危险

先搭只读工具（最安全），再搭修改工具，再搭破坏性工具，最后搭最复杂的 shell_exec。

```
fs_read ──→ fs_list ──→ fs_glob ──→ fs_grep     只读，可并发，无审批需求
    │
fs_write ──→ fs_edit ──→ fs_mkdir ──→ fs_move ──→ fs_link    修改，需审批
    │
fs_delete                                       破坏性，需审批
    │
shell_exec                                      开放世界，需审批 + 洋葱守卫
ask_user                                        特殊：AI 主动问用户
```

### 工具清单与安全标记

| 工具名 | action | read_only | destructive | open_world | 说明 |
|--------|--------|-----------|-------------|------------|------|
| `fs_read` | read | ✅ | ❌ | ❌ | 读文件，支持行范围/tail，最大 1MB |
| `fs_list` | read | ✅ | ❌ | ❌ | 列目录，限制深度和条目数 |
| `fs_glob` | read | ✅ | ❌ | ❌ | glob 模式搜索文件 |
| `fs_grep` | read | ✅ | ❌ | ❌ | ripgrep 内容搜索 |
| `fs_write` | edit | ❌ | ❌ | ❌ | 写文件（新建/整文件覆盖），路径白名单约束 |
| `fs_edit` | edit | ❌ | ❌ | ❌ | 唯一匹配替换（`old_string`→`new_string`），审批卡片展示 diff |
| `fs_mkdir` | edit | ❌ | ❌ | ❌ | 创建目录（含 parents） |
| `fs_move` | edit | ❌ | ❌ | ❌ | 移动/重命名文件 |
| `fs_link` | edit | ❌ | ❌ | ❌ | 创建/检查链接（symlink/hardlink/junction） |
| `fs_delete` | delete | ❌ | ✅ | ❌ | 删除文件/目录，需审批 |
| `shell_exec` | execute | ❌ | ✅ | ✅ | Shell 命令执行，需审批+超时+洋葱守卫 |
| `ask_user` | — | ✅ | ❌ | ❌ | AI 主动问用户，暂停等待回答 |

> **关于 `fs_link`**：它不在上述「典型 Agent 工具集」之列——参考项目内置工具为 Read / Write / Delete / LS / Glob / Bash / AskUser，不含 Link；建链在主流的 Agent 工具集里归入 shell（`ln -s`、`mklink`）。本仓库的 `fs_link` 是**为验证桥接链路**而先落地的 ToolBox 用户侧工具（服务「人点按钮」），不是给模型的能力。
>
> **受众区分已落地**：`Tool` trait 提供 `audience() -> ToolAudience`（默认 `BOTH`），含两个独立维度——`agent` 决定是否进入 LLM 的 `tools` 列表，`toolbox` 决定是否允许 `os_tool_exec` 按名调用。`fs_link` 声明为 `TOOLBOX_ONLY`，因此 Step 7 的注入链路建成后它**不会**出现在模型的工具列表，同时 ToolBox 入口不受影响。详见 I31 §4.2.1（原 O1 已解决）。
>
> **新增工具的硬约束**：实现 `Tool` 时必须显式声明受众，不得依赖默认值。典型 Agent 工具（`fs_read` … `shell_exec`）为 `BOTH`；仅服务 ToolBox 的为 `TOOLBOX_ONLY`；仅服务 Agent Loop、不应从前端透传入口暴露的为 `AGENT_ONLY`。默认值只是为存量工具提供零破坏迁移，不是新工具的推荐写法。
>
> **`shell_exec` 受众为 `BOTH` 的前提**：它的 SafetyBelt 与全部纵深防御必须在 `execute()` 内部（见 shell_exec 规格的「实现不变量」），而不是依赖 Pipeline。只有满足这一点，ToolBox 直接调用才与 Agent 入口同等级受保护；否则应改为 `AGENT_ONLY`。
>
> **注册暴露约束**：`os_tool_exec` 是按名字透传的通用入口，任何 `toolbox == true` 的工具注册后都会立刻获得一个可从 Webview 控制台调用的入口，**与是否存在前端组件无关**。因此：Step 4/5 就绪前不得把 `shell_exec` 加入生产注册表；`fs_delete` 等破坏性工具须与前端组件同期上线；新增工具时 `os/mod.rs` 应提供分组注册（只读 / 修改 / 危险），`setup.rs` 只注册已确认可暴露的分组。

### 每个工具的内部结构

每个 OS 工具遵循统一模式（以 fs_read 为例）：

```rust
pub struct FsReadTool;

#[async_trait]
impl Tool for FsReadTool {
    fn name(&self) -> &str { "fs_read" }
    async fn description(&self) -> Result<String, String> { Ok("Read file contents".into()) }
    fn input_schema(&self) -> Value { /* JSON Schema */ }
    fn tier(&self) -> ToolTier { ToolTier::Os }
    fn audience(&self) -> ToolAudience { ToolAudience::BOTH }
    fn behavior_hints(&self) -> BehaviorHints {
        BehaviorHints { read_only: true, destructive: false, open_world: false }
    }
    fn permission_intents(&self, input: &Value, _ctx: &ToolUseContext) -> Result<Vec<PermissionIntent>> {
        let path = input.get("path").and_then(Value::as_str).unwrap_or("");
        Ok(vec![PermissionIntent::new("read", path)])
    }
    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String> {
        // 1. 解析参数
        // 2. resolve_path() — 相对路径→绝对路径
        // 3. validate_path() — ..遍历 + 系统目录保护
        // 4. 执行 IO
        // 5. 构造 ToolResult
    }
}
```

### 共享基础设施

所有文件系统工具共享：

**`resolve_path(path_str, context)`**：相对路径 + workspace_root → 绝对路径。无 workspace_root 时拒绝相对路径。

**`validate_path(path)`**：两层校验——
1. `..` 遍历检测：任何 Component::ParentDir 拒绝
2. 系统目录保护：`/etc` `/usr` `/bin` `/sbin` `/System` `/Library` `C:\Windows` `C:\Program Files` 等拒绝

> **实施要求**：上述两个函数必须落在共享模块 `assistant-tools/src/os/path.rs`，不得内联进任一工具。`fs_link` 当前的私有实现存在规范化降级缺陷（目标不存在时 `canonicalize` 失败并回退原路径），具体算法与 Windows 前缀比较规则见 `docs/store/I31-phase1-tool-framework-spec.md` §5.1。**在实现第二个文件系统工具之前必须先完成共享化**。

### fs_read 规格

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
- 默认读全部内容，最大 1MB / 2000 行
- offset + limit 支持行范围读取
- 超过限制返回截断提示 + 实际行数
- 二进制文件检测：前 8KB 含 NULL 字节 → 拒绝，提示用 hex 工具

**参考**：OpenBitFun `FileReadTool` 含 `default_max_lines_to_read` / `max_line_chars` / `max_total_chars` 三级限制，支持 document-read（PDF/DOCX 转 markdown），支持 `render_mode`(Auto/Source/Markdown)。

### fs_list 规格

```json
{
  "type": "object",
  "properties": {
    "path": { "type": "string", "description": "Directory path to list" },
    "depth": { "type": "integer", "description": "Max recursion depth (default 1)" }
  },
  "required": ["path"]
}
```

**行为**：默认 depth=1（仅当前目录），最大 depth=10，每层最大 1000 条目。

**参考**：OpenBitFun `LsTool` 支持 `--all`（显示隐藏文件）和路径解析。

### fs_glob 规格

```json
{
  "type": "object",
  "properties": {
    "pattern": { "type": "string", "description": "Glob pattern (e.g. '**/*.ts')" },
    "path": { "type": "string", "description": "Base directory" }
  },
  "required": ["pattern"]
}
```

**行为**：使用 `glob` crate，最大返回 1000 条路径。

**参考**：OpenBitFun `GlobTool` 使用 `wax::Glob` 支持否定模式（`!node_modules`）。

### fs_grep 规格

```json
{
  "type": "object",
  "properties": {
    "pattern": { "type": "string", "description": "Regex pattern" },
    "path": { "type": "string", "description": "Directory to search" },
    "include": { "type": "string", "description": "File glob filter" }
  },
  "required": ["pattern"]
}
```

**行为**：使用 `grep` crate（ripgrep Rust API），最大返回 100 条匹配。

**参考**：OpenBitFun `GrepTool` 支持 `--include` / `--exclude` glob 过滤 + `--context` 行上下文。

### fs_write 规格

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

**行为**：创建不存在的父目录。append=false 时覆盖已有文件。最大写入 10MB。

**参考**：OpenBitFun `FileWriteTool` 支持 diff-based 写入（`+++ path` / `---` 分隔的多文件写入），支持 plan artifact 诊断。

### fs_edit 规格

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

**行为**：读取文件 → 精确匹配 `old_string`（含缩进与换行）→ 校验匹配次数 → 替换写回。

**匹配约束**：

| 情况 | 行为 |
|---|---|
| 0 匹配 | 失败，`error_kind = Business`，错误信息回传「未找到匹配，建议先 `fs_read` 确认内容」 |
| 1 匹配 | 替换并写回，返回替换位置（起始行）与改动前后 diff |
| 多匹配且 `replace_all = false` | 失败，回传匹配次数与首个匹配行号，提示扩大上下文或显式 `replace_all` |
| 多匹配且 `replace_all = true` | 全部替换，返回替换次数 |

**为什么不做整文件覆盖式编辑**：整文件覆盖要求模型重述全文——长文件易截断、易误删未读部分，且审批卡片无法展示「改了什么」。唯一匹配替换只要求最小上下文，天然产出 diff。

**与 `fs_write` 的分工**：`fs_write` 用于新建文件或确实需要整文件重写；`fs_edit` 是默认编辑手段。

**参考**：codex `apply_patch`（自定义 patch 语法 + `FileChange` 预览 + 审批）与本工具目标相同——让模型表达「最小改动」而非「重述全文」；我们不用 patch 语法，避免额外解析器与格式教学成本。OpenBitFun `file_edit` 的 `old_string`/`new_string` 唯一匹配语义与本工具一致。

**审批展示**：`fs_edit` 的 `permission_intents()` 生成 `edit` + 目标路径，审批卡片必须展示 diff（新增/删除行数 + 片段），而不是只展示路径。

### fs_delete 规格

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

**行为**：`destructive = true`，必须经审批。recursive=true 时删除目录及内容。拒绝删除 workspace_root 本身。

**参考**：OpenBitFun `DeleteFileTool`。

### shell_exec 规格

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

**行为**：`destructive = true, open_world = true`。守卫按归属拆两层：**执行层**（SafetyBelt + 结构守卫 + 环境隔离 + 命令分类 + BareExec）落在 `execute()` 内部、两入口共享；**审批层**（AllowlistGate + ApprovalGate）在 Pipeline 内、仅 Agent 入口。详见 [OSCALLDESIGN.md](../designed/OSCALLDESIGN.md) §3.3–§3.4。

**Shell 守卫分层**：

```
执行层（execute() 内部，ToolBox 与 Agent 都经过）
┌─ SafetyBelt ──────────────────────────┐
│  ┌─ 结构守卫 / 环境隔离 / 命令分类 ──┐  │
│  │  ┌─ BareExec ────────────────┐  │  │
│  │  │  tokio::process::Command   │  │  │
│  │  └────────────────────────────┘  │  │
│  └──────────────────────────────────┘  │
└───────────────────────────────────────┘

审批层（Pipeline 内，仅 Agent 入口）
┌─ ApprovalGate ─────────────────────────┐
│  ┌─ AllowlistGate ──────────────────┐  │
│  └───────────────────────────────────┘  │
└────────────────────────────────────────┘
```

| 层 | 归属 | 职责 | 可配置 | 失败行为 |
|----|------|------|--------|---------|
| SafetyBelt | 执行层 | 致命命令硬拒绝 | 编译期固定 | Deny |
| 结构守卫 / 环境隔离 / 命令分类 | 执行层 | 见下表 | 编译期固定 | Deny |
| AllowlistGate | 审批层 | 常用命令免审批（自动 Approve，不跳过审批） | 用户可配置 | Pass → 下一层 |
| ApprovalGate | 审批层 | 未白名单命令需用户确认 | 用户可配置 | Approve/Deny |
| BareExec | 执行层 | 实际执行 | 无 | 执行结果 |

**实现不变量**：`execute()` 内部必须包含 SafetyBelt 与全部纵深防御，不得假设「Pipeline 会拦」。`validate_input()` 的命令判定必须复用 SafetyBelt 的判定函数，命中时返回 `blocks_input_rewrite = true`。

**BareExec 纵深防御**：

| 防御层 | 作用 |
|--------|------|
| 结构守卫 | 阻止 `$()` / `` `...` `` / `&` / `bash -c "..."` |
| 环境隔离 | `env_clear()` + `SAFE_ENV_VARS` 白名单 |
| 命令分类 | 分词 + 基名提取 → 风险分级（High/Medium/Low） |
| heredoc 处理 | 剥离引号包裹的 heredoc body |

### ask_user 规格

```json
{
  "type": "object",
  "properties": {
    "question": { "type": "string", "description": "Question to ask the user" }
  },
  "required": ["question"]
}
```

**行为**：AI 主动问用户，暂停 Agent Loop 等待回答。通过 ApprovalChannel 机制实现——发射 `AgenticEvent::ToolAwaitingUserInput`，前端渲染输入框，用户提交后继续。

**参考**：OpenBitFun `AskUserQuestionTool` 支持 `UserQuestionController`（超时/取消控制）+ 多层 controller 链（子 Agent 问题冒泡到父 Agent）。

### fs_link 实施状态

✅ 已实现，`assistant-tools/src/os/fs_link.rs`。支持 create/check 两个 action，symlink/hardlink/junction 三种类型。

**与规格偏差**：
- Junction 用 `cmd /c mklink /J` 子进程而非 Win32 API `CreateSymbolicLinkW`（有进程开销但功能正确）
- 无特权检测+自动降级（Windows 无特权时直接报错，不自动降级为 Junction）
- `behavior_hints.open_world = true`（规格为 false，实际更保守）
- `permission_intents` 仍产出 `fs_link_create` / `fs_link_target`，未使用 design §2.1 的 `read` / `edit` 词汇（由 I31 T3 修正）
- `input_schema` 中 `source` 的描述与实现语义相反（`target` 才是必须存在的一侧），待 T3 修正
- hardlink 检测用 Win32 `GetFileInformationByHandle` 查 `nNumberOfLinks`
- junction 目标读取用 Win32 `GetFinalPathNameByHandleW`

### 其他 OS 工具实施状态

❌ `fs_read` / `fs_write` / `fs_delete` / `fs_list` / `fs_glob` / `fs_grep` / `fs_mkdir` / `fs_move` / `shell_exec` / `ask_user` 均未实现。

---

## Step 3 — ToolRegistry

> **建什么**：工具注册中心，全局唯一，所有工具注册到同一 Registry。
> **为什么现在建**：有了 Tool Trait + 具体实现，需要一个地方存 `Arc<dyn Tool>` 并支持查找。
> **建完能干什么**：可以通过 `registry.find("fs_read")` 拿到工具实例并调用。os_tool_exec 已经在用。

```rust
pub struct ToolRegistry {
    tools: DashMap<String, Arc<dyn Tool>>,
}
```

### 操作契约

| 方法 | 签名 | 行为 |
|------|------|------|
| `new()` | `→ ToolRegistry` | 创建空 Registry |
| `register()` | `(Arc<dyn Tool>) → ()` | 以 `tool.name()` 为 key 插入；同名覆盖 |
| `find()` | `(&str) → Option<Arc<dyn Tool>>` | 按 name 精确查找，**全量视图**（管理/诊断用） |
| `find_for_toolbox()` | `(&str) → Option<Arc<dyn Tool>>` | 按 name 查找，仅返回 `audience().toolbox == true` 的工具（`os_tool_exec` 使用） |
| `tool_names()` | `() → Vec<String>` | 返回所有已注册工具名，**全量视图**（当前未排序，仅诊断用） |
| `tool_definitions_for_tier()` | `(ToolTier) → Vec<ToolDefinition>` | 返回 **tier ≤ 参数 且 `audience().agent == true`** 的工具定义列表，按 name 字典序排序 |

### tool_definitions_for_tier 修正

**改造前实现**：`filter(|entry| entry.value().tier() == tier)` — 仅返回精确匹配层级。

**规格要求**：`filter(|entry| entry.value().tier() <= max_tier && entry.value().audience().agent)` — 返回指定层级及以下、且对 Agent 可见的所有工具。

**理由**：Agent 模式声明 `max_tool_tier: Os` 时，应同时获得 Sandbox 工具。Sandbox < Os，Sandbox 工具更安全，没有理由排除。`audience().agent` 过滤则保证 `TOOLBOX_ONLY` 工具（如 `fs_link`）不会因为注册而进入模型上下文。

**参考**：OpenBitFun 通过 `ToolExposure`（Direct / Deferred）+ `GetToolSpec` / `CallDeferredTool` 机制实现延迟加载——Deferred 工具不在初始 manifest 中，LLM 需先调 `GetToolSpec` 获取定义再调 `CallDeferredTool` 执行。我们的 tier 过滤更简单直接。

### 注册时机

```rust
// setup.rs 中
let registry = Arc::new(ToolRegistry::new());
os::register_os_tools(&registry);  // L1 OS 工具
// sandbox::register_sandbox_tools(&registry);  // L0 沙箱工具（待实现）
handle.manage(registry);
```

**实施状态**：✅ 完成，`assistant-tools/src/registry.rs`。`<=` 语义、按名排序、`description()` 降级、`audience` 过滤（`agent` / `toolbox`）均已落地并有单测。

---

## Step 4 — Permission Policy Engine

> **建什么**：纯决策逻辑——给定 (action, resource)，返回 Allow / Ask / Deny。零 IO，零副作用，可独立写单测。
> **为什么现在建**：安全框架的核心。ToolGateway（Step 5）和 Agent 模式（Step 10）都依赖它。先建 Policy 再建 Gateway，Gateway 才有东西可调。
> **建完能干什么**：可以写 `policy.evaluate("read", "/src/main.ts") → Allow` 的单测了。

### 4.1 PermissionEffect

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionEffect {
    Allow,   // 放行
    Ask,     // 需用户审批
    Deny,    // 硬拒绝
}
```

**组合语义**：`most_restrictive` —— Deny > Ask > Allow

```rust
impl PermissionEffect {
    pub const fn most_restrictive(self, other: Self) -> Self {
        match (self, other) {
            (Self::Deny, _) | (_, Self::Deny) => Self::Deny,
            (Self::Ask, _) | (_, Self::Ask) => Self::Ask,
            (Self::Allow, Self::Allow) => Self::Allow,
        }
    }
}
```

**参考**：OpenBitFun `PermissionEffect` 完全同构，`most_restrictive` 语义一致。openhuman 的 `PolicyDecision` 是 `Allow / Deny / RequireApproval` 三值，同构。

### 4.2 PermissionRule

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRule {
    pub action: String,        // "read" / "edit" / "delete" / "execute" / "*"
    pub resource: String,      // 路径 glob / 命令名 / "tier:os" / "*"
    pub effect: PermissionEffect,
}
```

**匹配语义**：
- 同一规则集内：**后匹配优先**（last-match-wins）——宽泛规则在前，具体规则在后覆盖
- 不同规则集之间：取 `most_restrictive`

**参考**：OpenBitFun `PermissionRule` 同构。`PermissionConstraintsLayer` 封装一个规则集，未匹配时默认 Allow，层间取 most_restrictive。

### 4.3 ResolvedPermissionPolicy

```rust
#[derive(Debug, Clone, Default)]
pub struct ResolvedPermissionPolicy {
    rules: Vec<PermissionRule>,                          // 应用级主规则
    constraint_layers: Vec<PermissionConstraintLayer>,    // 项目级 + Agent 模式级
}
```

**评估算法**：

```
evaluate(action, resource):
  result = Allow
  // 主规则（last-match-wins）
  result = result.most_restrictive(eval_rules(rules, action, resource))
  // 各约束层独立评估
  for layer in constraint_layers:
    result = result.most_restrictive(eval_rules(layer.rules, action, resource))
  return result
```

### 4.4 PermissionRuntimeCeiling

委托执行的权限天花板。**禁止携带 Allow 规则**——委托只能收紧不能放宽。

```rust
pub struct PermissionRuntimeCeiling {
    rules: Vec<PermissionRule>,  // 所有 effect != Allow
}

impl PermissionRuntimeCeiling {
    pub fn try_new(rules: Vec<PermissionRule>) -> Result<Self, CeilingValidationError> {
        if let Some(rule) = rules.iter().find(|r| r.effect == PermissionEffect::Allow) {
            return Err(CeilingValidationError { rule: rule.clone() });
        }
        Ok(Self { rules })
    }
}
```

### 4.5 默认规则集

```rust
fn default_global_rules() -> Vec<PermissionRule> {
    vec![
        PermissionRule::new("*", "*", PermissionEffect::Ask),           // 默认全部需审批
        PermissionRule::new("read", "*", PermissionEffect::Allow),      // 文件读取显式 Allow
        PermissionRule::new("read", "/etc/shadow", PermissionEffect::Deny),    // 系统敏感路径 Deny
        PermissionRule::new("read", "/etc/ssh/**", PermissionEffect::Deny),
        PermissionRule::new("edit", "/etc/**", PermissionEffect::Deny),
        PermissionRule::new("edit", "C:/Windows/**", PermissionEffect::Deny),
        PermissionRule::new("edit", "C:/Program Files/**", PermissionEffect::Deny),
    ]
}
```

**设计原则**：默认 Ask + 显式 Allow——新增 action 类型默认需审批，避免意外放行。

### 项目级权限配置

用户可在项目根目录创建 `.assistant/permissions.json`：

```json
{
  "rules": [
    { "action": "read",  "resource": "src/**",    "effect": "allow" },
    { "action": "edit",  "resource": "src/**",    "effect": "allow" },
    { "action": "edit",  "resource": "*.lock",    "effect": "deny"  },
    { "action": "execute", "resource": "npm run *",  "effect": "allow" },
    { "action": "execute", "resource": "git *",      "effect": "allow" },
    { "action": "execute", "resource": "*",          "effect": "ask"   }
  ]
}
```

加载优先级：应用默认 → 项目配置 → Agent 模式配置，取 `most_restrictive`。

**实施状态**：❌ 未实现。需新增 `assistant-tools/src/policy.rs`。

---

## Step 5 — Tool Gateway

> **建什么**：权限网关——连接 Policy（Step 4）和用户审批，是 RoundExecutor 和工具执行之间的安全层。
> **为什么现在建**：Policy 已就位，但"Ask 效果怎么变成用户交互"需要 Gateway 来桥接。
> **建完能干什么**：`gateway.evaluate(tool, input, context, policy)` 可以返回 Allowed / Denied / RejectedByUser。

### 5.1 核心结构

```rust
pub struct ToolGateway {
    policy_engine: Arc<PolicyEngine>,
    approval_channel: ApprovalChannel,
    grant_store: Arc<dyn PermissionGrantStore>,
    audit_store: Arc<dyn PermissionAuditStore>,
    event_queue: Arc<EventQueue>,
}
```

### 5.2 评估流程

```
evaluate(tool, input, context, policy) → GatewayDecision:

  1. 收集权限意图
     intents = tool.permission_intents(input, context)

  2. 逐条评估
     for intent in intents:
       for resource in intent.resources:
         a. 查询 GrantStore（记忆授权）→ 命中则 Allow，跳过
         b. 策略评估 policy.evaluate(action, resource)
            - Allow → 记录审计，继续
            - Ask → 加入 need_approval 列表
            - Deny → 记录审计，立即返回 Denied

  3. 需审批的意图 → 请求用户
     emit AgenticEvent::ToolAwaitingApproval
     decision = approval_channel.wait_for_approval()
     - Approve → 记录审计
     - AlwaysApprove → 写入 GrantStore + 记录审计
     - Deny → 记录审计，返回 RejectedByUser

  4. 全部通过 → Allowed
```

**参考**：OpenBitFun 的 `PermissionRequestManager` + `PendingPermissionReceiver` 实现批量审批——同轮多工具的权限请求合并为一个 `PermissionIntentPlan` 展示给用户，用户可一次性批准/拒绝。我们的单工具审批是简化版。

### 5.3 ApprovalChannel

```rust
pub struct ApprovalChannel {
    pending: TokioMutex<HashMap<String, ApprovalWaiter>>,
    timeout: Duration,
}

pub enum ApprovalDecision {
    Approve,                           // 本次允许
    Deny { reason: Option<String> },   // 拒绝
    AlwaysApprove,                     // 永久允许（写入 GrantStore）
}
```

**超时语义**：审批请求带超时，超时后**自动 Deny**——用户未响应视为拒绝，而非无限挂起或默认放行。

**参考**：OpenBitFun 通过 `PermissionReplySource`（User / AutoApproved / Ceiling）区分审批来源。openhuman 的 `ToolPolicy::check()` 返回 `PolicyDecision`，无独立审批通道——审批在 harness middleware 层处理。

### 5.4 GrantStore / AuditStore

```rust
#[async_trait]
pub trait PermissionGrantStore: Send + Sync {
    async fn is_granted(&self, action: &str, resource: &str) -> bool;
    async fn save_grant(&self, action: &str, resource: &str) -> Result<()>;
}

#[async_trait]
pub trait PermissionAuditStore: Send + Sync {
    async fn log_decision(&self, entry: AuditEntry) -> Result<()>;
}
```

### 5.4.1 ApprovalKey — 审批键规范化

`GrantStore` 的匹配不能以「工具名」为键，否则批准一次 `fs_write` 会放行所有路径写入。键必须是**规范化后的操作指纹**：

```rust
pub struct ApprovalKey {
    pub tool: String,
    pub scope: ApprovalScope,
}

pub enum ApprovalScope {
    Path { canonical: String },                                  // canonicalize + 分隔符/大小写归一
    Command { argv: Vec<String>, cwd: String, sandbox_tier: ToolTier }, // 完整 argv，非前缀
    Endpoint { host: String, method: String },                   // 规范化 host/URL pattern
    Opaque { fingerprint: String },                              // 工具自定义
}
```

**规则**：

| 规则 | 说明 |
|---|---|
| 路径归一 | `canonicalize` 后再比较；符号链接必须解析到真实目标，避免经链接绕过已批准路径 |
| 命令归一 | 分词为完整 argv，`cwd` 与沙箱档位一起入键；**不做命令前缀匹配** |
| 空键不命中 | 无法生成键（如命令解析失败）时直接走审批，不允许命中缓存 |
| 会话级 | 缓存生命周期 = 会话，会话结束即失效；跨会话记忆走 `AlwaysApprove` 持久化 |
| 委托 | 子 Agent 可读父 `GrantStore`，新增批准不写入父库 |

**为什么不做前缀匹配**：批准 `cargo test` 后按前缀命中会顺带放行 `cargo publish`。前缀匹配只能由工具显式声明（如 `ApprovalScope::Opaque`），不能作为默认。

**参考**：codex `with_cached_approval(keys: Vec<K>, fetch)`——`keys` 为空时直接调用审批函数（防误命中）；`ApplyPatchApprovalKey { environment_id, path }`、`UnifiedExecApprovalKey { environment_id, executable, command, cwd, tty, sandbox_permissions }` 把「可执行文件 + 完整命令 + cwd + 沙箱档位」一并入键。

**实施状态**：❌ 未实现，随 Step 5 Gateway 一并落地。

### 5.5 GatewayDecision

```rust
pub enum GatewayDecision {
    Allowed,
    Denied { action: String, resource: String },
    RejectedByUser { reason: Option<String> },
}
```

**实施状态**：❌ 未实现。需新增 `assistant-tools/src/gateway.rs`。

---

## Step 6 — Tool Pipeline

> **建什么**：工具执行编排器——串联 Registry（Step 3）+ Gateway（Step 5）+ 执行（Step 2）+ 持久化 + 事件。
> **为什么现在建**：Gateway 能决策了，但"查找工具 → 评估权限 → 审批 → 执行 → 持久化 → 发事件"的完整编排需要 Pipeline。
> **建完能干什么**：`pipeline.execute(tool_calls, context)` 可以完成一个工具调用的完整生命周期。

### 6.1 核心结构

```rust
pub struct ToolPipeline {
    registry: Arc<ToolRegistry>,
    gateway: Arc<ToolGateway>,
    result_storage: Arc<ToolResultStorage>,
    default_timeout: Duration,
}
```

### 6.2 执行流程

```
execute(tool_calls, context) → Vec<ToolResult>:

  for tool_call in tool_calls:
    1. 查找工具
       tool = registry.find(tool_call.name)
       未找到 → ToolResult::err_kind(Internal, "Unknown tool")

    2. 输入校验（见 6.2.1）
       validation = tool.validate_input(input, context)
       不变量违规 → ToolResult::err_kind(Business, message)，禁止改写重试
       格式错误   → ToolResult::err_kind(Business, message)，允许修正重试

    3. 权限网关评估
       decision = gateway.evaluate(tool, input, context, policy)
       Denied/RejectedByUser → ToolResult::err_kind(Denied, reason)

    4. 执行（带超时 + 取消）
       result = timeout(default_timeout, tool.execute(input, context))
       超时       → ToolResult::err_kind(Timeout, "Tool execution timed out")
       内部 Err   → ToolResult::err_kind(Internal, msg)

    5. 持久化（见 6.4）
       result_storage.persist(tool_call_id, conversation_id, ...)

    6. 发射事件
       event_queue.emit(AgenticEvent::ToolCompleted { ... })

    7. 收集结果
       UI 取 data；回填模型取 result_for_assistant（缺省时用 data 紧凑序列化兜底）
```

### 6.2.1 输入校验：可修正错误与不可改写不变量

`Tool` trait 增加可选方法，默认实现为「通过」（存量工具无需改动）：

```rust
pub struct ValidationResult {
    pub ok: bool,
    pub message: Option<String>,
    pub error_code: Option<String>,
    pub blocks_input_rewrite: bool,   // true = 安全不变量违规，禁止任何上游改写后重试
}

fn validate_input(&self, input: &Value, context: &ToolUseContext) -> ValidationResult {
    ValidationResult::ok()
}
```

| 类别 | 例子 | `blocks_input_rewrite` | Pipeline 行为 |
|---|---|---|---|
| 可修正输入错误 | schema 不匹配、枚举值非法、必填字段缺失 | `false` | 返回错误，允许模型改参数或上游钩子修正后重试 |
| 不可改写不变量 | 路径越出工作区、目标为受保护目录、tier 越界、沙箱档位不足 | `true` | 直接拒绝，且任何自动改写输入都不得绕过 |

**为什么校验要先于权限评估**：明显非法的输入（如缺必填参数）不应弹出审批框——否则用户会为一个根本执行不了的操作做决策。

**仅适用于 Agent 入口**：`validate_input` 是 Pipeline 阶段，ToolBox 入口不经过 Pipeline，因此没有这个阶段。ToolBox 的不变量保护由 `execute()` 内部的 `validate_path()` 等函数保证。两者必须复用同一套判定函数，否则同一条路径在两个入口可能得出不同结论（详见 [I18b](I18b-tool-unified-bridge-spec.md) §2.2）。

**参考**：OpenBitFun `ValidationResult { result, message, error_code, meta }` + `blocks_input_rewrite()`——不变量违规打标后，PreToolUse 钩子不得改写输入，只能终止本次调用。

**实施状态**：❌ 未实现，随 Step 6 Pipeline 落地；`Tool` trait 的新增方法需给默认实现，避免破坏已有工具。

### 6.3 并发编排

只读工具可并发执行，修改/破坏性工具串行：

```
[fs_read_1, fs_read_2]  ← 并发 batch
[fs_write_1]            ← 串行
[fs_grep_1, fs_grep_2]  ← 并发 batch
[shell_exec_1]          ← 串行
```

**并发安全判定**：默认由 `behavior_hints().read_only` 推导，同时保留可覆写点——工具可按 input 动态判定（如 `shell_exec` 的 `ls` 可并发、`rm` 不可）：

```rust
fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
    self.behavior_hints().read_only
}
```

未覆写者行为与静态标记完全一致；覆写者按 input 判定。集中默认值在 trait 层，避免每个工具各写一份。

**参考**：OpenBitFun `partition_tool_batches()` 将工具按 `is_concurrency_safe` 分组——连续的并发安全工具合并为一个 batch 用 `join_all` 执行，遇到非安全工具切分。测试验证：`[read_1, read_2, write_1, grep_1, grep_2, bash_1]` → 4 个 batch（2 并发 + 2 串行）。

### 6.4 结果持久化策略

大结果（>50KB）自动落盘，消息历史中只保留引用 + 预览：

```
ToolResult (小) → 直接嵌入消息历史
ToolResult (大) → 写入文件 → 消息历史中放 <persisted-output> 引用 + 前 2000 字符预览
```

**三层上限**：

| 参数 | 取值 | 作用 |
|---|---|---|
| `per_tool_limit_chars` | 50K | 单个工具结果超过即落盘 |
| `per_round_limit_chars` | 200K | 单轮所有工具结果的总预算 |
| `preview_chars` | 2K | 落盘后回填的预览长度 |

**选择算法**：不是「超阈值就全落盘」。按序列化字符数**降序**排序，依次累加，累计超过 `per_round_limit_chars` 的结果才落盘——优先牺牲最大的几个，保留尽可能多的完整小结果。

**跨轮去重**：同一会话内对同一路径、同一行范围且文件修订号未变的重复读取，返回「已在上文返回」语义（不回传内容，只提示复用上文）；文件修订号变化则回执失效，必须重读。

**参考**：OpenBitFun `ToolResultStoragePolicy` 含 `per_tool_limit_chars`(50K) / `per_round_limit_chars`(200K) / `preview_chars`(2K)。`select_tool_result_indices_for_persistence()` 按大小降序选择需持久化的结果，直到总字符数低于限制。`file_read_tool` 用 `already_served` + `file_revision` 实现读取回执与失效。

**实施状态**：❌ 未实现。需新增 `assistant-tools/src/pipeline.rs`。

---

## Step 7 — RoundExecutor（Agent Loop）

> **建什么**：Agent Loop 核心——消费 LLM 流式输出，提取 tool_calls，执行并回填，形成闭环。
> **为什么现在建**：Pipeline 已能执行工具了，但"LLM 返回 tool_call → 执行 → 回填结果 → 继续生成"的循环需要 RoundExecutor。
> **建完能干什么**：LLM 可以真正调工具了。Agent Loop 闭环。

### 7.1 核心结构

```rust
pub struct RoundExecutor {
    ai_client: Arc<AIClient>,
    tool_pipeline: Arc<ToolPipeline>,
    stream_processor: Arc<StreamProcessor>,
    event_queue: Arc<EventQueue>,
    max_iterations: u32,
}
```

### 7.2 循环流程

```
execute_round(context) → RoundOutcome:

  for iteration in 0..max_iterations:
    1. 解析可用工具（两级可用性配置叠加）
       available = resolve_available_tools(
         registry,
         context.conversation_tool_config,
       )
       tool_defs = available.map(|t| ToolDefinition::from_tool(t))

    2. 构建请求
       request = ModelRequest { messages, tools: tool_defs }

    3. 调用 LLM（流式）
       stream_outcome = stream_processor.process_stream(request, ai_client)

    4. 提取 assistant message（含 text + tool_calls）
       messages.push(stream_outcome.assistant_message)

    5. 无 tool_call → 对话结束
       return Finished(stream_outcome)

    6. 发射 tool_call running 事件
       emit AgenticEvent::ToolCallStarted { ... }

    7. 执行工具
       tool_results = tool_pipeline.execute(tool_calls, context)

    8. 回填 tool_results 到消息历史
       messages.push(Message::tool_results_message(...))

    9. 继续循环（LLM 基于工具结果继续生成）

  return MaxIterationsReached
```

### 7.3 当前缺口

当前 `StreamAccumulator`（`chat_runtime.rs`）已能累积 `tool_calls`，`StreamOutcome.tool_calls` 已有数据。但 **tool_call 到此为止**——没有执行、没有回填、没有继续循环。RoundExecutor 是补全 Agent Loop 的关键缺失。

> **注**：上述累积能力属「错位现状」。按 I30 阶段一剥离计划，`StreamAccumulator` / `StreamOutcome` 中的 tool call 字段将被移除，该能力迁入 `agent_loop/tool_call.rs`。RoundExecutor 应基于迁移后的实现，而非 `chat_runtime`。

**参考**：OpenBitFun 的 `agent-runtime` crate 包含完整的 turn loop——`AgentSession` 管理消息历史 + tool 执行 + 子 Agent 委托 + 代际守卫。openhuman 的 `agent_turn_loop` 在 `tinyagents` harness 中实现，含 middleware 链（policy → output capture → compaction）。

**实施状态**：❌ 未实现。需新增 `assistant-agent/src/round_executor.rs`。

**前置依赖**：tool call 的累积能力必须先落到独立的 `assistant-agent/src/agent_loop/`，并从 chat 构建路径中剥离。剥离范围、类型契约与验收标准见 [I30-phase1-toolcall-separation-spec.md](./I30-phase1-toolcall-separation-spec.md)。Step 7 落地时直接复用 `agent_loop/tool_call.rs`，**不得**另起第三套累积器。

---

## Step 8 — Tauri 桥接（双入口分流）

> **建什么**：两个入口共享 Tool trait 实现，但执行路径分流。
> **为什么现在建**：ToolBox 入口（os_tool_exec）已实现且工作。Agent 入口（RoundExecutor → Pipeline）在 Step 7 建完后自然就位。
> **建完能干什么**：用户可以通过 ToolBox 点击调用工具（已有），LLM 可以通过 Agent Loop 自主调用工具（新增）。

### 8.1 双入口架构

```
                    ┌──────────────────────┐
                    │   Tool trait 实现     │  共享：实现一次
                    └──────────┬───────────┘
                               │
                    ┌──────────┴───────────┐
                    │    ToolRegistry       │  共享：注册一次
                    └────┬────────────┬────┘
                         │            │
           ┌─────────────┘            └──────────────┐
           │                                         │
  ┌────────┴──────────────┐           ┌──────────────┴─────────┐
  │   ToolBox 入口         │           │   Agent 入口            │
  │   os_tool_exec         │           │   RoundExecutor         │
  │   ↓ ToolRegistry.find  │           │   ↓ ToolPipeline        │
  │   ↓ Tool.execute()     │           │     → ToolGateway      │
  │   ↓ 返回结果           │           │     → Tool.execute()   │
  │   直达执行              │           │     → ResultStorage     │
  └────────────────────────┘           └────────────────────────┘
```

### 8.2 为什么 ToolBox 不经过 ToolPipeline

| ToolPipeline 基础设施 | Agent 场景（LLM 自主调用） | ToolBox 场景（用户主动调用） |
|---|---|---|
| ToolGateway 审批流 | LLM 自主决策 → 需要人类审批 | 用户自己点的 → 审批自己无意义 |
| Permission Policy 评估 | 防止 LLM 越权 | 用户就是权限主体 |
| 审计日志 | 追踪 AI 自主行为 | 用户操作，不是 AI 行为 |
| 超时保护 | 防止 LLM 发起的命令挂起 | 共享 Rust 侧 `DEFAULT_TOOL_TIMEOUT` 兜底（`exec::invoke`），前端只维护加载态 |
| 结果回填消息历史 | Agent Loop 需要继续对话 | 结果直接返回组件渲染 |

**安全约束不缺失**：路径校验等在 `Tool.execute()` 内部，两个入口均受保护；`shell_exec` 的 SafetyBelt 与纵深防御同样在 `execute()` 内，只有 AllowlistGate / ApprovalGate 这层审批留在 Pipeline（见 [OSCALLDESIGN.md](../designed/OSCALLDESIGN.md) §3.3–§3.4）。

### 8.3 os_tool_exec Tauri command

请求 / 返回的 wire 类型定义在 `src-tauri/src/protocol/dto/os_tool.rs`（全仓唯一 DTO 落点，`*Dto` 后缀 + `rename_all = "camelCase"`）：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OsToolExecRequestDto {
    pub tool: String,
    pub action: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OsToolResultDto {
    pub ok: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
}

impl From<assistant_tools::ToolResult> for OsToolResultDto { /* 逐字段搬运 */ }
```

命令本体：

```rust
#[tauri::command]
pub async fn os_tool_exec(
    registry: State<'_, Arc<ToolRegistry>>,
    request: OsToolExecRequestDto,
) -> Result<OsToolResultDto, String> {
    let tool = registry.find_for_toolbox(&request.tool)
        .ok_or_else(|| format!("tool '{}' not found in registry", request.tool))?;

    let mut input = request.params;
    if let Some(obj) = input.as_object_mut() {
        obj.insert("action".to_string(), Value::String(request.action));
    } else {
        input = serde_json::json!({ "action": request.action });
    }

    let context = ToolUseContext::for_toolbox(None);
    let result = tool.execute(&input, &context).await.map_err(|e| e.to_string())?;
    Ok(OsToolResultDto::from(result))
}
```

**约束**：`os_tool_exec` 是**唯一**的 Tauri command 入口，禁止为单个 OS 工具新增独立 Tauri command。新增 OS 工具**不新增 request DTO**——复用上面的通用 `OsToolExecRequestDto`，参数差异只体现在 `params` 内容与工具自身的 `input_schema()`。

**已知待修**：`ToolUseContext::for_toolbox(None)` 使 `workspace_root` 恒为 `None`，相对路径工具在 ToolBox 入口下不可用，需补工作区来源（见 8.2）。

### 8.4 前端 os.ts 代理对象

```typescript
export const os = {
  fsLink: {
    create(source, target, kind): Promise<OsResult<FsLinkCreateResult>>,
    check(source): Promise<OsLinkCheckResult>,
  },
  // 新增工具只在这里加一个命名空间
};
```

### 8.5 新增 OS 工具标准流程（5 步）

| 步骤 | 位置 | 内容 |
|------|------|------|
| 1 | `assistant-tools/os/{tool}.rs` | 实现 Tool trait |
| 2 | `assistant-tools/os/mod.rs` | 加 `mod {tool};` + `registry.register()` |
| 3 | `src/tools/os.ts` | 加一个命名空间 + 类型 |
| 4 | `src/tools/{tool-dir}/index.tsx` | React 组件，调 `os.{namespace}.*()` |
| 5 | `src/tools/registry.ts` | 加一行 `defineTool()` |

**不需要**：单独的 Tauri command、单独的 service 文件、cmd/mod.rs 注册。

**实施状态**：✅ os_tool_exec 已实现（`cmd/os_tool_exec.rs`）。✅ os.ts 已实现。✅ fs_link 完整流程已走通。

---

## Step 9 — 前端集成

> **建什么**：ChatDelta 扩展（tool_call_status 五态）+ ToolCallBubble 渲染 + 审批交互 UI。
> **为什么现在建**：后端 RoundExecutor + Gateway 已能发射事件了，但前端需要消费这些事件、渲染工具调用状态、处理审批交互。
> **建完能干什么**：用户可以看到工具调用过程（running/success/error），可以审批危险操作（awaiting_user → Approve/Deny）。

### 9.1 ChatDelta 扩展

```typescript
interface ChatDeltaPayload {
  // ...现有字段...
  tool_call_status?: ToolCallStatus
  tool_call_result?: string
  tool_call_started_at?: number
  tool_call_finished_at?: number
  tool_call_awaiting_approval?: ToolApprovalRequest
}

type ToolCallStatus = 'running' | 'success' | 'error' | 'awaiting_user' | 'cancelled'

interface ToolApprovalRequest {
  tool_call_id: string
  tool_name: string
  arguments: Record<string, unknown>
  intents: PermissionIntent[]
}
```

### 9.2 ToolCallBubble 五态渲染

```
┌─ running ───────────┐   ┌─ success ───────────┐
│ 🔄 fs_read          │   │ ✅ fs_read  (120ms) │
│ /src/main.ts        │   │ /src/main.ts        │
└─────────────────────┘   │ ▼ [45 lines]        │
                           └─────────────────────┘
┌─ error ─────────────┐   ┌─ awaiting_user ─────┐
│ ❌ shell_exec        │   │ ⚠️ shell_exec       │
│ Permission denied   │   │ rm -rf /tmp/cache   │
└─────────────────────┘   │ [✓ Approve] [✗ Deny]│
                           └─────────────────────┘
┌─ cancelled ──────────┐
│ ⊘ fs_delete         │
│ User rejected       │
└──────────────────────┘
```

**参考**：OpenBitFun `ToolCardStatus` 含 `running` / `completed` / `error` / `cancelled` / `awaiting_approval` 五态，`toolInteractionModel` 管理交互状态机。openhuman `tool-call.tsx` / `tool-error.tsx` / `tool-fallback.tsx` 分组件渲染。

### 9.3 审批交互流

```
RoundExecutor → ToolGateway evaluate → Ask
  → emit AgenticEvent::ToolAwaitingApproval
  → 前端 ChatDeltaPayload.tool_call_awaiting_approval
  → ToolCallBubble 渲染 [Approve] [Deny] 按钮
  → 用户点击 → Tauri command: approve_tool_call / deny_tool_call
  → Rust ApprovalChannel 收到决策
  → 继续执行或拒绝
```

**实施状态**：❌ 未实现。ChatDelta 已有 `delta_tool_call_id/name/arguments`（流式累积），但 `tool_call_status` 五态和审批交互未实现。

---

## Step 10 — Agent 模式集成 + 两级可用性配置

> **建什么**：两级可用性配置（AgentMode 预设 → 对话级）+ Agent 模式到 PermissionPolicy 的映射 + deny-by-default 语义。
> **为什么最后建**：Policy（Step 4）和前端（Step 9）都已就位，只需要把 Agent 模式配置接入 Policy 生成 + 可用性配置接入 RoundExecutor。
> **建完能干什么**：不同 Agent 模式有不同的工具能力和审批级别；用户可在对话中随时增删可用工具。

### 10.1 AgentMode 扩展字段

```typescript
interface AgentMode {
  // ...现有字段...
  tools: string[]                    // 工具白名单（空=无可用工具，deny-by-default）
  max_tool_tier: ToolTier           // 允许的最高工具层级
  approval_level: ApprovalLevel     // 审批级别
}

type ToolTier = 'sandbox' | 'os' | 'external'

type ApprovalLevel =
  | 'none'              // 全部自动执行
  | 'dangerous_only'    // 仅 destructive/open_world 需审批
  | 'all_os_tools'      // 所有非 Sandbox 工具需审批
```

**`tools` 语义**：白名单，deny-by-default。`tools: []` = 无可用工具，`tools: ["fs_read"]` = 仅 fs_read 可用。所有内置模式默认 `tools: []`，用户必须显式启用工具。

### 10.2 内置预设

| 模式 | max_tool_tier | approval_level | tools | 说明 |
|------|---------------|----------------|-------|------|
| 默认 | `sandbox` | `none` | `[]` | 无可用工具，用户按需启用 |
| 快速 | `sandbox` | `none` | `[]` | 同上 |
| 深度 | `os` | `dangerous_only` | `[]` | 层级开放到 OS，但工具仍需显式启用 |
| 翻译助手 | `sandbox` | `none` | `[]` | 同默认 |

**设计理由**：deny-by-default 确保新增工具不会自动对任何模式可用，与最小权限原则一致。用户在 AgentMode 编辑表单或对话工具栏中显式勾选所需工具。

### 10.3 ConversationToolConfig（对话级）

对话创建时从 AgentMode 预设**深拷贝**，之后独立演化。AgentMode 修改不回溯影响已创建的对话。

```typescript
interface ConversationToolConfig {
  maxToolTier: ToolTier              // 层级上限（硬上限，不可被 enabledTools 超越）
  approvalLevel: ApprovalLevel       // 审批级别
  enabledTools: string[]             // 显式启用的工具名列表
  disabledTools: string[]            // 显式禁用的工具名列表（优先级高于 enabledTools）
}
```

**不变量**：
- `disabledTools` 优先级高于 `enabledTools`——同一工具同时出现在两个列表中，结果为禁用
- `maxToolTier` 是硬上限——即使 `enabledTools` 包含 External 工具，`maxToolTier: Os` 时 External 工具仍不可用
- 工具 `name` 是稳定标识符，不因 i18n 变化

**预填充逻辑**：
```
conversation.tool_config = {
  maxToolTier:     agentMode.max_tool_tier,
  approvalLevel:   agentMode.approval_level,
  enabledTools:    [...agentMode.tools],   // 深拷贝
  disabledTools:   [],
}
```

### 10.4 resolveAvailableTools() 解析算法

两级配置叠加解析：

```rust
fn resolve_available_tools(
    registry: &ToolRegistry,
    config: &ConversationToolConfig,
) -> Vec<Arc<dyn Tool>> {
    // 1. 层级过滤（硬上限）
    let tier_filtered: Vec<_> = registry.all_tools()
        .filter(|t| t.tier() <= config.max_tool_tier)
        .collect();

    // 2. 白名单过滤（deny-by-default：空=无工具）
    let candidates: Vec<_> = if config.enabled_tools.is_empty() {
        vec![]  // deny-by-default：无显式启用 = 无可用工具
    } else {
        tier_filtered.into_iter()
            .filter(|t| config.enabled_tools.contains(&t.name().to_string()))
            .collect()
    };

    // 3. 排除禁用列表
    candidates.into_iter()
        .filter(|t| !config.disabled_tools.contains(&t.name().to_string()))
        .collect()
}
```

### 10.5 可用性层与权限层正交声明

| 层 | 问题 | 机制 | 时机 |
|----|------|------|------|
| **可用性层** | LLM **能否看到**这个工具？ | `resolveAvailableTools()` | 构建 LLM 请求时 |
| **权限层** | LLM **能否执行**这个操作？ | `ToolGateway.evaluate()` | 执行 tool_call 时 |

**交互规则**：
- 可用性层先过滤——不可用的工具不出现在 `tools` 参数中，LLM 不会生成对应 tool_call
- 权限层后评估——即使工具可用，具体操作仍可能被 Policy Deny 或要求审批
- 两者不冲突——可用性是"工具级开关"，权限是"操作级控制"

### 10.6 Agent 模式 → Policy 映射

```rust
fn policy_from_agent_mode(mode: &AgentModeConfig) -> ResolvedPermissionPolicy {
    let mut global_rules = default_global_rules();

    match mode.approval_level {
        ApprovalLevel::None => {
            global_rules.insert(0, PermissionRule::new("*", "*", PermissionEffect::Allow));
        },
        ApprovalLevel::DangerousOnly => {
            // 保持默认（read=Allow, edit/execute=Ask）
        },
        ApprovalLevel::AllOsTools => {
            global_rules.push(PermissionRule::new("read", "*", PermissionEffect::Ask));
        },
    }

    let tier_rules = match mode.max_tool_tier {
        ToolTier::Sandbox => vec![
            PermissionRule::new("*", "tier:os", PermissionEffect::Deny),
            PermissionRule::new("*", "tier:external", PermissionEffect::Deny),
        ],
        ToolTier::Os => vec![
            PermissionRule::new("*", "tier:external", PermissionEffect::Deny),
        ],
        ToolTier::External => vec![],
    };

    ResolvedPermissionPolicy::new(global_rules, vec![
        PermissionConstraintLayer::new(tier_rules),
    ])
}
```

**参考**：OpenBitFun 通过 `agents.md` + `ToolExposure` override 实现——每个 Agent 定义可覆盖工具的默认暴露级别（Direct/Deferred）。OpenHuman 通过 `ToolPolicy` trait 的 `check()` 方法实现——`AllowAllToolPolicy` 是默认，可安装自定义策略。

### 10.7 数据模型扩展清单

| 层 | 变更 | 说明 |
|----|------|------|
| **Rust 存储** | `AgentModeRow` 新增 `max_tool_tier: String`（`"sandbox"/"os"/"external"`）+ `approval_level: String`（`"none"/"dangerous_only"/"all_os_tools"`） | 当前仅有 `tools_json`，缺两个控制字段 |
| **Rust 存储** | `ConversationDetail` / `ConversationRow` 新增 `tool_config_json: String` | 对话级工具配置 |
| **SQLite** | `ALTER TABLE agent_modes ADD COLUMN max_tool_tier TEXT NOT NULL DEFAULT 'sandbox';` | 迁移 |
| **SQLite** | `ALTER TABLE agent_modes ADD COLUMN approval_level TEXT NOT NULL DEFAULT 'none';` | 迁移 |
| **SQLite** | `ALTER TABLE conversations ADD COLUMN tool_config_json TEXT NOT NULL DEFAULT '{}';` | 迁移 |
| **前端类型** | `AgentMode` interface 新增 `maxToolTier` + `approvalLevel` | 对齐 Rust |
| **前端类型** | 新增 `ConversationToolConfig` interface | 对话级配置 |

### 10.8 前端交互规格

| 交互点 | 位置 | 功能 |
|--------|------|------|
| **AgentMode 编辑表单** | 设置页 | 工具多选列表（按 tier 分组展示），deny-by-default 默认全不勾选 |
| **对话工具栏** | 聊天输入区旁 | 当前对话的可用工具概览（图标列表），点击展开可增删 |
| **工具可用性变更提示** | 聊天流中 | 当工具配置变更时，系统消息提示"已启用 shell_exec"等 |

**AgentMode 编辑表单的工具选择器**：

```
┌─ 工具配置 ────────────────────────────────────────┐
│                                                     │
│  层级上限:  [Sandbox ▼]  (Sandbox / Os / External)  │
│  审批级别:  [dangerous_only ▼]                      │
│                                                     │
│  ── Sandbox 工具 ──                                │
│  ☐ json-viewer  ☐ color-picker  ☐ base64           │
│                                                     │
│  ── OS 工具 ──                                     │
│  ☐ fs_read  ☐ fs_list  ☐ fs_glob  ☐ fs_grep       │
│  ☐ fs_write  ☐ fs_delete  ☐ shell_exec             │
│  ☐ ask_user                                         │
│                                                     │
│  ── External (MCP) ──                              │
│  (无 MCP server 连接时显示"暂无外部工具")            │
│                                                     │
└─────────────────────────────────────────────────────┘
```

- 默认全不勾选（deny-by-default）
- 层级上限切换时，下方工具列表自动展开/折叠对应分组
- 内置 AgentMode（`isBuiltin: true`）的工具预设**可查看但不可修改**——用户需创建自定义模式

**实施状态**：❌ `max_tool_tier` / `approval_level` / `ConversationToolConfig` / `resolveAvailableTools` 均未实现。需扩展 `agent_modes` 表 + `conversations` 表 + `AgentModeRow` struct + `ConversationRow` struct + 前端 interface + RoundExecutor 集成。

---

## Crate 组织

```
src-tauri/
├── crates/
│   ├── assistant-agent/
│   │   └── src/
│   │       ├── chat_runtime.rs    # 现有：StreamAccumulator
│   │       ├── chat_tasks.rs      # 现有：任务管理+取消
│   │       ├── round_executor.rs  # 新增：Agent Loop
│   │       └── ...
│   ├── assistant-tools/
│   │   └── src/
│   │       ├── lib.rs             # 现有
│   │       ├── trait.rs           # 现有：Tool trait + ToolTier + BehaviorHints + ToolAudience
│   │       ├── definition.rs      # 现有：ToolDefinition（工具定义唯一内部形状）
│   │       ├── registry.rs        # 现有：ToolRegistry
│   │       ├── result.rs          # 现有：ToolResult
│   │       ├── context.rs         # 现有：ToolUseContext
│   │       ├── exec.rs            # 新增：invoke() 统一执行入口 + 超时（I31 T2）
│   │       ├── policy.rs          # 新增：PermissionEffect + PermissionRule + ResolvedPermissionPolicy
│   │       ├── gateway.rs         # 新增：ToolGateway + ApprovalChannel
│   │       ├── pipeline.rs        # 新增：ToolPipeline
│   │       ├── sandbox/           # 新增：L0 沙箱工具桥接
│   │       │   └── mod.rs
│   │       ├── os/                # 现有+扩展
│   │       │   ├── mod.rs         # 现有：register_os_tools()
│   │       │   ├── fs_link.rs     # 现有
│   │       │   ├── process_output.rs  # 现有
│   │       │   ├── fs_read.rs     # 新增
│   │       │   ├── fs_write.rs    # 新增
│   │       │   ├── fs_delete.rs   # 新增
│   │       │   ├── fs_list.rs     # 新增
│   │       │   ├── fs_glob.rs     # 新增
│   │       │   ├── fs_grep.rs     # 新增
│   │       │   ├── fs_mkdir.rs    # 新增
│   │       │   ├── fs_move.rs     # 新增
│   │       │   ├── shell_exec.rs  # 新增
│   │       │   └── ask_user.rs    # 新增
│   │       └── mcp/               # 新增：L2 MCP 工具
│   │           ├── mod.rs
│   │           ├── bridge.rs
│   │           ├── client.rs
│   │           └── server_manager.rs
│   └── assistant-providers/       # 现有：provider 适配+流式
├── src/
│   └── cmd/
│       └── os_tool_exec.rs        # 现有：唯一 Tauri command 入口
```

---

## 实施状态总览

| Step | 模块 | 状态 | 关键文件 |
|------|------|------|---------|
| 0 | 值类型 | ✅ 完成 | `trait.rs` `result.rs` `context.rs` |
| 1 | Tool Trait | ✅ 完成 | `trait.rs`（含 `audience()` / `ToolAudience`） |
| 2 | L1 OS: fs_link | ✅ 完成 | `os/fs_link.rs`（`TOOLBOX_ONLY`） |
| 2 | L1 OS: 其他 | ❌ 未实现 | 需新增 fs_read/write/edit/delete/list/glob/grep/mkdir/move/shell_exec/ask_user |
| 3 | ToolRegistry | ✅ 完成 | `registry.rs`（`<=`、排序、`audience` 过滤） |
| 4 | Permission Policy | ❌ 未实现 | 需新增 `policy.rs` |
| 5 | Tool Gateway | ❌ 未实现 | 需新增 `gateway.rs` |
| 6 | Tool Pipeline | ❌ 未实现 | 需新增 `pipeline.rs` |
| 7 | RoundExecutor | ❌ 未实现 | 需新增 `round_executor.rs` |
| 8 | Tauri 桥接 | ⚠️ 部分完成 | `os_tool_exec.rs` `os.ts`；DTO 已归位 `protocol/dto/os_tool.rs`。待补：workspace_root 来源、错误通道统一（超时已由 `exec::invoke` 统一兜底） |
| 9 | 前端集成 | ❌ 未实现 | ChatDelta 五态 + ToolCallBubble + 审批 UI |
| 10 | Agent 模式+两级可用性 | ❌ 未实现 | max_tool_tier + approval_level + ConversationToolConfig + resolveAvailableTools + Policy 映射 |

> **本表之外的规格新增**（来自参考项目对比，均为规格先行、代码未落地）：`ToolResult` 双通道与 `error_kind`（0.3）、`ApprovalKey` 规范化（5.4.1）、`validate_input` 校验阶段（6.2.1）、动态并发安全（6.3）、持久化选择算法与读取去重（6.4）、`fs_edit`（Step 2）。设计理由见 [I18-design](I18-agent-tool-architecture-design.md) §3.12–§3.15、§4.2.1。

---

## 安全原则

1. **工具自描述权限，策略层决策**：工具 `permission_intents()` 声明需求，ToolGateway 决定放行
2. **多层策略取最严**：应用默认 + 项目配置 + Agent 模式三层叠加，`most_restrictive` 组合
3. **委托只能收紧不能放宽**：`PermissionRuntimeCeiling` 禁止携带 `Allow` 规则
4. **审批流统一**：`Ask` → `awaiting_user` → 前端审批 UI → approve/reject
5. **先建框架再加能力**：Policy → Gateway → Pipeline → RoundExecutor → OS 工具 → MCP
6. **最小权限**：工具默认声明最高限制，Agent 模式按需放宽
7. **超时隔离**：所有工具执行带超时，Shell 执行默认 30s
8. **审计日志**：所有工具调用记录到 `tool_calls` 表
9. **路径约束**：文件操作限制在工作区或用户授权路径内，系统敏感路径硬拒绝
10. **双入口不破坏现有体验**：前端 ToolBox 独立使用不变，Agent Loop 调用是新增能力
11. **deny-by-default**：工具白名单空=无可用工具，新增工具不自动对任何模式可用，必须显式启用
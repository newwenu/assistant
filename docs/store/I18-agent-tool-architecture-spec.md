# I18 — Agent Tool 框架层规格（Step 0-3、8）

| 项 | 值 |
|---|---|
| 状态 | 现行（框架层已完成，桥接待收口） |
| 日期 | 2026-11-04（结构重构） |
| 关联 | [I38 roadmap](./I38-agent-roadmap.md)（阶段入口）、[I18 design](./I18-agent-tool-architecture-design.md)（决策依据） |
| 影响范围 | `assistant-tools`（框架层）、`src-tauri/src/cmd/os_tool_exec.rs`、`src-tauri/src/protocol/dto/os_tool.rs`、前端 `src/tools/os.ts` |

> **本文职责已收窄**：只覆盖**词汇层与框架层**（Step 0-3、Step 8 桥接）。
> 以下内容已迁出，不在本文维护：
>
> | 主题 | 权威文档 |
> |---|---|
> | 权限控制层（Policy / Gateway / Pipeline / 审批 / 结果预算） | [I38a](./I38a-tool-control-layer-spec.md) |
> | Agent Loop（RoundExecutor / 累积器 / tools 注入） | [I38b](./I38b-agent-loop-spec.md) |
> | OS 工具逐个实现与 Schema | [I38c](./I38c-os-tools-spec.md) |
> | 前端五态 / 审批 UI / Agent 模式 | [I38d](./I38d-agent-frontend-modes-spec.md) |
> | MCP 外部工具 | [I38e](./I38e-mcp-tools-spec.md) |
> | 运行时内核（turn / 状态机 / 端口） | [I36 spec](./I36-agent-runtime-spec.md) |
>
> 决策依据（为什么这样分层、参考了什么、取舍）见 [I18 design](./I18-agent-tool-architecture-design.md)，本文不重复论证。

---

## 搭建总览

```
Step 0  值类型              ToolTier · BehaviorHints · ToolResult · PermissionIntent · ToolUseContext
Step 1  Tool Trait          统一工具抽象接口
Step 2  工具实现            → 已迁出到 I38c（P5）
Step 3  ToolRegistry        注册 · 查找 · 层级 / 受众过滤
Step 8  Tauri 桥接          os_tool_exec · os.ts · 双入口分流
```

### 层次职责

| 层 | 本文章节 | 说明 |
|---|---|---|
| 词汇层 | Step 0-1 | 定义「工具长什么样、结果长什么样」。零行为，纯类型 |
| 执行层 | Step 2（迁出） | `execute()` 是唯一有 IO 副作用的地方；实现见 I38c |
| 控制层 | 迁出 | Policy / Gateway / Pipeline 见 I38a |
| 连接层 | Step 8 | 把框架接到 Tauri 与前端；ToolBox 双入口 |

### 实施状态（2026-11-04）

| Step | 模块 | 状态 | 说明 |
|---|---|---|---|
| 0 | 值类型 | ✅ 完成 | `trait.rs` / `result.rs` / `context.rs` |
| 1 | Tool Trait | ✅ 完成 | 含 `audience()` / `ToolAudience` |
| 2 | 工具实现 | ❌ 未完成 | 仅 `fs_link`（`TOOLBOX_ONLY`）；其余见 I38c |
| 3 | ToolRegistry | ✅ 完成 | `<=` 语义、排序、`description()` 降级、受众过滤 |
| 8 | Tauri 桥接 | ⚠️ 部分完成 | `os_tool_exec` 已走 `exec::invoke`；待补 `workspace_root` 来源 |

---

## Step 0 — 值类型

> **建什么**：工具体系的基础词汇——层级标记、安全标记、结果格式、权限声明、执行上下文。
> **为什么先建**：上面每一层都引用这些类型。没有 `ToolTier` 就写不出 `Tool` trait，没有 `ToolResult` 就写不出 `execute()` 签名。

### 0.1 ToolTier

工具能力层级，**描述执行环境**，不决定安全边界（安全决策在控制层）。

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolTier {
    Sandbox,   // 前端沙箱，无 OS 访问
    Os,        // Rust 后端，有 OS 访问
    External,  // 外部进程（MCP），有 OS + 网络访问
}
```

**PartialOrd 语义**：`Sandbox < Os < External`。Agent 模式声明 `max_tier` 即可限制可用层级——`tool_definitions_for_tier(Os)` 返回 Sandbox + Os 工具。

**实施状态**：✅ 已实现，`assistant-tools/src/trait.rs`。

### 0.2 BehaviorHints

三维安全标记，**描述工具对系统的潜在影响**，不决定是否放行（放行在控制层）。

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BehaviorHints {
    pub read_only: bool,      // 不修改任何状态 → 可并发安全执行
    pub destructive: bool,    // 删除/覆盖数据 → 需审批
    pub open_world: bool,     // 网络/不可预测副作用 → 需审批
}
```

**推导规则**（上层消费）：

- `read_only = true` → Pipeline 可并发执行
- `destructive = true` → Gateway 触发 Ask 审批
- `open_world = true` → Gateway 触发 Ask 审批

**实施状态**：✅ 已实现，`assistant-tools/src/trait.rs`。

### 0.3 ToolResult

工具执行结果，Rust ↔ 前端统一序列化格式。

**当前实现**（`assistant-tools/src/result.rs`）：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResult {
    pub ok: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
}
```

**P2（I38a §3.2）将增量扩展**：

```rust
pub struct ToolResult {
    pub ok: bool,
    pub data: Option<Value>,                  // UI 渲染
    pub result_for_assistant: Option<String>, // 回填模型的精简文本
    pub error: Option<String>,
    pub error_kind: Option<ToolErrorKind>,
}

pub enum ToolErrorKind { Business, Timeout, Cancelled, Internal, Denied }
```

| `error_kind` | 产生位置 | 模型应有的反应 |
|---|---|---|
| `Business` | 工具内部业务失败（路径不存在、参数缺失、未知 action） | 可改参数重试 |
| `Timeout` | `exec::invoke` 超时兜底 | 可重试，但应缩小范围或提高 timeout |
| `Cancelled` | 用户取消 / 代际切换 | 不重试 |
| `Internal` | 工具返回 `Err` 后被 `invoke` 归一化 | 不重试，向用户报告 |
| `Denied` | Gateway 拒绝或审批拒绝 | 不重试，换方案 |

**wire 约束**：

- Tauri 返回类型是 `protocol/dto/os_tool.rs::OsToolResultDto`，与 `ToolResult` 同构。
- 输出 DTO 的 `Option` 字段**不加** `skip_serializing_if`：键恒在、值 `null`，前端声明 `T | null`。
- 改 DTO 后执行 `npm run contract:gen`，并提交 `src/types/protocol.contract.ts`。

**实施状态**：✅ 基础字段已实现；双通道与 `error_kind` 由 P2 扩展（I38a §3.2），存量工具零改动。

### 0.4 PermissionIntent

工具声明的单个权限需求。**纯声明**——工具说「我需要什么」，Gateway 决定「允不允许」。

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionIntent {
    pub action: String,          // read / edit / delete / execute / network / external
    pub resources: Vec<String>,  // 路径 glob / 命令名 / URL pattern / *
}
```

**硬约束**：`permission_intents()` 不执行任何 IO、不修改状态；只读工具返回空 `Vec`。

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

**工厂方法**：`ToolUseContext::for_toolbox(workspace_root)`。

**语义约束**：

1. `workspace_root` 若为 `Some`，必须是已存在的绝对路径；不合法则所有相对路径解析失败。
2. `workspace_root` 为 `None` 时，文件系统工具只接受绝对路径。
3. 不新增 deadline / 取消令牌字段：超时由 `exec::invoke` 承担，取消由控制层承担。

**已知待修**：`os_tool_exec` 当前恒传 `workspace_root: None`，ToolBox 侧相对路径不可用（Step 8 收口）。

**实施状态**：✅ 已实现，`assistant-tools/src/context.rs`。

---

## Step 1 — Tool Trait

统一工具抽象接口，三层工具（Sandbox / OS / MCP）均实现此 trait。

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
    fn permission_intents(&self, _input: &Value, _context: &ToolUseContext)
        -> Result<Vec<PermissionIntent>, String> { Ok(vec![]) }
    async fn execute(&self, input: &Value, context: &ToolUseContext)
        -> Result<ToolResult, String>;
}
```

### 1.1 方法契约

| 方法 | 调用时机 | 副作用 | 说明 |
|---|---|---|---|
| `name()` | 注册 / 查找 / 日志 | 无 | 全局唯一，Rust snake_case（`fs_read`），前端 camelCase（`os.fsRead`） |
| `description()` | LLM function calling 构建 | 无 | 供模型理解工具用途 |
| `input_schema()` | LLM function calling 构建 | 无 | JSON Schema |
| `tier()` | 层级过滤 | 无 | 返回 `ToolTier` |
| `audience()` | 两个入口的查询侧过滤 | 无 | 见 §1.2 |
| `behavior_hints()` | 安全评估 | 无 | 默认全 `false` |
| `is_readonly()` | 并发决策 | 无 | 默认从 `behavior_hints()` 推导 |
| `permission_intents()` | Gateway 评估 | **无** | 纯声明；只读工具返回空 |
| `execute()` | Pipeline / `os_tool_exec` | **有** | 唯一有副作用的方法；内含路径校验等安全约束 |

### 1.2 ToolAudience

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolAudience {
    pub agent: bool,
    pub toolbox: bool,
}

impl ToolAudience {
    pub const BOTH: Self;
    pub const TOOLBOX_ONLY: Self;
    pub const AGENT_ONLY: Self;
    pub const NONE: Self;
}

impl Default for ToolAudience {
    fn default() -> Self { Self::NONE }
}

fn audience(&self) -> ToolAudience { ToolAudience::NONE }
```

| 常量 | `agent` | `toolbox` | 典型用途 |
|---|---|---|---|
| `BOTH` | 是 | 是 | 通用工具，两个入口都可用 |
| `TOOLBOX_ONLY` | 否 | 是 | `fs_link`：用户点按钮可用，模型不应看到 |
| `AGENT_ONLY` | 是 | 否 | 只服务 Agent Loop，不从前端透传入口暴露 |
| `NONE`（默认） | 否 | 否 | fail-closed：未显式声明受众的新工具不可见 |

**硬约束**：

1. 新增工具必须显式声明受众。默认值是 `NONE`：忘记声明时工具不会出现在任何入口，属于**安全失败**而非静默暴露。
2. `audience` 只决定「能否被某个入口看到」，不决定「是否启用」；启用由 `ConversationToolConfig` 白名单控制（I38d §4）。
3. `audience().agent == false` 的工具不得进入 `tool_definitions_for_tier`。

### 1.3 与 P2 的增量接口

P2 在 `Tool` trait 增加以下**带默认实现**的方法（契约见 I38a §3.1）：

```rust
fn validate_input(&self, _input: &Value, _ctx: &ToolUseContext) -> ValidationResult;
fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool;
```

- `validate_input`：Pipeline 的 validate 阶段；不变量违规需标 `blocks_input_rewrite = true`。
- `is_concurrency_safe`：默认等于 `is_readonly()`；工具可按 input 覆写。
- 存量工具零改动。

**实施状态**：✅ 基础 trait 已实现；P2 增量方法未实现（I38a §4-C3）。

### 1.4 参考

- OpenBitFun：`Tool` trait 含 `short_description()` / `default_exposure()` / `is_concurrency_safe(input)` / `validate_input()` / `call_impl()` + `call()` / `permission_intents()`。核心模式一致：`permission_intents()` 纯声明，执行方法有副作用。
- openhuman：`ToolPolicy` 是独立策略接口，不混入 `Tool` trait；本项目的 Policy 同样独立（I38a）。

---

## Step 2 — 工具实现（已迁出）

工具清单、安全标记、每个工具的 JSON Schema 与实施顺序 → 见 [I38c](./I38c-os-tools-spec.md)。

**当前唯一实现**：`fs_link`（`assistant-tools/src/os/fs_link.rs`，`TOOLBOX_ONLY`），完整设计见 [I18b](./I18b-tool-unified-bridge-spec.md) §4。

---

## Step 3 — ToolRegistry

工具注册中心，全局唯一，所有工具注册到同一 Registry。

```rust
pub struct ToolRegistry {
    tools: DashMap<String, Arc<dyn Tool>>,
}
```

### 3.1 操作契约

| 方法 | 签名 | 行为 |
|---|---|---|
| `new()` | `→ ToolRegistry` | 创建空 Registry |
| `register()` | `(Arc<dyn Tool>) → ()` | 以 `tool.name()` 为 key 插入；同名覆盖 |
| `find()` | `(&str) → Option<Arc<dyn Tool>>` | 按 name 精确查找，**全量视图**（管理/诊断用） |
| `find_for_toolbox()` | `(&str) → Option<Arc<dyn Tool>>` | 仅返回 `audience().toolbox == true` 的工具（`os_tool_exec` 使用） |
| `tool_names()` | `() → Vec<String>` | 返回所有已注册工具名（全量视图，当前未排序） |
| `tool_definitions_for_tier()` | `(ToolTier) → Vec<ToolDefinition>` | 返回 **tier ≤ 参数 且 `audience().agent == true`** 的工具定义，按 name 字典序排序 |

### 3.2 约束

1. **`<=` 语义**：`entry.value().tier() <= max_tier`。
2. **先收集再 await**：`DashMap` 迭代产生的 guard 不得跨 `await`；先收集 `Arc<dyn Tool>`，drop guard，再逐个 `await description()`。
3. **确定性排序**：结果按 `name` 字典序排序；顺序不稳定会破坏 prompt 缓存命中。
4. **描述失败降级**：`description()` 失败不 panic，降级为空串并 `log::warn`。
5. **受众过滤**：`tool_definitions_for_tier` 只收集 `audience().agent == true`；`find_for_toolbox` 只返回 `audience().toolbox == true`。

### 3.3 注册时机

```rust
// setup.rs
let registry = Arc::new(ToolRegistry::new());
os::register_readonly_tools(&registry);   // P5-R1
// os::register_write_tools(&registry);    // P5-R2（P2 审批就绪后）
// os::register_dangerous_tools(&registry);// P5-R3（P2 + P6 就绪后）
handle.manage(registry);
```

**实施状态**：✅ 完成，`assistant-tools/src/registry.rs`。`<=` 语义、按名排序、`description()` 降级、受众过滤均已落地并有单测。

---

## Step 8 — Tauri 桥接（ToolBox 入口）

> Agent 入口（RoundExecutor → ToolPipeline）见 I38b；本文只定义 ToolBox 直达路径。
> 完整的 5 步新增流程、`os.ts` 代理与 `fs_link` 设计见 [I18b](./I18b-tool-unified-bridge-spec.md)。

### 8.1 双入口架构

```
Tool trait 实现 ──► ToolRegistry ──┬──► ToolBox：os_tool_exec → execute()
                                   └──► Agent：RoundExecutor → ToolPipeline → Gateway → execute()
```

两个入口共享 Tool trait 与 Registry；ToolBox 不经过 Pipeline（用户点击即执行意图），安全不变量由 `execute()` 内部保证。

### 8.2 os_tool_exec（**以代码为准**）

```rust
#[tauri::command]
pub async fn os_tool_exec(
    registry: State<'_, Arc<ToolRegistry>>,
    request: OsToolExecRequestDto,
) -> Result<OsToolResultDto, String> {
    let tool = registry
        .find_for_toolbox(&request.tool)
        .ok_or_else(|| format!("tool '{}' not found in registry", request.tool))?;

    let mut input = request.params;
    if let Some(obj) = input.as_object_mut() {
        obj.insert("action".to_string(), Value::String(request.action));
    } else {
        input = serde_json::json!({ "action": request.action });
    }

    let context = ToolUseContext::for_toolbox(None);
    let result = invoke(tool.as_ref(), &input, &context, DEFAULT_TOOL_TIMEOUT).await;
    Ok(OsToolResultDto::from(result))
}
```

**与旧文档的差异（已修正）**：旧版示例调用 `tool.execute().await.map_err(...)`，已过时。当前实现走 `exec::invoke`：

- `Err` 通道在 `invoke` 内归一化为 `ToolResult { ok: false }`；
- 超时由 `DEFAULT_TOOL_TIMEOUT`（30s）兜底；
- 前端从 `invoke()` reject 变为 resolve 出 `OsResult { ok: false, error }`，`os.ts` 已按 `OsResult` 消费。

### 8.3 wire 类型

请求 / 返回类型在 `src-tauri/src/protocol/dto/os_tool.rs`（全仓唯一 DTO 落点，`*Dto` 后缀 + `rename_all = "camelCase"`）：

- `OsToolExecRequestDto { tool, action, params }`，`deny_unknown_fields`。
- `OsToolResultDto { ok, data, error }`；P2 扩展双通道后新增 `resultForAssistant` / `errorKind`，按输出 DTO 规则处理。

**硬约束**：

1. `os_tool_exec` 是**唯一**的 Tauri command 入口，禁止为单个 OS 工具新增独立 command。
2. 新增 OS 工具不新增 request DTO；参数差异只体现在 `params` 与 `input_schema()`。
3. `tools` 受众为 `AGENT_ONLY` 的工具不会出现在 `find_for_toolbox`，前端不可达。

### 8.4 待收口

| # | 问题 | 归属 |
|---|---|---|
| B1 | `workspace_root` 恒为 `None`，ToolBox 相对路径不可用 | P5/P6 |
| B2 | `ToolUseContext` 不携带 session / call id | P4（Agent 入口由 Pipeline 注入） |
| B3 | `tool_names()` 未排序（仅诊断用） | 后续需要确定性诊断输出时补 |

---

## 与既有文档的关系

| 文档 | 关系 |
|---|---|
| [I38 roadmap](./I38-agent-roadmap.md) | 阶段、状态、门禁的唯一入口 |
| [I18 design](./I18-agent-tool-architecture-design.md) | 分层、权限、双入口、安全原则的决策依据 |
| [I38a](./I38a-tool-control-layer-spec.md) | 控制层（Policy / Gateway / Pipeline / 审批 / 预算） |
| [I38b](./I38b-agent-loop-spec.md) | RoundExecutor、累积器、`ChatOptions.tools` |
| [I38c](./I38c-os-tools-spec.md) | OS 工具清单与 Schema |
| [I38d](./I38d-agent-frontend-modes-spec.md) | 五态 UI、审批 UI、Agent 模式 |
| [I38e](./I38e-mcp-tools-spec.md) | MCP 外部工具 |
| [I18b](./I18b-tool-unified-bridge-spec.md) | ToolBox 桥接与新增工具 5 步流程 |
| [I31](./I31-phase1-tool-framework-spec.md) | 阶段一框架（T1/T2 已落地，T3 待做） |
| [OSCALLDESIGN](../designed/OSCALLDESIGN.md) | Shell 洋葱模型 |

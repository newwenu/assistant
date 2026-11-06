# I31 — 阶段一工具框架规格（纯 Rust，不接入前后端，不新增工具）

| 项 | 值 |
|---|---|
| 状态 | 已归档（阶段一工具框架规格；进度见 [roadmap](./I38-agent-roadmap.md) §2） |
| 日期 | 2026-11-04 |
| 关联 | [I38 roadmap](./I38-agent-roadmap.md)（P0/P1）、[I18 design](./I18-agent-tool-architecture-design.md)、[I18 spec](./I18-agent-tool-architecture-spec.md)（Step 0-3 / 8）、[I30](./I30-phase1-toolcall-separation-spec.md)、[I18b](./I18b-tool-unified-bridge-spec.md)、[I38c](./I38c-os-tools-spec.md)（工具能力去向） |
| 影响范围 | `src-tauri/crates/assistant-tools` 单 crate |

> 执行清单：[I31-phase1-tool-framework-tasklist.md](./I31-phase1-tool-framework-tasklist.md)（T3 逐项勾选与验收映射）。

> **实施进度**（2026-11-04 复核）：T1、T2、T3 已完成——`definition.rs` 新建；`registry.rs` 的 `<=` 语义 / 按名排序 / `description()` 降级 / `audience` 过滤均有单测；`fs_link` 已声明 `TOOLBOX_ONLY`；`os_tool_exec` 已改用 `find_for_toolbox()` 并改走 `exec::invoke`（`Err` 通道不再外泄，超时 30s 兜底）；`os/path.rs` 落地五个路径安全函数，`fs_link` 已迁移并修正 `permission_intents` 词汇。§2 的事实表已按下述状态标注，§11 的实施顺序同步更新。

本文定义阶段一「工具框架」的交付内容：**类型契约、执行边界、共享路径安全、验收标准**。

**本阶段不实现任何新工具能力**——不新增 `read` / `ls` / `glob` / `grep` / `write` / `mkdir` / `move` / `delete` / `shell`。工具能力属 P5（[I38c](./I38c-os-tools-spec.md)），等控制层与桥接语义稳定后再做。本阶段只保证「工具插进来时，框架是对的」。

---

## 1 边界

### 1.1 范围内

| 项 | 说明 |
|---|---|
| 类型契约 | 引入 `ToolDefinition`；`ToolRegistry` 修正（`<=` 语义、排序、异步构建） |
| 执行边界 | `exec::invoke()` 统一错误通道 + 超时 |
| 共享基础设施 | `os/path.rs` 路径安全，供**将来**所有文件系统工具复用 |
| 存量迁移 | 将 `fs_link` 迁移到 `os/path.rs`，修掉其私有实现的安全缺陷 |
| 单元测试 | 框架类型 + 路径安全 + `invoke` 边界 |

### 1.2 范围外

| 项 | 归属 |
|---|---|
| 9 个新 OS 工具实现 | P5 / [I38c](./I38c-os-tools-spec.md)，见 §6 |
| 控制层（Policy / Gateway / Pipeline） | P2 / [I38a](./I38a-tool-control-layer-spec.md) |
| RoundExecutor 与累积器 | P4 / [I38b](./I38b-agent-loop-spec.md) |
| Tauri 桥接（`os_tool_exec` 语义、`workspace_root` 来源、DTO） | I18b / I18 spec Step 8 |
| 前端 ToolBox 组件、`os.ts`、审批 UI | P6 / [I38d](./I38d-agent-frontend-modes-spec.md) |
| tool call 流式累积器与 chat 剥离 | I30（A 段已完成） |
| `ask_user` | 依赖前端输入框，见 [I38c §9](./I38c-os-tools-spec.md) |
| `ChatOptions.tools` 注入 | P4 / [I38b](./I38b-agent-loop-spec.md) §4 |

**一句话判据**：本阶段交付物必须是「能在 `cargo test` 里被直接调用的纯 Rust 能力」，且**不扩大工具能力集合**。任何需要 `tauri::State`、`invoke`、React 组件才能验证的东西，都不属于本阶段。

### 1.3 为什么只保留 `fs_link`，且不把它当作 Agent 工具

`fs_link` 不是典型的 Agent 工具。本阶段保留它，只因它已存在、已在生产注册表、已接入两个入口，且其路径校验缺陷是**现存安全问题**。

依据：

1. **参考资料没有它**。I18 design §2.1 列出的内置 OS 工具为 Read / Write / Delete / LS / Glob / Bash / AskUser，**不含 Link**。`fs_link` 是本项目自加，不是继承自参考项目。
2. **主流 Agent 工具集不含建链**。Claude Code / Cursor 类工具的写侧能力是 write / edit，建链归入 shell（`ln -s`、`mklink`）。把建链做成独立工具，对模型是低收益的额外 schema 负担。
3. **本项目的定位是验证载体**。I18 spec 明确写「先用 `fs_link` 验证端到端，再建安全框架」。它的价值在于证明 `ToolBox → os_tool_exec → ToolRegistry → Tool` 这条链路通，而不在于给 LLM 一项能力。
4. **它是 ToolBox 的用户侧工具**。I18b 把它列为 ToolBox 的第 22 个工具（21 个纯前端 + 1 个 OS 能力），服务对象是「人点按钮」，不是「模型自主调用」。

**O1 已在本阶段解决**：`Tool` trait 新增带默认实现的 `audience()` 返回 `ToolAudience`，用两个独立维度表达工具受众：

| 维度 | 执行点 | 语义 |
|---|---|---|
| `agent` | `tool_definitions_for_tier()` 过滤 | 是否进入 LLM 的 `tools` 列表 |
| `toolbox` | `find_for_toolbox()` 过滤（`os_tool_exec` 使用） | 是否允许 ToolBox 桥接入口按名调用 |

默认值为 `ToolAudience::NONE`（fail-closed）：未显式声明的工具对两个入口都不可见。`fs_link` 显式声明为 `ToolAudience::TOOLBOX_ONLY`，因此存量行为不变：

- `agent = false` → P4 注入链路建成后，它**不会**出现在 LLM 的 `tools` 列表；
- `toolbox = true` → `os_tool_exec` 仍可调用，ToolBox 能力不受影响。

这取代了原先「拆两个 registry」的方案：不新增容器、不改 `ToolRegistry` 的存储模型，只在查询侧加过滤条件，成本更低且语义更明确。

> `os_tool_exec` 由 `find()` 改为 `find_for_toolbox()`。这是**过滤增强而非语义变更**：现有唯一工具 `fs_link` 的 `toolbox = true`，故对外行为零变化；但今后任何声明 `toolbox = false` 的工具不会因注册而意外暴露给前端。

---

## 2 现状实测

以下为 2026-11-02 复核后的源码事实，并标注当前状态（T1 落地后部分事实已不成立，保留原编号以维持与其他文档的引用关系）。

| # | 事实 | 位置 | 状态 |
|---|---|---|---|
| F1 | **改造前**：`Tool::description()` 为 `async fn`，但唯一潜在调用方 `tool_definitions_for_tier` 是同步函数且硬编码 `"description": ""`，该方法**零调用者** | `trait.rs`、`registry.rs` | 已修复（T1） |
| F2 | **改造前**：`tool_definitions_for_tier` 用 `==` 精确匹配 tier，而非「不超过」。`ToolTier` 已 `derive(Ord)` 却未使用 | `registry.rs` | 已修复（T1） |
| F3 | **改造前**：`tool_definitions_for_tier` 返回手写 `serde_json::json!`，与 `Tool::input_schema()` / `description()` 契约重复；`inputSchema` 键名靠字面量维护 | `registry.rs` | 已修复（T1） |
| F4 | `resolve_path` / `validate_path` 是 `fs_link.rs` 的私有函数与关联函数。**任何后续工具都无法复用** | `os/fs_link.rs` | 已修复（T3：私有实现删除，统一走 `os::path`） |
| F5 | `validate_path` 用 `canonicalize().unwrap_or_else(\|_\| path.to_path_buf())`：目标不存在时（新建文件、新建链接的 source）规范化**必然失败**，回退为原路径，仅剩 `ParentDir` 一道检查 | `os/fs_link.rs` | 已修复（T3：`canonicalize_lenient` 向上找存在祖先，失败即 `Err`） |
| F6 | Windows 保护目录用字符串前缀比较（`starts_with("C:\\Windows")`）。大小写敏感（`c:\windows` 绕过）、无分隔符边界（`C:\Windows.old` 误拒）、写死 `C:` 盘符 | `os/fs_link.rs` | 已修复（T3：组件级比较 + 大小写不敏感 + 环境变量取值） |
| F7 | `permission_intents` 产出 `fs_link_create` / `fs_link_target`，与 design §2.1 的 `read` / `edit` / `delete` / `execute` 词汇表冲突 | `os/fs_link.rs` vs design §2.1 | 已修复（T3：统一为 `edit`） |
| F8 | **改造前**：`execute()` 的错误双通道：`Err(String)` 与 `ToolResult::err` 并存，无归一化层 | `trait.rs`、`cmd/os_tool_exec.rs` | 已修复（T2：`exec::invoke` + `ToolResult::from_exec_error`） |
| F9 | 除 `process_output` 外**无任何单测**；`os/mod.rs` 只注册 `fs_link` | crate 全局 | 已修复（T1/T2/T3 共 50 个单测） |
| F10 | **改造前**：`assistant-tools` 未声明 `tokio`，无法实现超时；`tempfile` 仅作为传递依赖存在于 `Cargo.lock` | `Cargo.toml` | 已修复（T2：`tokio` 升为正式依赖，`tempfile` 进入 `dev-dependencies`） |
| F11 | `os_tool_exec` 是**通用透传**：按 `request.tool` 查表即执行。任何注册进 `ToolRegistry` 的工具都会立即获得一个前端可达入口 | `src-tauri/src/cmd/os_tool_exec.rs` | 部分缓解（`find_for_toolbox` 已过滤 `toolbox == false`；`toolbox == true` 的工具仍注册即可达） |

F4/F5/F6 是本阶段唯一涉及「现有能力」的部分：`fs_link` 已在生产注册表、已可从 Webview 控制台调用，其路径校验缺陷是**现存安全问题**，不是新功能。修它是本阶段的正当理由。

---

## 3 目标产物

```
src-tauri/crates/assistant-tools/src/
├── lib.rs          (已改) 导出 definition / exec
├── context.rs      (待改) 补充 workspace_root 语义约束的文档注释
├── result.rs       (已改) 增加 ToolResult::from_exec_error
├── trait.rs        (已改) 新增 ToolAudience + audience()（默认 NONE，fail-closed），补充错误语义分层注释
├── definition.rs   (已建) ToolDefinition
├── exec.rs         (已建) invoke() 统一入口 + 超时
├── registry.rs     (已改) <= 语义 + 确定性排序 + description() 降级 + audience 过滤
└── os/
    ├── mod.rs            (已改) 仍只注册 fs_link；新增 `pub mod path`
    ├── path.rs           (已建) 共享路径安全
    ├── process_output.rs (不改) 编码检测，保留
    └── fs_link.rs        (已改) 迁移到 path.rs，修正 permission 词汇（受众声明已完成）
```

**不新增**：`os/output.rs`、任何 `agent_*.rs`。

> `os/output.rs`（输出截断）本阶段**不建**。它的唯一消费方是 `read` / `glob` / `grep` / `shell`，没有工具就没有调用者，属死代码。等 Step 2 第一个需要截断的工具落地时一并实现。

---

## 4 类型契约修正

### 4.1 ToolDefinition（新增 `definition.rs`）

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}
```

要点：

1. **不携带 `tier`**。tier 是调度决策依据，不应进入发给模型的工具定义。
2. `camelCase` 序列化产出 `inputSchema`，与既有 `json!` 字面量一致。
3. 这是全仓**唯一的工具定义内部形状**，`registry.rs` 不再手写 `json!`（修 F3）。
4. 与 I18 spec Step 3 的 `Vec<Value>` 相比这是**收窄**（从裸 `Value` 变为具名类型），语义与序列化结果不变。
5. **它是内部形状，不是 provider wire 形状**。OpenAI 需要 `{"type":"function","function":{...}}` 包壳，Gemini 需要 schema 方言转换，Anthropic 需要 `input_schema` 直传。本阶段没有 provider 消费方，因此**不实现转换层**；P4 接入时由 provider adapter 从 `ToolDefinition` 转换，禁止让 `ToolDefinition` 自身承担多协议适配职责。

### 4.2 ToolRegistry（修 F1/F2/F3）

```rust
impl ToolRegistry {
    pub fn register(&self, tool: Arc<dyn Tool>);
    pub fn find(&self, name: &str) -> Option<Arc<dyn Tool>>;
    pub fn find_for_toolbox(&self, name: &str) -> Option<Arc<dyn Tool>>;
    pub fn tool_names(&self) -> Vec<String>;
    pub async fn tool_definitions_for_tier(&self, max_tier: ToolTier) -> Vec<ToolDefinition>;
}
```

约束：

1. **`<=` 语义**：`entry.value().tier() <= max_tier`（`ToolTier` 已 `Ord`，`Sandbox < Os < External`）。
2. **先收集再 await**：`DashMap` 迭代产生的 guard 不得跨 `await`。实现必须先把 `Arc<dyn Tool>` 收集进 `Vec`，drop guard，再逐个 `await description()`。否则在 `tokio` 多线程下存在死锁风险。
3. **确定性排序**：结果按 `name` 字典序排序。工具定义进入 system prompt，顺序不稳定会破坏 prompt 缓存命中，也会让快照测试抖动。
4. `description()` 失败不 panic，降级为空串并 `log::warn`。单个工具的描述失败不应让整轮对话无法构建工具列表。
5. **受众过滤**：`tool_definitions_for_tier` 只收集 `audience().agent == true` 的工具；`find_for_toolbox` 只返回 `audience().toolbox == true` 的工具。`find` 与 `tool_names` 保持全量视图，供管理与诊断使用。

> **为什么改 registry 而不改 trait**：I18 定义 `description()` 为 async，是为将来动态描述留口子。改 trait 会破坏 I18 契约；改 registry 只影响一个尚无调用方的函数。
>
> **例外**：§4.2.1 的 `audience()` 是新增方法，**带默认实现**（`ToolAudience::NONE`），因此不需要改动存量实现；唯一存量工具 `fs_link` 已显式声明 `TOOLBOX_ONLY`，收紧默认值不改变其行为。

**关于「尚无调用方」**：`tool_definitions_for_tier` 在 P4 之前不会有生产调用者。本阶段交付它 + 单测，是为了让框架在 P4 可直接被消费，不是提前写死代码。该定位以 A5 固化（必须有测试覆盖，否则视为未交付）。

### 4.2.1 ToolAudience（新增到 `trait.rs`）

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

// Tool trait 新增
fn audience(&self) -> ToolAudience { ToolAudience::NONE }
```

| 常量 | `agent` | `toolbox` | 典型用途 |
|---|---|---|---|
| `BOTH` | 是 | 是 | 通用工具，两个入口都该能用 |
| `TOOLBOX_ONLY` | 否 | 是 | `fs_link`：用户点按钮可用，模型自主调用不该有 |
| `AGENT_ONLY` | 是 | 否 | 只服务 Agent Loop、不应从前端透传入口暴露的工具 |
| `NONE`（默认） | 否 | 否 | fail-closed：未显式声明受众的新工具不可见 |

**为什么用两个布尔而非单一 `exposed_to_agent`**：「LLM 可见」与「桥接可达」是两个不同执行点的判断。若只用一个标记，`AGENT_ONLY` 这类需求将来仍要再改一次契约。两个维度一次性表达完整。

**为什么默认 `NONE`**：新增工具必须显式声明受众，遗漏时宁可不可见（fail-closed），也不能静默暴露给 LLM 或前端桥接。存量工具只有 `fs_link` 一个且已显式声明 `TOOLBOX_ONLY`，因此收紧默认值不改变现有行为。

**与 deny-by-default 的关系**：`audience` 只决定「工具能否被某个入口看到」，不决定「是否启用」。工具是否启用仍由 `conversation.tool_config.tools` 白名单控制（design §3.8）。两者正交：一个 `BOTH` 工具若不在白名单，模型依然看不到它。

### 4.3 invoke（新增 `exec.rs`，修 F8）

```rust
pub const DEFAULT_TOOL_TIMEOUT: Duration = Duration::from_secs(30);

pub async fn invoke(
    tool: &dyn Tool,
    input: &Value,
    context: &ToolUseContext,
    timeout: Duration,
) -> ToolResult;
```

语义（**边界只返回 `ToolResult`，永不返回 `Err`**）：

| 情况 | 返回 |
|---|---|
| `input` 不是 JSON object | `ToolResult::err("input must be a JSON object")`（先于超时判断，不消耗超时预算） |
| `execute` 返回 `Ok(r)` | `r` 原样 |
| `execute` 返回 `Err(e)` | `ToolResult::from_exec_error(name, e)` → `tool '<name>': <e>` |
| 超过 `timeout` | `ToolResult::from_exec_error(name, "timed out after {:?}")` → `tool '<name>': timed out after 30s` |

配套在 `result.rs` 增加：

```rust
impl ToolResult {
    pub fn from_exec_error(tool: &str, error: impl Into<String>) -> Self;
}
```

错误文本统一前缀工具名（`tool '<name>': <message>`），便于日志检索与模型纠错；超时也复用同一前缀，避免出现两种错误形状。

**错误语义分层**（写入 `trait.rs` 文档注释）：

| 通道 | 含义 | 调用方行为 |
|---|---|---|
| `ToolResult { ok: false }` | 工具正常执行，业务失败（路径不存在、权限不足、参数缺失、未知 action） | 结果回填给模型，模型可自行纠正 |
| `Err(String)` | 工具无法完成本次调用（内部缺陷、无法构造结果） | 由 `invoke` 归一化为 `ToolResult::err`，对上游不可见 |

**净效果**：`os_tool_exec` 与将来的 Pipeline 都只消费 `ToolResult`，`Err` 不外泄（修 F8）。

> **超时落点与 design 的差异**：design §3.6 表格把超时写成「UI 层自行处理（ToolBox 场景）」。本阶段把超时放进 Rust `invoke`，理由是 I18b 实际未实现任何超时，且将来 Agent 入口必须在 Rust 侧有兜底。这是一处**有意的设计变更**，已在 design §3.6 留痕。

**超时的真实能力边界（必须写进 `exec.rs` 文档注释）**：

`tokio::time::timeout` 只在 `await` 点抢占。对**同步阻塞**的 `execute()`（如当前 `fs_link` 内部直接调 `std::fs` / `std::process::Command`）超时到点**无法中断**已阻塞的线程，`invoke` 只能丢弃结果、不能停止工作。因此：

| 工具类型 | 超时手段 |
|---|---|
| 异步、`await` 点密集 | `tokio::time::timeout` 直接生效 |
| 同步文件 IO | 必须 `spawn_blocking` 包裹，超时只保证调用方及时返回，后台线程仍会跑完 |
| 子进程（`shell`） | **不能依赖 `tokio::timeout`**，必须子进程级超时 + kill（Windows 用 `TerminateProcess` / Job Object） |

本阶段无 `shell`，但契约必须先写清，避免 Step 2 误以为 `invoke` 的超时对进程有效。

### 4.4 ToolUseContext（保持字段不变）

```rust
pub struct ToolUseContext {
    pub workspace_root: Option<String>,
    pub session_id: Option<String>,
    pub tool_call_id: Option<String>,
}
```

**不改 `String` 为 `PathBuf`**：该结构实现 `Serialize/Deserialize` 且 `camelCase`，已是潜在 wire 类型；改类型会波及 DTO 与前端。路径转换统一在 `os/path.rs` 内完成。

**新增语义约束（写入文档注释）**：

1. `workspace_root` 若为 `Some`，必须是**已存在的绝对路径**。`path.rs` 在首次使用时校验，不合法则所有相对路径解析失败。
2. `workspace_root` 为 `None` 时，文件系统工具**只接受绝对路径**。
3. 不新增 deadline / 取消令牌字段：超时由 `invoke` 承担，取消由上层（Step 6）承担。

> `os_tool_exec` 当前恒传 `workspace_root: None`（I18b 遗留问题），因此 ToolBox 侧所有相对路径必然失败。该问题属 Step 8 收口，本阶段不改，但 `path.rs` 的错误信息必须明确指出「未提供 workspace_root，相对路径不可用」，避免用户看到含糊的失败。

### 4.5 permission_intents 词汇表（修 F7）

统一为 design §2.1 的四个 action：

| action | 语义 | 资源 |
|---|---|---|
| `read` | 读取内容 | 目标路径 |
| `edit` | 创建 / 修改 / 移动 | 每个受影响路径各一条 |
| `delete` | 删除 | 目标路径 |
| `execute` | 执行命令 | 命令基名 |

`fs_link` 修正为：

```rust
PermissionIntent::new("edit", source)   // 原 fs_link_create
PermissionIntent::new("edit", target)   // 原 fs_link_target
```

只读工具（`read_only = true`）的 `permission_intents` 仍返回空 `Vec`（沿用默认实现），与 `behavior_hints` 保持一致。上表的 `read` 仅为将来 Policy 需要细粒度读授权时预留，当前实现返回空。

---

## 5 共享基础设施：os/path.rs

```rust
pub fn resolve(input: &str, context: &ToolUseContext) -> Result<PathBuf, String>;
pub fn resolve_leaf_preserving(input: &str, context: &ToolUseContext) -> Result<PathBuf, String>;
pub fn canonicalize_lenient(path: &Path) -> Result<PathBuf, String>;
pub fn reject_system_dirs(path: &Path) -> Result<(), String>;
pub fn ensure_within_root(path: &Path, root: &Path) -> Result<(), String>;
```

### 5.1 canonicalize_lenient（修 F5）

算法：

1. 任何 `Component::ParentDir` → 拒绝（保持现有第一道防线）。
2. `path.exists()` → `canonicalize()` 并简化前缀（见 5.2）。
3. 否则：向上找到第一个存在的祖先 `A`，剩余尾部组件 `R`；返回 `canonicalize(A)?.join(R)`。`R` 中若出现 `ParentDir` 已被步骤 1 拦截，因此拼接结果不会逃逸。
4. 找不到任何存在的祖先（如盘符不存在）→ `Err`，不得回退为原路径。

> **绝不允许 `unwrap_or_else(|_| path.to_path_buf())`**。这正是 F5 的根因：目标不存在时静默降级，导致校验对象与实际使用对象不一致。

### 5.2 Windows `\\?\` 前缀处理

`canonicalize()` 在 Windows 返回 `\\?\C:\...`（UNC 形式）。该形式会破坏 `mklink` 参数、前端展示与用户可见输出。必须统一简化：

- 优先使用 `dunce::simplified()`；
- 若不引入 `dunce`，手写剥离：以 `\\?\` 开头且第三字符为盘符时截断前缀；`\\?\UNC\` 转换为 `\\`。

#### 5.2.1 Windows 路径形态补充（A4 必须覆盖）

仅处理 `..` 与 `\\?\` 不足以覆盖真实输入，`resolve()` 必须显式处理以下形态，否则行为未定义：

| 形态 | 示例 | 要求 |
|---|---|---|
| 盘符相对路径 | `C:foo`、`C:\` | `Path::is_absolute()` 为 `false`，但也不是普通相对路径。**不得**与 `workspace_root` 简单拼接（会得到 `C:\ws\C:foo`）。必须先判断「盘符 + 无根」形态，按盘符基准解析或直接拒绝 |
| UNC 路径 | `\\server\share\x`、`\\?\UNC\server\share` | 视为绝对路径；`ensure_within_root` 时不得因 `\\?\UNC\` 前缀剥离而与 root 比较失败 |
| 尾随分隔符 | `C:\a\b\`、`/a/b/` | 比较与拼接前统一去尾随分隔符（根目录除外） |
| `.` / `..` 混合 | `a/./b/../c` | 先按组件规范化，再执行 5.1 的 `ParentDir` 检查 |
| `root` 自身是符号链接 | `workspace_root` 指向链接目录 | `ensure_within_root` 两侧都用 `canonicalize_lenient` 后的结果比较，避免一侧已解析、一侧未解析 |

盘符相对路径建议**直接拒绝**并返回明确错误（`relative path with drive prefix is not supported`），比猜测基准更安全。

### 5.3 reject_system_dirs（修 F6）

1. 按**组件序列**比较，不按字符串前缀。`C:\Windows.old` 不属于 `C:\Windows`，`C:\Windows\System32` 属于。
2. Windows 比较**大小写不敏感**。
3. 受保护目录**从环境变量取值**，不写死盘符：
   - `SystemRoot`（通常为 Windows 目录）
   - `ProgramFiles` / `ProgramFiles(x86)`
   - `ProgramData`
4. Unix 保持 `["/etc", "/usr", "/bin", "/sbin", "/System", "/Library"]`，同样按组件序列比较。

### 5.4 ensure_within_root

1. `root` 也走 `canonicalize_lenient`。
2. `path` 必须以 `root` 为组件级前缀（`path == root` 允许）。
3. Windows 大小写不敏感。

### 5.5 resolve 组装顺序

```
1. 绝对路径 / 相对路径 + workspace_root → joined
2. canonicalize_lenient(&joined)
3. reject_system_dirs(&canon)
4. workspace_root 为 Some 时 → ensure_within_root(&canon, root)
5. 返回 canon（调用方必须使用返回值，不得再用原始字符串）
```

第 5 条是硬约束：`execute()` 内**只能**用 `resolve()` 的返回值做 IO，避免「校验 A、使用 B」。

### 5.6 resolve_leaf_preserving（`fs_link` 必需）

`resolve()` 内部 `canonicalize` 会**穿过符号链接解析到真实目标**，这对「操作链接本身」的工具是错的：

| 场景 | 用 `resolve()` 的后果 | 正确行为 |
|---|---|---|
| `check`：判断 `source` 是不是链接 | `canonicalize` 跟随链接 → `symlink_metadata` 看到的是普通文件/目录 → `isLink` 恒为 `false` | 必须保留叶子，只校验父目录 |
| `create`：`target` 本身是符号链接 | 链接指向被改写成真实路径，建出的链接语义偏移 | 必须保留叶子 |
| `create`：`source` 尚未存在 | 与 F5 同类，需要宽松规范化 | 父目录规范化 + 叶子原样拼接 |

算法：**只规范化父目录，叶子组件原样保留**。

1. 父目录走 `canonicalize_lenient`；
2. 叶子的 `FileName` 直接 `join`，不做任何解析；
3. `reject_system_dirs` 与 `ensure_within_root` 仍作用于拼接后的完整路径。

现状之所以「碰巧正确」，是因为 `resolve_path` 返回的是**未规范化的原始路径**——这不是设计，是 F5 的副作用。迁移时必须显式用 `resolve_leaf_preserving`，不能用 `resolve`。

### 5.7 落地范围

`path.rs` 本阶段只服务一件事：**替换 `fs_link.rs` 内的私有实现**。

- `fs_link.rs` 删除自身的 `resolve_path` / `validate_path`。
- `check` / `create` 的 `source` / `target` 一律走 `resolve_leaf_preserving`；仅当将来有工具需要「解析到真实目标」时才用 `resolve`。
- `fs_link` 的 source 在创建时通常不存在，正是 F5 的触发场景，迁移后必须补回归测试。
- `fs_link` 的 `target` 在 Windows junction 场景下父目录可能不存在，同样需覆盖。

**不新增任何调用方**。等 Step 2 的工具落地时，它们直接 import 此模块，无需再写一份。

### 5.8 受保护目录的拒绝面会变大（需回归确认）

从字符串前缀改为组件级比较 + 大小写不敏感后，拒绝面变化如下：

| 路径 | 现状 | 迁移后 | 性质 |
|---|---|---|---|
| `C:\Windows.old` | 误拒（`starts_with("C:\Windows")`） | 放行 | 修复 |
| `c:\windows\system32` | 绕过（大小写敏感） | 拒绝 | 修复 |
| `c:\programdata\...` | 绕过（大小写敏感 + 写死 `C:`） | 拒绝 | 收紧 |

第三行是**行为收紧**：若用户 `workspace_root` 恰好位于 `ProgramData` / `Program Files` 之下，迁移后该 workspace 的**全部**文件操作都会被拒绝。现状同样会拒绝（只是容易被大小写绕过），因此这是「把既有的正确意图真正落实」，不是新增限制。

必须在 A7 显式覆盖「`workspace_root` 位于受保护目录」这一用例，并在实现中明确：**`root` 自身不豁免**（`ensure_within_root` 的放行不等于 `reject_system_dirs` 的放行，后者先执行）。

---

## 6 工具能力为何推迟

本阶段刻意不做任何新工具。理由按优先级：

| # | 理由 |
|---|---|
| 1 | **没有消费方**。`ChatOptions` 无 `tools` 字段，`ToolDefinition` 无法进入请求；工具实现了也没有 LLM 能看到 |
| 2 | **控制层未就绪**。`shell` / `delete` 属破坏性能力，按 design §3.10「先安全框架再加能力」，必须在 Policy/Gateway 之后 |
| 3 | **桥接语义未收敛**。`workspace_root` 恒为 `None`、无超时、DTO 刚落位，此时铺 9 个工具会把错误形态复制 9 份 |
| 4 | **框架先稳定收益更大**。`invoke` + `path.rs` + `ToolDefinition` 一旦定稿，后续每个工具都是「填一个文件 + 注册一行」，成本恒定 |

**框架就绪后，新增工具可直接复用的能力**：

- 路径校验：`os::path::resolve`
- 错误归一化：`exec::invoke`
- 注册与 tier 过滤：`ToolRegistry`
- LLM 定义构建：`ToolDefinition` + `tool_definitions_for_tier`
- 输出截断：**尚未提供**，第一个需要截断的工具落地时再建 `os/output.rs`

---

## 7 排除项

### 7.1 ask_user

排除。其行为是「暂停 Agent Loop，等待前端输入」，依赖审批端口 + 前端输入框。归入 I38a（审批端口）与 I38d（输入 UI），最终在 I38c §9 落地。

### 7.2 ChatOptions.tools

排除。`assistant-providers` 的 `ChatOptions` **没有 `tools` 字段**，工具定义无法进入请求。补齐属 P4（I38b §4）的注入链路。本阶段只保证类型与构建能力就绪。

### 7.3 接入层替换（记录为 Step 8 待办）

~~`os_tool_exec` 直接调用 `tool.execute(...)` 并 `.map_err(|e| e.to_string())?`，仍走双通道。~~ **已由 T2 修复**：`os_tool_exec` 已改走 `exec::invoke(...)`，`Err` 通道在 `invoke` 内归一化，前端从 reject 变为 `OsResult { ok: false }`。本待办关闭。

---

## 8 安全边界（F11 的现状与将来）

本阶段不新增工具，因此**前端可达的工具集合不变**：仍只有 `fs_link`。F11 在本阶段不构成新增风险，但必须记录为前置约束：

> 任何被 `register_os_tools()` 注册的工具，立刻获得一个可从 Webview 控制台调用的入口，无需审批。

「无前端组件」只意味着**没有 UI 入口**，不意味着**不可调用**。

由此产生的硬约束（同步写入 I38c 与 I18 spec Step 8）：

1. **Step 4/5 就绪前，不得把 `shell` 加入生产注册表**。
2. `delete` 等破坏性工具与前端组件同期上线，不得先行注册。
3. 新增工具时，`os/mod.rs` 应提供分组注册（只读 / 修改 / 危险），`setup.rs` 只注册已确认可暴露的分组。

第 3 条本阶段不实现（无工具可分组），仅作为约定记录。

---

## 9 依赖变更

`crates/assistant-tools/Cargo.toml` 新增：

```toml
[dependencies]
tokio = { workspace = true }
dunce = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
tempfile = { workspace = true }
```

| 依赖 | 用途 | 备注 |
|---|---|---|
| `tokio` | `time::timeout` | workspace 已声明 `features = ["full"]`；T2 已将其升为**正式依赖**，同时保留 `dev-dependencies` 声明供单测使用 `macros` / `rt-multi-thread` |
| `tempfile` | 路径测试临时目录 | 根 `Cargo.toml` 已声明 `tempfile = "3"`，按项目惯例写 `{ workspace = true }`；T2 已加入 `dev-dependencies`，T3 的 `path.rs` 单测直接使用 |
| `dunce` | 剥离 Windows `\\?\` 前缀（§5.2） | T3 新增：根 `Cargo.toml` 声明 `dunce = "1"`，crate 以 `{ workspace = true }` 引入；仅在 `cfg(windows)` 分支实际调用 |

**不引入**：`glob` / `ignore` / `grep`（无工具使用）、`thiserror`（契约是 `Result<ToolResult, String>`，类型化错误会连带改动 `invoke`、`os_tool_exec`、将来的 Pipeline 三处，留到 Step 6）。

---

## 10 验收标准

| # | 标准 | 状态 |
|---|---|---|
| A1 | `cargo test -p assistant-tools` 全绿 | 已达成（T3：50 个单测） |
| A2 | `cargo clippy -p assistant-tools -- -D warnings` 无告警 | 当前通过 |
| A3 | `cargo fmt --all --check` 通过 | 当前通过 |
| A4 | `os/path.rs` 单测覆盖：`..` 拒绝、不存在路径的宽松规范化、`C:\Windows.old` 不被误拒、`c:\windows\system32` 被拒（大小写不敏感）、`\\?\` 前缀剥离、盘符相对路径 `C:foo` 被明确拒绝、UNC 路径（`\\server\share` 与 `\\?\UNC\`）可解析、workspace 越界拒绝、`root` 自身允许、`root` 为符号链接时比较正确、`workspace_root=None` 时相对路径报出明确错误 | 已达成（T3，`path.rs` 20 个单测） |
| A5 | `registry` 单测覆盖：`<=` 语义（`Os` 查询返回 `Sandbox + Os`）、按名排序、`description()` 失败降级为空串、`audience().agent == false` 的工具不进入 `tool_definitions_for_tier`、`find_for_toolbox` 拒绝 `toolbox == false` 的工具、默认受众为 `NONE` | 已达成（T1，默认值已收紧） |
| A6 | `exec::invoke` 单测覆盖：非 object 输入、`Err` 归一化、超时归一化、超时对同步阻塞调用只保证「调用方返回」而不中断工作（用 `spawn_blocking` 构造） | 已达成（T2，5 个单测） |
| A7 | `fs_link` 迁移后回归：source 不存在时可创建、`source` 父目录不存在时错误信息明确、junction 用例在 Windows 通过（无特权/CI 环境允许 `#[ignore]` 或跳过，不得因此判 A1 失败）、`check` 对真实符号链接仍返回 `isLink = true`（叶子不被跟随）、`workspace_root` 位于受保护目录时被明确拒绝 | 已达成（T3，`fs_link` 10 个回归单测） |
| A8 | `permission_intents` 只产出 `read` / `edit` / `delete` / `execute` 四类 action（全 crate grep 校验） | 已达成（T3：仅 `edit`，无 `fs_link_create` / `fs_link_target`） |
| A9 | 生产 `ToolRegistry` 中工具数仍为 1（`fs_link`），无新增注册；`fs_link.audience()` 为 `TOOLBOX_ONLY`，故 `tool_definitions_for_tier()` 返回空列表 | 已达成（T1） |
| A10 | crate 内不存在 `unwrap_or_else(\|_\| path.to_path_buf())` 形式的规范化降级 | 已达成（T3，grep 校验无匹配） |

---

## 11 实施顺序

> 本规格已归档；下表只保留「顺序与前置」，不记录完成状态（阶段结论见 [roadmap](./I38-agent-roadmap.md) §2）。

| 阶段 | 内容 | 前置 |
|---|---|---|
| T1 | `definition.rs` + `registry.rs` 改造（F1/F2/F3）+ `trait.rs::ToolAudience`（O1）+ 单测 | 无 |
| T2 | `exec.rs` + `result.rs::from_exec_error`（F8）+ 单测 | T1 |
| T3 | `os/path.rs`（F4/F5/F6）+ 单测；`fs_link` 迁移并修 `permission_intents`（F7）+ 受众声明 | T2 |

T1 与 T2 不触碰任何工具，可独立提交、独立回滚。T3 是本阶段唯一的存量改动，涉及已上线能力，需单独回归。

> T1 含一处存量改动：`os_tool_exec` 由 `find()` 换为 `find_for_toolbox()`。因 `fs_link.toolbox == true`，对外行为零变化，但该行与 T1 同批提交，需在 review 中单独确认。

> **T1 实际交付内容**（与计划一致，另有两处计划外补强）：`registry.rs` 的 `tool_names()` 未排序（仅诊断用）；`ToolAudience::NONE` 无使用点，保留常量但不做运行时校验。`tool_names()` 排序仍未处理，留待后续需要确定性诊断输出时补。

> **T2 实际交付内容**：`exec.rs`（`invoke` + `DEFAULT_TOOL_TIMEOUT = 30s` + 5 个单测）、`result.rs::from_exec_error`、`lib.rs` 导出 `exec`、`Cargo.toml` 依赖调整。另有一处存量改动：`os_tool_exec` 由直接调 `execute()` 改为调 `invoke()`，因此 `execute` 的 `Err` 不再变成 Tauri `Err`，而是归一化为 `ToolResult { ok: false }` 返回给前端。**前端行为变化**：错误从 `invoke()` reject 变为 resolve 出 `OsResult { ok: false, error }`，`os.ts` 已按 `OsResult` 消费，无需改动。

> **T3 实际交付内容**（2026-11-04）：新建 `os/path.rs`（`resolve` / `resolve_leaf_preserving` / `canonicalize_lenient` / `reject_system_dirs` / `ensure_within_root`，20 个单测），`os/mod.rs` 导出 `pub mod path`；workspace `Cargo.toml` 新增 `dunce = "1"`，`assistant-tools` 以 workspace 依赖引入。`fs_link.rs` 删除私有 `resolve_path` / `validate_path`，`create` / `check` 的 `source` / `target` 全部改走 `resolve_leaf_preserving`，`permission_intents` 由 `fs_link_create` / `fs_link_target` 改为 `edit`（10 个回归单测，含 junction 与越界/受保护目录用例）。两处计划外补强：`\\?\UNC\` 在 `dunce::simplified()` 之上再归一到 `\\`（`dunce` 只处理 `VerbatimDisk`）；`workspace_root` 非绝对路径或不存在时给出明确错误（§4.4 语义约束的落实）。验收：`cargo test -p assistant-tools` 50 项全绿、`clippy --all-targets -D warnings` 无告警、`cargo fmt --all --check` 通过、`cargo test --workspace --no-run` 全量编译通过（`assistant-app` lib 测试二进制在本机启动即 `STATUS_ENTRYPOINT_NOT_FOUND`，属已记录的机器级限制，与本阶段改动无关）。

---

## 12 非目标

- 不实现任何新工具（含 `shell`）
- 不新增 Tauri command；`os_tool_exec` 仅把 `find()` 换为 `find_for_toolbox()`（过滤增强，对外行为不变）
- 不新增前端组件，不改 `os.ts`
- 不实现控制层（Policy / Gateway / Pipeline / RoundExecutor）
- 不实现 `ask_user`
- 不建 `os/output.rs`（无消费方）
- 不改 DB schema、不改 DTO
- 不引入 `thiserror`、不做工具结果落盘、不做并发批次编排

---

## 13 风险

| # | 风险 | 缓解 |
|---|---|---|
| R1 | `ToolDefinition` 与 `tool_definitions_for_tier` 在 P4 前无生产调用方 | 以 A5 强制测试覆盖；文档明确其「框架就绪」定位，不视为死代码 |
| R2 | `tokio::time::timeout` 需要运行时，且**只在 `await` 点生效** | 测试统一 `#[tokio::test(flavor = "multi_thread")]`；同步 IO 必须 `spawn_blocking`（见 §4.3），不得假设超时能中断线程 |
| R3 | `canonicalize` 在 Windows 返回 `\\?\` 前缀，影响 `mklink` | §5.2 强制简化；`fs_link` 迁移后回归 junction |
| R4 | `fs_link` 迁移属存量改动，可能引入回归 | T3 单独提交；A7 覆盖 source 不存在 / 父目录不存在两类 |
| R5 | 框架就绪后，后续工具可能绕过 `path.rs` 自行实现 | 在 I38c 写入「必须 import `os::path`」的硬约束，A10 以 grep 校验 |
| R6 | `DashMap` guard 跨 `await` 死锁 | §4.2 明确「先收集再 await」，列为 code review 检查项 |
| R7 | 默认值已收紧为 `NONE`（fail-closed），忘记声明不会再静默暴露；剩余风险是新工具漏声明后「功能不可见」而非「安全暴露」 | 文档约束新增工具必须显式声明；`setup.rs` 或 CI 校验「新增工具清单 vs 显式 audience 清单」，避免功能静默失效 |
| R8 | `invoke` 的超时对子进程无效（`shell` 将来会踩） | §4.3 明确「子进程必须进程级超时 + kill」；P5 实现 `shell` 时以进程组/Job Object 收口，禁止复用 `tokio::timeout` 了事 |
| R9 | A7 的 junction 用例依赖本机环境（开发者模式/权限策略） | 允许 `#[ignore]` 或运行时跳过，不因此判 A1 失败；symlink 用例同理 |
# I38c — P5：OS 工具能力规格

| 项 | 值 |
|---|---|
| 状态 | 现行（P5 规格索引；进度见 [roadmap](./I38-agent-roadmap.md) §2） |
| 日期 | 2026-11-04（2026-11-06 修订：C0 口径定稿、C6b 接入面按代码实测校正、读取回执键补 `session_id`、工具细节拆为逐工具分册；**同日移除 `shell`，改为暂不提供**，见 §13。2026-11-07 修订：新增 §14 多编码支持（跨工具约束，实现归 P11 候选、任务项 C9）；同日修订 §14.4 规则 4 与 §14.8——footer 由「`edit` 自动剥离」改为「不剥离、只在 0 匹配诊断里识别」，理由见该处） |
| 阶段 | P5（见 [I38 roadmap](./I38-agent-roadmap.md)） |
| 关联 | [I18 spec](./I18-agent-tool-architecture-spec.md) Step 2（已迁入本文）、[I31](./I31-phase1-tool-framework-spec.md) §5（共享路径）、[OSCALLDESIGN](../designed/OSCALLDESIGN.md)（Shell 洋葱）、[I38a](./I38a-tool-control-layer-spec.md)（Policy/Gateway/Pipeline）、[I18b](./I18b-tool-unified-bridge-spec.md)（ToolBox 桥接） |
| 影响范围 | `assistant-tools/src/os/*`；`src/tools/os.ts` 与 ToolBox 组件按 I18b 的 5 步流程 |

> 本文是 P5 的**唯一权威索引**：工具清单、共同结构、白名单解析、输出截断、验收标准、风险与开放问题只在此维护；**每个工具的安全标记、schema、行为、错误、参考来源拆到逐工具分册**（§5 / §6 / §7 的表格给出入口）。分册与本文冲突时以分册为准，并回写本文。
> **门禁**：P1（`os/path.rs`）与 P2（Policy/Gateway）未完成前，不得新增任何破坏性工具，也不得把 `delete` 加入生产注册表。

> 执行清单：[I38c-os-tools-tasklist.md](./I38c-os-tools-tasklist.md)（C0–C8 逐项勾选与验收映射；C9 为 P11 候选增量）。任务级进度只在该清单维护。

---

## 1 目标与非目标

### 1.1 目标

| # | 交付 |
|---|---|
| 1 | 只读工具：`read` / `ls` / `glob` / `grep` |
| 2 | 修改工具：`write` / `edit` / `mkdir` / `move` |
| 3 | 破坏性工具：`delete` |
| 4 | ~~命令执行：`shell` + 执行层洋葱守卫~~ **暂不提供**（决策与理由见 §13） |
| 5 | 输出截断基础设施 `os/output.rs`（第一个需要截断的工具落地时建） |
| 6 | 每个工具显式声明 `audience()`，并复用 `os/path.rs` 与 `exec::invoke` |
| 7 | **白名单工具解析** `resolve_available_tools`：替换 P4 的 `RegistryToolCatalog`（tier + audience 自动全量注入），改为 deny-by-default + 显式白名单/黑名单 |

### 1.2 非目标

- 控制层（P2 范围）；RoundExecutor（P4 范围，规格见 [I38b](./I38b-agent-loop-spec.md)）。阶段状态见 [roadmap](./I38-agent-roadmap.md) §2。
- 自由格式命令执行（`shell`）：**暂不提供**，理由与恢复条件见 §13。
- ToolBox 前端组件与审批 UI（P6）；本阶段只要求 Rust 侧可被 `os_tool_exec` 与 Pipeline 调用。
- `ask_user` 的完整交互：依赖 P6 输入框，见 §9。
- MCP（P7）。
- 文件内容语义摘要（I36 design §3.19 未采纳）。
- 白名单配置的前端 UI（AgentMode 编辑表单、对话工具栏、`ConversationToolConfigDto` 内存态）归 P6（见 [I38d §4](./I38d-agent-frontend-modes-spec.md)）；P5 只做后端 `resolve_available_tools` 与 `send_agent_message` 接入。
- 非 UTF-8 编码支持的**代码落地**：跨工具约束已在 §14 定稿，实现归 P11 候选（任务项 C9），不进入 P5 门禁。

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
| ~~`shell`~~ | — | — | — | — | **暂不提供**（§13） |
| `ask_user` | — | 是 | 否 | 否 | AGENT_ONLY |
| `fs_link` | — | 实际 `read_only=false` | 否 | 是 | TOOLBOX_ONLY |

**命名规范**：文件名是代码维护名，`Tool::id()` 是内部唯一标识（注册表键、白名单匹配、审批与审计、`os_tool_exec` 的 `tool` 参数），两者刻意分离；`Tool::name()` 是发给模型的调用名（`ToolSpec.name`），默认与 `id()` 同名，仅在需要内外命名分离时覆写。`register` 只接收工具本身，两个身份字段都由实现声明。

`agent_` 前缀是**受众标记**，不是通用目录前缀：只有 `audience().agent == true` 的工具（`BOTH` / `AGENT_ONLY`）才用 `agent_`，一眼即可看出哪些文件会被注入模型；`TOOLBOX_ONLY` 工具与 Agent 无关，不套该前缀。注册名保持纯动作词，模型侧不出现领域前缀。

| 文件（`assistant-tools/src/os/`） | 注册名（`Tool::id()`） | 旧名 |
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
| ~~`agent_shell.rs`~~ | — | 暂不提供（§13） |
| `agent_ask_user.rs` | `ask_user` | `ask_user` |

1. 注册名一律小写 snake_case、字母开头、≤64 字符，字符集 `[a-z0-9_]`；**禁用 kebab**（Gemini 方言不接受 `-`）。
2. 文件名与注册名解耦：改注册名只影响 `Tool::id()` 与调用方（前端 `os_tool_exec` 的 `tool` 参数），不影响文件组织。
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
危险组：delete
特殊：  os/output.rs（第一个需要截断的工具 = read）
```

**注册分批**：

| 批次 | 可注册内容 | 条件 |
|---|---|---|
| P5-R1 只读组 | `read` / `ls` / `glob` / `grep` | P1 完成；P2 的 validate 已就绪 |
| P5-R2 修改组 | `write` / `edit` / `mkdir` / `move` | P2 的 Gateway 审批可用 |
| P5-R3 危险组 | `delete` | P2 完整；前端组件同期上线（P6），不得先行注册 |

`os/mod.rs` 以 `register_readonly_tools` / `register_write_tools` / `register_dangerous_tools` 分组；`setup.rs` 只调用已确认的组。

**P11 增量（C9 多编码）的落地顺序**：`os/text_encoding.rs`（共享检测 / 编码模块）→ `read`（检测 + footer + 回执）→ `write`（五道闸）→ `edit`（footer 识别诊断 + 按原编码写回）→ `grep`（配置链 + 两层探测）。顺序由依赖决定：`read` 是唯一编码发现点，其余三个工具只消费回执或复用同一检测模块（§14.2）。

### 3.1 白名单工具解析（接入层替换）

P4 的 `RegistryToolCatalog` 使用 `tool_definitions_for_tier(MAX_TIER)` 做自动全量注入——只要工具注册了、tier 不超上限、audience 是 agent，就全部暴露给模型。这套方案没有白名单/黑名单，也没有 deny-by-default。

P5 在工具实现完成后，新增 `resolve_available_tools` 替换 P4 的接入逻辑，同时删除 `RegistryToolCatalog`。

**函数签名**（落点 `src-tauri/src/adapters/tool_resolver.rs`）：

```rust
pub async fn resolve_available_tools(
    registry: &ToolRegistry,
    enabled_tools: &[String],
    disabled_tools: &[String],
    max_tier: ToolTier,
) -> Vec<ToolSpec> {
    // 1. 层级硬上限 + 受众过滤：ToolRegistry::tool_definitions_for_tier(max_tier).await
    //    → Vec<ToolDefinition>（已按注册名排序）
    // 2. deny-by-default：enabled_tools 为空 → 空集
    // 3. 白名单过滤：definition.id in enabled_tools
    // 4. disabled_tools 优先于 enabled_tools
    // 5. ToolDefinition → ToolSpec（原 tool_catalog.rs 的 into_tool_spec 搬入本文件）
}
```

**为什么是 async**：`Tool::description()` 是 `async`，`ToolRegistry::tool_definitions_for_tier` 也是 `async fn`（`assistant-tools/src/registry.rs`）。解析器只是在其结果上做名单过滤，因此必须保持 `async`。调用点 `send_agent_message` 本身是 async 命令，额外 `await` 无运行时代价。

**复用既有过滤**：`tool_definitions_for_tier(max_tier)` 已实现「`tier <= max_tier` + `audience().agent == true` + 按注册名排序」，即上表第 1、2 步与确定性排序；它返回 `Vec<ToolDefinition>`，而 `TurnSpec.tools` 需要 `Vec<ToolSpec>`（`assistant_providers::types::ToolSpec`），因此转换函数 `into_tool_spec` 随 `tool_catalog.rs` 一并搬到 `tool_resolver.rs`。解析器只补 deny-by-default / 白名单 / 黑名单，不再重复实现层级与受众判断，也避免两处过滤口径分叉。

**规则**：

| 步 | 规则 | 实现位置 | 说明 |
|---|---|---|---|
| 1 | 层级硬上限 | `tool_definitions_for_tier` | `tool.tier() <= max_tier`；`External` 在 P5 恒不注入 |
| 2 | 受众过滤 | `tool_definitions_for_tier` | `tool.audience().agent == true`；`TOOLBOX_ONLY` 不出现在此 |
| 3 | deny-by-default | 解析器 | `enabled_tools` 为空数组 → 返回空 `Vec`，模型看不到任何工具 |
| 4 | 白名单匹配 | 解析器 | 仅 `enabled_tools` 中出现的注册名通过 |
| 5 | 黑名单覆盖 | 解析器 | 同一工具同时出现在两个列表 → 禁用；`disabled_tools` 是最终否决 |

第 1、2 步与排序由 `ToolRegistry::tool_definitions_for_tier` 承担（P2 交付，`assistant-tools/src/registry.rs`），解析器不复制该逻辑。

**与 P4 的对比**：

| | P4 `RegistryToolCatalog` | P5 `resolve_available_tools` |
|---|---|---|
| 过滤方式 | `tier + audience` 自动 | `tier + audience + enabled_tools + disabled_tools` |
| 默认行为 | 注册了就可见 | deny-by-default，空白名单 = 无工具 |
| 配置来源 | 无（硬编码 `MAX_TIER = Os`） | `SendChatMessageArgsDto.tool_config`（前端内存态，每次请求携带） |
| `ToolCatalog` trait | 调用 `agent_tools()` | **不再使用 `ToolCatalog` trait**，改为 async 函数直接调用 `tool_definitions_for_tier` |

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
)
.await;
spec.tools = Some(tools);
```

**空集归一化留在 `request::resolve_tools`**：`kernel/request.rs` 已有 `resolve_tools(&[ToolSpec]) -> Option<Vec<ToolSpec>>`，空集即 `None`（循环退化为单次调用），且有单测覆盖。接入层不再重复判空，`spec.tools = Some(vec![])` 与 `None` 语义等价。

注意 `start_turn`（`src-tauri/src/cmd/agent/mod.rs`）对 `SendChatMessageArgsDto` 做**穷尽解构**，新增字段后必须显式忽略（`tool_config: _` 或 `..`），否则编译失败；`chat` 链路复用同一 `start_turn`，其 `TurnSpec::chat()` 的 `tools` 恒为 `None`，不受影响。

**`tool_config = null` 的语义**：P5 后端不认识 AgentMode（请求里没有 `modeId`），因此 `None` 一律按 `ConversationToolConfigDto::default()` 处理，且空白名单即 deny-by-default → 空工具集（模型看不到任何工具）。「null 时按会话所属 AgentMode 预设解析」由 P6 保证：前端恒从内存态构造并下发 `toolConfig`，不下发即等价于「无工具」。`Default` 取值：`max_tool_tier = Os`、`approval_level = DangerousOnly`（fail-closed 兜底，P6 显式下发覆盖）、两个名单为空。

**清理范围**（P5 完成时删除）：

| 删除项 | 位置 |
|---|---|
| `RegistryToolCatalog` 结构体与 impl（含 `MAX_TIER`） | `src-tauri/src/adapters/tool_catalog.rs`（整文件；其中 `into_tool_spec` **搬入** `tool_resolver.rs`，不是删除） |
| `mod tool_catalog;` 与 `pub use tool_catalog::RegistryToolCatalog` | `src-tauri/src/adapters/mod.rs` |
| `use crate::adapters::RegistryToolCatalog;` | `src-tauri/src/cmd/agent/mod.rs` |
| `ToolCatalog` async trait（P4 唯一实现者即 `RegistryToolCatalog`） | `assistant-agent/src/kernel/ports.rs` |
| `TurnSpec.tools` 类型由 `Option<Arc<dyn ToolCatalog>>` 改为 `Option<Vec<ToolSpec>>` | `assistant-agent/src/kernel/mod.rs`（字段 + `chat()` / `agent()` 默认值）、`kernel/runner.rs`（`TurnRunCtx.tools` 赋值；`spec.tools.as_ref()` 改为 `.as_ref().map(Vec::as_slice)`）、`kernel/request.rs`（`build()` 签名由 `Option<&Arc<dyn ToolCatalog>>` 改为 `Option<&[ToolSpec]>`；`resolve_tools` 保留，去掉 `agent_tools().await`） |
| `TurnRunCtx.tools` / `ToolExecCtx.tools` 同步收窄 | `assistant-agent/src/agent/round_executor.rs`（两处字段声明）、`agent/loop_executor.rs`（`request::build(ctx.tools.as_deref())`、`ToolExecCtx` 构造） |
| `ToolExecCtx.tools` 字段删除（P4 遗留未使用字段：生产适配器 `PipelineToolExecution` 只读 `observer` / `turn_id` / `iteration` / `conversation_id`，从不读 `tools`） | `assistant-agent/src/agent/round_executor.rs`、`agent/loop_executor.rs` |
| `test_doubles.rs` 中 `StaticToolCatalog` 替身 | `assistant-agent/src/kernel/ports/test_doubles.rs` |
| 测试调用点同步 | `assistant-agent/tests/loop_executor.rs`（`catalog` 构造、`spec.tools`、`TurnRunCtx.tools`）、`assistant-agent/tests/kernel_request_tools.rs`（`StaticCatalog` + `request::build` 三处） |

**保留不动**：

- `ToolSpec` 类型（`name` / `description` / `input_schema`）——P4 C2.1 定义，白名单解析仍产出它
- 三个 provider adapter 的 `ToolSpec` → 方言转换（OpenAI / Anthropic / Gemini）——P4 C2.10–C2.16
- `ChatOptions.tools` 字段——P4 C2.2，仍由 `TurnSpec.tools` 填入
- RoundExecutor 的 `ToolExecutionPort`——P4 C3.6，与工具注入无关

**DTO 扩展**（P5 新增）：

`SendChatMessageArgsDto`（`src-tauri/src/protocol/dto/chat.rs`，`Deserialize` + `TS`）新增 `tool_config: Option<ConversationToolConfigDto>`：

```rust
// src-tauri/src/protocol/dto/agent.rs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "snake_case")]
pub enum ToolTierDto {
    Sandbox,
    Os,
    External,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalLevelDto {
    None,
    DangerousOnly,
    AllOsTools,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationToolConfigDto {
    pub max_tool_tier: ToolTierDto,
    pub approval_level: ApprovalLevelDto,
    pub enabled_tools: Vec<String>,
    pub disabled_tools: Vec<String>,
}
```

- `Option` 输入字段加 `#[ts(optional = nullable)]` + `#[serde(default)]`（项目输入 DTO 规则）。
- 命名一律 `*Dto` 后缀；`ToolTier` 在 `assistant-tools` 已有同形枚举（`Sandbox` / `Os` / `External`），DTO 侧另建 `ToolTierDto` 并提供 `From` 双向转换（与 `ToolErrorKindDto` 同法），**不**把 `assistant-tools` 的类型直接暴露到 wire。
- `ToolTierDto` 用 `lowercase` 或 `snake_case` 均可（单词值同形）；`ApprovalLevelDto` 必须 `snake_case`，与 [I38d §4.2](./I38d-agent-frontend-modes-spec.md) 的 `'none' | 'dangerous_only' | 'all_os_tools'` 对齐（仓库已有同类先例 `ConversationListChangeKindDto` 用 `snake_case`）。
- `approval_level` 在 P5 **只承载不消费**：审批级别 → Policy 规则的映射是 P6 的职责（I38d §4.6），P5 只保证字段随契约往返。
- 执行 `npm run contract:gen` 生成前端类型，并在 `src-tauri/tests/protocol_bindings.rs` 的 `declarations` 数组补三个新类型（该测试会扫描 `protocol/dto/*.rs` 的 `TS` 派生类型，漏声明即失败）。

**P6 的关系**：P5 只做后端解析与 `send_agent_message` 接线；前端 UI（AgentMode 编辑表单、对话工具栏、`ConversationToolConfig` 内存态初始化与传递）仍在 P6（见 [I38d §4](./I38d-agent-frontend-modes-spec.md)）。P6 的 `resolveAvailableTools`（I38d §4.5）后端部分已由 P5 完成，P6 只做前端调用与 UI 绑定。

---

## 4 每个工具的共同结构

```rust
pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn id(&self) -> &str { "read" }
    async fn description(&self) -> Result<String, String>;
    fn input_schema(&self) -> Value;
    fn tier(&self) -> ToolTier { ToolTier::Os }
    fn audience(&self) -> ToolAudience { ToolAudience::BOTH }
    fn behavior_hints(&self) -> BehaviorHints;
    fn validate_input(&self, input: &Value, ctx: &ToolUseContext) -> ValidationResult;
    fn permission_intents(&self, input: &Value, ctx: &ToolUseContext) -> Result<Vec<PermissionIntent>, String>;
    async fn preview_for_approval(&self, input: &Value, ctx: &ToolUseContext) -> Option<ApprovalPreview>;
    async fn execute(&self, input: &Value, ctx: &ToolUseContext) -> Result<ToolResult, String>;
}
```

`id()` 是内部唯一标识（注册表键、白名单匹配、审批与审计、`os_tool_exec` 的 `tool` 参数），必须由工具自身声明；`name()` 是发给模型的调用名，默认返回 `id()`，只有需要内外命名分离时才覆写。注册只接收工具本身（`register(tool)`），身份不从注册处传入，因此不存在「注册点与实现两处同步」的错配。

### 4.1 审批预演（`preview_for_approval`）

P5 各分册要求「审批卡片展示量级」，但量级只能来自审批**前**的一次只读预演——`execute()` 的结果 `data` 在审批时尚未产生，**不得**用它充当预演信息。契约与超时语义见 [I38a §2.8.1 / §3.1](./I38a-tool-control-layer-spec.md)。

预演返回 `ApprovalPreview`：`summary`（后端格式化的一句话）+ `truncated` + `estimated`。**不返回固定的 `size` 字段**——各工具的量纲不同（条目数 / 字节数 / 行数 / 替换次数），由 `summary` 按工具语义自行表达；需要结构化补充时走 `extensions`，不扩 `preview`（见 [I38a §2.8.1](./I38a-tool-control-layer-spec.md)）。

| 工具 | 预演内容（`summary`） | 成本 | 参考实现 |
|---|---|---|---|
| `delete` | 「删除 N 个条目（X MB）」；超上限时 `truncated = true`（复用 [delete §3.3](./I38c-delete-spec.md) 的预扫描） | 一次目录遍历，上限 `DELETE_SCAN_LIMIT` | 把 §3.3 的扫描抽成可复用函数，`execute` 与 `preview` 共用 |
| `write` | 「覆盖 1 个现有文件（当前 N 行 / X KB）」；新建则「新建文件（N 行 / X KB）」 | `metadata` + 行计数（**不读全文内容**） | 与 `write` 的「覆盖前检查」共用同一次统计 |
| `edit` | 「替换 N 处（约 M 行变更）」；`estimated = true` | 零 IO（参数推导） | diff 已在 `arguments` 中展示，预演只补量级 |
| 只读组 / `mkdir` / `move` | 不实现（默认 `None`） | — | `mkdir` 的「目标已存在」由 `validate_input` 拒绝，无需预演 |

**约束**：

- 预演与 `execute` **必须复用同一套路径解析与统计函数**，禁止「展示 A、执行 B」。
- 预演超时 2s 或出错 → `None`，审批继续；`None` 在卡片上表示「未知」，**不得**渲染成 `0`。
- 预演不得写入 `ReadReceiptStore`、不得写 grant、不得创建/删除任何文件。
- `delete` 的预扫描与实际删除之间目标可能变化（TOCTOU），预演数字只用于文案，**以 `execute` 返回为准**。

**共享基础设施**（定义与算法见 I31 §5，不在此重复）：

- `resolve()` / `resolve_leaf_preserving()` / `canonicalize_lenient()` / `reject_system_dirs()` / `ensure_within_root()`。
- `execute()` 内**只能**使用 `resolve()` 的返回值做 IO，禁止「校验 A、使用 B」。
- 所有 `Err` 先由 `exec::invoke` 归一化；工具内部只返回业务失败（`ToolResult::err`）。
- `ReadReceiptStore`（`assistant-tools/src/storage.rs`）：`lookup` / `record` / `invalidate_path`；破坏性工具（`delete` / `move`）成功后必须失效受影响路径（见 [delete §3.6](./I38c-delete-spec.md)）。

> **受众与路径约束的关系**：本表工具受众为 `BOTH`，两个入口都会调用同一个 `execute()`。`reject_system_dirs` 等**绝对不变量**两侧都生效；`ensure_within_root` 只在 `workspace_root` 有值时执行，而 ToolBox 入口恒为 `None`，因此**各分册错误表里的「路径越界」只在 Agent 入口发生**。ToolBox 由用户本人调用，不存在越界概念（[I18b](./I18b-tool-unified-bridge-spec.md) §2.2）。

**权限词汇表**（与 P2 一致）：

| action | 语义 | 资源 |
|---|---|---|
| `read` | 读取内容 | 目标路径 |
| `edit` | 创建 / 修改 / 移动 | 每个受影响路径各一条 |
| `delete` | 删除 | 目标路径 |
| `execute` | 执行命令 | 命令基名 |

---

## 5 只读工具

逐工具分册（本索引只维护清单与公共约束，工具细节不在此重复）：

| 工具 | 分册 | 任务项 |
|---|---|---|
| `read` | [I38c-read-spec.md](./I38c-read-spec.md) | C2.7–C2.13 |
| `ls` | [I38c-ls-spec.md](./I38c-ls-spec.md) | C2.14–C2.17 |
| `glob` | [I38c-glob-spec.md](./I38c-glob-spec.md) | C2.18–C2.22 |
| `grep` | [I38c-grep-spec.md](./I38c-grep-spec.md) | C2.23–C2.27 |

四个工具的安全标记、`input_schema`、行为、错误与参考来源统一在上表分册内维护；跨工具的约束（§4 共同结构、§8 输出截断）只在本文维护。`read` / `grep` 的编码参数与行为见 §14 与各分册的编码小节（C9 / P11 候选）。

---

## 6 修改工具

| 工具 | 分册 | 任务项 |
|---|---|---|
| `write` | [I38c-write-spec.md](./I38c-write-spec.md) | C3.5–C3.7 |
| `edit` | [I38c-edit-spec.md](./I38c-edit-spec.md) | C3.8–C3.13 |
| `mkdir` | [I38c-mkdir-spec.md](./I38c-mkdir-spec.md) | C3.14–C3.16 |
| `move` | [I38c-move-spec.md](./I38c-move-spec.md) | C3.17–C3.21 |

`edit` 是全阶段风险最高的工具（匹配与写回做错会静默损坏文件），其匹配流水线、结果分支、写回约束、读前置 guard 全部在 [I38c-edit](./I38c-edit-spec.md) 内定义，实现时以该分册为准。`write` / `edit` 的编码继承与写回规则见 §14.5 / §14.6 / §14.8。

---

## 7 破坏性工具

| 工具 | 分册 | 任务项 |
|---|---|---|
| `delete` | [I38c-delete-spec.md](./I38c-delete-spec.md) | C4.1–C4.11 |
| ~~`shell`~~ | — | **暂不提供**，见 §13 |

### 7.1 危险组注册门禁

`delete` 满足以下全部条件前**不得**加入生产注册表：

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

参考形态（OpenBitFun `assembly/core/src/agentic/tools/implementations/ask_user_question_tool.rs`）：

```json
{
  "type": "object",
  "properties": {
    "questions": {
      "type": "array",
      "minItems": 1,
      "maxItems": 4,
      "items": {
        "type": "object",
        "properties": {
          "question": { "type": "string" },
          "header": { "type": "string", "maxLength": 20 },
          "options": {
            "type": "array",
            "minItems": 2,
            "maxItems": 10,
            "items": {
              "type": "object",
              "properties": { "label": { "type": "string" }, "description": { "type": "string" } },
              "required": ["label", "description"],
              "additionalProperties": false
            }
          },
          "multiSelect": { "type": "boolean", "default": false }
        },
        "required": ["question", "header", "options"],
        "additionalProperties": false
      }
    }
  },
  "required": ["questions"],
  "additionalProperties": false
}
```

- 语义：暂停 Agent Loop，等待前端回答，再继续。1–4 个问题、每题 2–10 个选项；「其他」选项由前端自动提供，schema 不声明（避免模型自己造 `Other` 与前端重复）。
- 结构化形态（而非单个 `question` 字符串）的理由：选项需要 label + description 才能表达权衡，纯字符串会把「选项 + 说明」塞进一段自然语言，前端无法渲染成可点选的输入框。
- `audience = AGENT_ONLY`（不从前端透传入口暴露）。
- 依赖 P2 的审批端口与 P6 输入框；**不属于 P5 退出条件**，P5 只登记形态与阻塞点。
- **超时（已定稿，见 §11 OQ5）**：采纳参考实现的「工具自管超时」形态。`Tool` trait 增加 `fn manages_own_execution_timeout(&self) -> bool { false }`（默认 `false`，存量工具零改动）；`exec::invoke` 在套统一 deadline 前先查该标记，返回 `true` 时**不套** `DEFAULT_TOOL_TIMEOUT`，改由工具自身的 deadline 收口。超时值来自全局配置 `user_question_timeout_secs`（`None` → 默认 180s；`Some(0)` → 不设 deadline，只由取消终止），schema 不声明超时参数。
- 绕过统一超时**不等于**不可中断：turn 子令牌的取消仍在 `exec::invoke` 外层生效（`cancellable` 包装），用户停止或链路切换时立即终止等待。
- 超时按用户未回答处理（返回 `Cancelled` 或业务错误，不得默认继续）。

---

## 10 验收标准

| # | 标准 |
|---|---|
| A1 | 所有文件工具只通过 `os::path` 解析路径；crate 内 grep 无 `unwrap_or_else(\|_\| path.to_path_buf())` |
| A2 | 每个工具都有 `input_schema` 单测（合法 / 缺必填 / 类型错误） |
| A3 | 只读工具 `permission_intents` 为空、`is_concurrency_safe = true`；修改/删除工具产出规定的 action |
| A4 | `edit` 的 0 / 1 / 多匹配三分支行为符合 [I38c-edit §3 / §4](./I38c-edit-spec.md)；渐进匹配 6 档每档各有用例；歧义时不自动取首个；CRLF 文件写回后仍为 CRLF 且无混合行尾；写回原子（临时文件 → `sync_all` → rename）且崩溃 / 断电不留半截或空文件；审批载荷含 diff。档 2 / 档 4 依赖 `read` 的行号前缀与 ` [truncated]` 标记，契约见 [I38c-read §3.2](./I38c-read-spec.md)，两侧共用同一常量并有对拍用例 |
| A4b | `edit` 读前置：无回执 → 拒绝并提示 `read`；回执修订号过期 → 拒绝并提示重读；`write` 后紧接 `edit` 不被误拦（[I38c-edit §6](./I38c-edit-spec.md)） |
| A4c | `write` 覆盖走临时文件 → `sync_all` → rename；内容与现有文件逐字节相同时不写盘且 mtime 不变；已存在但不可读的文件拒绝覆盖（[I38c-write §3](./I38c-write-spec.md)） |
| A5 | `delete` 拒绝 workspace_root / 文件系统根 / 家目录本身；链接不跟随（删链接不删目标）；递归预扫描给出条目数与字节数且超限不拒删；结果含 `path` / `isDirectory` / `recursive` / `entriesDeleted` / `bytesDeleted` / `truncated`；批准只对规范化路径生效（[I38c-delete §3](./I38c-delete-spec.md)） |
| A5b | 删除 / 移动成功后 `ReadReceiptStore::invalidate_path` 按组件级前缀失效，删除后重建的文件 `read` 不回放旧内容（[I38c-delete §3.6](./I38c-delete-spec.md)） |
| A6 | ~~`shell` 守卫与进程级 kill~~ **不适用**：`shell` 暂不提供（§13） |
| A7 | 分组注册：默认 `setup.rs` 只注册 P5-R1；危险组未调用时生产注册表不包含 `delete` |
| A8 | `read` 的读取回执：同一会话内修订号不变 → 复用提示；修订号变更 → 重读；不同会话读同一文件互不抑制 |
| A9 | `cargo test -p assistant-tools` 全绿；`cargo clippy -- -D warnings` 无告警 |
| A10 | `resolve_available_tools`：空白名单 → 空集；白名单生效；`disabled_tools` 覆盖 `enabled_tools`；超 `max_tool_tier` 与 `TOOLBOX_ONLY` 工具不出现；同输入两次调用顺序一致 |
| A11 | P4 遗留清理到位：`RegistryToolCatalog` / `ToolCatalog` trait / `StaticToolCatalog` 全部删除，`ToolExecCtx.tools` 未使用字段删除；`cargo test --workspace` 全绿 |
| A12 | **（C9）** 编码检测顺序固定为 BOM → 显式 `encoding` → 二进制判定 → UTF-8 整文件校验 → chardetng 猜测（§14.3）；带 BOM 的 UTF-16 文件不被二进制检测误杀；`had_errors == true` 一律拒绝，不存在 `?` 替换式解码或写回 |
| A13 | **（C9）** footer 契约（§14.4）：UTF-8 无 BOM 静默；其余输出 `[encoding: <label>]`，自动检测加 `(guess)`、BOM 来源加 `(bom)`；footer 独占一行、单次结果至多一次、不进入 `data` 正文；`edit` 不剥离，0 匹配时按标记集给出诊断 |
| A14 | **（C9）** 回执携带编码（§14.5）：`ReadReceipt` 含 `encoding` / `has_bom`；`edit` / `write` 只消费不重判；同一文件在 `read` 与 `edit` 上不会得出两种编码结论 |
| A15 | **（C9）** `write` 转码五道闸（§14.6）全部生效：编码确定、内容可完整编码、≤1MB、已存在文件须有回执、BOM 按目标编码写；任一道不过即不写盘 |
| A16 | **（C9）** `grep` 复用 `grep-searcher` 标准配置链（§14.7）：`binary_detection = quit(b'\x00')`、BOM sniffing 默认开、无 BOM 零转码；`data` 含 `binarySkipped` / `largeSkipped` / `unreadableFiles` / `lossyLines` / `nonUtf8Probe`；零命中时前缀探测有界且跳过 BOM / 二进制文件；`encoding` 已设时不执行 probe |
| A17 | **（二修）** `grep` 输出预算与可中断性（[grep §3.2 / §3.3](./I38c-grep-spec.md)）：`context` 超出 `0..=10` 即 `Business`；`MAX_MATCHES` / `MAX_OUTPUT_LINES`（命中 + 上下文合计）任一触发即停并置 `truncated`；deadline 在 `matched` 与 `context` 回调内均检查；`max_filesize = 16 MiB`；`pattern` 原样编译不 `trim`；`include` 与 `glob` 同为 `literal_separator(true)`；二进制文件整文件丢弃、读取失败计入 `unreadableFiles` |

---

## 11 风险与开放问题

| 风险 | 缓解 |
|---|---|
| 工具绕过 `os/path.rs` 自行实现 | A1 grep；code review |
| 破坏性工具提前注册即前端可达 | 分组注册 + §7.1 门禁；I31 §8 |
| 同步 IO 超时无法中断 | `spawn_blocking`；超时只保证调用方及时返回（I31 §4.3）。`grep` 在此之上加了遍历内 deadline（外层剩余预算 − 500ms），检查点覆盖「每文件开始前」与 `matched` / `context` 两个回调，命中即停并返回已收集结果，不依赖强杀 blocking 线程（[grep §3](./I38c-grep-spec.md)）；单文件粒度由 `max_filesize = 16 MiB` 收口（[grep §3.3](./I38c-grep-spec.md)） |
| `read` 输出格式漂移让 `edit` 档 2 / 档 4 静默失效 | 行号前缀（`{n:>6}\t`）与 ` [truncated]` 标记的唯一产出点是 `read`，两侧共用同一常量；契约写在 [read §3.2](./I38c-read-spec.md)，`edit` 只做检测与剥离 |
| `ls` 不读 `.gitignore`、`glob` / `grep` 读 `.gitignore`，口径不一致 | 刻意差异（理由见 [ls §3](./I38c-ls-spec.md)）；在三个工具的 `description` 里各自写明「是否受 `.gitignore` 影响」，避免模型把「`ls` 没列出来」当成「文件不存在」 |
| 大结果挤爆上下文 | P2 三层预算 + `os/output.rs` 单条上限；`grep` 另设自身预算（`MAX_MATCHES` / `MAX_OUTPUT_LINES` / `MAX_CONTEXT`，[grep §3.2](./I38c-grep-spec.md)）——上下文行不经过 `MAX_MATCHES`，必须单独计行 |
| Windows 路径形态（UNC / 盘符相对） | I31 §5.2.1 全覆盖；A1 只允许走共享函数 |
| 符号链接 / junction 让删除作用到 workspace 之外 | 目标分类一律 `symlink_metadata`（不跟随），链接按文件删除、递归不进入链接（[delete §3.2](./I38c-delete-spec.md)） |
| 递归删除无法中断，可能部分删除 | `remove_dir_all` 无取消点，超时只记录；失败后重查目标存在性并区分「未删除 / 已删除但随后失败」（[delete §3.4](./I38c-delete-spec.md)） |
| 删除 → 重建后 `read` 回执误判「已读过」（保时间戳的复制 / 解包使长度 + mtime 相同） | 删除 / 移动成功后 `invalidate_path` 按组件级前缀失效（[delete §3.6](./I38c-delete-spec.md)、[I38a §4-P2-C6](./I38a-tool-control-layer-spec.md)） |
| 崩溃 / 断电后 rename 已完成但内容未落盘（文件存在却为空） | 写临时文件后先 `sync_all()` 再 rename（[write §3](./I38c-write-spec.md)、[edit §5](./I38c-edit-spec.md)），不依赖 rename 单独保证持久性 |
| 跨会话并发写同一路径，后写者静默覆盖前者 | 原子 rename 只保证不产生半截文件、不保证先后顺序；P5 **不引入** per-target 锁（需在 `os/` 层加跨调用共享状态），冲突可见性由审批卡片（覆盖提示 + 现有行数 / 字节数）与 `read` 回执修订号承担（[write §3 / §5](./I38c-write-spec.md)） |
| （C9）非 UTF-8 文件被误判编码后写回，静默损坏内容 | 检测顺序固定（BOM → 显式 `encoding` → 二进制 → UTF-8 校验 → chardetng），猜测仅在「置信 + 整文件解码无损」时接受；写回前强制「解码 → 编码」往返无损（§14.3 / §14.6 G2） |
| （C9）二进制文件被当成「非 UTF-8 文本」报给模型 | `grep` 主遍历 `quit(b'\x00')`；`read` 在猜测前查原始字节前 8KB 的 `0x00`（§14.3 步骤 3）；probe 跳过含 `0x00` 的文件（§14.7） |
| （C9）footer 被模型复制进 `edit` 的 `old_string`，导致 0 匹配 | footer 是结构化已知后缀（`[encoding: …]` 独占一行），但**不自动剥离**：自动剥离会把模型误解静默修正为成功，模型学不到「footer 不是文件内容」。改为 0 匹配诊断精确指出标记，由模型自行移除；行号前缀因每行必有、模型无法避开，仍走档 2 自动剥离（§14.4 规则 4） |
| （C9）编码结论分叉（`read` 判 GBK、`edit` 判 UTF-8） | 唯一发现点原则：检测只在 `read` 做，`edit` / `write` 只消费回执；显式参数与 BOM 的优先级固定（§14.2 / §14.3） |
| （C9）`grep` 设 `encoding` 后全 scope 转码，改变 UTF-8 文件的匹配语义 | `encoding` 是 scope 级显式开关，语义与 ripgrep `--encoding` 一致；未设置时无 BOM 走 `utf8_passthru` 零转码（§14.7） |

**开放问题**（编号即引用名，改动需同步全文引用）：

- **OQ1** ~~`grep` 是否引入 `ignore` crate 以遵守 `.gitignore`？~~ **已定稿（2026-11-06）**：引入。`glob` 用 `ignore` + `globset`，`grep` 用 `grep-searcher` + `grep-regex`，全部为 ripgrep 官方组件，语义一致且无 CLI 依赖。
- **OQ2** ~~文件工具的超时策略如何统一？~~ **已定稿（2026-11-06）**：沿用 `exec::invoke` 的统一 deadline 保证调用方及时返回，工具内部不自行强杀；`spawn_blocking` 任务无法中断，因此需要「提前停止」的工具（`grep`）在遍历循环内按「外层剩余预算 − 500ms」自查 deadline，命中即停并返回已收集结果（[grep §3](./I38c-grep-spec.md)）。`delete` 的 `remove_dir_all` 无取消点，只记录超时并按失败后重查处理（见上表）。
- **OQ3** ~~`shell` 的 Job Object 实现是否需要 `windows-sys` 新依赖？~~ **已作废（2026-11-06）**：`shell` 改为暂不提供（§13），该依赖问题随之关闭。若将来恢复，原结论可复用：不引入 `windows-sys`，用现有 `windows` 0.62 补 `Win32_System_JobObjects` / `Win32_System_Threading` features，Unix 侧用 `libc` 进程组 kill。
- **OQ4** **`edit` 是否支持单次多 hunk（codex `apply_patch` 形态）？** 已定稿（2026-11-06）：**P5 不做**。理由：多 hunk 会扩大工具面与审批粒度（一次审批覆盖多处改动），且需要独立的补丁语法解析器；P5 保持「一次调用一次替换」，模型可用多次 `edit` 达成同一效果。若 P6 之后出现「批量重构」痛点，再按新工具 `apply_patch` 单列。
- **OQ5** ~~`ask_user` 如何避开 30s 统一工具超时？~~ **已定稿（2026-11-07）**：采纳路径 (b)「工具自管超时标记」。参考实现（OpenBitFun `ask_user_question_tool.rs`）的做法是 `manages_own_execution_timeout() = true` + 外部配置 `user_question_timeout_secs`（schema 不声明超时参数），本项目照搬：

  | 项 | 定稿 |
  |---|---|
  | trait 增量 | `Tool::manages_own_execution_timeout(&self) -> bool`，默认 `false`；存量工具零改动 |
  | `exec::invoke` | 先查标记：`true` → 不套 `DEFAULT_TOOL_TIMEOUT`，直接 `execute()`；`false` → 保持 30s 统一 deadline |
  | 可中断性 | turn 子令牌的 `cancellable` 包装在 `exec::invoke` 外层，绕过统一超时后取消仍立即生效 |
  | 配置 | 新增 `AppConf.user_question_timeout_secs: Option<u32>`；`None` → 180s，`Some(0)` → 不设 deadline |
  | 超时结果 | 返回 `Cancelled`（与用户取消同形），turn 继续，模型看到「用户未回答」 |

  不采纳路径 (a)「`timeout_hint` 返回 `None`」：它让「无 deadline」与「工具自管 deadline」共用同一个 `None`，`exec::invoke` 无法区分「真的不该超时」与「工具自己会收口」，也无法为后者注入配置值。实现归 P6（[I38d §2.4 / §5 P6-C8](./I38d-agent-frontend-modes-spec.md)）。
- **OQ6** `write` 的「覆盖」是否应标记 `destructive`？当前倾向：否，但审批卡片必须显示覆盖提示。
- **OQ7** **（C9）大文件转码写回**：>1MB 的非 UTF-8 文件 `read` 可读，但 `edit` / `write` 转码写回被 §14.6 G3 拒绝。当前结论：**不做**，错误文案提示先外部转码再重读。若将来要做，必须同时解决「流式解码 + 流式编码 + 原子写 + 编码边界与 `sync_all` 的交互」，成本与 P5 的 1MB 上限不是一个量级。
- **OQ8** **（C9）chardetng 置信度阈值**：不引入可调阈值。接受条件固定为「`guess_assess` 的 `is_confident == true` **且** 整文件解码无损」，两个条件都是二值判定，没有需要调参的旋钮；若实践中误判偏多，只允许收紧为「额外要求猜测结果经往返无损」，不允许放宽到「低置信也接受」。
- **OQ9** **（C9）回执里的 `encoding` 是否参与修订号 / 失效判定？** 当前结论：**否**。`revision` 仍是「长度 + mtime」，编码变化必然伴随字节变化（长度或 mtime 变化），无需额外维度；「等长等时戳的编码转换」与「删除后重建」同类，由 `invalidate_path` 兜底（§14.5 / §14.9）。

---

## 12 参考

> 路径根：OpenBitFun = `references/1-ai-agent/OpenBitFun/src/crates/`；codex = `references/1-ai-agent/codex/codex-rs/`。
> 工具级逐项采纳 / 不采纳记录在各自分册的「参考」节；本表只保留索引级条目。

| 要素 | 参考项目 | 参考实现 | 去向 |
|---|---|---|---|
| Read / LS / Glob / Grep | OpenBitFun | `implementations/{file_read,ls,glob,grep}_tool.rs`、`execution/tool-execution/src/{fs/read_file,fs/listing,search/glob_search,search/grep_search}.rs` | [read](./I38c-read-spec.md) §5、[ls](./I38c-ls-spec.md) §5、[glob](./I38c-glob-spec.md) §5、[grep](./I38c-grep-spec.md) §5（含逐项采纳 / 不采纳：`tail`、64000 字符总上限、mtime 排序、`EXCLUDED_NAMES` + `.gitignore`、绝对 pattern 推导搜索根、`output_mode` 三态、`head_limit` 250 等） |
| Write | OpenBitFun + deepseek-harness-orb | `assembly/core/src/agentic/tools/implementations/file_write_tool.rs`、`packages/fs/fs-local/README.md` | [write](./I38c-write-spec.md) §5（幂等成功 / 不可读拒绝覆盖 / 原子写 + `fsync` 采纳；单字符串 `payload` 形态、`force` 逃生口、写前读前置、200 行 / 20KB 软限制、版本守卫覆盖、per-target 锁不采纳） |
| Delete | OpenBitFun + codex + openhuman + deepseek-harness-orb | `implementations/delete_file_tool.rs`、`execution/tool-execution/src/fs/delete_path.rs`、`apply-patch/src/lib.rs`、`security/policy/path_checks.rs`、`packages/fs/fs-local/src/index.ts` | [delete](./I38c-delete-spec.md) §5（含逐项采纳与不采纳理由） |
| **Edit（匹配 / 归一化 / 诊断 / 原子写 / 读前置）** | OpenBitFun + codex | `execution/tool-execution/src/fs/edit_file.rs`、`util/{read_line_prefix,string}.rs`、`apply-patch/src/seek_sequence.rs` | [edit](./I38c-edit-spec.md) §8（含逐项采纳与不采纳理由） |
| mkdir / move | 无对应实现 | 已核对：OpenBitFun `implementations/` 无 `mkdir` / `rename` 工具；codex `apply-patch` 只有 Add / Delete / Update；goose / deepseek-harness-orb 走 shell | [mkdir](./I38c-mkdir-spec.md) §5、[move](./I38c-move-spec.md) §5——参数与返回语义均为本项目自行定义，无可对照项 |
| 凭据目录硬禁 | openhuman | `security/policy/path_checks.rs`（`is_always_forbidden`） | 本文 §4 共享 `os::path::reject_system_dirs`，见 [I31 §5.3](./I31-phase1-tool-framework-spec.md) |
| ~~Bash / shell 守卫~~ | OpenBitFun / codex / openhuman / goose | `BashTool` + `edit_constraint_guard`、`exec_command.rs`、`core/src/exec.rs`、`tools/impl/system/shell.rs` | **暂不提供**，见 §13（评估结论留档） |
| AskUser | OpenBitFun | `implementations/ask_user_question_tool.rs`（结构化 `questions[]`：1–4 问 / 每问 2–10 选项 / `multiSelect`；`manages_own_execution_timeout()` + 外部 `user_question_timeout_secs`） | 本文 §9（P6 联调，不分册）；超时阻塞点见 §11 OQ5 |
| 多 hunk 补丁 | codex | `apply-patch/src/{parser,file_update}.rs` | 不采纳，见 §11 OQ4 |
| **多编码检测 / 解码 / 流式转码**（C9） | ripgrep + encoding_rs_io + helix | `references/7-text-encoding/ripgrep/crates/searcher/src/searcher/mod.rs`（`SearcherBuilder::build` 的 `DecodeReaderBytesBuilder` 配置链、`Encoding::new`、`binary_detection`）、`.../crates/core/flags/hiargs.rs`（`BinaryDetection::quit(b'\x00')`）、`references/7-text-encoding/encoding_rs_io/src/lib.rs`（`utf8_passthru` / `strip_bom` / 解码错误契约）、`references/7-text-encoding/helix-view/src/document.rs`（BOM → chardetng → 编码随文档生命周期） | 本文 §14（逐项采纳 / 不采纳：`DecodeReaderBytesBuilder` 配置链、`utf8_passthru`、`strip_bom`、`bom_sniffing`、`quit(b'\x00')` 采纳；逐文件 chardetng 猜测、mmap 加速、`--encoding` 的 CLI 语义外壳不采纳） |

---

## 13 暂不提供的工具

### 13.1 `shell`

**状态**：**暂不提供**（2026-11-06 定稿）。P5 不实现 `shell`，`I38c-shell-spec.md` 已删除，任务清单 C5 整节移除。

**决策理由**：

1. **不可控性当前难以把握**。命令执行的效果完全取决于 `command` 字符串的语义，而字符串语义要经过「模型生成 → 我们的转义/包裹 → 解释器解析 → 目标程序解析」四层传递。任何一层的偏差都会让实际执行偏离模型意图，且**执行前无法可靠验证**——这是破坏性后果（误删、误覆盖、误推送）的主要来源。
2. **额外包裹本身就是风险源**。为了让输出编码正确、让 shell 不加载 profile，需要在 `command` 前后拼接固定片段（UTF-8 前缀、`-NoProfile` / `-lc` 等）。这些片段会改变「首语句」约束、影响 `param(...)` / `using` / heredoc / 续行片段的语义，等于用一层新的转义去修另一层转义的问题。参考项目各自的做法（OpenBitFun 拼前缀、goose 不拼前缀、openhuman 只用于内部 CIM 探测）并不一致，说明该做法在业界也没有形成可靠共识。
3. **收益与风险不匹配**。`shell` 能覆盖的场景（构建、测试、git 操作）在当前阶段并非必需；文件工具已能覆盖读写改删。为一个非必需能力承担「静默执行非预期命令」的风险，不划算。
4. **守卫只能拦已知模式**。SafetyBelt / 结构守卫是模式匹配，无法证明「未被拦截的命令就是安全的」；把安全性建立在不完备的黑名单上，会给出虚假的安全感。

**恢复条件**（任一满足才重新评估，且必须新建分册与任务项，不得直接复用旧稿）：

- 需要真正的 OS 级隔离（容器 / 沙箱 / 受限进程令牌），使「命令做了什么」在系统层面有边界，而非依赖字符串判定；
- 或引入**结构化执行**：不接收自由格式 `command`，只接收「工具名 + 结构化参数」（如 `git_status` / `npm_test`），把语义校验前移到参数层；
- 或先具备可验证的命令预演能力（dry-run / 解析树比对），使审批文本与实际执行在语义层面等价。

**对其他阶段的影响**：

- `ask_user`（§9）与 P6 前端联调不受影响。
- 若其他分册或设计文档提到 `shell`，一律以本节为准；相关任务编号（原 C5）不再使用，后续编号顺延不受影响（C6 / C7 / C8 保持原号，不重排）。

---

## 14 多编码支持（P11 候选：C9）

> 本节是**跨工具**的编码约束，适用于 `read` / `write` / `edit` / `grep`；各工具的参数、行为与错误表仍在各自分册，本节只定「四个工具之间必须一致」的部分。
> **阶段归属**：P11 候选（[roadmap §2.1](./I38-agent-roadmap.md)），不改变 P0–P7 门禁；任务项见[执行清单](./I38c-os-tools-tasklist.md) C9。
> **依赖**：`encoding_rs` + `chardetng` 已在 `assistant-tools/Cargo.toml`（`os/process_output.rs` 使用），`encoding_rs_io` 随 `grep-searcher` 传递引入——**零新增依赖**。

### 14.1 目标与非目标

| # | 目标 |
|---|---|
| 1 | `read` 能读 GBK / GB18030 / Big5 / Shift_JIS / Windows-1252 / UTF-16（含 BOM）等非 UTF-8 文本 |
| 2 | `write` / `edit` 能**按原编码**写回，不把非 UTF-8 文件悄悄转成 UTF-8 |
| 3 | `grep` 能显式按指定编码搜索整个 scope，并对「scope 内存在非 UTF-8 文件」给出事实 |
| 4 | 编码信息以 footer 一次性呈现，成功路径只给事实 |

**非目标**：

- 不改变 UTF-8 路径的既有行为。唯一例外是 §14.3 的**二进制检测顺序修正**（存量缺陷：先查 `0x00` 再解码，会误杀带 BOM 的 UTF-16 文件），该修正随 C9.6 单独落地，不进 P5 门禁。
- 不做逐文件编码猜测：`grep` 的 probe 只判**字节合法性**，不判**是什么编码**。
- 不引入 `iconv` / 外部转码进程 / 新依赖。
- 不做大文件转码写回（§14.9 OQ7）。

### 14.2 唯一发现点原则

| 工具 | 编码来源 | 禁止 |
|---|---|---|
| `read` | **唯一检测入口**：BOM → UTF-8 校验 → 二进制判定 → chardetng | — |
| `edit` | 回执（`ReadReceipt.encoding` / `has_bom`） | 自行 `chardetng` 猜测；自行按 BOM 重判 |
| `write` | 显式 `encoding` 参数 → 已存在文件的回执继承 → UTF-8 兜底 | 对已存在文件无回执时猜编码 |
| `grep` | 显式 `encoding` 参数 → `grep-searcher` 的 BOM sniffing | 逐文件猜测 |

理由：同一文件在两个工具上得出两种编码结论，是「读对了、写坏了」的根因。`read` 的检测结果随回执流向写侧，写侧**只消费不重判**。

### 14.3 检测流程（`read`）

固定顺序，前一步命中即停：

| 步 | 判定 | 结果 |
|---|---|---|
| 1 | BOM（前 4 字节）：`EF BB BF` → UTF-8、`FF FE` → UTF-16LE、`FE FF` → UTF-16BE | 用该编码解码，`has_bom = true`，来源标 `(bom)`；BOM 优先于显式 `encoding` |
| 2 | 无 BOM 且传了 `encoding` | 用该编码解码，`has_bom = false`，footer 标该 label；显式声明同时豁免下一步的二进制判定（无 BOM 的 UTF-16 走这条路径） |
| 3 | 原始字节前 8KB 含 `0x00` | 拒绝（`Business`），提示「可能是二进制文件；若确为无 BOM 的 UTF-16，请显式传 `encoding`」 |
| 4 | `std::str::from_utf8(bytes)` 通过 | UTF-8，`has_bom = false`，**不进入猜测**（绝大多数文件的路径） |
| 5 | `EncodingDetector::feed` + `guess_assess` | **仅当** `is_confident == true` **且** 该编码解码整文件 `had_errors == false` 时接受，来源标 `(guess)`；否则拒绝（`Business`），提示显式 `encoding` |
| 6 | `IBM866 → WINDOWS_1252` 修正 | 沿用 `os/process_output.rs` 的既有规则（ASCII 正文 + 0x80–0x9F 区间仅出现引号 / 破折号字节）；该逻辑抽到共享模块，`process_output` 与文件工具共用一份 |

- **步骤 3 的安全边界**：GBK / GB18030 / Big5 / Shift_JIS / Windows-1252 的字节域均不含 `0x00`，因此该判定不会误杀这些编码；被它拦下的只有真二进制与无 BOM 的 UTF-16。
- **步骤 3 必须排在步骤 4 之前**：纯 ASCII 的无 BOM UTF-16（`h\0i\0`）本身就是合法 UTF-8，顺序反过来会让它被静默当成「带 NUL 的文本」读入——`read` 会返回看似成功的结果，模型无从察觉。代价是每个非 BOM 文件多一次前 8KB 的 NUL 扫描，`read` 本就整文件读入，相对成本可忽略。
- **显式 `encoding` 的优先级**：`BOM` > 显式 `encoding` > 自动检测。BOM 是文件自述，比调用方声明更权威；显式参数被 BOM 覆盖时，footer 标 `(bom)` 说明实际来源。这与 `grep-searcher` 的既有语义一致（`bom_override(true)`，见 §12 参考）。
- **有损解码一律拒绝**：`had_errors == true` 表示该编码无法无损表示全部字节，此时任何写回都必然改变文件内容。因此「解码无损」既是 `read` 的接受条件，也是 `edit` / `write` 的前置条件（§14.6 G2）。
- **检测范围**：检测在整文件字节上进行（`read` 已把内容读入内存），不做「只喂前 N 字节」的采样猜测——采样会让 `is_confident` 与「整文件无损」两个条件互相矛盾。大文件的内存成本属既有行为，不在 C9 处理。
- **步骤 5 的实际边界**（实测）：`chardetng` 的 `guess_assess(None, true)` 对非空输入恒返回 `is_confident == true`（1 字节到 200 字节均如此），因此「置信」这一半条件在实践中不构成过滤；真正生效的拒绝条件是「猜到的编码整文件解码有损」——单字节编码（windows-1252 / ISO-8859-x / IBM866 等）能解码任意字节，故不会触发；多字节编码（EUC-KR / GBK / Shift_JIS 等）遇到非法序列才会触发。结论：**任意非二进制、非 UTF-8 的文本都会被猜成某个编码并成功读出**，footer 的 `(guess)` 后缀是模型唯一能察觉「这是猜的」的信号。

### 14.4 呈现契约（footer）

**形态**：结果文本**最后一行**为 `[encoding: <label>]`，可带一个来源后缀：

| 情形 | footer |
|---|---|
| UTF-8、无 BOM（默认） | **不输出**（静默） |
| UTF-8、有 BOM | `[encoding: UTF-8 (bom)]` |
| BOM 判定为 UTF-16 | `[encoding: UTF-16LE (bom)]` |
| 显式 `encoding` 生效 | `[encoding: GBK]` |
| chardetng 猜测生效 | `[encoding: GBK (guess)]` |

`<label>` 用 WHATWG Encoding Standard 的规范名（`Encoding::name()`，如 `UTF-8` / `GBK` / `Shift_JIS` / `windows-1252`），不用用户传入的原始 label（别名 `gb2312` 会被规范化为 `GBK`），保证回执与 footer 是同一个字符串。

**五条规则**：

1. **只给事实**：成功路径不写「你可以传 encoding 覆盖」这类指令，参数用法写在 `input_schema` 的 `description` 里。
2. **动作只在失败路径**：检测失败 / 有损时，错误文案才提示「传 `encoding`（如 `gbk`）」。
3. **一次性**：footer 只出现在结果尾部一次，不逐行、不进入 `data` 的正文行与 `matches`。
4. **结构化已知后缀，但工具不剥离**：以 `[encoding: ` 开头、`]` 结尾、独占一行——该形态是 `edit` **诊断**的标记集（§14.8），不是剥离对象。理由：行号前缀每行必有、模型无法避开，故必须剥离；footer 只在末尾一行且仅非 UTF-8 时出现，模型只复制正文即可避开。自动剥离会把「模型误解」静默修正为成功，模型学不到「footer 不是文件内容」，与 fail-closed 取向相反。改为在 0 匹配诊断里精确指出，由模型自行移除。副作用：源文件自身末行就是 `[encoding: …]` 时不再冲突，档 1 原样命中。
5. **标记集可扩展**：`grep` 用 tag `non-utf8` / `skipped` / `lossy`（§14.7），同一套「独占一行 + 标记集」规则；新增 tag 必须回写本节。当前登记：`encoding`（`read` / `edit`）、`non-utf8` / `skipped` / `lossy`（`grep`）。

**`description` 的提示边界**：提示只补「工具接口缺口」，不补「编码常识」。

| 该说（模型不知道的接口事实） | 不该说（模型已有的常识） |
|---|---|
| 本工具**支持**非 UTF-8 文本 | 什么是 GBK / UTF-16 / BOM |
| 参数名是 `encoding`，取值是 WHATWG label | GB18030 与 GBK 的关系、`cp936` / `gb2312` 别名 |
| 何时该传：无 BOM 的非 UTF-8；有 BOM 自动识别、无需传 | 完整 label 清单（`utf-16le` 走 BOM 路径、`windows-1252` 是猜测兜底，列出无助于决策） |
| `write` 缺省继承上次 `read` 的编码；新建文件默认 UTF-8 | 转码原理、字节序、码页历史 |

- 示例只给 1–2 个最常见 label（`e.g. gbk`）：形态示例优先于枚举清单。
- **长度对齐相邻参数**：`description` 不超过同 schema 内其它参数的平均长度（一句话、无第二句）。编码只是可选参数之一，写成长段落会让它在工具描述里**喧宾夺主**，反被模型当成必填项去主动管理。
- 失败路径的补救提示压成一句（`该文件无 BOM 且非 UTF-8，请传 encoding（如 gbk）`），不展开编码知识。
- 理由：footer 的价值本就只在「模型主动转码」与「排查乱码」两处；若 `description` / 错误文案再堆编码知识，会让模型误以为编码是它必须主动管理的东西，反而诱发多余的 `encoding` 传参。

**参数描述判据（适用于所有工具参数，不限于编码）**：schema 随每次请求下发，多一句就是常驻成本，且长描述会稀释工具选择的注意力。按「不知道会发生什么」分三档：

| 档 | 条件 | 处理 |
|---|---|---|
| 必写 | 不知道就会**静默**得到错误结论（无报错、结果看似正常） | 一句写清。例：`grep` 的 `pattern` 单行匹配（多行正则零命中而不报错）、`glob` 的 `pattern` 相对 `path`（绝对 pattern 静默零命中） |
| 可写 | 不知道会**报错**，但错误文案已带出该约束 | 允许写**值域**（`context` 的 `0-10`），省一次往返；不写解释 |
| 不写 | 不知道也不影响结果正确性（宽松语义自纠正 / 纯常识） | 只写进分册 §3。例：`*.rs` 自动前置 `**/` 的深度语义（两种写法结果相同）、`pattern` 是否 `trim`（`trim` 只会放宽命中，不会漏）、`*` 不跨 `/`（glob 常识） |

- 「自纠正」的判定标准：模型按错误理解构造的输入，是否仍能得到**正确或超集**结果。是 → 不写；得到**静默子集或零命中** → 必写。
- 因此**放宽行为不该配说明书**：自动前置 `**/` 已经让 `*.rs` 与 `**/*.rs` 等价，再在 `description` 里教一遍深度语义，等于为一个模型踩不到的坑付常驻 token。
- 反向约束：`description` 的措辞只能描述**已实现**的行为。写成「matched verbatim」而代码仍在 `trim`，会让规格与实现分叉且无人察觉；实现契约一律以分册 §3 为准。

### 14.5 回执扩展

`ReadReceipt`（`assistant-tools/src/storage.rs`）增两个字段：

| 字段 | 类型 | 语义 |
|---|---|---|
| `encoding` | `String` | 实际生效编码的规范名（§14.4），如 `UTF-8` / `GBK` |
| `has_bom` | `bool` | 文件是否带 BOM；写回时据此补回 |

- `read` 成功时写入；`write` / `edit` 成功时按**自身实际写出**的编码与 BOM 状态刷新。
- `edit` / `write` 只消费不重判（§14.2）。
- 键结构不变（`session_id` / `path` / `start_line` / `end_line`）。`write` / `edit` 消费的是整文件键（`start_line = end_line = None`），因此能拿到该文件的编码；`read` 的行范围键同样带编码，但写侧不使用。
- **跨阶段契约变更**：`ReadReceipt` 属 P2 定义的 `ReadReceiptStore` 端口，字段扩展已回写 [I38a §4-C6](./I38a-tool-control-layer-spec.md)，落地时两侧必须同步。

### 14.6 `write` 转码五道闸

编码解析顺序：`encoding` 参数 → 已存在文件的回执 `encoding` → `UTF-8`。解析后过五道闸，任一不过即 `Business` 且**不写盘**：

| 闸 | 规则 | 目的 |
|---|---|---|
| G1 | 目标编码必须**确定**：显式参数或回执继承，二者皆无则 UTF-8 | 禁止猜测 |
| G2 | 内容可完整编码：`encoding.encode(content)` 的 `had_errors == false` | 禁止 `?` 替换式写回（不可逆） |
| G3 | 转码写入 ≤ **1MB**：`content` + 追加时保留的现有内容（覆盖场景现有内容被整篇替换，不计入） | 转码需整文件驻留内存；大文件登记 OQ7 |
| G4 | 目标是**已存在**文件且要求非 UTF-8 写入时，必须有本会话回执 | 禁止盲转码覆盖 |
| G5 | BOM 按目标编码规范：UTF-16 恒写 BOM；UTF-8 仅在回执 `has_bom` 时写 BOM | 避免 BOM 无声丢失 |

- `append` 与转码叠加时按「解码现有内容 → 拼接 → 整体编码」处理，**不**把两段字节直接拼接（编码边界处的字节拼接会产生非法序列）；解码现有内容用**现有文件的编码**（回执 `encoding`），不是目标编码——显式转码时两者可以不同。
- 目标编码与回执继承的编码不同时，按显式参数整篇写出新编码（覆盖即整篇替换，追加则先按现有编码解码再整体按目标编码编码）。
- 幂等判定改为**字节级**：`payload_bytes == existing_bytes` 时仍不写盘，原有语义不变。
- 新建文件无回执，G4 不适用；`encoding` 缺省时写 UTF-8（与现状一致）。
- 「转码写回」的适用边界：`write` 全量替换的场景（模型把整篇内容重新给出）。`edit` 的局部替换不需要模型给出整篇，因此不受 G3 约束之外的额外限制，但同样受 G2 / G5 约束（§14.8）。

### 14.7 `grep` 两层探测

`grep` **不猜测编码**，只做两件事：按显式 `encoding`（或 BOM）搜索；对 scope 内「存在非 UTF-8 文件」给出事实。

**搜索层配置**（复用 `grep-searcher`，不自行实现转码）：

```rust
SearcherBuilder::new()
    .line_number(true)
    .before_context(context).after_context(context)
    .binary_detection(BinaryDetection::quit(b'\x00'))   // 新增：默认是 None，等于不做二进制检测
    .encoding(explicit)                                  // Option<grep_searcher::Encoding>
    .build()
```

| 配置 | 现状 | 结论 |
|---|---|---|
| `utf8_passthru(true)`（`build()` 内固定） | 已是 | 无 BOM 时**零转码**，UTF-8 路径无额外成本 |
| `bom_sniffing` 默认 `true` | 已是 | UTF-16（带 BOM）**今天就能搜**，且首行不含 `U+FEFF`；因此 probe 必须跳过 BOM 文件，否则会误报「非 UTF-8」 |
| `binary_detection` 默认 `None` | **未开**（存量缺陷） | 改为 `quit(b'\x00')`：二进制文件不再产出乱码行，命中 `Sink::binary_data` 计入 `binarySkipped` |
| `encoding` | 未接 | 显式参数映射为 `Encoding::new(label)`（`Result`，非法 label → `Business`）；**scope 级开关**，设置后 scope 内每个文件都走转码 |
| `mmap` | 默认 `MmapChoice::never()` | 保持关闭。转码路径与 mmap 天然不兼容（`DecodeReaderBytes` 需要读流），开启 mmap 只会让「什么时候转码」变成隐式分支 |

**热路径（命中行）**：`trim_line_end` 先 `std::str::from_utf8`，失败才 `from_utf8_lossy` 并置 `lossy_lines = true`。合法路径因此比现状**少一次分配**（`from_utf8_lossy` 内部本来就会先跑 `from_utf8`）。命中行与上下文行走同一函数，**两者都计入** `lossyLines`：context 行同样以替换字符渲染，只数命中行会让「结果里有乱码」没有对应计数。

**冷路径（零命中时的 probe）**：仅当 `match_count == 0`、未超时**且未显式指定 `encoding`** 时执行，热路径零成本：

1. 逐文件读**前 8KB**（复用单个缓冲，不每文件新建 `Vec`），用 `read` 同一套前缀分类（§14.3 步骤 1–3）。
2. 跳过：含 `0x00`（二进制）、有 BOM（已可搜）、合法 UTF-8。
3. 计数「非 UTF-8 且非二进制」文件；达 `PROBE_FILE_LIMIT`（200）或 deadline 即停，结果标 `nonUtf8Probe.truncated`。
4. 计数 > 0 时输出 footer `[non-utf8: N file(s)]`；为 0 时静默。

**显式 `encoding` 时跳过 probe**：`encoding` 已是 scope 级声明，此时再报「scope 内有非 UTF-8 文件」等于暗示换编码重试，属误导；`nonUtf8Probe` 恒为 `null`。

**结果 `data` 新增**：`binarySkipped`（命中 `binary_data` 的文件数，该文件不产出任何行）、`largeSkipped`（超 `max_filesize` 的文件数）、`unreadableFiles`（读取失败的文件数）、`lossyLines`（含替换字符的渲染行数，命中行与上下文行都计）、`nonUtf8Probe`（`{ "files": N, "truncated": bool }` 或 `null`）。

**footer**：`[non-utf8: N file(s)]`（probe 命中时）、`[skipped: N file(s)]`（`binarySkipped + largeSkipped + unreadableFiles > 0`，有命中 / 零命中都输出）、`[lossy: N line(s)]`（`lossyLines > 0`）。三者都只给事实，动作提示留在错误文案与 `description`。

**输出预算与可中断性**（`grep` 侧细则见 [grep §3.2 / §3.3](./I38c-grep-spec.md)）：`MAX_MATCHES`（100 块）+ `MAX_OUTPUT_LINES`（300 行，命中 + 上下文合计）+ `MAX_CONTEXT`（10，超出即 `Business`）；deadline 检查点覆盖「每文件开始前」与 `matched` / `context` 两个回调；`max_filesize = 16 MiB` 作为单文件闸门。这些约束不属于编码范畴，但只在 `grep` 上生效，故在此登记索引，避免读者误以为全工具统一。

**已知限制**（不进 `description`，只在错误表与文档里说明）：非 UTF-8 文件未传 `encoding` 时按 UTF-8 逐字节匹配，命中行以替换字符渲染——这是「不猜测」的直接代价。`description` 只保留一句「省略即按原样搜索」。

### 14.8 `edit` 的编码约束

`edit` 不新增 `encoding` 参数：它的输入是「模型看到的文本」，而那段文本来自 `read`，编码已由回执携带。

| 环节 | 约束 |
|---|---|
| footer 识别 | **不做剥离**（§14.4 规则 4）：`old_string` / `new_string` 原样进入档位；仅在 `old_string` 0 匹配诊断里检测末尾独占一行的 `[encoding: …]` 标记集并提示「这不是文件内容，请移除后重试」。只加提示、不改判定；**不检查 `new_string`**（硬拒绝会误伤正文本就含该形态的合法内容）。不预处理的理由见 §14.4 规则 4；副产品是源文件自身末行就是该形态时仍可原样命中 |
| 写回编码 | 用回执 `encoding` + `has_bom`：`encode(新全文)` 的 `had_errors == true` → `Business`（G2），**不**写回 |
| BOM | `has_bom == true` → 补 BOM（G5）；`false` → 不补 |
| 行尾 | 沿用现有 `uses_crlf` 判定，编码支持不引入新规则 |
| 无回执 | 保持现有行为：拒绝并提示先 `read`（[edit §6](./I38c-edit-spec.md)） |
| 写后刷新 | 用实际写出的编码 / BOM 状态刷新回执，供后续 `edit` / `write` 复用 |

### 14.9 开放问题

- **OQ7 大文件转码写回**：>1MB 的非 UTF-8 文件 `read` 可读，但 `edit` / `write` 转码写回被 G3 拒绝。当前结论：**不做**，错误文案提示先外部转码再重读。详见 §11 OQ7。
- **OQ8 chardetng 置信度阈值**：不引入可调阈值，接受条件是二值判定。详见 §11 OQ8。
- **OQ9（本节新增）** **回执里的 `encoding` 是否应参与修订号 / 失效判定？** 当前结论：**否**。`revision` 仍是「长度 + mtime」，编码变化必然伴随字节变化 → 长度或 mtime 变化，无需额外维度；若将来出现「等长等时戳的编码转换」，与「删除后重建」同类，由 `invalidate_path` 兜底。
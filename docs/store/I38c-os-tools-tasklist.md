# I38c — P5 OS 工具能力任务清单

> 规格：[I38c-os-tools-spec.md](./I38c-os-tools-spec.md)（索引：§3 实现顺序、§4 共同结构、§10 验收标准）+ 逐工具分册（入口见索引 §5 / §6 / §7 的表格）
> 阶段：[I38-agent-roadmap.md](./I38-agent-roadmap.md) P5
> 顺序：C0 → C1 → C2 → C3 → C4 → C6 → C6b → C7 → C8（C5 已移除，`shell` 暂不提供）
> 勾选条件：代码已改 **且** 对应验收已跑。
> **任务项粒度**：一项 = 一个可独立完成、可独立验证的工作单元。语义与边界条件写规格，任务项只写「落点 + 判定条件」，不复制规格原文；同一工具的测试合并为一项，用例清单见规格 §10。任务项内**禁止写行号**，只写符号名（行号会随改动立刻腐化）。
> 范围：`src-tauri/crates/assistant-tools/src/os/*`、`os/mod.rs` 分组注册、`src-tauri/src/setup.rs` 的注册调用、workspace / crate 依赖；不新增前端组件、不改控制层、不改 DB、不做 MCP。**例外（C6b）**：为接入白名单解析，新增 `ConversationToolConfigDto` / `ToolTierDto` / `ApprovalLevelDto`，`SendChatMessageArgsDto` 增 `tool_config` 字段，`TurnSpec.tools` 收窄为 `Option<Vec<ToolSpec>>`，并删除 P4 的 `ToolCatalog` 接入层（详见 C6b）。
> 测试位置：公开行为测试放 `src-tauri/crates/assistant-tools/tests/<模块>.rs` 集成测试；仅当必须访问私有函数时才保留源文件内 `#[cfg(test)]` 模块（与 `os/path.rs`、`os/fs_link.rs` 现状一致）。
> **状态唯一位置**：本清单是 P5 任务级进度的唯一来源（含逐个工具的实现状态）；阶段级结论只在 [roadmap](./I38-agent-roadmap.md) §2 写一次（roadmap §7）。本清单**不管理提交**（不写提交拆分 / 提交信息 / 回滚批次，见 roadmap §7 第 2 条）。

---

## C0 口径与前置确认

- [x] C0.1 确认 P1 已通过：`os/path.rs` 的 `resolve` / `resolve_leaf_preserving` / `canonicalize_lenient` / `reject_system_dirs` / `ensure_within_root` 可用，`fs_link` 已迁移
- [x] C0.2 确认 P2 已通过：`ToolPipeline`（`find → validate → gateway → execute → persist`）、`ToolGateway`、`partition_tool_batches`、`select_persist_targets` 可用；P5 不改控制层
- [x] C0.3 确认 P5 不实现前端组件、审批 UI、`ask_user`、MCP、RoundExecutor
- [x] C0.4 确认所有路径工具只经 `os::path`：读 / 写 / 删 / 移动一律用 `resolve()`；`fs_link` 的叶子保留语义仍用 `resolve_leaf_preserving()`；不新增私有 canonicalize 或失败回退
- [x] C0.5 确认 `os_tool_exec` 恒向 `params` 注入 `action`（已核对 `src-tauri/src/cmd/os_tool_exec.rs`）：新工具的 `input_schema` 不声明 `action`、不设 `additionalProperties: false`，`execute()` 忽略未知字段（ToolBox 路径不经过 `validate_input`）
- [x] C0.6 确认 `workspace_root` 来源不在 P5（I18 spec B1 标注为 P5/P6）：P5 只保证绝对路径可用；ToolBox 侧相对路径仍不可用（`ToolUseContext::for_toolbox` 传 `workspace_root = None`），待 P6 补工作区来源
- [x] C0.7 **已定稿**：P2 只定义 `ReadReceiptStore` trait（无实现者、无调用点），P5 补 `InMemoryReadReceiptStore`（`dashmap` 已是 crate 依赖）并由 `ReadTool` 构造注入；`ToolUseContext.session_id` 为 `None`（ToolBox）时归入同一桶
- [x] C0.8 **已定稿**：`ReadReceiptKey` 补 `session_id: Option<String>`（键结构与理由见 [I38a §4-C6](./I38a-tool-control-layer-spec.md)）
- [x] C0.9 **已定稿**：依赖选型 `glob` = `ignore` + `globset`，`grep` = `grep-searcher` + `grep-regex`，遵守 `.gitignore`（见 [I38c §11 OQ1](./I38c-os-tools-spec.md)）
- [x] C0.10 **已定稿**：超时策略见 [I38c §11 OQ2](./I38c-os-tools-spec.md)
- [x] C0.11 **已定稿**：注册门禁见 [I38c §7.1](./I38c-os-tools-spec.md)
- [x] C0.12 **已定稿**：`audience` 必填且 fail-closed（见 [I38c §4](./I38c-os-tools-spec.md)）；原「`shell` 按守卫完成度降级」随 C5 移除而失效
- [x] C0.13 **已定稿**：`ReadReceiptStore` 补 `invalidate_path`，按组件级前缀失效（见 [I38a §4-P2-C6](./I38a-tool-control-layer-spec.md)、[delete §3.6](./I38c-delete-spec.md)）
- [x] C0.14 **已定稿**：`reject_system_dirs` 扩展凭据目录，按路径组件匹配（见 [I31 §5.3](./I31-phase1-tool-framework-spec.md)）

---

## C1 共享基础设施与依赖

实现：`src-tauri/Cargo.toml`、`src-tauri/crates/assistant-tools/Cargo.toml`、`src-tauri/crates/assistant-tools/src/os/{output,mod,path}.rs`、`src-tauri/crates/assistant-tools/src/storage.rs`
测试：`src-tauri/crates/assistant-tools/tests/{output,storage}.rs`、`os/path.rs` 单测

- [x] C1.1 workspace `Cargo.toml` 增加 `ignore` / `globset` / `grep-searcher` / `grep-regex`（按 C0.9 定稿）
- [x] C1.4 `assistant-tools/Cargo.toml` 以 `workspace = true` 引入 C1.1 的依赖
- [x] C1.5 新建 `os/output.rs`：`OutputLimits { max_bytes, max_lines, preview_bytes }`、`TruncatedOutput`（`text` / `truncated` / `total_bytes` / `total_lines` / `returned_lines`）、`truncate_output`
- [x] C1.6 `truncate_output` 在截断时显式写「已截断 + 实际行数 / 字节数」，不静默丢弃
- [x] C1.7 `os/mod.rs` 增加 `pub mod output;`，并预留各工具模块声明
- [x] C1.8 `storage.rs` 新增 `InMemoryReadReceiptStore`（`dashmap`），实现 `ReadReceiptStore::lookup` / `record`
- [x] C1.9 `ReadReceiptKey` 补 `session_id: Option<String>`（C0.8），`InMemoryReadReceiptStore` 以其作键的一部分；同一路径 + 行范围 + 修订号在**不同会话**间互不抑制
- [x] C1.10 明确「修订号」计算方式：文件长度 + 修改时间（纳秒），不整文件哈希；写入 `agent_read.rs` 注释
- [x] C1.11 单测：未超限原样返回；超行 / 超字节各一条；截断提示包含实际值；回执键跨会话隔离
- [x] C1.12 `ReadReceiptStore::invalidate_path`（C0.13）：`InMemoryReadReceiptStore` 按组件级前缀移除条目；单测覆盖「子项失效」与「`/a/bc` 不被 `/a/b` 误伤」
- [x] C1.13 `os/path.rs::reject_system_dirs` 增加凭据目录（C0.14），按路径组件匹配、大小写不敏感；单测覆盖 `.ssh`、`Microsoft\Credentials` 与「项目目录名恰好含 `keychains` 但不作为组件」的用例

---

## C2 只读组：`read` / `ls` / `glob` / `grep`

实现：`src-tauri/crates/assistant-tools/src/os/{agent_read,agent_list,agent_glob,agent_grep}.rs`
测试：`src-tauri/crates/assistant-tools/tests/{agent_read,agent_list,agent_glob,agent_grep}.rs`

**组内共同项**

- [x] C2.1 四个工具均实现 `id` / `description` / `input_schema` / `tier = Os` / `audience = BOTH` / `behavior_hints.read_only = true` / `destructive = false` / `open_world = false`
- [x] C2.2 `permission_intents` 返回空 `Vec`（沿用默认实现，A3）
- [x] C2.3 `is_concurrency_safe` 返回 `true`（只读默认；不因 input 收紧）
- [x] C2.4 `validate_input` 覆盖必填缺失与类型错误，并对所有路径参数调用 `os::path::resolve`（A1 / A2）
- [x] C2.5 `execute` 内同步文件 IO / 目录遍历一律包 `tokio::task::spawn_blocking`，不阻塞 tokio worker
- [x] C2.6 四个工具均能容忍 `os_tool_exec` 注入的多余 `action` 字段（C0.5）

**`read`**

- [x] C2.7 `input_schema`：`path` / `offset` / `limit` 及边界语义（[read §2 / §3](./I38c-read-spec.md)）
- [x] C2.8 默认读全部，上限 1MB / 2000 行；`offset` + `limit` 支持行范围
- [x] C2.9 二进制检测：读取前 8KB，含 NUL 字节即拒绝，提示改用十六进制 / 文本工具
- [x] C2.10 超限与单行截断的提示文案、输出格式契约（与 `edit` 共用常量）见 [read §3 / §3.2](./I38c-read-spec.md)
- [x] C2.11 读取回执：同一 session + 同一路径 + 同一行范围且修订号未变 → 返回「已在上文返回」，不回传内容
- [x] C2.12 修订号变化 → 重新读取并刷新回执
- [x] C2.13 测试：schema、缺 `path`、`offset` / `limit` 边界、二进制拒绝、截断提示、长行 `[truncated]`、行号前缀格式、回执复用与刷新、空文件

**`ls`**

- [x] C2.14 `input_schema`：`path`（必填）+ `depth`（默认 1、最大 10）+ `all`（包含隐藏项）
- [x] C2.15 输出按名称排序，每层最大 1000 条目，超出时显式提示
- [x] C2.16 目录遍历不跟随符号链接（避免越界读取）；对每个条目仍保持 workspace 边界结论一致
- [x] C2.17 测试：默认深度、深度夹取、隐藏项开关、排序、1000 条上限、越界路径拒绝、符号链接目录不被跟随

**`glob`**

- [x] C2.18 `input_schema`：`pattern`（必填）+ `path`（基准目录）+ `exclude`（数组）
- [x] C2.19 `path` 缺省时取 `workspace_root`；无 `workspace_root` 则返回明确错误
- [x] C2.20 使用 `ignore` 的 walker + `globset` 处理 pattern / exclude，遵守 `.gitignore`（C0.9）
- [x] C2.21 结果只含 workspace 内路径，最大 1000 条，按路径字典序排序
- [x] C2.22 测试：pattern 命中、exclude 生效、1000 条上限、越界拒绝、缺省基准目录、排序稳定

**`grep`**

- [x] C2.23 `input_schema`：`pattern`（必填）+ `path` + `include`（文件 glob）+ `context`（默认 0）
- [x] C2.24 使用 `ignore` 遍历 + `grep-searcher` / `grep-regex` 搜索，`include` 用 `globset` 过滤
- [x] C2.25 输出含文件路径、行号、匹配行与 context 行；最大 100 条匹配；内部 deadline 提前停止的语义见 [grep §3](./I38c-grep-spec.md)
- [x] C2.26 非法正则返回 `ToolResult::err`（`Business`），不 panic、不中断 turn
- [x] C2.27 测试：匹配与行号、context 行数、include 过滤、100 条上限、非法正则、越界拒绝、内部 deadline 提前停止

---

## C3 修改组：`write` / `edit` / `mkdir` / `move`

实现：`src-tauri/crates/assistant-tools/src/os/{agent_write,agent_edit,agent_mkdir,agent_move}.rs`
测试：`src-tauri/crates/assistant-tools/tests/{agent_write,agent_edit,agent_mkdir,agent_move}.rs`

- [x] C3.1 四个工具 `behavior_hints.read_only = false`、`destructive = false`、`open_world = false`、`audience = BOTH`
- [x] C3.2 `is_concurrency_safe` 保持默认（写类串行），不显式放宽
- [x] C3.3 `permission_intents` 产出 `edit` + 受影响路径（A3）；`move` 的 `source` / `target` 各一条
- [x] C3.4 `validate_input` 校验必填 / 类型并调用 `os::path::resolve`；`execute` 仍自行校验，不依赖 Pipeline

**`write`**

- [x] C3.5 schema：`path` / `content`（必填）+ `append`（默认 false）；覆盖已有文件自动创建父目录
- [x] C3.6 单次写入上限 10MB，超出返回 `Business`；`append = true` 时追加而非覆盖；幂等成功、不可读拒绝覆盖与原子写（临时文件 → `sync_all` → rename）见 [write §3](./I38c-write-spec.md)
- [x] C3.7 测试：新建、覆盖、追加、父目录自动创建、10MB 拒绝、越界拒绝、相同内容不写盘、不可读文件拒绝覆盖

**`edit`**

- [x] C3.8 schema：`path` / `old_string` / `new_string`（必填）+ `replace_all`（默认 false）
- [x] C3.9 前置拒绝 + 6 档渐进匹配（精确 → 行号前缀剥离 → 尾随换行变体 → 行尾空白不敏感 → 首尾空白不敏感 → 归一化），归一化命中必须回写**真实原文片段**；见 [I38c-edit §3](./I38c-edit-spec.md)「匹配流水线」（含逐档参考来源）与「不变量」
- [x] C3.10 结果分支：0 / 1 / 多匹配（`replace_all` 两分支）与 diff 形状（`start_line` / `old_end_line` / `new_end_line` / `match_count`），见 [I38c-edit §4](./I38c-edit-spec.md)
- [x] C3.11 写回约束：CRLF 文件整体按 CRLF 写回、BOM 保留、`tempfile` + `sync_all` + `persist` 原子替换，见 [I38c-edit §5](./I38c-edit-spec.md)
- [x] C3.12 读前置 guard：复用 `ReadReceiptStore`（无回执拒绝 / 修订号过期拒绝 / `write` 后紧接 `edit` 不误拦），见 [I38c-edit §6](./I38c-edit-spec.md)
- [x] C3.13 测试：覆盖 §10 A4 / A4b（三分支、6 档各一例、歧义不取首个、行尾形态与 BOM、原子写、guard 三例）

**`mkdir`**

- [x] C3.14 schema：`path`（必填）+ `parents`（默认 true）
- [x] C3.15 `parents = false` 且父目录不存在 → `Business`；目录已存在时幂等返回成功并注明
- [x] C3.16 测试：单层 / 多层创建、`parents = false` 失败、已存在幂等、`edit` intent

**`move`**

- [x] C3.17 schema：`source` / `target`（必填）+ `overwrite`（默认 false）
- [x] C3.18 目标已存在且 `overwrite = false` → `Business`；`overwrite = true` 时先删目标再重命名；非空目标目录的处理见 [move §3](./I38c-move-spec.md)
- [x] C3.19 仅用 `std::fs::rename`；跨设备失败返回 `Business`，**不**静默降级为复制 + 删除
- [x] C3.20 成功后 `ReadReceiptStore::invalidate_path(source)`（C1.12），连带子树失效；`target` 不失效
- [x] C3.21 测试：移动文件 / 目录、`overwrite` 两分支、非空目标目录被拒、两条 `edit` intent、目标父目录不存在、移动后源路径回执不命中

---

## C4 破坏性组：`delete`

实现：`src-tauri/crates/assistant-tools/src/os/agent_delete.rs`
测试：`src-tauri/crates/assistant-tools/tests/agent_delete.rs`

- [x] C4.1 schema：`path`（必填）+ `recursive`（默认 false）；不声明 `force`
- [x] C4.2 `behavior_hints`：`read_only = false`、`destructive = true`、`open_world = false`；`is_concurrency_safe = false`
- [x] C4.3 `permission_intents` 产出 `delete` + **解析后归一化的绝对路径**（供 P2 `ApprovalKey::Path` 归一，A5）。解析用 `resolve_leaf_preserving`（`resolve` 会跟随叶子链接，与 C4.5 冲突，见 [delete §3.1](./I38c-delete-spec.md)）
- [x] C4.4 `validate_input` 拒绝不可删目标：`workspace_root` 本身、文件系统根 / 盘符根、家目录本身（`HOME` / `USERPROFILE`，只拒本身不拒子项）；对叶子保留路径与跟随链接后的真实路径各判一次；拒绝先于任何删除动作
- [x] C4.5 目标分类用 `symlink_metadata`（不跟随）：链接按文件删除、`recursive` 不影响链接、结果 `isDirectory = false`；junction 由 `FILE_ATTRIBUTE_REPARSE_POINT` 识别（`is_symlink()` 对 junction 为 false，见 [delete §3.2](./I38c-delete-spec.md)）
- [x] C4.6 递归预扫描：非空判定 + 条目 / 字节量级 + `DELETE_SCAN_LIMIT` 截断不拒删；`scan_tree` 由 `preview_for_approval` 与 `execute` 共用
- [x] C4.7 执行分支：文件 / 链接 / 目录非递归 / 目录递归 + Windows 只读前置拒绝（`std::fs::remove_file` 会自动清属性，故改为删前判定，见 [delete §3.4](./I38c-delete-spec.md)）
- [x] C4.8 失败后重查目标存在性，区分「未删除 / 已删除但随后失败」；不重试、不分批
- [x] C4.9 结果 `data` 六字段 + `result_for_assistant` 三形态
- [x] C4.10 删除成功后 `ReadReceiptStore::invalidate_path`（C1.12），按组件级前缀失效路径及其子树
- [x] C4.11 测试：常规删除（文件 / 递归目录 / 非空目录非递归失败 / 空目录非递归成功）；不可删目标（`workspace_root` 自删含尾随分隔符与 `.`、文件系统根、家目录本身）；链接语义（文件链接、目录链接 `recursive` 两分支、dangling）；预扫描（量级文案、缺失目标无预演）；失败措辞（只读文件、`describe_failure` 两分支单测）；`delete` intent 与 `..` / 相对路径归一后同一审批键；删除 → 子树回执失效

---

## C5（已移除）`shell`

**不做**。`shell` 改为**暂不提供**（决策与恢复条件见 [I38c §13](./I38c-os-tools-spec.md)），原 C5.1–C5.15 全部取消，编号 C5 不再使用（C6 / C7 / C8 保持原号不重排）。

---

## C6 分组注册与桥接兼容

实现：`src-tauri/crates/assistant-tools/src/os/mod.rs`、`src-tauri/src/setup.rs`
测试：`src-tauri/crates/assistant-tools/tests/registration.rs`

- [x] C6.1 `os/mod.rs` 提供 `register_readonly_tools` / `register_write_tools` / `register_dangerous_tools` 三组函数
- [x] C6.2 `register_os_tools` 只保留 `fs_link`（`TOOLBOX_ONLY`），不与新工具混在同一函数
- [x] C6.3 `setup.rs` 调用 `os::register_os_tools` + `os::register_readonly_tools`；不调用修改组 / 危险组（A7）。只读组需要回执端口，`setup.rs` 装配 `InMemoryReadReceiptStore` 并 `handle.manage`
- [x] C6.4 单测：只读组注册后包含 `read` / `ls` / `glob` / `grep`，且 `fs_link` 仍在
- [x] C6.5 单测：未调用修改组 / 危险组时，`find_for_toolbox` 查不到 `write` / `edit` / `mkdir` / `move` / `delete`
- [x] C6.6 单测：`tool_definitions_for_tier(ToolTier::Os)` 仅返回 4 个只读工具且按名排序（`fs_link` 被 `audience` 过滤）
- [x] C6.7 单测：显式调用修改组 / 危险组后，对应工具可被 `find_for_toolbox` 查到（仅测试内使用，不改变生产注册）
- [x] C6.8 确认不改 `os_tool_exec` 命令与 `OsToolExecRequestDto`；新工具在注入 `action` 后仍可执行（C0.5）

---

## C6b 白名单工具解析（接入层替换）

实现：`src-tauri/src/adapters/tool_resolver.rs`、`src-tauri/src/cmd/agent/mod.rs`、`src-tauri/src/protocol/dto/{agent,chat}.rs`、`src-tauri/crates/assistant-agent/src/kernel/{ports,mod,runner,request}.rs`、`src-tauri/crates/assistant-agent/src/agent/{round_executor,loop_executor}.rs`
测试：`src-tauri/tests/tool_resolver.rs`、`assistant-agent/tests/{loop_executor,kernel_request_tools}.rs`（同步改）

**背景**：P4 的 `RegistryToolCatalog` 用 `tool_definitions_for_tier(MAX_TIER)` 做自动全量注入，无白名单/黑名单。P5 替换为 deny-by-default + 显式白名单/黑名单的 `resolve_available_tools`。

- [x] C6b.1 新建 `src-tauri/src/adapters/tool_resolver.rs`：`pub async fn resolve_available_tools(registry: &ToolRegistry, enabled_tools: &[String], disabled_tools: &[String], max_tier: ToolTier) -> Vec<ToolSpec>`（**必须 async**：`tool_definitions_for_tier` 与 `Tool::description` 均 async）
- [x] C6b.2 过滤顺序固定为 `tier → audience → deny-by-default → whitelist → blacklist`（规格 §3.1）；前两步与排序复用 `ToolRegistry::tool_definitions_for_tier(max_tier)`（返回 `Vec<ToolDefinition>`），并把原 `tool_catalog.rs` 的 `into_tool_spec` 搬入本文件完成 `ToolDefinition → ToolSpec` 转换
- [x] C6b.3 `disabled_tools` 优先于 `enabled_tools`：同一工具同时出现 → 排除
- [x] C6b.4 工具匹配按 `Tool::id()` 注册名，不按文件名；未知工具名静默忽略（不报错）
- [x] C6b.5 返回 `Vec<ToolSpec>` 顺序继承 `tool_definitions_for_tier` 的注册名字典序（确定性输出，便于测试）
- [x] C6b.6 在 `src-tauri/src/protocol/dto/agent.rs` 新建 `ConversationToolConfigDto`：`max_tool_tier: ToolTierDto` / `approval_level: ApprovalLevelDto` / `enabled_tools: Vec<String>` / `disabled_tools: Vec<String>`（`camelCase` + `deny_unknown_fields`）
- [x] C6b.7 同文件新建 `ToolTierDto`（`Sandbox` / `Os` / `External`）与 `ApprovalLevelDto`（`None` / `DangerousOnly` / `AllOsTools`，`rename_all = "snake_case"` 以对齐 I38d §4.2 的 `dangerous_only` / `all_os_tools`）；`ToolTier` 已存在于 `assistant-tools`，DTO 侧提供 `From` 双向转换，不把内部类型暴露到 wire
- [x] C6b.8 `SendChatMessageArgsDto` 新增 `tool_config: Option<ConversationToolConfigDto>`（`#[ts(optional = nullable)]` + `#[serde(default)]`）
- [x] C6b.9 执行 `npm run contract:gen` 生成前端类型；`src-tauri/tests/protocol_bindings.rs` 的 `declarations` 加三个新类型（该测试扫描 `protocol/dto/*.rs`，漏声明即失败）
- [x] C6b.10 `npm run contract:check` 幂等（生成后无 diff；P5 只新增 DTO，不改既有契约）

**接入 `send_agent_message`**：

- [x] C6b.11 `send_agent_message` 从 `args.tool_config` 取值，`None` 时按 `ConversationToolConfigDto::default()`（`max_tier = Os`、`approval_level = DangerousOnly`、空白名单 = deny-by-default），`resolve_available_tools(...).await`
- [x] C6b.12 `spec.tools = Some(tools)`（不判空）：空集归一化由 `kernel/request.rs` 的 `resolve_tools` 负责（`Some(vec![])` 与 `None` 等价，已有单测），接入层不重复判空
- [x] C6b.13 `TurnSpec.tools` 类型由 `Option<Arc<dyn ToolCatalog>>` 改为 `Option<Vec<ToolSpec>>`，变更面（全部需同步，缺一处即编译失败）：
  - `kernel/mod.rs`：`TurnSpec` 字段声明 + `chat()` / `agent()` 的 `tools: None`
  - `kernel/runner.rs`：`TurnRunCtx.tools` 赋值；`request::build(spec.tools.as_ref())` 需改为 `.as_ref().map(Vec::as_slice)`（`Option<&Vec<_>>` 不会自动转 `Option<&[_]>`）
  - `kernel/request.rs`：`build()` 签名由 `Option<&Arc<dyn ToolCatalog>>` 改为 `Option<&[ToolSpec]>`，`resolve_tools` 保留（去掉 `agent_tools().await`）
  - `agent/round_executor.rs`：`TurnRunCtx.tools` 与 `ToolExecCtx.tools` 字段类型
  - `agent/loop_executor.rs`：`request::build` 传参用 `ctx.tools.as_deref()`；`ToolExecCtx` 构造处的 `tools: ctx.tools` 随字段删除一并去掉
  - `tests/loop_executor.rs`：`catalog` 构造与 `with_tools` 语义
  - `tests/kernel_request_tools.rs`：`StaticCatalog` 的三处调用
- [x] C6b.14 `TurnSpec::agent()` 与 `TurnSpec::chat()` 的 `tools` 默认保持 `None`
- [x] C6b.15 `start_turn` 对 `SendChatMessageArgsDto` 的穷尽解构补 `tool_config: _`（或 `..`）；`chat` 链路复用 `start_turn` 不受影响

**清理 P4 接入代码**：

- [x] C6b.16 删除 `src-tauri/src/adapters/tool_catalog.rs`（整文件：`RegistryToolCatalog` + `MAX_TIER`）
- [x] C6b.17 `src-tauri/src/adapters/mod.rs` 删除 `mod tool_catalog;` 与 `pub use` 导出
- [x] C6b.18 `src-tauri/src/cmd/agent/mod.rs` 删除 `use crate::adapters::RegistryToolCatalog;`
- [x] C6b.19 删除 `assistant-agent/src/kernel/ports.rs` 的 `ToolCatalog` async trait（唯一实现者即 `RegistryToolCatalog`）
- [x] C6b.20 删除 `assistant-agent/src/kernel/ports/test_doubles.rs` 的 `StaticToolCatalog`
- [x] C6b.21 **P4 遗留缺陷清理**：删除 `ToolExecCtx.tools` 字段——生产适配器 `PipelineToolExecution` 从不读取它（只用 `observer` / `turn_id` / `iteration` / `conversation_id`），P4 遗留的未使用字段；同步删 `round_executor.rs` 字段声明与 `loop_executor.rs` 构造处
- [x] C6b.22 全仓 grep 确认 `RegistryToolCatalog` / `ToolCatalog` / `StaticToolCatalog` 无残留引用

**测试**：

- [x] C6b.23 测试：覆盖 [§10 A10](./I38c-os-tools-spec.md) 全部用例（空白名单 / 白名单 / 黑名单覆盖 / 未知工具名静默忽略 / `max_tier` 硬上限 / `TOOLBOX_ONLY` 受众过滤 / 字典序确定性 / `ConversationToolConfigDto` 往返）

**补充修复**（随 C7 全量验收暴露的存量问题）：

- [x] C6b.24 `setup.rs` 的 `#[cfg(all(target_os = "windows", feature = "vault"))]` 违反 `assembly_contract` A2（特性门禁只允许出现在 `cmd/mod.rs` 或与特性同名的 `*_plugin.rs`）。把 Windows keyring 预热移入 `vault_plugin::init()` 的 `setup` 闭包（该 `init()` 已由 `#[cfg(feature = "vault")]` 整体门控），内层只保留 `#[cfg(target_os = "windows")]`；`setup.rs` 删除该段，`feature = "vault"` 门禁行数维持 `VAULT_CFG_LIMIT = 6`

---

## C7 验收命令

`cwd = src-tauri`

- [x] C7.1 `cargo test -p assistant-tools` 全绿（A9）
- [x] C7.2 `cargo clippy -p assistant-tools --all-targets -- -D warnings` 无告警（A9）
- [x] C7.3 先执行格式化修复，再 `cargo fmt --all --check` 通过（A9）
- [x] C7.4 grep 静态自查：`assistant-tools/src/os/` 无第二条路径规范化实现、无 `unwrap_or_else(|_| path.to_path_buf())`（A1）；每个新工具都有显式 `audience()`（C0.12）
- [x] C7.5 确认 `setup.rs` 生产注册表只含 `fs_link` + 只读四工具（A7）
- [x] C7.6 `npm run contract:check` 幂等：执行生成后 `git status` 对契约文件无新增 diff（C6b 已生成并提交 `src/types/protocol.contract.ts`）
  - 复核口径：生成器对同一工作区重复执行输出稳定；`contract:check` 对 **HEAD** 报 diff 属预期（P5 新增的三个 DTO 尚未提交），提交后即回归无 diff
- [x] C7.7 `git status` 路径核对：无前端组件、无 DB、无 `assistant-tools` 控制层文件改动；允许的变更仅限 `src/types/protocol.contract.ts`、`src-tauri/tests/protocol_bindings.rs` 与 C6b 列出的 Rust 文件（C0.3）
  - 实际变更额外包含 C6b.24 的两处 A2 修复文件（`src-tauri/src/setup.rs`、`src-tauri/src/vault_plugin.rs`），以及 C6.3 要求的 `setup.rs` 只读组装配；均为 Rust 侧装配/边界修复，无前端组件、无 DB schema
- [x] C7.8 `cargo test --workspace` 全绿（含全量编译，以及 `assistant-agent` 的 `loop_executor` / `kernel_request_tools` 同步改动，A11）
  - 另跑 `cargo clippy --workspace --all-targets -- -D warnings` 与 `npm run lint` / `npm run typecheck` 一并通过

---

## C8 文档

- [x] C8.1 [I38c-glob §3](./I38c-glob-spec.md) / [I38c-grep §3](./I38c-grep-spec.md) 的依赖表述按 C0.9 定稿回写（`ignore` + `globset` + `grep-*`）
- [x] C8.2 [I38c §11 OQ1](./I38c-os-tools-spec.md) 标注为已定稿（遵守 `.gitignore`）；OQ3 同时定稿（不引入 `windows-sys`，仅补 `windows` features）
- [x] C8.3 回写 [I38a §4-C6](./I38a-tool-control-layer-spec.md)：`ReadReceiptKey` 补 `session_id`（C0.8 的跨阶段契约变更）
- [x] C8.4 回写 [I38d §4.5](./I38d-agent-frontend-modes-spec.md)：函数签名改为 `async` + `Vec<ToolSpec>`，DTO 名补 `Dto` 后缀（与 P5 落点一致）
- [x] C8.5 更新 I38 roadmap §2：P5 标记已完成并记录日期
- [x] C8.6 在本清单勾选 C1–C7 完成项（任务级进度只在本清单维护）

---

## 验收映射

| 规格验收 | 对应任务 |
|---|---|
| A1 路径只经 `os::path`，无回退规范化 | C0.4、C2.4、C3.4、C4.4、C7.5 |
| A2 每个工具 `input_schema` 单测（合法 / 缺必填 / 类型错误） | C2.4、C2.7、C2.14、C2.18、C2.23、C3.5、C3.8、C3.14、C3.17、C4.1 |
| A3 只读工具 intents 为空 / 并发安全；写删工具 action 正确 | C2.2、C2.3、C3.3、C4.3 |
| A4 `edit` 3 分支 + 渐进匹配 + 写回形态 + 审批载荷含 diff（含 `read` 行号前缀 / ` [truncated]` 契约） | C3.8–C3.13、C2.10 |
| A4b `edit` 读前置 guard（无回执 / 过期 / `write` 后不误拦） | C3.12、C3.13 |
| A4c `write` 原子写（含 `sync_all`）、幂等不写盘、不可读拒绝覆盖 | C3.6、C3.7 |
| A5 `delete` 拒绝不可删目标、链接不跟随、预扫描与结构化结果、审批键按规范化路径 | C4.1–C4.11、C1.12 |
| A5b `invalidate_path` 按组件级前缀失效（删除 / 移动后回执不复用旧内容） | C1.12、C3.20、C4.10、C4.11 |
| A6 `shell` 守卫与进程级 kill | 不适用（`shell` 暂不提供，见 [I38c §13](./I38c-os-tools-spec.md)） |
| A7 分组注册：默认只注册只读组 | C0.11、C6.1–C6.7、C7.7 |
| A8 `read` 读取回执（含跨会话隔离） | C1.8–C1.11、C2.11–C2.13 |
| A9 测试、clippy、fmt | C7.1–C7.3 |
| A10 `resolve_available_tools` 过滤与确定性 | C6b.1–C6b.5、C6b.23–C6b.29 |
| A11 P4 遗留清理（`ToolCatalog` 全链路删除） | C6b.16–C6b.22、C7.10 |

---

## 不做

- 不改 `Tool` trait、`ToolPipeline`、`ToolGateway`、Policy 规则集
- 不改 DB schema、不新增表
- 不把修改组 / 危险组加入生产注册；不做运行时注册开关
- 不引入 `windows-sys`；不新增原生构建依赖
- 不做前端 ToolBox 组件、`os.ts` 命名空间、审批 UI（P6）
- 不实现 `ask_user`（P6 联调）
- 不实现 MCP 工具（P7）
- 不做白名单配置的前端 UI（AgentMode 编辑表单、对话工具栏）——P5 只做后端 `resolve_available_tools`，前端 UI 仍归 P6
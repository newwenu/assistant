# I38c — P5 OS 工具能力任务清单

> 规格：[I38c-os-tools-spec.md](./I38c-os-tools-spec.md)（索引：§3 实现顺序、§4 共同结构、§10 验收标准）+ 逐工具分册（入口见索引 §5 / §6 / §7 的表格）
> 阶段：[I38-agent-roadmap.md](../../store/I38-agent-roadmap.md) P5
> 顺序：C0 → C1 → C2 → C3 → C4 → C6 → C6b → C7 → C8（C5 已移除，`shell` 暂不提供）；C9 为 P5 之后的独立增量（P11 候选），不参与 P5 门禁
> 勾选条件：代码已改 **且** 对应验收已跑。
> **任务项粒度**：一项 = 一个可独立完成、可独立验证的工作单元。语义与边界条件写规格，任务项只写「落点 + 判定条件」，不复制规格原文；同一工具的测试合并为一项，用例清单见规格 §10。任务项内**禁止写行号**，只写符号名（行号会随改动立刻腐化）。
> 范围：`src-tauri/crates/assistant-tools/src/os/*`、`os/mod.rs` 分组注册、`src-tauri/src/tool_setup.rs` 的注册调用、workspace / crate 依赖；不新增前端组件、不改控制层、不改 DB、不做 MCP。**例外（C6b）**：为接入白名单解析，新增 `ConversationToolConfigDto` / `ToolTierDto` / `ApprovalLevelDto`，`SendChatMessageArgsDto` 增 `tool_config` 字段，`TurnSpec.tools` 收窄为 `Option<Vec<ToolSpec>>`，并删除 P4 的 `ToolCatalog` 接入层（详见 C6b）。
> 测试位置：公开行为测试放 `src-tauri/crates/assistant-tools/tests/<模块>.rs` 集成测试；仅当必须访问私有函数时才保留源文件内 `#[cfg(test)]` 模块（与 `os/path.rs`、`os/fs_link.rs` 现状一致）。
> **状态唯一位置**：本清单是 P5 任务级进度的唯一来源（含逐个工具的实现状态）；阶段级结论只在 [roadmap](../../store/I38-agent-roadmap.md) §2 写一次（roadmap §7）。本清单**不管理提交**（不写提交拆分 / 提交信息 / 回滚批次，见 roadmap §7 第 2 条）。

---

## C0 口径与前置确认

- [x] C0.1 确认 P1 已通过：`os/path.rs` 的 `resolve` / `resolve_leaf_preserving` / `canonicalize_lenient` / `reject_system_dirs` / `ensure_within_root` 可用，`fs_link` 已迁移
- [x] C0.2 确认 P2 已通过：`ToolPipeline`（`find → validate → gateway → execute → persist`）、`ToolGateway`、`partition_tool_batches`、`select_persist_targets` 可用；P5 不改控制层
- [x] C0.3 确认 P5 不实现前端组件、审批 UI、`ask_user`、MCP、RoundExecutor
- [x] C0.4 确认所有路径工具只经 `os::path`：读 / 写 / 删 / 移动一律用 `resolve()`；`fs_link` 的叶子保留语义仍用 `resolve_leaf_preserving()`；不新增私有 canonicalize 或失败回退
- [x] C0.5 确认 `os_tool_exec` 恒向 `params` 注入 `action`（已核对 `src-tauri/src/cmd/os_tool_exec.rs`）：新工具的 `input_schema` 不声明 `action`、不设 `additionalProperties: false`，`execute()` 忽略未知字段（ToolBox 路径不经过 `validate_input`）
- [x] C0.6 确认 `workspace_root` 来源不在 P5（I18 spec B1 标注为 P5/P6）：P5 只保证绝对路径可用；ToolBox 侧相对路径仍不可用（`ToolUseContext::for_toolbox` 传 `workspace_root = None`），待 P6 补工作区来源
  - 后续（2026-11-09 补）：已按 I18b §2.2 的「解析基准 / 权限边界分离」落地——`ToolUseContext` 新增 `resolve_base`，`for_toolbox` 只填基准、`workspace_root` 仍为 `None`；`os_tool_exec` 用 `workspace_root_from_conf()` 作基准，ToolBox 相对路径可用且不产生越界规则。详见 [I38 §2 G3](./I38-agent-code-gaps.md)
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
- [x] C1.9 `ReadReceiptKey` 补 `session_id: Option<String>`（C0.8），`InMemoryReadReceiptStore` 以其作键的一部分；不同会话的回执互不影响
  - 2026-11-09 修订：去重设计移除后键收窄为 `(session_id, path)`，`start_line` / `end_line` 已从键中删除（见 C2.11）
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
- [x] C2.11 读取回执**不做去重**（2026-11-09 修订，原「同一 session + 同一路径 + 同一行范围且修订号未变 → 返回『已在上文返回』」已撤销）：每次 `read` 都完整回传内容，成功读取刷新该 `(session, path)` 回执的修订号
- [x] C2.12 回执修订号变化不触发任何「重读 / 复用」分支——`read` 无去重分支；修订号只供 `edit` 读前置判过期
- [x] C2.13 测试：schema、缺 `path`、`offset` / `limit` 边界、二进制拒绝、截断提示、长行 `[truncated]`、行号前缀格式、重复读取仍完整回传、回执刷新、空文件

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

实现：`src-tauri/crates/assistant-tools/src/os/mod.rs`、`src-tauri/src/tool_setup.rs`
测试：`src-tauri/crates/assistant-tools/tests/registration.rs`

- [x] C6.1 `os/mod.rs` 提供 `register_readonly_tools` / `register_write_tools` / `register_dangerous_tools` 三组函数
- [x] C6.2 `register_os_tools` 只保留 `fs_link`（`TOOLBOX_ONLY`），不与新工具混在同一函数
- [x] C6.3 `tool_setup::init`（装配入口，由 `setup.rs` 调用）调用 `os::register_os_tools` + `os::register_readonly_tools`；不调用修改组 / 危险组（A7）。只读组需要回执端口，`tool_setup` 装配 `InMemoryReadReceiptStore` 并 `handle.manage`
  - 后续：P6 由 [I38d C9.5a](./I38d-agent-frontend-modes-tasklist.md) 追加修改组、[C9.5b](./I38d-agent-frontend-modes-tasklist.md) 追加危险组；本项描述保留 P5 时的形态
- [x] C6.4 单测：只读组注册后包含 `read` / `ls` / `glob` / `grep`，且 `fs_link` 仍在
- [x] C6.5 单测：未调用修改组 / 危险组时，`find_for_toolbox` 查不到 `write` / `edit` / `mkdir` / `move` / `delete`
- [x] C6.6 单测：`tool_definitions_for_tier(ToolTier::Os)` 仅返回 4 个只读工具且按名排序（`fs_link` 被 `audience` 过滤）
  - 后续：随修改组 / 危险组启用，该断言已扩为「生产注册形态下返回 9 个 OS 工具」，见 `registration.rs` 的 `tool_definitions_for_os_tier_expose_every_os_facing_tool`
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
- [x] C7.5 确认 `tool_setup.rs` 生产注册表只含 `fs_link` + 只读四工具（A7）
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

## C9 多编码支持（P11 候选，不参与 P5 门禁）

> 规格：[I38c §14](./I38c-os-tools-spec.md)（跨工具约束）+ [read §3.3](./I38c-read-spec.md) / [write §3.1](./I38c-write-spec.md) / [edit §5](./I38c-edit-spec.md) / [grep §3.1](./I38c-grep-spec.md)
> 阶段：[I38-agent-roadmap.md](../../store/I38-agent-roadmap.md) §2.1（P11 候选）
> 前置：P5 已完成（C0–C8 全绿）；依赖 `encoding_rs` / `chardetng` / `encoding_rs_io` 均已在 crate 依赖中，**不新增依赖**
> 勾选条件：代码已改 **且** 对应验收（A12–A18）已跑。
> **门禁**：本组任一项未完成都不影响 P5 结论；不得为 C9 改动 P0–P7 的既有验收口径。
> **状态**：C9.1–C9.34 全部完成，A12–A18 已跑（`cargo test -p assistant-tools` / `-p assistant-agent`、clippy `-D warnings`、`fmt --check` 全通过）。

实现：`src-tauri/crates/assistant-tools/src/os/{text_encoding,agent_read,agent_write,agent_grep,process_output}.rs`、`src-tauri/crates/assistant-tools/src/os/agent_read/window.rs`（长文件分块扫描）、`src-tauri/crates/assistant-tools/src/os/agent_edit/`（`locate.rs` / `apply.rs`）、`src-tauri/crates/assistant-tools/src/storage.rs`
测试：`src-tauri/crates/assistant-tools/tests/{text_encoding,agent_read,agent_write,agent_edit,agent_grep,storage}.rs`

**共享检测模块**

- [x] C9.1 新建 `os/text_encoding.rs`：从 `os/process_output.rs` 抽出 `EncodingDetector` + `decode_process_output` 的编码判定部分，提供 `detect_and_decode(bytes, explicit) -> Result<DecodedText, DecodeError>`（返回 `text` / `encoding` / `has_bom` / `source`）与 `encode_text(text, encoding, has_bom) -> Result<Vec<u8>, EncodeError>`；`process_output` 改为 re-export 该模块，行为不变（`IBM866 → WINDOWS_1252` 修正规则随迁）
- [x] C9.2 `detect_and_decode` 的判定顺序固定为 BOM → 显式 `encoding`（无 BOM 时）→ 二进制判定（原始字节前 8KB 含 `0x00`）→ UTF-8 整文件校验 → `chardetng` 猜测；猜测仅在 `is_confident == true` **且** 整文件解码 `had_errors == false` 时接受（A12）
  - 二进制判定**必须**排在 UTF-8 校验之前：纯 ASCII 的无 BOM UTF-16（`h\0i\0`）本身是合法 UTF-8，顺序反过来会被静默当成「带 NUL 的文本」读入。
- [x] C9.3 `encode_text` 有损（`had_errors == true`）返回 `EncodeError`，绝不返回 `?` 替换后的字节；BOM 按编码规则补写（UTF-16 恒补，UTF-8 由调用方 `has_bom` 决定）（A12 / A15）
  - UTF-16 不能走 `Encoding::encode`：`encoding_rs` 的该 API 是 Web 语义，对 UTF-16 输出 UTF-8；UTF-16 写回按 `encode_utf16` 逐码元拼接。
- [x] C9.4 单测：UTF-8 / UTF-8+BOM / UTF-16LE+BOM / GBK / Big5 / Shift_JIS / windows-1252 各一例；带 BOM 的 UTF-16 不被二进制检测误杀；无 BOM 的 ASCII UTF-16 在 UTF-8 校验前被拒；不可编码字符（如 emoji 写入 GBK）返回错误；`process_output` 既有用例全部保持通过

**回执与 `read`**

- [x] C9.5 `storage.rs`：`ReadReceipt` 增 `encoding: String` 与 `has_bom: bool`；`InMemoryReadReceiptStore` 读写路径同步；`storage.rs` 单测覆盖新字段
- [x] C9.6 `agent_read.rs`：`input_schema` 增 `encoding`（可选）；`execute` 改用 `detect_and_decode`，删除 `String::from_utf8(bytes)` 硬失败分支；BOM 在解码时剥离，不进入渲染（A12）
- [x] C9.7 `agent_read.rs`：footer 一次性追加到结果末尾，UTF-8 无 BOM 时**不输出**；来源后缀 `(bom)` / `(guess)` 按实际生效路径决定；footer 不进 `data` 正文（A13）
- [x] C9.8 `agent_read.rs`：成功时把 `encoding` / `has_bom` 写入回执；回执键为 `(session_id, path)`，每次读取覆盖刷新（A14）
- [x] C9.9 测试：schema 合法 / 非法 label；GBK 文件读出的正文与 footer；带 BOM 的 UTF-16 文件可读且首行无 `U+FEFF`；无 BOM 的 UTF-16 被拒并提示传 `encoding`；二进制拒绝；显式 `encoding` 覆盖猜测；UTF-8 无 BOM 时无 footer；回执含新字段
  - 实测补充：`chardetng` 的 `guess_assess(None, true)` 对非空输入恒返回 `is_confident == true`（1 字节到 200 字节均如此），故「猜测不置信」分支实际不可达；真正生效的兜底是「猜测到的编码整文件解码有损」（如猜成 EUC-KR / GBK 却含 `0xFF`）。单测按后者构造。

**`write` 转码**

- [x] C9.10 `agent_write.rs`：`input_schema` 增 `encoding`（可选）；编码解析顺序为 `encoding` → 已存在文件回执 → UTF-8（G1）
- [x] C9.11 `agent_write.rs`：五道闸 G1–G5 全部生效，任一不过即 `Business` 且不写盘；`append` 与转码叠加时按「解码现有 → 拼接 → 整体编码」处理，现有内容解码有损即拒绝（A15）
- [x] C9.12 `agent_write.rs`：幂等判定改为字节级（编码后字节 vs 现有字节）；写后回执刷新为实际写出的 `encoding` / `has_bom`（A14）
- [x] C9.13 测试：新建 GBK 文件；无回执时对已存在文件要求 GBK → 拒绝（G4）；emoji 写入 GBK → 拒绝（G2）；>1MB 转码写入 → 拒绝（G3）；UTF-8 无 BOM 路径行为与 P5 一致（含幂等不写盘）；`append` 到 GBK 文件后整体仍为 GBK 且无非法序列

**`edit` 写回**

- [x] C9.14 `agent_edit/report.rs`：`not_found_message` 增一段诊断——`old_string` 末尾独占一行的 footer 标记集命中时提示「这不是文件内容，请移除后重试」；只加提示、不改判定，**不检查 `new_string`**（A13，上级 §14.4 规则 4）
- [x] C9.15 `agent_edit/apply.rs` + `mod.rs`：写回用回执 `encoding` + `has_bom`；`encode` 有损即 `Business` 且不写盘；写后回执刷新（A14 / A15）
- [x] C9.16 测试：GBK 文件 `edit` 后仍为 GBK；带 BOM 的 UTF-8 文件编辑后 BOM 保留；`old_string` 尾部带 footer 时 0 匹配且诊断含移除提示（重试去掉 footer 后成功）；源文件末行本身即 `[encoding: …]` 时档 1 原样命中、不被误判；`new_string` 含目标编码不可表示字符 → 拒绝且文件未变；无回执仍拒绝
  - 实测补充：`edit` 解码时仍先查 BOM，因此「回执 `has_bom` 与实际字节不一致」时以文件自述为准（回执只作解码闸门，不作覆盖依据）；回执刷新写入的是实际落盘的 `encoding` / `has_bom`

**`grep` 编码与探测**

- [x] C9.17 `agent_grep.rs`：`SearcherBuilder` 增 `binary_detection(BinaryDetection::quit(b'\x00'))`，保持 `mmap` 关闭、`bom_sniffing` 默认开；`binarySkipped` 计数（A16）
- [x] C9.18 `agent_grep.rs`：`input_schema` 增 `encoding`；非法 label → `Business`；设置后作为 scope 级开关传入 `SearcherBuilder::encoding`（A16）
- [x] C9.19 `agent_grep.rs`：`trim_line_end` 改为 `from_utf8` 快速路径 + 失败时 `from_utf8_lossy` 并置 `lossy_lines`，移除静默 `from_utf8_lossy`（A16）
- [x] C9.20 `agent_grep.rs`：零命中时执行有界 probe（复用单缓冲、前 8KB、跳过 BOM / 二进制 / 合法 UTF-8、`PROBE_FILE_LIMIT = 200`、尊重内部 deadline），结果写 `nonUtf8Probe`，计数 > 0 时输出 `[non-utf8: N file(s)]` footer（A16）
- [x] C9.21 测试：GBK 目录传 `encoding` 能命中；不传时零命中且 footer 报 `non-utf8`；有命中时不执行 probe（`nonUtf8Probe == null`）；二进制文件不产出乱码行且计入 `binarySkipped`；probe 到上限时 `truncated == true`；UTF-8 目录零命中时无 footer
  - 实测补充：probe 的 UTF-8 判定把 `Utf8Error::error_len() == None`（8KB 窗口截断在半个字符处）算作合法 UTF-8，否则大文件会被误报为「非 UTF-8」；`binary_data` 返回 `Ok(false)` 才是 `quit` 语义（返回 `true` 会继续读同一文件并产出乱码行）；footer 严格保持 `[non-utf8: N file(s)]` 形态，截断信息只进 `nonUtf8Probe.truncated`（不延长 footer，避免喧宾夺主）

**`grep` 输出预算与边界（二修，规格 [grep §3.2 / §3.3](./I38c-grep-spec.md) / 上级 A17）**

- [x] C9.25 `agent_grep.rs`：`pattern` 原样编译（`trim` 只用于判空，不改编译字符串）；`context` 校验 `0..=MAX_CONTEXT(10)`，超出 `Business`；`encoding` 校验与执行统一走 `SearcherEncoding::new`，非法 label 在 `validate_input` 阶段即返回（A17）
- [x] C9.26 `agent_grep.rs`：输出预算——`MAX_OUTPUT_LINES = 300`（命中行 + 上下文行 + `--` 分隔符合计，`context_break` 与行共用预算），`Sink::matched` 与 `Sink::context` 共用同一预算；`context` 回调补 deadline 检查；`truncated` 语义改为「任一上限触发」，不新增原因字段（A17）
- [x] C9.27 `agent_grep.rs`：按文件缓冲并在文件结束时提交（路径字典序 → 行号升序）；`Sink::binary_data` 整文件丢弃（NUL 之前的命中不输出）；超 `16 MiB` 文件在 `decide_file` 用 `metadata.len()` 跳过并计 `largeSkipped`（不用 `WalkBuilder::max_filesize`：遍历层静默丢弃、无法计数）；`search_path` 的 `Err` 计 `unreadableFiles`（A17）
- [x] C9.28 `agent_grep.rs`：`include` 改 `literal_separator(true)`，pattern 不含 `/` 时自动前置 `**/`（与 `glob` 对齐，两者共用 `agent_glob::build_glob_set`）；`encoding.is_some()` 时跳过零命中 probe（`nonUtf8Probe == null`）（A17）
- [x] C9.29 `agent_grep.rs`：footer 增 `[skipped: N file(s)]`（`binarySkipped + largeSkipped + unreadableFiles > 0`）与 `[lossy: N line(s)]`（`lossyLines > 0`）；`lossyLines` 覆盖上下文行；超时且零行时不再产生前导换行（A17）
- [x] C9.30 测试：`pattern` 带前导空格可命中、全空白被拒；`context = 11` 被拒；单文件大 `context` 被行数上限截断且 `truncated == true`；二进制文件 NUL 前的命中不输出且计入 `binarySkipped`；超 `16 MiB` 文件计入 `largeSkipped`；不可读文件计入 `unreadableFiles` 且 footer 出现（`#[cfg(windows)]` 用 `share_mode(0)` 独占句柄模拟）；`include: "src/*.rs"` 不跨层级、`*.rs` 递归；已设 `encoding` 时 `nonUtf8Probe == null`；`glob` 侧补 `*.ts` 递归 / `src/*.ts` 不递归两例（A17）
  - 实测补充：`context` 回调的 deadline 检查无法在测试里确定性触发——每个块的上下文行紧跟在同块的 `matched` 之后（微秒级），能到点的场景必然先在 `matched` 或遍历入口触发；该检查保留为防御（单文件内 `before_context` 回溯与 `after_context` 逐行产出都可能落在慢盘上），不写断言

**长文件流式读取（三修，规格 [read §3.4](./I38c-read-spec.md) / 上级 §14.10 / A18）**

- [x] C9.31 `agent_read/window.rs`：新建分块扫描器——`CHUNK_BYTES = 256KB` 块 + 换行切分；头部 8KB 做 BOM / NUL 判定；UTF-8 乐观路径逐块 `from_utf8`（`error_len() == None` 且非 EOF 时把尾巴留给下一块）；后续块真非法时按 `GUESS_SAMPLE_BYTES = 1MB` 样本重猜并从头流式解码；非 UTF-8 由同一个 `encoding_rs::Decoder` 跨块续解（A18）
  - 实现要点：`Decoder` 内部区分 `Locked(encoding_rs::Decoder)` 与 `Utf8 { carry }` 两种形态；`read_chunk` 用 `Read::take` 读满 `CHUNK_BYTES` 才判定非 EOF，因此「读满恰好等于块大小」不会被误判为文件结束。
- [x] C9.32 `agent_read/window.rs`：行窗口——`limit` 与 1MB / 2000 行取小；窗口填满后按 `EOF_GRACE_CHARS = 1MB` 只计数不渲染；`totalLines` 读到 EOF 才给值、否则 `None`；截断提示按 EOF 到达与否给精确 / 下界两种形态；超长单行累积到 `MAX_LINE_CHARS` 即停并置截断标记（A18）
  - 实现要点：`line_cap_is_output_cap` 记录「本次截断是输出上限所致还是 `limit` 所致」，`limit` 主动截断不算 `truncated`；`total_rendered_bytes` 在只计数阶段继续累加，供「N of M bytes」的精确形态使用。
- [x] C9.33 `agent_read.rs`：删除整文件 `std::fs::read` 与 `MAX_READ_FILE_BYTES` 闸门，改调 `window::scan`；闸门重命名 `MAX_WHOLE_FILE_BYTES` 归 `edit` 使用，`file_too_large_message` 拆成 `edit` 专用文案（不再提 `read` / `grep`）；`data.totalLines` 输出 `Option`（未达 EOF 为 `null`）（A18）
  - 附带：`read` 的 `description` 同步收紧为只写可观察契约（任意大小可读、单次 2000 行 / 1MB、`offset` / `limit` 翻页、`totalLines` 可能为 `null`），分块大小 / 缓冲策略 / 解码器复用等机制不写进描述（见 [read §3.4](./I38c-read-spec.md)）；`encoding_footer` 改为接收 `DecodedInfo`，`truncate_line` 由 `window` 内的 `append` 取代。
- [x] C9.34 测试：>16MiB 文件可读且 `offset` 翻页；块边界切断多字节字符时 GBK / UTF-8 正文仍正确（构造恰好跨块的字符）；超长单行不撑爆内存；`totalLines` 精确值与 `null` 两例；截断提示两种形态；`edit` 超限文案不提 `read`（A18）
  - 实测：`agent_read` 37 例全绿（新增 `a_gbk_character_split_across_chunks_is_decoded` / `a_utf8_character_split_across_chunks_is_decoded` / `files_above_the_old_whole_file_limit_are_readable` / `a_single_line_longer_than_the_output_budget_is_cut_without_loading_it` / `a_utf8_file_that_turns_invalid_later_falls_back_to_a_guess`）；`agent_edit` 29 例全绿（新增 `a_file_above_the_whole_file_limit_is_rejected_with_an_edit_only_message`）。
  - 构造手法：用 `"ab\n"` 重复到 `CHUNK_BYTES - 1` 字节（每行 3 字节整除），使紧随其后的多字节字符第一个字节落在块边界前、其余字节落在下一块；UTF-8 回退用例改为「头部 >8KB 全 ASCII + 之后 3 万行 GBK 正文」，确保触发「头部合法、后续块真非法」而非块尾截断分支。

**文档与验收**

- [x] C9.22 回写 [I38a §4-C6](./I38a-tool-control-layer-spec.md) 的 `ReadReceipt` 值扩展（规格已写，落地后确认与代码一致）
  - 核对结论：规格写的 `encoding: String` + `has_bom: bool`、键结构不变、`revision` 仍是「长度 + mtime」且编码不参与失效，与 `storage.rs` 的 `ReadReceipt` 逐项一致，无需改文档
- [x] C9.23 `cwd = src-tauri`：`cargo test -p assistant-tools` / `cargo clippy -p assistant-tools --all-targets -- -D warnings` / 先修格式化再 `cargo fmt --all --check` 全通过
- [x] C9.24 回归确认：P5 的 A1–A11 验收仍全绿（UTF-8 路径行为未变，唯一差异是 §14.3 的二进制检测顺序修正）
  - 实测：`cargo test -p assistant-tools` 16 个测试目标全绿（含 `agent_read` 37 / `agent_write` 24 / `agent_edit` 29 / `agent_grep` 20）；`cargo test -p assistant-agent` 全绿（A11 的 `ToolCatalog` 清理链路）；`grep` 的 `[non-utf8: …]` tag 已在 §14.4 规则 5 登记，无需回写

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
| A5b `invalidate_path` 按组件级前缀失效（删除 / 移动后旧回执不再放行 `edit`） | C1.12、C3.20、C4.10、C4.11 |
| A6 `shell` 守卫与进程级 kill | 不适用（`shell` 暂不提供，见 [I38c §13](./I38c-os-tools-spec.md)） |
| A7 分组注册：默认只注册只读组 | C0.11、C6.1–C6.7、C7.7 |
| A8 `read` 读取回执（每次完整回传、刷新修订号与编码、跨会话隔离） | C1.8–C1.11、C2.11–C2.13 |
| A9 测试、clippy、fmt | C7.1–C7.3 |
| A10 `resolve_available_tools` 过滤与确定性 | C6b.1–C6b.5、C6b.23–C6b.29 |
| A11 P4 遗留清理（`ToolCatalog` 全链路删除） | C6b.16–C6b.22、C7.10 |
| A12（C9）检测顺序 / BOM 不误杀 / 有损一律拒绝 | C9.1–C9.4、C9.6、C9.9 |
| A13（C9）footer 契约与 `edit` 标记集诊断 | C9.7、C9.14、C9.16、C9.20 |
| A14（C9）回执携带编码，写侧只消费不重判 | C9.5、C9.8、C9.12、C9.15 |
| A15（C9）`write` 五道闸 | C9.3、C9.10、C9.11、C9.13、C9.15 |
| A16（C9）`grep` 标准配置链与两层探测 | C9.17–C9.21 |
| A17（二修）`grep` 输出预算与边界 | C9.25–C9.30 |
| A18（三修）`read` 流式分块读，闸门归 `edit` / `write` / `grep` | C9.31–C9.34 |

---

## 不做

- 不改 `Tool` trait、`ToolPipeline`、`ToolGateway`、Policy 规则集
- 不改 DB schema、不新增表
- 不把修改组 / 危险组加入生产注册（P5 范围内）；P6 由 [I38d C9.5a / C9.5b](./I38d-agent-frontend-modes-tasklist.md) 按门禁启用；不做运行时注册开关
- 不引入 `windows-sys`；不新增原生构建依赖
- 不做前端 ToolBox 组件、`os.ts` 命名空间、审批 UI（P6）
- 不实现 `ask_user`（P6 联调）
- 不实现 MCP 工具（P7）
- 不做白名单配置的前端 UI（AgentMode 编辑表单、对话工具栏）——P5 只做后端 `resolve_available_tools`，前端 UI 仍归 P6
- 不做多编码支持的代码落地（C9 单列为 P11 候选）；不引入 `iconv` / 外部转码进程 / 新依赖；不做逐文件编码猜测；不做大文件转码写回（[I38c §11 OQ7](./I38c-os-tools-spec.md)）
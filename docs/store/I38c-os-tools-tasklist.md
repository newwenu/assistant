# I38c — P5 OS 工具能力任务清单

> 规格：[I38c-os-tools-spec.md](./I38c-os-tools-spec.md) §3 实现顺序、§4–§9 工具规格、§10 验收标准
> 阶段：[I38-agent-roadmap.md](./I38-agent-roadmap.md) P5
> 顺序：C0 → C1 → C2 → C3 → C4 → C5 → C6 → C7 → C8
> 勾选条件：代码已改 **且** 对应验收已跑。
> 范围：`src-tauri/crates/assistant-tools/src/os/*`、`os/mod.rs` 分组注册、`src-tauri/src/setup.rs` 的注册调用、workspace / crate 依赖；不新增前端组件、不改控制层、不改 DTO / DB、不做 MCP。
> 测试位置：公开行为测试放 `src-tauri/crates/assistant-tools/tests/<模块>.rs` 集成测试；仅当必须访问私有函数时才保留源文件内 `#[cfg(test)]` 模块（与 `os/path.rs`、`os/fs_link.rs` 现状一致）。
> **状态唯一位置**：本清单是 P5 任务级进度的唯一来源（含逐个工具的实现状态）；阶段级结论只在 [roadmap](./I38-agent-roadmap.md) §2 写一次（roadmap §7）。本清单**不管理提交**（不写提交拆分 / 提交信息 / 回滚批次，见 roadmap §7 第 2 条）。

---

## C0 口径与前置确认

- [ ] C0.1 确认 P1 已通过：`os/path.rs` 的 `resolve` / `resolve_leaf_preserving` / `canonicalize_lenient` / `reject_system_dirs` / `ensure_within_root` 可用，`fs_link` 已迁移
- [ ] C0.2 确认 P2 已通过：`ToolPipeline`（`find → validate → gateway → execute → persist`）、`ToolGateway`、`partition_tool_batches`、`select_persist_targets` 可用；P5 不改控制层
- [ ] C0.3 确认 P5 不实现前端组件、审批 UI、`ask_user`、MCP、RoundExecutor
- [ ] C0.4 确认所有路径工具只经 `os::path`：读 / 写 / 删 / 移动一律用 `resolve()`；`fs_link` 的叶子保留语义仍用 `resolve_leaf_preserving()`；不新增私有 canonicalize 或失败回退
- [ ] C0.5 确认 `os_tool_exec` 恒向 `params` 注入 `action`：新工具的 `input_schema` 不声明 `action`、不设 `additionalProperties: false`，`execute()` 忽略未知字段（ToolBox 路径不经过 `validate_input`）
- [ ] C0.6 确认 `workspace_root` 来源不在 P5（I18 spec B1 标注为 P5/P6）：P5 只保证绝对路径可用；ToolBox 侧相对路径仍不可用，待 P6 补工作区来源
- [ ] C0.7 确认 `ReadReceiptStore` 的落地方式：P2 只定义了 trait，P5 需补生产实现并由 `FsReadTool` 构造注入；`ToolUseContext.session_id` 为 `None`（ToolBox）时归入同一桶
- [ ] C0.8 确认 `ReadReceiptKey` 是否补 `session_id`：P2 类型只有 `path` / `start_line` / `end_line`，与规格 §5.1「同一会话内」不符；若补，先回写 I38a 规格再改代码
- [ ] C0.9 确认 `fs_glob` / `fs_grep` 依赖选型（规格 §11 OQ1）：统一用 `ignore` + `globset` + `grep-searcher` / `grep-regex`，遵守 `.gitignore`；若确认，回写 I38c §5.3 / §5.4 中「glob crate / grep crate」的表述
- [ ] C0.10 确认 `shell_exec` 超时口径：Pipeline / `exec::invoke` 外层默认 30s，请求 `timeout` 不得大于外层；进程级超时 + `kill_on_drop`，外层超时不得留下孤儿进程
- [ ] C0.11 确认注册分批：`setup.rs` 只调用只读组；修改组 / 危险组函数存在但**不调用**（修改组注册需 P4 审批端口生产绑定，危险组需 P6 审批 UI）
- [ ] C0.12 确认每个工具显式声明 `audience`：只读 / 修改 / 删除为 `BOTH`；`shell_exec` 仅在全部执行层守卫落地后声明 `BOTH`，任一缺失则改为 `AGENT_ONLY`

---

## C1 共享基础设施与依赖

实现：`src-tauri/Cargo.toml`、`src-tauri/crates/assistant-tools/Cargo.toml`、`src-tauri/crates/assistant-tools/src/os/{output,mod}.rs`、`src-tauri/crates/assistant-tools/src/storage.rs`
测试：`src-tauri/crates/assistant-tools/tests/output.rs`

- [ ] C1.1 workspace `Cargo.toml` 增加 `ignore` / `globset` / `grep-searcher` / `grep-regex`（按 C0.9 定稿）
- [ ] C1.2 workspace `Cargo.toml` 的 Unix 目标增加 `libc`（进程组 kill 用）
- [ ] C1.3 workspace `windows` 依赖补 `Win32_System_JobObjects` / `Win32_System_Threading` 特性（Job Object 杀进程树），不新增 `windows-sys`
- [ ] C1.4 `assistant-tools/Cargo.toml` 以 `workspace = true` 引入 C1.1 / C1.2 的依赖
- [ ] C1.5 新建 `os/output.rs`：`OutputLimits { max_bytes, max_lines, preview_bytes }`、`TruncatedOutput`（`text` / `truncated` / `total_bytes` / `total_lines` / `returned_lines`）、`truncate_output`
- [ ] C1.6 `truncate_output` 在截断时显式写「已截断 + 实际行数 / 字节数」，不静默丢弃
- [ ] C1.7 `os/mod.rs` 增加 `pub mod output;`，并预留各工具模块声明
- [ ] C1.8 `storage.rs` 新增 `InMemoryReadReceiptStore`（`DashMap`），实现 `ReadReceiptStore::lookup` / `record`
- [ ] C1.9 明确「修订号」计算方式：文件长度 + 修改时间（纳秒），不整文件哈希；写入 `fs_read.rs` 注释
- [ ] C1.10 单测：未超限原样返回；超行 / 超字节各一条；截断提示包含实际值

---

## C2 只读组：`fs_read` / `fs_list` / `fs_glob` / `fs_grep`

实现：`src-tauri/crates/assistant-tools/src/os/{fs_read,fs_list,fs_glob,fs_grep}.rs`
测试：`src-tauri/crates/assistant-tools/tests/{fs_read,fs_list,fs_glob,fs_grep}.rs`

**组内共同项**

- [ ] C2.1 四个工具均实现 `name` / `description` / `input_schema` / `tier = Os` / `audience = BOTH` / `behavior_hints.read_only = true` / `destructive = false` / `open_world = false`
- [ ] C2.2 `permission_intents` 返回空 `Vec`（沿用默认实现，A3）
- [ ] C2.3 `is_concurrency_safe` 返回 `true`（只读默认；不因 input 收紧）
- [ ] C2.4 `validate_input` 覆盖必填缺失与类型错误，并对所有路径参数调用 `os::path::resolve`（A1 / A2）
- [ ] C2.5 `execute` 内同步文件 IO / 目录遍历一律包 `tokio::task::spawn_blocking`，不阻塞 tokio worker
- [ ] C2.6 四个工具均能容忍 `os_tool_exec` 注入的多余 `action` 字段（C0.5）

**`fs_read`**

- [ ] C2.7 `input_schema`：`path`（必填）+ `offset`（整数，1 起）+ `limit`（整数）
- [ ] C2.8 默认读全部，上限 1MB / 2000 行；`offset` + `limit` 支持行范围
- [ ] C2.9 二进制检测：读取前 8KB，含 NUL 字节即拒绝，提示改用十六进制 / 文本工具
- [ ] C2.10 超过 1MB 或 2000 行时返回截断提示 + 实际行数 / 字节数（复用 `os/output.rs`）
- [ ] C2.11 读取回执：同一 session + 同一路径 + 同一行范围且修订号未变 → 返回「已在上文返回」，不回传内容
- [ ] C2.12 修订号变化 → 重新读取并刷新回执
- [ ] C2.13 测试：schema、缺 `path`、`offset` / `limit` 边界、二进制拒绝、截断提示、回执复用与刷新、空文件

**`fs_list`**

- [ ] C2.14 `input_schema`：`path`（必填）+ `depth`（默认 1、最大 10）+ `all`（包含隐藏项）
- [ ] C2.15 输出按名称排序，每层最大 1000 条目，超出时显式提示
- [ ] C2.16 目录遍历不跟随符号链接（避免越界读取）；对每个条目仍保持 workspace 边界结论一致
- [ ] C2.17 测试：默认深度、深度夹取、隐藏项开关、排序、1000 条上限、越界路径拒绝、符号链接目录不被跟随

**`fs_glob`**

- [ ] C2.18 `input_schema`：`pattern`（必填）+ `path`（基准目录）+ `exclude`（数组）
- [ ] C2.19 `path` 缺省时取 `workspace_root`；无 `workspace_root` 则返回明确错误
- [ ] C2.20 使用 `ignore` 的 walker + `globset` 处理 pattern / exclude，遵守 `.gitignore`（C0.9）
- [ ] C2.21 结果只含 workspace 内路径，最大 1000 条，按路径字典序排序
- [ ] C2.22 测试：pattern 命中、exclude 生效、1000 条上限、越界拒绝、缺省基准目录、排序稳定

**`fs_grep`**

- [ ] C2.23 `input_schema`：`pattern`（必填）+ `path` + `include`（文件 glob）+ `context`（默认 0）
- [ ] C2.24 使用 `ignore` 遍历 + `grep-searcher` / `grep-regex` 搜索，`include` 用 `globset` 过滤
- [ ] C2.25 输出含文件路径、行号、匹配行与 context 行；最大 100 条匹配
- [ ] C2.26 非法正则返回 `ToolResult::err`（`Business`），不 panic、不中断 turn
- [ ] C2.27 测试：匹配与行号、context 行数、include 过滤、100 条上限、非法正则、越界拒绝

---

## C3 修改组：`fs_write` / `fs_edit` / `fs_mkdir` / `fs_move`

实现：`src-tauri/crates/assistant-tools/src/os/{fs_write,fs_edit,fs_mkdir,fs_move}.rs`
测试：`src-tauri/crates/assistant-tools/tests/{fs_write,fs_edit,fs_mkdir,fs_move}.rs`

- [ ] C3.1 四个工具 `behavior_hints.read_only = false`、`destructive = false`、`open_world = false`、`audience = BOTH`
- [ ] C3.2 `is_concurrency_safe` 保持默认（写类串行），不显式放宽
- [ ] C3.3 `permission_intents` 产出 `edit` + 受影响路径（A3）；`fs_move` 的 `source` / `target` 各一条
- [ ] C3.4 `validate_input` 校验必填 / 类型并调用 `os::path::resolve`；`execute` 仍自行校验，不依赖 Pipeline

**`fs_write`**

- [ ] C3.5 schema：`path` / `content`（必填）+ `append`（默认 false）；覆盖已有文件自动创建父目录
- [ ] C3.6 单次写入上限 10MB，超出返回 `Business`；`append = true` 时追加而非覆盖
- [ ] C3.7 测试：新建、覆盖、追加、父目录自动创建、10MB 拒绝、越界拒绝

**`fs_edit`**

- [ ] C3.8 schema：`path` / `old_string` / `new_string`（必填）+ `replace_all`（默认 false）
- [ ] C3.9 匹配语义：0 匹配 → `Business`；1 匹配 → 替换；多匹配且 `replace_all = false` → `Business`（附匹配次数与首个匹配行号）
- [ ] C3.10 多匹配且 `replace_all = true` → 全部替换并返回替换次数
- [ ] C3.11 成功结果返回起始行与 diff（新增 / 删除行数 + 片段）；审批卡片所需 diff 由 P6 从 `ApprovalRequest.arguments` 渲染，不新增载荷字段
- [ ] C3.12 测试：0 / 1 / 多匹配三分支、`replace_all` 计数、缩进与 CRLF 原样匹配、diff 形状

**`fs_mkdir`**

- [ ] C3.13 schema：`path`（必填）+ `parents`（默认 true）
- [ ] C3.14 `parents = false` 且父目录不存在 → `Business`；目录已存在时幂等返回成功并注明
- [ ] C3.15 测试：单层 / 多层创建、`parents = false` 失败、已存在幂等、`edit` intent

**`fs_move`**

- [ ] C3.16 schema：`source` / `target`（必填）+ `overwrite`（默认 false）
- [ ] C3.17 目标已存在且 `overwrite = false` → `Business`；`overwrite = true` 时先删目标再重命名
- [ ] C3.18 仅用 `std::fs::rename`；跨设备失败返回 `Business`，**不**静默降级为复制 + 删除
- [ ] C3.19 测试：移动文件 / 目录、`overwrite` 两分支、两条 `edit` intent、目标父目录不存在

---

## C4 破坏性组：`fs_delete`

实现：`src-tauri/crates/assistant-tools/src/os/fs_delete.rs`
测试：`src-tauri/crates/assistant-tools/tests/fs_delete.rs`

- [ ] C4.1 schema：`path`（必填）+ `recursive`（默认 false）
- [ ] C4.2 `behavior_hints`：`read_only = false`、`destructive = true`、`open_world = false`；`is_concurrency_safe = false`
- [ ] C4.3 `permission_intents` 产出 `delete` + **`resolve()` 返回的规范化绝对路径**（供 P2 `ApprovalKey::Path` 归一，A5）
- [ ] C4.4 拒绝删除 `workspace_root` 本身：比较 `resolve()` 结果与 `canonicalize_lenient(workspace_root)`；`workspace_root = None` 时不做该比较
- [ ] C4.5 `recursive = false` 删除非空目录 → `Business`；`recursive = true` 删除目录及内容；不存在的路径 → `Business`
- [ ] C4.6 测试：删文件、非空目录非递归失败、递归删除、根目录自身拒绝（含尾随分隔符 / 链接形式）、`delete` intent、`..` / 符号链接归一后同一审批键

---

## C5 破坏性组：`shell_exec` 与执行层守卫

实现：`src-tauri/crates/assistant-tools/src/os/shell_exec.rs`、`os/shell/{safety_belt,structural_guard,env_guard,classify,bare_exec}.rs`
测试：`src-tauri/crates/assistant-tools/tests/shell_exec.rs`

- [ ] C5.1 schema：`command`（必填）+ `timeout`（秒，默认 30）+ `cwd`
- [ ] C5.2 `os/shell/safety_belt.rs`：致命命令清单 + 分词 / 基名提取，作为**唯一命令判定源**
- [ ] C5.3 `os/shell/structural_guard.rs`：拦截 `$(...)`、反引号、`&`、`bash -c "..."` 等隐藏执行构造
- [ ] C5.4 `os/shell/env_guard.rs`：`env_clear()` + `SAFE_ENV_VARS` 白名单，父进程 secret 不进入子进程
- [ ] C5.5 `os/shell/classify.rs`：命令风险分级 `High / Medium / Low`，`High` 硬拒绝
- [ ] C5.6 实现 heredoc 剥离：只对引号包裹的 heredoc body 生效，避免 prose 中的危险词误触发
- [ ] C5.7 `os/shell/bare_exec.rs`：`tokio::process::Command` + `kill_on_drop(true)`；Windows 用 Job Object 杀进程树，Unix 用进程组 + `libc::killpg`
- [ ] C5.8 `execute()` 顺序固定为 `SafetyBelt → 结构守卫 → 环境隔离 → 命令分类 → BareExec`，无旁路
- [ ] C5.9 `validate_input` 复用 `safety_belt` 判定；命中时 `blocks_input_rewrite = true`（安全不变量）
- [ ] C5.10 `cwd` 经 `os::path::resolve`；`command` 的基名作为 `permission_intents` 的 `execute` 资源
- [ ] C5.11 stdout / stderr 用 `os/process_output::decode_process_output` 解码，并按 `os/output.rs` 截断
- [ ] C5.12 非零退出码视为正常执行结果（`ok = true` + `exitCode`），不作为 `Business`
- [ ] C5.13 `timeout` 钳制在外层 `exec::invoke` 超时之内（C0.10），且始终由进程级计时触发 kill
- [ ] C5.14 `audience = BOTH` 仅在 C5.2–C5.8 全部落地后声明；任一守卫缺失则降级 `AGENT_ONLY`（C0.12）
- [ ] C5.15 测试：SafetyBelt 拒绝 `rm -rf /`、结构守卫拒绝 `$()` / 反引号、环境隔离（父进程 secret 不可见）、`High` 风险拒绝、超时后子进程被 kill（Windows 断言无孤儿）、`cwd` 越界拒绝、`blocks_input_rewrite`、heredoc 剥离、非零退出码
- [ ] C5.16 测试：非法 / 空命令不 panic；命令含中文 / 非 UTF-8 输出时解码正确

---

## C6 分组注册与桥接兼容

实现：`src-tauri/crates/assistant-tools/src/os/mod.rs`、`src-tauri/src/setup.rs`
测试：`src-tauri/crates/assistant-tools/tests/registration.rs`

- [ ] C6.1 `os/mod.rs` 提供 `register_readonly_tools` / `register_write_tools` / `register_dangerous_tools` 三组函数
- [ ] C6.2 `register_os_tools` 只保留 `fs_link`（`TOOLBOX_ONLY`），不与新工具混在同一函数
- [ ] C6.3 `setup.rs` 调用 `os::register_os_tools` + `os::register_readonly_tools`；不调用修改组 / 危险组（A7）
- [ ] C6.4 单测：只读组注册后包含 `fs_read` / `fs_list` / `fs_glob` / `fs_grep`，且 `fs_link` 仍在
- [ ] C6.5 单测：未调用修改组 / 危险组时，`find_for_toolbox` 查不到 `fs_write` / `fs_edit` / `fs_mkdir` / `fs_move` / `fs_delete` / `shell_exec`
- [ ] C6.6 单测：`tool_definitions_for_tier(ToolTier::Os)` 仅返回 4 个只读工具且按名排序（`fs_link` 被 `audience` 过滤）
- [ ] C6.7 单测：显式调用修改组 / 危险组后，对应工具可被 `find_for_toolbox` 查到（仅测试内使用，不改变生产注册）
- [ ] C6.8 确认不改 `os_tool_exec` 命令与 `OsToolExecRequestDto`；新工具在注入 `action` 后仍可执行（C0.5）

---

## C7 验收命令

`cwd = src-tauri`

- [ ] C7.1 `cargo test -p assistant-tools` 全绿（A9）
- [ ] C7.2 `cargo clippy -p assistant-tools --all-targets -- -D warnings` 无告警（A9）
- [ ] C7.3 先执行格式化修复，再 `cargo fmt --all --check` 通过（A9）
- [ ] C7.4 `cargo test --workspace --no-run` 确认全量编译通过
- [ ] C7.5 grep 确认 `assistant-tools/src/os/` 无第二条路径规范化实现，无 `unwrap_or_else(|_| path.to_path_buf())`（A1）
- [ ] C7.6 grep 确认每个新工具都有显式 `audience()`，无遗漏默认 `NONE`（C0.12）
- [ ] C7.7 确认 `setup.rs` 生产注册表只含 `fs_link` + 只读四工具（A7）
- [ ] C7.8 `npm run contract:check` 无 diff（P5 不改 DTO / 前端契约）
- [ ] C7.9 `git status` 路径核对：无前端组件、无 DB、无 `assistant-tools` 控制层文件改动（C0.3）

---

## C8 文档

- [ ] C8.1 规格 §5.3 / §5.4 的依赖表述按 C0.9 定稿回写（`ignore` + `globset` + `grep-*`）
- [ ] C8.2 规格 §11 OQ1 标注为已定稿（遵守 `.gitignore`）
- [ ] C8.3 更新 I38 roadmap §2：P5 标记已完成并记录日期
- [ ] C8.4 在本清单勾选 C1–C7 完成项（任务级进度只在本清单维护）

---

## 验收映射

| 规格验收 | 对应任务 |
|---|---|
| A1 路径只经 `os::path`，无回退规范化 | C0.4、C2.4、C3.4、C4.4、C5.10、C7.5 |
| A2 每个工具 `input_schema` 单测（合法 / 缺必填 / 类型错误） | C2.4、C2.7、C2.14、C2.18、C2.23、C3.5、C3.8、C3.13、C3.16、C4.1、C5.1 |
| A3 只读工具 intents 为空 / 并发安全；写删工具 action 正确 | C2.2、C2.3、C3.3、C4.3 |
| A4 `fs_edit` 3 分支 + 审批载荷含 diff | C3.8–C3.12 |
| A5 `fs_delete` 拒绝根目录、审批键按规范化路径 | C4.3–C4.6 |
| A6 `shell_exec` 守卫与进程级 kill | C5.2–C5.15 |
| A7 分组注册：默认只注册只读组 | C0.11、C6.1–C6.7、C7.7 |
| A8 `fs_read` 读取回执 | C1.8–C1.9、C2.11–C2.13 |
| A9 测试、clippy、fmt | C7.1–C7.3 |

---

## 不做

- 不实现前端 ToolBox 组件、`os.ts` 命名空间、审批 UI（P6）
- 不实现 `ask_user`（P6 联调）
- 不实现 MCP 工具（P7）
- 不改 `Tool` trait、`ToolPipeline`、`ToolGateway`、Policy 规则集
- 不改 `OsToolExecRequestDto` / `OsToolResultDto` / 任何前端契约
- 不改 DB schema、不新增表
- 不把修改组 / 危险组加入生产注册；不做运行时注册开关
- 不引入 `windows-sys`；不新增原生构建依赖
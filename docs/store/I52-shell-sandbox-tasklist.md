# I52 — 沙箱执行层与 `shell` 工具 任务清单

> 规格：[I52-shell-sandbox-spec.md](./I52-shell-sandbox-spec.md)（阶段：范围 / 术语 / 分批 / 验收归属）、I56-shell-tool-spec.md（已归档）（`shell` 工具规格，2026-11-18 起为工具侧唯一来源）
> 事实基线：I56 §2 的 P1–P8（已就绪）与 G1–G8（C5 / C6 / C7 待补）；沙箱准备（S0）的交付物与判据见 I53 §9（已归档）
> 前置：P2（工具控制层）、P5（OS 工具能力）、P6（前端与审批 UI）、P12（工作区）均已完成；S0 沙箱准备已落地并接线

批次对应规格章节：C1 审批链前置（I56 §6.2 / §6.4）→ C2–C4 沙箱准备（I53 §9）→ C4b 输出存储与回读（I56 §8.1–§8.7）→ C5 工具本体（I56 §3–§8、§12）→ C6 卡片与审计（I56 §6.5 / §10）→ C7 前端与配置（I56 §11）→ C8 端到端（I56 §13）。

依赖：C1 无前置（不依赖沙箱）；C2–C4 依赖 C1；C4b 依赖 C1、可与 C2–C4 并行；C5 依赖 C2–C4 与 C4b；C6 / C7 依赖 C5；C8 依赖 C1–C7。

> 本清单不管理提交。提交粒度按代码依赖与可编译性现场决定；这里只回答「代码改没改、验收跑没跑」。
> 勾选表示代码已改且对应验收已跑。

---

## 阶段 C1 — 审批链前置（I56 §6.2 / §6.4）

不依赖沙箱，先于工具本体落地：两处修正都是全局性缺陷，不是 `shell` 的铺路工作。

- [x] `contract/trait.rs`：`Tool` trait 新增 `fn sandbox_enforcement(&self) -> Option<SandboxEnforcement> { None }`，默认 `None`（非沙箱工具键形状不变）
- [x] `sandbox/mod.rs`：`SandboxEnforcement` 补 `Serialize` / `Deserialize`，`#[serde(rename_all = "lowercase")]` 与 `as_str()` 同形；仅用于审批键 JSON，不进 wire
- [x] `gateway/approval_key.rs`：`ApprovalScope::Command` 增字段 `enforcement: Option<SandboxEnforcement>`，`#[serde(default)]`（旧 `grant_key` JSON 解出 `None`），**不加** `skip_serializing_if`
- [x] `gateway/approval_key.rs`：`from_intent` 增形参 `enforcement`；仅 `"execute" | "run" | "shell"` 分支写入，其余分支不受影响
- [x] `gateway/mod.rs`：`evaluate` 在 intent 循环前取一次 `tool.sandbox_enforcement()`，逐 resource 传给 `from_intent`
- [x] `gateway/mod.rs`：求值顺序改为 `policy.evaluate` 先 → `Deny` 立即返回 `Denied`（写审计）→ `Allow` / `Ask` 再查 `is_granted`（I56 §6.4）
- [x] 保留既有不变部分：命中授权仍记 `GrantHit`；空键仍不查缓存；tier 硬上限仍先于 intent 判定
- [x] `tests/gateway.rs`：5 处 `from_intent` 调用点补 `None`；新增 A7（同键 `full` 授权后置 `partial` → 重新 `Ask`）与 A8（已授权键 + 策略改判 `Deny` → `Denied`）用例
- [x] A6 回归：既有 command 键用例（同命令两次命中 / 命令差一字符重问）不改断言即通过

> 验收记录（2026-11-18）：`cargo test -p assistant-tools` 全绿（gateway 43 项，含新增 A7 / A8 与键 JSON 兼容用例）；`cargo test -p assistant-app --lib` 117 项通过；`cargo check --workspace --all-targets` 通过。
> 顺带收口：`cmd/permission/{grants,audit}.rs` 的 `ApprovalScope::Command` 解构改用 `..`，摘要**不**展示隔离强度（I56 §10 词表约束）。

---

## 阶段 C2–C4 — 沙箱准备（S0）

已由 I53 §9（已归档）交付并验收：`SandboxBackend` 抽象、三平台后端、链式探测、`tool_setup.rs` 的注册期探测与 `shell_registration_allowed` 分支（分支内 `register_shell_tools` 仍是占位）。本清单不重列该批条目，判据见 I53 §9 的 A-S1–A-S6。

---

## 阶段 C4b — 输出存储与回读（I56 §8.1–§8.7）

- [x] `runtime/storage.rs`：`ToolResultStorage` 增 `async fn open_sink(&self, req) -> Result<Box<dyn ToolResultSink>, String>` 与 `fn namespace_root(&self, session_id: Option<&str>) -> Option<PathBuf>`（默认 `None` = 不提供回读）
- [x] `runtime/storage.rs`：新增 `OutputStream` / `ToolResultSink`（同步 `write` / `finish`）/ `SpilledOutput`，字段与 I56 §8.1 一致
- [x] `runtime/storage.rs`：新增常量 `MAX_SPILL_BYTES = 60 MiB`、`STDERR_RING_CHARS = 2 MiB`
- [x] `runtime/storage.rs`：`persist` / `open_sink` 统一走 `namespace_root(...)` 取目录，**禁止**各自 `root.join(session_namespace(..))`（清理 / 落盘 / 回读三个事实源合一）
- [x] `runtime/storage.rs`：sink 的 stdout 路径——原始字节直写文件、增量 UTF-8 解码计数、尾窗口只留内存；`total_chars` / `output_bytes` / `output_lines` 触达上限后继续计数
- [x] `runtime/storage.rs`：sink 的 stderr 路径——2 MiB 环形窗口，超出后覆盖并置 `stderr_truncated`
- [x] `runtime/storage.rs`：`write` **永不**因超限返回 `Err`（超限只计数 / 覆盖，仍须消费管道）
- [x] `runtime/storage.rs`：`finish` 四件收尾——`total_chars == 0` 删空文件且 `reference = None`；stdout 触达上限补 `[truncated: output exceeded 60 MiB; total unknown]`；stderr 非空追加 `--- stderr ---` 段（超窗口时头行带 `(last N of M chars)`）；`reference.bytes` 取文件实际字节
- [x] `runtime/storage.rs`：编码边界——**禁止**逐 chunk `String::from_utf8_lossy` 后拼窗口；尾窗口从首个合法 UTF-8 起始字节解码
- [x] `runtime/storage.rs`：`persisted_reference_from_tail(path, tail, total_chars)`——尾窗口独占预算（头窗口不进预览），标签与省略标记与既有 `persisted_reference` 渲染**逐字节一致**（标签只带 `path`）
- [x] `contract/result.rs`：`ToolResult` 增 `#[serde(skip)] pub persisted: Option<PersistedRef>`；新增 `err_with_data(kind, error, data, text)` 与 `with_persisted(...)`
- [x] `runtime/pipeline.rs`：`ToolOutcome::new` 以 `result.persisted` 为初值（失败结果同样）；`execute` 对 `persisted.is_some()` **跳过** `persist()`
- [x] `runtime/pipeline.rs`：暴露 `storage()`；`PipelineToolExecution::execute` 填 `tool_output_root`
- [x] `contract/context.rs`：`ToolUseContext` 增 `#[serde(skip)] pub tool_output_root: Option<PathBuf>`；`for_toolbox` 补 `None`
- [x] `os/shared/path/`：新增 `ReadableScope` / `ReadablePath` / `resolve_readable`——形态校验 → `reject_system_dirs` → **输出域优先** → 工作区
- [x] `os/tools/agent_read`：`validate_input` / `permission_intents` / `execute` **三处一起**换 `resolve_readable`，回执键仍取 `ReadablePath.path`
- [x] `os/tools/agent_read`：`permission_intents` 按 `scope` 分流——`Workspace` → `read`，`ToolOutput` → `readToolOutput`，`resource` 取**本会话命名空间目录**（不是具体文件）
- [x] `os/tools/agent_read`：`description` 补一句「带 `<persisted-output path="…">` 标签的文件可用本工具读取，首次读取会请求授权」
- [x] `runtime/policy.rs`：`base_rules` 追加 `PermissionRule::new("readToolOutput", "*", PermissionEffect::Ask)`，排在 `("*", "*")` 与 `("read", "*")` 之后
- [x] `gateway/approval_key.rs`：`Path` 分支动作名追加 `"readToolOutput"`（不新增 `ApprovalScope` 变体，`grant_key` 列无迁移）
- [x] `gateway/context.rs`：新增 `output_extensions`（零 IO），仅凭 `pending.resource` 落在 `tool_output_root` 内产出一条 `output.scope` / `Info`；与 `workspace_extensions` 并列
- [x] `os/tools/agent_read`：`preview_for_approval` 在 `ToolOutput` 作用域给出「读本会话输出 `<文件名>` · 第 N–M 行 · `<human_bytes>`」；文件名为末段，不写绝对路径
- [x] 测试 A19 / A20 / A21 / A22 / A23（A26 的 `(no output)` 文案与 `outputPath = null` 依赖 `shell` 本体，随 C5 落地；2026-11-20 已由 `a26_empty_output_leaves_no_file` 覆盖）

> 验收记录（2026-11-18）：`cargo test -p assistant-tools` 全绿（agent_read 41 项、gateway 47 项、pipeline 19 项、storage 30 项，含 A19 落盘→回读→分页、A20 命名空间一次授权覆盖、A21 跨会话拒绝、A22 写方向只读、A23 不二次落盘）；`cargo check --workspace --all-targets` 通过。
> 设计定稿：回填预览改为**尾窗口独占**（`persisted_reference_from_tail`），头窗口不进预览——命令输出信息密度集中在末尾，头部多是启动横幅与进度条重绘。

---

## 阶段 C5 — 工具本体（I56 §3–§8、§12）

模块形态见 I56 §12.1：`os/tools/agent_shell/{mod.rs, schema.rs, classify.rs, spawn.rs, run.rs, deny.rs}`。

> 收口记录（2026-11-20）：C5 全部条目已改并跑完验收。`cargo test -p assistant-tools --test agent_shell` 21 项全绿（A6 / A9–A13（超时 + 取消两条）/ A15 / A24–A38）；`cargo test -p assistant-tools` 全绿（lib 321 项）；`cargo clippy -p assistant-tools --all-targets` 无警告。
>
> 分类器边界修复（同批）：单个 `&` 原先漏判，`ls & del a.txt` 会被整体归 `readOnly`；只读程序带执行类参数（`find -exec/-delete`、`rg --pre[=cmd]`、`git --ext-diff`）原先只按 `argv[0]` 判，同样归 `readOnly`。两处都改为「宁可更危险」，规格 I56 §5.2 硬约束 4 与判定表已同步。

- [x] `os/tools/agent_shell/`：身份（`id` / `name` = `shell`、`tier` = `Os`、`audience` = `AGENT_ONLY`、`behavior_hints` 取最坏值、`is_concurrency_safe` 恒 `false`、`manages_own_execution_timeout` = `true`、`sandbox_enforcement()` 返回缓存探测的 `Some(..)`）
- [x] `os/tools/agent_shell/schema.rs`：`input_schema` 仅 `command` / `cwd` / `timeout_ms`，`additionalProperties: false`；**无** `shell` / `program` / `env` / `stdin`
- [x] `os/tools/agent_shell/schema.rs`：`validate_input` 四步（命令形状 → `cwd` 解析 + `ensure_within_root` → `timeout_ms` 范围 → `workspace_root` 缺失）；**禁止**命令内容匹配
- [x] `os/tools/agent_shell/mod.rs`：`permission_intents` 返回单条 `execute` 意图，`resources = [command]`（不拆多意图）
- [x] `os/tools/agent_shell/classify.rs`：`CommandClass` 纯函数 + 五步判定顺序（控制符 → 只读 → 网络 → 破坏 → 其余 `unknown`），网络先于破坏
- [x] `os/tools/agent_shell/mod.rs`：`preview_for_approval` 返回 `summary`（含 `CommandClass` 与隔离档）+ `estimated = true`、`truncated = false`
- [x] `os/tools/agent_shell/run.rs`：`execute` 十一步顺序固定——取 `writable_root` → 读探测结论（`none` 即 `sandbox_unavailable`）→ 构造 `SandboxPlan`（含临时目录）→ **先 `open_sink`** → `spawn` → **立刻起两个读取任务** → `wait` 收尸与 deadline / cancel 竞争 → 终态「杀 + 收尸」→ 有界 join（宽限 2s）→ `finish` → 组装结果
- [x] `os/tools/agent_shell/spawn.rs`：`SandboxPlan` 的 `args`——Windows `["/d", "/C", command]`（`/d` 关掉 `Command Processor\AutoRun`）、Unix `["-c", command]`；`cwd` 传子进程前剥 `\\?\` 前缀
- [x] `os/tools/agent_shell/spawn.rs`：`program` 有序候选解析（I56 §7.3）——Windows `%SystemRoot%\System32\cmd.exe`（`SystemRoot` 缺失按 `C:\Windows`）→ `%COMSPEC%`（仅 basename 为 `cmd.exe` 时采用）→ 全不存在即判不可用（`shell` 不注册，**不**退化为裸名 `cmd`）；Unix `/bin/sh`；解析来源只进 `data.sandbox.shell` 与日志，**不进**审批键 / 审批卡
- [x] `sandbox/windows/mod.rs` + `sandbox/mod.rs`：`is_cmd_program` 由私有 `fn` 提升为 `pub(crate) fn`，并在 `sandbox/mod.rs` 加 `pub(crate) use windows::is_cmd_program;` 导出供 `agent_shell/spawn.rs` 复用（`mod windows` 保持私有即可；只改可见性，实现与沙箱后端行为零改动）
- [x] `os/tools/agent_shell/run.rs`：调用级临时目录 `<writable_root>/.assistant-tmp/<call_id>`——第 3 步创建并覆盖 `TEMP` / `TMP` / `TMPDIR`；创建失败 → `Business` + `scratch_dir_failed`（未创建子进程）；`finish` 之后由守卫删除（失败只记日志）
- [x] `runtime/storage.rs`：码页转码（I56 §8.3）——**首 4 KiB 探测一次**；Windows 上非法 UTF-8 流按 `GetOEMCP()` 流式解码后以 UTF-8 写盘，Unix 保持原始字节（非法字节计 `U+FFFD`）；`SpilledOutput` 增 `output_encoding` 字段由 `finish` 填写，`run.rs` 只读该字段填 `data.outputEncoding`（**不得**二次探测），并据此回报 `outputEncoding`
- [x] `os/tools/agent_shell/run.rs`：线程模型——`open_sink` / `finish` 由实现内部 `spawn_blocking`，`spawn` / 读取循环 / `wait` 由调用方 `spawn_blocking`；**禁止** `tauri::async_runtime`、**禁止**自建 `std::thread`
- [x] `os/tools/agent_shell/run.rs`：`kill_tree()` 幂等且杀整棵树（Windows Job / Unix `kill(-pgid)`）；**正常结束也扫尾**；`Drop` 守卫兜底收尸与 `finish`
- [x] `os/tools/agent_shell/spawn.rs`：环境变量白名单 + 强制注入（五项常量 `TERM=dumb` / `NO_COLOR=1` / `PAGER=cat` / `GIT_PAGER=cat` / `GIT_TERMINAL_PROMPT=0`，加 `TEMP` / `TMP` / `TMPDIR` 指向调用级临时目录）+ git 加固（`GIT_CONFIG_NOSYSTEM=1`、`GIT_CONFIG_COUNT` / `GIT_CONFIG_KEY_0=core.fsmonitor` / `GIT_CONFIG_VALUE_0=false`）；`TEMP` / `TMP` / `TMPDIR` 与工具链路径变量（`CARGO_HOME` / `RUSTUP_HOME` / `APPDATA` / `LOCALAPPDATA` 等）**不入白名单**；敏感名与 `ASSISTANT_*` 不透传；**不**注入 `CI=1`
- [x] `os/tools/agent_shell/run.rs`：结果 `data` 十项（`exitCode` / `signal` / `outputPath` / `outputBytes` / `outputLines` / `outputTruncated` / `stderrTruncated` / `durationMs` / `outputEncoding` / `sandbox`）；`sandbox` 含 `backend` / `enforcement` / `network` / `fsRead` / `denied` / `shell`；`exitCode` 与 `signal` 二选一，不用 `-1` 哨兵
- [x] `os/tools/agent_shell/deny.rs`：I56 §8.5 拒绝判定纯函数——`enforcement != none` **且** 非零退出码 **且**（`128+SIGSYS` 或关键词命中）；退出码 `2` / `126` / `127` 一律不判拒绝 → `data.sandbox.denied = true` + 末尾固定行，`ok` 仍为 `true`
- [x] `os/tools/agent_shell/run.rs`：回填文本——落盘引用（`persisted_reference_from_tail`）+ 固定标记词汇（`--- stderr ---` / `[sandbox: write outside writable root denied]`（先于退出码行）/ `[exit code: N]` / `[killed by signal: N]` / `[timed out after Nms]` / `[stopped: turn cancelled]` / `(no output)`），**正文在前、标记在末尾**
- [x] `os/tools/agent_shell/schema.rs`：`timeout_ms` 范围 `1000–1800000`、缺省 `120000`（长任务由模型显式请求）
- [x] `os/tools/agent_shell/run.rs`：四种终态一律 `finish()`；超时 / 取消保留 `outputPath` 与已读计数（用 `err_with_data`）
- [x] `os/tools/agent_shell/run.rs`：非零退出码 = `ok = true`，不判失败；输出触达上限**不杀树**
- [x] `runtime/compact/mod.rs`：`subject` 取 `arguments.command`（折空白 + 中间省略）；`scale` 取 `exit <code> · N lines · <size>`，`outputTruncated` 时行数带 `+`；`anchor` 为 `None`；`persisted` 取自 `ToolResult.persisted`
- [x] `os/mod.rs`：导出 `agent_shell` 并新增 `register_shell_tools(registry, probe, storage)`；`tool_setup.rs` 的占位分支改为调用它——**注册点须后移**：现有门禁分支位于 `registry` 与 `storage` 构造之前，必须把调用放到两者都就绪处（`registry` 创建后、`storage` 创建后），否则拿不到两个入参；门禁不满足时一律不调用，**禁止**在分支里注册空工具（I56 §3.2 规则 5）
- [x] 测试 A6 / A9–A13 / A15 / A24 / A25 / A27–A32 / A34–A38（A29 中文码页可读、A30 临时目录落在可写根、A31 拒绝标记与「排除 2 / 126 / 127」单测、A32 AutoRun 不注入、A34 `sandbox` 三布尔、A35 注册门禁、A36 参数不可越权、A37 `program` 候选解析 fail-closed、A38 `sandbox.shell` 只写种类）

---

## 阶段 C6 — 卡片与审计（I56 §6.5 / §10）

- [x] `gateway/context.rs`：`tool.sandbox_enforcement()` 为 `Some(e)` 时追加两条扩展——`sandbox.enforcement`（`Info` / `Caution` 按档）与 `sandbox.root`（可写根末段目录名）；`label` / `value` 用中文（`隔离强度` / `可写根`，与 C4b `agent_read` 预览同语言）；既有 `output.scope` 的英文串不随本批改动
- [x] 审计脱敏：`resource` 落库前做词法打码（`NAME=value` 命中敏感词表 → `<redacted>`；`--password` / `--passwd` / `--token` / `--api-key` 的后一 token 同样打码；**`-p` 不打码**——`docker run -p` / `ssh -p` 是端口），实际执行不变
- [x] 审计不变：沿用 `action = "execute"` 与既有 `AuditOutcome`，不新增 outcome；`backend` / `enforcement` / `network` / `fsRead` / `denied` / `outputEncoding` 只进 `data.sandbox` 与日志固定串
- [x] 测试 A14 / A16

> 收口记录（2026-11-20）：`sandbox_extensions` 直接读 `tool.sandbox_enforcement()`，不二次探测——卡片与执行必须同源；`None` 档整条不产出（不能把「无边界」写成一行 `Info`）。可写根取 `canonicalize_lenient(workspace_root)` 的 `file_name`，卡片不出现绝对路径；无末段时省掉该条而不伪造 `?`。中文档位名收进 `SandboxEnforcement::label_zh()`，与 `shell` 的 `summary_for` 共用，避免两处措辞漂移。脱敏落在 `redact_secrets()`（`stores.rs`），在 `audit()` 里与 `redact_resource` 串行：先打密钥再折路径，顺序反了会让 `--password /abs/path` 的路径先被折叠成 `…/path` 而漏掉密钥判定。
> 验收：`cargo test -p assistant-tools` 全绿（`gateway` 52 项，含新增 A14×2 / A16×2）；`cargo clippy -p assistant-tools --all-targets` 无警告。

---

## 阶段 C7 — 前端与配置（I56 §11）

- [x] `src/i18n/locales/`：新增 `tool_desc.shell`（zh / en）面板一句话说明；模型侧 6 项契约由 `description()` 承载（I56 §11.2）
- [x] 模式预置 `toolConfig`：`shell` 默认**不勾选**（破坏性工具需显式启用）；`shell` 是 `Os` 层级，内置种子 `max_tool_tier = Sandbox` 下勾选也不可见（`tool_resolver` 按 `tier <= max_tier` 过滤）——本批**不改**种子默认层级，只在 A18 用例里显式把上限设为 `os`
- [x] 确认零改动项：工具装配面板、审批卡、落盘展示、不可用提示均无新增分支
- [x] 测试 A33

> 收口记录（2026-11-20）：i18n 落地时发现 §11.2 原表述把「模型侧契约」与「面板短句」写成同一份 key，两者断言对象不同，已改规格（`description()` 单语英文承载 6 项，`tool_desc.shell` 只写一句话），两侧各有一条 A33。默认不勾选无需改代码：`ModeToolConfig::default()` 与 `BUILTIN_AGENT_TOOL_CONFIG` 的 `enabled_tools` 都是空 `Vec`，且种子 `max_tool_tier = Sandbox` 让 `Os` 层整组置灰。`ToolConfigPanel` 只加了一行 `TOOL_DESC_KEYS` 映射（否则悬停回落成十几行模型提示词），不是新分支。
> 验收：`cargo test -p assistant-tools --test agent_shell a33` 通过；`npx vitest run src/chat/tool/__tests__/panelStructure.test.ts` 13 项通过。

---

## 阶段 C8 — 端到端（I56 §13）

- [x] 集成测试：真实命令跑通（含 `git log` 分页、`sh -c 'sleep 300 &'` 扫尾、`cmd /C exit 0` 空输出）
- [x] 越界写失败（沙箱内重定向到输出域）与无沙箱不执行（探测 `none` 不注册）
- [x] 审批三态回归（`Ask` / `GrantHit` / `Denied`）覆盖新增 `readToolOutput` 动作
- [x] `cargo clippy -D warnings` / `cargo fmt --check` / `npm run typecheck` / `npm test`（`vitest run`）全绿

> 收口记录（2026-11-20）：三条集成断言各自落在已有验收上，只补了真正缺的一条——`c8_git_log_completes_and_lands_in_the_output_file`（`git init` + 三次空提交后 `git log --oneline`，断言 10s 内返回且三条提交都落进输出文件；机器无 `git` 时跳过探针而不是判失败）。后台进程扫尾复用 A28（`start /b ping` / `sleep 30 &`），空输出复用 A26（`cmd /C exit 0` 不留文件），越界写复用 A31（重定向到输出域被拒且 `ok = true`），无沙箱不注册复用 A35（`SandboxProbe::unavailable` 既不注册也构造不出实例）。审批三态里 `Ask` / `GrantHit` 已有，补 `read_tool_output_denied_by_the_user_is_not_granted`：审计记 `Rejected`（不是策略层的 `Denied`），且不落授权键。
> 验收（2026-11-20）：`cargo fmt --check` 与 `cargo clippy --workspace --all-targets -- -D warnings` 均 0 退出；`npm run typecheck` 0；`npm test`（`vitest run`）73 个文件 / 865 通过 / 1 跳过。`cargo test -p assistant-tools` 全绿（lib 321 项；`--test agent_shell` 23 项 / 6.1s，含 C8 新增用例，A31 在本机通过即证明 Windows 沙箱边界真实生效）；A18 落在 `cargo test -p assistant-app --test tool_resolver` 19 项，`assistant-app --lib` 154 项亦通过。
# I52 — 沙箱执行层与 `shell` 工具 任务清单

> 规格：[I52-shell-sandbox-spec.md](./I52-shell-sandbox-spec.md)
> 事实基线：规格 §1.3 的 N1–N9（编写规格时对 `assistant-tools` 与 `tool_setup.rs` 的实读结果）；沙箱准备（S0）的交付物与判据见 I53 §9（已归档）
> 前置：P2（工具控制层）、P5（OS 工具能力）、P6（前端与审批 UI）、P12（工作区）均已完成；S0 沙箱准备已落地并接线

批次对应规格章节：C1 审批链前置（§6.2 / §6.4）→ C2–C4 沙箱准备（I53 §9）→ C4b 输出存储与回读（§5.11.1–§5.11.4）→ C5 工具本体（§5.1–§5.12）→ C6 卡片与审计（§6.3 / §8）→ C7 前端与配置（§9）→ C8 端到端（§10）。

依赖：C1 无前置（不依赖沙箱）；C2–C4 依赖 C1；C4b 依赖 C1、可与 C2–C4 并行；C5 依赖 C2–C4 与 C4b；C6 / C7 依赖 C5；C8 依赖 C1–C7。

> 本清单不管理提交。提交粒度按代码依赖与可编译性现场决定；这里只回答「代码改没改、验收跑没跑」。
> 勾选表示代码已改且对应验收已跑。

---

## 阶段 C1 — 审批链前置（§6.2 / §6.4）

不依赖沙箱，先于工具本体落地：两处修正都是全局性缺陷，不是 `shell` 的铺路工作。

- [x] `contract/trait.rs`：`Tool` trait 新增 `fn sandbox_enforcement(&self) -> Option<SandboxEnforcement> { None }`，默认 `None`（非沙箱工具键形状不变）
- [x] `sandbox/mod.rs`：`SandboxEnforcement` 补 `Serialize` / `Deserialize`，`#[serde(rename_all = "lowercase")]` 与 `as_str()` 同形；仅用于审批键 JSON，不进 wire
- [x] `gateway/approval_key.rs`：`ApprovalScope::Command` 增字段 `enforcement: Option<SandboxEnforcement>`，`#[serde(default)]`（旧 `grant_key` JSON 解出 `None`），**不加** `skip_serializing_if`
- [x] `gateway/approval_key.rs`：`from_intent` 增形参 `enforcement`；仅 `"execute" | "run" | "shell"` 分支写入，其余分支不受影响
- [x] `gateway/mod.rs`：`evaluate` 在 intent 循环前取一次 `tool.sandbox_enforcement()`，逐 resource 传给 `from_intent`
- [x] `gateway/mod.rs`：求值顺序改为 `policy.evaluate` 先 → `Deny` 立即返回 `Denied`（写审计）→ `Allow` / `Ask` 再查 `is_granted`（§6.4）
- [x] 保留既有不变部分：命中授权仍记 `GrantHit`；空键仍不查缓存；tier 硬上限仍先于 intent 判定
- [x] `tests/gateway.rs`：5 处 `from_intent` 调用点补 `None`；新增 A7（同键 `full` 授权后置 `partial` → 重新 `Ask`）与 A8（已授权键 + 策略改判 `Deny` → `Denied`）用例
- [x] A6 回归：既有 command 键用例（同命令两次命中 / 命令差一字符重问）不改断言即通过

> 验收记录（2026-11-18）：`cargo test -p assistant-tools` 全绿（gateway 43 项，含新增 A7 / A8 与键 JSON 兼容用例）；`cargo test -p assistant-app --lib` 117 项通过；`cargo check --workspace --all-targets` 通过。
> 顺带收口：`cmd/permission/{grants,audit}.rs` 的 `ApprovalScope::Command` 解构改用 `..`，摘要**不**展示隔离强度（§8 词表约束）。

---

## 阶段 C2–C4 — 沙箱准备（S0）

已由 I53 §9（已归档）交付并验收：`SandboxBackend` 抽象、三平台后端、链式探测、`tool_setup.rs` 的注册期探测与 `shell_registration_allowed` 分支（分支内 `register_shell_tools` 仍是占位）。本清单不重列该批条目，判据见 I53 §9 的 A-S1–A-S6。

---

## 阶段 C4b — 输出存储与回读（§5.11）

- [x] `runtime/storage.rs`：`ToolResultStorage` 增 `async fn open_sink(&self, req) -> Result<Box<dyn ToolResultSink>, String>` 与 `fn namespace_root(&self, session_id: Option<&str>) -> Option<PathBuf>`（默认 `None` = 不提供回读）
- [x] `runtime/storage.rs`：新增 `OutputStream` / `ToolResultSink`（同步 `write` / `finish`）/ `SpilledOutput`，字段与规格 §5.11.1 一致
- [x] `runtime/storage.rs`：新增常量 `MAX_SPILL_BYTES = 60 MiB`、`STDERR_RING_CHARS = 2 MiB`
- [x] `runtime/storage.rs`：`persist` / `open_sink` 统一走 `namespace_root(...)` 取目录，**禁止**各自 `root.join(session_namespace(..))`（清理 / 落盘 / 回读三个事实源合一）
- [x] `runtime/storage.rs`：sink 的 stdout 路径——原始字节直写文件、增量 UTF-8 解码计数、尾窗口只留内存；`total_chars` / `output_bytes` / `output_lines` 触达上限后继续计数
- [x] `runtime/storage.rs`：sink 的 stderr 路径——2 MiB 环形窗口，超出后覆盖并置 `stderr_truncated`
- [x] `runtime/storage.rs`：`write` **永不**因超限返回 `Err`（超限只计数 / 覆盖，仍须消费管道）
- [x] `runtime/storage.rs`：`finish` 四件收尾——`total_chars == 0` 删空文件且 `reference = None`；stdout 触达上限补 `[truncated: output exceeded 60 MiB; total unknown]`；stderr 非空追加 `--- stderr ---` 段（超窗口时头行带 `(last N of M chars)`）；`reference.bytes` 取文件实际字节
- [x] `runtime/storage.rs`：编码边界——**禁止**逐 chunk `String::from_utf8_lossy` 后拼窗口；尾窗口从首个合法 UTF-8 起始字节解码
- [x] `runtime/storage.rs`：`persisted_reference_from_tail(path, tail, total_chars)`——尾窗口独占预算（头窗口不进预览），标签与省略标记与既有 `persisted_reference` 渲染**逐字节一致**（标签只带 `path`）
- [x] `result.rs`：`ToolResult` 增 `#[serde(skip)] pub persisted: Option<PersistedRef>`；新增 `err_with_data(kind, error, data, text)` 与 `with_persisted(...)`
- [x] `pipeline.rs`：`ToolOutcome::new` 以 `result.persisted` 为初值（失败结果同样）；`execute` 对 `persisted.is_some()` **跳过** `persist()`
- [x] `pipeline.rs`：暴露 `storage()`；`PipelineToolExecution::execute` 填 `tool_output_root`
- [x] `contract/context.rs`：`ToolUseContext` 增 `#[serde(skip)] pub tool_output_root: Option<PathBuf>`；`for_toolbox` 补 `None`
- [x] `os/path.rs`：新增 `ReadableScope` / `ReadablePath` / `resolve_readable`——形态校验 → `reject_system_dirs` → **输出域优先** → 工作区
- [x] `os/agent_read.rs`：`validate_input` / `permission_intents` / `execute` **三处一起**换 `resolve_readable`，回执键仍取 `ReadablePath.path`
- [x] `os/agent_read.rs`：`permission_intents` 按 `scope` 分流——`Workspace` → `read`，`ToolOutput` → `readToolOutput`，`resource` 取**本会话命名空间目录**（不是具体文件）
- [x] `os/agent_read.rs`：`description` 补一句「带 `<persisted-output path="…">` 标签的文件可用本工具读取，首次读取会请求授权」
- [x] `runtime/policy.rs`：`base_rules` 追加 `PermissionRule::new("readToolOutput", "*", PermissionEffect::Ask)`，排在 `("*", "*")` 与 `("read", "*")` 之后
- [x] `gateway/approval_key.rs`：`Path` 分支动作名追加 `"readToolOutput"`（不新增 `ApprovalScope` 变体，`grant_key` 列无迁移）
- [x] `gateway/context.rs`：新增 `output_extensions`（零 IO），仅凭 `pending.resource` 落在 `tool_output_root` 内产出一条 `output.scope` / `Info`；与 `workspace_extensions` 并列
- [x] `os/agent_read.rs`：`preview_for_approval` 在 `ToolOutput` 作用域给出「读本会话输出 `<文件名>` · 第 N–M 行 · `<human_bytes>`」；文件名为末段，不写绝对路径
- [x] 测试 A19 / A20 / A21 / A22 / A23（A26 的 `(no output)` 文案与 `outputPath = null` 依赖 `shell` 本体，随 C5 落地）

> 验收记录（2026-11-18）：`cargo test -p assistant-tools` 全绿（agent_read 41 项、gateway 47 项、pipeline 19 项、storage 30 项，含 A19 落盘→回读→分页、A20 命名空间一次授权覆盖、A21 跨会话拒绝、A22 写方向只读、A23 不二次落盘）；`cargo check --workspace --all-targets` 通过。
> 设计定稿：回填预览改为**尾窗口独占**（`persisted_reference_from_tail`），头窗口不进预览——命令输出信息密度集中在末尾，头部多是启动横幅与进度条重绘。

---

## 阶段 C5 — 工具本体（§5.1–§5.12）

- [ ] `os/agent_shell.rs`：身份（`id` / `name` = `shell`、`tier` = `Os`、`audience` = `AGENT_ONLY`、`behavior_hints` 取最坏值、`is_concurrency_safe` 恒 `false`、`manages_own_execution_timeout` = `true`、`sandbox_enforcement()` 返回缓存探测的 `Some(..)`）
- [ ] `os/agent_shell.rs`：`input_schema` 仅 `command` / `cwd` / `timeout_ms`，`additionalProperties: false`；**无** `shell` / `program` / `env` / `stdin`
- [ ] `os/agent_shell.rs`：`validate_input` 四步（命令形状 → `cwd` 解析 + `ensure_within_root` → `timeout_ms` 范围 → `workspace_root` 缺失）；**禁止**命令内容匹配
- [ ] `os/agent_shell.rs`：`permission_intents` 返回单条 `execute` 意图，`resources = [command]`（不拆多意图）
- [ ] `os/agent_shell.rs`：`CommandClass` 纯函数 + 五步判定顺序（控制符 → 只读 → 网络 → 破坏 → 其余 `unknown`），网络先于破坏
- [ ] `os/agent_shell.rs`：`preview_for_approval` 返回 `summary`（含 `CommandClass` 与隔离档）+ `estimated = true`、`truncated = false`
- [ ] `os/agent_shell.rs`：`execute` 十一步顺序固定——取 `writable_root` → 读探测结论（`none` 即 `sandbox_unavailable`）→ 构造 `SandboxPlan` → **先 `open_sink`** → `spawn` → **立刻起两个读取任务** → `wait` 收尸与 deadline / cancel 竞争 → 终态「杀 + 收尸」→ 有界 join（宽限 2s）→ `finish` → 组装结果
- [ ] `os/agent_shell.rs`：线程模型——`open_sink` / `finish` 由实现内部 `spawn_blocking`，`spawn` / 读取循环 / `wait` 由调用方 `spawn_blocking`；**禁止** `tauri::async_runtime`、**禁止**自建 `std::thread`
- [ ] `os/agent_shell.rs`：`kill_tree()` 幂等且杀整棵树（Windows Job / Unix `kill(-pgid)`）；**正常结束也扫尾**；`Drop` 守卫兜底收尸与 `finish`
- [ ] `os/agent_shell.rs`：环境变量白名单 + 五项强制注入（`TERM=dumb` / `NO_COLOR=1` / `PAGER=cat` / `GIT_PAGER=cat` / `GIT_TERMINAL_PROMPT=0`）；敏感名与 `ASSISTANT_*` 不透传；**不**注入 `CI=1`
- [ ] `os/agent_shell.rs`：结果 `data` 八项（`exitCode` / `signal` / `outputPath` / `outputBytes` / `outputLines` / `outputTruncated` / `stderrTruncated` / `durationMs` / `sandbox`）；`exitCode` 与 `signal` 二选一，不用 `-1` 哨兵
- [ ] `os/agent_shell.rs`：回填文本——落盘引用（`persisted_reference_from_windows`）+ 固定标记词汇（`--- stderr ---` / `[exit code: N]` / `[killed by signal: N]` / `[timed out after Nms]` / `[stopped: turn cancelled]` / `(no output)`），**正文在前、标记在末尾**
- [ ] `os/agent_shell.rs`：四种终态一律 `finish()`；超时 / 取消保留 `outputPath` 与已读计数（用 `err_with_data`）
- [ ] `os/agent_shell.rs`：非零退出码 = `ok = true`，不判失败；输出触达上限**不杀树**
- [ ] `runtime/compact.rs`：`subject` 取 `arguments.command`（折空白 + 中间省略）；`scale` 取 `exit <code> · N lines · <size>`，`outputTruncated` 时行数带 `+`；`anchor` 为 `None`；`persisted` 取自 `ToolResult.persisted`
- [ ] `os/mod.rs`：导出 `agent_shell`；`tool_setup.rs` 的 `register_shell_tools` 落地并替换占位分支
- [ ] 测试 A6 / A9–A13 / A15 / A24 / A25 / A27 / A28

---

## 阶段 C6 — 卡片与审计（§6.3 / §8）

- [ ] `gateway/context.rs`：`tool.sandbox_enforcement()` 为 `Some(e)` 时追加两条扩展——`sandbox.enforcement`（`Info` / `Caution` 按档）与 `sandbox.root`（可写根末段目录名）
- [ ] 审计脱敏：`resource` 落库前做词法打码（`NAME=value` 命中敏感词表 → `<redacted>`；`--password` / `-p` 的后一 token 同样打码），实际执行不变
- [ ] 审计不变：沿用 `action = "execute"` 与既有 `AuditOutcome`，不新增 outcome；`enforcement` / `backend` 只进 `data.sandbox` 与卡片扩展
- [ ] 测试 A14 / A16

---

## 阶段 C7 — 前端与配置（§9）

- [ ] `src/i18n/locales/`：新增 `tool_desc.shell`（zh / en）
- [ ] 模式预置 `toolConfig`：`shell` 默认**不勾选**（破坏性工具需显式启用）
- [ ] 确认零改动项：工具装配面板、审批卡、落盘展示、不可用提示均无新增分支

---

## 阶段 C8 — 端到端（§10）

- [ ] 集成测试：真实命令跑通（含 `git log` 分页、`sh -c 'sleep 300 &'` 扫尾、`cmd /C exit 0` 空输出）
- [ ] 越界写失败（沙箱内重定向到输出域）与无沙箱不执行（探测 `none` 不注册）
- [ ] 审批三态回归（`Ask` / `GrantHit` / `Denied`）覆盖新增 `readToolOutput` 动作
- [ ] `cargo clippy -D warnings` / `cargo fmt --check` / `npm run typecheck` / `npm run vitest` 全绿
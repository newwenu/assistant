# I38e — MCP 外部工具 任务清单（P7）

> 状态：✅ 已完成（2026-11-14；C1–C6 全部完成，A1–A13 均有测试映射；范围外登记见文末）
> 规格：[I38e-mcp-tools-spec.md](./I38e-mcp-tools-spec.md)（2026-11-09 定稿，无未决项）
> 阶段登记：[I38 §2](../../store/I38-agent-roadmap.md) P7
> 前置：P2 工具控制层、P5 OS 工具能力（均已完成）
> 本清单不管理提交；只回答「代码改没改、验收跑没跑」。

六个批次按依赖排序：C1 桥接层（纯 Rust）→ C2 stdio 客户端 → C3 `McpServerManager` → C4 配置与持久化 → C5 端到端 → C6 最小配置页。C1 不引入 tauri、子进程或协议库，是唯一可独立验证的批次；C2 才引入进程与 JSON-RPC；C3 才引入注册表生命周期。

**门禁速查**（对应规格 §8）：A1 握手与崩溃收敛 / A2 命名稳定 / A3 标记只升级 / A4 tier + 受众 + 白名单三重门 / A5 禁用即不可调用 / A6 超时与取消 / A7 大结果复用 P2 预算 / A8 `assistant-tools` 无 tauri 依赖 / A9 `enabled` 持久化 / A10 配置加密 / A11 精确审批键 / A12 配置变更下一 turn 生效 / A13 崩溃隔离（宿主侧 panic）。

---

## 批次 C1 — 桥接层（纯 Rust，已完成 2026-11-14）

新增 `src-tauri/crates/assistant-tools/src/mcp/`（`mod.rs` + `bridge.rs`）。本批只做 Tool trait 适配、命名、标记推断、结果映射与传输抽象；不做 IO、不碰 `ToolRegistry`。

### C1.1 命名与规范化

对应规格 §3.1 / 门禁 A2。

- [x] `McpToolName { full, server_id, tool, original, display }`：`full = mcp__{server_id}__{tool}`
- [x] 段规范化：非 `[a-zA-Z0-9_-]` 字符替换为 `_`；空段回退 `_`
- [x] 同一 server 内规范化后重名：追加 `sha256(原始 tool 名)` 前 8 位 hex（`_` 分隔）
- [x] `display = {server 展示名} / {原始 tool 名}`；展示名不参与 `full`（展示名可改，工具名不变）
- [x] `build_tool_names(server_id, display_name, defs)` 保持输入顺序，重名判定只看同一次 `tools/list` 快照
- [x] 单测：三段式、展示名变更不改 `full`、非法字符替换、重名短哈希、顺序稳定

### C1.2 Tool trait 映射

对应规格 §3.2 / 门禁 A4。

- [x] `McpToolDef`：`name` / `description` / `inputSchema` / `annotations`（camelCase 反序列化）
- [x] `McpTool` 持有 `McpToolName`、`McpToolDef`、`config_hash`、`Arc<dyn McpCaller>`
- [x] `id()` / `name()` = 规范化全名；`tier()` = `External`；`audience()` = `AGENT_ONLY`（ToolBox 不可达）
- [x] `description()`：MCP `description`，缺省空串
- [x] `input_schema()`：原样透传；缺省 `{"type":"object"}`
- [x] `permission_intents()`：`("mcp", "mcp:{server_id}:{config_hash}")`，两段均不含 `:`
- [x] 单测：tier / 受众 / 空描述 / schema 透传 / intent 资源形状

### C1.3 安全标记推断

对应规格 §3.3 / 门禁 A3。

- [x] `McpAnnotations { read_only_hint, destructive_hint, open_world_hint }`，全部 `Option<bool>`
- [x] `read_only = read_only_hint.unwrap_or(false)`
- [x] `destructive = destructive_hint.unwrap_or(false) || !read_only`（自报只能升级）
- [x] `open_world` 同 `destructive` 规则
- [x] 无 annotations → `destructive = true`、`open_world = true`
- [x] 只读声明只影响 hints，不改变 tier、不自动 `Allow`（Policy 仍评估）
- [x] 单测：无声明 / 只读声明 / 显式 destructive 升级 / `false` 不能降级

### C1.4 结果映射

对应规格 §3.4 / 门禁 A6、A7。

- [x] `McpCaller`（async trait，`call_tool(tool, arguments) -> McpCallOutcome`）
- [x] `McpCallOutcome { Content { blocks, is_error }, ToolNotFound, ServerUnavailable, Internal { message } }`
- [x] 文本块拼接为 `result_for_assistant`（`\n` 连接）；`data` 保留原样内容块数组
- [x] `isError = true` → `ok = false`、`error_kind = Business`
- [x] 工具不存在 → `Business`；server 断开 → `Internal`；客户端内部错误 → `Internal`
- [x] 图片 / 资源块只进 `data`，不进 `result_for_assistant`（避免 base64 进模型文本通道）
- [x] 无文本块时给出计数摘要，禁止回退到 `data` 序列化（否则会把图片负载写进模型通道）
- [x] 不做截断：大结果交 P2 预算（`execute` 内不调用任何截断函数）
- [x] `execute` 把**原始** MCP 工具名（非规范化名）发给 server
- [x] 单测：文本拼接 / 业务失败 / 工具不存在 / server 断开 / 图片不进文本 / 非对象输入拒绝 / 原始名透传

### C1.5 精确审批键

对应规格 §3.5 / 门禁 A11。

- [x] `ApprovalScope::Mcp { server_id, tool, args_fingerprint, config_hash }`
- [x] `ApprovalKey::from_intent` 新增 `"mcp"` 分支：解析 `mcp:{server_id}:{config_hash}`（恰好两段、均非空、均不含 `:`）
- [x] `args_fingerprint = sha256(canonical_json(input))`：对象键递归排序、无多余空白、数组顺序保留
- [x] `scope.tool` 从全名反解第二段（`mcp__{server_id}__` 前缀）；反解失败返回 `None`（空键 → 直接审批）
- [x] 非对象 `input` → `None`（不得退化成「同 server 全放行」）
- [x] `src/cmd/permission.rs` `scope_summary` 增加 `Mcp` 分支（前端仍不解析 `grantKey`）
- [x] 单测：四字段全等 / 参数变化不命中 / 对象键序不变 / 数组顺序变化命中不同 / `config_hash` 变化失效 / 多冒号资源为空键 / 前缀不符为空键

### C1.6 出口

对应规格 §3 / 门禁 A8。

- [x] `lib.rs` 注册 `pub mod mcp;`
- [x] `assistant-tools/Cargo.toml` 增加 `sha2` / `hex`（workspace 依赖，纯 Rust）
- [x] `cargo test -p assistant-tools` 通过；`cargo clippy -p assistant-tools --all-targets -- -D warnings` 与 `cargo fmt --check` 通过

**验收记录（2026-11-14）**：`cargo test -p assistant-tools` 全绿（lib 167 项 + `gateway` 38 项，含 C1 新增 20 项）；`cargo clippy --all-targets -p assistant-tools` 无告警；`cargo fmt` 已应用；`cargo check -p assistant-app` 通过（`scope_summary` 新分支编译）。

---

## 批次 C2 — stdio 客户端（已完成 2026-11-14）

新增 `src-tauri/crates/assistant-tools/src/mcp/stdio.rs`，实现 `McpCaller`；本批开始接触子进程，仍不碰 `ToolRegistry`。

### C2.1 协议

对应规格 §4.2。

- [x] JSON-RPC 2.0 行分隔读写（stdin / stdout）；stderr 单独收敛日志（不写密钥、不写整帧）
- [x] `initialize`：`protocolVersion` / `capabilities`（只声明 tools）/ `clientInfo`；握手 30s
- [x] `notifications/initialized`
- [x] `tools/list` 按 `cursor` 分页收集完整快照
- [x] `tools/call`：`{ name, arguments }` → 内容块 / `isError` → `McpCallOutcome`
- [x] 协议错误与 `-32xxx` 错误码 → `Internal`；`unknown tool` → `ToolNotFound`
- [x] 单测：假 server（tokio duplex 或测试子进程）握手 / 分页 / 错误响应 / 非法帧

### C2.2 超时与取消

对应规格 §4.3 / 门禁 A6。

- [x] 单次调用默认 30s，可被 server 配置覆盖（`with_call_timeout`；配置接线在 C4）
- [x] `ToolUseContext` 新增 `cancel` 字段（执行侧从 `ToolExecCtx.child_token` 填充）
- [x] `select! { biased; token.cancelled() => Cancelled, response => ... }`
- [x] 取消时尽力发 `notifications/cancelled`（携带 `requestId`），**不**杀进程、不注销工具
- [x] 单测：超时 → `Timeout`；取消 → `Cancelled` 且后续调用仍可用

### C2.3 崩溃隔离

对应规格 §4.4 / 门禁 A13。**本批只做客户端侧的防线**；turn 生命周期缺口见「范围外登记」。

- [x] `mcp/` 内零 panic：无 `unwrap` / `expect` / 直接索引 / 无检查切片（含测试外的全部路径）
- [x] 非法帧（坏 JSON / 缺 `jsonrpc` / 缺 `id` / 非 UTF-8 / 超长行）只跳过该帧，读取循环继续
- [x] 读取任务结束（EOF / 出错 / panic）用 drop guard 把所有在途 `pending` 收敛为 `ServerUnavailable`，调用方不永久等待
- [x] `call_tool` 外层 `catch_unwind(AssertUnwindSafe(...))` → panic 归一化为 `Internal`（仅 unwind 构建有效，注释说明 release 为 `panic = "abort"`）
- [x] 日志不写整帧、不写 `env` / `args` 中的密钥
- [x] 单测：坏帧后续帧仍可读 / EOF 时在途请求收敛 / 注入 panic 的 caller 返回 `Internal` 而非中断测试任务

**验收记录（2026-11-14）**：`cargo test -p assistant-tools` 全绿（lib 184 项，含 `mcp::stdio` 15 项 / `mcp::bridge` panic 收敛 1 项）；`cargo clippy -p assistant-tools --all-targets -- -D warnings` 通过（仅剩增量编译目录 `os error 5` 的环境告警）；`cargo check -p assistant-app` 通过（`tool_exec.rs` 的 `cancel` 接线编译）。

---

## 批次 C3 — `McpServerManager`

新增 `src-tauri/crates/assistant-tools/src/mcp/manager.rs`；`ToolRegistry` 补 `unregister(id)`（按 id 精确清理 id 与 name 两张索引）。

### C3.1 状态机与启停

对应规格 §4.1 / 门禁 A1、A5。

- [x] `Stopped → Starting → Ready → Error → Stopped` 状态机，状态变更只在单点写入
- [x] `Starting` 失败 → `Error`（记录原因，工具保留且调用返回 `Internal`）
- [x] 启用：握手 + `tools/list` 成功后才注册工具（`Ready` 后单点注册）
- [x] 禁用：**先注销工具再杀进程**，无「已注销但仍在跑」窗口
- [x] 单测：状态迁移表、注销先于 kill、失败不阻塞其它 server

### C3.2 重连

对应规格 §4.2 / 门禁 A1、A11。

- [x] 指数退避 1s / 2s / 4s / 8s，上限 60s，最多 5 次；超限停在 `Error` 等手动重启
- [x] 重连成功重新 `tools/list`；`server_id` / `tool` / `config_hash` 不变的工具名与授权键不变
- [x] 配置变更：停旧 → 注销 → 新配置重启；当前 turn 工具清单不变，下一 turn 生效（A12；turn 快照由宿主 turn 生命周期保证，端到端在 C5 验证）
- [x] 单测：退避序列、重连后工具名不变、配置变更触发重新注册

**验收记录（2026-11-14）**：`cargo test -p assistant-tools --lib` 全绿（194 项，含 `mcp::manager` 9 项 / `registry::unregister_clears_both_indexes`）；`cargo clippy -p assistant-tools --all-targets -- -D warnings` 通过（仅剩增量编译目录 `os error 5` 的环境告警）；`cargo fmt` 已应用。

设计要点：工具在注册时拿到的是 `ServerCaller` 间接层而非具体连接，连接替换只改间接层，因此 `Ready → Error → Ready` 反复不会改动已注册工具名；`Error` 态工具保留、调用经间接层返回 `ServerUnavailable` → 桥接层归一化为 `Internal`。

---

## 批次 C4 — 配置与持久化

### C4.1 表与加密

对应规格 §6.1 / §6.2 / 门禁 A9、A10。

- [x] `mcp_servers` 迁移：`id` 主键（不可变）、`name`、`transport`、`config_encrypted`、`config_hash`、`enabled`、时间戳
- [x] 默认 `enabled = 0`；`Ready` / `Error` / 重试次数 / `last_error` 不落库
- [x] 宿主层加解密复用 `assistant-crypto` machine-id AES-GCM；`assistant-tools` 不感知明文
- [x] `config_hash` 用**明文规范 JSON**（键排序、无空白）SHA-256；不存可逆信息
- [x] 解密失败 / 配置非法：不启动、状态 `Error`、日志不含密钥

### C4.2 命令与启动序列

对应规格 §4.2 / §5 / §6.3 / 门禁 A5、A12。

- [x] `src/cmd/mcp.rs`：列表 / 增 / 改（`name` 可改、`id` 只读）/ 删 / 启停
- [x] 启停命令写 `enabled` 并立即执行启停，跨重启保留
- [x] `tool_setup.rs`：DB 初始化后拉起 `enabled = 1`；失败只记日志、不阻塞启动
- [x] 禁用 server：Policy 对 `mcp` + `server_id` 记 `Deny`，工具从 Registry 注销
- [x] DTO 与 `npm run contract:gen`（生成 `src/types/protocol.contract.ts` 并提交）
- [x] 单测：CRUD、`id` 不可变、`enabled` 跨重启、密文不含明文

---

## 批次 C5 — 端到端（已完成 2026-11-14）

对应规格 §8 全部门禁。

夹具：`src-tauri/tests/fixtures/dev_mcp_server.rs`（纯标准库 stdio server，由 `[[bin]] dev_mcp_server` 编译成真实进程；7 个工具，一条工具对应一条验收项；`--work-dir` 限定写入目录，路径穿越一律拒绝）。用例：`src-tauri/tests/mcp_e2e.rs`（11 项，真实子进程 + `McpServerManager` + Registry + 审批网关 + P2 Pipeline）。路径取自 `CARGO_BIN_EXE_dev_mcp_server`，不依赖解释器、不猜 `target` 布局，缺产物即编译失败——不存在「环境缺失 → 静默跳过」的假通过路径。

- [x] mock stdio server：只读 `dev_echo` / 破坏性 `dev_append` / 失败 `dev_fail`，另加 `dev_sleep`（超时与取消）、`dev_flood`（大结果）、`dev_crash`（猝死）、`dev_bad_frame`（坏帧）
- [x] 只读通路：`find → validate → gateway → execute → persist` 全链路（`a_read_only_tool_round_trips_through_the_pipeline`）
- [x] 破坏性工具审批：`Ask` → `AllowSession` 命中；参数变化重新询问；`config_hash` 变化旧授权失效（`session_grants_are_bound_to_args_and_config_hash`）
- [x] `maxToolTier != external` 或白名单未命中 → 工具不进入 LLM `tools` 列表（`external_tools_are_gated_by_tier_whitelist_and_audience`）
- [x] ToolBox 入口不可达（`AGENT_ONLY`，`find_for_toolbox` 恒为空）
- [x] 三条错误通道：超时 `Timeout` / 取消 `Cancelled`（进程存活，后续调用成功）/ 崩溃 `Internal`（连接归一化为 `ServerUnavailable`）
- [x] 业务失败与未知工具：server `isError` → `Business`；`-32601` → `ToolNotFound`（直连 `McpCaller` 验证）
- [x] 大结果按 P2 预算落盘，MCP 不单独截断（60000 字符完整落盘并回填引用）
- [x] 坏帧只丢自己，连接继续工作
- [x] 禁用即不可调用：`disable` 后工具从 Registry 注销、进程回收
- [x] server 配置变更下一 turn 生效，当前 turn 工具清单不变：工具名不变、按新 `config_hash` 重新注册、旧授权失效；工具清单在每轮发送前由 `resolve_available_tools` 现算（`cmd/agent/mod.rs`），「当前 turn 不变」由 turn 边界保证

**验收记录（2026-11-14）**：`cargo test --test mcp_e2e` 11 项全绿（0.30s）；`cargo test`（`src-tauri`，`assistant-app` 包）全绿——lib 74 项 + 10 个集成测试文件共 84 项（`mcp_e2e` 11 项由原 lib 内联模块迁出，总数不变）；`cargo clippy --lib --tests --bins --fix` 仅剩增量编译目录 `os error 5` 的环境告警；`cargo fmt` 已应用。为此在 `src-tauri/Cargo.toml` 增加 `[[bin]] dev_mcp_server`（仅测试用，不进 Tauri bundle），`[dev-dependencies]` 增加 `tokio-util`（仅测试构造 `CancellationToken`，运行期依赖不变）。

---

## 批次 C6 — 最小 server 配置页（已完成 2026-11-14）

对应规格 §6.4。

- [x] 列表：`name` / `enabled` / 运行时状态（Ready / Error / Stopped）——`McpServerSettings` 行内展示名称、状态 Tag（就绪 / 启动中 / 错误 / 已停止）、`command + args`、`cwd`、`env` 掩码、`lastError`（`config_error` 已由后端并入 `lastError`），右侧 `Switch` 显示 `enabled`
- [x] 增删改：`name` / `command` / `args` / `cwd` / `env`；`id` 创建后只读——新建走 `create_mcp_server`（`id` 由后端派生，抽屉内不出现）；编辑时 `id` 以禁用输入框只读展示，仅改展示名时不下发 `launch`（不重建配置、不换授权键）
- [x] 启停开关即时生效；`env` 值掩码显示、不回显明文——`set_mcp_server_enabled` 直接更新行状态；`env` 只回键名，列表用 `KEY=••••••` 掩码，编辑抽屉显示「值不回显」提示；改启动配置且已有 `env` 时强制整份重填，避免旧值被静默清空
- [x] 隔离边界文案（不得宣称 OS 沙箱）——页首常驻提示：以当前用户权限在本机运行，只按「工具名 + 审批键」准入，不是操作系统沙箱
- [x] i18n + vitest；`npm run lint` / `typecheck` / `vitest` 通过

**验收记录（2026-11-14）**：`npm run typecheck` 通过；`npm run lint` 通过（含 format）；`npx vitest run` 53 文件 / 650 项通过（1 项既有跳过），新增 `src/settings/mcp/__tests__/serverForm.test.ts`（16 项）与 `src/services/__tests__/mcpService.test.ts`（5 项）；`settingsSnapshot.golden` 两个夹具同步更新（仅新增 `mcp-servers` 页，`pages` 7→8 / 8→9）。新增文件：`src/services/mcpService.ts`、`src/stores/mcpServerStore.ts`、`src/settings/mcp/serverForm.ts`、`src/settings/mcp/McpServerSettings.tsx`；页面注册在 `builtinRegistry`（`categoryId: provider`，`order: 55`，`emptyPage: true`）。

---

## 验收记录

| 批次 | 日期 | 结果 |
|---|---|---|
| C1 | 2026-11-14 | 完成（lib 167 + `gateway` 38，clippy / fmt 通过） |
| C2 | 2026-11-14 | 完成（lib 184 项全绿，`mcp::stdio` 15 项 + panic 收敛 1 项，clippy `-D warnings` 通过） |
| C3 | 2026-11-14 | 完成（lib 194 项全绿，`mcp::manager` 9 项 + `registry` 注销 1 项，clippy `-D warnings` 通过） |
| C4 | 2026-11-14 | 完成（storage `mcp_servers` 6 项 + `mcp::config` 4 项 + `permission_policy` 禁用层 1 项全绿；`protocol_bindings` 与 `contract:gen` 通过；clippy / fmt 通过） |
| C5 | 2026-11-14 | 完成（`tests/mcp_e2e` 11 项全绿，Rust 夹具二进制；全仓 `cargo test` 通过；clippy / fmt 通过） |
| C6 | 2026-11-14 | 完成（最小 server 配置页；typecheck / lint / vitest 650 项通过，golden 夹具同步） |

---

## 范围外登记

以下事项在排查 P7 崩溃风险时发现，但**不属于 P7 批次**，不在本清单勾选范围；此处只登记，避免丢失。

| 事项 | 位置 | 现状 | 影响 |
|---|---|---|---|
| turn 任务丢弃 `JoinHandle`，panic 后无终态事件 | `src-tauri/src/cmd/agent/mod.rs` turn 派发处 | dev（unwind）下工具 panic 只杀 turn task，前端永远停在「进行中」；release（`panic = "abort"`）下直接退出进程 | 与 MCP 无关，但 MCP 面对不可信外部输入会显著提高触发概率；修复点在 turn 生命周期（保留 `JoinHandle` + 补发 `Failed` 终态） |

规格侧对应记录见 [I38e §4.4](./I38e-mcp-tools-spec.md) 「已知缺口」段。
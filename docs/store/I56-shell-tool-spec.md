# I56 — `shell` 工具规格

| 项 | 值 |
|---|---|
| 状态 | 草稿（P14 的 C5–C7 实施规格；任务级进度只在 [I52 tasklist](./I52-shell-sandbox-tasklist.md) 维护） |
| 日期 | 2026-11-19 |
| 阶段 | P14（工具能力轴；见 [I38 roadmap](./I38-agent-roadmap.md) §2 / §3） |
| 交接 | 本文接管 [I52](./I52-shell-sandbox-spec.md) 原 §5–§13 的 `shell` 工具职能。I52 保留阶段定义、术语与分层（两根轴 / 三层职责 / 封闭词汇）与沙箱指针，**不再复述工具细节**；两份文档的分工见本文 §2.3 |
| 关联 | [I52](./I52-shell-sandbox-spec.md)（阶段规格与术语唯一来源）、I53（已归档）（沙箱准备：后端抽象 / 三平台实现 / 探测收口）、[I52-Ext](./I52-ext-background-task.md)（后台任务改动面，未立项）、[I38g](./I38g-agent-approval-advisor-spec.md)（P10 顾问与三个接入点）、[I38f](./I38f-agent-subagent-spec.md)（子代理输出域）、I18 design §3.17 / I50（均已归档，分别给检测分层与审计词表）、[OSCALLDESIGN](../designed/OSCALLDESIGN.md)（Shell 洋葱模型）、[agent-sandbox-reference-analysis](./agent-sandbox-reference-analysis.md)（Codex / OpenHuman / DSH 实现细节） |
| 影响范围 | `assistant-tools`：`os/tools/agent_shell/`（新增；`spawn.rs` 内做 `program` 候选解析，复用 `sandbox/windows/mod.rs` 已有的 `is_cmd_program`，**不改沙箱后端**）、`os/mod.rs`（`register_shell_tools`）、`runtime/storage.rs`（sink，已落地）、`runtime/compact`（信封分支）、`runtime/policy.rs`（`readToolOutput` 规则，已落地）、`runtime/gateway`（键与求值顺序，已落地）、`contract/trait.rs`（`sandbox_enforcement`，已落地）、`os/shared/path/resolve.rs`（`resolve_readable`，已落地）；`src-tauri/src/tool_setup.rs`（注册门禁，分支已就位）；结果 `data.sandbox.shell` 走契约生成（`npm run contract:gen`）；前端 i18n 与模式预置 |

> 一句话：把 `shell` 从「一个能跑命令的工具」写成「一份可逐条验收的契约」——身份、输入、审批、执行、输出、失败、文案七个面各有唯一判据，
> 且每个判据都能落到一个具体文件与一条具体断言上。

本文中的「必须 / 禁止」为可验收约束；「应当」为默认做法，偏离需在代码注释里写明理由。

---

## 1 目标与非目标

### 1.1 目标

1. 交付 `shell` 工具本体：`execute` 意图、命令级审批键、进程级超时、整树回收、输出流式落盘与回读、结果可被模型纠错；
2. 让审批卡如实回答「这次执行被什么边界包着」（隔离强度 / 可写根 / 命令类别）；
3. 让「一次工具调用」的生命周期有唯一终态定义：**调用结束 = 进程树已回收 = 输出已收尾**；
4. 让工具输出可被模型回读（消除「落盘即失联」），且回读不越出本会话输出域；
5. 全程 fail-closed：不存在「探测失败就无沙箱执行」的路径；
6. 给 P10-b 一个稳定事实源：命令类别（`CommandClass`）与隔离强度（`enforcement`）。

### 1.2 非目标

- **不新增命令字符串黑名单**：字符串匹配不是边界，禁止作为拦截依据（I52 §2.3）；
- **不替代审批**：沙箱不改变 Policy / Gateway 的决策（I52 §2.3 硬规则 1）；
- **不做交互式会话**：无 PTY、无 stdin 写入、无中途返回，一次调用 = 一次执行；
- **不做后台任务**：异步执行 / 任务 id / 状态机 / 轮询回收不在 P14，改动面与立项前置见 [I52-Ext](./I52-ext-background-task.md)，边界约束见 [I52 §8](./I52-shell-sandbox-spec.md)；
- **不做命令级授权归一化**（同一 `argv[0]` 合并键）——S1 候选，见本文 §15 OQ2；
- **不开放跨会话输出读取**：回读只作用于本会话输出命名空间（§8.7）；
- **不改 I50 审计词表**：隔离强度走工具结果与审批卡，不进 `AuditOutcome`（§10）；
- **不新增前端审批组件分支**：扩展是开放字段，前端按 `label` + `value` 通用渲染（§6.5、§8.7）。

### 1.3 交付边界（本文与其它文档的分工）

| 主题 | 唯一来源 |
|---|---|
| 术语、两根轴（检测层 / 虚拟化层）、三层职责（Policy / 审批 / 沙箱） | [I52 §2](./I52-shell-sandbox-spec.md) |
| 沙箱后端抽象、`SandboxPlan` 推导、链式探测、`enforcement` 判据、三平台实现、`kill_tree` 语义 | I53（已归档）；本文只消费结论 |
| `shell` 工具的身份、输入、审批、执行、输出、失败与文案 | **本文** |
| 阶段级验收（A1–A5）、阶段分批（C1–C8）与任务进度 | [I52](./I52-shell-sandbox-spec.md) §10 / §11、[I52 tasklist](./I52-shell-sandbox-tasklist.md) |

---

## 2 现状（以代码为准，2026-11-18）

### 2.1 已就绪（可直接消费，本文不重复实现）

| # | 事实 | 落点 |
|---|---|---|
| P1 | `Tool` trait 已有 `sandbox_enforcement()`，默认 `None`；`ShellTool` 必须覆写为 `Some(probe.enforcement)` | `contract/trait.rs` |
| P2 | 审批求值顺序已是「策略先、`Deny` 立即返回、`Ask` / `Allow` 才查授权」 | `runtime/gateway/mod.rs`（顺序注释在求值入口） |
| P3 | `ApprovalScope::Command { argv, cwd, sandbox_tier, enforcement }` 与 `split_argv` / `command_cwd` 已实现，首个生产者是本文的工具 | `runtime/gateway/approval_key.rs` |
| P4 | 落盘 sink 已落地：`ToolResultStorage::{persist, open_sink, namespace_root}`、`ToolResultSink::{write, finish}`、`SpilledOutput`、`MAX_SPILL_BYTES = 60 MiB`、`STDERR_RING_CHARS = 2 MiB`、`persisted_reference_from_tail` | `runtime/storage.rs` |
| P5 | `ToolResult.persisted` 已落地且 `#[serde(skip)]`；Pipeline 对已带 `persisted` 的结果跳过二次落盘 | `contract/result.rs`、`runtime/pipeline.rs` |
| P6 | `resolve_readable` / `ReadableScope` / `ToolUseContext.tool_output_root` 已落地；`readToolOutput` 规则与 `output.scope` 卡片扩展已落地 | `os/shared/path/resolve.rs`、`runtime/policy.rs`、`runtime/gateway/context.rs` |
| P7 | 沙箱三后端与 `sandbox::detect`（进程内 `OnceLock` 缓存）已落地；`tool_setup.rs` 已有 `shell_registration_allowed(probe)` 门禁，分支内 `register_shell_tools` 是**占位** | `sandbox/`、`src-tauri/src/tool_setup.rs` |
| P8 | 进程输出解码（`decode_process_output`）与 `truncate_output` 已就绪；后者**不再用于 `shell`**（改为落盘 + 窗口预览） | `os/shared/output.rs`、`os/shared/text_encoding.rs` |

### 2.2 未就绪（本文定义、C5 交付）

| # | 缺口 | 归属 |
|---|---|---|
| G1 | `os/tools/agent_shell/` 不存在：schema / validate / intents / 分类器 / preview / execute 全缺 | C5 |
| G2 | `os::register_shell_tools` 不存在，`tool_setup.rs` 分支仍是占位 | C5 |
| G3 | `compact` 的 `subject` / `scale` 无 `shell` 分支（head 会退化为无信息量的 `ok shell`） | C5 |
| G4 | `ToolErrorKind::Business` + `output_spill_failed` / `sandbox_unavailable` 等错误码尚无生产点 | C5 |
| G5 | `tool_desc.shell` 的 zh / en 文案与模式预置（默认不勾选） | C7 |
| G6 | 审计侧命令行脱敏（`NAME=value` / `--token <v>`）尚无调用点 | C6 |

### 2.3 与 I52 的交接边界

- 本文接管原 I52 §5–§13 的全部内容；I52 保留阶段定义（§1）、术语分层（§2）、沙箱指针（§3–§4）、阶段分批（§6）与阶段边界（§8）；
- **同一事实只在一处定义**：本文不再复述沙箱机制、`enforcement` 判据与三层职责，只写「`shell` 如何使用它们」；
- 后续修改：工具行为改本文，阶段范围改 I52，沙箱改 I53（已归档，重启需另立规格）。

---

## 3 身份与注册

### 3.1 身份声明

| 项 | 值 | 理由 |
|---|---|---|
| `id` / `name` | `shell` | 白名单、审计、报错前缀同源；不另设别名 |
| `tier` | `ToolTier::Os` | 创建 OS 进程。**不得**降为 `Sandbox`：tier 上限是「装配过滤」而非隔离，降级会让 `max_tool_tier = sandbox` 静默放行高危工具 |
| `audience` | `ToolAudience::AGENT_ONLY` | ToolBox 入口不解析工作区（P12），拿不到 `writable_root` → 必然失败，故不暴露（§15 OQ4） |
| `behavior_hints` | `read_only = false`、`destructive = true`、`open_world = true` | 静态声明取最坏值（该方法无入参）；逐命令的真实类别由 §5.2 表达 |
| `is_concurrency_safe` | 恒 `false` | 进程执行争用资源，输出无法与其它调用交错 |
| `manages_own_execution_timeout` | `true` | `exec::invoke` 的统一 deadline 对子进程无效，必须自管超时 + 杀树 |

`behavior_hints` 取最坏值的副作用：审批卡 L0 恒显示「破坏性 / 可外连」。这是**有意**的——卡片另有 `CommandClass` 扩展给出这一次的真实类别（§5.2），两者不合并。

### 3.2 注册门禁（fail-closed 的第一道闸）

```rust
// os/mod.rs
/// 沙箱组：只有探测结论 `enforcement != none` 时才允许调用。
pub fn register_shell_tools(registry: &ToolRegistry, probe: &'static SandboxProbe) { ... }
```

| 规则 | 判据 |
|---|---|
| 1 | `tool_setup.rs` 在 `shell_registration_allowed(&probe)` 为 `true` 时调用 `register_shell_tools`，否则**不注册**（模型看不到工具，比看到再报错少一轮无效迭代） |
| 2 | `ShellTool` 持有 `&'static SandboxProbe`（`detect` 的缓存引用），运行期**不再探测** |
| 3 | `sandbox_enforcement()` 返回 `Some(probe.enforcement)`；`probe.enforcement == None` 时不得构造 `ShellTool`（构造期断言，防门禁被绕过） |
| 4 | 工具装配面板**不提示**「沙箱不可用」：不注册即不出现，平台差异不得噪声化 |

### 3.3 默认启用策略

- 模式预置里 **默认不勾选**：`shell` 属破坏性工具，需用户显式启用（I38c §7.1 破坏性工具原则）；
- 未勾选时模型 `tools` 列表不含 `shell`（A18）。

---

## 4 输入契约

### 4.1 schema

| 字段 | 类型 | 必填 | 约束 |
|---|---|---|---|
| `command` | string | 是 | 去首尾空白后非空，长度 ≤ 8192 字符 |
| `cwd` | string | 否 | 相对路径按 `writable_root` 解析；解析后必须落在 `writable_root` 内 |
| `timeout_ms` | integer | 否 | 1000–1800000；缺省 120000 |

- `additionalProperties: false`；
- **无** `shell` / `program` / `env` / `stdin` 字段：可执行程序与可写根都不接受模型输入（`SandboxPlan` 的推导是沙箱侧的硬约束）；
- `timeout_ms` 上限取 1800000（30 分钟）而非 600000：本仓是 Rust 工作区，冷 `cargo build` / 全量 `cargo test` 轻易越过 10 分钟，参考实现（OpenHuman）正因此取消了默认超时。**默认值仍是 120000**——长任务必须由模型显式请求（写进工具描述，§11.2）。

### 4.2 `validate_input`（先于审批）

顺序固定 `find → validate → gateway → execute`。本步失败**不弹审批框**：

| 顺序 | 检查 | 失败形态 |
|---|---|---|
| 1 | `command` 形状（非空 / 长度） | `ValidationResult::invalid`（可改写重试） |
| 2 | `cwd` 解析 + `ensure_within_root` | `blocked`（`blocks_input_rewrite = true`：换路径重试同一意图不构成新意图） |
| 3 | `timeout_ms` 范围 | `invalid` |
| 4 | `workspace_root` 缺失 | `blocked` + `error_code = "sandbox_root_missing"` |

**禁止**在本步做命令内容匹配（I52 §2.3 硬规则 3）。`cwd` 解析必须复用 `os::path`，不得另写一套路径归一。

### 4.3 输入的规范化

- `command` **不裁剪内部空白、不改写引号**：审批键与执行体必须是**同一个字符串**（§6.1 的 R2 取舍）；
- `cwd` 缺省取 `writable_root`；给出相对路径时以 `writable_root` 为基准；
- 超出 `MAX` 长度的 `command` 直接 `invalid`，不做截断：截断会让「看到的就是执行的」失效。

---

## 5 权限意图、分类器与预演

### 5.1 `permission_intents`

返回单条 `PermissionIntent { action: "execute", resources: [command] }`。

- 一条命令 = 一条意图 = 一个审批键；**不拆**成多条意图（拆了会让「`A && B` 只批了 A」的错觉成立）；
- 审批键由既有 `ApprovalKey::from_intent` 生成：`ApprovalScope::Command { argv, cwd, sandbox_tier, enforcement }`；
- `split_argv` 对未闭合引号返回 `None` → 空键 → 不查缓存直接走审批（既有 fail-closed 行为，不改）。

### 5.2 命令分类器（`CommandClass`）

纯函数、无 IO、输入为 `command` 字符串；**只用于展示与 P10-b 输入**，不参与任何放行决策。

判定顺序（首个命中即返回）：

| 顺序 | 条件 | 结果 |
|---|---|---|
| 1 | 含 shell 控制符（`|`、`&&`、`||`、`;`、`>`、`<`、`` ` ``、`$(`、换行） | `unknown` |
| 2 | `argv[0]` 与子命令命中只读清单（`git status` / `git log` / `git diff` / `git show`、`ls` / `dir`、`cat` / `type`、`rg` / `grep` / `find` / `where`、`head` / `tail` / `wc`、`pwd`、`echo`、`--version` 类） | `readOnly` |
| 3 | 命中网络清单（`curl` / `wget` / `ssh` / `scp`、`npm install` / `pip install`、`git clone` / `git push` / `git fetch`） | `network` |
| 4 | 命中破坏清单（`rm` / `del` / `rd` / `rmdir`、`format`、`shutdown`、`taskkill`、`reg delete`、`git reset --hard` / `git clean`） | `destructive` |
| 5 | 其余 | `unknown` |

硬约束：

1. 顺序 3 先于 4：`git clean` 这类复合语义宁可归 `network` 也不归「无害」——**错误方向必须是更危险**；
2. 未知**不得**映射为 `mutating`，更不得映射为 `readOnly`；`unknown` 是显式类别，卡片如实显示「无法判定」；
3. 清单是**开放集合**（命中即分类，未命中即 `unknown`），不做「未在清单内 = 禁止」。

### 5.3 `preview_for_approval`

返回 `ApprovalPreview { summary, truncated: false, estimated: true }`：

- `summary` 形如 `执行 1 条命令 · 只读 · 隔离：完全`；
- `estimated = true`：命令的实际影响在执行前不可知（TOCTOU），**禁止**声称「将删除 N 项」这类精确量；
- `truncated` 恒 `false`（不存在扫描上限）；
- 纯字符串工作、不做 IO（因此不会因 2s 预算超时降级为 `None`）。

---

## 6 审批链集成

### 6.1 审批键

`ApprovalScope::Command` 是 P2 已实现、至今无生产者的分支；本文的工具是它的第一个生产者。**不新增键类型、不改归一化规则**。

| 键组成 | 取值 | 归一化 |
|---|---|---|
| `argv` | `split_argv(command)` | 未闭合引号 → `None`（空键，不查缓存） |
| `cwd` | 输入 `cwd` ?? `writable_root` | `canonical_path_for_key`（不可归一 → 原样，仍可作键） |
| `sandbox_tier` | `tool.tier()`（恒 `Os`） | 不变 |
| `enforcement` | §6.2 | 三档枚举 |

`AllowSession` 的授权粒度 = **这一条命令行** + 这个 `cwd` + 这个隔离强度。同会话内重复执行完全相同的命令不再弹框；命令文本任何差异都会重新询问。这是有意的安全取舍，归一化候选见 §15 OQ2。

### 6.2 `enforcement` 必须进键

**问题**：`sandbox_tier` 取自 `tool.tier()`，对 `shell` 是常量 `Os`。若沙箱从 `full` 退化为 `partial`（Linux 从 ABI≥4 换到 ABI<4、或后端建立失败），同一 `argv` + `cwd` 会命中同一授权键——**等于把「全隔离下批准的授权」复用到更弱的边界上**，方向是放宽权限。

`Command` 变体因此带一个字段（已落地，见 P3）：

```rust
Command {
    argv: Vec<String>,
    cwd: String,
    sandbox_tier: ToolTier,
    #[serde(default)]
    enforcement: Option<SandboxEnforcement>,
}
```

序列化口径：`#[serde(default)]` 用于旧 `grant_key` JSON 解码（旧行 → `None`），**不加** `skip_serializing_if`（键恒在、值可为 `null`，与全仓 Option 口径一致）。`shell` 恒为 `Some(..)`（只在 `>= partial` 时注册），故新旧键必然不相等 → 旧授权不复用（fail-closed，方向是多问一次）。

### 6.3 策略

- `base_rules` 现状已覆盖 `execute`（`dangerous_only` 恒 `Ask`、`none` 恒 `Allow`），`execute` 侧**不新增规则**；
- 本阶段唯一新增的 Policy 规则是 `readToolOutput`（恒 `Ask`，任何审批级别不放宽，§8.7）；
- **不新增命令黑名单规则**；`hard_deny_rules` 继续只针对路径；
- `none` 档下 `shell` 无审批，此时唯一边界是沙箱（I52 §2.3 硬规则 1）——这是 P10-a `EscalateOnly` 的真实用武之地（§6.6）。

### 6.4 `Deny` 先于授权命中

顺序已修正（P2），本文只声明这条顺序是 `shell` 的正确性前提：

```
policy.evaluate(action, resource)
  ├─ Deny  → 审计 Denied，立即返回 Denied
  ├─ Allow / Ask → 继续
grant_store.is_granted(key)
  ├─ 命中 → 审计 GrantHit，continue
  └─ 未命中 → 按 effect：Allow 审计 Allowed / Ask 进 pending
```

理由：`shell` 是首个把「同一个键可能长期有效」变成常态的工具（会话级授权 + 命令级键）。先查缓存会让「此后被策略改判 `Deny` 的键」继续放行。

### 6.5 审批卡扩展

`build_approval_context` 在 `tool.sandbox_enforcement()` 为 `Some(e)` 时追加两条 `ApprovalExtension`：

| `kind` | `label` | `value` | `severity` |
|---|---|---|---|
| `sandbox.enforcement` | 隔离强度 | `完全隔离` / `部分隔离` | `Info` / `Caution` |
| `sandbox.root` | 可写根 | `writable_root` 的**末段目录名**（不写绝对路径） | `Info` |

命令类别由 `preview_for_approval` 的 `summary` 携带（§5.3），**不重复**成第三条扩展。

零前端改动的依据：`ApprovalExtensionDto` 的 `kind` / `label` / `value` / `severity` 是开放字段，前端按 `label` + `value` 通用渲染（I38d §3.2.1，已归档）。**禁止**为这两个 `kind` 新增前端分支或 DTO 字段。

### 6.6 P10 的三个接入点

| 接入点 | 本文交付的事实源 | P10 侧用途 |
|---|---|---|
| `Ask` 消费方 | `execute` 意图在 `dangerous_only` 档恒 `Ask` | 关闭 I38g §1.3 的 P10-b 前置 |
| 命令类别 | `CommandClass`（§5.2，纯函数） | P10-b 的**唯一**输入事实；顾问不得自行再判命令语义 |
| 内核隔离 | `enforcement`（沙箱侧推导） | 关闭 I18 design §3.17.6 的 `Adjudicate` 门禁 1 |

边界：P10-b 的放宽条件由 [I38g](./I38g-agent-approval-advisor-spec.md) 规定，本文**不写**放宽逻辑，只保证「`readOnly` + `full` 可被顾问识别」，且至少要求 `full` 才具备放宽资格（`partial` 不放宽）。`Adjudicate` 不因本阶段完成而开工（门禁 2 未关）。

---

## 7 执行契约

### 7.1 顺序（固定）

顺序不是风格问题，而是正确性：**先建 sink、先接管管道、最后才进入等待**。反过来（先 `wait()` 再读管道）会死锁——子进程写满匿名管道缓冲区（Windows 4–64 KiB、Unix 64 KiB）后阻塞，父进程在 `wait()` 上等子进程退出，双方互等；`yes` / `cargo build` / 任何有进度输出的命令都会立刻触达该缓冲区。

| 步 | 动作 | 失败处理 |
|---|---|---|
| 1 | 取 `writable_root`（`ToolUseContext.workspace_root`） | `Business` + `sandbox_root_missing` |
| 2 | 读 `probe.enforcement`；`none` → 拒绝 | `Business` + `sandbox_unavailable`，**不得**无沙箱执行 |
| 3 | 构造 `SandboxPlan`（§7.3）并创建调用级临时目录、注入 `TEMP` / `TMP` / `TMPDIR` | `Business` + `scratch_dir_failed`（此时无子进程；已建 sink 按第 5 步同法删除） |
| 4 | **先 `open_sink`** | `Business` + `output_spill_failed`，此时尚未创建任何子进程 |
| 5 | `backend.spawn(&plan)` | 删空文件（尽力而为）；边界失败 → `sandbox_setup_failed`，程序/进程失败 → `spawn_failed` |
| 6 | **立刻**起两个并发读取任务接管 stdout / stderr（各占一个 blocking 线程），逐块 `sink.write(stream, chunk)` 直到 EOF | — |
| 7 | `wait()` 丢进 `spawn_blocking` 得到收尸任务；async 侧 `select!` 与 `timeout_ms` deadline、`context.cancel` 竞争 | — |
| 8 | 收尾：**任何终态都「杀 + 收尸」**（§7.5） | — |
| 9 | **有界 join** 两个读取任务（宽限 2s）；仍无 EOF 即放弃读取句柄继续 | 放弃的是 `spawn_blocking` 任务而非句柄本身，线程滞留到管道关闭（§15 R8） |
| 10 | `sink.finish()`（**四种终态一律调用**） | 失败 → `Business` + `output_spill_failed`，**不得**退回内联全文 |
| 11 | 组装结果（§8.4），`ToolResult.persisted` 置为 sink 给出的引用 | — |

第 4 步必须在第 5 步之前：落盘是执行的前置条件，不是事后收尾——否则「磁盘满」会先跑完命令再判失败。

### 7.2 线程模型（实现约束）

| 步骤 | 落点 | 理由 |
|---|---|---|
| 4 `open_sink` | `async fn` 入口，实现内部用 `spawn_blocking` 包住建文件与写头 | 接口本就是 `async`，调用方无法再包一层；阻塞必须收在实现内部 |
| 5 `spawn`、6 两个读取循环、8 的 `wait` 收尸 | 调用方 `tokio::task::spawn_blocking` | 全是同步阻塞调用（`CreateProcess` / `Read::read` / `Child::wait`） |
| 7 deadline / cancel 竞争、9 有界 join | async 侧（`select!` + `tokio::time::timeout`） | 只有 `await` 点可被抢占 |
| 10 `sink.finish()` | `spawn_blocking` | 追加 stderr 段、删空文件、解码头尾窗口都是同步文件 IO |

- `assistant-tools` 不依赖 tauri，一律用 `tokio::task::spawn_blocking`，**禁止**用 `tauri::async_runtime`；
- **禁止**为 `shell` 自建 `std::thread`：自建线程不在 tokio 收口范围内，turn 取消令牌管不到它；
- 一个活跃 `shell` 常驻 3 个 blocking 线程（stdout 读 / stderr 读 / 收尸），长命令期间不释放；资源账见 §15 R8。

### 7.3 `SandboxPlan` 的推导（消费侧口径）

| 字段 | 取值 | 约束 |
|---|---|---|
| `program` | 平台固定 shell 的**绝对路径**，由**有序候选 + 存在性校验**解析（见下） | Windows `%SystemRoot%\System32\cmd.exe` → `%COMSPEC%`；Unix `/bin/sh` |
| `args` | Windows `["/d", "/C", command]`；Unix `["-c", command]` | `/C` 之后是**一段脚本**而非参数，引号与元字符由 shell 自己解析 |
| `cwd` | 已 canonicalize 且落在 `writable_root` 内 | 传给子进程前剥掉 `\\?\` / `\\?\UNC\` 前缀（否则 `cmd.exe` 当成 UNC 起始，回退到 `Windows` 目录） |
| `writable_root` | 会话解析结果 | **不可**由模型输入指定 |
| `env` | §7.4 的白名单 + 强制注入 | 不做用户自定义 env 入口 |
| `network` | P14 恒 `false` | 不代表「命令不能联网」，见 §8.4 |
| `stdin` | 恒 `None` | 无 stdin 通道 |

**`program` 的解析**（有序候选，任一步失败即 fail-closed）：

| 步 | Windows | Unix |
|---|---|---|
| 1 | `%SystemRoot%\System32\cmd.exe`（`SystemRoot` 缺失时按 `C:\Windows`） | `/bin/sh` |
| 2 | `%COMSPEC%`——**仅当 basename 为 `cmd.exe`**（大小写不敏感）时采用 | — |
| 3 | 候选全不存在 → 并入探测结论判为不可用：走 §3.2 规则 1 的同一道闸，`shell` **不注册**，不降级为裸名 | 同左 |

**为什么是「候选」而不是「单一常量」**：三处参考实现都用有序候选（DSH `candidatePwshPaths`、Codex `get_shell_path` 的 fallback 列表、OpenHuman `bash_path()`），没有一家把 shell 路径写成编译期常量（§16.4）。本文采纳的是**解析方式**，不是它们的 shell 种类。收益只有两条，且都不涉及隔离强度：

| 收益 | 说明 |
|---|---|
| 失败前移 | cmd 缺失时是「工具未注册」，而不是用户批准后 `spawn_failed` |
| 环境异常兜底 | 父进程 `SystemRoot` 被裁剪 / 篡改时，`COMSPEC` 是独立来源 |

**三条约束必须同时成立**，否则原不变式（§7.7）会被绕过：

1. **候选集只含系统目录与 `COMSPEC`，不遍历 `PATH`**：裸名交给 `CreateProcessW` 时搜索顺序第一项是**应用自身目录**，比 `PATH` 更容易被工作区或应用目录里的同名文件顶替；
2. **`COMSPEC` 必须校验 basename**：它是用户可改写的环境变量，不校验等于「shell 种类可被环境变量替换」，与 §4.1「可执行程序不接受模型输入」并列的硬约束形同失效。沙箱后端已有 `is_cmd_program`（basename 大小写不敏感比较，`sandbox/windows/mod.rs`），解析处直接复用，不写第二份判定；
3. **解析结果不进审批键**：键仍是整串命令行 + `cwd` + `enforcement`（§6.1 / §6.2）；解析来源（`system32` / `comspec`）只进 `data.sandbox.shell`（§8.4）与日志，**不进审批卡**（卡片不写绝对路径，§6.5）。

### 7.4 环境变量

**透传白名单**（不存在则跳过）：Windows `SystemRoot` / `SystemDrive` / `PATH` / `USERPROFILE` / `HOMEDRIVE` / `HOMEPATH` / `COMSPEC` / `PATHEXT` / `OS` / `PROCESSOR_ARCHITECTURE` / `NUMBER_OF_PROCESSORS` / `ProgramFiles` / `ProgramFiles(x86)` / `ProgramData`；Unix `PATH` / `HOME` / `LANG` / `LC_ALL`。

两条**刻意排除**：

1. **`TEMP` / `TMP` / `TMPDIR` 不透传**：父进程真实临时目录在可写根之外，透传等于让每个写临时文件的命令（`python tempfile`、`npm` / `pip` 缓存、`cargo` 构建脚本、`7z`、部分链接器）必然 `Access is denied`，而模型只看到「命令失败」并原地重试。改为强制注入调用级临时目录；
2. **工具链路径变量一律不注入**（`CARGO_HOME` / `RUSTUP_HOME` / `JAVA_HOME` / `GRADLE_USER_HOME` / `VIRTUAL_ENV` / `APPDATA` / `LOCALAPPDATA`）：它们指向可写根之外（缓存写入必然失败），且 `CARGO_HOME/credentials.toml`、`~/.npmrc` 一类凭据文件常驻其中，注入等于把「读这些目录」写成工具契约的一部分。缺失导致的行为按 §15 R4 处理（如实报错 + 模型纠错）。构建缓存重定向列为 S1 候选。

**强制注入**（覆盖白名单结果）：

| 变量 | 值 | 目的 |
|---|---|---|
| `TERM` | `dumb` | 关掉 ANSI 与进度条重绘，落盘文件不被控制字符污染 |
| `NO_COLOR` | `1` | 同上（多数 CLI 尊重） |
| `PAGER` / `GIT_PAGER` | `cat` | 无 PTY 时 `git log` / `man` 类命令会挂到超时 |
| `GIT_TERMINAL_PROMPT` | `0` | 缺凭据立即失败，而不是等一个永远不来的输入（stdin 已为 null） |
| `TEMP` / `TMP` / `TMPDIR` | `<writable_root>/.assistant-tmp/<call_id>` | 临时文件必须落在可写根内 |

**git 配置加固**（同样强制注入，切断 git 从工作区配置拉外部程序）：

| 变量 | 值 | 目的 |
|---|---|---|
| `GIT_CONFIG_NOSYSTEM` | `1` | 忽略系统级配置，行为不受宿主机器影响 |
| `GIT_CONFIG_COUNT` / `GIT_CONFIG_KEY_0` / `GIT_CONFIG_VALUE_0` | `1` / `core.fsmonitor` / `false` | 关掉仓库级 `core.fsmonitor` 拉起的外部程序 |

**不穷举** `filter.*.clean` / `core.hooksPath`：它们指向工作区内的文件，命令仍在沙箱内执行，越界写照样被拒；穷举会让「加固清单」变成第二个需要维护的黑名单。

不变式：强制注入只做三件事——**消除交互式挂起与渲染噪声**、**把临时文件收进可写根**、**切断 git 从外部拉程序**。三者都不改变命令语义。**不得**注入 `CI=1` 一类会改变构建行为的变量。

**强制规则**：

1. 名字（大小写不敏感）命中 `KEY` / `TOKEN` / `SECRET` / `PASSWORD` / `CREDENTIAL` 的一律**不透传**；
2. `ASSISTANT_*` 前缀一律不透传；
3. 不做「用户自定义 env」入口；
4. 强制注入的变量不受规则 1 约束（它们是工具自己的常量与计算值）；
5. 临时目录里的文件**不进落盘输出、不进审计**，调用结束随目录删除。

### 7.5 超时、取消与杀进程树

| 触发 | 行为 | `error_kind` | `data` |
|---|---|---|---|
| 正常结束 | 收尸 → `kill_tree()` 扫尾 → join（宽限 2s）→ `finish` | 无（`ok = true`） | 完整（§8.4） |
| `timeout_ms` 到期 | `kill_tree()` → 收尸 → join → `finish` | `Timeout` | **保留**：`outputPath` + 已读计数 |
| turn 取消令牌 | 同上 | `Cancelled` | **保留**：同上 |
| 输出触达文件上限 | **不杀树**，继续读并丢弃 | 无（`ok` + `outputTruncated`） | 完整 |

**正常结束也要扫尾**：`wait()` 只回收**直接子进程**，`cmd /C` 与 `sh -c` 派生的孙进程可能仍在运行并继续持有管道写端。若不扫尾，第 9 步 join 会等一个永不到来的 EOF，而 deadline 此时已失效——调用挂死且无人兜底。因此「命令退出」与「整树回收」是两件事：正常路径必须补一次 `kill_tree()`。`kill_tree()` 幂等，两种终态调用同一实现。

**四种终态一律 `finish()`**：sink 持有文件句柄与头尾窗口，不 `finish` 等于把已写好的输出丢掉。超时 / 取消因此是「失败 + 可回读的部分输出」——`data` 仍带 `outputPath` / `outputBytes` / `outputLines`（截至杀树时的计数），回填文本末尾追加 `[timed out after <N>ms]` 或 `[stopped: turn cancelled]`。失败但保留 `data` 由 `ToolResult::err_with_data`（已落地）构造。

**`kill_tree` 必须是整棵树**：Windows `TerminateJobObject`（一次杀光 Job 内全部进程），Unix `kill(-pgid, SIGKILL)`。禁止只杀直接子进程——孙进程继承管道写端，杀不干净就让 join 等不到 EOF。

**收尸必须 `wait()` 到结束再返回**，避免僵尸进程与句柄泄漏；`wait()` 是阻塞调用，落在 `spawn_blocking`。

**异常路径不得遗留进程或文件**：`spawn` 成功后立刻把子进程登记进 `Drop` 守卫（守卫 drop 时若发现未收尸则 `kill_tree()`）；sink 的 `finish` 同样由守卫兜底——`execute` 中任何 `?` 提前返回都不得绕过它。

### 7.6 临时目录

- 路径：`<writable_root>/.assistant-tmp/<call_id>`；
- 在 `finish` 之后、结果组装之前删除（尽力而为：子进程刚退出时 Windows 上可能仍有句柄未释放，删除失败只记日志、不改结果；同名目录下次调用前按「已存在即复用/清空」处理）；
- 四种终态都必须走到这一步，因此正常结束、超时、取消、落盘失败之后工作区里都不留该目录（残留风险见 §15 R11）。

### 7.7 不做

不做 `PATH` 遍历（`program` 只取系统目录与 `COMSPEC` 候选，§7.3；结果仍是绝对路径）、不写脚本临时文件、不注入 `cd` 前缀（用 `cwd` 字段）、**不 source 用户 profile / 非 login shell**、不分配 PTY、不提供 stdin 写入通道、不做 `yield_time_ms` 式中途返回。

- **不 source profile**：参考实现（Codex `shell_snapshot`）会抓取并 source 用户 shell 配置；本阶段**不做**——它让「同一命令在不同机器上行为不同」，与「命令效果只由沙箱与命令文本决定」冲突，且抓取结果还要落盘、还要随配置变更失效。代价是 `PATH` 之外的别名 / 函数不可用，由白名单环境变量与绝对路径兜底；
- **不跟随用户默认 shell**：Codex 探测 `$SHELL`（Windows 上探测 PowerShell），并允许模型传 `shell` / `login`（`login` 默认 `true`）；本文**不采纳**——shell 种类随机器漂移会让「同一条命令文本在不同机器上语法不同」，模型无法预知该写 cmd 还是 PowerShell，审批键也无法描述执行体。代价是 Windows 上只有 cmd 方言，由工具描述（§11.2 第 3 条）与 `data.sandbox.shell` 回显兜底；
- **Windows 固定 `cmd.exe` 且带 `/d`**：`/d` 关闭 `HKCU\Software\Microsoft\Command Processor\AutoRun`——否则每次调用都会先执行注册表里配置的一段命令，污染输出并改变行为。由此 **Windows 上的命令语法是 `cmd` 而非 PowerShell**（`Get-ChildItem` / `$env:X` 不可用），这是工具描述必须写明的硬约束（§11.2）。独立 `pwsh` 工具列为 S1 候选。

---

## 8 输出契约

### 8.1 落盘模型

单文件、顺序固定：

```
<stdout 全文（上限 60 MiB）>
[truncated: output exceeded 60 MiB; total unknown]   ← 仅当 stdout 触达上限
--- stderr ---                                       ← 仅当 stderr 非空
<stderr 尾部窗口（≤ 2 MiB）>
```

**stderr 独占保留区**是本节的关键取舍：若 stdout 与 stderr 抢同一个总预算，`cargo build` 写满几十 MB 进度后失败的那一行错误会被挤掉，尾部窗口只剩进度条——工具在最需要它的场景下失效。因此 stdout 上限压到 60 MiB，stderr 单独留 2 MiB 内存环形窗口，`finish` 时追加到文件末尾。

单文件而非双文件：`compact.persisted` 只有一个槽位，双文件会让回填出现两个路径、`parse_persisted_tag` 出现两个候选。

**文件内容是「子进程说了什么」的原样记录**：只有 `[truncated: …]` 写进文件（它是内容事实）；`[exit code: …]` / `[timed out …]` 等是**执行事实**，只进回填文本。

**存储抽象（C4b 已落地，此处固定不变式）**：

```rust
pub trait ToolResultSink: Send {
    /// 逐块写入原始字节；`stream` 决定落盘区还是 stderr 环形窗口。同步签名：
    /// 调用方是 blocking 池里的读取循环，无法 `await`。
    fn write(&mut self, stream: OutputStream, chunk: &[u8]) -> Result<(), String>;
    /// 同步收尾；由调用方置于 `spawn_blocking`。
    fn finish(self: Box<Self>) -> Result<SpilledOutput, String>;
}
```

五条不变式（编号供其它文档引用）：

| # | 不变式 | 违反的后果 |
|---|---|---|
| 1 | `open_sink` 与 `persist` 写**同一根目录、同一 `session_namespace`、同一文件名规则**（`<unique_prefix>-<tool>-<call_id>.txt`），目录一律由 `namespace_root(...)` 派生 | 清理 / 落盘 / 回读出现第二个事实源 |
| 2 | `write` **永不**因超限返回 `Err`（stdout 超 60 MiB 只计数，stderr 超 2 MiB 环形覆盖），两种超限都继续消费管道 | 返回 `Err` 会让调用方停止读取 → 子进程写阻塞 → §7.1 的死锁 |
| 3 | 两个读取任务并发写入：sink 由 `Arc<std::sync::Mutex<Option<Box<dyn ToolResultSink>>>>` 持有，临界区只有一次 `write`；`finish` 在 §7.1 第 9 步 join 之后用 `lock().take()` **取走**再调用，**不得**用 `Arc::try_unwrap` | 有界 join 放弃后仍可能有读取线程滞留，引用计数归零不可保证；`take()` 与线程数无关，取走后滞留线程再 `write` 命中 `None`，只丢弃字节 |
| 4 | `finish` 负责四件收尾：空输出删文件且 `reference = None`；stdout 触达上限在段末补 `[truncated: …]`；stderr 非空追加 `--- stderr ---` 段（超窗口时头行带 `(last N of M chars)`）；`reference.bytes` 取**文件实际字节** | 少一件就会出现 0 字节文件 / 丢 stderr / 字节数失真 |
| 5 | 落盘失败 → `Business` + `output_spill_failed`，**不得**退回「全文内联」 | 会把已经消除的内存放大重新引回来 |

### 8.2 五条口径

1. **非零退出码不是工具失败**：`ok = true` + `exitCode != 0`，由模型判断（与 `grep` 零命中同理）。工具失败只保留给「无法执行」（spawn 失败、超时、取消、落盘失败）；
2. **输出不驻留内存**：stdout 边读边写落盘文件，内存只留尾窗口（`PREVIEW_CHARS = 1200` 字符）与计数器；stderr 额外驻留**有界**的 2 MiB 环形窗口（常量内存）；
3. **stdout 上限 `MAX_SPILL_BYTES = 60 MiB`**：触达后**继续读取并丢弃**（只计数），不杀进程（产出大量输出是合法结果）；置 `outputTruncated = true`，并在 stdout 段末尾写 `[truncated: output exceeded 60 MiB; total unknown]`。`total unknown` 是必写标记：此处无法给出总字节，不得写精确数；
4. **`truncated` 语义单一**：`outputTruncated` 只表示「落盘文件被上限截断」，不表示「预览窗口被裁剪」；预览裁剪用 `[… N chars omitted; full output in the file above …]`，二者不得混用；
5. **空输出不落盘**：`total_chars == 0` 时 `finish` 删除空文件、返回 `reference = None`，`outputPath = null`，回填文本为 `(no output)` + 标记行。留着 0 字节文件只会让回读拿到空串。

### 8.3 编码边界

两件事：分块切分与码页。

**分块切分**：`write` 收的是原始字节；`total_chars` 与尾窗口需要字符，但分块边界可能切在多字节 UTF-8 序列中间——**禁止**对每个 chunk 各自 `String::from_utf8_lossy` 后拼窗口（会把一个汉字拆成两个 `U+FFFD`）。正确做法：字符计数与尾窗口由一个**增量** UTF-8 解码器推进（非法序列计 1 字符、跨块未完成的尾字节留到下一块），`finish` 时把剩余不完整字节按 1 字符收口。

**码页（Windows）**：`cmd.exe` 内建命令（`dir` / `type` / `where` / `echo`）输出的是控制台 OEM 码页（zh-CN 为 936），而 `git` / `cargo` / `rg` 输出 UTF-8——同一条管道里可能两种编码混流。**禁止**用 `chcp 65001` 前置：它会改写命令文本（污染审批键与预览），且对已在运行的 `cmd` 无效。正确做法是**首 4 KiB 探测**（缓冲区满 4 KiB 或流提前 EOF 时判定一次）：

| 探测结果 | `outputEncoding` | 写盘方式 |
|---|---|---|
| 合法 UTF-8（允许末尾不完整序列） | `utf8` | 原始字节直写（不改编码、不做 CRLF 转换） |
| 非法 UTF-8 且 Windows | `oem<cp>`（`GetOEMCP()` 取值） | 按该码页**流式解码**（跨块保留未配对尾字节）后以 UTF-8 写入 |
| 非法 UTF-8 且 Unix | `utf8` | 原始字节直写，非法字节计 `U+FFFD` |

- Unix 侧不做转码：非 UTF-8 流可能是 GB18030 / Shift-JIS / Latin-1，猜错会把可读文本变成乱码，而 UTF-8 是唯一有明确自校验的编码。P14 接受该限制（§15 R9）；
- **判定只做一次、不按行重判**：按行重判会让同一文件里两种编码来回切换，`read` 每次拿到不同结果；
- **文件恒为 UTF-8**：转码发生在写盘前，因此回读（按 UTF-8 读）与落盘内容始终一致。这是「文件按字节原样写入」的唯一例外；`outputBytes` 仍按源流字节计、`PersistedRef.bytes` 按文件字节计。

### 8.4 结果形状（`data`）

```json
{
  "exitCode": 0,
  "signal": null,
  "outputPath": "<AppDirs::tool_results()>/<会话命名空间>/<unique_prefix>-shell-<call_id>.txt",
  "outputBytes": 123456,
  "outputLines": 1234,
  "outputTruncated": false,
  "stderrTruncated": false,
  "durationMs": 123,
  "outputEncoding": "utf8",
  "sandbox": {
    "backend": "landlock",
    "shell": "sh",
    "enforcement": "full",
    "network": false,
    "fsRead": true,
    "denied": false
  }
}
```

`sandbox` 里三个布尔是**本次调用的真实边界**，不是注册期结论的复述；`shell` 是本次执行的方言标签：

| 字段 | 含义 | 来源 |
|---|---|---|
| `network` | 命令**能否**联网 | `SandboxPlan.network` 与后端 `missing` 求交——Windows / Landlock ABI<4 恒 `false`（边界不存在，命令实际可联网） |
| `fsRead` | 读侧是否有隔离 | 同上（Windows 恒 `false`：可读任意用户可读文件） |
| `denied` | 本次是否被沙箱拒绝 | §8.5 |
| `shell` | 本次实际执行的 shell **种类**（`cmd` / `sh`），不是路径 | `SandboxPlan.program` 的解析结果（§7.3） |

`shell` 是**诊断字段**，唯一用途是让模型在命令失败后能区分「方言不对」与「沙箱拒绝 / 路径不存在」——工具描述（§11.2 第 3 条）只能事前告知，模型拿到 stderr 后仍需要一条事后证据。三条约束：只写种类（**不得**写绝对路径）、**不进审批键**（§7.3 约束 3）、不进审批卡（§6.5）。封闭集合用枚举，新增种类（如 `pwsh`）时同步契约生成。

**退出状态二选一**：Unix 被信号杀死时 `exitCode = null`、`signal` 为信号号；Windows 恒有 `exitCode`、`signal` 恒 `null`。**不得**用 `-1` 冒充「没有退出码」——`-1` 是合法退出码，混淆后模型会把正常退出读成崩溃。

**两个字节数不混用**：`outputBytes` 是子进程总产出字节（精确，触达上限后仍继续计数），`PersistedRef.bytes` 是文件实际字节；`outputTruncated` 时二者不等。Windows 转码同样让二者不等——**不得**因此置 `outputTruncated`：该标志只由「写入文件字节触达 `MAX_SPILL_BYTES`」触发。

`outputPath` 是绝对路径，是模型回读的唯一入口（§8.7）；空输出时为 `null`。`data` 不携带 `stdout` / `stderr` 全文。

### 8.5 沙箱拒绝判定（`data.sandbox.denied`）

命令内部的越界写不会让工具失败——它只表现为非零退出码 + 原始 stderr（Windows `Access is denied.`、Unix `Permission denied` / `Read-only file system`）。若不识别，模型会以为命令写错了而原地重试，用户也看不出「是沙箱拦的」。因此正常结束后做一次**纯函数**判定：

判定为「沙箱拒绝」需**同时**满足：

1. `enforcement != none`（本次确实有边界）；
2. `exitCode != 0`（Windows 侧无信号，Unix 侧含被信号杀死的路径）；
3. 下列之一：退出码等于 `128 + SIGSYS`；或 stdout / stderr 出现关键词（大小写不敏感）——`operation not permitted` / `permission denied` / `read-only file system` / `sandbox` / `landlock` / `access is denied` / `failed to write file`。

**必须排除** `2` / `126` / `127` 三个退出码：它们是「命令用错 / 文件不可执行 / 命令不存在」的常规信号（`sh` 语义），当成沙箱拒绝会让模型得出「换个落点就行」的错误结论。

命中后：`data.sandbox.denied = true`，回填文本末尾追加固定行 `[sandbox: write outside writable root denied]`。**不判工具失败**（`ok` 仍 `true`、`exitCode` 原样保留）——命令确实跑完了，失败的是它内部的一次文件操作。**不加**升级提示：P14 的三档是探测结论而非可选项，只让模型知道「换个落点重试」。

误判方向：宁可漏判（模型按普通失败处理、靠 stderr 纠错），不可错判成「沙箱拦的」——后者会引导模型绕过边界。

### 8.6 回填文本与标记

`result_for_assistant` = 落盘引用（标签 + **尾窗口** + 省略标记，由 `persisted_reference_from_tail` 渲染）+ 末尾标记行。**禁止**手写标签或自行拼省略标记。

**只取尾窗口，不取头窗口**：命令输出的信息密度集中在末尾——失败栈、测试摘要、最后一条进度。头部通常是启动横幅、依赖解析、进度条重绘（`cargo build` 的 60 MiB 日志即典型）。因此 `shell` 的预览**整块**取自文件末（`PREVIEW_CHARS = 1200` 字符），不与头部分享预算。这与整段生产者 `persisted_reference`（`read` / `grep` 用，头尾各 600）刻意不同：那里预览的是**文档**，开头就是内容；这里预览的是**日志**，结尾才是结论。

标记词汇固定（模型按这些串判断结果性质，实现不得各自发明）：

| 位置 | 标记 | 出现条件 |
|---|---|---|
| 段落头 | `--- stderr ---` | stderr 非空且未超窗口 |
| 段落头 | `--- stderr (last <N> of <M> chars) ---` | stderr 超过 2 MiB 环形窗口 |
| stdout 段之后 | `[truncated: output exceeded 60 MiB; total unknown]` | stdout 触达上限 |
| 文本末尾（先于退出码行） | `[sandbox: write outside writable root denied]` | `denied = true` |
| 文本末尾 | `[exit code: <N>]` | `exitCode` 非 `null` 且非 0 |
| 文本末尾 | `[killed by signal: <N>]` | `signal` 非 `null` |
| 文本末尾 | `[timed out after <N>ms]` | 超时 |
| 文本末尾 | `[stopped: turn cancelled]` | turn 取消 |

顺序固定：**正文在前、标记在末尾**。理由是 `parse_truncation` 从尾部 `rfind`，且尾窗口本就为「抓住末行」而设；标记若前置会被省略标记与正文隔断。空输出时正文位置写 `(no output)`，避免模型面对空串无从判断。

### 8.7 回读（`read` 的第二作用域）

`read` 的边界是 `workspace_root`，而落盘目录在应用数据目录内——对文件工具无伤（落盘的是源文件副本），对 `shell` 是硬伤（命令输出不可重现）。因此打通回读，但**必须带额外许可**：跨出工作区边界不是「工作区内的普通读」。

**（1）路径解析**：`os::path::resolve_readable`（已落地）

判定顺序（**输出域优先于工作区**）：

1. 形态校验与 `resolve` 完全一致（盘符相对路径、含 `..`、不可归一 → 拒绝）；
2. `reject_system_dirs` 照旧（凭据目录在任何作用域内都不可读）；
3. 落在 `context.tool_output_root` 内 → `ReadableScope::ToolOutput`；
4. 否则按 `workspace_root` 判定（既有行为，`Workspace`）。

第 3 步必须先于第 4 步：若输出域与工作区意外重叠（配置失误），路径必须走**更严**的 `ToolOutput` 分支。`read` 的三个调用点（`validate_input` / `permission_intents` / `execute`）必须一起换，少换一处就出现「校验按 A 口径、意图按 B 口径、IO 按 C 口径」。

`tool_output_root` 是运行时字段（`#[serde(skip)]`），**由落盘存储自身派生**（`namespace_root`），不在第二处重算路径。子代理链路（[I38f](./I38f-agent-subagent-spec.md)）的存储根是 `tool-results/subagents/<slug>/`，因此子 turn 的输出可被自己回读、父会话读不到——回读域**跟着存储走**，而不是跟着硬编码根走。

**（2）意图**：`read` 按 `scope` 分流——`Workspace` → `read`（不变），`ToolOutput` → `readToolOutput`，`resource` 取**本会话输出命名空间目录**而非具体文件（一次授权覆盖本会话全部输出；跨会话仍由路径判定拒绝——隔离靠路径，不靠审批兜）。

**（3）策略**：`readToolOutput` 恒 `Ask`，任何审批级别都不放宽（`none` 档的语义是「工作区内免审」，不覆盖跨出工作区的读）；可被会话级授权覆盖（键为 `ApprovalScope::Path { canonical: 输出命名空间 }`）。

**（4）卡片**：`preview_for_approval` 写「读本会话输出 `<文件名>` · 第 N–M 行 · `<human_bytes>`」；`build_approval_context` 只产出一条 `output.scope` 扩展（`Info`），不写绝对路径。

**（5）写方向仍然关闭**：`write` / `edit` / `delete` / `move` 在该域内仍由 `resolve` 的 workspace 边界拒绝；子进程不得写该域（可写根仍是 `workspace_root`）；读过的输出文件不产生可编辑入口。

**（6）调用级约束**：回读仍受 `MAX_READ` 约束，模型用 `offset` / `limit` 分页；**不得**放宽 `MAX_READ`，**不得**让回读结果再落盘（否则「输出文件之输出文件」无限套娃）。

**（7）写入者是父进程**：输出文件由工具进程（沙箱外）创建与写入，子进程只经管道交 stdout / stderr；子进程**不需要**对输出域的写权限，可写根保持单一。

### 8.8 信封投影（`compact`）

`compact` 的 `subject` / `scale` 必须为 `shell` 增加分支，否则 head 退化为无信息量的 `ok shell`：

| 槽位 | `shell` 取值 | 约束 |
|---|---|---|
| `subject` | `arguments.command`（输入事实，不读 `data`） | 换行与连续空白折成单空格，再按 `SUBJECT_MAX_CHARS` 中间省略 |
| `scale` | `exit <code> · <outputLines> lines · <human_bytes(outputBytes)>` | `outputTruncated = true` 时行数后缀 `+`（如 `1200+ lines`），不写「约」「至少」 |

四条约束：`truncated` 只来自 `parse_truncation`；`persisted` 只来自 `ToolResult.persisted`（不得由文本解析）；`anchor` 为 `None`（输出文件不是用户文档）；失败分支沿用通用规则（`subject` 取命令、`reason` 取错误首行）。

---

## 9 失败矩阵

| 编号 | 场景 | 检测点 | 行为 | `error_kind` | 可否改写重试 |
|---|---|---|---|---|---|
| F1 | 无 `writable_root`（ToolBox 入口 / 会话未绑定） | `validate_input` | 拒绝，不弹审批 | `Business` + `sandbox_root_missing` | 否（`blocked`） |
| F2 | 后端探测 `none` | 注册期 | 不注册 `shell` | — | 工具不存在 |
| F3 | 运行期缓存结论为 `none` | `execute` 第 2 步 | 拒绝执行 | `Business` + `sandbox_unavailable` | 否 |
| F4 | 边界建立失败（ACL / ruleset / profile） | `execute` 第 5 步 | 拒绝执行，**不降级** | `Business` + `sandbox_setup_failed` | 否 |
| F5 | 子进程侧限制失败（Unix `pre_exec`） | 子进程 stderr 首行专用前缀 + 非零退出码 | `pre_exec` 失败时**先**向 stderr 写一行 `[sandbox-setup-failed]` 再 `_exit(126)`；父进程只认「前缀 **且** 非零退出码」双条件。仅凭 126 判负会误伤 `sh -c 'exit 126'` | `Business` + `spawn_failed` | 否 |
| F6 | 可执行程序不存在 | `spawn` | 返回失败 | `Business` + `spawn_failed` | 是 |
| F7 | `cwd` 越界 | `validate_input` | 拒绝，不弹审批 | —（`blocked`） | 否 |
| F8 | 参数形状非法 | `validate_input` | 拒绝 | —（`invalid`） | 是 |
| F9 | 超时 | 等待竞争 | 杀树 → 收尸 → `finish()`；**保留已写输出** | `Timeout` | 是（调大 `timeout_ms` 或回读输出） |
| F10 | turn 取消 | 等待竞争 | 杀树 → 收尸 → `finish()`；**保留已写输出** | `Cancelled` | 否 |
| F11 | 输出触达上限 | 写入计数 | 继续读取并丢弃，**不杀树**；结果仍成功 | 无（`ok` + `outputTruncated` / `stderrTruncated`） | 是（收窄输出 / 自行重定向到工作区文件） |
| F12 | 非零退出码 | 正常结束 | 结果成功 | 无（`ok` + `exitCode`） | 是（模型据 stderr 纠错） |
| F13 | 授权键命中但 `enforcement` 已降级 | 键不相等 | 重新走审批 | — | 是 |
| F14 | 输出落盘失败（磁盘满 / 权限） | `open_sink` / `finish` | 失败回填，**不内联全文** | `Business` + `output_spill_failed` | 是 |
| F15 | 回读路径跨出本会话输出域 | `resolve_readable` | 拒绝，不弹审批 | —（`invalid`） | 是（改用回填里的路径） |
| F16 | 回读本会话输出域 | `policy.evaluate` | `readToolOutput` 恒 `Ask`；授权后 `GrantHit` | — | 是（授权一次即可） |
| F17 | 命令内部被沙箱拒绝 | §8.5 判定（正常结束后） | 结果仍 `ok`；`denied = true` + 末尾固定行；**不**升级、不自动重试 | 无（`ok` + `exitCode`） | 是（换落点 / 缩小范围） |
| F18 | 调用级临时目录创建失败 | `execute` 第 3 步 | 拒绝执行（未创建子进程），已建 sink 删除 | `Business` + `scratch_dir_failed` | 是 |

**Windows 侧没有 `pre_exec` 对应物**，F5 的语义由时点决定：令牌 / 受限 SID / Job 配置在 `spawn` 之前完成，任一步失败都是后端返回 `Err`（无子进程产生）→ 按 F4 处理；`spawn` 成功但 Job 分配失败（极少数时序）→ 立刻 `kill_tree` 并收尸，按 F5 判失败。两条路径都**不得**降级为「无沙箱执行」。

三条 fail-closed 底线（F2 / F4 / F5）：**探测失败不注册**、**建立失败不执行**、**限制失败不落地**。禁止任何「先跑起来再补边界」的中间态。

回读侧的第三条底线：**越域不靠审批兜**。跨会话读取必须在路径判定阶段被拒（F15），不能落到「弹个框让用户批准」；审批只回答「要不要放行本会话的输出读取」（F16）。

---

## 10 审计与度量

1. **不改 I50 词表**：`shell` 的审计沿用 `action = "execute"` 与既有 `AuditOutcome`（`Approved` / `Allowed` / `Denied` / `GrantHit` / `Bypassed` 等），不新增 outcome；
2. **隔离强度不进 `permission_audit`**：`enforcement` / `backend` 落在 `ToolResult.data.sandbox` 与审批卡扩展，避免跨阶段改封闭词表；
3. **命令行脱敏**：`resource` 会落库整条命令行，其中可能夹带密钥（`TOKEN=…`、`--password …`）。落库前必须做词法级打码：形如 `NAME=value` 且 `NAME` 命中 §7.4 的敏感词表 → `value` 替换为 `<redacted>`；`--password` / `--passwd` / `--token` / `--api-key` 的**后一个 token** 同样打码。**不对 `-p` 打码**：`docker run -p 8080:80`、`ssh -p 2222`、`curl -p` 里的 `-p` 是端口 / 代理，打码会把审计记录毁成不可读。脱敏只作用于审计与日志，不改变实际执行；
4. **日志固定串**：`backend` / `enforcement` / `network` / `fsRead` / `denied` / `exitCode` / `durationMs` / `outputBytes` / `outputEncoding` / `outputTruncated` 可记；命令行全文与输出正文只记长度。回读审计同样只记路径与范围，不记文件内容。

---

## 11 前端与工具描述

### 11.1 前端与配置

| 项 | 内容 | 落点 |
|---|---|---|
| i18n | `tool_desc.shell`（zh / en）+ 审批卡扩展的 `label` 由后端下发，前端不新增 key | `src/i18n/locales/` |
| 工具装配面板 | 无改动：目录来自注册表（`list_tool_catalog`），后端不注册即不出现 | `src/settings/shared/sections/ToolAssemblySection.tsx` |
| 默认启用 | **默认不勾选**，需用户显式启用（§3.3） | 模式预置 `toolConfig` |
| 审批卡 | 无改动：扩展按 `label` / `value` 通用渲染 | `src/chat/` 审批组件 |
| 落盘展示 | 无改动：气泡仍只展示路径与字节数；P14 **不新增**「打开输出文件」入口 | `src/chat/tool/toolCallDisplay.ts` |
| 不可用提示 | 不注册即不提示（§3.2） | — |

### 11.2 工具描述文案（模型侧唯一的能力说明）

工具描述不是「介绍」，是**契约**：模型不读规格，只读描述。`tool_desc.shell` 的 zh / en 文案**必须**包含以下 6 句，缺一句就是模型会用错的地方：

| # | 必须写明 | 不写的后果 |
|---|---|---|
| 1 | **非零退出码不是工具失败**：结果里带 `exitCode`，按 stderr 纠错重试 | 模型把 `grep` 无匹配（退出 1）当工具坏了，换工具绕路 |
| 2 | **无 stdin / 无 PTY**：交互式命令（`git commit` 不带 `-m`、`vim`、`npm login`、`ssh` 首连）会挂到超时 | 模型写出必挂的命令，白等 120s |
| 3 | **Windows 是 `cmd` 语法**（Unix 是 `sh`）：`Get-ChildItem` / `$env:X` / `$VAR` 不可用；单引号不是引号、`;` 不是分隔符（用 `&`）、`\` 不是转义符。结果 `data.sandbox.shell` 回显本次实际种类，以它为准 | 模型按开发机惯例写 pwsh 或 bash 语法，命令或直接报错、或**静默错误**（§15 R12） |
| 4 | **`cwd` 缺省为工作区根**，且只能落在工作区内；越界路径不会执行 | 模型猜工作目录，或写绝对路径被拒后不理解原因 |
| 5 | **输出会落盘**，结果里给 `outputPath`，需要更多内容就用 `read` 读它（不要重复跑命令） | 模型为看全日志反复重跑同一命令 |
| 6 | **长任务需显式调大 `timeout_ms`**（默认 120s、上限 1800000），超时会杀掉整棵进程树但保留已写输出 | 模型以为超时是环境问题，或不知道可以调大 |

**不写**的：隔离强度 / 可写根（由审批卡扩展与 `data.sandbox` 提供，描述里重复会与真实值冲突）、命令分类（模型无法预知）。文案随实现落地时同步更新 `zh` / `en` 两份，**不得**只改其一（A33）。

---

## 12 实现落点（C5 施工图）

### 12.1 模块形态

```
os/tools/agent_shell/
  mod.rs        ShellTool：id / name / description / input_schema / tier / audience /
                behavior_hints / sandbox_enforcement / validate_input / permission_intents /
                preview_for_approval / execute（只做编排，不写 IO 细节）
  schema.rs     输入解析与校验（§4.1 / §4.2），返回结构化 ShellInput
  classify.rs   CommandClass 纯函数（§5.2）
  spawn.rs      SandboxPlan 推导 + `program` 候选解析 + 环境变量块 + 命令拼装（§7.3 / §7.4）
  run.rs        execute 的 11 步编排、超时/取消竞争、守卫（§7.1 / §7.5）
  deny.rs       沙箱拒绝判定（§8.5，纯函数，可单测）
```

拆分判据沿用 `docs/principle/CODE-SPLIT-PRINCIPLE.md`：`classify.rs` / `deny.rs` 是**纯函数**（可直接单测）；`run.rs` 只做编排；`mod.rs` 只做协议适配。

### 12.2 关键签名（建议形态）

```rust
pub struct ShellTool {
    probe: &'static SandboxProbe,
    storage: Arc<dyn ToolResultStorage>,
}

impl ShellTool {
    /// `probe.enforcement == None` 时不得构造（门禁在注册处，构造期再断言一次）。
    pub fn new(probe: &'static SandboxProbe, storage: Arc<dyn ToolResultStorage>) -> Self;
}

/// 分类结果只用于展示与 P10-b 输入，不参与放行。
pub fn classify(command: &str) -> CommandClass;

/// 纯函数：判定本次是否被沙箱拒绝（§8.5）。
pub fn is_sandbox_denied(
    enforcement: SandboxEnforcement,
    exit: Option<i32>,
    stderr_and_stdout_tail: &str,
) -> bool;
```

### 12.3 测试清单（C5 必带）

| 类别 | 用例 | 对应验收 |
|---|---|---|
| 纯函数 | `classify` 六类样例 + 控制符优先 + 未命中 → `unknown` | A24（间接） |
| 纯函数 | `is_sandbox_denied`：命中关键词、退出码 2 / 126 / 127 不判、`enforcement = none` 不判、`denied` 只置标志不判失败 | A31 |
| 校验 | 空命令 / 超长 / `timeout_ms` 越界 → `invalid`；`cwd` 越界 → `blocked` 且 prompter 计数为 0 | A13 |
| 审批键 | 同命令同 `cwd` 同 `enforcement` 二次调用 → `GrantHit`；命令改一字符 → 重新 `Ask` | A6 / A7 |
| 编码 | UTF-8 / GBK 双流、跨块切分（汉字被 chunk 边界切开）、`GetOEMCP` 分支 | A29 |
| 落盘 | 空输出删文件、60 MiB 截断、stderr 保留区不被进度条挤掉 | A11 / A25 / A26 |
| 终态 | 超时 / 取消各自保留 `outputPath` 且文件可读；正常结束补扫尾 | A12 / A28 |
| 解析 | `program` 候选：`SystemRoot` 正常 → `System32\cmd.exe`；`SystemRoot` 缺失 → 取 `COMSPEC`；`COMSPEC` 指向非 `cmd.exe` / 两变量皆缺 → 判不可用 | A37 |
| 回显 | `data.sandbox.shell` 只写种类、不含路径；审批卡扩展数量仍为 2（§6.5） | A38 |

---

## 13 验收标准

沙箱侧的 A1–A5 见 I53（已归档）§8；下表只列 `shell` 工具侧判据（编号沿用，不重排以免断链）。

| 编号 | 验收项 | 判据 |
|---|---|---|
| A6 | 审批键 | 同命令 + 同 `cwd` + 同 `enforcement` 两次调用：第一次 `Ask`，`AllowSession` 后第二次 `GrantHit`；命令文本差一个字符 → 重新 `Ask` |
| A7 | 降级不复用授权 | 同一键在 `full` 下授权后把 `enforcement` 置 `partial` → 必须重新 `Ask` |
| A8 | `Deny` 先于授权 | 已授权键 + 策略改判 `Deny` → `Denied` |
| A9 | 非零退出码 | `exit 3` → `ok = true` + `exitCode = 3`，模型收到 `exit code 3` 文本 |
| A10 | 输出不截断、只裁剪预览 | 输出 10 万行 → 落盘文件含全部 10 万行，回填为省略标记 + **尾 1200 字符**（无头窗口），`compact.truncated` 为 `None` |
| A11 | 输出触达上限 | 输出超过 60 MiB → `outputTruncated = true`，进程**未被杀**且自然退出，尾部含 `[truncated: output exceeded 60 MiB; total unknown]` |
| A12 | 超时 / 取消 | 两者各自触发，`error_kind` 分别为 `Timeout` / `Cancelled`，无残留进程；**且 `outputPath` 非空、文件保留已写内容、`read` 该路径可读到杀树前的输出** |
| A13 | `cwd` 越界先于审批 | 越界 `cwd` → 审批未被调用（prompter 计数为 0），`blocked` |
| A14 | 审批卡 | `Ask` 时卡片含 `sandbox.enforcement` 与 `sandbox.root` 两条扩展，`summary` 含 `CommandClass` 文案 |
| A15 | 环境变量 | 注入 `MY_API_TOKEN=…` 后子进程看不到；`PATH` / `SystemRoot` 等白名单可见 |
| A16 | 审计脱敏 | `set TOKEN=abc` 落库为 `set TOKEN=<redacted>`，实际执行仍用原值 |
| A17 | 依赖边界 | `assistant-tools` 不依赖 tauri（`cargo tree`） |
| A18 | 工具可见性 | 未启用时模型 `tools` 列表不含 `shell`；启用且探测可用时含 |
| A19 | 落盘与回读闭环 | 输出 10 万行 → `outputPath` 指向 `tool-results/<本会话命名空间>/`；`read` 该路径在授权后成功，内容与文件一致，可分页读到末尾 |
| A20 | 额外许可恒定 | `readToolOutput` 在 `dangerous_only` 与 `none` 两档均为 `Ask`；`AllowSession` 后读同域**另一个**文件为 `GrantHit` |
| A21 | 跨会话隔离 | 读其它会话命名空间路径 → `invalid`，prompter 计数为 0 |
| A22 | 输出域只读 | `write` / `edit` / `delete` 指向输出域 → `blocked`；沙箱内命令重定向到输出域 → 写失败 |
| A23 | 不二次落盘 | `compact.persisted.path` 等于 `data.outputPath`，且目录下只有一份该次调用的输出文件 |
| A24 | 信封投影 | 成功信封 head 为 `ok shell · <command> · exit 0 · N lines · <size>`；`outputTruncated` 时行数带 `+`；`anchor` 为 `None` |
| A25 | stderr 保留区 | `cargo build` 式场景：stdout 写满 60 MiB 后命令失败 → 文件尾部仍是 stderr 末行，回填尾窗口含真实错误行 |
| A26 | 空输出 | `cmd /C exit 0` → `outputPath` 为 `null`、回填 `(no output)`、目录下无 0 字节文件 |
| A27 | 交互式命令不挂死 | `git log`（输出足够触发分页）→ 自然结束，不超时（`PAGER` / `GIT_PAGER` 生效） |
| A28 | 正常结束也扫尾 | 命令派生后台子进程后主进程立即退出（`sh -c 'sleep 300 &'`）→ 调用秒级返回；返回后该后台进程已不存在 |
| A29 | 中文输出可读 | zh-CN 上 `dir` 含中文名目录 / `type` UTF-8 文本 → 落盘与尾窗口均为可读中文（无 `U+FFFD`），`outputEncoding` 分别为 `oem936` / `utf8` |
| A30 | 临时目录在可写根内 | 命令内打印 `tempfile.gettempdir()` → 落在 `<writable_root>/.assistant-tmp/<call_id>`，调用结束后目录不存在 |
| A31 | 沙箱拒绝可见 | 命令内越界写 → `ok = true`、`denied = true`、末尾含固定行；判定函数单测覆盖「2 / 126 / 127 不判」与「`none` 不判」 |
| A32 | AutoRun 不注入 | 注册表 `AutoRun` 写入 `echo INJECTED` → 输出不含 `INJECTED`（`/d` 生效）；测毕恢复原值 |
| A33 | 工具描述契约 | zh / en 文案均含 §11.2 的 6 项约束（断言关键词，防只改一份或漏项） |
| A34 | 真实边界可见 | `data.sandbox` 的 `network` / `fsRead` / `denied` 与探测 `missing` 一致：Windows `network = false`、`fsRead = false`；Linux ABI≥4 `fsRead = true` |
| A35 | 注册门禁不得绕过 | `probe.enforcement == none` 时构造 `ShellTool` 即失败（或注册函数拒绝），且 `shell` 不出现在 `list_tool_catalog` 与模型 `tools` 中 |
| A36 | 参数不可越权 | 模型给出 `shell` / `program` / `env` / `stdin` 字段 → schema 拒绝（`additionalProperties: false`），**不得**静默忽略 |
| A37 | shell 解析 fail-closed | 令 `SystemRoot` 与 `COMSPEC` 同时不可用（单测注入空 env）→ 判探测不可用、`shell` 不注册；**不得**退化为裸名 `cmd` / `sh`。另测 `COMSPEC` 指向非 `cmd.exe` 时被拒 |
| A38 | 方言回显 | `data.sandbox.shell` 为 `cmd`（Windows）/ `sh`（Unix），且**不含路径**；审批卡（§6.5）无该字段；契约生成后前端类型为封闭枚举 |

---

## 14 与批次的关系

批次定义（C1–C8）与任务勾选**只在** [I52 §6](./I52-shell-sandbox-spec.md) 与 [I52 tasklist](./I52-shell-sandbox-tasklist.md) 维护，本文不另立批次表。映射：

| 批次 | 本文对应章节 |
|---|---|
| C5 工具本体 | §3（身份与注册）、§4（输入）、§5（意图/分类/预演）、§7（执行）、§8（输出）、§12（落点） |
| C6 卡片与审计 | §6.5（卡片扩展）、§10（脱敏） |
| C7 前端与配置 | §11（i18n / 默认不勾选 / 描述契约） |
| C8 端到端 | §13（A6–A36） |

---

## 15 风险与开放问题

沙箱侧的开放问题与风险见 I53（已归档）§10；下表只保留 `shell` 工具与审批链，编号沿用 I52 原编号以免断链。

| 编号 | 问题 | 倾向与处置 |
|---|---|---|
| OQ2 | 审批键粒度：整命令行 vs 按 `argv[0]` 归一 | P14 保持**精确命令行**（安全优先）；归一化（只读类按 `argv[0]` 合并）列为 S1 候选，需与 P10-b 一并评估 |
| OQ4 | ToolBox 入口不支持 `shell` 是否最终形态 | 暂定 `AGENT_ONLY`（无工作区即无边界）；若将来 ToolBox 引入显式工作区，再评估放开 |
| OQ6 | 回读复用 `read` 还是新增独立 `read_output` 工具 | 已定论：复用 `read` + `ReadableScope`（§8.7）。独立工具会带来第二套路径校验、第二套审批键与第二套 i18n，收益为零 |
| OQ7 | 回读是否新增「只读中间段」参数 | 不新增：沿用 `read` 的 `offset` / `limit`。输出文件是纯文本，既有分页语义已够 |
| OQ8 | 运行期进度不可见（无 PTY、无 stdin、无中途返回） | P14 接受：模型只看到「running + 取消按钮」，长命令期间拿不到中间输出。三项能力都要求「工具调用可被挂起后再恢复」，而当前 turn kernel 的工具调用是**同步阻塞**的一次性执行，没有第二次进入点。**不新增前端本地计时器**（属前端自造状态，且不解决真问题）。列为 S1 候选，与后台任务一并评估 |
| OQ9 | 交互式命令在无 PTY 下挂起 | 已用强制环境变量压掉主要来源（`PAGER` / `GIT_PAGER` / `GIT_TERMINAL_PROMPT` / `TERM=dumb`）；残余（`npm login`、`vim`）由 `timeout_ms` 兜底为 `Timeout`。不为此引入 PTY |
| R2 | `cmd.exe` 的引号与元字符语义使「审批看到的 argv」≠「实际执行」 | 键基于**整串**而非 argv 语义推断；`CommandClass` 遇控制符即 `unknown`；边界由沙箱承担，不由字符串承担 |
| R3 | 命令级授权导致提示偏多 | 接受：`AllowSession` 对重复命令生效；长期方向见 OQ2 |
| R4 | 沙箱内命令功能受限（缺环境变量 / 网络 / 缓存不可写） | 白名单已覆盖构建类工具；**刻意不注入**工具链路径变量与临时目录。缺失项按「如实报错 + 模型纠错」处理，不扩大白名单到敏感名；构建缓存重定向列为 S1 |
| R5 | 输出文件累积体积（单会话可轻易数百 MB） | 单文件 stdout 60 MiB + stderr 2 MiB 上限 + 30 天清理；不额外做会话配额，触达上限在 `outputTruncated` / `stderrTruncated` 与审计里可见 |
| R6 | 输出域读取被误当成「普通读」而免审 | `readToolOutput` 独立动作名 + 恒 `Ask` + 输出域优先判定，三重各自独立；任一缺失都不构成放行 |
| R7 | 落盘文件被外部删除 / 截断（清理与并发读取竞争） | 回读失败按普通 IO 失败回填（`Business`），不重建、不缓存；清理只删过期会话目录，不删正在写的文件（写入侧持句柄，Windows 上删除会失败并被跳过） |
| R8 | 线程占用与滞留：一个活跃 `shell` 常驻 3 个 blocking 线程；有界 join 放弃时被放弃的读任务会卡在阻塞 `read` 上直到管道关闭 | 接受：`is_concurrency_safe = false` 已限制同轮并行。滞留只发生在「孙进程逃逸整树回收」这一后端缺陷路径上；缓解手段是正常结束也 `kill_tree()` 扫尾，使管道写端在 join 前必然关闭；滞留线程在 `finish` 之后继续读，命中的是已被 `take()` 取走的 sink，只丢弃字节。不在 `shell` 内自建线程或改用异步管道（引入第二套读取模型，收益为零） |
| R9 | 编码判定只在首 4 KiB 做一次：Windows 单条命令混用 OEM 码页与 UTF-8 时另一段含 `U+FFFD`；Unix 非 UTF-8 locale 一律按 UTF-8 解会乱码 | 接受：按行重判会让同一文件内容随读取时机变化（`read` 每次拿到不同结果），比偶发乱码更难排查；`outputEncoding` 让问题可观测（A29）。彻底解法（按 locale 探测 / 逐段记录编码）列为 S1 |
| R10 | 凭据文件可读性取决于沙箱的读拒绝列表：`HOME` / `USERPROFILE` 在白名单内，`~/.npmrc` 能否被 `cat` 到完全看 `sensitive_read_deny` 是否覆盖 | Windows 读侧本就无隔离（`fsRead = false` 已暴露）。**不在本文补第二份路径黑名单**（两份列表必然漂移）；处置是审批卡按 `partial` 展示 + `fsRead` 可见，S1 评估把凭据文件纳入读拒绝列表 |
| R11 | 调用级临时目录残留：崩溃或删除失败时 `.assistant-tmp/<call_id>` 留在工作区（`git status` 可见） | 接受：目录名以 `.` 开头、调用结束即删、删除失败只记日志；同名目录下次调用前复用并清空。彻底解法需改「单一可写根」不变式，列为 S1 |
| R12 | 方言静默失败：模型按 bash 习惯写 `'msg'`（单引号不是引号）、`cmd1; cmd2`（`;` 不是分隔符）、`\"x\"`（`\"` 翻转引号），cmd 不报错但语义不同；pwsh 侧同理（`$VAR` 未定义时**静默展开为空**） | 接受：**不改写命令文本**（§4.3 / §6.1），因此无自动修补手段。缓解是 §11.2 第 3 条（事前）+ `data.sandbox.shell`（事后）；验收只能覆盖显式失败场景，静默错误靠 S1「方言可观测性」补 |
| R13 | `program` 解析依赖 `SystemRoot` / `COMSPEC` 两个环境变量：两者都被裁剪或篡改时工具不可用；`COMSPEC` 指向非 `cmd.exe` 时被拒 | 接受：候选顺序固定、basename 校验、解析失败即不注册（A37）；不改用 `PATH` 探测（§7.3 约束 1） |

---

## 16 参考

### 16.1 本仓文档

- [I52](./I52-shell-sandbox-spec.md)：阶段定义、术语分层与三层职责（本文引用不复述）；
- I53（已归档）：沙箱后端抽象、三平台实现、探测收口与 `kill_tree` 语义；
- [OSCALLDESIGN](../designed/OSCALLDESIGN.md)：Shell 洋葱模型（SafetyBelt / 结构守卫 / 环境隔离 / 命令分类），[I52 §2.3](./I52-shell-sandbox-spec.md) 与本文 §7.4 与其边界划分一致；
- [agent-sandbox-reference-analysis](./agent-sandbox-reference-analysis.md)：Codex / OpenHuman / DSH 的**沙箱后端**实现细节（后端抽象、平台选择、升级阶梯、`kill_tree`）；**shell 种类选择与方言契约**的事实只在 §16.2 与 §16.4 维护，两处不互相复述；
- [I38g](./I38g-agent-approval-advisor-spec.md)：P10 单向顾问与失败矩阵；
- [I38f](./I38f-agent-subagent-spec.md)：子代理输出域；
- I50（已归档）：审计词表（本文不改）。

### 16.2 参考实现的事实

只记做法与位置，不做评价。路径简写：`CX` = `references/1-ai-agent/codex/codex-rs`、`DSH` = `references/1-ai-agent/deepseek-harness/packages`、`OH` = `references/1-ai-agent/openhuman/crates/openhuman-core/src`。

| 主题 | Codex | DSH | OpenHuman |
|---|---|---|---|
| `tool-schema` | `exec_command`：`cmd` / `workdir` / `tty` / `yield_time_ms` / `max_output_tokens` / `shell` / `login` + `sandbox_permissions` / `justification` / `prefix_rule`（`CX/core/src/tools/handlers/shell_spec.rs`） | `bash`：`command` / `description` / `timeoutMs` / `workdir` / `run_in_background` + 同样的 `sandbox_permissions` / `justification`（`DSH/shell/tool-bash/src/index.ts`） | `shell` 仅 3 参：`command` / `category`（只能抬高审批要求）/ `timeout_secs` 1–3600（`OH/tools/impl/system/shell.rs`） |
| `shell-selection` | `ShellType` 五值（zsh/bash/sh/pwsh/cmd）；`derive_exec_args`：zsh/bash/sh 走 `-lc`（`login = true` 时）或 `-c`；pwsh 走 `-NoProfile -Command`（**仅** `login = false` 才加 `-NoProfile`）；cmd 走 `/c`；默认取 `default_user_shell_from_path`——Windows 上**不读 `$SHELL`**，直接探测 PowerShell（`CX/core/src/shell.rs`、`CX/shell-command/src/shell_detect.rs`） | 平台整组换包：POSIX 固定裸名 `bash -c`（非 login）；win32 固定 `[pwshPath, '-NoLogo', '-NoProfile', '-NonInteractive', '-Command', ENCODING_PREAMBLE + command]`（`DSH/shell/bash-local/src/index.ts`、`DSH/shell/pwsh-local/src/index.ts`） | Windows 固定裸名 `cmd /C`（`raw_arg` 直传）；Unix 有 bash 则 `bash -lc`（调用方要求时前置 `set -o pipefail\n`），否则 `sh -lc`（`OH/agent/platform_shell.rs`） |
| `shell-resolution` | `get_shell_path`：`$SHELL`（类型匹配且存在）→ `which` → 常量 fallback 列表（`PWSH_FALLBACK_PATHS = [C:\Program Files\PowerShell\7\pwsh.exe]`、`POWERSHELL_FALLBACK_PATHS = [C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe]`）；全失败返回**裸名**（`cmd.exe` / `/bin/sh`） | `candidatePwshPaths`：`%ProgramFiles%\PowerShell\7\pwsh.exe` → 各 `PATH` 目录下 `pwsh.exe` → `%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe` → 裸 `pwsh`；存在性用 `lstatSync`（Store 别名走 `stat` 会命中目标 ACL 报 `EACCES`）（`DSH/shell/pwsh-local/src/resolve.ts`） | `bash_path()`：`/usr/bin/bash` → `/bin/bash`，`OnceLock` 缓存，否则退回 `sh`；Windows 恒裸名 `cmd`（`OH/agent/platform_shell.rs`） |
| `shell-dialect` | 一个工具、多方言：`shell` 参数**只用于判定类型**，可执行文件仍自行探测（`get_shell_by_model_provided_path` 注释 "uses the model-provided path only to select a shell type, then discovers its executable"）；当**执行环境**平台为 Windows 时拼入 `windows_shell_guidance`（`CX/core/src/tools/handlers/shell_spec.rs`、`CX/core/src/tools/spec_plan.rs` 的 `should_include_windows_shell_guidance`） | **两个工具、不互译**：`bash` 与 `pwsh` 各带方言契约，`cordis.patch.yml` 按 `process.platform` 整组启停；`tool-pwsh/README.md` 原文 "there is no dialect translation" | 单一方言：Windows cmd、Unix sh/bash；无方言参数、无 `login` 参数 |
| `login-profile` | `login` 默认 `true`：POSIX 走 `-lc`，PowerShell 默认**不加** `-NoProfile`（即加载 profile）；另有 shell 快照抓取 + `derive_exec_args_with_path_prepends` 把执行环境目录回注 `PATH`（`CX/shell-command/src/shell_detect.rs`） | 恒不加载：`bash -c`（非 login）、`pwsh -NoProfile` | Unix 恒 `-lc`（login）；Windows 无此概念 |
| `sandbox-coupling` | Store 版 PowerShell 位于 `WindowsApps` 下、受限令牌不可读 → `fallback_powershell_shell_for_windows_sandbox` 在沙箱需要时换成未打包路径；`ShellInfo.path` 注释明写 fallback 如 `cmd.exe` **不必是绝对路径**（命令可能跑在附加 / 远程执行环境） | `pwsh-sandbox` 只在 win32 装载、`bash-sandbox` 只在非 win32；Windows 上被强杀结算为 exit 1 且无 signal，故 `[killed by signal: …]` 是 POSIX-only | jail 是可选路径；`wrap_with_output_redirection` 负责把 `> "out" 2> "err"` 拼进命令行（Windows 靠 `/C` 尾部重定向绑定整段，Unix 靠 `{ … ; }` 分组），并依赖 `raw_arg` 避免 MSVCRT 引号转义 |
| `approval` | `SandboxPermissions` 三值；`justification` 必须配显式 `sandbox_permissions`；`prefix_rule` 仅升级模式可用（`CX/protocol/src/models.rs`） | `sandbox_permissions` 与 `justification` 必须成对、非空，违规报 `SANDBOX_ESCALATION_INVALID_ARGUMENT`；只允许向更宽方向升级（`DSH/sandbox/sandbox/src/escalation.ts`） | `classify_command` → `gate_decision(class) == Prompt` 才提示；模型自报 `category` 只取 `max()`（`OH/tools/impl/system/shell.rs`） |
| `timeout` | 默认 10 000 ms；`EXEC_TIMEOUT_EXIT_CODE = 124`；取消后终止宽限 50 ms、IO 抽取超时 2 s（`CX/core/src/exec.rs`） | 默认 120 000 ms，上限 600 000 ms（`clampTimeout`），`onExpiry` 默认 `kill`（`DSH/shell/bash-local/src/index.ts`） | **无默认超时**（缺省或 0 → `Unbounded`），上限 3600 s，沙盒路径用 86 400 s 代替无界（`OH/tools/impl/system/shell.rs`、`OH/tools/timeout/mod.rs`） |
| `cancellation` | `CancellationToken` + `ExecExpiration` 三态 `select`；TERM → 宽限 → SIGKILL，按进程组杀（`CX/core/src/exec.rs`） | `deadline(signal, timeoutMs, 'BASH_TIMEOUT')`；SIGTERM → SIGKILL 宽限 3 000 ms（`DSH/shell/bash-local/src/index.ts`） | 仅 `tokio::time::timeout(...)` 包住 `cmd.output()`，未见进程组 kill 或信号升级 |
| `output-limit` | 1 MiB，**截中间**（保留头尾）；unified_exec 另有 1 MiB / `max_output_tokens = 10 000`（`CX/utils/output-truncation/src/lib.rs`） | 前台 64 000 字节/流，**保留尾部**；工具描述写明 "truncated to its tail" | 1 MiB，**截尾部**并追加 `... [output truncated at 1MB]`；进历史前再按 30 000 字符限一次 |
| `output-storage` | 未见溢出落盘通道（截断即丢） | 每流溢出到临时文件，`maxSpillBytes = 64 MiB`，结果带 `spillPath`（`DSH/shell/bash-local/src/index.ts`） | 未见（`MAX_OUTPUT_BYTES` 后直接截断） |
| `encoding` | PowerShell 前置 `[Console]::OutputEncoding = UTF8`（`CX/shell-command/src/powershell.rs`） | pwsh 每条命令前拼 `ENCODING_PREAMBLE`：`[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); $OutputEncoding = …; `——注释写明「为 5.1 的 OEM 码页兜底，pwsh 7 默认 UTF-8 不受影响」，并刻意用 `; ` 留在**第 1 行**以免报错行号偏移（`DSH/shell/pwsh-local/src/index.ts`） | 未见转码：cmd 侧靠 `raw_arg` + `>` / `2>` 做**字节透明**重定向（PowerShell 5.1 的 `>` 写 UTF-16LE 正是被排除的理由），解码侧未处理 OEM 码页 |
| `env-vars` | 策略式：`ShellEnvironmentPolicy`（inherit 三态 + 默认排除 `*KEY*` / `*SECRET*` / `*TOKEN*` + 不可继承名单）；启动时抓 shell 快照并 source 回来（`CX/protocol/src/shell_environment.rs`） | 继承父进程后叠三层覆盖，`ENV_OVERRIDES = {NO_COLOR:1, TERM:dumb, PAGER:cat, GIT_PAGER:cat}`；凭据清洗由 subprocess 服务独立负责 | `env_clear()` + 23 项白名单；`TMPDIR` / `TMP` / `TEMP` 改指 scratch 目录（`OH/tools/impl/system/shell.rs`） |
| `cwd` | `workdir` 可选，缺省取 turn cwd | `workdir` 可选，缺省 `config.cwd` → `process.cwd()` | **模型不能传 cwd**，由 `effective_action_dir_for_context()` 决定 |
| `denial-detection` | `is_likely_sandbox_denied`：先排除 `[2,126,127]`，`128 + SIGSYS` 判 seccomp，否则 7 条关键词匹配（`CX/sandboxing/src/denial.rs`） | 统一拒绝词 `[sandbox: file access denied under <mode> mode]`；另产出 `[sandbox: escalation available …]` 提示行 | 事后按错误文本分类：先认策略标记，再 timeout，再 policy 关键词，OS 权限词归 `MissingPermission`（`OH/tools/status/ops.rs`） |
| `background-job` | 同一条 `exec_command` 即会话：未跑完返回 session ID，`write_stdin` 轮询/写入；进程上限 64，后台等待上限 300 s | job registry：`run_in_background` 立即返回 jobId，`promoteOnTimeout = true` 让前台超时的命令转后台继续跑，另有 `job_output` / `job_list` / `job_kill` | 未见后台入口 |

### 16.3 关键分歧与本规格的取舍

| 分歧点 | 各家做法 | 本规格 | 理由 |
|---|---|---|---|
| 默认超时 | 10 s（Codex）/ 120 s（DSH）/ 无默认（OpenHuman） | **120 s，上限 1800 s** | 本仓是 Rust 工作区，10 s 会让常规构建必然超时；无默认则挂死无兜底。上限放宽到 30 min 只改单次占用，不改并发（§15 R8） |
| 前台超时之后 | 转后台继续跑（DSH `promoteOnTimeout`）/ 杀进程组（Codex）/ 仅 timeout（OpenHuman） | **杀树 + 保留已写输出** | P14 无后台任务（§1.2）：转后台要求把进程归属从调用栈上移，属 P9（[I52-Ext](./I52-ext-background-task.md)）；保留输出让超时退化为「有现场可查」 |
| 输出策略 | 内存截断：1 MiB 截中间（Codex）/ 64 KB/流 截尾（DSH）/ 1 MiB 截尾（OpenHuman） | **落盘 60 MiB + 2 MiB stderr 保留区 + 尾窗口预览** | 截断会让「日志很大但合法」被判失败；落盘消掉内存放大，stderr 保留区保证错误行不被进度条挤掉（§8.1） |
| login shell / source profile | Codex 默认 `login = true`（PowerShell 默认加载 profile，另有 shell 快照抓取）；OpenHuman Unix 恒 `bash -lc`；DSH 两者都**不**加载（`bash -c`、`pwsh -NoProfile`） | **不 source、非 login** | 同一命令在不同机器上行为不同，与「命令效果只由沙箱与命令文本决定」冲突；代价由白名单 env 与绝对路径兜底（§7.7） |
| shell 可执行文件如何解析 | 有序候选 + 存在性探测（三家一致），差别只在候选集与最后兜底：Codex 兜底为裸名，DSH 兜底为裸 `pwsh`，OpenHuman 兜底为 `sh` | **有序候选**：`%SystemRoot%\System32\cmd.exe` → `%COMSPEC%`（校验 basename），全失败即不注册 | 采纳「候选」这一解析方式、不采纳「种类探测」；候选集不含 `PATH` 与工作区（§7.3） |
| Windows 方言选哪个 | Codex 默认 PowerShell（另留 `shell` 参数可切）；DSH 用 pwsh 并另设 bash 工具；OpenHuman 固定 cmd | **cmd**（带 `/d` 抑制 AutoRun） | 重定向字节透明、系统自带且路径确定；pwsh 的 profile / 编码 / 引号规则都更重，另立工具列为 S1（§7.7） |
| 环境变量来源 | 继承 + 排除（Codex/DSH）/ `env_clear` + 白名单（OpenHuman） | **白名单 + 强制注入** | 继承会把 `*TOKEN*` 一类凭据带进子进程；白名单的漏项按「如实报错 + 模型纠错」处理（§15 R4） |
| 审批升级通道 | Codex `sandbox_permissions` 三值 + `prefix_rule`；DSH `WIDER_MODES` + escalation 提示行 | **不提供**：三档是探测结论，不是可申请档位 | 没有更宽档位可升；因此 §8.5 只给拒绝标记、不给 escalation hint（否则模型会反复尝试不可能的升级） |
| 后台任务原语 | session + `write_stdin`（Codex）/ job registry（DSH） | **不做**，登记 P9 | 见 §1.2 与 [I52 §8](./I52-shell-sandbox-spec.md)：判据是「进程归属由谁持有」，不是加参数 |

### 16.4 shell 选择与方言：逐家展开

§16.2 的 `shell-selection` / `shell-resolution` / `shell-dialect` / `login-profile` 四行压缩过度，本节展开，回答三个问题：**它们为什么这样选**、**Windows 端具体怎么落地**、**哪些做法本文不采纳**。沙箱后端的事实见 [agent-sandbox-reference-analysis](./agent-sandbox-reference-analysis.md)，此处不重复。

#### 16.4.1 DSH：平台整组换包，没有运行时种类探测

- **种类由部署层决定**：`bundle/base/cordis.patch.yml` 里 `bash-sandbox` / `tool-bash` 标 `disabled: !!js process.platform === 'win32'`，`pwsh-sandbox` / `tool-pwsh` 标 `disabled: !!js process.platform !== 'win32'`；`shell/README.md` 把这条写成原则：「A profile layer selects exactly one executor implementation (the win32 layer swaps the POSIX rows for the pwsh ones; mounting two fails loudly at load time on the duplicate service registration)」。一个进程里不可能同时存在两种方言的执行器，因此**不存在「该写 cmd 还是 PowerShell」的歧义**。
- **Windows 端具体形态**：`pwsh-local` 的 `argv()` 返回 `[pwshPath, '-NoLogo', '-NoProfile', '-NonInteractive', '-Command', ENCODING_PREAMBLE + command]`。命令**整体作为一个 argv 元素**交给 `-Command`，模块头注释写明「no intermediate shell exists, so there is no shell-quoting layer to escape」——这是不走 `cmd` 路径时避免引号二次解析的做法。
- **路径解析是纯函数**：`resolvePwshPath(configured, env, platform)` 可注入 `env` 单测；顺序为 PowerShell 7 安装目录 → `PATH` 各项 → Windows PowerShell 5.1 → 裸 `pwsh`；用 `lstatSync` 而非 `statSync`，因为 Store 的 app execution alias 在 `stat` 上会命中目标 ACL 报 `EACCES`。
- **方言不翻译**：`tool-pwsh/README.md` 原文「PowerShell-dialect contract — the model must write PowerShell (native paths, `$env:` variables), not bash; **there is no dialect translation**」。代价被接受为「两个工具、两套描述、两套测试」。
- **环境变量按平台分化**：bash 侧 `ENV_OVERRIDES = {NO_COLOR, TERM: 'dumb', PAGER, GIT_PAGER}`；pwsh 侧**刻意去掉** `TERM`（注释：`TERM=dumb` is a POSIX concept）。
- **退出状态语义**：Windows 上被强杀结算为 exit 1、无 signal，因此 `[killed by signal: …]` 是 POSIX-only——与本文 §8.4「Windows 恒有 `exitCode`、`signal` 恒 `null`」是同一事实，两家独立得出。

#### 16.4.2 Codex：探测用户 shell + 模型可覆盖

- **默认取用户 shell，但 Windows 上不读 `$SHELL`**：`default_user_shell_from_path` 在 Windows 分支直接 `get_shell(PowerShell)`（pwsh 7 → 5.1 → fallback 列表），全失败才 `ultimate_fallback_shell()`（裸名 `cmd.exe`）；Unix 分支读 `$SHELL` 并匹配类型，macOS 兜 zsh、Linux 兜 bash，最后 `/bin/sh`。
- **`login` 默认 `true`**：`derive_exec_args` 里 POSIX 走 `-lc`；PowerShell 分支只在 `!use_login_shell` 时才加 `-NoProfile`——即**默认加载用户 profile**。此外有 shell 快照与 `derive_exec_args_with_path_prepends`（login 之后把执行环境目录回注 `PATH`）。
- **模型给路径、系统给类型**：`get_shell_by_model_provided_path` 的注释是「Uses the model-provided path only to select a shell type, then discovers its executable」——模型不能塞任意可执行文件，但**能选择方言**。
- **方言混淆是已知问题**：`windows_shell_guidance` 在**执行环境**平台为 Windows 时拼进工具描述（`should_include_windows_shell_guidance` 读 `executor_platform_os`，不是宿主 `cfg!(windows)`——命令可能跑在附加 / 远程环境），首条即「Do not enumerate paths in PowerShell and then pass them to `cmd /c` … Use one shell end-to-end」。这是「shell 可切」的直接代价，也是本文 §11.2 第 3 条的同源动机。
- **沙箱反向约束 shell**：Store 版 PowerShell 在受限令牌下不可读，`fallback_powershell_shell_for_windows_sandbox` 只在沙箱需要时把它换成未打包路径（`which_all` 逐个过滤 + fallback 列表），正常探测顺序不变。
- **路径不必是绝对路径**：`ShellInfo.path` 注释「Fallbacks such as `cmd.exe` need not be absolute, so this is not a `PathUri`」——因为命令可能跑在**附加 / 远程执行环境**，shell 是那个环境上报的事实（`from_environment_shell_info` 按 `name` 反解 `ShellType`）。

#### 16.4.3 OpenHuman：固定平台 shell，且把理由写进模块头

- **唯一一家把「为什么不是 PowerShell」写成代码注释**：`%VAR%` 展开符合 Windows 用户与模型预期；`>` / `2>` 需要**字节透明**重定向供输出捕获路径使用；而 PowerShell 5.1 的 `>` 写 UTF-16LE 且不展开 `%VAR%`。
- **`raw_arg` 是这条理由的实现要求**：`cmd.arg("/C").raw_arg(command)`——`arg` 会按 MSVCRT 规则把内部 `"` 转成 `\"`，而 `cmd` 只认裸 `"`，结果是 `>` / `2>` 落进引号区间、捕获文件永不生成。这与本文 §4.3「不改写命令文本」同源，只是它写在 spawn 层。
- **Unix 侧反而用 login shell**：`bash -lc`，并在调用方要求时前置 `set -o pipefail\n`；理由是管道中段失败会被最后一段掩盖，导致「反复失败但不触发熔断」。`/bin/sh` 在 Debian / Ubuntu 是 dash、拒绝 `set -o pipefail`，故先探测 bash（`OnceLock` 缓存 `/usr/bin/bash` → `/bin/bash`）。
- **无方言参数、无 `login` 参数、模型不能传 cwd**：选择权全部在服务端，模型只能写 `command` / `category` / `timeout_secs`。

#### 16.4.4 三家共同点与本文的取法

| 观察 | 三家事实 | 本文取法 |
|---|---|---|
| shell 路径如何得到 | **都是有序候选 + 存在性探测**，没有一家用编译期常量 | 采纳（§7.3），但候选集只含系统目录与 `COMSPEC` |
| 谁决定 shell 种类 | Codex 模型可指定；DSH 部署层；OpenHuman 服务端常量 | 服务端常量（§4.1 / §7.7），理由见 §16.3 |
| 是否加载 profile | Codex 默认加载；OpenHuman Unix 加载；DSH 不加载 | 不加载（§7.7） |
| 方言混淆如何处置 | Codex 写 Windows 指引；DSH 拆两个工具；OpenHuman 只写 cmd 方言 | 一个工具 + §11.2 第 3 条 + `data.sandbox.shell` 回显 |
| 编码 | Codex / DSH 前置改写命令；OpenHuman 靠字节透明重定向 | **不前置改写**，首 4 KiB 探测 + 落盘转码（§8.3） |

**为什么采纳「候选」而不采纳「种类探测」**：三家的路径探测都只发生在**本机文件系统**上，结果只影响「跑的是哪一个同名 shell」；而种类探测（Codex `$SHELL`、模型 `shell` 参数）会影响**命令语法**与**审批键语义**。前者是解析细节，后者是契约变化——本文只跟前者。

**方言失败的两种形态**（直接决定本文的处置方式）：

| 形态 | 典型例子 | 是否可自愈 | 本文处置 |
|---|---|---|---|
| 显式失败 | `Get-ChildItem` 在 cmd 里报「不是内部或外部命令」 | 可：模型读 stderr 一次即改 | §11.2 第 1 / 3 条 + `data.sandbox.shell` |
| 静默错误 | `git commit -m 'msg'`（单引号不是引号）、`a; b`（`;` 不是分隔符）、`echo \"x\"`（`\"` 翻转引号）、pwsh 的 `$VAR` 未定义展开为空 | **不可**：退出码 0，模型看不到错 | 只能事前预防；**禁止**自动修补（会改写命令文本、污染审批键，§4.3）；登记 §15 R12 |

两者频率上并不对称：显式失败频率更高但代价一次纠错；静默错误频率低但会直接产出**看似成功的错误结果**——这正是本文宁可在工具描述里多写一行方言约束、也不做「按平台改写命令」的原因。
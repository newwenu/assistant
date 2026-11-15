# I52 — P14：沙箱执行层与 `shell` 工具规格

| 项 | 值 |
|---|---|
| 状态 | 草稿（P14 实施规格；进度只在 [I38 roadmap](./I38-agent-roadmap.md) §2 维护） |
| 日期 | 2026-11-15 |
| 阶段 | P14（工具能力轴；见 [I38 roadmap](./I38-agent-roadmap.md) §2 / §3） |
| 关联 | [I53](./I53-sandbox-preparation-spec.md)（沙箱准备：后端抽象 / 链式探测 / 三平台实现；本文 §3–§4 已拆出，本文只引用不复述）、I18 design §3.17（已归档）（审批链路预留与检测分层）、[OSCALLDESIGN](../designed/OSCALLDESIGN.md)（Shell 洋葱模型，执行层边界）、[沙箱开源调研](./agent-sandbox-research.md)（后端选型与 ADR-1–ADR-4）、[沙箱参考分析](./agent-sandbox-reference-analysis.md)（Codex / OpenHuman / DSH 实现细节）、[I38g](./I38g-agent-approval-advisor-spec.md)（P10 顾问与本阶段的三个接入点）、I50（已归档）（审计口径；本文**不改**其封闭词表） |
| 影响范围 | `assistant-tools`（`os/agent_shell.rs`、`trait.rs` 新增一个方法、`gateway.rs` 键字段与求值顺序、输出 sink 与回读作用域）；`src-tauri/src/adapters/tool_exec.rs`（输出域注入）、前端 i18n 与模式预置。沙箱模块、`tool_setup.rs` 探测收口与 `Cargo.toml` 依赖变更见 [I53](./I53-sandbox-preparation-spec.md) |

> 一句话：本阶段把「审批」从**纸面声明**变成**可执行边界**。审批回答「要不要做」，沙箱回答「最多能做到哪」；
> `shell` 让 `execute` 意图第一次真正出现在生产链路上，从而让审批链（含 P10 顾问）第一次有真实消费方。

本文中的「必须 / 禁止」为可验收约束；「应当」为默认做法，偏离需在代码注释里写明理由。

**编号约定**：调研文档 [agent-sandbox-research](./agent-sandbox-research.md) 用 `P0 / P1 / P2` 指沙箱**实施批次**，与 roadmap 的阶段号 `P0–P14` 同形易混。本文一律写 **S0 / S1 / S2**，映射为 P14-a / P14-b / P14-c，见 §11。

---

## 1 目标与非目标

### 1.1 现状（以代码为准）

| # | 事实 | 落点 |
|---|---|---|
| N1 | `ApprovalScope::Command { argv, cwd, sandbox_tier }` 与 `split_argv` / `command_cwd` 已实现，但**无生产者**：`execute` 意图只出现在 `policy.rs` 的单测里 | `assistant-tools/src/gateway.rs` |
| N2 | `ToolTier` 三档只用于装配过滤与 tier 上限，**不产生任何运行时隔离** | `assistant-tools/src/trait.rs`、`policy.rs` |
| N3 | `exec::invoke` 的统一超时对子进程无效（源码注释已写明），子进程类工具必须自管超时 + kill | `assistant-tools/src/exec.rs` |
| N4 | 进程输出解码（`decode_process_output`）已就绪可复用；`truncate_output` **不再用于 `shell`**（改为落盘 + 窗口预览，§5.9 第 2 条） | `assistant-tools/src/os/output.rs`、`os/text_encoding.rs` |
| N5 | `ToolUseContext.workspace_root` 是唯一权限边界，P12 已改为按会话解析 | `assistant-tools/src/context.rs`、`src-tauri/src/tool_setup.rs` |
| N6 | `ApprovalExtension { kind, label, value, severity }` 是开放出口，新增 `kind` 不需要 wire / DTO 变更 | `assistant-tools/src/gateway.rs`、`protocol/dto/agent.rs` |
| N7 | `I38c §13`（已归档）记录 `shell` 暂不提供，并给出三条恢复条件 | 已归档，只作历史参照 |
| N8 | `OsToolResultDto` 由 `From<ToolResult>` **逐字段显式映射**（`ok` / `data` / `result_for_assistant` / `error` / `error_kind`），`data` 是 `#[ts(type = "unknown")]` 开放 JSON | `src-tauri/src/protocol/dto/os_tool.rs` |

### 1.2 目标

1. 交付一个**可验收的边界**：可写根之外的写入必须失败、进程树必须可被整体回收、敏感目录不可读（后端实现与判据见 [I53](./I53-sandbox-preparation-spec.md)）；
2. 交付 `shell` 工具：`execute` 意图、命令级审批键、进程级超时、输出落盘与回读、结果可被模型纠错；
3. 让审批卡片如实回答「这次执行被什么边界包着」（隔离强度 / 可写根 / 网络可达）；
4. 让 P10 的三个接入点有稳定事实源（`execute` 恒 `Ask` 的消费方、命令分类器、内核隔离已就绪）；
5. 全程 fail-closed：**不存在「探测失败就无沙箱执行」的路径**；
6. 让工具输出**可回读**：输出落到应用管理的同一存储，模型按引用路径回读，消除「落盘即失联」（§5.11）。

### 1.3 非目标

- 不新增命令字符串黑名单（§2.3：字符串匹配不是边界，禁止作为拦截依据）；
- 不替代审批：沙箱不改变 Policy/Gateway 的决策（§2.3、§6.5）；
- 不做容器 / microVM / 内核驱动级后端（虚拟化层 L3+，见 §2.1），不做云端沙箱；
- 不做 `shell` 的交互式会话（PTY / stdin 注入 / 长驻进程），一次调用 = 一次执行；
- **不做后台任务**（异步执行 / 任务 id / 状态机 / 轮询回收）：后台任务的发起点只能是 turn kernel 或 app supervisor——它们决定「一次工具调用何时算结束」与「进程归谁回收」，而 P14 的改动面是 `assistant-tools` 的工具实现与审批链，这两处都不在其中。故不在 P14（属独立轴，与 P9 持久化运行同类）；运行期无进度见 §12 OQ8；
- 不做命令级授权归一化（「同 `argv[0]` 合并键」是 S1 候选，见 §12 OQ2）；
- 不开放跨会话输出读取：回读只作用于**本会话**输出命名空间（§5.11.4）；
- 不改 I50 的 `AuditOutcome` 封闭词表（隔离强度走工具结果与审批卡，不进审计词表，§8）。

### 1.4 与 `I38c §13`（已归档）恢复条件的对照

| 恢复条件 | 本阶段结论 |
|---|---|
| 具备真正的 OS 级隔离（而非仅字符串守卫） | **满足**：[I53 §5](./I53-sandbox-preparation-spec.md) 三个平台后端均为系统强制（Token+ACL / Landlock / Seatbelt） |
| 或具备结构化执行（不经过 shell 解释器） | **不采用**：模型需要的是通用命令能力；结构化执行会把能力砍到白名单，且仍无法限制子进程行为 |
| 或具备可验证的 dry-run | **部分满足**：`preview_for_approval` 只报「命令类别 + 隔离强度 + 规模未知」，不承诺 dry-run 语义（§5.6） |

结论：本阶段满足条件 1，`shell` 可以恢复；条件 2 明确不采用并记录理由。

---

## 2 术语与分层（唯一出处）

### 2.1 两根轴：检测层 vs 虚拟化层

「L2 / L3 / L4」在本仓文档里被两套轴同时使用，必须写清是哪根轴：

| 轴 | 取值 | 出处 | 含义 |
|---|---|---|---|
| **检测层** | L2 字符串/规则匹配 → L3 解析后判定（`canonicalize` / argv）→ L4 内核强制 | I18 design §3.17.1（已归档）、[参考分析 §5.4](./agent-sandbox-reference-analysis.md) | 判定「这次操作到底碰了什么」的手段强度 |
| **虚拟化层** | L1 OS 原生虚拟化（Token+Job / Landlock / Seatbelt / bwrap）→ L2 LSM / seccomp → L3 用户态内核（gVisor）→ L4 硬件虚拟化 | [沙箱调研 §2–§7](./agent-sandbox-research.md) | 隔离由谁提供 |

**约束**：本文及后续文档禁止裸写「L4 沙箱」，必须写「检测层 L4」或「虚拟化层 L1/L2」。I18 design §3.17.6 的 `Adjudicate` 门禁指的是**检测层 L4**（内核强制隔离），由本阶段的虚拟化层 L1/L2 后端关闭（§6.6）。

### 2.2 封闭词汇

沙箱三词（`SandboxKind` / `SandboxEnforcement` / `SandboxDimension`）的唯一定义已移至 [I53 §3](./I53-sandbox-preparation-spec.md)，本文只使用不定义。本文自有词汇：

| 词 | 定义 | 取值 |
|---|---|---|
| `CommandClass` | 命令类别（静态分类器输出） | `readOnly` / `mutating` / `network` / `destructive` / `unknown` |
| `ReadableScope` | `read` 的路径归属域（§5.11.4） | `workspace` / `toolOutput` |
| `readToolOutput` | 读取本会话输出域的权限动作（§5.11.4） | 恒 `Ask`，独立于审批级别 |

### 2.3 三层职责，互不替代

| 层 | 回答的问题 | 归属 | 失败方向 |
|---|---|---|---|
| Policy | 「这个意图按规则该不该问」 | `policy.rs` | 未知 → `Ask` |
| 审批 | 「用户要不要放行这一次」 | `gateway.rs` + 审批卡 | 无审批通道 → `Deny` |
| 沙箱 | 「放行之后最多能做到哪」 | [I53](./I53-sandbox-preparation-spec.md)（本文只消费其结论） | 无边界 → 不注册工具 |

三条硬规则：

1. **沙箱不得改变审批决策**：`execute` 在 `dangerous_only` 档恒为 `Ask`（`base_rules` 现状），在 `none` 档恒为 `Allow`；沙箱不参与求值，只在执行阶段生效。
2. **审批不得依赖沙箱结论放宽**：审批键里携带隔离强度（§6.2）是为了「降级后不复用授权」，不是为了「有沙箱就免审」。
3. **字符串匹配不得作为拦截依据**：`shell` 的 `validate_input` 只校验参数形状与 `cwd` 边界，不解析命令语义；命令分类器只用于展示与 P10-b 输入（§5.5）。

---

## 3 沙箱准备（拆分为独立规格）

沙箱后端抽象、`SandboxPlan` 推导、链式探测与注册期收口、`enforcement` 判据、三平台后端实现、固定敏感目录、依赖变更与「不做的事」的**唯一来源**是 [I53 沙箱准备规格](./I53-sandbox-preparation-spec.md)。本节不再复述。

本文只消费其中三条结论（写入 I53，此处不重复判据）：

1. 探测结论 `enforcement == none` → **不注册** `shell`（I53 §4.4），运行期再读缓存仍为 `none` 则拒绝执行；
2. `SandboxPlan` 的 `writable_root` / `program` / `network` **不可**由模型输入指定（I53 §4.3）；
3. `SandboxChild::kill_tree` 必须回收**整棵**进程树且可重复调用（I53 §4.2、§6）。

## 4 平台后端（拆分为独立规格）

Windows（Token + ACL + Job）/ Linux（Landlock）/ macOS（Seatbelt）三个后端的部件、时序、敏感目录列表与依赖变更，全部见 [I53 §5](./I53-sandbox-preparation-spec.md)。

沙箱准备与清理的幂等要求见 [I53 §6](./I53-sandbox-preparation-spec.md)；沙箱侧失败矩阵与验收见 [I53 §7 / §8](./I53-sandbox-preparation-spec.md)。

---


## 5 `shell` 工具

### 5.1 身份

| 项 | 值 | 理由 |
|---|---|---|
| `id` / `name` | `shell` | 白名单、审计、报错前缀同源 |
| `tier` | `Os` | 创建 OS 进程；不受 `max_tool_tier = sandbox` 放行 |
| `audience` | `AGENT_ONLY` | ToolBox 入口不解析工作区（P12），拿不到 `writable_root` → 必然失败，故不暴露 |
| `behavior_hints` | `read_only = false`、`destructive = true`、`open_world = true` | 静态声明取最坏值（该方法是纯静态、无入参）；逐命令的真实类别由 §5.5 表达 |
| `is_concurrency_safe` | 恒 `false` | 进程执行会争用资源，且输出无法与其它工具调用交错 |
| `manages_own_execution_timeout` | `true` | 子进程必须自管超时 + kill（`exec.rs` 注释的硬约束） |

`behavior_hints` 取最坏值会带来一个副作用：审批卡 L0 恒显示「破坏性 / 可外连」。这是**有意**的——卡片另有 §5.5 的 `CommandClass` 扩展给出这一次的真实类别，两者不合并。

### 5.2 输入 schema

| 字段 | 类型 | 必填 | 约束 |
|---|---|---|---|
| `command` | string | 是 | 去首尾空白后非空，长度 ≤ 8192 字符 |
| `cwd` | string | 否 | 相对路径按 `writable_root` 解析；解析后必须落在 `writable_root` 内 |
| `timeout_ms` | integer | 否 | 1000–600000；缺省 120000 |

无 `shell` / `program` / `env` / `stdin` 字段：可执行程序与可写根都不接受模型输入（[I53 §4.3](./I53-sandbox-preparation-spec.md)）。`additionalProperties: false`。

### 5.3 `validate_input`

按 Pipeline 的 `find → validate → gateway → execute` 顺序，本步骤**先于审批**，避免为非法输入弹框：

1. `command` 形状（非空 / 长度）→ 失败 `invalid`（可改写重试）；
2. `cwd` 解析 + `ensure_within_root` → 越界时 `blocked`（`blocks_input_rewrite = true`：换一个路径重试同一意图不构成新意图）；
3. `timeout_ms` 范围 → `invalid`；
4. `workspace_root` 缺失 → `blocked`，`error_code = "sandbox_root_missing"`。

**禁止**在本步骤做命令内容匹配（§2.3）。

### 5.4 `permission_intents`

返回单条 `PermissionIntent { action: "execute", resources: [command] }`。

- 一条命令 = 一条意图 = 一个审批键；不把命令行拆成多条意图（拆了反而会让「`A && B` 只批了 A」的错觉成立）。
- 审批键由既有 `ApprovalKey::from_intent` 生成：`ApprovalScope::Command { argv, cwd, sandbox_tier, enforcement }`（`enforcement` 为 §6.2 的新增字段）。
- `split_argv` 对未闭合引号返回 `None` → 空键 → 不查缓存直接走审批（既有 fail-closed 行为，不改）。

### 5.5 命令分类器（`CommandClass`）

纯函数，无 IO，输入为 `command` 字符串；**只用于展示与 P10-b 输入**，不参与任何放行决策。

判定顺序（首个命中即返回）：

| 顺序 | 条件 | 结果 |
|---|---|---|
| 1 | 含 shell 控制符（`|`、`&&`、`||`、`;`、`>`、`<`、`` ` ``、`$(`、换行） | `unknown` |
| 2 | `argv[0]` 与子命令命中只读清单（`git status` / `git log` / `git diff` / `git show`、`ls` / `dir`、`cat` / `type`、`rg` / `grep` / `find` / `where`、`head` / `tail` / `wc`、`pwd`、`echo`、`--version` 类） | `readOnly` |
| 3 | 命中网络清单（`curl` / `wget` / `ssh` / `scp`、`npm install` / `pip install`、`git clone` / `git push` / `git fetch`） | `network` |
| 4 | 命中破坏清单（`rm` / `del` / `rd` / `rmdir`、`format`、`shutdown`、`taskkill`、`reg delete`、`git reset --hard` / `git clean`） | `destructive` |
| 5 | 其余 | `unknown` |

硬约束：

1. 顺序 3 先于 4：`git clean` 之类的复合语义宁可归到 `network` 也不归到「无害」——分类器的错误方向必须是**更危险**；
2. 未知不得映射为 `mutating`（更不得映射为 `readOnly`）：`unknown` 是显式类别，卡片如实显示「无法判定」；
3. 清单是**开放集合**（命中即分类，未命中即 `unknown`），不做「未在清单内 = 禁止」。

### 5.6 `preview_for_approval`

返回 `ApprovalPreview { summary, truncated: false, estimated: true }`：

- `summary` 形如 `执行 1 条命令 · 只读 · 隔离：完全` / `执行 1 条命令 · 无法判定 · 隔离：部分`；
- `estimated = true`：命令的实际影响在执行前不可知（TOCTOU），不得声称「将删除 N 项」这类精确量；
- `truncated` 恒 `false`（不存在扫描上限）；
- 纯字符串工作，不做 IO（因此不会因预算超时降级为 `None`）。

### 5.7 `execute` 流程（顺序固定）

顺序不是风格问题，而是正确性：**先建 sink、先接管管道、最后才进入等待**。反过来（先 `wait()` 再读管道）会死锁——子进程写满匿名管道缓冲区（Windows 4–64 KiB、Unix 64 KiB）后阻塞，父进程在 `wait()` 上等子进程退出，双方互等。`yes` / `cargo build` / 任何有进度输出的命令都会立刻触达这个缓冲区。

1. 取 `writable_root`（`ToolUseContext.workspace_root`）；缺失 → `Business` + `sandbox_root_missing`；
2. 读 [I53 §4.4](./I53-sandbox-preparation-spec.md) 的缓存探测结论；`enforcement == none` → `Business` + `sandbox_unavailable`（**不得**无沙箱执行）；
3. 构造 `SandboxPlan`（[I53 §4.3](./I53-sandbox-preparation-spec.md)）：`program` = 平台固定 shell，`args` = Windows `["/C", command]` / Unix `["-c", command]`；
4. **先 `open_sink`**（§5.11.1）：失败 → `Business` + `output_spill_failed`，**此时尚未创建任何子进程**。落盘是执行的前置条件，不是事后收尾——否则「磁盘满」会先跑完命令再判失败；
5. `backend.spawn(&plan)`：stdin 一律 `Stdio::null()`，stdout / stderr 走管道；失败 → 删除第 4 步刚建的空文件（尽力而为），错误码按失败来源区分（§7 F4 / F5 / F6）：边界建立失败 → `sandbox_setup_failed`；程序不存在或进程创建失败 → `spawn_failed`；
6. **立刻起两个并发读取任务**（各占一个 blocking 池线程）接管 stdout / stderr：逐块读字节 → `sink.write(stream, chunk)`（§5.11.1），读到 EOF 结束。本步必须在第 7 步之前落地，否则回到死锁；
7. 等待：把 `wait()` 丢进 `spawn_blocking` 得到**收尸任务**，在 async 侧用 `select!` 与 `timeout_ms` deadline / `context.cancel` 竞争（只有 `await` 点可被抢占）；
8. 收尾：**任何终态都要「杀 + 收尸」**——超时 / 取消是 `kill_tree()` → `await` 收尸任务（杀树后它必然返回）；正常退出是收尸任务已返回 → 再 `kill_tree()` 扫尾（§5.8「正常结束也要扫尾」）。两种终态收尾后的状态必须相同（整树已回收、管道写端已关）；
9. **有界 join 两个读取任务**（宽限 2s）：整树已回收，管道写端应全部关闭；若宽限内仍未 EOF（孙进程逃逸整树回收 / 后端未彻底清理），放弃读取句柄继续往下，已读内容照常落盘，**不得**无限期等管道。放弃的是 `spawn_blocking` 任务而非句柄本身，该线程会滞留到管道关闭为止（§12 R8）；
10. `sink.finish()`（**四种终态一律调用**，§5.8）得到 `SpilledOutput`，按 §5.11.2 渲染回填文本；
11. 组装结果（§5.9），`ToolResult.persisted` 置为 sink 给出的引用。

**执行器归属**（线程模型，实现约束）：

| 步骤 | 落点 | 理由 |
|---|---|---|
| 4 `open_sink` | `async fn` 入口，实现内部用 `spawn_blocking` 包住建文件与写头 | 接口本就是 `async`（§5.11.1），调用方无法再包一层；阻塞必须收在实现内部 |
| 5 `backend.spawn`、6 两个读取循环、8 的 `wait` 收尸 | 调用方用 `tokio::task::spawn_blocking` 包住 | 全是同步阻塞调用（`CreateProcess` / `Read::read` / `Child::wait`）；直接写在 `async fn` 体内会占死 tokio worker |
| 7 deadline / cancel 竞争、9 有界 join | async 侧（`select!` + `tokio::time::timeout`） | 只有 `await` 点可被抢占 |
| 10 `sink.finish()` | 调用方用 `spawn_blocking` 包住 | 追加 stderr 段、删空文件、解码头尾窗口都是同步文件 IO |

- `assistant-tools` 不依赖 tauri，一律用 `tokio::task::spawn_blocking`，**禁止**用 `tauri::async_runtime`；
- **禁止**为 `shell` 自建 `std::thread`：自建线程不在 tokio 的收口范围内，turn 取消令牌管不到它；
- 一个活跃 `shell` 常驻 3 个 blocking 池线程（stdout 读、stderr 读、收尸），长命令期间不释放；资源账与滞留风险见 §12 R8。

**不做**：不做 PATH 查找（`program` 是绝对路径）、不写脚本临时文件、不注入 `cd` 前缀（用 `cwd` 字段）、不分配 PTY、不提供 stdin 写入通道、不做 `yield_time_ms` 式中途返回。后三者是「运行期交互」能力，P14 不引入（§12 OQ8）。

### 5.8 超时、取消与杀进程树

| 触发 | 行为 | `error_kind` | `data` |
|---|---|---|---|
| 正常结束 | `wait()` 收尸 → `kill_tree()` 扫尾 → join（宽限 2s）→ `finish` | 无（`ok = true`） | 完整（§5.9） |
| `timeout_ms` 到期 | `kill_tree()` → `wait()` → join（宽限 2s）→ `finish` | `Timeout` | **保留**：`outputPath` + 已读计数 |
| turn 取消令牌 | 同上 | `Cancelled` | **保留**：同上 |
| 输出触达文件上限 | **不杀树**，继续读并丢弃 | 无（`ok` + `outputTruncated`） | 完整 |

**正常结束也要扫尾**：`wait()` 只回收**直接子进程**，`cmd /C` 与 `sh -c` 派生的孙进程可能仍在运行并继续持有管道写端（Unix `cmd &`、Windows 脱离 Job 的派生）。若不扫尾，第 9 步的 join 会等一个永不到来的 EOF，而 deadline 此时已失效（超时不再生效）——调用挂死且无人兜底。因此「命令退出」与「整树回收」是两件事，正常结束路径必须补一次 `kill_tree()`；一次 `shell` 调用的生命周期止于这次调用，退出后不留任何残留进程（与 A12 口径一致）。`kill_tree()` 幂等，两种终态都调用同一实现。

**四种终态一律 `finish()`**：sink 持有文件句柄与头尾窗口，不 `finish` 等于把已写好的输出丢掉。超时 / 取消因此是「失败 + 可回读的部分输出」——`data` 仍带 `outputPath` / `outputBytes` / `outputLines`（截至杀树时的计数），`result_for_assistant` 仍渲染落盘引用 + 头尾窗口，末尾追加 `[timed out after <N>ms]` 或 `[stopped: turn cancelled]`。

失败前已产出的日志正是模型纠错所需（构建中断、测试挂死），丢掉它等于让超时退化成「一无所知」。为此 `ToolResult` 需要一个带 `data` 的失败构造器（`result.rs` 现状的 `err_with_kind` 把 `data` 置 `None`）：

```rust
/// 失败但保留已产出的部分结果（shell 超时 / 取消路径用）。
pub fn err_with_data(
    kind: ToolErrorKind,
    error: impl Into<String>,
    data: Value,
    text: impl Into<String>,
) -> Self
```

信封侧无需额外分支：失败分支的 `subject` 取命令、`reason` 取错误首行，`persisted` 仍来自 `ToolResult.persisted`，因此超时信封同时显示失败原因与输出文件。该路径**不经过** Pipeline 的 `persist()`（它只对 `ok` 结果工作），不存在二次落盘。

`kill_tree` 的语义必须是**整棵树**：Windows `TerminateJobObject`（一次杀光 Job 内所有进程），Unix `kill(-pgid, SIGKILL)`。禁止只杀直接子进程——`cmd /C` 与 `sh -c` 都会派生；孙进程还会继承管道写端，杀不干净就让 §5.7 第 9 步的 join 等不到 EOF。

收尸必须 `wait()` 到结束再返回，避免僵尸进程与句柄泄漏。`wait()` 是阻塞调用，落在 `spawn_blocking`（§5.7「执行器归属」），不得在 async 任务里直接调。

**异常路径不得遗留进程或文件**：`spawn` 成功后立刻把子进程登记进 `Drop` 守卫（守卫 drop 时若发现未收尸则 `kill_tree()`）；sink 的 `finish` 同样由守卫兜底——`execute` 中任何 `?` 提前返回都不得绕过它。panic / 上层取消传播时至少不留孤儿进程与半截文件。

### 5.9 结果形状

`data`（`ToolResult.data`，camelCase 键，属既有开放 JSON，不新增 wire 字段）：

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
  "sandbox": { "backend": "landlock", "enforcement": "full" }
}
```

`outputPath` 是**绝对路径**（`PersistedRef.path` 原样），是模型回读的唯一入口（§5.11.4）；空输出时为 `null`（第 5 条）。`data` 不携带 `stdout` / `stderr` 全文。

**退出状态二选一**：Unix 被信号杀死时 `exitCode` 为 `null`、`signal` 为信号号；Windows 恒有 `exitCode`、`signal` 恒 `null`。**不得**用 `-1` 之类哨兵冒充「没有退出码」——`-1` 是合法退出码，混淆后模型会把正常退出读成崩溃。

**两个字节数不混用**：`outputBytes` 是子进程**总产出**字节（精确；触达上限后仍继续计数），`PersistedRef.bytes` 是**文件实际字节**，`outputTruncated` 时二者不等。`compact.persisted.bytes` 取后者（§5.11.2），信封 `scale` 取前者（§5.12）。

`outputLines` 统计 stdout + stderr 的换行数（末尾无换行的残行也计 1 行），同样在触达上限后继续计数。

**回填文本**：`result_for_assistant` = 落盘引用（标签 + 头窗口 + 省略标记 + 尾窗口，由 §5.11.2 的 `persisted_reference_from_windows` 渲染）+ 末尾标记行。**禁止**手写标签或自行拼省略标记。

标记词汇固定（模型按这些串判断结果性质，实现不得各自发明）：

| 位置 | 标记 | 出现条件 |
|---|---|---|
| 段落头 | `--- stderr ---` | stderr 非空且未超窗口 |
| 段落头 | `--- stderr (last <N> of <M> chars) ---` | stderr 超过 2 MiB 环形窗口（头部已丢） |
| stdout 段之后 | `[truncated: output exceeded 60 MiB; total unknown]` | stdout 触达上限 |
| 文本末尾 | `[exit code: <N>]` | `exitCode` 非 `null` 且非 0 |
| 文本末尾 | `[killed by signal: <N>]` | `signal` 非 `null` |
| 文本末尾 | `[timed out after <N>ms]` | 超时 |
| 文本末尾 | `[stopped: turn cancelled]` | turn 取消 |

顺序固定：**正文在前、标记在末尾**。理由是 `parse_truncation` 从尾部 `rfind`，且尾部窗口本就为「抓住末行」而设；标记若前置会被头窗口淹没。空输出时正文位置写 `(no output)`（与 Codex / DSH 一致，避免模型面对空串无从判断）。

**文件内布局**（单文件，顺序固定）：

```
<stdout 全文（上限 60 MiB）>
[truncated: output exceeded 60 MiB; total unknown]   ← 仅当 stdout 触达上限
--- stderr ---                                       ← 仅当 stderr 非空
<stderr 尾部窗口（≤ 2 MiB）>
```

**stderr 独占保留区**是本节的关键取舍：若 stdout 与 stderr 抢同一个总预算，`cargo build` 写满几十 MB 进度后失败的那一行错误会被挤掉，尾部窗口只剩进度条——工具在最需要它的场景下失效。因此 stdout 上限压到 60 MiB，stderr 单独留 2 MiB 内存环形窗口，`finish` 时追加到文件末尾；尾窗口（末 600 字符）因此必然落在 stderr 的末行上，与 `PREVIEW_TAIL_CHARS` 的设计前提吻合。

单文件而非双文件：`compact.persisted` 只有一个槽位，双文件会让回填出现两个路径、`parse_persisted_tag` 出现两个候选。

五条口径：

1. **非零退出码不是工具失败**：`ok = true` + `exitCode != 0`，由模型判断（与 `grep` 零命中同理）。工具失败只保留给「无法执行」（spawn 失败、超时、取消、落盘失败）；
2. **输出不驻留内存**：stdout 边读边写入落盘文件，内存只留头 / 尾窗口（各 `PREVIEW_HEAD_CHARS` / `PREVIEW_TAIL_CHARS` 字符）与计数器；stderr 额外驻留一个**有界**的 2 MiB 环形窗口（常量内存，非无界放大）。旧稿的「8 MiB 采集上限 → 杀树」作废：它会让「日志很大但合法」的命令被判失败，而内存放大问题本可用落盘消除；
3. **stdout 上限 `MAX_SPILL_BYTES = 60 MiB`**：触达后**继续读取并丢弃**（只计数），不杀进程（产出大量输出是合法结果，不是错误）；置 `outputTruncated = true`，并在文件 stdout 段末尾写一行 `[truncated: output exceeded 60 MiB; total unknown]`。该行同时保证出现在 `SpilledOutput.tail` 末尾（文件里只写一处；若 stderr 段把它挤出尾窗口，由 sink 在 `tail` 末尾补一行，两处不重复），`parse_truncation` 直接可判。`total unknown` 是必写标记（P13 门禁 3）：此处无法给出总字节，故不得写精确数；
4. **`truncated` 语义单一**：`outputTruncated` 只表示「落盘文件被上限截断」，不表示「预览窗口被裁剪」；预览裁剪用 `[… N chars omitted; full output in the file above …]` 标记，二者不得混用，也不得让 `[truncated:` 出现在预览裁剪场景（P13 §2.2 口径）；
5. **空输出不落盘**：`total_chars == 0` 时 `finish` 删除已建的空文件并返回 `reference = None`，`outputPath = null`，回填文本为 `(no output)` + 标记行。留着 0 字节文件只会让回读拿到空串，且给 30 天清理增加无意义条目。

另一条不变式：**只有 `[truncated: …]` 写进文件**——它是内容事实；`[exit code: …]` / `[timed out …]` 等是**执行事实**，只进回填文本，不改写子进程产物。文件因此始终是「子进程说了什么」的原样记录。

### 5.10 环境变量

**透传白名单**（不存在则跳过）：Windows `SystemRoot` / `PATH` / `TEMP` / `TMP` / `USERPROFILE` / `HOMEDRIVE` / `HOMEPATH` / `COMSPEC` / `PATHEXT` / `OS` / `PROCESSOR_ARCHITECTURE` / `NUMBER_OF_PROCESSORS`；Unix `PATH` / `HOME` / `TMPDIR` / `LANG` / `LC_ALL`。

**强制注入**（覆盖白名单结果；白名单里因此不含 `TERM`）：

| 变量 | 值 | 目的 |
|---|---|---|
| `TERM` | `dumb` | 关掉 ANSI 与进度条重绘，落盘文件不被控制字符污染 |
| `NO_COLOR` | `1` | 同上（多数 CLI 尊重） |
| `PAGER` | `cat` | `git log` / `man` 类命令不进分页器——无 PTY 时分页器会挂到超时 |
| `GIT_PAGER` | `cat` | 同上 |
| `GIT_TERMINAL_PROMPT` | `0` | 缺凭据立即失败，而不是等一个永远不来的输入（stdin 已为 null） |

不变式：强制注入只**消除交互式挂起与渲染噪声**，不改变命令语义。**不得**注入 `CI=1` 一类会改变构建行为的变量——那属于「让命令跑得不一样」，不是「让它不挂住」。

**强制规则**：

1. 名字（大小写不敏感）命中 `KEY` / `TOKEN` / `SECRET` / `PASSWORD` / `CREDENTIAL` 的一律**不透传**；
2. `ASSISTANT_*` 前缀一律不透传（内部状态不得外泄给子进程）；
3. 不做「用户自定义 env」入口（P14 无此输入字段）；
4. 强制注入的五个变量不受规则 1 约束（它们是工具自己的常量，不是继承自父进程的秘密）。

### 5.11 输出落盘与回读（统一输出存储）

本阶段把「工具输出」收敛到**一个**存储：`shell` 是第一个**流式**生产者，既有 `persist()` 是第二个**整段**生产者。两者共用同一目录、同一命名规则、同一清理策略、同一引用标签、同一回读路径——「统一输出内容管理」就落在这里，不新增第二个目录。

#### 5.11.1 存储抽象（`storage.rs` 新增）

```rust
#[async_trait]
pub trait ToolResultStorage: Send + Sync {
    async fn persist(&self, req: PersistToolResult) -> Result<PersistedRef, String>;

    /// 流式入口：先建文件，边写边算头尾窗口，结束时给出引用与窗口。
    async fn open_sink(&self, req: PersistToolResult) -> Result<Box<dyn ToolResultSink>, String>;
}

pub enum OutputStream {
    Stdout,
    Stderr,
}

pub trait ToolResultSink: Send {
    /// 逐块写入原始字节；`stream` 决定落盘区还是 stderr 环形窗口。
    /// 同步签名：调用方是 blocking 池里的读取循环，无法 `await`（§5.7「执行器归属」）。
    fn write(&mut self, stream: OutputStream, chunk: &[u8]) -> Result<(), String>;
    /// 同步收尾（追加 stderr 段 / 删空文件 / 解码头尾窗口），由调用方置于 `spawn_blocking`。
    fn finish(self: Box<Self>) -> Result<SpilledOutput, String>;
}

pub struct SpilledOutput {
    pub reference: Option<PersistedRef>, // 空输出为 None（文件已删除）
    pub head: String,                    // 文件首个 PREVIEW_HEAD_CHARS 字符
    pub tail: String,                    // 文件末个 PREVIEW_TAIL_CHARS 字符（truncated 时以 [truncated: …] 结尾）
    pub total_chars: usize,              // 已写入文件的字符数（truncated 时为截至上限的计数）
    pub output_bytes: usize,             // 子进程总产出字节（精确，含被丢弃部分）
    pub output_lines: usize,             // 子进程总产出行数（精确，含被丢弃部分）
    pub truncated: bool,                 // stdout 触达 MAX_SPILL_BYTES
    pub stderr_truncated: bool,          // stderr 超过 STDERR_RING_CHARS
}
```

新增常量（与既有 `PREVIEW_*` 并列）：

```rust
pub const MAX_SPILL_BYTES: usize = 60 * 1024 * 1024;   // stdout 落盘上限
pub const STDERR_RING_CHARS: usize = 2 * 1024 * 1024;  // stderr 内存环形窗口
```

六条约束：

1. `open_sink` 与 `persist` 必须写同一根目录、同一 `session_namespace`、同一文件名规则（`<unique_prefix>-<tool>-<call_id>.txt`，复用既有 `unique_prefix` / `safe_component` / `result_file_name`）。两套路径会让清理与回读出现两个事实源；`open_sink` 只用 `req` 的 `tool_call_id` / `tool_name` / `session_id`，`content` 字段忽略（内容由 sink 逐块写入）；
2. `head` 取**文件首**的前 N 字符，`tail` 取**文件末**的后 N 字符（stderr 非空时即 stderr 末行），中间只落盘、不进内存；`output_bytes` / `output_lines` 对**全流**计数，触达上限后继续计数（因此仍是精确值，见 §5.9「两个字节数不混用」）；
3. `write` **永不因超限返回 `Err`**：stdout 超过 `MAX_SPILL_BYTES` 后只计数不落盘，stderr 超过 `STDERR_RING_CHARS` 后环形覆盖并置 `stderr_truncated`。二者都必须继续消费管道，否则子进程写阻塞 → §5.7 的死锁；
4. 落盘失败 → `finish` 返回 `Err`，工具按 `Business` 失败回填（§7 F14），**不得**退回「全文内联」：那会把已经消除的内存放大问题重新引回来；
5. **并发写入由 sink 内部串行化**：sink 由 `Arc<std::sync::Mutex<Option<Box<dyn ToolResultSink>>>>` 持有，两个读取任务（blocking 池线程）逐块 `lock()` 写入。临界区只有一次 `write`，内部**禁止**再取其它锁、禁止做跨进程等待，因此不会与「读管道」互相卡死；用 `std::sync::Mutex` 而非 `tokio::sync::Mutex` 是因为 `write` 是同步签名（§5.7「执行器归属」）。stdout 与 stderr 落在文件的不同区域（stderr 在 `finish` 时追加），因此**不需要**交错顺序保证。`finish` 在 §5.7 第 9 步 join 之后用 `lock().take()` **取走** box 再调用，**不得**依赖 `Arc::try_unwrap`：有界 join 放弃后仍可能有读取线程滞留（§12 R8），引用计数归零不可保证，而 `take()` 与线程数无关。取走之后滞留线程再 `write` 命中 `None`，只丢弃字节、不再触碰文件；
6. `finish` 负责四件收尾：`total_chars == 0` → 删除空文件并返回 `reference = None`；stdout 触达上限 → 在 stdout 段末尾补 `[truncated: …]` 行；stderr 非空 → 追加 `--- stderr ---` 段（超窗口时头行写 `--- stderr (last <N> of <M> chars) ---`）；`reference.bytes` 取**文件实际字节**；
7. **编码边界**：`write` 收的是**原始字节**，文件按字节原样写入（不改编码、不做 CRLF 转换）；`total_chars` / 头尾窗口需要字符，但分块边界可能切在多字节 UTF-8 序列中间——因此**禁止**对每个 chunk 各自 `String::from_utf8_lossy` 后拼窗口（会把一个汉字拆成两个 `U+FFFD`）。正确做法：字节先落盘、同时累计字符数用**增量** UTF-8 解码器（非法序列计 1 字符，不 panic）；窗口在 `finish` 时对**已缓冲的字节**统一解码（尾部窗口从首个合法 UTF-8 起始字节开始解码，跳过被切掉的前导续字节）。

#### 5.11.2 引用标签的唯一渲染点

```rust
/// 流式落盘后的回填文本：标签 + 头窗口 + 省略标记 + 尾窗口。
pub fn persisted_reference_from_windows(
    path: &str,
    head: &str,
    tail: &str,
    total_chars: usize,
) -> String
```

`persisted_reference(path, content)`（P13 已有，整段生产者用）保留不动；两者必须渲染**逐字节一致**的标签与省略标记：

| 元素 | 形态 |
|---|---|
| 标签 | `<persisted-output path="…">`，**只带 `path`**（现状不写 `bytes`；`parse_persisted_tag` 虽容忍 `bytes`，但两个生产者不得各自发明形态） |
| 省略标记 | `[… N chars omitted; full output in the file above …]`，`N = total - PREVIEW_HEAD_CHARS - PREVIEW_TAIL_CHARS` |
| 短内容 | `total <= PREVIEW_CHARS` 时不加省略标记，直接标签 + 全文 |

`parse_persisted_tag` 才能对历史行 round-trip（`rebuild_compact` 的回退路径依赖这一点）。`compact.persisted.bytes` 取自 `PersistedRef.bytes`，**不**取自标签。

两条补充：

1. **`tail` 的语义是「文件末」而非「stdout 末」**：stderr 非空时，`tail` 落在 stderr 末行上——这正是错误末行所在。实现不得用「stdout 的最后一块」充当 `tail`；
2. **`reference == None`（空输出）不加标签**：回填文本直接是 `(no output)` + 标记行，`compact.persisted` 为 `None`。不得为「空文件」渲染一个指向 0 字节文件的标签（模型会去读它，拿到空串后无法判断是「没输出」还是「读失败」）。

#### 5.11.3 `ToolResult` 携带落盘引用（前置修正）

`ToolResult` 新增内部字段（`result.rs`）：

```rust
#[serde(skip)]
pub persisted: Option<PersistedRef>,
```

- **不进 wire**：`OsToolResultDto` 是显式字段映射（N8），不因该字段变更 DTO，故无 `contract:gen`；
- `ToolOutcome::new` 以 `result.persisted` 为初值（**失败结果同样**：`err_with_data` 构造的 `ToolResult` 也带 `persisted`，超时信封因此能显示输出文件）；
- `ToolPipeline::execute` 对 `outcome.persisted.is_some()` 的结果**跳过** `persist()`：否则会对「预览 + 标签」二次落盘，且 `compact.persisted` 指向二次产物（模型按它回读读不到全文）；
- `ToolPipeline::persist` 只对 `ok` 结果工作（失败结果的错误文本本就该完整回填）；`shell` 的失败路径自带落盘引用，因此**不依赖**该函数，也不存在「失败结果被二次落盘」；
- `build_compact` 的 `persisted` 仍只来自 `PersistedRef`（门禁 8：不得由文本解析产生）。

#### 5.11.4 回读：`read` 的第二作用域与额外许可

P13 之后工具输出**不可回读**：落盘目录在应用数据目录内，`read` 的边界是 `workspace_root`，`path::resolve` 直接判越界。对文件工具无伤（落盘的是源文件副本，读源文件即可）；对 `shell` 是硬伤——命令输出是**不可重现**的产物（构建日志、测试失败栈），落盘即失联。

本阶段打通回读，但**必须带额外许可**：跨出工作区边界不是「工作区内的普通读」，不得沿用 `read` 的默认 `Allow`。

**（1）路径解析：新增 `os::path::resolve_readable`**

```rust
pub enum ReadableScope { Workspace, ToolOutput }

pub struct ReadablePath { pub path: PathBuf, pub scope: ReadableScope }

pub fn resolve_readable(input: &str, context: &ToolUseContext) -> Result<ReadablePath, String>
```

判定顺序（**输出域优先于工作区域**）：

1. 形态校验与 `resolve` 完全一致（盘符相对路径、含 `..`、不可归一 → 拒绝）；
2. `reject_system_dirs` 照旧（敏感目录即便落在输出域内也不放行）；
3. 落在 `context.tool_output_root` 内 → `ToolOutput`；
4. 否则按 `workspace_root` 判定（既有行为，`Workspace`）。

第 3 步必须先于第 4 步：若输出域与工作区意外重叠（配置失误），路径必须走**更严**的 `ToolOutput` 分支，不得因为「也在工作区内」而落回 `read` 的 `Allow`。

**三个调用点必须一起换**（`agent_read.rs`）：`validate_input`、`permission_intents`、`execute`。少换一处就出现「校验按 A 口径、意图按 B 口径、IO 按 C 口径」的三套边界；`read` 的回执键仍取 `ReadablePath.path`，两种作用域共用同一键空间（路径本身已能区分）。

`reject_system_dirs` 的位置不变（第 2 步）：它先于一切边界判定，凭据目录（`.ssh` / `.aws` 等）在任何作用域内都不可读。

`tool_output_root` 是**运行时字段**（`#[serde(skip)]`，与 `cancel` 同类，不进 wire），**由落盘存储自身派生**，不在第二处重算路径：

```rust
pub trait ToolResultStorage: Send + Sync {
    async fn persist(&self, req: PersistToolResult) -> Result<PersistedRef, String>;

    /// 本存储为某个会话分配的落盘目录（回读域）。默认 `None` = 不提供回读。
    fn namespace_root(&self, session_id: Option<&str>) -> Option<PathBuf> { None }
}

// FileToolResultStorage：与 `persist` 内部用的是**同一个**函数，路径只有一个来源
fn namespace_root(&self, session_id: Option<&str>) -> Option<PathBuf> {
    Some(self.root.join(session_namespace(session_id)))
}
```

`persist` 与 `open_sink` 都改为调用 `namespace_root(...).ok_or(...)?` 取目录，**禁止**各自 `root.join(session_namespace(..))`——这正是「清理 / 落盘 / 回读三个事实源」的防呆点。

注入链路只有一步：

1. `ToolPipeline` 暴露 `pub fn storage(&self) -> &Arc<dyn ToolResultStorage>`（或 `output_root(session_id)` 薄封装）；
2. `PipelineToolExecution::execute` 填 `tool_output_root: self.pipeline.storage().namespace_root(Some(conversation_id))`，与 `workspace_root` 同一处、同一时机求值。

三条好处：**不新增** `PipelineToolExecution` 字段（`with_policy` 派生无需补 clone）、**不改** `tool_setup.rs`、**不改** `FileToolResultStorage` 的构造方式。而 `ToolUseContext::for_toolbox` 是显式字段字面量构造，新增字段后**必须**在该处补 `tool_output_root: None`（编译期可见「ToolBox 没有输出域」）。

`ToolBox` 链路不经 Pipeline、`session_id` 恒 `None` → 输出域不可读：无会话即无归属，不给「任何会话都能读」。

子代理链路（I38f）的 `ToolResultStorage` 根是 `tool-results/subagents/<slug>/`（[I38f §6](./I38f-agent-subagent-spec.md)），因此 `namespace_root` 自动落在该子树内：子 turn 的 `shell` 输出**可被子 turn 自己回读**，父会话读不到（两棵子树互不包含），子 turn 收尾时随守卫一并清理。这是 `namespace_root` 相对「注入全局 `results_dir`」的关键收益：回读域**跟着存储走**，而不是跟着某个硬编码的根走——否则子代理的输出域会错指到主会话命名空间（读到别人的输出，或读不到自己的）。

目录按需创建（落盘侧已 `create_dir_all`），回读侧**不创建**目录：只读路径不产生副作用，目录不存在即「无输出可读」，返回业务失败而非「路径非法」。

**（2）意图：新增动作 `readToolOutput`**

`read` 的 `permission_intents` 按 `scope` 分流：

| `scope` | `action` | `resource` |
|---|---|---|
| `Workspace` | `read` | 具体路径（不变） |
| `ToolOutput` | `readToolOutput` | **本会话输出命名空间目录**（不是具体文件） |

`resource` 取目录而非文件，是为了让审批键覆盖「本会话全部输出」：模型读第 2、第 3 个输出文件不再弹框；而跨会话仍被 `resolve_readable` 直接拒绝——**隔离靠路径判定，不靠审批兜**。

`read` 的 `description` 需补一句，否则模型不知道「回填里的路径是可读的」：

> Tool output files referenced by a `<persisted-output path="…">` tag can be read with this tool; the first such read asks the user for permission.

只加这一句，不新增参数、不新增工具：模型看到标签就会试，试失败才需要解释。

**回读仍受 `MAX_READ` 约束**：输出文件可达 60 MiB，`read` 的既有单次上限不变，模型用 `offset` / `limit` 分页（OQ7）。不得为回读放宽 `MAX_READ`；也不得让回读结果再落盘——否则出现「输出文件之输出文件」，路径无限套娃。

**（3）策略：`readToolOutput` 恒 `Ask`**

`base_rules` 追加一条（排在 `("*", "*")` 与 `("read", "*")` 之后）：

```rust
PermissionRule::new("readToolOutput", "*", PermissionEffect::Ask),
```

三条硬约束：

1. **任何审批级别都不放宽**：`ApprovalLevel::None` 的改写只作用于 `base_rules[0]`，本规则位于其后且更具体 → `none` 档同样 `Ask`。理由：`none` 档的语义是「工作区内免审」，不覆盖跨出工作区的读；
2. **不会被 `read` 的默认 `Allow` 漏过来**：`("read", "*", Allow)` 按动作名精确匹配，不匹配 `readToolOutput`；
3. **可被会话级授权覆盖**：键为 `ApprovalScope::Path { canonical: 输出命名空间 }`，`AllowSession` 一次覆盖本会话全部输出读取，`AllowAlways` 可持久化。

**（4）审批键**

`ApprovalKey::from_intent` 的 `Path` 分支追加 `"readToolOutput"`（与 `read` 同分支，`canonical_path_for_key` 对目录同样成立）。**不新增** `ApprovalScope` 变体，`grant_key` 列无迁移。

**（5）卡片信息**

两处各司其职，不混用：

- **`preview_for_approval`（有 IO、2s 预算）**：`summary` 写「读本会话输出 `<文件名>` · 第 N–M 行 · `<human_bytes>`」，文件名只取末段（不写绝对路径），行窗口来自 `offset` / `limit`，字节数来自 `stat`；
- **`build_approval_context`（零 IO）**：`output_extensions(pending, context)` 与既有 `workspace_extensions` 并列，仅凭 `pending.resource` 是否落在 `tool_output_root` 内产出**一条**扩展：

| `kind` | `label` | `value` | `severity` |
|---|---|---|---|
| `output.scope` | 读取范围 | `本会话工具输出目录（只读）` | `Info` |

分工的理由：`ApprovalExtension` 只承载**能从待审批资源直接推出的事实**（纯函数、无 IO），与 `workspace_extensions` 完全同构；任何需要 `stat` 的量级信息留在 `preview` 里，才不会把 IO 预算和超时降级扩散到扩展构造路径。

零前端改动：`kind` / `label` / `value` / `severity` 是开放字段，前端按 `label` + `value` 通用渲染（I38d §3.2.1（已归档）口径，N6）。**禁止**为 `output.scope` 新增前端分支或 DTO 字段。

**（6）写方向仍然关闭**

输出域**只读**，三条各自独立成立：

- `write` / `edit` / `delete` / `move` 在该域内仍由 `path::resolve` 的 workspace 边界拒绝（`ValidationResult::blocked`）——`resolve_readable` 只被 `read` 使用，不被其它工具调用；
- 子进程不得写该域：可写根仍是 `workspace_root`（[I53 §4.3](./I53-sandbox-preparation-spec.md)），模型在命令里重定向到输出域会写失败（I53 A-S1 同口径）；
- 读过的输出文件不产生可编辑入口：`edit` 的读回执校验仍以 workspace 边界为前提（回执存在不等于可编辑）。

#### 5.11.5 写入者是父进程，不是子进程

1. 输出文件由工具进程（父进程、沙箱外）创建与写入，子进程只经管道交出 stdout / stderr；
2. 子进程**不需要**对输出域的写权限，可写根保持单一（[I53 §4.3](./I53-sandbox-preparation-spec.md)），`SandboxPlan` 不新增字段，I53 A-S1 / A-S2 判据不受影响；
3. 父进程写文件失败（磁盘满 / 权限）→ `Business` 失败（§5.11.1 第 4 条），不静默降级为内联全文。

### 5.12 信封投影（`compact`）

`compact.rs` 的 `subject` / `scale` 必须为 `shell` 增加分支，否则 head 退化为无信息量的 `ok shell`：

| 槽位 | `shell` 取值 | 约束 |
|---|---|---|
| `subject` | `arguments.command`（命令是**输入事实**，不读 `data`） | 换行与连续空白折成单空格（head 单行约束），再按 `SUBJECT_MAX_CHARS` 中间省略 |
| `scale` | `exit <code> · <outputLines> lines · <human_bytes(outputBytes)>` | `outputTruncated = true` 时行数后缀 `+`（如 `1200+ lines`），不写「约」「至少」 |

四条约束：

1. `truncated` 只来自 `parse_truncation`（§5.9 第 3 条的 footer），`shell` 不得自行拼 `[truncated:`；
2. `persisted` 来自 `ToolResult.persisted`（§5.11.3），不得由文本解析（门禁 8）；
3. `anchor` 为 `None`：输出文件不是用户文档，不进文档锚点；
4. 失败分支沿用通用规则：`subject` 取命令、`reason` 取错误首行；`err <kind> shell · <command> · <reason>`。

---

## 6 与审批链的集成

### 6.1 `Command` 审批键：既有分支，本阶段首次激活

`gateway.rs` 的 `ApprovalScope::Command`、`split_argv`、`command_cwd` 在 P2 已实现，但至今没有生产者（N1）。本阶段不新增键类型、不改归一化规则，只让它第一次被真实调用。

| 键组成部分 | 取值 | 归一化 |
|---|---|---|
| `argv` | `split_argv(command)` | 未闭合引号 → `None`（空键，不查缓存） |
| `cwd` | 输入 `cwd` ?? `workspace_root` | `canonical_path_for_key`（不可归一 → 原样，仍可作键） |
| `sandbox_tier` | `tool.tier()`（恒 `Os`） | 不变 |
| `enforcement` | §6.2 新增 | 三档枚举 |

`AllowSession` 授权粒度 = 这一条命令行 + 这个 `cwd` + 这个隔离强度。同一会话里重复执行**完全相同**的命令不再弹框；命令文本任何差异都会重新询问。这是有意的安全取舍，归一化候选见 §12 OQ2。

### 6.2 审批键必须携带隔离强度（前置修正）

**问题**：`sandbox_tier` 取自 `tool.tier()`，对 `shell` 是常量 `Os`。若沙箱从 `full` 退化为 `partial`（例如 Linux 从 ABI≥4 的机器换到 ABI<4 的机器、或后端建立失败），同一 `argv` + `cwd` 会命中同一授权键——**等于把「全隔离下批准的授权」复用到更弱的边界上**，方向是放宽权限。

**修正**（两处，均为内部类型，非 wire DTO）：

1. `Tool` trait 新增一个方法：

```rust
/// 本工具命令类操作的实际隔离强度；进审批键，避免「强边界下批准」的授权被弱边界执行复用。
/// 默认 `None`（非沙箱工具键形状不变）。
fn sandbox_enforcement(&self) -> Option<SandboxEnforcement> { None }
```

2. `ApprovalScope::Command` 增加字段：

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

影响面：`from_intent` 的调用点与单测需要多传一个参数；`ApprovalKey` 仍 derive `Serialize` / `Deserialize`，`grant_key` 列无需迁移。

### 6.3 审批卡片扩展（零前端改动）

`build_approval_context` 在 `tool.sandbox_enforcement()` 为 `Some(e)` 时追加两条 `ApprovalExtension`：

| `kind` | `label` | `value` | `severity` |
|---|---|---|---|
| `sandbox.enforcement` | 隔离强度 | `完全隔离` / `部分隔离` | `Info` / `Caution` |
| `sandbox.root` | 可写根 | `writable_root` 的**末段目录名**（不写绝对路径） | `Info` |

另外由 `preview_for_approval` 的 `summary` 携带 `CommandClass`（§5.6），不重复成第三条扩展。

零前端改动的依据：`ApprovalExtensionDto` 的 `kind` / `label` / `value` / `severity` 是开放字段，前端按 `label` + `value` 通用渲染（I38d §3.2.1（已归档）口径，N6）。**禁止**为这两个 `kind` 新增前端分支或 DTO 字段。

### 6.4 `Deny` 必须先于授权命中（前置修正）

现状（`ToolGateway::evaluate`）：先查 `grant_store.is_granted`，命中即 `continue`（跳过 `policy.evaluate`）。后果：某键此前被授权后，即使策略对该键的结论是 `Deny`（硬保护规则或 tier 约束变更），仍会放行。

`shell` 是首个把「同一个键可能长期有效」变成常态的工具（会话级授权 + 命令级键），因此本阶段把顺序改为：

```
policy.evaluate(action, resource)
  ├─ Deny  → 审计 Denied，立即返回 Denied
  ├─ Allow / Ask → 继续
grant_store.is_granted(key)
  ├─ 命中 → 审计 GrantHit，continue
  └─ 未命中 → 按 effect：Allow 审计 Allowed / Ask 进 pending
```

不变的部分：命中授权仍留 `GrantHit` 审计；空键仍不查缓存；`tier` 硬上限仍先于 intent 判定。

### 6.5 策略与审批级别

- `base_rules` 现状已覆盖 `execute`（`dangerous_only` 恒 `Ask`、`none` 恒 `Allow`），`execute` 侧**不新增规则**；
- **本阶段唯一新增的 Policy 规则**是 `readToolOutput`（恒 `Ask`，任何审批级别不放宽，§5.11.4(3)）；它不是命令规则，也不进 `hard_deny_rules`；
- **不新增命令黑名单规则**（§2.3）；`hard_deny_rules` 继续只针对路径；
- `none` 档下 `shell` 无审批，此时唯一边界是沙箱（§2.3 第 1 条）——这是 P10-a `EscalateOnly` 的真实用武之地（§6.6）。

### 6.6 P10 的三个接入点

| 接入点 | 本阶段交付的事实源 | P10 侧用途 |
|---|---|---|
| `Ask` 消费方 | `execute` 意图在 `dangerous_only` 档恒 `Ask` | 关闭 I38g §1.3 的 P10-b 前置（`RelaxOnly` 终于有只读调用可放宽） |
| 命令类别 | `CommandClass`（§5.5，纯函数） | P10-b 的**唯一**输入事实；顾问不得自行再判命令语义 |
| 内核隔离 | `enforcement`（[I53 §4.5](./I53-sandbox-preparation-spec.md)） | 关闭 I18 design §3.17.6 的 `Adjudicate` 门禁 1（检测层 L4） |

边界约束：

1. P10-b 的放宽条件由 [I38g](./I38g-agent-approval-advisor-spec.md) 规定，本规格**不写**放宽逻辑；本文只保证「`readOnly` + `full` 可被顾问识别」，且**至少**要求 `enforcement = full` 才具备放宽资格（`partial` 不放宽）。
2. P10-a `EscalateOnly` 不依赖本阶段（可在 `none` 档把危险命令提回 `Ask`），两者可并行。
3. `Adjudicate` **不因本阶段完成而开工**：门禁 2（独立 ADR）仍未关闭。

---

## 7 失败矩阵

| 编号 | 场景 | 检测点 | 行为 | `error_kind` | 可否改写重试 |
|---|---|---|---|---|---|
| F1 | 无 `writable_root`（ToolBox 入口 / 会话未绑定） | `validate_input` | 拒绝，不弹审批 | `Business` + `sandbox_root_missing` | 否（`blocked`） |
| F2 | 后端探测 `none` | 注册期 | 不注册 `shell` | — | 工具不存在 |
| F3 | 运行期缓存结论为 `none` | `execute` 第 2 步 | 拒绝执行 | `Business` + `sandbox_unavailable` | 否 |
| F4 | 边界建立失败（ACL / ruleset / profile） | `execute` 第 4 步 | 拒绝执行，**不降级** | `Business` + `sandbox_setup_failed` | 否 |
| F5 | 子进程侧限制失败（Unix `pre_exec`） | 子进程 | `_exit(126)`，父进程收到该退出码，按「限制未生效」判失败 | `Business` + `spawn_failed` | 否 |
| F6 | 可执行程序不存在 | `spawn` | 返回失败 | `Business` + `spawn_failed` | 是 |
| F7 | `cwd` 越界 | `validate_input` | 拒绝，不弹审批 | —（`ValidationResult::blocked`） | 否 |
| F8 | 参数形状非法 | `validate_input` | 拒绝 | —（`ValidationResult::invalid`） | 是 |
| F9 | 超时 | 等待竞争 | 杀树 → 收尸 → `finish()`；**保留已写输出**（带 `outputPath`） | `Timeout` | 是（调大 `timeout_ms`，或直接回读输出） |
| F10 | turn 取消 | 等待竞争 | 杀树 → 收尸 → `finish()`；**保留已写输出** | `Cancelled` | 否 |
| F11 | 输出触达上限（stdout 60 MiB / stderr 2 MiB 环形窗口） | 写入计数 | 继续读取并丢弃（stderr 环形覆盖），**不杀树**；结果仍成功 | 无（`ok` + `outputTruncated` / `stderrTruncated`） | 是（收窄命令输出 / 自行重定向到工作区文件） |
| F12 | 非零退出码 | 正常结束 | 结果成功 | 无（`ok` + `exitCode`） | 是（模型据 stderr 纠错） |
| F13 | 授权键命中但 `enforcement` 已降级 | §6.2 键不相等 | 重新走审批 | — | 是 |
| F14 | 输出落盘失败（磁盘满 / 权限） | `open_sink` / `finish` | 失败回填，**不内联全文** | `Business` + `output_spill_failed` | 是 |
| F15 | 回读路径跨出本会话输出域 | `resolve_readable` | 拒绝，不弹审批 | —（`ValidationResult::invalid`） | 是（改用回填里的路径） |
| F16 | 回读本会话输出域 | `policy.evaluate` | `readToolOutput` 恒 `Ask`；授权后 `GrantHit` | — | 是（授权一次即可） |

三条 fail-closed 底线（对应 F2 / F4 / F5）：**探测失败不注册**、**建立失败不执行**、**限制失败不落地**。禁止任何「先跑起来再补边界」的中间态。

回读侧的第三条底线：**越域不靠审批兜**。跨会话读取必须在路径判定阶段被拒（F15），不能落到「弹个框让用户批准」。审批只回答「要不要放行本会话的输出读取」（F16），不回答「要不要允许越界」。

---

## 8 审计与度量

1. **不改 I50 词表**：`shell` 的审计沿用 `action = "execute"` 与既有 `AuditOutcome`（`Approved` / `Allowed` / `Denied` / `GrantHit` / `Bypassed` 等），不新增 outcome；
2. **隔离强度不进 `permission_audit`**：`enforcement` 与 `backend` 落在 `ToolResult.data.sandbox`（§5.9）与审批卡扩展（§6.3），避免跨阶段改封闭词表；
3. **命令行的审计脱敏**：`resource` 会落库整条命令行，其中可能夹带密钥（`TOKEN=…`、`--password …`）。落库前必须做词法级打码：形如 `NAME=value` 且 `NAME` 命中 §5.10 的敏感词表 → `value` 替换为 `<redacted>`；`--password` / `-p` 的**后一个 token** 同样打码。脱敏只作用于审计与日志，不改变实际执行；
4. **日志固定串**：`backend` / `enforcement` / `exitCode` / `durationMs` / `outputBytes` / `outputTruncated` 可记；命令行全文与输出正文只记长度。回读的审计同样只记路径与范围（`readToolOutput` 的 `resource` 是本会话命名空间目录），不记文件内容。

---

## 9 前端与配置

| 项 | 内容 | 落点 |
|---|---|---|
| i18n | `tool_desc.shell`（zh / en）+ 审批卡扩展的 `label` 由后端下发，前端不新增 key | `src/i18n/locales/` |
| 工具装配面板 | 无改动：目录来自注册表（`list_tool_catalog`），后端不注册即不出现 | `src/settings/shared/sections/ToolAssemblySection.tsx` |
| 默认启用 | **默认不勾选**：`shell` 属破坏性工具，需用户在模式设置里显式启用（I38c §7.1 破坏性工具原则） | 模式预置 `toolConfig` |
| 审批卡 | 无改动：扩展按 `label` / `value` 通用渲染（§6.3、§5.11.4(5)） | `src/chat/` 审批组件 |
| 落盘展示 | 无改动：气泡仍只展示路径与字节数；P14 **不新增**「打开输出文件」入口（回读是模型侧 `read` 的能力，用户侧不需要额外界面） | `src/chat/tool/toolCallDisplay.ts` |
| 不可用提示 | 后端不注册时前端**不提示**「沙箱不可用」：用户看不到该工具即已足够，避免把平台能力差异暴露成噪音 | — |

---

## 10 验收标准

| 编号 | 验收项 | 判据 |
|---|---|---|
| A1 | 写边界 | 见 [I53 A-S1](./I53-sandbox-preparation-spec.md) |
| A2 | 敏感读边界 | 见 [I53 A-S2](./I53-sandbox-preparation-spec.md) |
| A3 | 进程树回收 | 见 [I53 A-S3](./I53-sandbox-preparation-spec.md) |
| A4 | 无沙箱不执行 | 见 [I53 A-S4](./I53-sandbox-preparation-spec.md) |
| A5 | 边界建立失败不降级 | 见 [I53 A-S5](./I53-sandbox-preparation-spec.md) |
| A6 | 审批键 | 同命令 + 同 `cwd` + 同 `enforcement` 两次调用：第一次 `Ask`，`AllowSession` 后第二次 `GrantHit`；命令文本差一个字符 → 重新 `Ask` |
| A7 | 降级不复用授权 | 同一键在 `full` 下授权后把 `enforcement` 置 `partial` → 必须重新 `Ask` |
| A8 | `Deny` 先于授权 | 已授权键 + 策略改判 `Deny` → `Denied`（§6.4 顺序修正） |
| A9 | 非零退出码 | `exit 3` → `ok = true` + `exitCode = 3`，模型收到 `exit code 3` 文本 |
| A10 | 输出不截断、只裁剪预览 | 输出 10 万行 → 落盘文件含全部 10 万行，回填为头 600 + 省略标记 + 尾 600，`compact.truncated` 为 `None` |
| A11 | 输出触达上限 | 输出超过 60 MiB → `outputTruncated = true`，进程**未被杀**且自然退出，回填尾部含 `[truncated: output exceeded 60 MiB; total unknown]` |
| A12 | 超时 / 取消 | 两者各自触发，结果 `error_kind` 分别为 `Timeout` / `Cancelled`，无残留进程；**且 `data.outputPath` 非空、文件保留已写内容、`read` 该路径可读到杀树前的输出** |
| A13 | `cwd` 越界先于审批 | 越界 `cwd` → 审批未被调用（prompter 计数为 0），`ValidationResult.blocked` |
| A14 | 审批卡 | `Ask` 时卡片含 `sandbox.enforcement` 与 `sandbox.root` 两条扩展，`preview.summary` 含 `CommandClass` 文案 |
| A15 | 环境变量 | 注入 `MY_API_TOKEN=…` 后子进程看不到该变量；`PATH` / `SystemRoot` 等白名单可见 |
| A16 | 审计脱敏 | `set TOKEN=abc` 落库为 `set TOKEN=<redacted>`，实际执行仍用原值 |
| A17 | 依赖边界 | 见 [I53 A-S6](./I53-sandbox-preparation-spec.md) |
| A18 | 工具可见性 | 未启用时模型 `tools` 列表不含 `shell`；启用且探测可用时含 |
| A19 | 落盘与回读闭环 | `shell` 输出 10 万行 → `data.outputPath` 指向 `tool-results/<本会话命名空间>/` 下文件；`read` 该路径在授权后成功，内容与文件一致，可分页读到末尾 |
| A20 | 额外许可恒定 | `readToolOutput` 在 `dangerous_only` 与 `none` 两档均为 `Ask`；`AllowSession` 后读同域**另一个**文件为 `GrantHit`（不再弹框） |
| A21 | 跨会话隔离 | 读其它会话命名空间路径 → `ValidationResult.invalid`，prompter 计数为 0（不弹框） |
| A22 | 输出域只读 | `write` / `edit` / `delete` 指向输出域 → `blocked`；沙箱内命令重定向到输出域 → 写失败 |
| A23 | 不二次落盘 | `compact.persisted.path` 等于 `data.outputPath`，且目录下只有一份该次调用的输出文件 |
| A24 | 信封投影 | 成功信封 head 为 `ok shell · <command> · exit 0 · N lines · <size>`；`outputTruncated` 时行数带 `+`；`anchor` 为 `None` |
| A25 | stderr 保留区 | `cargo build` 式场景：stdout 写满 60 MiB 后命令失败 → 文件尾部仍是 stderr 的末行（而非进度条），回填尾窗口包含真实错误行 |
| A26 | 空输出 | `cmd /C exit 0`（无任何输出）→ `data.outputPath` 为 `null`、回填为 `(no output)`、目录下无 0 字节文件残留 |
| A27 | 交互式命令不挂死 | `git log`（输出足够触发分页）→ 命令自然结束，不超时（强制 `PAGER=cat` / `GIT_PAGER=cat` 生效） |
| A28 | 正常结束也扫尾 | 命令派生后台子进程后主进程立即退出（Unix `sh -c 'sleep 300 &'`）→ 调用在秒级返回、不挂死；返回后该后台进程已不存在（`kill_tree()` 扫尾生效） |

---

## 11 分批实施

| 批次 | 内容 | 交付物 | 前置 |
|---|---|---|---|
| **C1 审批链前置** | §6.2 键字段 + `sandbox_enforcement()`；§6.4 求值顺序；A7 / A8 单测 | `trait.rs`、`gateway.rs`、`policy.rs` 单测 | 无（不依赖沙箱） |
| **C2–C4 沙箱准备（S0）** | 三平台后端 + 探测与注册收口；判据、交付物与前置全部见 [I53 §9](./I53-sandbox-preparation-spec.md) | 见 I53 §9 | C1 |
| **C4b 输出存储与回读** | §5.11.1 sink + §5.11.2 标签渲染 + §5.11.3 `ToolResult.persisted` + §5.11.4 `resolve_readable` / `readToolOutput` / `base_rules` 新规则 / 卡片扩展；A19–A23 / A26 | `storage.rs`、`result.rs`、`pipeline.rs`、`os/path.rs`、`os/agent_read.rs`、`policy.rs`、`gateway.rs` | C1（不依赖沙箱，可与 C2 / C3 并行） |
| **C5 工具本体** | schema / validate / intents / 分类器 / preview / execute / 超时 / 输出落盘 / §5.12 信封分支；A6 / A9–A13 / A15 / A24 / A25 / A27 / A28 | `os/agent_shell.rs`、`compact.rs` | C2–C4（I53）、C4b |
| **C6 卡片与审计** | §6.3 扩展 + §8 脱敏；A14 / A16 | `gateway.rs`、审计写入点 | C5 |
| **C7 前端与配置** | i18n、模式预置入口、默认不勾选 | `src/i18n/locales/`、模式设置 | C5 |
| **C8 端到端** | 真实命令 + 越界写失败 + 无沙箱不执行 + 审批三态回归；`clippy -D warnings` / `fmt --check` / `typecheck` / `vitest` | 集成测试 | C1–C7 |

S1 / S2（P14-b / P14-c，不在本规格验收范围）：S1 追加降级后端（Linux bwrap）与 Windows 双模式评估；S2 为高安全场景的可选后端（Sandboxie / OpenShell / 容器 / microVM），须独立立项（[沙箱调研 §10.5](./agent-sandbox-research.md)）。

任务级进度在本阶段开工后建 tasklist（[roadmap §7 第 2 条](./I38-agent-roadmap.md)），本文不记进度。

---

## 12 风险与开放问题

沙箱侧的开放问题与风险（原 OQ1 / OQ3 / OQ5 / R1 及新增的平台能力变更项）已移至 [I53 §10](./I53-sandbox-preparation-spec.md)；下表只保留 `shell` 工具与审批链的风险，编号不重排以免断链。

| 编号 | 问题 | 倾向与处置 |
|---|---|---|
| OQ2 | 审批键粒度：整命令行 vs 按 `argv[0]` 归一 | P14 保持**精确命令行**（安全优先）；归一化（只读类按 `argv[0]` 合并）列为 S1 候选，需与 P10-b 一并评估 |
| OQ4 | ToolBox 入口不支持 `shell` 是否最终形态 | 暂定 `AGENT_ONLY`（无工作区即无边界）；若将来 ToolBox 引入显式工作区，再评估放开 |
| OQ6 | 回读复用 `read` 还是新增独立 `read_output` 工具 | 已定论：复用 `read` + `ReadableScope`（§5.11.4）。独立工具会带来第二套路径校验、第二套审批键与第二套 i18n，收益为零 |
| OQ7 | 回读是否新增「只读中间段」参数 | 不新增：沿用 `read` 的 `offset` / `limit`。输出文件是纯文本，既有分页语义已够 |
| OQ8 | 运行期进度不可见（无 PTY、无 stdin 写入、无 `yield_time_ms` 式中途返回） | P14 接受：模型看到的只有「running 状态 + 取消按钮」，长命令期间拿不到任何中间输出。理由是三项能力都需要「工具调用可被挂起后再恢复」的运行期形态（Codex 的 `session_id` 轮询、DSH 的前台转后台 job），而当前 turn kernel 的工具调用是**同步阻塞**的一次性执行，没有第二次进入点（§5.7 末尾「不做」）。**不新增前端本地计时器**：卡片已有 running 态与取消按钮，加计时器属前端自造状态，与「零前端改动」冲突，且不解决「拿不到中间输出」这个真问题。列为 S1 候选（与后台任务一并评估） |
| OQ9 | 交互式命令（分页器 / 凭据提示 / 编辑器）在无 PTY 环境下挂起 | 已用强制环境变量压掉主要来源（`PAGER` / `GIT_PAGER` / `GIT_TERMINAL_PROMPT` / `TERM=dumb`，§5.10）；残余（如 `npm login`、`vim`）由 `timeout_ms` 兜底为 `Timeout`，模型据回填的 `[timed out after <N>ms]` 纠错。不为此引入 PTY |
| R2 | `cmd.exe` 的引号与元字符语义使「审批看到的 argv」≠「实际执行」 | 键基于**整串**而非 argv 语义推断；`CommandClass` 遇控制符即 `unknown`；边界由沙箱承担，不由字符串承担 |
| R3 | 命令级授权导致提示偏多 | 接受：`AllowSession` 对重复命令生效；长期方向见 OQ2 |
| R4 | 沙箱内命令功能受限（缺环境变量 / 网络） | 白名单已覆盖构建类工具；缺失项按「如实报错 + 模型纠错」处理，不扩大白名单到敏感名 |
| R5 | 输出文件累积体积（单会话可轻易到数百 MB） | 单文件 stdout 60 MiB + stderr 2 MiB 上限 + 30 天清理；不额外做会话配额，磁盘压力由清理周期承担，触达上限在 `outputTruncated` / `stderrTruncated` 与审计里可见 |
| R6 | 输出域读取被误当成「普通读」而免审 | `readToolOutput` 独立动作名 + 恒 `Ask` + 输出域优先判定（§5.11.4），三重各自独立；任一缺失都不构成放行 |
| R7 | 落盘文件被外部删除 / 截断（30 天清理与并发读取竞争） | 回读失败按普通 IO 失败回填（`Business`），不重建、不缓存；模型据此改跑命令或缩短读取范围。清理只删过期会话目录，不删正在写的文件（写入侧持句柄，删除在 Windows 上会失败并被跳过） |
| R8 | 线程占用与滞留：一个活跃 `shell` 常驻 3 个 blocking 池线程（stdout 读 / stderr 读 / 收尸），长命令期间不释放；第 9 步有界 join 放弃时，被放弃的 `spawn_blocking` 读任务**不会停止**，会卡在阻塞 `read` 上直到管道关闭 | 接受：`is_concurrency_safe = false` 已限制同轮并行，跨会话并发在 512 线程上限下约有 170 个 `shell` 的余量，够用。滞留只发生在「孙进程逃逸整树回收」这一后端缺陷路径上，属已知成本：缓解手段是正常结束也 `kill_tree()` 扫尾（§5.8），使管道写端在 join 前必然关闭；滞留线程若在 `finish` 之后继续读，命中的是已被 `lock().take()` 取走的 sink（§5.11.1 约束 5），只丢弃字节，不会污染已收尾的文件；不在 `shell` 内自建线程或改用异步管道来绕开（异步管道会引入第二套读取模型，收益为零） |

---

## 13 参考

**本仓文档**

- I18 design §3.17（已归档）：审批链路预留、检测分层、`Adjudicate` 门禁；
- [OSCALLDESIGN](../designed/OSCALLDESIGN.md)：Shell 洋葱模型（SafetyBelt / 结构守卫 / 环境隔离 / 命令分类），本文 §2.3 与 §5.10 与其边界划分一致，不重复其条目；
- [agent-sandbox-research](./agent-sandbox-research.md)：后端选型（ADR-1 Windows Token+Job、ADR-2 Linux Landlock、ADR-3 不选 gVisor、ADR-4 fail-closed）；
- [agent-sandbox-reference-analysis](./agent-sandbox-reference-analysis.md)：Codex 四后端与违规检测、OpenHuman 诚实 Noop、DSH 链式探测与升级阶梯；
- [I38g](./I38g-agent-approval-advisor-spec.md)：P10 单向顾问与失败矩阵；
- I50（已归档）：审计词表（本文不改）。

**参考实现**（沙箱后端的借鉴点已移至 [I53 §11](./I53-sandbox-preparation-spec.md)，此处只留 `shell` 侧相关项）

| 项目 | 借鉴点 |
|---|---|
| Codex / Claude Code | `shell` 的流式输出接管、超时杀树、结果回填的窗口预览 |
| DSH | 前台执行与取消的边界；不做后台 job 的理由与本文 OQ8 一致 |
| OpenHuman | 后端不可用时诚实报告，不静默降级 |
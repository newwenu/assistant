# I71 — 工具执行链路加固规格

> 性质：规格（实施约束，不含实施记录）
> 事实来源：`docs/analysis/TOOLCALL-DISPATCH-PARADIGM-ANALYSIS.md`（下称「分析」）§3 现状取证、§4 缺口清单、§6 依赖事实
> 术语唯一出处：`docs/analysis/ORCHESTRATION-CONCEPT-SEMANTIC-TRANSFER.md` §2.5 / §2.6
> 判据来源：`docs/principle/CODE-REVIEW-PRINCIPLE.md`、`docs/principle/CODE-SPLIT-PRINCIPLE.md`
> 编排归属：本规格**不新增编排原语**，不改 `docs/store/I69-agent-orchestration-roadmap.md` 的波次；工具调用节点的接入仍走宿主既有端口（见 §6）

---

## 1 定位

本规格只做一件事：**在不新增第二条执行路径、不改 wire 语义的前提下，把工具调用链路里「已经产 bug」与「下一波必疼」的三处补齐。**

分析给出的九项缺口按「命中频率 × 严重度 ÷ 成本」筛出三项，其余六项见 §4（附触发条件，不排期）。三项的取舍判据：

| 项 | 为什么是现在 | 为什么不是更多 |
|---|---|---|
| W1 取消兜底 | 现状即有资源泄漏缺陷，每次「跑长命令 → 取消」都命中 | 只修泄漏，不改取消语义 |
| W2 调用来源 | 台账已存在两种可信度不同的来源却无法区分（分析 §3.2） | 只加归因字段，不做来源差异化策略 |
| W3 单工具内联预算 | 常见文件读取被迫落盘 + 回读再审批，每轮都可能命中 | 只加自述口子，不重做结果视图 |

---

## 2 W1 取消兜底（缺陷修复，P0）

### 2.1 现状事实

取消在 agent 侧是**丢弃 future**，不是发信号：

| 事实 | 位置 |
|---|---|
| 取消令牌与 `port.execute(...)` 竞争，令牌先到即 `return None`，future 被 drop | `assistant-agent/src/agent/loop_executor/budget.rs::cancellable`（`tokio::select!`，`biased`） |
| `None` 直接走 `cancelled_tool_exec` 收口，不再消费任何工具结果 | `assistant-agent/src/agent/loop_executor/execute.rs` |
| 审批卡片撤回已由 `PendingGuard::drop` 完成 | `assistant-tools/src/runtime/gateway/mod.rs` |

因此**审批等待无需另加取消竞争**：future 被 drop 时挂起项随守卫析构一起撤回。这条更正了分析 §4.2 G4「后果」栏中「卡在审批框的调用不被中断」的表述（见 §4 第 1 条）。

真正的缺陷只有一处：**`agent_shell` 的子进程树不随 future 一起消失。**

| 环节 | 现状 | 后果 |
|---|---|---|
| 杀树句柄 | `KillHandle` 只在第 8 步（`Outcome::TimedOut` / `Cancelled` 分支）显式 `kill()` | future 在 `select!` 处被 drop 时第 8 步永不执行，子进程树继续运行 |
| 收尸任务 | `child` 被移进 `spawn_blocking(move || reap(child))`，`JoinHandle` 被 drop 只 detach 不 abort | 阻塞线程继续 `wait()`，进程存活期间永不返回 |
| 落盘 sink | `Arc<Mutex<Option<Box<dyn ToolResultSink>>>>` 被两个读取任务持有，`FileSpillSink` 无 `Drop` | 半成品落盘文件残留，且无任何引用指向它 |
| 调用级临时目录 | 只在第 4/5/10 步的错误分支与 `finish_sink` 内删除 | drop 路径不经过任何一处，目录残留 |

### 2.2 目标

`agent_shell` 的 future 被 drop 后，**三条资源都必须回收**：子进程树、落盘文件、调用级临时目录。

### 2.3 改动面

| # | 改动 | 位置 | 语义 |
|---|---|---|---|
| W1.1 | 新增 `RunGuard`（持 `KillHandle` + `scratch` 路径，`Drop` 内 `kill()` + `cleanup_scratch()`） | `assistant-tools/src/os/tools/agent_shell/run.rs` | 守卫在第 7 步取到 `kill_handle()` 后立即构造，随 `execute` 函数体存活到返回；drop 路径与正常路径都经过它 |
| W1.2 | `FileSpillSink` 增加 `finished` 标志与 `Drop` 实现：未 `finish` 即删除 `path` | `assistant-tools/src/runtime/storage.rs` | `finish` 首行置 `finished = true`（早退分支同样视为已收尾，行为不变）；`Drop` 只在「从未 finish」时删文件 |
| W1.3 | `cleanup_scratch` 对 `NotFound` 静默 | `assistant-tools/src/os/tools/agent_shell/spawn.rs` | 守卫在正常路径上会二次调用；目录已删不算异常，不得污染日志 |

不变式：

| 编号 | 不变式 |
|---|---|
| R1 | `kill()` 必须幂等：正常路径守卫 drop 时树已收尸，重复调用不得改变结果（`TerminateJobObject` / `kill(-pgid)` 本身幂等，不额外加锁） |
| R2 | 守卫**不持有** `slot`：sink 的归属仍是两个读取任务，删除由 `FileSpillSink::Drop` 在最后一个持有者释放时完成 |
| R3 | 守卫的 `Drop` 不得 `await`：`remove_dir_all` 同步执行，目录只含重定向输出，量级有界 |
| R4 | 正常路径行为逐字节不变：成功 / 超时 / 取消三条既有路径的 `ToolResult`、事件序列、落盘引用都不变 |

### 2.4 验收

| 编号 | 判据 |
|---|---|
| A1 | 取消路径下 `active_processes()` 归零（Windows 后端已提供该探针，见 `SandboxChild::active_processes`） |
| A2 | 取消路径下调用级临时目录不存在 |
| A3 | 取消路径下 sink 已创建的落盘文件不存在（`open_sink` 已产生文件时） |
| A4 | 既有 shell 用例（成功 / 超时 / 取消 / 空输出 / 编码判定）全部不回归 |

---

## 3 W2 调用来源归因（P1）

### 3.1 现状事实

| 入口 | 会话 | 审批 | 边界 | 审计落库 |
|---|---|---|---|---|
| 轮内派发 | 有 | 有 | 有 | `permission_audit`，坐标齐全 |
| ToolBox 直调 | 无 | 无 | 无 | `permission_audit`，`bypassed`，坐标全 `NULL` |
| 编排节点 | 未接线 | — | — | — |

两条入口写同一张表，**只能靠「坐标是否为 NULL」间接推断来源**；这个推断对第三种来源（有会话、有审批、但非轮内派发）立刻失效。

### 3.2 目标

在 `ToolUseContext` 上增加一个封闭集合的来源维度，由它一路带到审计行与追溯面板，**不改变任何决策逻辑**。

### 3.3 改动面

| # | 改动 | 位置 |
|---|---|---|
| W2.1 | 新增 `ToolCallSource { Agent, Toolbox, Orchestration }`，`Default = Agent`，`as_str()` 给出落库串 | `assistant-tools/src/contract/context.rs` |
| W2.2 | `ToolUseContext` 增加 `source` 字段，`#[serde(skip)]`（运行时字段，不进 wire，与分析 §0 命题 13 一致） | 同上 |
| W2.3 | `for_toolbox()` 填 `Toolbox`；Agent 装配点走 `..Default::default()` 自动为 `Agent`，不额外写一行 | `contract/context.rs`、`adapters/tool_exec.rs`（无改动，仅确认） |
| W2.4 | `AuditEntry` 增加 `source: Option<ToolCallSource>`；`None` = 非工具调用写入（授权管理动作） | `runtime/gateway/stores.rs` |
| W2.5 | 网关三条审计构造点填 `Some(context.source)`；`bypassed()` 填 `Some(Toolbox)`；`allowed()` 填 `None` | `runtime/gateway/{mod.rs,stores.rs}` |
| W2.6 | `permission_audit` 增列 `source TEXT`，沿用既有「`pragma_table_info` 守卫 + `ALTER TABLE`」补列循环，不回填存量行 | `assistant-storage/src/db.rs` |
| W2.7 | `INSERT` 增参（`DbPermissionAuditStore`、`write_grant_audit`） | `adapters/permission_store.rs`、`cmd/permission/audit.rs` |
| W2.8 | `PermissionAuditEntryDto` 增 `source: Option<String>`，查询投影增列，跑 `npm run contract:gen` | `assistant-protocol/src/agent.rs`、`cmd/permission/audit.rs` |

不变式：

| 编号 | 不变式 |
|---|---|
| S1 | 来源**不参与**任何策略求值：`ToolGateway::evaluate` 的入参、`ApprovalKey`、审批缓存命中判定都不读它 |
| S2 | 存量行 `source` 为 `NULL`，不得用 `""` 或占位串伪造来源（与分析 §5「不伪造归属」同一原则） |
| S3 | `ToolCallSource` 是封闭集合：新增变体必须同时改 `as_str()`、审计词表说明与前端兜底，不得只改一侧 |
| S4 | 来源进 wire 只到审计 DTO 为止；`ToolUseContext` 不序列化该字段 |

### 3.4 验收

| 编号 | 判据 |
|---|---|
| B1 | 轮内调用落库 `source = agent` |
| B2 | ToolBox 直调落库 `source = toolbox` |
| B3 | 授权管理动作（撤销 / 清除 / 改作用域）落库 `source = NULL` |
| B4 | 存量库升级后旧行为 `NULL`，新写入行有值；补列可重复执行 |
| B5 | 追溯面板查询返回 `source` 字段；前端不展示也不报错（展示归 UI，见 §5） |

---

## 4 W3 单工具内联预算（P2）

### 4.1 现状事实

| 事实 | 位置 |
|---|---|
| 单结果阈值与单轮预算都是全局常量（`50_000` / `200_000` 字符） | `runtime/storage.rs` |
| 判定入口唯一：`select_persist_targets(&[usize])`，只吃尺寸、不吃工具身份 | 同上 |
| 工具无法自述「我的结果天然该小 / 该大」 | `contract/trait.rs` |

后果（可复现的具体路径）：`read` 单次上限为 2000 行 / 1MB，一个 2000 行的源文件通常 60–100k 字符 → 超过 50k 即被迫落盘 → 模型只拿到引用 + 1200 字符预览 → 需要再 `read` 那个落盘文件，而**首次读取落盘文件会弹一次审批**（`read` 的描述已明示）。一次普通读文件因此多一次审批往返。

### 4.2 目标

把「单结果阈值」从全局常量改为「全局默认 + 工具可覆盖」，并只给一个工具启用，**不动单轮预算**。

### 4.3 改动面

| # | 改动 | 位置 |
|---|---|---|
| W3.1 | `Tool` 增默认方法 `inline_output_limit(&self) -> Option<usize>`，默认 `None`（= 用全局阈值） | `contract/trait.rs` |
| W3.2 | 新增 `select_persist_targets_with_limits(sizes, limits)`；`select_persist_targets(sizes)` 改为「全 `None`」的薄委托，既有调用点与单测不变 | `runtime/storage.rs` |
| W3.3 | `ToolPipeline::execute` 在预算判定前按 `tool_name` 回查注册表取上限；`read` 声明 `Some(2 * PER_TOOL_LIMIT_CHARS)` | `runtime/pipeline.rs`、`os/tools/agent_read/mod.rs` |

不变式：

| 编号 | 不变式 |
|---|---|
| T1 | 单轮预算（`PER_ROUND_LIMIT_CHARS`）仍是硬上限：工具覆盖只抬单结果阈值，不得绕过轮预算 |
| T2 | 已自带落盘引用的结果（`shell`）尺寸按 `0` 计，覆盖对其无意义（沿用既有行为） |
| T3 | 回查失败（工具已注销）时按 `None` 处理，不得 panic、不得跳过落盘判定 |
| T4 | 覆盖值是「阈值」，不是「截断」：超阈值仍走落盘 + 预览，不新增截断语义 |

### 4.4 验收

| 编号 | 判据 |
|---|---|
| C1 | `read` 返回 60k 字符结果时不落盘（旧行为为落盘） |
| C2 | `read` 返回 120k 字符结果时仍落盘 |
| C3 | 其它工具行为逐字节不变（默认 `None`） |
| C4 | 单轮多结果时，总内联量仍不超 `PER_ROUND_LIMIT_CHARS` |

---

## 5 不做项

本规格**不**包含下表。每项给出触发条件，满足时另开规格，不在此处预埋。

| 项 | 分析编号 | 不做的理由 | 触发条件 |
|---|---|---|---|
| 延迟暴露与检索式发现 | G1 | 与本次三项零耦合；需要注册表先提供可检索元数据 | 注册表工具数或 MCP server 数使 prompt 工具段成为瓶颈 |
| 并发 / 取消语义自述拆分 | G2 | **非缺陷**：`is_concurrency_safe` 默认回落 `is_readonly()` 属默认取值选择，两个方向都可由工具覆写（`agent_shell` 已覆写为 `false`） | 出现「只读但不可并行」或「非只读但可并行」的真实工具 |
| 结果多视图（日志视图 vs 模型视图） | G3 | 只做阈值自述，不做视图分裂 | 需要按视图分别裁剪同一结果 |
| 取消贯穿到审批与批内排队 | G4 其余部分 | **已由 future drop + `PendingGuard::drop` 覆盖**（§2.1 事实表），无需另加令牌竞争 | 需要为取消产出可回填的终态结果（而非直接丢弃 future） |
| 长时 / 交互式进程工具 | G5 | 新增工具，与链路加固正交 | 出现「构建 / 测试 / 起服务需要分段读或中途输入」的需求 |
| 工具级生命周期钩子 | G6 | 扩展性缺口，当前无消费者 | 出现第二个需要「按工具注入 / 改写」的中间件 |
| 注册表分层与命名空间 | G8 | 唯一涉 `ToolSpec.name` 生成方式的一项，需产品决策 | 接入多个 MCP server 且出现同名工具 |
| 模型可见的上下文预算 | G9 | 体验项，与链路加固正交 | 出现「撞墙才被截断」的体感反馈 |
| 追溯面板展示来源列 | W2 的 UI 部分 | 本规格只保证事实落库与可查询 | 追溯面板需要按来源筛选或分列统计 |

---

## 6 边界

| 编号 | 边界 |
|---|---|
| E1 | 不新增执行路径：三项全部落在既有 `ToolPipeline` / `ToolGateway` / `ToolResultStorage` 内部 |
| E2 | 不改 I69 波次：工具调用节点接入仍复用 `ToolExecutionPort`，本规格只为其前置（W1 取消兜底、W2 来源归因）铺路 |
| E3 | 不污染对话消息行：W1 只在 drop 路径回收资源，不产出 `ToolOutcome`；W2 只写审计表 |
| E4 | 失败不阻断业务：审计写失败、临时目录删除失败一律只记日志（沿用既有约定） |
| E5 | 三项可独立验收、独立回滚：W1 只动 `agent_shell` 与 storage 的 sink 析构；W2 只加字段与列；W3 只加默认方法与一个覆写 |

---

## 7 风险

| 风险 | 处置 |
|---|---|
| 守卫 `Drop` 在 async 线程上同步删目录 | 目录只含重定向输出，量级有界；若实测有卡顿，改为「守卫只 kill，删除交给独立 `spawn_blocking`」 |
| `FileSpillSink::Drop` 删除失败（Windows 句柄未释放） | 只记 `debug`；文件句柄在 `Drop` 内先 `take()` 关闭，再删 |
| 补列在旧库上失败 | 沿用既有守卫写法（先 `pragma_table_info` 查列、再 `ALTER TABLE`），对存量库幂等 |
| W2 的 DTO 新增字段被前端误当作必展示 | DTO 字段为 `Option`，前端类型为 `string \| null`；展示列另开 UI 变更 |
| W3 抬阈值放大单轮上下文 | 轮预算不变（T1），只有单结果可越过 50k，总量仍受 200k 约束 |

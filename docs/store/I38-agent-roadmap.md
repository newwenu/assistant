# I38 — Agent 实施阶段总纲

| 项 | 值 |
|---|---|
| 状态 | 现行（agent 实施的唯一阶段入口） |
| 日期 | 2026-11-04 |
| 关联 | [I18 工具架构决策](./I18-agent-tool-architecture-design.md)、[I18 工具框架规格](./I18-agent-tool-architecture-spec.md)、[I36 运行时决策](./I36-agent-runtime-design.md)、[I36 运行时规格](./I36-agent-runtime-spec.md) |
| 影响范围 | 文档结构；不改变任何代码契约 |

> 本文只回答三件事：**现在处在哪个阶段、下一阶段从哪开始、每份文档归谁管**。
> 每阶段的“怎么做”在对应阶段 spec；“为什么这样设计”在 design/ADR。
> **状态唯一台账**：agent 各阶段的实施状态只在本文 §2 阶段表维护；spec 头表只写文档状态，design 与基线 / 验证文档不写进度（详见 §7）。

---

## 1 阅读顺序

1. 本文：阶段、状态、门禁。
2. 当前阶段的 spec（见 §4 文档地图）。
3. 决策记录：I18 design（工具架构）、I36 design（运行时）。
4. 专项：`OSCALLDESIGN.md`（Shell 洋葱）、I18b（ToolBox 桥接）、I30（toolcall 剥离）、I31（阶段一工具框架）。

---

## 2 阶段表

| 阶段 | 目标 | 权威 spec | 状态（2026-11-05） | 前置 |
|---|---|---|---|---|
| **P0 基线** | toolcall 从 chat 剥离；工具词汇层、Registry、`exec::invoke`、ToolBox 桥接 | I30 A 段、I31 T1/T2、I18b | ✅ 已完成 | — |
| **P1 框架收口** | 共享路径安全 `os/path.rs`；`fs_link` 迁移与权限词汇修正 | I31 T3 | ✅ 已完成（2026-11-04） | P0 |
| **P2 工具控制层** | Policy / Gateway / Pipeline / 审批键 / 校验 / 并发 / 结果持久化 | I38a | ✅ 已完成（2026-11-05） | P1 |
| **P3 运行时内核** | `kernel/` 共用内核、端口、状态机、取消代际、双层超时、chat 等价迁移 | I36 spec、[I36a](./I36a-agent-kernel-baseline.md)、[I39](./I39-compaction-chat-merge-spec.md) | ✅ 已完成（C1–C11，2026-11-05；chat / agent 两条链路均已改调内核，H1–H9 全部收口；压缩共享边界（I39）已落地：互斥源 / 投影 / 刷新点 / 失败通道统一） | P0 |
| **P4 Agent Loop 闭环** | RoundExecutor 多迭代循环；tool call 累积器；`ChatOptions.tools` 注入 | I38b | ❌ 未开始（规格已定稿，2026-11-05） | P2、P3；I30 B 段决策 |
| **P5 OS 工具能力** | `fs_*`、`shell_exec`、`ask_user` 逐个落地 | I38c | ❌ 未开始 | P1、P2 |
| **P6 前端与模式** | 五态 ToolCallBubble、审批 UI、AgentMode / 对话级工具配置；chat / agent 链路显式切换（新建会话按 `mode.kind` 选路，会话内锁定） | I38d | ❌ 未开始 | P4、P5 |
| **P7 MCP 外部工具** | MCP 客户端、进程管理、`mcp__{server}__{tool}` 桥接 | I38e | ❌ 未开始 | P2、P5 |

**P0 的完成判据**（已复核代码）：`assistant-agent` 与前端不再持有 chat tool call 累积器与渲染组件；`ToolRegistry` 具备 `<=` 语义与受众过滤；`os_tool_exec` 已走 `exec::invoke`；生产注册表只有 `fs_link`（`TOOLBOX_ONLY`）。

### 2.1 P8+ 演进候选（不进入当前门禁）

参考项目（DeepSeek Harness、Codex、Goose、OpenBitFun、OpenHuman）都在单 agent 闭环之上提供了会话级编排能力。本表只登记候选，**不改变 §2 的 P0–P7 门禁**；任何候选开工前必须先按 §7 第 3 条修改本文 §2/§3，再写 spec。

| 候选 | 目标 | 参考 | 前置 |
|---|---|---|---|
| **P8.1 子代理** | `spawn_agent` 作为工具接入 ToolPipeline；默认 `max_depth = 1`、只读工具、结果摘要回父 | DSH `ctx.subagents`；Codex collaboration tools；Goose `SubagentRunner` | P4、P5；I36 D16-D18 |
| **P8.2 Worker 会话** | 可继续的子会话、roster / 任务板、steer / cancel、后台活动投影 | DSH continuable subagent + Agent Teams；OpenBitFun `subagentProjection`；OpenHuman `spawn_async_subagent` | P8.1、P6 |
| **P8.3 Router** | 会话之上的全局协调、跨会话记忆、事件入口与任务分派 | DeepSeek Harness Super-agent 的 Router / Worker | P8.2；P9 的持久化前置 |
| **P9 continuity** | 重启恢复、Watchdog、事件唤醒与主动执行 | DSH goal / schedule / webhook；OpenHuman `reaper.rs` | 单独 ADR；I36 D13b |

**范围约束**：

- P8.1 只做「父 turn 内的一次性委派」，不做跨会话路由。
- P8.2 的子会话必须继承 I36 D18 的取消树；父 turn 结束不得留下孤儿 worker。
- P8.3 的 Router 是**会话之上的新层**，不得把单会话内核改造成全局调度器。
- P9 的持久化运行不得绕过 I36 D13a 的现行边界，必须单独决策。

---

## 3 依赖与并行

```
P0 ──► P1 ──► P2 ──┬──► P4 ──┐
                   │         ├──► P6
                   └──► P5 ──┘
P0 ──► P3 ─────────► P4
P2 + P5 ───────────► P7
```

> P8+ 候选不在上图；其前置关系见 §2.1，未立项前不参与 P0–P7 的依赖计算。

| 关系 | 说明 |
|---|---|
| P1 → P2 | `validate_input` 路径检查必须复用 `os/path.rs`，否则两个入口的结论会分叉（I31 §5） |
| P2 → P4 | Pipeline 是 RoundExecutor 的唯一工具执行入口，未就绪前不得驱动工具 |
| P3 → P4 | TurnRunner 提供 turn 生命周期、状态机、预算与事件；P4 只替换迭代循环体 |
| P1 + P2 → P5 | 路径安全与控制层先于任何破坏性工具；`shell_exec` / `fs_delete` 未过 P2 不得进入生产注册表 |
| P4 + P5 → P6 | 前端五态与审批 UI 需要真实工具与事件；模式配置需要 RoundExecutor 的白名单解析 |
| P2 + P5 → P7 | MCP 工具复用同一 Policy/Gateway/Pipeline，安全标记按 External 推断 |

**可并行**：P1 完成后，P2 与 P3 可并行（一个改 `assistant-tools`，一个改 `assistant-agent`，无文件交叠）。P5 在 P2 完成后可与 P3/P4 并行。

---

## 4 文档地图

| 主题 | 文档 | 性质 |
|---|---|---|
| 阶段与门禁 | 本文 | 总纲 |
| 工具词汇层 / Tool trait / Registry / Tauri 桥接 | [I18 spec](./I18-agent-tool-architecture-spec.md) | 实施规格（Step 0-3、8） |
| 工具架构决策（分层、权限、双入口、安全原则） | [I18 design](./I18-agent-tool-architecture-design.md) | ADR |
| 工具控制层 | [I38a](./I38a-tool-control-layer-spec.md) | 实施规格 |
| Agent Loop / 累积器 / tools 注入 | [I38b](./I38b-agent-loop-spec.md) | 实施规格 |
| OS 工具逐个实现 | [I38c](./I38c-os-tools-spec.md) | 实施规格 |
| 前端五态 / 审批 UI / Agent 模式 | [I38d](./I38d-agent-frontend-modes-spec.md) | 实施规格 |
| MCP 外部工具 | [I38e](./I38e-mcp-tools-spec.md) | 实施规格 |
| P8+ 演进候选（子代理 / Worker / Router / continuity） | 本文 §2.1 | 候选清单 |
| 运行时内核（turn / 状态机 / 端口 / chat 迁移） | [I36 spec](./I36-agent-runtime-spec.md) | 实施规格 |
| 运行时决策（D1-D18、参考项目提取） | [I36 design](./I36-agent-runtime-design.md) | ADR |
| 内核能力基线（能力矩阵 / 参考实现对照 / 扩展点 / 门禁） | [I36a](./I36a-agent-kernel-baseline.md) | 基线文档 |
| 内核一致性验证（确定性要求 / 验证矩阵 / 实检缺口） | [I36b](./I36b-agent-kernel-conformance-spec.md) | 验证规格 |
| 压缩与 chat 的共享边界（共享积木 / 保留独立编排） | [I39](./I39-compaction-chat-merge-spec.md) | 专项规格 |
| P3 任务级进度（C1–C11 勾选） | [I36 tasklist](./I36-agent-runtime-tasklist.md) | 任务清单（唯一任务级进度） |
| P4 任务级进度（C1–C9 勾选） | [I38b tasklist](./I38b-agent-loop-tasklist.md) | 任务清单（唯一任务级进度） |
| toolcall 从 chat 剥离 | [I30](./I30-phase1-toolcall-separation-spec.md) | 阶段一 A 段 |
| 阶段一工具框架 | [I31](./I31-phase1-tool-framework-spec.md) | 阶段一 |
| ToolBox 桥接 | [I18b](./I18b-tool-unified-bridge-spec.md) | 专项规格 |
| Shell 洋葱模型 | [OSCALLDESIGN](../designed/OSCALLDESIGN.md) | 专项设计 |

本表只回答「谁管什么」，**不写进度**（进度见 §2）；各文档内部 Step / T / C 编号的勾选由该文档自身维护，属规格内部条目，不是阶段状态。

**单一权威源规则**：

- 工具类型、Trait、Registry、桥接 → I18 spec；控制层 → I38a；循环 → I38b；工具实现 → I38c；前端 → I38d；MCP → I38e；运行时 → I36 spec；内核一致性判定（DT/V/H）→ I36b。
- 同一契约不得在两份 spec 中各写一份。引用时写「见 X 文档 §Y」，不复制类型定义。
- I18 spec 已收敛为框架层规格，不再承载 Step 4-7、9-10。

---

## 5 冲突决议（2026-11-04）

历史文档之间存在以下冲突，本文给出唯一结论；各文档已按此修正。

| # | 冲突 | 结论 |
|---|---|---|
| R1 | RoundExecutor 落点：I18 spec 写 `assistant-agent/src/round_executor.rs`，I36 写 `agent/` | 统一落在 `assistant-agent/src/agent/`：`round_executor.rs`（循环）、`tool_call.rs`（累积器）、`mod.rs`（trait 与导出）。`assistant-agent/src/kernel/` 不得引用工具类型 |
| R2 | RoundExecutor 职责：I36 design 称其为迭代循环，I36 spec 的主流程把循环写在 TurnRunner | **P3 过渡形态**：TurnRunner 内部跑单迭代循环，`round_executor = None`。**P4 起**：迭代循环（模型调用 → 工具执行 → 回填 → 下一迭代）整体迁入 RoundExecutor；TurnRunner 只保留 turn 生命周期、状态机、预算、阶梯、落库、事件与收尾。P4 的入口固定为 `RoundExecutor::run_iterations` |
| R3 | `os_tool_exec` 示例仍写 `tool.execute()` + `map_err` | 以代码为准：`exec::invoke(tool, input, context, DEFAULT_TOOL_TIMEOUT)`，`Err` 通道已在 `invoke` 内归一化 |
| R4 | 工具阶段门禁：I18 design 的 Phase 3 与 I31 的「暂缓」表述不一致 | 以本文 P5 为准：P1 + P2 完成后才可新增破坏性工具；`shell_exec` / `fs_delete` 必须与 P2 的 Policy/Gateway 同期可用 |
| R5 | 前端字段命名：I18 spec Step 10 的 `AgentMode` 同时出现 snake_case 与 camelCase | wire / 前端一律 camelCase（`maxToolTier`、`approvalLevel`、`enabledTools`、`disabledTools`）；Rust 存储 struct 与 DB 列保持 snake_case。转换在 DTO 层完成 |
| R6 | 工具实现状态散落在 I18 spec Step 2 | 工具清单、Schema 与实施状态统一迁移到 I38c；I18 spec 不再维护逐个工具状态 |
| R7 | 终止式压缩与续跑式压缩的归属 | 压缩生命周期（事件、hook、trigger）归 I36 spec Step 6；「终止式 → 续跑式」的切换点归 P4（I38b），不另写第三份规格 |
| R8 | `ToolResult` 双通道（`result_for_assistant` / `error_kind`）归属 | 类型扩展归 P2（I38a §3）；P0 的 `ToolResult { ok, data, error }` 保持不变，P2 以新增可选字段落地，存量工具零改动 |
| R9 | `ToolAudience` 默认值：早期为迁移便利用 `BOTH` | **2026-11-04 收紧为 `NONE`（fail-closed）**：未显式声明受众的工具对两个入口都不可见。唯一存量工具 `fs_link` 已显式声明 `TOOLBOX_ONLY`，行为不变；新工具必须在 `audience()` 中显式声明 |
| R10 | 子代理 / 多 agent 是否属于当前阶段 | **P0–P7 不实现**。P4 只预留 `agent_id` / `parent_turn_id` / `delegation_depth` 三个可选字段（I36 D16-D18），实现归 P8.1；不得在 RoundExecutor 内写 `if is_subagent` |
| R11 | Super-agent 的 Router / Worker 与现有单会话 turn | **不作为当前架构目标**。P0–P7 是 Worker 层基础；P8.2/P8.3 才补子会话与 Router。跨重启的 persistence / Watchdog 归 P9，单独 ADR（I36 D13b），不得直接叠加后台进程 |
| R12 | 压缩与 chat 的共享边界 | **不属 R7 的三份边界**：生命周期语义仍归 I36 spec Step 6，续跑式切换点仍归 P4。共享到什么程度归 [I39](./I39-compaction-chat-merge-spec.md)。结论：**压缩不收编为 `TurnSpec` 第三种实例**，保留 `execute_compaction` 独立编排；只共享 `TurnTasks` 互斥源、`stream::consume`、`TurnEmitter`、usage 构造、`project_chat_messages` 投影、`emit_stream_finished` 刷新点与 `chat-delta.error` 失败通道 |

---

## 6 阶段门禁

每阶段只有在**退出条件**全部满足后才可标记完成；细节验收在该阶段 spec 的「验收标准」章节。

| 阶段 | 退出条件（摘要） |
|---|---|
| P1 | `os/path.rs` 有单测；`fs_link` 无私有路径实现；`permission_intents` 仅用 `read/edit/delete/execute`；`cargo test -p assistant-tools` 全绿 |
| P2 | Policy 单测覆盖 `most_restrictive` / last-match-wins / Ceiling 拒绝 Allow；Gateway 区分 Allow/Ask/Deny；Pipeline 顺序固定为 `find → validate → gateway → execute → persist`；无 tauri 依赖 |
| P3 | `cargo tree -p assistant-agent` 无 tauri；chat 对比测试（事件序列 / seq / 落库 / 错误文案）逐条一致；关闭窗口后 turn 可跑完 |
| P4 | 多迭代闭环可跑通；`max_iterations` / 收尾调用 / 空响应重试 / 工具结果回填均有测试；`ChatOptions.tools` 在三个 adapter 生效 |
| P5 | 每个工具独立 spec 验收；破坏性工具只在 P2 就绪后注册；ToolBox 与 Agent 两入口共享 `execute()` 内安全守卫 |
| P6 | 五态渲染与审批闭环可用；`resolveAvailableTools` deny-by-default；跨窗口 phase 不漂移；链路选路按 `mode.kind` 分流且会话内不可切换 |
| P7 | MCP server 启停/重连；工具命名与安全标记推断符合 I18 design §3.8；External 工具的 Policy 生效 |

---

## 7 状态维护规则

**唯一台账**：agent 各阶段的实施状态**只在本文 §2 阶段表的「状态」列维护**。其他文档一律不设「实施状态 / 现状 / 进度」列或段。

1. 阶段级进度：只改 §2 阶段表；写「阶段名 + 一句话结论 + 日期」，不写任务明细。
2. 任务级进度：只在对应阶段 tasklist 勾选（如 I36 tasklist C1–C11、I38c 工具清单）。tasklist 随阶段关闭而冻结，不再回写正文。
   - tasklist **不管理提交**：不写「单独提交 / 提交拆分 / 提交信息标注 / 回滚批次」这类条目。提交粒度由开发者按代码依赖与可编译性现场决定，属工程操作，不进入文档台账；tasklist 只回答「代码改没改、验收跑没跑」。
3. spec / design / 基线 / 验证文档**不记录进度**：
   - spec 头表的「状态」只表示**文档状态**（现行 / 草稿），不写「已实现 / 部分完成」；
   - design（ADR）永不记录进度；
   - I36a 只写「能力 → 归属阶段」，I36b 只写「判定方式」，都不写判定结果。
4. 需要表达「某能力尚未接线」时，写法固定为「归属 P4，见 tasklist 条目 / I36b H7」，**不新增第三处状态表**。
5. 新增阶段必须先改本文 §2/§3，再写 spec；禁止先写 spec 后补阶段号。
6. 阶段完成时保留 spec（不删除），只在本表标注日期与结论，不改 spec 头表状态。
7. 任何跨阶段契约变更（类型、事件名、落库格式）必须同时更新受影响阶段的 spec，不允许只改当前阶段。
8. 禁止在 §4 文档地图或阶段汇总类清单里再列「状态」列——这类表只回答「谁管什么」，不回答「做到哪了」；任务级清单（tasklist）是唯一例外，见第 2 条。
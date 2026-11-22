# I38 — Agent 实施阶段总纲

| 项 | 值 |
|---|---|
| 状态 | 现行（agent 实施的唯一阶段入口） |
| 日期 | 2026-11-04 |
| 关联 | I18 工具架构决策（已归档）、I18 工具框架规格（已归档）、I36 运行时决策（已归档）、I36 运行时规格（已归档） |
| 影响范围 | 文档结构；不改变任何代码契约 |

> 本文只回答三件事：**现在处在哪个阶段、下一阶段从哪开始、每份文档归谁管**。
> 每阶段的“怎么做”在对应阶段 spec；“为什么这样设计”在 design/ADR。
> **状态唯一台账**：agent 各阶段的实施状态只在本文 §2 阶段表维护；spec 头表只写文档状态，design 与基线 / 验证文档不写进度（详见 §7）。

---

## 1 阅读顺序

1. 本文：阶段、状态、门禁。
2. 当前阶段的 spec（见 §4 文档地图）。
3. 决策记录：I18 design（工具架构）、I36 design（已归档，运行时）。
4. 专项：`OSCALLDESIGN.md`（Shell 洋葱）、I18b（ToolBox 桥接）、I30（已归档，toolcall 剥离）、I31（已归档，阶段一工具框架）。

---

## 2 阶段表

| 阶段 | 目标 | 权威 spec | 状态（2026-11-09） | 前置 |
|---|---|---|---|---|
| **P0 基线** | toolcall 从 chat 剥离；工具词汇层、Registry、`exec::invoke`、ToolBox 桥接 | I30 A 段、I31 T1/T2、I18b | ✅ 已完成 | — |
| **P1 框架收口** | 共享路径安全 `os/path.rs`；`fs_link` 迁移与权限词汇修正 | I31 T3 | ✅ 已完成（2026-11-04） | P0 |
| **P2 工具控制层** | Policy / Gateway / Pipeline / 审批键 / 校验 / 并发 / 结果持久化 | I38a | ✅ 已完成（2026-11-05） | P1 |
| **P3 运行时内核** | `kernel/` 共用内核、端口、状态机、取消代际、双层超时、chat 等价迁移 | I36 spec（已归档）、[I36a](./I36a-agent-kernel-baseline.md)、I39（已归档） | ✅ 已完成（C1–C11，2026-11-05；chat / agent 两条链路均已改调内核，H1–H9 全部收口；压缩共享边界（I39）已落地：互斥源 / 投影 / 刷新点 / 失败通道统一） | P0 |
| **P4 Agent Loop 闭环** | RoundExecutor 多迭代循环；tool call 累积器；`ChatOptions.tools` 注入；停止钩子三维预算；阶梯 ③④⑤ 与中间件链（② 不随 P4）；审批默认 fail-closed 装配 | I38b | ✅ 已完成（2026-11-06；C1–C6、C8（除 C8.6）、C9 全部完成，`cargo test -p assistant-agent` / `assistant-providers` / `assistant-storage` 全绿，`clippy -D warnings` 与 `cargo fmt --check` 通过。**C7 端到端批次整体跳过**（用户决策）：`agent_loop_e2e.rs` / `agent_loop_dialect.rs` 未编写，C7.1–C7.17 与 C8.6 不计完成，端到端链路无自动化覆盖为已知缺口；组件级等价覆盖见 tasklist C7 说明） | P2、P3；I30 B 段决策 |
| **P5 OS 工具能力** | `agent_*.rs` 九个文件工具（注册名 `read` / `ls` / `glob` / `grep` / `write` / `edit` / `mkdir` / `move` / `delete`）、`ask_user` 逐个落地；**`shell` 暂不提供**（I38c §13，已归档）；白名单工具解析 `resolve_available_tools` 替换 P4 的 `RegistryToolCatalog`（deny-by-default + 显式白名单/黑名单）；清理 P4 接入代码（`ToolCatalog` trait、`RegistryToolCatalog`、`TurnSpec.tools` 类型收窄） | I38c | ✅ 已完成（2026-11-06；C1–C4、C6、C6b、C7、C8 全部完成，`cargo test --workspace` 全绿，`cargo clippy --workspace --all-targets -- -D warnings` 与 `cargo fmt --check` 通过，`npm run contract:check` 幂等。`ask_user` 不属 P5 退出条件，阻塞点登记在 I38c §9（已归档），归 P6；`shell` 按 §13 暂不提供） | P1、P2 |
| **P6 前端与模式** | 五态 ToolCallBubble、审批 UI、AgentMode / 对话级工具配置；chat / agent **同会话内可切换**（每次发送按会话 `mode.kind` 选路，默认 chat，turn 进行中禁止切换）；**共同视图 + 双布局**（`ConversationView` + `ChatLayout` / `AgentLayout`）；**审批卡片携带用户判断信息**（单个 `ApprovalContext` 包裹：L0 `tier`/`behavior` + L1 `Policy::explain` 原因 + L2 `preview_for_approval` 量级预演 + `extensions` 后端检测出口，I38d §3.2.1（已归档）） | I38d | ✅ 已完成（2026-11-07 开工，2026-11-09 关闭）：开放问题全部定稿——`alwaysApprove` 跨会话持久 + 设置页撤销入口、审批等待计入墙钟（P4 D12）、`workspace_root` 用全局配置（会话级绑定已提取为独立阶段 P12，见 §2，2026-11-09）、审批超时默认 120s；`ask_user` 归 P6-C8 且**不属主体退出条件**。C1（DTO 契约）→ C1a（模式收敛）→ C1b（链路选路）→ C1c（审批信息装配）→ C1d（共同视图 + 双布局）→ C2–C7 → C8（`ask_user`）代码与单测全部落地，`contract:gen` / `clippy -D warnings` / `fmt --check` / `npm run lint` / `typecheck` / `vitest` 均通过；2026-11-07 决策调整：**会话内锁定改为同会话可切换**，C1b.4 锁定项作废；2026-11-08 C5a 流式分层重构落地（`stream/transport.ts` / `stream/turnOrchestrator.ts` / `chatTurn` + `agentTurn` 双策略 / `agent/agentSegments.ts`，`chatOrchestrator.ts` 已删除），相关 4 个测试文件 79 项与 `typecheck` 通过，A27 满足；A26 根因（turn 内 `active_branch_id` 停在首行导致全量刷新缺行、前端合并丢弃同轮后续消息，切换分支可恢复）已修复——追加 `assistant`/`tool` 行即推进活动分支叶子（`advance_active_branch`），存储层新增 2 个单测、`cargo check` 与 `typecheck` 通过，待实机回归；2026-11-09 C9.5a / C9.5b 落地：P5-R2 修改组（`write` / `edit` / `mkdir` / `move`）与 P5-R3 危险组（`delete`）先后接入生产注册（`tool_setup` 显式调用 `register_write_tools` / `register_dangerous_tools`，A2 已于同日通过）；同日 C9.6 / C9.6a 实机回归通过（同会话模式切换、切回 agent 复用配置、重启后装配恢复、审批三态与超时置灰），A2 / A23 满足；C1a.10 自定义双形态行为验证 2026-11-09 实机通过，A24 满足；A28 满足（修改组与危险组均已注册）。**推迟验证项**：C2.4 / C9.6b「多窗口同一会话 phase 一致」——用户决策（2026-11-09）推迟至后续阶段，当前构建无可控的双窗口同会话入口，不阻塞本阶段完成；代码侧后端 `session-state-changed` 与前端订阅均已接线 | P4、P5 |
| **P7 MCP 外部工具** | MCP 客户端、进程管理、`mcp__{server_id}__{tool}` 桥接；`mcp_servers` 配置与精确审批键 | I38e | ✅ 已完成（2026-11-14；C1–C6 全部完成：C5 端到端 11 项全绿（`tests/mcp_e2e.rs` + Rust 夹具二进制 `dev_mcp_server`，无解释器依赖），C6 最小配置页（`settings/mcp/`，列表 / 增删改 / 启停 / `env` 掩码 / 隔离边界文案 / i18n），`cargo test`（`assistant-app`：lib 74 项 + 集成 84 项）全绿、`cargo test --test mcp_e2e` 11 项全绿、`npm run contract:check` 幂等、`typecheck` / `lint` / `vitest` 650 项通过；任务级进度见 I38e tasklist（已归档）。**范围外登记**：turn task panic 无终态事件（`cmd/agent/mod.rs`），见 tasklist 文末） | P2、P5 |
| **P10 AI 审批顾问（后续轴）** | 在策略与人工审批之间插入单向 AI 顾问：P10-a `EscalateOnly` → P10-b `RelaxOnly`；`Adjudicate` 需 L4 沙箱 ADR | I38g | ❌ 未开始（规格已就绪，2026-11-09；不进入 P0–P7 门禁） | P6 |
| **P12 工作区（后续轴）** | 工作区一级概念：`conversations.workspace_root` 会话绑定（**唯一来源，空串 `''` = 未配置、列 `NOT NULL`，不占用 `null`，无默认层**）+ `workspaces` 目录簿（仅展示，不参与解析）；唯一解析点 `resolve_workspace(conversation_id)` 与四处消费点同源；ToolBox 不解析工作区；遗留全局值直接作废（不迁移）；会话内切换（不必进设置页）；切换即失效该会话会话级授权；解析结果同时作为未来 MCP `roots` 的唯一输入 | I38h | ✅ 已完成（2026-11-09；代码与单测全绿，`contract:gen` / `typecheck` / `vitest` 通过；实机验收 C4.13 / C6.11 待补，不阻塞阶段关闭） | P5、P6 |
| **P13 工具结果紧凑信封（compact）** | 把「一次工具结果长什么样」收敛为单一展示投影：三段信封（head / body / tail）、省略三态、口径分离、保留策略分裂（`head` / `tail` / `headTail`）、落盘生命周期（会话命名空间 + 启动年龄清理）、`compact` 增量 wire 字段（`Started.argsPreview` / `Finished.compact` / `Failed.compact` / `MessageToolCallDto.compact`）、历史重载重建与字节相等；**不改变 turn 事件语义、不改压缩投影与审批链** | I46 紧凑消息格式设计（参考记录，信封与门禁仍有效） | ✅ 已完成（2026-11-14；C1–C10 全部勾选，门禁 1–15 有测试映射，`cargo test --workspace` / clippy / fmt / typecheck / lint / vitest / `contract:check` 全绿；任务级进度见 I46 compact tasklist（已归档）） | P2、P5；I46 节点布局 P1 门已由本阶段关闭 |
| **P14 沙箱执行层与 `shell` 工具** | 把「声明式权限」闭环成「内核强制边界」：`SandboxBackend` 抽象 + 平台后端（Windows Token+ACL+Job / Linux Landlock / macOS Seatbelt）+ 链式探测与 fail-closed；`shell` 工具（`execute` 意图、命令级审批键、进程级超时与杀进程树、输出落盘与回读）；审批卡片显示实际隔离强度 | [I52](./I52-shell-sandbox-spec.md)（阶段）· [I56](../achievement/I56-shell-tool-spec.md)（工具） | 🚧 进行中（2026-11-18：C1 审批链前置与 C4b 输出存储/回读已验收（A6–A8、A19–A23 通过）；C2–C4 沙箱准备由 I53 交付；下一批 C5 工具本体，规格见 I56；任务级进度见 [I52 tasklist](./I52-shell-sandbox-tasklist.md)） | P2、P5、P6、P12 |
| **P15 Turn 观测面** | 一轮 agent turn 的 12 个既有节点出声（单一 `agent-node-event` + 7 值节点状态闭集 + 三投影），供主窗口按 `(conversationId, turnId)` 渲染 orb agent 的完整执行过程；**只读事实、不参与控制**（观测 sink 不作控制通道；将来的编排器经独立控制端口接入，见 I55 §11）、不改既有事件语义 | I55（已归档） | ✅ 已完成（2026-11-19）：V0–V2 全部交付（后端 12 节点出声 + 前端投影与渲染），V3 回放受闸未开工；观测轴只读既有事实、不参与控制，不阻塞任何在途阶段；规格与任务清单已归档 | P4（`TurnObserver` 已存在）、P6（`transport` 已接线） |
| **P16 长行读取（有界水平窗口）** | 把 `read` 的「无界」收口成「有界 + 如实报告」，**不新增任何读取侧状态**：扫描宽限改字节口径 `EOF_GRACE_BYTES`（8 MiB）+ **整行计费**（预算耗尽时即使窗口未满也停）；超长行带截断标记、`data` 增 `truncatedLines`；footer 增 `[long-lines]`；`ReadReceipt` 增 `truncated_lines` / `line_window_max_chars` 并驱动 `edit` fail-closed 闸门；**不做**行内水平窗口（`column` / `maxLineChars` 已移交 P17，I57 §0.1）、行索引 / seek / 精确 `totalLines`（I57 §9 L1 / L3 / L6 / L7 是边界，不是缺陷） | I57（已归档） | ✅ 已完成（2026-11-18；T1–T4 全部勾选，G1–G8 / G10–G12 有测试映射，`cargo test -p assistant-tools` 全绿、clippy 无警告、fmt 无 diff；P17 的 T2–T6 前置解除；任务级进度见 I57 tasklist（已归档）） | P5、P13 |
| **P17 超级工具（能力发现与索引型读取）** | 新增三个工具：`request_tool`（`Tool::manual()` + 本轮可用集透传，按需返回清单与手册，**不改变本轮工具集**）、`super_read`（落盘行索引 + 按行 `seek` + 精确 `totalLines` + `unit: char\|byte`）、`super_edit`（行地址区间替换 + `expected` 校验，不做子串匹配）；索引目录 / revision 失效 / 单飞 / 容量淘汰；**契约对齐、实现独立**（I58 §0.3）：B 自写扫描 / 渲染 / 索引，只对齐 P16 的行格式字面量、footer 标记集、`data` 键名与回执字段，`data` 只增 `index` 一项并回写 I57 §3.3 | I58（已归档） | ✅ 已完成（2026-11-19；T1–T8 全部勾选，G1–G23 逐条有测试映射：G20 渲染层门禁落在**源码结构 + CSS 声明**层（`chat/tool/__tests__/panelStructure.test.ts` 11 条，仓库无 jsdom / testing-library，不为此引入新依赖），失败矩阵 F1–F14 逐条映射（F1 / F2 / F3 走公开入口、F4 文案在判据层钉住，其余在对应工具或判据层断言）；升级通道 G4·G21·G22·G23 由 `src-tauri/tests/tool_escalation.rs` 11 用例 + `assistant-agent` 循环侧 3 用例 + `request_tool` 工具侧 10 用例覆盖；`cargo test --workspace` / `clippy` / `fmt --check` / `vitest` / `tsc --noEmit` 全绿，`npm run contract:gen` 对两个生成文件零 diff（本阶段未改 DTO）；任务级进度与验收记录见 I58 tasklist（已归档）） | P2、P4、P5、P13（格式字面量对齐 I57，非实现依赖） |

**P0 的完成判据**（P0 时点复核；后续 P5/P6 新增工具不改变该判据的时点含义）：`assistant-agent` 与前端不再持有 chat tool call 累积器与渲染组件；`ToolRegistry` 具备 `<=` 语义与受众过滤；`os_tool_exec` 已走 `exec::invoke`；P0 时点生产注册表只有 `fs_link`（`TOOLBOX_ONLY`）。

### 2.1 P8+ 演进候选（不进入当前门禁）

参考项目（DeepSeek Harness、Codex、Goose、OpenBitFun、OpenHuman）都在单 agent 闭环之上提供了会话级编排能力。本表只登记候选，**不改变 §2 的 P0–P7 门禁**；任何候选开工前必须先按 §7 第 5 条修改本文 §2/§3，再写 spec。

P8.1 规格已留档但**暂缓接入**（可选备选，见本表末行）；P8.2 仍以 P8.1 为前提，P8.1 暂缓期间 P8.2 不得开工。P10 已立项（规格 [I38g](./I38g-agent-approval-advisor-spec.md)，后续轴）；P15 **已完成**（2026-11-18 立项、2026-11-19 V0–V2 交付并归档，见 §2，规格 I55（已归档），观测轴：只读既有事实、不参与控制，范围约束见本节末）。P8.2–P9 属「会话编排」轴，P10 属「审批链」轴，P11 属「工具能力」轴，P12 属「环境（工作区）」轴，P15 属「观测」轴，五条轴不互相前置。

工作区（原 I38d §4.9（已归档）的推迟项，早期口径挂 P8.2）已于 2026-11-09 **提取为独立阶段 P12**（见 §2，规格 I38h（已归档））：它只依赖 P5 + P6，与 P8.1 无关，不应被 P8.1 的暂缓连带阻塞。

| 候选 | 目标 | 参考 | 前置 |
|---|---|---|---|
| **P8.2 Worker 会话** | 可继续的子会话、roster / 任务板、steer / cancel、后台活动投影 | DSH continuable subagent + Agent Teams；OpenBitFun `subagentProjection`；OpenHuman `spawn_async_subagent` | P8.1、P6 |
| **P8.3 Router** | 会话之上的全局协调、跨会话记忆、事件入口与任务分派 | DeepSeek Harness Super-agent 的 Router / Worker | P8.2；P9 的持久化前置 |
| **P9 continuity** | 重启恢复、Watchdog、事件唤醒与主动执行；**后台任务**（进程归属上移，改动面见 [I52-Ext](./I52-ext-background-task.md)，未立项） | DSH goal / schedule / webhook；OpenHuman `reaper.rs` | 单独 ADR；I36 D13b |
| **P11 多编码工具支持** | 文件工具（`read` / `write` / `edit` / `grep`）支持非 UTF-8 文本：自动检测（BOM → UTF-8 → 二进制 → chardetng）、按原编码写回、footer 一次性呈现、`grep` 显式 `encoding` + 零命中探测 | ripgrep `searcher`（`DecodeReaderBytes` / `Encoding` / `binary_detection`）；encoding_rs_io；helix `document.rs`（BOM → chardetng → 按原编码保存） | P5（工具面已上线）；规格见 I38c §14（已归档）；任务见 C9（已归档）；**C9 已全部完成（2026-11-07）** |
| **P8.1 子代理（可选，暂缓接入）** | `spawn_agent` 作为工具接入 ToolPipeline；父 turn 内一次性只读委派；`max_depth = 1`、结果摘要回父；规格已留档于 [I38f](./I38f-agent-subagent-spec.md)，**当前不进入 §2 / §3** | DSH `ctx.subagents`；Codex collaboration tools；Goose `SubagentRunner` | P4、P5；I36 D16-D18；**恢复实施前需重新立项** |

**范围约束**：

- P8.1 只做「父 turn 内的一次性委派」，不做跨会话路由。
- P8.2 的子会话必须继承 I36 D18 的取消树；父 turn 结束不得留下孤儿 worker。
- P8.3 的 Router 是**会话之上的新层**，不得把单会话内核改造成全局调度器。
- P9 的持久化运行不得绕过 I36 D13a 的现行边界，必须单独决策；后台任务的进程归属 / 沙箱上下文 / 输出 sink 改动面见 [I52-Ext](./I52-ext-background-task.md)，不得在 P14 内预留。
- P10 只允许单向顾问形态（`EscalateOnly` / `RelaxOnly`）；`Adjudicate` 需先有 L4 内核级沙箱的独立 ADR，不得在现有 L2/L3 检测层上直接启用（I18 design §3.17.1）。
- P11 只做文件工具的编码支持：不引入 `iconv` / 外部转码进程 / 新依赖，不做逐文件编码猜测（`read` 是唯一编码发现点，`edit` / `write` 只消费回执），不做大文件转码写回（I38c §11 OQ7，已归档）。UTF-8 路径行为不变，唯一例外是 §14.3 的二进制检测顺序修正。
- P14 只做「沙箱边界 + `shell` 工具」：沙箱**不替代**审批（`execute` 仍走 Policy/Gateway 求值），命令字符串黑名单**不是边界**（禁止作为拦截依据），不提供无沙箱执行路径（探测失败即不注册工具），不做容器 / microVM / 内核驱动级后端（虚拟化层 L3+ 留作后续轴）。
- P15 只做观测：不新建编排器、不做画布交互、不落正文、不参与任何控制判定（观测面一旦参与控制即成为第二套状态机），**但也不阻止将来的编排器**——观测 sink 恒为只读输入（无返回值 / 无回执 / 无背压），编排器无论置于前端还是后端，都须另设控制端口、经既有命令入口发起 turn（I55 §11）；节点集合随内核结构走静态模板，不引入运行时节点注册表；不把节点状态写入消息行或模型上下文；不把节点模板下发到 wire；事件必须自带路由键（`conversationId` / `agentId` / `parentTurnId`），补值可以、补键不行；V1 不落库。

| 关系 | 说明 |
|---|---|
| P1 → P2 | `validate_input` 路径检查必须复用 `os/path.rs`，否则两个入口的结论会分叉（I31 §5） |
| P2 → P4 | Pipeline 是 RoundExecutor 的唯一工具执行入口，未就绪前不得驱动工具 |
| P3 → P4 | TurnRunner 提供 turn 生命周期、状态机、预算与事件；P4 只替换迭代循环体 |
| P1 + P2 → P5 | 路径安全与控制层先于任何破坏性工具；`delete` 未过 P2 不得进入生产注册表；`shell` 暂不提供 |
| P4 + P5 → P6 | 前端五态与审批 UI 需要真实工具与事件；模式配置需要 RoundExecutor 的白名单解析 |
| P2 + P5 → P7 | MCP 工具复用同一 Policy/Gateway/Pipeline，安全标记按 External 推断 |
| P6 → P10 | AI 顾问复用 P6 已上线的审批卡片 / 审批信息装配；P10 默认关闭，不改变 P6 行为 |
| P5 + P6 → P12 | 工作区解析复用 P5 的路径语义（`os/path.rs`），chip 与切换入口挂在 P6 的共同视图；P12 不改工具面与审批链 |
| P5 → P13 | 信封 head 的 `subject` / `scale` 只能取各工具已有结果事实（`data`）与参数，工具侧不新增 wire 字段；`compact` 的锚点取 `edit` 结果时刻行区间，与 P5 的写入回执同源 |
| P13 → I46 节点布局 | I46 §1.2 的 P1 开工门（`compact`）已由本阶段关闭（2026-11-14）；I46 节点布局的 §6.2 / §6.3 形态不变，单行走正式分支取 `compact.head`，降级分支只作旧数据（`compact == null`）兜底 |
| P2 + P5 → P14 | `execute` 意图复用既有 Policy/Gateway 求值，沙箱只加边界、不参与审批决策；`ApprovalScope::Command` 是 P2 已实现但无生产者的分支，P14 是首个生产者 |
| P12 → P14 | 沙箱可写根 = `resolve_workspace` 的唯一解析结果；不解析工作区的入口（ToolBox）不提供 `shell` |
| P14 → P10-b | `RelaxOnly` 的前置是「存在确实会产生 `Ask` 的只读调用」；`shell` 的 `execute` 意图在 `dangerous_only` 档恒为 `Ask`，该前置由 P14 关闭（I38g §1.3） |
| P14 → P10-c | `Adjudicate` 的门禁 1（内核强制隔离）由 P14 关闭；仍需独立 ADR，**不因 P14 完成而自动开工**（I18 design §3.17.6） |
| P13 → P14 | `shell` 结果的 `compact` 信封只消费既有 `data`（退出码 / 输出规模），不新增 wire 字段 |
| P14 → P9 | P14 交付的 `kill_tree` 幂等、`persisted` 输出域与 `enforcement` 审批键是后台任务的可复用资产；但进程归属与终态语义必须由 P9 重新持有（改动面与前置见 [I52-Ext](./I52-ext-background-task.md)），P14 不预留 |
| P4 + P6 → P15 | 观测面复用 P4 的 `TurnObserver`（已有 4 个方法，只加 1 个）与 P6 的 `transport` / `turnOrchestrator` 分层；只加方法与事件，不改既有事件语义（I55 O8），因此不阻塞任何在途阶段 |
| P5 + P13 → P16 | 水平窗口复用 P5 的 `read` 分块扫描（`window.rs`）与 `append` 截断点；`[truncated: …]` 四态与 `data` 开放 JSON 口径取自 P13（不新增 wire 字段、无 DTO 变更、前端零改动） |
| P4 + P5 + P13 → P17（T1） | `request_tool` 的可用集取自 P4 已持有的 `TurnRunCtx.tools`（`Option<&[ToolSpec]>`），其生产者是 P5 的 `resolve_available_tools`；`super_read` / `super_edit` 复用 P2 的 Pipeline 与回执存储（`ReadReceiptStore`）、P13 的落盘与预览口径 |
| P16 → P17（T2–T6） | **只是格式对齐，不是实现依赖**（I58 §0.3「契约对齐、实现独立」）：P17 自写扫描 / 渲染 / 索引，只须保证行格式**字面量**（`{number:>6}` + `\t`、` [truncated]`）、footer 标记集、`data` 既有键名与 `ReadReceipt` 字段与 P16 一致，否则 `edit` 的前缀剥离与闸门会失效（I58 §12 R6）。P16 已完成，无排期阻塞 |

**可并行**：P1 完成后，P2 与 P3 可并行（一个改 `assistant-tools`，一个改 `assistant-agent`，无文件交叠）。P5 在 P2 完成后可与 P3/P4 并行。

---

## 4 文档地图

| 主题 | 文档 | 性质 |
|---|---|---|
| 阶段与门禁 | 本文 | 总纲 |
| 工具词汇层 / Tool trait / Registry / Tauri 桥接 | I18 spec（已归档） | 实施规格（Step 0-3、8） |
| 工具架构决策（分层、权限、双入口、安全原则） | I18 design（已归档） | ADR |
| 工具控制层 | I38a（已归档） | 实施规格 |
| Agent Loop / 累积器 / tools 注入 | I38b（已归档） | 实施规格 |
| Agent Loop 实施指南（未完成步骤细化） | I38b 指南（已归档） | 实施指南 |
| OS 工具逐个实现 | I38c（已归档） | 实施规格 |
| 前端五态 / 审批 UI / Agent 模式 | I38d（已归档） | 实施规格 |
| MCP 外部工具 | I38e（已归档） | 实施规格 |
| P7 任务级进度（C1–C6 勾选） | I38e tasklist（已归档） | 任务清单（唯一任务级进度） |
| P8.1 子代理（`spawn_agent` / 子作用域 / 取消树 / 结果回传；**可选，暂缓接入**） | [I38f](./I38f-agent-subagent-spec.md) | 可选实施规格 |
| P10 AI 审批顾问（`EscalateOnly` / `RelaxOnly` / 审计与度量） | [I38g](./I38g-agent-approval-advisor-spec.md) | 实施规格 |
| P12 工作区（会话绑定 / 目录簿 / 唯一解析点） | I38h（已归档） | 实施规格 |
| P14 阶段规格（沙箱执行层：范围 / 术语分层 / 分批 / 阶段验收归属） | [I52](./I52-shell-sandbox-spec.md) | 阶段规格 |
| P14 `shell` 工具规格（身份 / 输入 / 审批键 / 执行 / 输出与回读 / 失败矩阵 / 文案 / 验收 A6–A36） | [I56](../achievement/I56-shell-tool-spec.md) | 实施规格（工具侧唯一来源） |
| P14 任务级进度（C1–C8 勾选） | [I52 tasklist](./I52-shell-sandbox-tasklist.md) | 任务清单（唯一任务级进度） |
| P14 阶段扩展：后台任务改动面（E1–E8 / 接口约定 / 立项前置 G1–G6；**未立项**） | [I52-Ext](./I52-ext-background-task.md) | 阶段扩展登记 |
| P15 Turn 观测面（节点级事件 / 7 值节点状态闭集 / 三投影 / 回放形态） | I55（已归档） | 实施规格 |
| P15 任务级进度（V0–V3 勾选） | I55 tasklist（已归档） | 任务清单（唯一任务级进度） |
| P16 长行读取规格（水平窗口 / 字节口径扫描宽限 / 截断报告 / 回执与 `edit` 闸门 / 门禁 G1–G12） | I57（已归档） | 实施规格（`read` 长行与扫描收口唯一来源） |
| P16 任务级进度（T1–T4 勾选） | I57 tasklist（已归档） | 任务清单（唯一任务级进度） |
| P17 超级工具规格（`request_tool` / `super_read` / `super_edit` / 索引生命周期 / 失败矩阵 / 门禁 G1–G23） | I58（已归档） | 实施规格（三个新工具唯一来源；与 I57 逐节对齐） |
| P17 任务级进度（T1–T8 勾选） | I58 tasklist（已归档） | 任务清单（唯一任务级进度） |
| assistant 消息身份指示（头像 / 模式 / 模型、消息级 `mode_id`） | I43-assistant-identity-spec.md（已归档） | 实施规格 |
| 模型能力事实（三态 + 来源 + 主体 key；内容/列表/推理三处止损） | I45-model-capability-facts-spec.md（已归档） | 实施规格 |
| 模型能力事实任务级进度（C1–C3 勾选） | I45 tasklist（已归档） | 任务清单（唯一任务级进度） |
| Agent 节点布局（坐标 / 容器 / 分组 / 密度 / 挂载） | I46 design（已归档） · I46 spec（已归档） | 设计 ADR + 实施规格 |
| 工具结果紧凑信封（三段信封 / 省略三态 / 落盘生命周期 / `compact` wire 字段 / 渲染形态选路） | I46 紧凑消息格式设计（已归档） | 参考记录（被 I46 节点布局设计取代为权威源；信封、省略、落盘、预算与门禁 1–12 仍有效） |
| P13 任务级进度（C1–C10 勾选） | I46 compact tasklist（已归档） | 任务清单（唯一任务级进度） |
| I46 任务级进度（S1–S6 勾选） | I46 tasklist（已归档） | 任务清单（唯一任务级进度） |
| Orb 运行态与可见性（失焦门控 / 档位常驻 / 窗口属性 / 冷启动锚点） | I47-orb-runtime-state-spec.md（已归档） | 实施规格 |
| Orb 运行态任务级进度（C1–C4 勾选） | I47 tasklist（已归档） | 任务清单（唯一任务级进度） |
| P8.2+ 演进候选（Worker / Router / continuity / 多编码） | 本文 §2.1 | 候选清单 |
| 运行时内核（turn / 状态机 / 端口 / chat 迁移） | I36 spec（已归档） | 实施规格 |
| 运行时决策（D1-D18、参考项目提取） | I36 design（已归档） | ADR |
| 内核能力基线（能力矩阵 / 参考实现对照 / 扩展点 / 门禁） | [I36a](./I36a-agent-kernel-baseline.md) | 基线文档 |
| 内核一致性验证（确定性要求 / 验证矩阵 / 实检缺口） | [I36b](./I36b-agent-kernel-conformance-spec.md) | 验证规格 |
| 压缩与 chat 的共享边界（共享积木 / 保留独立编排） | I39（已归档） | 专项规格 |
| P3 任务级进度（C1–C11 勾选） | I36 tasklist（已归档） | 任务清单（唯一任务级进度） |
| P4 任务级进度（C1–C9 勾选） | I38b tasklist（已归档） | 任务清单（唯一任务级进度） |
| P6 任务级进度（C1–C9 勾选） | I38d tasklist（已归档） | 任务清单（唯一任务级进度） |
| P12 任务级进度（C1–C7 勾选） | I38h tasklist（已归档） | 任务清单（唯一任务级进度） |
| 前四阶段代码规模与职责分布 | 代码清单（已归档） | 现状快照（不写进度） |
| 代码与文档差异、缺口与处置建议 | 代码缺口审计（已归档） | 审计快照（不写进度） |
| toolcall 从 chat 剥离 | I30（已归档） | 阶段一 A 段 |
| 阶段一工具框架 | I31（已归档） | 阶段一 |
| ToolBox 桥接 | I18b（已归档） | 专项规格 |
| Shell 洋葱模型 | [OSCALLDESIGN](../designed/OSCALLDESIGN.md) | 专项设计 |

本表只回答「谁管什么」，**不写进度**（阶段级进度见 §2，任务级勾选见各 tasklist）；各文档内部 Step / T / C 编号的勾选由该文档自身维护，属规格内部条目，不是阶段状态。

标注「（已归档）」的文档已移入存档目录并冻结，只作历史参照：不再修改、不再作为权威源、不再提供链接路径。现行权威源仅指同表未标注的行。

**单一权威源规则**：

- 工具类型、Trait、Registry、桥接 → I18 spec；控制层 → I38a；循环 → I38b；工具实现 → I38c；前端 → I38d；MCP → I38e；运行时 → I36 spec；内核一致性判定（DT/V/H）→ I36b；模型能力事实（三态 / 来源 / 主体 key / 未知策略）→ I45。其中 I18 spec、I38a、I38b、I38c、I38d 与 I36 spec 均已归档：P0–P6 关闭后，工具框架、控制层、循环、工具实现、前端与运行时的实际行为以代码为准，归档文本只作基线对照。模型能力演进的现状来源是 `MODEL-CAPABILITY-EVOLUTION-ANALYSIS.md`（已归档，只读分析，不作规格依据）。
- 同一契约不得在两份 spec 中各写一份。引用时写「见 X 文档 §Y」，不复制类型定义。
- I18 spec 已收敛为框架层规格（Step 0-3、8），归档时不再承载 Step 4-7、9-10。

---

## 5 冲突决议（2026-11-04）

历史文档之间存在以下冲突，本文给出唯一结论；各文档已按此修正。

| # | 冲突 | 结论 |
|---|---|---|
| R1 | RoundExecutor 落点：I18 spec 写 `assistant-agent/src/round_executor.rs`，I36 写 `agent/` | 统一落在 `assistant-agent/src/agent/`：`round_executor.rs`（循环）、`tool_call.rs`（累积器）、`mod.rs`（trait 与导出）。`assistant-agent/src/kernel/` 不得引用工具类型 |
| R2 | RoundExecutor 职责：I36 design 称其为迭代循环，I36 spec 的主流程把循环写在 TurnRunner | **P3 过渡形态**：TurnRunner 内部跑单迭代循环，`round_executor = None`。**P4 起**：迭代循环（模型调用 → 工具执行 → 回填 → 下一迭代）整体迁入 RoundExecutor；TurnRunner 只保留 turn 生命周期、状态机、预算、阶梯、落库、事件与收尾。P4 的入口固定为 `RoundExecutor::run_iterations` |
| R3 | `os_tool_exec` 示例仍写 `tool.execute()` + `map_err` | 以代码为准：`exec::invoke(tool, input, context, DEFAULT_TOOL_TIMEOUT)`，`Err` 通道已在 `invoke` 内归一化 |
| R4 | 工具阶段门禁：I18 design 的 Phase 3 与 I31 的「暂缓」表述不一致 | 以本文 P5 为准：P1 + P2 完成后才可新增破坏性工具；`delete` 必须与 P2 的 Policy/Gateway 同期可用；`shell` 暂不提供（I38c §13，已归档） |
| R5 | 前端字段命名：I18 spec Step 10 的 `AgentMode` 同时出现 snake_case 与 camelCase | wire / 前端一律 camelCase（`maxToolTier`、`approvalLevel`、`enabledTools`、`disabledTools`）；Rust 存储 struct 与 DB 列保持 snake_case。转换在 DTO 层完成 |
| R6 | 工具实现状态散落在 I18 spec Step 2 | 工具清单、Schema 与实施状态统一迁移到 I38c；I18 spec 不再维护逐个工具状态 |
| R7 | 终止式压缩与续跑式压缩的归属 | 压缩生命周期（事件、hook、trigger）归 I36 spec Step 6；「终止式 → 续跑式」的切换点**不归 P4**，归 P4 之后的续跑式压缩单独批次（I38b D13），不另写第三份规格。P4 只定 `CompactTrigger::Auto` 契约、不接线阶梯 ② |
| R8 | `ToolResult` 双通道（`result_for_assistant` / `error_kind`）归属 | 类型扩展归 P2（I38a §3）；P0 的 `ToolResult { ok, data, error }` 保持不变，P2 以新增可选字段落地，存量工具零改动 |
| R9 | `ToolAudience` 默认值：早期为迁移便利用 `BOTH` | **2026-11-04 收紧为 `NONE`（fail-closed）**：未显式声明受众的工具对两个入口都不可见。唯一存量工具 `fs_link` 已显式声明 `TOOLBOX_ONLY`，行为不变；新工具必须在 `audience()` 中显式声明 |
| R10 | 子代理 / 多 agent 是否属于当前阶段 | **P0–P7 不实现**。P4 只预留 `agent_id` / `parent_turn_id` / `delegation_depth` 三个可选字段（I36 D16-D18），实现归 P8.1；不得在 RoundExecutor 内写 `if is_subagent` |
| R11 | Super-agent 的 Router / Worker 与现有单会话 turn | **不作为当前架构目标**。P0–P7 是 Worker 层基础；P8.2/P8.3 才补子会话与 Router。跨重启的 persistence / Watchdog 归 P9，单独 ADR（I36 D13b），不得直接叠加后台进程 |
| R12 | 压缩与 chat 的共享边界 | **不属 R7 的三份边界**：生命周期语义仍归 I36 spec（已归档）Step 6，续跑式切换点归 P4 之后的续跑式压缩批次。共享到什么程度归 I39（已归档）。结论：**压缩不收编为 `TurnSpec` 第三种实例**，保留 `execute_compaction` 独立编排；只共享 `TurnTasks` 互斥源、`stream::consume`、`TurnEmitter`、usage 构造、`project_chat_messages` 投影、`emit_stream_finished` 刷新点与 `chat-delta.error` 失败通道 |
| R13 | chat / agent 能否在同一会话内切换（I38d 早期规格要求「会话内锁定」） | **2026-11-07 改为同会话可切换**：链路由每次发送时的 `modeId → kind` 决定，默认 chat；turn 进行中禁止切换（前端置灰 + 后端 `conversation_busy`）。前端形态为**共同视图 + 双布局**（`ConversationView` + `ChatLayout` / `AgentLayout`），复用 chat 共享组件（I38d §4.8 / §4.10，已归档） |

---

## 6 阶段门禁

每阶段只有在**退出条件**全部满足后才可标记完成；细节验收在该阶段 spec 的「验收标准」章节。

| 阶段 | 退出条件（摘要） |
|---|---|
| P1 | `os/path.rs` 有单测；`fs_link` 无私有路径实现；`permission_intents` 仅用 `read/edit/delete/execute`；`cargo test -p assistant-tools` 全绿 |
| P2 | Policy 单测覆盖 `most_restrictive` / last-match-wins / Ceiling 拒绝 Allow；Gateway 区分 Allow/Ask/Deny；Pipeline 顺序固定为 `find → validate → gateway → execute → persist`；无 tauri 依赖 |
| P3 | `cargo tree -p assistant-agent` 无 tauri；chat 对比测试（事件序列 / seq / 落库 / 错误文案）逐条一致；关闭窗口后 turn 可跑完 |
| P4 | 多迭代闭环可跑通；`max_iterations` / 收尾调用 / 空响应重试 / 工具结果回填均有测试；`ChatOptions.tools` 在三个 adapter 生效；三维预算（迭代 / 工具调用 / 墙钟）各自可触发收尾；阶梯 ③④⑤ 按序生效且不超预算；审批通道缺失时判 `Ask` 的工具为 `Deny` 并写审计 |
| P5 | 每个工具独立 spec 验收；破坏性工具只在 P2 就绪后注册；ToolBox 与 Agent 两入口共享 `execute()` 内安全守卫；`resolve_available_tools` deny-by-default + 白名单/黑名单有单测；P4 的 `RegistryToolCatalog` / `ToolCatalog` trait 已删除且全仓无残留引用；**P5-R2 修改组与 P5-R3 危险组的「启用」推迟至 P6**（P5 关闭时按 A7 只接只读组），见 I38d tasklist C9.5a / C9.5b（已归档）；2026-11-09 两组均已启用（`delete` 已接入生产注册） |
| P6 | 五态渲染与审批闭环可用；`ConversationToolConfig` 前端内存态初始化与传递；跨窗口 phase 不漂移（**推迟验证**，2026-11-09，见 I38d tasklist C2.4 / C9.6b（已归档））；链路按 `mode.kind` 每次发送分流、**会话内可切换**（turn 进行中禁止）；共同视图 + 双布局，默认 chat |
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
9. **wire 类型不在文档里重复声明**：DTO 的唯一权威源是 `src-tauri/src/protocol/dto/*.rs`（Rust 侧 derive `TS`），前端类型由 `npm run contract:gen` 生成、`npm run contract:check` 校验。因此：
   - 已生成的类型，完整声明**最多出现一次**，且只允许出现在其**设计来源 spec**（如 `AgentToolCallDto` 在 I38b §6.1.1）作为溯源；其余文档（含后续阶段 spec）只写「字段名 + 约束」表或增量描述；
   - 文档与生成结果不一致时**以代码为准**并修正文档，文档不得成为第二个契约源；
   - 尚未实现的 DTO 允许在唯一声明处写完整定义（通常是对应阶段 spec）；
   - 内部类型（非 wire，如 `assistant-tools` 的 `ApprovalContext`）不在生成范围内，可在其所属 spec 写完整定义，DTO 侧只写映射关系。
   新增 DTO 必须同时登记到 `src-tauri/tests/protocol_bindings.rs` 的 `declarations` 数组，漏登记该测试直接失败。
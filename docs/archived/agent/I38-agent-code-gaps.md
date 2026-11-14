# I38 — Agent 代码现状与缺口审计

| 项 | 值 |
|---|---|
| 状态 | 现行快照（只记录代码与文档的差异，不改变任何阶段状态） |
| 日期 | 2026-11-09 |
| 关联 | [I38 阶段总纲](../../store/I38-agent-roadmap.md)、[I36 tasklist](./I36-agent-runtime-tasklist.md)、[I38b tasklist](./I38b-agent-loop-tasklist.md)、[I38c tasklist](./I38c-os-tools-tasklist.md)、[I38d tasklist](./I38d-agent-frontend-modes-tasklist.md) |
| 影响范围 | 文档；不含代码契约变更 |

> 本文只回答一件事：**按现行文档核对代码后，哪些地方「说的」和「做的」不一致**。
> 阶段级状态仍以 [I38 §2](../../store/I38-agent-roadmap.md) 为唯一台账，任务级勾选仍以各 tasklist 为唯一来源；本文不重复记录进度，只登记缺口与处置建议。
> 缺口分三类：**代码缺口**（实现行为与规格语义不符）、**验证缺口**（无自动化或实机证据）、**记账缺口**（清单勾选与代码现状脱节）。

---

## 1 结论摘要

| 类别 | 数量 | 结论 |
|---|---|---|
| 代码缺口 | 2 | 1 项语义缺口（审批级别对只读工具不可达）、1 项审计缺口（拒绝发生在 validate 阶段，审计点只在网关内）；原「ToolBox 无工作区根」（G3）与「`delete` 未注册」（G4）均已于 2026-11-09 消解 |
| 验证缺口 | 1 | 仅 P6 C2.4 / C9.6b 多窗口 phase 一致为**推迟验证项**；P3 的 C5.29 / C5.30 已于 2026-11-09 改口径关闭（V2）；P4 C7 接缝已由一条冒烟用例覆盖（V1，整批仍不计完成） |
| 记账缺口 | 0 | A1（C5.34）与 A2（前端改动登记）均已于 2026-11-09 收口 |
| 已消解 | 4 | ①工具基础设施装配入口已从 `setup.rs` 提取为独立模块 `tool_setup.rs`，相关文档已同步（见 §5）；②I36 清单的 C0 段、C2.17 / C2.18、C5.34 已回填对齐（见 §5）；③G4 危险组 `delete` 已于 2026-11-09 接入生产注册（见 §2 G4）；④G3 解析基准 / 边界分离已于 2026-11-09 落地（见 §2 G3） |

阶段完成度不受上述缺口影响的部分：P0–P5 的已勾任务均有对应实现，未发现「勾了但没写」的任务；P6 代码与单测全部落地，2026-11-09 已关闭（唯一遗留为推迟验证项 C2.4 / C9.6b）；P7 确认未开工。

---

## 2 代码缺口

### G1 `all_os_tools` 审批级别对只读工具不可达

**现状**：`policy.rs` 的 `policy_from_agent_mode` 在 `ApprovalLevel::AllOsTools` 下追加 `("read", "*", Ask)` 规则；但 `read` / `ls` / `glob` / `grep` 四个工具**没有覆写** `Tool::permission_intents`（默认返回空集），而 `gateway.rs` 在 `intents.is_empty()` 时直接返回 `Allowed`，**不进入 `Policy::evaluate`**。

**证据**：
- `src-tauri/crates/assistant-tools/src/policy.rs`：`policy_from_agent_mode` 的 `AllOsTools` 分支
- `src-tauri/crates/assistant-tools/src/gateway.rs`：`intents.is_empty()` → `GatewayDecision::Allowed`
- `src-tauri/crates/assistant-tools/src/os/agent_read.rs`、`agent_list.rs`、`agent_glob.rs`、`agent_grep.rs`：均无 `permission_intents` 实现
- 覆写点仅存在于 `os/agent_write.rs`、`agent_move.rs`、`agent_mkdir.rs`、`agent_edit/mod.rs`、`agent_delete.rs`、`os/fs_link/mod.rs`

**影响**：`I38d §4.2` 写「`all_os_tools` = 所有非 Sandbox 工具需审批」，实际对只读组不成立。`policy.rs` 的单测 `all_os_tools_asks_even_for_reads` 只验证策略层结论，掩盖了网关层短路。用户选择「全部工具需审批」后，读操作仍会静默放行。

**关键事实（决定选型）**：网关短路本身**不是**缺陷。`intents.is_empty()` 的正确语义是「该工具没有权限面」，因此直接 `Allowed` 是对的（Sandbox 类纯计算工具就靠它免于无意义的策略求值）。真正的缺陷是**只读组有 OS 资源面却声明了空 intent**，属于工具侧漏声明，不是网关侧逻辑错。

**处置建议**（须先定稿再改代码）：
1. **方案 A（推荐，产品要「读也审批」时）**：给四个只读工具补 `permission_intents`，但资源用**类级 `"*"`**而不是解析后的绝对路径。理由：路径级资源会让每个文件弹一张审批卡（20 个文件 20 张卡），而类级资源只需一次「会话内允许读」即可覆盖全部读操作；且路径级 Deny 规则对读本来就不起作用（见 G2 的关键事实），所以降为类级不损失任何安全性。同步改 `I38d §4.2` 的 A10 口径为「读操作每次会话首读触发一次审批」。
2. **方案 B（推荐，产品不要读审批时）**：保持只读组自动执行，把 `I38d §4.2` 改为「`all_os_tools` 作用于**声明了 `permission_intents`** 的工具」，并**删除** `policy.rs` 中不可达的 `("read","*",Ask)` 规则与 `hard_deny_rules()` 的两条 `read` 规则——留着会持续误导后来者以为「读已纳入策略」。

> 注：原文档写的「给只读工具补 intent」若按写类工具的写法（资源 = `path::resolve` 结果）落地，会引入两个新问题：①敏感路径在 `permission_intents` 内就被 `reject_system_dirs` 拒绝，直接 `Err`，根本到不了策略层；②每个不同路径一张审批卡。故方案 A 必须用类级资源。

---

### G2 只读工具的硬保护 `Deny` 不产生审计

**现状**：`policy.rs` 的 `hard_deny_rules()` 含针对敏感路径（如系统目录）的 `read` 拒绝规则，但同样因只读工具 `permission_intents` 为空、网关短路放行，这些规则在网关层**不会被求值**，因此也不会写 `permission_audit`。

**证据**：同 G1 的调用链；`src-tauri/crates/assistant-tools/src/policy.rs` 的 `hard_deny_rules()`。

**影响**：行为层面仍被 `os/path.rs` 的 `reject_system_dirs` 兜住（不会真的读到敏感文件），但**审计缺失**：这类「模型尝试读取系统目录」的信号不会出现在审批审计里，P6 的审计查询看不到。

**关键事实（原处置建议的前提是错的）**：`hard_deny_rules()` 的两条 `read` 规则（`/etc/shadow`、`/etc/ssh/**`）**在当前链路上永远不可达**——`os/path.rs` 的 `protected_dirs()` 在 unix 下把 `/etc` 列入受保护目录，Windows 下把 `SystemRoot` / `ProgramFiles` / `ProgramData` 列入；`reject_system_dirs` 对读、写一视同仁，且发生在 `validate_input` → `PipelineStage::Validate`，**早于网关**。因此：
- 即使按 G1 方案 A 给只读工具补上 intent，这些 `read` 规则也不会命中（路径在 intent 计算里就被 `Err` 掉了）；
- 原文档「选方案 1 此缺口自动消解」**不成立**。

**处置建议（与 G1 解耦）**：审计缺失的根因是「拒绝发生在 validate 阶段，而审计点只在网关循环内」，不是「intent 为空」。正确落点是**在 pipeline 终态统一审计**：`pipeline.rs` 的 `run_call` 在 `PipelineStage::Validate` 失败与 `GatewayDecision::Denied` 两处调用一个新增的 `ToolGateway::audit_denial(tool, action, resource, stage)`（只写审计、不参与决策）。这样「模型尝试读系统目录」的信号可以进 `permission_audit`，且不需要给只读工具加 intent、不产生审批噪声。若只做 G1 方案 B（删除死规则），此审计点仍需补，因为被 `reject_system_dirs` 拦下的写操作同样没有审计。

---

### G3 ToolBox 执行路径没有工作区根（**已于 2026-11-09 消解**）

**原状**：`src-tauri/src/cmd/os_tool_exec.rs` 构造 `ToolUseContext::for_toolbox(None)`；工作区根只有 Agent 链路经 `tool_setup.rs` 的 `PipelineToolExecution::with_workspace_source` 注入。

**影响**：ToolBox 调用 `read` / `ls` / `glob` / `grep` 时相对路径与缺省 `path` 不可用，报「未配置工作区」。Agent 链路不受影响。

**语义定论**：[I18b §2.2](./I18b-tool-unified-bridge-spec.md#L83-L96) 明确「ToolBox 没有越界一说」，`for_toolbox(None)` 是**语义正确的选择**；[I31 §280](./I31-phase1-tool-framework-spec.md#L280) 把它定性为「可用性缺口而非安全缺口」。因此不存在「ToolBox 是否受工作区边界约束」的待定问题——答案是**不受**。

**处置结果**：按「解析基准 / 权限边界分离」实现，未给 ToolBox 填 `workspace_root`：

- `crates/assistant-tools/src/context.rs`：`ToolUseContext` 新增 `resolve_base: Option<String>`；`for_toolbox()` 只填 `resolve_base`，`workspace_root` 恒为 `None`
- `crates/assistant-tools/src/os/path.rs`：`join_input` 改用 `resolution_base()`（优先 `resolve_base`，**回退** `workspace_root`）；`ensure_within_root` 仍只认 `workspace_root`，一行未动
- `src-tauri/src/adapters/tool_exec.rs`：Agent 链路**只填** `workspace_root`（兼作边界，同时经回退作为基准），不额外填 `resolve_base`，避免出现「有基准无边界」的上下文
- `src-tauri/src/cmd/os_tool_exec.rs`：`for_toolbox(workspace_root_from_conf())`，配置的工作区根只作基准
- `os/agent_glob.rs` / `os/agent_grep.rs`：缺省 `path` 的默认目录同样走 `resolution_base()`

**边界未被削弱**：Agent 链路的绝对路径仍要过 `ensure_within_root`，`path_outside_workspace_root_is_rejected` 断言不变；新增 `differing_base_and_boundary_fails_closed` 锁住「基准在边界外必须拒绝」，`toolbox_base_resolves_relative_paths_without_creating_a_boundary` 锁住「ToolBox 只给基准、不产生越界结论」。

---

### G4 危险组 `delete` 未进入生产注册（**已于 2026-11-09 消解**）

**原状**：`tool_setup.rs` 已调用 `os::register_os_tools` + `register_readonly_tools` + `register_write_tools` + `register_interactive_tools`；`register_dangerous_tools` 有实现但生产未调用。

**处置结果**：`I38c §7.1` 三项门禁（P2 控制层可用、P6 前端审批卡片上线、A2 审批三态验收）已全部满足，2026-11-09 完成接线：

- `src-tauri/src/tool_setup.rs`：修改组之后追加 `os::register_dangerous_tools(&registry, receipts.clone())`
- `crates/assistant-tools/tests/registration.rs`：`production_registry()` 扩为「只读组 + 修改组 + 危险组」，新增 `write_registry()` 保留修改组批次断言；`the_production_registry_registers_every_group` 断言五组全在；`tool_definitions_for_os_tier_expose_every_os_facing_tool` 的 id 清单加入 `delete`
- 台账：[I38d C9.5b](./I38d-agent-frontend-modes-tasklist.md)（A28）、[I38c C6.3 / C6.6 后续注](./I38c-os-tools-tasklist.md)
- 实跑：`cargo test -p assistant-tools` 全绿（`--test registration` 7 项）、`cargo test --test tool_resolver` 10 项全通过

**注意**：`delete` 是 `ToolAudience::BOTH`，因此同时进入 LLM 工具清单与 `os_tool_exec` 透传入口；前端 `os.ts` 未新增 `delete` 命名空间（与已启用的 `write` / `edit` 一致，ToolBox 组件不在本期范围）。

---

## 3 验证缺口

### V1 P4 C7 端到端批次整体跳过 → **接缝已于 2026-11-09 补一条冒烟用例**

`I38b tasklist` C7.1–C7.17 与 C8.6 未编写，`src-tauri/crates/assistant-agent/tests/` 下无 `agent_loop_e2e.rs` / `agent_loop_dialect.rs`。已由用户决策跳过，组件级等价覆盖见该 tasklist C7 说明。整批仍不补写（不计入完成项）。

**已补的接缝**：真正没有覆盖的只有一条——「真实工具装配 → 真实 Pipeline → 真实 `os` 工具 → 真实文件系统」。2026-11-09 在 `crates/assistant-tools/tests/pipeline.rs` 补 `production_os_tools_round_trip_through_the_pipeline`：用 `os::register_os_tools` + `register_readonly_tools` + `register_write_tools` 装配真实注册表，经真实 `ToolGateway`（真实策略 / 真实审计）跑 `read`（预置文件）与 `write`（落 tempdir）各一次，断言 `PipelineStage::Execute`、真实文件内容与审计落点。

**顺带确认的两点**：
- `write` 成功后会为同一会话记录读回执，立刻重读会被去重成 `deduplicated`——这是控制层语义，故读的目标改用预置文件而非刚写出的文件
- `read` 当前不声明 `permission_intents`，网关走 `intents.is_empty()` 短路、不留审计（即 G1）；用例只断言「write 有审计、非 write 无审计」，不锁条目总数，避免 G1 定稿后误伤

dialect / 重试 / 收尾等分支仍由 `loop_executor` / `kernel_runner` 的替身测试覆盖，不重复投入。

### V2 P3 两项未勾 → **已于 2026-11-09 按改口径关闭**（C2.19 / C5.18 / C5.34 已于同日实跑并勾选，见 §5.3）

| 项 | 处置结果 |
|---|---|
| ~~C5.29 关闭 / 从未创建 orb 窗口时 turn 仍跑完并落库~~ | **已改口径并关闭**：`EventSink::emit(&self, event: TurnEvent)` 返回 `()`，sink 在类型层面无法让 turn 失败，「turn 仍跑完」是编译期保证；「落库」由 C2.19 的 `InMemoryStore` 覆盖。等价证据为 C5.28 `emit_orb_only`（窗口缺失只 `log::error!`）+ C2.19 端到端用例。写实机用例需给 `tauri` 加 `test` feature 用 `mock_app`，性价比不成立 |
| ~~C5.30 四组场景 `TurnEvent` 序列逐条一致~~ | **已改口径为 golden 不变量并落地**：对比基线随 C5.31 / C12 删除，改为断言「事件序列快照 + `Delta seq` 连续递增 + `Terminated.last_seq` 一致 + 单条末尾 `Terminated` + `Finalize` 唯一且最后 + 错误码文案」。落地于 `crates/assistant-agent/tests/kernel_runner.rs` 的 `golden_event_sequence_{completed,cancelled_mid_stream,provider_error,empty_response}`，实跑 17 项全通过 |

### V3 P6 手工回归：仅一项推迟验证

2026-11-09 实机回归后，`I38d tasklist` 仅 C2.4 / C9.6b 未勾，且已按用户决策登记为**推迟验证项**（不阻塞 P6 完成）。

| 项 | 状态 |
|---|---|
| C2.4 / C9.6b 多窗口同一会话 phase 一致 | **推迟验证**（2026-11-09 用户决策）：当前构建缺少可控的双窗口同会话场景（orb 面板的历史入口直接跳主窗口，面板自身不能独立选会话），无法构造可观测场景；代码侧后端 `session-state-changed` 已接线、前端订阅存在。恢复验证时补双窗口同会话手工回归即可 |
| ~~C1a.10 自定义模式双形态~~ | 2026-11-09 通过（A24 满足） |
| ~~C1d.10 验收 A11 / A21 / A22 / A23~~ | 2026-11-09 通过 |
| ~~C9.6 非审批项实机回归~~ | 2026-11-09 通过 |
| ~~C9.6a 审批三态与超时~~ | 2026-11-09 通过（A2 满足） |
| ~~C9.5a 修改组生产注册~~ | 2026-11-09 落地（A28 满足） |

---

## 4 记账缺口

### A1 `I36 tasklist` C5.34 已改名并实跑通过（2026-11-09 收口）

条目原写「`chat_streamCharacterization` / `chatOrchestrator` 两套测试保持全绿」，`chatOrchestrator` 已在 P6 C5a 流式分层重构中删除。2026-11-09 已把条目改为 `chatStreamCharacterization` + `turnOrchestrator` 并实跑 53 项全通过，条目已勾选。

### A2 工作区 12 个前端改动未登记

当前工作区有 12 个前端文件改动（`src/chat/core/toolCallView.ts`、`src/stores/agent/agentSegments.ts`、`src/services/stream/turnOrchestrator.ts`、`src/stores/chatConversationSlice.ts` 等，合计约 +384 行），内容是「补拉快照时把挂起的工具调用回挂到发起消息并改写为 `awaiting_user`」。这属于 P6 C8 与 A26 实机回归的加固；2026-11-09 实机回归通过，`I38d tasklist` C8 / C9.6 已勾选并登记该批改动。

---

## 5 本轮已消解

### 5.1 工具基础设施装配独立成模块

工具基础设施装配从 `setup.rs` 提取为独立模块 `src-tauri/src/tool_setup.rs`：

- 迁出内容：注册表与三组注册、`InMemoryReadReceiptStore`、`PendingQuestionRegistry` + `TauriQuestionPrompter`、授权 / 审计存储、`TimingApprovalPrompter` + `TauriApprovalPrompter`、`ToolGateway`、`FileToolResultStorage`、`ToolPipeline`、`PipelineToolExecution`，以及三个配置取值来源（审批超时、提问超时、工作区根）。
- 对外形态：`tool_setup::init(handle)`；`setup::init` 单行调用；`workspace_root_from_conf` 由 `cmd/agent/mod.rs` 直接引用，仍与执行期同源。
- 文档同步：`docs/ARCHITECTURE.md` 模块树、`I38c spec §1 / §3 / §7 / A7`、`I38c tasklist C6 / C7.5`、`I38b tasklist C4.18d`、`I38d spec §4.9`、`I38d tasklist C1c` 的实现位置已改指 `tool_setup.rs`。

---

### 5.2 I36 任务清单回填

2026-11-09 按代码现状回填 `docs/archived/agent/I36-agent-runtime-tasklist.md`：

- **C0.1–C0.9 全部勾选**，其中 C0.2 / C0.3 / C0.6 / C0.8 / C0.9 附复核结论；C0.3 注明端口已由四个扩为七个，C0.5 注明该形态仅在 P3 成立、P4 R2 起迁入 `agent/loop_executor/`。
- **C2.17 / C2.18 勾选**，并注明由 `assistant-agent/tests/kernel_runner.rs` 的 `RecordingSink::failing_after(1)` / `InMemoryStore::failing()` 覆盖。
- **C5.34 改名并实跑勾选**：`chatOrchestrator` → `turnOrchestrator`，注明等价套件位置；2026-11-09 实跑 2 文件 53 项全通过。

---

### 5.3 P3 三项按既有测试回填（2026-11-09）

`I36 tasklist` 的 C2.19 / C5.18 原判「未覆盖」，复核后确认既有测试已完整覆盖，只需回填证据：

- **C2.19**：`kernel_runner.rs` 的 `chat_turn_completes_end_to_end_with_doubles`，`harness()` 注入 `FixtureSource` + `RecordingSink` + `InMemoryStore` + `FixedClock`，全程无 Tauri。实跑 `cargo test -p assistant-agent --features test-doubles --test kernel_runner` 13 项全通过。
- **C5.18**：`loop_executor.rs` 的 `a2_wrap_up_call_drops_tools_and_marks_skipped`，`max_iterations = 2` 下 `iterations == 2` 且 `model_calls == 3`。实跑 `--test loop_executor` 34 项全通过。
- **C5.34**：前端两套测试 53 项全通过。

注：`assistant-agent` 的集成测试需显式 `--features test-doubles`，否则 `cargo test` 会报 0 passed（端口替身未编入）。

---

## 6 处置建议顺序（按投入产出比）

| 顺序 | 项 | 前置 | 成本 | 说明 |
|---|---|---|---|---|
| ~~1~~ | ~~**G4 `delete` 注册**~~ | 无 | 10 分钟 | **已完成 2026-11-09**：`tool_setup.rs` 接线 + `registration.rs` 断言扩为五组 + `delete` 进入 OS 工具清单；实跑全绿 |
| ~~2~~ | ~~**C5.29 改口径并关闭**~~ | 无 | 10 分钟 | **已完成 2026-11-09**：按类型级保证 + C2.19 覆盖改口径，见 V2 |
| ~~3~~ | ~~**C5.30 改口径 + golden 测试**~~ | 无 | 半天 | **已完成 2026-11-09**：4 条 golden 用例 + 共用不变量断言，`kernel_runner` 17 项全绿，见 V2 |
| ~~4~~ | ~~**V1 冒烟 e2e（一条）**~~ | 无 | 半天 | **已完成 2026-11-09**：`pipeline.rs` 新增 `production_os_tools_round_trip_through_the_pipeline`，真实注册表 + 真实网关 + 真实 tempdir 跑 `read` / `write`，`cargo test -p assistant-tools` 全绿 |
| ~~5~~ | ~~**G3 解析基准 / 边界分离**~~ | 无（I18b §2.2 已定稿） | 1 天 | **已完成 2026-11-09**：`ToolUseContext` 加 `resolve_base`；`join_input` 走 `resolution_base()`（回退 `workspace_root`）、`ensure_within_root` 仍只认 `workspace_root`；Agent 只填边界、ToolBox 只填基准；补「ToolBox 无边界」与「基准在边界外拒绝」两条单测，见 §2 G3 |
| 6 | **G2 pipeline 终态审计** | 无（与 G1 解耦） | 半天 | 新增 `ToolGateway::audit_denial`，在 Validate 失败与 Denied 两处调用 |
| 7 | **G1 审批级别语义定稿** | 需产品决策（A / B） | 取决于方案 | 方案 A 给只读工具补类级 `"*"` intent；方案 B 改文档口径 + 删死规则 |

原顺序中的「先定稿 G1」不再是阻塞项：G2 已确认与 G1 解耦，G3 已于 2026-11-09 落地，剩余只有 G2 与需产品决策的 G1。
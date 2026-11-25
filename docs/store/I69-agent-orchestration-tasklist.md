# I69 — 编排层任务清单

| 项 | 值 |
|---|---|
| 状态 | 任务清单（**唯一任务级进度**；进度只在本文件勾选，DU 结果与阶段结论不在此登记） |
| 日期 | 2026-11-25 |
| 设计 | [I69-agent-orchestration-roadmap.md](./I69-agent-orchestration-roadmap.md)（波次、前置、退出条件、回退点唯一来源；本文只列改动点与验收入口，不复述规则） |
| 判据 | 每波退出条件见 I69 §4；验收编号 A1–A24、A26、A28–A32 见 [I69a §13](./I69a-agent-orchestration-base-spec.md)，A25 / A27 见 [I69b §4](./I69b-agent-orchestration-contract-spec.md)；DU 切分与覆盖见 I69 §1.4.1 |
| 范围 | 后端：`src-tauri/crates/assistant-orchestrator/`（新增）、`src-tauri/crates/assistant-storage/src/orchestration.rs`、`src-tauri/src/cmd/agent/mod.rs`、`src-tauri/src/cmd/orchestration/`、`src-tauri/src/adapters/orchestration.rs`、`src-tauri/crates/assistant-protocol/src/orchestration.rs`；前端：`src/features/orchestration/`（接缝）、`src/types/protocol.contract.ts`（生成物） |
| 边界 | 不新增执行语义（执行节点只有 `turn`，`hitl` 只等待、`group` 只分组）；不建第二个 turn 发射点；R0–R3 零契约 diff、R4 一次出齐；界面段 W3 / W4（DU7）与卡片形态不属本文，任务级进度见 [I69d tasklist](./I69d-agent-orchestration-card-tasklist.md) |

批次：R0 骨架与归属键 → R1 模板 schema v1 与保存期校验 → R2 持久化与崩溃收口 → R3 并行、取消树与条件路由 → R4 HITL、预算归因与对外契约 → R5 前端接缝与断线补齐。逐波严格串行（I69 §5），每波对应 DU1–DU6（I69 §1.4.1）。

> 勾选表示**代码已改且对应验收已跑**。每波交付后按 I69 §1.4.5 写功能实现总结（本文该波末尾的总结项），并在 I69 §1.4.6 台账登记「已交付 / 覆盖 DoD / 未覆盖 DoD / 总结位置」；本文不写 DU 结论与 DoD 覆盖判定，也不写阶段结论。

---

## 开工检查（I69 §4.1）

- [x] 建立 P18 tasklist 并登记 R0–R5（2026-11-25）
- [ ] 记录契约基线：`npm run contract:check` 幂等，作为 R0–R3 零 diff 起点
- [x] 改动面交叠确认（2026-11-25）：`src-tauri/` 代码侧无未提交改动，`src-tauri/src/cmd/agent/mod.rs` 最近改动为 2026-11-20（I61 存储管理器），无在途阶段占用；工作树内未提交改动均为 `docs/store/` 文档（I75 启动链路分层规格、doc-naming-spec 修订、I38 / I69 / 本文），与 R0 改动面不交叠

---

## R0 — 骨架与归属键（I69 §4.1 / DU1）

crate 与依赖：

- [ ] `src-tauri/crates/assistant-orchestrator/`（新增 crate）+ `src-tauri/Cargo.toml` 的 `[workspace].members` 注册；依赖只允许通用库（serde / serde_json / thiserror / tracing / async-trait / tokio），不依赖 `assistant-agent` / `assistant-tools` / `tauri`（I69a §5.2）
- [ ] `lib.rs`：对外 API `Orchestrator` / `RunHost` / `RunLedger` / `OrchestratorError`（I69a §5.1）
- [ ] `model.rs`：最小模板结构（节点 / 边；完整 schema 与校验留 R1）；类型名不含 turn 模板相关词，不引用 `src/stores/agent/observation/template.ts` 与内核侧 turn 模板常量（I69a §5.1 命名纪律）
- [ ] `port.rs`：`RunHost` / `TurnTap` / `LaunchTurnSpec` / `StartedTurn` / `LaunchError`；`LaunchTurnSpec` 只带纯数据（字符串与 tap），不带任何 `assistant-protocol` DTO（I69a §7.1）
- [ ] `port.rs`：`TurnStatusTap` 四值在编排 crate 内独立定义，不依赖内核 `TurnStatus`（I69a §7.2）
- [ ] `schedule.rs`：就绪集合最小实现，只推进单节点串行（并行、条件边、冲突域、并行上限留 R3）
- [ ] `run.rs`：run / node_run 状态机骨架，只含串行所需迁移；非法迁移在测试中即 panic
- [ ] `ledger.rs`：`RunLedger` trait + `MemoryLedger`（内存台账）
- [ ] `event.rs`：`RunEvent` 与广播端口骨架，与宿主接线分离；R0 不接 wire、不接宿主事件（I69a §5.1）

宿主侧（I69a §5.4，最小面）：

- [ ] `src-tauri/src/cmd/agent/mod.rs`：`start_turn` 改为回传 `StartedTurn`（`turn_id` / `assistant_message_id` / `turn_index`）；既有调用方忽略返回值，零行为变化
- [ ] 同上：`TauriEventSink` 外包 `TappedSink`（转发 + 通知 tap）；**不改** `EventSink` trait，tap 失败只记日志、不 panic 传播
- [ ] 同上：`cancel_conversation` 复用 `TurnTasks::cancel`，不新增取消路径
- [ ] 宿主 `RunHost` 实现：`launch_turn` 内部调 `start_turn`（禁止 `block_on`）/ `cancel_conversation` / `is_conversation_busy` / `workspace_root`
- [ ] 宿主 `now_ms`：以进程启动 `Instant` 为基准的单调毫秒；**禁止**复用 `src-tauri/src/adapters/system_time_source.rs` 的系统时间实现（I69a §7.1 时钟约束）

测试与门禁：

- [ ] 集成测试（假 `RunHost` + 真内存台账）：两个 `turn` 节点串行 run 跑完，节点与 run 状态正确，`assistant_message_id` 指向真实消息行（A1）
- [ ] 单测：宿主返回 `SessionBusy` 时节点 `failed(busy)`、run 不挂死（A4）
- [ ] 单测：tap 内 panic 时 turn 仍正常终态（A5）
- [ ] 门禁：`cargo tree -p assistant-orchestrator` 不含 `tauri` / `assistant-agent` / `assistant-tools`（A3）；全仓仍只有命令层一处 turn 发射点、编排层不构造 `TurnSpec`（A2）
- [ ] 门禁：契约文件零 diff（A6）＋ 共性门禁（编译 / 测试 / clippy / fmt 全绿、无新增警告、验证资产随交付物同波完成，C7）
- [ ] 功能实现总结（I69 §1.4.5 五条）已写入本节，并在 I69 §1.4.6 台账登记 DU1

> 验收：`cargo test -p assistant-orchestrator`；`cargo test -p assistant-app`；`cargo clippy --workspace --all-targets -- -D warnings`；`cargo fmt --check`；`npm run contract:check`。
> 回退点：整个 crate 删除即可回退；无 schema、无 wire、无迁移。

---

## R1 — 模板 schema v1 与保存期校验（I69 §4.2 / DU2）

- [ ] `model.rs` 冻结模板 v1：`id` / `name` / `version` / 语义字段（节点、边、分组）/ 呈现字段（布局）；语义与坐标分列（I69a §6.1）
- [ ] 节点类别闭集恰 `turn` / `hitl`；`TurnNodeParams` 为常量参数，不接受引用
- [ ] 边条件闭集恰 `always` / `onSuccess` / `onFailure`；分组结构（`id` / `title` / 成员）
- [ ] `validate.rs`：纯函数 `Template -> Result<(), Vec<ValidateError>>`；`ValidateErrorKind` 为 10 值闭集；错误可累积、不首错即停；每项至少带 `node_id` / `edge_id` / `group_id` 之一
- [ ] 十类非法各被拒并定位（重复 id / 悬空边 / 环 / 缺必填参数 / 未知类别 / 条件与类别不匹配 / 分组引用不存在节点 / 空分组 / 分组 id 或 title 重复 / 节点属多个分组）（A7）
- [ ] 检测顺序：`1 → 2 → 3` 先于其余项；环检测跳过未知端点；自环按该边单独报、长环逐回边各报一条且带 `edge_id`；同一缺陷不被报成两类（A30 / A9）
- [ ] 条件与类别匹配：`hitl` 源的 `always` 被拒、`onSuccess` / `onFailure` 通过；`turn` 源三种条件全通过（A31）
- [ ] 未知节点类别被拒、不降级为默认态（A11）；只改 `layout` 不改变任何校验结果与 `spec_json`（A10）
- [ ] `version` 与当前 schema 版本不等即拒绝加载，不降级、不迁移（A32 加载侧）
- [ ] 编译层：合法模板编译出正确的 `LaunchTurnSpec` 序列（A8 / G6）
- [ ] 功能实现总结（I69 §1.4.5 五条）已写入本节，并在 I69 §1.4.6 台账登记 DU2

> 验收：`cargo test -p assistant-orchestrator`；`cargo clippy --workspace --all-targets -- -D warnings`；`cargo fmt --check`；`npm run contract:check`（零 diff）。
> 回退点：schema 只存在于 crate 内部与内存、未落库 → 改结构零迁移成本。

---

## R2 — 持久化与崩溃收口（I69 §4.3 / DU3）

- [ ] `src-tauri/crates/assistant-storage/src/orchestration.rs`：三表 `workflow_templates` / `workflow_runs` / `workflow_node_runs` + 索引 + 迁移（I69a §8.1）
- [ ] 三表时间列一律由宿主 `now_ms()` 写入（单调毫秒，不是 Unix 时间戳）；读取侧不把它当墙钟时间展示（I69a §8.1）
- [ ] `ledger.rs` 增 `StorageLedger`（落库版）；`RunLedger` trait 不因落库而改签名
- [ ] 发起幂等：同一 `(template_id, origin_conversation_id, client_run_key)` 重复发起返回既有 run，不双执行（A13 / I69a §8.3）
- [ ] 启动扫描收口孤儿 run：在跑 run 置 `orphaned`、节点置 `failed(orphaned)`；活跃判定只来自 `TurnTasks`，禁止按时间戳判定（A14 / I69a §8.4）
- [ ] 删除会话：台账行保留、引用按「结果已删除」呈现、无外键错误（引用列不加外键，A15 / I69a §8.5）
- [ ] 台账写失败：run 置 `failed(ledger)`，turn 终态不变、不回滚 turn（A16）
- [ ] 不落原始事件（I69a §8.6）
- [ ] 集成测试（真 DB）：重启后 run / node_run 可读且状态与终止前一致（A12）；模板行可读，`spec` 与 `layout` 仍分列、`version` 不变、内容与保存时一致（A29）
- [ ] 回退登记：删表按 I69 §2 C9 单列并登记为不可逆动作；已落库数据默认保留
- [ ] 功能实现总结（I69 §1.4.5 五条）已写入本节，并在 I69 §1.4.6 台账登记 DU3

> 验收：`cargo test -p assistant-storage`；`cargo test -p assistant-orchestrator`；`cargo clippy --workspace --all-targets -- -D warnings`；`cargo fmt --check`；`npm run contract:check`（零 diff）。
> 回退点：代码路径可整体下线（删模块 + 停写）；已落库数据默认保留。

---

## R3 — 并行、取消树与条件路由（I69 §4.4 / DU4）

- [ ] `schedule.rs`：并发键 `(run_id, node_id)` 与就绪集合推进（I69a §9.1）
- [ ] run 级取消树（run → 节点 → turn），取消幂等、二次取消无副作用（A19）
- [ ] 条件边求值只读 turn 终态（闭集 `always` / `onSuccess` / `onFailure`）；只激活一条下游，其余标跳过且语义为调度阻断（A20 / I69a §9.4）
- [ ] 冲突域判据：同 `workspace_root` 的两个 `turn` 节点不并行（A21 / I69a §9.3）
- [ ] 会话互斥守门：同一会话同一时刻最多一个运行中节点，`TurnTasks` 不出现 `SessionBusy`（A18）
- [ ] 并行上限闸门：同时 `running` 节点数不超过 `maxParallel`（缺省 4），超出的就绪节点排队而非并发，且不丢节点、不标失败（A28 / I69a §9.2）
- [ ] 集成测试：两个不同会话的节点并行推进，墙钟区间明显重叠（A17）
- [ ] 功能实现总结（I69 §1.4.5 五条）已写入本节，并在 I69 §1.4.6 台账登记 DU4

> 验收：`cargo test -p assistant-orchestrator`；`cargo test -p assistant-app`；`cargo clippy --workspace --all-targets -- -D warnings`；`cargo fmt --check`；`npm run contract:check`（零 diff）。
> 回退点：回退到串行只改调度模块，台账与 schema 不动；已落库数据保留。

---

## R4 — HITL、预算归因与对外契约（I69 §4.5 / DU5）

能力侧（纯后端）：

- [ ] `hitl` 节点与 gate 状态机：run 停 `waiting`，proceed / reject 分别置 `finished` / `failed`，`note` 落台账、**不进**对话历史（A22 / I69a §9.5）
- [ ] gate 无默认超时：只有显式取消或解决能结束等待
- [ ] run 墙钟预算（`now_ms` 单调口径）耗尽 → `failed(timeout)` 且在跑节点被取消（A23 / I69a §9.6）
- [ ] 节点数上限 → 后续节点不发起，run `failed(node_limit)`（A24）
- [ ] token 归因只读、按节点分摊；审计与归因一律走 `assistant_message_id`（不依赖进程内 `turn_id`）

契约侧（一次性出齐，波内顺序固定：协议类型 → 命令 → 事件 → 生成物 → 绑定登记，I69 §2 C8）：

- [ ] `src-tauri/crates/assistant-protocol/src/orchestration.rs`：I69b §2.1 全部 DTO 一次出齐（含 `WorkflowGateInputDto`、`WorkflowValidationIssueDto` / `WorkflowValidationIssueKindDto`）
- [ ] `src-tauri/src/cmd/orchestration/`：10 个命令一次出齐（模板读 / 存、run 发起 / 取消 / 单查 / 列表、节点明细、gate 解决、校正）
- [ ] `src-tauri/src/adapters/orchestration.rs`：DTO ↔ 领域映射（含 `WorkflowRunStartInputDto` 的 `nodeConversations` / `clientRunKey` / `budgets`，I69b §2.2）；crate 不依赖 `assistant-protocol`
- [ ] `workflow-run-event` 广播（结构见 I69b §1.1；多 run 不串台，A25）
- [ ] `save_workflow_template` 的校验失败走 `issues` 返回、不走 `CommandError`（I69b §2.4）
- [ ] `reconcile_workflow_run`：丢事件后把台账拉回事实，不发起新 turn、不回退状态（A26 / I69a §8.7）
- [ ] `npm run contract:gen` 生成 `src/types/protocol.contract.ts`；契约生成与校验幂等、绑定清单登记齐全
- [ ] 前端未接入时 run 仍完整推进（无前端依赖）
- [ ] 功能实现总结（I69 §1.4.5 五条）已写入本节，并在 I69 §1.4.6 台账登记 DU5

> 验收：`cargo test -p assistant-orchestrator`；`cargo test -p assistant-app`；`npm run contract:gen` 后 `npm run contract:check` 幂等；`cargo clippy --workspace --all-targets -- -D warnings`；`cargo fmt --check`。
> 回退点：DTO 与命令可整体移除（不删台账、不删模板数据）；事件是广播，无订阅者即无影响。

---

## R5 — 前端接缝与断线补齐（I69 §4.6 / DU6）

- [ ] `src/features/orchestration/` 接缝：命令封装统一经 `tauriInvoke`，组件不直接调用底层 `invoke`
- [ ] 全仓唯一 run 事件订阅入口，组件不直接 `listen`
- [ ] 按 `(runId, nodeId)` 键控投影，不用单缓冲
- [ ] 重连流程：先全量读 → 事件增量 → 漏事件调 `reconcile_workflow_run`，重连后与台账一致
- [ ] 模块注册只留入口与空态；不改既有卡片投影（卡片形态归 I69d）
- [ ] 读取纪律与事件消费按 I69b §3.2 / §3.3；验收 A27（前端接缝）
- [ ] 功能实现总结（I69 §1.4.5 五条）已写入本节，并在 I69 §1.4.6 台账登记 DU6

> 验收：`npx vitest run src/features/orchestration/`；`npm run typecheck`；`npm run lint`。
> 回退点：删除 R5 新增的接缝模块（命令封装 / 事件订阅 / 键控投影 / 重连）即可回退；后端、契约与数据不动；I69d 的卡片投影与既有画布设施不属 R5 回退面。
> 不交付：画布节点渲染、模板编辑器、运行态叠加、任何新 DTO / 命令 / 事件。

---

## 阶段收口（不属任何一波）

- [ ] 回写 [I38 §2](./I38-agent-roadmap.md)：按 I69 §1.4.6 填写规则第 4 条——仅当该台账汇总为整体完成时，在 P18 行写 `✅ 已完成` + 日期，不改 I38 其它位置、不出现 DU 编号
- [ ] 整体完成后 I69 / I69a / I69b / I69c / I69d 移入 `docs/achievement/` 待归档，本清单归档
# I69b — 编排层对外契约与前端接缝（P18-R4 暴露面 / R5）

| 项 | 值 |
|---|---|
| 状态 | 定稿（可实施） |
| 日期 | 2026-11-23 |
| 阶段 | P18（编排轴 L2；见 [I38 §2](./I38-agent-roadmap.md) / §3） |
| 覆盖波次 | **P18-R4 的对外暴露面 + P18-R5 的前端接缝**（R0–R4 的语义与结构约束见 [I69a](./I69a-agent-orchestration-base-spec.md)） |
| 波次与交付顺序 | [I69](./I69-agent-orchestration-roadmap.md)（**唯一来源**：交付物、前置、退出条件、回退点；本文不排波次） |
| 上游 | [I69a](./I69a-agent-orchestration-base-spec.md)（**单向**：不变量 INV1–INV13、决议 D1–D22、模板 / 台账 / 调度语义；本文只暴露，不改其任何约束） |
| 上游承接清单 | 逐条 ID 见 [I69a §16.1](./I69a-agent-orchestration-base-spec.md)；本卷落点清单见 §6（**逐条，不用区间**），上游新增冻结项时同批次同步 |
| 事实来源 | `docs/analysis/AGENT-WORKFLOW-ORCHESTRATION-ANALYSIS.md`（2026-11-20 快照：N1–N18、G1–G11、D1–D10、O1–O16；时点记录，不随后续代码追改） |
| 术语唯一出处 | `docs/analysis/ORCHESTRATION-CONCEPT-SEMANTIC-TRANSFER.md` §2.5（用户可见词）· §2.6（开发原语）；本文只引用，不复述定义 |
| 影响范围 | 后端：`assistant-orchestrator/src/event.rs`；`assistant-protocol/src/orchestration.rs`；`src-tauri/src/cmd/orchestration/`；`src-tauri/src/adapters/orchestration.rs`。前端：`src/features/orchestration/`（只做接缝）；生成物 `src/types/protocol.contract.ts` |
| 契约影响 | **本卷即冻结点**：wire 在此一次性出齐（§2）；此前各波零契约 diff（INV12） |

> **本文管理**：事件与投影（§1）、wire 契约（§2）、前端接缝（§3）、验收 A25 / A27（§4）、本卷相关未决问题索引（§5）、上游承接清单（§6）。
> **本文不管理**：语义与结构（[I69a](./I69a-agent-orchestration-base-spec.md)）；波次、交付物、退出条件、回退点（[I69](./I69-agent-orchestration-roadmap.md)）；模板编辑器与运行态叠加的视觉形态（[I69c](./I69c-agent-orchestration-canvas-spec.md)）。
> **怎么读**：I69 定顺序 → I69a 定「做出来必须满足什么」→ 本文定「怎么暴露、前端怎么读」。本文是 I69a 的最后一层皮：**不新增语义，不重复约束**，遇到语义问题一律回 I69a。

> 判据编号 **A1–A32 跨两卷统一定义、不重编**：A1–A24、A26、A28–A32 在 [I69a §13](./I69a-agent-orchestration-base-spec.md)；A25 / A27 在本文 §4。本文中的「必须 / 禁止」为可验收约束。

---

## 1 事件与投影

### 1.1 `workflow-run-event`

```ts
// 事件名：workflow-run-event（Rust emit → 前端 listen）
{
  runId: string,
  nodeId: string,            // 与台账 (run_id, node_id) 同粒度
  kind: "turn" | "hitl",
  state: "pending" | "running" | "waiting" | "finished" | "failed" | "skipped" | "cancelled",
  runState: "pending" | "running" | "waiting" | "finished" | "failed" | "cancelled" | "orphaned",
  reason: string | null,
  turnId: string | null,
  assistantMessageId: string | null,
  note: string | null,
  at: number                 // 宿主 now_ms()：进程内单调毫秒，非 Unix 时间戳，不得用于显示墙钟时间
}
```

| 约束 | 说明 |
|---|---|
| 键控 | 事件按 `(runId, nodeId)` 键控，多 run 并行不串台（O12 / D6） |
| 无回执 | 事件是单向广播，前端不回 ack（INV3） |
| 可丢 | 事件丢失由 `reconcile` 收口（[I69a §8.7](./I69a-agent-orchestration-base-spec.md)）；**不得**据此判定 run 状态（INV8） |
| 事件名不复用 | 不得复用 `agent-node-event`（L1 观测面专用） |
| 排序 | 同一 `(runId, nodeId)` 的 `at` 单调不减；跨节点无序（并行本就无序） |
| 状态闭集 | `state` / `runState` 与 [I69a §8.2](./I69a-agent-orchestration-base-spec.md) 的 `NodeRunState` / `RunState` 逐值一致，不增不减 |

### 1.2 投影

| 投影对象 | 来源 | 刷新时机 |
|---|---|---|
| run 概览（状态 / 预算 / 节点计数） | 台账（`workflow_runs` + `workflow_node_runs` 聚合） | 事件到达 + 打开面板时全量 |
| 节点明细 | 台账单行 | 事件到达 |
| 结果跳转 | `assistant_message_id` → 会话消息行 | 点击时按需 |
| 预算剩余 | `now_ms()` 与台账 `started_at` | 面板打开时算一次，**不做秒级 tick** |

**事件不落库**（D5）。投影只读台账；前端缓存不得作为 run 状态的第二真相（INV8）。

---

## 2 wire 契约

### 2.1 DTO 与命令清单

**DTO（`assistant-protocol/src/orchestration.rs` → `npm run contract:gen` → `src/types/protocol.contract.ts`）**

| DTO | 用途 |
|---|---|
| `WorkflowTemplateDto` | 模板全量（含 `spec` 与 `layout` 分列） |
| `WorkflowNodeDto` / `TurnNodeParamsDto` | 节点与 turn 参数 |
| `WorkflowEdgeDto` / `WorkflowGroupDto` / `WorkflowLayoutDto` | 边 / 分组 / 坐标 |
| `WorkflowTemplateSaveInputDto` | 保存模板（整模板替换） |
| `WorkflowTemplateSaveResultDto` | 保存结果：`template` + `issues`；`issues` 为空即成功（§2.4） |
| `WorkflowValidationIssueDto` / `WorkflowValidationIssueKindDto` | 单条校验失败：闭集类别 + 定位字段（§2.4） |
| `WorkflowTemplateSummaryDto` | 列表项 |
| `WorkflowRunDto` | run 概览（`list_workflow_runs` 复用同一结构，不另立摘要 DTO） |
| `WorkflowNodeRunDto` | 节点明细 |
| `WorkflowRunStartInputDto` | 发起 run（见 §2.2） |
| `WorkflowGateInputDto` | gate 决策（`{ runId, nodeId, proceed, note? }`） |

**命令**

| 命令 | 入参 | 出参 |
|---|---|---|
| `list_workflow_templates` | — | `WorkflowTemplateSummaryDto[]` |
| `get_workflow_template` | `id` | `WorkflowTemplateDto` |
| `save_workflow_template` | `WorkflowTemplateSaveInputDto` | `WorkflowTemplateSaveResultDto`（校验失败在 `issues` 内返回，不走 `CommandError`；见 §2.4） |
| `start_workflow_run` | `WorkflowRunStartInputDto` | `WorkflowRunDto` |
| `cancel_workflow_run` | `runId` | `()` |
| `get_workflow_run` | `runId` | `WorkflowRunDto` |
| `list_workflow_runs` | `templateId?` / `originConversationId?` / `limit?`（缺省 20，上限 100） | `WorkflowRunDto[]`（按 `started_at` 倒序） |
| `list_workflow_node_runs` | `runId` | `WorkflowNodeRunDto[]` |
| `reconcile_workflow_run` | `runId` | `WorkflowRunDto` |
| `resolve_workflow_gate` | `WorkflowGateInputDto` | `WorkflowNodeRunDto` |

### 2.2 `WorkflowRunStartInputDto`

```ts
{
  templateId: string,
  originConversationId: string,          // UI 定位锚点，必填
  nodeConversations?: Record<string, string> | null,  // nodeId → conversationId；缺省 = origin
  clientRunKey?: string | null,          // 幂等键
  budgets?: {
    wallClockMs?: number | null,
    maxNodes?: number | null,
    maxParallel?: number | null
  } | null
}
```

| 字段 | 规则 |
|---|---|
| `originConversationId` | 必填；用于 UI 定位与缺省会话（D12） |
| `nodeConversations` | 只允许引用已存在会话；未知 `nodeId` 键 → 拒（`InvalidInput`）；解析结果写台账快照（[I69a §8.1](./I69a-agent-orchestration-base-spec.md)） |
| `clientRunKey` | 命中唯一约束 → 返回既有 run（[I69a §8.3](./I69a-agent-orchestration-base-spec.md)） |
| `budgets` | `null` 或字段为 `null` → 「未配置」（INV11）；`maxParallel` 缺省 4；四者口径独立（[I69a §9.6](./I69a-agent-orchestration-base-spec.md)） |

### 2.3 契约纪律

| # | 纪律 | 依据 |
|---|---|---|
| 1 | wire 在 R4 一次性冻结；此前各波零 diff | INV12 |
| 2 | 改 DTO 后必须 `npm run contract:gen` 并提交生成物，禁止手改 | 项目规则 |
| 3 | 事件与台账同粒度键控 | O12 |
| 4 | 命令层只做参数映射与错误转换，业务逻辑全在 crate | [I69a §5.4](./I69a-agent-orchestration-base-spec.md) |
| 5 | DTO 与 crate 内模型分离：crate 不依赖 `assistant-protocol`；映射在 `src-tauri/src/adapters/orchestration.rs` | [I69a §7.1](./I69a-agent-orchestration-base-spec.md) |
| 6 | 输出 DTO 的 `Option` 不加 `skip_serializing_if`（键恒在、值 `null`，前端声明 `T \| null`）；输入 DTO 的 `Option` 加 `#[ts(optional = nullable)]` 并加 `deny_unknown_fields` | 项目规则 |
| 7 | 禁止下划线开头的 wire 字段；封闭集合用 Rust `enum` + `serde(rename_all = "camelCase")`，前端不窄化 | 项目规则 |
| 8 | 错误码复用 `CommandErrorCodeDto`（`SessionBusy` / `Validation` / `StorageFailure` / `ConversationNotFound`），不新增码；模板校验失败走 §2.4，不占错误码 | — |
| 9 | 不新增「图级授权」类命令 | D7 |
| 10 | 前端读取纪律见 §3.2 | — |

### 2.4 模板校验失败的 wire 形态

```ts
// save_workflow_template 出参
{
  template: WorkflowTemplateDto | null,   // 通过时非 null
  issues: WorkflowValidationIssueDto[]    // 空数组 = 通过
}

// 单条
{
  kind: "duplicateId" | "danglingEdge" | "cycle" | "missingParam" | "unknownKind"
      | "conditionKindMismatch" | "groupUnknownNode" | "groupEmpty"
      | "groupDuplicate" | "nodeInMultipleGroups",
  nodeId: string | null,
  edgeId: string | null,
  groupId: string | null,
  message: string
}
```

| 约束 | 说明 |
|---|---|
| 不走 `CommandErrorDto` | 该结构有意不承载 `details`（`assistant-protocol/src/error.rs`）；校验失败需逐条定位，塞进 `message` 会让编辑器无法高亮 |
| 全量返回 | 一次返回全部命中项，不首错即停（[I69a §6.3](./I69a-agent-orchestration-base-spec.md)） |
| 定位三选一 | `nodeId` / `edgeId` / `groupId` 至少一个非 `null`；`kind` 与 I69a §6.3 的十类一一对应 |
| 类别闭集 | `kind` 用 Rust `enum`，前端不窄化、不做兜底分支 |
| 语义归属 | 判据全在 [I69a §6.3](./I69a-agent-orchestration-base-spec.md)，本文只承载形态 |
| 运行期不复用 | `start_workflow_run` 遇模板不合法（含版本不等，D22）仍走 `CommandError`（`Validation`）：那是命令被拒，不是编辑反馈 |

---

## 3 前端接缝

### 3.1 允许出现的界面

| 界面 | 内容 | 数据来源 |
|---|---|---|
| 模板列表 | 名称 / schema 版本 / 更新时间 | `list_workflow_templates` |
| 模板详情（只读） | 节点与边的结构化列表（非画布） | `get_workflow_template` |
| run 详情 | run 状态 / 节点状态 / 预算 / 结果跳转 | `get_workflow_run` + `list_workflow_node_runs` |
| run 列表 | 选叠加对象（模板 / 来源会话过滤） | `list_workflow_runs` |
| 运行入口 | 选模板 + 选会话 + 发起 | `start_workflow_run`；**表单形态以 [I69c §4.5](./I69c-agent-orchestration-canvas-spec.md) 为准**，本文只定数据来源与字段规则 |
| gate 操作 | proceed / reject + 备注 | `resolve_workflow_gate` |

> **现状（2026-11-22）**：`src/features/orchestration/` 已存在并注册为独立页面（路由 `/agent-orchestration`，侧栏键 `common.navigation.agent_orchestration`），当前是**画布占位**——渲染内置 Agent 预设、可拖动放置示例卡片、可缩放平移；数据全部来自前端示例（`canvasContentOf(null, …)` + `exampleCardOf`），不接任何后端命令、不持久化。编排已从观测页移除，观测页回到「只读观测」职责。R5 只在此占位之上补命令封装、事件订阅与键控投影。

**不出现**：画布编辑器、节点拖拽、连线绘制、自动布局（属 W3 / W4，见 [I69c](./I69c-agent-orchestration-canvas-spec.md)）。占位页现有的画布交互属画布侧既有设施（W1 / W2，I66 / I68 已归档，代码在 `src/features/canvas/`），本阶段不扩展、不接后端；R5 交付的接缝不复用其编辑行为。

### 3.2 读取纪律（K1–K12）

| # | 纪律 | 依据 |
|---|---|---|
| **K1** | 只经 §2.1 命令与 `workflow-run-event` 读写；不直连数据库、不读 Rust 内部状态 | INV8 |
| **K2** | run / 节点状态只来自台账命令结果与事件；**不得**从本地缓存、时间戳、消息列表反推 | INV8 |
| **K3** | 前端**不产生** `workflow-run-event`；事件只由宿主发出 | INV3 |
| **K4** | 预算未配置渲染为「未配置」，**不得**渲染 `0`，也不得以 `0` 表示无限 | INV11 |
| **K5** | `NodeRunState`（L2，7 值）与 `NodeState`（L1，7 值）**各自独立类型**，不互借、不互相转换 | [I69a §1.3](./I69a-agent-orchestration-base-spec.md) |
| **K6** | `skipped` 在编排语境必须带「调度阻断」限定语；不得与观测面的「报告未执行」混用 | [I69a §2.2](./I69a-agent-orchestration-base-spec.md) |
| **K7** | 前端**不做**就绪集合计算、并行判定、冲突域判定——这些只在 `schedule.rs` | [I69a §9.1](./I69a-agent-orchestration-base-spec.md) |
| **K8** | 前端只允许两类 run 级控制：`resolve_workflow_gate` 与**用户显式发起**的 `cancel_workflow_run`；不自动 resolve、不代填 `note`、不自动取消、不自动 `reconcile` | [I69a §9.5](./I69a-agent-orchestration-base-spec.md)；形态见 [I69c §4.6](./I69c-agent-orchestration-canvas-spec.md) |
| **K9** | 运行入口只有一个：`start_workflow_run`；不得在前端拼装 turn 请求或调用会话发送命令来「模拟」节点 | INV1 |
| **K10** | 不实现节点结果缓存、不做「已完成节点跳过重跑」 | D3 / D15 |
| **K11** | 模板编辑器与运行态叠加的形态不在本波；本波只交付只读详情与 run 面板 | §3.1 |
| **K12** | 前端卸载 / 刷新 / 断连**不影响** run：run 在宿主推进，前端重连后经命令拉全量 | D4 |

### 3.3 事件消费

| 项 | 规则 |
|---|---|
| 订阅 | `listen("workflow-run-event")`，按 `(runId, nodeId)` 归并到本地视图 |
| 非当前 run | 视图只维护**当前选中 `runId`** 的节点状态；其它 run 的事件只更新 run 列表项（若有），不建第二份节点视图。切换 run 时按 §3.3「重连」同一路径整表重拉，丢弃上一 run 的节点增量 |
| 落库 | 事件不写任何前端持久化（localStorage / IndexedDB） |
| 一致性 | 收到事件后只更新对应节点与 run 概览；**不**用事件序列重建 run 状态 |
| 重连 | 重连或页面恢复 → 调 `get_workflow_run` + `list_workflow_node_runs` 拉全量，丢弃本地增量 |
| 缺事件 | 视图与台账不一致时以台账为准；可提示调 `reconcile_workflow_run`，但**不得**前端自行修补状态。提示与入口的呈现归 [I69c §4.6](./I69c-agent-orchestration-canvas-spec.md)，且只由用户显式触发（K8） |

---

## 4 验收（A25、A27）

| # | 判据 | 检查方式 |
|---|---|---|
| **A25** | 两个 run 并行时事件不串台（按 `(runId, nodeId)` 键控） | 集成测试（O12） |
| **A27** | **仅约束 R5 交付时的目录形态**：`src/features/orchestration/` 出现且只出现：模板列表 / 模板详情（只读）/ run 列表 / run 详情 / 运行入口；K1–K12 逐条可核。**豁免**：R5 之前的独立编排页（`OrchestrationPage.tsx`，路由 `/agent-orchestration`）当前为画布占位——纯前端示例数据、不接后端命令、不持久化，不计入；R5 落地时该占位须下线或收敛为上述五项之一。**不约束 W3 / W4**：模板编辑器与运行态叠加在同一 feature 内落地时，其形态与判据由 [I69c §6](./I69c-agent-orchestration-canvas-spec.md) 的 UI1–UI15 接管，本条不再作为审查依据 | 目录审查 + 单测（K1–K12） |

> 其余判据 A1–A24、A26、A28–A32 在 [I69a §13](./I69a-agent-orchestration-base-spec.md)。

---

## 5 本卷相关未决问题

全表见 [I69a §15](./I69a-agent-orchestration-base-spec.md)；与本卷直接相关的三条：

| # | 问题 | 当前处置 |
|---|---|---|
| OQ6 | HITL 是否支持「多选项 + 表单」 | v1 只有 `proceed` + `note`；gate 输入 DTO 不为此预留字段 |
| OQ7 | run 预算的 token 实时闸门 | 只归因不闸门；`budgets` 不设 token 字段 |
| OQ8 | 模板版本与用户可见版本号的关系 | `version` 只表示 schema 版本；DTO 不引入用户版本号 |

> OQ9 已定案（D22）：`version` 与当前 schema 不等时，`get_workflow_template` 与 `start_workflow_run` 均返回 `CommandError`（`Validation`），不降级、不迁移。
>
> **关闭权**：OQ 只由 [I69a §15](./I69a-agent-orchestration-base-spec.md) 关闭。本卷只承接裁定输入（OQ6 的多选项形态、OQ8 的历史版本）；触发裁定时回提 I69a，由 I69a 同批次更新全表，本卷无权自行关闭或另立编号。

---

## 6 上游承接清单（附录）

> 与 [I69a §16.1](./I69a-agent-orchestration-base-spec.md) 一一对应：本表是本卷对上游冻结项的**落点清单**。上游新增冻结项时，本表与 [I69a §16.1](./I69a-agent-orchestration-base-spec.md) 同批次更新；缺任一落点即视为上游成果丢失，本卷不得标记为可实施。

| 承接 ID | 内容（不复述，回上游） | 本卷落点 |
|---|---|---|
| `INV1` | 唯一发射者 | K9（§3.2） |
| `INV3` | 观测只读 | §1.1 事件单向、§2.3 |
| `INV4` | 失败不回灌 | K12 |
| `INV5` | 执行节点集合受 O3 约束 | §2.2 节点类别闭集 |
| `INV6` | 双模板分离 | §2 DTO 不引用 turn 模板类型 |
| `INV7` | 静态图 | §2.4 校验错误闭集 |
| `INV8` | 台账唯一 | K1 / K2 |
| `INV9` | 取消树幂等 | `cancel_workflow_run`（§2.1） |
| `INV10` | 会话互斥不被绕过 | `nodeConversations` 解析（§2.2） |
| `INV11` | 预算未配置 ≠ 0 | `budgets`（§2.2）/ K4 |
| `INV12` | wire 最后冻结 | §2 头部与 §2.3 |
| `INV13` | 坐标不进语义 | `WorkflowTemplateDto` 的 `spec` / `layout` 分列 |
| `D5` | 不落原始事件 | §1.1 事件是广播、无回执 |
| `D6` | 并发键 `(run_id, node_id)` | §1.1 事件键 |
| `D8` | 预算分层 | §2.2 `budgets` 无 token 字段 |
| `D9` | HITL 回答只进台账 | §2.1 `resolve_workflow_gate` |
| `D12` | run 与会话归属 | §2.2 `originConversationId` / `nodeConversations` |
| `D13` | 坐标与语义分列 | §2 `WorkflowTemplateDto` |
| `D14` | 冲突域 | 前端不做判定（K7） |
| `D18` | `group` 不是节点类别 | §2.2 节点类别闭集 |
| `D22` | 版本不等即拒 | §2.4 / §5 |
| `CONST-NODE-KIND-2` · `CONST-EDGE-COND-3` · `CONST-INVALID-GRAPH-10` · `CONST-RUN-STATE-7` · `CONST-NODERUN-STATE-7` · `CONST-CMD-10` · `CONST-EVENT-1` · `CONST-BUDGET-4` · `CONST-K-12` | 常量值见 [I69a §16.2](./I69a-agent-orchestration-base-spec.md) | §2 wire 与 §2.4 |
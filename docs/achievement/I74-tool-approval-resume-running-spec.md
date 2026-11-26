# I74 — 审批结束后工具恢复执行态

| 项 | 值 |
|---|---|
| 日期 | 2026-11-25 |
| 类型 | 修复规格（最小改动） |
| 关联 | I72-approval-card-decision-design（已归档）、I60-tool-running-status-spec（已归档）、I55-agent-turn-observability-spec（已归档） |
| 一句话 | 审批通过后补发一次 `Started`，让工具视图从 `awaiting_user` 回到 `running` |

## 1 缺陷事实

| 编号 | 事实 |
|---|---|
| N1 | `Started` 由 `pipeline.execute` 在**批次入口**对每个调用发射，早于 `Gateway` 阶段的审批弹窗（`src-tauri/crates/assistant-tools/src/runtime/pipeline.rs`） |
| N2 | 审批通过后 `Gateway` 返回 `GatewayDecision::Allowed`，与「策略直通 / 授权命中」**同形**，下游无从区分是否刚批准过（`src-tauri/crates/assistant-tools/src/runtime/gateway/mod.rs`、`.../gateway/decision.rs`） |
| N3 | `ApprovalClosed` 只在等待 future 被 drop（超时 / turn 取消）时发；正常决策走 `guard.decided = true` 分支**故意不发**（`src-tauri/src/adapters/approval.rs`） |
| N4 | 前端 `applyAgentToolCall` 的 `started` 分支是唯一能把状态置 `running` 的入口；7 个相位中没有任何一个能把 `awaiting_user` 推进回 `running`（`src/chat/core/toolCallView.ts`） |
| N5 | I60 §3.3 事件表只定义「进入 `awaiting_user`」，未定义「退出」；§5 的 L1–L12 也没有登记这一条 |

可观察后果：

| 编号 | 现象 | 成因 |
|---|---|---|
| S1 | 批准后状态标签一直是「等待确认」，看不到「执行中」与执行计时，全局指示器不出现 | N1 + N2 + N4 |
| S2 | 批准按钮永久转圈 | `ApprovalCard.decide` 成功路径不清 `submitting`，只能等组件卸载 |
| S3 | 执行时长超过审批超时后，卡片翻成「已超时，本次按拒绝处理」并置灰 | 倒计时 `deadline` 基于 `startedAt`，不因批准重置；`expired` 只读本地时钟 |

S3 是**事实错误**（工具已批准并在执行），不是文案瑕疵。

## 2 目标与非目标

**目标**

1. 批准后 1 帧内工具视图回到 `running`，执行计时从批准时刻起算。
2. S1 / S2 / S3 一并消失，且不改 `ApprovalCard` 的交互逻辑。
3. 不新增 wire 相位、不改 DTO、不跑 `npm run contract:gen`。

**非目标**

1. 不修 `ask_user`（`AwaitingAnswer` 之后同样没有回到 `running` 的事件）；本规格只覆盖审批，`ask_user` 待同类修复时复用同一机制。
2. 不引入执行期进度事件（I60 L6 维持原状）。
3. 不改「超时只置灰、不自动 Deny」的既有约定。

## 3 方案

### 3.1 后端

**a. 决策值域增一个变体**（`src-tauri/crates/assistant-tools/src/runtime/gateway/decision.rs`）

```rust
pub enum GatewayDecision {
    Allowed,
    /// 用户（或会话/全局授权）在本次弹窗中批准；与「策略直通」区分，供执行前补发 `Started`。
    AllowedAfterApproval,
    Denied { action: String, resource: String },
    RejectedByUser { reason: Option<String> },
    ApprovalTimedOut { timeout: Duration },
}
```

保留 `Allowed` 而不改成结构体变体，是为了让「策略直通 / `GrantHit`」的既有断言与匹配保持不变。

**b. 三处决策分支改返回值**（`src-tauri/crates/assistant-tools/src/runtime/gateway/mod.rs`）

| 分支 | 现值 | 改为 |
|---|---|---|
| `ApprovalWait::Decided(ApprovalDecision::Approve)` | `Allowed` | `AllowedAfterApproval` |
| `ApprovalWait::Decided(ApprovalDecision::AllowSession)` | `Allowed` | `AllowedAfterApproval` |
| `ApprovalWait::Decided(ApprovalDecision::AllowAlways)` | `Allowed` | `AllowedAfterApproval` |
| `pending.is_empty()`（授权已命中） | `Allowed` | **不变** |
| 策略直通（`Allow` 规则） | `Allowed` | **不变** |

**c. 执行前补发 `Started`**（`src-tauri/crates/assistant-tools/src/runtime/pipeline.rs`，`run_call` 的 `match decision`）

```rust
match decision {
    GatewayDecision::Allowed => {}
    GatewayDecision::AllowedAfterApproval => {
        sink.emit(PipelineEvent::Started {
            tool_call_id: call.tool_call_id.clone(),
            tool_name: call.tool_name.clone(),
            args_preview: args_preview(&call.tool_name, &call.arguments),
        })
        .await;
    }
    GatewayDecision::Denied { .. } => { /* 不变 */ }
}
```

位置在 `match decision` 内、`sink.emit(entered(PipelineStage::Execute))` 之前。`args_preview` 与首次 `Started` 同源同值，前端摘要不会变化。

### 3.2 前端

只改一处：`started` 分支清掉残留的审批对象（`src/chat/core/toolCallView.ts`）。

```ts
case "started": {
  const current = views.find((view) => view.callId === call.callId);
  return upsert(views, call.callId, call.name, {
    status: "running",
    startedAt: now,
    finishedAt: null,
    resultText: null,
    compact: null,
    argsText: current?.argsText ?? "",
    argsPreview: payload.argsPreview,
    approval: null,
  });
}
```

`approval: null` 是必需项：`awaitingApproval` 由 `status` 派生，渲染不会出错，但视图里会残留 stale 审批对象。`question` 不在本规格范围（见 §2 非目标 1）。

`ApprovalCard` **不改**：状态一旦为 `running`，`awaitingApproval` 判定失效，气泡改渲染普通工具行，卡片组件卸载，`submitting` / `remainingMs` / `deadline` 随组件销毁，S2 与 S3 自然消失。

### 3.3 事件序列（批准一次 `shell`）

| 序 | 事件 | 前端 `status` |
|---|---|---|
| 1 | `Started`（批次入口） | `running` |
| 2 | `AwaitingApproval` | `awaiting_user`（卡片） |
| 3 | `Started`（审批通过后重发） | `running`（卡片卸载，行显示「执行中」+ 计时，指示器出现） |
| 4 | `Finished` / `Failed` | `success` / `error` |

`Started` 对同一 `callId` 允许出现两次：第二次的语义是「审批结束，执行起点」。这与 I60 §3.2 约束 1「`startedAt` 由 `Started` 覆写为执行起点」一致——批准时刻正是真正的执行起点。

### 3.4 为何复用 `Started` 而不新增相位

| 方案 | 代价 | 结论 |
|---|---|---|
| 新增 `ApprovalResolved` 相位 | 改 `AgentToolCallPayloadDto` + `contract:gen` + 前端新分支 + I60 §3.3 事件表重写 | 拒绝：收益与成本不匹配 |
| 重发 `Started` | 后端 3 处 + 前端 1 行；语义已被 I60 §3.2 覆盖 | **采用** |
| 前端自行把 `awaiting_user` 推回 `running` | 前端需在提交成功后臆造「已执行」事实 | 拒绝：违反 I72 §2.4「前端不产生事实」 |

## 4 边界

| 编号 | 情形 | 行为 |
|---|---|---|
| B1 | 用户拒绝 | 不重发；卡片保持到 `Failed`（`submitting` 保持置灰，天然防重复决策） |
| B2 | 审批超时 | 不重发；`ApprovalTimedOut` → `Failed(timeout)`，与现状一致 |
| B3 | 授权命中（`GrantHit`）或策略直通 | 不重发，本就无 `awaiting_user` 区间 |
| B4 | 并发批次内多个调用各自弹窗 | 每个 `callId` 独立判断，互不影响 |
| B5 | 观测面 / 节点图 | `applyToolCall` 只消费 `awaitingApproval` / `awaitingAnswer` / `approvalClosed`，重发的 `Started` 被忽略；`StageEntered(Execute)` 仍把 `pipeline` 节点置 `running` |
| B6 | `confirmationWaitMs` | 仍由 `ApprovalWaitRegistry::peek` 在 `Finished` / `Failed` 时取值，与重发无关 |
| B7 | 历史回读 | 落库不含运行期事件，重发 `Started` 不写入任何持久化结构 |

## 5 测试

**后端**

1. `src-tauri/crates/assistant-tools/tests/pipeline.rs` 的 `approved_call_runs_and_reports_events_in_order`：`stateful_events` 由 2 条变 3 条（`Started` → `Started` → `Finished`）。
2. 同文件 `rejected_by_user_is_tagged_denied` 补事件断言：仍只有 `Started` + `Failed`（证明拒绝不重发）。
3. `src-tauri/crates/assistant-tools/tests/gateway.rs`：由 prompter 决策产生的 `Allowed` 断言改为 `AllowedAfterApproval`（至少覆盖 `gateway_always_approve_persists_key_and_skips_second_prompt` 的 first、「用户批准后放行」、`command_grant_is_not_reused_after_enforcement_downgrade` 的 first）；`GrantHit` 与策略直通仍断言 `Allowed`。以 `cargo test` 全绿为准。

**前端**

4. `src/chat/core/__tests__/toolCallView.test.ts` 新增用例：`awaitingApproval` → `started`，断言 `status === "running"`、`startedAt` 被覆写、`approval === null`、`argsText` 保留。

## 6 验收（手工）

| 编号 | 步骤 | 期望 |
|---|---|---|
| A1 | 触发需审批的 `shell`（如 `ping` 长跑）并批准 | 卡片立即消失；该行显示「执行中」并计时；全局指示器出现 |
| A2 | 同 A1 但执行时长超过审批 `timeoutMs` | 全程不再出现「已超时，本次按拒绝处理」 |
| A3 | 拒绝一次审批 | 卡片保留到终态，行显示「失败」，不出现「执行中」 |
| A4 | 用「始终允许」批准后再触发同键调用 | 第二次不弹窗、无 `awaiting_user` 区间，行为与现状一致 |

## 7 实施顺序

1. `decision.rs` 加变体 → 2. `gateway/mod.rs` 三处返回值 → 3. `pipeline.rs` 补发 → 4. `toolCallView.ts` 清 `approval` → 5. 改/加测试 → 6. `cargo test`（cwd = `src-tauri`）与前端测试。

无 DTO 变更，不跑 `contract:gen`；无配置项变更。
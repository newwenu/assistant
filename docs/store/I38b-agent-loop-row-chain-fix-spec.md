# I38b-row-chain-fix — 行链单链不变量与多工具批次修复规格

| 项 | 值 |
|---|---|
| 状态 | 现行（优化规格，决策已定稿；不属阶段门禁，不进 roadmap 文档地图） |
| 日期 | 2026-11-08 |
| 上级 | [I38b spec](./I38b-agent-loop-spec.md)（§6.1 落库、A20 验收）；[I38 roadmap](./I38-agent-roadmap.md) §2（P4 / P6） |
| 关联 | [I38b impl-guide](./I38b-agent-loop-impl-guide.md)（C4.20 / C4.40）；[I38b tasklist](./I38b-agent-loop-tasklist.md)（核对缺陷 1、C7.14） |
| 影响范围 | `src-tauri/crates/assistant-agent/src/agent/loop_executor/{persist,terminal,mod}.rs`、`src-tauri/src/adapters/message_store.rs`、`src-tauri/crates/assistant-storage/src/conversations/{branch,mod}.rs`、`src-tauri/crates/assistant-agent/tests/loop_executor.rs`、`src-tauri/crates/assistant-agent/src/kernel/ports/test_doubles.rs`、`src-tauri/crates/assistant-storage/tests/conversations.rs` |

> 本文是「多工具批次打断活跃分支」的**唯一权威规格**，性质为**优化规格**：它只约束行链不变量与本次修复的验收，**不是阶段执行规格**，不改变 P0–P7 门禁、不进 roadmap 文档地图。缺陷事实、推演、影响面与根因见 §2，规范条款见 §3–§7。
> 本文只定义「应当怎样」与「如何验收」，不展开实施步骤；批次划分见 §9。

---

## 1 目标与非目标

### 1.1 目标

| # | 交付 |
|---|---|
| 1 | 定义行链不变量 INV1–INV6（§3），使「写入顺序 = 分支链顺序 = 叶子推进顺序」三者恒等 |
| 2 | 修复 D1：同一迭代内 ≥2 个工具调用时，活跃叶子停在首条 tool 行 |
| 3 | 修复 D5：`ensure_row` / `persist_row_as_new` 不回写 `last_row_id`（方案 A 的前置条件） |
| 4 | 同步放宽 D2 分支深度上限：该上限在现状下对单工具批次已偏紧（约 165–250 轮即整会话打不开），修复后链更完整，必须同批放宽 |
| 5 | 补强单测与存储层用例，使多工具批次、`skipped`、四类终态出口均被单链断言覆盖 |
| 6 | 修订父规格中与 INV1 冲突的落库口径（§4） |

### 1.2 非目标

- **不改** `advance_active_branch` 的 `parent == leaf` 判定。该判定是「不打断用户手动切换分支」的正确保护，缺陷出在写入侧违反顺序追加前提，放宽判定只会掩盖不变量违例。
- **不做**存量数据迁移。已产生的错位行不影响新写入；若确需修复历史会话，另立迁移批次。
- **不做**投影期「自动修复叶子」（例如详情读取失败时回退到全量 `get_messages`）。这会让损坏数据看起来正常，并制造假分支视图。
- **不做**端口收口（方案 B，§6.2）。它改变 `MessageStore` 签名并影响全部调用点，作为独立批次。
- **不改**前端 `mergeMessagesPreservingFrontendState` 的「以后端列表为准」策略。该策略是 D1 后果的放大镜，不是根因。

---

## 2 缺陷判定（修复前事实）

> 本章记录**修复前**的代码事实与时间线，用于说明根因；现状已按 §5 改完。引用一律用符号名，不写行号（行号会随改动腐化）。

### 2.1 三个写入口

| 入口 | 位置 | 写入的 `parent_id` | 是否推进叶子 |
|---|---|---|---|
| 命令层占位行 | `src-tauri/src/cmd/chat/insert.rs`（`insert_normal_append` / `insert_placeholder_assistant_message`） | 新 user 行父 = `detail.messages.last()`；占位 assistant 父 = 新 user 行 | 是（`update_active_branch_id` 无条件） |
| 迭代 assistant 行 | `src-tauri/crates/assistant-agent/src/agent/loop_executor/persist.rs` 的 `ensure_row` / `persist_row_as_new` | `state.last_row_id` | 是（`create_assistant_message` 内 `advance_active_branch`，`src-tauri/src/adapters/message_store.rs`） |
| 工具结果行 | 同文件 `append_tool_results` | **固定为形参 `parent_id`**，三处调用点均传 `state.msg_id`（本迭代 assistant 行） | 是（`append_tool_message` 内 `advance_active_branch`），但条件不满足时不推进 |

`state.last_row_id` 初值 = 占位行 id（`loop_executor/mod.rs` 的 `RoundState` 初始化），修复前**只有** `append_tool_results` 会更新它（失败分支回退为形参值）。

### 2.2 叶子推进规则

`src-tauri/crates/assistant-storage/src/conversations/branch.rs` 的 `advance_active_branch`：

```rust
let extends_active = match (active.as_deref(), parent_id) {
    (None, _) => true,
    (Some(active_id), Some(parent)) => active_id == parent,
    (Some(_), None) => false,
};
if extends_active { update_active_branch_id(conn, conversation_id, new_id)?; }
```

即**只有当新行父节点正好是当前叶子时**叶子才前移。该函数失败时**不报错、不告警**，静默保持原叶子。

### 2.3 投影读取规则

`get_conversation_detail`（`conversations/mod.rs`）在有 `active_branch_id` 时只走 `trace_active_branch`（`branch.rs`）：从叶子沿 `parent_id` 逐级回溯，无父即止。因此**任何不在该链上的行对上层不可见**。

### 2.4 缺陷时间线（D1）

场景：第 1 迭代返回 3 个工具调用，第 2 迭代返回最终文本。占位 assistant 行记 `A1`（父 = `U1`），第 2 迭代 assistant 行记 `A2`。

| 步 | 写入行 | `parent` | 写入前叶子 | 推进判定 | 写入后叶子 |
|---|---|---|---|---|---|
| 0 | `U1` | `None` | `None` | `(None, _)` → 推进 | `U1` |
| 0 | `A1`（占位） | `U1` | `U1` | 相等 → 推进 | `A1` |
| 1 | `T1` | `A1` | `A1` | 相等 → 推进 | **`T1`** |
| 1 | `T2` | `A1` | `T1` | 不等 → **不推进** | `T1` |
| 1 | `T3` | `A1` | `T1` | 不等 → **不推进** | `T1` |
| 2 | `A2` | `T3` | `T1` | 不等 → **不推进** | `T1` |

终态：叶子停在 `T1`，可回溯链为 `U1 → A1 → T1`（实际写入 6 行）。`A2`、`T2`、`T3` 全部脱离活跃分支。

对照单工具批次：`T1` 推进后叶子 = `T1`，`A2` 父 = `T1` → 推进，链完整。**这是现有测试全绿的原因。**

### 2.5 错位结果

- 写入顺序：`U1, A1, T1, T2, T3, A2`
- 父链：`A1←U1, T1←A1, T2←A1, T3←A1, A2←T3`
- 叶子：`T1`
- 结论：「父节点正确」与「叶子正确」不再等价——历史修复（[I38b tasklist](./I38b-agent-loop-tasklist.md) 核对缺陷 1）只修正了下一迭代新行的父节点，未修正叶子推进。

### 2.6 复现条件

- **必要条件**：单个迭代（一次模型响应）内产生 ≥2 个 `tool_calls`，且该批次有结果行写入。
- **典型触发**：模型并行调用两个以上只读工具（同时读两个文件、读文件 + 列目录、grep + read 等）。
- **不触发**：每迭代恰好 1 个工具调用、纯文本响应、首迭代即失败。

### 2.7 影响面

| 级别 | 影响 | 机制 |
|---|---|---|
| P0 | 流结束瞬间 UI 丢消息 | `emit_stream_finished`（`src-tauri/src/adapters/event_sink.rs`）按 `active_branch_id` 重建列表；前端 `mergeMessagesPreservingFrontendState`（`src/services/chatLifecycleEvents.ts`）以**后端列表**为结果集并整体 `setState`，最终回答与第 2..n 个工具卡片消失 |
| P0 | 下一轮上下文被截断 | `insert_normal_append`（`src-tauri/src/cmd/chat/insert.rs`）用 `detail.messages.last()` 作为新 user 行父节点，得到 `T1`；投影期 `prompt_builder.rs` 的 `flush_pending` 把其余调用补成 `is_error = true` 的「未执行：本轮中断」，模型看不到上一轮最终回答 |
| P1 | 库内积累不可见行 | `A2`、`T2`、`T3` 仍在 `messages` 表内，详情不返回，但 `get_subtree_size` / `delete_subtree` 仍统计与删除 |
| P1 | 缺失结果被伪装成取消 | `src-tauri/src/cmd/conversation_utils.rs` 找不到结果时置 `Cancelled`，真实成功过的调用显示为「已取消」 |
| P0 | 验收不达标 | 父规格 A20（`I38b-agent-loop-spec.md` §7 验收表）两项断言不成立 |

### 2.8 现有覆盖为何全绿

| 覆盖点 | 事实 | 缺口 |
|---|---|---|
| `tests/loop_executor.rs` 的 `assert_single_chain` | 只检查「每行父 = 上一行」 | 修复前无任何用例把多工具批次喂给它 |
| A20 用例 | 三个迭代各只脚本化 1 个工具调用 | 单工具批次**恰好**满足单链 |
| A14 用例（`a14_batch_runs_once_and_keeps_port_order`） | 有 3 个调用，但只断言执行顺序 | 未断言行链 |
| `InMemoryStore::append_tool_message`（`test_doubles.rs`） | 只记录 `parent_id`，不维护叶子 | 替身无法复现叶子停滞 |
| 存储层 `advance_active_branch_tracks_multi_iteration_chain`（`tests/conversations.rs`） | 每迭代 1 条 tool 行 | 与缺陷条件正交 |
| 端到端 C7.14（`I38b-agent-loop-tasklist.md` C7 段） | 批次级跳过 | 无端到端兜底，属已接受缺口 |

### 2.9 根因

| # | 根因 | 说明 |
|---|---|---|
| R1 | 父节点由调用方指定，没有唯一归属 | `AppendToolMessageReq.parent_id` 与 `CreateAssistantReq.parent_id` 都是调用方给值，`RoundState.last_row_id` 由编排层手工维护；两个字段都是 `String`，类型系统拦不住 `msg_id` / `last_row_id` 的混用。更隐蔽的是**维护不对称**：修复前只有 `append_tool_results` 回写 `last_row_id`，`ensure_row` / `persist_row_as_new` 不回写 |
| R2 | 叶子推进条件过窄 | `advance_active_branch` 的 `parent == leaf` 同时承担两个语义：「不打断用户已选择的分支」（正确，须保留）与「写入顺序应等于分支顺序」（隐含假设，未表达）。批量同父写入违反后者，被前者静默拒绝，且函数不报错、不告警 |
| R3 | 测试替身不建模分支推进 | `InMemoryStore` 只记录 `parent_id`，不维护叶子；`assert_single_chain` 只检查「每行父 = 上一行」。多工具批次会立刻失败，但没有任何用例把它喂给该断言，于是替身缺口未被暴露 |
| R4 | 规格自相矛盾 + 端到端验收被跳过 | 实现依据（§6.1 / C4.20）要求同父兄弟，验收依据（A20）要求严格单链，两者在多工具批次下不可同时成立；唯一能发现冲突的 C7.14 又随批次整体跳过，而单测覆盖恰好避开多工具批次 |

> R2 的处置是**修正写入侧**（§5.1），不是放宽判定；R1 的终态处置是端口收口（§6.2），本批先消除维护不对称（§5.2）。

---

## 3 不变量（本规格的规范条款）

以下不变量是行链的**唯一判定口径**，实现与测试都必须按此表述。

| 编号 | 条款 | 违反后果 | 验收 |
|---|---|---|---|
| INV1 | `messages` 的写入顺序等于 `parent_id` 链顺序：每条新行的父节点是**上一次写入的行**（对话首行父为 `NULL`） | 链断开，部分行不可见 | A20a / §7.2 |
| INV2 | 在无用户手动切换分支的前提下，`conversations.active_branch_id` 恒等于**最后一条写入行**的 id | 详情返回截断链 | A20a / A20c / §7.2 |
| INV3 | `role='tool'` 行逐条链式追加（父 = 前一行），`sibling_index = 0`，**不构成同父兄弟**、不产生分支 | 叶子停滞（本缺陷） | A20a / A20b / A14+ |
| INV4 | 新迭代 assistant 行的父 = 上一迭代**最后一条写入行**（tool 行或 assistant 行均可） | 跨迭代断裂 | A20a |
| INV5 | 终态出口写完后：(a) `active_branch_id` 恒指向**最后一条写入行**；(b) `final_message_id` 指向承载终态文本的 assistant 行，且该行**必可从叶子回溯到**（在活跃分支上）。预算耗尽 / 无进展 / 收尾调用均**新建终态行**（收尾调用前本迭代 assistant 行已在第 4.5 步 finalize，故 `persist_final_text` 走 `persist_row_as_new`），终态行 = 最后写入行 = 叶子；**仅工具执行期取消**（`cancelled_tool_exec`）复用本迭代 assistant 行——该行先于其 tool 行写入，故叶子 = 最后一条 cancelled tool 行，终态行早于叶子但仍在链上，属既定语义，不算违例 | 终态回答不可见 | A20c |
| INV6 | 一次 turn 内写入的行，全部出现在 `get_conversation_detail` 的返回集合中（除用户显式切换隐藏的分支） | 上下文与 UI 均缺行 | A20d / §7.2 |

> INV1–INV6 是**必须项**，由方案 A 一次达成。原子性（INV7：追加路径的 insert / status / 叶子推进在同一事务内）属方案 B 的扩展条款，见 §6.2，不在本批验收。

---

## 4 口径修订（规格冲突决议）

父规格与实施指南中的三处表述与 INV3 冲突，须按下表修订。**修订后 S1/S2/S4 与 A20 不再矛盾。**

| 编号 | 文档位置 | 原文要点 | 决议 |
|---|---|---|---|
| S1 | `I38b-agent-loop-spec.md` §6.2 落库（D11） | 「结果行 `parent_id` 指向对应 assistant 行，`sibling_index = 0`」 | 改为「结果行 `parent_id` 指向**上一条写入行**（首条 tool 行指向本迭代 assistant 行），`sibling_index = 0`」 |
| S2 | `I38b-agent-loop-impl-guide.md` C4.20 / 辅助函数落库职责表 / 执行器骨架 | 「`store.append_tool_message(parent = msg_id)`」，返回最后一条 tool 行 id | 改为「`store.append_tool_message(parent = last_row_id)`，逐条推进 `last_row_id`」；`append_tool_results` 不再接收 `parent_id` 形参 |
| S3 | `I38b-agent-loop-spec.md` §7 验收表（A20） | 「`parent_id` 依次指向前一条」「`active_branch_id` 最终指向 `assistant₃`」 | **保持不变**，作为权威验收 |
| S4 | `I38b-agent-loop-spec.md` §9 风险表 | 「结果行 `parent_id` 挂 assistant 行、`sibling_index = 0`，不产生新分支」 | 改为「结果行链式挂在前一行、`sibling_index = 0`，不产生新分支」；「不产生新分支」结论不变 |
| S5 | `I38b-agent-loop-tasklist.md` C7.14 | 端到端行链 / 活跃叶验收「跳过」 | 保持跳过决定；等价覆盖下沉到 §7 的存储层用例与单测 |
| S6 | `I38b-agent-loop-tasklist.md` 核对缺陷 1 | 历史修复删除 `append_tool_results` 末尾的 `last_row_id = msg_id` 回写 | 记录为「修复不完整」：父节点已修、叶子未修，并留下 D5 的维护不对称 |

> S1/S2/S4 的修订必须与本批代码改动同批提交，避免规格与实现再次分叉。

---

## 5 改动清单

### 5.1 D1：`append_tool_results` 改为链式追加（必做）

`persist.rs` 的 `append_tool_results` 删除形参 `parent_id: String`，父节点在函数内部逐条取 `state.last_row_id`：

```text
append_tool_results(state, ctx, results):
    for result in results:
        req = AppendToolMessageReq { parent_id: state.last_row_id.clone(), ... }
        row_id = store.append_tool_message(req).await?
        state.last_row_id = row_id
```

- 失败分支不再回退 `last_row_id`（保留上一次成功值，与「已写入的行仍在链上」一致）。
- 内存侧 `request.chat_messages.push(...)` 逻辑不变（投影按线性顺序匹配，不依赖 `parent_id`）。

### 5.2 D5：assistant 行回写链尾（方案 A 前置，必做）

| 函数 | 现状 | 目标 |
|---|---|---|
| `ensure_row` | 建行后只写 `state.msg_id` | 追加 `state.last_row_id = msg_id` |
| `persist_row_as_new` | 建行后只写 `state.msg_id` | 追加 `state.last_row_id = msg_id` |

**不做这一项，方案 A 会在下一次「有工具调用」的迭代立刻断裂**：迭代 2 新建 `A2`（父 = `T3`）后 `last_row_id` 仍是 `T3`，其 tool 行父 = `T3`，而叶子已推进到 `A2`，`advance_active_branch` 条件不成立。

回写完成后语义才对齐：`msg_id` = 本迭代 assistant 行；`last_row_id` = 链尾行。

### 5.3 调用点同步（必做）

三处均改为不传 `parent_id`：

| 调用点 | 位置 |
|---|---|
| 正常迭代 | `loop_executor/mod.rs` 的 `run_iterations` 内 `append_tool_results` 调用处 |
| 取消出口 | `terminal.rs` 的 `cancelled_tool_exec` |
| 收尾调用 | `terminal.rs` 的 `wrap_up_on_last_iteration` |

### 5.4 D2：分支深度上限（必做，与 5.1 同批）

`branch.rs` 的 `MAX_BRANCH_DEPTH = 1000`，超限即返回 `"... possible data corruption"` 错误（`trace_active_branch` 与 `create_branch` 各一处），`get_conversation_detail` 随之失败。

| 形态 | 每轮链上行数 | 触顶轮次 |
|---|---|---|
| 现状·单工具批次（缺陷不触发，链完整） | 4–6 行（每迭代 assistant + tool 共 2 行） | 约 165–250 轮 |
| 现状·多工具批次（缺陷触发） | 链在首条 tool 行截断，约 3 行 | 约 330 轮（但内容已丢，属缺陷后果，不能算作「更安全」） |
| 修复 D1 后（链式追加） | 5–6 行（每迭代 2 行） | 约 165–200 轮 |

> 关键结论：该上限在**现状**下对「单工具批次」已偏紧（约 165–250 轮即失败，失败表现是整个会话打不开），修复只是让此前被截断的行也计入链。因此放宽与 D1 必须同批，且不能指望「修复前更安全」。

决议：环检测保留（`trace_active_branch` / `create_branch` 已有 `visited`），深度上限改为「防环冗余」的宽松值（建议 10000）或「超过阈值只告警不失败」。**该修订必须与 5.1 同批**，否则长会话由「丢消息」变成「打不开」。

### 5.5 其余关联问题处置

| 编号 | 问题 | 级别 | 处置 |
|---|---|---|---|
| D3 | `trace_active_branch` 逐行 N+1 查询（`branch.rs`） | P2 | 后续批次：recursive CTE 一次取回，或按 id 批量取 |
| D4 | `append_tool_message` / `create_assistant_message` 三条语句无事务（`message_store.rs`） | P1 | 方案 B 一并解决（`conn.unchecked_transaction()`，与 `branch.rs::create_branch` 现有写法一致） |
| D6 | tool 行 `sibling_index` 恒为 0（同父兄弟 index 重复） | P2 | 链式追加后同父兄弟消失，问题自然消解；若保留同父形态则须按同父计数分配 |
| D7 | `not_executed` 生成的 `skipped` 行走同一缺陷路径（生成于 `budget.rs`；收尾出口 `terminal.rs` 的 `wrap_up_on_last_iteration` 生成 skipped 并链式追加；预算切分在 `budget.rs` 的 `split_by_budget`） | P0 | 随 D1 一并修复，无需单独改动 |
| D8 | 缺失结果被投影成 `cancelled`（`src-tauri/src/cmd/conversation_utils.rs`），且 `flush_pending` 日志级别为 `debug`（`prompt_builder.rs`） | P1 | 后续批次：缺失至少 `log::warn!`（带 `conversation_id` / `call_id` / `message_id`）；状态语义不再等同「取消」 |

---

## 6 方案 A 与方案 B

### 6.1 方案 A：链式追加（本批实施）

即 §5.1 + §5.2 + §5.3 + §5.4。改动范围：2 个函数逻辑 + 2 处回写 + 3 处调用点 + 1 个常量。

达成效果：

- 链为 `A1 ← T1 ← T2 ← T3 ← A2`，`advance_active_branch` 的 `parent == leaf` 恒成立；
- `trace_active_branch` 可回溯全部行，`detail.messages.last()` 自然等于终态 assistant 行；
- `prompt_builder` 的配对修复不再被触发（每个调用都有配对结果）；
- 三处调用点的出口语义不变（取消出口的终态文本仍落在本迭代 assistant 行，见 INV5），且 `skipped` 行自动获得同一修复。

### 6.2 方案 B：父节点收口到端口（后续独立批次）

在 `MessageStore` 端口提供「追加一行到当前分支尾部」的原子操作：

```text
append_row(role, content, tool_call_id?, is_error?) -> Result<String, StoreError>
  // 事务内：读 active_branch_id 作为 parent → insert → update status
  //         → advance_active_branch（父已知，条件恒成立）→ 返回新 id
```

- 调用方不再持有 `last_row_id`，`parent_id` 从端口 API 消失，D1 的根因（父节点由调用方指定）被结构性消除；
- `create_assistant_message` 收口到同一语义（命令层占位行除外，它需要显式父 = 新 user 行）；
- 同时解决 D4（事务）与 INV7。

**约束**：端口签名变更需同步 `test_doubles.rs` 与全部调用点，**不得与方案 A 混在同一提交**，否则回滚粒度不可控。

### 6.3 不做什么

见 §1.2。补充一条：**不**用「写入时自动纠正叶子为最后一行」绕过 `advance_active_branch` 判定——那会把「用户手动选中的分支」也一并覆盖。

---

## 7 验收标准

### 7.1 单测层（`src-tauri/crates/assistant-agent/tests/loop_executor.rs`）

| 编号 | 用例 | 断言 |
|---|---|---|
| A20a | 第 1 迭代 `multi_tool_script` 3 个调用，第 2 迭代文本 | `assert_single_chain(&replay, "user-1")`；`assistant_rows` = 2；只有占位行挂在 `U1` 下；`final_message_id` = 最后写入行 |
| A20b | `max_tool_calls` 只允许 1 个，同批 3 个调用（含 `skipped`） | 链仍为单链；`skipped` 行同样链式；`is_error = true` 行数与 `skipped` 数一致 |
| A20c | 四类终态出口各 1 例：预算耗尽 / 无进展 halt / 收尾调用 / 工具执行期取消 | 四者共同：叶子 = 最后一条写入行，`final_message_id` 行可从叶子回溯。预算耗尽 / 无进展 / 收尾调用新建终态行，故 `final_message_id` = 叶子、其父 = 最后一条 tool 行；工具执行期取消复用本迭代 assistant 行，故 `final_message_id` = 该行、叶子 = 最后一条 cancelled tool 行 |
| A14+ | A14 现有 3 调用用例（`a14_batch_runs_once_and_keeps_port_order`） | 追加 `assert_single_chain`，锁住「执行顺序不影响写入链」 |

> 编号约定：`A20a`–`A20c` 为本规格新增单测；`A14+` 为既有 A14 用例的加强（不新编号）；`A20d` 专指 §7.4 的端到端验收，`A20e` 指深度上限验收，`A20f` 属方案 B 批次。

### 7.2 存储层（`src-tauri/crates/assistant-storage/tests/conversations.rs`）

新增用例，按 executor 真实产生的 `parent` 序列调用 `insert_message` + `advance_active_branch`：

```text
U1 → A1 → T1 → T2 → T3 → A2
断言 active_branch_id == A2
断言 get_conversation_detail().messages == [U1, A1, T1, T2, T3, A2]
断言 get_sibling_count(U1) == 1（无假分支）
```

这是不依赖 `tauri` 前提下唯一能复现「叶子推进」的层级，作为 D1 的回归锚点。

### 7.3 替身改造（`test_doubles.rs`）

`InMemoryStore` 增加 `active_leaf: Mutex<Option<String>>`：`create_assistant_message` / `append_tool_message` 内按 `parent == leaf` 推进，并提供 `active_leaf()` 读口。使单测可直接断言「叶子 = 终态行」，不必下沉到存储层。

### 7.4 验收口径

| 编号 | 标准 |
|---|---|
| A20a | 单迭代 3 个工具调用 + 下一迭代文本：`parent_id` 严格依次相连；`active_branch_id` 指向最后一条 assistant 行；`detail.messages` 返回全部 6 行；不存在挂在 `U1` 下的第二条分支 |
| A20b | 同批次含 `skipped`（预算裁剪）时，同上 |
| A20c | 四类终态出口：叶子均 = 最后一条写入行；`final_message_id` 行均可从叶子回溯（预算耗尽 / 无进展 / 收尾调用的终态行即叶子；工具执行期取消的终态行早于叶子，见 INV5） |
| A20d | 端到端（可手工回归）：多工具 turn 结束后 `ChatMessagesChangedPayload.messages` 含终态 assistant 行；下一轮 `insert_normal_append` 的新 user 行父 = 终态 assistant 行 |
| A20e | 构造 >1000 行的合法链，`get_conversation_detail` 仍成功（D2 验收） |
| A20f | 方案 B 批次：`append_tool_message` 内部任一语句失败时不留半写状态 |

### 7.5 命令

`cwd = src-tauri`：

- `cargo test -p assistant-agent`
- `cargo test -p assistant-storage`
- `cargo clippy -p assistant-agent -p assistant-storage --all-targets -- -D warnings`
- `cargo fmt --check`

---

## 8 风险与决策

| 风险 | 缓解 |
|---|---|
| 链长增长放大深度上限 / N+1 回溯 / 兄弟计数开销（多工具批次由被截断的约 3 行恢复为完整 5–6 行；单工具批次本就 4–6 行） | D2 与 D1 同批；D3 列入后续批次；A20e 覆盖深度 |
| 只改 `append_tool_results` 而漏改 D5，导致迭代 2 断裂 | §5.2 列为前置条件；A20a 覆盖跨迭代 |
| 规格与实现再次分叉（S1/S2/S4 仍写「挂 assistant 行」） | §4 修订与本批同批提交 |
| 误以为放宽 `advance_active_branch` 即可修复 | §1.2 / §6.3 明确禁止；保护用户分支选择 |
| 以「自动纠正叶子」掩盖违例 | 同上 |
| 存量错位行被误认为新缺陷 | §1.2 明确不做迁移；只验收新写入 |

**决策定稿**：

1. 父节点归属以「链尾」为唯一来源，`msg_id` 只表示本迭代 assistant 行。
2. tool 行不构成分支，`sibling_index` 恒 0。
3. 不修改 `advance_active_branch` 判定，不修改前端合并策略。
4. 深度上限的宽松化与链式追加绑定为同一变更单元。

---

## 9 实施顺序

| 顺序 | 内容 | 风险 |
|---|---|---|
| 1 | D1 + D5 + 三处调用点 + D2（方案 A） | 必须同批；单独改 D1 会在迭代 2 断裂 |
| 2 | A20a–A20c 单测 + A14+ 加强 + 7.2 存储层用例 + 7.3 替身 | 只增测试，无行为变更 |
| 3 | §4 文档口径修订 | 与 1、2 同批提交 |
| 4 | D4 事务 + D3 recursive CTE | 存储层改造，独立提交便于回滚 |
| 5 | D6 / D8 收尾 | 低风险，可合批 |
| 6 | 方案 B（端口收口） | 独立小批次，不与其他改动混合 |

---

## 10 与既有文档的关系

| 文档 | 关系 |
|---|---|
| [I38b-agent-loop-spec.md](./I38b-agent-loop-spec.md) | 父规格；本文修订其 §6.2 落库（D11）与 §9 风险表，§7 验收表的 A20 保持为权威验收 |
| [I38b-agent-loop-impl-guide.md](./I38b-agent-loop-impl-guide.md) | 本文修订 C4.20、辅助函数落库职责表、执行器骨架三处的 `parent = msg_id` 表述 |
| [I38b-agent-loop-tasklist.md](./I38b-agent-loop-tasklist.md) | 本文承接其「核对缺陷 1」的未完成部分；C7.14 跳过决定不变 |
| [I38b-agent-loop-row-chain-fix-tasklist.md](./I38b-agent-loop-row-chain-fix-tasklist.md) | 本规格的任务清单（C0–C7）；批次顺序与 §9 对齐，是任务级进度的唯一来源 |
| [I38-agent-roadmap.md](./I38-agent-roadmap.md) | 本文属**优化规格**，不改变 P0–P7 阶段门禁，也不进其 §4 文档地图；仅作 §2 阶段表 P6 行的背景说明——A26 修复（追加行即推进叶子）只覆盖单工具批次，多工具批次的叶子停滞由本文闭合 |
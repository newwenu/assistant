# I38b-row-chain-fix — 行链单链修复任务清单

> 规格：[I38b-agent-loop-row-chain-fix-spec.md](./I38b-agent-loop-row-chain-fix-spec.md)（§3 不变量、§5 改动清单、§7 验收标准、§9 实施顺序）
> 上级：[I38b-agent-loop-spec.md](./I38b-agent-loop-spec.md)（§6.1 落库、A20 验收）；[I38b-agent-loop-tasklist.md](./I38b-agent-loop-tasklist.md)（核对缺陷 1、C7.14）
> 顺序：C0 → C1 → C2 → C3 → C4 → C5 → C6 → C7
> 勾选条件：代码已改 **且** 对应验收已跑。
> **性质**：本清单属**优化规格**的配套任务清单，不改变 P0–P7 阶段门禁，不进 [roadmap](./I38-agent-roadmap.md) §4 文档地图；C6 为后续独立批次，不计入本批完成度。
> **任务项粒度**：一项 = 一个可独立完成、可独立验证的工作单元。语义与边界条件写规格，任务项只写「落点 + 判定条件」，不复制规格原文；任务项内**禁止写行号**，只写符号名（行号会随改动立刻腐化）。
> **状态唯一位置**：本清单是本规格任务级进度的唯一来源；规格正文不记录进度。本清单**不管理提交**（不写提交拆分 / 提交信息 / 回滚批次）。
> 范围：`assistant-agent/src/agent/loop_executor/{persist,terminal,mod}.rs`、`assistant-agent/src/kernel/ports/test_doubles.rs`、`assistant-agent/tests/loop_executor.rs`、`assistant-storage/src/conversations/branch.rs`、`assistant-storage/tests/conversations.rs`、`I38b` 系列文档口径。不涉及 `assistant-tools`、前端合并策略、`MessageStore` 端口签名（端口收口见 C6.5）。
> 测试位置：`assistant-agent` 单测一律放 `crates/assistant-agent/tests/loop_executor.rs` 集成测试；存储层用例放 `crates/assistant-storage/tests/conversations.rs`；不写 `#[cfg(test)]` 内联模块。
> 决策基线：规格 §8「决策定稿」4 条已定稿，开工后不再新增决策；遇到未覆盖问题先回写规格，不在代码里临时拍板。

---

## C0 口径与前置确认

- [x] C0.1 确认缺陷触发条件：单迭代 ≥2 个 `tool_calls` 且有结果行写入（规格 §2.6）；单工具批次与纯文本响应不触发
- [x] C0.2 确认权威验收为父规格 A20（`parent_id` 依次指向前一条、`active_branch_id` 指向最终 assistant 行）；规格 §4 的 S1/S2/S4 属「实现依据待改」，A20 不改
- [x] C0.3 确认**不改** `advance_active_branch` 的 `parent == leaf` 判定：该判定保护用户手动切换分支，缺陷在写入侧违反顺序追加前提（规格 §1.2 / §6.3）
- [x] C0.4 确认本批不做存量数据迁移、不做投影期「自动修复叶子」、不与方案 B（端口收口）混合提交
- [x] C0.5 确认字段语义基线：`msg_id` = 本迭代 assistant 行；`last_row_id` = 链尾行（规格 §5.2 末尾）
- [x] C0.6 确认现状事实：`append_tool_results` 是唯一回写 `last_row_id` 的函数，`ensure_row` / `persist_row_as_new` 不回写（规格 §2.1 / 根因 R1）
- [x] C0.7 确认测试命令与工作目录：`cwd = src-tauri`，见 C7

---

## C1 链式追加（D1 + D5 + 调用点，方案 A 核心）

实现：`src-tauri/crates/assistant-agent/src/agent/loop_executor/{persist,terminal,mod}.rs`

- [x] C1.1 `append_tool_results` 删除 `parent_id: String` 形参，父节点在函数内部逐条取 `state.last_row_id`（2026-11-08 完成）
- [x] C1.2 每条结果写入成功后回写 `state.last_row_id = row_id`（保持现有行为）
- [x] C1.3 失败分支删除 `state.last_row_id = parent_id.clone()` 回退逻辑，改为保留上一次成功值（已写入的行仍在链上）
- [x] C1.4 `ensure_row` 建行成功后追加 `state.last_row_id = msg_id`（仅在 `create_assistant_message` 返回分支内，早退分支不改）
- [x] C1.5 `persist_row_as_new` 建行成功后追加 `state.last_row_id = msg_id`（须在调用 `persist_row` 之前，避免被 `ensure_row` 早退吞掉）
- [x] C1.6 正常迭代调用点（`run_iterations` 内 `append_tool_results` 调用处）改为不传 `parent_id`，并清理随之失效的 `msg_id` 局部绑定
- [x] C1.7 `cancelled_tool_exec` 调用点同上；确认取消出口终态文本仍落在本迭代 assistant 行（INV5 取消分支）
- [x] C1.8 `wrap_up_on_last_iteration` 调用点同上；确认 `skipped` 行（`not_executed` 产出）自动获得链式追加
- [x] C1.9 回归确认：内存侧 `state.request.chat_messages.push(...)` 逻辑未改（投影按线性顺序匹配，不依赖 `parent_id`）
- [x] C1.10 回归确认：代际守卫（`is_current` 为假时跳过落库）行为未改，旧代际仍不写行、不发事件
- [x] C1.11 `cargo clippy -p assistant-agent --all-targets -- -D warnings` 无未使用变量 / 未使用形参告警

> C1.4 / C1.5 是 C1.1 的**前置条件**，必须同批交付：只改 C1.1 会在下一次「有工具调用」的迭代断裂（规格 §5.2）。

---

## C2 分支深度上限（D2，与 C1 同批）

实现：`src-tauri/crates/assistant-storage/src/conversations/branch.rs`

- [x] C2.1 `MAX_BRANCH_DEPTH` 由 1000 放宽至防环冗余量级（建议 10000），或改为「超阈值只告警不失败」；二选一在实现时定稿并写入注释（取前者：10000，注释说明环检测为主防线、深度上限仅作防环冗余）
- [x] C2.2 保留环检测：`trace_active_branch` 与 `create_branch` 的 `visited` 判定不删
- [x] C2.3 确认超限错误的唯一后果路径（`get_conversation_detail` 失败）在放宽后不再被长会话触发；错误文案保持不变
- [x] C2.4 确认该上限在现状下对单工具批次已偏紧（约 165–250 轮即整会话打不开），因此不得把 D2 推迟到后续批次

---

## C3 单测补强与替身改造

实现：`src-tauri/crates/assistant-agent/tests/loop_executor.rs`、`src-tauri/crates/assistant-agent/src/kernel/ports/test_doubles.rs`

- [x] C3.1 新增 A20a：`multi_tool_script` 3 个调用（第 1 迭代）+ 文本（第 2 迭代）；断言 `assert_single_chain`、assistant 行数为 2、仅占位行挂在 `U1` 下、`final_message_id` = 最后写入行（2026-11-08 完成）
- [x] C3.2 新增 A20b：`with_max_tool_calls(1)` + 同批 3 个调用（含 `skipped`）；断言单链、`skipped` 行同样链式、`is_error` 行数与 `skipped` 数一致
- [x] C3.3 新增 A20c：四类终态出口各 1 例（预算耗尽 / 无进展 halt / 收尾调用 / 工具执行期取消）；四者共同断言叶子 = 最后一条写入行、`final_message_id` 行可从叶子回溯；预算耗尽 / 无进展 / 收尾调用的终态行即叶子（父 = 最后一条 tool 行），工具执行期取消的终态行为本迭代 assistant 行、早于叶子
- [x] C3.4 A14 现有 3 调用用例（`a14_batch_runs_once_and_keeps_port_order`）追加 `assert_single_chain`，锁住「执行顺序不影响写入链」
- [x] C3.5 新增跨迭代断言：第 2 迭代首个 tool 行父 = 第 2 迭代 assistant 行（直接覆盖 C1.4 / C1.5 的前置条件，缺此项则 D5 遗漏不会被发现）
- [x] C3.6 `InMemoryStore` 增加 `active_leaf` 状态：`create_assistant_message` / `append_tool_message` 内按 `parent == leaf` 推进，并提供 `active_leaf()` 读口
- [x] C3.7 在 A20a / A20c 中改用 `active_leaf()` 断言「叶子 = 终态行」，使单测层即可发现叶子停滞，不必下沉到存储层
- [x] C3.8 确认替身改动不改变 `StoreCall` 记录序列，现有调用序断言（`call_labels`）全部保持通过

---

## C4 存储层用例

实现：`src-tauri/crates/assistant-storage/tests/conversations.rs`

- [x] C4.1 新增用例：按 executor 真实产生的 parent 序列（`U1 → A1 → T1 → T2 → T3 → A2`）调用 `insert_message` + `advance_active_branch`；断言 `active_branch_id == A2`、`get_conversation_detail` 返回全部 6 行、`U1` 的兄弟数为 1（无假分支）（2026-11-08：`multi_tool_batch_chain_advances_leaf_to_terminal_row`）
- [x] C4.2 新增 A20e：构造超过放宽前上限（1000）的合法链，`get_conversation_detail` 仍成功（与 C2 同批验收）（2026-11-08：`long_single_chain_detail_still_opens`，1200 行）
- [x] C4.3 保留 `advance_active_branch_keeps_user_branch_selection` 与 `advance_active_branch_tracks_multi_iteration_chain` 原样通过，作为「判定未被放宽」的防回归锚点

---

## C5 文档口径修订（与 C1–C4 同批提交）

实现：`docs/store/` 下 `I38b` 系列

- [x] C5.1 S1：`I38b-agent-loop-spec.md` §6.2 结果行 `parent_id` 表述改为「指向**上一条写入行**（首条 tool 行指向本迭代 assistant 行）」，并注明链式追加的叶子推进理由（2026-11-08 完成）
- [x] C5.2 S2：`I38b-agent-loop-impl-guide.md` 三处 `store.append_tool_message(parent = msg_id)` 改为 `parent = last_row_id`、逐条推进；同步删除 `append_tool_results` 的 `parent_id` 形参表述；两处 `ensure_row` 代码样例补 `last_row_id` 回写（2026-11-08 完成）
- [x] C5.3 S4：`I38b-agent-loop-spec.md` 风险表「结果行挂 assistant 行」改为「链式挂在前一行」，「不产生新分支」结论保留（2026-11-08 完成）
- [x] C5.4 S5：`I38b-agent-loop-tasklist.md` C7.14 备注补「端到端覆盖下沉至本规格 §7.2 / §7.3」，跳过决定不变（2026-11-08 完成）
- [x] C5.5 S6：`I38b-agent-loop-tasklist.md` 核对缺陷 1 补记「修复不完整：父节点已修、叶子未修，遗留 D5 维护不对称」（2026-11-08 完成）
- [x] C5.6 本规格 §10 关系表补入本清单条目（已随清单建立完成）

> C5.1 / C5.2 / C5.3 必须与 C1 同批，否则规格与实现再次分叉。

---

## C6 后续独立批次（不计入本批完成度）

- [x] C6.1 D4：`create_assistant_message` / `append_tool_message` 的 insert → status → 叶子推进收进同一事务（`unchecked_transaction`，与 `create_branch` 现有写法一致）（2026-11-08：`src-tauri/src/adapters/message_store.rs` 两条路径均改为 `let tx = conn.unchecked_transaction()` → 各语句传 `&tx` → `tx.commit()`；`cargo test -p assistant-storage` 35 项与 `assistant-agent` 全量通过）
- [ ] C6.2 D3：`trace_active_branch` 的逐行 N+1 查询改为 recursive CTE 或按 id 批量取（**不做**：实测链长量级下无可感知延迟，留待观测后再启动）
- [x] C6.3 D6：tool 行 `sibling_index` 分配（链式追加后同父兄弟消失，若保留同父形态则按同父计数）（2026-11-08：问题已自然消解，无代码改动；每条 tool 行独占一个父节点）
- [x] C6.4 D8：缺失结果投影不再等同 `Cancelled`；`flush_pending` 至少 `log::warn!` 并带 `conversation_id` / `call_id` / `message_id`（2026-11-08：`MessageToolCallStatusDto` 新增 `Missing` 并跑 `npm run contract:gen` 重生成；`conversation_utils.rs` 缺失结果映射为 `missing`；`prompt_builder.rs` 的 `flush_pending` 与「结果缺父调用」两处升为 `warn` 并带定位三要素；前端 `ToolCallStatus` 扩为六态，`ToolCallBubble` 新增图标/配色/文案，`zh-CN` / `en-US` 补 `missing` 文案；`npm run typecheck`、`npm run lint`、`vitest run`（33 文件）全通过）
- [ ] C6.5 方案 B：`MessageStore` 端口收口为「追加到分支尾部」原子操作，`parent_id` 从端口 API 消失；须同步 `test_doubles.rs` 与全部调用点，独立提交（**不做**：C6.1 已覆盖用户可见收益；若要补编译期保险，先只把 `msg_id` / `last_row_id` 收成不同 newtype）
- [ ] C6.6 A20f：方案 B 批次补「`append_tool_message` 内部任一语句失败不留半写状态」用例（随 C6.5 一并取消）

---

## C7 验收执行

`cwd = src-tauri`，逐条跑通并记录结果：

- [x] C7.1 `cargo test -p assistant-agent`（A20a–A20d、A14、A1、A22 全绿）（2026-11-08：`--features test-doubles` 全量 16 个测试目标均 ok，`loop_executor` 34 项通过）
- [x] C7.2 `cargo test -p assistant-storage`（C4.1–C4.3、A20e 全绿）（2026-11-08：全部目标 ok，`conversations` 35 项通过）
- [x] C7.3 `cargo clippy -p assistant-agent -p assistant-storage --all-targets -- -D warnings`（2026-11-08：两包均无 lint 告警，仅有 `target/incremental` 目录清理的 os error 5 提示，不影响结论）
- [x] C7.4 `cargo fmt`（先直接格式化，再 `cargo fmt --check`）（2026-11-08：格式化后 `--check` 无 diff）
- [x] C7.5 手工端到端（A20d）：多工具 turn 结束后 `ChatMessagesChangedPayload.messages` 含终态 assistant 行；下一轮新 user 行父 = 终态 assistant 行（2026-11-08：人工确认通过——多工具 turn 的工具卡片与终态回答同时可见，追问一轮上下文连续）
- [x] C7.6 规格 §3 INV1–INV6 逐条对照验收结果确认，无条款被绕过（2026-11-08：INV1 由 A20a 单链断言、INV2 由 A20a / A20c 叶子断言、INV3 由 A20b skipped 链、INV4 由 A20a 跨迭代用例（`a20a_cross_iteration_chains_onto_last_tool_row`）、INV5 由 A20c 四类终态出口、INV6 由 `conversations.rs` 真实序列用例覆盖）

**敏感性验证（2026-11-08）**：临时把 `append_tool_results` 父节点改回 `state.msg_id`，`loop_executor` 立即 8 项失败（链断言与叶子断言全部报出），恢复后 34 项全绿——证明新用例不是恒真断言。

**C7 结论**：除 C7.5（需真实 provider + 前端，留人工）外全部通过；C6 为后续独立批次，不在本批完成度内。
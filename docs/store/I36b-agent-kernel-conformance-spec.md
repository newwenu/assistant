# I36b — Agent 内核一致性验证规格（确定性可用）

| 项 | 值 |
|---|---|
| 状态 | 现行（验证规格；定义判定，不改设计） |
| 日期 | 2026-11-05 |
| 关联 | [I36a 内核基线](./I36a-agent-kernel-baseline.md)、[I36 运行时规格](./I36-agent-runtime-spec.md)、[I36 任务清单](./I36-agent-runtime-tasklist.md)、[I38b Agent Loop](./I38b-agent-loop-spec.md)、[I38 阶段总纲](./I38-agent-roadmap.md) |
| 影响范围 | 文档；§4 缺口 H1–H9 在对应阶段落地时才触碰代码 |

> 定位：I36a 回答「内核应该具备哪些能力」，I36b 回答「**怎么判定实现确实符合基线，并且能在任意机器上确定性复现**」。
> 本文不新增能力设计：所有检查点都能回溯到 I36a 的 `K*/G*` 条目或 I36 spec §10 的验收编号（下称「验收 n」）；本文只把「约定遵守」翻译成「可机检」。
> 编号空间独立：`DT*` 确定性要求、`V*` 验证项、`H*` 实检缺口，与 I38b 的 `D*`、I36a 的 `K*/G*` 不共用。

---

## 1 验证对象与判定口径

### 1.1 三个必须同时成立的问题

| 编号 | 问题 | 不成立的后果 |
|---|---|---|
| Q1 符合性 | I36a 的 K1–K7 不变式、G1–G8 门禁，在当前代码上逐条成立吗？ | 行为是错的，但测试仍全绿 |
| Q2 强制性 | 每条基线是「有强制点」（类型约束 / 断言 / 源码扫描）还是「靠约定」？ | 约定没有守卫，后续改动会静默回退 |
| Q3 确定性 | 同一输入 + 同一脚本化外部响应，两次运行的 `TurnEvent` 序列、`StoreCall` 序列、`TurnOutcome` 是否逐项相同？ | 修不动也测不准：测试红绿取决于机器与网络 |

**Q1 ∧ Q2 ∧ Q3 同时成立**，才称「确定性可用」。只有 Q1 是「现在能跑」；加 Q2 是「不会退化」；加 Q3 是「可以安全改」。

### 1.2 验证对象

| 层 | 对象 | 位置 | 现有覆盖 |
|---|---|---|---|
| 入口 | `TurnRunner::run` 主流程 | `src-tauri/crates/assistant-agent/src/kernel/runner.rs` | `tests/kernel_runner.rs`（10 条端到端） |
| 端口 | `EventSink` / `MessageStore` / `TurnInputSource` / `Clock` | `kernel/ports.rs`、`kernel/ports/test_doubles.rs` | `tests/kernel_ports.rs`（2 条）+ lib 单测 |
| 状态机 | 会话状态、代际、Busy 语义 | `kernel/state.rs`、`kernel/tasks.rs` | `tests/kernel_state.rs`（8）+ `tests/kernel_tasks.rs`（10） |
| 事件 | `TurnEvent`、seq 分配、终态事件 | `kernel/ports.rs`、`kernel/emit.rs` | `tests/kernel_stream.rs`（8）+ runner 断言 |
| 落库 | 终态持久化、usage 记录 | `kernel/persist.rs` | `tests/usage_records.rs`（7）+ runner 断言 |
| 参数 | `TurnSpec` / `TurnLimits` / `LadderPolicy` | `kernel/mod.rs`、`kernel/limits.rs` | `tests/kernel_values.rs`（10） |
| 契约 | 事件 → wire 映射 | `src/types/protocol.contract.ts`、`assembly_contract` | `cargo test --test assembly_contract`（cwd = `src-tauri`） |

**实检基线（2026-11-05）**：`cargo test -p assistant-agent --features test-doubles`（cwd = `src-tauri`）→ 11 个测试二进制，**149 passed / 0 failed**（其中 kernel 相关 48 条，含 `kernel_runner` 10 条）。这是 Q1 的当前证据，不是 Q2/Q3 的证据。

### 1.3 不在范围

- P4 循环体行为（工具执行、收尾调用、压缩阶梯）→ I38b；本文只验证「参数已声明」与「参数未被静默忽略」。
- 工具实现正确性 → I38c；前端五态 → I38d。
- 真实 provider 协议兼容性 → 只做 L2 冒烟（能跑通），不做逐字节比对。
- 性能、吞吐、并发上限 → 与确定性无关，不入门禁。

---

## 2 确定性要求 DT1–DT8

| 编号 | 要求 | 违例形态 | 机检方式 |
|---|---|---|---|
| DT1 时间单源 | 内核一切时间读取经 `Clock`；turn 起点、超时判定、TTFT、耗时统计、落库时间戳来自同一时钟 | `Instant::now()` / `chrono::Utc::now()` / `SystemTime::now()` 出现在 `src/kernel/`（`Clock` 实现与 `ports::test_doubles` 除外） | 源码扫描（扩展现有 `kernel_sources_have_no_tauri_types` 的白名单机制） |
| DT2 外部世界单源 | 网络、磁盘、环境变量只经端口进入内核；内核不自行构造 provider 适配器或 HTTP client | `kernel/request.rs` 内 `create_provider_adapter`（已收口，见 H1） | 扫描 `src/kernel/` 无 `create_provider_adapter` / `reqwest` / `std::fs`；`cargo tree -p assistant-agent` 无 `tauri`（G1） |
| DT3 无真实等待 | L1 用例零 `sleep`、零真实 TCP；超时由虚拟时间推进触发 | `tokio::time::sleep(400ms)` 等真实延时（已收口，见 H8） | L1 测试文件 grep `sleep(` / `TcpListener`；`--test-threads=4` 并行墙钟 < 5 s |
| DT4 事件确定性 | 同一输入下 `TurnEvent` 序列（含 `seq` / `iteration` / `phase`）逐项相同；seq 分配与发射同序 | 先 `fetch_add` 后 `dispatch`，两者不在同一临界区（见 H5） | 黄金轨迹两遍比对（§5.2） |
| DT5 落库确定性 | `StoreCall` 序列（Create / Save / Usage / Finalize）与内容逐项可断言；时间戳可注入 | `build_usage_records` 内取 `Utc::now()`（见 H3） | `InMemoryStore::calls()` 断言 + 两次运行序列相同 |
| DT6 终态唯一 | 每次 `run` 恰好 1 次 `Terminated` + 1 次 `Finalize`；代际不符时 0 次 | 某分支提前 `return` 造成 0 次或 2 次 | 对 `kernel_runner` 全部用例统一断言「Terminated 计数 == 1」 |
| DT7 无隐藏状态 | 内核不持有跨 turn 可变全局；顺序敏感处不依赖 `HashMap` 迭代序；锁中毒可恢复 | 用 map 迭代序拼消息；中毒后 panic | grep 顺序敏感处的 `HashMap`；`kernel_state` 中毒恢复用例 |
| DT8 可重复 | 同一测试连续 3 次、`--test-threads=4`、任意顺序，结果一致 | 依赖真实时钟 / 端口 / 外部服务 | CI 连跑 3 遍比对结果行 |

**DT 与 Q 的对应**：DT1–DT3 服务 Q3 的「输入可控」；DT4–DT6 服务 Q3 的「输出可比」；DT7–DT8 服务 Q3 的「可重复」。

---

## 3 一致性矩阵 V1–V30

现状取值：`已通过`（有测试且断言到位）、`部分`（有测试但覆盖面不足）、`未通过`（无强制点或行为不符）、`未接线`（字段/接口已声明但无执行点）。矩阵只做判定，不提出新设计。

### 3.1 隔离与端口

| 编号 | 基线条目 | 检查点 | 判定方式 | 现状 |
|---|---|---|---|---|
| V1 | G1 / K1 | `src/kernel/` 无 `tauri::` / `AppHandle` / `WebviewWindow` | `kernel_ports::kernel_sources_have_no_tauri_types` | 部分（源码扫描有；`cargo tree` 断言缺） |
| V2 | G1 | 内核可只用测试替身跑完整轮，不启动 Tauri | `kernel_runner` 全量 | 已通过（零监听端口，见 H1 / H8 收口记录） |
| V3 | DT2 | 内核不自行构造 provider 适配器 | grep `create_provider_adapter` | 已通过（仅 `adapters/input_source.rs` 构造，内核只消费 `TurnInput.adapter`） |
| V4 | G3 / 验收 3 | `EventSink` 抛错时 turn 不中止、仍落库 | `kernel_runner::sink_failure_does_not_abort_turn` | 部分（不中止已验证；「日志有记录」无断言） |
| V5 | G3 / 验收 4 | `MessageStore` 失败 → `Failed` + `Finalize(Failed)` | `kernel_runner::store_failure_marks_turn_failed_and_finalizes_failed` | 已通过 |

### 3.2 参数化

| 编号 | 基线条目 | 检查点 | 判定方式 | 现状 |
|---|---|---|---|---|
| V6 | K2 / 验收 5 | `TurnSpec::chat()` / `agent()` 字段值符合 I36 spec §0B.2 | `kernel_values::chat_spec_field_values`、`agent_spec_field_values` | 已通过 |
| V7 | K2 | 内核代码无 `is_agent` / `is_chat` 分支 | `kernel_values::kernel_sources_have_no_branch_on_agent_kind` | 已通过 |
| V8 | K2 / Q2 | 差异只通过入参表达，且**每个入参都有读取点** | 逐个字段 grep 使用点 | 未接线（`ladder` / `wrap_up` / `stop_hooks` 无读取点；已在 `kernel_values` 显式登记为 P4 清单，见 H7） |

### 3.3 状态机与代际

| 编号 | 基线条目 | 检查点 | 判定方式 | 现状 |
|---|---|---|---|---|
| V9 | K4 / 验收 7 | `Processing` 下 `Busy` 拒绝新 turn，且不取消旧 turn | `kernel_tasks::reject_returns_busy_and_keeps_old_turn_running` | 已通过 |
| V10 | K4 | `OnBusy::Replace` 取消旧 turn 并递增代际 | `kernel_tasks::replace_cancels_previous_turn_and_bumps_counter` | 已通过 |
| V11 | K4 / 验收 8 | 取消后落库、`status='interrupted'`、状态回 `Idle` | `kernel_runner` 取消用例 + `kernel_state` | 已通过 |
| V12 | K5 / 验收 9 | 旧代际迟到完成：不写库、不改状态、不发终止事件 | `kernel_runner::stale_turn_*` | 已通过 |
| V13 | K5 | 不可恢复错误后拒绝新 turn（`Unrecoverable`） | `kernel_state::unrecoverable_error_blocks_new_turn` | 已通过 |

### 3.4 事件

| 编号 | 基线条目 | 检查点 | 判定方式 | 现状 |
|---|---|---|---|---|
| V14 | 验收 21 | 每条事件带 `turn_id`；同 turn 内 `seq` 严格单调、从 1 起 | `kernel_stream::seq_starts_at_one_and_increases_monotonically` + runner 断言 | 已通过 |
| V15 | K6 | 先 `StreamStart`、后 `Delta`、末 `Terminated`；终态唯一 | runner 首末事件断言 | 部分（仅 1 条用例断言首末；缺「计数 == 1」统一断言，见 DT6） |
| V16 | K6 | 部分结果保留：provider 错误 / 超时 / 取消都保留已累积正文 | `kernel_stream::provider_error_returns_partial_outcome`、`timeout_keeps_accumulated_content`、`cancelled_stream_keeps_accumulated_content` | 已通过 |
| V17 | DT4 / Q3 | `TurnEvent` 可稳定序列化用于轨迹比对 | `kernel_emit::event_trace_is_byte_identical_across_runs` | 已通过（两遍轨迹逐字节相同） |

### 3.5 落库

| 编号 | 基线条目 | 检查点 | 判定方式 | 现状 |
|---|---|---|---|---|
| V18 | K7 | 先落库、后改状态（`Finalize` 先于 `finish_turn`） | runner 断言 `StoreCall` 尾部 + 状态断言 | 已通过 |
| V19 | K7 | 消息创建 → 增量保存 → 终态 finalize 的调用序列固定 | `kernel_runner::chat_turn_completes_end_to_end_with_doubles` 断言 `calls[0..]` | 部分（仅 1 条用例逐项断言；其余用例只查 `contains`） |
| V20 | 验收 23 | usage 记录字段齐全、每次 attempt 各一条 | `usage_records`（7 条） | 已通过 |
| V21 | DT5 | 落库时间戳可注入、可复现 | grep `Utc::now`（`src/kernel/` 应为空） | 已通过（`PersistCtx.created_at_ms` 由 `Clock::now_millis` 注入） |

### 3.6 预算与终止

| 编号 | 基线条目 | 检查点 | 判定方式 | 现状 |
|---|---|---|---|---|
| V22 | K3 / 验收 10 | `turn_wall_clock` 可触发超时 | `kernel_runner::wall_clock_timeout_fails_turn_before_model_call`（`StepClock` 推进） | 已通过（虚拟时钟触发，无 `sleep`） |
| V23 | K3 / 验收 10 | 单次模型调用超时可触发，且**口径与 spec 一致** | `kernel_stream::timeout_caps_whole_call_even_when_chunks_keep_arriving` + 语义核对 | 已通过（整段封顶，见 H6） |
| V24 | K3 | 工具执行不受 `model_call` 限制 | P3 无工具执行点 | 未接线（P4） |
| V25 | 验收 11 | 空响应重试 1 次仍空 → `Failed`，且 `iteration` 不递增 | `kernel_runner::chat_spec_does_not_retry_empty_response` + agent 重试用例 | 已通过 |
| V26 | 验收 18 | 迭代触顶收尾：末次请求无 `tools`、`wrap_up_injected = true` | 无执行点 | 未接线（P4，见 H7） |

### 3.7 声明但未强制（Q2 专项）

| 编号 | 检查点 | 判定方式 | 现状 |
|---|---|---|---|
| V27 | `TurnSpec::limits` 各字段都有读取点 | 逐个字段 grep 使用点 | 部分（`max_iterations` / 两个超时有；`max_tool_calls` 无） |
| V28 | `TurnSpec::ladder` 能改变阶梯行为 | `kernel/runner.rs` 读取点 | 未接线（H7） |
| V29 | `TurnSpec::wrap_up` 能触发收尾 | 同上 | 未接线（H7） |
| V30 | `TurnSpec::stop_hooks` 能在迭代前拦截 | 同上 | 未接线（H7） |

**判定规则（V27–V30）**：字段已声明但无读取点，一律记为「未接线」，**不得**记为「已实现」。这条规则对 I36a 的能力矩阵同样适用——「代码里有这个类型」不等于「机制在生效」。

---

## 4 实检缺口 H1–H9

每条缺口给出：证据（相对路径 + 符号）、现象、判定标准、收口建议、归属阶段。收口建议只是「按既有约定补齐」，不改设计口径；若需要改口径，一律写回 I36 spec 并保留原条目。

### H1 模型调用不可注入（阻断 Q3）

- 证据：`src-tauri/crates/assistant-agent/src/kernel/request.rs` `build()` 内 `base::create_provider_adapter(build_provider_config(provider_row))`。
- 现象：内核路径的 provider 适配器由 `build()` 自行构造，测试只能起真实 TCP 假服务（`tests/kernel_runner.rs` 的 `SseServer`）。provider 失败、重试、分片、慢流全部要靠 HTTP 行为模拟。
- 判定标准：`src/kernel/` 内不再出现 `create_provider_adapter`；L1 用例能注入返回预设 `ProviderStream` 的脚本化适配器，且用例内无监听端口。
- 收口建议：`ChatRequest` 已有 `adapter: Box<dyn BaseProviderAdapter>` 字段，只需把构造点上移——`TurnInput` 携带 `adapter`（由输入端口提供），`build()` 只消费不再构造。
- 归属：P3（P4 开工前必须完成：`RoundExecutor` 的多轮模型响应同样只能靠脚本化适配器构造）。
- 收口记录（2026-11-05）：`TurnInput` 新增 `adapter: Arc<dyn BaseProviderAdapter>`，`kernel/request.rs` 的 `build()` 只取 `input.adapter`，构造点移至 `src/adapters/input_source.rs`；`ports::test_doubles` 新增 `ScriptedAdapter`（按调用顺序返回预设流），`kernel_runner` 11 条 / `kernel_emit` 5 条 L1 用例全部改为脚本化响应，零监听端口，全量 0.27 s 通过。

### H2 双时间源

- 证据：`src-tauri/crates/assistant-agent/src/kernel/ids.rs` `TurnContext::new` 内 `Instant::now()`；`kernel/runner.rs` 的 deadline 判定用 `ctx.clock.now()`。
- 现象：turn 起点来自真实时钟，超时基准来自注入时钟。`elapsed_ms` / TTFT 的零点与 deadline 不同源；测试中两者偏移一旦不同，`elapsed_ms` 就不可复现。
- 判定标准：`src/kernel/` 内无 `Instant::now()`（`Clock` 实现与 `test_doubles` 除外）；`TurnContext` 的起点可由调用方注入。
- 收口建议：`TurnContext::new` 增加 `started_at` 入参，由 runner 用 `clock.now()` 传入。
- 归属：P3。
- 收口记录（2026-11-05）：`TurnContext` 不再持有起点（`started_at` 字段移除），改由 `TurnContext::new` 的 `started_at` 入参传入，调用方 `kernel/runner.rs` 统一取 `ctx.clock.now()`；`src/kernel/` 内已无 `Instant::now()`。

### H3 落库时间戳不可注入

- 证据：`src-tauri/crates/assistant-agent/src/kernel/persist.rs` `build_usage_records` 内 `chrono::Utc::now()`。
- 现象：usage 记录带真实时间戳，黄金轨迹无法逐字节比对（DT5）。
- 判定标准：`persist` 内无 `Utc::now()`；时间戳来自入参。
- 收口建议：`PersistCtx` 增加 `created_at_ms: i64`，由 runner 从 `Clock` 取（`Clock` 需补一个 wall-clock 方法或直接注入 `now_millis`）。
- 归属：P3。
- 收口记录（2026-11-05）：`Clock` 端口补齐 `now_millis()`（Unix 毫秒，与 `now()` 同源）；`PersistCtx` 携带 `created_at_ms`，usage 记录时间戳全部来自该入参，`src/kernel/` 内已无 `Utc::now()`。

### H4 测试时钟无推进能力

- 证据：`src-tauri/crates/assistant-agent/src/kernel/ports/test_doubles.rs` 的 `FixedClock`；`tests/kernel_runner.rs` 的墙钟用例用 `Instant::now() + Duration` 构造偏移 deadline。
- 现象：超时用例依赖真实时间偏移（含 120 ms 级窗口），慢机器或时钟回拨会改变结果，不满足 DT3 / DT8。
- 判定标准：超时用例通过推进虚拟时钟触发，用例内无 `Instant::now()`、无 `sleep`。
- 收口建议：`ManualClock { base, offset }` + `advance(d)`；配合 `tokio::time::pause()`（`dev-dependencies` 已启用 `test-util`）。
- 归属：P3。
- 收口记录（2026-11-05）：`ports::test_doubles` 新增 `ManualClock`（显式 `advance`）与 `StepClock`（每次读取按固定步长前进）；`wall_clock_timeout_fails_turn_before_model_call` 改用 `StepClock` 自然越预算，`model_call_timeout_fails_turn_after_call_started` 与 `kernel_stream::cancelled_stream_keeps_accumulated_content` 改用 `#[tokio::test(start_paused = true)]` 虚拟时间，用例内无真实等待。

### H5 事件序号两段式

- 证据：`src-tauri/crates/assistant-agent/src/kernel/emit.rs` `delta()`：先 `fetch_add` 分配 seq，再 `dispatch()` 发射，两者不在同一临界区。
- 现象：当前单写者（流式回调串行）所以全绿；P4 引入工具事件 / 并行子代理后，可能出现「seq 小的后到」，破坏 DT4。
- 判定标准：seq 分配与发射在同一临界区，或有单写者 `debug_assert` 守卫并在 P4 换成锁内分配。
- 收口建议：P3 定策（写进 I36 spec §3.x 的事件契约），P4 落地。
- 归属：P3 定策 / P4 实现。

### H6 单次调用超时口径不一致

- 证据：`src-tauri/crates/assistant-agent/src/kernel/stream.rs` `consume` 对每次 `stream.next()` 施加 `model_call` 超时；I36 spec §3.2 与验收 10 表述为「单次模型调用超时（`model_call` 或 turn 剩余，取小）」。
- 现象：实现是**空闲超时**语义——只要持续有 chunk 到达，单次调用可以无限超过 `model_call`；与 spec 字面不符，也偏离「预算必须封顶」的意图。
- 判定标准：`consume` 的整段耗时受 `model_call` 封顶，或 spec 明确改为「chunk 间隔超时 + turn 墙钟」并同步改验收 10 的措辞。
- 收口建议：优先「整段包一层 `timeout(model_call_budget)`」，保留 chunk 级超时作为二级（两者叠加，先到先触发）；若选改口径，必须回写 I36 spec 并保留旧条目作历史。
- 归属：P3（先定口径，再补用例）。
- 收口记录（2026-11-05）：按「两者叠加」落地——`consume` 在调用入口取 `call_started = clock.now()`，每次等 chunk 的超时额度 = `model_call` 减去已耗时（剩余为 0 即判超时），chunk 级超时降为同一额度的实现手段；I36 spec §3.2 措辞保持不变（本就是「整段封顶」）。新增用例 `timeout_caps_whole_call_even_when_chunks_keep_arriving`：流每 60 ms 稳定出 chunk、预算 100 ms，断言超时且保留已到达 chunk（旧实现会一直跑完，必红）。

### H7 声明但未强制的字段

- 证据：`TurnSpec` 的 `ladder` / `wrap_up` / `stop_hooks` 在 `kernel/runner.rs` 内无读取点（`wrap_up`、`stop_hooks` 仅出现在 `kernel/mod.rs` 的构造处）。
- 现象：`TurnSpec::agent()` 声明 `wrap_up = true`，但 P3 循环不执行收尾（验收 18 不可达）；`LadderPolicy::Full` 与 `ProjectionOnly` 在 runner 内行为相同。
- 判定标准：要么有读取点与行为，要么在 `kernel_values` 里显式维护「P3 未接线字段清单」断言，防止误判为已实现。
- 收口建议：P3 增加该清单断言并在 I36 spec 标注「P4 接线」；P4 接线后删除断言。
- 归属：P3（口径）/ P4（实现）。
- 收口记录（2026-11-05）：`tests/kernel_values.rs` 新增 `unwired_turn_spec_fields_have_no_read_points_yet`，扫描 `src/kernel/**/*.rs` 断言不存在 `spec.ladder` / `spec.wrap_up` / `spec.stop_hooks` 读取点；一旦 P4 接线，该用例失败并强制同步清单（接线时删除本断言）。原有 `kernel_values` 的取值断言保留，作为「默认值口径」证明。

### H8 L1 依赖真实网络与真实等待

- 证据：`src-tauri/crates/assistant-agent/tests/kernel_runner.rs` 的 `SseServer::spawn` 与 `tokio::time::sleep`（400 ms / 120 ms 级）。
- 现象：L1 用例占用端口、受 CI 抖动影响；`kernel_runner` 耗时 0.37 s，是全套中最慢的一档。
- 判定标准：L1 无监听端口、无 `sleep`；真实 TCP 只保留 1 条冒烟用例并置于 `slow-tests` 特性（`Cargo.toml` 已有该特性）。
- 收口建议：H1 落地后改为脚本化适配器；TCP 用例降级。
- 归属：P3。
- 收口记录（2026-11-05）：`kernel_runner` / `kernel_emit` 内的 `SseServer` 全部移除，改由 `ScriptedAdapter` 给出预设流；两个文件 grep 无 `TcpListener` / `127.0.0.1` / `sleep(`。`kernel_tasks::cancelled_token_completes_select_branch` 的哨兵分支由 `sleep(50ms)` 改为 `std::future::pending::<()>()`，测试文件内已无任何 `sleep(`。真实 TCP 冒烟用例待 P4 接线后按 `slow-tests` 特性补入。

### H9 事件无稳定序列化

- 证据：`src-tauri/crates/assistant-agent/src/kernel/ports.rs` `TurnEvent` 仅 `#[derive(Debug, Clone)]`；`TurnPhase` / `TurnStatus` / `SessionState` 已可序列化并有 camelCase 用例。
- 现象：黄金轨迹只能比 `{:?}`，字段顺序与新增字段都会造成假红/假绿。
- 判定标准：`TurnEvent` 可稳定序列化为 JSON（字段白名单、无时间戳、无地址），或提供 `TurnTrace::from_events` 投影。
- 收口建议：给 `TurnEvent` 加 `Serialize`（camelCase，与 wire 一致），轨迹只序列化白名单字段。
- 归属：P3。
- 收口记录（2026-11-05）：`TurnEvent` 派生 `Serialize`（camelCase），**仅用于测试轨迹比对，不进入 wire / DTO 契约**；`kernel_emit::event_trace_is_byte_identical_across_runs` 断言同一输入两遍运行的事件 JSON 行逐字节相同。

---

## 5 验证方法

### 5.1 三层分层

| 层 | 内容 | 允许的依赖 | 用例位置 | 运行时机 |
|---|---|---|---|---|
| L0 | 值类型、纯函数、状态机迁移 | 无 IO、无 async | `kernel_values` / `kernel_state` / `kernel_tasks` / `usage_records` | 每次提交 |
| L1 | 端口替身 + 虚拟时钟 + 脚本化适配器 | 仅内存与虚拟时间 | `kernel_runner`（改造后） | 每次提交 |
| L2 | 真实 provider / 真实 Tauri | 网络、窗口 | `slow-tests` 特性下 1–2 条 | 手动 / 发版前 |

分层判据：**能用 L1 表达的用例不得放进 L2**。L2 只回答「接线没断」，不回答行为正确性。

### 5.2 黄金轨迹（golden trace）

轨迹由三部分组成，全部从端口替身读取，不含真实时间：

```
TurnTrace {
  events: Vec<String>,        // TurnEvent 白名单投影，逐条 JSON 行
  store_calls: Vec<String>,   // Create / Save / Usage / Finalize 及关键字段
  outcome: String,            // status / iterations / model_calls / tool_calls / hit_cap / wrap_up_injected
}
```

规则：

1. 同一输入跑两遍，`TurnTrace` 逐字节相同（DT4 / DT5 / DT8）。
2. 轨迹禁止包含：真实时间戳、端口号、内存地址、`turn_id` 以外的随机量（`turn_id` 用固定夹具值）。
3. 新增事件类型必须同时更新轨迹投影的白名单，否则用例红——这是**故意**的：新增事件必须被显式确认。
4. 轨迹快照随用例放在测试目录内，与代码同批评审；不写进 `docs/`。

### 5.3 反例注入矩阵

每个端口至少覆盖一种失败模式，且失败模式必须产生**可断言的终态**：

| 端口 | 失败模式 | 期望 |
|---|---|---|
| `EventSink` | 第 N 条事件后抛错 | turn 不中止，终态正常，失败次数被记录（当前 `RecordingSink` 静默丢弃，需补计数断言，见 V4） |
| `MessageStore` | 任一写调用失败 | turn `Failed` + `Finalize(Failed)`（V5） |
| `TurnInputSource` | 未知 id / 缺 provider | 早失败、不落库、终态 `Failed`（`FixtureSource` 已有） |
| `Clock` | 时钟回拨 / 不推进 | 不 panic；不推进时超时由外部推进触发，回拨时 deadline 判定不出现负值 |
| 适配器（H1 已收口） | 空响应 / 推理无正文 / 流中断 / 慢流 | 对应验收 11 与部分结果保留（V16 / V25） |

### 5.4 虚拟时间规则

- L1 用例一律 `tokio::time::pause()`，需要时间流逝时用 `advance` / `ManualClock::advance`。
- 禁止 `sleep` 等待被测逻辑「自然结束」；若必须等待，先断言「应有事件已发出」，再 `yield_now`。
- 超时类用例必须能证明「是哪个超时先到」：墙钟用例断言 `Failed` 且错误文案含 `turn wall clock`，单次用例断言文案含 `model call`（H6 定口径后同步改）。

### 5.5 命令

在 `src-tauri` 目录下执行（cargo 必须在 manifest 目录内运行）：

```
cargo test -p assistant-agent --features test-doubles
cargo test -p assistant-agent --features test-doubles --test kernel_runner
cargo test -p assistant-agent --features test-doubles --test kernel_values
cargo test --test assembly_contract
```

连跑三遍验证 DT8（同一命令重复执行，结果行必须一致）。

---

## 6 退出准则

### 6.1 P3 收口（Q1 + Q2）

1. §3 矩阵中「已通过」之外的条目，要么收敛为「已通过」，要么在 I36 spec 明确标注「P4 接线」并同时存在 V27–V30 的未接线断言。
2. H2 / H3 / H4 / H9 收敛（时间单源、时间戳可注入、时钟可推进、事件可序列化）。——**已完成（2026-11-05）**
3. H6 口径定稿并写回 I36 spec。——待办
4. H7 的未接线清单断言存在且为绿。——待办

### 6.2 P4 开工前置（Q3 阻断项）

H1 与 H8 **必须**先收敛：没有脚本化适配器，`RoundExecutor` 的多轮模型响应无法在 L1 构造，P4 的测试只能继续依赖真实 TCP，确定性从第一行代码起就不可得。

**状态（2026-11-05）：已收敛。** `TurnInput` 携带适配器、`ScriptedAdapter` 可按调用顺序给出多轮流，L1 全量用例零监听端口、零真实等待，`RoundExecutor` 可在 L1 直接构造多轮响应。

### 6.3 判定表

| 条件 | 结论 |
|---|---|
| Q1 全绿、Q2 有未接线断言、Q3 未达成 | 「现在能跑，不能安全改」→ 只允许修 bug，不允许改结构 |
| Q1 全绿、Q2 全绿、Q3 两遍一致 | 「确定性可用」→ 允许 P4 并行开工 |
| Q1 有红 | 停止新功能，先修行为 |

### 6.4 回归门禁

每次提交必须全绿：`cargo test -p assistant-agent --features test-doubles`、`cargo test --test assembly_contract`、`cargo fmt --check`、`cargo clippy`（内核改动时）。任一门禁红，不接受「下一条提交修」。

---

## 7 文档关系与维护

| 文档 | 关系 |
|---|---|
| [I36a 内核基线](./I36a-agent-kernel-baseline.md) | 本文的输入：能力矩阵与 K/G 条目；本文只判定，不改其条目 |
| [I36 运行时规格](./I36-agent-runtime-spec.md) | 契约与验收编号来源；H6 / H7 需要改口径时必须回写该文档 |
| [I36 任务清单](./I36-agent-runtime-tasklist.md) | H1–H9 的落地任务为 C11.1–C11.8（P3 收口 + P4 前置） |
| [I38b Agent Loop](./I38b-agent-loop-spec.md) | P4 循环体规格；本文的 V24 / V26 / H7 在 P4 转「已通过」 |

维护规则：

1. 矩阵只增不改编号；条目失效时改「现状」列并注明日期，不删除行。
2. 缺口收敛后，在 H 条末追加「收口记录（日期 + 提交/测试名）」，保留原文。
3. 新增内核能力时，先补 DT / V 条目，再实现；顺序倒置视为规格缺口。
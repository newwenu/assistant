# Agent 前四阶段代码规模与职责分布

> **快照日期**：2026-11-06
> **统计口径**：Rust 源文件**非空行数**（含注释、文档注释与文件内联测试；空行不计）。命令见 §6，`Measure-Object -Line` 会跳过空行，故与编辑器显示的总行数略有差异（如 `test_doubles.rs` 736 总行 / 711 非空行）。不含前端 TS、不含 `src/types/protocol.contract.ts` 等生成产物。
> **性质**：现状快照，**不是进度台账**。阶段进度唯一来源是 [I38 roadmap](./I38-agent-roadmap.md) §2 与各阶段 tasklist（[I36 tasklist](./I36-agent-runtime-tasklist.md)、[I38b tasklist](./I38b-agent-loop-tasklist.md)）。
> **时效**：P4 仍在进行（C4.24–C4.38、C5–C9 未完成），本表数字会随后续任务变化；重测方式见 §6。

---

## 1 总览

### 1.1 按目录

| 目录 | 阶段归属 | 源文件 | 源行数 | 测试文件 | 测试行数 | 用例数 |
|---|---|---|---|---|---|---|
| `src-tauri/crates/assistant-tools/src` | P0–P2 | 15 | 3397 | 4 | 1534 | 70 |
| `src-tauri/crates/assistant-agent/src/kernel` | P3 | 14 | 2685 | 10 | 2160 | 84 |
| `src-tauri/crates/assistant-agent/src/agent` | P4 | 8 | 2691 | 4 | 859 | 45 |
| `src-tauri/crates/assistant-agent/src`（顶层遗留） | I2x / I15 遗留，P3 有改动 | 11 | 1729 | 4 | 1112 | 54 |
| `src-tauri/src/adapters` | P3 605 + P4 434 | 11 | 1039 | — | — | — |
| `src-tauri/src/protocol/dto/agent.rs` | P4 | 1 | 70 | — | — | — |
| `src-tauri/tests`（集成/契约） | 跨阶段 | — | — | 6 | 1938 | 63 |

`assistant-agent` crate 合计：**33 个源文件 / 7105 行**；测试 **18 个文件 / 4131 行 / 183 例**。

> 拆分规划 §1.1 中 `assistant-agent` 一行（12 文件 / 1476 行）是 2026-11-01 的 P3/P4 之前快照，已被本表取代；拆分规划 §3.6 里部分 `assistant-agent` 条目（如 `chat_runtime.rs`）在 P3 重构后已不存在。

### 1.2 按阶段（新增代码，不含遗留）

| 阶段 | 新增/主要改动模块 | 源行数 | 测试行数 |
|---|---|---|---|
| P0 工具词汇层与执行边界 | `assistant-tools` 的 `trait` / `registry` / `result` / `context` / `os/fs_link` / `os/process_output` / `definition` / `exec` | 1590 | 含内联 |
| P1 框架收口 | `os/path.rs` 共享路径安全 | 567 | 含内联 |
| P2 工具控制层 | `policy` / `gateway` / `pipeline` / `storage` | 1240 | 1534（70 例） |
| P3 运行时内核 | `kernel/` 全部 + `adapters` 七端口实现 | 3290 | 2750（117 例） |
| P4 Agent Loop | `agent/` 全部 + `adapters` 四个新实现 + `dto/agent.rs` | 3195 | 859（45 例） |

> P0–P4 的 Rust 主体约 **9800 行源码 + 5100 行测试**；另有 `assistant-providers`（tools 注入、三方言）、`assistant-storage`（`messages.tool` 列）的**改动级增量**（无新文件，各数十行），未单列。

---

## 2 按阶段职责归因

### 2.1 P0 — 工具词汇层与统一执行边界（1590 行）

| 文件 | 行数 | 职责 |
|---|---|---|
| `os/fs_link.rs` | 766 | 唯一生产工具；含内联回归测试 |
| `registry.rs` | 235 | `<=` 语义与受众过滤（`ToolAudience` 默认 `NONE`，fail-closed） |
| `exec.rs` | 170 | 统一执行边界：`Err` 归一化 + 30s 超时 |
| `trait.rs` | 145 | `Tool` trait：`definition` / `validate_input` / `invoke` / `audience` |
| `result.rs` | 129 | `ToolResult` 双通道（给模型 / 给 UI） |
| `os/process_output.rs` | 73 | 子进程输出归一 |
| `definition.rs` / `context.rs` / `lib.rs` / `os/mod.rs` | 22 / 20 / 21 / 9 | 词汇类型与模块出口 |

### 2.2 P1 — 共享路径安全（567 行）

`os/path.rs`：路径规范化、越界判定、权限词汇统一（`edit`），是 P2 `validate_input` 与 P5 全部 `agent_*` 的唯一路径判定来源。

### 2.3 P2 — 工具控制层（1240 行 + 1534 行测试）

| 文件 | 行数 | 职责 |
|---|---|---|
| `gateway.rs` | 561 | 审批决策 + 权限审计写入 + `ApprovalPrompter` 契约 |
| `pipeline.rs` | 355 | 唯一工具执行入口：校验 → 策略 → 审批 → 并发分批 → 结果落盘 |
| `policy.rs` | 203 | 三档权限解析（`ResolvedPermissionPolicy`）与工具级覆盖 |
| `storage.rs` | 121 | 结果持久化端口（大结果落盘 + 引用回填） |

测试集中在 `assistant-tools/tests`：`gateway.rs` 649、`pipeline.rs` 518、`policy.rs` 296、`storage.rs` 71。

### 2.4 P3 — 运行时内核（2685 行 + 2160 行测试 + 605 行适配器）

| 文件 | 行数 | 职责 |
|---|---|---|
| `kernel/ports/test_doubles.rs` | 711 | 端口替身（`cfg(any(test, feature = "test-doubles"))`，**不进生产二进制**） |
| `kernel/runner.rs` | 376 | `TurnRunner`：turn 生命周期编排 |
| `kernel/stream.rs` | 299 | 流式消费、双层超时、TTFT |
| `kernel/ports.rs` | 224 | 四端口 + 停止钩子 + 事件契约 |
| `kernel/request.rs` | 191 | 请求构建与消息投影入口 |
| `kernel/state.rs` | 181 | 状态机与会话状态表 |
| `kernel/persist.rs` | 152 | 终态落库口径 |
| `kernel/tasks.rs` / `emit.rs` / `mod.rs` / `outcome.rs` / `limits.rs` / `ids.rs` / `phase.rs` | 114 / 112 / 113 / 101 / 59 / 31 / 21 | 取消树、事件 seq、值类型 |

适配器（P3 部分 605 行）：`event_sink` 243、`message_store` 126、`input_source` 94、`compaction_sink` 62、`runner` 41、`mod` 23、`system_clock` 16。

### 2.5 P4 — Agent Loop（2691 行 + 859 行测试 + 434 行适配器 + 70 行 DTO）

| 文件 | 行数 | 职责 |
|---|---|---|
| `agent/loop_executor.rs` | 1403 | 多迭代循环体（详见 §3.1） |
| `agent/middleware.rs` | 517 | 7 个中间件 + 固定顺序 + 失败分类 |
| `agent/tool_call.rs` | 321 | `ToolCallAccumulator` 增量累积与截断 JSON 修复 |
| `agent/round_executor.rs` | 168 | `RoundExecutor` trait + `RoundOutcome` 值类型 |
| `agent/ladder.rs` | 160 | 阶梯 ③④⑤：微压缩 / 收尾还原 / 硬裁剪 |
| `agent/stop.rs` | 87 | 三个恒装停止钩子（迭代 / 工具数 / 墙钟） |
| `agent/tool_event.rs` | 28 | 工具事件投影 |
| `agent/mod.rs` | 7 | 模块出口 |

适配器（P4 部分 434 行）：`tool_exec` 177（`PipelineToolExecution`，`ToolExecutionPort` 适配）、`approval` 116（`DenyAllApprovalPrompter` + `TimingApprovalPrompter`）、`permission_store` 108、`tool_catalog` 33。

---

## 3 P4 职责分支

### 3.1 `loop_executor.rs` 的分段

| 区段 | 行范围（约） | 行数 | 职责 |
|---|---|---|---|
| 头部与导入 | 1–38 | 38 | — |
| `RoundState` + `brake` | 39–180 | 142 | 单 turn 可变状态、预算刹车判定 |
| 纯函数工具族 | 181–386 | 206 | usage 合并、nudge 构造、结果构造、预算切分、指纹、模型调用预算、可取消包装 |
| `run()` 主循环 | 397–911 | **514** | 见 §3.2 |
| 终态收尾族 | 913–1184 | 272 | `stream_failure` / `cancelled_before_call` / `cancelled_tool_exec` / `stopped` / `halted_with_summary` / `halted_by_budget` |
| 无工具收尾调用 | 1185–1281 | 97 | `call_model_without_tools`（收尾一轮不带工具） |
| 持久化族 | 1282–1430 | 149 | `ensure_row` / `persist_row` / `persist_row_as_new` / `persist_final_text` / `append_tool_results` |

### 3.2 `run()` 的阶段分支（514 行）

| 编号 | 阶段 | 职责 |
|---|---|---|
| 0 | 请求投影 | 只投影一次历史；读库同属长 await，取消优先 |
| 1 | 迭代开头 | 计算 elapsed → `brake()` → 钩子判定 → 预算终止 |
| 2 | `before_model` | 阶梯与中间件只改本次请求的消息副本；可 `StopTurn` |
| 2.5 | nudge 注入 | 只进本次请求，不落库（D19） |
| 2.6 | 请求快照 | `tools` 已在第 0 步注入，此处只 clone |
| 3 | 单次模型调用 | 内层 loop 处理空响应重试与 provider 失败重试（不递增 iteration，D22） |
| 4 | 输出判定 | 纯工具调用是有效输出；输出上限截断不重发 |
| 4.1 | 重试用尽仍空 | → `Failed`（首迭代复用占位行） |
| 4.5 | 落库本次调用 | 建行 → 正文/推理 → usage → `tool_calls` → `finalize` |
| 5 | 无工具 | → `Completed` |
| 6 | 最后一轮 + `wrap_up` | 未执行调用写「已达迭代上限」结果行 → 收尾调用 |
| 6.5 | 预算内裁剪 | 超额/超时调用不执行但仍写配对结果行 |
| 7 | 工具执行 | 经 `ToolExecutionPort` → `after_tool` 中间件；无端口时全部标「未执行」 |
| 8 | 迭代后 | nudge 进下一轮；`after_iteration` 可 `StopTurn` → 带摘要终止 |

**设计要点**：`run()` 是「线性阶段流水线 + 早返回」结构，每个阶段的失败都收敛到 §3.1 的终态收尾族，因此虽然单函数长，但**没有嵌套分支地狱**——`match` 只出现在模型调用结果与中间件结果两处。

### 3.3 中间件链（7 个，固定顺序）

`projection` → `microcompact` → `wrap-up-restore` → `hard-trim` → `tool-output-cap` → `tool-outcome-capture` → `repeated-failure`

- 阶梯 ③④⑤ 由前四个承载（`ladder.rs`），② 摘要压缩不随 P4。
- `repeated-failure`：同参连续失败 2 次注入 nudge、3 次 halt（`EXEMPT_TOOLS` 预留 P8 轮询类工具）。
- `tool-output-cap` 的 50 000 字符是 P2 `per_tool_limit_chars` 之外的运行时兜底。

### 3.4 停止钩子（`stop.rs`，三个恒装）

`iteration-limit` / `tool-call-limit` / `turn-wall-clock`；工具调用数按**发起数**计——被拒绝、失败、取消的调用同样占名额。

---

## 4 拆分判据、扫描与待办

本节回答一个问题：**前四阶段的代码里，哪些文件需要拆。** 判据沿用 [REFACTOR-SPLIT-PLAN](./REFACTOR-SPLIT-PLAN.md)（下称「拆分规划」），本节只做前四阶段的落地扫描，不另立标准。

### 4.1 判据与档位

**主判据（拆分规划 §6.2）**：一个文件只对应一个**变化原因**。同步策略、加密算法、UI 布局、DTO 契约、循环阶段互不混居；职责数 ≥3 且彼此无共同依赖即为职责混杂。**拆分粒度由职责决定，不设行数目标**。

**行数只作初筛（拆分规划头部与 §1.2）**：

| 档位 | 动作 |
|---|---|
| ≥900 行 | 优先审查（必看职责数） |
| 300–899 行 | 视职责数决定 |
| <300 行 | 一般不动 |

**内联测试的处理（拆分规划 §2.2 + §6.3）**：内联测试超标不是拆分理由，动作是**移测试**而非拆实现。因此下表把「内联测试」单列一栏，只用于区分「长度来自实现」还是「长度来自测试」，不参与职责判断。

**单函数长度**：拆分规划未设阈值，本节按「先拆函数、再考虑拆文件」的次序记录——`run()` 是前四阶段唯一超 400 行的函数。

### 4.2 全量扫描（前四阶段，非空行 ≥ 300）

**≥900 行档（优先审查）**

| 文件 | 非空行 | 内联测试 | 职责数 | 结论 |
|---|---|---|---|---|
| `agent/loop_executor.rs` | 1403 | 0 | **6** | 职责混杂，**待拆**，见 §4.3 SP-1 |

**300–899 行档（视职责数）**

| 文件 | 非空行 | 内联测试 | 职责数 | 结论 |
|---|---|---|---|---|
| `os/fs_link.rs` | 766 | 294 | 4（工具壳 / 路径校验 / 链接创建 / 链接检查） | 职责混杂，拆分规划 §3.6 已列方案 `os/fs_link/{mod,validate,create,check}.rs` |
| `multimodal.rs` | 761 | 0 | 6 | I2e 遗留、非前四阶段新增；拆分规划 §3.5 已列方案，此处不重复 |
| `kernel/ports/test_doubles.rs` | 736 | 53 | 1（端口替身） | 职责单一，不动；`cfg` 隔离不进生产二进制 |
| `os/path.rs` | 567 | 271 | 1（路径安全） | 职责单一，不动；长度为内联测试 |
| `assistant-tools/gateway.rs` | 561 | 0 | 2（审批决策 / Prompter 契约） | 二者共用同一契约，单一变化原因，观察 |
| `agent/middleware.rs` | 517 | 0 | 7 中间件 + 链装配 | 同 trait、同链、同装配入口，变化原因收敛在「中间件链契约」，观察 |
| `kernel/runner.rs` | 376 | 0 | 2（turn 编排 / 终态收敛） | 单一变化原因（turn 生命周期），观察 |
| `assistant-tools/pipeline.rs` | 355 | 0 | 1（唯一执行入口） | 职责单一，不动 |
| `token_estimate.rs` | 342 | 92 | 2（估算 / 校准） | 观察 |
| `agent/tool_call.rs` | 321 | 0 | 1（增量累积） | 职责单一，不动 |

**<300 行档（一般不动）**：`kernel/stream.rs` 299、`adapters/event_sink.rs` 243，及其余全部 P0–P4 文件。

**扫描结论**：前四阶段新增代码中，**需要动手的只有 `agent/loop_executor.rs`**；`os/fs_link.rs` 与 `multimodal.rs` 的拆分已由拆分规划登记，本节只做交叉引用，避免两处方案分叉。

### 4.3 拆分待办

| # | 文件 | 现状 | 切分方案 | 状态 |
|---|---|---|---|---|
| **SP-1** | `agent/loop_executor.rs` | 1403 行，`run()` 514 行，6 段职责 | 三刀，见下表 | **暂缓**，见 §4.4 |

SP-1 的三个切分点，每一刀都是**按依赖闭合的整块**（切出后不产生新的跨文件状态传递，也不改公开 API）：

| 刀 | 切出内容 | 目标文件 | 依赖闭合性 |
|---|---|---|---|
| 1 | `stream_failure` / `cancelled_before_call` / `cancelled_tool_exec` / `stopped` / `halted_with_summary` / `halted_by_budget` | `agent/terminal.rs` | 只依赖 `RoundState` + `TurnRunCtx` |
| 2 | `ensure_row` / `persist_row` / `persist_row_as_new` / `persist_final_text` / `append_tool_results` | `agent/persist.rs` | 只依赖 `RoundState` + `ctx` |
| 3 | `merge_usage` / `split_by_budget` / `effective_model_call_budget` / `decide_stop_phase` / 指纹与 JSON 构造 | `agent/budget.rs` | 纯函数，无 `self` 状态 |

三刀切完后 `run()` 只剩阶段编排与调用，`loop_executor.rs` 归入「循环编排」单一变化原因；三刀之外的 `run()` 本身按拆分规划 §6.3 的顺序要求，先拆函数（按 §3.2 的 14 个阶段编号抽阶段函数），再考虑是否拆文件。

### 4.4 当前结论

**暂缓，不动。** 两条理由：

1. **P4 未验收**：C4.24–C4.38 循环单测与 C7 端到端仍待补，此时移动文件会让测试基线漂移，且 [I38b tasklist](./I38b-agent-loop-tasklist.md) 里的路径引用需要连带更新。
2. **拆分规划 §6.3 要求搬移与改写分开提交、一个 PR 只拆一个文件**：SP-1 需要独立 PR 与完整回归，不能混进 P4 收尾提交。

**注意**：本节的「暂缓」是执行时序安排，不是「已判定不需要拆」——按 §4.1 的主判据，`loop_executor.rs` 的 6 段职责已构成职责混杂，拆分方案在 §4.3 已备好，执行时照表搬运即可。

---

## 5 后续阶段的增量落点

| 阶段 | 落点 | 预计规模（估算，非承诺） |
|---|---|---|
| P5 OS 工具 | `assistant-tools/src/os/`：`agent_read` / `agent_list` / `agent_glob` / `agent_grep` / `agent_write` / `agent_edit` / `agent_mkdir` / `agent_move` / `agent_delete` / `agent_shell`（注册名依次为 `read` / `ls` / `glob` / `grep` / `write` / `edit` / `mkdir` / `move` / `delete` / `shell`）；新增 `os/output.rs` 截断；`setup.rs` 分三组注册 | 1500–2500 行源码 + 同量测试 |
| P6 前端与模式 | 主体在 TS；Rust 侧仅替换 `adapters/approval.rs` 内层实现、新增审批命令、`dto/agent.rs` 增审批事件字段 | Rust 侧数百行 |
| P7 MCP | 新模块承载客户端与 `McpServerManager`，`mcp__{server}__{tool}` 桥接到现有 `Tool` trait，复用 P2 管线 | 1000–2000 行 |

---

## 6 维护约定

- **重测命令**（在 `src-tauri` 目录下）：

```powershell
Get-ChildItem -Path crates\assistant-agent\src -Recurse -Filter *.rs |
  ForEach-Object { "L={0} F={1}" -f ((Get-Content -LiteralPath $_.FullName | Measure-Object -Line).Lines), $_.FullName }
```

- **拆分判据归属**：§4 的判据、档位、目录模块模式、禁止事项全部沿用 [REFACTOR-SPLIT-PLAN](./REFACTOR-SPLIT-PLAN.md) §6；本文只负责前四阶段的扫描结果与待办，不另立标准。方案冲突时以拆分规划为准。
- **不写进度**：本文只记录「有什么、多大、归谁管」；某项是否完成一律回指 roadmap §2 与 tasklist。
- **行数不是指标**：行数只用于初筛定位（≥900 优先审查 / 300–899 视职责数 / <300 一般不动），拆分与否由职责边界决定；本文也不做工作量或质量评价。
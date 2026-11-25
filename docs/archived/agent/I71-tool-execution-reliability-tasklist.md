# I71 — 工具执行链路加固任务清单

| 项 | 值 |
|---|---|
| 状态 | 任务清单（唯一任务级进度；不占阶段号，进度只在本文件勾选） |
| 日期 | 2026-11-23 |
| 规格 | [I71-tool-execution-reliability-spec.md](./I71-tool-execution-reliability-spec.md)（唯一设计来源；本文不复述语义，只勾进度） |
| 判据 | 规格 §2.4 A1–A4 / §3.4 B1–B5 / §4.4 C1–C4 |
| 前置 | P2（工具控制层）、P6（前端与审批 UI）、P14（`shell` 沙箱执行层）均已完成；本批不改 P0–P18 门禁，不改 I69 波次 |

批次对应规格章节：W1 取消兜底（§2.1–§2.3 / A1–A4）→ W2 调用来源归因（§3.1–§3.3 / B1–B5）→ W3 单工具内联预算（§4.1–§4.3 / C1–C4）。三项互相独立，可按任意顺序落地、独立回滚。

> 本清单不管理提交。提交粒度按代码依赖与可编译性现场决定；这里只回答「代码改没改、验收跑没跑」。
> 勾选表示代码已改且对应验收已跑。

---

## 阶段 W1 — 取消兜底（规格 §2，A1–A4）

- [x] `os/tools/agent_shell/run.rs`：新增 `RunGuard { kill: Option<KillHandle>, scratch: PathBuf }` 与 `Drop`（`kill()` + `cleanup_scratch()`），在调用级临时目录创建后立即构造、杀树句柄在建进程后填入
- [x] `runtime/storage.rs`：`FileSpillSink` 增 `finished` 标志；`finish` 首行置位；`Drop` 仅在未收尾时先 `take()` 句柄再删文件，`NotFound` 静默
- [x] `os/tools/agent_shell/spawn.rs`：`cleanup_scratch` 对 `NotFound` 静默（守卫在正常路径二次调用不产生日志噪音）
- [x] `tests/agent_shell.rs`：新增 `a39_abandoned_future_reclaims_tree_scratch_and_spill`（放弃 future 后 `active_processes()` 归零、临时目录不存在、落盘文件不存在）

> 验收：`cargo test -p assistant-tools --test agent_shell`（24 项）。

---

## 阶段 W2 — 调用来源归因（规格 §3，B1–B5）

- [x] `contract/context.rs`：新增 `ToolCallSource { Agent, Toolbox, Orchestration }`（`Default = Agent`，`as_str()` 给出 `agent` / `toolbox` / `orchestration`）
- [x] 同文件 `ToolUseContext`：增 `#[serde(skip)] source`；`for_toolbox()` 显式填 `Toolbox`
- [x] `runtime/gateway/stores.rs`：`AuditEntry` 增 `source: Option<ToolCallSource>`；`allowed()` 填 `None`、`bypassed()` 填 `Some(Toolbox)`
- [x] `runtime/gateway/mod.rs`：网关审计构造点填 `Some(context.source)`
- [x] `assistant-storage/src/db.rs`：`permission_audit` 建表增 `source TEXT`；`pragma_table_info` 守卫补列，不回填存量行
- [x] `adapters/permission_store.rs`：`log_decision` 的 `INSERT` 增 `source` 参数
- [x] `cmd/permission/audit.rs`：`write_grant_audit` 写 `source = NULL`；`query_permission_audit` 投影增列
- [x] `assistant-protocol/src/agent.rs`：`PermissionAuditEntryDto` 增 `source: Option<String>`；`npm run contract:gen` 已重生成 `src/types/protocol.contract.ts`
- [x] `tests/gateway.rs`：轮内调用审计行 `source = agent`；新增「来源只做归因、不改决策」用例（同策略下 `Agent` / `Toolbox` 决策相同、来源不同）
- [x] `assistant-storage/tests/db.rs`：新增 `init_db_adds_audit_source_column_to_legacy_table_without_backfill`（存量行保持 `NULL`、新行带来源、补列可重复执行）
- [x] `src/cmd/permission/audit.rs`：新增 `source_is_projected_without_backfilling_legacy_rows`（查询投影原样透出、存量行 `None`）

> 验收：`cargo test -p assistant-tools --test gateway`；`cargo test -p assistant-storage`。

---

## 阶段 W3 — 单工具内联预算（规格 §4，C1–C4）

- [x] `contract/trait.rs`：`Tool` 增默认方法 `inline_output_limit(&self) -> Option<usize>`，默认 `None`
- [x] `runtime/storage.rs`：新增 `select_persist_targets_with_limits(sizes, limits)`；`select_persist_targets(sizes)` 改为全 `None` 薄委托（既有调用点与单测不变）；`limits` 缺位 / 缺项按全局阈值
- [x] `runtime/pipeline.rs`：预算判定前按 `tool_name` 回查注册表取上限，回查失败按 `None`
- [x] `os/tools/agent_read/mod.rs`：`read` 覆写 `Some(2 * PER_TOOL_LIMIT_CHARS)`
- [x] `tests/storage.rs`：新增 C1（60k 不落盘）、C2（120k 仍落盘）、C4（四份 60k 仍受单轮预算约束）、T3（`limits` 缺位回落默认）四例

> 验收：`cargo test -p assistant-tools`（含 storage 单测）。

---

> 验收记录（2026-11-23）：`cargo test -p assistant-tools` 全绿（lib 321 项；agent_shell 24 项含新增 A39；gateway 含新增来源归因用例；storage 含新增 C1/C2/C4/T3）；`cargo test -p assistant-storage` 全绿（含 `builtin_seed_tool_config_matches_defaults` 同步 `workspace_write` 种子与空规则层；新增 `init_db_adds_audit_source_column_to_legacy_table_without_backfill` 覆盖 B4）；`cargo test -p assistant-app --lib` 全绿（含新增 `source_is_projected_without_backfilling_legacy_rows` 覆盖 B5）；`cargo test -p assistant-protocol` 全绿；`cargo test -p assistant-app --test permission_policy --test tool_resolver` 全绿；`cargo check --workspace --tests` 通过；`cargo fmt --all -- --check` 通过；`npm run contract:gen` 已重生成且 `source` 字段入契约；`npm run typecheck` 通过；`npx vitest run`（conversationLink / toolConfig / turnOrchestrator）79 项通过。
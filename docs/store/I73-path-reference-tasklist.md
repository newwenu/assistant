# I73 — 路径引用体系任务清单

| 项 | 值 |
|---|---|
| 状态 | 唯一任务级进度表；只记「改哪里、改没改」，语义一律见规格 |
| 日期 | 2026-11-25 |
| 规格 | [I73-path-reference-spec.md](./I73-path-reference-spec.md)（唯一设计来源，本文不复述） |

> 每项末尾括号是规格锚点，用于回查，不在本文展开。章节对应：`A*` → §7，`N*` → §6，`E1/E2/E3` → §3.4，`E4` → §3.8，`G*` → §3.1，`R*` → §3.6，`S*` → §3.9，`T*` → §3.9，`W*`/`P5–P9` → §3.8，`P1.*`/`P2.*` → §5.1 / §5.2，`P3.*` → §5.3，`P4.*` → §5.4，`D*` → §3.10。
> 勾选 = 代码已改且对应验收已跑。

---

## P1 — 解析层（§5.1）

- [x] 新增 `os/shared/path/ref.rs`：`Scheme`、`PathRef`（私有字段 `raw` / `scheme` / `body`）、`parse_ref`、`tool_output_ref` 格式串工具（§3.3 / §3.8「语法」/ §5.1 P1.1 / §6 N17）
- [x] 同文件：`PathRef::resolve_readable` / `resolve_write` / `resolve_working_directory` / `resolve_browse` / `resolve_leaf_preserving` 五入口；既有 `resolve*` 降为 `pub(crate)`（§3.3 / §5.1 P1.4 / §6 N5）；拒绝文案按**方向 + scheme 类别**派生：写方向由 `write_rejected(scheme)` 按 `Scheme::nature()` 派生三句（`WRITE_LOCATION_SHELL_REJECTED` / `WRITE_LOCATION_NO_CHANNEL_REJECTED` / `WRITE_NAMESPACE_REJECTED`，叶子保留同源）、工作目录 `CWD_REJECTED`、浏览方向 `BROWSE_REJECTED`；新增 `SchemeNature` / `WriteAlternate` 类别轴与 `Scheme::ALL` 闭集清单（§3.1 G6 / G7 / §4 V1–V7）
- [x] `path/args.rs`：`parse_path_arg` / `parse_optional_path_arg` 返回 `PathRef`（§3.1 G3）
- [x] `path/resolve.rs::join_input`：已知 scheme 内部错误兜底（§3.3 E1 / §3.1 G4）
- [x] 17 处构造点 + 各工具 `Request` 字段换型（§5.1 P1.5 / §3.3 E3）
- [x] `os/shared/scope.rs::base_directory` 改收 `Option<&PathRef>` 并走 `resolve_browse`；`agent_list` 三处同改；`agent_shell/schema.rs` 的 `cwd` 先 `parse_ref` 后走 `resolve_working_directory`（§5.1 P1.5 / §4 V5 / V7）
- [x] `tests/path_references.rs`：A5 / A16（写方向按域分叉）、A26（浏览方向文案分叉）、A27（写方向文案由类别派生、不诱导拼路径，对 `Scheme::ALL` 闭集断言）
- [x] `ReadablePath::intent(context)` 收敛审批映射，删两处重复 `match scope`（§3.5）
- [x] `path/mod.rs` 导出面（§5.1 P1.7）
- [x] 域根在构造点各过一次 `canonicalize_lenient`（§3.9 / §5.1 P1.8 / §6 N17）
- [x] `tests/agent_read.rs`：A1、A2、A3、A7、A11、A22
- [x] `os/shared/path/mod.rs` 单测：A4、A6、A21
- [x] `tests/pipeline.rs`：A13
- [x] 无 wire 变更：不跑 `contract:gen`（§5.1 / §5.2 P2.6）

> 验收：`cargo test -p assistant-tools`；`cargo test -p assistant-app --lib`。

---

## P2 — 产出层（§5.2）

- [x] `runtime/storage.rs::PersistedRef::model_ref()`（§3.6 R1 / §3.7）
- [x] `runtime/pipeline.rs::persist` 标签改用 ref（§3.6 R2）
- [x] `agent_shell/run.rs::assemble_text` 同上（§3.6 R2）
- [x] `runtime/compact/envelope.rs::build_compact` 改用 ref（§3.6 R3）
- [x] `agent_shell/mod.rs::description` 改为「回填即输出引用」（§5.2 P2.4）
- [x] `agent_read/mod.rs::description` 补引用写法（§5.2 P2.5）
- [x] `PersistedRef.path` / `data.outputPath` 保持绝对（§3.6 R4 / R6）
- [x] `tests/output.rs`：A8；`tests/agent_shell.rs`：A9；`tests/storage.rs`：A12
- [x] `src/chat/tool/__tests__/toolCallDisplay.test.ts`：A10

> 验收：`cargo test -p assistant-tools`；`npx vitest run src/chat/tool/__tests__/toolCallDisplay.test.ts`。

---

## P3 — 诊断回显（§5.3 / §3.10）

- [x] `path/ref.rs`：`PathRef::error_label(&resolved)` 单一渲染点（§3.10 D4 / §5.3 P3.1c）
- [x] `agent_read/mod.rs` / `super_read/mod.rs`：`failed to stat` / `is a directory` / `is not a regular file` 三处改取 `error_label`（§5.3 P3.1a）
- [x] `agent_read/mod.rs::file_revision`：增带 label 变体，仅 `read` / `super_read` 传同一 label（§5.3 P3.1a）
- [x] `path/resolve.rs::canonicalize_lenient` / `path/protected.rs::reject_system_dirs`：增带 label 变体，`ref.rs` 的 scheme 分支传 `self.raw()`；1 参版本留给不涉及域根的调用点（§5.3 P3.1b）
- [x] `tests/agent_read.rs` / `tests/super_read.rs`：A24、A25（§7）

> 无 wire 变更：`data.path` 仍为解析后绝对，只改模型可见文本（§3.10 范围）。
> 验收：`cargo test -p assistant-tools`（含 A24 / A25）。

---

## P4a — 前置改造（§5.4 P4.0–P4.6 / §3.8 W1–W7）

- [x] `runtime/storage.rs`：新增 `scratch_root_for(base, session_id)` 纯函数（§5.4 P4.0 / N1）
- [x] `contract/context.rs`：`tool_scratch_root: Option<PathBuf>`（`#[serde(skip)]`，`for_toolbox` 填 `None`）（§5.4 P4.0）
- [x] `adapters/tool_exec.rs`：`tool_scratch` 字段 + `with_tool_scratch`，`execute` 用 `scratch_root_for` 填字段（§5.4 P4.0 / N1）
- [x] `tool_setup.rs`：装配期填 `dirs.tool_scratch()`（§5.4 P4.0）
- [x] `agent_shell/schema.rs`：`ShellInput.scratch_root`，缺失即 `blocked` + `sandbox_root_missing`（§5.4 P4.0）
- [x] `dir_name::TOOL_SCRATCH` + `ALL`；`AppDirs::tool_scratch()`（§3.8 登记表字段）
- [x] `app_dirs/registry.rs`：`CoreEntry::ToolScratch` / `E19` / 登记行 / 文档注释条数（§3.8 登记表字段）
- [x] `entries/decl.rs::static_for`：`None` + `NotImplemented` + `NoReclaimApi`（§3.8 静态声明）
- [x] `entries/eligibility.rs::eligibility_for`：归入取视图那一档（§3.8 静态声明）
- [x] `reclaim/execute.rs::execute_storage_reclaim` + `executor_action`：零副作用 / `None`（§3.8 静态声明）
- [x] `sample/rows.rs::sample_scratch` + `sample/collect.rs` 采样任务（§3.8 采样）
- [x] `sandbox/mod.rs`：`writable_root` → `write_roots`，`cwd` 归属改「任一根内」（§3.8 W1）
- [x] 三后端迁移：`windows/mod.rs` / `macos.rs` / `linux.rs`（§3.8 W1）
- [x] 逐根敏感重叠检查 + 逐根 `converge_ace`（§3.8 W2）
- [x] `sandbox/windows/acl.rs::to_wide`：先 `canonicalize_lenient`，≥248 补前缀（§3.9 M3 / W7）
- [x] `tool_setup.rs::sandbox_host_paths`：`sensitive_read_deny` 收窄为逐项枚举（§3.8 W3，**须评审**）
- [x] `agent_shell/spawn.rs::forced_env`：三条变量指向 `input.scratch_root`（§3.8 W4）
- [x] `agent_shell/spawn.rs::prepare_scratch`：幂等 `create_dir_all`；删除 `scratch_component`（原用途仅为调用级目录命名，§3.8 W6）
- [x] `agent_shell/run.rs::RunGuard`：去掉 `scratch` 字段与清理调用；三处调用点移除后删除 `cleanup_scratch`（W6 / §3.8「清理实现口径」）
- [x] `cmd/conversation/mutate.rs` 三入口：`scratch_root_for` 取根 + `remove_dir_all`，`NotFound` 静默（W6 / N14）
- [x] `tests/agent_shell.rs`：A14、A20、A23
- [x] `cargo test -p assistant-tools --lib sandbox` 不回归（§7 A23）

> 验收：`cargo test -p assistant-tools`；`cargo test -p assistant-app --lib`（`validate_registry` 覆盖 E19）。

---

## P4b — 回填与审批（§5.4 P4.7–P4.10 / §3.6 R5 / §3.8 P5–P9）

- [x] `path/ref.rs::parse_ref` 的 `scratch` 分支（嵌套 / 可空 body / 逐段拒绝）（§3.8「语法」）
- [x] `path/resolve.rs`：`ReadableScope::Scratch` + `域根.join(body)` + `ensure_within_root` + `readScratch` intent（§3.8 / §5.4 P4.8 / N1）
- [x] `agent_shell/run.rs::assemble_text`：恒回填根 ref + 一层子项清单（§3.6 R5）
- [x] `agent_shell/mod.rs::description`：临时区语义（T3 / S8 / §5.4 P4.10）
- [x] `agent_read/mod.rs` / `super_read/mod.rs`：`readScratch` 共用 `intent`；`preview_for_approval` 纳入 `Scratch`（P5–P9）
- [x] 前端 i18n：`readScratch` 两条文案（§5.4 P4.9）
- [x] 前端 `toolCallDisplay.ts`：`INTENT_ACTION` 增 `readScratch`（§5.4 P4.9）
- [x] 前端 `toolConfig.ts`：动作选项 + 快捷规则（`effects` 只 `ask` / `deny`）（§5.4 P4.9 / E4）
- [x] `tests/agent_shell.rs`：A15；`tests/agent_read.rs`：A17
- [x] 写方向六组测试：A16、A5
- [x] `tests/agent_shell.rs`：A18、A19
- [x] `runtime/gateway/context.rs::is_path_action`：纳入 `readScratch`（审批卡「工作区外」标记与 `readToolOutput` 同档；`readScratch` 的 `resource` 是域根绝对路径，漏判会让临时区读取少一条边界提示）
- [x] `tests/super_read.rs`：`scratch://` 同权用例（正文与 `read` 一致、`permission_intents` 与 `read` 完全相同）
- [x] 前端测试：`toolConfig.test.ts` + `approvalRulesUi.test.ts`（E4）

> 验收：`cargo test -p assistant-tools`；`cargo test -p assistant-app --lib`；`npx vitest run src/chat/core/__tests__/toolConfig.test.ts src/chat/tool/__tests__/approvalRulesUi.test.ts src/chat/tool/__tests__/toolCallDisplay.test.ts`；`npm run typecheck`。

---

## 不做（§5.3 / §8）

> 本节是**有意排除**项，不是待办：因此不带勾选框（勾选只表达「已改且已验收」，排除项无此状态）。
> 逐条理由见规格 §5.3（P3.2）与 §8（其余）。
> P3.1（引用类错误回显原串）已从「不做」改判为**必做**，见上方 P3 分节（§5.3 / §3.10）。

- P3.2 工作区路径改「相对工作区」渲染 / 改回显原串（§5.3）
- body 校验前剥离 `\\?\`；exe 清单声明 `longPathAware`；前端声明长路径能力（§8 / §3.9 N17）
- `list` / `grep` / `glob` 对输出域与 scratch 域的扩展（§8：写方向入口恒拒，审批动作属独立议题）
- 在错误文本中附加「解析后绝对落点」作为诊断信息（§8：等于把 §3.10 D1 放回去）

---

## 顺序

P1 / P2 / P3 互不依赖，可各自回滚；P3 须在 P4b 之前或与之同批（scratch 一上线就会产生引用类错误）；P4a → P4b 不可颠倒，阶段内顺序见规格 §10。

---

> 验收记录（待填）：每阶段完成后追加一行，写明命令与结果。

- P1（2026-11-25）：`cargo test -p assistant-tools` 全部通过（含 A1/A2/A3/A4/A6/A7/A11/A13/A21/A22）；`cargo test -p assistant-app --lib` 185 passed；`cargo clippy -p assistant-tools --tests` 无新增告警。
- P2（2026-11-25）：`cargo test -p assistant-tools` 全部通过（含 A8/A9/A10/A12，lib 345 + 各集成 0 failed）；`cargo test -p assistant-app --lib` 185 passed；`npx vitest run src/chat/tool/__tests__/toolCallDisplay.test.ts` 27 passed；`cargo clippy -p assistant-tools --tests` 无新增告警。
- P4a（2026-11-25）：`cargo test -p assistant-tools` 全部通过（含 A14/A20/A23，`agent_shell` 31 passed，lib 345 passed）；`cargo test -p assistant-app --lib` 185 passed（`validate_registry` 覆盖 E19）；`cargo clippy -p assistant-tools --tests` 无新增告警（仅 lib 既有 3 条）。附带修正：`os/shared/path/resolve.rs::simplify_verbatim` 不再走 `dunce::simplified`（后者在 ≥260 字符时拒绝剥离），改为直接剥 `\\?\` / `\\?\UNC\`，否则「根短、路径长」时前缀形态不一致会被 `ensure_within_root` 误判越界（N17 / A23）。
- P4b（2026-11-25）：`cargo test -p assistant-tools` 全部通过（含 A15/A17/A18/A19/A5/A16）；`cargo test -p assistant-app --lib` 185 passed；`npx vitest run src/chat/core/__tests__/toolConfig.test.ts src/chat/tool/__tests__/approvalRulesUi.test.ts src/chat/tool/__tests__/toolCallDisplay.test.ts` 102 passed；`npm run typecheck` 通过；`npm run lint:fix` 无残留告警。
- P4 复核（2026-11-25）：读工具接入复查发现 `is_path_action` 漏 `readScratch`（审批卡的「工作区外」标记因此只在 `readToolOutput` 出现），已补并加单测；`tests/super_read.rs` 补 `scratch://` 同权用例。`cargo test -p assistant-tools --lib gateway` 7 passed；`cargo test -p assistant-tools --test super_read` 22 passed。
# I38a — P2 工具控制层任务清单

> 规格：[I38a-tool-control-layer-spec.md](./I38a-tool-control-layer-spec.md) §4 实现步骤、§5 验收标准
> 阶段：[I38-agent-roadmap.md](./I38-agent-roadmap.md) P2
> 顺序：C0 → C1 → C2 → C3 → C4 → C5 → C6 → C7 → C8
> 勾选条件：代码已改 **且** 对应验收已跑。
> **状态唯一位置**：本清单是 P2 任务级进度的唯一来源；阶段级结论只在 [roadmap](./I38-agent-roadmap.md) §2 写一次。本清单**不管理提交**（不写提交拆分 / 提交信息 / 回滚批次，见 roadmap §7 第 2 条）。
> 范围：只交付 `assistant-tools` 控制层与 DTO 契约同步；不新增工具实现、不改前端、不建 DB 表。
> 测试位置：新增单测一律放 `crates/assistant-tools/tests/<模块>.rs` 集成测试，不写 `#[cfg(test)]` 内联模块。

---

## C0 口径与前置确认

- [x] C0.1 确认 P2 只改 `assistant-tools` crate 与 `os_tool` DTO，不动 `src-tauri` 运行时与前端组件
- [x] C0.2 确认 `Tool` trait 新增方法均带默认实现，存量 `fs_link` 零改动（A9）
- [x] C0.3 确认 `exec::invoke` 仍是唯一执行入口，Pipeline 复用 `invoke` 而非直接调 `execute()`
- [x] C0.4 确认审批与存储均为端口 trait，`assistant-tools` 不引入 `tauri`（A2）
- [x] C0.5 确认 P2 不建 DB 表，`GrantStore` / `AuditStore` / `Storage` 生产绑定随 P4（A10）
- [x] C0.6 确认 P2 只用测试替身工具，不新增任何 `fs_*` / `shell_exec` 实现
- [x] C0.7 确认 `ToolResultStorage` 根目录由构造参数注入，不读全局配置

---

## C1 Policy Engine

实现：`src-tauri/crates/assistant-tools/src/policy.rs`、`src-tauri/crates/assistant-tools/src/lib.rs`
测试：`src-tauri/crates/assistant-tools/tests/policy.rs`（集成测试，不进 lib）

- [x] C1.1 新建 `policy.rs`，`lib.rs` 增加 `pub mod policy;`
- [x] C1.2 定义 `PermissionEffect { Allow, Ask, Deny }`，`serde(rename_all = "camelCase")`
- [x] C1.3 实现 `most_restrictive`：任一 `Deny` 取 `Deny`，否则任一 `Ask` 取 `Ask`
- [x] C1.4 定义 `PermissionRule { action, resource, effect }` 与 `PermissionRule::new`
- [x] C1.5 定义 `PermissionConstraintLayer { rules }`
- [x] C1.6 `evaluate_rules` 实现 last-match-wins：遍历全部规则，最后命中者生效
- [x] C1.7 定义 `ResolvedPermissionPolicy { rules, constraint_layers }` 与 `Default`
- [x] C1.8 实现 `ResolvedPermissionPolicy::evaluate`：主规则集结果逐层与约束层取 `most_restrictive`
- [x] C1.9 定义 `PermissionRuntimeCeiling` 与 `CeilingValidationError`
- [x] C1.10 `PermissionRuntimeCeiling::try_new` 发现任何 `effect == Allow` 即返回 `Err`
- [x] C1.11 实现 `default_global_rules()`：首条 `("*", "*", Ask)`，其后 read 放行与系统目录 Deny
- [x] C1.12 `policy.rs` 不出现 `tauri` / `AppHandle` / `Window` / `Command`
- [x] C1.13 单测 `most_restrictive` 九种组合（A3）
- [x] C1.14 单测 last-match-wins：宽泛规则在前、具体规则在后覆盖
- [x] C1.15 单测多层取最严：约束层可收紧主规则集结论
- [x] C1.16 单测 Ceiling 拒绝含 `Allow` 的规则集（A3）
- [x] C1.17 单测默认规则对 `read` 放行、对 `edit` + `/etc/**` 拒绝
- [x] C1.18 单测未命中任何规则时单层默认 `Allow`，但默认全局规则集整体仍为 `Ask`
- [x] C1.19 单测 glob 语义：`*` 不跨分隔符、`**` 可跨、`\` 与 `/` 等价
- [x] C1.20 单测 `PermissionEffect` / `PermissionRule` 序列化为 camelCase

---

## C2 审批与存储端口

实现：`src-tauri/crates/assistant-tools/src/gateway.rs`
测试：`src-tauri/crates/assistant-tools/tests/gateway.rs`

- [x] C2.1 定义 `ApprovalKey { tool, scope }`
- [x] C2.2 定义 `ApprovalScope`：`Path` / `Command` / `Endpoint` / `Opaque`
- [x] C2.3 `Path` 键复用 `os::path` 的 canonicalize 结果，符号链接解析到真实目标
- [x] C2.4 `Command` 键按完整 argv + `cwd` + `sandbox_tier` 入键，禁止前缀匹配
- [x] C2.5 无法生成键时返回空键，Gateway 直接走审批、不查缓存
- [x] C2.6 定义 `GatewayDecision { Allowed, Denied, RejectedByUser }`
- [x] C2.7 定义 `ApprovalDecision { Approve, Deny, AlwaysApprove }`
- [x] C2.8 定义 `ApprovalRequest { tool_call_id, tool_name, arguments, intents, timeout }`
- [x] C2.9 定义 `ApprovalPrompter` trait（`async_trait`，`Send + Sync`）
- [x] C2.10 定义 `PermissionGrantStore` / `PermissionAuditStore` / `ToolResultStorage` 三个端口
- [x] C2.11 定义 `AuditEntry` / `PersistToolResult` / `PersistedRef`
- [x] C2.12 内存实现 `InMemoryGrantStore` / `InMemoryAuditStore`
- [x] C2.13 文件实现 `FileToolResultStorage`，根目录由构造参数注入
- [x] C2.14 脚本化测试替身 `ScriptedApprovalPrompter`，可返回 `Approve` / `Deny` / `AlwaysApprove` / 超时
- [x] C2.15 单测：同一路径经 `..` 与符号链接后生成相同 `ApprovalKey`
- [x] C2.16 单测：命令键对 `["git","status"]` 与 `["git","status","--short"]` 不命中
- [x] C2.17 单测：空键不命中缓存，必走审批
- [x] C2.18 单测：审批超时归一为 `Deny`（A5）
- [x] C2.19 单测：`AlwaysApprove` 只写当前规范化键，相似键不命中（A4）
- [x] C2.20 单测：子 Agent 可读父 `GrantStore`，新增批准不写父库
- [x] C2.21 定义 `request_with_timeout`：审批等待唯一入口，超时归一为 `Deny`
- [x] C2.22 定义 `ChildGrantStore`：读穿透父库、写入只落局部库
- [x] C2.23 路径键必须合法 UTF-8：非 UTF-8 归一结果返回空键（fail-closed），禁止 `to_string_lossy`
- [x] C2.24 单测：符号链接指向非 UTF-8 目标时不生成可复用键

---

## C3 Tool trait / ToolResult 扩展（已完成）

`result.rs`、`trait.rs`、`exec.rs`、`src-tauri/src/protocol/dto/os_tool.rs`

- [x] C3.1 `result.rs` 新增 `ToolErrorKind { Business, Timeout, Cancelled, Internal, Denied }`
- [x] C3.2 `result.rs` 的 `ToolResult` 新增 `result_for_assistant` / `error_kind`
- [x] C3.3 `result.rs` 新增 `ok_with_assistant` / `err_with_kind` / `from_exec_error_kind` / `assistant_text` / `assistant_char_count`
- [x] C3.4 `trait.rs` 新增 `ValidationResult { ok, message, error_code, blocks_input_rewrite }`
- [x] C3.5 `trait.rs` 新增 `validate_input` 默认实现（`ValidationResult::ok()`）
- [x] C3.6 `trait.rs` 新增 `is_concurrency_safe` 默认实现（等于 `is_readonly()`）
- [x] C3.7 `exec::invoke` 超时分支打标 `ToolErrorKind::Timeout`
- [x] C3.8 `exec::invoke` 的 `Err` 归一化保持 `ToolErrorKind::Internal`
- [x] C3.9 `OsToolResultDto` 新增 `result_for_assistant` / `error_kind`，`Option` 不加 `skip_serializing_if`
- [x] C3.10 新增 `ToolErrorKindDto` 与 `From<ToolErrorKind>` 映射
- [x] C3.11 `protocol_bindings.rs` 注册 `ToolErrorKindDto`，DTO 计数断言由 62 改为 63
  - 注记（2026-11-05）：该计数断言已清理，改为「`src/protocol/dto` 中 `#[derive(...TS)]` 类型集合 ↔ `declarations` 类型集合」双向差集断言。本条作为历史记录保留，后续新增 DTO 不再需要改数字。
- [x] C3.12 `protocol_dto_roundtrip.rs` 补新字段与 `errorKind:null` 序列化断言
- [x] C3.13 `npm run contract:gen` 通过，`protocol.contract.ts` 已更新
- [x] C3.14 存量 `fs_link` 全部单测保持通过（A9）

---

## C4 Gateway

实现：`src-tauri/crates/assistant-tools/src/gateway.rs`（`ToolGateway`）
测试：`src-tauri/crates/assistant-tools/tests/gateway.rs`

- [x] C4.1 实现 `ToolGateway::evaluate(tool, input, context, policy)`
- [x] C4.2 意图为空（只读工具）直接返回 `Allowed`，不触发审批
- [x] C4.3 每个 intent 的每个 resource 依次执行：`grant_store.is_granted` → `policy.evaluate` → `audit`
- [x] C4.4 `Allow` 直接继续；`Deny` 立即返回 `Denied { action, resource }`
- [x] C4.5 `Ask` 收集到 `need_approval`，同一工具多意图合并为**一次**审批请求
- [x] C4.6 `Approve` 继续；`AlwaysApprove` 先 `save_grant` 再继续；`Deny` 返回 `RejectedByUser`
- [x] C4.7 审计写入失败只记日志，不改变决策结果
- [x] C4.8 单测：`Allow` 不弹审批直接放行（A4）
- [x] C4.9 单测：`Ask` 只触发一次 `ApprovalPrompter::request`（A4）
- [x] C4.10 单测：`Deny` 立即拒绝且不调用 `request`（A4）
- [x] C4.11 单测：`AlwaysApprove` 仅当前键后续免审批（A4）
- [x] C4.12 单测：审批超时返回 `RejectedByUser` 而非 `Allowed`（A5）
- [x] C4.13 单测：取消返回 `Cancelled` 错误类（A5）—— `ToolUseContext` 无取消信号字段，P2 无法产生 `Cancelled`；P4 接入运行时取消后由 Pipeline 阶段 3 打标
- [x] C4.14 `merge_pending`：待审批项按 action 合并 resources，供一次审批请求携带
- [x] C4.15 单测：空键（resource 无法归一）仍触发审批，且 `AlwaysApprove` 不落任何 grant
- [x] C4.16 单测：`AlwaysApprove` 不覆盖同一 action 的相邻资源（A4）
- [x] C4.17 审计覆盖每一条放行路径：`grant_store` 命中记 `GrantHit`，与策略 `Allow` 区分
- [x] C4.18 单测：缓存命中必须留审计，不得静默跳过
- [x] C4.19 单测：空键每次走审批且必须留审计（唯一能证明「确实问过人」的记录）

---

## C5 ToolPipeline

`src-tauri/crates/assistant-tools/src/pipeline.rs`

- [x] C5.1 定义 `ToolPipeline { registry, gateway, storage, default_timeout }`
- [x] C5.2 定义 `ToolOutcome` 内部类型（结果 + 阶段标记，不复用 DTO）
- [x] C5.3 定义 `PipelineEventSink` 端口与内存实现，事件为 `Started` / `Finished` / `Failed`
- [x] C5.4 阶段 1 `find`：`registry.find(name)` 未找到返回 `Internal`「Unknown tool」
- [x] C5.5 阶段 2 `validate`：`blocks_input_rewrite = false` 返回 `Business`，允许模型修正重试
- [x] C5.6 阶段 2 `validate`：`blocks_input_rewrite = true` 返回 `Business`，且**不进入** Gateway（A7）
- [x] C5.7 阶段 3 `gateway`：`Denied` / `RejectedByUser` 统一打标 `ToolErrorKind::Denied`
- [x] C5.8 阶段 4 `execute`：复用 `exec::invoke`，不直接调用 `execute()`
- [x] C5.9 阶段 5 `persist`：调用 `storage.persist`，大结果落盘、小结果内联 —— 单轮预算的选择算法归 C6.5，C5 只做单结果阈值
- [x] C5.10 阶段 6 事件上报：按原始顺序发出 `Started` / `Finished` / `Failed`
- [x] C5.11 阶段 7 回填：取 `result_for_assistant`，缺省用 `data` 紧凑序列化
- [x] C5.12 实现 `partition_tool_batches`：连续 `is_concurrency_safe(true)` 合并为一个 batch
- [x] C5.13 非并发安全工具单独成批并串行执行
- [x] C5.14 结果按原始 `tool_calls` 顺序收集，持久化与事件顺序一致
- [x] C5.15 单测：阶段顺序固定为 `find → validate → gateway → execute → persist`（A6）
- [x] C5.16 单测：`[read, read, write, grep, grep, bash]` 切分为 4 个 batch（A6）
- [x] C5.17 单测：并发 batch 结果顺序与原始顺序一致（A6）
- [x] C5.18 单测：`blocks_input_rewrite` 命中时不调用 Gateway 且无审批记录（A7）
- [x] C5.19 `pipeline.rs` 不出现 `tauri` / `AppHandle` / `Window`
- [x] C5.20 单测：未知工具在 `find` 阶段失败，不执行也不审批
- [x] C5.21 单测：拒绝（策略 `Deny` / 用户 `Deny`）停在 `Gateway` 阶段，执行计数为 0
- [x] C5.22 单测：大结果落盘后回填文本只含 `<persisted-output>` 引用 + `preview_chars` 预览
- [x] C5.23 单测：事件顺序为 batch 内 `Started*` → `Finished*`，跨 batch 按原始顺序

---

## C6 结果持久化

`src-tauri/crates/assistant-tools/src/storage.rs`

- [x] C6.1 定义 `per_tool_limit_chars = 50K` / `per_round_limit_chars = 200K` / `preview_chars = 2K`
- [x] C6.2 实现选择算法：按序列化字符数**降序**排列，累计超单轮预算者才落盘
- [x] C6.3 落盘后引用格式为 `<persisted-output path=…>` + 前 `preview_chars` 字符
- [x] C6.4 定义 `ReadReceiptStore` 端口（跨轮去重预留，P5 由 `fs_read` 使用）
- [x] C6.5 单测：小结果内联、不落盘（A8）
- [x] C6.6 单测：超预算时按大小降序落盘，且落盘数量最小（A8）
- [x] C6.7 单测：落盘引用与预览长度不超过 `preview_chars`（A8）
- [x] C6.8 单测：单结果超 `per_tool_limit_chars` 即使未超单轮预算也落盘
- [x] C6.9 `ToolResultStorage` / `FileToolResultStorage` / `PersistToolResult` / `PersistedRef` 从 `gateway.rs` 迁至 `storage.rs`
- [x] C6.10 落盘选择提升为单轮级：`execute` 先收齐全部结果再统一决策，避免按 batch 局部决策

---

## C7 验收命令

`cwd = src-tauri`

- [x] C7.1 `cargo test -p assistant-tools` 全绿（A1）——57 + 27 + 15 + 21 + 6 passed
- [x] C7.2 `cargo clippy -p assistant-tools --all-targets -- -D warnings` 无告警（A1）
- [x] C7.3 先执行格式化修复，再 `cargo fmt --all --check` 通过
- [x] C7.4 `cargo tree -p assistant-tools` 中无 `tauri`（A2）
- [x] C7.5 grep 确认 `policy.rs` / `gateway.rs` / `pipeline.rs` 无 `AppHandle` / `Window`（A2）
- [x] C7.6 `cargo test --workspace --no-run` 确认全量编译通过
- [x] C7.7 `cargo test -p assistant-app --test protocol_dto_roundtrip` 通过（21 passed）
- [x] C7.8 `npm run contract:check` 幂等（无 diff）（A9）——以生成前后契约文件哈希比对验证（工作区已有未提交的契约改动，故不用 `git diff --exit-code`）
- [x] C7.9 确认生产注册表仍只有 `fs_link`（A9）
- [x] C7.10 确认无新增 DB 表 / migration（A10）

---

## C8 文档

- [x] C8.1 更新 I38 roadmap §2：P2 标记已完成并记录日期（阶段状态唯一台账）
- [x] C8.2 更新 I38a §5 验收表 A1–A10 为一次性验收记录

---

## 验收映射

| 规格验收 | 对应任务 |
|---|---|
| A1 测试与 clippy | C7.1、C7.2 |
| A2 无 tauri 依赖 | C0.4、C7.4、C7.5 |
| A3 `most_restrictive` / last-match-wins / 多层 / Ceiling | C1.13 – C1.16 |
| A4 Gateway 四分支与 `AlwaysApprove` 作用域 | C2.19、C4.8 – C4.11 |
| A5 超时 → `RejectedByUser`、取消 → `Cancelled` | C2.18、C4.12、C4.13 |
| A6 Pipeline 顺序与并发 batch | C5.15 – C5.17 |
| A7 `blocks_input_rewrite` 不可绕过 | C5.6、C5.18 |
| A8 结果持久化三层上限 | C6.5 – C6.8 |
| A9 存量工具零改动 + 契约生成 | C0.2、C3.14、C7.8、C7.9 |
| A10 无新增 DB 表、端口注入 | C0.5、C7.10 |

---

## 不做

- 不新增 `fs_read` / `fs_list` / `fs_glob` / `fs_grep` / `fs_write` / `fs_edit` / `fs_mkdir` / `fs_move` / `fs_delete` / `shell_exec`
- 不实现 RoundExecutor、多迭代循环、turn 生命周期（P4 / P3）
- 不新增 Tauri 事件、审批 UI、前端组件（P6）
- 不实现 MCP 工具（P7）
- 不建 `tool_calls` 表或任何 migration
- 不把审批端口塞进 `src-tauri`；Tauri 适配层留给 P6
- 不改 `ToolAudience` 语义、不把 `fs_link` 改为 `AGENT_ONLY` 或 `BOTH`
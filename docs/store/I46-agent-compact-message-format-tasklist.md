# I46 — 工具结果紧凑信封 任务清单（P13）

> 状态：✅ 已完成（2026-11-14 关闭；C1–C10 全部勾选，门禁 1–15 有测试映射，全量验收记录见 C10）
> 规格：[I46-agent-compact-message-format-design.md](./I46-agent-compact-message-format-design.md)（参考记录，信封 / 省略 / 落盘 / 预算与门禁 1–15 仍有效）
> 阶段登记：[I38 §2](./I38-agent-roadmap.md) P13
> 前置：P2 工具控制层、P5 OS 工具能力（均已完成）。本阶段关闭 I46 节点布局 §1.2 的 P1 开工门。

十个批次按依赖排序：C1 信封核心 → C2 Pipeline 与事件 → C3 DTO 契约 → C4 历史重建 → C5 落盘布局与清理 → C6 采集策略与截断三态 → C7 前端消费 → C8 渲染选路 → C9 密度折叠 → C10 验收。C5 与 C6 可在 C1 后与 C2–C4 并行；C7/C8/C9 依赖 C3 的生成类型。

> 本清单不管理提交。提交粒度按代码依赖与可编译性现场决定；这里只回答「代码改没改、验收跑没跑」。

**门禁编号速查**（对应规格 §3.5）：1 字节相等 / 2 head 单行 / 3 三态一致 / 4 round-trip / 5 一次传输 / 6 事件 / 7 口径分离 / 8 定位符非解析（写入路径）/ 9 分类可回放 / 10 清理安全 / 11 锚点结果时刻 / 12 形态不越界 / 13 选路不看身份 / 14 折叠不改数据 / 15 安全阀。

---

## 阶段 C1 — 信封核心（已完成 2026-11-14）

新增 `src-tauri/crates/assistant-tools/src/compact.rs`，作为信封的唯一拼装点（规格 §3.6「后端」条）。

### C1.1 数据结构与常量

对应规格 §2.1 / §2.2 / 门禁 2、3。

- [x] `lib.rs` 注册 `pub mod compact;`
- [x] `ToolResultCompact { head, truncated, persisted, error_kind, anchor }`（`error_kind` 复用 `ToolErrorKind`，不新增分类枚举）
- [x] `TruncatedInfo { unit, returned, total }` + `TruncatedUnit`（`Lines` / `Items` / `Bytes` / `Unknown`）
- [x] `PersistedInfo { path, bytes }`、`DocAnchor { path, ranges }`、`LineRange { start, end }`
- [x] 常量 `HEAD_MAX_CHARS = 160` / `SUBJECT_MAX_CHARS = 80` / `ARGS_PREVIEW_MAX_CHARS = 200` / `PREVIEW_VALUE_MAX_CHARS = 60`

### C1.2 渲染与重建

对应规格 §2.3 / §3.4 / 门禁 1、4、7、8、9、11。

- [x] `build_compact(tool, arguments, result, persisted)`：`persisted` 只接受 `Option<&PersistedRef>`，签名上禁止文本解析（门禁 8）
- [x] `rebuild_compact(tool, arguments, result_text, is_error)`：历史读取回退，允许解析 `<persisted-output>` 标签
- [x] `build_head`：成功 `ok <tool> · <subject> · <scale>`；失败 `err <kind> <tool> · <subject> · <reason>`
- [x] `subject` 优先取结果事实 `data`，失败时回退参数；`edit` / `write` / `read` 带行区间
- [x] `scale` 按工具取规模：`read` 行数 + 字节、`write` 行数 + 状态、`edit` 替换数、`grep` 命中 + 文件、`glob` 路径数、`list` 条目 + 深度、`delete` 条目 + 字节
- [x] head 单行化：错误文案只取首行；总长按 `HEAD_MAX_CHARS` 尾部截断（门禁 2）
- [x] `truncate_middle`：subject 超长走中间省略（首 40 + `…` + 尾 38）
- [x] `parse_truncation`：`[truncated: showing N of M lines|items|bytes]` → 三态；无提示即 `None`（门禁 3）
- [x] `parse_persisted_tag` / `attribute`：`<persisted-output path="…" bytes="…">` 读取回退
- [x] `error_kind_from_text`：按 `tool '<name>': <reason>` 前缀还原 `Denied` / `Timeout` / `Internal`，其余 `Business`（门禁 9）
- [x] `build_anchor`：仅 `edit` / `write` / `read` 出行区间；`edit` 取 `startLine` + `newEndLine`，不由 `args` 估算（门禁 11）

### C1.3 `argsPreview`

对应规格 §2.5 / §6 OQ1（**已定稿：按工具显式声明 `preview_keys`，不读 JSON Schema**）。

- [x] `preview_keys(tool)`：十个工具各自的白名单键，未知工具走通用键集合
- [x] `render_value`：字符串按 60 字符截断，数组 → `[N items]`，对象 → `{N keys}`，`null` 跳过
- [x] 总长 ≤ 200 字符，无可用键返回 `None`（前端据此回退 `ArgsDelta`）
- [x] 单元测试 21 项（`compact::tests`）：head 形状 / 失败分类 / 单行约束 / 长度上限 / 三态 / 落盘 round-trip / 锚点行区间（含结果时刻与不回退参数）/ 多文件无锚点 / 预览有界 / 分类回放 / 历史重建 / 全组合 round-trip / 口径分离

### C1.4 自验

- [x] `cargo test -p assistant-tools` 全绿（`compact::tests` 21 项）
- [x] `cargo clippy -p assistant-tools --all-targets -- -D warnings` 通过
- [x] `cargo fmt --check` 通过

**C1 已知取舍**：

- `rebuild_compact` 不产出 `anchor`（`anchor` 需 `data`，历史路径无 `data`）；文档形态对旧数据按 `anchor == null` 走卡片，符合规格 §3.6「旧数据」条。
- `persisted.bytes` 对旧数据只走文本标签回退，`bytes` 缺失时为 `null`（规格 §3.6 明确「不承诺」）。

---

## 阶段 C2 — Pipeline 与事件（已完成 2026-11-14）

对应规格 §3.6「落盘」「契约」两条，门禁 5、6、8。

### C2.1 `assistant-tools`

- [x] `pipeline.rs` `ToolOutcome` 追加 `arguments: Value`（head subject 与预览的来源）与 `compact: Option<ToolResultCompact>`
- [x] `ToolOutcome::new` 从 `ToolCall` 复制 `arguments`，`compact` 初始 `None`
- [x] `PipelineEvent::Started` 追加 `args_preview`，由 `args_preview(tool, arguments)` 填充
- [x] `PipelineEvent::Finished` / `Failed` 追加 `compact`
- [x] 批处理末尾按 `persist → compact → report` 顺序生成信封（落盘先于信封，`persisted` 才能进 head 之外的结构字段）
- [x] `report` 转发 `compact`，不再只发 `assistant_text`

### C2.2 `assistant-agent` 中性信封

- [x] `agent/tool_event.rs` `ToolEventPayload::Started` 加 `args_preview`（原单元变体改为结构变体）
- [x] `Finished` / `Failed` 加 `compact: Option<ToolCompact>`
- [x] 新增 `ToolCompact` / `CompactTruncated` / `CompactUnit` / `CompactPersisted` / `CompactAnchor` / `CompactLineRange`（内核侧不依赖 `assistant-tools` 具体类型）

### C2.3 适配层

- [x] `adapters/tool_exec.rs` `ObserverPipelineSink::tool_event` 搬运 `args_preview` 与 `compact`
- [x] `into_compact`：只搬字段，不做判定；`into_unit` 镜像 `TruncatedUnit`
- [x] `into_error_class` 复用既有端口镜像，`compact.error_kind` 与载荷 `error_kind` 同源

### C2.4 自验

- [x] `cargo check --workspace --all-targets` 通过
- [x] `cargo test -p assistant-agent` 全绿（事件序列化不变式未回归）
- [x] `cargo clippy --workspace --all-targets -- -D warnings` 通过

---

## 阶段 C3 — DTO 契约（已完成 2026-11-14）

对应规格 §5「契约」，门禁 12。

### C3.1 新增 DTO

- [x] `protocol/dto/agent.rs` 新增 `ToolResultCompactDto`（`head` / `truncated` / `persisted` / `errorKind` / `anchor`），输出 `Option` 不加 `skip_serializing_if`
- [x] 新增 `TruncatedDto` / `TruncatedUnitDto` / `PersistedResultDto` / `DocAnchorDto` / `DocLineRangeDto`
- [x] `ToolErrorKindDto` 复用现有 `os_tool.rs` 枚举，不新增第二套错误词汇
- [x] `AgentToolCallPayloadDto`：`Started` 加 `argsPreview`（`#[ts(optional = nullable)]`），`Finished` / `Failed` 加 `compact`（键恒在、值 `null`）
- [x] `protocol/dto/chat.rs` `MessageToolCallDto` 加 `compact`
- [x] `From<ToolCompact> for ToolResultCompactDto`（适配层唯一转换点）

### C3.2 生成物

- [x] `npm run contract:gen`，提交 `src/types/protocol.contract.ts` 与 `src/vault/core/contract/vault.contract.ts`
- [x] `npm run contract:check` 幂等（`protocol_bindings.rs` 已注册 6 个新 DTO）

---

## 阶段 C4 — 历史重建（已完成 2026-11-14）

对应规格 §3.4 三条路径、§3.6「旧数据」，门禁 1、5、9。

- [x] `cmd/conversation_utils.rs` 组装 `MessageToolCallDto` 时调用 `rebuild_compact`（工具名 + 参数 + 结果文本 + 状态）
- [x] tool 行 `content` 清空为 `""`，正文只留在 assistant 行 `toolCalls[].result`（门禁 5）
- [x] `Finished.result` / 落库 tool 行 `content` / 历史 `result` 三者字节相等（门禁 1）——写入侧由 `pipeline` 测试断言 `Finished.result == assistant_text`，读取侧由 `conversation_utils` 测试断言历史 `result` 与落库正文一致
- [x] 旧数据无 `compact` 时仍能重建；无法重建则只渲染 `result`（`result` 缺失时 `compact` 一并缺失）
- [x] `db.rs` `tool_calls_json` 注释修正（F15：已非「恒 NULL」）
- [x] 测试：历史重载后 `compact.errorKind` 与写入时一致（门禁 9）；`adapters/event_sink.rs` 全量下发统一走 `wire_message_dto`

---

## 阶段 C5 — 落盘布局与清理（已完成 2026-11-14）

对应规格 §2.9 / §6 OQ5–OQ7（**已定稿：默认保留 30 天、可配、`0` 禁用；只删文件不修剪空目录；旧布局不迁移、允许定位符失效**），门禁 10。

- [x] `storage.rs` `FileToolResultStorage` 目录改为会话命名空间（会话哈希 FNV-1a 64）+ 随机前缀
- [x] 启动时按 `mtime` 年龄清理：只删常规文件、不跟随符号链接、失败不抛出、`0` 完全禁用（`tool_setup::spawn_result_cleanup` 独立线程执行，失败只 warn）
- [x] 保留天数进配置（`assistant-config` + `*ConfDto` 同步，Rust 侧 `#[serde(default = "default_result_retention_days")]`）
- [x] 旧目录 `tool-results/<call_id>.txt` 不迁移；读取失败即视为定位符失效（无迁移代码即满足）
- [x] 测试：超期删除 / 未超期保留 / 符号链接不跟随 / `0` 禁用 / 清理失败不影响启动（`tests/storage.rs` 7 项 + `storage.rs` 7 项单测）

---

## 阶段 C6 — 采集策略与截断三态（已完成 2026-11-14）

对应规格 §2.7 D7 / §2.8 D8，门禁 3、7。

- [x] `os/output.rs` `OutputLimits` 加 `strategy`（`Head` / `Tail` / `HeadTail`），默认 `Head` 保持现有行为；提供 `head` / `tail` / `head_tail` 三个构造函数
- [x] `truncate_output` 按策略选保留区间，UTF-8 边界裁切；字节预算一律向内贴行边界（绝不超预算）；`HeadTail` 省略区间不含整行时退化为纯 head，且不写中段标记（避免把半行谎报成「省略 N 行」）
- [x] `truncation_notice` 两分支收敛为三态：精确（`showing N of M`）/ 未知（显式 `total unknown`）/ 无省略；`parse_truncation` 与之严格双向等价
- [x] 策略归属按规格 §2.7：`read`（行窗口）/ `grep` / `glob` / `list` 保持 `head`（现状即此，行为不变）；`tail` / `headTail` 作为工具声明能力已就绪，当前无命令执行类工具，故不切换任何现有调用方（原「`read` 用 `headTail`」与规格 §2.7 D7 冲突，**以规格为准**）
- [x] `incomplete` / `skipped` / `decoding_failed` 只进 body 或 head 的 `scale`，不进 `[truncated:`（门禁 7）
- [x] 测试：三种策略 × 三态组合（`os/output.rs` 15 项单测；`read` / `grep` / `glob` 提示断言同步为三态）

---

## 阶段 C7 — 前端消费（已完成 2026-11-14）

对应规格 §3.6「前端」，门禁 12。

- [x] `src/chat/core/toolCallView.ts` `ToolCallView` 加 `compact`（来自 `Finished` / `Failed` 载荷与历史 `MessageToolCallDto`）
- [x] `started` 事件用 `argsPreview` 初始化参数；`ArgsDelta` 保留为受控回退（无预览时使用）
- [x] `src/chat/tool/ToolCallBubble.tsx` 优先读 `view.compact`：`head` 作标题、`truncated` 作规模、`persisted` 作「查看全文」入口
- [x] `toolCallDisplay.ts` `parsePersistedOutput` / `resultStats` 降级为 `compact == null` 的兜底（新增唯一投影函数 `resolveResultDisplay`）
- [x] `MessageList` 的 `role !== "tool"` 过滤降为防御性过滤（正文已在 assistant 行）
- [x] 测试：`compact` 存在时不触达文本解析（`resolveResultDisplay` 断言 `source === "compact"`，且信封无 `persisted` 时正文原样保留）

---

## 阶段 C8 — 渲染选路（已完成 2026-11-14）

对应规格 §3.7 / §2.11 D11，门禁 12、13。

- [x] `src/chat/tool/ToolCallRenderer.tsx` 选路：`compact?.anchor != null && anchor.path === docPanelPath` → `ToolCallDocStub`，否则 → `ToolCallBubble`（判据抽为纯函数 `shouldRenderDocStub`）
- [x] 新增 `ToolCallDocStub`（单行引用：`compact.head` + 行号跳转），与 `ToolCallBubble` 并列
- [x] 选路不读 `role` / `modeId` / `TurnKind` / 工具名（门禁 13）
- [x] `docPanelPath` 由 `AgentLayoutSlots` 注入（`AgentLayout` → `ConversationFrame` → `MessageList` → `TurnGroup` / `MessageBubble` → `ToolCallRenderer`），`ChatLayout` 不传即 `null`（chat 链路恒走卡片）
- [x] 测试：同一 assistant 行内一个调用有锚点、一个没有，分别走批注引用与卡片；面板开关两态（`shouldRenderDocStub` 6 项断言）

---

## 阶段 C9 — 密度折叠（已完成 2026-11-14）

对应规格 §2.12 D12 / §3.8，门禁 14、15。

- [x] `src/chat/message/groupNodeRows.ts` 已有分组逻辑补齐 `compact == null` 退化：`hasStepSummary()` 优先 `compact.head`，缺失退回 `summarizeArguments`；两者皆无则该组不折叠（宁可高，不可空）
- [x] 折叠判据只读段数与 `status`，不读 `modeId`（门禁 14）；`link` 仍只写入视图
- [x] `error` / `awaiting_user` 步、末段、流式中的 run 不折叠；组头 `failed` 计数即待处理数（门禁 15）
- [x] 折叠是渲染态：`groupNodeRows` 只投影 `NodeView`，不改 `Message`、不改落库、不产生 wire 字段；展开后消息 / `id` 逐项还原（门禁 14）
- [x] 测试：流式中不折叠 / `compact == null` 退化仍折叠 / `compact` 与参数都缺失不折叠 / `head` 空白视同缺失 / 展开后节点逐项一致

---

## 阶段 C10 — 验收

- [x] 门禁 1–15 逐条有对应测试，且测试名可映射到门禁编号（映射表见本节末）
- [x] `cargo test --workspace` 全绿
- [x] `cargo clippy --workspace --all-targets -- -D warnings` 与 `cargo fmt --check` 通过
- [x] `npm run contract:gen` / `contract:check` 幂等
  - 2026-11-14 行为变更：`contract:check` 原先混了两件事（生成幂等 + 生成物已提交），末段 `git diff --exit-code` 比对「工作区 vs 索引」，因此只要 DTO 改了未提交就必挂，开发期无法使用（I45 tasklist 曾手工绕过）。
  - 现在拆分为：`contract:check` = `scripts/contract-check.mjs`，取当前生成物哈希 → 跑**一次** `contract:gen` → 再比对，判据是「生成物与源码同步」，**与 git 状态无关**，本地随时可跑；`contract:verify` = 原语义（gen + `git ls-files --error-unmatch` + `git diff --exit-code`），仅用于提交后/CI 的同步门禁。
  - 实测：正向 `contract:check` exit=0（4 个生成物与源码一致）；负向在 `protocol.contract.ts` 尾部追加探针后 exit=1，精确报出文件与前后哈希，`contract:gen` 自动覆盖回正确内容；`contract:verify` 在未提交生成物上按预期 exit=1。
- [x] `npm run lint` / `npm run typecheck` / `npm run vitest` 通过
- [x] 全量验收命令与结果记入本节（含日期）
- [x] 关闭 I46 节点布局 §1.2 的 P1 开工门：在 I46 spec §1.2 标注 `compact` 已可用

### 门禁 → 测试映射

| 门禁 | 测试 | 位置 |
|---|---|---|
| 1 字节相等 | `finished_result_matches_tool_row_text`；`history_rebuilds_compact_and_keeps_result_bytes` | `src-tauri/crates/assistant-tools/tests/pipeline.rs`；`src-tauri/src/cmd/conversation_utils.rs` |
| 2 head 单行 | `head_is_single_line_even_when_error_has_multiple_lines`；`head_is_capped_at_the_budget`；`rebuild_from_history_keeps_head_shape_and_kind`（无 `\n` 且 ≤ 160） | `src-tauri/crates/assistant-tools/src/compact.rs` |
| 3 三态一致 | `truncation_notice_maps_to_lines_unit`；`explicit_unknown_notice_drops_half_known_numbers`；`unknown_unit_never_carries_numbers`；`absent_truncation_notice_means_null`；`every_strategy_reports_exact_totals_when_truncated`；`every_strategy_reports_no_omission_when_nothing_is_dropped` | `src-tauri/crates/assistant-tools/src/compact.rs`；`src-tauri/crates/assistant-tools/src/os/output.rs` |
| 4 round-trip | `envelope_matches_text_reparse_for_every_combination`（三态 × 落盘/不落盘 × 成败）；`rendered_truncation_notice_round_trips_across_strategies`；`persisted_tag_round_trips`；`rebuild_from_history_keeps_head_shape_and_kind` | `src-tauri/crates/assistant-tools/src/compact.rs` |
| 5 一次传输 | `tool_row_content_is_empty_on_wire`；`finished_result_matches_tool_row_text` | `src-tauri/src/cmd/conversation_utils.rs`；`src-tauri/crates/assistant-tools/tests/pipeline.rs` |
| 6 事件 | `approved_call_runs_and_reports_events_in_order`；`failed_call_reports_failed_event`；`seq_is_strictly_monotonic_within_a_turn_and_terminated_reuses_last_seq` | `src-tauri/crates/assistant-tools/tests/pipeline.rs`；`src-tauri/crates/assistant-agent/tests/kernel_emit.rs` |
| 7 口径分离 | `skipped_and_incomplete_wording_is_not_a_truncation_notice`；`explicit_unknown_notice_drops_half_known_numbers` | `src-tauri/crates/assistant-tools/src/compact.rs` |
| 8 定位符非解析（写入路径） | `persisted_comes_from_the_reference_not_from_text`；`large_result_is_persisted_with_preview` | `src-tauri/crates/assistant-tools/src/compact.rs`；`src-tauri/crates/assistant-tools/tests/pipeline.rs` |
| 9 分类可回放 | `error_kind_is_replayable_from_text`；`history_rebuild_keeps_error_kind_from_text`；`rebuild_from_history_keeps_head_shape_and_kind` | `src-tauri/crates/assistant-tools/src/compact.rs`；`src-tauri/src/cmd/conversation_utils.rs` |
| 10 清理安全 | `cleanup_removes_expired_files_and_keeps_fresh_ones`；`cleanup_disabled_when_retention_is_zero`；`cleanup_sweeps_namespace_directories_and_keeps_empty_dirs`；`cleanup_counts_failures_without_panicking_on_missing_root`；`cleanup_does_not_follow_symlinks` | `src-tauri/crates/assistant-tools/tests/storage.rs` |
| 11 锚点结果时刻 | `edit_anchor_uses_result_time_lines`（参数里塞过期行号，锚点仍取结果行）；`anchor_is_absent_when_result_has_no_line_range` | `src-tauri/crates/assistant-tools/src/compact.rs` |
| 12 形态不越界 | `tool_result_compact_keeps_a_single_envelope_field_set`；`resolveResultDisplay` 组（只读 `compact`，`persisted` 取自信封） | `src-tauri/tests/protocol_dto_roundtrip.rs`；`src/chat/tool/__tests__/toolCallDisplay.test.ts` |
| 13 选路不看身份 | `ToolCallRenderer 不读 role / modeId / TurnKind / 工具名分支`；`同一消息内按各自锚点独立选路` | `src/chat/message/__tests__/nodeLayoutGates.test.ts`；`src/chat/tool/__tests__/toolCallDisplay.test.ts` |
| 14 折叠不改数据 | `展开后节点与折叠前逐项一致（不改数据，只改密度）`；`折叠判据只读段数与 status，不读 modeId（compact 门禁 14）` | `src/chat/message/__tests__/groupNodeRows.test.ts`；`src/chat/message/__tests__/nodeLayoutGates.test.ts` |
| 15 安全阀 | `失败步切断 run，残余段不足阈值时不折叠（安全阀）`；`失败步留在气泡内，不被同条消息的折叠步吞掉`；`单节点错误不阻止其他节点折叠（不再整组失效）`；`待处理步同样切断 run`；`无摘要步切断 run`；`流式增量期间不产生 run，组头计数不随全局更新跳动`；`含边界节点时不折叠` | `src/chat/message/__tests__/groupNodeRows.test.ts` |

### 全量验收记录（2026-11-14）

| 命令 | 结果 |
|---|---|
| `cargo test --workspace`（cwd `src-tauri`） | 全绿，0 failed；`assistant-tools` lib 144 passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit=0，无 lint 告警（仅增量编译目录清理的 cargo 级 warning） |
| `cargo fmt --check` | exit=0 |
| `npm run typecheck` | exit=0 |
| `npm run lint` | exit=0（prettier 全部 unchanged，eslint 0 warning） |
| `npx vitest run` | 51 files / 608 passed，1 skipped |
| `npm run contract:check` | exit=0：「4 个生成物与源码一致」 |

---

## 阻塞性未决问题（本阶段开工前须定稿）

规格 §6 的 13 条 OQ，其中影响本阶段落地的四条已定稿如下（其余按「倾向」执行，不阻塞）：

| OQ | 结论 |
|---|---|
| OQ1 `argsPreview` 抽取口径 | 按工具显式声明 `preview_keys`（C1.3） |
| OQ4 `edit` diff 保真度 | 保持调用时刻 diff，不升级为 applied-hunk；锚点单独走 `compact.anchor`（C1.2） |
| OQ5 落盘保留天数 | 默认 30 天，可配，`0` 禁用（C5） |
| OQ10 锚点多文件 | 保持单值 `DocAnchorDto`，多文件工具（`grep` / `glob` / `list`）不产出锚点、走卡片（C1.2 测试 `multi_file_tools_have_no_anchor`） |
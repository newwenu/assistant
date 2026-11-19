# I57 — P16：`read` 长行如实报告与扫描收口 任务清单

> 规格：[I57-long-line-read-spec.md](./I57-long-line-read-spec.md)（现状 §1 / 契约 §3 / 行为 §4 / 失败矩阵 §5 / 文案 §6 / 验收 §8 / 门禁 §10 / 切片 §11）
> 事实基线：I57 §1 的 N1–N16（2026-11-18 已按代码逐条核对，见下方「前置核对」）
> 前置：P5（OS 工具能力，`read` 分块扫描已就绪）、P13（落盘与预览口径）、P2（工具控制层）均已完成；本阶段不依赖 P17（I38 §3 的依赖是 P17 → P16 单向）
> 阶段台账：[I38-agent-roadmap.md](../../store/I38-agent-roadmap.md) §2 阶段表（唯一状态来源）

**方案：B（只做报告面，不做行内翻页）**。`column` / `maxLineChars` / `lineWindow` / `linesBeyondColumn` / `[column]` 已从本阶段整体移出，由 [I58](./I58-super-tool-spec.md) 的 `super_read` 承接（I57 §0.2 例外登记、§12 R10）。本清单只覆盖 `truncatedLines` 报告、整行计费、字节预算、回执与 `edit` 闸门。

批次对应规格章节：**T1 扫描核心 sink 化 + 宽限改字节口径**（§0.3 P1 / §3.2 预算表 / §10 G1·G12）→ **T2 整行计费与窗口未满停止**（§4.1 第 3·4·5 条 / §5 F1–F4 / §10 G2–G5）→ **T3 报告面**（§3.3 / §3.4 / §6.1 / §6.2 / §10 G6·G7）→ **T4 回执与 `edit` 闸门**（§3.5 / §3.6 / §6.3 / §10 G8）。

依赖：T1 无前置；T2 依赖 T1（整行计费挂在 sink 与 `Scanner` 的计量分离上）；T3 依赖 T2（报告的是最终计数与「预算耗尽」事实）；T4 依赖 T3（回执写的是 `data.truncatedLines`）。**全程不改 wire、不改 DTO、不改前端**（§10 G9）。

> 本清单不管理提交。提交粒度按代码依赖与可编译性现场决定；这里只回答「代码改没改、验收跑没跑」。
> 勾选表示代码已改且对应验收已跑。

---

## 前置核对（2026-11-18，按代码）

| 事实 | 结论 | 落点 |
|---|---|---|
| N1：单行上限是常量 `MAX_LINE_CHARS = 2000`，`append` 到上限即停并追加 `" [truncated]"` | **不改**。B 方案下它是硬上限，不做成可调参数（§12 R2）；`LINE_TRUNCATED_MARKER` 字面量不动（`edit` 档 2 / 档 4 依赖它） | `src-tauri/crates/assistant-tools/src/os/tools/agent_read/mod.rs` |
| N2 / N3：超限部分仍解码后丢弃，且**不提前提交**（必须等 `\n` 或 EOF） | **不提前提交**（§12 R3）。T2 只把「被丢弃的尾部」计入 `current_line_bytes` | `.../agent_read/window.rs` |
| N4：宽限是**字符**口径 `EOF_GRACE_CHARS = 1 MiB`，只在 `stopped` 之后累加 | T1 换口径（字符 → 字节），**不改停止条件**；真正的「窗口未满也停」在 T2 | 同上 |
| N5：`rendered_lines > 0 && rendered_bytes + line_bytes > byte_cap` 才 `stopped` | 根因是 `line_bytes` 由**可见段**组成（4 MiB 的行在预算里只值 2000 字符）。T2 改为整行计费并补停止分支 | 同上 `flush_line` |
| N6：`total_rendered_bytes` 已按字节累计，行号前缀用 `prefix_bytes(number)` 估算 | 口径不变；T2 新增 `line_budget_bytes` 是**另一套**累加器，两者不混用 | 同上 |
| N7：截断提示两态（`Some(total)` / `None`） | T3 扩到四态 S1–S4，**S1 / S2 文案逐字节不变** | `agent_read/mod.rs::truncation_notice` |
| N8：`data` 恒有 `path` / `startLine` / `endLine` / `totalLines` / `truncated` | T3 只**追加** `truncatedLines` 一项，既有五项键与类型不动 | `agent_read/mod.rs::execute` |
| N9：footer 只有 `[encoding: …]`；`FOOTER_TAGS` 现为 `["encoding", "non-utf8", "catalog"]` | T3 只追加 `long-lines`（`catalog` 由 I58 T1 追加，B 只追加） | `agent_read/mod.rs::encoding_footer`、`agent_edit/report.rs` |
| N10：`ReadReceipt` 只有四字段；`ReadReceipt::utf8(..)` 是既有构造器 | T4 追加两字段并给构造器补默认值（`write` 等既有调用点零改动） | `src-tauri/crates/assistant-tools/src/runtime/storage.rs` |
| N11：`edit` 只校验「读过」+「revision 未变」 | T4 的闸门插在 `revision` 校验之后、`apply::apply_edit` 之前，不重扫文件 | `agent_edit/mod.rs` |
| N12 / N13：`edit` 匹配整文件、整文件上限 16 MiB | 本阶段**不动** `apply.rs`；闸门只做拒绝，不提供局部替换 | `agent_edit/apply.rs` |
| N14：`grep` 无单行上限 | 不动；闸门文案建议「先 `grep` 定位再按行读」（§6.3） | `agent_grep/` |
| N16：现有三条截断测试（长行 / 行数上限 / 字节上限） | 是 G1 的基线；`long_lines_are_truncated_with_a_marker` 的 `assistant_text` 断言**会因新增 `[long-lines]` 而需要更新**（唯一允许修改的既有断言，T3 同步改），其余断言不改 | `src-tauri/crates/assistant-tools/tests/agent_read.rs` |
| `request_tool` 已在只读组注册（I58 T1） | 与本阶段无关；`registration.rs` 的 id 列表已含它，T1 起跑时不要回改 | `src-tauri/crates/assistant-tools/tests/registration.rs` |
| `column` / `maxLineChars` 已从 `read` 撤出 | 2026-11-18 回退完毕：schema 四项、`ReadRequest` 三字段、`WindowRequest` 三字段；测试新增**负向**断言锁定（schema 不得出现这两键） | `agent_read/{mod.rs,window.rs}`、`tests/agent_read.rs` |

---

## T1 — 扫描核心 sink 化 + 宽限改字节口径（I57 §0.3 P1 / §3.2 / §10 G1·G12）· 已完成

退出条件：G1 / G12 通过——既有 `agent_read` 测试全通过（除 T3 才改的那一条 `assistant_text` 断言），输出逐字节不变；`LineSink` 成为唯一渲染口；`EOF_GRACE_CHARS` 已换成 `EOF_GRACE_BYTES` 且**停止条件未变**。

- [x] `agent_read/window.rs`：抽出 `LineSink`（`emit(number, visible, truncated, line_bytes)`），`Scanner` 只负责计量与预算，**不再自己拼字符串**
- [x] `agent_read/window.rs`：`LineSink` 的默认实现复刻现有渲染（`{number:>6}\t{visible}` + 截断标记），保证 T1 阶段输出零变化
- [x] `agent_read/window.rs`：`scan` 接受 `&mut dyn LineSink`（或内部构造默认 sink），`WindowOutcome` 从 sink 取正文与 `rendered_lines` / `rendered_bytes`
- [x] `agent_read/window.rs`：`LineSink::reset()` 为**必实现**方法——sink 在扫描外持有，编码回退重扫会复用它，不清空会残留首轮脏行（§0.3 P1）
- [x] `agent_read/window.rs`：`EOF_GRACE_CHARS`（字符）**删除**，换 `EOF_GRACE_BYTES = 8 * 1024 * 1024`；本切片只换计量单位与常量名，**停止条件不变**；该常量仅在 `window.rs` 内部使用（无 crate 外引用），删除不外溢
- [x] `agent_read/window.rs`：`flush_line` 里的宽限累加改为「解码后字节」（`grace_bytes`），语义与旧 `grace_chars` 一一对应
- [x] `agent_read/window.rs`：`CHUNK_BYTES` / `DEFAULT_MAX_BYTES` / `DEFAULT_MAX_LINES` 不动；`prefix_bytes` 估算不动
- [x] `tests/agent_read.rs`：G1 回归——现有 `agent_read` 测试全通过，三条截断测试的断言**不改**（`[long-lines]` 在 T3 才出现）
- [x] `tests/agent_read.rs`：G12 —— 新增「`LineSink` 是唯一渲染口」的形态断言：用测试 sink 断言行号 / 可见段 / 截断标记 / 行字节四项都来自 `emit`，正文不含其它拼接；默认 sink 输出与旧拼接逐字节一致
- [x] `tests/agent_read.rs`：新增负向 schema 断言——`input_schema` 不得出现 `column` / `maxLineChars`（§3.1，把「行内翻页属 I58」固化）

---

## T2 — 整行计费与窗口未满停止（I57 §4.1 第 3·4·5 条 / §5 F1–F4 / §10 G2–G5）· 已完成

退出条件：G2 / G3 / G4 / G5 通过；单行文件不再等于全量扫描；「窗口未满但预算耗尽」会停。

- [x] `agent_read/window.rs`：`Scanner` 增 `current_line_bytes`（**整行**解码字节，含被 `MAX_LINE_CHARS` 切掉的尾部），`append` 在累积可见段的同时累加它
- [x] `agent_read/window.rs`：`line_budget_bytes` = 行号前缀字节 + `current_line_bytes` + 标记字节（若截断）+ 换行字节（§3.2）；截断尾部同样付费
- [x] `agent_read/window.rs`：`rendered_bytes` 只累计可见段、`line_budget_bytes` 只进预算，**两者不混用**（`data` 与提示用前者）；`budget_bytes` 为预算唯一累加器
- [x] `agent_read/window.rs`：`stopped` 后每行只累加 `grace_bytes += line_budget_bytes`；`grace_bytes >= EOF_GRACE_BYTES` → 停止扫描、`total_lines = None`
- [x] `agent_read/window.rs`：补 N5 的洞——即使 `rendered_lines < line_cap`，预算耗尽也必须 `stopped = true`，并记录「窗口未满」事实（`window_full`，供 S3 / S4）
- [x] `agent_read/window.rs`：`truncated = stopped && (stopped_by_bytes || line_cap_is_output_cap)`（§4.2），不改既有语义
- [x] `agent_read/window.rs`：`WindowOutcome` 增 `stopped_by_bytes` / `window_full` / `scanned_bytes`（供 S3 文案与 G2 断言；本切片只被测试读取，故带 `#[allow(dead_code)]` 与「T3 消费」注释）
- [x] `window.rs` 单测 G2 —— 5 行各 4 MiB、`offset=1`：`truncated = true`、`totalLines = null`、`window_full = true`；扫描字节 ≤ `EOF_GRACE_BYTES + 单行整行字节 + CHUNK_BYTES` 且小于文件总长
- [x] `window.rs` 单测 G3 —— 120 行各 100 KB：`returnedLines = 10`、`truncated = true`、`totalLines = null`、`window_full = false`（S3 文案断言放 T3）
- [x] `window.rs` 单测 G4 —— 3 行各 4 MiB、`offset=2`：第 1 行跳过且不计费，第 2 行整行计费后渲染，第 3 行被预算拒 → `returnedLines = 1`、`totalLines = 3`、`window_full = true`
- [x] `window.rs` 单测 G5 —— 2500 行普通文件仍得 `totalLines = 2500`（既有集成用例 `line_cap_truncates_and_reports_the_real_total` 同时覆盖）
- [x] `window.rs` 单测 F1 / F5 / F6 —— 单行 1 MiB 收口、`\n` 结尾不产生额外空行、BOM-only 文件 `totalLines = 0`

> G2–G5 的判据依赖 `scanned_bytes` / `window_full` / `budget_bytes` 等**扫描内计量**，`read` 的结果里不暴露这些值，因此落在 `window.rs` 的 `#[cfg(test)]` 里（可直接读 `Scanner` 字段），集成层只保留行为面断言。
>
> 本切片对 §3.2 做了两处判据校正（已同步 I57 规格）：① `offset` 之前的行**不进**预算（否则 10 万行 `offset=99991` 的既有分页用例会读不到末尾）；② G2 的扫描量上界要加「正在扫的那一行」——宽限按行累加，4 MiB 的行会让上界从 `8 MiB + 1 块` 抬到 `8 MiB + 4 MiB + 1 块`。

> 未决问题 OQ1（预算 8 MiB 是否按文件大小分档）在本切片落地前定稿；若定为分档，须先改 I57 §3.2 表与本清单 G2 / G3 判据。

---

## T3 — 报告面（I57 §3.3 / §3.4 / §6.1 / §6.2 / §10 G6·G7）· 已完成

退出条件：G6 / G7 通过；`data.truncatedLines` 恒在且与 `[long-lines]` 的 `n` 一致；footer 顺序固定且被 `edit` 的 `trailing_footer_marker` 正确跳过。

- [x] `agent_read/window.rs`：`WindowOutcome` 增 `truncated_lines`（只数**渲染过**的行，`stopped` 之后不计数，保证 `truncatedLines <= returnedLines`）
- [x] `agent_read/mod.rs`：`execute` 的 `data` 追加 `truncatedLines`，**恒在、类型稳定**（§3.4）
- [x] `agent_read/mod.rs`：`truncatedLines` **不影响** `truncated`（`truncatedLines > 0` 且 `truncated = false` 是合法组合）
- [x] `agent_read/mod.rs`：`description` 升级为**可归因**表述（被切的行带 `[truncated]` 标记、结果报告被切行数），保留 `totalLines` 可能为 `null` 与「输出域文件需授权」两句；**不得**提 `column` / 宽度可调（§6.1）
- [x] `agent_read/mod.rs`：`truncation_notice` 扩为四态 S1–S4（§6.2）；**S1 / S2 逐字节不变**，只新增 S3（`scan budget exhausted after {scanned} bytes; showing {r} lines, total unknown`）/ S4（`no lines rendered; scan budget exhausted`）
- [x] `agent_read/mod.rs`：footer 顺序固定为 正文 → 截断提示（若 `truncated`）→ `[long-lines]` → `[encoding]`，每标记独占一行（复用 `append_footer`）
- [x] `agent_read/mod.rs`：`[long-lines: {n} lines cut at {width} chars]`，`n = truncatedLines`、`width = MAX_LINE_CHARS`（与 `data` 同一计数，不二次统计）
- [x] `agent_edit/report.rs`：`FOOTER_TAGS` 追加 `"long-lines"`（与 I57 §3.3 逐字同步，§0.3 P2）；`catalog` 已在册，不重排顺序
- [x] `tests/agent_read.rs`：G6 —— `truncatedLines` 恒在、`truncatedLines <= returnedLines`、与 `truncated` 正交（构造 `truncatedLines > 0 && truncated = false` 的用例）
- [x] `tests/agent_read.rs`：G7 —— `[long-lines]` 仅在 `truncatedLines > 0` 出现、独占一行、在 `[encoding]` 之前、`n` 与 `data.truncatedLines` 一致
- [x] `tests/agent_read.rs`：`long_lines_are_truncated_with_a_marker` 的 `assistant_text` 断言补上 `[long-lines: 1 lines cut at 2000 chars]`（**唯一**允许修改的既有断言）
- [x] `tests/agent_edit.rs`：G7（读侧）—— `trailing_footer_marker` 能跳过 `[long-lines]` / `[catalog]`，`old_string` 不含标记

> 未决问题 OQ2（`truncatedLines` 是否拆分计数）、OQ4（`[long-lines]` 是否带 `grep` 引导语）在本切片落地前定稿。

---

## T4 — 回执与 `edit` 闸门（I57 §3.5 / §3.6 / §6.3 / §10 G8）· 已完成

退出条件：G8 通过——`truncated_lines > 0` 的回执上 `edit` 返回业务错误且文件未被修改；`truncated_lines = 0` 的回执上 `edit` 仍成功。

- [x] `runtime/storage.rs`：`ReadReceipt` 追加 `truncated_lines: usize` 与 `line_window_max_chars: usize`（§3.5 是唯一字段表，B 不得再增）
- [x] `runtime/storage.rs`：`ReadReceipt::utf8(..)` 补默认值 `0` / `MAX_LINE_CHARS`，既有 `write` 等调用点零改动
- [x] `agent_read/mod.rs`：`execute` 的 `record` 写入 `truncated_lines = data.truncatedLines` 与 `line_window_max_chars = MAX_LINE_CHARS`
- [x] `agent_read/mod.rs`：回执写入点**只有一处**（`execute` 末尾），不得在 `preview_for_approval` / `validate_input` 另写
- [x] `agent_edit/mod.rs`：在 `receipts.lookup` 的 `Some(receipt)` 分支之后、`apply::apply_edit` 之前插入闸门：`receipt.truncated_lines > 0` → `ToolResult::err`
- [x] `agent_edit/mod.rs`：闸门**不重扫文件**、**不改输入 schema**（不给模型开关，§3.6 末条）；`revision` 校验仍在闸门之前（N11 语义不变）
- [x] `agent_edit/mod.rs`：闸门文案按 §6.3 逐字实现——含 `path` / `n` / `width`，逃生路径只写 `grep` 定位；**不得**出现 I58 的工具名
- [x] `tests/agent_read.rs`：`receipt_records_the_detected_encoding_and_bom_state` 补断言两个新字段（`truncated_lines == 0`、`line_window_max_chars == 2000`）
- [x] `tests/agent_edit.rs`：G8 / F8 —— 先读超长行文件（`truncated_lines > 0`），再 `edit`：断言业务错误、文案含宽度与行数、文件内容逐字节未变、`apply_edit` 未被调用
- [x] `tests/agent_edit.rs`：闸门不误伤——`truncated_lines == 0` 的回执上 `edit` 仍成功（既有 `edit` 用例全绿）

> 未决问题 OQ3（闸门整文件 vs 逐行）在本切片落地前定稿：若定为逐行，须先在回执里记「被切行号区间」并改 I57 §3.5 字段表；现清单按**整文件**口径实现（§9 L8 记录其误伤面）。OQ5（`line_window_max_chars` 改名）若定稿须同步 I57 §3.5 与 I58 §7。

---

## 收口核对（每批次结束跑，不单独占切片）

- [x] G9：`npm run contract:gen` 无 diff、`src/types/protocol.contract.ts` 与 `src/vault/core/contract/vault.contract.ts` 无变化（本阶段无 DTO 改动）
- [x] G10：`grep` / `ls` / `write` 测试全通过（`cargo test -p assistant-tools`）
- [x] G11：编码路径全通过——BOM / 显式 `encoding` / 猜测回退 / 跨块多字节字符
- [x] 命令（cwd = `src-tauri`）：`cargo test -p assistant-tools`、`cargo clippy -p assistant-tools --all-targets`、`cargo fmt --check`
- [x] 邻接确认：`agent_edit` 的 `MAX_WHOLE_FILE_BYTES`（16 MiB）、`grep` 的无上限单行、落盘回填的头尾窗口**均未改**
- [x] 撤出确认：`read` 的 schema 仍是四项、`ReadRequest` / `WindowRequest` 无 `column` / `max_line_chars` 字段（B 方案边界不回退）

> 收口执行记录（2026-11-18，P16 收尾）：`contract:gen` 重跑后 `src/types/protocol.contract.ts` 的既有 22 行改动全部来自 P15（节点事件），与 I57 无关（diff 中无 `truncated` / `read` 相关行）；`vault.contract.ts` 无变化。G10 / G11 由 `cargo test -p assistant-tools` 全量覆盖（含 `agent_grep` / `agent_ls` / `agent_write` / 编码路径用例）。邻接与撤出两项经代码核对无改动。

---

## 验收记录

（每批次完成时在此追加：日期 · 批次 · 门禁编号 · 结果摘要。无记录的门禁视为未交付。）

- **2026-11-18 · T1 · G1 / G12 通过**
  - `LineSink` 落地为 `pub(crate)` trait：`emit(number, visible, truncated, line_bytes)` + 必实现 `reset()`；`TextSink` 复刻原格式，`Scanner` 只留 `rendered_lines` / `rendered_bytes` / `total_rendered_bytes` 三个计量器，不再持有 `rendered`。
  - 入口拆成 `scan`（内部建 `TextSink`，把正文 move 回 `WindowOutcome.rendered`）与 `scan_into`（外部 sink，`rendered` 恒空）；`agent_read/mod.rs` 的调用点零改动。
  - `EOF_GRACE_CHARS`（1 MiB 字符）删除 → `EOF_GRACE_BYTES = 8 * 1024 * 1024`，`grace_chars` → `grace_bytes`，累加量改为解码后字节；**停止条件未变**（仍是 `rendered_lines >= line_cap` / 字节预算耗尽）。
  - G12 形态断言落在 `window.rs` 内 `#[cfg(test)]`：`RecordingSink` 断言四项事实全来自 `emit`、`outcome.rendered` 为空、默认 sink 输出与 `scan` 逐字节一致；另有 `reset` 清脏行与超长行截断标记透传两例。
  - `cargo test -p assistant-tools`：lib 234 条、`agent_read` 41 条全绿；`cargo clippy -p assistant-tools --all-targets` 无警告；`cargo fmt --check` 无 diff。
  - 修正一处自造的错误断言：`rendered_bytes` 不可与 `rendered.len()` 划等号（`prefix_bytes` 只算行号数字列、不含 `\t`，N6 既有估算口径），改为断言「等于 sink 各次 `emit` 的 `line_bytes` 之和」。
- **2026-11-18 · T2 · G2 / G3 / G4 / G5 通过**
  - `Scanner` 增 `current_line_bytes`：`append` 先无条件累加整行字节，再走「可见段最多 2000 字符」的累积逻辑，因此被切掉的尾部照常付费（N5 的根因修复）。
  - `flush_line` 分出两个字节数：`line_bytes`（可见段口径，进 `rendered_bytes` 与 `data`）与 `line_budget_bytes`（整行口径，只进 `budget_bytes` 与 `grace_bytes`）；预算判定用后者，对外文案用前者。
  - 停止判定改为「行窗口满 || `budget_bytes + line_budget_bytes > byte_cap`」，**不再依赖 `rendered_bytes`**，于是「窗口未满但预算耗尽」也会停（N5 的洞）；首个渲染行不受预算限制（`rendered_lines == 0` 时只判行窗口），保证「永远至少给一行」。
  - 归因：`window_full = 行窗口满 || line_budget_bytes > byte_cap`；`stopped_by_bytes` 记录成因；`scanned_bytes` 累计读盘字节（BOM 跳过量不计，头部探测另计）。三者带 `#[allow(dead_code)]`，T3 的 S2 / S3 提示消费。
  - 判据校正两处（已回写 I57 §3.2 / §4.1 / §5 F2 / §10 G2·G4）：① `offset` 之前的行不进预算——给跳过行计费会让 10 万行 `offset=99991` 的既有分页用例（跳过段约 1.6 MB）读不到末尾，属行为回归；② 宽限按行累加，扫描量上界必须是「`EOF_GRACE_BYTES` + 正在扫的那一行 + 一块」，原判据 `+ CHUNK_BYTES` 在 4 MiB 行的用例上不可满足。
  - 新增 `window.rs` 单测 8 条：整行计费、窗口满归因、窗口未满也停、跳过不计费、行窗口停止仍算准总数、单行 1 MiB 收口、`\n` 结尾不增空行、BOM-only 得 0 行；`tests/agent_read.rs` 41 条既有用例零改动全绿。
  - OQ1 定稿：**不分档**，固定 `EOF_GRACE_BYTES = 8 MiB`——分档要在扫描前额外拿文件大小做分支，收益（小文件少一次宽限截断）不足以抵掉两套阈值的测试面。
  - `cargo test -p assistant-tools`：lib 240 条、`agent_read` 41 条及其余集成用例全绿；`cargo clippy -p assistant-tools --all-targets` 无警告；`cargo fmt -p assistant-tools --check` 无 diff。
- **2026-11-18 · T3 · G6 / G7 通过**
  - `Scanner` 增 `truncated_lines`：在 `flush_line` 的 `emit` 之后按 `content_truncated` 累加，**只在渲染路径**上计数，因此 `stopped` 之后被跳过的行不会把计数抬到 `returnedLines` 之上（`<=` 是结构性保证，不是事后裁剪）。
  - `data` 追加 `truncatedLines`（键恒在，`0` 也是显式值）；`truncated` 的语义一行未改，二者正交——单行 4 MiB 的用例正是 `truncatedLines = 1 && truncated = false`。
  - `truncation_notice` 的归因轴只有两个：`total_lines` 是否 `Some`（读到 EOF 否）与 `window_full`（窗口满否）。S1 / S2 的格式串与改动前逐字相同，S3 才消费 `scanned_bytes`。
  - **S4 当前不可达**（已回写 §6.2）：停止判定只在渲染过至少一行之后才可能成立（行窗口要 `rendered_lines >= line_cap`，字节预算要 `rendered_lines > 0`），而首个渲染行不受预算限制；分支保留以免四态出现缺口，并在代码注释里写明不可达原因。
  - **S3 的构造条件比原判据严**（已回写 §6.2 / §10 G3）：必须让「剩余内容 > `EOF_GRACE_BYTES`」，否则宽限会一路读到 EOF 而给出精确总数（那是 S1）。用例定为 20,000 行 × 600 字节（≈12 MB）：预算在第 ~1727 行收口（`window_full = false`），剩余 ≈11 MB > 8 MiB。
  - `[long-lines]` 由 `long_lines_footer` 生成，`n` 与 `data.truncatedLines` 同源；footer 顺序经 `append_footer` 串接，天然各占一行。
  - `FOOTER_TAGS` 追加 `"long-lines"`；同时把 0 匹配诊断里「after non-UTF-8 output」改成「after the body」——原措辞只适用于 `[encoding]`，对 `[long-lines]` / `[catalog]` 是错的。
  - 新增测试 6 条：`tests/agent_read.rs` 4 条（正交性、`0` 显式值、footer 顺序、逐行累加）、`tests/agent_edit.rs` 1 条（`[long-lines]` 被诊断）、`window.rs` 2 条（只数渲染行、与 sink 标记数一致）。
  - `cargo test -p assistant-tools`：lib 242 条、`agent_read` 46 条、`agent_edit` 30 条及其余集成用例全绿；`cargo clippy -p assistant-tools --all-targets` 无警告；`cargo fmt -p assistant-tools --check` 无 diff。
- **2026-11-18 · T4 · G8 通过**
  - OQ3 定为**整文件**口径：回执只记「有行被切」的计数，不记行号区间——记区间会把回执变成行索引（§0.2 明确划归 B），且闸门若逐行放行，`old_string` 仍可能跨到被切行上，校验不可靠。
  - 宽度常量收敛到 `storage::DEFAULT_LINE_WINDOW_MAX_CHARS`，`agent_read::MAX_LINE_CHARS` 改为它的别名。这样「闸门文案里的 `{width}`」与「实际切行宽度」只有一处定义，不会各写一个 `2000` 而漂移。
  - 闸门位置在 `Some(receipt) => receipt` 之后、`resolve_label` 之前：`revision` 校验仍先跑（N11 语义不变），且 `apply_edit` 根本不会被调用（文件逐字节不变的直接原因）。
  - 闸门不重扫文件：只消费回执，`truncated_lines` 由 `read` 在唯一写入点 `execute` 末尾写入，值就是 `data.truncatedLines`（同一计数器，无二次统计）。
  - `edit` 成功后的回执把 `truncated_lines` 置 `0`（写入后文件内容已由 `edit` 全量掌握），`line_window_max_chars` 继承原回执——`write` 的回执同理取默认宽度。
  - `description` 追加一句说明闸门与逃生路径（只用 `grep`，不出现 I58 工具名）；输入 schema 一行未改。
  - 新增测试 5 条：`tests/agent_edit.rs` 3 条（闸门拦截且文件未变、`0` 不误伤、`revision` 校验优先）、`tests/agent_read.rs` 1 条（回执计数与 `data.truncatedLines` 同源）、`tests/storage.rs` 1 条既有用例补两字段。
  - `cargo test -p assistant-tools`：lib 242、`agent_read` 47、`agent_edit` 33、`storage` 27、`agent_write` 30 及其余集成用例全绿；`cargo clippy -p assistant-tools --all-targets` 无警告；`cargo fmt -p assistant-tools --check` 无 diff。
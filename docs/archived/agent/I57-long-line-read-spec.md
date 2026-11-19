# I57 — P16：`read` 长行如实报告与扫描收口规格

| 项 | 值 |
|---|---|
| 阶段 | **P16 长行读取（如实报告与扫描收口）**；切片 T1–T4 全部在本阶段内（§11），不拆子阶段 |
| 状态 | 定稿（**B 方案**：只做报告面，不做行内翻页）；T1–T4 未开工 |
| 日期 | 2026-11-18 |
| 阶段台账 | [I38 §2](../../store/I38-agent-roadmap.md)（阶段、前置、状态的唯一登记点）；本文 §10 的 G1–G12 即 P16 退出条件 |
| 关联 | [I58](./I58-super-tool-spec.md)（**P17 阶段规格**：能力发现、索引型读取、行地址编辑；承接本文 §9 的 L1 / L2 / L3 / L6 / L7）、I38c read 分册（已归档）（§3.4 流式分块与单行截断的原始口径，本文**替代**其长行部分）、I38c P5 总规格（已归档）（§14.4 footer 五条规则继续有效）、I46 compact 分册（已归档）（§2.2 截断提示三态）、[I27](../../store/I27-tool-data-store-spec.md)（落盘阈值与预览窗口） |
| 影响范围 | `src-tauri/crates/assistant-tools/src/os/tools/agent_read/{mod.rs,window.rs}`、`src-tauri/crates/assistant-tools/src/os/tools/agent_edit/{mod.rs,report.rs}`、`src-tauri/crates/assistant-tools/src/runtime/storage.rs`（`ReadReceipt`）、`src-tauri/crates/assistant-tools/tests/{agent_read.rs,agent_edit.rs}`。**不改** wire / DTO（`data` 是开放 JSON）、不改 `grep` / `ls` / `write`、不改前端 |

> 一句话：`read` 现在的单行上限只解决「不进内存」，没解决**「截断之后会发生什么」**——第 2001 个字符起**静默消失**（无计数、无提示），单行文件**仍全量扫描到 EOF**（预算看不见真实行宽），且截断后 `edit` 会**静默**只替换前 2000 字符。本文把这三件事一次修掉：截断**如实报告**（`data.truncatedLines` + `[long-lines]` + 四态提示）、扫描成本**按整行计费**并收口到字节预算、`edit` 在截断回执上 **fail-closed**。行内翻页（`column` / `maxLineChars`）**不在本文**，由 [I58](./I58-super-tool-spec.md) 的 `super_read` 承接（§0.2 例外登记）。

**编号约定**：`G`（门禁）、`T`（切片）、`L`（已知限制）、`R`（拒绝方案）、`OQ`（未决问题）、`N`（现状事实）、`F`（失败项）在本文与 [I58](./I58-super-tool-spec.md) 中**同名独立**。跨文档引用必须带文档前缀（`I58 §9 L1` / `I58 §3.4`），裸写 `§9` 一律指本文。

---

## §0 范围与切分轴

### 0.1 交付范围

| 项 | 内容 |
|---|---|
| **纳入** | ① 扫描宽限从**字符**口径改**字节**口径（`EOF_GRACE_CHARS` → `EOF_GRACE_BYTES = 8 MiB`）；② 每行预算按**整行**字节计（`line_budget_bytes`），修复「真实行宽对预算不可见」；③ 补「窗口未满但预算耗尽也停」（N5 的洞）；④ `data` 增 `truncatedLines`；⑤ footer 增 `[long-lines]` 并登记进 `FOOTER_TAGS`；⑥ `ReadReceipt` 增 `truncated_lines` / `line_window_max_chars`，并驱动 `edit` fail-closed 闸门；⑦ 截断提示按字节口径收敛为四态；⑧ 扫描核心 sink 化（§0.3 P1） |
| **排除** | `column` / `maxLineChars` / `lineWindow` / `linesBeyondColumn` / `[column]` / 提前提交 / 列前丢弃——全部移交 [I58](./I58-super-tool-spec.md) 的 `super_read`（§0.2 例外登记、§12 R10）；行索引、`seek`、精确 `totalLines` 的跨调用保证、`unit=byte` 列、跨会话缓存、子串匹配编辑——同属 I58（P17） |

### 0.2 切分轴

主判据：**一次线性扫描内能否有界完成**。

| 侧 | 判据 | 归属 |
|---|---|---|
| A（本文） | 一次扫描内可有界完成：报告截断事实、按整行计费、在预算内停 | 本阶段做 |
| B（[I58](./I58-super-tool-spec.md)） | 需要跨调用状态（行索引 / 游标）、随机访问（`seek`）、或答案需要「全文件已知」（精确 `totalLines` 无预算约束） | 以**独立工具**承接，不在 `read` 内隐式发生 |

**例外登记（必须显式写明，不得用轴「圆」过去）**：`column` / `maxLineChars` 本身**可以**在一次扫描内有界完成（跳过前 N 个字符、只累积 M 个字符），因此它们**在轴的判据上属于 A 侧**。本文仍把它们划给 B，理由只有一条，且是成本理由而非边界理由：

- B 的 `super_read` 在索引就绪后**必然**要实现「行内任意位置」的列算术（含 `unit=char|byte` 两种口径，I58 §3.2）；A 侧若同时实现一套字符口径的列算术，两处会漂移（同一行同一列取到不同内容）。
- A 侧实现列窗口要引入两条**没有测试能兜住**的不变量：可见字节 ≠ 计费字节、同一行可能被 `flush` 两次（提前提交 + 换行）。这两条只在「含超长行且后面还有行」的用例上暴露，默认路径（`column=1`）完全不经过。
- 收益侧：P16 的核心价值是「截断可归因」，不是「行内可翻页」。截断可归因能立刻阻止模型基于残缺内容做 `edit`；行内翻页只是便利性。

因此本文的**报告面**（`truncatedLines` / `[long-lines]` / 回执 / 闸门）与 B 的**翻页面**（`column` / `maxLineChars` / `lineWindow` / `linesBeyondColumn` / `[column]`）分开，且**报告面先落地**。

### 0.3 预埋接口（A 侧唯一需要为 B 预留的东西）

| 编号 | 预埋点 | 形态 | 约束 |
|---|---|---|---|
| P1 | 扫描核心 sink 化 | `window.rs` 的渲染路径抽成 `LineSink`（逐行 `emit(number, visible, truncated, line_bytes)` + 必实现的 `reset()`），`Scanner` 只负责计量与预算 | 本预埋点是 A 的内部结构，**B 可不消费**（I58 自写扫描与渲染，I58 §0.3）：B 只须保证行格式**字面量**一致（`{number:>6}` + `\t`、` [truncated]`），否则 `edit` 的前缀剥离会失效；若 B 选择复用 sink，则必须实现 `reset()`（编码回退重扫的前置条件）。A 侧无行为分支 |
| P2 | footer 标记集登记 | §3.3 是**唯一**登记表，`agent_edit/report.rs` 的 `FOOTER_TAGS` 与之逐字对应 | B 只能**追加**条目并回写本表，不能改既有语义 |
| P3 | 回执字段登记 | §3.5 是 `ReadReceipt` 的唯一字段表 | B 的 `super_read` 必须写同一份回执（同闸门语义），**不新增字段**；`line_window_max_chars` 就是为 B 的 `maxLineChars` 预留的 |

A 侧任何文案都不得断言 B 已启用（§6.3）。

### 0.4 依赖方向

单向：**B 可引用 A 的任意节；A 只在 §0.3 的三个预埋点提及 B，且不产生任何行为分支**。P1 只是 A 的内部重构成果，B 不复用也不构成违约（I58 §0.3「契约对齐、实现独立」）；P2 / P3 是对齐层，B 必须遵守。

---

## §1 现状（代码事实）

以下为 2026-11-18 的实际实现，路径相对仓库根。

| 编号 | 事实 | 位置 |
|---|---|---|
| N1 | 单行上限 `MAX_LINE_CHARS = 2000`（字符），超出即停累积并追加 `" [truncated]"` | `agent_read/mod.rs`、`window.rs::append` |
| N2 | 上限**只影响累积**：超出部分仍被解码并丢弃，`current_chars` 不再增长 | `window.rs::append` |
| N3 | 超长行**不提前提交**：必须等 `\n` 或 EOF 才 `flush_line`，单行文件等于全量扫描 | `window.rs::feed` / `run` |
| N4 | 扫描宽限是**字符**口径 `EOF_GRACE_CHARS = 1 MiB`，且只在 `stopped` 之后累加 | `window.rs::flush_line` / `run` |
| N5 | 「窗口未满但字节预算耗尽」不会触发 `stopped`：`rendered_lines > 0 && rendered_bytes + line_bytes > byte_cap` 才停 | `window.rs::flush_line` |
| N6 | `total_rendered_bytes` 已按字节累计（含前缀 / 标记 / 换行），但用 `prefix_bytes(number)` 估算 | `window.rs::flush_line` / `prefix_bytes` |
| N7 | 截断提示两态：`Some(total)` → `showing X of Y lines, A of B bytes`；`None` → `showing X lines, A bytes; total unknown` | `agent_read/mod.rs::truncation_notice` |
| N8 | `data` 恒有 `path` / `startLine` / `endLine` / `totalLines` / `truncated` | `agent_read/mod.rs::execute` |
| N9 | footer 只有 `[encoding: …]`；`edit` 侧 `FOOTER_TAGS = ["encoding", "non-utf8", "catalog"]` | `agent_read/mod.rs::encoding_footer`、`agent_edit/report.rs` |
| N10 | `ReadReceipt` 只有 `revision` / `tool_call_id` / `encoding` / `has_bom`；`revision` = `len:nanos` | `runtime/storage.rs`、`agent_read/mod.rs::file_revision` |
| N11 | `edit` 只校验「读过」与「revision 未变」，**不看**该次读取是否被截断 | `agent_edit/mod.rs`（`receipts.lookup` 分支） |
| N12 | `edit` 的 `old_string` 匹配基于整文件内容，截断行在模型侧只到 2000 字符，匹配失败/歧义无法归因 | `agent_edit/apply.rs`、`agent_edit/report.rs::ambiguous_message` |
| N13 | `edit` 有整文件上限 `MAX_WHOLE_FILE_BYTES = 16 MiB` | `agent_edit/apply.rs` |
| N14 | `grep` **没有**单行长度上限（`text.push_str(line)`），但单文件 `MAX_FILESIZE = 16 MiB`、`MAX_MATCHES = 100`、`MAX_OUTPUT_LINES = 300` | `agent_grep/{mod.rs,render.rs}` |
| N15 | 落盘：单结果超 `PER_TOOL_LIMIT_CHARS = 50 000` 直接落盘，回填 = 头 600 + 尾 600 | `runtime/storage.rs` |
| N16 | 现有测试已覆盖「长行截断」「行数上限截断」「字节上限截断」，但**无**预算口径与截断计数的断言 | `tests/agent_read.rs` |

结论：**当前实现没有任何一处为「截断发生了多少次」负责**，也没有任何一处让「真实行宽」进入预算；`edit` 的静默部分替换同样没有任何闸门。

---

## §2 目标与非目标

### 2.1 目标

1. 截断**可归因**：本次输出里有多少行被切，在 `data.truncatedLines` 与 `[long-lines]` footer 两处**一致可见**。
2. 「不可达」的成本**有界且可见**：扫描量按**整行**字节计费，与可见段长度解耦；上限由常量表达，并在提示里如实写「没扫完」。
3. `edit` 在「本次读取有截断行」时**拒绝执行**，不再静默部分替换。
4. 既有行为零回归：**不含长行**的文件输出逐字节不变；含长行的文件只多一行 `[long-lines]` footer。

### 2.2 非目标

- 不做行内翻页（`column` / `maxLineChars`）——属 [I58](./I58-super-tool-spec.md) `super_read`（§0.2 例外登记、§12 R10）。
- 不建行索引、不 `seek`、不缓存（§9 L1 / L7）。
- 不保证预算耗尽时的 `totalLines`（§9 L3）。
- 不做 `unit=byte` 的列口径（§9 L5）。
- 不改 `grep`（N14 的 `grep` 路径不受本文影响）。
- 不改 wire / DTO / 前端。

---

## §3 契约

### 3.1 输入

**不变**。`input_schema` 仍是 `path` / `offset` / `limit` / `encoding` 四项，`required = ["path"]`。

- `MAX_LINE_CHARS = 2000` 仍是**硬上限**，**不**做成可调参数（§12 R2）：宽度可调等价于把列窗口的一半提前搬到 A 侧。
- 新增一条**负向**断言：schema 不得出现 `column` / `maxLineChars`（把「行内翻页属 B」固化成测试，避免后续误加）。

### 3.2 渲染与预算

渲染格式**不变**：`{line_number:>6}\t{visible}`，被截断的行在行尾追加 `" [truncated]"`（`LINE_TRUNCATED_MARKER`）。`LINE_NUMBER_WIDTH = 6` / `LINE_NUMBER_SEPARATOR = '\t'` 不动（`edit` 档 2 依赖它们）。

预算口径（`window.rs`）：

| 常量 | 现值 | 改为 | 说明 |
|---|---|---|---|
| `EOF_GRACE_CHARS` | `1 MiB` 字符 | **删除** | 字符口径随编码放大，不可控（§12 R5） |
| `EOF_GRACE_BYTES` | — | `8 * 1024 * 1024` | 窗口填满后「只计量不渲染」的字节预算 |
| `CHUNK_BYTES` | `256 KiB` | 不变 | 单次读盘粒度 |
| `DEFAULT_MAX_BYTES` / `DEFAULT_MAX_LINES` | `1 MiB` / `2000` | 不变 | 输出上限 |

每行进入预算的字节数 `line_budget_bytes` = 行号前缀字节 + **整行**字节 + 标记字节（若截断）+ 换行字节。

- 「整行」指该行**解码后的全部字节**，不是可见段的字节：被 `MAX_LINE_CHARS` 切掉的尾部同样付费。这是 N5 的根因修复——旧实现用可见段长度组 `line_bytes`，所以 4 MiB 的行在预算里只值 2000 字符。
- 两个累加器**不混用**：`rendered_bytes` 只累计可见段（含前缀 / 标记 / 换行），供 `data` 与提示使用；`line_budget_bytes` 只进预算，不出现在任何对外文案。
- 预算只对 `offset` **及之后**的行生效：`offset` 之前的行既不进输出、也不进预算。给跳过行计费会让深分页白吃预算（10 万行文件 `offset=99991` 的既有用例正是靠这条才读得到末尾）。
- 停止归因：`window_full` = 行窗口已满，**或**下一条行的 `line_budget_bytes` 本身就大于 `DEFAULT_MAX_BYTES`（这一行单独就塞不进任何窗口）。后者与「累加到这一步放不下」区分开，S2 / S3 靠它二选一。

### 3.3 footer 标记登记表（本阶段拥有的封闭集合）

登记表是**单一来源**：`os/shared/footer.rs`（`footer::TAGS` + 常量 + `render()` / `trailing_marker()`）。本节只记录 `read` 自己产出的两个标记。

| 标记 | 出现条件 | 形态 | 归属 |
|---|---|---|---|
| `[long-lines]` | `truncatedLines > 0` | `[long-lines: {n} lines cut at {width} chars]` | P16 |
| `[encoding]` | 现有规则（UTF-8 无 BOM 时静默） | 不变 | P16（沿用） |

- `n = truncatedLines`（**本次输出窗口内**带截断标记的行数），`width = MAX_LINE_CHARS`（`2000`）。两者必须与 `data.truncatedLines` 一致（同一计数，不二次统计）。
- 顺序固定：正文 → 截断提示（若 `truncated`）→ `[long-lines]` → `[encoding]`；每个标记独占一行。
- **生成方式**：`read` 只调 `footer::render(footer::LONG_LINES / ENCODING / TRUNCATED / HINT, detail)`，不手写 `"[...: ...]"` 字面量。tag 只能来自 `footer` 常量，因此「新增标记漏登记」在编译期就暴露。
- **识别方式**：`agent_edit/report.rs` 不再维护本地白名单，改调 `footer::trailing_marker()` 读同一份 `footer::TAGS`。原先「本节 / `FOOTER_TAGS` / 生产方三处同步」的约定已作废（漏配正是由此产生）。
- **B 只能追加**（I58 §3.4：`index` / `column` / `catalog` / `hint`），不得改本节既有语义；追加的标记插在 `[encoding]` **之前**，因此 `read` 自身的顺序（正文 → `[long-lines]` → `[encoding]`）不变。

### 3.4 `data`

保留 N8 的五项，追加**一项**（恒在，类型稳定）：

| 键 | 类型 | 语义 |
|---|---|---|
| `truncatedLines` | number | 本次输出中**加了截断标记**的行数，即 `line_chars > MAX_LINE_CHARS` 的行数 |

不变量：

- `truncatedLines <= returnedLines`（只数渲染过的行；`stopped` 之后不再渲染，也不再计数）。
- 与 `truncated` **正交**：`truncatedLines > 0` 而 `truncated = false` 是合法组合（窗口取完了行，但行本身超宽）。
- 值恒在（`0` 表示本次输出无被切行），不用 `null`。
- B 可追加 `lineWindow` / `linesBeyondColumn`（`super_read` 落地时回写本表），A 侧不预留空键。

### 3.5 `ReadReceipt`

`runtime/storage.rs` 的 `ReadReceipt` 追加两个字段（A 侧唯一字段表，B 不得再增）：

| 字段 | 类型 | 语义 |
|---|---|---|
| `truncated_lines` | `usize` | 写入方（`read` / I58 的 `super_read`）报告「本次读取有截断行」的计数；`0` 表示完整 |
| `line_window_max_chars` | `usize` | 本次读取使用的每行宽度，供 `edit` 文案归因 |

- A 侧写入点**只有一处**（`agent_read/mod.rs::execute` 末尾的 `record`）：`truncated_lines = data.truncatedLines`，`line_window_max_chars = MAX_LINE_CHARS`（常量）。
- `line_window_max_chars` 在 A 内是常量、不产生任何分支，它是 **P3 为 B 预留**的：`super_read` 落地后写自己的宽度，`edit` 的闸门文案因此自动正确（§6.3 的 `{width}` 不需要再改）。
- `ReadReceipt::utf8(...)` 构造器补默认值 `0` / [`DEFAULT_LINE_WINDOW_MAX_CHARS`]，避免既有 `write` 等调用点全改。回执不落盘、不进 wire，无需 `serde` 属性。
- 宽度常量只有一处定义（`runtime/storage.rs::DEFAULT_LINE_WINDOW_MAX_CHARS = 2000`），`agent_read::MAX_LINE_CHARS` 是它的别名：闸门文案的 `{width}` 与实际切行宽度因此不可能漂移。
- 写侧回执（`edit` / `write` 成功后的刷新）一律写 `truncated_lines = 0`：写入完成后文件内容已由写工具全量掌握，不存在「未见尾部」。`edit` 继承原回执的 `line_window_max_chars`，`write` 取默认宽度。

### 3.6 `edit` 闸门

在 `agent_edit/mod.rs` 的 `receipts.lookup` 分支之后、`apply::apply_edit` 之前插入一道闸门：

- `receipt.truncated_lines > 0` → 业务错误（`ToolResult::err`，非 panic），文案见 §6.3，**不执行写**。
- 闸门只看回执，不重扫文件；`revision` 校验仍在闸门之前（N11 语义不变）。
- 闸门**不改** `edit` 的输入 schema（不给模型加开关），避免「知道有闸门就绕过」。
- 闸门是**整文件**口径：只要有 1 行被切，该文件整体不可 `edit`。这是有意的 fail-closed，代价见 §9 L8，逐行口径见 §13 OQ3。

---

## §4 行为

### 4.1 扫描主循环（`window.rs::Scanner`）

1. 头部判定不变（BOM → 显式 `encoding` → 二进制 → UTF-8 逐块校验 → 猜测回退）。
2. 逐块解码，`feed` 按 `\n` 切行；每行进入 `flush_line`。
3. **整行计费**：`append` 在累积可见段（最多 `MAX_LINE_CHARS` 个字符）的同时，把**被切掉的尾部字节**也计入 `current_line_bytes`；`flush_line` 用 `current_line_bytes` 组 `line_budget_bytes`（§3.2）。
4. **预算**：`stopped` 判定后（窗口满或预算耗尽），每行只累加 `grace_bytes += line_budget_bytes`；`grace_bytes >= EOF_GRACE_BYTES` → 停止扫描，`total_lines = None`。宽限按**行**累加，因此实际扫描量上界是「宽限 + 正在扫的那一行 + 一块」（§10 G2）。
5. **窗口未满也停**：即使 `rendered_lines < line_cap`，只要字节预算耗尽就必须停（N5 的洞），`truncated = true`，提示走 S3 / S4（`window_full` 归因见 §3.2）。
6. EOF 时若残留 `current` 非空则 `flush_line`（BOM-only 文件得 0 行，与现行为一致）。
7. **不提前提交**：仍等 `\n` 或 EOF（§12 R3）。没有列窗口时提前提交不省 IO，只多一条「同一行可能被 `flush` 两次」的不变量。

### 4.2 `truncated` 语义（保持）

`truncated = stopped && (stopped_by_bytes || line_cap_is_output_cap)`——本次输出**没取完**。它与 §3.4 的 `truncatedLines` 正交：`truncatedLines > 0` 而 `truncated = false` 是合法组合（窗口取完了行，但行本身超宽）。

### 4.3 行号与 `endLine`

`startLine = offset`、`endLine = startLine + returned_lines - 1`（`returned_lines = 0` 时为 `null`），不变。

### 4.4 与 `edit` 的联动

- `read` 写完回执时把 `truncated_lines` / `line_window_max_chars` 一起写入（§3.5）。
- 建议路径（只写进 §6.3 文案，不写进代码逻辑）：先用 `grep` 定位到具体行，再按行号 `offset` / `limit` 读；A 不提供行内翻页，被切行只保证「前 2000 字符可见」。

---

## §5 边界情形与失败矩阵

| 编号 | 情形 | 期望 |
|---|---|---|
| F1 | 单行 1 MiB、`offset=1` | 渲染 2000 字符 + 标记；`truncatedLines = 1`；扫描在预算内收口（≤ `EOF_GRACE_BYTES + CHUNK_BYTES`） |
| F2 | 3 行各 4 MiB、`offset=1` | 第 1 行按**整行** 4 MiB 计费（首个渲染行不受预算限制）；第 2 行因预算不足被拒 → `truncated = true`、`totalLines = null`、提示 S2 |
| F3 | 行数远少于 `limit`、总字节 > 预算 | `truncated = true`、`totalLines = null`、提示 S3（窗口未满也停） |
| F4 | 无输出行 + 预算耗尽 | 提示 S4，不报错 |
| F5 | 文件以 `\n` 结束 | 不产生额外空行（现行为保持） |
| F6 | BOM-only 文件 | `totalLines = 0`，`returned_lines = 0`，正文空 |
| F7 | 编码不可判定 / 二进制 / 目录 | 与现文案一致（`DecodeError` / `binary` / `use ls`） |
| F8 | `edit` 在 `truncated_lines > 0` 的回执上调用 | 业务错误，`apply_edit` 未被调用，文件逐字节未变 |
| F9 | 无长行的普通文件 | 输出与改动前**逐字节一致**（无 `[long-lines]`），`truncatedLines = 0` |

---

## §6 文案

### 6.1 `description`

保留「`totalLines` 可能为 `null`」与「输出域文件需授权」两句；把「cut and marked」升级为**可归因**表述：被切的行带 `[truncated]` 标记，结果里报告被切行数。**不得**承诺行内翻页（不提 `column` / 宽度可调）。

描述是静态的，**不写**「先用 `read`，读不动再换 `super_read`」一类假设性反向引导——此刻并不知道会不会撞上限。升级引导改由**运行时事实**触发（I58 §3.6）：本次确实有行被切（`truncatedLines > 0`）或 `totalLines` 为 `null`，且 `super_read` 在本轮工具集里，才追加 `[hint: …]`。三个条件缺一不可，普通文件上不出现任何提示。

### 6.2 截断提示（四态，字节口径）

| 态 | 触发 | 文案 |
|---|---|---|
| S1 | 窗口满 + 已到 EOF | `[truncated: showing {r} of {t} lines, {a} of {b} bytes]` |
| S2 | 窗口满 + 预算耗尽 | `[truncated: showing {r} lines, {a} bytes; total unknown]` |
| S3 | 窗口**未满** + 预算耗尽 | `[truncated: scan budget exhausted after {scanned} bytes; showing {r} lines, total unknown]` |
| S4 | 无输出行 + 预算耗尽 | `[truncated: no lines rendered; scan budget exhausted]` |

S1 / S2 与现实现**逐字节一致**，只新增 S3 / S4。禁止「more content follows」一类既无总数也无未知标记的写法（I46 compact §2.2 D2 的收敛要求）。

归因轴只有两个独立事实：`total_lines` 是否有值（是否读到 EOF）与 `window_full`（窗口是否已满）。`scanned_bytes` 只被 S3 消费。

- **S4 当前不可达**：停止判定只在渲染过至少一行之后才可能成立（行窗口需 `rendered_lines >= line_cap`，字节预算需 `rendered_lines > 0`），而首个渲染行不受预算限制，故 `returned_lines == 0` 与 `truncated` 不能同时成立。分支保留以免四态出现缺口；若将来引入「首行即超预算」的规则，此态才生效。
- **S3 的触发前提**：窗口未满 + 预算耗尽 + **剩余内容超过 `EOF_GRACE_BYTES`**。剩余量在宽限内时扫描会读到 EOF，落到 S1（有精确总数），不是 S3。
- S4 的 `truncated` 取值：与其它三态一致，由 `outcome.truncated` 决定（提示只在 `truncated` 时追加）。

### 6.3 `edit` 闸门文案

`'{path}' was read with {n} line(s) cut at {width} chars; the unseen part of those lines cannot be verified before editing (use 'grep' to locate the exact line first)`。

- `{width}` 取回执的 `line_window_max_chars`（A 内恒为 `2000`）。
- 文案**不得**提到 I58 的工具名（A 不依赖 B 的存在；逃生路径只写 `grep` + 按行号读）。

---

## §7 与相邻组件

| 组件 | 关系 |
|---|---|
| `grep` | 不受影响；长行匹配仍整行返回（N14）。文案建议「用 `grep` 定位再按行读」 |
| `ls` / `write` | 不受影响；`write` 的 `ReadReceipt::utf8` 构造器补默认值即可 |
| 落盘（I27） | 不受影响：落盘回填是头尾窗口，与行内位置正交 |
| `edit` | 新增闸门（§3.6），输入 schema 不变 |
| I58（P17） | 承接行内翻页（`super_read` 的 `column` / `maxLineChars` / `lineWindow` / `linesBeyondColumn` / `[column]`）；复用 §0.3 三个预埋点；A 不感知 B |

---

## §8 验收

- 单元 / 集成断言落在 `tests/agent_read.rs` 与 `tests/agent_edit.rs`（现有 N16 三条截断测试必须继续通过）。
- 每条门禁（§10）对应至少一条断言；无断言的项视为未交付。
- 回归口径：`cargo test -p assistant-tools`（cwd = `src-tauri`）。

---

## §9 已知限制（边界，不是缺陷）

| 编号 | 限制 | 理由 | 逃生路径 |
|---|---|---|---|
| L1 | 无行索引、无 `seek`：每次读取都从头扫 | 跨调用状态属于 B（§0.2） | I58 `super_read` |
| L2 | 无行内翻页：第 2001 个字符起**不可达** | 列算术与 B 的 `super_read` 双写会漂移（§0.2 例外登记） | I58 `super_read` 的 `column` |
| L3 | 预算耗尽时 `totalLines = null` | 精确总数需要读全文件 | I58 `super_read` |
| L4 | 单行最多 2000 字符，且**不可调** | 宽度可调等价于把列窗口搬到 A 侧（§12 R2） | I58 `super_read` 的 `maxLineChars` |
| L5 | 无 `unit=byte` 的列口径 | 渲染就是字符口径，byte 列会与可见段错位 | I58 `super_read` 的 `unit` |
| L6 | 「精确总数 + 行内任意位置」不能一次拿到 | 前者要扫全文件，后者要列窗口 | 两次调用（`read` + `super_read`） |
| L7 | 不缓存跨会话结果 | 缓存失效需要 revision 之外的机制 | I58 的落盘索引 |
| L8 | 闸门是**整文件**口径：本次输出有 1 行被切，该文件整体不可 `edit`（含未被切的短行） | 逐行口径需要在回执里记「被切行号区间」，改动 §3.5 字段表（§13 OQ3） | A 侧暂无逃生路径：等 I58 T5 的 `super_edit`；这是 fail-closed 的有意代价（§12 R7） |
| L9 | `truncatedLines` 只统计**单行截断**，不含「行数上限截断」 | 后者已由 `truncated` 表达（§4.2） | — |

---

## §10 门禁

| 编号 | 门禁 | 判据 |
|---|---|---|
| G1 | 零回归 | 现有 `agent_read` 测试全通过；**不含长行**的用例输出逐字节不变；含长行的用例只多一行 `[long-lines]` footer |
| G2 | 整行计费 | 5 行各 4 MiB、`offset=1`：`truncated = true`、`totalLines = null`、提示 S2、`window_full = true`；扫描字节 ≤ `EOF_GRACE_BYTES + 单行整行字节 + CHUNK_BYTES` 且小于文件总长（证明没为算总数把文件读全） |
| G3 | 预算覆盖窗口未满 | 「行数远少于 `limit`、总字节 > 预算」的文件（如 120 行各 100 KB）：`returnedLines = 10`、`truncated = true`、`totalLines = null`、`window_full = false`（S3 文案断言放 T3）。注意 100 KB 的行本身超宽，该构造下 `truncatedLines` 也为 10；要**隔离** S3 与长行截断，用窄行构造（20,000 行 × 600 字节，见 T3 验收记录） |
| G4 | 跳过不计费 | 3 行各 4 MiB、`offset=2`：第 1 行跳过且不进预算；第 2 行整行 4 MiB 计入预算后渲染，第 3 行因预算不足被拒 → `returnedLines = 1`、`totalLines = 3`（宽限够读到 EOF）、`window_full = true` |
| G5 | 精确总数不回归 | 普通大小文件（如 2500 行）仍得 `totalLines = 2500` |
| G6 | `data` 字段 | `truncatedLines` 恒在、类型稳定；`truncatedLines <= returnedLines`；与 `truncated` 正交（构造 `truncatedLines > 0 && truncated = false`） |
| G7 | footer | `[long-lines]` 仅在 `truncatedLines > 0` 出现、独占一行、在 `[encoding]` 之前；`n` 与 `data.truncatedLines` 一致；`edit` 的 `trailing_footer_marker` 能跳过它 |
| G8 | 回执与闸门 | `truncated_lines > 0` 时 `edit` 返回业务错误、文案含 `n` / `width`、文件未被修改；`truncated_lines = 0` 时 `edit` 仍成功 |
| G9 | 无 wire 变更 | `npm run contract:gen` 无 diff；前端零改动 |
| G10 | 邻接不回归 | `grep` / `ls` / `write` 测试全通过 |
| G11 | 编码路径 | BOM / 显式 `encoding` / 猜测回退 / 跨块多字节字符用例全部通过 |
| G12 | sink 等价 | `LineSink` 成为唯一渲染口：用测试 sink 断言四项事实（行号 / 可见段 / 截断标记 / 行字节）都来自 `emit`，且默认 sink 输出与旧拼接逐字节一致 |

---

## §11 切片

| 编号 | 切片 | 交付物 | 依赖 |
|---|---|---|---|
| T1 | 扫描核心 sink 化（预埋 P1）+ 宽限改字节口径 | `window.rs` 抽 `LineSink`、`Scanner` 走 sink；`EOF_GRACE_CHARS` → `EOF_GRACE_BYTES`（**先只换口径，不改停止条件**）；G1 / G12 | — |
| T2 | 整行计费与窗口未满停止 | `current_line_bytes`、`line_budget_bytes`、`grace_bytes`；补 N5 的洞；G2 / G3 / G4 / G5 | T1 |
| T3 | 报告面 | `data.truncatedLines`、`[long-lines]` + `FOOTER_TAGS`、四态提示 S3 / S4、`description`；G6 / G7 | T2 |
| T4 | 回执与 `edit` 闸门 | `ReadReceipt` 两字段、写入点、闸门与文案；G8 | T3 |

T1 与 P17 的 T1（`request_tool`）**无依赖关系**，可并行；T2–T4 与 I58 的 T2–T6 是**串行**（B 复用 A 的 sink）。

---

## §12 拒绝方案

| 编号 | 方案 | 拒绝理由 |
|---|---|---|
| R1 | 直接把 `MAX_LINE_CHARS` 提到 1 MiB | 单行输出与内存同时放大，`PER_TOOL_LIMIT_CHARS` 也会被单行顶穿 |
| R2 | 把 `MAX_LINE_CHARS` 做成可调参数（`maxLineChars`） | 宽度可调等价于把列窗口的一半提前搬到 A 侧，与 B 的 `super_read` 双写（§0.2 例外登记） |
| R3 | 提前提交（不等换行符） | 没有列窗口时不省 IO，只多一条「同一行可能被 `flush` 两次」的不变量 |
| R4 | `column` 用字节口径 | 与渲染的字符口径错位；`[truncated]` 标记位置也会漂移 |
| R5 | 预算按字符 | 编码放大不可控（GBK 与 UTF-8 同一字符不同字节） |
| R6 | 只截断不报告 | 现有 `[truncated]` 标记已承诺「可归因」，静默截断破坏该承诺 |
| R7 | `edit` 在截断回执上继续 | 静默只替换前 2000 字符，比报错更危险 |
| R8 | 把 `[long-lines]` 塞进 `data` 之外的旁路输出 | footer 是封闭集合，旁路会被 `edit` 的前缀剥离误吞 |
| R9 | 给 `read` 加新的失败态 | 破坏既有 `data` / `totalLines = null` 契约，前端与提示词都要改 |
| R10 | 在 P16 内做行内翻页（`column` / `maxLineChars`） | 与 B 的 `super_read` 双写同一套列算术，必然漂移；且引入两条无测试可兜的不变量（可见字节 ≠ 计费字节、同一行可能被 `flush` 两次） |

---

## §13 未决问题

| 编号 | 问题 | 影响 |
|---|---|---|
| OQ1 | ~~预算 8 MiB 是否按文件大小分档~~ | **已定稿（2026-11-18，T2）**：不分档，固定 `EOF_GRACE_BYTES = 8 MiB`。分档要在扫描前多取一次文件大小并引入两套阈值，收益不足以抵掉测试面 |
| OQ2 | `truncatedLines` 是否要区分「行数上限截断」与「单行截断」 | 现口径只数单行截断（§9 L9）；若拆分则 `[long-lines]` 的 `n` 需重新定义 |
| OQ3 | 闸门是**整文件**还是**逐行**（逐行需在回执里记被切行号区间，改动 §3.5 字段表） | §9 L8 的误伤面；I58 T5 前的可用性 |
| OQ4 | `[long-lines]` 是否要给「用 `grep` 定位」的引导语（当前只报事实） | 文案长度与模型行为 |
| OQ5 | 回执字段名 `line_window_max_chars` 是否改为 `line_window_width` | 与 I58 §7 的登记表一致性 |
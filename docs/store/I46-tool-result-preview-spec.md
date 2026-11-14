# I46-tool-result-preview — 落盘回填预览由「只留头」改为「头尾中间省略」

| 项 | 值 |
|---|---|
| 状态 | Draft（实施规格，不含进度记录） |
| 日期 | 2026-11-14 |
| 变更对象 | `src-tauri/crates/assistant-tools/src/storage.rs`（`PREVIEW_CHARS` / `persisted_reference`）；测试 `src-tauri/crates/assistant-tools/tests/storage.rs`、`src-tauri/crates/assistant-tools/tests/pipeline.rs` |
| 替代 | [I46 紧凑消息格式设计](./I46-agent-compact-message-format-design.md) §3.1 信封中 `<persisted-output>` 的**预览形态**、§3.3 预算表「回填」行的预览取值（该文档其余信封 / 省略 / 落盘生命周期结论继续有效） |
| 继续有效 | `PER_TOOL_LIMIT_CHARS` / `PER_ROUND_LIMIT_CHARS` 落盘阈值与降序选择规则；`<persisted-output path="…">` 标签语法与「首行」位置；落盘目录布局与清理策略；`compact.persisted { path, bytes }` wire 字段 |
| 前端影响 | **无代码改动**。`parsePersistedOutput` 的正则仍匹配，`trimPersistedTag` 的前缀裁剪仍有效，`shouldCollapseResult` 的默认折叠态不变 |
| 契约影响 | **无**。不改 DTO、不改事件载荷，**不需要** `npm run contract:gen` |

> 本文只回答一件事：**落盘后回填给模型与落库的那段预览文本，取原文的哪一段、多大、怎么标注省略**。
> 落盘阈值（何时落盘）、目录与生命周期（文件存哪、多久清理）、`compact` 信封（结构化字段）都不在本文范围。

---

## 1 变更摘要

一句话：**预览从「原文前 2000 字符」改为「头 600 + 省略标记 + 尾 600」，总量从 2000 降到 1200。**

| 维度 | 旧行为 | 新行为 |
|---|---|---|
| 取哪一段 | 纯头部 `content.chars().take(2000)` | 头 600 字符 + 尾 600 字符，中段丢弃 |
| 预览总量 | 2000 字符 | 1200 字符（`PREVIEW_HEAD_CHARS + PREVIEW_TAIL_CHARS`） |
| 省略说明 | **无**。模型只看到截断文本，无法判断「被砍了多少」 | 单独一行标记：`[… N chars omitted; full output in the file above …]` |
| 尾部信息 | 全部丢弃（工具自有 footer 一并丢失） | 保留 600 字符，覆盖工具 footer（`[truncated: showing …]` / `[encoding: …]`） |
| 短结果 | 不落盘，不涉及 | 不落盘；万一落盘（`total ≤ 1200`）原样回填，**不加标记** |
| 落盘阈值 | 单结果 50 000 / 单轮 200 000 字符 | 不变 |
| 目录与清理 | `tool-results/<会话哈希>/…`、mtime 年龄清理 | 不变 |

## 2 现状事实

| # | 事实 | 代码位置 |
|---|---|---|
| F1 | 预览是单一常量 `PREVIEW_CHARS = 2_000`（字符） | `src-tauri/crates/assistant-tools/src/storage.rs` |
| F2 | `persisted_reference(path, content)` 只取头部：`content.chars().take(PREVIEW_CHARS)`，其余全部丢弃，**不写任何省略说明** | 同上 |
| F3 | 落盘发生在事件发射之前，`Finished.result` 与落库 tool 行正文已经是预览，二者逐字节相等（门禁 1） | `src-tauri/crates/assistant-tools/src/pipeline.rs`（`persist` → `report`） |
| F4 | 落盘标签解析是「首个 `<persisted-output` + 首个 `>`」，因此标签**必须在首行且自闭合**，预览正文不得包含会抢先闭合的 `>` 形态 | `src-tauri/crates/assistant-tools/src/compact.rs`（`parse_persisted_tag`） |
| F5 | 省略三态解析用 `rfind("[truncated:")` 从**文本尾部**回溯，因此工具自有的 `[truncated: …]` footer 必须落在尾部才能被识别 | `src-tauri/crates/assistant-tools/src/compact.rs`（`parse_truncation`） |
| F6 | 关键信息普遍在尾部：`glob` / `grep` 的省略 footer、`read` 的 `truncation_notice`、命令行类输出的错误栈与末行，都是文本最后一段 | `src-tauri/crates/assistant-tools/src/os/agent_glob.rs`、`os/agent_grep.rs`、`os/agent_read.rs`、`os/output.rs` |
| F7 | 前端反解预览：`parsePersistedOutput` 用 `^<persisted-output path="([^"]+)">\r?\n?([\s\S]*)$` 捕获标签后的**全部剩余文本**；`trimPersistedTag` 按已知前缀裁剪；`resultStats` 统计整段预览的行数与字节 | `src/chat/tool/toolCallDisplay.ts` |
| F8 | 前端默认折叠阈值是 12 行 / 1024 字节，现有 2000 字符预览**已经超阈**（默认折叠）；降到 1200 字符后仍超阈，折叠态不变 | 同上（`RESULT_COLLAPSE_LINES` / `RESULT_COLLAPSE_BYTES`） |
| F9 | 存储后端只负责「原样存全文 + 返回定位符」，预览与替换策略归 Pipeline，存储接口不感知预览形态 | `src-tauri/crates/assistant-tools/src/storage.rs`（`ToolResultStorage::persist`） |

## 3 目标与非目标

**目标**

1. **尾部可见**：让模型与用户在预览里看到工具自己写的省略 footer 与错误末行，而不是只看到开头。
2. **省略可判定**：预览必须自述「还有 N 字符在文件里」，与工具自有的「结果被采集侧截断」区分开，不让两种省略混为一谈。
3. **总量下降**：预览同时进 DB 与模型上下文，总量从 2000 降到 1200，单条落盘结果省 800 字符。
4. **零契约变更**：不改 DTO、不改事件、不改标签语法，前端不动。
5. **确定性预算**：预览长度只由常量决定，不依赖内容形状（不做行对齐、不做按工具分叉）。

**非目标**

- 不改落盘阈值（`PER_TOOL_LIMIT_CHARS` / `PER_ROUND_LIMIT_CHARS`）、不改降序选择规则。
- 不改落盘目录、命名、清理与生命周期。
- 不改 `compact` 信封字段与前端渲染。
- 不为预览引入二次摘要（摘要会与历史文本分叉）。
- 不迁移 DB 里已存的旧预览（旧行保持 head-only）。

## 4 参考项目结论与选型

| 项目 | 做法 | 结论 |
|---|---|---|
| codex | 统一中间截断：保留头尾、丢中段、插标记；`split_budget` 默认**对半切分**；边界按字符安全位置回退 | **直接采纳**形态与对半切分：我们无法按工具预知「哪一半更有用」，对称是唯一不引入工具耦合的默认值 |
| codex | 截断标记与「上游不完整」是两件事：持久化截断有独立文案（`… command output truncated for persistence …`） | **采纳**语义分离：框架的预览省略不得复用 `[truncated:` 语法 |
| deepseek-harness | `TextRetainer` 按字节保头 / 尾 / 首尾；`truncated` 只表示「被预算省略了本可用的内容」，**不表示上游不完整** | **采纳**语义边界；不采纳其字节预算（我们是字符预算，见 §5.4） |
| deepseek-harness | 超限结果落盘、正文留 locator、footer 指向文件 | 与现状一致，不涉及 |
| OpenBitFun | 只解析 harness 文本，`(...)` footer 表示被截断 | 无新增可借鉴项 |

未采纳的方向见 §9。

## 5 规范

### 5.1 预览形态

落盘后的回填文本（也是落库正文与 `Finished.result`）：

```
<persisted-output path="…/tool-results/<ns>/<ns>-<tool>-<call_id>.txt">
<head 600 字符>
[… <omitted> chars omitted; full output in the file above …]
<tail 600 字符>
```

- 第 1 行**恒为** `<persisted-output path="…">`，无 `bytes` 属性（现状不变），单行、自闭合、无换行。
- head / tail **原样保留**原文对应片段，不改字节、不做行对齐、不补省略号。
- 标记行**独占一行**，前后各一个 `\n`，全段**恰好出现一次**，且只在 `total > PREVIEW_CHARS` 时出现。
- `total ≤ PREVIEW_CHARS` 时回填 `标签 + \n + 原文全文`，**不加标记**（此时不存在省略）。

### 5.2 常量

| 常量 | 旧 | 新 | 说明 |
|---|---|---|---|
| `PREVIEW_HEAD_CHARS` | — | `600` | 头部窗口 |
| `PREVIEW_TAIL_CHARS` | — | `600` | 尾部窗口 |
| `PREVIEW_CHARS` | `2_000` | `PREVIEW_HEAD_CHARS + PREVIEW_TAIL_CHARS` | 保留为**合计**常量，既有引用（测试）继续可用 |

取值理由：600 字符足以覆盖最长的工具 footer（`grep` / `glob` 的省略提示约 100–150 字符单行，`read` 的 `[truncated: …]` 与 `[encoding: …]` 各自独立成行），并留出上下文余量；对半切分沿用 codex 的 `split_budget` 默认值。

### 5.3 算法

```
total   = content.chars().count()
if total <= PREVIEW_CHARS:
    return 标签 + "\n" + content
head    = content.chars().take(PREVIEW_HEAD_CHARS).collect()
tail    = content.chars().skip(total - PREVIEW_TAIL_CHARS).collect()
omitted = total - PREVIEW_HEAD_CHARS - PREVIEW_TAIL_CHARS
return 标签 + "\n" + head + "\n[… {omitted} chars omitted; full output in the file above …]\n" + tail
```

签名不变：`persisted_reference(path: &str, content: &str) -> String`。调用点不变（`pipeline.rs` 的 `persist` 后回填处）。

### 5.4 不变量

| # | 不变量 | 违反后果 |
|---|---|---|
| I1 | 第 1 行恒为 `<persisted-output path="…">`，且 `>` 之前不含其他 `>` | `parse_persisted_tag` 取首个 `>`，标签被截断 → 历史回退丢失 `persisted` |
| I2 | 预览总字符 ≤ `PREVIEW_CHARS` + 标记行长度；head 与 tail 各 ≤ 600 | 预算失效，DB 与上下文体积不可控 |
| I3 | **框架新增的省略标记不得包含 `[truncated:` 子串**；工具自有的 `[truncated: …]` footer 可以且应当出现在 tail 中 | `parse_truncation` 会把「预览被省略」误判成「结果被截断」，污染 `compact.truncated` |
| I4 | 标记行独占一行，不与 head / tail 粘连 | 模型读到 `…tail…[… N chars omitted…]` 混排，可读性塌陷 |
| I5 | 全部按 `chars()` 计数与切片，不做字节切片 | 多字节字符（CJK、emoji）会被切成半个码位，输出非法 UTF-8 |
| I6 | 落盘内容与定位符**不受影响**：文件里仍是全文，`PersistedRef.bytes` 仍是全文字节数 | 预览改写误伤落盘产物 |
| I7 | 标签不新增属性（保持无 `bytes`） | 与 I46 未落地的 `bytes` 属性项重复定义，产生两套语法 |

> 关于 I3 的边界：tail 里出现工具 footer 是**期望行为**——`parse_truncation` 正是靠尾部 `rfind` 才能读到它。禁止的只是框架自己用同一语法写预览省略。

### 5.5 边界条件

| 输入 | 期望输出 |
|---|---|
| `total = 0` | 标签 + `\n`（不落盘，实际不可达；防御性分支不 panic） |
| `total = 1 200` | 标签 + 全文，无标记 |
| `total = 1 201` | 头 600 + 标记（`omitted = 1`）+ 尾 600 |
| `total = 50 001` | 头 600 + 标记（`omitted = 48 801`）+ 尾 600 |
| 含 CJK / emoji | 按字符精确切分，无 panic、无半个码位 |
| head 或 tail 内自带换行 | 原样保留，标记行仍独占一行 |
| 原文尾部是工具 `[truncated: showing 30 of 159 items …]` | 该 footer 落在 tail 内，`parse_truncation` 仍能解析 |

## 6 影响面与改动清单

| # | 文件 | 改动 | 验收 |
|---|---|---|---|
| C1 | `src-tauri/crates/assistant-tools/src/storage.rs` | 常量替换（§5.2）；`persisted_reference` 按 §5.3 重写；注释同步说明「头尾中间省略 + 标记不得复用 `[truncated:`」 | 单测覆盖 §5.5 全部分支 |
| C2 | `src-tauri/crates/assistant-tools/tests/storage.rs` | `preview_is_capped_at_preview_chars` 改为断言：首行标签、head 600、标记行存在且 `omitted` 数值正确、tail 600 | 精确断言字符数与标记内容 |
| C3 | `src-tauri/crates/assistant-tools/tests/pipeline.rs` | `large_result_is_persisted_with_preview` 的长度断言由 `prefix + PREVIEW_CHARS` 改为 `prefix + PREVIEW_CHARS + 标记行长度`，并断言 tail 内容存在 | 门禁 1（`Finished.result` 与落库正文相等）继续成立 |
| C4 | 新增单测（落在 C2 文件内） | ①短结果无标记；②`total = 1201` 时 `omitted = 1`；③UTF-8 多字节精确切分；④标记不含 `[truncated:` 且尾部工具 footer 仍被 `parse_truncation` 解析 | 四条全部通过 |

**不改**：`pipeline.rs` 的 `select_persist_targets` 与落盘触发；`storage.rs` 的 `persist` / `cleanup_expired_results`；`compact.rs`；任何前端文件；任何 DTO。

## 7 兼容性

| 面 | 影响 | 处置 |
|---|---|---|
| 存量 DB 行 | 旧预览是 head-only、无标记 | 不迁移。读取路径只依赖首行标签，head-only 与 head+tail 都能解析；差异只体现在「能看到多少尾部信息」 |
| `parse_persisted_tag` | 只读首行标签 | 无影响 |
| `parse_truncation` | 尾部 `rfind` | 新形态**改善**命中率（旧形态把 footer 砍掉了，旧行本就解析不到；新行能解析到） |
| 前端 `parsePersistedOutput` | 捕获标签后全部文本 | 无影响 |
| 前端 `resultStats` / `shouldCollapseResult` | 统计的是预览，不是全文（现状即如此） | 1200 字符仍超 1024 字节阈值，默认折叠态不变 |
| 模型侧 | 少 800 字符头部、多 600 字符尾部与一行省略说明 | 净收益：看到 footer 与错误末行 |
| wire / DB schema | 无字段变化 | 不需要 `contract:gen` |

回退方式：还原 `PREVIEW_CHARS` 常量与 `persisted_reference` 旧实现即可，无数据面残留（不写额外字段、不做迁移）。

## 8 验收门禁

| # | 门禁 | 验证方式 |
|---|---|---|
| G1 | 落盘结果回填文本首行恒为 `<persisted-output path="…">` | `tests/storage.rs` 断言 `starts_with` |
| G2 | 预览总字符 = `PREVIEW_CHARS` + 标记行长度，且 head / tail 各 600 | `tests/storage.rs` 精确字符数断言 |
| G3 | 标记行的 `omitted` 数值 = `total − 1200`，与原文实际字符数一致 | `tests/storage.rs` 数值断言 |
| G4 | 框架省略标记不含 `[truncated:`；工具自有 footer 在 tail 内且 `parse_truncation` 可解析 | 新增单测（C4-④） |
| G5 | `total ≤ 1200` 时不出现标记 | 新增单测（C4-①） |
| G6 | 多字节内容切分不 panic、字符数精确 | 新增单测（C4-③） |
| G7 | `Finished.result` 与落库 tool 行正文逐字节相等 | `tests/pipeline.rs` 既有门禁 1 用例 |
| G8 | 落盘文件仍为全文、`PersistedRef.bytes` 仍为全文字节数 | `tests/storage.rs` 既有 `file_storage_writes_the_full_content` |
| G9 | 落盘阈值与选择规则零改动 | `tests/storage.rs` 既有 `select_persist_targets` 三例 |

## 9 不采纳的方案

| 方案 | 不采纳理由 |
|---|---|
| 按工具类别分预算（bash 偏尾、`read` 偏头） | `persisted_reference` 是纯函数 `(path, content)`，加工具名要连带改调用点与两处测试；收益边际，等实际观察到某类工具预览失效再做 |
| 按行数 / 条目保留（harness 的 `ItemRetainer`） | 需要工具结果的结构化条目；我们的 `assistant_text` 是扁平字符串，前提不存在 |
| 改为字节预算 | 采集侧与回填侧现有预算都是字符口径，混入字节会造成两套口径；字符口径天然 UTF-8 安全 |
| 向行边界对齐切分 | 预览长度变得依赖内容形状（单行可能极长），预算失去确定性 |
| 复用 `[truncated:` 语法写预览省略 | 与 `compact.truncated` 的语义冲突（§5.4 I3），模型会把存储预览省略误读成结果被截断 |
| 保留 2000 总量、只改形态 | 预览同时进 DB 与模型上下文，总量不减则 DB 与上下文成本不减 |

## 10 后续项（不在本文范围）

| 项 | 说明 |
|---|---|
| 落盘阈值下调 | 预览是小头，`PER_TOOL_LIMIT_CHARS = 50 000` 才是 DB 体积大头；下调阈值会让 DB 变小、文件数变多，与预览大小是反向杠杆，需单独评估 |
| `<persisted-output>` 补 `bytes` 属性 | I46 参考记录 §3.1 已定义、代码未落地；落地时须与本文字段保持同一处定义 |
| 清理逻辑补「修剪空会话目录」 | 「目录不存在短路」已落地（缺失根目录按正常空态静默返回，不告警不计数，见 `cleanup_skips_missing_root_silently`）；剩余未做的是整体修剪空会话目录，现状仍只删文件、保留空目录 |
| 文档同步 | [I46 节点布局设计](./I46-agent-node-layout-design.md) §9 处置表中「预算归属」条目，在本文落地后由「保留有效」改为「以本文为准」 |
# I38c-edit — `edit`（修改工具）

| 项 | 值 |
|---|---|
| 状态 | 现行（P5 工具分册） |
| 日期 | 2026-11-06 |
| 上级 | [I38c-os-tools-spec.md](./I38c-os-tools-spec.md)（工具清单 / 共同结构 / 验收标准） |
| 任务 | [I38c-os-tools-tasklist.md](./I38c-os-tools-tasklist.md) C3.8–C3.13 |
| 实现 | `src-tauri/crates/assistant-tools/src/os/agent_edit.rs` |
| 注册名 | `edit`（旧名 `fs_edit`） |
| 相关 | [I38c-read](./I38c-read-spec.md) §3.1（读取回执键）、[I38a §4-C6](./I38a-tool-control-layer-spec.md) |

> 共同约束见上级 §4，本文不重复。本文是全阶段风险最高的工具：匹配与写回做错会静默损坏文件，实现时以本文为准。

## 1 安全标记

| action | read_only | destructive | open_world | audience | 并发 |
|---|---|---|---|---|---|
| `edit` | 否 | 否 | 否 | `BOTH` | 串行（`is_concurrency_safe = false`）：仅保证同轮工具批次不并行，**不提供** per-target 锁（§5） |

`permission_intents` → `edit` + 目标路径。

## 2 input_schema

```json
{
  "type": "object",
  "properties": {
    "path": { "type": "string" },
    "old_string": { "type": "string", "description": "要替换的原文，从最近一次 read 结果原样复制（不含行号前缀）" },
    "new_string": { "type": "string", "description": "替换后的内容，可为空以删除该段；缩进风格与 old_string 保持一致" },
    "replace_all": { "type": "boolean", "description": "允许替换全部匹配（默认 false）" }
  },
  "required": ["path", "old_string", "new_string"]
}
```

## 3 匹配流水线

渐进匹配，fail-closed：逐档尝试，命中即停；**任一档命中多处立即返回歧义错误，不自动取首个**。

匹配语义采用**子串匹配**（OpenBitFun 形态），不是 codex 的行数组 + 补丁定位形态；空白宽容度取自 codex 的三层退让。

| 档 | 规则 | 解决的问题 | 参考来源 |
|---|---|---|---|
| 0 | 前置拒绝：`old_string` 为空、`old_string == new_string`、目标是目录或不存在 | 无意义写回 | OpenBitFun `apply_match_and_replace`（空 `old` 拒绝）、`EditContentError::no_change` |
| 1 | 原文精确匹配（含缩进） | 正常路径 | 两者共有 |
| 2 | 行号前缀剥离：**仅当 `old_string` 每个非空行都形如「空格 + 数字 + tab 或 `→`」时启用**，否则跳过本档 | 整段复制自 Read 工具输出 | OpenBitFun `sanitize_read_tool_copied_text` → `read_line_prefix.rs::strip_read_line_number_prefix` |
| 3 | 尾随换行变体：`old_string` 不以 `\n` 结尾、且文件中存在 `old_string + "\n"` 时，`old_string` / `new_string` 各补一个 `\n` | 替换末尾行时丢掉行尾换行 | OpenBitFun `edit_string_candidates` 的 newline 分支 |
| 4 | 行尾空白不敏感（逐行 `trim_end` 后比较） | 模型丢掉行尾空格 | codex `seek_sequence` 第 2 轮（rstrip pass） |
| 5 | 首尾空白不敏感（逐行 `trim` 后比较） | 缩进风格不一致 | codex `seek_sequence` 第 3 轮（trim pass） |
| 6 | 归一化匹配：`\r\n → \n`、弯引号（`U+2018/2019 → '`、`U+201C/201D → "`）、制表符 ↔ 2/4 空格（前导空格转 tab 仅在该行前导为纯空格且空格数能被 `tab_width` 整除时生效） | CRLF 文件、富文本粘贴、tab/space 混用 | OpenBitFun `normalize_string`（CRLF）+ `find_actual_string`（弯引号）+ `convert_tabs_to_spaces` / `convert_leading_spaces_to_tabs`（tab↔空格）；codex `seek_sequence` 第 4 轮 `normalise`（Unicode 标点 → ASCII） |

**档位顺序理由**：档 2 / 3 是**结构性变换**（要么全行带前缀、要么补一个确定的 `\n`），误判风险接近零，因此排在空白宽容档之前；档 4–6 逐级放宽、误命中概率递增，越靠后越保守。OpenBitFun 把前缀剥离放在第 2 位、tab↔空格放在最后，与本次顺序一致。

**不采纳的参考特性**：

| 特性 | 出处 | 不采纳理由 |
|---|---|---|
| EOF 锚定（`eof = true` 时从文件尾反向定位） | codex `seek_sequence` | 属于多 hunk 补丁的定位语义，单次替换无「末尾上下文」需求；且会与 `replace_all` 冲突 |
| 多 hunk 补丁语法（`*** Begin Patch` 等） | codex `apply-patch/src/parser.rs` | 见[上级 §11 OQ4](./I38c-os-tools-spec.md) |
| 空 pattern 视为 `Some(start)` 无操作 | codex `seek_sequence` | 与档 0 的 fail-closed 冲突；空 `old_string` 必须报错 |

**不变量**：

1. 匹配在**归一化视图**上进行，写回使用命中的**真实原文片段**（按字符偏移把归一化命中映射回原文），绝不把归一化结果写进文件。参考 OpenBitFun `find_actual_string`：弯引号归一为逐字符 1:1 映射，因此命中在归一化文本中的字符偏移与原文一致，可安全回取。
2. 档 2–6 只变换 `old_string` / `new_string` 的**候选形态**，不预改写文件内容；候选必须在该档上重新精确命中才允许写回。
3. 每档的判定顺序固定：先判断命中数，再决定成功 / 歧义；歧义优先于任何写回。
4. 文件内容只在匹配时做 `\r\n → \n` 归一（`normalize_string`），不做其它预改写。

## 4 结果与错误

| 情况 | 行为 | 参考来源 |
|---|---|---|
| 0 匹配（全部档失败） | `Business`；错误体含「最近片段」（按最长公共前缀 / 后缀打分取 1 段 ±2 行）+ 外部改动提示 + 行号前缀 / 截断标记提示 | OpenBitFun `build_not_found_diagnostics`（`longest_shared_prefix_len` / `longest_shared_suffix_len` 打分 + `snippet_context` 取 ±2 行）；`contains_read_tool_line_prefixes` / `contains_read_truncation_marker` |
| 1 匹配 | 替换并写回；返回 `start_line` / `old_end_line` / `new_end_line` / `match_count` 与 diff 片段 | OpenBitFun `EditFileResult`（`start_line` / `old_end_line` / `new_end_line`） |
| 多匹配且 `replace_all = false` | `Business`；错误体含匹配次数 + 最多 5 段 ±2 行上下文，提示「扩大 `old_string`」或「用 `replace_all`」 | OpenBitFun `apply_match_and_replace` 的多匹配错误文案 + `match_contexts` |
| 多匹配且 `replace_all = true` | 全部替换，返回替换次数与首处行号 | 同上 |
| 命中但 `new_string` 与命中片段完全相同 | `Business`（no-change），不写盘 | OpenBitFun `EditContentError::no_change` |

## 5 写回

- **行尾**：`uses_crlf = 文件内容含 \r\n`；命中后先归一为 LF 再做替换，写回时若 `uses_crlf` 则整体 `\n → \r\n`。纯 LF 文件不做任何转换，保证不产生混合行尾文件（参考 OpenBitFun `apply_match_and_replace`）。
- **BOM / 编码**：全程使用原始解码内容，不 trim、不剥离 BOM；`old_string` 命中即写回，不做字节级重排。
- **原子写**：同目录 `tempfile::NamedTempFile` 落盘 → `sync_all()` 刷盘 → `persist` 覆盖目标，崩溃不留半截文件，也不留下「存在但为空」的文件；`tempfile` 已是 workspace 依赖，不新增。参考实现中 deepseek-harness-orb 的 `ctx.fs` 提供「stage → fsync → publish」原子变更（`packages/fs/fs-local/README.md`），OpenBitFun / codex 是直接 `fs::write`；本项目统一走原子写并采纳其 `fsync`。
- **不做 per-target 锁**：并发 `edit` 同一文件时，原子 rename 只保证不产生半截文件，不保证先后顺序；后写者覆盖前者且无人察觉。参考实现的 per-target FIFO 锁需要跨调用存活的共享状态，P5 不引入（理由见 [write §5](./I38c-write-spec.md)）。跨会话冲突由审批卡片与 `read` 回执修订号暴露（§6）。
- **写后回执**：写回成功后按 [I38c-read §3.1](./I38c-read-spec.md) 的回执键记录新修订号（见 §6）。

## 6 读前置（freshness guard）

与 [I38c-read §3.1](./I38c-read-spec.md) 共用 `ReadReceiptStore`，不新增端口。**本次新增，参考实现无此机制**（OpenBitFun / codex 均允许直接改盘）：

- 本会话对该路径**无回执** → `Business`，提示先 `read`（禁止盲改）。
- 有回执但修订号（长度 + mtime 纳秒）已变 → `Business`，提示「文件已被外部修改，请重新 `read`」。
- `write` / `edit` 写回成功后**自己写入回执**（等价于「本会话已见过该文件」），因此「先 `write` 新建、再 `edit`」的链路不会被误拦。
- ToolBox 链路（`session_id = None`）同样适用，桶为全局。

## 7 审批展示

审批卡片必须展示 diff（新增 / 删除行数 + 片段），不能只展示路径。审批发生在 `execute` 之前，diff 只能按 `arguments` 渲染，可能与档 2–6 的实际命中片段存在空白差异——卡片需标注「按参数估算」。

## 8 参考

| 要素 | 参考项目 | 参考实现 | 采纳情况 |
|---|---|---|---|
| 匹配主流程 | OpenBitFun | `execution/tool-execution/src/fs/edit_file.rs`：`apply_edit_to_content`（候选遍历 + 首命中即停）、`apply_match_and_replace`（CRLF 归一 → 替换 → 回写 CRLF） | 采纳（§3 档 1 / 3、§5 行尾） |
| 候选生成 | OpenBitFun | 同文件 `edit_string_candidates` | 采纳（§3 档 2 / 3 / 6） |
| 行号前缀剥离 | OpenBitFun | `execution/tool-execution/src/util/read_line_prefix.rs`：`strip_read_line_number_prefix` / `read_tool_output_to_file_content` | 采纳（§3 档 2） |
| 归一化工具 | OpenBitFun | `execution/tool-execution/src/util/string.rs`：`normalize_string`（仅 CRLF）；同文件 `find_actual_string`（弯引号 1:1 映射）、`convert_tabs_to_spaces` / `convert_leading_spaces_to_tabs` | 采纳（§3 档 6 + §3 不变量 1） |
| 诊断提示 | OpenBitFun | 同文件 `build_not_found_diagnostics` / `match_contexts` / `snippet_context` / `longest_shared_prefix_len` / `longest_shared_suffix_len` / `contains_read_truncation_marker` | 采纳（§4） |
| 行级渐进匹配 | codex | `apply-patch/src/seek_sequence.rs`：精确 → `trim_end` → `trim` → `normalise`（Unicode 标点 → ASCII）四轮 | 采纳（§3 档 4 / 5 / 6） |
| EOF 锚定 | codex | 同文件 `seek_sequence` 的 `eof` 参数 | 不采纳（§3「不采纳的参考特性」） |
| 多 hunk 补丁 | codex | `apply-patch/src/{parser,file_update}.rs` | 不采纳（[上级 §11 OQ4](./I38c-os-tools-spec.md)） |
| 原子写 | deepseek-harness-orb | `packages/fs/README.md`「atomic mutations with an optional version guard」；`fs-local/README.md`「stage → fsync → publish」；`tool-fs/src/edit.ts` 走 `ctx.fs.writeText` | 采纳原子写与 `fsync`（§5）。**不采纳** 版本守卫（`replaceIfVersion`），理由见 [write §5](./I38c-write-spec.md)；**不采纳** per-target 锁，理由同 [write §5](./I38c-write-spec.md) |
| 读前置 guard | 无 | — | 本次新增（参考实现无此机制） |
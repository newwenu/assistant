# I38c-read — `read`（只读工具）

| 项 | 值 |
|---|---|
| 状态 | 现行（P5 工具分册） |
| 日期 | 2026-11-06（2026-11-07 修订：新增 §3.3 多编码支持，任务项 C9.6–C9.9） |
| 上级 | [I38c-os-tools-spec.md](./I38c-os-tools-spec.md)（工具清单 / 共同结构 / 验收标准；多编码跨工具约束见 §14） |
| 任务 | [I38c-os-tools-tasklist.md](./I38c-os-tools-tasklist.md) C2.7–C2.13（编码支持见 C9.6–C9.9） |
| 实现 | `src-tauri/crates/assistant-tools/src/os/agent_read.rs` |
| 注册名 | `read`（旧名 `fs_read`） |

> 共同约束（只允许经 `os::path::resolve` 做 IO、`Err` 由 `exec::invoke` 归一化、权限词汇表、同步 IO 包 `spawn_blocking`）见上级 §4，本文不重复。

## 1 安全标记

| action | read_only | destructive | open_world | audience | 并发 |
|---|---|---|---|---|---|
| `read` | 是 | 否 | 否 | `BOTH` | 安全（`is_concurrency_safe = true`） |

`permission_intents` 恒返回空 `Vec`。

## 2 input_schema

```json
{
  "type": "object",
  "properties": {
    "path": { "type": "string", "description": "File path to read" },
    "offset": { "type": "integer", "description": "Start line (1-based; 0 is treated as 1)" },
    "limit": { "type": "integer", "description": "Max lines to read (must be >= 1)" },
    "encoding": {
      "type": "string",
      "description": "Optional encoding label (e.g. gbk); omit to auto-detect, BOM wins"
    }
  },
  "required": ["path"]
}
```

## 3 行为

- 默认读全部内容，上限 1MB / 2000 行；`offset` + `limit` 支持行范围读取。`offset` 从 1 起（`0` 按 1 处理），`limit = 0` 拒绝（`Business`），与参考实现一致。
- **单行上限 2000 字符**，超出部分截断并在该行尾追加 ` [truncated]`（参考 `max_line_chars = 2000`）；不得静默丢弃，也不得把截断后的行当成完整内容返回。
- 超限时返回截断提示 + 实际行数 / 字节数（复用 `os/output.rs`），不得静默丢弃。
- 二进制检测：**顺序固定**为「BOM → 显式 `encoding`（无 BOM 时）→ 二进制判定（原始字节前 8KB 含 NUL）→ UTF-8 整文件校验 → 编码猜测」，详见 §3.3 与上级 §14.3。判定在 UTF-8 校验之前但对带 BOM 的文件与显式声明让路：带 BOM 的 UTF-16 不会被 NUL 误杀，纯 ASCII 的无 BOM UTF-16 也不会被当成合法 UTF-8 静默读入。
- 空文件返回成功 + 空内容，不报错。

### 3.1 读取回执

同一会话内，对同一路径 + 同一行范围 + 文件修订号未变 → 返回「已在上文返回」，不回传内容；修订号变化则重读并刷新回执。端口为 `ReadReceiptStore`（P2 定义于 `assistant-tools/src/storage.rs`）。

**回执键必须含会话维度**：P2 的 `ReadReceiptKey` 当前只有 `path` / `start_line` / `end_line`，与「同一会话内」不符——两个会话读同一文件会互相抑制。P5 给键补 `session_id: Option<String>`，取值来自 `ToolUseContext.session_id`（agent 链路由 `PipelineToolExecution` 填 `conversation_id`，ToolBox 链路为 `None`，全部 ToolBox 调用归入同一桶）。该字段是**跨阶段契约变更**，已回写 [I38a §4-C6](./I38a-tool-control-layer-spec.md)（roadmap §7 第 7 条）。

**修订号算法**：文件长度 + 修改时间（纳秒），不做整文件哈希——哈希对大文件是纯开销，且回执只需判断「是否可能已变」。

**破坏性操作主动失效**：修订号无法识别「删除后重建且长度 + mtime 恰好相同」的情况（保时间戳的复制 / 解包）。`delete` 与 `move` 成功后调用 `ReadReceiptStore::invalidate_path`，按组件级前缀清除回执，本工具无需感知（见 [I38c-delete §3.6](./I38c-delete-spec.md)）。

**编码随回执（C9）**：`ReadReceipt` 增 `encoding: String`（实际生效编码的规范名）与 `has_bom: bool`。`read` 是**唯一编码发现点**，`edit` / `write` 只消费回执、不重判（上级 §14.2 / §14.5）。键结构与修订号算法不变，编码不参与失效判定（上级 §14.9 OQ9）。属跨阶段契约变更，已回写 [I38a §4-C6](./I38a-tool-control-layer-spec.md)。

### 3.2 输出格式（被 `edit` 依赖，不得自由变动）

正文行有两个要素会被 `edit` 消费，改动会让 `edit` 静默失效：

| 要素 | 产出格式 |
|---|---|
| 行号前缀 | `{line_number:>6}\t{line}`：行号 1-based、6 列右对齐、制表符分隔；行号超过 6 位时自然溢出（前导空格数不固定） |
| 长行截断 | 单行超 2000 字符时截断，行尾追加 ` [truncated]`（前置一个空格） |
| 编码 footer（C9） | 结果**末尾独占一行** `[encoding: <label>]`，可带 `(bom)` / `(guess)` 后缀；UTF-8 且无 BOM 时**不输出**（上级 §14.4） |

- **产出点唯一**：这三个形态只由 `read` 生成，`edit` 只检测、不生成。
- 两侧共用同一常量定义是**本次新增**：参考实现是 `read_file.rs` 内联字面量 `:>6`、`read_line_prefix.rs` 按「空格 + 数字 + 分隔符」容错解析，靠解析宽容而非编译期约束维持一致；本项目改为共享常量，让漂移在编译期暴露（风险见上级 §11）。
- 消费侧规则（`old_string` 如何剥离前缀、命中截断标记如何拒绝、`→` 容错分支、footer 如何识别）属于 `edit` 的入参处理，写在 [I38c-edit §3 档 2 / §4](./I38c-edit-spec.md)，本工具不重复定义。footer 是**结构化已知后缀**（`[encoding: ` 开头、`]` 结尾、独占一行），但 `edit` **不剥离**它，只在 0 匹配时据此给出诊断——行号前缀每行必有、模型无法避开，故必须剥离；footer 只在末尾一行、模型只复制正文即可避开，自动剥离会掩盖模型的误解（上级 §14.4 规则 4）。

### 3.3 多编码支持（C9）

跨工具约束见上级 §14；本节只写 `read` 侧落地点。

**检测流程**（固定顺序，前一步命中即停）：

| 步 | 判定 | 结果 |
|---|---|---|
| 1 | BOM（前 4 字节）：`EF BB BF` / `FF FE` / `FE FF` | 用该编码解码，`has_bom = true`，footer 标 `(bom)` |
| 2 | 无 BOM 且传了 `encoding` | 用该编码解码，`has_bom = false`，footer 标该 label |
| 3 | 原始字节前 8KB 含 `0x00` | 拒绝（`Business`）：二进制文件；提示「若确为无 BOM 的 UTF-16，请显式传 `encoding`」 |
| 4 | `std::str::from_utf8(bytes)` 通过 | UTF-8，`has_bom = false`，**不进入猜测**，footer 静默 |
| 5 | `EncodingDetector::feed` + `guess_assess` | 仅当 `is_confident == true` **且** 该编码解码整文件 `had_errors == false` 时接受，footer 标 `(guess)`；否则拒绝 |
| 6 | `IBM866 → WINDOWS_1252` 修正 | 沿用 `os/process_output.rs` 的既有规则；该逻辑抽到 `os/text_encoding.rs`，与进程输出共用一份 |

- **显式 `encoding` 的优先级**：`BOM` > 显式 `encoding` > 自动检测。被 BOM 覆盖时 footer 标 `(bom)`，如实反映实际来源。
- **二进制判定排在 UTF-8 校验之前**：纯 ASCII 的无 BOM UTF-16（`h\0i\0`）本身是合法 UTF-8，顺序反过来会被静默当成带 NUL 的文本读入，返回「成功」而模型无从察觉。
- **有损解码一律拒绝**：`had_errors == true` 即拒绝，不返回替换字符、不做 `?` 替换。
- **BOM 剥离**：解码后剥离前导 `U+FEFF` 再渲染，首行不再出现 BOM 字符；`has_bom` 记入回执供写侧补回。
- **`encoding` 参数只影响解码，不影响行号与截断**：行号仍是解码后的文本行号，`offset` / `limit` 语义不变。
- **检测成本**：UTF-8 文件只多一次前 8KB 的 NUL 扫描与一次整文件 `from_utf8` 校验（`read` 本就已读入全部字节），无额外 IO；chardetng 只在 UTF-8 校验失败后运行。
- **footer 只在本次结果尾部出现一次**，与 ` [truncated]` 截断提示并列但不混行：截断提示属于 `os/output.rs` 的既有输出，footer 追加在其后。

## 4 错误

| 情况 | 行为 |
|---|---|
| `path` 缺失 / 类型错误 | `validate_input` 拒绝（`Business`） |
| 路径越界或命中系统目录 | `os::path::resolve` 拒绝（`Business`） |
| 目标不存在 / 是目录 | `Business`，提示改用 `ls` |
| 前 8KB 含 NUL | `Business`，提示二进制文件；附「若确为无 BOM 的 UTF-16，请显式传 `encoding`」 |
| 显式 `encoding` label 非法 | `Business`，提示用 WHATWG 规范名（示例 `gbk` / `big5` / `shift_jis`），不枚举全表 |
| 自动检测不置信，或猜测编码解码有损 | `Business`，提示「传 `encoding`（如 `gbk`）」；**不**以替换字符降级返回 |
| 显式 `encoding` 解码有损 | `Business`，提示该 label 不适用于此文件；**不**做部分解码 |

## 5 参考

| 来源 | 做法 | 决定 |
|---|---|---|
| OpenBitFun `assembly/core/src/agentic/tools/implementations/file_read_tool.rs` | `default_max_lines_to_read = 2000`、`max_line_chars = 2000`、`DEFAULT_READ_MAX_TOTAL_CHARS = 64_000`；超长行截断并追加 ` [truncated]`；`offset` / `limit` / `tail` 三参数（`tail` 与 `offset` 互斥）；截断后提示「用 offset / limit 续读，Edit 只用这些 Read 结果里的文本」；`offset = 0` 视为 1，`limit = 0` 报错 | **部分采纳**：2000 行、单行 2000 字符、` [truncated]` 标记、`offset` / `limit` 边界语义、续读提示（§3 / §3.2）。**不采纳** `tail`（只服务日志尾部，`offset + limit` 已能表达，少一个参数就少一类互斥误用）与 64000 字符总上限（P2 三层预算已按单结果 50K 字符落盘，`read` 再设 64000 会与之重叠；这里用 1MB 兜底「一次读入超大文件」） |
| OpenBitFun `execution/tool-execution/src/fs/read_file.rs` | `ReadLineSelection` 逐行渲染 `format!("{line_number:>6}\t{line}")`，超长行尾追 ` [truncated]`；总字符上限命中后停止选中并置 `hit_total_char_limit` | **采纳**（§3.2）——`edit` 档 2 / 档 4 的前缀与标记检测必须与之一致 |
| deepseek-harness-orb `packages/fs/tool-fs/src/read-render.ts` | 行号前缀 `${line.number}: ${line.text}`（不补齐，冒号 + 空格），整段包在 `<path>` / `<type>` / `<content>` 信封内，末尾附 footer（`Showing lines a-b of N` / `End of file - total N lines`）；单行超 `readMaxLineLength` 截为 `... (line truncated to N chars)` | **部分采纳**：1-based 行号 + 续读提示（§3）。**不采纳** 信封与 footer——信封会让模型复制片段时带上 `<content>` 标签，与 `edit` 的纯文本匹配流水线冲突；续读信息已由 `os/output.rs` 的截断提示承担。截断文案统一用 OpenBitFun 的 ` [truncated]`，不引入第三种写法 |
| Claude Code（openhuman `bench/harnesses/prompts/claude-code/tools.json`） | `Read` 描述：「Results are returned using cat -n format, with line numbers starting at 1」；`Edit` 描述：「`old_string` must match the file exactly … **Strip the Read line prefix (line number + tab) before matching**」 | **采纳**行号前缀本身（§3.2）——这是「产出侧插前缀、消费侧要求剥离」的最直接外部证据，说明前缀是主流形态而非本项目独创。**不采纳**其 `cat -n` 具体排版（`{n}\t`，列宽随行号位数变化），改用 OpenBitFun 的定宽 6 列 |
| 前缀的 token 成本 | 无参考实现做省前缀的替代（三家均为逐行编号） | **保留前缀，成本已知**：每行 6 列 + `\t` ≈ 7 字符，2000 行约 14K 字符。收益是模型可精确引用行号、按 `offset` 续读、并在 `edit` 失败时给出定位提示；代价由 2000 行上限 + 回执去重（§3.1，同一范围不重复返回）压制 |
| OpenBitFun 文档转换（`anydoc`：PDF / Office → GFM Markdown，30s 超时） | 非文本文件先转 Markdown，再按 Markdown 行号读取 | **不采纳**：引入文档解析依赖与另一套行号语义（转换后行号不等于源文件行号，会让 `edit` 的行号假设失效），P5 只读纯文本，非文本统一拒绝 |
| 读取回执（会话维度 + 修订号 + 主动失效）与二进制检测 | 参考实现无对应机制 | **本次新增**（§3.1 / §3） |
| helix `references/7-text-encoding/helix-view/src/document.rs` | 编码检测顺序：先查 BOM → 不合法时 `chardetng` 统计猜测 → 回退 UTF-8；编码随文档生命周期保存，保存时按原编码写回；`encode` 失败即报错而非替换 | **采纳**检测顺序与「编码随文件生命周期」模型（§3.3 / §3.1）。**不采纳**其「编辑器会话内存态编码」的存储位置——本项目编码属于工具调用产物，随 `ReadReceipt` 走（上级 §14.5） |
| `encoding_rs` 官方 `decode` / `encode` 契约 | `decode_without_bom_handling` 返回 `(Cow<str>, had_errors)`；`encode` 同样返回 `had_errors` | **采纳** `had_errors` 作为唯一接受判据（§3.3 / 上级 §14.6 G2）：有损即拒绝，不做 `?` 替换 |
| ripgrep `references/7-text-encoding/ripgrep/crates/searcher/src/searcher/mod.rs` | `DecodeReaderBytesBuilder` + `utf8_passthru(true)` + `bom_sniffing(true)` + `strip_bom(true)`；`Encoding::new(label)` 做 label 合法性校验 | **部分采纳**：label 校验用 `Encoding::for_label_no_replacement` 的同一套表；`DecodeReaderBytes` 的流式转码只用于 `grep`（`read` 已全量读入，直接 `encoding_rs::decode` 更简单），见上级 §14.7 |
| `os/process_output.rs`（本项目存量） | `chardetng` 检测 + `encoding_rs` 解码 + `IBM866 → WINDOWS_1252` 修正 | **采纳并抽为共享模块** `os/text_encoding.rs`：进程输出与文件工具共用同一份检测器与修正规则，避免两处结论不一致（§3.3 步骤 5） |
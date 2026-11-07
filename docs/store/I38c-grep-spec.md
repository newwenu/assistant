# I38c-grep — `grep`（只读工具）

| 项 | 值 |
|---|---|
| 状态 | 现行（P5 工具分册） |
| 日期 | 2026-11-06（2026-11-07 一修：`encoding` 参数与两层探测，C9.17–C9.24；二修：输出预算与边界，C9.25–C9.30；三修：正则语义与参数面，§3.4） |
| 上级 | [I38c-os-tools-spec.md](./I38c-os-tools-spec.md)（工具清单 / 共同结构 / 验收标准；多编码跨工具约束见 §14） |
| 任务 | [I38c-os-tools-tasklist.md](./I38c-os-tools-tasklist.md) C2.23–C2.27（编码支持见 C9.17–C9.21；输出预算与边界见 C9.25–C9.30）。三修无代码改动（`pattern` 描述已同步），仅 §3.4 文档收敛，故不新增任务项 |
| 实现 | `src-tauri/crates/assistant-tools/src/os/agent_grep.rs` |
| 注册名 | `grep`（旧名 `fs_grep`） |

> 共同约束见上级 §4，本文不重复。

## 1 安全标记

| action | read_only | destructive | open_world | audience | 并发 |
|---|---|---|---|---|---|
| `grep` | 是 | 否 | 否 | `BOTH` | 安全（`is_concurrency_safe = true`） |

`permission_intents` 恒返回空 `Vec`。

## 2 input_schema

```json
{
  "type": "object",
  "properties": {
    "pattern": {
      "type": "string",
      "description": "Regex pattern (single-line match; e.g. 'fn main')"
    },
    "path": { "type": "string", "description": "Directory to search" },
    "include": {
      "type": "string",
      "description": "File glob filter (e.g. '*.rs')"
    },
    "context": {
      "type": "integer",
      "description": "Lines of context before and after each match (0-10, default 0)"
    },
    "encoding": {
      "type": "string",
      "description": "Optional encoding label (e.g. gbk) for the whole search; omit to search as-is"
    }
  },
  "required": ["pattern"]
}
```

`description` 只写「参数是什么 + 一个示例 + 会静默失败的单行匹配约束」；`context` 上限、`include` 深度语义、`pattern` 原样编译、正则语法边界属实现契约，写在 §3 / §3.2 / §3.4，不进 schema（判据见上级 §14.4「参数描述判据」）。

## 3 行为

- 依赖选型：`ignore`（遍历 + `.gitignore`）+ `grep-searcher`（读文件 + 行迭代）+ `grep-regex`（`regex` 后端）；`include` 用 `globset` 过滤。不走 `grep` 元 crate 与二进制 CLI。
- `path` 缺省时取 `workspace_root`；无 `workspace_root` 则返回明确错误；`path` 必须是目录（单文件定位由 `read` 承担）。
- **`pattern` 原样编译，不做 `trim`**：前导 / 尾随空格是正则的一部分（搜缩进、搜行尾空格都依赖它）。`trim` 只用于拒绝「全空白」输入，不改变编译用的字符串。
- **正则与参数面**：逐行匹配、不支持 look-around / backreferences、不新增开关（详见 §3.4）。
- 输出行形态固定：命中行 `path:line:text`，上下文行 `path-line-text`。两种分隔符与 `rg` 一致，上下文行不会被误读为命中行。行号**恒开**，不提供开关（模型拿行号去定位 / 续读是主要用途）。
- **`include` 的分隔符语义与 [glob](./I38c-glob-spec.md) 对齐**：`literal_separator(true)`，即 `/` 只匹配真实的层级分隔；pattern **不含 `/`** 时自动前置 `**/`，使 `*.rs` 仍能递归命中。结果：`*.rs` 全深度匹配、`src/*.rs` 只匹配 `src` 的直接子文件，两个工具对同一个 pattern 给出同一集合。
- 结果只含 workspace 内路径。
- **上限与截断**见 §3.2；**遍历顺序、跳过计数与单文件体积上限**见 §3.3。
- **内部时间上限**：搜索在 `spawn_blocking` 内同步运行，外层 `exec::invoke` 的 30s 到点只能放弃 future，blocking 线程无法被强杀（见 §4 与上级 §11）。因此遍历循环内按「外层剩余预算 − 500ms」设内部 deadline，检查点覆盖**每个文件开始前**与 `Sink::matched` / `Sink::context` 两个回调——后者是必需的：单文件内的海量上下文行不会经过 `matched`，只在 `context` 里出现。到点后返回**已收集结果**并在文案注明「已达时间上限，结果可能不完整，请缩小 `path` / `include`」——不报错、不丢弃已得结果。
- **二进制检测默认开**（C9）：`BinaryDetection::quit(b'\x00')`。命中 `Sink::binary_data` 时停止该文件，并把**该文件已收集的行整体丢弃**，计入 `binarySkipped`——NUL 之前也可能已有命中，逐行即时输出会把乱码混进结果，因此按文件缓冲、整文件丢弃（§3.3）。

### 3.1 编码支持（C9）

跨工具约束见上级 §14.7；本节只写 `grep` 侧落地点。

**原则**：`grep` **不猜测编码**。它只做两件事——按显式 `encoding`（或文件自带 BOM）搜索；对「scope 内存在非 UTF-8 文件」给出事实。逐文件 `chardetng` 猜测会引入第三个编码发现点，与 `read` / 回执的结论冲突（上级 §14.2）。

**搜索层配置**（复用 `grep-searcher`，不自行实现转码）：

| 配置 | 值 | 说明 |
|---|---|---|
| `binary_detection` | `quit(b'\x00')` | 新增。`SearcherBuilder` 默认 `None`（= 不做检测），必须显式打开 |
| `encoding` | `encoding` 参数 → `Encoding::new(label)` | `Option`；`Encoding::new` 返回 `Result`，非法 label → `Business` |
| `bom_sniffing` | 默认 `true`（不显式设置） | 带 BOM 的 UTF-16 无需 `encoding` 即可搜，且首行不含 `U+FEFF` |
| `utf8_passthru` | `build()` 内固定 `true` | 无 BOM 时零转码，UTF-8 路径无额外成本 |
| `mmap` | 默认 `never` | 保持关闭：转码需要读流（`DecodeReaderBytes`），mmap 与转码路径不兼容，开启只会让「何时转码」变成隐式分支 |
| `multi_line` | 保持关闭（§3.4） | 不影响编码路径 |

**label 校验前置**：`validate_input` 与 `execute` 必须用同一个构造函数（`SearcherEncoding::new`，内部走 `encoding_rs::Encoding::for_label_no_replacement`）。两处各写一套会分叉——`for_label` 接受、`for_label_no_replacement` 拒绝的 label（如 `iso-2022-kr`、`hz-gb-2312`）会变成「校验通过、执行才报错」，违反上级 §4 的 `validate_input` 约定。

**热路径（有命中）**：`trim_line_end` 先 `std::str::from_utf8`，失败才 `from_utf8_lossy` 并置 `lossy_lines = true`。合法 UTF-8 行因此比朴素写法**少一次分配**（`from_utf8_lossy` 内部本来就会先跑 `from_utf8`），且不再静默吞掉「这行不是 UTF-8」的事实。命中行与上下文行走同一个 `trim_line_end`，**两者都计入** `lossyLines`：context 行同样以替换字符渲染给模型，只统计命中行会让「结果里有乱码」这件事没有对应的计数。

**冷路径（零命中时的 probe）**：仅当 `match_count == 0`、未达内部 deadline **且未显式指定 `encoding`** 时执行，热路径零成本：

1. 逐文件读**前 8KB**到**同一个可复用缓冲**（不每文件新建 `Vec`），按 [read §3.3](./I38c-read-spec.md) 步骤 1–3 同一套前缀分类判定。
2. **跳过**：含 `0x00`（二进制）、有 BOM（已可搜）、合法 UTF-8。
3. 计数「非 UTF-8 且非二进制」文件；达 `PROBE_FILE_LIMIT = 200` 或 deadline 即停，置 `nonUtf8Probe.truncated = true`。
4. 计数 > 0 时输出 footer `[non-utf8: N file(s)]`；为 0 时**静默**（成功路径不给指令）。

**显式 `encoding` 时跳过 probe**：probe 的用途是「零命中可能是编码不对」，而 `encoding` 已是 scope 级显式声明——此时再报「scope 里有非 UTF-8 文件」等于暗示模型换编码重试，属于误导。`encoding` 已设时 `nonUtf8Probe` 恒为 `null`。

**结果 `data` 字段**：

| 字段 | 类型 | 语义 |
|---|---|---|
| `totalMatches` | `u32` | 命中**块**数（不含上下文行；当前逐行匹配，1 块 = 1 行） |
| `matches` | `string[]` | 渲染行数组，**含**上下文行（`path-line-text` 形态），按 §3.3 排序 |
| `truncated` | `bool` | 任一输出上限触发（§3.2），不区分原因 |
| `timedOut` | `bool` | 内部 deadline 触发（§3） |
| `binarySkipped` | `u32` | 命中 `binary_data` 的文件数（该文件不产出任何行） |
| `largeSkipped` | `u32` | 超过 `max_filesize` 的文件数（§3.3） |
| `unreadableFiles` | `u32` | 打开 / 读取失败的文件数（权限、占用、路径过长） |
| `lossyLines` | `u32` | 渲染时含替换字符的行数（命中行与上下文行都计；仅在未设 `encoding` 且该行非法 UTF-8 时非零） |
| `nonUtf8Probe` | `Option<{ files: u32, truncated: bool }>` | 零命中 probe 的结果；有命中、已设 `encoding` 或未执行时为 `null` |

**footer 形态**（独占一行、出现在结果尾部，上级 §14.4 规则 4 / 5 的标记集）：

| tag | 触发条件 | 形态 |
|---|---|---|
| `non-utf8` | `nonUtf8Probe.files > 0`（零命中） | `[non-utf8: N file(s)]` |
| `skipped` | `binarySkipped + largeSkipped + unreadableFiles > 0` | `[skipped: N file(s)]` |
| `lossy` | `lossyLines > 0` | `[lossy: N line(s)]` |

三个 tag 都是「只给事实」：不给「换个编码试试」「缩小 path」这类指令，动作留给 `description` 与错误文案。`skipped` 不展开三种原因的明细（明细在 `data`），避免 footer 变成一行长句。它们与 `read` 的 `[encoding: …]` 同属标记集；`edit` **不剥离**末尾 footer 行，仅在 0 匹配诊断里据此提示「这不是文件内容，请移除后重试」（上级 §14.4 规则 4）。

**已知限制**（不进 `description`，只在错误表与文档里说明）：未传 `encoding` 时非 UTF-8 文件按 UTF-8 逐字节匹配，命中行以替换字符渲染。这是「不猜测」的直接代价——要正确匹配就传 `encoding`。`description` 只保留一句「省略即按原样搜索」（上级 §14.4 提示边界）。

**为什么不复用 `read` 的编码检测**：`grep` 的 scope 可能有上千个文件，逐文件跑 `chardetng` 是 O(文件数 × 解码成本)，且「scope 内多编码并存」时没有任何单一结论可用。显式 `encoding` 是 scope 级声明，语义与 ripgrep `--encoding` 一致，行为可预测。

### 3.2 输出预算与截断

三个上限互相独立，任一触发即停止搜索并置 `truncated = true`：

| 上限 | 值 | 计量对象 | 触发后 |
|---|---|---|---|
| `MAX_MATCHES` | 100 | 命中**块**（`SinkMatch`；当前 1 块 = 1 行） | 停止；文案「showing the first 100 matching lines」 |
| `MAX_OUTPUT_LINES` | 300 | **命中行 + 上下文行 + `--` 分隔符**合计的渲染行数（分隔符与行同占预算，`context_break` 在预算耗尽时不再输出） | 停止；文案「output line limit (300) reached」 |
| `MAX_CONTEXT` | 10 | `context` 入参（前后各 N 行） | `validate_input` 直接 `Business`，不静默夹紧 |

- **为什么必须有总行数上限**：`MAX_MATCHES` 只数命中块，上下文行完全绕过它——`context: 1000000` 配上一个靠前的命中，就能把整个文件塞进结果（`break` 只在文件之间生效，单文件内部不会被中断）。总行数上限让「命中少但上下文多」与「命中多」落到同一口径，同时给 §3 的 deadline 之外再加一道确定性闸门（deadline 依赖机器速度，行数上限不依赖）。
- **为什么 `context` 单独设上限**：总行数上限能兜住输出，但 `context` 会先进入 `SearcherBuilder` 的上下文窗口并触发回溯与缓冲开销，超大值在执行前就该拒绝。拒绝而非夹紧：夹紧会让模型以为「传了 100 就真的给了 100 行上下文」，而结果其实被静默改写。
- `truncated` 的语义统一为「任一上限触发」，`data` 不新增 `hitLimit` / `lineLimit` 之类的原因字段——原因对模型没有决策差异，动作都是收窄 `path` / `include` 或降低 `context`。
- 截断文案与 `[skipped]` / `[lossy]` footer 各自独占一行，顺序为：结果行 → 截断 / 超时提示 → footer。

### 3.3 遍历顺序、跳过计数与体积上限

**顺序**：`ignore::Walk` 的产出顺序依赖文件系统，本工具**不做**「先收集全部路径再排序搜索」——那会让 deadline 在遍历阶段就被消耗，且对大仓库等于多一次全量遍历。因此：

- **截断子集可能与平台相关**：同一输入在不同文件系统上「前 100 条命中」可能来自不同文件。测试**不得**断言跨文件顺序，构造用例时用单文件或单目录。
- **呈现稳定**：单文件的行先进入该文件的缓冲，文件处理完成后按「路径字典序 → 行号升序」提交，同一子集的输出顺序固定、可复现（与 [glob §3](./I38c-glob-spec.md) 的排序取向一致）。

**跳过计数**（模型只看得到 `result_for_assistant`，计数必须进文本；`data` 同步给结构化值）：

| 计数 | 含义 |
|---|---|
| `binarySkipped` | 命中 `binary_data`（首个 `0x00`）的文件数；该文件已收集的行整体丢弃 |
| `largeSkipped` | 超过 `max_filesize` 的文件数 |
| `unreadableFiles` | 打开 / 读取失败的文件数（权限、被占用、路径过长） |

- 三者合并为 footer `[skipped: N file(s)]`，只要总和非零就输出，**零命中与有命中都输出**：跳过意味着「结果不完整」，与是否已有命中无关。明细只进 `data`。
- **单文件体积上限 `max_filesize = 16 MiB`**：单文件在 `spawn_blocking` 内无法被 deadline 中断，体积上限是唯一能在「一个文件」粒度收口的杠杆。代码文件极少超过 1 MiB，超限的多为生成物 / 数据文件，行级匹配价值低。ripgrep 的 `-M` 默认无上限，本项目为可中断性加此上限（§5）。判定在 `decide_file` 内用 `metadata.len()`，**不用 `WalkBuilder::max_filesize`**——后者在遍历层静默丢弃条目，`largeSkipped` 无从计数。
- **读取失败不报错**：单个文件失败不应让整次搜索失败（`path` 指向的大目录里总会有不可读文件）。失败计入 `unreadableFiles`，由 `[skipped]` footer 让模型知道结果不完整。

### 3.4 正则语义与参数面

**能力边界**：`pattern` 由 `grep-regex` 编译，取 `RegexMatcherBuilder` 默认配置（`case_insensitive` / `multi_line` / `dot_matches_new_line` / `fixed_strings` 均关，`unicode` 开）。支持字面量、字符类、量词（含懒惰）、分组与交替、`^` `$` `\b`、Unicode 类 `\p{...}`、内联标志 `(?i)` / `(?s)` / `(?U)` / `(?x)`；**不支持 look-around 与 backreferences**（编译期 `Business`）；**逐行匹配、不能跨行**，内联 `(?m)` 不解锁跨行（`SinkMatch` 仍按行切分）。编译期上限 `size_limit = 100 MiB` / `nest_limit = 250`，超限同样 `Business`，不 panic。

**参数面**：不新增 `-i` / `multiline` / `-A` / `-B` / `offset` / `output_mode` / `type`，逐项取舍见 §5。`multiline` 若重开须先按上级 §11 的 OQ 格式登记，并同步改 §3.2 的计量口径；`-i` 永不重开（内联 `(?i)` 等价）。

**校验归属**：两处各编译一次，共用 `RegexMatcher::new`：

| 入口 | 落点 |
|---|---|
| Agent（Pipeline，经 `validate_input`） | `ValidationResult::invalid`——可修正的输入错误，`blocks_input_rewrite = false`，不是安全不变量 |
| ToolBox（用户直调，不经 Pipeline） | `execute` 内 → `ToolResult::err` |

删掉任一处都会让对应入口在非法正则上缺收口，因此 `execute` 内的第二次编译不是冗余。

**错误文案**：`invalid regex '<pattern>': <引擎原文>`。不改写引擎报错、不追加 hint、不自动转义或降级为字面量——引擎原文已自述不支持项，而静默降级会把「语法写错」变成「搜不到」，与 fail-closed 取向相反。

## 4 错误

| 情况 | 行为 |
|---|---|
| `pattern` 缺失 / 类型错误 / `trim` 后为空 | `Business`，文案区分「缺失」与「只有空白」；空模式会匹配每一行、等于把整个 scope 塞进结果，因此**拒绝而非编译空模式** |
| 非法正则（含 look-around / backreferences 等引擎不支持的特性） | `Business`，文案固定 `invalid regex '<pattern>': <引擎原文>`，不改写、不自动修正（§3.4）；`validate_input` 与 `execute` 两处均拒绝；不 panic、不中断 turn |
| 基准目录越界 / 不存在 / 不是目录 | `Business` |
| `context` 超出 `0..=10` | `Business`，附上限（§3.2） |
| 达到内部 deadline | **不报错**：返回已收集结果 + 「结果可能不完整」提示（§3） |
| `encoding` label 非法 | `Business`，**在 `validate_input` 阶段**返回（与执行同一构造函数），附规范名示例（`gbk` / `big5` / `shift_jis`）；不静默退回 UTF-8 |
| 单文件读取失败 | **不报错**：计入 `unreadableFiles`，footer `[skipped: N file(s)]`（§3.3） |

## 5 参考

| 来源 | 做法 | 决定 |
|---|---|---|
| OpenBitFun `assembly/core/src/agentic/tools/implementations/grep_tool.rs` | schema：`pattern` / `path` / `glob` / `output_mode`（`content` / `files_with_matches` / `count`，**默认 `files_with_matches`**）/ `-A` / `-B` / `-C` / `context` / `-n` / `-i` / `type` / `head_limit`（默认 250）/ `offset` / `multiline`；内部 `tokio::time::timeout(30s)`，超时文案「Workspace search timed out after 30000ms; narrow the search path」 | **部分采纳**：`context`（等价 `-C`）、`glob` 过滤（本项目名 `include`）、行号输出、截断后建议收窄的文案形态、内部时间上限（§3）。**不采纳**：三态 `output_mode`（P5 恒 `content`；`files_with_matches` 用 `glob` 表达，`count` 价值低且可后续加）、`-A` / `-B` 单侧上下文（`context` 已覆盖主用例）、`offset`、`multiline`、`type`（`include` 按语言后缀过滤已够）、`-i`（可用内联标志 `(?i)` 表达，少一个参数就少一类误用） |
| 上限取舍 | 参考 `head_limit` 默认 250 / 本项目 100 | `content` 模式下 250 行会挤占上下文预算；100 条 + 明确的截断提示足以驱动模型收窄搜索 |
| **`pattern` 描述声明引擎边界**（三修） | OpenBitFun `grep_tool.rs`：`description` 为「Rust regular expression … Escape literal regex metacharacters. Look-around and backreferences are not supported.」 | **采纳**声明，**压缩为一句**（§3.4）：只保留「会失败 / 会静默零命中」的项，去掉「记得转义元字符」这类可由语法常识推导的提醒；参数面仍按 §3.4 收敛，不因描述变长而放宽 |
| **非法正则的判定方式**（三修） | deepseek-harness-orb `packages/fs/tool-fs-search/src/{grep,search-core}.ts`：`--regexp=<pattern>` 原样透传，靠解析 ripgrep stderr（`/regex parse error|error parsing glob/i`）判 `SEARCH_INVALID_PATTERN` | **不采纳**「解析 stderr 文案」：本项目在 `validate_input` / `execute` 内用 `RegexMatcher::new` 拿到类型化 `Err`；stderr 文案随引擎版本漂移，且同一条 stderr 无法区分「正则非法」与「glob 非法」（§3.4） |
| **输出预算**（二修） | 参考只限 `head_limit`（命中条数），上下文行不受限 | **本项目新增**：`MAX_MATCHES`（100 块）+ `MAX_OUTPUT_LINES`（300 行，命中 + 上下文合计）+ `MAX_CONTEXT`（10，超出即拒绝）。参考实现的 `context` 无上限，`-C 1000` 时输出量由调用方负责；本项目把它当预算问题（§3.2） |
| OpenBitFun `execution/tool-execution/src/search/grep_search.rs` | 自带结果 reducer、行内 hydration（`line_hydration.rs`，` [truncated]` 后缀）与连续行分隔符逻辑 | **不采纳** reducer 与行水合（P5 结果直接进上下文，由 P2 三层预算兜底）；**采纳**「不连续行之间加分隔符」的输出形态（与 rg 一致），避免 context 行被误读为连续片段 |
| 搜索层依赖 | 参考为自研搜索模块 | **采纳拆分 crate 选型**：`ignore` + `grep-searcher` + `grep-regex` + `globset`（定稿见上级 §11 OQ1） |
| **编码支持**（C9） | ripgrep | `references/7-text-encoding/ripgrep/crates/searcher/src/searcher/mod.rs`：`SearcherBuilder::build` 用 `DecodeReaderBytesBuilder::new().encoding(enc).utf8_passthru(true).strip_bom(true).bom_override(true).bom_sniffing(true)`；`Encoding::new(label)` 经 `encoding_rs::Encoding::for_label_no_replacement` 校验；`mmap` 默认 `MmapChoice::Never`。`crates/core/flags/hiargs.rs`：`BinaryDetection::quit(b'\x00')` | **采纳**：配置链（`binary_detection` / `encoding` / `bom_sniffing` / `utf8_passthru`）、label 校验表、mmap 关闭（§3.1）。**不采纳**：`--encoding` 的 CLI 外壳与 `encoding` 的 per-type override（本项目 scope 级单值足够）、逐文件 `chardetng` 猜测（会与 `read` 形成第二个编码发现点）、mmap 加速（与转码路径不兼容） |
| **零命中非 UTF-8 探测**（C9） | 无 | 参考实现无「搜索无结果时告知存在非 UTF-8 文件」的能力 | **本次新增**：两层探测（热路径 `from_utf8` 快速路径 + 冷路径 8KB 前缀有界 probe），见 §3.1 |
| **单文件体积上限**（二修） | ripgrep `-M` / `max_filesize`（默认无上限） | `crates/core/flags/hiargs.rs` 的 `WalkBuilder::max_filesize` | **部分采纳**：取 `16 MiB` 作为阈值（ripgrep 默认不设）。理由见 §3.3：单文件不可被 deadline 中断，需要体积闸门。**不用 `WalkBuilder::max_filesize` 本身**——它在遍历层静默丢弃条目，`largeSkipped` 无从计数；改在 `decide_file` 用 `metadata.len()` 判定，同一阈值、可计数 |
| **`include` glob 语义**（二修） | 本仓库 [glob](./I38c-glob-spec.md)（`literal_separator(true)`） | `agent_glob.rs` 的 `GlobBuilder` 配置 | **对齐**：grep 侧同样 `literal_separator(true)`，并在 pattern 不含 `/` 时自动前置 `**/`。原先 grep 用 `literal_separator(false)`，导致 `src/*.rs` 在 grep 里跨层级匹配、在 glob 里不匹配 |
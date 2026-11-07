# I38c-grep — `grep`（只读工具）

| 项 | 值 |
|---|---|
| 状态 | 现行（P5 工具分册） |
| 日期 | 2026-11-06（2026-11-07 修订：新增 `encoding` 参数与两层探测，任务项 C9.17–C9.21） |
| 上级 | [I38c-os-tools-spec.md](./I38c-os-tools-spec.md)（工具清单 / 共同结构 / 验收标准；多编码跨工具约束见 §14） |
| 任务 | [I38c-os-tools-tasklist.md](./I38c-os-tools-tasklist.md) C2.23–C2.27（编码支持见 C9.17–C9.21） |
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
    "pattern": { "type": "string", "description": "Regex pattern" },
    "path": { "type": "string", "description": "Directory to search" },
    "include": { "type": "string", "description": "File glob filter" },
    "context": { "type": "integer", "description": "Lines of context (default 0)" },
    "encoding": {
      "type": "string",
      "description": "Optional encoding label (e.g. gbk) for the whole search; omit to search as-is"
    }
  },
  "required": ["pattern"]
}
```

## 3 行为

- 依赖选型：`ignore`（遍历 + `.gitignore`）+ `grep-searcher`（读文件 + 行迭代）+ `grep-regex`（`regex` 后端）；`include` 用 `globset` 过滤。不走 `grep` 元 crate 与二进制 CLI。
- `path` 缺省时取 `workspace_root`；无 `workspace_root` 则返回明确错误。
- 输出含文件路径、行号、匹配行；`context > 0` 时附前后 context 行。行号**恒开**，不提供开关（模型拿行号去定位 / 续读是主要用途）。
- 最大返回 100 条匹配；超出时显式提示已截断并建议收窄 `path` / `include`（大结果落盘由 P2 的三层预算负责）。
- **内部时间上限**：搜索在 `spawn_blocking` 内同步运行，外层 `exec::invoke` 的 30s 到点只能放弃 future，blocking 线程无法被强杀（见 §4 与上级 §11）。因此遍历循环内按「外层剩余预算 − 500ms」设内部 deadline，命中即停止遍历，返回**已收集结果**并在文案注明「已达时间上限，结果可能不完整，请缩小 `path` / `include`」——不报错、不丢弃已得结果。
- 结果只含 workspace 内路径。
- **二进制检测默认开**（C9）：`BinaryDetection::quit(b'\x00')`，二进制文件在首个 NUL 处停止、不再产出乱码行，计入 `binarySkipped`。存量实现未开（默认 `None`），属缺陷修正。

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
| `multi_line` | 不变 | 不影响编码路径 |

**热路径（有命中）**：`trim_line_end` 改为先 `std::str::from_utf8`，失败才 `from_utf8_lossy` 并置 `lossy_lines = true`。合法 UTF-8 命中因此比现状**少一次分配**（`from_utf8_lossy` 内部本来就会先跑 `from_utf8`），且不再静默吞掉「这行不是 UTF-8」的事实。

**冷路径（零命中时的 probe）**：仅当 `match_count == 0` 且未达内部 deadline 时执行，热路径零成本：

1. 逐文件读**前 8KB**到**同一个可复用缓冲**（不每文件新建 `Vec`），按 [read §3.3](./I38c-read-spec.md) 步骤 1–3 同一套前缀分类判定。
2. **跳过**：含 `0x00`（二进制）、有 BOM（已可搜）、合法 UTF-8。
3. 计数「非 UTF-8 且非二进制」文件；达 `PROBE_FILE_LIMIT = 200` 或 deadline 即停，置 `nonUtf8Probe.truncated = true`。
4. 计数 > 0 时输出 footer `[non-utf8: N file(s)]`；为 0 时**静默**（成功路径不给指令）。

**结果 `data` 新增三个字段**：

| 字段 | 类型 | 语义 |
|---|---|---|
| `binarySkipped` | `u32` | 被二进制检测跳过的文件数 |
| `lossyLines` | `u32` | 渲染时含替换字符的行数（命中行与上下文行都计；仅在未设 `encoding` 且该行非法 UTF-8 时非零） |
| `nonUtf8Probe` | `Option<{ files: u32, truncated: bool }>` | 零命中 probe 的结果；有命中或未执行时为 `null` |

**footer 形态**：`[non-utf8: N file(s)]`，独占一行、出现在结果尾部（上级 §14.4 规则 4 / 5 的标记集）。它与 `read` 的 `[encoding: …]` 同属标记集；`edit` **不剥离**末尾 footer 行，仅在 0 匹配诊断里据此提示「这不是文件内容，请移除后重试」（上级 §14.4 规则 4）。

**已知限制**（不进 `description`，只在错误表与文档里说明）：未传 `encoding` 时非 UTF-8 文件按 UTF-8 逐字节匹配，命中行以替换字符渲染。这是「不猜测」的直接代价——要正确匹配就传 `encoding`。`description` 只保留一句「省略即按原样搜索」（上级 §14.4 提示边界）。

**为什么不复用 `read` 的编码检测**：`grep` 的 scope 可能有上千个文件，逐文件跑 `chardetng` 是 O(文件数 × 解码成本)，且「scope 内多编码并存」时没有任何单一结论可用。显式 `encoding` 是 scope 级声明，语义与 ripgrep `--encoding` 一致，行为可预测。

## 4 错误

| 情况 | 行为 |
|---|---|
| `pattern` 缺失 / 类型错误 | `Business` |
| 非法正则 | `Business`，附 `regex` 报错；不 panic、不中断 turn |
| 基准目录越界 / 不存在 | `Business` |
| 达到内部 deadline | **不报错**：返回已收集结果 + 「结果可能不完整」提示（§3） |
| `encoding` label 非法 | `Business`，附 `Encoding::new` 的报错 + 规范名示例（`gbk` / `big5` / `shift_jis`）；不静默退回 UTF-8 |

## 5 参考

| 来源 | 做法 | 决定 |
|---|---|---|
| OpenBitFun `assembly/core/src/agentic/tools/implementations/grep_tool.rs` | schema：`pattern` / `path` / `glob` / `output_mode`（`content` / `files_with_matches` / `count`，**默认 `files_with_matches`**）/ `-A` / `-B` / `-C` / `context` / `-n` / `-i` / `type` / `head_limit`（默认 250）/ `offset` / `multiline`；内部 `tokio::time::timeout(30s)`，超时文案「Workspace search timed out after 30000ms; narrow the search path」 | **部分采纳**：`context`（等价 `-C`）、`glob` 过滤（本项目名 `include`）、行号输出、截断后建议收窄的文案形态、内部时间上限（§3）。**不采纳**：三态 `output_mode`（P5 恒 `content`；`files_with_matches` 用 `glob` 表达，`count` 价值低且可后续加）、`-A` / `-B` 单侧上下文（`context` 已覆盖主用例）、`offset`、`multiline`、`type`（`include` 按语言后缀过滤已够）、`-i`（可用内联标志 `(?i)` 表达，少一个参数就少一类误用） |
| 上限取舍 | 参考 `head_limit` 默认 250 / 本项目 100 | `content` 模式下 250 行会挤占上下文预算；100 条 + 明确的截断提示足以驱动模型收窄搜索 |
| OpenBitFun `execution/tool-execution/src/search/grep_search.rs` | 自带结果 reducer、行内 hydration（`line_hydration.rs`，` [truncated]` 后缀）与连续行分隔符逻辑 | **不采纳** reducer 与行水合（P5 结果直接进上下文，由 P2 三层预算兜底）；**采纳**「不连续行之间加分隔符」的输出形态（与 rg 一致），避免 context 行被误读为连续片段 |
| 搜索层依赖 | 参考为自研搜索模块 | **采纳拆分 crate 选型**：`ignore` + `grep-searcher` + `grep-regex` + `globset`（定稿见上级 §11 OQ1） |
| **编码支持**（C9） | ripgrep | `references/7-text-encoding/ripgrep/crates/searcher/src/searcher/mod.rs`：`SearcherBuilder::build` 用 `DecodeReaderBytesBuilder::new().encoding(enc).utf8_passthru(true).strip_bom(true).bom_override(true).bom_sniffing(true)`；`Encoding::new(label)` 经 `encoding_rs::Encoding::for_label_no_replacement` 校验；`mmap` 默认 `MmapChoice::Never`。`crates/core/flags/hiargs.rs`：`BinaryDetection::quit(b'\x00')` | **采纳**：配置链（`binary_detection` / `encoding` / `bom_sniffing` / `utf8_passthru`）、label 校验表、mmap 关闭（§3.1）。**不采纳**：`--encoding` 的 CLI 外壳与 `encoding` 的 per-type override（本项目 scope 级单值足够）、逐文件 `chardetng` 猜测（会与 `read` 形成第二个编码发现点）、mmap 加速（与转码路径不兼容） |
| **零命中非 UTF-8 探测**（C9） | 无 | 参考实现无「搜索无结果时告知存在非 UTF-8 文件」的能力 | **本次新增**：两层探测（热路径 `from_utf8` 快速路径 + 冷路径 8KB 前缀有界 probe），见 §3.1 |
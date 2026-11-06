# I38c-grep — `grep`（只读工具）

| 项 | 值 |
|---|---|
| 状态 | 现行（P5 工具分册） |
| 日期 | 2026-11-06 |
| 上级 | [I38c-os-tools-spec.md](./I38c-os-tools-spec.md)（工具清单 / 共同结构 / 验收标准） |
| 任务 | [I38c-os-tools-tasklist.md](./I38c-os-tools-tasklist.md) C2.23–C2.27 |
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
    "context": { "type": "integer", "description": "Lines of context (default 0)" }
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

## 4 错误

| 情况 | 行为 |
|---|---|
| `pattern` 缺失 / 类型错误 | `Business` |
| 非法正则 | `Business`，附 `regex` 报错；不 panic、不中断 turn |
| 基准目录越界 / 不存在 | `Business` |
| 达到内部 deadline | **不报错**：返回已收集结果 + 「结果可能不完整」提示（§3） |

## 5 参考

| 来源 | 做法 | 决定 |
|---|---|---|
| OpenBitFun `assembly/core/src/agentic/tools/implementations/grep_tool.rs` | schema：`pattern` / `path` / `glob` / `output_mode`（`content` / `files_with_matches` / `count`，**默认 `files_with_matches`**）/ `-A` / `-B` / `-C` / `context` / `-n` / `-i` / `type` / `head_limit`（默认 250）/ `offset` / `multiline`；内部 `tokio::time::timeout(30s)`，超时文案「Workspace search timed out after 30000ms; narrow the search path」 | **部分采纳**：`context`（等价 `-C`）、`glob` 过滤（本项目名 `include`）、行号输出、截断后建议收窄的文案形态、内部时间上限（§3）。**不采纳**：三态 `output_mode`（P5 恒 `content`；`files_with_matches` 用 `glob` 表达，`count` 价值低且可后续加）、`-A` / `-B` 单侧上下文（`context` 已覆盖主用例）、`offset`、`multiline`、`type`（`include` 按语言后缀过滤已够）、`-i`（可用内联标志 `(?i)` 表达，少一个参数就少一类误用） |
| 上限取舍 | 参考 `head_limit` 默认 250 / 本项目 100 | `content` 模式下 250 行会挤占上下文预算；100 条 + 明确的截断提示足以驱动模型收窄搜索 |
| OpenBitFun `execution/tool-execution/src/search/grep_search.rs` | 自带结果 reducer、行内 hydration（`line_hydration.rs`，` [truncated]` 后缀）与连续行分隔符逻辑 | **不采纳** reducer 与行水合（P5 结果直接进上下文，由 P2 三层预算兜底）；**采纳**「不连续行之间加分隔符」的输出形态（与 rg 一致），避免 context 行被误读为连续片段 |
| 搜索层依赖 | 参考为自研搜索模块 | **采纳拆分 crate 选型**：`ignore` + `grep-searcher` + `grep-regex` + `globset`（定稿见上级 §11 OQ1） |
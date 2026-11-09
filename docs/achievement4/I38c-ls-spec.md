# I38c-ls — `ls`（只读工具）

| 项 | 值 |
|---|---|
| 状态 | 现行（P5 工具分册） |
| 日期 | 2026-11-06 |
| 上级 | [I38c-os-tools-spec.md](./I38c-os-tools-spec.md)（工具清单 / 共同结构 / 验收标准） |
| 任务 | I38c-os-tools-tasklist（已归档） C2.14–C2.17 |
| 实现 | `src-tauri/crates/assistant-tools/src/os/agent_list.rs` |
| 注册名 | `ls`（旧名 `fs_list`） |

> 共同约束见上级 §4，本文不重复。

## 1 安全标记

| action | read_only | destructive | open_world | audience | 并发 |
|---|---|---|---|---|---|
| `ls` | 是 | 否 | 否 | `BOTH` | 安全（`is_concurrency_safe = true`） |

`permission_intents` 恒返回空 `Vec`。

## 2 input_schema

```json
{
  "type": "object",
  "properties": {
    "path": { "type": "string", "description": "Directory path to list" },
    "depth": { "type": "integer", "description": "Max recursion depth (default 1)" },
    "all": { "type": "boolean", "description": "Include hidden entries" }
  },
  "required": ["path"]
}
```

## 3 行为

- `depth` 默认 1、最大 10，超出按 10 夹取并显式提示已夹取。
- 每层最大 1000 条目，超出时显式提示「已截断 + 实际条目数」。上限是**每层**而非全局，避免深层条目把浅层条目挤掉（参考实现是 BFS + 全局 `limit`，存在该问题）。
- 输出按名称排序（字典序，稳定），目录与文件不分组。**不按修改时间排序**：批量解包 / 克隆后大量条目的 mtime 同值，排序结果不可复现（参考实现按 mtime 倒序）。
- `all = false` 时跳过以 `.` 开头的条目。
- 目录遍历**不跟随符号链接**，避免越界读取；链接条目照常列出但不下降，对每个条目仍保持 workspace 边界结论一致。
- **不读 `.gitignore`、不硬编码排除 `node_modules` / `target` / `dist` 等目录**：`ls` 的语义是「目录实况」，过滤会造成「文件确实存在但 `ls` 看不到」，与 `read` 能读到的事实矛盾，模型会据此判断「目录不存在」。忽略规则统一由 `glob` / `grep` 承担（它们遵守 `.gitignore`）。

## 4 错误

| 情况 | 行为 |
|---|---|
| `path` 缺失 / 类型错误 | `Business` |
| 路径越界或命中系统目录 | `Business` |
| 目标不存在 / 不是目录 | `Business`，提示改用 `read` |

## 5 参考

| 来源 | 做法 | 决定 |
|---|---|---|
| OpenBitFun `assembly/core/src/agentic/tools/implementations/ls_tool.rs` | schema 只有 `path` + `limit`（默认 200，正数校验）；结果按 mtime 倒序（新的在前）；隐藏项一律排除，无开关 | **部分采纳**：隐藏项跳过、`limit` 的「正数 + 上限 + 夹取」语义（§3）。**不采纳** mtime 排序（输出不可复现）与「只有全局 limit」（§3 改为每层上限）。`depth` / `all` 为本项目新增参数：模型需要「只看一层」与「含隐藏项」两个正交开关，参考实现都缺 |
| OpenBitFun `execution/tool-execution/src/fs/listing.rs` | BFS + 全局 `limit`（`result.len() == limit` 即返回 `truncated`）；`EXCLUDED_NAMES` 硬排除（`node_modules` / `target` / `build` / `dist` / `Cargo.lock` 等）；每层读 `.gitignore`（`MAX_IGNORE_BYTES = 1MiB`，超限直接报错）；跳过隐藏项与符号链接；每个目录先 `sort_by(compare_entries)` 再按 BFS 顺序输出 | **部分采纳**：跳过隐藏项、跳过符号链接、目录内先排序再输出（§3）。**不采纳** 硬编码排除清单与 `.gitignore` 过滤（理由见 §3）、BFS 全局 limit |
| 条目计数与截断文案 | 参考实现无「每层计数」概念 | **本次新增** |
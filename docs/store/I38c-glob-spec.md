# I38c-glob — `glob`（只读工具）

| 项 | 值 |
|---|---|
| 状态 | 现行（P5 工具分册） |
| 日期 | 2026-11-06 |
| 上级 | [I38c-os-tools-spec.md](./I38c-os-tools-spec.md)（工具清单 / 共同结构 / 验收标准） |
| 任务 | [I38c-os-tools-tasklist.md](./I38c-os-tools-tasklist.md) C2.18–C2.22 |
| 实现 | `src-tauri/crates/assistant-tools/src/os/agent_glob.rs` |
| 注册名 | `glob`（旧名 `fs_glob`） |

> 共同约束见上级 §4，本文不重复。

## 1 安全标记

| action | read_only | destructive | open_world | audience | 并发 |
|---|---|---|---|---|---|
| `glob` | 是 | 否 | 否 | `BOTH` | 安全（`is_concurrency_safe = true`） |

`permission_intents` 恒返回空 `Vec`。

## 2 input_schema

```json
{
  "type": "object",
  "properties": {
    "pattern": { "type": "string", "description": "Glob pattern (e.g. '**/*.ts')" },
    "path": { "type": "string", "description": "Base directory" },
    "exclude": { "type": "array", "items": { "type": "string" } }
  },
  "required": ["pattern"]
}
```

## 3 行为

- 依赖选型：`ignore`（目录遍历，遵循 `.gitignore`，跳过隐藏目录与符号链接）+ `globset`（模式与 `exclude` 编译）。两者是 ripgrep 官方组件，与 `grep` 系 crate 同源，不引入第二套遍历 / 匹配语义。
- `path` 缺省时取 `workspace_root`；无 `workspace_root` 则返回明确错误，不静默退回当前目录。
- `pattern` 一律**相对 `path`**（`path` 缺省时相对 `workspace_root`）。**不采纳**「绝对 pattern 自动推导搜索根」的隐式行为（参考 `extract_glob_base_directory` / `derive_walk_root`）：隐式基准会让边界判定出现第二套口径，审批资源与实际搜索范围不一致；绝对路径一律由 `path` 显式给出。
- `exclude` 在 `pattern` 命中后二次过滤，不改变遍历范围。
- 结果只含 workspace 内路径；越界由 `ensure_within_root` 统一拒绝。
- 最大返回 1000 条，按路径字典序排序（稳定），超出时显式提示已截断，并给出「收窄 pattern / 指定更小 `path`」的下一步提示。

## 4 错误

| 情况 | 行为 |
|---|---|
| `pattern` 缺失 / 类型错误 | `Business` |
| `pattern` 非法 glob 语法 | `Business`，附 `globset` 报错位置 |
| 无 `workspace_root` 且未给 `path` | `Business`，提示显式传 `path` |
| 基准目录越界 / 不存在 | `Business` |

## 5 参考

| 来源 | 做法 | 决定 |
|---|---|---|
| OpenBitFun `assembly/core/src/agentic/tools/implementations/glob_tool.rs` | `GLOB_RESULT_LIMIT = 100`；schema 只有 `pattern` + `path`（无 `exclude`），`pattern` 说明明确写「relative to `path`」；空结果与截断各有固定文案（`Found N matches` / `Showing N of M matches … This list is truncated and not complete. Narrow the pattern or search a more specific path …`） | **部分采纳**：截断文案「明确告知结果不完整 + 给出收窄动作」的形态（§3）。**不采纳** 100 条上限（见下行）与「无 `exclude`」——`exclude` 为本项目新增：模型最常需要排除生成目录，缺它会让同一搜索多轮重试 |
| OpenBitFun `execution/tool-execution/src/search/glob_search.rs` | `extract_glob_base_directory` / `derive_walk_root`：绝对 pattern 推导静态父目录为搜索根，再按相对 pattern 匹配 | **不采纳**（理由见 §3：隐式基准与边界判定冲突） |
| 上限取舍 | 参考 100 / 本项目 1000 | 100 对中型仓库过紧（`**/*.ts` 常超 100 条），模型要多轮试探；上下文膨胀由 P2 三层预算（单结果 50K 字符落盘）兜底，`glob` 侧只需给出明确的截断提示 |
| 遍历与匹配层 | 参考自带 `ignore` 包装与结果后置排序 | **采纳依赖选型**：直接用 ripgrep 官方 crate（`ignore` + `globset`），定稿见上级 §11 OQ1；本项目额外规定按路径字典序排序，保证同一输入输出可复现 |
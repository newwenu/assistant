# I38c-move — `move`（修改工具）

| 项 | 值 |
|---|---|
| 状态 | 现行（P5 工具分册） |
| 日期 | 2026-11-06 |
| 上级 | [I38c-os-tools-spec.md](./I38c-os-tools-spec.md)（工具清单 / 共同结构 / 验收标准） |
| 任务 | I38c-os-tools-tasklist（已归档） C3.17–C3.21 |
| 实现 | `src-tauri/crates/assistant-tools/src/os/agent_move.rs` |
| 注册名 | `move`（旧名 `fs_move`） |

> 共同约束见上级 §4，本文不重复。

## 1 安全标记

| action | read_only | destructive | open_world | audience | 并发 |
|---|---|---|---|---|---|
| `edit` | 否 | 否 | 否 | `BOTH` | 串行（保持默认） |

`permission_intents` → **两条** `edit`：`source` 与 `target` 各一条（两个路径都受影响）。

## 2 input_schema

```json
{
  "type": "object",
  "properties": {
    "source": { "type": "string" },
    "target": { "type": "string" },
    "overwrite": { "type": "boolean", "description": "Allow overwrite (default false)" }
  },
  "required": ["source", "target"]
}
```

## 3 行为

- 仅使用 `std::fs::rename`，保证同设备内原子。
- 目标已存在且 `overwrite = false` → `Business`，不自动改名。
- `overwrite = true` 时先删除目标再重命名。**目标是目录时只允许空目录**（`remove_dir` 成功即继续）；非空目录 → `Business`，提示先用 `delete`。`move` 不得隐式递归删除——那会让一次审批覆盖两类破坏性改动（`delete` 的预扫描与不可删目标判定见 [I38c-delete](./I38c-delete-spec.md)）。
- 跨设备失败返回 `Business`，**不**静默降级为「复制 + 删除」（降级会丢失原子性且无法保留权限位）。
- 目标的父目录必须已存在，不自动创建（避免一次调用隐式产生两处目录改动）。
- 成功后失效 `source` 的读取回执：`ReadReceiptStore::invalidate_path(source)`，按组件级前缀连带子树（理由与 `delete` 相同，见 [I38c-delete §3.6](./I38c-delete-spec.md)）。`target` 无需失效——移动后路径上是新内容，回执键的修订号会自然不匹配。

## 4 错误

| 情况 | 行为 |
|---|---|
| `source` / `target` 缺失或类型错误 | `Business` |
| 任一路径越界或命中系统目录 | `Business` |
| `source` 不存在 | `Business` |
| 目标已存在且 `overwrite = false` | `Business` |
| 目标是已存在的**非空目录**且 `overwrite = true` | `Business`，提示先 `delete`（不递归删除，§3） |
| 跨设备（`EXDEV`） | `Business`，提示改用 `write` + `delete` 两步（不降级、不提供不存在的复制能力） |
| 目标父目录缺失 | `Business`，提示先 `mkdir` |

## 5 参考

- 参考项目均**无独立 `move` 工具**（已核对：OpenBitFun 无 rename 工具；codex `apply-patch` 只有 Add / Delete / Update 三类，无 rename；goose / deepseek-harness-orb 走 `shell`）。因此 `overwrite` / 跨设备 / 父目录语义均按 `std::fs::rename` 与 POSIX 语义自行定义，无参考实现可对照。
- 拒绝跨设备降级为「复制 + 删除」为**本次明确的保守约束**：降级会丢失原子性、权限位与硬链接，中途失败还会留下两份半成品。
- 拒绝隐式递归删除目标目录为**本次明确的保守约束**（理由见 §3）。
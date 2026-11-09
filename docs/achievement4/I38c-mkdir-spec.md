# I38c-mkdir — `mkdir`（修改工具）

| 项 | 值 |
|---|---|
| 状态 | 现行（P5 工具分册） |
| 日期 | 2026-11-06 |
| 上级 | [I38c-os-tools-spec.md](./I38c-os-tools-spec.md)（工具清单 / 共同结构 / 验收标准） |
| 任务 | I38c-os-tools-tasklist（已归档） C3.14–C3.16 |
| 实现 | `src-tauri/crates/assistant-tools/src/os/agent_mkdir.rs` |
| 注册名 | `mkdir`（旧名 `fs_mkdir`） |

> 共同约束见上级 §4，本文不重复。

## 1 安全标记

| action | read_only | destructive | open_world | audience | 并发 |
|---|---|---|---|---|---|
| `edit` | 否 | 否 | 否 | `BOTH` | 串行（保持默认） |

`permission_intents` → `edit` + 目标路径。

## 2 input_schema

```json
{
  "type": "object",
  "properties": {
    "path": { "type": "string" },
    "parents": { "type": "boolean", "description": "Create parent directories (default true)" }
  },
  "required": ["path"]
}
```

## 3 行为

- `parents = true`（默认）等价 `create_dir_all`。
- `parents = false` 且父目录不存在 → `Business`，不静默降级为递归创建。
- 目录已存在时**幂等成功**，返回体注明「已存在，未做改动」。
- 目标已存在但是文件 → `Business`。

## 4 错误

| 情况 | 行为 |
|---|---|
| `path` 缺失或类型错误 | `Business` |
| 路径越界或命中系统目录 | `Business` |
| `parents = false` 且父目录缺失 | `Business` |
| 目标是已存在的文件 | `Business` |

## 5 参考

- 参考项目均**无独立目录创建工具**（已核对：OpenBitFun `assembly/core/src/agentic/tools/implementations/` 无 `mkdir` 类工具；codex 无；goose / deepseek-harness-orb 走 `shell`）。因此没有可直接对照的参数与返回体。
- `parents` 语义的依据：OpenBitFun `file_write_tool.rs` 在写入前自动创建父目录（等价 `create_dir_all`），说明其产品口径把「建目录」视为写入的附属步骤而非独立审批对象。本项目仍单列 `mkdir`：目录创建是独立的 `edit` intent，审批粒度更细，且失败原因（父目录缺失 / 目标是文件）比写入失败更好定位。
- 幂等返回体（「已存在，未做改动」）与 `parents = false` 的硬失败为**本项目明确**，参考实现无对应返回语义。
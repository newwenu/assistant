# I38c-delete — `delete`（破坏性工具）

| 项 | 值 |
|---|---|
| 状态 | 现行（P5 工具分册） |
| 日期 | 2026-11-06 |
| 上级 | [I38c-os-tools-spec.md](./I38c-os-tools-spec.md)（工具清单 / 共同结构 / 注册门禁 / 验收标准） |
| 任务 | [I38c-os-tools-tasklist.md](./I38c-os-tools-tasklist.md) C4.1–C4.11 |
| 实现 | `src-tauri/crates/assistant-tools/src/os/agent_delete.rs` |
| 注册名 | `delete`（旧名 `fs_delete`） |

> 共同约束见上级 §4，本文不重复。**注册门禁见上级 §7.1**：P2 审批可用 + 前端审批卡片上线（P6）之前不得加入生产注册表。

## 1 安全标记

| action | read_only | destructive | open_world | audience | 并发 |
|---|---|---|---|---|---|
| `delete` | 否 | **是** | 否 | `BOTH` | 串行（`is_concurrency_safe = false`） |

`permission_intents` → `delete` + **`resolve()` 返回的规范化绝对路径**（供 P2 `ApprovalKey::Path` 归一，批准一次只对该路径生效）。

递归删除的审批展示必须给出量级（条目数 / 字节数）。这些数字来自审批**前**的预演（`preview_for_approval` 复用 §3.3 预扫描），**不是** `execute` 结果里的 `data`——后者在审批时尚未产生。P5 负责提供预演函数，展示样式归 P6 审批卡片（[I38d §3.2.1](./I38d-agent-frontend-modes-spec.md)）。

## 2 input_schema

```json
{
  "type": "object",
  "properties": {
    "path": { "type": "string" },
    "recursive": { "type": "boolean", "description": "Delete directory recursively" }
  },
  "required": ["path"]
}
```

`schema` 不暴露 `force`：模型不得跳过任何前置检查（见 §5）。

## 3 行为

### 3.1 解析与不可删目标

- 一律 `os::path::resolve()`，`execute()` 只用其返回值做 IO。
- 以下目标在 `validate_input` 内直接拒绝（`Business`），不进入审批：

| 目标 | 判定 |
|---|---|
| `workspace_root` 本身 | `resolve()` 结果与 `canonicalize_lenient(workspace_root)` 组件级相等；`workspace_root = None` 时跳过该条 |
| 文件系统根 / 盘符根 | `Path::parent()` 为 `None`（`C:\`、`/`、UNC 共享根） |
| 家目录本身 | 与 `HOME` / `USERPROFILE` 组件级相等（Windows 大小写不敏感）。**只拒家目录本身，不拒其子项**——工作区通常就在家目录下 |
| 系统目录 / 凭据目录 | `os::path::reject_system_dirs`（含 `.ssh` / `.gnupg` / `.aws` / `.azure` / `.kube` / `keychains` 与 `Microsoft\{Protect,Credentials,Crypto,Vault}`，清单见 I31 §5.3） |
| workspace 之外 | `ensure_within_root`（仅 `workspace_root = Some` 时生效） |

- 拒绝必须发生在任何删除动作之前，不得「先尝试删除、失败再报边界错误」。

### 3.2 目标分类（不跟随链接）

- 用 `symlink_metadata`（不跟随）分类：不存在 / 文件 / 目录 / 链接。
- 链接（Unix symlink、Windows `mklink /J` junction、dangling 链接）一律**按文件删除**：删链接本身、不进入目标；`recursive` 对链接不改变行为；结果 `isDirectory = false`。
- 目录链接的删除分支：先 `remove_file`，失败后 `remove_dir`（NTFS 上 junction 是目录条目，`DeleteFile` 会失败）；两者都失败才报错。
- 禁止用 `Path::exists()` / `Path::is_dir()` 判定目标类型：它们跟随链接，会把「删链接」变成「删目标」。
- 递归删除只下降真实目录；子项中的链接只删链接本身，不进入。

### 3.3 递归预扫描

预扫描只有两个用途，**不承担安全职责**——边界已由 §3.1 一次性确定，子树也无法借链接逃逸（§3.2）：

1. 非空目录 + `recursive = false` 的确定性失败；
2. 审批卡片与结果的量级信息。

- 遍历：显式栈 + `read_dir`，不下降链接；累计 `entries`（文件 / 目录 / 链接各计 1）与 `bytes`（`symlink_metadata().len()`，链接计 0）。
- 上限 `DELETE_SCAN_LIMIT = 100_000` 条目：达到即停止扫描并置 `truncated = true`，**不因此拒绝删除**。
- 空目录判定只读第一项（`read_dir(...).next()`），不整目录列举。
- `read_dir` 失败 → `Business`（无法确认目标结构，拒绝删除），不做「先试再删」。
- 竞态：预扫描与实际删除之间目标可能变化，**以删除操作的返回为准**；预扫描结论只用于文案与审批展示。

**可复用性要求**：扫描函数必须抽成独立单元，**同时**被 `preview_for_approval`（审批前，供卡片）与 `execute`（审批后，供结果与 `recursive = false` 判定）调用；两处禁止各写一份遍历逻辑，否则会出现「审批说 3 项、实际删 300 项」。`preview_for_approval` 只返回 `entries` / `bytes` / `truncated`，不做删除判定，`recursive = false` 的拒绝仍只在 `execute` 内发生。

### 3.4 执行

- 分支：文件 / 链接 → `remove_file`（目录链接按 §3.2 回退）；目录 + `recursive = false` → `remove_dir`；目录 + `recursive = true` → `remove_dir_all`。
- 不自动清只读属性：Windows 只读文件删除失败 → `Business`，提示先移除只读属性（改属性本身是额外的破坏性写）。
- `remove_dir` 返回非空 → `Business`，提示 `recursive = true`。与 §3.3 的预扫描重复是刻意的：预扫描结论可能已过期。
- 失败或超时后重查目标：仍存在 → 文案「未删除或已部分删除」；已不存在 → 文案「删除已完成，但随后失败」。不得谎报成功，也不得谎报未删。
- 不重试、不自动分批。`remove_dir_all` 没有取消点，超时只能记录、无法中断（见上级 §11 风险表）。

### 3.5 结果

```json
{
  "path": "<resolve() 返回的绝对路径>",
  "isDirectory": false,
  "recursive": false,
  "entriesDeleted": 1,
  "bytesDeleted": 1234,
  "truncated": false
}
```

- `path` 必须是 `resolve()` 的返回值，与审批键一致；不得回传用户原始字符串。
- `result_for_assistant`：
  - 文件 / 链接 → `Deleted file at: <path>`
  - 目录 → `Deleted directory at: <path> (N entries, M bytes)`
  - `truncated = true` → 条目数与字节数写成 `>= 100000`
- 失败结果不得出现「已删除」字样，唯一例外是 §3.4 的「删除已完成，但随后失败」。

### 3.6 回执失效

- 删除成功后调用 `ReadReceiptStore::invalidate_path(path)`，按**组件级前缀**失效（`/a/b` 不得匹配 `/a/bc`）。
- 理由：回执的修订号是「文件长度 + 修改时间」，删除后重建若长度与时间戳恰好相同（保时间戳的复制 / 解包），`edit` 的读前置 guard 会误判「已读过」。[I38c-read §3.1](./I38c-read-spec.md) 的回执设计依赖修订号自愈，删除是最强的「内容已变」信号，直接失效比赌修订号更可靠。
- 该方法是 `ReadReceiptStore` 的**新增端口方法**（P2 只有 `lookup` / `record`），跨阶段契约变更已回写 [I38a §4-P2-C6](./I38a-tool-control-layer-spec.md)；`move` 的源路径同样适用（见 [I38c-move](./I38c-move-spec.md) §3）。

## 4 错误

| 情况 | 行为 |
|---|---|
| `path` 缺失 / 类型错误 | `validate_input` 拒绝（`Business`） |
| 目标越界 / 系统目录 / 凭据目录 | `resolve` 拒绝（`Business`） |
| 目标是 `workspace_root`、文件系统根、家目录 | `validate_input` 拒绝（`Business`） |
| 目标不存在 | `Business`（不视为幂等成功，避免掩盖拼写错误） |
| 非空目录 + `recursive = false` | `Business`，提示 `recursive = true` |
| `read_dir` 失败 | `Business`（无法确认目标结构） |
| Windows 只读文件 | `Business`，提示先移除只读属性 |
| `remove_*` 失败 / 超时 | `Business`，按 §3.4 区分「未删除 / 部分删除」 |
| 审批被拒 | `Denied`（P2 网关打标） |

## 5 参考

> 参考项目里只有 OpenBitFun 提供专用 `delete` 工具，其余用 shell（`rm`）完成删除。这决定了本文的采纳重心：**专用工具的价值在审批粒度与结构化结果**，不在新增能力。

| 来源 | 做法 | 决定 |
|---|---|---|
| OpenBitFun `assembly/core/src/agentic/tools/implementations/delete_file_tool.rs` | 专用 `Delete` 工具；`inspect_workspace_delete_target` 用 `path_kind_no_follow` 判类型；结果含 `path` / `is_directory` / `recursive`；成功文案区分文件与目录 | **采纳**（§3.2 / §3.5） |
| OpenBitFun `execution/tool-execution/src/fs/delete_path.rs` | 非空目录 + 非递归前置拒绝；`read_dir_bounded(path, 1)` 判空只读一项；成功文案 `Successfully deleted {file\|directory} at: …` | **采纳**（§3.3 / §3.4）。其 `inspect_local_delete_target` 用 `exists()` / `is_dir()`（跟随链接），与 workspace 版本不一致，**不采纳该不一致** |
| OpenBitFun `assembly/core/src/agentic/execution/edit_constraint_guard.rs` | 递归删除前遍历全部非链接子项做保护约束检查，条目上限 `100_000`，超限 413、检查失败 503 | **部分采纳**：上限与「不进入链接」采纳（§3.3）；逐项约束检查不采纳——我们的边界由 `resolve()` 一次性确定，子树无法逃逸 |
| OpenBitFun `assembly/core/src/agentic/tools/tool_context_runtime.rs` | 删除前记轻量 checkpoint（git 分支 / 脏状态），供事后追溯 | **不采纳**：依赖其 evidence ledger，我们无对应设施（见下表） |
| codex `apply-patch/src/lib.rs`（`DeleteFile`） | 无独立删除工具；删除前读内容做 diff / 撤销元数据；`ensure_not_directory` 拒绝目录；`remove(recursive: false, force: false)`；失败后校验是否「无副作用」 | **部分采纳**：拒绝目录、不暴露 `force`、失败后重查存在性（§3.4）；读内容不采纳（见下表） |
| openhuman `crates/openhuman-core/src/security/policy/path_checks.rs` | `is_always_forbidden` 硬禁凭据目录（`.ssh` / `.gnupg` / `.aws` / `.azure` / `.kube` / `keychains` / `Microsoft\{Protect,Credentials,Crypto,Vault}`）与核心系统目录，且该判定先于任何 trusted-root 授权 | **采纳**（§3.1，落在共享 `os::path::reject_system_dirs`，见 I31 §5.3） |
| openhuman `crates/openhuman-core/src/tools/impl/filesystem/gate.rs` | `FsGate` 抽象：`validate_path` / `validate_parent_path` / `write_needs_approval` / `record_action` / 按 root 作用域授权 | **不采纳**：P2 的 Policy + Gateway 已承担同等职责，再套一层会出现两个路径校验真源 |
| deepseek-harness-orb `packages/fs/fs-local/src/index.ts` + `snapshots/session/fs-delete-recreate/` | 无删除工具，`rm` 走 shell；FS 层提供 no-follow `probeNoFollow` 与 per-target 串行锁；快照验证「删除后读报 not found、重建得到新内容」 | **采纳** no-follow 语义（§3.2）；**不采纳** per-target 串行锁（P2 的串行批次只作用于同轮工具调用，不能跨会话串行化同一路径，理由见 [write §5](./I38c-write-spec.md)）；「删除 → 重建」正是 §3.6 回执失效的动机 |
| goose | 无删除工具（shell / 扩展） | 不适用 |

**不采纳汇总**（补充上表未展开理由的取舍）：

| 不做 | 理由 |
|---|---|
| 回收站 / 软删除 / 自动备份 | 需要跨会话持久化与容量管理，超出 P5；可恢复性由用户侧版本控制承担 |
| `force` 覆盖参数 | OpenBitFun 有单测断言 schema 不含 `force`；模型不应能跳过前置检查 |
| `ToolResult` 结构化 `meta` | 会改动 `ToolResult` 线上结构（需 `contract:gen`）；P5 用 `error` 文本 + `data` 承载同等信息 |
| 删除审计落盘 | P2 无审计端口；`data` 已含路径 / 类型 / 条目数 / 字节数，前端与日志层可自行留存 |
| 删除前轻量 checkpoint | 依赖 git 分支 / 脏状态证据账本，我们无回滚设施 |
| 删除前读文件内容做 diff 元数据 | 同上：无回滚设施，大文件读取成本高于收益 |
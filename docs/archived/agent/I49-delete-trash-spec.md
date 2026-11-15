# I49-delete-trash — `delete` 默认移入回收站 + 显式永久删除

| 项 | 值 |
|---|---|
| 状态 | 现行（`delete` 工具行为变更） |
| 日期 | 2026-11-14 |
| 变更对象 | `delete`（注册名不变），实现 `src-tauri/crates/assistant-tools/src/os/agent_delete.rs`；测试 `src-tauri/crates/assistant-tools/tests/agent_delete.rs`；i18n `src/i18n/locales/{zh-CN,en-US}.json` |
| 替代 | I38c-delete-spec（已归档）§3.4「执行」、§3.5「结果」、§5 不采纳汇总中「回收站 / 软删除」一行 |
| 继续有效 | I38c-delete-spec（已归档）§2 schema 骨架、§3.1 解析与不可删目标、§3.2 目标分类、§3.3 预扫描、§3.6 回执失效 |
| 新增依赖 | `trash = "5.3.0"`：根 `src-tauri/Cargo.toml` 的 `[workspace.dependencies]` 声明，`assistant-tools` 侧 `trash = { workspace = true }`（全平台，默认特性） |
| 前端影响 | 仅 i18n 文案（`settings.agent_mode.tool_desc.delete`）；**无 DTO 变更**，不需要 `npm run contract:gen` |

## 1 变更摘要

一句话：**默认把目标移入系统回收站（可恢复）；只有显式传 `permanent = true` 才走不可逆删除；回收站失败绝不自动回退成永久删除。**

| 场景 | 旧行为 | 新行为 |
|---|---|---|
| 不传新参数（既有调用） | 直接 `remove_*`，不可逆 | 移入系统回收站，可在系统回收站还原 |
| `permanent = true` | 不存在 | 直接 `remove_*`，不可逆（沿用旧实现） |
| 回收站不可用 / 失败 | — | `Business` 失败，文案提示可显式改传 `permanent = true`，**不自动回退** |
| 审批 | 单条 `delete` intent | 回收站模式：`delete`；永久模式：`delete` + `purge`（§5） |
| 非空目录 | 必须 `recursive = true` | 不变（两种模式一致） |
| 链接 | 删链接、不进入目标 | 不变（回收站模式回收链接本身） |

## 2 目标与非目标

**目标**

1. 默认破坏性降级：模型误删的常见后果从「数据丢失」变成「可还原」。
2. 显式不可逆：需要真删时由调用方（最终是用户）明确选择，并在审批卡片上看得见。
3. 行为确定：任何失败都不得静默降级为永久删除；宁可确定性失败，也不静默破坏（沿用 I38c-delete-spec（已归档）§3.4 的立场）。
4. 向后兼容：schema 只增不改，旧调用不报错。

**非目标**

- 不做应用级「回收站 UI / 保留期 / 容量管理」：恢复入口就是操作系统回收站，本工具不提供还原能力（工具集内没有 restore 工具）。
- 不改 `move` / `edit` / `write` 的语义；不改审批网关、策略层、授权存储。
- 不新增 DTO 字段、不改 wire 协议。

## 3 参考项目结论与选型

### 3.1 参考项目调研结论

对 `references/` 下 13 个项目逐一检索删除相关实现，结论一致：

| 类别 | 代表 | 做法 | 对本变更的意义 |
|---|---|---|---|
| 专用删除工具 | OpenBitFun `delete_file_tool.rs` / `fs/delete_path.rs` | 直接 `remove_*`，无回收站；schema 不含 `force` | 提供了本工具既有的安全骨架（已采纳，见 I38c-delete-spec（已归档）） |
| shell 删除 | deepseek-harness-orb、goose、codex 等 | `rm` / `remove(recursive, force)` | 无回收站语义，跨平台行为不一致 → 不采纳 |
| 应用级软删除 | 各项目附件 / 会话 / 文档回收站 | 元数据标记 + 保留期 + 应用内 restore | 只能管自己托管的目录；对任意工作区路径无效 → 不采纳（§3.2） |
| 系统级回收站调用 | **无** | 13 个项目均无 OS 回收站调用代码 | 必须自建；选型见 §3.2 |

值得注意的**本项目内先例**：vault 条目回收站已是「默认进回收站、显式 purge 永久删除、可 restore」的成熟形态（`vault` 设置页 `trash` / `restore` / `purge` 文案）。本变更把同一套用户心智推广到工作区文件，词汇沿用 `purge`。

### 3.2 方案对比

| 方案 | 跨平台 | 与工具层边界 | 结论 |
|---|---|---|---|
| `trash` crate（5.3.0） | Windows `IFileOperation`+`FOF_ALLOWUNDO`；macOS `NSFileManager` 回收；Linux freedesktop trash 1.0 | 纯库，无 Tauri / 无运行时 | **采纳** |
| Tauri 回收站插件 | 同上（本质是 `trash` 包一层命令层） | `assistant-tools` 刻意不依赖 Tauri 运行时（crate 边界，见 crate-architecture），引入插件会把工具层拖进 Tauri 依赖与命令权限面 | 不采纳 |
| 自研（Win32 COM + objc2 + 手写 `.trashinfo`） | 需三套平台代码 | 维护成本与安全面远高于收益（回收站是系统设施，不是产品差异化能力） | 不采纳 |
| shell `rm` / `del` | 语义不一致 | 绕开审批与结构化结果；`del` 不进回收站 | 不采纳 |
| 应用级软删除（参考项目做法） | 是 | 需要托管目录 + 容量管理 + 自有 restore UI，超出工具层职责；且用户无法用系统回收站找回 | 不采纳 |

`trash` 5.3.0 的关键事实（已核对源码，供实现对照）：

| 项 | 事实 |
|---|---|
| 平台实现 | `windows.rs`：`CoCreateInstance(FileOperation)` + `SetOperationFlags(FOF_NO_UI \| FOF_ALLOWUNDO \| FOF_WANTNUKEWARNING)`，逐个 `DeleteItem`，并用 `IFileOperationProgressSink` 收集**每个条目**的失败 HRESULT（`RECYCLE_BIN_NOT_FOUND` / `RECYCLE_SIZE_TOO_BIG` / `RECYCLE_FORCE_NUKE` 等映射成可读原因）；`macos.rs`：`NSFileManager` 的 trash 调用，失败即返回错误；`freedesktop.rs`：`.trashinfo` + `rename`，跨设备时回退 `copy + 删源` |
| 叶子语义 | `canonicalize_paths` **只归一父目录**，保留文件名 → 符号链接仍是链接，不会被换成目标（与 §6.1 的 `resolve_leaf_preserving` 一致） |
| 根目录保护 | 父目录为 `None` → `Error::TargetedRoot` |
| Windows COM 初始化 | 线程局部 `CoInitializeEx(STA)`，**失败直接 panic**（`RPC_E_CHANGED_MODE` 等）；blocking 线程被复用时可能已按 MTA 初始化 → 实现必须 `catch_unwind`（§6.2） |
| 回收站查询 | `trash::os_limited::{list, purge_all}` 仅在 Windows 与 freedesktop 可用（macOS 无）→ 事后校验用得上（§6.2），测试清理也用得上（§9） |
| 依赖增量 | Windows：`windows 0.62`（与 workspace 同版本，feature 取并集，不额外编译一份）+ `windows-core 0.62` + `scopeguard`；Linux：`libc` / `scopeguard` / `urlencoding`（`chrono` 已在用）；macOS：`objc2` / `objc2-foundation` / `percent-encoding`；MSRV 1.87 ≤ 本仓库 1.93 |
| COM 单元 | 默认特性 `coinit_apartmentthreaded` → STA。**不启用** `coinit_multithreaded`：改 MTA 会与同线程其它 COM 使用方（剪贴板等）的 STA 假设相冲，风险面大于收益；STA 冲突用 `catch_unwind` 兜底（§6.2） |

`assistant-tools/Cargo.toml` 只加一行（全平台，不用 `target.'cfg(...)'` 分区——`trash` 自身按平台裁剪依赖）。版本统一在 workspace 声明，与仓库既有外部依赖惯例一致：

```toml
# src-tauri/Cargo.toml [workspace.dependencies]
trash = "5.3.0"

# src-tauri/crates/assistant-tools/Cargo.toml [dependencies]
trash = { workspace = true }
```

接入后必须核对（`cwd = src-tauri`）：`cargo tree -p assistant-tools --edges normal` 确认 (a) `trash` 版本为 5.3.x；(b) `windows` 只有一份 0.62（feature 并集，不额外编译第二份）；(c) `trash` 的默认特性含 `coinit_apartmentthreaded`（STA）且不含 `coinit_multithreaded`。任一不成立即回改 §3.2 的选型结论（`catch_unwind` 兜底策略本身不受影响）。

## 4 接口契约

### 4.1 input_schema

```json
{
  "type": "object",
  "properties": {
    "path": { "type": "string", "description": "Path to delete" },
    "recursive": {
      "type": "boolean",
      "description": "Delete directory recursively (default false)"
    },
    "permanent": {
      "type": "boolean",
      "description": "Permanently delete instead of moving to the system recycle bin (default false). Only set true when the user explicitly asks for irreversible deletion."
    }
  },
  "required": ["path"]
}
```

- 只增字段，`required` 不变 → 旧调用不受影响。
- 仍不暴露 `force`（I38c-delete-spec（已归档）§2 的约束继续成立）：`permanent` 只决定「回收站 vs 不可逆」，不跳过任何前置检查。

### 4.2 解析与校验

`DeleteRequest::parse` 增加一个字段，规则与 `recursive` 完全同构：

| 输入 | 结果 |
|---|---|
| 缺失 / `null` | `permanent = false`（默认回收站） |
| `true` / `false` | 采用 |
| 其它类型（字符串、数字、对象） | `validate_input` 拒绝：`'permanent' must be a boolean`（`Business`） |

未知键维持现状（忽略），与其它工具一致。

### 4.3 描述文案

模型可见（`description()`，英文，无 i18n）：

> Delete a file, a symlink or a directory. By default the target is **moved to the system recycle bin** and can be restored from there. Pass `permanent = true` to delete irreversibly — only when the user explicitly asked for permanent deletion. There is no automatic fallback: if the recycle bin is unavailable the call fails instead of deleting permanently. A directory is only removed with `recursive = true`, and a non-empty directory without it is rejected. Links are removed as links; their targets are never entered.

UI 文案（`src/i18n/locales/{zh-CN,en-US}.json` → `settings.agent_mode.tool_desc.delete`）：

| locale | 新值 |
|---|---|
| `zh-CN` | `删除文件或目录，默认移入回收站；可显式永久删除` |
| `en-US` | `Delete files or directories (moved to the recycle bin by default)` |

### 4.4 结果

`data`（在旧结构上**只增一个键**）：

```json
{
  "path": "<resolve 返回的绝对路径>",
  "isDirectory": false,
  "recursive": false,
  "permanent": false,
  "entriesDeleted": 1,
  "bytesDeleted": 1234,
  "truncated": false
}
```

- `entriesDeleted` / `bytesDeleted` **不改名**：它们是模型可见的输出契约，改名对「回收站模式」没有实质收益，且会打断既有提示词与日志。语义按模式解释：`permanent = false` 时表示「移出工作区（进入回收站）」的条目数与字节数。文档与工具描述负责把语义讲清楚。
- `path` 仍必须是 `resolve` 返回值（与审批键一致），不回传用户原始字符串。

`result_for_assistant`：

| 目标 | `permanent = false` | `permanent = true` |
|---|---|---|
| 文件 / 链接 | `Moved file to the system recycle bin: <path>` | `Permanently deleted file at: <path>` |
| 目录 | `Moved directory to the system recycle bin: <path> (N entries, M bytes)` | `Permanently deleted directory at: <path> (N entries, M bytes)` |

`truncated = true` 时条目数 / 字节数仍写成 `>= 100000`。链接沿用「file」措辞（`isDirectory = false`），与旧行为一致；`data` 不新增 `isLink`。

### 4.5 审批预览（`preview_for_approval`）

预览必须让用户在**批准之前**看到「这是回收站还是不可逆」，文案按 `permanent` 分叉：

| 目标 | `permanent = false` | `permanent = true` |
|---|---|---|
| 链接 | `move the link at <path> to the system recycle bin (its target is not touched)` | `delete the link at <path> (its target is not touched)` |
| 文件 | `move the file at <path> (<size>) to the system recycle bin` | `permanently delete the file at <path> (<size>)` |
| 目录 + `recursive = true` | `move the directory at <path> (<n> entries, <bytes>) to the system recycle bin` | `permanently delete the directory at <path> (<n> entries, <bytes>)` |
| 目录 + `recursive = false` | `move the directory at <path> (<n> entries, <bytes>) to the system recycle bin; it is only moved if it is empty` | `delete the directory at <path> (<n> entries, <bytes>); it is only deleted if it is empty` |

- 量级数字仍来自 `scan_tree`，与 `execute` 共用同一份实现（I38c-delete-spec（已归档）§3.3 的「审批说 3 项、实际删 300 项」约束不变）。
- `truncated` / `estimated` 语义不变（`estimated = true`：执行前可能变化）。

## 5 审批与权限

### 5.1 intents

| 模式 | `permission_intents` |
|---|---|
| `permanent = false` | `[PermissionIntent::new("delete", <abs path>)]` |
| `permanent = true` | `[PermissionIntent::new("delete", <abs path>), PermissionIntent::new("purge", <abs path>)]` |

永久模式**追加** `purge` 而不是替换 `delete`，原因是两者都必须被评估：

1. **授权缓存必须分离。** `ApprovalKey::from_intent` 对已知 action（`read|edit|write|delete|move|copy`）生成 `ApprovalScope::Path` 键，对未知 action 生成 `ApprovalScope::Opaque` 键。若永久模式复用 `delete` 键，用户此前对同一路径的「本会话允许删除」（回收站语义）会被**静默套用到不可逆删除**上——这是本变更要消除的风险，不能反向引入。
2. **保留既有规则的作用面。** 项目/用户配置里针对 `delete` 的 `Deny` 规则、以及 `approvalLevel` 施加的 `Ask` 必须继续命中；只发 `purge` 会让这些规则失效。网关对每条 intent 逐条评估、任一 `Deny` 立即终止本次调用（`gateway.rs::evaluate`），因此「禁删某路径」的既有配置对永久模式**依然有效**：`delete` intent 被拒 → 整个调用被拒，`purge` 只是额外加了一道闸。若用户想单独禁止不可逆删除，可加 `("purge", "*", Deny)` 规则，无需改代码。
3. **策略层无需改动。** `purge` 落到 `ApprovalKey::Opaque{fingerprint: path}`，不走 `ApprovalScope::Path` 前缀匹配；`ResolvedPermissionPolicy` 的基座规则 `("*","*",Ask)`（`dangerous_only` / `all_os_tools`）与硬保护 `Deny` 照常生效，`evaluate_tier_ceiling` 不受影响。因此 §5 不需要改 `gateway.rs` / `policy.rs` / `permission_store.rs`。

`purge` 是**工具内私有 action 词**，只出现在审批键与审计里；`PermissionRule` 的 action 匹配是通配/字符串匹配，无需枚举登记。

### 5.2 审批卡片与审计

- 卡片按 intent 逐行渲染（`ApprovalCard` 现状即如此）：永久模式会显示两行 `delete <path>` / `purge <path>`。`purge` 与 vault 的「永久删除」用词一致，用户可读。
- 审计表（`permission_audit`）对每条 intent 各写一行，action 分别为 `delete` / `purge` → 「何时永久删过什么」可被直接检索，不需要解析工具参数。
- 审批结论（Approve / AllowSession / AllowAlways）对本次调用的所有 intent 一起生效（网关既有行为，不改）。
- `approvalLevel = none` 下两条 intent 都是 `Allow`：该级别本就是「全部自动执行」，与现状一致，不是本变更引入的放宽。

## 6 执行语义

### 6.1 两种模式共用的前置（全部沿用，不改）

1. 解析：`resolve_leaf_preserving` + `reject_undeletable`（根 / workspace 根 / 家目录本身 / 系统目录 / 凭据目录 / 越界）。拒绝发生在任何删除或回收动作之前。
2. 分类：只用 `symlink_metadata`（不跟随），链接类含 Windows junction 的 `FILE_ATTRIBUTE_REPARSE_POINT`。
3. 链接：按链接处理，**不进入目标**，`recursive` 不改变行为。
4. 非空目录 + `recursive = false`：`Business` 拒绝，提示 `recursive = true`。
   - **回收站模式同样要求 `recursive = true`**：虽然回收站可恢复，但「一次搬走整棵树」仍应被显式声明，两种模式保持同一契约，避免模型在不同模式下学到两套规则。
5. 预扫描 `scan_tree`（上限 `DELETE_SCAN_LIMIT = 100_000`）：只服务非空判定与量级文案，不承担安全职责；`preview_for_approval` 与 `execute` 共用。
6. 回执失效 `ReadReceiptStore::invalidate_path(path)`：**条件是「目标已不在磁盘上」，不绑定成功路径**。回收站模式的失败文案可能是「已完成，但随后失败」（目标已消失），此时回执同样必须失效；永久模式失败后若目标已消失亦然。仍存在 → 保留回执（内容未变，读上下文依旧有效）。

### 6.2 回收站分支（`permanent = false`，新增）

- **同步调用，必须在 `spawn_blocking` 内**（现状已如此）：`trash::delete(&target)`。
- **必须 `catch_unwind`**：Windows 上 COM 初始化失败会 panic（`RPC_E_CHANGED_MODE` 等，§3.2），blocking 线程被复用后可能已按 MTA 初始化。用 `std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| trash::delete(&target))`（`&Path` 不实现 `UnwindSafe`，必须显式包裹），panic → `Business`，文案：`cannot move '<path>' to the recycle bin: the recycle bin is unavailable in this thread`。禁止让 panic 穿透到工具层（会变成 `Err(String)` 并被当成内部缺陷）。注意 `catch_unwind` 会吞掉 panic 信息，需在闭包内先 `log::debug!` 记录调用上下文。
- **Windows 前置检查**（只做一条，其余交给 crate 的错误映射）：目标所在卷根（`Path::ancestors()` 末项）必须存在 `$Recycle.Bin` 目录，否则 `Business`：`'<path>' is on a volume without a recycle bin; pass permanent = true only if the user agreed to irreversible deletion`。理由：网络共享 / 未启用回收站的卷是最常见的「无回收站」来源，一次 `symlink_metadata` 就能确定性失败，不必依赖 shell 错误码。**这条只是廉价筛查**：组策略可禁用回收站而目录仍存在，因此最终结论以「事后校验」为准。
  - 实现上抽成纯函数 `volume_has_recycle_bin(volume_root: &Path) -> bool`（只做 `$Recycle.Bin` 存在性检查），让「无回收站卷」用例可用 `tempdir` 确定性单测（§9 第 12 条），不必依赖环境注入。
  - UNC 路径（`\\server\share\...`）直接判为「无回收站」：网络共享没有本机回收站，`ancestors()` 末项是共享根而非卷根，`$Recycle.Bin` 判定不可靠；crate 的错误映射作为兜底。
- **事后校验**（Windows 与 freedesktop）：调用 `trash::os_limited::list()`，找 `original_path() == target`（`os::path::same_path`）且 `time_deleted >= 调用开始时刻 - 2s` 的条目。
  - 时间比较用 `SystemTime::checked_sub(Duration::from_secs(2))`：`SystemTime` 的 `-` 在边界上会 panic，取不到下界时退化为「不做时间过滤、只按路径匹配」，不得因此 panic。
  - 找到 → 正常返回。
  - 未找到 → `Business`：`'<path>' is gone but was not found in the recycle bin; it may have been deleted permanently by the shell`。理由：Windows 在回收站容量/配额不足时，`IFileOperation` 可能**永久删除并返回成功**（`FOF_WANTNUKEWARNING` 在 `FOF_NO_UI` 下不产生交互），crate 的失败 sink 只覆盖「被拒绝」的条目，不覆盖「被 nuke」的条目。**这条校验是唯一能发现静默永久删除的手段。**
  - `list()` 不可用（macOS）或本身失败 → 记录 `log::debug!`，跳过校验，不改变本次结果（校验是附加诊断，不是功能前提）。
- **失败绝不自动回退**：不因为「回收站不可用」就改调 `remove_*`。文案统一附一句 `; pass permanent = true only if the user agreed to irreversible deletion`（`Business` 由模型回填给用户，用户可自行决定）。
- 失败文案沿用 I38c-delete-spec（已归档）§3.4 的「重查目标」规则：仍存在 → 「未移动 / 可能已部分移动」；已不存在 → 「已完成，但随后失败」。

### 6.3 永久分支（`permanent = true`，沿用旧实现）

- 文件 / 链接 → `remove_file`（目录链接回退 `remove_dir`）；目录 + `recursive = false` → `remove_dir`；目录 + `recursive = true` → `remove_dir_all`。
- **只读文件前置拒绝（`Business`）只在永久模式生效**：`remove_file` 会顺手清只读属性（等于删除改了属性，旧 §3.4 明确禁止）。回收站模式是**移动语义**，不触碰属性，因此允许回收只读文件——若在这里也拒绝，会凭空限制一个合法操作。
- 不做 `os_limited::list()` 校验（永久模式本来就不该进回收站）。

### 6.4 平台矩阵

| 平台 | 回收站实现 | 落点 | 失败是否可能静默永久删除 | 本工具的兜底 |
|---|---|---|---|---|
| Windows | `IFileOperation` + `FOF_ALLOWUNDO`（每卷回收站） | 各卷 `$Recycle.Bin` | **可能**（配额 / 超限 / 策略 nuke） | 卷根 `$Recycle.Bin` 前置检查 + `os_limited::list` 事后校验（§6.2） |
| macOS | `NSFileManager` trash | `~/.Trash` | 否（不可回收即返回错误） | 无需校验 |
| Linux | freedesktop trash 1.0：同卷 `rename`；跨卷 `copy + 删源` | `~/.local/share/Trash` 或卷内 `.Trash` | 否（失败保留源） | 跨卷时源被直接删除属「移动语义」的必然（目标已在回收站），不算降级 |

Linux 跨卷补充：`copy + 删源` 在只读 / 无权限 / 磁盘满时可能**复制成功但删源失败**，此时源与回收站副本同时存在（数据不丢，只是残留），crate 返回错误 → `Business`，文案按 §6.2 的「仍存在」分支表述。

## 7 错误

| 情况 | 行为 | 相对旧版 |
|---|---|---|
| `path` 缺失 / 类型错误 / `permanent` 非布尔 | `validate_input` 拒绝（`Business`） | 新增 `permanent` 行 |
| 目标越界 / 系统目录 / 凭据目录 / 根 / workspace 根 / 家目录 | `validate_input` 拒绝（`Business`） | 不变 |
| 目标不存在 | `Business` | 不变 |
| 非空目录 + `recursive = false` | `Business`，提示 `recursive = true` | 不变（两种模式一致） |
| `read_dir` 失败 | `Business` | 不变 |
| Windows 卷无 `$Recycle.Bin` | `Business`，提示可显式 `permanent = true` | 新增 |
| `trash::delete` 返回错误 | `Business`，按 §6.2 区分「未移动 / 已完成但随后失败」，附 `permanent = true` 提示 | 新增 |
| 回收站调用 panic（COM 初始化失败） | `Business`（`catch_unwind` 兜住） | 新增 |
| 目标已消失但不在回收站 | `Business`，明示「可能已被永久删除」 | 新增 |
| Windows 只读文件 + `permanent = true` | `Business`，只陈述「delete 不改属性」 | 不变 |
| `remove_*` 失败 / 超时 | `Business`，按「未删除 / 部分删除」区分 | 不变 |
| 审批被拒 | `Denied`（网关打标） | 不变 |

失败结果中不得出现「已移动 / 已删除」字样，唯一例外是「操作已完成，但随后失败」。

## 8 兼容性与迁移

| 面 | 影响 |
|---|---|
| schema | 只增 `permanent`，`required` 不变 → 旧调用不报错 |
| 行为 | **默认值变化即行为变更**：不传 `permanent` 的旧调用从「不可逆删除」变为「移入回收站」。这是本次变更的目的，需在发布说明中明示 |
| 结果 `data` | 只增 `permanent` 键；无前端消费方（已检索 `entriesDeleted` / `bytesDeleted`），模型侧按工具描述理解 |
| 审批缓存 | 旧 `delete` 授权继续有效，作用面收窄为「回收站模式」；永久删除需要新的 `purge` 授权（旧授权不会被套用，§5.1） |
| 审计 | 永久模式多一行 `purge` 记录；表结构不变 |
| 协议 / DTO | 不变（`ToolResult`、`ApprovalPreviewDto` 均未改字段）→ 不跑 `contract:gen` |
| 依赖 | `trash 5.3.0` 声明在根 `[workspace.dependencies]`，仅 `assistant-tools` 引用；workspace 其它 crate 无变化；`Cargo.lock` 新增若干平台依赖（Windows 侧仅 `scopeguard`，`windows 0.62` 已共用，接入后按 §3.2 用 `cargo tree` 复核） |
| 文档 | I38c-delete-spec（已归档）冻结不改，其 §3.4 / §3.5 / §5 的冲突条款由本文取代（见 §12） |

## 9 测试计划

**单元（无回收站副作用）**

1. schema：含 `permanent` 对象、`required` 仍为 `["path"]`、仍无 `force`。
2. 解析：缺失 / `null` → `false`；`true` / `false` → 原值；字符串 / 数字 → `Business` 且文案含 `'permanent' must be a boolean`。
3. intents：`permanent = false` → 1 条 `delete`；`permanent = true` → 2 条（`delete` + `purge`），两条 resource 都是同一个规范化绝对路径。
4. 预览：同一目标在两种模式下的 `summary` 必须分别含 `system recycle bin` 与 `permanently delete`；量级数字与 `scan_tree` 一致。
5. `data`：两种模式都带 `permanent`，值与入参一致。
5b. `volume_has_recycle_bin`（仅 Windows，纯函数）：`tempdir` 卷根无 `$Recycle.Bin` → `false`；UNC 路径 → `false`。
5c. 回执失效：`permanent = true` 成功后回执失效（既有用例覆盖）；「目标已消失但调用失败」的文案分支由 `trash_failure_message_reports_the_actual_state_on_disk` 直接断言纯函数，`execute` 的回执失效只看 `gone`（不看返回结果），因此两条失败路径共用同一条件。

**既有用例迁移（已落地，最终名称）**

- `schema_declares_path_recursive_and_permanent_without_force`：原 `schema_declares_path_and_recursive_without_force`，断言补 `permanent` 存在。
- `a_file_goes_to_the_recycle_bin_by_default` / `a_directory_goes_to_the_recycle_bin_recursively` / `an_empty_directory_goes_to_the_recycle_bin_without_recursive`：原「直接删除」三例改为默认模式断言 `Moved ... to the system recycle bin` + `data.permanent = false`。
- `a_file_is_permanently_deleted_on_request` / `a_directory_is_permanently_deleted_on_request` / `an_empty_directory_is_permanently_deleted_without_recursive`：显式 `permanent = true`，保留「不可逆删除」语义，且不产生回收站条目。
- `the_preview_reports_the_scale_and_the_destination`：原 `the_preview_reports_the_scale_before_approval`，按 §4.5 分叉断言 `system recycle bin` 与 `permanently delete`。
- `a_read_only_file_is_refused_in_permanent_mode`（原 `a_read_only_file_is_refused_without_an_unreachable_hint`）+ `a_read_only_file_is_moved_to_the_recycle_bin`（新增）：只读语义只在永久模式拒绝。
- `a_non_empty_directory_needs_recursive_in_both_modes`：原单模式用例扩成两模式断言。
- `preview_and_execution_report_the_same_scale`（源码内 `#[cfg(test)]`）：改用 `permanent = true` 保持无副作用。
- 链接三例（`deleting_a_file_link_keeps_its_target` / `deleting_a_directory_link_keeps_its_contents` / `deleting_a_dangling_link_succeeds`）与回执两例（`the_receipt_is_invalidated_after_delete` / `deleting_a_directory_invalidates_receipts_of_its_subtree`）：统一改用 `permanent = true`，避免测试残留进开发者回收站。
- 新增 `permanent_mode_adds_a_purge_intent`：断言两模式 intent 条数与 resource 一致性。

**集成（真实文件系统）**

6. 回收站移走文件：`execute` 成功 → 目标不存在、`data.permanent = false`；若平台支持 `os_limited::list`，能查到 `original_path() == target` 的条目。
7. 回收站移走目录（`recursive = true`）：目标不存在，条目数 / 字节数与 `scan_tree` 一致。
8. `permanent = true`：目标不存在，且 `os_limited::list` 中**没有**该路径的新条目（证明没进回收站）。
9. 链接：回收站模式下链接消失、目标仍存在且内容未变。
10. 非空目录 + `recursive = false`：两种模式都 `Business`，且目标仍在。
11. 只读文件（仅 Windows）：`permanent = true` → `Business`；`permanent = false` → 成功移入回收站。
12. 卷无 `$Recycle.Bin`（仅 Windows）：由 5b 的纯函数用例覆盖；`execute` 侧只多一次 `ancestors().last()` 判定，测试环境无法构造「真实但无回收站的卷」，故不设集成用例。

**测试卫生（必须遵守）**

- 会进回收站的用例**必须自己清理**：`os_limited::list()` 过滤 `original_path()` 落在 `tempdir` 下的条目，`purge_all` 掉。不得把 `tempdir` 垃圾留在开发者 / CI 的真实回收站里。
- 回收站可用性探针：首次 `trash::delete` 失败即视为「本机无可用回收站」，`eprintln!` + 跳过（沿用既有 `try_link_dir` 的 skip 模式），不把环境缺失判成代码缺陷。
- 依赖 `os_limited` 的用例 `#[cfg(any(windows, target_os = "linux"))]`，macOS 跳过。

## 10 验收标准

- [x] `delete` 默认把文件 / 链接 / 目录移入系统回收站（自动化用例已通过）；「可在系统回收站还原」待 Windows 手动验证一次。
- [x] `permanent = true` 走不可逆删除，且不产生回收站条目。
- [x] 回收站不可用时返回 `Business`，目标仍在磁盘上，**没有任何**永久删除发生（无自动回退分支）。
- [x] 永久模式的审批卡片同时出现 `delete` 与 `purge` 两行；回收站模式只有 `delete`。
- [x] 旧调用（不传 `permanent`）不报 schema 校验错误。
- [x] 非空目录 + `recursive = false` 在两种模式下都确定性失败。
- [x] 链接始终按链接处理，目标内容未被触碰。
- [x] 依赖接入后 `cargo tree -p assistant-tools` 复核通过（`trash` 5.3.x、`windows` 仅一份 0.62、默认特性为 STA）。
- [x] `cargo test -p assistant-tools --test agent_delete`（cwd = `src-tauri`）24 例通过；`cargo test -p assistant-tools` 全量通过；`cargo clippy -p assistant-tools --all-targets` 无新增告警。
- [ ] 前端 i18n 两语言文案已更新（`zh-CN.json` / `en-US.json` 键位一致），`npm run lint` 与 `npm run typecheck` 通过。

## 11 风险与残差

| # | 风险 | 处置 |
|---|---|---|
| R1 | Windows 在配额不足 / 超限 / 策略 nuke 时可能「永久删除并返回成功」 | `os_limited::list()` 事后校验（§6.2）+ 卷根 `$Recycle.Bin` 前置检查；仍无法**阻止**，只能**发现并如实报告**。要彻底消除需自建 COM 实现并检查 `PostDeleteItem` 的落点，成本不划算（§3.2） |
| R2 | 回收站是共享资源：大目录可能撑满用户配额，导致后续操作更难回收 | 不代管容量（非目标）；失败文案明确「可显式 `permanent = true`」 |
| R3 | Linux 跨卷 `copy + 删源` 慢、可能残留副本 | 文案如实报告「目标仍存在」；不做自动重试 / 分批 |
| R4 | 回收站内容不受本工具管控：模型删掉的敏感文件会在回收站留痕 | 属系统行为；系统目录 / 凭据目录已在解析阶段拒绝，回收站不扩大可删范围 |
| R5 | Windows COM STA 初始化与既有 MTA 调用同线程冲突 | `catch_unwind` 兜底 → `Business`，不让 panic 穿透（§6.2） |
| R6 | 审批级别 `none` 下永久删除也不弹卡 | 该级别语义即「全部自动执行」，与现状一致；用户可改用 `dangerous_only` / `all_os_tools` |

## 12 与归档文档的条款对照

| I38c-delete-spec（已归档） | 本文 |
|---|---|
| §3.4「不自动清只读属性……对目录递归删除仍交给 `remove_dir_all`」 | 保留，但**只读前置拒绝收窄为永久模式**（§6.3）：回收站是移动语义，不碰属性 |
| §3.4 分支表（`remove_file` / `remove_dir` / `remove_dir_all`） | 作为 `permanent = true` 分支保留（§6.3）；默认分支改为回收站（§6.2） |
| §3.5 结果 `data` 与 `result_for_assistant` | 增 `permanent` 键、文案按模式分叉（§4.4） |
| §5「不采纳：回收站 / 软删除 / 自动备份（超出 P5）」 | **本条被取代**：回收站改为默认行为；「应用级软删除」仍不采纳（§3.2） |
| §1「`permission_intents` → `delete` + 规范化绝对路径」 | 回收站模式不变；永久模式增 `purge`（§5.1） |
| §2「schema 不暴露 `force`」 | 保留；新增 `permanent` 不承担跳过检查的职责（§4.1） |
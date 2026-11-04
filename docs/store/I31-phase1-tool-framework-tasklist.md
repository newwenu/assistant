# I31 — P1 框架收口（T3）任务清单

> 规格：[I31-phase1-tool-framework-spec.md](./I31-phase1-tool-framework-spec.md) §5、§10、§11
> 阶段：[I38-agent-roadmap.md](./I38-agent-roadmap.md) P1
> 顺序：T3.0 → T3.1 → T3.2 → T3.3 → T3.4 → T3.5 → T3.6 → T3.7 → T3.8 → T3.9 → T3.10 → T3.11
> 勾选条件：代码已改 **且** 对应验收已跑。
> 范围：只交付 `os/path.rs` 与 `fs_link` 迁移；不新增工具、命令、DTO 或前端改动。

---

## T3.0 口径与前置确认

- [x] T3.0.1 确认 P1 只做 `os/path.rs` + `fs_link` 迁移，不新增任何 `fs_*` / `shell_exec` 工具
- [x] T3.0.2 确认 `fs_link` 的 `audience()` 保持 `TOOLBOX_ONLY`，不改为 `BOTH`
- [x] T3.0.3 确认 `fs_link` 的 `source` / `target` 一律走 `resolve_leaf_preserving`；`resolve` 本阶段没有生产调用方
- [x] T3.0.4 确认 `\\?\` 统一用 `dunce::simplified()`：workspace 根 `Cargo.toml` 增加 `dunce = "1"`，`assistant-tools` 以 workspace 依赖引入
- [x] T3.0.5 确认 `workspace_root = None` 时相对路径返回明确错误；`os_tool_exec` 恒传 `None` 属 Step 8，本阶段不改
- [x] T3.0.6 确认 `tempfile` 已在 `dev-dependencies`，T3 单测直接使用，无需新增依赖
- [x] T3.0.7 确认 T3 单独提交、单独回滚，不与其他阶段改动混批

---

## T3.1 `os/path.rs` 骨架

`src-tauri/crates/assistant-tools/src/os/path.rs`、`src-tauri/crates/assistant-tools/src/os/mod.rs`、`Cargo.toml`

- [x] T3.1.1 新建 `os/path.rs`，`os/mod.rs` 增加 `pub mod path;`
- [x] T3.1.2 workspace 根 `Cargo.toml` 增加 `dunce = "1"`；`assistant-tools/Cargo.toml` 增加 `dunce = { workspace = true }`
- [x] T3.1.3 定义五个 `pub fn`：`resolve` / `resolve_leaf_preserving` / `canonicalize_lenient` / `reject_system_dirs` / `ensure_within_root`
- [x] T3.1.4 每个函数补文档注释：输入、返回、失败条件；`resolve` 明确「调用方必须使用返回值」
- [x] T3.1.5 `path.rs` 不得依赖 `tauri`、`AppHandle`、`Window`、`Command`
- [x] T3.1.6 不新增 `pub` 类型；路径统一用 `PathBuf` / `Path`

---

## T3.2 `canonicalize_lenient`

`src-tauri/crates/assistant-tools/src/os/path.rs`

- [x] T3.2.1 任何 `Component::ParentDir` 直接拒绝，错误信息包含原始路径
- [x] T3.2.2 `path.exists()` 时调用 `canonicalize()`，再经 `dunce::simplified()` 归一
- [x] T3.2.3 路径不存在时，向上找到第一个存在的祖先 `A`，剩余组件 `R` 原样拼接：`canonicalize(A)?.join(R)`
- [x] T3.2.4 找不到任何存在的祖先（如盘符不存在）返回 `Err`，不得回退原路径
- [x] T3.2.5 crate 内不存在 `unwrap_or_else(|_| path.to_path_buf())` 形式的降级

---

## T3.3 Windows 路径形态

`src-tauri/crates/assistant-tools/src/os/path.rs`

- [x] T3.3.1 `\\?\C:\...` 经 `dunce::simplified()` 后不含 `\\?\` 前缀
- [x] T3.3.2 `\\?\UNC\server\share` 归一为 `\\server\share`
- [x] T3.3.3 盘符相对路径（`C:foo`）明确拒绝：`relative path with drive prefix is not supported`
- [x] T3.3.4 UNC 路径（`\\server\share\x`）视为绝对路径，可解析
- [x] T3.3.5 比较与拼接前统一去尾随分隔符，根目录除外
- [x] T3.3.6 `a/./b/../c` 先做组件规范化，再执行 `ParentDir` 检查
- [x] T3.3.7 `workspace_root` 自身是符号链接时，`ensure_within_root` 两侧都用 `canonicalize_lenient` 后的结果比较

---

## T3.4 `reject_system_dirs`

`src-tauri/crates/assistant-tools/src/os/path.rs`

- [x] T3.4.1 按组件序列比较，不用字符串前缀
- [x] T3.4.2 Windows 比较大小写不敏感
- [x] T3.4.3 Windows 受保护目录从 `SystemRoot` / `ProgramFiles` / `ProgramFiles(x86)` / `ProgramData` 读取，不写死盘符
- [x] T3.4.4 Unix 保持 `["/etc", "/usr", "/bin", "/sbin", "/System", "/Library"]`，按组件序列比较
- [x] T3.4.5 `C:\Windows.old` 放行；`c:\windows\system32` 拒绝
- [x] T3.4.6 `root` 自身不豁免：`reject_system_dirs` 先于 `ensure_within_root`

---

## T3.5 `resolve` / `ensure_within_root` / `resolve_leaf_preserving`

`src-tauri/crates/assistant-tools/src/os/path.rs`

- [x] T3.5.1 `resolve` 按顺序执行：绝对路径或相对路径 + `workspace_root` → `canonicalize_lenient` → `reject_system_dirs` → `ensure_within_root` → 返回归一后的 `PathBuf`
- [x] T3.5.2 `workspace_root = None` 且路径为相对路径时，错误信息明确包含「requires workspace_root」
- [x] T3.5.3 `ensure_within_root`：`path` 以 `root` 为组件级前缀；`path == root` 允许；Windows 大小写不敏感
- [x] T3.5.4 `resolve_leaf_preserving`：父目录走 `canonicalize_lenient`，叶子 `FileName` 原样 `join`
- [x] T3.5.5 `resolve_leaf_preserving` 仍执行 `reject_system_dirs` 与 `ensure_within_root`
- [x] T3.5.6 `resolve` 与 `resolve_leaf_preserving` 不修改 `ToolUseContext`

---

## T3.6 `canonicalize_lenient` / Windows 形态单测

`src-tauri/crates/assistant-tools/src/os/path.rs`（`#[cfg(test)]`）

- [x] T3.6.1 `..` 遍历被拒绝
- [x] T3.6.2 不存在路径的宽松规范化：返回「存在祖先 + 尾部」
- [x] T3.6.3 不存在祖先（非法盘符）返回 `Err`
- [x] T3.6.4 `\\?\` 前缀被剥离（Windows）
- [x] T3.6.5 `\\?\UNC\` 归一为 `\\`（Windows）
- [x] T3.6.6 `C:foo` 被明确拒绝（Windows）
- [x] T3.6.7 UNC 路径可解析（Windows）
- [x] T3.6.8 尾随分隔符与 `.` / `..` 混合输入归一正确

---

## T3.7 `reject_system_dirs` / workspace 单测

`src-tauri/crates/assistant-tools/src/os/path.rs`（`#[cfg(test)]`）

- [x] T3.7.1 `C:\Windows.old` 不被误拒（Windows）
- [x] T3.7.2 `c:\windows\system32` 被拒绝（Windows，大小写不敏感）
- [x] T3.7.3 workspace 越界路径被拒绝
- [x] T3.7.4 `root` 自身允许
- [x] T3.7.5 `root` 为符号链接时比较正确
- [x] T3.7.6 `workspace_root = None` 时相对路径报出明确错误
- [x] T3.7.7 `workspace_root` 位于受保护目录时被拒绝
- [x] T3.7.8 Unix 受保护目录按组件序列拒绝

---

## T3.8 `fs_link` 迁移

`src-tauri/crates/assistant-tools/src/os/fs_link.rs`

- [x] T3.8.1 删除私有 `resolve_path` / `validate_path`
- [x] T3.8.2 `create` 的 `source` / `target` 与 `check` 的 `source` 全部改走 `os::path::resolve_leaf_preserving`
- [x] T3.8.3 `execute()` 内只用解析后的 `PathBuf` 做 IO，不再使用原始字符串
- [x] T3.8.4 `exec_create` 的「target 不存在」检查基于解析后的路径
- [x] T3.8.5 `exec_check` 继续用 `symlink_metadata` 判断叶子，不跟随链接
- [x] T3.8.6 Windows junction 的 `mklink` 入参使用简化后的路径，不含 `\\?\`
- [x] T3.8.7 父目录不存在的错误信息指出具体路径，且不泄露内部实现细节

---

## T3.9 `permission_intents` 词汇修正

`src-tauri/crates/assistant-tools/src/os/fs_link.rs`

- [x] T3.9.1 `create` 产出 `PermissionIntent::new("edit", source)` 与 `PermissionIntent::new("edit", target)`
- [x] T3.9.2 `check` 仍返回空 `Vec`
- [x] T3.9.3 全 crate 不再出现 `fs_link_create` / `fs_link_target`
- [x] T3.9.4 全 crate 的 `permission_intents` 只产出 `read` / `edit` / `delete` / `execute`

---

## T3.10 `fs_link` 回归测试

`src-tauri/crates/assistant-tools/src/os/fs_link.rs`（`#[cfg(test)]`）

- [x] T3.10.1 source 不存在时可创建
- [x] T3.10.2 source 父目录不存在时错误信息明确
- [x] T3.10.3 `check` 对真实符号链接返回 `isLink = true`（叶子不被跟随）
- [x] T3.10.4 `workspace_root` 位于受保护目录时被明确拒绝
- [x] T3.10.5 Windows junction 用例通过；无特权/CI 环境允许 `#[ignore]` 或跳过
- [x] T3.10.6 target 父目录不存在的 junction 场景有明确错误
- [x] T3.10.7 迁移前后 `create` / `check` 的正常路径结果一致

---

## T3.11 验收命令

`cwd = src-tauri`

- [x] T3.11.1 `cargo test -p assistant-tools` 全绿
- [x] T3.11.2 `cargo clippy -p assistant-tools -- -D warnings` 无告警
- [x] T3.11.3 先执行格式化修复，再 `cargo fmt --all --check` 通过
- [x] T3.11.4 `cargo test --workspace` 通过（至少确认 `assistant-tools` 之外无编译回归）— 本机 `assistant-app` lib 测试二进制启动即 `STATUS_ENTRYPOINT_NOT_FOUND`（已记录的机器级限制），改以 `cargo test --workspace --no-run` 确认全量编译通过
- [x] T3.11.5 全 crate grep 确认无 `unwrap_or_else(|_| path.to_path_buf())`（A10）
- [x] T3.11.6 全 crate grep 确认无 `fs_link_create` / `fs_link_target`（A8）
- [x] T3.11.7 确认生产注册表仍只有 `fs_link` 一个工具（A9）

---

## T3.12 文档与提交

- [x] T3.12.1 更新 I31 规格头表状态为「T3 已落地」
- [x] T3.12.2 更新 I31 §2 事实表：F4/F5/F6/F7 状态改为已修复
- [x] T3.12.3 更新 I31 §10 验收表：A4/A7/A8/A10 状态改为已达成
- [x] T3.12.4 更新 I31 §11：T3 标记完成，补一行实际交付说明
- [x] T3.12.5 更新 I38 roadmap §2：P1 标记已完成，并记录日期
- [ ] T3.12.6 单独提交，提交信息标注 `I31 T3` / `P1`

---

## 验收映射

| 规格验收 | 对应任务 |
|---|---|
| A4 `path.rs` 单测覆盖 | T3.6、T3.7 |
| A7 `fs_link` 迁移回归 | T3.8、T3.10 |
| A8 `permission_intents` 四类 action | T3.9、T3.11.6 |
| A10 无规范化降级 | T3.2.5、T3.11.5 |
| A1 / A2 / A3 | T3.11.1 – T3.11.3 |
| A9 生产注册表仍为 1 | T3.11.7 |

---

## 不做

- 不新增 `fs_read` / `fs_list` / `fs_glob` / `fs_grep` / `fs_write` / `fs_edit` / `fs_mkdir` / `fs_move` / `fs_delete` / `shell_exec`
- 不建 `os/output.rs`
- 不新增 Tauri command、不改 DTO、不改前端
- 不实现 Policy / Gateway / Pipeline
- 不引入 `thiserror`
- 不把 `fs_link` 改成 `AGENT_ONLY` 或 `BOTH`
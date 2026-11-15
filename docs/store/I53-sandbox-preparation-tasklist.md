# I53 — P14-a 沙箱准备 任务清单

> 规格：[I53-sandbox-preparation-spec.md](./I53-sandbox-preparation-spec.md)
> 事实基线：规格 §1.1 的 S-N1–S-N4（编写清单时对 `assistant-tools/src/lib.rs`、`src-tauri/src/tool_setup.rs`、两处 `Cargo.toml` 的实读结果）
> 前置：I52 的 C1（审批键 `sandboxEnforcement` 字段与求值顺序）。C1 只改 `trait.rs` / `gateway.rs` / `policy.rs` 与单测，与本文代码路径不重叠，可并行；但 C4 的注册期结论是 C1 审批卡展示的输入，C4 不得晚于 I52 的 C5。

**范围**：只覆盖规格 §9 的 C2 / C3 / C4（S0 沙箱准备）。`shell` 工具本体（I52 的 C1 / C4b / C5–C8）不在本清单，随 I52 开工另建。

批次对应规格章节：C2.0 骨架与依赖（§4.1 / §4.2 / §5.5）→ C2.1 Windows 探测（§5.1 / §4.5）→ C2.2 Windows 准备（§5.1 / §6）→ C2.3 Windows 令牌 + Job + spawn 时序（§5.1）→ C2.4 Windows 验收（§8）→ C3.1 Linux（§5.2 / §10 OQ10）→ C3.2 macOS（§5.3）→ C3.3 跨平台编译 → C4 探测与注册收口（§4.4）→ C5 门禁与收口（§8 全量）。

依赖：C2.0 → C2.1 → C2.2 → C2.3 → C2.4 严格串行（探测结论决定准备是否落地，准备产物是 spawn 的输入）；C3 只依赖 C2.0，与 C2 并行（两平台不共享代码）；C4 依赖 C2 / C3 的后端实现（可先接桩）；C5 依赖全部。

**环境事实**：开发机为 Windows，C2 可实机验收；C3 在本机只能做编译检查（`rustup target add` 后 `cargo check --target`），实机断言项一律标「未验证」，不得打勾。

> 本清单不管理提交。提交粒度按代码依赖与可编译性现场决定；这里只回答「代码改没改、验收跑没跑」。
> 全部任务初始未实施；勾选表示代码已改且对应验收已跑。

---

## 阶段 C2.0 — 骨架、类型与依赖（跨平台共用）

对应规格 §4.1 / §4.2 / §5.5。本批不产生任何平台实现，只把抽象钉死，避免三个后端各写一套类型。

- [ ] `assistant-tools/src/lib.rs` 增 `pub mod sandbox;`（`mod` 块插在 `result` 与 `storage` 之间，按字母序）与 `pub use sandbox::*;`（`pub use` 块就近同位置），与既有模块同风格
- [ ] 新增 `assistant-tools/src/sandbox/mod.rs`：`SandboxKind` / `SandboxEnforcement` / `SandboxDimension` / `SandboxProbe` / `SandboxPlan` / `SandboxError` 按规格 §4.1 逐字段定义
- [ ] 三个枚举派生 `Debug, Clone, Copy, PartialEq, Eq`；`SandboxProbe` 派生 `Debug, Clone` 且**只含 `kind` / `enforcement` / `missing` / `detail`**——禁止塞 `PathBuf`、用户名、命令内容（`detail` 是固定串）
- [ ] `SandboxChild` 与 `SandboxBackend` 按规格 §4.2 逐字落地：trait 必须 `Send + Sync`；`stdout` / `stderr` 返回 `Option<Box<dyn std::io::Read + Send>>`，**禁止**写 `std::process::ChildStdout` / `ChildStderr`；`wait` 为同步签名
- [ ] `SandboxPlan` 保留 `network` / `stdin` 字段但 P14 恒 `false` / `None`；**禁止**新增 `writable_paths: Vec<PathBuf>` 一类的多根字段（单根是写边界成立的前提，§4.3）
- [ ] 新增 `SandboxHostPaths`（或等价入参结构）：只含 `sensitive_read_deny: Vec<PathBuf>`；`writable_root` **不在其中**（按会话解析、每次 `spawn` 现传，§4.3）
- [ ] `assistant-tools` 侧**禁止**自行解析 `AppDirs` / 用户主目录：路径由 `tool_setup` 注入，`assistant-tools` 仍不依赖 tauri / `app_dirs`（A-S6）
- [ ] 各平台后端结构体在构造期持有 `SandboxHostPaths`（`SandboxBackend::spawn` 的签名不带 host 入参，§4.2），`prepare` / `converge` 一律从 `self` 取敏感列表
- [ ] `SandboxEnforcement` 由 `missing` 唯一推导：`enforcement_from_missing(&[SandboxDimension]) -> SandboxEnforcement`（§4.5），三个后端只填 `missing`，**禁止**各自写死档位
- [ ] `src-tauri/Cargo.toml` 的 workspace `windows` 特性按实际用到的 API 逐个补：预计 `Win32_System_JobObjects`、`Win32_System_Threading`、`Win32_System_Pipes`、`Win32_Security_Authorization`；以 `cargo check` 报的缺模块为准，**禁止**整包开特性
- [ ] `assistant-tools/Cargo.toml` 增 `[target.'cfg(target_os = "linux")'.dependencies] landlock = { workspace = true }`，并在 workspace `[workspace.dependencies]` 登记版本；**禁止**加 `[target.'cfg(target_os = "macos")']` 依赖（macOS 走外部 `sandbox-exec`）
- [ ] 跑 `cargo check -p assistant-tools`（Windows）与 `cargo tree -p assistant-tools`，确认无 tauri / 无 `app_dirs`（A-S6）

**依赖**：无

---

## 阶段 C2.1 — Windows：`probe()`

对应规格 §5.1 与 §4.5。探测决定「注不注册」，必须是最保守的一步。

- [ ] `assistant-tools/src/sandbox/windows.rs`：`probe()` 三项能力试跑——`CreateRestrictedToken` 能造出受限令牌、`CreateJobObjectW` + 设置 `JOBOBJECT_EXTENDED_LIMIT_INFORMATION` 能成功、`GetNamedSecurityInfoW` 能读到 `sensitive_read_deny` 中首个存在目录的 ACL（探测期拿不到 `writable_root`，§4.4 入参约束）
- [ ] 试跑产物用完即关（令牌 / Job 句柄立刻 `CloseHandle`）；**禁止**在 `probe` 里改任何 ACL、建任何文件、spawn 任何进程（§6「探测只读」）
- [ ] `SandboxHostPaths.sensitive_read_deny` 为空 → 直接 `enforcement = None`（fail-closed）：空列表等于「不拒读」，不得算作 `partial` / `full`
- [ ] 三项成功 → `kind = WindowsRestrictedToken`、`missing = [Network]`，`enforcement` 走 C2.0 的共用推导得 `Partial`（§4.5 平台预期表）；任一失败 → `missing` 覆盖到 `FsWrite` → 推导出 `None` + `detail` 固定串（后端名 / API 名 / 失败类别，不含路径）
- [ ] 单测：注入失败桩 → `enforcement == None`；成功桩 → `missing == [Network]` 且 `enforcement == Partial`

**依赖**：C2.0

---

## 阶段 C2.2 — Windows：准备（ACL 写边界）

对应规格 §5.1 与 §6。持久副作用只在这里产生，幂等与「只碰允许的路径」是本批的两条硬线。

- [ ] 新增 `assistant-tools/src/sandbox/windows/acl.rs`（或 `sandbox/windows_acl.rs`）：`prepare(&SandboxPlan, &SandboxHostPaths)` 与 `converge(&SandboxHostPaths)` 两个入口
- [ ] 机制二选一按 §10 OQ1 原型定稿：**先跑机制 A**（合成 SID 在可写根上授写），A-S1 / A-S2 不通过再跑机制 B（受保护路径上加 deny 写）；两者都不过 → `probe` 改判 `enforcement = None`，不允许「先跑起来再说」
- [ ] 可写根：受限 SID 授 `FILE_GENERIC_READ` / `FILE_GENERIC_WRITE` / `FILE_GENERIC_EXECUTE`
- [ ] 固定敏感目录：受限 SID 加 **deny** ACE（`DENY` + `FILE_GENERIC_READ`）；目录不存在则跳过，**禁止**创建（创建会改变用户目录布局）
- [ ] 只碰 `writable_root` 与 `sensitive_read_deny` 列出的子目录：**禁止**修改盘根、`C:/Windows/**`、用户主目录本身（§5.6 / §6 硬约束 2）
- [ ] 幂等：先 `GetExplicitEntriesFromAclW` 按「SID + ACE 头」查重，仅缺失时 `SetEntriesToAclW` / `SetNamedSecurityInfoW`；**禁止**用进程内布尔「已准备」短路（跨重启失效，且残留无法收敛，§6 硬约束 3）
- [ ] `converge()`：清理可写根与敏感子目录上的受限 SID 残留 ACE；失败只 `log::warn!`，**禁止**阻断注册（§6 收敛行）
- [ ] 单测（`#[cfg(windows)]`）：临时目录连跑两次 `prepare` → ACE 计数不变（A-S7）；`converge` 后断言无受限 SID ACE 残留

**依赖**：C2.1

---

## 阶段 C2.3 — Windows：受限令牌 + Job + spawn 时序

对应规格 §5.1。创建与限制是同一段时序，拆开就会出现「跑起来但不在 Job 里」的窗口。

- [ ] `spawn` 首步调 `prepare(&plan, &self.host)`；失败直接返回 `SandboxError::SetupFailed`，**不创建任何进程**（§6 硬约束 1 / F-S3）
- [ ] `CreateRestrictedToken`：`DISABLE_MAX_PRIVILEGE` + `Everyone` / `Users` / `Authenticated Users` 置 deny-only
- [ ] `CreateProcessW` 用 `CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT`，`program` / `args` / `cwd` 全部取自 `SandboxPlan`，**禁止**经 `PATH` 解析
- [ ] `CreatePipe` 建 stdout / stderr 两根管道，父进程侧用 `File::from_raw_handle` 包成 `Box<dyn Read + Send>`；**必须在 `ResumeThread` 之前关掉父进程持有的写端句柄**，否则读侧永不 EOF（I52 §5.7 第 9 步的 join 会挂死）
- [ ] `stdin` 恒 `None` → 不建 stdin 管道，子进程侧 stdin 置空句柄
- [ ] `CreateJobObjectW` + `KILL_ON_JOB_CLOSE` + UI 限制全开（剪贴板 / 桌面切换 / 全局原子 / 系统参数）；**禁止**设 `ACTIVE_PROCESS` 上限（`cmd /C` 需要派生子进程）
- [ ] 时序固定 `CreateProcessW(CREATE_SUSPENDED)` → `AssignProcessToJobObject` → `ResumeThread`；任一步失败 → 杀进程 + 关全部句柄 + 按来源返回 `SetupFailed` / `SpawnFailed`
- [ ] `wait()`：`WaitForSingleObject` + `GetExitCodeProcess` → `ExitStatus`（Windows 侧用 `std::os::windows::process::ExitStatusExt::from_raw`）；**禁止**声明为 `async`
- [ ] `kill_tree()`：`TerminateJobObject` 一次杀光，幂等可重复调用；**禁止**只 `TerminateProcess` 直接子进程
- [ ] `SandboxChild` 的 `Drop` 兜底 `kill_tree()` + `wait()`（收尸），并在 drop 时关闭 Job 句柄（`KILL_ON_JOB_CLOSE` 双保险）
- [ ] 单测（`#[cfg(windows)]`）：`SandboxPlan` 直接驱动后端跑 `cmd /C exit 7` → 退出码 7；`stdout` 有内容时 `read` 到 EOF 后 `wait` 立即返回（证明写端已关）

**依赖**：C2.2

---

## 阶段 C2.4 — Windows 验收（A-S1 / A-S2 / A-S3 / A-S5 / A-S7）

对应规格 §8。`shell` 工具尚不存在，测试一律直接驱动 `SandboxBackend`（`program` = `cmd.exe`，`args` = `["/C", ...]`），不经审批链。

- [ ] 新增 `assistant-tools/tests/sandbox_windows.rs`（整体 `#![cfg(windows)]`）
- [ ] A-S1 写边界：可写根内 `echo x> f.txt` 成功；`%USERPROFILE%` 下临时文件写失败且 stderr 可读
- [ ] A-S2 敏感读边界：`type %USERPROFILE%\.ssh\...` 读失败（固定列表命中项）
- [ ] A-S3 进程树回收：命令派生后台进程（`start /b` 或 `cmd /C start`）后 `kill_tree()` → Job 内进程计数归 0，且无残留
- [ ] A-S5 建立失败不降级：构造必然失败的准备（如不存在 / 不可写的可写根）→ 后端返回 `SandboxError::SetupFailed` 且**未产生任何子进程**（用进程计数或桩断言）；`sandbox_setup_failed` 的 `error_kind` 映射归 I52 C5，不在本批
- [ ] A-S7 准备幂等：见 C2.2 单测，此处补一条「残留 ACE 下重复执行命令不失败」
- [ ] OQ1 定稿：把选中的机制与实测结论写回规格 §10 OQ1 行（含 A-S1 / A-S2 的实机结果）

**依赖**：C2.3

---

## 阶段 C3 — Unix 后端（与 C2 并行）

### C3.1 Linux（Landlock）

对应规格 §5.2，**先解 §10 OQ10 再写代码**：Landlock 是 allow-list（默认全拒），`/` 授 `READ_FILE` 之后无法再对 `~/.ssh` 单独拒读——规格 §5.2「固定敏感目录显式不给读」在「`/` 全授读」的规则形状下不成立。

- [ ] 先定 OQ10 形状：(a) 收窄读白名单（`/` 只授 `EXECUTE`，读权按 `PathBeneath` 逐条授 `/usr`、`/etc`、`/proc`、`/dev`、`writable_root`、`$HOME` 的非敏感子树）；(b) 保留 `/` 全授读并把 `fsRead` 记入 `missing`（Linux 落 `partial`）。判据只看 A-S2 是否通过
- [ ] 新增 `assistant-tools/src/sandbox/linux.rs`：`probe()` 读 Landlock ABI（`landlock_create_ruleset` 探测），ABI 缺失 → `enforcement = None`；ABI ≥ 4 且形状 (a) → `missing` 为空 → `Full`；ABI < 4 → `missing` 含 `Network`
- [ ] `spawn` 在 `Command::pre_exec` 内完成限制：`PR_SET_NO_NEW_PRIVS` → 建 ruleset → `landlock_restrict_self`；**任一步失败即 `_exit(126)`**，父进程不得在未限制形态下继续（F-S4）
- [ ] `pre_exec` 内 `setsid`；`kill_tree()` 用 `kill(-pgid, SIGKILL)`，幂等
- [ ] `pre_exec` 闭包内**禁止**分配堆内存之外的危险操作（只允许预先构造好的数据 + 系统调用），避免 fork 后锁状态未定义
- [ ] `wait()` / `stdout()` / `stderr()` 直接复用 `std::process::Child` 能力（Unix 可用标准库，与 Windows 的裸句柄路径不同）
- [ ] 单测（`#[cfg(target_os = "linux")]`）覆盖 A-S1 / A-S2 / A-S3；本机无法跑 → 标「未验证」

**依赖**：C2.0

### C3.2 macOS（Seatbelt）

对应规格 §5.3。

- [ ] 新增 `assistant-tools/src/sandbox/macos.rs`：`probe()` 检查 `/usr/bin/sandbox-exec` 存在且可执行，并用最小 profile 试跑 `/usr/bin/true`；失败 → `enforcement = None`
- [ ] `program` 硬编码 `/usr/bin/sandbox-exec -p <profile> <program> <args…>`，**禁止**经 `PATH` 查找（防同名劫持）
- [ ] profile：`(deny default)` + `(allow file-read*)` + `(allow file-write* (subpath "<writable_root>"))` + 必需的 `process-exec*` / `sysctl-read` / `mach-lookup` 最小集；敏感目录在 `file-read*` 之后**后置** `(deny file-read* (subpath "…"))`（Seatbelt 后置规则覆盖前置）
- [ ] `(deny network*)`；`pre_exec` 内 `setsid`，`kill_tree()` 用 `kill(-pgid, SIGKILL)`
- [ ] profile 拼串必须转义路径中的 `"` 与 `\`，**禁止**直接把未转义路径插进 `subpath` 表达式
- [ ] 单测（`#[cfg(target_os = "macos")]`）覆盖 A-S1 / A-S2 / A-S3；本机无法跑 → 标「未验证」

**依赖**：C2.0

### C3.3 跨平台编译检查

- [ ] `rustup target add x86_64-unknown-linux-gnu aarch64-apple-darwin` 后跑 `cargo check -p assistant-tools --target <t>`，两目标均无警告/错误
- [ ] 目标不可安装或本机缺链接器时登记「待 CI / 实机」，**不得**因此跳过 `#[cfg]` 门（未编译的分支等于未实现）

**依赖**：C3.1、C3.2

---

## 阶段 C4 — 探测与注册收口

对应规格 §4.4。本批是「fail-closed 真正生效」的一步：探测结论必须决定工具是否存在。

- [ ] `assistant-tools/src/sandbox/mod.rs` 增 `detect(host: &SandboxHostPaths) -> &'static SandboxProbe`，结论存 `OnceLock`；每平台候选列表按 §4.4（Windows `[restricted_token]` / Linux `[landlock]` / macOS `[seatbelt]` / 其它 `[none]`）；后端实例（已持有 host）的缓存与注入 `shell` 归 I52 C5，本批不落
- [ ] `detect()` 首次调用时执行一次 `converge()`（仅 Windows 有产物），失败只 `log::warn!`
- [ ] 候选按强度降序，第一个 `enforcement != None` 者胜出；P14 每平台一个候选，**禁止**预置未实现的降级候选（S1 才加）
- [ ] `src-tauri/src/tool_setup.rs` 的 `init`：构造 `SandboxHostPaths`（§5.4 固定列表：`~/.ssh`、`~/.aws`、`~/.gnupg`、`~/.netrc` + `AppDirs` 解析出的配置 / 密钥 / 数据库目录 + 平台凭据目录；不存在的目录不加入，**禁止**创建）→ 调 `sandbox::detect(&host)` → 结论为 `None` 时**不注册** `shell`
- [ ] C5（I52）之前 `register_shell_tools` 尚不存在：本批只落「探测 + 结论 + 条件分支占位」，分支体留空并注明归 I52 的 C5；**禁止**注册任何空工具或临时 `shell`
- [ ] 注册期只探测一次；运行期读缓存，**禁止**再调 `probe()`（A-S8，用计数桩断言调用次数 == 1）
- [ ] 单测：注入 `probe → None` 桩 → 注册表内无 `shell`（A-S4 注册侧）；注入运行期 `None` 结论 → 执行前置拒绝且**未产生子进程**（`sandbox_unavailable` 的 `error_kind` 映射归 I52 C5）
- [ ] `assistant-tools` 侧不解析工作区：`writable_root` 仍由 `ToolUseContext.workspace_root`（P12 的 `resolve_workspace` 唯一解析点）提供，缺失按 F-S5 拒绝

**依赖**：C2、C3（可先接桩）

---

## 阶段 C5 — 门禁与收口

- [ ] `cargo test -p assistant-tools`（Windows 全绿，含新增 `sandbox_windows.rs`）
- [ ] `cargo clippy -p assistant-tools --all-targets -- -D warnings`
- [ ] `cargo fmt --check`
- [ ] `cargo tree -p assistant-tools` 复核无 tauri / 无 `app_dirs`（A-S6）
- [ ] 逐条记录 A-S1–A-S8 的「平台 / 结论 / 日期」，未实跑的项标「未验证」，**禁止**为凑齐而打勾
- [ ] 确认本批次零 DTO 变更：`git diff --stat` 不含 `src/types/protocol.contract.ts` 与 `src-tauri/src/protocol/`（无需 `npm run contract:gen`）
- [ ] 确认未改 I52 的 `trait.rs` / `gateway.rs` / `policy.rs`（C1 归属，避免与本清单并行冲突）

**依赖**：C2.4、C3.3、C4

---

## 已知缺口（登记，不在本清单解决）

| 缺口 | 说明 | 归属 |
|---|---|---|
| Linux `fsRead` 形状未定 | 见 §10 OQ10；C3.1 开工第一件事是定形状，否则 Linux 只能落 `partial` | C3.1 |
| Windows 写边界机制未定 | 见 §10 OQ1；C2.2 原型二选一 | C2.2 |
| `sandbox-exec` 已 deprecated | 见 §10 OQ3；替换方案不在 P14 | S1 / 独立立项 |
| 非 Windows / Linux / macOS 平台 | `detect` 返回 `none` → 不注册 `shell`，不提供降级路径 | 规格 §4.4 已定 |
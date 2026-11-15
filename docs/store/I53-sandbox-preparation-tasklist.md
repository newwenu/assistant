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

- [x] `assistant-tools/src/lib.rs` 增 `pub mod sandbox;`（`mod` 块插在 `result` 与 `storage` 之间，按字母序）与 `pub use sandbox::*;`（`pub use` 块就近同位置），与既有模块同风格
- [x] 新增 `assistant-tools/src/sandbox/mod.rs`：`SandboxKind` / `SandboxEnforcement` / `SandboxDimension` / `SandboxProbe` / `SandboxPlan` / `SandboxError` 按规格 §4.1 逐字段定义
- [x] 三个枚举派生 `Debug, Clone, Copy, PartialEq, Eq`；`SandboxProbe` 派生 `Debug, Clone` 且**只含 `kind` / `enforcement` / `missing` / `detail`**——禁止塞 `PathBuf`、用户名、命令内容（`detail` 是固定串）
- [x] `SandboxChild` 与 `SandboxBackend` 按规格 §4.2 逐字落地：trait 必须 `Send + Sync`；`stdout` / `stderr` 返回 `Option<Box<dyn std::io::Read + Send>>`，**禁止**写 `std::process::ChildStdout` / `ChildStderr`；`wait` 为同步签名
- [x] `SandboxPlan` 保留 `network` / `stdin` 字段但 P14 恒 `false` / `None`；**禁止**新增 `writable_paths: Vec<PathBuf>` 一类的多根字段（单根是写边界成立的前提，§4.3）
- [x] 新增 `SandboxHostPaths`（或等价入参结构）：只含 `sensitive_read_deny: Vec<PathBuf>`；`writable_root` **不在其中**（按会话解析、每次 `spawn` 现传，§4.3）
- [x] `assistant-tools` 侧**禁止**自行解析 `AppDirs` / 用户主目录：路径由 `tool_setup` 注入，`assistant-tools` 仍不依赖 tauri / `app_dirs`（A-S6）
- [x] 各平台后端结构体在构造期持有 `SandboxHostPaths`（`SandboxBackend::spawn` 的签名不带 host 入参，§4.2）；**Windows 后端持有但不使用 `sensitive_read_deny`**（无读边界，§5.1），Linux / macOS 的 `pre_exec` / profile 从这里取列表
- [x] `SandboxEnforcement` 由 `missing` 唯一推导：`enforcement_from_missing(&[SandboxDimension]) -> SandboxEnforcement`（§4.5），三个后端只填 `missing`，**禁止**各自写死档位
- [x] `src-tauri/Cargo.toml` 的 workspace `windows` 特性按实际用到的 API 逐个补：预计 `Win32_System_JobObjects`、`Win32_System_Threading`、`Win32_System_Pipes`、`Win32_Security_Authorization`；以 `cargo check` 报的缺模块为准，**禁止**整包开特性
- [x] `assistant-tools/Cargo.toml` 增 `[target.'cfg(target_os = "linux")'.dependencies] landlock = { workspace = true }`，并在 workspace `[workspace.dependencies]` 登记版本；**禁止**加 `[target.'cfg(target_os = "macos")']` 依赖（macOS 走外部 `sandbox-exec`）
- [x] 跑 `cargo check -p assistant-tools`（Windows）与 `cargo tree -p assistant-tools`，确认无 tauri / 无 `app_dirs`（A-S6）

**依赖**：无

---

## 阶段 C2.1 — Windows：`probe()`

对应规格 §5.1 与 §4.5。探测决定「注不注册」，必须是最保守的一步。

- [x] `assistant-tools/src/sandbox/windows.rs`：`probe()` 三项能力试跑——`CreateRestrictedToken`（带 `WRITE_RESTRICTED`）能造出受限令牌、`CreateJobObjectW` + 设置 `JOBOBJECT_EXTENDED_LIMIT_INFORMATION` 能成功、`GetNamedSecurityInfoW` 能读到探测路径（`%TEMP%`）的 DACL（探测期拿不到 `writable_root`，§4.4 入参约束）
- [x] 试跑产物用完即关（令牌 / Job 句柄立刻 `CloseHandle`）；**禁止**在 `probe` 里改任何 ACL、建任何文件、spawn 任何进程（§6「探测只读」）
- [x] **不**对 `sensitive_read_deny` 做空列表判负：Windows 无读边界机制（§5.1），该列表不参与探测
- [x] 三项成功 → `kind = WindowsRestrictedToken`、`missing = [FsRead, Network]`，`enforcement` 走 C2.0 的共用推导得 `Partial`（§4.5 平台预期表）；任一失败 → `missing` 覆盖到 `FsWrite` → 推导出 `None` + `detail` 固定串（后端名 / API 名 / 失败类别，不含路径）
- [x] 单测：注入失败桩 → `enforcement == None`；成功桩 → `missing == [FsRead, Network]` 且 `enforcement == Partial`

**依赖**：C2.0

---

## 阶段 C2.2 — Windows：准备（ACL 写边界）

对应规格 §5.1 与 §6。持久副作用只在这里产生，幂等与「只碰允许的路径」是本批的两条硬线。

- [x] 新增 `assistant-tools/src/sandbox/windows/acl.rs`：单一入口 `prepare(&SandboxPlan)`（**不再需要** `SandboxHostPaths`：Windows 不碰敏感目录，§5.1）；「准备」与「收敛」合成一步（§6）
- [x] 机制已定稿（§10 OQ1）：`WRITE_RESTRICTED` + 合成 SID 在可写根授可继承写掩码；机制 B（受保护路径加 deny 写）**不再实现**
- [x] 可写根授权掩码 = `(FILE_GENERIC_WRITE | DELETE | FILE_DELETE_CHILD) & ~STANDARD_RIGHTS_WRITE`（`0x00110156`，核验见规格 §12），带 `SUB_CONTAINERS_AND_OBJECTS_INHERIT`；**不授**读位 / 执行位（pass-2 只检查写类访问），**不授** `WRITE_DAC` / `WRITE_OWNER`
- [x] **禁止**在敏感目录加 deny ACE：对合成 SID 的 deny 对访问检查无作用，对调用者自身 SID 的 deny 会破坏用户本人访问（§5.1「Windows 没有读边界」）
- [x] 只碰 `writable_root`：**禁止**修改盘根、`C:/Windows/**`、用户主目录本身、敏感目录（§5.6 / §6 硬约束 2）
- [x] 幂等：先 `GetExplicitEntriesFromAclW` 按「SID + ACE 头」查重，缺失或与期望掩码不一致时用 `SET_ACCESS` 语义一次 `SetEntriesInAclW` + 一次 `SetNamedSecurityInfoW` 完成「旧 ACE 替换 + 期望 ACE 写入」；**禁止**用进程内布尔「已准备」短路（跨重启失效，且残留无法收敛，§6 硬约束 3）
- [x] 收敛并入 `prepare`：同一 trustee 的旧 ACE 由期望 ACE 替换，不依赖 `REVOKE_ACCESS` + `GRANT_ACCESS` 两次调用的时序；失败返回 `SetupFailed` 且不 spawn（§6 硬约束 1 / 4）
- [x] 单测（`#[cfg(windows)]`）：临时目录连跑两次 `prepare` → ACE 计数不变（A-S7）；预置一条陈旧 ACE 后再 `prepare` → 只剩期望的那一条
- [x] 掩码口径用例 `write_grant_mask_shape`：把 `WRITE_GRANT_MASK` 的**字面值**（`0x00110156`）与 `WRITE_DAC` / `WRITE_OWNER` / `READ_CONTROL` 三条禁用位钉死（规格 §12）；**禁止**只拿常量与自身比较——那样常量改错也测不出来

> **运行要求**：本批用例会真的改 DACL，必须由**未受限令牌**执行（应用从普通桌面会话启动即满足）。从写受限令牌的终端（例如受沙箱的 AI shell）运行，会以 `acl_write_dacl` 失败——那是环境约束，不是回归（规格 §10 R4 / §12）。

**依赖**：C2.1

---

## 阶段 C2.3 — Windows：受限令牌 + Job + spawn 时序

对应规格 §5.1。创建与限制是同一段时序，拆开就会出现「跑起来但不在 Job 里」的窗口。

- [x] `spawn` 首步调 `prepare(&plan)`；失败直接返回 `SandboxError::SetupFailed`，**不创建任何进程**（§6 硬约束 1 / F-S3）
- [x] `CreateRestrictedToken`：`DISABLE_MAX_PRIVILEGE | WRITE_RESTRICTED`；**不用** `SidsToDisable`（宽泛组置 deny-only 会让 `cmd.exe` 以 `0xC0000022` 死亡）；restricting SID 表 = `[logon SID, Everyone, 合成 SID]`（§5.1 / §10 OQ1）
- [x] `CreateProcessAsUserW` 用 `CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT`（必须带令牌，`CreateProcessW` 无法指定令牌），**禁止**传 `CREATE_NO_WINDOW` / `CREATE_NEW_CONSOLE`（子进程会 `0xC0000142` 死亡，§5.1 已知缺口）；`program` / `args` / `cwd` 全部取自 `SandboxPlan`，**禁止**经 `PATH` 解析；`cwd` 传子进程前剥掉 `\\?\` 前缀（`cmd.exe` 会把它当 UNC 起点并回退到 `Windows` 目录，§4.3）
- [x] `CreatePipe` 建 stdout / stderr 两根管道，父进程侧用 `File::from_raw_handle` 包成 `Box<dyn Read + Send>`；**必须在 `ResumeThread` 之前关掉父进程持有的写端句柄**，否则读侧永不 EOF（I52 §5.7 第 9 步的 join 会挂死）
- [x] `stdin` 恒 `None` → 不建 stdin 管道，子进程侧 stdin 置空句柄
- [x] `CreateJobObjectW` + `KILL_ON_JOB_CLOSE` + UI 限制全开（剪贴板 / 桌面切换 / 全局原子 / 系统参数）；**禁止**设 `ACTIVE_PROCESS` 上限（`cmd /C` 需要派生子进程）
- [x] 时序固定 `CreateProcessAsUserW(CREATE_SUSPENDED)` → `AssignProcessToJobObject` → `ResumeThread`；任一步失败 → 杀进程 + 关全部句柄 + 按来源返回 `SetupFailed` / `SpawnFailed`
- [x] `wait()`：`WaitForSingleObject` + `GetExitCodeProcess` → `ExitStatus`（Windows 侧用 `std::os::windows::process::ExitStatusExt::from_raw`）；**禁止**声明为 `async`
- [x] `kill_tree()`：`TerminateJobObject` 一次杀光，幂等可重复调用；**禁止**只 `TerminateProcess` 直接子进程
- [x] `SandboxChild` 的 `Drop` 兜底 `kill_tree()` + `wait()`（收尸），并在 drop 时关闭 Job 句柄（`KILL_ON_JOB_CLOSE` 双保险）
- [x] 单测（`#[cfg(windows)]`）：`SandboxPlan` 直接驱动后端跑 `cmd /C exit 7` → 退出码 7；`stdout` 有内容时 `read` 到 EOF 后 `wait` 立即返回（证明写端已关）

**依赖**：C2.2

---

## 阶段 C2.4 — Windows 验收（A-S1 / A-S2(诚实性) / A-S3 / A-S5 / A-S7）

对应规格 §8。`shell` 工具尚不存在，测试一律直接驱动 `SandboxBackend`（`program` = `cmd.exe`，`args` = `["/C", ...]`），不经审批链。

- [x] 验收单测落在 `assistant-tools/src/sandbox/windows.rs` 的 `#[cfg(test)] mod tests`：`acl::prepare` 是 `pub(crate)`，集成测试文件（`tests/sandbox_windows.rs`）够不到它，而 A-S7 的前半必须直接驱动 `prepare`
- [x] A-S1 写边界：可写根内 `echo x> f.txt` 成功、`del f.txt` 成功（掩码含删除位）；`%USERPROFILE%` 下临时文件写失败且 stderr 可读
- [x] A-S2 诚实性（Windows 无正向读拒绝，§5.1）：断言 `probe.missing` 含 `FsRead`；**不得**写「读 `~/.ssh` 失败」的断言（机制不可达，写了就是假验收）。另留一条反向护栏：敏感目录的读取仍成功，任何「给敏感目录加 deny ACE」的回归都会让它失败
- [x] A-S3 进程树回收：命令派生后台进程（`start /b` 或 `cmd /C start`）后 `kill_tree()` → Job 内进程计数归 0，且无残留
- [x] A-S5 建立失败不降级：构造必然失败的准备（如不存在 / 不可写的可写根）→ 后端返回 `SandboxError::SetupFailed` 且**未产生任何子进程**（用进程计数或桩断言）；`sandbox_setup_failed` 的 `error_kind` 映射归 I52 C5，不在本批
- [x] A-S7 准备幂等：见 C2.2 单测，此处补一条「残留 ACE 下重复执行命令不失败」
- [x] OQ1 定稿：机制与实测结论已写回规格 §5.1 / §10 OQ1 行（含 `0xC0000022` 否证结论与 Windows 无读边界）

**依赖**：C2.3

---

## 阶段 C3 — Unix 后端（与 C2 并行）

### C3.1 Linux（Landlock）

对应规格 §5.2。OQ10 已于 2026-11-15 定稿（形状 a 再收窄，见下），代码按定稿形状落地。

- [x] 先定 OQ10 形状：**已定稿（2026-11-15，WSL2 内核 ABI = 3 实测）取形状 (a) 并进一步收窄**——`/` 只授 `EXECUTE`，读权按 `PathBeneath` 逐条授 `/usr`、`/bin`、`/sbin`、`/lib*`、`/etc`、`/proc`、`/sys`、`/run`、`/var`、`/dev`，`$HOME` **整体不授读**（未列入白名单即默认不可读，`~/.ssh` 因此天然被拒，无需「先授读再拒读」）；实测 `/bin/sh` 可运行、`~/.ssh` 拒读、可写根外写失败三项同时成立。形状 (b) 弃用（它把 A-S2 降级为诚实性断言）
- [x] 新增 `assistant-tools/src/sandbox/linux.rs`：`probe()` 用 `landlock_create_ruleset(∅, 0, VERSION)` 读 ABI（`0` → `enforcement = None`），再试建一次 ruleset 验证形状可用；ABI ≥ 4 → `missing` 为空 → `Full`，ABI < 4 → `missing` 含 `Network` → `Partial`（`enforcement` 仍只走 `enforcement_from_missing`）
- [x] `spawn` 在 `Command::pre_exec` 内完成限制：ruleset 在父进程预先建好（`pre_exec` 只做系统调用），`restrict_self()` 内置 `PR_SET_NO_NEW_PRIVS`，并要求返回 `RulesetStatus::FullyEnforced`；**任一步失败即子进程非 0 退出**（std 固定 `_exit(127)`，口径差异已记入规格 §5.2），父进程不得在未限制形态下继续（F-S4）
- [x] `pre_exec` 内 `setsid`；`kill_tree()` 用 `kill(-pgid, SIGKILL)`，幂等（重复调用只发一次信号，`ESRCH` 忽略）
- [x] `pre_exec` 闭包内**禁止**堆分配与加锁：只允许预先构造好的 ruleset + `setsid` / `restrict_self` 两个系统调用
- [x] `wait()` / `stdout()` / `stderr()` 直接复用 `std::process::Child` 能力（Unix 用标准库，与 Windows 裸句柄路径不同）
- [x] 相交处理（fail-closed，规格 §5.2）：白名单目录罩住敏感目录 → 跳过该白名单项；`writable_root` 与敏感目录互相包含 → `SetupFailed("writable_root_overlaps_sensitive")`，不启动
- [ ] 单测（`#[cfg(test)]`，`linux.rs` 内）覆盖 A-S1 / A-S2 / A-S3：**已落地但未实机运行**（开发机为 Windows 且 WSL2 未装 Rust 工具链），标记「未验证」，待 CI / Linux 实机执行

**依赖**：C2.0

### C3.2 macOS（Seatbelt）

对应规格 §5.3。已落地（2026-11-15）：`probe()` 会试跑最小 profile，`spawn()` 生成 SBPL 并接管进程组；本机为 Windows，A-S1 / A-S2 / A-S3 单测**已落地但未实机运行**，标「未验证」。

- [x] 新增 `assistant-tools/src/sandbox/macos.rs`：`probe()` 检查 `/usr/bin/sandbox-exec` 存在且可执行，并用最小 profile 试跑 `/usr/bin/true`；失败 → `enforcement = None`
- [x] `program` 硬编码 `/usr/bin/sandbox-exec -p <profile> <program> <args…>`，**禁止**经 `PATH` 查找（防同名劫持）
- [x] profile：`(deny default)` + `(allow file-read*)` + `(allow file-write* (subpath "<writable_root>"))` + 必需的 `process-exec*` / `sysctl-read` / `mach-lookup` 最小集；敏感目录在 `file-read*` 之后**后置** `(deny file-read* (subpath "…"))`（Seatbelt 后置规则覆盖前置）
- [x] `(deny network*)`；`pre_exec` 内 `setsid`，`kill_tree()` 用 `kill(-pgid, SIGKILL)`
- [x] profile 拼串必须转义路径中的 `"` 与 `\`，**禁止**直接把未转义路径插进 `subpath` 表达式
- [ ] 单测（`#[cfg(target_os = "macos")]`）覆盖 A-S1 / A-S2 / A-S3：**已落地但未实机运行**，待 macOS 实机 / CI 执行

实现记录（与规格 §5.3 的对应关系，细节以规格为准）：最小放行集除规格列举项外还含 `process-fork` 与 `signal (target same-sandbox)`（shell 派生子进程与作业控制必需）；写白名单含 `/dev/null`、`/dev/zero`、`/dev/random`、`/dev/urandom`、`/dev/tty` 与 `/dev/fd` 子树；敏感目录与 `writable_root` 在生成 profile 前先 canonicalize（Seatbelt 在 vnode 层匹配真实路径，符号链接目录必须展开）；含控制字符的路径直接拒绝生成 profile。

**依赖**：C2.0

### C3.3 跨平台编译检查

- [x] `rustup target add x86_64-unknown-linux-gnu aarch64-apple-darwin` 后跑 `cargo check -p assistant-tools --target <t>`：**Linux 目标（`--all-targets`）零警告零错误**（2026-11-15）；顺带清掉 `os/` 下 4 条只在 Unix 目标暴露的既有警告（`fs_link/check.rs`、`fs_link/create.rs`、`fs_link/mod.rs`、`path.rs`）
- [x] macOS 目标（`--all-targets`）**零警告零错误**（2026-11-15）；顺带收口 3 处只在 macOS 目标暴露的既有缺陷：`os/agent_delete.rs` 的 `trash_guard_time` 与其 `std::time` 导入按 Windows / Linux 门收敛、`tests/agent_delete.rs` 的 `assert_no_trash_entry_for` 在 macOS 退化为空实现（`trash::os_limited` 在该平台被 cfg 关掉）、`SandboxChild::new` 曾随 C3.2 占位加平台门（C3.2 落地后已去掉）
- [x] Linux / macOS 目标 `cargo clippy --all-targets -- -D warnings` 零警告（2026-11-15）；顺带修掉 `os/path.rs` `reject_credential_dirs` 的 `unused_enumerate_index`（`Microsoft\Protect` 分支只在 Windows 成立，改为按平台拆成两次遍历）

**依赖**：C3.1、C3.2

---

## 阶段 C4 — 探测与注册收口

对应规格 §4.4。本批是「fail-closed 真正生效」的一步：探测结论必须决定工具是否存在。

- [x] `assistant-tools/src/sandbox/mod.rs` 增 `detect(host: &SandboxHostPaths) -> &'static SandboxProbe`，结论存 `OnceLock`（`ProbeCache`）；每平台候选列表按 §4.4（Windows `[restricted_token]` / Linux `[landlock]` / macOS `[seatbelt]` / 其它 `[none]`）；后端实例（已持有 host）的缓存与注入 `shell` 归 I52 C5，本批不落。C3 未落地前 Unix 候选列表为空（`Vec::new()`），由 `pick` 归一为 `None`（fail-closed），**不**预置未实现的后端
- [x] ~~`detect()` 首次调用时执行一次 `converge()`（仅 Windows 有产物），失败只 `log::warn!`~~ → **本条被规格 §6 取代**：注册期没有 `writable_root`（它按会话解析），Windows 唯一的持久产物（可写根 ACE）只能在 `spawn` 前的 `prepare` 里按 SID 收敛（`SET_ACCESS` 语义），因此注册期不存在可收敛对象，`detect()` 保持纯查询（§6「探测」时点：**禁止**任何写操作）。已由 `WindowsBackend::spawn` → `acl::prepare` 承接
- [x] 候选按强度降序，第一个 `enforcement != None` 者胜出（`pick`，胜出后不再探后续候选）；全败时保留首个候选的 `detail` 供诊断。P14 每平台一个候选，**禁止**预置未实现的降级候选（S1 才加）
- [x] `src-tauri/src/tool_setup.rs` 的 `init`：构造 `SandboxHostPaths`（`sandbox_host_paths`：§5.4 固定列表：`~/.ssh`、`~/.aws`、`~/.gnupg`、`~/.netrc` + `AppDirs` 解析出的配置 / 密钥 / 数据库目录 + 平台凭据目录；`retain(|p| p.exists())` 剔除不存在的项，**不创建**）→ 调 `sandbox::detect(&host)` → 结论为 `None` 时**不注册** `shell`（`shell_registration_allowed` 门禁 + `log::warn!`）
- [x] C5（I52）之前 `register_shell_tools` 尚不存在：本批只落「探测 + 结论 + 条件分支占位」，分支体留空并注明归 I52 的 C5；**未**注册任何空工具或临时 `shell`
- [x] 注册期只探测一次；运行期读缓存，**禁止**再调 `probe()`（A-S8，`sandbox::tests::cache_probes_only_once` 用计数桩断言调用次数 == 1）
- [x] 单测（注册侧）：`tool_setup::tests::registry_has_no_shell_when_probe_reports_none`（A-S4）+ `shell_registration_gate_follows_enforcement`（`partial` / `full` 仍注册）；**运行期**「注入 `None` → 执行前置拒绝且未产生子进程」随 `shell` 工具本体归 I52 C5（当前无 `shell` 工具，无可拒绝的执行点）
- [x] `assistant-tools` 侧不解析工作区：`SandboxHostPaths` 只含 `sensitive_read_deny`，`writable_root` 仍由 `ToolUseContext.workspace_root`（P12 的 `resolve_workspace` 唯一解析点）按会话提供、每次 `spawn` 现传；缺失按 F-S5 拒绝

**依赖**：C2、C3（可先接桩）

---

## 阶段 C5 — 门禁与收口

- [x] `cargo test -p assistant-tools`（Windows 全绿）；C3 后复跑：lib 212 passed，14 个集成套件 0 failed，0 警告
- [x] `cargo clippy -p assistant-tools -p assistant-app --all-targets -- -D warnings`（C4 改了 `tool_setup.rs`，故连应用 crate 一起跑）
- [x] `cargo fmt --check -p assistant-tools -p assistant-app`
- [x] `cargo tree -p assistant-tools` 复核无 tauri / 无 `app_dirs`（A-S6）
- [x] 逐条记录 A-S1–A-S8 的「平台 / 结论 / 日期」，未实跑的项标「未验证」，**禁止**为凑齐而打勾
- [x] 确认本批次零 DTO 变更：`git diff --stat` 不含 `src/types/protocol.contract.ts` 与 `src-tauri/src/protocol/`（无需 `npm run contract:gen`）
- [ ] 确认未改 I52 的 `trait.rs` / `gateway.rs` / `policy.rs`（C1 归属，避免与本清单并行冲突）

### 验收记录（2026-11-15）

| 编号 | 平台 | 结论 | 依据 |
|---|---|---|---|
| A-S1 | Windows | 通过 | `windows::tests::a_s1_write_boundary` |
| A-S2 | Windows | 通过（诚实性口径：断言 `missing` 含 `FsRead`；反向护栏 `read_side_is_not_confined_on_windows`） | `windows::tests::probe_reports_fs_read_missing` |
| A-S3 | Windows | 通过 | `windows::tests::a_s3_process_tree_reclaimed` |
| A-S4 | Windows | 通过（注册侧）；运行期「`none` → `sandbox_unavailable` 且无子进程」未验证（无 `shell` 工具可拒，归 I52 C5） | `tool_setup::tests::registry_has_no_shell_when_probe_reports_none`、`shell_registration_gate_follows_enforcement` |
| A-S5 | Windows | 通过 | `windows::tests::a_s5_setup_failure_does_not_spawn` |
| A-S6 | Windows | 通过（`cargo tree` 无 tauri / 无 `app_dirs`）；Linux 与 macOS 目标 `cargo check --all-targets` 均零警告零错误 | `cargo tree -p assistant-tools`、`cargo check -p assistant-tools --target {x86_64-unknown-linux-gnu, aarch64-apple-darwin}` |
| A-S7 | Windows | 通过 | `acl::tests::prepare_is_idempotent`、`acl::tests::prepare_replaces_stale_ace`、`windows::tests::a_s7_rerun_with_residual_ace` |
| A-S8 | Windows | 通过（计数桩断言探测 1 次、两次读到同一份结论） | `sandbox::tests::cache_probes_only_once`、`sandbox::tests::pick_takes_first_enforcing_candidate_and_stops` |
| A-S1 | Linux | **未验证**（开发机为 Windows，WSL2 未装 Rust 工具链）：用例已落地 | `linux::tests::a_s1_write_boundary` |
| A-S2 | Linux | **未验证**：用例已落地；规则形状已在 WSL2 用等价脚本实测通过（`~/.ssh` 拒读） | `linux::tests::a_s2_sensitive_read_deny`、`linux::tests::a_s2_sensitive_overlap_refuses_to_spawn` |
| A-S3 | Linux | **未验证**：用例已落地（按进程组 `/proc/<pid>` 断言） | `linux::tests::a_s3_process_tree_reclaimed` |
| A-S1 / A-S2 / A-S3 | macOS | **未验证且未实现**：C3.2 仅为占位，`probe()` 恒报不可用 → `shell` 不注册（fail-closed） | `macos.rs` 占位 |

**依赖**：C2.4、C3.3、C4

---

## 已知缺口（登记，不在本清单解决）

| 缺口 | 说明 | 归属 |
|---|---|---|
| Linux `fsRead` 形状 | OQ10 已定稿（形状 a 再收窄：`/` 只授 `EXECUTE`，读权按固定白名单逐条授，`$HOME` 不授读）；WSL2 等价脚本实测通过，实机单测仍待 CI / Linux 机执行 | C3.1（已闭合）/ 验收待跑 |
| Windows 无读边界 / 根外删除 / 控制台隔离 / 默认 DACL | 机制不可达，已定稿记入规格 §5.1 已知缺口与 `missing`（OQ1 已闭合）；不设「读敏感目录失败」验收项 | 规格 §5.1 / §8 |
| `sandbox-exec` 已 deprecated | 见 §10 OQ3；替换方案不在 P14 | S1 / 独立立项 |
| 非 Windows / Linux / macOS 平台 | `detect` 返回 `none` → 不注册 `shell`，不提供降级路径 | 规格 §4.4 已定 |
# I53 — P14-a：沙箱准备规格（后端抽象 / 链式探测 / 三平台实现）

| 项 | 值 |
|---|---|
| 状态 | 草稿（P14-a 实施规格；进度只在 [I38 roadmap](./I38-agent-roadmap.md) §2 维护） |
| 日期 | 2026-11-15 |
| 阶段 | P14-a（P14 的沙箱准备批次；见 [I38 roadmap](./I38-agent-roadmap.md) §2 / §3） |
| 关联 | [I52](./I52-shell-sandbox-spec.md)（`shell` 工具、审批链、输出存储；本文是其沙箱章节的**唯一来源**）、I18 design §3.17（已归档）（检测分层与 `Adjudicate` 门禁）、[沙箱调研](./agent-sandbox-research.md)（后端选型 ADR-1–ADR-4）、[沙箱参考分析](./agent-sandbox-reference-analysis.md)（Codex / OpenHuman / DSH 实现细节） |
| 影响范围 | `assistant-tools`（新增 `sandbox/` 模块）、`src-tauri/src/tool_setup.rs`（注册期探测收口）、`src-tauri/Cargo.toml`（Windows 特性与 `landlock` 依赖） |

> 一句话：把「审批放行之后最多能做到哪」变成系统强制的边界——抽象一个后端、探测一次、注册期收口、三平台各自实现，全程 fail-closed。

本文中的「必须 / 禁止」为可验收约束；「应当」为默认做法，偏离需在代码注释里写明理由。

**批次编号**：调研文档 [agent-sandbox-research](./agent-sandbox-research.md) 用 `P0 / P1 / P2` 指沙箱**实施批次**，与 roadmap 的阶段号 `P0–P14` 同形易混。本文一律写 **S0 / S1 / S2**，映射 P14-a / P14-b / P14-c（映射表见 [I52 §11](./I52-shell-sandbox-spec.md)）。

---

## 1 目标与非目标

### 1.1 现状（以代码为准）

| # | 事实 | 落点 |
|---|---|---|
| S-N1 | 无 `sandbox` 模块：`ToolTier` 只用于装配过滤与 tier 上限，**不产生任何运行时隔离**（同 [I52](./I52-shell-sandbox-spec.md) N2） | `assistant-tools/src/trait.rs`、`policy.rs` |
| S-N2 | `ToolUseContext.workspace_root` 是唯一权限边界，P12 已改为按会话解析（同 I52 N5） | `assistant-tools/src/context.rs`、`src-tauri/src/tool_setup.rs` |
| S-N3 | `windows` crate 已启 `Win32_Security`（`CreateRestrictedToken` 可用），未启 Job Objects / Threading | `src-tauri/Cargo.toml` |
| S-N4 | 无 `landlock` 依赖；`sandbox-exec` 走系统自带可执行文件 | `assistant-tools/Cargo.toml` |

### 1.2 目标

1. 抽象 `SandboxBackend`（探测 / 创建 / 进程树回收），把「边界」收敛到一个 trait；
2. 每平台一个候选后端，探测结论**进程内只算一次**；
3. 注册期收口：探测结论 `none` 即**不注册** `shell`；
4. `enforcement` 由**维度集合**推导，不由后端自报档位；
5. 准备与清理幂等，崩溃残留可收敛；
6. 全程 fail-closed：**探测失败不注册、建立失败不执行、限制失败不落地**。

### 1.3 非目标

- 不做 `shell` 工具本体：schema / 审批意图 / 执行流程 / 输出落盘与回读都在 [I52 §5–§6](./I52-shell-sandbox-spec.md)，本文只负责「把受限子进程交回给调用方」；
- 不参与审批决策（I52 §2.3）：沙箱不改变 Policy / Gateway 的结论；
- 不做容器 / microVM / 内核驱动级后端（虚拟化层 L3+，见 I52 §2.1），不做云端沙箱；
- 不做降级后端（Linux bwrap）与 Windows 双模式评估（S1）；不做高安全场景后端（S2，须独立立项）；
- 不做 Advisory 模式：不提供无隔离执行路径（[沙箱调研 ADR-4](./agent-sandbox-research.md)）；
- 不做命令字符串黑名单：字符串匹配不是边界（I52 §2.3）。

---

## 2 术语

两根轴（检测层 vs 虚拟化层）与三层职责（Policy / 审批 / 沙箱）的**唯一定义**在 [I52 §2.1 / §2.3](./I52-shell-sandbox-spec.md)，本文不复述。本文只补沙箱自身的封闭词汇（§3）。

---

## 3 封闭词汇（本文唯一来源）

| 词 | 定义 | 取值 |
|---|---|---|
| `SandboxKind` | 后端身份 | `windowsRestrictedToken` / `landlock` / `seatbelt` / `none` |
| `SandboxEnforcement` | 实际隔离强度（判据见 §4.5） | `full` / `partial` / `none` |
| `SandboxDimension` | 隔离维度 | `fsWrite` / `fsRead` / `processTree` / `network` |

`SandboxEnforcement` 同时是审批键的一部分（[I52 §6.2](./I52-shell-sandbox-spec.md)）：它进键的目的是「降级后不复用授权」，不是「有沙箱就免审」。

---

## 4 后端抽象

### 4.1 类型

```rust
pub enum SandboxKind { WindowsRestrictedToken, Landlock, Seatbelt, None }

pub enum SandboxEnforcement { Full, Partial, None }

pub enum SandboxDimension { FsWrite, FsRead, ProcessTree, Network }

/// 探测结论：进程内一次性计算并缓存（注册期算一次，运行期只读）。
pub struct SandboxProbe {
    pub kind: SandboxKind,
    pub enforcement: SandboxEnforcement,
    /// 未被系统强制的维度；`enforcement` 的判据由此推导，不由各后端自报。
    pub missing: Vec<SandboxDimension>,
    /// 固定串（后端名 / 系统 API 名 / 失败原因类别），**禁止**写入路径、用户名或命令内容。
    pub detail: Option<String>,
}

pub struct SandboxPlan {
    pub program: PathBuf,          // 绝对路径，不经 PATH 解析
    pub args: Vec<String>,
    pub cwd: PathBuf,              // 已 canonicalize，且落在 writable_root 内
    pub writable_root: PathBuf,    // 唯一可写根
    pub env: Vec<(String, String)>,
    pub network: bool,             // P14 恒 false
    pub stdin: Option<Vec<u8>>,    // P14 恒 None
}

pub enum SandboxError {
    SetupFailed(String),           // 建立边界失败（ACL / ruleset / profile 生成）
    SpawnFailed(String),           // 进程无法创建
    RestrictFailed(String),        // 子进程侧限制失败（Unix pre_exec 内）
}
```

### 4.2 trait

```rust
pub trait SandboxBackend: Send + Sync {
    fn kind(&self) -> SandboxKind;

    /// 纯查询：只读系统能力（版本 / 可用性 / ABI），不创建进程、不修改 ACL。
    fn probe(&self) -> SandboxProbe;

    /// 在边界内创建并接管一个进程。
    fn spawn(&self, plan: &SandboxPlan) -> Result<SandboxChild, SandboxError>;
}

impl SandboxChild {
    pub fn stdout(&mut self) -> Option<Box<dyn std::io::Read + Send>>;
    pub fn stderr(&mut self) -> Option<Box<dyn std::io::Read + Send>>;
    pub fn id(&self) -> u32;
    /// 同步阻塞到退出；调用方必须置于 `spawn_blocking`（I52 §5.7「执行器归属」）。
    /// 不声明为 `async`：Windows 侧进程由裸 `CreateProcess` 创建，无法转成 tokio `Child`。
    pub fn wait(&mut self) -> std::io::Result<ExitStatus>;
    /// 回收**整棵**进程树；可重复调用，幂等。
    pub fn kill_tree(&mut self);
}
```

**与调研草案的差异**（[沙箱调研 §10.3](./agent-sandbox-research.md) 的 `prepare / exec / cleanup` 三方法）：本规格改为单方法 `spawn` + `SandboxChild`，理由三条，调研草案不再作为实现依据：

1. `exec` 返回「已收集完的输出」会丢掉流式读取、进程级超时与中途 kill 的能力，而 [I52 §5.8](./I52-shell-sandbox-spec.md) 的取消与超时必须在「阻塞 `wait` 任务」与「deadline / 取消令牌」之间竞争；
2. Windows 后端必须「挂起创建 → 加入 Job → 恢复线程」，创建与限制是同一段时序，不能拆成 `prepare` + `exec`；
3. `cleanup` 在 Windows 上是 `Job` 句柄 drop（`KILL_ON_JOB_CLOSE`）、在 Unix 上是进程组回收，属于 `SandboxChild` 的生命周期，不是后端的状态。

`SandboxChild` 的 `stdout` / `stderr` 交回后由调用方接管（[I52 §5.7](./I52-shell-sandbox-spec.md) 的读取任务）；本文**不**定义输出读取、解码或落盘。

**读取器类型必须是 `Box<dyn Read + Send>`，不得写成 `std::process::ChildStdout` / `ChildStderr`**：Windows 后端不能用 `std::process::Command`——受限令牌要求 `CreateProcessAsUser` / `CreateProcessWithTokenW`，`CREATE_SUSPENDED` 后需 `ResumeThread` 而 `std::Child` 不暴露线程句柄，故只能裸调 `CreateProcess` + 自建 `CreatePipe`，产出的是裸 `HANDLE`；而 `ChildStdout` 没有公开构造器，无法从裸句柄造出来。`Box<dyn Read + Send>` 不丢任何能力：调用方本来就是「阻塞读 + `spawn_blocking`」，Windows 侧用 `File::from_raw_handle` 包一层、Unix 侧直接给 `ChildStdout` 即可。

### 4.3 `SandboxPlan` 的推导（唯一来源）

| 字段 | 来源 | 约束 |
|---|---|---|
| `writable_root` | `ToolUseContext.workspace_root`（P12 按会话解析的结果） | 必须已 canonicalize；`None` → 不执行（§7 F-S5） |
| `cwd` | 输入 `cwd`，相对路径按 `writable_root` 解析 | 必须通过 `os::path::ensure_within_root` |
| `program` | 平台固定 shell（[I52 §5.7](./I52-shell-sandbox-spec.md)） | 绝对路径；Windows 从 `%SystemRoot%\System32\cmd.exe` 取，Unix 固定 `/bin/sh` |
| `network` | 常量 `false` | 不读输入、不读配置；后端做不到就进 `missing` |
| `env` | [I52 §5.10](./I52-shell-sandbox-spec.md) 白名单 | 禁止透传敏感名 |

**禁止**由工具输入直接指定 `writable_root`、`program`、`network`。这三个字段一旦可被模型影响，「边界」就退化为建议。

**输出域不是可写根**：工具输出文件由父进程（沙箱外）写入（[I52 §5.11](./I52-shell-sandbox-spec.md)），子进程只经管道交出其 stdout / stderr，故 `SandboxPlan` 不新增输出目录字段，可写根恒为单一 `workspace_root`。模型在命令里把输出重定向到输出域同样会失败（写边界不认「这是自家目录」）。

### 4.4 链式探测与注册期收口

```
tool_setup::init
  └─ sandbox::detect(&host)       // 每平台一个候选列表，逐个 probe()
       ├─ Windows: [restricted_token]
       ├─ Linux:   [landlock]
       └─ macOS:   [seatbelt]
  └─ probe.enforcement >= Partial ? register(shell) : skip(shell)
```

**`detect` 的唯一入参是 `SandboxHostPaths`**（由 `tool_setup` 构造），只含 `sensitive_read_deny` = §5.4 的固定列表（`~/.ssh` 等凭据目录 + `AppDirs` 解析出的配置 / 密钥 / 数据库目录 + 平台凭据目录）。**必须注入、不得由 `assistant-tools` 自行解析**：它不依赖 tauri，拿不到 `AppDirs`（A-S6）。`writable_root` **不在入参内**——它按会话解析（§4.3），每次 `spawn` 现传，因此「同一进程内多个会话不同可写根」不需要重建后端。

列表为空视为配置缺失 → `probe` 直接返回 `enforcement = None`（fail-closed）：读拒绝列表为空等于「不拒读」，不得算作 `partial` / `full`。

规则：

1. **探测只在注册期做一次**，结论放进进程内 `OnceLock`；运行期不再探测（避免「同一会话前后两套边界」）。
2. 候选列表按强度降序；第一个 `enforcement != None` 的候选胜出。P14 每个平台只有一个候选，S1 才追加降级候选（Linux bwrap）。
3. `enforcement == None` → **不注册** `shell`，而不是注册后运行期拒绝。理由：模型看不到不存在的工具，比看到工具再报错少一轮无效迭代；`enabled_tools` 白名单里的 `shell` 名字自然落空（`resolve_available_tools` 是 deny-by-default）。
4. 注册期结论同时用于审批卡扩展（[I52 §6.3](./I52-shell-sandbox-spec.md)）与执行前置校验（运行期再读一次缓存，仍为 `None` 则拒绝执行）。

### 4.5 `enforcement` 判据

判据固定为**维度集合**，不由后端自报档位（防「自称 full」）：

| 档位 | 条件 |
|---|---|
| `full` | `missing` 为空：写边界 + 敏感读边界 + 进程树 + 网络四项全部系统强制 |
| `partial` | `fsWrite` 与 `processTree` 均被强制，但 `missing` 非空（至少 `fsRead` 或 `network` 缺失） |
| `none` | `fsWrite` 未被强制（即不存在写边界） |

各平台预期结论（验收按此对齐，实机为准）：

| 平台 | `fsWrite` | `fsRead` | `processTree` | `network` | 预期档位 |
|---|---|---|---|---|---|
| Windows（Token + ACL + Job） | 强制（ACL） | 强制（敏感目录 deny ACE） | 强制（Job） | 缺失 | `partial` |
| Linux（Landlock ABI ≥ 4） | 强制 | 强制 | 强制（进程组） | 强制（TCP） | `full` |
| Linux（Landlock ABI < 4） | 强制 | 强制 | 强制 | 缺失 | `partial` |
| macOS（Seatbelt） | 强制 | 强制 | 强制（进程组） | 强制 | `full` |

`partial` 不是「降级放行」：它只影响审批卡的展示文案与 P10-b 的放宽条件（P10-b 要求 `full`，见 [I52 §6.6](./I52-shell-sandbox-spec.md)），不改变是否注册工具（`partial` 仍注册）。

---

## 5 平台后端

### 5.1 Windows（S0 必做）

三个部件同时存在才成立，缺一即 `enforcement = none`（不注册工具）：

| 部件 | 作用 | 关键约束 |
|---|---|---|
| 受限令牌 | 去掉提权与宽泛组授权 | `CreateRestrictedToken`：`DISABLE_MAX_PRIVILEGE` + 宽泛组（Everyone / Users / Authenticated Users）置 deny-only；令牌只用于**本工具**创建的子进程 |
| 写边界 ACL | 让「可写根之外」真正失败 | 可写根与固定敏感目录的 ACL 由后端在 `spawn` 前幂等准备；**禁止**修改系统目录与用户主目录本身 |
| Job Object | 进程树回收 + 桌面隔离 | `KILL_ON_JOB_CLOSE`；UI 限制（剪贴板 / 桌面切换 / 全局原子 / 系统参数）全开；**不设** `ACTIVE_PROCESS` 上限（`cmd /C` 需要派生子进程） |

创建时序固定：`CREATE_SUSPENDED` → `AssignProcessToJobObject` → `ResumeThread`。禁止「先跑起来再挂 Job」：那段时间子进程可派生出 Job 外的孙进程。

写边界的具体机制（合成 SID 在可写根上授写 vs 在受保护路径上 deny 写）在 C2 原型阶段定稿，见 §10 OQ1；**验收口径不受机制选择影响**：可写根内写成功、根外写失败、敏感目录读失败。

### 5.2 Linux（S0 必做）

- 依赖 `landlock` crate；子进程侧 `PR_SET_NO_NEW_PRIVS` + 构建 ruleset；
- 规则形状：`/` 授 `READ_FILE | READ_DIR | EXECUTE`；`writable_root` 追加全量 `AccessFs`（含 `MAKE_*` / `REMOVE_*`）；固定敏感目录显式不给读（**该形状与 allow-list 语义冲突，定稿见 §10 OQ10**）；
- ABI ≥ 4 时处理 `AccessNet`（不授任何 TCP 规则 = 全拒），ABI < 4 时把 `network` 记入 `missing`；
- 限制在 `Command::pre_exec` 内完成：失败即 `_exit(126)`，**父进程不得**在未限制的形态下继续执行；
- 进程组：`pre_exec` 内 `setsid`，回收用 `kill(-pgid, SIGKILL)`。

### 5.3 macOS（S0 必做）

- `/usr/bin/sandbox-exec -p <profile> <program> <args…>`；路径**硬编码**（不查 `PATH`，防同名劫持）；
- profile：`(deny default)` + `(allow file-read*)` + `(allow file-write* (subpath "<writable_root>"))` + 必需的 `process-exec*` / `sysctl-read` / `mach-lookup` 最小集；敏感目录在 `file-read*` 之后单独 `(deny file-read* (subpath "…"))`；
- `(deny network*)`；进程组同 Linux；
- `sandbox-exec` 已被 Apple 标记为 deprecated 但仍可用；替换风险登记在 §10 OQ3。

### 5.4 固定敏感目录

沙箱层的读拒绝只针对一份**固定列表**（不读用户配置、不可被模型输入扩展）：

| 类别 | 内容 |
|---|---|
| 凭据 | `~/.ssh`、`~/.aws`、`~/.gnupg`、`~/.netrc` |
| 应用自身 | `AppDirs` 解析出的配置 / 密钥 / 数据库目录（本应用写入的所有目录） |
| 系统凭据 | 平台凭据存储目录（Windows 凭据文件目录、macOS `~/Library/Keychains`） |

与 `policy.rs` 的 `hard_deny_rules` **互不替代**：后者作用于路径意图（`read` / `edit` / …），而 `shell` 只有一条 `execute` 意图，路径级拒绝在 shell 路径上不生效——这正是沙箱层必须自带读拒绝的原因。

### 5.5 依赖与特性变更

| 变更 | 位置 | 说明 |
|---|---|---|
| `windows` 增 `Win32_System_JobObjects`、`Win32_System_Threading` | `src-tauri/Cargo.toml` 的 workspace 依赖特性 | `Win32_Security` 已启用，`CreateRestrictedToken` 可用 |
| 新增 `landlock`（Linux 目标） | `assistant-tools/Cargo.toml` 的 `[target.'cfg(target_os = "linux")'.dependencies]` | 仅 Linux 编译 |
| 不新增其他依赖 | — | macOS 走外部 `sandbox-exec`，Windows 走 `windows` crate |

依赖变更后必须验证：`cargo tree -p assistant-tools` 在三个平台的可编译性（CI 至少覆盖 Windows + Linux），且 `assistant-tools` 仍**不依赖 tauri**。

### 5.6 不做的事

- 不写内核驱动、不做系统级安装；
- 不修改 `%SystemDrive%` 根、`C:/Windows/**`、用户主目录本身的 ACL（只在其下的具体子目录上幂等加/减 ACE）；
- 不做「Advisory 模式」（[沙箱调研 ADR-4](./agent-sandbox-research.md)：不提供无隔离路径）。

---

## 6 准备与清理（生命周期与幂等）

「沙箱准备」的持久副作用**只在 Windows**；Linux / macOS 的准备产物随进程消失。这个差异决定了清理策略只能是「按平台各自收口」，不能做成统一的后端状态机。

| 平台 | 准备产物 | 持久副作用 | 清理方式 |
|---|---|---|---|
| Windows | 受限令牌 + 可写根 / 敏感目录 ACE + Job 对象 | **是**（ACE 落在文件系统 ACL 上） | Job 句柄 drop（`KILL_ON_JOB_CLOSE`）；ACE 在进程启动时收敛 |
| Linux | 子进程内 Landlock ruleset + `no_new_privs` | 否（进程内生效） | 无需清理 |
| macOS | Seatbelt profile 字符串 | 否（每次 `spawn` 现拼） | 无需清理 |

生命周期五个时点：

| 时点 | 动作 | 幂等要求 |
|---|---|---|
| 探测（注册期） | `probe()` 只读系统能力 | 纯查询，**禁止**任何写操作（§4.2） |
| 准备（`spawn` 前） | ACL 幂等准备 | 重复准备**不叠加** ACE（按 SID 查重，仅缺失时添加） |
| 限制（`spawn` 内） | 令牌 / Job / `pre_exec` | 失败即不落地（§7 F-S3 / F-S4） |
| 回收（`SandboxChild` 生命周期） | Job 句柄关闭 / `kill(-pgid, SIGKILL)` | `kill_tree` 可重复调用；`wait` 必须收尸 |
| 收敛（进程启动时） | 清理可写根与固定敏感子目录的残留 ACE | 尽力而为：失败只 `log::warn!`，不阻断工具注册 |

硬约束：

1. **准备失败不 spawn**：`SetupFailed` 直接上抛，调用方按 §7 F-S3 拒绝执行，**不得**退回「无边界先跑」；
2. **只碰可写根与固定敏感子目录**：系统目录、用户主目录本身、其它任意路径一律不动（§5.6）；
3. **幂等优先于性能**：准备阶段允许重复执行；不要为省一次 ACL 查询引入「是否已准备」的进程内布尔状态（跨重启即失效，且会让残留无法收敛）；
4. **崩溃残留不得阻断后续执行**：进程被杀后可能留下 ACE；下一次 `spawn` 的准备与启动收敛各自独立地收敛它，二者都不依赖「上次是否正常退出」；
5. **回收必须整棵树**：Windows 一次 `TerminateJobObject` 杀光 Job 内全部进程，Unix `kill(-pgid)`；只杀直接子进程会让孙进程继承管道写端，导致调用方 join 等不到 EOF（[I52 §5.7 第 9 步](./I52-shell-sandbox-spec.md)）。

---

## 7 失败矩阵（沙箱侧）

| 编号 | 场景 | 检测点 | 行为 | `error_kind` | 可否改写重试 |
|---|---|---|---|---|---|
| F-S1 | 后端探测 `none` | 注册期 | **不注册** `shell` | — | 工具不存在 |
| F-S2 | 运行期缓存结论为 `none` | 执行前置校验 | 拒绝执行 | `Business` + `sandbox_unavailable` | 否 |
| F-S3 | 边界建立失败（ACL / ruleset / profile） | `spawn` 前 | 拒绝执行，**不降级** | `Business` + `sandbox_setup_failed` | 否 |
| F-S4 | 子进程侧限制失败（Unix `pre_exec`） | 子进程 | `_exit(126)`，父进程收到该退出码，按「限制未生效」判失败 | `Business` + `spawn_failed` | 否 |
| F-S5 | 无 `writable_root`（ToolBox 入口 / 会话未绑定） | 输入校验 | 拒绝，不弹审批 | `Business` + `sandbox_root_missing` | 否（`blocked`） |
| F-S6 | 可执行程序不存在 / 进程创建失败 | `spawn` | 返回失败 | `Business` + `spawn_failed` | 是 |

三条 fail-closed 底线（对应 F-S1 / F-S3 / F-S4）：**探测失败不注册**、**建立失败不执行**、**限制失败不落地**。禁止任何「先跑起来再补边界」的中间态。

---

## 8 验收标准

| 编号 | 验收项 | 判据 |
|---|---|---|
| A-S1 | 写边界 | 可写根内 `write` 成功；根外（如用户主目录下的临时文件）写失败且 stderr 可读 |
| A-S2 | 敏感读边界 | 固定列表目录（如 `~/.ssh`）的读取失败 |
| A-S3 | 进程树回收 | 命令派生后台进程后超时 → 进程树全部消失（按进程组 / Job 断言） |
| A-S4 | 无沙箱不执行 | 强制 `probe` 返回 `none` → `shell` 未注册；构造运行期 `none` → `sandbox_unavailable` 且**未产生任何子进程** |
| A-S5 | 边界建立失败不降级 | 注入 `SetupFailed` → 返回 `sandbox_setup_failed`，无子进程 |
| A-S6 | 依赖边界 | `cargo tree -p assistant-tools` 无 tauri；Linux 目标编译通过 |
| A-S7 | 准备幂等 | 同一 `writable_root` 连续两次准备：ACE 不叠加；重复执行命令不因残留 ACL 失败 |
| A-S8 | 探测缓存 | 注册期探测一次；运行期 `execute` 不重复 `probe()`（用计数桩断言） |

沙箱侧验收只看**边界是否真的生效**，不看机制选择（§5.1 的两种 ACL 机制任选其一，A-S1 / A-S2 必须通过）。

---

## 9 分批实施

| 批次 | 内容 | 交付物 | 前置 |
|---|---|---|---|
| **C2 Windows 后端（S0）** | 受限令牌 + ACL 写边界 + Job Object；A-S1 / A-S2 / A-S3 在 Windows 上通过 | `sandbox/windows.rs`、原型结论（§10 OQ1 定稿） | I52 的 C1（审批键与求值顺序） |
| **C3 Unix 后端（S0）** | Landlock（含 ABI 探测与 `missing` 计算）+ Seatbelt profile；A-S1 / A-S2 / A-S3 在对应平台通过 | `sandbox/linux.rs`、`sandbox/macos.rs` | I52 的 C1 |
| **C4 探测与注册收口** | `detect()` + `OnceLock` 缓存 + `tool_setup` 条件注册；A-S4 / A-S7 / A-S8 | `sandbox/mod.rs`、`tool_setup.rs` | C2 / C3（可先接桩） |

C4 完成后，[I52](./I52-shell-sandbox-spec.md) 的 C5（`shell` 工具本体）才具备可执行前置。

S1 / S2（P14-b / P14-c，不在本文验收范围）：S1 追加降级后端（Linux bwrap）与 Windows 双模式评估；S2 为高安全场景的可选后端（Sandboxie / OpenShell / 容器 / microVM），须独立立项（[沙箱调研 §10.5](./agent-sandbox-research.md)）。

---

## 10 风险与开放问题

| 编号 | 问题 | 倾向与处置 |
|---|---|---|
| OQ1 | Windows 写边界机制：合成 SID 在可写根上授写 vs 在受保护路径上 deny 写 | C2 原型二选一，判据只看 A-S1 / A-S2 是否通过；不通过的机制直接放弃 |
| OQ3 | `sandbox-exec` 已 deprecated | 记录为平台替换风险；替换方案（自建 Seatbelt profile + `libsandbox`）不在 P14 |
| OQ5 | Linux Landlock ABI 分布（< 4 时网络不可拒） | 按 §4.5 记 `partial`；不为此引入 seccomp（S1） |
| OQ10 | Linux 读规则形状：Landlock 是 allow-list，§5.2 的「`/` 授 `READ_FILE`」与「固定敏感目录显式不给读」互斥（授 `/` 之后无法再对 `~/.ssh` 单独拒读） | C3.1 开工前二选一：(a) 收窄读白名单（`/` 只授 `EXECUTE`，读权按 `PathBeneath` 逐条授 `/usr`、`/etc`、`/proc`、`/dev`、`writable_root`、`$HOME` 非敏感子树）；(b) 保留 `/` 全授读并把 `fsRead` 记入 `missing`（落 `partial`）。判据只看 A-S2 是否通过 |
| R1 | ACL 修改残留（进程崩溃留下 ACE） | 只对可写根与固定敏感子目录操作、幂等、启动时收敛；不碰系统目录与主目录本身（§6） |
| R2 | 探测结论与实际不符（`probe` 通过但 `spawn` 时限制失败） | 两个时点各自判定：`probe` 决定是否注册，`spawn` 失败按 F-S3 / F-S4 拒绝执行；不允许「探测说行就跳过限制」 |
| R3 | 进程内缓存与系统能力变更（升级后 ABI 变化） | 缓存只在进程内有效，重启即重新探测；不做持久化缓存（避免旧结论跨版本沿用） |

---

## 11 参考

**本仓文档**

- [I52](./I52-shell-sandbox-spec.md)：`shell` 工具、审批链集成、输出存储与回读、整体验收与分批；
- I18 design §3.17（已归档）：检测分层、`Adjudicate` 门禁；
- [OSCALLDESIGN](../designed/OSCALLDESIGN.md)：Shell 洋葱模型（SafetyBelt / 结构守卫 / 环境隔离 / 命令分类）；
- [沙箱调研](./agent-sandbox-research.md)：后端选型（ADR-1 Windows Token+Job、ADR-2 Linux Landlock、ADR-3 不选 gVisor、ADR-4 fail-closed）；
- [沙箱参考分析](./agent-sandbox-reference-analysis.md)：Codex 四后端与违规检测、OpenHuman 诚实 Noop、DSH 链式探测与升级阶梯。

**参考实现**

| 项目 | 借鉴点 |
|---|---|
| Codex `windows-sandbox-rs` | Token + ACL + Job 三段式；挂起创建 → 挂 Job → 恢复的时序 |
| Codex `linux-sandbox` | Landlock self-restrict-then-exec；ABI 探测 |
| Codex / Claude Code | Seatbelt profile 生成与硬编码 `sandbox-exec` 路径 |
| DSH `sandbox-windows-acl` | Windows ACL 后端的幂等准备与清理 |
| OpenHuman | 后端不可用时诚实报告，不静默降级 |
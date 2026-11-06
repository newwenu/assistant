# 参考项目沙盒实现深度分析

> **本文档职责**：对 `references/` 中 3 个有沙盒实现的参考项目进行源码级深度分析，提取架构模式、关键数据结构、平台策略、安全边界及设计取舍，为本项目沙盒实现提供可直接对照的工程参考。
> **关联文档**：[agent-sandbox-research.md](./agent-sandbox-research.md)（开源项目调研与演进路线） · [I18 架构决策](./I18-agent-tool-architecture-design.md) · [I18 实施规格](./I18-agent-tool-architecture-spec.md)
> **参考项目**：Codex（OpenAI Codex CLI）· OpenHuman · DeepSeek Harness Orb（DSH）
> **补充样本**：§5.4 另引 goose 与 OpenBitFun 作为「无沙箱」对照组（goose 无任何沙箱原语；OpenBitFun 无沙箱但有边界层路径检查），用于说明「路径检查」与「操作范围检测」的区别。

---

## 1. 项目概览与定位差异

| 维度 | Codex | OpenHuman | DSH |
|------|-------|-----------|-----|
| **语言** | Rust（全量） | Rust（核心）+ tinybox（submodule） | TypeScript（全量） |
| **定位** | CLI Agent 工具 | 多模态 Agent 平台 | Agent Harness 框架 |
| **沙盒范围** | shell 命令执行 | shell + 工具执行 | shell + fs + 所有工具族 |
| **后端数** | 4 | 3 | 4 |
| **Docker** | 无 | 有 | 无 |
| **升级机制** | 无 | ElevatedOp 声明式 | 有，严格宽化阶梯 |
| **enforcement 报告** | 无 | 有，Inactive/Ready/Busy/Error | 有，full/partial |
| **完整性检查** | 有 | 无 | 无 |
| **违规检测** | 有，结构化 | 无 | 有，RunnerFailureRule |
| **策略注入 prompt** | 有，模板文件 | 无 | 有，自然语言渲染 |
| **Noop 诚实** | 无 | 有，Inactive 不等于 Ready | 无 |

---

## 2. Codex — 四后端 + 深度防御

### 2.1 架构总览

```
codex-rs/
├── protocol/src/
│   ├── sandbox.rs          ← SandboxType 枚举 + SandboxOverride
│   ├── permissions.rs      ← FileSystemAccessMode/Policy, NetworkSandboxPolicy
│   └── protocol.rs         ← SandboxPolicy, WritableRoot
├── sandboxing/src/
│   ├── lib.rs              ← 模块入口，条件编译平台后端
│   ├── manager.rs          ← SandboxManager：平台选择 + 策略转换 + 命令变换
│   ├── bwrap.rs            ← Linux bubblewrap 命令构建
│   ├── seatbelt.rs         ← macOS Seatbelt profile 生成
│   ├── seatbelt_base_policy.sbpl     ← 基础 SBPL 策略
│   ├── seatbelt_network_policy.sbpl  ← 网络 SBPL 策略
│   ├── landlock.rs         ← 遗留 Landlock 实现
│   ├── violation.rs        ← 违规检测与记录
│   ├── denial.rs           ← 拒绝推断
│   ├── spawn.rs            ← 进程 spawn
│   ├── sandbox_integrity/  ← 二进制完整性检查
│   └── windows.rs          ← Windows 受限令牌策略
├── linux-sandbox/src/
│   ├── seccomp.rs          ← seccomp-bpf 过滤器安装
│   └── linux_run_main.rs   ← Linux 沙盒执行主流程
├── windows-sandbox-rs/     ← Windows 受限令牌 + Job Object 实现
└── mxc-sandbox/            ← Windows MXC 容器后端
```

### 2.2 核心数据结构

#### SandboxType — 后端枚举

来源：`protocol/src/sandbox.rs`

```rust
pub enum SandboxType {
    None,                      // 无沙盒
    MacosSeatbelt,             // macOS: sandbox-exec
    LinuxSeccomp,              // Linux: bwrap + seccomp
    WindowsRestrictedToken,    // Windows: 受限令牌 + ACL
    WindowsMxc,                // Windows: MXC 容器
}
```

#### SandboxOverride — 控制器覆盖

```rust
pub enum SandboxOverride {
    NoOverride,                              // 正常选择
    EscalatedSandboxWithRestrictions,       // 扩大文件系统访问但保留 denied-read
    BypassSandboxFirstAttempt,              // 首次尝试绕过沙盒
}
```

#### FileSystemAccessMode — 文件系统访问模式

来源：`protocol/src/permissions.rs`

```rust
pub enum FileSystemAccessMode {
    Read,   // 只读
    Write,  // 可写
    Deny,   // 拒绝（legacy alias "none"）
}
// 冲突优先级：Deny > Write > Read
```

#### NetworkSandboxPolicy — 网络策略

```rust
pub enum NetworkSandboxPolicy {
    Restricted,  // 受限（默认）
    Enabled,     // 允许
}
```

#### 受保护元数据路径

```rust
const PROTECTED_METADATA_PATH_NAMES: &[&str] = &[".git", ".agents", ".codex", ".aws"];
```

在可写根下，这些目录被重新挂载为只读，防止 Agent 意外修改版本控制或凭证数据。

### 2.3 平台选择逻辑

来源：`sandboxing/src/manager.rs`

```rust
pub fn get_platform_sandbox(windows_sandbox_enabled: bool) -> Option<SandboxType> {
    if cfg!(target_os = "macos") {
        Some(SandboxType::MacosSeatbelt)
    } else if cfg!(target_os = "linux") {
        Some(SandboxType::LinuxSeccomp)
    } else if cfg!(target_os = "windows") {
        if windows_sandbox_enabled {
            Some(SandboxType::WindowsRestrictedToken)
        } else {
            None  // Windows 默认不启用
        }
    } else {
        None
    }
}
```

**设计要点**：
- Windows 需显式启用（`windows_sandbox_enabled`），macOS/Linux 默认启用
- MXC 后端通过 `effective_windows_sandbox_type()` 函数处理：显式选择 MXC 时保留，否则按 `WindowsSandboxLevel` 决定

### 2.4 Linux 实现：bwrap + seccomp 双层

#### bwrap 命令构建（文件系统隔离）

来源：`sandboxing/src/bwrap.rs`

关键策略：
1. **默认只读**：`--ro-bind / /` 将整个根文件系统挂载为只读
2. **可写根目录**：`--bind <root> <root>` 按需开放写权限
3. **保护子路径**：可写根下的 `.git`/`.codex`/`.aws` 重新 `--ro-bind` 为只读
4. **Deny glob**：通过 `rg` 或内部 globset walker 展开匹配文件，在 bwrap 中屏蔽
5. **命名空间**：`--unshare-user`（用户）+ `--unshare-pid`（PID，默认）+ `--unshare-net`（网络受限时）
6. **WSL 处理**：WSL1 不支持（无法创建用户命名空间），WSL2 走正常路径

bwrap 探测：
- 系统路径搜索 `find_system_bwrap_in_path()`
- 用户命名空间可用性探测 `system_bwrap_has_user_namespace_access()`
- WSL1 检测 `is_wsl1()`
- 超时探测（500ms）防止挂起

#### seccomp 过滤器（系统调用层）

来源：`linux-sandbox/src/seccomp.rs`

```rust
pub(crate) fn apply_permission_profile_to_current_thread(
    permission_profile: &PermissionProfile,
    cwd: &Path,
    apply_landlock_fs: bool,
    managed_network: Option<&ManagedNetworkSandboxContext>,
    proxy_routing_active: bool,
) -> Result<()>
```

分层策略：
1. **`PR_SET_NO_NEW_PRIVS`**：仅在需要 seccomp 或 Landlock 时设置（避免影响 bwrap setuid）
2. **网络 seccomp**：`NetworkSeccompMode` 控制网络系统调用过滤
3. **VM Socket 限制**：阻塞 `AF_VSOCK`（WSL2 可通过此通道启动 Windows 进程）
4. **io_uring 阻塞**：所有模式都阻塞 io_uring 系统调用（防止绕过 socket-family 限制）
5. **代理模式**：允许 IP socket 但阻塞独立 `AF_UNIX` socket

### 2.5 macOS 实现：Seatbelt

来源：`sandboxing/src/seatbelt.rs`

#### 策略文件

`seatbelt_base_policy.sbpl`：

```scheme
(version 1)
(deny default)                          ; 默认拒绝所有
(allow process-exec)                    ; 允许执行子进程
(allow process-fork)                    ; 允许 fork
(allow signal (target same-sandbox))    ; 同沙盒内信号
(allow process-info* (target same-sandbox))
(allow file-write-data                  ; 仅允许写 /dev/null
  (require-all
    (path "/dev/null")
    (vnode-type CHARACTER-DEVICE)))
(allow sysctl-read ...)                ; 精细控制 sysctl 读取
```

**安全措施**：
- 仅使用 `/usr/bin/sandbox-exec`（防 PATH 注入，硬编码路径 `MACOS_PATH_TO_SEATBELT_EXECUTABLE`）
- 支持 daemon socket 沙盒
- 支持 TLS 信任代理（`com.apple.TrustEvaluationAgent` mach-lookup）
- 网络策略通过独立 SBPL 文件 `seatbelt_network_policy.sbpl` 控制
- 偏好设置策略通过 `seatbelt_preferences_policy.sbpl` 控制

### 2.6 Windows 实现：双后端

#### 受限令牌后端（windows-sandbox-rs）

- `CreateRestrictedToken()` 创建受限令牌
- 合成 SID ACL：只有携带特定 synthetic SID 的沙箱进程才能写入指定目录
- Job Object 限制进程创建/剪贴板/窗口站访问
- 双模式：Advisory（unelevated）→ Mandatory（elevated）
- 环境变量白名单：`USERNAME`, `USERPROFILE`, `SYSTEMROOT`

#### MXC 后端（mxc-sandbox）

- 使用微软 MXC `BaseContainerRunner`
- 不修改宿主 ACL、不创建沙盒用户、不请求提升
- 仅允许 IPv4/IPv6 loopback，拒绝非 loopback 出站
- 限制 Win32k：剪贴板、输入注入、桌面/系统控制受限
- 通过 `windows.sandbox = "mxc"` 严格选择，或 `features.prefer_mxc` 自动选择

### 2.7 违规检测

来源：`sandboxing/src/violation.rs`

```rust
pub enum SandboxViolationEvent {
    FileSystem(FileSystemSandboxViolation),
    Network(NetworkSandboxViolation),
}

pub enum SandboxViolationBackend {
    LinuxSandbox,
    ManagedNetworkProxy,
    Seatbelt,
    WindowsSandbox,
    WindowsMxc,
}

pub enum FileSystemSandboxViolationReason {
    OperationNotPermitted,    // EPERM
    PermissionDenied,         // EACCES
    ReadOnlyFileSystem,       // EROFS
    PolicyDenied,             // seccomp/landlock/seatbelt
    FailedToWriteFile,
}
```

关键词匹配表：

```rust
const SANDBOX_DENIED_KEYWORDS: [(FileSystemSandboxViolationReason, &str); 7] = [
    (OperationNotPermitted, "operation not permitted"),
    (PermissionDenied, "permission denied"),
    (ReadOnlyFileSystem, "read-only file system"),
    (PolicyDenied, "seccomp"),
    (PolicyDenied, "sandbox"),
    (PolicyDenied, "landlock"),
    (FailedToWriteFile, "failed to write file"),
];
```

#### 拒绝推断（denial.rs）

`EPERM`/`EACCES` 关键词匹配 + SIGSYS 信号检测（seccomp 拒绝信号，exit code = 128 + SIGSYS）+ 快速拒绝退出码排除（2/126/127 是 shell 错误而非沙盒拒绝）。

### 2.8 完整性检查

`sandbox_integrity/` 模块在沙盒启动前检查二进制文件是否被篡改：
- `FileContentsChecker` — 校验关键二进制内容
- `IntegrityFinding` / `IntegrityFindingDetails` — 篡改发现报告
- 覆盖 Seatbelt、bubblewrap、MXC、Landlock 各后端

### 2.9 用户侧模式

TypeScript SDK 暴露三级模式，对应 prompt 模板：

| 模式 | 文件系统 | 网络 | Prompt 模板 |
|------|---------|------|------------|
| `read-only` | 只读 | 受限 | `prompts/templates/permissions/sandbox_mode/read_only.md` |
| `workspace-write` | cwd + writable_roots 可写 | 受限 | `prompts/templates/permissions/sandbox_mode/workspace_write.md` |
| `danger-full-access` | 无限制 | 取决于配置 | `prompts/templates/permissions/sandbox_mode/danger_full_access.md` |

### 2.10 Codex 设计模式总结

| 模式 | 说明 |
|------|------|
| **编译时平台选择** | `cfg!(target_os)` 条件编译，非运行时分支 |
| **默认只读** | 整个根文件系统 `--ro-bind / /`，按需开放写 |
| **元数据保护** | 可写根下 `.git`/`.codex`/`.aws` 强制只读 |
| **双层隔离** | bwrap（文件系统视图）+ seccomp（系统调用层） |
| **完整性先验** | 启动前检查沙盒二进制完整性 |
| **违规后验** | 执行后检测拒绝关键词 + SIGSYS 信号 |
| **硬编码路径** | Seatbelt 仅用 `/usr/bin/sandbox-exec`，防 PATH 注入 |

---

## 3. OpenHuman — 三后端 + Docker + 诚实 Noop

### 3.1 架构总览

```
openhuman-core/src/sandbox/
├── mod.rs      ← 模块入口，re-export tinybox_jail as cwd_jail
├── types.rs    ← SandboxBackendKind, SandboxPolicy, SandboxExecRequest/Result
├── ops.rs      ← 策略解析 + 后端创建 + 路由执行
├── docker.rs   ← Docker 一次性容器后端
└── schemas.rs  ← 控制器 schema 注册

vendor/tinybox/crates/
├── tinybox-jail/    ← OS 级进程 jail（Landlock/Seatbelt/AppContainer/Noop）
└── tinybox-docker/  ← Docker CLI 封装
```

**关键设计**：`cwd_jail`（OS 级 jail）委托给 `tinybox-jail`（git submodule），OpenHuman 自身只负责**何时隔离**的策略决策。

### 3.2 核心数据结构

#### SandboxBackendKind — 后端枚举

```rust
pub enum SandboxBackendKind {
    None,    // 无沙盒
    Local,   // OS 级进程 jail（Landlock/Seatbelt/AppContainer）
    Docker,  // Docker 容器隔离
}
```

#### SandboxPolicy — 策略结构

```rust
pub struct SandboxPolicy {
    pub backend: SandboxBackendKind,
    pub workspace_root: PathBuf,
    pub read_only_mounts: Vec<PathBuf>,
    pub allow_network: bool,
    pub env_passthrough: Vec<String>,
    pub docker_overrides: Option<DockerOverrides>,
}
```

#### DockerOverrides — Docker 特定覆盖

```rust
pub struct DockerOverrides {
    pub image: Option<String>,
    pub network: Option<String>,
    pub memory_limit_mb: Option<u64>,
    pub cpu_limit: Option<f64>,
    pub read_only_rootfs: Option<bool>,
    pub extra_caps_drop: Vec<String>,
}
```

#### SandboxStatus — 生命周期状态

```rust
pub enum SandboxStatus {
    Inactive,  // 未初始化
    Ready,     // 就绪
    Busy,      // 执行中
    Error,     // 需重启/清理
}
```

#### ElevatedOp — 提升操作

```rust
pub const ELEVATED_TOOLS: &[&str] = &[
    "git_operations",
    "install_tool",
    "docker_management",
    "process_management",
];
```

### 3.3 策略解析

```rust
pub fn resolve_sandbox_policy(
    mode: SandboxMode,
    action_dir: &Path,
    runtime_config: &RuntimeConfig,
    is_remote_session: bool,
) -> SandboxPolicy {
    let backend = match mode {
        SandboxMode::None => SandboxBackendKind::None,
        SandboxMode::ReadOnly => SandboxBackendKind::None,  // 只读由工具层自行限制
        SandboxMode::Sandboxed => {
            if runtime_config.kind == "docker" || is_remote_session {
                SandboxBackendKind::Docker
            } else {
                SandboxBackendKind::Local
            }
        }
    };
}
```

**设计要点**：
- `SandboxMode::ReadOnly` 映射到 `None`——只读限制由工具层（如 fs 工具）自行实现，不走 OS 级沙盒
- 远程会话默认走 Docker（更强隔离），本地交互走 Local（OS 级 jail）
- 网络策略：`Sandboxed` 模式下远程会话禁止网络，本地允许

### 3.4 Noop 诚实报告

```rust
pub(crate) fn local_status_for_backend(backend_name: &str) -> SandboxStatus {
    if backend_name == cwd_jail::NOOP_BACKEND_NAME {
        SandboxStatus::Inactive  // Noop 不是 Ready——诚实报告"未初始化"
    } else {
        SandboxStatus::Ready
    }
}
```

**这是 OpenHuman 独有的关键设计**：当 OS 不支持任何 jail（如无 Landlock 的旧 Linux、无 Seatbelt 的非 macOS），`tinybox-jail` 降级为 `NoopBackend`。此时 `SandboxBackendHandle.status` 报告 `Inactive` 而非 `Ready`，**防止调用方误以为命令已隔离**。

### 3.5 执行路由

```rust
pub async fn execute_in_sandbox(
    policy: &SandboxPolicy,
    command: &str,
    working_dir: &Path,
    extra_env: HashMap<OsString, OsString>,
    timeout: Duration,
) -> anyhow::Result<SandboxExecResult> {
    match policy.backend {
        SandboxBackendKind::None => execute_unsandboxed(...).await,
        SandboxBackendKind::Local => execute_local_jail(...).await,
        SandboxBackendKind::Docker => docker::docker_exec(...).await,
    }
}
```

**CWD 验证差异**：None/Local 验证 `working_dir`（宿主路径），Docker 验证 `policy.workspace_root`（宿主侧挂载源，容器内路径如 `/workspace` 不应在宿主 stat）。

### 3.6 Local Jail 执行

```rust
async fn execute_local_jail(...) -> anyhow::Result<SandboxExecResult> {
    let mut jail = Jail::new(&policy.workspace_root, "sandbox.agent");
    if !policy.allow_network {
        jail = jail.deny_net();
    }
    for ro in &policy.read_only_mounts {
        jail = jail.add_read_only(ro);
    }
    // 输出捕获：Seatbelt 不转发 piped stdio，用临时文件中转
    let stdout_file = policy.workspace_root.join(".sandbox_stdout");
    let stderr_file = policy.workspace_root.join(".sandbox_stderr");
    let wrapped = platform_shell::wrap_with_output_redirection(command, &stdout_file, &stderr_file);
    // ...
    let os_backend = cwd_jail::default_backend();
    let spawn_result = if os_backend.is_available() {
        cwd_jail::spawn(&jail, cmd)
    } else {
        cwd_jail::spawn_with(&NoopBackend, &jail, cmd)  // 降级到 Noop
    };
}
```

**输出捕获技巧**：macOS Seatbelt 会重建命令内部，不转发 piped stdio 设置。解决方案：在 jail 内将 stdout/stderr 重定向到临时文件，执行后读回。

### 3.7 Docker 后端

来源：`sandbox/docker.rs`

**Docker 沙盒特性**：
- 一次性容器（`docker run --rm`）— 自动清理
- 工作区挂载到 `/workspace`
- 默认网络 `none`（完全网络隔离）
- `--cap-drop ALL` + 可选额外 cap drop
- 可选只读 rootfs
- 资源限制（memory, CPU）
- 环境变量白名单透传
- 容器标签 `openhuman.sandbox=true` 用于孤立容器清理

### 3.8 环境变量透传

```rust
pub const SANDBOX_ENV_PASSTHROUGH: &[&str] = &[
    "PATH", "HOME", "TERM", "LANG", "LC_ALL", "LC_CTYPE", "USER", "SHELL", "TMPDIR",
];
```

### 3.9 OpenHuman 设计模式总结

| 模式 | 说明 |
|------|------|
| **策略-执行分离** | `resolve_sandbox_policy()` 决策，`execute_in_sandbox()` 执行 |
| **ReadOnly 工具层实现** | OS 级沙盒只做 `Sandboxed`，`ReadOnly` 由工具层自行限制 |
| **Noop 诚实** | `Inactive` 不等于 `Ready`，防止调用方误以为已隔离 |
| **Docker 可选** | 远程会话默认 Docker，本地默认 Local |
| **ElevatedOp 声明式** | git/install/docker/process 必须在宿主执行 |
| **输出文件中转** | Seatbelt 不转发 piped stdio，用临时文件捕获 |

---

## 4. DSH — 四包架构 + 链式探测 + 升级阶梯

### 4.1 架构总览

```
packages/sandbox/
├── sandbox/                  ← 核心 seam 定义
│   └── src/
│       ├── index.ts          ← SandboxMode, SandboxExecutionPolicy, SandboxPolicy, ConfinedArgv
│       ├── escalation.ts     ← 升级词汇 + 严格宽化阶梯
│       ├── roots.ts          ← 可写根解析
│       └── diagnostics.ts    ← 诊断
├── sandbox-policy/           ← 策略解析服务
│   └── src/
│       ├── index.ts          ← SandboxPolicyService, 策略渲染
│       ├── session-invariant.ts  ← 不变量断言
│       └── session-mode.ts   ← per-session 模式覆盖
├── sandbox-local/            ← 本地后端
│   └── src/
│       ├── index.ts          ← 链式探测 + 平台选择 + confine()
│       └── profiles.ts       ← bwrap/Landlock/Seatbelt profile 构建
└── sandbox-windows-acl/      ← Windows ACL 后端
    └── src/
        ├── index.ts          ← AclSandbox, AclSandboxOptions
        ├── token.ts          ← createRestrictedToken, findLogonSid
        ├── acl.ts            ← grantWrite, revokeWrite, buildExplicitAccess
        ├── spawn.ts          ← spawnSandboxed
        ├── grant.ts          ← AclWriteGrant
        ├── path-boundary.ts  ← 路径边界断言
        ├── ffi.ts            ← Win32 FFI 绑定
        ├── win32-abi.ts      ← Win32 常量
        └── workspace-sid.ts  ← 双 SID 派生
```

### 4.2 核心 Seam（sandbox）

#### SandboxMode — 三级模式

```typescript
export type SandboxMode = 'read-only' | 'workspace-write' | 'danger-full-access'
export type ConfinedSandboxMode = Exclude<SandboxMode, 'danger-full-access'>
```

#### SandboxExecutionPolicy — 执行策略

```typescript
export interface SandboxExecutionPolicy {
  mode: SandboxMode
  workspaceRoot: string
  sessionId?: SessionId
}
```

#### SandboxPolicy — 限制策略（仅 Confined 模式）

```typescript
export interface SandboxPolicy extends SandboxExecutionPolicy {
  mode: ConfinedSandboxMode  // 非 danger 模式才需要策略
}
```

#### SandboxEnforcement — 执行完整度

```typescript
export type SandboxEnforcement = 'full' | 'partial'
```

- `full`：后端完全执行策略承诺的所有文件效果
- `partial`：后端或旧内核 ABI 无法完全执行，调用方需知

#### ConfinedArgv — 包装后的命令

```typescript
export interface ConfinedArgv {
  argv: string[]
  enforcement: SandboxEnforcement
  runnerFailureRules: readonly RunnerFailureRule[]
}
```

#### RunnerFailureRule — Runner 失败识别

```typescript
export interface RunnerFailureRule {
  allowedExitCodes?: readonly number[]
  fatalSignatures: readonly string[]
  informationalLines?: readonly string[]
}
```

**设计要点**：沙盒 runner 自身可能失败（非子命令失败），需要精确识别。退出码 + stderr 关键词 + 信息行排除三重判定。

### 4.3 升级词汇（escalation.ts）

#### 严格宽化阶梯

```typescript
export const WIDER_MODES: Record<string, readonly SandboxMode[]> = {
  'read-only': ['workspace-write', 'danger-full-access'],
  'workspace-write': ['danger-full-access'],
}
```

**严格宽化**：只能向更宽松的方向升级，不能降级。`read-only` 可升级到 `workspace-write` 或 `danger-full-access`，`workspace-write` 只能升级到 `danger-full-access`。

#### 升级目标词汇

```typescript
export const ESCALATION_TARGETS: readonly SandboxMode[] = ['workspace-write', 'danger-full-access']
```

#### 参数验证

```typescript
export function validateEscalationArgs(
  sandboxPermissions: string | undefined,
  justification: string | undefined,
): void {
  // sandbox_permissions + justification 必须成对出现
  // justification 必须非空
}
```

#### 拒绝标记

```typescript
export function sandboxDenialMarker(mode: SandboxMode): string {
  return `[sandbox: file access denied under ${mode} mode]`
}
```

**设计要点**：两个工具族（bash 和 fs）共享相同的拒绝词汇，模型无论遇到哪种工具的拒绝，都能识别为同一策略拒绝。

### 4.4 策略服务（sandbox-policy）

#### 默认 fail-safe

```typescript
export interface Config {
  mode?: SandboxMode      // 默认: 'read-only'
  workspaceRoot?: string  // 默认: process.cwd()
}
```

**`read-only` 是 fail-safe 默认**：部署若需 workspace-writable，必须显式 opt-in。

#### 策略渲染

策略以**自然语言**注入 system prompt，模型在每次请求前看到当前策略，指导其行为：

- `read-only`：告知模型当前只读，写操作会被拒绝
- `workspace-write`：告知模型可在 workspace root 下写入，其他位置只读
- `danger-full-access`：告知模型无文件系统限制

#### Per-Session 模式覆盖

```typescript
export function setSandboxMode(session: Session, mode: SandboxMode): void {
  session.append('sandbox/mode', { mode })
}
```

- 模式覆盖是**会话事件**（log-only，非 surface event）
- `effective = projection state ?? deployment default`
- 覆盖存活重启（通过 replay）
- 两个会话互不可见对方的状态
- `source: 'delegation'` 标记子会话继承的覆盖

### 4.5 本地后端（sandbox-local）

#### 链式探测

```typescript
type SelectedRunner = {
  runner: 'bwrap' | 'landlock' | 'seatbelt' | 'windows-acl'
  enforcement: SandboxEnforcement
}
```

| 平台 | 候选链 | 探测顺序 |
|------|--------|---------|
| Linux | bwrap → Landlock | 先 bwrap（namespace 虚拟化），bwrap 不可用则 Landlock（LSM） |
| macOS | Seatbelt | 单候选 |
| Windows | windows-acl | 单候选 |

**功能探测**：每个 rung 真正执行一次（如 `bwrap ... -- true`），退出 0 表示可用。探测结果**缓存**，后续调用直接使用。

#### Profile 构建（profiles.ts）

##### bwrap

```typescript
export function bwrapProfileArgs(policy: SandboxPolicy): string[] {
  const args = ['--ro-bind', '/', '/', '--dev', '/dev', '--unshare-pid',
                '--proc', '/proc', '--die-with-parent']
  if (policy.mode === 'workspace-write') {
    args.push('--tmpfs', '/tmp')
    args.push('--bind', policy.workspaceRoot, policy.workspaceRoot)
  }
  return args
}
```

##### Landlock

```typescript
export function landlockProfileArgs(policy: SandboxPolicy): string[] {
  const readWrite = ['/dev/null']
  if (policy.mode === 'workspace-write') {
    readWrite.push('/tmp', policy.workspaceRoot)
  }
  return landlockGrantArgs({ readOnly: ['/'], readWrite })
}
```

##### Seatbelt

```typescript
export function seatbeltProfileArgs(policy: SandboxPolicy): string[] {
  const forms = [
    '(version 1)', '(allow default)', '(deny file-write*)',
    `(allow file-write* (literal ${sbplString('/dev/null')}))`
  ]
  const roots = writableRoots(policy)
  if (roots.length > 0) {
    forms.push(`(allow file-write* ${roots.map(r => `(subpath ${sbplString(r)})`).join(' ')})`)
  }
  return ['-p', forms.join(' ')]
}
```

**Seatbelt 策略生成**：`(deny file-write*)` 默认拒绝写，然后逐个允许可写根的子路径。

### 4.6 Windows ACL 后端（sandbox-windows-acl）

#### WRITE_RESTRICTED Token 机制

执行流程：

1. `OpenProcessToken` → 获取当前进程令牌
2. `findLogonSid` → 提取 logon session SID (S-1-5-5-x-y)
3. `CreateRestrictedToken` → 创建受限令牌（移除特权 SID、添加 deny-only SID）
4. `restrictTokenIntegrity` → 降级到 Low 完整性级别
5. `setTokenDefaultDaclGrant` → 设置默认 DACL
6. `spawnSandboxed` → 在受限令牌下 spawn 子进程

#### 双 SID 设计

| SID | 生成方式 | 跨会话复用 | 撤销 |
|-----|---------|-----------|------|
| workspace SID | 确定性（canonical path 哈希） | 是，ACE 一次创建永久有效，后续 provision O(1) | 否，永不撤销（是复用缓存） |
| temp SID | 随机 | 否，per-session，会话间互不可见 | 是，dispose 时撤销 |

**workspace SID 复用**：workspace-root ACE 一次创建永久有效，后续 provision 命中 exact-ACE skip（O(1) 而非 O(n) 全树传播）。

#### ACL 编辑

一次 `SetNamedSecurityInfoW` 调用应用三个编辑：

1. **capability-SID allow ACE** — 授予写权限
2. **Deny ACE** — 移除 ambient `FILE_DELETE_CHILD`（world SID），防止跨可写根删除
3. **Low no-write-up mandatory label** — 防止低完整性进程写高完整性对象

#### 并发控制

per-path exclusive `LockFileEx` lock：

- lock 文件路径：`<GetTempPathW()>\dsh-acl-locks\<sha256(lowercased path)[0:16]>.lock`
- 防止并发沙盒实例互相 clobber 对方的 ACE
- Windows 路径大小写不敏感 → lowercased 确保同一目录只有一个 lock

#### 失败关闭

```typescript
// 每个 Win32 API 调用都检查返回值
// 任何失败直接 throw，包含 API 名称 + Win32 错误码 + 系统文本 + 受影响路径
// 绝不静默继续——绝不无限制 spawn 子进程
```

#### 已知边界

1. **仅限制写**：WRITE_RESTRICTED 令牌只影响写，读/网络/进程可见性不受限
2. **Low 完整性级限制**：低完整性进程无法写中/高完整性对象，但可读所有
3. **无网络隔离**：Windows ACL 后端不限制网络访问
4. **无进程创建限制**：受限令牌下子进程仍可创建（但继承受限令牌）

### 4.7 DSH 设计模式总结

| 模式 | 说明 |
|------|------|
| **Seam 分包** | sandbox / sandbox-policy / sandbox-local / sandbox-windows-acl 四包分离 |
| **链式探测** | 按平台选候选链，功能探测每个 rung，选第一个可用的 |
| **enforcement 报告** | `full` / `partial` 告知调用方策略执行完整度 |
| **严格宽化阶梯** | `WIDER_MODES` 只允许向更宽松方向升级 |
| **sandbox_permissions + justification** | 升级必须成对出现，justification 非空 |
| **fail-safe 默认** | `read-only` 是默认，workspace-write 需显式 opt-in |
| **策略渲染到 prompt** | 自然语言注入 system prompt，模型感知当前策略 |
| **per-session 覆盖** | 模式覆盖是会话事件，存活重启，会话间隔离 |
| **双 SID 复用** | workspace SID O(1) 复用，temp SID per-session 隔离 |
| **并发 ACL lock** | per-path LockFileEx 防止 ACE clobber |
| **失败关闭** | Win32 API 失败直接 throw，绝不无限制 spawn |

---

## 5. 关键设计对比

### 5.1 平台后端选择策略

| 策略 | Codex | OpenHuman | DSH |
|------|-------|-----------|>-----|
| **选择时机** | 编译时 `cfg!` | 运行时 `resolve_sandbox_policy()` | 运行时链式探测 |
| **降级方案** | 无（编译时固定） | NoopBackend（诚实 Inactive） | 下一个 rung（bwrap→Landlock） |
| **Windows 双后端** | RestrictedToken + MXC | AppContainer（tinybox） | windows-acl（单后端） |
| **Docker** | 无 | 有（远程会话默认） | 无 |

### 5.2 安全边界声明

| 边界 | Codex | OpenHuman | DSH |
|------|-------|-----------|-----|
| **文件系统写** | bwrap mount / Seatbelt deny / Token ACL | Landlock/Seatbelt/Docker mount | bwrap/Landlock/Seatbelt/ACL |
| **文件系统读** | bwrap 可选 deny-read | 无限制 | 无限制 |
| **网络** | seccomp / namespace / Seatbelt network | jail.deny_net() / Docker network=none | bwrap --unshare-net / Seatbelt network |
| **进程创建** | Job Object (Windows) | 无限制 | 无限制 |
| **系统调用** | seccomp-bpf (Linux) | 无 | 无 |
| **完整性** | 二进制完整性检查 | 无 | 无 |
| **违规检测** | 结构化 ViolationEvent | 无 | RunnerFailureRule |

### 5.3 模型交互

| 交互 | Codex | OpenHuman | DSH |
|------|-------|-----------|-----|
| **策略注入 prompt** | 模板文件 | 无 | 自然语言渲染 |
| **升级机制** | 无 | ElevatedOp 声明式 | WIDER_MODES + sandbox_permissions + justification |
| **拒绝反馈** | stderr 关键词 | 无 | sandboxDenialMarker |
| **enforcement 信号** | 无 | SandboxStatus | SandboxEnforcement |

### 5.4 操作范围检测的分层

前面三节比较的是「后端有多硬」。这一节换一个维度：**检查发生在什么时刻、依据是什么对象**。同样一句「禁止访问 `/etc`」，写在四个不同位置，安全性差几个数量级。

| 层 | 检查对象 | 检查时机 | 依赖参数字符串 | 项目 |
|------|---------|---------|:---:|------|
| **L4 内核强制** | 内核实际收到的 syscall | 进程运行时 | **否** | Codex（bwrap / Seatbelt / restricted token）· OpenHuman（tinybox-jail / Docker）· DSH（bwrap / Landlock / Seatbelt / windows-acl） |
| **L3 解析后判定** | `canonicalize` / `realpath` 后的真实路径 | 调用前 | 部分（入口是字符串，判定对象是解析结果） | Codex（`FileContentsChecker`）· OpenHuman（`validate_path`）· OpenBitFun（`host_policy::validate_path`）· DSH（`realpathNormalize`） |
| **L2 字符串匹配** | 参数字符串本身 | 调用前 | **是** | OpenHuman（`is_path_string_allowed` / `is_always_forbidden`）· Codex（glob deny 条目）· goose（`security/patterns.rs` 正则） |
| **L1 无检测** | — | — | — | 各项目的文件工具内部实现 |

**只有 L4 不依赖参数形态。** 下面三行对内核来说都是同一个 `open()`：

```text
cat /etc/shadow
cat "$(echo L2V0Yy9zaGFkb3c= | base64 -d)"
ln -s /etc evil && cat evil/shadow
```

L2 只能拦住第一种。后两行的字符串里根本不含 `/etc`，前缀匹配无从命中。所以 L2 保护的是「老实写法」，不是「操作范围」。

#### L4 的三种落点

| 项目 | Linux | macOS | Windows |
|------|-------|-------|---------|
| Codex | bwrap（`--ro-bind / /` 全盘只读起步 + 可写根 `--bind` 叠加）+ 进程内 `PR_SET_NO_NEW_PRIVS` + seccomp | Seatbelt `(deny default)` + 按访问类型生成 `(allow file-write* (subpath ...))` | restricted token + capability SIDs；另有 MXC 容器后端 |
| OpenHuman | Landlock（经 `tinybox-jail`） | Seatbelt（经 `tinybox-jail`） | AppContainer（经 `tinybox-jail`）；远程会话默认 Docker |
| DSH | bwrap → Landlock 链式探测 | Seatbelt | windows-acl（ACL 写入 + workspace SID） |

三个细节说明它确实是边界，而不是「参数校验的另一种写法」：

1. **符号链接与非存在路径用挂载遮罩**。Codex 在 bwrap 下对可写根内的 symlink-in-path 和缺失的受保护路径，挂 `/dev/null` 覆盖首个缺失组件——不是「解析路径后拒绝」，而是让该路径在沙箱视图里根本不存在。这是 L4 独有的手段：L3 只能对「已存在的东西」做判定，对「还不存在但将来会被创建」的路径无能为力。
2. **glob deny 条目在启动前展开再遮罩**。`**/*.env` 这类条目由 `rg --files` 展开成具体文件后逐个 mask，展开失败则整个沙箱构造中止（fail-closed）。
3. **可写根含符号链接直接报错**。Seatbelt 的 `normalize_writable_root_for_sandbox` 对含 symlink 组件的可写根返回 `symlinked writable roots are not supported`，而不是猜它指向哪里。

#### L3 的共同形状与共同弱点

四个项目的 L3 写法高度相似：先要求绝对路径，再解析，最后对解析结果做包含性判断。

| 项目 | 入口 | 解析方式 | 失败处理 |
|------|------|---------|---------|
| Codex | `FileContentsChecker::check()` | `fs::metadata` + `target.canonicalize()` | 注释自述边界：「resolves current objects; it does not assess ownership or OS ACLs」 |
| OpenHuman | `validate_path()` | `tokio::fs::canonicalize()`，对结果重跑 `is_resolved_path_allowed_for` / `check_resolved_against_forbidden` | `validate_parent_path()` 向上走到最深的已存在祖先再解析，用来抓「父目录是符号链接」 |
| OpenBitFun | `AppServerHostPolicy::validate_path()` | `dunce::canonicalize()` | 相对路径拒绝、解析失败也拒绝（fail-closed）；但只覆盖 `workspacePath` / `repositoryPath` / `projectWorkspacePath` 这 6 个固定 wire key |
| DSH | `realpathNormalize()` | `fs.realpath` | 非完全限定路径在 `realpath` 前就抛 `TypeError`；不存在路径保留原始 `ENOENT` |

OpenHuman 的注释直接写明了动机：「The string-level checks above can be bypassed by creating a symlink inside the workspace that points to a forbidden tree (e.g. `evil -> /etc/shadow`)」。配套测试有 `validate_path_blocks_symlink_to_forbidden_path`、`validate_parent_path_blocks_symlinked_parent_dir` 一组。

**L3 的共同弱点：它只保护经过这个函数的调用。** 任何绕开该函数的执行路径——子进程、脚本内部、第三方工具、MCP 服务——都退回 L1。这正是 L4 存在的理由，也是「有路径检查 ≠ 有边界」的分界线。

#### L2 单独使用时的两种形态

- **作为 L3 的前置快速拒绝**（OpenHuman）。`is_path_string_allowed()` 注释自己标了边界：「String-only path check. Does NOT resolve symlinks. Use `validate_path()` for any path that will be used for file I/O.」它挡的是 `\0`、`..` 组件、URL 编码的 `..%2f`、`~` 展开、凭据目录前缀、`workspace_only` 下的绝对路径。这层便宜、快、覆盖面广，但它的定位是「减少进入昂贵解析的次数」，不是安全边界。
- **作为威胁特征库**（goose）。`security/patterns.rs` 是一组正则（`ssh_key_exfiltration`、`password_file_access`、`systemd_service_creation` 等），由 `scanner.rs` 对 shell 工具的 command 文本做匹配，可选叠加一个 ML 分类器。它只处理 shell 工具，且输出是 `ScanResult { is_malicious, confidence, explanation }`——**一个评分，不是一个拒绝**。这类实现的价值是提示，不是隔离。

#### 降级语义：三个项目三种做法

沙箱不可用时怎么办，是这一节里分歧最大的一点。

| 项目 | 后端不可用时的行为 | 调用方如何得知 |
|------|-------------------|---------------|
| Codex | 启动期告警（bwrap 缺失 / 版本过旧 / 无 user namespace / WSL1）；策略与后端不兼容时**拒绝启用**该策略 | 启动日志 |
| OpenHuman | `cwd_jail::default_backend()` 替换成 `NoopBackend`，句柄报 `SandboxStatus::Inactive` 而非 `Ready` | 主动读状态字段 |
| DSH | 链式探测落到下一个 rung；`SandboxEnforcement` 报 `full` / `partial` | 主动读 enforcement |

三者都不 fail-open。差别在于**「谁知道边界没了」**：Codex 是启动期可见的告警，另两个需要调用方主动读字段才能发现。OpenHuman 的 `sandbox/README.md` 因此写了一句必须一起读的条款：

> **Do not treat a resolved `SandboxBackendKind` as a security boundary on its own.**

这句话适用于三个项目：**枚举值解析成功，不等于边界已经建立。**

#### 事后观测：没有主动监控

四个项目里没有任何一个做运行时文件系统监控（fsevents / inotify / auditd / ETW 都没有）。Codex 的 `violation.rs` 是**事后**从执行结果反推：

```rust
const SANDBOX_DENIED_KEYWORDS: [(FileSystemSandboxViolationReason, &str); 7] = [
    (OperationNotPermitted, "operation not permitted"),
    (PermissionDenied,      "permission denied"),
    (ReadOnlyFileSystem,    "read-only file system"),
    (PolicyDenied,          "seccomp"),
    (PolicyDenied,          "sandbox"),
    (PolicyDenied,          "landlock"),
    (FailedToWriteFile,     "failed to write file"),
];
```

分类依据是 exit_code + stderr 关键词，`extract_denied_path_from_text()` 从文本里捞出被拒路径，`SignalSyscall`（`128 + SIGSYS`）单独识别 seccomp 拦截。这是「从沙箱的实际裁决结果里回收信息」，不是主动扫描。它的价值在于把「被拦住」变成可审计事件（`SandboxViolationBackend` 区分 `linux_sandbox` / `seatbelt` / `windows_sandbox` / `windows_mxc` / `managed_network_proxy`），而不是事前预防。

#### 归纳

- **L4 只有三个有沙箱的项目具备**，且都带降级路径。没有沙箱的项目（goose）连 L2 都只有 advisory 形态。
- **L3 出现得比 L4 广**，因为它不需要平台能力——OpenBitFun 没有沙箱但有 L3。但 L3 的作用域由函数调用决定，不是由系统决定。
- **L2 从不单独成立**。凡是认真写 L2 的项目，都会在注释里标出它的边界并指向 L3。
- **「路径检查」和「操作范围检测」是两件事**。前者是字符串或解析结果上的判定，后者要求一个不随参数形态变化的裁决点。只有 L4 提供后者。

---

## 6. 对本项目的借鉴建议

### 6.1 必须借鉴

| 模式 | 来源 | 理由 |
|------|------|------|
| **链式探测** | DSH | 优雅处理"沙盒不可用"，比编译时固定更灵活 |
| **enforcement 报告** | DSH | 调用方必须知道策略是否完整执行 |
| **fail-safe 默认** | DSH | `read-only` 默认，workspace-write 需显式 opt-in |
| **受保护元数据路径** | Codex | `.git`/`.codex`/`.aws` 在可写根下强制只读 |
| **硬编码 Seatbelt 路径** | Codex | `/usr/bin/sandbox-exec` 防 PATH 注入 |
| **失败关闭** | DSH Windows ACL | API 失败直接 throw，绝不无限制 spawn |

### 6.2 强烈建议借鉴

| 模式 | 来源 | 理由 |
|------|------|------|
| **严格宽化阶梯** | DSH | 升级只能向更宽松方向，防止降级攻击 |
| **策略渲染到 prompt** | DSH | 模型感知当前策略，减少无效尝试 |
| **Noop 诚实报告** | OpenHuman | 防止调用方误以为已隔离 |
| **违规检测** | Codex | 结构化 ViolationEvent 用于审计和遥测 |
| **双 SID 复用** | DSH | Windows ACL 性能优化，O(1) 复用 |
| **并发 ACL lock** | DSH | 多沙盒实例并发安全 |

### 6.3 可选借鉴

| 模式 | 来源 | 理由 |
|------|------|------|
| **Docker 后端** | OpenHuman | 远程/云端场景可选，桌面端不优先 |
| **完整性检查** | Codex | 增加启动延迟，但防篡改 |
| **MXC 后端** | Codex | Windows 新方案，需 PSEC 环境 |
| **ElevatedOp 声明式** | OpenHuman | git/install 等必须在宿主执行的操作 |

### 6.4 不建议借鉴

| 模式 | 来源 | 理由 |
|------|------|------|
| **编译时平台选择** | Codex | 不够灵活，无法运行时降级 |
| **Seatbelt 输出文件中转** | OpenHuman | 是 Seatbelt 限制的 workaround，应优先找更好方案 |
| **Landlock 遗留** | Codex | Codex 已弃用 Landlock 做文件系统隔离（无法隔离 Unix socket） |

---

## 7. 关键源码索引

### Codex

| 文件 | 职责 |
|------|------|
| `references/1-ai-agent/codex/codex-rs/protocol/src/sandbox.rs` | SandboxType 枚举 + SandboxOverride |
| `references/1-ai-agent/codex/codex-rs/protocol/src/permissions.rs` | FileSystemAccessMode, NetworkSandboxPolicy, 受保护路径 |
| `references/1-ai-agent/codex/codex-rs/sandboxing/src/lib.rs` | 模块入口，条件编译 |
| `references/1-ai-agent/codex/codex-rs/sandboxing/src/manager.rs` | SandboxManager，平台选择 + 策略转换 |
| `references/1-ai-agent/codex/codex-rs/sandboxing/src/bwrap.rs` | Linux bubblewrap 命令构建 |
| `references/1-ai-agent/codex/codex-rs/sandboxing/src/seatbelt.rs` | macOS Seatbelt profile 生成 |
| `references/1-ai-agent/codex/codex-rs/sandboxing/src/seatbelt_base_policy.sbpl` | 基础 SBPL 策略 |
| `references/1-ai-agent/codex/codex-rs/sandboxing/src/violation.rs` | 违规检测与记录 |
| `references/1-ai-agent/codex/codex-rs/sandboxing/src/denial.rs` | 拒绝推断 |
| `references/1-ai-agent/codex/codex-rs/linux-sandbox/src/seccomp.rs` | seccomp-bpf 过滤器 |
| `references/1-ai-agent/codex/codex-rs/mxc-sandbox/README.md` | MXC 后端说明 |

### OpenHuman

| 文件 | 职责 |
|------|------|
| `references/1-ai-agent/openhuman/crates/openhuman-core/src/sandbox/mod.rs` | 模块入口 |
| `references/1-ai-agent/openhuman/crates/openhuman-core/src/sandbox/types.rs` | SandboxBackendKind, SandboxPolicy, SandboxStatus |
| `references/1-ai-agent/openhuman/crates/openhuman-core/src/sandbox/ops.rs` | 策略解析 + 执行路由 |
| `references/1-ai-agent/openhuman/crates/openhuman-core/src/sandbox/docker.rs` | Docker 一次性容器后端 |

### DSH

| 文件 | 职责 |
|------|------|
| `references/1-ai-agent/deepseek-harness-orb/packages/sandbox/sandbox/src/index.ts` | 核心 seam：SandboxMode, SandboxPolicy, ConfinedArgv |
| `references/1-ai-agent/deepseek-harness-orb/packages/sandbox/sandbox/src/escalation.ts` | 升级词汇 + 严格宽化阶梯 |
| `references/1-ai-agent/deepseek-harness-orb/packages/sandbox/sandbox-policy/src/index.ts` | 策略解析 + 渲染 |
| `references/1-ai-agent/deepseek-harness-orb/packages/sandbox/sandbox-policy/src/session-mode.ts` | per-session 模式覆盖 |
| `references/1-ai-agent/deepseek-harness-orb/packages/sandbox/sandbox-local/src/index.ts` | 链式探测 + confine() |
| `references/1-ai-agent/deepseek-harness-orb/packages/sandbox/sandbox-local/src/profiles.ts` | bwrap/Landlock/Seatbelt profile 构建 |
| `references/1-ai-agent/deepseek-harness-orb/packages/sandbox/sandbox-windows-acl/src/token.ts` | WRITE_RESTRICTED Token 构建 |
| `references/1-ai-agent/deepseek-harness-orb/packages/sandbox/sandbox-windows-acl/src/acl.ts` | ACL 编辑 + 并发 lock |

### 对照组（无沙箱，仅用于 §5.4 分层对照）

| 文件 | 职责 |
|------|------|
| `references/1-ai-agent/goose/crates/goose-roaming/SECURITY.md` | 自述无 per-peer capability scoping / read-only mode / session sandboxing |
| `references/1-ai-agent/goose/crates/goose/src/security/patterns.rs` | 正则威胁特征库（advisory，L2） |
| `references/1-ai-agent/goose/crates/goose/src/security/scanner.rs` | 仅对 shell 工具文本评分，不产生拒绝 |
| `references/1-ai-agent/OpenBitFun/src/crates/interfaces/app-server/src/server/host_policy.rs` | App Server 边界路径检查：绝对路径 + `dunce::canonicalize` + 包含性判断（L3） |
| `references/1-ai-agent/deepseek-harness-orb/packages/workspace/workspace/src/paths.ts` | `realpathNormalize`：workspace 身份的 canonical 基准（L3） |
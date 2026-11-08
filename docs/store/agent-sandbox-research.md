# Agent 命令行隔离环境开源项目调研

> **本文档职责**：梳理当前 Agent 命令行隔离/沙箱领域的开源项目，按虚拟化深度分类，评估对本项目的参考价值，给出演进路线建议。
> **关联文档**：[I18 架构决策](./I18-agent-tool-architecture-design.md) · [I18 实施规格](./I18-agent-tool-architecture-spec.md) · [OS Call 设计](../designed/OSCALLDESIGN.md)

---

## 1. 本项目现状

### 1.1 已有能力

| 层级 | 现状 | 代码位置 |
|------|------|---------|
| Tool Trait | `Tool` trait 含 `tier()` → `ToolTier::{Sandbox, Os, External}`，`behavior_hints()` → `{read_only, destructive, open_world}`，`permission_intents()` → 声明式权限意图 | `assistant-tools/src/trait.rs` |
| Tool Registry | `DashMap` 注册表，支持按 tier 过滤生成 LLM 工具定义 | `assistant-tools/src/registry.rs` |
| 执行入口 | `os_tool_exec` Tauri command，查注册表 → 拼装参数 → `tool.execute()` | `cmd/os_tool_exec.rs` |
| 已有工具 | 仅 `fs_link`（symlink/hardlink/junction），tier=Os | `assistant-tools/src/os/fs_link.rs` |
| 上下文 | `ToolUseContext { workspace_root, session_id, tool_call_id }` | `assistant-tools/src/context.rs` |
| 安全模块 | `assistant-security` 覆盖 vault/crypto/biometric/clipboard_guard/reveal_guard | `assistant-security/src/lib.rs` |

### 1.2 关键缺口

| 缺口 | 说明 |
|------|------|
| **Sandbox tier 无实现** | `ToolTier::Sandbox` 已定义但无对应工具 |
| **PermissionIntent 无拦截** | `permission_intents()` 仅声明式，无实际执行层拦截 |
| **无进程级隔离** | Os 层工具与主进程同进程运行，无 OS 级隔离 |
| **无 shell 命令执行** | 无 `shell` 工具，Agent 无法执行命令行 |
| **External 层无隔离** | MCP 进程无 cgroup/seccomp/namespace 隔离（OSCALLDESIGN §1.2 已声明） |

### 1.3 参考项目内已有实现

本项目 `references/1-ai-agent/OpenBitFun` 提供了可直接借鉴的模块：

| 模块 | 能力 | 代码位置 |
|------|------|---------|
| `shell_analysis` | 静态 shell 命令分析：词法分析 + 递归下降解析，提取文件读写/网络/重定向等语法事实，**不执行任何命令** | `execution/tool-execution/src/shell_analysis/` |
| `terminal-core` | PTY 进程管理、shell 集成与解析、跨平台 (Win/Mac/Linux) 兼容 | `services/terminal/` |

---

## 2. 开源项目分类总览

按虚拟化深度从浅到深分为五类：

```
L0 声明式拦截 ─── 无实际隔离，仅策略决策
L1 OS 原生虚拟化 ── namespace / Token / Seatbelt（进程级，共享内核）
L2 LSM 强制访问 ── Landlock / seccomp（内核安全模块，不可绕过）
L3 用户态内核 ─── gVisor Sentry（系统调用全拦截，用户态实现）
L4 硬件虚拟化 ─── microVM / KVM（独立内核，最强隔离）
```

> 本文档聚焦 **L1-L3 软件虚拟层**，L4（CubeSandbox/E2B/Firecracker）在 §7 简述。

---

## 3. L1 — OS 原生虚拟化层

利用操作系统内核提供的虚拟化原语，构建进程级隔离——不引入额外虚拟机或容器运行时，直接在宿主内核上运行，但通过内核特性虚拟化文件系统/网络/进程可见性。

### 3.1 Bubblewrap (bwrap)

| 维度 | 内容 |
|------|------|
| 虚拟化机制 | Linux namespace（mount/pid/net/user/ipc）+ pivot_root |
| 原理 | 进程看到虚拟化的文件系统视图：`--ro-bind` 只读挂载、`--bind` 读写挂载、`--dev /dev` 虚拟设备树、`--unshare-net` 隔离网络栈 |
| 开源 | GitHub: containers/bubblewrap，Flatpak 底层运行时 |
| 语言 | C，极轻量 |
| 特权 | 仅需 setuid root（用于 user namespace），非 root 也可用 |

**虚拟层本质**：mount namespace 构建虚拟文件系统视图——进程以为自己在真实文件系统上操作，但实际所有路径都被重定向到隔离的挂载点。网络 namespace 构建虚拟网络栈。

**对本项目的参考价值**：
- ⭐⭐⭐ Linux 上的首选虚拟层：DeepSeek Harness 和 Codex 都选 bwrap 作为 Linux 默认沙箱后端
- ⭐ 声明式配置：`--ro-bind /usr /usr` 风格与本项目 `PermissionIntent` 天然对齐
- ⚠️ 仅 Linux，Windows/macOS 需另选方案

### 3.2 Firejail

| 维度 | 内容 |
|------|------|
| 虚拟化机制 | Linux namespace + seccomp-bpf 系统调用过滤 + Linux capabilities 裁剪 + AppArmor profile |
| 原理 | 比 bwrap 更激进：不仅虚拟化文件系统/网络，还通过 seccomp 在系统调用层拦截，通过 capabilities 裁剪特权 |
| 开源 | GitHub: netblue30/firejail，4k+ Stars |
| 语言 | C |
| 特点 | 声明式 profile：`private-bin bash,ls,cat` → 自动构建虚拟 PATH；`private-etc` → 虚拟化 /etc；`nodvd/nosound/notv` → 设备黑名单 |

**虚拟层本质**：三层虚拟化叠加——namespace（资源视图虚拟化）+ seccomp（系统调用层虚拟化）+ capabilities（特权虚拟化）。

**对本项目的参考价值**：
- ⭐⭐ profile 声明式设计：`private-bin`、`private-etc` 等可映射到 `PermissionIntent { action, resources }` → 自动生成 Firejail profile
- ⭐ seccomp 系统调用过滤：比 namespace 更细粒度
- ⚠️ SUID root 安全争议：Firejail 本身有过本地提权 CVE

### 3.3 Sandboxie-Plus — Windows 内核驱动级虚拟化

| 维度 | 内容 |
|------|------|
| 虚拟化机制 | **Object-Based Virtualization**：Windows 内核过滤驱动（SbieDrv）拦截 Ring 0 层 API |
| 原理 | 文件系统重定向：写 `C:\Windows\System32\config` → 实际写入 `C:\Sandbox\User\DefaultBox\drive\C\...`；注册表重定向：写 `HKLM\Software\...` → 实际写入虚拟注册表 hive；IPC/命名管道过滤 |
| 开源 | GitHub: sandboxie-plus/Sandboxie，GPL v3 |
| 语言 | C/C++（内核驱动 + 用户态服务） |
| 平台 | Windows only |

**虚拟层本质**：最纯粹的"软件虚拟层"——通过内核驱动在系统调用入口处拦截，将所有写操作重定向到虚拟存储区，读操作优先读虚拟区再 fallback 到真实系统。进程完全不知道自己运行在虚拟环境中。

**对本项目的参考价值**：
- ⭐⭐⭐ Windows 上最成熟的软件虚拟层：无需 Docker/VM/WSL，纯 Windows 内核驱动实现
- ⭐⭐ 重定向范式：文件/注册表/IPC 三维重定向，覆盖 Windows 上 Agent 命令执行的所有副作用面
- ⭐ "无痕运行"理念：关闭沙箱即清除所有变更，天然适合 Agent 工具的"试运行"场景
- ⚠️ 内核驱动稳定性：蓝屏风险，需 WHQL 签名；GPL 协议与本项目兼容性需评估

### 3.4 OpenAI Codex windows-sandbox-rs — Windows Token+Job 虚拟化

| 维度 | 内容 |
|------|------|
| 虚拟化机制 | Write-Restricted Token + Job Object + 临时桌面对象 + 合成 SID ACL |
| 原理 | ① `CreateRestrictedToken()` 创建受限令牌（移除特权 SID、添加 deny-only SID）② Job Object 限制进程创建/剪贴板/窗口站访问 ③ 合成 SID 写入 ACL：只有携带特定 synthetic SID 的沙箱进程才能写入指定目录 |
| 开源 | Codex CLI 内的 `windows-sandbox-rs` 模块 |
| 语言 | Rust，约 30 个文件 |
| 双模式 | Advisory（unelevated，建议级）→ 失败时升级 → Mandatory（elevated，强制级） |

**虚拟层本质**：Windows 没有类似 Linux namespace 的轻量虚拟化，Codex 用 Token + Job Object + Desktop 三件套构建等价虚拟层——进程在受限令牌下运行，文件系统是真实的但写入被 SID ACL 虚拟化（只有特定 SID 可写特定目录）。

**对本项目的参考价值**：
- ⭐⭐⭐ Windows 上最成熟的 Rust 沙箱实现：直接可参考或 fork
- ⭐⭐ Write-Restricted Token + 合成 SID：精巧的权限虚拟化——不隐藏文件，但通过 SID ACL 让写入"虚拟化"到允许的目录
- ⭐ 双模式降级：advisory → mandatory 渐进式隔离，用户体验友好
- ⭐ Rust 实现：与本项目技术栈完全匹配

### 3.5 macOS Seatbelt (sandbox-exec)

| 维度 | 内容 |
|------|------|
| 虚拟化机制 | Apple 原生沙箱框架，通过 `sandbox-exec` / `sandbox_init_with_parameters()` 应用 Seatbelt profile |
| 原理 | 声明式 profile 定义允许/拒绝的文件路径、网络访问、信号、IPC、sysctl 等。进程 exec 后 profile 不可移除 |
| 开源 | macOS 内置，非开源；Claude Code / DeepSeek Harness 均使用此方案 |
| 语言 | 系统调用，profile 为文本格式 |

**对本项目的参考价值**：
- ⭐⭐ macOS 唯一可行的轻量虚拟层方案
- ⭐ 声明式 profile 与 `PermissionIntent` 对齐
- ⚠️ Apple 私有 API，profile 格式无官方文档（社区逆向工程）

---

## 4. L2 — LSM 强制访问控制层

### 4.1 Landlock (Linux LSM 5.13+)

| 维度 | 内容 |
|------|------|
| 虚拟化机制 | Linux LSM (Linux Security Module)，在内核 VFS 层拦截文件访问 |
| 原理 | 进程 exec 时绑定一组不可移除的文件系统访问规则（allow-list）：未列入的路径一律拒绝。规则在 exec 后仍然生效（比 seccomp 更强） |
| 开源 | Linux 内核内置；Rust crate: `landlock` (crates.io) |
| 无需特权 | 无需 root/CAP_SYS_ADMIN，普通进程可自行降权 |

**虚拟层本质**：不是"虚拟化"而是"强制访问控制"——进程仍然看到真实文件系统，但访问被 LSM 层强制过滤。效果等价于文件系统虚拟化（不可访问 = 不存在）。

**对本项目的参考价值**：
- ⭐⭐⭐ Rust crate 直接可用：`landlock` crate 可直接加入 `assistant-tools` 依赖
- ⭐⭐ self-restrict-then-exec 模式：先限制自己，再 exec 目标命令——约 300 行 C/Rust 即可实现
- ⭐ 与 PermissionIntent 天然对齐：`PermissionIntent { action: "read", resources: ["/workspace"] }` → 直接生成 Landlock ruleset
- ⚠️ 仅 Linux 5.13+，macOS/Windows 需另选方案

### 4.2 seccomp-bpf

| 维度 | 内容 |
|------|------|
| 虚拟化机制 | Linux 系统调用过滤，在内核系统调用入口处拦截 |
| 原理 | BPF 程序对每个系统调用返回 ALLOW / DENY / TRAP / USER_NOTIFY。USER_NOTIFY 模式可将系统调用转发到用户态代理决策 |
| 开源 | Linux 内核内置；libseccomp (C) / seccomp-tiny-rs (Rust) |
| 特权 | 需 CAP_SYS_ADMIN 或 PR_SET_NO_NEW_PRIVS |

**对本项目的参考价值**：
- ⭐⭐ USER_NOTIFY 模式：截获 `connect()`/`socket()` → 用户态代理决策 → 放行/拒绝，可实现网络访问控制
- ⭐ 与 Landlock 互补：Landlock 管文件系统，seccomp 管系统调用（网络/信号/IPC）
- ⚠️ BPF 程序编写复杂，需仔细测试

---

## 5. L3 — 用户态内核虚拟化层

### 5.1 gVisor (runsc)

| 维度 | 内容 |
|------|------|
| 虚拟化机制 | **Sentry**：用户态 Go 程序，充当"内核"，拦截应用所有系统调用并在用户态处理 |
| 原理 | 应用 → 系统调用 → Sentry（用户态内核）→ 仅少量必要调用转发到宿主内核。Sentry 自己运行在 seccomp 严格限制下 |
| 开源 | GitHub: google/gvisor，Apache 2.0 |
| 语言 | Go |
| OCI 兼容 | 是，`runsc` 可替换 `runc` 作为 containerd 运行时 |
| 性能 | 系统调用密集型 workload 有 2-10x 开销 |

**虚拟层本质**：最彻底的软件虚拟化——应用与宿主内核之间插入一个完整的用户态内核层。应用以为自己在跟 Linux 内核交互，实际在跟 Sentry 交互。

**对本项目的参考价值**：
- ⭐⭐ 架构参考价值极高：用户态内核 = 最强软件隔离，可作为"高安全模式"的后端选项
- ⭐ OCI 兼容：如果本项目未来支持容器后端，gVisor 可直接替换 runc
- ⚠️ Go 语言：与本项目 Rust 技术栈不匹配，直接集成困难
- ⚠️ 仅 Linux

---

## 6. AI Agent 专用虚拟层（跨平台多后端抽象）

### 6.1 OpenShell (NVIDIA)

| 维度 | 内容 |
|------|------|
| 虚拟化机制 | 三层：① Landlock LSM（文件系统白名单）② seccomp user-notify（网络/DNS 拦截）③ eBPF（运行时监控） |
| 原理 | Landlock 做文件系统虚拟化，seccomp user-notify 截获 `connect()`/`socket()` 系统调用并转发到用户态代理决策，eBPF 做运行时行为审计 |
| 开源 | GitHub: NVIDIA/OpenShell，19k Stars |
| 语言 | Rust（核心沙箱 crate） |
| 定位 | "The safe, private runtime for fleets of autonomous AI agents" |

**虚拟层本质**：组合虚拟层——文件系统层(Landlock) + 网络层(seccomp) + 审计层(eBPF)，三层协同构建完整的 Agent 执行虚拟环境。

**对本项目的参考价值**：
- ⭐⭐⭐ Rust 技术栈完全匹配：核心沙箱 crate 可直接参考或依赖
- ⭐⭐ 三层架构设计：文件系统 + 网络 + 审计，恰好覆盖 `BehaviorHints { read_only, destructive, open_world }` 三个维度的实际执行
- ⭐ seccomp user-notify 网络拦截：`open_world=false` 时截获网络系统调用 → 用户态代理决策
- ⚠️ 仅 Linux（Landlock/seccomp/eBPF 都是 Linux 内核特性）

### 6.2 DeepSeek Harness (DSH) — 跨平台多后端

| 维度 | 内容 |
|------|------|
| Linux | bwrap（namespace 虚拟化）+ Landlock（LSM 强制访问控制），双打互补 |
| macOS | Seatbelt（`sandbox-exec`，Apple 原生沙箱 profile） |
| Windows | 受限 Token + ACL（类似 Codex 方案） |
| 降级策略 | 无可用 runner 时返回 `sandbox_unavailable`，**fail-closed**——绝不静默无沙箱执行 |

**虚拟层本质**：跨平台抽象层——每个平台选择该平台最强的虚拟化原语，统一暴露为相同的沙箱 API。

**对本项目的参考价值**：
- ⭐⭐⭐ 跨平台方案最完整：Linux(bwrap+Landlock) / macOS(Seatbelt) / Windows(Token+ACL) 三平台全覆盖
- ⭐⭐ fail-closed 策略：沙箱不可用时拒绝执行而非放行——安全优先
- ⭐ bwrap + Landlock 双打：bwrap 做 namespace 虚拟化（文件系统视图），Landlock 做 LSM 强制访问控制（安全兜底），两者互补

### 6.3 Claude Code Sandboxing

| 维度 | 内容 |
|------|------|
| macOS | `sandbox-exec` (seatbelt profile) |
| Linux | Landlock LSM + cgroups |
| 跨平台 | Docker 容器（可选） |
| 特点 | 用户可配置 `--sandbox` 模式，自动批准模式下仍受沙箱保护 |

**对本项目的参考价值**：
- ⭐⭐ seatbelt/landlock 配置文件范式：声明式沙箱 profile 与本项目 `PermissionIntent` 声明式设计天然对齐
- ⭐ 渐进式隔离：默认无沙箱 → 可选沙箱 → 强制沙箱
- ⚠️ macOS seatbelt 无 Windows 等价物

### 6.4 OpenAI Codex Sandbox — 双模式 + 四层架构

| 维度 | 内容 |
|------|------|
| 架构 | **双模式**：Advisory（无提权，建议级）+ Mandatory（提权，强制级）；**四层**：进程 → 网络 → 文件系统 → 中间件 |
| macOS | seatbelt/pledge |
| Linux | landlock/cgroups |
| Windows | unelevated token + Job Object（`windows-sandbox-rs`） |

**对本项目的参考价值**：
- ⭐⭐⭐ 分层隔离架构最值得借鉴：四层模型可直接映射到本项目
  - 进程层 → `ToolTier::Sandbox` 的进程创建约束
  - 网络层 → `BehaviorHints::open_world` 的实际拦截
  - 文件系统层 → `PermissionIntent` 的路径白名单强制执行
  - 中间件层 → 环境变量/信号/IPC 过滤
- ⭐⭐ Windows 沙箱方案：Codex 的 `unelevated token + Job Object` 是当前最成熟的 Windows Agent 沙箱工程实践
- ⭐ Advisory vs Mandatory 双模式：对应本项目 Agent 模式的 Quick（advisory）/ Deep（mandatory）

### 6.5 Chromium Sandbox — 工业级多层虚拟化

| 维度 | 内容 |
|------|------|
| Windows | Restricted Token + Job Object + Alternate Desktop + Integrity Level |
| Linux | namespace + seccomp-bpf |
| macOS | Seatbelt profile |
| 架构 | Target 进程（受限）↔ Broker 进程（特权，按需代理 IO 操作） |

**虚拟层本质**：受限进程无法直接做任何 IO，所有 IO 请求通过 IPC 发给 Broker 进程代为执行——**代理式虚拟化**。

**对本项目的参考价值**：
- ⭐⭐ Broker 代理模式：Agent 工具进程 → IPC → Broker（Tauri 主进程）→ 实际 IO，可实现最细粒度的权限控制
- ⭐ Integrity Level：Windows 低完整性级别进程无法写中/高完整性对象
- ⚠️ 实现复杂度高：Chrome 沙箱历经 15+ 年演进，直接移植不现实

### 6.6 learn-claude-code — Agent Harness 教学项目

| 维度 | 内容 |
|------|------|
| Stars | 17k+ |
| 内容 | 20 章从零构建 Claude Code 式 Harness：工具注册 → 权限模型 → 沙箱隔离 → 流式输出 → 错误恢复 |
| 开源 | MIT 协议 |

**对本项目的参考价值**：
- ⭐⭐ 工程教学价值最高：从 Tool Registry → Permission Check → Sandbox Isolation 的完整链路
- ⭐ Harness 概念：Agent 执行环境 = 工具注册 + 权限拦截 + 沙箱隔离 + 输出捕获 + 错误恢复

---

## 7. L4 — 硬件虚拟化层（简述）

| 项目 | 隔离技术 | 启动速度 | 语言 | Rust 兼容 | 说明 |
|------|---------|---------|------|----------|------|
| CubeSandbox (TencentCloud) | RustVMM + KVM microVM | 60ms，<5MB | Rust | ⭐⭐⭐ | Rust 技术栈完全匹配，兼容 E2B SDK；仅 Linux/WSL2 |
| E2B | Firecracker microVM | ~150ms | Go/TS SDK | ⭐ | 云端服务，桌面端不直接适用；OpenAI Codex 官方沙箱提供商 |
| Docker Sandbox | Docker 容器 + microVM | 秒级 | 多语言 | ❌ | 需用户安装 Docker，桌面端体验门槛高 |

> L4 方案作为"高安全场景可选后端"保留，不在 P0/P1 路线中。

---

## 8. 云端沙箱服务（简述）

| 项目 | 提供方 | 特点 | 桌面端适用性 |
|------|--------|------|-------------|
| E2B | e2b-dev | Firecracker microVM，Python/JS SDK | ❌ 云端 |
| AgentRun SDK | 阿里云函数计算 | Serverless 安全容器，LangChain 集成 | ❌ 云端 |
| Daytona | 开源 | OCI 容器，sub-90ms 冷启动，OpenAI Agents SDK 官方提供商 | ❌ 偏服务端 |
| AIO Sandbox | 开源 | 单 Docker 镜像整合浏览器+Shell+MCP+VSCode | ⚠️ 需 Docker |

> 云端方案可作为未来"云端执行"扩展选项，当前桌面端不优先。

---

## 9. 综合对比矩阵

| 项目 | 虚拟化深度 | 虚拟化方式 | 语言 | Windows | macOS | Linux | Rust 兼容 | Agent 专用 |
|------|-----------|-----------|------|---------|-------|-------|----------|-----------|
| Sandboxie-Plus | L3 内核驱动 | 文件/注册表/IPC 重定向 | C/C++ | ✅ | ❌ | ❌ | ❌ | ❌ |
| gVisor | L3 用户态内核 | 系统调用全拦截 | Go | ❌ | ❌ | ✅ | ❌ | ❌ |
| bwrap | L1 namespace | 文件系统/网络视图虚拟化 | C | ❌ | ❌ | ✅ | ❌ | ❌ |
| Firejail | L1+ namespace+seccomp | 三层虚拟化叠加 | C | ❌ | ❌ | ✅ | ❌ | ❌ |
| Landlock | L2 LSM | 文件系统强制访问控制 | Rust crate | ❌ | ❌ | ✅ | ✅ | ❌ |
| seccomp-bpf | L2 系统调用过滤 | 系统调用拦截 | C/Rust | ❌ | ❌ | ✅ | ✅ | ❌ |
| OpenShell | L2+ 组合 | Landlock+seccomp+eBPF | Rust | ❌ | ❌ | ✅ | ✅ | ✅ |
| Codex win-sandbox | L1 Token+Job | SID ACL 写入虚拟化 | Rust | ✅ | ❌ | ❌ | ✅ | ✅ |
| DSH | L1 跨平台 | bwrap/Landlock/Seatbelt/ACL | 多语言 | ✅ | ✅ | ✅ | 部分 | ✅ |
| Claude Code | L1+ 跨平台 | Landlock/Seatbelt/Docker | TS | ❌ | ✅ | ✅ | ❌ | ✅ |
| Codex Sandbox | L1 四层 | Token+Job/Landlock/Seatbelt | TS+Rust | ✅ | ✅ | ✅ | 部分 | ✅ |
| Chromium | L3 Broker代理 | Token+Job+Desktop+seccomp | C++ | ✅ | ✅ | ✅ | ❌ | ❌ |

---

## 10. 对本项目的演进路线建议

### 10.1 设计原则

1. **先安全框架再加能力**：I18-spec 已确立此原则——先用 fs_link 验证端到端，再建安全框架，再安全地加 shell
2. **声明式权限 → 实际拦截闭环**：`PermissionIntent` 声明 → `SandboxBackend` trait 执行，完成从"策略决策"到"虚拟化执行"的闭环
3. **fail-closed**：沙箱不可用时拒绝执行而非放行
4. **跨平台抽象**：每个平台选最强虚拟化原语，统一暴露为 `SandboxBackend` trait

### 10.2 演进路线

```
当前: Tool trait + PermissionIntent (声明式，无拦截)
  │
  ▼ P0 — 静态预检 + 平台原生隔离
  │
  ├─ shell_analysis 静态预检 (借鉴 OpenBitFun)
  │   → permission_intents() 返回值 → 实际拦截/放行决策
  │
  ├─ Windows: Write-Restricted Token + Job Object (借鉴 Codex windows-sandbox-rs)
  │   → Rust 实现，SID ACL 写入虚拟化
  │
  ├─ Linux: Landlock crate (Rust crate 直接依赖)
  │   → self-restrict-then-exec，文件系统 allow-list
  │
  └─ macOS: Seatbelt profile (sandbox-exec)
      → 声明式 profile 生成
  │
  ▼ P1 — 增强隔离 + 降级方案
  │
  ├─ Linux 补充: bwrap (namespace 虚拟化)
  │   → Landlock 不可用时的降级方案
  │
  ├─ Linux 增强: seccomp user-notify (网络拦截)
  │   → open_world=false 时截获网络系统调用
  │
  ├─ Windows 增强: 双模式 (Advisory → Mandatory)
  │   → 借鉴 Codex 双模式降级
  │
  └─ 跨平台: SandboxBackend trait 统一抽象
      → 每个平台实现该 trait
  │
  ▼ P2 — 高安全场景可选后端
  │
  ├─ Windows: Sandboxie 集成 (内核驱动重定向)
  ├─ Linux: OpenShell 集成 (Landlock+seccomp+eBPF)
  ├─ 可选: Docker 后端 (AIO/Daytona 模式)
  └─ 可选: microVM 后端 (CubeSandbox 模式)
```

### 10.3 SandboxBackend trait 设计草案

```rust
#[async_trait]
pub trait SandboxBackend: Send + Sync {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;

    async fn prepare(
        &self,
        config: &SandboxConfig,
    ) -> Result<SandboxHandle, SandboxError>;

    async fn exec(
        &self,
        handle: &SandboxHandle,
        command: &str,
        args: &[&str],
        env: &HashMap<String, String>,
    ) -> Result<ProcessOutput, SandboxError>;

    async fn cleanup(
        &self,
        handle: &SandboxHandle,
    ) -> Result<(), SandboxError>;
}

pub struct SandboxConfig {
    pub workspace_root: PathBuf,
    pub readonly_paths: Vec<PathBuf>,
    pub writable_paths: Vec<PathBuf>,
    pub network_allowed: bool,
    pub env_allowlist: Vec<String>,
}
```

### 10.4 与现有架构的对接

| 现有概念 | 对接方式 |
|---------|---------|
| `ToolTier::Sandbox` | Sandbox tier 工具通过 `SandboxBackend` 执行 |
| `PermissionIntent { action, resources }` | → 生成 `SandboxConfig { readonly_paths, writable_paths, network_allowed }` |
| `BehaviorHints { read_only, destructive, open_world }` | → `read_only=true` → 全部 readonly_paths；`open_world=false` → `network_allowed=false` |
| `ToolUseContext { workspace_root }` | → `SandboxConfig::workspace_root` |
| 洋葱模型 (OSCALLDESIGN §3) | SafetyBelt → AllowlistGate → ApprovalGate 之上新增 SandboxGate |

### 10.5 优先级总结

| 优先级 | 方案 | 虚拟化层 | 参考项目 | 理由 |
|--------|------|---------|---------|------|
| **P0** | shell_analysis 静态预检 | L0 声明式 | OpenBitFun | 参考项目内已有，零外部依赖 |
| **P0** | Windows: Token+Job Object | L1 SID ACL | Codex `windows-sandbox-rs` | Rust 原生，Windows 主要用户群 |
| **P0** | Linux: Landlock crate | L2 LSM | `landlock` crate | Rust crate 直接加依赖 |
| **P0** | macOS: Seatbelt | L1 原生 | DSH / Claude Code | macOS 唯一可行轻量方案 |
| **P1** | Linux: bwrap 降级 | L1 namespace | bwrap / DSH | Landlock 不可用时的降级 |
| **P1** | Linux: seccomp 网络拦截 | L2 系统调用 | OpenShell | `open_world` 实际执行 |
| **P1** | Windows: 双模式降级 | L1 Token | Codex | Advisory → Mandatory |
| **P2** | Windows: Sandboxie | L3 内核驱动 | Sandboxie-Plus | 高安全场景可选 |
| **P2** | Linux: OpenShell | L2+ 组合 | OpenShell | 高安全场景可选 |
| **P2** | Docker/microVM 后端 | L4 | CubeSandbox/E2B | 高安全场景可选 |

---

## 11. 关键设计决策记录

### ADR-1: 为什么 P0 选 Codex windows-sandbox-rs 而非 Sandboxie

- **Rust 技术栈匹配**：Codex 方案是 Rust 实现，可直接参考；Sandboxie 是 C/C++ 内核驱动，集成需 FFI + 驱动签名
- **无内核驱动依赖**：Token+Job Object 是 Windows 用户态 API，无需安装驱动；Sandboxie 需安装 SbieDrv 内核驱动
- **双模式降级**：Codex 的 Advisory → Mandatory 模式用户体验更友好
- **Sandboxie 保留为 P2**：高安全场景（如企业部署）可选用，但不应作为默认方案

### ADR-2: 为什么 Linux 选 Landlock 而非 bwrap 作为 P0

- **Rust crate 直接可用**：`landlock` crate 可直接加入依赖；bwrap 需系统安装 + setuid
- **无需特权**：Landlock 不需 root/CAP_SYS_ADMIN，bwrap 需 setuid root
- **与 PermissionIntent 对齐**：Landlock 的 allow-list 语义与 `PermissionIntent` 完全一致
- **bwrap 保留为 P1 降级**：Landlock 需 Linux 5.13+，旧内核需 bwrap 降级

### ADR-3: 为什么不选 gVisor 作为默认方案

- **Go 语言不匹配**：本项目是 Rust 技术栈，gVisor 是 Go 实现
- **性能开销**：系统调用密集型 workload 有 2-10x 开销，桌面端不可接受
- **仅 Linux**：无法解决 Windows/macOS 隔离需求
- **架构过重**：用户态内核是"核武器级"隔离，日常 Agent 工具执行不需要如此强的隔离

### ADR-4: fail-closed 策略

借鉴 DeepSeek Harness：沙箱后端不可用时返回 `SandboxError::Unavailable`，由调用方决定是否降级为非沙箱执行（需用户显式确认），绝不静默无沙箱执行。
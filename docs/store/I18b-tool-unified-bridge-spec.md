# I18b ToolBox OS 交互桥接规格

> **本文档职责**：ToolBox 双入口桥接的**权威源**——os_tool_exec 统一 Tauri command、os.ts 前端代理、新增 OS 工具标准 5 步流程、fs_link 完整设计。
> **架构决策记录**见 [I18-agent-tool-architecture-design.md](./I18-agent-tool-architecture-design.md)（§3.6 双入口分流决策）。
> **实施规格**见 [I18-agent-tool-architecture-spec.md](./I18-agent-tool-architecture-spec.md)（Step 8 Tauri 桥接）。
> **Shell 安全架构**见 [OSCALLDESIGN.md](../designed/OSCALLDESIGN.md)。
>
> Agent 入口（RoundExecutor → ToolPipeline）已在 I18 中定义，本文档聚焦 ToolBox 侧桥接——用户主动调用的工具如何直达执行，与 Agent 入口共享 Tool trait 实现但分流执行路径。

> **实施状态**（2026-11-02 复核）：桥接已落地——`os_tool_exec` 使用 `find_for_toolbox()`，`os.ts` 与 `fs-link` 组件已存在，`fs_link` 已声明 `TOOLBOX_ONLY`。本文中 §2.3 / §2.4 / §3.2 / §4.1-4.6 已按**实际代码**校正；早期草稿的 `find()`、`original/link/linkType/targetType`、`open_world=false`、`CreateSymbolicLinkW` 均未落地，勿按旧内容实现。`fs_link` 的路径安全与 permission 词汇修正待 I31 T3。

---

## 1. 问题

当前 21 个 ToolBox 工具全部是纯前端实现（浏览器 API / fetch），没有任何工具调 Tauri `invoke()`。但新增需要 OS 能力的工具（如 fs_link）时，如果按"每工具一个 Tauri command"的方式，扩展性极差——每加一个工具就要堆 5-6 个文件。

I18 已定义完整的 Agent 执行架构：`RoundExecutor → ToolPipeline → (ToolGateway.evaluate + Tool.execute + ToolResultStorage.persist)`。但 **ToolBox 工具是用户主动点击使用的，不是 LLM 自主调用的**——用户已经通过点击表达了执行意图，ToolPipeline 的审批流、策略评估、审计日志对此场景均不适用。

ToolBox 入口需要一条**直达路径**：前端组件 → Tauri command → ToolRegistry.find_for_toolbox() → Tool.execute() → 返回结果。与 Agent 入口共享 Tool trait 实现（实现一次，两个入口可用），但执行路径分流。

---

## 2. 方案：统一桥接 + 注册式扩展

### 2.1 核心思路

**Rust 侧永远只有一个 `os_tool_exec` Tauri command**，不管有多少个 OS 工具。

`os_tool_exec` 是一个薄垫片，职责仅限于：接收前端参数 → 从 ToolRegistry 查找工具（`find_for_toolbox()`，即仅返回 `audience().toolbox == true` 的工具）→ 调用 Tool.execute() → 返回 ToolResult。不经过 ToolPipeline，不触发审批流。

新增工具只做三件事：
1. 实现 Tool trait（含显式 `audience()`）+ 注册到 ToolRegistry
2. 前端 `os.ts` 加一个命名空间
3. React 组件 + `defineTool()`

不需要：单独的 Tauri command、单独的 service 文件、cmd/mod.rs 注册。

### 2.2 双入口分流架构

两个入口共享 **Tool trait 实现**（实现一次，两个入口可用），但执行路径分流——Agent 经过 ToolPipeline 的完整安全基础设施，ToolBox 直达执行：

```
                    ┌──────────────────────┐
                    │   Tool trait 实现     │  共享：实现一次
                    │   (fs_link, fs_read…) │  两个入口可用
                    └──────────┬───────────┘
                               │
                    ┌──────────┴───────────┐
                    │    ToolRegistry       │  共享：注册一次
                    │    (查找工具)         │
                    └────┬────────────┬────┘
                         │            │
           ┌─────────────┘            └──────────────┐
           │                                         │
  ┌────────┴──────────────┐           ┌──────────────┴─────────┐
  │   ToolBox 入口         │           │   Agent 入口            │
  │   os_tool_exec         │           │   RoundExecutor         │
  │   Tauri command        │           │                        │
  │                        │           │   ↓ ToolPipeline        │
  │   ↓ ToolRegistry.find  │           │     → ToolGateway      │
  │   ↓ Tool.execute()     │           │       (权限评估+审批)   │
  │   ↓ 返回结果           │           │     → Tool.execute()   │
  │                        │           │     → ResultStorage     │
  │   直达执行              │           │       (审计+持久化)     │
  └────────────────────────┘           └────────────────────────┘
```

**为什么 ToolBox 不经过 ToolPipeline：**

| ToolPipeline 基础设施 | Agent 场景（LLM 自主调用） | ToolBox 场景（用户主动调用） |
|---|---|---|
| ToolGateway 审批流 | LLM 自主决策 → 需要人类审批 | 用户自己点的 → 审批自己无意义 |
| Permission Policy 评估 | 防止 LLM 越权 | 用户就是权限主体，无需策略评估 |
| 审计日志 (ResultStorage) | 追踪 AI 自主行为 | 用户操作，不是 AI 行为 |
| 超时保护 | 防止 LLM 发起的命令挂起 | UI 层自行处理加载/超时状态 |
| 结果回填消息历史 | Agent Loop 需要继续对话 | 结果直接返回组件渲染 |

路径校验（`validate_path()`）等安全约束是 Tool.execute() 内部逻辑，不依赖 ToolPipeline，两个入口均受保护。

### 2.3 `os_tool_exec` Tauri command 实现

请求 / 返回的 wire 类型定义在 `src-tauri/src/protocol/dto/os_tool.rs`（全仓唯一 DTO 落点，`*Dto` 后缀 + `rename_all = "camelCase"`）：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OsToolExecRequestDto {
    pub tool: String,
    pub action: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OsToolResultDto {
    pub ok: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
}
```

命令本体：

```rust
#[tauri::command]
pub async fn os_tool_exec(
    registry: State<'_, Arc<ToolRegistry>>,
    request: OsToolExecRequestDto,
) -> Result<OsToolResultDto, String> {
    let tool = registry
        .find_for_toolbox(&request.tool)
        .ok_or_else(|| format!("tool '{}' not found in registry", request.tool))?;

    let mut input = request.params;
    if let Some(obj) = input.as_object_mut() {
        obj.insert("action".to_string(), Value::String(request.action));
    } else {
        input = serde_json::json!({ "action": request.action });
    }

    let context = ToolUseContext::for_toolbox(None);
    let result = tool.execute(&input, &context).await.map_err(|e| e.to_string())?;
    Ok(OsToolResultDto::from(result))
}
```

关键：`os_tool_exec` 直接调 `Tool.execute()`，不经过 ToolPipeline。路径校验等安全约束由 Tool.execute() 内部保证，与 Agent 入口调同一个 execute() 时行为一致。

**已知待修**：`ToolUseContext::for_toolbox(None)` 使 `workspace_root` 恒为 `None`，相对路径工具在 ToolBox 入口下不可用，需补工作区来源（见 I18 spec 8.2）。

### 2.4 前端 `os` 代理对象

组件调 OS 能力和调 `crypto-js` 一样简单：

```typescript
import { os } from "@/tools/os";
const result = await os.fsLink.create({ source, target, kind });
```

> 参数命名以**实际实现**为准：`source` = 链接将被创建的位置，`target` = 已存在的被指向对象，`kind` = `symlink | hardlink | junction`。早期草稿使用的 `original / link / linkType / targetType` 未落地，勿按旧名调用。`fs_link` 为 `TOOLBOX_ONLY`，不进入 LLM 工具列表。

`os` 对象内部统一走 `invoke("os_tool_exec", { tool, action, params })`，每个 OS 工具一个命名空间。`tool` 字段为字符串字面量，直接匹配 Rust `Tool.name()` 返回值，Tauri invoke 自动处理序列化，无需运行时命名转换。

**OsResult 类型**：`{ ok: true; data: T } | { ok: false; error: string; code: string }`。

> **设计要求**：
> - `os.ts` 中每个工具命名空间的类型定义，可由 Rust 侧通过 `ts-rs` crate 自动导出，避免手写类型
> - `os_tool_exec` command 是唯一的 Tauri command 入口，禁止为单个 OS 工具新增独立 Tauri command
> - `os` 代理对象应支持 tree-shaking，未使用的工具命名空间不打入 bundle

---

## 3. 新增 OS 工具的标准流程

以 fs_link 为例，展示新增一个需要 OS 能力的工具的完整步骤。

### 3.1 步骤总览

| 步骤 | 位置 | 内容 | 是否必须 |
|------|------|------|---------|
| 1 | `assistant-tools/os/fs_link.rs` | 实现 Tool trait（含 `permission_intents` 与 `audience`） | ✓ 每个工具必做 |
| 2 | `assistant-tools/os/mod.rs` | 加一行 `mod fs_link;` + 注册到 Registry | ✓ 每个工具必做 |
| 3 | `src/tools/os.ts` | 加一个命名空间 + 类型 | ✓ 每个工具必做 |
| 4 | `src/tools/fs-link/index.tsx` | React 组件，调 `os.fsLink.xxx()` | ✓ 每个工具必做 |
| 5 | `src/tools/registry.ts` | 加一行 `defineTool()` | ✓ 每个工具必做 |
| 6 | `docs/store/I18x-xxx-spec.md` | 独立 spec 文档 | ✓ 每个工具必做 |

**对比纯前端工具只需步骤 4-5**，OS 工具多出步骤 1-3（Rust 实现 + 注册 + 代理），这是 OS 能力本身必须的，无法省略。

**受众必须先定**：步骤 1 的 `audience()` 决定该工具是否进入 LLM 的 `tools` 列表（`agent`）以及是否允许 `os_tool_exec` 按名调用（`toolbox`）。纯前端工具走 `src/tools/` 注册表，与 `ToolRegistry` 无关；OS 工具一旦注册即受 `toolbox` 维度约束，故**必须在步骤 2 之前明确受众**，避免「注册即暴露」的意外。

**不需要**：单独的 Tauri command、单独的 service 文件、cmd/mod.rs 注册。

### 3.2 Rust 侧：Tool trait 实现

在 `assistant-tools/os/` 下新增 `fs_link.rs`，实现 I18-spec Step 1 定义的 Tool trait：

- `name()` → `"fs_link"`
- `description()` → 供 LLM 理解的描述
- `input_schema()` → JSON Schema，定义 create / check 两个 action 的参数结构
- `tier()` → `ToolTier::Os`
- `audience()` → `ToolAudience::TOOLBOX_ONLY`（本工具服务「人点按钮」，不进 LLM 的 `tools` 列表；新增 Agent 工具时应为 `BOTH`，不得依赖默认值）
- `behavior_hints()` → 实际实现为 `BehaviorHints { read_only: false, destructive: false, open_world: true }`（创建链接会触及工作区外路径，故 `open_world = true`；早期草稿写 `false` 与实现不符，以代码为准）
- `permission_intents(input, context)` → **当前实际词汇**为 create action 的 `fs_link_create` / `fs_link_target`；与 I18 design §2.1 的 `read` / `edit` / `delete` / `execute` 不一致，属已知偏差，由 I31 T3 统一改为 `read`(target) + `edit`(source)
- `execute(input, context)` → 根据 input 中的 action 字段分发到 create / check 内部方法（内部包含 `validate_path()` 路径校验，两个入口均受保护）

在 `assistant-tools/os/mod.rs` 中加一行注册：
- `mod fs_link;`
- `registry.register(Arc::new(FsLinkTool::new()));`

### 3.3 前端侧：os 代理 + React 组件

**os.ts 新增命名空间**：

```typescript
export const os = {
  fsLink: {
    create: (params) => invoke("os_tool_exec", { tool: "fs_link", action: "create", params }),
    check:   (params) => invoke("os_tool_exec", { tool: "fs_link", action: "check", params }),
  },
  // 未来新增工具只在这里加一个命名空间
};
```

**React 组件**：和现有 21 个 ToolBox 工具完全一致的模式——自包含组件，内部直接调 `os.fsLink.create()`，不需要单独的 service 文件。

**registry.ts 新增注册**：一行 `defineTool()`，和现有工具无区别。

---

## 4. fs_link 工具详细设计

### 4.1 三种链接类型

| 类型 | Linux/macOS | Windows | 需要管理员？ |
|------|-------------|---------|------------|
| symlink（符号链接） | `unix::fs::symlink()` | `symlink_file()` / `symlink_dir()`（需特权或开发者模式） | Windows 默认需要 |
| hardlink（硬链接） | `fs::hard_link()` | `fs::hard_link()` | 不需要 |
| junction（目录联接） | — 不支持 | `cmd /c mklink /J`（**实际实现**，非 Win32 API） | **不需要** |

### 4.2 跨平台实现策略

**符号链接**：
- Unix：直接调用 `std::os::unix::fs::symlink()`，自动区分文件/目录
- Windows：对文件调 `std::os::windows::fs::symlink_file()`，对目录调 `symlink_dir()`
- Windows 无特权时：**实际实现直接报错**，不自动降级为 Junction（与早期设计不同，见 4.3）

**硬链接**：
- 跨平台统一：`std::fs::hard_link()`
- 限制：只能链接文件，不能链接目录；源和目标必须在同一文件系统

**Junction（仅 Windows）**：
- **实际实现**：`cmd /c mklink /J` 子进程，有进程开销但功能正确；未引入 `windows-sys`
- 不需要管理员特权
- 非 Windows 平台请求 junction 时返回 `UnsupportedLinkType`

### 4.3 特权检测（未实现）

早期设计为「尝试创建 + 回滚」+ `OnceCell<bool>` 缓存，**当前未实现**：Windows 无特权时直接返回错误，由用户自行开启开发者模式。

> 实施偏差以 I18-spec「fs_link 实施状态」为准；本节保留设计意图，不代表代码现状。

### 4.4 安全校验

| 校验项 | 规则 |
|--------|------|
| 路径校验 | `source`（链接位置）与 `target`（被指向对象）均须通过路径安全带；`source` 需保留叶子语义（不跟随已存在的链接），`target` 需宽松规范化（当前仍是 `fs_link` 私有实现，共享化见 I31 T3） |
| 类型校验 | symlink/hardlink 要求 `target` 必须存在；junction 要求 `target` 必须是目录 |
| 冲突校验 | `source` 路径已存在 → 拒绝，不自动覆盖 |
| 同文件系统 | hardlink 要求 `source` 与 `target` 在同一文件系统（由 OS 返回错误，不前置校验） |

### 4.5 Tool trait 声明

> 以下为**代码现状**；早期草稿的 `original` / `link` / `linkType` / `targetType` 字段名与 `open_world = false` 均已废弃。

- `name()` → `"fs_link"`
- `tier()` → `ToolTier::Os`
- `audience()` → `ToolAudience::TOOLBOX_ONLY`
- `behavior_hints()` → `BehaviorHints { read_only: false, destructive: false, open_world: true }`
- `permission_intents(input, context)` → 当前产出 `fs_link_create`（source）+ `fs_link_target`（target）；I31 T3 将统一为 `edit`(source) + `read`(target)
- `input_schema()` → 定义 create 和 check 两个 action

**create action 参数**（以 `input_schema` 实现为准）：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| action | "create" | ✓ | 动作标识 |
| source | string | ✓ | 链接路径（要创建的链接位置） |
| target | string | ✓ | 源路径（被链接指向的**已存在**文件/目录） |
| kind | "symlink" \| "hardlink" \| "junction" | ✓ | 链接类型 |

**check action 参数**：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| action | "check" | ✓ | 动作标识 |
| path | string | ✓ | 要检测的路径 |

**check action 返回**：

| 字段 | 类型 | 说明 |
|------|------|------|
| exists | bool | 路径是否存在 |
| is_link | bool | 是否为链接 |
| link_type | Option | 链接类型（symlink/hardlink/junction） |
| target | Option | 链接指向的目标路径 |

> **schema 描述已知问题**：当前 `input_schema` 中 `source` 的描述写成「existing file/dir for create; link path for check」，与实现语义相反（`target` 才是必须存在的对象）。`fs_link` 为 `TOOLBOX_ONLY` 不进 LLM，但该描述会误导任何按 schema 调用的人，I31 T3 迁移时一并修正。

### 4.6 Windows Junction 实现

**实际实现**为 `cmd /c mklink /J` 子进程，非 Win32 API：

- 有进程开销，但功能正确、错误信息可直接透出
- 未引入 `windows-sys`；`cfg!(windows)` 仅用于分支
- 非 Windows 平台请求 junction 类型时返回 `UnsupportedLinkType`

> 早期草稿的「`windows-sys` + `CreateSymbolicLinkW`」为设计意图，未落地；若后续需要精确错误码再切换。

### 4.7 错误码（设计意图，未实现）

> **代码现状**：`fs_link` 返回**自由文本错误字符串**（如 `symlink create failed: ...`），不存在错误码枚举。下表为设计意图；只有当前端需要按错误类型分支渲染时才引入枚举，避免为「未使用的结构化」付成本。

| 错误码 | 含义 |
|--------|------|
| PathBlocked | 路径被安全带拦截 |
| TargetNotFound | 被指向对象（`target`）不存在 |
| SourceAlreadyExists | 链接位置（`source`）已存在 |
| PrivilegeRequired | Windows 符号链接需要特权且无法降级 |
| UnsupportedLinkType | 当前平台不支持该链接类型（如 Linux 上的 junction） |
| CrossDevice | 硬链接跨文件系统 |
| OsError | 操作系统错误，附带原始错误信息 |

---

## 5. Crate 组织变更

在 I18-design §6 的基础上新增。`os_tool_exec` Tauri command 位于应用命令层（`src-tauri/src/cmd/`），而非 `assistant-tools` 库 crate 内——库 crate 不依赖 Tauri runtime。

```
src-tauri/
├── src/
│   └── cmd/
│       └── os_tool_exec.rs   # 新增：唯一 Tauri command 入口（直达执行，不经过 ToolPipeline）
├── crates/
│   └── assistant-tools/
│       └── src/
│           ├── os/
│           │   ├── mod.rs
│           │   ├── fs_link.rs   # 新增
│           │   └── ...          # I18 已定义的 OS 工具
│           └── ...
```

前端侧新增：

```
src/tools/
├── os.ts                  # 新增：OS 工具统一代理（invoke 桥接 + 类型定义）
├── fs-link/               # 新增：fs_link 工具组件
│   └── index.tsx
└── ...                    # 现有 21 个纯前端工具不变
```

> **设计要求**：
> - `cmd/os_tool_exec.rs` 是唯一的 Tauri command，禁止为单个工具新增独立 command
> - `os_tool_exec` 直达执行：ToolRegistry.find_for_toolbox() → Tool.execute()，不经过 ToolPipeline
> - 前端 `os.ts` 是所有 OS 工具的类型安全入口，新增工具加一个命名空间即可
> - `os/` 目录下每个工具一个文件，工具清单及接口由独立 spec 文档规定
> - `os.ts` 命名空间用 camelCase（`os.fsLink`），`tool` 字段为字符串字面量直接匹配 Rust `Tool.name()`（`"fs_link"`），Tauri invoke 自动处理序列化

---

## 6. 与 I18 的关系

| 关注点 | I18（架构） | 本文档（ToolBox 桥接） |
|--------|------------|----------------------|
| Tool trait 定义 | ✓ 定义 | **共享**——实现一次，两个入口可用 |
| ToolRegistry | ✓ 定义 | **共享**——注册一次，两个入口可查 |
| ToolPipeline | ✓ 定义 | 不经过——ToolBox 是用户主动调用，无需审批/审计/策略评估 |
| ToolGateway（权限网关） | ✓ 定义 | 不经过——用户就是权限主体 |
| RoundExecutor | ✓ 定义 | 不涉及（Agent 专用入口） |
| Tool.execute() 内部安全约束 | ✓ 定义 | **共享**——路径校验等在 execute() 内部，两个入口均受保护 |
| ToolBox ↔ Rust 桥接 | ✗ | ✓ 本文档核心 |
| os_tool_exec Tauri command | ✗ | ✓ 本文档核心（直达执行） |
| os.ts 代理对象 | ✗ | ✓ 本文档核心 |
| 新增工具标准流程 | ✗ | ✓ 本文档核心 |
| 具体工具设计 | ✗ | ✓ fs_link 为例 |

I18 定义"Agent 怎么安全地执行"（ToolPipeline + ToolGateway），本文档定义"ToolBox 怎么直达执行"（os_tool_exec → ToolRegistry → Tool.execute()）。两个入口共享 Tool trait 实现和 ToolRegistry，但执行路径分流——Agent 经过完整安全基础设施，ToolBox 直达执行。安全约束（路径校验等）由 Tool.execute() 内部保证，不依赖 ToolPipeline。
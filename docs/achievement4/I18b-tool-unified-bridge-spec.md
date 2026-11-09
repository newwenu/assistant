# I18b ToolBox OS 交互桥接规格

> **本文档职责**：ToolBox 双入口桥接的**权威源**——os_tool_exec 统一 Tauri command、os.ts 前端代理、新增 OS 工具标准 5 步流程、fs_link 完整设计。
> **阶段与状态**见 [I38-agent-roadmap.md](./I38-agent-roadmap.md)（P0/P1/P6）。
> **架构决策记录**见 [I18-agent-tool-architecture-design.md](./I18-agent-tool-architecture-design.md)（§3.6 双入口分流决策）。
> **框架规格**见 [I18-agent-tool-architecture-spec.md](./I18-agent-tool-architecture-spec.md) Step 8；**工具能力**见 [I38c](./I38c-os-tools-spec.md)。
> **Shell 安全架构**见 [OSCALLDESIGN.md](../designed/OSCALLDESIGN.md)。
>
> Agent 入口（RoundExecutor → ToolPipeline）见 I38b（已归档）与 [I38a](./I38a-tool-control-layer-spec.md)，本文档聚焦 ToolBox 侧桥接——用户主动调用的工具如何直达执行，与 Agent 入口共享 Tool trait 实现但分流执行路径。

> **实施状态**（2026-11-04 复核）：桥接已落地——`os_tool_exec` 使用 `find_for_toolbox()` 并走 `exec::invoke`（`DEFAULT_TOOL_TIMEOUT = 30s`），`os.ts` 与 `fs-link` 组件已存在，`fs_link` 已声明 `TOOLBOX_ONLY`。本文中 §2.4 / §3.2 / §4.1-4.6 已按**实际代码**校正；早期草稿的 `find()`、`original/link/linkType/targetType`、`open_world=false`、`CreateSymbolicLinkW` 均未落地，勿按旧内容实现。`fs_link` 的路径安全已迁至共享 `os::path`（I31 T3 完成）；ToolBox 入口不做 `workspace_root` 包含性检查，属设计而非缺陷（§2.2）。

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
                    │   (fs_link, read…)    │  两个入口可用
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
| 超时保护 | 防止 LLM 发起的命令挂起 | 共享 Rust 侧 `DEFAULT_TOOL_TIMEOUT` 兜底（`exec::invoke`），前端只维护加载态 |
| 结果回填消息历史 | Agent Loop 需要继续对话 | 结果直接返回组件渲染 |
| 输入校验（`validate_input`） | Pipeline 的校验阶段，区分可修正错误与不可改写不变量 | 不经过 Pipeline，无校验阶段；不变量由 `execute()` 内部判定 |

安全约束位于 `Tool.execute()` 内部，不依赖 ToolPipeline。但**两个入口生效的约束集合并不相同**，必须区分两类不变量：

| 类别 | 内容 | 保护谁 | Agent 入口 | ToolBox 入口 |
|---|---|---|---|---|
| **绝对不变量** | `..` 穿越拒绝、盘符相对路径拒绝、规范化失败不回退、受保护系统目录与凭据目录拒绝、不自动覆盖已存在目标 | 防误操作与工具自身缺陷，与调用者身份无关 | 生效 | **同样生效** |
| **作用域不变量** | `workspace_root` 包含性（`ensure_within_root`） | 防「权限小于用户的行动者」越权 | 生效 | **不适用** |

**ToolBox 没有「越界」一说**。ToolBox 的调用者是用户本人，用户就是权限主体：把 `workspace_root` 当作边界施加给他，只会阻止他操作工作区之外的文件——而这恰恰是他在 ToolBox 里想做的事。ToolBox 的真实边界是操作系统的账户 ACL，不是工作区。

因此 §2.3 中 `ToolUseContext::for_toolbox(None)` 让 `workspace_root` 恒为 `None`，是**语义正确的选择**：ToolBox 只接受绝对路径，且不做包含性检查。将来若要补相对路径支持，**不得**直接把 `workspace_root` 填进 ToolBox 上下文——那会让 `ensure_within_root` 连带生效，凭空造出一条越界规则；必须把「相对路径解析基准」与「权限边界」拆成两个独立字段（基准与边界分离），ToolBox 只填基准。

**`shell` 同理，且更严格**：它的 SafetyBelt、结构守卫、环境隔离、命令分类全部位于 `execute()` 内部（执行层不变量），ToolBox 直接调用也照样受保护；只有 AllowlistGate / ApprovalGate 这层审批留在 Pipeline，ToolBox 不经过。守卫分层归属见 [OSCALLDESIGN.md](../designed/OSCALLDESIGN.md) §3.3–§3.4。

**ToolBox 永远不涉及审批**：用户点击即执行意图的表达，不存在「谁来批准」的问题。Pipeline 的 `validate_input` 阶段服务于 Agent 入口——它要在弹审批框之前先排除非法输入；ToolBox 没有审批框，因此不需要这个阶段。两者共享的底线是**绝对不变量不可绕过**，由 `execute()` 内部的 `os::path::resolve()` / `resolve_leaf_preserving()` 保证，而不是靠 Pipeline 关卡；作用域不变量只对 Agent 入口生效。

> 因此 `validate_input` 与 `execute()` 内部的路径校验必须**复用同一套判定函数**——复用的是绝对不变量的判定，不包含边界判定。目标是同一条路径在两侧得到相同的前置校验结果，同时确保边界判定不会被误带进 ToolBox。详见 [I38a](I38a-tool-control-layer-spec.md) §3.1。

### 2.3 `os_tool_exec` Tauri command 实现

请求 / 返回的 wire 类型定义在 `src-tauri/src/protocol/dto/os_tool.rs`（全仓唯一 DTO 落点，`*Dto` 后缀 + `rename_all = "camelCase"`）。**本节不复述字段**，字段以该文件为准；前端类型由 `npm run contract:gen` 生成，不要照文档手写。

| DTO | 字段 | 说明 |
|---|---|---|
| `OsToolExecRequestDto` | `tool` / `action` / `params` | 输入，`deny_unknown_fields` |
| `OsToolResultDto` | `ok` / `data` / `resultForAssistant` / `error` / `errorKind` | `resultForAssistant` 与 `errorKind` 由 P2 控制层追加（[I38a §3.2](./I38a-tool-control-layer-spec.md)） |

命令本体：

```rust
use assistant_tools::{invoke, ToolRegistry, ToolUseContext, DEFAULT_TOOL_TIMEOUT};

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

    let context = ToolUseContext::for_toolbox(workspace_root_from_conf());
    let result = invoke(tool.as_ref(), &input, &context, DEFAULT_TOOL_TIMEOUT).await;
    Ok(OsToolResultDto::from(result))
}
```

关键：`os_tool_exec` 通过 `exec::invoke` 调 `Tool.execute()`，不经过 ToolPipeline。`invoke` 负责超时（`DEFAULT_TOOL_TIMEOUT = 30s`）与 `Err` 通道归一化；**绝对不变量**（`..` 穿越、盘符相对路径、系统目录、规范化不回退）由 `execute()` 内部保证，两个入口一致。`shell` 的 SafetyBelt 与纵深防御也在 `execute()` 内，因此 ToolBox 调用它同样受保护——缺的只是 AllowlistGate / ApprovalGate 这层审批。

**与 Agent 入口唯一不一致的地方是作用域检查**：`for_toolbox(...)` 只填 `resolve_base`、`workspace_root` 恒为 `None`，`ensure_within_root` 整段跳过。这是设计而非缺陷——ToolBox 无「越界」概念（§2.2）。代价原本是相对路径不可用；按「解析基准 / 权限边界分离」补上 `resolve_base` 后，ToolBox 相对路径可用且仍不产生越界规则。

**错误文案只陈述事实，不给调用策略**：工具错误会原样进入模型上下文，因此路径类错误**禁止**携带「改用绝对路径」「换个目录」这类建议——那等于教模型去试探边界。示例：相对路径无解析基准时只报 `relative path '<x>' cannot be resolved: no workspace directory is configured`，不追加替代做法；`..` 穿越只报 `contains '..' traversal, which is not allowed`。这与 `grep` footer「只给事实」的口径一致（[I38c-grep-spec](./I38c-grep-spec.md) §117），仅编码类错误例外（那里提示 `encoding` 是产品决定，见 [I38c §634](./I38c-os-tools-spec.md)）。

### 2.4 前端 `os` 代理对象

组件调 OS 能力和调 `crypto-js` 一样简单：

```typescript
import { os } from "@/tools/os";
const result = await os.fsLink.create({ source, target, kind });
```

> 参数命名以**实际实现**为准：`source` = 链接将被创建的位置，`target` = 已存在的被指向对象，`kind` = `symlink | hardlink | junction`。早期草稿使用的 `original / link / linkType / targetType` 未落地，勿按旧名调用。`fs_link` 为 `TOOLBOX_ONLY`，不进入 LLM 工具列表。

`os` 对象内部统一走 `invoke("os_tool_exec", { tool, action, params })`，每个 OS 工具一个命名空间。`tool` 字段为字符串字面量，必须与 Rust `Tool::name()` 返回值**逐字一致**，Tauri invoke 自动处理序列化，不做任何运行时命名转换（没有 snake_case → camelCase 的映射）。

**两套命名，职责不同**：

| 层 | 名字 | 约束 | 是否进 wire |
|---|---|---|---|
| Rust | `Tool::name()` / 文件名 | 注册名为小写 snake_case 纯动作词（`read` / `write` / `ask_user`）；文件名为 `agent_` 前缀（`agent_read.rs`），见 I38c §2 | 是（`tool` 字面量） |
| 前端 | `os.ts` 命名空间属性 | camelCase 仅是 TS 本地风格；注册名单词时与注册名同名（`os.read`），多词时驼峰（`os.askUser`） | 否（只影响调用写法） |

因此 `os.fsRead` 这类「Rust `fs_read` ↔ 前端 `fsRead`」的双向转换不再需要：注册名本身就是 `read`，前端直接 `os.read(...)` + `tool: "read"`。

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
- `audience()` → `ToolAudience::TOOLBOX_ONLY`（本工具服务「人点按钮」，不进 LLM 的 `tools` 列表；新增 Agent 工具时应显式声明 `BOTH` / `AGENT_ONLY`，默认值为 `NONE`）
- `behavior_hints()` → 实际实现为 `BehaviorHints { read_only: false, destructive: false, open_world: true }`（创建链接会触及工作区外路径，故 `open_world = true`；早期草稿写 `false` 与实现不符，以代码为准）
- `permission_intents(input, context)` → `create` 产出 `edit`(source) + `edit`(target)，`check` 返回空 `Vec`；词汇与 I18 design §2.1 的 `read` / `edit` / `delete` / `execute` 对齐（I31 T3 已统一）
- `execute(input, context)` → 根据 input 中的 action 字段分发到 create / check 内部方法（内部包含 `resolve_leaf_preserving` 路径校验；绝对不变量两个入口一致，作用域检查仅 Agent 入口生效，见 §2.2）

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
| 路径校验 | `source`（链接位置）与 `target`（被指向对象）均走 `os::path::resolve_leaf_preserving`：父目录规范化、叶子保留（不跟随已存在的链接），拒绝 `..` 穿越、盘符相对路径与受保护系统目录。**不做 `workspace_root` 包含性检查**——ToolBox 无越界概念（§2.2） |
| 类型校验 | symlink/hardlink 要求 `target` 必须存在；junction 要求 `target` 必须是目录 |
| 冲突校验 | `source` 路径已存在 → 拒绝，不自动覆盖 |
| 同文件系统 | hardlink 要求 `source` 与 `target` 在同一文件系统（由 OS 返回错误，不前置校验） |

### 4.5 Tool trait 声明

> 以下为**代码现状**；早期草稿的 `original` / `link` / `linkType` / `targetType` 字段名与 `open_world = false` 均已废弃。

- `name()` → `"fs_link"`
- `tier()` → `ToolTier::Os`
- `audience()` → `ToolAudience::TOOLBOX_ONLY`
- `behavior_hints()` → `BehaviorHints { read_only: false, destructive: false, open_world: true }`
- `permission_intents(input, context)` → `create` 产出 `edit`(source) + `edit`(target)；`check` 返回空 `Vec`（只读，不产生审批项）。该意图对 ToolBox 入口无消费方——ToolBox 不经过 Gateway
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
> - `os/` 目录下每个工具至少一个文件（复杂工具可拆为多文件，如 `fs_link` 按职责分文件），工具清单及接口由独立 spec 文档规定
> - `os.ts` 命名空间用 camelCase（`os.fsLink`），`tool` 字段为字符串字面量直接匹配 Rust `Tool::name()`（`"fs_link"`），Tauri invoke 自动处理序列化
> - P5 新增的 agent 工具注册名为纯动作词，前端命名空间与注册名同名（`os.read` / `os.ls` / `os.glob` / `os.grep` / `os.write` / `os.edit` / `os.mkdir` / `os.move` / `os.delete` / `os.shell`，`tool` 字面量分别为 `"read"` … `"shell"`）；仅 `ask_user` 为多词，前端写 `os.askUser`，`tool` 仍为 `"ask_user"`

---

## 6. 与 I18 的关系

| 关注点 | I18（架构） | 本文档（ToolBox 桥接） |
|--------|------------|----------------------|
| Tool trait 定义 | ✓ 定义 | **共享**——实现一次，两个入口可用 |
| ToolRegistry | ✓ 定义 | **共享**——注册一次，两个入口可查 |
| ToolPipeline | ✓ 定义 | 不经过——ToolBox 是用户主动调用，无需审批/审计/策略评估 |
| ToolGateway（权限网关） | ✓ 定义 | 不经过——用户就是权限主体 |
| RoundExecutor | ✓ 定义 | 不涉及（Agent 专用入口） |
| Tool.execute() 内部安全约束 | ✓ 定义 | **共享**——绝对不变量（穿越、系统目录、Shell 执行层守卫等）在 execute() 内部，两个入口均生效；作用域不变量（`workspace_root` 包含性）仅 Agent 入口生效 |
| ToolBox ↔ Rust 桥接 | ✗ | ✓ 本文档核心 |
| os_tool_exec Tauri command | ✗ | ✓ 本文档核心（直达执行） |
| os.ts 代理对象 | ✗ | ✓ 本文档核心 |
| 新增工具标准流程 | ✗ | ✓ 本文档核心 |
| 具体工具设计 | ✗ | ✓ fs_link 为例 |

I18 定义"Agent 怎么安全地执行"（ToolPipeline + ToolGateway），本文档定义"ToolBox 怎么直达执行"（os_tool_exec → ToolRegistry → Tool.execute()）。两个入口共享 Tool trait 实现和 ToolRegistry，但执行路径分流——Agent 经过完整安全基础设施，ToolBox 直达执行。Tool.execute() 内部保证的是**绝对不变量**（路径穿越、系统目录、执行层守卫），这部分两个入口共享；**作用域不变量**（工作区包含性）只对 Agent 生效，ToolBox 不存在「越界」（§2.2）。
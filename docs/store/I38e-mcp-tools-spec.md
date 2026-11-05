# I38e — P7：MCP 外部工具规格

| 项 | 值 |
|---|---|
| 状态 | 现行（P7 规格；进度见 [roadmap](./I38-agent-roadmap.md) §2） |
| 日期 | 2026-11-04 |
| 阶段 | P7（见 [I38 roadmap](./I38-agent-roadmap.md)） |
| 关联 | [I18 design](./I18-agent-tool-architecture-design.md) §3.8 / §4.3、[I18 spec](./I18-agent-tool-architecture-spec.md)（框架层）、[I38a](./I38a-tool-control-layer-spec.md)（Policy/Gateway/Pipeline）、[I38c](./I38c-os-tools-spec.md)（工具实现范式）、[I36 spec](./I36-agent-runtime-spec.md)（超时与取消） |
| 影响范围 | `assistant-tools/src/mcp/`（桥接）、`src-tauri/src/mcp/`（进程与配置）、`setup.rs`、Agent 模式配置 |

> 本文是 P7 的唯一权威规格；I18 design §4.3 的隔离边界声明与 §3.8 的安全标记推断在本文落地为契约。
> **门禁**：P2（Policy/Gateway/Pipeline）与 P5（至少一组内置工具）完成后才可开工。

---

## 1 目标与非目标

### 1.1 目标

| # | 交付 |
|---|---|
| 1 | MCP 客户端（stdio transport）与 server 生命周期管理 |
| 2 | 工具发现与动态注册（`mcp__{server}__{tool}`） |
| 3 | 安全标记从 annotations 保守推断 |
| 4 | 复用 P2 的 Policy / Gateway / Pipeline 与 P5 的超时、输出预算 |
| 5 | server 启停、重连、失败降级；不影响主应用与内置工具 |

### 1.2 非目标

- HTTP / SSE transport（v1 只做 stdio）。
- cgroup / seccomp / namespace 级 OS 隔离。
- MCP resources / prompts / sampling 等非 tools 能力。
- MCP 工具的专用 UI；它们只进入 Agent Loop，不进入 ToolBox 透传入口。
- 跨进程后台存活（I36 design D13 取舍）。

### 1.3 前置

1. P2 完成：`Policy` / `Gateway` / `Pipeline` / `ApprovalKey` 可用。
2. P5 完成至少只读组，且 `os/path.rs`、`exec::invoke`、输出预算稳定。
3. P4 完成：RoundExecutor 能通过 `ToolCatalog` 获取动态工具定义。

---

## 2 架构边界

```
Agent Loop (P4)
   │  ToolCatalog（端口）
   ▼
ToolRegistry
   ├── 内置工具（P5，ToolTier::Os）
   └── McpTool 桥接（ToolTier::External）
          │
          ▼
   McpServerManager（src-tauri/src/mcp/）
          │  stdio
          ▼
   MCP server 子进程
```

**依赖方向**：

| 组件 | 位置 | 说明 |
|---|---|---|
| `McpTool` / 命名与标记推断 | `assistant-tools/src/mcp/bridge.rs` | 纯 Rust，依赖 MCP 协议类型，不依赖 tauri |
| MCP 客户端 | `assistant-tools/src/mcp/client.rs` | stdio 读写；不管理进程 |
| 进程与重连 | `src-tauri/src/mcp/server_manager.rs` | 依赖 `tokio::process` / `AppHandle`；动态注册到 `ToolRegistry` |
| 配置 | `src-tauri` | 见 §6 开放问题 |

**隔离边界（必须写进用户可见文案）**：

> MCP server 以独立子进程运行，但当前**不提供** cgroup / seccomp / namespace 级 OS 隔离。
> 权限策略对 External 层的作用是逻辑层控制（决定是否调用），**不是**执行层隔离（限制调用后的行为）。
> 禁用某 server 时必须**拒绝调用**，不得宣称「沙箱内安全执行」。

---

## 3 工具桥接契约

### 3.1 命名

```
mcp__{server}__{tool}
```

- `server` / `tool` 中的非 `[a-zA-Z0-9_-]` 字符替换为 `_`；重名时追加短哈希。
- 名称是稳定标识：server 重连后同名工具保持同一 `name`，避免模型上下文漂移。
- 展示名可用原始名称，但 LLM `tools` 列表使用规范化名。

### 3.2 Tool trait 映射

| Tool 方法 | McpTool 实现 |
|---|---|
| `name()` | `mcp__{server}__{tool}` |
| `description()` | MCP `description`，缺省为空串 |
| `input_schema()` | MCP `inputSchema`，原样透传 |
| `tier()` | `ToolTier::External` |
| `audience()` | `AGENT_ONLY`（不进入 ToolBox 透传入口） |
| `behavior_hints()` | 见 §3.3 |
| `permission_intents()` | `external` + `mcp:{server}`，资源为 server 名 |
| `execute()` | 发 `tools/call`；结果映射为 `ToolResult` |

### 3.3 安全标记推断（保守默认）

| 字段 | 规则 |
|---|---|
| `read_only` | 采纳 MCP `annotations.readOnlyHint`；无声明 → `false` |
| `destructive` | `declared_destructive || !declared_read_only` |
| `open_world` | 同 `destructive` 逻辑 |
| 无 annotations | `destructive = true`、`open_world = true` |

**原则**：MCP 自报只能升级（更严），不能降级。`read_only` 声明可采纳，但只读工具仍经 Policy 评估，不自动 `Allow`。

### 3.4 结果映射

| MCP 结果 | `ToolResult` |
|---|---|
| `content` 文本块 | 拼接为 `data`（结构化数组）+ `result_for_assistant`（文本拼接） |
| `isError = true` | `ok = false`，`error_kind = Business` |
| 超时 | `error_kind = Timeout`（由 `exec::invoke` 打标） |
| server 断开 | `error_kind = Internal`，提示「MCP server 不可用」 |
| 工具不存在 | `error_kind = Business`，提示「工具未在 server 注册」 |
| 图片 / 资源块 | v1 不采纳；作为 `data` 原样返回，不进入模型文本通道 |

大结果复用 P2 的三层预算与落盘；MCP 不单独实现截断。

---

## 4 生命周期

### 4.1 状态机

```
Stopped → Starting → Ready → Error → Stopped
                ↑                 │
                └──── Retry ──────┘
```

| 状态 | 行为 |
|---|---|
| Stopped | 无进程；工具不注册 |
| Starting | 启动子进程、握手 `initialize`、`tools/list` |
| Ready | 工具已注册；可调用 |
| Error | 记录原因；工具保留但调用返回 `Internal`；按退避重试 |
| Stopped（用户禁用） | 杀掉进程、注销工具；不重试 |

### 4.2 启动与重连

1. `setup.rs` 在 DB / Registry 初始化后，启动已启用的 server；失败只记日志，不阻塞启动。
2. 握手超时 30s；`tools/list` 结果缓存为工具定义快照。
3. 断线重连：指数退避（1s / 2s / 4s / 8s，上限 60s），最多 5 次；连续失败进入 `Error`，等待用户手动重启。
4. 重连成功后重新 `tools/list`；名称不变的工具保留已批准的 `AlwaysApprove` 键，名称变化的工具视为新工具。
5. 用户禁用 server：先注销工具再杀进程，确保无「已注销但仍在跑」的窗口。

### 4.3 调用超时与取消

- 单次调用默认 30s，可在 server 配置中覆盖；受 turn 剩余预算约束。
- 取消：中止本次请求并返回 `Cancelled`；**不**杀 server 进程（避免影响其它并发调用）。
- turn 取消（用户停止）时，所有在途 MCP 调用随 child token 取消。

---

## 5 注册与可用性

| 层 | 规则 |
|---|---|
| ToolRegistry | server `Ready` 后动态注册；`Stopped` / `Error` 时保留或注销（见 §4.1） |
| `tool_definitions_for_tier(External)` | 仅当 Agent 模式 / 对话配置的 `maxToolTier = external` 时返回 |
| 白名单 | 仍受 `ConversationToolConfig.enabledTools` deny-by-default 控制 |
| 禁用 server | 工具从 Registry 注销，且 Policy 对 `external` + 该 server 记录 `Deny` |
| 审计 | 走 P2 的 `PermissionAuditStore`；不新建表 |

**硬约束**：不得因为「server 已连接」就自动把工具暴露给所有模式；必须同时满足 tier、受众、白名单三条。

---

## 6 配置（待 P7 开工前定稿）

以下为候选方案，本文只登记，不在 P7 前实现：

| 方案 | 优点 | 风险 |
|---|---|---|
| `AppConf` 内 `mcpServers` 数组 | 复用配置加载与 UI | 环境变量 / 凭据可能进入明文配置文件 |
| 新建 `mcp_servers` 表 | 可做凭据加密、启停状态 | 新迁移、新命令、新 UI 面 |
| 独立 `mcp.json`（项目级） | 贴近主流 MCP 配置 | 与 AppConf 双源，需要加载优先级 |

**已确定的约束**：

1. 凭据不得写入明文配置；需要密钥时复用 provider / vault 的既有加密路径。
2. server 默认禁用；启用是显式动作。
3. 配置变更只影响新会话，不回溯已创建会话（与 AgentMode 语义一致：对话级配置仅存在于内存，新对话重新初始化）。

---

## 7 实施步骤

| 步骤 | 内容 | 依赖 |
|---|---|---|
| P7-C1 | `McpTool` + 命名 / 标记推断 / 结果映射（纯 Rust）+ 单测 | P2、P5 |
| P7-C2 | stdio 客户端：`initialize` / `tools/list` / `tools/call` / 错误与超时 | P7-C1 |
| P7-C3 | `McpServerManager`：状态机、启停、重连、动态注册 / 注销 | P7-C2 |
| P7-C4 | `setup.rs` 启动序列；配置加载；禁用 / 启用命令 | P7-C3 |
| P7-C5 | 端到端：mock server 三工具（只读 / 破坏性 / 失败）跑通 Agent Loop | P4、P7-C4 |
| P7-C6 | 隔离边界文案与设置页提示（不含 MCP 专用 UI 交互） | P7-C5 |

---

## 8 验收标准

| # | 标准 |
|---|---|
| A1 | mock stdio server 可启动、握手、发现工具、调用成功；进程退出后状态收敛为 `Error` 并按退避重试 |
| A2 | 工具名 `mcp__{server}__{tool}` 稳定；重连后同名工具保留，改名工具视为新工具 |
| A3 | 无 annotations 的 MCP 工具 `destructive = true`、`open_world = true`；`readOnlyHint` 采纳但不自动 Allow |
| A4 | 只有 `maxToolTier = external` + 白名单命中的工具进入 LLM 列表；ToolBox 入口不可达（`AGENT_ONLY`） |
| A5 | 禁用 server 后调用返回 `Denied` / `Internal`，且进程不再运行 |
| A6 | 调用超时返回 `Timeout`；取消返回 `Cancelled`，server 进程仍存活 |
| A7 | 大结果按 P2 预算落盘；MCP 不单独截断 |
| A8 | `cargo test -p assistant-tools` / `cargo test --test assembly_contract` 通过；`assistant-tools` 无 tauri 依赖 |

---

## 9 风险与开放问题

| 风险 | 缓解 |
|---|---|
| 误把逻辑权限当执行隔离 | §2 文案硬约束；设置页展示提示 |
| server 崩溃拖垮主应用 | 独立子进程 + 启动失败只记日志；调用失败归一化 |
| 工具名漂移破坏 prompt 缓存 | 规范化命名 + 重连保留同名 |
| 重连注册竞态 | 注销先于杀进程；注册在 `Ready` 后单点完成 |
| 凭据进入明文配置 | §6 约束 1；P7 开工前定稿配置方案 |

**开放问题**：

1. 配置落点（§6 三选一）。
2. `mcp_servers` 启停状态是否需要持久化到 DB（还是每次启动按配置启动）？
3. `AlwaysApprove` 是否跨 server 重连保留？当前倾向：按工具名 + 规范化参数保留，server 改名则失效。
4. 是否需要 `resources` / `prompts`：v1 不做，后续按真实需求解冻。
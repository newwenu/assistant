# I38e — P7：MCP 外部工具规格

| 项 | 值 |
|---|---|
| 状态 | 现行（P7 规格；进度见 [roadmap](./I38-agent-roadmap.md) §2） |
| 日期 | 2026-11-04（2026-11-09 定稿：配置落点、启停持久化、MCP 审批键、resources/prompts 结论；工具名改用稳定 server id） |
| 阶段 | P7（见 [I38 roadmap](./I38-agent-roadmap.md)） |
| 关联 | [I18 design](./I18-agent-tool-architecture-design.md) §3.8 / §4.3、I18 spec（已归档）（框架层）、I38a（已归档）（Policy/Gateway/Pipeline/ApprovalKey）、I38c（已归档）（工具实现范式）、I36 spec（已归档；超时与取消） |
| 影响范围 | `assistant-tools/src/mcp/`（桥接）、`assistant-tools/src/gateway.rs`（`ApprovalScope::Mcp`）、`assistant-storage/src/db.rs`（`mcp_servers` 表）、`src-tauri/src/mcp/`（进程与配置）、`src-tauri/src/adapters/`（配置加解密）、`tool_setup.rs`、设置页最小 server 配置面 |

> 本文是 P7 的唯一权威规格；I18 design §4.3 的隔离边界声明与 §3.8 的安全标记推断在本文落地为契约。
> **门禁**：P2（Policy/Gateway/Pipeline）与 P5（至少一组内置工具）完成后才可开工。

---

## 1 目标与非目标

### 1.1 目标

| # | 交付 |
|---|---|
| 1 | MCP 客户端（stdio transport）与 server 生命周期管理 |
| 2 | 工具发现与动态注册（`mcp__{server_id}__{tool}`） |
| 3 | 安全标记从 annotations 保守推断 |
| 4 | 复用 P2 的 Policy / Gateway / Pipeline 与 P5 的超时、输出预算 |
| 5 | server 启停、重连、失败降级；不影响主应用与内置工具 |
| 6 | server 配置持久化：`mcp_servers` 表、凭据加密、`enabled` 跨重启保留 |

### 1.2 非目标

- HTTP / SSE transport（v1 只做 stdio）。
- cgroup / seccomp / namespace 级 OS 隔离。
- MCP resources / prompts / sampling 等非 tools 能力。
- MCP **工具卡片**的专用 UI；工具只进入 Agent Loop，不进入 ToolBox 透传入口。server 配置页（列表 / 开关 / 编辑）属 P7 最小必要面。
- `mcp.json` / 项目级配置作为运行时第二源；后续只允许作为**一次性导入格式**。
- 跨进程后台存活（I36 design D13 取舍）。

### 1.3 前置

1. P2 完成：`Policy` / `Gateway` / `Pipeline` / `ApprovalKey` 可用。
2. P5 完成至少只读组，且 `os/path.rs`、`exec::invoke`、输出预算稳定。
3. P4 / P5 完成：`TurnSpec.tools` 注入与 `resolve_available_tools`（tier + 受众 + 白名单）稳定；MCP 工具在每轮发送前按该口径解析，不走已删除的 `ToolCatalog` trait。

---

## 2 架构边界

```
Agent Loop (P4)
   │  TurnSpec.tools（每轮按 resolve_available_tools 解析）
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
| 配置 | `src-tauri` + `assistant-storage` | `mcp_servers` 表持久化；凭据加密；见 §6 定稿 |

**隔离边界（必须写进用户可见文案）**：

> MCP server 以独立子进程运行，但当前**不提供** cgroup / seccomp / namespace 级 OS 隔离。
> 权限策略对 External 层的作用是逻辑层控制（决定是否调用），**不是**执行层隔离（限制调用后的行为）。
> 禁用某 server 时必须**拒绝调用**，不得宣称「沙箱内安全执行」。

---

## 3 工具桥接契约

### 3.1 命名

```
mcp__{server_id}__{tool}
```

- `server_id` = `mcp_servers.id`，**不可变**（创建后禁止修改）；`name` 只是展示名，可改。
- `server_id` / `tool` 中的非 `[a-zA-Z0-9_-]` 字符替换为 `_`；同一 server 内规范化后重名时追加短哈希。
- 名称是稳定标识：server 重连、应用重启、展示名变更都不改变 `name`；只有 `server_id` 或 MCP 工具名变化才视为新工具（避免模型上下文漂移）。
- 展示名 = `{server display name} / {原始 tool name}`；LLM `tools` 列表使用规范化名，UI 可显示展示名。

### 3.2 Tool trait 映射

| Tool 方法 | McpTool 实现 |
|---|---|
| `name()` | `mcp__{server_id}__{tool}` |
| `description()` | MCP `description`，缺省为空串 |
| `input_schema()` | MCP `inputSchema`，原样透传 |
| `tier()` | `ToolTier::External` |
| `audience()` | `AGENT_ONLY`（不进入 ToolBox 透传入口） |
| `behavior_hints()` | 见 §3.3 |
| `permission_intents()` | `("mcp", "mcp:{server_id}:{config_hash}")`；审批键另含规范化 args 指纹，见 §3.5 |
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

### 3.5 审批键与授权作用域（2026-11-09 定稿）

MCP 的 `AllowSession` / `AllowAlways` 必须精确到**一次调用**，不得退化为「整个工具全放行」。

`assistant-tools` 的 `ApprovalScope` 增加变体（P2 / I38a §2.6（已归档）同步）：

```rust
pub enum ApprovalScope {
    // 既有变体不变
    Mcp {
        server_id: String,
        tool: String,
        args_fingerprint: String,
        config_hash: String,
    },
}
```

| 规则 | 要求 |
|---|---|
| intent 资源格式 | `mcp:{server_id}:{config_hash}`；两段均不得含 `:`（`server_id` 为 slug，`config_hash` 为 hex） |
| 参数归一 | `args_fingerprint = SHA-256(canonical_json(arguments))`；对象键排序、无多余空白、数组顺序保留 |
| 键构成 | `ApprovalKey { tool: mcp__{server_id}__{tool}, scope: Mcp { server_id, tool, args_fingerprint, config_hash } }`；`tool` 为规范化后的 MCP 工具名（与 `name()` 第二段一致） |
| 命中条件 | 四个字段**全等**；禁止前缀匹配、禁止「同 server 任意工具/参数」命中 |
| 重连 / 重启 | `server_id` / `tool` / `config_hash` / `args_fingerprint` 不变 → 授权保留 |
| 展示名变更 | `server_id` 不变 → 授权保留 |
| server 配置变更 | `command` / `args` / `cwd` / `env` 任一变化 → `config_hash` 变化 → 旧授权失效 |
| 工具改名 | `tool` 变化 → 新工具，旧授权不命中 |
| 网关分支 | `ApprovalKey::from_intent` 识别 action `mcp`，从 `input` 计算 args 指纹；MCP 工具不得返回 `external` / `Opaque` 版本 |

> 若后续希望给 `read_only && !destructive` 的 MCP 工具提供工具级授权以降低打扰，必须另立决策；P7 v1 不使用工具级键。

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

1. `tool_setup.rs` 在 DB / Registry 初始化后，从 `mcp_servers` 读取 `enabled = 1` 的 server 并启动；失败只记日志，不阻塞启动。
2. 握手超时 30s；`tools/list` 结果缓存为工具定义快照。
3. 断线重连：指数退避（1s / 2s / 4s / 8s，上限 60s），最多 5 次；连续失败进入 `Error`，等待用户手动重启。
4. 重连成功后重新 `tools/list`；`server_id` / `tool` / `config_hash` / 规范化参数不变的工具保留已批准的 `AllowAlways` 键；任一项变化视为新调用/新工具，旧键不命中（§3.5）。
5. 用户禁用 server：先注销工具再杀进程，确保无「已注销但仍在跑」的窗口。
6. 配置变更：更新 `mcp_servers` 行 → 停旧进程 → 注销旧工具 → 用新配置重启；当前 turn 的 `TurnSpec.tools` 不变，**下一 turn 生效**；`config_hash` 变化使旧授权失效。

### 4.3 调用超时与取消

- 单次调用默认 30s，可在 server 配置中覆盖；受 turn 剩余预算约束。
- 取消：中止本次请求并返回 `Cancelled`；**不**杀 server 进程（避免影响其它并发调用）。
- turn 取消（用户停止）时，所有在途 MCP 调用随 child token 取消。
- 实现落点：令牌经 `ToolUseContext.cancel` 下发（新增字段，由执行侧从 `ToolExecCtx.child_token` 填充）；客户端在 `call_tool` 内以 `select! { biased; _ = token.cancelled() => ..., r = request => r }` 等待，取消分支返回 `ToolErrorKind::Cancelled` 并尽力发送 `notifications/cancelled`（携带 `requestId`）；只中止本次请求，**不**杀进程、不注销工具，后续调用照常可用。设计依据见 I41（已归档）§3。

---

## 5 注册与可用性

| 层 | 规则 |
|---|---|
| ToolRegistry | server `Ready` 后动态注册；`Stopped` / `Error` 时保留或注销（见 §4.1） |
| `tool_definitions_for_tier(External)` | 仅当 Agent 模式 / 对话配置的 `maxToolTier = external` 时返回 |
| 白名单 | 仍受 `ConversationToolConfig.enabledTools` deny-by-default 控制 |
| 禁用 server | 工具从 Registry 注销，且 Policy 对 `mcp` + `server_id` 记录 `Deny` |
| 审计 | 走 P2 的 `PermissionAuditStore`；不新建表 |

**硬约束**：不得因为「server 已连接」就自动把工具暴露给所有模式；必须同时满足 tier、受众、白名单三条。

---

## 6 配置（2026-11-09 定稿）

**结论：新建 `mcp_servers` 表作为唯一运行时配置源；`mcp.json` 只作为后续一次性导入格式，不做运行时第二源。**

### 6.1 表结构

```sql
CREATE TABLE IF NOT EXISTS mcp_servers (
    id               TEXT PRIMARY KEY,       -- 不可变稳定 id；模型工具名使用它
    name             TEXT NOT NULL,          -- 仅展示名，可修改
    transport        TEXT NOT NULL DEFAULT 'stdio',
    config_encrypted TEXT NOT NULL,          -- JSON {command,args,cwd,env} 的 AES-GCM 密文
    config_hash      TEXT NOT NULL,          -- 明文配置的 SHA-256；授权键使用
    enabled          INTEGER NOT NULL DEFAULT 0,
    created_at       INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL
);
```

- `id` 创建后禁止修改；删除后重建视为新 server（旧工具名与旧授权失效）。
- `name` 可改，只影响 UI 展示；模型工具名使用 `id`（§3.1）。
- v1 只存 `transport = 'stdio'`；HTTP / SSE 字段不在本批。

### 6.2 凭据与加密

- `config_encrypted` 加密整块启动配置（`command` / `args` / `cwd` / `env`），避免 `args` 中的 token 或 `env` 值进入明文 DB / 备份。
- 复用 [api_key.rs](../../src-tauri/crates/assistant-crypto/src/api_key.rs) 的 machine-id AES-GCM 路径（与 provider API key 同源）；加密/解密只在宿主层完成，`assistant-tools` 不感知。
- 解密失败或配置非法：server 不启动，状态 `Error`，只记日志；不得把密文或明文密钥写日志。
- `config_hash` 在保存时用**明文规范 JSON**（对象键排序、无多余空白）计算 SHA-256；用于授权键失效判断（§3.5），不得存储可逆信息。

### 6.3 生命周期与生效规则

1. server 默认 `enabled = 0`；启用是显式动作。
2. `enabled` **持久化**；`Ready` / `Error` / 重试次数 / `last_error` 只是运行时状态，不落库。
3. 启动时只拉起 `enabled = 1` 的 server；用户禁用后重启应用也不会自动恢复。
4. 配置变更：停旧进程 → 注销工具 → 按新配置重启；当前 turn 的工具清单不变，**下一 turn 生效**。
5. 禁用 server：先注销工具再杀进程，确保无「已注销但仍在跑」的窗口。
6. 导入：后续可提供 `mcp.json` → `mcp_servers` 的一次性导入命令；导入后运行时只读表，不保留双源。

### 6.4 最小设置页

P7 需要一个**最小 server 配置面**（不是工具卡片 UI）：

| 能力 | 要求 |
|---|---|
| server 列表 | 展示 `name`、`enabled`、运行时状态（Ready / Error / Stopped） |
| 增删改 | 编辑 `name` / `command` / `args` / `cwd` / `env`；`id` 创建后只读 |
| 启停 | 显式开关；写 `enabled` 并立即执行启停 |
| 秘密字段 | `env` 值以密文保存；UI 掩码显示，不回显明文 |
| 隔离提示 | 展示 §2 的隔离边界文案，不得宣称 OS 沙箱 |

---

## 7 实施步骤

| 步骤 | 内容 | 依赖 |
|---|---|---|
| P7-C1 | `McpTool` + 命名 / 标记推断 / 结果映射（纯 Rust）+ 单测 | P2、P5 |
| P7-C2 | stdio 客户端：`initialize` / `tools/list` / `tools/call` / 错误与超时 | P7-C1 |
| P7-C3 | `McpServerManager`：状态机、启停、重连、动态注册 / 注销 | P7-C2 |
| P7-C4 | `mcp_servers` 表 / 加解密 / 增删改查与启停命令；`tool_setup.rs` 启动序列 | P7-C3 |
| P7-C5 | 端到端：mock server 三工具（只读 / 破坏性 / 失败）跑通 Agent Loop | P4、P7-C4 |
| P7-C6 | 最小 server 配置页（列表 / 开关 / 编辑 / 掩码 / 隔离文案）；不含工具卡片专用 UI | P7-C5 |

---

## 8 验收标准

| # | 标准 |
|---|---|
| A1 | mock stdio server 可启动、握手、发现工具、调用成功；进程退出后状态收敛为 `Error` 并按退避重试 |
| A2 | 工具名 `mcp__{server_id}__{tool}` 稳定；展示名变更 / 重连 / 重启不改变工具名；`server_id` 或 MCP 工具名变化视为新工具 |
| A3 | 无 annotations 的 MCP 工具 `destructive = true`、`open_world = true`；`readOnlyHint` 采纳但不自动 Allow |
| A4 | 只有 `maxToolTier = external` + 白名单命中的工具进入 LLM 列表；ToolBox 入口不可达（`AGENT_ONLY`） |
| A5 | 禁用 server 后调用返回 `Denied` / `Internal`，且进程不再运行 |
| A6 | 调用超时返回 `Timeout`；取消返回 `Cancelled`，server 进程仍存活 |
| A7 | 大结果按 P2 预算落盘；MCP 不单独截断 |
| A8 | `cargo test -p assistant-tools` / `cargo test --test assembly_contract` 通过；`assistant-tools` 无 tauri 依赖 |
| A9 | `mcp_servers` 默认 `enabled = 0`；`enabled` 跨重启保留；`Ready` / `Error` 不落库；禁用后重启不自动启动 |
| A10 | `config_encrypted` 中看不到明文 `command` / `args` / `env`；解密失败 server 不启动且日志不含密钥 |
| A11 | MCP 审批键为 `ApprovalScope::Mcp`：参数变化 / `config_hash` 变化不命中旧授权；展示名变更与重连仍命中；不存在工具级宽授权 |
| A12 | server 配置变更在下一 turn 生效，当前 turn 工具清单不变；`resources` / `prompts` / sampling 不被调用 |

---

## 9 风险与已定稿问题

| 风险 | 缓解 |
|---|---|
| 误把逻辑权限当执行隔离 | §2 文案硬约束；设置页展示提示 |
| server 崩溃拖垮主应用 | 独立子进程 + 启动失败只记日志；调用失败归一化 |
| 工具名漂移破坏 prompt 缓存 | 稳定 `server_id` + 规范化命名；展示名不参与工具名 |
| 重连注册竞态 | 注销先于杀进程；注册在 `Ready` 后单点完成 |
| 凭据进入明文配置 | `mcp_servers.config_encrypted`；整块启动配置加密；日志不写密钥 |
| 宽授权被 MCP 工具复用 | §3.5 精确审批键；禁止工具级宽授权 |

**已定稿（2026-11-09）**：

| 原开放问题 | 结论 |
|---|---|
| 配置落点（§6 三选一） | **`mcp_servers` 表**为唯一运行时源；`mcp.json` 只做后续一次性导入 |
| 启停状态是否持久化 | **持久化 `enabled`**；`Ready` / `Error` / 重试等仅运行时 |
| `AllowAlways` 是否跨重连保留 | **保留，但键必须精确**：`server_id + tool + 规范化 args 指纹 + config_hash`；展示名变更保留，配置变更失效 |
| 是否做 `resources` / `prompts` | **v1 不做**；只做 tools，不调用 `resources/list`、`prompts/list`、sampling；有真实消费方再解冻 |

**未决项**：无（P7 可开工；任务级清单在实施时另建）。
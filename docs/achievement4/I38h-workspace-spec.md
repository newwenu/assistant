# I38h — P12：工作区根目录（Workspace Root）规格

| 项 | 值 |
|---|---|
| 状态 | 已完成（2026-11-09）：C1–C7 代码与单测完成，`contract:gen` / `typecheck` / `vitest` 通过，实机验收 C4.13 / C6.11 通过 |
| 阶段 | P12（后续轴「环境」），不进入 P0–P7 门禁 |
| 前置 | P5（OS 工具）、P6（前端与模式）；与 P8.1 无依赖 |
| 任务清单 | [I38h-workspace-tasklist.md](./I38h-workspace-tasklist.md)（唯一进度来源） |
| 影响范围 | 存储（1 新表 + 1 新列，无数据迁移）、`tool_setup`、4 处消费点、ToolBox、DTO/命令、前端会话头、审批授权缓存、设置页字段移除 |

本文只讲**一个值**：会话的「工作区根目录」——它是什么、存在哪里、由谁解析、谁消费、改变时连带什么。

边界判定算法（`ensure_within_root` / 相对路径 `join_input`）属工具层既有实现，见 [I38c](./I38c-os-tools-spec.md)；权限档位与审批决策见 [I38a](./I38a-tool-control-layer-spec.md)。本文不重述。

阶段登记见 [I38-agent-roadmap.md](./I38-agent-roadmap.md) §2；任务拆解只在 [tasklist](./I38h-workspace-tasklist.md)，本文不复制任务列表。

---

## 0 现状与目标

**现状**（P5/P6 已落地）：

```
AppConf.workspace_root（全局配置，1 个值）
        ↓ tool_setup::workspace_root_from_conf()（每次现读配置）
   ┌────────┬───────────┬───────────┬──────────┐
  工具执行   提示词事实   占用估算   策略加载   ToolBox 基准
```

问题：全局值无法表达「这个会话用 A 目录、那个会话用 B 目录」；改一次影响全部会话；用户每次换项目都要进设置页。

**目标**：工作区从「全局配置」变为「会话绑定」，且**只有这一个来源**。

```
conversations.workspace_root（每个会话 1 个值：空串 = 未配置 / 绝对路径 = 已绑定）
        ↓ resolve_workspace(conversation_id)（唯一解析点，每次现算）
   ┌────────┬───────────┬───────────┬──────────┐
  工具执行   提示词事实   占用估算   策略加载      （ToolBox 不再解析工作区）
```

| 维度 | 现状 | P12 |
|---|---|---|
| 值的作用域 | 全局 1 个，对所有会话生效 | 每会话 1 个，互不影响 |
| 未配置的表示 | 全局留空 → `None` | **空串哨兵**（`""`），是合法状态，不是错误；列**不出现 `NULL`** |
| 取值方式 | 5 处各自读配置 | 4 处统一调 `resolve_workspace(id)` |
| 换项目 | 进设置页改全局值 | 会话内一键切换 |
| 失效处置 | 变 `None` | 变 `None`（**无兜底层可退**） |

**移除默认层的理由**（本阶段明确决策）：

1. 默认层让「本会话用哪个目录」取决于另一个界面上的设置，用户无法从当前会话本身判断；显式绑定或显式「未配置」才可预测。
2. 有了默认层，「未配置」就永远不会出现，用户也就永远不会知道自己没设工作区——而这恰好是权限边界缺失的状态（见 §7.2）。
3. 默认层与「一个会话一个根」的语义冲突：全局值变化会让所有未绑定会话的边界静默漂移。
4. 参考项目里默认层是「多数实现的选择」，但它带来的隐式继承恰好是用户抱怨的来源；本阶段**明确不采用**（§11）。

**为什么未配置用空串、不占用 `null`**（本阶段明确决策）：

1. `null` 在 wire 上已有确定语义：输入 DTO 的 `null` = 「未提供该字段 / 不改」（§8.2）。若值本身也用 `null` 表示未配置，同一份 JSON 里 `null` 会同时承担两种含义，前端无法区分「没传」与「传了但未配置」。
2. 空串在类型上仍是 `String`，输出 DTO 无需 `Option`，键恒在且值恒为字符串——前端不需要「`null` 归一」分支（`??` 兜底正是漏字段 bug 的温床）。
3. 空串**不可能与合法绑定冲突**：合法值必须是绝对路径，空串永远不是；哨兵不会被误判成真实目录。
4. 空串让「未配置」在 DB 里可被查询、可被统计（`WHERE workspace_root = ''`），而 `NULL` 在 SQL 里天然带「未知」色彩，与本阶段要表达的「明确未配置」不符。

---

## 1 这个值是什么

**工作区根目录 = 规范化后的绝对目录路径**，同时承担两件事：

1. 工具路径操作的**权限边界**：绝对路径越界即拒绝；
2. 相对路径的**解析基准**：`"src/main.rs"` 相对它展开。

| 项 | 规定 |
|---|---|
| 类型 | `String`（**非** `Option`），值域只有两种：`""`（未配置）或规范化绝对路径（已绑定） |
| 存储形态 | 绝对路径**快照**，不存目录 id、不存相对路径；未配置存空串，**不存 `NULL`** |
| 归属 | 属于**会话**（`conversation_id`），不属全局、不属模式、不属分支 |
| 数量 | 一个会话恰好一个值：空串或一个路径 |
| 有效性（已绑定时） | 非空 + 绝对 + `canonicalize` 成功 + `is_dir()` 为真，四条同时满足 |
| 哨兵 | 空串 = 未配置；常量名 `WORKSPACE_UNCONFIGURED: &str = ""`，读写两侧共用 |

```rust
pub const WORKSPACE_UNCONFIGURED: &str = "";

fn validate_workspace_root(raw: &str) -> Option<String> {
    if raw.trim().is_empty() { return None; }
    let path = Path::new(raw);
    if !path.is_absolute() { return None; }
    let canonical = path.canonicalize().ok()?;
    if !canonical.is_dir() { return None; }
    Some(canonical.to_string_lossy().into_owned())
}
```

`validate_workspace_root` 的返回值只表达「是否是一个可用绑定」，`None` 同时覆盖「空串（未配置）」与「脏值（已绑定但当前不可用）」。两者在 DB 中可区分（空串 vs 非空），在执行期不可区分（都无可用工作区）。

**唯一实现**：写入校验（patch）与读取校验（resolve）必须调同一个函数，禁止两处各写一遍（否则会出现「保存时说合法、执行时说越界」）。

**它不承担**：审批档位、审批记忆、工具白名单、模型选择、MCP 服务器范围。这些各有归属，不得塞进工作区字段。

**未配置是合法状态**：不是「配置错误」，不是「待修复数据」。新会话插入时即写空串；用户可以不选工作区继续对话（工具报错，见 §7.2），也可以在任意时刻补选。

---

## 2 这个值存在哪里

### 2.1 唯一来源：`conversations.workspace_root`

```sql
ALTER TABLE conversations ADD COLUMN workspace_root TEXT NOT NULL DEFAULT '';
```

- `''` = 未配置（新会话插入即为此值）；非空 = 绑定该路径快照。
- **列不允许 `NULL`**：`NOT NULL DEFAULT ''` 使存量行在加列时直接落到「未配置」，无需 NULL 分支；读取侧永远拿到 `String`。
- 若将来需要表达「未知 / 未决」这类第三种状态，**新增独立状态列**，不复用 `null`，也不借用空串。
- 迁移只做 `ADD COLUMN`，不做破坏性变更（不改既有列类型、不删列）。
- 不存 `workspace_id` 的理由：目录簿条目可被删除、可被改名，而会话必须保留一个自洽的值；存 id 会让「删除收藏」间接改变会话行为，违反 §2.2 的分离原则。

### 2.2 目录簿 `workspaces`（**不参与解析**）

```sql
CREATE TABLE IF NOT EXISTS workspaces (
    path       TEXT PRIMARY KEY,
    name       TEXT,
    created_at INTEGER NOT NULL,
    used_at    INTEGER NOT NULL
);
```

用途只有两个：会话头下拉的「最近工作区」候选；UI 显示名。

- **不参与解析**：`resolve_workspace` 只读 `conversations.workspace_root`，绝不查此表。
- **删除收藏不影响任何会话**：`forget_workspace` 只删簿行；已绑定该路径的会话行为完全不变（值仍在 `conversations` 里）。
- 显示名缺省取目录 basename；用户改名只改簿，不改会话绑定值。
- 写入时机：仅在用户**显式选择**一个目录（新建绑定或改绑）时 `upsert` 并刷新 `used_at`。

### 2.3 遗留全局键：不迁移

旧实现把值存在 `AppConf.workspace_root`（全局 1 个）。本阶段**不做任何迁移**：

1. 加列时 `NOT NULL DEFAULT ''` 已让存量会话全部落到「未配置」，这就是升级后的期望状态。
2. 全局值本来就不是任何会话的「选择」，把它复制给所有历史会话等于凭空制造一批没人确认过的权限边界——正是 §0 要消灭的隐式继承，迁移会把这种隐式继承一次性固化到库里。
3. 不迁移就不需要标记位、不需要回填事务、不需要判断「旧值是否合法」，升级路径只保留一条 `ADD COLUMN`。
4. 旧键在 `AppConf` 中保留字段仅作历史残留：**移除设置页入口、移除 config DTO 字段、执行路径不再读取**（§3.3 不变量）。用户配置文件里的残留键不主动删除、不回写，也不再产生任何效果。
5. 升级后用户第一次进入会话会看到「选择工作区」提示（§9.3），这是预期且唯一正确的引导——一次显式选择，换来一个可解释的边界。

### 2.4 被移除的东西

| 项 | 处置 |
|---|---|
| `AppConf.workspace_root` 的**生效语义** | 移除：不再被任何执行路径读取 |
| 设置页「默认工作区」字段 | 移除（§9.4） |
| config DTO 的 `workspaceRoot` | 移除（§8.4），跑 `npm run contract:gen` |
| `tool_setup::workspace_root_from_conf` | 删除（§3.4） |
| 「全局默认 / 会话覆盖」两级回退 | 不存在（§3.2） |
| 遗留全局值 → 会话绑定的迁移 | **不做**（§2.3），存量会话保持空串 |
| 用 `NULL` 表达「未配置」 | 不采用：改用空串哨兵（§0） |

---

## 3 解析：唯一入口

### 3.1 签名

```rust
// src-tauri/src/tool_setup.rs
pub fn resolve_workspace(conn: &rusqlite::Connection, conversation_id: &str) -> Option<String>
```

- 入参必须是 `conversation_id`，**不提供无参重载**（无参重载会让调用方回到「全局取值」的老路）。
- `conn` 是显式入参而非全局单例：本仓没有全局 DB 句柄，强行造一个会让解析点绕过 `DbState` 生命周期；装配点（§4）负责把 `AppHandle → DbState → conn` 的取用包成唯一的闭包。
- 返回 `None` 只有三种来源：值为空串（未配置）、值校验失败（绑定已失效）、会话行读取失败。
- `Option` 只用于 Rust 内部表达「执行期是否有可用工作区」，**不进入 wire**：输出 DTO 用 `String` + 空串（§8.1）。
- **不做缓存**：每次调用现读 DB（与现有 `workspace_root_from_conf` 的「每次现读」一致）。原因是失效必须立刻可见，且切换工作区后不能残留旧值。

### 3.2 判定矩阵（两级，无兜底）

| 会话行状态 | `conversations.workspace_root` | 返回 |
|---|---|---|
| 存在 | `""`（空串哨兵） | `None`（未配置） |
| 存在 | 有效绝对目录 | `Some(canonical)` |
| 存在 | 非空但校验失败（相对 / 目录已不存在） | `None`（未配置） |
| 不存在 | — | `None`（未配置） |
| 读取报错 | — | `None` + `warn!`（fail-closed 到未配置） |

**矩阵里没有「退回全局」「退回进程 cwd」任何一列**——这是本阶段的核心约束。列是 `NOT NULL`，因此不存在 `NULL` 分支。

### 3.3 不变量

1. **单一来源**：解析只读 `conversations.workspace_root`；不读 `AppConf`、不读 `workspaces` 表、不读环境变量。
2. **绝不退回 cwd**：进程工作目录与用户意图无关，退回 cwd 会把权限边界锚到开发者机器上。
3. **绝不隐式继承**：没有默认层，空串就是未配置。
4. **单一校验实现**：读写共用 §1 的 `validate_workspace_root`。
5. **执行路径零直读**：`AppConf::load()` 后读 `workspace_root` 在任何位置都不允许（无迁移模块例外，§2.3）；一律走 `resolve_workspace`。
6. **不引入 `NULL`**：列 `NOT NULL`，解析侧不写 `Option<Option<String>>`、不写 `NULL` 分支、不在 wire 上输出 `null`。

### 3.4 删除旧函数

`workspace_root_from_conf()` 整体删除，**不保留为兼容包装**（保留包装等于给「全局取值」留了后门）。其现有单测一并删除或改写为 `resolve_workspace` 的矩阵测试。

---

## 4 消费点：4 处同调 + ToolBox 解耦

| # | 位置 | 现状 | P12 改造 |
|---|---|---|---|
| 1 | `src/adapters/tool_exec.rs` | `WorkspaceRootSource = Fn() -> Option<String>`，调用点 `(self.workspace_root)()` | 改为 `Fn(&str) -> Option<String>`，调用点 `(self.workspace_root)(ctx.conversation_id)`（`ToolExecCtx` 已带该字段，**不加新入参**） |
| 2 | `src/adapters/input_source.rs` | `prompt_facts()` 无参 | `prompt_facts(conversation_id: &str)`，内部调解析点 |
| 3 | `src/cmd/conversation.rs` | 占用估算按会话算，但根来自全局 | 已有 `conversation_id`，改为调解析点 |
| 4 | `src/cmd/agent/mod.rs` | `policy_executor` 装配读全局 | 入参增加 `conversation_id`，改为调解析点 |
| 5 | `src/cmd/os_tool_exec.rs`（ToolBox） | `for_toolbox(workspace_root_from_conf())` | `for_toolbox(None)`（§4.2） |

装配点唯一：`WorkspaceRootSource` 在 `tool_setup::init` 里构造一次——闭包只做「取 DB 连接 → 调 `resolve_workspace`」，禁止各处再包一层闭包改语义。

### 4.1 不扩散边界

四处消费点拿到的都是**同一个 `Option<String>`**：同一会话在同一时刻，执行、事实行、估算、策略看到的路径必须逐字节相同。任何一处自行 canonicalize / 自行 trim / 自行判断目录是否存在，都视为违反 A2。

### 4.2 ToolBox 语义（本阶段固定）

ToolBox（`os_tool_exec` 的 `fs_link` 桥接）**不是会话，没有工作区**：

- `ToolUseContext::for_toolbox(None)`：`workspace_root = None`、`resolve_base = None`。
- 后果一：ToolBox 的**相对路径一律报「未配置工作区」**（`join_input` 无基准分支）；前端 `fs_link` 表单必须以绝对路径提交并给出提示。
- 后果二：ToolBox 的绝对路径**不受工作区边界约束**（`ensure_within_root` 跳过），仅受受保护目录约束；这与现状一致，不是本阶段新增风险。
- **不引入 ToolBox 的全局基准**：给 ToolBox 配「全局工作区」正是 §0 要移除的默认层，换个名字复活没有意义。

---

## 5 进入工具层后（P12 不改）

`ToolUseContext.resolve_base` 在 Agent 链路不填，边界与基准都用 `workspace_root`；`resolution_base()` 的回退顺序、`ensure_within_root` 的实现、`join_input` 的报错文案，均为 P1/P5 既有行为，本阶段不动。

P12 只改变**这个 `Option<String>` 的来源**，不改变它进入工具层之后的任何判定。

---

## 6 改变这个值时发生什么

### 6.1 生效时机

- 写入 `conversations.workspace_root` 即刻生效（下一次工具调用、下一次提示词装配、下一次估算）。
- 切换工作区**不影响已产生的消息**：历史工具调用记录不回填、不重算、不重放。
- 同一 turn 内切换的并发口径：与 `mode_id` 一致——**turn 进行中拒绝切换**（`error.conversation_busy`），避免一个 turn 内前后两步落在不同边界。
- 多窗口：以 DB 为准；切换后另一窗口的会话头在下次刷新时看到新值（不推送专项事件，见 §14）。

### 6.2 授权联动（必须同事务）

工作区是权限边界；边界换了，旧边界下授予的会话级授权就失去意义，必须一并失效：

| 授权范围 | 处置 |
|---|---|
| `GrantScope::Session`（键前缀 `s:<conversation_id>\|`） | **删除该会话全部**（含 old value → new value 任意组合，含 value 为空/为「清除绑定」） |
| 全局授权（跨会话） | **保留**（它不属于某个工作区） |

- 删除条件按 `conversation_id` 前缀匹配，**不按资源路径过滤**：旧边界下的资源在新边界下可能仍然「看起来合法」，逐条判断会漏掉「越权但路径仍存在」的授权。
- 与工作区写入**同一事务**：先写 `workspace_root`，再删授权，再提交；任一步失败整体回滚（不允许出现「边界已换、旧授权仍在」）。
- 删除动作写审批审计（reason 标记为工作区切换），便于事后追溯。
- 「清除绑定（写回空串）」同样触发清理：未配置 = 无边界，旧授权更不应残留。

### 6.3 不扩散

切换工作区**不**改动：工具白名单/黑名单、模式、模型、审批记忆中的全局项、MCP 服务器配置（P7 尚未开工）、会话消息、分支结构。

---

## 7 未配置 / 失效时

### 7.1 判定

DB 层可区分（空串 = 未配置；非空 = 已绑定），执行层不可区分（都是 `None`，§3.2）。

- **未配置**：值为空串，UI 无需任何额外探测即可判定。
- **失效**：值为非空路径，但 `validate_workspace_root` 失败（目录被删/被改名）；UI 用自己的 `is_dir` 探测即可判定，**不依赖目录簿**。

### 7.2 表现（三处一致）

| 场景 | 表现 |
|---|---|
| 相对路径工具调用 | 报错「未配置工作区」/「无法解析相对路径」，不执行、不审批 |
| 绝对路径工具调用 | **可以执行，但无工作区边界**：仅受受保护目录与策略档位约束；审批卡片不产出「区内/区外」条目（无边界即无结论，不伪装成「在区内」） |
| 提示词事实行 | 整行不出现（不写「未配置」占位，避免模型据此推测路径） |
| 占用估算 | 按无根估算（现有行为） |
| 会话头 chip | 显示「选择工作区」（警示色），点击即开选择器 |

**风险登记**：未配置时绝对路径无边界，是既有行为；但本阶段把「未配置」从罕见（全局留空）变成**新会话默认**，因此必须在 UI 上持续可见地提示，并在首次尝试相对路径工具时给出明确报错文案（§9.3 / A5）。

### 7.3 没有自动降级

绑定失效（目录被删/被改名）时：

- 不退回任何默认层（没有默认层）；
- 不自动改库（不自动清空绑定、不自动改指向）；
- 不 panic、不阻塞发送；
- 用户必须**显式**重新选择目录或显式清除绑定。

这是与「有默认层」最大的行为差异，也是本阶段刻意选择：失效必须是用户可见的事件，而不是被兜底层悄悄吸收。

---

## 8 DTO / 命令 / 契约

| 项 | 变化 |
|---|---|
| `ConversationDto` | 新增 `workspaceRoot: string`（**非** `string \| null`；`""` = 未配置） |
| `ConversationPatchDto` | 新增 `workspaceRoot`（输入三态，见 §8.2）与 `clearWorkspaceRoot: bool` |
| `WorkspaceDto` | `{ path, name, missing }` |
| `list_workspaces` / `choose_workspace_dir` / `register_workspace` / `forget_workspace` | 新增 4 个命令 |
| `patch_conversation` | 复用，**返回类型保持 `Result<(), String>`**，不改成返回 DTO |
| config DTO | **移除** `workspaceRoot` 字段（§8.4） |

### 8.1 `ConversationDto`

```rust
pub workspace_root: String,
```

- 类型是 `String` 而不是 `Option<String>`：**wire 上永远不出现 `null`**，未配置输出空串。
- `From<ConversationRow>` 直接透传（DB 列 `NOT NULL`，行里就是 `String`）。
- 前端声明为必填 `string`，判断未配置用 `workspaceRoot === ""`，不写 `??` 兜底。

### 8.2 patch 三态

| 请求 | 语义 |
|---|---|
| `workspaceRoot` 缺省（键不存在） | 不改 |
| `workspaceRoot: null` | **不改**（沿用既有 `#[ts(optional = nullable)]` 口径：`null` 只承载「未提供」） |
| `workspaceRoot: "<绝对路径>"` | 校验通过则写入 + 清会话级授权；校验失败 `Err`，不落库、不清授权 |
| `workspaceRoot: ""` | `Err`（空串是未配置哨兵，不允许作为「设置值」传入；清除请用 `clearWorkspaceRoot`） |
| `clearWorkspaceRoot: true` | 显式清除绑定（写回 `""`）+ 清会话级授权；与 `workspaceRoot` 同时出现时 `Err`（歧义拒绝） |

要点：

1. **`null` 只在输入侧出现，且只有「不改」一个含义**；值域里的「未配置」是空串，两者不会互相污染（§0）。
2. 「清除」必须有独立布尔位：若用 `null` 表达清除，就与「不改」无法区分。
3. 空串在输入侧一律 `Err`，避免「设置」与「清除」两条路径写同一个值而语义含糊。

### 8.3 命令行为

- `choose_workspace_dir`：返回所选路径（用户取消 → `None`），**不写库**。
- `register_workspace(path)`：校验后 `upsert` 目录簿，返回 `WorkspaceDto`；不改任何会话。
- `forget_workspace(path)`：只删簿行；**不触碰 `conversations`**（A11）。
- `list_workspaces`：按 `used_at` 倒序；`missing` 由服务端 `is_dir()` 探测，每项探测要有上限（§14）。

### 8.4 移除 config 字段

`AppConf.workspace_root` 从 config DTO 中删除 → 前端 `confTypes` 不再有该键 → 设置页字段随之删除。改完必须 `npm run contract:gen` 并提交 `src/types/protocol.contract.ts`。Rust 侧 `AppConf` 结构保留字段（迁移输入），但**不进入 DTO**，因此不会出现在 wire 上。

### 8.5 契约纪律

- 改完 DTO 跑 `npm run contract:gen` + `npm run contract:check`；
- `protocol_bindings` 测试需覆盖新增命令与 `ConversationDto` 新字段；
- 禁止手改生成类型，禁止用 `Record<string, unknown>` 绕过；
- 断言 `ConversationDto` 的 `workspaceRoot` 是 `string` 而非 `string | null`（A12）。

---

## 9 前端

### 9.1 组件

`WorkspaceSelector`：会话头 chip + 下拉。下拉项：最近工作区（含 `missing` 标记）→「选择文件夹…」→「清除绑定（未配置）」。

### 9.2 挂载位置

- 只挂在 `AgentLayout` 的 `ConversationFrame` 会话头 slot；
- `ChatLayout` 不渲染（chat 模式不用工具，工作区无意义）；
- 不放进设置页、不放进全局工具栏。

### 9.3 三态显示

| 状态 | 判定 | chip | 下拉 |
|---|---|---|---|
| 未配置 | `workspaceRoot === ""` | 「选择工作区」（警示色） | 最近工作区 + 选择文件夹 |
| 有效 | 非空且 `is_dir` | 目录显示名（常规色） | 同上 + 清除绑定 |
| 失效 | 非空且 `is_dir` 为假 | 显示名 + 失效标记（警示色） | 同上 + 清除绑定 |

三态**只依赖值本身 + 一次本地 `is_dir` 探测**，不依赖目录簿；因此「路径已从簿中删除」不会再退化为「看起来有效」。

### 9.4 交互

1. 点选目录 → `register_workspace` → `patch_conversation({ workspaceRoot })` → 成功后**用提交值更新本地状态**（不做乐观更新：失败保留原值并 toast）。
2. 点「清除绑定」→ `patch_conversation({ clearWorkspaceRoot: true })` → 本地置 `""`。
3. turn 进行中禁用整个 chip（与模式切换一致），tooltip 说明原因。
4. 切换成功后刷新会话级授权缓存（后端已删，前端不得残留「已允许」）。
5. **设置页移除**：删「默认工作区」字段、删对应 i18n 键；「Agent 工具」节只保留审批等待上限。

### 9.5 i18n

新增 `workspace.chip.select` / `workspace.chip.missing` / `workspace.action.choose` / `workspace.action.clear` / `workspace.error.invalid` / `workspace.error.busy` / `workspace.switch_cleared_session_grants`；删除默认工作区相关键（含旧「跟随默认」文案）。

---

## 10 与其它阶段的关系

| 阶段 | 关系 |
|---|---|
| P5（OS 工具） | 已落地；P12 只换根来源，工具内部判定不变（§5） |
| P6（前端与模式） | 已落地；会话头 slot 复用 `ConversationFrame` |
| P7（MCP） | 未来 `roots` 的唯一输入就是 `resolve_workspace` 的结果；**未配置时 `roots` 为空列表**，不补默认值。MCP 阶段还需实现「工作区变更通知」，本阶段只保证解析点可复用 |
| P8.1 / P8.2 | 无依赖（roadmap §2.1 已登记） |
| P10 | 无交集 |

---

## 11 参考依据

| 来源 | 采用 | 不采用 |
|---|---|---|
| Goose `mcp-roots.md` | 工作区是权限/roots 的唯一边界 | 变更通知机制（留 P7） |
| DeepSeek Harness（workdir） | 显式目录绑定 + 失效即报错 | 缺省回退到某个默认目录 |
| OpenBitFun `workspaceManager.ts` | 目录簿：最近 + 显示名 + 上次使用 | 目录簿参与解析（本阶段严格分离，§2.2） |
| 通用实现（多数带默认层 / 用 null 表示空） | — | **默认工作区层**（§0）；**用 `null` 表示未配置**（§0，改空串哨兵） |

---

## 12 验收

| 编号 | 判据 |
|---|---|
| A1 | 升级不迁移：存量会话加列后全部为 `''`（未配置），遗留全局值不影响任何会话；重复启动无变化 |
| A2 | 四处消费点同源：同一会话同一时刻，执行/事实行/估算/策略取到逐字节相同的值；全仓无 `AppConf.workspace_root` 直读 |
| A3 | 绑定指向已删除目录 → 值仍非空（DB 可区分），执行报未配置、事实行不出现、chip 显示失效、不 panic、不自动改库 |
| A4 | 绑定为脏值（相对路径等）→ 同 A3，**不退回 cwd** |
| A5 | 未配置（空串）→ 相对路径工具报「未配置工作区」；绝对路径可执行且审批无区内/区外条目；chip 提示选择 |
| A6 | 切换工作区 → 该会话 `GrantScope::Session` 授权全删、全局授权保留；回滚时不出现「边界已换、授权仍在」 |
| A7 | 「清除绑定」写回空串，同样清理会话级授权 |
| A8 | 同一 turn 内切换被拒（`error.conversation_busy`） |
| A9 | `permissions.json`（项目级）按会话工作区生效：两个会话绑不同目录时各自读各自的配置 |
| A10 | ToolBox 不再解析工作区：`fs_link` 相对路径报错、绝对路径可用，行为与现状一致 |
| A11 | `forget_workspace` 后已绑定该路径的会话行为完全不变 |
| A12 | `contract:gen` / `contract:check` 幂等；`ConversationDto.workspaceRoot` 为必填 `string`（不出现 `null`）；`workspaceRoot: ""` 在 patch 中返回 `Err`；前端 `confTypes` 无 `workspaceRoot`；设置页无该字段 |

---

## 13 明确不做

- **默认工作区层（全局兜底）**——本阶段移除，不预留开关；
- **遗留全局值迁移**——不做，存量会话保持「未配置」（§2.3）；
- **用 `null` 表达未配置**——值域固定为「空串 / 绝对路径」两种；
- 多工作区并行 / 工作区列表内的多根；
- 工作区级工具白名单、工作区级模型覆盖；
- 工作区自动创建、自动清理、自动改名跟随；
- 把工作区写进消息或分支（分支共享会话的工作区）；
- 工作区变更推送事件（多窗口一致性按 §6.1 以库为准）。

---

## 14 未决项

| 项 | 说明 |
|---|---|
| Windows 路径大小写 | 存储用 `canonicalize` 结果；跨大小写比较行为在实机确认后再定 |
| `list_workspaces` 探测上限 | `missing` 逐项 `is_dir()`，需定超时/并发上限，避免网络盘卡住 UI |
| turn 中切换的替代方案 | 当前与模式切换一致直接拒绝；若实机反馈强烈，再评估「排队到 turn 结束」 |
| 空串哨兵的展示 | 空串在日志/审计里不可读，审计需按 §6.2 写成 `"none"`，不得直接落空串 |
# I70 — 审批粒度（预设档 + 用户规则）任务清单

| 项 | 值 |
|---|---|
| 状态 | 任务清单（唯一任务级进度；不占阶段号，进度只在本文件勾选） |
| 日期 | 2026-11-23 |
| 规格 | [I70-approval-granularity-spec.md](./I70-approval-granularity-spec.md)（唯一设计来源；本文不复述语义，只勾进度） |
| 判据 | 规格 §9 A1–A16 |
| 前置 | P2（工具控制层）、P6（前端与审批 UI）、P14（`shell` / `readToolOutput`）均已完成；本批不改 P0–P18 门禁 |

批次对应规格章节：T1 枚举与契约（§5.1 六处镜像 / A14）→ T2 预设档与出厂种子（§3.1 / §3.1.1 / §5.3 / A1–A2 / A13 / A15）→ T3 用户规则（§3.2 / §4.1 / §5 / §6 / A5–A7 / A9 / A11 / A12）→ T4 解释链（§7.1 / A8）→ T5 前端配置面（§7.3 / §7.4 / A5 前端 / A10 / A16）→ T6 收口验收（§8 / A3–A4 / 通用门禁）。

依赖：T1 无前置；T2 依赖 T1；T3 依赖 T1、T2；T4 依赖 T3；T5 依赖 T1、T3（与 T4 可并行）；T6 依赖 T1–T5。

> 本清单不管理提交，也不安排任何 git 提交动作：提交粒度按代码依赖与可编译性现场决定。这里只回答「代码改没改、验收跑没跑」。
> 勾选表示代码已改且对应验收已跑。

---

## 阶段 T1 — 枚举与契约（规格 §5.1，A14）

六处枚举必须同一批改完，否则穷尽 `match` 编译不过。

- [x] `assistant-protocol/src/agent.rs`：`ApprovalLevelDto` 追加 `WorkspaceWrite`（`snake_case` → `workspace_write`）；顶部注释里的取值枚举同步
- [x] 同文件 `impl From<&str> for ApprovalLevelDto`：追加 `"workspace_write"` 分支；`_` 兜底仍为 `AllOsTools`（fail-closed 不变）
- [x] `assistant-tools/src/runtime/policy.rs`：`ApprovalLevel` 追加 `WorkspaceWrite`；`as_str()` 返回 `"workspace_write"`（该值进卡片原因投影，i18n `level_hint` 必须有键）
- [x] 同文件：`ApprovalLevel::DangerousOnly` 的文档注释「仅 `destructive` / `open_world` 需审批」改为「读取放行、其余确认」（规格 §3.1 的**必须**项 / 分析 G2）
- [x] `assistant-tools/src/dto_bridge.rs`：`From<ApprovalLevelDto> for ApprovalLevel` 追加分支
- [x] `assistant-storage/src/agent_modes.rs`：镜像 `ApprovalLevel` 追加 `WorkspaceWrite`
- [x] `src/chat/core/toolConfig.ts`：`APPROVAL_LEVELS` 在 `none` 之后插入 `"workspace_write"`（数组顺序即 UI 展示顺序，由宽到严）
- [x] `npm run contract:gen`：`src/types/protocol.contract.ts` 的 `ApprovalLevelDto` 已同步（生成物由命令产出，本清单不安排提交）

> 验收：`cargo check --workspace --all-targets`；`npm run contract:check` 幂等；`npm run typecheck`。

---

## 阶段 T2 — 预设档与出厂种子（规格 §3.1 / §3.1.1 / §5.3）

- [x] `policy.rs`：抽 `fn preset_rules(approval_level: ApprovalLevel) -> Vec<PermissionRule>`，把现 `policy_from_agent_mode` 中「`base_rules()` + 三档改写 / 追加」整体迁入，逐字节等价
- [x] `preset_rules` 新增 `WorkspaceWrite` 分支：在基座三条之后追加 W1–W5，`origin` 全为 `RuleOrigin::ApprovalLevel { level }`
  - W1 `("edit", "**", Allow)`；W2 `("edit", "**/.env", Ask)`；W3 `("edit", "**/.env.*", Ask)`；W4 `("edit", "**/.env.example", Allow)`；W5 `("edit", "**/.git/**", Ask)`
- [x] `policy_from_agent_mode` 改为 `preset_rules(l)` + `hard_deny_rules()`，**签名与三档行为不变**（A1）
- [x] `assistant-storage/src/agent_modes.rs`：`BUILTIN_AGENT_TOOL_CONFIG.approval_level` 由 `DangerousOnly` 改为 `WorkspaceWrite`；上方文档注释「沙箱层级 + 仅危险操作审批」同步改为「沙箱层级 + 读取与工作区写入放行」
- [x] 同文件：`ModeToolConfig::default()` 保持 `ApprovalLevel::None`（聊天模式不受影响）
- [x] `policy.rs` 单测：`workspace_write` × 动作矩阵——`edit` 工作区路径 → `Allow`；`edit` `**.env` / `**.env.local` → `Ask`；`edit` `**.env.example` → `Allow`；`edit` `**.git/config` → `Ask`；`delete` / `execute` / `mcp` → `Ask`；`read` → `Allow`；`readToolOutput` → `Ask`；`edit` `/etc/passwd` → `Deny`
- [x] `policy.rs` 单测：三档既有用例不改即通过（A1 的直接形态）
- [x] `assistant-storage/tests/agent_modes.rs`：新建库的 `builtin-agent` 读回 `approvalLevel == workspace_write`
- [x] 同文件：手工落库的 `dangerous_only` 行读回不被改写（A15）

> 验收：`cargo test -p assistant-tools`；`cargo test -p assistant-storage`。

---

## 阶段 T3 — 用户规则（规格 §3.2 / §4.1 / §5 / §6）

**契约**

- [x] `assistant-protocol/src/agent.rs`：新增 `ApprovalRuleEffectDto`（`Allow` / `Ask` / `Deny`，`camelCase`）与 `ApprovalRuleDto { action, resource, effect }`（`camelCase` + `deny_unknown_fields`）
- [x] 同文件 `ConversationToolConfigDto` 追加 `#[serde(default)] pub approval_rules: Vec<ApprovalRuleDto>`（输出恒含键、旧 JSON 解为 `[]`）
- [x] 同文件 `ApprovalReasonDto` 追加 `UserRule { action, resource }`（`tag = "kind"` 自动得 `userRule`）
- [x] `npm run contract:gen`：`src/types/protocol.contract.ts` 已同步
- [x] `assistant-protocol/tests/protocol_bindings.rs`：`ApprovalRuleDto` / `ApprovalRuleEffectDto` 的 `decl` 注册
- [x] 同文件：旧 JSON（无 `approvalRules`）可读、输出含该键（A9）

**存储**

- [x] `assistant-storage/src/agent_modes.rs`：`ModeToolConfig` 追加 `#[serde(default)] approval_rules: Vec<ModeApprovalRule>`；新增镜像 `ModeApprovalRule` / `ModeApprovalRuleEffect`
- [x] 同文件：`ModeToolConfig::default()` 与 `BUILTIN_AGENT_TOOL_CONFIG` 的规则列表均为空（出厂用户规则层为空）
- [x] 落库形态 `ModeApprovalRule` 与 `ApprovalRuleDto` 字段同名同形，`AgentModeDto::from` 经 serde 直接解析，无需显式 `From`

**求值**

- [x] `policy.rs`：`RuleOrigin` 追加 `UserRule`（无载荷）
- [x] `policy.rs`：新增 `policy_from_agent_mode_with_rules(max_tier, approval_level, rules) = preset_rules + user(origin = UserRule) + hard_deny_rules()`；`policy_from_agent_mode` 委托传空规则
- [x] `policy.rs`：新增规则校验 `validate_rules(rules: &[PermissionRule]) -> Result<(), RuleValidationError>`，覆盖 E1–E5（条数 ≤ 64 / `action` 非空 ≤ 32 / `resource` 非空 ≤ 256 / `allow` 不得匹配 `readToolOutput` 与 `mcp`（含 `*` `**`）/ `resource` 不得以 `tier:` 开头）；E6（`effect` 三值之外）由反序列化天然拒绝，不在此函数内

**装配与保存**

- [x] `src-tauri/src/adapters/permission_policy.rs`：`policy_for_tool_config` 改调 `policy_from_agent_mode_with_rules` 并透传 `config.approval_rules`；校验失败 → 忽略规则、档位升 `AllOsTools`、`log::error`，不 panic（A6）
- [x] `src-tauri/src/cmd/agent_mode.rs`：`save_agent_mode` 在序列化前校验，失败即 `Err` 且不落库（A11）
- [x] `src-tauri/src/cmd/conversation/mutate.rs`：`patch.tool_config` 分支同校验（同上）
- [x] `src-tauri/src/cmd/agent/mod.rs`：确认每轮只经 `policy_for_tool_config` 取策略，无第二处拼规则（规格 O3）
- [x] 单测 `src-tauri/tests/permission_policy.rs`：`workspace_write` 档 + 用户规则四组——放宽预设 `Ask`、被项目 `Deny` 压住、被 tier 上限压住、`("*","*",Allow)` 无法放宽硬保护（A2 / A4）
- [x] 单测 `assistant-tools`（policy）：冲突规则 last-match-wins；`allow` 命中 `readToolOutput` / `mcp` 被校验拒绝（A5 / A7）
- [x] 单测 `assistant-tools`（装配 / context）：条数超限 → 升档 + 空规则；非法 `effect` → 解码失败走缺省装配（A6）
- [x] 单测 `cmd` 层：条数 / 长度超限保存被拒且不部分写入（A11）

> 验收：`cargo test -p assistant-tools`；`cargo test -p assistant-storage`；`cargo test`（cwd = `src-tauri`）。

---

## 阶段 T4 — 解释链（规格 §7.1，A8）

- [x] `assistant-tools/src/runtime/gateway/context.rs`：`ApprovalReason` 追加 `UserRule { action, resource }`
- [x] 同文件 `reason_from_rule`：追加 `RuleOrigin::UserRule` 分支；`RuleOrigin::Config` 分支**不得**再承载用户规则
- [x] `assistant-tools/src/dto_bridge.rs`：`From<ApprovalReason> for ApprovalReasonDto` 追加分支
- [x] `context.rs` 单测：用户规则命中 `Ask` → 原因为 `UserRule`；与项目规则原因**不合并**（两条并存，主规则集先于约束层）
- [x] `src/chat/tool/ApprovalCard.tsx`：`reasonText` 追加 `case "userRule"`（穷尽 `switch` 由类型检查强制补齐）

> 验收：`cargo test -p assistant-tools`；`npm run typecheck`。

---

## 阶段 T5 — 前端配置面（规格 §7.3 / §7.4，A5 前端 / A10 / A16）

**配置解析**

- [x] `src/chat/core/toolConfig.ts`：`DEFAULT_TOOL_CONFIG` 追加 `approvalRules: []`
- [x] 同文件 `resolveEffectiveToolConfig`：透传并拷贝 `approvalRules`（会话级整体覆盖模式级，不做逐条 merge）
- [x] 同文件：`ToolConfigDiff` 追加 `rulesChanged: boolean`，`diffToolConfig` 按三元组序列相等比较，`isEmptyToolConfigDiff` 纳入
- [x] 单测 `src/chat/core/__tests__/toolConfig.test.ts`：规则不丢、`rulesChanged` 正确（A10）

**编辑器**

- [x] 新增 `ApprovalRulesEditor` 组件，**只在** `ToolAssemblySection`（模式级设置页）使用；会话级 `ToolConfigPanel` 改用只读 `ApprovalRulesSummary` + 「去设置编辑」按钮（会话面板不内嵌输入控件）
- [x] 编辑器形态：序号 + 动作（下拉含生产集 + 自定义）+ 资源（文本）+ 效果（自动执行 / 每次询问 / 直接拒绝）+ 行尾删除；新增追加末尾；显示「后写覆盖先写」
- [x] 编辑器视觉：`deny` 行与 `allow` 行可区分（危险色 / 中性色）；效果文案不得用「允许 / 拒绝」（避免与工具白名单混淆）
- [x] 快捷规则按钮组（命令执行免确认 / 工作区写入免确认 / 读取一律确认 / MCP 调用确认 / 工具输出回读确认），写入标准用户规则、不存开关状态；`mcp` 与 `readToolOutput` 不提供 `allow`
- [x] 前端预校验与后端同规则（E1–E5），提交前拦截并给字段级提示
- [x] 变更提示文案纳入 `rulesChanged`

**i18n**

- [x] `zh-CN` / `en-US` 的 `settings.agent_mode.field.approval_level.options` 与 `chat.tool.approval.level`：各补 `workspace_write`（工作区写入 / Workspace write）
- [x] 两处的 `dangerous_only` 由「默认 / Default」改为「谨慎 / Cautious」，同处「默认」字样一并清理
- [x] `chat.tool.approval.level_hint`：四档齐备；`workspace_write` 说明必须含敏感文件例外（`.env*` / `.git/**`）
- [x] `chat.tool.approval.reason.user_rule`（`{action}` / `{resource}`）
- [x] 编辑器与快捷规则的全部文案（表头 / 效果选项 / 顺序提示 / 错误文案）

**测试**

- [x] `src/chat/tool/__tests__/approvalRulesUi.test.ts`：`ApprovalCard` 的 `userRule` 原因渲染（A8 前端侧）
- [x] 同文件：源码结构断言（会话只读 / 设置页可编辑、快捷规则与动作集），沿用既有源码结构口径、不引入 jsdom

**项目规则编辑视图（规格 §7.5）**

- [x] `src-tauri/src/cmd/`：`project_permission_status` 返回 `{ path, exists, valid, content, rules }`，复用 `PROJECT_PERMISSIONS_RELATIVE_PATH` 与 `load_project_permissions` 的解析结果；新增 `project_rule_save_cmd` 整体写入 `.assistant/permissions.json`，文件（及 `.assistant/` 目录）不存在时自动创建
- [x] 写入前与模式级用户规则共用 `validate_approval_rules`：非法规则整体拒绝、不写盘，磁盘上不留半份内容
- [x] 对应 DTO 进 `assistant-protocol`（输出 `Option` 不加 `skip_serializing_if`）+ `npm run contract:gen`
- [x] 前端设置界面复用 `ApprovalRulesEditor`（与模式级同一套编辑器与校验），展示三态（缺失 / 存在且合法 / 存在但非法）+ 保存 / 撤销 / 重新读取；非法时附原文供定位；文案说明「项目规则只能收紧」；移除「用外部编辑器打开」路径
- [x] i18n：该视图的状态文案、路径说明与保存反馈

> 验收：`npm run typecheck`；`npm run lint`；`npx vitest run`。

---

## 阶段 T6 — 收口验收（规格 §8 / §9）

- [x] A3 回归：`("*","*",Allow)` 与 `("edit","/etc/**",Allow)` 均无法把硬保护 `Deny` 放宽
- [x] A12 回归：规则不让未注入 / 已禁用工具变为可用，不放宽白名单
- [x] A13 复核：`preset_rules` + `hard_deny_rules` 与规格 §3.1.1 的序 1–8 与 W1–W5 逐条一致（条数 / `action` / `resource` / `effect` / 顺序）
- [x] A16 复核：四档在两语言各两处的选项名与说明齐备；`dangerous_only` 不再叫「默认」；`workspace_write` 说明含敏感文件例外
- [x] 通用门禁：`npm run contract:gen` 后生成物无差异；`cargo clippy --workspace --all-targets`；`npm run typecheck`；`npm run lint`；`npx vitest run`；`cargo test`（cwd = `src-tauri`）
- [x] 升级说明：写明「存量 `builtin-agent` 档位不自动迁移，需用户在设置界面主动改一次」（规格 §5.5）

> 升级说明（I70 §5.5）：出厂默认档位 `workspace_write` 只作用于**新建库**与**新模式**的初始化（`seed_builtin_modes` / `reset_builtin_mode`）。存量库中 `builtin-agent` 已存的 `dangerous_only`（或 `none` / `all_os_tools`）**不会被自动迁移**——用户此前做出的选择不静默改写；想用上新默认档位，需在「设置 → 智能体模式」把审批级别手动改为「工作区写入」一次。`tool_config_json` 为 `NULL` 或整体解析失败的行仍按缺省装配（沙箱 + `none` + 空名单），也不会自动获得 `workspace_write`。

> 验收记录：待补（落地时按批次回填，格式同 I52 tasklist）。
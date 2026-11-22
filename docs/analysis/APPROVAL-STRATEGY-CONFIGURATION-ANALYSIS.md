# 审批策略配置粒度：现状、参考项目与可行设计分析

> 日期：2026-11-21｜性质：**分析 + 可行设计候选**，非规格、非实施计划
> 范围：本项目审批配置面（前端 `toolConfig` / 后端 `assistant-tools` 策略层 / `agent_modes` 持久化）与 `references/1-ai-agent/` 下参考项目的审批策略配置机制
> 关联：[AI 审批参考分析](./AGENT-AI-APPROVAL-REFERENCE-ANALYSIS.md)（AI 在审批链上的位置）、[I38g AI 审批顾问规格](../store/I38g-agent-approval-advisor-spec.md)（单向顾问，P10 未实现）、[项目设计指导纲要](./PROJECT-DESIGN-GUIDELINES.md)（DTO / 分层约定）
> 阅读约定：参考项目事实以本地检出为准，`path:line` 为 2026-11-21 时点快照；已有参考分析 §2–§6 是历史快照，本文只补「配置面」事实，不追改其结论。

---

## 1 结论摘要

1. **三档不是策略引擎的粒度上限，而是用户配置面的粒度上限。** `none / dangerous_only / all_os_tools` 只做三件固定的事：改写基座规则、或追加一条宽泛 `read` 规则；真正的策略引擎已经是 `(action, resource) → allow/ask/deny` 的规则求值器，并有 tier 约束层、项目规则层、硬保护规则、授权缓存和四种人工决策。
2. **参考项目的共同形态是「少数预设 + 可叠加的例外规则 + 不可放宽的硬底线 + 授权缓存」。** goose 用 two principals × three buckets 的 YAML + 上下文哈希授权；codex 用 `AskForApproval` 四值（其中 `Granular` 按「审批请求来源」分五个开关）+ `permissions.toml` 可继承 profile；openhuman 用单个 `AutonomyConfig` 暴露十余个旋钮，同时保留独立于配置的硬地板；OpenBitFun 证明三档模式本身也是合法的产品选择。
3. **推荐的演进方向：保留三档作为内置默认档，增加「用户规则覆盖层 + 按请求来源的细粒度开关」，并把所有约束层统一为同一种 `PermissionRule` 形状。** 这样用户可调粒度显著变细，但策略合并算子、fail-closed 语义、审批卡片协议和授权缓存都不必重写。
4. **不建议：单纯把三档扩成七档；让项目级规则可以放宽；在没有内核级沙箱时引入 AI 双向裁决；把「全局免审批」做成无审计的静默开关。**

---

## 2 现状：三档配置面与多层执行面

### 2.1 三档的定义与落点

| 层 | 位置 | 形态 |
|---|---|---|
| 前端枚举 | `src/chat/core/toolConfig.ts:19` | `APPROVAL_LEVELS = ["none", "dangerous_only", "all_os_tools"]` |
| 协议 DTO | `src-tauri/src/protocol/dto/agent.rs:46-66` | `ApprovalLevelDto`，`snake_case`；未知字符串 fail-closed 到 `AllOsTools` |
| 策略内部 | `src-tauri/crates/assistant-tools/src/runtime/policy.rs:80-104` | `ApprovalLevel` 三值镜像 |
| 持久化 | `src-tauri/crates/assistant-storage/src/agent_modes.rs:156-201` | `ModeToolConfig.approval_level` 存入 `agent_modes.tool_config_json`；内置 agent 模式种子是 `DangerousOnly` |
| 会话覆盖 | `resolveEffectiveToolConfig`（`src/chat/core/toolConfig.ts:41`） | 会话级配置 ?? 模式级配置 ?? 契约缺省 |

三档的实际语义（`policy.rs:199-249`）：

| 档位 | 规则改写 | 效果 |
|---|---|---|
| `none` | 把首条 `("*","*",Ask)` 改写为 `Allow` | 未命中具体规则时自动执行；硬保护 `Deny` 仍在后面 |
| `dangerous_only` | 不改写 | `read=Allow`，`edit`/`execute` 等 `Ask`；内置 agent 种子档 |
| `all_os_tools` | 追加 `("read","*",Ask)` | 连读取也要审批；敏感路径的硬 `Deny` 不会被覆盖 |

### 2.2 策略执行面的完整分层

```
主规则集（last-match-wins）
  base_rules(): [("*","*",Ask), ("read","*",Allow), ("readToolOutput","*",Ask)]
  ← 审批档改写 / 追加
  + hard_deny_rules(): /etc/shadow、/etc/ssh/**、/etc/**、C:/Windows/** 等
约束层（逐层 most_restrictive：Deny > Ask > Allow）
  tier 上限（maxToolTier → "tier:os" / "tier:external" Deny）
  项目 .assistant/permissions.json（只能收紧）
  已禁用 MCP server 拒绝层
运行时旁路
  PermissionGrantStore（AllowSession / AllowAlways，按 ApprovalKey 精确命中）
  人工审批卡片（approve / deny / allowSession / allowAlways）
```

关键代码：`policy.rs:118-193`（合并算子、`evaluate_tier_ceiling`）、`src-tauri/src/adapters/permission_policy.rs:10-59`（项目规则加载与「非法即拒绝全部」）、`crates/assistant-tools/src/runtime/gateway/approval_key.rs:11-58`（授权键的五种 scope）、`gateway/decision.rs:27-36`（四种决策）。

### 2.3 「粒度较粗」的精确位置

| # | 现象 | 依据 |
|---|---|---|
| G1 | 用户可调的只有一个全局标量；同一会话里无法对某个工具、某个路径、某类来源单独设例外 | `ConversationToolConfigDto` 只有 `approvalLevel` + 工具名单（`protocol/dto/agent.rs:81-88`） |
| G2 | `dangerous_only` 实际是**动作维度**（read vs 其它），不是风险维度；`destructive` / `open_world` / `read_only` 只用于卡片展示，不参与策略求值 | `context.rs:149-184` 把 `behavior` 放入 `ApprovalContext`；`policy.rs` 只按 action 字符串匹配 |
| G3 | 项目级规则引擎已经很细，但用户不可见、无编辑入口；格式非法时整层退化为 `Deny * *` | `permission_policy.rs:32-58`、`src-tauri/tests/permission_policy.rs:39-118` |
| G4 | 授权粒度两极分化：`AllowSession` / `AllowAlways` 是精确键，缺少「本工具在本工作区」这类中间作用域 | `approval_key.rs:11-58`；i38d §3.3.1 定义 session/global 两档 |
| G5 | 档位与层级、工具名单绑在同一个 `tool_config_json`，没有命名预设或继承，模式之间只能复制配置 | `agent_modes.rs:166-201` |
| G6 | 审批最长时间是可配的（`AppConf.approvalTimeoutSecs`），但「哪些请求要问、哪些自动拒绝」没有对应旋钮 | `protocol.contract.ts:43`；`i38d §3.3`（超时默认 120s） |

### 2.4 设计红线（任何细化方案都必须保持）

1. `PermissionEffect::most_restrictive` 的合并方向不变：约束层只能收紧（`policy.rs:20-29`）。
2. 硬保护 `Deny` 不被任何用户配置放宽（`policy.rs:341-350`）。
3. `readToolOutput` 恒 `Ask`，不随档位放宽（I52 §5.11.4(3)，测试 `policy.rs:446-476`）。
4. 未知档位字符串、非法项目规则一律 fail-closed（`agent.rs:57-65`、`permission_policy.rs:49-58`）。
5. 审批卡片只能回答「为什么问我」，不能成为配置入口；新增解释只能进 `ApprovalContext`（I38d §3.2.1，`context.rs:56-71`）。
6. ToolBox 直调不经网关，细化配置只影响 Agent 链路（设计纲要 §7.7）。

---

## 3 参考项目的配置面事实

### 3.1 goose：双 principal × 三桶 + 上下文哈希授权

- `permission.yaml` 以 principal 分组，内置 `user` 与 `smart_approve` 两个；每个 principal 有 `always_allow` / `ask_before` / `never_allow` 三个桶，读取优先级 `never > always > ask`（`references/1-ai-agent/goose/crates/goose/src/config/permission.rs:13-33, 77-84, 134-157`）。
- 文件解析失败直接 panic 拒绝启动，注释写明 "Refusing to start with corrupted permission config"（同文件 `:224-238`）。写入使用临时文件 + fsync + rename，并对符号链接目标加文件锁（`:196-278`）。
- 工具名带扩展命名空间（`ext__tool`），删除扩展时按前缀批量清理权限（`:280-305`）——这是「按工具族管理例外」的先例。
- 运行时授权另存 `permissions/tool_permissions.json`：键为 `tool + blake3(arguments)`，记录 `allowed` / 时间戳 / 可选过期时间，加载时清理过期项（`crates/goose/src/permission/permission_store.rs:11-28, 79-143`）。
- 模式（`GooseMode`）与权限配置是**正交两轴**：`Auto / Approve / SmartApprove / Chat`，默认 `Auto`；`SmartApprove` 才启用只读判定与模型分类（见已有参考分析 §3.1）。
- **可借鉴**：档位与例外名单分离；`never_allow` 优先级最高；授权可带 TTL；损坏配置 fail-closed。
- **不可直接借鉴**：panic 式 fail-closed 不适合桌面常驻应用；按参数哈希授权会把「同工具不同参数」当成不同授权，粒度比本项目现有键更细但打扰更多。

### 3.2 codex：按审批请求来源细化 + 可继承 profile

- `AskForApproval` 四值：`untrusted` / `on-request`（默认，别名 `on-failure`）/ `granular(GranularApprovalConfig)` / `never`（`references/1-ai-agent/codex/codex-rs/protocol/src/protocol.rs:1080-1121`）。
- `GranularApprovalConfig` 的五个布尔字段是按**「审批请求为什么产生」**切分，而不是按工具或路径：`sandbox_approval`、`rules`（execpolicy prompt）、`skill_approval`、`request_permissions`、`mcp_elicitations`（同文件 `:1123-1138`）。字段为 `true` 表示放行该类请求，为 `false` 表示自动拒绝而非弹给人工（`:1110-1115`）。
- `permissions.toml` 是命名 profile 表：`PermissionProfileToml { description, extends, workspace_roots, filesystem, network }`；解析 `extends` 链、检测环、禁止继承内置 profile（`:` 前缀），子 profile 覆盖父 profile（`codex-rs/config/src/permissions_toml.rs:22-119, 158-191`）。
- `filesystem` 的权限值可为单一访问模式或 `{ pattern → access mode }` 映射；`network.domains` 为 pattern → `allow|deny`（同文件 `:209-281`）。
- **可借鉴**：细粒度维度选「请求来源」而非穷举工具；profile 继承让预设可复用；配置解析阶段做环检测与未知字段拒绝。
- **不可直接借鉴**：`granular` 的 false 语义是自动拒绝，本项目当前卡片没有「自动拒绝该来源」的用户表达；`permissions.toml` 同时承担沙箱边界配置，本项目无对应 L4 边界。

### 3.3 openhuman：单一配置结构上的多旋钮 + 独立硬地板

- `AutonomyConfig`（`references/1-ai-agent/openhuman/crates/openhuman-core/src/config/schema/autonomy.rs:8-86`）字段：`enabled`（**默认 false**）、`level`（默认 `Supervised`）、`workspace_only`、`allowed_commands`（默认约 25 个命令）、`forbidden_paths`、`max_actions_per_hour`、`max_cost_per_day_cents`、`require_approval_for_medium_risk`、`block_high_risk_commands`、`auto_approve`（默认 7 个只读工具）、`auto_approve_all`、`trusted_roots`、`allow_tool_install`。
- 源码注明：`enabled=false` 时除 `is_always_forbidden`（`~/.ssh`、`~/.gnupg`、`~/.aws`、系统根）外，分类、白名单、审批门、预算全部失效（`:11-31`）。已有参考分析 §5.1 的 `readonly()/supervised()` 是显式打开审批门的预设，与「原始配置默认关闭」不矛盾，是两个入口。
- `auto_approve_all` 是显式的一揽子绕过开关，注释诚实记录了代价：远程 triage dispatch 将不再写 `pending_approvals` 审计行（`:55-75`）。
- **可借鉴**：旋钮多但默认值保守；硬地板与配置开关正交；把「一揽子放行」的审计代价写在配置定义旁。
- **不可直接借鉴**：本项目 `none` 档已经是默认放行，不需要再加一个更宽的全局开关；openhuman 的容量/成本旋钮属于任务资源管理器范畴（`docs/analysis/TASK-RESOURCE-MANAGER-ANALYSIS.md`），不应塞进审批配置。

### 3.4 OpenBitFun：三档模式即完整配置面

- `DISPATCH_PERMISSION_MODES = ['ask', 'auto', 'reject']`，分别映射为 `remote` / `auto` / `reject-and-report`；`full_access` 折叠进 `auto`（`references/1-ai-agent/OpenBitFun/src/web-ui/src/features/dispatch/approvalPolicy.ts:16-35`）。
- 注释明确：权限策略的唯一编辑点是对话框上的控制条，协议逐轮携带策略，刻意不提供第二处配置入口（同文件 `:1-10`）。
- **可借鉴**：如果产品目标只是「用户表达意图」，三档足够；细化配置必须有明确收益（减少打扰或表达硬约束），否则只增加认知成本。

### 3.5 参考矩阵

| 项目 | 预设档 | 例外/细化载体 | 继承 | 硬地板 | 授权缓存 | 失败语义 |
|---|---|---|---|---|---|---|
| goose | 4 模式 | principal × 三桶名单 + 参数哈希 | 无 | 无 | 有，可带 TTL | 配置损坏即拒绝启动 |
| codex | 4 值 + 5 开关 | `permissions.toml` profile（roots / fs / network） | 有 | 沙箱（L4） | 有，且验证动作未变 | 证据不足 → deny |
| openhuman | 3 预设 | 单结构多旋钮 + auto_approve 名单 | 无 | `is_always_forbidden` | 有，SQLite 持久化 | 超时 deny |
| OpenBitFun | 3 模式 | 无 | 无 | 无 | 无 | 直接拒绝并上报 |
| **本项目现状** | **3 档** | 项目 JSON 规则（只能收紧）+ 精确键授权 | 无 | 硬编码 Deny 列表 | 有，session/global | 解析失败 → deny all |

---

## 4 可行设计候选

### 4.1 候选 A（推荐）：三档预设 + 规则覆盖层 + 按来源开关

保留 `approvalLevel` 作为主档位与「用户一眼能懂」的锚点，在 `ConversationToolConfigDto` / `ModeToolConfig` 上新增两个可选字段：

```ts
export type ApprovalRuleEffectDto = "allow" | "ask" | "deny";
export type ApprovalRuleDto = { action: string; resource: string; effect: ApprovalRuleEffectDto };
export type FlowApprovalDto = {
  shell?: ApprovalRuleEffectDto | null;
  mcp?: ApprovalRuleEffectDto | null;
  network?: ApprovalRuleEffectDto | null;
  readToolOutput?: ApprovalRuleEffectDto | null;
};

// 在现有 ConversationToolConfigDto / ModeToolConfig 上追加（均带缺省）：
//   approvalRules: ApprovalRuleDto[]
//   flowApproval: FlowApprovalDto
```

- `approvalRules`：用户规则，复用现有 `PermissionRule` 的 `(action, resource, effect)` 与 glob 语义；允许把预设的 `Ask` 放宽为 `Allow`（这是「用户自定义」的意义），但排在硬保护之前，无法覆盖硬 `Deny`。
- `flowApproval`：codex 式按审批来源的开关，内部展开为 `("execute"|"mcp"|"network"|"readToolOutput", "*", effect)` 规则，只是规则编辑器的语法糖；`readToolOutput` 必须保持「只能收紧、不能放宽」（I52 边界）。
- 内置三档仍映射到现有 `policy_from_agent_mode` 的三分支，零配置用户的行为逐字节不变。

**优点**：复用现有规则引擎、审批卡片解释链、授权键和审计；迁移只需给两个结构的旧字段加缺省；用户配置与项目配置的信任边界清晰。
**代价**：规则编辑器的 UX 与校验需要处理用户写错的场景；`resource` 仍是字符串匹配，安全上限不变（见 §6）。

### 4.2 候选 B（后续）：命名策略档 + 继承

参考 codex profile：把规则集合命名为 profile，支持 `extends`，模式与会话只持有 profile id；内置 `:strict` / `:balanced` / `:off` 对应三档。

**优点**：可复用、可测试、可为不同项目/工作区切换整套策略；规则与模式解耦（解决 G5）。
**代价**：需要新表或新配置文件、迁移、profile 解析与环检测、选择 UI；当前只有 3 个内置模式，收益尚未兑现。建议作为 A 之后的第二阶段，除非明确要求跨模式共享自定义策略。

### 4.3 候选 C（不推荐单独采用）：扩展档位枚举

把三档扩成 `off / read_only / low / medium / high / custom` 之类的阶梯。

**优点**：改动最小，UI 仍是一个下拉框。
**代价**：没有解决按工具/路径/来源的例外（G1–G3）；旧持久化值语义漂移风险；档位越多，用户越难判断两档差别（OpenBitFun 的反例说明三档有时就是最优）。可作为 A 的 UI 呈现补充（三档 + `custom` 指向规则编辑器），不应作为唯一机制。

### 4.4 对比与建议

| 维度 | A 预设+规则 | B profile 继承 | C 扩枚举 |
|---|---|---|---|
| 解决 G1/G2/G3 | 是 | 是 | 否 |
| 解决 G5 | 否 | 是 | 否 |
| 迁移成本 | 低 | 中高 | 低（但有语义漂移风险） |
| 新增 UI 面 | 规则编辑器 | 规则编辑器 + profile 管理 | 无 |
| 与现有策略引擎契合 | 高 | 中 | 高 |
| 推荐 | **第一阶段落地** | 第二阶段候选 | 仅作档位 UI 的补充说明 |

---

## 5 候选 A 的关键设计细节

### 5.1 求值顺序

```
preset_rules = policy_from_agent_mode(...).rules()       // 现有三分支不变
base = preset_rules
     + flow_switches 展开的规则（origin = FlowSwitch）
     + user_rules（origin = UserRule）
global_rules = base + hard_deny_rules()                  // 硬保护永远最后
layers = [tier ceiling, project permissions.json, mcp disabled]
evaluate = last_match(global_rules) → most_restrictive(layers...)
```

- 用户规则可以放宽预设，但**不能**放宽 tier 上限、项目规则、硬保护（`most_restrictive` 保证）。
- 用户规则与项目规则冲突时项目规则胜出；卡片 `reasons` 会同时显示命中的 `Ask` 规则，用户能看到「用户规则说 allow，项目规则说 ask」。
- 新增字段全缺省时 `global_rules` 与今天逐字节一致。

### 5.2 类型与迁移

- Rust：`ModeToolConfig`（`agent_modes.rs:166-174`）与 `ConversationToolConfigDto`（`agent.rs:81-88`）新增字段时必须 `#[serde(default)]`；两者都有 `deny_unknown_fields`，缺省字段只影响**缺失**，不影响旧数据读取。
- 前端：`src/types/protocol.contract.ts` 由 `npm run contract:gen` 生成，不手改；`DEFAULT_TOOL_CONFIG`（`toolConfig.ts:30-35`）补空规则数组。
- 旧库行 `tool_config_json = NULL` 仍走协议缺省；旧值 `approval_level` 三个字符串含义不变。
- `ApprovalLevelDto::from(&str)` 的未知值兜底保持 `AllOsTools`，不为新档位开后门。
- 规则数量上限与 `resource` 长度上限需要写入校验，避免配置文件膨胀拖慢每轮规则求值；阈值由压测决定，不在本文定值。

### 5.3 解释链与 UI

- `RuleOrigin` 增加 `FlowSwitch` / `UserRule` 两个变体（`policy.rs:71-78`），`reason_from_rule`（`context.rs:342-362`）映射到新 i18n 键 `chat.tool.approval.reason.flow_switch` / `user_rule`；未识别分支由前端通用降级渲染。
- 设置页：`ToolAssemblySection.tsx` / `ToolConfigPanel.tsx` 保留「审批级别」下拉为主控件，新增可折叠的「细粒度规则」区；项目 `permissions.json` 提供只读视图与 JSON Schema 下载，不提供写入 UI，以维持「只能收紧」的信任边界。
- 档位文案需要纠正 G2：`dangerous_only` 的中文标签「默认」与提示「读取自动放行，修改和执行需确认」（`zh-CN.json:1083-1092`）已经比枚举名更准确；**不要改枚举值**，只优化说明。

### 5.4 授权与审计

- 四种卡片决策与 `ApprovalKey` 形状不变（`decision.rs:27-36`、`approval_key.rs:11-58`）。
- G4 的「中间作用域」单独评估：新增工作区级 grant 需要定义 `ApprovalScope::WorkspaceTool { canonical_root, tool, pattern }` 及撤销 UI；它比现有精确键更宽，必须配套审计与二次确认，建议放到 A 稳定后的独立批次。
- 用户规则命中 `Allow` 而实际执行仍被项目规则拦下时，写一条审计说明来源；`AuditOutcome` 现有的 `Allowed` / `Approved` / `Denied` 语义不变，避免重复计数。

### 5.5 失败语义

- 用户规则解析或校验失败：整份用户配置视为非法，按 `AllOsTools` + 清空用户规则 + 写醒目日志处理（与项目规则「非法即拒绝全部」同向），不静默丢弃可疑的放宽规则。
- 规则编辑器在保存前做同步校验并即时反馈，尽量让非法配置在写入前就被拒绝；读取时的 fail-closed 是第二道防线。
- 规则 OS 匹配不产生 IO；审批卡片的 2s 预演预算不变（`context.rs:17-20`）。

### 5.6 验证清单（实施时）

- Rust：`cargo test -p assistant-tools`（重点 `tests/policy.rs` / `tests/pipeline.rs`）、`cargo test -p assistant-app --test permission_policy --test tool_resolver --test protocol_bindings`、`cargo clippy -D warnings`、`cargo fmt --check`。
- 前端：`npm run contract:gen && npm run contract:check`、`npm run typecheck`、`npm run lint`、`npm run test`（重点 `toolConfig.test.ts`、`settingsSnapshot.golden.test.ts`、`chatPendingQuestions.test.ts`）。
- 契约断言：新 DTO 登记到 `src-tauri/tests/protocol_bindings.rs`；输出 `Option` 键恒在（`docs/analysis/PROJECT-DESIGN-GUIDELINES.md:3.1`）。
- 兼容性用例：旧 `tool_config_json`（无新字段）、`NULL` 行、未知档位字符串、非法规则、用户规则与项目规则冲突、硬保护路径尝试放宽。

---

## 6 风险与未决问题

| # | 风险 / 问题 | 判断 |
|---|---|---|
| R1 | 规则是字符串 glob（L2），不等于真实操作范围；`cat /etc/shadow` 的变体可绕过 | 规则不能当安全边界；硬保护 + 沙箱仍是下限（已有参考分析 §7.6） |
| R2 | 无内核级 L4 沙箱，不能引入 AI 双向裁决 | I38g P10-c 需独立 ADR；P10-a 单向顾问与细化配置正交，启用后需重测 base 分布 |
| R3 | 用户规则可放宽预设，可能被误配成「全放行」 | 编辑器显著标注 effect；`deny` 类规则优先展示；审计可追溯 |
| R4 | `dangerous_only` 名不副实（实为 read vs 其它） | 保留枚举值避免迁移；i18n 文案承担语义澄清 |
| R5 | 规则数无上限会拖慢每轮求值 | 写入校验设上限；上限值需压测后定 |
| R6 | 项目规则与用户规则同时存在时用户困惑 | 卡片同时展示两条来源；设置页只读展示项目规则 |
| R7 | 「中间作用域授权」会扩大放行范围 | 独立批次；需要二次确认、撤销入口与审计 |

**开放问题**：

1. 用户规则是否允许 `allow` 放宽预设？（本文建议允许；若产品判断「配置只能收紧」，则 A 退化为更强的规则层，收益减半。）
2. `flowApproval` 的 `deny` 是否等价于 codex 的「自动拒绝、不弹卡片」？（本文建议是，但需要确认模型收到什么错误文案。）
3. 工作区级 grant 是否必要？若做，键里要不要包含 sandbox enforcement（现有命令键已包含该字段，见 `approval_key.rs:27-33`）。
4. 是否需要 profile 继承（候选 B）？触发条件应是「出现第二个自定义模式且规则重复」。
5. 细化后如何度量收益？已有参考分析 §9.5 指出「打扰量」缺乏度量；若上线规则编辑器，应同步定义减少的审批次数指标。
6. 规则编辑入口放设置页还是工具面板？前者偏全局、后者偏会话，需要产品决策。

---

## 7 参考索引

| 主题 | 文件 |
|---|---|
| 本项目三档与缺省 | `src/chat/core/toolConfig.ts:19-35`、`src-tauri/src/protocol/dto/agent.rs:46-101` |
| 本项目策略生成与合并 | `src-tauri/crates/assistant-tools/src/runtime/policy.rs:118-249, 322-350` |
| 项目规则加载 | `src-tauri/src/adapters/permission_policy.rs:10-79`、`src-tauri/tests/permission_policy.rs` |
| 授权键与决策 | `src-tauri/crates/assistant-tools/src/runtime/gateway/approval_key.rs:11-58`、`gateway/decision.rs:8-95` |
| 审批卡片解释链 | `src-tauri/crates/assistant-tools/src/runtime/gateway/context.rs:23-71, 319-362` |
| AI 顾问边界 | `docs/store/I38g-agent-approval-advisor-spec.md` §1.3 / §3.2 / §8 |
| goose 权限配置 | `references/1-ai-agent/goose/crates/goose/src/config/permission.rs:13-33, 77-84, 134-157, 196-305`、`crates/goose/src/permission/permission_store.rs:11-143` |
| codex 审批配置 | `references/1-ai-agent/codex/codex-rs/protocol/src/protocol.rs:1080-1138`、`codex-rs/config/src/permissions_toml.rs:22-119, 208-348` |
| openhuman 自主配置 | `references/1-ai-agent/openhuman/crates/openhuman-core/src/config/schema/autonomy.rs:8-210` |
| OpenBitFun 三模式 | `references/1-ai-agent/OpenBitFun/src/web-ui/src/features/dispatch/approvalPolicy.ts:1-53` |

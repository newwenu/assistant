# I70 — 审批粒度规格（预设档 + 用户规则 + 不可放宽底线）

| 项 | 值 |
|---|---|
| 状态 | 主题规格（设计定稿，**未立项**；主题任务不占阶段号，实施进度只在 [I70 tasklist](./I70-approval-granularity-tasklist.md) 勾选） |
| 日期 | 2026-11-23 |
| 归属 | 审批链轴的配置面扩展（P6 已完成能力的粒度补强）；不新增阶段编号、不改 P0–P18 门禁 |
| 事实来源 | `docs/analysis/APPROVAL-STRATEGY-CONFIGURATION-ANALYSIS.md`（2026-11-23 快照：G1–G6、候选 A/B/C、R1–R7、OQ1–OQ7；**时点记录，不随后续代码追改**；与本文冲突以本文为准） |
| 关联 | [I38g](../../store/I38g-agent-approval-advisor-spec.md)（P10 顾问：单向放宽）· [I52](../../store/I52-shell-sandbox-spec.md)（沙箱不替代审批；`readToolOutput` 恒 `Ask`）· I38e（已归档）（MCP 精确键）· I38d（已归档）（审批卡片 L0–L3、`policy_for_tool_config` 装配）· I38a（已归档）（Policy / Gateway 求值）· [I69a](../../store/I69a-agent-orchestration-base-spec.md) §3（编排层保持调用级授权） |
| 影响范围 | 后端：`src-tauri/crates/assistant-protocol/src/agent.rs`、`src-tauri/crates/assistant-storage/src/agent_modes.rs`、`src-tauri/crates/assistant-tools/src/runtime/policy.rs`、`src-tauri/crates/assistant-tools/src/runtime/gateway/context.rs`、`src-tauri/src/adapters/permission_policy.rs`、`src-tauri/src/cmd/agent_mode.rs`；前端：`src/chat/core/toolConfig.ts`、`ToolConfigPanel`、`ToolAssemblySection`、`ApprovalCard`、`src/i18n/locales/{zh-CN,en-US}.json` |
| 契约影响 | `ApprovalLevelDto` 追加 `workspace_write`（六处枚举镜像同步，A14）；新增 `ApprovalRuleDto` / `ApprovalRuleEffectDto`；`ConversationToolConfigDto` 追加 `approvalRules`；`ApprovalReasonDto` 追加 `userRule` 分支。改后必须 `npm run contract:gen` 并提交生成物 |
| 判据 | 本文 §9 A1–A16 |

> **本文管理**：粒度模型（§3）、求值顺序与合并（§4）、DTO 与持久化（§5）、校验与失败语义（§6）、解释链与 UI（§7）、不可放宽底线（§8）、验收（§9）、与相邻规格的关系（§10）、未决清单（§11）。
> **本文不管理**：预设档的求值实现细节（以 `policy.rs` 与 I38d（已归档）为基线，本文只定义档位语义与规则集，不复述求值机制）、AI 顾问的判断逻辑（[I38g](../../store/I38g-agent-approval-advisor-spec.md)）、沙箱边界（[I52](../../store/I52-shell-sandbox-spec.md)）、工具注册与白名单（I58（已归档））。
> **与分析的差异**：分析 §4.1 的候选 A 同时提出 `approvalRules` 与 `flowApproval` 两个字段；本文**只保留 `approvalRules`**，「按来源开关」降级为 UI 快捷规则（§3.3）。理由：`flowApproval` 与规则表达同一维度，两套真值必然漂移（同一件事两处改、两处解释），而规则已能精确表达其全部语义（分析 R5 的结论）。

> 本文中的「必须 / 禁止」为可验收约束；「应当」为默认做法，偏离须在代码注释写明理由。

---

## 1 目标与非目标

### 1.1 目标

1. **调整出厂默认档位**：新增第四档 `workspace_write` 并设为 `builtin-agent` 出厂默认，把开箱状态从「读取放行、其余确认」改为「读取放行、工作区写入放行、执行 / 删除 / MCP / 工具输出回读仍确认」（规则集见 §3.1.1）。原有三档的语义与取值**不变**，已存配置**不迁移、不静默改档**（§3.1.2 D8 / §5.5）；
2. 提供**一层用户规则**，让「工作区读文件自动放行、`rm` 必须问、某目录一律拒绝」这类真实诉求不必退化成 `all_os_tools`（解决分析 G1 / G3）；
3. 规则**只放宽应用默认，不放宽底线**：硬保护、tier 上限、项目规则、`readToolOutput`、已禁用 MCP 的收紧效果在任何配置下都成立（解决分析 G4 / G5）；
4. 规则**参与解释链**：审批卡片能说「这条规则是你自己写的」，而不是伪装成项目规则或默认策略（解决分析 G6）；
5. 规则**可校验、可失败**：非法规则在保存时被拒；运行时读到非法配置一律 fail-closed（§6）；
6. 全部改动**不引入新求值语义**：仍是单层 last-match-wins + 跨层 most_restrictive（§4.2），不新增合并算子、不改求值顺序。

### 1.2 非目标

- **不做按来源的布尔开关**：不做 `flowApproval` 这类按来源的开关（理由见文首「与分析的差异」）；档位只有 §3.1 列出的四值，不再派生 `read_only` / `custom` 等与既有档语义重叠的档。
- **不细化授权的作用域**：不做工作区级 / 目录级授权，也不做按调用参数细分的授权（如「只允许 `git status`」）；授权缓存仍以会话 + 作用域为界（I38d（已归档）），命令级细分属 `shell` 工具规格。
- **不做规则继承**：模式 → 会话是整体覆盖，不做逐条 merge（§5.4）。
- **不开新的配置写入口**：审批卡片只写授权缓存，AI 顾问不写规则（[I38g](../../store/I38g-agent-approval-advisor-spec.md)）；项目级 `permissions.json` 由设置界面用与模式级相同的规则编辑器写入（§7.5），不引入第三种规则语法。

> 「哪些放宽被禁止」属约束而非范围问题，集中在 §8 INV-3 / INV-6，本节不重复。

---

## 2 术语与边界

### 2.1 术语

| 词 | 含义 |
|---|---|
| 审批粒度 | 配置面表达「哪些调用需要人工确认」的维度集合：预设档（粗）+ 用户规则（细） |
| 预设档 | `approvalLevel` 的四值 `none` / `workspace_write` / `dangerous_only` / `all_os_tools`，决定基础规则集与首条结论；`workspace_write` 是本文新增且为出厂默认 |
| 默认规则集 | 应用内置、非用户书写的规则全集 = `base_rules()` + `hard_deny_rules()` + 档位分支追加的规则（§3.1.1 逐条列出）；用户规则只能追加在它之后 |
| 用户规则 | `approvalRules` 中的一条 `{action, resource, effect}`，由用户显式书写，来源标记为 `UserRule` |
| 快捷规则 | 设置界面上一键写入若干条用户规则的按钮；**不是**独立配置项，不持久化开关状态（§3.3） |
| 动作 | 工具审批意图的 `action`（`PermissionIntent.action`），如 `read` / `edit` / `execute` / `mcp` |
| 资源 | 工具审批意图的 `resource`（如绝对路径、`mcp:{server}:{tool}`），按 glob 匹配 |
| 不可放宽底线 | 任何用户配置都无法改变为更宽松的收紧项：硬保护 Deny、tier 上限、项目规则、`readToolOutput` 恒 `Ask`、已禁用 MCP 恒 `Deny` |

### 2.2 三层信任边界

| 层 | 谁能改 | 方向 | 载体 |
|---|---|---|---|
| 应用默认 + 预设档 | 用户（选择档位） | 可放宽，也可收紧 | `approvalLevel` |
| 用户规则 | 用户 | 可放宽预设、也可收紧（写 `Deny`），但不得放宽底线 | `approvalRules` |
| 项目规则 / tier / 硬保护 | 项目维护者 / 用户（仅 `maxToolTier`） | **只能收紧** | `.assistant/permissions.json`、`maxToolTier`、`hard_deny_rules()` |

> 边界成立的机制是 §4.2 的顺序不变式：用户规则排在预设规则之后、硬保护之前，且只能进主规则集，不能进约束层。

### 2.3 与相邻机制的边界

| 机制 | 负责 | 不负责 |
|---|---|---|
| 审批粒度（本文） | 决定一次调用「要不要问」 | 决定「允许做什么」（沙箱）、「记不记得住答案」（授权缓存） |
| 授权缓存 | 记住用户对某作用域的「本会话允许」 | 不产生规则、不跨会话、不写配置 |
| 沙箱（[I52](../../store/I52-shell-sandbox-spec.md)） | 决定「最多能做到哪」 | 不替代审批；沙箱内也要审批 |
| AI 顾问（[I38g](../../store/I38g-agent-approval-advisor-spec.md)） | 在 `Ask` 之后给建议 | 不放宽 `Deny`、不写规则、不跳过卡片 |
| 审计（I50（已归档）） | 记录 `Allow` / `Deny` 与命中原因 | 不参与决策 |
| 工具白名单（I58（已归档）） | 决定模型看得见哪些工具 | 不决定审批粒度；规则不能把未注入工具变可见 |

---

## 3 配置模型

### 3.1 预设档（保留三档语义，新增第四档并改为出厂默认）

`policy_from_agent_mode(max_tier, approval_level)` 的实际行为如下，**本文以此为准**：

| 档位 | 基础规则集（改写后） | 效果 |
|---|---|---|
| `none` | `("*","*",Allow)` · `("read","*",Allow)` · `("readToolOutput","*",Ask)` | 未命中具体规则的调用全部自动执行；`readToolOutput` 仍 `Ask` |
| `workspace_write`（**新增，出厂默认**） | `("*","*",Ask)` · `("read","*",Allow)` · `("readToolOutput","*",Ask)` · `("edit","**",Allow)` + 敏感文件例外（§3.1.1 W1–W5） | **读取与工作区写入自动执行**；执行 / 删除 / MCP / 工具输出回读仍需确认；敏感文件（`.env*` / `.git/**`）写入仍需确认 |
| `dangerous_only` | `("*","*",Ask)` · `("read","*",Allow)` · `("readToolOutput","*",Ask)` | **读取自动执行，其余一律需确认** |
| `all_os_tools` | 同上 + `("read","*",Ask)` 追加在硬保护之前 | 读取也需确认；敏感路径的硬保护 `Deny` 不被覆盖 |

> **必须**：`ApprovalLevel::DangerousOnly` 的代码注释与用户文案不得再写「仅 `destructive` / `open_world` 需审批」——实际语义是「读取放行、其余确认」，工具级 flag 不参与该档判定（§7.4）。这是分析 G2 的落点，属文案与注释修正，不改枚举、不改求值。

#### 3.1.1 默认规则集（逐条镜像，改动须同步）

`base_rules()` + 档位分支追加 + `hard_deny_rules()` 的完整内容如下。**本表是代码的逐条镜像**：改代码必须同时改表（A13）；表中「谁可改」列决定 §8 哪些条目是底线。

| 序 | action | resource | effect | 来源 | 谁可改 |
|---|---|---|---|---|---|
| 1 | `*` | `*` | `Ask` | `base_rules()` 基座 | 档位 `none` **改写**为 `Allow`；用户规则可覆盖 |
| 2 | `read` | `*` | `Allow` | `base_rules()` | 档位 `all_os_tools` 追加 `("read","*",Ask)` 覆盖；用户规则可覆盖 |
| 3 | `readToolOutput` | `*` | `Ask` | `base_rules()`（I52 §5.11.4(3)） | 用户规则**只能收紧为 `deny`**，不可放宽（§6 E4 / INV-3） |
| 4 | `read` | `/etc/shadow` | `Deny` | `hard_deny_rules()` | 无人可改（INV-1） |
| 5 | `read` | `/etc/ssh/**` | `Deny` | 同上 | 无人可改 |
| 6 | `edit` | `/etc/**` | `Deny` | 同上 | 无人可改 |
| 7 | `edit` | `C:/Windows/**` | `Deny` | 同上 | 无人可改 |
| 8 | `edit` | `C:/Program Files/**` | `Deny` | 同上 | 无人可改 |

`workspace_write` 档在其后追加五条（插在序 3 之后、序 4–8 之前，因此先命中宽泛 `Allow` 再被例外覆盖）：

| 序 | action | resource | effect | 来源 | 谁可改 |
|---|---|---|---|---|---|
| W1 | `edit` | `**` | `Allow` | `policy_from_agent_mode` 的 `workspace_write` 分支追加（`RuleOrigin::ApprovalLevel`） | 用户规则可收紧为 `ask` / `deny` |
| W2 | `edit` | `**/.env` | `Ask` | 同上 | 用户规则可放宽或收紧；**不属底线** |
| W3 | `edit` | `**/.env.*` | `Ask` | 同上 | 同上 |
| W4 | `edit` | `**/.env.example` | `Allow` | 同上 | 同上（排在 W3 之后，last-match-wins 覆盖 W3） |
| W5 | `edit` | `**/.git/**` | `Ask` | 同上 | 同上 |

- W1 的 `**` **不需要**展开工作区根：产生 `edit` 意图的工具（`agent_write` / `agent_edit` / `super_edit` / `agent_mkdir` / `agent_move` / `fs_link`）都经 `path::resolve` 的 `ensure_within_root` 强制落在工作区内，越界在解析阶段即报错、根本不进审批求值；因此 `**` 的实际作用域恰好是「工作区内全部路径」；
- W1 **只覆盖 `edit` 动作**：`delete` / `purge` / `execute` / `mcp` 不被任何 W 条目覆盖，落回序 1 的 `Ask`。删除之所以不放行，是因为「删掉工作区内的文件」不可撤销，与「可回退的写入」不同（用户可用 §3.3 快捷规则或自写 `("delete","*",Allow)` 显式放宽）；
- W2–W5 选的是「写入即可获得代码执行或凭证泄露」的两类路径：`.env*`（凭证）、`.git/**`（钩子可执行任意代码）。它们**不是**安全底线（用户规则可显式放宽，§8），只是出厂减噪规则的例外；
- Windows 下路径模式匹配按**大小写不敏感**（`.ENV` / `.GIT` 与文件系统同义）；这与 §3.2 的匹配口径一致，属匹配细节而非新合并语义。

补充事实（避免误读）：

- 序 1–3 是**主规则集前段**，序 4–8 是**末段**，W1–W5 只存在于 `workspace_write` 档并插在两者之间；用户规则固定插在 W 段之后、序 4–8 之前（§4.1），这正是「可放宽应用默认、不可放宽底线」的实现形态；
- 序 7–8 只在 Windows 命中（大小写不敏感匹配）；在 Linux/macOS 上这两条因路径不存在而永不命中，不是「漏保护」；
- 序 4–6 只覆盖「读取凭证」与「写入 `/etc`」，**不覆盖** `~/.ssh` / `~/.aws` / `~/.gnupg` / `~/.kube` 等凭证目录（取舍见 §3.1.2 D4）；
- tier 上限层不在此表（属约束层，见 §4.2 O4）。

出厂档位（`BUILTIN_AGENT_TOOL_CONFIG`）：沙箱 + `workspace_write` + 空 MCP 名单 + 空用户规则。即开箱状态是「读取与工作区写入自动执行、敏感文件与执行/删除仍确认、只能动沙箱内工具」。`builtin-default`（聊天模式）沿用 `ModeToolConfig::default()`（沙箱 + `none` + 空白名单），本次**不改**：它名单为空，档位不产生实际效果。

#### 3.1.2 参考项目默认规则对照与采纳结论

五个参考项目的出厂默认规则（均为源码事实，非文档转述）：

| 项目 | 出厂默认档 | 默认放行集 | 默认收紧 / 底线 | 出处 |
|---|---|---|---|---|
| goose | `GooseMode::Auto`（**全自动批准**） | 无内置规则；`SmartApprove` 由工具的 `read_only_hint` 注解推导：非只读 → `AskBefore` | 无内置路径底线（边界交给沙箱） | `goose_mode.rs`（`#[default] Auto`）、`config/permission.rs`（空 map + 注解推导） |
| codex | `AskForApproval::OnRequest`（**模型决定何时问**） | 项目 untrusted → `UnlessTrusted`：除 execpolicy 显式 `allow` 外全要批准 | sandbox mode 是独立轴（`read_only` / `workspace_write` / `danger_full_access`）；execpolicy `prefix_rule` 的 `decision` 缺省 `allow` | `protocol.rs`（`AskForApproval` 的 `#[default] OnRequest`）、`execpolicy/README.md` |
| openhuman | `autonomy.enabled = false`（策略整体 **opt-in**）+ `level = Supervised` | 7 个只读工具名（`file_read` / `memory_search` / `memory_list` / `get_time` / `list_dir` / `glob` / `grep`）+ 命令白名单（`git` / `npm` / `pnpm` / `yarn` / `cargo` / `make` / `cmake` + 只读 shell 命令） | `forbidden_paths`：`/etc` `/root` `/home` `/usr` `/bin` … `/tmp`、`~/.ssh` `~/.gnupg` `~/.aws` `~/.config`；`is_always_forbidden`：`.ssh` `.gnupg` `.aws` `.azure` `.kube` `keychains` 段匹配 + `Microsoft/{Protect,Credentials,Crypto,Vault}` + 系统根；`require_approval_for_medium_risk` / `block_high_risk_commands` 均 `true`，`auto_approve_all = false`，`workspace_only = true` | `config/schema/autonomy.rs`、`security/policy/path_checks.rs` |
| OpenBitFun | `PermissionMode::Ask` | `Ask` 档基线：`("*","*",Ask)` → `("read","*",Allow)` → `read` 的 `.env` 三条例外（`*/.env` `Ask`、`*/.env.*` `Ask`、`*/.env.example` `Allow`）→ `websearch` / `webfetch` / `task` / `skill` `Allow` → 9 条 git 只读子命令 `Allow` | 基线之后的 project / agent / enforced / constraint 层**只收紧** | `contracts/product-domains/src/tool_permissions.rs`（`baseline_rules()`） |
| deepseek-harness（dsh） | `permission-presets` 缺省 `workspace-write`（= `workspace-write` 沙箱 + `ask` 审批）；未组合该服务时 `sandbox-policy.mode` 缺省 `read-only` | **没有用户可编辑的规则表**。免确认面由沙箱承担：工作区内写入与 `/tmp` 不触发审批，只有「升级到更宽沙箱模式」才走审批；其余 `ask` 由部署期装配的 answerer / hook 判定，出厂无任何 allow 列表 | 无路径黑名单，边界由 OS 沙箱后端强制（bwrap / Landlock / Seatbelt / pwsh ACL）。审批结果只有 `allowed-once`，无 `allow-always`、无规则记忆、无授权存储；`never` 策略确定性拒绝。缺 answerer 时 `unavailable` 失败关闭 | `interaction/user-approval`（`ask` / `never`、`ApprovalOutcome`）、`interaction/permission-presets`（preset 表、保留名 `custom` / `auto`）、`shell/bash-sandbox`（三模式）、`docs/tool-execution-pipeline.md`（pre-execute → approval → 单调 guard） |

**采纳结论**（每条都要能对应到 §3.1.1 的某一行为「不采纳」提供理由）：

| 编号 | 结论 | 理由 |
|---|---|---|
| D1 | **保留**序 1–8 八条逐字节不变，新增规则只在新档追加 | 八条已覆盖「读取放行 + 敏感路径不可写 + 凭证不可读」的最小集；A1 的判据改为「三档 × 动作结果与改动前一致 + `workspace_write` 按 §3.1.1 W1–W5」，因此既不动既有档、又允许新档 |
| D2 | **采纳** OpenBitFun 的 `.env` 例外，但只放进 `workspace_write`（W2–W4） | 它成立的前提是「宽泛 `allow` 之后跟例外」，这正是 W1 → W2–W5 的结构。`dangerous_only` 不加：该档 `edit` 本身就是 `Ask`，例外无效果且会破坏档位文案（§7.4）。两处差异：① 模式由 `*/.env` 改为 `**/.env`——本项目 glob 引擎里 `*` 不跨分隔符，`*/.env` 只能匹配一级目录，会漏掉 `src/.env` 这类子目录；② 额外把 `.git/**` 列为例外（W5），理由是写入 `.git/**` 即可通过钩子获得代码执行 |
| D3 | **不采纳** openhuman 的命令白名单 | 命令级细分属 `shell` 工具规格（§1.2）；在 `dangerous_only` 下白名单会让「免确认执行任意 `git` 子命令」成立，绕开审批，且我们无命令解析的可信边界 |
| D4 | **本期不扩**凭证目录底线（`~/.ssh` / `~/.aws` / `~/.gnupg` / `~/.kube`） | 扩 `hard_deny_rules()` 会改变出厂行为（A1 / A3 都需重写），且归属安全基线而非审批粒度；已登记 §11，用户规则层**可以**先写 `deny` 自行收紧（规则可收紧，不受本条限制） |
| D5 | **不采纳** goose 的「默认全自动」与 codex 的「模型决定何时问」 | 前者与我们「读取与工作区写入放行、执行仍确认」的出厂承诺相反；后者把「要不要问」交给模型，等于在审批链里引入第二个决策者，与 [I38g](../../store/I38g-agent-approval-advisor-spec.md) 的单向顾问约束冲突 |
| D6 | **结构上对齐** OpenBitFun（预设档 + 基线规则 + 用户规则 + 只收紧的约束层） | 这正是本文 §4 的模型；差异只在「用户规则位置」——OpenBitFun 放在 preset 之后，我们同样如此，故无迁移成本；档数从三扩到四不影响结构 |
| D7 | **不采纳** DSH 的「无规则表、免确认面交给沙箱」 | 它成立的前提是「沙箱能表达全部免确认边界」。我们的沙箱虽有 `writable_root` 写边界（I53（已归档）），但 [I52](../../store/I52-shell-sandbox-spec.md) §2.3 硬规则 1 规定「沙箱不得改变审批决策」——`execute` 在 `workspace_write` 下恒为 `Ask`，故免确认面不能交给沙箱；且 DSH 的审批无记忆（只 `allowed-once`），我们已有授权缓存，两者不可互换。可借的只有「把沙箱与审批捆成一个用户选项」这一层——我们已由 `ModeToolConfig`（`maxToolTier` + `approvalLevel`）承担，档位命名与放行面另见 D8 |
| D8 | **采纳** DSH 的 `workspace-write` 作为出厂默认的**命名与放行面**，但不采纳其边界执行者 | 放行面（工作区写入免确认、执行与删除仍问）正是 §3.1 的 `workspace_write`。区别在边界由谁保证：DSH 靠 OS 沙箱让写入根本不进审批，我们靠 `path::resolve` 的 `ensure_within_root` 保证写入目标恒在工作区内，再由审批层显式 `Allow`。后者不依赖平台沙箱后端、跨平台一致，且保留审批链的可解释性与授权缓存 |

### 3.2 用户规则层

一条规则 = 一个 `(action 模式, resource 模式, effect)` 三元组，进入主规则集，追加在预设规则之后、硬保护规则之前（§4.1）。

- `effect = allow`：把预设的 `Ask` 放宽为自动执行；
- `effect = ask`：把预设的 `Allow` 收紧为需确认；
- `effect = deny`：直接拒绝（模型收到拒绝原因，卡片不弹出）；
- 模式匹配：`*` 不跨路径分隔符、`**` 可跨；**单独一个 `*` 或 `**` 作为整条模式时视为全匹配**（`pattern_matches` 的短路分支，快捷规则的 `edit` + `*` 能覆盖绝对路径正依赖这一点）；分隔符统一按 `/` 比较，大小写按平台判定（Windows 下按 ASCII 不敏感，与文件系统同义）；
- 顺序：**后写覆盖先写**（last-match-wins），新增规则追加在末尾，因此「后来的规则更具体、优先级更高」是可预期的；
- 适用范围：所有经 Gateway 求值的工具意图（OS 工具、MCP、`readToolOutput`），不作用于 tier 判定与白名单。

### 3.3 快捷规则（UI 层，不是配置项）

设置界面提供一组一键追加规则的按钮，写入的都是标准用户规则；**不维护任何开关状态**，因此不存在「开关与规则不一致」的可能：

| 快捷规则 | 追加的规则 | 可选效果 |
|---|---|---|
| 命令执行免确认 | `execute` + `*` | `allow` / `ask` / `deny` |
| 工作区写入免确认 | `edit` + `*` | `allow` / `ask` / `deny`（出厂 `workspace_write` 已放行；本条只对 `dangerous_only` / `all_os_tools` 有意义） |
| 读取一律确认 | `read` + `*` | `ask` / `deny` |
| MCP 调用确认 | `mcp` + `*` | `ask` / `deny`（`allow` 被 §6 E4 拒绝） |
| 工具输出回读确认 | `readToolOutput` + `*` | `ask` / `deny`（`allow` 被 §6 E4 拒绝） |

> 「按来源开关」的全部语义都能用上表表达：例如「关闭 MCP 审批」在 `dangerous_only` 下等于追加 `mcp * allow`，但该放宽被底线禁止（§8 INV-3），故快捷规则对 `mcp` 不提供 `allow`——这正是底线在 UI 上的可见形态。

### 3.4 动作词汇与资源词汇

- **动作**：生产集为 `read` / `edit` / `delete` / `purge` / `execute` / `mcp` / `readToolOutput`；`action` 字段是开放字符串（未知动作不匹配任何意图，等于无效果，天然 fail-safe），编辑器以下拉提供上述集合 + 「自定义」。
- **`delete` 与 `purge` 是两个动作**：`delete` 规则不覆盖永久删除；要一并覆盖须写两条规则。编辑器在用户只写 `delete` 时应当给出提示，但不得自动合并。
- **资源**：路径类资源为绝对路径（Windows 用 `/` 归一后比较）；MCP 资源为 `mcp:{server_id}:{tool_id}`（I38e（已归档））。编辑器对路径类动作提供路径输入与「工作区根」占位，不提供通配符拼接助手。
- **禁止**：资源以 `tier:` 开头（`tier:*` 属约束层专用语法，写进用户规则只会永远不命中，属误导配置，见 §6 E5）。

---

## 4 求值顺序与合并语义

### 4.1 装配：唯一入口

```
会话有效配置（approvalLevel + approvalRules）
  → 校验用户规则（§6）
  → 主规则集 = 预设规则（base_rules + 档位改写/追加，§3.1.1，含 workspace_write 的 W1–W5）
             + 用户规则（origin = UserRule）
             + 硬保护规则（hard_deny_rules）
  → 约束层 = [tier 上限层, 项目 permissions.json 层, MCP 已禁用层]
  → ResolvedPermissionPolicy::new(主规则集, 约束层)
```

实现约束：

- 把 `policy_from_agent_mode` 里「构造基础规则 + 档位改写/追加」的一段抽成私有 `preset_rules(approval_level) -> Vec<PermissionRule>`（含 `workspace_write` 的 W1–W5）；`policy_from_agent_mode` 改为 `preset_rules(l) + hard_deny_rules()`，签名与既有三档行为逐字节不变（A1）；
- 新增 `policy_from_agent_mode_with_rules(max_tier, approval_level, rules)` = `preset_rules(l) + rules + hard_deny_rules()`；`policy_from_agent_mode(max_tier, approval_level)` 委托给它（传空规则）；
- `policy_for_tool_config(config, workspace_root)`（`src-tauri/src/adapters/permission_policy.rs`）改为调用新函数并透传 `config.approval_rules`；这是会话级策略的**唯一装配点**，禁止在其它位置另行拼接规则；
- 用户规则的插入位置由 `_with_rules` 的拼接顺序**结构化保证**（preset → user → hard_deny），不依赖调用方排序，也不做后置排序；
- 每轮现算，改配置后下一轮生效（既有语义不变）。

### 4.2 顺序不变式

| 编号 | 不变式 |
|---|---|
| O1 | 用户规则**必须**排在预设规则之后（能放宽预设的 `Ask`） |
| O2 | 用户规则**必须**排在硬保护规则之前（不能放宽硬保护 `Deny`） |
| O3 | 用户规则**只能**进主规则集，**禁止**进任何约束层 |
| O4 | 约束层顺序与内容不变（tier → 项目 → MCP 禁用），逐层 `most_restrictive` |
| O5 | 主规则集内仍是 last-match-wins；`evaluate_rules` 未命中默认 `Allow`（由首条预设规则决定实际结论） |

### 4.3 冲突矩阵（用户规则 vs 其它收紧项）

| 用户规则 | 对抗项 | 最终结果 | 卡片原因 |
|---|---|---|---|
| `allow` | 预设 `Ask` | `Allow`（自动执行） | 不上卡片 |
| `allow` | 项目规则 `Ask` / `Deny` | `Ask` / `Deny`（项目胜） | `Ask` 时 `projectRule`；`Deny` 不弹卡 |
| `allow` | tier 上限层 `Deny` | `Deny`（tier 胜） | 不弹卡（tier 判定在意图求值之前） |
| `allow` | MCP 已禁用层 `Deny` | `Deny` | 不弹卡（该层规则投影为 `projectRule`，但 `Deny` 不上卡） |
| `allow` | 硬保护 `Deny` | `Deny` | 不弹卡 |
| `allow` | `readToolOutput` | **配置非法**（§6 E4），保存被拒 | — |
| `allow` | 授权缓存 | 无关：缓存只在策略为 `Ask` 时查询（命中即按已授权执行）；策略已是 `Allow` 则不查缓存 |
| `deny` | 授权缓存 | 立即生效：策略 `Deny` 短路返回，缓存里同键的旧 `Allow` 不再被读取 |
| `ask` | 预设 `Allow` / `none` 的 `("*","*",Allow)` | `Ask` | `userRule` |
| `deny` | 任何 `Allow` | `Deny` | 不弹卡，模型收到拒绝原因 |
| 任一 | AI 顾问（P10） | 规则结果不变；顾问只在 `Ask` 之后给建议，且 `RelaxOnly` 不得放宽用户 `deny`、`readToolOutput` | `advisor`（叠加，不替换） |

> 授权缓存与规则的关系只有一条：**缓存只存「已允许」，没有「已拒绝」**。因此用户新增 `deny` 规则对已授权的调用**立刻生效**（策略先求值、`Deny` 短路，缓存根本不参与），不需要清缓存；反过来，`allow` 规则只是让调用不再走到缓存查询。

### 4.4 解释与去重

`ask_reasons` 保持既有语义：`policy.explain(action, resource)` 对**主规则集与每个约束层各取实际生效的那一条**（单层 last-match，顺序为主规则集 → 约束层依次），只有 `effect = Ask` 的规则进入原因列表，原因相等即去重。用户规则命中 `Ask` 时产生 `UserRule { action, resource }`，与项目规则产生的 `ProjectRule` **不得合并成同一条原因**（用户需要分清「我写的」与「项目规定的」）。

---

## 5 DTO 与持久化

### 5.1 新增类型（设计唯一来源，落地后以生成物为准）

**先改既有枚举**（六个代码点必须同一批改动，A14）：

| # | 位置 | 改动 |
|---|---|---|
| 1 | `assistant-protocol/src/agent.rs` `ApprovalLevelDto` | 追加 `WorkspaceWrite`（`snake_case` → `workspace_write`） |
| 2 | 同文件 `impl From<&str> for ApprovalLevelDto` | 追加 `"workspace_write" => Self::WorkspaceWrite`；未知值兜底仍为 `AllOsTools`（fail-closed 不变） |
| 3 | `assistant-tools/src/runtime/policy.rs` `ApprovalLevel` | 追加 `WorkspaceWrite`，`as_str()` 追加 `"workspace_write"`（该值进审批卡片原因投影，i18n 的 `level` 组必须有对应键），并修正 `DangerousOnly` 的文档注释（G2） |
| 4 | `assistant-tools/src/dto_bridge.rs` `From<ApprovalLevelDto> for ApprovalLevel` | 追加分支（穷尽 `match` 会强制补齐） |
| 5 | `assistant-storage/src/agent_modes.rs` `ApprovalLevel`（镜像） | 追加 `WorkspaceWrite` |
| 6 | `src/chat/core/toolConfig.ts` `APPROVAL_LEVELS` | 追加 `"workspace_write"`；该数组「顺序即 UI 展示顺序」，按由宽到严插在 `none` 之后 |

之后跑 `npm run contract:gen`（`ApprovalLevelDto` 是 `TS` 导出类型，前端联合类型由生成物提供）。

再新增规则类型：

`src-tauri/crates/assistant-protocol/src/agent.rs`：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalRuleEffectDto {
    Allow,
    Ask,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalRuleDto {
    pub action: String,
    pub resource: String,
    pub effect: ApprovalRuleEffectDto,
}
```

- `ApprovalRuleDto` 是**输入 DTO**：`deny_unknown_fields`（与 `ConversationToolConfigDto` 一致），无 `Option` 字段，不需要 `#[ts(optional = nullable)]`；
- `ApprovalRuleEffectDto` 是封闭集合：未知取值反序列化失败（fail-closed），前端不得兜底成 `allow`。

### 5.2 字段追加

`ConversationToolConfigDto`（同一文件）追加：

```rust
#[serde(default)]
pub approval_rules: Vec<ApprovalRuleDto>,
```

- `#[serde(default)]`：旧 JSON（无该键）可读，语义等价于「无用户规则」；
- 输出方向键恒在（`Vec` 序列化为 `[]`），与「输出 `Option` 键恒在」的口径同向，前端声明 `approvalRules: ApprovalRuleDto[]` 必填；
- `Default` 实现追加 `approval_rules: Vec::new()`，与既有 fail-closed 缺省（沙箱 + `none` + 空名单）一致。

`ApprovalReasonDto` 追加分支（`src-tauri/crates/assistant-protocol/src/agent.rs`）：

```rust
UserRule { action: String, resource: String },
```

### 5.3 存储与内置种子

- `assistant-storage/src/agent_modes.rs` 的 `ModeToolConfig` 追加同名字段（`#[serde(default)]`），并新增镜像类型 `ModeApprovalRule` / `ModeApprovalRuleEffect`；**禁止**让 `assistant-storage` 依赖协议层（既有分层约束）；
- `ModeToolConfig::default()` 与 `BUILTIN_AGENT_TOOL_CONFIG` 的规则列表**保持为空**（用户规则层出厂为空）；但 `BUILTIN_AGENT_TOOL_CONFIG.approval_level` 由 `DangerousOnly` 改为 `WorkspaceWrite`（§3.1.1 出厂档位）。这是**唯一一处种子值变更**，`ModeToolConfig::default()`（聊天模式）保持 `none` 不变；其上方文档注释「沙箱层级 + 仅危险操作审批」须同步改为「沙箱层级 + 读取与工作区写入放行」；
- `dto_bridge` 负责 `ModeApprovalRule` ↔ `ApprovalRuleDto` 转换；`agent_modes.tool_config_json` 的实际落库形态是 `ConversationToolConfigDto` 的 JSON（app 层 `save_agent_mode` 直接序列化协议 DTO），因此镜像只服务于种子与解码兼容。

### 5.4 前端解析、默认与覆盖语义

| 位置 | 必须改动 |
|---|---|
| `DEFAULT_TOOL_CONFIG` | 追加 `approvalRules: []`；`approvalLevel` 保持 `none`——它是「模式未找到」的 fail-closed 兜底，不是出厂档位（出厂档位在 `BUILTIN_AGENT_TOOL_CONFIG`） |
| `resolveEffectiveToolConfig` | **必须显式复制 `approvalRules`**（该函数逐字段重建对象；漏字段等于静默丢用户规则，属本规格最高优先级的回归点） |
| `ToolConfigDiff` / `diffToolConfig` | 追加 `rulesChanged: boolean`（按规则三元组序列相等比较），`isEmptyToolConfigDiff` 同步纳入 |
| 变更提示文案 | 规则变化时提示「审批规则已更新，下一轮生效」，不得只提示档位 |
| 会话 → 模式覆盖 | 会话级配置整体覆盖模式级配置（含规则），**不做逐条 merge**：合并会产生「用户删掉的规则又回来了」的不可解释行为 |

### 5.5 旧数据兼容

| 情形 | 行为 |
|---|---|
| `tool_config_json` 为 `NULL` / 无 `approvalRules` 键 | 解析为 `[]`，等价于现状（A1） |
| `approvalLevel` 未知字符串 | 沿用 `ApprovalLevelDto::from` 的 fail-closed → `all_os_tools`（不变） |
| `approvalLevel` 为 `dangerous_only` / `none` / `all_os_tools` 的**已存记录** | **不迁移**：用户已做出的选择不被静默改写。改档只能由用户在设置界面主动操作（出厂默认只作用于新库 / 新模式的初始化） |
| `tool_config_json` 整体解析失败（含非法规则） | 沿用 `AgentModeDto::from` 的 `.ok().unwrap_or_default()` → 沙箱 + `none` + 空名单；因 tier 为沙箱、OS 工具不可执行，整体仍是 fail-closed（A6 的存储侧） |
| 规则含未知 `effect` 字符串 | 整体解析失败 → 上一行行为（不得降级为 `allow`） |

> **出厂默认的作用范围（明确取舍）**：新档位只通过 `BUILTIN_AGENT_TOOL_CONFIG` 影响**新建库**的 `builtin-agent` 初始化。存量库里 `builtin-agent` 的行**不动**——因为无法区分「用户没改过」与「用户主动选了 `dangerous_only`」，无条件改写就是静默改档。因此存量用户要拿到新的开箱体验，必须在设置界面主动把档位改成「工作区写入」一次。这一点在升级说明里写明，不做自动迁移（若产品后续要求「存量也升级」，必须先引入「用户是否显式改过」的标记，属另一项设计）。

---

## 6 校验与失败语义

### 6.1 校验规则

| 编号 | 规则 | 违反时 |
|---|---|---|
| E1 | `rules.len() <= 64` | 拒绝保存（超出上限提示，不截断） |
| E2 | `action` 非空且长度 `<= 32` | 拒绝保存 |
| E3 | `resource` 非空且长度 `<= 256` | 拒绝保存 |
| E4 | `effect = allow` 时，`action` 模式**不得**匹配 `readToolOutput` 或 `mcp`（含 `*` / `**` 通配） | 拒绝保存（INV-3） |
| E5 | `resource` 不得以 `tier:` 开头 | 拒绝保存（§3.4） |
| E6 | `effect` 必须是三个已知值之一 | 反序列化失败（fail-closed） |

> E4 的直接后果：`("*","*",Allow)` 不可写——想全放行请把档位设为 `none`，规则层不提供等价写法。这是刻意的：让「一刀切放宽」只能发生在**档位**这个一眼可见的位置。

### 6.2 两个时机，两套语义

| 时机 | 位置 | 语义 |
|---|---|---|
| 保存时 | `save_agent_mode`（`src-tauri/src/cmd/agent_mode.rs`）+ 前端提交前预校验 | **拒绝**：返回字段级错误，整条配置不落库，不得部分写入 |
| 运行时 | `policy_for_tool_config` / 每轮装配 | **fail-closed**：任何校验失败 → 忽略规则并把档位提升为 `all_os_tools`，同时 `log::error`；不得 panic、不得静默按 `allow` 执行 |

> 运行时降级为什么是「提升档位」而不是「退化为空规则」：空规则意味着按用户选的低档位执行，而低档位正是规则想放宽的对象；配置不可信时只能取最严。此路径必须与 `load_project_permissions` 的「非法即 `("*","*",Deny)`」区分：项目文件非法 → 全拒绝；用户规则非法 → 全审批（因为规则只能放宽，收紧项仍在）。

### 6.3 失败矩阵

| 输入 | 结果 | 是否执行工具 |
|---|---|---|
| 合法配置 | 正常求值 | 按求值结果 |
| 规则条数超限 | 运行时降级 `all_os_tools` + 空规则 | `Ask` |
| `effect` 未知 | 解析失败 → 存储侧 `default()`（沙箱 + `none`） | 沙箱内工具可用，OS 工具被 tier 拒绝 |
| `allow` 命中 `readToolOutput` | 运行时降级 `all_os_tools` + 空规则 | `Ask`（`readToolOutput` 仍 `Ask`） |
| 项目文件非法 | 项目层 `("*","*",Deny)` | 拒绝 |
| 工作区缺失 | 无项目层（既有行为） | 按配置求值 |

---

## 7 解释链与 UI

### 7.1 规则来源与原因映射

- `RuleOrigin` 追加 `UserRule`（无载荷）；
- `reason_from_rule`（`runtime/gateway/context.rs`）追加分支：`RuleOrigin::UserRule` → `ApprovalReason::UserRule { action, resource }`；
- 既有 `Config` 分支的 `ProjectRule` 投影**不得**再用于用户规则（用户规则不得伪装成项目规则）；
- 前端 `ApprovalCard.tsx` 的 `reasonText` 是穷尽 `switch`（无 `default`），新增分支由类型检查强制补齐；
- i18n 追加 `chat.tool.approval.reason.user_rule`（`{action}` / `{resource}` 两个参数），`zh-CN` 与 `en-US` 同时补齐。

### 7.2 审批卡片

- 卡片仍只展示 `Ask` 命中原因（`Allow` 规则不上卡，既有语义不变）；
- 用户规则与项目规则同时命中时，两条原因并列展示，顺序为主规则集先于约束层；
- 卡片**不提供**「按此规则长期生效」按钮：写规则必须回到设置界面（卡片只写授权缓存，§1.2）。

### 7.3 规则编辑器

- 会话级（`ToolConfigPanel`）与模式级（`ToolAssemblySection`）复用**同一个**编辑器组件（`ApprovalRulesEditor`），保证两处语义一致；
- 表格三列：动作（下拉 + 自定义）、资源（文本输入）、效果（下拉：自动执行 / 每次询问 / 直接拒绝），行尾删除；
- 必须显示序号与「后写覆盖先写」提示；新增追加末尾；是否支持拖动排序属实现细节，但**不得改变**「顺序即生效顺序」的语义；
- `deny` 行与 `allow` 行在视觉上必须可区分（危险色 / 中性色），效果文案不得用「允许 / 拒绝」这类易与工具白名单混淆的措辞；
- 前端预校验与 §6.1 同规则（同一份错误文案来源），提交后由后端复校验（后端为准）。

### 7.4 档位文案修正

四个档位的说明文案必须描述**实际语义**：

| 档位 | 选项名（zh-CN） | 说明（zh-CN 语义，实现时按 i18n 现有措辞风格落地） |
|---|---|---|
| `none` | 直接运行 | 全部自动执行（工具输出回读除外） |
| `workspace_write` | 工作区写入 | 读取与工作区文件修改自动执行；执行命令、删除、MCP 调用与工具输出回读需确认；敏感文件（`.env*`、`.git/**`）修改需确认 |
| `dangerous_only` | 谨慎 | 读取自动执行；其余（写入、执行、删除、MCP）需确认 |
| `all_os_tools` | 手动审批 | 所有 OS 工具调用都需确认 |

落点：`src/i18n/locales/{zh-CN,en-US}.json` 的两组 `approval_level.options` / `level`（设置页模式编辑与工具装配两处各一份）+ 两组 `level_hint`。**`dangerous_only` 现有选项名是「默认 / Default」**——它不再是出厂默认，必须改名（否则用户会以为选它才是默认行为）；同一处文案里凡是「默认」字样都要一并清理。

禁止沿用「危险操作 / 开放式操作」这类指向工具 flag 的措辞（G2）；`workspace_write` 的文案必须写出敏感文件例外，否则用户会以为 `.env` 也能免确认。`policy.rs` 中 `ApprovalLevel::DangerousOnly` 的文档注释「仅 `destructive` / `open_world` 需审批」同样要改为「读取放行、其余确认」（§3.1 的**必须**项）。

### 7.5 项目规则编辑视图

设置界面提供 `.assistant/permissions.json` 的编辑视图：文件路径、是否存在、解析是否成功，以及**与模式级审批规则完全同一套的 `ApprovalRulesEditor`**（同一份校验、同一份错误文案）。用户可在界面内增删改规则并保存；文件不存在时保存会连同 `.assistant/` 目录一起创建，不再要求用户先用外部编辑器手工建文件。落盘格式保持 `{ "rules": [...] }`，用户仍可手工编辑同一份文件，两条写入路径互不排斥。

目的：一是当用户写了 `allow` 却仍被询问时，能自己看到「项目规则更严」（§4.3 第二行）；二是让项目规则的维护不必依赖「文件已存在」这一前置条件。

非法规则（超条数、触碰底线动作等）整体拒绝、不写盘；文件已存在但无法解析时，界面从空规则开始编辑，并保留原文供定位错误。

---

## 8 不可放宽底线

| 编号 | 底线 | 违反后果 |
|---|---|---|
| INV-1 | 硬保护规则（`/etc/shadow`、`/etc/ssh/**`、`/etc/**` 写入、`C:/Windows/**`、`C:/Program Files/**`）在任何档位与规则下恒 `Deny` | 规则顺序被破坏 → 安全回归，A3 失败 |
| INV-2 | tier 上限层恒生效：沙箱档看不到 `tier:os` / `tier:external`，`os` 档看不到 `tier:external` | 规则进约束层 → A4 失败 |
| INV-3 | `readToolOutput` 恒 `Ask`（[I52](../../store/I52-shell-sandbox-spec.md) §5.11.4(3)）；`mcp` 不得被用户规则放宽为 `allow`（MCP 信任由授权缓存的「本会话允许」承担） | `allow` 规则被接受 → A5 失败 |
| INV-4 | 项目级 `permissions.json` 恒只能收紧；非法即全拒绝 | 项目层被跳过 → A4 失败 |
| INV-5 | 已禁用 MCP server 的 `Deny` 层恒生效 | 规则覆盖 → A4 失败 |
| INV-6 | AI 顾问不得放宽用户 `deny` 与 `readToolOutput`，且不写规则 | [I38g](../../store/I38g-agent-approval-advisor-spec.md) 单向约束被破坏 |
| INV-7 | 规则不改变工具可见性：未注入 / 已禁用的工具不会因规则而出现 | 白名单被绕过 → A12 失败 |
| INV-8 | 运行时规则校验失败（解码成功、校验不过）一律 fail-closed 到 `all_os_tools`；解码失败则整条配置退缺省装配（沙箱 + `none` + 空规则）。两条路径都**没有**「忽略校验、按原档位继续」的选项 | A6 失败 |

---

## 9 验收标准

| 编号 | 判据 | 落点 |
|---|---|---|
| A1 | 空规则下 `policy_from_agent_mode` 对 `none` / `dangerous_only` / `all_os_tools` 三档 × 各动作的结果与改动前**完全一致**；`workspace_write` 档按 §3.1.1 W1–W5 求值；`builtin-agent` 种子 JSON 仅 `approvalLevel` 一处变更 | `assistant-tools` 策略单测；`assistant-storage/tests/agent_modes.rs` |
| A2 | `workspace_write` + `("edit","docs/a.md",…)` → `Allow`；`workspace_write` + `.env` → `Ask`；`dangerous_only` + `("edit","*",Allow)` → 写入自动执行；`none` + `("edit","*",Ask)` → 写入需确认 | `assistant-tools` 策略单测 |
| A3 | 任何规则都无法把 `/etc/**` 写入或 `/etc/shadow` 读取变成非 `Deny` | 同上（含 `("*","*",Allow)` 与 `("edit","/etc/**",Allow)`） |
| A4 | `allow` 规则遇 tier 上限 / 项目 `Deny` / MCP 禁用层时仍为 `Deny`；项目文件非法时全拒绝 | `src-tauri/tests/permission_policy.rs` |
| A5 | `("readToolOutput","*",Allow)`、`("mcp","*",Allow)`、`("*","*",Allow)` 均被拒绝 | 校验单测（后端 + 前端预校验） |
| A6 | 规则条数 / 长度超限（解码成功、运行时校验失败）→ 忽略规则并把档位升为 `all_os_tools`；`effect` 未知（解码失败）→ 缺省装配（沙箱 + `none` + 空规则）；两条路径均不 panic 且 `log::error` | 装配单测 + 日志断言 |
| A7 | 两条冲突规则按书写顺序 last-match-wins；`ask_reasons` 与求值同源 | 策略单测 + `context` 单测 |
| A8 | 用户规则命中 `Ask` 时原因为 `userRule`（不是 `projectRule`），且与项目规则原因不合并 | `context` 单测 + 前端 `ApprovalCard` 单测 |
| A9 | 旧 JSON（无 `approvalRules`）可读；输出恒含该键；未知 `effect` 解析失败 | `assistant-protocol` 契约测试（`crates/assistant-protocol/tests/protocol_bindings.rs`，decl 注册） |
| A10 | `resolveEffectiveToolConfig` 不丢规则；`diffToolConfig.rulesChanged` 正确 | `src/chat/core/__tests__/toolConfig.test.ts` |
| A11 | 规则上限 64 / 长度限制在保存时拒绝且不部分写入 | `cmd/agent_mode` 测试 |
| A12 | 规则不让未注入工具变得可用；规则不放宽白名单 | 工具解析 / Gateway 集成测试 |
| A13 | §3.1.1 的序 1–8 八条与 `base_rules()` / `hard_deny_rules()` 逐条一致，W1–W5 五条与 `workspace_write` 分支一致（条数、`action`、`resource`、`effect`、顺序） | 评审项（改默认规则集必须同时改表；不设自动化） |
| A14 | `ApprovalLevel` 在协议枚举 / `From<&str>` / `assistant-tools` 枚举 / `dto_bridge` 映射 / `assistant-storage` 镜像 / `toolConfig.ts` 六处同名同值（wire 值 `workspace_write`），`contract:gen` 生成物含该值 | 契约测试 + 评审项 |
| A15 | 已存 `dangerous_only` / `none` / `all_os_tools` 记录的 `approvalLevel` 不被迁移；新建库的 `builtin-agent` 为 `workspace_write` | `assistant-storage/tests/agent_modes.rs` |
| A16 | 四档在 `zh-CN` / `en-US` 各两处选项名与说明齐备，`dangerous_only` 不再叫「默认 / Default」，`workspace_write` 说明含敏感文件例外 | 评审项 + i18n 键存在性单测 |

> 通用门禁：`npm run contract:gen` 生成物已提交且无差异、`cargo clippy`、`npm run typecheck`、`npm run lint`、`vitest`、`cargo test`（cwd = `src-tauri`）。

---

## 10 与相邻规格的关系

| 规格 | 关系 |
|---|---|
| [I38g](../../store/I38g-agent-approval-advisor-spec.md) | 顾问在策略求值**之后**运行；用户 `deny` 为终局，顾问不得放宽；顾问不读写规则（INV-6） |
| [I52](../../store/I52-shell-sandbox-spec.md) | 沙箱不替代审批；`readToolOutput` 恒 `Ask` 由本文 INV-3 承接 |
| I38d（已归档） | 审批卡片 L0–L3 结构不变，本文只追加一种 `reason`；`policy_for_tool_config` 装配顺序在本文 §4.1 处补一层 |
| I38e（已归档） | MCP 精确键 `mcp:{server}:{tool}` 不变；禁用层语义不变 |
| [I69a](../../store/I69a-agent-orchestration-base-spec.md) | 编排层保持**调用级**授权，不做图级 / 批量授权；规则仍是单次调用求值 |
| I58（已归档） | 白名单与元工具注入规则不变；规则不参与可见性（INV-7） |

---

## 11 未决与后续（本文不做）

本表只登记**本文明确不做、但已能看出归属**的项，每行一个方向而不是一个功能点；方向内的具体形态留到立项时再定。

| 方向 | 包含 | 触发条件 |
|---|---|---|
| 授权作用域细化 | 工作区级 / 目录级授权缓存、MCP server 级信任（`mcp:{server}:*`）、按命令参数细分（`execute` + 命令白名单） | 出现「同目录 / 同 server 反复确认」的实测打扰数据；MCP 与命令两类还须先解决 INV-3 的表达方式与命令解析的可信边界（后者属 `shell` 工具规格） |
| 规则共享与继承 | 模式 → 会话的逐条 merge、规则模板、规则编辑入口是否收敛到单一位置 | 出现「多模式共用一套规则」的真实诉求；首期两处入口复用同一组件（§7.3）即可 |
| 配置的写入与度量 | 项目级 `permissions.json` 的 UI 写入口、审批打扰指标（问了多少次 / 放行多少） | 写入口需要文件锁与冲突合并策略；度量需要审计侧指标规格（I50（已归档）） |
| 默认规则集扩展 | 凭证目录底线（`~/.ssh` / `~/.aws` / `~/.gnupg` / `~/.kube`）、`.env` 类敏感文件的**读取**例外（§3.1.2 D2 / D4）、`workspace_write` 的敏感路径清单是否继续扩大（`*.pem` / `id_rsa` / `Dockerfile` / `.github/workflows/**` 等） | 前者归安全基线（I52），须先定「跨平台凭证目录清单」与命中规则；`.env` 的**写入**例外已由 W2–W5 落地，**读取**例外仍不做（只在 `read` 默认为 `Ask` 的档位成立，与 `workspace_write` 的「读取放行」承诺冲突）；敏感路径清单的扩大须有实测误伤数据支撑 |
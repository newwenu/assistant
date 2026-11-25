# I72 — 审批卡片决策化改版设计

| 项 | 内容 |
| --- | --- |
| 状态 | 设计定稿 v2.6（可实施） |
| 修订 | v2.1 按代码核对；v2.2 移除 ToolBox-only 工具并新增 §2.5 受众执行期强制；v2.3 前后端同批交付、取消按数据可得性缩减范围；v2.4 补齐 MCP 动词来源、`summary` 共存规则、shell 边界段生效条件；v2.5 敲定 `extensions` 删 `label`/`value`、`shell` 的 `summary` 置空、范围说明不做常驻偏好项；v2.6 补「世界」定义、折叠区固定顺序（§4.5），并修正 `find_by_name` 三处、`kind.code` 措辞对齐 |
| 日期 | 2026-11-23（v2；v2.1–v2.6 修订 2026-11-24） |
| 关联 | I38d-agent-frontend-modes-spec（已归档，审批信息分级与四动作硬约束）、I70 审批粒度与原因归因、I71 工具执行链路加固、I58 §3.6 super_edit 升级；任务级进度见 [I72-approval-card-decision-tasklist.md](./I72-approval-card-decision-tasklist.md) |
| 影响范围 | 前端：`src/chat/tool/ApprovalCard.tsx`、`ApprovalCard.module.css`、`src/chat/tool/approvalPresentation/`（新增目录模块，见 §10.1）、`src/i18n/locales/*.json`、拒绝理由可选输入（§8.5，wire 字段已存在）。后端：`agent_delete` 校验位置上移、`find_for_agent` 受众强制（§2.5）、`preview` 结构化（规模 / 目标类型 / 链接 / `verbCode` / shell 语义与目标范围）与 `extensions` 改 `kind + code`（§10.2，含 `contract:gen`）。两侧同批交付 |
| 核心结论 | 卡片只回答两件事：**批准后世界会怎样变**、**我在授权什么范围**。常态 3 行、硬顶 4 行。字段去留由**区分度**决定，不由「行数能不能砍」决定 |
| 展示决策归属 | **三层分离**（§2.4）：内容由数据决定、语义由参数字典决定、呈现由前端决定。前端不产生事实，也不产生语义 |

## 目录

| 章节 | 内容 |
| --- | --- |
| §0 数据来源 | 卡片只消费 wire 上真实存在的字段；本次补齐清单与铁律 |
| §1 问题陈述 | S1–S13 现状症状、三层根因 |
| §2 决策模型 | 两问判据（含「世界」定义）、什么不属于审批信息、三个筛选判据、三层分离、受众执行期强制 |
| §3 信息处置表 | 通用字段处置、常量字段清单、`extensions` 收敛为 `kind + code` |
| §4 卡片结构 | 行预算、决策句求值、布局草图、尺寸核算、折叠区固定顺序 |
| §5 要素求值与参数语义 | 决策要素、参数语义字典、证据形态、求值示例、`TOOL_PROFILES`、真实卡片形态 |
| §6 shell 语义理解 | 四层模型、效果集、目标与置信、实现顺序 S1–S4 |
| §7 原因翻译 | 六种原因 → 一句主句、合并规则 |
| §8 授权动作与范围 | 四按钮、范围说明、二次确认、拒绝后果、拒绝理由、超时态 |
| §9 降级与不变式 | N1–N19、R1–R2 |
| §10 改动面 | 前端文件、后端议题、设计决策 D1–D11 |
| §11 验收判据 | A 纯函数 / B 结构 / C 双语 / D 手工 / E 拒绝理由 |
| §12 不做项 | 明确排除的范围 |

---

## 0. 数据来源

卡片只消费 wire 上真实存在的字段，否则就是前端自造事实。逐项核对来源（「本次补齐」= 由 §10.2 在同一批交付中提供）：

| 事实 | 来源 | 依据 |
| --- | --- | --- |
| `arguments`（模型原文） | 现有 wire | `ToolApprovalRequest.arguments` |
| `intents`（`action` + `resources`，resources 为**绝对路径**或命令原文） | 现有 wire | `PermissionIntentDto` 只有这两个字段 |
| `behavior` / `tier` / `reasons` | 现有 wire | `ApprovalContext` |
| `preview.summary` / `truncated` / `estimated` | 现有 wire | `ApprovalPreviewDto` |
| `preview.entries` / `bytes` / `targetKind` / `exists` / `symlink` / `verbCode` | 本次补齐（§10.2） | 当前不存在这些字段，只在 `summary` 散文里 |
| `extensions`（`workspace` / `sandbox.enforcement` / `sandbox.root` / `output.scope`） | 现有 wire（`kind` + `label` + `value` + `severity`，其中 `label` / `value` 为中英混排散文） | `ApprovalExtensionDto`；本次改为 `kind` + `code` + `severity` 并删 `label` / `value`，`sandbox.root` / `output.scope` 停止产出（§10.2） |
| 文件是否存在 / 是否为链接 / 目录条目数与字节数 | 本次补齐（§10.2；前端无文件系统知识） | 判据在 `agent_delete/scan.rs`、`agent_write/preview.rs`，当前只进散文 |
| shell 命令类别、语句数、重定向目标、目标范围 | 本次补齐（§10.2；当前只有 `summary` 散文里的 `label_zh`） | `summary_for` 输出散文；结构解析见 §6 |
| 拒绝理由的**输入**字段 `ToolApprovalDecisionDto.reason` | 现有 wire（后端链路完整，仅前端未使用） | [agent.rs](../../../src-tauri/crates/assistant-protocol/src/agent.rs) + [pipeline.rs](../../../src-tauri/crates/assistant-tools/src/runtime/pipeline.rs)；见 §8.5 |

由此确定一条铁律：**卡片只消费 wire 上真实存在的字段，前端不产生事实**。表中标「本次补齐」的字段由 §10.2 的后端改动在同一批交付（含 `contract:gen`），是本设计的组成部分。字段值为 `null` 时，唯一允许的表现是该要素无值即省略（N14）——不允许前端猜值，也不允许用「字段缺失」去缩小设计范围。

具体到要素层面：

1. **规模段**（`N 个文件 + M 个子目录`、`3.2 KB`）与**「目标为链接」「覆盖 vs 新建」**这类需要文件系统知识的事实，只能来自 `preview` 结构化字段；该字段由本次 §10.2 补齐，前端不得自行推断。
2. **shell 行 1 的类别 / 语句数 / 串联数**只能来自 §6.7 的结构化输出；该结构由本次 §10.2 上 wire，前端不解析散文。

行 1 的事实来源收敛为四处：`arguments` 可证的参数语义（§5.2）、`intents` 的对象（取末段）、`preview` 的结构化字段（规模 / 后果 / shell 类别与边界，§10.2）、`extensions[workspace]` 的越界结论。其余要素无值即省略（N14）——这不是妥协，正是「要素无值整段消失」的规则在数据边界上的自然结果。

---

## 1. 问题陈述

### 1.1 现状渲染路径

后端把审批上下文装配成 `ToolApprovalRequest`（`toolName` / `arguments` / `intents` / `context{tier,behavior,reasons,preview,extensions}`），前端 [ApprovalCard.tsx](../../../src/chat/tool/ApprovalCard.tsx) 用同一套 `row`（`label` + `value`）把六类信息平铺输出。信息**有**，但没有回答「我该不该点允许」。

### 1.2 具体症状

| # | 现象 | 根因 | 用户代价 |
| --- | --- | --- | --- |
| S1 | 首屏没有「要做什么」的句子，只有固定文案「需要你的确认」 | 标题不承载动作与对象 | 必须自己读工具名和参数才知道发生什么 |
| S2 | 分组按**数据来源**（预览/资源/影响/检测/原因）而非**决策问题** | 装配顺序照抄后端结构 | 认知顺序与决策顺序错位 |
| S3 | `edit` / `super_edit` 的多行 diff 被压成一行 | `preview.summary` 含 `\n`，但渲染进 `<span class="value">` 且 CSS 无 `white-space` 保留 | **最关键的证据直接丢失**，用户看不到改了什么 |
| S4 | 规模（行数/字节/条目）与「永久删除」等权 | 同一 `row` 字号字色 | 严重度信息被淹没 |
| S5 | 原因是策略术语（`approvalLevel` / `defaultPolicy` + 原始 rule 串） | 直接透出后端枚举与规则字符串 | 用户不知道「为什么问我」 |
| S6 | 不可逆只有 `destructive` 标签，没有后果句 | 标签只描述分类，不描述后果 | 无法判断能不能撤销 |
| S7 | 授权范围（一次/本会话/始终）藏在 11px 灰字 `always_hint` | 固定两行提示，与具体授权键无关 | 不知道「允许」到底放行了什么 |
| S8 | `readOnly` 正常态与异常态共用一行 | 无异常判定 | 诊断信息占用首屏，正常态又是零信息 |
| S9 | 中英混排：`delete` 预演是英文（`move the file at … to the system recycle bin`），`extensions` 标签中英混（`隔离强度` / `Output`） | 各工具 `preview_for_approval` 与 `extensions` 各自硬编码语言 | 中文界面出现英文句。本次把 `extensions` 收敛为 `kind + code`、`shell` 的 `summary` 置空并改由结构化字段承载（§5.3、§10.2）；卡片侧本就不解析散文 |
| S10 | 没有「拒绝后会怎样」 | 未设计 | 用户担心拒绝会中断整个任务，于是倾向于点允许 |
| S11 | 每张卡都重复同一批**常量警告** | 见 §3.2：`delete` 的 `destructive` 恒真、`shell` 的 `destructive` + `openWorld` 恒真、`super_edit` 的 `destructive` 恒真 | 警示失效——用户学会无视告警条 |
| S12 | `shell` 的命令原文被放在「范围」语义下 | 命令原文**已在** `intents[0].resources`（`PermissionIntent::new("execute", parsed.command)`），但卡片把它渲染进「范围」行、与 `execute` 动作名混排、无等宽块、无截断控制 | 用户要么在 285px 行内读不到命令尾部，要么把「我授权 execute」误读成「我授权这个命令」 |
| S13 | 授权范围行把**绝对路径全量**列出 | `intents.resources` 是解析后的绝对路径（策略按绝对路径匹配），卡片原样输出 | 卡片被截图/转发时带出本机目录结构；用户也读不出末段对象 |

### 1.3 根因分三层

| 层 | 问题 | 后果 |
| --- | --- | --- |
| 选择 | 按「后端有哪些字段」决定展示什么 | 常量字段、默认值、技术术语全部上首屏 |
| 包装 | 字段按来源平铺，无决策句、无视觉权重 | 高价值事实（永久删除、工作区外）被淹没 |
| 证据 | 最关键证据要么被压扁（S3）要么缺席（S12） | 首屏信息多但无法支撑决定 |

三层必须一起修：只做「删行」（层一）会把证据一起删掉；只做「加证据」（层三）会让卡片更高。本设计对三层的处置分别是 §3（选择）、§4（包装）、§5（证据）。

---

## 2. 决策模型

### 2.1 判据：卡片只回答两个问题

| 问题 | 内容 | 例 |
| --- | --- | --- |
| Q1 批准后世界会怎样变？ | 后果 | 永久删除 → 文件消失且不可恢复；覆盖 → 原内容被替换 |
| Q2 我在授权什么范围？ | 授权 | 「始终允许」会放行 `delete` + `purge` 两个键 |

Q1 是**陈述句**（不是疑问句）：用户不需要自己从「没看到永久字样」反推可恢复，卡片必须给互斥结论。

**「世界」的定义**：指**本次调用执行后可观察的外部状态**，只含四域——① 文件系统（文件消失 / 内容被替换 / 目录多出条目）、② 不可逆性（回收站可恢复 vs 直接删除不可恢复）、③ 系统边界（出网 / 越出工作区 / 沙箱降级）、④ **授权集合**（「始终允许」写入策略后，同类调用不再询问）。**不含「这次调用能否成功」**——那是有效性，归 §2.2 的校验阶段。Q2 之所以也属「世界变化」：它不改文件，但改变后续所有审批的判定结果。

### 2.2 什么**不**属于审批信息

| 类别 | 例 | 归属 |
| --- | --- | --- |
| 有效性 / 校验 | 「目录非空且未加递归，本次会失败」 | **校验阶段**：应在 `validate_input` 拦下，用户根本看不到卡片（§10.2） |
| 诊断 | `readOnly` 与工具语义冲突、预演超时降级 | 折叠区，不占首屏 |
| 预演耗时 | 预演超时 / 降级为 `None` | 不进卡片，只记日志 |
| 工具内部实现 | 临时目录名、shell 程序路径、可写根 | 不进卡片 |

**分界线**：`permanent=true` 是后果（数据永久消失）→ 进卡片；「目录非空会失败」是有效性（调用能否成功）→ 不进卡片。而且正是「回收站不可用时直接失败、不降级为永久删除」这条后端保证（[agent_delete/mod.rs](../../../src-tauri/crates/assistant-tools/src/os/tools/agent_delete/mod.rs) 工具描述），才让卡片可以放心写「可恢复」——**保证本身不需要展示**。

### 2.3 三个筛选判据

**判据 A：区分度。** 同一工具的所有审批里取值恒定的字段 = 零信息，一律不显示。

| 工具 | 恒定字段 | 处置 |
| --- | --- | --- |
| `delete` | `behavior.destructive = true` | 删除本身就是破坏性操作，「删除」二字已承载，不写 |
| `shell` | `behavior.destructive = true`、`behavior.openWorld = true` | 每张卡都报「破坏性 + 会访问工作区外」→ 警示失效（S11） |
| `super_edit` | `behavior.destructive = true` | 同上，常量告警 |
| 所有工具 | `behavior.readOnly = false`（除只读类） | 正常态零信息 |

**范围限定**：`fs_link` 是 `TOOLBOX_ONLY`——ToolBox 直调不经网关、不产生审批卡片（§2.5），因此不进本表，也不进全文任何求值规则。

**判据 B：反向默认。** 正常态恒为默认值的结论不显示，只显示异常。

| 字段 | 默认值 | 显示条件 |
| --- | --- | --- |
| `extensions[workspace]` | `code=inside`（`Info`） | 仅 `Caution`（`code=outside` / `partial`）时以危险色标记 |
| `extensions[sandbox.enforcement]` | `code=full`（`Info`） | 仅 `Caution`（`code=partial`）时进折叠区 |
| `extensions[sandbox.root]` | 当前工作区末段名 | 产出点删除（恒等于工作区，见 §3.1） |
| `extensions[output.scope]` | 本会话输出目录 | 产出点删除（§3.1） |
| `tier` | `sandbox` | 仅 `os` / `external` 显示徽标 |

**判据 C：可证事实。** 只陈述可被参数或后端事实证明的内容；无法证明就不写，尤其不承诺可恢复。表里必须写明**来源**：参数可证，或后端 wire 上确有的事实（§0）。

| 场景 | 可证事实 | 来源 | 文案 |
| --- | --- | --- | --- |
| `delete` `permanent=false` | 后端移入系统回收站 | 参数可证（`permanent`） | 动词「删除」+ 后果「移入回收站，可恢复」 |
| `delete` `permanent=true` | 直接删除 | 参数可证（`permanent`） | 动词「永久删除」+ 后果「不经回收站，无法恢复」 |
| `write` `append=true` | 追加，已有内容保留 | 参数可证（`append`） | 动词「追加到」+ 后果「已有内容保留」 |
| `write` `append=false` 且目标不存在 | 新建文件 | 后端事实（`preview.exists=false`） | 动词「新建」+ 规模段「N 行 / X」 |
| `write` `append=false` 且目标存在 | 写整段内容替换原内容 | 后端事实（`preview.exists=true`） | 动词「覆盖」+ 后果「原内容被替换」；`exists` 为 `null` 时才退化为「新建或覆盖」（无值即省略不适用，动词必须有值） |
| `move` `overwrite=false` | 目标已存在则拒绝，不会覆盖 | 参数可证（`overwrite` 默认值，后端明写） | 动词「移动」+ 后果「不会覆盖已有目标」 |
| `move` `overwrite=true` | 目标存在时原内容被替换 | 参数可证（`overwrite`），**加条件** | 后果「目标已存在时原内容被替换」 |
| `delete` 目标是链接 | 后端明写不进入目标 | 后端事实（`preview.symlink`） | 后果「只删链接，不碰目标」 |
| `edit` / `super_edit` 行数 | 参数行数可计数 | 参数可证（`old_string` / `new_string`） | 规模段「N 行 → M 行」 |
| `shell` | 影响不可预知（TOCTOU） | 无 | 不写可逆性（类别取 §6.7 结构化输出） |

三处措辞收紧，都是「可证」口径的直接结果：

1. **不写「无自动备份」**（`write` 覆盖、`edit`）。它既非参数可证、也非后端 wire 提供，写它就是前端自造事实（D11）。
2. **`write` 的存在性由 `preview.exists` 给出，不用「写入」这种含糊动词**。`append=false` 时动词在「新建」与「覆盖」之间二选一，取值来自结构化字段；`exists` 为 `null` 才退化为「新建或覆盖」，此时折叠区 `summary` 仍给后端原话。
3. **`move` 的「可反向移动恢复」改成条件句**。`overwrite=true` 时目标原内容已消失，「可反向恢复」不再成立；只有 `overwrite=false` 才能说「不会覆盖」，而「能恢复」还需目标未被改动这一后端事实。

禁止用「安全 / 危险」替代事实陈述。

### 2.4 职责划分（选定方案）

**选定：三层分离——内容由数据决定，语义由参数字典决定，呈现由前端决定。**

| 层 | 谁决定 | 输入 | 例 |
| --- | --- | --- | --- |
| 内容：哪些事实存在 | **数据** | `arguments` 含哪些参数、后端给哪些 facts | `delete` 参数含 `permanent` → 存在「可逆性」事实；`read` 不含 → 不存在 |
| 语义：该取值意味着什么 | **参数字典**（§5.2） | `(参数名, 判别取值)` 组合 | `permanent=true` → 「永久删除」+「无法恢复」 |
| 呈现：取舍 / 顺序 / 权重 / 折叠 / 文案 / 降级 | **前端** | 上两层结果 | 要素无值则整段消失（N14）；动词加粗置左；diff 默认折叠 |

因此「前端决定展示什么」这个说法不准确，准确说法是「**前端决定如何呈现**」。前端不产生任何事实，也不产生任何语义。

三方案对照（另两个明确拒绝）：

| 方案 | 内容 | 结论 | 理由 |
| --- | --- | --- | --- |
| A 后端出模板 | 后端返回填好的行 / 面向用户的句子 | **拒绝** | 工具作者写散文时无区分度意识（现状 `shell` 10 行含 3 行常量）；散文无法本地化（S9）；每次改版要动 Rust |
| B 前端重推导 | 前端从 `arguments` 重算越界、覆盖语义、命令类别 | **拒绝** | 需要文件系统与后端策略知识，必然与后端沙箱结论不一致；等于把后端规则抄一遍 |
| **C 三层分离（选定）** | 后端出事实与结论码，参数字典出语义，前端出呈现 | **采用** | 一致性由契约保证，排版由前端统一，新增工具成本最低 |

划线判据：**得出这个结论是否需要文件系统知识或后端策略知识？**

| 需要 → 后端出事实 | 不需要 → 前端出呈现 |
| --- | --- |
| 目标是否越界、沙箱是否降级、命令类别与语句数、目标条目数/字节数 | 该字段对本工具是否恒定、要素取舍与顺序、折叠阈值、视觉权重、全部面向用户的句子 |

现状已有一个正确范例：`reasons` 是 tagged union（`{kind:"projectRule", action, resource}`），后端只出码与结构化字段，前端翻译。`extensions.label` 与 `preview.summary` 是偏离范例的遗留，应改结构化（§10.2），未改前**不得扩大其用法**。

四条禁令：

1. **后端不得返回面向用户的散文或视图模型。** 不得新增「已填好的行」字段。
2. **前端不得从 `arguments` 重算策略结论**（越界、覆盖语义、命令类别）。这类结论只能由后端事实给出。
3. **前端不得用 `toolName` 决定要素集合、布局或文案分支。** 唯一允许的用途有两处：§5.5 的恒定字段判定（只输出一个布尔值「该字段对本工具是否恒定」，属呈现策略输入，不产生内容）；**未命中任何参数字典的未知工具**回显 `toolName` 原文作为对象段（§5.4，仅回显标识、不产生语义）。
4. **新增工具只允许改两处**：后端事实枚举、前端参数语义字典（§5.2）。禁止新增「工具 → 模板」条目。

---

### 2.5 卡片存在的前提：受众过滤必须执行期强制

卡片只对**模型可见**的工具产生。`fs_link` 的 `audience()` 返回 `ToolAudience::TOOLBOX_ONLY`（[fs_link/mod.rs](../../../src-tauri/crates/assistant-tools/src/os/tools/fs_link/mod.rs)）：ToolBox 入口经 `os_tool_exec` 直调 `invoke`、**不经网关**（不调用 `permission_intents`），因此**永远不产生审批卡片**，只记 `bypassed` 审计。本文档的全部规则不覆盖它。

但「对模型不可见」目前只靠**列举过滤**实现，按名查找没有受众判断，属 fail-open：

| 查找入口 | 受众过滤 | 调用点 |
| --- | --- | --- |
| `tool_definitions_for_tier` | `audience().agent` | 模型 `tools` 列表 |
| `ToolCatalog::list` | `audience().agent` | `request_tool` 候选 |
| `find_for_toolbox` | `audience().toolbox` | ToolBox 入口（`os_tool_exec`） |
| `find_by_name` | **无** | [pipeline.rs](../../../src-tauri/crates/assistant-tools/src/runtime/pipeline.rs) 三处 + [tool_escalation.rs](../../../src-tauri/src/adapters/tool_escalation.rs) 的 `resolve`；另作注册期存在性检查 |
| `find` | **无** | 管理 / 诊断入口；`resolve` 的回落分支 |

后果链：模型输出 `tool_name: "fs_link"`（幻觉、提示注入、跨会话历史）→ Pipeline 按名解析成功 → **用 Agent 上下文执行**（`workspace_root` 已填、走网关、按 `edit` 意图求值）→ `workspace_write` 档 `edit → Allow` → **零审批**就在工作区内建出 symlink / hardlink / junction。链接建好后解析目标在界外，而 `ensure_within_root` 只校验路径字符串，后续经链接读写即绕过边界。

处置（与卡片改版同批，不改 wire）：

1. **执行期强制**：新增 `find_for_agent(name)`（过滤 `audience().agent`），替换 [pipeline.rs](../../../src-tauri/crates/assistant-tools/src/runtime/pipeline.rs) 的三处 `find_by_name`（`partition_tool_batches` 并发判定、`inline_output_limit` 落盘阈值回查、`run_call` 的 Find 阶段）与 [tool_escalation.rs](../../../src-tauri/src/adapters/tool_escalation.rs) 的 `resolve`；`find` / `find_for_toolbox` 保留为显式管理入口与 ToolBox 入口。`find_by_name` 保留，用途收窄为「注册期存在性检查」+ `find_for_agent` 的内部实现（其文档注释须写明这条边界，替换现在「模型回传的调用名走这条路径」的表述）。
2. **命名承担契约**：`register_os_tools` → `register_bridge_tools`，名字即受众声明，不靠注释约束（[os/mod.rs](../../../src-tauri/crates/assistant-tools/src/os/mod.rs)）。
3. **两条守卫测试**：`TOOLBOX_ONLY` 工具的 `find_for_agent` 必为 `None`；Pipeline 收到该名字必在 `Find` 阶段失败且无执行副作用（§11 B18b / B18c）。

**三处全换的依据（含 `inline_output_limit` 回查）：**

- **它解析的是模型给的调用名。** `ToolOutcome::new` 取 `call.tool_name`，回查用的 `outcome.tool_name` 与 `call.tool_name` 同源，因此命中「模型调用名唯一解析入口」这条规则。
- **换与不换当前等价。** `inline_output_limit()` 默认 `None`（[trait.rs](../../../src-tauri/crates/assistant-tools/src/contract/trait.rs)），只有 `agent_read` 覆写；`fs_link` 不覆写。而带 `fs_link` 名字的 outcome 只可能来自 Find 失败信封（正文是 `unknown tool '…'`），永远触及不到落盘阈值。等价即无需权衡风险，取口径更干净的一侧。
- **保留宽松路径等于保护一个「必须不存在」的状态。** 若日后真有 `TOOLBOX_ONLY` 的输出来到这里，恰恰是 §2.5 要消灭的情形；宽松解析会让它静默通过而非报错。

不采用的替代方案：让 `run_call` 把 `inline_output_limit` 直接写进 `ToolOutcome`（查「跑过的那个工具」而非「这个名字」）。语义更准，但 `run_call` 有 5 处早返回构造 `ToolOutcome`，需全量补赋值或拆内外两层，代价是重构一个长函数、收益只是一个语义标签，本次不做；留待将来需要「执行期工具快照」时再动。

**`resolve` 去掉 `find` 回落是安全的。** 生产工具里 `id == name`（OS 工具 `name()` 默认返回 `id()`，MCP 的 `id()` 返回 `name.full` 且未覆写 `name()`），name ≠ id 只出现在测试夹具，回落分支实际不可达，删除不改变行为。

卡片侧因此只需保证一件事：**`TOOL_PROFILES` 不收录 `TOOLBOX_ONLY` 工具**（§5.5）。

---

## 3. 信息处置表

每行回答一个问题，字段处置必须写明**依据哪条判据**，不允许「反正能砍就砍」。

### 3.1 通用字段处置

| 字段 | 处置 | 依据 |
| --- | --- | --- |
| `toolName` | 不进首屏文字；**不用于选模板**，仅在 §5.5 恒定字段表中作为查表键；未知工具例外回显（§5.4） | 技术标识（§2.4 禁令 2） |
| `arguments` | **证据来源**，按参数形态求值到行 1 或折叠区（§5.3） | 事实性最强（模型原文） |
| `intents` | 行 1 的对象 + 范围说明的授权键 | 同时回答 Q1 与 Q2 |
| `context.tier` | 仅 `os` / `external` 显示徽标 | 判据 B |
| `behavior.destructive` | 按判据 A 逐工具判定 | 判据 A |
| `behavior.openWorld` | 按判据 A 逐工具判定 | 判据 A |
| `behavior.readOnly` | 异常才进折叠区 | 判据 B |
| `reasons` | 主句 1 行进折叠区入口说明，其余全部折叠 | §7 |
| `preview.summary` | 仅作诊断 / 降级原文进折叠区（若存在；保留 `\n`，修复 S3）；**当结构化证据已覆盖同一内容时不重复渲染，`summary` 为空即整项消失**（N5、§5.3） | 判据 C |
| `preview.truncated` | 折叠区 `summary` 前标「预演已截断」，不进首屏；`summary` 为空时无标注可附，随 `summary` 一起消失（N2） | 判据 C |
| `preview.estimated` | 折叠区 `summary` 前标「按参数估算」，不进首屏；`summary` 为空时同上（N3） | 判据 C |
| `preview.entries` / `bytes` | 规模段的唯一来源（本次由 §10.2 补齐该结构化字段）；`arguments` 可计数者（`edit` / `super_edit` 行数）由参数直接合成，两者互不重叠 | §0 / §5.1 |
| `extensions[workspace.outside]` / `[workspace.partial]` | 仅 `Caution` 时行 1 末尾红标；文案取 **`kind.code` 复合键字典**（`workspace.outside` → 「工作区外」、`workspace.partial` → 「部分工作区外」） | 判据 B / N16 |
| `extensions[sandbox.enforcement.partial]` | 仅 `code=partial` 时进折叠区（目前只有 `shell` 产出该扩展） | 判据 B |
| `extensions[workspace.inside]` / `[sandbox.enforcement.full]` | `Info`，删除 | 判据 B |
| `extensions[sandbox.root]` / `[output.scope]` | **后端不再产出**（产出点整体删除，不是前端过滤） | 判据 B |
| 其余 `extensions` | `Caution`/`Danger` 折叠区（渲染 `kind.code` 原文），`Info` 删除 | 判据 A/B |

`extensions` 的 `label` / `value` 现状是硬编码散文（`"隔离强度"` + `"完全隔离"`、`"Workspace"` + `"inside the workspace root"`），直接渲染必然中英混排（S9）。本次把 wire 收敛为 `kind` + `code` + `severity`，**删除 `label` / `value`**：

| `kind` | `code` 取值 | 产出来源 |
| --- | --- | --- |
| `workspace` | `inside` / `outside` / `partial` | [`workspace_extensions`](../../../src-tauri/crates/assistant-tools/src/runtime/gateway/context.rs) |
| `sandbox.enforcement` | `full` / `partial` | [`sandbox_extensions`](../../../src-tauri/crates/assistant-tools/src/runtime/gateway/context.rs) |

不保留 `label` / `value` 的理由：前后端同批交付，不存在旧客户端/旧后端并存窗口，没有「过渡期原文」可渲染；保留它只会让同一事实同时存在散文与 `code` 两份，而散文必然回落到英文（判据 B 判为噪声）。`kind` 与 `code` 在 wire 上均为 `String`（开放集合）：N4 要求未知条目原样保留、不窄化，因此不做 `enum`；未知 `kind.code` 折叠区渲染原文，不本地化。

`sandbox.root` / `output.scope` 的产出点（[`writable_root_name`](../../../src-tauri/crates/assistant-tools/src/runtime/gateway/context.rs)、[`output_extensions`](../../../src-tauri/crates/assistant-tools/src/runtime/gateway/context.rs)）随本次改动删除，不做「产出但前端不显示」：产出即后端事实，恒定不显示的条目属于白算的 IO 与路径比较。

### 3.2 常量字段清单（判据 A 的落地）

| 工具 | 恒定 | 为什么是零信息 |
| --- | --- | --- |
| `delete` | `destructive=true` | 删除即破坏性，动词已表达 |
| `shell` | `destructive=true`、`openWorld=true` | 硬编码常量（[agent_shell/mod.rs](../../../src-tauri/crates/assistant-tools/src/os/tools/agent_shell/mod.rs) `behavior_hints`），连 `git status` 也报「破坏性 + 会出网」 |
| `super_edit` | `destructive=true` | 同上，硬编码常量（[super_edit/mod.rs](../../../src-tauri/crates/assistant-tools/src/os/tools/super_edit/mod.rs) `behavior_hints`）；不登记就会每张卡刷「不可逆 · 破坏性」 |
| `write` / `edit` / `move` / `mkdir` | `destructive=false`、`openWorld=false` | 默认值，判据 B 也不显示 |

`read` / `ls` / `grep` / `glob` / `super_read` 的 `readOnly=true` **不进本表**：只读工具被要求审批本身是异常，要按 N7 进折叠区诊断（现状文案「只读操作被要求确认（配置异常）」），不能按判据 A 静默删除。

`shell` 的处置与 `delete` 同构：**真正随调用变化的只有命令本身**（§6），因此 `shell` 的行 1 主语是命令，而不是常量警告。

### 3.3 处置结论

现状首屏 10 行（`shell`）中，按上表只剩 **1 行决策句 + 1 行命令块 + 1 行按钮**。删除的不是「信息」，是常量、默认值与技术标识；被重构并提到首屏的是命令原文（S12：从「范围」行移到对象段与命令块）与越界标记。

---

## 4. 卡片结构

### 4.1 行预算

| 行 | 内容 | 何时出现 |
| --- | --- | --- |
| 行 1 | **决策句**：§5.1 要素求值结果（动词 / 对象 / 规模 / 后果 / 边界） | 恒有 |
| 行 2 | **证据块**：命令原文 / diff / 内容（§5.3） | 命令长度超阈值或含控制操作符；diff 与内容默认进折叠区（D6） |
| 行 3 | **按钮行**：允许一次 / 拒绝 / 本会话允许 / 始终允许 | 恒有 |
| 行 4 | **详情入口**：`▸ 为什么问我 · 全部参数 · 诊断` | 仅当确有折叠内容 |

| 场景 | 行数 |
| --- | --- |
| 要素求值后无命令形态（多数工具） | 3 |
| 对象为命令（命令块独占一行） | 4 |
| **硬顶** | **4** |

异常标记（「工作区外」「沙箱降级」）**不占行**，附着在行 1 末尾；图标由 `ExclamationCircleOutlined` 承载，文案为纯文字（本项目 UI 不用 emoji，见 [ToolCallRenderer.tsx](../../../src/chat/tool/ToolCallRenderer.tsx)）。超过 4 行即说明有非决策信息混入。

**守恒规则：新增一行必须挤掉一行。** 不允许「这行更清楚、旧行也留着」——否则每轮改版净增，几轮后回到 10 行。

### 4.2 决策句求值规则（通用）

行 1 是 §5.1 那组决策要素的求值结果，不是预设四段填空：**要素无值即整段省略，不猜、不留空位**。

```
动词        对象              规模            后果
永久删除    docs/tmp/a.md     3.2 KB          不可恢复
```

上例只是 `permanent=true` 恰好六个要素都有值的样子（`3.2 KB` 来自 `preview.bytes`，无该字段时该段省略）；`read` 求值后只有两段（§5.4）。

| 段 | 来源 | 规则 |
| --- | --- | --- |
| 动词 | `arguments` 关键开关 | **不可截断**。`permanent=true` → 「永久删除」，不是「删除」 |
| 对象 | `arguments.path` / `source → target`；缺失时取 `intents.resources` **末段** | 前端**能**取到当前会话工作区根（`useChatStore((s) => s.workspaceRoot)`，空串 = 未绑定），但仍**一律取末段**（S13）：① 相对路径会把常态 3 行撑破，信息增量只有目录上下文；② 卡片会被截图/转发，相对路径同样带出目录结构；③ 把绝对路径相对化必须复刻后端 `canonicalize_lenient` 语义（verbatim 前缀、大小写、分隔符、`..` 与盘符相对路径拒绝、符号链接），属前端重算后端事实：前端 `workspaceRoot` 是绑定原样值（可能带 `\\?\` 前缀），`intents.resources` 是规范化后的绝对路径，两者不能直接前缀对齐。工作区内外由 `extensions[workspace]` 的 `Caution` 判定，不由路径前缀推断。`source → target` 两端各取末段；单个对象即末段本身，不再省略。若要显示工作区相对路径，须由后端结构化上 wire。
| 规模 | `arguments` 可计数者（`edit` 的 `old_string` / `new_string` 行数、`super_edit` 的行区间）+ `preview.entries` / `bytes` | 「N 行 → M 行」「N 个文件 + M 个子目录」「3.2 KB」；后端未给该字段时无值即省略 |
| 后果 | 判据 C 的「参数可证」行 + `preview` 的文件系统事实（链接、覆盖 vs 新建） | 一句话，不重复动词已表达的内容 |

**视觉落点**：动词加粗且在最左。若四段等权，路径会抢占第一落点，用户先看到路径而不是「永久删除」。

### 4.3 布局草图

卡片只有一种布局：常态 3 行（决策句 / 按钮 / 折叠），命令形态 4 行（多一块恒展开的命令块）。要素无值即省略，行数不因工具而变。

```
永久删除  docs/tmp/report.md · 3.2 KB · 不可恢复
[允许一次] [拒绝] [本会话允许] [始终允许]
▸ 为什么问我 · 全部参数
```

```
执行命令  2 条串联 · 无法判定影响 · 改动限于工作区内
┌──────────────────────────────────────────────┐
│ rm -rf build && npm run build                │
└──────────────────────────────────────────────┘
[允许一次] [拒绝] [本会话允许] [始终允许]
▸ 为什么问我 · 全部参数 · 超时设置
```

```
永久删除  docs/tmp/report.md · 3.2 KB · 不可恢复 · 工作区外
[允许一次] [拒绝] [本会话允许] [始终允许]
▸ 为什么问我 · 全部参数
```

### 4.4 尺寸核算

| 场景 | 现状首屏 | 改版首屏 | 变化 |
| --- | --- | --- | --- |
| `write` 覆盖 | ≈183px | ≈90px | −51% |
| `delete` 永久删除 | ≈225px | ≈92px | −59% |
| `shell` 单条命令 | ≈285px | ≈130px | −54% |
| `shell` 串联 8 行 | ≈285px（命令被挤在「范围」行内、无换行保留） | ≈250px（命令独占等宽块） | −12% 且命令可读性从「不可读」变为「可读」 |
| `edit` 20+20 行 diff | ≈783px（无界） | ≈90px（diff 折叠） | −89% |

两处变化的原因不同，必须分开看：

1. **常态骤降**来自「证据进折叠区」。现状 `edit` 把 40 行 diff 压进一个无 `white-space` 的 `<span>`，CSS 折叠换行后硬折成 30 多行——信息与高度双输。改版首屏只有决策句，diff 在展开后以等宽保留换行。
2. **`shell` 有意增高**来自「把命令做成独立命令块」（S12）。现状 10 行里 3 行是零信息常量，而命令原文与 `execute` 混在「范围」行、既无换行保留也无截断控制；改版用 1 行命令块替换那些常量行，净高度下降但可读性提升。

### 4.5 折叠区结构（固定顺序）

折叠区是唯一承载「详情」的位置，区块顺序固定，**每项有内容才出现**（N14），入口文案按实际存在的区块拼接（如 `▸ 为什么问我 · 全部参数`）：

| # | 区块 | 内容 | 数据来源 |
| --- | --- | --- | --- |
| 1 | 为什么问我 | §7 主句 + 其余原因逐条 | `reasons` |
| 2 | 授权范围 | `intents` 全量清单（action 人话 + 资源末段） | `intents`（§8.2） |
| 3 | 证据补充 | diff（等宽、保留换行）/ 内容首段（≤ 20 行 / 1 KB） | `arguments`（§5.3） |
| 4 | 全部参数 | `arguments` 全量原文 | `arguments` |
| 5 | 诊断 | `readOnly` 异常 /「后端预演」`summary` 原文 / 部分隔离 / 其余 `Caution`·`Danger` extensions | `behavior` / `preview` / `extensions`（§5.3、N7） |
| 6 | 拒绝后果 | 末尾一句 | 静态文案（§8.4） |

顺序理由：先给「为什么问我」与「我授权什么」（决策信息），再给证据与原文（核对材料），最后给诊断与后果（兜底说明）。§5.6 各工具示例只列出该工具实际出现的区块，顺序与本表一致。

---

## 5. 要素求值与参数语义

行 1 不是「每个工具一套模板」，而是**一组决策要素求值**：有值就显示，无值整段消失。工具之间的差异由**参数形态与判别取值**涌现（§2.4 三层分离），不由工具名定义——这是本设计与「后端返回固定内容、前端照抄」的分界（§10.3 D9）。

### 5.1 决策要素（全局一套）

| 要素 | 有值条件 | 数据来源 |
| --- | --- | --- |
| 动词 | 命中参数语义字典的动词项；参数里没有判别项时取 `preview.verbCode`，再回退 `intents[0].action` 的 action 字典（§8.2） | 参数字典（§5.2）/ `preview.verbCode` / `intents` |
| 对象 | `arguments` 含路径 / 命令 / `source → target`，否则取 `intents.resources` 末段 | `arguments` 原文 / `intents` |
| 规模 | `arguments` 可计数（`edit` 的 `old_string`/`new_string` 行数、`super_edit` 的行区间与 `replacement` 行数）或 `preview.entries` / `bytes` | `arguments` / `preview` |
| 后果 | 参数能证明可逆性或覆盖语义，或 `preview` 给出文件系统事实（链接、覆盖 vs 新建） | 参数字典（§5.2）/ `preview` |
| 边界告警 | 路径型工具：`extensions[workspace]` 为 `Caution`；`shell`：`preview.targetsScope` 为 `inside`（「改动限于工作区内」）或 `outside`（「可写入工作区外」） | `extensions` / `preview`（§6.5） |
| 证据 | `arguments` 含命令 / diff / 内容 | `arguments` 原文 |

求值顺序固定（动词 → 对象 → 规模 → 后果 → 边界），**但空要素不占位、不留分隔符**（N14）。顺序固定只为视觉一致性：用户学一次「动词在最左、危险标记在末尾」即可。

### 5.2 参数语义字典（与工具无关）

字典的 key 是**参数名 + 判别取值**，不是工具名。新增工具若复用 `path` / `content` / `command` / `permanent`，自动获得翻译；只有**新参数名**才需加一条。

**参数名不是天然全局一致的**，所以 key 必须允许判别参数参与：

| 参数组合 | 语义 | 只按参数名会误译成 |
| --- | --- | --- |
| `source` + `target`（无判别参数） | 移动 `source → target` | — |
| `source` + `target` + 判别参数（未来工具） | 由判别取值决定（如「创建」「检查」），不得落到「移动」 | 「移动」 |

判别参数本身来自 `arguments`，因此「不按 `toolName` 分支」仍然成立（§2.4 禁令 3）。判别参数只有 `permanent` / `recursive` / `append` / `overwrite`；`source` + `target` 当前只出现在 `move`，故无歧义。**新增工具复用 `source` / `target` 时必须同时给出判别参数，否则该组合不产出动词**——宁缺勿错，不猜（N14）。

| 参数 | 取值 | 贡献 | 数据来源 |
| --- | --- | --- | --- |
| `permanent` | `true` | 动词「永久删除」+ 后果「不经回收站，无法恢复」 | 参数 |
| `permanent` | `false` | 动词「删除」+ 后果「移入回收站，可恢复」 | 参数 |
| `recursive` | `true` | 动词改「删除目录」；与 `permanent=true` 组合为「永久删除目录」（组合查表，不拼接，§5.6.8 G3）；条目计数取 `preview.entries` | 参数 + `preview` |
| `append` | `true` | 动词「追加到」+ 后果「已有内容保留」 | 参数 |
| `append` | `false` + `exists=false` | 动词「新建」+ 规模段（行数 / 字节） | 参数 + `preview.exists` |
| `append` | `false` + `exists=true` | 动词「覆盖」+ 后果「原内容被替换」 | 参数 + `preview.exists` |
| `append` | `false` + `exists=null` | 动词「新建或覆盖」（预演降级时的唯一退化形态，不断言目标已存在，也不含糊成「写入」） | 参数 |
| `overwrite` | `false`（默认） | 后果「不会覆盖已有目标」 | 参数 |
| `overwrite` | `true` | 后果「目标已存在时原内容被替换」（条件句，不声称目标一定存在） | 参数 |
| 目标为符号链接 | — | 后果「只删链接，不碰目标」 | `preview.symlink` |
| 命令含控制操作符 | — | 不写可逆性；类别取 §6.7 结构化输出 | `preview` 结构化 |

`overwrite` 的默认语义来自后端明写（[agent_move.rs](../../../src-tauri/crates/assistant-tools/src/os/tools/agent_move.rs)「rejected unless `overwrite = true`」），前端不得自行假设「覆盖目标」。但**卡片只陈述后果**（「不会覆盖已有目标」），不陈述有效性（「会失败」）——后者是 N13 的禁区。

**只写字典能证明的。** 字典里没有的语义 → 该要素无值，不猜。

**动词的第二来源：`preview.verbCode` 与 `intents[0].action`。** `read` / `super_read` / `ls` / `glob` / `grep` / `edit` / `super_edit` 的参数里没有判别项（`path`、`pattern`、`old_string` 都不区分动作），此时用 action 字典补动词（§8.2：`read`→「读取」、`edit`→「修改」、`execute`→「执行命令」、`delete`→「删除」、`purge`→「永久删除」、`mcp`→「调用外部工具」）。action 是后端 `permission_intents` 产出的事实，不是前端按 `toolName` 分支，禁令 3 仍然成立。`mkdir` 与 `write` / `edit` 共用 action `edit`、参数又只有 `path` + `recursive`，action 回退只会得到「修改」，因此其动词由后端在 `preview.verbCode` 里直接给出（§10.2），前端按 code 取词。

特别注意：**工具能力事实不进字典。**「无自动备份」这类既非参数可证、也非后端 wire 提供的内容一律不写——写它就是前端自造事实。若确需展示，应由后端以 `extensions` 事实给出。

### 5.3 证据形态由参数形态决定

不按 `toolName` 选证据块，而按参数形态：

| 形态特征 | 证据块 | 位置 |
| --- | --- | --- |
| 含 `command`，且长度未超阈值、无控制操作符 | 无（命令即对象） | 行 1 对象段（可中间省略） |
| 含 `command`，长度超阈值**或**含控制操作符 | 命令全文 | 行 2，恒展开（R1）。阈值按显示宽度计（约 120 字符），不按是否含 `&&` 判定——单条长命令同样会读不到尾部 |
| 含 `old_string` + `new_string` | diff（等宽、保留换行） | 折叠区（D6） |
| 含 `content` | 内容首段：**最多 20 行 / 1 KB**，超出截断并标「已截断」 | 折叠区 |
| 含 `source` + `target` | 无（已在对象段） | — |
| 仅 `path` | 无 | — |

`edit` 类 diff **不进首屏**：它是逐行核对材料，不是决策结论；行 1 的 `N 行 → M 行` 已给量级（修复 S3：保留换行但默认折叠）。

**diff 的唯一来源是 `arguments`。** 后端 `build_preview` 已经把 diff 拼进 `preview.summary`（`"编辑 <绝对路径>：N 行 → M 行（按参数估算）\n<diff>"`，见 [report.rs](../../../src-tauri/crates/assistant-tools/src/os/tools/agent_edit/report.rs)）。若同时渲染 `summary` 与参数合成 diff，折叠区会出现两份 diff，且 `summary` 里那份还带绝对路径。处置：

**`preview.summary` 与结构化证据的共存规则**（后端 `summary` 的形态各不相同，不能一刀切）：

| 工具 | `summary` 内容 | 与结构化证据的关系 | 处置 |
| --- | --- | --- | --- |
| `edit` / `super_edit` | `"编辑 <绝对路径>：N 行 → M 行（按参数估算）\n<diff>"`（[report.rs](../../../src-tauri/crates/assistant-tools/src/os/tools/agent_edit/report.rs)） | diff **重复**，且带绝对路径 | `summary` 不渲染；仅「诊断」子项留原文，标「后端预演」 |
| `write` | 目标存在：`"追加到 1 个现有文件（当前 N 行 / X）"` 或 `"覆盖 1 个现有文件（当前 N 行 / X）"`；目标不存在：`"新建文件（N 行 / X）"`（[preview.rs](../../../src-tauri/crates/assistant-tools/src/os/tools/agent_write/preview.rs)） | 与 `preview.exists` / `entries` / `bytes` 同内容 | 同内容不重复渲染；仅 `exists=null` 的降级态进折叠区并标「后端预演」 |
| `delete` | 路径 + 条目数 + 字节数 | 与 `preview.entries` / `bytes` 同内容 | 同内容不重复渲染；`entries=null` 时进折叠区标「后端预演」 |
| `read` / `super_read` | 工作区内返回 `null`；跨出到工具输出目录时给「读 <文件> · 第 N–M 行 · X」（[agent_read/mod.rs](../../../src-tauri/crates/assistant-tools/src/os/tools/agent_read/mod.rs)、[super_read/mod.rs](../../../src-tauri/crates/assistant-tools/src/os/tools/super_read/mod.rs)） | 与结构化证据无重复 | 仅异常审批时进折叠区，标「后端预演」 |
| `move` / `mkdir` / `ls` / `glob` / `grep` / MCP 桥接工具 | `preview_for_approval` 未实现，`preview` 为 `null` | 无结构化证据 | 折叠区只有原因 / 参数 / 授权范围，**无「后端预演」子项** |
| `shell` | 现为 `"执行 1 条命令 · <类别> · 隔离：<强度>"`；本次改为**空串**（语句数 / 类别 / 隔离强度全部由结构化字段给出，见下行） | 三部分均与结构化证据重复（隔离强度另由 `sandbox.enforcement` 扩展给出） | `summary` 为空，**不渲染**；折叠区改渲染结构化摘要（§6.5、§6.7） |

**`summary` 的唯一职责是「诊断 / 降级原文」。** 首屏永不渲染；结构化证据齐备时后端直接置空串（`shell` 即此例，`summary_for` 一并删除），`summary` 为空时不渲染任何占位、不渲染 `truncated` / `estimated` 标注（标注只是 `summary` 的前缀限定语，没有正文就没有可限定对象）。`truncated` / `estimated` 仍保留在 wire 上（它们是后端事实），只是没有渲染位。

这样 `summary` 与 N5 不再互相牵制：N5 约束的是「`summary` 若被渲染则按 `\n` 分行、保留换行、不改写」，与「多数工具不渲染」并不冲突；需要原文时（`edit` / `super_edit` 的诊断子项、`write` / `delete` 的 `null` 降级态、`read` 的异常审批）它仍是唯一的诊断来源。

内容截断是呈现策略，不产生新事实：只裁行数并显式标注，不改写文本（N10）。

### 5.4 求值结果示例（差异来自数据）

```
delete  permanent=true    永久删除  report.md · 3.2 KB · 不可恢复
delete  permanent=false   删除  report.md · 3.2 KB · 可恢复
delete  recursive=true    删除目录  docs/tmp · 12 个文件 + 3 个子目录 · 4.2 MB · 可恢复
write   append=true       追加到  a.log · 新 20 行 · 已有内容保留
write   append=false 且目标存在   覆盖  src/a.ts · 新 40 行 · 原内容被替换
move    overwrite=false   移动  a.ts → b.ts · 不会覆盖已有目标
read    只读工具被审批    读取  build.log（无后果段；异常走折叠区诊断）
shell   任意命令          执行命令  <命令原文> · 2 条串联 · 无法判定影响 · 改动限于工作区内
mkdir   verbCode=mkdir      新建目录  docs/new-dir（无后果段；动词取自 `preview.verbCode`，§10.2）
mcp     mcp__* 动态名      调用外部工具  <toolName> <参数摘要>（动词取自 action 字典 `mcp`，无后果段）
未知工具                  调用 <toolName> <参数摘要>（无 action 可查，动词退化为通用「调用」）
```

`read` 只有两段、未知工具只有两段——这不是「模板缺格」，是**要素无值**。`recursive=false` 且目标为目录时的「会失败」判定不进卡片，改为 `validate_input` 拦截（§10.2）；同理 `move overwrite=false` 只陈述「不会覆盖」，不陈述「会失败」。

`read` 的 `output.scope` 扩展按判据 B 删除（默认态零信息），所以「本会话输出目录」不会出现在行 1；只读工具需要审批本身就是异常，走 N7 折叠区诊断。

未知工具是 `toolName` 上首屏的**唯一例外**：它没有任何参数字典命中，也没有可查的 action，动词段退化为通用「调用」，对象段为 `toolName` 原文 + `summarizeArguments` 摘要——只是回显标识，不产生语义（§3.1 / 禁令 3 显式豁免）。MCP 工具同属「不在 `TOOL_PROFILES`」，但它的 `intents[0].action` 是 `mcp`，因此动词由 action 字典给出（「调用外部工具」），对象段仍按回显规则（`toolName` + 参数摘要），不取 `mcp:<server>:<hash>` 的末段。

### 5.5 恒定字段声明（判据 A 的输入）

`TOOL_PROFILES` 只声明「该工具哪些 `behavior` 字段恒定」（供判据 A 过滤），**不声明显示哪些行**：

| 工具（wire `toolName`） | 恒定字段 |
| --- | --- |
| `delete` | `destructive=true` |
| `shell` | `destructive=true`、`openWorld=true` |
| `super_edit` | `destructive=true` |
| `read` / `ls` / `grep` / `glob` / `super_read` / `write` / `edit` / `move` / `mkdir` / `ask_user` / `request_tool` | 无（默认值由判据 B 处理；`readOnly=true` 属异常诊断，见 §3.2） |

**本表不收录 `TOOLBOX_ONLY` 工具**（当前 `fs_link`）：它不产生审批卡片（§2.5），收录只会让「恒定字段」清单与实际渲染集合漂移。

**id 必须用 wire 上的 `toolName`，不是 Rust 路径名。** 工具 id 常量是 `"delete"` / `"shell"` / `"super_edit"` 等（各工具 `TOOL_ID`，[agent_delete/mod.rs](../../../src-tauri/crates/assistant-tools/src/os/tools/agent_delete/mod.rs)、[agent_shell/mod.rs](../../../src-tauri/crates/assistant-tools/src/os/tools/agent_shell/mod.rs)），`Tool::name()` 默认返回 `id()`（[contract/trait.rs](../../../src-tauri/crates/assistant-tools/src/contract/trait.rs)），因此 wire `toolName` 就是这些常量。写成 `agent_delete` / `agent_shell` 会让查表全部落空、常量过滤失效。

护栏：`TOOL_PROFILES` 的键集合必须等于「后端注册工具 id 扣除 `TOOLBOX_ONLY` 集合」（当前 14 个）；条目内容只是「恒定字段」，显示逻辑仍由 §5.1 求值决定。测试口径为**源码扫描**：用 `node:fs` 递归读取 `src-tauri/crates/assistant-tools/src/os/tools/` 下全部 `*.rs`（不新增 glob 依赖，**不维护文件清单**——清单本身会漂移，反而削弱护栏），提取全部 `const TOOL_ID: &str = "…"`，扣除前端 `TOOLBOX_ONLY_IDS` 后与 `TOOL_PROFILES` 逐一比对；`TOOLBOX_ONLY_IDS` 与 Rust `audience()` 实际结果的一致性由 Rust 侧测试断言（§11 B18b）。

护栏只断言**键集合**，不断言条目内容：条目字段（如 `destructive=true`）是常量抄录，比对它属于变更检测，其行为侧已由 A13 覆盖。

### 5.6 真实工具卡片形态（与代码侧 1:1）

本节示例由卡片取到的真实数据构成：`arguments` 是模型原文；`intents` 由各工具 `permission_intents` 产出；`preview` 由 `preview_for_approval` 产出（未实现的工具为 `null`，此时规模段与文件系统事实无值即省略）；`extensions` 由 [gateway/context.rs](../../../src-tauri/crates/assistant-tools/src/runtime/gateway/context.rs) 装配。

**哪些工具会弹卡**（`audience().agent = true` 且 `permission_intents` 非空）：

| wire `toolName` | `intents`（action） | 弹卡条件 |
| --- | --- | --- |
| `delete` | `delete`；`permanent=true` 时追加 `purge`（同一资源） | 除「直接运行」外的全部级别 |
| `shell` | `execute`（资源 = 命令原文） | 同上 |
| `write` / `edit` / `super_edit` / `move` / `mkdir` | `edit` | 「工作区写入」下仅 `.env*` / `.git/**` 与越界路径；「谨慎」「手动审批」下全部 |
| `read` / `super_read` / `ls` / `glob` / `grep` | `read`；跨出工作区到本会话工具输出目录时改 `readToolOutput` | 仅「手动审批」，或 `readToolOutput`（恒 `Ask`，任何级别都不放宽） |
| `mcp__<server>__<tool>` | `mcp`（资源 = `mcp:<server>:<hash>`） | 除「直接运行」外的全部级别 |
| `ask_user` / `request_tool` | 空（trait 默认实现） | **永不**：空意图直接 `Bypassed` |
| `fs_link` | —（`TOOLBOX_ONLY`） | **永不**（§2.5） |

下列示例统一省略按钮行（四按钮恒在：`[允许一次] [拒绝] [本会话允许] [始终允许]`）与折叠入口（`▸ 为什么问我 · 全部参数`）。

#### 5.6.1 删除

**① 删除单个文件（`permanent=false`）** — `arguments` `{path:"docs/tmp/report.md"}`、`intents` `[delete]`、`extensions` `workspace` 的 `code=inside`（`Info`，判据 B 不显示）

```
删除  report.md · 3.2 KB · 可恢复
```

折叠区：原因「默认策略：`* *`」→ 授权范围「删除 · report.md」→ 全部参数 →「后端预演」原文（保留 `\n`；与结构化字段同内容时不重复渲染，§5.3）。

**② 永久删除单个文件（`permanent=true`）** — `intents` `[delete, purge]`

```
永久删除  report.md · 3.2 KB · 不可恢复
```

折叠区：授权范围要列出**两个** action（`删除` + `永久删除`）——「始终允许」等于以后所有永久删除不再询问（§8.2 约束 1）。

**③ 永久删除目录（`recursive=true` + `permanent=true`）** — 规模段取 `preview.entries` / `bytes`

```
永久删除目录  tmp · 12 个文件 + 3 个子目录 · 4.2 MB · 不可恢复
```

规模段由结构化字段填充；`entries` 为 `null` 时整段消失，其余要素照常（N14）。

**④ 越界删除**

```
永久删除  report.md · 3.2 KB · 不可恢复 · 工作区外
```

「工作区外」来自 `extensions[workspace]` 的 `Caution`（判据 B），附着行 1 末尾、不额外占行。

#### 5.6.2 命令

**① 短命令（无控制操作符、显示宽度 ≤ 约 120）** — 命令即对象段，类别与语句数取 `preview` 结构

```
执行命令  git status · 只读
```

单条命令不显示语句数（`1 条` 是默认值、恒零信息，判据 A）；`&&` / `;` / `|` 使语句数 ≥ 2 时才出现规模段。

**② 含控制操作符或超宽** — 命令全文进证据块（恒展开），行 1 给语句数与重定向目标

```
执行命令  2 条串联 · 写入 build.log
┌──────────────────────────────────┐
│ rm -rf build && npm run build > build.log │
└──────────────────────────────────┘
```

**③ 部分隔离（`sandbox.enforcement` 的 `code=partial`）** — 首屏不变，折叠区多一条「部分隔离」（`Caution`，判据 B）；`code=full` 时不出现。

`shell` 的边界段不来自 `extensions[workspace]`（`intents` 的资源是命令原文，非路径，§5.6.8 G1），而来自 §6.5 的目标求交结果 `preview.targetsScope`；该字段为 `unknown` / `null` 时边界段整段省略（N14）。

#### 5.6.3 写入

**① 追加（`append=true`）**

```
追加到  app.log · 已有内容保留
```

折叠区：内容首段（≤ 20 行 / 1 KB）→ 诊断位「后端预演」原文（与结构化字段同内容时不重复渲染，§5.3）。

**② 整段写入（`append=false`）** — 存在性取自 `preview.exists`，两张卡首屏不同

目标不存在（`exists=false`）：

```
新建  config.ts · 120 行 / 3.4 KB
```

目标存在（`exists=true`）：

```
覆盖  config.ts · 120 行 / 3.4 KB · 原内容被替换
```

折叠区：内容首段（≤ 20 行 / 1 KB）→「后端预演」原话（`summary` 与结构化字段同内容时不重复，§5.3）。

`exists` 为 `null`（预演降级）时动词退化为「新建或覆盖」，并在 `diagnostics` 标「后端预演不可用」——这是唯一的降级形态，不是默认形态（A4）。

#### 5.6.4 编辑

**① `edit`**

```
修改  src/a.ts · 3 行 → 5 行
```

折叠区：diff（等宽、保留换行）→「后端预演」原文（`编辑 <绝对路径>：3 行 → 5 行（按参数估算）\n<diff>`，与结构化 diff 重复，只留诊断位）。

**② `super_edit`（行区间）**

```
修改  src/a.ts · 12–20 行 → 2 行
```

规模段由 `endLine - startLine + 1` 与 `replacement` 行数求出，都是参数可证。

#### 5.6.5 移动 / 目录

**① `move`（`overwrite=false`，默认）** — `intents` 两条 `edit`（源、目标），按 action 合并为一个 action 两个资源

```
移动  a.ts → b.ts · 不会覆盖已有目标
```

`move` 没有 `preview_for_approval` → 折叠区无「后端预演」，只有原因 / 参数 / 授权范围。

**② `move`（`overwrite=true`）**

```
移动  a.ts → b.ts · 目标已存在时原内容被替换
```

**③ `mkdir`** — 动词取自 `preview.verbCode`（§10.2）

```
新建目录  src/components
```

#### 5.6.6 只读工具（`read` / `super_read` / `ls` / `glob` / `grep`）

工作区内读取默认 `Allow`，**不弹卡**。只有两种情形会弹：

**① 「手动审批」级别下的工作区内读取**（`read` 的 `preview` 对工作区返回 `null`）

```
读取  build.log
```

折叠区：诊断「只读操作被要求确认（配置异常）」+ 原因「审批级别：手动审批」。

**② 跨出工作区读工具输出（`readToolOutput`，恒 `Ask`）**

```
读取工具输出  output.txt
```

折叠区：「后端预演：读本会话输出 output.txt · 第 1–2000 行 · 3.2 KB」；`output.scope` 扩展按判据 B 不显示。

**③ `glob` 省略 `path`** — 对象段取 `intents.resources` 末段（工作区根末段）

```
读取  myapp
```

#### 5.6.7 MCP 外部工具

`toolName` 是 `mcp__<server>__<tool>`，不在 `TOOL_PROFILES`（该表只收录固定 `TOOL_ID`），对象段走**回显**规则；动词取 `intents[0].action`（`mcp`）的字典文案：

```
调用外部工具  mcp__github__create_issue  {"repo":"o/r","title":"Bug: …"}
```

无后果段、无规模段；折叠区保留 `mcp` action 原文与 `mcp:<server>:<hash>` 资源。桥接层未实现 `preview_for_approval`，故无「后端预演」。`workspace` 扩展对 `mcp` 资源不产出（§5.6.8 G1），所以不会出现「工作区外」误标。

#### 5.6.8 已知缺口与处置

| # | 缺口 | 当前代码行为 | 处置 |
| --- | --- | --- | --- |
| G1 | `workspace` 扩展对非路径资源误判 | [context.rs](../../../src-tauri/crates/assistant-tools/src/runtime/gateway/context.rs) 对每条 `pending.resource` 直接做工作区成员判定；`shell` 的资源是命令原文、MCP 的是 `mcp:<server>:<hash>`，路径解析必然失败 → 恒判「工作区外」 | 只对路径型 action（`read` / `readToolOutput` / `edit` / `delete` / `purge`）判定，`shell` / MCP 不产出 `workspace` 条目（§10.2） |
| G2 | 无动词工具 | `mkdir` 参数只有 `path` + `recursive`，`intents` 的 action 与 `write` / `edit` 同为 `edit`；参数无法区分「新建目录」与「修改文件」，按 `toolName` 分支被禁令 3 禁止 | 由 `preview.verbCode` 给出动词（§10.2），前端按 code 取词，参数形态不参与 |
| G3 | 动词组合 | `permanent` 与 `recursive` 各贡献一个动词，同时为 `true` 时按字符串拼接会得到「永久删除删除目录」 | 动词 key 取**判别参数组合**查表（四种组合四条），不拼接（§5.2） |
| G4 | `move` 无 `preview` | `move` 未实现 `preview_for_approval`，无条目数 / 字节数可给 | 规模段与后果由参数与 `intents` 求值，不依赖 `preview`；`move` / 只读类 / MCP 的 `preview` 为 `null`（§5.3） |

---

## 6. `shell` 命令语义理解

> **本章的产出依赖后端结构化字段。** 语句数、串联数、类别、重定向目标、内嵌载荷由 §6.7 的解析器产出，并在本次改动中随 §10.2 一起上 wire；卡片只读结构化字段，不解析散文。本章是 §10.2 结构化改造的设计输入。

### 6.1 现状能力边界

[classify.rs](../../../src-tauri/crates/assistant-tools/src/os/tools/agent_shell/classify.rs) 的判定路径是：遇到任何控制操作符 → `Unknown`；否则用 `argv[0]` 查四张清单（只读 / 联网 / 破坏性 / 未知）。方向安全（永远偏向更危险），但**没有解析语义**，在四类场景必然失效：

| 失效类型 | 例子 | 现状 | 真实语义 |
| --- | --- | --- | --- |
| 壳程序，载荷才是真命令 | `bash -c "rm -rf /"` | `Unknown` | 删除根目录 |
| 参数改变语义 | `sed -i 's/a/b/' f` | `Unknown` | 写入 `f` |
| 重定向即写入 | `echo hi > out.txt` | `Unknown` | 写入 `out.txt` |
| 包装前缀 | `timeout 30 rm -rf build` | `Unknown`（`argv[0]=timeout`） | 删除 `build` |

`Unknown` 在这里不只是保守，它**等于把卡片废掉**：用户看到「无法判定」只能凭运气点允许。

### 6.2 四层模型

```
① 结构层：切成语句 + 管道 + 重定向 + 内嵌载荷
② 语句层：每条简单命令 → 效果集（读/写/删/执行/联网/提权）
③ 目标层：从参数抽路径 → 与工作区求交 → 内/外/未知
④ 置信层：Certain / Inferred / Unknown → 决定措辞强度
```

**①必须先做**。真实执行是 `cmd /d /C <整段>` / `sh -c <整段>`（[spawn.rs](../../../src-tauri/crates/assistant-tools/src/os/tools/agent_shell/spawn.rs)），字符串就是 shell 语法；不切分就没有语义。

### 6.3 结构层产出

| 结构 | 语义 | 对卡片的意义 |
| --- | --- | --- |
| `;` `&&` `\|\|` `&` 换行 | 顺序 / 条件 / 后台 | 语句数 → 「N 条串联」 |
| `\|` | 两侧都执行 | 效果并集；管道右侧常是真动作 |
| `>` `>>` `<` `2>` | 重定向即写入/读取目标 | **可直接说出「写入 X」** |
| `$( )` 反引号 | 子命令先执行 | 该语句 `Unknown` |
| `$VAR` `%VAR%` | 变量展开 | 目标层 `Unknown` |
| `bash -c` / `sh -c` / `cmd /c` / `powershell -Command` / `python -c` / `node -e` / `xargs` | 载荷才是真命令 | **递归解析载荷，并把载荷作为审批主体展示** |

`bash -c` 这条最重要：卡片应显示「内嵌命令: `rm -rf /`」，因为用户批准的是载荷而不是 `bash`。

### 6.4 语句层：效果集而非单一类别

现状 `CommandClass` 是四选一，但真实命令常是多效果：`npm install` 既联网又写 `node_modules`；`curl -o f` 联网 + 写。数据模型应为效果位集合：

```rust
pub struct CommandEffect {
    pub statements: Vec<StatementEffect>,
    pub verbs: EffectVerbs,        // READ|WRITE|DELETE|EXEC|NETWORK|PRIVILEGE
    pub confidence: Confidence,    // Certain / Inferred / Unknown
}

pub struct StatementEffect {
    pub program: String,           // 归一：basename、去 .exe、小写
    pub verbs: EffectVerbs,
    pub targets: Vec<TargetPath>,  // 每个带 inside / outside / unknown
    pub nested: Option<String>,    // 内嵌载荷原文
}
```

需要的静态表只有五张：

| 表 | 内容 | 例 |
| --- | --- | --- |
| 程序效果档 | `argv[0] → 默认效果集` | `rm`→DELETE，`cp`→WRITE，`curl`→NETWORK |
| 子命令效果档 | `(program, sub) → 效果集` | `git status`→READ，`git clean`→DELETE，`npm run`→EXEC |
| 参数效果增量 | `flag → 增量` | `-i`→WRITE，`-delete`→DELETE，`-o`→WRITE，`--hard`→WRITE |
| 包装前缀 | 跳过并继续解析 | `env` `sudo` `timeout` `nohup` `time` `nice` `command` |
| 载荷旗标 | 后续参数是整段命令 | `-c` `-e` `--command` `/c` `-Command` |

### 6.5 目标层与置信层

目标抽取：跳过 `-` 开头 token 与程序名；命中以下任一即视为路径——含 `/` 或 `\`、是 `.`/`..`、有扩展名、是重定向目标。相对 `cwd` 归一后与可写根求交，得 `inside` / `outside` / `unknown`。路径中出现 `$VAR` `%VAR%` `*` `?` → `unknown`，**不得当成工作区内**。

| 置信 | 来源 | 卡片措辞 |
| --- | --- | --- |
| `Certain` | 命中程序/子命令表且无动态成分 | 「删除」「写入」 |
| `Inferred` | 由 flag / 载荷 / 重定向推出 | 「按参数推断为写入」 |
| `Unknown` | 变量展开、未知程序、解析失败 | 「无法判定」**但保留结构事实** |

行 1 后果段的合成（边界段只在**效果含 WRITE / DELETE** 且 `targetsScope` 有值时出现，其余情形整段省略）：

| 类别 | 后果句 | 边界段 |
| --- | --- | --- |
| `ReadOnly` | 只读类命令 | —（无可写目标，边界无意义） |
| `Network` | 联网命令 | `inside` → `· 改动限于工作区内`；`outside` → `· 可写入工作区外` |
| `Destructive` | 破坏性命令 | 同上 |
| `Unknown` | 无法判定影响 | 同上（`unknown` / `null` 时省略） |

注意与 `delete` 的不对称：`delete` 的目标是已解析路径，影响面等于该路径，**不需要**边界；`shell` 影响面未知，边界是唯一能约束最坏情况的信息，因此「改动限于工作区内」在 `delete` 是噪声、在 `shell` 是决策信息。

### 6.6 三条不可让步的规则

1. **`Unknown` 不等于安全。** 解析失败只降置信、不降风险等级，仍要显示「N 条串联」「写入 X」「目标含变量」等**与程序语义无关的结构事实**——结构事实永远可证，程序语义才需推断。
2. **必须区分 shell 方言。** Windows 走 `cmd.exe /C`（转义 `^`、变量 `%X%`），Unix 走 `sh -c`（转义 `\`、变量 `$X`）。`ShellFlavor` 已存在，分类器必须接受它，否则会把 `%TEMP%\x` 当普通路径。
3. **只用于展示与审批输入，绝不用于放行。** 真正决定执行边界的是沙箱；分层必须保持，否则一次分词失误就变成一次越权。

### 6.7 实现顺序

四步是**同一次改造内部的实现顺序**，不是排期切割；每一步都必须随本次改动交付，不允许停在 S1。

| 步骤 | 内容 | 规模 | 收益 |
| --- | --- | --- | --- |
| S1 | 词法 + 结构切分（引号/转义/操作符/重定向），`Unknown` 降级为「结构已知、语义未知」 | ~250 行 + 测试 | 消除 `git log > f` 误判，能说出「N 条串联」「写入 X」 |
| S2 | 程序/子命令/参数三张效果表 | ~200 行静态表 | 大量 `Unknown` 变为可读动词 |
| S3 | 包装前缀 + 载荷递归（深度 1） | ~100 行 | `bash -c` 显示真命令 |
| S4 | 目标抽取 + 工作区求交 | ~120 行 | 精确「工作区外」，不再依赖英文散文 |

四步全部属于本次实现；`CommandClass` 四值输出保持不变（`as_str` 仍是事实输入，新结构并行提供）。

**结构化输出必须上 wire**：语句数 / 重定向目标 / 内嵌载荷目前只在 `preview.summary` 的散文里（`summary_for` 固定输出「执行 1 条命令 · <类别> · 隔离：<强度>」）。本次新增结构化字段并**删除 `summary_for`**（`summary` 置空串），卡片只读结构化字段（§5.3），改 wire + `contract:gen` 与 §10.2 同批交付。

---

## 7. 原因的人话翻译

### 7.1 六种原因 → 一句主句

沿用 [ApprovalCard.tsx](../../../src/chat/tool/ApprovalCard.tsx) 现有 `reasonText` 分支（六种全覆盖、无 `default` 兜底，已由 `approvalRulesUi.test.ts` 锁定），只调整**选取规则**与**呈现位置**：

职责切分：**选取**（§7.2 优先级排序，产出 `primary` 与 `rest` 两个数据槽）归视图模型 `buildApprovalView`；**文案**（`reasonText` 六分支）仍留在卡片，视图模型不产出任何本地化文本。因此 A11 继续由 `approvalRulesUi.test.ts` 的现有断言覆盖，A12（主句优先级）写在 `approvalPresentation.test.ts`。

| 原因 | 主句模板 |
| --- | --- |
| `approvalLevel` | 当前档位（<档位人话名>）下，<动作> 需要你确认 |
| `projectRule` | 项目规则 `<规则>` 要求 <动作> 前确认 |
| `userRule` | 你为本项目设置的自定义规则要求确认 |
| `tierCeiling` | 本会话只允许沙箱级工具，此操作超出上限 |
| `defaultPolicy` | 该操作默认需要确认 |
| `advisor` | 安全顾问建议确认 |

### 7.2 多条原因的合并

- 主句优先级：`userRule` > `projectRule` > `tierCeiling` > `approvalLevel` > `advisor` > `defaultPolicy`（越「专属于本用户/本项目」越靠前）。
- 其余原因不丢弃，全部进折叠区，逐条一行。
- 同一 `rule` 串在多条原因中重复出现时只显示一次。

### 7.3 技术细节下沉

`defaultPolicy` 的原始规则串、`tier` 的枚举名、`intents` 的 action 名只在折叠区出现；首屏一律用人话。

---

## 8. 授权动作与范围

### 8.1 按钮

顺序与文案保持四按钮（I38d 硬约束：不新增决策动作）：

`允许一次`（主色实心）→ `拒绝` → `本会话允许` → `始终允许`。

### 8.2 范围说明（Q2 的落点）

删除固定一行 `always_hint`（[ApprovalCard.tsx](../../../src/chat/tool/ApprovalCard.tsx) `alwaysHint` 块，与具体授权键无关的固定提示），改为**按当前可点范围生成、按需展示**：

| 按钮 | 说明内容 |
| --- | --- |
| `本会话允许` | 同「<动作人话> <对象末段>」不再询问，会话结束失效 · 可在此撤销 |
| `始终允许` | 所有会话都不再询问，将放行 <`intents` 全量清单> · 可在此撤销 |

`intents` 全量清单的渲染规则（`PermissionIntentDto` 只有 `action` + `resources`，没有 `displayedResource`，[agent.rs](../../../src-tauri/crates/assistant-protocol/src/agent.rs)）：

| 部分 | 首屏 | 折叠区 |
| --- | --- | --- |
| `action` | 取 **action 字典**（`execute`→执行命令、`edit`→修改、`delete`→删除、`purge`→永久删除、`read`→读取、`mcp`→调用外部工具；未知 action 原样回显并标「未识别」） | 原文 |
| `resources` | 路径项取**末段**（`a.md`、`b.ts`）；`action="execute"` 的项是命令原文，**不按路径切段**，原样渲染（超长时中间省略，保留首尾）；`action="mcp"` 的项是 `mcp:<server>:<hash>`，首屏只显示 `<server>` 段（hash 是内部标识，无决策价值） | 绝对路径 / 命令原文 / 完整 `mcp:` 串 |

三条约束：

1. **必须列出放行键**。`delete` + `permanent=true` 时 `intents` 含 `delete` 与 `purge` 两个 action——「始终允许」等于以后所有永久删除都不再询问。现状把它们塞在 `resources` 行（`delete path, purge path`），用户读不出这层含义。
2. **对象取末段**，不泄露绝对路径。撤销入口指向设置页「已授权操作」。
3. **action 名必须字典翻译**，不得直接渲染 wire 值（`purge` 是技术术语，属 §7.3 的「技术细节下沉」）；绝对路径与原始 action 名只进折叠区（N15）。

呈现方式**只做按需展示**：默认随按钮悬停/聚焦显示，不常驻占行。**本次不做「偏好项常驻」开关**——现有设置面没有对应键，新增需走 Rust 配置组 + DTO + `contract:gen`，而常驻只是重复悬停已能查到的内容，与判据 B（默认态零信息）方向相反（§12）。

### 8.3 二次确认

现状 `始终允许` 已有 `Popconfirm`（`always_confirm` 键），沿用并扩充：确认内容改为将放行的 `intents` 全量清单 + 撤销入口；`本会话允许` 不弹确认（会话结束即失效）。**`拒绝` 也永不弹确认**——理由为可选展开（§8.5），不给「不写理由」的用户增加点击成本。

### 8.4 拒绝后果

折叠区末尾一句：「拒绝后这次不会执行该操作；模型会收到拒绝结果，可能改用其它方式完成任务」。消除「拒绝=中断」误解（S10）。

### 8.5 拒绝理由（可选输入）

**现状：wire 已就绪，前端从未使用。** `ToolApprovalDecisionDto` 已有 `reason: Option<String>`（`#[ts(optional = nullable)]`，[agent.rs](../../../src-tauri/crates/assistant-protocol/src/agent.rs)），后端链路完整：

| 环节 | 现状 |
| --- | --- |
| 命令层 | `resolve_agent_tool_call` 把 `args.reason` 装进 `ApprovalDecision::Deny { reason }`（[cmd/agent/mod.rs](../../../src-tauri/src/cmd/agent/mod.rs)） |
| 网关 | `Deny { reason }` → `GatewayDecision::RejectedByUser { reason }`（[gateway/mod.rs](../../../src-tauri/crates/assistant-tools/src/runtime/gateway/mod.rs)） |
| 工具结果 | `Some(reason)` → `"rejected by user: <reason>"`；`None` → `"rejected by user"`（[pipeline.rs](../../../src-tauri/crates/assistant-tools/src/runtime/pipeline.rs)） |
| 前端 | `chatService.resolveAgentToolApproval` 原样透传 `ToolApprovalDecision`，**当前调用未传 `reason`**（[ApprovalCard.tsx](../../../src/chat/tool/ApprovalCard.tsx) `decide`） |

**因此拒绝理由这部分无需新增 wire 字段**：生成类型里 `reason?: string | null` 已存在（`src/types/protocol.contract.ts`），改动集中在前端 UI + i18n（§10.2 的结构化字段是另一组改动，两组同批交付）。

**硬前提：拒绝路径不增加任何点击成本。** 大多数用户拒就是不写理由，所以「拒绝」保持现状——**单击即提交**，不弹面板、不弹确认、不多一次点击。理由只在用户主动要求时才出现，走 §8.2 的「按需展示」：常态不占行、不占焦点。

常态动作行（与现在完全一致，仅右侧多一个弱化文本入口）：

```
[允许一次] [拒绝] [本会话允许] [始终允许]          填写理由
```

- 点「拒绝」→ 立即提交 `reason` 缺省（`undefined`），与改版前行为逐字节一致。
- 点「填写理由」→ 在卡片内展开一行单行输入（不换卡片、不新开层、不遮挡），自动聚焦；这是**唯一**会额外占行的情形，且由用户主动触发，不进入行预算常态核算。

展开态：

```
[允许一次] [拒绝] [本会话允许] [始终允许]          收起
┌────────────────────────────────────────────────────┐
│ 可选：为什么拒绝（例如「这个目录不能动」「先备份再删」） │
└────────────────────────────────────────────────────┘
模型会看到这段话 · 留空则与不填一样
```

此时点「拒绝」= 一次点击带理由提交；**仍不弹二次确认**——理由框本身就是那个「确认」，再加一层确认就是双重成本。

规则：

1. **默认路径零成本**。理由框默认不渲染；不展开、直接点「拒绝」= 与现状完全一致的拒绝。**不得因为「支持理由」而把拒绝变成两步**（禁止 Popconfirm、禁止模态、禁止「先展开再确认」）。
2. **理由可选且不传空串**。`trim()` 后为空 → 请求体**省略** `reason` 字段（不是 `""`，否则模型读到 `"rejected by user: "`）。
3. **仅 `deny` 携带 `reason`**。`approve` / `allowSession` / `allowAlways` 不传该字段（后端也只读 `Deny` 分支）；展开后改点其它按钮 → 输入内容不发送、框自动收起，不打断放行。
4. **纯文本 + 长度上限**。`maxLength` 限制（建议 200 字符），提交前 `trim()`；不做 Markdown/HTML 渲染，不解析其中的路径或命令（N10 同理）。回车**不提交**（避免误拒），只接受点击「拒绝」。
5. **理由会进入模型上下文**（作为工具结果文本），框下方常驻一句「模型会看到这段话」，避免误认为本地备注。它**不写审计表**（`Outcome::Rejected` 无理由列；审计扩展见 §10.2）。
6. **超时态不可拒绝**，故理由入口一并隐藏（按钮已置灰，§8.6）。
7. **提交失败**（`NoPendingInteraction`，已超时/已决策）沿用现有 `errorText` 展示；框保持展开、已输入内容不清空，允许直接再点「拒绝」重试——不静默丢弃用户输入。

### 8.6 超时态

超时后卡片整体降为只读：按钮置灰，行 1 位置替换为「已超时，本次按拒绝处理」。

---

## 9. 降级与不变式

沿用 I38d 硬约束，新增 N1–N19 与 R1–R2：

- **N1** 规模段无值即省略：只有 `edit` 的 `old_string` / `new_string` 行数、`super_edit` 的行区间可计数，其余场景依赖 `preview.entries` / `bytes`；`preview` 为 `null` 时同样省略，不显示「0 项 / 0 字节」。
- **N2** `preview.truncated`：在折叠区 `summary` 前标「预演已截断」，不得据此把量级写成精确值；`summary` 为空时无标注可附（§3.1）。
- **N3** `preview.estimated`：在折叠区 `summary` 前标「按参数估算」，不四舍五入成精确数；`summary` 为空时同上。
- **N4** 未知 `extension.kind` / `code`：不丢弃、按纯文本（`kind.code`）进折叠区，仍参与 severity 排序。
- **N5** 不解析 `preview.summary` 的内部结构：只按 `\n` 分行渲染（修复 S3），不改写措辞；`summary` 为空即不渲染。**补充**：当结构化证据（参数合成 diff / 命令全文）已覆盖同一内容时，`summary` 不重复渲染（§5.3）。
- **N6** 不推断缺失参数：`arguments` 缺字段就降级，不猜路径/命令。
- **N7** 诊断项不得消失：`readOnly` 异常、预演降级等一律进折叠区，不因「首屏简化」被删。
- **N8** 决策动作仍是四个，不新增「查看文件」「编辑参数」等入口。
- **N9** 行数守恒：新增一行必须挤掉一行（§4.1）。
- **N10** 证据块内容是纯文本：不渲染 Markdown/HTML，不解析 URL 为链接；截断只裁行数并显式标注。
- **N11** 审批载荷不脱敏（用户此刻就在授权），仅范围说明与首屏对象使用路径末段。
- **N12** 首屏不得出现常量字段与默认值（§3.2 / 判据 B）；新增工具接入时必须先声明其「随调用变化的字段集」。
- **N13** 有效性判定（会失败、参数非法）不得进入卡片：应在 `validate_input` 或执行阶段处理。
- **N14** 空要素必须整段消失，不得留下分隔符或空槽（禁止 `动词 · · 后果`）。要素有无由数据决定，不由工具名决定。
- **N15** `intents` 的原始 `action` 名与绝对路径只在折叠区出现；首屏一律用 action 字典人话 + 路径末段（§8.2）。
- **N16** `extensions` 的 wire 为 `kind` + `code` + `severity`，**不含 `label` / `value`**；首屏只允许 **`kind.code` 复合键字典**的本地化文案（§3.1）。未知 `kind.code` 原样进折叠区，不本地化、不丢弃。
- **N17** 拒绝理由不改变拒绝路径：`deny` 仍为**单击即提交**，理由输入默认不渲染、由用户主动展开；只走 `deny`，`trim()` 后为空必须**省略字段**（不传空串），长度受 `maxLength` 约束，纯文本不渲染；理由只进模型上下文、不写审计（§8.5）。
- **N18** 路径显示单点：首屏对象与 `resources` 路径项一律取末段，复用 `workspaceBasename`，**不在前端做绝对 → 相对化**。前端能拿到工作区根（`useChatStore` 的 `workspaceRoot`），但相对化需复刻后端 `canonicalize_lenient` 语义（verbatim 前缀、大小写、分隔符、`..` 与盘符相对路径拒绝、符号链接），属重算后端事实；需要相对路径时由后端结构化上 wire（§4.2 对象段、§0）。
- **N19** 卡片只对 `audience().agent` 的工具产生：受众判断必须在**执行入口**强制（`find_for_agent`），不得只依赖列表过滤（§2.5）。`TOOLBOX_ONLY` 工具（当前仅 `fs_link`）不产生卡片，其意图词汇只服务 ToolBox 路径的策略求值。
- **R1** 命令块（行 2）恒展开，不默认折叠：它是用户唯一能核对「我批准了什么」的证据。
- **R2** `shell` 含控制操作符时**不产生「只读」结论**：结构未解析前，任何「只读」判断都是猜测（类别须由 §6.7 结构化输出给出）。

---

## 10. 改动面

### 10.1 前端

| 文件 | 改动 |
| --- | --- |
| `src/chat/tool/approvalPresentation/`（新增目录模块，入口 `index.ts`） | 纯函数 `buildApprovalView(approval) → ApprovalView`：对 §5.1 决策要素**求值**（有值则产出、无值则省略），动词与后果由 §5.2 参数语义字典驱动，证据形态由参数形态决定（§5.3）；按 §7.2 优先级选取原因（只产出 `reasons.primary` / `reasons.rest` 两个数据槽，**不产出任何本地化文案**）；内含四张纯表：参数语义字典、`TOOL_PROFILES`（§5.5）、`extension kind.code` 字典（§3.1）与 `intent action` 字典（§8.2）；不依赖 React、不按 `toolName` 分支（唯一例外：`TOOL_PROFILES` 查表与未知工具回显）。按 `docs/principle/CODE-SPLIT-PRINCIPLE.md` 按职责拆为 `types.ts`（视图契约）/ `tables.ts`（四表与阈值）/ `text.ts`（叶子纯函数）/ `elements.ts`（要素求值 + 取词）/ `evidence.ts`（证据块）/ `diagnostics.ts`（诊断与降级）/ `detail.ts`（原因 / 授权范围 / 区块顺序）/ `view.ts`（组合入口），依赖单向无环 |
| `src/chat/tool/ApprovalCard.tsx` | 按视图模型渲染 4 行结构；折叠区按 §4.5 固定顺序、入口文案按实际区块拼接；保留现有 reason 六分支与四按钮逻辑；删除 `row` 平铺；`decide` 增加可选 `reason` 透传（仅 `deny`，空则省略）；「填写理由」为弱化文本入口，默认不渲染输入框（§8.5） |
| `src/chat/tool/ApprovalCard.module.css` | 新增决策句（动词加粗）/ 命令块（等宽 + `white-space: pre-wrap`）/ 危险标记 / 折叠区 / 展开态单行理由输入；删除 `row` 平铺样式 |
| `src/chat/tool/toolCallDisplay.ts` | 复用 `summarizeArguments`、`sortApprovalExtensions`；新增 `formatAction`（action 字典）。末段复用 `src/chat/core/workspaceDisplay.ts` 的 `workspaceBasename`（已含 verbatim 前缀剥离、双分隔符、尾分隔符，见 N18），**不新增 `formatPathTail`** |
| `src/i18n/locales/zh-CN.json` / `en-US.json` | 新增动词、后果句、范围说明、拒绝后果、诊断项、`extension` 的 `kind.code` 复合键文案（如 `extension.workspace.outside`）与 `intent.action` 文案；新增拒绝理由文案（`deny_reason_open` / `deny_reason_close` / `deny_reason_placeholder` / `deny_reason_hint`）；移除 `always_hint` 固定行 |
| `src/chat/tool/__tests__/approvalPresentation.test.ts`（新增） | 视图模型纯函数单测：A1–A8、A9b、A10b–A10f、A12、A13–A13f（A9 只测首屏部分、A13b 只测键集合）；含 `TOOL_PROFILES` 键集合与 Rust `TOOL_ID` 的源码扫描比对（§5.5）。A11 与 A13e 的签名断言不在本文件（§11） |
| `src/chat/tool/__tests__/approvalRulesUi.test.ts` | 追加结构断言（4 行结构、`white-space` 保留、四按钮不变、`always_hint` 不再被引用、折叠区截断 / 估算标注）；`reasonText` 六分支的现有断言沿用不改（A11） |
| `src/chat/tool/ToolCallBubble.tsx` | 等待审批态隐藏上方 `name + argsSummary` 行，避免与行 1 / 证据块重复 |

后端改动见 §10.2（同批交付）。

### 10.2 后端

本节全部是本次改动的一部分，**与前端同批交付**；「是否改 wire」只说明需要多跑一次 `npm run contract:gen` 并提交生成类型，不构成延后理由。

| 议题 | 现状 | 改动 | 改 wire | 对应验收项 |
| --- | --- | --- | --- | --- |
| 目录非空检查位置 | 在 `execute` 阶段才调用 `ensure_empty_directory`——回收站分支 [trash.rs](../../../src-tauri/crates/assistant-tools/src/os/tools/agent_delete/trash.rs) 与永久分支 [exec.rs](../../../src-tauri/crates/assistant-tools/src/os/tools/agent_delete/exec.rs) 各一处，用户先批准再失败 | 上移到 `validate_input`，审批前即返回「pass recursive = true」。这是 N13 与 N5 冲突（`summary` 里的 `"only deleted if it is empty"` 有效性陈述会被原样渲染）的唯一低成本解法 | 否 | D21 |
| `summary_for` 语句数与语言混排（S9） | 固定输出「执行 1 条命令」，`a && b` 仍说 1 条；`delete` 英文、其余中文 | 删除 `summary_for`，`shell` 的 `summary` 置空串（语句数 / 类别 / 隔离强度全部改由结构化字段给出，§5.3、§6.7） | 是 | A8、S9 |
| 受众过滤 fail-open | 列表接口按 `audience()` 过滤，但执行期 `find_by_name` / `find` 无受众判断——模型回传 `fs_link` 即可在工作区建链接绕过沙箱边界（§2.5） | 新增 `find_for_agent(name)` 作为模型调用名的唯一解析入口，`pipeline` 与 `tool_escalation` 全改用它；`register_os_tools` 改名 `register_bridge_tools` | 否 | B18b / B18c |
| `workspace` 扩展误判（§5.6.8 G1） | `workspace_extensions` 对每条 `pending.resource` 无条件做工作区成员判定；`shell`（命令原文）与 MCP（`mcp:<server>:<hash>`）不是路径，解析必然失败 → 恒判「工作区外」 | 只对路径型 action（`read` / `readToolOutput` / `edit` / `delete` / `purge`）判定，其余不产出 `workspace` 条目 | 否 | B18d |
| `preview` 规模结构化 | `entries` / `bytes` / 目标类型 / 是否存在 / 是否链接只在散文 `summary` 里 | `ApprovalPreviewDto` 增结构化字段（`entries` / `bytes` / `targetKind` / `exists` / `symlink`），各工具 `preview_for_approval` 同步填充 | 是 | A3 / A4 / A9 / A9b、规模段、§4.3 草图 |
| `shell` 语义理解 | 仅黑名单匹配，控制操作符一律 `Unknown` | 按 §6.7 S1–S4 实现解析器，`preview` 增并行结构（语句数 / 串联数 / 类别 / 重定向 / 内嵌载荷 / `targetsScope`） | 是 | A8、shell 类别与语句数、边界段 |
| `mkdir` 无动词来源（§5.6.8 G2） | `intents` action 与 `write` / `edit` 同为 `edit`，参数只有 `path` + `recursive`，前端无法在不按 `toolName` 分支的前提下得出「新建目录」 | `preview` 结构化增 `verbCode`（如 `mkdir`），前端按 code 取词，参数形态不参与 | 是 | A13f |
| `extensions.label` 中英混排 | 各工具硬编码 `label` / `value` 散文 | wire 收敛为 `kind` + `code` + `severity`，**删除 `label` / `value`**；`sandbox.root` / `output.scope` 产出点整体删除；前端按 `kind.code` 复合键字典本地化 | 是 | A13c 升级为「无英文原文」 |

`contract:gen` 的落地方式：改 DTO → `npm run contract:gen` → 提交 `src/types/protocol.contract.ts` 与 `src/vault/core/contract/vault.contract.ts`，按项目类型安全规范（输出 DTO `Option` 不加 `skip_serializing_if`，输入 DTO `Option` 加 `#[ts(optional = nullable)]`）。

§0 标「本次补齐」的四类字段全部落在本表后三行：**规模段与 shell 结构的数据源由本次改动提供**，验收项直接覆盖它们（A3 / A4 / A8 / A9 / A9b / A13c / A13f、D21b / D21c）。

### 10.3 设计决策记录

| # | 议题 | 选择 | 理由 |
| --- | --- | --- | --- |
| D1 | 判据 | 「后果 + 授权范围」两条 | 有效性/校验不属于审批信息（§2.2） |
| D2 | 字段去留 | 按区分度判定，不按行数 | 删行会连带删掉证据（§2.3 判据 A） |
| D3 | 行数 | 常态 3 / 命令形态 4 / 硬顶 4 | 行数由「必须回答几个问题」决定 |
| D4 | 证据块数据来源 | 前端从 `arguments` 合成 | 零 wire 变更；`arguments` 是模型原文 |
| D5 | `preview.summary` 散文 | 仅作诊断 / 降级原文，首屏不渲染；结构化齐备时置空串 | 后端事实不重写；混排由 `kind + code` 与结构化字段替换（§5.3、§10.2 S9） |
| D6 | `edit` diff 位置 | 折叠区，不进首屏 | diff 是核对材料，非决策结论（§5.2） |
| D7 | 四按钮 | 保留，靠范围说明降风险 | I38d 硬约束；不减能力 |
| D8 | `tier` 徽标与倒计时 | 保留但降权（`sandbox` 隐藏） | 默认态零信息 |
| D9 | 展示决策归属 | **三层分离**：内容由数据、语义由参数字典、呈现由前端 | 「前端决定展示什么」不成立，准确说法是「前端决定如何呈现」（§2.4）。显示哪些要素由参数形态求值得出，不由 `toolName` 查表（§5.1）。新增工具只需新增参数名翻译，不需新增模板 |
| D10 | 参数名歧义 | 字典 key 含判别参数 | `source` + `target` 当前只属 `move`，无歧义；未来工具复用该组合必须给出判别参数，否则不产出动词（宁缺勿错，§5.2）。判别参数来自 `arguments`，仍属数据驱动 |
| D11 | 不可证事实 | 不显示，不猜 | 「无自动备份」既非参数可证也非后端 wire 提供，写它就是前端自造事实（§5.2） |

---

## 11. 验收判据

**A. 纯函数（`approvalPresentation.test.ts`；A11 与 A13e 的签名部分除外）**

1. `delete` + `permanent=true` → 行 1 动词为「永久删除」，后果含「无法恢复」。
2. `delete` + `permanent=false` → 动词为「删除」，后果含「回收站」「可恢复」。
3. `delete` + `recursive=true` → 动词为「删除目录」，规模段含「N 个文件 + M 个子目录」（`preview.entries`）。
3b. `delete` + `permanent=true` + `recursive=true` → 动词为「永久删除目录」（组合查表），**不得**拼成「永久删除删除目录」（§5.2、G3）。
4. `write` + `append=true` → 动词「追加到」+ 后果「已有内容保留」；`append=false` 且目标不存在 → 动词「新建」；`append=false` 且目标存在 → 动词「覆盖」+ 后果「原内容被替换」，**不出现「写入」这种含糊动词**（§5.2、`preview.exists`）。
5. `move` + `overwrite=true` → 后果为条件句「目标已存在时原内容被替换」，不得写成「目标已存在」断言；`overwrite=false` → 「不会覆盖已有目标」（N13）。
6. `edit` → 规模段为 `N 行 → M 行`，diff 进 `detail` 且保留 `\n`；`super_edit` → 规模段为 `startLine–endLine 行 → M 行`（均由参数计数，§5.6.4）。
7. `shell` 命令长度未超阈值 → 对象段为命令原文；超阈值**或**含控制操作符 → 命令全文进证据块（`detail`）且未被截断、保留 `\n`（§5.3、R1）。
8. `shell` 含 `&&` → 行 1 含语句数「2 条串联」与类别；含 `>` → 行 1 含重定向目标；单条命令**不出现**「1 条」规模段；`targetsScope=inside` → 边界段「改动限于工作区内」，`unknown` / `null` → 边界段整段消失；以上均**不产生「只读」结论**（R2、§6.5）。
9. `preview=null` → 无规模段（N1）；`truncated` / `estimated` 不进首屏（首屏不出现「≥」及任何截断 / 估算措辞），也不得据此把 `entries` / `bytes` 写成精确值（N2 / N3）。折叠区 `summary` 前的「预演已截断」/「按参数估算」标注属渲染职责，由 B14 的折叠区结构断言覆盖，本文件不测。
9b. 目标为链接 → 后果「只删链接，不碰目标」（`preview.symlink`）。
10. 未知 `toolName`（无 `intents`、无参数字典命中）→ 求值视图只含「调用 + `toolName` + 参数摘要」（无后果段），不抛异常、不猜字段（§5.4、N14）。
10b. 要素缺失时不产生空槽与多余分隔符：`read` 求值结果为两段（动词 + 对象）、`delete`（`permanent=false`，`preview.bytes` 有值）为四段（动词 + 对象 + 规模 + 后果），且都不含 `· ·`（N14）。
10c. 参数名歧义不跨工具复用：`move`（`source` + `target`，无判别参数）→ 动词「移动」；构造含 `source` + `target` + 未知判别取值的视图 → **不产出动词**，不得回落成「移动」（§5.2、D10）。
10d. 不可证事实不出现：任何视图的后果段都不含「自动备份」「无备份」字样（§5.2、D11）。
10e. 有效性措辞不进卡片：后果段不含「会失败」「可能失败」（N13）。
10f. `mcp__<server>__<tool>`（`intents[0].action = "mcp"`）→ 动词为「调用外部工具」，对象段为 `toolName` + 参数摘要（**不含** `mcp:<server>:<hash>` 的 hash 末段），无后果段（§5.4 / §5.6.7）。
11. 六种 `reason` 全覆盖且无 `default` 兜底——**不在本文件断言**：`reasonText` 留在 `ApprovalCard.tsx`（§7.1），沿用 `approvalRulesUi.test.ts` 的现有源码断言，本文件不重复。
12. 多原因时主句取优先级最高者（视图模型产出 `reasons.primary`），其余全部进 `reasons.rest`，渲染时落折叠区 `detail`（§7.2；选取归视图模型、文案归卡片）。
13. `destructive=true` 对 `delete` / `shell` / `super_edit`、`openWorld=true` 对 `shell` **均不产生首屏文案**（N12、§5.5）。
13b. `TOOL_PROFILES` 键集合 = Rust `TOOL_ID` 集合扣除 `TOOLBOX_ONLY_IDS`（当前 14 个，排除清单当前只含 `fs_link`）。**只断言键集合**：条目字段（`destructive` / `openWorld` 等）与 §5.5 表的字面量比对属变更检测，其行为侧由 A13 覆盖，不重复断言（§5.5）。
13c. `extensions` 首屏只出现 `kind.code` 字典文案，不含任何英文散文；未知 `kind.code` 折叠区保留原文（N16）。
13d. `intents` 首屏为 action 人话 + 路径末段，不含 `purge` 字样与绝对路径；折叠区保留原文（N15、§8.2）。
13e. 路径显示单点：`arguments.path` 与 `intents.resources` 的绝对路径在首屏只剩末段（verbatim 前缀、双分隔符、尾分隔符均已归一，复用 `workspaceBasename`）；不产出相对路径（N18、§4.2 对象段）。签名层面（不接收工作区根、不引用 `useChatStore`）属源码断言，并入 B18。
13f. `mkdir` → 行 1 动词为「新建目录」（取自 `preview.verbCode`），**不回退成「修改」**、不抛异常（§5.2、G2）。

**B. 结构断言（源码层，沿用 `panelStructure` 口径）**

14. `ApprovalCard.tsx` 只有 4 个区块标记（决策句 / 证据 / 按钮 / 折叠），无 `row` 平铺；折叠区区块顺序符合 §4.5。
15. 证据块容器含 `white-space: pre-wrap`，且使用等宽字体。
16. 决策按钮仍为四个，文案键不变。
17. `always_hint` 固定文案不再被引用（改为动态范围说明键）。
18. `approvalPresentation/` 整组模块不 import React，且不含 `toolName ===` 分支（`TOOL_PROFILES` 查表与未知工具回显除外）；同时不引用 `useChatStore` / 工作区根，签名不接收工作区根（N18、A13e）。
18b. 受众守卫（Rust，`tests/registration.rs`）：`TOOLBOX_ONLY` 工具（当前 `fs_link`）`find_for_agent(name)` 为 `None`、`find_for_toolbox(id)` 为 `Some`；`TOOLBOX_ONLY` 集合当前等于 `{"fs_link"}`（§2.5）。
18c. 执行期守卫（Rust，`tests/pipeline.rs`）：以 `tool_name = "fs_link"` 构造调用 → 在 `Find` 阶段失败、错误含 `unknown tool`、临时工作区内无新建链接（§2.5）。
18d. `workspace` 扩展守卫（Rust，`gateway/context.rs` 单测）：`pending` 的 `action` 为 `execute` / `mcp` 时**不产出** `workspace` 条目；`edit` / `delete` / `read` / `readToolOutput` 照常判定（§5.6.8 G1）。同时断言 `extensions` 的 `kind` 只可能是 `workspace` / `sandbox.enforcement`，且每项只有 `kind` / `code` / `severity`（`sandbox.root` / `output.scope` 已无产出点，§3.1）。

**C. 双语**

19. `zh-CN` / `en-US` 新增键完全对齐，占位符集合一致（含动词、后果、规模、边界、`kind.code` 字典、action 字典、折叠区与拒绝理由键）。

**D. 手工**

20. 对 `shell` / `write` / `edit` / `delete` 各触发一次真实审批：首屏 ≤ 4 行且无需滚动即可回答 Q1–Q2；`edit` 展开后 diff 换行正确；永久删除卡片出现「不可恢复」；`shell` 命令原文可见且未被静默截断；超时态置灰。
21. `delete` 非空目录 + `recursive=false`：审批阶段即被拒绝并提示「pass recursive = true」，不再出现「先批准后失败」（§10.2）。
21b. `delete` 目录（`recursive=true`）规模段显示「N 个文件 + M 个子目录」与字节数；`write` 覆盖已有目标显示动词「覆盖」；`mkdir` 行 1 显示「新建目录」（§10.2 结构化字段）。
21c. `shell` 含 `&&` / `>` 时行 1 显示语句数、类别与重定向目标；`bash -c "…"` 显示载荷真命令（§6.7、§10.2）。

**E. 拒绝理由（§8.5）**

22. **默认路径零成本**：不展开理由框、直接点「拒绝」→ 一次点击即提交，全程无输入框、无确认层、无模态；请求体不含 `reason`。
23. 空理由（展开后不填就点「拒绝」）→ 请求体 `reason` 为 `undefined`/`null`（**不得为空串**），模型侧文案为 `rejected by user`。
24. 展开理由框并输入 → 点「拒绝」一次提交，`reason` 为 `trim` 后原文；模型侧文案为 `rejected by user: <reason>`；**不出现二次确认**。
25. `approve` / `allowSession` / `allowAlways` 的请求体**不含** `reason`；展开后改点其它按钮 → 内容不发送且框自动收起。
26. 理由超 `maxLength` 无法继续输入；粘贴超长文本被截断而非拒绝提交；回车不触发提交。
27. 提交失败（已超时 / 已决策）时错误文案可见，且理由框保持展开、已输入理由**未被清空**。
28. 超时态下理由入口与四按钮一起隐藏/置灰（§8.6）。

---

## 12. 不做项

- 不做卡片内 diff 编辑（证据块只读）。
- 不做 AI 顾问式解释（`advisor` 仅翻译既有原因）。
- 不做卡片内撤销授权（只给设置页入口）。
- 不新增审批动作或批量审批（仍一次一个）。
- 不把「会失败」等有效性信息搬进卡片（N13）。
- 不为 `TOOLBOX_ONLY` 工具（当前 `fs_link`）设计卡片呈现：受众过滤由后端执行期强制（§2.5、N19），前端不为它预留模板或动词分支。
- 不引入 jsdom / testing-library（沿用源码断言 + 纯函数单测口径）。
- 不把「需要改 wire」当作缩减范围的理由：§10.2 的结构化字段属于本次交付，`contract:gen` 只是同一批改动里的一个步骤。
- 拒绝理由不做结构化分类（不提供「原因下拉 + 预置标签」），只做自由文本；不持久化、不入审计、不做历史回填（§8.5、§10.2）。
- 拒绝理由不做 Markdown / 富文本 / 附件，不解析其中的路径或命令（N17）。
- 不为拒绝加二次确认或弹层：拒绝保持单击生效，理由输入仅为可选展开（§8.5 规则 1、N17）。
- 不做「范围说明常驻一行」的偏好开关：只做悬停/聚焦按需展示（§8.2）。
- 不为 `extensions` 保留 `label` / `value` 兼容字段：前后端同批交付，无过渡期形态（§3.1、N16）。
- 不保留 `sandbox.root` / `output.scope` 扩展的「产出但前端不显示」版本：产出点整体删除（§3.1）。
# I58 — P17：超级工具规格（能力发现、索引型读取与行地址编辑）

| 项 | 值 |
|---|---|
| 阶段 | **P17 超级工具（能力发现与索引型读取）**；切片 T1–T8（§11）。T1 无前置；**T2–T6 无 P16 实现依赖**（只对齐 A 的格式字面量与回执字段，§0.3）；T7 依赖 T6、T8 依赖 T7 |
| 状态 | 草稿 **实现独立**：B 自写扫描 / 渲染 / 索引，只对齐 A 的格式字面量与回执字段（§0.3）。**2026-11-18 设计修订**：①索引正确性由「写侧主动通知失效」改为「行起点自证」，A 的写侧工具零改动（§4 / §9 L11·L12 / §12 R10–R12）；②索引层抽为 B 私有公共层 `super_index/{format,cache,proof}.rs`，`super_read` 与 `super_edit` 消费同一份实现（§7.2 / §12 R14）。**2026-11-19 设计修订**：③注入策略由「伴随展开（勾基工具即注入）」改为「**伴随准入 + 模型申请**」——勾基工具只给超级工具「可被申请」的资格，模型判断能力不足后经 `request_tool(detail = "enable")` 申请，由循环在下一迭代尾部追加；用户不勾选，面板以**伴随形式**展示（从属行、无独立勾选框）（§3.1 / §3.6 / §3.7 / §9 L4·L15·L17 / §10 G4·G17·G20–G22 / §11 T7·T8 / §12 R7·R15–R17）；④**描述下发与可用性解耦**——提示词分 `none` / `brief` / `full` 三档，`brief` 下发而不可调用是合法态；面板改为**伴随展示**（从属行、无勾选框、状态随基工具），默认「跟随基工具可用」（§3.6 / §10 G23 / §11 T7 / §12 R18 / §9 L18）；⑤**配置路径改为身份过滤**——`expand_enabled_tools` → `exclude_companions`，白名单里任何伴随 id 一律排除（不查询基工具、`disabledTools` 中的伴随 id 同样忽略），确保 `callable` 只有「模型申请」一个来源，并消除「禁用基工具而伴随残留」的过滤顺序缺陷（§3.6 / §10 G17·G21 / §11 T7 / §12 R19） |
| 日期 | 2026-11-18 |
| 阶段台账 | [I38 §2](./I38-agent-roadmap.md)（阶段、前置、状态的唯一登记点）；本文 §10 的 G1–G23 即 P17 退出条件 |
| 关联 | [I57](./I57-long-line-read-spec.md)（**P16 阶段规格**：`read` 的水平窗口与扫描收口；本文只与其 §0.3 的 **P2 / P3** 对齐——footer 登记表与回执字段，P1 的 `LineSink` 不被 B 消费——并沿用其 §9 的 L1 / L3 / L4 / L6 / L7 边界）、[I27](./I27-tool-data-store-spec.md)（落盘阈值与预览窗口）、[I38f](./I38f-agent-subagent-spec.md)（`ToolAudience` / `AGENT_ONLY` 先例）、I38c 各分册（已归档）（工具描述三档与 footer 五条规则来源）、[CODE-SPLIT-PRINCIPLE](../principle/CODE-SPLIT-PRINCIPLE.md)（拆分原则唯一来源，本文只引用不复述） |
| 影响范围 | `assistant-tools`：`src/contract/trait.rs`（`Tool::manual()`）、`src/runtime/registry.rs`（手册派生）、`src/os/tools/agent_request_tool.rs`（新增）、`src/os/tools/super_index/{mod.rs,format.rs,cache.rs,proof.rs}`（新增，B 私有公共层）、`src/os/tools/super_read/{mod.rs,render.rs}`（新增 / 消费公共层）、`src/os/tools/super_edit/{mod.rs,locate.rs,apply.rs}`（新增）、`src/os/mod.rs`（注册组）、`src/runtime/storage.rs`（回执写入方扩展）。`assistant-agent`：`src/agent/round_executor.rs`（`ToolExecCtx` 增 `tools`）、`src/agent/loop_executor/execute.rs`（唯一构造点）。`src-tauri/src`：`tool_setup.rs`（注册 meta / super 组）、`adapters/tool_exec.rs`（`available_tool_names` / 升级端口透传）、`src-tauri/src/adapters/tool_escalation.rs`（新增：升级端口实现）、`src-tauri/src/adapters/tool_resolver.rs`（`exclude_companions` 身份过滤与 `request_tool` 并入）、`app_dirs/{roots.rs,dir_name.rs}`（索引目录）。前端：`src/chat/tool/ToolConfigPanel.tsx`（伴随条目渲染为基工具下的从属行，无勾选框）+ `src/chat/core/toolConfig.ts`（分组 / 计数排除伴随条目）+ i18n `chat.tool.panel.companion_of` / `companion_blocked`；不改 wire / DTO。测试：`crates/assistant-tools/tests/{super_read.rs,super_edit.rs,request_tool.rs}`。**不改** wire / DTO、**不改 A 的 `read` / `edit` / `write` / `delete` / `move` 任何实现与既有输出**（A 侧零改动，§12 R10）；`read` / `edit` 结果里的 `[hint]` 文案与门控属 B 侧调整（§3.6） |

> 一句话：A 把长行做成「有界但不完整」，B 用**显式成本**换完整性——首次调用建一份落盘行索引，之后按行 `seek` 取任意行任意列；同时用 `request_tool` 把「有哪些工具、每个工具怎么用、代价多大」变成模型可主动索取的信息，而不是塞进每轮都重复的系统提示。

**编号约定**：`G`（门禁）、`T`（切片）、`L`（已知限制）、`R`（拒绝方案）、`OQ`（未决问题）、`N`（现状事实）、`F`（失败项）在本文与 [I57](./I57-long-line-read-spec.md) 中**同名独立**。跨文档引用必须带文档前缀（`I57 §9 L1` / `I57 §3.3`），裸写 `§9` 一律指本文。

**骨架对齐**：本文与 I57 逐节对应——`§9` 恒为「已知限制」、`§10` 恒为「门禁」、`§11` 恒为「切片」。

---

## §0 范围与依赖

### 0.1 三个工具的分工

| 工具 | 解决什么 | 不解决什么 |
|---|---|---|
| `request_tool` | 「我有哪些工具、怎么调、代价多大」——按需返回**本轮可用集**与手册；能力不足时可申请把超级工具**本轮启用** | 不自行注入：`detail = "enable"` 只登记申请，由循环在下一迭代尾部追加，且基工具必须在本轮；自身也是普通勾选项（未勾则不存在，§3.6） |
| `super_read` | 长行任意段、任意行、精确 `totalLines`，代价 = 一次建索引 | 不改文件，不替代 `read` 的默认路径，不默认注入（须经申请） |
| `super_edit` | 按**行地址区间**替换，`expected` 前缀校验防漂移 | 不做子串匹配、不做多处编辑，不默认注入（须经申请） |

### 0.2 与 A 的切分轴

沿用 I57 §0.2：**一次线性扫描内能否有界完成**。B 的全部内容都需要跨调用状态（行索引）或随机访问（`seek`），因此不在 `read` 内隐式发生，而是**独立工具 + 显式成本**。

### 0.3 依赖方向：契约对齐、实现独立

单向：**B 可引用 A 的任意节；A 侧对 B 的感知只有两处文案追加（§7.1 的「B 侧最小触碰」），且不产生任何行为分支**——`read` / `edit` 的既有输出、判据、字段一律不变，`[hint]` 只是尾部多一行。

本阶段不再把 A 的实现当作 B 的底座。分清两层，避免「为了复用而自我设限」：

| 层 | 内容 | 约束强度 |
|---|---|---|
| **对齐层（硬）** | 行格式字面量（`{number:>6}` + `\t`、` [truncated]`、footer 形态 `[<tag>: …]`）、footer 登记表追加、`ReadReceipt` 字段与 `data` 既有键名 | **必须逐字一致**。理由不是「复用代码」，而是**互操作**：模型会把 `super_read` 的输出片段当作 `edit` 的 `old_string`，`edit` 档 2 靠这些字面量剥前缀、`trailing_footer_marker` 靠方括号形态识别 footer；不一致会让 `edit` 静默匹配失败（§12 R6） |
| **实现层（自由）** | 扫描器、解码循环、行渲染器、索引格式与落盘、缓存 / 单飞 / 淘汰、列推进、`data` 新增键 | **B 自持，不复用 A 的 `LineSink`**。索引可以加 CRC、压缩、列提示；生命周期可以比 A 复杂得多 |

| 依赖 | 内容 |
|---|---|
| `P16 §3.3` → `T3` | footer 标记集是**共享登记表**（`os/shared/footer.rs`）：B 加 tag 只改这一处，`edit` 的识别自动跟随（对齐层，§3.4） |
| `P16 §3.5` → `T4` | 回执字段由 A 拥有，`super_read` 必须写同一份回执（同闸门语义），**不加字段**（对齐层；`edit` 的闸门读它） |
| `P4` / `P5` / `P13` → `T1` | 本轮可用集来自 `TurnRunCtx.tools`（生产者是 P5 的 `resolve_available_tools`） |
| `I57 §3.2` → `T2` / `T3` | 只对齐**行数与切行判据**（`MAX_LINE_CHARS` 默认宽度、`\n` 计数 + 末尾残留 +1、BOM-only = 0），不复用扫描代码 |

`T1` 不读文件、不消费任何 A 契约，与 P16 完全解耦；`T2–T6` 与 P16 **无实现依赖**（P16 已完成只是解除「先后顺序」的排期约束，不是技术前置）。

### 0.4 契约复用表

| 契约 | 拥有方 | B 的动作 |
|---|---|---|
| 行格式**字面量**（`LINE_NUMBER_WIDTH = 6` / `LINE_NUMBER_SEPARATOR = '\t'` / `LINE_TRUNCATED_MARKER = " [truncated]"` / 位置） | A（互操作契约） | **对齐，不改**；可引用常量，但不复用 A 的渲染代码 |
| 行渲染**实现**（`LineSink` / `TextSink` / 扫描循环） | 无（B 自持） | `super_read/render.rs` 自己写；A 的 `LineSink` 不被 B 消费 |
| footer 标记集与顺序 | A | **追加** `catalog` / `index` / `column` / `hint`（§3.4），回写 I57 §3.3 |
| `data` 键名与语义（`startLine` / `endLine` / `totalLines` / `truncated` / `truncatedLines` / `lineWindow` / `linesBeyondColumn`） | A | `super_read` 全沿用，**只增** `index` 一项；B 自有键另起（不得与 A 的键同名改义） |
| `ReadReceipt` 字段（`revision` / `tool_call_id` / `encoding` / `has_bom` / `truncated_lines` / `line_window_max_chars`） | A | `super_read` 写同一份，不加字段（`edit` 的闸门读它） |
| `file_revision` 的**语义**（`len:nanos`，粗粒度漂移提示） | A | 只作粗筛，**不承担正确性**：B 的索引正确性由行起点自证（§4）。不得为了 B 把 `revision` 升级成内容哈希——`read` 是流式的，给回执塞整文件哈希会把成本压到零成本路径上 |
| 截断提示的**归因轴**（`totalLines` 有值 = 读到 EOF；`window_full` = 窗口满） | A | B 可自写文案，但必须同轴、不得新增第五态；`data.truncated` / `truncatedLines` 语义不变 |

---

## §1 现状（代码事实）

| 编号 | 事实 | 位置 |
|---|---|---|
| N1 | `Tool` trait 有 15 个方法，**没有**任何「手册 / 用法」入口；描述只有 `description()` | `assistant-tools/src/contract/trait.rs` |
| N2 | `TurnRunCtx` 已持有本轮工具清单 `tools: Option<&'a [ToolSpec]>` | `assistant-agent/src/agent/round_executor.rs:116` |
| N3 | `ToolExecCtx` **没有** `tools` 字段（`turn_id` / `conversation_id` / `iteration` / `child_token` / `clock` / `observer`），构造点唯一 | `round_executor.rs:149`、`loop_executor/execute.rs:65` |
| N4 | `ToolSpec { name, description, input_schema }`——**无** `id`，因此透传只能按名字 | `assistant-providers/src/types/tool.rs:13` |
| N5 | `resolve_available_tools` 是 `ToolDefinition` → `ToolSpec` 的唯一生产者 | `src-tauri/src/adapters/tool_resolver.rs` |
| N6 | `ToolUseContext` 无「本轮可用工具名」字段；`tool_output_root` 由 Pipeline storage 注入 | `contract/context.rs`、`src-tauri/src/adapters/tool_exec.rs:194` |
| N7 | `ToolRegistry` 有 `find_by_name` / `find_for_toolbox` / `tool_ids` / `tool_definitions_for_tier`（后者按 `tier <= max_tier && audience.agent` 过滤） | `runtime/registry.rs` |
| N8 | `AppDirs` 只有 `data` / `local` / `database` / `attachments` / `tool_results` / `mascot_assets` / `logs` / `crashes`，**没有**索引目录 | `src-tauri/src/app_dirs/roots.rs` |
| N9 | `read` 线性扫描、无 `seek`；`revision` = `len:nanos` | `agent_read/{mod.rs,window.rs}` |
| N10 | `edit` 读整文件（`MAX_WHOLE_FILE_BYTES = 16 MiB`）、按 `old_string` 子串匹配、原子写 | `agent_edit/{mod.rs,apply.rs}` |
| N11 | `ReadReceiptStore` 有 `record` / `lookup` / `invalidate_path`；`InMemoryReadReceiptStore` 是测试实现 | `runtime/storage.rs` |
| N12 | 注册分组为 `register_os_tools` / `register_readonly_tools` / `register_write_tools` / `register_dangerous_tools` / `register_interactive_tools`，签名均为 `&ToolRegistry` | `os/mod.rs`、`src-tauri/src/tool_setup.rs:195` |
| N13 | `resolve_available_tools` 在过滤前调用 `expand_enabled_tools`，把 `companion_of` 的伴随工具**并入白名单**——即当前实现是「勾基工具 = 直接注入」 | `src-tauri/src/adapters/tool_resolver.rs:28`、`assistant-tools/src/runtime/registry.rs` |
| N14 | 两个超级工具的 `companion_of` 已返回 `Some("read")` / `Some("edit")` | `super_read/mod.rs`、`super_edit/mod.rs` |
| N15 | `request_tool` 只有 `detail = "list"` / `"manual"`，**无启用通道**；`tier = Os`、`read_only: true`，注册在只读组 | `agent_request_tool.rs`、`os/mod.rs` |
| N16 | `ToolUseContext` 已有 `available_tool_names` 与 `tool_catalog`，`read` 的 `capability_hint` 以「`available_tool_names` 含 `super_read`」为门 | `contract/context.rs`、`agent_read/mod.rs` |
| N17 | 缺省工具配置 `maxToolTier = "sandbox"`、`enabledTools = []`；`read` / `edit` / 两个超级工具 / `request_tool` 同为 `Os` 层（同层即同层级准入，无跨层问题） | `src/chat/core/toolConfig.ts`、`contract/trait.rs`（`ToolTier`） |
| N18 | 面板把 `companionOf != null` 的条目当普通条目渲染：`Checkbox.Group` 直接遍历 `groupToolsByTier(tools)` 的结果，`enabled_count` 取 `enabledTools.length`，勾选走 `setTierSelection` 整体替换该 tier | `src/chat/tool/ToolConfigPanel.tsx`、`src/chat/core/toolConfig.ts` |

结论：能力发现（T1）与索引型读取（T2–T5）**已落地**；当前缺口有两处——①**注入策略仍是「伴随 = 直接注入」**（N13 / N14），既没有「仅准入」的语义，也没有模型申请通道（N15）；②**面板仍把伴随条目当可勾选项**（N18），与「跟随基工具可用」的语义不一致。本次修订（§3.6）即补这两处：`companion_of` 保留但语义改为准入，`expand_enabled_tools` 改为身份过滤 `exclude_companions`（§12 R19 说明为何不用关系剪枝），新增 `request_tool(detail = "enable")` 与循环尾部追加，面板改为伴随展示。

---

## §2 目标与非目标

### 2.1 目标

1. 模型能**主动问**「有哪些工具、怎么用、代价多大」，且答案以**本轮真实可用集**为准。
2. 长行任意段、任意行可**一次定位**取得，成本在审批摘要里**显式可见**。
3. `edit` 的截断静默问题有第二条出路：按行地址 + `expected` 校验的区间替换。
4. 索引的建立、**自证**、并发、容量四件事各有唯一判据，且失败一律 fail-closed；正确性不依赖任何「写侧通知」。

### 2.2 非目标

- 不把超级工具塞进每轮工具集：它们只在**模型申请**、且**基工具本轮在集**时，由循环在下一迭代**尾部追加**（§3.6）。用户不勾选，面板也不给勾选项。
- 不做会话级升级状态：升级只在当前轮有效，下一轮重新申请（§9 L17）。
- 不替换 `read` / `edit` 的默认路径（它们是零成本路径）。
- 不做子串匹配编辑（§9 L3）。
- 不做跨进程 / 跨会话共享索引（§9 L2）。
- 不让 A 的写侧工具感知 B（§12 R10）；索引正确性靠自证，不靠通知。
- 不改 wire / DTO；前端只做**伴随展示**（缩进在基工具下方、不可勾选、状态随基工具），不提供任何独立开关（§3.6）。
- 不把「描述下发」与「可调用」绑成一档：三档见 §3.6（`none` / `brief` / `full`），模型可以先知道用途、后获得调用权。

---

## §3 契约

### 3.1 `request_tool`

**身份**：`id = "request_tool"`，`tier = ToolTier::Os`，`audience = BOTH`（子代理同样需要发现能力），`behavior_hints { read_only: true, destructive: false, open_world: false }`，无审批（`permission_intents` 返回空）。**注入由用户勾选决定**（`companion_of = None`，按普通条目过滤；未勾或列入 `disabledTools` 即不注入，§3.6）。

**输入**：

| 键 | 类型 | 默认 | 语义 |
|---|---|---|---|
| `detail` | `"list"` \| `"manual"` \| `"enable"` | `"list"` | `list` 只回清单；`manual` 回清单 + 命中工具的手册；`enable` 申请把 `tools` 里的工具**本轮启用**（§3.6） |
| `query` | string | 无 | `manual` 时的匹配串（按 `id` / `name` 子串匹配，大小写不敏感）；缺省则返回全部手册 |
| `tools` | string[] | 无 | `enable` 的申请名单（按 `id` 或 `name` 匹配）；缺失即业务错误（F13） |

**可用集来源**（本切片的核心改动）：

1. `ToolExecCtx` 增 `tools: Option<&'a [ToolSpec]>`（N3 的唯一构造点填本轮工具集；升级后传「基础 + 已升级」合并切片，§3.6）。
2. `src-tauri/src/adapters/tool_exec.rs` 在构造 `ToolUseContext` 时映射出 `available_tool_names: Option<Vec<String>>`（取 `ToolSpec.name`，N4 无 `id`）。
3. `request_tool` 优先用 `available_tool_names`（`data.source = "turn"`）；缺失时回退 `registry.tool_ids()`（`data.source = "registry"`），且**文案必须标注**「回退清单可能含本轮未启用的工具」。
4. 升级端口由适配器注入：`ToolUseContext.escalation: Option<Arc<dyn ToolEscalationPort>>`（`#[serde(skip)]`）。端口只持有**注册表引用、本轮层级、本轮收集器**三样，不读其它状态，且只做校验与登记（§3.6、§3.8）。

**升级通道（`detail = "enable"`，§3.6）**：

- 工具**不写任何状态**：`ToolUseContext` 是不可变借用，`request_tool` 也看不到 provider 请求。它把 `tools` 交给端口，端口逐条校验并把通过项登记到本轮收集器。
- 两条准入，任一不满足即拒绝该条并计入 `data.rejected`：
  1. **基工具本轮在集**：`companion_of()` 返回的基工具 id ∈ `available_tool_names`（可升级段只列满足此条的工具，但校验仍必须在端口做——清单可被模型猜名绕过）；
  2. **层级放行**：工具 `tier <= 本轮 maxToolTier`（基工具能进本轮即已放行；本条只为防将来基/超不同层）。
- `disabled_tools` **不是**准入判据：伴随 id 在 `enabled` / `disabled` 两个名单里一律忽略（§3.6），用户的唯一控制点是基工具——基工具被禁用后自然不在本轮，第 1 条即失败。为伴随 id 单设一条否决会让「面板无开关」与「配置能关」并存，形成第二个事实来源。
- 通过者由**端口**解析成 `ToolSpec`（`description()` + `input_schema()`，只有适配器持有注册表）并登记「本轮待追加」；循环在**本次工具批次执行完后**取出已登记项、**尾部追加**到 `state.request.options.tools`，**下一迭代**模型才能调用（§3.8）。响应文案必须写明「已在本轮启用，下一次工具调用起生效」，不得说「现在就能调用」。
- `available_tool_names` 下一迭代自动扩展：适配器从 `ToolExecCtx.tools`（循环已更新为合并集）重新映射，无需回写，否则下一迭代的清单与 `read` 的 `[hint]` 会看不到刚启用的工具。

**输出**：仅 assistant 文本（不落盘，输出上限远小于 `PER_TOOL_LIMIT_CHARS`）。

- 清单行：`- {name}`（按 `id` 字典序，与 `tool_definitions_for_tier` 的排序一致）。
- 手册块：`## {name}` + 手册正文（每工具 ≤ `MANUAL_MAX_CHARS = 2000`，单份超限时正文尾部追加 `(manual truncated)`），总计 ≤ `MANUAL_TOTAL_CHARS = 8000`；总量超限时在末尾追加 `[catalog: showing {k} of {n}]`（footer 标记 `catalog`，§3.4）。
- `data`：`{ "available": number, "listed": string[], "manuals": number, "source": "turn"|"registry", "escalatable": string[], "enabled": string[], "rejected": string[] }`。`enabled` / `rejected` 只在 `detail = "enable"` 时非空；`escalatable` 只在 `source = "turn"` 且非空时下发。

**可升级段（`escalatable`，§3.6 的发现路径）**：

- 仅当 `available_tool_names` 存在（`source = "turn"`）时计算：`escalatable = 注册表中 audience.agent 且 tier <= 本轮 maxTier、**有 `companion_of` 且基工具在本轮可用**、**不在本轮可用名** 的工具`（`disabled_tools` 不参与——伴随 id 在两个名单里一律忽略，§3.6）；为空则不输出该段（回退模式下不计算，否则会与「回退清单本身含未启用工具」重复）。
- 形态：段首一句 `These tools are not enabled this turn. You may call request_tool with detail = "enable" and tools = [...] to enable them for the next iteration; their base tool is already available.`，随后每行 `- {name}: {brief 档}`（**取 `Tool::brief()`**，默认回落到 `description()` 首句、截断到 120 字符；**不**附完整手册）。
- **`brief` 档的边界**：可升级段只给 `name` + 一句用途 + 申请方式，**不含 `input_schema`、不含手册、不含调用约束**。这就是「描述下发」与「可调用」分离的具体形态（§3.6 三档）：模型据此能判断「该不该申请」，但不能据此调用。
- **`brief` 是独立来源而不是 `description` 的前缀**：`full` 档的读者已经拿到 schema、正要调用，需要的是「怎么用不出错」（会失败的约束、互操作事实）；把「什么时候该申请我 / 默认用别的工具」写在那里，在调用时变成反向指令。两者挤在一段文本里只会互相稀释，因此 `Tool::brief()` 单独声明（默认 `None` → 回落 `description()` 首句），超级工具各自覆写。`description()` 也不再承担选择指引（G19 已按档位拆分）。
- **互斥不变式**：`listed ⊆ callable`、`escalatable ⊆ eligible ∧ ¬callable`、`listed ∩ escalatable = ∅`。`data.listed` 与 `data.escalatable` 若出现同名项，模型会把不可调用的名字当可调用（G23）。
- 基工具不在本轮时该条**整段不出现**，而不是「列出但标注不可用」：模型不该知道一个连申请资格都没有的工具存在。
- `detail = "manual"` 的 `query` **不**命中可升级工具——手册预算只给本轮真能调用的工具，可升级项只给一句用途。

**硬约束**：`request_tool` **不得**修改 `TurnRunCtx.tools` / provider 请求 / 注册表。`detail = "list"` / `"manual"` 完全只读；`detail = "enable"` 只能经升级端口登记，由循环统一追加。**列出不等于可用、摘要不等于可调用**：可升级段是申请入口，不是注入通道（§12 R7）。

### 3.2 `super_read`

**身份**：`id = "super_read"`，`tier = Os`，`audience = BOTH`，`read_only: true`，审批键与 `read` 同构（工作区 → `read`；输出域 → `readToolOutput`，`resource` 取命名空间目录）。

**输入**（与 `read` 的四项对齐 + 三项）：

| 键 | 类型 | 默认 | 取值域 | 语义 |
|---|---|---|---|---|
| `path` | string | — | — | 同 `read`（`path::resolve_readable`） |
| `offset` | integer | `1` | `>= 1` | 起始行（1-based，`0` 视同 `1`） |
| `limit` | integer | `2000` | `1..=20000` | 行数上限 |
| `encoding` | string | 无 | — | 同 `read` |
| `unit` | `"char"` \| `"byte"` | `"char"` | — | `column` 的计量单位 |
| `column` | integer | `1` | `1..=MAX_COLUMN` | 行内起始位置（1-based） |
| `maxLineChars` | integer | `2000` | `200..=20000` | 每行可见宽度 |

**语义**：

- 首次调用建索引（`[index: built in {ms} ms · {lines} lines · {bytes} bytes]`），之后同 `revision` 复用（`[index: reused]`）。
- `totalLines` **恒精确**（索引就是行数的事实来源），`data.totalLines` 不再出现 `null`。
- `unit = "byte"` 时 `column` 是**行内容起始**（BOM 已剥离）的字节偏移，1-based；`unit = "char"` 时是解码字符。
- `unit = "char"` 且 `column > MAX_COLUMN_CHARS = 200_000` → 业务错误（F6），提示改用 `unit = "byte"`。这是 CPU 上界：单行解码字符数被 20 万封顶。
- 渲染由 B 自持（`super_read/render.rs`），**不复用** A 的 `LineSink`；但输出**格式**必须与 `read` 逐字节一致（G12）——这是互操作要求，不是实现复用。footer 顺序 = `[long-lines]` → `[column]` → `[index]` → `[encoding]`。
- 扫描也由 B 自持：可以按索引 `seek` 到行起点再读单行，不必像 A 那样从头线性扫描；`MAX_LINE_CHARS` 的默认宽度与切行判据对齐 A（§0.3），但 B 的 `maxLineChars` 可调。
- 回执：写 `ReadReceipt`（`truncated_lines` / `line_window_max_chars` 同 A 口径，I57 §3.5），**不新增字段**。

**`data`**：A 的八项全沿用（`path` / `startLine` / `endLine` / `totalLines` / `truncated` / `truncatedLines` / `lineWindow` / `linesBeyondColumn`），追加一项：

| 键 | 类型 | 语义 |
|---|---|---|
| `index` | object | `{ "state": "built"\|"reused", "buildMs": number, "lines": number, "bytes": number, "revision": string }` |

### 3.3 `super_edit`

**身份**：`id = "super_edit"`，`tier = Os`，`audience = BOTH`，`read_only: false`、`destructive: true`（审批走 `edit` 的既有键）。

**输入**（单区间替换）：

| 键 | 类型 | 默认 | 语义 |
|---|---|---|---|
| `path` | string | — | 同 `edit` |
| `startLine` | integer | — | 区间起始行（1-based） |
| `startColumn` | integer | `1` | 区间起始列；`unit` 决定口径 |
| `endLine` | integer | `startLine` | 区间结束行（含） |
| `endColumn` | integer | 行尾 | 区间结束列（**不含**，即半开区间） |
| `toEndOfLine` | boolean | `false` | 结束位置取该行行尾；与 `endColumn` **互斥**（F9） |
| `unit` | `"char"` \| `"byte"` | `"char"` | 列口径；`byte` 时必须落在字符边界（F5） |
| `expected` | string | 见下 | 被替换区间的现有文本 |
| `replacement` | string | `""` | 替换文本；空串即删除 |

**`expected` 必填规则**（消除「静默替换」的核心）：

- 区间**非空**（即会删除或覆盖既有文本）→ `expected` **必填**，且必须与现有区间文本**逐字节相等**，否则 F7 / F8。
- 区间为空（纯插入：`startLine == endLine && startColumn == endColumn` 或 `toEndOfLine` 未给出且 `endColumn == startColumn`）且 `replacement` 非空 → `expected` 可省略。

**前提**（与 `super_read` 的关系）：

- 唯一硬前提是**索引层可用**：`ToolUseContext.index_root` 已注入。缺失即 fail-closed，不降级为线性扫描（与 `super_read` 同一口径）。
- **不要求先调用过 `super_read`**：行起点表是惰性资源，首次 `super_edit` 自行建索引（同一 `(path, revision)` 缓存与单飞，§4）。
- **不要求存在读取回执**：`edit` 的「本会话读过 + revision 一致」闸门不适用于本工具——本工具没有子串匹配，`expected` 必须等于磁盘上此刻该区间的字节，盲写风险已由判据本身封死（F7 / F8 / F10）。因此 `super_edit` 不查 `ReadReceipt`，只在写成功后**写**回执。

**语义**（索引驱动 + 流式写，**不整文件驻留**）：

- 定位**只按行地址**：索引给行起点 `offsets[i]`，列由该行内容解码后推进，**不做**子串匹配（§9 L3）。最终产出的是一个**字节区间** `[start, end)`，不是整文件文本。
- **两端自证**（与 `super_read` 同一判据，§4）：`start` 非首行时前移一个行尾符长度并断言前缀等于行尾符；`end` 必须后跟行尾符、或恰好等于文件总字节（末行）。任一不成立 → `StaleIndex` → 重建索引后重试一次。
- **只解码目标区间**：`expected` 与现区间文本的比对在 `[start, end)` 内完成，内存 O(区间)；列推进只解码该行起点到 `endColumn`，受 `MAX_COLUMN_CHARS = 200_000` 上界约束（与 `super_read` 同，F6）。
- **区间外字节零改动**：写回 = `head[0..start)` + `replacement` + `tail[end..EOF)` 顺序写；BOM 天然留在 head 里，编码只作用于 `replacement`。因此没有 `MAX_WHOLE_FILE_BYTES` 整文件闸门——`super_read` 能读的文件 `super_edit` 就能改。
- **闸门按区间计**：`MAX_EDIT_RANGE_BYTES = 16 MiB`（`end - start`，F11）；`MAX_TRANSCODE_BYTES = 1 MiB` 只约束 `replacement` 的转码字节数，不再约束整个文件。这是相对「沿用 `agent_edit/apply.rs` 整文件读」的关键差别：2 MiB 的 GBK 文件里改 5 个字符是常规路径，而不是被 1 MiB 转码闸门拒绝。
- **行尾风格唯一确定**：`super_index::format::line_break(encoding).pattern()` 给出该编码的行尾字节序列，自证已断言它就是文件实际的行尾符；`replacement` 内的 `\n` 按该 pattern 展开（与 `edit` 的「CRLF 文件保持 CRLF」一致）。
- **`expected` 比较口径**：区间文本与 `expected` 都先做 `\r\n` → `\n` 归一再逐字节比较（与 `edit` 同），模型不必在 `expected` 里写 `\r\n`。
- **原子性**：temp file + `sync_all` + rename 不变，只是内容改为三段顺序写，不整份驻留。
- **漂移防护**：写前再比一次 `file_revision`，与读取时不同即 F10（提示重新 `super_read`）。
- 写成功后写回执（`truncated_lines = 0`；`line_window_max_chars` **沿用既有回执值**，本会话无回执时用 `read` 的默认宽度 `DEFAULT_LINE_WINDOW_MAX_CHARS`——本工具入参里没有「读取宽度」，不能写「沿用本次读取宽度」），并调 `ReadReceiptStore::invalidate_path` 与 `super_index::cache::invalidate_path`（B→B，纯缓存卫生；正确性另有行起点自证兜底，§4）。
- 不修改 `edit` 的既有行为与 schema。

**`data`**：`{ "path", "startLine", "endLine", "replacedChars", "bytesWritten", "revision" }`。`bytesWritten` 为写回后的文件总字节数（head + replacement + tail）。

### 3.4 footer 登记表：单一来源（`os/shared/footer.rs`）

footer 是 `[<tag>: <detail>]` 独占一行的尾注，由工具在正文之后追加，**不是文件内容**。它有三个使用方，必须同源：

| 角色 | 使用方 | 动作 |
|---|---|---|
| 生成 | `read` / `super_read` / `grep` / `list` / `glob` / `request_tool` / `output`（采集侧裁切） | 调 `footer::render(tag, detail)`，`tag` 只能取自常量 |
| 登记 | `footer::TAGS` | 新增 tag 只改这一处 |
| 识别 | `agent_edit/report.rs` 的 0 匹配诊断 | 调 `footer::trailing_marker()` 判断「`old_string` 把 footer 当成了文件内容」 |

登记表（`footer::TAGS` 与常量名）：

| 常量 | tag | 出现条件 | 形态 | 归属 |
|---|---|---|---|---|
| `ENCODING` | `encoding` | 非 UTF-8 无 BOM 时 | `[encoding: {name}]` / `[encoding: {name} ({source})]` | P16 |
| `NON_UTF8` | `non-utf8` | 有含非法字节的文件 | `[non-utf8: {n} file(s)]` | P16 |
| `CATALOG` | `catalog` | 手册总量超限 | `[catalog: showing {k} of {n}]` | P17 |
| `LONG_LINES` | `long-lines` | 本次输出有被切行 | `[long-lines: {n} lines cut at {width} chars]` | P16 |
| `INDEX` | `index` | `super_read` 恒出 | `[index: built in {ms} ms · {lines} lines · {bytes} bytes]` / `[index: reused]` | P17 |
| `COLUMN` | `column` | `column > 1` | `[column: {unit} {n}]` | P17 |
| `SKIPPED` | `skipped` | 有不可读文件被跳过 | `[skipped: {n} file(s)]` | P16 |
| `LOSSY` | `lossy` | 有损解码的行 | `[lossy: {n} line(s)]` | P16 |
| `SEARCH` | `search` | 搜索撞时间预算 | `[search: stopped at the {ms}ms time budget; …]` | P16 |
| `TRUNCATED` | `truncated` | 输出窗口被预算截断 | `[truncated: …]`（三态口径见 I46 compact §2.2 D2） | P16 |
| `HINT` | `hint` | `read` / `edit` 确实表达不了，且 `request_tool` 本轮可用（即可申请） | `[hint: …]`（§3.6） | P17 |

- **为什么必须同源**：原先生产方各写字符串字面量、`edit` 另存一份白名单，`super_read` 新增的 `index` / `column` 只加在生产方，消费方静默漏判——两侧一致只能靠单测事后发现。收敛后，生产方漏登记直接编译不过（tag 只能来自常量），识别方读同一份 `TAGS`。
- 形态必须是 `[<tag>: …]`：`trailing_marker` 只认方括号 + 冒号前缀，圆括号形态（`(catalog truncated: …)`）无法被识别，等价于未登记。
- `render` 与 `trailing_marker` 互为逆运算，由 `every_registered_tag_round_trips` 单测覆盖；`tags_are_unique` 守住「不得两处维护」。
- `[search]` 由原先的无 tag 形态 `[search stopped at the …]` 收敛为登记形态 `[search: …]`；`parse_truncation` 的前缀也取自 `footer::TRUNCATED`，生产方改 tag 时不会静默失配。

### 3.5 索引文件格式（`super_index/format.rs`）

| 段 | 内容 |
|---|---|
| magic | `AIDX1`（5 字节） |
| header | `lineCount: u64`、`fileBytes: u64`、`revisionLen: u32` + `revision`（`len:nanos`，N9） |
| offsets | `lineCount + 1` 个 `u64` |

`offsets` 定义（**BOM 已剥离**，与 A 的字符口径对齐）：

- `offsets[0]` = 首行内容起始的原始字节偏移（= BOM 长度）；
- `offsets[i]` = 第 `i+1` 行内容起始（`1 <= i <= lineCount`，末项 = 文件总字节）；
- 行数定义与 A 一致：剥离 BOM 后 `\n` 个数 + 末尾残留非空 +1；BOM-only 文件 = 0 行。

**行起点自证**：`offsets[i]` 的定义是「紧跟在行尾符之后」，因此 `offsets[i] - pattern.len()` 处必定是行尾符本身（`i = 0` 例外——首行前面是 BOM 或文件头，不做断言；空行同样成立，因为 `offsets[i] - pattern.len()` 就是前一行的行尾）。读取时据此断言前缀，不等即判 `StaleIndex`（§4）。

**扩展点（B 自持，不受 A 约束）**——索引格式是 B 的私有产物，A 不消费它，因此可以比 A 复杂：

| 可选段 | 用途 | 约束 |
|---|---|---|
| `crc32`（每 N 行一个校验值） | 检测落盘文件被截断 / 篡改，减少「重建一次」的盲目性 | 无；校验失败并入 F4 |
| 压缩（如 `offsets` 差分编码） | 把 8 字节/行压到 1–2 字节/行，抬升 `MAX_INDEX_FILE_BYTES` 覆盖行数 | 需自写编码器；读取仍需 O(单行) |
| `maxLineBytes` / 前 K 行字节采样 | 建索引时预判容量、给 F1 文案更准的实测值 | 只是采样，不改变 offsets 语义 |
| 列提示（每行首个制表位 / 标记行号位图） | 让 `column` 推进少扫一点 | 仍不得替代 offsets（§9 L1 的边界保留） |

**版本规则**：任何段布局变更必须 bump magic（`AIDX1` → `AIDX2`）；旧版本文件按「损坏」路径处理（删除重建一次），不得尝试兼容解析。

**读取**：按 `[offset, offset+limit]` 只读**所需切片**（`offsets[start..=start+limit]`），不整份载入；每行从 `offsets[i] - pattern.len()`（即行尾符起点）`seek` 起读，读到 `offsets[i+1]`——先断言前缀等于行尾符（行起点自证，§4），再剥掉它解码，内存 O(单行)。

**目录与命名**：

- 目录 `AppDirs::indexes()`（`app_dirs/roots.rs` 新增，`dir_name.rs` 新增子目录名 `tool-index`），与 `tool_results` 同级；
- 文件名 `{sha2(绝对路径)}_{revision}.idx`（`revision` 中的 `:` 替换为 `_`）。**不用 blake3**：workspace 未引入该依赖，`sha2 = "0.10"` 已在册（见 T2 的依赖核对条目）。
- 索引目录**不在工作区**，读写走内部路径，不经 `path::resolve_readable`；但工具入参 `path` 仍必须过 `resolve_readable`。

### 3.6 注入策略：伴随准入 + 模型申请（超级工具不勾选，元工具需勾选）

**决策（2026-11-19，取代「伴随展开」与「超级工具用户勾选」两版）**：超级工具**不直接注入本轮工具集**；它们与基工具的关系是**准入**（`companion_of`）而不是注入——基工具在本轮 → 超级工具获得「可被申请」的资格；模型判断能力不足 → 经 `request_tool(detail = "enable")` 申请 → 循环在下一迭代尾部追加。用户不需要、也没有针对**超级工具**的勾选项。

**决策补充（2026-11-19 修订）**：`request_tool`（元工具）**改为普通可勾选条目**——勾上才注入，取消勾选或列入 `disabledTools` 即不注入。理由是面板一致性：清单里的每一项都由用户勾选决定是否进入本轮，元工具若例外，面板就会出现「没勾却生效」的条目，与 deny-by-default 的整体语义冲突（`enabledTools` 为空时本轮工具集必须为空）。代价是升级能力变成一个可关闭的开关：默认不勾时模型无从申请——这是显式关闭，**不**由任何默认值代劳（见 §3.6 末尾「默认值」）。

**术语（全文档与面板文案统一口径）**：**可用** = 准入（`eligible`，跟随基工具）；**启用 / 注入** = 进入本轮 tools 数组（`callable`）；**摘要** = `brief` 档描述（一句用途 + 申请方式）。三者不得互相替换使用——面板说「可用」、`request_tool` 说「启用」、模型看到的是「摘要」，各指一件事。元工具的「勾选」是普通工具的「注入开关」，不属于上述三档语义。

**三个正交量，不可混用**：

| 量 | 取值 | 判据 | 载体 | 消费者 |
|---|---|---|---|---|
| **准入 eligible** | 是 / 否 | `companion_of()` 的基工具 id ∈ 本轮工具集（无 `companion_of` 的工具恒为「是」） | `ToolUseContext.available_tool_names`（唯一事实） | 可升级段、升级端口校验 |
| **描述档位 described** | `none` / `brief` / `full` | `none` ⇔ ¬准入；`brief` ⇔ 准入 ∧ ¬可调用；`full` ⇔ 可调用 | `brief`：`Tool::brief()`（默认回落 `description()` 首句）→ `request_tool` 可升级段 + 基工具 `[hint]`；`full`：provider tools 数组（`description()` + `input_schema()`） | 模型 |
| **可调用 callable** | 是 / 否 | 准入 ∧ 模型申请（`disabled_tools` 不参与：伴随 id 在两个名单里一律忽略，§3.6） | 循环持有的本轮工具集 | provider 请求、`ToolExecCtx.tools` |

**不变式**（G23 逐条断言）：

- `full ⇔ callable`：可调用必有完整描述与 schema；完整描述也只随可调用下发（没有 schema 就调不了，反之完整描述常驻就是本方案要省的 ≈ 3.9 KB）。
- `brief ⇒ eligible`：不可申请的工具连一句摘要都不占预算。
- `none ⇔ ¬eligible`：基工具不在本轮时，清单、摘要、注入三处同时消失。
- 因此**准入定描述下限、申请定描述上限**：默认「跟随基工具可用」（停在 `brief`），只有真要调用时才升到 `full`（额外注入）。

**为什么是三档而不是两态**：两态（在 / 不在）会逼出二选一——要么常驻 ≈ 3.9 KB，要么模型根本不知道该申请什么。三档把「知道它存在、知道怎么申请」（`brief`）与「能调用它」（`full`）变成两个独立决定：前者由准入决定，后者由模型申请决定。这就是「**发送给模型的工具提示词**」与「**工具是否可用**」分离的落点——提示词分档下发，可用性单独判定；面板显示的「可用」只映射 `eligible`，与 `callable` 无关。

三条理由：

1. **「伴随」不该等于「注入」**：`read` / `edit` 是零成本默认路径，勾上它们表达的是「我要读/改文件」，不代表「我要行内定位与精确总行数」。把超级工具一并注入等于替模型做了这个判断，且用户没有任何关闭途径（原实现的实际缺口）。
2. **没有基工具就不该存在**：超级工具不是独立能力。基工具不在本轮时，模型既不该看到它、也不该能申请它——准入判据把这条约束落到唯一一处（`available_tool_names`），清单、摘要、注入三者同源。
3. **默认跟随、按需升档**：不申请就永远停在 `brief`（一句用途 + 申请方式），省下 ≈ 3.9 KB 常驻描述与两个语义相邻条目的选择噪声；代价是升级轮付一次全量 prefill（§3.7 的定量分析）。

**机制表**：

| 机制 | 现状（T1–T5） | 目标（T7 / T8） | 说明 |
|---|---|---|---|
| `Tool::companion_of()` | `Some("read")` / `Some("edit")` | **保持**，语义改为「准入基工具」 | 默认 `None` 不变；返回基工具内部 id |
| `ToolDescriptor.companion_of` | 已下发 | **保持** | 字段在生成的 DTO 契约里（`protocol.contract.ts`），删它要改 wire 并跑 `contract:gen`（违反 G15）；语义由「自动注入」改为「跟随准入」 |
| `ToolRegistry::expand_enabled_tools()` | 被 `resolve_available_tools` 调用（加法） | **改为 `exclude_companions()`（身份判据，不是关系判据）** | 配置路径上**一律排除** `companion_of != None` 的工具，不论基工具是否在白名单。展开（加法）是「勾基工具 = 注入」的来源，本次移除；「基工具在不在」也不再被查询——配置域内根本不表达 `callable` |
| `request_tool` 注入 | 需用户勾选 | **保持需用户勾选**：勾上才注入，`disabledTools` 可否决（普通条目，无特殊待遇） | 元工具是申请入口，但入口本身由用户开关；面板给它「申请入口」标记而不是特殊注入路径 |
| 升级通道 | 无 | `ToolUseContext.escalation`（`Arc<dyn ToolEscalationPort>`，`#[serde(skip)]`） | 工具只登记申请，不触碰请求（§3.1） |
| 描述下发 | `description()` 只随 tools 数组下发 | **三档**：`none` / `brief`（`Tool::brief()` → 可升级段 + 基工具 `[hint]`）/ `full`（tools 数组的 `description()`） | 与可调用解耦：`brief` 下发而不可调用是合法态（G23）；两档由**两个方法**供料，不是同一段文本的两截 |

**白名单与伴随条目（配置路径：身份判据，不看关系）**：

配置路径只回答一个问题——「这个工具是否有权进入白名单」。伴随工具没有独立身份（它是基工具的扩展能力），因此：

- 默认（面板不写伴随 id）：`enabledTools` 里不含 `super_read` → 本轮只有 `read`；`super_read` 仅「准入」（`brief`），模型可申请。
- `enabledTools` 里出现任何 `companion_of != None` 的 id → **一律排除**，不注入。这与基工具是否也在名单里**无关**：两者都在时同样排除（显式配置不构成第二个 `callable` 来源）。
- `disabledTools` 里的伴随 id → **同样忽略**（既不阻断也不生效）。用户的唯一控制点是基工具；面板不写这两个名单，残留项属脏数据，不为脏数据定义第三种行为。
- 因此白名单处理是**纯身份过滤**：`available = {id | id ∈ enabledTools, companion_of(id) = None}`，不需要任何依赖查询，也不存在「禁用基工具而伴随残留」的顺序问题（`disabled` 对基工具生效时，伴随本来就从未进入候选集）。
- 历史配置里同时残留 `read` 与 `super_read` → 只有 `read` 注入，行为与「从未勾选过 `super_read`」完全一致；不做一次性迁移，由前端在用户下一次写回时静默剔除（§3.6 残留清理）。
- **元工具没有身份特殊性**：`request_tool` 的 `companion_of = None`，因此它按普通条目走同一条过滤链——勾上则注入，未勾则不注入，列入 `disabledTools` 则被否决。它不参与「伴随 id 一律忽略」的规则（那条只针对 `companion_of != None` 的 id）。
- **未勾元工具的后果**：本轮没有申请入口，`read` / `edit` 的 `[hint]` 门控（「`request_tool` 在本轮」）不成立，提示不出现；`super_*` 停在 `brief` 也无人能升档。这是用户可接受的显式关闭，不是缺陷。

**前端：伴随展示（既不是勾选，也不是隐藏）**

`ToolConfigPanel` 把 `companionOf != null` 的条目渲染成**与基工具同排的从属条目**（同一渲染单元、无独立开关），而不是与基工具并列的可勾选项：

| 项 | 规则 |
|---|---|
| 位置 | 同一 tier 分组内，与基工具**同处一个不可断行的渲染单元**（`groupToolUnits` 把伴随条目并入基工具单元，`.toolUnit` 为 `inline-flex`）。不缩进、不另起一行、不加符号前缀：横向换行流里一旦分开，伴随条目会被看成无关的独立工具。基工具不在本 tier 时该伴随条目自成一个单元（与 `groupToolsByTier` 的并列规则一致） |
| 勾选 | **无勾选框**（也不加状态点），不写 `enabledTools`，也不写 `disabledTools` |
| 状态 | 只靠**字色弱化**（`--color-text-tertiary`，对比普通工具的 `--color-text-secondary`）区分「不是独立能力」；基工具已勾 → 常色；基工具未勾 → 整条 `opacity: 0.55` |
| 文案 | 用途与从属状态全部收进悬停提示（`chat.tool.panel.companion_of` / `companion_blocked`），条目本身只占一个工具名的宽度 |
| 计数 | 分子 `countEnabledSelectableTools`、分母 `countSelectableTools` **都只数可勾选条目**；伴随项两侧都不出现（它永远没有独立开关，计入分母会让「已启用 / 全部」看起来永远差几个） |
| 残留清理 | 配置里残留的伴随 id **不提示、不迁移**：`stripCompanionIds` 在每次写回前剔除（面板 `useToolRegionState.applyChange`、`onSetDefault`，设置页 `emit` 与 `useModeForm.handleSave`），用户下一次保存即静默修掉。后端同一事实仍打一条 `log::warn`（每 id 每进程一次，`warn_ignored_companion`） |
| 提示 | 悬停提示复用 `TOOL_DESC_KEYS`；未收录时用「一句话用途 + 模型按需启用」，**不得**直接展示 `description()` 全文（那是给模型的 `full` 档） |
| 措辞 | 禁止「已启用 / 关闭 / 开」等暗示独立开关的词；面板只表达 `eligible`，不表达 `callable`（可调用是轮内瞬态，配置面不呈现） |

**残留 id 的边界**：`stripCompanionIds` 只剔除**能识别为伴随**的 id（`companionOf != null`），**不**剔除清单里查不到的未知 id——工具可能因 MCP 断开、特性开关临时缺席，静默删除会丢掉用户的勾选，恢复后也不会自己回来。未知 id 只在计数两侧被忽略（分子按清单过滤），不改写配置。

理由：面板是**配置面**，唯一真值是「基工具是否在白名单」。把伴随条目做成可勾选项会造出第二个事实来源——勾上后被 `exclude_companions` 一律排除（勾了也不生效），基工具勾上后又变成「由用户决定 `callable`」，而 `callable` 本该由模型按需申请（§3.6 三档），勾选动作没有可解释的语义；直接隐藏则让用户看不出「超级工具跟着 `read` 走」，与后端的准入语义脱节。伴随展示是唯一能同时说清「它属于谁」「谁决定它」的形态。

**前端：元工具展示（可勾选 + 标记）**

`request_tool` 与普通工具一样是可勾选项，只在视觉上区分它是「申请入口」而非普通能力：

| 项 | 规则 |
|---|---|
| 位置 | 同 tier 分组内、排在该组**末尾**（`groupToolsByTier` 先普通工具、再元工具、最后伴随条目） |
| 勾选 | 有勾选框，写 `enabledTools`；与普通工具共享 `disabledTools` 否决语义 |
| 标记 | 名称后缀一个「申请入口」标签（键 `chat.tool.panel.meta_badge`），悬停提示用 `TOOL_DESC_KEYS.request_tool` |
| 计数 | 计入 `enabled_count`（它是真正的注入项） |
| 措辞 | 设置页说明用「勾选后模型可据此申请启用超级工具；取消则模型无法升级」（键 `...tool_assembly.meta_hint`），明确它控制的是升级能力而非某个具体能力 |

**默认值（2026-11-19 修订，取代「模式预设应含 `request_tool`」）**：元工具**不进入任何默认值**——`ConversationToolConfigDto::default()` 的 `enabledTools` 为空，内置模式的种子（`BUILTIN_AGENT_TOOL_CONFIG`）同样保持空白名单。默认注入被否掉的三条理由：

| # | 理由 |
|---|---|
| 1 | **deny-by-default 不可破例**：任何默认名单都等于替用户做了一次勾选，而用户看不到这次勾选发生过；随后「取消勾选」只是把配置改回默认态，动作与结果不对称 |
| 2 | **单独注入元工具是半启用状态**：可升级段的判据是「基工具在本轮」（§3.1），空白名单下 `read` / `edit` 都不在本轮，`escalatable` 恒为空——模型只拿到一份清单，升级通道完全无效。要真正可用，默认值必须**同时**含基工具与元工具，那就不是「补一个入口」，而是替用户装配整套工具 |
| 3 | **默认值只在一个来源**：工具装配的初始值由模式种子唯一定义（协议层只有契约缺省，app 层不设默认），不存在「种子 + 特例」的第二来源 |

因此元工具的可用性**完全由用户勾选决定**：新建会话默认没有申请入口，`read` / `edit` 的 `[hint]` 门控不成立（不出现指向不存在调用方式的提示）；用户勾了基工具而没勾元工具时，`super_*` 停在 `brief` 且无人能升档。这是显式关闭的既定后果（§3.6「未勾元工具的后果」），不是缺陷；面板把元工具渲染成可勾选项并带「申请入口」标记，用户要升级能力时勾上即可。

**发现与申请（唯一路径）**：`request_tool` 的可升级段（§3.1）是唯一发现路径，`detail = "enable"` 是唯一申请路径。两者都只在基工具本轮可用时存在，且只下发 `brief`（一句用途 + 申请方式），**不含 schema**——`full` 只在追加进 tools 数组后才出现。

**审批与层级不变**：升级只让模型「能调用」；`super_edit` 执行时仍走 `edit` 的审批键，`super_read` 仍走 `read` 键。层级上限不被绕过：基工具能进本轮说明 `tier <= maxToolTier`，超级工具与基工具同层（N17），准入即继承该上限。

**普通工具优先**：`read` / `edit` 仍是默认路径；`super_*` 只在普通工具表达不了、且模型申请之后才出现。

**待改项（T7 准入语义 / T8 升级通道）**：

| 待改 | 现状 | 目标 | 切片 |
|---|---|---|---|
| `resolve_available_tools` | 过滤前调 `expand_enabled_tools`（加法） | 改为 `exclude_companions`（身份过滤：`companion_of != None` 的 id 一律不进候选集，`disabled` 中的伴随 id 同样忽略）；**移除元工具特殊注入**，`request_tool` 按普通条目走同一条白名单过滤 | T7 |
| `super_read/mod.rs` / `super_edit/mod.rs` 的 `companion_of` | `Some("read")` / `Some("edit")` | **不变**（语义改为准入） | — |
| `ToolConfigPanel` | 按 tier 渲染全部条目，伴随条目也是可勾选项 | 伴随条目与基工具同处一个不可断行单元（无勾选框、无状态点、字色弱化、文案进 tooltip、计数不计），写回前 `stripCompanionIds` 静默剔除残留 id；**元工具改为可勾选项**（排组内末尾、带「申请入口」标签、计入 `enabled_count`） | T7 |
| `read` 的 `[hint]` | 门控在 `available_tool_names` 含 `super_read` | 门控改为含 `request_tool`，文案指向 `detail = "enable"` | T7 |
| `edit` 的 0 匹配诊断 | 无超级工具提示 | 同门控加一句，指向 `super_edit` | T7 |
| 两条 `description()` / `brief()` | ≈ 1.1 KB / ≈ 0.83 KB，选择指引与调用说明混在一段 | 拆成两个方法：`description()` 只写「怎么用不出错」（≤ 600 字符，补回会失败的约束），`brief()` 只写「一句用途 + 何时该申请」（≤ 120 字符）；`Tool` trait 增 `brief()` 默认实现（`None` → 回落 `description()` 首句） | T7 |
| `request_tool` | 无 `enable`、无可升级段 | 按 §3.1 增 `detail` / `tools` / `escalatable` / `rejected`；`enable` 的 `ok` 语义见 §3.8 ⑤ | T8 |
| `assistant-tools/contract/` | 无升级契约 | **新增 `escalation.rs`**：`ToolEscalationPort`（`candidates` / `request`）+ `EscalationCandidate` / `EscalationRejectReason` / `EscalationOutcome`（§3.8 ①） | T8 |
| `ToolUseContext` | 无升级端口 | 增 `escalation: Option<Arc<dyn ToolEscalationPort>>`（`#[serde(skip)]`） | T8 |
| `assistant-agent` 循环契约 | 无 | **新增 `EscalationSink`**（`push` / `drain`，只消费成品 `ToolSpec`）+ `ToolExecCtx.escalations: Option<Arc<dyn EscalationSink>>`（§3.8 ②） | T8 |
| `adapters/tool_exec.rs` | 每次调用重建 `ToolUseContext` | 现构造端口并注入：`registry` + `MaxToolTierSource` + 本轮 `sink`（§3.8 ③） | T8 |
| 循环 | 每迭代复用同一份 `state.request` | `RoundState.tools: Arc<Vec<ToolSpec>>`，批次后尾部追加，下一迭代生效（§3.8 ④） | T8 |

**运行时能力提示**（`read` / `edit` 侧，唯一引导点）：

| 触发条件 | 文案 |
|---|---|
| 本次输出有行被切（`truncatedLines > 0`） | `[hint: long lines were cut; call request_tool with detail="enable" and tools=["super_read"] to read further into a line via `column` this turn]` |
| `totalLines` 为 `null`（扫描撞预算） | `[hint: totalLines is unknown; call request_tool with detail="enable" and tools=["super_read"] to get an exact `totalLines` this turn]` |
| `edit` 的 `old_string` 0 匹配且疑似截断/超长行 | `[hint: if the target line was truncated, call request_tool with detail="enable" and tools=["super_edit"] to replace by line range]` |

- 门控从「`available_tool_names` 含 `super_read`」改为「`available_tool_names` 含 `request_tool`」：升级后 `super_read` 当然在本轮，但提示必须出现得更早——在它还没被启用、模型正需要它的时候。指向一个本轮调不到的申请入口比不提示更糟，因此仍必须门控。
- 提示**不写进** `read` / `super_read` 的 `description`：描述是静态的，此时并不知道会不会撞上限，反向引导会让模型在能一次读完的普通文件上也先绕一圈。用法细节放 `manual()`，按需取。
- 三条提示都在**结果尾部**（footer 形态），不进系统提示；`edit` 那条由 B 在 A 的诊断文件里追加（§7.1 的「B 侧最小触碰」清单）。

### 3.7 注入成本与升级路径

**注入形态（改造后）**：每轮 `send_agent_message` 走一次 `resolve_available_tools` → 写进 `spec.tools`；`request::build` 在 `loop_executor/mod.rs` 只构建一次 provider 请求。升级发生在**轮内**：本批次执行完后，把申请项解析成 `ToolSpec` 追加到本轮工具集，下一迭代生效；`ToolExecCtx.tools` 传「基础 + 已升级」的合并切片。

**注入内容（`full` 档）**：只有 `description()` + `input_schema()`；`manual()` / `brief()` 都不注入（`manual` 只能经 `request_tool` 取，`brief` 只出现在可升级段）。按源码字符数估算（2026-11-19，描述按「怎么用不出错」重写后的实际长度）：

| 工具 | 描述（full） | schema | 合计 | brief |
|---|---|---|---|---|
| `super_read` | ≈ 0.55 KB | ≈ 0.85 KB | ≈ 1.4 KB | ≈ 0.11 KB |
| `super_edit` | ≈ 0.55 KB | ≈ 1.2 KB | ≈ 1.75 KB | ≈ 0.10 KB |
| `request_tool` | ≈ 0.3 KB | ≈ 0.3 KB | ≈ 0.6 KB | —（元工具本身不申请） |

**`brief` 档的成本**：可升级段每行 ≈ 60–120 字符（`name` + 一句用途 + 申请方式），只在 `detail = "list"` / `"manual"` 的**结果**里出现，不常驻工具数组——它花的是**那一次工具调用**的输入 token，而不是每一轮的常驻 token。这是三档分离在成本上的意义：`brief` 不进 tools 数组，因此不参与前缀缓存的计算。

**为什么描述可以放宽到 ≈ 600 字符**：T1 定的 ≤ 400 字符是给「常驻」设的预算。T7 之后 `description()` 只在可调用时下发，省下的字节从来不进常驻预算，继续用它去砍「模型唯一能看到完整说明的那一次」是砍错了对象。放宽后补回的是**会导致调用失败的约束**（`unit = "byte"` 的字符边界、`expected` 必填、空区间 = 纯插入），这些原先只存在于 `manual()`，要多一次往返才取得到。

**不升级的轮次**：只有 `request_tool` ≈ 0.6 KB——且**只在用户勾选它时**（未勾则为 0，模型因而无从申请）。两个超级工具为 0（`brief` 也不在数组里）。

**升级的轮次**：再加被申请项 ≈ 1.4 / 1.75 KB，且只在该轮出现。

**缓存机制（必须写准）**：工具数组位于请求**最前部**（system 之后、消息之前）。在数组**尾部**追加一项，`[system, tools_old]` 这段前缀逐字节不变，因此**到旧数组末尾为止的缓存仍然命中**；但从新增项之后开始，消息序列的 token 位置整体后移，**该轮的消息部分不再命中缓存，需要重新 prefill**。因此准确的成本是：

| 轮次 | 成本 |
|---|---|
| 不升级的轮次 | ≈ 0.6 KB（`request_tool`），超级工具为 0 |
| 升级轮（首次追加） | 消息部分的一次全量 prefill（≈ 会话当前 token 数，按未命中价）+ 新增工具 ≈ 1.6 KB + 一次模型往返（申请本身是一次工具调用） |
| 升级后的后续轮次 | 工具数组稳定，重新进入可缓存前缀，成本回到接近 0 |

**因此收益分两块**：①不升级轮次的常驻注入从 ≈ 3.2 KB 降到 ≈ 0.6 KB；②更重要的**选择噪声**——工具表里不再常驻两个与 `read` / `edit` 语义高度相邻的条目，模型每次挑工具不必多判一次。后者不随缓存消失，正是本方案的主要收益。

**代价的判定**：升级轮的全量 prefill 是实打实的，会话越长越贵。是否划算取决于升级频率与平均会话长度——几乎每轮都升级则不如默认注入；只在少数长行 / 截断场景升级则显著更优。这条留给实测（OQ8），本阶段不调参。

| 路径 | 改动面 | 收益 | 结论 |
|---|---|---|---|
| **A 描述瘦身** | 只改两个 `description()` | 升级轮增量 ≈ 3.9 KB → ≈ 2.85 KB（schema ≈ 2.05 KB 是硬成本，压不动） | **做**（T7） |
| **B 面板伴随展示** | `ToolConfigPanel` 把 `companionOf != null` 渲染为基工具下的从属行（无勾选框、状态随基工具） | 让用户看出「它跟着 `read` 走」，且不产生新状态、不写 `enabledTools` | **做**（T7，后端零改动） |
| **C 模型申请 + 尾部追加（本方案）** | `request_tool(detail = "enable")` + 升级端口 + 循环追加 | 不升级轮零常驻、零噪声；升级轮付一次 prefill | **做**（T8，§3.6） |
| D 会话级升级状态（跨轮记忆） | 落库 + DTO | 免去重复申请 | **不做**（R15，破 G15） |
| E 链式伴随（`super_edit` → `super_read`） | 改准入关系为传递闭包 | 少一次申请 | **不做**（R16） |
| F 信号驱动自动升级（基工具在结果里填 `escalation`、循环直接注入） | `ToolResult` 增内部字段 + 循环判定 | 省一次模型往返 | **不做（本阶段）**（OQ9：与「模型主动请求」的产品语义不一致） |

**为什么不在工具内部直接改工具集**：`request_tool` 拿到的 `ToolUseContext` 是不可变借用，它也不该知道 provider 请求的形状。工具只登记申请、循环统一追加，「谁能改工具集」只有一处，`G4` 才可断言（§12 R7）。

**理论最优解但不采用**：把 `column` / `unit` 做成 `read` 的可选参数、把行地址做成 `edit` 的可选参数，两个工具定义可全部消失。它同时违反 §2「不替换 `read` / `edit` 默认路径」与 §12 R1，且让 99% 的普通调用多背两个字段（R17）。列在此仅为成本对照。

### 3.8 升级通道接口契约（T8）

**约束来源**：依赖方向决定「谁定义什么」——`assistant-tools` 不依赖 `assistant-providers`（工具侧契约里不能出现 `ToolSpec`）；`assistant-agent` 不依赖 `assistant-tools`（循环侧不能出现 `ToolEscalationPort`，也拿不到注册表）。两侧只能由适配器（`src-tauri/src/adapters/tool_escalation.rs`）接合，契约因此分三份，缺一份就得让某个 crate 越界依赖（§12 R20）。

**① 工具侧契约（`assistant-tools/src/contract/escalation.rs`）**

```rust
pub struct EscalationCandidate {
    pub id: String,
    pub name: String,
    pub brief: String,
}

pub enum EscalationRejectReason {
    /// 基工具不在本轮可用集
    BaseNotAvailable,
    /// 目标没有 `companion_of`（不是伴随工具，申请无意义）
    NotCompanion,
    /// `tier > 本轮 maxToolTier`
    TierExceeded,
    /// 名字解析不到任何工具
    NotInCatalog,
}

pub struct EscalationRejection {
    pub id: String,
    pub reason: EscalationRejectReason,
}

pub struct EscalationOutcome {
    pub enabled: Vec<String>,
    pub rejected: Vec<EscalationRejection>,
}

#[async_trait]
pub trait ToolEscalationPort: Send + Sync {
    /// 可升级段（§3.1）：注册表中 `audience.agent`、`tier <= 本轮上限`、
    /// 有 `companion_of` 且基工具在 `available`、且自身不在 `available`。
    async fn candidates(&self, available: &[String]) -> Vec<EscalationCandidate>;

    /// 逐条校验并把通过项**解析成完整工具声明**后登记到本轮收集器。
    async fn request(&self, requested: &[String], available: &[String]) -> EscalationOutcome;
}
```

- `candidates` 是 `async`：`brief()` 缺省时按 §3.6 回落到 `description()` 首句，而 `description()` 是 async；同步签名会让回落实现不出来。
- **解析归端口**：`id → name + description() + input_schema()` 需要注册表，只有适配器持有。`request` 不返回 `ToolSpec`——工具既看不到也不该看到本轮请求形状（R7）；登记进收集器的是适配器构造的 `ToolSpec`。
- 端口不持有「注册表 + 本轮层级 + 本轮收集器」以外的状态，且不读 `disabled_tools`（§3.1 准入两条）。

**② 循环侧契约（`assistant-agent`，`ToolExecCtx.escalations`）**

```rust
pub trait EscalationSink: Send + Sync {
    fn push(&self, tool: assistant_providers::types::ToolSpec);
    fn drain(&self) -> Vec<assistant_providers::types::ToolSpec>;
}
```

- `ToolExecCtx.escalations: Option<Arc<dyn EscalationSink>>`：循环**每轮**创建一个 `Mutex<Vec<ToolSpec>>` 收集器（`Arc` 包一层），turn 结束即丢弃——跨轮不携带（L17 / G22）。
- 用 `Arc` 而不是借用：`ToolUseContext.escalation` 是 `Option<Arc<dyn ToolEscalationPort>>`（无生命周期参数，端口要随工具调用存活到调用结束），端口内部持有收集器就必须能 `'static` 持有——`&'a dyn EscalationSink` 会把这个生命周期传染到 `ToolUseContext`，而后者是工具签名的一部分。
- 追加时机：**本批次 `ToolExecutionPort::execute` 返回之后**调用 `drain()`；`drain` 为空则**不重建请求**（G4 的「逐字节不变」靠这一点）。
- 追加形态：结果**尾部**追加到本轮工具集，旧前缀逐字节不变；同步刷新 `state.request.options.tools`（provider 请求的唯一来源）。`available_tool_names` 下一迭代由适配器从 `ToolExecCtx.tools` 重新映射，自然含新项（G22），不需要额外回写。
- 追加只在本迭代生效之后的下一次模型调用可见：申请所在迭代的请求已经发出，不追溯（§3.1）。

**③ 适配器（`src-tauri/src/adapters/tool_escalation.rs`）**

- 每次 `execute` 现构造端口并注入 `ToolUseContext.escalation`：`registry: Arc<ToolRegistry>` + 本轮层级 + `sink: Arc<dyn EscalationSink>`（取自 `ToolExecCtx.escalations`）。任一侧缺失（适配器未装配 / 本链路无收集器）→ `escalation = None`，`request_tool(detail = "enable")` 返回业务错误（§3.8 ⑥），**不**静默降级成「申请成功但没生效」。
- **`max_tool_tier` 按 `conversation_id` 每次求值**（`MaxToolTierSource = Arc<dyn Fn(&str) -> ToolTier + Send + Sync>`，与 `WorkspaceRootSource` 同形态）：层级是会话级配置（模式 `maxToolTier`），不塞进 `TurnRunCtx`——循环内核不需要知道配置域（R21）。
- 名字解析：先 `find_by_name` 再回落到 `find`（申请按 `id` 或 `name` 均可），取 `id` 比对；解析不到 → `NotInCatalog`。
- 端口**直接用注册表**（`tier` / `companion_of` / `brief()` / `audience`），不经 `ToolCatalog`：后者只有 `id` / `name` / `manual`，表达不了层级与伴随关系；`ToolCatalog` 的职责不变（回退清单与手册），不为升级通道扩面。
- 两条准入按 §3.1 执行，**`disabled_tools` 不参与**；`rejected` 按申请顺序逐条给出，不去重、不合并。

**④ 循环持有工具集的形态（借用约束）**

- `RoundState.tools: Option<Arc<Vec<ToolSpec>>>`，初值 = `ctx.tools` 的 owned 副本。`execute_tools(&mut state, …)` 调用前先 `let tools = state.tools.clone();`（Arc 克隆），再传 `tools.as_deref()`——避免「`&mut state` 与 `&state.tools` 同时存活」的借用冲突，也不每迭代深拷贝（§3.7 的成本模型要求追加之外的迭代不重建请求，这里是唯一例外）。
- 追加用 `Arc::make_mut` 或重建 `Arc::new(merged)`；两者都必须保持旧元素顺序与内容不变（G4）。

**⑤ `detail = "enable"` 的返回语义（细化 F13 / F14）**

| 结果 | `ok` | `data.enabled` | `data.rejected` | 文案 |
|---|---|---|---|---|
| 全部通过 | `true` | 全部 | `[]` | 「已在本轮启用，下一次工具调用起生效」+ 名单 |
| 部分通过 | `true` | 通过项 | 未通过项 | 先写生效名单与生效时机，再逐条写未生效原因 |
| 全部被拒 / `tools` 缺失 / 名字不存在 | `false` | `[]` | 逐条原因（F13 时为空） | 业务错误 + 可升级段（可能为空） |

- 部分成功返回 `ok = true` 的理由：合法项**已经登记**，`ok = false` 会让模型把「部分成功」当整体失败重试，重试只会重复申请已生效项；`rejected` 已逐条给出原因，模型据此只补申请真正缺的那些。
- 全部被拒时 `ok = false`，与 F14「业务错误」一致；此时 `data.enabled` 为空、工具集不变。

**⑥ 端口缺失（`ToolUseContext.escalation = None`）**

- 非适配器注入的调用点（单测、其它装配路径）没有升级通道：`escalatable` 段**不输出**（不猜、不用 `ToolCatalog` 代算，否则会出现「清单里有、申请不了」的第二种事实来源）；`detail = "enable"` 返回 `ok = false` 业务错误，文案说明「本轮未提供升级通道」。
- 这一分支保证 `list` / `manual` 的既有行为**逐字节不变**（现有测试无需改动），`enable` 只在适配器装配下可达。

---

## §4 索引生命周期

**正确性判据只有一个：行起点自证。** 索引只存行起点、不存任何内容，所以「内容变了」本身从不需要失效——内容永远是本次从磁盘现读的。唯一会失效的是**换行位置**：某个起点若已不再是「紧跟行尾符之后」的位置，按它切出的行就是错的。

**判据只有一份实现**：索引格式 / 缓存 / 单飞 / 自证都在 B 私有公共层 `super_index`（`format.rs` / `cache.rs` / `proof.rs`），`super_read` 与 `super_edit` 都消费它，不各自复制（§7.2 / §12 R14）。这样「读能定位的行」与「写能定位的区间」由同一段代码判定，不存在两份判据漂移。

| 环节 | 规则 |
|---|---|
| 建立 | 首次调用（`super_read` 或 `super_edit`，二者同源）时：`file_revision` → 查内存缓存 → 未命中则单飞建索引（B 自写扫描器，按原始字节找行尾、不解码整行）→ 原子落盘 → 整表读入内存缓存 |
| **自证** | 读第 `i` 行时把起点前移一个行尾符长度再读，**断言读到的前缀等于行尾符**；不等 → `super_index::proof::IndexError::StaleIndex { line }` → 重建索引后重试一次。代价是每行多 1–2 字节与一次比较，syscall 数不变（本来就是每行一次 `seek` + `read`，只是起点前移） |
| 完备性 | 行起点集合完全由换行位置决定：窗口内任一换行移动，对应边界断言必然失败；全部边界通过 ⇒ 窗口内换行位置与建索引时逐一相同 ⇒ 切分正确。这是**判据**而非采样，不依赖概率 |
| 粗筛 | `file_revision = len:nanos` 只用于**尽早**发现变化（键不同即重建），不承担正确性。它没有文件身份（inode / FileId）也没有内容指纹，等长改写落在同一 mtime 时间片（FAT/exFAT 2 s、SMB 1 s、Windows 惰性 mtime）时识别不了 |
| 单飞 | `Mutex<HashMap<Key, Arc<Flight>>>` + `Condvar`；`Key = (indexRoot, path, revision)`。同键并发只建一次，其余等待后取同一结果；leader 失败时把同一个业务错误转给等待者，不重复扫描。成为 leader 后需再查一次缓存（上一个 leader 可能刚写完就退出登记） |
| 容量 | 内存缓存 `INDEX_CACHE_BYTES = 64 MiB`（按 `(lineCount+1)*8` 计），按字节 LRU 淘汰并连带删除被淘汰项的落盘文件；落盘文件同受 `MAX_INDEX_FILE_BYTES = 64 MiB` 单文件闸门 |
| 过大 | 索引文件 > `MAX_INDEX_FILE_BYTES` → F1 fail-closed（不建、不写、不缓存） |
| 损坏 | magic / 行数 / 文件字节不符 → 删除重建一次；仍失败 → F4 |
| 编码回退 | UTF-8 乐观路径遇非法字节 → 采样重猜；若猜到 UTF-16（换行双字节、单字节切行失效）则**按新编码重建并整体替换缓存项**，避免下次调用拿到旧切法的起点表 |

**为什么写侧不做主动通知（§12 R10）。** 曾经的方案是让 `edit` / `write` / `delete` / `move` 在成功后调 `super_index::cache::invalidate_path`。它被放弃，因为：

1. **覆盖面错位**：通知只能看见「经过本工具集」的写入。真正的风险源——编辑器保存、`git checkout`、构建产物覆盖——根本不经过这些代码，通知对它无效；而我们的写工具走 temp file + rename（`persist_atomically`），新文件必然换 mtime，`revision` 自己就能发现，通知是重复劳动。
2. **原则反了**：四个通用写工具与超级工具没有任何语义关系，却要硬编码知道 `super_read` 存在，违反 §0.3 / §7.2 的单向依赖。
3. **机制不可靠**：靠「每处调用点都记得加一行」维持正确性，`move` 覆盖目标时就已经漏了 `target`——这本身就是该机制不可靠的证据。
4. **它不解决它声称要解决的问题**：等长改写 + 同一 mtime 时间片，恰恰是通知覆盖不到的那一格（外部改写不经过我们），所以它不是正确性手段。

自证落地后，`invalidate_path` 只可能是**缓存卫生**（尽早回收内存与落盘文件），不是正确性手段。因此四个写工具不再调用它；只有 B 自己的写工具（`super_edit`）在成功后调用——B→B 自洽，且与回执失效同批。

**残余边界（§9 L11）。** 自证保证「本次渲染出的这段内容」正确，不保证 `totalLines`：等长改写若在**窗口之外**移动了换行，窗口本身仍有效，但总行数已陈旧。要彻底堵住只能全文件重扫，那等于取消索引。

---

## §5 失败矩阵

| 编号 | 情形 | 期望 |
|---|---|---|
| F1 | 索引文件将超过 `MAX_INDEX_FILE_BYTES` | 业务错误，含实测行数/字节；**不得**暗示「换工具就能读」 |
| F2 | 编码不可判定 / 二进制 / 目录 | 与 `read` 同文案（复用 `DecodeError` 与 `binary` 判据） |
| F3 | `path` 越界 / 输出域未授权 | 与 `read` 同（`resolve_readable` + `permission_intents`） |
| F4 | 索引损坏且重建后仍不符 / 行起点自证失败且重建后仍失败 | 业务错误，提示重试；不静默降级为线性扫描 |
| F5 | `unit = "byte"` 的列不落在字符边界 | 业务错误 + 给出最近边界列 |
| F6 | `unit = "char"` 且 `column > 200_000` | 业务错误，提示 `unit = "byte"` |
| F7 | 非空区间缺 `expected` | 业务错误，说明「替换/删除必须给 `expected`」 |
| F8 | `expected` 与现区间文本不符 | 业务错误 + 现区间文本（带行号前缀，行号列宽与分隔符取 `agent_read` 常量，可直接复制回填 `expected`） |
| F9 | `toEndOfLine` 与 `endColumn` 同时给出 / `endLine < startLine` / 列越界 | 业务错误 |
| F10 | 读取后到写入前 `revision` 漂移 | 业务错误，提示重新 `super_read` |
| F11 | 目标区间 `end - start` 超过 `MAX_EDIT_RANGE_BYTES = 16 MiB` | 业务错误，给出实测区间字节数；提示拆成多次调用（**不**提示改用 `edit`） |
| F12 | `replacement` 转码后超过 `MAX_TRANSCODE_BYTES = 1 MiB` | 业务错误，给出实测字节数；提示缩短替换文本 |
| F13 | `detail = "enable"` 缺 `tools`，或名单里的名字**全部**解析不到 | 业务错误（`ok = false`），附可升级段（可能为空）；工具集不变 |
| F14 | 申请项不满足准入（基工具不在本轮 / 没有 `companion_of` / 超出层级 / 本轮可用集未知即回退清单 / 名字解析不到） | 逐条说明原因；**合法项照常登记**。全部被拒 → `ok = false`（业务错误）；部分通过 → `ok = true` + `data.enabled` 与 `data.rejected` 同时下发（§3.8 ⑤）；工具集不变。**`disabled_tools` 不是准入判据**：伴随 id 在两个名单里一律忽略（§3.6），用户唯一控制点是基工具——基工具不在本轮时 F14 自然生效 |

---

## §6 文案

描述只写**是什么**（职责、代价、边界），用法细节一律进 `Tool::manual()`——`description()` 只在工具进入本轮 tools 数组时下发（`full` 档），`manual()` 只在模型主动 `request_tool` 时才付代价。

- `request_tool` 描述必须写明：只返回清单与手册、**默认不改本轮工具集**、`source` 可能是回退清单；`detail = "enable"` 只登记申请、**下一次工具调用起生效**，且基工具必须已在本轮。
- `super_read` 描述必须写明：首次调用有建索引成本、`totalLines` 恒精确、`column` 可进入行内；`unit` 两口径、`column` 上限、`maxLineChars` 范围等**参数细节**属 schema，不在描述里复述。
- `super_edit` 描述必须写明：**按行列地址定位**（不靠文本锚点）、**只改目标区间（区间外字节原样保留）**；`expected` 必填条件属 schema / 手册。
- **不得用否定式定义工具**（「不做子串匹配」这类）。两条理由：一是冗余——「按行列地址」已经蕴含了不靠文本定位；二是会被误读成「不用提供准确文本」，而 `expected` 恰恰要求逐字节相等，两个句子连起来自相矛盾。该说的是**定位方式**（按位置，不需要唯一锚点），不是「不比对文本」。面板文案同理：只写用户能感知的收益（「重复内容不影响定位」），不写机制否定。
- 三个工具的文案都**不得**承诺「替代 `read` / `edit`」，也不得在描述里写「先用 `read` 再……」一类假设性反向引导：默认路径仍是零成本的 `read` / `edit`，升级引导只在 §3.6 的运行时 `[hint]` 里、按事实触发。
- **描述的第一段必须含否定式选择指引**（「默认用 `read` / `edit`；只有 X 时才用本工具」）。这句话**只进 `brief()`**——它服务「申请前」的判断，放在 `full` 档（`description()`）里会变成调用时的反向指令（模型已经申请成功、工具已在数组里）。`brief()` 在可升级段里逐条出现，抑制效果比常驻描述更强，成本还更低。
- **描述预算按档位分开计**：`description()`（`full`）≤ 600 字符，内容是「怎么用不出错」——会失败的约束、与相邻工具的互操作；`brief()` ≤ 120 字符，内容是「一句用途 + 何时该申请」。`input_schema` 的字段说明是硬成本，压不动。旧口径「单条 ≤ 400 字符 + 首段否定式」是给常驻描述设的，T7 拆档后已作废（G19 已重写）。
- **同一工具三套文案，各自受众**：`description()` 是 `full` 档（只随 tools 数组下发给模型）；`brief()` 是 `brief` 档（可升级段逐条，≤ 120 字符，给模型判断要不要申请）；面板文案是**给用户**的（`chat.tool.panel.*`，一句话用途 + 「跟随 `read` 可用」）。三者不得互相替代——尤其**不得把 `description()` 直接搬到面板**（用户看到的是给模型的调用指令，与面板要表达的「它属于谁、谁决定它」无关）。

---

## §7 与 A 的接口

### 7.1 A 拥有（B 只对齐字面量，不复用实现）

`LINE_NUMBER_WIDTH` / `LINE_NUMBER_SEPARATOR` / `LINE_TRUNCATED_MARKER` / `MAX_LINE_CHARS`（默认宽度）、footer 标记集与顺序、`data` 既有八键、`ReadReceipt` 六字段、截断提示的归因轴。

**B 不消费** A 的 `LineSink` / `TextSink` / `scan_into` / `Scanner`：这些是 A 的私有实现，B 自写扫描与渲染，只保证输出字面量与判据对齐。

**B 侧最小触碰（穷举，共两处，均为文案追加、不改变 A 的行为分支）**：

| 触碰点 | 追加内容 | 门控 |
|---|---|---|
| `agent_read/mod.rs` 的 `capability_hint` | `[hint: …]`（§3.6 前两条） | `available_tool_names` 含 `request_tool` |
| `agent_edit/report.rs` 的 0 匹配诊断 | `[hint: …]`（§3.6 第三条） | 同上 |

除这两处外，A 的任何文件都不因 B 而改动；A 也不因此感知 `super_read` / `super_edit` 的存在（§0.3 / §12 R10）。

### 7.2 B 自持（实现全部归 B）

`Tool::manual()`（trait 新方法）、`Tool::companion_of()`（语义为「准入基工具」）、`ToolDescriptor.companion_of`、`ToolRegistry::exclude_companions()`（原 `expand_enabled_tools`，语义由「加法展开」改为「身份排除」）、`ToolEscalationPort`（`contract/` 新增）与 `ToolUseContext.escalation`、`ToolExecCtx.tools` / `ToolExecCtx.escalations`、`ToolUseContext.available_tool_names`、`ToolUseContext.index_root`（索引根路径由调用方注入——`assistant-tools` 是库 crate，不解析 `AppDirs`）、`AppDirs::indexes()`、footer 登记表 `os::footer`（`render` / `TAGS` / `trailing_marker`）、`data.index`、三个新工具的 `data`。

**B 内部再分两层**（都属 B，A 一律不感知）：

| 层 | 模块 | 职责 | 消费者 |
|---|---|---|---|
| 公共层 | `super_index/{format.rs,cache.rs,proof.rs}` | 索引格式与落盘（`format`：`build` / `load` / `store` / `line_break` / `detect`）、缓存与单飞（`cache`：`index_for` / `Flight` / `rebuild_offsets` / `invalidate_path` / `IndexState`）、行起点自证与单行解码（`proof`：`read_line` / `decode_line` / `byte_column_to_char_start` / `IndexError`） | `super_read`、`super_edit` |
| 工具层 | `super_read/{mod.rs,render.rs}` | 请求解析、`unit` 口径、窗口渲染（格式对齐 A）、footer、回执 | 注册表 |
| 工具层 | `super_edit/{mod.rs,locate.rs,apply.rs}` | 行地址定位（消费 `proof`）、`expected` 比对、三段流式写、回执与失效 | 注册表 |

- **为什么公共层只到「索引 + 自证」，不合并成一个工具**：`super_read` 与 `super_edit` 的对外 schema、审批键、`data`、回执语义都不同；能共享的只有「行起点表长什么样、怎么建、怎么证」。再往上合并会把读写两套语义耦合成一个巨型工具。
- **`proof` 的错误类型只定义一次**（`IndexError`）：`render.rs` 用 `use ... as RenderError` 别名保留既有命名，`locate.rs` 直接用 `IndexError`——两份消费者读同一组变体，新增失败码不会只加在一侧。
- **`invalidate_path` 的调用方**：只有 B 自己的写工具（`super_edit`），属 B→B，与回执失效同批。A 的写侧工具（`edit` / `write` / `delete` / `move`）**不调用它**：索引正确性由行起点自证（§4），不靠写侧通知；A 因此完全不感知 B 的存在（§12 R10）。

### 7.3 回写登记点

footer 的登记点已**收敛为一处**：`os/shared/footer.rs`。生产方（`read` / `super_read` / `grep` / `list` / `glob` / `request_tool` / `output`）只调 `footer::render()`，消费方（`agent_edit/report.rs`）只调 `footer::trailing_marker()`，两侧读同一份 `footer::TAGS`。

- 新增 tag 的唯一动作：在 `footer.rs` 加常量 + 进 `TAGS`。生产方若绕过 `render` 手写字面量，视为未交付。
- 原先「I57 §3.3 / `FOOTER_TAGS` / 生产方三处同步」的约定**作废**：它正是漏配的来源（`super_read` 新增的 `index` / `column` 只加在生产方）。
- 回执字段的登记点仍是 I57 §3.5（A 拥有，B 只写同一份、不加字段）。

---

## §8 验收

- 断言落在 `crates/assistant-tools/tests/{request_tool.rs,super_read.rs,super_edit.rs}`；每条门禁（§10）至少一条断言。
- 回归口径：`cargo test -p assistant-tools`（cwd = `src-tauri`）；`request_tool` 与升级追加另需 `cargo test -p assistant-agent`（`ToolExecCtx.tools` / `ToolExecCtx.escalations` 变更），G21 / G22 在 `assistant-agent` 侧断言（准入校验需要注册表与 `available_tool_names`，属适配器层；`disabled_tools` 不参与伴随项校验）。

---

## §9 已知限制

| 编号 | 限制 | 理由 | 逃生路径 |
|---|---|---|---|
| L1 | 索引只有行起点，无列信息 | 列信息会让索引体积随行长增长 | 列由 `unit` + 扫描推进 |
| L2 | 索引不跨进程 / 不跨会话共享 | 需要锁与一致性协议 | 同 revision 复用落盘文件 |
| L3 | `super_edit` 无子串匹配 | 子串匹配正是 `edit` 已暴露的歧义来源 | 用 `grep` 定位后按行地址编辑 |
| L4 | `request_tool` 只能在**下一迭代**生效，申请当轮调不到 | 工具执行发生在迭代中段，改工具集只能从下一次 provider 请求起算 | 重复申请幂等；同轮内先做别的事，下一迭代再调 |
| L5 | 索引不覆盖二进制与不可判定编码 | 与 `read` 同一判据 | 无（fail-closed） |
| L6 | `unit = "char"` 的列上限 20 万 | 单行解码 CPU 上界 | `unit = "byte"` |
| L7 | 默认不压缩（8 字节/行） | 压缩要额外 CPU 与随机读；差分编码属 §3.5 扩展点，默认不启用 | 调 `MAX_INDEX_FILE_BYTES` 或启用差分编码 |
| L8 | 单次只替换一个区间 | 多区间要处理偏移联动 | 多次调用 |
| L9 | **双实现漂移**：B 自写渲染，与 A 的格式只靠测试对齐，不靠共享代码 | 实现独立是刻意选择（B 可以加列窗口、索引提示，A 不受牵连） | G12 / G6 的逐字节对齐测试是唯一防线；A 改字面量时 B 的测试先红，而不是线上静默错位 |
| L10 | 索引文件对用户不可见、不进 `asset` 协议作用域 | 它是派生数据，不是用户资产 | 删除索引目录即回到「首次调用重建」 |
| L11 | `totalLines` 的精确性以「建索引时看到的那一版换行布局」为界 | `file_revision = len:nanos` 无文件身份、无内容指纹：等长改写落在同一 mtime 时间片（FAT/exFAT 2 s、SMB 1 s、Windows 惰性 mtime）时它识别不了；窗口内的切分由行起点自证保证正确（§4），窗口**外**的换行变化则不会被发现 | 给 `encoding` 显式参数、或先 `read` 一次触发重建；`totalLines` 的可信度按「是否刚重建过」判断 |
| L12 | 写侧不通知派生缓存 | 通知只能看见经过本工具集的写入，对外部改写无效（§4 / §12 R10）；把它当正确性手段会让 A 反向依赖 B | 正确性由自证保证；`invalidate_path` 退化为纯缓存卫生，仅 `super_edit` 自调 |
| L13 | `super_edit` 只改目标区间，**不做**整篇行尾归一化 | 区间外字节保真是流式写的直接结果：若文件本身混用行尾（部分 `\n`、部分 `\r\n`），`edit` 会把整篇统一成一种，`super_edit` 则只让 `replacement` 跟随自证到的 pattern，区间外保持原样 | 需要整篇归一化时用 `edit`；两者语义差异已在 §3.3 写明 |
| L14 | `super_edit` 的目标区间上限 16 MiB | 区间要解码驻留以比对 `expected`，无上限等于把内存交还给输入 | 拆成多次调用（F11） |
| L15 | 注入面**只增不减**，且升级轮要付一次全量 prefill | 工具数组在消息之前，尾部追加会让该轮的消息前缀失配（§3.7）；零常驻的收益只有在升级频率低时才成立 | 升级频率高时回到默认注入（OQ8）；升级不持久化，下一轮自然回到零常驻（L17） |
| L16 | 两个超级工具与 `read` / `edit` 语义相邻，存在选择噪声 | 相邻语义 + 较大 schema 会增加误选与参数填错；token 可缓存，选择成本不可 | 默认不出现已消掉常驻噪声（§3.6）；升级轮由 `brief()` 在申请前抑制（§6），`description()` 只讲调用约束，不重复「默认用别的工具」 |
| L17 | 升级不跨轮持久化 | 会话级状态要落库 + 改 DTO（破 G15），且「上一轮需要」不代表「这一轮需要」；陈旧状态会让工具集逐轮膨胀 | 模型在需要的轮次重新申请；准入判据每轮现算，无陈旧状态 |
| L18 | 用户**无法单独关掉**超级工具（面板不提供开关） | 「跟随基工具可用」的必然结果：伴随条目的唯一真值是基工具，给独立开关就造出第二个事实来源（§3.6 / R18） | 关掉基工具（`read` / `edit`）即整体不可用；`disabledTools` 里的伴随 id 被忽略（既不阻断也不生效），因此不存在「配置导入关掉它、面板却显示可用」的分叉。**残留项不靠提示解释**：写回前 `stripCompanionIds` 静默剔除（四个写回口统一），用户下一次保存即完成清理；后端 `exclude_companions` 另打一条 `log::warn`（每 id 每进程一次去重）供排查 |

---

## §10 门禁

| 编号 | 门禁 | 判据 |
|---|---|---|
| G1 | `Tool::manual()` 有默认实现 | 未实现 `manual()` 的既有工具零改动编译通过 |
| G2 | 可用集透传 | `request_tool` 返回的 `listed` 与本轮 `TurnRunCtx.tools` 的 `name` 集合一致（`source = "turn"`） |
| G3 | 回退标注 | 无 `tools` 时 `source = "registry"`，文案含回退说明 |
| G4 | 工具集只经循环尾部追加 | `detail = "list"` / `"manual"` 前后，下一迭代 provider 请求的工具清单逐字节不变；`detail = "enable"` 后只允许**尾部追加**（旧数组前缀逐字节不变），且被追加项满足 §3.6 准入三条 |
| G5 | 手册上限 | 每工具 ≤ 2000 字符、总量 ≤ 8000；超出时 `[catalog: showing {k} of {n}]` 且 `FOOTER_TAGS` 能识别 |
| G6 | 索引正确性 | 任意文本（含 CRLF、末尾无换行、BOM、GBK）建索引后，`offsets` 切片读出的行与 `read` 输出**逐字节一致**（B 自写渲染，靠本门禁而非共享代码对齐，§9 L9） |
| G7 | 精确总数 | `super_read` 的 `totalLines` 恒等于 `wc -l` 语义值（含末尾无换行的 +1 规则） |
| G8 | `unit = "byte"` | 同一行 `unit=char` 与 `unit=byte` 的 `column` 在 ASCII 行上等价；BOM 文件首列字节列 = 内容起始 |
| G9 | 单飞 | 同 `(path, revision)` 并发 N 次只建一次索引（断言 `state` 为 `built` 的次数 = 1） |
| G10 | 失效 | **外部等长改写**（同长度、mtime 显式设回原值，使 `revision` 不变）且移动了窗口内换行 → 下一次 `super_read` 经行起点自证发现并自动重建，正文正确；`super_edit` 后索引失效 |
| G11 | 容量 | 缓存按字节 LRU 淘汰，落盘单文件 > 64 MiB 时 F1 |
| G12 | 渲染一致 | `super_read` 与 `read` 在同一窗口参数下的正文**逐字节一致**（footer 除外）；覆盖超长行与 `column > 1`。B 的渲染是独立实现，本门禁即 §9 L9 的唯一防线 |
| G13 | 回执同源 | `super_read` 写入的回执字段与 `read` 完全同构；`truncated_lines > 0` 时 `edit` 同样 fail-closed |
| G14 | `super_edit` 校验 | F7 / F8 / F9 / F10 / F11 各有用例；成功路径下**区间外字节逐字节不变**（head / tail 原样拷贝，含 BOM 与 CRLF 混用文件）、`invalidate_path` 被调用 |
| G15 | 无 wire 变更 | `npm run contract:gen` 无 diff；前端零改动（**限 T1–T6**；T7 只做面板的伴随展示（渲染层，无新字段）、T8 后端零前端改动，均不改 wire / DTO）；`read` / `edit` 既有测试全通过 |
| G16 | footer 单一来源 | 生产方不出现手写 `[<tag>: …]` 字面量（tag 只能来自 `footer` 常量）；`footer::TAGS` 每一项都能被 `trailing_marker` 识别、无重复项；`edit` 的 0 匹配诊断对每个登记 tag 都给出文案 |
| G17 | 伴随 = 准入（不是注入） | `super_read` / `super_edit` 的 `companion_of` 分别为 `Some("read")` / `Some("edit")`；勾 `read` 时本轮 `ToolSpec` **不含** `super_read`；`enabledTools` 含 `super_read`（无论 `read` 是否也在）→ 本轮 `ToolSpec` 仍**不含** `super_read`；`disabledTools` 含 `super_read` 而 `enabledTools` 含 `read` → `read` 正常注入、`super_read` 仍不注入（伴随 id 在两个名单里一律忽略）；`disabledTools` 含 `read` → `read` 与 `super_read` **都**不在本轮（不存在孤儿残留）；`request_tool` 的可升级段在 `read` 与 `request_tool` **都**在本轮时出现，任一缺失则整段不出现 |
| G18 | 判据单一实现 | 行起点自证、单行解码、索引缓存与单飞只存在于 `super_index`；`super_read/{mod,render}.rs` 与 `super_edit/{mod,locate,apply}.rs` 里**没有**第二份起点断言或第二份缓存实现（结构性判据：删掉 `super_index/proof.rs` 后两工具均无法编译）。`super_edit` 在从未调用过 `super_read` 的会话里首次调用即可成功（断言它自行建索引） |
| G19 | 注入预算（按档位分别计） | **`full` 档**（`description()`）：两个超级工具各 ≤ 600 字符、`description + input_schema` 合计 ≤ 4 KiB；内容为「怎么用不出错」（会失败的约束 + 与 `read` / `edit` 的互操作），**不得**含「默认用 X」这类选择指引或「何时该申请」；**`brief` 档**（`Tool::brief()`）：各 ≤ 120 字符，内容为「一句用途 + 何时该申请」，**不得**含 schema、手册正文或调用约束；`brief` 缺省时回落到 `description()` 首句且该首句仍须满足上述 brief 约束；provider 请求里的 `ToolSpec` 只由 `description` 与 `input_schema` 构成（`manual()` / `brief()` 不进注入） |
| G20 | 用户路径：伴随展示、元工具勾选 | 面板里 `companionOf != null` 的条目**渲染在基工具下方**（缩进一级、无勾选框、不写 `enabledTools` / `disabledTools`）；基工具勾选时显示「跟随 `read` 可用，模型按需启用」，未勾时显示「需先启用 `read`」；`enabled_count` 不计伴随条目、**计入元工具**；勾 `read` 后本轮工具集不含 `super_read`；**`request_tool` 是可勾选项**：未勾 → 不在本轮、`read` 的 `[hint]` 不出现；勾上 → 在本轮、`[hint]` 出现且文案指向 `detail = "enable"`；`disabledTools` 含 `request_tool` → 同样不注入 |
| G21 | 升级准入 | `request_tool(detail = "enable", tools = ["super_read"])` 而 `read` 不在本轮 → F14，工具集不变；**`request_tool` 未勾选时模型根本无法发起该调用**（不在 tools 数组，provider 侧即不可用），不构成「绕过准入」的路径；`disabledTools` 里的 `super_read` **不构成阻断**（与 G17 一致：伴随 id 在两个名单里一律忽略，用户唯一控制点是基工具）；申请没有 `companion_of` 的工具 → F14；`tools` 缺失或名字全部不存在 → F13；**部分通过 → `ok = true` 且 `data.enabled` 与 `data.rejected` 同时非空**（§3.8 ⑤），合法项照常登记 |
| G22 | 升级生效面 | 合法申请后：**下一迭代**的 provider 请求含 `super_read` 且旧工具数组前缀逐字节不变；`ToolExecCtx.tools` 与 `available_tool_names` 同步含它；申请所在迭代内不生效；跨轮不携带（不持久化，L17） |
| G23 | 描述档位三档不变式 | 逐条断言 §3.6：①`full ⇔ callable`——本轮 provider 请求里的每一项都有 `description` + `input_schema`，且 `data.escalatable` 里的项**不在** tools 数组内；②`brief ⇒ eligible`——可升级段的每一项基工具都在 `available_tool_names`；③`none ⇔ ¬eligible`——基工具不在本轮时该工具既不出现在 `listed`、也不出现在 `escalatable`、也不出现在 tools 数组；④`listed ∩ escalatable = ∅`；⑤`brief` 内容不含 `input_schema` 与手册正文 |

---

## §11 切片

| 编号 | 切片 | 交付物 | 依赖 |
|---|---|---|---|
| T1 | 能力发现 | `Tool::manual()` 默认实现；`ToolExecCtx.tools` + 唯一构造点；`ToolUseContext.available_tool_names` 透传；`agent_request_tool.rs`；注册进 `register_readonly_tools`；G1–G5 | **无**（可与 P16 并行） |
| T2 | 索引核心 | `AppDirs::indexes()` + `dir_name.rs`；`super_index/format.rs`（格式、建、读切片、校验）；G6 / G7 / G11 | **无**（B 自写扫描器与行起点记录；只对齐行数与切行判据，不复用 A 的代码） |
| T3 | `super_read` 工具 | schema、审批键、`super_read/render.rs`（自写渲染，格式对齐 A）、`unit` 两种口径、`data.index`、`[index]` / `[column]` / `[hint]` footer、`companion_of`；G8 / G12 / G16 / G17 | T2 |
| T4 | 生命周期 | 单飞、行起点自证（`StaleIndex` → 重建一次）、`revision` 粗筛、LRU、回执写入；G9 / G10 / G13 | T3 |
| T5 | `super_edit` | **索引层抽为 `super_index/{format,cache,proof}.rs`**（`super_read` 改为消费它，行为零变化）、schema、`expected` 规则、行地址定位、**索引驱动的字节区间 + 两端自证**、**三段流式写**（区间外零改动）、回执与失效；G14 / G18；footer 新 tag 只在 `os::footer` 登记一处 | T4 |
| T6 | 收口 | 三工具文案、失败矩阵全用例、`tool_setup.rs` 注册组、门禁复核 | T5 |
| T7 | 准入语义 + 伴随展示 | `expand_enabled_tools` → `exclude_companions`（**身份过滤**：`companion_of != None` 的 id 一律不进候选集，两个名单里的伴随 id 均忽略）；`resolve_available_tools` **移除元工具特殊注入**，`request_tool` 按普通条目过滤；`read` 的 `[hint]` 门控改为 `request_tool` 在本轮、文案指向 `detail = "enable"`；`edit` 的 0 匹配诊断加同门控提示；**描述按档位拆分**（`Tool::brief()` 新增默认实现，两个超级工具各自覆写；`description()` 改为只写「怎么用不出错」并补回会失败的约束）；**面板伴随展示**（`companionOf` 从属行、无勾选框、状态随基工具、计数不计、`companion_of` / `companion_blocked` 两条文案）+ **元工具可勾选**（组内末尾、「申请入口」标签、计入计数）；G17 / G19 / G20 | T6 |
| T8 | 升级通道 | `request_tool` 增 `detail = "enable"` / `tools` / `escalatable` / `enabled` / `rejected`；**`contract/escalation.rs`**（`ToolEscalationPort` + `EscalationCandidate` / `EscalationRejectReason` / `EscalationRejection` / `EscalationOutcome`）+ `ToolUseContext.escalation`；**`EscalationSink` + `ToolExecCtx.escalations: Option<Arc<dyn EscalationSink>>`**（`assistant-agent`）；**`adapters/tool_escalation.rs`**（registry + `MaxToolTierSource` + 基工具集校验 + 解析 `ToolSpec`；`disabled_tools` 不参与伴随项）；循环 `RoundState.tools: Option<Arc<Vec<ToolSpec>>>` + 轮内尾部追加；G4 / G21 / G22；契约见 §3.8 | T7 |

- T7 是本阶段**唯一**触碰前端的切片：`ToolConfigPanel` 把 `companionOf != null` 的条目渲染为基工具下的从属行（缩进、只读状态点、状态随基工具勾选、`enabled_count` 不计、两条新文案），不写 `enabledTools` / `disabledTools`；`request_tool` 改为可勾选项（组内末尾、「申请入口」标签、计入计数）。无需改 DTO / wire。T1–T6 与 T8 的「前端零改动」不受影响（G15 已限定范围）。
- T7 的判据是**身份**而不是**关系**：`exclude_companions` 只问「这个 id 是不是伴随工具」，不问「它的基工具在不在名单里」。这样 `callable` 只剩一个来源（模型申请），白名单里残留的伴随 id 不构成第二个事实来源，也不会出现「禁用基工具而伴随残留」的过滤顺序缺陷（§12 R19）。元工具的 `companion_of = None`，因此它天然落在同一条身份过滤之外，走普通条目路径——「需勾选」不需要任何特殊分支。
- T8 的顺序约束：**先 T7 再 T8**。T7 把「伴随 = 注入」拿掉之后，`request_tool` 才有必要成为唯一入口；反过来做会让升级通道与展开机制并存，门禁无法区分两条注入路径。

---

## §12 拒绝方案

| 编号 | 方案 | 拒绝理由 |
|---|---|---|
| R1 | 把索引型读取塞进 `read` 的参数（如 `useIndex=true`） | 让零成本路径隐式变贵，且 `read` 会持有跨调用状态 |
| R2 | 用内存缓存代替落盘索引 | 进程重启即失效，大文件重复扫描无法避免 |
| R3 | 索引按字节偏移存「每 N 行」采样 | 任意行定位退化为区间扫描，`totalLines` 仍不精确 |
| R4 | `super_edit` 复用 `old_string` 子串匹配 | 与 `edit` 重复同一歧义来源，且长行 `old` 不可得 |
| R5 | `expected` 设为可选 | 大区间替换会在漂移后静默覆盖 |
| R6 | 让 B 自定义**行格式**（行号宽度 / 分隔符 / 截断标记 / footer 形态） | 格式是互操作契约：`edit` 档 2 靠这些字面量剥前缀、`trailing_footer_marker` 靠方括号识别 footer；格式一漂移，模型把 `super_read` 输出喂给 `edit` 就静默匹配失败。**注意**：本阶段允许 B 自带渲染**实现**（§0.3），被拒的只是「自定格式」 |
| R7 | 让 `request_tool` 在工具内部**直接**改写本轮工具集 / provider 请求 | ①工具持有的是不可变 `ToolUseContext`，改请求需要把请求所有权下放给工具，破坏「工具只读上下文」的既有边界；②审批面与事件面都以「循环持有唯一请求」为前提；③同批次并发工具会争抢同一份可变请求。替代方案：工具只登记申请，循环在批次结束后统一尾部追加（§3.6 / T8） |
| R8 | 索引文件放工作区 | 污染用户仓库，且 `resolve_readable` 会把索引当用户文件 |
| R9 | 索引共享给子代理进程 | 需要跨进程锁与一致性协议（§9 L2） |
| R10 | 让 A 的写侧工具（`edit` / `write` / `delete` / `move`）在成功后主动通知 B 失效索引 | ①**覆盖面错位**：通知只能看见经过本工具集的写入，编辑器保存 / `git checkout` / 构建产物覆盖都不经过它，而我们的写工具走 temp+rename 必然换 mtime，`revision` 自己就能发现；②**原则反了**：通用写工具硬编码知道 `super_read`，违反 §0.3 单向依赖；③**机制不可靠**：靠「每处调用点都记得加一行」，`move` 覆盖目标时已漏 `target`；④它不解决它声称的问题——等长改写 + 同 mtime 时间片正是通知覆盖不到的那一格。替代方案：行起点自证（§4），B 自持、A 零改动 |
| R11 | 把 `file_revision` 升级成内容哈希以支持索引失效 | `read` 是流式分块读，给回执塞整文件哈希会把成本压到零成本路径上；且 `super_read` 只需发现「换行位置是否变化」，哈希是过度手段。索引正确性改由行起点自证承担（§4 / §9 L11） |
| R12 | 索引命中时比对首尾采样指纹（概率性新鲜度检查） | 仍是采样而非判据，采样区之外漏判；且每次命中多一次 IO。行起点自证是**判据**，代价更小（每行多 1–2 字节） |
| R13 | `super_edit` 沿用 `agent_edit/apply.rs` 的整文件读 + 整篇重编码写回 | ①**闸门错位**：`super_read` 能读 1 GB 文件，`super_edit` 却被 `MAX_WHOLE_FILE_BYTES = 16 MiB` 挡住，同一家族读写能力不一致；②**非 UTF-8 更糟**：整篇重编码受 `MAX_TRANSCODE_BYTES = 1 MiB` 约束，2 MiB 的 GBK 文件里改 5 个字符会被拒；③**保真倒退**：整篇归一化行尾会让区间外字节漂移，而编辑本应只动目标区间。替代方案：索引给出字节区间 + 两端自证 + `head`/`replacement`/`tail` 三段流式写（§3.3） |
| R14 | 让 `super_edit` 自带一份定位与自证（或让 `super_read` 把内部符号逐个 `pub(crate)` 暴露给它） | ①**判据必须单源**：两端自证是「区间正确」的唯一判据，复制两份后任一侧改判据都会让读写对同一文件给出不同结论，而这种漂移不会被任何单测发现；②**缓存与单飞必须共享**：`super_edit` 首次调用要复用 `super_read` 建好的起点表，各自一份等于同一文件两套缓存、两份落盘索引；③逐符号 `pub(crate)` 会让「编辑依赖读取模块」，第三个超级工具出现时继续堆在同一个模块里。替代方案：抽出 B 私有公共层 `super_index`（`format` / `cache` / `proof`），两侧只做消费（§7.2 / G18） |
| R15 | 会话级升级状态（本轮启用后跨轮保持，或落库记忆） | ①要落库并扩 DTO，破 G15；②「上一轮需要行内读取」不代表「这一轮需要」，陈旧状态会让工具集逐轮膨胀，回到默认注入的老问题；③准入判据必须每轮现算（`available_tool_names` 是唯一事实）。替代方案：升级只本轮有效（§9 L17） |
| R16 | 链式伴随（`super_edit` 的准入基工具设成 `super_read`，或一次申请连带带入 `super_read`） | ①准入关系退化为传递闭包，判据不再是「单一基工具在本轮」，校验与清单都要处理链；②一次申请带入两个工具，注入面翻倍，与「少而准」相反；③`super_edit` 自己就能惰性建索引（§3.3），不需要 `super_read` 先跑 |
| R17 | 把 `column` / `unit` 做成 `read` 的可选参数、把行地址做成 `edit` 的可选参数 | ①违反 §2「不替换 `read` / `edit` 默认路径」；②让 99% 的普通调用多背两个字段，schema 膨胀落在零成本路径上；③索引状态会进 `read`，`read` 变成有状态工具（同 R1）。列在此仅为成本对照 |
| R18 | 面板给超级工具**独立勾选框**（哪怕默认勾选、或做成「三态」） | ①勾选没有可解释语义：伴随工具在配置路径上被 `exclude_companions` 一律排除（§3.6），勾了等于没勾；②勾选框只能表达「要不要注入」，而那正是模型按需决定的事，用户无从判断；③一旦可勾，`enabledTools` 就成了第二个事实来源，与「`callable` 只由申请产生」冲突（G17 / G23 无法断言）。替代方案：伴随展示（只读从属行，§3.6） |
| R19 | 白名单**关系剪枝**（`prune_orphan_companions`：保留 companion ⟺ 基工具也在白名单） | ①**残留第二种来源**：显式配置里两者都在时会注入超级工具，`callable` 便有两个来源（用户配置 + 模型申请），与 §3.6 三档分离直接冲突；②**面板文案与事实不符**：面板已无勾选框，那条「跟随基工具可用、模型按需启用」的说明会与实际每轮 ≈1.95 KB 的常驻注入矛盾，而这正是本次要消除的理解不一致；③**过滤顺序缺陷**：`disabled` 在剪枝之后生效时，`enabled = {read, super_read}` + `disabled = {read}` 会留下孤儿 `super_read`（否决只删自身、不重算依赖），修它要额外规定顺序，而身份过滤天然免疫；④剪枝让「没有基础工具就不该能用」变成需要运行时查表的判据，而身份过滤把它变成结构性事实。替代方案：`exclude_companions`（§3.6），代价是接受「显式配置的伴随 id 视为脏数据」 |
| R20 | 让 `assistant-agent` 直接持有 `ToolEscalationPort` / 注册表来解析 `ToolSpec` | ①**依赖方向**：`assistant-agent` 不依赖 `assistant-tools`，`assistant-tools` 不依赖 `assistant-providers`——循环侧拿不到注册表，工具侧也造不出 `ToolSpec`；②即使强行加依赖，也会把注册表与 provider 类型塞进循环内核，`assistant-agent` 的单测被迫启动工具注册表；③「谁解析申请」与「谁改工具集」分开是 R7 的前提：解析必须在持有注册表的一侧（适配器），循环只消费成品 `ToolSpec`。替代方案：三段契约（§3.8），适配器接合 |
| R21 | 把本轮 `maxToolTier` 塞进 `TurnRunCtx` 让循环传给端口 | ①**层级是会话级配置**（模式 `maxToolTier`），而 `TurnRunCtx` 是执行快照，塞进去等于让内核知道配置域，与 `ToolUseContext` 只携带「本轮事实」的定位冲突；②层级每轮可能随会话切换变化，快照语义会诱导「一次 turn 内不变」的误用；③现成的 `WorkspaceRootSource` 已示范「按 `conversation_id` 现求值」的形态，复用即可。替代方案：`MaxToolTierSource`（§3.8 ③） |

---

## §13 未决问题

| 编号 | 问题 | 影响 |
|---|---|---|
| OQ1 | `MAX_INDEX_FILE_BYTES` 与缓存条数是否按实测校准（64 MiB ≈ 830 万行） | G11 阈值 |
| OQ2 | `MAX_COLUMN_CHARS = 200_000` 的 CPU 上界是否需按机器分档 | F6 阈值 |
| OQ3 | ~~`super_edit` 是否随第一版交付~~ **已定稿**：随第一版交付，写入形态为「索引驱动的字节区间 + 三段流式写」（§3.3 / §12 R13） | T5 排期 |
| OQ4 | ~~`[catalog]` 用圆括号是否与 footer 封闭集冲突~~ **已定稿**：统一为方括号 `[catalog: showing {k} of {n}]`（`trailing_footer_marker` 只认 `[<tag>: …]`，圆括号无法识别） | §3.4 形态（T1 已按此实现） |
| OQ5 | `request_tool` 的 `query` 是否支持 `tier` / 分组过滤 | 手册检索精度 |
| OQ6 | 索引扩展段（`crc32` / 差分编码 / 列提示）是否第一版启用 | §3.5 扩展点；默认**全不启用**，按实测（建索引耗时、索引体积）再逐个开 |
| OQ7 | 一次 `detail = "enable"` 是否允许申请多个工具 | 当前按 `tools` 数组放开（每项独立校验），但首版只预期一个；实测滥用后再收紧为单选 |
| OQ8 | 升级频率高到何种程度就该回到「默认注入」 | L15 的取舍点：升级轮要付一次全量 prefill，若命中率接近每轮，零常驻反而更贵；需要实测命中率后再定 |
| OQ9 | 是否由基工具在结果里填内部 `escalation` 信号、循环直接注入（省一次模型往返） | 与「模型主动请求」的产品语义不一致，且工具会重新变成「能影响工具集」；本阶段不做（§3.7 表 F） |
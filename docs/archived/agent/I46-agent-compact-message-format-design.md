# I46 — Agent 紧凑消息格式设计（ADR）

| 项 | 值 |
|---|---|
| 状态 | 参考记录（已被 [I46 节点布局设计](./I46-agent-node-layout-design.md) 取代，**不再作为权威源**；信封 / 省略 / 落盘 / 预算部分仍有效，见该文档 §9） |
| 日期 | 2026-11-13（2026-11-13 修订：补入参考项目研究结论，见 §7） |
| 关联 | [I18 工具架构 ADR](./I18-agent-tool-architecture-design.md)（工具分层与权限网关）、[I38 阶段总纲](../../store/I38-agent-roadmap.md)（阶段与门禁唯一入口）、[I36a 内核能力基线](../../store/I36a-agent-kernel-baseline.md)（turn 事件不变式）、[I36b 内核一致性规格](../../store/I36b-agent-kernel-conformance-spec.md)、[I27 工具数据存储规格](../../store/I27-tool-data-store-spec.md)、[I38f 子 Agent 规格](../../store/I38f-agent-subagent-spec.md)（`data` 进程内消费的现例）；参考项目：DeepSeek Harness `architecture` notes（规范工具输出契约、工具结果保留库、工具输出 spill 策略、本地 spill 启动清理、结果时刻 applied-hunk diff、结构化错误分类、会话持久化、zstd JSONL 日志） |
| 影响范围 | 设计层：定义 Agent 输出的文本信封语法、省略口径、wire 增量字段、预算归属、落盘生命周期、渲染形态选路（卡片 / 文档批注）与 **turn 级密度控制**（迭代折叠）。不修改现有 DTO 定义，不改 turn 事件语义 |

> 本文只回答六件事：Agent 的一次工具输出在**四个消费面**（模型回填 / 落库 / 事件 / 前端）各是什么形状；**谁负责收缩**；收缩之后**如何说明省略**；**落盘产物如何命名、定位与过期**；**前端按什么条件选卡片还是文档批注**；**一个多迭代 turn 占多少高度、如何折叠**。
> 实施批次与进度不在本文维护，归 [I38 §2](../../store/I38-agent-roadmap.md) 与对应 tasklist。新增阶段须先改 I38 §2/§3，本文不自行编号。

---

## 1 问题

### 1.1 现状事实基线

| # | 事实 | 代码位置 |
|---|---|---|
| F1 | 工具结果声明了双通道：`data`（UI 完整结构化值）+ `result_for_assistant`（模型精简文本）；`assistant_text()` 按 `result_for_assistant > error > data` 回退 | `src-tauri/crates/assistant-tools/src/result.rs` |
| F2 | 传输层只带一条文本：`exec_result.content` 取 `assistant_text()`，事件 `Finished.result` 与落库 tool 行 `content` 是同一字符串 | `src-tauri/src/adapters/tool_exec.rs`、`src-tauri/crates/assistant-tools/src/pipeline.rs` |
| F3 | `data` 不上 wire、不落库、不跨 crate 边界，只在执行进程内存在 | 同 F2（适配器只读 `assistant_text()`） |
| F4 | 回填收缩全部在 Pipeline：单结果 50000 字符落盘、单轮内联 200000 字符、落盘预览 2000 字符；落盘发生在事件发射之前，故 `Finished.result` 已是收缩后的预览 | `src-tauri/crates/assistant-tools/src/storage.rs`、`pipeline.rs`（`select_persist_targets` → `persist` → `report`） |
| F5 | 采集侧上限分散在各工具常量里，取值互不相同 | `os/agent_read.rs`（2000 行 / 1 MB / 单行 2000 字符）、`os/agent_grep.rs`（100 命中 / 300 行）、`os/agent_glob.rs`（1000）、`os/agent_list.rs`（1000 条目 / 深度 10）、`os/agent_edit/mod.rs`（diff 20 行 / 5 处上下文） |
| F6 | 省略提示有 4 种语法、精确度不一：有的给总数，有的只给「还有更多」 | `os/output.rs`、`os/agent_read.rs`（`truncation_notice` 两分支）、`os/agent_grep.rs`（两处各写一句） |
| F7 | 前端反解模型文本：`parsePersistedOutput` 用正则锚定 `<persisted-output path="...">`，`resultStats` 自行统计行数/字节 | `src/chat/tool/toolCallDisplay.ts`、`src/chat/tool/ToolCallBubble.tsx` |
| F8 | 同一结果在 wire 上出现两遍：assistant 行 `toolCalls[].result` 由 `get_tool_results` 从 tool 行回填，而 tool 行本身也在 messages 列表里下发；前端 `MessageList` 又过滤掉 `role !== "tool"` | `src-tauri/src/cmd/conversation_utils.rs`、`src-tauri/crates/assistant-storage/src/conversations/msg_ops.rs`、`src/chat/message/MessageList.tsx` |
| F9 | `ArgsDelta` 原样透传 JSON 片段：不节流、无上限，前端逐片累积后 best-effort 解析，解析失败保留上一份快照 | `src-tauri/src/protocol/dto/agent.rs`、`src/chat/core/toolCallView.ts` |
| F10 | 消息段边界已存在：一个迭代一条 assistant 行一个气泡，工具卡片按 `iteration` 归属 | `src/stores/agent/agentSegments.ts` |
| F11 | 错误分类只在实时载荷里存在：`ToolErrorKind` 有五类（`Business` / `Timeout` / `Cancelled` / `Internal` / `Denied`），前端 `ToolCallView.errorKind` 也有，但 `MessageToolCallDto` 只有 `status` / `result`，历史重载后分类丢失 | `assistant-tools/src/result.rs`、`protocol/dto/chat.rs`、`src/chat/core/toolCallView.ts` |
| F12 | 落盘定位符在进程内是结构化的（`PersistedRef { path, bytes }`），但 `report` 只发 `assistant_text`，结构在事件边界被丢弃；落盘目录 `tool-results/<call_id>.txt` 全局扁平、无会话命名空间、**全仓无任何清理逻辑** | `assistant-tools/src/pipeline.rs`（`persist` / `report`）、`storage.rs`、`tool_setup.rs` |
| F13 | 采集侧截断只有 head 语义：`retained_end` 从尾部向内收缩，不存在 tail / headTail 策略；命令行类输出的有效信息通常在末尾 | `os/output.rs`（`truncate_output`） |
| F14 | `edit` 的 diff 是**调用时刻**由 `old_string` / `new_string` 生成的片段，上限 20 行、5 处上下文，不是结果时刻的 applied-hunk diff | `os/agent_edit/report.rs` |
| F15 | `messages.tool_calls_json` 的建表注释仍写「当前写入恒 NULL」，实际已在读写结果回填，注释与实现不符 | `assistant-storage/src/db.rs`、`src/cmd/conversation_utils.rs` |
| F16 | 工具结果只有一种渲染形态：`ToolCallBubble` 卡片，按调用组织；`MessageContent` 只对 assistant 正文做 markdown 渲染，工具结果不进入正文流 | `src/chat/tool/ToolCallBubble.tsx`、`src/chat/message/MessageContent.tsx` |
| F17 | `edit` 在结果里已算出命中行区间（`line_index_at`），但只用于内部定位与诊断文本，未出结构、未进事件 | `os/agent_edit/locate.rs`、`os/agent_edit/text.rs` |
| F18 | 迭代数直接决定容器数：`openSegment` 每迭代追加一条占位 assistant 行（一个气泡）。`showIdentity` 已按「连续 assistant 段」折叠（一轮只显示一次身份行），但 `timestamp` / `MessageActions` / `BranchPicker` 仍是**逐气泡**渲染 | `src/stores/agent/agentSegments.ts`、`src/chat/message/MessageList.tsx`、`MessageBubble.tsx` |
| F19 | `ToolCallBubble` 是**固定高度的多段卡片**：状态头 + 参数行 + 结果预览（≤ 12 行 / 1024 字节）+ 折叠按钮 + 规模统计；`resultStats` 与 `parsePersistedOutput` 都要读正文，故当前**无法只凭头部渲染成单行** | `src/chat/tool/ToolCallBubble.tsx`、`src/chat/tool/toolCallDisplay.ts` |

### 1.2 缺口

- **G1 双通道塌缩**：F1 的双通道在 F2/F3 处收口成单文本，「UI 拿结构、模型拿摘要」这个设计意图在传输层丢失；UI 只能读模型文本。
- **G2 结果重复传输**：F8 同一份正文传两遍，其中一遍必然被前端丢弃；会话历史越长，重复体积越大。
- **G3 前端是文本解析器**：F7 的正则锚定了标签形态与位置，任何格式调整都会让前端静默降级（不报错、只是不再折叠/不再显示落盘提示）。
- **G4 省略不可判定**：F6 的「还有更多内容」与「showing X of Y」混用，消费方无法区分「确实只有这么多」和「被砍了但不知道砍了多少」，模型会误判结果完整性。
- **G5 流内参数无界**：F9 的 `ArgsDelta` 让事件流体积与参数长度线性相关，前端 `argsText` 同步膨胀。
- **G6 定位符未出进程**：F12 的 `PersistedRef` 在事件边界被丢弃，落盘信息只能以文本标签形式回流，UI 与回放只能反解字符串；同时落盘目录无命名空间、无清理，只能无限增长。
- **G7 错误分类不可回放**：F11 的分类只在实时事件里，历史重载后退化为「失败但不知为何失败」，卡片无法按 kind 路由。
- **G8 保留策略单一**：F13 只有 head 语义，所有工具被迫用同一种裁切方向，命令行类输出（错误栈、`exit` 前的末行）会被砍在最不该砍的位置。
- **G9 渲染形态单一**：F16 的卡片按「调用」组织，创作型会话里用户要的是「文档第几行改了什么」，而 F17 已经算出位置却不出结构，只能让用户自己在卡片里对着 diff 猜行号。
- **G10 turn 高度随迭代线性增长**：F18 让 N 次迭代 = N 个气泡（各带时间戳 / 操作 / 分支控件），F19 让每次调用 = 一张固定高度卡片。一次 8 迭代 × 2 调用的 turn 可轻易吃掉 2000+ px，用户需要滚很久才能看到结论；而 F19 的卡片结构使「压成一行」在现状下不可能——`resultStats` 依赖正文。

### 1.3 目标与非目标

目标：一次工具结果在 wire 上**只传一次**；UI 的展示信息**全部来自结构化字段**；每个收缩点都有**精确或显式未知**的省略说明；模型回填文本与历史文本**字节相等**；落盘定位符与错误分类**可回放**；渲染形态可按**锚点可得性**在卡片与文档批注之间切换，且两种形态共用同一份 `compact`；多迭代 turn 的高度**与迭代数解耦**（折叠后只随「步数」线性增长，且每步一行）。

非目标：不改 turn 事件语义（`turn_id` / `seq` 单调 / 终态唯一，见 I36a）；不改压缩投影与分支链拓扑；不改工具权限网关与审批链；不为「省 token」引入二次摘要（摘要会与历史文本分叉）。

---

## 2 决策

### 2.1 D1 三段信封：head / body / tail

一次工具结果 = **一行 head** + 有界 body + 可选 tail。head 由框架渲染，工具只填字段（`subject` / `scale`），禁止工具自己写整句；body 归工具所有；tail 只由框架写。

理由：模型读 head 即知「对象 / 规模 / 成败」，不必扫正文；UI 读 head 即能画卡片标题；head 只有一份拼装点，格式漂移不可能发生（对比 F6 的四处各写各的）。

### 2.2 D2 省略三态：无省略 / 精确 / 显式未知

`truncated` 只有三态，且必须与文本一一对应：

| 态 | 文本 | 结构化 |
|---|---|---|
| 无省略 | 不含 `[truncated:` | `truncated = null` |
| 精确 | `[truncated: showing 120 of 1000 lines, 3.2 KB of 41.5 KB]` | `returned` / `total` 均有值 |
| 未知 | `[truncated: showing 120 lines, 3.2 KB; total unknown]` | `unit = unknown`，`returned` / `total` 为 null |

禁止「more content follows」这类既无总数也无未知标记的写法；确实无法计数时必须显式写 `total unknown`，不得留空。

### 2.3 D3 双通道收口：`result`（文本）+ `compact`（展示投影）

先对齐术语，避免把三个不同东西混成一个「双通道」（术语来源见 §7）：

| 概念 | 我们的载体 | 是否跨 seam | 是否持久化 |
|---|---|---|---|
| **规范值**（执行期程序化值） | `ToolResult.data` | 否（仅进程内） | 否 |
| **模型可见文本** | `ToolResult.result_for_assistant` → `assistant_text()` → `result` | 是 | 是（tool 行 `content`） |
| **展示投影**（UI 载荷） | 新增 `compact` | 是 | 否（可由前两者重建） |

- `result` 文本保留，继续承担「模型回填 + 历史正文 + 字节相等基准」。
- `compact ≡ 展示投影`，作为 **UI 唯一解析来源**：`head` / `truncated` / `persisted` / `errorKind`。
- **显式声明：不引入跨 seam 的规范值通道。** `ToolResult.data` 的合法消费者只有「同进程内的程序化调用方」（现例：subagent 的父 turn 记账，见 [I38f §3.5.1](../../store/I38f-agent-subagent-spec.md)）。一旦出现跨 seam 需求（code-mode / 链式编排 / 结构化剪枝 / 文本装不下的全保真值），**跨 seam 的也必须是它的持久化投影，而不是值本身**。引入条件写在 §7.4，避免将来「因为加了某工具所以加 value」。
- **重建判据（纯函数边界）**：`compact` 的字段分两类——
  - 可由 `(args, 结果文本)` **纯函数算出**的 → 允许在读取时重建，不落库；
  - **算不出**的（如结果时刻的文件前后内容、耗时、退出码之外的进程事实）→ 必须随结果一起持久化，不得指望重建。
- 落库格式不变（`content` 列只存文本）；历史读取时由后端用**唯一解析器**从信封文本重建 `compact`，`parse(render(x)) == x` 由测试锁定。解析失败 → `compact = null`，前端退回只渲染 `result` 原文，不报错。
- 落盘信息**不从文本解析**：`PersistedRef`（F12 已在进程内存在）直接随事件下发，文本标签只是模型侧的检索提示（见 §3.4）。

### 2.4 D4 结果只传一次：tool 行空正文

wire 上 `role = "tool"` 行的 `content` 恒为空串，结果只经 assistant 行 `toolCalls[].result` 下发；DB 里 tool 行正文照常写入（模型回填、压缩投影、崩溃恢复都依赖它）。

理由：F8 已证明前端过滤掉 tool 行，它只承担分支链拓扑。清空正文不改拓扑、不改 sibling 计算，直接把 wire 体积砍掉一半。

### 2.5 D5 事件紧凑：参数有界 + 终态唯一

- `Started` 增加 `argsPreview`（≤ 200 字符，后端按工具声明的关键参数抽取）；UI 首帧即可显示参数摘要，不必等 JSON 完整。
- `ArgsDelta` 降级为**受控回退通道**：仅当 `argsPreview` 无法生成时发送；同一 `callId` 合并窗口 200 ms、累计上限 4 KB，超限停发；终态事件之后不再发任何 args 事件。
- 终态事件（`Finished` / `Failed`）每个 `callId` 恰好一次，仍由 `TurnEmitter` 保证 seq 单调与代际丢弃。

### 2.6 D6 段边界沿用迭代，不新增机制

一个迭代 = 一条 assistant 行 = 一个气泡（F10 现状）。段内最小字段集：`text` + `reasoning` + `toolCalls(callId, name, status, result, compact)`。不引入第二份结果、不引入段级摘要。

### 2.7 D7 保留策略按资源模式分裂，不强行统一

采集侧上限（F5 的 `MAX_*`）取值互不相同**不是债**，是正确形态：条目型（`glob` / `grep` / `list`）需要条目计数，文本型（命令输出）需要字节预算，`read` 需要行窗口。强行统一会让一个计数同时表达三种语义。

在此基础上只补一件缺失的能力——**裁切方向**：

| 策略 | 语义 | 适用 |
|---|---|---|
| `head` | 保留前缀 | 条目型（`glob` / `grep` / `list`）、`read`（行窗口）——**现状即此** |
| `tail` | 保留后缀，需读到结束 | 未来命令执行类工具（错误栈、`exit` 前的末行）；当前无此工具，故为预留 |
| `headTail` | 稳定首尾，省略中段 | 大段日志 / 抓取正文（未来） |

约束：策略是**工具的声明**，不是框架的全局开关；`truncate_output`（F13）由单一 head 语义扩展为按 `OutputLimits` 选策略；预算归属与阈值不变（§3.3）。

### 2.8 D8 省略口径分裂：`truncated` 与 `incomplete` 不得混用

`truncated` 只表示「**因预算**省略了本已取得的内容」；其余「不完整」必须由工具以独立字段表达，不得写进截断提示：

| 状态 | 归属 | 示例 |
|---|---|---|
| `truncated` | 框架（tail 只由框架写） | 输出超字节/行数上限 |
| `skipped` | 工具 | 跳过二进制文件、不可读候选 |
| `incomplete` | 工具 | 搜索时间预算耗尽、提供方局部失败 |
| `decoding_failed` | 工具 | 编码探测失败、无效 UTF-8 |

理由：混用会让模型把「结果本身不完整」误读为「被预算砍了，再读一次就好」，从而重复调用同一工具。

### 2.9 D9 落盘生命周期：定位符结构化 + 年龄过期清理

- **定位符出进程**：`PersistedRef { path, bytes }` 随事件与 `compact` 下发，UI 与回放不再反解 `<persisted-output>`；文本标签退化为**模型侧检索提示**（模型仍需要知道「结果在哪、怎么读」）。
- **目录加命名空间**：`tool-results/<会话哈希>/<随机前缀>-<safe 名>`，与 `tool-results` 根解耦，使整会话产物可独立回收，也让 fork / 恢复的旧定位符仍然有效。
- **清理用年龄过期，不用引用计数**：启动时扫一次，删除 `mtime` 早于 `now − 保留天数` 的常规文件；`0` 表示禁用；不跟随符号链接、失败只 warn、绝不阻塞启动。**这是刻意的取舍**：会话本身有保留期，两者一起老化；为了一个可重新执行的工具结果去建引用索引，成本高于收益（§7.5 修正了本文早期版本「无索引无法 GC」的判断）。
- 清理不负责「旧定位符仍可用」的承诺：超过保留期的定位符**允许失效**，这必须写进契约，而不是靠 UI 兜底。

### 2.10 D10 文档形态渲染：以文档位置为轴，不以调用为轴

卡片与文档的差别不是皮肤，是**组织轴**：卡片以「一次调用」为容器，文档以「文档里的位置」为锚点。前者适合探索（`read` / `grep` / `glob` / 命令），后者适合创作（`write` / `edit`）——用户关心的是「第 40 行改成了什么」，不是「第 3 次调用干了什么」。

因此不设「文档模式专用渲染器」，而是**按锚点可得性分裂**（与 D7 同一条原则）：

| 条件 | 形态 |
|---|---|
| `compact.anchor != null` 且文档面板正打开该 `path` | 文档批注（gutter 标记 + 行内 diff / 区段高亮） |
| `compact.anchor != null` 但面板未打开该文件 | 卡片 + 「打开文件」入口（不静默切换面板） |
| `compact.anchor == null` | 卡片（现有 `ToolCallBubble`） |

三条硬约束：

1. **锚点只来自结果时刻的实际落点**，不得由 `args` 估算（`replace_all` 有多个落点，文件也可能在调用前后被改动）。
2. **锚点是位置，不是内容**。diff 行内容仍由 `result` 文本承载（模型要读），其格式已规范化（`- ` / `+ ` 前缀），文档渲染器从文本纯函数重建行级视觉，**不新增第二份 diff 字段**（F8 的教训：同一事实只存一处）。
3. **锚点无法纯函数重建**（文本里没有行号）→ 按 §2.3 判据必须随 `compact` 持久化；`write` / `read` 的锚点可由 `args` 重建，故为可选字段。

### 2.11 D11 切换点：按「能力」选路，不按 `role` / `modeId` 分叉，也不重写消息渲染

D10 说「按锚点分裂」，落点选在哪一层是个独立决策。现链路是 `MessageList` → `MessageBubble`（逐消息）→ `MessageContent` + `toolCalls.map(ToolCallBubble)`（逐调用）。四个候选 seam：

| seam | 影响面 | 判定 |
|---|---|---|
| 按 `role` 分叉（`MessageBubble` 顶部或 `MessageList` 过滤） | `role` 是**传输概念**（`user` / `assistant` / `tool` / `compact_summary`），不携带「这次调用有没有锚点」；工具卡片实际挂在 `assistant` 行内（F16），探索型与创作型调用共用同一角色，按角色切换会一并改掉 | 拒绝 |
| 按 `modeId` / 链路分叉（`resolveConversationRoute` → `TurnKind`） | `modeId` 是**会话级身份**，一个会话内既有 `read` / `grep` 也有 `edit` / `write`，形态必须逐调用变化，`modeId` 表达不了；且该字段已有唯一含义（策略选择的唯一判据，`turnStrategy.ts`），再挂一个渲染含义会让「同一字段两处语义」；历史消息自带 `modeId`（I43），中途换模式会让同一份数据在不同消息上渲染成不同形态 | 拒绝 |
| 整体重写消息渲染（会话即文档流） | 需重做用户气泡、reasoning、error / retry、`BranchPicker`、`EditBlock`、`MessageActions`、头像与流式光标，并失去消息级操作与分支树；收益仅是「换一种排版」 | 拒绝 |
| 单点替换工具调用的渲染函数（`MessageBubble` 里那段 `toolCalls.map`） | 影响面 = 一个渲染点；选路输入天然可用（`view.compact`）；上方所有消息级结构一行不动 | 采纳 |

五条规则：

1. **选路判据只有两个**：`compact.anchor != null`、文档面板是否正打开该 `path`。`role`、`modeId`、链路种类、工具名都不参与——工具名会随工具增删漂移，锚点才是稳定事实。
2. **`modeId` 的唯一合法渲染作用已在布局层发生**：链路决定「有没有工具 UI」（`AgentLayout` 注入工具插槽，`ChatLayout` 不注入），这是**存在性**判断，不是**形态**判断。本文不再往 `modeId` 上加任何形态含义；若将来出现 `docs` 模式，也按 §6 OQ8 不新增 `AgentModeKindDto` 值。
3. **两种形态是并列投影，不是互斥替换**。面板打开时，消息列表里的卡片**降级为单行引用**（`head` + 「见文档第 N 行」），而不是消失：否则会话历史出现空洞，分支切换与消息操作失去落点。
4. **不新增消息级容器**。批注组（同迭代多处 `edit` 归一组）由批注容器在渲染时聚合，`MessageList` 的 `role !== "tool"` 过滤、迭代分段（D6）均不变。
5. **「整个会话变成一篇文档」不属于本文**。那是工作区 / 文档面板的独立特性（渲染面是文档，不是消息列表），本文只定义「结果如何落到文档上」；若将来要做，它消费的仍是同一份 `compact`。

### 2.12 D12 turn 级密度：折叠已完成的中间迭代，不折叠结论

D10 / D11 解决的是「一次调用画成什么」（**轴 A：形态**）。G10 是另一条轴：**轴 B：容器数量**。用户看到的爆炸不是某张卡片太高，而是「N 个气泡 × 每气泡若干卡片」相乘——所以轴 B 必须单独处理，改卡片皮肤治不了它。

先厘清「模式决定单条还是一串」这句话：`modeId` 确实**相关**，但它是**代理量**，不是判据本身。

- chat 链路恒为单条，agent 链路**可以是单条**（一轮就答完、零工具），也可以是几十条；
- 若拿 `modeId` 当判据，就会把「agent 一轮答完」这种最常见的短对话也塞进折叠容器，反而多一层壳；
- 判据应该是**数据本身**：`turn` 内 assistant 段数、工具调用数。它天然覆盖两种情形（0–1 段 → 不折叠；≥2 段 → 折叠），且不需要给 `modeId` 增加第二语义（与 D11 规则 2 一致）。

**三档密度**（判据全为数据，逐 turn 计算）：

| 档 | 条件 | 渲染 |
|---|---|---|
| 单条 | turn 内 assistant 段数 = 1 | 现状不变：一个气泡 + 若干卡片（F19） |
| 折叠 | 段数 ≥ 2 **且该 turn 已结束** | 末段完整保留；**前面各段折叠为一个「运行组」**：一行组头（`N 步 · 耗时 · 失败数`）+ 每步一行 |
| 展开 | 用户点开组头，或该 turn 正在流式 | 恢复逐迭代气泡（D6 原样） |

五条硬规则：

1. **只折叠已完成 turn**。正在流式的 turn 恒为展开：折叠中途会让用户看不见进展，且组高在增量中反复跳动。
2. **末段永不折叠**。最后一段承载结论正文与最后一次调用，是用户真正要读的内容。
3. **折叠是渲染态，不是数据态**。不改 `agentSegments` 段表、不改落库、不改 D6 的气泡边界；展开后逐段消息、`id`、分支控件全部还原。因此折叠**不产生任何 wire 字段**——这是本决策最重要的约束，否则会退化成「UI 状态进历史」。
4. **失败 / 待处理的步不折叠**。`error` / `awaiting_user` 的步始终完整显示（含审批卡与提问卡）：折叠只能压成功步。安全阀优先于密度——把需要用户点确认的东西藏进折叠组是事故。
5. **单步一行只读 `compact.head` 与 `view` 元数据**，不读正文。这正是 D1 三段信封的直接收益：没有 `head`，压成一行只能靠截断正文前 N 字，既不准也不稳定（F19）。

单步行的字段映射：

| 元素 | 来源 | 缺失时 |
|---|---|---|
| 状态图标 | `view.status` | — |
| 工具名 | `view.name` | — |
| 对象 + 规模 + 成败 | `compact.head` | `compact == null`（旧数据）→ 退回 `summarizeArguments(view.arguments)`（现 `toolCallDisplay` 已有） |
| 耗时 | `view.startedAt` / `finishedAt` | 省略 |
| 结果规模 | `compact.truncated` / 正文行数 | 省略（**不得**为此再读正文，见规则 5） |

高度账（单步 ≈ 24 px）：

| 场景 | 现状 | 折叠后 |
|---|---|---|
| 8 迭代 × 2 调用 | ≈ 8 份气泡 chrome + 16 张卡片 ≈ 2000–2800 px | ≈ 组头 28 px + 16 步 × 24 px ≈ 410 px |
| 1 迭代 × 1 调用 | ≈ 1 气泡 + 1 卡片 | **不变**（不进折叠档） |

---

## 3 规范

### 3.1 文本信封语法

```
<head>                                  ← 恒为第 1 行，单行，无换行
<persisted-output ...>                  ← 仅落盘时，紧随 head（后接预览正文）
<body>                                  ← 工具正文，行数受回填预算
[truncated: ...]                        ← 仅 body 被收缩时
[encoding: ...]                         ← read 专用 footer（现状保留）
```

- head 语法：

```
ok  <tool> · <subject> · <scale>
err <kind> <tool> · <subject> · <reason>
```

示例：`ok read src/a.rs:1-120 · 120/120 lines · 3.2 KB`、`err business edit src/b.ts · anchor not found (3 candidates)`。

- head 约束：单行、≤ 160 字符（UTF-8 字节 ≤ 480）；`subject` ≤ 80 字符，超长按「首 40 + `…` + 尾 38」中间省略；`kind` ∈ `business | timeout | cancelled | internal | denied`（与 `ToolErrorKind` 对齐）。
- **head 只装模型下一步决策需要的字段**（成败、对象、规模、以及必要的进程事实如退出码 / 任务 id）。展示用的其余字段（耗时、字节明细、diff 全貌）走 `compact` 或持久化投影，**不得为塞进 head 而压缩语义**——head 一旦变成展示字段的容器，单行与 ≤160 字符约束就会被反复突破。
- tail 只由框架写，且只写 `truncated`（§2.8）；`skipped` / `incomplete` / `decoding_failed` 由工具写在 body 内，位置自定但必须在 head 的 `scale` 里可见其存在。
- 落盘标签保留现形态并补 `bytes`：`<persisted-output path="..." bytes="N">`。`bytes` 为新增属性，旧数据无该属性时前端按 `null` 处理；**该标签只服务模型检索，UI 不解析它**（UI 读 `compact.persisted`）。

### 3.2 工具 → head 映射

| tool | subject | scale |
|---|---|---|
| `read` | `path:start-end` | `returned/total lines · bytes` |
| `write` | `path` | `created` 或 `updated · +A/-B lines` |
| `edit` | `path` | `K replacements` |
| `grep` | `pattern @ path` | `H hits · F files` |
| `glob` | `pattern` | `N paths` |
| `list` | `path` | `N entries · depth D` |
| `mkdir` / `move` / `delete` | `path` | `created` / `moved` / `deleted` |
| `ask_user` | `question id` | `N options` |
| 未登记工具（MCP 等） | 参数中存在 `path` / `query` 则取其值，否则 `-` | 缺省时整段省略（不写 `-`） |

### 3.3 预算归属（分段，互不越界）

| 段 | 边界 | 常量 | 现值 | 归属 |
|---|---|---|---|---|
| 采集 | 单工具产出上限 | `MAX_*`（F5 各工具） | 不变 | 工具 |
| 采集策略 | 裁切方向（`head` / `tail` / `headTail`） | `OutputLimits`（新增 `strategy`） | 新增 | 工具声明 |
| 回填 | 单结果 50000 / 单轮 200000 / 预览 2000 字符 | `PER_TOOL_LIMIT_CHARS` / `PER_ROUND_LIMIT_CHARS` / `PREVIEW_CHARS` | 不变 | Pipeline |
| 信封 | head ≤ 160 字符 / `argsPreview` ≤ 200 字符 / `ArgsDelta` ≤ 4 KB·200 ms | 新增 | 新增 | 框架 |
| 展示 | 折叠阈值 | `RESULT_COLLAPSE_LINES` / `RESULT_COLLAPSE_BYTES` | 不变 | 前端 |
| 存储生命周期 | 落盘保留天数 | `RESULT_RETENTION_DAYS`（`0` 禁用） | 新增 | 存储后端 |

推论：单轮历史里内联正文合计 ≤ 200000 字符，落盘结果只回填 2000 字符预览 —— 会话加载体积因此有上界，不需要额外的「历史截断」规则。

### 3.4 wire 增量字段（草案，遵循 DTO 规则：输出 `Option` 键恒在、值 null）

```rust
pub struct ToolResultCompactDto {
    pub head: String,
    pub truncated: Option<TruncatedDto>,
    pub persisted: Option<PersistedResultDto>,
    pub error_kind: Option<ToolErrorKindDto>,
    pub anchor: Option<DocAnchorDto>,
}

pub enum TruncatedUnitDto { Lines, Items, Bytes, Unknown }

pub struct TruncatedDto {
    pub unit: TruncatedUnitDto,
    pub returned: Option<u64>,
    pub total: Option<u64>,
}

pub struct PersistedResultDto {
    pub path: String,
    pub bytes: Option<u64>,
}

pub struct DocAnchorDto {
    pub path: String,
    pub ranges: Vec<LineRangeDto>,
}

pub struct LineRangeDto {
    pub start: u64,
    pub end: u64,
}
```

挂载点（全部为**新增字段**，无删除）：

| 载体 | 新增字段 | 来源 |
|---|---|---|
| `AgentToolCallPayloadDto::Started` | `argsPreview: Option<String>` | 框架从参数抽取 |
| `AgentToolCallPayloadDto::Finished` | `compact: Option<ToolResultCompactDto>` | 框架渲染信封时同时产出 |
| `AgentToolCallPayloadDto::Failed` | `compact: Option<ToolResultCompactDto>` | 同上（`errorKind` 必填） |
| `MessageToolCallDto` | `compact: Option<ToolResultCompactDto>` | 读取历史时由后端解析器重建 |

`PersistedResultDto` 在**写入时**必须来自 `PersistedRef`（F12 已在 `persist` 阶段构造），**不得由解析文本产生**；`error_kind` 让 F11 的分类随 `compact` 一起进入历史（未引入 `compact` 的旧数据仍为 `null`，前端按 `status` 兜底）。`DocAnchorDto` 同属「算不出」的一类：`edit` 的锚点必须在**结果时刻**取（§3.7.1），不得由 `args` 估算。

历史重载的三条路径（按可用性择优，不改变门禁 8 的语义）：

| 数据 | 来源 | 说明 |
|---|---|---|
| 新写入 | 事件里的 `PersistedRef` | 唯一真源，路径与字节由存储后端返回 |
| 有持久化列的历史 | 该列 | 若将来引入（理由见 §4，只为展示保真） |
| 仅文本的历史 | 解析 `<persisted-output>` 标签 | 标签本身由 `PersistedRef` 渲染，round-trip 保证一致；`bytes` 缺失时为 `null` |

### 3.5 门禁

1. **字节相等**：`Finished.result` == 落库 tool 行 `content` == 历史 `MessageToolCallDto.result`。
2. **head 单行**：head 不含 `\n`，字符数 ≤ 160。
3. **三态一致**：文本无 `[truncated:` ⟺ `compact.truncated == null`；`unit == Unknown` ⟺ 文本含 `total unknown` 且 `returned`/`total` 为 null。
4. **round-trip**：`parse(render(x)) == x`，覆盖三态 × 落盘/不落盘 × 成败的组合。
5. **一次传输**：wire 上同一 `callId` 的正文只出现在 assistant 行；`role = "tool"` 行 `content` 为空串。
6. **事件**：终态唯一 + `seq` 单调（沿用 I36a 不变式，不新增规则）。
7. **口径分离**：`compact.truncated` 只反映预算省略；`skipped` / `incomplete` / `decoding_failed` 出现在 body 或 head 的 `scale` 中，但**不得**在 `[truncated:` 提示里出现。
8. **定位符非解析（写入路径）**：`Finished` 事件的 `compact.persisted` 与 `PersistedRef` 逐字段相等；测试禁止用正则从 `result` 生成写入路径的 `persisted`。仅文本历史的读取回退（§3.4 第三行）不受此约束，但其结果必须与 round-trip 一致。
9. **分类可回放**：`Failed` 事件写入历史后，重载得到相同的 `compact.errorKind`。
10. **清理安全**：清理只按 `mtime` 删除常规文件、不跟随符号链接、失败不抛出、`0` 时完全禁用。
11. **锚点结果时刻**：`edit` 的 `compact.anchor.ranges` 等于实际写入行区间；测试须覆盖「调用期间文件被外部改动」，证明锚点不是由 `args` 估算。
12. **形态不越界**：文档形态只读 `compact`（`result` 文本仅用于 §3.7.2 的 diff 行重建）；不得为文档形态新增第二套 wire 字段。
13. **选路不看身份**：`ToolCallRenderer` 的选路逻辑不得读 `role` / `modeId` / `TurnKind` / 工具名；测试须覆盖「同一 `assistant` 行内一个调用有锚点、一个没有」，两者分别走批注引用与卡片。
14. **折叠不改数据**：运行组只由渲染层聚合，`agentSegments` 段表、落库行、wire 字段均不因折叠而变化；门禁要求「展开后消息列表与折叠前逐项一致」。且折叠判据只读段数与 `status`，不得读 `modeId`。
15. **安全阀**：`error` / `awaiting_user` 步、末段、正在流式的 run 一律不折叠；测试须覆盖这四种反例。

### 3.6 迁移与兼容

- 后端：新增 `src-tauri/crates/assistant-tools/src/compact.rs`（`render` / `parse` / 信封预算常量）；各工具改为填字段而非拼句；`truncation_notice` 的两分支收敛为三态。
- 采集策略：`os/output.rs` 的 `OutputLimits` 增加 `strategy`（`head` / `tail` / `headTail`），`truncate_output` 按策略选保留区间；现有调用方默认 `head`，行为不变。
- 落盘：`storage.rs` 的 `FileToolResultStorage` 改目录布局（会话哈希 + 随机前缀），并新增启动年龄清理；`pipeline.rs` 的 `report` 把 `persisted` 送进事件，不再丢弃。
- 契约：增量字段合入后跑 `npm run contract:gen`，提交生成物；`conversation_utils` 注入 `compact` 并清空 tool 行正文。
- 前端：`ToolCallBubble` 读 `view.compact`；`parsePersistedOutput` / `resultStats` 降级为 `compact == null` 时的兜底；`toolCallView` 在 `started` 用 `argsPreview` 初始化参数。
- 旧数据：无 `compact` 的历史由后端解析器重建（同一语法）；无法重建时只渲染 `result`。落盘信息对旧数据只能走文本兜底，**不承诺** `persisted.bytes`。
- 注释修正：`db.rs` 中 `tool_calls_json` 的「当前写入恒 NULL」已与实现不符（F15），随本次改动一并更新，避免后人按注释误判该列无用。

### 3.7 文档形态渲染（D10 的落地口径）

文档形态不是第二套信封，而是**同一 `compact` 的另一种消费形态**。渲染器只读 `compact`，禁止读 `result` 文本猜结构（唯一例外是 §3.7.2 的 diff 行重建）。

#### 3.7.1 锚点字段

```rust
pub struct DocAnchorDto {
    pub path: String,
    pub ranges: Vec<LineRangeDto>,
}

pub struct LineRangeDto {
    pub start: u64,
    pub end: u64,
}
```

挂到 `ToolResultCompactDto.anchor: Option<DocAnchorDto>`。`ranges` 为向量而非单区间，因为 `replace_all` 的落点是多个（与 F14 同源）。

| tool | 锚点来源 | 是否持久化 |
|---|---|---|
| `edit` | 结果时刻命中的行区间（`line_index_at` 已算出） | **是**（文本里无行号，不可重建） |
| `write` | 整文件 `[1, 总行数]` | 否（可由 `args` + 结果行数重建） |
| `read` | 请求窗口 `[start, end]` | 否（可由 `args` + `returned` 重建） |
| `grep` / `glob` / `list` / 命令 | 无（多文件、无位置语义） | — |

#### 3.7.2 字段 → 视觉映射

| 视觉元素 | 读自 | 缺失时 |
|---|---|---|
| 批注标题 | `head`（单行，已含对象 / 规模 / 成败） | 无锚点时整卡走 `ToolCallBubble` |
| 行级 diff 视觉 | `result` 文本的 `- ` / `+ ` 行（规范化格式，纯函数重建） | 无 diff 行 → 区段高亮 |
| 区段高亮范围 | `anchor.ranges` | `anchor == null` → 卡片 |
| 「查看全文」 | `persisted`（`path` + `bytes`） | `persisted == null` → 不显示入口 |
| 失败批注样式 | `errorKind` | `null` → 按 `status` 兜底 |
| 规模统计 | `truncated`（三态） | `null` → 不显示 |

#### 3.7.3 与迭代分段的关系

两个渲染面并存，各自保留分段信息：

- **文档批注面**（文档面板 gutter）：没有气泡，但保留迭代分组——一次迭代的多处 `edit` 归为同一批注组（同色标记、可整组收起），跨迭代按 `iteration` 分色。这是 D6 在文档形态下的等价物：分段信息仍在，容器换了。
- **消息列表面**（`MessageList`）：完全不变，仍是逐迭代气泡；卡片按 D11 规则 2 降级为单行引用。

#### 3.7.4 渲染切换点（D11 的落地）

```tsx
// MessageBubble 内，唯一改动点
{message.toolCalls?.map((view) => (
  <ToolCallRenderer key={view.callId} view={view} docPanelPath={docPanelPath} />
))}
```

`ToolCallRenderer` 只做选路：`view.compact?.anchor != null && anchor.path === docPanelPath` → `ToolCallDocStub`（单行引用）；否则 → `ToolCallBubble`。它不读 `role`、不读 `modeId` / `TurnKind`、不读工具名，因此 `ChatLayout` 与 `AgentLayout` 走同一段代码且行为一致（chat 链路无锚点，恒走卡片）。

`docPanelPath` 由布局插槽注入（`AgentLayoutSlots` 已有插槽位），`ChatLayout` 不传即 `null`，无需分叉。

#### 3.7.5 不做的部分

- 不定义文档编辑器本体（保存、版本、协作批注）——本文只管「工具结果如何落到文档上」。
- 不定义面板开合与布局（归 P12 工作区与后续 UI 迭代）。
- 不做「跨会话的文档批注持久化」：批注是 `compact` 的渲染态，随会话历史回放，不单独存储。
- 不做「会话即文档」的整体重排（D11 规则 4）。

### 3.8 turn 级密度（D12 的落地口径）

#### 3.8.1 分组依据

**不改段表，只在渲染时聚合**。`MessageList` 已能识别「连续 assistant 段」的起点（`showIdentity` 用的就是这个判据，F18），运行组复用同一判据：连续 `role === "assistant"` 的消息 = 一个 turn 的迭代段。

```tsx
// MessageList 内，逐消息渲染前计算，不改现有 map
const groups = useMemo(() => groupAssistantRuns(visibleMessages), [visibleMessages]);
```

`groupAssistantRuns` 为纯函数：输入消息列表，输出 `{ kind: "message", msg }` 或 `{ kind: "run", segments: Message[] }`。**不读 `modeId`**，chat 链路恒为单段 run 且段数 = 1，自然不进折叠档。

#### 3.8.2 折叠判据

```
可折叠 = segments.length >= 2
      && 该 run 已结束（最后一段 status !== "streaming"）
      && 该 run 不是最后一轮（或虽是最后一轮但已结束）
展开态 = 用户手动展开 || 该 run 正在流式
```

三档（与 §2.12 表一致）：`length === 1` → 单条；否则折叠；用户展开 → 逐段还原。

#### 3.8.3 运行组结构

```
┌ 运行组 12 步 · 8.4s · 1 失败                    [展开]   ← 组头，一行
├ ✓ read  src/a.ts · 120 行 · 40ms                        ← 步行，一行
├ ✓ grep  "compact" · 12 命中 · 90ms
├ ✓ edit  src/b.ts · 第 40-52 行 · 13 行
└ ✗ bash  npm test · 失败                                 ← 失败步：整卡展开，不压行
```

- 组头字段：步数（= 调用数）、总耗时（首段 `timestamp` → 末段结束）、失败数；**全部可由 `view` 与 `message` 算出**，无新字段。
- 步行点击 → 就地展开该步的完整卡片（`ToolCallBubble`），而不是展开整组。
- 末段不进组：组只包**中间段**，末段按现状单独渲染（含正文与卡片）。

#### 3.8.4 与其它决策的边界

| 关系 | 结论 |
|---|---|
| 与 D6（一迭代一气泡） | 气泡边界仍是数据模型；折叠只是渲染态，展开即还原（规则 3） |
| 与 D10（文档批注） | 轴 A 与轴 B 正交。折叠组里的步行，若 `anchor != null` 且面板打开，仍走 `ToolCallDocStub`（单行引用），形态不变 |
| 与 `compact == null`（旧数据） | 步行退化为 `summarizeArguments`；若连参数也没有，则**不折叠该段**（宁可高，不可空） |
| 与审批 / 提问 | `awaiting_user` 步不折叠（规则 4），且组头显式提示「有 N 步待处理」 |

#### 3.8.5 不做的部分

- 不做跨 turn 的全局折叠（「整个会话只有结论」）——那会让用户找不到中间过程，且与分支回放冲突。
- 不做自动摘要（把 16 步合成一句自然语言）：摘要与历史文本分叉，且需要额外模型调用。
- 不做持久化折叠状态（哪个组被展开过）：属 UI 本地偏好，不进历史、不进 wire。

---

## 4 备选方案（不采纳理由）

| 方案 | 不采纳理由 |
|---|---|
| 只保留 `data`，UI 自行渲染（参考项目路线） | 领域值需持久化才能回放；我们的历史回放必须自足，不能依赖进程内值 |
| 只保留文本，前端继续解析 | 即 G3：格式调整会静默降级，且解析器与渲染逻辑分散在 UI 层 |
| 落库只存摘要，全量只留落盘文件 | 崩溃恢复与压缩投影需要正文；且回填预算已由 Pipeline 控制，无需再省 |
| 给 `MessageToolCallDto.result` 加 2 KB 预算 | 破坏门禁 1 的字节相等：历史与模型所见分叉，回放与续聊不一致 |
| 直接删除 `ArgsDelta` | MCP 工具无法提供 `argsPreview` 时 UI 完全看不到参数；保留为受控回退通道更稳 |
| 持久化规范值 `data`（参考项目也拒绝） | 值需要程序化消费者才有意义；我们的回放必须自足，持久化投影（`compact`）已足够，多存一份会带来两份结果互相矛盾的经典问题 |
| 给 `messages` 加落盘引用列，用引用计数驱动 GC | 见 §7.5：年龄过期即可获得有界生命周期，引用索引的成本（写入时序、fork/恢复继承、并发删除）高于收益。该列若将来引入，理由是「展示保真」，不是「可清理」 |
| 重写会话日志为事件溯源 + zstd 帧 | 该形态由参考项目「`seq = log.length`、仅追加、崩溃按帧起点修复」约束驱动；我们是 DB 行模型 + 事务，重写等于换存储引擎，收益不抵风险 |
| 用周期性定时器清理落盘文件 | 引入定时器生命周期、重叠控制与又一个配置旋钮；启动扫一次即可，长期运行进程的重启本来就是边界 |
| 在会话删除时级联删落盘文件 | 持久会话、恢复与 fork 仍可能引用旧定位符；按文件年龄过期能保住新写入，也不会误删 |
| 文档形态复用 `ToolCallBubble` 换皮肤 | 组织轴不同：卡片以调用为容器，文档以位置为锚点；换皮肤只会得到「把 diff 塞进小卡」的形态，锚点信息仍然浪费 |
| 为文档形态单开一套 wire（文档块 / 批注 DTO） | 会变成第二个结果通道，与 D3「`compact` 是唯一展示投影」直接冲突；锚点作为 `compact` 的可选字段即可 |
| 锚点由 `args` 估算（省一次持久化） | `replace_all` 有多个落点、文件可能在调用期间变化；估算出来的锚点会把批注画到错误行，且无法被测试证伪 |
| 按 `modeId` 分叉渲染形态（如「`docs` 模式才用批注」） | `modeId` 是会话级身份且已被 `turnStrategy` 独占为策略判据；同一会话内探索与创作调用混合，形态必须逐调用变化。把形态写进模式会让「换模式 = 历史消息换样子」，并迫使新增 `AgentModeKindDto` 值 |
| 用 `TurnKind`（`chat` / `agent`）分叉形态 | 链路只该决定「有没有工具 UI」（布局层插槽），不该决定「工具长什么样」；同一个 `agent` 链路里形态仍须逐调用变化 |
| 用 `modeId` 决定是否折叠（「chat 单条、agent 折叠」） | `modeId` 只是**代理量**：agent 链路也会一轮答完（段数 = 1），按模式折叠会给最常见的短对话多加一层壳；判据取段数即可，且不必给 `modeId` 加第二语义（D12） |
| 把所有迭代（含末段）一起折叠成「结论 + 摘要」 | 用户要读的正是末段的结论正文；把结论也折进去等于让用户多点一次才能看到答案 |
| 把失败 / 待审批步一起压成一行 | 安全阀优先：需要用户点确认的步被藏进折叠组是事故（D12 规则 4） |
| 持久化「哪个组被展开过」 | UI 本地偏好，不是会话事实；进历史会与分支 / fork 产生不一致状态 |

---

## 5 影响

- **契约**：4 个新增字段（`Started.argsPreview`、`Finished.compact`、`Failed.compact`、`MessageToolCallDto.compact`），无删除；旧前端忽略新字段仍可工作（`result` 未变）。`compact` 内部新增 `errorKind`、`persisted.bytes` 与 `anchor`，同为可选。
- **后端**：信封拼装集中到 `compact.rs`；Pipeline 落盘与 `report` 顺序不变（F4 已保证事件载荷有界）；`report` 不再丢弃 `persisted`；`conversation_utils` 负责注入 `compact` 与空正文 tool 行。
- **存储**：落盘目录改为会话命名空间 + 随机前缀，新增启动年龄清理（默认保留天数可配、`0` 禁用）；旧目录布局是否迁移见 §6 OQ7（当前倾向「不迁移、旧定位符允许失效」）。
- **前端**：`ToolCallBubble` 不再依赖字符串约定；`MessageList` 的 `role !== "tool"` 过滤从「功能依赖」降为「防御性过滤」。文档形态新增 `ToolCallDocAnnotation`（读 `compact.anchor` + `compact.head`），与 `ToolCallBubble` 并列，由 `anchor != null` 与「面板是否打开该 `path`」两个条件选路；`toolCallDisplay.ts` 的 `parsePersistedOutput` / `resultStats` 只保留为 `compact == null` 的兜底。
- **前端·密度**：`MessageList` 新增 `groupAssistantRuns` 纯函数与运行组渲染（组头 + 步行），`MessageBubble` 与段表不改；步行复用 `ToolCallBubble` 的头部组件（`StatusIcon` / 状态文案 / 耗时格式化），不新造一套视觉；失败 / `awaiting_user` 步与末段不参与折叠。
- **测试**：round-trip、字节相等、三态一致、口径分离、定位符非解析、分类可回放、`argsPreview` 上限与 `ArgsDelta` 合并、tool 行空正文、清理安全性、锚点结果时刻、形态选路（有/无锚点 × 面板开/关）各一组；密度侧另加 `groupAssistantRuns` 纯函数用例（1 段不折叠 / ≥2 段折叠 / 流式中不折叠 / 失败步不折叠 / `compact == null` 退化）；原有 pipeline 落盘测试保留。
- **不做**：turn 事件语义、压缩投影、审批链、权限网关、分支链拓扑、事件溯源日志重写、`data` 跨 seam、跨 turn 全局折叠。

## 6 未决问题

1. `argsPreview` 的抽取口径：按 JSON Schema `required` 取前 N 个字段，还是按工具显式声明 `preview_keys`？前者零成本、后者更准。
2. MCP 工具无 `subject` 时的 head 是否允许整段省略，还是强制 `-` 占位（影响 UI 标题布局）。
3. `Failed` 是否也需要 body 预算（当前 reason 单行，是否需要多行诊断）。
4. `edit` 的 diff 保真度（F14）：是否升级为**结果时刻** applied-hunk diff（带 ±3 行上下文、`replace_all` 每处一个 hunk）。若升级，该 diff **不可**由 `(args, 文本)` 重建，按 §2.3 判据必须持久化投影，不能塞进 head。
5. 落盘保留天数的默认值：参考项目取 30 天；我们需要按会话保留期对齐，还是独立取值？
6. 清理是否需要按「会话命名空间目录」整体修剪空目录（当前只删文件、保留空目录）。
7. 旧目录布局（`tool-results/<call_id>.txt`）是否需要一次性迁移，还是直接按失效处理。
8. 文档形态是否由 `mode` 声明？当前 `AgentModeKindDto` 只有 `chat` / `agent` 两值（`protocol/dto/settings.rs`），没有 `docs`；倾向**不新增 kind**——形态由「锚点可得 + 面板状态」在渲染层决定，避免把一个渲染选择写进会话身份。
9. 锚点坐标系：`edit` 锚点用**文件行号**（与 `read` 的 `{line_number}` 一致），但文档面板若做过 markdown 渲染，展示行号与文件行号不同。需要确认面板是否提供行号映射，否则批注只能按「段落/块」定位。
10. 一次调用跨多文件（`grep` 命中多文件、未来批量编辑）时 `DocAnchorDto.path` 单值不够；是否改为 `anchors: Vec<DocAnchorDto>`。当前倾向先保持单值 + 多文件走卡片，等出现真实多文件编辑工具再扩。
11. 折叠阈值的默认取值：段数 ≥ 2 即折叠，还是 ≥ 3（或「总步数 ≥ 6」）才折叠？2 段折叠会让「读一个文件再改一处」这种两段 turn 也变成组，可能过度收纳；倾向按**步数**阈值（如总步数 ≥ 5）而非段数。
12. 运行组是否允许「整组一次性展开」的快捷操作（当前设计只有组头展开 + 步行单独展开）；若加上，需确认与消息级操作（删除 / 分支）的冲突边界。
13. 流式 turn 结束后是否自动收起中间段（当前倾向「不自动收起」：刚结束的 turn 保持展开，避免内容在用户眼前突然塌缩）。

---

## 7 参考项目研究结论（DSH）

本节记录对参考项目（DeepSeek Harness `architecture` notes）的核对结果，作为上面各决策的依据来源；不重复描述其实现细节，只记「采纳 / 拒绝 / 自研」的判定与理由。

### 7.1 采纳（构成本文决策的依据）

| 经验 | 参考项目出处 | 落点 |
|---|---|---|
| 规范值 + 两个投影（模型文本、展示载荷），拒绝「工具分别返回值和内容」 | 规范工具输出契约 | §2.3 三概念表 |
| 省略三态 `none / exact / unknown`，库只产出 `none`/`exact`，`unknown` 留给无法计数的调用方 | 工具结果保留库 | §2.2 D2 |
| `truncated` 只表示「因预算省略本已取得的内容」，`incomplete` / 跳过 / 解码失败是工具自己的字段 | 同上 | §2.8 D8 |
| 保留抽象按资源模式分裂（条目计数 vs 文本字节 vs 行窗口），不为统一而统一 | 同上 | §2.7 D7 |
| 文本保留需支持 `head` / `tail` / `headTail`，并按 UTF-8 边界裁切 | 同上 | §2.7 D7 |
| 展示载荷必须是 `args + 结果` 的纯函数、可回放、无 I/O | 结果时刻 applied-hunk diff | §2.3 重建判据 |
| 结构化错误：`code` 与 `message` 分离，结构化字段跨 seam 保留并进历史，模型只看文本 | 结构化错误分类 | §2.3 / §3.4 `errorKind` |
| 存储 seam 只负责「原样存全文 + 返回定位符 + 检索提示」，预览与替换策略归消费方 | 工具输出 spill 策略 | §2.9 D9 |
| 定位符是不透明值，消费方不解析，只配合 `retrievalHint` 渲染 | spill 子系统 | §2.9 / §3.1 |
| 落盘清理用启动时 mtime 年龄扫描，不用定时器、不绑会话删除；接受「超期定位符失效」 | 本地 spill 启动清理 | §2.9 / §5 |

### 7.2 拒绝

| 参考项目做法 | 拒绝理由 |
|---|---|
| 每个工具声明 `output.schema` + 运行时校验 + 深冻结 | 我们已有 Rust 强类型 + serde，再加 schema 会成为第三份定义；只取「单一拼装点」这一半 |
| 结果归一等 tool-role message，结果块属于该 message | 形态相反：我们的 wire 结果挂在 assistant 行 `toolCalls[].result`（§2.4 D4），原因是前端按 assistant 行渲染卡片、tool 行只承担分支拓扑 |
| 事件溯源会话日志 + zstd 帧 + 按帧起点崩溃修复 | DB 行模型 + 事务已有等价能力，重写存储引擎收益不抵风险 |
| 周期性定时器清理 / 会话 dispose 时删除 spill | 见 §4 |
| 持久化规范值本身 | 见 §4 |

### 7.3 参考项目未覆盖、由本项目自研

1. **结果在 wire 上出现两遍**（F8）：参考项目结果只有一处，天然无此问题；我们是「assistant 行带 `tool_calls_json` + tool 行带 `content`」的历史形态遗留，靠 D4 消除。
2. **流内参数通道无界**（F9）：参考项目的参数是 `tool-call` content block，没有增量参数通道；只借其「增量流与最终结果不同通道」的边界划分，节流与上限自研（D5）。
3. **前端反解文本**（F7）：参考项目用 slot + 结构化载荷，不做文本解析；我们的解析器集中到后端一处（D3）。
4. **未欠的债**：参考项目拆掉 per-tool 视图注册表（`toolview-dissolution`）；我们前端只有 `ToolCallBubble` 一个渲染点，没有该债。
5. **文档形态渲染**（D10）：参考项目的展示载荷是「结果块 + slot」，按调用组织，没有「以文档位置为轴」的形态；锚点分裂、面板选路、迭代分组三条规则由本项目自研。它唯一可借的是「展示载荷必须可回放」——我们的 `anchor` 因此必须持久化，不能靠 `args` 估算。
6. **turn 级密度折叠**（D12）：参考项目没有「一迭代一气泡」这个前提（它的结果归 message、按内容块组织），因此不存在「N 个气泡 × 卡片」的高度乘法；我们的段表模型（F10/F18）决定了必须单独处理容器数量。可借的只有一条边界原则：**折叠属渲染投影，不得改变持久化事实**——与其「展示载荷可回放」同源。

### 7.4 跨 seam 规范值通道的引入条件（任一满足才引入）

1. **code-mode / 程序化调用**：工具在代码沙箱里被生成代码调用，结果被当 JSON 消费。
2. **链式编排**：一个工具的输出直接作为另一个工具的输入，中间不做文本往返。
3. **结构化剪枝**：压缩按结构裁剪工具结果，而不是按字符数。
4. **UI 需要文本装不下的全保真值**（典型：结果时刻 applied-hunk diff）。

四条的共同点是**消费者不是模型**。触发任一条时，跨 seam 的也应是**持久化投影**，不是值本身。

### 7.5 对本文早期结论的修正

- 早期版本称「没有引用索引就无法安全清理落盘文件」，**该判断已被参考项目证伪**：年龄过期即可获得有界生命周期。引用索引若要引入，理由是展示保真（回放不依赖正则、可显示字节），与可清理性无关。这两件事必须分开记账。
- 早期版本把 `compact` 称作「UI 唯一解析来源」而未说明它对应哪个通道；现已明确 `compact ≡ 展示投影`，并显式声明不引入跨 seam 的规范值（§2.3）。
- 早期版本未让 `errorKind` 进入历史，导致失败卡片无法按分类路由；现已并入 `compact`（§3.4）。
# 模型级编排与后台产物作业：可行性分析

> 日期：2026-11-19（初稿）｜2026-11-19（增补 §4.14 内核接管边界、G11/G12、D9、A13/A14）｜性质：**预先分析**（只判定「模型级编排能不能做、做成什么形状、哪些口径必须先定案」；不含实施步骤与排期）
> 判据来源：`docs/principle/CODE-REVIEW-PRINCIPLE.md`、`docs/principle/CODE-SPLIT-PRINCIPLE.md`
> 事实基线：仓库当前代码（2026-11-19）；`docs/analysis/AGENT-WORKFLOW-ORCHESTRATION-ANALYSIS.md`（L1/L2/L3 三层粒度、结论 11「节点只能取三语义单元」）、`docs/store/I38-agent-roadmap.md` §2/§2.1、`docs/store/I36a-agent-kernel-baseline.md`、`docs/store/I38f-agent-subagent-spec.md`、`docs/store/I52-ext-background-task.md`（未立项）、I55 观测规格（已归档）
> 参考实现：`references/9-workflow-orchestration/` 的稀疏检出——ComfyUI（`nodes.py`、`comfy_execution/`）、n8n（`packages/@n8n/api-types/src/agents/`）、langgraph、Dify。凡标「参考」的形态判断只出自上述检出部分，不外推到未检出的部分。
> 范围界定：本文主体是**「模型级」执行体的结构可行性**——模型调用能不能成为编排对象、除 turn 之外还需要几种执行体、每种执行体的定义性特征是什么、它们与既有内核 / 观测面 / 审批 / 预算 / 取消 / 落库的边界在哪。**不含**逐家 provider 的端点适配细节、**不含**画布与图编辑器、**不含**排期。
> 范围例外：§7 的取舍表决定 DTO 字段与事件名，必须与结构一并定案。
> 引用已归档规格时只写「文档名 +（已归档）」，不给路径。

---

## 0 结论速览

| # | 命题 | 判定 | 依据 |
|---|---|---|---|
| 1 | 「模型级编排」= 在图里新增一种「模型调用节点」 | **不成立** | 既有编排分析结论 11 已判定节点只能取 turn / 工具调用 / 会话三语义单元；模型调用是 turn 的内部实现步骤（§4.1） |
| 2 | 「模型级编排」的实质是「换执行体」，不是「加节点」 | **成立** | 需求（伴生 / 压缩 / 嵌入 / 周期注入 / 后台产物）问的是「谁在跑这段模型调用、结果回哪里」，而非「图里多一个框」（§1.1、§4.1） |
| 3 | 模型调用已有独立的最小面 | **成立** | `BaseProviderAdapter` 只有 `chat_completion_stream` / `list_models` 两个方法，与 turn 无耦合，可被非 turn 执行体直接调用（§2 M1、§4.4） |
| 4 | 同步模型单元可以绕开 turn 独立执行 | **不成立** | turn 是唯一同时具备生命周期、取消、预算、观测、落库五件事的单元；绕开即五项全缺（§4.2） |
| 5 | 压缩是「turn 外模型调用」的完整先例，可直接推广 | **部分成立** | 它复用 `TurnTasks` 的会话键、写 usage、写消息行，形态可推广；但它硬编码 `temperature`、用会话主模型、以 `OnBusy::Replace` 抢键，三点都不能照搬（§2 M9、§4.2） |
| 6 | 模型目录已能表达「图像输出模型」 | **部分成立** | `models-dev.json` 里有 `output: ["text","image"]` 的模型（如 `gemini-2.5-flash-image`），但 Rust 侧 `StoredModelEntry` 只解析输入模态，输出模态被丢弃（§2 M3、§4.5） |
| 7 | 现有链路能取到图像输出 | **不成立** | 适配器解析结构与 `UnifiedResponse` 都只有 text / reasoning / tool_call；Gemini 的 `inlineData` 未解析，图像部分被静默丢弃（§2 M4、§4.6） |
| 8 | 图像产物可以直接用远端 URL | **不成立** | 远端 URL 有有效期，会话是长期事实；产物必须落 `AttachmentStore`（§4.7） |
| 9 | 产物可以当普通文本消息存 | **不成立** | 产物是附件（内容寻址），消息只持引用；wire 与渲染侧已支持 `Image` 块（§2 M11/M12、§4.7） |
| 10 | 伴生通道可以复用会话并发键 | **不成立** | `TurnTasks` 的键是 `conversation_id`，语义是「单会话单 turn 互斥」；伴生与后台必须走独立键，否则主对话被侧车锁死（§4.9） |
| 11 | 周期提示词注入可以做成一种执行体 | **不成立** | 它不产生模型调用、不产生产物、不占预算，只改「下一次投影的输入」；做成执行体会得到空壳（§4.8） |
| 12 | 后台作业 = P9 后台任务 | **不成立** | I52-Ext（未立项）的后台任务定义性特征是「进程持有者」，判据是「任务结束 = 进程树回收」；模型型作业没有进程树（§4.10） |
| 13 | 后台作业必须复用 P9 的任务台账语义 | **成立** | 任务 id / 状态机 / 轮询 / 终态事件四件事必须共用一套语义，否则前端出现两套「运行中」；但持有者不同（§4.10、§9） |
| 14 | 无会话的模型调用可以存在 | **不成立（在当前归因体系下）** | usage 归因、附件归属、观测 key、审批键都挂在会话上；无会话调用无处记账（§4.11） |
| 15 | 模型级编排不改观测面 | **成立** | I55 的只读订阅与独立控制端口足以承载；新增的只是事件种类，不是观测面结构（§4.12） |
| 16 | 模型级编排可以不等 P8.1 / P8.2 | **部分成立** | 同步单元与伴生通道可以不等（它们是 turn 内的旁路）；后台作业与跨会话调度必须等 P9 / P8.2（§9） |
| 17 | 只做「伴生压缩」也有独立价值 | **成立** | 它解决「压缩模型与主模型解耦」这一现实痛点，且是既有硬编码的直接替代（§4.13） |
| 18 | 技术上可行 | **成立** | 缺的是四件基础设施——独立并发键、非文本输出通道、产物落库链路、任务台账；不缺架构（§6） |
| 19 | 内核应当接管更多底层安排 | **部分成立** | 该接管的是「并发键与取消的语义泛化」（Companion 必须进内核）；不该接管的是「任务台账与产物链路」（Job 必须不进，违反 I36a 判据 2）（§4.14） |
| 20 | 新增执行体种类必须动内核 | **不成立** | `TurnSpec.kind` 是开放字符串（M18），K7「链路差异只经 `TurnSpec` 表达」已容得下；只有「键的语义范围」必须动内核（§4.14） |

一句话：**「模型级编排」不是往图里加「模型调用节点」，而是把「谁执行这段模型调用、结果回到哪里」从 turn 内核里抽出来，做成三种有定义性特征的执行体——同步单元（走 turn，复用全套）、伴生通道（无产物、有结论、独立键）、后台作业（有产物、可失败、跨 turn 存活）——外加一种不算执行体的形态（注入钩子，只改投影输入）。可行，但必须先补齐四件基础设施，并且必须先承认一个事实：现有 provider 链路是「输入可多模态、输出只有文本」，后台图像生成不是加一个节点，而是新增一条输出通道。**

---

## 1 边界

### 1.1 「模型级」的两种读法

「模型级编排」在讨论里同时指向两种完全不同的东西，混谈必然得出错误结论：

| 读法 | 含义 | 判定 | 为什么 |
|---|---|---|---|
| **A 粒度下探** | 把「一次模型调用」当成图里的节点，画布上出现 `model-call` 框，可连线、可配参数 | **不成立** | 模型调用是 turn 内部的实现步骤（迭代内的一次请求）；把它做成节点，等于把中间件顺序、预算切分位置写进 wire，与既有编排分析结论 11 同源冲突 |
| **B 编排对象** | 编排的对象是**模型**而非 turn：主模型、伴生模型、压缩模型、嵌入模型、图像模型各司其职，编排决定「谁在什么时机被调用、结果流向哪里」 | **成立，本文定义域** | 它不改变 turn 的内部结构，只新增「执行体种类」；每一种执行体仍是一个完整生命周期单元 |

**对 B 的关键限定**：编排对象是「模型调用这件事」，但**承载单元仍是执行体**，且执行体种类是**有限闭集**（§1.3），不是任意可配的组合。把「执行体可配」误读成「节点可配」，就会滑回读法 A。

### 1.2 与既有编排分析的关系

既有编排分析把粒度分成 L1（turn 内）/ L2（turn 之间）/ L3（会话之上），并判定「编排只能落在 L2 / L3，turn 是最小可用单元」。本文**不推翻**这条，而是回答它留下的两个缺口：

| 缺口 | 既有分析的边界 | 本文的回答 |
|---|---|---|
| 需求要求「比 turn 更细」的单元（一次旁路模型调用） | L1 不可编排，L2 最小单元是 turn | 不把单元变小，而是新增**伴生通道**：turn 内的旁路执行体，有独立键、独立结论，但不占主 turn |
| 需求要求「比 turn 更长」的单元（跨 turn 的产物） | L2 的单元生命周期 ≤ turn | 新增**后台作业**：独立台账、产物落库、可跨 turn 存活 |

因此本文与既有分析是**补集关系**，不是替代关系：既有分析回答「turn 之间怎么连」，本文回答「turn 之外还有哪几种执行体」。

### 1.3 四种执行体（本文定义域）

| 形态 | 定义性特征 | 有产物 | 占主 turn 键 | 生命周期 | 对应需求 |
|---|---|---|---|---|---|
| **Unit 同步单元** | 在 turn 内执行模型调用，结果回灌 turn 上下文 | 否（回灌上下文） | 是（本就是 turn） | ≤ turn | 主模型、子代理、条件路由 |
| **Companion 伴生通道** | 与主 turn 并行执行，结论回到会话但**不阻塞主 turn** | 否 | **否** | ≤ turn | 伴生压缩、旁路校验、嵌入索引 |
| **Job 后台作业** | 有**产物**、可失败、可跨 turn 存活 | **是** | **否** | > turn | 图像生成、长文档产物 |
| **Hook 注入钩子** | 不加执行体，只改「下一次投影的输入」 | 否 | 是（属主 turn） | 会话级 | 周期性固定提示词注入 |

**判定：前三者是执行体，必须新增基础设施；Hook 不是执行体**（它落在投影层，见 §4.8）。

### 1.4 不属于本文

- 逐家 provider 的图像 / 嵌入端点字段适配（本文只定「必须有一条输出通道」这个结论，不定字段）
- 画布、图编辑器、run 台账 schema（属既有编排分析的范围）
- 多模态产物在 UI 的交互设计（只引用现有渲染能力，见 §2 M7）

---

## 2 代码事实基线

以下事实均取自当前代码，是后续所有命题的依据。

| # | 事实 | 位置 | 对本文的含义 |
|---|---|---|---|
| **M1** | provider 层唯一端口是 `BaseProviderAdapter { chat_completion_stream, list_models }` | `src-tauri/crates/assistant-providers/src/base.rs` | 没有「生成端点」抽象；模型调用的最小面就是「配置 + key + 消息 + 模型 + 选项 + 取消令牌」 |
| **M2** | `ChatOptions` = `temperature / max_tokens / stream / context_window_tokens / reasoning_enabled / reasoning_effort / tools` | `src-tauri/crates/assistant-providers/src/types/config.rs` | 全是 chat 语义，**没有输出模态维度**；图像参数（size / steps / seed）无处安放 |
| **M3** | `StoredModelEntry` 只解析 `contextWindow / supportVision / inputModalities / vision / pdf / attachment` | 同上 | 能力事实**单向**：只描述「能吃什么」，不描述「能吐什么」；`outputModalities` 被丢弃 |
| **M4** | `UnifiedResponse` 只有 `text / reasoning_content / thinking_signature / tool_call / usage / finish_reason / provider_metadata`；`GeminiPart` 无 `inlineData` 字段 | `src-tauri/crates/assistant-providers/src/gemini/{types,stream}.rs` | 输出侧**只有文本**；图像输出部分被 serde 静默忽略 |
| **M5** | `models-dev.json` 含 `modalities.output: ["text","image"]` 的模型（`gemini-2.5-flash-image`、`gemini-3-pro-image-preview` 等） | `src-tauri/crates/assistant-providers/assets/models-dev.json` | 目录**知道**有图像输出模型，但解析层（M3）不读，适配器（M4）不取 |
| **M6** | `ContentBlock::{Text, Image, File, Thinking, ToolUse, ToolResult}`；OpenAI 侧把 `Image` 转成 `image_url` / `input_image` | `src-tauri/crates/assistant-providers/src/types/message.rs`、`openai/message_converter.rs` | **输入侧**图像已完备；缺口只在输出侧 |
| **M7** | `ContentBlockDto` 含 `Image { attachment }`，且 `src/chat/message/MessageContent.tsx` 已渲染图片块 | `src-tauri/src/protocol/dto/chat/content.rs`、`src/chat/message/MessageContent.tsx` | wire 与渲染侧对「消息带图」已就绪；不需要新 DTO 变体 |
| **M8** | `AttachmentStore` 提供 `save_attachment / read / file_path`，内容寻址 | `src-tauri/crates/assistant-storage/src/attachment_store.rs` | 产物落库能力现成 |
| **M9** | 压缩是「turn 外模型调用」的唯一先例：`execute_compaction` 取 `tasks.begin(conversation_id, OnBusy::Replace)`、用会话主模型、硬编码 `temperature: 0.3`、写 `role = "compact_summary"` 行（`turn_index: None, iteration: 0`）、usage `kind = "compaction"` | `src-tauri/src/cmd/compaction.rs` | 它**没有绕开 turn**，而是「用合成 turn 占住会话键」；这恰好反证 §4.2 |
| **M10** | `TurnTasks` 的键是 `conversation_id`，`turn_id = "{conversation_id}:{counter}"`；`finish` 校验代际；`child_token` 提供父子取消；`OnBusy` 只有 `Replace`（chat 顶替）/ `Reject`（agent 拒绝）两值 | `src-tauri/crates/assistant-agent/src/kernel/tasks.rs`、`kernel/mod.rs` | 会话键不是 run 键；伴生/作业必须另立键；互斥策略是二值闭集（G12） |
| **M11** | `TurnSpec` = `on_busy / max_iterations / tools / ladder / stop_hooks / wrap_up / route / kind / limits`；`TurnLimits` = `max_iterations / max_tool_calls / turn_wall_clock / model_call / empty_response_retries / microcompact_keep_recent` | `src-tauri/crates/assistant-agent/src/kernel/{mod,limits}.rs` | turn 级预算是现成的；伴生/作业的预算没有对应结构 |
| **M12** | `TurnRunCtx` 含 `agent_id / parent_turn_id / delegation_depth` | `src-tauri/crates/assistant-agent/src/agent/round_executor.rs` | 委派语义已预留，但 P8.1 暂缓 |
| **M13** | `AgentModeDto` = `id / name / description / kind / prompt / model / temperature / max_tokens / reasoning_enabled / reasoning_effort / tool_config / avatar` | `src-tauri/src/protocol/dto/settings.rs` | 它是「模型 + 提示词 + 参数 + 工具集」的完整快照，即天然的**执行体配置单元** |
| **M14** | 前端事件传输只处理**当前活跃会话**：`transport.ts` 按 `getActiveConversationId()` 过滤 | `src/services/stream/transport.ts` | 后台作业的终态事件不能走这条通道，否则切会话即丢失 |
| **M15** | 后台异步唯一先例是 `TauriCompactionSink` + `tokio::task::spawn`；且代码注释明确「若改为 spawn，则只有日志可见，前端永远收不到取消语义」 | `src-tauri/src/adapters/compaction_sink.rs`、`src-tauri/src/cmd/compaction.rs` | 「后台 + 无回传」已被判为不可接受；作业必须有回传通道 |
| **M16** | 观测面 12 节点固定模板、7 值状态闭集，事件只读订阅，控制走独立端口 | `src/stores/agent/observation/template.ts`、I55 观测规格（已归档） | 新执行体只能**新增事件种类**，不能改模板结构 |
| **M17** | `I52-Ext`（未立项）把后台任务定义为「异步 + 任务 id + 状态机 + 轮询回收 + 跨 turn 存活」，归属 P9，判据是「任务结束 = 进程树已回收」 | `docs/store/I52-ext-background-task.md` | 与本文的 Job 有语义重叠但**持有者不同**（§4.10） |
| **M18** | `TurnSpec.kind: &'static str`（`"chat"` / `"agent"`），是开放字符串而非枚举 | `src-tauri/crates/assistant-agent/src/kernel/mod.rs` | 新增执行体种类**不必**动终态 / phase / 事件字段；K7 已容得下（§4.14） |
| **M19** | 内核定义判据：①单 turn 闭环需要 ②单会话自足（不引入跨会话 / 跨进程 / 跨重启状态）③有强制点；另有七条不变式 K1–K7 | `docs/store/I36a-agent-kernel-baseline.md` §1.2 / §1.3 | 判据 ② 直接把 Job 挡在内核之外（§4.14） |
| **M20** | `TurnTasks` 的公开面全部以 `conversation_id` 为键：`begin` / `cancel` / `finish` / `current` / `is_current` / `is_busy`；`turn_id` 格式为 `"{conversation_id}:{counter}"` | `src-tauri/crates/assistant-agent/src/kernel/tasks.rs` | 「互斥范围」与「执行槽」耦合在同一参数上，是 Companion 必须动内核的唯一理由（§4.14） |

---

## 3 参考对照

本节不重复既有编排分析 §3 的图执行结论，只列与本主题（**非文本产物**与**执行体种类**）直接相关的三点。

| # | 参考 | 检出事实 | 对本项目的含义 |
|---|---|---|---|
| **R1** | ComfyUI | 节点用 `RETURN_TYPES = ("IMAGE",)` / `("LATENT",)` 声明类型化返回值（`references/9-workflow-orchestration/ComfyUI/nodes.py`） | 在图编排里，图像是**一等值类型**，可与文本并列流动；这证明「产物」可以是图的正常输出，不是特例 |
| **R2** | n8n | provider 能力表只有 `ProviderAttachmentCapabilities { image, pdf, audio }`（`packages/@n8n/api-types/src/agents/provider-capabilities.ts`） | 参考实现的能力模型同样是**输入侧**；「模型能产出什么」在业界也没有统一表达，本项目不是落后而是普遍空白 |
| **R3** | langgraph | checkpoint 以「状态快照」为单位持久化，节点产出并入 state | 提示「执行体结论」应落为**可重放的状态**，而不是只活在内存里（对应 §4.8 的注入钩子落库要求） |
| **R4** | Dify | `nodes/agent_v2/`、`nodes/human_input/` 说明节点可以是「异步 / 等待外部介入」 | 节点级等待是既有形态，但它依赖「图 + run 台账」；本文的伴生通道刻意**不引入 run 台账**，以避免与既有编排分析的前置耦合 |

---

## 4 结构命题

### 4.1 命题一：模型调用不是编排单元

既有编排分析结论 11 判定「节点只能取 turn / 工具调用 / 会话」，理由是「把模型调用做成节点，等于把实现细节写进 wire」。这条判定对本文依然有效，而且本文的需求不与之冲突——因为需求问的根本不是「节点」。

需求原文指向的五件事是：主模型创建伴生模型、伴生压缩模型、嵌入模型做简单事、周期性固定提示词注入、后台节点调用图片生成模型。把这五件事抽象一次，得到的是同一句话：**「这段模型调用由谁执行、什么时候执行、结果回到哪里」**。这句话的主语是执行体，不是节点。

因此：

- 读法 A（加节点）**不成立**，且与既有结论直接冲突；
- 读法 B（换执行体）**成立**，且不触碰 L1 的固定模板。

**推论**：本文的一切设计都不得新增「可连线的模型节点」。若某个需求只能用「加节点」表达，则该需求应被判定为不成立，而不是被实现。

### 4.2 命题二：同步模型单元必须走 turn

「同步单元」（主模型、子代理）必须复用 turn，不能新造一个「轻量 turn」。理由不是保守，而是 turn 是当前唯一同时具备五件事的单元：

| # | 事项 | 现有承载 | 绕开 turn 后的后果 |
|---|---|---|---|
| 1 | 生命周期 | `TurnTasks::begin/finish` 代际 | 没有代际，旧轮迟到完成会清掉新轮状态 |
| 2 | 取消 | 取消令牌挂在调用栈上，`child_token` 父子传播 | 无处挂令牌，超时与取消全部失效 |
| 3 | 预算 | `TurnLimits`（迭代 / 工具 / 墙钟 / 单次模型调用） | 无预算，一次失控调用可烧穿 |
| 4 | 观测 | I55 事件 + `TurnEmitter` | 前端看不到任何过程，只有最终结果 |
| 5 | 落库 | 消息行 + usage 归因 | 无消息行、无 usage，历史重载即丢失 |

**关键反证**：唯一的「turn 外模型调用」先例（压缩，M9）**并没有绕开 turn**——它 `tasks.begin(conversation_id, OnBusy::Replace)` 拿了一个合成 turn id，用 `TurnEmitter` 发事件，用 `PersistCtx` 记 usage。也就是说，现有代码已经用行动给出了答案：**模型调用要独立执行，代价是必须先有一个「占键的合成 turn」**。

这条反证同时暴露了压缩的三个不能照搬之处：

| 不能照搬 | 现状 | 为什么 |
|---|---|---|
| `OnBusy::Replace` | 压缩会抢走会话键，正在跑的 turn 被替换 | 伴生通道的定义就是「不打断主 turn」，抢键即自相矛盾 |
| 会话主模型 | 压缩用 `detail.conversation.model_id` | 需求明确要求「伴生压缩模型」与主模型不同 |
| 硬编码 `temperature: 0.3` | 压缩写死温度 | 执行体配置必须来自配置单元（M13），不能写死在调用点 |

### 4.3 命题三：伴生通道的定义性特征是「不占主键 + 有结论 + 无产物」

伴生通道要成立，必须同时满足三条，缺一条就退化成别的东西：

| 条件 | 判定 | 缺了会怎样 |
|---|---|---|
| **不占主 turn 键** | 必须。主 turn 的 `is_busy` 只反映主 turn | 占键即 `OnBusy::Replace`，主 turn 被打断（M9 的现状） |
| **有结论** | 必须。结论要能被主 turn 或下一次投影读取 | 无结论 = 纯后台日志，与 M15 已判死的形态相同 |
| **无产物** | 必须。伴生只产出**文本结论**或**状态更新** | 有产物就该走 Job（产物需要落库与展示，见 §4.7） |

**伴生结论的落点**：不能写进主 turn 的分支链（会与 `branch_leaf_id` / `sibling_index` 冲突），只能走压缩已验证的「平铺行」形态（`turn_index: None, iteration: 0`，独立 `role`）。这条形态是现成的，不需要新设计，但需要**新 role 与新的投影规则**（哪些 role 参与下一次投影、哪些只供审计）。

**伴生的取消语义**：跟随主 turn（父取消 → 子失效，复用 `TurnTasks::child_token`）。这不需要新机制。

### 4.4 命题四：模型调用端口已与 turn 解耦

`base::create_provider_adapter(config)` + `chat_completion_stream(&api_key, &messages, &model, &options, cancel_token)` 是纯函数式的：输入只有配置、key、消息、模型名、选项、取消令牌，**不依赖任何 turn 状态**。压缩（M9）已经在 turn 之外直接调用它，证明这条路径可用。

**推论**：伴生通道与后台作业**不需要新的内核抽象**，只需要新的「调用点 + 配置来源 + 结果去向」。真正的缺口不在「怎么调模型」，而在「调完之后结果去哪」（§6）。

### 4.5 命题五：能力门禁在输入与输出两侧不对称

| 侧 | 现状 | 门禁形态 |
|---|---|---|
| **输入** | `inputModalities` 已解析（M3）；`ContentBlock::Image` 已转换（M6） | 已有：模型不声明 `image` 输入 → 附件投影降级 |
| **输出** | `outputModalities` **未解析**（M3）；`UnifiedResponse` **无媒体字段**（M4）；`ChatOptions` **无输出模态**（M2） | **缺失**：无法判断「这个模型能不能出图」，也无法要求它出图 |

一个具体的后果：用户在模型选择器里选中 `gemini-2.5-flash-image`（M5 证明它在目录里），当前链路会把它当普通 chat 模型用，请求成功、返回的图像部分被 `GeminiPart` 静默丢弃，用户看到一条空文本回复。**这不是「不支持」，而是「静默降级」**——比报错更糟。

**判定**：输出模态必须成为一等能力事实，且门禁必须 fail-closed：模型不声明 `image` 输出，就不得发起图像作业（对应门禁 A6）。

### 4.6 命题六：缺的是输出通道，不是节点

把「后台节点调用图片生成模型」拆成两半看：

| 半 | 现状 | 缺口性质 |
|---|---|---|
| 「后台节点」 | 有 `tokio::spawn` 先例（M15），但没有任务台账 | 台账缺口（§6 G6） |
| 「图片生成模型调用」 | 端口只有 chat（M1），响应无媒体（M4），目录有模型但不解析（M3/M5） | **输出通道缺口** |

输出通道有两种可能的形态，二者**不可互相替代**：

| 形态 | 覆盖对象 | 依据 |
|---|---|---|
| **a. 扩展现有 chat 端口，新增媒体 part** | Gemini 系「chat 内出图」（`gemini-2.5-flash-image`，input 含 image，可做图像编辑） | M5 显示这类模型是 chat 兼容的；M4 显示只差 `inlineData` 解析 |
| **b. 新增独立生成端口** | DALL·E / Flux / Seedream 等「专用生成端点」（请求体不是 messages，响应是 `b64_json` 或 url） | M2 显示 `ChatOptions` 无图像参数；这类端点无法塞进 chat 语义 |

**判定**：两者都需要，缺一不可；但**可以分期**——先做 a（改动面小、覆盖 Gemini 系、与 chat 端口同构），b 作为后续。这是取舍 D2。

### 4.7 命题七：产物必须落附件库，消息只持引用

三条硬约束决定了产物不能是「消息里的一个 URL」：

1. **远端 URL 有有效期**（多数生成端点的 url 是短期签名），会话是长期事实，重载历史时必然 404；
2. **附件是内容寻址**（M8），天然去重、天然可离线、天然可被再次作为输入喂给主模型（M6 的 `ContentBlock::Image`）；
3. **wire 与渲染已就绪**（M7）：`ContentBlockDto::Image { attachment }` + 前端 `ImageThumbnail`。

因此产物链路固定为：**生成端点 → 字节 → `AttachmentStore::save_attachment` → 引用 → `ContentBlockDto::Image`**。中间任何一步走 URL 直传都违反约束 1。

**推论**：Job 的「产物」不是一种新数据类型，而是**既有附件 + 既有 Image 块**；Job 新增的是「产物何时产生、归属哪个消息」这条链路，不是产物的表示。

### 4.8 命题八：注入钩子不是执行体

「周期性固定提示词注入」的三个特征与执行体完全相反：

| 特征 | 注入钩子 | 执行体 |
|---|---|---|
| 是否发起模型调用 | 否 | 是 |
| 是否有预算消耗 | 否 | 是 |
| 是否有独立生命周期 | 否（属主 turn / 会话） | 是 |
| 影响面 | 只改「下一次投影的输入」 | 产生新结论 |

**落点**：投影层。当前唯一投影点是 `request::project_chat_messages`（压缩也复用它，M9），注入钩子应作为它的一个输入源，而不是新模块。

**但有一个硬约束**：注入内容必须**可重放**。若注入只存在于内存，历史重载后同一份历史会得到不同的投影，破坏「重载一致」。因此注入内容必须落库（独立消息行或 turn 元数据），且注入规则必须确定性可重算（时间、随机数这类不稳定源要么禁用，要么先落库再使用）。这是门禁 A9。

### 4.9 命题九：`conversation_id` 不是 run 键

`TurnTasks` 的键是 `conversation_id`，语义是「单会话单 turn 互斥」（M10）。伴生通道与后台作业的定义都要求「不打断主 turn」，因此**必须另立键**：

| 执行体 | 键 | 取消传播 |
|---|---|---|
| 主 turn | `conversation_id` | 根 |
| 伴生通道 | `companion:<conversation_id>:<purpose>`（或 run 级键） | 随主 turn（`child_token`） |
| 后台作业 | `job:<job_id>`（跨会话唯一） | **不随 turn**；随会话删除 / 应用退出 |

**与既有编排分析的关系**：既有分析已判定「并行需要 run 级键与 run 级取消树」，本文的伴生通道是同一问题的更小形态——它不需要 run 台账，只需要一个不占会话键的旁路键。因此本文**可以先于** run 级基础设施落地，但键的命名与语义必须与将来 run 键兼容（取舍 D1）。

### 4.10 命题十：与 P9 后台任务的分界是「持有者」，不是「语义」

`I52-Ext`（未立项）的后台任务与本文的 Job 语义高度重叠，但定义性特征不同：

| 维度 | P9 后台任务（进程型） | 本文 Job（模型型） |
|---|---|---|
| 定义性特征 | 持有**进程树** | 持有**任务状态 + 产物** |
| 结束判据 | 任务结束 = 进程树已回收 | 任务结束 = 产物已落库 |
| 取消 | `kill_tree` | 取消令牌（中断 HTTP 请求） |
| 重启恢复 | reaper 识别并标记失败 | 无进程可恢复，标记失败即可 |
| 归属 | P9（需单独 ADR） | 本文（不属 P9） |
| **共用** | 任务 id、状态机闭集、轮询语义（超时即成功）、终态事件、turn 外入口 | 同 |

**判定**：必须共用**语义**（同一套状态机、同一种轮询约定、同一类终态事件），不得共用**持有者**（进程回收与产物落库是两种回收动作）。若两套各自发明「运行中」，前端会出现两个互不相识的任务列表（门禁 A10）。

### 4.11 命题十一：无会话调用无法归因

当前所有归因都挂在会话上：

| 归因项 | 载体 | 无会话时的后果 |
|---|---|---|
| token 用量 | `PersistCtx.conversation_id`（必填） | 无处记账 |
| 附件归属 | 会话附件库 | 产物无主 |
| 观测 key | `conversationId` 过滤（M14） | 事件被丢弃 |
| 审批键 | `ApprovalScope` | 无法求值 |

**判定**：不允许存在「无会话锚点」的模型调用。后台作业必须归属一个会话（发起它的会话），这与 `I52-Ext` X1 的倾向一致（「绑定发起它的 turn 供审计，但不随 turn 回收」）。取舍 D7。

### 4.12 命题十二：观测面不用改，但要加事件种类

I55 的硬约束是「sink 恒只读、控制走独立端口、模板静态」（M16）。伴生与作业只需满足：

1. 事件**只新增种类**，不改 12 节点模板；
2. 事件名**不得复用 turn 事件名**——`transport.ts` 会按会话过滤并做 seq 去重（M14），复用会让作业事件被当成 turn 事件处理；
3. 作业的终态必须走**独立通道**（不能走只处理活跃会话的流），否则切会话即丢。

### 4.13 命题十三：价值门槛

只串「伴生压缩」也有独立价值，因为它直接替代了 M9 的三处硬编码（抢键 / 主模型 / 固定温度），这是**现实痛点**，不是想象需求。

但「嵌入模型做简单事」的价值需要打折：turn 内已有工具调用，简单事多数可以用工具做，嵌入模型的不可替代场景只有「语义检索 / 去重 / 聚类」这类**非工具化**任务。

**判定**：真正的不可替代价值只有两件——**(1) 主模型与压缩模型解耦；(2) 非文本产物（图像）**。其余需求若无法证明这两条中的一条，应判定为「可用现有工具调用近似，不做」。

### 4.14 命题十四：内核接管的边界是「语义泛化」，不是「职责扩张」

「内核要不要接管更多底层安排」这句话本身是二义的，必须先拆成两种完全不同的动作：

| 读法 | 做法 | 判定 | 判据 |
|---|---|---|---|
| **语义泛化** | 把**已经事实唯一**的持有者（并发键、取消令牌、`seq` 序列）从「会话专用」改为「执行槽通用」 | **该做** | 同一个东西只能有一个持有者，它已经在内核里，扩它的语义范围成本最低 |
| **职责扩张** | 内核新增任务台账、产物链路、生成端口、注入规则 | **不该做** | 这些东西天然可以有多个持有者（会话级 / app 级 / storage / providers），塞进内核即制造第二个权威源 |

判据不是「内核做得多不多」，而是「**同一个东西是不是只能有一个持有者**」。这条判据与 M19 的三条入阶段判据同源。

**四层逐层判定**：

| 层 | 是否进内核 | 依据 |
|---|---|---|
| **Unit** | 已在，不动 | 它本身就是 turn（K1 单入口） |
| **Companion** | **要，且只改一处**：`TurnTasks` 的键语义泛化 | 见下「四项复制成本」 |
| **Job** | **必须不进** | 违反 M19 判据 ②（单会话自足）；进了会让内核变成第二个 app supervisor，与 P9 的持有者设计直接冲突（§4.10、D8） |
| **Hook** | 半个：投影点在内核，规则在上层 | `kernel/request.rs` 的投影点已在；内核只多一个「注入来源」参数，不持有规则 |

**为什么 Companion 必须进内核（四项复制成本）**：若不进内核，app 层就得自己建一套旁路并发表，并复制以下四样：

| # | 内核现有物 | 复制后的后果 | 违反 |
|---|---|---|---|
| 1 | `finish` 的代际校验（`entry.turn_id == *turn_id`） | 出现第二套代际，旧轮迟到写入无法统一丢弃 | K5 |
| 2 | `child_token` 的父子取消传播 | 取消不再是「只置位令牌」的单一源 | K5 |
| 3 | `TurnEmitter` 构造时以 `tasks` 做 `is_current` 校验 | 两套键会让伴生的 emitter 校验错对象（校验到主 turn 或永远失败） | K6 |
| 4 | 同 turn `seq` 严格单调 | 同会话两个 emitter 共用 sink 时 `seq` 会撞 | K6 |

**为什么是「改一处」而不是「加一个模块」**：`is_busy` 是两件**用户可见**事情的判据——发送时是否 `Reject`、停止按钮是否可点。伴生若占会话键，用户在伴生运行中点发送就会被拒，这是硬错误而非取舍。因此需要的不是「第二套并发表」，而是**第三种 `OnBusy` 策略**（旁路：占槽但不参与互斥），键本身仍由 `TurnTasks` 唯一持有（M20）。

**新增执行体种类不需要动内核**：`TurnSpec.kind` 是开放字符串（M18），K7 已规定「链路差异只经 `TurnSpec` 参数表达」。因此「多一种执行体」不落在 I36a §5 的「必须动内核」清单里——那份清单只有四条：新增终态、新增 phase、事件新增必填字段、改变工具执行唯一入口。本文不触碰任何一条。

**明确不进内核的四项及其归属**：

| 能力 | 归属 | 理由 |
|---|---|---|
| 任务台账（id / 状态机 / 轮询 / 终态） | app supervisor（与 P9 共用语义，§4.10） | 跨 turn 存活，违反判据 ② |
| 产物链路（job → artifact → message） | storage + 上层装配 | 内容是存储事实，不是执行决策 |
| 生成端口与输出模态 | providers | 内核不决定「模型能吐什么」，只决定「这一轮怎么走」 |
| 注入规则（周期 / 条件） | 上层配置 | 内核只提供投影输入点，不持有规则表 |

**收口**：内核接管「**一次执行怎么活、怎么死**」，不接管「**执行完之后东西放在哪**」。

**为什么压缩是反证而不是先例**：它之所以「看起来绕开了 turn」，恰恰是因为它 `tasks.begin()` **借了内核的键**（M9）。谁想真正绕开，就得在内核之外复制上表四样——那不是解耦，是分裂。

---

## 5 形态取舍

| 需求 | 形态 | 承载 | 可复用资产 | 必须新增 |
|---|---|---|---|---|
| 主模型 / 子代理 | Unit | turn | 全套内核（M11/M12） | 无（P8.1 暂缓） |
| 伴生压缩（独立压缩模型） | Companion | 合成 turn + 独立键 | 压缩全链路（M9）、`TurnEmitter`、`PersistCtx` | 独立键、伴生结果 role 与投影规则 |
| 旁路校验 / 嵌入索引 | Companion | 同上 | 同上 | 同上 |
| 周期固定提示词注入 | Hook | 投影输入 | `request::project_chat_messages` | 注入内容的落库点 |
| 图像生成（Gemini 系） | Job | job 台账 + 扩展 chat 端口 | `AttachmentStore`、`ContentBlockDto::Image`、前端渲染（M7/M8） | 媒体 part 解析、输出模态事实、job 台账 |
| 图像生成（专用端点） | Job | job 台账 + 生成端口 | 同上 | 独立生成端口 + 参数面 |
| 长文档产物 | Job | 同上 | 同上 | 同上 |

---

## 6 缺口清单

| # | 缺口 | 现状证据 | 性质 |
|---|---|---|---|
| **G1** | 独立并发键 | `TurnTasks` 键 = `conversation_id`（M10） | 伴生 / 作业的必要前提 |
| **G2** | 非文本输出通道 | `UnifiedResponse` 无媒体字段、`GeminiPart` 无 `inlineData`（M4） | 图像作业的必要前提 |
| **G3** | 生成端口 | `BaseProviderAdapter` 只有 chat / list_models（M1） | 专用端点作业的必要前提 |
| **G4** | 输出模态能力事实 | `StoredModelEntry` 不解析 `outputModalities`（M3） | 门禁 A6 的必要前提 |
| **G5** | 产物链路（job → artifact → message） | `AttachmentStore` 有，链路无 | 作业结果可见的必要前提 |
| **G6** | 任务台账（id / 状态机 / 轮询 / 终态事件） | 只有 `tokio::spawn` 先例（M15） | 作业可观测、可取消的必要前提 |
| **G7** | turn 外前端入口 | `transport.ts` 只处理活跃会话（M14） | 作业终态不丢的必要前提 |
| **G8** | 注入内容落库点 | 投影层无「注入来源」概念 | 重放一致的必要前提（A9） |
| **G9** | 伴生 / 作业的预算归因 | `TurnLimits` 只覆盖 turn（M11） | 成本可见的必要前提 |
| **G10** | 伴生 / 作业的取消树 | `child_token` 有，键与归属无 | 取消语义正确的必要前提 |
| **G11** | `TurnTasks` 的键语义只覆盖「会话互斥」 | `begin` / `is_busy` / `cancel_token` 三面均以 `conversation_id` 为参数（M20） | Companion 可用的必要前提（§4.14） |
| **G12** | 第三种 `OnBusy` 策略（旁路：占槽但不参与互斥） | `OnBusy` 只有 `Replace` / `Reject` 两种（M10） | 伴生不打断主 turn 的必要前提（A1） |

---

## 7 必须现在定案的取舍

以下八条决定 DTO 字段、事件名与键形态，必须在写规格前定案，不能留到实施阶段。

| # | 取舍 | 选项 | 倾向 | 理由 |
|---|---|---|---|---|
| **D1** | 伴生键的形态 | `companion:<conv>:<purpose>` / `run:<run_id>` | 前者，且预留 `run:` 前缀语义 | 不依赖尚未落地的 run 台账；命名与将来 run 键不冲突 |
| **D2** | 输出通道 | 扩 chat 端口 / 独立生成端口 / 两者 | **两者，分期做**（先 a 后 b） | M5/M2 证明两类模型请求形态不同，不能互相覆盖（§4.6） |
| **D3** | 伴生结论的落库形态 | 独立 role 行 / turn 元数据 / 仅内存 | 独立 role 行（沿用压缩形态） | 只有落库才能参与下次投影与审计；内存形态已被 M15 判死 |
| **D4** | 作业终态如何进消息链 | 复用 `Finished` / 独立终态事件 | **独立终态事件** | 复用会重放两次（I52-Ext X2 已判定），历史重载必错 |
| **D5** | 作业是否绑定发起 turn | 绑定 / 不绑定 | 绑定审计、不随 turn 回收 | 与 I52-Ext X1 一致；否则产物无来源 |
| **D6** | 注入钩子的确定性来源 | 落库快照 / 每次重算 | 落库快照 | 重算会因时间等不稳定源破坏重放一致（A9） |
| **D7** | 无会话调用的锚点 | 禁止 / 归属会话 | **禁止无锚点，一律归属会话** | 归因体系全部挂会话（§4.11） |
| **D8** | 与 P9 的台账关系 | 同一张表 / 两张表 | 同一套**语义**、两个**持有者**，wire 上同一套 DTO | 避免前端出现两套「运行中」（§4.10、A10） |
| **D9** | 内核键语义的泛化方式 | 新增 `OnBusy::Sidecar` 策略 / 键加前缀 / 新开一张表 | **新增 `OnBusy::Sidecar`**，键仍由 `TurnTasks` 唯一持有 | 只扩语义范围、不复制持有者；新开表即复制代际与取消源（§4.14） |

---

## 8 门禁

开工前必须先满足以下判定，任一条不满足即不得进入实施。

| # | 门禁 | 判据 |
|---|---|---|
| **A1** | 伴生不得抢主 turn 键 | 伴生运行期间 `TurnTasks::is_busy(conversation_id)` 仍只反映主 turn |
| **A2** | 伴生失败不影响主 turn | 伴生错误只进自身结论，主 turn 状态机不受影响（O9 的扩展） |
| **A3** | 事件不复用 turn 事件名 | 伴生 / 作业事件名与 turn 事件名无交集，且 `transport.ts` 不会误判 |
| **A4** | 作业终态不重放 | 历史重载后同一产物只出现一次（D4） |
| **A5** | 产物必须内容寻址落盘 | wire 上只出现附件引用，不出现远端 URL（§4.7） |
| **A6** | 输出模态门禁 fail-closed | 模型未声明 `image` 输出 → 拒绝发起图像作业，不静默降级（§4.5） |
| **A7** | 取消语义分治 | 伴生随主 turn；作业不随 turn，随会话删除 / 应用退出（§4.9） |
| **A8** | 预算可归因 | 每次模型调用都记 usage，且带执行体种类（turn / companion / job） |
| **A9** | 注入可重放 | 历史重载后投影结果与首次一致（D6） |
| **A10** | 台账状态机闭集一致 | 作业状态取值与 P9 后台任务同一闭集，不出现第三态 |
| **A11** | 观测面结构不变 | I55 的 12 节点模板与 7 值状态不因本特性改动 |
| **A12** | 后台作业不得静默失败 | 失败必须进消息链或系统通知，不得只写日志（M15 的反面） |
| **A13** | 内核不变式不因本特性改变 | K1–K7 逐条不变；I36a §5 的四条「必须动内核」事项一条未碰（§4.14） |
| **A14** | 伴生不产生第二套代际 / 取消源 | 全仓只有一个 `TurnTasks` 实例持键；伴生的 `finish` 校验与 `child_token` 均走它（D9） |

---

## 9 与既有阶段 / 文档的关系

| 对象 | 关系 |
|---|---|
| **P9 continuity** | Job 的任务台账必须与 P9 的「turn 外入口 / 状态机 / 轮询语义 / 终态事件」共用语义；但进程型与模型型的持有者不同（§4.10）。`I52-Ext` G5（turn 外入口）对本文同样成立 |
| **P8.1 子代理（暂缓）** | Unit 的委派形态；`TurnRunCtx` 的 `parent_turn_id` / `delegation_depth` 已预留（M12），本文不推动它 |
| **P8.2 / P8.3 会话编排（未立项）** | 跨会话调度的前置；本文的伴生通道刻意不依赖它 |
| **P15 观测面（已交付）** | 本文不改其结构，只新增事件种类（A11） |
| **压缩（已实现）** | 是 Companion 的第一个落地场景，也是它的直接替代对象（M9 的三处硬编码） |
| **既有编排分析** | 补集关系：它管 turn 之间的图，本文管 turn 之外还有哪几种执行体（§1.2） |
| **I36a 内核基线** | 本文只做「语义泛化」（`OnBusy` 加一种策略、键语义扩到执行槽），**不碰** I36a §5 的四条「必须动内核」事项（A13、§4.14） |

**阶段归属建议**：本文的能力不构成单一阶段，应拆成三个独立候选，且**不得**与既有编排分析的候选（P8.2 / P8.3 / P15）混编——否则「模型级编排」会被误读成「图编排的一部分」。阶段号先登记，再写 spec（roadmap §7 第 5 条）。

---

## 10 未决问题

| # | 问题 | 倾向 |
|---|---|---|
| **Q1** | 嵌入模型的用途边界在哪（「做简单事」是否只是工具调用的劣化替代） | 只保留非工具化场景（语义检索 / 去重 / 聚类）；其余不做 |
| **Q2** | 伴生压缩与现有 auto-compact 是替换还是并存 | 替换：伴生通道落地后，压缩不再是特例而是 Companion 的一个 `purpose` |
| **Q3** | 图像参数面（size / steps / seed / 参考图）如何进 DTO 而不污染 `ChatOptions` | 独立参数结构，挂在执行体配置上（M13 的扩展），不塞进 `ChatOptions` |
| **Q4** | 作业产物能否再次作为输入喂给主模型 | 可以，且是 `AttachmentStore` 内容寻址的天然收益（M6）；但需定「谁决定引用」 |
| **Q5** | 作业的并发配额 | 与前台工具分开计账（沿用 I52-Ext X5 的结论） |
| **Q6** | 一次作业产出多个产物（多张图） | 允许，产物列表挂在同一 job 上；不做「一 job 一产物」限制 |

---

## 11 收口

**模型级编排成立，但它不是图，也不是节点。** 它是「执行体种类」的扩展：同步单元继续走 turn（因为 turn 是唯一五件事齐备的单元，压缩先例反证了这一点）；伴生通道是 turn 内的旁路，必须另立键、必须有结论、必须无产物；后台作业是 turn 之外的长生命周期单元，必须有产物、必须可失败、必须跨 turn 存活；周期注入不是执行体，是投影层的一个可重放输入源。

三条最硬的现实约束必须写在规格的第一页：**第一，现有链路输入可多模态、输出只有文本，图像作业的第一步不是「加节点」而是「加输出通道」；第二，所有归因挂在会话上，不存在无锚点的模型调用；第三，后台异步不能只 `spawn`——M15 的注释已经把「后台 + 无回传」判为不可接受。**

价值门槛同样必须写死：真正不可替代的只有「压缩模型与主模型解耦」和「非文本产物」两件；其余需求若不能归到这两条，应判定为「工具调用可近似，不做」。**不做价值门槛的模型级编排，会得到一个「用不同模型重复做工具调用能做的事」的空壳。**

内核边界同样必须写死：**内核接管「一次执行怎么活、怎么死」，不接管「执行完之后东西放在哪」。** 具体到本特性，内核只做一处语义泛化（`OnBusy` 增一种旁路策略、键语义从「会话互斥」扩到「执行槽」），其余四项——任务台账、产物链路、生成端口、注入规则——一律不进内核（§4.14、A13/A14）。**凡是需要「在 app 层再建一套并发表」的方案，都已经走错了方向。**

---

## 参考

**项目文档**

- `docs/analysis/AGENT-WORKFLOW-ORCHESTRATION-ANALYSIS.md`（L1/L2/L3 粒度、结论 11、run 级键）
- `docs/store/I38-agent-roadmap.md` §2 / §2.1 / §7（阶段台账与状态维护规则）
- `docs/store/I52-ext-background-task.md`（未立项；后台任务的改动面 E1–E8、开放问题 X1–X5）
- `docs/store/I38f-agent-subagent-spec.md`（P8.1，暂缓）
- `docs/store/I36a-agent-kernel-baseline.md`（内核能力基线）
- I55 观测规格（已归档；只读订阅、独立控制端口、模板静态）
- `docs/principle/CODE-REVIEW-PRINCIPLE.md`、`docs/principle/CODE-SPLIT-PRINCIPLE.md`

**代码**

- `src-tauri/crates/assistant-providers/src/base.rs`（`BaseProviderAdapter` 唯一端口）
- `src-tauri/crates/assistant-providers/src/types/config.rs`（`ChatOptions`、`StoredModelEntry`、模态解析）
- `src-tauri/crates/assistant-providers/src/types/message.rs`（`ContentBlock`）
- `src-tauri/crates/assistant-providers/src/gemini/{types,stream}.rs`（`GeminiPart` 无 `inlineData`）
- `src-tauri/crates/assistant-providers/assets/models-dev.json`（`modalities.output` 含 image 的模型）
- `src-tauri/crates/assistant-agent/src/kernel/{mod,limits,tasks}.rs`（`TurnSpec` / `TurnLimits` / `TurnTasks`）
- `src-tauri/crates/assistant-agent/src/agent/round_executor.rs`（`TurnRunCtx`）
- `src-tauri/src/cmd/compaction.rs`（合成 turn 先例）
- `src-tauri/src/adapters/compaction_sink.rs`（后台异步先例与其注释约束）
- `src-tauri/crates/assistant-storage/src/attachment_store.rs`（产物落库）
- `src-tauri/src/protocol/dto/chat/content.rs`（`ContentBlockDto::Image`）
- `src/chat/message/MessageContent.tsx`（图片块渲染）
- `src/services/stream/transport.ts`（活跃会话过滤）

**参考实现**（`references/9-workflow-orchestration/`）

- ComfyUI `nodes.py`（`RETURN_TYPES = ("IMAGE",)` 类型化返回值）
- n8n `packages/@n8n/api-types/src/agents/provider-capabilities.ts`（能力表只有附件输入）
- langgraph（checkpoint 状态快照）
- Dify `api/core/workflow/nodes/{agent_v2,human_input}/`（异步 / 等待型节点）
# 编排层「缝」的代码安排：参考项目专项分析

> 日期：2026-11-19｜2026-11-20（结论速览改命题核对、基线改时点快照、移除决议性表述）｜2026-11-22（§4.1 补计数口径）
> 性质：**预先分析**（只回答「清晰是靠什么机制维持的、我们差在哪一条」；**不含取舍决议**——缝怎么立、注册表落哪层、要不要动 crate 结构，属规格职责）
> 代码事实快照：仓库代码的 2026-11-19 时点（**时点记录**，不随代码演进追改，需要最新事实时以代码为准）
> 时效与终止：本文是**一次性决策输入**。编排层结构调整的规格定案后整体归档冻结、不再修改；有效内容由规格承接，事实条目作为历史快照保留。
> 判据来源：`docs/principle/CODE-SPLIT-PRINCIPLE.md`、`docs/principle/CODE-REVIEW-PRINCIPLE.md`
> 参考实现：`references/9-workflow-orchestration/` 稀疏检出——Dify（`api/core/workflow/`：`node_factory.py`、`node_runtime.py`、`node_execution.py`、`graph_topology.py`、`nodes/*`）、langgraph（`libs/langgraph/pregel/`：`_read.py`、`_write.py`）、n8n（`packages/workflow/src/`：`graph/graph-utils.ts`、`common/get-runnable-nodes.ts`）、ComfyUI（`execution.py`、`nodes.py`、`comfy_execution/graph.py`）。凡标「参考」的判断只出自上述检出部分，不外推到未检出部分。
> 与既有文档的关系：**能力缺口**（缺哪些编排原语）见 `AGENT-WORKFLOW-ORCHESTRATION-ANALYSIS.md` §12，本文不复述；**执行体定义域**（Unit / Companion / Job / Hook 各管什么）见 `MODEL-LEVEL-ORCHESTRATION-ANALYSIS.md`。本文只回答第三个问题：**同样的能力，代码凭什么排得清**。

---

## 0 命题核对

| # | 命题 | 核对 | 依据 |
|---|---|---|---|
| 1 | 「清晰」可以被设计出来（先定风格再写代码） | **不成立** | 参考项目里每一处被抽出的纯模块，docstring 都写着一个**具体的第二个调用点**或**一次具体的越界事实**；拆分是被协调压力逼出来的，不是审美驱动的（§2.5） |
| 2 | 目录整齐就是清晰 | **不成立** | Dify 10 个节点目录中，漂移只发生在**文件名拼写**（`exc.py`/`_exc.py`、`node.py`/`<name>_node.py`），**结构从不漂移**；被约束的是「四个问题各有一处可寻址文件」，不是模板（§2.2） |
| 3 | 缝的好坏有一个可机检指标：**反向依赖边为 0** | **成立** | 四个参考项目全部满足；本项目不满足（§2、§4） |
| 4 | Dify 的引擎在 `core/workflow` 里 | **不成立** | 引擎是外部包 `graphon`（本检出 `api/` 下无该目录）；`core/workflow` 单向 import 它，反方向不存在（§2.1） |
| 5 | langgraph 靠「节点类型词汇表」把引擎与节点解耦 | **不成立** | 它只认识两个动词——`ChannelRead` 与 `ChannelWrite`；状态通道只有 `CONFIG_KEY_READ` / `CONFIG_KEY_SEND` 两个常量（§2.3） |
| 6 | n8n 的图论跑在引擎里 | **不成立** | 图论在 `packages/workflow`（只 import 类型），执行在 `packages/core`；纯函数被两个消费方共享（§2.4） |
| 7 | ComfyUI 的节点契约不可机检 | **不成立** | 契约是类属性，引擎全部用 `hasattr`/`getattr` 读取，可机检；但**文件级没有形状**，2634 行节点实现挤在一个 `nodes.py`（§3） |
| 8 | 本项目存在「内核 / 链路」之间的缝 | **不成立** | `kernel → agent` 12 条边、`agent → kernel` 17 条边，双向互指，缝不存在（§4.1） |
| 9 | K7 已经管住了这条边界 | **不成立** | K7 管住了「内核内不写 `if is_agent`」（判断），没管住「内核文件里 `new` 出 agent 类型」（构造）；实测确实无 `if is_agent`，但 `TurnSpec::agent()` 就在 `kernel/mod.rs`（§4.3） |
| 10 | 端口层是最后一道防线，所以词汇一定是通用的 | **不成立** | `kernel/ports/mod.rs` 的 `TurnEvent` 已含 `Node(AgentNodeEvent)` / `Tool(ToolEvent)` 两个 agent 专有变体——端口层已被单一链路占据（§4.2） |
| 11 | 不做缝也能做编排 | **不成立** | 编排层要能表达「任意节点的状态」；当前 `TurnEvent` 只能表达「agent 节点」，所以只能编排 agent，无法编排 Companion / Job 等第二种执行体（§4.4） |

一句话：**参考项目的「清晰」不是代码风格，是三个可复述的机制——单向的缝、可枚举的命名、被协调压力逼出来的下沉。本项目当前一条都不具备：`kernel` 与 `agent` 互指，端口层词汇已被单一链路占据。这是结构事实，其后果是编排层能否容纳第二种执行体的前置条件不成立。**

---

## 1 切入点：为什么量「反向依赖边」，而不是「行数 / 目录整齐度」

### 1.1 三个候选指标，两个不可用

| 指标 | 为什么不可用 |
|---|---|
| 文件行数 / 圈复杂度 | 参考项目的行数差异极大且与清晰无关：Dify `graph_topology.py` 86 行 vs `node_runtime.py` 1081 行；ComfyUI `graph.py` 392 行 vs `nodes.py` 2634 行。行数少可能只是没人用，行数多可能只是同形重复 |
| 目录整齐度（同形率） | 同形可以**照抄而不理解**。抄出一个 `entities.py` / `exc.py` 的空壳，同形率 100%，契约仍然没人填 |
| **反向依赖边** | 一次 grep 可数、可解释、且能直接落到本项目的具体缺陷（§4）。它是「缝是否被击穿」的直接度量 |

### 1.2 「缝」的定义

本文用「缝」指一条边界，同时满足三件事：

1. **位置**：边界两侧有各自独立的词汇（一侧说「节点」，另一侧说「agent 节点」）；
2. **方向**：跨过去是单向的，反向边为 0；
3. **可枚举**：边界面上的适配器有统一命名，一次搜索能把整条缝面列出来。

三件事缺一：有位置没方向 → 双向耦合；有方向没命名 → 缝存在但没人找得到，新人仍然读不懂。

### 1.3 由定义直接派生的三个子问题

- **缝在哪**：机制（引擎）与策略（产品）各自持有什么？
- **缝怎么被认出来**：命名前缀 / 包名 / 动词，能否一次 grep 枚举？
- **缝凭什么不被击穿**：注册、bootstrap、纯函数下沉分别归谁？

§2 按这三个子问题逐个项目取证；§3 给反面参照；§4 落到本项目；§5 收成五条判据。

---

## 2 参考对照：四个参考项目的缝

### 2.1 Dify：缝是包边界，产品单向依赖引擎

Dify 的编排引擎不在 `core/workflow` 里——它是一个**外部包** `graphon`。`core/workflow/node_factory.py` 顶层就有 18 条 `from graphon...` import，反方向不存在。

引擎给的四件东西：

| 类别 | 具体符号 | 性质 |
|---|---|---|
| **词汇** | `NodeType`、`NodeExecutionType`、`BuiltinNodeTypes` | 闭集枚举，节点级通用 |
| **实体** | `BaseNodeData`、`WorkflowNodeExecution` | 数据形状 |
| **注册表** | `Node.get_node_type_classes_mapping()` | 查找机制（不持生命周期） |
| **端口** | `NodeFactory`、`CodeExecutorProtocol` | 接口 |

产品给的三件东西：

| 类别 | 具体符号 | 性质 |
|---|---|---|
| **工厂实现** | `DifyNodeFactory(NodeFactory)` | 适配器 |
| **差异字段** | `WorkflowNodeExecution` 的 9 行子类，只加 `triggered_from_workflow_id` / `triggered_from_node_execution_id` | 最小扩展 |
| **bootstrap** | `register_nodes()` | 策略 |

三件事各自有一个可引用的证据：

**(a) bootstrap 归属写在 docstring 里。** `register_nodes()` 的说明原文大意是：工作流层拥有节点 bootstrap，因为它必须把内置的 `graphon.nodes.*` 与工作流本地节点组合起来；把这个 import 副作用留在这里，是为了**不把注册表 bootstrap 重新塞回低层图原语**。——这是一句「为什么策略不能下沉」的直接陈述，不是注释装饰。

**(b) 注册是数据，不是分支。** `_import_node_package` 遍历包内所有模块并逐个 import，节点在被 import 时自注册。因此**新增一种节点 = 新增一个目录**，不改引擎任何一行。

**(c) 缝面一次 grep 可枚举。** `node_runtime.py` 里成排的 `Dify*` 类各自实现一个 `*Protocol`：

| 适配器 | 实现的端口 |
|---|---|
| `DifyFileReferenceFactory` | `FileReferenceFactoryProtocol` |
| `DifyPreparedLLM` | `LLMProtocol` |
| `DifyPromptMessageSerializer` | `PromptMessageSerializerProtocol` |
| `DifyRetrieverAttachmentLoader` | `RetrieverAttachmentLoaderProtocol` |
| `DifyToolFileManager` | `ToolFileManagerProtocol` |
| `PollingLLMRuntimeProtocol` | 轮询能力端口 |

`grep "class Dify"` 一次就能列出全部适配器；`grep "Protocol"` 一次就能列出全部端口。**缝被命名露出来了。**

**(d) 产品差异被压缩到「两个字段」。** `node_execution.py` 全文 9 行：docstring 一句「Dify 持久化身份并列于 Graphon 的工作流执行状态」，然后一个子类加两个字段。差异没有扩散成「两套实体」。

### 2.2 Dify：节点目录的形状是「四问」，不是模板

Dify 的 `nodes/` 下 10 个节点目录，实际文件清单：

| 目录 | 文件 | 归类 |
|---|---|---|
| `datasource` | `__init__` / `entities` / `exc` / `datasource_node` | 基准形 |
| `trigger_plugin` | `__init__` / `entities` / `exc` / `trigger_event_node` | 基准形 |
| `trigger_schedule` | `__init__` / `entities` / `exc` / `trigger_schedule_node` | 基准形 |
| `knowledge_index` | 基准形 + `protocols` | 骨架 + 依赖面 |
| `knowledge_retrieval` | 基准形 + `retrieval` / `template_prompts` | 骨架 + 内部实现 |
| `trigger_webhook` | `__init__` / `entities` / `exc` / `node` | 拼写漂移（`node` vs `<name>_node`） |
| `human_input` | `__init__` / `entities` / `_exc` / `boundary` / `callback` / `constants` / `enums` / `pause_reason` / `session_binding` | 拼写漂移（`_exc` vs `exc`）+ 复杂 |
| `agent` | 9 个文件 | 复杂 |
| `agent_v2` | 18 个文件 | 复杂 |
| `human_input_v2` | `entities` / `events` / `form_compiler` / `migration` / `node` / `presentation` / `runtime` | 复杂（无 `__init__` 之外的基准名） |

统计：**3 个完全一致 + 2 个骨架加料 + 2 个拼写漂移 + 3 个复杂目录**。

关键判读：漂移**只发生在名字，从不发生在结构**。这反过来说明被约束的不是「文件必须叫 `exc.py`」，而是「下面四个问题各自必须有一处可寻址的文件」：

| 四问 | 落点 | 缺了会怎样 |
|---|---|---|
| 吃什么 / 吐什么 | `entities.py` | 引擎无法校验类型，只能当 `Any` 传 |
| 怎么失败 | `exc.py` | 失败被吞成通用异常，上层无法分支 |
| 依赖什么 | `protocols.py` | 依赖无法被替身替换，只能真跑 |
| 行为在哪 | `<name>_node.py` | —— |

这四问与 `AGENT-WORKFLOW-ORCHESTRATION-ANALYSIS.md` §12.2 的九条自述是同一件事的两种写法：那边问「节点该声明什么」，这边问「声明写在哪才丢不掉」。

### 2.3 langgraph：缝是动词

langgraph 的 `pregel/` 里，状态访问本身被升成了与节点同一代数的对象：

- `_read.py`：`ChannelRead(RunnableCallable)`，docstring 明说「既可作为 runnable 运行，也可作为静态方法直接调用」；
- `_write.py`：`ChannelWrite`，配 `ChannelWriteEntry` 与 `SKIP_WRITE` / `PASSTHROUGH` 两个哨兵值，表达「跳过写」「原样透传」。

全部状态通道被压缩成两个常量：`CONFIG_KEY_READ` / `CONFIG_KEY_SEND`。

判读：**引擎不认识节点类型，只认识「读」和「写」两个动词。** 节点是 `Runnable`，读和写也是 `Runnable`——引擎因此不需要一张节点词汇表，也就不可能被某种节点类型污染。

代价是分层的清晰而非总量的清晰：`_loop.py` 约 94KB / 2208 行、`_algo.py` 约 53KB / 1465 行（本检出规模）。这提醒一件事：**缝做得好，不等于代码总量小。**

### 2.4 n8n：缝是包 + 纯函数

- `packages/workflow`：数据模型与图论，**只 import 类型**。`graph/graph-utils.ts` 的全部 import 就是 `../interfaces` 里的 `IConnection` / `IConnections`。
- `packages/core`：执行引擎；`src/execution-engine/workflow-execute.ts` 从 `'n8n-workflow'` 取值，反方向不存在。

`common/get-runnable-nodes.ts` 的 docstring 是这个模式的说明书：

> 「Publishing 在两处被这道门禁约束，而这两处必须一致：服务端拒绝激活会让一次 run 坏掉的节点，编辑器因同一原因灰掉 Publish 按钮。一次 run 够不到的节点不可能弄坏它，所以两个面都不该因它阻塞。」

判读：抽成纯函数的**理由是「两个消费方必须给出同一个答案」**，不是「文件太大」。

### 2.5 三份 docstring 的共同点：清晰性的成因

| 项目 | 抽出 / 下沉的对象 | 写下的理由 |
|---|---|---|
| Dify | `graph_topology.py`（86 行） | 从 `nodes/agent_v2/validators.py` 抽出，使**两个调用点解析同一个 `Workflow.graph` JSON 形状** |
| Dify | `register_nodes()` | 工作流层拥有 bootstrap，**避免把注册表 bootstrap 重新塞回低层图原语** |
| n8n | `get-runnable-nodes.ts` | 服务端与编辑器**两处必须一致** |

三条理由没有一条是「代码太多所以拆」。归纳成一句话：

> **参考项目的模块边界，全部可以追溯到一次具体的协调压力——要么「两处必须一致」，要么「低层不许反向引导高层」。**

这是本文最重要的发现：**清晰性是被逼出来的，不是被设计出来的。** 由此反推一条判据——如果一处逻辑只有一个调用点、且没有越界，它就该留在原地；这正与 `CODE-SPLIT-PRINCIPLE.md` 的判据同向。

---

## 3 反面参照：ComfyUI 的缝在哪、哪不在

选 ComfyUI 作反面参照，是因为它的**契约机制最值得学**，而**文件与状态的组织最不该学**。

### 3.1 在的地方：契约用类属性，引擎只读词汇表

`execution.py`（1470 行）对具体节点零硬编码，全靠 `hasattr` / `getattr` 读节点自述：

| 属性 | 引擎怎么用 |
|---|---|
| `INPUT_TYPES()` | `class_def.INPUT_TYPES()` 取输入声明并校验 |
| `RETURN_TYPES` | 按返回类型个数做列表广播（`tuple([r] * len(obj.RETURN_TYPES))`） |
| `FUNCTION` | `obj.FUNCTION` 取出要调用的方法名 |
| `VALIDATE_INPUTS` | 取 `validate_function`，无则回落到 `validate_inputs` |
| `OUTPUT_NODE` | `class_def.OUTPUT_NODE is True` 判定该节点产出是最终产物 |
| `INPUT_IS_LIST` / `OUTPUT_IS_LIST` | 决定输入/输出是否按列表自动展开 |
| `IS_CHANGED` | 参与缓存键（节点未变则跳过） |
| `check_lazy_status` | 惰性求值状态钩子 |

`nodes.py` 里这些声明属性的声明行就有 211 行。也就是说：**ComfyUI 的「节点自述」是真实存在的、且引擎只认词汇不认节点。** 注册表同样存在且是数据——`NODE_CLASS_MAPPINGS`，引擎按名字查表。

### 3.2 不在的地方：文件级无形状 + 调度状态靠继承

- `nodes.py` 2634 行 / 110KB：全部节点实现挤在一个文件。**加节点不改引擎（好），但文件本身没有形状（不好）**——读者无法通过「目录里有几个文件」判断某个节点是否补齐了自述。
- `comfy_execution/graph.py` 392 行：`TopologicalSort` 持有 `blockCount` / `staged_node` / external block 等**可变调度状态**，`ExecutionList(TopologicalSort)` 再在其上叠加 cache 与 inhibit。

判读：**调度状态通过继承层层传递**，读一个方法要同时持有两级状态；对照 n8n 把图论做成无状态纯函数。

### 3.3 四项目清晰度来源对照

| 维度 | Dify | langgraph | n8n | ComfyUI |
|---|---|---|---|---|
| 缝的位置 | 包边界 | 动词（读 / 写） | 包边界 | 类属性 |
| 缝的命名 | `Dify*` / `graphon` | `_read` / `_write` | `n8n-workflow` / `core` | UPPER_CASE 属性 |
| 反向边 | 0 | 0 | 0 | 0 |
| 契约可机检 | 是（Protocol + 枚举） | 是（类型系统） | 是（类型） | 是（`hasattr`） |
| 契约可人读 | 是（目录四问） | 中（动词分文件） | 是（图论 / 引擎分文件） | 否（单文件 2634 行） |
| 状态放哪 | 产品实体子类 | channel + checkpoint | execution data | 继承字段 |
| 最大短板 | 适配器数量多 | 抽象层级高、总量大 | —— | 文件级与状态级无形状 |

核对：**ComfyUI 的清晰是「类级契约清晰 + 文件级与状态级不清晰」，不能整体平移；其中可复述的机制只有「引擎只读词汇表」这一条。**

---

## 4 本项目实测

### 4.1 反向边计数

| 方向 | 条数 | 分布 |
|---|---|---|
| `kernel → agent` | **12** | `kernel/mod.rs` 3、`kernel/ports/mod.rs` 2、`kernel/turn/emit.rs` 2、`kernel/turn/runner.rs` 5 |
| `kernel → agent`（测试面，不计入） | 2 | `kernel/ports/test_doubles.rs` |
| `agent → kernel` | **17** | 分布 9 个文件：`middleware` 1、`round_executor` 1、`stop` 1、`loop_executor/budget` 1、`loop_executor/call` 4、`loop_executor/execute` 1、`loop_executor/mod` 3、`loop_executor/persist` 2、`loop_executor/terminal` 3 |

计数口径：按 `use crate::agent::…` 的 import 条目计——同一文件多条分别计，`super` / 同层引用不计，测试面单列不计入。复算时须按同一口径，否则条数不可比。

核对：**双向互指，缝不存在。** 参考项目在这个指标上全是 0（§3.3）。

### 4.2 三处最重的击穿

**(1) 端口层词汇被单一链路占据。** `kernel/ports/mod.rs` 顶部 import `crate::agent::node_event::AgentNodeEvent` 与 `crate::agent::tool_event::ToolEvent`，随后 `TurnEvent` 里直接含 `Tool(ToolEvent)` 与 `Node(AgentNodeEvent)` 两个变体。

后果：内核无法在不认识 agent 的情况下表达「节点状态变化」。任何第二种执行体（Companion / Job）想发自己的节点事件，只能改 `TurnEvent` —— 而 `TurnEvent` 是端口契约，改它等于改 wire 投影的输入面。

对照：ComfyUI 的 `ExecutionBlocker`、Dify 的 `NodeExecutionType`，都是**节点级通用词**，不含任何一条链路的专名。

**(2) 策略写在引擎文件里。** `kernel/mod.rs` 里既有 `TurnSpec::agent()` 构造器，也有 `agent_stop_hooks()` 这个私有函数，直接 `Arc::new(crate::agent::stop::IterationLimitHook)` / `ToolCallLimitHook` / `TurnWallClockHook`。

后果：「agent 恒装哪三个钩子」这条**策略**与引擎同文件。新增链路要么改内核文件，要么复制这段。

对照：Dify 明确把 bootstrap 放在产品层并写下了理由（§2.1(a)）。

**(3) 引擎认识具体执行器。** `kernel/turn/runner.rs` 直接 `crate::agent::loop_executor::LoopRoundExecutor` 并 `new` 出来。

后果：`RoundExecutor` 这个端口形同虚设——实现由引擎自己 `new`，测试替身与生产实现落在同一处硬编码，换实现要改引擎。

对照：Dify 由 `DifyNodeFactory` 造节点，引擎只持有 `NodeFactory`。

### 4.3 与 K7 的关系

K7 原文：链路差异只经 `TurnSpec` 参数表达，内核内不写 `if is_agent`。

实测：**确实没有 `if is_agent`**——K7 在字面上被遵守了。但它只约束了**判断**，没约束**构造**：

| K7 管住的 | K7 没管住的 |
|---|---|
| 内核里不出现 `if is_agent` 分支 | 内核文件里 `new` 出 agent 的具体类型 |
| 链路差异进 `TurnSpec` 字段 | `TurnEvent` 里出现 agent 的词汇 |
| 参数化取代分支 | 参数化到极限也只是把分支变成表项，**词汇问题不解决** |

推论：**参数化解决「同一段代码按参数走不同分支」，不解决「不同链路需要不同词汇」。** `TurnEvent::Node(AgentNodeEvent)` 就是后者——它不是分支，是类型。

### 4.4 为什么这是编排的前置问题

`AGENT-WORKFLOW-ORCHESTRATION-ANALYSIS.md` §12 已给出：调度器要能驱动一张图，前提是「被编排的单元能自述形状」。本文从另一侧提供同一条判断的**结构证据**：

- 编排层要能表达「**任意**节点的状态」；
- 当前 `TurnEvent` 只能表达「**agent** 节点」；
- 因此当前编排能力只能编排 agent 一种执行体，无法编排 Companion / Job；
- 而 `MODEL-LEVEL-ORCHESTRATION-ANALYSIS.md` 的四种执行体（Unit / Companion / Job / Hook）恰恰要求引擎认识「节点」而不是「agent 节点」。

### 4.5 观测面（V0）带来的反向边

在 §4.1 的 12 条 `kernel → agent` 边中，由 P15 观测面净新增的是 3 条，其余为既有积累：

| 位置 | 引入的边 | 类别 |
|---|---|---|
| `kernel/ports/mod.rs` | `use crate::agent::node_event::AgentNodeEvent` | 词汇进入端口层（S3 违规） |
| `kernel/turn/emit.rs` | `use crate::agent::node_event::{AgentNode, AgentNodeDetail, AgentNodeEvent, NodeState}` | 发射器依赖链路类型 |
| `kernel/turn/runner.rs` | `use crate::agent::node_event::{AgentNode, AgentNodeDetail, NodeState, ROOT_AGENT_ID}` | 执行器依赖链路类型 |

`agent → kernel` 的 17 条中没有 V0 新增。三处性质不同：端口层那条属词汇问题（与 `ToolEvent` 同属一类），`emit.rs` / `runner.rs` 两条属事件词汇与执行器构造问题——后者与 U2 的通用 `NodeEvent` 方向同域；若只处理端口层，结果是「缝立了，事件仍绑着 agent」。

**处理这三条边不等于取消观测**：把 agent 专名从内核端口层挪走后，观测能力由适配层投影继续提供；适配层认识 agent 词汇不越界（Dify 的做法），内核端口层认识才越界。

---

## 5 判据（供规格验收）

| # | 判据 | 可机检形态 | 参考项目证据 | 本项目现状 |
| --- | --- | --- | --- | --- |
| **S1** | 反向依赖边 = 0 | 对模块路径做 grep 计数 | 四项目全 0 | `kernel↔agent` 12 / 17 |
| **S2** | 缝面有统一前缀，一次搜索可枚举 | `grep "class Dify"` / `grep "_read"` | Dify / langgraph | 无前缀，两侧直接互指 |
| **S3** | 引擎词汇是节点级通用词 | 引擎内不出现链路专名 | ComfyUI / Dify | `TurnEvent::Node(AgentNodeEvent)` |
| **S4** | 注册表在引擎、bootstrap 在产品、注册项是**数据** | 加一个条目 = 加一个目录 / 文件，不改引擎 | Dify `register_nodes()`、ComfyUI `NODE_CLASS_MAPPINGS` | 无注册表；`TurnSpec::chat/agent` 是**构造器** |
| **S5** | 每个纯模块 / 下沉点，注释写明第二个调用点或越界事实 | 注释可检索 | 三份 docstring（§2.5） | 部分已有（如 ports 注释），未成惯例 |

### 5.1 S1 的度量口径

- 只数 `use crate::<模块>` 路径边，**不数测试面**（`test_doubles.rs` 这类端口替身豁免）；
- 边按**出现次数**计（同一文件多次 import 同一模块记多次），因为每次 import 都是一次耦合点；
- 口径是 **0**；一旦口径放宽为「少量可接受」，缝就会在下次重构里被扩成大量。

### 5.2 S3 的度量口径

- 在引擎文件里搜链路专名（`agent` / `chat` / `orb` / `compaction`）；
- **例外**：`TurnSpec` 的构造器名（`chat()` / `agent()`）——它是「选择哪条链路」的唯一入口，名字本身就是策略面；
- **不算例外**：构造器体内出现 `crate::<链路>` 的具体类型。当前 `TurnSpec::agent()` 与 `agent_stop_hooks()` 正是踩这一条。

### 5.3 S4 的形状

- 「注册项是数据」意味着**新增链路不改引擎代码**：Dify 靠 `_import_node_package` 遍历目录；ComfyUI 靠节点模块被 import 时写入 `NODE_CLASS_MAPPINGS`。
- 反例：`TurnSpec::chat()` / `TurnSpec::agent()` 是构造器，新增链路必须改 `kernel/mod.rs`——这不是注册，这是硬编码。

### 5.4 S5 的形态

- 不新增文档；证据写在**代码注释**里，与实现同址；
- 参考句式：「从 X 抽出，使两处解析同一形状」/「本层拥有 bootstrap，因为必须组合 Y，且不得把 bootstrap 塞回低层原语」；
- 与 `CODE-SPLIT-PRINCIPLE.md` 的分工：那条判据回答「该不该拆」，S5 回答「拆了要留什么证据」。

---

## 6 差异对照：不能直接平移的三处

| # | 参考做法 | 差异原因 | 本项目对应的结构形态 |
|---|---|---|---|
| 1 | ComfyUI 的类属性契约 | 无类型约束，靠 `hasattr` 运行时探测；Rust 侧没有运行时属性表 | 显式 `Spec` 结构 + 编译期构造，`Option<OutputSchema>` 缺省即编译错误 |
| 2 | langgraph 的动词抽象 | 把读 / 写升为 `Runnable` 需要 channel + checkpoint 全栈；本项目无 checkpoint 概念 | 只对应「状态访问要有唯一入口」这一条，不对应它的对象代数 |
| 3 | Dify 的包边界 | `graphon` 是独立分发包；本项目是 workspace 内 crate | 拆 crate 可彻底隔离，但成本高于模块级 grep（见 U1） |

---

## 7 待定案项

以下取舍属规格职责，本文只列各项的后果对照，不给结论：

| # | 待定案项 | 选项与后果 |
|---|---|---|
| **U1** | S1 用「模块级 grep」还是「拆 crate 编译期强制」 | 前者便宜但可绕过（`#[path]`、`pub(crate)` 滥用）；后者彻底但要动 crate 结构。分界在「缝要不要跨 crate 共享」：只在本 crate 内则 grep 足够 |
| **U2** | `TurnEvent` 通用化的方向：抽出 `Node(NodeEvent)` 抽象层，还是让适配层做映射 | 前者改端口契约（影响 wire 投影输入面）；后者把映射责任推给适配层（Dify 的做法）。两者都可行，但只做一半会留下「缝立了、事件仍绑 agent」的中间态 |
| **U3** | `TurnSpec::agent()` 留在 `kernel/mod.rs` 是否成立 | 若视为「选择入口」则可保留；但按 S3 口径，构造器体内出现 agent 类型即 S3 不成立 |
| **U4** | 注册表下沉到引擎后，内核会不会变成插件框架 | Dify 的注册表只是 `dict[NodeType, dict[version, type[Node]]]`，**不持生命周期、不持状态**；分界在「注册表只做查找」 |
| **U5** | 是否给 `scripts/probe-structure.mjs` 增加反向边统计 | 它已用 `cargo metadata` 覆盖 crate 间依赖；crate 内模块边需另加一步源码 grep |

---

## 8 结构核对

参考项目的「清晰」不是风格，是三个可复述的机制：**单向的缝**、**可枚举的命名**、**被协调压力逼出来的下沉**。

本项目当前**没有缝**：`kernel` 与 `agent` 互指（12 / 17），端口层词汇已被单一链路占据。

结构后果：不做缝，编排层只能编排 agent 一种执行体；而 §12 的缺口清单与 `MODEL-LEVEL-ORCHESTRATION-ANALYSIS.md` 的四种执行体，都要求引擎认识「节点」而不是「agent 节点」。

反向边的结构分布（§4.1 实测）：

| 边集 | 条数 | 集中位置 | 性质 |
| --- | --- | --- | --- |
| `kernel/ports/mod.rs` | 2 | 端口层词汇 | 词汇进入端口层（S3 违规） |
| `kernel/mod.rs`（`agent_stop_hooks()`） | 3 | 选择入口旁的构造 | 构造器体内出现 agent 类型（S3 违规） |
| `kernel/turn/runner.rs` | 5 | 执行器 | 执行器依赖链路类型 |
| `kernel/turn/emit.rs` | 2 | 发射器 | 发射器依赖链路类型 |

两类性质：前两处是**词汇与构造**，后两处是**事件词汇与执行器构造**——正好对应 U2 与 U3 的待定案范围。这个分布说明「缝」的剩余工作不是均匀的，而是集中在两个文件的两个问题上。

---

## 参考

- 能力缺口与编排原语对照：`docs/analysis/AGENT-WORKFLOW-ORCHESTRATION-ANALYSIS.md`（§12）
- 执行体定义域（Unit / Companion / Job / Hook）：`docs/analysis/MODEL-LEVEL-ORCHESTRATION-ANALYSIS.md`
- 拆分判据：`docs/principle/CODE-SPLIT-PRINCIPLE.md`
- 内核职责与不变式：`docs/store/I36a-agent-kernel-baseline.md`
- 参考实现（稀疏检出）：
  - Dify：`references/9-workflow-orchestration/Dify/api/core/workflow/node_factory.py`、`node_runtime.py`、`node_execution.py`、`graph_topology.py`、`nodes/knowledge_index/protocols.py`
  - langgraph：`references/9-workflow-orchestration/langgraph/libs/langgraph/langgraph/pregel/_read.py`、`_write.py`
  - n8n：`references/9-workflow-orchestration/n8n/packages/workflow/src/graph/graph-utils.ts`、`src/common/get-runnable-nodes.ts`
  - ComfyUI：`references/9-workflow-orchestration/ComfyUI/execution.py`、`nodes.py`、`comfy_execution/graph.py`
- 本项目实测对象：`src-tauri/crates/assistant-agent/src/kernel/`（`mod.rs`、`ports/mod.rs`、`turn/emit.rs`、`turn/runner.rs`）、`src-tauri/crates/assistant-agent/src/agent/`（9 个文件）
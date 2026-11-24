# 工具调用范式归属与能力缺口：参考项目对照分析

> 日期：2026-11-23｜性质：**预先分析**（只核对范式归属、对照参考实现、枚举缺口与证据；**不含取舍决议**——补哪几条、按什么顺序、是否纳入编排波次、如何分期，属规格职责）
> 判据来源：`docs/principle/CODE-REVIEW-PRINCIPLE.md`、`docs/principle/CODE-SPLIT-PRINCIPLE.md`
> 事实快照：截至 2026-11-23 的仓库代码与参考项目稀疏检出（**时点记录**，不随代码演进追改；需要最新事实时以代码为准）
> 参考实现：`references/1-ai-agent/`——codex（`codex-rs/core/src/tools/`、`codex-rs/core/src/hook_runtime.rs`、`codex-rs/core/src/function_tool.rs`、`codex-rs/tools/src/tool_executor.rs`）、goose（`crates/goose-agent/src/`：`tool.rs`、`machine.rs`、`operation.rs`、`tool_approval.rs`、`pre_tool_call_hook.rs`、`post_tool_call_hook.rs`）；`references/9-workflow-orchestration/`——langgraph（`libs/prebuilt/langgraph/prebuilt/tool_node.py`）、Dify（`api/core/workflow/workflow_tool_node.py`、`web/app/components/workflow/nodes/tool/types.ts`）、n8n（`packages/core/src/execution-engine/workflow-execute.ts`、`packages/workflow/src/from-ai-parse-utils.ts`、`packages/workflow/src/interfaces.ts`）。凡标「参考」的判断只出自上述检出部分，不外推到未检出部分。
> 术语唯一出处：`docs/analysis/ORCHESTRATION-CONCEPT-SEMANTIC-TRANSFER.md` §2.5（用户可见词）· §2.6（开发原语）；本文只引用，不复述定义。
> 与既有文档的关系：**编排能力缺口**（缺哪些编排原语）见 `docs/analysis/AGENT-WORKFLOW-ORCHESTRATION-ANALYSIS.md` §12；**「缝」的代码安排**见 `docs/analysis/ORCHESTRATION-CODE-SHAPE-REFERENCE-ANALYSIS.md`；**工具调用方言（参数形态）**见 `docs/analysis/TOOLCALL-DIALECT-EXTENSION-ANALYSIS.md`。本文只回答一个问题：**工具调用这条链路上，本项目属于哪一种范式，与同范式的参考实现差哪几条。**
> 时效与终止：本文是**一次性决策输入**。工具调用链路的规格定案后整体归档冻结、不再修改；有效内容由规格承接，事实条目作为历史快照保留。

---

## 0 命题核对

| # | 命题 | 核对 | 依据 |
|---|---|---|---|
| 1 | 本项目的工具调用是「agent 内派发」范式 | **成立** | 工具身份是 `call_id`，生命周期挂在 turn 上；入口唯一为 `ToolExecutionPort` → `ToolPipeline`（§1.3、§3.1） |
| 2 | 所有参考项目用同一种方式让 agent 调用工具 | **不成立** | 存在三种互不相同的范式：agent 内派发（codex / goose）、图节点工具（langgraph / Dify）、图内附属子节点（n8n）（§1） |
| 3 | 「工具节点」是本项目独有的发明 | **不成立** | langgraph `ToolNode`、Dify `DifyWorkflowToolNode`、n8n `ai_tool` 连接三者都是既有实现（§2.3–§2.5） |
| 4 | 工具调用已经是独立的执行体 | **不成立** | 无独立取消、无独立台账行、无独立审批单元；三者都以 `call_id` 为粒度寄生在 turn 上（§3.1） |
| 5 | 取消已贯穿到「等待」层 | **不成立** | `ToolUseContext.cancel` 字段存在，但 `pipeline.rs` 全文无 `cancel`；审批等待只有 `tokio::time::timeout`（§4 G4） |
| 6 | 并发性由工具自述 | **部分成立** | `is_concurrency_safe` 存在，但默认实现回落为 `is_readonly()`；「可并行但非只读」无法表达（§4 G2） |
| 7 | 本轮工具可见性在本轮内冻结 | **成立** | turn 起始冻结 `state.tools`，仅 escalation 时追加并合并（§5 非缺口第 1 条） |
| 8 | 工具结果回注模型上下文 | **成立** | 经 `result_for_assistant` + 消息行工具块回注；与 codex 的 `ResponseInputItem` 等价而非同一（§5 非缺口第 2 条） |
| 9 | 注册表支持分层与命名空间 | **不成立** | 同 id 覆盖、同 name 后者优先 + `log::warn`；无 trusted/external 分层、无 namespace（§4 G8） |
| 10 | 存在工具级扩展点（钩子） | **不成立** | 七个中间件全为 turn 级；工具级只有 `PipelineEventSink` 观测事件（§4 G6） |
| 11 | 结果预算可按工具定制 | **不成立** | `ToolResult` 单一形状 + 统一 `ToolOutputCapMiddleware`（§4 G3） |
| 12 | 编排路线图当前已把工具调用节点纳入波次 | **不成立** | `docs/store/I69-agent-orchestration-roadmap.md` 的范围外清单明列「工具调用节点」一律不进入任何波次，波次仍为 R0–R5（§3.3） |
| 13 | 缺口可以全部零 wire 变更补齐 | **部分成立** | 九项中七项为纯 Rust 改动；G3 的元数据、G7 的来源、G8 的命名空间若要上 wire 则涉契约（§6） |

一句话：**本项目与 codex / goose 属同一范式（agent 内派发），因此下面的九项全是「同范式内的能力差距」而非范式分歧；参考项目中只有 langgraph / Dify / n8n 持有「工具即图节点」的建模，那条建模补的是治理层空档，不改变本项目的派发主路。**

---

## 1 三种范式：判据是「工具身份归谁」

### 1.1 判据

判断一个实现属于哪种范式，只看两件事：

| 判据 | 含义 |
|---|---|
| **工具身份** | 一次工具调用的标识是什么——`call_id`（挂 turn），还是图节点 ID（挂 run） |
| **参数来源** | 参数由模型在运行时生成，还是由人预先写死（或由人写模板、模型只填空槽） |

两者交叉给出三种范式，互不等价：

| 范式 | 工具身份 | 参数来源 | 结果去向 | 代表 |
|---|---|---|---|---|
| **A. agent 内派发** | `call_id`（turn 内） | 模型生成全部参数 | 回注模型上下文 | codex、goose、**本项目** |
| **B. 图节点工具** | 图节点 ID（run 内） | 人写模板（可引用上游输出） | 图状态 / 下游节点 | langgraph、Dify |
| **B′. 图内附属子节点** | 子节点 ID，运行时被 agent 重入 | 人定槽位，模型填 `$fromAI` | 回投 agent 的 `ai_tool` 通道 | n8n |

### 1.2 三者的能力边界

| 维度 | A | B | B′ |
|---|---|---|---|
| 参数自由度 | 高（模型自由组合） | 低（模板固定，或仅引用上游） | 中（槽位由人定，值由模型填） |
| 工具集裁剪粒度 | 会话级（可表达「同一工具不同会话不同权限」） | 工作流级（静态配置） | 工作流级 |
| 审批上下文完整度 | 高（有真实参数、intent、工作区、turn/iteration） | 依赖工作流级授权 | 依赖 agent 侧审批 |
| 多步闭环 | 有（结果回注后模型决定下一步） | 无（结果只喂下游） | 有（结果回投 agent） |
| 独立取消 / 独立落库 | **无**（只能取消整个 turn） | 有（节点级） | 部分（子节点可独立执行） |
| 扩展成本 | 低（实现 trait 即可，零契约变更） | 高（新增节点类型 = 新增 schema + DTO） | 中 |

### 1.3 本项目归属

**本项目是 A 范式，且多了一条参考项目都没有的入口。**

| 入口 | 范式 | 证据 |
|---|---|---|
| 轮内工具链 | A | `ToolExecutionPort` → `ToolPipeline`，工具身份 `call_id`（§3.1） |
| ToolBox 直调 | **无对应范式** | `os_tool_exec` 直接 `invoke`，`ToolUseContext::for_toolbox(None)`，记 `bypassed` 审计（§3.2） |
| 编排节点（提案态） | 若落地则为 B′ 的变体 | 当前未在路线图波次内（§3.3） |

**「多一条 ToolBox 直调」是风险面而非能力面**：参考项目均不提供「无会话、无审批、无工作区边界」的裸直调入口。它的存在意味着「同一次工具调用」在系统内有两种可信度完全不同的来源，而台账里没有字段区分（§4 G7）。

---

## 2 参考实现取证

### 2.1 codex：A 范式最完整的实现

三层结构，逐层对应我们的组件：

| 层 | codex 符号 | 本项目对应 | 差异 |
|---|---|---|---|
| 全局注册表 | `ToolRegistry`（`core/src/tools/registry.rs`），`RegisteredTool { runtime, exposure, model_spec }` | `ToolRegistry` + `RegisteredTool { id, name, tool }` | 我们无 `exposure`、无 `model_spec` 覆盖 |
| 每步可见性快照 | `ToolRouter`（`core/src/tools/router.rs`），持 `model_visible_specs` / `tool_mode` | turn 起始冻结 `state.tools` | 等价；但节点链路尚未做同样快照 |
| 唯一派发入口 | `dispatch_tool_call_with_state(.. cancellation_token ..)` | `ToolExecutionPort::execute` | 我们的令牌不进入等待层 |

codex 有而本项目没有的九项机制：

| 机制 | 符号位置 | 作用 |
|---|---|---|
| 可见性四态 | `ToolExposure::{Direct, Deferred, CodeModeOnly, Hidden}`（`registry.rs`） | 延迟暴露：工具不进 prompt，需要时检索发现 |
| 检索式发现 | `ToolSearchHandler` + `bm25::Embedder`（`core/src/tools/handlers/tool_search.rs`） | BM25 检索工具描述，替代「全量塞 prompt」 |
| 动态工具延迟加载 | `DynamicToolHandler::from_parts`：`defer_loading` → `ToolExposure::Deferred`（`handlers/dynamic.rs`） | 第三方/动态工具默认不进 prompt |
| 并发自述 | `supports_parallel_tool_calls()`（`tools/src/tool_executor.rs`、`registry.rs::supports_parallel_tool_calls`） | 工具声明自己能否并行 |
| 取消收尾自述 | `finishes_on_cancellation()`（`registry.rs`） | 取消时「等工具自己收尾」还是「立即 abort」 |
| 取消贯穿 | `tokio::select!` race 令牌 + `abort()` + `aborted_response`（`core/src/tools/parallel.rs`）；`OrCancelToolExt::or_cancel_tool`（`core/src/function_tool.rs`） | 取消在 await 层生效，不依赖工具自觉 |
| 工具级生命周期钩子 | `notify_tool_start / notify_tool_finish / notify_tool_aborted / notify_command_start / process_mcp_tool_result`（`core/src/tools/lifecycle.rs`），走 `tool_lifecycle_contributors()` | 可插拔的工具级观测与副作用 |
| pre/post tool use 钩子 | `run_pre_tool_use_hooks` / `run_post_tool_use_hooks`（`core/src/hook_runtime.rs`），含 `ContextInjectingHookOutcome` | 执行前注入上下文、执行后校验或改写 |
| 结果多视图与预算覆盖 | `log_output` / `success_for_logging` / `to_response_item` / `code_mode_result` / `tool_result_metadata` / `fallback_token_limit_override`（`registry.rs`） | 工具自己声明日志形态与 token 预算 |
| 注册表分层与命名空间 | `register_trusted` / `prepend_trusted` / `register_external`、`record_collision` / `first_collision`、`ToolName { namespace, name }`、`mcp_namespaces()`（`registry.rs`） | 第三方工具与内置工具的优先级、冲突记录、同名隔离 |
| 调用来源与归因 | `ToolInvocation.source` → `extension_tool_call_source(...)`（`lifecycle.rs`）；`telemetry_tags` / `is_third_party_tool` / `mcp_server_name`（`registry.rs`） | 按来源切分台账与 telemetry |

**关键事实：codex 里没有「工具节点」。** 工具永远是 agent 内部派发，身份止于 `call_id`。

### 2.2 goose：A 范式，MCP 统一形状 + 批级审批

| 事实 | 位置 | 对本文的意义 |
|---|---|---|
| 工具形状直接用 MCP 的 `ToolBase` / `AsyncTool` / `SyncTool` | `crates/goose-agent/src/tool.rs` | 与本项目 `McpTool` 桥接同构，验证 A 范式下工具契约可以是「schema + serde」 |
| 会话是状态机，步骤是 `Operation`，`Step::Operation` 带 `CancellationToken` | `machine.rs`、`operation.rs` | 取消在步骤级贯穿，不是靠工具自觉 |
| 审批是 trait 且一次批回多个决策 | `tool_approval.rs`：`ToolApproval::on_confirmation`，`Permission::{AllowOnce, AlwaysAllow}` | 与我们的 `ApprovalDecision::{Approve, Deny, AllowSession, AllowAlways}` 相比，goose 只有两级正向决策 |
| **工具级钩子是独立 trait** | `pre_tool_call_hook.rs`（`PreToolCallHook::before_call` → `PreToolCallDecision::{Allow, Deny}`）、`post_tool_call_hook.rs`、`stop_hook.rs` | 直接反证本项目「无工具级扩展点」 |

**goose 也没有「工具节点」。**

### 2.3 langgraph：B 范式的教科书

| 事实 | 位置 | 含义 |
|---|---|---|
| `ToolNode` 是图节点，输入 `AIMessage(tool_calls=[...])` 或 `list[ToolCall]` | `libs/prebuilt/langgraph/prebuilt/tool_node.py` | 工具调用被建模为「一次图节点执行」 |
| 一次吃多个 tool_call，`executor.map` 并行 | 同上 `_func` | 与我们的批内 `join_all` 同形 |
| 输出 `ToolMessage` 或 `Command` | 同上 | `Command` 可直接改图状态——**本项目不允许**（工具节点不改上游） |
| `handle_tool_errors` 决定「抛异常 / 转 ToolMessage / 只吞指定类型」 | 同上 | 错误处置是节点级策略，不是工具级 |
| `InjectedState` 让工具读图状态 | 同上 | 与 `ToolUseContext` 同构的注入 |

**本项目工具节点若落地，严格是 langgraph 的子集**：单调用、参数常量、结果不回注、不改上游。

### 2.4 Dify：B 范式，参数可引用上游

| 事实 | 位置 | 含义 |
|---|---|---|
| `DifyWorkflowToolNode extends ToolNode` | `api/core/workflow/workflow_tool_node.py` | 工具节点是容器化执行体，独立于 agent turn |
| 参数从 `variable_pool` 解析 | 同上 `_generate_parameters(... variable_pool ...)` | **上游输出可直接喂工具参数** |
| DTO 为 `tool_parameters: ResourceVarInputs` | `web/app/components/workflow/nodes/tool/types.ts` | 参数在 schema 层就是「变量引用」而非常量 |
| 节点结果带 `metadata` + `llm_usage` | 同上 | 节点级成本归因 |

**这直接说明：本项目「工具节点不引用上游输出」是与主流 B 范式的主动偏离，不是行业惯例。**

### 2.5 n8n：B′ 范式，工具卡片即 agent 可用工具

| 事实 | 位置 | 含义 |
|---|---|---|
| 工具是独立节点，经 `NodeConnectionTypes.AiTool` 挂到 Agent | `packages/workflow/src/interfaces.ts` | 「工具卡片 = agent 可用工具接入」是既有建模 |
| agent 运行时**重入**该节点，注入 `inputOverride.ai_tool = [[{ json: { query, toolCallId } }]]` | `packages/core/src/execution-engine/workflow-execute.ts`（`isAiToolExecution`、`rewireOutputLogTo`） | 节点被当作一次工具调用执行，执行完回投 agent |
| 参数用 `$fromAI('placeholder')` 标记「这格由模型填」 | `packages/workflow/src/from-ai-parse-utils.ts` | 槽位由人定、值由模型填 |
| 工具节点失败默认**不中断工作流**，错误投到 `ai_tool` 通道交给 agent | `workflow-execute.ts`（`onError !== 'stopWorkflow'` 分支） | 错误处置是「降级为可观测事实」而非硬失败 |

---

## 3 本项目现状取证

### 3.1 轮内工具链（A 范式，主路）

链路：`ToolExecutionPort` → `ToolPipeline::execute` → 批切分 → `run_call` 五阶段 `find → validate → gateway → execute → persist` → 事件转发。

| 环节 | 位置 | 事实 |
|---|---|---|
| 端口 | `src-tauri/src/adapters/tool_exec.rs`（`PipelineToolExecution`） | 唯一接入点；持 `ToolPipeline` 与 `ApprovalWaitRegistry`；入参含 `ToolExecCtx` / `observer` / `escalations` |
| 批切分 | `crates/assistant-tools/src/runtime/pipeline.rs::partition_tool_batches` | 按 `is_concurrency_safe` 决定能否并入同一批；不安全即独占一批 |
| 批内并发 | 同上 `execute`（`futures::future::join_all`） | 结果按原始顺序回填，与切分无关 |
| 阶段事件 | 同上 `PipelineEvent::{Started, StageEntered, Finished, Failed}` | 工具级**观测**事件，无扩展语义 |
| 超时 | `runtime/exec.rs::invoke` | `manages_own_execution_timeout()` 为真则不套 deadline；否则 `tokio::time::timeout` |
| 审批 | `runtime/gateway/mod.rs` + `gateway/decision.rs` | `ApprovalRequest` 携带 `tool_call_id` / `conversation_id` / `turn_id` / `iteration` / `intents` / `context`；等待为 `tokio::time::timeout` |
| 审批决策集 | `gateway/decision.rs::ApprovalDecision` | `Approve` / `Deny` / `AllowSession` / `AllowAlways` |
| 结果 | `contract/result.rs::ToolResult` | `{ok, data, result_for_assistant, error, error_kind, persisted}` 单一形状 |

工具契约（`contract/trait.rs`）：

| 方法 | 默认行为 |
|---|---|
| `audience()` | `ToolAudience { agent, toolbox }` 双维度准入 |
| `tier()` | 层级上限 |
| `companion_of()` | 伴随工具（`super_read` / `super_edit`），不参与白名单展开 |
| `behavior_hints()` | `{ read_only, destructive, open_world }` |
| `is_concurrency_safe()` | **默认 `= is_readonly()`** |
| `manages_own_execution_timeout()` | 默认 `false` |
| `sandbox_enforcement()` | 进审批键，防「强边界批准、弱边界复用」 |
| `validate_input()` | 校验失败不弹审批框 |

准入（`src-tauri/src/adapters/tool_resolver.rs::resolve_available_tools`）：`tier → audience → deny-by-default → 身份过滤（排除伴随）→ 名单过滤`，未知名静默忽略。

### 3.2 ToolBox 直调（无对应范式）

`src-tauri/src/cmd/os_tool_exec.rs`：

- 直接 `invoke(...)`，不经 Gateway、不弹审批、无工作区边界（`ToolUseContext::for_toolbox(None)`）；
- 执行前写 `AuditEntry::bypassed(id, "toolbox")`，与「策略允许」区分；
- 审计失败只 `warn`，执行照常。

**后果**：同一次工具调用存在两种可信度不同的来源，而台账与 telemetry 无字段区分（见 §4 G7）。

### 3.3 编排节点（提案态）

`docs/store/I69-agent-orchestration-roadmap.md` 当前波次为 R0–R5，其范围外清单明列「工具调用节点」一律不进入任何波次，模板 schema 不为它预留字段。

因此本文讨论的「工具节点」是**提案态建模**，不是既有实现；其范式归属（B′ 变体）与可行性不改变 §3.1 的主路事实。

---

## 4 缺口清单

九项，按性质分三层。**「性质」列区分「能力」（参考项目能做、我们不能）与「语义」（同一能力我们做得不完整）。**

### 4.1 契约层：工具能声明什么

#### G1 延迟暴露与检索式发现

| 项 | 内容 |
|---|---|
| codex 做法 | `ToolExposure` 四态 + `defer_loading` 自动落 `Deferred` + `tool_search` 走 BM25 |
| 本项目现状 | 只有 `ToolAudience` 二元 + `tier` + 白名单；`request_tool` 的文档明示「不改变本轮工具集：不触碰注册表、不触碰 provider 请求」，只把手册投影成文本（单份 2000 字符 / 合计 8000 字符上限） |
| 后果 | 模型无法「发现一个本轮没给它的工具然后调用它」；工具一多只能靠「默认少给」硬压 prompt |
| 性质 | 能力 |
| 成本 | 纯 Rust（复用既有 escalation 骨架与 `tool_catalog`） |

#### G2 工具自述并发与取消语义

| 项 | 内容 |
|---|---|
| codex 做法 | `supports_parallel_tool_calls()` + `finishes_on_cancellation()` 两个独立自述 |
| 本项目现状 | `is_concurrency_safe()` 默认回落 `is_readonly()`——**并发性被硬绑到只读性**；`agent_shell` 直接返回 `false` 并注释「进程执行争用资源」 |
| 后果 | 「只读且可并行」能跑并行纯属巧合；「可并行但非只读」无法表达；取消时「等工具收尾还是立即 abort」无声明位 |
| 性质 | 能力 |
| 成本 | 纯 Rust（trait 默认方法，零 wire） |

#### G3 结果多视图与 per-tool 预算

| 项 | 内容 |
|---|---|
| codex 做法 | `log_output` / `success_for_logging` / `to_response_item` / `code_mode_result` / `tool_result_metadata` / `fallback_token_limit_override` |
| 本项目现状 | `ToolResult` 单一形状 + 统一 `ToolOutputCapMiddleware` 裁预算；`compact` 信封是唯一展示事实源 |
| 后果 | `read` 的窗口、`grep` 的命中集、`shell` 的日志，合理预算差异很大，只能靠统一阈值 + 落盘兜；工具无法声明「我这个结果天然该小」 |
| 性质 | 能力 |
| 成本 | 纯 Rust；若 `metadata` 上 wire 则需 `npm run contract:gen` |

### 4.2 执行层：链路能做什么

#### G4 取消贯穿到「等待」层（现状缺陷）

| 项 | 内容 |
|---|---|
| codex 做法 | `tokio::select!` race 令牌，按 `finishes_on_cancellation` 决定「等收尾」或 `abort()` 并产出 `aborted_response`；底层 `or_cancel_tool` 兜成 `RespondToModel("tool call cancelled")`；审批被取消是**专门 outcome**，不等价于超时 |
| 本项目现状 | `ToolUseContext.cancel` 字段存在；但 `pipeline.rs` 全文无 `cancel` 字样；审批等待只有 `tokio::time::timeout`；`ToolErrorKind::Cancelled` 只由工具自己产出（`ask_user`、`shell`） |
| 后果 | 取消 run 时，卡在审批框或批内排队位置的调用不被中断，要等审批超时；结果无法区分「用户取消」与「审批超时」 |
| 性质 | **语义缺陷**（现状即有，非新能力） |
| 成本 | 纯 Rust |

#### G5 长时 / 交互式进程工具

| 项 | 内容 |
|---|---|
| codex 做法 | `exec_command` + `write_stdin` + `process_manager`（网络审批与进程退出竞速）+ `wait_for_environment` + `sleep` |
| 本项目现状 | `agent_shell` 一次性阻塞 + 自管超时，无 stdin 续写、无后台会话 |
| 后果 | 构建 / 测试 / 起服务这类长任务只能「等一次超时再看输出」，无法分段读、无法中途输入 |
| 性质 | 能力 |
| 成本 | 纯 Rust（新增工具，与编排无耦合） |

#### G6 工具级扩展点

| 项 | 内容 |
|---|---|
| codex 做法 | `lifecycle.rs` 五类通知（start / finish / aborted / command_start / mcp_result）+ `hook_runtime.rs` pre/post tool use（含上下文注入） |
| goose 做法 | `PreToolCallHook` / `PostToolCallHook` / `StopHook` 独立 trait，`PreToolCallDecision::{Allow, Deny}` |
| 本项目现状 | 七个中间件（Projection / Microcompact / WrapUpRestore / HardTrim / ToolOutputCap / ToolOutcomeCapture / RepeatedFailure）**全为 turn 级**；工具级只有 `PipelineEventSink` 观测事件 |
| 后果 | 无法做「某工具执行前注入规则、执行后校验或改写结果、按工具统计」；工具一多就会逼着改 pipeline |
| 性质 | **扩展性** |
| 成本 | 纯 Rust |

#### G7 调用来源与归因

| 项 | 内容 |
|---|---|
| codex 做法 | `ToolInvocation.source`（lifecycle 直接映射）+ `telemetry_tags` + `is_third_party_tool` + `mcp_server_name` |
| 本项目现状 | `ToolUseContext` 有 `session_id` / `turn_id` / `iteration` / `tool_call_id`，**无来源维度**；三条入口（轮内 / ToolBox / 节点提案）靠不同函数隐式区分 |
| 后果 | 台账里三条入口的调用混在一起无法归因；telemetry 无法按来源切分。这是引入节点后最先疼的地方 |
| 性质 | 治理 |
| 成本 | 运行时字段零成本；进台账 / telemetry 需契约 |

### 4.3 管理 / 上下文层

#### G8 注册表分层与命名空间

| 项 | 内容 |
|---|---|
| codex 做法 | `register_trusted` / `prepend_trusted` / `register_external` 三档；`record_collision` / `first_collision`；`ToolName { namespace, name }`；`mcp_namespaces()` |
| 本项目现状 | `register` 同 id 覆盖、同 name 后者优先 + `log::warn`；无分层、无 namespace |
| 后果 | 第三方 / 动态工具与内置同名时策略不可表达；多个 MCP server 的同名工具只能靠重命名约定硬躲 |
| 性质 | 治理 |
| 成本 | 内部结构纯 Rust；若名字上 wire（`ToolSpec.name`）则涉契约——**唯一需要产品决策的一项** |

#### G9 模型可见的上下文预算

| 项 | 内容 |
|---|---|
| codex 做法 | `get_context_remaining` / `new_context_window` 两个工具 |
| 本项目现状 | microcompact 全自动，模型无手段自感知余量 |
| 后果 | 撞墙才被中间件兜，体感上「突然被截断」 |
| 性质 | 体验 |
| 成本 | 纯 Rust（新增工具） |

---

## 5 明确不算缺口的项

避免把「不同」误判为「缺」。

| 项 | 结论 | 依据 |
|---|---|---|
| 每步可见性快照 | agent 链路**已对齐** codex | turn 起始冻结 `state.tools`，仅 escalation 时 `append_escalations` 合并并回写 `state.request.options.tools`。**但节点链路需在 run 起始做同样快照**，否则同一 run 内不同节点看到不同工具集 |
| 结果回注方式 | 与 codex 等价，非缺口 | 本项目用 `result_for_assistant` + 消息行工具块；codex 用 `ResponseInputItem` + `code_mode_result` |
| `ToolAudience` 双维度准入 + ToolBox 直调 | **本项目独有**，参考项目无对应物 | 能力面；同时是风险面（无会话、无审批、无边界，§3.2） |
| 审批决策集粒度 | 本项目**更细** | `AllowSession` / `AllowAlways` 两级正向决策；goose 只有 `AllowOnce` / `AlwaysAllow` |
| 批内并发实现 | 同形 | 本项目 `join_all` 按原始顺序回填；langgraph `executor.map` |
| 伴随工具升级机制 | 同构于 codex 的权限申请工具 | codex `RequestPermissionsHandler`；本项目 `request_tool` + `companion_of` + escalation |

---

## 6 缺口之间的依赖与成本事实

只陈述依赖关系与成本性质，不排序、不决议。

| 关系 | 事实 |
|---|---|
| G4 ↔ 节点链路 | 任何以「取消按节点归因」为退出条件的波次，其前置是 G4——否则取消只能落在 turn 边界，节点级归因无从产生 |
| G7 ↔ 节点链路 | 任何把工具调用写入独立台账的波次，其前置是 G7——否则台账无法区分轮内调用与节点调用 |
| G2 ↔ G4 | G4 的「等收尾还是 abort」判定**需要** G2 的 `finishes_on_cancellation`；缺 G2 时 G4 只能一刀切 abort |
| G1 ↔ 注册表 | G1 的检索式发现需要注册表提供「可检索的描述元数据」；与 G8 的 namespace 正交 |
| G3 ↔ 中间件 | G3 落地即把预算判定从 `ToolOutputCapMiddleware` 移到工具自述，二者不能同时为唯一事实源 |
| G5 / G9 | 与其余各项零耦合，可独立成立 |
| G8 | 唯一涉及 `ToolSpec.name` 生成方式的一项；其余八项不动 wire |

---

## 7 判据小结

四条可复述的事实判断：

1. **范式归属**：本项目与 codex / goose 同属 A 范式（agent 内派发）。工具身份是 `call_id`，生命周期挂 turn，参数由模型生成，结果回注上下文。
2. **A 范式的优势是能力层的**：参数自由度、会话级工具裁剪、审批上下文完整度、多步闭环、扩展零契约成本。
3. **A 范式的局限是治理层的**：工具调用无独立身份——不能独立取消、不能独立落库、不能独立编排；一旦脱离对话只能走 `os_tool_exec` 的裸直调。
4. **九项缺口全部落在「同范式内」**：七项纯 Rust、两项涉契约（G8 的 `ToolSpec.name`、G3 的元数据）；其中 G4（取消贯穿）与 G7（调用来源）是现状缺陷，其余是能力或扩展性差距。

第 3 条正是参考项目中 langgraph / Dify / n8n 转向 B / B′ 范式的动因；第 4 条说明该动因在本项目里**不构成换范式的理由**——补齐 G4 / G7 后，治理层空档可由既有链路的扩展承接，无需第二条执行路径。
# I45 — 模型能力事实规格（止损、三态与主体 key）

| 项 | 值 |
|---|---|
| 状态 | Draft |
| 日期 | 2026-11-13 |
| 复核 | 2026-11-13 逐条对照代码施工级复核：核心问题（C1 四处止损、C2 三态、C3 档位与限额）全部仍在，结论不变；修正 8 处落点/依赖细节（前端派生文本同构、不透明推理落点、discovery 三个写者、`ModelFacts.attachment`、DTO 同一提交、可表达档位随模型区分、限额结构类型不一致、验收层级），并补 F13 / F14；第二轮复核再修正 5 处：`Provenance` 去掉 `AdapterKnown`（无写者且会把 PDF 误升为 `Supported`）、合并顺序与判定链改为按来源排序（不再是「先命中先返回」）、`AttachmentLimits` 字段可见性、`content_text_preview` 截断口径、前端存量回退口径的引用 |
| 关联 | [I45 任务清单](./I45-model-capability-facts-tasklist.md)（任务级进度唯一来源）；`MODEL-CAPABILITY-EVOLUTION-ANALYSIS.md`（现状分析，已归档，只读，本文不回改）；`docs/ARCHITECTURE.md`；I36a 内核基线（投影与预算口径）；I44 应用目录管理规格（附件存储与边界）；I38 阶段总纲（编号与文档规范） |
| 影响范围 | `assistant-providers`（新增 `capability` 模块；`vision` / `model_catalog` / `types::config` / `base` / `openai`·`gemini`·`anthropic` 三个 `discovery` / `client/quirks` / `stream/stats`）；`assistant-storage`（`content_codec`）；`assistant-agent`（`kernel/request`、`multimodal`）；`src-tauri`（`cmd/provider`、`cmd/attachment`、`ipc`、`protocol/dto/settings`、`protocol/dto/storage`）；前端（`src/settings/provider`、`src/chat/attachment`、`src/chat/core`、`src/chat/ConversationView`、`src/chat/selectors`）；生成物 `src/types/protocol.contract.ts` |

> 本文只落三件事：**止损**（当前会误判或静默丢数据的四处）、**能力事实的可信形态**（三态 + 来源 + 主体 key）、**词汇型前提**（去枚举白名单、限额单一来源）。
> 不落现状分析里的 E2 / E3 / E5–E8 / E10，也不落 E1 的落库与回放。未落项各自的触发条件与归属见 §10；统一判据是「当前没有写者或没有消费方，加了就是死字段」。
> 现状分析的 §2 基线有 4 处表述漂移、1 处断点位置前移，§2.2 有 2 个字段在本仓库资产里并不存在；本文按 2026-11-13 复核后的事实写。

---

## 1 概述

能力判定的现状是：**一个布尔值承担了三种语义**，且这个布尔值没有主体、没有来源、没有时间。

1. **三态被压成两态**：目录未收录的模型走兜底 `["text"]` / `false`，于是「不知道」被写成了「不支持」。调用方无法区分「确定不支持」与「没数据」，只能一律按不支持处理。
2. **事实没有主体**：`resolve_*_from_stored` 只按 `model_id` 查，不看 provider 也不看端点形态；内置目录遍历所有 provider 后按 `model_id` 覆盖写入，后写者胜。同一模型在原生端点与兼容端点上结论不同时，只能有一个胜出。
3. **数据在链路上丢**：非文本内容块在空判定与派生文本处被静默丢弃（用户可见的拒绝）；`models_json` 一个坏项让整表变空；不透明推理在转换点被静默丢弃。
4. **词汇表被硬编码**：目录声明的推理档位经白名单过滤，端点可表达档位又在适配器里另有一套硬编码；两者之差被静默吞掉。

交付三批：

- **C1 止损**（§5）：四项语义修正，无 schema、无 DTO 变更，可独立合入。
- **C2 能力事实**（§6）：`CapabilityFact`（三态 + 来源）+ `(provider_id, endpoint_shape, model_id)` key + `models_json` 可选字段 + `ModelConfigDto` 契约。
- **C3 词汇型前提**（§7）：去枚举白名单、附件限额单一来源、未消费字段登记。

## 2 复核后的事实（2026-11-13）

### 2.1 与现状分析 §2 的差异

| 现状分析说法 | 复核结果 | 本文处置 |
|---|---|---|
| P1 不透明推理「已捕获，无落库无回放」 | 成立，且断点**更靠前**：`UnifiedResponse → ChatDelta` 转换处即丢，之后没有任何环节见过它 | C1-4：把丢弃变为可观测 |
| P2 `tool_calls` / `tool_call_id` 在重建时被置空 | 已从存储恢复；断点只剩 `thinking_signature`（无存储列）与 `reasoning`（有列、重建不回灌） | 不动，记入 §10 |
| P3 推理持久化开关由 UI 模式决定 | 落库无条件（非空即存）；模式只控制是否产生推理 | 不动 |
| P6 流协议工具字段单槽位 | provider 内部已有 `delta_tool_call_index`；但对外 `ChatDeltaPayloadDto` 没有任何工具字段 | 结论不变：对外仍无法表达并发 item |
| P9 预算口径取满窗口 | 现为 `窗口 − max_tokens` | 不动 |
| P4 / P5 / P7 / P8 | 成立 | C1 / C2 / C3 处置 |

### 2.2 关键事实（逐条可核）

| # | 事实 | 位置 |
|---|---|---|
| F1 | 前端允许「只发附件不写文字」（`if (!hasText && !hasAttachments) return;`），产出无 text 块的 content | `src/chat/message/MessageInput.tsx`、`src/chat/core/contentBlockUtils.ts` |
| F2 | 后端空判定只认 text 块，无 text 块即判空 → 用户看到 `chat.empty_content` | `src-tauri/crates/assistant-storage/src/content_codec.rs`、`src-tauri/src/cmd/agent/mod.rs` |
| F3 | `content_plain_text` / `content_text_preview` 只取 text 块，其余块静默丢弃（标题、预览、搜索都受影响）；前端 `extractPlainText` 同构，影响自动标题兜底与历史行预览（另有 `MessageBubble::startEdit` 用它回填编辑框，语义必须保持「只取 text」） | `src-tauri/crates/assistant-storage/src/content_codec.rs`、`src/chat/core/contentBlockUtils.ts` |
| F4 | `parse_stored_models` 用 `serde_json::from_str(...).unwrap_or_default()`：一个坏项让整表为空 | `src-tauri/crates/assistant-providers/src/types/config.rs` |
| F5 | `models_json` 由前端 `save_provider` 原样写回；不在 `ModelConfigDto` 里的字段会被 round-trip 抹掉 | `src-tauri/src/protocol/dto/settings.rs`、`src/services/providerService.ts` |
| F6 | `input_modalities_for` 对目录未收录模型返回 `["text"]`（兜底伪造了一个事实） | `src-tauri/crates/assistant-providers/src/vision.rs` |
| F7 | `resolve_support_vision` 把 Unknown 与 Unsupported 压成 `false` | 同上 |
| F8 | 内置目录遍历所有 provider 后按 `model_id` 覆盖写入，后写者胜 | `src-tauri/crates/assistant-providers/src/model_catalog.rs` |
| F9 | PDF 能力已有「端点维度」判定：`ProviderKind` 决定能否原生发 PDF | `src-tauri/crates/assistant-agent/src/kernel/request.rs` |
| F10 | 目录档位白名单 `KNOWN_EFFORTS` 只有 6 个值；通用端点可表达档位只有 3 个（low / medium / high） | `model_catalog.rs`、`src-tauri/crates/assistant-providers/src/client/quirks.rs` |
| F11 | 附件限额三处硬编码：后端 agent 侧常量、后端命令层常量、前端常量 | `assistant-agent/src/multimodal.rs`、`src-tauri/src/cmd/attachment.rs`、`src/chat/attachment/types.ts` |
| F12 | `ChatDelta` 的构造点集中在 `chat_delta.rs` 的 3 处字面量（测试用 `..Default::default()`），转换函数本身丢弃 `model_response_replay` | `src-tauri/crates/assistant-providers/src/types/chat_delta.rs` |
| F13 | `ModelConfigDto` 带 `deny_unknown_fields`，且既是 `list_providers` 输出又是 `save_provider` 输入；存储结构与 DTO 任一方向字段不同步都会让列表项被跳过或字段失效 | `src-tauri/src/protocol/dto/settings.rs`、`src-tauri/src/cmd/provider.rs` |
| F14 | `input_modalities_for` / `resolve_support_vision` 的调用方是**三个** discovery 文件（openai 两处、gemini 一处、anthropic 一处） | `openai/discovery.rs`、`gemini/discovery.rs`、`anthropic/discovery.rs` |

### 2.3 目录资产实际携带的字段

`src-tauri/crates/assistant-providers/assets/models-dev.json` 实际存在：`attachment`（bool）、`modalities.input`、`modalities.output`、`reasoning`、`tool_call`、`reasoning_options`、`limit.context`、`limit.output`、`cost.input`、`cost.output`、`family`、`release_date`、`open_weights`。

**不存在**：`structured_output`、`limit.input`、`status`、`knowledge`。

> 所以现状分析 §2.2 的「未消费字段」里有两项是无源之水。本文只对**实际存在**的字段规划消费；`limit` 只有 context / output，输入预算口径仍是 `窗口 − max_tokens`。

## 3 目标与非目标

### 3.1 目标

| # | 交付 | 批次 |
|---|---|---|
| G1 | 纯附件 / 纯文件消息不再被判为空 | C1 |
| G2 | 派生文本（标题、预览、搜索）对非文本块给出占位符，不静默丢弃 | C1 |
| G3 | 模型列表逐项容错：坏项跳过并记日志，不再整表变空 | C1 |
| G4 | 不透明推理的「已捕获但未落库」变为显式可观测 | C1 |
| G5 | 能力事实从 `bool` 升级为「三态 + 来源」 | C2 |
| G6 | 能力事实 key 从 `model_id` 升级为 `(provider_id, endpoint_shape, model_id)`，合并策略显式 | C2 |
| G7 | 目录档位不再被白名单静默吞掉；目录档位与端点可表达档位的差集显式呈现 | C3 |
| G8 | 附件数量 / 体积限额在前后端只有一个来源 | C3 |

### 3.2 非目标

- 不做 `ContentBlock::Opaque` / `Artifact` 变体，不落库不透明推理（E1 需独立规格）。
- 不给 `ChatDeltaPayloadDto` 加 `item_id` / `item_kind` / `item_phase`（E2；当前对外 DTO 连工具字段都没有，占位无承载点）。
- 不引入 `session_state` / `session_ref` / `revision` / `valid_until`。
- 不消费 `structured_output`、`limit.input`（本仓库资产不携带，§2.3）。
- 不做能力协商（E5）、运行时探测（E8）、多模型组合（E6）、策略层覆盖（E7）。
- 不改历史重建（P2 断点属 E1）；不改预算求解（P9 属 E4）。
- 不把 `models_json` 迁成结构化表；仍是 per-provider JSON，仍由前端 round-trip。
- 不引入新依赖，不改 `models-dev.json` 的既有字段语义。

## 4 能力事实的形态

### 4.1 三态与来源

新增 `src-tauri/crates/assistant-providers/src/capability.rs`：

```rust
pub enum Support { Supported, Unsupported, Unknown }

pub enum Provenance {
    ApiDeclared,   // 拉取模型列表时由 Provider 响应字段得出
    Catalog,       // 内置 models-dev 资产
    Snapshot,      // per-provider 快照；存量数据由旧布尔 / 模态派生，未记录原始来源
    Fallback,      // 兜底默认值
}

pub struct CapabilityFact {
    pub support: Support,
    /// wire 上的键名是 `source`（`#[serde(rename = "source")]`），与 §6.2 的 `models_json` 及 DTO 一致
    pub provenance: Provenance,
}
```

**没有 `AdapterKnown` 变体**（这一点是硬约束，不是省略）：

- 本批唯一「适配器已知」的事实是**端点能否原生发 PDF**（`Anthropic` / `Gemini` / `GeminiCodeAssist` / `OpenAIResponses` 可，`OpenAI` / `Custom` 不可）。它是**端点维度**，不是模型维度。
- 若把它当成同一合并链里的一层（且排在 `Catalog` 之上），那么「目录明确说该模型不支持 PDF + Anthropic 端点」会被合并成 `Supported` —— 直接把 §6.4 的三态判定改回误报。
- 所以端点可表达性继续留在消费点的合取里（`resolve_native_pdf_support` = 模型事实 ∧ 端点可发，现状即如此），不进 `CapabilityFact`。
- 结果：本批 `Provenance` 的每个变体都有真实写者，符合 §3.2「没有写者就是死字段」的判据。将来若出现真正的适配器级模型事实，再加变体与优先级。

| 约束 | 理由 |
|---|---|
| `CapabilityFact` 不带 `params` | E4 的约束参数需要真实数据源；当前资产没有「几张 / 多大 / 几页」，加了是空结构 |
| 不带 `revision` / `valid_until` | 目录资产无版本号，没有写者（E10 归 §10） |
| `Unknown` 必须能与 `Unsupported` 区分 | 现状分析 §7.2 第 2 条；这是本文的主要收益 |
| 来源是**判定链的一等产物** | 判定函数直接返回 `CapabilityFact`，调用方无法「忘记取来源」 |

### 4.2 主体 key

```rust
pub enum EndpointShape {
    ChatCompletions, Responses, AnthropicMessages,
    GeminiGenerateContent, GeminiCodeAssist, Custom,
}

impl EndpointShape {
    pub fn from_kind(kind: &ProviderKind) -> Self { /* 单一映射点 */ }
}
```

| 规则 | 结论 |
|---|---|
| `endpoint_shape` 由 `ProviderKind` 派生，**不做 base_url 归一** | base_url 归一无标准，且会把自建代理误判为官方端点；F9 已有同构判定可复用 |
| key = `(provider_id, endpoint_shape, model_id)` | provider_id 定位快照行，endpoint_shape 定位该行的请求形态 |
| 目录只贡献 model 级断言（`provenance = Catalog`） | `models-dev.json` 无 provider 维度，不能伪造 provider 级事实 |
| 合并顺序：`ApiDeclared` > `Catalog` > `Snapshot` > `Fallback`，同层以快照命中者优先 | 去掉现状分析 §7.1 里的「用户覆盖」「组织策略」两层：本项目没有这两层来源；快照里存的是**它自己记录的 `source`**，按该 `source` 参与同一排序，而不是整块当作 `Snapshot` |
| 端点可表达性不进合并链 | PDF 端点能力是端点维度，作为消费点合取项保留（见 §4.1 的说明） |

### 4.3 未知策略（按特性定义）

| 特性 | `Unknown` 时的行为 | 依据 |
|---|---|---|
| 图片 | 允许附加，UI 标注「该模型可能忽略图片」 | 误拦会阻止用户；错发代价是服务端报错或忽略 |
| PDF | 按不支持处理（走文本投影） | 原生 PDF 发送失败代价高、token 大 |
| 附件（非 PDF 文件） | 允许 | 文件走文本提取，不依赖模型能力 |
| reasoning 档位 | 只给 `default`；目录档位与端点可表达档位取交集 | 静默丢弃档位属「看起来支持」 |
| 工具调用 | 不参与本文 | 工具可用性由工具白名单决定，归 I38 工具面 |

> 未知策略是**唯一**解释点：各消费点不允许自行解释 `Unknown`（否则三态会退化成第三种布尔）。

## 5 C1 止损

### 5.1 空内容判定识别非文本主体

现状：空判定只认 text 块，`[{"type":"image"}]` 被判为空，与 F1 的前端行为直接冲突。

另一处现状细节（**不改**）：`parse_content_blocks` 对空数组返回 `None`（`Value::Array(arr) if !arr.is_empty()`），所以 `"[]"` 走「非块」路径被判非空、`content_plain_text("[]")` 返回字面量 `"[]"`，`is_content_empty` 里的 `blocks.is_empty()` 分支是死代码。前端 `parseContent` 同样用 `parsed.length > 0`，`"[]"` 也回退成原文。两侧行为一致，且前端 `buildContentBlocks` 不会产出 `[]`（无文本无附件时直接 return），故本次**保持原样**，只删除死分支。

改法：空判定改为「没有任何块」或「所有块都是空白 text」；非 text 块一律视为有内容。

断言：`[{"type":"image"}]` → 非空；`[{"type":"file"}]` → 非空；`[{"type":"text","text":"   "}]` → 空；`[{"type":"text","text":""},{"type":"image"}]` → 非空；`""` → 空。

### 5.2 派生文本给占位符

现状：后端 `content_plain_text` / `content_text_preview` 用 `filter_map` 只留 text 块；前端 `extractPlainText`（`src/chat/core/contentBlockUtils.ts`）同样是 `filter(b => b.type === "text")`。

**两侧必须一起改**，否则止损只做一半：

| 侧 | 位置 | 消费方 |
|---|---|---|
| 后端 | `content_codec::content_plain_text` | 会话标题（`conversations/ops.rs`）、会话列表摘要 |
| 后端 | `content_codec::content_text_preview` | 分支/历史预览（`conversations/branch.rs`） |
| 前端（派生） | `contentBlockUtils` 新增 `extractDerivedText` | 自动标题兜底（`turnOrchestrator::fallbackTitleFromUserMessage`）、历史行预览（`HistoryStubRow`） |
| 前端（编辑） | `contentBlockUtils::extractPlainText`（**保持只取 text**） | 编辑框回填（`MessageBubble::startEdit` / `cancelEdit`） |

改法：非 text 块映射为固定占位符——`image → [图片]`、`file → [文件 <name>]`（`attachment.name` 缺失则 `[文件]`）、其它类型 → `[<type>]`。占位符**硬编码单一语言**：受众是模型与索引，不是用户（现状分析 §7.3）。前端新增 `extractDerivedText` 用同一套占位符规则，与后端 `content_plain_text` 保持一致。

`content_text_preview` 的截断口径（现状是「拼完再按字符边界截断」，改成按块累加后必须写清，否则会突破 `max_len` 上界）：

- 文本块：按**剩余预算**做字符边界截断（与现状同口径），超预算即停并补 `…`。
- 占位符：**整体放入**或整体不放；放不下就停在该块之前并补 `…`，不切半个占位符。
- 上界不变：结果长度仍不超过 `max_len + 1`（`…`），因为调用方是列表/预览的固定宽度展示（`branch.rs` 传 100）。

**`extractPlainText` 不能改成带占位符**：`MessageBubble::startEdit` 用它回填编辑框，`confirmEdit` 又按 `[{type:"text",text:trimmed}, ...imageBlocks, ...fileBlocks]` 重建内容。若回填文本含 `[图片]`，用户一保存就会把占位符落成真实文本块，并和保留的图片块重复。所以「派生文本」与「可编辑文本」必须是两个函数，后者维持现状（只取 text 块）。

断言：`[{text:"hello"},{image}]` 的 `content_plain_text` 同时含 `hello` 与 `[图片]`；`content_text_preview` 截断后仍能见到占位符整体，不出现半个占位符；前端 `extractDerivedText` 对同一输入给出与后端 `content_plain_text` 一致的占位符；纯附件消息的自动标题非空；编辑含图片的消息后保存，文本块不含 `[图片]`、图片块不重复。

### 5.3 模型列表逐项容错

现状：`parse_stored_models` 整数组 `unwrap_or_default()`（F4）。

改法：先按 `Vec<Value>` 解析，再逐项 `from_value`；坏项跳过并 `log::warn!`（带 provider 与索引），好项保留。仅当整个 JSON 无法解析时才返回空，并记 `log::error!`。

**同一形态共两处**，必须一起改，否则用户看到的列表仍会整表变空：

| # | 位置 | 解析目标 |
|---|---|---|
| 1 | `assistant-providers/src/types/config.rs` `parse_stored_models` | `Vec<StoredModelEntry>`（能力判定用） |
| 2 | `src-tauri/src/cmd/provider.rs` `list_providers` | `Vec<ModelConfigDto>`（UI 列表用） |

两处共用一个泛型辅助函数（放在 `assistant-providers`，`src-tauri` 侧直接复用）：`parse_items_lossy<T: DeserializeOwned>(json: &str, ctx: &str) -> Vec<T>`。

断言：`[{good},{bad},{good}]` → 2 项 + 1 条 `warn`；`"not-json"` → 0 项 + 1 条 `error`；`[]` → 0 项且不记 `error`；两处解析点对同一坏 JSON 给出一致的项数。

### 5.4 不透明推理显式不可回放

现状：`model_response_replay` 在 `unified_response_into_chat_deltas` 内被丢弃，无日志、无计数（F12）。

改法：**落点为既有统计器**——`stream/stats.rs` 的 `record_unified_response` 已对每个 `UnifiedResponse` 分类计数，`log_summary` 已在每个 stream 末尾调用一次。所以只加两项：`record_unified_response` 里补一个 `out:replay_capture` 分类（并置 `classified`，避免同时计入 `out:other`）；`log_summary` 里当该计数非零时额外输出一条 `warn!`（含 provider、次数、protocol）。适配器无需改动。

样本来源：capture 的唯一产出方是 `openai/responses_stream.rs`（`completed_replay_capture` → 终态 `UnifiedResponse.model_response_replay`），单测可直接构造该终态响应，不必走真实流。

**不在 `ChatDelta` 上加字段**：40+ 构造点、且没有消费方，加了就是死字段（§3.2 的判据）。E1 需要的是真正的通道（落库 + 回灌），不是再多一个中转字段。

断言：responses 流含 encrypted reasoning → 该 stream 恰好一条 `warn`，且 counter 出现 `out:replay_capture`；`messages` 表无新增列、无新增写入；对外 DTO 不变。

> 这一项只消除「看起来已支持」的假象，不改变任何对外行为。

## 6 C2 能力事实

### 6.1 判定链

```
resolve_capability(kind, provider_id, models_json, model_id, feature)
  候选 = [
    快照项（含该 feature 时）        → { support, provenance = 快照内记录的 source }   // source=snapshot 的存量派生值排在 catalog 之下
    Provider 响应声明（vision 的 api_hint） → ApiDeclared
    目录断言（models-dev）           → Catalog
    Fallback（按 §4.3 的特性策略）    → Fallback（Unknown）
  ]
  → pick_highest(候选)               // 按 provenance 排序，不是「先命中先返回」
```

**必须按 provenance 排序取最高，不能写成「先命中先返回」**：快照里可能存着上一次从目录派生的 `catalog` 值，而这次 Provider 已明确声明 `api`；先命中会把声明压掉。同理，`pdf` 的端点可表达性**不在**候选列表内（见 §4.1），仍在 `resolve_native_pdf_support` 里与模型事实做合取。

`vision.rs` 的两个函数改签名：

- `resolve_support_vision` → 返回 `CapabilityFact`；目录未收录 → `Unknown`（不再 `false`）。
- `input_modalities_for` → 返回 `Option<Vec<String>>`；目录未收录 → `None`（**不再兜底 `["text"]`**，F6 是「不知道」被写成「不支持」的根源）。

**改签名的调用方共三个 discovery 文件，必须一起改**（漏改即编译失败）：

| 文件 | 调用点 | 现传参 |
|---|---|---|
| `openai/discovery.rs` | `extract_model_configs` 两处（`models.data` 分支与 `body.models` 兜底分支） | `api_hint`（`resolved_support_vision()`） |
| `gemini/discovery.rs` | `extract_model_configs` 一处 | `None` |
| `anthropic/discovery.rs` | `extract_model_configs` 一处 | `None` |

`types/mod.rs` 的 `pub use` 列表同步改名（当前 re-export 四个 `resolve_*_from_stored`）。

### 6.2 存储形态

`models_json` 每项新增三个字段（全部可选、全部 `serde(default)`）：

```json
{
  "id": "gpt-4o",
  "supportVision": true,
  "inputModalities": ["text", "image"],
  "vision":     { "support": "supported",   "source": "api" },
  "pdf":        { "support": "unsupported", "source": "catalog" },
  "attachment": { "support": "supported",   "source": "catalog" }
}
```

| 规则 | 结论 |
|---|---|
| 写入时**同时**写 `supportVision` 派生布尔 | 旧前端 / 版本回退仍能工作（P8 的版本回退防御） |
| 读取时优先 `vision`，缺失才回退旧字段 | 存量数据不需要迁移 |
| 存量兼容取值：无 `vision` 时，`inputModalities` 非空则按模态派生，否则按 `supportVision` 派生；`provenance = Snapshot` | 保持旧行为不变；三态只对**重新拉取后**的数据生效 |
| 重新拉取模型列表即完成升级 | 不设迁移脚本、不重写历史 JSON（`db.rs` 的迁移能力也不支持数据重写） |

**写侧来源**（三个 discovery 文件都会构造 `ModelConfig`，`list_models_cmd` / `preview_models_cmd` 经 `adapter.list_models` 分发到对应适配器，所以**三处都要填**，只填 openai 会让 gemini / anthropic 的模型三项全为 `None`）：

| 字段 | 来源 | 未命中 |
|---|---|---|
| `vision` | `resolve_support_vision(id, api_hint)`：API 声明 > 目录 | `Unknown / Fallback` |
| `pdf` | 目录 `modalities.input` 含 `pdf` | `Unknown / Fallback` |
| `attachment` | 目录 `attachment` 布尔 | `Unknown / Fallback` |

写者：`openai/discovery.rs::extract_model_configs`（两处分支）、`gemini/discovery.rs`、`anthropic/discovery.rs`。

注意：`ModelFacts` 当前**没有** `attachment` 字段（只有 `supports_vision` / `context_limit` / `output_limit` / `reasoning` / `tool_call` / `input_modalities` / `reasoning_levels`），需在 `model_catalog.rs` 的 map 构建里补解析，并新增 `catalog_supports_attachment(model_id) -> Option<bool>`。目录资产（`assets/models-dev.json`）中确有该键，不是凭空造的字段。

### 6.3 DTO 与契约

`ModelConfigDto` 同时是 `list_providers` 的输出与 `save_provider` 的输入（F5）：

- 新增 `vision` / `pdf` / `attachment: Option<ModelCapabilityDto>`，写法与既有 `reasoning_levels` 完全同构（`#[serde(default)]`，不加 `skip_serializing_if`），不引入新写法。
- 新增 `ModelCapabilityDto { support, source }`，两字段都是**闭合枚举**（`CapabilitySupportDto` / `CapabilitySourceDto`），前端得到联合类型而非字符串。
- 保留 `supportVision: bool` 作为派生只读投影（写回不参与判定）。
- 输入侧沿用 `deny_unknown_fields`；输出 `Option` 恒在、值为 `null`。

> 契约纪律：改 DTO 后必须 `npm run contract:gen` 并提交 `src/types/protocol.contract.ts`；禁止手改生成类型。

**硬约束：写侧与 DTO 必须同一提交**。`save_provider` 把 `ProviderInputDto.models`（即 `Vec<ModelConfigDto>`）原样序列化进 `models_json`，`list_providers` 又按 `ModelConfigDto` 反解，而该结构带 `deny_unknown_fields`。所以「存储解析结构新增字段」与「DTO 新增字段」之间不能有中间态：写侧先落 `vision/pdf/attachment` 而 DTO 未同步 → 每个模型都带未知字段 → UI 列表逐项被跳过（C1.3 只能把「整表空」降级成「逐项消失 + warn」，不能消除）。反方向同理：DTO 有字段而存储解析结构没有，`models_json` 里该字段会被 `StoredModelEntry` 忽略（无 `deny_unknown_fields`，不报错但也不参与判定）。

### 6.4 消费点

| 消费点 | 现在 | 改后 |
|---|---|---|
| 后端图片投影 | `resolve_support_vision_from_stored` → bool | 三态；`Unsupported` 走文本投影，`Unknown` 保留图片并记降级 |
| 后端 PDF | `resolve_native_pdf_support` → bool | 三态；`Unknown` 按不支持 |
| 前端图片入口 | `model.supportVision` | `vision.support`；`unknown` 仍显示入口并标注 |
| 前端附件准入 | `admitAttachment(ctx.supportVision)` | 三态入参；`unknown` 允许 |
| 前端文件选择器扩展名 | `supportVision` 决定是否含图片扩展 | 同上 |

## 7 C3 词汇型前提

### 7.1 去枚举白名单

现状：目录档位经 `KNOWN_EFFORTS`（6 值）过滤（F10）；通用端点可表达档位只有 `low/medium/high`。后果：目录声明 `minimal` 时 UI 展示该档位，下发时被静默丢弃。

改法：

1. `model_catalog` 保留目录声明的**任意**非空档位，不再过滤（`none` / `off` 仍归入开关语义）。
2. 可表达集合由各适配器的归一化函数**导出**，不在目录侧硬编码第二份：在 `client/quirks.rs` 内新增 `expressible_efforts(kind, model_id) -> Option<Vec<&'static str>>`（`None` 表示该端点不做归一、任意非空档位原样下发）与 `unsupported_efforts(kind, model_id, declared) -> Vec<String>`，实现时把 `normalize_generic_reasoning_effort` / `normalize_deepseek_reasoning_effort` / `normalize_glm_reasoning_effort` 的接受分支改为引用同一组常量集合（+ 别名归一），保证「可表达集合」与「归一化行为」同源。

   分派必须与各适配器**实际调用的归一化函数**一一对应（这是本节唯一的硬约束，写错就会把「不支持」误报成「支持」）：

   | 端点 | 实际归一化 | 可表达集合 |
   |---|---|---|
   | `Anthropic` / `Gemini` / `GeminiCodeAssist` | `normalize_generic_reasoning_effort` | `low` / `medium` / `high` |
   | `OpenAI` / `Custom` + 模型名命中 `is_deepseek_reasoning_effort_model` | `normalize_deepseek_reasoning_effort` | flash：`low` / `high` / `max`；非 flash：`high` / `max` |
   | `OpenAI` / `Custom` + 模型名命中 `is_glm_reasoning_effort_model` | `normalize_glm_reasoning_effort` | `high` / `max` |
   | `OpenAI` / `Custom` + 其他模型 | `normalize_generic_reasoning_effort` | `low` / `medium` / `high` |
   | `OpenAIResponses` | **不归一**，`effort` 原样写入 `reasoning.effort` | `None`（不限制） |

   注意 deepseek 的 `low` 仅对 `deepseek-v4-flash` 可表达（`is_flash` 分支），集合需按模型区分，不能写成单一常量；quirk 分派依据是**模型名**而非 URL（`openai/chat.rs` 现行逻辑），所以 `expressible_efforts` 不需要 `base_url` 参数。
3. **两个集合的语义必须分清**，否则会把「能发出去但被改写」的档位误报成不支持：

   - `expressible` = 归一化后**实际会下发**的档位（别名已折叠），仅作说明用途。
   - `unsupported` = 目录声明但归一化返回 `None` 的档位，也就是**真正会被静默丢弃**的那些。
   - 归一化返回 `Some(别的值)` 的档位（如 deepseek 的 `medium` → `high`、glm 的 `low` → `high`）既不在 `expressible` 里，也**不算** `unsupported`：它会正常发出，只是被改写成另一个档位。

   因此前端选项 = 目录档位 **减去** `unsupported`（黑名单语义，而不是和 `expressible` 求交集）；`default` / `off` / `on` 是内置语义位，原样保留。差集不进入选项，但在 `ReasoningSelector` 的既有 Tooltip 末尾追加一行「该端点不支持：minimal / xhigh」（不做静默消失）。

**载体**：可表达集合随 `list_providers` 下发，放在 `ProviderListDto` 的**只读**字段里，不进 `ModelConfigDto`。原因：`ModelConfigDto` 同时是 `save_provider` 的输入并被原样写回 `models_json`（F5），把派生值放进去会落成过期快照。`ProviderListDto` 是纯输出，天然不可回写。

展示落点（与数据来源一一对应，不要混淆）：
- 档位差集说明 → `ReasoningSelector`（它已从 `useProviderStore.providers`（即 `ProviderListDto[]`）取到 provider 与 model，`effortSupport` 就在那里）。
- `attachment` / `vision` 三态 → `ConnectDialog` 的模型行（该处 `models` 来自 `listModels` / `previewModels`，返回的正是 `ModelConfigDto[]`，三态字段随 DTO 一起到）。

```rust
pub struct ProviderListDto {
    // ...既有字段
    /// 每个模型的档位可表达性（只读投影，不参与存储）
    pub effort_support: Vec<ModelEffortSupportDto>,
}

pub struct ModelEffortSupportDto {
    pub model_id: String,
    pub expressible: Vec<String>,   // 归一化后实际会下发的档位（别名已折叠，说明用途）
    pub unsupported: Vec<String>,   // 目录声明且归一化返回 None 的档位（真正会被丢弃的差集）
}
```

断言：目录声明 `["minimal","low","medium","high","xhigh","max"]` + 通用端点（`OpenAI` / `Anthropic` / `Gemini`）→ `expressible = ["low","medium","high"]`、`unsupported = ["minimal","xhigh","max"]`，前端选项为 `default/low/medium/high`，且差集说明含 `minimal`；同一目录 + `OpenAIResponses` → `unsupported = []`（端点不归一）；deepseek 模型声明含 `medium` → `medium` 既不在 `expressible` 也不在 `unsupported`（选项保留、下发时改写为 `high`）；不再出现「被过滤且无任何说明」的档位。

### 7.2 附件限额单一来源

现状：三处常量互相不校验（F11），且**类型不一致**——`assistant-agent/src/multimodal.rs` 的字节限额是 `usize`，`src-tauri/src/cmd/attachment.rs` 的是 `u64`，不能直接互相引用。

改法：`assistant-agent/src/multimodal.rs` 定义唯一的限额结构（`multimodal.rs` 已有参数化的 `MultimodalConfig`，单一来源应是结构而非裸常量）：

```rust
pub struct AttachmentLimits {
    pub max_images: usize,
    pub max_files: usize,
    pub max_image_bytes: u64,
    pub max_file_bytes: u64,
}
pub const ATTACHMENT_LIMITS: AttachmentLimits = /* 4 / 4 / 8MiB / 16MiB */;
```

- `MultimodalConfig::trusted_channel_input()` 从该结构取（字节字段 `as usize`，这是唯一的类型转换点）。
- `cmd/attachment.rs` 删除本地 `MAX_IMAGE_BYTES` / `MAX_FILE_BYTES`，改读 `ATTACHMENT_LIMITS`（`u64` 直接可用，无需转换）。
- 新增 `AttachmentLimitsDto { max_images: u32, max_files: u32, max_image_bytes: u64, max_file_bytes: u64 }`（与既有 DTO 的「计数用 `u32`、字节用 `u64`」口径一致），由该结构构造（计数字段 `as u32`），经**专用命令** `get_attachment_limits()` 下发，与 `get_asset_dirs()` 同一形态（都是「小配置 DTO + 单命令」，前端首次调用时缓存）。
- 前端常量删除，`admit.ts` / `notify.ts` / `factory.ts` 只读 DTO 值。
- 准入判定是同步函数（`attachmentStore.addAttachment` 返回 `AdmitError | null`），所以前端**只能同步读**限额：`limits.ts` 与 `paths.ts` 同形态（模块级 `Promise` 缓存 + 同步读接口），在 `ConversationView` 启动 `useEffect` 里与 `loadProviders()` 并列预取。**不做数值兜底**（未加载时抛错），否则前端又会出现一份独立数值，A14 的「随 DTO 变化」就不再成立。

断言：全仓检索限额数值字面量只剩 `ATTACHMENT_LIMITS` 定义处与 DTO 构造处；前端不再有独立数值；把 DTO 的 `maxImages` 改成 1 后，第 2 张图片被拒（证明前端随 DTO 变化而非编译期常量）。

### 7.3 未消费字段登记

| 字段 | 处置 | 消费前置 |
|---|---|---|
| `attachment` | 本批**只落事实**（与 `vision` / `pdf` 同一写入形态，成本为零）；**不参与准入判定**（文件一律走后端文本提取）；展示随 C3.4 的模型设置行同批呈现 | 已具备（目录资产携带该键） |
| `limit.context` / `limit.output` | 已消费 | 已具备 |
| `cost.input` / `cost.output` | 不消费 | 需要成本展示需求 + 币种 / 汇率口径 |
| `modalities.output` | 不消费 | 需要产物类输出（E9） |
| `family` / `release_date` / `open_weights` | 不消费 | 需要模型详情页；只读展示，不参与判定 |
| `structured_output` / `limit.input` | **资产不携带** | 需先扩目录资产，扩资产后按 C2 同一形态接入 |

## 8 分批与退出条件

| 批次 | 内容 | 前置 | 退出条件 |
|---|---|---|---|
| **C1 止损** | §5 四项；无 schema、无 DTO 变更 | 无 | §9 A1–A5 |
| **C2 能力事实** | §6；改 `models_json` 与 `ModelConfigDto` | C1（不同批里既改语义又改契约） | §9 A6–A11 |
| **C3 词汇型前提** | §7；前端选项与限额收口 | C2（前端先有三分支） | §9 A12–A15 |

C1 可独立合入并单独发布；C2 与 C3 必须同版本上线（否则前端拿到三态却仍按布尔判定）。

## 9 验收

| # | 断言 | 层级 |
|---|---|---|
| A1 | 纯图片 / 纯文件 content 不再被判空；`chat.empty_content` 不再误报 | 单测（`content_codec`）+ 手工 |
| A2 | 派生文本对非文本块产出占位符，且不出现半个占位符；前后端派生结果一致 | 单测（`content_codec` + `contentBlockUtils`） |
| A3 | 混合好坏项的 `models_json` 保留好项并记 `warn`；整表坏记 `error` | 单测（`types::config` + `cmd/provider`） |
| A4 | 含 encrypted reasoning 的流产生恰好一条可观测日志；落库路径无新增写入 | 单测（`stream/stats` 计数器） |
| A5 | 四项均不改变对外 DTO 与数据库 schema | `contract:check` 幂等 |
| A6 | `vision` 可由 API / 目录两条来源产生，`pdf` 可由端点 / 目录两条来源产生，`attachment` 由目录产生（未收录 → `Unknown / Fallback`），`provenance` 正确 | 单测（`capability`） |
| A7 | 目录未收录模型 → `Unknown`，不再返回 `["text"]` / `false` | 单测（`vision`） |
| A8 | `Unknown` 与 `Unsupported` 在投影层走不同分支（保留图片 + 降级记录 vs 文本投影） | 单测（`kernel/request`） |
| A9 | 无新字段的存量 `models_json` 可正常解析：模型在列表中时派生结论与改动前一致；不在列表中者由 `false` 变 `Unknown`（即 A7） | 单测（兼容样本） |
| A10 | 新增字段经 `contract:gen` 生成，前端 round-trip 后不丢字段 | `contract:check` + `protocolContract.test.ts` |
| A11 | 同一 `model_id` 在两个 provider 上可得出不同能力结论（不再后写者胜） | 单测（双 provider 用例） |
| A12 | 目录档位不再被白名单过滤；选项 = 目录档位 − 端点会丢弃的档位（别名档位如 deepseek `medium` 保留） | 单测（`model_catalog` + 前端 `reasoning`） |
| A13 | 档位差集以可读说明呈现，不静默消失 | 前端单测 + 手工 |
| A14 | 附件限额只有一处数值来源，前端随 DTO 变化 | 单测 + 全仓检索 |
| A15 | `npm run lint:fix` / `typecheck` / `vitest` / `cargo test` / `cargo clippy -D warnings` / `cargo fmt --check` / `contract:check` 全绿 | 门禁 |

## 10 对现状分析 §10 六个待决策的结论

| # | 问题 | 结论 | 理由 |
|---|---|---|---|
| 1 | 是否接受 `(provider, endpoint_shape, model_id)`？`endpoint_shape` 如何定义？ | 接受；`endpoint_shape` = `ProviderKind` 派生的闭合枚举，不做 base_url 归一 | F9 已有同构判定；base_url 归一没有标准答案 |
| 2 | 不透明内容保护语义三值是否够用？fork 复制还是重放？ | 本文不落，只做「显式不可回放」 | 需先有落库与回灌，属 E1 独立规格 |
| 3 | 能力事实是实时投影 + revision，还是快照 + 可刷新？ | 快照 + 可刷新；刷新点 = 重新拉取模型列表 | 目录资产无版本号，revision 没有写者 |
| 4 | 历史重建是否应成为唯一入口？ | 本文不落 | 属 E1 / P2 断点，需独立规格 |
| 5 | 若 E2 长期不落地，流协议 item 维度是否仍值得占位？ | **不占位** | 对外 DTO 当前无任何工具字段，占位没有承载点；且「开放数据 + 闭合消费」仍是语法型，加字段不等于占位 |
| 6 | 降级结构化回传的最小形态？ | 只做未消费字段登记 + 未知策略表 | 现在做会先造一个没有读者的通道 |

## 11 未落项与触发条件

| 项 | 现状 | 触发条件 | 归属 |
|---|---|---|---|
| E1 不透明推理落库 / 回放 | 捕获即丢（C1-4 后变可观测） | 出现必须回传的 Provider 行为，或用户报告多轮推理退化 | 独立规格 |
| E2 流协议 item 维度 | 对外无工具字段 | 出现并发工具 / 托管工具 | 独立规格（同时改 DTO 与前端消费） |
| E3 远端会话状态 | 无 | 采用 `previous_response_id` 或服务端记忆 | 独立规格（需重定义 fork / 重试） |
| E4 约束型能力 | 无 `params`；限额硬编码（C3 收口但不参数化） | 需要按模型限制张数 / 页数 | C2 结构上加 `params` |
| E5 能力协商 | 无 | 需要错误驱动的能力降级 | 独立规格（先做错误粒度） |
| E6 多模型组合 | 无 | 需要 fallback / router | 独立规格 |
| E7 策略层覆盖 | 无 | 企业策略下发 | 独立规格 |
| E8 运行时探测 | 无 | 自托管模型目录失效 | 独立规格 |
| E9 输出产物 | 无 | 图像 / 语音输出 | 独立规格（依赖 C1-1 的判空修正） |
| E10 时间漂移 | 快照冻结 | 目录失真造成实际错误 | C2 结构上加 `revision` |
| P2 历史重建有损 | `reasoning` 有列不回灌 | 同 E1 | 独立规格 |
| P9 预算求解 | 单一规则 | 需要多约束降级 | 独立规格（先有 E4 参数） |

## 12 风险

| 风险 | 处置 |
|---|---|
| DTO 契约变更影响前端多处消费点 | C2 与 C3 同版本；A10 用契约测试守护；`supportVision` 保留为派生投影 |
| 三态让 UI 多出「不确定」分支，调试复杂度上升 | 未知策略收敛到 §4.3 一张表；禁止各消费点自行解释 `Unknown` |
| 存量数据 `Snapshot` 来源不可判定 | 不迁移、不重写；重新拉取即升级；不承诺存量准确性 |
| 逐项容错掩盖真实损坏 | 坏项必须记 `warn` 且带 provider 与索引；整表坏记 `error` |
| 去白名单后目录出现端点无法表达的档位 | 差集显式说明（A13），不静默丢弃 |
| 前后端限额口径漂移 | 单一来源 + A14 的「改常量前端随之变化」用例 |
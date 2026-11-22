# OpenAI Responses 端点处置

| 项 | 值 |
|---|---|
| 状态 | 已决策（A：补正式入口）｜前端已实施，运行验收待真实端点 |
| 日期 | 2026-11-17 |
| 来源 | I9-provider-multi-api-spec.md（已归档）Phase 7 交付项的运行可达性复核 |
| 关联 | code-structure-followups.md（已归档）（F1-b）、[crate-architecture.md](../../store/crate-architecture.md) |
| 判据 | [CODE-SPLIT-PRINCIPLE.md](../../principle/CODE-SPLIT-PRINCIPLE.md)（结构整理不改语义） |

---

## 1 问题陈述

`ProviderKind::OpenAIResponses` 有一整套生产实现（SSE 解析、重试、工具调用累积、replay capture），但前端设置页**没有任何正式入口**能产出 `type = "openairesponses"` 的 Provider 记录。当前状态是「编译在内、官方 UI 不可达、仅靠自定义 Provider ID 自由文本绕过」。

它既不是误加的残留，也不是可以随手删的死代码：它是 I9 规格 Phase 7 里被明确勾选完成的交付项，同时被多处能力判定当作事实引用。因此需要一次显式处置决策，而不是默认放着。

---

## 2 事实清单（可复核）

### 2.1 后端实现完整

| 事实 | 证据 |
|---|---|
| 枚举变体存在（6 变体：`OpenAI` / `OpenAIResponses` / `Anthropic` / `Gemini` / `GeminiCodeAssist` / `Custom`） | `src-tauri/crates/assistant-providers/src/base.rs` `pub enum ProviderKind` |
| 字符串映射存在 | `base.rs` `provider_kind_from_str`：`"openairesponses" => ProviderKind::OpenAIResponses` |
| 工厂分派到专用流 | `src-tauri/crates/assistant-providers/src/openai/mod.rs` `chat_completion_stream`：`ProviderKind::OpenAIResponses => responses_stream::responses_stream(...)`，其余落 `chat::chat_completion_stream` |
| 流实现 688 非空行 | `src-tauri/crates/assistant-providers/src/openai/responses_stream.rs`（单函数内 `'retry: loop` + `async_stream::stream!`） |
| 事件与 replay 类型 | `src-tauri/crates/assistant-providers/src/openai/responses_types.rs`；测试被 `#[cfg(all(test, feature = "internal-tests"))]` 门控，默认不跑 |
| 请求端点拼接 | `responses_stream.rs`：`format!("{}/responses", base_url)` |

### 2.2 被当作事实引用（删变体即连带改动）

| 引用点 | 语义 |
|---|---|
| `base.rs` `use_native_api()` | 返回 `true` |
| `base.rs` `cache_capability()` | 返回 `CacheCapability::ReadWrite`；生产调用点在 `src-tauri/src/cmd/token_usage_record.rs` 与 `src-tauri/crates/assistant-agent/src/kernel/turn/persist.rs` |
| `src-tauri/crates/assistant-providers/src/capability.rs` `EndpointShape::from_kind` | `=> EndpointShape::Responses` |
| `src-tauri/crates/assistant-agent/src/kernel/request.rs` `endpoint_can_send_pdf` | 归入「可原生发 PDF」 |
| `src-tauri/crates/assistant-providers/src/client/quirks.rs` `expressible_efforts` | 返回 `None`（effort 不归一，原样写入 `reasoning.effort`） |
| `responses_stream.rs` 工具调用 | `arguments_is_snapshot = true`（参数是完整快照而非增量） |
| `src-tauri/crates/assistant-providers/tests/base.rs` | 有 `ProviderKind::OpenAIResponses` 断言 |

### 2.3 前端无入口（决策前事实，已由 §9 处置）

| 事实 | 证据 |
|---|---|
| `ApiFormat` 曾是封闭三值联合 | `src/settings/provider/types.ts`：`"openai" \| "anthropic" \| "local"` |
| format 注册表曾只有 3 项 | `src/settings/core/provider/providerDefinitions.ts`：`defineProviderFormat` 三次（`openai` / `anthropic` / `local`） |
| preset 注册表曾为 10 项，无 responses | 同上：`openai` / `deepseek` / `groq` / `openrouter` / `mistral` / `zhipu` / `qwen` / `anthropic` / `ollama` / `lmstudio` |
| 未知 type 静默兜底为 openai | `src/settings/core/provider/providerLookup.ts` `getApiFormatByType`：查不到 preset 时 `log.warn` 并返回 `"openai"` |

结论（决策前）：**官方 UI 无法产出 `openairesponses`**。同批缺口的还有 `gemini` / `geminicodeassist`——后端 5 个具名 kind，前端只能产出 2 个（`openai`、`anthropic`）。

### 2.4 决策前的绕过路径（已随 A 关闭）

| 环节 | 行为 |
|---|---|
| 自定义 Provider 的「Provider ID」是自由文本 | `src/settings/provider/ConnectDialog/fieldSchemas.ts`：`key: "type"`, `type: "string"`，占位符「小写英文，如 my-gateway」 |
| 提交时原样取该文本 | `src/settings/provider/ConnectDialog/useConnectForm.ts`：`type: isPreset ? preset.slug : values.type \|\| "custom"` |
| 后端无白名单校验 | `src-tauri/src/cmd/provider.rs` `save_provider` 直接入库 |
| 数据库无约束 | `src-tauri/crates/assistant-storage/src/db.rs`：`type TEXT NOT NULL`，无 `CHECK` |

即：手输 `openairesponses` + 填 baseUrl，能真的打到 `{baseUrl}/responses`。副作用有三：

1. **标签与分组错误**：`getApiFormatByType` 兜底为 `openai`，`FormatTag` 显示「OpenAI 兼容」，`ProviderManager` 分到 openai 分组。
2. **编辑态无法修正 type**：`src/settings/provider/ConnectDialog/index.tsx` 中 `showType = !isEdit && !isPreset`，编辑时 type 字段隐藏；写错只能删除重建。
3. **列模型仍可用**：`src-tauri/crates/assistant-providers/src/openai/discovery.rs` 走 `{base_url}/models`，与 Responses 端点共用，不受影响。

A 落地后：自定义 Provider 的 type 默认取所选 format 的 `customType`（`custom` / `openairesponses` / `anthropic` / `gemini` / `local`），上表副作用 1、2 消失（标签与分组由反查修正，编辑态不再需要改 type）；副作用 3（列模型）保持不变。

### 2.5 规格侧的成因

- I9-provider-multi-api-spec.md（已归档）§4.6.3 原文定调「**无需修改前端**，`provider_kind_from_str` 已支持映射」。
- 同规格 Phase 7 勾选项只有 Rust 侧文件（`responses_types.rs` / `responses_stream.rs` / 枚举 / 映射），**无任何前端入口项**。
- 同规格 H2「功能验证（运行时）」8 项**全部未勾选**，其中第 6 项正是「`ProviderKind::OpenAIResponses` 工厂分发到 responses_stream」，第 8 项是「ChatDelta 接口不变，前端无需修改」。
- 成因链：编译通过 + 171 单测通过（H1 ✅）→ 运行验证从未执行 → 入口缺失未被发现 → Phase 7 直接标 ✅。

---

## 3 处置选项

| 选项 | 动作 | 语义变更 | 工作量 | 主要风险 |
|---|---|---|---|---|
| **A. 补正式入口** | 前端加 `responses` format + preset，闭环 Phase 7 | 功能新增（不改既有语义） | 中（前后端 + i18n + 真实 API 验收） | 需真实端点验收，否则重复「勾选但未验证」 |
| **B. 保留现状并记录事实** | 不改代码，把「无正式入口 / 仅自定义手输可达」写入本文与 F1-b 条目 | 无 | 极小 | 继续背一段无人验证的代码；`responses_stream` 无 fixture |
| **C. 移除** | 删枚举变体 + `responses_stream.rs` + `responses_types.rs` + 全部能力声明分支 | **功能删除** | 中（含存量数据迁移） | 已有 `type = "openairesponses"` 的行会落到 `Custom` 分支，需迁移或明确降级 |

**三选一的判据**：按 [CODE-SPLIT-PRINCIPLE.md](../../principle/CODE-SPLIT-PRINCIPLE.md)「结构整理只改结构、不改语义」，A 与 C 都不是代码拆分任务，不能混入 F1-b 的提交里。

---

## 4 推荐与决策

**决策：A（补正式入口）**。理由是这条链路的后端实现完整且被 7 处能力判定引用，缺的只是前端注册表入口；补入口的改动集中在 4 个注册点、后端零改动，且能顺带把 §2.4 的「只能手输、写错改不回」这条绕过路径关掉。

决策前的备选判断记录如下（B 曾是默认项）：

1. B 的理由是「没有真实 API 验收前，把 A 做完仍只是另一份未验证的代码」——该风险没有消失，已转为 §9.2 的验收待办，而不是阻止入口落地。
2. C 是功能删除，且牵动 7 处能力声明与存量数据，收益（少 688 行 + 若干分支）不足以在没有产品决策的情况下承担迁移风险。
3. B 的交付物是本文 §2 的事实清单，以及 code-structure-followups.md（已归档） F1-b 条目里补一句可达性说明；这部分内容已保留为历史依据。

---

## 5 A 实施清单（已执行）

### 5.1 前端

| 改动 | 文件 |
|---|---|
| `ApiFormat` 联合类型加 `"responses"` / `"gemini"` | `src/settings/provider/types.ts` |
| format 注册：`responses`（`customType: "openairesponses"`）、`gemini`（`customType: "gemini"`） | `src/settings/core/provider/providerDefinitions.ts` |
| preset 注册：`openairesponses` / `gemini` / `geminicodeassist` | 同上。**slug 必须逐字等于后端 kind 字符串**——`useConnectForm` 直接把 `preset.slug` 写库，`provider_kind_from_str` 才认 |
| 「format → 落库 type」单一来源 | `ProviderFormatDef.customType`；`providerLookup.getFormatCustomType()` |
| 自定义 Provider 的 type 不再退化为 `custom` | `useConnectForm` 新增 `customType` 入参，提交与回填都用它；`fieldSchemas` 用其作占位符与提示 |
| type 反查 format 支持自定义 type | `getApiFormatByType` 先查 preset slug，再查 `customType`，最后才兜底 `openai` |
| i18n | `zh-CN.json` / `en-US.json`：`format.responses(.description)`、`format.gemini(.description)`、`preset.openairesponses` / `gemini` / `geminicodeassist`、`field.type.default_hint` |
| 格式配色 | `src/assets/styles/global.css`：`--color-format-responses` / `--color-format-gemini`（亮暗两套） |

`FormatTag.tsx` / `AddProviderDialog.tsx` / `ProviderManager.tsx` 由注册表驱动，无需改动。

### 5.2 后端

无需改代码：`provider_kind_from_str` 已有 `"openairesponses"` / `"gemini"` / `"geminicodeassist"` 映射，`EndpointShape` / `cache_capability` / `endpoint_can_send_pdf` / `expressible_efforts` 均已就位。

**未做**：`save_provider` 仍不校验 type 白名单。自定义 Provider 的 type 仍可手输，但已不再是产出 responses 的必要手段；引入白名单会禁止未注册的自定义 type，需与「自定义 Provider ID」的产品意图对齐后再定。

### 5.3 验收（对应 I9 H2 未勾选的 4 项）

| # | 验收项 | 方式 |
|---|---|---|
| 1 | 文本 + reasoning + reasoning_summary + tool_call 四类 delta 正常 | 真实端点流式对话 |
| 2 | `response.output_item.done` 的 arguments 尾部补齐正确 | 触发多轮工具调用，检查参数完整性 |
| 3 | `response.failed` / `response.incomplete` 正确分类为错误 | 构造超限或异常请求 |
| 4 | `model_response_replay` 正确构建（含 opaque reasoning） | 检查第二轮请求体是否回传 replay item |

验收通过后再回填 I9 规格 H2（该文档已归档，冻结不改，改为在本文记录结论）。

---

## 6 若改选 C：移除清单

按依赖顺序：

1. `base.rs`：删 `ProviderKind::OpenAIResponses` 变体；同步 `use_native_api()` / `cache_capability()` / `provider_kind_from_str` 映射。
2. `src-tauri/crates/assistant-providers/src/capability.rs`：删 `EndpointShape::Responses` 及 `from_kind` 分支、对应单测。
3. `src-tauri/crates/assistant-agent/src/kernel/request.rs`：`endpoint_can_send_pdf` 删该变体。
4. `src-tauri/crates/assistant-providers/src/client/quirks.rs`：`expressible_efforts` 删该分支及测试。
5. `src-tauri/crates/assistant-providers/src/openai/mod.rs`：删分派分支；删 `responses_stream.rs` / `responses_types.rs`；`mod.rs` 的模块声明与 `pub use` 同步。
6. `src-tauri/crates/assistant-providers/tests/base.rs`：删相关断言。
7. **存量数据**：`type = "openairesponses"` 的行会因映射落到 `ProviderKind::Custom { base_url }`，行为从「打 `/responses`」变成「打 `/chat/completions`」，属静默行为变更。需在迁移中二选一：改写为 `custom` 并提示用户重配，或直接拒绝加载并给出明确错误。

---

## 7 对 F1-b 的门

code-structure-followups.md（已归档） 的 F1-b（拆分 `responses_stream.rs`）价值完全取决于本节决策，**决策已定为 A**：

| 决策 | 对 F1-b 的影响 |
|---|---|
| A（已采纳） | **先 A 后拆**。入口已补上（§5.1），但 §5.3 的运行验收未完成，拆分前提「先补 fixture」的收益仍受限——fixture 只能喂纯函数，无法证明端点可用。故 F1-b 维持「暂缓」，等运行验收有结论再评估 |
| B | 可拆，但优先级维持在「低」；且拆分前提「先补 fixture」的收益同样有限 |
| C | F1-b 作废，改为删除任务（§6） |

在运行验收完成前不动 `responses_stream.rs`，避免出现「刚拆完就删」；且该文件 688 非空行在 900 行优先审查线以下，行数本身不构成拆分义务。

---

## 8 待决策项（已答复）

| # | 问题 | 结论 |
|---|---|---|
| 1 | 是否把 Responses 端点做成面向用户的可选格式 | **是**，选 A，已实施（§5.1） |
| 2 | 是否同时补齐 `gemini` / `geminicodeassist` | **是**，三者缺口同源，同批补齐，避免重复改注册表与 i18n |
| 3 | `save_provider` 是否对 `type` 引入白名单校验 | **暂不引入**。手输拼错的兜底改由 `customType` 默认值承担，白名单涉及「自定义 Provider ID」产品意图，另行决策 |

---

## 9 实施记录

### 9.1 已落地（2026-11-17）

- 前端 5 个 format（`openai` / `responses` / `anthropic` / `gemini` / `local`）+ 13 个 preset，官方 UI 可直接产出 `openairesponses` / `gemini` / `geminicodeassist`。
- `customType` 成为「format → 落库 type」的唯一来源，堵掉 §2.4 的隐式退化：此前「自定义 OpenAI 兼容」会写成 `custom`，现在按 format 取值。
- 顺带修正标签错位：`local` 的自定义 Provider 落库为 `local` 后，`getApiFormatByType` 经 `customType` 反查回到 `local`，不再显示为「OpenAI 兼容」。
- `cargo` / 后端零改动。

### 9.2 未完成（阻塞项）

§5.3 的 4 项运行时验收全部未执行——本机没有可用的 Responses 端点。**在完成前，本文不得标为「已验证」**；`responses_stream.rs` 的单测仍被 `#[cfg(all(test, feature = "internal-tests"))]` 门控，默认不跑。
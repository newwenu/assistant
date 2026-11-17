# OpenAI Responses 端点处置

| 项 | 值 |
|---|---|
| 状态 | 待决策（活跃） |
| 日期 | 2026-11-17 |
| 来源 | I9-provider-multi-api-spec.md（已归档）Phase 7 交付项的运行可达性复核 |
| 关联 | [code-structure-followups.md](./code-structure-followups.md)（F1-b）、[crate-architecture.md](./crate-architecture.md) |
| 判据 | [CODE-SPLIT-PRINCIPLE.md](../principle/CODE-SPLIT-PRINCIPLE.md)（结构整理不改语义） |

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

### 2.3 前端无入口

| 事实 | 证据 |
|---|---|
| `ApiFormat` 是封闭三值联合 | `src/settings/provider/types.ts`：`"openai" \| "anthropic" \| "local"` |
| format 注册表只有 3 项 | `src/settings/core/provider/providerDefinitions.ts`：`defineProviderFormat` 三次（`openai` / `anthropic` / `local`） |
| preset 注册表 10 项，无 responses | 同上：`openai` / `deepseek` / `groq` / `openrouter` / `mistral` / `zhipu` / `qwen` / `anthropic` / `ollama` / `lmstudio` |
| 未知 type 静默兜底为 openai | `src/settings/core/provider/providerLookup.ts` `getApiFormatByType`：查不到 preset 时 `log.warn` 并返回 `"openai"` |

结论：**官方 UI 无法产出 `openairesponses`**。同批缺口的还有 `gemini` / `geminicodeassist`——后端 5 个具名 kind，前端只能产出 2 个（`openai`、`anthropic`）。

### 2.4 但存在一条未文档化的绕过路径

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

**三选一的判据**：按 [CODE-SPLIT-PRINCIPLE.md](../principle/CODE-SPLIT-PRINCIPLE.md)「结构整理只改结构、不改语义」，A 与 C 都不是代码拆分任务，不能混入 F1-b 的提交里。

---

## 4 推荐

**本轮默认 B**，A 与 C 作为需要独立决策的功能项。

理由：

1. 当前没有运行证据表明这条链路可用；在没有真实 API 验收前，把 A 做完仍只是「另一份未验证的代码」。
2. C 是功能删除，且牵动 7 处能力声明与存量数据，收益（少 688 行 + 若干分支）不足以在没有产品决策的情况下承担迁移风险。
3. B 的成本几乎为零，且能把当前缺失的那条事实补齐——F1-b 的价值判断正缺这一行。

B 的交付物就是本文 §2 的事实清单，以及 [code-structure-followups.md](./code-structure-followups.md) F1-b 条目里补一句可达性说明。

---

## 5 若改选 A：实施清单

### 5.1 前端

| 改动 | 文件 |
|---|---|
| `ApiFormat` 联合类型加 `"responses"` | `src/settings/provider/types.ts` |
| 注册 format | `src/settings/core/provider/providerDefinitions.ts`：`defineProviderFormat({ id: "responses", ... })` |
| 注册 preset（可选，但建议） | 同上：`defineProviderPreset({ slug: "openairesponses", format: "responses", ... })`。**slug 必须等于 `openairesponses`**——`useConnectForm` 直接把 `preset.slug` 写库 |
| i18n | `src/i18n/locales/zh-CN.json` / `en-US.json`：补 `settings.provider.format.responses(.description)` 与 `settings.provider.preset.openairesponses` |

`providerLookup.ts` / `providerFormatRegistry.ts` / `FormatTag.tsx` / `AddProviderDialog.tsx` / `ProviderManager.tsx` 均由注册表驱动，无需改动。

### 5.2 后端

无需改代码：`provider_kind_from_str` 已有 `"openairesponses"` 映射，`EndpointShape` / `cache_capability` / `endpoint_can_send_pdf` / `expressible_efforts` 均已就位。

需确认一项策略：`save_provider` 是否引入 type 白名单校验。引入可堵住手输拼错（如 `openai-responses`），但同时会禁止未注册的自定义 type，需与「自定义 Provider ID」的产品意图对齐。

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

[code-structure-followups.md](./code-structure-followups.md) 的 F1-b（拆分 `responses_stream.rs`）价值完全取决于本节决策：

| 决策 | 对 F1-b 的影响 |
|---|---|
| A | **先 A 后拆**。入口补上后再拆，拆出来的结构才有真实调用方与验收支撑 |
| B | 可拆，但优先级维持在「低」；且拆分前提「先补 fixture」的收益同样有限——fixture 只能喂纯函数，无法证明端点可用 |
| C | **F1-b 作废**，改为删除任务（§6） |

在决策落地前不动 `responses_stream.rs`，避免出现「刚拆完就删」。

---

## 8 待决策项

1. 是否要把 Responses 端点做成**面向用户的可选格式**？（决定 A / B / C）
2. 若要，是否同时补齐 `gemini` / `geminicodeassist` 两个同样缺入口的 kind，还是只补 Responses？（三者缺口同源，分批处理会重复改前端注册表与 i18n）
3. `save_provider` 是否对 `type` 引入白名单校验？（影响手输拼错的兜底方式，也影响 A 的入口唯一性）
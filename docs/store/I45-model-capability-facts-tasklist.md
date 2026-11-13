# I45 — 模型能力事实 任务清单

> 规格：[I45-model-capability-facts-spec.md](./I45-model-capability-facts-spec.md)
> 前置：无。**C1 可独立合入**；C2 与 C3 必须同版本上线（否则前端拿到三态却仍按布尔判定）。

三个批次对应规格 §8：C1 止损（无 schema、无 DTO 变更）→ C2 能力事实（改 `models_json` 与 `ModelConfigDto`）→ C3 词汇型前提（前端选项与限额收口）。

> 本清单不管理提交。提交粒度按代码依赖与可编译性现场决定；这里只回答「代码改没改、验收跑没跑」。

---

## 阶段 C1 — 止损（无 schema、无 DTO 变更）

### C1.1 空内容判定识别非文本主体

对应规格 §5.1 / 验收 A1。

- [x] `assistant-storage/src/content_codec.rs`：`is_content_empty` 改为三分支——解析不出块时按原字符串判定；有块时「存在任一非 text 块 → 非空」；否则「所有 text 块文本 trim 后为空 → 空」
- [x] `parse_content_blocks` 保持「空数组 → `None`」现状（`"[]"` 两侧一致回退为原文，前端 `buildContentBlocks` 不会产出 `[]`）；顺带删除 `is_content_empty` 内已不可达的 `blocks.is_empty()` 分支
- [x] 更新既有用例 `test_content_blocks_image_only`：由断言「空」翻转为断言「非空」（这是行为修正，不是放宽测试）
- [x] 新增用例：纯 `file` 块非空；`[{text:""},{image}]` 非空；`[{text:"   "}]` 空；`""` 空
- [x] 确认 `src-tauri/src/cmd/agent/mod.rs::validate_and_preprocess` 无需改动（只经 `is_content_empty`）
- [ ] 手工：只发图片、不写文字 → 不再返回 `chat.empty_content`（**未执行**：需桌面端实发；判定层已由 `is_content_empty` 单测覆盖）

**依赖**：无

---

### C1.2 派生文本给占位符

对应规格 §5.2 / 验收 A2。

- [x] `content_codec.rs`：新增 `fn block_placeholder(v: &Value) -> Option<String>`，规则为 `image → [图片]`、`file → [文件 <name>]`（`attachment.name` 缺失则 `[文件]`）、其它类型 → `[<type>]`
- [x] `content_plain_text` 改为「text 块取文本、非 text 块取占位符」，仍以 `\n` 连接
- [x] `content_text_preview` 改为按块累加：保持现状的空格连接（`" "`，与 `content_plain_text` 的 `"\n"` 不同）；**文本块按剩余预算做字符边界截断**（与现状同口径，超预算即停并补 `…`），**占位符整体放入或整体不放**（放不下就停在该块之前并补 `…`）；结果长度仍不超过 `max_len + 1`（调用方 `branch.rs` 传 100，是固定宽度预览）
- [x] `extract_text_from_block` 改完后会失去全部调用点（`is_content_empty` 改用 `is_text_block` + 读 `text` 字段；派生文本改用 `block_placeholder`）→ **直接删除该函数**，否则 `cargo clippy -D warnings` 会因 `dead_code` 失败
- [x] **前端派生函数** `src/chat/core/contentBlockUtils.ts` 新增 `extractDerivedText`（同一套占位符规则），`extractPlainText` **保持只取 text 块不变**
- [x] 两个派生消费方改调 `extractDerivedText`：`turnOrchestrator.ts::fallbackTitleFromUserMessage`（自动标题）、`HistoryStubRow.tsx`（历史行预览）
- [x] **不要**把 `MessageBubble` 的 `startEdit` / `cancelEdit`（两处都调 `extractPlainText` 回填编辑框）换成 `extractDerivedText`：`confirmEdit` 会用 `[{text: 编辑框内容}, ...editImageBlocks, ...editFileBlocks]` 重建内容，占位符会落成真实文本块并与附件块重复（已确认两处调用保持 `extractPlainText`）
- [x] 更新既有用例：`test_content_blocks_image_only` 的 `content_plain_text` 由 `""` 改为 `[图片]`；`test_content_blocks_mixed` 改为 `"look at this\n[图片]"`
- [x] 新增用例：`[{text:"hello"},{file attachment:{name:"a.pdf"}}]` 的 plain_text 含 `[文件 a.pdf]`；未知类型 `{type:"artifact"}` → `[artifact]`；preview 截断在占位符中间时不出现半个占位符
- [x] 新增前端用例（`contentBlockUtils.test.ts`）：同一输入的 `extractDerivedText` 结果与后端 `content_plain_text` 一致（含 `[图片]`）；纯附件消息的 `fallbackTitleFromUserMessage` 非空（落在新增的 `src/services/stream/__tests__/turnOrchestrator.test.ts`，为此把该函数改为 `export`）；`extractPlainText` 对含图片内容仍返回空字符串

**依赖**：C1.1（同一函数的判定与派生必须一起改，否则空判定与非空内容的派生文本不一致）

---

### C1.3 模型列表逐项容错

对应规格 §5.3 / 验收 A3。

- [x] `assistant-providers/src/types/config.rs`：新增 `pub fn parse_items_lossy<T: DeserializeOwned>(json: &str, ctx: &str) -> Vec<T>`——先按 `Vec<Value>` 解析，再逐项 `serde_json::from_value`；坏项 `log::warn!` 带 `ctx` 与索引；整体解析失败 `log::error!` 返回空
- [x] `parse_stored_models` 改为调用 `parse_items_lossy`（`ctx` 形如 `models_json:<provider_id>`）
- [x] `types/mod.rs` 导出 `parse_items_lossy`
- [x] `src-tauri/src/cmd/provider.rs::list_providers` 的 `serde_json::from_str::<Vec<ModelConfigDto>>(...).unwrap_or_default()` 改为调用同一函数（第二处同构解析点，见规格 §5.3 的表）
- [x] 新增 `types/config.rs` 单测：`[{good},{bad},{good}]` → 2 项；`"not-json"` → 0 项；`[]` → 0 项
- [x] 新增 `cmd/provider.rs` 单测（或复用同一 JSON 样本）：两处解析点对同一坏 JSON 给出相同项数（两处共用样本 `MIXED_MODELS_JSON`，各断言 2 项）

**依赖**：无

---

### C1.4 不透明推理显式不可回放

对应规格 §5.4 / 验收 A4。

- [x] `assistant-providers/src/stream/stats.rs`：`StreamStats` 新增 `last_replay_protocol: Option<String>`
- [x] `record_unified_response` 新增分类：`response.model_response_replay` 为 `Some` 时 `increment("out:replay_capture")`、记录 `protocol`、并置 `classified = true`（避免同时计入 `out:other`）
- [x] `log_summary`：当 `out:replay_capture` 计数非零时额外输出一条 `warn!`（含 provider、次数、protocol），位置在既有 `debug!` 汇总之后
- [x] 新增 `pub fn counter(&self, label: &str) -> usize` 只读访问器，供测试断言（不新增公开字段）
- [x] 新增单测：含 capture 的响应 → `counter("out:replay_capture") == 1` 且 `out:other` 未增加；无 capture 的响应 → 计数为 0
- [x] 断言 `ChatDelta` 结构未变、`messages` 表未新增列、对外 DTO 未变（`contract:check` 幂等）

**依赖**：无

> 适配器无需改动：五个流式适配器都已实例化 `StreamStats` 并调用 `record_unified_response`，`log_summary` 每 stream 调用一次，故「每 stream 至多一条 warn」自动成立。

### C1 执行记录（2026-11-13）

- 改动文件：`assistant-storage/src/content_codec.rs`、`assistant-providers/src/types/config.rs`、`assistant-providers/src/types/mod.rs`、`assistant-providers/src/stream/stats.rs`、`src-tauri/src/cmd/provider.rs`、`src/chat/core/contentBlockUtils.ts`、`src/chat/core/__tests__/contentBlockUtils.test.ts`、`src/services/stream/turnOrchestrator.ts`、`src/history/detail/HistoryStubRow.tsx`、新增 `src/services/stream/__tests__/turnOrchestrator.test.ts`
- 计划外改动（为覆盖「自动标题非空」用例）：`turnOrchestrator.ts::fallbackTitleFromUserMessage` 加 `export`，无行为变化
- 门禁结果：`npm run lint:fix` / `npm run typecheck` / `npm run test`（45 文件 536 通过、1 跳过）/ `npm run contract:check` 幂等 / `cargo test --workspace` 全绿 / `cargo clippy --workspace --all-targets -- -D warnings` 0 告警 / `cargo fmt --all --check` 通过
- 为让 clippy 门禁转绿，顺带修掉三处既有 lint 阻塞（非本次功能改动）：`assistant-vault/src/security_db/rekey.rs` 的复杂元组类型提为 `EntryCipherColumns`、`assistant-vault/tests/legacy_residue.rs` 去掉被忽略的 `?Sized`、`tests/assembly_contract.rs` 两处 `iter().any()` 改 `contains()`
- A5 成立：`ChatDelta` / `messages` 表 / 对外 DTO 均未变，`contract:check` 无 diff
- 未完成：C1.1 最后一条桌面端手工验证（需实机发送纯附件消息）

---

## 阶段 C2 — 能力事实（三态 + 来源 + 主体 key）

### C2.1 新建 `capability` 模块

对应规格 §4.1 / §4.2 / 验收 A6。

- [x] 新建 `assistant-providers/src/capability.rs`，`lib.rs` 声明模块
- [x] 定义 `Support { Supported, Unsupported, Unknown }`，serde 小写（`supported` / `unsupported` / `unknown`）
- [x] 定义 `Provenance { ApiDeclared, Catalog, Snapshot, Fallback }`，serde 小写（`api` / `catalog` / `snapshot` / `fallback`）。**不要加 `AdapterKnown`**：本批唯一的适配器已知事实是「端点能否原生发 PDF」，属端点维度；放进同一合并链会把「目录说不支持 PDF + Anthropic 端点」误升为 `Supported`。端点可发继续留在 `resolve_native_pdf_support` 的合取里
- [x] 定义 `CapabilityFact { support, provenance }`，`provenance` 在 wire 上键名为 `source`（`#[serde(rename = "source")]`，与规格 §6.2 JSON 及 `ModelCapabilityDto` 一致），提供 `fn is_supported(&self) -> bool`（只对 `Supported` 为真）与 `fn is_unknown(&self)`
- [x] 定义 `CapabilityFeature { Vision, Pdf, Attachment }`
- [x] 定义 `EndpointShape` 六个变体 + `fn from_kind(kind: &ProviderKind) -> Self`（**唯一映射点**；`Custom { .. }` → `Custom`）
- [x] 实现 `fn pick_highest(facts: &[CapabilityFact]) -> CapabilityFact`，优先级 `ApiDeclared > Catalog > Snapshot > Fallback`，同层以快照命中者优先。**按 provenance 排序，不是「先命中先返回」**：否则快照里上次派生的 `catalog` 值会压掉本次 Provider 的 `api` 声明
- [x] 单测：优先级合并；快照 `catalog` 值 + 本次 `api` 声明 → 结果为 `ApiDeclared`；`Unknown` 与 `Unsupported` 在 `is_supported` 上同假、在 `is_unknown` 上不同
- [x] 单测：`EndpointShape::from_kind` 对六个 `ProviderKind` 逐一映射

**依赖**：无

---

### C2.2 `vision` 三态化

对应规格 §6.1 / 验收 A7。

- [x] `assistant-providers/src/vision.rs`：`resolve_support_vision(model_id, api_hint) -> CapabilityFact`——`api_hint` 有值时 `provenance = ApiDeclared`；目录命中 `Catalog`；两者都无 → `Unknown`（**不再 `false`**）
- [x] `input_modalities_for(model_id) -> Option<Vec<String>>`——目录未收录返回 `None`（**不再兜底 `["text"]`**）
- [x] 更新**三个** discovery 文件的调用点（漏改即编译失败）：
  - `openai/discovery.rs` 两处（`models.data` 分支 + `body.models` 兜底分支，`api_hint` 由 `resolved_support_vision()` 提供）
  - `gemini/discovery.rs` 一处（`None`）
  - `anthropic/discovery.rs` 一处（`None`）
- [x] 三处统一：`input_modalities` 由 `Option` 展开为 `Vec`（`None` → 空数组，与存量 `#[serde(default)]` 同义），`support_vision` 由 `CapabilityFact` 派生布尔
- [x] `types/mod.rs` 的 `pub use` 列表同步改名（当前 re-export 四个 `resolve_*_from_stored`）
- [x] 重写 `vision.rs` 单测：`unknown-model` + 无 hint → `Unknown`（原断言 `false` 需改）；`modalities_and_vision_agree_when_no_hint` 改为「两者同为 `Unknown` 时一致」
- [x] 新增用例：`input_modalities_for("unknown-model")` 为 `None`

**依赖**：C2.1

---

### C2.3 存储侧三态解析

对应规格 §6.2 / 验收 A9、A11。

- [x] `assistant-providers/src/types/config.rs`：`StoredModelEntry` 新增 `vision` / `pdf` / `attachment: Option<CapabilityFact>`（**直接复用 C2.1 的类型，不另造 `StoredCapability`**；`#[serde(default)]`；不加 `deny_unknown_fields`，保持存量宽容）
- [x] 子字段不合法（如 `support: "weird"`）会让该 entry 反序列化失败 → 由 C1.3 的 `parse_items_lossy` 跳过并 `warn`，属可观测行为；不要为此把三态改回 `String`
- [x] `resolve_support_vision_from_stored` → `resolve_vision_from_stored(model_id, models_json) -> CapabilityFact`
- [x] `resolve_support_pdf_from_stored` → `resolve_pdf_from_stored(model_id, models_json) -> CapabilityFact`
- [x] 新增 `resolve_attachment_from_stored(model_id, models_json) -> CapabilityFact`
- [x] 存量兼容：无 `vision` 字段时——`input_modalities` 非空则按含 `image` 派生，否则按 `supportVision` 派生，`provenance = Snapshot`；模型不在列表 → `Unknown / Fallback`
- [x] `resolve_input_modalities_from_stored` 返回 `Option<Vec<String>>`（模型不在列表 → `None`），与 C2.2 的 `input_modalities_for` 语义对齐
- [x] `types/mod.rs` 的 `pub use` 列表同步改名为新的四个 `resolve_*_from_stored`（改名不同步会留下未解析导入）
- [x] 更新既有 6 条 `resolve_support_vision_from_stored` 单测为三态断言（含 `"not-json"` → `Unknown`）
- [x] 新增用例：同一 `model_id` 在两个 provider 的 `models_json` 上得出不同结论（验证不再「后写者胜」，验收 A11）

**依赖**：C2.1

---

### C2.4 写入侧落三态

对应规格 §6.2 / 验收 A6。

- [x] `assistant-providers/src/base.rs`：`ModelConfig` 新增 `vision` / `pdf` / `attachment: Option<CapabilityFact>`（`#[serde(default)]`，与既有 `support_vision` / `input_modalities` 并存）
- [x] `model_catalog.rs`：`ModelFacts` **当前没有 `attachment` 字段**，需新增并在 map 构建里解析 `model_val.get("attachment").as_bool()`；新增 `catalog_supports_attachment(model_id) -> Option<bool>`
- [x] `model_catalog.rs`：新增 `catalog_supports_pdf(model_id) -> Option<bool>`（复用 `catalog_input_modalities` 判断含 `pdf`，`None` 语义与 `catalog_supports_vision` 一致）
- [x] **三个** discovery 文件都填充三项（`list_models_cmd` / `preview_models_cmd` 经 `adapter.list_models` 分发，只填 openai 会让 gemini / anthropic 的模型三项全为 `None`）：
  - `openai/discovery.rs::extract_model_configs`（两处分支，`api_hint` 来自 `resolved_support_vision()`）
  - `gemini/discovery.rs`（`api_hint = None`）
  - `anthropic/discovery.rs`（`api_hint = None`）
- [x] 三处填充规则一致：`vision = resolve_support_vision(id, hint)`；`pdf = catalog_supports_pdf(id)`；`attachment = catalog_supports_attachment(id)`；目录未收录一律 `Unknown`
- [x] `src-tauri/src/cmd/provider.rs::model_config_dto` 透传三项到 `ModelConfigDto`；`CapabilityFact → ModelCapabilityDto` 的转换写在 `cmd/provider.rs` 内（`assistant-providers` 不依赖 `protocol/dto`，不要在那里加 `From` 实现）
- [x] 保留 `support_vision` / `input_modalities` 的写入（旧前端与版本回退仍读它们）
- [x] 单测：目录收录且无 API 声明 → `vision = Catalog`；API 声明为 true/false → `ApiDeclared`；目录未收录 → 三项均 `Unknown / Fallback`

**依赖**：C2.2、C2.3

> **与 C2.5 同一提交**：`save_provider` 把 `ProviderInputDto.models`（`Vec<ModelConfigDto>`）原样写进 `models_json`，`list_providers` 又按 `ModelConfigDto` 反解，而该结构带 `deny_unknown_fields`。写侧先落字段而 DTO 未同步 → 每个模型都带未知字段 → UI 列表逐项被跳过（C1.3 只把「整表空」降级成「逐项消失 + warn」）。

---

### C2.5 DTO 与契约生成

对应规格 §6.3 / 验收 A10。

- [x] `src-tauri/src/protocol/dto/settings.rs`：新增 `CapabilitySupportDto`（`supported` / `unsupported` / `unknown`）、`CapabilitySourceDto`（`api` / `catalog` / `snapshot` / `fallback`），两个枚举都用 `#[serde(rename_all = "lowercase")]`（与既有 `AgentModeKindDto` 同风格，且与 `models_json` 里的字符串一致）；`ModelCapabilityDto { support, source }` 用 `#[serde(rename_all = "camelCase", deny_unknown_fields)]`
- [x] `ModelConfigDto` 新增 `vision` / `pdf` / `attachment: Option<ModelCapabilityDto>`，写法与既有 `reasoning_levels` 同构（`#[serde(default)]`，**不加 `skip_serializing_if`**，键恒在、值 `null`）
- [x] 保持 `ModelConfigDto` 的 `deny_unknown_fields`：新增字段必须在此结构内，否则前端 round-trip 会失败
- [x] `src-tauri/tests/protocol_bindings.rs` 补 `ModelCapabilityDto` 的 `decl`
- [x] 运行 `npm run contract:gen`，提交生成的 `src/types/protocol.contract.ts`（禁止手改）
- [x] 更新 `src/chat/core/__tests__/protocolContract.test.ts` 的 `ModelConfigDto` fixture（新增三键）
- [x] 新增契约用例：含三态的 model 经前端读取再写回后字段不丢（验收 A10）

**依赖**：C2.3、C2.4

---

### C2.6 后端消费点三分支

对应规格 §4.3 / §6.4 / 验收 A8。

- [x] `assistant-agent/src/kernel/request.rs`：图片分支改为 `match fact.support`——`Supported` 走 `rehydrate_images`；`Unsupported` 走 `project_images_for_text_model`；`Unknown` 保留图片并记一条降级日志（含 model 与 provenance）
- [x] `resolve_native_pdf_support` 改为 `match`：`Supported` 且端点可发 → 原生 PDF；`Unknown` 与 `Unsupported` 一律走文本投影（`Unknown` 记日志）
- [x] 附件（非 PDF）不新增判定：走既有文本提取路径
- [x] 单测：`Unknown` 与 `Unsupported` 在投影层走不同分支（验收 A8）；`Unknown` 的日志恰好一条
- [x] `grep` 确认没有其它 `resolve_support_vision_from_stored` / `resolve_support_pdf_from_stored` 调用点残留

**依赖**：C2.3

---

### C2.7 前端消费点三分支

对应规格 §6.4 / 验收 A8。

- [x] `src/chat/ConversationView.tsx`：`supportVision` 的派生改为读 `model.vision.support`；`vision` 为 `null`（存量 `models_json`）时按后端 `types::config::resolve_entry_modalities` 的同一口径回退——`inputModalities` 非空 → 含 `image` 则 `"supported"` 否则 `"unsupported"`；为空 → `supportVision ? "supported" : "unsupported"`（该 Rust 函数是私有的，前端只是复刻同一规则，不要试图调用它）
- [x] `ConversationView.tsx`：模型未找到（`model` 为 `undefined`）时保持 `"unsupported"`（与改动前 `?? false` 一致，避免在未选模型时冒出图片入口）
- [x] `src/stores/attachmentStore.ts`：`supportVision: boolean` → `visionSupport: "supported" | "unsupported" | "unknown"`，初始值 `"unsupported"`；setter 改名 `setVisionSupport` 并更新全部调用点（`ConversationView.tsx`、`intake.ts`）
- [x] `src/chat/attachment/admit.ts`：`ctx.supportVision` → `ctx.visionSupport`；仅 `unsupported` 返回 `no_vision`，`supported` 与 `unknown` 放行
- [x] `src/chat/attachment/sources/filePicker.ts`：形参 `supportVision: boolean` → `visionSupport: 三态`；`unsupported` 时只给文件扩展名，`supported` / `unknown` 都给图片 + 文件扩展名
- [x] `src/chat/attachment/intake.ts`：`getState().supportVision` → `getState().visionSupport` 并原样传给 `pickAttachmentPaths`（该文件只做转发，不改判定）
- [x] 图片入口在 `unknown` 时保留并加可读标注（新增 i18n key，中英各一条）
- [x] 前端单测：`unknown` 时图片准入通过、`unsupported` 时返回 `no_vision`；`filePicker` 在 `unknown` 时含图片扩展名
- [x] `src/chat/attachment/__tests__/` 既有用例按新签名更新

**依赖**：C2.5

### C2 执行记录（2026-11-13）

- 改动文件（Rust）：新增 `assistant-providers/src/capability.rs`；`assistant-providers/src/vision.rs`、`model_catalog.rs`、`base.rs`、`types/config.rs`、`types/mod.rs`、`lib.rs`、`openai/discovery.rs`、`gemini/discovery.rs`、`anthropic/discovery.rs`、`assistant-agent/src/kernel/request.rs`、`src-tauri/src/protocol/dto/settings.rs`、`src-tauri/src/cmd/provider.rs`、`src-tauri/tests/protocol_bindings.rs`
- 改动文件（前端）：`src/types/protocol.contract.ts`（生成）、`src/stores/attachmentStore.ts`、`src/chat/ConversationView.tsx`、`src/chat/attachment/{types,admit,intake,notify}.ts`、`src/chat/attachment/sources/filePicker.ts`、`src/chat/core/__tests__/protocolContract.test.ts`、新增 `src/chat/attachment/__tests__/admit.test.ts`、`src/i18n/locales/{zh-CN,en-US}.json`
- 计划外改动：`Provenance::ApiDeclared` 加 `#[serde(rename = "api")]`——默认小写化会产出 `apideclared`，与 `models_json` 和 `ModelCapabilityDto` 的 `api` 不一致
- 门禁结果：`npm run lint`（0 告警）/ `npm run typecheck` / `npx vitest run`（46 文件 545 通过、1 跳过）/ `cargo clippy --workspace --all-targets -- -D warnings` 0 告警 / `cargo fmt --all` / `cargo test -p assistant-app --tests` 全绿
- 契约生成注意：`contract:gen` 内部的 ts-rs `export_bindings` 用例并行写同一份 `vault.contract.ts`，并发时会互相截断（本次首跑即丢掉了 `TotpInputDto` / `TotpCodeDto` / `ParsedOtpAuthDto` / `UnreadableReasonDto` / `UnreadableEntriesDto` 五个类型，导致 `vault.types.ts` 的 re-export 悬空）。改用 `cargo test -p assistant-vault --lib export_bindings -- --test-threads=1` 串行重生成后，`vault.contract.ts` 与 HEAD 完全一致、`protocol.contract.ts` 仅剩 C2 的 16 行新增
- A6/A7 成立：`capability.rs` 的 `pick_highest` 按 provenance 排序（`api_declaration_beats_snapshot_catalog_value`、`provenance_order_is_respected_regardless_of_position`），`Unknown` 与 `Unsupported` 仅在 `is_unknown` 上分叉
- A8 成立：`unknown_and_unsupported_take_different_branches`、`unknown_produces_exactly_one_log_line`、`pdf_unknown_is_projected_to_text_even_on_capable_endpoint`（端点可表达性仍留在消费点合取，未进合并链）
- A9/A11 成立：`vision_from_stored_three_states`、`stored_fact_wins_over_legacy_derivation`、`same_model_id_in_different_providers_differs`
- A10 成立：`ModelConfigDto` 三键 `#[serde(default)]` 且无 `skip_serializing_if`，契约用例 `Model capability facts survive a JSON round-trip` 覆盖读→写回不丢字段
- 未做：C2 无迁移脚本（存量 `models_json` 由 `resolve_*_from_stored` 按旧字段派生、`provenance = Snapshot`），符合「存量兼容不迁移」约束

---

## 阶段 C3 — 词汇型前提

### C3.1 目录档位去白名单

对应规格 §7.1 / 验收 A12。

- [x] `assistant-providers/src/model_catalog.rs`：删除 `KNOWN_EFFORTS` 白名单，改为「任意非空字符串均保留、去重、保序」
- [x] `none` / `off` 仍归入 `toggle`，不进入 `efforts`
- [x] 更新 `model_catalog.rs` 既有档位单测；新增用例：目录声明 `minimal` / `xhigh` / `max` 均被保留
- [x] 断言 `ReasoningLevelsDto.efforts` 不再出现「被过滤且无说明」的情况

**依赖**：无

---

### C3.2 适配器可表达集合单一来源

对应规格 §7.1 第 2 条。

- [x] `assistant-providers/src/client/quirks.rs`：新增 `pub const GENERIC_EFFORTS: [&str; 3] = ["low", "medium", "high"]`、`GLM_EFFORTS`、`DEEPSEEK_EFFORTS`，以及 flash 专用的 `DEEPSEEK_FLASH_EFFORTS`（`low` 仅在 `is_flash`（`deepseek-v4-flash`）时并入，其余为 `high` / `max`，不能写成单一常量）
- [x] 归一化函数改为「先查可表达集合（命中即原样返回），再查别名表，都不命中 → `None`」：`GENERIC_ALIASES` 为空表、`DEEPSEEK_ALIASES = [("medium","high"),("xhigh","max")]` + 非 flash 的 `("low","high")`、`GLM_ALIASES = [("low","high"),("medium","high"),("xhigh","max")]`。这样「可表达集合」与「归一化行为」同源，且**行为与现状完全等价**（含 `minimal` / `none` 仍返回 `None`）
- [x] 新增 `pub fn expressible_efforts(kind: &ProviderKind, model_id: &str) -> Option<Vec<&'static str>>`（`None` = 该端点不归一、任意非空档位原样下发）。分派必须与适配器实际调用的归一化函数一一对应：
  - `Anthropic` / `Gemini` / `GeminiCodeAssist` → generic 集合
  - `OpenAI` / `Custom` + `is_deepseek_reasoning_effort_model` → deepseek 集合（flash 含 `low`）
  - `OpenAI` / `Custom` + `is_glm_reasoning_effort_model` → glm 集合
  - `OpenAI` / `Custom` + 其他 → generic 集合
  - `OpenAIResponses` → `None`（`responses_stream.rs` 直接把 `effort` 原样写入 `reasoning.effort`，不做归一）
- [x] 新增 `pub fn unsupported_efforts(kind: &ProviderKind, model_id: &str, declared: &[String]) -> Vec<String>`：**逐项跑该端点的归一化函数**，返回 `None` 的才计入（保序）；`expressible_efforts` 为 `None` 时返回空。**不能用「declared − expressible」**——别名项（deepseek 的 `medium`、glm 的 `low`）会正常下发，只是被改写成别的档位，算进 `unsupported` 会让 UI 误报「不支持」
- [x] 单测（等价性护栏，仅覆盖三个归一化函数；`OpenAIResponses` 无归一化，不参与此断言）：对 `["", "low", "medium", "high", "xhigh", "max", "minimal", "none", "weird"]` 逐端点断言 `normalize(x)` 与改动前逐一相同，且结果恒属于 `expressible_efforts()` ∪ `{None}`
- [x] 单测：通用端点 + 目录六档 → `expressible = low/medium/high`、`unsupported = minimal/xhigh/max`；glm 模型 → `max` 可表达；deepseek flash → `low` 可表达；deepseek pro → `low` **归一到 `high`**（不在 `expressible`、也不在 `unsupported`）；glm 的 `low` / `medium` 同理不记 `unsupported`

**依赖**：C3.1

---

### C3.3 `ProviderListDto` 只读投影

对应规格 §7.1「载体」/ 验收 A13。

- [x] `src-tauri/src/protocol/dto/settings.rs`：新增 `ModelEffortSupportDto { model_id, expressible, unsupported }`
- [x] `ProviderListDto` 新增 `effort_support: Vec<ModelEffortSupportDto>`（**只读输出**，不进 `ModelConfigDto`，避免被写回 `models_json` 成过期快照）
- [x] `src-tauri/src/cmd/provider.rs::list_providers` 按 provider 的 `type_` + 每个模型 id 填充该字段：`expressible = expressible_efforts(...)`（`None` 时取目录声明档位）、`unsupported = unsupported_efforts(...)`（`expressible_efforts` 为 `None` 时为空，与「端点不限制」语义一致）
- [x] `src-tauri/tests/protocol_bindings.rs` 补 `decl`
- [x] 运行 `npm run contract:gen` 并提交/随本批提交生成物（生成物已在工作区）
- [x] 断言 `ProviderInputDto` 未新增字段（保存路径不受影响）

**依赖**：C3.2、C2.5（同一批 `contract:gen` 合并提交生成物）

---

### C3.4 前端档位选项与差集说明

对应规格 §7.1 第 3 条 / 验收 A13。

- [x] `src/chat/core/reasoning.ts`：`reasoningChoices(levels, unsupported?)` 增加第二参数（**黑名单语义**：从目录档位里剔除 `unsupported`，不要用 `expressible` 求交集，否则别名档位会被误删）；**剔除只作用于 effort 项**，`default` / `off` / `on` 是内置语义位、原样保留；`unsupported` 为 `undefined` 或空数组时保持现行为
- [x] `src/chat/selectors/ReasoningSelector.tsx`：从 `useProviderStore.providers`（即 `ProviderListDto[]`）按 `selectedProviderId` 取 `effortSupport`，再按 `selectedModelId` 取 `unsupported` 传入 `reasoningChoices`
- [x] `ReasoningSelector.tsx`：`unsupported` 非空时，在既有 `tooltip` 文案末尾追加一行「该端点不支持：minimal / xhigh」（新增 i18n key，中英各一条；档位名复用 `EFFORT_LABEL_KEYS` 的既有 label）
- [x] 注意既有 `useEffect` 会把「不在 `choices` 里的当前档位」重置为 `inherit`：`providers` 尚未加载时 `unsupported` 为 `undefined`（不过滤），加载后 `choices` 可能收缩并触发一次重置。这是既有机制，不要为此改成异步或加特判；`unsupported` 必须来自已加载的 `providers`，不要为规避重置去缓存旧值
- [x] `src/settings/provider/ConnectDialog.tsx` 的 `connect-model-item` 增加 `attachment` 三态小标签（`supported` / `unsupported` / `unknown` → 「未知」），这是 `attachment` 事实本批的唯一消费点（准入判定不使用它；该处 `models` 来自 `listModels` / `previewModels`，返回 `ModelConfigDto[]`，字段随 DTO 一起到）
- [x] 前端单测：六档 + 通用端点（`unsupported = minimal/xhigh/max`）→ 选项为 `default/low/medium/high`；带 `toggle` 时 `off` 仍在；`unsupported` 缺省时行为与改动前一致
- [x] 前端单测：deepseek pro 声明含 `medium`、`unsupported` 为空 → `medium` **仍在选项里**（别名档位不被剔除）
- [x] 前端单测：`unsupported` 非空时说明文案含 `minimal`

**依赖**：C3.3

---

### C3.5 附件限额收口（后端）

对应规格 §7.2 / 验收 A14。

- [x] `assistant-agent/src/multimodal.rs`：新增 `pub struct AttachmentLimits { pub max_images: usize, pub max_files: usize, pub max_image_bytes: u64, pub max_file_bytes: u64 }` 与 `pub const ATTACHMENT_LIMITS`（**唯一数值来源**）。字段必须 `pub`：`src-tauri/src/cmd/attachment.rs` 是另一个 crate，私有字段读不到
- [x] `MultimodalConfig::trusted_channel_input()` 改为从 `ATTACHMENT_LIMITS` 取（字节字段 `as usize`，这是唯一的类型转换点）；`MAX_*` 四处私有常量删除
- [x] `src-tauri/src/cmd/attachment.rs`：删除本地 `MAX_IMAGE_BYTES` / `MAX_FILE_BYTES`（`u64`），改读 `ATTACHMENT_LIMITS`（`u64` 直接可用，无需转换）
- [x] `src-tauri/src/protocol/dto/storage.rs`：新增 `AttachmentLimitsDto { max_images: u32, max_files: u32, max_image_bytes: u64, max_file_bytes: u64 }`（计数 `u32` / 字节 `u64`，与同文件既有 DTO 口径一致）
- [x] `src-tauri/src/cmd/attachment.rs`：新增 `get_attachment_limits() -> AttachmentLimitsDto`，由 `ATTACHMENT_LIMITS` 构造（计数字段 `as u32`，字节字段直接取）
- [x] `src-tauri/src/ipc.rs` 注册该命令（与 `cmd::get_asset_dirs` 同一处）
- [x] `src-tauri/tests/protocol_bindings.rs` 补 `decl`
- [x] `npm run contract:gen` 并提交/随本批提交生成物（生成物已在工作区）
- [x] 断言全仓限额数值字面量只剩 `ATTACHMENT_LIMITS` 定义处与 DTO 构造处

**依赖**：无（可与 C1 同批）

---

### C3.6 附件限额收口（前端）

对应规格 §7.2 / 验收 A14。

- [x] 新增 `src/chat/attachment/limits.ts`：与 `paths.ts` 同形态（模块级 `Promise` 缓存 + `storageService.getAttachmentLimits()`），导出**同步读**接口 `attachmentLimits(): AttachmentLimitsDto`（未加载时抛错，不做数值兜底——前端不得再持有任何限额字面量）
- [x] 预取点：`src/chat/ConversationView.tsx` 既有的启动 `useEffect` 内，与 `loadProviders()` 并列 `void prefetchAttachmentLimits()`；这样在用户能点击附件入口前已完成加载（`addAttachment` 保持同步，不改签名）
- [x] `src/services/storageService.ts`：新增 `getAttachmentLimits: () => invoke<AttachmentLimitsDto>("get_attachment_limits")`（与 `getAssetDirs` 同处）
- [x] `src/chat/attachment/types.ts`：删除 `ATTACHMENT_MAX_IMAGES` / `ATTACHMENT_MAX_FILES` / `ATTACHMENT_MAX_IMAGE_SIZE_BYTES` / `ATTACHMENT_MAX_FILE_SIZE_BYTES`
- [x] `src/chat/attachment/admit.ts`、`notify.ts`、`factory.ts` 改为读 limits（`intake.ts` 只改 `supportVision` 传参，不涉及限额）
- [x] 更新 `__tests__/factory.test.ts` 等用例：先 `mock` `getAttachmentLimits` 并预取，再用 limits 值构造超限样本
- [x] 新增用例：把 DTO 的 `maxImages` 改成 1 后，第 2 张图片被拒（验收 A14 的「随 DTO 变化」）
- [x] 全仓检索确认前端不再有独立数值

**依赖**：C3.5

---

### C3 执行记录（2026-11-13）

- 改动文件（Rust）：`assistant-providers/src/model_catalog.rs`、`assistant-providers/src/client/quirks.rs`、`src-tauri/src/protocol/dto/{settings,storage}.rs`、`src-tauri/src/cmd/{provider,attachment}.rs`、`src-tauri/src/ipc.rs`、`assistant-agent/src/multimodal.rs`、`src-tauri/tests/{protocol_bindings,protocol_dto_roundtrip}.rs`
- 改动文件（前端）：`src/types/protocol.contract.ts`（生成）、新增 `src/chat/attachment/limits.ts`、`src/services/storageService.ts`、`src/chat/ConversationView.tsx`、`src/chat/attachment/{types,admit,notify,factory}.ts`、`src/chat/core/reasoning.ts`、`src/chat/selectors/ReasoningSelector.tsx`、`src/settings/provider/ConnectDialog.tsx`、`src/settings/styles/settings.css`、`src/i18n/locales/{zh-CN,en-US}.json`、`src/chat/core/__tests__/{reasoning,protocolContract}.test.ts`、`src/chat/attachment/__tests__/{admit,factory,structure}.test.ts`、`src/services/__tests__/turnOrchestrator.test.ts`
- 计划外改动：`reasoning.ts` 新增 `unsupportedEffortSummary`（把差集说明的档位清单抽成纯函数，便于对「说明文案含 `minimal`」直接断言，`ReasoningSelector` 改为调用它）；`quirks.rs` 单测里 `fn(&str, &str) -> Option<&'static str>` 提为 `type Normalizer`（否则 `clippy::type_complexity` 在 `-D warnings` 下失败）
- 门禁结果：`npm run typecheck` / `npx vitest run`（46 文件 556 通过、1 跳过）/ `npm run lint:fix` / `cargo clippy --workspace --all-targets -- -D warnings`（0 告警）/ `cargo fmt` / `cargo test --workspace` 全绿
- 契约生成：`cargo test -p assistant-vault --lib export_bindings -- --test-threads=1` + `cargo test -p assistant-app --test protocol_bindings` 两次生成前后三个生成物哈希一致（幂等）；`vault.contract.ts` 与 HEAD 相同，`protocol.contract.ts` 仅含 C2/C3 新增
- A12 成立：`KNOWN_EFFORTS` 删除后目录声明的任意非空档位全部保留（`reasoning_levels_keep_any_non_empty_word`）；`none` / `off` 仍只翻 `toggle`（`reasoning_levels_route_none_and_off_to_toggle`）；选项 = 目录档位 − `unsupported`，别名档位（deepseek pro 的 `medium`）不被剔除（`keeps alias efforts that are only rewritten downstream`）
- A13 成立：`ProviderListDto.effort_support` 为只读投影（`ProviderInputDto` 未加字段），`ModelEffortSupportDto` 有 round-trip 用例；前端 `ReasoningSelector` 的 tooltip 追加「该端点不支持：…」，由 `unsupportedEffortSummary` 拼装
- A14 成立：`ATTACHMENT_LIMITS` 为唯一数值来源（全仓 Rust 检索只剩定义处与日志展示除法）；前端 `attachmentLimits()` 同步读后端 DTO、未加载即抛错；`admit.test.ts` 把 `maxImages` 置 1 后第 2 张图片被拒，`factory.test.ts` 把 `maxImageBytes` 置 10 后按 DTO 判定超限

---

## 门禁

- [x] `npm run typecheck`
- [x] `npm run test`
- [x] `npm run contract:check`（生成幂等：两次生成前后三个生成物哈希一致；`git diff --exit-code` 部分待提交时满足）
- [x] `cargo test`（cwd = `src-tauri`）
- [x] `cargo clippy -- -D warnings`（cwd = `src-tauri`）
- [x] `cargo fmt --check`（cwd = `src-tauri`）
- [x] `npm run lint:fix`（直接跑修复，不再重复跑检查）

---

## 验收对照

| 验收 | 规格断言 | 覆盖任务 |
|---|---|---|
| A1 | 纯图片 / 纯文件不再判空 | C1.1 |
| A2 | 派生文本给占位符，不出现半个占位符；前后端派生一致 | C1.2 |
| A3 | 坏项跳过并 `warn`，整表坏 `error`，两处解析点一致 | C1.3 |
| A4 | 含 capture 的流恰好一条 `warn`，落库无新增写入 | C1.4 |
| A5 | C1 不改对外 DTO 与数据库 schema | C1.1–C1.4 + `contract:check` |
| A6 | `vision` 有 API / 目录两源，`pdf` 有端点 / 目录两源，`attachment` 由目录产生（未收录 → `Unknown`），`provenance` 正确 | C2.1、C2.4 |
| A7 | 目录未收录 → `Unknown`，不再 `["text"]` / `false` | C2.2 |
| A8 | `Unknown` 与 `Unsupported` 走不同分支 | C2.6、C2.7 |
| A9 | 存量 `models_json`（无新字段）可解析：在列表中的模型结论不变，不在列表者 `false` → `Unknown` | C2.3 |
| A10 | 新增字段经 `contract:gen` 生成，round-trip 不丢 | C2.5 |
| A11 | 同一 `model_id` 在两个 provider 上结论可不同 | C2.3 |
| A12 | 目录档位不被白名单过滤；选项 = 目录档位 − 端点会丢弃的档位（别名档位保留） | C3.1、C3.2、C3.4 |
| A13 | 差集以可读说明呈现 | C3.3、C3.4 |
| A14 | 限额只有一处数值来源，前端随 DTO 变化 | C3.5、C3.6 |
| A15 | 全部门禁通过 | 门禁 |
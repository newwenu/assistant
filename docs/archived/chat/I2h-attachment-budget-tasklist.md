# I2h — 附件链路收敛（长文本预算 + PDF 降级）

> 来源：I2g 归档时迁出的残留项，见 [I2g-tasklist.md](./I2g-tasklist.md) 的「归档时状态」
> 前置：I2g 已完成（图片/文件附件主链路、能力感知投影、准入哨兵、规范化、offload）
> 目标：**消除正确性缺口**，而非补齐所有重依赖能力。HEIC/AVIF、PDF 渲染已明确不做（理由见 I2g 归档文档）。
> 口径来源：附件预算与 Token Usage 共用估算与阈值。I29 已归档到 `docs/archived/chat/`，只读，不再修改；本文只引用其定稿口径，不另立第二套数值。
> 最近核对：2026-11-02，按当前代码更新状态与事实描述。
> 维护约定：引用代码位置一律写「文件 + 符号名」，不写行号（行号随重构漂移）。

---

## 0. 当前状态

| 条目 | 状态 | 说明 |
| --- | --- | --- |
| P0.1 统一估算 | 已完成 | `token_estimate` 提供文本 / 图片 / 块 / 消息级统一口径；PDF 不再按 base64 线性记账 |
| P0.2 文件预算 | 已完成 | `project_files` 接收 `Option<usize>` 预算；`rehydrate` 已合并进单次遍历；按新消息优先分配 |
| P0.3 头尾截断 | 已完成 | 按 token 预算 60/40 切分，围栏自动闭合，标注真实数字 |
| P0.4 offload 扩展 | 已完成 | 最旧优先、覆盖 Tool、区分 `None` / `Some(0)`、卸载后记录实际数值 |
| P1 OpenAI PDF | 已完成 | 转换器按端点分派；Responses 支持 `input_file`；ChatCompletions 明确降级并告警 |
| P2 PDF 文本层 | 已完成 | `pdf-extract` 惰性抽取 + `derived/` 版本化侧车；单飞、`spawn_blocking` panic 隔离 |
| P3 验收测试 | 已完成 | 内联单测 + `tests/attachment_budget.rs`（17 个集成用例） |

已由 I29 落地、本清单不应重复实现的项：

- 文本估算加权口径（I29 T5.3）：`assistant-agent/src/token_estimate.rs`
- offload 预算取 `W - max_tokens`（I29 T5.2）：`chat_request::build_chat_request` 已在投影前算好
- 压缩判定窗口未知返回 `Undetermined`（I29 T5.5）：`compaction::CompactDecision`
- `context_length_exceeded` 分类与提示（I29 T5.6）：`assistant-providers/src/client/quirks.rs`
- 存储层 SQL 估算已移除（I29 E3），不要在本清单里再改 SQL

---

## 0.1 四个真实缺口（G1–G4）

### G1 — 多文件没有合计预算

`project_files` 对每个文件独立处理，单文件硬截断 `MAX_TEXT_FILE_BYTES = 512KB`（`assistant-agent/src/multimodal.rs`），但没有总量概念。按英文散文约 4 字节/token 折算，**单个 512KB 文件已约 13 万 token**；`MAX_FILES = 4` 时四个文件合计约 52 万 token，是 128k 窗口的 4 倍。中文文件更严重（3 字节/字，512KB ≈ 17.5 万字符）。

即「一个文件就足以顶满窗口」，四个文件只是让它更离谱。且历史消息里的附件不受任何总量约束——`project_files` 逐消息处理，每条用户消息各自独立投影。

### G2 — offload 只覆盖 Image

`offload_if_over_budget`（`assistant-agent/src/request_projection.rs`）现状：

1. 只遍历 `MessageRole::User`，且消息里不含 `ContentBlock::Image` 就跳过。文本 / 文件块超预算时函数静默返回——**对文本型溢出完全失效**
2. `ContentBlock::ToolResult` 未覆盖。注意当前 `prompt_builder` 不把 `tool_calls_json` / `tool_call_id` 透传到 `ChatMessage`，Tool 消息实际是 `role=tool` + 纯字符串；因此本项要先按 role 处理 Tool 消息，不能假设 `ToolResult` 块一定存在（见 P0.4.3）
3. 遍历用 `(0..len).rev()`，**从最新一条用户消息往前删**，恰好先牺牲用户刚发的那一轮；代码与注释「优先保留早期的完整图文上下文」相反
4. `context_limit = None` 时直接 `return`，把「窗口未知」当成「还没满」（对应 I29 的 A16 / M13）；压缩侧已返回 `Undetermined`，offload 侧仍未落地

### G3 — OpenAI 转换器忽略 PDF `cached_data`

`openai/message_converter.rs` 的 `ContentBlock::File` 分支无条件转文本。`rehydrate_files` 已按 `support_pdf` 填好 base64，转换器又丢弃，等于白做一次磁盘 IO。

影响面不止「GPT 不支持 PDF」：OpenAI 兼容协议下存在支持 PDF 的模型（如 GLM-5V-Turbo 走兼容端点），这类模型 `support_pdf = true`，`cached_data` 会被静默降级为 `[file: name (size)]`。

更麻烦的是 `convert_messages` 被**两条端点共用**（`openai/chat.rs` 与 `openai/responses_stream.rs`），而两者的原生附件字段不同：`/chat/completions` 只有 `text` / `image_url`，`/responses` 支持 `input_file`。同一函数无法同时产出两种形状，见 P1a / P1b。

另外 `resolve_support_pdf_from_stored` 只读模型能力（`models_json`），不感知端点。兼容端点标了 `pdf` 但走 `/chat/completions` 时，会先整份读盘、base64，再由转换器丢弃——**能力判定与发送形状必须统一**。

### G4 — 文件预算在现有管道里取不到（部分已修复）

管道顺序已经按 I29 T5.2 重排过：`build_chat_request` 先解析 `W` 与 `max_tokens`、算出 `offload_budget`，再进入 `project_chat_messages`：

```
解析 W 与 max_tokens / build_chat_options → 计算 offload_budget
→ prepare_messages → 图片投影 → rehydrate_files → project_files → offload
```

剩余缺口只有两处：

- `project_files` 仍在 `offload` 之前执行，但**没有接收预算**，拿不到「文件可用多少」
- `estimate_branch_tokens`（占用展示兜底）固定传 `offload_budget = None`，展示结果与实际出站消息不是同一份

因此**不需要再重排管道，也不需要从 `build_chat_options` 抽出 `resolve_max_tokens`**：`options.max_tokens` 在投影前已经可用，直接复用。

---

## 0.2 与 I29 的统一口径（动手前必读）

| 事项 | 唯一来源 | 现状 / 本清单的用法 |
| --- | --- | --- |
| 文本估算 | I29 T5.3 / E1：`assistant-agent/src/token_estimate.rs` | 已落地。ASCII 记 1/4、CJK 记 1、其余记 1/2，`div_ceil(4)`，**无每条消息 +4**。禁止再写第二套系数 |
| 图片估算 | I29 E2：固定 `765` | P0.1.3 将扩展为按宽高 + tile 估算（`85 + 170 * tiles`，768×768 恰为 765）；这是对 E2 的**显式增补**，需同步补测试，不得悄悄改 |
| 输入预算 | I29 T5.2：`W - max_tokens` | 已在 `build_chat_request` 落地。**文件预算必须复用同一个 `offload_budget`**，不得另算一个带 `SAFETY_RESERVE` 的值 |
| 窗口未知（`None`） | I29 T5.5：返回「不可判定」 | 压缩侧已落地（`CompactDecision::Undetermined`）；offload 侧仍未落地（P0.4.5） |
| 超窗错误处理 | I29 T5.6 | `client/quirks.rs` 已分类为 `ContextOverflow`；P0.4.6 只负责日志与可操作提示 |
| 安全裕度 | `compaction::COMPACTION_SAFETY_TOKENS = 1_000` | 只用于压缩阈值。若要把安全裕度引入 offload / 文件预算，必须同时改两处并单独立项，**不要在本清单里只给文件预算加 safety** |

**时序约束（当前已满足）**：预算必须在投影前算出。`project_files` 的签名扩展后，调用方直接透传 `offload_budget`；`offload` 的估算基于**投影后的出站消息**，不得回读数据库中的 `content`（I29 §3.3 已提示「按库内容估算会偏高」）。

---

## P0 — 长文本预算治理（最高优先）

### P0.1 统一 token 估算口径

现状：文本估算已统一到 `token_estimate::estimate_text_tokens`；`request_projection` 里仍有私有的 `estimate_blocks_tokens`（图片固定 `765`、文件已缓存按 E1 估算 base64、未缓存 `20`），且 `ContentBlock::File` 实际用 base64 长度当 token。

- [x] P0.1.1 `assistant-agent/src/token_estimate.rs` 已作为全仓唯一文本估算模块落地；系数为 ASCII 1/4、CJK 1、其余 1/2，不再新增第二套系数
- [x] P0.1.2 补齐 block 级接口，并把 `request_projection` 的私有实现迁进来：

```rust
pub fn estimate_text_tokens(s: &str) -> usize; // 已存在
pub fn estimate_image_tokens(width: Option<u32>, height: Option<u32>) -> usize;
pub fn estimate_block_tokens(b: &ContentBlock) -> usize;
pub fn estimate_messages_tokens(m: &[ChatMessage]) -> usize;
```

- [x] P0.1.3 图片按「归一化后的宽高 + tile」估算：`85 + 170 * ceil(w / 512) * ceil(h / 512)`；宽高缺失时回退固定 `765`（`85 + 170 × 4 = 765`，与现有常量自洽）。宽高已在 `ImageAttachmentRef.width / height` 由准入阶段填充。高分辨率图片不再被系统性低估；本条是 I29 E2 的增补，需在测试中同时锚定 768×768 = 765
- [x] P0.1.4 `ContentBlock::File` 分支**不得用 base64 长度当 token**。当前 `estimate_text_tokens(cached_data)` 会把 1MB PDF 的 base64（约 133 万字符）估成约 33 万 token，**挂任何 PDF 都必然判超预算，而 offload 又降不下来（G2）**。改为：
  - 原生 PDF（`cached_data` 非空且 `.pdf`）按固定常量记账（`30_000` token/份，作为「页数 × 每页估算」的粗粒度建模），不随 base64 长度线性增长
  - `cached_data` 为空 → 按 `handle_text()` 的文本估算
  - `30_000` 是占位常量：P2 落地后可按页数校准（P2.7）
- [x] P0.1.5 `chat_runtime::estimate_tokens` 已改为转发 `estimate_text_tokens`；`agent_modes` 的 SQL 估算已被 I29 T3 移除。本项无剩余工作

### P0.2 `project_files` 接收预算并分配配额

- [x] P0.2.1 签名扩展为带预算。预算必须是 `Option`：`None` 表示窗口未知（不可判定），**不得压成 `0`**，否则下游会把「未知」当成「预算耗尽、全部降级」：

```rust
pub async fn project_files(
    messages: &mut [ChatMessage],
    store: &AttachmentStore,
    support_pdf: bool,
    file_budget_tokens: Option<usize>,
)
```

- [x] P0.2.2 预算来源：**直接复用 `build_chat_request` 已算好的 `offload_budget`**（`W - max_tokens`，与 I29 T5.2 一致），不要另写公式，也不要抽 `resolve_max_tokens`
- [x] P0.2.3 配额分配顺序写死，避免实现各自解释：
  1. 先扣除原生 PDF 的固定常量（P0.1.4）与图片估算
  2. 剩余预算中，**当前轮（最后一条用户消息）的文件优先**
  3. 历史文件按消息从新到旧分配
  4. 单文件最低配额建议 2k token；不足时按上述顺序满足，其余降级为 `file_handle_text`
  5. 降级顺序与分配顺序相反：**最旧的文件先降级**
- [x] P0.2.4 `file_budget_tokens = None` 时按 I29 T5.5 走「不可判定」分支：按**总上限** `MAX_UNKNOWN_WINDOW_FILE_TOKENS`（建议 32k，覆盖所有文件合计）投影，并 `log::warn!` 记录「窗口未知，按文件总额上限投影」。不要用「每文件 32k」——4 个文件仍会到 128k，等于没限
- [x] P0.2.5 预算为 `Some(0)` 时**全部文件**降级为 `file_handle_text`，包括本可原生发送的 PDF
- [x] P0.2.6 **原生 PDF 也必须过预算门**：在 `should_send_file_as_is` 之前判断。剩余预算不足以容纳 PDF 固定常量时，依次回退到 P2 侧车文本、`file_handle_text`；不得因为 `support_pdf = true` 就直接 `push`
- [x] P0.2.7 `rehydrate_files` 不能早于预算做无用功：优先把 rehydrate 合并进 `project_files` 的单次遍历；若保留两段，必须把 `file_budget_tokens` 传给 `rehydrate_files`，跳过注定降级的 PDF，避免整份读盘与 base64 内存占用
- [x] P0.2.8 展示路径（`estimate_branch_tokens`）必须传**与发送相同**的预算；当前固定传 `None` 会让展示占用偏高。mode 未知时用默认 `max_tokens` 计算，不要用「跳过 offload」当作默认
- [x] P0.2.9 `None` 只在**发送路径**记录一次 warn（每请求一次，不要每文件一次）；展示路径不应触发 warn。若后续要区分「未知」与「不做降级」，再引入枚举，不要用 `Option` 硬扛两种语义

### P0.3 头尾截断替代硬截断

现状（`assistant-agent/src/multimodal.rs::read_as_text`）有三个问题：

1. 判定用 `data.len()`（字节），截断用 `text.chars().take(512 * 1024)`（字符）——两者口径不同。600KB 的中文文件字节数超限、字符数只有约 20 万，`take` 会把内容**全量保留**，截断完全失效
2. 截断标注是无条件追加的固定字符串 `[truncated]`，即使一个字都没删
3. 只保留头部，尾部信息（最新内容、结论、diff 末段）全部丢失

- [x] P0.3.1 改为按 token 预算截断（复用 P0.1 的 `estimate_text_tokens`），头 60% / 尾 40%
- [x] P0.3.2 截断标注写真实数字，便于模型理解：

```
[file: name — truncated, kept head 1234 + tail 823 tokens of 20456]
```

- [x] P0.3.3 统一按字符遍历（`chars()`）做预算切分，不再混用字节/字符口径。`MAX_TEXT_FILE_BYTES` 不再承担 token 语义
- [x] P0.3.4 截断后必须闭合 Markdown 代码围栏：头部若切在围栏内部，补一行闭合围栏；尾部片段前补一行带语言标记的开栏。否则模型会看到半截代码块

### P0.4 offload 覆盖文本块与工具结果

- [x] P0.4.1 `offload_if_over_budget` 增加文本块卸载：从**最旧的用户消息**开始，把注入的文件正文块替换为占位文本（保留 `file_handle_text`，保留文本消息本身的结构与指令）
  - 现有实现是 `(0..len).rev()`，**从最新一条用户消息往前删**，恰好先牺牲用户刚发的那一轮。本项必须一并纠正为「最旧优先」
- [x] P0.4.2 卸载顺序：文件正文块（体积最大）→ 图片块 → Tool 消息的 `content`（按头尾截断，存在 `tool_call_id` 时保留）→ 历史用户文本块（最后手段，避免破坏指令）
- [x] P0.4.3 工具结果的真实数据流要先核对：`prompt_builder::prepare_messages` 当前不读 `tool_calls_json`、不填 `tool_call_id`，Tool 消息是 `role=tool` + 纯字符串。**在本项落地的同一阶段，按 role 处理 `MessageRole::Tool` 的 `content` 截断**；`ContentBlock::ToolResult` 分支只在 I18/I31 把工具链透传到 `ChatMessage` 后再补，不能写成死代码
- [x] P0.4.4 `Assistant` 消息与 `ToolUse` 块不动（改动会破坏工具调用链）。`Tool` 消息只截断内容、不改 id，因此不在禁令范围内
- [x] P0.4.5 最后一条用户消息的**指令文本永不整体降级**；其附件正文（文件 / 图片）仍受预算约束。G1 的场景正是最后一条消息带 4 个文件，不能因为「最新消息」就把附件原样全发
- [x] P0.4.6 `budget = None` 时按 I29 T5.5 返回「不可判定」：`log::warn!` 后原样返回。当前实现 `let Some(budget) = ... else { return; }` 把未知当成「不超限」，必须改掉
- [x] P0.4.7 卸载后仍超预算时 `log::warn!(estimated, budget, over_ratio)` 记录实际数值，并按 I29 T5.6 处理 `context_length_exceeded`：给出可操作提示，**不得静默交给 Provider 报错**
- [x] P0.4.8 估算必须与实际出站形状一致：`openai/message_converter.rs` 当前把 `Thinking` / `ToolUse` 转成空对象，估算却计入它们，会虚高并触发不必要的降级。要么修正转换器，要么在估算时排除「不会被发送的块」，二者取其一并在测试中锚定

### P0.5 文本文件扩展名覆盖（低优先，建议同期）

`is_text_file`（`assistant-agent/src/multimodal.rs`）只认 13 种扩展名，`.rs`、`.toml`、`.log`、`.sql`、`.sh` 等本项目常用文件只能拿到 `file_handle_text`，模型读不到内容。

- [x] P0.5.1 补齐常见源码 / 配置扩展名（`.rs`、`.toml`、`.java`、`.go`、`.c`、`.h`、`.cpp`、`.sh`、`.ps1`、`.sql`、`.log`），沿用现有 UTF-8 + 替换字符比例判定
- [x] P0.5.2 若本阶段不做，则移入「明确不做」并写明理由，避免看起来像遗漏

---

## P1 — OpenAI 转换器

`convert_messages` 被两条端点共用，而两者的原生附件字段不同：

| 调用点 | 端点 | 原生附件 part |
| --- | --- | --- |
| `openai/chat.rs` | `/chat/completions` | `text` / `image_url`（无原生 PDF） |
| `openai/responses_stream.rs` | `/responses` | `input_text` / `input_image` / `input_file` |

同一函数无法同时产出两种形状，必须按目标端点分派。拆成两段：P1a 是独立的形状正确性问题，建议前置；P1b 的 PDF 原生发送在 P2 落地后价值下降，可作为可选。

### P1a — Responses 形状修正与能力判定（建议前置）

- [x] P1a.1 `convert_content_block` / `build_content` 增加目标参数（`OpenAiTarget::ChatCompletions` / `OpenAiTarget::Responses`），两个调用点各自传入
- [x] P1a.2 `Responses` 的文本 / 图片 part 改为 `input_text` / `input_image`（`image_url` 为字符串），不要再沿用 chat completions 的形状
- [x] P1a.3 `support_pdf` 改为**端点感知**：`/chat/completions` 路径不得把 PDF 当作原生附件，`rehydrate_files` 对该路径直接跳过，避免「读盘 → base64 → 转换器丢弃」
- [x] P1a.4 单测：两种端点的文本 / 图片 part 形状；`/chat/completions` + `support_pdf = true` 不 rehydrate

### P1b — Responses 原生 PDF（P2 之后可选）

- [x] P1b.1 `Responses` 且 `cached_data` 非空且扩展名为 `.pdf`：产出 `{"type": "input_file", "file_data": "data:application/pdf;base64,...", "filename": ...}`
- [x] P1b.2 `ChatCompletions`：维持 `attachment.handle_text()` 文本降级，但**必须** `log::warn!` 说明「PDF 已降级为占位文本」，避免静默
- [x] P1b.3 单测：`Responses` + `.pdf` + `cached_data` → `input_file`；`ChatCompletions` + `.pdf` → 文本；`cached_data` 为空 → 文本；非 PDF 扩展名 → 文本
- [x] P1b.4 明确边界：P1b 单独无法解决多 PDF 场景（4 份 × 30k = 120k 已超常见窗口），真正的正确性修复来自 P2 侧车文本

---

## P2 — PDF 文本层抽取

目标：不支持 PDF 的模型也能读到 PDF 内容（文本层可读即可，不要求排版）。

- [x] P2.1 引入纯 Rust 依赖（`pdf-extract` 0.12，MIT，内部 `lopdf` / `postscript` / `euclid` 等均为 MIT/Apache-2.0），不引入 C/C++ 工具链
- [x] P2.2 **惰性抽取 + 内容寻址缓存**，而不是准入时无条件抽取：`admit_attachments` 只入库原文件；首次需要侧车时才抽取，结果按原文件的 SHA-256 落侧车。`AttachmentStore` 已是内容寻址（`save_attachment` 以 SHA-256 命名，见 `assistant-storage/src/attachment_store.rs`），同一份 PDF 天然只抽一次
  - 原稿「准入时抽取」的问题：`support_pdf = true` 的模型根本用不到侧车，准入时抽等于白付 CPU 与延迟
- [x] P2.3 `project_file_block` 读取侧车文本，走与文本文件相同的预算截断路径（P0.3）
- [x] P2.4 失败必须回退：扫描件（无文本层）、加密 PDF、CJK 字体编码异常 → 回退 `file_handle_text`，不阻断发送
- [x] P2.5 侧车缺失（历史附件未抽取）时按需补抽一次，仍失败则回退（与 P2.2 是同一机制，不是两套路径）
- [x] P2.6 侧车落地约束（原稿缺失，必须补齐）：
  - **独立命名空间**：侧车放 `AttachmentStore` 下的子目录 `derived/`，不与附件根目录混放。`list_all` 只遍历根目录且只收 `is_file()`，天然不会把侧车当附件
  - **版本化**：文件名含抽取器版本（`{sha}.extract-v1.txt`），算法变更后不会读到旧文本
  - **单飞**：同一 SHA 并发请求只抽取一次（进程内按附件 id 加锁 / 去重），避免重复 CPU
  - **不阻塞异步运行时**：抽取放 `tokio::task::spawn_blocking`
  - **panic 隔离**：PDF 是不可信输入，解析器 panic 由 `JoinError` 隔离，失败按 P2.4 回退
  - **生命周期**：删除原附件时同步删除侧车；`storage.rs` 的存储统计与孤儿清理天然忽略 `derived/`
- [ ] P2.7（可选）原生 PDF 计费校准：既然 P2 引入了解析器，可顺带读取页数并按 `每页 token × 页数` 估算，侧车缓存页数；在落地前继续用 P0.1.4 的 `30_000` 常量
- [x] P2.8 两条 PDF 路径的优先级必须明确（见下表），避免同一份文件两套计费口径。**建议原生优先、侧车兜底**：原生 PDF 保留表格与版式，侧车文本只服务不支持的模型；若后续实测原生计费偏差过大，再切换为「侧车优先」并单独立项

| 条件 | 发送形式 | 记账口径 |
| --- | --- | --- |
| `support_pdf = true` 且 `cached_data` 非空 | 原生文件（`input_file` / `document`） | P0.1.4 固定常量 / P2.7 页数估算 |
| `support_pdf = false` 且侧车可用 | 侧车文本 | 按文本 token（P0.3 预算内） |
| 侧车不可用 + `support_pdf = true` | 原生文件（`cached_data` 非空；为空则回退占位） | P0.1.4 固定常量 |
| 侧车不可用 + `support_pdf = false` | `file_handle_text` 占位 | 按占位文本 |

> **边界声明**：文本抽取不承诺必成功。扫描件与加密 PDF 属预期失败，不是缺陷。

---

## P3 — 验收测试

`assistant-agent` 已有集成测试（`tests/chat_runtime_spec.rs`、`tests/compaction.rs`），但 `src/` 内没有 `#[cfg(test)]`。新增的模块边界条件最密集，建议：

- 纯函数（`token_estimate`、截断）放模块内联单测
- 涉及 `AttachmentStore` / 临时目录的用例放 `tests/attachment_budget.rs`，与现有集成测试风格一致

- [x] P3.1 `request_projection`：超预算卸载图片 / 超预算卸载文件正文 / 超预算截断 Tool 消息 / 不超预算不动 / **`context_limit = None` 打 warn 且不静默返回**（原稿的 `context_limit = 0` 早退用例与 `Option<u32>` 类型不符，已改）
- [x] P3.2 `request_projection`：最后一条用户消息的文本指令不被降级；附件仍可降级；降级顺序为「最旧优先」
- [x] P3.3 `multimodal::admit_attachments`：不可信通道拒绝附件（V2-10）、`..` 与绝对路径拒绝（V2-11）、伪造 MIME 拒绝
- [x] P3.4 `multimodal::project_files`：非 PDF → 文本（V2-3）、`support_pdf = false` 时 PDF 也降级（V2-5）、多文件配额分配（4 × 512KB 在 64k 预算下总和不超）、`None` 预算走**总额上限**、`Some(0)` 全降级（含原生 PDF）
- [x] P3.5 `normalization`：长边 2048 缩放、BMP/TIFF → PNG、MIME 与字节不符时拒绝
- [x] P3.6 `token_estimate`：纯 ASCII / 纯中文 / 混排口径回归；`estimate_image_tokens` 在 768×768 恰为 `765`、随 tile 数递增、宽高缺失回退 `765`；`ContentBlock::File` 带 PDF `cached_data` 时**不得**随 base64 长度线性增长
- [x] P3.7 `truncate_to_tokens`：CJK 下不超预算、head / tail 比例正确、围栏闭合、未截断时不打 `[truncated]`
- [x] P3.8 展示与发送一致性：`estimate_branch_tokens` 与 `project_chat_messages` 使用同一预算，降级后两侧数值一致
- [x] P3.9 P2 侧车：临时目录 + `derived/` 子目录；抽取失败回退；`list_all` / 孤儿清理不把侧车当附件

> 构造 `AttachmentStore` 需临时目录（`AttachmentStore::new(tempdir)`）。估算口径的断言与 I29 T6.5 共用，避免两套期望值。

---

## 实施记录（2026-11-02）

实现过程中对原清单做了 4 处显式修正，均已落到代码与测试：

1. **文件预算不是 `offload_budget` 本身**：`file_budget = offload_budget - estimate_non_file_tokens(消息)`。否则文件会与 system prompt、图片、工具结果重复占用同一份输入预算。
2. **文件降级收敛到 `project_files`**：`ContentBlock::File` 投影后已变成 `Text`，`offload` 无法再识别文件正文。因此 `offload` 只处理图片 → Tool → 历史用户文本，文件不再二次处理；`rehydrate_files` 已删除并合并进 `project_files`。
3. **`Some(0)` 必须与 `None` 区分**：`W - max_tokens` 可能得到 `0`，表示「没有输入预算、必须降级」；只有 `None` 才是「窗口未知、不可判定」。
4. **单文件配额加上限**：文件数 ≥ 2 时每个文件最多占文件预算的 50%，避免最新一个文件吞掉全部配额后其余文件直接降级。

另有两处实现细节偏离原稿，原因写在括号内：

- `MAX_TEXT_FILE_BYTES` 不再参与截断（准入阶段 `MAX_FILE_BYTES = 16MB` 仍是读取上限；正文截断全部按 token，标注数字因此是精确值）。
- 文本解码由「UTF-8 + 替换字符比例」改为「UTF-8 优先，失败时用 `chardetng` 猜测编码」，并补充控制字符比例兜底（原 `invalid_ratio` 在 `String::from_utf8` 失败时根本到不了，属于死代码）。

P1a/P1b 已按清单实现，但当前前端 `ApiFormat` 只有 `openai | anthropic | local`，没有 Responses 入口；该路径属于「协议就绪、UI 未开放」，不影响本轮 G3 的 ChatCompletions 修复。

---

## 涉及文件

| 文件 | 变更 |
|------|------|
| `src-tauri/crates/assistant-agent/src/token_estimate.rs` | 文本 / 图片 / block / 消息级统一估算、PDF 固定记账、头尾截断 |
| `src-tauri/crates/assistant-agent/src/request_projection.rs` | 共享块级估算、最旧优先卸载、Tool 截断、`None` / `Some(0)` 分支 |
| `src-tauri/crates/assistant-agent/src/multimodal.rs` | `project_files` 预算参数、合并 rehydrate、头尾截断、PDF 侧车读取、文本扩展名 |
| `src-tauri/crates/assistant-agent/src/pdf_text.rs` | **新增**：PDF 文本层惰性抽取、单飞、panic 隔离、侧车缓存 |
| `src-tauri/crates/assistant-agent/src/chat_request.rs` | 文件预算计算、端点感知 `support_pdf`、展示路径同预算 |
| `src-tauri/crates/assistant-agent/src/chat_options.rs` | `DEFAULT_MAX_TOKENS` 常量 |
| `src-tauri/crates/assistant-providers/src/openai/message_converter.rs` | 按端点分派的形状（P1a）与 PDF 分支（P1b） |
| `src-tauri/crates/assistant-providers/src/openai/chat.rs` / `responses_stream.rs` | 传入目标端点 |
| `src-tauri/crates/assistant-storage/src/attachment_store.rs` | PDF 侧车命名空间、读写与生命周期 |
| `src-tauri/crates/assistant-agent/tests/attachment_budget.rs` | **新增**：预算、截断、侧车、准入、展示一致性的集成测试 |
| `src-tauri/crates/assistant-agent/Cargo.toml` | `pdf-extract`、`chardetng`、`encoding_rs`、`tempfile`（dev） |

## 明确不做

- HEIC/HEIF、AVIF 解码（理由见 I2g 归档文档）
- PDF 转图片渲染、混合模式
- 分块摘要（引入二次模型调用，成本不匹配收益）
- 流式注入（移交 I18 工具架构）
- docx / xlsx / pptx 等 Office 文档解析：`is_text_file` 只认文本扩展名，Office 文件当前只能降级为 `file_handle_text`。**这是已接受的边界，不是缺陷**；若后续要补，参考项目 OpenBitFun 的做法是引入 `anydoc` 统一转 Markdown（带内容哈希缓存），应作为独立条目整体引入，不与本清单混做
- 为文件预算单独引入 `SAFETY_RESERVE`（安全裕度只属于压缩阈值；要改必须同时改 offload，单独立项）

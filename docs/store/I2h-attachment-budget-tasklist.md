# I2h — 附件链路收敛（长文本预算 + PDF 降级）

> 来源：I2g 归档时迁出的残留项，见 [I2g-tasklist.md](../achievement3/I2g-tasklist.md) 的「归档时状态」
> 前置：I2g ✅（图片/文件附件主链路、能力感知投影、准入哨兵、规范化、offload）
> 目标：**消除正确性缺口**，而非补齐所有重依赖能力。HEIC/AVIF、PDF 渲染已明确不做（理由见 I2g 归档文档）。

---

## 0. 背景：四个真实缺口（G1–G4）

### G1 — 多文件没有合计预算

`project_files` 对每个文件独立处理，单文件硬截断 `MAX_TEXT_FILE_BYTES = 512KB`（`assistant-agent/src/multimodal.rs:14`），但没有总量概念。按英文散文约 4 字节/token 折算，**单个 512KB 文件已约 13 万 token**；`MAX_FILES = 4`（`assistant-agent/src/multimodal.rs:12`）时四个文件合计约 52 万 token，是 128k 窗口的 4 倍。中文文件更严重（3 字节/字，512KB ≈ 17.5 万字符）。

即「一个文件就足以顶满窗口」，四个文件只是让它更离谱。且历史消息里的附件不受任何总量约束——`project_files` 逐消息处理，每条用户消息各自独立投影。

### G2 — offload 只覆盖 ImageBlock

`offload_if_over_budget`（`assistant-agent/src/request_projection.rs:11-65`）存在四个叠加缺陷：

1. 循环里消息若不含 `ContentBlock::Image` 就 `continue`。当超预算的根源是文本/文件块时，所有消息被跳过，函数静默返回，超限请求照样发出——**对文本型溢出完全失效**
2. `ContentBlock::ToolResult` 完全未覆盖。agent 循环里 token 的实际大头是工具结果，窗口被打满时它降不下来
3. 遍历用 `(0..len).rev()`，**从最新一条用户消息往前删**，恰好先牺牲用户刚发的那一轮；而函数注释写的是「优先保留早期的完整图文上下文」，与代码相反
4. `context_limit = None` 时 `let Some(budget) = ... else { return; }`，把「窗口未知」当成「还没满」（对应 I29 的 A16 / M13）

### G3 — OpenAI 转换器忽略 PDF `cached_data`

`openai/message_converter.rs:43-46` 无条件把 `ContentBlock::File` 转文本。`rehydrate_files` 已按 `support_pdf` 填好 base64，转换器又丢弃，等于白做一次磁盘 IO。

影响面不止「GPT 不支持 PDF」：OpenAI 兼容协议下存在支持 PDF 的模型（如 GLM-5V-Turbo 走兼容端点），这类模型 `support_pdf = true`，`cached_data` 会被静默降级为 `[file: name (size)]`。

更麻烦的是 `convert_messages`（`assistant-providers/src/openai/message_converter.rs:70`）被**两条端点共用**，而两者的原生附件字段不同：`openai/chat.rs:37` 走 `/chat/completions`（无原生 PDF），`openai/responses_stream.rs:148` 走 `/responses`（支持 `input_file`）。同一函数无法同时产出两种形状，见 P1。

### G4 — 预算在现有管道里算不出来

`build_chat_request`（`assistant-agent/src/chat_request.rs:42-68`）的实际顺序是：

```
prepare_messages → 图片投影 → rehydrate_files → project_files → 解析窗口 → offload → build_chat_options
```

- `project_files`（`:50`）在窗口解析（`:57`）**之前**，拿不到 `W`
- `offload`（`:58`）在 `build_chat_options`（`:63`）**之前**，拿不到 `max_tokens`
- `max_tokens` 恒为 `mode.max_tokens.unwrap_or(4096)`（`assistant-agent/src/chat_options.rs:47`），因此「缺失时按比例兜底」是死分支

照原稿 P0.2.2 直接写，预算根本算不出来，必须先重排顺序（见 §0.1）。

---

## 0.1 与 I29 的统一口径（动手前必读）

附件预算与 Token Usage 共用同一套估算与阈值，两份文档不得各写一套数值。以下四条**以 I29 为唯一来源**：

| 事项 | 唯一来源 | I2h 的用法 |
| --- | --- | --- |
| 文本 / 图片 / 文件 token 估算式 | `I29-token-usage-semantics-spec.md` §3.4–§3.5、`I29-token-usage-tasklist.md` T5.3 | 调用共享函数，**不在本文定义第二套系数** |
| 输入预算公式 | I29 T5.1 / T5.2：`W - max_tokens - safety` | P0.2 直接引用，不自创「25% 兜底」 |
| 窗口未知（`None`） | I29 T5.5：返回「不可判定」，不得当成「不超限」 | P0.2 / P0.4 必须实现 `None` 分支 |
| 超窗错误处理 | I29 T5.6：捕获 `context_length_exceeded` 并给可操作提示 | P0.4 引用，不写「由 Provider 兜底」 |

**时序约束**：P0.2 必须先重排 `build_chat_request`，否则预算取不到输入：

```
解析 W 与 max_tokens → 计算输入预算 → 投影（project_files）→ 降级（offload）→ build_chat_options
```

重排后 `offload` 的估算基于**投影后的出站消息**，不得回读数据库中的 `content`（I29 §3.3 已提示「按库内容估算会偏高」）。

---

## P0 — 长文本预算治理（最高优先）

### P0.1 统一 token 估算口径

现状存在三套互不相同的估算（`I29-token-usage-semantics-spec.md` §3.4 的 E1/E2/E3）：

- `assistant-agent/src/chat_runtime.rs:496` — `estimate_tokens`：`(len + 3) / 4 + 4`
- `assistant-agent/src/request_projection.rs:81-96` — 各分支 `text.len() / 4`，图片固定 `765`，文件已缓存 `cached_data.len() / 4`，未缓存 `20`
- `assistant-storage/src/agent_modes.rs:284` — SQL 版 `COALESCE(tokens, (LENGTH(content) + 3) / 4 + 4)`

三处都用 `len() / 4`（字节数除以 4）。UTF-8 中文 3 字节/字，折算约 0.75 token/字，实际约 1~1.5 token/字，**中文与代码场景系统性低估 25%~50%**。

- [ ] P0.1.1 新建 `assistant-agent/src/token_estimate.rs`，作为**全仓唯一**估算模块。系数与结构开销以 `I29-token-usage-tasklist.md` T5.3 为准（ASCII 0.3 token/字符、非 ASCII 0.6 token/字符、每条消息 +4），**本文不另立第二套系数**
- [ ] P0.1.2 模块至少提供以下接口，供投影、降级、压缩判定共用：

```rust
pub fn estimate_text_tokens(s: &str) -> usize;
pub fn estimate_image_tokens(width: Option<u32>, height: Option<u32>) -> usize;
pub fn estimate_block_tokens(b: &ContentBlock) -> usize;
pub fn estimate_messages_tokens(m: &[ChatMessage]) -> usize;
```

- [ ] P0.1.3 图片按「归一化后的宽高 + tile」估算：`50 + ceil(w / 512) * ceil(h / 512) * 200`，宽高缺失时回退固定 `765`。宽高已在 `ImageAttachmentRef.width / height`（`assistant-providers/src/types/message.rs:57-59`）由准入阶段填充。当前固定 `765` 与分辨率无关，高分辨率图片被系统性低估
- [ ] P0.1.4 `ContentBlock::File` 分支**不得用 base64 长度当 token**。当前 `data.len() / 4` 会把 1MB PDF 的 base64（约 133 万字符）估成约 33 万 token，**挂任何 PDF 都必然判超预算，而 offload 又降不下来（G2）**。改为：
  - 原生 PDF（`cached_data` 非空且 `.pdf`）按固定常量记账（建议 `30_000` token/份，作为「页数 × 每页估算」的粗粒度建模），不随 base64 长度线性增长
  - `cached_data` 为空 → 按 `handle_text()` 的文本估算
- [ ] P0.1.5 `chat_runtime::estimate_tokens` 改为调用 `estimate_text_tokens`；`agent_modes` 的 SQL 估算在 I29 T3（聚合改造）落地后一并移除，**不要在本清单里单独改 SQL**

### P0.2 `project_files` 接收预算并分配配额

- [ ] P0.2.1 签名扩展为带预算。预算必须是 `Option`：`None` 表示窗口未知（不可判定），**不得压成 `0`**，否则下游会把「未知」当成「预算耗尽、全部降级」：

```rust
pub async fn project_files(
    messages: &mut [ChatMessage],
    store: &AttachmentStore,
    support_pdf: bool,
    file_budget_tokens: Option<usize>,
)
```

- [ ] P0.2.2 预算来源（`assistant-agent/src/chat_request.rs` 管道内计算），公式与 I29 T5.1 / T5.2 一致：

```
输入预算 = W - max_tokens - SAFETY_RESERVE
文件配额 = 输入预算 - 已占用（非文件块消息的估算）
```

- `W` 取 `resolve_context_window_from_stored` 的结果，**可能为 `None`**
- `max_tokens` 取 `chat_options::resolve_max_tokens(mode)`（需从 `build_chat_options` 抽出，与 `ChatOptions` 共用同一个值，当前为 `mode.max_tokens.unwrap_or(4096)`）
- `SAFETY_RESERVE` 用单一常量（参考 OpenBitFun `10_000` / codex `12_000`），与 I29 T5.1 共用
- **时序前提**：`project_files` 当前在窗口解析之前、`build_chat_options` 之前执行，必须先按 §0.1 重排，否则 `W` 与 `max_tokens` 都取不到
- 删去原稿的「`max_tokens` 缺失按 25% 兜底」：该分支在当前链路不可达（G4）

- [ ] P0.2.3 配额分配：先按待投影文件数均分，单文件最低配额（建议 2k token）不足时按剩余预算顺序优先满足靠前的文件，其余降级为 `file_handle_text`。**原生 PDF 按 P0.1.4 的固定常量从总预算中扣除**——不是「不占配额」：PDF 仍消耗 provider 输入 token，不计账会让预算系统性偏乐观
- [ ] P0.2.4 `file_budget_tokens = None` 时按 I29 T5.5 走「不可判定」分支：每个文件用绝对上限 `MAX_TEXT_FILE_TOKENS`（建议 32k）投影，并 `log::warn!` 记录「窗口未知，按单文件上限投影」。**既不得**当成「预算充足」无限注入，**也不得**当成「预算 ≤ 0」全部降级
- [ ] P0.2.5 预算为 `Some(0)` 时全部文件降级为 `file_handle_text`（不注入正文）

### P0.3 头尾截断替代硬截断

现状（`assistant-agent/src/multimodal.rs:613-626`）有三个问题：

1. 判定用 `data.len()`（字节），截断用 `text.chars().take(512 * 1024)`（字符）——两者口径不同。600KB 的中文文件字节数超限、字符数只有约 20 万，`take` 会把内容**全量保留**，截断完全失效；
2. 截断标注是无条件追加的固定字符串 `[truncated]`，即使一个字都没删；
3. 只保留头部，尾部信息（最新内容、结论、diff 末段）全部丢失。

- [ ] P0.3.1 改为按 token 预算截断（复用 P0.1 的 `estimate_text_tokens`），头 60% / 尾 40%
- [ ] P0.3.2 截断标注写真实数字，便于模型理解：

```
[file: name — truncated, kept head 1234 + tail 823 tokens of 20456]
```

- [ ] P0.3.3 统一按字符遍历（`chars()`）做预算切分，不再混用字节/字符口径。`MAX_TEXT_FILE_BYTES` 降级为「读取前的字节护栏」，不再承担 token 语义
- [ ] P0.3.4 截断后必须闭合 Markdown 代码围栏：头部若切在围栏内部，补一行闭合围栏；尾部片段前补一行带语言标记的开栏。否则模型会看到半截代码块

### P0.4 offload 覆盖文本块与工具结果

- [ ] P0.4.1 `offload_if_over_budget` 增加文本块卸载：从**最旧的用户消息**开始，把注入的文件正文块替换为占位文本（保留 `file_handle_text`，保留文本消息本身的结构与指令）
  - 现有实现是 `(0..len).rev()`，**从最新一条用户消息往前删**，恰好先牺牲用户刚发的那一轮；函数注释写的是「优先保留早期上下文」，与代码相反。本项必须一并纠正为「最旧优先」
- [ ] P0.4.2 卸载顺序：文件正文块（体积最大）→ 图片块 → `ContentBlock::ToolResult`（agent 循环里的实际大头，按头尾截断而非整块丢弃，保留 `tool_call_id`）→ 历史用户文本块（最后手段，避免破坏指令）
- [ ] P0.4.3 `Assistant` 消息与 `ToolUse` 块不动（改动会破坏工具调用链）。`ToolResult` 只截断内容、不改 id，因此不在禁令范围内——原稿「`Assistant` / `Tool` 消息一律不动」会让 agent 场景彻底降不下来
- [ ] P0.4.4 最后一条用户消息**永不降级**（用户刚发的那一轮是当前意图，降级等于答非所问）
- [ ] P0.4.5 `input_budget = None` 时按 I29 T5.5 返回「不可判定」：`log::warn!` 后原样返回。当前实现 `let Some(budget) = ... else { return; }` 把未知当成「不超限」，必须改掉
- [ ] P0.4.6 卸载后仍超预算时 `log::warn!(estimated, budget, over_ratio)` 记录实际数值，并按 I29 T5.6 处理 `context_length_exceeded`：给出可操作提示，**不得静默交给 Provider 报错**（原稿「由 Provider 报错兜底」与 I29 T5.6 冲突）

---

## P1 — OpenAI 转换器补 PDF 分支

`convert_messages`（`assistant-providers/src/openai/message_converter.rs:70`）被两条端点共用，而两者的原生附件字段不同：

| 调用点 | 端点 | 是否支持原生 PDF |
| --- | --- | --- |
| `openai/chat.rs:37` | `/chat/completions` | 否，只有 `text` / `image_url` |
| `openai/responses_stream.rs:148` | `/responses` | 是，`input_file` |

同一函数无法同时产出两种形状，因此必须按目标端点分派，不能只加一个无条件分支。

- [ ] P1.1 `convert_content_block` 增加目标参数（`OpenAiTarget::ChatCompletions` / `OpenAiTarget::Responses`），两个调用点各自传入
- [ ] P1.2 `Responses` 且 `cached_data` 非空且扩展名为 `.pdf`：产出 `{"type": "input_file", "file_data": "data:application/pdf;base64,...", "filename": ...}`
- [ ] P1.3 `ChatCompletions`：维持 `attachment.handle_text()` 文本降级，但**必须** `log::warn!` 说明「PDF 已降级为占位文本」，避免静默
- [ ] P1.4 顺带核对 `Responses` 的文本 / 图片 part 形状（应为 `input_text` / `input_image`，当前共用产出 `text` / `image_url`，本身就有偏差）
- [ ] P1.5 单测：`Responses` + `.pdf` + `cached_data` → `input_file`；`ChatCompletions` + `.pdf` → 文本；`cached_data` 为空 → 文本；非 PDF 扩展名 → 文本

---

## P2 — PDF 文本层抽取

目标：不支持 PDF 的模型也能读到 PDF 内容（文本层可读即可，不要求排版）。

- [ ] P2.1 引入纯 Rust 依赖（`pdf-extract` 或 `lopdf`），不引入 C/C++ 工具链
- [ ] P2.2 **惰性抽取 + 内容寻址缓存**，而不是准入时无条件抽取：`admit_attachments` 只入库原文件；首次需要侧车时才抽取，结果按原文件的 SHA-256 落侧车。`AttachmentStore` 已是内容寻址（`save_attachment` 以 SHA-256 命名，见 `assistant-storage/src/attachment_store.rs:74`），同一份 PDF 天然只抽一次
  - 原稿「准入时抽取」的问题：`support_pdf = true` 的模型根本用不到侧车，准入时抽等于白付 CPU 与延迟
- [ ] P2.3 `project_file_block` 读取侧车文本，走与文本文件相同的预算截断路径（P0.3）
- [ ] P2.4 失败必须回退：扫描件（无文本层）、加密 PDF、CJK 字体编码异常 → 回退 `file_handle_text`，不阻断发送
- [ ] P2.5 侧车缺失（历史附件未抽取）时按需补抽一次，仍失败则回退（与 P2.2 是同一机制，不是两套路径）
- [ ] P2.6 明确两条 PDF 路径的优先级，避免同一份文件有两套计费口径。**侧车文本优先**（可预算、跨 provider 行为一致），原生 PDF 仅作侧车不可用时的兜底：

| 条件 | 发送形式 | 记账口径 |
| --- | --- | --- |
| 侧车可用（与 `support_pdf` 无关） | 侧车文本 | 按文本 token（P0.3 预算内） |
| 侧车不可用 + `support_pdf = true` | 原生文件（`input_file` / `document`） | 按 P0.1.4 固定常量 |
| 侧车不可用 + `support_pdf = false` | `file_handle_text` 占位 | 按占位文本 |

> **边界声明**：文本抽取不承诺必成功。扫描件与加密 PDF 属预期失败，不是缺陷。

---

## P3 — 验收测试

`assistant-agent` 当前**零单测**（`src/` 下无 `#[cfg(test)]`），而新增模块边界条件最密集。参考 `assistant-providers/src/types/config.rs:176` 的既有写法，直接在模块内加 `#[cfg(test)] mod tests`。

- [ ] P3.1 `request_projection`：超预算卸载图片 / 超预算卸载文件正文 / 超预算截断 `ToolResult` / 不超预算不动 / **`context_limit = None` 打 warn 且不静默返回**（原稿的 `context_limit = 0` 早退用例与 `Option<u32>` 类型不符，已改）
- [ ] P3.2 `request_projection`：最后一条用户消息不被降级；降级顺序为「最旧优先」
- [ ] P3.3 `multimodal::admit_attachments`：不可信通道拒绝附件（V2-10）、`..` 与绝对路径拒绝（V2-11）、伪造 MIME 拒绝
- [ ] P3.4 `multimodal::project_files`：非 PDF → 文本（V2-3）、`support_pdf = false` 时 PDF 也降级（V2-5）、多文件配额分配（4 × 512KB 在 64k 预算下每文件约 16k 且总和不超）、`None` 预算走单文件绝对上限
- [ ] P3.5 `normalization`：长边 2048 缩放、BMP/TIFF → PNG、MIME 与字节不符时拒绝
- [ ] P3.6 `token_estimate`：纯 ASCII / 纯中文 / 混排口径回归；`estimate_image_tokens` 随 tile 数递增、宽高缺失回退 `765`；`ContentBlock::File` 带 PDF `cached_data` 时**不得**随 base64 长度线性增长
- [ ] P3.7 `truncate_to_tokens`：CJK 下不超预算、head / tail 比例正确、围栏闭合、未截断时不打 `[truncated]`

> 构造 `AttachmentStore` 需临时目录（`AttachmentStore::new(tempdir)`）。估算口径的断言与 I29 T6.5 共用，避免两套期望值。

---

## 涉及文件

| 文件 | 变更 |
|------|------|
| `src-tauri/crates/assistant-agent/src/token_estimate.rs` | **新增**：全仓唯一估算模块（P0.1） |
| `src-tauri/crates/assistant-agent/src/request_projection.rs` | 改用共享估算、文件正文 / `ToolResult` 卸载、卸载顺序纠正、`None` 分支 |
| `src-tauri/crates/assistant-agent/src/multimodal.rs` | `project_files` 预算参数、头尾截断、PDF 侧车读取 |
| `src-tauri/crates/assistant-agent/src/chat_request.rs` | **重排管道顺序**、计算输入预算并传入 `project_files` |
| `src-tauri/crates/assistant-agent/src/chat_options.rs` | 抽出 `resolve_max_tokens`，供预算与 `ChatOptions` 共用 |
| `src-tauri/crates/assistant-agent/src/chat_runtime.rs` | `estimate_tokens` 改用共享函数 |
| `src-tauri/crates/assistant-providers/src/openai/message_converter.rs` | 按端点分派的 PDF 分支（`chat.rs` / `responses_stream.rs` 两处调用点同步改） |
| `src-tauri/crates/assistant-agent/Cargo.toml` | PDF 抽取依赖（P2） |

## 明确不做

- HEIC/HEIF、AVIF 解码（理由见 I2g 归档文档）
- PDF 转图片渲染、混合模式
- 分块摘要（引入二次模型调用，成本不匹配收益）
- 流式注入（移交 I18 工具架构）
- docx / xlsx / pptx 等 Office 文档解析：`is_text_file`（`assistant-agent/src/multimodal.rs:569`）只认 13 种文本扩展名，Office 文件当前只能降级为 `file_handle_text`。**这是已接受的边界，不是缺陷**；若后续要补，参考项目 OpenBitFun 的做法是引入 `anydoc` 统一转 Markdown（带内容哈希缓存），应作为独立条目整体引入，不与本清单混做
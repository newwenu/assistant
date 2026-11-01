# I2h — 附件链路收敛（长文本预算 + PDF 降级）

> 来源：I2g 归档时迁出的残留项，见 [I2g-tasklist.md](../achievement3/I2g-tasklist.md) 的「归档时状态」
> 前置：I2g ✅（图片/文件附件主链路、能力感知投影、准入哨兵、规范化、offload）
> 目标：**消除正确性缺口**，而非补齐所有重依赖能力。HEIC/AVIF、PDF 渲染已明确不做（理由见 I2g 归档文档）。

---

## 0. 背景：三个真实缺口

### G1 — 多文件没有合计预算

`project_files` 对每个文件独立处理，单文件硬截断 `MAX_TEXT_FILE_BYTES = 512KB`（`assistant-agent/src/multimodal.rs:15`），但没有总量概念。4 个文件各自 512KB 文本注入后，仅文件块就约 20 万 token，直接超出多数模型的上下文窗口。

### G2 — offload 只覆盖 ImageBlock

`offload_if_over_budget`（`assistant-agent/src/request_projection.rs:11-66`）的循环里，消息若不含 `ContentBlock::Image` 就 `continue`。当超预算的根源是文本/文件块时，所有消息被跳过，函数静默返回，超限请求照样发出——**对文本型溢出完全失效**。

### G3 — OpenAI 转换器忽略 PDF `cached_data`

`openai/message_converter.rs:43-46` 无条件把 `ContentBlock::File` 转文本。`rehydrate_files` 已按 `support_pdf` 填好 base64，转换器又丢弃，等于白做一次磁盘 IO。

影响面不止「GPT 不支持 PDF」：OpenAI 兼容协议下存在支持 PDF 的模型（如 GLM-5V-Turbo 走兼容端点），这类模型 `support_pdf = true`，`cached_data` 会被静默降级为 `[file: name (size)]`。

---

## P0 — 长文本预算治理（最高优先）

### P0.1 统一 token 估算口径

现状存在两套互不相同的估算：

- `assistant-agent/src/chat_runtime.rs:388` — `estimate_tokens`：`(len + 3) / 4 + 4`
- `assistant-agent/src/request_projection.rs:86-94` — 各分支 `text.len() / 4`

两者都用 `len() / 4`（字节数除以 4）。UTF-8 中文 3 字节/字，折算约 0.75 token/字，而实际约 1~1.5 token/字，**中文场景系统性低估约一倍**，预算判断会偏乐观。

- [ ] P0.1.1 抽出共享函数（建议放 `assistant-agent`，两处复用）：

```rust
/// 保守估算：ASCII 按 4 字符/token，非 ASCII 按 1.5 token/字。
pub fn estimate_text_tokens(s: &str) -> usize
```

- [ ] P0.1.2 `chat_runtime::estimate_tokens` 与 `request_projection` 各分支改为调用该函数
- [ ] P0.1.3 `estimate_blocks_tokens` 中 `ContentBlock::File { cached_data }` 分支同理（base64 长度与原始字节不等价，需先按 3/4 折算）

### P0.2 `project_files` 接收预算并分配配额

- [ ] P0.2.1 签名扩展为带预算：

```rust
pub async fn project_files(
    messages: &mut [ChatMessage],
    store: &AttachmentStore,
    support_pdf: bool,
    file_budget_tokens: usize,
)
```

- [ ] P0.2.2 预算来源（`assistant-agent/src/chat_request.rs:36-58` 的管道内计算）：

```
可用 = context_window_tokens
     - 已占用（非文件块的消息 token 估算）
     - 输出预留（建议取 options.max_tokens，缺失时按 25% 兜底）
```

`context_window_tokens` 已在 `build_chat_request` 中解析，无需新增数据源。

- [ ] P0.2.3 配额分配：先按文件数均分，单文件最低配额（建议 2k token）不足时按剩余预算顺序分配；PDF 原样发送的文件不占文本配额
- [ ] P0.2.4 预算 ≤ 0 时全部文件降级为 `file_handle_text`（不注入正文）

### P0.3 头尾截断替代硬截断

现状（`assistant-agent/src/multimodal.rs:592-625`）有两个问题：

1. 判定用 `data.len()`（字节），截断用 `text.chars().take(512*1024)`（字符）——中文场景下截断结果可能仍超过 512KB 字节；
2. 只保留头部，尾部信息（最新内容、结论、diff 末段）全部丢失。

- [ ] P0.3.1 改为按 token 预算截断，头 60% / 尾 40%
- [ ] P0.3.2 截断标注明确，便于模型理解：

```
[file: name — truncated, kept head 1234 + tail 823 tokens of 20456]
```

- [ ] P0.3.3 统一按字符遍历（`chars()`）做预算切分，不再混用字节/字符口径

### P0.4 offload 覆盖文本块

- [ ] P0.4.1 `offload_if_over_budget` 增加文本块卸载：从**最旧的用户消息**开始，把注入的文件正文块替换为占位文本（保留 `file_handle_text`，保留文本消息本身的结构与指令）
- [ ] P0.4.2 卸载顺序：先文件正文块（体积最大）→ 再图片块 → 最后历史用户文本块（最后手段，避免破坏指令）
- [ ] P0.4.3 只卸载 `role == User` 的块，`Assistant` / `Tool` 消息不动（否则会破坏工具调用链）
- [ ] P0.4.4 卸载后仍超预算时不阻断请求，记录 `log::warn!`（由 Provider 报错兜底）

---

## P1 — OpenAI 转换器补 PDF 分支

- [ ] P1.1 `openai/message_converter.rs:43-46` 改为：`cached_data` 非空且文件为 PDF 时，按 OpenAI 兼容的 `input_file` 形式发送；否则降级文本

> 实现前需确认目标端点是否接受 `input_file`（Chat Completions 与 Responses API 的字段不同）。若目标模型不支持，则维持文本降级，但**必须**在日志中说明「PDF 已被丢弃」，避免静默。

- [ ] P1.2 单测：`cached_data` 有值 + `.pdf` → 非文本分支；`cached_data` 为空 → 文本降级；非 PDF 扩展名 → 文本降级

---

## P2 — PDF 文本层抽取

目标：不支持 PDF 的模型也能读到 PDF 内容（文本层可读即可，不要求排版）。

- [ ] P2.1 引入纯 Rust 依赖（`pdf-extract` 或 `lopdf`），不引入 C/C++ 工具链
- [ ] P2.2 **准入时**抽取，而非投影时每轮解析：`admit_attachments` 中 PDF 入库后，按 `attachment_id` 落一份文本侧车（内容寻址存储天然可复用）
- [ ] P2.3 `project_file_block` 读取侧车文本，走与文本文件相同的预算截断路径
- [ ] P2.4 失败必须回退：扫描件（无文本层）、加密 PDF、CJK 字体编码异常 → 回退 `file_handle_text`，不阻断发送
- [ ] P2.5 侧车缺失（历史附件未抽取）时按需补抽一次，仍失败则回退

> **边界声明**：文本抽取不承诺必成功。扫描件与加密 PDF 属预期失败，不是缺陷。

---

## P3 — 验收测试

`assistant-agent` 当前**零单测**（`src/` 下无 `#[cfg(test)]`），而新增模块边界条件最密集。

- [ ] P3.1 `request_projection`：超预算卸载图片 / 超预算卸载文本块 / 不超预算不动 / `context_limit = 0` 早退
- [ ] P3.2 `multimodal::admit_attachments`：不可信通道拒绝附件（V2-10）、`..` 与绝对路径拒绝（V2-11）、伪造 MIME 拒绝
- [ ] P3.3 `multimodal::project_files`：非 PDF → 文本（V2-3）、`support_pdf = false` 时 PDF 也降级（V2-5）、多文件配额分配
- [ ] P3.4 `normalization`：长边 2048 缩放、BMP/TIFF → PNG、MIME 与字节不符时拒绝
- [ ] P3.5 `estimate_text_tokens`：纯 ASCII / 纯中文 / 混排的口径回归

---

## 涉及文件

| 文件 | 变更 |
|------|------|
| `src-tauri/crates/assistant-agent/src/request_projection.rs` | 文本块卸载、共享 token 估算、卸载顺序 |
| `src-tauri/crates/assistant-agent/src/multimodal.rs` | `project_files` 预算参数、头尾截断、PDF 侧车读取 |
| `src-tauri/crates/assistant-agent/src/chat_request.rs` | 预算计算并传入 `project_files` |
| `src-tauri/crates/assistant-agent/src/chat_runtime.rs` | `estimate_tokens` 改用共享函数 |
| `src-tauri/crates/assistant-providers/src/openai/message_converter.rs` | PDF `cached_data` 分支 |
| `src-tauri/crates/assistant-agent/Cargo.toml` | PDF 抽取依赖（P2） |

## 明确不做

- HEIC/HEIF、AVIF 解码（理由见 I2g 归档文档）
- PDF 转图片渲染、混合模式
- 分块摘要（引入二次模型调用，成本不匹配收益）
- 流式注入（移交 I18 工具架构）
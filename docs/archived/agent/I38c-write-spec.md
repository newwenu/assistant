# I38c-write — `write`（修改工具）

| 项 | 值 |
|---|---|
| 状态 | 现行（P5 工具分册） |
| 日期 | 2026-11-06（2026-11-07 修订：新增 `encoding` 参数与转码五道闸，任务项 C9.10–C9.13） |
| 上级 | [I38c-os-tools-spec.md](./I38c-os-tools-spec.md)（工具清单 / 共同结构 / 验收标准；多编码跨工具约束见 §14） |
| 任务 | I38c-os-tools-tasklist（已归档） C3.5–C3.7（编码支持见 C9.10–C9.13） |
| 实现 | `src-tauri/crates/assistant-tools/src/os/agent_write.rs` |
| 注册名 | `write`（旧名 `fs_write`） |
| 相关 | [I38c-read](./I38c-read-spec.md) §3.3（编码检测）、[I38a §4-C6](./I38a-tool-control-layer-spec.md)（回执端口） |

> 共同约束见上级 §4，本文不重复。

## 1 安全标记

| action | read_only | destructive | open_world | audience | 并发 |
|---|---|---|---|---|---|
| `edit` | 否 | 否 | 否 | `BOTH` | 串行（`is_concurrency_safe = false`）：仅保证同轮工具批次不并行，**不提供** per-target 锁（§3） |

`permission_intents` → `edit` + 目标路径。覆盖已有文件属于修改而非破坏，但审批卡片必须展示「覆盖」与目标路径。

## 2 input_schema

```json
{
  "type": "object",
  "properties": {
    "path": { "type": "string" },
    "content": { "type": "string" },
    "append": { "type": "boolean", "description": "Append mode (default false)" },
    "encoding": {
      "type": "string",
      "description": "Optional target encoding (e.g. gbk); omit to keep the file's encoding, new files use UTF-8"
    }
  },
  "required": ["path", "content"]
}
```

## 3 行为

- `append = false` 时覆盖已有文件；`append = true` 时追加。
- 自动创建父目录（等价 `mkdir -p`），不因父目录缺失失败。
- 单次写入上限 10MB，超出返回 `Business`，不截断后静默写入。
- 内容与现有文件**逐字节相同** → 幂等成功，不写盘、不改 mtime（参考 `existing_file_matches_content`）。写盘会刷新修订号，让其他会话的读取回执全部失效，并产生无意义的审计噪声。
- 目标是**已存在但不可读**的文件（权限 / 被占用）→ `Business` 拒绝覆盖：无法确认现有内容，覆盖即不可逆（参考 `write_never_overwrites_an_unreadable_existing_file`）。
- 原子写：同目录写临时文件 → `sync_all()` 刷盘 → rename 覆盖，避免中途失败留下半截文件。`fsync` 是必要的一步：只做 rename 时，崩溃 / 断电后可能出现「文件存在但内容为空」，把「不留半截」退化成「留下空文件」（参考 deepseek-harness-orb `fs-local`「stage → fsync → publish」）。
- **不做 per-target 锁**：同一路径的并发写只由「临时文件 + 原子 rename」保证不产生半截文件，**不保证先后顺序**——后写者会静默覆盖前者。本项目以单会话为主，跨会话冲突改由审批卡片（展示「覆盖」+ 现有行数 / 字节数）与 `read` 回执修订号暴露（[read §3.1](./I38c-read-spec.md)）。引入 per-target 锁需要在 `os/` 层维护跨调用的共享状态（参考实现是 per-target FIFO 锁包住 read→guard→write 整个临界区），P5 不做，理由见 §5。
- 写回成功后按 [I38c-read §3.1](./I38c-read-spec.md) 的回执键写入新修订号，使「先 `write` 新建、再 `edit`」链路不被读前置误拦。

### 3.1 编码（C9）

编码解析顺序：`encoding` 参数 → 已存在文件的回执 `encoding` → `UTF-8`。解析后过**五道闸**，任一不过即 `Business` 且**不写盘**（上级 §14.6）：

| 闸 | 规则 | 违反时 |
|---|---|---|
| G1 | 目标编码必须确定：显式参数或回执继承，二者皆无则 UTF-8 | 不适用（兜底恒可确定） |
| G2 | `encoding_rs::encode(content)` 的 `had_errors == false` | `Business`：内容含该编码无法表示的字符；**不做 `?` 替换式写回** |
| G3 | 转码写入 ≤ **1MB**（`content` + `append` 时的现有内容） | `Business`：只陈述超限事实，不给建议（工具集内无转码手段，上级 §4.2 / §11 OQ7） |
| G4 | 目标**已存在**且要求非 UTF-8 写入时，必须有本会话回执 | `Business`：提示先 `read`，禁止盲转码覆盖 |
| G5 | BOM 按目标编码规范：UTF-16 恒写 BOM；UTF-8 仅在回执 `has_bom` 时写 BOM | 不适用（规则内部确定） |

- **G1 的「回执继承」**：`encoding` 缺省且目标是已存在文件时，用回执里的 `encoding` 与 `has_bom`，等价于「按原编码写回」。这是「模型不必知道文件是什么编码」的关键——`read` 已经发现过，`write` 直接用。
- **UTF-8 路径不受影响**：目标编码解析为 UTF-8 且 `has_bom == false` 时，走原有 `content.as_bytes()` 直写，G2 / G3 / G4 的额外开销为零（`encode` 对 UTF-8 是恒等变换，`had_errors` 恒 false）。
- **`append` 与转码叠加**：按「解码现有内容 → 拼接 → 整体编码」处理，**不**把两段字节直接拼接（编码边界处的字节拼接会产生非法序列，例如 GBK 多字节字符被截断）。现有内容解码有损时（`had_errors == true`）直接 `Business`，不回退到字节拼接。
- **幂等判定改为字节级**：`payload_bytes == existing_bytes` 时仍不写盘、不改 mtime；转码路径下 `payload_bytes` 是编码后的字节，因此「同内容不同编码」不会被误判为幂等。
- **新建文件**：无回执，G4 不适用；`encoding` 缺省写 UTF-8（与现状一致）；显式 `encoding` 生效时按 G2 / G3 / G5 校验。
- **写后回执**：用**实际写出的**编码与 BOM 状态刷新 `encoding` / `has_bom`，供后续 `edit` / `write` 复用。

## 4 错误

| 情况 | 行为 |
|---|---|
| `path` / `content` 缺失或类型错误 | `Business` |
| 路径越界或命中系统目录 | `Business` |
| 目标是目录 | `Business`，只陈述「write 只面向文件」（不提示 `mkdir`：目录已存在，该建议无效） |
| 内容超 10MB | `Business` |
| 目标是已存在但不可读的文件 | `Business`，只陈述「无法确认现有内容故拒绝覆盖」（不提权限 / 占用进程，模型无从处置） |
| `encoding` label 非法 | `Business`，提示 WHATWG Encoding Standard 的规范名 |
| 内容含目标编码无法表示的字符（G2） | `Business`，指明无法编码的字符；**不**以 `?` 替换写回 |
| 转码写入超 1MB（G3） | `Business`，只陈述超限事实（工具集内无转码手段，上级 §4.2） |
| 已存在文件要求非 UTF-8 写入但无回执（G4） | `Business`，提示先 `read` |
| `append` 时现有内容解码有损 | `Business`，提示现有内容编码不一致，先 `read` 确认或整体重写 |

## 5 参考

| 来源 | 做法 | 决定 |
|---|---|---|
| OpenBitFun `assembly/core/src/agentic/tools/implementations/file_write_tool.rs` | 只接受**单个 `payload` 字符串**：首行 `+++ {file_path}` 指定目标，其后全部为内容；缺 `+++` 时把整个 `payload` 写到 `__OPENBITFUN_WRITE_FALLBACK_DIRECTORY__/write_{suffix}.tmp`；明确禁止另传 `path` / `file_path` / `content`。`existing_file_matches_content` 逐字节比对，相同即成功不写盘；`write_never_overwrites_an_unreadable_existing_file` 在现有文件不可读时拒绝覆盖；`edit_constraint_guard::check_write` 强制「先 Read 再写」并留 `force` 逃生口；`LARGE_WRITE_SOFT_LINE_LIMIT = 200` / `LARGE_WRITE_SOFT_BYTE_LIMIT = 20 * 1024` 只做提示、不拦截 | **部分采纳**：幂等成功、不可读拒绝覆盖、父目录自动创建（§3）。**不采纳** 单字符串 `payload` 形态——它把路径塞进字符串，等于在 JSON 之上再加一层解析，且缺 `+++` 时**静默落到 fallback 临时文件**，模型看不到「路径写错了」，只会困惑「文件为什么没出现」；本项目用 schema 原生的 `path` + `content` 两字段，路径非法直接 `Business`。**不采纳**「先 Read 再写」强制 guard 与 `force` 逃生口——`write` 的 `content` 是完整替代，不存在匹配失败，读前置只多一次往返；覆盖风险改由审批卡片承担（展示「覆盖」+ 现有行数 / 字节数，取 `metadata` 即可，不读内容）。**不采纳** 200 行 / 20KB 软限制——只提示不拦截，模型仍会写大文件，只增加提示噪音；改为 10MB 硬上限 |
| deepseek-harness-orb `packages/fs/tool-fs/src/write.ts`、`packages/fs/fs-local/README.md` | 定位为「unconditional atomic create-or-overwrite」：`ctx.fs.writeText` 原子写、写前不 stat；可选 `fs/write-intent` 策略槽返回 `createIfAbsent` / `replaceIfVersion`，把覆盖变成**带版本守卫的条件写**；结果区分 `operation: create / update`，并附 before / after 生成 diff 卡片。落盘流程是「暂存到目标旁的私有 staging 目录 → `fsync` → 原子 publish」，并保留既有文件权限（Windows 侧保留 DACL）；同一目标有 per-target FIFO 锁，把 read→guard→write 串成一个临界区 | **采纳** 原子写、`fsync`、create / update 区分（§3）。**不采纳** 版本守卫覆盖（`replaceIfVersion`）——它要求 backend 持有文件版本，本项目已有 `read` 回执修订号（[read §3.1](./I38c-read-spec.md)），再引入第二套版本语义会让「回执失效」与「写冲突」并存；覆盖风险改由审批卡片承担。**不采纳** per-target 锁——锁要跨调用存活，需在 `os/` 层加共享状态，而 P5 以单会话为主，锁的收益（并发写有序）远低于成本；并发覆盖的可见性已由审批卡片 + 回执修订号提供 |
| goose `crates/goose/src/agents/platform_extensions/developer/edit.rs`（`file_write_with_cwd`） | `is_new = !path.exists()`；父目录缺失则 `create_dir_all`；`fs::write` 直接覆盖，无守卫、非原子；结果文案 `Created` / `Wrote` + 行数 | **部分采纳**：父目录自动创建、create / overwrite 文案区分（§3）。**不采纳** 非原子 `fs::write` 与无守卫覆盖 |
| codex `codex-rs/apply-patch`（`Hunk::AddFile`） | `AddFile` 无条件写入，已存在即覆盖；写前用 `read_optional_file_text_for_delta` 读出旧内容记入 `AppliedPatchFileChange::Add { overwritten_content }`，供 diff 渲染 | **采纳**「覆盖是正常操作、不是错误」的语义（§3）。**不采纳** 写前读旧内容生成 diff——覆盖目标可达 10MB，为审批卡片读全文不划算；卡片只取 `metadata` 的行数 / 字节数。**不采纳** patch 文本形态（`*** Add File:` 段落），本项目 `write` 是独立工具，不做 patch 解析 |
| Claude Code（openhuman `bench/harnesses/prompts/claude-code/tools.json`） | `Write`：「Writes a file to the local filesystem, overwriting if one exists … **Overwriting an existing file you haven't Read will fail**」 | **不采纳** 读前置强制（理由同上：`content` 是完整替代，不存在匹配失败，读前置只多一次往返）；保留其「覆盖已有文件」的基本语义 |
| `append` 参数 | 五个参考实现均只有「创建 / 覆盖」，无追加参数 | **本次新增**：追加是模型高频需求，用 `append` 表达比「先 `read` 再 `write` 全文」便宜一个数量级 |
| 10MB 硬上限 / 写后回执 | 参考无对应约束 | **本次新增** |
| `encoding` 参数与转码五道闸（C9） | helix `references/7-text-encoding/helix-view/src/document.rs`（保存按原编码 `encode`，失败即报错）；ripgrep `crates/searcher/src/searcher/mod.rs`（`Encoding::new` 的 label 校验表） | **采纳**「按原编码写回 + 有损即报错」（§3.1 G2 / G5）。**不采纳** 编辑器内存态编码作为来源——本项目用 `ReadReceipt` 继承（§3.1 G1 / G4），避免第二个编码发现点（上级 §14.2） |
# I58 — P17：超级工具 任务清单

> 规格：[I58-super-tool-spec.md](./I58-super-tool-spec.md)（现状 §1 / 契约 §3 / 生命周期 §4 / 失败矩阵 §5 / 文案 §6 / 与 A 的接口 §7 / 验收 §8 / 已知限制 §9 / 门禁 §10 / 切片 §11 / 未决问题 §13）
> 事实基线：I58 §1 的 N1–N12（2026-11-18 已按代码逐条核对，见下方「前置核对」）
> 前置：P2（工具控制层）、P4（`TurnObserver` / `TurnRunCtx`）、P5（OS 工具能力）、P13（落盘与预览口径）均已完成；**T2–T6 无 P16 实现依赖**——I58 自写扫描 / 渲染 / 索引，只对齐 A 的格式字面量与回执字段（规格 §0.3），T1 亦然
> 阶段台账：[I38-agent-roadmap.md](../../store/I38-agent-roadmap.md) §2 阶段表（唯一状态来源）

批次对应规格章节：**T1 能力发现**（§3.1 / §10 G1–G5）→ **T2 索引核心**（§3.5 / §4 / §10 G6·G7·G11）→ **T3 `super_read`**（§3.2 / §3.4 / §10 G8·G12）→ **T4 生命周期**（§4 / §10 G9·G10·G13）→ **T5 `super_edit`**（§3.3 / §7.3 / §10 G14）→ **T6 收口**（§5 / §6 / §10 G15）→ **T7 准入语义 + 伴随展示**（§3.6 / §10 G17·G19·G20）→ **T8 升级通道**（§3.6 / §10 G4·G21·G22·G23）。

依赖：T1 无前置（**已完成**）；T2 无前置（自写扫描器与行起点记录，只对齐行数判据）；T3 依赖 T2；T4 依赖 T3；T5 依赖 T4；T6 依赖 T5；T7 依赖 T6；T8 依赖 T7（**顺序不可颠倒**：T7 拿掉「伴随 = 注入」之后，`request_tool` 才成为唯一申请入口）。
不改面：**T1–T6 不改 wire / DTO / 前端**；T7 只改前端**渲染层**（无新字段、不写 `enabledTools` / `disabledTools`）；T8 后端零前端改动（§10 G15）。

> 本清单不管理提交。提交粒度按代码依赖与可编译性现场决定；这里只回答「代码改没改、验收跑没跑」。
> 勾选表示代码已改且对应验收已跑。

---

## 前置核对（2026-11-18，按代码）

| 事实 | 结论 | 落点 |
|---|---|---|
| N1：`Tool` trait 无任何「手册 / 用法」入口 | T1 已加 `fn manual() -> Option<&'static str>` 默认实现；**全 crate 仅此一处定义**（无工具覆写，故目录手册全部回退 `description()`） | `src-tauri/crates/assistant-tools/src/contract/trait.rs` |
| N2：`TurnRunCtx.tools: Option<&'a [ToolSpec]>` | 生产者已就绪，T1 只消费 | `src-tauri/crates/assistant-agent/src/agent/round_executor.rs` |
| N3：`ToolExecCtx` 无 `tools`，构造点唯一 | T1 已加字段并在唯一构造点填 `ctx.tools` | `round_executor.rs`、`loop_executor/execute.rs` |
| N4：`ToolSpec` 无 `id`，只有 `name` | 透传按名字，`data.listed` 与 `ToolSpec.name` 同源 | `assistant-providers/src/types/tool.rs` |
| N5：`resolve_available_tools` 是唯一生产者 | 不改；它产出的名字即 `available_tool_names` | `src-tauri/src/adapters/tool_resolver.rs` |
| N6：`ToolUseContext` 无「本轮可用工具名」 | T1 已加 `available_tool_names` 与 `tool_catalog`，均 `#[serde(skip)]`，`Default` 为 `None` | `contract/context.rs` |
| N7：`ToolRegistry` 有 `tool_definitions_for_tier`（按 `tier <= max && audience.agent` 过滤） | 目录只按 `audience.agent` 过滤、**不按 tier**；排序按 `id` 字典序（与 `definitions_are_sorted_by_id` 同序） | `runtime/registry.rs` |
| N8：`AppDirs` 无索引目录 | T2 新增 `indexes()`；`dir_name.rs` 新增 `TOOL_INDEX` | `src-tauri/src/app_dirs/{roots.rs,dir_name.rs}` |
| **补充事实**：`assistant-tools` 是库 crate，**不解析 `AppDirs`** | 索引根必须由调用方注入（`ToolUseContext.index_root`，`#[serde(skip)]`），与 `tool_output_root` 同模式；已回写 §7.2 | `contract/context.rs`、`adapters/tool_exec.rs` |
| **补充事实**：workspace **无 blake3**，已有 `sha2 = "0.10"` | 索引文件名用 `sha2(绝对路径)`，不新增依赖；已回写 §3.5 | `src-tauri/Cargo.toml` |
| N9：`read` 线性扫描、无 `seek`；`revision = len:nanos` | `file_revision` 是公开函数，`super_read` 直接复用（同一 revision 口径才能让回执与索引的**粗筛**一致）。索引正确性不靠它：`len:nanos` 识别不了等长同片改写，由行起点自证承担（§4 / §9 L11） | `agent_read/mod.rs::file_revision` |
| N10：`edit` 整文件读（`MAX_WHOLE_FILE_BYTES = 16 MiB`）、子串匹配、原子写 | `super_edit` 可复用 `apply.rs` 的读 + 原子写（也可自写），只换定位方式；闸门与原子性不改 | `agent_edit/{mod.rs,apply.rs}` |
| N11：`ReadReceiptStore` 有 `record` / `lookup` / `invalidate_path` | **回执**失效仍用 `invalidate_path`，不新增 trait 方法；**索引**不再由写侧通知失效（§12 R10），改由行起点自证 | `runtime/storage.rs` |
| N12：注册分组签名均为 `&ToolRegistry` | T1 已把 `request_tool` 注册进 `register_readonly_tools`；T6 只补 `super_*` 组 | `os/mod.rs`、`src-tauri/src/tool_setup.rs` |
| `FOOTER_TAGS = ["encoding", "non-utf8", "catalog"]` | 已收敛为单一来源 `os/shared/footer.rs`（`TAGS` + 常量 + `render()` / `trailing_marker()`）；`agent_edit/report.rs` 不再持有本地白名单 | `os/shared/footer.rs`、`agent_edit/report.rs` |
| 只读组 id 列表 `["fs_link", "glob", "grep", "ls", "read", "request_tool"]` | T1 已更新；后续切片不得回改本行 | `tests/registration.rs` |

---

## T1 — 能力发现（§3.1 / §10 G1–G5）· 已完成

退出条件：G1–G5 通过；`request_tool` 只读、不改变本轮工具集；无本轮事实时回退注册表并显式标注。

- [x] `contract/trait.rs`：`Tool::manual()` 默认实现返回 `None`，注释写明「缺省回退 `description()`，覆写时只写怎么调 / 代价 / 边界」（G1）
- [x] `contract/catalog.rs`（新增）：`CatalogEntry { id, name, manual }` + `ToolCatalog` trait（**只有 `entries()` 一个读入口**，无写方法；工具不持有注册表，避免 `Arc<ToolRegistry>` → `Arc<dyn Tool>` 成环）
- [x] `contract/context.rs`：`available_tool_names: Option<Vec<String>>`、`tool_catalog: Option<Arc<dyn ToolCatalog>>`，均 `#[serde(skip)]`，`Default` 与 `ToolUseContext::new` 都填 `None`
- [x] `runtime/registry.rs`：`impl ToolCatalog for ToolRegistry`——只列 `audience.agent`、按 `id` 字典序；手册取 `manual()`，缺省回退 `description()`
- [x] `runtime/pipeline.rs`：每次调用前注入 `tool_catalog = registry`（调用方已注入时**不覆盖**，测试可替换成受控目录）
- [x] `assistant-agent/round_executor.rs`：`ToolExecCtx` 增 `tools: Option<&'a [ToolSpec]>`，注释指向 G2
- [x] `loop_executor/execute.rs`：唯一构造点填 `tools: ctx.tools`（原样透传，不重新解析）
- [x] `src-tauri/src/adapters/tool_exec.rs`：`available_tool_names = ctx.tools.map(|t| t.iter().map(|s| s.name.clone()).collect())`；`None` 表示无本轮事实
- [x] `os/tools/agent_request_tool.rs`（新增）：`MANUAL_MAX_CHARS = 2000`、`MANUAL_TOTAL_CHARS = 8000`、`MANUAL_TRUNCATED_MARKER = "\n(manual truncated)"`（`catalog` 形态改由 `footer::render(footer::CATALOG, …)` 生成）
- [x] `os/tools/agent_request_tool.rs`：`detail`（`list` / `manual`）+ `query`（大小写不敏感子串，匹配 `id` 或 `name`）；`data = { available, listed, manuals, source }`
- [x] `os/tools/agent_request_tool.rs`：`source = "turn"` 优先、`"registry"` 回退且文案标注回退；总量超限追加 `[catalog: showing {k} of {n}]`（**方括号形态**，OQ4 已定稿）
- [x] `os/mod.rs`：注册进 `register_readonly_tools`（`request_tool` 无审批、`read_only: true`、`audience: BOTH`）
- [x] `tests/registration.rs`：只读组 id 列表追加 `request_tool`；`tool_definitions_for_os_tier_expose_every_os_facing_tool` 追加该名
- [x] `tests/request_tool.rs`（新增）：10 用例覆盖 G1（默认手册 / 目录排序 / `fs_link` 被排除）、G2（清单跟随本轮工具集）、G3（回退标注）、G4（调用前后目录与工具集不变）、G5（单份与总量上限）、空目录错误、非法 `detail`、Pipeline 注入目录
- [x] `os/shared/footer.rs`（新增）：footer 唯一登记表——tag 常量 + `TAGS` + `render()` / `trailing_marker()`；单测 `every_registered_tag_round_trips` / `tags_are_unique` / `unregistered_tags_are_not_footers`（G16）
- [x] `agent_edit/report.rs`：删除本地 `FOOTER_TAGS` 与 `trailing_footer_marker`，改调 `footer::trailing_marker()`；单测改为遍历 `footer::TAGS` 验证每个 tag 都走到诊断文案（G16）

验收：`cargo test -p assistant-tools --test request_tool` 10/10 通过；`cargo test -p assistant-tools --lib` 227/227 通过（含新增 footer 单测）。

---

## T2 — 索引核心（§3.5 / §4 / §10 G6·G7·G11）· 已完成

退出条件：G6 / G7 / G11 通过；索引格式、建、读切片、校验四件事各有唯一实现。

- [x] `src-tauri/src/app_dirs/dir_name.rs`：新增 `pub const TOOL_INDEX: &str = "tool-index"`
- [x] `src-tauri/src/app_dirs/roots.rs`：新增 `pub fn indexes(&self) -> PathBuf`，与 `tool_results()` 同根（`data`）
- [x] `contract/context.rs`：`ToolUseContext` 增 `index_root: Option<PathBuf>`（`#[serde(skip)]`，`Default` 为 `None`；`None` 时 `super_read` 报业务错误，不静默降级为线性扫描）
- [x] `src-tauri/src/adapters/tool_exec.rs`：注入 `index_root = Some(app_dirs.indexes())`（`AppDirs` 经现有装配链传入，不在此处重新 `resolve()`）
- [x] `os/tools/super_read/index.rs`（新增）：格式常量 `MAGIC = b"AIDX1"`、header（`lineCount: u64` / `fileBytes: u64` / `revisionLen: u32` + `revision`）、`offsets: lineCount + 1` 个 `u64`
- [x] `super_read/index.rs`：`offsets[0]` = 剥离 BOM 后的首行内容起始；`offsets[i]` = 第 `i+1` 行内容起始；末项 = 文件总字节；行数定义与 A 一致（`\n` 数 + 末尾残留非空 +1；BOM-only = 0）
- [x] `super_read/index.rs`：`build(path, revision, encoding)` 自建扫描器（B 自持，**不复用** P16 的 `LineSink` / `Scanner`），按原始字节找行尾、不解码整行；行数与切行判据对齐 A（`\n` 计数 + 末尾残留 +1、BOM-only = 0）
- [x] `super_read/index.rs`：`LoadedIndex::offsets_range(first, count)` 只读所需区间的起点（含哨兵），逐行 `seek` 由调用方做，内存与窗口大小相关、与文件大小无关
- [x] `super_read/index.rs`：`load(index_root, source, revision, fileBytes)` 校验 magic / `fileBytes` / `revision` / 文件长度，不符返回 `None` 由调用方重建（`store` 总是覆盖，残留坏索引即被替换）
- [x] `super_read/index.rs`：原子落盘（临时文件 + rename），文件名 `{sha2(绝对路径)}_{revision}.idx`（`:` → `_`），**不新增 blake3 依赖**
- [x] `super_read/index.rs`：单文件闸门 `MAX_INDEX_FILE_BYTES = 64 MiB`——预计超出即 F1（不建、不写、不缓存），文案含实测行数与字节
- [x] `super_read/index.rs`：索引目录读写**不经** `path::resolve_readable`；工具入参 `path` 仍必须过它
- [x] `tests/super_read.rs`：G6——正文与 `read` 逐字节一致（`matches_read_byte_for_byte` / `matches_read_on_cut_long_lines` / `bom_file_byte_column_one_starts_at_content`）；CRLF / 末尾无换行 / BOM / GBK 四种文本的切行等价由 `index.rs` / `render.rs` 单测断言（`offsets_match_a_byte_wise_line_split` / `a_final_line_without_a_newline_still_gets_an_offset` / `a_bom_only_file_has_no_lines_and_skips_the_bom` / `gbk_content_is_split_on_the_single_byte_newline`）
- [x] `tests/super_read.rs`：G7——`totalLines` 等于 `wc -l` 语义值（`total_lines_is_exact_where_read_gives_null`；末尾无换行 +1、BOM-only = 0 见 `index.rs::the_index_line_count_matches_reads_total_lines`）
- [x] `tests/super_read.rs`：G11——超 `MAX_INDEX_FILE_BYTES` 的文件返回 F1 业务错误（`an_index_over_the_capacity_cap_is_f1`：8 MiB 纯换行文件 → 约 839 万行 → 起点表 `8 × (lines + 1)` 自然超过 64 MiB，**不必真造 64 MiB 索引**、也**不必注入小阈值**；闸门边界另有 `format.rs::the_capacity_gate_is_measured_in_bytes`）

> 未决问题 OQ1（`MAX_INDEX_FILE_BYTES` 与缓存条数按实测校准）在本切片落地前定稿；定稿后同步 §4 与本清单 G11 判据。
>
> **后续变更**：本切片的 `super_read/index.rs` 已在 T5 搬移为 `super_index/format.rs`（T5 修订 2），条目按当时的文件形态记录。

---

## T3 — `super_read`（§3.2 / §3.4 / §3.6 / §10 G8·G12·G16·G17）· 已完成

退出条件：G8 / G12 / G16 / G17 通过；`super_read` 与 `read` 在同一窗口参数下正文逐字节一致（footer 除外）。

- [x] `os/tools/super_read/mod.rs`（新增）：`id = "super_read"`、`tier = Os`、`audience = BOTH`、`read_only: true`；审批键与 `read` 同构（工作区 → `read`，输出域 → `readToolOutput`）
- [x] `super_read/mod.rs`：schema = `read` 的四项（`path` / `offset` / `limit` / `encoding`）+ 三项（`unit` / `column` / `maxLineChars`），`required` 仍只有 `path`
- [x] `super_read/mod.rs`：`unit = "char"`（默认）与 `"byte"` 两种口径；`byte` 列的基准是**剥离 BOM 后**的行内容起始
- [x] `super_read/mod.rs`：`unit = "char" && column > MAX_COLUMN_CHARS = 200_000` → F6（提示改用 `unit = "byte"`）；`byte` 列不落字符边界 → F5（给出最近边界列）
- [x] `super_read/mod.rs`：渲染由 `super_read/render.rs` 自写（**不复用** A 的 `LineSink`），但行格式**字面量**必须与 A 一致：`LINE_NUMBER_WIDTH` / `LINE_NUMBER_SEPARATOR` / `LINE_TRUNCATED_MARKER` 可直接引用 A 的常量，渲染逻辑自持
- [x] `super_read/mod.rs`：footer 顺序 `[long-lines]` → `[column]` → `[index]` → `[encoding]`（`index` 插在 `[encoding]` 之前，`read` 自身顺序不变）；全部经 `footer::render()` 生成（G16）
- [x] `super_read/mod.rs`：`[index: built in {ms} ms · {lines} lines · {bytes} bytes]` / `[index: reused]` 两种形态
- [x] `super_read/mod.rs`：`data` = A 的八项 + `index { state, buildMs, lines, bytes, revision }`；`totalLines` **恒非 null**
- [x] `super_read/mod.rs`：回执写同一份 `ReadReceipt`（`truncated_lines` / `line_window_max_chars` 同 A 口径），**不新增字段**
- [x] `os/mod.rs`：`register_readonly_tools` 注册 `super_read`
- [x] `contract/trait.rs`：新增 `Tool::companion_of()`（默认 `None`）；`super_read` 返回 `Some("read")`（G17）
- [x] `contract/definition.rs` + `runtime/registry.rs`：`ToolDescriptor.companion_of` 透传；新增 `expand_enabled_tools()` 做白名单加法（G17）
- [x] `src-tauri/src/adapters/tool_resolver.rs`：过滤前先展开伴随工具；层级上限与 `disabled_tools` 对伴随工具同样生效（G17）
- [x] `agent_read/mod.rs`：删除描述里的反向引导；新增 `capability_hint()`——只在「有行被切 / `totalLines` 为 `null`」**且** `available_tool_names` 含 `super_read` 时输出 `[hint: …]`（G17）
- [x] `super_read/mod.rs`：描述收敛为「是什么」（建索引成本、`totalLines` 恒精确、`column` 可进内层），参数范围与用法细节移入 `manual()`
- [x] `tests/super_read.rs`：G8——同一 ASCII 行 `unit=char` 与 `unit=byte` 的 `column` 等价；BOM 文件首列字节列 = 内容起始
- [x] `tests/super_read.rs`：G12——与 `read` 同窗口参数下正文逐字节一致（footer 除外），覆盖超长行与 `column > 1`
- [x] `tests/super_read.rs`：伴随工具门控——`read` 的 `[hint]` 只在超级工具本轮可用且确实表达不了时出现
- [x] `tests/super_read.rs`：F5 一条用例（`byte` 列不在字符边界，见 `render.rs::a_byte_column_inside_a_multibyte_char_is_f5`）
- [x] `tests/super_read.rs`：F6 一条用例（`char` 列超 20 万，见 `a_char_column_beyond_the_cap_is_f6`）

> 未决问题 OQ2（`MAX_COLUMN_CHARS = 200_000` 是否按机器分档）在本切片落地前定稿。

---

## T4 — 生命周期（§4 / §10 G9·G10·G13）· 已完成

退出条件：G9 / G10 / G13 通过；单飞只建一次、行起点自证可用、回执同源。

> **2026-11-18 设计修订（§4 / §9 L11·L12 / §12 R10–R12）**：正确性判据由「写侧主动通知失效」改为「行起点自证」。理由：通知只覆盖经过本工具集的写入，对编辑器保存 / `git checkout` / 构建产物覆盖无效；而我们的写工具走 temp+rename 必然换 mtime，`revision` 自己就能发现。索引只存行起点、不存内容，所以「内容变了」本不需要失效，唯一会失效的是换行位置——这一点可在使用时零成本自证。故 `invalidate_path` 降级为纯缓存卫生，A 的四个写工具不再调用它。

- [x] `super_read/mod.rs`：调用前 `file_revision`（复用 `agent_read::file_revision`）作**粗筛**，键不同即重建；旧条目在 LRU 淘汰时删除
- [x] `super_read/mod.rs`：单飞 `Mutex<HashMap<CacheKey, Arc<Flight>>>` + `Condvar`（`CacheKey = (indexRoot, path, revision)`）——同键并发只建一次，其余等待后取同一结果；leader 失败时错误转给等待者，不重复扫描
- [x] `super_read/mod.rs`：内存缓存 `INDEX_CACHE_BYTES = 64 MiB`（按 `(lineCount + 1) * 8` 计），按字节 LRU 淘汰并连带删除被淘汰项的落盘文件
- [x] `super_read/index.rs`：`LoadedIndex::all_offsets()` 整表读入缓存（落盘仍是按窗口切片读）；`index_file()` 供淘汰时定位落盘文件
- [x] `super_read/mod.rs`：`pub fn invalidate_path(path)`——组件级前缀匹配（`os::path::is_component_prefix`），移除该路径及子树的缓存项并删除落盘文件
- [x] `super_read/mod.rs`：编码回退到 UTF-16 时按新编码重建索引并整体替换缓存项（否则下次调用拿到旧切法的起点表）
- [x] `tests/super_read.rs`：G9——同 `(path, revision)` 并发 8 次，断言 `data.index.state == "built"` 的次数恰为 1（`concurrent_calls_build_the_index_once`）
- [x] `tests/super_read.rs`：G13——回执字段与 `read` 完全同构（`revision` / `encoding` / `has_bom` / `truncated_lines` / `line_window_max_chars`）；`truncated_lines > 0` 时 `edit` 同样 fail-closed（`the_receipt_is_isomorphic_and_gates_edit`）
- [x] `super_read/render.rs`：**行起点自证**——`read_line(file, start, end, LinePosition, path, pattern)` 把起点前移一个行尾符长度再读，断言前缀等于行尾符、正文不含行尾符、非末行必以行尾符结尾；任一不成立 → `RenderError::StaleIndex { line }`
- [x] `super_read/mod.rs`：`prepare` 把 `StaleIndex` 与 `Utf8Fallback` 并列处理——重建索引（`rebuild_offsets`，按当前检测到的编码）后重试一次；仍失败 → F4 文案（两种自愈各限一次，`state` 报 `built`）
- [x] `agent_edit/mod.rs` / `agent_write/mod.rs`：**移除** `crate::os::super_read::invalidate_path(&resolved)` 调用与相关注释（A 不再感知 B，§12 R10）
- [x] `agent_delete/mod.rs` / `agent_move.rs`：**移除**索引失效调用（回执的 `receipts.invalidate_path` 保留——那是 A 自己的机制）
- [x] `tests/super_read.rs`：G10 重写——构造**等长改写**（同字节数、换行位置移动），并把 mtime 显式设回原值使 `revision` 不变；断言下一次 `super_read` 经自证重建、`data.index.state == "built"` 且正文正确（`an_equal_length_rewrite_with_a_restored_mtime_rebuilds_via_self_proof`）
- [x] `tests/super_read.rs`：自证的反向用例——内容等长改写但**换行位置不变**时不得误重建（断言 `state == "reused"`，正文仍是新内容），证明自证不是「每次都重建」（`a_rewrite_that_keeps_the_line_breaks_is_not_a_stale_index`）
- [x] `tests/super_read.rs`：A 侧 `edit` 不再通知索引时，下一次 `super_read` 仍返回编辑后正文（`an_edit_is_picked_up_without_any_write_side_notification`）

验收：`cargo test -p assistant-tools --test super_read` 全绿（含新增自证两条）；`cargo test -p assistant-tools` 全绿；`cargo clippy -p assistant-tools --all-targets` 无警告；`cargo check --workspace --all-targets` 通过；`cargo fmt --check` 无 diff。

---

## T5 — `super_edit`（§3.3 / §7.3 / §10 G14·G18）· 已完成

退出条件：G14 / G18 通过；F7 / F8 / F9 / F10 / F11 / F12 各有用例；三处登记点同步。

> **2026-11-18 设计修订（§3.3 / §9 L13·L14 / §12 R13）**：写入形态由「沿用 `agent_edit/apply.rs` 的整文件读 + 整篇重编码写回」改为「索引驱动的字节区间 + 两端自证 + 三段流式写」。理由：①`super_read` 能读的文件 `super_edit` 必须能改，而整文件闸门 `MAX_WHOLE_FILE_BYTES = 16 MiB`、转码闸门 `MAX_TRANSCODE_BYTES = 1 MiB` 会让「2 MiB 的 GBK 文件里改 5 个字符」被拒；②整篇归一化行尾会让区间外字节漂移，编辑本应只动目标区间。改后内存 O(区间)，闸门改按区间计（`MAX_EDIT_RANGE_BYTES = 16 MiB`）。
>
> **2026-11-18 设计修订 2（§7.2 / §10 G18 / §12 R14）**：索引层抽为 B 私有公共层 `super_index/{format.rs,cache.rs,proof.rs}`，`super_read` 与 `super_edit` 只做消费。理由：①两端自证是「区间正确」的唯一判据，复制两份会让读写对同一文件给出不同结论；②`super_edit` 首次调用必须复用 `super_read` 的缓存与单飞，各持一份等于同文件两套缓存与两份落盘索引；③逐符号 `pub(crate)` 会让「编辑依赖读取模块」。本切片**先做搬移**（`super_read` 行为零变化、既有 14 条测试全绿），再落 `super_edit`。

- [x] **索引层搬移（先行，行为零变化）**：`os/tools/super_index/{mod.rs,format.rs,cache.rs,proof.rs}`（新增）——`format.rs` 承接原 `super_read/index.rs`（格式 / `build` / `load` / `store` / `line_break` / `detect`），`cache.rs` 承接缓存 / `Flight` 单飞 / `index_for` / `build_offsets` / `rebuild_offsets` / `invalidate_path` / `IndexState`，`proof.rs` 承接 `read_line` / `decode_line` / `byte_column_to_char_start` / `LinePosition` / `IndexError`（原 `RenderError`，由 `render.rs` 别名沿用）
- [x] `super_read/mod.rs` / `super_read/render.rs`：改为消费 `super_index`，删除本地副本；`super_read::invalidate_path` 保留为转调（对外签名不变）；`cargo test -p assistant-tools --test super_read` 全绿后再继续
- [x] G18 结构性判据：以源码搜索断言代替「删文件编译」——`tests/super_edit.rs::the_line_proof_has_exactly_one_implementation` 断言 `read_line` / `decode_line` / `byte_column_to_char_start` / `char_index_to_byte` 与 `StaleIndex` 构造在 `src/os/tools` 下**只命中 `super_index/proof.rs` 一处**
- [x] `os/tools/super_edit/mod.rs`（新增）：`id = "super_edit"`、`tier = Os`、`audience = BOTH`、`read_only: false`、`destructive: true`（审批走 `edit` 既有键）、`companion_of = Some("edit")`
- [x] `super_edit/mod.rs`：schema = `path` / `startLine` / `startColumn` / `endLine` / `endColumn` / `toEndOfLine` / `unit` / `expected` / `replacement`
- [x] `super_edit/mod.rs`：`expected` 必填规则——区间非空必须给且逐字节相等；纯插入可省略（F7 / F8）
- [x] `super_edit/mod.rs`：**不查读取回执**（无子串匹配，盲写风险由 `expected` + `revision` 判据封死）；唯一硬前提是 `index_root` 存在，缺失 fail-closed
- [x] `super_edit/mod.rs`：F9——`toEndOfLine` 与 `endColumn` 互斥、`endLine < startLine`、列越界，各为业务错误
- [x] `super_edit/locate.rs`：定位**只按行地址**（索引 `offsets` + 列推进），**不做**子串匹配（§9 L3）；产出字节区间 `[start, end)`，列推进只解码该行且受 `MAX_COLUMN_CHARS = 200_000` 上界约束
- [x] `super_edit/locate.rs`：**两端自证**——直接复用 `super_index::proof`（`start` 非首行时前移一个 `pattern.len()` 断言前缀等于行尾符；`end` 必须后跟行尾符或等于文件总字节）；不等 → `StaleIndex` → 重建索引后重试一次（与 `super_read` 同一判据，§4）
- [x] `super_edit/mod.rs`：F11——`end - start > MAX_EDIT_RANGE_BYTES = 16 MiB` → 业务错误，给出实测区间字节数，提示拆分（**不**提示改用 `edit`）
- [x] `super_edit/apply.rs`（自写，**不**复用 `agent_edit/apply.rs` 的整文件读）：`head[0..start)` + `replacement` + `tail[end..EOF)` 三段顺序写；temp + `sync_all` + rename 原子性不变（Windows 上须在 `persist` 前关掉源句柄，否则 rename 报 access denied）
- [x] `super_edit/apply.rs`：`replacement` 编码只作用于该段（`\n` 按 `super_index::format::line_break(encoding).pattern()` 展开）；转码超 `MAX_TRANSCODE_BYTES = 1 MiB` → F12
- [x] `super_edit/mod.rs`：`expected` 与区间文本比较前双方都做 `\r\n` → `\n` 归一（与 `edit` 同口径）
- [x] `super_edit/mod.rs`：写成功后写回执（`truncated_lines = 0`；`line_window_max_chars` 沿用既有回执值、无回执用 `DEFAULT_LINE_WINDOW_MAX_CHARS`）并调 `super_index::cache::invalidate_path`（B→B 缓存卫生；正确性由行起点自证兜底，不依赖它）
- [x] `super_edit/mod.rs`：F10——读取后到写入前 `revision` 漂移 → 业务错误，提示重新 `super_read`
- [x] `os/mod.rs`：`register_write_tools` 注册 `super_edit`；`tests/registration.rs` 同步写组列表与元数据（`destructive = true`）
- [x] **回写登记点**：本工具**未引入新 tag**（只复用 `INDEX` 与 `ENCODING`），`os/shared/footer.rs` 无需新增；footer 形态仍由 `footer::render()` 单一来源产出
- [x] `tests/super_edit.rs`（新增，15 条）：F7 / F8 / F9 / F5 / F11 各一条；成功路径**区间外字节逐字节不变**（BOM 文件、混用行尾文件）、`invalidate_path` 生效（等长改写 + mtime 复原后仍 `built`）、回执同构、首次调用自建索引；大文件小改（20 MiB 文件改首行）不被闸门拒绝
- [x] `super_edit/apply.rs` 单测：F10 修订号漂移、F12 转码闸门、CRLF 展开与 `bytes_written` 口径

验收：`cargo test -p assistant-tools --test super_edit` 15 条全绿；`cargo test -p assistant-tools` 全绿；`cargo clippy -p assistant-tools --all-targets` 无警告；`cargo check --workspace --all-targets` 通过；`cargo fmt --check` 无 diff。

> 未决问题 OQ3（`super_edit` 是否随第一版交付）在本切片开始前定稿；若推迟，T6 的「失败矩阵全用例」须相应缩小范围。

---

## T6 — 收口（§5 / §6 / §10 G15）· 已完成

退出条件：G15 通过；三工具文案齐备、失败矩阵每条有对应用例、注册组无遗漏。

- [x] 三工具 `description` 按 §6 逐条落地：`request_tool` 写明「只返回清单与手册、不改变本轮工具集、`source` 可能是回退清单」（`agent_request_tool.rs::description`）；`super_read` 写明「首次建索引成本、`totalLines` 恒精确、`column` 可进内层」（`super_read/mod.rs::description`）；`super_edit` 写明「按行列地址定位、无需唯一锚点」（`super_edit/mod.rs::description`）。参数细节（`unit` 两口径 / `column` 上限 / `maxLineChars` 范围）留在 schema 与 `manual()`。**口径变更**：原文「不做子串匹配」在 T7 改为正向表述（§6 措辞修正），回归项改为断言两档都不含 `substring`
- [x] 三工具文案都**不得**承诺「替代 `read` / `edit`」；默认路径仍是零成本的 `read` / `edit`——`super_read` 只写「输出格式与 `read` 一致、结果可互换」，`brief` 统一为「`read` / `edit` 表达不了时才申请」；`request_tool` 明说「列举与手册从不把工具加进本轮」
- [x] 失败矩阵逐条映射（**按判据所在层断言**，不硬凑公开入口）：F1 `super_read.rs::an_index_over_the_capacity_cap_is_f1`；F2 `format.rs::binary_files_are_rejected` + 公开入口 `super_read.rs::a_binary_file_is_f2`；F3 `super_read.rs::a_path_outside_the_workspace_is_f3`（与 `read` 共用 `resolve_readable`）；F4 `proof.rs::the_stale_message_names_the_line_and_the_cause` + 重建一次的成功路径 `super_read.rs::an_equal_length_rewrite_with_a_restored_mtime_rebuilds_via_self_proof`（「重建后仍失败」是竞态，公开入口无法稳定触发，故文案在判据层钉住）；F5 `render.rs::a_byte_column_inside_a_multibyte_char_is_f5` / `super_edit.rs::a_byte_column_inside_a_character_is_refused`；F6 `super_read.rs::a_char_column_beyond_the_cap_is_f6`；F7 / F8 / F9 / F11 见 `super_edit.rs` 五条；F10 / F12 是「定位与写回之间的竞态 / 转码闸门」，由判据层 `apply.rs` 单测钉住
- [x] `src-tauri/src/tool_setup.rs`：`super_read` 进只读组、`super_edit` 进写组（注册在 `os/mod.rs::register_readonly_tools` / `register_write_tools`，`tool_setup.rs` 只调这两个函数）；索引根注入 `.with_index_root(dirs.indexes())`（`tool_setup.rs` 装配链）
- [x] 门禁复核 G1–G17 无回退：G1–G5 `tests/request_tool.rs`；G6 / G7 / G11 见 T2 条目；G8 / G12 / G13 `tests/super_read.rs`；G9 / G10 同上（含自证两条）；G14 / G18 `tests/super_edit.rs`；G15 实测 `npm run contract:gen` 后 `src/types/protocol.contract.ts` 与 `vault.contract.ts` **零 diff**（本阶段未改 wire / DTO）；G16 `footer.rs` 单测（每项可识别、无重复）+ 两个超级工具源码内**无手写 footer 字面量**（全部走 `footer::render`）；G17 `tests/tool_resolver.rs` 身份过滤两条

> 未决问题 OQ5（`query` 是否支持 `tier` / 分组过滤）在本切片前定稿；若采纳，须同步 §3.1 与本清单 T1 的 schema 条目（T1 已交付版本为纯子串匹配）。

---

## T7 — 准入语义 + 伴随展示（§3.6 / §10 G17·G19·G20）· 已完成

退出条件：G17 / G19 / G20 通过；`callable` 只剩「模型申请」一个来源；配置里残留的伴随 id 不构成第二个事实来源，也不静默消失。

> **本切片取代 T6 两条 `description` 条目的口径**：T6 的「三工具 `description` 按 §6 逐条落地」「文案不得承诺替代 `read` / `edit`」写于「一段文本同时服务申请前与调用时」的旧结构；T7 拆成 `description()`（`full` 档）+ `brief()`（`brief` 档）后，原条目按当时的文件形态保留记录，判据以 G19 为准。
>
> **判据是身份而不是关系**：`exclude_companions` 只问「这个 id 是不是伴随工具」，不问「基工具在不在名单里」。因此 `enabledTools` 含 `super_read`（无论 `read` 在不在）都不进候选集，`disabledTools` 里的伴随 id 既不阻断也不生效，不存在「禁用基工具而伴随残留」的过滤顺序缺陷（§12 R19）。`request_tool` 的 `companion_of = None`，天然落在身份过滤之外，走普通条目路径——「需勾选」不需要特殊分支。

- [x] `runtime/registry.rs`：**删除** `expand_enabled_tools`（白名单加法），改为 `exclude_companions`（身份过滤，`companion_of != None` 一律剔除）
- [x] `src-tauri/src/adapters/tool_resolver.rs`：过滤前先 `exclude_companions`；**移除元工具特殊注入**，`request_tool` 按普通条目走同一条白名单过滤；`disabled_tools` 对伴随 id 同样不生效
- [x] `contract/trait.rs`：新增 `Tool::brief()` 默认实现（`None` → 回落 `description()` 首句）；`description()` 注释限定为「只在工具已进入本轮 tools 数组时下发」；`companion_of()` 注释改为「准入而非注入」
- [x] `super_read/mod.rs`：`description()` 删掉 `Default to read` 反向指引，补回 `unit = "byte"` 必须落在字符边界（否则计入 `linesBeyondColumn`）与「输出与 `read` 逐字节一致」两条互操作事实；新增 `brief()`
- [x] `super_edit/mod.rs`：`description()` 删掉 `Default to edit`，补回「`endColumn = startColumn` 是空区间（纯插入）」「非空区间 `expected` 必填且逐字节相等」「`endLine` inclusive / `endColumn` exclusive」三条会失败的约束；新增 `brief()`
- [x] `agent_read/mod.rs`：`[hint]` 门控改为「本轮 `available_tool_names` 含 `request_tool`」，文案指向 `detail = "enable"`
- [x] `agent_edit/{mod.rs,report.rs}`：0 匹配诊断加同门控一句，指向 `request_tool(detail = "enable", tools = ["super_edit"])`
- [x] `chat/tool/ToolConfigPanel.tsx`：`companionOf != null` 与基工具同处一个不可断行渲染单元（`groupToolUnits`，`inline-flex`）——无勾选框、无状态点、字色弱化、用途与从属状态进 tooltip；`enabled_count` 不计；`request_tool` 改为可勾选条目（组内末尾、「申请入口」标签、计入计数）
- [x] `settings/shared/sections/ToolAssemblySection.tsx`：模式级面板同口径（从属条目 + 元工具可勾选）
- [x] i18n（中英各两份，覆盖 `chat.tool.panel` 与 `settings.agent_mode.tool_assembly`）：`companion_of` / `companion_blocked` / `meta_badge` / `meta_hint`
- [x] **可观测性补充（§3.6 / §9 L18）**：`exclude_companions` 对每个被剔除的伴随 id 打一条 `log::warn`（`warn_ignored_companion`，`OnceLock<Mutex<HashSet>>` 按 id 做进程级去重，避免脏配置每轮刷屏）。语义不变——只是让「被忽略」在日志侧可观测
- [x] `chat/core/toolConfig.ts`：新增 `stripCompanionIds(config, tools)`（剔除两个名单里能识别为伴随的 id，无残留时返回原对象），由 `useToolRegionState.applyChange` / `applySetDefault`、`ToolAssemblySection.emit`、`useModeForm.handleSave` 四个写回口统一调用；**不剔除清单里查不到的未知 id**（MCP 断开等临时缺席不应丢用户勾选）
- [x] `chat/core/toolConfig.ts`：新增 `countSelectableTools(tools)`；`ToolToolbar` 的「已启用 / 全部」分子分母都改走可勾选口径（原来分子取 `enabledTools.length`、分母取 `tools.length`，伴随项只撑大分母，显示成 11/13）
- [x] `chat/core/__tests__/toolConfig.test.ts`：计数分子/分母口径、`stripCompanionIds` 剔除范围与「无残留返回原对象」共 5 条断言
- [x] `tests/super_read.rs`：`description_serves_the_full_tier_only` 按档位双向断言（`full` ≤ 600 字符、不含 `Default to`、含 `linesBeyondColumn`；`brief` ≤ 120 字符、含 `Request it when`、不含调用约束）
- [x] `tests/super_read.rs`：`read_hints_super_read_only_when_it_cannot_express_the_result` 上下文改为 `["read", "request_tool"]`（旧上下文 `["read", "super_read"]` 对应已被 T7 取代的门控）
- [x] `tests/tool_resolver.rs`：G17 身份过滤用例——`companion_ids_in_the_whitelist_never_get_injected`、`a_companion_without_its_base_injects_nothing`
- [x] `tests/super_edit.rs`：`description_serves_the_full_tier_only` 对称断言（`full` ≤ 600 字符、不含 `Default to`、含 `no unique anchor` 与 `empty range`；`brief` ≤ 120 字符、含 `Request it when`）；另断言两档**都不含 `substring`**，把「不得用否定式定义」钉成回归项（§6）
- [x] **措辞修正（§6）**：`super_edit` 的 `description` / `brief` 与 `agent_edit/report.rs` 的升级提示原写作「不做子串匹配 / instead of substring matching」，改为表达**定位方式**（「按行列地址定位，不需要唯一锚点」）。原措辞既冗余（「按行列地址」已蕴含），又会被读成「不用提供准确文本」——与紧随其后的「`expected` 须逐字节相等」自相矛盾。面板 `tool_desc`（中英）同步改为用户可感知的收益（「重复内容不影响定位」）
- [x] G20 面板用例：`src/chat/tool/__tests__/panelStructure.test.ts`（11 用例）——聊天面板「伴随条目并入基工具的不可断行渲染单元」（源码结构 + `.toolUnit` `inline-flex` / `.toolList` `flex-wrap`）、「伴随分支无 `Checkbox` / `onChange` / `setTierSelection`」、「元工具分支有 `Checkbox` + `metaBadge` + 写回」、计数走 `countEnabledSelectableTools`（并断言源码不出现 `config.enabledTools.length` / `tools.length`）；设置页「缩进从属行 + 只读状态点、无勾选框」与 `.companionRow` 缩进声明、元工具同口径；中英两份文案的键与 `{{base}}` 占位符。**渲染层门禁为何落在源码结构**：仓库无 jsdom / `@testing-library/react`（`vitest.config.ts` 无 `environment`、`src/testSetup.ts` 只补空 `window`），且 I48 §9 第 7 条已决定「不为此引入组件测试基建」——断言结构性事实（哪条分支有勾选框、渲染单元怎么折、CSS 怎么声明），不断言像素与运行时布局；纯函数级口径仍由 `src/chat/core/__tests__/toolConfig.test.ts` 承担

验收：`cargo test -p assistant-tools --tests` 全绿；`cargo test --test tool_resolver` 18/18；`cargo clippy -p assistant-tools --tests -D warnings` 无警告；`npx tsc --noEmit` 通过；`npx eslint`（改动文件）无问题；`npx vitest run src/chat` 23 文件 / 341 用例全绿（新增 `chat/tool/__tests__/panelStructure.test.ts` 11 条）。

---

## T8 — 升级通道（§3.6 / §10 G4·G21·G22·G23）· 已完成

退出条件：G21 / G22 / G23 通过；升级只改变**下一迭代**的工具数组，且旧前缀逐字节不变；跨轮不携带。

> **状态（2026-11-19）**：契约、实现与门禁用例**全部落地**（下面 17 条已全勾）。验收命令全绿：`cargo test -p assistant-tools --tests`（`request_tool` 19/19、`agent_read` 47/47、`super_edit` 16/16、`super_read` 20/20 等各组）、`cargo test -p assistant-agent --features test-doubles`（`loop_executor` 40/40）、`cargo test -p assistant-app`（含新增 `tests/tool_escalation.rs` 11/11）、`cargo clippy`、`cargo fmt --check`。

> 顺序约束：**先 T7 再 T8**。T7 把「伴随 = 注入」拿掉之后 `request_tool` 才有必要成为唯一入口；反过来做会让升级通道与白名单展开机制并存，门禁无法区分两条注入路径（§11）。
>
> 拒绝方案 R7 仍成立：工具**不**直接改写本轮工具集 / provider 请求——工具只登记申请，循环在批次结束后统一尾部追加。理由是 `ToolUseContext` 不可变、审批与事件面以「循环持有唯一请求」为前提、同批次并发工具会争抢同一份可变请求。

- [x] `contract/escalation.rs`（新增，取代「放进 `trait.rs`」）：`ToolEscalationPort`（`candidates` / `request` 两个 async 方法）+ `EscalationCandidate` / `EscalationRejectReason` / `EscalationRejection` / `EscalationOutcome`；`lib.rs` 与 `contract/mod.rs` 导出。独立成文件而不是塞进 `trait.rs`：升级契约只被 `request_tool` 与适配器消费，与 `Tool` trait 无耦合，混在一起会让「工具定义」与「工具集变更」两种职责共用一处
- [x] `contract/context.rs`：`ToolUseContext` 增 `escalation: Option<Arc<dyn ToolEscalationPort>>`（`#[serde(skip)]`，`Default` 为 `None`）
- [x] `os/tools/agent_request_tool.rs`：`detail` 增 `"enable"`；入参增 `tools`；`data` 增 `escalatable` / `enabled` / `rejected`；`list` / `manual` 两个分支逐字节不变
- [x] `os/tools/agent_request_tool.rs`：F13——`detail = "enable"` 时 `tools` 缺失 → 业务错误（`tools` 缺失不做成 `validate_input` 失败：校验失败没有 `data` 通道，可升级段会跟着丢）；F14——申请没有 `companion_of` 的工具、或其基工具不在本轮 → 计入 `rejected`，全部被拒才 `ok = false`
- [x] `os/tools/agent_request_tool.rs`：`disabledTools` 里的伴随 id **不构成阻断**（端口不读 `disabled_tools`，与 G17 一致，用户唯一控制点是基工具）
- [x] `src-tauri/src/adapters/tool_escalation.rs`（新增）：`RegistryEscalationPort` = 注册表 + 本轮层级 + 本轮收集器，只校验与登记；`MaxToolTierSource`（按 `conversation_id` 每次求值，与 `WorkspaceRootSource` 同形态）；名字解析先 `find_by_name` 再回落 `find`
- [x] `src-tauri/src/adapters/tool_exec.rs`：每次 `execute` 现构造端口并注入；任一侧缺失（未装配 / 无收集器）→ `escalation = None`，`enable` 返回业务错误而**不**静默降级
- [x] `src-tauri/src/tool_setup.rs`：装配 `with_escalation(registry, MaxToolTierSource)`；层级来源读会话落库的 `tool_config_json`，读不到 fail-closed 到 `Sandbox`
- [x] `assistant-agent`：`ToolExecCtx` 增 `escalations: Option<Arc<dyn EscalationSink>>`；`tools` 传 `state.tools.as_deref()`（轮内工具集快照）
- [x] `assistant-agent` 循环：`RoundState.tools: Option<Arc<Vec<ToolSpec>>>` 持有工具集，批次结束后 `drain` + 尾部追加，**下一迭代**生效（申请所在迭代内不生效）
- [x] 尾部追加保证旧工具数组**前缀逐字节不变**（`append_escalations` 只 push 到末尾，同名去重不重排旧元素）；`drain` 为空则不重建请求
- [x] 跨轮不携带：`RoundEscalationSink` 每轮现创建、turn 结束即丢弃，不持久化（§9 L17）
- [x] `tests/request_tool.rs` / `src-tauri/tests/tool_escalation.rs`：G21——`read` 不在本轮时申请 `super_read` → F14 且工具集不变；`request_tool` 未勾选时模型根本无法发起（不在 tools 数组）。用例：`escalating_without_the_base_tool_is_rejected` / `escalating_a_non_companion_tool_is_rejected` / `escalating_an_unknown_name_is_rejected` / `escalating_above_the_turn_tier_limit_is_rejected` / `enable_all_rejected_reports_every_reason` / `enable_without_tools_is_a_business_error_that_keeps_the_candidates` / `the_meta_tool_requires_an_explicit_check`（`tests/tool_resolver.rs`）
- [x] `crates/assistant-agent/tests/loop_executor.rs`：G22——合法申请后下一迭代 provider 请求含 `super_read`，`ToolExecCtx.tools` 与 `available_tool_names` 同步含它。用例：`g22_escalation_takes_effect_from_the_next_iteration`（下一迭代请求 = `["echo", "super_read"]`、执行上下文快照不含新项）、`g22_duplicate_escalation_is_merged_once`、`a_fresh_sink_does_not_carry_previous_escalations`（跨轮不携带）
- [x] G4 回归——调用 `request_tool(detail = "enable")` 前后**本轮**工具集不变（变更只发生在批次结束后）。用例：`g4_no_escalation_keeps_the_request_tool_set_stable`（空 `drain` 不重建请求）、`enable_does_not_touch_the_turn_tool_set`（目录与本轮可用名逐项不变）
- [x] G23 三档不变式逐条断言——①`full ⇔ callable`（tools 数组内每项都有 `description` + `input_schema`，且 `escalatable` 项不在数组内）；②`brief ⇒ eligible`（可升级段每项基工具都在 `available_tool_names`）；③`none ⇔ ¬eligible`（基工具不在本轮则既不在 `listed`、也不在 `escalatable`、也不在 tools 数组）；④`listed ∩ escalatable = ∅`；⑤`brief` 不含 `input_schema` 与手册正文。用例：`g23_the_three_description_tiers_hold_across_the_turn_matrix`（9 组本轮工具集 × 2 个层级上限的全矩阵）、`the_escalatable_section_carries_the_brief_only`、`an_available_companion_is_not_escalatable`、`enabled_items_leave_the_escalatable_section`
- [x] `agent_request_tool.rs`：可升级段取 `Tool::brief()`（缺省回落 `description()` 首句），**不**取 `description()` 全文、**不**取 `manual()`（G19 / G23 ⑤）。实现落在端口的 `candidates` 一侧（`brief()` 是 async，只有适配器能调），工具侧只消费端口给出的成品条目

验收：`cargo test -p assistant-tools --tests`、`cargo test -p assistant-agent`、`cargo clippy -p assistant-tools --all-targets`、`cargo fmt --check` 全通过；`npm run contract:gen` 无 diff（T8 不改 DTO）。

---

## 收口核对（每批次结束跑，不单独占切片）

- [x] G15：`npm run contract:gen` 无 diff、`src/types/protocol.contract.ts` 与 `src/vault/core/contract/vault.contract.ts` 无变化（本阶段无 DTO 改动）——已跑，`git status` 对两个生成文件为空
- [x] `read` / `edit` 既有测试全通过（P16 的改动不回退）——`agent_read` 47/47、`agent_write` 27/27、`super_edit` 16/16、`super_read` 20/20
- [x] 命令（cwd = `src-tauri`）：`cargo test -p assistant-tools`、`cargo test -p assistant-agent`、`cargo clippy -p assistant-tools --all-targets`、`cargo fmt --check`——全绿；另跑 `cargo test -p assistant-app`（含 `tests/tool_escalation.rs` 11/11）
- [x] 邻接确认：`ToolBox` 入口（`ToolUseContext::new`）下 `request_tool` 报「catalog is unavailable」、`super_read` / `super_edit` 报「index root is unavailable」，均不 panic（用例：`missing_catalog_is_a_business_error`、`gateway.rs` 的索引根缺失路径）
- [x] 依赖确认：未新增 crate 依赖（`git diff --stat -- "*.toml"` 为空；`Arc` / `Arc<dyn …>` 均来自 std）
- [x] G16 复核：`crates/assistant-tools/src` 下不存在手写 `"[encoding:"` / `"[long-lines:"` / `"[index:"` / `"[truncated:"` 一类字面量（全部走 `footer::render()`）——检索命中项仅为 `footer.rs` 自身渲染/断言、`compact` 解析用例与 `storage.rs` 的 spill 常量，无新增手写

---

## 验收记录

- 2026-11-18 · T1 · G1–G5 · `cargo test -p assistant-tools --test request_tool` 10/10、`--lib` 227/227 通过；`request_tool` 只读、回退标注、手册上限（2000/8000）与 `[catalog: showing k of n]` 均有用例；`FOOTER_TAGS` 登记与识别各有单测。
- 2026-11-18 · T3 · G8/G12/G16/G17 · `cargo test -p assistant-tools --test super_read` 11/11 通过；footer 生产方全部改走 `footer::render()`，`edit` 的诊断对 `TAGS` 每一项都有文案；伴随工具展开与 `[hint]` 触发条件各有用例。
- 2026-11-18 · T4 · G9/G10/G13 · `cargo test -p assistant-tools --test super_read` 14/14 通过（新增单飞、失效、回执同构三条）；`cargo test -p assistant-tools` 全绿、`cargo clippy -p assistant-tools --all-targets` 无警告、`cargo check --workspace --all-targets` 通过；`edit` / `write` / `delete` / `move` 均已在成功后调用 `super_read::invalidate_path`，既有测试未回退。
- 2026-11-18 · T4 修订 · 设计变更（未改代码）· 「写侧主动通知失效」改为「行起点自证」（§4 / §9 L11·L12 / §12 R10–R12）。上述 T4 记录中「四个写工具均调用 `super_read::invalidate_path`」一项**随本次修订作废**，待按 T4 新增条目撤销；正确性改由自证承担，`invalidate_path` 降为纯缓存卫生。
- 2026-11-19 · T7 · G17/G19（部分）/G20（部分）· 身份过滤落地（`expand_enabled_tools` → `exclude_companions`），`tool_resolver` 18/18；描述按档位拆分，`super_read` 的 full/brief 双向断言通过；`npx vitest run src/chat` 22 文件 / 325 用例全绿；`npx tsc --noEmit` 通过（顺带修掉 `ToolConfigPanel` 的 `isCompanionTool` 未使用导入）。**未关闭项**：G20 缺渲染层用例（`super_edit` 的 full/brief 断言已于同日补齐）。
- 2026-11-19 · T7 补充 · 可观测性（未改语义）· 配置里残留的伴随 id 曾短暂改为「剔除 + 面板告警」（`isIgnoredCompanionInConfig` + `companion_ignored`）；同日**撤销**该设计：实际不存在需要靠提示解释的忽略项，改为写回前 `stripCompanionIds` 静默修掉（四个写回口统一），后端 `warn_ignored_companion` 日志保留。`exclude_companions` 判据不变，G17 的「配置不构成 `callable` 第二来源」仍成立；规格 §3.6 已同步。
- 2026-11-19 · T8 · 代码落地（门禁用例待补）· `contract/escalation.rs`（端口 + 四个数据结构）、`ToolUseContext.escalation`、`request_tool` 的 `enable` 分支与可升级段、`EscalationSink` + `ToolExecCtx.escalations`、`RoundState.tools` + 尾部追加、`adapters/tool_escalation.rs`、`tool_setup.rs` 装配全部完成；`cargo check --all-targets` 通过、`cargo test -p assistant-tools --lib` 286/286 通过。**未关闭项**：G21 / G22 / G4 / G23 四条门禁缺用例（下面四个未勾条目即缺口）。
- 2026-11-19 · T8 修订 · 借用形态（§3.8 ②）· `ToolExecCtx.escalations` 由 `Option<&'a dyn EscalationSink>` 改为 `Option<Arc<dyn EscalationSink>>`。原因：`ToolUseContext.escalation` 是 `Option<Arc<dyn ToolEscalationPort>>`（无生命周期参数），端口要随工具调用存活到调用结束，借用会让 `'a` 传染到 `ToolUseContext`，而后者是工具签名的一部分。规格 §3.8 ② 已同步。
- 2026-11-19 · T8 修订 · 默认值（§3.6）· 元工具 `request_tool` **不进任何默认值**：内置模式种子 `BUILTIN_AGENT_TOOL_CONFIG` 保持空白名单（`agent_modes.rs` 及其测试同步为 `enabled_tools.is_empty()`）。原规格 §3.6「模式默认值应显式包含 `request_tool`」一句**已删除**并改写为「元工具不进入任何默认值 + 三条理由」（deny-by-default 不可破例 / 单独注入元工具是半启用态、`escalatable` 恒为空 / 默认值只允许一个来源）。G17·G20·G21 措辞本就按「未勾即无入口」写成，无需连带改动。
- 2026-11-19 · T8 · 门禁用例落地（G4/G21/G22/G23）· 新增 `src-tauri/tests/tool_escalation.rs`（11 用例：准入五条拒绝路径、`disabledTools` 不阻断、跨轮不携带、G23 全矩阵三档不变式）；`crates/assistant-agent/tests/loop_executor.rs` 新增 3 用例（`g22_escalation_takes_effect_from_the_next_iteration` / `g22_duplicate_escalation_is_merged_once` / `g4_no_escalation_keeps_the_request_tool_set_stable`）；`crates/assistant-tools/tests/request_tool.rs` 新增 9 用例（可升级段只带 brief、无端口不输出、F13/F14 语义、部分通过、`enable` 不改本轮工具集、已生效项离开可升级段）。顺带修掉两处实现缺口：F13 业务错误此前丢失可升级段（模型拿到错误却不知可申请什么），已按 §3.8 ⑤ 补回；非法 `detail` 的校验文案同步列出 `enable`。验收：`cargo test -p assistant-tools --tests`、`cargo test -p assistant-agent --features test-doubles`、`cargo test -p assistant-app`、`cargo clippy`、`cargo fmt --check` 全绿。
- 2026-11-19 · T7 · G20 渲染层用例落地 · 新增 `src/chat/tool/__tests__/panelStructure.test.ts`（11 用例）。仓库测试环境无 jsdom / `@testing-library/react`（`src/testSetup.ts` 只补空 `window`），故断言落在**源码结构 + CSS 声明**层：伴随条目并入 `groupToolUnits` 单元、伴随分支无 `Checkbox`、元工具分支带 `metaBadge`、计数走 `countEnabledSelectableTools`、`.toolList { flex-wrap: wrap }` + `.toolUnit { inline-flex }` + `.companion { nowrap }` 三条 CSS 声明齐全、中英 i18n 键集合完整且带 `{{base}}` 占位符、设置页 `ToolAssemblySection` 同口径（缩进一级 + 只读状态点）。`npx vitest run src/chat` 23 文件 / 341 用例全绿。**G20 至此关闭**。
- 2026-11-19 · T2/T6 · 失败矩阵 F1–F4 公开入口补全 · `tests/super_read.rs` 新增 3 条（`an_index_over_the_capacity_cap_is_f1`：8 MiB 纯换行 → 起点表自然超 64 MiB，不造大文件也不注入阈值；`a_binary_file_is_f2`：与 `read` 同判据同文案；`a_path_outside_the_workspace_is_f3`：与 `read` 共用 `resolve_readable`）；`super_index/proof.rs` 新增 `the_stale_message_names_the_line_and_the_cause` 钉住 F4 文案（「重建后仍失败」是竞态，公开入口无法稳定构造，故在判据层断言）。`super_read` 由 17 条增至 20 条。**T6 失败矩阵与 G11 关闭**。
- 2026-11-19 · T6 · 收口复核 · G15 实测：`npm run contract:gen` 后 `src/types/protocol.contract.ts` 与 `src/vault/core/contract/vault.contract.ts` 零 diff；G16 实测：两个超级工具源码内无手写 footer 字面量（全走 `footer::render`），`footer.rs` 单测保证每个登记 tag 可识别且不重复；注册组复核：`super_read` 在 `register_readonly_tools`、`super_edit` 在 `register_write_tools`，索引根由 `tool_setup.rs` 注入 `dirs.indexes()`。**P17 全部验收点闭环**。
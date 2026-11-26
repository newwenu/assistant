# I77 — 工具临时区所有者与用户清空规格

| 项 | 值 |
|---|---|
| 状态 | 待实施（后端规格：`tool-scratch` 的所有者化 + 用户显式清空） |
| 日期 | 2026-11-26 |
| 类型 | 设计规格（契约、能力档、改动面、验收） |
| 关联 | I73 路径与引用规格（**冻结稿**，`docs/achievement/`）：本文推翻其 §3.8 中 `E19`「只观测、不提供独立回收动作」的结论，理由见 §4.1；I61 存储管理器规格（**已归档**）：本文沿用其「所有者 / 观测者 / 回收者」三分与「管理器只发起」原则，不复述 |
| 影响范围 | 后端：`src-tauri/crates/assistant-tools/src/runtime/scratch.rs`（新增）、`.../runtime/storage/`（由 `.../runtime/storage.rs` 拆出的目录模块，见 §5.1）、`.../os/tools/agent_shell/spawn.rs`、`src-tauri/src/adapters/tool_exec.rs`、`src-tauri/src/boot/build/tools.rs`、`src-tauri/src/cmd/conversation/mutate.rs`、`src-tauri/src/cmd/storage/{sample/collect.rs, sample/rows.rs, sample/mod.rs, reclaim/executors.rs, reclaim/execute.rs, reclaim/mod.rs, entries/decl.rs}`、`src-tauri/src/app_dirs/registry.rs`、`src-tauri/crates/assistant-protocol/src/storage.rs`（新增 `ClearAll` 动作）。前端**不改交互、不加按钮**；仅随 `npm run contract:gen` 同步生成类型，并补一条动作文案（见 §5 C15） |
| 一句话 | 把 `tool-scratch` 的三项权利（建 / 观测 / 删）收进 `assistant-tools` 的一个真实所有者；删除只分两档——会话删除时整段释放、用户在存储管理器一键清空整根。**不做孤儿识别**、不按时间清扫、不读会话表 |

**文档性质**：设计规格。本文只定契约与判据，不记进度；实施顺序见 §6，验收见 §8。

**编号约定**：`INV1–INV7` 不变量、`V1–V11` 验收判据、`OQ1–OQ4` 开放问题、`D1–D7` 判定项。本文中的「必须 / 禁止」为可验收约束。

---

## 1 判定

| # | 判定 | 依据 |
|---|---|---|
| D1 | 引入**真实所有者** `ToolScratchStore`（trait + impl，位于 `assistant-tools`），不是把登记表里的字符串改名 | §3.1 |
| D2 | 建目录、观测、删除三条路径**全部改道所有者**；应用层不再自己 `join` 段、不再自己 `remove_dir_all` | §3.2 |
| D3 | 回收动作定为 `ClearAll` + `reason: Transient`；**禁止** `DeleteOrphans`（无差集）与 `SweepExpired`（无时间判据） | §4.1 |
| D4 | **不做孤儿识别**：所有者不读会话表、不算差集、不接受 `live` 集合；删除判据只有「用户显式发起」与「会话删除」两种 | §4.2 |
| D5 | 本期**不做**启动期自动清扫；只提供存储管理器的手动清空入口 | §4.4 |
| D6 | `runtime/storage.rs` 的拆分作为**顺手做区块**纳入本次（R0 先于 R1），纯机械搬移、公开 API 路径不变 | §5.1 |
| D7 | 「打开目录」登记为保留条目（OQ4），本期不做；其形态须对全部 `Local` 条目通用，不得为 `E19` 单独开洞 | §9 |

---

## 2 现状（事实，非设计）

`tool-scratch` 是唯一「有登记条目、无所有者实现」的目录：登记表 `owner: "ShellToolScratch"` 只是展示标签，全仓无同名类型。三项权利散落三处，且都绕过登记表：

| 职责 | 现在在哪 | 是否 owner |
|---|---|---|
| 拼路径（会话段） | `runtime/storage.rs` 的 `scratch_root_for`（纯函数，注释自述「唯一实现点」） | 否 |
| 建目录（幂等） | `os/tools/agent_shell/spawn.rs` 的私有 `prepare_scratch`，只拿到基根 `PathBuf` | 否 |
| 观测 | `cmd/storage/sample/rows.rs` 的 `sample_scratch` 直接 `tree_stats(dirs.tool_scratch())`，由管理器遍历 | 否（违反「遍历归 owner」） |
| 删除 | `cmd/conversation/mutate.rs` 的 `remove_session_scratch`，应用层直接 `std::fs::remove_dir_all` | 否 |

能力档现状：`entries/decl.rs` 给 `E19` 声明 `action: None` / `eligibility: NotImplemented` / `reason: NoReclaimApi` / `auto_active: false`；`entries/eligibility.rs` 落 `None` 分支；`reclaim/execute.rs::executor_action` 对 `ToolScratch` 返回 `None`。

已知两个真实缺口：`shared` 段（`session_id == None` 落点）无对应会话行，正常删除路径永不触及；崩溃 / 断电留下的孤儿 `<session_hash>` 目录无任何回收路径。

**这两个缺口由同一只手覆盖**：用户显式「一键清空」删除基根下全部子项（§4.1），无需识别谁是孤儿、谁是 `shared`。会话段是 FNV-1a 单向哈希、目录名不可反推会话 id（§4.2），因此「按会话精确清理」只有会话删除一条路；其余一律交给用户的显式意图。

对照样本：`E3`（`ToolResultStorage`）已是完整形态——trait 自带观测 / 策略 / 回收，应用层只注入 `Arc<dyn ...>`。本文按同一范式改造 `E19`。

---

## 3 所有者契约

### 3.1 形状

新增 `assistant-tools/src/runtime/scratch.rs`：

```rust
pub trait ToolScratchStore: Send + Sync {
    /// 基根（`AppDirs::tool_scratch()`）；只读，供装配层核对。
    fn root(&self) -> &Path;
    /// 会话段路径；内部委托 `scratch_root_for`，不新增第二套拼法。
    fn session_root(&self, session_id: Option<&str>) -> PathBuf;
    /// 幂等建目录；已存在直接复用，不清空。shell 调用前唯一入口。
    fn prepare(&self, session_id: &str) -> std::io::Result<PathBuf>;
    /// 整树只读统计，口径与回收共用同一份遍历。
    fn stats(&self) -> std::io::Result<DirStats>;
    /// 会话结束：删整棵会话段；`NotFound` 静默。
    fn delete_session(&self, session_id: &str) -> DeleteReport;
    /// 用户显式清空：删基根下**全部**子项（含 `shared`、含活跃会话段），**保留基根本身**。
    fn clear(&self) -> DeleteReport;
}
```

约束：

1. `session_root` **必须**委托 `scratch_root_for`（拆分后位于 `runtime/storage/naming.rs`）。该函数是会话段拼法的唯一实现点，owner 化之后调用点收敛为「owner 内部一处」。
2. `stats` **必须**复用 `runtime/storage/tree.rs` 的 `dir_tree_stats`。任何调用方自写 `read_dir` 循环都会让统计口径与回收口径漂移。
3. `DeleteReport` 必须区分 `removed` / `failed` / `skipped`（沿用 `E3` 同款字段语义）：文件被占用导致的部分失败是正常结果，不回滚、不重试、不上抛为命令错误。
4. `clear` 的四条纪律（缺一条就会产生错误报告或越界删除）：
   - **字节必须在删除前取**：逐子项先 `dir_tree_stats` 再删，否则 `removed_bytes` 恒为 0；
   - **不跟随符号链接**：`symlink_metadata` 判定，链接与普通文件走 `remove_file`，只有真目录走 `remove_dir_all`；
   - **部分失败计入 `failed`**：Windows 下被占用的子项删不掉属正常结果（INV6）；
   - **基根必须保留**：只删子项、不删基根本身；`prepare` 是 `create_dir_all`，下一次 shell 调用自动重建（§8 V7「自愈」）。
5. **不提供** `collect_orphans` / `delete_namespaces` / `ScratchCandidate`：不做孤儿识别（D4）。R1 曾按旧方案落地的差集三件套在本批次删净，不留死代码。

    **`delete_namespaces` 删除时必须搬运的两件事**（否则产生回归）：

   - `delete_session` 当前委托 `delete_namespaces(&[session_namespace(...)])`，删掉后被委托方要自己实现函数体；
   - `delete_namespaces` 里的 `shared` 守卫**必须搬进 `delete_session`**：`session_namespace` 对空串归入 `shared`（`naming.rs` 的 `filter(|id| !id.is_empty())`），故 `delete_session("")` 算出的命名空间就是 `shared`。守卫消失会让它删掉 `shared` 目录。

   `namespace_dir` **不在此列**：它是 `scratch_root_for` 的实现细节，仍然活着（见 C12）。

### 3.2 注入与改道

| 路径 | 现在 | 改后 |
|---|---|---|
| shell 建目录 | `adapters/tool_exec.rs` 用 `with_tool_scratch(PathBuf)` 注入基根，`execute` 内派生会话段 | 注入 `Arc<dyn ToolScratchStore>`；`execute` 只把 `session_id` 交给工具，工具调 `store.prepare()` |
| 会话删除 | `mutate.rs` 三入口调 `remove_session_scratch` | 三入口调 `store.delete_session()`；删除函数整体移除 |
| 存储观测 | `sample/rows.rs::sample_scratch` 自遍历 | 调 `store.stats()` |
| 存储清空 | 无 | 新增执行器调 `store.clear()` |

所有者实例在 `boot/build/tools.rs` 装配期创建一次，`Arc` 同时交给 `ToolExec` 与存储管理器（与 `E3` 的 `storage()` 同款双重暴露），**禁止**两处各自 `new` 一个。

---

## 4 能力决策

### 4.1 动作：`ClearAll`，不是 `DeleteOrphans`、不是 `SweepExpired`

- **不是 `SweepExpired`**：`mtime` 不可判。会话期文件持续被写，**仍存活但空闲**的会话目录会被误判为过期；按时间删等于删活会话，直接击穿 I73 的「会话期必须存在」。
- **不是 `DeleteOrphans`**：孤儿判据要求所有者做磁盘侧差集、应用层读会话表算 `live`（旧方案 §4.2）。这条链的**唯一产出**是「自动回收时排除活会话」，而本期删除权完全交给用户显式意图（§4.4），差集不再有消费者。保留它等于为一个不存在的执行路径维护跨域依赖（`cmd/storage/sample` → conversation 表）。
- **是 `ClearAll`**：删除基根下全部子项。语义与 `E2` / `E4` / `E14` 同档——「这条目录整体可弃」，不做逐项判定。
- `StorageReclaimActionDto` **新增成员** `ClearAll`（`assistant-protocol/src/storage.rs`），需跑 `npm run contract:gen`，并补前端动作文案（§5 C15）。这是本规格**唯一**的 wire 变更。
- 因此本文**推翻** I73 §3.8 对 `E19` 的「只观测」结论；这是能力升级，必须在本文登记，不得只改代码。

### 4.2 不做孤儿识别：判据只有两个来源

会话段是 FNV-1a 单向哈希，目录名**不可反推**会话 id：

```
3f2a9c81d4e0b7f2  ← 属于哪个会话？磁盘上读不出来
```

因此「哪些目录可以删」不能由所有者从磁盘侧自证。本期把删除判据收敛为两个**显式**来源，二者都不需要差集：

| 判据 | 谁发起 | 删什么 | 精度 |
|---|---|---|---|
| 会话删除 | 应用层（用户删会话） | `session_namespace(session_id)` 单段 | 精确到会话 |
| 用户清空 | 用户点「清理」（`ClearAll`） | 基根下全部子项 | 精确到整根 |

- **所有者不读 DB**、不接受 `live` 集合、不计算差集（crate 边界与简化收益）。
- `cmd/storage/sample` 因此**不需要读会话表**：`scratch` 句柄从 `boot::tool_exec(app).scratch()` 取，与 `E3` 的 `storage()` 同款。
- 崩溃 / 断电残留的孤儿目录**不再被自动发现**，只能由用户「清空」带走。这与 OQ1（不做启动期自动清扫）一致：旧方案提供的也**不是自动发现**，只是「手动回收时排除活会话」。

### 4.3 清空的四个后果（必须写进 note 与文案）

用户清空是全局动作，代价落在具体会话上，须如实说明而不是隐藏：

| 后果 | 影响面 | 缓解 |
|---|---|---|
| 其他并发会话的 `scratch://` 引用失效 | 用户对 B 会话无感知 | `prepare` 幂等重建（§3.1 约束 4），`read` 报 not-found，重跑命令即可 |
| 飞行中子进程的 `TEMP` / `TMP` / `TMPDIR` 被清空 | 正在跑的 `cargo` / `npm` 拿到 IO 错误 | 同上：目录自愈，本轮重跑 |
| `shared` 段被一并清空 | 无会话归属的落点 | OQ2 由「永续」改为「随用户清空，不自动回收」 |
| 基根本身保留 | 无 | `prepare` 是 `create_dir_all`，下一次调用自动建回会话段 |

**内容可丢弃 ≠ 可在会话中途由系统静默删除**。承诺的对象是「会话段的**存在性**」（I73 N14），段内文件内容本就不保证稳定（`shell` 描述已写「same names overwrite」）。用户清空打破的是内容，不是承诺——因为承诺由用户在会话结束后才终止，而清空是用户的显式选择。

### 4.4 时机：本期只有手动清空

- `delete_session`（会话删除时）与 `clear`（用户在存储管理器点清理）两条。
- **不做**启动期自动清扫：需要 Idle 泳道额外读一次会话表，属 `I75` 启动分层的独立议题；本期登记为推迟项（OQ1）。
- `auto_active` 保持 `false`。

### 4.5 能力档终态

| 字段 | 值 |
|---|---|
| `action` | `ClearAll` |
| `reason` | `Transient`（前端已有文案「临时文件」，无需新增 reason） |
| `risk` | `Low`（内容为可重建的临时产物） |
| `confirm` | `confirm`（由 `RiskTier::Low` 自动派生，不单独指定） |
| `auto_active` | `false` |
| `inventory` | `false`（长命令运行期文件持续增删，登记值与扫描值来自两个时刻） |
| `eligibility` | **静态 `Eligible`**（同 `E2` / `E4` / `E14`）：执行器不依赖采样数字，采样失败也不该藏按钮 |
| `reclaimable` | **整根字节**（清空即全清），与 `bytes` 同值 |

静态 `Eligible` 的直接后果：`entries/eligibility.rs` **零改动**（`ToolScratch` 继续落在「其余条目」分组），旧方案的三态判据（`Eligible` / `Referenced` / `Unknown`）与 `scratch_eligibility` 函数整体不需要。

---

## 5 改动面（漏一处即静默漂移）

| # | 位置 | 改什么 |
|---|---|---|
| C1 | `app_dirs/registry.rs` | `owner` 由 `"ShellToolScratch"` 改为 `"ToolScratchStore"`（满足既有命名约束：不含 `::`、非 feature 名、不以 `State` 结尾） |
| C2 | `assistant-tools/src/runtime/{scratch.rs（新增）, mod.rs}` | trait + impl + `DeleteReport`；**移除** `collect_orphans` / `delete_namespaces` / `ScratchCandidate` / `SHARED_NAMESPACE`（R1 已按旧方案落地，本批次删净）；新增 `clear`；`mod.rs` 导出 |
| C3 | `agent_shell/spawn.rs` | `prepare_scratch` 降级为 owner 内部实现；工具改收 store，不再收基根 `PathBuf` |
| C4 | `adapters/tool_exec.rs` | `with_tool_scratch` 改收 `Arc<dyn ToolScratchStore>`；`execute` 删掉自派生段；新增 `scratch()` 只读句柄 |
| C5 | `boot/build/tools.rs` | 创建唯一实例并双重注入（工具 + 存储管理器） |
| C6 | `cmd/conversation/mutate.rs` | 三入口（`delete_conversation` / `batch_delete_conversations` / `purge_conversations`）改调 `delete_session`；删除 `remove_session_scratch`；保持「先提交 DB 事务、再删磁盘」顺序 |
| C7 | `cmd/storage/entries/decl.rs` | `E19` 按 §4.5 改档；note 改为「会话删除时随会话释放；也可在存储管理器一键清空（含其他会话与共享段）」（`shared` 段在用户可见文案里意译为「共享段」，不写内部标识） |
| C8 | `cmd/storage/reclaim/execute.rs` | `executor_action` 放行 `ClearAll`（与 C7 必须同档）；主路由新增 `ToolScratch` 分支 |
| C9 | `cmd/storage/reclaim/executors.rs` | 新增 `execute_scratch_clear`：直接 `store.clear()` → 填 `DeleteReport`；**无** `live` 重算、**无**差集 |
| C10 | `cmd/storage/sample/{mod.rs, collect.rs, rows.rs}` | `SampleInputs` 加 `scratch: Option<Arc<dyn ToolScratchStore>>`；`sample_scratch` 改调 `stats()`，`reclaimable = bytes`；**不加** `live_namespaces` 字段、不读会话表 |
| C11 | `cmd/storage/reclaim/mod.rs` | `ReclaimPorts` 加 `scratch: Option<Arc<dyn ToolScratchStore>>` |
| C12 | `assistant-tools/src/runtime/storage/{naming.rs, mod.rs}` | `namespace_dir` **保留**：`scratch_root_for` 仍调用它（`session_root` / `delete_session` 的路径基础）。只做两件事：①可见性 `pub(crate)` → 私有 `fn`（唯一调用方在同文件）；②删掉 `mod.rs` 的 `pub(crate) use naming::namespace_dir`（`scratch.rs` 停用后该再导出变成 unused import）。同时修正 `naming.rs` 的 doc：原文「调用方拿到的是已算好的 namespace 字符串」是差集的 rationale，需改写。`session_namespace` 与 `SHARED_NAMESPACE` 一并保留（见 C2 守卫说明） |
| C13 | `assistant-protocol/src/storage.rs` | `StorageReclaimActionDto` 新增 `ClearAll`；`reason: Transient` 复用既有成员，无需新增 |
| C14 | 文档 | 在本文登记「推翻 I73 §3.8 `E19` 结论」；I73 自身冻结不改 |
| C15 | 前端：`lexicon.ts` / `locales/{zh-CN,en-US}.json` / `types/protocol.contract.ts`（生成） | `ACTION_KEYS` 加 `clearAll`；两个 locale 各加一条动作文案（如「清空临时文件」/「Clear all temp files」）；跑 `npm run contract:gen` 同步 DTO 联合类型。**不加按钮、不改交互**（`EntryBlock` 只透传 `entry.reclaim.action`，`dialogBody` 用 `actionText` 渲染，缺 key 会显示原串并 warn） |
| C16 | `assistant-tools/src/runtime/storage.rs` → `runtime/storage/` | 顺手做区块：按 §5.1 拆成目录模块，纯机械搬移，公开 API 路径不变 |

编译期兜底：`eligibility.rs` 的 `match` 穷尽列出全部 `CoreEntry`，`E19` 落在「其余条目」分组后不再需要单独分支；`sample/collect.rs` 的 `E19` 分支不是 `match`，**编译器不点名**，必须人工核对（该文件注释已有同款警告）。`execute.rs` 的 `executor_action` 白名单与 `decl.rs` 的 `action` 若不同档，会在计划期被整体拒绝——这是有意的双重校验。

### 5.1 顺手做区块：`runtime/storage.rs` 拆分

拆分判据的唯一来源是 `docs/principle/CODE-SPLIT-PRINCIPLE.md`；本节只登记本次的事实、目标结构与边界，不复述判据条款。

**事实**：`runtime/storage.rs` 共 1447 行 / 非空 1327（≥900 必审档），承担 6 类互不相关的变化原因（端口与共享类型归 `mod.rs`，不单列为变化原因）：

| # | 变化原因 | 代表项 |
|---|---|---|
| 1 | 命名派生 | `session_namespace` / `scratch_root_for` / `unique_prefix` / `safe_component` |
| 2 | 过期判定与回收 | `ExpiredCandidate` / `CleanupReport` / `collect_expired` / `delete_candidates` / `sweep_directory` |
| 3 | 目录统计与整树清理 | `DirStats` / `dir_tree_stats` / `for_each_file` / `PurgeReport` / `purge_tree_files` |
| 4 | 落盘预算与预览文本 | `select_persist_targets*` / `persisted_reference(_from_tail)` / `PREVIEW_*` |
| 5 | 流式 sink 与编码判定 | `FileSpillSink` / `StreamCodec` / `Utf8Decoder` / `probe_encoding` / 码页映射 |
| 6 | 读回执 | `ReadReceiptKey` / `ReadReceipt` / `ReadReceiptStore` / `InMemoryReadReceiptStore` |

**目标结构**（`runtime/storage/` 目录模块）：

| 子模块 | 职责 | 依赖 |
|---|---|---|
| `mod.rs` | trait `ToolResultStorage`、共享类型与常量、`FileToolResultStorage` 组合实现、`pub use` 全部子模块 | 子模块 |
| `naming.rs` | 命名派生（含 `scratch_root_for`） | **无（叶子）** |
| `tree.rs` | 目录统计与整树清理 | `naming` |
| `preview.rs` | 落盘预算与预览文本 | 无 |
| `sink.rs` | 流式 sink、编码判定与码页映射 | `preview` |
| `retention.rs` | 过期判定、收集与删除报告 | `naming` / `tree` |
| `receipt.rs` | 读回执键、条目与内存实现 | 无 |

**边界约束**：

1. **纯机械搬移**：函数体逐字节不动，只改 `use` / `pub use` / 可见性；搬移与语义改写分开提交（`CODE-SPLIT-PRINCIPLE` §7）。
2. **公开 API 路径不变**：`assistant_tools::{DirStats, DeleteReport, ToolResultStorage, SPILL_TRUNCATED_MARKER}`、`storage::scratch_root_for`、`select_persist_targets_with_limits` 的对外路径由 `mod.rs` 的 `pub use` 保住，`src-tauri` 与 `crates/assistant-tools/tests/storage.rs` 零改动。
3. `naming.rs` **必须**是无依赖叶子：R1 新增的 `runtime/scratch.rs` 只依赖它；若它反向依赖 `tree` / `retention`，`scratch → storage` 与 `storage → scratch` 会成环（违反 `CODE-SPLIT-PRINCIPLE` §6）。
4. `dir_tree_stats` 的 `pub(crate)` 可见性保持：`os/tools/super_index` 与 R1 的 `scratch.rs` 都直接调用它。
5. 本文件**无内联测试**（`tests/storage.rs` 是集成测试），拆分不涉及测试搬移；不得借拆分顺手外移或新增测试。
6. 顺序：**R0 先于 R1**。先拆后建——若先建 `scratch.rs` 再拆 `storage.rs`，`scratch.rs` 会先指向旧路径，拆完还得再改一次 import。

**本次不拆的相邻文件**：`adapters/tool_exec.rs`（503 行）与 `os/tools/agent_shell/run.rs`（470 行）虽各有两类变化原因，但 I77 对它们的改动只涉及注入形态与入参，不构成职责混杂；按 `CODE-SPLIT-PRINCIPLE` §7「不为拆分而拆分」留待各自契约变更时处理。

---

## 6 实施顺序

| 批次 | 内容 | 可独立验证 |
|---|---|---|
| R0 | C16：`runtime/storage.rs` 拆分（顺手做区块，见 §5.1） | `cargo test -p assistant-tools` 全绿；公开 API 路径不变 |
| R1 | C2 + C3 + C4 + C5：owner 落地，shell 改道 | shell 调用行为不变；`scratch://` 回读仍可用 |
| R2 | C6：会话删除改道 | 删除会话后目录消失；DB 事务顺序不变 |
| R3 | C2（删差集三件套 + 加 `clear`）+ C12 + C10 + C11 + C1：观测改道、去跨域依赖 | 存储页 `bytes` 与整根一致；全仓无 `live_namespaces` |
| R4 | C13 + C7 + C8 + C9 + C15：新动作落地 | 计划期通过、清空生效、对话框显示正确文案 |
| R5 | C14：文档登记 | — |

R0 / R1 / R2 是行为等价替换（R0 只搬移、不改语义），先合入可降低后续批次的风险面。R0 必须最先合入：R1 的 `runtime/scratch.rs` 依赖拆分后的 `naming.rs` 叶子。R3 起 owner 契约收缩，R4 才引入新能力；C15（`contract:gen` + 文案）与 C13 必须同批，否则 DTO 联合类型与后端不一致会导致类型检查失败。

---

## 7 不变量

| # | 不变量 |
|---|---|
| INV1 | 调用结束**不**清理临时区；`RunGuard` 只负责回收进程树，不碰目录 |
| INV2 | 会话存续期内该会话段**存在性**由系统保证：系统不得以时间、大小或「看起来没人用」为由静默删除；用户显式 `ClearAll` 是唯一例外（§4.3） |
| INV3 | 会话段拼法只有 `scratch_root_for` 一处；所有者、工具、应用层都不得自行 `join` 段名 |
| INV4 | 管理器是**唯一发起者**，所有者是**唯一执行者**；管理器不得自己 `read_dir` / `remove_dir_all` |
| INV5 | 所有者**不读 DB**、不接受外部 `live` 集合；删除判据只来自「会话删除」与「用户清空」两个显式来源 |
| INV6 | 删除失败（文件被占用）不回滚 DB、不重试、不上抛为命令错误，只在报告里体现 |
| INV7 | 拆分（§5.1）不改公开 API 路径与行为；`runtime/storage/` 子模块依赖单向无环，`naming.rs` 保持无依赖叶子 |
| INV8 | `clear` 只删基根**子项**、不删基根；`removed_bytes` 必须在删除前统计 |

---

## 8 验收判据

| # | 判据 |
|---|---|
| V1 | 全仓无 `ShellToolScratch` 标识符；`owner` 指向存在的类型 |
| V2 | 全仓除 `runtime/storage/naming.rs` 外无第二处拼接会话段（字面量 / `join` 均无） |
| V3 | 全仓除 `runtime/scratch.rs` 外无对 `tool_scratch` 基根的 `remove_dir_all` / `read_dir` |
| V4 | shell 连续两次调用同一会话：目录复用、不清空、回读可用 |
| V5 | 删除会话（含批量、purge）后该会话段消失；目录已不存在时不报错 |
| V6 | `E19` 恒为 `Eligible` / `Transient` / `action: ClearAll`，与采样成败无关 |
| V7 | 造文件 → `E19` 显示占用 → 点清空 → 子项消失、**基根仍在**、`reclaimable` 归零、报告 `removedBytes` 与清空前 `bytes` 相符 |
| V8 | **自愈**：清空后立刻发起一次 shell 调用，`prepare` 重建会话段且命令正常执行、`scratch://` 回读可用 |
| V9 | `shared` 段随清空被删除（不再有「永续」特例） |
| V10 | Windows 下目录被占用 → 报告 `failed`，命令不报错，再次清空可捞回剩余部分 |
| V11 | 全仓无 `live_namespaces` / `collect_orphans` / `delete_namespaces` / `ScratchCandidate` 残留；`cmd/storage/sample` 不引用 conversation 表；`namespace_dir` 仍存在且仅剩 `scratch_root_for` 一个调用方（私有，不再是 `pub(crate)`） |

---

## 9 非目标与开放问题

**非目标**：

1. 不合并进 `ToolResultStorage`：两者生命周期语义相反（输出域按天数过期、临时区要求会话期恒存在）。
2. 不改 `RunGuard` / 进程回收；不改 `scratch://` 解析语义。
3. 不引入「cache 所有者」类角色；缓存只是条目的 `reason` / `action` 分类。
4. 不做启动期自动清扫（见 OQ1）。
5. **不做孤儿识别**：不读会话表、不算差集、不提供 `DeleteOrphans`（D4）。
6. 不改前端交互：不加按钮、不改组件；前端改动仅限动作文案与 `contract:gen` 产物（C15）。
7. 「打开目录」本期不做（OQ4）。它**不是 `E19` 专属能力**，落地时必须对全部 `Local` 条目通用。

**开放问题**：

| # | 问题 | 暂定 |
|---|---|---|
| OQ1 | 启动期 Idle 泳道做一次清空是否值得 | 本期不做，另立议题评估 `I75` 启动分层约束 |
| OQ2 | `shared` 段是否需要在会话结束或启动时单独处理 | 本期不单独处理：随用户 `ClearAll` 一并清除，不自动回收 |
| OQ3 | 活跃会话删除时目录被占用（Windows）的残留是否要立即重试 | 本期不重试，由用户下一次 `ClearAll` 捞回 |
| OQ4 | **打开目录**（保留条目） | 本期不做，登记形态要求：①后端命令入参只能是条目 `key`，**禁止接受前端传来的路径**（否则构成任意目录打开漏洞），由后端按 `MANAGED_ROOTS` 查出 `segments` 并自行拼 `AppDirs`；②只对 `RootScope::Local` 且非敏感条目开放；③与既有 `open_attachment` 同形，复用 `tauri_plugin_opener`；④属通用能力，不为 `E19` 单独开洞（非目标 7） |
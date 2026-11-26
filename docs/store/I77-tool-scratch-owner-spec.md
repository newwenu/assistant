# I77 — 工具临时区所有者与孤儿回收规格

| 项 | 值 |
|---|---|
| 状态 | 待实施（后端规格：`tool-scratch` 的所有者化 + 孤儿回收） |
| 日期 | 2026-11-26 |
| 类型 | 设计规格（契约、能力档、改动面、验收） |
| 关联 | I73 路径与引用规格（**冻结稿**，`docs/achievement/`）：本文推翻其 §3.8 中 `E19`「只观测、不提供独立回收动作」的结论，理由见 §4.1；I61 存储管理器规格（**已归档**）：本文沿用其「所有者 / 观测者 / 回收者」三分与「管理器只发起」原则，不复述 |
| 影响范围 | 后端：`src-tauri/crates/assistant-tools/src/runtime/scratch.rs`（新增）、`.../runtime/storage.rs`、`.../os/tools/agent_shell/spawn.rs`、`src-tauri/src/adapters/tool_exec.rs`、`src-tauri/src/boot/build/tools.rs`、`src-tauri/src/cmd/conversation/mutate.rs`、`src-tauri/src/cmd/storage/{sample/collect.rs, sample/rows.rs, sample/mod.rs, reclaim/plan.rs, reclaim/executors.rs, reclaim/execute.rs, entries/decl.rs, entries/eligibility.rs}`、`src-tauri/src/app_dirs/registry.rs`。前端与 wire **零改动**（`DeleteOrphans` / `Orphan` 已是既有枚举成员，不跑 `contract:gen`） |
| 一句话 | 把 `tool-scratch` 的三项权利（建 / 观测 / 删）收进 `assistant-tools` 的一个真实所有者，回收动作定为 `DeleteOrphans`（引用集合由应用层按会话 id 正向算出后传入），不按时间清扫 |

**文档性质**：设计规格。本文只定契约与判据，不记进度；实施顺序见 §6，验收见 §8。

**编号约定**：`INV1–INV6` 不变量、`V1–V9` 验收判据、`OQ1–OQ3` 开放问题、`D1–D5` 判定项。本文中的「必须 / 禁止」为可验收约束。

---

## 1 判定

| # | 判定 | 依据 |
|---|---|---|
| D1 | 引入**真实所有者** `ToolScratchStore`（trait + impl，位于 `assistant-tools`），不是把登记表里的字符串改名 | §3.1 |
| D2 | 建目录、观测、删除三条路径**全部改道所有者**；应用层不再自己 `join` 段、不再自己 `remove_dir_all` | §3.2 |
| D3 | 回收动作定为 `DeleteOrphans` + `reason: Orphan`；**禁止** `SweepExpired` | §4.1 |
| D4 | 孤儿判据由**应用层**把「存活会话 → 命名空间」集合算好后传入；所有者只做磁盘侧差集，不读 DB | §4.2 |
| D5 | 本期**不做**启动期自动清扫；只提供存储管理器的手动回收入口 | §4.4 |

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
    /// 磁盘侧差集：盘上有、`live` 中无的命名空间。`live` 由调用方给。
    fn collect_orphans(&self, live: &HashSet<String>) -> std::io::Result<Vec<ScratchCandidate>>;
    /// 会话结束：删整棵会话段；`NotFound` 静默。
    fn delete_session(&self, session_id: &str) -> DeleteReport;
    /// 批量删除命名空间（孤儿回收用），逐项结果可见。
    fn delete_namespaces(&self, namespaces: &[String]) -> DeleteReport;
}
```

约束：

1. `session_root` **必须**委托 `scratch_root_for`。该函数是会话段拼法的唯一实现点，owner 化之后调用点收敛为「owner 内部一处」。
2. `stats` **必须**复用 `runtime/storage.rs` 的 `dir_tree_stats`。任何调用方自写 `read_dir` 循环都会让统计口径与回收口径漂移。
3. `DeleteReport` 必须区分 `removed` / `failed` / `skipped`（沿用 `E3` 同款字段语义）：文件被占用导致的部分失败是正常结果，不回滚、不重试、不上抛为命令错误。

### 3.2 注入与改道

| 路径 | 现在 | 改后 |
|---|---|---|
| shell 建目录 | `adapters/tool_exec.rs` 用 `with_tool_scratch(PathBuf)` 注入基根，`execute` 内派生会话段 | 注入 `Arc<dyn ToolScratchStore>`；`execute` 只把 `session_id` 交给工具，工具调 `store.prepare()` |
| 会话删除 | `mutate.rs` 三入口调 `remove_session_scratch` | 三入口调 `store.delete_session()`；删除函数整体移除 |
| 存储观测 | `sample/rows.rs::sample_scratch` 自遍历 | 调 `store.stats()` |
| 存储回收 | 无 | 新增执行器调 `store.delete_namespaces()` |

所有者实例在 `boot/build/tools.rs` 装配期创建一次，`Arc` 同时交给 `ToolExec` 与存储管理器（与 `E3` 的 `storage()` 同款双重暴露），**禁止**两处各自 `new` 一个。

---

## 4 能力决策

### 4.1 动作：`DeleteOrphans`，不是 `SweepExpired`

- `mtime` 不可判：会话期文件持续被写，**仍存活但空闲**的会话目录会被误判为过期；按时间删等于删活会话，直接击穿 I73 的「会话期必须存在」。
- `DeleteOrphans` + `reason: Orphan` 已是既有协议成员（`E1` 在用），**无需改 DTO、无需 `contract:gen`**。
- 因此本文**推翻** I73 §3.8 对 `E19` 的「只观测」结论；这是能力升级，必须在本文登记，不得只改代码。

### 4.2 孤儿判据：正向算，应用层给

会话段是 FNV-1a 单向哈希，目录名**不可反推**会话 id，只能正向算：

```
孤儿 = { 盘上目录名 } − { session_namespace(id) | id ∈ 会话表 }
```

- `assistant-tools` 不读 DB（crate 边界）；应用层在 `sample/collect.rs` 与 `reclaim/plan.rs` 用与 `ledger::load_attachment_refs` 同款方式算出 `live` 集合，作为 `SampleInputs` 新字段传入（照搬 `E1` 的 `referenced` 模式）。
- **执行时必须重算** `live`，不得复用采样时刻的快照（与 `E1`「执行时在锁内重算」同源）。
- 单向安全性：采样后新建的会话其目录尚不存在，不会造成误删；采样后删除的会话其目录变孤儿，删掉即正确结果。

### 4.3 `shared` 段口径

`shared` 无对应会话行，按差集恒为孤儿。本期取**保守口径**：`live` 集合恒定包含 `"shared"`，即 `shared` 永不被自动回收，也不计入 `reclaimableBytes`。若将来要清空它，必须另立议题并明确它与 `tool-output` 的 `shared` 桶是否同策。

### 4.4 时机：本期只有手动回收

- `delete_session`（会话删除时）与 `delete_namespaces`（用户在存储管理器点回收）两条。
- **不做**启动期自动清扫：需要 Idle 泳道额外读一次会话表，属 `I75` 启动分层的独立议题；本期登记为推迟项（OQ1）。
- `auto_active` 保持 `false`。

### 4.5 能力档终态

| 字段 | 值 |
|---|---|
| `action` | `DeleteOrphans` |
| `reason` | `Orphan` |
| `risk` | `Low`（内容为可重建的临时产物） |
| `confirm` | `confirm`（由 `RiskTier::Low` 自动派生，不单独指定） |
| `auto_active` | `false` |
| `inventory` | `false`（不参与对账；孤儿差集是 owner 自身遍历） |
| `eligibility` | 动态：孤儿数 `> 0` → `Eligible` / `Orphan`；`== 0` → `NotEligible` / `Referenced`；判据不可算（取不到会话表）→ `NotEligible` / `Unknown`。**禁止**把「不知道」当 0 |

---

## 5 改动面（漏一处即静默漂移）

| # | 位置 | 改什么 |
|---|---|---|
| C1 | `app_dirs/registry.rs` | `owner` 由 `"ShellToolScratch"` 改为 `"ToolScratchStore"`（满足既有命名约束：不含 `::`、非 feature 名、不以 `State` 结尾） |
| C2 | `assistant-tools/src/runtime/{scratch.rs（新增）, mod.rs}` | trait + impl + `ScratchCandidate` / `DeleteReport`；`mod.rs` 导出 |
| C3 | `agent_shell/spawn.rs` | `prepare_scratch` 降级为 owner 内部实现；工具改收 store，不再收基根 `PathBuf` |
| C4 | `adapters/tool_exec.rs` | `with_tool_scratch` 改收 `Arc<dyn ToolScratchStore>`；`execute` 删掉自派生段；新增 `scratch()` 只读句柄 |
| C5 | `boot/build/tools.rs` | 创建唯一实例并双重注入（工具 + 存储管理器） |
| C6 | `cmd/conversation/mutate.rs` | 三入口（`delete_conversation` / `batch_delete_conversations` / `purge_conversations`）改调 `delete_session`；删除 `remove_session_scratch`；保持「先提交 DB 事务、再删磁盘」顺序 |
| C7 | `cmd/storage/entries/decl.rs` | `E19` 按 §4.5 改档；note 改为「会话删除时随会话删除；孤儿可在存储管理器回收」 |
| C8 | `cmd/storage/entries/eligibility.rs` | `ToolScratch` 从 `None` 分组移入动态分支，新增 `scratch_eligibility`（判据：`reclaimable` 三态，照搬 `orphans_eligibility`） |
| C9 | `cmd/storage/reclaim/execute.rs` | `executor_action` 放行 `DeleteOrphans`（与 C7 必须同档）；主路由新增 `ToolScratch` 分支 |
| C10 | `cmd/storage/reclaim/executors.rs` | 新增 `execute_scratch_orphans`：重算 `live` → `delete_namespaces` → 填 `DeleteReport` |
| C11 | `cmd/storage/reclaim/plan.rs` | 计算 `live` 命名空间集合（会话表 → `session_namespace`），随 `SampleInputs` 传入 |
| C12 | `cmd/storage/sample/{mod.rs, collect.rs, rows.rs}` | `SampleInputs` 加 `scratch: Option<Arc<dyn ToolScratchStore>>` 与 `live_namespaces: Option<HashSet<String>>`；`sample_scratch` 改调 `stats()` 并给出 `reclaimable` |
| C13 | 文档 | 在本文登记「推翻 I73 §3.8 `E19` 结论」；I73 自身冻结不改 |

编译期兜底：`eligibility.rs` 的 `match` 穷尽列出全部 `CoreEntry`，新增分支会被点名；`sample/collect.rs` 的 `E19` 分支不是 `match`，**编译器不点名**，必须人工核对（该文件注释已有同款警告）。

---

## 6 实施顺序

| 批次 | 内容 | 可独立验证 |
|---|---|---|
| R1 | C2 + C3 + C4 + C5：owner 落地，shell 改道 | shell 调用行为不变；`scratch://` 回读仍可用 |
| R2 | C6：会话删除改道 | 删除会话后目录消失；DB 事务顺序不变 |
| R3 | C7 + C8 + C9 + C10 + C11 + C12 + C1：观测与回收改道 | 存储页显示、资格、回收结果 |
| R4 | C13：文档登记 | — |

R1 / R2 是行为等价替换，先合入可降低后续批次的风险面；R3 才引入新能力。

---

## 7 不变量

| # | 不变量 |
|---|---|
| INV1 | 调用结束**不**清理临时区；`RunGuard` 只负责回收进程树，不碰目录 |
| INV2 | 会话存续期内该会话段必须存在；任何回收动作不得以时间或大小为由删除存活会话的段 |
| INV3 | 会话段拼法只有 `scratch_root_for` 一处；所有者、工具、应用层都不得自行 `join` 段名 |
| INV4 | 管理器是**唯一发起者**，所有者是**唯一执行者**；管理器不得自己 `read_dir` / `remove_dir_all` |
| INV5 | 判据不可算（取不到会话表）时一律降级为 `NotEligible` / `Unknown`，不得当作「无孤儿」 |
| INV6 | 删除失败（文件被占用）不回滚 DB、不重试、不上抛为命令错误，只在报告里体现 |

---

## 8 验收判据

| # | 判据 |
|---|---|
| V1 | 全仓无 `ShellToolScratch` 标识符；`owner` 指向存在的类型 |
| V2 | 全仓除 `runtime/storage.rs` 外无第二处拼接会话段（字面量 / `join` 均无） |
| V3 | 全仓除 `runtime/scratch.rs` 外无对 `tool_scratch` 基根的 `remove_dir_all` / `read_dir` |
| V4 | shell 连续两次调用同一会话：目录复用、不清空、回读可用 |
| V5 | 删除会话（含批量、purge）后该会话段消失；目录已不存在时不报错 |
| V6 | 造一个无对应会话的目录 → 存储页 `E19` 变 `Eligible` / `Orphan`，回收后目录消失、`reclaimable` 归零 |
| V7 | 全部会话存活时 `E19` 为 `NotEligible` / `Referenced`，回收按钮不出现 |
| V8 | 会话表读取失败时 `E19` 为 `NotEligible` / `Unknown`，不出现按钮，不误删任何目录 |
| V9 | `shared` 段在任何回收路径下均不被删除 |

---

## 9 非目标与开放问题

**非目标**：

1. 不合并进 `ToolResultStorage`：两者生命周期语义相反（输出域按天数过期、临时区要求会话期恒存在）。
2. 不改 `RunGuard` / 进程回收；不改 `scratch://` 解析语义。
3. 不引入「cache 所有者」类角色；缓存只是条目的 `reason` / `action` 分类。
4. 不做启动期自动清扫（见 OQ1）。
5. 不新增 DTO 字段；不改前端。

**开放问题**：

| # | 问题 | 暂定 |
|---|---|---|
| OQ1 | 启动期 Idle 泳道做一次孤儿清扫是否值得 | 本期不做，另立议题评估 `I75` 启动分层约束 |
| OQ2 | `shared` 段是否需要应用级易失策略（启动清空 / 永续） | 本期永续，`live` 恒含 `"shared"` |
| OQ3 | 活跃会话删除时目录被占用（Windows）的残留是否要立即重试 | 本期不重试，交由下一轮孤儿回收捞回 |
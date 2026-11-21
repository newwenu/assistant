# I61 — 存储管理器 剩余任务清单

> 本文件是 I61 待办事项的**唯一基线**。原清单 [I61-storage-manager-tasklist.md](./I61-storage-manager-tasklist.md) 中未完成条目已划掉并指向本文件；已完成条目的核对记录仍留在原清单（历史事实，不改写）。
> 规格：[I61-storage-manager-spec.md](./I61-storage-manager-spec.md)（判据 / 词表 / 白名单 / 门禁以它为准）；前端：[I61-storage-manager-frontend.md](./I61-storage-manager-frontend.md)
> 统计：截至 2026-11-21 共 39 项——T1 17 项（T1-g 修订 15 + T1 遗留 2）、T4 20 项（T4-a 13 + T4-b 7）、散文面收口 2 项（R-39 / R-40）。原清单把 `E12` 的 `A-2` 与 T1-d 的 `C` 记成两条，方案 C 下两者是同一件事（删除 `availabilityReason`），合并为本文件 `R-16` 一处实施、两处一起勾；原 `R-18`（`locked` 文案一致性）同样随 C 方案作废，编号不复用。

**勾选规则**：勾选表示代码已改且对应验收已跑；只改代码未跑验收不勾。本文件不管理提交。

**执行顺序**：`R-01`–`R-15` 必须**整组落地**（同一批编译通过，拆开会导致 `probe` 退役后整片编译失败）；`R-17` 随该组一起关闭；随后 `R-16`（删 `availabilityReason`，依赖 `R-12` 的 golden 改写）与 `R-40`（规格 / 前端设计同步修订）**同一批**落地，先改 `R-40` 的文档再改 `R-16` 的代码，避免代码先删、文档仍要求该字段；`R-39` 的代码改动分散在 `R-08` / `R-09` / `R-11` 的同一批改动里（两处散文文案就在这些文件产出），断言可在 T4 前单独跑；T4 依赖 T1-g 落地 **且** 依赖 I54 `R0`（页面容器与三 tab 挂载位）。

---

## 规格锚点（不复制规格正文，执行时按指针回查）

> 规格是**唯一事实基线**（判据 / 词表 / 白名单 / 门禁全部以它为准），且已达 144 KB / 1114 行。整段嵌入会造出第二份来源：规格一改（如 §3.10 表末行文案、`owner` 命名域），清单里的副本不会跟着改，执行时反而按错的字面量落地。因此本文件**只引用章节号 + 判据 ID + 门禁 ID**，不复制正文；只有「必须逐字对齐」的少数几处才嵌入原文（见下表下方）。

| 组 | 规格章节 | 判据 | 门禁 | 本清单条目 |
|---|---|---|---|---|
| T1-g 契约与登记表 | §3.1.1 / §3.10「可选功能分区自注册」1–8 / §3.11.1 | V19 / V21 / V22 / V24 | G1 / G4 / G16 / G17 / G19 | R-01 – R-03、R-05 |
| T1-g 装配与取数 | §3.10 取用第 5–6 条 + 并行超时 / §3.11.2 / §3.11.3 | V13 / V20 / V23 | G16 / G18(d)(f) / G19(a) | R-04、R-08 – R-11 |
| T1-g 测试改写 | §10 | V13 / V19 / V20 / V21–V24 | G15–G19 | R-12 – R-15 |
| T1 遗留 | §3.1 `E12` 行 / §3.1 `E7`·`E9` 行 / §7.1 | V2 / V4 / V13 | G6 / G8 / G13 | R-16 – R-17 |
| 散文面收口 | §3.2（`note` 约束）/ §6.2（`message`）/ §7.1 | V4 / V14 / V15 | G6 / G12 / G13 | R-39、R-40 |
| T4 前端 | 前端设计 §3 / §5 / §7 / §11 / §13；规格 §5 / §6.1 / §6.2 / §7 / §10（§6.3 已作废） | V14 / V15 / V16 / V17 / V20 | G10 / G13 / G14 | R-19 – R-31 |
| T4 收尾 | §10「整体验收」/ §11「切片依赖」/ §12 迁移清单 | V14–V17 / V21–V24 | G1–G19 逐条签字 | R-32 – R-38 |

**必须给出原文的三处**（不能只给指针；**均不做逐字断言**，形态约束见 R-39；其中两处原清单与规格冲突，已按规格修正）：

- `E12` 的缺口说明 → `reclaim.note`：规格 §3.1 的 `E12` 行与 §3.10 的 `E12` 行均写「**该组件不提供占用统计**」（`E12` 在 §3.10 表中是倒数第二行，末行是 `E13`）。`availabilityReason` 字段已按 C 方案删除（R-16），该说明改由 `note` 承载；原清单第 90 行写的「`tauri-plugin-log` 不暴露 stats」出自 §7 现状矩阵的缺口说明列，且含插件名（§3.2 明令禁止），**以规格为准**。
- `notImplemented` 的 `note`：原清单的「缺少 owner 方法」出自规格 §6.3，而 §6.3 已整体划出作废；前端设计 §5.7 / §13 要求显示**后端 `note` 原文**，不写死固定文案（见 R-22）。
- `owner` 命名域：`E7` / `E8` / `E11` = `SecurityDb`、`E9` = `SecurityRepo`、`E10` = `SecurityRecovery`；判据三条——不含 `::`、不等于任何 feature 名、不是可选能力 crate 的类型名（V22③ 锁死，见 R-05 / R-06）。

**为什么不再逐字锁文案**：C 方案下 wire 只剩 `availability` 枚举承载状态，前端按已有中英词表渲染，文案不随环境变化也不需要防漂移断言；`note` / `message` 是诊断载荷，用 R-39 的形态断言（非空 / 长度 / 禁词 / 透传）约束即可。逐字断言会让「改措辞」与「改契约」耦合，收益为零。

---

## 一、T1-g 修订 — 可选功能分区自管理（15 项）

> 规格依据：I61 §3.1.1 / §3.10「可选功能分区自注册」/ §3.8 G16–G19 / §3.11.1 登记纪律 2–6 / V21–V24。
> 旧口径（`SECURITY_OBSERVABILITY` 能力名、`RootMount::Feature` 行、`ProbeRegistry`、`Feature(capability)` 行）随本组作废，测试中对应旧断言一并删除（见 `R-15`）。
> 已完成 2 项（`cmd/storage/section.rs` 契约、`SectionRegistry::register` 校验）不在本清单重复。

- [ ] **R-01** `src-tauri/src/cmd/storage/mod.rs`：`mod section;` 引入新模块；删除 `pub(crate) mod probe;`，`cmd/storage/probe.rs` 整体移除（旧 trait / 注册表退役，不留双份契约）
  - 验收：全仓检索 `StorageOwnerProbe` / `ProbeRegistry` / `StorageOwnerProbeHandle` 零命中
  - 现状（2026-11-21，部分完成）：`pub mod section;` 已加；`pub(crate) mod probe;` 仍在（`sample` / `inventory` / `reclaim` / `plugins` / `vault_plugin` 尚未切换，删早了会整片编译不过）——与 `R-03`–`R-11` 必须成组落地
- [ ] **R-02** `src-tauri/src/app_dirs/registry.rs`：`MANAGED_ROOTS` 删 `E7`–`E11` 五行（收敛为 8 条：`E1`–`E6` / `E12` / `E13`）；删除 `SECURITY_OBSERVABILITY` 与 `RootMount`（`Feature` 分支随条目一起消失，只剩 `Always` 时枚举无意义）；新增 `pub const OPTIONAL_SEGMENTS: &[&str] = &["security"];`（**只写首段**，不写条目 / 策略 / 实现）；`validate_registry` 改为校验 8 条 core 行 + `OPTIONAL_SEGMENTS` 非空、不含 `::`、不等于 feature 名 / 实现类型名（G16 / G19 / V19 / V24）
  - 验收：`app_dirs_contract` 绿；`registry.rs` 内检索 `security/`（带子段）零命中；`OPTIONAL_SEGMENTS` 每项都是单段且无大写
  - 现状（2026-11-21，部分完成）：`OPTIONAL_SEGMENTS` 已新增并附语义边界注释（只写首段，条目 / 策略 / 实现留给功能 crate）；`MANAGED_ROOTS` 的 `E7`–`E11` 五行、`SECURITY_OBSERVABILITY`、`RootMount`、`validate_registry` 的 13 条口径均**未改**——它们被 `section.rs` 的 core 重名校验引用，须与 `R-03`–`R-11` 同批切换，否则注册校验失去对照表
- [ ] **R-03** `src-tauri/src/plugins.rs`：`manage(Arc::new(ProbeRegistry::new()))` 改 `manage(Arc::new(SectionRegistry::new()))`——仍在 `Builder` 阶段、先于 `feature_mounts::on_setup`（G18(b)，建槽与注册不得跨层）
  - 验收：槽类型与 `sample.rs` / `inventory.rs` / `reclaim.rs` 的 `try_state::<Arc<SectionRegistry>>()` 同源
- [ ] **R-04** `src-tauri/src/feature_mounts.rs`：`on_setup` 钩子改为调用各功能装配缝导出的注册入口（当前只有 vault）；取不到槽或 `register` 返回 `Err` 时只 `log::error!` 后继续（不 panic、不覆盖、不静默降级成占位行）
  - 验收：vaultless 构建下 `on_setup` 不注册任何分区，`entries` 只剩 core 8 条
- [ ] **R-05** `src-tauri/crates/assistant-vault/src/storage_section.rs`（新建）：用功能**自有普通类型**自述 `E7`–`E11`——`VaultStorageEntry` 清单（`entry` / `key` / `segments` / `scope` / `coverage` / `owner` / `inventory` / `decl`）+ `VaultStats` / `VaultSectionError`；`owner` 用 `SecurityDb` / `SecurityRepo` / `SecurityRecovery`，动作 / 确认档 / `autoActive` / `note` / 策略 / 资格判据 / 回收入口全部在此；**不依赖 app crate**（§3.1.1 依赖方向）
  - 验收：`cargo test -p assistant-vault` 绿；crate 依赖图里无 `assistant-app`
- [ ] **R-06** `src-tauri/src/vault_plugin.rs`：`#[cfg(feature = "vault")] mod mounted` 提供 `StorageSection` 适配器——`id()` 返回分区标识（只用于日志 / 去重）、`entries()` 转发功能侧清单、`stats(key)` / `eligibility` / `policy` / `reclaim_actions` / `reclaim` 按 `key` 分派到功能侧并**无损映射**成 core 类型；适配器内**不出现**条目 `key` / `security` 字面量（G17(b) / V21 / V24）
  - 验收：故意漏搬 `newestAt` 的负例被 V21 判红；适配器源码检索 `security/` 零命中
- [ ] **R-07** `src-tauri/src/vault_plugin.rs`：删除 `VaultStorageProbe` / `StorageOwnerProbe` 实现与 `ProbeRegistry` 注册；改为导出注册入口（`fn register_storage_section(app: &AppHandle)`）供 `feature_mounts::on_setup` 调用，分区实例由 app 侧 `SectionRegistry` 接收，插件不 `manage` 独立 state（G18(a)(d)）
  - 验收：`vault_plugin.rs` 内 `app.manage` 零命中；注册入口只在 `on_setup` 被调用
- [ ] **R-08** `src-tauri/src/cmd/storage/sample.rs`：条目清单改为「core 8 条（登记表）+ 各分区 `entries()` 快照」；采样按 `key` → 所属分区索引取数（分区条目走 `stats(key)` + `eligibility` / `policy`，core 条目走既有入口）；分区 stats 同步阻塞，统一在 `spawn_blocking` 里调用并套同一套 **500 ms** 超时（超时只放弃等待、不取消）；删除 `RootMount::Feature` 分支、`probe_sample_for`、`unmounted_probe_sample`——**未挂载 = 该行不存在**，不是 `notImplemented` 占位（G18(f) / §3.10 第 5–6 条）
  - 验收：`sample.rs` 内 `notImplemented` 只剩 `E12` 一处；分区条目零硬编码
  - 同时关闭 `R-17`（原 T1-d `B`）
- [ ] **R-09** `src-tauri/src/cmd/storage/entries.rs`：`ENTRY_COUNT = 13` 与 `static_for` 的 `E7`–`E11` 分支删除；静态档只保留 core 8 条；分区条目的 `action` / 静态 `eligibility` / `reason` / `policy` / `note` 一律从 `SectionEntry::decl` + 分区 `eligibility` / `policy` 取，core 不写功能条目 `key` / `entry` 字面量（G17(b) / V22④）；`complete` 改为「除 `E12` 外 `entries` 中全部 `availability ∈ {available, missing}`」（条目数动态、未挂载分区条目不参与）
  - 验收：`entries.rs` 检索 `security` / `E7`–`E11` 零命中；`assemble_breakdown` 入参改为动态长度（不再 `&[EntrySample; 13]`）
- [ ] **R-10** `src-tauri/src/cmd/storage/inventory.rs`：挂载判定从「`ProbeRegistry::capabilities()` 含能力名」改为「首段 ∈ `OPTIONAL_SEGMENTS` 且**无**已注册分区声明该首段 → `optionalNotMounted`；有分区声明 → 按分区 `entries()` 的 `segments` / `coverage` 做减法归 `declared`」；`E7`–`E11` 的 `declaredBytes` 只来自分区条目（G16 / G18(d) / §3.11.3）
  - 验收：零分区时 `security` 归 `optionalNotMounted`（不是 `unregistered`）；单分区时与改造前逐字段等价
- [ ] **R-11** `src-tauri/src/cmd/storage/reclaim.rs`：可执行动作校验从「core 硬编码 `E1`–`E8` 白名单」改为「core 8 条动作表 + 分区 `reclaim_actions()` ∪ 按 `key` 调分区 `reclaim`」；`E8` 的 `pruneToLimit` 只由分区自述，core 不出现 `security/backups` 字面量；vaultless 构建下该 `key` 不存在 → 请求按未知 key 拒绝（G17(b) / V23⑤）
  - 验收：`reclaim.rs` 检索 `security` 零命中；`E8 + A4` 在 vault 构建放行、vaultless 构建拒绝且零副作用
- [ ] **R-12** `src-tauri/tests/storage_breakdown.rs`：golden 改为两种构建分别断言——`vault` 13 行、`vaultless` 8 行；`vaultless` 断言 `security` 组**整体不存在**（不得出现 `notImplemented` 占位行），core 条目逐字段与 `vault` 构建一致（V13 / V23①⑤）
  - 验收：删掉 `unmounted_probe_keeps_e7_to_e11_visible_and_not_reclaimable`，改为「未挂载 = 行缺失」用例
  - 现状（2026-11-21）：文件内重复的 `file_link` 辅助函数已删除，改为 `#[path = "../crates/assistant-tools/tests/support/link.rs"] mod link;` 引入全仓唯一实现；golden 断言本身未改，仍待本条正式改写
- [ ] **R-13** `src-tauri/tests/storage_inventory.rs`：`optionalNotMounted` 断言改为按 `OPTIONAL_SEGMENTS` 首段判定（不再依赖能力名注册）；覆盖「槽缺失 / 零分区 / 单分区 / 双分区」四态（V19 / V23）
  - 验收：注册一个首段不在 `OPTIONAL_SEGMENTS` 的假分区 → `register` 拒绝且 breakdown 不出现该行
- [ ] **R-14** `src-tauri/tests/storage_reclaim.rs`：`entryKeys` 空 = core 8 条 + 已挂载分区条目（不再写 13）；白名单用例改为分区自述动作驱动；请求已退役分区的 `key` → 按未知 key 拒绝（V20 / V23）
  - 验收：上述两组在 `vault` / `vaultless` 各跑一遍
- [ ] **R-15** 验收：V21–V24 按新口径落地（口径四元组一致 / 分区自述与 core 事实一致 / 通用槽四态 / core 不感知具体功能），并把 `T1-c 修订` 的 V22–V24 旧断言（能力名 `SECURITY_OBSERVABILITY`、`ProbeRegistry`、`Feature(capability)` 行）在测试中删除

**随本组同步作废 / 改写的旧断言**（改写时按新口径，不保留双份）：

| 位置 | 旧断言 | 新口径 |
|---|---|---|
| T1-b 标题 / 遍历口径 | 「13 条共用」 | 口径本身不变，条目数不再是常量 13（`vault` 13 / `vaultless` 8） |
| T1-d 装配 | 「13 条按 `E1`→`E13` 固定顺序」、`ENTRY_COUNT`、`&[EntrySample; 13]` | core 8 条固定 + 分区条目按 `entries()` 快照顺序 |
| T1-e 测试 | 「两种构建 `entries` 恒 13 条」 | 两种构建分别断言 13 / 8，`vaultless` 无占位行 |
| T1-f 对账 | 「13 条登记」、`subtract_declared(&RawNode, &[EntrySample; ENTRY_COUNT])` | 8 条 core + 已挂载分区条目（长度动态），`optionalNotMounted` 按首段判 |
| T2-a 计划 | 「`entryKeys` 空 = 全部 13 条」 | core 8 条 + 已挂载分区条目 |
| T3-b `E8` | `StorageOwnerProbe::prune_backups`、core 侧 `E8 + A4` 白名单、vaultless 时 `E8` 为 `notImplemented` | 分区 `reclaim_actions` / `reclaim` 自述，core 无 `security/backups` 字面量；vaultless 时该行不存在 |
| T4-b vaultless 回归 | 「`entries` 恒 13 条；`E7`–`E11` 为 `notImplemented`」 | `entries` 8 条，安全域整组不出现 |
| 门禁 G10 | 「不把 13 条相加」 | 「不把 core 8 条 + 已挂载分区条目相加」 |

---

## 二、T1 遗留收尾（2 项）

> 原 7 项中 A-1 / D-1 / D-2 / D-3 已于 2026-11-21 清零（见原清单 T1-h 表）；剩余 2 项都与 T1-g 改同一处代码。
> 原 `R-18`（`locked` 文案一致性）随 C 方案**作废**：`locked` 由 `availability` 枚举表达，wire 不再有原因文案字段，该项要防的「文案不一致」已不存在，编号不复用。

- [ ] **R-16** 删除 `availabilityReason` 字段（原 T1-b `E12` 文案条目 / T1-h `A-2`，两处同一件事；方案 C：原因文案不上前端）
  - **决策**：wire **不再承载**可用性原因文案。可用性只由封闭枚举 `availability`（8 值，前端已有完整中英词表，见 `zh-CN.json` 的 `entries.availability.*`）表达，前端回退到 `availabilityText(availability)`；这样 en-US 界面不会再出现中文诊断，也不必为自由文本写逐字断言
  - **后端**：`src-tauri/crates/assistant-protocol/src/storage.rs` 删 `StorageEntryDto::availability_reason`；`entries.rs` 删该字段与 `EntrySample::degraded` 的 `reason` 形参（签名收敛为 `degraded(availability)`）、`availability_reason: None` 初始化；`sample.rs` 删 `io_reason()`（IO 错误细节只进日志，不上 wire）；`section.rs` 删 `SectionError::reason()`（`availability()` 保留，错误细节由分区写日志）
  - **契约**：`npm run contract:gen`，确认 `src/types/protocol.contract.ts` 的 `StorageEntryDto` 不再含 `availabilityReason`（生成物随改动一起提交）
  - **前端**：`src/settings/pages/storage/entryView.ts` 的角标 `reason` 改用 `availabilityText`；`EntryDetail.tsx` 删「状态说明」行；`fixtures.ts` / `entryView.test.ts` 删该字段与相关断言；`zh-CN.json` / `en-US.json` 删 `entries.facts.availability_reason`
  - **`E12` 说明去处**：缺口说明改由 `reclaim.note` 承载（`note` 本就是规格认可的结构性缺口通道，§3.2 允许「暂未提供清理入口」这类结构性说明），取值「**该组件不提供占用统计**」；详情「诊断」行仍显示 `note` 原文（前端设计 §5.9）
  - **不再逐字锁任何文案**：`E12` 断言改为「`availability = notImplemented` + `note` 非空」，文案形态由 `R-39` 约束
  - 验收：全仓检索 `availability_reason` / `availabilityReason` 零命中（`docs/` 里的历史记录与规格修订除外）；`tests/storage_breakdown.rs` 原「降级必须给原因」的断言改为断言 `availability` 属于异常集合；把 `note` 改成任意别的措辞**不判红**（形态断言仍应通过），删掉 `note` 判红
- [ ] **R-17** `src-tauri/src/cmd/storage/sample.rs`（原 T1-d `B`）：`E7`–`E11` 走 `timed_probe`，其 `call()` 在 async future 内联同步执行、未走 `spawn_blocking`——分区采样统一进 `spawn_blocking` 后满足「单条 500 ms 超时」口径
  - 验收：沿用原 T1-d 条目（同步 owner 一律 `spawn_blocking` + 对 `JoinHandle` 超时；超时只放弃等待、不取消）
  - 关闭方式：随 `R-08` 一起落地并勾选

---

## 三、T4-a 数据与交互（13 项）

> 前置：I54 `R0` 已交付（页面容器 + 三 tab 挂载位）。R0 未开工前，只能做与容器无关的组件内部改造，**不得**自行新建第二个页面。
> 前端实现基线：[I61-storage-manager-frontend.md](./I61-storage-manager-frontend.md)；规格 §6.3 已整体划出作废，判据 / 字段语义 / 资格与档位仍以规格 §3 / §5 / §6.1 / §6.2 / §7 / §10 为准。

- [ ] **R-19** `src/services/desktop/storageService.ts`：**扩展现有 service**（不新建 store、不新建 service）——加 `getStorageReclaimPlan(entryKeys)` 与 `reclaimStorage(planId, entries, confirmed)`，以及 `refreshStorageInventory()`（返回 `StorageInventoryStatusDto`，只取 `status`，**不取数字**）
- [ ] **R-20** `src/settings/pages/StorageSettings.tsx`：**保留附件明细区全部既有能力**（会话分组 / 孤儿分组 / 折叠 / 组全选 / 勾选删除，服务 `A2`），在其后**追加**已知条目清理区块（4 列：名称 / 占用·条数 / 策略 / 清理；条目数、分组与行序**完全由后端 `entries` 驱动**，前端不假设 13 条、不补占位行、不写死条目名或分区名；未挂载的可选分区（如安全域）整组不渲染，G14 / V16）
- [ ] **R-21** `src/settings/pages/StorageSettings.tsx`：条目行按前端设计 §5.4 / §5.6 / §5.7 渲染（可用性 = 占用列角标（tooltip 用 `availabilityText`）+ 详情「诊断」行（只显示 `note`，`availabilityReason` 已按 `R-16` 删除），资格 = 清理按钮可用态，不单独成列）；`bytes = null` 显示占位符（**不得**直接传 `formatBytes`，N17），不改 `src/utils/formatBytes.ts` 的签名
- [ ] **R-22** `src/settings/pages/StorageSettings.tsx`：不可回收条目按钮禁用并显示 `reason` 文案；`notImplemented` 且后端给了 `note` 时显示 **`note` 原文**（前端设计 §5.7 / §13），不写死「缺少 owner 方法」这类固定文案（那句出自已作废的规格 §6.3）
- [ ] **R-23** `src/settings/pages/StorageSettings.tsx`：回收流程固定为 `get_storage_reclaim_plan` → 确认（`strongConfirm` 需二次输入条目名或等价强交互）→ `reclaim_storage` → 结果 toast → 重新 `get_storage_breakdown`
- [ ] **R-24** `src/settings/pages/StorageSettings.tsx`：**禁止**前端自算「可回收字节 / 是否安全」，**禁止**按 `expectedBytes` 自行降档确认强度（档位只信后端 `confirmation`）
- [ ] **R-25** `src/settings/pages/StorageSettings.tsx`：`planExpired` / `planNotFound` / `busy` 提示重试，**不自动重放**；`complete = false` 时不展示跨条目合计（G10）
- [ ] **R-26** `src/settings/pages/StorageSettings.tsx`：条目清理区块**不**提供第二套「删指定附件」入口，也**不得**把明细区删除按钮搬进条目表
- [ ] **R-27** `src/settings/pages/StorageSettings.tsx`：**第二段只读余量区（未登记占用）**（`breakdown.inventory`）——五态渲染、`partial` / `overRegistered` 提示、列示范围与 `declared` 折叠条件全部按前端设计 §7；**禁止**任何删除 / 清理按钮，**禁止**把余量数字加进页面任何合计（G10）
- [ ] **R-28** `src/settings/pages/StorageSettings.tsx`：`useEffect` 内 `listen("storage:inventory-updated", ...)`（与 `src/vault/core/hooks/useVaultEvents.ts` 同款写法，清理函数里调 `UnlistenFn`），收到后**只重拉一次** `getStorageBreakdown`，**不新增轮询**；「重新扫描」按钮调 `refreshStorageInventory()` 后按返回的 `status` 刷新按钮态
- [ ] **R-29** `src/settings/styles/settings.css`：新增条目表样式（与既有 `storage-*` 类名同族，不重命名既有类）＋ 余量区样式（含 `overRegistered` 警示态）
- [ ] **R-30** `src/i18n/locales/zh-CN.json` / `en-US.json`：新增 `availability` / `eligibility` / `reason` 词表、策略摘要、确认文案（`strongConfirm` 必须含条目 / 数量 / 字节三件套）、结果 toast 文案，以及余量区文案（按前端设计 §11 键树；`residual.hint` 必须覆盖 `declared` 节点，不能写成「不属于任何条目」）；条目名是**可选覆盖层**（`entries.names.<key 规范化>`，未命中回退 `key` 原文，不要求穷举、不随挂载状态变化），**不写死分组标题 / 行序**；两份 key 结构必须一致
- [ ] **R-31** 组件测试（按前端设计 §13 测试矩阵）：`entries` 13 条（全挂载）/ 8 条（未挂载，安全域整组不出现）/ 空 `entries` 三组夹具各一例；断言行数与分组完全来自夹具，不出现占位行、不写死条目名
  - 验收：把夹具从 13 条改成 8 条，组件渲染行数随之变化且无残留分组标题

---

## 四、T4-b 收尾（7 项）

- [ ] **R-32** `npm run contract:gen` 后确认 `src/types/protocol.contract.ts` 无手改、无残留
- [ ] **R-33** 留痕复核：`execute` 日志只含 `planId` / key / action / 计数 / `sampledAt`（V15）
- [ ] **R-34** 全量回归：`cargo check --workspace --all-targets`、`cargo test -p assistant-app --lib`、`cargo test -p assistant-storage`、`cargo test -p assistant-tools`、`cargo test --test storage_breakdown`、`cargo test --test storage_inventory`、`cargo test --test storage_reclaim`、`cargo test --test protocol_bindings`、`cargo test --test app_dirs_contract`、`cargo test --test assembly_contract`
- [ ] **R-35** vaultless 构建再跑一遍：`cargo test --no-default-features --test storage_breakdown`（`entries` **8 条**，安全域 `E7`–`E11` 整组不出现，**不得**出现 `notImplemented` 占位行；`security` 首段在 `inventory` 归 `optionalNotMounted`，不是 `unregistered`，见 `R-02` / `R-10`）
- [ ] **R-36** 前端：`npm run typecheck`、`npm run lint`、`npm run test`
- [ ] **R-37** 附件明细区能力逐项回归（V16 的手动清单）：会话分组 / 孤儿分组 / 折叠 / 组全选 / 单选删除 / 删除结果提示
- [ ] **R-38** I61 §12 迁移清单复核：I54 中存储 tab 的残留条目已全部改为指针，且**未**删除任何共享 DTO 旧字段（`availabilityReason` 是 I61 新增字段、按 `R-16` 删除，不属于「旧字段」，两者不冲突）

---

## 五、散文面收口（2 项）

> 背景：`availabilityReason` 已按 C 方案从 wire 删除（`R-16`），wire 上除标识符外只剩**两处自由文本（散文）**，其余状态语义全部是封闭枚举（§6.2 列出的 11 个枚举，前端按枚举分支渲染、不做字符串兜底）。这两处**不能**枚举化——`note` 是结构性能力缺口说明、`message` 是 OS 错误细节，都无法穷举；但也**不能**靠逐字断言，只能做形态断言。
> 规格依据：§3.2（`note` 字段约束）/ §6.2（`StorageReclaimErrorDto`）/ §7.1；判据 V4 / V14 / V15；门禁 G6 / G12 / G13。

- [ ] **R-39** 两处散文字段收口（`note` / `message`）：范围与约束固定如下，代码改动分散在 `R-09`（`entries`）/ `R-11`（`reclaim`）产出的同一批改动里，本条负责断言与既成事实整改
  - **范围**（仅此两处是散文，其余都是枚举或标识符）：`ReclaimViewDto.note`（`String | null`，≤120 字符）、`StorageReclaimErrorDto.message`（人类可读）。`key` / `owner` / `planId` / `truncatedNodes` 是标识符（单段名 / 不透明串，不承载状态语义），**不得**当作散文处理，也**不得**把状态语义塞进它们；`availabilityReason` 已删除，不在本范围
  - **内容约束**（§3.2 / §6.2）：`note` 只写结构性、不随配置与版本变化的能力缺口，禁策略数值（上限 / 份数 / 天数）/ 路径 / 时间点 / 临时错误 / 内部方法名 / 插件名；`message` 不含绝对路径与文件名，`errors` 最多 5 条
  - **形态断言**（不逐字比对）：① 必填态与 `null` 态对齐——`eligibility = notImplemented` 的 `note` 非空（`E12` 的 `note` 即「该组件不提供占用统计」），其余条目可 `null`；`notEligible` / `notImplemented` / `locked` / `ownerManaged` 的 `reason` 非空；② 长度上限（`note` ≤120）；③ 禁词正则（`::`、`\`、`/`、盘符、`ProbeError` / `Locked` / `probe` / `AttachmentStore` 等内部名与插件名）；④ 透传不变形——后端写入的字符串经 DTO 到前端展示逐字相同，前端除截断外不做匹配 / 翻译 / 改写
  - **既成事实整改**：原 T1-d 要求 `note` 逐字点名缺的方法（`AttachmentStore::delete_orphans_by_ids`），与 §3.2「禁内部方法名」冲突 → 改为「暂未提供清理入口」这类结构性说明（缺口本身仍要用户能读懂）
  - **验收**：契约测试扫描 DTO，断言除上述两字段外无 `String` 承载状态语义；两字段各跑一遍形态断言；**不设任何逐字断言**（`E12` 的 `note` 也只走形态断言，见 `R-16`）
- [ ] **R-40** 规格与前端设计同步修订（C 方案的文档面，与 `R-16` **同一批**提交，先改文档再改代码）
  - **规格**：§3.2 删 `availabilityReason` 行；§3.1 `E12` 行、§3.10 `E12` 行把缺口说明移到 `reclaim.note`；§3.4 `noStats` 行删「`availabilityReason` 用自由文本说明」改为 `note`；§7.1 的 L939 / L940 / L941 / L954 删文案要求（保留「`locked` 与 `unavailable` 用户下一步不同」的**理由**——现在由枚举 + 前端 `availabilityText` 承担）；§10 V2 补一句「`availability` 是唯一可用性载体，wire 不再有原因文案」
  - **前端设计**：§3 字段表删 `availabilityReason`（L283）；§5.6 角标 tooltip 只用 `availabilityText`（L324 / L367 / L375）；硬规则 1 收窄到只剩 `note`（L386）；详情「诊断」行只剩 `note`（L431）；§11 键树删 `availability_reason`（L736）；§11 语言规则删该字段（L778）；§13 测试矩阵同步
  - **记录取舍**：`note` / `message` 仍是后端原文（含语言），en-US 界面会看到中文诊断——这是本方案接受的代价，写进规格「取舍」段，避免被当成后续 bug；`availability` 枚举本身已覆盖状态语义，用户「下一步怎么做」靠枚举 + 前端文案表达，不再依赖后端散文
  - 验收：`docs/store` 内检索 `availabilityReason` 仅剩规格修订记录与 `R-16` 的历史说明；规格 / 前端设计的 V2 / V4 / §5.6 / §5.9 / §11 / §13 与代码一致
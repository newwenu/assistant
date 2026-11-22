# I67 — 存储对账统计完整性规格（基根总量 + 全类余量合计）

| 项 | 值 |
|---|---|
| 状态 | 定稿（待按 §11 切片实施；`E12` 相关口径已按 E12 全链路修订复核，见 §2 N13 / §3.5 / §12；同日修订 §13 登记判据：准入只看「有没有 owner」，登记后必须上报大小（R1），四个配置文件改为登记，见 §13.2.2 / §13.4 / T6；同日修订归属：登记表只记录归属、不造常量——`EBWebView` 作为受管目录由 `dir_name::WEBVIEW_PROFILE` 持有（落点与 Tauri 默认逐字相同、零迁移），配置名引用 owner 的 `ConfGroup::FILE_NAME`，见 §13.3 / N16；同日修订 `E16` 归属：`vault-config.json` 不再由 core 登记，改由 vault 特性在可选分区内自注册为 `security/vault-config.json`（落点 `<data>/security/`，core 侧零知识、零字面量、不做旧位置迁移），见 §13.4 / T6；同日修订 `E14` 定性：由「只观测、不接管」改为「应用接管落点声明 / 目录创建 / 回收时机 / 观测」，落点仍与 Tauri 强制默认逐字相同、零迁移，见 §13.3 接管表 / T5；同日新增 §3.8：锁定不构成观测豁免，`E9` 锁定态仍须上报大小、锁的信息只走资格侧，见 N17 / T7 / V16） |
| 日期 | 2026-11-22（同日复核：breakdown 侧 `E12` 占用已落地，对账侧仍待 §11 T1；同日修订 §13 登记判据与切片 T6；同日新增 §3.8 观测与资格解耦、切片 T7） |
| 关联 | I61 存储管理器规格（已归档）（§3.11 对账余量的**事实基线**，本文只做增量修订，不复述其分类与减法）、I61 前端设计（已归档）（前端唯一实现基线，本文只给增量条款）、[I54 资源监视器规格](../../store/I54-resource-monitor-spec.md)（存储侧已迁出，本文不涉及进程 / 自身占用） |
| 影响范围 | 后端：`src-tauri/src/cmd/storage/inventory/`（`walk.rs` 累计、`cache.rs` 汇总、`mod.rs` 快照结构）、`src-tauri/crates/assistant-protocol/src/storage.rs`（DTO 扩展）、`src-tauri/src/cmd/storage/entries/`（`E` 编号常量收敛、静态档）、`src-tauri/src/app_dirs/dir_name.rs`（新增 `WEBVIEW_PROFILE`）、`src-tauri/src/app_dirs/registry.rs`（登记表：`E14` / `E15` / `E17` / `E18`，首段引用 `dir_name` / owner 常量）、`src-tauri/src/app_dirs/roots.rs`（按登记表拼路径，不持字面量；`E14` 的 `webview_data_directory` / `webview_profile` / `ensure_webview_profile`）、`src-tauri/src/window.rs`（编程创建窗口显式声明 `data_directory`）、`src-tauri/src/main.rs` / `src-tauri/src/setup.rs`（`E14` 建窗前创建目录与建窗后漂移守卫）、`src-tauri/src/cmd/storage/sample/`（配置条目采样与分派）、`src-tauri/crates/assistant-vault/src/storage_section.rs`（`E9` 锁定态观测解耦（T7）与 `E16` 条目自注册（T6））、`src-tauri/src/vault_plugin.rs`（`E16` 配置文件路径注入）；前端：`src/settings/pages/storage/residualView.ts`、`src/settings/pages/storage/lexicon.ts`、`src/features/vault/module.ts`（`E16` 条目名称自注册）、`src/i18n/locales/*.json`、`src/types/protocol.contract.ts`（生成物） |

> 一句话：**分母必须存在**——对账余量现在只有「未登记」这一个分子，没有「受管目录一共多大」这个分母，也没有「所有类别余量之和」这个可闭合的分子；本规格补齐三者并规定：统计**不许丢弃**任何盘上字节，不知道的部分**必须显式标成下界**，而不是悄悄少算。

本文中的「必须 / 禁止」为可验收约束；「应当」为默认做法，偏离需在代码注释里写明理由。

**编号约定**：`N1–N17` 现状事实、`§5` 新增字段（不另编号）、`G1–G12` 门禁、`V1–V16` 验收判据、`T1–T7` 实施切片、`X1–X9` 拒绝方案、`OQ1–OQ8` 开放问题。本文与 I61（已归档） 的编号**同名独立**；跨文档引用一律带文档前缀（`I61 §3.11.3`），裸写 `§` 一律指本文。`E*` / `key` / `class` / `node` / `scope` 等术语沿用 I61 定义，本文不重复定义。

---

## 1 定位与边界

### 1.1 本规格只回答一个问题

I61 §3.11 定义的对账余量回答「**有没有**没被条目认领的东西」。它答不了另外两个问题：

1. **受管目录一共多大**（分母）。没有分母，界面上的「未登记 682.1 MB」无法判断是「几乎全都没认领」还是「总量 50 GB 里只有 682 MB 漏了」；
2. **所有类别的余量一共多少**（完整分子）。现有合计只覆盖 `unregistered` 一类，`declared` 差额、`companion`、`ignored`、`optionalNotMounted` 四类看得见、算不进。

本文补齐这两个量，并把「盘上确实有、但无法命名所以不进节点」的那部分字节显式上报，使 `Σ 节点 + 不可命名 == 基根总量` 成为可验证的守恒式。

### 1.2 与 I61 的关系

**增量修订，不是替代**：

| 项 | 归属 |
|---|---|
| 扫描范围、有界策略、首层分类、减法公式、状态机、只读约束 | I61 §3.11（本文不改） |
| 节点级字段语义（`scanBytes` / `declaredBytes` / `residualBytes` / `overRegistered`） | I61 §6.2（本文只对 `E12` 一处修订，§3.5） |
| `E12` 的**条目占用**（breakdown 侧：应用侧只读遍历、`reclaim.note = null`、策略数值只由 `policy` 表达） | I61 §3.2 / §3.10（本文**不改**，现状见 §2 N13） |
| `E12` 的**对账观测**（inventory 侧：`scanBytes` 有值、不做减法） | **本文**（§3.5；I61 原文为恒 `null`） |
| `unregisteredBytes` / `unregisteredCount` 的语义与 `null` 传染 | I61 §6.2（本文**不改**，回归判据 V8） |
| 基根总量、全类余量合计、不可命名部分、下界标记 | **本文**（§3 / §5） |
| `E` 编号与 `key` 的身份边界 | **本文**（§4，I61 未成文） |
| 登记准入（有 owner 即可登记）与登记必报项 R1–R4、`E14` / `E15` / `E17` / `E18` 的新增，以及 `E16` 的 feature 侧自注册 | **本文**（§13，独立切片 T5 / T6） |

冲突时以本文 §3 / §5 / §4 为准；其余仍以 I61 为准。

### 1.3 纳入与排除

**纳入**：新增统计字段的计算、`null` / 下界语义、契约扩展、前端合计行增量、`E` 编号边界、`E9` 锁定态观测解耦（§3.8 / T7，缺陷修复，不改登记表）。

**排除**（明确不做，理由见 §9）：

1. 不新增「查总量」命令（拒绝方案 X7）；
2. 不改 `entries` / `totalBytes` / `complete`（I61 G10 / V20 不变）；
3. 不把 `data` / `local` 之外的新基根纳入（`AppDirs` 只有两个基根）；
4. 不改扫描的有界阈值（`MAX_DEPTH` / `MAX_ENTRIES` / `MAX_DURATION`），只增加一次 `E12` 的独立预算（§3.5）；
5. 不给 `unregistered` 之外的四类新增可回收能力（I61 G10 不变）；
6. 本规格正文（§3–§8、T1–T4）不改 `MANAGED_ROOTS`（T1–T4 范围内仍为 core 8 条；T5 / T6 落地后为 core 12 条，见 §13）、不改 `IGNORED_NAMES` / `OPTIONAL_SEGMENTS`；登记表变更全部放在 §13 的独立切片：`EBWebView`（`E14`，T5）与三个 core 配置文件（`E15` / `E17` / `E18`，T6）；`E16`（`vault-config.json`）由 vault 特性自注册、不进 core 登记表。

---

## 2 现状事实（以代码为准）

| 编号 | 事实 |
|---|---|
| N1 | `walk::scan` 只对**首层条目**建节点，根目录自身不参与（I61 §3.11.2 第 3 条「目录自身不计」），因此「基根总量」从未被计算，也无字段承载 |
| N2 | `cache::read` 只用 `sum_unregistered` 求和；该函数对非 `unregistered` 类 `continue`，因此合计**结构性**只覆盖一类 |
| N3 | `StorageInventoryDto` 只有 `unregisteredBytes` / `unregisteredCount` 两个合计字段，无总量、无全类合计 |
| N4 | 前端 `residualView.totalsOf` 直接读上述两字段，`null` → `—`，不做任何求和 |
| N5 | `walk::list_root` 对「名字不是合法 UTF-8」或「不是安全单段名」的首层条目 `continue`：既不建节点，也不进任何合计——**分母因此结构性偏小**，且无法从任何字段看出少了多少 |
| N6 | `build_node` 由 `reconciled()` 同时决定「是否 `observe`」，对 `inventory = false` 的首段（`E12`）直接写 `Observed { bytes: None, count: None }`，该节点 `scanBytes` / `scanCount` 恒 `null`（§3.5 落地后改为「观测一次、`declaredBytes` 置 `null`」） |
| N7 | 每个首层节点在 `observe` 内各取一份独立预算（`Budget::new()`），节点子树互不重叠，因此 `Σ 节点 scanBytes` 在数学上等于该基根总量（除 N5 丢弃项）——**可推导但未上报** |
| N8 | `partial` 目前 = 任一节点触界 ∨ 任一节点 `overRegistered`；它不表达「合计本身是不是完整值」（后者由 `totals.partial` 表达，§3.6）。前端消费已按 `truncatedNodes` 收窄到「真触界」，避免 `overRegistered` 冒充「只扫描了一部分目录」（§3.6.1） |
| N9 | `StorageInventoryNodeDto.entry` 是唯一跨 wire 的 `E` 编号；前端 `residualView.rowView` 不读它，`StorageEntryDto` 根本没有 `E` 字段 |
| N10 | 前端条目名称按 `key` 索引（`lexicon.ts` 的 `ENTRY_NAME_KEYS`），渲染路径不出现 `E*`（前端设计 D13） |
| N11 | 后端 `E*` 是裸字符串字面量，分散在登记表 `MANAGED_ROOTS`、`entries/decl.rs`、`entries/eligibility.rs`、`reclaim/execute.rs`、`reclaim/plan.rs`、`sample/collect.rs`、`entries/mod.rs` 七处 |
| N12 | 其中两处是**静默兜底**：`eligibility_for` 的 `_ => {}`（`E1` / `E3` 的动态资格会静默退化为静态值）、`reclaim/execute.rs` 的 `_ =>`（core 条目会被误路由进分区执行分支），二者都不会报错 |
| N13 | `E12` 的**条目占用**已由 breakdown 采样侧统计（`sample/rows.rs::sample_logs` → `app_dirs::fs_stats::tree_stats`，应用侧只读遍历；I61 §3.10），可用性由该遍历给出（`available` / `missing` / `unreadable`）、`reclaim.note = null`。它属于 breakdown 快照，与对账快照**不是同一次观测**，不可互相复用（§3.5 / X7） |
| N14 | Windows 上 `dirs::config_dir()` 与 `dirs::data_dir()` 都解析到 `%APPDATA%`，再 join 同一个 `APP_IDENTIFIER`（`json_config.rs::get_conf_dir` 与 `roots.rs::resolve`），因此 core 配置（`config.json` / `orb-config.json` / `mascot-config.json`）必然落在数据基根首层并被扫成 `unregistered`；Linux / macOS 两路径分离，不会出现。这是平台事实，不是设计失误。`vault-config.json` 已改落 `<data>/security/`（`VaultConf::get_conf_dir` 与 vault 分区首段对齐，见 §13.4），不再属于「基根首层未登记」这一类 |
| N15 | `EBWebView/` 是 WebView2（Chromium 内核）的用户数据目录，由 main / orb 两个窗口产生。**落点不是自由约定**：Tauri 运行时在 Windows / Linux 上强制把 `data_directory` 设为 `app_local_data_dir()`（`tauri-2.12.5/src/manager/webview.rs`：用户未指定时 `data_directory = Some(app_local_data_dir())`），该值即 `%LOCALAPPDATA%` + `APP_IDENTIFIER`（`com.assistant` 只是常量当前值，代码侧无字面量），与 `AppDirs::local()` 同值——后者是 `dirs::data_local_dir().join(APP_IDENTIFIER)`，而 `tauri.conf.json` 的 `identifier` 与 `APP_IDENTIFIER` 的相等已由 `tests/app_dirs_contract.rs::identifier_and_asset_scope_match_app_dirs` 钉住；WebView2 在该目录下固定创建 `EBWebView/`。因此应用侧**不改 `data_directory`、不改窗口创建、不做迁移**——落点与现状逐字相同，登记只是把这个已被 Tauri 固定的落点纳入统计。字节的 owner 是 WebView2 运行时（外部组件，与 `E12` 的 `tauri-plugin-log` 同形），目录名由应用拼路径时需要，故归 `dir_name`（§13.3）；其子项（`Default` / `BrowserMetrics` / `Crashpad` / `GrShaderCache` / `component_crx_cache` / `GPUPersistentCache` 等）是**同一父节点的展开明细，不与父节点相加**；前端 `CHILD_LIMIT = 20` 造成「还有 n 项未展开」 |
| N16 | 三个 core 配置文件名由各 owner 的 `ConfGroup::FILE_NAME` 持有（`AppConf` → `config.json`、`OrbConf` → `orb-config.json`、`MascotConf` → `mascot-config.json`）。**`app_dirs` 不持这些字面量**：它只持「`AppDirs` 拼路径」的目录名（如 `EBWebView`，§13.3），而配置文件的路径由 owner crate 自己用 `FILE_NAME` 拼；**登记表也不为它们造常量**——`E15` / `E17` / `E18` 的 `segments` 直接引用 owner 常量（`<AppConf as ConfGroup>::FILE_NAME` 等），字面量只有一份。`E16`（`vault-config.json`）**不归 core**：core 侧零知识、零字面量，由 vault 特性在自己的可选分区（`security/`）内声明为 `security/vault-config.json`，文件名同样只取自 `<VaultConf as ConfGroup>::FILE_NAME`（§13.4） |
| N17 | `E9`（`security/vault`）的 `repo_stats` 在锁定态直接 `return Err(VaultSectionError::Locked)`，**未走到 `tree_stats`**；而 `tree_stats` 只做 `read_dir` + `symlink_metadata` + `meta.len()`，不依赖解密、不开库。因此锁定态该条目 `bytes = null` → `security/` 节点 `declaredBytes` 整体降为 `null` → `residual = null` → `totals.partial = true`，且 `assemble` 的 `complete` 只接受 `available` / `missing`，整页 `complete = false`。同 crate 的 `E7` 已写明「只读 `metadata`：vault 锁定不影响它」，两处判据不一致（§3.8 / T7） |

N1–N4 共同解释了一个观感：界面上有「未登记合计 682.1 MB」，却没有「受管目录合计」，于是这个数字既不能当分子也不能当分母，只能孤零零地摆着。N14 / N15 则解释了这个 682.1 MB 具体是什么——处置记录见 §13。

---

## 3 统计口径（本文核心）

### 3.1 三个量，各答一问

| 量 | 回答的问题 | 字段 | 覆盖范围 |
|---|---|---|---|
| **基根总量**（分母） | 受管目录一共多大 | `inventory.totals.scanBytes` / `.scanCount` | 全部首层节点 + 不可命名部分 |
| **全类余量合计**（完整分子） | 没被条目认领的一共多少 | `inventory.totals.residualBytes` / `.residualCount` | 全部节点的 `residualBytes` 之和（含 `declared` 差额） |
| **未登记合计**（主信号） | 完全没人认领的多少 | `inventory.unregisteredBytes` / `.unregisteredCount`（**既有，语义不变**） | 仅 `class = unregistered` |

三者必须同时存在：只有分子没有分母，无法判断严重程度；只有主信号没有完整分子，四类余量永远「看得见、算不进」。

### 3.2 定义与公式

节点级公式沿用 I61 §3.11.3，不变：

```
node.residualBytes = node.scanBytes − node.declaredBytes
```

新增 scope 级与全基根级（`scope ∈ {data, local}`）：

```
scope.scanBytes        = Σ(node.scanBytes, 该 scope 内)  + scope.unnameableBytes
scope.scanCount        = Σ(node.scanCount, 该 scope 内)  + scope.unnameableCount
scope.declaredBytes    = Σ(node.declaredBytes, 该 scope 内)
scope.declaredCount    = Σ(node.declaredCount, 该 scope 内)
scope.residualBytes    = Σ(node.residualBytes, 该 scope 内)
scope.residualCount    = Σ(node.residualCount, 该 scope 内)
scope.unregisteredBytes = Σ(node.scanBytes, class = unregistered 且该 scope 内)
scope.unregisteredCount = Σ(node.scanCount, class = unregistered 且该 scope 内)
scope.unreconciledBytes = Σ(node.scanBytes, `inventory = false` 的 declared 节点且该 scope 内)
scope.unreconciledCount = Σ(node.scanCount, 同上)
scope.unnameableBytes  = 该 scope 首层「不可命名条目」的字节之和
scope.unnameableCount  = 同一次遍历计到的条数之和

totals.scanBytes     = data.scanBytes + local.scanBytes
totals.scanCount     = data.scanCount + local.scanCount
totals.residualBytes = data.residualBytes + local.residualBytes
totals.residualCount = data.residualCount + local.residualCount
```

**口径要点**：

1. `declared` / `residual` 的 scope 求和与既有节点字段**同源**，不重新遍历、不重新算减法——只在 `cache::read` 里对已经算好的 `node_dto` 结果累加（X1 拒绝为合计再扫一遍）；
2. `unregisteredBytes` 的 scope 级求和**只用于分 scope 展示**；顶层 `inventory.unregisteredBytes` 保持既有定义（全基根、仅 `unregistered`），两者数值必须一致（V6）；
3. `totals.scanBytes` 由后端给出，**禁止**前端把 `data` / `local` 相加（G5）；
4. `unreconciled` 是 `inventory = false` 首段（`E12`）的扫描值：**计入分母、不参与减法**（§3.5）。它必须单列，不能并进 `residualBytes`——否则日志会被误报成「没被认领」，也不能丢弃——否则分母缺一块。判定「哪些节点属于这一类」只能读登记条目的 `inventory` 标志（按 `node.entry` 回查登记表），**不得**匹配 `E` 字面量（§4.3）。

### 3.3 守恒式（必须可验证）

各项均非 `null` 时，每个 scope 必须严格满足：

```
scope.scanBytes == scope.declaredBytes + scope.residualBytes
                 + scope.unreconciledBytes + scope.unnameableBytes
```

成立的原因（必须在代码注释里写清）：

1. `residualBytes = scanBytes − declaredBytes`（定义，逐节点成立）；
2. 登记表与分区自述的 `segments` 首段本身是安全单段名，所以**不可命名部分必不属于任何 `declared` 条目**，其 `declaredBytes` 恒 `0`，整段落在余量侧；
3. 首层节点互不重叠（每个首层条目恰好一个节点），不可命名条目不在任何节点内；
4. `inventory = false` 的节点只贡献 `scanBytes`，既不进 `declaredBytes` 也不进 `residualBytes`，因此必须单独计入 `unreconciledBytes`（§3.5）。

`overRegistered`（`declared > scan`）会让 `residualBytes` 经 `saturating_sub` 落到 `0`，此时守恒式退化为不等式——这是 I61 理念 6 的既有语义，本文不改；守恒门禁（G1）只在无 `overRegistered` 的构造用例上断言等式。

### 3.4 不可命名部分（`unnameable`，N5 的修复）

**判据**：`walk::list_root` 遇到「文件名不是合法 UTF-8」或「不是安全单段名」的首层条目，即为不可命名条目。

**处理**（与现状的差别）：

| 项 | 现状 | 本文 |
|---|---|---|
| 是否 `observe` | 否（直接 `continue`） | **是**（一次 `observe`，与普通节点同一口径、同一预算规则） |
| 是否进 `nodes` | 否 | 否（无 `name` 可展示，进列表会破坏「单段名」契约） |
| 是否进分母 | 否 | **是**（计入 `scope.unnameableBytes` / `scanBytes`） |
| 是否进 `unregisteredBytes` | 否 | 否（该字段定义为节点之和，不含不可命名部分） |
| 是否进 `children` | 否 | 否 |

**理由**：统计不许丢弃（用户可见的「受管目录合计」若静默少算一批字节，分母本身就是错的，余量占总量比例会被高估）。既然无法命名，就**单列**一个桶而不是装没看见——这与 I61「不知道不许写成 0」是同一原则的另一面：**知道有、知道多大、只是叫不出名字**，那就如实上报。

**`null` 规则**：不可命名条目的 `observe` 触界或读不到 → `unnameableBytes` / `unnameableCount` = `null`，并按 §3.6 让 `totals.partial = true`。

### 3.5 `E12` 的扫描值（分母必需，减法仍不做）

I61 让 `E12`（`logs`，`inventory = false`）的 `scanBytes` 恒 `null`，理由是「内容全归条目自身、不存在未认领项」，并且「扫描值与观测值来自两个时刻，减法只有噪声没有信息量」。这两条只否定**减法**（以及由此带来的噪声），不否定**分母**——日志目录确实占盘，缺了它总量就不完整。

`E12` 的**条目占用**已由 breakdown 采样侧统计（应用侧只读遍历，I61 §3.10；`reclaim.note = null`），但那是 breakdown 快照里的数字，与对账快照**不是同一次观测**，不得互相借用（同 X1 / X7 的单一来源原则）。因此对账侧必须自己 `observe` 一次，并用 `declaredBytes = null` 消除减法噪声——「不观测」只解决了噪声，代价是分母恒缺一块。

本文修订（**取代 I61 §3.11.2 第 2 条末句与 §3.11.3 中「`E12` 的 `scanBytes` 恒 `null`」的表述**，OQ11 的「不做减法」结论不变）：

| 字段 | `inventory = false` 的首段（`E12`） |
|---|---|
| `scanBytes` / `scanCount` | **有值**（`observe` 一次，进分母并计入 `scope.unreconciledBytes`；触界 / 读不到为 `null`） |
| `declaredBytes` / `declaredCount` | `null`（不主张登记值，避免持续写入造成假 `overRegistered`） |
| `residualBytes` / `residualCount` | `null`（不做减法） |
| `overRegistered` | `false`（恒假） |
| 前端是否显示该行 | **不显示**（既有 `visible()` 判据是 `residualBytes > 0`，`null` 自然隐藏，前端零改动） |

`inventory = false` 的语义因此收窄为：**只关减法，不关观测**。实现上 `reconciled()` 只决定「是否调用 `subtract_declared`」，不再决定「是否 `observe`」。**现状**（N6）：当前 `walk::build_node` 仍用 `reconciled()` 同时决定「是否 `observe`」，按 §11 T1 改；在 T1 落地前 `E12` 节点 `scanBytes = null`、`totals.partial` 恒 `true`，这是已知临时态，不作为验收基线（V1 / V4 以 T1 完成为前提）。

**预算**：`E12` 的 `observe` 使用独立预算（与其它首层节点同规则），扫描总时长上界因此增加一个节点的预算（V12）。日志目录持续写入会让该节点在两次扫描间数值变化，这是**分母的正常波动**，不产生任何减法噪声。

### 3.6 `null` 传染与下界标记

两类数字的降级策略**不同**，必须分开：

| 结构 | 规则 | 理由 |
|---|---|---|
| `unregisteredBytes` / `unregisteredCount` | **`null` 传染**：任一节点为 `null` 则整体 `null` | I61 既有语义，主信号必须诚实（不许把「不知道」写成「没有」），本文不改（V8） |
| `totals.*` 与 `scope.*` | **跳过 `null` 项累加**，并置 `totals.partial = true` | 用户需要分母；但下界必须显式标注，不许冒充完整值 |

`totals.partial` 判据（唯一）：**任一贡献项为 `null`**（任一节点 `scanBytes` / `scanCount`，或不可命名部分的 `unnameableBytes` / `unnameableCount`）。`inventory.partial`（I61 既有字段）语义不变，两者独立：

| 字段 | 含义 |
|---|---|
| `inventory.partial` | 有节点触界 ∨ 有节点 `overRegistered`（I61 既有）。**前端只对触界出提示**：`truncatedNodes` 非空才显示「只扫描了一部分目录」，`overRegistered` 由节点级标红与 error 提示独立表达（§3.6.1） |
| `inventory.totals.partial` | 本结构的数字是**下界**（少算的部分不知多大） |

**空基根 = `0`，不是 `null`**：某 scope 一个首层条目都没有时，`scanBytes = 0` 且 `totals.partial = false`——「确实没有」是已知事实，与「不知道」必须区分（I61 同一原则）。

**前端硬规则**：`totals.partial = true` 时，合计必须显示为下界形式（前缀 `≥` 或文案标注「以下为下界」），不得只显示数字（前端设计 §7.4 需同步回写，清单见 §12）。

#### 3.6.1 两个 `partial` 提示必须各归其因

`inventory.partial` 把两个互不相干的原因拼成一个布尔：**真触界**（深度 / 条目数 / 耗时上限，此时 `truncatedNodes` 必非空）与 **`overRegistered`**（`declaredBytes > scanBytes`，扫描本身是完整的）。前端提示若只判 `partial`，后一种情况也会弹出「只扫描了一部分目录，下面显示的数字可能偏小」——而事实正好相反：扫描完整，是登记侧的采样更旧 / 更大。

`E14`（`EBWebView`）在持续写入，`declared` 与 `scan` 又是两次独立遍历，因此会偶发 `overRegistered`（§13.3.6 已接受此噪声）。若提示只看 `partial`，这条 warning 会在该条目上常亮，从「异常可见」退化为背景噪音，并掩盖同栈里真正表达该原因的 error 提示。

**规则**：前端「只扫描了一部分目录」提示的唯一触发条件是 `inventory.truncatedNodes.length > 0`（与真触界等价）；`overRegistered` 只由节点级标红 + `residual.notice.over_registered` 表达。后端 `inventory.partial` 字段语义与取值不变（契约不动），收窄只发生在前端消费侧。

### 3.7 分 scope 与不合并的理由

`data` / `local` 是两个独立基根（I61 §3.11.2 第 1 条），合并成一个数会掩盖「哪个盘在涨」。因此：

1. `totals.data` / `totals.local` 各自一套完整数字；
2. 顶层 `totals.scanBytes` 等为两 scope 之和，仅用于「受管目录合计」这一行的总量展示；
3. 前端**不得**自行相加（G5），需要分 scope 时只读 `totals.data` / `totals.local`。

### 3.8 观测与资格解耦（锁定 ≠ 取不到数）

**「多大」与「能不能清」是两件正交的事，必须用两组字段分别表达。** `bytes` / `count` / `availability` 只回答「观测到什么」；`eligibility` / `reason` 只回答「能不能回收」。锁定（`locked`）是**回收侧的结论**，不是**观测侧的结论**——只要取数不依赖解密，锁定态就必须照常上报大小。

**反面案例（现状缺陷，必须修）**：`E9`（`security/vault`）的 `repo_stats` 先判锁再观测：

```rust
if self.state.is_locked() {
    return Err(VaultSectionError::Locked);   // 未走到 tree_stats
}
storage_stats::tree_stats(&self.repo_dir)
```

而 `storage_stats::tree_stats` 只做 `read_dir` + `symlink_metadata` + `meta.len()`，**不解密、不开库、不走 `with_session`**——加密 blob 对操作系统就是普通文件，长度恒可见。因此这是主动放弃观测，不是技术上量不出来。

**同一个 crate 里已经写着正确判据**：`E7`（`security/security.db`）的注释是「只读 `metadata`：vault 锁定不影响它」。`E9` 做的是同一类只读 metadata 遍历，却用了两套判据。

**代价是整条分母**：`Err(Locked)` → `EntrySample::degraded(Locked)`（`bytes = None`）→ `subtract_declared` 对 `security/` 节点遇 `None` 整体降为 `None` → `residual = null` → 按 §3.6 `totals.*` 退化为下界且 `partial = true`；同时 `assemble` 的 `complete` 只接受 `available` / `missing`，`locked` 会把**整页** `complete` 拉成 `false`。一个条目的观测缺口，污染全页的完整性承诺。

**规则（可验收）**：

1. **锁定不构成观测豁免**：`inventory = true` 的条目，只要数据在文件系统层面可见，锁定态必须照常产出 `bytes` / `count`，`availability` 用 `available` / `missing`；
2. **锁的信息只走资格侧**：`eligibility = locked` / `reason = locked`，与静态的 `notEligible` / `userData` 在「能不能清」上等价（都不能清），差别只是给用户一句「先解锁」的提示；
3. **`availability = locked` 的收窄语义**：仅用于「锁定确实导致观测不可得」的条目（例如内容必须解密后才能枚举）。本仓库当前**没有任何此类条目**——`E7`–`E11` / `E16` 的观测全部是文件系统元数据；新增此类条目必须同时说明为何无法绕开，并在 §13 式的处置记录里留档，不得默认套用；
4. **禁止用 `Locked` 表达「取不到」**：读不到、超时、IO 失败各有其档（`unreadable` / `unavailable`），把锁定塞进观测失败会让「先解锁再重试」成为错误建议（I61 §7.1 特例 2 的同一条理由）。

**修法**：删除 `repo_stats` 的判锁短路，让 `tree_stats` 恒执行；`eligibility()` 里 `KEY_REPO` 的锁定分支（现已被短路饿成死代码）随即承担「锁定 = 不可清」的表达职责，大小照常上报。落点见 §11 T7。

---

## 4 条目身份边界（`E` 编号 / `key`）

本节回答「前后端的 `E` 编号是否需要严格对应」：**不需要，也不允许需要**。

### 4.1 `E` 编号不进 wire 语义（N9 / N10）

1. `E*` 是**规格文档编号 + 后端内部路由 id**，不是契约身份；
2. 唯一跨 wire 的是 `StorageInventoryNodeDto.entry`，定位为**调试用惰性标签**：前端不得据它渲染名称、不得据它分支、不得据它排序；缺失或错乱只影响日志可读性，不影响任何 UI 行为；
3. 条目身份对前端只有一种：`key`。

### 4.2 `key` 是唯一跨线身份

`entryKeys`（回收输入）、`ENTRY_NAME_KEYS`（名称）、`GROUP_NAME_KEYS`（分组）三者共用**同一字符串空间**。新增 / 改名条目必须同步：

1. 后端：`MANAGED_ROOTS` 或分区 `entries()` 的 `key`；
2. 前端：`lexicon.ts` 两张表 + 两份 locale；
3. 门禁 G7 校验「后端 key 集合 ⊆ 前端名称表」，缺失即红（未知 key 在前端只 `log.warn` 并显示原串，属可接受的降级，但**新增条目时不允许依赖这个降级**）。

### 4.3 后端 `E` 字面量收敛（N11 / N12）

**应当**把 `E*` 收敛为单点定义，使静默兜底变为编译期穷尽：

1. 唯一来源是 `registry::MANAGED_ROOTS` 的 `entry` 字段；`decl.rs` / `eligibility.rs` / `execute.rs` 改为按该来源匹配（常量或枚举，任一形式均可，但不得重复写字面量）；
2. `decl.rs` 的 `unreachable!` 与 `eligibility.rs` 的 `_ => {}`、`execute.rs` 的 `_ =>` 三处，收敛后必须**不再存在**「匹配不到就静默继续」的分支；core 条目走不到分区分支，分区分支也接不到 core 条目；
3. 门禁 G8：后端生产代码中 `"E\d+"` 字面量只允许出现在登记表与测试。

---

## 5 契约

### 5.1 新增 DTO

```rust
pub struct StorageInventoryScopeTotalsDto {
    pub scan_bytes: Option<u64>,
    pub scan_count: Option<u64>,
    pub declared_bytes: Option<u64>,
    pub declared_count: Option<u64>,
    pub residual_bytes: Option<u64>,
    pub residual_count: Option<u64>,
    pub unregistered_bytes: Option<u64>,
    pub unregistered_count: Option<u64>,
    pub unreconciled_bytes: Option<u64>,
    pub unreconciled_count: Option<u64>,
    pub unnameable_bytes: Option<u64>,
    pub unnameable_count: Option<u64>,
}

pub struct StorageInventoryTotalsDto {
    pub scan_bytes: Option<u64>,
    pub scan_count: Option<u64>,
    pub residual_bytes: Option<u64>,
    pub residual_count: Option<u64>,
    pub partial: bool,
    pub data: StorageInventoryScopeTotalsDto,
    pub local: StorageInventoryScopeTotalsDto,
}
```

### 5.2 `StorageInventoryDto` 新增字段

| 字段 | 类型 | 约束 |
|---|---|---|
| `totals` | `StorageInventoryTotalsDto` | 恒存在（键恒在）；`status = neverRun` 时内部数字全 `null`、`partial = false` |

既有字段（`status` / `scannedAt` / `failedAt` / `nodes` / `unregisteredBytes` / `unregisteredCount` / `partial` / `truncatedNodes`）**键名、类型、语义全部不变**（G9 / V8）。

### 5.3 `StorageInventoryNodeDto` 语义修订（仅 `E12` 类）

| 字段 | 修订 |
|---|---|
| `scanBytes` / `scanCount` | `inventory = false` 的首段**不再恒 `null`**（§3.5） |
| `declaredBytes` / `declaredCount` | `inventory = false` 的首段恒 `null`（现状为条目登记值；置 `null` 是为了不产生 `residual = scan − declared`） |
| `residualBytes` / `residualCount` | 不变（`inventory = false` 时恒 `null`） |
| 其余字段 | 不变 |

`StorageInventoryScopeTotalsDto` / `StorageInventoryTotalsDto` 的字段语义：

| 字段 | 约束 |
|---|---|
| `scanBytes` / `scanCount` | 该 scope 全部首层节点之和 + `unnameable`；`null` 项跳过累加 |
| `declaredBytes` / `declaredCount` | 该 scope 内 `declared` 节点登记值之和；审计用，不用于展示分母 |
| `residualBytes` / `residualCount` | 该 scope 内全部节点余量之和（含 `declared` 差额、`companion`、`ignored`、`optionalNotMounted`、`unregistered`）；`inventory = false` 的节点为 `null`，不参与 |
| `unregisteredBytes` / `unregisteredCount` | 该 scope 内 `unregistered` 节点扫描值之和；与顶层同名字段的分 scope 投影（V6） |
| `unreconciledBytes` / `unreconciledCount` | 该 scope 内 `inventory = false` 节点（`E12`）的扫描值之和：在分母内、不参与减法（§3.5） |
| `unnameableBytes` / `unnameableCount` | 该 scope 首层不可命名条目的字节 / 条数（§3.4） |
| `partial`（仅 `totals`） | 本结构数字为下界（§3.6） |

### 5.4 契约规则

1. 输出 `Option` **不加** `skip_serializing_if`：键恒在、值 `null`；`partial` 为 `bool` 必填；
2. 嵌套结构不设 `Option`：`totals` / `totals.data` / `totals.local` 恒在，内部字段各自可为 `null`；
3. 改动后必须 `npm run contract:gen`，提交 `src/types/protocol.contract.ts`；禁止手改生成类型（G4）；
4. `refresh_storage_inventory` 的返回类型**不变**（`StorageInventorySnapshotStatusDto`），仍无法回数字（I61 §6.1 边界 9、X7）；
5. 不新增命令、不新增事件；`storage:inventory-updated` 复用不变。

---

## 6 前端增量条款

前端实现基线仍是 I61 前端设计（已归档） §7.4；本节只给**必须回写进该文档**的增量，实施时以该文档为落点。

| 项 | 增量 |
|---|---|
| 合计行结构 | 由「单段：未登记合计」改为**三段**：受管目录合计（`totals.scanBytes`，分 scope 见 tooltip / 次级行）→ 其中余量（`totals.residualBytes`）→ 其中未登记（`unregisteredBytes`） |
| 下界 | `totals.partial = true` 时，三个数**全部**加下界标注（前缀 `≥` + 固定 hint）；不得只标其中一个 |
| `null` | 任一数为 `null` → `—`（沿用 `bytesOrDash`，不填 0） |
| 不可命名 | `unnameableBytes > 0` 时追加一行说明（`n` 项无法命名，仅计入合计），不进入节点列表 |
| 数据来源 | 只读 `inventory.totals` 与 `inventory.unregisteredBytes`，**禁止**对 `nodes` 求和（G5） |
| 交互 | 不新增按钮、不新增请求；扫描按钮与五态不变 |

i18n 键（命名建议，zh-CN 为类型源）：`residual.totals.total` / `residual.totals.residual` / `residual.totals.unregistered` / `residual.totals.unnameable` / `residual.totals.lower_bound`；既有 `residual.totals.unregistered` 若被替换，旧键必须删除而不是留着并行。

---

## 7 门禁

| 编号 | 门禁 | 判据 |
|---|---|---|
| G1 | 守恒单测 | 构造「declared + unregistered + companion + unnameable + `inventory = false`」五类节点，断言 `scope.scanBytes == declared + residual + unreconciled + unnameable`（§3.3） |
| G2 | 下界标记 | 构造一个 `scanBytes = null` 的节点，断言 `totals.partial = true` 且 `totals.scanBytes` 等于其余非 `null` 项之和 |
| G3 | `unregistered` 回归 | 既有 `sum_unregistered` 用例**不改一行**且全绿（V8） |
| G4 | 契约生成一致 | `npm run contract:gen` 后 `protocol.contract.ts` 无未提交差异 |
| G5 | 前端不求和 | `residualView.ts` 中不出现对 `nodes` 的 `reduce` / `sum` / 循环累加；合计只读 `totals` |
| G6 | `E*` 渲染禁令 | 沿用前端 D13：渲染路径检索 `E1`..`E18` 结果为 0 |
| G7 | `key` 对齐 | 后端 `MANAGED_ROOTS` 的 `key` + 已挂载分区 `entries()` 的 `key` ⊆ 前端 `ENTRY_NAME_KEYS` |
| G8 | `E` 字面量收敛 | 后端生产代码 `"E\d+"` 只出现在登记表与测试（§4.3） |
| G9 | 只读边界 | `totals` 的任何数字不写回 `entries` / `totalBytes` / `complete` |
| G10 | 无第二套观测 | 不新增命令；`refresh_storage_inventory` 返回类型不变 |
| G11 | 登记必须上报大小 | `MANAGED_ROOTS` 中每条 `inventory = true` 的条目在 `available` / `missing` / `locked` 三种样本下都必须产出非 `null` 的 `declaredBytes`（单测逐个断言）；上报不了即判红，不允许「先登记、后补统计」长期悬空（§13.2.2 R1，义务而非准入）。**`locked` 已从豁免档移除**：锁定不构成观测豁免，取不到数只能落在 `unreadable` / `unavailable`（§3.8） |
| G12 | 登记链路同步 | 新增 core 条目必须同时进 `MANAGED_ROOTS`（首段来源：`dir_name` 常量或 owner crate 常量）、`decl.rs::static_for`、采样分派、前端 `ENTRY_NAME_KEYS`；缺一处即编译失败或单测红（`static_for` 收敛后无 `_` 兜底，见 §4.3）。可选特性条目（`E16`）不走 core 链路，由 feature 侧分区自注册并自测 |

---

## 8 验收判据

| 编号 | 判据 |
|---|---|
| V1 | 无触界、无 `overRegistered` 时，`totals.partial = false` 且 §3.3 守恒式（含 `unreconciled` 项）逐 scope 成立 |
| V2 | 空基根（无任何首层条目）时 `scanBytes = 0`、`partial = false`（不是 `null`） |
| V3 | 构造一个不可命名首层条目（如非法 UTF-8 名），断言其字节进 `unnameableBytes` 与 `scanBytes`、**不**进 `nodes`、**不**进 `unregisteredBytes` |
| V4 | `E12` 节点 `scanBytes` 有值并计入分母（`scope.unreconciledBytes`）；其 `declaredBytes` / `residualBytes` 为 `null`、`overRegistered = false`；前端仍不显示该行 |
| V5 | 任一节点触界时 `totals.partial = true`，且 `totals` 数字等于其余非 `null` 项之和（下界） |
| V6 | `totals.data.unregisteredBytes + totals.local.unregisteredBytes == inventory.unregisteredBytes`（同一次快照内） |
| V7 | 前端合计行三段齐全；`partial` 时三段均带下界标注；任一 `null` 显示 `—` |
| V8 | `unregisteredBytes` / `unregisteredCount` 在触界场景下仍为 `null`（`null` 传染未被下界逻辑污染） |
| V9 | `contract:gen` 生成结果与提交一致；`totals` 嵌套结构键恒在 |
| V10 | G7 门禁在「新增一个分区条目但未加前端名称」时判红 |
| V11 | 渲染路径无 `E*`（G6），`node.entry` 在 `residualView.ts` 中无读取 |
| V12 | 扫描总时长上界 = 原上界 + 一个节点预算（`E12` 独立预算），不因分母补全而无界增长 |
| V13 | core 配置条目（`E15` / `E17` / `E18`）在 Windows 上 `availability = available`、`declaredBytes = scanBytes`、`residualBytes = 0`，且其字节**不再**计入 `unregisteredBytes`；`E16`（`security/vault-config.json`）在 vault 特性挂载且文件存在时同形（由 vault 分区自注册、自观测）；V6 两侧仍相等 |
| V14 | core 配置条目（`E15` / `E17` / `E18`）在 Linux / macOS 上恒 `missing` / `0 B`，`complete` 仍为 `true`（`missing` 是既有合法状态）；`E16` 落在数据基根内的 `<data>/security/`，三平台落点一致 |
| V15 | 登记后 `unregistered` 区只剩无 owner 的条目：本机样本为 `probe-config.json`，它仍是 `unregistered` 且不出现在节点名称映射中（§13.5） |
| V16 | vault 锁定态下 `E9`（`security/vault`）仍产出非 `null` 的 `declaredBytes`、`availability = available`（空目录为 `missing`），`eligibility = locked` / `reason = locked`；解锁后 `eligibility = notEligible` / `reason = userData`；两种态下 `totals.partial` 与整页 `complete` 均不受锁定影响（§3.8） |

---

## 9 拒绝方案

| 编号 | 方案 | 拒绝理由 |
|---|---|---|
| X1 | 前端自行对 `nodes` 求和得出总量 / 全类余量 | 两套口径必然漂移；且前端拿不到「不可命名部分」，分母会永远偏小 |
| X2 | 把 `unregisteredBytes` 直接改名为「总量」 | 语义混淆：它只是主信号，不是分母；改名会让既有回归与前端文案全部失真 |
| X3 | `data` + `local` 合并成单一「受管目录总量」并删掉分 scope | 掩盖「哪个盘在涨」；两个基根是独立事实 |
| X4 | `partial` 时把已知节点相加直接当完整总量 | 「用已知冒充全部」，违反 I61 理念 3；必须标下界（§3.6） |
| X5 | 不可命名条目继续丢弃（不计入分母） | 分母偏小 → 余量占比被高估；且无法从任何字段看出丢了多少（N5） |
| X6 | 为 `E12` 保留 `scanBytes = null` 以维持「不做减法」 | 分母恒缺一块，`totals` 常态为下界；「不做减法」与「不做观测」是两件事（§3.5）。breakdown 侧已有的 `E12` 占用也不能顶替——不同快照、不同遍历，借用即引入第二套口径 |
| X7 | 新增「查总量」命令 | 第二套观测入口，违反 I61 §1.2 对外唯一性；总量随 `get_storage_breakdown` 同一次快照返回 |
| X8 | 把四个配置文件塞进 `IGNORED_NAMES` | 该表是首层名全局匹配（`data` / `local` 两个 scope 共用），会连带忽略任何基根下的同名首层；且其语义是「已知系统垃圾、不认领也不管」，把应用自有文件放进去等于用白名单抹掉观测（§13.4.5） |
| X9 | 把「体量 / 能否统计」当作登记门槛继续拒绝登记应用自有文件 | 归属由 owner 决定，不由体量或实现进度决定：体量门槛会把「有 owner、只是没登记」说成「没人认领」（污染 `unregistered`）；可统计性门槛会把 owner 的实现缺口藏起来而不是暴露它。正确做法是「有 owner 就登记，登记就必须上报大小」（§13.2.2） |

---

## 10 开放问题

| 编号 | 问题 | 倾向 |
|---|---|---|
| OQ1 | `unnameable` 是否需要按 scope 分别展示，还是只给合计 | 字段按 scope 给（§5.1），前端只展示合计行与提示；分 scope 留作 tooltip |
| OQ2 | `totals.partial` 与 `inventory.partial` 是否合并为一个字段 | 不合并：前者是「下界」，后者是「触界 / 口径异常」，前端提示层级不同 |
| OQ3 | `declaredBytes` 全类合计是否需要在 UI 展示 | 不需要；它是审计量（`scan = declared + residual + unnameable` 的中间项），展示会与条目表重复 |
| OQ4 | 不可命名条目的 `observe` 是否需要单独预算（避免拖长扫描） | 与普通节点同规则（每项独立预算）；若实测拖长再单独收窄（不在本期） |
| OQ5 | `E12` 观测后，`partial` 是否更容易为 `true`（日志目录条目多） | 是；这是诚实代价，由前端下界标注承接，不因此放宽阈值 |
| OQ6 | `E14`（`EBWebView`）的 `residualBytes` 因两次遍历 + 持续写入而出现 ± 噪声，是否需要容差 | 不加容差字段：噪声量级远小于总量，按既有 `overRegistered` 如实展示；若实测频繁再议（§13.3.6） |
| OQ7 | `E14` 在 macOS / Linux 上恒 `missing` / `0 B`，是否要按平台隐藏 | 不隐藏：`missing` 是既有合法状态，且登记表不参与平台门控；按平台隐藏需给 `ManagedRoot` 加可选段，属结构变更（§13.3.7） |
| OQ8 | 配置文件的伴生临时文件（`config.json.tmp` 等）是否要认领 | 不认领：`atomic_write` 的中间态按既有规则落 `unregistered`（`IGNORED_NAMES` 不含它），量与生命周期都极短；要收则走伴生后缀规则扩展，属独立变更（§13.4.4） |

---

## 11 实施切片

| 切片 | 内容 | 落点 |
|---|---|---|
| T1 | 扫描累计：`walk::scan` 返回每 scope 的节点 / 不可命名部分；`RawNode` 不变，新增 `ScanTotals`（scope 级 scan / unnameable）；`reconciled()` 只控减法不控观测（§3.5） | `inventory/walk.rs`、`inventory/mod.rs` |
| T2 | 汇总与契约：`cache::read` 用已算好的 `node_dto` 累加出 `totals`（不重扫）；`StorageInventoryTotalsDto` / `StorageInventoryScopeTotalsDto` 落地；`npm run contract:gen` | `inventory/cache.rs`、`assistant-protocol/src/storage.rs`、`src/types/protocol.contract.ts` |
| T3 | 前端合计行：`residualView.totalsOf` 改读 `totals`，三段 + 下界 + `unnameable` 提示；i18n 两 locale；回写前端设计 §7.4 | `src/settings/pages/storage/`、`src/i18n/locales/`、前端设计文档 |
| T4 | 身份收敛：`E*` 单点常量 + 三处静默兜底消除；G7 / G8 门禁接入 | `entries/`、`reclaim/`、`app_dirs/registry.rs`、门禁脚本 |
| T5 | `EBWebView` 登记 + 接管（§13.3）：`dir_name` 加 `WEBVIEW_PROFILE = "EBWebView"` 并进 `ALL` + `MANAGED_ROOTS` 一条（`E14` / `EBWebView` / `Local` / `RecursiveTree` / `inventory = true`，`segments` 引用该常量，V19 无需豁免）+ `EXPECTED_ENTRY_COUNT` / `CORE_ENTRY_IDS` 同步 + `sample/rows.rs` 的 `tree_stats` 与 `reclaimable` 子集 + `decl.rs` 静态档 + 前端 `lexicon.ts` 补 `key`（G7）；**接管四项**（§13.3 表）：`AppDirs::webview_data_directory()` 单点声明落点、编程创建窗口显式 `.data_directory(...)`、`AppDirs::ensure_webview_profile()` 在建窗前创建目录、`setup` 建窗后做存在性漂移守卫。**落点与 Tauri 强制默认逐字相同，不迁移、不改 `tauri.conf.json`**；回收执行器（运行期只登记、启动期生效，返回 `scheduled`）与缓存白名单随本切片落地，资格恒 `eligible`（§13.3.5） | `app_dirs/dir_name.rs`、`app_dirs/registry.rs`、`app_dirs/roots.rs`、`app_dirs/webview_profile.rs`、`window.rs`、`main.rs`、`setup.rs`、`cmd/storage/sample/`、`cmd/storage/entries/`、`cmd/storage/reclaim/`、`src/settings/pages/storage/`、`src/i18n/locales/` |
| T6 | 四个配置文件登记（§13.4）：core `MANAGED_ROOTS` 三条（`E15` / `E17` / `E18`，`Data` / `SingleFile` / `inventory = true`），`segments` 引用 owner 常量 + `EXPECTED_ENTRY_COUNT` / `CORE_ENTRY_IDS` 同步（最终 core 12 条）+ `decl.rs` 静态档（`None` / `NotEligible` / `BusinessData`）+ `sample/rows.rs` 的 `file_stats` 采样 + `collect.rs` 分派 + 前端 `lexicon.ts` / 两份 locale / `lexicon.test.ts`（G7 / G11）。`E16`（`security/vault-config.json`）由 vault 特性自注册：`assistant-vault/src/storage_section.rs` 的 `ENTRIES` 增一条（文件名取 `<VaultConf as ConfGroup>::FILE_NAME`）、`stats` / `eligibility` 补分支、`VaultConf::get_conf_dir()` 对齐 `<data>/security/`、`vault_plugin.rs` 传 `conf_path`、前端由 vault 模块 `registerEntryNames` 自注册文案。**core 侧零知识、零字面量、`dir_name` 不加常量** | `app_dirs/registry.rs`、`cmd/storage/entries/`、`cmd/storage/sample/`、`crates/assistant-vault/src/storage_section.rs`、`crates/assistant-vault/src/conf.rs`、`src/vault_plugin.rs`、`src/settings/pages/storage/`、`src/features/vault/module.ts`、`src/i18n/locales/` |
| T7 | vault 锁定态观测解耦（§3.8）：删除 `repo_stats` 的 `is_locked()` 短路，让 `tree_stats` 恒执行；`eligibility()` 的 `KEY_REPO` 锁定分支继续承担「锁定 = 不可清」；改单测「锁定态仍上报大小 + 资格为 `locked`」、补「解锁后 `notEligible` / `userData`」；G11 覆盖 `locked` 档（V16） | `crates/assistant-vault/src/storage_section.rs` |

**现状注记（T5，2026-11-22）**：`E14` 已全链路落地——登记（`E14` / `EBWebView` / `Local` / `RecursiveTree` / `inventory = true`，T5 落地时 `EXPECTED_ENTRY_COUNT` = 9、T6 落地后为 12）、观测（`tree_stats` 整棵树 + `reclaimable` 只算缓存白名单）、资格（恒 `eligible`，观测失败 `notEligible`）、接管（`webview_data_directory` 单点声明 + 编程建窗显式 `.data_directory(...)` + 建窗前 `ensure_webview_profile()` + 建窗后漂移守卫）、回收（运行期 `mark_pending` 返回 `scheduled`、启动期 `run_pending_purge` 删白名单缓存）。`complete` 判定与 `E12` 同处理，排除持续写入条目。

**现状注记（T6，2026-11-22）**：`E15` / `E17` / `E18` 已全链路落地——core 登记（`Data` / `SingleFile` / `Forbidden` / `inventory = true`，`EXPECTED_ENTRY_COUNT` = 12，`segments` 直接引用 owner 的 `ConfGroup::FILE_NAME`）、观测（`sample_config_file` = `fs_stats::file_stats`，与 `E6` 同形；路径由 `AppDirs::managed_path_of(entry)` 按登记表拼出）、静态档（`None` / `NotEligible` / `BusinessData`，`executor_action` 白名单拒绝）、前端词表（`lexicon.ts` + 两 locale + 测试同步）。`E16` 由 vault 特性自注册：`assistant-vault/src/storage_section.rs::ENTRIES` 增 `security/vault-config.json`（文件名取 `<VaultConf as ConfGroup>::FILE_NAME`，分区条目 6 条）、`stats` / `eligibility` 补分支、`VaultConf::get_conf_dir()` 返回 `<data>/security/`（旧位置不迁移）、`vault_plugin.rs` 初始化时传 `conf_path`、前端 `features/vault/module.ts` 调用 `registerEntryNames` 登记文案；core 侧无该条目任何知识。V13 / V14 由 `tests/storage_inventory.rs::config_files_are_declared_and_reconciled` 与登记形状单测覆盖。

T1 / T2 必须先于 T3；T4 与 T1–T3 无依赖，可并行；T5 / T6 独立于 T1–T4，但只有 T1 / T2 落地后它们的字节才会出现在分母里（§13.3.3 / §13.4.3）。T5 与 T6 互不依赖，但两者都改 `EXPECTED_ENTRY_COUNT` / `CORE_ENTRY_IDS`：按落地顺序递增（先 T5 则以 9 为基数、T6 再加 3 得 12；顺序反过来则 11 → 12），最终 core 12 条；`E16` 不计入 core 计数。

**现状注记（2026-11-22）**：`E12` 的 breakdown 侧条目占用已落地（§2 N13），`inventory` 侧仍是 N6 状态——`reconciled()` 同时控观测、`E12` 节点 `scanBytes = null`，因此 `totals.partial` 在 T1 落地前恒为 `true`。T1 只需把 `reconciled()` 的职责收窄到「是否 `subtract_declared`」，并在 `build_node` 对 `inventory = false` 的首段照常 `observe`、把 `declaredBytes` 置 `null`；减法路径与守恒式（§3.3）其余部分不动。`E9` 的锁定短路（N17 / T7）是**独立缺陷**，与 T1 无依赖：即使 T1 落地，vault 锁定态仍会把 `security/` 节点打成 `null`，故 T7 必须单独完成。

---

## 12 回写清单

实施时同步修改以下**已存在**的文档，避免两处口径漂移：

| 文档 | 位置 | 回写内容 | 状态 |
|---|---|---|---|
| I61 存储管理器规格 | §3.11.2 第 2 条末句 | `E12` 的「不遍历子树」改为「只关减法、仍观测」（加注「由 I67 修订」） | 未回写（随 T1 落地，避免文档先于实现） |
| I61 存储管理器规格 | §3.11.3 减法段 | `E12` 的 `scanBytes` 恒 `null` → 改为有值、`declaredBytes` 为 `null` | 未回写（随 T1 落地） |
| I61 存储管理器规格 | §6.2 `StorageInventoryDto` / `StorageInventoryNodeDto` 表 | 补 `totals` 字段与 `E12` 语义修订，指向本文 | 未回写（随 T2 落地） |
| I61 存储管理器规格 | §3.2 / §3.10 `E12` 条目占用与 `note` | 应用侧只读遍历给出可用性、`reclaim.note = null`、策略数值只由 `policy`（`keepCount = 3`）表达 | **已回写**（E12 全链路修订），本文不再改动 |
| I61 前端设计 | §7.4 合计行 | 三段结构 + 下界标注 + 不可命名提示 + 禁自行求和 | 未回写（随 T3 落地） |
| I61 前端设计 | §7.1 / §7.3 | 数据来源补 `totals`；`partial` 与 `totals.partial` 两级提示的区分 | 未回写（随 T3 落地） |
| I61 存储管理器规格 | §3.2 条目清单 / §3.11.3 分类表 | core 条目 8 → 12：补 `E14`（`EBWebView`，T5）与 `E15` / `E17` / `E18`（`config.json` / `orb-config.json` / `mascot-config.json`，T6）四行，各自注明 `scope` / `coverage` / 风险档 / `inventory = true`；`E16`（`security/vault-config.json`）属 vault 可选分区条目，由特性自注册，不计入 core 清单 | T5 的 `E14` 已落地；`E15` / `E17` / `E18` 未回写（随 T6 落地）；`E16` 回写为分区条目 |
| I61 存储管理器规格 | §3.10 `E9` 观测 / §7.1 特例 2 / §8 V13 | `E9`（`security/vault`）锁定态仍上报大小（`tree_stats` 恒执行），锁的信息只走 `eligibility = locked` / `reason = locked`；`availability` 为 `available` / `missing`（不再 `= locked`），V13 的「`availability = eligibility = locked`」随之改为「`eligibility = locked` + `availability = available`」；§7.1 中「为取长度而 unlock vault」的拒绝理由不变——本修订正是**不解锁、只读 metadata**（本文 §3.8） | 未回写（随 T7 落地） |

回写时机以切片为准：文档不得先于实现写成「`E12` 已有扫描值」而代码仍为 `null`（§11 现状注记）。本文不修改任何归档文档；I61 三份文档已移入 `docs/achievement/`（待归档），回写后随其归档一并冻结。

---

## 13 已知未登记条目处置记录

**触发**：2026-11-22 实机快照，数据基根首层出现 5 个未登记名字（4 个配置文件 + 1 个历史残留），本地基根首层出现 1 个未登记目录（`EBWebView/`），合计 6 个 / `15174 项 / 682.1 MB`（字节几乎全部来自 `EBWebView/`）。本节记录每个条目的归属判定与处置决策，作为后续「哪些登记、哪些不登记」的事实依据，避免每次看到未登记告警都重新讨论一遍。

### 13.1 归属与成因

| 条目 | 位置 | 是谁的 | 未登记成因 | 处置 |
|---|---|---|---|---|
| `EBWebView/` | 本地基根（`%LOCALAPPDATA%` + `APP_IDENTIFIER`，代码侧取 `AppDirs::local()`） | **外部写入、应用接管**：内容字节由 WebView2 运行时写入，落点 / 目录创建 / 回收时机 / 观测由本应用接管（§13.3 接管表）；落点与 Tauri 强制默认 `app_local_data_dir()` 同值（N15） | 登记表未收录；准入与必报项全满足（§13.3.1） | **登记为受管目录**（core 条目 `E14`，T5，首段进 `dir_name::WEBVIEW_PROFILE`，见 §13.3） |
| `config.json` | 数据基根首层 | **应用自有**：`AppConf` 主配置 | 登记表未收录（旧版按体量排除，§13.2.2） | **登记**（core 条目 `E15`，T6，见 §13.4） |
| `vault-config.json` | 数据基根首层（旧位置）；现落 `<data>/security/` | **应用自有**：保险库配置，可能含远端 URL 凭据 | 同上 | **登记**（`E16`，T6）：由 vault 特性在可选分区内自注册为 `security/vault-config.json`，core 侧零知识；禁止任何清理动作；旧位置文件不迁移，留盘即视为未登记余量 |
| `orb-config.json` | 数据基根首层 | **应用自有**：悬浮球配置 | 同上 | **登记**（core 条目 `E17`，T6） |
| `mascot-config.json` | 数据基根首层 | **应用自有**：吉祥物配置 | 同上 | **登记**（core 条目 `E18`，T6） |
| `probe-config.json` | 数据基根首层 | **应用自有历史残留**：旧「探针」特性（I24，已归档）配置文件，当前代码零引用 | 特性下线后**无人认领**（准入不通过） | **保留、不登记**（无 owner，R1 无从履行，见 §13.5） |

### 13.2 判据：登记与回收能力正交

#### 13.2.1 纠正一个错误前提

前一版本把「没有可回收语义」写成不登记的理由，这是错的：登记表里**本来就有「登记但不回收」与「登记且可回收缓存」两种挡位**，回收能力不是登记的判据。

| 挡位 | 条目 | `action` | `eligibility` | `reason` | 含义 |
|---|---|---|---|---|---|
| 登记、永不回收 | `E5` mascot-assets | `None` | `NotEligible` | `UserData` | 应用自有用户资产，只观测 |
| 登记、永不回收 | `E6` assistant.db | `None` | `NotEligible` | `BusinessData` | 业务数据，只观测 |
| 登记、可重建缓存 | `E2` attachments/derived | `RebuildCache` | `Eligible` | `DerivedCache` | 删了能重建 |
| 登记、可重建缓存 | `E4` tool-index | `RebuildCache` | `Eligible` | `DerivedCache` | 删了能重建 |
| 登记、可重建缓存 | `E14` EBWebView（T5） | `RebuildCache` | `Eligible`（运行期只登记，启动期执行） | `DerivedCache` | Chromium 缓存可重建，用户态部分不算余量（§13.3.4）；应用接管落点 / 创建 / 回收时机（§13.3） |
| 登记、外部自持回收 | `E12` logs | `Rotate` | `OwnerManaged` | `ExternalOwner` | 回收归插件 |
| 登记、永不回收 | `E15` / `E16` / `E17` / `E18` 四个配置文件（T6） | `None` | `NotEligible` | `BusinessData` | 应用自有配置，只观测 |

因此判据只能是**准入 + 必报项**：准入只看「有没有 owner」（§13.2.2 开头）；登记后必须上报大小（R1）并声明回收结论 / 布局 / 风险档（R2–R4）。

#### 13.2.2 登记即义务：必须上报大小（旧「收益三项」已废止）

**准入只有一个条件：有 owner。** 属于本应用自己写入 / 自持的对象就登记；没有 owner 的字节留在 `unregistered`——那正是该类的语义（「有字节、无 owner」），而不是「有 owner、没登记」。

**R1 是登记后的强制义务，不是准入门槛。** 登记表的每一行都承诺上报 `declared`：**能不能统计大小不构成能不能登记的条件；登记了就必须上报大小**。owner 暂时答不出大小，是它的实现缺口，必须补齐，而不是拒绝登记（把缺口藏进 `unregistered` 只会让主信号失真）。

`declared` 的来源按 `coverage` 固定（两次取数各自如实上报，不从对账扫描结果反填）：

| `coverage` | `declared` 来源 |
|---|---|
| `SingleFile` | owner 对自己路径做 `app_dirs::fs_stats::file_stats`（`metadata.len()`；`E6` 已是此形） |
| `SelfFilesOnly` / `RecursiveTree` | owner stats API（`E4`）或 `app_dirs::fs_stats::tree_stats`（`E5` / `E12`） |

**不履行 R1 的后果是结构性的**：`declared = null` → `residual = null` → `totals.residualBytes` 退化为下界（§3.6）。因此 G11 把这条义务钉死在单测里：任一登记条目在 `available` 样本下 `declaredBytes == null` 即判红，不允许「先登记、后补统计」长期悬空。

**旧版错在哪。** 前一版把两件事都写成了「门槛」，两者都错：

1. **体量门槛**（旧 D1「体量与增长值得用户看见」）：把「应用自有、但很小」的配置排除在登记之外，于是它们以 `unregistered` 出现——等于把「有 owner、只是没登记」说成「没人认领」，直接污染 §3.1 的主信号。体量只影响展示权重，不影响归属事实（X9）；
2. **可统计性门槛**（把「owner 能给出统计口径」当准入条件）：逻辑上颠倒。owner 答不出大小是它的实现缺口，当准入条件只会让条目躲进 `unregistered` 而不是暴露缺口；正确做法是登记并强制上报（R1）。

因此现在只剩一个准入条件（有 owner）和一组登记必报项（R1–R4）。

**登记必报项（R2–R4，与 R1 并列，缺一项即不可合入）**：

| # | 必填 | 说明 |
|---|---|---|
| R2 | 回收结论 | `action` / `eligibility` / `reason` 三件套必须给出结论，可以是「永不回收」（`None` / `NotEligible`）；「不知道能不能清」不是结论 |
| R3 | `scope` + `coverage` | 必须与 owner 的真实布局一致；`scope` 只有 `Data` / `Local` 两个基根 |
| R4 | 风险档 | `RiskTier` 四档之一，与 `action` 独立（§3.6 硬约束 5–6） |

成本两项：①跨平台可能永久 `missing` / `0 B`（`scope` 只有两个基根；配置文件在 Linux / macOS 落在 config 基根，见 §13.4.4）；②统计口径必须与既有 owner 实现对齐，不得造第二套。

`SingleFile` 的 `declared` 与扫描值同路径同源（都取 `metadata.len()`），`residualBytes` 结构性为 `0`。这不违反 R1 的「不反填」：声明值来自 owner 的**路径知识**，不是从扫描结果里抄；两次取数若因写入竞争不同，各自如实上报，不做修正。

### 13.3 `EBWebView`：受管目录（新增 core 条目 `E14`）

**它当然有 owner，而且应用接管它的管理——不是只读观测。** 内容字节由 WebView2 运行时写入，但落点、创建、回收时机与观测都由本应用负责，四条各自有可执行的代码落点：

| 接管项 | 代码落点 | 说明 |
|---|---|---|
| 落点声明 | `AppDirs::webview_data_directory()` 返回 `local()`；编程创建窗口时显式 `.data_directory(AppDirs::webview_data_directory().to_path_buf())` | 落点从此由本应用声明，不再只是「Tauri 内部默认」；值与 Tauri 强制默认（`app_local_data_dir()`，`tauri-2.12.5/src/manager/webview.rs`）逐字相同，故落点不变、无迁移 |
| 目录创建 | `AppDirs::ensure_webview_profile()`：Windows 上 `create_dir_all(<local>/EBWebView)`，在 `Builder::run` 之前调用 | 该目录的「存在」由应用保证，不依赖 WebView2 首次启动的副作用；非 Windows 直接返回，不建空目录 |
| 回收 | 运行期只**登记待执行**（`app_dirs::webview_profile::mark_pending`），下次启动在 `Builder::run` 之前消费标记并删缓存白名单子目录；**前置条件：进程内无任何 webview 存活** | 唯一可行时机是建窗之前（用户请求 → 记录待执行 → 下次启动执行）；运行期执行结果如实返回 `scheduled`，不是 `done` 也不是静默跳过（§13.3.5） |
| 观测 | `app_dirs::fs_stats::tree_stats` | 与 `E5` / `E12` 同一实现，不造第二套口径 |

`data_directory` 取 `AppDirs::local()`（即 `%LOCALAPPDATA%` + `APP_IDENTIFIER`），**代码侧不写字面量**；文档里出现的 `com.assistant` 只是常量当前值，不是第二份来源。WebView2 在该 user data folder 下固定创建 `EBWebView/` 子目录，因此它恒落在本地基根首层——正是对账要覆盖的那一层。

**落点与现状完全一致，不做迁移。** `tauri.conf.json` 的窗口 `create` 不动（配置窗口无法声明 `data_directory`，仍由 Tauri 强制设为 `app_local_data_dir()`；编程创建路径显式设成同值，两条路径因此共用同一个 `WebContext`）。不重命名目录、不迁数据、不产生第二份 profile；应用接管的是「声明 + 创建 + 回收 + 观测」，不是窗口创建链路本身。

**首段进 `dir_name`，与 `E12` / `E13` 完全同形。** `E12`（`logs`）与 `E13`（`crashes`）的 owner 同样是外部组件（`tauri-plugin-log` / `assistant-crash`），目录名同样不是应用定义的，但因为 `AppDirs` 要拼 `<local>/logs`、`<local>/crashes` 做统计与清理，名字就放在 `dir_name`。`E14` 同理：`AppDirs` 要拼 `<local>/EBWebView`（观测、创建、回收都用它），故 `dir_name::WEBVIEW_PROFILE = "EBWebView"` 并进 `dir_name::ALL`。**V19 规则 3 无需豁免**——首段就是 `dir_name` 常量，该规则防拼错 / 防漏登记的作用完整保留。

**与配置文件的差别在「谁拼路径」。** core 配置文件由 owner crate 用 `ConfGroup::FILE_NAME` 自己拼（`app_dirs` 不拼），所以名字留在 owner crate（N16）；`vault-config.json` 由 vault 特性用 `VaultConf::get_conf_dir()` + `FILE_NAME` 拼到 `<data>/security/`，名字与首段都在 vault crate 内；`EBWebView` 由 `app_dirs` 拼（内容写入方是外部运行时，仓库内没有可引用的常量），所以名字进 `dir_name`。判据是「谁负责构造这段路径」，不是「谁写入这些字节」——`dir_name` 的头注释本就只要求「`AppDirs` 拼路径要用的首段名」，`logs` / `crashes` 早已满足这条。

**登记表仍然不造常量。** 登记表只记录「哪个首段、归哪条条目、由谁提供」；`E14` 的 `segments` 直接写 `[dir_name::WEBVIEW_PROFILE]`，名字的唯一来源在 `dir_name`，登记表只是引用。

**漂移守卫替代「无法比对」的缺口。** 子目录名 `EBWebView` 由 WebView2 固定创建，仓库内没有可比对的常量；但接管落点后漂移不再只能靠实机核对：`setup` 在窗口创建之后（Windows 上）校验 `AppDirs::webview_profile()` 是否存在，缺失即 `warn` 并如实记为该条目 `missing`。这样 Tauri / WebView2 若改落点或改名，启动日志直接暴露，不会静默把字节漏在 `unregistered`。配置文件不同——`ConfGroup::FILE_NAME` 就在 owner crate 内，`E15` / `E17` / `E18` 与 vault 侧自注册的 `E16` 都是零字面量复制。

#### 13.3.1 准入与必报项全部满足

| 判据 | `EBWebView` | 结论 |
|---|---|---|
| 准入：有 owner | 内容写入方是 WebView2 运行时（外部组件）；落点 / 创建 / 回收 / 观测由本应用接管（§13.3 接管表） | 通过 |
| R1 上报大小 | `app_dirs::fs_stats::tree_stats` 可直接给出整棵树（与 `E5` / `E12` 同一实现） | 可履行 |
| R2 回收结论 | Chromium profile 的缓存子目录（`BrowserMetrics` / `Crashpad` / `GrShaderCache` / `GPUCache` / `component_crx_cache` / `extensions_crx_cache` / `Default/Cache` 等）删了可重建；用户态部分（Cookies / Local Storage / IndexedDB / Login Data）不删 → `RebuildCache` / `DerivedCache` / `Eligible`（运行期只登记，启动期执行） | 满足 |
| R3 scope / coverage | `Local` + `RecursiveTree`（整棵子树归该条目） | 满足 |
| R4 风险档 | `Medium`（清缓存会丢登录态 / 会话，属「可重建但影响体验」→ `Confirm`） | 满足 |

成本两项都可接受：`scope = Local` 已存在；统计复用 `tree_stats`，不造第二套口径。

#### 13.3.2 登记字段草案

| 字段 | 取值 | 理由 |
|---|---|---|
| `entry` | `E14`（`E7`–`E11` / `E16` 已被可选分区占用，core 顺延） | 编号是文档内部槽位，不是跨线身份（§4） |
| `key` | `EBWebView` | 唯一跨线身份，V19 断言 `key == segments.join("/")`；`lexicon.ts` 必须补同名键（G7） |
| `scope` | `Local` | 目录在本地基根（`%LOCALAPPDATA%` + `APP_IDENTIFIER`，与 `AppDirs::local()` 同值）下 |
| `segments` | `[dir_name::WEBVIEW_PROFILE]` | 首段即对账单位；名字的唯一来源在 `dir_name`（与 `E12` 的 `logs` 同形），**进** `dir_name::ALL`，V19 规则 3 无需豁免；登记表只引用、不造常量 |
| `coverage` | `RecursiveTree` | 整棵子树都归该条目 |
| `owner` | `WebView2`（内容写入方；落点 / 创建 / 回收由本应用接管） | 展示标签指「统计来源」，不是「应用自有实现」；不得写成应用侧类型名 |
| `risk` | `Medium` | 清缓存会丢登录态 / 会话，属「可重建但影响体验」→ `Confirm` |
| `inventory` | **`true`** | 见 §13.3.3 |
| `action` / `reason` / `eligibility` | `RebuildCache` / `DerivedCache` / `Eligible`（运行期只登记，启动期执行） | 与 `E4` 同形；资格必须保持 `eligible`，否则前端无清理入口（§13.3.5 第 1 条） |

#### 13.3.3 `inventory` 必须为 `true`（本节的技术硬约束）

`inventory = false` 会让 `reconciled()` 返回 `false`、`build_node` 跳过 `observe`，于是 `scanBytes = null`；再经 §3.6 的 `null` 传染，`totals.scanBytes` 整条变 `null`，分母消失——与本规格的目的正好相反。`E12` 能 `false`，是因为它的占用已由 breakdown 侧（`sample_logs`）报出，对账侧不做减法仍有第二个口径；`EBWebView` 没有第二个口径，`false` 等于把它从所有统计里删掉。

#### 13.3.4 已记录 / 可回收的划分

| 量 | 取值来源 | 语义 |
|---|---|---|
| `EntrySample.bytes`（已记录） | `tree_stats(EBWebView)` | 整棵树，与扫描同一棵树 ⇒ `residualBytes` 只剩噪声量级 |
| `EntrySample.reclaimable`（可回收） | 缓存子目录求和 | 「可重建」的那部分；与 `bytes` 是两个量，和 `E4` 同形 |

关键取舍：**不把用户态部分算进 `residualBytes`**。Cookies / Local Storage / IndexedDB / Login Data 在语义上是条目自持的用户数据，不是「没人认领的余量」；把它们塞进 residual 会让大块用户态数据被标成「未认领」，误报比漏报更糟。正确表达是「可回收 < 已记录」，由 `reclaimable` 承载。

#### 13.3.5 执行时机：运行期只登记，启动期才删

WebView2 运行中持有 profile 文件句柄，运行期删除会失败或损坏 profile。因此回收由应用接管、但**只在没有任何 webview 存活时执行**。这里必须区分「资格」与「执行时机」两件事：

1. **运行期资格恒 `eligible`**（`eligibility::webview_eligibility` 按 `availability = available` / `missing` 定档，`reason = DerivedCache`）。**不能**把运行期说成 `locked`：`entryView` 只在 `eligible` 时渲染清理入口，资格一旦是 `locked`，用户根本发不出请求，「记录待执行」这条链路就成了死代码——与「登记即管理」正好相反。运行期删不掉是**执行时机**问题，不是资格问题；
2. **执行结果用 `scheduled` 表达**（`StorageReclaimStatusDto` 的封闭枚举新增值）：执行器只写标记（`mark_pending`），零删除，返回 `status = scheduled` + `removed = 0`。与 `skipped` 的区别是「已登记、有明确后续执行点」而非「没做也不需要做」；与 `done` 的区别是「本次没有删除任何字节」。前端据此显示「已登记清理请求，将在下次启动时执行」，不假装删过、不静默吞掉；
3. **执行时机**：用户请求清理 → 记「待执行」→ 下次启动在 `Builder::run` 之前（webview 尚未创建、目录未被打开）执行白名单删除。`setup` 钩子晚于配置窗口创建（`tauri-2.12.5/src/app.rs` 的 `setup()` 先建配置窗口再调用户钩子），因此**不能**把清理放在 `setup`；`main` 里在 `Builder::run` 之前调用 `ensure_webview_profile()` 与 `run_pending_purge()`，这是唯一安全点；
4. **白名单**：只删缓存子目录（`BrowserMetrics` / `Crashpad` / `GrShaderCache` / `GPUCache` / `component_crx_cache` / `extensions_crx_cache` / `Default/Cache` 等），用户态部分（Cookies / Local Storage / IndexedDB / Login Data）一律不删；删除前逐项校验存在，失败如实计数不吞错；
5. **标记文件**：`<profile>/reclaim-pending.tmp`，`.tmp` 与普通文件同口径计入 `tree_stats` 的 `bytes` / `count`，因此也计入 `E14` 的已记录（8 B，只存在到下次启动清理为止）；它不在缓存白名单子目录内，故不计入 `reclaimable`。标记只消费一次，清完即删。
   **伴生文件口径（取代 I61 §3.9 的「不计入」）**：`tree_stats` 的 owner 条目（`E5` / `E12` / `E14`）把伴生文件与普通文件一并计入 `bytes` / `count`，与对账扫描同一口径——它们确实占盘，扫描侧也计入 `scanBytes`，两侧同口径才不会留下「已登记·未计入」的假余量；首层伴生文件（如 `assistant.db-wal`，其 owner 是只观测自身的文件条目）仍归 `companion`、不带登记值。

这是 T5 执行器的实现契约；本规格规定「资格恒 `eligible` + 执行返回 `scheduled`」这一组合不得退化成运行期硬删，也不得退化成「资格 `locked`、请求发不出去」。

#### 13.3.6 已知噪声（如实记录，不加容差字段）

`declared` 与 `scan` 是两次独立遍历（I61 纪律：不复用同一次观测），而 `EBWebView` 在持续写入，因此 `residualBytes` 会出现 ± 写入量的噪声，偶发 `overRegistered`。接受此噪声，不为此新增容差字段（OQ6）；若实测频繁再议。

`complete` 判定与 `E12` 同处理：`assemble` 排除「采样期间持续写入」的条目（`Logs` / `WebViewProfile`），否则整页 `complete` 会因一个持续写入的条目恒为 `false`，把「完整性承诺」变成一句永远不成立的话。

#### 13.3.7 跨平台

`EBWebView` 是 WebView2 专属的目录名，只有 Windows 存在。Linux 上 WebKitGTK 把 `data_directory` 当自己的 base data / cache 目录用，不建 `EBWebView` 子目录；macOS 上 `data_directory` 只作为 `WebContext` 的键，WKWebView 用系统默认存储，不读这个路径（wry 的 macOS `WebContext` 实现忽略该值）。因此应用即使统一声明 `webview_data_directory()`，该条目在 Linux / macOS 仍恒为 `missing`（`missing` 是既有合法状态，I61 §7.1）；`ensure_webview_profile()` 在非 Windows 直接返回，不建空目录，避免把「不存在的落点」造成 0 B 假条目。Linux 上 WebKitGTK 若在基根首层留下名字不同的目录，按既有规则落 `unregistered`，不在本节范围。不做 `#[cfg]` 门控——登记表不参与特性 / 平台门控（见模块头注释），`dir_name::WEBVIEW_PROFILE` 也无条件存在。若将来要按平台隐藏，需给 `ManagedRoot` 加可选段，属结构变更（OQ7）。

### 13.4 四个配置文件：登记（`E15` / `E16` / `E17` / `E18`）

四个配置 JSON 由本应用自己写入、owner 明确——准入通过；单文件大小 `metadata` 一读即得，R1 可履行。登记后它们从 `unregistered` 移入 `declared`，主信号不再把「有 owner 的文件」算成「没人认领」。

**归属分两侧**：`config.json` / `orb-config.json` / `mascot-config.json` 由 core 登记表登记（`E15` / `E17` / `E18`）；`vault-config.json`（`E16`）的 owner 在可选 crate `assistant-vault` 内，core 侧**不持有任何相关条目知识**——不登记、不写字面量、不留枚举分支，由 vault 特性在自己的可选分区内自注册（首段 `security/`，与 `E7`–`E11` 同分区，是该分区的第六条）。可选特性未挂载时该条目整体不存在（无占位行、无假余量）。

#### 13.4.1 准入与必报项

| 判据 | 四个配置 JSON | 结论 |
|---|---|---|
| 准入：有 owner | `AppConf` / `VaultConf` / `OrbConf` / `MascotConf` 各自持有文件名与路径 | 通过 |
| R1 上报大小 | 各自 `fs_stats::file_stats` 即给出 `declared`（`metadata.len()`） | 可履行 |
| R2 回收结论 | 永不回收（`vault-config.json` 含远端 URL 凭据，其余为应用运行配置）→ `None` / `NotEligible` / `BusinessData` | 满足 |
| R3 scope / coverage | `E15` / `E17` / `E18` 在 Windows 上落在数据基根首层（N14）→ `Data` + `SingleFile`；`E16` 落在 `<data>/security/` 首层 → `Data` + `SingleFile` | 满足 |
| R4 风险档 | `Forbidden`（禁止任何回收动作） | 满足 |

旧版用两条错误理由拒绝登记：①体量门槛（旧 D1「合计 1.6 KB、无独立观测价值」）已随 §13.2.2 废止（X9）；②「配置类不在登记表范围内」实质是把「能否统计」当准入条件，现已改为「有 owner 就登记、登记即必报大小」（R1）。成本侧的「`scope` 只有 `Data` / `Local` 两个基根」本来就不是障碍（`missing` 是合法状态，§13.4.4）。

#### 13.4.2 登记字段草案

| `entry` | `key` | `segments` | `scope` | `coverage` | `owner` | `risk` | `inventory` | `action` / `eligibility` / `reason` |
|---|---|---|---|---|---|---|---|---|
| `E15` | `config.json` | `[<AppConf as ConfGroup>::FILE_NAME]` | `Data` | `SingleFile` | `AppConf` | `Forbidden` | `true` | `None` / `NotEligible` / `BusinessData` |
| `E16` | `security/vault-config.json` | `[SEGMENT, <VaultConf as ConfGroup>::FILE_NAME]`（vault crate 内 `storage_section::ENTRIES`，core 侧零知识） | `Data` | `SingleFile` | `VaultConf` | `Forbidden` | `true` | `None` / `NotEligible` / `BusinessData` |
| `E17` | `orb-config.json` | `[<OrbConf as ConfGroup>::FILE_NAME]` | `Data` | `SingleFile` | `OrbConf` | `Forbidden` | `true` | `None` / `NotEligible` / `BusinessData` |
| `E18` | `mascot-config.json` | `[<MascotConf as ConfGroup>::FILE_NAME]` | `Data` | `SingleFile` | `MascotConf` | `Forbidden` | `true` | `None` / `NotEligible` / `BusinessData` |

`key == segments.join("/")` 由 V19 断言；`lexicon.ts` 必须补三个 core 条目同名键（G7），`security/vault-config.json` 的名称由 vault 特性模块在加载时自注册（`registerEntryNames`），core 词表不持有它。

#### 13.4.3 结构影响（实施必须同步的六处）

1. **文件名来源**：`dir_name` 不加任何配置常量——`app_dirs` 既不拥有这些文件，也不负责命名；core 登记表也不为它们造常量。`E15` / `E17` / `E18` 的 `segments` 直接写 owner 常量（`<AppConf as ConfGroup>::FILE_NAME` / `<OrbConf ...>` / `<MascotConf ...>`），字面量只有一份。`E16` 的文件名在 vault crate 内**同样只取自 `<VaultConf as ConfGroup>::FILE_NAME`**（零字面量复制），core 侧不出现该文件名；
2. **路径对齐**：`VaultConf::get_conf_dir()` 返回 `<data>/security/`，使条目首段等于 vault 分区首段（`SectionRegistry::register` 校验）；旧位置 `<config>/vault-config.json` **不迁移**，留盘即视为未登记余量；
3. **登记表与计数**：core `MANAGED_ROOTS` 只加三条（`E15` / `E17` / `E18`），`EXPECTED_ENTRY_COUNT` / `CORE_ENTRY_IDS` 同步（与 T5 的 `E14` 一起算，最终 core 12 条）；`E16` 计入 vault 分区条目数（`storage_section.rs` 单测断言分区条目 6 条），不计入 core；
4. **静态档**：core `decl.rs::static_for` 补三个分支（`E15` / `E17` / `E18`，`None` / `NotEligible` / `BusinessData`）。`static_for` 的 `unreachable!` 会在装配时对未声明条目 panic，这一步不能漏（G12）；`E16` 的静态档由 vault 分区的 `stats` / `eligibility` 分支给出，core 侧无分支；
5. **采样**：`sample/rows.rs` 新增 `sample_config_file(path)`（`from_io(fs_stats::file_stats(path))`，与 `E6` 同形），`collect.rs` 为 core 三条各派一个任务，路径由 `AppDirs::managed_path_of(entry)` 按登记表拼出；`E16` 由 vault 分区自身观测（`VaultStorageSection::stats`），路径由插件初始化时传入的 `security_dir.join(VaultConf::FILE_NAME)` 提供。**漏采样 = `EntrySample::missing()` → `declared = 0` → 整个文件被误报成余量**（G11）；
6. **前端**：core `lexicon.ts` 的 `ENTRY_NAME_KEYS` 补三个键，`zh-CN` / `en-US` 的 `settings.storage.entries.names` 补三条，`lexicon.test.ts` 的 `ENTRY_KEYS` / `ENTRY_NAME_PATHS` 同步（G7 / V10）；`security/vault-config.json` 的文案键由 vault 特性模块调用 `registerEntryNames` 自注册，core 侧无该键。

#### 13.4.4 跨平台与成本

Linux / macOS 的 config 基根与数据基根分离（N14），`E15` / `E17` / `E18` 不在数据基根下，因此登记后这两平台恒为 `missing` / `0 B`；`E16` 已对齐到 `<data>/security/`（数据基根内），三平台落点一致。`missing` 是既有合法状态（I61 §7.1），`complete` 不受影响（V14）；不做 `#[cfg]` 门控（登记表不参与特性 / 平台门控）。要按平台隐藏需给 `ManagedRoot` 加可选段，属结构变更（同 OQ7 的结论）。

伴生临时文件（`config.json.tmp` 等）不认领：`atomic_write` 的中间态按既有规则落 `unregistered`，要收则走伴生后缀规则扩展，属独立变更（OQ8）。

#### 13.4.5 `IGNORED_NAMES` 不作为选项

该表是**首层名全局匹配**（`classify` 对 `data` / `local` 两个 scope 共用），加 `config.json` 会连带忽略任何基根下同名首层；且其语义是「已知系统垃圾、不认领也不管」，把应用自有文件塞进去等于用白名单抹掉观测，与 I61「不知道不许写成 0」同源原则冲突（X8）。正确做法是登记（本节），不是屏蔽。

### 13.5 `probe-config.json`：保留、不登记（无 owner，准入不通过）

曾按「无引用即可清理」判断删除，随后**恢复**。在新的判据下它的处置有唯一解：它没有 owner（旧「探针」特性 I24 已归档，代码零引用），**没人能履行 R1 的上报义务**，因此不登记；同时它确实是本应用留下的字节，不该删。

保留的理由：

1. 四个配置登记后，它是 `unregistered` 区里**唯一剩下的应用自有条目**，是 `totals` 分子 / 分母关系的现成样本（V15）；
2. 45 B 的体积对总量无影响，保留成本可忽略；
3. 「特性下线后遗留的配置」是**普遍现象**，不是本条目独有的错误；保留它让这一类问题在界面上持续可见，而不是被一次性清理掩盖。

因此该条目**保留、不登记**。若将来要「认领」它，前置是给它一个 owner（例如一条只读的 `LegacyConf` 声明），使 `declared` 有来源——那是独立决策，不在 T6。若要清理历史配置，必须作为独立的迁移项（带版本判据与用户确认），不得借「未登记」之名顺手删除。

### 13.6 边界

1. §13.3 / §13.4 的登记是**独立切片**（`EBWebView` → T5，四个配置文件 → T6），不属于本规格 T1–T4 的实施范围；I67 对它们的唯一要求是「T1 / T2 落地后，这些字节必须出现在分母里」（无论是否已登记）。
2. 本节不引入门禁编号冲突：G1–G10 / V1–V12 不变；新增的 G11（登记必须上报大小，义务而非准入）与 G12（登记链路同步）只约束登记本身，`E14` / `E15` / `E17` / `E18` 仍由既有门禁覆盖（V19 首段一致性：`E14` 用 `dir_name` 常量、`E15` / `E17` / `E18` 用 owner 常量，均无需豁免；G7 前端 `lexicon`）。`E16` 不在 core 登记表内，其首段 `security` 与文件名均由 vault 分区在 crate 内自持，特性边界门禁（core 侧禁止出现可选特性名与条目路径）由 `coreBoundary.test.ts` 覆盖。
3. **不允许**为了让「未登记区变干净」而登记任何条目：准入只看「有没有 owner」，登记即承担上报大小义务（R1）并声明 R2–R4；**没有 owner 的条目**（如 `probe-config.json`）留在 `unregistered`，那正是该类的正当样本。
4. 本节不改 `IGNORED_NAMES` / `OPTIONAL_SEGMENTS`：它们分别是「系统垃圾」与「可选分区首段事实」，与本节处置无关。
5. `probe-config.json` 的保留不构成登记先例：它属于本应用但没有 owner，准入不通过（§13.5）。
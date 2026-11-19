# I61 — 存储管理器规格（资源监视条目 + 回收）

| 项 | 值 |
|---|---|
| 状态 | 草稿（设计定案前；观测承接原 I54 存储 tab（已迁入本文），新增回收与策略治理） |
| 日期 | 2026-11-19 |
| 关联 | [I54 资源监视器规格](./I54-resource-monitor-spec.md)（存储 tab 已由本文承接并在 I54 中收敛为指针；进程 / 自身占用仍归 I54）、[STORAGE-MONITOR-ANALYSIS](../analysis/STORAGE-MONITOR-ANALYSIS.md)（存储观测机制的唯一事实基线）、[CODE-SPLIT-PRINCIPLE](../principle/CODE-SPLIT-PRINCIPLE.md)（拆分原则唯一来源，本文只引用不复述）、I44 应用目录管理规格（已归档）（目录归属与字面量门禁）、I58 超级工具规格（已归档）（`tool-index/` 的 owner 与缓存边界） |
| 影响范围 | 后端：`src-tauri/src/cmd/storage.rs`（快照 + 计划 + 执行）、`src-tauri/src/protocol/dto/storage.rs`（DTO 扩展）、`src-tauri/src/ipc.rs`（命令注册）、各 owner 的 stats / 回收方法（`assistant-storage`、`assistant-tools`、`assistant-vault`、`assistant-crash`）；前端：`src/settings/pages/StorageSettings.tsx`、`src/services/desktop/storageService.ts`、i18n 两份 locale、`src/settings/__tests__/fixtures/*.json`（golden 投影） |

> 一句话：存储管理器把「存储占用」从监视器的一行提升为**可观测、可计划、可回收**的资源监视条目集合——观测只读、回收只委托；任何删除都必须先有 owner 的方法，任何数字都必须带可用性与采样时刻，任何「不知道」都不许写成 `0`。

本文中的「必须 / 禁止」为可验收约束；「应当」为默认做法，偏离需在代码注释里写明理由。

**编号约定**：`E1–E13` 是资源监视条目（Entry），`A1–A7` 是回收动作，`Q1–Q5` 是资格状态，`N1–N14` 是现状事实，`G1–G12` 是门禁，`V1–V15` 是验收判据，`T1–T4` 是实施切片，`X1–X10` 是拒绝方案，`OQ1–OQ10` 是开放问题。

---

## 1 定位与边界

### 1.1 存储管理器是什么

存储管理器是**存储类资源监视条目的唯一观测与回收入口**：

1. 它承接原 [I54](./I54-resource-monitor-spec.md) 存储 tab 的全部内容（12 行观测表、D1–D4 四个方向、三个口径、账本闭合），I54 §4 现已收敛为指向本文的指针；本文把 12 行扩展为 13 行；
2. 它新增原 I54 明确推迟的**回收动作**与**策略治理**（I54 §1.3 现已改为指向本文）；
3. I54 收敛为「资源监视器」，只保留**进程**与**自身占用**两类资源监视条目；存储类条目不再由 I54 维护；
4. 它不是新 crate：落在应用层 `src-tauri/src/cmd/storage.rs`，因为它需要 `AppDirs` 与各 owner 的 stats / 回收方法（与 [STORAGE-MONITOR-ANALYSIS](../analysis/STORAGE-MONITOR-ANALYSIS.md) §10 第 1 条一致）；
5. 它**不拥有任何目录**：目录名与基根只来自 `AppDirs` 与可选能力模块的 getter，本文不新增目录名、不新增目录字面量。

**「资源监视条目」的定义**：一个条目是一个**受管资源身份**，不是一个路径。条目的 `key` 是稳定标识（如 `attachments`、`security/audit`），前端不得据此拼路径；条目的 `bytes` / `count` 是**观测字段**（占用），不是条目本身。把条目理解成「目录」会立刻导致两个错误：自行遍历、自行删除。

### 1.2 三个身份在存储侧的落点（术语定义见 I54 §2.1）

术语（所有者 / 观测者 / 回收者）由 [I54 §2.1](./I54-resource-monitor-spec.md) 定义，本文不重述；本节只写它们在本管理器里的**落点**，可验收的硬约束见 §3.8 的 `G1`–`G3`：

| 身份 | 在本管理器里的落点 |
|---|---|
| **所有者** | `AppDirs`、可选能力模块（如 `cmd/vault`）、各库的库内子布局 |
| **观测者** | 各 owner 的 stats / list 方法 |
| **回收者** | 各 owner 的回收方法（删除、裁剪、轮转、清扫） |

这条分工对应 I44（已归档）的两条门禁：目录字面量只允许命中 `app_dirs::dir_name` 与可选能力模块；`dirs::` 只允许出现在 `app_dirs` 与 `assistant-config`。存储管理器一旦自己拼路径 + 遍历 + 删除，两条门禁同时失效，且三个身份退化成一份实现。

### 1.3 纳入与排除

| 纳入 | 排除 |
|---|---|
| 13 个存储类资源监视条目的观测（`E1–E13`） | 进程与自身占用（归 I54） |
| 每个条目的回收资格、策略、计划、执行、结果 | 任务级资源归因、进程登记表、停止 / 回收进程树 |
| 附件孤儿判定与删除、过期清扫、数量裁剪、可重建缓存清理 | 数据库 `VACUUM`、日志轮转实现、vault git 历史整理 |
| 回收留痕（应用日志）与账本闭合复算 | 跨 tab / 跨条目的「总占用」展示 |
| 设置页存储 tab 的观测表与回收入口 | 新增目录、新增存储介质、云同步、配额计费 |

### 1.4 与现有命令和页面的关系

| 既有物 | 本文处理 |
|---|---|
| `get_storage_breakdown` | **扩展**，不重定义旧字段；新增字段（`sampledAt`、`byteBasis`、`sharedBytes`、`referencedOnceBytes`、`entries`）的定义在本文 §6.2，条目上再追加 `reclaim` |
| `delete_attachments` | **保留**，仍是「用户显式选中附件」的唯一入口（`A2`）；存储管理器的 `reclaim_storage` **不**接管它，避免两条删除路径 |
| `delete_derived_text` | **保留**，仅随附件删除调用；管理器不直接调用 |
| `src/settings/pages/StorageSettings.tsx` | **并入** I54 §3 的三 tab 资源监视器页：页面壳由 I54 定（R0 先建容器并留三个 tab 挂载位），本文只提供**存储 tab 的内容与交互契约**（§6.3），不重复页面决策 |
| `get_storage_reclaim_plan` / `reclaim_storage` | **新增**（§6.1），只服务策略派生动作（`A1` / `A3` / `A4` / `A5`） |

---

## 2 现状事实（以代码为准）

| # | 事实 | 落点 |
|---|---|---|
| N1 | `AppDirs` 已暴露 7 个无条件入口：`database` / `attachments` / `tool_results` / `indexes` / `mascot_assets` / `logs` / `crashes`；`indexes()` 是 I58 新增，I54 N6 的「6 个入口」已过时 | `src-tauri/src/app_dirs/roots.rs`、`dir_name.rs` |
| N2 | 附件观测走 `list_all`，只取第一层 `is_file()`；`AttachmentEntry.created_at` 实际取 `metadata.modified()`（I54 N5 的命名缺陷仍在） | `src-tauri/crates/assistant-storage/src/attachment_store.rs` |
| N3 | 附件删除有三条既有路径：`delete_attachments`（先删 `attachment_refs`，再逐文件删；任一 ID 非法则整体失败）、`delete`（单个）、`clean_tmp`（只清第一层 `.tmp`） | `src-tauri/crates/assistant-storage/src/attachment_store.rs`、`src-tauri/src/cmd/storage.rs` |
| N4 | 孤儿判定已在 `get_storage_breakdown` 内实现：磁盘集合 − `attachment_refs` 引用集合；但**没有任何删除孤儿的入口** | `src-tauri/src/cmd/storage.rs` |
| N5 | `tool-results/` 的清理是 `cleanup_expired_results(root, days)`：只删常规文件、不跟随符号链接、`0` 禁用、失败只计数；**只在启动线程调用一次**，无 stats、无运行期 / 手动入口 | `src-tauri/crates/assistant-tools/src/runtime/storage.rs`、`src-tauri/src/tool_setup.rs` |
| N6 | `tool-results/` 的保留天数启动读一次配置；**读配置失败时回退常量 30 天**（`RESULT_RETENTION_DAYS`），而审计侧读失败回退 `0`（不清理）——两处 fail 策略相反 | `src-tauri/src/tool_setup.rs`、`src-tauri/crates/assistant-config/src/constant.rs` |
| N7 | `tool-index/` 的缓存有 64 MiB 内存 LRU，逐出时删对应落盘文件；单索引文件上限 64 MiB；索引文件名含 revision，**没有启动时全目录清扫**，跨进程 / 跨 revision 的残留不在 LRU 记账内 | `src-tauri/crates/assistant-tools/src/os/tools/super_index/cache.rs`、`format.rs` |
| N8 | 素材写入走 `atomic_write`，临时文件名是 `path.with_extension("tmp")`（如 `custom.tmp`）；卸载只删允许扩展名与 manifest，**没有 `.tmp` 清扫** | `src-tauri/crates/assistant-config/src/atomic_write.rs`、`src-tauri/src/cmd/mascot_asset.rs` |
| N9 | `assistant.db` 只有文件长度可读；`-journal` / `-wal` / `-shm` 不计入（本文 §3.9 包含范围），无回收 | `src-tauri/src/protocol/dto/storage.rs`、`src-tauri/src/setup.rs` |
| N10 | 安全库备份走 `VACUUM INTO`，每次备份后 `prune_backups(MAX_BACKUPS = 5)`；`purge_backups` 存在但无对外入口；无 stats | `src-tauri/crates/assistant-vault/src/security_db/state.rs`、`mod.rs` |
| N11 | 审计 JSONL 有保留期（默认 90 天，`0` 禁用）：`prune_before(cursor_ms, days)` 必须同时满足「早于补录游标日期」与「早于今天 − 保留天数」，当日文件永不删；DB 审计行在启动时按保留期裁剪，读配置失败按 `0` | `src-tauri/crates/assistant-vault/src/audit/jsonl.rs`、`security_db/state.rs`、`src-tauri/src/setup.rs`、`cmd/vault/mod.rs` |
| N12 | `security/recovery/` 在 `vault_setup` 与恢复流程后由 `cleanup_recovery_dir` 清理；无 stats | `src-tauri/src/cmd/vault/mod.rs`、`recovery.rs` |
| N13 | `logs/` 由 `tauri-plugin-log` 轮转（`KeepSome(3)` + 10 MiB）；插件不暴露 stats，应用侧无法枚举 | `src-tauri/src/plugins.rs` |
| N14 | `crashes/` 每次事件写 `panic_<ts>.log` / `crash_<ts>.dmp` / `crash_<ts>.log` 三个文件；`assistant-crash` 无 `list` / `stat` / `prune`（I54 N14） | `src-tauri/crates/assistant-crash/` |

**事实结论**：13 个条目里，真正「观测 + 回收」都已有的只有附件（观测 + 显式删除）与安全备份（自动裁剪）；`tool-results` / 审计有回收但无观测与手动入口；`tool-index` / 派生文本 / `crashes` 有回收欠账；`logs` 由插件管；`vault` / `assistant.db` / `security.db` 禁止回收。

---

## 3 资源监视条目（13 行）

### 3.1 条目表

| ID | key | owner | 观测入口（现状） | 回收入口（现状） | 本期动作 | 风险 |
|---|---|---|---|---|---|---|
| E1 | `attachments` | `AttachmentStore`（`assistant-storage`） | `list_all`（第一层文件） | `delete_attachments` / `delete` / `clean_tmp` | `A1` + `A2` | 高（用户数据） |
| E2 | `attachments/derived` | `AttachmentStore` | 无 | `delete_derived_text`（随附件删除） | `A5`（欠账） | 低（可重建） |
| E3 | `tool-results` | `FileToolResultStorage`（`assistant-tools`） | 无 | `cleanup_expired_results`（启动一次） | `A3` | 中（回放定位可能失效） |
| E4 | `tool-index` | `SuperIndex` 缓存（`assistant-tools`） | 无 | 内存 LRU 逐出删文件；单文件 64 MiB | `A5`（欠账） | 低（可重建） |
| E5 | `mascot-assets` | `cmd/mascot_asset` | 无 | 卸载 / 覆盖 | 无（owner 管） | 高（用户素材） |
| E6 | `assistant.db` | `DbState` | 文件长度 | 无 | 无（禁止） | 禁止 |
| E7 | `security/security.db` | `SecurityDbState` | 无 | 备份时裁剪审计行 | 无（禁止 / 锁定） | 禁止 |
| E8 | `security/backups` | `SecurityDbState` | 无 | `prune_backups(5)`（备份时） | `A4`（自动，无对外入口） | 中（派生副本） |
| E9 | `security/vault` | vault repo | 无 | 无 | 无（禁止） | 禁止（用户数据） |
| E10 | `security/recovery` | `cmd/vault` | 无 | `cleanup_recovery_dir`（启动 / 恢复后） | `A3`（owner 自动） | 低（瞬态） |
| E11 | `security/audit` | `SecurityDbState` / `AuditJsonlWriter` | 无 | `prune_before` + `cleanup_expired_audit` | `A3`（仅 owner 调度） | 中（历史决策证据） |
| E12 | `logs` | `tauri-plugin-log` | 无 | 插件轮转 `KeepSome(3)` + 10 MiB | `A6`（仅展示） | 低 |
| E13 | `crashes` | `assistant-crash` | 无 | 无 | `A4`（欠账） | 低（诊断产物） |

**与 I54 原 12 行的差异**：新增 `E4 tool-index`（I58 之后才出现的受管目录）；I54 说「安全域各子目录无保留策略」已过时（N11 的审计保留已存在）；I54 说「回收不在本期」由本文取代。其余 12 行的身份与字节口径沿用原 I54 §4 的结论（该节已改为指向本文的指针，事实基线仍为 [STORAGE-MONITOR-ANALYSIS](./../analysis/STORAGE-MONITOR-ANALYSIS.md)），不在此重述。

### 3.2 条目字段

条目 DTO **由本文定义**（原在 I54 §8.2，随存储 tab 一并迁入，I54 已不再维护该结构）；旧字段含义不变，新增回收视图：

| 字段 | 类型 | 约束 |
|---|---|---|
| `key` | `String` | 稳定身份，不是路径；前端不据此拼路径 |
| `layer` | 封闭枚举 | `data` / `local`；安全域条目仍是 `data`，靠 `security/` 前缀分组，**不新增第三层** |
| `owner` | `String` | 提供 stats 的 owner 名，用于「由 X 提供」文案 |
| `bytes` / `count` | `u64 \| null` | 取不到数为 `null`，禁止填 `0` 冒充；**唯一例外**是 `availability = missing`（确实没有，计 `0`，见 I54 §2.4）；键恒在 |
| `oldestAt` / `newestAt` | `i64 \| null` | 输出侧 `Option` 不加 `skip_serializing_if`；键恒在 |
| `availability` | 封闭枚举 | I54 §2.4 词表 |
| `availabilityReason` | `String \| null` | `notImplemented` 时必填（点名缺的方法）；其余非 `available` / `missing` 状态应填错误类别；`available` 与 `missing` 为 `null`；键恒在 |
| `includesDerived` | `bool` | 口径进 DTO，不只进文档 |
| `countsSelfLogs` | `bool` | 自指标注；`logs` 为 `true` |
| `reclaim` | `ReclaimViewDto` | **必填**，无回收能力的条目用 `action = none` 表达，不用 `null` |

`ReclaimViewDto`：

| 字段 | 类型 | 约束 |
|---|---|---|
| `action` | 封闭枚举 | `A1–A7`（§3.3） |
| `eligibility` | 封闭枚举 | `Q1–Q5`（§3.4） |
| `reason` | 封闭枚举 \| `null` | `notEligible` / `notImplemented` / `locked` / `ownerManaged` 必填；`eligible` 允许 `null` |
| `policy` | `ReclaimPolicyDto` | 必填；无策略用 `source = none` 表达 |
| `reclaimableBytes` / `reclaimableCount` | `u64 \| null` | 只对 `eligible` 给数；必须 `≤ bytes`；`bytes` 为 `null` 时本字段也 `null` |
| `confirmation` | 封闭枚举 | `none` / `confirm` / `strongConfirm`（§3.6） |
| `autoActive` | `bool` | owner 已在启动 / 写入路径自动执行（如备份裁剪、审计裁剪） |
| `note` | `String \| null` | 只放「缺哪个 owner 方法」这类结构性说明，不放路径、不放临时错误 |

### 3.3 动作词表（封闭）

| ID | 动作 | 语义 | 允许条目 | 幂等 | 确认 |
|---|---|---|---|---|---|
| A1 | `deleteOrphans` | 删除「引用集合之外」的对象 | `E1`（`E2` 后续） | 是（已删的跳过） | `strongConfirm` |
| A2 | `deleteSelected` | 用户显式选中后删除 | `E1` | 是 | `confirm`（既有语义） |
| A3 | `sweepExpired` | 按时间判据删除 | `E3` / `E10` / `E11` | 是 | `confirm`；owner 自动路径除外 |
| A4 | `pruneToLimit` | 按数量上限保留 | `E8` / `E13` | 是 | `confirm`；owner 自动路径除外 |
| A5 | `rebuildCache` | 删除可重建缓存 | `E2` / `E4` | 是 | `confirm` |
| A6 | `rotate` | owner 内部轮转 | `E12` | 是 | `none`（仅展示，不执行） |
| A7 | `none` | 无回收能力 | 其余 | — | `none` |

**动作边界**：

1. `A2` 由既有 `delete_attachments` 执行，**不经过** `reclaim_storage`；`reclaim_storage` 只接 `A1` / `A3` / `A4` / `A5`；
2. `A1` 的「引用集合」必须由 owner 在**执行时重算**，不能直接信任计划里的孤儿列表；计划只给数量与字节；
3. `A6` 只用于展示「已由插件轮转」，管理器不得调用插件内部 API；
4. `A5` 的「可重建」必须由 owner 自述；管理器不得自行判断某个文件「看起来像缓存」。

### 3.4 资格状态（封闭）

| ID | 状态 | 含义 | 典型条目 |
|---|---|---|---|
| Q1 | `eligible` | 有 owner 回收方法、判据可算、策略可用、风险不是禁止 | `E1` 的 `A1`、`E3`、`E8` |
| Q2 | `notEligible` | 有方法但当前不满足判据，或判据不可算 | `E1` 的 `A1` 在无孤儿时、`E11` 在游标不可得时 |
| Q3 | `locked` | 存在但受保护读不到 | `E7`、`E9` 在 vault 锁定态 |
| Q4 | `ownerManaged` | 由 owner 自动管理，管理器只展示 | `E10`、`E12` |
| Q5 | `notImplemented` | owner 尚无 stats / 回收入口 | `E2`、`E4`、`E13` |

**资格判据**：`eligible` 必须同时满足 ①动作不是 `none`；②owner 方法存在；③判据可算；④策略可用且未禁用；⑤风险不是禁止。缺一即降级，**禁止**用「大概可以删」的推断。

`reason` 封闭词表：

| reason | 含义 | 适用 |
|---|---|---|
| `orphan` | 引用集合之外 | `E1` / `E2` |
| `expired` | 早于保留期 | `E3` / `E11` |
| `overLimit` | 超过数量上限 | `E8` / `E13` |
| `derivedCache` | 可重建缓存 | `E2` / `E4` |
| `transient` | 瞬态产物 | `E10` |
| `referenced` | 仍被引用 | `E1` / `E3` |
| `userData` | 用户数据 | `E1` / `E5` / `E9` |
| `businessData` | 业务数据 | `E6` / `E7` |
| `replayRisk` | 删除会让历史回放定位失效 | `E3` |
| `locked` | 受保护 | `E7` / `E9` |
| `noStats` | 缺 stats 入口 | `E2` / `E4` / `E7` / `E8` / `E11` / `E12` / `E13` |
| `noReclaimApi` | 缺回收入口 | `E2` / `E4` / `E13` |
| `externalOwner` | 外部 owner 管理 | `E12` |
| `policyDisabled` | 策略为 `0` / 未注入 | `E3` / `E11` |
| `unknown` | 判据推不出 | 所有条目 |

**硬约束**：`notEligible` / `notImplemented` / `locked` / `ownerManaged` 的 `reason` 不得为空；`notImplemented` 的 `note` 必须点名缺哪个 owner 的哪个方法（`reason` 是封闭枚举，装不下方法名），否则用户会把「未实现」误读为「系统认为没问题」。

### 3.5 策略模型

`ReclaimPolicyDto`：

| 字段 | 类型 | 约束 |
|---|---|---|
| `source` | 封闭枚举 | `config` / `constant` / `owner` / `none` |
| `retentionDays` | `u32 \| null` | 仅时间判据；`0` 表示禁用 |
| `keepCount` | `u32 \| null` | 仅数量判据 |
| `maxBytes` | `u64 \| null` | 仅体积判据（本期仅展示） |
| `effectiveSince` | `i64 \| null` | `config` 来源必填：本次启动注入时刻（秒） |
| `mutable` | `bool` | `config` 为 `true`；常量 / owner 为 `false` |
| `failClosed` | `bool` | 本期恒 `true` |

| source | 含义 | 实例 | 生效方式 |
|---|---|---|---|
| `config` | `AppConf` 字段在启动时注入 owner | `result_retention_days`（默认 30）、`audit_retention_days`（默认 90） | 改设置**下次重启**生效；`effectiveSince` 标注本次注入时刻 |
| `constant` | 代码常量 | 备份 `MAX_BACKUPS = 5`、日志 `KeepSome(3)` + 10 MiB、索引单文件 64 MiB / 内存 LRU 64 MiB | 随版本 |
| `owner` | owner 内部策略，管理器只显示「由 owner 管理」 | 索引 LRU、日志轮转 | owner 自述 |
| `none` | 无策略，只能手动 | 附件、素材、vault、两个 DB | — |

**四条硬约束**：

1. **管理器不得直接 `AppConf::load()`**。策略由 owner 自述；组合根（启动装配）可以把配置注入 owner，但管理器不能成为第 6 个配置读取入口（I54 §1.3 的「不自持目录清单」同源：管理器的自持范围越小，门禁越难被绕开）；
2. **`retentionDays = 0` 表示禁用，不表示「全部删除」**。现有两个 owner 已按此实现，管理器不得把 `0` 翻译成「无保留期」；
3. **策略读不到 / 判据推不出 → 不删**。禁止回退到默认天数，禁止「按文件数猜」；
4. **策略必须随条目一起展示**，即使当前 `notEligible`；用户需要看到「为什么现在不能删」。

**本文要求修的两处现状不一致**：

| 问题 | 现状 | 本文要求 |
|---|---|---|
| `tool-results` 配置读失败回退 30 天 | N6：会按 30 天删 | 回退 `0`（禁用），与审计侧一致；`spawn_result_cleanup` 不再自行读配置，保留天数由 owner 在启动时注入 |
| 审计 JSONL 的 `prune_before` 需要补录游标 | N11：不能用「今天 − 90 天」直接算 | 管理器**不得**提供「立即裁剪审计」；只有 owner 的补录 / 启动调度可调用，且必须传它自己的游标 |

### 3.6 风险分级与确认

| 风险 | 判据 | 条目 | 确认 | 自动 |
|---|---|---|---|---|
| 低 | 删掉只会重建 / 只影响诊断 | `E2`、`E4`、`E13` | 预览 + 一次确认 | 仅 owner 既有自动路径 |
| 中 | 删掉会让历史定位失效 / 丢失派生副本 | `E3`、`E8`、`E11` | 预览 + 确认 | 仅 owner 既有自动路径（备份裁剪、审计裁剪） |
| 高 | 用户数据 / 用户素材 | `E1` 的 `A1`、`E5` | 预览 + 显式选择 + 强确认（`E1` 的 `A2` 走既有 `delete_attachments` 的确认语义，不升级档位） | **禁止**自动 |
| 禁止 | 业务数据 / 用户数据根 | `E6`、`E7`、`E9`，以及被引用的 `E1` | 不提供入口 | **禁止** |

**硬约束**：

1. **禁止**「一键清理全部」；每个动作都必须先出计划，再按条目确认；
2. **禁止**对高风险条目做任何自动删除；孤儿附件即使「无人引用」，也必须有用户确认；
3. 强确认的文案必须写出**条目、数量、字节**三件套；只写「确定删除吗」不算强确认；
4. 后端只校验 `confirmed = true`，不校验用户输入的文本；强确认的交互由前端落实，但后端不得因此放宽计划校验。

### 3.7 引用完整性

| 条目 | 引用模型 | 能判孤儿 | 能按时间过期 | 删除后果 |
|---|---|---|---|---|
| `E1` | `attachment_refs` | 是 | 否（无保留策略） | 被引用附件永久丢失 |
| `E2` | 基文件是否存在 | 是 | 否 | 可重建，无用户可见损失 |
| `E3` | 消息体里的落盘定位，无表 | 否（全表扫描不可接受） | 是（原 I54 待定案项允许，已迁入本文） | 历史回放定位可能失效，必须提示 |
| `E4` | 无（纯缓存） | 不适用 | 是 | 重建索引，无损失 |
| `E5` | manifest + 设置引用 | 否 | 否 | 用户素材丢失 |
| `E6` | 消息 / 会话表 | 否 | 否 | 业务数据丢失 |
| `E7` | 安全库内部 | 否 | 否 | 业务数据丢失 |
| `E8` | 派生副本 | 不适用 | 否（按数量） | 少一份可恢复副本 |
| `E9` | 用户数据 + git 历史 | 否 | 否 | 用户数据丢失 |
| `E10` | 瞬态目录 | 是（存在即残留） | 是 | 无损失；但恢复进行中禁止删 |
| `E11` | 只追加，无删除引用 | 不适用 | 是（按日期 + 游标） | 历史审计证据丢失 |
| `E12` | 插件内部 | 不适用 | 是（插件） | 诊断日志丢失 |
| `E13` | 无 | 不适用 | 是 | 诊断产物丢失 |

**关键约束**：

1. `E3` 的「按时间过期」是**允许**的，但必须在 UI 上写「过期后旧回放定位可能失效」；不得宣称「不影响历史」；
2. `E11` 的「按日期」必须同时满足游标日期与保留期（N11），**禁止**用当前时间直接算截止日；
3. `E1` 的孤儿判定必须与引用写入**串行化**（§5.5），否则内容寻址去重会让「删孤儿」与「并发导入」互相踩。

### 3.8 门禁

| # | 门禁 |
|---|---|
| G1 | 管理器不得出现受管目录名 / 路径拼接；只调 `AppDirs` 与可选能力模块 getter |
| G2 | 管理器不得 `read_dir` 遍历受管目录；观测只调 owner 的 stats / list |
| G3 | 管理器不得 `fs::remove_file` / `remove_dir` / `remove_dir_all`；回收只调 owner 方法 |
| G4 | 每个条目必须有 owner getter 与 stats 入口；缺一个就不许把该条目标 `available` |
| G5 | 策略读不到 / 判据推不出 / 非普通文件 / 符号链接 → 不删（fail-closed） |
| G6 | 取不到数一律 `null` + `availability` + `reason`；禁止 `0` 冒充（例外：`missing` 计 `0`，因为「确实没有」是已知事实，不是「不知道」） |
| G7 | 观测 / 计划 / 执行三步分离；计划无副作用；执行必须带 `planId` 与 `confirmed = true` |
| G8 | 禁止自动回收用户数据（`E1` / `E5` / `E9`）与业务数据（`E6` / `E7`） |
| G9 | 回收后必须重新采样；账本恒等式必须仍成立 |
| G10 | 禁止跨条目 / 跨层求「总占用」；计划合计只在 `complete = true` 时给出 |
| G11 | 禁止全局回收锁；串行化由 owner 负责，管理器只串行化自己的执行请求 |
| G12 | 回收留痕不得写绝对路径；只写条目 key、动作、数量、字节、`planId` |

### 3.9 口径与包含范围（原 I54 §4.3，迁入后为唯一出处）

**字节口径**：

1. 只算文件长度（`metadata.len()`），不算磁盘占用 / 表观大小；目录自身与空目录不计入；
2. 符号链接与非常规文件（设备节点、FIFO 等）不计入 `bytes`，也不得成为回收对象（G5，fail-closed）；
3. 同一物理文件被多处引用时，字节只归属其 owner 条目的 `bytes`，账本里按引用次数分列（§3.7、V5）；**禁止**在条目间重复累加；
4. 复合条目（如 `E1` 与派生目录的关系）必须在 `includesDerived` 里表达，派生字节是否计入由该条目的 owner 声明，**DTO 与 UI 文案同源**。

**时间口径**：

1. 只提供 `oldestAt` / `newestAt` 两端，取 `modified()`；**不新增** `createdAt`（N2 的 `AttachmentEntry.created_at` 实际就是 mtime，不得据其名推断创建时刻）；
2. 遍历不到任何文件时两端为 `null`，不是 `0`；
3. 保留期判据一律按「本地日期」比较，不用 UTC 秒差（N11 的审计游标日期同理），避免时区导致多删一天。

**包含范围**（什么算进 `bytes`）：

| 类别 | 是否计入 | 表达方式 |
|---|---|---|
| 目标文件本体 | 计入 | — |
| 伴生文件（`-journal` / `-wal` / `-shm`、`.tmp`、`.lock`、部分写入残留） | **不计入** | 由 owner 在 `note` 说明；回收时由 owner 一并处理，管理器不单独列条目 |
| 目录自身 / 空目录 | 不计入 | — |
| 旧位置残留（迁移后遗留的旧目录） | 不计入任何条目 | 归 owner 的迁移清理，**不**进 13 行 |
| 符号链接 / 非常规文件 | 不计入 | 观测记 `unreadable`，回收跳过 |

**自指产物**：观测与回收本身产生的日志（`E12`）计入该条目字节，但必须 `countsSelfLogs = true`；**禁止**把它算进「本次回收收益」，否则数字随观测频率自增。

---

## 4 条目 × 动作矩阵

| 条目 | 动作 | 资格判据 | 策略 | 执行者 | 现状可执行 | 缺口 |
|---|---|---|---|---|---|---|
| `E1` | `A1` / `A2` | `A1`：磁盘 ID − `attachment_refs` 非空；`A2`：用户显式选中 | `none`（手动） | `AttachmentStore` + DB 引用 | `A2` 是；`A1` 仅预览 | `A1` 执行入口；按 ID 锁；`E2` 观测 |
| `E2` | `A5` | 基附件不存在 | `none` | `AttachmentStore` | 否 | `list` / `stat` + 回收方法 |
| `E3` | `A3` | mtime 早于「今天 − 保留天数」；只常规文件 | `config` 30 天，启动注入 | `cleanup_expired_results` | 仅启动自动 | 手动入口；stats；策略自述；fail-closed |
| `E4` | `A5` | 索引缓存文件（含旧 revision 残留） | `owner`（LRU 64 MiB / 单文件 64 MiB） | `SuperIndex` 缓存 | 仅内存 LRU | stats；启动清扫；公开回收方法 |
| `E5` | `none` | 不适用 | `none` | 卸载 / 覆盖命令 | 卸载是 | stats；`.tmp` 清扫 |
| `E6` | `none` | 禁止 | `none` | 无 | 文件长度是 | 无（`VACUUM` 不在本期） |
| `E7` | `none` | 禁止 / 锁定 | `none` | 无 | 否 | stats；锁定态 |
| `E8` | `A4` | 备份数 > 5 | `constant` 5 | `prune_backups` | 备份时自动 | stats；对外 `pruneToLimit` |
| `E9` | `none` | 禁止 | `none` | 无 | 否 | 无（git 整理属外部） |
| `E10` | `A3` | 目录存在且有残留 | `constant`（存在即清） | `cleanup_recovery_dir` | 启动 / 恢复后自动 | 不提供手动入口（避免删进行中的恢复） |
| `E11` | `A3` | JSONL：文件日期 < 游标日期且 < 保留截止；DB 行：`at` < 保留截止 | `config` 90 天，启动注入 | `prune_before` + `cleanup_expired_audit` | 启动 / 补录自动 | stats；只能由 owner 带游标调度 |
| `E12` | `A6` | 插件轮转 | `constant` 3 份 × 10 MiB | `tauri-plugin-log` | 自动 | 仅展示；插件不暴露 stats |
| `E13` | `A4` | 数量 > N（N 待定） | `none`（待定） | `assistant-crash` | 否 | `list` / `stat` / `prune`（本文 `T2`） |

**矩阵读法**：

1. 「现状可执行」= 代码今天已经有的能力；「缺口」= owner 欠账，**不是**管理器可以直接补的；
2. 任何条目的 `Q1 eligible` 都必须等「缺口」补完；在此之前按实际情况降级为 `Q2` / `Q3` / `Q4` / `Q5`（缺回收入口的是 `Q5 notImplemented` + `reason = noReclaimApi`，缺 stats 但可回收的是 `Q2` + `reason = noStats`）；**禁止**为凑「可回收」而提前标 `Q1`；
3. `E1` 的 `A1` 虽然判据已在 N4 实现，但**没有执行入口**，本期新增的只是「把已算出的孤儿交给 owner 删」，不是重新实现孤儿判定；
4. `E11` 的 `A3` 不给手动入口，因为 `prune_before` 需要补录游标（N11）；把它包装成「立即清理」会绕过游标，直接删掉尚未补录的审计。

---

## 5 回收执行语义

### 5.1 串行化与幂等

1. 管理器同一时刻只允许一个 `reclaim_storage` 执行；第二个请求返回 `busy`，不排队、不合并；
2. 管理器不持有跨 owner 的全局锁；owner 自己保证「同一资源」的并发安全（附件按 ID 锁、索引按 key 单飞、备份按目录串行）；
3. 所有动作必须幂等：同一 `planId` 重复执行，已删项计入 `skipped`，不报错；**禁止**用「文件已不存在」当失败；
4. 单实例已启用（I54 §1.1 的 N11），计划状态可以放进程内 `Mutex`，不落盘。

### 5.2 计划与执行

```
snapshot（只读）
  → plan（只读，10 分钟有效，返回 planId + sampledAt + 每条的预期数量/字节）
  → execute（带 planId + confirmed）
  → result（每条 removed / failed / skipped）
  → snapshot（重新采样，账本复算）
```

**硬约束**：

1. `plan` 必须无副作用；验收时用「调用前后 mtime / 文件数不变」断言（V6）；
2. `plan` 自己重新采样，不直接复用上一次 `snapshot` 的数字；两者的 `sampledAt` 可以不同，UI 必须显示计划自己的时刻；
3. `execute` 必须校验 `planId` 存在且未过期（10 分钟）；过期返回 `planExpired`，不执行任何删除；
4. `execute` 必须重新校验资格：附件引用集合重算、`tool-results` 重新 stat、备份重新计数；**禁止**直接按计划里的数量删；
5. 计划期间被修改的项（mtime 晚于 `sampledAt`、引用新增、备份新增）一律跳过并计入 `skipped`，不计入失败。

### 5.3 结果语义

字段与类型**只在 §6.2 定义**（`StorageReclaimEntryResultDto`），本节只定义各 `status` 的含义，不重复列字段：

| `status` | 含义 |
|---|---|
| `done` | 全部候选处理成功 |
| `partial` | 部分成功、部分失败；**允许**出现在清扫类动作（`A3` / `A4`），因为单文件失败不应回滚整批 |
| `failed` | 一个候选都没处理成功，或前置校验失败（如引用重算失败） |
| `skipped` | 候选在执行前已不满足判据（已被删除、已被引用、mtime 变新） |
| `notEligible` | 执行时资格复核不通过，一个都没动 |

**语义硬约束**：

1. `removedCount` / `removedBytes` 只记实际删除，**禁止**填「预计」；
2. `failedCount` 与 `skippedCount` 分开计数，不合并；
3. `errors` 最多 5 条，`message` 不含绝对路径与文件名；
4. **禁止**把「文件不存在」计为失败；**禁止**把「预计删除」计为 `removedBytes`。

### 5.4 失败语义

| 场景 | 处理 |
|---|---|
| 附件引用删除成功、文件删除失败 | 该条 `partial`；引用已删是事实，必须如实上报；不得声称「完全成功」 |
| 附件 ID 非法（`delete_attachments`） | 整批失败（既有语义，不变） |
| `tool-results` 单文件删除失败 | `failed` 计数 + 继续；不回滚 |
| 备份裁剪失败 | 保留多余备份；`failed` 计数；不得删最新一份 |
| 审计裁剪失败 | 保留文件；`failed` 计数；不得回退到更激进的判据 |
| 计划过期 / `planId` 不存在 | `planExpired` / `planNotFound`；零副作用 |
| 另一个执行进行中 | `busy`；零副作用 |
| owner stats 返回错误 | 快照该条 `unreadable`；不得用 `0` 或上一次缓存冒充 |

**没有回滚**：文件删除不可逆，本文不承诺事务性回滚；只能保证「不误删」与「如实报告」。

### 5.5 与写入的竞态

| 条目 | 竞态 | 要求 |
|---|---|---|
| `E1` | 内容寻址去重：删孤儿与并发导入同一内容 | owner 必须让「引用写入」与「文件删除」共享同一 attachment ID 锁；管理器不得在无锁状态下先删文件再删引用 |
| `E1` | 用户同时删除选中附件 | `delete_attachments` 与 `reclaim_storage` 的 `A1` 不得并发；管理器串行化自己的请求，owner 串行化同一 ID |
| `E3` | 工具正在写新结果 | 按 mtime 年龄判据，新文件天然不满足；mtime 在未来（时钟偏移）时按「未过期」处理 |
| `E8` | 新备份刚创建 | 先创建、后裁剪；裁剪不得删最新一份；`keepCount` 至少 1 |
| `E11` | 补录正在读旧文件 | 只删「早于游标日期」且「早于保留截止」的文件；当日文件永不删 |
| `E9` | 外部 git 进程改写 vault | 管理器不回收 `E9`；观测必须标 `sampledAt`，并接受「外部改写导致数字不稳定」 |
| `E7` / `E6` | DB 正在写 | 管理器不回收；stats 只读，失败按 `unreadable` |

### 5.6 留痕

1. 每次 `execute` 写一条结构化应用日志：`planId`、条目 key、动作、`removedCount`、`removedBytes`、`failedCount`、`skippedCount`、`sampledAt`；
2. **禁止**写绝对路径、文件名、用户内容；只写条目身份与计数；
3. 安全域删除（备份裁剪）由 owner 决定是否写安全审计；管理器不直接写审计库，避免「删除审计的同时写审计」的自指；
4. 审计裁剪本身不再写审计（会被同一次裁剪删掉）；只写应用日志；
5. 日志落在 `logs/`，而 `E12` 只展示不回收，因此不存在「管理器删自己的日志」的自指。

---

## 6 契约

### 6.1 命令

| 命令 | 性质 | 请求 | 返回 | 备注 |
|---|---|---|---|---|
| `get_storage_breakdown` | 只读快照 | 无 | `StorageBreakdownDto`（扩展） | 保留旧字段；13 条在 `entries` |
| `get_storage_reclaim_plan` | 只读计划 | `StorageReclaimPlanRequestDto` | `StorageReclaimPlanDto` | 无副作用；10 分钟有效 |
| `reclaim_storage` | 有副作用 | `StorageReclaimRequestDto` | `StorageReclaimResultDto` | 必须 `planId` + `confirmed = true` |
| `delete_attachments` | 有副作用（既有） | `Vec<String>` | `AttachmentDeleteResultDto` | 只服务 `A2`；不经过 `reclaim_storage` |

**命令边界**：

1. **不新增** `get_storage_capabilities`：存储侧没有「整 tab 不可用」的概念，可用性挂在每个条目上（`availability`）；这与 I54 的进程 / 自身占用侧不同；
2. `get_storage_reclaim_plan` 的 `entryKeys` 为空表示「全部条目」，但只用于预览；`reclaim_storage` 的 `entries` **不得为空**，避免「空请求 = 全删」；
3. `reclaim_storage` 的每个 `entry + action` 必须出现在对应计划里；计划里没有的组合直接拒绝，不做「顺手加一个」；
4. `delete_attachments` 保持既有全量校验语义（任一 ID 非法则整体失败），**不得**改造成「部分成功」；
5. 所有命令的错误返回沿用现有 Tauri 错误串；`busy` / `planExpired` / `planNotFound` / `confirmationRequired` 作为命令级错误，不塞进条目结果；
6. 计划可以返回 `action = none` / `rotate` 的条目（用于展示），但 `reclaim_storage` 只接受 `A1` / `A3` / `A4` / `A5`；`none` / `rotate` 一律拒绝，避免把「插件自己会轮转」误当成可执行动作。

### 6.2 DTO

**封闭枚举**（全部 `#[serde(rename_all = "camelCase")]`，前端按枚举分支渲染，不做字符串兜底）：

| 枚举 | 取值 |
|---|---|
| `StorageLayer` | `data` / `local` |
| `StorageByteBasis` | `fileLength`（本期唯一值；保留值位以便将来扩展，但**禁止**未经定义直接产出新值） |
| `StorageReclaimAction` | `none` / `deleteOrphans` / `deleteSelected` / `sweepExpired` / `pruneToLimit` / `rebuildCache` / `rotate` |
| `StorageEligibility` | `eligible` / `notEligible` / `locked` / `ownerManaged` / `notImplemented` |
| `StorageReclaimReason` | `orphan` / `expired` / `overLimit` / `derivedCache` / `transient` / `referenced` / `userData` / `businessData` / `replayRisk` / `locked` / `noStats` / `noReclaimApi` / `externalOwner` / `policyDisabled` / `unknown` |
| `StoragePolicySource` | `config` / `constant` / `owner` / `none` |
| `StorageConfirmLevel` | `none` / `confirm` / `strongConfirm` |
| `StorageReclaimStatus` | `done` / `partial` / `failed` / `skipped` / `notEligible` |
| `StorageReclaimErrorCode` | `permissionDenied` / `io` / `ownerUnavailable` / `policyDisabled` / `locked` / `unknown` |

**`StorageBreakdownDto`**（`get_storage_breakdown` 的返回，扩展既有结构；旧字段含义不变）：

| 字段 | 类型 | 约束 |
|---|---|---|
| `totalBytes` / `totalCount` | `u64` / `u32` | **既有字段，含义不变**（附件目录的字节与条数） |
| `orphans` / `conversations` | 既有结构 | **既有字段，含义不变** |
| `sampledAt` | `i64` | 一次快照一个时刻（秒）；遍历非原子，必须随附 |
| `byteBasis` | 封闭枚举 | 本期恒 `fileLength`；口径进 DTO，不只进文档 |
| `sharedBytes` | `u64` | 被 ≥2 个会话引用的附件字节之和（同一附件只计一次），§3.7 |
| `referencedOnceBytes` | `u64` | 仅被 1 个会话引用的附件字节之和，§3.7 |
| `entries` | `Vec<StorageEntryDto>` | 13 条，顺序固定；前端不据此拼路径 |

**`StorageEntryDto`**：字段与约束见 §3.2（`reclaim` 必填，其余为原 I54 §8.2 的字段），此处不重复。

**`ReclaimPolicyDto`**：

| 字段 | 类型 | 约束 |
|---|---|---|
| `source` | `StoragePolicySource` | 必填 |
| `retentionDays` | `u32 \| null` | `0` = 禁用；键恒在 |
| `keepCount` | `u32 \| null` | 数量上限；键恒在 |
| `maxBytes` | `u64 \| null` | 本期仅展示；键恒在 |
| `effectiveSince` | `i64 \| null` | `config` 来源必填（本次启动注入时刻，秒）；键恒在 |
| `mutable` | `bool` | `config` 为 `true` |
| `failClosed` | `bool` | 本期恒 `true` |

**`ReclaimViewDto`**：字段见 §3.2（`action` / `eligibility` / `reason` / `policy` / `reclaimableBytes` / `reclaimableCount` / `confirmation` / `autoActive` / `note`）。本节不再重复列字段，避免与 §3.2 两处定义漂移；枚举名以本节上表为准（`StorageReclaimAction` 等）。

**`StorageReclaimPlanRequestDto`**（输入）：

| 字段 | 类型 | 约束 |
|---|---|---|
| `entryKeys` | `Vec<String>` | 必填；空 = 全部 13 条；未知 key 整体拒绝 |

**`StorageReclaimPlanDto`**（输出）：

| 字段 | 类型 | 约束 |
|---|---|---|
| `planId` | `String` | 不透明；前端只回传不解析 |
| `sampledAt` | `i64` | 计划采样时刻（秒） |
| `entries` | `Vec<StorageReclaimPlanEntryDto>` | 与请求 key 顺序一致 |
| `totalReclaimableBytes` | `u64 \| null` | 任一 `eligible` 条目算不出则 `null` |
| `complete` | `bool` | 所有请求条目均能给出判据时为 `true` |

**`StorageReclaimPlanEntryDto`**：

| 字段 | 类型 | 约束 |
|---|---|---|
| `key` | `String` | 13 条之一 |
| `action` | `StorageReclaimAction` | 与矩阵一致 |
| `eligibility` | `StorageEligibility` | 规划时刻判定 |
| `reason` | `StorageReclaimReason \| null` | 同上 |
| `expectedCount` | `u64 \| null` | 不可回收为 `null`，不得填 0 冒充 |
| `expectedBytes` | `u64 \| null` | 同上 |
| `policy` | `ReclaimPolicyDto` | 规划时刻快照 |
| `confirmation` | `StorageConfirmLevel` | 由 `expectedBytes` 决定档位 |

**`StorageReclaimRequestDto`**（输入，`deny_unknown_fields`）：

| 字段 | 类型 | 约束 |
|---|---|---|
| `planId` | `String` | 必须命中未过期计划 |
| `entries` | `Vec<StorageReclaimEntryRequestDto>` | 非空；每项必须在该计划的条目集合内 |
| `confirmed` | `bool` | 必须 `true` |

`StorageReclaimEntryRequestDto` 仅 `key` + `action` 两个字段；`action` 必须与计划一致，防止「预览 30 天、执行孤儿」式的偷换。

**`StorageReclaimResultDto`**（输出）：

| 字段 | 类型 | 约束 |
|---|---|---|
| `planId` | `String` | 回显 |
| `executedAt` | `i64` | 秒 |
| `entries` | `Vec<StorageReclaimEntryResultDto>` | 与请求顺序一致，不因单条失败中断 |

**`StorageReclaimEntryResultDto`**：

| 字段 | 类型 | 约束 |
|---|---|---|
| `key` | `String` | 13 条之一 |
| `action` | `StorageReclaimAction` | 回显 |
| `status` | `StorageReclaimStatus` | 必填 |
| `removedCount` | `u64` | 实际删除数 |
| `removedBytes` | `u64` | 实际释放字节 |
| `failedCount` | `u64` | 尝试但失败 |
| `skippedCount` | `u64` | 执行时被判据排除（如刚好被引用） |
| `errors` | `Vec<StorageReclaimErrorDto>` | 只含失败原因，不含路径 |

`StorageReclaimErrorDto` 为 `code`（`StorageReclaimErrorCode`）+ `message`（人类可读，不含绝对路径 / 文件名）。

**契约规则**：

1. 输出 `Option` 不加 `skip_serializing_if`，键恒在、值 `null`；输入 `Option` 加 `#[ts(optional = nullable)]`；
2. 输入 DTO 全部 `deny_unknown_fields` + `rename_all = "camelCase"`；输出同理；
3. 新增 / 改动 DTO 后跑 `npm run contract:gen`，提交 `src/types/protocol.contract.ts`；禁止手改生成类型；
4. 13 条 key 作为 `String` 承载（前端渲染顺序固定、后端未知 key 拒绝），不建封闭枚举，避免条目增删时前端硬崩；
5. `bytes` / `count` 用 `u64`，前端按既有 DTO 约定转数字展示；不得在 DTO 里转 `f64`。

### 6.3 前端

1. **不新建 store、不新建 service**：数据由 `get_storage_breakdown` 一次拉全，回收动作在组件内串行调用；I54 §3 三 tab 页里的 `storage` tab 只做展示与确认（页面壳与另两个 tab 归 I54）；
2. 每个条目行渲染：名称、字节、条数、`availability`、`reclaim.eligibility` 徽标、`reclaim.action` 按钮、策略摘要；
3. 不可回收条目按钮禁用并显示 `reason` 文案；`notImplemented` 额外显示 `note`（「缺少 owner 方法」）；
4. 点击回收 → 先 `get_storage_reclaim_plan` → 弹确认（`strongConfirm` 需二次输入条目名，或长按 / 勾选）→ `reclaim_storage` → 结果 toast + 重新拉 `get_storage_breakdown`；
5. **禁止**前端自行计算「可回收字节」或「是否安全」；只显示后端给的值；
6. `A2` 仍走既有附件删除入口，管理器 UI 不提供「删指定附件」；
7. 请求失败 / `planExpired` 时提示重试，不自动重放（避免「用户没看到就删了」）。

---

## 7 失败与降级

### 7.1 观测失败

| 情况 | 表现 | 不允许 |
|---|---|---|
| 目录不存在 | `bytes = 0`、`count = 0`、`availability = missing` | 报错整页失败；自动创建目录 |
| 权限不足 | `bytes / count = null`、`availability = unreadable` | 用 0 冒充空；回退到缓存旧值 |
| 遍历中断（IO 错误） | 同上 + `availabilityReason` 记错误类别 | 把已遍历部分当完整值上报 |
| DB 被占用 / 迁移中 | `E6` / `E7` 的 `availability = locked` | 强行打开；复制 DB 文件统计 |
| 超时 | `bytes / count = null`、`availability = unavailable` | 无限等待阻塞 UI |

**总则**：单条目失败只降级该条目，`get_storage_breakdown` 仍返回 200 语义；前端逐条展示，不整页报错。

### 7.2 回收失败

| 情况 | 表现 | 不允许 |
|---|---|---|
| owner 方法返回错误 | 该条目 `status = failed` 或 `partial`，其余条目继续 | 整批回滚（已删的无法还原） |
| owner 方法不存在 | `eligibility = notImplemented`，`note` 指出缺哪个方法 | 管理器自己 `fs::remove_dir_all` |
| 执行中判据变化（文件被引用） | 计入 `skippedCount`，不算失败 | 强删 |
| 权限不足 | `errors` 记 `permissionDenied` | 提权重试；静默忽略 |
| 执行超时 | 停止后续条目，已完成条目如实上报 | 假装全部成功 |

### 7.3 计划失效

1. 计划缓存只存内存，重启即失效；
2. 过期 / 不存在 / `planId` 对不上 → 命令级错误，前端重新规划；
3. 计划内条目 `action` 与请求不一致 → 拒绝整个请求，不做部分执行；
4. 计划生成后 owner 策略被改（配置热更新）→ 执行前重新校验策略；不一致则 `status = skipped`，不按旧策略删。

### 7.4 崩溃恢复

1. 管理器**不写**回收日志文件，不引入「未完成事务」状态；
2. 半途崩溃只会留下「部分已删」，下次观测自然反映真实占用；孤儿判定基于引用表，天然幂等；
3. 安全备份裁剪的半途崩溃由 owner 的「先创建后裁剪」顺序兜底，管理器不介入。

---

## 8 被否决的方案

| ID | 方案 | 否决理由 |
|---|---|---|
| X1 | 管理器直接遍历 / 删除目录 | 违反所有者边界；绕过 owner 锁与引用表，必然产生「删了仍被引用」或「漏删」 |
| X2 | 用 `du` 式全盘扫描替代 owner stats | 慢、重复实现、跨平台差异；owner 最懂自己目录的结构与排除项 |
| X3 | 把回收合并进 `get_storage_breakdown` | 观测变有副作用，违反只读快照；前端一刷新就删数据不可接受 |
| X4 | 一键「清理全部」不区分条目 | 用户数据 / 业务数据 / 回放风险条目不可批量删除；必须逐条确认 |
| X5 | 在管理器里维护「已删除清单」做审计 | 与安全审计自指；且删除清单本身需要存储与保留策略 |
| X6 | 给每个条目建独立 Tauri 命令 | 命令爆炸、权限面扩大；统一走 plan + execute |
| X7 | 前端本地计算可回收量 | 前后端两套判据必然漂移；判据只能有一个来源 |
| X8 | 自动回收默认开启 | 默认 `retentionDays = 0`，用户显式配置后才生效；避免升级即删数据 |
| X9 | 用 `f64` 传字节 | 精度与契约不一致；沿用 `u64` |
| X10 | 管理器自建定时任务调度回收 | 与 owner 的启动 / 写入路径抢跑，且无 UI 时静默删数据；调度权只能归 owner |

---

## 9 开放问题

| 编号 | 问题 | 当前处置 |
|---|---|---|
| `OQ1` | 跨会话附件「共享字节」是否需要在 UI 单列 | 本期只保证账本闭合（`total = once + shared + orphan`），不单列；`sharedBytes` 已在 breakdown 顶层 |
| `OQ2` | `E5` mascot 素材是否需要回收 | 本期 `none`；素材随版本走，属安装资产，删了会导致缺图 |
| `OQ3` | `E9` vault 是否允许「压缩历史」 | 本期不做；git 外部可改写，管理器无法保证判据，列为 `externalOwner` |
| `OQ4` | `E10` recovery 的过期判据 | 本期 `none`；恢复点属业务数据，删除需产品定义 |
| `OQ5` | 是否支持按条目配置不同保留天数 | 本期只有两个 config 项（`toolResults` / `securityAudit`，见 §3.5）；`attachments` 无时间判据，其余用常量 |
| `OQ6` | `maxBytes` 配额何时生效 | 本期只展示不执行；需先有「配额命中后的行为」定义（拒绝写入还是触发回收） |
| `OQ7` | 回收是否需要进度事件 | 本期同步返回；条目数量有限，暂不引入事件流 |
| `OQ8` | `E13` 崩溃报告是否上报后删除 | 本期只观测；上报流程未定，删除会丢诊断信息 |
| `OQ9` | 附件 `A3` 是否暴露给用户 | 本期不暴露；孤儿回收由 `A1` 覆盖，单独入口易误用 |
| `OQ10` | 计划有效期 10 分钟是否合适 | 先按 10 分钟；若用户操作慢导致频繁过期，再调至 30 分钟 |

---

## 10 验收判据

| ID | 判据 | 验证方式 |
|---|---|---|
| V1 | `get_storage_breakdown` 返回 13 条，`key` 与 §3.1 完全一致，顺序稳定 | golden 投影对比 `entries[*].key` |
| V2 | 取不到数的条目 `bytes` / `count` 为 `null`，且 `availability ∈ {locked, unreadable, unavailable, notImplemented}`；`missing` 例外（计 `0`）。不存在用 `0` 冒充未知 | 单测：伪造 `read_dir` / `metadata` 失败 |
| V3 | 每个条目 `reclaim` 必填；`action = none` 的条目 `eligibility ∈ {notEligible, ownerManaged, notImplemented}` | 快照断言 |
| V4 | `notEligible` / `notImplemented` / `locked` / `ownerManaged` 的 `reason` 非空；`notImplemented` 的 `note` 点名缺失方法 | 快照断言 + 人工检查 |
| V5 | 账本恒等式成立：`totalBytes = referencedOnceBytes + sharedBytes + orphanBytes`（`orphanBytes` = `orphans[*].bytes` 求和；按条目口径分列时不重复计数） | 单测：构造跨会话共享附件 |
| V6 | `get_storage_reclaim_plan` 无副作用：调用前后目录字节与 mtime 不变 | 临时目录单测 |
| V7 | 计划 `expectedBytes` 与执行 `removedBytes` 的偏差可解释；不可回收条目 `expectedCount/Bytes = null` | 单测 + 结果对比 |
| V8 | `reclaim_storage` 在 `confirmed = false` / `planId` 过期 / `entries` 为空 / `action` 与计划不符时全部拒绝 | 四组负例单测 |
| V9 | 单个条目失败不影响其余条目；结果 `status` 如实反映 `done/partial/failed/skipped` | 注入 owner 失败 |
| V10 | 管理器源码中不存在 `fs::remove_*`、`read_dir`、受管目录名字面量、`AppConf::load()` | 静态检查 / grep 门禁 |
| V11 | `E1` 的 `A1` 执行与 `delete_attachments` 并发时不出现「删了仍被引用」；同一 ID 串行 | 并发单测 |
| V12 | `E3` 配置读失败时回退 `0`（禁用），不会按 30 天删；`E11` 无游标时 `eligibility ≠ eligible` | 单测：配置缺失 / 游标缺失 |
| V13 | `E6` / `E7` / `E9` 永远 `action = none` 且 `eligibility ≠ eligible`；`E5` 无自动删除入口 | 快照断言 |
| V14 | DTO 契约生成后 `protocol.contract.ts` 与 Rust 一致；输入 DTO `deny_unknown_fields`，输出 `Option` 键恒在 | `npm run contract:gen` + 类型检查 |
| V15 | 回收留痕只含 `planId` / key / 动作 / 计数 / `sampledAt`，不含路径与文件名；`logs` 条目不被管理器删除 | 日志断言 + 代码审查 |

**整体验收**：V1–V15 全部通过，且 §3.8 的 G1–G12 在代码审查中逐条签字，方可从「草稿」转「定稿」。

---

## 11 实施切片

| 切片 | 范围 | 交付 | 退出条件 |
|---|---|---|---|
| T1 | **观测补全**：13 条 stats 入口、`get_storage_breakdown` 扩展（`sampledAt` / `byteBasis` / 账本字段 / `reclaim` 视图）、`availability` 降级 | 只读，无回收；所有条目 `reclaim.action` 按矩阵填好，未实现的标 `notImplemented` | V1–V5、V10、V13 通过；前端只读表可展示 13 行 |
| T2 | **低风险回收**：`E2` / `E4` / `E13` 的 owner stats + 回收方法；plan / execute 骨架（`planId`、确认、结果 DTO）；`E1` 孤儿计划（只出计划不执行） | `reclaim_storage` 可执行 `A5` / `A4`（仅低风险条目） | V6–V10 通过；单条失败隔离 |
| T3 | **中风险回收**：`E3` 手动 `A3`（含 fail-closed 修正）、`E8` 对外 `pruneToLimit`、`E1` 孤儿 `A1`（owner 按 ID 锁 + 执行时重算引用） | 用户可在 UI 触发三类回收；`E11` 仍只由 owner 调度 | V7、V9、V11、V12 通过；并发用例通过 |
| T4 | **前端与收尾**：`StorageSettings.tsx` 条目行 / 徽标 / 计划确认 / 结果 toast；i18n 两份；golden fixtures；留痕日志；文档定稿 | 完整存储管理器 tab | V14、V15 通过；全量 `contract:gen` + lint + 相关测试通过 |

**切片依赖**：

1. T1 是 T2–T4 的前置：没有 stats 就无法给计划数字，禁止先做执行再补观测；
2. T2 的 plan / execute 骨架必须先于任何具体条目接入，避免每个条目各写一套确认逻辑；
3. T3 的 `E1` 孤儿执行依赖 T2 的骨架与 owner 的按 ID 锁，不得在 T2 之前上线；
4. T4 只做展示与交互，**不得**在 T4 新增后端判据；发现缺判据应回到 T1/T2/T3。

---

## 12 与 I54 的迁移清单

| I54 内容 | 迁移后 |
|---|---|
| 原 I54 §4 存储 tab 的 12 行（该节现为指针） | 本文 §3.1 的 13 行（新增 `E4 tool-index`） |
| 原 I54 §4.3 的字节 / 时间 / 包含范围三个口径 | 本文 §3.9（唯一出处） |
| 原 I54 §8.1 的 `get_storage_breakdown`（该节现只登记进程侧命令） | 本文 §6.1 / §6.2 扩展，旧字段语义不变 |
| 原 I54 §8.2 的 `StorageEntryDto`（该节现只定义能力声明与进程快照 DTO） | 本文 §3.2 定义条目字段（含 `reclaim`），不删旧字段 |
| §10 第 1 项（D5 资格 / D6 策略，已迁出） | 本文 §3.3–§3.6 承接：动作、资格、策略、确认四套词表 |
| §10 第 2 项（`tool-results` 允许回收，已迁出） | 本文 `E3` 的 `A3`，补 stats 与手动入口；fail-closed 修正见 §3.5 |
| §10 第 3 项（审计保留策略，已迁出） | 本文 `E11`：只由 owner 带补录游标调度，不给手动入口 |
| §10 第 4 项（`derived/` 是否计入附件，已迁出） | 本文 `E2` 独立成行，`E1` 的 `includesDerived = false` |
| §10 第 5 项（`created_at` 语义，已迁出） | 本文 §3.9 时间口径：只给 `oldestAt` / `newestAt`（取 `modified()`），不新增 `createdAt`；字段可改名但不删 |
| 原 §4 末条「回收不在本期」（已迁出） | 由本文 §3.3–§3.6 取代 |
| I54 的进程 / 自身占用条目 | **不动**，仍归 I54；本文不引用其字段、不合并其 DTO |

**迁移硬约束**：

1. I54 删除存储 tab 时，**不得**同时删除 `StorageEntryDto` 等共享 DTO 的旧字段；只把「由谁维护」改为本文；
2. 两文档不得各自定义一套 `StorageEntryDto` / `get_storage_breakdown`；本文是唯一维护方；
3. I54 的「存储 tab 不可用」概念不得迁移到本文：存储侧按条目降级，不按 tab 降级；
4. 前端页面归属**已定**为并入 I54 §3 的三 tab 页（R0 建容器并留三个挂载位）；本文只提供存储 tab 的数据与交互契约（§6.3），不重复页面决策。
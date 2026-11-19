# I61 — 存储管理器规格（资源监视条目 + 回收）

| 项 | 值 |
|---|---|
| 状态 | 定稿（设计已定案，待按 §11 切片实施；观测承接原 I54 存储 tab（已迁入本文），新增回收与策略治理） |
| 日期 | 2026-11-19 |
| 关联 | [I54 资源监视器规格](./I54-resource-monitor-spec.md)（存储 tab 已由本文承接并在 I54 中收敛为指针；进程 / 自身占用仍归 I54）、[STORAGE-MONITOR-ANALYSIS](../analysis/STORAGE-MONITOR-ANALYSIS.md)（存储观测机制的唯一事实基线）、[CODE-SPLIT-PRINCIPLE](../principle/CODE-SPLIT-PRINCIPLE.md)（拆分原则唯一来源，本文只引用不复述）、I44 应用目录管理规格（已归档）（目录归属与字面量门禁）、I58 超级工具规格（已归档）（`tool-index/` 的 owner 与缓存边界） |
| 影响范围 | 后端：`src-tauri/src/cmd/storage/`（`mod.rs` 命令与快照、`probe.rs` 可选能力 trait、`inventory.rs` 对账扫描、`reclaim.rs` 计划与执行；由现有 `cmd/storage.rs` 拆分而来）、`src-tauri/src/app_dirs/registry.rs`（受管根登记表）、`src-tauri/src/protocol/dto/storage.rs`（DTO 扩展）、`src-tauri/src/protocol/events.rs`（`storage:inventory-updated` 广播）、`src-tauri/src/ipc.rs`（命令注册）、各 owner 的 stats / 回收方法（`assistant-storage`、`assistant-tools`、`assistant-vault`、`assistant-crash`）；前端：`src/settings/pages/StorageSettings.tsx`、`src/services/desktop/storageService.ts`、i18n 两份 locale、`src/settings/__tests__/fixtures/*.json`（golden 投影） |

> 一句话：存储管理器把「存储占用」从监视器的一行提升为**可观测、可计划、可回收**的资源监视条目集合——**对外是唯一的发起与展示通道，对内不是唯一执行者**（观测来自 owner stats、删除来自 owner 方法，owner 自己的自动清理照旧）；任何删除都必须先有 owner 的方法，任何数字都必须带可用性与采样时刻，任何「不知道」都不许写成 `0`。

本文中的「必须 / 禁止」为可验收约束；「应当」为默认做法，偏离需在代码注释里写明理由。

**编号约定**：`E1–E13` 是资源监视条目（Entry），`A1–A7` 是回收动作，`Q1–Q5` 是资格状态，`N1–N19` 是现状事实，`G1–G14` 是门禁，`V1–V20` 是验收判据，`T1–T4` 是实施切片，`X1–X13` 是拒绝方案，`OQ1–OQ11` 是开放问题。§3.11 的「对账余量」**不是条目**（无 `E` 编号、不进 `entries`、不进任何合计、不可回收），它只回答「基根上有没有未被 13 条登记认领的东西」。

---

## 1 定位与边界

### 1.1 存储管理器是什么

存储管理器是**存储类资源监视条目在 UI / IPC 层的唯一观测聚合与回收发起入口**（下称「唯一对外入口」）。它**不是唯一执行者**，owner 在自己路径上的自动清理不受本管理器约束：

1. 它承接原 [I54](./I54-resource-monitor-spec.md) 存储 tab 的全部内容（12 行观测表、D1–D4 四个方向、三个口径、账本闭合），I54 §4 现已收敛为指向本文的指针；本文把 12 行扩展为 13 行；
2. 它新增原 I54 明确推迟的**回收动作**与**策略治理**（I54 §1.3 现已改为指向本文）；
3. I54 收敛为「资源监视器」，只保留**进程**与**自身占用**两类资源监视条目；存储类条目不再由 I54 维护；
4. 它不是新 crate：落在应用层 `src-tauri/src/cmd/storage/`（`mod.rs` 承载命令、`probe.rs` 承载可选能力 trait、`inventory.rs` 承载 §3.11 的对账扫描、`reclaim.rs` 承载计划与执行），因为它需要 `AppDirs` 与各 owner 的 stats / 回收方法（与 [STORAGE-MONITOR-ANALYSIS](../analysis/STORAGE-MONITOR-ANALYSIS.md) §10 第 1 条一致）；
5. 它**不拥有任何目录**：目录名与基根只来自 `AppDirs` 与可选能力模块的 getter，本文不新增目录名、不新增目录字面量；
6. 它的观测由**两层**组成，两者互不替代（§3.11）：**声明层**（13 条条目，数字来自 owner 实测，回答「这块多大、属于谁、能否回收」）与**对账层**（对基根做有界枚举后与声明层相减，回答「还有多少没被认领」）。对账层不新增条目、不进任何合计、不可回收，因此「唯一对外入口」不变：存储数字仍然只来自 `get_storage_breakdown`。

**「唯一」的准确范围（本节最关键的一句）**：

| 层 | 唯一？ | 含义 |
|---|---|---|
| **对外**（IPC / 设置页 / 用户可见） | **是** | 不允许页面或其它命令另立一套存储观测与回收；用户可见的存储数字与回收动作只来自 `get_storage_breakdown` / `get_storage_reclaim_plan` / `reclaim_storage`（`delete_attachments` 是既有例外，§1.4）。这一层必须唯一，否则同一份判据会出现第二份实现并漂移 |
| **对内**（实际观测与删除的执行者） | **否** | 观测由各 owner 的 stats 提供、删除由各 owner 的方法执行（§1.2）；owner 在**启动路径 / 写入路径**上的自动清理（备份裁剪 N10、审计裁剪 N11、recovery 清理 N12、日志轮转 N13、索引 LRU 逐出 N7、`.tmp` 清扫 N8）**继续存在**，本文既不接管、也不禁止、更不改其触发时机 |

**因此本文不主张下面两种读法**：

1. 「owner 不许自己清理」——错。这与本表「对内非唯一」冲突，也与 §3.6「仅 owner 既有自动路径」、§4 矩阵的「启动 / 写入路径自动」、X10「调度权只能归 owner」冲突；
2. 「管理器负责执行删除」——错。管理器只负责**发起与校验**（计划、白名单、确认、结果汇总），执行者始终是 owner 的方法（§1.2、G3）；
3. 正确读法：**管理器是唯一「发起 / 展示」通道，owner 是唯一「执行」者**。这两件事不是同一件事，本文所有条款都按这个二分展开。

**「资源监视条目」的定义**：一个条目是一个**受管资源身份**，不是一个路径。条目的 `key` 是稳定标识（如 `attachments`、`security/audit`），前端不得据此拼路径；条目的 `bytes` / `count` 是**观测字段**（占用），不是条目本身。把条目理解成「目录」会立刻导致两个错误：自行遍历、自行删除。

### 1.2 三个身份在存储侧的落点（术语定义见 I54 §2.1）

术语（所有者 / 观测者 / 回收者）由 [I54 §2.1](./I54-resource-monitor-spec.md) 定义，本文不重述；本节只写它们在本管理器里的**落点**，可验收的硬约束见 §3.8 的 `G1`–`G3`：

| 身份 | 在本管理器里的落点 |
|---|---|
| **所有者** | `AppDirs`、可选能力模块（如 `cmd/vault`）、各库的库内子布局 |
| **观测者** | 各 owner 的 stats / list 方法 |
| **回收者** | 各 owner 的回收方法（删除、裁剪、轮转、清扫） |

这条分工对应 I44（已归档）的两条门禁：目录字面量只允许命中 `app_dirs::dir_name` 与可选能力模块；`dirs::` 只允许出现在 `app_dirs` 与 `assistant-config`。存储管理器一旦自己拼路径 + 遍历 + 删除，两条门禁同时失效，且三个身份退化成一份实现。

**推论（对应 §1.1 的「对内非唯一」）**：owner 的自动清理（启动路径 / 写入路径）属于**回收者自己的调度**，管理器不参与、不接管、也不禁止；管理器的回收只是「代表用户请求」去调同一个回收者方法。因此「回收者」这一身份在本文里**始终指 owner**，管理器从不成为回收者。

**对账余量在这套身份里的位置（§3.11）**：余量**不属于任何身份**——它不是观测者（它不产生条目数字，条目数字仍来自 owner stats），不是回收者（它不删任何东西），也不是所有者（它不认识内容，只认名字与长度）。它是**唯一的只读旁路**：可以遍历基根（含受管目录内部）取「盘上真实字节」作为被减数，但①只读、②有界、③产出绝不回流 `entries`（G10 / V20）。正因为要给它开 G2 例外，才必须把这三条写成门禁——例外有边界，身份才不混淆。

### 1.3 纳入与排除

| 纳入 | 排除 |
|---|---|
| 13 个存储类资源监视条目的观测（`E1–E13`） | 进程与自身占用（归 I54） |
| 每个条目的回收资格、策略、计划、执行、结果 | 任务级资源归因、进程登记表、停止 / 回收进程树 |
| 附件孤儿判定与删除、过期清扫、数量裁剪、可重建缓存清理 | 数据库 `VACUUM`、日志轮转实现、vault git 历史整理 |
| 回收留痕（应用日志）与账本闭合复算 | 跨 tab / 跨条目的「总占用」展示 |
| 设置页存储 tab 的观测表与回收入口 | 新增目录的**回收**、新增存储介质、云同步、配额计费 |
| **已登记受管根的对账余量**（§3.11：对基根有界枚举 + 与声明层相减，只读、不进 `entries`、不进任何合计、不可回收） | 未登记内容的**回收**与**归属判定**（余量只回答「有、多大」，不回答「是什么、该不该删」） |

**「新增目录」的分界（唯一解释）**：新增受管目录的**观测**不再是排除项——它由 `app_dirs::registry::MANAGED_ROOTS`（§3.11.1）强制登记，未登记即构建红（V19），磁盘上真实存在但未登记的则由余量以 `unregistered` 暴露（V18）。新增受管目录的**回收**仍是排除项：没有 owner 回收方法、不进 §3.3 白名单。

### 1.4 与现有命令和页面的关系

| 既有物 | 本文处理 |
|---|---|
| `get_storage_breakdown` | **扩展**，不重定义旧字段；新增字段（`sampledAt`、`byteBasis`、`sharedBytes`、`referencedOnceBytes`、`entries`、`inventory`）的定义在本文 §6.2，条目上再追加 `reclaim`。旧字段 `totalBytes` / `totalCount` / `orphans` / `conversations` 与 `E1` 条目**必须由同一次附件采样填充**（§3.10），不得各算一遍 |
| `delete_attachments` | **保留**，仍是「用户显式选中附件」的唯一入口（`A2`）；存储管理器的 `reclaim_storage` **不**接管它，避免两条删除路径 |
| `delete_derived_text` | **保留**，仅随附件删除调用；管理器不直接调用 |
| `open_attachment` / `import_attachment` / `get_asset_dirs` / `get_attachment_limits` | **不动**：签名、语义、错误串均不改。前两条服务聊天侧（`src/chat/attachment/`、`src/orb/panelActions.ts`），后两条服务导入流程；本文不新增同义命令 |
| `src/settings/pages/StorageSettings.tsx` | **并入** I54 §3 的三 tab 资源监视器页：页面壳由 I54 定（R0 先建容器并留三个 tab 挂载位），本文只提供**存储 tab 的内容与交互契约**（§6.3），不重复页面决策。并入时**必须保留**现有附件明细区（会话分组 / 孤儿分组 / 折叠 / 组全选 / 勾选删除），它承载 `A2`；13 行条目表与明细区是**同页两段**，不是替代关系 |
| `src/services/desktop/storageService.ts` | **保留并扩展**：既有 6 个方法签名不变，新增 2 个（计划 / 执行）。本文 §6.3 的「不新建 service」指不另建文件，不是不扩展现有文件 |
| `get_storage_reclaim_plan` / `reclaim_storage` | **新增**（§6.1），只服务策略派生动作（`A1` / `A3` / `A4` / `A5`），且**只对 §4 白名单条目开放** |

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
| N15 | `cmd/storage.rs` 在 `ipc.rs` 无条件注册；`cmd/vault` 受 `feature = "vault"` 门控。装配契约要求 `#[cfg]` 只能出现在 `cmd/mod.rs` 与 `*_plugin.rs`，因此安全域 5 条（`E7`–`E11`）的 stats **不能**由 `cmd/storage.rs` 直接调用 vault crate，必须经「可选能力 provider」注入（§3.10） | `src-tauri/src/ipc.rs`、`src-tauri/src/cmd/mod.rs`、`src-tauri/tests/assembly_contract.rs` |
| N16 | `get_storage_breakdown` 现有会话标题 / 更新时间查询是每会话 2 次 `query_row` 的 N+1；再串行叠加 12 条 stats 会让「刷新」明显变慢 | `src-tauri/src/cmd/storage.rs` |
| N17 | `formatBytes(bytes: number)` 只接受 `number`；新条目表的 `bytes` 是 `number \| null`，直接传入会渲染成 `null B` | `src/utils/formatBytes.ts` |
| N18 | 新增 DTO 必须同时登记在 `protocol_bindings.rs` 的 `use` 与 `decl()` 列表，否则 `protocol_bindings` 测试直接失败；命令注册与 ACL 由 `assembly_contract` 校验（ACL 由构建生成，不手写） | `src-tauri/tests/protocol_bindings.rs`、`src-tauri/tests/assembly_contract.rs`、`src-tauri/gen/schemas/acl-manifests.json` |
| N19 | 受管目录的「知识」是**散的**：`dir_name` 7 个无条件常量、`roots.rs` 7 个 getter、vault 自持的 `SECURITY_DIR` 与其子目录各在自己 crate；**没有任何断言**把「常量 / getter 集合」钉到「13 条 key」，`app_dirs_contract` 只校验 identifier 与 asset 作用域，`roots.rs` 单测只逐个断言路径字符串。因此新增一个受管目录却不加条目时**构建不会红**，表现为 UI 存储数字「凭空少一块」的静默漏账；反之，磁盘上存在但从未登记的目录（历史残留、第三方写入）在管理器视野里根本不存在 | `src-tauri/src/app_dirs/dir_name.rs`、`roots.rs`、`src-tauri/tests/app_dirs_contract.rs`、`src-tauri/crates/assistant-vault/src/security_db/` |

**事实结论**：13 个条目里，真正「观测 + 回收」都已有的只有附件（观测 + 显式删除）与安全备份（自动裁剪）；`tool-results` / 审计有回收但无观测与手动入口；`tool-index` / 派生文本 / `crashes` 有回收欠账；`logs` 由插件管；`vault` / `assistant.db` / `security.db` 禁止回收。

---

## 3 资源监视条目（13 行）+ 对账余量（§3.11，非条目）

### 3.1 条目表

| ID | key | owner | 观测入口（现状） | 回收入口（现状） | 本期动作 | 风险 |
|---|---|---|---|---|---|---|
| E1 | `attachments` | `AttachmentStore`（`assistant-storage`） | `list_all`（第一层文件） | `delete_attachments` / `delete` / `clean_tmp` | `A1` + `A2` | 高（用户数据） |
| E2 | `attachments/derived` | `AttachmentStore` | 无 | `delete_derived_text`（随附件删除） | `A5`（欠账） | 低（可重建） |
| E3 | `tool-results` | `FileToolResultStorage`（`assistant-tools`） | 无 | `cleanup_expired_results`（启动一次） | `A3` | 中（回放定位可能失效） |
| E4 | `tool-index` | `SuperIndex` 缓存（`assistant-tools`） | 无 | 内存 LRU 逐出删文件；单文件 64 MiB | `A5`（欠账） | 低（可重建） |
| E5 | `mascot-assets` | `cmd/mascot_asset` | 无 | 卸载 / 覆盖 | `none`（仅补 stats；`.tmp` 由 owner 在写入路径清扫） | 高（用户素材） |
| E6 | `assistant.db` | `DbState` | 文件长度 | 无 | 无（禁止） | 禁止 |
| E7 | `security/security.db` | `SecurityDbState` | 无 | 备份时裁剪审计行 | 无（禁止 / 锁定） | 禁止 |
| E8 | `security/backups` | `SecurityDbState` | 无 | `prune_backups(5)`（备份时） | `A4`（现状仅备份时自动；T3 补对外 `pruneToLimit`） | 中（派生副本） |
| E9 | `security/vault` | vault repo | 无 | 无 | 无（禁止） | 禁止（用户数据） |
| E10 | `security/recovery` | `cmd/vault` | 无 | `cleanup_recovery_dir`（启动 / 恢复后） | `A3`（仅 owner 自动；**不给手动入口**） | 低（瞬态） |
| E11 | `security/audit` | `SecurityDbState` / `AuditJsonlWriter` | 无 | `prune_before` + `cleanup_expired_audit` | `A3`（仅 owner 调度；**不给手动入口**） | 中（历史决策证据） |
| E12 | `logs` | `tauri-plugin-log` | 无 | 插件轮转 `KeepSome(3)` + 10 MiB | `A6`（仅展示） | 低 |
| E13 | `crashes` | `assistant-crash` | 无 | 无 | `none`（仅补 stats；`A4` 见 OQ8） | 低（诊断产物） |

**本期动作口径（唯一解，逐条按此填 DTO，不允许实现者自行选择）**：

| 条目 | `action` | `eligibility` | `reason` | `policy.source` | 备注 |
|---|---|---|---|---|---|
| `E1` | `deleteOrphans` | 有孤儿 → `eligible`；无孤儿 → `notEligible` | `orphan` / `referenced` | `none` | `A2` 不在本表（走 `delete_attachments`） |
| `E2` | `rebuildCache` | T1：`notImplemented`（`noReclaimApi`）；T2 后 → `eligible` | T1：`noReclaimApi`；T2 后 `derivedCache` | `none` | `note` 点名缺 `AttachmentStore::list_derived` / `purge_derived_orphans` |
| `E3` | `sweepExpired` | `retentionDays > 0` → `eligible`；`0` → `notEligible` | `expired` / `policyDisabled` | `config` | 回放风险只在 UI 文案提示 |
| `E4` | `rebuildCache` | T1：`notImplemented`（`noReclaimApi`）；T2 后 → `eligible` | T1：`noReclaimApi`；T2 后 `derivedCache` | `owner` | 单文件 64 MiB 上限写 `note`，不填 `maxBytes` |
| `E5` | `none` | `notEligible` | `userData` | `none` | 只出 stats |
| `E6` | `none` | `notEligible` | `businessData` | `none` | 只出文件长度 |
| `E7` | `none` | `notEligible`（`businessData`）；仅当 security 文件正在被替换 / 迁移（`metadata` 失败）→ `locked` | `businessData` / `locked` | `none` | **只读 `metadata`，不需要解锁**；vault 锁定**不**影响本条目（见 §7.1） |
| `E8` | `pruneToLimit` | 备份数 > 5 → `eligible`；否则 `notEligible` | `overLimit` | `constant` | `autoActive = true` |
| `E9` | `none` | `notEligible`（`userData`）；vault 锁定态 → `locked`；vaultless 构建 → `notEligible`（`userData`） | `userData` / `locked` | `none` | **永不 `eligible`**；锁定态观测 `locked`、vaultless 观测 `notImplemented`（§7.1 特例 3 / 4） |
| `E10` | `sweepExpired` | `ownerManaged` | `transient` | `constant` | `autoActive = true`，不给手动入口 |
| `E11` | `sweepExpired` | `ownerManaged` | `expired` | `config` | `autoActive = true`，不给手动入口 |
| `E12` | `rotate` | `ownerManaged` | `externalOwner` | `constant` | `availability = notImplemented` + `availabilityReason` 写「`tauri-plugin-log` 不暴露 stats」；`bytes` / `count` 恒 `null` |
| `E13` | `none` | `notImplemented` | `noReclaimApi` | `none` | `note` 点名缺裁剪入口（`assistant_crash` 现只导出 `install`，无 `prune_to_limit`） |

**与 I54 原 12 行的差异**：新增 `E4 tool-index`（I58 之后才出现的受管目录）；I54 说「安全域各子目录无保留策略」已过时（N11 的审计保留已存在）；I54 说「回收不在本期」由本文取代。其余 12 行的身份与字节口径沿用原 I54 §4 的结论（该节已改为指向本文的指针，事实基线仍为 [STORAGE-MONITOR-ANALYSIS](./../analysis/STORAGE-MONITOR-ANALYSIS.md)），不在此重述。

**本表是终态口径（T4 完成时）**：上表的 `eligibility` 描述的是「缺口补完后」的判定，**不是**某个中间切片的状态。中间切片（T1–T3）必须按 §11 的「切片期资格」表填 DTO，且遵守一条硬规则：

> **`eligibility = eligible` 当且仅当该 `key + action` 落在 §3.3 白名单且 owner 方法已存在**；也就是「UI 显示可回收」必须等于「`reclaim_storage` 会接受」。**禁止**出现「显示可回收、执行被拒」的中间态，也禁止用 `eligible` + `note` 表达「还不能执行」。

因此中间切片里，`E1`（缺 `delete_orphans_by_ids`）、`E3`（缺手动入口封装）、`E8`（缺对外 `pruneToLimit`）一律 `notImplemented` + `reason = noReclaimApi`，`note` 点名缺的方法，直到对应切片补齐。

### 3.2 条目字段

条目 DTO **由本文定义**（原在 I54 §8.2，随存储 tab 一并迁入，I54 已不再维护该结构）；旧字段含义不变，新增回收视图：

| 字段 | 类型 | 约束 |
|---|---|---|
| `key` | `String` | 稳定身份，不是路径；前端不据此拼路径 |
| `layer` | 封闭枚举 | `data` / `local`；安全域条目仍是 `data`，靠 `security/` 前缀分组，**不新增第三层** |
| `owner` | `String` | 提供 stats 的 owner 名，用于「由 X 提供」文案 |
| `bytes` / `count` | `u64 \| null` | 取不到数为 `null`，禁止填 `0` 冒充；**唯一例外**是 `availability = missing`（确实没有，计 `0`，见 I54 §2.4）；键恒在。`E1` 的取数失败不降级为 `null`，而是整条命令 `Err`（§7.1） |
| `oldestAt` / `newestAt` | `i64 \| null` | 输出侧 `Option` 不加 `skip_serializing_if`；键恒在 |
| `availability` | 封闭枚举 | I54 §2.4 词表 |
| `availabilityReason` | `String \| null` | `notImplemented` 时必填：正常构建点名缺的方法，vaultless 构建写「可选能力未装配」；`locked` 写「vault 未解锁」；`unavailable` / `unreadable` 填错误类别；`available` 与 `missing` 为 `null`；键恒在 |
| `includesDerived` | `bool` | 语义固定为「本条目 `bytes` 是否**包含**派生 / 伴生文件字节」。13 条全为 `false`：`E1` 不含 `derived/`（派生独立成 `E2`），`E2` 自身即派生，`E3`–`E13` 不含伴生文件（§3.9 包含范围）。将来若某 owner 把伴生文件计入自身，该条改 `true` 并在 `note` 说明 |
| `countsSelfLogs` | `bool` | 自指标注；`E12` 为 `true`，其余为 `false` |
| `reclaim` | `ReclaimViewDto` | **必填**，无回收能力的条目用 `action = none` 表达，不用 `null` |

`ReclaimViewDto`：

| 字段 | 类型 | 约束 |
|---|---|---|
| `action` | 封闭枚举 | `A1–A7`（§3.3） |
| `eligibility` | 封闭枚举 | `Q1–Q5`（§3.4） |
| `reason` | 封闭枚举 \| `null` | `notEligible` / `notImplemented` / `locked` / `ownerManaged` 必填；`eligible` 允许 `null` |
| `policy` | `ReclaimPolicyDto` | 必填；无策略用 `source = none` 表达 |
| `reclaimableBytes` / `reclaimableCount` | `u64 \| null` | 只对 `eligible` 给数；必须 `≤ bytes`；`bytes` 为 `null` 时本字段也 `null` |
| `confirmation` | 封闭枚举 | `none` / `confirm` / `strongConfirm`；**两步判定**（§3.6 硬约束 5）：`eligibility ≠ eligible` 恒 `none`；`eligible` 按条目风险档定档。与 `expectedBytes` 无关，`expectedBytes` 只用于确认文案 |
| `autoActive` | `bool` | owner 已在启动 / 写入路径自动执行（如备份裁剪、审计裁剪） |
| `note` | `String \| null` | 只放「缺哪个 owner 方法」这类结构性说明，不放路径、不放临时错误；长度上限 120 字符（前端单行展示） |

### 3.3 动作词表（封闭）

| ID | 动作 | 语义 | 允许条目 | 幂等 | 确认 |
|---|---|---|---|---|---|
| A1 | `deleteOrphans` | 删除「引用集合之外」的对象 | `E1`（`E2` 后续） | 是（已删的跳过） | `strongConfirm` |
| A2 | `deleteSelected` | 用户显式选中后删除 | `E1` | 是 | `confirm`（既有语义） |
| A3 | `sweepExpired` | 按时间判据删除 | `E3`（可手动）/ `E10` / `E11`（仅 owner 调度） | 是 | `confirm`；owner 自动路径除外 |
| A4 | `pruneToLimit` | 按数量上限保留 | `E8`（本期）/ `E13`（待 OQ8 定案后） | 是 | `confirm`；owner 自动路径除外 |
| A5 | `rebuildCache` | 删除可重建缓存 | `E2` / `E4` | 是 | `confirm` |
| A6 | `rotate` | owner 内部轮转 | `E12` | 是 | `none`（仅展示，不执行） |
| A7 | `none` | 无回收能力 | 其余 | — | `none` |

**动作边界**：

1. `A2` 由既有 `delete_attachments` 执行，**不经过** `reclaim_storage`；`reclaim_storage` 只接 `A1` / `A3` / `A4` / `A5`，且**只对下列白名单组合**开放（其余一律拒绝，不做「顺手加一个」）：
   - `E1` + `A1`、`E2` + `A5`、`E3` + `A3`、`E4` + `A5`、`E8` + `A4`；
   - `E10` + `A3`、`E11` + `A3` **禁止**手动执行（会删进行中的恢复 / 绕过审计补录游标）；
   - `E13` + `A4` 本期**禁止**（OQ8 未定案）；
   - `A6` / `A7` 条目（`E5` / `E6` / `E7` / `E9` / `E12` / `E13`）**禁止**出现在请求里。
2. `A1` 的「引用集合」必须由 owner 在**执行时重算**，不能直接信任计划里的孤儿列表；计划只给数量与字节；
3. `A6` 只用于展示「已由插件轮转」，管理器不得调用插件内部 API；
4. `A5` 的「可重建」必须由 owner 自述；管理器不得自行判断某个文件「看起来像缓存」。

### 3.4 资格状态（封闭）

| ID | 状态 | 含义 | 典型条目 |
|---|---|---|---|
| Q1 | `eligible` | 有 owner 回收方法、判据可算、策略可用、风险不是禁止 | `E1` 的 `A1`、`E3`、`E8` |
| Q2 | `notEligible` | 有方法但当前不满足判据，或判据不可算 | `E1` 的 `A1` 在无孤儿时、`E11` 在游标不可得时 |
| Q3 | `locked` | 存在但受保护读不到 | `E9` 在 vault 锁定态（`availability` 与 `eligibility` 同为 `locked`）；`E7` **不会** `locked`——它只读 `metadata`，失败按 `unavailable`，vault 锁定**不**影响它（§7.1 特例 2） |
| Q4 | `ownerManaged` | 由 owner 自动管理，管理器只展示 | `E10`、`E11`、`E12` |
| Q5 | `notImplemented` | owner 尚无 stats / 回收入口 | `E13`（全程）、`E1` / `E3` / `E8`（T1–T2）、`E2` / `E4`（T1，见 §11 切片期资格表） |

**资格判据**：`eligible` 必须同时满足 ①动作不是 `none`；②owner 方法存在；③判据可算；④策略可用且未禁用；⑤风险不是禁止。缺一即降级，**禁止**用「大概可以删」的推断。

`reason` 封闭词表：

| reason | 含义 | 适用 |
|---|---|---|
| `orphan` | 引用集合之外 | `E1` / `E2` |
| `expired` | 早于保留期 | `E3` / `E11` |
| `overLimit` | 超过数量上限 | `E8`（`E13` 待 OQ8） |
| `derivedCache` | 可重建缓存 | `E2` / `E4` |
| `transient` | 瞬态产物 | `E10` |
| `referenced` | 仍被引用 | `E1` / `E3` |
| `userData` | 用户数据 | `E1` / `E5` / `E9` |
| `businessData` | 业务数据 | `E6` / `E7` |
| `replayRisk` | 删除会让历史回放定位失效 | `E3` |
| `locked` | 受保护 | `E9`（`E7` 不用 `locked`，见 §7.1 特例 2） |
| `noStats` | 缺 stats 入口 | **保留值，本期不产出**：`availabilityReason` 用自由文本点名缺的方法（§3.2），`E12` 的 `reason` 归 `externalOwner`；T1 完成后所有条目都有 stats 或已归其他 reason |
| `noReclaimApi` | 缺回收入口 | `E13`（全程）、`E1` / `E3` / `E8`（T1–T2）、`E2` / `E4`（T1） |
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
| `maxBytes` | `u64 \| null` | **条目级**配额（整个条目允许占用的字节上限）；本期仅展示、不执行。`E4` 的「单文件 64 MiB」是**单文件**上限，不是条目配额，因此 `E4` 的 `maxBytes = null`，该上限写进 `note` |
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
| `tool-results` 配置读失败回退 30 天 | N6：会按 30 天删 | 回退 `0`（禁用），与审计侧一致；保留天数改由**组合根**（`tool_setup.rs`，它本来就是配置读取点）读一次后注入 owner 状态（照 `SecurityDbState::set_audit_retention_days` 的做法，用 `AtomicU32`），`result_stats` 与 `cleanup_expired_results` 都取注入值；管理器只读 owner 自述，不读配置 |
| 审计 JSONL 的 `prune_before` 需要补录游标 | N11：不能用「今天 − 90 天」直接算 | 管理器**不得**提供「立即裁剪审计」；只有 owner 的补录 / 启动调度可调用，且必须传它自己的游标 |

### 3.6 风险分级与确认

| 风险 | 判据 | 条目 | 确认 | 自动 |
|---|---|---|---|---|
| 低 | 删掉只会重建 / 只影响诊断 | `E2`、`E4`、`E13`（`E13` 本期不执行，见 OQ8） | 预览 + 一次确认 | 仅 owner 既有自动路径 |
| 中 | 删掉会让历史定位失效 / 丢失派生副本 | `E3`、`E8`、`E11` | 预览 + 确认 | 仅 owner 既有自动路径（备份裁剪、审计裁剪） |
| 高 | 用户数据 / 用户素材 | `E1` 的 `A1`、`E5` | 预览 + 显式选择 + 强确认（`E1` 的 `A2` 走既有 `delete_attachments` 的确认语义，不升级档位） | **禁止**自动 |
| 禁止 | 业务数据 / 用户数据根 | `E6`、`E7`、`E9`，以及被引用的 `E1` | 不提供入口 | **禁止** |

**硬约束**：

1. **禁止**「一键清理全部」；每个动作都必须先出计划，再按条目确认；
2. **禁止**对高风险条目做任何自动删除；孤儿附件即使「无人引用」，也必须有用户确认；
3. 强确认的文案必须写出**条目、数量、字节**三件套；只写「确定删除吗」不算强确认；
4. 后端只校验 `confirmed = true`，不校验用户输入的文本；强确认的交互由前端落实，但后端不得因此放宽计划校验；
5. `confirmation` 的档位判定分两步，**顺序固定**：
   - 第一步看**用户能否发起**：`eligibility ≠ eligible`（`notEligible` / `locked` / `ownerManaged` / `notImplemented`）→ `none`。用户点不动的东西不需要确认档位，给了档位反而会让前端误以为可执行（`E10` / `E11` / `E12` / `E13` 全部落在这里）；
   - 第二步对 `eligible` 按**风险档**定档：低 / 中 → `confirm`，高 → `strongConfirm`。`expectedBytes` / `expectedCount` **不参与判定**，只用于确认文案；这样「字节很少的孤儿附件」不会因金额小而降低确认强度；
6. 风险档由**条目**决定，不由动作决定：`E1` 无论走 `A1` 还是 `A2` 都是高风险（`A2` 的确认交互沿用既有实现，档位不改 DTO）。

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
| G1 | 管理器不得出现受管目录名字面量 / 路径拼接；受管根的段序列与首段一律来自 `app_dirs::registry::MANAGED_ROOTS`（§3.11.1），管理器只消费 `key` 与 `segments`。观测只调 `AppDirs`、可选能力模块 getter 与 **owner 的 stats / 回收方法**（owner 内部可以拼自己的目录，管理器不行）。`cmd/storage/` 不得 `use assistant_vault` / `assistant_security`，也不得出现 `#[cfg(feature = ...)]`；跨构建差异只能经 §3.10 的 `StorageOwnerProbeHandle` 表达 |
| G2 | 管理器不得 `read_dir` 遍历**受管目录**；观测只调 owner 的 stats / list。**唯一例外**：`cmd/storage/inventory.rs`（§3.11.2）可对**整棵基根树**（`AppDirs::data()` / `AppDirs::local()`，含受管目录内部）做**只读、有界**递归，用于取对账的被减数；该例外必须同时满足三条——只读（不删 / 不移 / 不写 / 不写日志）、有界（§3.11.2 第 5 条）、产出**绝不回流** `entries`（G10 / V20） |
| G3 | 管理器不得 `fs::remove_file` / `remove_dir` / `remove_dir_all`；回收只调 owner 方法 |
| G4 | 每个条目必须有 owner getter 与 stats 入口；缺一个就不许把该条目标 `available`。**登记表是 getter 的唯一来源**（§3.11.1）：`MANAGED_ROOTS` 里没有该 key，就不许产出该条目；反过来，`dir_name` 里每新增一个受管目录常量都必须同时新增一条登记（V19） |
| G5 | 策略读不到 / 判据推不出 / 非普通文件 / 符号链接 → 不删（fail-closed） |
| G6 | 取不到数一律 `null` + `availability` + `reason`；禁止 `0` 冒充（例外：`missing` 计 `0`，因为「确实没有」是已知事实，不是「不知道」） |
| G7 | 观测 / 计划 / 执行三步分离；计划无副作用；执行必须带 `planId` 与 `confirmed = true` |
| G8 | 禁止自动回收用户数据（`E1` / `E5` / `E9`）与业务数据（`E6` / `E7`） |
| G9 | 回收后必须重新采样；账本恒等式必须仍成立 |
| G10 | 禁止把 13 条相加成一个「总占用」（`E12` 恒不可得、条目间可能重叠）。**只允许两处合计**：①附件账本（`total = once + shared + orphan`，全部来自 `E1` 同一数据源，§3.7）；②计划合计 `totalReclaimableBytes`（只累加 `eligible` 条目，且 `complete = true`）。**§3.11 的对账余量另立一栏**：`declared` / `companion` / `ignored` / `optionalNotMounted` / `unregistered` 都**不得**与 13 条相加，也不得与 `totalBytes` 相加成「总占用」 |
| G11 | 禁止全局回收锁；串行化由 owner 负责，管理器只串行化自己的执行请求 |
| G12 | 回收留痕不得写绝对路径；只写条目 key、动作、数量、字节、`planId` |
| G13 | **超集门禁**：既有 wire 字段不得改名、不得删除、不得窄化类型。`AttachmentSummary` / `ConversationAttachmentSummary` / `AttachmentDeleteResultDto` / `AssetDirsDto` / `AttachmentLimitsDto` / `AttachmentImportDto` 与 `StorageBreakdownDto` 的既有 4 字段全部原样保留；新语义只能用**新增**字段表达（`oldestAt` / `newestAt` 等） |
| G14 | 前端必须保留附件明细区（会话分组 / 孤儿分组 / 折叠 / 组全选 / 勾选删除），它承载 `A2`；不得用 13 行条目表替代它。两者是同一页的两段 |

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
| 符号链接 / 非常规文件 | 不计入 | **跳过、不计数，且不使整条降级**；`unreadable` 只用于目标路径本身的 `read_dir` / `metadata` 失败（§7.1）。回收跳过（G5） |
| 未登记目录 / 未登记文件（§3.11 的 `unregistered`） | 不计入任何条目 | 只在 `inventory` 里报「单段名 + 字节 + 条数」，**不**进 13 行、**不**进任何合计、**不**可回收 |

**自指产物**：观测与回收本身产生的日志（`E12`）计入该条目字节，但必须 `countsSelfLogs = true`；**禁止**把它算进「本次回收收益」，否则数字随观测频率自增。

### 3.10 采样装配（owner 入口、可选能力与超时）

**13 条 owner 入口（唯一来源；标「+」的为 T1 需新增的方法）**：

| 条目 | stats 入口 | 路径来源 | 取数失败时的降级 |
|---|---|---|---|
| `E1` | `AttachmentStore::list_all()`（已有） | `AttachmentStore` 自持 `dir` | **不降级**：命令级 `Err`（旧字段非 `Option`，见 §7.1） |
| `E2` | `AttachmentStore::derived_stats()`（+） | 同上（owner 内部拼 `derived/`） | `null` + `unreadable` |
| `E3` | `assistant_tools::runtime::storage::result_stats(root, retention_days)`（+） | `AppDirs::tool_results()` | `null` + `unreadable` |
| `E4` | `assistant_tools::os::tools::super_index::index_stats(root)`（+） | `AppDirs::indexes()` | `null` + `unreadable` |
| `E5` | `cmd::mascot_asset::asset_stats()`（+） | `AppDirs::mascot_assets()` | `null` + `unreadable` |
| `E6` | `DbState::db_file_stats()`（+，只 `metadata`） | `DbState` 自持 path | `null` + `unreadable` |
| `E7` | `SecurityDbState::db_file_stats()`（+，只 `metadata`） | 可选能力 provider | `null` + `unavailable`（`metadata` 失败，如文件正被替换）/ `unreadable`；provider 未注入 → `null` + `notImplemented` |
| `E8` | `SecurityDbState::backup_stats()`（+） | provider | `null` + `unreadable` |
| `E9` | `VaultProbe::repo_stats()`（+） | provider | `null` + `locked`（vault 未解锁）/ `unreadable`；provider 未注入（vaultless 构建）→ `null` + `notImplemented`（**不是** `locked`） |
| `E10` | `VaultProbe::recovery_stats()`（+） | provider | `null` + `unreadable` |
| `E11` | `SecurityDbState::audit_stats()`（+） | provider | `null` + `unreadable` |
| `E12` | 无（插件不暴露） | — | 恒 `null` + `notImplemented`（`availabilityReason` 写「`tauri-plugin-log` 不暴露 stats」） |
| `E13` | `assistant_crash::dump_stats(dir)`（+） | `AppDirs::crashes()` | `null` + `unreadable` |

**三条 owner 接口的既有形状（决定 T1–T3 的改动量）**：

1. `cleanup_expired_results(root, days)` 只返回 `CleanupReport { removed, failed }`，**没有字节**；它按 `mtime` 递归清扫会话子目录、跳过符号链接与非常规文件、`0` 禁用。因此 `E3` 的 `A3` 执行要报 `removedBytes`，必须新增「先按同一判据收集候选（path + bytes），再逐个删除并累加」的 owner 方法，**不能**复用 `CleanupReport` 后自己补算（补算时文件已删，长度取不到）；
2. `result_stats` / `index_stats` 必须与各自的清扫函数**用同一套遍历规则**（递归 / 跳符号链接 / 跳非常规文件），否则「统计 100 MiB、清理 0 字节」这种漂移必然出现；两个 stats 都返回 `{ bytes, count, oldestAt, newestAt }` 供条目与账本共用；
3. `super_index` 在 `assistant-tools` 内是 `pub(crate)`，`index_stats` 需在 `os::tools` 加一层公开转发，管理器只调公开入口（G1）。

**可选能力 provider（解决 N15，本文档唯一的 feature 缝）**：

1. **trait 定义位置**：`src-tauri/src/cmd/storage/probe.rs`（由 `storage/mod.rs` 内 `mod probe;` 引入）——该文件**无条件编译**，因此可以安全承载 trait。trait 声明为 `pub trait StorageOwnerProbe: Send + Sync`，方法覆盖 `E7`–`E11` 的 stats，**参数与返回值只用普通数据**（`protocol::dto::storage` 里的 DTO 或纯 `u64/i64/bool`），**不得**出现 `assistant_vault` / `assistant_security` 的任何类型（否则 vaultless 构建直接编不过）；
2. **方法必须是同步阻塞函数**，不写 `async fn`（`async fn` 在 trait 里不对象安全，需额外 box）；管理器在 `spawn_blocking` 里调用，trait 只需 `Send + Sync` 即可跨线程；
3. **注册类型必须是 `Arc<dyn StorageOwnerProbe>`**：Tauri 的 `manage` / `try_state` 要求 `T: Send + Sync + 'static`（隐含 `Sized`），`dyn StorageOwnerProbe` 本身不满足。因此定义 `pub type StorageOwnerProbeHandle = Arc<dyn StorageOwnerProbe>;`，**注入端与取用端必须用同一个类型别名**（`manage(Arc::new(VaultProbe) as StorageOwnerProbeHandle)` / `try_state::<StorageOwnerProbeHandle>()`），类型不一致会静默取不到（表现为「永远降级」，不报编译错误）；
4. **实现位置**：vault 侧实现在 `src-tauri/src/vault_plugin.rs` 的 `#[cfg(feature = "vault")] mod mounted` 里（该文件是装配契约承认的唯一 vault 特性缝），在插件 `setup` 阶段 `app.manage(...)` 注入。**禁止**把实现放进 `cmd/storage/` 或 `cmd/vault/`——前者会引入 feature 依赖，后者会在 vaultless 构建里消失却留下悬空的注入点；
5. **取用**：`cmd/storage/mod.rs` 只 `app.try_state::<StorageOwnerProbeHandle>()`：取到 → 采 `E7`–`E11`；取不到（vaultless 构建 / 插件未装配）→ 五条 `availability = notImplemented` + `availabilityReason` 点名「可选能力未装配」，`bytes` / `count = null`，`eligibility` 按**禁止回收**语义填（不因装配缺失而变成可执行）：
   - `E7` → `notEligible` + `reason = businessData`；`E9` → `notEligible` + `reason = userData`（两条本来就禁止回收，与是否装配无关）；
   - `E8` → `notImplemented` + `reason = noReclaimApi`（无 owner 方法，V17）；
   - `E10` / `E11` → `ownerManaged`（调度权在 owner；构建里没有该 owner 时自然不发生，不降级为 `notImplemented`）；
6. **禁止** `cmd/storage/` 直接 `use assistant_vault::*` 或出现 `#[cfg(feature = ...)]`（G1 / 装配契约）。trait 本身也**不得**被 `#[cfg]` 门控——它必须在两种构建里都存在，否则 vaultless 构建的降级分支没有类型可写；
7. **声明但未挂载**（`Mount::Feature("vault")` 且 provider 未注入）不是「未登记」：`security/` 段在 vaultless 构建里可能仍在盘上（升级前用过 vault、或用户手工复制），对账必须把它归 `optionalNotMounted` 而不是 `unregistered`（§3.11.3），否则每个 vaultless 用户都会看到一个永远消不掉的「未登记目录」告警。

**并行、超时与同源（解决 N16）**：

1. `E1` 与顶层字段**同源**：一次 `list_all()` + 一次 `attachment_refs` 查询，同时填 `totalBytes` / `totalCount` / `orphans` / `conversations` / `E1` 的 `bytes` / `count`；**禁止**为条目再遍历一次目录；
2. `E2`–`E13` 与 `E1` **并行**采样（`spawn_blocking` / `join`），单条超时 **500 ms** → `availability = unavailable` + `availabilityReason = timeout`，不阻塞整页。**超时只是放弃等待，不是取消**：`spawn_blocking` 的任务无法 abort，超时后该线程会跑完才退出；因此 owner stats 必须①只读、②幂等、③不在持锁状态下做长遍历（否则超时条目会在后台继续占着 owner 锁，影响后续回收）。`E1` 与顶层字段同源，**不参与**这个超时（它失败即整命令 `Err`，见 §7.1 特例 1）；
3. 会话标题 / 更新时间改为一次 `JOIN`（或直接复用 `conversations` 表行），消除 N16 的每会话 2 次 `query_row`；
4. 全部条目共用一个 `sampledAt`（采样开始时刻，秒）；**禁止**每条各取一次时间。

**T1 实现顺序**（按「无依赖 → 有依赖」）：`E1`（已有）→ `E6` / `E13`（纯 `metadata`）→ `E3` / `E4`（owner 目录遍历）→ `E2` / `E5` → `E7`–`E11`（provider）→ `E12`（常量降级）。

---

### 3.11 对账余量（声明 + 扫描，非条目）

**本节回答的问题只有一个**：「基根上有没有东西没被 13 条认领？」它不回答「是什么」「该不该删」，也不产出任何条目、不参与任何合计、不可回收（G10）。

**设计理念（六条，实现时按此自检）**：

1. **双覆盖**：声明层答「这块多大、属于谁、能否回收」（owner 实测，同步）；扫描层答「盘上还有什么没被认领」（基根枚举 + 减法，异步）。两层职责正交，永不互相替代。
2. **观测与扫描物理分离**：观测走同步有界路径（单条 500 ms，§3.10），扫描走后台任务；`get_storage_breakdown` **绝不等待扫描**，因此页面永远不会因扫描而卡住。
   - **为什么观测必须同步**（三条，按强度）：①G9 要求「回收后重新采样、账本恒等式仍成立」，而 `total = once + shared + orphan` 与 `E1` 同源（§3.7、§3.10 第 1 条）只在**同一时刻的同一份数据**下成立；异步快照会把账本闭合从「同步不变量」退化为「最终一致」，等于放弃对 I54 的核心承诺。②§5.5 的「执行时重判 + 跳过晚于 `sampledAt` 的条目」以 `sampledAt` 新鲜为前提；5 分钟 TTL 的快照会让「计划 200 个、执行删 30 个」常态化，重判从主力防线退化为兜底。③观测成本**结构性地有上界**（G2 禁止管理器遍历受管目录，stats 只来自 owner 的已知形状方法），扫描成本**无上界**（整棵基根树、未知子树、第三方写入）——能同步是因为门禁保证了上界，不是因为乐观。
   - 「同步」= 语义上同一次调用内出一致快照，**不是**阻塞运行时：实现是 `spawn_blocking` + 13 条并行 + 单条超时降级（`unavailable + timeout`，不写 `0`、不拖整页）。
3. **只用减法，不做第二套统计**：`余量 = 扫描值 − Σ 该节点覆盖范围内的登记条目字节`。禁止为算余量再写一遍「遍历 + 累加」——那正是 X2 拒绝的漂移源。
4. **减法成立的前提是「登记 ⊆ 扫描」**：每个节点的 `declaredBytes` 必须是 `scanBytes` 的子集。靠三条已有约定维持——13 条 `includesDerived` 全 `false`（§3.2）、伴生文件不计入任何条目（§3.9）、符号链接 / 非常规文件不跟随不计数（§3.9）。**注意后两条方向相反**：符号链接/非常规文件**两边都不计**（不产生余量）；伴生文件**扫描计、登记不计**（产生一个已知来源的小余量，属预期噪声，见 §3.11.2 第 4 条）。改这三条中的任何一条都必须复查本节。
5. **余量是「发现」，不是「动作」**：不进白名单、不进合计、不进 `complete`，用户点不动。
6. **负数即报警**：`Σ登记字节 > 扫描子树字节` → `overRegistered`。这是**唯一**能自动暴露「某条目重复计入 / 扫描漏扫 / owner 口径与 §3.9 不一致」的信号，必须显式上报（`partial = true` + 该节点标记），**禁止**静默取 `0`。

#### 3.11.1 登记表（唯一来源，`app_dirs::registry`）

新增 `src-tauri/src/app_dirs/registry.rs`，把散落在 `dir_name.rs` / `roots.rs` / vault 自持常量里的受管目录知识收敛为一张表（解决 N19）：

```rust
pub enum RootScope { Data, Local }
pub enum RootCoverage { SingleFile, SelfFilesOnly, RecursiveTree }
pub enum RootMount { Always, Feature(&'static str) }

pub struct ManagedRoot {
    pub entry: &'static str,                  // "E1".."E13"
    pub key: &'static str,                    // "attachments" / "attachments/derived" / ...
    pub scope: RootScope,
    pub segments: &'static [&'static str],    // 段序列；首段即对账单位
    pub coverage: RootCoverage,               // 该条目覆盖「哪些文件」
    pub owner: &'static str,
    pub mount: RootMount,                     // E7–E11 为 Feature("vault")
    pub inventory: bool,                      // 该 scope 是否参与余量对账（见 OQ11）
}

pub const MANAGED_ROOTS: &[ManagedRoot] = &[ /* 13 条，顺序 E1→E13 */ ];
pub const IGNORED_NAMES: &[&str] = &[".DS_Store", "Thumbs.db", "desktop.ini"];
```

| entry | `segments` | `coverage` | 说明 |
|---|---|---|---|
| `E1` | `["attachments"]` | `SelfFilesOnly` | 只顶层文件（`list_all` 口径），**不含** `derived/` |
| `E2` | `["attachments", "derived"]` | `RecursiveTree` | 与 `E1` 合起来正好覆盖 `attachments` 段全部常规文件——这是减法精确的原因 |
| `E3` | `["tool-results"]` | `RecursiveTree` | |
| `E4` | `["tool-index"]` | `SelfFilesOnly` | 单层 `*.idx` |
| `E5` | `["mascot-assets"]` | `RecursiveTree` | |
| `E6` | `["assistant.db"]` | `SingleFile` | 伴生 `-journal` / `-wal` / `-shm` 归 `companion` |
| `E7` | `["security", "security.db"]` | `SingleFile` | `Feature("vault")` |
| `E8` | `["security", "backups"]` | `RecursiveTree` | `Feature("vault")` |
| `E9` | `["security", "vault"]` | `RecursiveTree` | `Feature("vault")` |
| `E10` | `["security", "recovery"]` | `RecursiveTree` | `Feature("vault")` |
| `E11` | `["security", "audit"]` | `RecursiveTree` | `Feature("vault")` |
| `E12` | `["logs"]` | `RecursiveTree` | `scope = Local` |
| `E13` | `["crashes"]` | `RecursiveTree` | `scope = Local` |

**登记纪律（强制）**：

1. `dir_name` 的常量降为 `pub(crate)`，**只允许** `roots.rs` 与 `registry.rs` 引用；`roots.rs` 的 getter 改为从登记表派生（路径拼接仍留在 `app_dirs`，管理器不碰）；
2. 新增受管目录 = 加一个 `dir_name` 常量 **且** 加一条 `MANAGED_ROOTS`；只做前者，V19 直接红。这是「指导登记」的机制本体，比文档提醒可靠；
3. `MANAGED_ROOTS` 与 13 条 key、`dir_name` 常量三者双向一致，多一个少一个都失败（V19）；`E2` 是 `attachments` 的子项、不占独立 `dir_name` 常量，`E7`–`E11` 走 `Feature` 挂载，断言里必须分别处理这两类例外。

#### 3.11.2 扫描（`cmd/storage/inventory.rs`，唯一 `read_dir` 例外）

1. **范围**：对**整棵基根树**（`AppDirs::data()` 与 `AppDirs::local()`）做只读递归，**包含**受管目录内部——这是 G2 的显式例外，理由只有一个：余量需要「扫描总量」作为被减数，而只有拿到受管目录的真实子树字节，减法才成立（否则 `unregistered` 恒等于扫描值、减法形同虚设，X13 的「不减去登记值」也就无从区分）。例外的边界由三条硬约束守住：①**只读**（不删、不移、不写、不写日志）；②产出**绝不回流** `entries`（G10 / V20）；③有界（第 5 条）；
2. **首层分类**：首层条目名 → 匹配 `MANAGED_ROOTS` 的**首段**（`SingleFile` 条目的首段即文件名）；命中 `Always` → `declared`；命中 `Feature` 但 provider 未注入 → `optionalNotMounted`；命中 `IGNORED_NAMES` → `ignored`；命中伴生后缀 → `companion`；其余 → `unregistered`（§3.11.3）；
3. **子树字节与条数**：对每个首段递归累加**常规文件**长度并同时计数（同一次遍历产出 `scanBytes` 与 `scanCount`，**不分两次遍历**，否则两份数字会在扫描期间漂移）；口径与 §3.9 完全一致（不跟随符号链接、跳过非常规文件、目录自身不计），并且**必须与 owner stats 用同一套遍历规则**（§3.10 接口形状 2）——口径一旦分叉，减法就会常年出现假余量；
4. **伴生文件**：无论在哪一层，`-journal` / `-wal` / `-shm` / `.tmp` / `.lock` 都计入 `scanBytes` / `scanCount`（它们确实占盘），但不计入任何 `declaredBytes` / `declaredCount`（§3.9），因此会表现为该节点的 `residualBytes > 0`。这是**预期噪声**，不是报警（报警只看 `overRegistered`，见理念 6）；
5. **有界**：深度 ≤ 8、条目 ≤ 50 000、耗时 ≤ 5 s（软上限）；触界时该节点 `scanBytes` / `scanCount` = `null` + `partial = true` + 记入 `truncatedNodes`（宁可「知道有、不知多大」也不做无界扫描）；
6. **vault 锁定 / 无权限**：该子树 `scanBytes` / `scanCount` = `null`，不影响其它首段；不解锁、不提权；
7. **只读**：不删除、不移动、不写任何东西（含应用日志——扫描本身不写日志，避免污染 `E12` 的自指字节）；`symlink_metadata` 判断类型，**不跟随**符号链接；
8. 扫描在**后台任务**里跑，结果落快照（§3.11.4），不进 `get_storage_breakdown` 的关键路径。

**与 X2 的边界（不推翻 X2）**：X2 拒绝的是「用 `du` 式全盘扫描**替代** owner stats」。本节的扫描**确实**会遍历受管目录内部，但它不碰任何 E 条目的数字：条目 `bytes` 仍只来自 owner（V20 逐字段断言），扫描结果只作为**被减数**，余量与条目数字**零交集**、不可回收、有界、只回答「有没有没被认领的」。X2 已同步改写为「拒绝替代、允许有界对账」。

**为什么不用 owner 的 stats 当被减数（避免自我循环）**：若被减数也来自 owner，减法恒等于 0，余量永远为零——那就只能发现「未登记的**首层**目录」，发现不了「已登记目录里 owner 没统计到的文件」。扫描必须独立于 owner 才有信息量，这也是本节允许进入受管目录的唯一正当理由。

#### 3.11.3 分类与减法

首层每个条目必落一类（**互斥且穷尽**）：

| 类别 | 判据 | 是否进余量 |
|---|---|---|
| `declared` | 首段命中某 `Always` 登记根 | 否（归该条目） |
| `optionalNotMounted` | 首段命中 `Feature` 登记根，但该 provider 未注入 | 是（**单列**，不报「未登记」） |
| `companion` | 命中伴生后缀（`-journal` / `-wal` / `-shm` / `.tmp` / `.lock`） | 是（单列；若归入 `unregistered`，SQLite 的 `-wal` / `-shm` 会让余量天天非零，告警很快失效） |
| `ignored` | 命中 `IGNORED_NAMES` | 是（单列） |
| `unregistered` | 其余 | 是（**主信号**） |

**减法**（按节点，纯函数、无 IO）：

```
节点扫描字节 = 该首段下所有常规文件长度之和（递归，不跟随符号链接）
节点扫描条数 = 同一次遍历计到的常规文件个数（与字节同源，不分两次遍历）
节点登记字节 = Σ 覆盖范围落在该节点内的 declared 条目 bytes
节点登记条数 = Σ 覆盖范围落在该节点内的 declared 条目 count
节点余量字节 = 节点扫描字节 − 节点登记字节
节点余量条数 = 节点扫描条数 − 节点登记条数
overRegistered = 节点登记字节 > 节点扫描字节（字节维度报警；条数维度只随之下沉为 null，不单独报警）
```

- 被减数唯一（扫描树，一份），减数唯一（登记条目在同一节点上的 `bytes`）；
- 只有 `declared` 节点有减数；`optionalNotMounted` / `companion` / `ignored` / `unregistered` 的减数恒 `0`，因此它们的 `residualBytes` 就等于 `scanBytes`——「未登记」不需要额外算一遍，**它就是扫到的那份**；
- `unregistered` 的明细项逐条给「单段名 + 字节 + 条数」，`name` 强制校验不含 `/`、`\`、`..`（单段名不是路径泄漏；V15 禁的是**回收留痕日志**里的路径与文件名，不是观测 DTO）；
- **守恒**：`declared + companion + ignored + optionalNotMounted + unregistered == 该节点首层观测条目数`（V18）。

#### 3.11.4 异步快照与读路径

```
neverRun ──trigger──▶ running ──done──▶ fresh ──(TTL 过期 / 手动刷新)──▶ stale ──trigger──▶ running
                        └──fail──▶ failed（保留上次 fresh 的数字 + failedAt）
```

| 规则 | 内容 |
|---|---|
| 读路径零等待 | `get_storage_breakdown` 只读当前快照，**绝不** `await` 扫描 |
| single-flight | `running` 期间重复触发直接复用，不排队、不叠加（读一次刷一次不会把扫描堆成队列） |
| stale 触发 | 读到 `stale` 时**返回旧值并触发后台一次**，下次读拿新值 |
| TTL | 5 分钟（扫描比观测贵得多，余量变化频率低） |
| 失败保留 | `failed` 时 `inventory.status = failed` + `failedAt`，**13 条不受影响** |
| 通知 | 复用 `protocol::events::emit_broadcast` 发 `storage:inventory-updated`，前端据此重拉；**不新增轮询** |
| 命令边界 | **不新增**「查余量」命令（§1.2 对外唯一性）；可选 `refresh_storage_inventory`（无参、幂等、返回当前 `status`）仅作为前端「重新扫描」按钮的交互入口，不构成第二套观测 |

#### 3.11.5 四条不变量（实现期间随时自检）

1. 任一时刻 `entries` 恒 13 条、顺序 `E1→E13`，不因扫描状态改变；
2. `inventory` 的任何数字**从不**写入 `entries` 的 `bytes` / `count`；
3. `get_storage_breakdown` 的耗时**与扫描无关**（扫描永远在后台）；
4. `reclaim_storage` 不认识 `inventory` 的任何 key（白名单不变）。

---

## 4 条目 × 动作矩阵

| 条目 | 动作 | 资格判据 | 策略 | 执行者 | 现状可执行 | 缺口 |
|---|---|---|---|---|---|---|
| `E1` | `A1` / `A2` | `A1`：磁盘 ID − `attachment_refs` 非空；`A2`：用户显式选中 | `none`（手动） | `AttachmentStore` + DB 引用 | `A2` 是；`A1` 仅预览 | `A1` 执行入口；按 ID 锁；`E2` 观测 |
| `E2` | `A5` | 基附件不存在 | `none` | `AttachmentStore` | 否 | `list` / `stat` + 回收方法 |
| `E3` | `A3` | mtime 早于「今天 − 保留天数」；只常规文件 | `config` 30 天，启动注入 | `cleanup_expired_results` | 仅启动自动 | 手动入口；stats；策略自述；fail-closed |
| `E4` | `A5` | 索引缓存文件（含旧 revision 残留） | `owner`（LRU 64 MiB / 单文件 64 MiB） | `SuperIndex` 缓存 | 仅内存 LRU | stats；公开回收方法（启动清扫可选，归 owner 调度） |
| `E5` | `none` | 不适用 | `none` | 卸载 / 覆盖命令 | 卸载是 | `stats`（T1）；`.tmp` 由 owner 在 `atomic_write` 路径清扫（**不是**管理器动作） |
| `E6` | `none` | 禁止 | `none` | 无 | 文件长度是 | 无（`VACUUM` 不在本期） |
| `E7` | `none` | 禁止 / 锁定 | `none` | 无 | 否 | `stats`（T1，只 `metadata`）；锁定态 |
| `E8` | `A4` | 备份数 > 5 | `constant` 5 | `prune_backups` | 备份时自动 | `stats`；对外 `pruneToLimit` |
| `E9` | `none` | 禁止 | `none` | 无 | 否 | `stats`（T1；锁定态 `locked`、vaultless 构建 `notImplemented`）；git 整理属外部 |
| `E10` | `A3` | 目录存在且有残留 | `constant`（存在即清） | `cleanup_recovery_dir` | 启动 / 恢复后自动 | `stats`；**不提供**手动入口（避免删进行中的恢复） |
| `E11` | `A3` | JSONL：文件日期 < 游标日期且 < 保留截止；DB 行：`at` < 保留截止 | `config` 90 天，启动注入 | `prune_before` + `cleanup_expired_audit` | 启动 / 补录自动 | `stats`；只能由 owner 带游标调度 |
| `E12` | `A6` | 插件轮转 | `constant` 3 份 × 10 MiB | `tauri-plugin-log` | 自动 | 仅展示；插件不暴露 stats |
| `E13` | `none` | 不适用（`prune` 待 OQ8） | `none`（待 OQ8） | `assistant-crash` | 否 | `stats`（T1）；`prune` 待 OQ8 定案，本期**不**做 |

**矩阵读法**：

1. 「现状可执行」= 代码今天已经有的能力；「缺口」= owner 欠账，**不是**管理器可以直接补的；
2. 任何条目的 `Q1 eligible` 都必须等「缺口」补完；在此之前按实际情况降级为 `Q2` / `Q3` / `Q4` / `Q5`（缺回收入口的是 `Q5 notImplemented` + `reason = noReclaimApi`，判据不可算的是 `Q2` + `reason = unknown`）；**禁止**为凑「可回收」而提前标 `Q1`。各切片的具体期望见 §11「切片期资格」表（T1 结束时 `E1` / `E2` / `E3` / `E4` / `E8` / `E13` 均为 `Q5 notImplemented` + `noReclaimApi`）；
3. `E1` 的 `A1` 虽然判据已在 N4 实现，但**没有执行入口**，本期新增的只是「把已算出的孤儿交给 owner 删」，不是重新实现孤儿判定；
4. `E11` 的 `A3` 不给手动入口，因为 `prune_before` 需要补录游标（N11）；把它包装成「立即清理」会绕过游标，直接删掉尚未补录的审计；
5. **本期可执行集合**（`reclaim_storage` 白名单，与 §3.3 动作边界一致）：`E1+A1`、`E2+A5`、`E3+A3`、`E4+A5`、`E8+A4`。`E10` / `E11` / `E13` 即使矩阵写了动作，也**不得**进入请求；管理器必须按白名单拒绝，而不是依赖前端不发送。

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
| `E7` / `E6` | DB 正在写 | 管理器不回收；stats 只读 `metadata`，不需要解锁；失败按 `unreadable` / `locked`（§7.1 特例 2） |

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
| `get_storage_breakdown` | 只读快照 | 无 | `StorageBreakdownDto`（扩展） | 保留旧字段；13 条在 `entries`；**余量在 `inventory`，只读快照、零等待**（§3.11.4）；采样装配、并行与超时见 §3.10 |
| `refresh_storage_inventory` | 只读快照（可选） | 无 | `StorageInventoryStatusDto` | **不新增查询通道**：只触发后台扫描并返回当前 `status`，数字仍由 `get_storage_breakdown` 的 `inventory` 提供；幂等，`running` 期间重复调用直接返回 |
| `get_storage_reclaim_plan` | 只读计划 | `StorageReclaimPlanRequestDto` | `StorageReclaimPlanDto` | 无副作用；10 分钟有效 |
| `reclaim_storage` | 有副作用 | `StorageReclaimRequestDto` | `StorageReclaimResultDto` | 必须 `planId` + `confirmed = true` |
| `delete_attachments` | 有副作用（既有） | `Vec<String>` | `AttachmentDeleteResultDto` | 只服务 `A2`；不经过 `reclaim_storage` |

**命令边界**：

1. **不新增** `get_storage_capabilities`：存储侧没有「整 tab 不可用」的概念，可用性挂在每个条目上（`availability`）；这与 I54 的进程 / 自身占用侧不同；
2. `get_storage_reclaim_plan` 的 `entryKeys` 为空表示「全部条目」，但只用于预览；`reclaim_storage` 的 `entries` **不得为空**，避免「空请求 = 全删」；
3. `reclaim_storage` 的每个 `entry + action` 必须出现在对应计划里，**且必须落在 §3.3 的白名单**（`E1+A1` / `E2+A5` / `E3+A3` / `E4+A5` / `E8+A4`）；不在白名单或计划里没有的组合直接拒绝整个请求，不做「顺手加一个」、不做部分执行；
4. `delete_attachments` 保持既有全量校验语义（任一 ID 非法则整体失败），**不得**改造成「部分成功」；其返回结构 `AttachmentDeleteResultDto` 不改；
5. 所有命令的错误返回沿用现有 Tauri 错误串；`busy` / `planExpired` / `planNotFound` / `confirmationRequired` / `entryNotAllowed` 作为命令级错误，不塞进条目结果；
6. 计划可以返回 `action = none` / `rotate` 的条目（用于展示），但 `reclaim_storage` 只接受白名单内的 `A1` / `A3` / `A4` / `A5` 组合；`none` / `rotate` 一律拒绝，避免把「插件自己会轮转」误当成可执行动作；
7. 两个新命令必须在 `ipc.rs` 注册（`ipc.rs` 是纯核心注册表，**不得**出现 `#[cfg]`）；ACL 由构建生成，不手写（N18）；
8. `get_storage_breakdown` 的耗时上限：单条目 500 ms、整体并行采样（§3.10）；超时条目降级为 `unavailable`，**不得**让整页等一条慢 stats。该上限**不含**扫描：`inventory` 只是快照读取，扫描在任何情况下都不得进入本命令的 `await` 链（§3.11.5 不变量 3）；
9. **禁止**新增「查余量」命令（如 `get_storage_inventory`）：余量必须挂在 `get_storage_breakdown` 的 `inventory` 字段上，否则会出现「条目数字与余量来自两次不同快照」的账本不一致；`refresh_storage_inventory` 只允许触发扫描并回 `status`，不得回数字（§3.11.4）。

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
| `StorageInventoryStatus` | `neverRun` / `running` / `fresh` / `stale` / `failed`（§3.11.4 状态机） |
| `StorageInventoryClass` | `declared` / `optionalNotMounted` / `companion` / `ignored` / `unregistered`（§3.11.3，互斥且穷尽） |

**`StorageBreakdownDto`**（`get_storage_breakdown` 的返回，扩展既有结构；旧字段含义不变）：

| 字段 | 类型 | 约束 |
|---|---|---|
| `totalBytes` / `totalCount` | `u64` / `u32` | **既有字段，含义不变**（附件目录的字节与条数） |
| `orphans` / `conversations` | 既有结构 | **既有字段，含义不变** |
| `sampledAt` | `i64` | 一次快照一个时刻（秒）；遍历非原子，必须随附 |
| `byteBasis` | 封闭枚举 | 本期恒 `fileLength`；口径进 DTO，不只进文档 |
| `sharedBytes` | `u64` | 被 ≥2 个会话引用的附件字节之和（同一附件只计一次），§3.7 |
| `referencedOnceBytes` | `u64` | 仅被 1 个会话引用的附件字节之和，§3.7 |
| `entries` | `Vec<StorageEntryDto>` | 13 条，顺序固定（`E1`→`E13`）；不可用条目也必须出现，不因 `unavailable` 缺失；前端不据此拼路径 |
| `inventory` | `StorageInventoryDto` | §3.11 的对账余量快照；**恒存在**（未扫过时 `status = neverRun`、数字字段全 `null`，不用外层 `null` 表达两种「没有」）。**任何 `inventory` 数字都不得进入 `entries`、`totalBytes` 或任何合计**（G10） |
| `complete` | `bool` | 除 `E12` 外的 12 条全部 `availability ∈ {available, missing}` 时为 `true`；否则前端不得展示跨条目合计。`E12` **恒不参与合计**（插件不暴露 stats，永久 `notImplemented`），因此它的不可用不阻止 `complete`；`missing` 计 `0` 属可得，不阻止 `complete` |

`StorageBreakdownDto` 的**新增字段一律带 `#[serde(default)]`**：旧前端 / 旧 mock / 旧快照反序列化不炸；既有 4 字段的键名、类型、含义**不得**改动（G13）。

**`StorageEntryDto`**：字段与约束见 §3.2（`reclaim` 必填，其余为原 I54 §8.2 的字段），此处不重复。

**`ReclaimPolicyDto`**：

| 字段 | 类型 | 约束 |
|---|---|---|
| `source` | `StoragePolicySource` | 必填 |
| `retentionDays` | `u32 \| null` | `0` = 禁用；键恒在 |
| `keepCount` | `u32 \| null` | 数量上限；键恒在 |
| `maxBytes` | `u64 \| null` | **条目级**配额；本期仅展示；`E4` 恒 `null`（其 64 MiB 是单文件上限，见 §3.5）；键恒在 |
| `effectiveSince` | `i64 \| null` | `config` 来源必填（本次启动注入时刻，秒）；键恒在 |
| `mutable` | `bool` | `config` 为 `true` |
| `failClosed` | `bool` | 本期恒 `true` |

**`ReclaimViewDto`**：字段见 §3.2（`action` / `eligibility` / `reason` / `policy` / `reclaimableBytes` / `reclaimableCount` / `confirmation` / `autoActive` / `note`）。本节不再重复列字段，避免与 §3.2 两处定义漂移；枚举名以本节上表为准（`StorageReclaimAction` 等）。

**`StorageReclaimPlanRequestDto`**（输入，`deny_unknown_fields`）：

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
| `confirmation` | `StorageConfirmLevel` | 两步判定（§3.6 硬约束 5）：非 `eligible` 恒 `none`，`eligible` 按风险档定档；与 `expectedBytes` 无关 |

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

**`StorageInventoryDto`**（§3.11 对账余量快照；`StorageBreakdownDto.inventory` 与 `refresh_storage_inventory` 共用其 `status` 部分）：

| 字段 | 类型 | 约束 |
|---|---|---|
| `status` | `StorageInventoryStatus` | 必填；`running` / `stale` / `failed` 都必须能出现，`neverRun` 时其余数字字段全 `null` |
| `scannedAt` | `i64 \| null` | 上次**成功**扫描完成时刻（秒）；`neverRun` 为 `null` |
| `failedAt` | `i64 \| null` | 上次失败时刻；`failed` 时非 `null`，同时保留上次 `scannedAt` 的数字 |
| `nodes` | `Vec<StorageInventoryNodeDto>` | 按**首段**分组，顺序与 `MANAGED_ROOTS` 首段出现顺序一致 |
| `unregisteredBytes` | `u64 \| null` | `unregistered` 合计；任一节点算不出则 `null`（不填 0） |
| `unregisteredCount` | `u64 \| null` | 同上 |
| `partial` | `bool` | 有任一节点触界（深度 / 条目数 / 耗时上限）或 `overRegistered` 时为 `true` |
| `truncatedNodes` | `Vec<String>` | 触界的首段名（单段名，非路径）；供前端提示「有界扫描未覆盖完」 |

**`StorageInventoryNodeDto`**（每个首段一条；**只读、不进任何合计**）：

| 字段 | 类型 | 约束 |
|---|---|---|
| `name` | `String` | 单段名；后端强制校验不含 `/`、`\`、`..`（§3.11.3） |
| `class` | `StorageInventoryClass` | 互斥且穷尽（§3.11.3） |
| `scope` | `StorageLayer` | 该首段所在基根 |
| `entry` | `String \| null` | `declared` / `optionalNotMounted` 时给出对应 `E` 编号；其余 `null` |
| `scanBytes` | `u64 \| null` | 该子树扫描字节；触界为 `null` |
| `scanCount` | `u64 \| null` | 该子树扫描到的**常规文件条数**（与 `scanBytes` 同一次遍历、同一口径）；触界为 `null` |
| `declaredBytes` | `u64 \| null` | `declared` 节点：该首段下 `declared` 条目 `bytes` 之和（任一为 `null` 则 `null`）；其余类节点恒 `0`（该节点**没有**登记，是已知事实，不是「不知道」） |
| `declaredCount` | `u64 \| null` | `declared` 节点：该首段下 `declared` 条目 `count` 之和（任一为 `null` 则 `null`）；其余类节点恒 `0` |
| `residualBytes` | `u64 \| null` | `scanBytes − declaredBytes`；`scanBytes` 为 `null` 时 `null` |
| `residualCount` | `u64 \| null` | `scanCount − declaredCount`；`scanCount` 为 `null` 时 `null` |
| `overRegistered` | `bool` | `declaredBytes > scanBytes`（理念 6 的报警位） |
| `children` | `Vec<StorageInventoryItemDto>` | 仅 `unregistered` 节点有明细，其余为空数组 |

`StorageInventoryItemDto` 仅 `name`（单段名）+ `bytes`（`u64 \| null`）+ `count`（`u64 \| null`）；**不含**递归子路径、不含绝对路径。

**`StorageInventoryStatusDto`**（`refresh_storage_inventory` 的返回；**只回状态、不回数字**）：

| 字段 | 类型 | 约束 |
|---|---|---|
| `status` | `StorageInventoryStatus` | 必填 |
| `scannedAt` | `i64 \| null` | 上次成功扫描完成时刻 |
| `failedAt` | `i64 \| null` | 上次失败时刻 |

它的三个字段与 `StorageInventoryDto` 的同名部分**同义、同构造点填充**（避免两处口径漂移）；之所以单列，是为了让「刷新」命令在类型层面就无法回数字（§6.1 边界 9）。

**契约规则**：

1. 输出 `Option` 不加 `skip_serializing_if`，键恒在、值 `null`；输入 `Option` 加 `#[ts(optional = nullable)]`；
2. 输入 DTO 全部 `deny_unknown_fields` + `rename_all = "camelCase"`；输出同理；
3. 新增 / 改动 DTO 后跑 `npm run contract:gen`，提交 `src/types/protocol.contract.ts`；禁止手改生成类型；
4. 13 条 key 作为 `String` 承载（前端渲染顺序固定、后端未知 key 拒绝），不建封闭枚举，避免条目增删时前端硬崩；
5. `bytes` / `count` 用 `u64`，前端按既有 DTO 约定转数字展示；不得在 DTO 里转 `f64`。

### 6.3 前端

1. **不新建 store、不新建 service**：数据由 `get_storage_breakdown` 一次拉全，回收动作在组件内串行调用；I54 §3 三 tab 页里的 `storage` tab 只做展示与确认（页面壳与另两个 tab 归 I54）；
2. **同页两段，均保留**（G14）：上半段是**既有附件明细区**（会话分组 / 孤儿分组 / 折叠 / 组全选 / 勾选删除，服务 `A2`），下半段是**13 行条目表**（服务观测与 `A1`/`A3`/`A4`/`A5`）。改造只允许「追加」与「调整布局」，**不得**删除附件明细区的任何既有能力（分组、折叠、全选、单选删除）；
3. 每个条目行渲染：名称、字节、条数、`availability`、`reclaim.eligibility` 徽标、`reclaim.action` 按钮、策略摘要；
4. 不可回收条目按钮禁用并显示 `reason` 文案；`notImplemented` 额外显示 `note`（「缺少 owner 方法」）；
5. 点击回收 → 先 `get_storage_reclaim_plan` → 弹确认（`strongConfirm` 需二次输入条目名，或长按 / 勾选）→ `reclaim_storage` → 结果 toast + 重新拉 `get_storage_breakdown`；
6. **禁止**前端自行计算「可回收字节」或「是否安全」；只显示后端给的值；也**禁止**前端按 `expectedBytes` 自行降档确认强度（档位由后端 `confirmation` 给定）；
7. `A2` 继续由附件明细区的既有勾选删除执行（走 `delete_attachments`）；13 行条目表**不**提供「删指定附件」的第二套入口，也**不得**把附件明细区的删除按钮搬进条目表；
8. 请求失败 / `planExpired` 时提示重试，不自动重放（避免「用户没看到就删了」）；
9. **对账余量区（第三段，只读）**：在 13 行条目表下方追加一栏「未登记余量」，渲染 `inventory`：
   - `status` 为 `neverRun` → 显示「尚未扫描」+ 「扫描」按钮（调 `refresh_storage_inventory`）；
   - `running` → 显示「扫描中」+ 禁用按钮；`failed` → 显示「扫描失败」+ `failedAt`，**同时照常显示上次成功的数字**（不因失败清空）；
   - `stale` → 显示旧数字 + 「数据可能过期」提示；收到 `storage:inventory-updated` 广播后重拉 `get_storage_breakdown`；
   - 默认列出 `class ∈ {unregistered, optionalNotMounted, companion, ignored}` 的节点；`declared` 节点**只在 `residualBytes > 0` 时**以折叠行出现（它的总量已在条目表里，出现是为了暴露「已登记目录里未被 owner 统计的部分」）；
   - `partial = true` 时按 `truncatedNodes` 提示「有界扫描未覆盖完，以下数字是下界」；
   - `overRegistered` 为 `true` 的节点**必须**显著标红并给固定文案（「登记值大于盘上实际值，可能是统计口径不一致」），不得静默；
10. **禁止**给余量区任何删除 / 清理按钮（§3.11 理念 5）：`unregistered` 不是「可回收」，它连 `reclaim` 字段都没有；也**禁止**把余量数字加进页面任何「总计」。

---

## 7 失败与降级

### 7.1 观测失败

| 情况 | 表现 | 不允许 |
|---|---|---|
| 目录不存在 | `bytes = 0`、`count = 0`、`availability = missing` | 报错整页失败；自动创建目录 |
| 权限不足 | `bytes / count = null`、`availability = unreadable` | 用 0 冒充空；回退到缓存旧值 |
| 遍历中断（IO 错误） | 同上 + `availabilityReason` 记错误类别 | 把已遍历部分当完整值上报 |
| 文件正被替换 / 迁移（`metadata` 失败） | `E6` / `E7` 的 `availability = unavailable` + `availabilityReason` 写「文件正在替换 / 迁移」 | 强行打开 DB；复制 DB 文件统计；用 `locked` 冒充（`locked` 只表达受保护读不到，见特例 2） |
| vault 锁定 | `E9`：`availability = locked`（词表 Q3 语义「存在但受保护读不到」）+ `availabilityReason` 写「vault 未解锁」+ `eligibility = locked`；`E6` / `E7` / `E10` **不受影响**（只读 `metadata` / 目录） | 为取长度而 unlock vault；把 `locked` 写成 `unavailable`（前者提示「去解锁」，后者提示「重试」，用户下一步不同） |
| 可选能力未装配（vaultless 构建） | `E7`–`E11` 五条 `availability = notImplemented` + `availabilityReason` 写「可选能力未装配」，`bytes` / `count = null`；`eligibility` 仍按禁止回收填（见 §3.10 第 3 条） | 因构建裁剪条目（`entries` 恒 13 条）；把 `notImplemented` 写成 `locked`；把 `E9` 因「没有这个 owner」而变成可回收 |
| 超时 | `bytes / count = null`、`availability = unavailable` | 无限等待阻塞 UI |
| 扫描失败（`inventory`） | `inventory.status = failed` + `failedAt`，保留上次 `fresh` 的数字；13 条**完全不受影响** | 让扫描失败连累条目；清空上次数字；把扫描失败写成条目 `unavailable` |
| 扫描触界（深度 / 条目数 / 耗时） | 该节点 `scanBytes = null` + `partial = true` + 记入 `truncatedNodes` | 无界继续扫（阻塞后台线程）；用已扫部分冒充完整值（那会让余量偏低，等于谎报「没东西」） |
| 扫描发现登记值大于扫描值 | 该节点 `overRegistered = true` + `partial = true` | 静默取 `0`（会把「口径不一致」永久隐藏，这是本节唯一的自检信号） |

**总则**：单条目失败只降级该条目，`get_storage_breakdown` 仍返回 200 语义；前端逐条展示，不整页报错。

**四条特例**（与总则冲突时以此为准）：

1. **`E1` 是唯一例外**：`totalBytes` / `totalCount` / `orphans` / `conversations` 是既有非 `Option` 字段（G13 不得改），所以 `E1` 采样失败时**整个命令返回 `Err`**，前端按既有逻辑整页报错。**禁止**为了让 `E1` 也降级而把旧字段改成 `Option` 或加 `default` 伪装成 0。
2. **`E6` / `E7` 只读 `metadata`（文件长度 / mtime），不需要解锁任何 DB**：SQLite 的锁只影响读写连接，`fs::metadata` 不受影响。因此 vault 锁定态下 `E7` 仍为 `availability = available`；`metadata` 本身失败（文件正在被替换 / 迁移）按 `availability = unavailable` 处理，**不**用 `locked`——`locked` 只表达「存在但受保护读不到」（`E9` 的 vault 锁定态）。**禁止**在采样前为了拿文件长度去 unlock vault。
3. **`E9` / `E10` 才是 vault 锁定态的对象**：`E9` 需要读 repo 结构 → 锁定态 `availability = locked`（`availabilityReason` 写「vault 未解锁」）、`eligibility = locked`；`E10` 只是清残留目录 → 锁定态也可读，失败按 `unreadable` 处理。
4. **vaultless 构建（provider 未注入）不是「锁定」，是「没有这个 owner」**：`E7`–`E11` 一律 `availability = notImplemented` + `availabilityReason` 写「可选能力未装配」；**禁止**用 `locked` 冒充（那会让用户以为「解锁就能看」，而实际构建里根本没有该 owner）。`E9` 的 `eligibility` 仍是 `notEligible`（`userData`）、`E8` 是 `notImplemented`（`noReclaimApi`）、`E10` / `E11` 是 `ownerManaged`；**禁止**因为 owner 不存在就把条目从 13 行里删掉（`entries` 恒 13 条，§6.2）。

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
| X2 | 用 `du` 式全盘扫描替代 owner stats | 慢、重复实现、跨平台差异；owner 最懂自己目录的结构与排除项。**边界**：§3.11 允许对基根做**有界对账扫描**，但它不替代任何条目的 `bytes`（条目数字仍只来自 owner），且不进 `entries`、不进任何合计、不可回收——被否决的是「替代」，不是「补充对账」 |
| X3 | 把回收合并进 `get_storage_breakdown` | 观测变有副作用，违反只读快照；前端一刷新就删数据不可接受 |
| X4 | 一键「清理全部」不区分条目 | 用户数据 / 业务数据 / 回放风险条目不可批量删除；必须逐条确认 |
| X5 | 在管理器里维护「已删除清单」做审计 | 与安全审计自指；且删除清单本身需要存储与保留策略 |
| X6 | 给每个条目建独立 Tauri 命令 | 命令爆炸、权限面扩大；统一走 plan + execute |
| X7 | 前端本地计算可回收量 | 前后端两套判据必然漂移；判据只能有一个来源 |
| X8 | 自动回收默认开启 | 默认 `retentionDays = 0`，用户显式配置后才生效；避免升级即删数据 |
| X9 | 用 `f64` 传字节 | 精度与契约不一致；沿用 `u64` |
| X10 | 管理器自建定时任务调度回收 | 与 owner 的启动 / 写入路径抢跑，且无 UI 时静默删数据；调度权只能归 owner |
| X11 | 把对账余量做成 13 条之外的第 14 条（或每类各建一条） | 余量不是资源、没有 owner、没有回收方法，建条目会让「条目 = 可观测 + 可回收 + 有 owner」的定义破功，还会诱导给它加 `reclaim` 字段。它是**差额**，只能挂在 `inventory` 上 |
| X12 | 让 `get_storage_breakdown` 等扫描完成（同步扫） | 扫描成本无上界（未知子树），必然拖慢页面；且余量变化频率低（TTL 5 分钟足够），没有实时性需求 |
| X13 | 扫描时直接 `du` 累加全部子树，不减去登记值 | 等于同时维护两套口径（owner stats 与自扫），任何一侧口径变化都产生静默漂移；减法只需一份扫描 + 已登记的减数 |

---

## 9 开放问题

| 编号 | 问题 | 当前处置 |
|---|---|---|
| `OQ1` | 跨会话附件「共享字节」是否需要在 UI 单列 | 本期只保证账本闭合（`total = once + shared + orphan`），不单列；`sharedBytes` 已在 breakdown 顶层 |
| `OQ2` | `E5` mascot 素材是否需要回收 | 本期 `none`；素材随版本走，属安装资产，删了会导致缺图 |
| `OQ3` | `E9` vault 是否允许「压缩历史」 | 本期不做；git 外部可改写，管理器无法保证判据，列为 `externalOwner` |
| `OQ4` | `E10` recovery 的清理判据 | 已定：**存在即清**（`cleanup_recovery_dir` 只在启动 / 恢复完成后调用），属 owner 自动路径，`eligibility = ownerManaged`、`reason = transient`；不给手动入口（避免删进行中的恢复）。无需用户配置 |
| `OQ5` | 是否支持按条目配置不同保留天数 | 本期只有两个 config 项：`result_retention_days`（`E3`）/ `audit_retention_days`（`E11`），均在 `AppConf`（见 §3.5）；`attachments` 无时间判据，其余用常量 |
| `OQ6` | `maxBytes` 配额何时生效 | 本期只展示不执行；需先有「配额命中后的行为」定义（拒绝写入还是触发回收） |
| `OQ7` | 回收是否需要进度事件 | 本期同步返回；条目数量有限，暂不引入事件流 |
| `OQ8` | `E13` 崩溃报告是否上报后删除 | **本期只观测**：`action = none`、`eligibility = notImplemented`、`reason = noReclaimApi`、`note` 点名缺裁剪入口（`assistant_crash` 现只导出 `install`）；上报流程未定，删除会丢诊断信息。`A4` 的 `pruneToLimit` 留待上报流程定案后再开（同时解除 §3.3 白名单限制） |
| `OQ9` | 附件 `A3` 是否暴露给用户 | 本期不暴露；孤儿回收由 `A1` 覆盖，单独入口易误用 |
| `OQ10` | 计划有效期 10 分钟是否合适 | 先按 10 分钟；若用户操作慢导致频繁过期，再调至 30 分钟 |
| `OQ11` | `MANAGED_ROOTS` 的 `inventory` 标志（哪些 scope 参与余量对账）本期取值 | 已定：**13 条全 `true`**。理由：余量按**首段**分组，同一首段下的多条登记（`E1`+`E2` 同属 `attachments`、`E7`–`E11` 同属 `security`）在减法里天然合并为「该节点登记字节之和」，无需按条目开关。保留该字段只为将来「某 scope 明确不参与对账」留位；本期禁止用 `false` 关掉任何 scope，否则该首段的 `declared` 节点会凭空变成 `unregistered`（把正常目录报成未登记） |

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
| V10 | 管理器源码中不存在 `fs::remove_*`、受管目录名字面量、`AppConf::load()`；`read_dir` 只允许出现在 `cmd/storage/inventory.rs`（§3.11.2 的唯一例外），且该文件内不得出现 `fs::remove_*` | 静态检查 / grep 门禁（按文件白名单断言） |
| V11 | `E1` 的 `A1` 执行与 `delete_attachments` 并发时不出现「删了仍被引用」；同一 ID 串行 | 并发单测 |
| V12 | `E3` 配置读失败时回退 `0`（禁用），不会按 30 天删；`E11` 无游标时 `eligibility ≠ eligible` | 单测：配置缺失 / 游标缺失 |
| V13 | `E6` / `E7` / `E9` 永远 `action = none` 且 `eligibility ≠ eligible`；`E9` 锁定态 `availability = eligibility = locked`、vaultless 构建 `availability = notImplemented` 且 `eligibility = notEligible`；`E5` 无自动删除入口 | 快照断言（`vault` / `vaultless` 两种构建各跑一遍） |
| V14 | DTO 契约生成后 `protocol.contract.ts` 与 Rust 一致；输入 DTO `deny_unknown_fields`，输出 `Option` 键恒在 | `npm run contract:gen` + 类型检查 |
| V15 | 回收留痕只含 `planId` / key / 动作 / 计数 / `sampledAt`，不含路径与文件名；`logs` 条目不被管理器删除 | 日志断言 + 代码审查 |
| V16 | **超集不窄化**：`StorageBreakdownDto` 既有 4 字段键名 / 类型 / 含义不变；`delete_attachments` / `open_attachment` / `import_attachment` / `get_asset_dirs` / `get_attachment_limits` 签名与语义不变；前端附件明细区（会话分组 / 孤儿分组 / 折叠 / 组全选 / 勾选删除）逐项仍可用 | 既有测试全绿 + 手动回归清单逐项打勾（G13 / G14） |
| V17 | **资格与可执行一致**：任意切片下，`eligibility = eligible` ⇔ 该 `key + action` 在 §3.3 白名单且 owner 方法存在（`reclaim_storage` 会接受）；不存在「显示可回收、执行被拒」的条目；`confirmation` 在非 `eligible` 时恒 `none` | golden 断言逐切片（§11 切片期资格表）+ 白名单负例 |
| V18 | **余量守恒与互斥**：`inventory.nodes` 的 `class` 互斥且穷尽；`declared + optionalNotMounted + companion + ignored + unregistered` 的首层条目数等于基根首层实际条目数；`declared` 节点 `residualBytes = scanBytes − Σ declaredBytes`、`residualCount = scanCount − Σ declaredCount`（且两项均 `≥ 0`，负值走 `overRegistered`）；非 `declared` 节点 `declaredBytes = declaredCount = 0`、`residualBytes = scanBytes`、`residualCount = scanCount`；`unregisteredBytes = Σ unregistered 节点 residualBytes`、`unregisteredCount = Σ residualCount`（`null` 传染，不填 0） | 单测：构造 `attachments/`（含 `derived/` 与一个未统计的额外子目录）+ `security/` + 未知目录 + `-wal` + `.DS_Store` 的临时基根，逐类与逐节点断言 |
| V19 | **登记表双向一致**：`MANAGED_ROOTS` 的 `entry` 集合 == 13 条 key 集合 == `entries[*].key`；`dir_name` 每新增常量若无对应登记则测试失败（`E2` 子项与 `E7`–`E11` `Feature` 挂载按例外处理）；`segments` 首段与 `roots.rs` getter 解析出的路径段一致 | 单测：集合比对 + 临时新增一条 `dir_name` 常量的负例（构造 `&[ManagedRoot]` 缺项） |
| V20 | **余量不影响条目**：`inventory` 任何数字变化（含 `failed` / `partial` / `overRegistered`）都不改变 `entries` 的 `bytes` / `count` / `eligibility`，也不改变 `totalBytes`；`get_storage_breakdown` 耗时与扫描状态无关（`running` 下仍立即返回）；`reclaim_storage` 拒绝任何 `inventory` 节点名作为 `key` | 单测：`running` / `failed` 两态各拉一次 breakdown，逐字段比对；负例请求 `key = "unregistered-name"` |

**整体验收**：V1–V20 全部通过，且 §3.8 的 G1–G14 在代码审查中逐条签字，方可视为「实现完成」；本文的设计状态已是定稿，实现期间若需改判据，必须先改本文再改代码。

---

## 11 实施切片

| 切片 | 范围 | 交付 | 退出条件 |
|---|---|---|---|
| T1 | **观测补全**：13 条 stats 入口（`E7`–`E11` 走 `StorageOwnerProbe` 注入，vaultless 构建降级）、`get_storage_breakdown` 扩展（`sampledAt` / `byteBasis` / `complete` / 账本字段 / `reclaim` 视图）、`availability` 降级、§3.10 的并行 + 500 ms 超时；**对账余量**（§3.11）：`app_dirs::registry` 登记表 + `dir_name` 常量收敛、`storage/inventory.rs` 有界扫描、`StorageInventoryDto` + 异步快照状态机、`storage:inventory-updated` 广播、可选 `refresh_storage_inventory` | 只读，无回收；`reclaim.action` 一律按 §3.1 矩阵填**终态动作**，尚未具备 owner 方法的条目体现为 `eligibility = notImplemented` + `reason = noReclaimApi`（禁止把 `action` 也降级成 `none`）；`inventory` 只读、不进任何合计 | V1–V5、V10、V13、**V18–V20** 通过；golden 快照可断言 13 行（前端表格归 T4）；`inventory` 在 `neverRun` / `running` / `failed` 三态下均不影响 `entries` |
| T2 | **骨架 + 低风险回收**：`reclaim_storage` 计划缓存 / `planId` / 确认 / 结果 DTO 全套骨架；`E2` / `E4` 的 owner stats + 回收方法（`A5`）；`E13` 仍只观测（OQ8）；`E1` 仍 `notImplemented`（执行方法留到 T3） | `reclaim_storage` 可执行 `A5`（`E2` / `E4`），其余请求按白名单拒绝 | V6–V10 通过；单条失败隔离；白名单外请求被拒 |
| T3 | **中风险回收**：`E3` 手动 `A3`（含 fail-closed 修正；owner 需新增「带字节的候选收集 + 删除」方法，见 §3.10 接口形状 1）、`E8` 对外 `pruneToLimit`（需把 `prune_backups` / `MAX_BACKUPS` 从 `pub(super)` / 私有提升为 provider 可达的公开入口）、`E1` 孤儿 `A1`（owner 按 ID 锁 + 执行时重算引用） | 用户可在 UI 触发三类回收；`E11` 仍只由 owner 调度 | V7、V9、V11、V12 通过；并发用例通过 |
| T4 | **前端与收尾**：`StorageSettings.tsx` 在**保留附件明细区**（会话分组 / 孤儿分组 / 勾选删除，G14）的前提下追加 13 行条目表、徽标、计划确认、结果 toast、**只读余量区**（§6.3 第 9–10 条）；i18n 两份；golden fixtures；留痕日志；文档定稿 | 完整存储管理器 tab（同页三段：附件明细 / 条目表 / 余量） | V14、V15 通过；全量 `contract:gen` + lint + 相关测试通过；附件明细区能力逐项回归 |

**切片依赖**：

1. T1 是 T2–T4 的前置：没有 stats 就无法给计划数字，禁止先做执行再补观测；
2. T2 的 plan / execute 骨架必须先于任何具体条目接入，避免每个条目各写一套确认逻辑；
3. T3 的 `E1` 孤儿执行依赖 T2 的骨架与 owner 的按 ID 锁，不得在 T2 之前上线；
4. T4 只做展示与交互，**不得**在 T4 新增后端判据；发现缺判据应回到 T1/T2/T3；
5. T1 内部顺序：`app_dirs::registry` 登记表**必须先于** `inventory.rs` 扫描落地（扫描按首段匹配，没有登记表就无从分类）；登记表同时是 `roots.rs` getter 的新来源，先改登记表可让「新增目录必须登记」的 V19 从第一天就生效；
6. 余量与回收**无依赖**：`inventory` 在 T1 完成即终态，T2–T4 不得给它加 `reclaim` 字段、不得把它接入 `reclaim_storage`（G10 / V20）。

**切片期资格（golden 断言口径）**：`eligibility` 必须反映**当时代码的真实可执行性**（§3.1 末条硬规则），下表是各切片的期望值；`eligible` 之外一律 `reason = noReclaimApi`（缺方法）或 `unknown`（判据不可算），`confirmation` 一律 `none`。

| 条目 | T1 结束 | T2 结束 | T3 结束 | T4（终态） |
|---|---|---|---|---|
| `E1` | `notImplemented` | `notImplemented` | 有孤儿 → `eligible`（`strongConfirm`） | 同 T3 |
| `E2` | `notImplemented` | 有孤儿派生 → `eligible`（`confirm`） | 同 T2 | 同 T2 |
| `E3` | `notImplemented` | `notImplemented` | `retentionDays > 0` → `eligible`（`confirm`） | 同 T3 |
| `E4` | `notImplemented` | `eligible`（`confirm`） | 同 T2 | 同 T2 |
| `E5` / `E6` / `E7` / `E9` | `notEligible`（`E9` 锁定态为 `locked`、vaultless 为 `notEligible`） | 同左 | 同左 | 同左（永不 `eligible`） |
| `E8` | `notImplemented` | `notImplemented` | 备份数 > 5 → `eligible`（`confirm`） | 同 T3 |
| `E10` / `E11` / `E12` | `ownerManaged` | 同左 | 同左 | 同左 |
| `E13` | `notImplemented` | 同左 | 同左 | 同左（OQ8 未定案） |

**T2 的 `E4` 为何可以 `eligible`**：`SuperIndex` 缓存已有落盘文件与 LRU 逐出逻辑（N7），T2 只需把它包装成 owner 的「列出 + 删除可重建缓存」方法，不需要新判据；`E2` 同理（基附件是否存在已可判）。`E1` / `E3` / `E8` 则需要新的执行语义（按 ID 锁、手动入口封装、对外裁剪），因此压在 T3。

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
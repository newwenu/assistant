# I64 — vault 接入暴露面与特性存储接入方式审计规格

| 项 | 值 |
|---|---|
| 状态 | 修订（2026-11-21 二次复核 + 同日实施：D1 / D4 已落地，见 §10 / §11 / §12；N10 / N12 / D2 / D3 / D7 已被 T3 收尾追平） |
| 日期 | 2026-11-20（2026-11-21 复核修订；同日二次复核重写 D4；同日按 T2 落地 D1 / D4） |
| 关联 | I61（存储管理器：T1 / T2 / T3 均已落地、T3 清单已同步、T4 未开工）；I62（协议 crate 抽取，DTO 将迁 crate）；I63（领域归属，N21/X2 决定 `DirStats` 四份不合并）；`I61-storage-manager-spec.md`（§3.9 / §5.5 / V2 / V7 / V17 / V21）；`I61-storage-manager-tasklist.md`（T3 已勾选，残留偏差见 §7）；`vault-optional-build.md`（已过期，见 §7） |
| 影响范围 | `src-tauri/src/ipc.rs`、`src-tauri/src/cmd/storage/{entries,sample,inventory,reclaim,probe}.rs`、`src-tauri/src/vault_plugin.rs`、`src-tauri/src/cmd/vault/**`、`src-tauri/src/setup.rs`、`src-tauri/src/feature_mounts.rs`、`src-tauri/build.rs`、`src-tauri/capabilities/vault.json`、4 个 owner crate（`assistant-storage` / `assistant-tools` / `assistant-vault` / 宿主自身）的统计与清理入口、`src/services/featureIpc.ts` |
| 前端影响 | 零契约改动；仅提出 `featureInvoke` 运行时开关与存储页接入的门禁（§5） |

> 一句话：vault 自身的 feature 门控在工作区改动中**没有**退化，但 I61 把「观测 + 回收发起」提升为宿主 IPC 后，**新增了一条不受 `vault:default` 能力约束、却能读 vault 元数据并删除其备份的通道**；T3 收尾（golden 刷新、清单勾选、E3/E8 用例补齐）已于 2026-11-20 落地，本文初稿记录的「必红的 T2 golden」「清单全未勾」已失效。两条接入方式缺陷已于 2026-11-21 按 T2 落地：**D1「E8 硬编码 5」**（已改 owner 自述 + fail-closed）与 **D4「伴生后缀大小写漂移」**（管理器侧已对齐 owner 判定）。仍待处理的是 `validated_targets` 不校验 `eligibility` 的语义缺口（D3 残余）与 D5 / D6 / D8–D12 的 P1–P2 项。

**编号约定**：`N` 现状事实、`E` 暴露面、`D` 缺陷、`G` 门禁、`V` 验收判据、`T` 修复切片、`X` 拒绝方案、`OQ` 开放问题。`E7–E11` 等 `Exxx` 形式保留 I61 的条目编号，不与本文 `Exx` 暴露面编号混淆（本文暴露面编号统一写 `EXP-x`）。

---

## 1 定位与边界

### 1.1 本规格解决什么

1. **把新增暴露面写清楚**：区分「vault feature 自身门控」（未变）与「存储管理器总线」（新增），给出可达路径、现有缓解与残余风险。
2. **把已核实缺陷固化成可验收条目**：每条缺陷必须有 `file:line` 证据、违反的规格条款、修复方向。
3. **定门禁口径**：能力门、单一事实源、golden 同步、文档同步四类，防止同类问题复发。

### 1.2 本规格不解决什么

- **不重新设计 I61 架构**：`I61-storage-manager-spec.md` §1「对外唯一入口、对内非唯一执行者」继续成立；本文只在既有骨架上补洞。
- **不改 I62 / I63 的顺序约束**：DTO 搬迁与领域归属按各自规格执行；本文缺陷若落在待迁文件上，随迁时一并修（§9 OQ5）。
- **不实现 T4 前端页面**：只给出门禁与验收，实施归 I61 T4。
- **不判定 vault 是否应默认启用**：`config/features.json` 的开关与 Cargo feature 的关系属 OQ4。

---

## 2 现状事实

| 编号 | 事实 | 证据 |
|---|---|---|
| N1 | Cargo feature `vault = ["assistant-vault","assistant-sync","assistant-security"]`，`default = ["vault"]`；三个 crate 均为 `optional` | `src-tauri/Cargo.toml:150-154`、`:200-202` |
| N2 | 全 Rust 树仅 **3** 处 `cfg(feature = "vault")`：`cmd/mod.rs:20`、`vault_plugin.rs:5`、`vault_plugin.rs:259`。`cmd/mod.rs` 已无 `pub use vault::*`，文档所述的通配泄漏**已修复** | `cmd/mod.rs:20-21`、`vault_plugin.rs:5,259,274` |
| N3 | 72 个 `vault_*` 命令在插件自己的 `invoke_handler` 中，宿主 `ipc.rs` 无 vault 命令 | `vault_plugin.rs:166-237`、`ipc.rs:97-100` |
| N4 | `capabilities/vault.json` 只授予 `vault:default`，窗口 `["main","orb"]`；`build.rs:5-40` 从源码文本解析命令名并 `AllowAllCommands`，不感知 cfg | `capabilities/vault.json:5-6`、`build.rs:5-40` |
| N5 | `feature_mounts.rs:15-20` 的 vault 挂载项**无 cfg**；关 feature 时 `vault_plugin.rs:259-272` 仍构造名为 `vault` 的空插件，ACL 仍全量生成 | `feature_mounts.rs:8-24`、`vault_plugin.rs:259-272` |
| N6 | 新增 4 个宿主存储命令：`get_storage_breakdown` / `refresh_storage_inventory` / `get_storage_reclaim_plan` / `reclaim_storage`，均不出现在插件 ACL | `ipc.rs:97-100` |
| N7 | `VaultStorageProbe` 把 E7–E11（`security.db` / `backups` / `vault` / `recovery` / `audit`）的字节、条数、mtime 暴露给管理器；`prune_backups` 会经 `prune_backups_to` 物理删除文件 | `vault_plugin.rs:66-132`、`state.rs:515-562` |
| N8 | 管理器自身零 `AppConf::load()`；结果保留天数在 `tool_setup.rs` 单点注入；审计保留天数**两处**读 | `cmd/storage/**`（grep 为 0）、`tool_setup.rs:39-51`、`setup.rs:82`、`cmd/vault/mod.rs:350-351` |
| N9 | `MAX_BACKUPS` 以 `pub` 导出，注释明确「让 provider 不必硬编码 5」；**已按 D1 落地改为 owner 自述**：管理器侧 `entries.rs:196` 现为 `owner_policy()`（无字面量），`probe.rs:70` 的 `max_backups()` 由 `vault_plugin.rs:118` 用 `u32::try_from(MAX_BACKUPS).ok()` 实现，`sample.rs:206,480-499` 负责注入 | `security_db/mod.rs:30`、`entries.rs:196`、`probe.rs:64-72`、`vault_plugin.rs:118-120`、`sample.rs:206,493-499` |
| N10 | E3 / E8 的 `static_for` 已是 `Eligible`（`entries.rs:149` / `:194`），T2 golden **已按终态矩阵刷新**为 `t3_eligibility_matches_slice_table`（`:619-651`，断言 `1\|2\|3\|7` → `Eligible`、`0\|4\|5\|6\|8` → `NotEligible`）；D1 落地后 `samples()` 的 E8 也改为注入 owner 自述份数（`count = 6` + `keep = 5`）；`cargo test --lib cmd::storage` 41 条全绿（2026-11-21 实施后实测） | `entries.rs:149,194,619-651`、`entries.rs:573-579,606-609`（`backups_policy` 与 E8 样本） |
| N11 | 伴生后缀表字面量出现 5 处，但归属是 **4 个 owner + 1 个管理器**（不是 5 个 owner）：`attachment_store.rs`（`E1`/`E2`）、`runtime/storage.rs`（`E3`/`E4`）、`storage_stats.rs`（`E7`/`E8`/`E11`）、`fs_stats.rs`（`E5`/`E6`）各一份属 owner 边界内；`inventory.rs` 那份属管理器的对账扫描器，**不覆盖任何条目**。四个 owner 侧都做小写归一化；管理器侧已于 2026-11-21 按 D4 对齐为同一口径（`inventory.rs:99-104` 先 `to_ascii_lowercase()` 再比对） | `fs_stats.rs:81,83-90`、`inventory.rs:40,99-104`、`attachment_store.rs:79,82-89`、`runtime/storage.rs:528,530-537`、`storage_stats.rs:104,106-113` |
| N12 | T3 执行器与白名单已落地（`reclaim.rs:332-341,557-645,646-770`），tasklist `:323-370` 的 T3-a/b/c/d 已全部 `[x]` 并附核对；E3 有集成用例 `retention_zero_is_policy_disabled_and_deletes_nothing`（`tests/storage_reclaim.rs:682`），E8 有 owner 侧 5 条用例（`crates/assistant-vault/tests/security_db.rs:302-391`）；**但** `reclaim_storage` 经 probe 触达 E8 的集成路径仍无用例（测试装配的 `ReclaimPorts.probes` 恒 `None`） | `reclaim.rs:332-341,557-645,646-770`、`I61-storage-manager-tasklist.md:323-370`、`tests/storage_reclaim.rs:48-67,682`、`security_db.rs:302-391` |
| N13 | 前端 `featureInvoke` 无条件拼 `plugin:vault\|<cmd>`；保留的 `src/vault/` 持模块引用即可调用全部命令 | `featureIpc.ts:15-17`、`features/registry.ts:20-22`、`vault/core/vault.service.ts:72-295` |
| N14 | `docs/store/vault-optional-build.md` 描述的是旧设计（65 命令在 `lib.rs`、`get_vault_conf` 在 `cmd/config.rs`、通配泄漏未修），与 N2/N3 冲突 | `vault-optional-build.md:3,89-90,111,276-289`、`command_registry.rs:5,178-197` |

---

## 3 暴露面清单

### EXP-1 宿主存储命令触达 vault 观测与删除，无能力门（新增）

- **路径**：`ipc.rs:97-100`（宿主命令）→ `get_storage_reclaim_plan` → `reclaim_storage`（`reclaim.rs:241-330`）→ `execute_backups`（`:646-770`）→ `StorageOwnerProbe::prune_backups`（`probe.rs:70`）→ `VaultStorageProbe`（`vault_plugin.rs:111-132`）→ `SecurityDbState::prune_backups_to`（`state.rs:515-562`，`fs::remove_file`）。
- **观测侧**：`vault_plugin.rs:66-104` 经 `get_storage_breakdown` / `refresh_storage_inventory` 暴露 E7–E11 的字节数、条数、最旧/最新 mtime。
- **现有缓解**：白名单（`reclaim.rs:332-341`）、`confirmed` 标志（`:245`）、10 分钟计划 TTL（`PLAN_TTL_SECONDS = 600`，`:29`）、单实例闸门（`begin_execution`，`:136`）、`keep.max(1)`（`state.rs:519`）、错误信息去路径（`reclaim.rs:775`）、结构化日志只记 plan/key/计数。
- **残余风险**：任意能调用宿主命令的 WebView 可驱动 vault 备份删除，**无需 `vault:default`**；`capabilities/vault.json` 与生成 ACL 都覆盖不到这条路径。这是 I61 之前不存在的新面。
- **结论**：新增暴露面成立；是否收口见 OQ1（能力门落点未定）。

### EXP-2 关 feature 构建仍挂载空插件并生成全量 ACL（既有）

- `feature_mounts.rs:15-20` 无 cfg，`vault_plugin.rs:259-265` 构造空插件，`build.rs:5-40` 仍解析出 72 个命令并 `AllowAllCommands`。产物差异只在 invoke handler，不在 capability 层。属 `vault-optional-build.md` M7/T1 类问题，未被本次改动引入，但被本次审计确认仍在。

### EXP-3 前端无运行时开关校验（既有）

- `featureIpc.ts:15-17` 无条件构造 `plugin:vault|<cmd>`；唯一门是 `registry.ts:20-22` 的动态 import。`vault:default` 无 scope，全部 72 命令对 main/orb 窗口开放。`vault.contract.ts:62` / `reveal.rs:87,121` 按设计把明文跨 WebView 边界返回（注释自承边界无法清零）。

### EXP-4 配置 fail-open（既有，部分新路径沿用）

- `vault_plugin.rs:242-252` 配置损坏只记日志后 `Ok(())`；`cmd/vault/mod.rs:311-312` 剪贴板秒数失败回退 30、`:185-189` `use_pageant` 默认 true、`:185,289` `VaultConf::load().ok()`；`vault_plugin.rs:152-160` Probe 槽缺失只记日志继续（观测静默降级）。
- 反向 fail-closed 样本：`cmd/vault/mod.rs:393-396` 锁屏策略 `unwrap_or(true)`；`state.rs:519` 备份至少留 1。

### EXP-5 路径与文件名进入错误 / 日志（部分新增、部分既有）

- `state.rs:556` 把完整备份路径写进持久化日志；`lifecycle.rs:236-237` 把含路径的 `io::Error` 转成 IPC 错误；`cmd/mod.rs:266-268` `delete_attachments` 把 attachment id（即文件名）写进错误（既有语义，G13 允许）。
- 管理器侧与 probe 侧干净：`storage_stats.rs:132-151` 的 IO 错误含路径，但被 `vault_plugin.rs:40-47` 的 `map_io` 丢弃；`sample.rs:41-57` 只保留 `error.kind()`。

### EXP-6 锁屏回调与退出钩子使用 panicking 状态访问（既有）

- `cmd/vault/mod.rs:398,429,440` 在监视线程 / 退出钩子上用 `app.state::<SecurityDbState>()`（panic 式），而 `vault_plugin.rs:50-56` 已用防御式 `try_state`。装配正常时不会触发，但与既有防御风格不一致。

---

## 4 接入方式缺陷

### D1（P0，已落地 2026-11-21）E8 的 keep 由 owner 自述，管理器零字面量

- **原证据（修复前）**：`entries.rs:196` `constant_policy(Some(5))`；owner 侧 `security_db/mod.rs:30` `pub const MAX_BACKUPS: usize = 5`；`vault_plugin.rs` 把管理器的 `keep` 透传给 `prune_backups_to`；`backups_eligibility` 用 `unwrap_or(1)` 兜底。
- **违反**：`I61-storage-manager-tasklist.md:343`「keep 由 owner 侧常量 `MAX_BACKUPS` 传入，不得在管理器里硬编码 5」；§3.1 E8、§5.5；本文 G3。
- **落地结果**（按原「修复形状」五步逐条实施）：
  1. `probe.rs:63-72` 新增可选能力 `fn max_backups(&self) -> Option<u32>`，默认 `None`，doc 明确「管理器不得自填默认值」；
  2. `vault_plugin.rs:118-120` 实现为 `u32::try_from(MAX_BACKUPS).ok()`——事实源仍是 owner 常量；
  3. `sample.rs:206-218` 新增 `max_backups_of`（按登记表 `E8` 行的能力名找 provider），`sample.rs:332-338` 新增 `keep_policy`（`source = owner`、`keep_count = 自述值`），`sample.rs:480,493-499` 在采样汇总后把该值注入 `E8` 样本的 `policy`——与 `E11` 的 `audit_policy` 走同一条「策略自述覆盖」通道；
  4. `entries.rs:196` 改 `owner_policy()`（`keep_count` 留空，与 `E4` 同形）；`backups_eligibility`（`entries.rs:388-416`）改读 `sample.policy.keep_count`，删掉 `unwrap_or(1)`；份数或条数任一缺失即 `notEligible` + `unknown`（fail-closed，不再用 `0` 冒充「没超限」）；`build_entry` 的 `sample.policy.unwrap_or(static_part.policy)` 让自述值自动进计划快照；
  5. 执行侧 `execute_backups` 未改：它从计划快照的 `planned_policy.keep_count` 取数（`reclaim.rs:646-770`），而计划快照与判据来自同一次采样，故自动同源。
- **验收**（G3 / V3）：新增 `e8_keep_comes_from_owner_self_report`（`sample.rs:770-800`：自述原样透传、`keep_policy` 来源/份数、未装配/未自述均为 `None`）与 `e8_without_owner_self_report_is_unknown_not_guessed`（`entries.rs:762-805`：无自述 → `notEligible` + `unknown` 且 `policy.keepCount = null`；条数缺失 → 同样不可算）。`cargo test --lib cmd::storage` 41 passed / 0 failed，`cargo test --lib` 174 passed / 0 failed，`storage_breakdown` / `storage_inventory` / `storage_reclaim` 集成测试全绿（2026-11-21 实测）。
- **未做**：没有把份数搬进 `StorageReclaimPlanDto` 之外的新字段，也没有在管理器侧留任何 `5` 的字面量——`grep` `constant_policy(Some(` 现仅剩 `E12` 的 `Some(3)`（日志轮转份数，属宿主自身 owner 口径，见 D5 相邻项）。

### D2（已修复，2026-11-21 复核撤销）T2 golden 已刷新

- **复核结论**：初稿记录的「golden 必红」已不成立。`t2_eligibility_matches_slice_table` 已改名为 `t3_eligibility_matches_slice_table`（`entries.rs:619-651`），断言 `1|2|3|7` → `Eligible`、`0|4|5|6|8` → `NotEligible`、`9..=11` → `OwnerManaged`、`12` → `NotImplemented`；`samples()`（`:573-609`）已给 E3 注入 `retention = 30` 的 config 策略、给 E8 注入 `count = 6` + owner 自述份数 `5`，因此 `eligibility_for` 的 `policyDisabled` / `overLimit` 两条降级分支不会再被误判为 eligible。
- **实测**：`cargo test --lib cmd::storage` 41 passed / 0 failed（2026-11-21 D1 落地后复测）。
- **残余**：无需修复；若后续改 `static_for` 或 `samples()`，G5 要求与实现同批更新。

### D3（P1，部分过期）清单已同步，但 `validated_targets` 不校验 eligibility

- **已失效的部分**：tasklist `:323-370` 的 T3 条目已全部 `[x]` 并附核对；E3 有 V12 集成用例（`tests/storage_reclaim.rs:682`），E8 有 owner 侧 5 条用例（`crates/assistant-vault/tests/security_db.rs:302-391`）。「T3 落地但清单与验收未跟上」不再成立。
- **仍成立的语义缺口**：`validated_targets`（`reclaim.rs:342-356`）只校验 key / action / 白名单 / 计划成员，不校验计划里的 `eligibility`。计划标记为 `notEligible` 的请求仍被接受，执行侧返回 `Skipped`（`:589`、`:661`）——无误删，但与 V17「eligibility ⇔ 可执行」不符。
- **仍成立的测试缺口**：集成层装配的 `ReclaimPorts.probes` 恒 `None`（`tests/storage_reclaim.rs:48-67`），因此 `reclaim_storage` 经 probe 触达 E8 的**端到端**路径无用例；E8 的裁剪逻辑只在 owner 侧被覆盖。
- **修复**：要么在 `validated_targets` 拒绝非 eligible，要么把「接受但 Skipped」写进规格并加用例；E8 的集成用例按 §9 OQ5 与 I62 一并排期（需在集成层可构造 probe 替身）。

### D4（P1，管理器侧已落地 2026-11-21）伴生后缀在管理器侧未做大小写归一，产生虚假「未登记」告警

- **重写说明**：初稿把本条写成「后缀规则五份重复，应抽成单一声明」，这是**误读** I61 的 G15 / X14。原文明确区分两件事：
  - G15：「口径声明唯一，**遍历实现归 owner**」「判据**不是**『全仓只有一份遍历代码』，而是『同一棵 fixture 目录喂给任意 owner 的 stats 入口，四元组逐字一致』（V21）」；
  - X14：「被否决的是『口径规则多份』，**不是**『遍历代码多份』——后者是所有者边界，正当（G15）」。

  因此 4 个 owner 各持一份后缀表与遍历代码是**正当的**（`E1`/`E2`→`assistant-storage`、`E3`/`E4`→`assistant-tools`、`E7`/`E8`/`E11`→`assistant-vault`、`E5`/`E6`→宿主自身），N11 已把归属纠正为「4 个 owner + 1 个管理器」。**真正成立的缺陷只剩一条**：管理器侧那份与 owner 侧**行为不一致**。
- **证据（修复前）**：四个 owner 侧的 `is_companion_file` 都做 `to_ascii_lowercase()`（`fs_stats.rs:83-90`、`attachment_store.rs:82-89`、`runtime/storage.rs:530-537`、`storage_stats.rs:106-113`）；`inventory::classify`（原 `inventory.rs:96-99`）直接对原始 `name` 调 `ends_with`，而 `classify` 的入参正是首层节点名（如根目录下的 `assistant.db-WAL`）。后果：同名文件在 owner 侧被跳过（不计入 `bytes`/`count`），在扫描侧却落到 `unregistered`——§3.11.3 里的**主信号**，而不是应有的 `companion` 单列，于是产生「未登记」假告警；反过来 `declared` 节点若含大写伴生文件，余量也会天天非零（§3.11.3 特意把伴生单列正是为了避免这种噪声）。
- **违反**：V21（`I61-storage-manager-spec.md:1049`）——同一 fixture 喂给每个入口四元组必须一致；G15（`:368`）的行为一致判据；X14（`:999`）「被否决的是口径规则多份，不是遍历代码多份」。
- **修复（已落地 2026-11-21）**：
  1. 管理器侧一行语义：`inventory::classify` 的伴生判定改为大小写不敏感（`inventory.rs:96-104`，先 `to_ascii_lowercase()` 再比对），**未动** `COMPANION_SUFFIXES` 字面量所在位置，也未抽共享模块；
  2. 补用例：`companion_suffixes_are_case_insensitive`（`inventory.rs:701-721`，覆盖 `assistant.db-WAL` / `-SHM` / `-JOURNAL` / `X.TMP` / `X.LOCK` / `DB-journal`）；`uppercase_companion_suffix_is_not_unregistered`（`tests/storage_inventory.rs:386-425`，真实临时目录里放 `assistant.db-WAL` / `assistant.db-SHM`，断言两节点均 `companion`、无 `entry`、无 `unregistered`）；
  3. 负例已由现有 `stray` → `unregistered` 断言（`tests/storage_inventory.rs:99-160`）与上述用例共同锁死：改名不归一即落 `unregistered`，测试必红。
- **残余（未落地）**：修复形状第 2 步的**完整跨 owner 版本**（一棵 fixture 同时喂 `E2` / `E3` / `E4` / `E5` / `E6` / `E7` / `E8` / `E11` 的 stats 入口并比对四元组）未实现：集成层目前无法构造 `E7` / `E8` / `E11` 所需的 `SecurityDbState`，且各 owner stats 入口需要各自可注入的 store 实例。该用例与 D3 的「E8 端到端集成用例」同批排期（§9 OQ5，随 I62 的 DTO 搬迁一并做），落地前 V21 在管理器侧已有真实目录级替代用例。
- **明确不做**（见 §8 X5）：不在 `assistant-storage` 新建共享 `companion` 模块、不给 `assistant-tools` / `assistant-vault` 加依赖边。理由有三：① V21 原文点名「**不**以『`struct DirStats` 定义计数 == 1』为判据——结构份数由依赖方向决定，一致性由行为决定」；② `fs_stats.rs:9-12` 已写明域 crate 不为共享小结构互相依赖；③ 为 5 个字符串字面量引入 `rusqlite`/`tokio` 级重依赖，收益远小于成本。

### D5（P1）E13 crash 统计口径不符 §3.9

- **证据**：`crash_dump.rs:32-67` 的 `walk_stats` 接受任意 `meta.is_file()`，既不按 `panic_*.log` / `crash_*.dmp` / `crash_*.log` 过滤，也无伴生后缀过滤；`tasklist:73-74` 保留未勾选，且无 `dump_stats` 用例。
- **后果**：E13 的 `{bytes,count,oldestAt,newestAt}` 可能包含非 crash 文件，与 inventory 的 segmented 对账不一致。

### D6（P1）审计保留天数两处读配置，默认值与 X8 冲突

- **证据**：`setup.rs:82`（`AppConf::load().map(..).unwrap_or(0)`，供 DB 审计行裁剪）与 `cmd/vault/mod.rs:350-351`（`db_state.set_audit_retention_days`）各读一次；结果保留天数已在 `tool_setup.rs:39-51` 单点注入，审计未照做。
- **口径冲突**：X8（`I61-storage-manager-spec.md:993`）写「默认 retentionDays = 0，用户显式配置后才生效」，而 §3.5 表与 `constant.rs:57,61` 的 `RESULT_RETENTION_DAYS=30` / `AUDIT_RETENTION_DAYS=90` 是 30/90。规格内部不一致，需定案。

### D7（P2，部分过期）`PolicyDisabled` 错误码从未产出；清单注释过期

- **已失效的部分**：`entries.rs:363-366` 已产出 `NotEligible + PolicyDisabled`，且已有用例覆盖——`entries.rs:722-729`（`t3_dynamic_eligibility_follows_policy_and_count` 的 E3 段）与 `tests/storage_reclaim.rs:682-731`（V12）。「已实现但未测」不再成立。
- **仍成立的部分**：`StorageReclaimErrorCodeDto::PolicyDisabled`（`protocol/dto/storage.rs:192`）从未产出（全仓仅枚举定义，无构造点）；`tasklist:174-176` 的「全仓 `PolicyDisabled` 仅在枚举定义 / 无任何实现或用例」已过期，需按 §7 更新。

### D8（P2）vault 备份删除三套实现，符号链接处理不一致

- **证据**：`state.rs:492` `purge_backups`（`read_dir` + remove，无 symlink 检查）、`:515` `prune_backups_to`（新 public，symlink-safe、计字节）、`:564` `prune_backups`（`pub(super)`，自动备份路径 `:466` 与 `rekey.rs:240` 使用）。三套同目录清理口径不同，是 owner 自动路径与管理器发起路径之间的潜在 G5 差异。

### D9（P2）E9 锁定文案与规格不符

- **证据**：`sample.rs:162`、`entries.rs:830` 写「尚未解锁」；规格 `:175,397,933,946` 要求「vault 未解锁」；`tasklist:153-154` 因此保留未勾选。文案差异影响用户下一步动作（解锁 vs 重试的语义区分，见规格 §7.1）。

### D10（P2）部分 `ProbeStats` 造成 availability 自相矛盾

- **证据**：`sample.rs:136-156` 在 `bytes=Some(x), count=None` 时把 `availability` 置 `Unreadable`、reason 写「该条目未提供这项数据」，却保留 `bytes=Some(x)`；V2 / G6（规格 `:1022`）要求取不到的数据 `bytes/count` 均 `null`。无用例覆盖（`sample.rs:520-533` 只断言 `count = None`）。

### D11（P2）E4 预计字节与实际删除字节差距已知未闭

- **证据**：`super_index/mod.rs:33-43` 只删 `*.idx`，而 `index_stats` 计所有常规文件（外来文件被计不被删；`.tmp` 伴生既不计也不删）；注释自承「预计字节可能略大于实际删除字节」。V7 无测试（`tasklist:357` 未勾）。

### D12（P2）文档与清单滞后于代码

- `vault-optional-build.md:3,89-90,111,276-289` 仍描述旧布局（65 命令在 `lib.rs`、`get_vault_conf` 在 `cmd/config.rs`、通配泄漏未修），与 N2/N3 冲突。
- `command_registry.rs:5,178-197` 只匹配 `vault_` 前缀命令，**新存储命令触达 vault 数据/删除却不在覆盖内**（与 EXP-1 叠加）。
- tasklist 中已消失的偏差仍在列：`:141-142` 称 E7–E11 未走 `spawn_blocking`（现 `sample.rs:443` 已 `timed_blocking`）；`:174-176` 称 `PolicyDisabled` 无实现或用例（现已有，见 D7）；`:266` 称 busy 用例待补（现 `storage_reclaim.rs:416` 已有）。

---

## 5 门禁与验收

| 编号 | 门禁 | 验收 |
|---|---|---|
| G1 | 凡触达 vault 文件删除的 IPC 命令必须有能力门；宿主存储命令不得绕过 `vault:default` | V1：以无 `vault:default` 窗口调 `reclaim_storage` targeting E8，返回权限拒绝且零副作用； EXP-1 路径逐段可追 |
| G2 | 所有触达 vault 的命令（含宿主存储命令）纳入 `command_registry` 断言，不按 `vault_` 前缀过滤 | V2：注册表测试给出「vault 数据可达命令」清单，`reclaim_storage` / `get_storage_reclaim_plan` 在列 |
| G3 | owner 常量不得在 manager 内复制；manager 只消费 owner 自述值 | V3：改 `MAX_BACKUPS` 为 3 后仅改 owner 处，E8 的 `policy.keepCount` / `eligibility` 与实际裁剪同步；provider 未自述份数时判据为 `notEligible` + `unknown`（已落地，见 D1） |
| G4 | 伴生后缀的**判定行为**在 owner 与管理器之间一致（口径声明唯一在 I61 §3.9，遍历实现归 owner，不要求代码共享） | V4：`FOO.WAL` / `foo.wal` 在四个 owner stats 入口与 `inventory::classify` 得到同一分类（`companion` / 不计入四元组）；管理器侧已由 `companion_suffixes_are_case_insensitive` 与 `uppercase_companion_suffix_is_not_unregistered` 锁死（D4 已落地）；跨 owner 同一 fixture 的完整用例待 OQ5 |
| G5 | golden 与实现同批更新；eligibility ⇔ 可执行 | V5：`cargo test --lib cmd::storage` 全绿（2026-11-21 D1 落地后实测 41 passed / 0 failed）；E3/E8 在 eligible / notEligible 两态下 plan 与 execute 一致（V17；`validated_targets` 缺口见 D3） |
| G6 | 每个保留策略配置只在一个组合根读取并注入 | V6：`grep AppConf::load` 在 `cmd/storage/**` 为 0；审计天数仅一处读，E3/E11 的 `policy` 与启动裁剪使用同一 `effective_since` |
| G7 | 监视线程 / 退出钩子不用 panicking 状态访问 | V7：`cmd/vault/mod.rs` 的 `state::<>` 在回调路径为 0 处，`try_state` 全覆盖 |
| G8 | vault 文件路径与文件名不进入 IPC 错误和持久化日志 | V8：触发备份裁剪失败与备份超额，断言错误串与日志不含 `security/` 路径片段 |
| G9 | E13 / E4 / E9 的口径偏差按本文 D5 / D11 / D9 收敛 | V9：`dump_stats` 有文件名过滤用例；E4 写清 expected vs removed 差额；E9 文案 golden 写「vault 未解锁」 |

---

## 6 修复切片

| 切片 | 内容 | 对应缺陷 | 前置 |
|---|---|---|---|
| T1 暴露面收口 | 定 OQ1 落点；宿主存储命令能力门（或显式记录「宿主可信」例外并加 G2 清单）；`featureInvoke` 运行时开关；`build.rs` ACL 随 cfg 收敛 | EXP-1/2/3、D12 | OQ1/OQ4 定案 |
| T2 判据修复 | **已落地（2026-11-21）**：E8 keep 改 owner 自述（`probe.max_backups()` + `sample.rs` 注入 + `owner_policy()` + fail-closed）；`inventory::classify` 伴生判定改大小写不敏感 + 管理器侧真实目录用例；**残余**：`validated_targets` 拒绝非 eligible（或把「接受但 Skipped」写入 V17）、E8 端到端集成用例（与 I62 同批） | D1（已落地）/D4（已落地）/D3（残余） | —（D2 已完成，不再计入） |
| T3 配置与生命周期 | 审计配置单点注入 + 默认口径定案；备份删除三套归一；E9 文案；E13 文件名过滤；panicking 访问改 `try_state` | EXP-4/5/6、D5/D6/D8/D9 | OQ2 |
| T4 文档与清单同步 | 更新 `vault-optional-build.md`、`I61` tasklist 残留偏差（`:141-142` / `:174-176` / `:266`）、`command_registry` 覆盖面 | D12 | T1–T3 |

---

## 7 文档与清单同步要求

1. `docs/store/vault-optional-build.md`：标注「描述旧布局，已被插件边界取代」，或按 N2/N3 重写 §3.2 / §3.4 / §4.5 / §7.2。
2. `docs/store/I61-storage-manager-tasklist.md`：T3 已按实际落地情况勾选并附核对（2026-11-20 完成，本次复核确认）；**仍待清理**的残留偏差为 `:141-142`（E7–E11 已改 `timed_blocking`）、`:174-176`（`PolicyDisabled` 已有实现与用例）、`:266`（busy 用例已落地）；保留真实未决项（`:73-74` E13、`:153-154` E9 文案、`:357-362` V7/V11/V12 中仍属实的部分、T4 全部）。
3. `I61-storage-manager-spec.md`：X8 与 §3.5 的默认 retention 口径二选一（OQ2）；把 D3 的「接受但 Skipped」写入 V17 或改 `validated_targets`。

---

## 8 拒绝方案

| ID | 方案 | 拒绝理由 |
|---|---|---|
| X1 | 把 vault 从管理器整体排除（不注册 E7–E11） | I61 明确纳入 E7–E11 且 vaultless 已有 `notImplemented` 降级路径；排除等于放弃可观测性，且 `CAPABILITY` 缺口仍在（宿主命令本身仍可被调用） |
| X2 | 把 `MAX_BACKUPS` 复制一份常量到 manager 消除「引用 owner crate」 | 制造第二事实源，D1 的根因就是值重复而非依赖方向；应走 owner 自述 |
| X3 | 新建统一 crate 容纳 `DirStats` 与伴生后缀 | I63 N21 / X2 已决定 `DirStats` 四份不合并；问题只在后缀**规则**重复，一份声明 + 测试即可 |
| X4 | 用 cfg 把存储命令从 no-vault 构建整体移除 | 存储管理器是宿主能力，E7–E11 在 vaultless 下已有 `notImplemented` 语义；移除会让 13 条恒在契约失效 |
| X5 | 把伴生后缀判定抽到 `assistant-storage` 共享模块，并让 `assistant-tools` / `assistant-vault` 各加一条依赖边「强制单一来源」 | 把**行为一致性**问题改造成**结构共享**问题，正好踩 V21 明确排除的判据（「结构份数由依赖方向决定，一致性由行为决定」）；X14 已写明「遍历代码多份是所有者边界，正当」；`fs_stats.rs:9-12` 反对域 crate 为共享小结构互相依赖；为 5 个字符串字面量引入 `rusqlite`/`tokio` 级重依赖。正确做法是修管理器侧大小写漂移 + 补 V21 行为等价用例（D4） |

---

## 9 开放问题

| ID | 问题 | 影响 |
|---|---|---|
| OQ1 | 宿主存储命令是否纳入 Tauri capability 体系？若纳入，新增 `storage:default` 还是复用 `vault:default`？ | 决定 EXP-1 的 G1 落点；当前宿主命令不在任何 ACL 清单里 |
| OQ2 | 默认 `retentionDays = 0`（X8）与 §3.5 / `constant.rs` 的 30/90 冲突，谁是事实源？ | 决定 D6 修复方向（默认即清理 vs 显式配置才清理） |
| OQ3 | `--no-default-features` 是否应彻底不生成 `vault:default` ACL（需改 `build.rs` 的源码解析时机）？ | EXP-2；影响二进制攻击面与文档 M7 结论 |
| OQ4 | 前端 `featureInvoke` 是否加运行时 `enabled` 校验，还是仅依赖 `registry.ts` 的动态 import 单点？ | EXP-3；影响保留 `src/vault/` 时的可达性 |
| OQ5 | I62 把 DTO 迁 `assistant-protocol` 后，D3/D10 的用例路径随之变化，是否与 I62 T2 合并执行？ | 与 I62 OQ1 同顺序约束，避免重复搬迁；E8 集成用例需在集成层可构造 probe 替身 |

---

## 10 复核记录（2026-11-21）

复核方式：逐条对照代码与文档证据，并实跑 `cargo test --lib cmd::storage`（当时 38 passed / 0 failed；D1 落地后为 41）。

**结论成立、无需改动**：N1–N9、N11、N13、N14；EXP-1–EXP-6（其中 EXP-1 的「宿主存储命令无能力门却能删 vault 备份」链路完整可达，为本文核心结论）；D1、D4、D5、D6、D8、D9、D10、D11、D12。

**已被 T3 收尾追平（本文相应条目已改写）**：

| 原条目 | 初稿判断 | 复核后事实 | 处置 |
|---|---|---|---|
| N10 / D2 | T2 golden 必红 | golden 已刷新为 `t3_eligibility_matches_slice_table`，38 条全绿 | N10 改写为现状；D2 标记「已修复」 |
| N12 / D3 | tasklist 全 `[ ]`、无 E3/E8 用例 | T3 已全部 `[x]`；E3 有 V12 集成用例、E8 有 owner 侧 5 条 | N12 改写；D3 降为 P1，只保留 `validated_targets` 语义缺口与 E8 集成用例缺口 |
| D7 | `policyDisabled` 已实现但未测 | 已有两处用例（`entries.rs:722-729`、`storage_reclaim.rs:682`） | D7 降为 P2，只保留「`PolicyDisabled` 错误码未产出」与 tasklist 注释过期 |

**行号漂移修正**（初稿写于 `cmd/storage.rs` 拆分与 T3 收尾之前，语义未变）：`cmd/mod.rs:21`→`:20`、`vault_plugin.rs:165-238`→`:166-237`、`cmd/vault/mod.rs:338`→`:350-351`、`attachment_store.rs:64`→`:79`、`crash_dump.rs:32-62`→`:32-67`、`sample.rs:134`→`:162`、`entries.rs:635`→`:830`、`reclaim.rs:311-318`→`:332-341`、`reclaim.rs:534-630`→`:646-770`、`cmd/vault/mod.rs:385,416,427`→`:398,429,440`。

**当前待修优先级（2026-11-21 实施后更新）**：P0 已清零（D1 已落地）；P1 剩 D3 残余（`validated_targets` 不校验 eligibility）、D5（E13 口径）、D6（审计天数两处读 + X8 冲突）；D4 已落地。P2 为 D7–D12 与 §6 的 T1 / T3 / T4。

---

## 11 二次复核记录（2026-11-21 同日）

复核方式：对照 `git status`（工作区代码零改动，仅两份文档为 Modified）逐条回读代码，确认 D1 / D4 均**未落地**，并纠正初稿与首轮复核对 D4 的判据误读。

| 项 | 首轮复核结论 | 二次复核结论 | 处置 |
|---|---|---|---|
| D1 落地状态 | 首轮曾按「probe 自述」方向试做（`StorageOwnerProbe::max_backups` + `VaultStorageProbe` 实现 + `sample.rs` 注入 + `entries.rs` 去硬编码），后**全部回退** | 代码现状仍是 `entries.rs:196` `constant_policy(Some(5))`、`backups_eligibility` `unwrap_or(1)`、`probe.rs` 无 `max_backups`、`sample.rs` 无注入 | D1 保留 P0；补写「修复形状」五步与 fail-closed 要求，供下次一次性落地 |
| D4 判据 | 「伴生后缀规则五份重复且大小写漂移」，修复 = 抽单一声明 | 误读 G15 / X14 / V21。重复的是**遍历代码**，属 owner 边界，正当；唯一真缺陷是 `inventory::classify` 未做大小写归一 | D4 重写为「大小写漂移」；N11 改为「4 owner + 1 管理器」；G4 改为行为一致；新增 X5 拒绝共享 crate 方案 |
| N11 归属 | 写成「5 处重复」 | `attachment_store.rs`（E1/E2）、`runtime/storage.rs`（E3/E4）、`storage_stats.rs`（E7/E8/E11）、`fs_stats.rs`（E5/E6）为 owner；`inventory.rs` 为管理器扫描器，不覆盖条目 | N11 改写并补各行 `is_companion_file` 行号 |
| 依赖边 | 曾新增 `assistant-tools` / `assistant-vault` → `assistant-storage` 两条 Cargo 边，并新建 `assistant-storage/src/companion.rs` | 违反 V21 判据与 `fs_stats.rs:9-12` 的分层取向 | 已全部回退（`companion.rs` 不存在、两条 Cargo 边无残留、`attachment_store.rs` 恢复本地 helper）；方案记入 X5 |
| 行号复核 | — | `entries.rs:196`（E8 策略）、`:385-397`（`backups_eligibility`）、`:588`（`samples()` 的 E8 `count=6`）、`:619-651`（`t3_eligibility_matches_slice_table`）、`inventory.rs:96-99`（伴生判定）、`reclaim.rs:342-356`（`validated_targets`）、`:589,661,754`（`Skipped`）、`tests/storage_reclaim.rs:48-67`（`probes: None`）均与现码一致 | 无漂移，沿用 |

> 本节为历史记录：其中「D1 / D4 均未落地」的判断已被 §12 的实施结果取代，行号亦以 §12 为准。

---

## 12 实施记录（2026-11-21，T2 落地）

实施方式：按 §4 D1 / D4 已确认的修复形状逐条落地，只动管理器侧与 provider 的「自述」缝，未新增跨 crate 依赖边、未新建共享模块（守 X5 / V21）。

| 项 | 落地内容 | 代码位置 | 用例 |
|---|---|---|---|
| D1-a 接口 | `StorageOwnerProbe` 新增可选能力 `max_backups() -> Option<u32>`，默认 `None`，doc 写明「份数是 owner 私有常量，管理器不得自填默认值」 | `probe.rs:64-72` | — |
| D1-b owner 自述 | `VaultStorageProbe` 实现为 `u32::try_from(MAX_BACKUPS).ok()`，事实源仍是 owner 常量 | `vault_plugin.rs:118-120` | — |
| D1-c 采样注入 | `max_backups_of(probes)` 按登记表 `E8` 行的能力名分派；`keep_policy(keep)` 造 `source = Owner` 的策略；采样汇总末尾把自述值挂到 `E8` 样本 | `sample.rs:206-218,332-338,480,493-499` | `e8_keep_comes_from_owner_self_report`（`sample.rs:770-800`） |
| D1-d 判据 | `E8` 的 `static_for` 改 `owner_policy()`（`keep_count` 留空）；`backups_eligibility` 读 `sample.policy.keep_count`，删 `unwrap_or(1)`，份数或条数缺失即 `notEligible` + `unknown` | `entries.rs:160,196,388-416` | `e8_without_owner_self_report_is_unknown_not_guessed`（`entries.rs:762-805`）；`t3_eligibility_matches_slice_table` 仍全绿 |
| D1-e 执行侧 | 无需改动：`execute_backups` 从计划快照 `planned_policy.keep_count` 取数，与判据同源 | `reclaim.rs:646-770`（未改） | — |
| D4-a 判定 | `inventory::classify` 伴生判定前先 `name.to_ascii_lowercase()`，与四个 owner 侧 `is_companion_file` 同口径 | `inventory.rs:96-104` | `companion_suffixes_are_case_insensitive`（`inventory.rs:701-721`） |
| D4-b 目录级用例 | 真实临时目录放 `assistant.db-WAL` / `assistant.db-SHM`，断言两节点均 `companion`、`entry` 为空、无 `unregistered` | — | `uppercase_companion_suffix_is_not_unregistered`（`tests/storage_inventory.rs:386-425`） |

**验证（2026-11-21 实测）**：

- `cargo test --lib`：174 passed / 0 failed（其中 `cmd::storage` 41 条）；
- `cargo test --test storage_breakdown --test storage_inventory --test storage_reclaim`：29 passed / 0 failed（6 + 9 + 14，`storage_inventory` 较修复前 +1 即本条新增用例）；
- `cargo fmt --check`：无差异；`cargo clippy --lib --tests`：无警告（2026-11-21 实测）。

**未做 / 留待后续**：

1. D4 的跨 owner 行为等价用例（一棵 fixture 同时喂 E2 / E3 / E4 / E5 / E6 / E7 / E8 / E11）仍缺，受限于集成层无法构造 `SecurityDbState`，与 D3 的 E8 端到端用例同批排期（OQ5）；
2. `validated_targets` 仍不校验 `eligibility`（D3 残余），本次未动，避免把「接受但 Skipped」的语义单方面收紧；
3. 管理器侧已无 `MAX_BACKUPS` 字面量，`constant_policy(Some(` 仅剩 `E12` 的日志轮转份数（宿主自身 owner 口径）。
# I64 — vault 接入暴露面与特性存储接入方式审计规格

| 项 | 值 |
|---|---|
| 状态 | 草案（问题已核实，待排期修复；本文只记录与定口径，不改代码） |
| 日期 | 2026-11-20 |
| 关联 | I61（存储管理器：T1/T2 已落地、T3 后端已落地但清单未勾、T4 未开工）；I62（协议 crate 抽取，DTO 将迁 crate）；I63（领域归属，N21/X2 决定 `DirStats` 四份不合并）；`I61-storage-manager-spec.md`（§3.9 / §5.5 / V2 / V7 / V17 / V21）；`I61-storage-manager-tasklist.md`（清单滞后，见 §7）；`vault-optional-build.md`（已过期，见 §7） |
| 影响范围 | `src-tauri/src/ipc.rs`、`src-tauri/src/cmd/storage/{entries,sample,inventory,reclaim,probe}.rs`、`src-tauri/src/vault_plugin.rs`、`src-tauri/src/cmd/vault/**`、`src-tauri/src/setup.rs`、`src-tauri/src/feature_mounts.rs`、`src-tauri/build.rs`、`src-tauri/capabilities/vault.json`、5 个 owner crate 的统计 / 清理入口、`src/services/featureIpc.ts` |
| 前端影响 | 零契约改动；仅提出 `featureInvoke` 运行时开关与存储页接入的门禁（§5） |

> 一句话：vault 自身的 feature 门控在工作区改动中**没有**退化，但 I61 把「观测 + 回收发起」提升为宿主 IPC 后，**新增了一条不受 `vault:default` 能力约束、却能读 vault 元数据并删除其备份的通道**；同时 T3 后端抢在清单与 golden 之前落地，留下「E8 硬编码 5」「必红的 T2 golden」「伴生规则五份且大小写漂移」三类可验证缺陷。

**编号约定**：`N` 现状事实、`E` 暴露面、`D` 缺陷、`G` 门禁、`V` 验收判据、`T` 修复切片、`X` 拒绝方案、`OQ` 开放问题。`E7–E11` 等 `Exxx` 形式保留 I61 的条目编号，不与本文 `Exx` 暴露面编号混淆（本文暴露面编号统一写 `EXP-x`）。

---

## 1 定位与边界

### 1.1 本规格解决什么

1. **把新增暴露面写清楚**：区分「vault feature 自身门控」（未变）与「存储管理器总线」（新增），给出可达路径、现有缓解与残余风险。
2. **把已核实缺陷固化成可验收条目**：每条缺陷必须有 `file:line` 证据、违反的规格条款、修复方向。
3. **定门禁口径**：能力门、单一事实源、golden 同步、文档同步四类，防止同类问题复发。

### 1.2 本规格不解决什么

- **不重新设计 I61 架构**：`I61-storage-manager-spec.md` §1「对外唯一入口、对内非唯一执行者」继续成立；本文只在既有骨架上补洞。
- **不改 I62 / I63 的顺序约束**：DTO 搬迁与领域归属按各自规格执行；本文缺陷若落在待迁文件上，随迁时一并修（§9 OQ1）。
- **不实现 T4 前端页面**：只给出门禁与验收，实施归 I61 T4。
- **不判定 vault 是否应默认启用**：`config/features.json` 的开关与 Cargo feature 的关系属 OQ4。

---

## 2 现状事实

| 编号 | 事实 | 证据 |
|---|---|---|
| N1 | Cargo feature `vault = ["assistant-vault","assistant-sync","assistant-security"]`，`default = ["vault"]`；三个 crate 均为 `optional` | `src-tauri/Cargo.toml:149-155`、`:200-202` |
| N2 | 全 Rust 树仅 **3** 处 `cfg(feature = "vault")`：`cmd/mod.rs:21`、`vault_plugin.rs:5`、`vault_plugin.rs:259`。`cmd/mod.rs` 已无 `pub use vault::*`，文档所述的通配泄漏**已修复** | `cmd/mod.rs:21-22`、`vault_plugin.rs:5,259,274` |
| N3 | 72 个 `vault_*` 命令在插件自己的 `invoke_handler` 中，宿主 `ipc.rs` 无 vault 命令 | `vault_plugin.rs:165-238`、`ipc.rs:2` |
| N4 | `capabilities/vault.json` 只授予 `vault:default`，窗口 `["main","orb"]`；`build.rs:5-40` 从源码文本解析命令名并 `AllowAllCommands`，不感知 cfg | `capabilities/vault.json:5-6`、`build.rs:5-40` |
| N5 | `feature_mounts.rs:16-19` 的 vault 挂载项**无 cfg**；关 feature 时 `vault_plugin.rs:263-265` 仍构造名为 `vault` 的空插件，ACL 仍全量生成 | `feature_mounts.rs:8-24`、`vault_plugin.rs:259-272` |
| N6 | 新增 4 个宿主存储命令：`get_storage_breakdown` / `refresh_storage_inventory` / `get_storage_reclaim_plan` / `reclaim_storage`，均不出现在插件 ACL | `ipc.rs:97-100` |
| N7 | `VaultStorageProbe` 把 E7–E11（`security.db` / `backups` / `vault` / `recovery` / `audit`）的字节、条数、mtime 暴露给管理器；`prune_backups` 会经 `prune_backups_to` 物理删除文件 | `vault_plugin.rs:63-133`、`state.rs:515-562` |
| N8 | 管理器自身零 `AppConf::load()`；结果保留天数在 `tool_setup.rs` 单点注入；审计保留天数**两处**读 | `cmd/storage/**`（grep 为 0）、`tool_setup.rs:39-51`、`setup.rs:82`、`cmd/vault/mod.rs:338` |
| N9 | `MAX_BACKUPS` 以 `pub` 导出，注释明确「让 provider 不必硬编码 5」，但 `entries.rs` 仍写 `constant_policy(Some(5))` | `security_db/mod.rs:30`、`entries.rs:193` |
| N10 | E3 / E8 的 `static_for` 已是 `Eligible`（`:146` / `:191`），而 T2 golden 仍断言二者为 `NotImplemented`（`:559`），用例必红 | `entries.rs:146,191,555-565` |
| N11 | 伴生后缀表字面量重复 5 处；owner 侧小写归一化，`inventory::classify` 用原始 `ends_with` | `fs_stats.rs:81,83-90`、`inventory.rs:40,96-99`、`attachment_store.rs:64`、`runtime/storage.rs:528,534`、`storage_stats.rs:104,110` |
| N12 | T3 执行器与白名单已落地，但 `I61-storage-manager-tasklist.md:323-362` 全标 `[ ]`；`storage_reclaim.rs` 无 E3/E8 执行用例 | `reclaim.rs:311-318,445-526,534-630`、`tests/storage_reclaim.rs` |
| N13 | 前端 `featureInvoke` 无条件拼 `plugin:vault|<cmd>`；保留的 `src/vault/` 持模块引用即可调用全部命令 | `featureIpc.ts:15-17`、`features/registry.ts:18-20`、`vault/core/vault.service.ts:72-295` |
| N14 | `docs/store/vault-optional-build.md` 描述的是旧设计（65 命令在 `lib.rs`、`get_vault_conf` 在 `cmd/config.rs`、通配泄漏未修），与 N2/N3 冲突 | `vault-optional-build.md:3,89-90,111,276-289`、`command_registry.rs:5,179-197` |

---

## 3 暴露面清单

### EXP-1 宿主存储命令触达 vault 观测与删除，无能力门（新增）

- **路径**：`ipc.rs:97-100`（宿主命令）→ `get_storage_reclaim_plan` → `reclaim_storage`（`reclaim.rs:231-291`）→ `execute_backups`（`:534-630`）→ `StorageOwnerProbe::prune_backups`（`probe.rs:70`）→ `VaultStorageProbe`（`vault_plugin.rs:117-132`）→ `SecurityDbState::prune_backups_to`（`state.rs:515-562`，`fs::remove_file`）。
- **观测侧**：`vault_plugin.rs:72-107` 经 `get_storage_breakdown` / `refresh_storage_inventory` 暴露 E7–E11 的字节数、条数、最旧/最新 mtime。
- **现有缓解**：白名单（`reclaim.rs:311-318`）、`confirmed` 标志（`:235`）、10 分钟计划 TTL（`:28-29,113-133`）、单实例闸门（`:100-135`）、`keep.max(1)`（`state.rs:519`）、错误信息去路径（`reclaim.rs:646-661`）、结构化日志只记 plan/key/计数（`:273-282`）。
- **残余风险**：任意能调用宿主命令的 WebView 可驱动 vault 备份删除，**无需 `vault:default`**；`capabilities/vault.json` 与生成 ACL 都覆盖不到这条路径。这是 I61 之前不存在的新面。
- **结论**：新增暴露面成立；是否收口见 OQ1（能力门落点未定）。

### EXP-2 关 feature 构建仍挂载空插件并生成全量 ACL（既有）

- `feature_mounts.rs:16-19` 无 cfg，`vault_plugin.rs:263-265` 构造空插件，`build.rs:5-40` 仍解析出 72 个命令并 `AllowAllCommands`。产物差异只在 invoke handler，不在 capability 层。属 `vault-optional-build.md` M7/T1 类问题，未被本次改动引入，但被本次审计确认仍在。

### EXP-3 前端无运行时开关校验（既有）

- `featureIpc.ts:15-17` 无条件构造 `plugin:vault|<cmd>`；唯一门是 `registry.ts:18-20` 的动态 import。`vault:default` 无 scope，全部 72 命令对 main/orb 窗口开放。`vault.contract.ts:62` / `reveal.rs:87,121` 按设计把明文跨 WebView 边界返回（注释自承边界无法清零）。

### EXP-4 配置 fail-open（既有，部分新路径沿用）

- `vault_plugin.rs:246-250` 配置损坏只记日志后 `Ok(())`；`cmd/vault/mod.rs:296-300` 剪贴板秒数失败回退 30、`:171-185` `use_pageant` 默认 true、`:353-365` `VaultConf::load().ok()`；`vault_plugin.rs:152-162` Probe 槽缺失只记日志继续（观测静默降级）。
- 反向 fail-closed 样本：`cmd/vault/mod.rs:381-384` 锁屏策略 `unwrap_or(true)`；`state.rs:519` 备份至少留 1。

### EXP-5 路径与文件名进入错误 / 日志（部分新增、部分既有）

- `state.rs:556` 把完整备份路径写进持久化日志；`lifecycle.rs:235-236` 把含路径的 `io::Error` 转成 IPC 错误；`cmd/mod.rs:266-269` `delete_attachments` 把 attachment id（即文件名）写进错误（既有语义，G13 允许）。
- 管理器侧与 probe 侧干净：`storage_stats.rs:132-151` 的 IO 错误含路径，但被 `vault_plugin.rs:40-47` 的 `map_io` 丢弃；`sample.rs:41-57` 只保留 `error.kind()`。

### EXP-6 锁屏回调与退出钩子使用 panicking 状态访问（既有）

- `cmd/vault/mod.rs:385,416,427` 在监视线程 / 退出钩子上用 `app.state::<SecurityDbState>()`（panic 式），而 `vault_plugin.rs:52-56` 已用防御式 `try_state`。装配正常时不会触发，但与既有防御风格不一致。

---

## 4 接入方式缺陷

### D1（P0）E8 的 keep 在管理器硬编码，违反单一事实源

- **证据**：`entries.rs:193` `constant_policy(Some(5))`；owner 侧 `security_db/mod.rs:30` `pub const MAX_BACKUPS: usize = 5`（注释点名 I61 T3-b）；`vault_plugin.rs:117-132` 把管理器的 `keep` 透传给 `prune_backups_to`。全仓 `cmd/storage` 与 `vault_plugin` 未引用 `MAX_BACKUPS`。
- **违反**：`I61-storage-manager-tasklist.md:341`「keep 由 owner 侧常量 `MAX_BACKUPS` 传入，不得在管理器里硬编码 5」；§3.1 E8、§5.5。
- **后果**：改 `MAX_BACKUPS` 后 UI 判据（`count > 5` 才 eligible）与实际裁剪份数脱节。
- **修复**：由 owner 经 probe 自述 `maxBackups`（复用 `audit_policy()` 的同一模式，新增派生查询，而非让管理器读常量）；manager 只消费自述值。

### D2（P0）T2 golden 必红

- **证据**：`entries.rs:146`（E3 `Eligible`）、`:191`（E8 `Eligible`）；`:555-565` 的 `t2_eligibility_matches_slice_table` 仍把 index `0 | 2 | 7 | 12` 断言为 `NotImplemented`；`samples()`（`:518-527`）给 E3/E8 的是 `observed(1024,1,…)` 且无 policy，经 `eligibility_for` 只会得到 `NotEligible + PolicyDisabled`（`:301-321`）/ `NotEligible + OverLimit`（`:339-353`）。
- **旁证**：`entries.rs` mtime 19:23:52 晚于 tasklist 19:12:10（2026-11-20），即 T3 后端抢在 golden 之前落地。
- **修复**：按终态矩阵刷新 golden，并补 E3/E8 在 `policyDisabled` / `overLimit` 下的用例；与 D3 同批。

### D3（P0）T3 已落地但清单与验收未跟上

- **证据**：白名单 `reclaim.rs:311-318` 已含 `E3+A3`、`E8+A4`；执行器 `:445-526`、`:534-630`；`runtime/storage.rs:242-251,332-377` 已有 `collect_expired` / `delete_candidates`；`state.rs:515` 已 public。但 tasklist `:323-362` 全 `[ ]`，`tests/storage_reclaim.rs` 无 E3/E8 执行用例，V7 / V11 / V12 无测试。
- **附带语义缺口**：`validated_targets`（`:320-340`）只校验 key / action / 白名单，不校验计划里的 `eligibility`。计划标记为 `notEligible` 的请求仍被接受，执行侧返回 `Skipped`（`:471-479`、`:543-551`）——无误删，但与 V17「eligibility ⇔ 可执行」不符。
- **修复**：补 E3/E8 的 plan/execute/golden 用例；要么在 `validated_targets` 拒绝非 eligible，要么把「接受但 Skipped」写进规格并加用例。

### D4（P1）伴生后缀规则五份重复且大小写漂移

- **证据**：`fs_stats.rs:81`、`inventory.rs:40`、`attachment_store.rs:64`、`runtime/storage.rs:528`、`storage_stats.rs:104` 各写一份同一字面量。owner 侧 `is_companion_file` 做 lowercase（`fs_stats.rs:83-90`、`runtime/storage.rs:534`、`storage_stats.rs:110`），`inventory::classify` 用原始 `ends_with`（`inventory.rs:96-99`）。`FOO.WAL` 被 owner 统计跳过，却被 scanner 判为 `unregistered`，产生虚假 residual。
- **违反**：G15 / X14（`I61-storage-manager-spec.md:347,355,991`）要求规则**单一声明**，允许遍历代码重复；V21（`:1041`）行为等价测试不存在。
- **修复**：把后缀判定抽成单一声明（复用 I63 已定的「`DirStats` 四份不合并」结论——重复的是规则，不是结构体），并补 V21 行为等价用例。

### D5（P1）E13 crash 统计口径不符 §3.9

- **证据**：`crash_dump.rs:32-62` 的 `walk_stats` 接受任意 `meta.is_file()`，既不按 `panic_*.log` / `crash_*.dmp` / `crash_*.log` 过滤，也无伴生后缀过滤；`tasklist:73-74` 保留未勾选，且无 `dump_stats` 用例。
- **后果**：E13 的 `{bytes,count,oldestAt,newestAt}` 可能包含非 crash 文件，与 inventory 的 segmented 对账不一致。

### D6（P1）审计保留天数两处读配置，默认值与 X8 冲突

- **证据**：`setup.rs:82`（`AppConf::load().map(..).unwrap_or(0)`，供 DB 审计行裁剪）与 `cmd/vault/mod.rs:338`（`db_state.set_audit_retention_days`）各读一次；结果保留天数已在 `tool_setup.rs:39-51` 单点注入，审计未照做。
- **口径冲突**：X8（`I61-storage-manager-spec.md:985`）写「默认 retentionDays = 0，用户显式配置后才生效」，而 §3.5 表与 `constant.rs:57,61` 的 `RESULT_RETENTION_DAYS=30` / `AUDIT_RETENTION_DAYS=90` 是 30/90。规格内部不一致，需定案。

### D7（P1）E3 `policyDisabled` 已实现但未测，任务清单结论过期

- **证据**：`entries.rs:317-321` 已产出 `NotEligible + PolicyDisabled`，但 `tasklist:175-176` 仍称「全仓 `PolicyDisabled` 仅在枚举定义」；`StorageReclaimErrorCodeDto::PolicyDisabled`（`protocol/dto/storage.rs:192`）从未产出；无对应用例。

### D8（P2）vault 备份删除三套实现，符号链接处理不一致

- **证据**：`state.rs:492` `purge_backups`（`read_dir` + remove，无 symlink 检查）、`:515` `prune_backups_to`（新 public，symlink-safe、计字节）、`:564` `prune_backups`（`pub(super)`，自动备份路径 `:466` 与 `rekey.rs:240` 使用）。三套同目录清理口径不同，是 owner 自动路径与管理器发起路径之间的潜在 G5 差异。

### D9（P2）E9 锁定文案与规格不符

- **证据**：`sample.rs:134`、`entries.rs:635` 写「尚未解锁」；规格 `:175,397,933,946` 要求「vault 未解锁」；`tasklist:153-154` 因此保留未勾选。文案差异影响用户下一步动作（解锁 vs 重试的语义区分，见规格 §7.1）。

### D10（P2）部分 `ProbeStats` 造成 availability 自相矛盾

- **证据**：`sample.rs:108-129` 在 `bytes=Some(x), count=None` 时把 `availability` 置 `Unreadable`、reason 写「owner 未给出该维度」，却保留 `bytes=Some(x)`；V2 / G6（规格 `:1022`）要求取不到的数据 `bytes/count` 均 `null`。无用例覆盖（`:478-489` 只断言 `count = None`）。

### D11（P2）E4 预计字节与实际删除字节差距已知未闭

- **证据**：`super_index/mod.rs:33-43` 只删 `*.idx`，而 `index_stats` 计所有常规文件（外来文件被计不被删；`.tmp` 伴生既不计也不删）；注释自承「预计字节可能略大于实际删除字节」。V7 无测试（`tasklist:357` 未勾）。

### D12（P2）文档与清单滞后于代码

- `vault-optional-build.md:3,89-90,111,276-289` 仍描述旧布局（65 命令在 `lib.rs`、`get_vault_conf` 在 `cmd/config.rs`、通配泄漏未修），与 N2/N3 冲突。
- `command_registry.rs:5,179-197` 只匹配 `vault_` 前缀命令，**新存储命令触达 vault 数据/删除却不在覆盖内**（与 EXP-1 叠加）。
- tasklist 中已消失的偏差仍在列：`:141-142` 称 E7–E11 未走 `spawn_blocking`（现 `sample.rs:407-416` 已 `timed_blocking`）；`:266` 称 busy 用例待补（现 `storage_reclaim.rs:390` 已有）。

---

## 5 门禁与验收

| 编号 | 门禁 | 验收 |
|---|---|---|
| G1 | 凡触达 vault 文件删除的 IPC 命令必须有能力门；宿主存储命令不得绕过 `vault:default` | V1：以无 `vault:default` 窗口调 `reclaim_storage`  targeting E8，返回权限拒绝且零副作用； EXP-1 路径逐段可追 |
| G2 | 所有触达 vault 的命令（含宿主存储命令）纳入 `command_registry` 断言，不按 `vault_` 前缀过滤 | V2：注册表测试给出「vault 数据可达命令」清单，`reclaim_storage` / `get_storage_reclaim_plan` 在列 |
| G3 | owner 常量不得在 manager 内复制；manager 只消费 owner 自述值 | V3：改 `MAX_BACKUPS` 为 3 后仅改 owner 处，E8 的 `policy.maxBytes`/判据与实际裁剪同步 |
| G4 | 伴生后缀规则单一声明；V21 行为等价测试存在 | V4：`FOO.WAL` / `foo.wal` 在 owner stats 与 inventory classify 得到同一分类；跨 crate 断言指向同一声明 |
| G5 | golden 与实现同批更新；eligibility ⇔ 可执行 | V5：`cargo test -p assistant-app --lib cmd::storage` 全绿；E3/E8 在 eligible / notEligible 两态下 plan 与 execute 一致（V17） |
| G6 | 每个保留策略配置只在一个组合根读取并注入 | V6：`grep AppConf::load` 在 `cmd/storage/**` 为 0；审计天数仅一处读，E3/E11 的 `policy` 与启动裁剪使用同一 `effective_since` |
| G7 | 监视线程 / 退出钩子不用 panicking 状态访问 | V7：`cmd/vault/mod.rs` 的 `state::<>` 在回调路径为 0 处，`try_state` 全覆盖 |
| G8 | vault 文件路径与文件名不进入 IPC 错误和持久化日志 | V8：触发备份裁剪失败与备份超额，断言错误串与日志不含 `security/` 路径片段 |
| G9 | E13 / E4 / E9 的口径偏差按本文 D5 / D11 / D9 收敛 | V9：`dump_stats` 有文件名过滤用例；E4 写清 expected vs removed 差额；E9 文案 golden 写「vault 未解锁」 |

---

## 6 修复切片

| 切片 | 内容 | 对应缺陷 | 前置 |
|---|---|---|---|
| T1 暴露面收口 | 定 OQ1 落点；宿主存储命令能力门（或显式记录「宿主可信」例外并加 G2 清单）；`featureInvoke` 运行时开关；`build.rs` ACL 随 cfg 收敛 | EXP-1/2/3、D12 | OQ1/OQ4 定案 |
| T2 判据修复 | E8 keep 改 owner 自述；刷新 T2 golden；补 E3/E8 plan/execute 用例；伴生规则单一声明 + V21 | D1/D2/D3/D4 | — |
| T3 配置与生命周期 | 审计配置单点注入 + 默认口径定案；备份删除三套归一；E9 文案；E13 文件名过滤；panicking 访问改 `try_state` | EXP-4/5/6、D5/D6/D8/D9 | OQ2 |
| T4 文档与清单同步 | 更新 `vault-optional-build.md`、`I61` tasklist（勾掉已落地项、保留真实未决项）、`command_registry` 覆盖面 | D12 | T1–T3 |

---

## 7 文档与清单同步要求

1. `docs/store/vault-optional-build.md`：标注「描述旧布局，已被插件边界取代」，或按 N2/N3 重写 §3.2 / §3.4 / §4.5 / §7.2。
2. `docs/store/I61-storage-manager-tasklist.md`：T3 按实际落地情况勾选；删除已消失的偏差（`:141-142`、`:175-176`、`:266`）；保留真实未决项（`:73-74` E13、`:153-154` E9 文案、`:357-362` V7/V11/V12、T4 全部）。
3. `I61-storage-manager-spec.md`：X8 与 §3.5 的默认 retention 口径二选一（OQ2）；把 D3 的「接受但 Skipped」写入 V17 或改 `validated_targets`。

---

## 8 拒绝方案

| ID | 方案 | 拒绝理由 |
|---|---|---|
| X1 | 把 vault 从管理器整体排除（不注册 E7–E11） | I61 明确纳入 E7–E11 且 vaultless 已有 `notImplemented` 降级路径；排除等于放弃可观测性，且 `CAPABILITY` 缺口仍在（宿主命令本身仍可被调用） |
| X2 | 把 `MAX_BACKUPS` 复制一份常量到 manager 消除「引用 owner crate」 | 制造第二事实源，D1 的根因就是值重复而非依赖方向；应走 owner 自述 |
| X3 | 新建统一 crate 容纳 `DirStats` 与伴生后缀 | I63 N21 / X2 已决定 `DirStats` 四份不合并；问题只在后缀**规则**重复，一份声明 + 测试即可 |
| X4 | 用 cfg 把存储命令从 no-vault 构建整体移除 | 存储管理器是宿主能力，E7–E11 在 vaultless 下已有 `notImplemented` 语义；移除会让 13 条恒在契约失效 |

---

## 9 开放问题

| ID | 问题 | 影响 |
|---|---|---|
| OQ1 | 宿主存储命令是否纳入 Tauri capability 体系？若纳入，新增 `storage:default` 还是复用 `vault:default`？ | 决定 EXP-1 的 G1 落点；当前宿主命令不在任何 ACL 清单里 |
| OQ2 | 默认 `retentionDays = 0`（X8）与 §3.5 / `constant.rs` 的 30/90 冲突，谁是事实源？ | 决定 D6 修复方向（默认即清理 vs 显式配置才清理） |
| OQ3 | `--no-default-features` 是否应彻底不生成 `vault:default` ACL（需改 `build.rs` 的源码解析时机）？ | EXP-2；影响二进制攻击面与文档 M7 结论 |
| OQ4 | 前端 `featureInvoke` 是否加运行时 `enabled` 校验，还是仅依赖 `registry.ts` 的动态 import 单点？ | EXP-3；影响保留 `src/vault/` 时的可达性 |
| OQ5 | I62 把 DTO 迁 `assistant-protocol` 后，D3/D10 的用例路径随之变化，是否与 I62 T2 合并执行？ | 与 I62 OQ1 同顺序约束，避免重复搬迁 |

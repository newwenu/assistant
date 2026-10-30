# I23 — vault IPC 契约收口 任务清单

> 规格：[vault-ipc-contract.md](./vault-ipc-contract.md)
> **执行决策：规格 §6（D1–D6）。阶段编号、提交切分、剔除项均以 §6 为准。**
> 阶段编号与规格 §5 一致：**S1-A → S1-B → S1-C → S2 → S3 → S4-0 → S4**
> 阶段依赖：S1-B 依赖 S1-A；S1-C 依赖 S1-B；S2 与 S3 依赖 S1-C；S4-0 依赖 S1-C；S4 依赖 S4-0 结论回填。
> **本文不记载任何可由命令签名派生的数字**（命令数、各文件条数、DTO 个数、字段数）。需要核对数量时直接读 `cmd/vault/*.rs` 的签名与 `AppConf` 定义。

---

## 阶段总览

| 阶段 | 目标 | 门禁 | 依赖 |
|---|---|---|---|
| **S1-A** | 定稿规格 §3.1 八项 | 八项无待定 | 无 |
| **S1-B** ✅ | 类型层成型，**零签名变更** | `cargo check --workspace --all-targets` 通过；**新增** dto 类型未被命令引用；`From` 无 `..`；契约测试 19/19 全绿 | S1-A |
| **S1-C** ✅ | 签名一次性切换（唯一签名窗口），内部 4 个可编译提交（§6 D2） | 编译通过；`assistant_vault::types::` 指向 dto 类型命中 = 0；校验 A 可被破坏；契约测试全绿；前端无字符串匹配 | S1-B |
| **S2** ✅ | 门禁收敛 + 就地声明 `precondition` + 校验 D | 校验 D 可被破坏；`on_locked` 为唯一锁定入口 | S1-C |
| **S3** ✅ | 配置外置 + 校验 C（前端注册收敛已剔除，§6 D4） | `AppConf` 零 `vault_*` 字段；校验 C 全绿 | S1-C（可与 S2 并行，§6 D6） |
| **S4-0** ✅ | `ts-rs` spike：验证混合变体 / `Option` 载荷 / 枚举产出 | 结论回填规格 §6 D3 附注；三个类型均可用 | S1-C |
| **S4** | `ts-rs` 生成 + CI 漂移检查 | `cargo test export_bindings && git diff --exit-code` 通过 | S4-0 ✅ |

S1 拆三段的原因见规格 §5：设计 DTO 与替换签名压在一个窗口时，字段形态写错要等签名和前端全改完才暴露。S1-A / S1-B 不动运行行为。
S3 剔除前端注册收敛的理由见规格 §6 D4。
**S4-0 已完成**（2026-10-30）：USTC 镜像可拉取 `ts-rs v12.0.1`，三个高风险类型产出全部符合预期，结论已回填规格 §6 D3 附注。S4 全量铺开已无技术阻塞，仅剩生成物路径与 CI 接线工作量。

---

## 阶段 S1-A — 决策定稿

规格 §3.1 八项一旦写进代码，改动就要连带前端，故先锁定。本阶段**只确认，不写代码**。

- [x] S1-A.1 `Readyness` **保留原拼写**（不改 `Readiness`）——已核验 `assistant-sync/src/types/mod.rs` 定义即为 `Readyness`
- [x] S1-A.2 `EntryPreviewDto.category` 声明为 **`String`**，前端 `VaultCategory` 窄类型在 S1-C 一并放开——已核验 Rust `EntryPreview.category: String` 与 `vault.types.ts` 的 `VaultCategory` 确实不一致
- [x] S1-A.3 `RevealResult.kind` **不进生成**；前端类型拆为 `RevealResult & { kind }`——已核验 `dto::RevealResult` 无 `kind`，`kind` 只存在于前端 `RevealedField`
- [x] S1-A.4 `UnreadableEntries` 在 Rust 侧提成具名 `UnreadableEntriesDto`——已核验现为 `SyncResult::UnreadableEntries { ids, reason }` 匿名结构变体
- [x] S1-A.5 HIBP 元组改具名，字段 **`entry_id` / `match_count`**——已核验 `vault_hibp_audit` 返回 `Vec<(String, u32)>`
- [x] S1-A.6 `types` 的 `pub use crate::dto::*` **直接删除**，由编译错误驱动迁移（不试 `#[deprecated]`）——已核验 `types/mod.rs` 第 9 行即该 re-export
- [x] S1-A.7 vault `dto` **一律不加 `skip_serializing_if`**，`Option` 输出 `null`——已核验 `crates/assistant-vault/src` 全目录 `skip_serializing_if` 命中为 0
- [x] S1-A.8 `SyncResult::Error(String)` **折叠为 `Err`**，`SyncResultDto` 去掉该变体，码为 `repoNotInitialized` / `noRemoteConfigured`——已核验 `sync_engine/mod.rs` 恰 6 处（文本仅 `"Git repository not initialized"` / `"No remote configured"` 两句）

> 确认结论：八项全部成立，无待定项，S1-B 可开工。八项各自的源码依据见上，均可现场复核。

---

## 阶段 S1-B — 类型层成型（零签名变更）

命令签名保持返回领域类型，`dto` 只建不用。本阶段结束时 `cargo check` 通过但 `dto` 没有任何调用者，这是预期的。

### S1-B.1 DTO 定义

按文件分组，类型清单见规格 §3.2，此处不重复枚举。

- [x] S1-B.1.1 `dto/error.rs`：`VaultErrorCode` + `VaultErrorDto { code, message, retryable }`，**不设 `details`**；码按 §3.4 命名规则推导
- [x] S1-B.1.2 `dto/sync.rs`：`SyncResultDto`（5 变体，**无 `error`**）/ `SyncConfigDto` / `ReadynessDto` / `ConflictFileDto` / `HistoryCommitDto` / `EntryPreviewDto` / `ConflictPreviewDto` / `ConflictResolutionDto` 及其嵌套载荷
- [x] S1-B.1.3 `dto/{ssh,totp,hibp,recovery}.rs`：`SshKeyInfoDto` / `SshKeyMetaDto` / `TotpResultDto` / `ParsedOtpAuthDto` / `TotpAlgorithmDto` / `HibpAuditEntryDto` / `DisasterRecoveryResultDto`
- [x] S1-B.1.4 `dto/sync.rs`：`SyncActionDto` / `RemoteModeDto`（从 `cmd/vault/sync.rs` 下沉）；同文件另建 `RepositoryStateDto` / `PendingRemoval(s)Dto` / `UnreadableEntriesDto` / `EntryPreviewFieldDto`（`Readyness` / `SyncResult` 载荷所必需）
- [x] S1-B.1.5 `shared/`：`VaultCategory` / `CustomFieldInput` 已在 `shared`，`dto` 与 `types` 同时 re-export；`types` 保留 `pub use crate::shared::*`
- [x] S1-B.1.6 全部 `rename_all = "camelCase"`，不加 `skip_serializing_if`（S1-A.7）——`grep` 全 `dto/` 命中 0
- [x] S1-B.1.7 `dto` 只依赖 `serde` + `shared`，不 `use` 任何 domain 类型——类型定义层成立；`convert.rs` 为独立转换层，不在该约束内

### S1-B.2 `From` 实现

`src-tauri/crates/assistant-vault/src/dto/convert.rs`（新建）

- [x] S1-B.2.1 各 DTO 的 `impl From<Domain> for Dto` 集中在 `convert.rs`（SSH 两类例外，见 S1-B.2.6 / §6 D7）
- [x] S1-B.2.2 **全部穷尽解构，禁止 `..`**（规格 §3.2）：domain 新增字段必须编译失败——`grep '\.\.' dto/` 命中 0
- [x] S1-B.2.3 `impl From<VaultError> for VaultErrorDto` + `impl From<SyncError> for VaultErrorDto`
- [x] S1-B.2.4 `retryable` 按 `code` 判定：`rateLimited` / `gitUnavailable` → `true`，其余 `false`；不按 `message` 猜——由 `VaultErrorCode::is_retryable()` 单点决定
- [x] S1-B.2.5 `message` 脱敏：不拼接 `Crypto(String)` / `InvalidKeyFormat(String)` / `Database(String)` 的上游自由文本（规格 §3.5 ④）
- [x] S1-B.2.6 **不加** `assistant-security` 依赖边（§6 D7）：SSH 两类转换走 `cmd/vault/ssh.rs` 自由函数，不进 `convert.rs`；`impl From` 在此处三者皆外部、孤儿规则拒绝编译
- [x] S1-B.2.7 明确排除：`ImportSummary` / `ImportReport` / `SyncError` 本身不进 `dto`

### S1-B.3 契约测试

`src-tauri/crates/assistant-vault/tests/dto_contract.rs`（新建）

沿用 `src-tauri/tests/protocol_contract.rs` 的断言形态，不引入 `insta`（仓库内不存在，`.snap` 为 0）。

- [x] S1-B.3.1 **1:1 收编等价断言**：`SyncConfigDto` / `TotpResultDto` / `ReadynessDto` / `EntryPreviewDto`，断言 `to_value(domain) == to_value(Dto::from(domain))`。一条覆盖字段名、`Option` 形态、决策 4 的零破坏、`From` 漏字段
- [x] S1-B.3.2 **整体下沉字面量冻结**：`SyncActionDto` / `RemoteModeDto` / `DisasterRecoveryResultDto`，另加 `SshKeyInfoDto` / `SshKeyMetaDto`（§6 D7 使其同样无 `From` 对手方）。**这类无 domain 对手方，等价断言会恒真**，字面量是唯一手段
- [x] S1-B.3.3 **故意变更 wire**：`HibpAuditEntryDto` 断言 `{"entryId": …, "matchCount": …}`；`SyncResultDto` 5 个变体各一（`error` 变体不存在，另由 `try_from` 断言 `Err`）
- [x] S1-B.3.4 **码全量**：`VaultErrorCode` 逐个断言字符串，含新增 `repoNotInitialized` / `noRemoteConfigured`。`VaultError` 无 `Serialize` 做不了等价断言；§3.4 已删码表，此处即唯一权威清单
- [x] S1-B.3.5 **枚举按载荷形态测**：裸变体 / newtype / struct 变体 / `Option` 载荷各一 fixture，不按变体逐个铺开；`Readyness` 混合形态必须两种各测一次（`"noSync"` 与 `{"repoState":"clean"}`），前端靠 `typeof === "object"` 分流
- [x] S1-B.3.6 `EntryPreviewDto.category` fixture 放枚举外值 `"unlisted"` 并断言原样输出；fixture 全部为合成值（规格 §3.5 ①）

> `Option` 一律输出 `null`（S1-A.7）由 grep 全量覆盖，不靠测试抽样：`rg 'skip_serializing_if' crates/assistant-vault/src/dto/` = 0。
> 不用 `contains_key` 断言结构体字段：它只查列出的键在不在，新增字段照样绿。
> 测试放 `assistant-vault/tests/`：该 crate 已有 `tests/sync_engine.rs` / `tests/vault_sync_db.rs`，per-crate 集成测试是既有惯例，也免去链接整个 tauri app 的编译代价。

---

## 阶段 S1-C — 签名切换（前后端同批）

前端 `invoke` 的返回类型是手写的，后端改签名而前端不改时 **typecheck 不报错**，只在运行时出错。故本阶段前后端改动必须同批提交，不设独立的前端阶段。

### S1-C.0 提交切分（规格 §6 D2）

S1-C 是一个阶段，但**必须分 4 个各自可编译的提交**落地，顺序固定、不得合并：

| # | 提交 | 覆盖内容 | 提交后须通过 |
|---|---|---|---|
| C-1 | 错误通道 | S1-C.1 全部：`VaultErrorDto` + `map_err(VaultErrorDto::from)` + 折叠 `Ok(false)` + `SyncResult::Error` 改 `Err` + 前端改判 `code` | `cargo check` / `npm run typecheck` |
| C-2 | HIBP 元组结构化 | S1-C.2 全部 | 同上 |
| C-3 | cmd 层本地类型下沉 | `SyncAction` / `RemoteMode` 改由 `dto::` 提供（S1-C.3 中该部分） | 同上 |
| C-4 | 子 crate 类型收编 | 其余命令签名换 Dto + S1-C.4 `types` 退场 + S1-C.5 校验 A | `cargo check` / `npm run typecheck` / `npm run test` |

### S1-C.1 错误通道

- [x] S1-C.1.1 全部 `map_err(|e| e.to_string())` 替换为 `map_err(VaultErrorDto::from)`
- [x] S1-C.1.2 折叠成 `Ok(false)` 的命令改 `Result<(), VaultErrorDto>`（**不引入 `UnlockOutcomeDto`**）：`vault_unlock` / `vault_show_unlock_window` / `vault_unlock_biometric`；`cancelled` 码由 cmd 层构造
- [x] S1-C.1.3 `sync_engine` 的 6 处 `Ok(SyncResult::Error(...))` 改 `Err`，`SyncResultDto` 去掉 `error` 变体（S1-A.8）
- [x] S1-C.1.4 前端 `vault.types.ts` 的 `SyncResult` 删 `| { error: string }`
- [x] S1-C.1.5 前端 `VaultSyncSettings.tsx` 删 `RUST_ERROR_KEY_MAP`，改按 `code` 取文案（`repoNotInitialized` / `noRemoteConfigured`）
- [x] S1-C.1.6 前端 `VaultSyncSettings.tsx` 的 `KEY_MISMATCH_MARKER` 改判 `code === "keyMismatch"`
- [x] S1-C.1.7 前端 `vaultBiometricSlice.ts` 的 `msg.includes(...)` 改判 `code`。**现状已有 bug**：`VaultError::NotFound` 的 Display 是 `"Entry not found: {0}"`，`includes("NotFound")` 永远为假，是死分支；`Crypto` 能命中。改判 `code` 后两条才都生效
- [x] S1-C.1.8 前端 `vaultStore` 的 `unlock` / `useVaultUnlock.ts` 改判 `code`；`vaultStore.test.ts` 用例改断言 `code`

> `VaultEntryHistory.tsx` 的 `OLD_KEY_MARKER` **不改**：它匹配的是 git commit message 正文，不是错误码，无 `code` 可判。

### S1-C.2 元组结构化

- [x] S1-C.2.1 `vault_hibp_audit` 返回改 `Vec<HibpAuditEntryDto>`
- [x] S1-C.2.2 前端 `vaultAuditSlice.ts` 的 `for (const [id, count] of leaks)` 改字段访问
- [x] S1-C.2.3 前端 `vault.service.ts` 的 `hibpAudit` 返回类型 `[string, number][]` → `HibpAuditEntryDto[]`

### S1-C.3 命令签名替换

- [x] S1-C.3.1 `cmd/vault/` 八个文件一批替换（`lifecycle` / `entries` / `reveal` / `audit` / `sync` / `ssh` / `biometric` / `recovery`）
- [x] S1-C.3.2 `vault_sync_last_time`：`Option<i64>` → `Result<Option<i64>, VaultErrorDto>`；读取点暂保持 `AppConf::load()`，来源迁移留 S3
- [x] S1-C.3.3 前端 `vault.service.ts` 全量 `invoke` 返回类型对齐
- [x] S1-C.3.4 前端 `vault.types.ts` 逐字段对齐 dto；`EntryPreview.category` 由 `VaultCategory` 放开为 `string` 并清理依赖窄类型的分支；补 `SyncAction` / `RemoteMode` 类型，消除裸字符串 `"forcePush"`
- [x] S1-C.3.5 `npm run typecheck` + `npm run test` + `npm run lint`

### S1-C.4 `types` 退场

`src-tauri/crates/assistant-vault/src/types/mod.rs` + 调用点

- [x] S1-C.4.1 删除 `pub use crate::dto::*;`，`cargo check` 报错驱动逐处迁移
- [x] S1-C.4.2 cmd 与测试侧引用按性质分流（**只有 dto 类型改 `dto::`，其余保持原路径**）：

| 类型 | 性质 | 动作 |
|---|---|---|
| `VaultEntrySummary` / `VaultEntryCreate` / `VaultEntryDetail` / `VaultEntryUpdate` / `RevealResult` | dto | 改 `dto::` |
| `VaultCategory` / `CustomFieldInput` | shared | 保持 `types::`（`types` 保留对 `shared` 的 re-export） |
| `VaultEntryCsvRow` / `VaultEntryExport` / `VaultEntrySnapshot` / `VaultEntryImport` / `ImportOutcome` | domain | 保留 `types::` |
| `VaultError` | crypto | 保留（`dto` 不导出） |

- [x] S1-C.4.3 验收：`assistant_vault::types::` 中**指向 dto 类型**的命中数 = 0

### S1-C.5 校验 A

- [x] S1-C.5.1 `#[command]` 标注的 `vault_*` ↔ `generate_handler!` 双向一致；正则取函数名 + 切出注册块，不引入 `syn`(注意：考虑可引入)
- [x] S1-C.5.2 自检：临时删掉一条注册项，校验必须失败

---

## 阶段 S2 — 门禁收敛 + 锁定路径唯一

> **规格 §6 D5**：S2.2（`with_unlocked` 全量替换）与 S2.4（`on_locked` 收敛）必须同一 diff 提交。分开发布会短暂存在两套锁定清理路径，正是本阶段要消灭的形态。

### S2.1 `contract` 词汇表

`src-tauri/crates/assistant-vault/src/contract/mod.rs`（新建）+ `lib.rs`

- [x] S2.1.1 `lib.rs` 加 `pub mod contract;`
- [x] S2.1.2 `Precondition` 三值：`None` / `AnyState` / `Unlocked`（语义见规格 §3.3）；**不留 `Initialized` / `BiometricEnrolled` 死变体**
- [x] S2.1.3 **不建** `Effect` 枚举 / `commands.rs` / `VAULT_COMMANDS` 全表（规格 §3.3：校验证明不了声明完备性）

### S2.2 门禁高阶函数

`src-tauri/src/cmd/vault/mod.rs`

- [x] S2.2.1 `with_unlocked<T>(app, f) -> Result<T, VaultErrorDto>`（内部调 `get_conn()`，`Locked` 自动转 DTO）
- [x] S2.2.2 全部命令改用 `with_unlocked`（文件范围同 S1-C.3.1）
- [x] S2.2.3 删除散落的 `vault_state` / `sync_state` / `reveal_guard` / `clipboard_guard` 直调
- [x] S2.2.4 **就地声明 `precondition`** 于命令定义点，与门禁改造同一 diff；不另建清单文件

### S2.3 校验 D：门禁一致性

`src-tauri/crates/assistant-vault/tests/command_gate.rs`（新建，文本扫描）

- [x] S2.3.1 数据源：就地声明的 `precondition` + 命令体文本（按函数边界切分，不引入 `syn`）（注意考虑可引入）
- [x] S2.3.2 `Unlocked` 的命令必须出现 `with_unlocked(`（或显式 `get_conn()` 并处理 `Locked`），且在任何 DB 访问之前
- [x] S2.3.3 `None` 的命令不得出现门禁调用，不得访问 state
- [x] S2.3.4 `AnyState` 的命令必须访问 state
- [x] S2.3.5 **证据白名单不含** `vault_state(` / `sync_state(` / `app.state::<…>`（只 `try_into` 取 state，不校验锁定）
- [x] S2.3.6 自检：临时删掉某条命令的 `with_unlocked(`，测试必须失败

### S2.4 锁定语义单一入口

- [x] S2.4.1 新增 `on_locked(app)`：清剪贴板 + 摘 SSH key + 清 reveal 令牌 + 广播事件
- [x] S2.4.2 `vault_lock` 改调 `on_locked`
- [x] S2.4.3 屏幕锁回调改调 `on_locked`（现位于 `cmd/vault/mod.rs::setup`）
- [x] S2.4.4 `ssh_remove_all_on_lock` 并入 `on_locked`，删除独立调用

---

## 阶段 S3 — 配置外置 + 前端注册收敛

### S3.1 `VaultConf` 外置

- [x] S3.1.1 新增 `VaultConf`（现有 `vault_*` 字段全部迁入，同名字段 + `#[serde(default)]`）—— `crates/assistant-config/src/vault_conf.rs`，13 字段与迁移前一一对应
- [x] S3.1.2 独立 `vault-config.json` + 原子写（复用 `assistant-config` 既有能力）；路径固定 `dirs::config_dir()/APP_IDENTIFIER/`，**禁止落在 vault 仓库目录**（规格 §3.5 ②）—— 复用 `get_conf_dir()`，与 `config.json` 同目录；`atomic_write` 原子落盘
- [x] S3.1.3 `vault_*` 字段从 `AppConf` 移除，`AppConf` 只留 `vault_enabled: bool` —— `vault_enabled` 全仓零消费者（无读取点、无前端开关、无命令分支），未保留；`AppConf` 现为**零 `vault_*` 字段**，严于本项要求
- [x] S3.1.4 `protocol/dto/config.rs` 的对应镜像整块删除
- [x] S3.1.5 前端 `settingsStore.ts` 的 `AppConf` interface 同步（项目规则：设置项增删必须同步接口）—— 新增 `VaultConf` interface + `vaultConf` 状态 + `loadVaultConf()`；`CONF_KEY_MAP` / `CONF_LOAD_KEY_MAP` / `ConfOf` / `ConfUnion` / `GROUP_TO_PAGE` 同步加 `vault`
- [x] S3.1.6 读取点改造：`cmd/vault/mod.rs`、`cmd/vault/sync.rs`、`cmd/vault/mod.rs::on_app_conf_changed` —— 实际覆盖 `mod.rs` / `sync.rs` / `ssh.rs` 三处读取；`on_app_conf_changed` 更名 `on_vault_conf_changed` 并挂到 `apply_vault_side_effects`
- [x] S3.1.7 `vault_sync_last_time` 改读 vault 自身 state（**签名已在 S1-C 改完，此处不动签名**）—— 改读 `VaultConf::load().vault_sync_last_at`（字段随迁入改名 `vault_sync_last_at`），命令签名未动

### S3.2 校验 C：配置键存在性

`src-tauri/crates/assistant-vault/tests/conf_keys.rs`（新建，文本扫描）

- [x] S3.2.1 读取点引用的配置键必须在 `VaultConf` / `AppConf` 中存在 —— 扫描 `src/cmd/vault/*.rs` 的 `.vault_*` **字段访问**，与 `vault_conf.rs` 的 `pub vault_*` 声明取差集，差集非空即失败。**未按规格 §4.2 的 `ReadsConf` / `WritesConf` 声明落地**（与 S2.1.3「不建清单」同一取向：声明本身无法自证完备），改为扫描真实读取点；字段访问与**方法调用**按 `(` 区分，`.vault_file_manager()` 这类方法不计入配置键，因此不需要维护白名单副本
- [x] S3.2.2 自检：临时引用一个不存在的键，校验必须失败 —— `check_reports_an_undeclared_key` 向扫描结果注入 `vault_phantom`，断言差集恰为该键；另有 `check_reports_a_declared_key_without_readers` 覆盖反向（声明存在但无读取点不误报）。另做一次**真实破坏**：在 `ssh_auto_add_on_unlock` 插入 `conf.vault_phantom`，主测试立即失败（`left: ["vault_phantom"]`），回退后恢复 3 passed

### S3.3 前端注册收敛 —— **已剔除**（规格 §6 D4）

本节不再执行。vault 在前端只有单点注册：`src/layouts/MainLayout.tsx` 一处 lazy import、`src/components/AppSidebar.tsx` 一条 `/vault` 项，设置页已走 `definePage` 注册表。为单一消费者建 `defineVaultFeature()` 注册框架与 IPC 契约无关，属纯抽象成本。

唯一保留的动作（并入 S3 收尾，不建注册表）：

- [x] S3.3.5 核对 `vault-unlock` 路由：文档提及但代码无引用，按实际删除或补回 —— 结论：**无 `/vault-unlock` 路由，无需补回**。`VaultUnlock` 是 `src/vault/VaultApp.tsx:111` 按锁定态就地渲染的组件（`vault === null` 时返回），解锁窗口走原生对话框（spec-3c 已取代 webview 方案），路由层永远只挂 `/vault` 一条

---

## 阶段 S4-0 — `ts-rs` spike（规格 §6 D3）✅

`ts-rs` 是本次唯一新增第三方依赖，`Cargo.lock` 内无此 crate，拉取方式（联网 / 内网镜像）须先确认。**先验证再全量铺开。**

拉取确认：USTC sparse 镜像可解析 `ts-rs v12.0.1`（默认启用 `serde-compat`），无需额外配置。

- [x] S4-0.1 引入 `ts-rs`，仅给三个高风险类型加 `#[derive(TS)]`：`ReadynessDto`（混合变体）、`PendingRemovalEntryDto`（带 `Option` 载荷）、`RepositoryStateDto`
- [x] S4-0.2 跑 `cargo test -p assistant-vault --lib export_bindings`，产出写入 `crates/assistant-vault/bindings/`；人工比对三项全部通过：`ReadynessDto` 为 externally tagged（`"noSync" | { "repoState": RepositoryStateDto } | "dirty" | "ready"`，`typeof === "object"` 分流成立）、`Option` 输出 `string | null` 而非可选键、枚举为 serde `camelCase` 形态（`rebaseInteractive` 而非 `as_str()` 的 `rebase-interactive`）
- [x] S4-0.3 结论回填规格 §6 D3 附注：三个类型均可用，无类型需退回手写；`assistant-vault` 全量测试 66 passed，`cargo fmt --check` 0 diff

## 阶段 S4 — 生成 + CI

- [ ] S4.1 全部 dto 加 `#[derive(TS)]` + `#[ts(export)]`（`#[ts(export_to = "../../src/vault/core/vault.contract.gen.ts")]` 或保留 `bindings/` 后合并）
- [ ] S4.2 `cargo test export_bindings` 生成 `src/vault/core/vault.contract.gen.ts`
- [ ] S4.3 手写文件改为 `import` 生成物；生成物头部加「禁止手改」标记
- [ ] S4.4 CI 加 `cargo test export_bindings && git diff --exit-code`
- [ ] S4.5 `npm run typecheck` + `npm run test` + `npm run lint`

---

## 全局验收

- [x] V1：仓库内无中央命令清单（无 `VAULT_COMMANDS` / `commands.rs`），命令名与类型不落盘【S1-C–S2】—— 全仓 grep `VAULT_COMMANDS` / `pub mod commands` 命中 0
- [x] V2：`grep 'assistant_vault::types::'` 中指向 dto 类型的引用命中 = 0（豁免清单 = S1-C.4.2）【S1-C】—— 14 处命中全为豁免类型：`VaultCategory`（shared）、`VaultError`（crypto）、`VaultEntryCsvRow` / `VaultEntryExport` / `VaultEntrySnapshot` / `VaultEntryImport` / `ImportOutcome`（domain）、`CustomFieldInput`（shared）
- [x] V3：无 `Result<_, String>` 残留（`rg -U --multiline-dotall '->\s*Result<[\s\S]*?,\s*String>'` 于 `cmd/vault/` 为 0；不得用 `grep 'String>'`，会误命中 `Option<String>` / `Vec<String>`）【S1-C】—— 多行正则命中 0
- [x] V4：无 `Vec<(String, u32)>` 类元组返回【S1-C】—— `vault_hibp_audit` 返回 `Result<Vec<HibpAuditEntryDto>, VaultErrorDto>`；`audit.rs:201` 的 `Vec<(String, u32)>` 是 HIBP 并发流的**中间变量**，末段 `.map(...)` 转具名结构，不构成返回
- [x] V5：无 `Ok(false)` 折叠，且无 `Ok(SyncResult::Error(...))`（失败一律走 `Err`）【S1-C】—— `cmd/vault/` 两模式命中 0
- [x] V6：`VaultErrorDto` 无 `details` 字段；`retryable` 只由 `code` 决定【S1-B】—— `dto/error.rs` 仅 `code` / `message` / `retryable` 三字段，`retryable` 由 `code.is_retryable()` 计算
- [x] V7：契约测试按四类落位（等价断言 / 下沉字面量 / 故意变更 / 码全量），fixture 全为合成值【S1-B】—— `dto_contract.rs` 24 条：`*_is_one_to_one`（等价）、`*_wire_is_frozen` / `vault_error_code_strings_are_frozen`（字面量下沉）、`entry_preview_category_passes_unknown_value_through` / `error_message_does_not_carry_crypto_payload`（故意变更）、`sync_result_dto_has_exactly_five_variants` / `retryable_is_derived_from_code_only`（全量）
- [x] V8：`From` 实现无 `..`（`rg '\.\.[[:space:]]*\}'` 于 `dto/convert.rs` 命中 = 0；不得用 `grep '..'`，那是「任意两字符」）【S1-B】—— 实际路径为 `crates/assistant-vault/src/dto/convert.rs`，多行正则命中 0
- [x] V9：前端无 `message.includes(` 式错误判定；未知 `code` 走 `default` 不抛异常【S1-C】—— `src/vault/**` 的 `includes(` 仅剩标签去重、`navigator.platform` 判断、`VAULT_ERROR_CODES.includes` / `VAULT_CATEGORIES.includes`（枚举成员校验，非错误文案匹配）；错误判定全走 `src/vault/core/vaultError.ts` 的 `code` 分支
- [x] V10：校验 A / C / D 均全绿，且每条都有「故意破坏则失败」的用例【S1-C / S2 / S3】
- [x] V11：门禁证据不含 `vault_state(` / `sync_state(` / `app.state::<…>`；`on_locked` 是唯一锁定清理入口【S2】
- [x] V12：`AppConf` 无 `vault_*` 字段（仅 `vault_enabled`）；`vault-config.json` 不在 vault 仓库目录【S3】—— `vault_enabled` 零消费者未保留，实际为 0 个字段；`vault-config.json` 落 `dirs::config_dir()/APP_IDENTIFIER/`
- [x] V13：仓库目录内只有 `.vault-meta` 与 `entries/*.enc`（规格 §3.5 ② 白名单）【S3】—— `vault_file` 仅写这两类；`sync_engine` 导出路径同白名单；`.git` 属仓库自身，不计入
- [x] V14：已剔除——原「宿主三处硬编码消失」随 S3.3 一并取消（规格 §6 D4）；`vault-unlock` 路由核对已完成（无该路由）【S3】
- [ ] V15：`cargo test export_bindings && git diff --exit-code` 通过，且 S4-0 结论已回填规格 §6 D3 附注【S4】⏸ 随 S4 暂缓
- [x] V16：`cargo fmt --check` + `cargo clippy` + `npm run lint` + `npm run typecheck` + `npm run test` 全绿【各阶段】—— 2026-10-30 实测：`cargo fmt --check` 0 diff；`cargo clippy -p assistant-vault -p assistant-config --all-targets` 仅既有测试文件 `unused` 警告（非本次新增）；`cargo check --all-targets` 通过；`cargo test -p assistant-config` 64 passed；`npm run lint` / `npm run typecheck` 零错误；`npm run test` 184 passed（10 files）。**S4 暂缓，本条按 S1–S3 已交付范围验收**

---

## 禁止事项

- 不建前端注册表框架（`defineVaultFeature()` 已剔除，规格 §6 D4）
- 不建 `Effect` 枚举、不写 `effects` 声明（规格 §3.3）
- 不建命令清单 / 扫描器 / 过程宏（规格 §2.1）
- 不新增 `DecryptFailed` 变体（折叠进 `internal`，规格 §3.4）
- 不引入 `UnlockOutcomeDto`（失败走 `Err`，规格 §3.4）
- 不把 `EntryFile` / `VaultMetaFile` / `Argon2Params` / `VaultEntryCsvRow` 带进 `dto`（规格 §2.2 判据①）
- 不给 `dto` 加 `serde_json::Value` 或任何「以后可能有用」的袋子字段
- 契约测试 fixture 不用真实数据（规格 §3.5 ①）
- vault `dto` 不加 `skip_serializing_if`（S1-A.7）
- 不用 `contains_key` 断言结构体字段（新增字段照样绿，S1-B.3.9）

---

## S2 前后端对应检查结果（2026-10-30）

检查范围：65 个 vault 命令的注册名、参数名、返回类型、DTO 字段、枚举取值、事件定义与监听方。
检查方式：脚本化差集比对（后端 `pub fn` 定义、`generate_handler!` 注册、前端 `invoke()` 命令名三方取集）+ DTO 字段名 `snake_case → camelCase` 转换后与 `vault.types.ts` 逐字段比对 + 事件常量与 `listen()` / `emit_broadcast()` 调用点比对，并辅以人工逐条核对签名。

### 结论一：命令契约三方完全一致

| 维度 | 数量 | 差集 |
|---|---|---|
| `src-tauri/src/cmd/vault/*.rs` 中 `pub fn vault_*` | 65 | — |
| `src-tauri/src/lib.rs` `generate_handler!` 注册 | 65 | 与后端定义双向差集为空 |
| `src/vault/core/vault.service.ts` 中 `invoke("vault_*")` 唯一命令名 | 65 | 与后端定义双向差集为空 |

参数名核对依赖 Tauri 的 `camelCase → snake_case` 自动映射，以下易错点已逐条确认：

- `vault_change_master_password(current, new)`：前端传 `{ current, new: newPassword }`，与 Rust 形参 `new` 同名，非 `newPassword`，正确
- `vault_copy_field(id, field_name)`：前端传 `{ id, fieldName }`，正确
- `vault_history_list(entry_id, limit)`、`vault_disaster_recovery_export(git_url, recovery_key, export_path, commit)`：前端显式传 `null` 补位，Rust `Option` 收 `None`
- `vault_git_init(url: Option<String>)` 同时承载 init / clone：`gitInit()` 不传 `url`、`gitClone(url)` 传值，缺失字段自动落 `None`
- 枚举入参 `vault_sync(action)` / `vault_git_remote(mode)` / `vault_resolve_conflict(resolution)` 的字符串取值与后端 `#[serde(rename_all = "camelCase")]` 输出一致

### 结论二：DTO 字段与枚举零错位

`src-tauri/crates/assistant-vault/src/dto/*.rs` 与 `src/shared/*.rs` 共 25 个 `pub struct`，字段经 `snake_case → camelCase` 转换后与 `src/vault/core/vault.types.ts` 对应 `interface` 逐字段比对：**无 `RS_ONLY`、无 `TS_ONLY`、无 `NO_TS_MATCH`**。

枚举全量核对（变体数 / 拼写 / 顺序）：

| 枚举 | 后端 | 前端 | 结果 |
|---|---|---|---|
| `VaultErrorCode` | 24 | 24 | 一致 |
| `VaultCategory` | 6 | 6 | 一致 |
| `RepositoryState` | 13 | 13 | 一致 |
| `SyncResult` | 5 | 5 | 一致 |
| `SyncAction` / `RemoteMode` / `ConflictResolution` / `UnreadableReason` | 4 / 2 / 4 / 2 | 同 | 一致 |

补充确认：

- `ReadynessDto::RepoState(RepositoryStateDto)` 的外部标记序列化输出 `{"repoState":"clean"}`，与前端 `{ repoState: RepositoryState }` 形态一致
- 所有 DTO 的 `Option` 字段均无 `skip_serializing_if`，输出 `null`，与前端 `T | null` 匹配
- 混合变体 `SyncResult` 的 `Conflict` / `PendingRemovals` / `UnreadableEntries` 三个对象形态键名与前端 `in` 判定所用键名完全一致

### 结论三：事件层存在 3 处遗留契约（不影响命令调用，属空转）

**A. 后端广播、前端无消费者（2 个事件）**

- `vault:reveal`、`vault:reveal-rate-limited` 在 `src-tauri/src/cmd/vault/reveal.rs` 共 4 处广播（password / notes 各一正常分支 + 一限流分支），但全仓 `listen()` 无对应监听
- I9 规格第 13 条要求「前端可监听显示 toast」，当前未实现

**B. 前端监听、后端无广播点（1 个事件）**

- `src/vault/core/useVaultEvents.ts` 监听 `vault:meta-integrity-warning`；`event_name::VAULT_META_INTEGRITY_WARNING` 与 `VaultMetaIntegrityWarningPayload` 仍定义在 `src-tauri/src/protocol/events.rs`
- 全仓 `emit_broadcast` 调用点无一使用该事件：原触发点 `vault_unlock_with_recovery_key` 已随 v1.5 语义淘汰（见 `docs/achievement2/I9-vault-tasklist-p2.md` J3.4 备注），广播点消失后常量、payload、前端监听成为遗留三件套

处置待定，三个候选（进入 S3 前择一）：补前端 reveal 监听 + 删除 meta-integrity 三件套；或三处一并删除（含 reveal 广播点）；或维持现状仅记录。

### 结论四：其余核对项均通过

- 前端无 `message.includes(` 式错误判定，全部按 `code` 分支（`src/vault/core/vaultError.ts`）；全仓剩余 4 处 `includes(` 为标签数组去重与平台判断，非错误判定，V9 满足
- 事件载荷语义一致：`vault:locked` / `vault:unlocked` 广播 `()`，`vault:reveal` 广播 entry id，`vault:biometric-enroll-step` 广播 `"credential-created"`；前端对应监听均为无参回调，不解析 payload，无形态冲突
- `vaultService` 方法使用率扫描仅报 2 个「疑似未调用」：`historyList` 经链式调用 `src/vault/detail/VaultEntryHistory.tsx` 确认在用；`totpTimeRemaining` 确为死方法（命令 `vault_totp_time_remaining` 已注册，UI 走 store 本地倒计时），非契约缺陷，可择机清理
- 类型宽窄差异（非缺陷，仅记录）：`gitTestConnection` service 声明 `Promise<number>`、slice 声明 `Promise<number | null>`（catch 兜 `null`）；`StrengthResult.warnings` 后端为枚举 `Vec<StrengthWarning>`、前端为 `string[]`（序列化形态一致，仅弱约束）

### 工具链状态

`npm run typecheck`（`tsc --noEmit`）零错误。本次检查为只读核对，未改动源码。

### 本次检查未覆盖

- ~~S3.2 校验 C（配置键存在性）依赖 `VaultConf` 落地，尚未可执行~~ —— S3 已落地并全绿，见下文「S3 前后端对应检查结果」
- S4-0 `ts-rs` spike 未启动，DTO 仍为手写 + 契约测试断言

---

## S3 前后端对应检查结果（2026-10-30）

检查范围：13 个 `vault_*` 字段迁出 `AppConf` 后的残留、配置读写链路、前后端配置组映射、校验 C。
检查方式：`vault_*` 字段访问点全仓 grep + `VaultConf` 声明比对 + 配置组映射逐处核对 + 实际跑测试。

### 结论一：迁移零残留

| 维度 | 结果 |
|---|---|
| `VaultConf` 字段 | 13，与迁移前 `AppConf` 的 13 个 `vault_*` 同名同类型 |
| `AppConf` 残留 `vault_*` | 0（`vault_enabled` 零消费者，未保留） |
| `AppConfDto` 残留 vault 镜像 | 0，改为独立 `VaultConfDto`，`From<VaultConf>` 无 `..` |
| 前端 `AppConf` interface 残留 | 0，独立 `VaultConf` interface 13 字段 |
| 后端读取点 | 15 行 / 16 处（`cmd/vault/mod.rs` / `sync.rs` / `ssh.rs`），全部 `VaultConf::load()` |
| 前端业务读取点 | 3 处（`vaultLifecycleSlice` 自动锁时长、`VaultSyncSettings` SSH 主机、`SshAgentTab` 自动添加） |

迁移兼容：首次加载时 `vault-config.json` 不存在则从同目录 `config.json` 提取 13 字段并落独立文件；老 `config.json` 残留键不清理——`AppConf` 无 `deny_unknown_fields`（加上会破坏上述迁移），因此 `patch_conf("app", { vaultXxx })` 不报错也不生效。已核对 `dispatchConfPatch("app", …)` 全部调用点，只剩 `theme` / `keybindings` / `stayOnTop`，无 vault 键残留。

### 结论二：新增配置组需 6 处映射同步

缺任一处即类型报错或静默失效：

| 位置 | 作用 | 漏改后果 |
|---|---|---|
| `fieldRegistry.ts` `CONF_KEY_MAP` / `CONF_LOAD_KEY_MAP` | `vault` → `vaultConf` / `loadVaultConf` | 设置页读不到、改不动 |
| `settingsDispatcher.ts` `ConfOf` | `vault` → `VaultConf` | 类型落空 |
| `confUtils.ts` `ConfUnion` | 补 `VaultConf` | `toConfRecord` 报 TS2345 |
| `commandAggregation.tsx` `GROUP_TO_PAGE` | `vault: "vault"` | `Record<ConfGroup, string>` 缺键 |
| `settingEvents.ts` `ConfPatched` | `vault` 分支 → `loadVaultConf()` | 别的窗口改配置后本窗口不刷新 |
| `main.tsx` bootstrap | 与另外三组同批 `await` | `vaultLifecycleSlice` 读到 `null`，静默退化为默认 5 分钟 |

设置页侧无需改 `SettingsGroup` / `SettingsSection`：`VAULT_GENERAL_SETTINGS` / `VAULT_SSH_SETTINGS` 的 `group` 由 `"app"` 改 `"vault"` 即复用现有渲染与 patch 链路。

### 结论三：`on_app_conf_changed` 不是冗余回调

原 `on_app_conf_changed` 承担「配置变更 → 进程内全局状态」同步（`set_ssh_persist_hosts` + 清 `SSH_PERSIST_GUESS_CACHE`），不是残留。字段迁出后 `AppConf` 已无 `vault_sync_ssh_hosts`，故更名 `on_vault_conf_changed(prev, conf)`，由 `patch_conf` 的 `"vault"` 分支经 `apply_vault_side_effects` 调用，语义等价。其余 12 个字段按需 `VaultConf::load()`，无常驻内存副本，不需要 hook。

### 结论四：校验 C 的实现与规格偏差

规格 §4.2 数据源写的是「`ReadsConf` / `WritesConf` 声明 + `app_conf.rs`」，实际未落地声明（与 S2.1.3「不建清单」同一取向：声明无法自证完备），改为扫描真实读取点：

- 扫描 `src-tauri/src/cmd/vault/*.rs` 的 `.vault_*` **字段访问**，与 `vault_conf.rs` 的 `pub vault_*` 声明取差集，差集非空即失败
- 字段访问与方法调用按紧随的 `(` 区分，`.vault_file_manager()` 不计入配置键，因此**不维护白名单副本**
- 自检两条：注入 `vault_phantom` 必须被报出；声明存在但无读取点不得误报

已知盲区（记录，不修）：

- 只扫 `cmd/vault/` 目录；`protocol/dto/config.rs` 的 `From<VaultConf>` 逐字段映射不在扫描内，该处键名写错只能靠 review
- 只识别 `.vault_*` 字面量，别名导入 / 重命名不会被发现
- 前端 `vaultConf.vaultXxx` 读取点不在扫描内，由 `tsc` 兜底（字段名写错即编译失败）

### 工具链状态

`cargo fmt --check` / `cargo clippy -p assistant-vault -p assistant-config --all-targets` / `cargo check --all-targets` 通过；`cargo test -p assistant-config --test vault_conf` 10 passed、`cargo test -p assistant-vault --test conf_keys` 3 passed；`npm run typecheck` / `npm run lint` 零错误。
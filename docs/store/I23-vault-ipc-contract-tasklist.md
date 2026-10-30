# I23 — vault IPC 契约收口 任务清单

> 规格：[I23-vault-ipc-contract.md](./I23-vault-ipc-contract.md)
> **本清单只列可勾选项与完成证据。**阶段总览与依赖、执行决策（D1–D11）、各阶段说明、提交切分表、核对记录、禁止事项均在规格 **§6–§9**；阶段编号与规格 §5 一致。
> **本文不记载任何可由命令签名派生的数字**（命令数、各文件条数、DTO 个数、字段数）。需要核对数量时直接读 `cmd/vault/*.rs` 的签名与 `AppConf` 定义。

---

## S1-A — 决策定稿

- [x] S1-A.1 `Readyness` **保留原拼写**（不改 `Readiness`）——已核验 `assistant-sync/src/types/mod.rs` 定义即为 `Readyness`
- [x] S1-A.2 `EntryPreviewDto.category` 声明为 **`String`**，前端 `VaultCategory` 窄类型在 S1-C 一并放开——已核验 Rust `EntryPreview.category: String` 与 `vault.types.ts` 的 `VaultCategory` 确实不一致
- [x] S1-A.3 `RevealResult.kind` **不进生成**；前端类型拆为 `RevealResult & { kind }`——已核验 `dto::RevealResult` 无 `kind`，`kind` 只存在于前端 `RevealedField`
- [x] S1-A.4 `UnreadableEntries` 在 Rust 侧提成具名 `UnreadableEntriesDto`——已核验现为 `SyncResult::UnreadableEntries { ids, reason }` 匿名结构变体
- [x] S1-A.5 HIBP 元组改具名，字段 **`entry_id` / `match_count`**——已核验 `vault_hibp_audit` 返回 `Vec<(String, u32)>`
- [x] S1-A.6 `types` 的 `pub use crate::dto::*` **直接删除**，由编译错误驱动迁移（不试 `#[deprecated]`）——已核验 `types/mod.rs` 第 9 行即该 re-export
- [x] S1-A.7 vault `dto` **一律不加 `skip_serializing_if`**，`Option` 输出 `null`——已核验 `crates/assistant-vault/src` 全目录 `skip_serializing_if` 命中为 0
- [x] S1-A.8 `SyncResult::Error(String)` **折叠为 `Err`**，`SyncResultDto` 去掉该变体，码为 `repoNotInitialized` / `noRemoteConfigured`——已核验 `sync_engine/mod.rs` 恰 6 处（文本仅 `"Git repository not initialized"` / `"No remote configured"` 两句）

---

## S1-B — 类型层成型（零签名变更）

### S1-B.1 DTO 定义

类型清单见规格 §3.2，此处不重复枚举。

- [x] S1-B.1.1 `dto/error.rs`：`VaultErrorCode` + `VaultErrorDto { code, message, retryable }`，**不设 `details`**；码按 §3.4 命名规则推导
- [x] S1-B.1.2 `dto/sync.rs`：`SyncResultDto`（5 变体，**无 `error`**）/ `SyncConfigDto` / `ReadynessDto` / `ConflictFileDto` / `HistoryCommitDto` / `EntryPreviewDto` / `ConflictPreviewDto` / `ConflictResolutionDto` 及其嵌套载荷
- [x] S1-B.1.3 `dto/{ssh,totp,hibp,recovery}.rs`：`SshKeyInfoDto` / `SshKeyMetaDto` / `TotpResultDto` / `ParsedOtpAuthDto` / `TotpAlgorithmDto` / `HibpAuditEntryDto` / `DisasterRecoveryResultDto`
- [x] S1-B.1.4 `dto/sync.rs`：`SyncActionDto` / `RemoteModeDto`（从 `cmd/vault/sync.rs` 下沉）；同文件另建 `RepositoryStateDto` / `PendingRemoval(s)Dto` / `UnreadableEntriesDto` / `EntryPreviewFieldDto`（`Readyness` / `SyncResult` 载荷所必需）
- [x] S1-B.1.5 `shared/`：`VaultCategory` / `CustomFieldInput` 已在 `shared`，`dto` 与 `types` 同时 re-export；`types` 保留 `pub use crate::shared::*`
- [x] S1-B.1.6 全部 `rename_all = "camelCase"`，不加 `skip_serializing_if`（S1-A.7）——`grep` 全 `dto/` 命中 0
- [x] S1-B.1.7 `dto` 只依赖 `serde` + `shared`，不 `use` 任何 domain 类型——类型定义层成立；`convert.rs` 为独立转换层，不在该约束内

### S1-B.2 `From` 实现

- [x] S1-B.2.1 各 DTO 的 `impl From<Domain> for Dto` 集中在 `src-tauri/crates/assistant-vault/src/dto/convert.rs`（SSH 两类例外，见 S1-B.2.6 / 规格 D7）
- [x] S1-B.2.2 **全部穷尽解构，禁止 `..`**（规格 §3.2）：domain 新增字段必须编译失败——`grep '\.\.' dto/` 命中 0
- [x] S1-B.2.3 `impl From<VaultError> for VaultErrorDto` + `impl From<SyncError> for VaultErrorDto`
- [x] S1-B.2.4 `retryable` 按 `code` 判定：`rateLimited` / `gitUnavailable` → `true`，其余 `false`；不按 `message` 猜——由 `VaultErrorCode::is_retryable()` 单点决定
- [x] S1-B.2.5 `message` 脱敏：不拼接 `Crypto(String)` / `InvalidKeyFormat(String)` / `Database(String)` 的上游自由文本（规格 §3.5 ④）
- [x] S1-B.2.6 **不加** `assistant-security` 依赖边（规格 D7）：SSH 两类转换走 `cmd/vault/ssh.rs` 自由函数，不进 `convert.rs`
- [x] S1-B.2.7 明确排除：`ImportSummary` / `ImportReport` / `SyncError` 本身不进 `dto`

### S1-B.3 契约测试

- [x] S1-B.3.1 **1:1 收编等价断言**：`SyncConfigDto` / `TotpResultDto` / `ReadynessDto` / `EntryPreviewDto`，断言 `to_value(domain) == to_value(Dto::from(domain))`
- [x] S1-B.3.2 **整体下沉字面量冻结**：`SyncActionDto` / `RemoteModeDto` / `DisasterRecoveryResultDto`，另加 `SshKeyInfoDto` / `SshKeyMetaDto`（规格 D7 使其同样无 `From` 对手方）
- [x] S1-B.3.3 **故意变更 wire**：`HibpAuditEntryDto` 断言 `{"entryId": …, "matchCount": …}`；`SyncResultDto` 5 个变体各一（`error` 变体不存在，另由 `try_from` 断言 `Err`）
- [x] S1-B.3.4 **码全量**：`VaultErrorCode` 逐个断言字符串，含新增 `repoNotInitialized` / `noRemoteConfigured`；§3.4 已删码表，此处即唯一权威清单
- [x] S1-B.3.5 **枚举按载荷形态测**：裸变体 / newtype / struct 变体 / `Option` 载荷各一 fixture；`Readyness` 混合形态两种各测一次（`"noSync"` 与 `{"repoState":"clean"}`）
- [x] S1-B.3.6 `EntryPreviewDto.category` fixture 放枚举外值 `"unlisted"` 并断言原样输出；fixture 全部为合成值（规格 §3.5 ①）

---

## S1-C — 签名切换（前后端同批）

提交切分与 `types` 引用分流见规格 §7.4。

### S1-C.1 错误通道

- [x] S1-C.1.1 全部 `map_err(|e| e.to_string())` 替换为 `map_err(VaultErrorDto::from)`
- [x] S1-C.1.2 折叠成 `Ok(false)` 的命令改 `Result<(), VaultErrorDto>`（**不引入 `UnlockOutcomeDto`**）：`vault_unlock` / `vault_show_unlock_window` / `vault_unlock_biometric`；`cancelled` 码由 cmd 层构造
- [x] S1-C.1.3 `sync_engine` 的 6 处 `Ok(SyncResult::Error(...))` 改 `Err`，`SyncResultDto` 去掉 `error` 变体（S1-A.8）
- [x] S1-C.1.4 前端 `vault.types.ts` 的 `SyncResult` 删 `| { error: string }`
- [x] S1-C.1.5 前端 `VaultSyncSettings.tsx` 删 `RUST_ERROR_KEY_MAP`，改按 `code` 取文案（`repoNotInitialized` / `noRemoteConfigured`）
- [x] S1-C.1.6 前端 `VaultSyncSettings.tsx` 的 `KEY_MISMATCH_MARKER` 改判 `code === "keyMismatch"`
- [x] S1-C.1.7 前端 `vaultBiometricSlice.ts` 的 `msg.includes(...)` 改判 `code`。**改前已有 bug**：`VaultError::NotFound` 的 Display 是 `"Entry not found: {0}"`，`includes("NotFound")` 永远为假，是死分支
- [x] S1-C.1.8 前端 `vaultStore` 的 `unlock` / `useVaultUnlock.ts` 改判 `code`；`vaultStore.test.ts` 用例改断言 `code`

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

- [x] S1-C.4.1 删除 `src-tauri/crates/assistant-vault/src/types/mod.rs` 的 `pub use crate::dto::*;`，`cargo check` 报错驱动逐处迁移
- [x] S1-C.4.2 cmd 与测试侧引用按性质分流（分流表见规格 §7.4）
- [x] S1-C.4.3 验收：`assistant_vault::types::` 中**指向 dto 类型**的命中数 = 0

### S1-C.5 校验 A

- [x] S1-C.5.1 `#[command]` 标注的 `vault_*` ↔ `generate_handler!` 双向一致；正则取函数名 + 切出注册块，不引入 `syn`
- [x] S1-C.5.2 自检：临时删掉一条注册项，校验必须失败

---

## S2 — 门禁收敛 + 锁定路径唯一

同一 diff 约束见规格 D5。

### S2.1 `contract` 词汇表

- [x] S2.1.1 **不建 `contract` 模块**：`Precondition` 枚举无消费者，改为命令定义点的 `// precondition:` 注释标记（`src-tauri/crates/assistant-vault/src/contract/mod.rs` 已删除，`lib.rs` 无 `pub mod contract;`）
- [x] S2.1.2 `Precondition` 三值：`None` / `AnyState` / `Unlocked`（语义见规格 §3.3），由 `tests/command_gate.rs` 的 `PRECONDITIONS` 白名单强制；**不留 `Initialized` / `BiometricEnrolled` 死变体**
- [x] S2.1.3 **不建** `Effect` 枚举 / `commands.rs` / `VAULT_COMMANDS` 全表（规格 §3.3）

### S2.2 门禁高阶函数

- [x] S2.2.1 `with_unlocked<T>(app, f) -> Result<T, VaultErrorDto>`（内部调 `get_conn()`，`Locked` 自动转 DTO），落 `src-tauri/src/cmd/vault/mod.rs`
- [x] S2.2.2 全部命令改用 `with_unlocked`（文件范围同 S1-C.3.1）
- [x] S2.2.3 删除散落的 `vault_state` / `sync_state` / `reveal_guard` / `clipboard_guard` 直调
- [x] S2.2.4 **就地声明 `precondition`** 于命令定义点，与门禁改造同一 diff；不另建清单文件

### S2.3 校验 D：门禁一致性

- [x] S2.3.1 数据源：就地声明的 `precondition` + 命令体文本（按函数边界切分，不引入 `syn`），落 `src-tauri/crates/assistant-vault/tests/command_gate.rs`
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

## S3 — 配置外置

前端注册收敛已剔除（规格 D4）。

### S3.1 `VaultConf` 外置

- [x] S3.1.1 新增 `VaultConf`（现有 `vault_*` 字段全部迁入，同名字段 + `#[serde(default)]`）—— `crates/assistant-config/src/vault_conf.rs`，13 字段与迁移前一一对应
- [x] S3.1.2 独立 `vault-config.json` + 原子写（复用 `assistant-config` 既有能力）；路径固定 `dirs::config_dir()/APP_IDENTIFIER/`，**禁止落在 vault 仓库目录**（规格 §3.5 ②）—— 复用 `get_conf_dir()`，与 `config.json` 同目录；`atomic_write` 原子落盘
- [x] S3.1.3 `vault_*` 字段从 `AppConf` 移除 —— `vault_enabled` 全仓零消费者，未保留；`AppConf` 现为**零 `vault_*` 字段**，严于原「只留 `vault_enabled`」的要求
- [x] S3.1.4 `protocol/dto/config.rs` 的对应镜像整块删除
- [x] S3.1.5 前端 `settingsStore.ts` 的 `AppConf` interface 同步（项目规则：设置项增删必须同步接口）—— 新增 `VaultConf` interface + `vaultConf` 状态 + `loadVaultConf()`；`CONF_KEY_MAP` / `CONF_LOAD_KEY_MAP` / `ConfOf` / `ConfUnion` / `GROUP_TO_PAGE` 同步加 `vault`
- [x] S3.1.6 读取点改造：实际覆盖 `cmd/vault/mod.rs` / `sync.rs` / `ssh.rs` 三处；`on_app_conf_changed` 更名 `on_vault_conf_changed` 并挂到 `apply_vault_side_effects`
- [x] S3.1.7 `vault_sync_last_time` 改读 `VaultConf::load().vault_sync_last_at`（字段随迁入改名），命令签名未动

### S3.2 校验 C：配置键存在性

- [x] S3.2.1 读取点引用的配置键必须在 `VaultConf` 中存在 —— 扫描 `src/cmd/vault/*.rs` 的 `.vault_*` **字段访问**，与 `vault_conf.rs` 的 `pub vault_*` 声明取差集，差集非空即失败。未按规格 §4.2 的 `ReadsConf` / `WritesConf` 声明落地（与 §3.3「不建清单」同一取向），偏差记录见规格 §8.2 结论四
- [x] S3.2.2 自检：临时引用一个不存在的键，校验必须失败 —— `check_reports_an_undeclared_key` 注入 `vault_phantom` 断言差集恰为该键；`check_reports_a_declared_key_without_readers` 覆盖反向。另做真实破坏：在 `ssh_auto_add_on_unlock` 插入 `conf.vault_phantom`，主测试立即失败（`left: ["vault_phantom"]`），回退后恢复

### S3.3 收尾

- [x] S3.3.5 核对 `vault-unlock` 路由 —— 结论：**无 `/vault-unlock` 路由，无需补回**。`VaultUnlock` 是 `src/vault/VaultApp.tsx` 按锁定态就地渲染的组件，解锁窗口走原生对话框，路由层永远只挂 `/vault` 一条

---

## S4-0 — `ts-rs` spike

结论与产出已回填规格 D3 附注。

- [x] S4-0.1 引入 `ts-rs`，仅给三个高风险类型加 `#[derive(TS)]`：`ReadynessDto`（混合变体）、`PendingRemovalEntryDto`（带 `Option` 载荷）、`RepositoryStateDto`
- [x] S4-0.2 跑 `cargo test -p assistant-vault --lib export_bindings`，产出写入 `crates/assistant-vault/bindings/`；三项人工比对全部通过
- [x] S4-0.3 结论回填规格 D3 附注：三个类型均可用，无类型需退回手写；`assistant-vault` 全量测试 66 passed，`cargo fmt --check` 0 diff

---

## S4 — 生成 + 漂移检查

两个阻塞已定案（规格 §6 D8 / D9 / D10），按下列执行。

- [ ] S4.1 `dto/` 与 `shared/` 的全部 wire 类型加 `#[derive(TS)]` + `#[ts(export)]`；输入类 `VaultEntryCreate` / `VaultEntryUpdate` 的每个 `Option` 字段加 `#[ts(optional = nullable)]`，**不动 serde**（规格 D9）
- [ ] S4.1.1 先只加 `optional = nullable` 跑一次导出，确认产出可选键；不是则按 D9 回退方案 A（输入类保留手写）
- [ ] S4.2 `#[ts(export_to = "../../../src/vault/core/contract/")]`，`cargo test -p assistant-vault --lib export_bindings` 产出每类型一文件；删除 S4-0 的 `src-tauri/crates/assistant-vault/bindings/`
- [ ] S4.3 `vault.types.ts` 退化为纯 barrel（同名直出 + `XDto as X` 别名）；`RevealedField` 等仅前端组合类型保留手写；`ParsedOtpAuth.algorithm` / `StrengthResult.warnings` 改引生成物，收紧为 `TotpAlgorithmDto` / `StrengthWarning[]`
- [ ] S4.4 `scripts/check-vault-contract.mjs` + `package.json` 的 `contract:check`：跑 `export_bindings` 后断言 `git status --porcelain -- src/vault/core/contract` 为空
- [ ] S4.5 `eslint.config.ts` 的 `ignores` 与新增 `.prettierignore` 排除生成目录；`npm run typecheck` + `npm run test` + `npm run lint`

---

## S5 — 事件遗留清理（D11）

- [ ] S5.1 删 `src-tauri/src/cmd/vault/reveal.rs` 的 4 处 `emit_broadcast`（password / notes 各一正常分支 + 一限流分支），及 `event_name::VAULT_REVEAL` / `VAULT_REVEAL_RATE_LIMITED` 常量
- [ ] S5.2 删 `event_name::VAULT_META_INTEGRITY_WARNING` 常量、`VaultMetaIntegrityWarningPayload`、`useVaultEvents.ts` 的监听
- [ ] S5.3 全仓剩余 `emit_broadcast` 均有 `listen()` 消费者；`cargo clippy` + `npm run typecheck` 通过

---

## 全局验收

- [x] V1：仓库内无中央命令清单（无 `VAULT_COMMANDS` / `commands.rs`），命令名与类型不落盘【S1-C–S2】—— 全仓 grep `VAULT_COMMANDS` / `pub mod commands` 命中 0
- [x] V2：`grep 'assistant_vault::types::'` 中指向 dto 类型的引用命中 = 0（豁免清单 = S1-C.4.2）【S1-C】—— 14 处命中全为豁免类型：`VaultCategory`（shared）、`VaultError`（crypto）、`VaultEntryCsvRow` / `VaultEntryExport` / `VaultEntrySnapshot` / `VaultEntryImport` / `ImportOutcome`（domain）、`CustomFieldInput`（shared）
- [x] V3：无 `Result<_, String>` 残留（`rg -U --multiline-dotall '->\s*Result<[\s\S]*?,\s*String>'` 于 `cmd/vault/` 为 0；不得用 `grep 'String>'`，会误命中 `Option<String>` / `Vec<String>`）【S1-C】—— 多行正则命中 0
- [x] V4：无 `Vec<(String, u32)>` 类元组返回【S1-C】—— `vault_hibp_audit` 返回 `Result<Vec<HibpAuditEntryDto>, VaultErrorDto>`；`audit.rs` 的 `Vec<(String, u32)>` 是 HIBP 并发流的**中间变量**，末段 `.map(...)` 转具名结构，不构成返回
- [x] V5：无 `Ok(false)` 折叠，且无 `Ok(SyncResult::Error(...))`（失败一律走 `Err`）【S1-C】—— `cmd/vault/` 两模式命中 0
- [x] V6：`VaultErrorDto` 无 `details` 字段；`retryable` 只由 `code` 决定【S1-B】—— `dto/error.rs` 仅 `code` / `message` / `retryable` 三字段，`retryable` 由 `code.is_retryable()` 计算
- [x] V7：契约测试按四类落位（等价断言 / 下沉字面量 / 故意变更 / 码全量），fixture 全为合成值【S1-B】—— `dto_contract.rs` 24 条：`*_is_one_to_one`（等价）、`*_wire_is_frozen` / `vault_error_code_strings_are_frozen`（字面量下沉）、`entry_preview_category_passes_unknown_value_through` / `error_message_does_not_carry_crypto_payload`（故意变更）、`sync_result_dto_has_exactly_five_variants` / `retryable_is_derived_from_code_only`（全量）
- [x] V8：`From` 实现无 `..`（`rg '\.\.[[:space:]]*\}'` 于 `dto/convert.rs` 命中 = 0；不得用 `grep '..'`，那是「任意两字符」）【S1-B】—— 实际路径为 `crates/assistant-vault/src/dto/convert.rs`，多行正则命中 0
- [x] V9：前端无 `message.includes(` 式错误判定；未知 `code` 走 `default` 不抛异常【S1-C】—— `src/vault/**` 的 `includes(` 仅剩标签去重、`navigator.platform` 判断、`VAULT_ERROR_CODES.includes` / `VAULT_CATEGORIES.includes`（枚举成员校验，非错误文案匹配）；错误判定全走 `src/vault/core/vaultError.ts` 的 `code` 分支
- [x] V10：校验 A / C / D 均全绿，且每条都有「故意破坏则失败」的用例【S1-C / S2 / S3】
- [x] V11：门禁证据不含 `vault_state(` / `sync_state(` / `app.state::<…>`；`on_locked` 是唯一锁定清理入口【S2】
- [x] V12：`AppConf` 无 `vault_*` 字段（仅 `vault_enabled`）；`vault-config.json` 不在 vault 仓库目录【S3】—— `vault_enabled` 零消费者未保留，实际为 0 个字段；`vault-config.json` 落 `dirs::config_dir()/APP_IDENTIFIER/`
- [x] V13：仓库目录内只有 `.vault-meta` 与 `entries/*.enc`（规格 §3.5 ② 白名单）【S3】—— `vault_file` 仅写这两类；`sync_engine` 导出路径同白名单；`.git` 属仓库自身，不计入
- [x] V14：已剔除——原「宿主三处硬编码消失」随 S3.3 一并取消（规格 D4）；`vault-unlock` 路由核对已完成（无该路由）【S3】
- [ ] V15：`npm run contract:check` 通过（生成物与 `src/vault/core/contract/` 逐文件一致，含未跟踪新增文件），且 S4-0 结论已回填规格 D3 附注【S4】—— S4-0 结论已回填；漂移检查已定案为 D8，待 S4 落地
- [x] V16：`cargo fmt --check` + `cargo clippy` + `npm run lint` + `npm run typecheck` + `npm run test` 全绿【各阶段】—— 2026-10-30 实测：`cargo fmt --check` 0 diff；`cargo clippy -p assistant-vault -p assistant-config --all-targets` 仅既有测试文件 `unused` 警告（非本次新增）；`cargo check --all-targets` 通过；`cargo test -p assistant-config` 64 passed；`cargo test -p assistant-vault` 66 passed（含 S4-0 后 3 条 `export_bindings`）；`npm run lint` / `npm run typecheck` 零错误；`npm run test` 184 passed（10 files）。**S4 全量生成未开工，本条按 S1–S3 + S4-0 已交付范围验收**
- [ ] V17：`vault.types.ts` 无实体类型定义（仅 barrel 导出与 `RevealedField` 仅前端组合类型）；生成目录无手改痕迹、未被 prettier / eslint 改写【S4】
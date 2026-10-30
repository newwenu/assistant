# Vault IPC 契约边界审查与收口方案

> 审查日期：2026-10-29
> 范围：`src-tauri/crates/assistant-{vault,sync,security,crypto}` 与 `src-tauri/src/cmd/vault/*` 之间的 IPC 边界，以及 `src/vault/**/*.types.ts` 的手写镜像
> 命令事实的唯一来源：`cmd/vault/*.rs` 的 `#[command] pub fn` 签名。**本文不记载任何可由签名派生的数字或类型清单**，需要时现读源码

---

## 1. 问题

### 1.1 什么算跨边界

一个值形态算跨边界，当且仅当它出现在 `#[command]` 函数的**入参或返回类型**中，含容器、枚举变体载荷、字段嵌套。

**入参同样算**：`vault_set_sync_config(config)`、`vault_resolve_conflict(resolution)` 都在边界上。

### 1.2 两类值形态

处置动作不同，故分两类：

| 类别 | 现状 | 后果 |
|---|---|---|
| 值形态 | 子 crate / cmd 层定义的类型直接进签名，序列化格式由各自 serde 决定 | 上游改字段名，前端静默不报错 |
| 错误通道 | 命令统一 `Result<T, String>`；`VaultError` 无 `Serialize`；另有命令把失败折叠成 `Ok(false)` | 前端只能 `message.includes(...)` 判语义；结构化变体在边界处不可逆丢失 |

元组形态单列，因为它是**唯一连结构名都没有**的：Rust 侧改元数或换序，TS 位置解构静默错位，任何类型检查都兜不住。

---

## 2. 归属

### 2.1 DTO 落在 `assistant-vault::dto`

`src-tauri/src/protocol` 依赖 `storage` / `providers` / `config`，不是叶节点。把 DTO 搬进去只是把「子 crate 直出」升级成「更大 crate 的直出」，边界变大而不变清晰。`assistant-vault` 是准叶节点，`impl From<assistant_sync::X> for XxxDto` 写在 crate 内永远满足孤儿规则——**原地收编，不搬迁**。

[I16-protocol-type-misplacement-audit.md](./I16-protocol-type-misplacement-audit.md) §1 主张 wire 类型统一放 `protocol/dto`。该主张对 chat 域成立，对 vault 域不成立，理由同上。两者是两个域的不同结论，不是对同一份文档的让步。只有当 `protocol` 真的断开对 `storage` / `providers` / `config` 的类型依赖后，才重新讨论合并。

同样不做：拆细 crate（只增加 `From` 边界与编译单元）；过程宏收集命令（`generate_handler!` 只接受字面量路径列表）；手写中央命令清单（抄一遍即第二份副本）。

### 2.2 三问判据

任一为「是」即不进 `dto`：

| # | 问题 | 是 → 归属 |
|---|---|---|
| ① | 字段名会出现在 `.vault` 文件 / CSV / DB 列里？ | 持久化层（`types` / `vault_file/types`） |
| ② | 是否真的出现在某个 `#[command]` 的入参或返回类型里（含嵌套）？ | 否 → domain 层 |
| ③ | 承载加密语义（`MasterKey` / `VaultPlaintext` / `SecureString`）？ | 留在 `assistant-crypto` |

三问全否 → 进 `dto`，加 `#[serde(rename_all = "camelCase")]`。

判据① 的典型陷阱：`EntryFile` / `VaultMetaFile` / `Argon2Params` / `VaultEntryCsvRow` 这类落盘格式，字段名与 DTO 高度相似，但生命周期是「落盘兼容」而非「前端契约」，不得顺手带进 `dto`。

判据② 靠**一次性人工对照签名**，不建扫描器、不建清单。偏差由编译器收尾：签名改成 DTO 类型名后，漏改即编译失败；`From` 穷尽解构（§3.2）使 domain 加字段即编译失败。唯一无兜底的是「挑错类型」，多收会在 `cargo check` 时暴露「无人引用」。

---

## 3. 目标态

### 3.1 开工前定死的八项

字段形态与路径改一次，前端就要跟改一次，故必须在写 `dto/*.rs` 之前定稿。

| # | 决策点 | 决策 |
|---|---|---|
| 1 | `Readyness` 拼写 | **保留原拼写**。改名要同步结构体 / serde / TS / 契约测试四处，收益为零 |
| 2 | `EntryPreviewDto.category` | **`String`**（与 Rust 一致）。前端现声明为 `VaultCategory`，属「类型在撒谎」；生成后会报一批错，属预期而非回归 |
| 3 | `RevealResult.kind` | **不进生成**。手写类型拆为「生成的 `RevealResult`」+「前端 `RevealedField = RevealResult & { kind }`」 |
| 4 | `UnreadableEntries` | Rust 侧**提成具名** `UnreadableEntriesDto`，否则 ts-rs 生成不出对应类型 |
| 5 | HIBP 元组字段名 | **`HibpAuditEntryDto { entry_id, match_count }`**。不用 `id` / `count`：前者与条目主键混淆，后者过泛 |
| 6 | `types` 的 re-export | **直接删除** `pub use crate::dto::*`，由编译错误驱动迁移 |
| 7 | `Option` 的 wire 表示 | **不跳过，一律输出 `null`**。禁止在 vault `dto` 加 `skip_serializing_if = "Option::is_none"` |
| 8 | `SyncResult::Error(String)` | **折叠为 `Err`**，`SyncResultDto` 去掉该变体，新增码 `repoNotInitialized` / `noRemoteConfigured` |

第 7 项必须定，因为仓库两种约定并存：`protocol/dto/chat.rs` 用 `skip_serializing_if`（键省略），`assistant-sync` 只有 `#[serde(default)]`（输出 `null`）。而前端 `src/vault/core/vault.types.ts` 已按 `string | null` 书写。若某个 DTO 作者照 chat 的样式加了 skip，前端会收到 `undefined` 而 typecheck 不报错——**类型在撒谎**，只在运行时出错。

第 8 项是 §3.4「失败一律走 `Err`」的同类：`sync_engine` 有 6 处 `Ok(SyncResult::Error(...))`，文本只有两句——`"Git repository not initialized"`（3 处）、`"No remote configured"`（3 处）。前端 [VaultSyncSettings.tsx](src/vault/settings/VaultSyncSettings.tsx) 的 `RUST_ERROR_KEY_MAP` 正是逐字匹配这两句。保留该变体等于在 wire 上留一个自由文本通道，字符串化判定无法根除。码名取 `repoNotInitialized` / `noRemoteConfigured` 而非复用 `gitUnavailable`：后者指 git CLI 调用失败，与「仓库未 init」「无 remote」是不同条件，且前端两种文案不同。

### 3.2 模块落点

```
crates/assistant-vault/src/
├── shared/          VaultCategory / CustomFieldInput（dto 与 domain 的公共底座）
├── dto/             唯一 wire 层，全部 camelCase
│   ├── mod.rs       VaultEntry* / RevealResult
│   ├── audit.rs     StrengthResult / StrengthWarning / AuditLogEntry
│   ├── integrity.rs IntegrityResult
│   ├── sync.rs      SyncResultDto / SyncConfigDto / …
│   ├── ssh.rs       SshKeyInfoDto / SshKeyMetaDto
│   ├── totp.rs      TotpResultDto / ParsedOtpAuthDto
│   ├── recovery.rs  DisasterRecoveryResultDto
│   ├── hibp.rs      HibpAuditEntryDto
│   ├── error.rs     VaultErrorDto + VaultErrorCode
│   └── convert.rs   `From<Domain> for Dto` 集中实现
├── contract/        Precondition 三值（命令清单不落文件）
└── types/           domain + 持久化（不再 re-export dto）
```

约束：

- `dto` 的**类型定义**只依赖 `serde` + `shared`，不引用 domain 类型。
- `From` 实现集中在 `convert.rs`，可引用 sync / security / crypto 的领域类型。
- **`From` 必须穷尽解构 domain 字段**，禁止 `..`。带 `..` 时 domain 新增字段静默不上线（编译通过、前端永远收不到）；穷尽解构则使「加字段」直接编译失败，强制作者决定是否上线。这是 `From` 层唯一能兑现契约收口的写法。

> **SSH 两个 DTO 例外**：`SshKeyInfoDto` / `SshKeyMetaDto` 的源类型 `SshKeyInfo` / `SshKeyMeta` 定义在 `assistant-security`，转换**不进 `convert.rs`**，改为在 `cmd/vault/ssh.rs` 用自由函数实现（§6 D7）。`assistant-vault` 因此**不新增对 `assistant-security` 的依赖边**。

### 3.3 命令侧：只声明 `precondition`

| 事实 | 来源 | 手写 |
|---|---|---|
| `name` / `request` / `response` / `blocking` / `error` | 签名 + 命令体 | 否 |
| `precondition` | 人的判断 | 写在命令定义点 |

`precondition` 三值：`None`（纯函数或只读配置）、`AnyState`（需 state 但不碰密文）、`Unlocked`（碰明文 / 密文）。

**不声明 `effects`**。校验只能证明「声明与命令体一致」，证明不了声明完备——漏标不报错，误标无人发现，成本全在维护一份自述表上。真正约束锁定语义的是 `with_unlocked()` 单一入口（S2）。

### 3.4 错误契约

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultErrorDto {
    pub code: VaultErrorCode,
    pub message: String,   // 仅展示；禁止拼接敏感值（§3.5 ④）
    pub retryable: bool,
}
```

不设 `details`：前端不按字段高亮、不按 `retry_after_ms` 倒计时，无消费者。

**码命名规则**：

- `VaultError` 变体 → 同名 camelCase。
- 语义无法一一对应的（`AlreadyUnlocked` / `Database` / `Clipboard` / `Io` / `Other`）→ `internal`。
- `Crypto` **保留独立码 `crypto`**，不并入 `internal`：`vaultBiometricSlice` 据此判定凭据不可用并自动注销；若与 `Database` / `Io` 共码，一次瞬态错误就会误删用户已录入的生物凭据。
- `SyncError` 变体：`Conflict` → `syncConflict`，`GitCli` / `GitStatus` → `gitUnavailable`，`VaultKeyMismatch` → `keyMismatch`，`HistoryDecryptFailed` → `historyDecryptFailed`。
- `cancelled` 无 Rust 变体，由 cmd 层构造（用户取消解锁窗口）。

**两条 wire 承诺**：码只增不删、不改名；前端对未知 `code` 走 `default`，按 `retryable` 与通用文案处理，不得抛异常。

**`retryable`**：`rateLimited` / `gitUnavailable` → `true`，其余 `false`。不得按 `message` 猜。

**失败一律走 `Err`**：现在把失败折叠成 `Ok(false)` 的命令改为返回 `Result<(), VaultErrorDto>`，语义由 `code` 区分（取消 / 密码错误 / 需重新录入）。`sync_engine` 的 6 处 `Ok(SyncResult::Error(String))` 同理折叠为 `Err`，两个码 `repoNotInitialized` / `noRemoteConfigured`（§3.1 第 8 项）。不引入 `UnlockOutcomeDto`——`{ unlocked: bool, reason: Option<_> }` 会造出 `false + None`、`true + Some(_)` 两种无意义组合。

折叠后 `SyncResultDto` 只剩 5 个变体（`success` / `upToDate` / `conflict` / `pendingRemovals` / `unreadableEntries`），与 domain 形态不再 1:1，故按 §4.3 的「故意变更 wire」处理。

### 3.5 敏感面约束

违反即视为安全缺陷。

① **契约测试 fixture 全部为合成值**。禁止真实 URL、本机路径、恢复密钥、主密码、TOTP secret、SSH 指纹——测试代码一旦提交即进入不可撤销的 git 历史。

② **仓库目录文件白名单**：仅 `.vault-meta` 与 `entries/*.enc`。新增文件类型须先改清单。`vault-config.json` 不得落在仓库目录，固定为 `dirs::config_dir()/APP_IDENTIFIER/`。

③ **`.vault-meta` 的 `emergency` 字段只存加密材料**（token + salt），不得存明文恢复密钥。

④ **错误与日志不得携带敏感值**。`message` 禁止拼接 `Crypto(String)` / `InvalidKeyFormat(String)` / `Database(String)` 的上游自由文本；这些 payload 只进日志且需脱敏。前端展示一律用 `code` 映射文案。

---

## 4. 校验

### 4.1 门禁判据

`vault_state` / `sync_state` 只做 `app.state::<T>().try_into()`，不校验锁定；真正的锁检查在 `SecurityDbState::get_conn()` 返回 `VaultError::Locked`，cmd 层看不见。所以「命令体出现 `vault_state(` 即视为已验门禁」是错误判据——任何碰一下 state 的命令都会通过，该测试不可能失败。

**真实判据**：`precondition: Unlocked` 的命令必须出现 `with_unlocked(`（或显式 `get_conn()` 并处理 `Locked`），且在任何 DB 访问之前；`precondition: None` 的命令不得出现门禁调用。`app.state::<…>` 一律不计为门禁证据。

在 S2 落地 `with_unlocked()` 之前**不设门禁校验**。宁可没有，也不要一条永远绿的测试。

### 4.2 机械校验

| # | 校验 | 数据源 | 生效 |
|---|---|---|---|
| A | `#[command]` 标注的 `vault_*` ↔ `generate_handler!` 双向一致 | 正则取函数名 + 切出注册块，不引入 `syn` | S1-C |
| C | `cmd/vault/` 读取点引用的 `vault_*` 键存在于 `VaultConf` | 读取点文本扫描 + `vault_conf.rs` 字段声明（不建 `ReadsConf` / `WritesConf` 声明，理由同 §3.3） | S3 |
| D | 门禁一致性（§4.1） | 声明 + 命令体 | S2 |

A 拦的是编译器发现不了的错：加了命令忘注册、注册了不存在的命令——`generate_handler!` 里写错路径只在运行时 `invoke` 失败。

C 的数据源不是声明而是真实读取点：扫描 `cmd/vault/*.rs` 的 `.vault_*` 字段访问，与 `vault_conf.rs` 的 `pub vault_*` 声明取差集。字段访问与方法调用按紧随的 `(` 区分（`.vault_file_manager()` 是方法，不是配置键），因此不需要维护白名单副本。已知盲区：`protocol/dto/config.rs` 的 `From<VaultConf>` 映射、前端 `vaultConf.vaultXxx` 读取点均不在扫描内，前者靠 review、后者靠 `tsc`。

**每条校验必须有「故意破坏则失败」的用例**，否则无法证明它可失败。

### 4.3 契约测试与生成

测试放在 `crates/assistant-vault/tests/dto_contract.rs`，沿用 `src-tauri/tests/protocol_contract.rs` 已有的四种断言形态，不引入新依赖（`insta` 仓库内不存在，`.snap` 文件为 0）。断言按「有无对手方」分四类：

| 类别 | 适用类型 | 断言形态 | 为什么只能这样 |
|---|---|---|---|
| 1:1 收编 | `SyncConfigDto` / `SshKeyInfoDto` / `TotpResultDto` 等 domain 仍在 sync / security crate 且形态不变的类型 | `assert_eq!(to_value(domain), to_value(Dto::from(domain)))` | 有对手方，自维护、零字面量；这是唯一能证明「收编未改 wire」的写法 |
| 整体下沉 | `SyncActionDto` / `RemoteModeDto` / `DisasterRecoveryResultDto`（cmd 层类型搬入 dto，domain 消失） | 一次性 `json!` 字面量冻结搬迁前形态 | **无对手方**，等价断言会退化成自己跟自己比、恒真；字面量在此是唯一手段，也正是它能抓搬迁时写错的 serde 属性 |
| 故意变更 wire | `HibpAuditEntryDto`（元组 → 对象）、`SyncResultDto`（去掉 `error` 变体） | 断言新形态：`{"entryId":…,"matchCount":…}`、5 个变体各一 | 与 domain 形态不同，等价断言不成立；**这类必须有断言**，否则变更点无人看守 |
| 无 `Serialize` | `VaultErrorCode`（`VaultError` 不实现 `Serialize`） | 全量码字符串逐个断言 | 做不了等价断言；§3.4 已删除码表，此处即唯一权威清单，兑现「只增不删」 |

**不使用 `contains_key` 断言结构体字段。** 它只检查列出来的键在不在，DTO 新增字段照样绿——而新增字段正是要拦的漂移形态。项目里 `contains_key` 只用于命名约定检查（有 `titleSource` / 无 `title_source`），vault 侧这部分由 §3.2 的 `rename_all = "camelCase"` 与 §3.4 的统一约定保证，不再重复断言。

**枚举按载荷形态测，不按变体逐个铺开。** 裸变体、newtype、struct 变体、`Option` 载荷各一个 fixture。混合形态（`Readyness` 同时有裸变体与 `RepoState(_)` newtype）必须两种各测一次——前端正是靠 `typeof === "object"` 分流，这是全 vault 域最容易写错的一处。

`EntryPreviewDto.category` 的 fixture 放一个枚举外的值（如 `"unlisted"`）并断言原样输出。写成「断言为任意字符串」不可执行；字段若退回 `VaultCategory`，这行会直接编译失败。

**`Option` 一律输出 `null`**（§3.1 第 7 项）由 grep 全量覆盖，不靠测试抽样：`rg 'skip_serializing_if' crates/assistant-vault/src/dto/` = 0。

`RepositoryState` 的 wire 形态只用 serde（`rebaseInteractive`），`as_str()` 的 `rebase-interactive` 不外发。等价断言两边同走 serde，天然一致。

生成：`ts-rs` 产出 `vault.contract.gen.ts`，CI 跑 `cargo test export_bindings && git diff --exit-code`。前置条件三条：值形态与错误通道收编完成、手写类型拆分完成（`kind` 不进生成）、窄类型问题已排期。

`ts-rs` 是本次唯一新增第三方依赖，`Cargo.lock` 内无此 crate，拉取方式（联网 / 内网镜像）须在开工前确认。混合变体（`Readyness`）与 `Option` 载荷的产出形态不能按文档推断，必须先由 S4-0 spike 用真实输出验证（§6 D3）。

---

## 5. 落地顺序

| 阶段 | 动作 | 验收 |
|---|---|---|
| S1-A | 定稿 §3.1 八项 | 八项无待定 |
| S1-B | 类型层成型，零签名变更：写 `dto/*.rs` + `convert.rs` + `dto_contract.rs`，命令仍返回领域类型 | `cargo check` 通过；`dto` 未被命令引用；`From` 无 `..`；契约测试全绿 |
| S1-C | 签名一次性切换：全部命令改 Dto + `map_err(VaultErrorDto::from)` + 折叠 `Ok(false)` / `SyncResult::Error` 改 `Err` + 删 `types` re-export + 校验 A + 前端同批改判 `code` | 编译通过；`assistant_vault::types::` 指向 dto 类型命中 = 0；校验 A 可被破坏；契约测试全绿；前端无字符串匹配 |
| S2 | 门禁收敛为 `with_unlocked` + `on_locked`；就地声明 `precondition`；启用校验 D | 校验 D 可被破坏；`on_locked` 为唯一锁定入口 |
| S3 | `vault_*` 配置外置为 `VaultConf`；启用校验 C。**前端注册收敛（原 S3.3）已剔除**（§6 D4） | `AppConf` 零 `vault_*` 字段（`vault_enabled` 无消费者未保留）；校验 C 全绿 |
| S4-0 | `ts-rs` spike：仅导出 `Readyness` / 带 `Option` 载荷的 DTO / `RepositoryState`，人工比对真实产出 | 结论回填 §6 D3 附注；不可用类型退回手写 |
| S4 | `ts-rs` 生成 + CI 漂移检查 | `git diff` 为空 |

S1 拆三段是因为「设计 DTO」与「替换签名」压在一个窗口时，字段形态写错要等签名和前端全改完才暴露。S1-A / S1-B 不动运行行为。

S1-C 是唯一签名窗口：DTO 收编、错误 DTO、折叠修复都要改签名，一次做完。S3 只迁配置读取点，不再改签名。

S1-C 内部顺序：错误通道 → HIBP 元组结构化 → cmd 层本地类型下沉 → 子 crate 类型（可分批）。该顺序即 §6 D2 的四个可编译提交，不得合并；S4-0 结论回填后 S4 才开工。

---

## 6. 执行决策

§1–§5 定义「做什么」，本节定义「怎么落地」。两者冲突时以本节为准。

| # | 决策 | 理由 |
|---|---|---|
| D1 | 阶段顺序 **S1-A → S1-B → S1-C → S2 → S3 → S4-0 → S4** 不变 | S1-C 是唯一签名窗口；S4 的生成依赖 S1-C 的值形态与错误通道收编完成 |
| D2 | S1-C 拆成 **4 个各自可编译的提交**，顺序固定：错误通道 → HIBP 元组结构化 → cmd 层本地类型下沉 → 子 crate 类型 | 后端签名与前端手写镜像必须同批，但一次性大提交中途不可编译即无法定位失败点；按此顺序每段都能独立通过 `cargo check` + `npm run typecheck` |
| D3 | S4 前置 **S4-0 `ts-rs` spike**，只导出 `Readyness`（混合变体）/ 一个带 `Option` 载荷的 DTO / `RepositoryState`，人工比对产出 | `ts-rs` 是本次唯一新增第三方依赖，`Cargo.lock` 内无此 crate；混合变体能否用 `typeof === "object"` 分流、`Option` 是否输出 `\| null` 必须看真实产出，不能按文档推断 |
| D4 | **剔除**原 S3.3 前端 `defineVaultFeature()` 注册收敛 | vault 在前端只有单点注册：`src/layouts/MainLayout.tsx` 一处 lazy import、`src/components/AppSidebar.tsx` 一条 `/vault` 项，设置页已走 `definePage` 注册表。为单一消费者建注册表框架与 IPC 契约无关，是纯抽象成本 |
| D5 | S2.2 `with_unlocked` 全量替换与 S2.4 `on_locked` 收敛**同一 diff 提交** | 分开发布会短暂存在两套锁定清理路径，正是 S2 要消灭的形态 |
| D6 | S3 可与 S2 并行，且不阻塞 S4 | S3 只迁配置读取点、不改签名；两者在 `cmd/vault/mod.rs` / `sync.rs` 有文件重叠，并行时按文件划分，冲突则 S3 让位 S2 |
| D7 | SSH 两个 DTO（`SshKeyInfoDto` / `SshKeyMetaDto`）的转换放 **`cmd/vault/ssh.rs`**，用**自由函数**（`fn ssh_key_info_dto(…) -> …`），**不写 `impl From`**；`assistant-vault` **不加** `assistant-security` 依赖边 | ① `assistant-security` 传递依赖 `reqwest` / `arboard` / `ssh-key` / `rsa` / `windows`，vault 业务代码零处调用它，为两条 `From` 让整个 crate 继承这套依赖不划算；② 写在 `cmd` 层则 `impl From<assistant_security::X> for assistant_vault::dto::XDto` 三者皆外部，**孤儿规则拒绝编译**，只能用自由函数或本地 trait；③ cmd 层本就同时看得见两端，穷尽解构的编译期护栏不丢 |

**D3 附注（S4-0 已回填，2026-10-30）**：spike 用 `ts-rs` v12.0.1（`cargo add` 经 USTC 镜像可解析）导出三个真实类型，产出**全部可用**：

| 类型 | 生成产出 | 结论 |
|---|---|---|
| `RepositoryStateDto` | `"clean" \| "merge" \| "rebase" \| "rebaseInteractive" \| …` | 与前端手写 `RepositoryState` 逐字一致；取 serde `camelCase` 形态，**不是** `as_str()` 的 `rebase-interactive`（后者是 git 目录名，非 wire 值） |
| `PendingRemovalEntryDto` | `{ id: string, title: string \| null }` | `Option` 输出 `\| null` 而非可选键 `title?:`，与 §3.1 第 7 项「一律输出 `null`」一致 |
| `ReadynessDto` | `"noSync" \| { "repoState": RepositoryStateDto } \| "dirty" \| "ready"` | 混合变体为 externally tagged：单元变体出字符串、`RepoState(...)` 出单键对象。前端 `typeof x === "object"` 分流**依然成立**，`x.repoState` 取值路径不变 |

附带确认：`#[ts(export)]` 自动生成 `export_bindings_<类型名小写>` 单测，`cargo test -p assistant-vault --lib export_bindings` 即触发写盘；默认输出目录为 crate 根 `bindings/`，可用 `#[ts(export, export_to = "...")]` 改路径。三个类型均未退回手写，**S4 无需保留任何手写 DTO 类型**。

**文档勘误**：§3.2 曾写「收编 `assistant_security` 的类型需给 `assistant-vault` 新增该依赖边」。该表述已被 D7 取代——SSH 两类不走 `convert.rs`，故无依赖边。
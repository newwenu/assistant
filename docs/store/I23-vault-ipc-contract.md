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

生成：`ts-rs` 每类型一文件产出到 `src/vault/core/contract/`（§6 D10），漂移检查为 `npm run contract:check`（§6 D8：跑 `export_bindings` 后断言 `git status --porcelain -- src/vault/core/contract` 为空）。前置条件三条：值形态与错误通道收编完成、手写类型拆分完成（`kind` 不进生成）、窄类型问题已排期。

`ts-rs` 是本次唯一新增第三方依赖。**拉取已确认**：USTC sparse 镜像可解析 `ts-rs v12.0.1`（默认启用 `serde-compat`），`Cargo.lock` 已收录。混合变体（`Readyness`）与 `Option` 载荷的产出形态已由 S4-0 spike 用真实输出验证，结论见 §6 D3 附注。

**S4 开工前的两个新增阻塞**（S4-0 实测后暴露，非原规格预判）——**已全部定案（§6 D8 / D9）**：

1. **仓库无 CI**。`docs/ARCHITECTURE.md` 已记录「CI 目录不存在」，`.github/workflows` 为空。`git diff --exit-code` 无挂载点，须先定漂移检查的执行位置（三条备选见 §7.6，已定案 D8）。
2. **输入类 DTO 的可选键与生成物语义冲突**。`VaultEntryCreate` / `VaultEntryUpdate` 的 `Option<String>` 在 serde 默认形态下输出 `null`，ts-rs 生成必填键 `username: string | null`；前端手写为可选键 `username?: string`。§3.1 第 7 项「一律输出 `null`」只约束输出方向，输入方向未定。直接全量替换会让所有构造 entry 的调用点被迫显式写 `null`。三条备选取舍见 §7.6，已定案 D9（新增方案 D）。

---

## 5. 落地顺序

| 阶段 | 动作 | 验收 |
|---|---|---|
| S1-A | 定稿 §3.1 八项 | 八项无待定 |
| S1-B | 类型层成型，零签名变更：写 `dto/*.rs` + `convert.rs` + `dto_contract.rs`，命令仍返回领域类型 | `cargo check` 通过；`dto` 未被命令引用；`From` 无 `..`；契约测试全绿 |
| S1-C | 签名一次性切换：全部命令改 Dto + `map_err(VaultErrorDto::from)` + 折叠 `Ok(false)` / `SyncResult::Error` 改 `Err` + 删 `types` re-export + 校验 A + 前端同批改判 `code` | 编译通过；`assistant_vault::types::` 指向 dto 类型命中 = 0；校验 A 可被破坏；契约测试全绿；前端无字符串匹配 |
| S2 | 门禁收敛为 `with_unlocked` + `on_locked`；就地声明 `precondition`；启用校验 D | 校验 D 可被破坏；`on_locked` 为唯一锁定入口 |
| S3 | `vault_*` 配置外置为 `VaultConf`；启用校验 C。**前端注册收敛（原 S3.3）已剔除**（§6 D4） | `AppConf` 零 `vault_*` 字段（`vault_enabled` 无消费者未保留）；校验 C 全绿 |
| S4-0 | `ts-rs` spike：仅导出 `Readyness` / 带 `Option` 载荷的 DTO / `RepositoryState`，人工比对真实产出 | 结论回填 §6 D3 附注；三个类型均可用 | ✅ 2026-10-30 |
| S4 | `ts-rs` 生成 + 漂移检查 | `npm run contract:check` 通过（生成物与 `src/vault/core/contract/` 逐文件一致） | 待开工，阻塞已定案（§6 D8 / D9 / D10） |
| S5 | 事件遗留清理：删 reveal 广播与 meta-integrity 三件套（§6 D11） | 剩余 `emit_broadcast` 均有 `listen()` 消费者 | 无依赖，可与 S4 并行 |

S1 拆三段是因为「设计 DTO」与「替换签名」压在一个窗口时，字段形态写错要等签名和前端全改完才暴露。S1-A / S1-B 不动运行行为。

S1-C 是唯一签名窗口：DTO 收编、错误 DTO、折叠修复都要改签名，一次做完。S3 只迁配置读取点，不再改签名。

S1-C 内部顺序：错误通道 → HIBP 元组结构化 → cmd 层本地类型下沉 → 子 crate 类型（可分批）。该顺序即 §6 D2 的四个可编译提交，不得合并；S4-0 结论回填后 S4 才开工。

---

## 6. 执行决策

§1–§5 定义「做什么」，本节定义「怎么落地」。两者冲突时以本节为准。

| # | 决策 | 理由 |
|---|---|---|
| D1 | 阶段顺序 **S1-A → S1-B → S1-C → S2 → S3 → S4-0 → S4 → S5** 不变 | S1-C 是唯一签名窗口；S4 的生成依赖 S1-C 的值形态与错误通道收编完成；S5 只删无消费者的 wire 面，与 S4 无依赖，可并行 |
| D2 | S1-C 拆成 **4 个各自可编译的提交**，顺序固定：错误通道 → HIBP 元组结构化 → cmd 层本地类型下沉 → 子 crate 类型 | 后端签名与前端手写镜像必须同批，但一次性大提交中途不可编译即无法定位失败点；按此顺序每段都能独立通过 `cargo check` + `npm run typecheck` |
| D3 | S4 前置 **S4-0 `ts-rs` spike**，只导出 `Readyness`（混合变体）/ 一个带 `Option` 载荷的 DTO / `RepositoryState`，人工比对产出 | `ts-rs` 是本次唯一新增第三方依赖，`Cargo.lock` 内无此 crate；混合变体能否用 `typeof === "object"` 分流、`Option` 是否输出 `\| null` 必须看真实产出，不能按文档推断 |
| D4 | **剔除**原 S3.3 前端 `defineVaultFeature()` 注册收敛 | vault 在前端只有单点注册：`src/layouts/MainLayout.tsx` 一处 lazy import、`src/components/AppSidebar.tsx` 一条 `/vault` 项，设置页已走 `definePage` 注册表。为单一消费者建注册表框架与 IPC 契约无关，是纯抽象成本 |
| D5 | S2.2 `with_unlocked` 全量替换与 S2.4 `on_locked` 收敛**同一 diff 提交** | 分开发布会短暂存在两套锁定清理路径，正是 S2 要消灭的形态 |
| D6 | S3 可与 S2 并行，且不阻塞 S4 | S3 只迁配置读取点、不改签名；两者在 `cmd/vault/mod.rs` / `sync.rs` 有文件重叠，并行时按文件划分，冲突则 S3 让位 S2 |
| D7 | SSH 两个 DTO（`SshKeyInfoDto` / `SshKeyMetaDto`）的转换放 **`cmd/vault/ssh.rs`**，用**自由函数**（`fn ssh_key_info_dto(…) -> …`），**不写 `impl From`**；`assistant-vault` **不加** `assistant-security` 依赖边 | ① `assistant-security` 传递依赖 `reqwest` / `arboard` / `ssh-key` / `rsa` / `windows`，vault 业务代码零处调用它，为两条 `From` 让整个 crate 继承这套依赖不划算；② 写在 `cmd` 层则 `impl From<assistant_security::X> for assistant_vault::dto::XDto` 三者皆外部，**孤儿规则拒绝编译**，只能用自由函数或本地 trait；③ cmd 层本就同时看得见两端，穷尽解构的编译期护栏不丢 |
| D8 | 阻塞 1 定案：漂移检查挂 **`npm run contract:check`**（`scripts/check-vault-contract.mjs`：先跑 `cargo test -p assistant-vault --lib export_bindings`，再断言 `git status --porcelain -- src/vault/core/contract` 为空） | 沿用 `scripts/update-models-dev-snapshot.mjs --check` 既有「生成 + 比对」模式，零新增基础设施。**不用 `git diff --exit-code`**：ts-rs 每类型一文件，新增 DTO 会产出未跟踪文件，`git diff` 对未跟踪文件恒为空，`git status --porcelain` 才同时覆盖改动 / 新增 / 删除。不挂 `.git/hooks`（不入库，换机即失效），不新增 workflow（项目无 CI 基建，为单条检查引入整套 setup 成本远高于收益；将来若有 CI，直接挂该脚本即可） |
| D9 | 阻塞 2 定案：**方案 D（新增）**——输入类 `VaultEntryCreate` / `VaultEntryUpdate` 的每个 `Option` 字段加 `#[ts(optional = nullable)]`，生成可选键 `username?: string \| null`；**不动 serde**，不保留手写输入类型 | ① 该形态即 wire 真实语义：字段缺失与 `null` 反序列化都落 `None`，生成物不再比实际更严；② 前端 `\|\| undefined` 构造点零改动（`tsconfig` 未开 `exactOptionalPropertyTypes`，`undefined` 可赋给可选键）；③ 不违反 §9「dto 不加 `skip_serializing_if`」，也不触碰 §3.1 第 7 项的输出侧约定；④ 优于 A：A 会让两个**最易加字段**的输入类型永久留在手写侧，与 D3 附注「S4 无需保留任何手写 DTO 类型」自相矛盾。**回退条件**：`optional = nullable` 是 ts-rs 专属属性，v12 是否产出可选键未在本机源码验证——S4.1 先只加该属性跑一次导出，产出不是可选键即退方案 A（输入类保留手写 + `vault.types.ts` 分区注释） |
| D10 | 生成物落位与前端命名：产物落 **`src/vault/core/contract/`**（每类型一文件，`#[ts(export_to = "../../../src/vault/core/contract/")]` 相对 crate 根）；`vault.types.ts` 退化为**纯 barrel**（同名直出 + `XDto as X` 别名），前端调用点零改动；生成目录进 `eslint.config.ts` 的 `ignores`，并新增 `.prettierignore` | ① ts-rs 只能每类型一文件（S4-0 实测），§4.3 与任务清单 S4.2 原写的「单文件 `vault.contract.gen.ts`」作废；② 产物必须落在 `src/` 下才被 `tsconfig` 的 `include` 与 Vite 覆盖，落 crate 内 `bindings/` 会越出 `include`，且 eslint `projectService` 找不到项目；③ 生成物不符合仓库 prettier 风格，不加 ignore 会让 `lint`（`prettier/prettier: error`）与 `npm run format` 反复改写生成物，直接打爆漂移检查；④ 选别名而非改调用点：前端类型名与生成物只差 `Dto` 后缀，映射收敛在 barrel 一处，不必为纯命名差异改遍 vault 前端 |
| D11 | 事件遗留定案：**三处一并删除**（原候选二）——删 `reveal.rs` 的 4 处 `emit_broadcast` 与 `event_name::VAULT_REVEAL` / `VAULT_REVEAL_RATE_LIMITED` 常量；删 `event_name::VAULT_META_INTEGRITY_WARNING` 常量 + `VaultMetaIntegrityWarningPayload` + `useVaultEvents.ts` 监听 | reveal 结果已由命令返回值同步送达、限流已走 `Err(rateLimited)`，vault 单窗口、无跨窗口消费者，广播没有存在理由；meta-integrity 的触发点随 v1.5 淘汰，常量 / payload / 监听三件套无生产者。与 §2.2「wire 面必须有消费者」同一条判据。代价：I9 规格第 13 条「前端可监听显示 toast」被取代——该条只在存档目录 `docs/achievement2/` 内，不回溯修改 |

**D3 附注（S4-0 已回填，2026-10-30）**：spike 用 `ts-rs` v12.0.1（`cargo add` 经 USTC 镜像可解析）导出三个真实类型，产出**全部可用**：

| 类型 | 生成产出 | 结论 |
|---|---|---|
| `RepositoryStateDto` | `"clean" \| "merge" \| "rebase" \| "rebaseInteractive" \| …` | 与前端手写 `RepositoryState` 逐字一致；取 serde `camelCase` 形态，**不是** `as_str()` 的 `rebase-interactive`（后者是 git 目录名，非 wire 值） |
| `PendingRemovalEntryDto` | `{ id: string, title: string \| null }` | `Option` 输出 `\| null` 而非可选键 `title?:`，与 §3.1 第 7 项「一律输出 `null`」一致 |
| `ReadynessDto` | `"noSync" \| { "repoState": RepositoryStateDto } \| "dirty" \| "ready"` | 混合变体为 externally tagged：单元变体出字符串、`RepoState(...)` 出单键对象。前端 `typeof x === "object"` 分流**依然成立**，`x.repoState` 取值路径不变 |

附带确认：`#[ts(export)]` 自动生成 `export_bindings_<类型名小写>` 单测，`cargo test -p assistant-vault --lib export_bindings` 即触发写盘；默认输出目录为 crate 根 `bindings/`，可用 `#[ts(export_to = "...")]` 改路径。三个类型均未退回手写，**S4 无需保留任何手写 DTO 类型**。

spike 导出的是 **dto 层类型**（`ReadynessDto` 等），非领域类型（`Readyness`）。这是正确对象：前端消费的是 wire 形态，领域类型的 `as_str()` 等方法不外发。原规格 D3 行写的 `Readyness` / `RepositoryState` 是简写，实际落点为 dto 同名类型。

**S4 全量开工前的 2 个阻塞已定案（2026-10-30，§6 D8 / D9）**：仓库无 CI 挂载点 → `npm run contract:check`（D8）；输入类 DTO 可选键与生成物冲突 → 新增方案 D（D9）。S4 可开工；若 D9 的 `optional = nullable` 验证失败而退方案 A，则输入两类保留手写，`ts-rs` 依赖仍用于输出侧，不整体回退。

**文档勘误**：§3.2 曾写「收编 `assistant_security` 的类型需给 `assistant-vault` 新增该依赖边」。该表述已被 D7 取代——SSH 两类不走 `convert.rs`，故无依赖边。

---

## 7. 执行备注

> 原任务清单中的说明性内容集中于此。任务清单（[I23-vault-ipc-contract-tasklist.md](./I23-vault-ipc-contract-tasklist.md)）只保留可勾选项与完成证据。

### 7.1 阶段总览与依赖

| 阶段 | 目标 | 门禁 | 依赖 |
|---|---|---|---|
| **S1-A** | 定稿 §3.1 八项 | 八项无待定 | 无 |
| **S1-B** | 类型层成型，**零签名变更** | `cargo check --workspace --all-targets` 通过；**新增** dto 类型未被命令引用；`From` 无 `..`；契约测试 19/19 全绿 | S1-A |
| **S1-C** | 签名一次性切换（唯一签名窗口），内部 4 个可编译提交（D2） | 编译通过；`assistant_vault::types::` 指向 dto 类型命中 = 0；校验 A 可被破坏；契约测试全绿；前端无字符串匹配 | S1-B |
| **S2** | 门禁收敛 + 就地声明 `precondition` + 校验 D | 校验 D 可被破坏；`on_locked` 为唯一锁定入口 | S1-C |
| **S3** | 配置外置 + 校验 C（前端注册收敛已剔除，D4） | `AppConf` 零 `vault_*` 字段；校验 C 全绿 | S1-C（可与 S2 并行，D6） |
| **S4-0** | `ts-rs` spike：验证混合变体 / `Option` 载荷 / 枚举产出 | 结论回填 D3 附注；三个类型均可用 | S1-C |
| **S4** | `ts-rs` 生成 + 漂移检查（D8 / D9 / D10） | `npm run contract:check` 通过 | S4-0 |
| **S5** | 事件遗留清理（D11） | 剩余 `emit_broadcast` 均有 `listen()` 消费者 | 无（可与 S4 并行） |

阶段依赖链：S1-A → S1-B → S1-C → {S2, S3, S4-0} → S4；S5 独立。S1 拆三段的原因见 §5；S3 剔除前端注册收敛的理由见 D4。

**S4-0 完成记录（2026-10-30）**：USTC 镜像可拉取 `ts-rs v12.0.1`，三个高风险类型产出全部符合预期，结论已回填 D3 附注。依赖已引入（`crates/assistant-vault/Cargo.toml` 的 `ts-rs = "12"`），产物落 `crates/assistant-vault/bindings/`（每类型一文件），S4 迁出至 `src/vault/core/contract/`（D10）。S4 全量开工前的 2 个阻塞已定案（§6 D8 / D9）；若 D9 回退条件触发，须回退该依赖与三个 `#[derive(TS)]`，不保留未使用依赖。

### 7.2 S1-A 确认结论

§3.1 八项全部成立，无待定项，S1-B 可开工。各项源码依据逐条附在任务清单 S1-A 的勾选项上，均可现场复核。

### 7.3 S1-B 契约测试的三条取舍

- `Option` 一律输出 `null`（§3.1 第 7 项）由 grep 全量覆盖，不靠测试抽样：`rg 'skip_serializing_if' crates/assistant-vault/src/dto/` = 0
- 不用 `contains_key` 断言结构体字段：它只查列出的键在不在，新增字段照样绿
- 测试放 `assistant-vault/tests/`：该 crate 已有 `tests/sync_engine.rs` / `tests/vault_sync_db.rs`，per-crate 集成测试是既有惯例，也免去链接整个 tauri app 的编译代价

### 7.4 S1-C 提交切分与类型分流

**四个提交**（D2，顺序固定、不得合并）：

| # | 提交 | 覆盖内容 | 提交后须通过 |
|---|---|---|---|
| C-1 | 错误通道 | 任务清单 S1-C.1 全部：`VaultErrorDto` + `map_err(VaultErrorDto::from)` + 折叠 `Ok(false)` + `SyncResult::Error` 改 `Err` + 前端改判 `code` | `cargo check` / `npm run typecheck` |
| C-2 | HIBP 元组结构化 | 任务清单 S1-C.2 全部 | 同上 |
| C-3 | cmd 层本地类型下沉 | `SyncAction` / `RemoteMode` 改由 `dto::` 提供 | 同上 |
| C-4 | 子 crate 类型收编 | 其余命令签名换 Dto + `types` 退场 + 校验 A | `cargo check` / `npm run typecheck` / `npm run test` |

前端 `invoke` 的返回类型是手写的，后端改签名而前端不改时 **typecheck 不报错**，只在运行时出错。故 S1-C 前后端改动必须同批提交，不设独立的前端阶段。

**`types` 退场时的引用分流**（S1-C.4.1 删 `pub use crate::dto::*` 后按性质逐处处理）：

| 类型 | 性质 | 动作 |
|---|---|---|
| `VaultEntrySummary` / `VaultEntryCreate` / `VaultEntryDetail` / `VaultEntryUpdate` / `RevealResult` | dto | 改 `dto::` |
| `VaultCategory` / `CustomFieldInput` | shared | 保持 `types::`（`types` 保留对 `shared` 的 re-export） |
| `VaultEntryCsvRow` / `VaultEntryExport` / `VaultEntrySnapshot` / `VaultEntryImport` / `ImportOutcome` | domain | 保留 `types::` |
| `VaultError` | crypto | 保留（`dto` 不导出） |

### 7.5 S2 执行约束

D5 已定：`with_unlocked` 全量替换与 `on_locked` 收敛必须同一 diff 提交，分开发布会短暂存在两套锁定清理路径。

### 7.6 S4 的两个阻塞（已定案 2026-10-30）

**阻塞 1：仓库无 CI，`git diff --exit-code` 无挂载点**

`docs/ARCHITECTURE.md` 记录「CI 目录不存在」，`.github/workflows` 为空。原 S4.4 的写法无落点。三条备选（保留以备追溯，结论见下方「定案」）：

| 方案 | 做法 | 取舍 |
|---|---|---|
| A（推荐） | 照 `package.json` 已有 `models-dev:check` 先例，加 `npm run contract:check` = `cargo test -p assistant-vault --lib export_bindings && git diff --exit-code -- src/vault/core` | 与项目既有「生成 + 比对」模式一致，零新增基础设施；靠人工 / 本地 hook 触发，非自动拦截 |
| B | 新增 `.github/workflows/contract.yml` | 能自动拦截，但项目当前无 CI 基建，为单条检查引入整套 workflow 成本高 |
| C | 挂 `.git/hooks/pre-commit` | 本地强制，但 hook 不入库，换机器即失效 |

**阻塞 2：输入类 DTO 可选键与生成物冲突**

`VaultEntryCreate` / `VaultEntryUpdate` 的 `Option<String>` 经 serde 默认形态输出 `null`，ts-rs 生成必填键 `username: string | null`；前端 [vault.types.ts](../../src/vault/core/vault.types.ts) 手写为 `username?: string`。§3.1 第 7 项只约束输出方向。三条备选：

| 方案 | 做法 | 取舍 |
|---|---|---|
| A（推荐） | 输入类保留手写，仅输出 DTO 用生成物 | 改动面最小；`vault.types.ts` 变成混合文件，需分区注释说明哪些是生成、哪些手写 |
| B | 输入类给 `Option` 加 `#[serde(default, skip_serializing_if = "Option::is_none")]`，让 ts-rs 产出 `?:` | 输入形态与输出不一致；需先验证 ts-rs 是否把 `skip_serializing_if` 映射为可选键（v12 未验证） |
| C | 前端全量改为显式 `null` | 语义最一致，但要改所有构造 entry 的调用点 |

**定案（2026-10-30）**

| 阻塞 | 结论 | 依据 |
|---|---|---|
| 阻塞 1 | 采用 A，但比对命令改为 `git status --porcelain -- src/vault/core/contract`，脚本落 `scripts/check-vault-contract.mjs`，入口 `npm run contract:check` | 见 §6 D8。`git diff` 对未跟踪文件恒为空，ts-rs 每类型一文件，新增 DTO 必然产生未跟踪文件，用 `diff` 会漏报。不选 B（项目无 CI 基建），不选 C（hook 不入库） |
| 阻塞 2 | **A / B / C 全不选，新增方案 D**：输入类每个 `Option` 字段加 `#[ts(optional = nullable)]`，生成 `username?: string \| null`，serde 不动 | 见 §6 D9。该形态即 wire 真实语义（缺字段与 `null` 都落 `None`），前端 `\|\| undefined` 构造点零改动，且不让两个最易加字段的输入类型永久留在手写侧。**落地前须先验证**：v12 是否真把 `optional = nullable` 渲染为可选键；若不是，退方案 A |
| 事件遗留 | 采用「三处一并删除」 | 见 §6 D11。reveal 结果同步返回、限流走 `Err`，vault 单窗口无跨窗口消费者；meta-integrity 触发点已淘汰，常量 / payload / 监听三件套无生产者 |

三条结论均已回填 §6 决策表与任务清单 S4 / S5 步骤，S4 可开工。

---

## 8. 核对记录

### 8.1 S2 前后端对应检查（2026-10-30）

检查范围：65 个 vault 命令的注册名、参数名、返回类型、DTO 字段、枚举取值、事件定义与监听方。
检查方式：脚本化差集比对（后端 `pub fn` 定义、`generate_handler!` 注册、前端 `invoke()` 命令名三方取集）+ DTO 字段名 `snake_case → camelCase` 转换后与 `vault.types.ts` 逐字段比对 + 事件常量与 `listen()` / `emit_broadcast()` 调用点比对，并辅以人工逐条核对签名。

#### 结论一：命令契约三方完全一致

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

#### 结论二：DTO 字段与枚举零错位

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

#### 结论三：事件层存在 3 处遗留契约（不影响命令调用，属空转）

**A. 后端广播、前端无消费者（2 个事件）**

- `vault:reveal`、`vault:reveal-rate-limited` 在 `src-tauri/src/cmd/vault/reveal.rs` 共 4 处广播（password / notes 各一正常分支 + 一限流分支），但全仓 `listen()` 无对应监听
- I9 规格第 13 条要求「前端可监听显示 toast」，当前未实现

**B. 前端监听、后端无广播点（1 个事件）**

- `src/vault/core/useVaultEvents.ts` 监听 `vault:meta-integrity-warning`；`event_name::VAULT_META_INTEGRITY_WARNING` 与 `VaultMetaIntegrityWarningPayload` 仍定义在 `src-tauri/src/protocol/events.rs`
- 全仓 `emit_broadcast` 调用点无一使用该事件：原触发点 `vault_unlock_with_recovery_key` 已随 v1.5 语义淘汰（见 `docs/achievement2/I9-vault-tasklist-p2.md` J3.4 备注），广播点消失后常量、payload、前端监听成为遗留三件套

处置已定案（§6 D11，2026-10-30）：**三处一并删除**。reveal 结果已由命令返回值同步送达、限流已走 `Err(rateLimited)`，vault 单窗口无跨窗口消费者，广播无存在理由；meta-integrity 三件套无生产者。执行步骤见任务清单 S5，与 S4 无依赖。

#### 结论四：其余核对项均通过

- 前端无 `message.includes(` 式错误判定，全部按 `code` 分支（`src/vault/core/vaultError.ts`）；全仓剩余 4 处 `includes(` 为标签数组去重与平台判断，非错误判定，V9 满足
- 事件载荷语义一致：`vault:locked` / `vault:unlocked` 广播 `()`，`vault:reveal` 广播 entry id，`vault:biometric-enroll-step` 广播 `"credential-created"`；前端对应监听均为无参回调，不解析 payload，无形态冲突
- `vaultService` 方法使用率扫描仅报 2 个「疑似未调用」：`historyList` 经链式调用 `src/vault/detail/VaultEntryHistory.tsx` 确认在用；`totpTimeRemaining` 确为死方法（命令 `vault_totp_time_remaining` 已注册，UI 走 store 本地倒计时），非契约缺陷，可择机清理
- 类型宽窄差异（非缺陷，仅记录）：`gitTestConnection` service 声明 `Promise<number>`、slice 声明 `Promise<number | null>`（catch 兜 `null`）；`StrengthResult.warnings` 后端为枚举 `Vec<StrengthWarning>`、前端为 `string[]`（序列化形态一致，仅弱约束）

工具链状态：`npm run typecheck`（`tsc --noEmit`）零错误。本次检查为只读核对，未改动源码。

本次检查未覆盖：S3.2 校验 C 依赖 `VaultConf` 落地，当时尚未可执行（S3 已落地并全绿，见 §8.2）；S4-0 `ts-rs` spike 当时未启动（现已完成，见 §7.1）。

### 8.2 S3 前后端对应检查（2026-10-30）

检查范围：13 个 `vault_*` 字段迁出 `AppConf` 后的残留、配置读写链路、前后端配置组映射、校验 C。
检查方式：`vault_*` 字段访问点全仓 grep + `VaultConf` 声明比对 + 配置组映射逐处核对 + 实际跑测试。

#### 结论一：迁移零残留

| 维度 | 结果 |
|---|---|
| `VaultConf` 字段 | 13，与迁移前 `AppConf` 的 13 个 `vault_*` 同名同类型 |
| `AppConf` 残留 `vault_*` | 0（`vault_enabled` 零消费者，未保留） |
| `AppConfDto` 残留 vault 镜像 | 0，改为独立 `VaultConfDto`，`From<VaultConf>` 无 `..` |
| 前端 `AppConf` interface 残留 | 0，独立 `VaultConf` interface 13 字段 |
| 后端读取点 | 15 行 / 16 处（`cmd/vault/mod.rs` / `sync.rs` / `ssh.rs`），全部 `VaultConf::load()` |
| 前端业务读取点 | 3 处（`vaultLifecycleSlice` 自动锁时长、`VaultSyncSettings` SSH 主机、`SshAgentTab` 自动添加） |

迁移兼容：首次加载时 `vault-config.json` 不存在则从同目录 `config.json` 提取 13 字段并落独立文件；老 `config.json` 残留键不清理——`AppConf` 无 `deny_unknown_fields`（加上会破坏上述迁移），因此 `patch_conf("app", { vaultXxx })` 不报错也不生效。已核对 `dispatchConfPatch("app", …)` 全部调用点，只剩 `theme` / `keybindings` / `stayOnTop`，无 vault 键残留。

#### 结论二：新增配置组需 6 处映射同步

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

#### 结论三：`on_app_conf_changed` 不是冗余回调

原 `on_app_conf_changed` 承担「配置变更 → 进程内全局状态」同步（`set_ssh_persist_hosts` + 清 `SSH_PERSIST_GUESS_CACHE`），不是残留。字段迁出后 `AppConf` 已无 `vault_sync_ssh_hosts`，故更名 `on_vault_conf_changed(prev, conf)`，由 `patch_conf` 的 `"vault"` 分支经 `apply_vault_side_effects` 调用，语义等价。其余 12 个字段按需 `VaultConf::load()`，无常驻内存副本，不需要 hook。

#### 结论四：校验 C 的实现与规格偏差

§4.2 数据源写的是「`ReadsConf` / `WritesConf` 声明 + `app_conf.rs`」，实际未落地声明（与 §3.3「不建清单」同一取向：声明无法自证完备），改为扫描真实读取点：

- 扫描 `src-tauri/src/cmd/vault/*.rs` 的 `.vault_*` **字段访问**，与 `vault_conf.rs` 的 `pub vault_*` 声明取差集，差集非空即失败
- 字段访问与方法调用按紧随的 `(` 区分，`.vault_file_manager()` 不计入配置键，因此**不维护白名单副本**
- 自检两条：注入 `vault_phantom` 必须被报出；声明存在但无读取点不得误报

已知盲区（记录，不修）：

- 只扫 `cmd/vault/` 目录；`protocol/dto/config.rs` 的 `From<VaultConf>` 逐字段映射不在扫描内，该处键名写错只能靠 review
- 只识别 `.vault_*` 字面量，别名导入 / 重命名不会被发现

---

## 9. 禁止事项

- 不建前端注册表框架（`defineVaultFeature()` 已剔除，D4）
- 不建 `Effect` 枚举、不写 `effects` 声明（§3.3）
- 不建命令清单 / 扫描器 / 过程宏（§2.1）
- 不新增 `DecryptFailed` 变体（折叠进 `internal`，§3.4）
- 不引入 `UnlockOutcomeDto`（失败走 `Err`，§3.4）
- 不把 `EntryFile` / `VaultMetaFile` / `Argon2Params` / `VaultEntryCsvRow` 带进 `dto`（§2.2 判据①）
- 不给 `dto` 加 `serde_json::Value` 或任何「以后可能有用」的袋子字段
- 契约测试 fixture 不用真实数据（§3.5 ①）
- vault `dto` 不加 `skip_serializing_if`（§3.1 第 7 项）
- 不用 `contains_key` 断言结构体字段（新增字段照样绿，§7.3）
- 不手改 `src/vault/core/contract/` 生成物（D10）；要改形态就改 Rust DTO 再导出
- 不留无 `listen()` 消费者的 `emit_broadcast`（D11，§2.2 判据）
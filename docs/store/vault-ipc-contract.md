# Vault IPC 契约边界审查与收口方案

> 审查日期：2026-10-29
> 审查对象：`src-tauri/crates/assistant-vault`、`assistant-sync`、`assistant-security`、`assistant-crypto` 与 `src-tauri/src/cmd/vault/*` 之间的 IPC 边界；`src/vault/**/*.types.ts` 的前端镜像
> 审查方法：`src-tauri/src/lib.rs` 的 66 条 `vault_*` 注册逐条回溯到实现签名；按「跨边界值形态」分四类穷举，而非只审子 crate 直出；前端 30 个手写类型逐个与 Rust 侧字段对照
> 取代关系：本文**完整取代** [dto-boundary-analysis.md](./dto-boundary-analysis.md)，后者仅作历史记录保留

---

## 0. 结论摘要

1. **问题边界比旧文档宽**。旧文档只审「子 crate 类型直出」一类，实际跨 IPC 边界的值形态有**四类**：子 crate 直出、cmd 层本地类型、元组/匿名形态、错误。旧文档漏了后三类，其中「元组形态」与「错误」是最危险的两种。
2. **`protocol` 不是收容所**。`src-tauri/src/protocol` 依赖 `storage` / `providers` / `config`，把 DTO 搬进去只是把「子 crate 直出」升级成「更大 crate 的直出」。旧文档 P3「统一搬迁至 protocol」应作废，改为**原地收编到 `assistant-vault::dto`**。
3. **不需要搬迁就能合法**。`assistant-vault` 是准叶节点，`impl From<assistant_sync::X> for VaultXDto` 写在 `assistant-vault` 内永远满足孤儿规则。旧文档「等 P3 再决定归属」把一个可立即执行的动作挂成了悬案。
4. **`pub use` 兼容层是边界失效的机制本身**。`assistant_vault::types` 全量 re-export `dto`，导致 8 个 `cmd/vault` 文件全部走旧路径，编译器无法区分「wire 契约」与「领域类型」。必须给它定退场条件，而不是当成优点。
5. **前端镜像存在三类漂移，不只是「缺 camelCase」**：缺字段（真实 bug）、TS 超集（前端自造字段）、**TS 比 Rust 更窄（类型在撒谎）**。第三类旧文档完全没记录。
6. **错误不是成功路径的附属**。66 条命令全为 `Result<T, String>`，`VaultError` 没有 `Serialize`；另有 4 条命令把失败折叠成 `Ok(false)`。DTO 收编与错误结构化必须合并到**同一个签名冻结窗口**，否则要改两遍。

---

## 1. 审查口径

### 1.1 什么算「跨 IPC 边界」

一个值形态算跨边界，当且仅当它**出现在 `#[command]` 函数的入参或返回类型中**，包括：

- 直接出现（`-> VaultEntryDetail`）
- 包在容器里出现（`-> Vec<ConflictFile>`、`-> Option<PendingRemovals>`）
- 作为枚举变体的载荷出现（`SyncResult::Conflict(Vec<ConflictFile>)`）
- 作为字段嵌套出现（`ConflictPreview.local: Option<EntryPreview>`）

**入参同样算边界**。旧文档只审返回类型，漏掉了 `vault_set_sync_config(config: SyncConfig)` 与 `vault_resolve_conflict(resolution: ConflictResolution)` 这两处入参。

### 1.2 三条硬性口径

| 口径 | 含义 | 为什么 |
|---|---|---|
| 不靠人读表 | 清单必须能由命令 manifest 机械生成/校验 | 人工 grep 是一次性动作，不是机制（见 §4.1 的 `AuditLogEntry` bug） |
| 只认签名 | 「看起来像 DTO」不算，必须真的出现在 `#[command]` 签名里 | 避免把 `ImportReport` 这类纯领域类型误收编 |
| 格式与归属分开判 | 「字段名对不对」与「类型该放哪」是两个独立问题 | 旧文档把 13 个类型记为「契约本身没坏」，却仍在待办里，读者容易误以为可以不处理 |

### 1.3 与既有文档的关系

- [crate-architecture-analysis.md](./crate-architecture-analysis.md) §3.1.1 的「5 个直出类型」仍是低估值，本文 §3 给出修正后的完整口径。
- [I16-protocol-type-misplacement-audit.md](./I16-protocol-type-misplacement-audit.md) 记录的「子 crate 改枚举变体名，前端不会编译报错」是本文 §3.1 的同源问题，本文把它升级为「类型归属」而非「命名」问题。

---

## 2. 事实基线

### 2.1 规模

| 项 | 数量 | 位置 |
|---|---|---|
| 注册的 vault 命令 | **66** | `src-tauri/src/lib.rs` 的 `invoke_handler` |
| 命令实现文件 | 9 | `src-tauri/src/cmd/vault/{lifecycle,entries,reveal,audit,sync,ssh,biometric,recovery,mod}.rs` |
| 前端手写导出类型 | **30** | `src/vault/core/vault.types.ts` |
| `vault_*` 配置字段 | **13** | `crates/assistant-config/src/app_conf.rs` |
| `vault:*` 广播事件 | **6** | `src-tauri/src/protocol/events.rs` |
| 全局 `manage` 状态 | 5 | `cmd/vault/mod.rs::setup`：`SecurityDbState` / `RevealGuard` / `ClipboardGuard` / `SshAgentState` / `SyncEngine` |

### 2.2 五个耦合面

| 面 | 现状 | 位置 |
|---|---|---|
| 命令注册 | 66 条硬编码在 `generate_handler!` | `src-tauri/src/lib.rs` |
| 状态注册 | 5 个 state 由 `cmd::vault::setup()` 自己 `manage`，并顺带启动屏幕锁监听 | `cmd/vault/mod.rs` 的 `setup` |
| 配置 | 13 个字段进 `AppConf`，命令层直接 `AppConf::load()` 读写 | `app_conf.rs`、`cmd/vault/mod.rs`、`cmd/vault/sync.rs` |
| 前端接线 | 路由 `lazy`、侧边栏项、设置页三处硬编码 | `src/layouts/MainLayout.tsx`、`src/components/AppSidebar.tsx`、`src/settings/core/pageRegistry.ts` |
| 类型 | 30 个手写 TS 镜像，零自动校验 | `src/vault/core/vault.types.ts` |

---

## 3. 边界值形态的四个类别

这是本文相对旧文档的核心重构：**旧文档只审 A 类，实际 A/B/C/D 四类都在边界上。**

### 3.1 A 类：子 crate 类型直出

**13 个类型，出现在 15 处命令签名**（返回 13 处 + 入参 2 处）：

| crate | 类型 | 签名位置（文件:行） | 方向 |
|---|---|---|---|
| `assistant_sync` | `SyncResult` | `cmd/vault/sync.rs:26` | 返回 |
| `assistant_sync` | `SyncConfig` | `cmd/vault/sync.rs:70` / `:85` | 返回 + **入参** |
| `assistant_sync` | `PendingRemovals` | `cmd/vault/sync.rs:120`（`Option<…>`） | 返回 |
| `assistant_sync` | `Readyness` | `cmd/vault/sync.rs:255` | 返回 |
| `assistant_sync` | `ConflictFile` | `cmd/vault/sync.rs:261`（`Vec<…>`） | 返回 |
| `assistant_sync` | `HistoryCommit` | `cmd/vault/sync.rs:272`（`Vec<…>`） | 返回 |
| `assistant_sync` | `EntryPreview` | `cmd/vault/sync.rs:284` | 返回 |
| `assistant_sync` | `ConflictPreview` | `cmd/vault/sync.rs:327` | 返回 |
| `assistant_sync` | `ConflictResolution` | `cmd/vault/sync.rs:101` | **入参** |
| `assistant_security` | `SshKeyInfo` | `cmd/vault/ssh.rs:80`（`Vec<…>`） | 返回 |
| `assistant_security` | `SshKeyMeta` | `cmd/vault/ssh.rs:155` / `:164` | 返回 ×2 |
| `assistant_crypto` | `TotpResult` | `cmd/vault/audit.rs:16` | 返回 |
| `assistant_crypto` | `ParsedOtpAuth` | `cmd/vault/audit.rs:103` | 返回 |

**另有 5 个传递性类型随上述类型一起上线**（不出现在签名，但出现在 JSON 里）：

`UnreadableReason`、`PendingRemovalEntry`、`RepositoryState`（嵌套于 `Readyness`）、`EntryPreviewField`、`TotpAlgorithm`。

**刻意排除**（同模块但不在边界上，按 §5 判据②不得收编）：

`ImportSummary`、`ImportReport`、`SyncError`、`SyncAction` 之外的 `assistant_sync` 内部类型。其中 `ImportReport` / `ImportSummary` 带 `rename_all = "camelCase"`，容易被误判为 wire 类型——实测只在 `sync_engine/mod.rs` 内部使用，从不进命令。

> 旧文档把这批「有 camelCase 就收编」混在一起，是 §5 判据②缺失机械判定导致的。

### 3.2 B 类：cmd 层本地类型

定义在 `src-tauri/src/cmd/vault/*` 内、但序列化后进前端的类型。它们**不在任何 crate 的 DTO 清单里**，因此最容易在清单之间被漏掉：

| 类型 | 位置 | 方向 | 前端镜像 |
|---|---|---|---|
| `DisasterRecoveryResult` | `cmd/vault/recovery.rs:127`（含 `rename_all`） | 返回 | ✅ `vault.types.ts:229` |
| `SyncAction` | `cmd/vault/sync.rs:8`（含 `rename_all`） | 入参 | ❌ 无 TS 类型，调用侧用裸字符串 `"forcePush"` |
| `RemoteMode` | `cmd/vault/sync.rs:17`（含 `rename_all`） | 入参 | ❌ 无 TS 类型 |

后两者是**真正的裸契约**：前端 `vault.service.ts` 直接写 `invoke("vault_sync", { action: "forcePush" })`，字符串拼错不会被任何编译器或类型检查捕获。

### 3.3 C 类：元组 / 匿名形态

| 命令 | 返回 | 位置 | 问题 |
|---|---|---|---|
| `vault_hibp_audit` | `Vec<(String, u32)>` | `cmd/vault/audit.rs:154` | 无结构名、无字段名；前端 `vaultAuditSlice.ts` 用 `for (const [id, count] of leaks)` 位置解构 |

这类形态**连结构都没有**：Rust 侧把元组改成 `(String, u32, bool)`，TS 侧不报错、运行时静默错位。任何「手写镜像 + 契约测试只校验 TS 自身字段」的机制都覆盖不到它。

**判定**：C 类是四类中**唯一无法被任何类型检查兜底**的，优先级高于 A 类。

### 3.4 D 类：错误

三层问题，逐层加重：

**D1 — 错误被降级为字符串。** 66 条命令全部 `Result<T, String>`。前端 `catch` 拿到的是字符串，只能靠 `message.includes(...)` 判断语义，`locked` / `io` / `syncConflict` 无法区分。

**D2 — 结构化错误被主动丢弃。** `assistant_crypto::error::VaultError` 是 `thiserror` 枚举，有 `Locked` / `InvalidPassword` / `NotInitialized` / `BiometricReset` 等 21 个变体，但**没有 `Serialize`**。命令层 `map_err(|e| e.to_string())` 把枚举压成字符串，信息在边界处不可逆丢失。

**D3 — 失败被折叠成 `Ok(false)`。** 4 条命令用 bool 承载「失败但不算错」：

| 命令 | 折叠的语义 | 位置 |
|---|---|---|
| `vault_unlock` | `InvalidPassword` → `Ok(false)` | `cmd/vault/lifecycle.rs:19` |
| `vault_show_unlock_window` | `InvalidPassword` / `Cancelled` → `Ok(false)` | `cmd/vault/lifecycle.rs:74` |
| `vault_unlock_biometric` | `BiometricCancelled` → `Ok(false)` | `cmd/vault/biometric.rs:41` |
| `vault_unlock_with_recovery_key` | 解锁结果 bool | `cmd/vault/recovery.rs:34` |

后果：前端无法区分「用户取消」「密码错误」「需要重新录入生物识别」——而 `BiometricReset` 变体明明存在，只是永远到不了前端。

### 3.5 合计

| 类别 | 类型数 | 签名位置数 | 可否被现有机制发现 |
|---|---|---|---|
| A 子 crate 直出 | 13（+5 传递性） | 15 | ❌ 无 |
| B cmd 本地 | 3 | 3 | ❌ 无（不在任何清单） |
| C 元组 | 1 形态 | 1 | ❌ 无（无结构可校验） |
| D 错误 | 1 形态（21 变体） | 66 | ❌ 无 |

**非 dto 的 wire 类型合计 16 个 + 2 种形态**（旧文档记为 13 个，且把已收敛的 `IntegrityResult` 算在内，实际漏 B/C/D 三类）。

---

## 4. 前端镜像的三类漂移

`src/vault/core/vault.types.ts` 的 30 个导出并非「Rust 类型的镜像」，而是**三种不同关系的混合体**。分开看才知道哪些能直接替换、哪些不能：

### 4.1 缺字段漂移（真实 bug，已修）

`AuditLogEntry` 缺 `rename_all = "camelCase"`，Rust 输出 `event_type`，TS 声明 `eventType`，审计页「事件类型」列恒为 `undefined`。已修（`crates/assistant-vault/src/dto/audit.rs`）。

**但修复方式不可持续**：靠人逐个对照 30 个类型。旧文档的「同类排查无第二例」结论**不成立**——第二例就是 §3.3 的元组形态，且更严重。

### 4.2 TS 超集（前端自造字段）

| 类型 | Rust | TS | 说明 |
|---|---|---|---|
| `RevealedField` | `RevealResult { value, expires_at }` | `{ value, expiresAt, kind: "password" \| "notes" }` | `kind` 由前端 `vaultRevealSlice.ts` 自行注入，Rust 不产出 |
| `UnreadableEntries` | `SyncResult::UnreadableEntries { ids, reason }`（内联变体） | 独立 `interface UnreadableEntries` | TS 把内联变体提成了具名类型 |

**影响**：若直接用 `ts-rs` 生成 TS，生成的 `RevealResult` 会**缺少 `kind`**，覆盖手写文件会编译失败。所以生成不能「一键替换」，必须先把手写类型拆成「生成部分 + 前端扩展部分」。

### 4.3 TS 比 Rust 更窄（类型在撒谎）

| 类型 | Rust | TS | 风险 |
|---|---|---|---|
| `EntryPreview.category` | `String` | `VaultCategory` | DB 里出现非枚举值（旧数据 / 手改 / 同步冲突）时，TS 类型断言为假，前端 `switch` 落到 `default` 分支但类型检查认为不可能 |

这是**比缺字段更隐蔽**的一类：不是「前端少了个字段」，而是「前端把宽松类型声明成了窄类型」。任何「生成 TS」的方案都会让这个谎言变成**自动生成的真话**（生成 `string`），届时前端所有依赖 `VaultCategory` 的地方会报错——这其实是好事，但要提前知道，不能当成回归。

### 4.4 命名不一致

`Readyness`（Rust 与 TS 均为拼写错误，`crates/assistant-sync/src/types/mod.rs:47` 与 `vault.types.ts:103`）。若收编时改名 `Readiness`，必须 **Rust 结构体 / serde 输出 / TS 类型 / 序列化快照**四处同步，否则会在「格式没坏」的假象下引入真断链。

---

## 5. 归属判据：四问

三问判「是不是 wire」，一问判「放哪个 crate」。**任一为「是」即不进 `dto`**：

| # | 问题 | 是 → 归属 |
|---|---|---|
| ① | 字段名会出现在 `.vault` 文件 / CSV / DB 列里？ | 持久化层（`types` 或 `vault_file/types`） |
| ② | 是否**真的**出现在某个 `#[command]` 的入参或返回类型里（含嵌套）？ | 否 → domain 层（`types`） |
| ③ | 承载加密语义（`MasterKey` / `VaultPlaintext` / `SecureString`）？ | 留在 `assistant-crypto` |
| ④ | 是否有**第二个消费者**（vault 之外的业务）？ | 否 → 归 `assistant-vault::dto` |

四问全「否」→ 进 `dto`，加 `#[serde(rename_all = "camelCase")]`。

**判据②必须机械可判定**：数据源是 §6.2 的命令 manifest，不是人读表。这是四问里唯一能自动化的一问，也是旧文档三问版本失效的原因（它靠人工筛，于是 `ImportReport` 混进了候选）。

**判据④的作用**：把「暂放 vault、将来搬 protocol」的悬案就地结案。`assistant-sync` 与 `assistant-security` 当前都没有第二个消费者（sync 只为 vault 存在，SSH agent 只为 vault 存在），按 ④ 一律归 `vault::dto`。若将来出现第二个消费者，再评估搬迁——但那时的搬迁动因是**真实需求**，而不是「终局好看」。

---

## 6. 目标态设计

### 6.1 模块落点

```
crates/assistant-vault/src/
├── shared/          VaultCategory / CustomFieldInput（dto 与 domain 的公共底座）
├── dto/             ← 唯一 wire 层，全部 camelCase
│   ├── mod.rs       VaultEntry* / RevealResult + sync / ssh / totp DTO
│   ├── audit.rs     StrengthResult / StrengthWarning / AuditLogEntry
│   ├── integrity.rs IntegrityResult
│   ├── sync.rs      SyncResultDto / SyncConfigDto / …（新）
│   ├── ssh.rs       SshKeyInfoDto / SshKeyMetaDto（新）
│   ├── totp.rs      TotpResultDto / ParsedOtpAuthDto（新）
│   ├── recovery.rs  DisasterRecoveryResultDto（新，从 cmd 层下沉）
│   ├── hibp.rs      HibpAuditEntryDto（新，替代元组）
│   └── error.rs     VaultErrorDto + VaultErrorCode（新）
└── types/           domain + 持久化（**不再 re-export dto**）
```

约束：

- `dto` 只依赖 `serde` + `shared`，零 domain 依赖。
- `dto` 不得依赖 `types`（否则 domain 字段改动会静默改契约）。
- `types` 不再 `pub use crate::dto::*`（见 §7 的退场条件）。
- `From<Domain> for Dto` 写在 `assistant-vault` 内，孤儿规则永远满足。

### 6.2 命令 manifest（S0 的核心产出）

```rust
pub struct VaultCommandSpec {
    pub name: &'static str,               // "vault_sync"
    pub request: &'static str,            // "SyncAction" | "()" | "SyncConfig"
    pub response: &'static str,           // "SyncResultDto" | "u32" | "()"
    pub precondition: Precondition,       // None / Initialized / Unlocked / BiometricEnrolled
    pub blocking: bool,                   // 是否 spawn_blocking
    pub effects: &'static [Effect],       // EmitsEvent(..) / ReadsConf(..) / WritesConf(..) / MutatesDb / ClearsClipboard
}
```

三条校验（全部是机械的）：

1. **注册一致性**：`invoke_handler` 里出现的每个 `vault_*` 必须在 manifest 中登记，反之亦然。用文本扫描 `lib.rs` 的注册块即可，无需编译期魔法。
2. **DTO 一致性**：manifest 里每个 `response` / `request` 类型名，必须能在 `dto` 模块中找到（`()` 与标量除外）。这一条同时承担 §5 判据②的机械判定。
3. **门禁一致性**：`precondition: Unlocked` 的命令，函数体首个语句必须是门禁调用。`precondition: None` 的命令不得调用门禁。这一条能捕获「漏检解锁状态」——那是明文泄露级 bug。

**manifest 顺带暴露的越界**：`vault_sync_last_time` 没有 `AppHandle` 参数，直接 `assistant_config::app_conf::AppConf::load()` 读 `vault_sync_last_at`，且返回 `Option<i64>` 而非 `Result`。manifest 的 `effects` 应标记 `ReadsConf`，随后把该值迁入 vault 自己的状态，命令只读 state。

### 6.3 错误 DTO（与 DTO 收编合并为同一窗口）

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultErrorDto {
    pub code: VaultErrorCode,     // 稳定枚举，前端可 switch
    pub message: String,          // 人类可读，仅用于展示
    pub retryable: bool,          // 前端可自动重试
    pub details: Option<serde_json::Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VaultErrorCode {
    Locked, InvalidPassword, NotInitialized, AlreadyExists, RateLimited,
    NotFound, Validation, Clipboard, Io, Database,
    BiometricUnavailable, BiometricCancelled, BiometricReset, BiometricFailed,
    InvalidKeyFormat, InvalidRecoveryKey, RecoveryKeyNotSet, VaultMetaIntegrityFailed,
    SyncConflict, GitUnavailable, CorruptEntry, Internal,
}
```

映射规则：`impl From<VaultError> for VaultErrorDto`，21 个变体一一对应，`Other(String)` → `Internal`；`SyncConflict` / `GitUnavailable` / `CorruptEntry` 三个码由 `SyncError` 与文件读取路径补充映射（它们不属于 `VaultError`）。

**D3 的修法**：4 条折叠成 `Ok(false)` 的命令，改为返回 `Result<UnlockOutcomeDto, VaultErrorDto>`，其中 `UnlockOutcomeDto { unlocked: bool, reason: Option<UnlockFailReason> }`。这样「取消」「密码错误」「需重新录入」三个语义可区分，且 `BiometricReset` 终于能到前端。

**排期约束**：DTO 收编与错误 DTO 都要改命令签名。这是**唯一一次全量改签名的窗口**，必须合并，否则前端 `invoke` 的 reject 处理要改两遍。

### 6.4 状态门禁收敛

现状三种门禁辅助函数分散在 `cmd/vault/mod.rs`：`vault_state` / `sync_state` / `reveal_guard` / `clipboard_guard`，每条命令手写调用。建议收敛为高阶函数：

```rust
pub fn with_unlocked<T>(app: &AppHandle, f: impl FnOnce(&SecurityDbState) -> Result<T, VaultError>) -> Result<T, VaultErrorDto>
```

收益：门禁调用点从「每条命令一处」变为「一处」，manifest 的 `precondition` 校验也就有了唯一比对目标。

**同时明确职责边界**：锁定时的「清剪贴板 / 摘 SSH key / 清 reveal 令牌」目前散在 `vault_lock`、屏幕锁回调、`ssh_remove_all_on_lock` 三处，靠人记。应收敛为一个 `on_locked(app)` 单一入口，由 `vault_lock` 与屏幕锁回调共同调用。这条不是类型问题，但属于同一份契约（「锁定意味着什么」）。

### 6.5 校验机制：golden JSON 快照

每个 DTO 配一个快照测试，落在 `assistant-vault` 的测试里：

```rust
#[test]
fn sync_result_dto_shape() {
    let json = serde_json::to_value(SyncResultDto::UpToDate).unwrap();
    assert_eq!(json, serde_json::json!("upToDate"));
}
```

覆盖范围：A 类 13 个 + B 类 3 个 + C 类替代类型 + D 类 `VaultErrorDto`。这一层**不需要 ts-rs 就能成立**，是 S1 的验收基础，也是 S4 生成前的基线。

要点：快照断言的是 **JSON 形态**（key 集合 + 命名风格），不是 TS 声明。这样 §4.3「TS 比 Rust 更窄」这类问题会被暴露为「快照说 `category` 是任意字符串，TS 说它是枚举」。

### 6.6 生成（S4，非起点）

`ts-rs` 生成 `vault.contract.gen.ts`，CI 加 `cargo test export_bindings && git diff --exit-code`。

**前置条件（三条，缺一不可）**：

1. A/B/C/D 四类收编完成——否则生成的仍是半截契约。
2. 手写类型拆分为「生成部分」与「前端扩展部分」（§4.2 的 `kind` 字段不能进生成）。
3. §4.3 的窄类型问题先修——生成会把 `VaultCategory` 变成 `string`，前端会报一批错，这是预期内的、需要单独排期的清理。

---

## 7. 分阶段落地

| 阶段 | 动作 | 验收 | 风险 |
|---|---|---|---|
| **S0** | 写 `dto/../contract/commands.rs` manifest + 三条一致性测试，**不改任何行为** | 66 条命令 100% 登记；`precondition` 与实际门禁调用不一致即测试失败；`vault_sync_last_time` 的 `ReadsConf` 被标记 | 零 |
| **S1** | 错误 DTO + DTO 收编（A/B/C 三类）+ 4 条 `Ok(false)` 改 `UnlockOutcomeDto`；`types` 的 `pub use dto` 加 `#[deprecated]` | golden JSON 快照全绿；`grep 'assistant_vault::types::'` 命中数 = 0（领域类型除外）；前端无字符串匹配错误 | 中（全量签名） |
| **S2** | 门禁收敛为 `with_unlocked` + `on_locked` 单一入口 | 漏检命令数 = 0；锁定路径唯一 | 低 |
| **S3** | 13 个 `vault_*` 配置外置为 `VaultConf`；前端抽 `defineVaultFeature()` 收敛三处硬编码 | `AppConf` 只剩 `vault_enabled: bool`；宿主三处硬编码消失 | 低 |
| **S4** | `ts-rs` 生成 + CI 漂移检查 | `git diff` 为空才通过 | 低 |

**S0→S2 是「契约」，S3→S4 才是「工程化」**。只做 S0–S2 已解决 80% 的维护痛点（漂移、漏检、错误不可判），且 S0 完全不动运行行为，可以先落地。

**为什么 S0 必须排第一**：manifest 是 §5 判据② 的数据源，也是 S1 收编清单的数据源。跳过 S0 直接收编，等于用人工清单替换人工清单，机制没有变。

---

## 8. 明确不做

| 不做 | 理由 |
|---|---|
| **DTO 统一搬迁至 `protocol`** | `protocol` 依赖 `storage` / `providers` / `config`，不是叶节点。搬进去只是把「子 crate 直出」升级为「更大 crate 的直出」，边界变大不变清晰。且 `assistant-vault` 内写 `From` 已合法，搬迁无收益 |
| **拆细 crate**（`vault-domain` / `vault-sync` / `vault-ui`） | 当前 `assistant-vault` 是准叶节点，拆细只增加 `From` 边界与编译单元 |

---

## 9. 对既有文档的修正登记

| 文档 | 原记录 | 修正 |
|---|---|---|
| [dto-boundary-analysis.md](./dto-boundary-analysis.md) §0.1 | 问题定义为「子 crate 类型直出」 | 扩为「四类跨边界值形态」（本文 §3） |
| 同上 §4 | 13 个去重类型 / 14 处签名（含已收敛的 `IntegrityResult`） | A 类实际 13 个（不含 `IntegrityResult`）/ 15 处签名；另有 B 类 3 个、C 类 1 形态、D 类 1 形态 |
| 同上 §4「补充事实」 | 13 个类型 camelCase 都正确，「契约本身没坏」 | 格式正确 ≠ 归属正确；且 `EntryPreview.category` 存在 **TS 比 Rust 更窄**的漂移（§4.3） |
| 同上 §6「同类排查无第二例」 | 结论成立 | 不成立。第二例是元组形态（§3.3），且无任何机制可发现 |
| 同上 §7 P2 | 「需先确定 sync DTO 归 vault 还是主 app」 | 按判据④（无第二消费者）**立即归 `assistant-vault::dto`**，不再等待 |
| 同上 §7 P3 | 统一搬迁至 `protocol` | **作废**（§8） |
| 同上 §7 缺项 | 无错误 DTO | 补 D 类错误契约，且与 DTO 收编合并为同一窗口（§6.3） |
| 同上 §7 P1 | `ReadinessDto`（已改名） | 实际类型名 `Readyness`；改名须四处同步（§4.4） |
| [crate-architecture-analysis.md](./crate-architecture-analysis.md) §3.1.1 | 「5 个类型直出前端」 | A 类 13 个 + 传递性 5 个；B/C/D 另计 |
| 同上 §3.1.1 表格 | 未记 crypto / security / cmd 本地类型 | 补 `TotpResult` / `ParsedOtpAuth` / `SshKeyInfo` / `SshKeyMeta` / `DisasterRecoveryResult` |

---

## 10. 附录 A：66 条命令全表

形态列含义：`标量` = `()` / `bool` / `u32` / `usize` / `String` / `Option<i64>`，无需 DTO；`dto` = 已在 `assistant-vault::dto`；`A` / `B` / `C` = §3 的三类；`D` = 错误形态，全部命中。

| # | 命令 | 返回类型 | 形态 | 备注 |
|---|---|---|---|---|
| 1 | `vault_init` | `()` | 标量 | 入参 `master_password` |
| 2 | `vault_unlock` | `bool` | **D3** | `InvalidPassword` 折叠为 `false` |
| 3 | `vault_is_locked` | `bool` | 标量 | |
| 4 | `vault_is_initialized` | `bool` | 标量 | |
| 5 | `vault_lock` | `()` | 标量 | 触发 `VAULT_LOCKED` |
| 6 | `vault_list_entries` | `Vec<VaultEntrySummary>` | dto | |
| 7 | `vault_get_entry` | `VaultEntryDetail` | dto | |
| 8 | `vault_copy_password` | `()` | 标量 | 副作用：剪贴板 |
| 9 | `vault_copy_field` | `()` | 标量 | 副作用：剪贴板 |
| 10 | `vault_reveal_password` | `RevealResult` | dto | 触发 `VAULT_REVEAL` |
| 11 | `vault_reveal_notes` | `RevealResult` | dto | |
| 12 | `vault_create_entry` | `String`（id） | 标量 | 语义化标量 |
| 13 | `vault_update_entry` | `()` | 标量 | |
| 14 | `vault_delete_entry` | `()` | 标量 | |
| 15 | `vault_list_deleted_entries` | `Vec<VaultEntrySummary>` | dto | |
| 16 | `vault_restore_entry` | `()` | 标量 | |
| 17 | `vault_purge_deleted` | `u32` | 标量 | |
| 18 | `vault_change_master_password` | `()` | 标量 | |
| 19 | `vault_export_to_file` | `()` | 标量 | 落盘契约，非 wire |
| 20 | `vault_import` | `u32` | 标量 | |
| 21 | `vault_totp_generate` | `TotpResult` | **A** | crypto 直出 |
| 22 | `vault_copy_totp` | `()` | 标量 | 副作用：剪贴板 |
| 23 | `vault_totp_time_remaining` | `u32` | 标量 | 纯函数，无 `AppHandle` |
| 24 | `vault_totp_parse_uri` | `ParsedOtpAuth` | **A** | crypto 直出 |
| 25 | `vault_query_audit_log` | `Vec<AuditLogEntry>` | dto | 本次已补 camelCase |
| 26 | `vault_hibp_check` | `u32` | 标量 | |
| 27 | `vault_hibp_audit` | `Vec<(String, u32)>` | **C** | 无结构名，前端位置解构 |
| 28 | `vault_check_master_password_strength` | `StrengthResult` | dto | |
| 29 | `vault_show_unlock_window` | `bool` | **D3** | 取消 / 密码错误折叠 |
| 30 | `vault_is_biometric_available` | `bool` | 标量 | 无 `AppHandle` |
| 31 | `vault_is_biometric_enrolled` | `bool` | 标量 | |
| 32 | `vault_unlock_biometric` | `bool` | **D3** | `BiometricCancelled` 折叠 |
| 33 | `vault_enroll_biometric` | `()` | 标量 | 触发 `BIOMETRIC_ENROLL_STEP` |
| 34 | `vault_disenroll_biometric` | `()` | 标量 | |
| 35 | `vault_sync` | `assistant_sync::SyncResult` | **A** | 入参 `SyncAction`（B 类） |
| 36 | `vault_sync_last_time` | `Option<i64>` | 标量 | **无 `AppHandle`，直接读 `AppConf`**，且非 `Result` |
| 37 | `vault_get_sync_config` | `assistant_sync::SyncConfig` | **A** | |
| 38 | `vault_set_sync_config` | `()` | 标量 | **入参 A 类 `SyncConfig`** |
| 39 | `vault_resolve_conflict` | `()` | 标量 | **入参 A 类 `ConflictResolution`** |
| 40 | `vault_sync_abort` | `()` | 标量 | |
| 41 | `vault_sync_discard` | `()` | 标量 | |
| 42 | `vault_git_init` | `()` | 标量 | |
| 43 | `vault_git_remove_remote` | `()` | 标量 | |
| 44 | `vault_git_test_connection` | `usize` | 标量 | |
| 45 | `vault_git_remote` | `()` | 标量 | 入参 `RemoteMode`（B 类） |
| 46 | `vault_sync_readyness` | `assistant_sync::Readyness` | **A** | 含 `RepositoryState` |
| 47 | `vault_sync_conflicts` | `Vec<assistant_sync::ConflictFile>` | **A** | |
| 48 | `vault_sync_pending_removals` | `Option<assistant_sync::PendingRemovals>` | **A** | 含 `PendingRemovalEntry` |
| 49 | `vault_sync_confirm_removals` | `u32` | 标量 | |
| 50 | `vault_history_list` | `Vec<assistant_sync::HistoryCommit>` | **A** | |
| 51 | `vault_history_preview` | `assistant_sync::EntryPreview` | **A** | 含 `EntryPreviewField` |
| 52 | `vault_history_restore` | `String`（id） | 标量 | |
| 53 | `vault_conflict_preview` | `assistant_sync::ConflictPreview` | **A** | 嵌套 `EntryPreview` |
| 54 | `vault_conflict_reveal` | `String` | 标量 | 明文值 |
| 55 | `vault_ssh_add_to_agent` | `String`（指纹） | 标量 | |
| 56 | `vault_ssh_remove_from_agent` | `()` | 标量 | |
| 57 | `vault_ssh_list_agent` | `Vec<assistant_security::ssh_agent::SshKeyInfo>` | **A** | |
| 58 | `vault_ssh_auto_add_all` | `Vec<String>` | 标量 | |
| 59 | `vault_ssh_is_agent_running` | `bool` | 标量 | |
| 60 | `vault_ssh_parse_key` | `assistant_security::ssh_agent::SshKeyMeta` | **A** | |
| 61 | `vault_ssh_key_meta` | `assistant_security::ssh_agent::SshKeyMeta` | **A** | |
| 62 | `vault_generate_recovery_key` | `String` | 标量 | 敏感值 |
| 63 | `vault_unlock_with_recovery_key` | `bool` | **D3** | |
| 64 | `vault_has_recovery_key` | `bool` | 标量 | |
| 65 | `vault_verify_vault_meta_integrity` | `IntegrityResult` | dto | 路径仍经 re-export |
| 66 | `vault_disaster_recovery_export` | `DisasterRecoveryResult` | **B** | 定义在 `cmd/vault/recovery.rs` |

**统计（按返回形态，合计 66）**：A 类 13 · dto 8 · B 类 1 · C 类 1 · D3 类 4 · 标量 39。
**另有入参形态（不与返回重复计数）**：A 类入参 2 条（`vault_set_sync_config` / `vault_resolve_conflict`）· B 类入参 2 条（`vault_sync` / `vault_git_remote`）。

---

## 11. 附录 B：wire 类型全表（收编后目标态）

| 目标 DTO | 来源 | 类别 | 传递性依赖 |
|---|---|---|---|
| `SyncResultDto` | `assistant_sync::SyncResult` | A | `ConflictFileDto` / `PendingRemovalsDto` / `UnreadableReasonDto` |
| `SyncConfigDto` | `assistant_sync::SyncConfig` | A | — |
| `PendingRemovalsDto` | `assistant_sync::PendingRemovals` | A | `PendingRemovalEntryDto` |
| `ReadynessDto` | `assistant_sync::Readyness` | A | `RepositoryStateDto` |
| `ConflictFileDto` | `assistant_sync::ConflictFile` | A | — |
| `ConflictResolutionDto` | `assistant_sync::ConflictResolution` | A（入参） | — |
| `HistoryCommitDto` | `assistant_sync::HistoryCommit` | A | — |
| `EntryPreviewDto` | `assistant_sync::EntryPreview` | A | `EntryPreviewFieldDto`；`category` 应为 `String`（见 §4.3） |
| `ConflictPreviewDto` | `assistant_sync::ConflictPreview` | A | `EntryPreviewDto` |
| `SshKeyInfoDto` | `assistant_security::ssh_agent::SshKeyInfo` | A | — |
| `SshKeyMetaDto` | `assistant_security::ssh_agent::SshKeyMeta` | A | — |
| `TotpResultDto` | `assistant_crypto::vault_crypto::totp::TotpResult` | A | `TotpAlgorithmDto` |
| `ParsedOtpAuthDto` | `assistant_crypto::vault_crypto::totp::ParsedOtpAuth` | A | `TotpAlgorithmDto` |
| `DisasterRecoveryResultDto` | `cmd/vault/recovery.rs` 本地 struct | B | — |
| `SyncActionDto` | `cmd/vault/sync.rs` 本地 enum | B（入参） | — |
| `RemoteModeDto` | `cmd/vault/sync.rs` 本地 enum | B（入参） | — |
| `HibpAuditEntryDto` | `Vec<(String, u32)>` | C | — |
| `UnlockOutcomeDto` | 4 条 `Ok(false)` 命令 | D3 | `UnlockFailReasonDto` |
| `VaultErrorDto` | `assistant_crypto::error::VaultError` | D | `VaultErrorCode` |

**不收编**（按 §5 判据②）：`ImportReport`、`ImportSummary`、`SyncError`、`VaultEntrySnapshot`、`VaultEntryImport`、`ImportOutcome`、`VaultCryptoInfo`、`VaultEntryDecrypted`、`DecryptedCustomField`、`AuditEvent`、`EmergencyData`、`EntryFile`、`EntryBlob`、`VaultMetaFile`、`VaultCryptoMeta`、`Argon2Params`、`EmergencyDataFile`、`VaultEntryExport`、`VaultEntryCsvRow`。
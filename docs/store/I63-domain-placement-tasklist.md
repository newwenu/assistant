# I63 — 功能与领域归属修正 任务清单

> 规格：`I63-domain-placement-spec.md`（**唯一事实基线**：判定、文件映射、门禁、验收一律以它为准；本清单只拆「改哪里、验收跑什么」，不重述规格语义）
> 事实基线：规格 §2 的 N1–N24；映射：§5；门禁：§6 的 G1–G6；验收：§10 的 V1–V6
> 关联：`CODE-STRUCTURE.md`（§13 待同步）、`CODE-SPLIT-PRINCIPLE.md`（拆分判据唯一来源）、`I62-protocol-crate-spec.md`（OQ1 顺序约束）

切片与规格对应：T1 → §4.4 R1–R6｜T2 → §4.1 P0-1..P0-5｜T3 → §4.2 P1-1｜T4 → §4.2 P1-2｜T5 → §4.3 P2-1｜T6 → §4.3 P2-2｜T7 → §4.3 P2-3｜T8 → §4.3 P2-4｜T9 → §6-G5 + §13 + 索引重跑

依赖：T1 先行（R5 改行数合计口径，后搬要重算两次）；T2 无前置；T3 依赖 T2（同 crate 编译门禁）；T4 依赖 T2，受 OQ1 约束（**当前 `assistant-protocol` crate 不存在 → 按现路径 `src-tauri/src/protocol/dto/` 执行**）；T5 独立；T6/T7/T8 相互独立；T9 依赖 T1–T8。

> 本清单不管理提交，提交粒度按「可编译性」现场决定；勾选 = 代码已改且对应验收已跑。
> 下文行号均为**改造前现状行号**，只用于定位，不作改动后行号使用。

---

## 阶段 T1 — 索引口径修正（`scripts/structure-index.mjs`）

退出条件：G6 通过；V6 后半（`test_doubles.rs` 角色 + 合计排除）；§4.4 四处口径变化可在索引中定位。

### T1-a R5 测试替身门控（**先做**，会改行数合计）

- [x] `scripts/structure-index.mjs`：`rustItems()` 内新增门控识别——以文件 rel 反推父模块文件（优先 `<dir>/mod.rs`，否则 `<dir>.rs`），读父文件文本，匹配 `#[cfg(any(test, feature = "test-doubles"))]` 之后（允许中间夹其它 `#[...]` 属性）紧跟 `mod <stem>;` → 该文件 `role = "测试替身"` 并置 `testOnly = true`；未命中照旧走 `rsRole()`
- [x] `scripts/structure-index.mjs`：`domainOverview()` 合计只累加 `!testOnly` 项，并新增一行 `**测试替身** N 个文件 / M 行（不计入生产合计）`；子域 token 统计同样排除
- [x] 验收：`node scripts/structure-index.mjs` → `docs/CODE-INDEX.md` 中 `src-tauri/crates/assistant-agent/src/kernel/ports/test_doubles.rs` 角色列 = `测试替身`，`assistant-agent` 概览由 `46/9827` 变为 `45/8935`
  - 现状依据：门控声明在 `assistant-agent/src/kernel/ports/mod.rs:315-316`；被排除的是 `test_doubles.rs` 本体 892 行（非 `mod.rs` 行数）

### T1-b R1 `mod.rs` 按内容判定

- [x] 新增 `isAggregationOnly(text)`：剔除 `//` 与 `/* */` 注释、`#[...]` 属性行、空行后，剩余行**全部**匹配 `^(pub(\([^)]*\))?\s+)?mod\s+\w+[;{]` 或 `^(pub(\([^)]*\))?\s+)?use\s` → 返回真
- [x] `rsRole()` 的 `mod.rs` 分支改为：`isAggregationOnly` → `模块聚合`，否则 `实现（含聚合）`
- [x] 验收：重跑后 `assistant-crypto/src/vault_crypto/mod.rs`(510 行)、`assistant-vault/src/audit/mod.rs`(430 行)、`assistant-tools/src/os/mod.rs`(95 行) 角色为 `实现（含聚合）`；`protocol/dto/chat/mod.rs`、`assistant-vault/src/dto/convert/mod.rs`（仅声明 + 再导出）仍为 `模块聚合`
- [x] 修正预期（按 §4.4-R1 原文）：`protocol/dto/mod.rs`（含 `pub type Millis`）与 `assistant-agent/src/multimodal/mod.rs`（含 struct/const/impl）不是纯聚合 → 同为 `实现（含聚合）`

### T1-c R2 `dto/convert/` 优先匹配

- [x] `RS_ROLE_RULES` 新增 `[/\/dto\/convert\//, "转换实现"]`，**置于** `[/\bdto\.rs$|\/dto\//, "契约 DTO"]` 之前
- [x] 验收：重跑后 `assistant-vault/src/dto/convert/{entry,error,misc,sync}.rs` 与 `convert/mod.rs` 角色 = `转换实现`（`convert/mod.rs` 因纯聚合仍取 `模块聚合`，属 R1 优先）；`assistant-vault/src/dto/{audit,conf,entry,...}.rs` 仍 = `契约 DTO`

### T1-d R3 `types.ts` 运行时判定 + R4 `.d.ts`

- [x] R3：`types?\.ts$` / `Types\.ts$` 两条规则改为内容判定——文件内无 `export (async )?(const|let|function|class)` 且无 `export default (function|class)` → `类型`；否则 `类型与常量`
- [x] R4：新增 `[/\.d\.ts$/, "类型声明"]`，置于所有 `.ts` 规则**之前**（`walk` 谓词 `\.(ts|tsx|css)$` 已覆盖 `.d.ts`）
- [x] 验收：重跑后 `src/i18n/i18next.d.ts`、`src/vite-env.d.ts` = `类型声明`；`src/chat/attachment/types.ts`、`src/mascot/core/types.ts`、`src/stores/agent/observation/types.ts`、`src/settings/core/types/shortcutTypes.ts`、`src/vault/settings/sync/VaultConflictResolve/types.ts` = `类型与常量`

### T1-e 落盘

- [x] `node scripts/structure-index.mjs` 全量重跑；`git --no-pager diff --stat docs/CODE-INDEX.md` 记录变化规模
- [x] G6 判定：§4.4 四处口径变化均可在索引中定位；V6 前半（五条路径消失）留待 T9-c 复核（T1 只改口径、不删文件）
- 说明：本次重跑同时吸收了 I61 存储管理页新增的 17 个前端文件（旧索引已过期），故前端合计由 `482/59422` 变为 `499/62475`，与 T1 口径无关

---

## 阶段 T2 — P0 纯搬迁

退出条件：V1 的三条 `cargo check` 绿；G1 契约无 diff；V5 对应 grep 为空。

### T2-a P0-1 `conversation_utils.rs` → `cmd/conversation/to_dto.rs`

- [x] `git mv src-tauri/src/cmd/conversation_utils.rs src-tauri/src/cmd/conversation/to_dto.rs`（三个 `pub(crate) fn` 与私有助手逐字不动）
- [x] `src-tauri/src/cmd/mod.rs`：删 `pub(crate) mod conversation_utils;`（`:10`）；`mod conversation;`（`:9`）改 `pub(crate) mod conversation;`——`adapters/event_sink.rs` 需跨模块取路径，否则不可达
- [x] `src-tauri/src/cmd/conversation/mod.rs`：加 `pub(crate) mod to_dto;`
- [x] conversation 侧 4 处引用改 `super::to_dto::…`：`branch.rs:3,42,76,133`、`detail.rs:3,125`、`detail_dto.rs:1,14`、`mutate.rs:4,186`
- [x] `src-tauri/src/adapters/event_sink.rs:109,120` 改 `crate::cmd::conversation::to_dto::…`
- [x] 验收：`cargo check -p assistant-app`（cwd = `src-tauri`）；`git --no-pager grep -n "conversation_utils" -- src-tauri --untracked` 为空

### T2-b P0-2 `normalization.rs` / `pdf_text.rs` → `multimodal/`

- [x] `git mv src-tauri/crates/assistant-agent/src/normalization.rs src-tauri/crates/assistant-agent/src/multimodal/normalization.rs`；`pdf_text.rs` 同法
- [x] `src-tauri/crates/assistant-agent/src/lib.rs`：删 `pub mod normalization;`（`:8`）与 `pub mod pdf_text;`（`:9`）
- [x] `src-tauri/crates/assistant-agent/src/multimodal/mod.rs`：加 `mod normalization;`、`mod pdf_text;`（crate 外零引用，无需 `pub`）
- [x] 调用点改 `super::`：`multimodal/admit.rs:106` `crate::normalization::normalize_image` → `super::normalization::normalize_image`；`multimodal/project.rs:143` `crate::pdf_text::extract_or_load` → `super::pdf_text::extract_or_load`
- [x] 验收：`cargo check -p assistant-agent`；`git --no-pager grep -n "crate::normalization\|crate::pdf_text" -- src-tauri --untracked` 为空

### T2-c P0-3 删除 `assistant-security/src/vault_crypto/` 再导出垫片

- [x] `git rm src-tauri/crates/assistant-security/src/vault_crypto/mod.rs`（目录随之消失）
- [x] `src-tauri/crates/assistant-security/src/lib.rs`：删 `pub mod vault_crypto;`（`:7`）
- [x] 4 处 `use crate::vault_crypto::types::SecureString` → `use assistant_crypto::vault_crypto::types::SecureString`：`unlock_dialog/mod.rs:1`、`unlock_dialog/macos.rs:2`、`unlock_dialog/linux.rs:2`、`unlock_dialog/windows/text.rs:8`
- [x] 验收：`cargo check -p assistant-security`；`git --no-pager grep -n "assistant_security::vault_crypto" -- src-tauri --untracked` 与 `git --no-pager grep -n "crate::vault_crypto" -- src-tauri/crates/assistant-security --untracked` 均为空；`assistant-security/Cargo.toml` 依赖不变（跨 crate 边界语义不变）

### T2-d P0-4 `dto/window.rs`

- [x] 新建 `src-tauri/src/protocol/dto/window.rs`：从 `clock.rs:46` 至文件尾原样搬 `WindowModeDto` / `WindowModeChangedPayloadDto`（`#[serde]` / `#[ts]` 属性逐字不动）
- [x] `src-tauri/src/protocol/dto/clock.rs`：只留时钟 3 类型 + `use super::Millis;`
- [x] `src-tauri/src/protocol/dto/mod.rs`：加 `pub mod window;` 与 `pub use window::*;`（按字母序插在 `workspace` 之前）
- [x] 验收：`cargo check -p assistant-app`；`cargo test --test protocol_bindings` 绿（G1 无 diff）

### T2-e P0-5 聊天事件 payload 归位

- [x] `src-tauri/src/protocol/dto/branch.rs`：删 `MessageChangeKindDto`（`:9`）与 `ChatMessagesChangedPayloadDto`（`:38`）；保留 `SiblingInfoDto` + `From<SiblingInfo>`；删随之无用的 `use super::chat::MessageDto;`
- [x] `src-tauri/src/protocol/dto/chat/event.rs`：加 `use super::MessageDto;` 与上述两类型原文
- [x] 验收：`cargo check -p assistant-app`；`cargo test --test protocol_bindings`、`cargo test --test protocol_dto_roundtrip`；`npm run contract:gen` 后 `git diff --exit-code -- src/types/protocol.contract.ts src/vault/core/contract/vault.contract.ts` 为空

### T2-f 阶段收口

- [x] `cargo check -p assistant-app`、`cargo check -p assistant-agent`、`cargo check -p assistant-security`（cwd = `src-tauri`，逐个直接跑）
- [x] `cargo test --test assembly_contract` 仍绿（G4 前置；新断言在 T9-a 才追加）

---

## 阶段 T3 — P1-1 `cmd/vault/audit.rs` 拆 4 文件

退出条件：`cargo test --test assembly_contract` 绿；8 个命令名与 `vault_plugin.rs` 注册项零变化；前端 invoke 名零变化。

### T3-a 共享助手下沉

- [x] `src-tauri/src/cmd/vault/mod.rs`：`unix_now`（原 `audit.rs:26`）下沉为 `pub(super) fn unix_now() -> u64`（组内共享叶子）
- [x] 同文件：`mod audit;`（`:110`）保留，新增 `mod hibp;` / `mod strength;` / `mod totp;`；`pub use` 段（`:120`–`:128`）按同序补 3 行

### T3-b `totp.rs`

- [x] 新建 `src-tauri/src/cmd/vault/totp.rs`：搬 `vault_totp_generate`（`:49`）、`vault_copy_totp`、`vault_totp_parse_uri` 及私有 `load_totp_config`（`:31`）；`unix_now` 改调 `super::unix_now()`
- [x] `use` 只留 TOTP 所需：`assistant_crypto::vault_crypto::totp`、`TotpConfig`、`ParsedOtpAuthDto` / `TotpCodeDto` / `VaultErrorDto`、`AuditEvent` / `audit_log`、`SecurityDbState`、`VaultError`、`tauri::{command, AppHandle}`
- [x] `#[command]` 属性与 `// precondition:` 注释原样保留（ACL 生成与注册门禁依赖它们）

### T3-c `hibp.rs`

- [x] 新建 `src-tauri/src/cmd/vault/hibp.rs`：搬 `vault_hibp_check`、`vault_hibp_audit_cancel`（`:169`）、`vault_hibp_audit`（`:176`）、`HIBP_AUDIT_CANCEL`（`:21`）、`HibpProgress`（`:23`）
- [x] `use` 收敛为：`std::sync::atomic::{AtomicBool, AtomicU32, Ordering}`、`std::sync::Arc`、`serde::Serialize`、`tauri::{command, AppHandle, Emitter}`、`assistant_security::hibp::{HibpClient, SafePassword}`、`HibpAuditEntryDto` / `VaultErrorDto`
- [x] 逐条解密 → 立即 zeroize 的时序、并发取消位语义零改动（只搬不改）

### T3-d `strength.rs` 与 `audit.rs` 收尾

- [x] 新建 `src-tauri/src/cmd/vault/strength.rs`：只放 `vault_check_master_password_strength`（`:268`），`use assistant_vault::{dto::{StrengthResult, VaultErrorDto}, password_strength}`
- [x] `src-tauri/src/cmd/vault/audit.rs`：只留 `vault_query_audit_log`（`:111`）及其所需 `use`；文件头注释改为「审计日志查询」
- [x] 验收：`cargo check -p assistant-app`；`cargo test --test assembly_contract`；`git --no-pager grep -n "vault_totp_generate\|vault_query_audit_log\|vault_hibp_audit\|vault_hibp_audit_cancel\|vault_check_master_password_strength" -- src-tauri/src/vault_plugin.rs` 输出 5 行且行号不变

---

## 阶段 T4 — P1-2 `dto/settings.rs` 拆 5 文件

约束（OQ1）：`assistant-protocol` crate 当前不存在 → 按现路径执行；若期间 I62 落地，改按新路径并在 `I62-protocol-crate-spec.md` §9-T2 注明。

退出条件：`cargo test --test protocol_bindings`、`cargo test --test protocol_dto_roundtrip` 绿；G1 无 diff；类型总数守恒（5 enum + 14 struct + 1 `impl`）。

### T4-a `agent_mode.rs`（3 类型）

- [x] 新建 `src-tauri/src/protocol/dto/agent_mode.rs`：`AgentModeKindDto`（含两个 `impl`）、`AgentModeDto`、`AgentModeInputDto` + `impl AgentModeDto::from_row`
- [x] `use`：`assistant_storage::agent_modes::AgentModeRow`、`serde::{Deserialize, Serialize}`、`ts_rs::TS`、`super::agent::ConversationToolConfigDto`

### T4-b `provider.rs`（10 类型）

- [x] 新建 `src-tauri/src/protocol/dto/provider.rs`：`ReasoningLevelsDto` / `CapabilitySupportDto` / `CapabilitySourceDto` / `ModelCapabilityDto` / `ModelConfigDto` / `ProviderInputDto` / `ModelEffortSupportDto` / `ProviderListDto` / `SettingProviderChangedActionDto` / `SettingProviderChangedPayloadDto`

### T4-c `theme.rs`（2）/ `mascot.rs`（3）/ `shortcut.rs`（1）

- [x] `theme.rs`：`ResolvedThemeDto` / `ThemeChangedPayloadDto`
- [x] `mascot.rs`：`InstallMascotAssetResultDto` / `RiveManifestPropertiesDto` / `RiveManifestDto`（`MascotConfDto` 仍留 `dto/config.rs`，不触碰）
- [x] `shortcut.rs`：`ShortcutWarningPayloadDto`
- [x] 三文件均无内部 crate 依赖，`use` 只需 `serde` + `ts_rs::TS`

### T4-d 模块声明与删除

- [x] `src-tauri/src/protocol/dto/mod.rs`：删 `pub mod settings;` 与 `pub use settings::*;`；加 5 组 `pub mod` / `pub use`（`agent_mode` / `mascot` / `provider` / `shortcut` / `theme`，按字母序）
- [x] `git rm src-tauri/src/protocol/dto/settings.rs`
- [x] 验收：`cargo check -p assistant-app`；`cargo test --test protocol_bindings`（测试递归扫描 `src/protocol/dto` 下 `#[derive(...TS)]`，新文件自动纳入）；`cargo test --test protocol_dto_roundtrip`
- [x] `npm run contract:gen` 后 `git diff --exit-code -- src/types/protocol.contract.ts src/vault/core/contract/vault.contract.ts` 为空
- [x] `git --no-pager grep -n "dto::settings" -- src-tauri --untracked` 为空

---

## 阶段 T5 — P2-1 `os/tools/meta/`

退出条件：`cargo test -p assistant-tools`（含 `registration` / `request_tool` / `pipeline`）绿；既有 `os::agent_*` 路径不变。

### T5-a 迁移

- [x] `git mv src-tauri/crates/assistant-tools/src/os/tools/agent_ask_user.rs src-tauri/crates/assistant-tools/src/os/tools/meta/agent_ask_user.rs`；`agent_request_tool.rs` 同法
- [x] 新建 `src-tauri/crates/assistant-tools/src/os/tools/meta/mod.rs`：只放 `pub mod agent_ask_user;` / `pub mod agent_request_tool;`（不再导出类型，避免与 `os/tools/mod.rs` 重复）
- [x] `src-tauri/crates/assistant-tools/src/os/tools/mod.rs`：删 `pub mod agent_ask_user;`（`:1`）与 `pub mod agent_request_tool;`（`:10`），改为 `mod meta;` + `pub use meta::{agent_ask_user, agent_request_tool};`
- [x] `src-tauri/crates/assistant-tools/src/os/mod.rs`：核对 `pub use tools::{...}` 列表（`:11`–`:12`）两项仍可解析；仅按编译报错调整导入，不改 `:18` / `:40` / `:69` 的调用路径语义
- [x] 验收：`cargo check -p assistant-tools`；`cargo test -p assistant-tools`（`tests/request_tool.rs:7,51,320`、`tests/registration.rs:271` 走 `os::agent_*` 路径，必须仍编译通过）
- [x] 6 个 `register_*` 函数**留在** `os/mod.rs`（门禁不动）

---

## 阶段 T6 — P2-2 `vault/detail/` → `vault/entry/`

退出条件：`npm run typecheck` 通过；vault 相关 vitest 绿；G3 两组依赖方向测试绿。

### T6-a 目录改名 + Drawer/forms 并入

- [x] `git mv src/vault/detail src/vault/entry`
- [x] `git mv src/vault/VaultEntryDrawer.tsx src/vault/entry/VaultEntryDrawer.tsx`
- [x] `git mv src/vault/forms src/vault/entry/forms`（6 个字段编辑器）
- [x] `src/vault/entry/VaultEntryDrawer.tsx`：`./core/...` → `../core/...`（`vaultStore` / `vault.service` / `display/vaultCategory` / `vault.types`），`./forms/*` 不变，`./vault.module.css` 之类根目录相对引用改 `../`
- [x] `src/vault/entry/` 内既有 4 文件（`VaultEntryDetail.tsx` / `VaultEntryHistory.tsx` / `RevealFieldButton.tsx` / `TotpDisplay.tsx`）的 `../core/...` 与 `../vault.module.css` **不改**（目录深度未变）

### T6-b 装配点

- [x] `src/vault/VaultApp.tsx:16` `./VaultEntryDrawer` → `./entry/VaultEntryDrawer`
- [x] `src/vault/VaultApp.tsx:17` `./detail/VaultEntryDetail` → `./entry/VaultEntryDetail`
- [x] 验收：`git --no-pager grep -n "@/vault/detail/\|\./detail/" -- src/vault --untracked` 为空；`npm run typecheck`；`npx vitest run src/vault`；G3 的 `dependencyDirection` / `coreBoundary` 绿

---

## 阶段 T7 — P2-3 删除 `src/audit/` 伪域

退出条件：三个新测试文件绿；`@/audit/` grep 为空；permission 词表与 `permission_store.rs` 的一致性断言仍在。

### T7-a vault 侧

- [x] 新建 `src/vault/core/audit/vaultEvents.ts`：搬 `VAULT_AUDIT_EVENT_TYPES`（`:3`）、`VaultAuditEventType`、`EVENT_TEXT_KEYS`、`vaultAuditEventText` + `lookupKey`（与 permission 侧共用的私有助手各留一份，避免跨域依赖）
- [x] 新建 `src/vault/core/audit/parseDetail.ts`：`git mv src/audit/detail.ts`，导出集合不变（`DETAIL_CLOSED_TAGS` / `DETAIL_EXPORT_FORMATS` / `ParsedAuditDetail` / `parseAuditDetail`）
- [x] `src/vault/settings/security/SecurityLogTab.tsx:14-15`：`@/audit/lexicon` → `../../core/audit/vaultEvents`；`@/audit/detail` → `../../core/audit/parseDetail`（与该文件既有 `../../core/...` 风格一致）
- [x] `src/vault/core/__tests__/parseDetail.test.ts`：`git mv src/audit/__tests__/detail.test.ts`，import 改 `../audit/parseDetail`
- [x] 新建 `src/vault/core/__tests__/vaultEvents.test.ts`：从 `lexicon.test.ts` 拆出 vault 部分——`displayStrings()`（读 `assistant-vault/src/audit/mod.rs` 的 `Display` 块）、事件字典对齐、`vault.security_log.event.*` 双语存在性、已知值不回显、未知值原样兜底

### T7-b permission 侧

- [x] 新建 `src/settings/shared/sections/PermissionAuditSection/permissionOutcomes.ts`：搬 `PERMISSION_AUDIT_OUTCOMES`、`PermissionAuditOutcome`、`OUTCOME_TEXT_KEYS`、`permissionAuditOutcomeText`
- [x] `src/settings/shared/sections/PermissionAuditSection/AuditTable.tsx:3`：`@/audit/lexicon` → `./permissionOutcomes`
- [x] 新建 `src/settings/__tests__/permissionOutcomes.test.ts`：从 `lexicon.test.ts` 拆出 permission 部分——`outcomeStrings()`（读 `src-tauri/src/adapters/permission_store.rs` 的 `outcome_str`）、字典对齐、`settings.agent_mode.audit.outcome.*` 双语存在性、已知/未知值行为

### T7-c 删除与验收

- [x] `git rm src/audit/lexicon.ts src/audit/__tests__/lexicon.test.ts`（`detail.ts` / `detail.test.ts` 已在 T7-a 迁出）
- [x] 验收：`git --no-pager grep -n "@/audit/" -- src --untracked` 为空；`npx vitest run src/vault/core/__tests__/vaultEvents.test.ts src/vault/core/__tests__/parseDetail.test.ts src/settings/__tests__/permissionOutcomes.test.ts` 全绿；`npm run typecheck`；`npm run lint`

---

## 阶段 T8 — P2-4 `paths.ts` 三向拆

退出条件：`paths.test.ts` 两组断言绿；`structure.test.ts` 三条源码断言改写后绿；mascot 不反向依赖 `chat/attachment`。

### T8-a 通用助手

- [x] 新建 `src/services/desktop/assetDirs.ts`：搬 `assetDirsPromise` / `assetDirs()` / `assetUrl()`（`convertFileSrc` + `join` 各一次，保持唯一拼接点），导出 `assetDirs` 与 `assetUrl`
- [x] `use`：`convertFileSrc`、`join`、`type AssetDirsDto`、`storageService`

### T8-b mascot 侧

- [x] 新建 `src/mascot/core/assetUrl.ts`：`mascotAssetUrl(relPath, version?)` + `isLocalAssetUrl(src)`，内部 `await assetDirs()` / `assetUrl(dirs.mascotAssetsDir, relPath)`
- [x] `src/settings/shared/renderers/AssetPickerField.tsx:5`：`@/chat/attachment/paths` → `@/mascot/core/assetUrl`
- [x] `src/mascot/renderers/rive/useRiveManifest.ts:5`：同上

### T8-c attachment 侧

- [x] `src/chat/attachment/paths.ts`：只留 `attachmentUrl` + `openAttachmentFile`（后者仍需 `storageService.openAttachment`）；`attachmentUrl` 改调 `assetUrl(dirs.attachmentsDir, id)`，不得再自行 `join` / `convertFileSrc`
- [x] 验收：`git --no-pager grep -n "convertFileSrc" -- src --untracked` 只命中 `src/services/desktop/assetDirs.ts` 一处

### T8-d 测试迁移

- [x] `src/chat/attachment/__tests__/paths.test.ts`：拆两组 describe——`attachment paths`（`attachmentUrl` / `openAttachmentFile` / `getAssetDirs` 缓存失败重置）与 `mascot asset url`（`mascotAssetUrl` 含 version、`isLocalAssetUrl` 四种输入、缓存只取一次）；mascot 组从 `@/mascot/core/assetUrl` 导入，attachment 组仍 `import("../paths")`
- [x] `src/chat/attachment/__tests__/structure.test.ts`：三条断言改写——「exports only the URL helpers」改为断言 `paths.ts` 含 `attachmentUrl` / `openAttachmentFile` 且**不含** `mascotAssetUrl` / `isLocalAssetUrl`；「keeps exactly one convertFileSrc call」目标文件改 `src/services/desktop/assetDirs.ts`；「takes directory names from the backend」改为断言 `assetDirs.ts` 含 `getAssetDirs` 且无 `"attachments"` 字面量
- [x] 验收：`npx vitest run src/chat/attachment/__tests__/paths.test.ts src/chat/attachment/__tests__/structure.test.ts`；`npm run typecheck`；G3 两组测试绿（mascot → services 单向）

---

## 阶段 T9 — 回流门禁 + 文档同步 + 全量验收

退出条件：V1–V6 全绿；G5 断言落地；§13 六处文档改动完成。

### T9-a 回流门禁（G5）

- [x] `src-tauri/tests/assembly_contract.rs`：沿用 `manifest_dir()` / `src_root()` / `read()` 既有助手，新增一组断言：
  - `protocol/dto/settings.rs` 不存在
  - `protocol/dto/window.rs` 存在
  - `cmd/vault/audit.rs` 内 `#[command]` 计数 == 1 且函数名为 `vault_query_audit_log`
  - `assistant-security/src/vault_crypto` 不存在（路径基于 workspace 根，`manifest_dir().join("../crates/...")`）
  - `assistant-agent/src/normalization.rs` 不存在
- [x] 验收：`cargo test --test assembly_contract`（cwd = `src-tauri`）
- [x] 前端回流以 V5 grep 覆盖，不新增前端测试文件

### T9-b 文档同步（§13）

- [x] `docs/CODE-STRUCTURE.md` §3.2 域表：删 `src/audit/` 行；`src/vault/` 行补 `core/audit/`；`src/settings/` 行补 permission 词表落点
- [x] `docs/CODE-STRUCTURE.md` §3.2 域表 `src/vault/` 行：`detail/` `forms/` → `entry/`（含 `forms/`）
- [x] `docs/CODE-STRUCTURE.md` §8「新增 Agent 工具」：改为「`os/tools/`（文件系统工具）或 `os/tools/meta/`（元工具）+ `src-tauri/src/tool_setup.rs`」
- [x] `docs/CODE-STRUCTURE.md` §8「调整 Mascot 表情」补 `src/mascot/core/assetUrl.ts`
- [x] `docs/CODE-STRUCTURE.md` §8「新增窗口」补 `src-tauri/src/protocol/dto/window.rs`
- [x] `docs/CODE-STRUCTURE.md` §8 新增「新增审计事件类型」→ vault 侧 `src/vault/core/audit/vaultEvents.ts` + `assistant-vault` 审计事件枚举；permission 侧 `PermissionAuditSection/permissionOutcomes.ts` + `src-tauri/src/adapters/permission_store.rs`
- [x] `docs/crate-architecture.md`：`assistant-agent` 根模块表移除 `normalization` / `pdf_text`，注明已入 `multimodal`
- [x] `docs/crate-architecture.md`：`assistant-security` 模块清单删 `vault_crypto` 再导出桩（依赖面说明保留）
- [x] `docs/store/I62-protocol-crate-spec.md` §9-T2：与 P1-2 合并执行则注明 dto 五文件切分；未合并则不改
- [x] `docs/CODE-INDEX.md` 由 T9-c 脚本刷新，**不手改**

### T9-c 索引重跑与全量验收

- [x] `node scripts/structure-index.mjs`（1001 个文件）
- [x] V6：`CODE-INDEX.md` 中 `protocol/dto/settings.rs`、`cmd/conversation_utils.rs`、`assistant-security/src/vault_crypto/mod.rs`、`assistant-agent/src/normalization.rs`、`src/audit/` 五条不再出现；`test_doubles.rs` 角色 = `测试替身` 且不计入合计
- [x] V5：下列 grep 清单逐条为空
- [x] V1：`cargo test --test assembly_contract`（22 passed）、`cargo test --test protocol_bindings`、`cargo test -p assistant-tools`、`cargo test -p assistant-security`、`cargo test -p assistant-agent`（cwd = `src-tauri`，逐个直接跑）
- [x] V2：`npm run contract:gen` 后 `git diff --exit-code -- src/types/protocol.contract.ts src/vault/core/contract/vault.contract.ts src/types/shortcutDefaults.generated.ts` 为空（exit 0）
- [x] V3：`npm run typecheck`、`npm run lint` 通过
- [x] V4：`npx vitest run` 全量（79 文件 / 934 passed / 1 skipped），显式确认 `dependencyDirection`（6）、`coreBoundary`（9，1 skipped）、`panelStructure`（13）、`paths`（8）、`vaultEvents`（4）、`parseDetail`（16）、`permissionOutcomes`（4）七组通过
- [x] G3：`npx vitest run src/stores/agent/observation/__tests__/dependencyDirection.test.ts src/features/__tests__/coreBoundary.test.ts` 绿

---

## 残留 grep 清单（V5，逐条须为空）

```powershell
git --no-pager grep -n "conversation_utils" -- src-tauri --untracked
git --no-pager grep -n "assistant_security::vault_crypto" -- src-tauri --untracked
git --no-pager grep -n "crate::vault_crypto" -- src-tauri/crates/assistant-security --untracked
git --no-pager grep -n "@/audit/" -- src --untracked
git --no-pager grep -n "@/vault/detail/" -- src --untracked
git --no-pager grep -n "crate::normalization" -- src-tauri --untracked
git --no-pager grep -n "crate::pdf_text" -- src-tauri --untracked
```

`--untracked` 不可省：`git grep` 默认只搜已跟踪文件，新文件未 `git add` 时会漏检。

---

## 判据映射（规格 §10 / §6 → 本清单落点）

| 判据 / 门禁 | 落点 |
|---|---|
| V1 | T2-a、T2-f、T3-d、T9-c |
| V2 | T2-e、T4-d、T9-c |
| V3 | T7-c、T8-d、T9-c |
| V4 | T6-b、T7-c、T8-d、T9-c |
| V5 | T2-a、T2-b、T2-c、T7-c、T8-c、T9-c |
| V6 | T1-a、T1-e、T9-c |
| G1 | T2-d、T2-e、T4-d |
| G2 | T9-c |
| G3 | T6-b、T8-d、T9-c |
| G4 | T2-f、T3-d、T9-a |
| G5 | T9-a |
| G6 | T1-e、T9-c |

---

## 实施时需当场确认的开放项（不改规格判定）

- OQ1：`assistant-protocol` crate 尚不存在 → T4 按现路径执行；若期间 I62 落地，T4 改按新路径并同步 I62 §9-T2
- OQ2：`vault_check_master_password_strength` 落 `strength.rs`（暂定，见 T3-d）
- OQ3：前端回流不新建测试文件，用 V5 grep 覆盖（T9-a 末条）
- OQ4：`dto/mascot.rs` 只放资产与运行时类型，`MascotConfDto` 留 `dto/config.rs`（T4-c 不触碰）
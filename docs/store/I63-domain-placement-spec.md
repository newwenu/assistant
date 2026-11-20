# I63 — 功能与领域归属修正规格

| 项 | 值 |
|---|---|
| 状态 | 定稿（判定已复核，待按 §9 切片实施） |
| 日期 | 2026-11-20 |
| 关联 | `CODE-INDEX.md`（逐文件事实源；本文行数为源码非空行口径，与索引的物理行口径不同，见 §2 表头）；`CODE-STRUCTURE.md` §8（改动路由，需按 §13 同步）；`CODE-SPLIT-PRINCIPLE.md`（拆分判据，唯一来源，本文只引用不复述）；I62（协议 crate 抽取，与 §4.2 的 dto 拆分交叠）；I38c §7（工具箱注册门禁，约束 §4.3-P2-1）；`crate-architecture.md` |
| 影响范围 | 前端：1 个顶层域删除（`src/audit/`）、1 个目录重组（`vault/detail` → `vault/entry`）、1 个文件三向拆分（`paths.ts`）；后端：2 个文件拆分（`cmd/vault/audit.rs`、`protocol/dto/settings.rs`）、2 个目录重组（`cmd/vault/`、`os/tools/meta/`）、5 处文件搬迁、1 个垫片删除（`assistant-security/src/vault_crypto`）、1 个索引脚本口径修正 |
| wire 影响 | **零**。不新增、删除、改名任何 DTO 类型或字段，只改变类型所在文件；`src/types/protocol.contract.ts` 与 `src/vault/core/contract/vault.contract.ts` 内容不变 |
| 依赖方向影响 | **零**。无新增跨 crate 依赖，无新增跨层调用；仅消除 1 处 crate 内垫片与 1 处 crate 根目录错位 |

> 一句话：把「文件按谁先写就放哪」改成「文件按唯一变化原因归位」，同时修正索引脚本的失真口径；不碰 wire、不碰依赖方向。

## 1 定位与边界

### 1.1 本规格解决什么

从 `CODE-INDEX.md` 的逐文件事实做归属核对后，问题收敛为三类：

1. **真错位**：功能属于另一个域，但文件放在当前域（按「一个文件一个变化原因」判定，见 `CODE-SPLIT-PRINCIPLE.md`）。
2. **同域多实现**：同一观测形状或同一职责在互不依赖的位置各写一份，长期会漂移（本文仅记录，不合并，理由见 §4.5）。
3. **索引标注失真**：代码本身没错位，是脚本的角色口径与行数口径把事实说错了，导致复核时误判。

### 1.2 本规格不解决什么

- 不改任何 wire 形状、DTO 字段、命令名、事件名、审批键。
- 不重命名 `assistant-crypto` 的 `VaultError`（§11-X1）。
- 不把 `DirStats` 抽成共享 crate（§11-X2）。
- 不重排 `assistant-tools` 的六组注册函数（§11-X3）。
- 不做 `chat/message/avatars`、`stores/attachmentStore.ts` 的迁移（§4.5，判定为不成立）。
- 不把前端常量改成后端生成物（§11-X6，独立议题）。

### 1.3 判据归位

拆分判据（何时该拆、拆到什么粒度、子模块如何命名、依赖方向如何保持单向）**全部**以 `CODE-SPLIT-PRINCIPLE.md` 为唯一来源。本文只做两件事：判定「这个文件属于哪个域」，以及给出落位后的路径与引用改动面。

## 2 现状事实

行数为源码非空行数（不含空行，含内联测试）；`CODE-INDEX.md` 的行数列为物理行（含空行），两者不必相等，复核以文件实际为准。行数仅作初筛，不作拆分判据。

| 编号 | 事实 | 证据 |
|---|---|---|
| N1 | `cmd/vault/audit.rs` 242 行、8 个 `#[command]`：TOTP 3（`vault_totp_generate` / `vault_copy_totp` / `vault_totp_parse_uri`）、HIBP 3（`vault_hibp_check` / `vault_hibp_audit_cancel` / `vault_hibp_audit`）、审计查询 1（`vault_query_audit_log`）、密码强度 1（`vault_check_master_password_strength`）；另有 `unix_now` / `load_totp_config` 两个内部助手 | 逐函数核对 `cmd/vault/audit.rs` |
| N2 | 该文件 8 个命令里只有 1 个与文件名一致；TOTP、HIBP、密码强度三类功能与审计日志查询无共同变化原因 | 同上 |
| N3 | `protocol/dto/settings.rs` 310 行、19 个类型（5 enum + 14 struct）+ 1 个 `impl AgentModeDto::from_row`，跨 5 个互不相关的变化原因：agent mode 3、provider/model 10、theme 2、mascot/rive 3、shortcut 1 | 全文核对 `protocol/dto/settings.rs` |
| N4 | `dto/settings.rs` 唯一的内部 crate 依赖是 `assistant_storage::agent_modes::AgentModeRow`，只服务 `from_row` 桥接 | 文件头部 `use` 与 `from_row` 实现 |
| N5 | `protocol/dto/clock.rs` 46 行、5 类型：时钟 3 + `WindowModeDto` / `WindowModeChangedPayloadDto`；后两者的唯一消费者是 `src-tauri/src/window.rs` | 文件核对 + grep 两类型 |
| N6 | `protocol/dto/branch.rs` 42 行、3 类型：`SiblingInfoDto`（含 `From<assistant_storage::conversations::SiblingInfo>`，只被 `cmd/conversation/branch.rs` 消费）、`MessageChangeKindDto`（只被 `ChatMessagesChangedPayloadDto.change_kind` 消费）、`ChatMessagesChangedPayloadDto`（被 `cmd/conversation/{branch,mutate}.rs`、`adapters/event_sink.rs` 消费） | 文件核对 + grep 三类型消费方 |
| N7 | `protocol/dto/chat/event.rs` 已是聊天事件 payload 桶（13 个 payload/enum，含 `ConversationListChangedPayloadDto`） | 文件核对 |
| N8 | `protocol/dto/mod.rs` 以 `pub use <mod>::*` 平铺 13 个子模块；全 workspace 对 `dto::settings::` / `dto::clock::` / `dto::branch::` 的全限定引用为 **0**，所有消费方都走 `crate::protocol::<Type>` | grep 三个模块名 |
| N9 | `assistant-agent/src/normalization.rs` 123 行、`pdf_text.rs` 81 行；唯一调用方都在 `multimodal/` 内（`admit.rs` 调 `normalize_image`、`project.rs` 调 `extract_or_load`）；crate 外零引用 | 文件核对 + grep `normalize_image` / `pdf_text` |
| N10 | `assistant-tools/src/os/tools/` 的条目里，`agent_ask_user.rs` 495 行与 `agent_request_tool.rs` 536 行不触碰文件系统：前者依赖 `crate::question::QuestionPrompter` 与 `QuestionTimeoutSource`，后者只投影 `ToolUseContext` 已有事实（文件注释明言"不触碰注册表"） | 两文件头部注释与 `use` 列表 |
| N11 | `assistant-tools/src/os/mod.rs` 84 行，承载 6 个 `register_*` 组函数（os / readonly / write / dangerous / interactive / shell），注释写明分组即审批阶段门禁（I38c §7） | `os/mod.rs` 全文 |
| N12 | `assistant-security/src/vault_crypto/mod.rs` 仅 1 行 `pub use assistant_crypto::vault_crypto::*;`；crate 内 4 处 `use crate::vault_crypto::types::SecureString`；crate 外对 `assistant_security::vault_crypto` 的引用为 **0** | 文件核对 + grep |
| N13 | `assistant-crypto/src/error.rs` 的 `VaultError` 在 workspace 命中 **71 文件 / 1214 处** | grep 计数 |
| N14 | `src-tauri/src/cmd/conversation_utils.rs` 196 行、3 个 `pub(crate) fn`（`wire_message_dto` / `messages_with_sibling_counts` / `resolve_context_window_for_conversation`）+ 3 个私有助手；消费方为 `cmd/conversation/{branch,detail,detail_dto,mutate}.rs` 与 `adapters/event_sink.rs` | 文件核对 + grep 消费方 |
| N15 | `src/vault/VaultEntryDrawer.tsx` 322 行与 `src/vault/detail/VaultEntryDetail.tsx` 429 行同属「条目查看/编辑」，却分居根目录与 `detail/`；`forms/` 6 个字段编辑器只被 Drawer 引用；Drawer 与 Detail 的唯一消费者都是 `VaultApp.tsx` | 目录核对 + grep `VaultApp.tsx` |
| N16 | `src/chat/attachment/paths.ts` 60 行、4 个导出：`attachmentUrl`、`mascotAssetUrl`、`isLocalAssetUrl`、`openAttachmentFile`；其中 `mascotAssetUrl` 的消费者是 `settings/shared/renderers/AssetPickerField.tsx`，`isLocalAssetUrl` 的消费者是 `settings/shared/renderers/AssetPickerField.tsx` 与 `mascot/renderers/rive/useRiveManifest.ts`；三者共用 `storageService.getAssetDirs()` | 文件核对 + grep 4 个导出 |
| N17 | `src/audit/lexicon.ts` 111 行同时装两套无关词表：`VAULT_AUDIT_EVENT_TYPES` / `vaultAuditEventText`（消费者 `vault/settings/security/SecurityLogTab.tsx`）与 `PERMISSION_AUDIT_OUTCOMES` / `permissionAuditOutcomeText`（消费者 `settings/shared/sections/PermissionAuditSection/AuditTable.tsx`）；`src/audit/detail.ts` 153 行只服务前者 | 文件核对 + grep 消费者 |
| N18 | `src/audit/` 是只有 2 个文件的顶层"域"，无自身命令、无自身页面，纯属两个消费者的共用工具；其中 `lexicon.test.ts` 断言 permission 词表与 `src-tauri/src/adapters/permission_store.rs` 一致 | 目录核对 + 测试断言 |
| N19 | `src/chat/message/avatars/index.tsx` 137 行被 `chat/message/MessageAvatar.tsx` 与 `settings/pages/agentMode/avatar.tsx` 共用；`settings` 域已有 **7** 处 `@/chat/core/*` 既有依赖 | 文件核对 + grep |
| N20 | `src/stores/` 根目录已有 8 个按域平铺的 store（attachment / history / mcpServer / orb / provider / screenshot / settings / tool），只有 `chat/`、`agent/`、`system/` 三个子目录；`attachmentStore` 的消费者是 `screenshot/Overlay.tsx` 与 chat 侧 | 目录核对 |
| N21 | `DirStats` 在 4 个互不依赖的 crate 各定义一次（`app_dirs/fs_stats.rs`、`assistant-vault/storage_stats.rs`、`assistant-tools/runtime/storage.rs`、`assistant-crash/crash_dump.rs`）；`app_dirs/fs_stats.rs` 文件头已写明不在 crate 间共享的理由 | 4 文件核对 |
| N22 | 索引口径失真：`scripts/structure-index.mjs` 的 `mod.rs → 模块聚合`、`/dto/ → 契约 DTO`、`Types.ts → 类型` 三条规则不看内容；`.d.ts` 无规则（落为"逻辑"）；`#[cfg(any(test, feature = "test-doubles"))]` 门控的 `test_doubles.rs`（801 行）按生产实现计入 | 脚本角色规则段 + `assistant-agent/src/kernel/ports/mod.rs` 的 `test_doubles` 声明 |
| N23 | `mod.rs` 一律标"模块聚合"导致多个承载实现的文件被标错，例：`assistant-crypto/src/vault_crypto/mod.rs` 457 行、`assistant-vault/src/audit/mod.rs` 387 行 | 两文件核对 |
| N24 | `/dto/` 前缀规则把 `assistant-vault/src/dto/convert/{entry,error,misc,sync}.rs` 一并标为"契约 DTO"，实际是 `impl From` 转换实现 | 目录核对 |

## 3 目标结构

### 3.1 后端

```
src-tauri/src/
  cmd/
    conversation/
      to_dto.rs            ← 由 cmd/conversation_utils.rs 迁入并改名
    vault/
      audit.rs             ← 只留 vault_query_audit_log
      totp.rs              ← 新增（TOTP 3 命令 + load_totp_config）
      hibp.rs              ← 新增（HIBP 3 命令 + 取消位 + 进度）
      strength.rs          ← 新增（vault_check_master_password_strength）
      mod.rs               ← unix_now 下沉到此，作为组内共享叶子
  protocol/dto/
    window.rs              ← 新增（WindowModeDto / WindowModeChangedPayloadDto）
    agent_mode.rs          ← 新增（AgentModeKindDto / AgentModeDto / AgentModeInputDto）
    provider.rs            ← 新增（ReasoningLevelsDto 起 10 个类型）
    theme.rs               ← 新增（ResolvedThemeDto / ThemeChangedPayloadDto）
    mascot.rs              ← 新增（InstallMascotAssetResultDto / RiveManifest*）
    shortcut.rs            ← 新增（ShortcutWarningPayloadDto）
    settings.rs            ← 删除
    clock.rs               ← 只留时钟 3 类型
    branch.rs              ← 只留 SiblingInfoDto
    chat/event.rs          ← + ChatMessagesChangedPayloadDto / MessageChangeKindDto

src-tauri/crates/
  assistant-agent/src/multimodal/
    normalization.rs       ← 由 crate 根迁入
    pdf_text.rs            ← 由 crate 根迁入
  assistant-tools/src/os/tools/meta/
    agent_ask_user.rs      ← 由 os/tools 根迁入
    agent_request_tool.rs  ← 由 os/tools 根迁入
    mod.rs                 ← 新增（只做模块声明与再导出）
  assistant-security/src/vault_crypto/  ← 删除（纯再导出垫片）
```

### 3.2 前端

```
src/
  vault/
    entry/                 ← 新增，取代 detail/
      VaultEntryDrawer.tsx
      VaultEntryDetail.tsx
      VaultEntryHistory.tsx
      RevealFieldButton.tsx
      TotpDisplay.tsx
      forms/               ← 6 个字段编辑器整体迁入
    core/audit/            ← 新增，与 core/display、core/slices、core/hooks 并列
      vaultEvents.ts       ← 由 src/audit/lexicon.ts 的 vault 部分拆出
      parseDetail.ts       ← 由 src/audit/detail.ts 迁入
    core/__tests__/
      vaultEvents.test.ts  ← 由 src/audit/__tests__/lexicon.test.ts 的 vault 部分拆出
      parseDetail.test.ts  ← 由 src/audit/__tests__/detail.test.ts 迁入
  settings/
    __tests__/
      permissionOutcomes.test.ts ← 由 src/audit/__tests__/lexicon.test.ts 的 permission 部分拆出
    shared/sections/PermissionAuditSection/
      permissionOutcomes.ts ← 由 src/audit/lexicon.ts 的 permission 部分拆出
  mascot/core/assetUrl.ts  ← mascotAssetUrl / isLocalAssetUrl
  services/desktop/assetDirs.ts ← assetDirs() / assetUrl() 通用助手（与 storageService 同层）
  chat/attachment/paths.ts ← 只留 attachmentUrl / openAttachmentFile
  audit/                   ← 删除
```

### 3.3 不变项

- wire 形状与生成物：类型名、字段、`#[serde]`、`#[ts]` 属性一律不动。
- 依赖方向：`settings → chat/core` 保持既有方向（N19），不新增第三方共用点。
- `stores/` 根目录的按域平铺（N20）保持。
- `DirStats` 四处独立（N21）保持。
- `dto/config.rs` 继续只放配置组声明（`AppConfDto` / `OrbConfDto` / `MascotConfDto` / `ThemeModeDto`）；运行时与资产类 DTO 按 §4.2 落到各自域文件。

## 4 逐项处置

### 4.1 P0 纯搬迁（零语义变化，可独立回滚）

| 项 | 现状 | 处置 | 引用改动 |
|---|---|---|---|
| P0-1 | `cmd/conversation_utils.rs` 在 `cmd/` 根，是 conversation 的 wire 映射层（N14） | 迁入 `cmd/conversation/to_dto.rs`，`cmd/mod.rs` 删声明 | 5 处（4 个 conversation 文件 + `adapters/event_sink.rs`） |
| P0-2 | `normalization.rs` / `pdf_text.rs` 在 `assistant-agent` crate 根，只有 `multimodal/` 消费（N9） | 迁入 `multimodal/`，`lib.rs` 删 2 行 `pub mod`，`multimodal/mod.rs` 加 2 行 `mod` | 2 处调用改 `super::` |
| P0-3 | `assistant-security/src/vault_crypto/` 是 1 行再导出垫片（N12） | 删除目录与 `lib.rs` 声明 | 4 处 `use` 改 `assistant_crypto::vault_crypto::types::SecureString` |
| P0-4 | `dto/clock.rs` 混装窗口形态 DTO（N5） | 窗口两类型落 `dto/window.rs`，`dto/mod.rs` 加 `pub mod window; pub use window::*;` | 0 处（消费方走平铺再导出，N8） |
| P0-5 | `dto/branch.rs` 混装聊天事件 payload（N6） | `ChatMessagesChangedPayloadDto` 与 `MessageChangeKindDto` 落 `dto/chat/event.rs` | 0 处（同上） |

P0-3 的边界说明：`assistant-security` 的 `Cargo.toml` 已依赖 `assistant-crypto`，删除垫片后依赖边不变，只是少了中间层。

### 4.2 P1 文件拆分

| 项 | 现状 | 处置 | 目标形态 |
|---|---|---|---|
| P1-1 | `cmd/vault/audit.rs` 8 命令 / 4 类功能（N1、N2） | 按命令所属功能拆 4 文件 | `audit.rs`(1) + `totp.rs`(3) + `hibp.rs`(3) + `strength.rs`(1)；`unix_now` 下沉 `mod.rs`，`load_totp_config` 随 TOTP 走 |
| P1-2 | `dto/settings.rs` 19 类型 / 5 个变化原因（N3、N4） | 按域拆 5 文件并删除 `settings.rs` | `agent_mode.rs`(3) / `provider.rs`(10) / `theme.rs`(2) / `mascot.rs`(3) / `shortcut.rs`(1)，类型总数守恒 19 |

P1-2 的落位理由：`dto/` 目录的既有约定就是「按域平铺」（`agent.rs`、`mcp.rs`、`orb.rs`、`os_tool.rs`、`screenshot.rs`、`storage.rs`、`workspace.rs`、`config.rs`、`branch.rs`、`clock.rs`），`settings.rs` 是唯一按"页面"命名的桶；拆成域文件是回到既有约定，而不是新立约定（替代方案见 §11-X7）。

P1-2 与 I62 的顺序约束见 §12-OQ1：若 I62 先落地，本项按 `assistant-protocol` crate 的新路径执行，或并入 I62 T2 一次完成。

### 4.3 P2 结构重组

| 项 | 现状 | 处置 | 约束 |
|---|---|---|---|
| P2-1 | `agent_ask_user.rs` / `agent_request_tool.rs` 与文件系统工具混放（N10） | 在 `os/tools/` 下建 `meta/` 分组，两文件迁入；`os/tools/mod.rs` 的 2 行 `pub mod` 改为 `mod meta;` + `pub use meta::{agent_ask_user, agent_request_tool};` | `os/mod.rs` 的 `pub use tools::{...}` 列表随之改写，`crate::os::agent_ask_user` 等既有路径由再导出保持不变；6 个 `register_*` 函数**留在** `os/mod.rs`：分组即审批阶段门禁（I38c §7），迁移会改动门禁文档与 17 处调用，收益不足（§11-X3） |
| P2-2 | 条目查看与编辑分居 `vault/` 根与 `detail/`（N15） | `detail/` 改名 `entry/`，`VaultEntryDrawer.tsx` 与 `forms/` 并入 | 只改 `VaultApp.tsx` 2 行 import + Drawer 内 6 行相对路径 |
| P2-3 | `src/audit/` 是伪域，两个文件各服务不同域（N17、N18） | vault 词表与解析器落 `vault/core/audit/`；permission 词表落 `PermissionAuditSection/`；删除 `src/audit/` | 测试随被断言对象迁移（`detail.test.ts` → vault，`lexicon.test.ts` 拆两半）；`CODE-STRUCTURE.md` 域表同步见 §13 |
| P2-4 | `paths.ts` 同时拼附件 URL 与桌宠素材 URL（N16） | 拆三处：通用助手 `services/desktop/assetDirs.ts`（`getAssetDirs` 缓存 + `assetUrl`）、`mascot/core/assetUrl.ts`（`mascotAssetUrl` + `isLocalAssetUrl`）、`chat/attachment/paths.ts`（`attachmentUrl` + `openAttachmentFile`） | 不能让 `mascot` 反向依赖 `chat/attachment`；通用助手放 `services/desktop/` 与 `storageService` 同层 |

### 4.4 P3 索引口径修正（`scripts/structure-index.mjs`）

| 项 | 现行规则 | 修正后 |
|---|---|---|
| R1 | `mod.rs` → 一律「模块聚合」 | 仅当文件除 `mod` / `pub mod` / `pub use` / 属性外无其它项时标「模块聚合」，否则标「实现（含聚合）」 |
| R2 | 命中 `/dto/` → 一律「契约 DTO」 | `dto/convert/` 优先命中「转换实现」；`dto.rs` 与 `dto/<domain>.rs` 仍为「契约 DTO」 |
| R3 | `Types.ts` / `types.ts` → 「类型」 | 仅当无运行时导出（`const` / `function` / `class`）时标「类型」；否则标「类型与常量」 |
| R4 | `.d.ts` 无规则，落为「逻辑」 | 新增规则置于 `.ts` 通配之前：`.d.ts` → 「类型声明」 |
| R5 | `test_doubles.rs` 按生产实现计入 | 命中 `#[cfg(any(test, feature = ...))]` 门控的模块标「测试替身」，并在领域概览合计里单列，不计入生产行数 |
| R6 | 领域概览合计 | 随 R5 自动变化，不手改 |

R1-R4 只影响「角色」列与「关键导出」列的口径，不改路径与行数；R5 会同时影响行数合计，因此**必须先做**（§9-T1），否则后续搬迁后要重算两次。

### 4.5 判定澄清（复核后不改）

| 项 | 初判 | 复核结论 | 依据 |
|---|---|---|---|
| `chat/message/avatars/` | 被 settings 反向依赖 → 应上移 `shared` | **不成立**。settings → chat/core 已是既有方向（7 处），头像目录上移会新增第三个共用点，反而扩散 | N19 |
| `stores/attachmentStore.ts` | 孤立在 stores 根 | **不成立**。根目录本就按域平铺 8 个 store；该 store 有两个域消费者 | N20 |
| `DirStats` 四处重复 | 应抽共享 crate | **不成立**。四个 crate 互不依赖，抽共享会新增依赖边；`app_dirs/fs_stats.rs` 文件头已写明理由 | N21 |
| `assistant-crypto::VaultError` | 命名与 vault 域冲突，应改名 | **不改**。命中 71 文件 / 1214 处，且 vault 侧已有 `VaultErrorDto`，改名只是把混淆换到另一侧 | N13 |

## 5 迁移映射

| 旧路径 | 新路径 | 引用改动 |
|---|---|---|
| `src-tauri/src/cmd/conversation_utils.rs` | `src-tauri/src/cmd/conversation/to_dto.rs` | `cmd/mod.rs` 删 1 行 `pub(crate) mod conversation_utils;`；`cmd/conversation/mod.rs` 加 1 行 `pub(crate) mod to_dto;`；`cmd/conversation/{branch,detail,detail_dto,mutate}.rs` 与 `adapters/event_sink.rs` 共 5 处 `use` |
| `src-tauri/src/cmd/vault/audit.rs` | `audit.rs` + `totp.rs` + `hibp.rs` + `strength.rs` | `cmd/vault/mod.rs` 加 3 行 `mod` + 3 行 `pub use`（该文件既有风格是 `mod X;` 配 `pub use X::*;`）；命令注册表（`ipc` / ACL 生成物）命令名不变 |
| `src-tauri/src/protocol/dto/settings.rs` | `agent_mode.rs` / `provider.rs` / `theme.rs` / `mascot.rs` / `shortcut.rs` | `dto/mod.rs` 删 2 行（`pub mod settings` 与 `pub use settings::*`），加 10 行（5 组 `pub mod` + `pub use`）；消费方 0 处（N8） |
| `src-tauri/src/protocol/dto/clock.rs`（窗口部分） | `src-tauri/src/protocol/dto/window.rs` | `dto/mod.rs` 加 2 行（`pub mod window` + `pub use window::*`）；消费方 0 处 |
| `src-tauri/src/protocol/dto/branch.rs`（事件部分） | `src-tauri/src/protocol/dto/chat/event.rs` | 0 处（`dto/chat` 内部路径不变，`SiblingInfoDto` 留在 `branch.rs`） |
| `src-tauri/crates/assistant-agent/src/normalization.rs` | `.../src/multimodal/normalization.rs` | `lib.rs` 删 1 行 `pub mod normalization;`；`multimodal/mod.rs` 加 1 行 `mod normalization;`；`multimodal/admit.rs` 1 处 |
| `src-tauri/crates/assistant-agent/src/pdf_text.rs` | `.../src/multimodal/pdf_text.rs` | `lib.rs` 删 1 行 `pub mod pdf_text;`；`multimodal/mod.rs` 加 1 行 `mod pdf_text;`；`multimodal/project.rs` 1 处 |
| `src-tauri/crates/assistant-tools/src/os/tools/agent_ask_user.rs` | `.../os/tools/meta/agent_ask_user.rs` | `os/tools/mod.rs` 2 行改声明 + 1 行 `pub use meta::{...}`；`os/mod.rs` 的 `pub use tools::{...}` 列表 1 处；`register_interactive_tools` 路径由再导出保持不变 |
| `src-tauri/crates/assistant-tools/src/os/tools/agent_request_tool.rs` | `.../os/tools/meta/agent_request_tool.rs` | 同上（`register_readonly_tools` 路径由再导出保持不变） |
| `src-tauri/crates/assistant-security/src/vault_crypto/` | 删除 | `lib.rs` 删 1 行；4 处 `use` |
| `src/vault/VaultEntryDrawer.tsx` + `src/vault/forms/*` | `src/vault/entry/` | `VaultApp.tsx` 2 行；Drawer 内 6 行 |
| `src/vault/detail/*` | `src/vault/entry/*` | `VaultApp.tsx` 1 行；目录内相对路径不变 |
| `src/audit/lexicon.ts`（vault 部分）+ `src/audit/detail.ts` | `src/vault/core/audit/{vaultEvents,parseDetail}.ts` | `SecurityLogTab.tsx` 2 行 |
| `src/audit/lexicon.ts`（permission 部分） | `src/settings/shared/sections/PermissionAuditSection/permissionOutcomes.ts` | `AuditTable.tsx` 1 行 |
| `src/chat/attachment/paths.ts`（素材部分） | `src/mascot/core/assetUrl.ts` | `AssetPickerField.tsx` 1 行、`useRiveManifest.ts` 1 行 |
| `src/chat/attachment/paths.ts`（URL 助手） | `src/services/desktop/assetDirs.ts` | `paths.ts` 与 `assetUrl.ts` 各 1 处 |

## 6 门禁

| 编号 | 门禁 | 判据 |
|---|---|---|
| G1 | wire 零变化 | `npm run contract:gen` 后 `src/types/protocol.contract.ts` 与 `src/vault/core/contract/vault.contract.ts` 无 diff |
| G2 | 搬迁残留为零 | §10-V5 的 grep 清单全部为空 |
| G3 | 依赖方向不变 | `src/stores/agent/observation/__tests__/dependencyDirection.test.ts`、`src/features/__tests__/coreBoundary.test.ts` 全绿 |
| G4 | 注册门禁不变 | `cargo test --test assembly_contract`（cwd = `src-tauri`）通过；命令→权限映射与 feature 挂载断言不受影响 |
| G5 | 回流门禁 | 在 `src-tauri/tests/assembly_contract.rs` 追加路径断言：`protocol/dto/settings.rs` 不存在、`protocol/dto/window.rs` 存在、`cmd/vault/audit.rs` 只含 `vault_query_audit_log`、`assistant-security/src/vault_crypto` 不存在、`assistant-agent/src/normalization.rs` 不存在 |
| G6 | 索引口径 | §4.4 的 R1-R5 生效后重跑脚本，`CODE-INDEX.md` 中对应行按预期变化（§10-V6） |

G5 的前端部分（`src/audit/`、`src/vault/detail/` 不存在）以 G2 的 grep 断言覆盖，不新增前端测试文件。

## 7 前端影响

- 仅 import 路径与目录归属变化，无行为、无样式、无状态语义变化。
- `VaultApp.tsx` 是条目查看/编辑两侧的唯一装配点（N15），改动集中在一个文件。
- `src/audit/` 删除后，`CODE-STRUCTURE.md` §3.2 的域表少一行（§13）。
- 可选功能门控不受影响：vault 侧改动都在 `src/vault/**` 内，仍由 feature 与动态 `import` 隔离。

## 8 成本与收益

### 8.1 改动面

| 侧 | 文件移动/拆分 | 引用改动行 | 风险 |
|---|---|---|---|
| 后端 P0 | 6 个文件 | 约 12 行 | 低（无 wire、无依赖边变化） |
| 后端 P1 | 2 个文件拆成 9 个 | `dto/mod.rs` 与 `cmd/vault/mod.rs` 各数行 | 低-中（P1-2 与 I62 交叠） |
| 后端 P2 | 2 个文件迁目录 | 约 4 行 | 低（注册函数不动） |
| 前端 | 1 域删除、2 目录重组、1 文件三向拆 | 约 16 行 | 低-中（P2-3 测试随之迁移） |
| 脚本 | 5 条规则 | 约 30 行 | 低（只改口径） |

### 8.2 收益

- 复核成本下降：`CODE-INDEX.md` 的角色列不再把实现文件说成聚合、不再把转换实现说成契约、不再把测试替身算成生产代码（N22-N24 三类误判消失）。
- 改动路由变准：新增 TOTP / HIBP / 审计事件时，落点由文件名即可判定，不必先读 242 行混合文件（N1、N2）。
- 拆分的判据变纯：`dto/settings.rs` 的 5 个变化原因分开后，改 provider 不再触碰 mascot 与 shortcut（N3）。
- 假域消失：`src/audit/` 删除后，前端顶层域数与实际功能域数一致（N18）。

### 8.3 风险

- **重复搬迁**：P1-2 与 I62 若并行，`dto/**` 会被搬两次（§12-OQ1 已定顺序约束）。
- **索引读数波动**：R5 让行数合计下降（`test_doubles` 801 行移出生产口径），历史文档中引用过的合计需按 §13 同步。
- **测试迁移漏项**：`lexicon.test.ts` 一拆二，若只搬实现不搬断言，permission 词表与 Rust 的一致性会失去门禁（N18）。

## 9 实施切片

| 切片 | 内容 | 独立可验证 |
|---|---|---|
| T1 | §4.4 的 R1-R5 + 重跑脚本 | `CODE-INDEX.md` 中 `mod.rs` 角色、`dto/convert/*` 角色、`.d.ts` 角色、`test_doubles` 归属四处按预期变化 |
| T2 | §4.1 的 P0-1 至 P0-5（一次提交） | `cargo check -p assistant-app` + `cargo check -p assistant-agent` + `cargo check -p assistant-security` + G1 |
| T3 | P1-1（`cmd/vault/audit.rs` 拆分） | `cargo test --test assembly_contract` + 命令名零变化的 ACL 断言 |
| T4 | P1-2（`dto/settings.rs` 拆分），按 OQ1 顺序 | `cargo test --test protocol_bindings` 与 `protocol_dto_roundtrip` 通过，G1 无 diff |
| T5 | P2-1（`os/tools/meta/`） | `cargo test -p assistant-tools`（含 `registration` / `request_tool` / `pipeline`） |
| T6 | P2-2（`vault/entry/`） | `npm run typecheck` + vault 相关 vitest |
| T7 | P2-3（`src/audit/` 拆解删除） | `src/vault/core/__tests__/{vaultEvents,parseDetail}.test.ts` 与 `src/settings/__tests__/permissionOutcomes.test.ts` 全绿 |
| T8 | P2-4（`paths.ts` 三向拆） | `paths.test.ts` 按新归属拆为两组断言后绿 |
| T9 | G5 门禁 + §13 文档同步 + 最终重跑索引 | 全量判据见 §10 |

切片顺序依据：T1 先行避免两次重算（§4.4-R6）；T2 无 wire 变化可独立回滚；T4 受 OQ1 约束，未定顺序前不启动。

## 10 验收判据

| 编号 | 判据 |
|---|---|
| V1 | `cargo test --test assembly_contract`、`cargo test --test protocol_bindings`、`cargo test -p assistant-tools`、`cargo test -p assistant-security`、`cargo test -p assistant-agent` 全绿（cwd = `src-tauri`，逐个直接跑，不用 `--manifest-path`） |
| V2 | `npm run contract:gen` 后两个生成契约文件 `git diff --exit-code` 为空 |
| V3 | `npm run typecheck` 与 `npm run lint` 通过 |
| V4 | 前端 vitest 全绿，其中 `dependencyDirection`、`coreBoundary`、`panelStructure`、`paths` 与 T7 后新增的 `vaultEvents`、`parseDetail`、`permissionOutcomes` 三组必须显式通过 |
| V5 | 残留 grep 为空：`conversation_utils`、`assistant_security::vault_crypto`、`crate::vault_crypto`（`assistant-security` 内）、`@/audit/`、`@/vault/detail/`、`normalize_image`（经 `crate::normalization` 路径）、`pdf_text::`（经 crate 根路径） |
| V6 | 重跑索引后：`CODE-INDEX.md` 不再出现 `protocol/dto/settings.rs`、`cmd/conversation_utils.rs`、`assistant-security/src/vault_crypto/mod.rs`、`assistant-agent/src/normalization.rs`、`src/audit/` 五条；`test_doubles.rs` 角色为「测试替身」且不计入生产合计 |

## 11 拒绝方案

| 编号 | 方案 | 拒绝理由 |
|---|---|---|
| X1 | 重命名 `assistant-crypto::VaultError` | 1214 处 / 71 文件；且 vault 域已有 `VaultErrorDto`，改名只是把"Vault"标签从通用加密挪回 vault，混淆方向未消除 |
| X2 | `DirStats` 抽到共享 crate | 四个 crate 互不依赖；抽共享新增依赖边，违背单向无环与 crate 独立测试（N21） |
| X3 | 把 6 个 `register_*` 迁出 `os/mod.rs` | 分组即审批阶段门禁（I38c §7），迁移需同步改门禁文档与 17 处调用；`os/mod.rs` 仅 84 行，命名收益不足（N11） |
| X4 | `chat/message/avatars` 上移 `src/shared/` | settings → chat/core 已是既有方向（N19）；上移会新增第三个共用点，扩散而非收敛 |
| X5 | `attachmentStore` 迁入 `stores/chat/` | 根目录已按域平铺 8 个 store，且该 store 有两个域消费者（N20） |
| X6 | 前端常量改为 Rust 生成物 | 需要新开一条 `contract:gen` 通道与校验门禁，属独立议题，不在本规格范围 |
| X7 | `dto/settings.rs` 保留为纯再导出垫片 | 垫片会掩盖下一次误放，且与"删除易过期重复"的既定做法相冲突；`dto/mod.rs` 已承担平铺再导出职责 |

## 12 开放问题

| 编号 | 问题 | 当前处理 |
|---|---|---|
| OQ1 | I62 会把 `src-tauri/src/protocol/dto/**` 整体搬进 `assistant-protocol` crate，与本规格 P1-2 交叠 | 二者不得并行。建议：I62 T2 内顺带完成 P1-2 的 5 文件切分；若 I62 延后，则 P1-2 按现路径先做，I62 再整体搬 |
| OQ2 | `vault_check_master_password_strength` 落 `strength.rs` 还是并入 `lifecycle.rs` | 暂定 `strength.rs`（唯一命令、无生命周期状态）；若 vault 后续增加密码策略类命令，再统一收进 `lifecycle/password.rs` |
| OQ3 | 前端回流门禁是否自动化 | 暂用 G2 的 grep 断言；若要自动化，落点为 `coreBoundary` 既有路径断言组，不新建文件 |
| OQ4 | `dto/mascot.rs` 与 `dto/config.rs` 的 `MascotConfDto` 边界 | 按"config.rs 只放配置组声明、mascot.rs 放资产与运行时"处理；若后续 mascot 配置组膨胀，再合并为 `dto/mascot/{conf,asset}.rs` |

## 13 对既有文档的更新

| 文档 | 位置 | 改动 |
|---|---|---|
| `CODE-STRUCTURE.md` | §3.2 域表 `src/audit/` 行 | 删除该行；词表与解析器归属改在 vault 与 settings 两域内说明（`src/vault/` 行补 `core/audit/`，`src/settings/` 行补 permission 词表落点） |
| `CODE-STRUCTURE.md` | §3.2 域表 `src/vault/` 行 | `detail/` `forms/` 改为 `entry/`（含 `forms/`） |
| `CODE-STRUCTURE.md` | §8 定位索引「新增 Agent 工具」行 | 改为「`assistant-tools` 的 `os/tools/`（文件系统工具）或 `os/tools/meta/`（元工具）+ `src-tauri/src/tool_setup.rs`」 |
| `CODE-STRUCTURE.md` | §8 定位索引「调整 Mascot 表情」行 | 补 `src/mascot/core/assetUrl.ts`（素材 URL 与本地判定） |
| `CODE-STRUCTURE.md` | §8 定位索引「新增窗口」行 | 补 `src-tauri/src/protocol/dto/window.rs` |
| `CODE-STRUCTURE.md` | §8 定位索引 | 新增一行「新增审计事件类型」→ vault 侧 `src/vault/core/audit/vaultEvents.ts` + `assistant-vault` 的审计事件枚举；permission 侧 `PermissionAuditSection/permissionOutcomes.ts` + `src-tauri/src/adapters/permission_store.rs` |
| `CODE-INDEX.md` | 全文 | T9 重跑脚本刷新，不手改 |
| `crate-architecture.md` | `assistant-agent` 模块清单 | 从根模块表移除 `normalization` `pdf_text`，注明已入 `multimodal` |
| `crate-architecture.md` | `assistant-security` 模块清单与依赖面说明 | 删除 `vault_crypto` 再导出桩这一项，依赖面说明保留（`assistant-crypto` 依赖不变） |
| `I62-protocol-crate-spec.md` | §9 T2 | 若与 P1-2 合并执行，在 T2 内注明 dto 五文件切分 |
# I62 — 协议 crate 抽取与依赖边断开 任务清单

> 规格：`I62-protocol-crate-spec.md`（**唯一事实基线**：目标、依赖边三层、桥接归属、门禁、验收一律以它为准；本清单只拆「改哪里、验收跑什么」，不重述规格语义）
> 事实基线：规格 §2 的 N1–N17；桥接归属：§4.4；门禁：§6 的 G1–G8；验收：§10 的 V1–V10
> 关联：`I63-domain-placement-tasklist.md`（P1-2 已落地，`settings.rs` 不复存在）、`crate-architecture.md` §5、`compile-time-attribution.md`

切片与规格对应：T1 → §4.1 + §4.4（源码侧归零）｜T2 → §3 + §4.2 + §4.4 + §4.5（抽取与落位）｜T3 → §4.3 + §5.1 + §5.2｜T4 → §6｜T5 → §13

依赖：**T1 与 T2 必须同一提交**（T1 结束时桥接「已删除未落位」，`cargo check` 必然失败）；T3 依赖 T2（生成测试要能 `-p assistant-protocol` 跑）；T4 依赖 T2/T3；T5 依赖 T4。

> 本清单不管理提交，提交粒度按「可编译性」现场决定；勾选 = 代码已改且对应验收已跑。
> 行号均为**改造前现状行号**，只用于定位，不作改动后行号使用。

**状态（2026-11-21）：T1–T5 全部完成。** 全 workspace `cargo check --workspace --all-targets` 无错误无警告；`npm run contract:verify` 生成物零差异；G1–G7 门禁全绿；桥接实测 61 条按孤儿规则全部落位（agent 24 / tools 16 / storage 9 / providers 8 / config 4）。

---

## 阶段 T1 — 源码侧归零（`src-tauri/src/protocol/dto/**`）

退出条件：§4.4 清单核对完毕；`dto/**` 内 `use assistant_*` 归零、依赖内部 crate 的 `impl From` 归零、`AgentModeDto::from_row` 已移出。

### T1-a 60 条桥接清单（本清单即 T2 的逐条核对依据）

合计 **60** 条依赖内部 crate 的 `impl From`（59 条原在 `dto/**` + 1 条原在 app `cmd/mcp.rs`）+ **1** 个固有方法改造（`AgentModeDto::from_row` → `impl From<&AgentModeRow>`，同样落在桥接文件）。另有 2 条 `From<&str>`（`agent_mode.rs:24` → `AgentModeKindDto`、`agent.rs:57` → `ApprovalLevelDto`）无内部依赖，**留在 DTO crate**。落地后实测桥接总数 **61** 条（60 + 改造后的 1 条）。

**→ `assistant-tools`（16 条 = 15 + app `cmd/mcp.rs` 1 条）**

| # | 源类型 | 目标 DTO | 现状位置 | 备注 |
|---|---|---|---|---|
| 1 | `ToolErrorKind` | `ToolErrorKindDto` | `os_tool.rs:26` | |
| 2 | `assistant_tools::ToolResult` | `OsToolResultDto` | `os_tool.rs:53` | 头内全路径，迁入后改 `crate::ToolResult` |
| 3 | `ToolTier` | `ToolTierDto` | `agent.rs:26` | |
| 4 | `ToolTierDto` | `ToolTier` | `agent.rs:36` | **反向**，`for` 侧定归属 |
| 5 | `ApprovalLevelDto` | `assistant_tools::ApprovalLevel` | `agent.rs:68` | **反向**，头内全路径 |
| 6 | `assistant_tools::ToolDescriptor` | `ToolDescriptorDto` | `agent.rs:120` | 头内全路径 |
| 7 | `assistant_tools::compact::TruncatedUnit` | `TruncatedUnitDto` | `agent.rs:223` | 头内全路径 |
| 8 | `assistant_tools::compact::ToolResultCompact` | `ToolResultCompactDto` | `agent.rs:234` | 头内全路径 |
| 9 | `PermissionIntent` | `PermissionIntentDto` | `agent.rs:364` | |
| 10 | `BehaviorHints` | `ToolBehaviorDto` | `agent.rs:382` | |
| 11 | `ApprovalPreview` | `ApprovalPreviewDto` | `agent.rs:423` | |
| 12 | `ApprovalExtensionSeverity` | `ApprovalExtensionSeverityDto` | `agent.rs:451` | |
| 13 | `ApprovalExtension` | `ApprovalExtensionDto` | `agent.rs:461` | |
| 14 | `ApprovalReason` | `ApprovalReasonDto` | `agent.rs:472` | |
| 15 | `ApprovalContext` | `ApprovalContextDto` | `agent.rs:488` | |
| 61 | `assistant_tools::mcp::McpServerStatus` | `McpServerStatusDto` | app `cmd/mcp.rs`（原地实现） | **原不在 `dto/**`**，T2-c 一并迁入本 crate 的 `dto_bridge.rs` |

**→ `assistant-providers`（8 条）**

| # | 源类型 | 目标 DTO | 现状位置 |
|---|---|---|---|
| 16 | `ProviderErrorCode`（`base`） | `CommandErrorCodeDto` | `error.rs:56` |
| 17 | `ProviderError`（`base`） | `CommandErrorDto` | `error.rs:114` |
| 18 | `TokenUsage`（`types::chat_delta`） | `TokenUsageDto` | `chat/usage.rs:21` |
| 19 | `RetryNotification`（`types::chat_delta`） | `RetryNotificationDto` | `chat/event.rs:21` |
| 20 | `ProviderErrorPayload`（`base`） | `ProviderErrorPayloadDto` | `chat/event.rs:49` |
| 21 | `ImageAttachmentRef`（`types`） | `ImageAttachmentRefDto` | `chat/content.rs:15` |
| 22 | `FileAttachmentRef`（`types`） | `FileAttachmentRefDto` | `chat/content.rs:36` |
| 23 | `ContentBlock`（`types`） | `ContentBlockDto` | `chat/content.rs:75` |

**→ `assistant-storage`（8 条 + 1 个改造）**

| # | 源类型 | 目标 DTO | 现状位置 |
|---|---|---|---|
| 24 | `OccupancySummary`（`token_usage`） | `TokenUsageOccupancyDto` | `chat/usage.rs:102` |
| 25 | `CumulativeSummary`（`token_usage`） | `TokenUsageCumulativeDto` | `chat/usage.rs:118` |
| 26 | `CacheStats`（`token_usage`） | `TokenUsageCacheDto` | `chat/usage.rs:128` |
| 27 | `TokenUsageSummary`（`token_usage`） | `TokenUsageSummaryDto` | `chat/usage.rs:138` |
| 28 | `MessageRow`（`conversations`） | `MessageDto` | `chat/message.rs:82` |
| 29 | `MessageRow`（`conversations`） | `MessageRecordDto` | `chat/message.rs:138` |
| 30 | `ConversationRow`（`conversations`） | `ConversationDto` | `chat/conversation.rs:35` |
| 31 | `SiblingInfo`（`conversations`） | `SiblingInfoDto` | `branch.rs:13` |
| 32 | `&AgentModeRow`（`agent_modes`） | `AgentModeDto` | `agent_mode.rs:58` 固有 `from_row` → 改造为 `impl From<&AgentModeRow>`（§4.5） |

**→ `assistant-config`（4 条）**

| # | 源类型 | 目标 DTO | 现状位置 |
|---|---|---|---|
| 33 | `app_conf::ThemeMode` | `ThemeModeDto` | `config.rs:13` |
| 34 | `app_conf::AppConf` | `AppConfDto` | `config.rs:51` |
| 35 | `orb_conf::OrbConf` | `OrbConfDto` | `config.rs:93` |
| 36 | `mascot_conf::MascotConf` | `MascotConfDto` | `config.rs:122` |

**→ `assistant-agent`（24 条）**

| # | 源类型 | 目标 DTO | 现状位置 |
|---|---|---|---|
| 37 | `kernel::TurnPhase` | `TurnPhaseDto` | `chat/event.rs:110` |
| 38 | `kernel::SessionState` | `SessionStateDto` | `chat/event.rs:140` |
| 39 | `ToolCompact`（`agent::tool_event`） | `ToolResultCompactDto` | `agent.rs:183` |
| 40 | `CompactUnit`（`agent::tool_event`） | `TruncatedUnitDto` | `agent.rs:212` |
| 41 | `ToolErrorClass`（`agent::round_executor`） | `ToolErrorKindDto` | `agent.rs:315` |
| 42 | `ToolEventPayload`（`agent::tool_event`） | `AgentToolCallPayloadDto` | `agent.rs:327` |
| 43 | `AgentNode`（`agent::node_event`） | `AgentNodeDto` | `agent.rs:675` |
| 44 | `NodeState`（`agent::node_event`） | `NodeStateDto` | `agent.rs:707` |
| 45 | `ToolStage`（`agent::round_executor`） | `PipelineStageDto` | `agent.rs:732` |
| 46 | `BrakeVerdict`（`agent::node_event`） | `BrakeVerdictDto` | `agent.rs:751` |
| 47 | `MiddlewareAction`（`agent::node_event`） | `MiddlewareActionDto` | `agent.rs:767` |
| 48 | `BudgetRemaining`（`agent::node_event`） | `BudgetRemainingDto` | `agent.rs:785` |
| 49 | `BrakeDetail`（`agent::node_event`） | `BrakeDetailDto` | `agent.rs:803` |
| 50 | `LadderDetail`（`agent::node_event`） | `LadderDetailDto` | `agent.rs:821` |
| 51 | `ModelDetail`（`agent::node_event`） | `ModelDetailDto` | `agent.rs:842` |
| 52 | `PipelineDetail`（`agent::node_event`） | `PipelineDetailDto` | `agent.rs:866` |
| 53 | `BudgetSplitDetail`（`agent::node_event`） | `BudgetSplitDetailDto` | `agent.rs:886` |
| 54 | `BranchDetail`（`agent::node_event`） | `BranchDetailDto` | `agent.rs:904` |
| 55 | `PersistDetail`（`agent::node_event`） | `PersistDetailDto` | `agent.rs:921` |
| 56 | `MiddlewareVerdict`（`agent::node_event`） | `MiddlewareVerdictDto` | `agent.rs:938` |
| 57 | `AfterToolDetail`（`agent::node_event`） | `AfterToolDetailDto` | `agent.rs:954` |
| 58 | `AfterIterationDetail`（`agent::node_event`） | `AfterIterationDetailDto` | `agent.rs:969` |
| 59 | `AgentNodeDetail`（`agent::node_event`） | `AgentNodeDetailDto` | `agent.rs:993` |
| 60 | `AgentNodeEvent`（`agent::node_event`） | `AgentNodeEventDto` | `agent.rs:1027` |

> 行号合计 61 行而 `dto/**` 内实际 59 条 `impl From`：第 32 行是固有方法改造，第 61 行原在 app `cmd/mcp.rs`（第 60 行的 `AgentNodeEvent` 计入 59 之内，agent 侧 2 + 22 = 24）。
> **落地实测**（`Select-String 'impl From<'` 计数）：agent 24 / tools 16 / storage 9 / providers 8 / config 4 = **61**，与清单逐条一致。

### T1-b 删除 `dto/**` 的 13 行 `use assistant_*`

- [x] `os_tool.rs:1` 删 `use assistant_tools::ToolErrorKind;`
- [x] `error.rs:1` 删 `use assistant_providers::base::{ProviderError, ProviderErrorCode};`
- [x] `config.rs` 无 `use assistant_*`（全路径写在 impl 头内），无需删
- [x] `chat/usage.rs:1` 删 `use assistant_providers::types::chat_delta::TokenUsage;`
- [x] `chat/message.rs:1` 删 `use assistant_storage::conversations::MessageRow;`
- [x] `chat/event.rs:1-2` 删 `use assistant_providers::base::ProviderErrorPayload;` 与 `use assistant_providers::types::chat_delta::RetryNotification;`
- [x] `chat/conversation.rs:1` 删 `use assistant_storage::conversations::ConversationRow;`
- [x] `chat/content.rs` 无 `use assistant_*`（全路径写在 impl 头内），无需删
- [x] `branch.rs:1` 删 `use assistant_storage::conversations::SiblingInfo;`
- [x] `agent_mode.rs:1` 删 `use assistant_storage::agent_modes::AgentModeRow;`
- [x] `agent.rs:1-14` 删 4 段 `use assistant_agent::…` / `use assistant_tools::{…}`
- [x] 验收：`git --no-pager grep -n "^use assistant_" -- src-tauri/src/protocol/dto --untracked` 为空（原目录已整体迁出，`src/protocol/dto` 不存在）

### T1-c 移出固有方法 `AgentModeDto::from_row`

- [x] `agent_mode.rs:58` 起整块 `pub fn from_row(row: &AgentModeRow) -> Self { … }` 摘出（含其语义注释），函数体留给 T2-d 作为 `impl From<&AgentModeRow>` 的 `from` 体
- [x] 验收：`dto/**` 内不再出现 `AgentModeRow`（`assistant-protocol/src` 实测 0 命中）

### T1-d 保留项核对

- [x] `AgentModeKindDto::as_str`（`agent_mode.rs:16`）、`ApprovalLevelDto::From<&str>`（`agent.rs:57`）、`CommandErrorCodeDto::is_retryable` / `CommandErrorDto::{new,internal,with_retry_after_ms}` / `MessageStatusDto::from_db` / `ConversationQueryModeDto::as_str` 全部**原地不动**（固有 impl 必须与 DTO 同 crate，§4.5）

---

## 阶段 T2 — crate 抽取 + 桥接归位（与 T1 同一提交）

退出条件：V1、V7、V9 绿；`cargo check` 全 workspace 通过；垫片使 app 侧零改动。

### T2-a 目录搬迁

- [x] `git mv src-tauri/src/protocol/dto src-tauri/crates/assistant-protocol/src`
- [x] `git mv …/src/mod.rs …/src/lib.rs`（`pub mod` / `pub use` / `pub type Millis` 原样保留）
- [x] `chat/mod.rs` 的 `pub use` 与 `pub mod` 不动
- [x] 验收：`git --no-pager status` 显示 24 个 rename

### T2-b 新建 crate 骨架

- [x] `crates/assistant-protocol/Cargo.toml`：白名单只有 `serde` / `serde_json` / `ts-rs`（`serde-json-impl` + `no-serde-warnings`，§4.2）
- [x] `src-tauri/Cargo.toml` 的 `[workspace] members` 加 `"crates/assistant-protocol"`
- [x] app `Cargo.toml`：新增 `assistant-protocol = { path = "crates/assistant-protocol" }`，移除 `ts-rs`（N13；`ts-rs` 改为 `[workspace.dependencies]` 声明）

### T2-c 5 个 `dto_bridge.rs`

- [x] `assistant-agent/src/dto_bridge.rs`：24 条（#37–#60），`use assistant_protocol::*;` + 本 crate 域类型 `use crate::agent::node_event::{…}` 等
- [x] `assistant-tools/src/dto_bridge.rs`：16 条（#1–#15 + #61），头内 `assistant_tools::X` 改写为 `crate::X`
- [x] `assistant-providers/src/dto_bridge.rs`：8 条（#16–#23），头内 `assistant_providers::X` 改写为 `crate::X`
- [x] `assistant-storage/src/dto_bridge.rs`：8 条（#24–#31）+ `impl From<&AgentModeRow> for AgentModeDto`（#32）
- [x] `assistant-config/src/dto_bridge.rs`：4 条（#33–#36），头内 `assistant_config::X` 改写为 `crate::X`
- [x] 每个 crate 的 `lib.rs` 加 `mod dto_bridge;`（不 `pub use`，trait impl 无需导出）
- [x] 每个 crate 的 `Cargo.toml` 加 `assistant-protocol` 依赖
- [x] 验收：`impl From<` 各 bridge 计数与上表一致（实测 24 / 16 / 9 / 8 / 4 = 61）

### T2-d 调用点改写

- [x] `src-tauri/src/cmd/agent_mode.rs`：2 处 `AgentModeDto::from_row(row)` → `AgentModeDto::from(row)`（`row_to_dto` 与一处 `.map`）
- [x] 验收：`git --no-pager grep -n "from_row" -- src-tauri/src --untracked` 为空（实测 0 命中）

### T2-e 垫片

- [x] `src-tauri/src/protocol/mod.rs` 重写为：
  - `pub use assistant_protocol as dto;`
  - `pub use assistant_protocol::*;`
  - `pub mod events;` + `pub use events::*;`
- [x] 验收：`git --no-pager diff --stat -- src-tauri/src` 除 `protocol/mod.rs` 与 `cmd/agent_mode.rs` 外无 app 侧改动（V7；另含 2 处注释路径修正，无逻辑改动）

### T2-f 编译收口

- [x] `cargo check -p assistant-protocol`（cwd = `src-tauri`）
- [x] `cargo check -p assistant-agent`、`-p assistant-tools`、`-p assistant-providers`、`-p assistant-storage`、`-p assistant-config`（逐个直接跑）
- [x] `cargo check -p assistant-app`
- [x] `cargo tree -p assistant-protocol` 无 `assistant-*`（V1）
- [x] `cargo test -p assistant-agent -p assistant-tools -p assistant-storage -p assistant-providers -p assistant-config`（V9）

---

## 阶段 T3 — 生成器拆分

退出条件：V2、V3、V10 绿；两个生成物字节不变。

### T3-a DTO 生成测试迁入

- [x] `git mv src-tauri/tests/protocol_bindings.rs src-tauri/crates/assistant-protocol/tests/protocol_bindings.rs`
- [x] `use assistant_app::protocol::{…}` → `use assistant_protocol::{…}`（165 个类型名单不变）
- [x] 扫源码根 `src/protocol/dto` → `src`（crate 自身源码根，§4.3）
- [x] 输出路径 `CARGO_MANIFEST_DIR/../src/types/…` → `../../../src/types/protocol.contract.ts`
- [x] G3 断言消息里的 `src/protocol/dto` 文案改为 crate 源码根
- [x] 验收：`cargo test -p assistant-protocol --test protocol_bindings` 绿

### T3-b `shortcutDefaults` 生成独立

- [x] 新建 `crates/assistant-config/tests/shortcut_defaults_bindings.rs`：只读 `DEFAULT_SHORTCUT_PATTERNS`，写 `../../../src/types/shortcutDefaults.generated.ts`（§5.2）
- [x] 验收：`cargo test -p assistant-config --test shortcut_defaults_bindings` 绿

### T3-c 脚本三段

- [x] `package.json`：`contract:gen` = `contract:gen:vault && contract:gen:protocol && contract:gen:shortcuts`
- [x] 三段均前置 `cd src-tauri &&`（§5.1）
- [x] 验收：`npm run contract:gen`；`git --no-pager diff --exit-code -- src/types/protocol.contract.ts src/types/shortcutDefaults.generated.ts`（V4；实测总 wall ≈ 2.1s，三段无差异）

### T3-d 集成测试保位

- [x] `tests/protocol_contract.rs` / `tests/protocol_dto_roundtrip.rs` 零改动（靠垫片解析，N12）
- [x] 验收：`cargo test --test protocol_contract --test protocol_dto_roundtrip`（V10）

---

## 阶段 T4 — 门禁收口

退出条件：G1–G7 落地；注入违规样例时能报错（V5）。

- [x] 新增 `crates/assistant-protocol/tests/no_internal_deps.rs`
  - G1：扫 `src/**` 非注释行，零 `assistant_` 路径（排除 `assistant_protocol`，不扫 `tests/**`），失败打印 `文件:行号`
  - G1 注释例外：跳过 `//` 与 `///` 起始行（3 处，§4.1）
  - G2：解析 `Cargo.toml`，白名单外依赖即失败
  - G4：`#[ts(export)]` 不得出现
- [x] `src-tauri/tests/assembly_contract.rs`：成员清单加 `assistant-protocol`；新增 G6 **双向**桥接归属断言
- [x] `scripts/contract-check.mjs`：确认 2 文件 + 1 目录哈希条目仍对齐（G5）
- [x] 验收：临时加一行 `use assistant_config::…` 到 crate 源码 → G1 失败；删掉 → 通过（V5；实测已验证）

---

## 阶段 T5 — 文档与基线

- [x] `crate-architecture.md` §5 该行：24 文件 / 2,969 行 / 165 DTO，结论改「已抽为 `assistant-protocol`」，附 N16 四条判据全中
- [x] `compile-time-attribution.md` 第 6 条：修正「ts-rs 仅服务 vault DTO」；补 `contract:gen` 三段口径与实测 wall time（V3）
- [x] `ARCHITECTURE.md` crate 清单补 `assistant-protocol` 与依赖白名单
- [x] `docs/CODE-INDEX.md` 由 `node scripts/structure-index.mjs` 刷新，**不手改**

---

## 残留 grep 清单（逐条须为空）

```powershell
git --no-pager grep -n "^use assistant_" -- src-tauri/src/protocol/dto --untracked
git --no-pager grep -n "AgentModeRow" -- src-tauri/crates/assistant-protocol --untracked
git --no-pager grep -n "from_row" -- src-tauri/src --untracked
git --no-pager grep -n "assistant_app::protocol" -- src-tauri/crates --untracked
git --no-pager grep -n "ts_rs" -- src-tauri/src --untracked
```

`--untracked` 不可省：`git grep` 默认只搜已跟踪文件，新 crate 未 `git add` 时会漏检。

> **2026-11-21 实测**：5 条全部为空（`src/protocol/dto` 目录已不存在；`AgentModeRow` / `from_row` / `assistant_app::protocol` / `ts_rs` 均 0 命中）。

---

## 判据映射（规格 §10 / §6 → 本清单落点）

| 判据 / 门禁 | 落点 |
|---|---|
| V1 | T2-f |
| V2 | T3-a |
| V3 | T3-c、T5 |
| V4 | T3-c |
| V5 | T4 |
| V6 | T2-f、T3-c |
| V7 | T2-e |
| V8 | T3-c |
| V9 | T2-f |
| V10 | T3-d |
| G1 | T1-b、T4 |
| G2 | T2-b、T4 |
| G3 | T3-a、T4 |
| G4 | T4 |
| G5 | T4 |
| G6 | T2-c、T4 |
| G7 | T4 |

---

## 实施时需当场确认的开放项（不改规格判定）

- OQ1：`ts-rs` 是否同时提升到 `[workspace.dependencies]`；不阻塞本清单，默认沿用 app/vault 现有写法
  - **已决**：提升为 `[workspace.dependencies]`，由 `assistant-protocol` 与 `assistant-vault` 各自 `workspace = true` 引用；app 包不再直接声明。
- OQ2：`dto_bridge.rs` 的域类型导入方式——优先复用原 `use` 行并只把本 crate 前缀改为 `crate::`；若某类型在本 crate 根未再导出，则改用完整路径
  - **已决**：按此执行；`assistant-tools` / `assistant-providers` / `assistant-config` 三处头内全路径统一改写为 `crate::X`，其余保留原 `use`。
- OQ3：`MessageDto::from(MessageRow)` 若调用了 `MessageStatusDto::from_db`（`pub`，跨 crate 可调用）无需改造；若涉及私有项，需先把该私有项提升为 `pub`
  - **已决**：`MessageStatusDto::from_db` 为 `pub`，桥接无需改造。
- OQ4：`TruncatedUnitDto` / `ToolResultCompactDto` / `ToolErrorKindDto` 各有两个来源（agent 与 tools），两处 impl 分别落在各自 crate，互不冲突
  - **已决**：分别落在 `assistant-agent` 与 `assistant-tools` 的 `dto_bridge.rs`，G6 双向归属断言已覆盖该情形。
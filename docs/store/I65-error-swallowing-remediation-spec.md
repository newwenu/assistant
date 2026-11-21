# I65 错误吞没整改规格

| 项 | 值 |
|---|---|
| 状态 | 草案（现状事实已核实，待排期） |
| 日期 | 2026-11-20 |
| 事实来源 | `docs/achievement/error-swallowing-analysis.md`（检索快照与分组分析；已移至待归档目录） |
| 判定依据 | `docs/analysis/PROJECT-DESIGN-GUIDELINES.md` §4「错误处理」 |
| 相邻规格 | `docs/achievement/I64-storage-vault-integration-audit-spec.md`（其 EXP-4 的配置 fail-open 议题不在本规格范围，见 OQ-2；已移至待归档目录） |
| 影响范围 | `src/`、`src-tauri/`、`eslint.config.ts`、`scripts/check.mjs`、新增 `scripts/check-error-swallow.mjs` |
| 契约影响 | `VaultErrorCode` 扩展，需 `npm run contract:gen` 并提交 `src/vault/core/contract/vault.contract.ts` |
| 前端影响 | Store 新增错误状态字段、失败提示与新增 i18n 键 |

> 定位：把「已经会被吞掉的错误」逐个判定为**必须可感知 / 必须可追溯 / 明确容许**，并用机检门禁把新的吞没挡在门外。

---

## 1 定位与边界

### 1.1 解决什么

| # | 目标 | 完成判据 |
|---|---|---|
| 1 | 审计与持久化链路不再存在「错误丢失且无痕迹」的点 | §5.1 全部关闭，V1/V5 通过 |
| 2 | 消除前端 `.catch(() => {})` 与裸 `console.*` 两类硬规则违规 | V2/V3 通过 |
| 3 | 「加载失败」与「数据为空」在 Store 状态上可区分 | V4 通过 |
| 4 | 剩余 `let _ =` / `.ok()` 每一处都有归类：修复或标注 `best-effort` | V5/V6 通过 |
| 5 | 门禁生效，新吞没无法合入 | V7 通过 |

### 1.2 不解决什么

- 不重构错误类型体系（不引入 anyhow / thiserror 统一改造，不改六层防御分层）。
- 不改 Rust 崩溃包、审计 JSONL、结构化错误 DTO 的存储与 wire 格式（除 T2 的枚举扩展）。
- 不处理已判定为 P3「显式容错」的路径（解析降级、平台 FFI 尽力而为、清理动作、遥测），判定口径见分析文档 §1.4。
- 不把「日志级别不够」一律升级成用户提示（见 X2）。

### 1.3 与预先分析的差异（以本规格为准）

分析文档是 2026-11-20 的静态检索快照，部分结论已被后续重构消解。行号一律以本规格 §3 为准。

| 分析条目 | 分析结论 | 当前代码事实 | 处置 |
|---|---|---|---|
| P0-1 流式落库 `let _ = db_tx.send` | 缺口 | `cmd/chat/stream.rs` 已不存在；改为 adapter 模式（`kernel/turn/persist.rs`、`event_sink.rs`、`message_store.rs`），`persist_terminal` 返回 `Result` 且内容写失败也要写终态 | 失效，关闭 |
| P0-2 32 处 `let _ = audit_log` | 缺口 | `audit/jsonl.rs` 的 `log_dual` 返回 `()`，内部对 DB/JSONL 失败仅 `warn`；调用方已不再丢弃返回值 | 基本失效，仅剩 1 处（T1） |
| P0-3 错误语义替换 | 缺口 | 仍存在，位置移到 `dto/convert/error.rs`（`Database`/`Io`/`Clipboard`/`AlreadyUnlocked`/`Other` 全部塌缩为 `Internal`） | 保留，拆为 T2 |
| P0-3 附带：`open_dek` AEAD 失败 → `InvalidPassword` | 缺口 | 仍在 `vault_crypto/mod.rs:132`（长度不符的 `VaultCorrupted` 在 `:137`）；AEAD 失败在信息论上无法区分「密码错」与「密文损坏」 | 不可消除，改为 UI 文案补偿（T2-c） |
| P1 前端静默吞没 5 处 | 缺口 | 5 处全部仍在 | 保留（T3） |
| P1-3 `MessageBubble.tsx:76` 复制失败无反馈 | 缺口 | 已修复：`MessageActions.copyTextToClipboard` 返回布尔，`onCopy` 布尔约定决定是否显示「已复制」（I46 §4.2） | 失效，关闭 |
| P2-1 `FileCard.tsx:19` / `ImageThumbnail.tsx:75` 裸 `console.warn` | 缺口 | 已不存在，`src/chat/message/` 下无 `console.*` | 失效，关闭 |
| P1-4 剪贴板还原丢弃返回值 | 缺口 | 仍在 `clipboard_guard.rs:147/150`，分析中的 78/80 行号已过期 | 保留（T6） |
| P1-5 分支定位 `.ok()` | 缺口 | 仍在 `tree_ops.rs:162/171-172/175/186-187/190`；另 `:123` 的 `unwrap_or(false)` 会把「查询失败」误判为「分支不存在」并改写指针（静默数据损坏） | 保留（T7） |
| P1-9 settingsStore / SettingsGroup 仅 `log.error` | 缺口 | 仍在；且 `useSettingMessage` 已提供未被使用的 `showFailed()` | 保留（T5） |
| P1-10 落库失败日志缺上下文 | 缺口 | 仍在：`kernel/turn/runner.rs:140` 为 `log::error!("terminal persist failed: {err}")`，未带会话 / 消息 id | 保留（T1c） |
| 其余 P1/P2 | 缺口 | 行号漂移，结论成立 | 保留，§5 重列 |

---

## 2 分类与判定口径

- 吞没分类（S1–S6）、后果等级（P0–P3）、吞没与降级的判定原则：唯一来源为 `docs/achievement/error-swallowing-analysis.md` §1.2 / §1.3 / §1.4（待归档目录），本规格不复制。
- 本规格在执行层面把 §1.4 展开为四条可操作口径（D1）：

| 路径性质 | 失败处理要求 |
|---|---|
| 持久化写入、审计追加、安全判定（解锁 / 解密 / 权限 / 自动锁）、写后一致性 | 不可降级：必须留下可追溯痕迹，且用户可感知或状态可判定 |
| 解析 / 展示 / 清理 / 遥测 | 可降级：必须 `log.warn` 且带对象标识（id / key / 条目名） |
| 用户主动触发的动作 | 失败必须可见（`message.error` 或 Store 错误字段） |
| 后台自动动作（自动锁、预取、剪贴板还原、托盘唤起） | 至少 `log.warn`；若后果对用户可见（未锁定、内容未还原、窗口未出现）则必须可见 |

---

## 3 现状事实（2026-11-20 核实）

### 3.1 前端

| # | 位置 | 事实 |
|---|---|---|
| N1 | `src/vault/core/slices/vaultLifecycleSlice.ts:272` | 自动锁定时器 `void get().lock("autoLock").catch(() => {})`；`lock` 内部已 `log.error` 并置 `error/errorCode`，此处仅吞掉 rejection |
| N2 | `src/stores/chat/chatStorePorts.ts:227` | `chatService.incrementGeneration(convId).catch(() => {})`，代际推进失败无任何痕迹 |
| N3 | `src/chat/core/chatUtils.ts:66` | token 用量拉取 `.catch(() => {})`，无日志无反馈 |
| N4 | `src/history/detail/HistoryBranchRow.tsx:82` | 复制记录 JSON 到剪贴板失败 `.catch(() => {})`，用户主动动作无反馈 |
| N5 | `src/chat/ConversationView.tsx:95` | `void prefetchAttachmentLimits().catch(() => {})`，附件限制预取失败静默 |
| N6 | `src/settings/pages/storage/`：`entryView.ts:90`、`cleanupFlow.ts:186/227/237`、`residualView.ts:181`、`lexicon.ts:117` | 6 处裸 `console.warn`（枚举未知值兜底，属防御分支但违反硬规则 2；`reclaimFlow.ts` 已重命名为 `cleanupFlow.ts`） |
| N7 | `src/services/stream/transport.ts:175` | 1 处 `console.debug` 由 `import.meta.env.DEV` 门控（折叠器观测，可接受但应走 logger）；分析文档的「7 处」把本条并入 N6，两者级别不同 |
| N8 | `src/stores/toolStore.ts:71/94/112/122/133` | 5 处收藏 / 最近使用失败走 `log.debug`，默认生产级别（WARN）不可见 |
| N9 | `src/stores/historyStore.ts`（`selectConversation` / `refreshMessages` / `loadConversations`） | 失败仅 `log.error`，无错误状态字段，UI 与「空列表」不可区分 |
| N10 | `src/stores/settingsStore.ts`（`loadConf`） | 失败仅 `log.error`，设置页继续展示默认值，用户无感 |
| N11 | `src/stores/providerStore.ts`（`loadProviders`） | 失败仅 `log.error`，provider 列表退化为空 |
| N12 | `src/vault/core/slices/vaultSyncSlice.ts`（6 个 `load*` 读取动作：config / remoteStatus / readyness / conflicts / pendingRemovals / lastTime） | 失败仅 `log.error`；`remoteStatus` / `syncConflicts` 被置空，`syncConfig` / `pendingRemovals` / `syncLastAt` 保留旧值，均无失败标记（写动作 `saveSyncConfig` 已走 `failWith`，不受影响） |
| N12b | `src/vault/core/slices/vaultSyncSlice.ts:487/508/516/525`（`sshListAgent` / `sshCheckAgentRunning` / `sshParseKey` / `sshKeyMeta`） | 失败仅 `log.error`：前两者状态保持旧值（首次调用则为空列表 / `undefined`），后两者 `return null` 把「解析失败」与「无结果」压成同一返回 |
| N13 | `src/settings/shared/sections/SettingsGroup.tsx:119`、`:139` | 两处写失败（`handleChange` / `handleIntermediateChangeComplete`）仅 `log.error("patch failed for ${def.key}")`；同文件的 `useSettingMessage` 已提供 `showFailed(raw, key?)` 但未调用；`optimisticConfUpdate` 无回滚，失败后界面仍显示未落库的值 |
| N14 | `eslint.config.ts` | 无 `no-console`、无 `.catch(() => {})` 的 `no-restricted-syntax` 选择器；`no-empty` 由 `js.configs.recommended` 提供（块内含注释即通过），门禁实际为零 |

### 3.2 Rust

| # | 位置 | 事实 |
|---|---|---|
| N15 | `src-tauri/crates/assistant-vault/src/security_db/biometric_ops.rs:151` | `let _ = state.with_conn(log_dual);` 丢弃审计连接失败（全仓唯一残留的 `audit_log` 丢弃点） |
| N16 | `src-tauri/crates/assistant-vault/src/dto/convert/error.rs:10/16/20/21/38` | `AlreadyUnlocked`/`Database`/`Clipboard`/`Io`/`Other` 一律映射为 `VaultErrorCode::Internal`，前端无法区分 DB 故障与逻辑 bug |
| N17 | `src-tauri/crates/assistant-crypto/src/vault_crypto/mod.rs:132` | `open_dek` 的 AEAD 解密失败统一映射 `InvalidPassword`（`:132`）；长度不符则 `VaultCorrupted`（`:137`） |
| N18 | `src-tauri/crates/assistant-security/src/clipboard_guard.rs:129/147/150` | 后台线程 `let _ = thread::spawn`（句柄丢弃，P3）+ `let _ = backend.write`（`:147`）/ `let _ = backend.clear`（`:150`）（还原失败无痕迹） |
| N19 | `src-tauri/crates/assistant-storage/src/conversations/tree_ops.rs:162/171-172/175/186-187/190`，另 `:123` | `find_new_active_leaf` 用 `.ok()` / `.unwrap_or(false)` 把查询错误降级为 `None` / `false`，「无分支」与「查询失败」不可区分；`:123` 的 `still_exists` 判空会把查询失败当成「分支已不存在」，进而把 `active_branch_id` 置 NULL（静默数据损坏） |
| N20 | `src-tauri/crates/assistant-sync/src/git_sync/remote.rs:138` | `let _ = git::git_config_branch_set_remote(...)`，写 remote 失败静默 |
| N21 | `src-tauri/src/tray.rs:110/113/127/131/135/150` | 6 处 `let _ = window::show_or_create(...)`，唤起窗口失败无日志 |
| N22 | `src-tauri/src/setup.rs:218-220`、`:230` | `AppConf::load()` 失败直接 `return`（全局快捷键整体不注册且无日志）；`let _ = gs.unregister_all()` 在 `:230` |
| N23 | `src-tauri/crates/assistant-crash/src/crash_dump.rs:140/154` | `let _ = fs::write(&log_path, ...)`，崩溃路径写日志失败无输出（`prepare_dir` 失败已 `eprintln!`） |
| N24 | `src-tauri/crates/assistant-providers/src/openai/discovery.rs:16/49` | 条目被跳过时逐条 `continue`，无计数、无汇总，全部跳过时表现为「模型清单为空」 |
| N25 | `src-tauri/crates/assistant-agent/src/kernel/turn/runner.rs:140` | 终态落库失败 `log::error!("terminal persist failed: {err}")`，未带 `conversation_id` / `message_id`，无法定位是哪条消息 |
| N26 | `src-tauri/src/cmd/compaction.rs:390-392`、`:403` | 压缩收尾：`let Ok(conn) = get_db_conn(...) else { return }` 无日志直接返回；`update_message_content` 用 `let _ =` 丢弃结果（同函数内 token usage 写入失败已 `log::error`，风格不一致） |
| N27 | `src-tauri/src/menu.rs:15`、`src-tauri/src/window.rs:216` | 菜单项 / 内部流程唤起主窗口失败无日志（与 N21 同类，分析未覆盖） |
| N28 | `src-tauri/src/cmd/app.rs:17/23`、`src-tauri/src/window.rs:573`、`src-tauri/src/setup.rs:337`、`src-tauri/src/cmd/orb.rs:119/124/128/129/161/185/197/204` | 退出与拖拽过程中的窗口几何 / Orb 位置 / 配置落盘失败静默（偏好丢失，用户下次启动才发现）；`setup.rs` 另有 `:187/:191`（`window::toggle`）与 `:274/:276/:288/:333`（`set_position`），`window.rs` 另有 `:210/:568/:579/:588`，同类处理 |

### 3.3 全仓 `let _ =` 落点概览（不逐条列举）

`src-tauri/src` 共 60 处 / 21 文件，`src-tauri/crates`（含 tests、examples）共 218 处 / 61 文件。逐条整改不现实，按性质分类处置：

| 类别 | 典型落点 | 处置 |
|---|---|---|
| 平台 FFI 返回值（Win32 / DWM / 枚举窗口） | `src/screenshot/**`、`assistant-security/src/unlock_dialog/windows/**`、`screen_lock/windows.rs`、`assistant-tools/src/sandbox/**`、`biometric/{windows,linux}.rs` | P3 显式容错：目录级白名单 + `best-effort` 注释 |
| 清理 / 销毁 / 事件发射 | `tool_setup.rs`、`cmd/mascot_asset.rs`、`mcp/transport.rs`、`window.rs:210/579/588`、`cmd/tool.rs:10`、`cmd/vault/audit.rs:225`、`cmd/vault/lifecycle.rs:119` | P3：白名单 + 注释（前端已断开、进程将退出等） |
| 窗口唤起与偏好落盘 | 见 N21 / N27 / N28 | P2：T11 补 `warn`，不得阻断退出流程 |
| 测试 / 示例代码 | `tests/**`、`examples/**` | 不纳入门禁 |
| 真缺口（必须修） | N15（T1）、N18（T6）、N19（T7）、N20（T8）、N23（T11）、N26（T8b） | 按对应 T 项处理 |

---

## 4 决议

| # | 决议 | 说明 |
|---|---|---|
| D1 | 判定按路径性质，不按「有没有 catch」 | 见 §2 四口径表 |
| D2 | 复用现有基础设施，不新建错误层 | `createLogger`、`useSettingMessage`、`vaultErrorMessage`、`CommandErrorDto`、`audit log_dual` 均为既有能力 |
| D3 | 审计与持久化失败不得阻断业务，但必须留二级痕迹 | 保持 `log_dual` 返回 `()`；失败时降级到另一通道，双通道都失败升 `error` 级并计数，避免刷屏 |
| D4 | Store 读取失败必须有显式状态，禁止与空数据同形 | 每个读取动作配对应的 `xxxError` 字段（成功置 `null`，失败置错误文本） |
| D5 | 后台补偿动作失败必须 `warn` 且带对象标识 | 剪贴板还原、托盘唤起、分支定位等 |
| D6 | 门禁优先于人工约定 | ESLint 规则 + 脚本检索，见 §6 |
| D7 | 确实不改的丢弃点必须显式标注 `best-effort:` 注释 | 供脚本白名单识别，避免「看起来像吞没」 |

---

## 5 整改切片

### 5.1 P0：审计与错误语义

**T1 审计连接失败不得丢弃**

| 项 | 内容 |
|---|---|
| 位置 | `src-tauri/crates/assistant-vault/src/security_db/biometric_ops.rs:151` |
| 现状 | `let _ = state.with_conn(log_dual);` |
| 要求 | 改为显式 `if let Err(e) = state.with_conn(log_dual) { log::warn!(...) }`，warn 内容包含事件动作与 `state.with_conn` 失败原因；`log_dual` 自身签名与语义不变（仍返回 `()`，禁止调用方 `?`） |
| 验收 | V5：全仓无 `let _ = ... log_dual`；`cargo test -p assistant-vault` 通过 |

**T1b 审计双通道失败的二级痕迹**

| 项 | 内容 |
|---|---|
| 位置 | `src-tauri/crates/assistant-vault/src/audit/jsonl.rs`（`log_dual`） |
| 现状 | 签名 `log_dual(conn: Option<&Connection>, writer: Option<&AuditJsonlWriter>, event, detail)`，返回 `()`；DB 与 JSONL 任一失败仅 `warn`；两者都失败时无区分度，也无聚合信号 |
| 要求 | 三种结局分开记录：单通道失败（`warn`，带失败通道名 `db` / `jsonl`）、**双通道失败**（`error`，带事件类型与动作）、JSONL 追加失败不影响 DB 结果判定；注意两通道均为 `Option`，「双通道失败」只在两者都为 `Some` 且都失败时成立（只有一个通道时该通道失败即为全部失败，仍按单通道 `warn` 处理）；双通道失败用进程内 `AtomicU64` 计数器聚合，首次与每满 N 次（N=100）各输出一条汇总，避免每次审计都刷屏 |
| 验收 | 新增单测：注入必失败的 DB sink 与 JSONL sink，断言（1）业务调用返回成功，（2）双通道失败时输出 `error` 级日志，（3）连续 N 次失败只产生 1 条汇总日志 |

**T1c 终态落库失败日志补上下文**

| 项 | 内容 |
|---|---|
| 位置 | `src-tauri/crates/assistant-agent/src/kernel/turn/runner.rs:140` |
| 现状 | `log::error!("terminal persist failed: {err}")` |
| 要求 | 日志带 `conversation_id`、`turn_id`、`message_id`（该作用域内 `conversation_id` 与 `ctx.turn_id` / `ctx.message_id` 均可用，`PersistCtx` 也已持有），并保留 `TurnStatus::Failed` 与 `TurnError` 置位的既有行为；不得把错误吞成 `warn`（该路径属持久化，D1 要求 `error`） |
| 验收 | V6：日志字段完整；`cargo test -p assistant-agent` 通过 |

**T2 错误语义：`VaultErrorCode` 塌缩为 `Internal`**

| 项 | 内容 |
|---|---|
| 位置 | `src-tauri/crates/assistant-vault/src/dto/error.rs`、`src-tauri/crates/assistant-vault/src/dto/convert/error.rs:9/16/20/21/38` |
| 现状 | `Database` / `Clipboard` / `Io` / `Other` / `AlreadyUnlocked` 全部映射为 `Internal` |
| 要求（T2-a） | 枚举新增 `Database`、`Io`、`Clipboard`、`AlreadyUnlocked` 四码，一一映射；`Other(_)` 保持 `Internal`（未知即内部）；`#[serde(rename_all = "camelCase")]` 不变 |
| 要求（T2-b） | 前端 `src/vault/core/display/vaultErrorText.ts` 的 `ERROR_TEXT_KEYS`（`as const satisfies Record<VaultErrorCode, I18nPath>`）补四个键，并在 `src/i18n/locales/{zh-CN,en-US}.json` 的 `vault.error` 下补文案；动作提示（`needsRepositoryReset` / `needsDbRepair` / `needsAppUpgrade`）不扩：`Database` 归「可重试」，不得引导重置仓库 |
| 要求（T2-b2） | `src/vault/core/display/vaultError.ts` 的 `VAULT_ERROR_CODE_FLAGS` 镜像表必须同步加四个键（`as const satisfies Record<VaultErrorCode, true>` 会在漏加时编译报错；漏加则 `isVaultErrorDto` 拒收，前端会退化成直接展示后端 message） |
| 要求（T2-c） | `open_dek` 的 AEAD 失败 → `InvalidPassword` 保持不变，但在该处加注释说明信息论限制；解锁失败文案改为「密码错误，或 vault 数据已损坏」，禁止在文案中承诺「就是密码错」 |
| 契约 | 改后 `npm run contract:gen`，提交 `src/vault/core/contract/vault.contract.ts`；前端码表与契约同源，禁止手写副本 |
| 验收 | V1：DB 故障单测断言 code 为 `Database`；前端两张码表（`ERROR_TEXT_KEYS` / `VAULT_ERROR_CODE_FLAGS`）因 `satisfies` 在漏项时编译失败，`npm run typecheck` 即覆盖；`npm run contract:verify` 通过 |

### 5.2 P1：用户无感知的功能失败

**T3 前端 5 处静默吞没**

| 位置 | 改法 | 用户可见性 |
|---|---|---|
| `vaultLifecycleSlice.ts:272` | `.catch((e) => log.warn("autoLock failed", e))`；不重复置 `error`（`lock` 内部已置，避免二次提示） | 由 `lock` 内部状态承载 |
| `chatStorePorts.ts:227` | `.catch((e) => log.warn("incrementGeneration failed", convId, e))` | 否（一致性副作用） |
| `chatUtils.ts:66` | `.catch((e) => log.warn("fetchTokenUsage failed", conversationId, e))`；本文件目前无 logger，需新增 `createLogger("chatUtils")` | 否（展示型降级） |
| `HistoryBranchRow.tsx:82` | 改为 `navigator.clipboard.writeText(recordJson).then(...).catch(...)`：失败 `toast.error(t("common.actions.copy_failed"))` + `log.warn`，成功复用 `common.actions.copied`（两键均已存在）。注意本文件 `message` 已是行数据变量，antd 提示须另取名（`const { message: toast } = App.useApp()`），并新增 `createLogger("historyBranchRow")`。**不**引入 `@/chat/message/MessageActions`（跨域耦合且会带入 UI 组件依赖） | 是（用户主动动作） |
| `ConversationView.tsx:95` | `.catch((e) => log.debug("prefetchAttachmentLimits failed", e))` | 否（预取） |

验收：V2；日志必须带会话 id / 消息 id 或组件上下文（如 `[storage] key`）。

**T4 Store 读取失败与空数据可区分**

| 位置 | 新增状态 | UI 要求 |
|---|---|---|
| `historyStore.ts` | `messagesError`（`selectConversation` / `refreshMessages`） | 历史详情在 `messagesError` 非空时渲染失败占位（含重试入口），不得渲染成「没有记录」 |
| `chatConversationSlice.ts` | `conversationsError`（`loadConversations` 实际落点；`historyStore.loadConversations` 只是转调 `updateConversations`，不另设字段） | 会话列表为空且 `conversationsError` 非空时显示失败占位 |
| `settingsStore.ts` | `confErrors: Partial<Record<ConfGroup, string>>` | 设置分组加载失败时该组显示失败占位，不得静默展示默认值 |
| `providerStore.ts` | `providersError` | provider 列表为空且 `providersError` 非空时显示失败占位 |
| `vaultSyncSlice.ts` | 6 个 `load*` 各自失败标记（或统一的 `syncLoadErrors`）；`sshAgentKeys` / `sshAgentRunning` 补 `sshLoadErrors`；`sshParseKey` / `sshKeyMeta` 改为抛错或返回 `{ ok: false }`，不得用 `null` 兼表两种语义 | 同步页在失败标记非空时显示「数据可能已过期 + 重试」，禁止只保留旧值 |

统一约束：成功路径必须把对应字段置 `null`；错误文本用既有 `xxxErrorMessage` 转换函数，禁止直接展示原始 `String(e)`；新增 i18n 键（中/英各一份），命名沿用 `*.load_failed` 形态。

验收：V4；每个读取动作有「成功置空 / 失败置值」的单测。

**T5 设置写入失败必须可见且不留假值**

| 项 | 内容 |
|---|---|
| 位置 | `src/settings/shared/sections/SettingsGroup.tsx:118-119`、`:138-139` |
| 要求 | 两处 catch 保留 `log.error`，并调用 `msg.showFailed(e)`（`msg` 即本文件已有的 `useSettingMessage()` 返回值，默认键 `common.actions.save_failed_desc` 已存在，无需新增）；同时回滚乐观值：`await useSettingsStore.getState().loadConf(group)`（`group` 已是本文件入参，需从 `@/stores/settingsStore` 引入；也可在 `useConfGroup.ts` 里加一个 `reloadConf(group)` 封装，二选一，不得两处各写一套） |
| 边界 | `SettingDefinition.saveMessage === false` 只影响成功提示，不影响失败提示（`useSettingMessage.showFailed` 的注释已写明该约定）；回滚后若 `loadConf` 本身失败，按 T4 的 `confErrors` 显示分组占位，不得再弹一次失败提示 |
| 不在本项范围 | 读取失败由 T4 的 `settingsStore.confErrors` + 分组占位承载 |
| 验收 | V4：断开后端后修改任一设置项，应出现失败提示且界面值回到落库值 |

**T6 剪贴板自动还原失败**

| 项 | 内容 |
|---|---|
| 位置 | `src-tauri/crates/assistant-security/src/clipboard_guard.rs:147/150` |
| 要求 | 后台线程内三种结局各一条日志：还原成功（`debug`，带分支 `write`/`clear` 与耗时）、因密码已变更而跳过还原（`debug`，`should_clear == false` 是正常路径，不是失败）、还原/清空失败（`warn`，带底层错误）；`thread::spawn` 的句柄丢弃保留并标注 `best-effort` |
| 禁止 | 失败后重试（见 X4） |
| 验收 | V6：该文件不再存在无注释的 `let _ = backend.*` |

**T7 分支定位失败与「无分支」区分**

| 项 | 内容 |
|---|---|
| 位置 | `src-tauri/crates/assistant-storage/src/conversations/tree_ops.rs:162/171-172/175/186-187/190`（`find_new_active_leaf` 内 5 处），另 `:123`（删除路径 `still_exists`） |
| 要求 | `find_new_active_leaf` 返回 `Result<Option<String>>`，内部 3 处 `.ok().flatten()` 与 2 处 `.unwrap_or(false)` 一并改为 `?`；查询错误向上传播为 `CommandError`；仅「确实无分支」返回 `Ok(None)` 并写 NULL。删除路径上不得因查询失败而把 `active_branch_id` 置空（当前 `.unwrap_or(false)` 会把「查询失败」误判成「分支已不存在」进而改写指针，必须一并修） |
| 验收 | V5：无 `.ok()` 用于分支定位；新增单测覆盖「查询失败」与「无分支」两条路径行为不同 |

**T8 Git remote 写入失败**

| 项 | 内容 |
|---|---|
| 位置 | `src-tauri/crates/assistant-sync/src/git_sync/remote.rs:138`（`add_remote_url` 内） |
| 现状 | `if let Ok(ref branch) = git_current_branch(..) { let _ = git_config_branch_set_remote(..) }` —— 分支查询失败静默跳过，写 remote 失败静默丢弃；而 `add_remote_url` 本身返回 `Result<(), SyncError>` |
| 要求 | 写 `branch.<name>.remote` 失败必须 `?` 传播（该函数已有错误通道）；`git_current_branch` 失败（detached HEAD 等）可接受跳过，但必须 `warn!` 说明「未绑定上游」；两类失败不得合并成同一句日志 |
| 验收 | V5 |

**T8b 压缩摘要回写失败（P1，分析后补）**

| 项 | 内容 |
|---|---|
| 位置 | `src-tauri/src/cmd/compaction.rs:390-392`、`:403` |
| 现状 | 连接获取失败静默 `return`；`update_message_content` 结果丢弃 |
| 要求 | 连接获取失败 `log::error!`（带 `msg_id`）后返回；回写改为 `if let Err(e) = ... { log::error!("compaction persist failed", ...) }`，带 `msg_id` 与内容长度；保持「失败不阻断压缩流程」的既有语义（压缩结果仍返回给前端），但落库失败必须可追溯 |
| 验收 | V6：日志字段完整；`cargo test` 通过 |

### 5.3 P2：非关键路径与门禁

**T9 日志级别修正**

| 项 | 内容 |
|---|---|
| 位置 | `src/stores/toolStore.ts:71/94/112`（有 localStorage 兜底）、`:122/133`（无兜底） |
| 要求 | 全部 `log.debug` → `log.warn`：71/94/112 失败后已降级 localStorage，warn 文案须点明「已降级 localStorage」；122 失败后 `recentTools` 保持旧值（首次为空），文案带 `limit`；133 为乐观更新后的持久化失败，文案带 `toolId`。若失败导致面板空白，可按 T4 补 `error` 字段（可选，不强制） |
| 验收 | V6 |

**T10 裸 console 收口 + ESLint 门禁**

| 项 | 内容 |
|---|---|
| 位置 | 见 N6 / N7 |
| 要求（前端） | storage 页 6 处 `console.warn`（`entryView.ts:90`、`cleanupFlow.ts:186/227/237`、`lexicon.ts:117`、`residualView.ts:181`）改为 `createLogger("storage")` 的 `warn`；`transport.ts:175` 改为 `log.debug`，但**保留** `import.meta.env.DEV` 门控（该处是逐 delta 的热路径，模板串含 `JSON.stringify`，logger 的参数是即时求值的，去掉门控会在生产环境白做字符串拼接） |
| 要求（门禁） | `eslint.config.ts` 新增：<br>1. `no-console: "error"`，作用域 `src/**/*.ts`、`src/**/*.tsx`；在其后追加一个 `files: ["src/services/system/logger.ts"], rules: { "no-console": "off" }` 的块（ESLint 后置块覆盖前置块，顺序不能反）；<br>2. `no-restricted-syntax` 增加两条选择器：`CallExpression[callee.property.name="catch"] > ArrowFunctionExpression[body.type="BlockStatement"][body.body.length=0]`（`.catch(() => {})`）与 `CallExpression[callee.property.name="catch"] > ArrowFunctionExpression[body.type="Literal"][body.raw="null"]`（`.catch(() => null)`），消息统一为「禁止静默吞没：至少 `log.warn`，确属容错写 `best-effort:` 注释」；<br>3. 空 `catch {}` 由 `js.configs.recommended` 已启用的 `no-empty` 覆盖，不再额外开 `no-empty-function`（会误伤合法空回调，如 `onClick: () => {}`） |
| 实现注意 | 现有配置共 5 处声明 `no-restricted-syntax`（`src/**` 1 处、`BARREL_DOMAINS` 展开后 3 处、`INDEX_FILE_GUARD` 1 处），ESLint 对同一文件的规则数组是**整体覆盖**而非合并，因此吞没选择器必须抽成共享常量（如 `SWALLOW_SELECTORS`）并 `...` 展开进每一处，否则 `src/history/**`、`src/mascot/**`、`src/vault/**` 与所有 `index.ts` 会因被后置块覆盖而漏检 |
| 边界 | 测试目录当前无 `console.*`，无需豁免（现有 `**/__tests__/**` 的 `ignores` 只作用于 `no-deprecated` 那一个块，不构成全局豁免，勿误用）；`scripts/**` 不受前端规则影响；storage 页的纯函数模块可直接 `createLogger`（logger 在非 Tauri 环境自动降级为 console 输出，无需 mock）；空 `catch {}` 由已启用的 `no-empty` 覆盖，必须写 `best-effort:` 注释说明 |
| 验收 | V7：人为添加 `.catch(() => {})` 后 `npm run lint` 必须失败 |

**T11 后台与启动路径**

| 位置 | 要求 |
|---|---|
| `src-tauri/src/tray.rs:110/113/127/131/135/150` | 6 处改为 `if let Err(e) = window::show_or_create(...) { warn!(...) }`，带窗口标签 |
| `src-tauri/src/menu.rs:15`、`src-tauri/src/window.rs:216` | 同上（`warn` 带窗口标签与触发来源） |
| `src-tauri/src/cmd/app.rs:17/23`、`src-tauri/src/window.rs:573`、`src-tauri/src/setup.rs:338`、`src-tauri/src/cmd/orb.rs:119/124/129/161/204` | 几何 / Orb 位置 / 配置落盘失败改 `warn`（带键名），不阻断退出与拖拽；`perform_exit` 内保持「尽力保存后继续退出」 |
| `src-tauri/src/setup.rs:218-220` | `AppConf::load()` 失败改为 `log::warn!`（带错误）后 `return`；如需让用户可见，用 `emit_shortcut_warning` 广播一条合成 `BindingIssue { id: "config", binding: "", error }`（该函数签名要求 `BindingIssue`，配置读取失败时无真实绑定可用，故取空 binding） |
| `src-tauri/src/setup.rs:231` | `let _ = gs.unregister_all()` 保留但标注 `best-effort`（启动期注销失败无副作用） |
| `src-tauri/crates/assistant-crash/src/crash_dump.rs:122/136` | `fs::write` 失败改为 `if let Err(e) = ... { eprintln!("failed to write {}: {}", log_path.display(), e) }`（崩溃路径不得依赖 logger）；`:122` 处 panic 分支已 `eprintln!` 全文，写文件失败只补一行错误即可，不得改变原有输出顺序 |
| `src-tauri/crates/assistant-providers/src/openai/discovery.rs:16/49` | `extract_model_configs` 内两处 `continue`（`:16` 缺 id、`:49` `serde_json` 解析失败）：累计 `skipped` 计数，`:49` 分支的跳过原因（`e`）与 `key` 至少取第一个样本随汇总输出；结束时若 `skipped > 0` 输出一条 `warn`（`skipped` / `total` + 首个样本原因），全跳过时该汇总即「模型清单为空」的唯一解释 |

验收：V5/V6。

**T12（可选，P2）快捷键告警前端展示**

| 项 | 内容 |
|---|---|
| 事实 | 后端已有 `emit_shortcut_warning` 广播，前端无订阅展示（分析 G4 结论） |
| 要求 | 前端订阅该事件并以 `notification` 提示（带冲突快捷键名）；仅在 OQ-3 决议为「本轮做」时执行 |

### 5.4 保留为显式容错（不改，只标注）

| 位置 | 理由 |
|---|---|
| `clipboard_guard.rs:129` `let _ = thread::spawn(...)` | `JoinHandle` 丢弃是设计意图，无错误可言 |
| 平台 FFI 返回值（`screenshot/**`、`unlock_dialog/windows/**`、`screen_lock/windows.rs`、`tools/sandbox/**`、`biometric/*`） | Win32 / DWM 调用失败不影响主流程，且失败原因无法转化为用户动作 |
| 清理 / 销毁 / 事件发射（删临时文件、杀子进程、`hide` / `destroy`、进度 `emit`、通道 `send`） | 目标对象可能已消失或接收方已断开，重试与提示都无意义 |
| `crash_dump.rs` 内 `eprintln!` | 崩溃路径不可依赖 logger / 文件系统 |
| 解析类兜底（storage 枚举未知值分支） | 分支保留，只把输出通道从 `console` 改为 `logger`（T10） |

标注统一格式：`// best-effort: <一句话说明为什么可以丢>`，供 §6 脚本白名单识别。

---

## 6 门禁

| # | 门禁 | 落点 | 失败即阻断 |
|---|---|---|---|
| G1 | 前端静默吞没与裸 console | `eslint.config.ts`（T10） | `npm run lint` |
| G2 | Rust 侧丢弃点白名单 | 新增 `scripts/check-error-swallow.mjs`：检索 `let _ =` 与 `.ok()`，允许命中 `best-effort:` 注释前一行或白名单文件 | 在 `scripts/check.mjs` 的 `PHASE1` 数组末尾新增一项 `{ id: "swallow", label: "吞没", commands: [`"${process.execPath}" scripts/check-error-swallow.mjs`] }`（PHASE1 由 `runGroup` 并行执行，故与 `format` / `types` / `lint` 同批；该项不提供 `fix`，只报告行号与原因） |
| G3 | 契约同步 | 改 DTO 后跑 `npm run contract:verify`（内含 `contract:gen` + `git diff --exit-code`） | `npm run check` 的 `contract` 步骤 |
| G4 | 语义复核 | §7 复核清单，人工逐项打勾 | code review |

G2 检索口径（示例正则，实现时以脚本为准）：

- `let _ = .*log_dual`
- `let _ = .*(write|clear|fs::write|with_conn)`
- `.catch\(\(\) => \{\}\)`
- `.catch\(\(\) => null\)`

门禁启用顺序：先完成 T3 / T10 的存量修复，再开启 G1 与 G2，避免「规则先落地、仓库全红」。

---

## 7 验收判据

### 7.1 可机检

| # | 判据 | 命令 |
|---|---|---|
| V1 | `VaultErrorCode` 新码存在于契约，DB 故障映射为 `Database` | `npm run contract:verify`（diff 为空）；`cargo test -p assistant-vault` |
| V2 | 全仓无 `.catch(() => {})` / `.catch(() => null)` | `npm run lint` + `node scripts/check-error-swallow.mjs` |
| V3 | `src/` 内除 `logger.ts` 无 `console.*` | `npm run lint`（`no-console`） |
| V4 | 每个 Store 读取动作有失败状态单测 | `npm run test` |
| V5 | Rust 侧无未标注的审计 / 持久化丢弃点 | `node scripts/check-error-swallow.mjs`、`cargo test` |
| V6 | 日志级别与对象标识符合 §2 口径 | 脚本 + review |
| V7 | 人为注入违规写法后门禁失败 | 手动验证一次并记录在 PR 说明 |

### 7.2 可复核

| # | 判据 |
|---|---|
| R1 | 每个 T 项都能在 diff 中定位到「原写法 → 新写法」，无整段重写 |
| R2 | 新增错误提示均走既有 API（`useSettingMessage` / Store `error` 字段 / `vaultErrorMessage`），无新的全局 toast 封装 |
| R3 | 新增 i18n 键中英齐全，无硬编码中文 |
| R4 | `best-effort` 标注均有一句话理由，且理由与 §2 口径一致 |

---

## 8 拒绝方案

| # | 方案 | 拒绝理由 |
|---|---|---|
| X1 | 全局开启 `deny(let_underscore_must_use)` | 误伤 `JoinHandle`、`#[must_use]` 探针与 FFI 返回值，产生大量无意义噪声；改为白名单脚本 |
| X2 | 所有 `log.error` 一律升级为用户 toast | 加载类失败会变成每次进入页面的弹窗噪声，且掩盖真正的用户动作失败；按 §2 口径区分 |
| X3 | 用「换 key 再解密一次」区分密码错与数据损坏 | 信息论上不可行（AEAD 对两者不可区分），且会触发限速计数、放大暴力破解面 |
| X4 | 剪贴板还原失败后重试 | 还原窗口期内用户可能已复制新内容，重试会覆盖用户数据 |
| X5 | 给 `log_dual` 加返回值让调用方 `?` 传播 | 审计失败会阻断业务主流程，违反 D3 |
| X6 | 用 `Record<string, unknown>` 承载错误状态 | 违反类型安全规范，错误状态必须是显式字段 |

---

## 9 开放问题

| # | 问题 | 影响 | 待定方式 |
|---|---|---|---|
| OQ-1 | 自动锁失败是否复用 vault 全局 `error`（可能触发解锁页遮挡） | T3 该处改法 | 实施前确认 vault 错误横幅的展示条件 |
| OQ-2 | vault 配置 fail-open（`VaultConf::load().ok()`、剪贴板超时兜底 30s）归属 | 与 I64 EXP-4 重叠 | 由 I64 处理，本规格不重复 |
| OQ-3 | 快捷键告警前端展示（T12）是否本轮做 | P2 范围 | 排期时决议 |
| OQ-4 | 设置分组读取失败的交互形态（重试按钮 vs 自动重试一次） | T4 UI 形态 | 与设置页现有空态组件对齐后决议 |
| OQ-5 | 整改完成后是否回写分析文档顶部「已失效条目」 | 文档一致性 | 归档分析文档时一并处理 |

---

## 10 实施顺序

1. **存量收口**：T3 → T10（前端 5 处 + console 收口）
2. **门禁落地**：G1、G2（此后新违规即阻断）
3. **可感知**：T5 → T4 → T9（先补写失败提示，再补读失败状态，最后调日志级别）
4. **Rust 与后台**：T6 → T7 → T8 → T8b → T11
5. **P0 与契约**：T1 → T1b → T1c → T2（含 `contract:gen`）
6. **可选**：T12

依赖关系：T2 涉及契约生成，必须独立成一次提交；T4 与 T5 触及同一批设置页文件，按 §5 顺序改避免重复 diff。
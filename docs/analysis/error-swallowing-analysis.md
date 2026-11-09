# 错误吞没分组分析（前端 / Rust）

> 分析对象：当前工作区代码（前端 `src/`，后端 `src-tauri/`）
> 分析方式：静态检索 + 人工分组判定
> 关联规范：`docs/analysis/PROJECT-DESIGN-GUIDELINES.md` §4 错误处理、I19-error-system-spec（已归档）

## 目录

- [错误吞没分组分析（前端 / Rust）](#错误吞没分组分析前端--rust)
  - [目录](#目录)
  - [0 结论摘要](#0-结论摘要)
  - [1 分析口径](#1-分析口径)
    - [1.1 检索数据](#11-检索数据)
    - [1.2 吞没分类](#12-吞没分类)
    - [1.3 风险等级](#13-风险等级)
    - [1.4 判定原则：吞没 vs 降级](#14-判定原则吞没-vs-降级)
    - [1.5 方法局限](#15-方法局限)
  - [2 功能域分组总览](#2-功能域分组总览)
  - [3 分组逐项分析](#3-分组逐项分析)
    - [G1 对话与流式](#g1-对话与流式)
    - [G2 模型提供商](#g2-模型提供商)
    - [G3 密码库](#g3-密码库)
    - [G4 安全与系统集成](#g4-安全与系统集成)
    - [G5 存储与持久化](#g5-存储与持久化)
    - [G6 同步](#g6-同步)
    - [G7 配置与设置](#g7-配置与设置)
    - [G8 工具箱](#g8-工具箱)
    - [G9 悬浮球 / 桌宠 / 窗口 / 托盘](#g9-悬浮球--桌宠--窗口--托盘)
    - [G10 日志 / 崩溃 / 全局兜底](#g10-日志--崩溃--全局兜底)
    - [G11 历史记录](#g11-历史记录)
  - [4 横向共性模式](#4-横向共性模式)
  - [5 优先级整改清单](#5-优先级整改清单)
    - [P0（必须先改）](#p0必须先改)
    - [P1（本迭代）](#p1本迭代)
    - [P2（择机）](#p2择机)
  - [6 防回归与验收](#6-防回归与验收)
    - [6.1 机制性防回归](#61-机制性防回归)
    - [6.2 验收标准](#62-验收标准)
    - [6.3 建议的推进顺序](#63-建议的推进顺序)

---

## 0 结论摘要

1. 错误处理基础设施已经成型：`tauriInvoke` 统一记录并重抛、`createLogger` 分级日志、`globalErrorHandler` 兜底 `error` / `unhandledrejection`、`AppErrorBoundary` 组件级边界、Rust `crash_dump` panic 转储。基础设施不是缺口。
2. 真正的缺口是**业务分支上的吞没**：前端 3 处 `.catch(() => {})` 违反 §4 硬规则 1；Rust 侧 `let _ =` 203 处、`.ok()` 68 处、`map_err(|_| ...)` 33 处、`Err(_)` 29 处，其中密码库审计链路与流式落库链路属于高风险。
3. 按功能域分组后，风险集中在 3 个域：**密码库（Vault）**、**对话流式落库（Chat）**、**配置写入（Config/Settings）**。其余域多为可接受的容错（平台 FFI best-effort、解析降级、UI 展示降级）。
4. 需要区分「吞没」与「降级」：`Err(_) => 默认值` 在解析/展示路径上是设计意图，不应一刀切整改；但同类写法出现在**持久化、审计、安全判定**路径上时即为缺陷。

---

## 1 分析口径

### 1.1 检索数据

| 语言 | 检索模式 | 命中行数 | 涉及文件 | 说明 |
|------|---------|---------|---------|------|
| 前端 | `catch`（含 `.catch(`、`catch (`、`catch {`） | 212 | 82 | 全量捕获点 |
| 前端 | `catch {`（无错误绑定） | 37 | 30 | 无法引用 `error`，只能静默或统一文案 |
| 前端 | `.catch(() => {})` | 3 | 3 | 完全静默，违反 §4 硬规则 1 |
| 前端 | `.catch(() => 默认值)` | 5 | 4 | 静默默认值 |
| 前端 | `console.error` / `console.warn` | 6 | 3 | 其中 4 行为 `logger.ts` 内部实现，2 行为违规调用 |
| Rust | `let _ =` | 203 | 51 | 含 `tests/`、`examples/`，业务代码约 150 |
| Rust | `.ok()` | 68 | 35 | 多为解析/探测降级 |
| Rust | `map_err(\|_\| ...)` | 33 | 12 | 上下文丢失 |
| Rust | `Err(_)` | 29 | 19 | 分支跳过或语义替换 |
| Rust | `let _ = audit_log(...)` | 32 | 11 | 审计链路断链，全部 P0 |

说明：命中行数不等于缺陷数。`let _ =` 中相当一部分只是释放返回值（如 `DestroyWindow`、`thread::spawn` 的 `JoinHandle`），这类不计入吞没缺陷。

### 1.2 吞没分类

| 编号 | 类型 | 特征 | 典型写法 |
|------|------|------|---------|
| S1 | 静默吞没 | 错误被捕获后无任何记录、无用户反馈、无降级说明 | `.catch(() => {})`、`catch { /* ignore */ }`、`let _ =` |
| S2 | 日志级吞没 | 只写日志，用户无感知；日志级别低于错误严重性 | `catch { log.debug(...) }`、`log.warn` 用于失败动作 |
| S3 | 默认值吞没 | 用默认值替代失败，调用方无法区分「无数据」与「取数失败」 | `.catch(() => setVersion("-"))`、`Err(_) => default`、`.ok().unwrap_or_default()` |
| S4 | 分支跳过吞没 | `match`/`if let` 的 `Err` 分支直接 `return`/`continue`/`break` | `Err(_) => return`、`Err(_) => continue` |
| S5 | 上下文丢失 | 错误被转换成更泛化的错误，原始信息丢失 | `map_err(\|_\| X)`、`Err(_) => InvalidPassword` |
| S6 | 语义替换 | 错误被替换成另一类错误，误导排障方向 | DB 错误 → 「密码错误」 |

### 1.3 风险等级

| 等级 | 定义 | 判定标准 |
|------|------|---------|
| P0 | 崩溃 / 数据丢失 / 安全审计断链 | 落库失败被忽略、审计事件丢失、错误分类被错误替换导致误判 |
| P1 | 用户无感知的功能失败 | 用户主动操作失败但无提示；状态与后端不一致且无补偿 |
| P2 | 非关键路径，可接受 | 平台 FFI 返回值、解析降级、UI 展示降级、清理动作 |
| P3 | 合理设计 | 明确的容错分支，有注释或语义清晰，且不影响核心流程 |

### 1.4 判定原则：吞没 vs 降级

同一段代码在不同语境下结论可能相反，本文按以下三条原则判定：

1. **看路径性质**。解析、探测、展示、清理属于「降级友好」路径，失败返回默认值可接受；持久化、审计、安全判定、写后一致性属于「不可降级」路径，失败必须留痕。
2. **看信息是否可回溯**。只要日志中能定位到「哪个操作、哪个对象、什么错误」，即便用户无感知也可降为 P2；反之即使写了 `log.error` 但缺少关键字段，仍记为 P1。
3. **看用户是否可感知差异**。用户主动触发的动作（复制、保存、删除、同步）失败必须给反馈；后台补偿、被动刷新可以只记录日志。

据此，`Err(_) => 默认值` 在解析路径上记为 P3，在读取数据路径上记为 P1——差别不在写法，而在失败后果。

### 1.5 方法局限

- 本次为**静态检索 + 人工判定**，未结合运行时埋点数据，命中密度不等于实际发生频率。
- 未逐个执行测试或故障注入，P0/P1 判定依据是代码路径分析，可能遗漏调用方已有的补偿逻辑。
- `tests/`、`examples/` 中的命中未逐条剔除，仅在小结中给出估算。
- 前端只覆盖 `src/`，构建脚本、Tauri 配置文件不在范围内。

---

## 2 功能域分组总览

下表同时给出检索密度与结论。密度列仅用于定位，不作为缺陷计数（同一行可能同时命中 `let _ =` 与 `audit_log`）。

| 组 | 功能域 | 覆盖范围（前端 / Rust） | 前端 catch | Rust 命中 | 最高等级 | 结论 |
|----|--------|------------------------|-----------|----------|---------|------|
| G1 | 对话与流式 | `src/chat/`、`src/stores/chat*`、`src/services/chat*` / `src-tauri/src/cmd/chat/`、`crates/assistant-agent/` | 37 | 11 | P0 | 流式落库通道失败 + 3 处前端静默，需整改 |
| G2 | 模型提供商 | `src/settings/provider/`、`src/stores/providerStore.ts` / `crates/assistant-providers/` | 3 | 16 | P2 | 健康，仅建议补「全条目跳过」汇总日志 |
| G3 | 密码库 | `src/vault/` / `crates/assistant-vault/`、`src-tauri/src/cmd/vault/` | 72 | 89 | P0 | 审计链路 32 处 + 解锁错误语义替换，必须整改 |
| G4 | 安全与系统集成 | `src/features/vault/settings/` / `crates/assistant-security/`、`crates/assistant-crypto/` | 3 | 64 | P1 | FFI 部分可接受，剪贴板恢复静默需整改 |
| G5 | 存储与持久化 | `src/stores/historyStore.ts` / `crates/assistant-storage/` | 4 | 11 | P1 | 分支定位失败退化为 `None`，需区分「无」与「失败」 |
| G6 | 同步 | `src/vault/core/slices/vaultSyncSlice.ts` / `crates/assistant-sync/`、`assistant-vault/src/sync_engine/` | 27 | 9 | P1 | 分支 remote 写入失败 + 4 处加载静默需整改 |
| G7 | 配置与设置 | `src/settings/`、`src/stores/settingsStore.ts`、`src/stores/themeStore.ts` / `crates/assistant-config/`、`src-tauri/src/cmd/app.rs` | 22 | 6 | P1 | 唯一写出口统一吞掉异常，需透出失败 |
| G8 | 工具箱 | `src/tools/`、`src/stores/toolStore.ts` / `crates/assistant-tools/` | 30 | 8 | P3 | 设计合理，仅 `toolStore` 日志级别需上调 |
| G9 | 悬浮球 / 桌宠 / 窗口 / 托盘 | `src/orb/`、`src/mascot/` / `src-tauri/src/window.rs`、`tray.rs`、`setup.rs`、`menu.rs`、`cmd/orb.rs` | 15 | 27 | P2 | 表现层尽力而为，可接受 |
| G10 | 日志 / 崩溃 / 全局兜底 | `src/services/logger.ts`、`globalErrorHandler.ts`、`tauriInvoke.ts` / `crates/assistant-crash/` | 4 | 3 | P3 | 正确实现，作为整改模板 |
| G11 | 历史记录 | `src/history/` / `src-tauri/src/cmd/conversation.rs` | 12 | 2 | P1 | 操作类良好，读取类需区分错误态 |

**组间交叉引用**：`vaultSyncSlice.ts` 的 27 处 catch 同时出现在 G3 与 G6，按「同步语义」归属 G6 分析，G3 中仅保留结论引用。

---

## 3 分组逐项分析

### G1 对话与流式

**前端**

| 位置 | 写法 | 类型 | 等级 | 判定 |
|------|------|------|------|------|
| `src/stores/chatStorePorts.ts:161` | `chatService.incrementGeneration(convId).catch(() => {})` | S1 | P1 | 需整改 |
| `src/chat/core/chatUtils.ts:66` | `fetchTokenUsage(...).catch(() => {})` | S1 | P1 | 需整改 |
| `src/chat/message/MessageBubble.tsx:76` | `navigator.clipboard.writeText(text).catch(() => {})` | S1 | P1 | 需整改 |
| `src/chat/ChatView.tsx:87` | `.catch(() => useChatStore.getState().loadConversations())` | S3 | P2 | 可接受（有降级路径） |
| `src/chat/ChatView.tsx:166/205/245` | `.catch((e) => log.warn/log.error(...))` | S2 | P2 | 可接受 |
| `src/chat/core/contentBlockUtils.ts:147` | `catch { return null }` | S3 | P3 | 可接受（解析容错） |
| `src/chat/message/FileCard.tsx:19`、`ImageThumbnail.tsx:75` | `console.warn(...)` | S2 | P2 | 违反 §4 硬规则 2，建议改用 `createLogger` |

**关键问题说明**

`incrementGeneration` 与 `fetchTokenUsage` 两处吞没的共性：它们是**写后读一致性**的补偿动作。

- `incrementGeneration` 在流式完成后推进会话 generation 计数，用于版本/缓存判定。失败被静默吞掉后，前端认为已推进、数据库未推进，后续基于 generation 的判定会出现偏差，且没有任何日志可回溯。
- `fetchTokenUsage` 已经做了 seq 序号与活跃会话双重校验（防竞态），说明作者清楚这是异步补偿路径，但最终失败仍未记录，导致「上下文用量长期不更新」无法归因。
- 剪贴板写入失败是用户可感知动作（用户点了复制），完全静默会让用户误以为复制成功。

**Rust**

| 位置 | 写法 | 类型 | 等级 | 判定 |
|------|------|------|------|------|
| `src-tauri/src/cmd/chat/stream.rs:92` | `let _ = db_tx.send(DbWriteMsg { ... })` | S1 | P0 | 必须整改 |
| `src-tauri/src/cmd/chat/mod.rs:264` | `let _ = db_tx.send(DbWriteMsg { ... })` | S1 | P0 | 必须整改 |
| `src-tauri/src/cmd/chat/mod.rs:280` | `let _ = compact_tx.send(...)` | S1 | P2 | 可接受（压缩是可选增强） |
| `src-tauri/src/cmd/chat/mod.rs:218/292/312/328/350` | `let _ = finish_tx.send(conv_id)` | S1 | P2 | 可接受（收尾信号，通道关闭即无意义） |
| `src-tauri/src/cmd/chat/stream.rs:126/187` | `Err(_) => { error!(...) }` | S2 | P1 | 已记录但仅日志，落库失败用户无感知 |
| `src-tauri/src/cmd/chat/insert.rs:49` | `.and_then(\|id\| agent_modes::get_by_id(conn, id).ok().flatten())` | S3 | P2 | 可接受（模式缺失时走默认） |
| `crates/assistant-agent/src/chat_runtime.rs:293` | `Err(_) => TimedStreamItem::TimedOut` | S6 | P3 | 可接受（语义等价映射） |

**关键问题说明**

流式落库走 `mpsc` 通道交给后台写线程，`send` 失败意味着**接收端已关闭**（写线程退出或 panic）。此时整段流式内容不会落库，而 `send` 的返回值被丢弃，用户侧只看到「消息正常结束」，重启后消息丢失。这是本组唯一的 P0：建议 `send` 失败时记录 error 并向前端发一条明确的持久化失败事件，而不是静默。

`stream.rs:126/187` 的 `Err(_)` 分支已有 `error!` 日志，比纯静默好一档，但缺少「失败内容长度 + 会话 ID + 用户提示」，排障时无法定位是哪条消息丢了。

---

### G2 模型提供商

**Rust**

| 位置 | 写法 | 类型 | 等级 | 判定 |
|------|------|------|------|------|
| `crates/assistant-providers/src/client/error.rs:53` | `serde_json::from_str::<Value>(body).ok()` | S3 | P3 | 可接受（解析失败即无 code） |
| `crates/assistant-providers/src/openai/discovery.rs:44` | `Err(_) => continue` | S4 | P2 | 可接受（单模型条目跳过） |
| `crates/assistant-providers/src/gemini/message_converter.rs:203/259` | `Err(_) => json!({...默认结构})` | S3 | P2 | 可接受（消息结构降级） |
| `crates/assistant-providers/src/client/quirks.rs:9/91/94/134/139` | `.ok()` 链式判定 | S3 | P3 | 合理（URL/版本探测失败即视为不匹配） |
| `crates/assistant-providers/src/tool_call_accumulator.rs:98` | `from_str(...).ok()` | S3 | P2 | 可接受（增量 JSON 未完成时无法解析属正常） |
| `crates/assistant-providers/src/openai/responses_stream.rs:636/693` | `.ok().and_then(\|r\| r.usage)` | S3 | P2 | 可接受（usage 缺失不阻塞流） |
| `crates/assistant-providers/src/model_catalog.rs:98/104` | `.and_then(\|v\| u32::try_from(v).ok())` | S3 | P3 | 合理（类型不匹配即放弃该字段） |

**判定**

本组整体健康。所有 `.ok()` 都落在「可选元数据 / 探测 / 增量解析」路径，且缺失后的行为是明确的降级而非功能失败。唯一建议：`discovery.rs` 的 `continue` 若连续跳过全部条目，应升级为一次 `warn` 汇总，否则「模型列表为空」无法归因。

**前端**：`src/settings/provider/ConnectDialog.tsx:166` 已是 `msg.error(...)`，`196` 为 `catch { return }`（表单校验提前返回），均可接受。

---

### G3 密码库

这是命中密度最高的组，也是唯一同时存在 P0 与「安全审计断链」的组。

**前端**

| 位置 | 写法 | 类型 | 等级 | 判定 |
|------|------|------|------|------|
| `src/vault/VaultEntryDrawer.tsx:101` | `.catch(() => setNotes(""))` | S3 | P2 | 可接受（备注读取失败置空） |
| `src/vault/VaultEntryDrawer.tsx:141` | `catch { message.error(uri_invalid) }` | S2 | P2 | 可接受（用户可见） |
| `src/vault/VaultEntryDrawer.tsx:185`、`src/vault/settings/SshAgentTab.tsx:34` | `catch { message.error(save_failed) }` | S2 | P2 | 可接受，但丢失原始错误，建议带 code |
| `src/vault/core/slices/vaultSyncSlice.ts`（27 处） | 全部 `log.error + failSync/failWith` | S2 | P2 | 结构良好，但 4 处 `load*` 失败仅日志（见 G6） |
| `src/vault/core/slices/vaultEntriesSlice.ts`（9 处） | `log.error + set({ error, errorCode })` | S2 | P3 | 良好范例 |
| `src/vault/core/slices/vaultRevealSlice.ts`（3 处） | `log.error + set({ error })` | S2 | P3 | 良好范例 |
| `src/vault/core/slices/vaultBiometricSlice.ts:127` | `catch { log.warn("...cleanup") }` | S2 | P2 | 可接受（清理路径二次失败） |

**Rust — 审计链路（核心问题）**

| 位置 | 写法 | 类型 | 等级 |
|------|------|------|------|
| `src-tauri/src/cmd/vault/reveal.rs:25/54/69/81/100/112` | `let _ = audit_log(...)` | S1 | P0 |
| `src-tauri/src/cmd/vault/audit.rs:45/95/158/225` | `let _ = audit_log(...)` | S1 | P0 |
| `src-tauri/src/cmd/vault/ssh.rs:74/109` | `let _ = audit_log(SshKeyLoaded / SshKeyRemoved)` | S1 | P0 |
| `src-tauri/src/cmd/vault/recovery.rs:32` | `let _ = audit_log(RecoveryKeyGenerated)` | S1 | P0 |
| `src-tauri/src/cmd/vault/entries.rs:281` | `let _ = audit_log(RecoveryKeyInvalidated)` | S1 | P0 |
| `src-tauri/crates/assistant-vault/src/security_db/mod.rs:177/190/202` | `let _ = audit_log(...)` | S1 | P0 |
| `src-tauri/crates/assistant-vault/src/security_db/rekey.rs:230` | `let _ = audit_log(MasterPasswordChange)` | S1 | P0 |
| `src-tauri/crates/assistant-vault/src/security_db/entries/write.rs:66/181/268/286/303` | `let _ = audit_log(...)` | S1 | P0 |
| `src-tauri/crates/assistant-vault/src/security_db/entries/import.rs:41/68/285` | `let _ = audit_log(...)` | S1 | P0 |
| `src-tauri/crates/assistant-vault/src/security_db/entries/deleted.rs:80/193/208` | `let _ = audit_log(...)` | S1 | P0 |
| `src-tauri/crates/assistant-vault/src/security_db/biometric_ops.rs:82/135/185` | `let _ = audit_log(...)` | S1 | P0 |

共 32 处审计写入丢弃返回值。审计日志是密码库的可追责基础：`VaultUnlockFailed`、`MasterPasswordChange`、`VaultEntryDeleted`、`HibpLeakFound` 这些事件写失败时无任何痕迹，等价于审计链断链。这类调用**不能**用 `let _ =`，应至少 `if let Err(e) = ... { log::error!(...) }`，并考虑「审计写失败是否应阻断主操作」的产品决策（建议：不阻断，但必须留下二级日志）。

**Rust — 错误语义替换（误导排障）**

| 位置 | 写法 | 类型 | 等级 |
|------|------|------|------|
| `crates/assistant-vault/src/security_db/mod.rs:151/157/167` | `Err(_) => return Err(VaultError::InvalidPassword)` | S6 | P0 |

解锁流程里读取 `master_key_salt` / `hash_salt` / `master_key_hash` 失败（数据库损坏、权限问题、SQLCipher key 错误）会被统一报成「密码错误」。用户会反复重试密码，而真实原因是数据库不可读。这是典型 S6：不是吞没，但错误方向被替换，后果等同吞没（真实原因不可见）。

**Rust — 其他**

| 位置 | 写法 | 类型 | 等级 | 判定 |
|------|------|------|------|------|
| `crates/assistant-vault/src/recovery_key/mod.rs:90/110/139/159` | `map_err(\|_\| ...)` | S5 | P2 | 可接受（错误已是明确的领域错误） |
| `crates/assistant-vault/src/security_db/crypto_ops.rs`（4 处） | `map_err(\|_\| ...)` | S5 | P2 | 可接受 |
| `crates/assistant-vault/src/biometric/linux.rs`（10 处） | `map_err(\|_\| BiometricFailed(...))` | S5 | P2 | 可接受（FFI 层统一化） |
| `crates/assistant-vault/src/vault_file/mod.rs:220/223/325/423` | `.ok()` 链 | S3 | P2 | 可接受（meta 缺失时重建） |
| `crates/assistant-vault/src/sync_engine/history.rs:213-216` | `git.show_file(...).ok()` | S3 | P2 | 可接受（冲突三方内容缺失属正常） |

---

### G4 安全与系统集成

| 位置 | 写法 | 类型 | 等级 | 判定 |
|------|------|------|------|------|
| `crates/assistant-security/src/unlock_dialog/windows/wm_handlers.rs`（14 处） | `let _ = PostMessageW/SendMessageW/DestroyWindow/InvalidateRect` | S1 | P2 | 可接受（Win32 best-effort，失败无可恢复动作） |
| `crates/assistant-security/src/unlock_dialog/windows/paint.rs`（12 处）、`subclass.rs`（18 处） | 同上 | S1 | P2 | 可接受 |
| `crates/assistant-security/src/screen_lock/mod.rs:294-341` | `let _ = WTSRegisterSessionNotification` 等 | S1 | P2 | 可接受，但注册失败已 `warn`，反注册失败静默（对称性建议） |
| `crates/assistant-security/src/clipboard_guard.rs:61/78/80` | `let _ = thread::spawn / write_clipboard_internal / clear_clipboard_internal` | S1 | P1 | 需整改：剪贴板恢复失败静默，用户原始剪贴板内容可能丢失 |
| `crates/assistant-security/src/ssh_agent/validate.rs` | `map_err(\|_\| ...)` + `Err(_)` | S5/S4 | P2 | 可接受 |
| `crates/assistant-crypto/src/vault_crypto/mod.rs` | `map_err(\|_\| ...)` | S5 | P2 | 可接受 |

**判定**：平台 FFI 的 `let _ =` 是正确用法（返回值无有意义错误分支，且无替代动作）。本组唯一需处理的是 `clipboard_guard.rs` 的定时恢复：它保证「复制密码后 N 秒还原剪贴板」，恢复失败用户无法感知，安全承诺实际未兑现。

---

### G5 存储与持久化

| 位置 | 写法 | 类型 | 等级 | 判定 |
|------|------|------|------|------|
| `crates/assistant-storage/src/conversations/tree_ops.rs:113/171/175/186/190` | `.ok().flatten()` / `.ok()` | S3 | P1 | 需整改：分支树查询失败返回 `None`，与「无分支」不可区分 |
| `crates/assistant-storage/src/conversations/ops.rs:42` | `.filter_map(\|r\| r.ok())` | S4 | P2 | 可接受（搜索逐行容错） |
| `crates/assistant-storage/src/content_codec.rs:4` | `from_str(content).ok()?` | S3 | P3 | 合理（旧纯文本格式回退） |
| `crates/assistant-storage/src/attachment_store.rs:128` | `.ok()` 链取 mtime | S3 | P2 | 可接受（时间缺失用 0） |
| `crates/assistant-storage/src/attachment_store.rs:153` | `let _ = fs::remove_file(&path).await` | S1 | P2 | 可接受（清理失败仅残留文件） |
| `src-tauri/src/cmd/conversation.rs:530` | `conversations::increment_generation(&conn, &id)` 直接返回 | — | P1 | 已正确透传，但前端调用侧吞没（见 G1） |
| `src-tauri/src/cmd/conversation_utils.rs` | `.ok()` | S3 | P2 | 可接受 |

**判定**：`tree_ops` 的 `find_branch_leaf(...).ok()` 是组内最需要注意的一处：分支树重建/定位失败会退化为 `None`，上层按「无叶子节点」处理，可能导致分支切换后定位错误，且无日志。建议改为 `Result` 透传或在 `Err` 分支 `warn` 后返回 `None`。

---

### G6 同步

| 位置 | 写法 | 类型 | 等级 | 判定 |
|------|------|------|------|------|
| `src/vault/core/slices/vaultSyncSlice.ts:176/202/221/230` | `catch { log.error(...) }`（`loadSyncConfig`/`loadSyncReadyness`/`loadPendingRemovals`/`loadSyncLastTime`） | S2 | P1 | 需整改：加载失败后状态保持旧值或空值，UI 无提示 |
| `src/vault/core/slices/vaultSyncSlice.ts:122-311`（其余 23 处） | `log.error + failSync/failWith` | S2 | P3 | 良好范例（失败进入统一错误态） |
| `crates/assistant-sync/src/git_sync/mod.rs:173` | `let _ = git_config_branch_set_remote(...)` | S1 | P1 | 需整改：分支 remote 配置未写入，后续 push 会走错 remote |
| `crates/assistant-sync/src/git_sync/ssh.rs:113/118/178` | `Err(_) => return false/return` | S4 | P2 | 可接受（SSH 环境探测失败即判定不可用） |
| `crates/assistant-sync/src/git_sync/ssh.rs:59/84/89/137` | `let _ = hosts / repo / (cmd, repo)` | S1 | P2 | 可接受（`#[cfg]` 分支消除未使用变量） |
| `crates/assistant-sync/src/git_sync/git.rs:390` | `.and_then(\|v\| v.trim().parse::<i64>().ok()).unwrap_or(0)` | S3 | P3 | 合理（提交计数缺失用 0） |
| `crates/assistant-vault/src/sync_engine/*` | `map_err(\|_\| SyncError::VaultLocked)` | S5 | P3 | 合理（锁获取失败统一语义） |

**判定**：同步域整体结构较好，因为 `vaultSyncSlice` 建立了 `failSync` 统一失败出口。缺口是「只读加载类」动作：4 处 `load*` 失败仅日志，导致同步页展示的「上次同步时间/待删除项/就绪状态」可能是陈旧值，用户无法判断同步配置是否真的读到了。

---

### G7 配置与设置

| 位置 | 写法 | 类型 | 等级 | 判定 |
|------|------|------|------|------|
| `src/settings/core/settingsDispatcher.ts:69` | `catch { log.error("dispatchConfPatch failed", e) }` | S2 | P1 | 需整改：配置写入失败仅日志，调用方以为成功 |
| `src/settings/core/sideEffectRegistry.ts:26` | `.catch((err) => log.warn(...))` | S2 | P2 | 可接受（副作用为附加行为） |
| `src/stores/settingsStore.ts:38` | `catch { log.error(loadConf failed) }` | S2 | P1 | 需整改：配置读取失败后 UI 用默认值，用户不知情 |
| `src/stores/themeStore.ts:35/38/77` | `log.warn + 默认值` | S3 | P2 | 可接受（主题降级明确） |
| `src/settings/shared/renderers/SettingField.tsx:223/232` | `catch { /* ignore */ }` | S1 | P2 | 可接受（ColorPicker 取色异常，UI 局部） |
| `src/settings/pages/StorageSettings.tsx:50` | `catch { setBreakdown(null) }` | S3 | P2 | 可接受（统计面板降级） |
| `src/settings/pages/AboutSettings.tsx:68/72` | `.catch(() => setVersion("-"))` | S3 | P2 | 可接受（版本号展示降级） |
| `src/settings/shared/sections/SettingsGroup.tsx:118/138` | `log.error(patch failed)` | S2 | P1 | 需整改：与 `settingsDispatcher` 同源问题 |
| `crates/assistant-config/src/atomic_write.rs:26/30` | `let _ = fs::remove_file(&temp)` | S1 | P2 | 可接受（临时文件清理） |
| `crates/assistant-config/src/vault_conf.rs:93/94/95` | `.ok()?` 链迁移旧配置 | S3 | P2 | 可接受（旧配置不存在即跳过迁移） |

**判定**：配置域的共性是「写入失败被降级为日志」。`dispatchConfPatch` 是全局配置写入口，它吞掉异常后，所有调用方（主题、快捷键、外观、Mascot）都拿不到失败信号。虽然多数配置项是「尽力而为」，但快捷键、开机自启、Vault 相关配置失败需要用户感知。建议在 dispatcher 层返回 `Result`，由调用方决定提示或忽略，而不是在唯一出口统一吞掉。

---

### G8 工具箱

| 位置 | 写法 | 类型 | 等级 | 判定 |
|------|------|------|------|------|
| `src/tools/*/*.service.ts`（11 处） | `catch { return null / "" / false }` | S3 | P3 | 合理（纯函数解析，调用方展示错误态） |
| `src/tools/*/index.tsx`（13 处） | `catch { message.error(...) / return 错误文本 }` | S2 | P3 | 合理（用户可见） |
| `src/tools/regex-tester/index.tsx:66` | `catch { replaced = "" }` | S3 | P2 | 可接受（替换失败置空） |
| `src/stores/toolStore.ts:67-129`（6 处） | `catch { log.debug(...) + localStorage 回退 }` | S2 | P2 | 需降级关注：`log.debug` 级别过低，失败在默认日志级别下不可见 |
| `crates/assistant-tools/src/exec.rs:33` | `Err(_) => ToolResult::from_exec_error(name, "timed out")` | S6 | P2 | 可接受（超时语义明确） |
| `crates/assistant-tools/src/os/fs_link.rs:317/450/491` | `let _ = (source, target, is_dir)` / `let _ = CloseHandle` | S1 | P2 | 可接受（平台分支/句柄释放） |
| `crates/assistant-tools/src/os/process_output.rs:11` | `Err(_) => 编码探测降级` | S3 | P2 | 可接受 |

**判定**：工具箱是设计上最适合「解析失败即返回空」的域，几乎全部为 P3。唯一建议：`toolStore` 的 `log.debug` 改为 `log.warn`，因为收藏/最近使用属于用户可见数据，静默回退到 localStorage 会让「多设备不同步」难以排查。

---

### G9 悬浮球 / 桌宠 / 窗口 / 托盘

| 位置 | 写法 | 类型 | 等级 | 判定 |
|------|------|------|------|------|
| `src-tauri/src/window.rs:159/165/341/348` | `let _ = destroy/show_or_create/hide/emit` | S1 | P2 | 可接受（窗口操作失败无可恢复动作） |
| `src-tauri/src/tray.rs:101-130`（6 处） | `let _ = show_or_create(...)` | S1 | P2 | 可接受，但托盘点击无响应时无日志 |
| `src-tauri/src/setup.rs:139/145/175-234`（8 处） | `let _ = unregister_all/toggle/set_position` | S1 | P2 | 可接受（快捷键/位置为尽力而为） |
| `src-tauri/src/cmd/orb.rs:117-193`（8 处） | `let _ = save_orb_position_sync / set_position / conf.save()` | S1 | P2 | 可接受（位置保存失败仅下次启动位置回退） |
| `src/orb/OrbDrag.tsx:71/164` | `catch { stop() }` / `catch { /* ignore */ }` | S1 | P2 | 可接受（拖拽中断） |
| `src/orb/OrbPanel.tsx:94` | `catch { /* fall through */ }` | S1 | P2 | 可接受（坐标校验失败走折叠） |
| `src/orb/*`（其余 11 处） | `log.warn/log.error` | S2 | P3 | 良好 |
| `src/mascot/renderers/gif/*`（2 处） | `catch { /* cross-origin restriction */ }` | S1 | P3 | 合理（浏览器限制，无法恢复） |

**判定**：整组为「UI 表现层尽力而为」，P2 为主，无需整改。唯一可选改进：`setup.rs` 的快捷键注册失败应 `warn`，否则「快捷键无效」无任何线索。

---

### G10 日志 / 崩溃 / 全局兜底

| 位置 | 写法 | 类型 | 等级 | 判定 |
|------|------|------|------|------|
| `src/services/logger.ts:44/55` | `catch { return Object.prototype.toString.call(data) }` | S3 | P3 | 合理（序列化兜底，避免日志自身抛错） |
| `src/services/logger.ts:70` | `catch { /* fire-and-forget */ }` | S1 | P3 | 合理（日志持久化失败不能反过来影响业务） |
| `src/services/tauriInvoke.ts:12` | `catch { log.error + throw }` | — | P3 | 良好范例（记录后重抛，不吞没） |
| `src/services/globalErrorHandler.ts` | `error` / `unhandledrejection` 全量记录 + 白屏检测 | — | P3 | 良好范例 |
| `crates/assistant-crash/src/crash_dump.rs:22/52/75` | `let _ = create_dir_all / fs::write` | S1 | P2 | 可接受：写失败已 `eprintln!` 兜底，但 dump 文件缺失无二级提示 |

**判定**：本组是「吞没的正确姿势」参考实现。`logger` 自身吞没是必要的（日志系统不能因持久化失败而级联崩溃），且 `tauriInvoke` 与 `globalErrorHandler` 提供了完整的兜底。文档中应把这三个文件作为整改模板引用。

---

### G11 历史记录

| 位置 | 写法 | 类型 | 等级 | 判定 |
|------|------|------|------|------|
| `src/history/HistoryManager.tsx:106-288`（10 处） | `log.error + message.error/notifyError` | S2 | P3 | 良好范例 |
| `src/stores/historyStore.ts:60/84/121/141` | `log.error` | S2 | P1 | 需整改：列表/消息加载失败无用户提示，界面显示空列表 |
| `src/history/core/useHistorySearch.ts:48` | `log.error + setSearchResults([])` | S3 | P2 | 可接受（搜索结果降级为空） |
| `src/history/core/useHistoryRefresh.ts:27` | `log.error` | S2 | P2 | 可接受（刷新失败保留旧数据） |

**判定**：`HistoryManager` 的操作类动作（删除、重命名、导出、置顶、批量）全部有用户提示，是正面案例。缺口在 `historyStore` 的**读取类**动作：加载失败与「真的没有会话」在 UI 上完全一致，用户会误判数据丢失。

---

## 4 横向共性模式

| 模式 | 出现域 | 本质问题 | 统一整改方式 |
|------|--------|---------|-------------|
| 补偿动作静默失败 | G1、G5、G7 | 主流程成功，补偿失败无人知 | 补偿失败必须 `warn` + 结构化字段（会话 ID / 操作名） |
| 审计写入丢弃返回值 | G3 | 安全事件丢失 | 封装 `audit_or_log!`，禁止直接 `let _ = audit_log` |
| 读取失败伪装成空数据 | G5、G6、G11 | 「无数据」与「读失败」不可区分 | Store 层增加 `loadError` 字段，UI 区分空态与错误态 |
| 错误语义替换 | G3 | DB 故障报成密码错误 | 保留原始错误，仅对外文案降级 |
| 唯一出口统一吞掉 | G7 | 调用方无法感知失败 | dispatcher 返回 `Result`，提示策略下沉到调用方 |
| 日志级别过低 | G8 | `debug` 在默认级别不可见 | 用户可见数据的失败用 `warn` 起 |

---

## 5 优先级整改清单

### P0（必须先改）

| 编号 | 位置 | 问题 | 建议 |
|------|------|------|------|
| P0-1 | `src-tauri/src/cmd/chat/stream.rs:92`、`cmd/chat/mod.rs:264` | 流式落库 `send` 失败被丢弃，消息可能不落库 | 检查 `send` 结果，失败时 `error!` + 向前端发 `chat-persist-failed` 事件 |
| P0-2 | 32 处 `let _ = audit_log(...)`（`cmd/vault/*`、`assistant-vault/src/security_db/*`） | 审计事件写入失败无痕迹 | 新增 `audit_or_log` 辅助函数，失败 `log::error!`；评估关键事件（改密、解锁失败）是否需要阻断 |
| P0-3 | `crates/assistant-vault/src/security_db/mod.rs:151/157/167` | DB 读取失败报成 `InvalidPassword` | 区分 `Database` / `Crypto` / `InvalidPassword`，仅密码校验失败才返回后者 |

### P1（本迭代）

| 编号 | 位置 | 问题 | 建议 |
|------|------|------|------|
| P1-1 | `src/stores/chatStorePorts.ts:161` | `incrementGeneration` 完全静默 | 改为 `log.warn("incrementGeneration failed", { convId, error })` |
| P1-2 | `src/chat/core/chatUtils.ts:66` | token 用量失败无记录 | `log.warn` + 保留 seq/convId |
| P1-3 | `src/chat/message/MessageBubble.tsx:76` | 复制失败无反馈 | 失败时 `message.error(copy_failed)` |
| P1-4 | `crates/assistant-security/src/clipboard_guard.rs:78/80` | 剪贴板恢复失败静默 | `warn` + 记录是否恢复原文 |
| P1-5 | `crates/assistant-storage/src/conversations/tree_ops.rs:175/190` | 分支定位失败退化为 `None` | `warn` 后返回，或改为 `Result` |
| P1-6 | `crates/assistant-sync/src/git_sync/mod.rs:173` | 分支 remote 配置写入失败静默 | `warn` + 在同步状态中暴露 |
| P1-7 | `src/vault/core/slices/vaultSyncSlice.ts:176/202/221/230` | 同步配置/状态加载失败仅日志 | 增加 `syncLoadError` 状态并在 UI 提示 |
| P1-8 | `src/stores/historyStore.ts:60/84/121/141` | 历史加载失败等同空列表 | 增加 `loadError`，UI 显示「加载失败，重试」 |
| P1-9 | `src/settings/core/settingsDispatcher.ts:69`、`src/stores/settingsStore.ts:38`、`src/settings/shared/sections/SettingsGroup.tsx:118/138` | 配置读写失败仅日志 | dispatcher 返回 `Result`；关键配置失败提示 |
| P1-10 | `src-tauri/src/cmd/chat/stream.rs:126/187` | 落库失败只 `error!`，缺上下文 | 补充 `conv_id` / `msg_id` / 字节数 |

### P2（择机）

| 编号 | 位置 | 建议 |
|------|------|------|
| P2-1 | `src/chat/message/FileCard.tsx:19`、`ImageThumbnail.tsx:75` | `console.warn` 改 `createLogger`，满足 §4 硬规则 2 |
| P2-2 | `src/stores/toolStore.ts:67-129` | `log.debug` 升为 `log.warn` |
| P2-3 | `src-tauri/src/tray.rs:101-130` | 窗口唤起失败补 `warn` |
| P2-4 | `src-tauri/src/setup.rs:145` | 快捷键注册/反注册失败补 `warn` |
| P2-5 | `crates/assistant-crash/src/crash_dump.rs:52/75` | dump 写失败时除 `eprintln!` 外再尝试备用路径 |
| P2-6 | `crates/assistant-providers/src/openai/discovery.rs:44` | 全部条目跳过时输出汇总 `warn` |

---

## 6 防回归与验收

### 6.1 机制性防回归

1. **规则化静态检查**：在 CI 中加入两条 grep 级检查，命中即失败。
   - 前端：`\.catch\(\(\) => \{\}\)`（当前 3 处，修完后应为 0）
   - Rust：`let _ = audit_log`（当前 32 处，修完后应为 0）
2. **类型层面约束**：把 `audit_log` 的返回值封装为 `#[must_use]` 的 `AuditGuard`，让「忽略返回值」在编译期产生警告，从机制上消除 `let _ =`。
3. **状态可区分**：所有 Store 的读取类 action 增加 `loadError` 字段，UI 明确区分「空」与「失败」。这能一次性覆盖 G5 / G6 / G11 的同类问题。
4. **日志级别基线**：`debug` 只用于诊断细节；任何「用户可见数据」的失败必须 `warn` 起；任何「持久化 / 安全」路径的失败必须 `error`。
5. **模板复用**：以 `src/services/tauriInvoke.ts`（记录后重抛）、`src/vault/core/slices/vaultEntriesSlice.ts`（`log.error` + 结构化 error state）、`src/history/HistoryManager.tsx`（操作类失败必提示）作为三个正面范例，新增代码按此对齐。

### 6.2 验收标准

| 检查项 | 通过条件 |
|--------|---------|
| 静默吞没 | 全仓 `catch(() => {})` 与 `catch { /* ignore */ }` 数量为 0，或每处带 `// reason:` 说明 |
| 审计链路 | `let _ = audit_log` 数量为 0，且审计失败有二级 `error` 日志 |
| 落库链路 | `db_tx.send` 返回值被检查，失败路径有前端可见事件 |
| 错误语义 | 解锁失败可区分「密码错误」与「数据库不可读」 |
| 读取失败 | Store 读取类 action 均暴露 `loadError`，UI 有空态与错误态两种展示 |
| 日志规范 | 无裸 `console.*` 调用（`logger.ts` 内部除外） |
| 用户可感知 | 用户主动触发的动作失败均有 `message.error` / `notifyError` |

### 6.3 建议的推进顺序

1. 先做 P0-2（审计链路）与 P0-3（错误语义）—— 两者都在 `assistant-vault`，可一次改动完成，且是安全承诺的底线。
2. 再做 P0-1（流式落库）—— 涉及前后端协议，需要同步改事件定义。
3. 随后批量处理 P1 中的「读取类」三项（G5 / G6 / G11）—— 同一套 `loadError` 模式，可统一实现。
4. 最后清理 P2 与引入 CI 检查，把修复固化为规则。
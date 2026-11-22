# I65 — 错误吞没整改 任务清单

> 规格：[I65-error-swallowing-remediation-spec.md](./I65-error-swallowing-remediation-spec.md)（**唯一事实基线**；四口径判定 / D1–D7 决议 / V1–V7 验收 / G1–G4 门禁 / X1–X6 拒绝方案全部以它为准，本清单只拆「改哪里、怎么改、跑什么验收」，不重述语义）
> 事实基线：规格 §3 的 N1–N28；分析：`error-swallowing-analysis.md`（已归档）
> 关联：`I64-storage-vault-integration-audit-spec.md`（已归档）；其 OQ-2 由该规格处理，**不在本清单范围**

切片对应规格条目：**S0** 规格基线对齐（文档-only）→ **S1** 存量收口（T3 / T10）→ **S2** 门禁落地（G1 / G2）→ **S3** 可感知（T5 / T4 / T9）→ **S4** Rust 与后台（T6 / T7 / T8 / T8b / T11）→ **S5** P0 与契约（T1 / T1b / T1c / T2）→ **S6** 可选（T12）。

依赖：S2 依赖 S1（**先修存量再开门禁**，否则规则一落地仓库全红，见规格 D6）；S5-d（T2）必须独立提交（含 `contract:gen` 产物）；S6 依赖 OQ-3。

**进度总览**（勾选状态与各节同步）：

| 切片 | 状态 | 阻塞 |
|---|---|---|
| S0 规格基线对齐 | 完成 | — |
| S1 存量收口（T3 / T10） | 完成 | — |
| S2 门禁（G1 / G2） | 完成 | — |
| S3 可感知（T5 / T4 / T9） | 完成 | — |
| S4 Rust 与后台（T6–T11） | 完成 | — |
| S5 P0 与契约（T1 / T1b / T1c / T2） | 完成（代码与契约产物） | T2 独立提交待执行 |
| S6 可选（T12） | 未开工 | OQ-3 |

> 本清单不管理提交。提交粒度按代码依赖与可编译性现场决定；这里只回答「代码改没改、验收跑没跑」。
> 勾选表示代码已改且对应验收已跑。

> **路径读法**：文中行号均为**改造前现状**行号（2026-11-21 逐条核实），仅用于定位，不要当作改动后的行号。规格 §3 有 5 处行号/文件名因重构漂移，本清单已按代码修正并在 S0 中登记，**以本清单为准**。

---

## 阶段 S0 — 规格基线对齐（文档 only，不改代码）

退出条件：规格头部引用可解析；§3 的行号漂移修正完毕，后续阶段照本清单定位即可。

### S0-a 失效引用路径

- [x] `docs/store/I65-error-swallowing-remediation-spec.md` 头部「事实来源」：`docs/analysis/error-swallowing-analysis.md` → 现实际位于 `error-swallowing-analysis.md`（已归档）
- [x] 同文件头部「相邻规格」：`docs/store/I64-storage-vault-integration-audit-spec.md` → `I64-storage-vault-integration-audit-spec.md`（已归档）
- [x] 规格正文内对分析文档章节的引用（§1.2 / §1.3 / §1.4）随路径同步；归档完成后按项目文档规范改为「文档名 +（已归档）」的写法，不再带路径

### S0-b §3 行号漂移修正

- [x] **N6**：`reclaimFlow.ts` → `cleanupFlow.ts`；「7 处 `console.warn`」实为 **6 处 warn + 1 处 debug**：warn 在 `cleanupFlow.ts:186/:227/:237`、`entryView.ts:90`、`lexicon.ts:117`、`residualView.ts:181`，debug 在 `services/stream/transport.ts:175`（规格把它算进了 7 处，但级别是 debug）
- [x] **N6 补充**：`lexicon.ts:147/162` 实际只有 `:117` 一处；`entryView.ts:103` 实际为 `:90`
- [x] **N13**：`SettingsGroup.tsx:118-119` / `:138-139` → 实际为 `:119` / `:139`（两处 `log.error`）
- [x] **N16**：`convert/error.rs:9/16/20/21/38` → 实际为 `:10/16/20/21/38`（仅 `AlreadyUnlocked` 一处下移，其余行号未变；`message` 分支自 `:40` 起）
- [x] **N17**：`tree_ops.rs:131` → 实际 `.unwrap_or(false)` 在 `:123`（另有 `:162` 位于 helper `find_new_active_leaf` 内）
- [x] **N23**：`crash_dump.rs:122/136` → 实际为 `:140/154`
- [x] **N21/N28 补全**：`orb.rs` 实际 8 处（`119/124/128/129/161/185/197/204`）；`setup.rs` 实际 `:230`（`gs.unregister_all`）+ `:337`（`conf.save`）之外，还有 `:187/:191`（`window::toggle`）与 `:274/:276/:288/:333`（`set_position`）；`window.rs` 除 `:216/:573` 外还有 `:210`（`destroy`）/`:568`（`cancel_active`）/`:579`（`hide`）/`:588`（`emit`），按 S4-e 处理
- [x] **N18 路径漂移**：`clipboard_guard.rs` 不在 `src-tauri/src/`，实际在 `src-tauri/crates/assistant-security/src/clipboard_guard.rs`（行号 `129/147/150` 未变）
- [x] **N26 核对**：`src-tauri/src/cmd/compaction.rs:390` / `:403` 与规格一致，无漂移（注意**不是** `assistant-agent/src/compaction.rs`，该文件无 `let _ =`）

---

## 阶段 S1 — 存量收口（规格 T3 / T10）

退出条件：规格 V2 的 grep 为空；`npm run check` 通过（此时门禁尚未开启，靠 grep 兜底）。

### S1-a T3 前端 5 处静默吞没

> 5 处文件的 logger **已全部存在**（仅 `chatUtils.ts` 需新增），无需引入新全局 API。

- [x] `src/vault/core/slices/vaultLifecycleSlice.ts:272`：`void get().lock("autoLock").catch(() => {})` → `.catch((e) => log.warn("autoLock failed", e))`；`log` 已在 `:13`（`createLogger("vaultStore")`）。**不**在 catch 里再置 `error`（`lock` 内部已置），避免重复提示
- [x] `src/stores/chat/chatStorePorts.ts:227`：`chatService.incrementGeneration(convId).catch(() => {})` → `.catch((e) => log.warn("incrementGeneration failed", convId, e))`；`log` 已在 `:46`
- [x] `src/chat/core/chatUtils.ts:66`：`prefetchAttachmentLimits(...).catch(() => {})` → `.catch((e) => log.warn("prefetchAttachmentLimits failed", e))`；**本文件无 logger**，需新增 `import { createLogger } from "@/services/system/logger";` 与 `const log = createLogger("chatUtils");`（放在既有 import 块之后，与其它 store/slice 同风格）
- [x] `src/history/detail/HistoryBranchRow.tsx:82`：`handleCopyRecord` 的 `.catch(() => {})` → 复制失败必须可见（规格口径 3）：
  - `antd` import（`:8`）加 `App`；新增 `const { message: toast } = App.useApp();`（**组件内已有局部 `message` 行数据变量，禁止重名**）
  - 新增 `import { createLogger } from "@/services/system/logger";` 与 `const log = createLogger("historyBranchRow");`
  - 改为 `navigator.clipboard.writeText(recordJson).then(() => toast.success(t("common.actions.copied"))).catch((e) => { log.warn("copy record failed", message.id, e); toast.error(t("common.actions.copy_failed")); })`
  - 两个 i18n 键已存在（`zh-CN.json:39/40`、`en-US.json:39/40` 的 `common.actions.copied` / `common.actions.copy_failed`），无需新增
  - `handleCopyRecord` 的 `useCallback` 依赖数组补 `toast` / `t` / `message.id`（`:79-82` 当前只有 `[recordJson]`）
- [x] `src/chat/ConversationView.tsx:95`：`.catch(() => {})` → `.catch((e) => log.debug("prefetchAttachmentLimits failed", e))`；`log` 已在 `:37`（`createLogger("ConversationView")`）。仅升到 debug：属展示优化，失败不影响主流程
- [x] `src/chat/ConversationView.tsx:99-101`（同文件第二处，**规格漏记**）：`.then(() => loadConversations()).catch(() => useChatStore.getState().loadConversations())` → catch 内先 `log.warn("loadProviders failed, fallback to conversations", e)` 再 `void useChatStore.getState().loadConversations()`（保留原兜底行为，只补留痕；属后台自动动作，口径 4 → 至少 warn）
  - 该处是「吞没 + 兜底」（catch 体非空），**不在 G2 的扫描口径内**（G2 只抓 `.catch(() => {})` / `.catch(() => null)`，见 S2-b），所以必须在本条手工改掉；仓库另有 44 处 `catch {}` 带兜底/默认返回的写法（如 `toolStore.ts:76` 的 `/* corrupted localStorage, ignore */`）属口径 2 可降级，本次不动，也不开门禁（否则门禁一落地仓库全红，违背 D6）

### S1-b T10 裸 `console` 收口

> 规格写「7 处」，实际 **6 处**（`lexicon.ts` 只 1 处，见 S0-b）。全部替换为 `createLogger("storage")`，**不新增** message 键，日志文本原样保留。

- [x] `src/settings/pages/storage/entryView.ts:90`：`console.warn` → `storageLog.warn`（文件内新增 `createLogger("storage")`，模块级常量）
- [x] `src/settings/pages/storage/cleanupFlow.ts:186` / `:227` / `:237`：同上
- [x] `src/settings/pages/storage/lexicon.ts:117`：同上
- [x] `src/settings/pages/storage/residualView.ts:181`：同上
- [x] `src/services/stream/transport.ts:175`：`console.debug` → `log.debug`；`log` 已在 `:20`（`createLogger("streamTransport")`）；**保留**外层 `import.meta.env.DEV` 门控不变（该条是「开发期诊断」，不升 warn）

**验收**：`rg -n "console\.(log|warn|error|debug)" src` 只剩 `src/services/system/logger.ts`（及 `__tests__` 内的 spy 用法）。

---

## 阶段 S2 — 门禁落地（规格 G1 / G2）

退出条件：G1 能在新增一行 `.catch(() => {})` 时让 `npm run lint` 变红；G2 让 `npm run check` 多一步且失败可定位到 `文件:行`。

### S2-a G1 ESLint 规则

- [x] `eslint.config.ts`：在 `MODULE_REFERENCE_SELECTOR`（`:11`）下方新增**共享常量**（规格 D6：同文件多条 `no-restricted-syntax` 是**整体覆盖**，不是合并，必须抽常量再展开）：
  - `const SWALLOW_SELECTORS: { selector: string; message: string }[] = [...]`，含两条：
    - `.catch` 空回调：`CallExpression[callee.property.name='catch'] > ArrowFunctionExpression[body.type='BlockStatement'][body.body.length=0]`
    - `.catch(() => null)`：`CallExpression[callee.property.name='catch'] > ArrowFunctionExpression[body.type='ArrowFunctionExpression'][body.body.type='Literal'][body.body.value=null]`
    - message 统一写「禁止静默吞没：至少 `log.warn`，或写明 `// best-effort:` 理由」
  - 使用方式（与现有 `EXPLICIT_INDEX_SELECTOR` / `EXPLICIT_INDEX_MESSAGE` 同形）：
    - `INDEX_FILE_GUARD`（`:27-39`）：`["error", {selector: EXPLICIT_INDEX_SELECTOR, message: EXPLICIT_INDEX_MESSAGE}, {selector: "ExportAllDeclaration", ...}, ...SWALLOW_SELECTORS]`
    - base 块（`:102-110`）：`["error", {selector: EXPLICIT_INDEX_SELECTOR, message: EXPLICIT_INDEX_MESSAGE}, ...SWALLOW_SELECTORS]`
    - barrel 块（`:111-123`）：同上再追加 `{selector: selfBarrelSelector(domain), ...}` 与 `...SWALLOW_SELECTORS`
  - 全部用数组展开，**不要**改写成对象合并
- [x] 覆盖范围自检：三处声明分别是 `INDEX_FILE_GUARD`（`:27`）、base 块（`:102-110`）、`BARREL_DOMAINS.map` 块（`:111-123`）。漏任一处 → 该域（history/mascot/vault）或 `index.ts` 文件不受约束（这正是 D6 的坑）
- [x] 同文件新增 `no-console` 门禁（追加到 `defineConfig` 末尾；`INDEX_FILE_GUARD` 当前就在末位 `:124`，新块放它之后即可）
  - `{ files: ["src/**/*.ts", "src/**/*.tsx"], rules: { "no-console": "error" } }`
  - 紧随其后 `{ files: ["src/services/system/logger.ts"], rules: { "no-console": "off" } }`（唯一豁免点；flat config 中后声明的同规则覆盖前者，顺序不可颠倒）
  - 不额外加 `no-empty-function`：`js.configs.recommended` 的 `no-empty` 已覆盖空块，重复会与既有代码冲突
- [x] `scripts/**` 不纳入 `no-console`（脚本本来就靠 stdout 汇报）

**验收**：临时在 `src/chat/core/chatUtils.ts` 加一行 `.catch(() => {})` → `npm run lint` 报错并指出该行；删掉后恢复绿。再在 `src/history/detail/` 下同样验证（确认 barrel 域也生效）。

### S2-b G2 `check-error-swallow.mjs`

- [x] 新增 `scripts/check-error-swallow.mjs`（纯 node，无依赖，风格对齐 `scripts/contract-check.mjs`）：
  - 扫描面：
    - `src/**/*.ts,tsx`：`.catch(() => {})`（含多行空块）与 `.catch(() => null)`
    - `src-tauri/**/*.rs`：**限定** `let _ = .*(write|clear|fs::write|with_conn)`（与规格 §7 的 grep 口径一致）
      - **不要**用裸 `let _ = `：`let _ = (a, b)` / `let _ = &x` / `let _ = thread::spawn(...)` 是「未使用变量」或线程占位惯用法，不是错误吞没（`assistant-agent/src/agent/middleware.rs:87/96/101`、`loop_executor/mod.rs:543/583`、`kernel/request.rs:60`、`kernel/ports/mod.rs:202`、`round_executor.rs:110`、`kernel/turn/state.rs:186`、`crash_dump.rs` 测试段均属此类）
  - 排除：`**/tests/**`、`**/examples/**`（`crash_dump.rs:275` 起的 `fs::remove_dir_all` 均在 `#[cfg(test)]` 内）
  - 豁免：同行或上一行含 `best-effort:`；或命中白名单（逐条带理由注释）：
    - 实际落地：`crates/assistant-vault/src/security_db/biometric_ops.rs:195/291/302/303/304`（密钥清理 + 回调占位，口径 2 可降级）以 `// best-effort:` 注释豁免；`crates/assistant-security/src/clipboard_guard.rs:129`（`thread::spawn`）不匹配扫描正则
    - 白名单最终仅保留 2 条内联 `#[cfg(test)]` 用例（`crates/assistant-tools/src/mcp/stdio/mod.rs:540/541`）
  - 输出：每条 `文件:行  <片段>`，末尾汇总计数；有命中 → `process.exit(1)`
  - 不自动修复（`fix` 留空），避免「脚本改写代码」绕过评审
- [x] `scripts/check.mjs`：`PHASE1`（`:66`）数组末尾追加一步
  - `{ id: "swallow", label: "吞没", commands: [`"${process.execPath}" scripts/check-error-swallow.mjs`] }`
  - 不加 `fix`：`fixThenRetry` 只修带 `fix` 的项，该步失败会直接打印输出并退出，符合预期

**验收**：`node scripts/check-error-swallow.mjs` 在 S1 完成后 exit 0；`npm run check` 输出多一行 `[check] 吞没 ok`。

---

## 阶段 S3 — 可感知（规格 T5 / T4 / T9）

退出条件：规格 V4 / V5 / V6 通过——写失败有提示且值回滚；读失败有可见失败态 + 重试；后台工具链失败进日志。

### S3-a T5 设置写入失败必须可见

- [x] `src/settings/shared/sections/SettingsGroup.tsx:119`（`handleChange` catch）与 `:139`（`handleIntermediateChangeComplete` catch）：
  - `log.error(...)` 保留（诊断留痕），**追加** `msg.showFailed(e)`（`msg` 已在 `:57`，`useSettingMessage` 无需改）
  - **回滚**：`await useSettingsStore.getState().loadConf(group)`（`group` 是组件入参）。`useConfGroup.ts` 只有 `useConfGroup` / `getConfValue` 两个纯读函数，**没有** reload 能力，所以规格里「给 `useConfGroup` 加 reload」这一半不成立，回滚直接调 store；`SettingsGroup.tsx` 需补 `import { useSettingsStore } from "@/stores/settingsStore";`
  - 回滚本身失败会被 `settingsStore.loadConf` 内部 `log.error` 吞掉（`settingsStore.ts:39`）——不重复弹二次提示，由 S3-b 的 `confErrors` 承载
- [x] `handleIntermediateChange`（`:123`）的乐观更新（`optimisticConfUpdate`）在 `onPatchComplete` 失败时同理由 `:138` catch 分支回滚，确认 `loadConf` 会把乐观值覆盖回落库值
- [x] 若 `onPatch` 由外部传入（`externalOnPatch`），回滚路径不变：仍以 store 为准

**验收**：断开后端（或临时让 `patch_conf` 返回 `Err`），改任一开关 → 出现失败提示，且开关值回到落库值（不是停在乐观值）；控制台有一条 `patch failed for <key>`。

### S3-b T4 Store 读取失败必须有可见状态

> 统一形状：`xxxError: string | null`，动作开始置 `null`、成功置 `null`、catch 置错误文案；UI 在「空数据」分支**之前**判 `xxxError`。

- [x] `src/stores/historyStore.ts`：
  - state 增 `messagesError: string | null`（初值 `null`）
  - `selectConversation`（`:46-61`）：开始 `set({ ..., messagesError: null })`；`:56` catch 内 `set({ messagesError: translateKey("history.load_messages_failed") })`
  - `refreshMessages`（`:67-82`）：同形状（开始清、`:79` catch 置）
  - `clearSelection`（`:63`）一并清 `messagesError`
- [x] `src/history/detail/HistoryDetail.tsx`：**组件是纯 props 型**（不直连 store），需两处配套：
  - `HistoryDetailProps`（`:23-35`）加 `messagesError?: string | null` 与 `onRefreshMessages: () => void`
  - `:284`（`messages.length === 0 ? <Empty .../>`）**之前**插入 `messagesError` 分支——失败占位 + 「重试」按钮（调 `onRefreshMessages`），文案用新增键
  - `HistoryManager.tsx`：`:31-59` 已取 `refreshMessages`（`:43`）；补 `const messagesError = useHistoryStore((s) => s.messagesError)`，并在 `:362-373` 的 `<HistoryDetail ...>` 传 `messagesError={messagesError}` / `onRefreshMessages={() => void refreshMessages()}`
- [x] `src/stores/chat/chatConversationSlice.ts`：
  - state 增 `conversationsError: string | null`，并在同文件 `ChatConversationSlice` 接口（`:51`）声明
  - 新增动作 `setConversationsError: (msg: string | null) => void`（供第二条加载路径写入）
  - `loadConversations`（`:116-127`）：`if (!silent) set({ isLoadingConversations: true, conversationsError: null })`；成功清（`:119-120` 附近）；`:122` catch 置 `translateKey("history.load_conversations_failed")`（`finally` 在 `:124-126`，保持不动）；**本文件无 `translateKey` import**（`:1-31` 已核对），需补 `import { translateKey } from "@/i18n";`
- [x] `src/stores/historyStore.ts:93-100`（**第二条会话列表加载路径**，`loadConversations` → `chatService.listConversations` → `useChatStore.updateConversations`；catch 在 `:97`）：catch 内同步 `useChatStore.getState().setConversationsError(translateKey("history.load_conversations_failed"))`，否则历史页刷新失败仍然静默；**本文件无 `translateKey` import**（`:1-6` 已核对），需补
- [x] `src/history/sidebar/HistorySidebar.tsx:276-280`（`groups.length === 0` 的 `Empty` 分支，`description` 在 `:278`）：之前插入错误占位；`HistorySidebarProps`（`:18`）加 `conversationsError?: string | null`，由 `HistoryManager.tsx:332` 的 `<HistorySidebar` 从 `useChatStore((s) => s.conversationsError)` 透传；列表非空时不显示（陈旧错误不遮挡数据）
- [x] `src/stores/settingsStore.ts`：
  - state 增 `confErrors: Partial<Record<ConfGroup, string>>`（初值 `{}`）
  - `loadConf`（`:24-41`）：成功时清该组 key；`:38` catch 内写入 `translateKey("settings.load_conf_failed")`（**本文件无 i18n import**，需补 `import { translateKey } from "@/i18n";`）
- [x] `src/settings/shared/sections/SettingsGroup.tsx:102`：`if (!conf) return null;` → 先判 `confErrors[group]` 渲染分组级失败占位 + 重试（`loadConf(group)`），无错误才 `return null`。否则读失败时整组设置「凭空消失」，用户无从察觉
- [x] `src/stores/providerStore.ts`：
  - state 增 `providersError: string | null`
  - `loadProviders`（`:38-47`）：开始 `set({ loading: true, providersError: null })`；成功清；`:43-45` catch 置 `translateKey("settings.provider.load_failed")`（**本文件无 i18n import**，需补 `import { translateKey } from "@/i18n";`）
- [x] `src/settings/provider/ProviderManager.tsx`：`<EntityListContent>`（`:177`）**之前**插入失败占位 + 「重试」（`void loadProviders()`）；仅当 `providerStore.providers.length === 0 && providerStore.providersError` 时展示（列表非空不遮挡已有数据）；文案直接取 `providerStore.providersError`（已是译好的字符串，无需再 `t()`）
- [x] `src/vault/core/slices/vaultSyncSlice.ts`：**复用已有设施，不新增字段/键**
  - 6 个读取动作的 catch（`loadSyncConfig:214`、`loadRemoteStatus:237`、`loadSyncReadyness:254`、`loadSyncConflicts:263`、`loadPendingRemovals:273`、`loadSyncLastTime:282`）在 `log.error` 之后补 `failWith(e)`（`:82` 定义，置 `error`/`errorCode`）
  - 可见性已就绪：`VaultSyncSettings/index.tsx:75` 取 `error: s.error`，`SyncNotices.tsx:59-62` 用 `syncErrorText(errorCode, error)` 渲染 error Alert，无需新 UI
  - **不要**改 `syncError`（`:24`，仅 `failSync` 用于「同步动作失败」，会被 `deriveSyncDisplay` 判成 `failed`，把读取失败混进去会误报同步状态）
  - `sshParseKey:512` / `sshKeyMeta:521` 改**抛错**（当前返回 `null` 与「无数据」混淆）：catch 内 `log.error` 保留后 `throw e`，接口类型（`:64`/`:65`）从 `Promise<SshKeyMeta | null>` 改为 `Promise<SshKeyMeta>`
- [x] 调用点同步（改抛错后必须补 catch，否则变成 unhandled rejection，比现在更糟）：
  - `src/vault/entry/forms/SshKeyFields.tsx:36`：`handleBlur` 包 `try/catch`，catch 内 `setMeta(null); setParseFailed(true);`（沿用现有 `parseFailed` 渲染分支，**不新增** i18n）
  - `src/vault/entry/VaultEntryDetail.tsx:120`：`.then(...)` 追加 `.catch((e) => log.warn("sshKeyMeta failed", entryId, e))`（本文件无 logger 则新增 `createLogger("vaultEntry")`）
  - `src/vault/core/__tests__/vaultService.ssh.test.ts:29/30/51/54`：断言改为 `rejects` 或补 catch 后再断言 `log.error`，按既有用例风格改
- [x] i18n：`src/i18n/locales/zh-CN.json` 与 `en-US.json` 各新增 **4** 键（键位按现有分组落位，避免顶层膨胀；均已核对当前不存在）
  - `history.load_messages_failed` / `history.load_conversations_failed`（`history` 根下，与 `history.export_failed`（zh `:1289`）同级，该块起于 `:1269`）
  - `settings.load_conf_failed`（`settings` 根下，作为 `settings.not_found`（zh `:88`，对象）的字符串兄弟键）
  - `settings.provider.load_failed`（`settings.provider` 块内（zh `:769` 起），紧邻既有 `settings.provider.empty`（zh `:773`））
  - `I18nPath` 由 `zh-CN.json` 编译期推导（[index.ts](file:///g:/codeversion/assistant3/src/i18n/index.ts#L34-L46) 的 `DeepKeys<typeof zhCN>`），**无需**跑生成脚本；改完 `tsc` 即校验键名
  - 同步项的读取失败**不需要**新键：复用 `error` + `syncErrorText`（见上）
- [x] 单测（沿用各 store 现有 `__tests__` 风格）：每个读取动作 2 例（成功 → `xxxError === null`；`vi.mocked(service).xxx.mockRejectedValue(...)` → 文案非空）

**验收**：断后端后逐项复现——历史消息失败有占位+重试；会话列表失败有占位；设置分组失败有占位；provider 列表失败有占位；同步各面板失败有提示；SSH 密钥解析失败仍显示「解析失败」且控制台有 `log.error`。

### S3-c T9 toolStore 日志级别

- [x] `src/stores/toolStore.ts` 5 处 `log.debug` → `log.warn`（`:71` loadFavorites、`:94` toggleFavorite、`:112` reorderFavorites、`:122` loadRecentTools、`:133` recordUsage）：
  - 属「后台自动动作」，按规格口径 4 至少 warn
  - 其中 `:71/:94/:112` 是 localStorage 回退路径，文案补「fallback」语义（原文本已含，原样保留即可），不新增 `error` 字段（回退成功不算用户可见故障）
  - `:122/:133` 补上下文（`toolId` 等既有局部变量），便于定位

**验收**：规格 V6——`rg -n "log\.debug" src/stores/toolStore.ts` 为空；单测/手动触发一次工具收藏失败，日志级别为 warn。

---

## 阶段 S4 — Rust 与后台动作（规格 T6 / T7 / T8 / T8b / T11）

退出条件：规格 V3 / V7 通过——持久化与安全判定路径不再降级；后台动作至少 warn；`src-tauri` 内无「无注释的 `let _ =`」。

### S4-a T6 `clipboard_guard.rs`（实际位置在 `assistant-security` crate，规格 N18 写的 `src-tauri/src/` 已漂移）

- [x] `src-tauri/crates/assistant-security/src/clipboard_guard.rs:129`：`let _ = thread::spawn(...)` → 保留，补 `// best-effort: 定时清理线程，失败不影响复制结果`（属线程创建，非 IO 失败）
- [x] 同文件 `:147`：`let _ = backend.write(...)` → `if let Err(e) = ... { warn!("clipboard write failed: {e}") }`，成功走 `debug!`（带耗时）
- [x] 同文件 `:150`：`let _ = backend.clear()` → 同形状 warn（clear 失败 = 用户剪贴板残留密码，必须留痕）
- [x] `should_clear == false` 分支（不清空）走 `debug!`，属正常路径，不升级

**验收**：`rg -n "let _ = backend" src-tauri/crates/assistant-security/src/clipboard_guard.rs` 为空。

### S4-b T7 `tree_ops.rs`（查询失败 vs 无分支）

- [x] `src-tauri/crates/assistant-storage/src/conversations/tree_ops.rs`：`find_new_active_leaf`（`:150` 起，签名在 `:154`）返回类型 `Option<String>` → `Result<Option<String>, String>`；内部 5 处吞没全部 `?`：
  - `:162` `.unwrap_or(false)`（sibling_exists）→ `.map_err(|e| e.to_string())?`
  - `:171-172` `.ok().flatten()`（sibling_id）→ `.map_err(|e| e.to_string())?`
  - `:175` `find_branch_leaf(conn, &sid).ok()` → 去掉 `.ok()`，直接 `return find_branch_leaf(...)`（返回类型已改）
  - `:186-187` `.ok().flatten()`（any_msg）→ 同上
  - `:190` `find_branch_leaf(conn, &mid).ok()` → `Some(mid) => find_branch_leaf(conn, &mid)`，`None => Ok(None)`
- [x] 同文件调用方 `:128`：`let new_leaf = find_new_active_leaf(...)` → `let new_leaf = find_new_active_leaf(&tx, conv_id, parent_id.as_deref())?;`
- [x] 同文件 `:113-114` `.ok().flatten()`（current_ab）→ `.optional().map_err(|e| e.to_string())?.flatten()`（`query_row` 泛型为 `Result<Option<String>>`，`.optional()` 只把 `QueryReturnedNoRows` 转 `None`，真实错误仍上抛）；`:1` 的 `use rusqlite::{params, Connection}` 补 `OptionalExtension`（`msg_ops.rs:1` / `branch.rs:1` 已有同款 import）
- [x] 同文件 `:123` `.unwrap_or(false)`（still_exists）→ `.map_err(|e| e.to_string())?`
  - 语义：查询失败**不得**被当成「分支不存在」而把 `active_branch_id` 置 `NULL`（规格 N17 的静默数据损坏路径）
- [x] 新增/扩展用例：`src-tauri/crates/assistant-storage/tests/` 下补 2 例——正常删分支后 `active_branch_id` 指向剩余叶子；查询层失败（可用关闭连接 / 构造坏 schema）时返回 `Err` 而非清空

**验收**：`cargo test --test <对应测试文件>` 通过（cwd = `src-tauri`）；`rg -n "\.ok\(\)|unwrap_or\(false\)" src-tauri/crates/assistant-storage/src/conversations/tree_ops.rs` 为空。

### S4-c T8 `git_sync/remote.rs`

- [x] `src-tauri/crates/assistant-sync/src/git_sync/remote.rs:138`：`let _ = git::git_config_branch_set_remote(&self.repo_path, branch, remote)` → `git::git_config_branch_set_remote(...)?`（remote 设置是同步配置写入，属持久化，不可降级）
- [x] 失败语义：向调用方返回带上下文的 `Err`（含 remote 名与 branch），由既有命令层统一转 `String`

**验收**：`rg -n "let _ = git_config" src-tauri/crates/assistant-sync/src` 为空；现有 sync 用例通过。

### S4-d T8b `compaction.rs`

- [x] `src-tauri/src/cmd/compaction.rs:390`：`let Ok(conn) = super::get_db_conn(&state) else { return }` → else 分支补 `log::error!("compaction: get_db_conn failed, msg_id={}", msg_id)` 后再 `return`（保持「失败不阻断压缩流程」语义）
- [x] 同文件 `:403`：`let _ = conversations::update_message_content(...)` → `if let Err(e) = ... { log::error!("compaction persist failed: msg_id={} len={}: {e}", msg_id, partial.full_content.len()) }`（同函数内 token usage 失败已是 `log::error`，此处对齐）
  - 落点判定按规格四口径：这是历史写入（持久化），但规格 N26 明确「失败不阻断压缩流程」，故用 `log::error` 而非 `?`；**不得**改成 `?` 把压缩整体失败

**验收**：`rg -n "let _ = " src-tauri/src/cmd/compaction.rs` 为空；`cargo test --test compaction`（cwd = `src-tauri`）通过。

### S4-e T11 后台/窗口动作

- [x] `src-tauri/src/tray.rs`：`:110` / `:113` / `:127` / `:131` / `:135` / `:150` 共 6 处 `let _ = window::show_or_create(...)` → `if let Err(e) = ... { warn!("tray menu action failed: {e}") }`，日志带上目标窗口常量（`WINDOW_MAIN` / `WINDOW_ORB`）与菜单项 id
- [x] `src-tauri/src/menu.rs:15`：1 处 `window::show_or_create`，同上（菜单动作失败用户点不动，必须留痕）
- [x] `src-tauri/src/window.rs`：`:210`（`w.destroy()`）/ `:216`（`show_or_create_main`）/ `:573`（`persist_main_geometry`）/ `:579`（`window.hide()`）/ `:588`（`emit(ORB_BLUR)`）/ `:568`（`screenshot::cancel_active`）：
  - `:216` → warn + label；`:573` 持久化 → warn（调用方无法传播）；`:210` / `:579` / `:588` → warn（窗口动作失败用户会看到「点不动」）；`:568` 属取消动作，可 warn 或补 `best-effort:` 注释
- [x] `src-tauri/src/setup.rs`：
  - `:337` `let _ = conf.save()` → `if let Err(e) = ... { warn!(...) }`（配置持久化，不可静默）
  - `:230` `let _ = gs.unregister_all()` → warn（清理动作，可降级但需留痕）
  - `:187` / `:191` `window::toggle` 与 `:274/:276/:288/:333` `set_position` → warn（同类，批量处理，避免漏网）
- [x] `src-tauri/src/cmd/orb.rs`：`:119` / `:124` / `:128` / `:129` / `:161` / `:185` / `:197` / `:204` → 位置持久化与 `conf.save` 一律 warn（持久化路径），`set_position` 同类 warn
- [x] `src-tauri/crates/assistant-crash/src/crash_dump.rs:140` / `:154`：`let _ = fs::write(...)` → `if let Err(e) = ... { eprintln!("crash dump write failed: {e}") }`（崩溃路径不能依赖日志系统，用 `eprintln!`）
- [x] `src-tauri/crates/assistant-vault/src/security_db/biometric_ops.rs` 另有 5 处 `let _ = key_store.delete_key(...)`（`:195` / `:291` / `:302` / `:303` / `:304`）：属**清理动作**（口径 2 可降级），以 `// best-effort:` 注释豁免（G2 通过同行/上一行注释识别，未登记白名单）；`:195` 的 `let _ = on_credential_ensured` 属回调占位，同样加注释豁免
- [x] `src-tauri/crates/assistant-providers/src/openai/discovery.rs:16`（`None => continue`）与 `:49`（`Err(_) => continue`）：改为 `Err(e) => { skipped += 1; debug!("discovery skip entry: {e}"); continue; }`，循环结束后**一次性** `warn!("discovery skipped {n} entries")`（避免刷屏，规格 N28）

> **口径边界**：G2 / V3 的 Rust 正则只兜 `write|clear|fs::write|with_conn` 一类 IO 吞没，**抓不到** `conf.save()`（`orb.rs:204` / `setup.rs:337`）、`set_position` / `toggle` / `destroy` / `hide` / `emit` / `unregister_all` / `delete_key` 等形态。这些由本节人工清单逐条收口，**不能**指望门禁兜底；S4 完成后可把该正则扩到 `save\(|delete_key` 再收紧一次（可选，非必须）。

**验收**：`node scripts/check-error-swallow.mjs` exit 0（仅白名单/`best-effort:` 命中）；`rg -n "let _ = (write|clear|fs::write|with_conn)" src-tauri --glob '!**/tests/**' --glob '!**/examples/**'` 为空；`cargo clippy` 无新告警。

---

## 阶段 S5 — P0 与契约（规格 T1 / T1b / T1c / T2）

退出条件：规格 V1 通过——审计失败不阻断业务且有二级痕迹；`VaultErrorCode` 四码端到端可达且前端两张镜像表同步。

### S5-a T1 审计丢弃点

- [x] `src-tauri/crates/assistant-vault/src/security_db/biometric_ops.rs:151`：`let _ = state.with_conn(|conn| { log_dual(...); Ok(()) })` → 该路径是审计写入，**不得**丢弃：
  - 改为 `if let Err(e) = state.with_conn(...) { log::warn!("audit log_dual failed: {e}") }`（`with_conn` 返回 `Result<T, VaultError>`，定义在 `state.rs:379`）
  - `log_dual` 本身返回 `()`（见 S5-b），因此这里唯一失败源是取会话（保险库已锁），warn 即符合口径；本文件**没有** `use log::...`，既有写法是 `log::warn!` 全限定（`:78` / `:179` / `:249` / `:256`），照此写法即可，**不要**新增 `use log::warn;`

### S5-b T1b 审计双通道失败二级痕迹

- [x] `src-tauri/crates/assistant-vault/src/audit/jsonl.rs` `log_dual`（`:331-349`）：保持返回 `()`（规格 §4 已从类型上禁止传播）
  - 新增模块级 `static DB_FAILURES: AtomicU64` / `static JSONL_FAILURES: AtomicU64`（文件顶部与既有 `use` 块之后；该文件当前无 `AtomicU64` 导入，需补 `std::sync::atomic::{AtomicU64, Ordering}`）
  - `:341`（db 写失败）与 `:347`（jsonl 追加失败）两条 `warn!` 改为「首次失败 warn 全文 + 计数」，或「每 N 次 warn 一次并附累计数」（二选一，需在注释里写明策略），避免持续失败时刷屏
  - 保留 `at = crate::types::now_millis()`（`:337`）单次取值的既有约束（DB 行与 JSONL 行必须同值，否则 §2.5 去重失效）
- [x] 用例（`src-tauri/crates/assistant-vault/tests/` 或 `src/audit` 内 `#[cfg(test)]`）：双通道都失败时计数递增且 `log_dual` 不 panic、返回 `()`

**验收**：`cargo test`（vault crate）通过；人为制造 jsonl 写入失败后计数可观测（测试断言即可）。

### S5-c T1c 持久化失败上下文

- [x] `src-tauri/crates/assistant-agent/src/kernel/turn/runner.rs:140`：`log::error!("terminal persist failed: {err}")` → 补三个 id，格式统一为 `terminal persist failed: conv={} turn={} msg={}: {err}`
  - 作用域内可用：`conversation_id`（入参）、`ctx.turn_id`、`ctx.message_id`（`:113-115` 的 `PersistCtx` 已从这三者构造，直接引用即可，无需新增参数）

**验收**：`rg -n "terminal persist failed" src-tauri` 命中的行含三个 id。

### S5-d T2 `VaultErrorCode` 四码端到端

> 本条含契约生成，**必须独立提交**；改 DTO 后不跑 `contract:gen` 会直接破坏类型检查。

- [x] `src-tauri/crates/assistant-vault/src/dto/error.rs:44`：`VaultErrorCode` 在 `Internal` 之前新增 4 个变体 `Database` / `Io` / `Clipboard` / `AlreadyUnlocked`（`Internal` 保持末位兜底；`#[serde(rename_all = "camelCase")]` 下 wire 值为 `database` / `io` / `clipboard` / `alreadyUnlocked`）；`is_retryable`（`:48-49`）**不改**（新码均不可重试）
- [x] `src-tauri/crates/assistant-vault/src/dto/convert/error.rs`：`:10`（`AlreadyUnlocked => Internal`）、`:16`（`Database(_) => Internal`）、`:20`（`Clipboard(_) => Internal`）、`:21`（`Io(_) => Internal`）改为精确映射，`:38`（`Other(_) => Internal`）保持 `Internal`；不得把安全判定类错误降级成 `Internal`
  - 同时检查同文件 `message` 分支（`:40` 起）是否需为新码补文案；既有 `VaultError::Database` 已有 `"Vault database error"`（`:42`），可复用
- [x] `src/vault/core/display/vaultErrorText.ts`：`ERROR_TEXT_KEYS`（`satisfies Record<VaultErrorCode, I18nPath>`）新增 4 键：`database → "vault.error.database"`、`io → "vault.error.io"`、`clipboard → "vault.error.clipboard"`、`alreadyUnlocked → "vault.error.alreadyUnlocked"`（编译期会强制补齐，漏项即红）
- [x] `src/vault/core/display/vaultError.ts`：`VAULT_ERROR_CODE_FLAGS`（`satisfies Record<VaultErrorCode, true>`）同步新增 4 项
- [x] i18n `zh-CN.json` / `en-US.json` 的 `vault.error` 各新增 4 键：
  - `database`: 本机数据库访问失败
  - `io`: 文件读写失败
  - `clipboard`: 剪贴板操作失败
  - `alreadyUnlocked`: 保险库已处于解锁状态
- [x] `zh-CN.json` / `en-US.json` 的 `vault.error.invalidPassword` 文案改为「密码错误，或保险库数据已损坏」（对齐规格：解锁失败同时覆盖 `InvalidPassword` 与 `VaultCorrupted` 的判定歧义）
- [x] `src-tauri/crates/assistant-crypto/src/vault_crypto/mod.rs:122-138` 注释同步说明该歧义（注释-only，不改逻辑；`decrypt` 失败一律映射 `InvalidPassword`（`:129`），但明文长度异常映射 `VaultCorrupted`（`:136-138`））
- [ ] `npm run contract:gen`，提交生成产物 `src/types/protocol.contract.ts` 与 `src/vault/core/contract/vault.contract.ts`（**禁止手改生成文件**）
  - 生成已完成（两文件已更新且 `npm run check` 的 contract 阶段通过），仅剩「独立提交」这一步未执行
- [x] 单测：`vaultErrorText` 对 4 个新码返回对应键；`vaultError` 的 flags 对 4 个新码为 `true`

**验收**：规格 V1——`rg -n "database|clipboard|alreadyUnlocked" src/vault/core/contract/vault.contract.ts` 命中；`npm run check`（含 `contract` 阶段）通过。

---

## 阶段 S6 — 可选（规格 T12）

退出条件：OQ-3 有结论后再定；未定前不开工。

- [ ] 若 OQ-3 采纳：按规格 T12 描述补「吞没统计上报」通道（复用 S5-b 的 `AtomicU64`，出口由 OQ-3 决定）；若不采纳，本节标记为「不做」并回写规格

---

## 判据映射（验收跑什么）

| 判据 | 落点 | 命令 / 手段 |
|---|---|---|
| V1 | S5-d | `npm run check`（contract 阶段 + 前端两张镜像表编译期漏项检查） |
| V2 | S1 | `rg -n "\.catch\(\(\) => \{\}\)" src` 为空；`node scripts/check-error-swallow.mjs` exit 0 |
| V3 | S4 | `rg -n "let _ = .*(write|clear|fs::write|with_conn)" src-tauri --glob '!**/tests/**' --glob '!**/examples/**'` 为空（仅保留惯用法 `let _ = (a, b)` / `thread::spawn` 与 `best-effort:` 注释行） |
| V4 | S3-a | 断开后端改设置：失败提示 + 值回滚 |
| V5 | S3-b | 断后端逐个面板：失败占位 + 重试 |
| V6 | S3-c | `rg -n "log\.debug" src/stores/toolStore.ts` 为空 |
| V7 | S4 | `cargo test`（storage / sync / vault / agent 各自相关用例）+ `cargo clippy` |
| G1 | S2-a | 临时插一行空 catch → `npm run lint` 红 |
| G2 | S2-b | `npm run check` 多一步 `吞没`，失败带 `文件:行` |
| G3 / G4 | 全阶段 | 按规格原文执行（本清单不重复） |

---

## 残留 grep 清单（整改完成后仍允许命中，需逐条理由）

- `src/services/system/logger.ts`：唯一 `console.*` 出口
- `src/**/__tests__/**`：`console` spy / `vi.mocked` 断言
- `src-tauri/**/tests/**`、`**/examples/**`：测试与示例
- `src-tauri/crates/assistant-security/src/clipboard_guard.rs:129`：`let _ = thread::spawn`（`best-effort:` 注释）
- 平台 FFI 调用（`windows` / `objc` 绑定返回值）：白名单登记在 `scripts/check-error-swallow.mjs` 顶部，逐条注释理由

---

## 开放问题状态

| 编号 | 内容 | 状态 | 影响 |
|---|---|---|---|
| OQ-1 | 前端 5 处静默吞没的日志级别（warn vs debug） | 本清单按 S1-a 逐条定级，已在条目内写明 | S1-a |
| OQ-2 | 审计双通道失败的重试/补录 | 归 I64（已归档） | 不影响本清单 |
| OQ-3 | 是否新增吞没统计上报通道 | 未决 | S6 / S5-b 计数出口 |
| OQ-4 | `VaultErrorCode` 新增四码的 wire 兼容性 | 仅新增变体、不改既有值，前端 `ERROR_TEXT_KEYS` 编译期强制补齐 | S5-d |
| OQ-5 | 门禁白名单范围 | 先最小白名单（FFI + 测试），后续按误报再增 | S2-b |
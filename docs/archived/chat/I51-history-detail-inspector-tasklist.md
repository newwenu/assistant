# I51 — 历史页检视能力 任务清单

> 规格：[I51-history-detail-inspector-spec.md](./I51-history-detail-inspector-spec.md)
> 事实基线：规格 §1.3 的 H1–H8（编写规格时对 `src/history/`、`src/chat/message/`、`src/services/`、`src-tauri/crates/assistant-storage/` 的实读结果）
> 前置：无。M1 零后端改动、零契约变更；M2 新增 `MessageRecordDto` 并把原文行并入 `get_conversation` 详情返回，必须 `npm run contract:gen`。

批次对应规格章节：M1.1 身份归并（§2）→ M1.2 元数据行（§3）→ M1.3a 记录快照（§4.4）→ M1.3b 单视图改造（§4.1–§4.3 / §4.6）→ M1.4 导出接线与命名修正（§5.1 / §4.2）→ M1.5 测试与文案门禁（§9）→ M2.1 原文 DTO 与详情合并（§6.1 / §6.2）→ M2.2 导出内容增强（§5.2 / §5.3 / §5.4）→ M2.3a 双视图来源切换（旧形态已拆除）→ M2.3b 面板来源三态（§4.5 / §6.3）→ M3 请求体采集（§7，阻塞，需独立规格）。

依赖：M1.1 必须先于 M1.2 / M1.3b（元数据行与原文面板的挂载位置以「身份行 → 元数据行 → 内容 → 面板」的层序为前提）；M1.3b 依赖 M1.2 的行结构与 CSS 口径（`.rawPanel` 与 `.metaLine` 同属 `HistoryBranchView.module.css`）；M2.3b 依赖 M2.1、M2.2 与 M1.3b；M1 与 M2 可分批合并，M1 不得夹带 `src-tauri/` 改动（门禁 G11）。

> **2026-11-19 规格变更**：规格 §4 已由「整屏双视图（阅读 / 原始）」改为「单视图 + 行内折叠原文面板」，理由是双视图下滚动锚点、行标识、记录形状三处无法建立审查对应关系（见规格 §4.1）。按旧双视图完成的 M1.3 / M2.3 已返工完毕：`viewMode` 链路、分段控件、原始模式整屏分支全部拆除，改挂行内面板（M1.3b / M2.3b）。M2.3a 的条目仅作为能力沿革保留。

> 本清单不管理提交。提交粒度按代码依赖与可编译性现场决定；这里只回答「代码改没改、验收跑没跑」。
> 全部任务初始未实施；勾选表示代码已改且对应验收已跑。

---

## 阶段 M1 — 前端检视层（零契约变更）

### M1.1 身份行按 turn 归并（G1 / G2）

对应规格 §2.2；验收走 §9.2 第 1–6 条。

- [x] `src/history/detail/historyTree.ts`：`HistoryMessageRow` 新增派生字段 `showIdentity: boolean`（禁止进 store、禁止进 DTO、禁止进 `HistoryRow` 的持久结构）
- [x] 同文件 `flattenHistoryRows` 末尾加一次线性 pass：只扫 `kind === "message"` 的 `HistoryRow`，维护 `seenTurn: Set<number>`
- [x] `role !== "assistant"` → `showIdentity = false`；`role === "assistant"` 且 `turnIndex == null` → `true`（存量行不猜测）；`row.dimmed` → `false`（旁支不抢身份）；其余取 `!seenTurn.has(turnIndex)` 并登记该 turn
- [x] 禁止用「上一行是否同 turn」判定（可见序列被 stub 行隔开）；禁止用 `timestamp` 或父链位置推断轮次
- [x] `src/history/detail/HistoryBranchRow.tsx`：`MessageBubble` 显式传 `showIdentity={row.showIdentity}`，禁止继续依赖缺省 `true`
- [x] `src/chat/message/MessageBubble.tsx`：仅改 `showIdentity` 的 JSDoc 为「仅在无坐标上下文的调用方使用；历史页必须显式传值」；DOM / 默认值 / 行为零变化
- [x] 确认 `MessageBubble` 的 `compact` 未在历史页被引入（密度档位归 I46 §5，本文不动）

**依赖**：无

### M1.2 元数据行（§3）

对应规格 §3；验收走门禁 G4 与 §9.2 的 `recordSnapshot` 用例之外的渲染手测。

- [x] 新增 `src/history/detail/MessageMetaLine.tsx`，props 为 `message` + `defaultExpanded`（禁止接 store、禁止接 actions）
- [x] props 扩展 `rawOpen?: boolean` + `onToggleRaw?: () => void`（§3.1）；未传 `onToggleRaw` 即不渲染开关（stub 行），禁止把开关做成必传
- [x] 布局改横向：chips 区 `flex: 1` 可换行、右侧固定原文开关；开关与 chips 是**同级**按钮，禁止嵌进 chips 的 `<button>` 内（点击会被 chips 的展开/收起吞掉，且按钮嵌套非法）
- [x] 把 `MessageMetaLine.tsx` 内的 `formatStamp` 抽到 `src/history/detail/formatStamp.ts`，作为历史页唯一时间格式化函数（短模式 `HH:MM:SS` / 完整模式 `YYYY-MM-DD HH:MM:SS`），供元数据行与 `HistoryStubRow` 共用（G15）
- [x] 折叠态按规格 §3.2 输出 chips：坐标 → tokens → 状态 → 模型 → 通道 → 图钉，缺值整段省略
- [x] 坐标段：`turnIndex == null` 时整段渲染 `history.detail.coord_unknown`，禁止渲染成 `T0 · i0`；真实 `iteration === 0` 必须正常显示
- [x] 状态段：`completed` 省略，`interrupted` / `failed` / `streaming` 显示本地化短名
- [x] `modelId` / `providerId` 一律显示原始 id，禁止解析为展示名（身份解析归 `messageIdentity.ts`，本行不做二次解析）
- [x] 展开态用 `<dl>` 输出规格 §3.3 固定字段表；`content` / `reasoning` 只给 `{chars} chars`，`toolCalls` 只给 `{count} calls`，禁止展开正文
- [x] `timestamp` 折叠态时分秒、展开态 `YYYY-MM-DD HH:MM:SS`（本地时区）；两种模式必须由 `formatStamp` 同一函数产出，禁止在组件内再写一份 `toLocaleTimeString`
- [x] 挂载在 `HistoryBranchRow` 内、`MessageBubble` 之上；`role === "tool"` 的原始工具行同样挂载
- [x] 禁止把元数据行放进 `MessageBubble`（保证主会话 DOM 零变化，门禁 G3）
- [x] 折叠态高度与现状行高同量级（实测约 18px：`.metaChips` 的 `line-height: 16px` + `.metaLine` 的 `margin-bottom: 2px`），禁止为元数据行引入固定高度或 `ESTIMATED_ROW_HEIGHT` 特例（行高由 `HistoryVirtualList` 的 `ResizeObserver` 实测兜底）
- [x] 新增样式 `.metaLine` / `.metaTable` 到 `src/history/detail/HistoryBranchView.module.css`（10px 等宽、`--color-text-tertiary`）

**依赖**：M1.1

### M1.3a 记录快照（§4.4，已完成）

对应规格 §4.4；验收走门禁 G6 与 §9.2 的 `recordSnapshot.test.ts`。

- [x] 新增 `src/history/detail/recordSnapshot.ts`，导出纯函数 `toRecordSnapshot(message: Message): Record<string, unknown>`
- [x] 键序与 `MessageDto` 一致；缺值字段输出 `null`，禁止 `undefined`（`JSON.stringify` 会整键丢弃）
- [x] `toolCalls` 投影为 `{ callId, name, arguments, status, result, compact }`，剔除 `argsText` / `argsPreview` / `startedAt` / `finishedAt` / `approval` / `question` / `errorKind` / `confirmationWaitMs`
- [x] 禁止在 `historyStore` / `buildHistoryTree` / `flattenHistoryRows` / 任何 selector 中调用 `JSON.stringify`
- [x] 复制语义：面板复制本行记录 JSON；`MessageActions` 的复制仍走 `buildCopyText`，两个 tooltip 必须区分（`common.actions.copy` vs `history.detail.copy_record`）

**依赖**：无

### M1.3b 单视图改造（§4.1–§4.3 / §4.6，已完成 2026-11-19）

对应规格 §4.1–§4.3；验收走门禁 G5 / G9 / G13 与 §4.3 的结构约束逐条核对。

拆除旧双视图：

- [x] 删除 `HistoryViewMode` 类型与 `viewMode` prop 链路（`HistoryDetail` / `HistoryBranchView` / `HistoryBranchRow` 三处全拆，禁止留下 `viewMode` 残留，G13）
- [x] 删除 `HistoryDetail` 工具栏的 `Segmented` 分段控件（只保留「显示工具行」开关）；同步删除 `history.detail.view_read` / `view_raw` 文案键（`zh-CN.json` / `en-US.json`）
- [x] 删除 `HistoryBranchRow` 的 `viewMode === "raw"` 整屏分支，包括 rawHeader 中重复的 `role` / 时间戳 / 坐标 / id 组合

改挂行内面板：

- [x] `HistoryBranchRow` 新增行内 `rawOpen` 局部状态（默认 `false`，不进 store、不进 `HistoryRow` 的持久结构），面板挂在气泡**下方**、与气泡同属 `bubbleWrap`，`data-row-key` 不变
- [x] `recordJson` 的 `useMemo` 依赖改为 `[message, recordState.record, rawOpen]`，折叠态直接返回 `""`（折叠行零序列化，§4.6）
- [x] 面板结构按 §4.3：`Tag{来源标签}` + 等宽 `id` + 复制按钮 + `<pre>`；**禁止**重复 `role` / 时间戳 / 坐标（这三项由元数据行唯一承载，消除同一消息两种时间格式）
- [x] 气泡侧同步关闭时间戳：`MessageBubble` 新增 `showTimestamp?: boolean`（默认 `true`，聊天页行为不变），历史行显式传 `false`，避免气泡与元数据行出现两种时间格式
- [x] 面板内**禁止**渲染 markdown / 推理块 / 工具卡片；`role === "tool"` 行继续用 `rawRow` 做结构化展示，与面板并存（§3.4 末条）
- [x] `HistoryStubRow` 不加面板；其预览时间改走 `formatStamp`（删除 `toLocaleTimeString`）
- [x] `HistoryBranchRow` 内残留的 `toLocaleString()` 全部替换为 `formatStamp`（G15）
- [x] CSS：新增 `.rawPanel` / `.rawId` / `.metaHeader` / `.metaToggle`，`.rawRow` 保留给 tool 行；面板以 `--color-border` 竖线贴附气泡下方，`dimmed` 行继承整体 `opacity`，不额外降透明度；禁止复制一份同义 class
- [x] 禁止提供「全部展开」全局开关（一次展开 N 行等于重新引入整屏位移，§4.2）
- [ ] 手测：逐行展开 / 收起后面板与气泡同属一行、滚动位置不跳；`HistoryVirtualList` 的 `heightsRef` 仍按行缓存单一高度，展开只更新该行条目

**依赖**：M1.2

### M1.4 导出接线与命名修正（G7 / G8 的 UI 侧）

对应规格 §5.1、§4.2；验收走手测（两种 format 都落到 `exportConversationToFile`，后缀正确）。

- [x] `src/history/detail/HistoryDetail.tsx`：导出按钮改 `Dropdown`，两项 Markdown / JSON，分别 `onExport(selectedId, "markdown" / "json")`
- [x] 复用既有 `history.export_markdown` / `history.export_json` 文案，禁止新增同义键
- [x] 两条路径都必须走既有 `save()` 对话框 + `historyService.exportConversationToFile`，禁止在详情页新增第二套保存逻辑
- [x] 删除 `history.tree.raw_rows` 文案键（`zh-CN.json` / `en-US.json` 同步删），确认全仓无残留引用
- [x] 侧栏 `ConversationListItem` 的导出菜单保持现状（已含 md / json），不改动
- [ ] `HistoryDetail` 工具栏删掉分段控件后，导出 Dropdown 与「显示工具行」开关同处一个 header 的布局复核（禁止出现空位或换行错位）

**依赖**：M1.3b（工具栏改动与导出菜单同处一个 header，拆开会冲突）

### M1.5 测试与文案门禁（§9）

对应规格 §9.2 / §9.3；验收命令见 §9.3。

- [x] `src/history/detail/__tests__/historyTree.test.ts` 补 6 条身份归并用例：单 turn 单 assistant、单 turn 三 iteration、两 turn、兄弟分支展开（`dimmed` 行 false）、全 `null` 坐标、非 assistant 角色
- [x] 新增 `src/history/detail/__tests__/recordSnapshot.test.ts`：键序一致、`toolCalls` 投影无本地字段、缺值为 `null`、纯函数幂等
- [x] 新增 `history.detail.*` 文案到 `zh-CN.json` 与 `en-US.json`，键集必须一致
- [x] 全仓检索新增中文字面量，确认无硬编码（门禁 G10）
- [x] 两 locale 全文检索「原始模型消息 / raw model request」零命中（门禁 G12）；文案一律写「落库记录 / 记录原文」
- [x] 确认 `git diff --stat` 不含 `src-tauri/` 与 `src/types/protocol.contract.ts`（门禁 G11）：M1 改动本身未触及这两处；工作区其余 `src-tauri/` 与 `protocol.contract.ts` 改动来自并行的 MCP / 审计任务，非本批次引入
- [x] 跑 `npm run lint:fix` → `npm run typecheck` → `npm test`：eslint 与 prettier 均无改动项，`tsc --noEmit` 通过，`vitest run` 73 文件 / 863 用例通过（2026-11-19 单视图改造后复跑）
- [x] 门禁 G13：全仓零 `viewMode` / `view_read` / `view_raw` 残留（`src/history/` 已清零，其余命中在 tools/text-diff 与 mascot 的 `viewModel`，与历史页无关）；`HistoryBranchRow` 源码中面板与气泡同属 `bubbleWrap`（结构断言）
- [x] 门禁 G15：`src/history/` 内 `toLocaleString` / `toLocaleTimeString` 零命中（`MessageBubble` 的历史行已传 `showTimestamp={false}`，不再产出第二种时间格式）
- [x] 新增 `src/history/detail/__tests__/formatStamp.test.ts`：短模式 / 完整模式格式与补零（用本地构造的固定时间戳，不依赖运行时时区）
- [ ] 补 `MessageMetaLine` 用例（可选，若无测试基建则手测）：未传 `onToggleRaw` 时不渲染开关；chips 与开关为同级按钮

**依赖**：M1.1–M1.4

---

## 阶段 M2 — 后端原文能力（契约变更）

### M2.1 `MessageRecordDto` 与详情查询合并（§6.1 / §6.2）

对应规格 §6.1 / §6.2；验收走后端单测 + `npm run contract:check`。

- [x] `src-tauri/src/protocol/dto/chat.rs` 新增 `MessageRecordDto`，字段严格按规格 §6.1 表
- [x] `Option` 输出字段**不加** `skip_serializing_if`（键恒在、值 `null`）；`deny_unknown_fields` 只加在输入 DTO 上，本 DTO 是输出专用，禁止作为任何命令入参
- [x] `From<MessageRow>` 显式降级：`u32::try_from(...).ok()` / `iteration` 用 `unwrap_or(0)`，越界不 panic
- [x] **不新增独立命令**：`ConversationDetailDto` 增 `records: Option<Vec<MessageRecordDto>>`，`ConversationQueryOptsDto` 增 `include_records: bool`（`#[serde(default)]`，默认 `false`）；`get_conversation` 按开关填充，其余查询模式键恒在、值为 `null`。理由：逐条 IPC 在展开原文面板时会产生 N 次调用，合并到详情查询后一次拉全
- [x] `src-tauri/src/protocol/mod.rs` 补 re-export（已验证无需改动：`dto/mod.rs` 已 `pub use chat::*`、`protocol/mod.rs` 已 `pub use dto::*`）
- [x] `src-tauri/tests/protocol_bindings.rs` 的 `declarations` 注册 `MessageRecordDto::decl(&cfg)`（**硬依赖**：该测试会扫描 `src/protocol/dto/*.rs` 的 `#[derive(..., TS)]` 结构体并与 `declarations` 双向比对，漏注册直接失败）
- [x] `ConversationDetailDto` 加字段后同步 `tests/protocol_contract.rs`、`tests/protocol_dto_roundtrip.rs` 的结构体字面量
- [x] `src/services/historyService.ts` 新增 `getMessageRecords(conversationId)`（调 `get_conversation` 的 `mode: "all"` + `includeRecords: true`，只返回 `records`）
- [x] 跑 `npm run contract:gen`，提交 `src/types/protocol.contract.ts`，禁止手改生成物

**依赖**：M1（无代码依赖，但同一模块的 `recordSnapshot.ts` 抽象要先在 M1 定稿）

### M2.2 导出内容增强（§5.2 / §5.3 / §5.4）

对应规格 §5.2 / §5.3 / §5.4；验收走 `cargo test -p assistant-storage --test conversations`（markdown / JSON 模板）+ `cargo test --lib cmd::conversation`（JSONL 组装）。

- [x] `assistant-storage/src/conversations/ops.rs::export_conversation_markdown` 改固定模板：标题 + 元信息头 + `## {role} · T{turnIndex} i{iteration} · {本地时间}`
- [x] `reasoning` 以引用块输出；`turn_index IS NULL` 输出 `T?`，`iteration` 仍按落库值输出，禁止编造序号
- [x] 工具调用以表格输出（call / status / 单行 `arguments` JSON / 结果前 500 字 + 截断标记）
- [x] 多模态内联图片替换为 `![image omitted]` 占位 + 块计数，禁止把 base64 写进 md（派生在 `content_codec::content_markdown_text`，与全文检索用的 `content_plain_text` 分开，后者口径不动）
- [x] 时间戳改本地时间且带日期（现状 markdown 无任何时间信息）
- [x] `export_conversation_json` 外层包 `{ "schema": "assistant.conversation.raw.v1", ... }`，禁止把 `schema` 塞进 `ConversationDetail` 结构体本身
- [x] JSONL 每行一条 `MessageRecordDto`，首行是 schema 元信息（`schema` / `conversationId` / `messageCount`）。**落点定稿**：`assistant-storage` 不依赖 `assistant-app`，无法复用该 DTO，故组装在 `src-tauri/src/cmd/conversation.rs` 的 `export_conversation_jsonl` 内完成（`get_conversation_detail` → `MessageRecordDto` → 逐行 `serde_json::to_string`），不在 `ops.rs` 内重复定义镜像结构
- [x] `src-tauri/src/cmd/conversation.rs` 的 `export_conversation` 增加 `"jsonl"` 分支；未知 format 仍回落 markdown（现状行为不变）
- [x] JSONL 只从详情页 / 侧栏菜单进入（`HistoryDetail` 导出菜单 + `ConversationListItem` 右键菜单新增 `history.export_jsonl`），批量导出保持 md / json

**依赖**：M2.1（JSONL 直接复用 `MessageRecordDto`）

### M2.3a 双视图来源切换（§6.3，旧形态已随 M1.3b 拆除）

> 以下条目按旧双视图形态完成，触发条件曾是 `viewMode === "raw"`；M1.3b 已删除该链路，现由 M2.3b 的「首次展开触发」接管。保留记录只为说明能力沿革，代码以 M2.3b 为准。

- [x] `recordSnapshot.ts` 增加来源枚举 `RecordSource = "dto" | "raw"`，并新增 `toRawRecordJson(record, message)`（有原文用原文，无原文退回快照）
- [x] 进入原始模式时调用 `historyService.getMessageRecords(conversationId)` 一次性拉全会话原文，结果缓存于 `HistoryBranchView` 的 `Map<id, MessageRecordDto>`（**不得**放 `HistoryBranchRow`：虚拟列表会卸载滚出窗口的行，行内缓存会被反复丢弃），禁止进全局 store；会话切换时随 `HistoryDetail` 的 `selectedId` effect 一并清空
- [x] 拉取失败降级回 `"dto"` 并在块内标注失败（来源标签转 `error` 色 + Tooltip 展示原因），禁止静默空白
- [x] 块头部显示来源标签（`history.detail.source_dto` / `source_raw`）
- [x] 详情页新增 JSONL 导出入口（若 M2.2 已提供后端能力）

**依赖**：M2.1、M2.2

### M2.3b 面板来源三态（§4.5 / §6.3，已完成 2026-11-19）

对应规格 §4.5 状态表；验收走门禁 G14 逐行核对。

- [x] `RecordSource` 扩为三态 `"loading" | "raw" | "dto"`，删除「拉取成功但无记录」与「拉取失败」共用同一外观的做法：`loading` 与 `dto` 必须可区分
- [x] `HistoryBranchView` 拉取触发条件由 `viewMode === "raw"` 改为「任一行首次展开」（由行组件 `onRequestRecords` 回调上报，视图层用 `loadedForRef` 去重），仍一次拉全、仍缓存于常驻视图层 `Map`
- [x] `loading` 期间面板展示快照内容但标签走 `history.detail.source_loading`（中性色），复制按钮禁用
- [x] `error` 态标签用 `source_dto` + `error` 色 + Tooltip `history.detail.source_raw_failed`；`dto` 态（拉取成功但该 id 无记录）用默认色
- [x] 失败不留「已加载」标记：再次展开时允许重试（旧实现靠重新进入原始模式触发，改造后靠再次展开触发）
- [x] 行组件入参由 `record` / `recordError` 改为携带状态（`recordState: { source, record, error }`，类型定义在 `recordSnapshot.ts` 的 `HistoryRecordState`），禁止用 `record == null` 反推状态
- [x] 两 locale 新增 `history.detail.source_loading`，删除 `history.detail.view_read` / `view_raw`（若 M1.3b 未删）
- [ ] 手测：断网 / 后端拒绝时展开面板，标签转 `error` 色且 Tooltip 显示原因；恢复后再次展开能重试成功

**依赖**：M1.3b、M2.1、M2.2

---

## 阶段 M3 — 请求体采集（阻塞，需独立规格）

对应规格 §7；本阶段不在 I51 验收范围，仅固定准入条件。

- [ ] 独立规格评审通过后才可开工；禁止在 I51 内直接实现
- [ ] 新表 `chat_requests`（`id` / `conversation_id` / `message_id` / `attempt` / `provider_id` / `model_id` / `payload_json` / `created_at`），禁止复用 `messages` 表加列
- [ ] 默认关闭开关 + 容量上限 + 保留策略 + 落盘前脱敏（密钥 / `Authorization` 头 / `data:` 内联图截断）
- [ ] 采集失败不得阻断请求（沿用 I50「审计不阻断业务」的失败语义）
- [ ] 删除消息 / 删除会话 / 清空会话时请求记录同步删除（外键或事务内级联）
- [ ] 请求体只在原文面板按行展开时展示，禁止批量注入导出文件
- [ ] 解除规格 §7.1 文案禁令前，必须完成上述全部条目

**依赖**：独立规格（本清单不排期）
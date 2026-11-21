# I51 — 历史页检视能力规格（身份口径 / 元数据 / 行内原文面板 / 导出）

| 项 | 值 |
|---|---|
| 状态 | Draft（实施规格，不含进度记录） |
| 日期 | 2026-11-19 |
| 依据 | 现状缺口 H1–H8 见 §1.3，全部来自本次对 `src/history/`、`src/chat/message/`、`src/services/`、`src-tauri/crates/assistant-storage/` 的实读；身份口径冲突见 §2.1 |
| 落地对象 | 前端：`src/history/detail/`、`src/chat/message/`（仅新增可选 prop）、`src/services/historyService.ts`、`src/stores/historyStore.ts`、`src/i18n/locales/`；后端（仅 M2 / M3）：`src-tauri/src/cmd/conversation.rs`、`src-tauri/crates/assistant-storage/src/conversations/`、`src-tauri/src/protocol/dto/` |
| 关联 | I46 节点布局规格（已归档）（坐标判据与 `turn` 语义唯一出处）、I46 紧凑消息格式设计（已归档）（`compact` 信封，仍在用）、I40-history-message-tree-spec（已归档，树渲染基线）、I50 运行时审计规格（已归档）（相邻域，**口径不通用**：审计是安全事件，本文是会话内容） |
| 替代 | I43-assistant-identity-spec（已归档）§4.4 N7 中「历史树详情每条 assistant 行都渲染身份指示」一条；其余 N1–N6、N8–N11 继续有效 |

本文中的「必须 / 禁止」为可验收约束；「应当」为默认做法，偏离需在代码注释里写明理由。

---

## 1 范围与判定口径

### 1.1 定位

历史页（`src/history/`）当前是「可读的历史记录浏览器」：它能完整重放对话内容，但**不能**回答「这条消息是怎么产生的、花了多少 token、坐标在哪、落库原文长什么样」。本文把它补齐为**只读检视视图（inspector）**。

三个必须守住的边界：

1. **只读**：历史页新增的所有交互不得产生写库操作，唯一例外是既有「删除消息」；
2. **不猜测**：任何无法从落库数据得到的信息（`turn_index IS NULL` 的存量行、请求体），UI 必须显式标注「未知」，禁止用相邻关系或当前选中项推断（沿用 I46 与 I43 的禁止项）；
3. **不与主会话分叉**：同一份 `Message` 在聊天页与历史页的**正文渲染**必须来自同一套组件，历史页只允许叠加元数据层与原始层，禁止复制一份渲染逻辑。

### 1.2 信息三层

| 层 | 内容 | 数据来源 | 现状 |
|---|---|---|---|
| L0 渲染层 | 正文 markdown、推理块、工具卡片、审批卡、压缩摘要标记、中断标记 | `Message` 前端对象（`MessageBubble` 一族） | 已有，可用 |
| L1 元数据层 | 坐标（`turnIndex` / `iteration`）、`tokens`、`status`、`providerId` / `modelId` / `modeId`、`id`、`parentId`、`siblingIndex` / `siblingCount`、`isPinned`、完整时间戳 | 同上（全部字段已在 `MessageDto` 上） | **有数据、无 UI** |
| L2 原始层 | 落库行的逐列原文（`content` 原串、`tool_calls_json` 原串、`tool_call_id`、`is_error` 等 `MessageDto` 未承载的列） | 需后端新增只读命令（M2） | **无数据出口** |
| L3 请求层 | 实际发给 provider 的请求体（system prompt、tool schema、wire 消息） | 不落库 | **无数据源，非本文目标**（见 §7） |

### 1.3 现状缺口

| 编号 | 缺口 | 落点 | 归属 |
|---|---|---|---|
| H1 | 历史页依赖 `MessageBubble` 的缺省 `showIdentity = true`，每条 assistant 行都渲染头像 + 模式 + 模型；主会话已按 turn 归并（I46 §4.3），两页视觉不一致 | `src/history/detail/HistoryBranchRow.tsx`、`src/chat/message/MessageBubble.tsx` | M1 |
| H2 | 历史树是分支树，`groupNodeRows` 的坐标判据不适用；缺少「按 turn 归并」的历史页专用判据 | `src/history/detail/historyTree.ts` | M1 |
| H3 | `turnIndex` / `iteration` 已落库并在 `MessageDto` 上，历史页零展示 | 同上 | M1 |
| H4 | `tokens` 已落库，全局无任何消息级消费方 | `src/chat/message/MessageBubble.tsx` | M1 |
| H5 | 工具栏「原始行」开关（`history.tree.raw_rows`）真实语义只是「显示 `role === tool` 的链式行」，与「原始」无关；命名误导 | `src/history/detail/HistoryDetail.tsx`、`src/i18n/locales/` | M1 |
| H6 | 无原文出口：`tool_calls_json` 原文在前端被 `toChatMessage` 解析为视图后即丢弃，`arguments` 只保留可解析快照 | `src/chat/core/toolCallView.ts` | M2 |
| H7 | 详情页导出按钮硬编码 `"markdown"`，后端 `export_conversation` 已支持 `"json"` 却无人调用；markdown 导出只输出 `content_plain_text`，丢弃 `reasoning` / 工具调用 / 坐标 / 时间戳 | `src/history/detail/HistoryDetail.tsx`、`assistant-storage/src/conversations/ops.rs` | M1（UI）/ M2（内容） |
| H8 | 侧栏导出菜单与详情页导出能力不一致（侧栏有 JSON，详情页没有） | `src/history/sidebar/ConversationListItem.tsx` | M1 |

### 1.4 非目标

- 不做历史页写操作（编辑 / 重新生成 / 分叉 / 切换分支），`MessageBubble` 的这些回调在历史页继续保持不传；
- 不做跨会话检索（`search_history` 工具仍属 roadmap 待排期项）；
- 不改分支链拓扑、不改树渲染的折叠语义、不改 `HistoryVirtualList` 的虚拟化算法；
- 不做整屏视图切换：记录原文是行内折叠面板（§4.1），不新增第二条整屏渲染路径；
- 不做请求体采集（§7 单列为可选阶段，需独立规格）；
- 不把元数据层做成「审计日志」：本页不承载任何安全事件语义（与 I50 的 `security_audit_log` / `permission_audit` 无交集）。

---

## 2 身份行口径（M1）

### 2.1 冲突与裁决

I43-assistant-identity-spec（已归档）N7 规定「历史树详情不存在轮次概念，每条 assistant 行都渲染身份指示，不得把 N2 套用到历史树」。该判断写于 `turn_index` 落库之前（I46 S1 才引入坐标），前提已失效：`turn_index` 现已由后端在写入路径上分配（`cmd/chat/insert.rs` 的三条写路径，user 行 `iteration = 0`、assistant 行 `iteration >= 1`），历史页拿得到真实轮次。

**裁决**：历史页改为「同一 turn 内仅首个 assistant 行显示身份行」，与主会话可观测行为一致。N7 作废，N1–N6 与 N8–N11 继续有效。

### 2.2 判据（历史页专用）

历史页不走 `groupNodeRows`（那是聊天页的投影，输入是 `MessageTree` + turn 容器）。历史页的判据落在 `flattenHistoryRows` 的**输出后处理**上，理由有二：

- 历史树的可见顺序是 DFS 前序 + 旁支折叠，同一 turn 的节点在可见序列里可能被 stub 行隔开，不能用「前一行是不是同 turn」判定；
- 旁支展开后，非焦点路径的行是 `dimmed`，不应抢占该 turn 的身份行。

必须实现的规则（对 `rows` 线性扫描一次，`O(n)`）：

```ts
// 伪代码，落地为 flattenHistoryRows 末尾的纯函数 pass
const seenTurn = new Set<number>();
for (const row of rows) {
  if (row.kind !== "message") continue;
  const m = row.node.message;
  if (m.role !== "assistant") { row.showIdentity = false; continue; }
  if (m.turnIndex == null) { row.showIdentity = true; continue; }   // 存量行：不猜测，逐行显示
  if (row.dimmed) { row.showIdentity = false; continue; }           // 旁支不抢身份
  row.showIdentity = !seenTurn.has(m.turnIndex);
  seenTurn.add(m.turnIndex);
}
```

三条硬约束：

1. `HistoryMessageRow` 必须新增 `showIdentity: boolean` 字段（展示层派生字段，不进 store、不进 DTO）；
2. `HistoryBranchRow` 必须显式传 `showIdentity={row.showIdentity}`，**禁止**继续依赖 `MessageBubble` 的缺省 `true`（H1 的根因就是依赖缺省值）；
3. `turnIndex === null` 的存量行必须回退为「逐行显示」，禁止用 `timestamp` 或父链位置推断轮次（I46 §1.3「不回填存量行坐标」的延伸）。

`MessageBubble` 的 `showIdentity` 缺省值保持 `true`，但其 JSDoc 必须改为「仅在无坐标上下文的调用方使用；历史页必须显式传值」。

### 2.3 归并后的视觉口径

- 同一 turn 内 `iteration >= 2` 的 assistant 行（agent 循环的后续迭代）不显示身份行，只保留正文与 chrome；
- 身份行的头像尺寸沿用历史页现状（`--msg-avatar-size: 20px`，即不传 `compact`）；**禁止**在本文中改动密度档位，密度归 I46 §5；
- 同 turn 的首个 assistant 行若被删除（`delete_subtree`），身份行自然落到该 turn 剩余的首个 assistant 行——这是纯函数派生的结果，不需要额外补偿逻辑。

---

## 3 元数据层（M1）

### 3.1 组件

新增 `src/history/detail/MessageMetaLine.tsx`（历史页专用，禁止放进 `src/chat/message/`，避免主会话被默认挂载）。

```ts
interface MessageMetaLineProps {
  message: Message;
  /** 元数据是否展开为键值表；由行内局部状态持有，不进 store。 */
  defaultExpanded?: boolean;
  /** 原文面板开关状态（§4.2）。未传 onToggleRaw 即不渲染开关（stub 行）。 */
  rawOpen?: boolean;
  onToggleRaw?: () => void;
}
```

布局：`chips 区（flex: 1，可换行） + 右侧固定开关`。chips 区自身是元数据展开/收起按钮；原文开关是**同级**的独立按钮，二者禁止嵌套。

### 3.2 折叠态（默认）

单行 chips，10px 等宽字体，`--color-text-tertiary`，按以下顺序排列，缺值项**整段省略**（不是显示 `null`）：

| 段 | 取值 | 缺值规则 |
|---|---|---|
| 坐标 | `T{turnIndex} · i{iteration}` | `turnIndex == null` → 显示「坐标未知」（`history.detail.coord_unknown`），**不省略**，因为它标记了降级事实 |
| tokens | `{tokens} tok` | `tokens == null` → 省略 |
| 状态 | `status` 的本地化短名 | `completed` → 省略（默认态不占位）；`interrupted` / `failed` / `streaming` → 显示 |
| 模型 | `modelId`（原始 id，不解析为展示名） | 空 → 省略 |
| 通道 | `providerId`（原始 id） | 空 → 省略 |
| 标记 | `isPinned` 时显示图钉图标 | 未置顶 → 省略 |

坐标段必须能区分「真实 0」与「未知」：`iteration` 恒为数字，`turnIndex` 为 `null` 时整段降级为「坐标未知」，禁止渲染成 `T0 · i0`。

### 3.3 展开态

点击 chips 区切换为键值表，字段与顺序固定：

| 键 | 值 | 说明 |
|---|---|---|
| `id` | 消息 id，等宽，可选中复制 | |
| `conversationId` | 会话 id | |
| `role` | 原始角色串 | 含 `compact_summary` / `tool` |
| `turnIndex` / `iteration` | 数字或「未知」 | |
| `parentId` | 父 id 或「根」 | |
| `siblingIndex` / `siblingCount` | `n / m`；`siblingCount == null` → `n / ?` | |
| `status` | 原始枚举串 | |
| `tokens` | 数字或「—」 | |
| `providerId` / `modelId` / `modeId` | 原始 id 或「—」 | |
| `timestamp` | `YYYY-MM-DD HH:MM:SS`（本地时区，秒级） | 折叠态用同函数的短模式 `HH:MM:SS`；两种模式都由 `formatStamp` 产出（G15） |
| `content` | `{chars} chars · {blocks} blocks` | **禁止**展开正文，正文由气泡承载 |
| `reasoning` | `{chars} chars` | 同上 |
| `toolCalls` | `{count} calls` | 同上 |

展开态必须使用 `<dl>` 语义标签；数值与 id 使用等宽字体。

### 3.4 挂载位置

- 挂载在 `HistoryBranchRow` 内、`MessageBubble` **之上**（身份行之下），行内顺序为：身份行（气泡内部）→ 元数据行 → 气泡 → 原文面板（§4.3）；
- 元数据行**必须**位于 `HistoryBranchRow` 而非 `MessageBubble` 内部，保证主会话 DOM 零变化（门禁 G3）；
- 元数据行横向排布：chips 区可换行、占满剩余宽度，右侧固定「原文」开关（§4.2），开关不得与 chips 的展开按钮共用同一 `<button>`；
- `role === "tool"` 的原始工具行在 M1 保持现状（`rawRow` 渲染），但必须同时挂元数据行与原文面板（`rawRow` 是结构化结果，面板是逐列原文，二者同属一行、不互斥）。

### 3.5 与虚拟列表的协作

`HistoryVirtualList` 用 `ResizeObserver` 按 `data-row-key` 测量行高（`src/history/detail/HistoryVirtualList.tsx`）。展开 / 折叠元数据行会改变行高，测量自动修正，**禁止**为元数据行引入固定高度或 `ESTIMATED_ROW_HEIGHT` 特例。折叠态默认高度必须与现状行高同量级（±8px），避免首屏跳动。

---

## 4 记录原文层（M1 / M2，单视图）

### 4.1 设计裁决：取消「阅读 / 原始」两视图

初版规格把原始层做成整屏替换的第二视图（工具栏 `Segmented`「阅读 / 原始」）。落地后实读代码，该结构在三处无法建立审查对应关系，且都不是可局部修补的缺陷：

1. **滚动锚点丢失**：`HistoryDetail` 只保存像素 `scrollTop`，`HistoryVirtualList` 的行高缓存按 `data-row-key` 存且跨模式不失效，未测量行退回 `ESTIMATED_ROW_HEIGHT`。原始行高是气泡行的数倍 → 切到原始落到更早的消息，切回阅读被夹到底部；`rows.length <= VIRTUAL_SCROLL_THRESHOLD` 的非虚拟化路径根本不做测量，错位无法自愈。
2. **行标识不共享**：阅读模式折叠态 chips 不含 `id`，原始块头部含 `id`；原始块时间走 `toLocaleString()`，元数据行走 `formatStamp`；坐标 `T{turn} · i{iteration}` 本身不唯一（`compact_summary` 与 `tool` 行可同坐标）。跨模式只能靠「数第几条」重建对应。
3. **记录形状突变**：落库原文 `MessageRecordDto` 携带 `toolCallsJson` / `toolCallId` / `isError`，前端快照 `toRecordSnapshot` 携带已解析的 `toolCalls`；拉取在「进入原始模式」时才发起，返回前整屏显示快照、返回后整屏键集切换，且加载中与降级态外观相同 → 复制出来的可能是快照而被当成落库事实。

**裁决**：删除视图模式切换。原始层改为挂在**同一行内、默认折叠、逐行展开**的面板。对应关系由 DOM 位置直接给出（同 `data-row-key`、同缩进、同轨道色），不需要任何跨视图匹配机制。

### 4.2 交互与行内顺序

工具栏只保留「显示工具行」开关（`history.detail.show_tool_rows`，语义等于原 `rawRows`：`role === "tool"` 的链式行是否作为独立行渲染，默认关）；删除「视图」分段控件，`history.detail.view_read` / `view_raw` 文案键随之删除，禁止保留同义重复键（旧 `history.tree.raw_rows` 的删除依据不变：其真实语义只是工具行可见性，与原文无关）。

行内顺序固定：身份行（气泡内部）→ 元数据行（chips + 原文开关）→ 气泡 → 记录原文面板。

- 原文开关是元数据行右侧的独立图标按钮，**禁止**嵌进 chips 的 `<button>` 内（按钮嵌套非法，点击会被 chips 的展开/收起吞掉）；每行独立状态，默认折叠；
- **禁止**提供「全部展开」全局开关：一次展开 N 行会让视口内所有行高同时变化，等于重新引入整屏位移；逐行展开只改一行，其它行偏移不变，无需滚动补偿；
- `HistoryVirtualList` 的 `heightsRef` 保持「每行一个高度」的单一口径（不区分视图状态），展开只更新该行条目；
- `stub` 行（`HistoryStubRow`）**不提供**原文面板：它不是消息行，只有折叠占位语义，继续渲染 `extractDerivedText` 摘要。

### 4.3 面板结构

记录原文面板挂在气泡**下方**，与气泡同属一个行容器：

```tsx
<div className={css.bubbleWrap}>
  <MessageMetaLine message={message} />   {/* 含右侧原文开关 */}
  <MessageBubble ... />                   {/* 或 tool 行的既有载体 */}
  {rawOpen && (
    <div className={css.rawPanel}>
      <div className={css.rawHeader}>
        <Tag>{sourceLabel}</Tag>
        <span className={css.rawId}>{message.id}</span>
        <CopyButton disabled={source === "loading"} />
      </div>
      <pre className={css.rawBody}>{recordJson}</pre>
    </div>
  )}
</div>
```

约束：

- 面板**禁止**重复 `role` / 时间戳 / 坐标：这三项由同一行的元数据行唯一承载（消除同一条消息出现两种时间格式）；
- 气泡侧必须关闭自身时间戳（历史行显式传 `MessageBubble` 的 `showTimestamp={false}`，默认 `true` 只服务聊天页），元数据行是历史页唯一的时间出口；
- `id` 等宽、可选中复制；来源标签按 §4.5 的状态表取值；
- 面板以左侧竖线（`--color-border`）贴附气泡下方；`dimmed` 行整体继承 `opacity`，不额外降透明度；
- 面板与气泡同属一个 `data-row-key`，虚拟列表只测量一个整体高度；分支缩进、轨道色、焦点徽标、「删除消息」入口均不变（面板只换内容载体，不动树结构）；
- `recordJson` 必须由 `JSON.stringify(record, null, 2)` 生成，等宽字体，`white-space: pre-wrap`，长行折行（沿用现状 `.rawRow` 的 CSS 口径）；
- 面板内**禁止**渲染 markdown / 推理块 / 工具卡片；气泡继续由 `MessageBubble` 承载。二者是同一行的上下两层，不再互斥。

初版规格「原始模式下禁止渲染 markdown、推理块、工具卡片」的禁令在此废止（该条针对的是整屏替换视图，会双重呈现同一屏内容）；行内面板的双层对应关系由位置直接给出，双呈现即为检视目的。

### 4.4 记录快照形状（M1）

M1 无后端改动，快照由前端 `Message` 投影而来。**禁止**直接 `JSON.stringify(message)`：`Message` 携带前端本地字段（`error`、`toolCalls[].argsText`、`startedAt` 等），直接序列化会把运行期状态混进「落库记录」。

必须新增纯函数 `toRecordSnapshot(message: Message): Record<string, unknown>`（落点 `src/history/detail/recordSnapshot.ts`），规则：

1. 键序固定，与 `MessageDto` 字段顺序一致（便于 diff）；
2. `toolCalls` 必须投影为 DTO 形状 `{ callId, name, arguments, status, result, compact }`，剔除 `argsText` / `argsPreview` / `startedAt` / `finishedAt` / `approval` / `question` / `errorKind` / `confirmationWaitMs`；
3. 缺值字段**必须**输出 `null`（键恒在），禁止 `undefined`（`JSON.stringify` 会整键丢弃）；
4. 函数必须纯、无副作用、不缓存结果。

### 4.5 来源状态机（M2）

`RecordSource` 由 `"dto" | "raw"` 扩为四态。加载态必须可与降级态区分——旧实现两者外观相同，会把快照误当落库事实：

| 状态 | 触发 | 标签 | 复制 |
|---|---|---|---|
| （不渲染面板） | 行未展开 | — | — |
| `loading` | 已发起拉取、未返回 | `history.detail.source_loading`（中性色） | 禁用 |
| `raw` | 命中 `MessageRecordDto` | `history.detail.source_raw`（主色） | 落库原文 |
| `dto` | 拉取成功但该 id 无记录 | `history.detail.source_dto`（默认色） | 前端快照 |
| `error` | 拉取失败 | `history.detail.source_dto` + `error` 色 + Tooltip `history.detail.source_raw_failed` | 前端快照 |

- 拉取时机：**首次展开任一行**时触发一次，结果缓存于 `HistoryBranchView` 的 `Map<id, MessageRecordDto>`（**必须挂在此常驻组件**，虚拟列表会卸载滚出窗口的行）；会话切换时随 `HistoryDetail` 的 `selectedId` effect 清空；
- 失败不留「已加载」标记：再次展开时允许重试，避免一次瞬时失败让该会话永久停在降级态；
- 禁止静默空白：任何状态下展开都必须有内容（落库原文或前端快照）；
- `loading` 期间展示快照内容但标签明确标注，**禁止**用 `source_dto` 标签冒充已降级。

### 4.6 序列化惰性

`recordJson` 只在该行**展开时**于行组件内 `useMemo` 生成（依赖 `message` 引用、展开态与记录引用）。折叠行零序列化开销——比初版「原始模式下序列化整个可见窗口」更强。

**禁止**在 `historyStore`、`buildHistoryTree`、`flattenHistoryRows` 或任何 selector 中预先序列化记录。

### 4.7 复制

面板头部的复制按钮复制**该行**记录 JSON；`MessageActions` 的既有复制按钮继续复制 `buildCopyText(message)`（消息正文的可读文本：多模态消息按块派生 `[图片]` / `[文件 name]`，失败消息附错误标题与详情），**禁止**混入记录 JSON。两个复制入口必须在 tooltip 上区分（`common.actions.copy` vs `history.detail.copy_record`）。

---

## 5 导出（M1 UI / M2 内容）

### 5.1 详情页导出格式菜单（M1）

`HistoryDetail` 头部导出按钮改为 `Dropdown`，两项：

| 项 | `format` 实参 | 文案键 | 后缀 |
|---|---|---|---|
| Markdown | `"markdown"` | `history.export_markdown` | `.md` |
| JSON | `"json"` | `history.export_json` | `.json` |

两条路径都必须走既有的 `save()` 对话框 + `historyService.exportConversationToFile`（`src/history/HistoryManager.tsx`），**禁止**在详情页新增第二套保存逻辑。M1 只修 UI 接线（H7 / H8），不改后端行为。

### 5.2 Markdown 导出内容（M2）

现状 `export_conversation_markdown` 只写 `## {role}` + `content_plain_text`，必须扩展为固定模板：

```markdown
# {title}

> provider: {provider_id} · model: {model_id} · mode: {mode_id}
> exported at {本地时间}

## {role} · T{turnIndex} i{iteration} · {本地时间}

{content_plain_text}

<!-- 以下小节按存在性输出，缺值整节省略 -->

### Reasoning

> {reasoning 逐行加 "> "}

### Tool calls

| call | status | arguments | result |
|---|---|---|---|
| {name} | {status} | `{arguments 单行 JSON}` | {result 前 500 字，超出标 …} |
```

约束：

- 时间戳必须是本地时间且带日期（现状 markdown 无任何时间信息）；
- `turn_index IS NULL` 的存量行坐标段输出 `T?`，`iteration` 仍按落库值输出（该列非空），**禁止**编造 `turnIndex`；
- 多模态 `content` 里的内联图片（`data:` 前缀）**必须**替换为 `![image omitted]` 占位 + 块计数，禁止把 base64 写进 md（现状经 `content_plain_text` 天然规避，扩展时必须保持）；
- 工具结果超过 500 字符截断并标注，避免导出文件爆量；
- 参数必须是单行 JSON（表格内不能换行）。

### 5.3 JSON 导出（M2）

`export_conversation_json` 现状序列化的是 `ConversationDetail`（`MessageRow` 结构，含 `tool_calls_json` 原串），**不是**前端 `MessageDto`。该差异必须在导出文件头部的 `schema` 字段里声明，避免消费方误判：

```json
{ "schema": "assistant.conversation.raw.v1", "conversation": {...}, "messages": [...] }
```

**禁止**把 `schema` 字段塞进 `ConversationDetail` 结构体本身（那是内部存储结构）；包装必须在 `export_conversation_json` 内完成。

### 5.4 原始 JSONL 导出（M2，可选）

新增 `format = "jsonl"`：每行一条 `MessageRecordDto`（§6.1），行序 = `get_messages` 的坐标排序。准入条件：

- 单行必须自包含（含 `conversationId`），可直接 `jq` 过滤；
- 必须带 `schema` 版本号：**首行写一条 schema 头**（`schema` / `conversationId` / `messageCount`），其后每行一条 `MessageRecordDto`。禁止把 `schema` 塞进 `MessageRecordDto`（它同时是 `get_conversation` 的返回类型，加版本字段会污染在线契约）；
- 该格式只允许从详情页与侧栏菜单进入，默认不展示在批量导出里（批量导出保持 md/json）。

---

## 6 后端原文能力（M2）

### 6.1 新 DTO

`src-tauri/src/protocol/dto/chat.rs`：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MessageRecordDto {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    /// 落库原文：可能是纯文本，也可能是 content blocks 的 JSON 串。
    pub content: String,
    pub reasoning: Option<String>,
    /// `tool_calls_json` 落库原串，未解析。
    pub tool_calls_json: Option<String>,
    pub tokens: Option<i64>,
    pub timestamp: i64,
    pub parent_id: Option<String>,
    pub sibling_index: i64,
    pub is_pinned: bool,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub mode_id: Option<String>,
    pub status: String,
    pub tool_call_id: Option<String>,
    pub is_error: bool,
    pub turn_index: Option<u32>,
    pub iteration: u32,
}
```

契约约束（沿用项目 DTO 规则）：

- 输出 DTO 的 `Option` **不加** `skip_serializing_if`，键恒在、值 `null`；
- 必须 `npm run contract:gen`，提交 `src/types/protocol.contract.ts`；
- `From<MessageRow>` 显式降级（`u32::try_from(...).ok()` / `unwrap_or(0)`），越界不 panic；
- 这是**输出专用** DTO，禁止作为任何命令的入参。

### 6.2 命令落点（并入 `get_conversation`，不新增命令）

原文行随会话详情一起返回，字段落在既有 DTO 上：

```rust
// ConversationDetailDto
pub records: Option<Vec<MessageRecordDto>>,

// ConversationQueryOptsDto
pub include_records: bool,   // #[serde(default)]，默认 false
```

约束：

- 只有 `include_records == true` 时填充 `records`；其余查询模式键恒在、值为 `null`（输出 `Option` 不加 `skip_serializing_if`）；
- 数据源仍是 `get_messages` / `get_conversation_detail` 已取回的 `MessageRow`，`MessageRecordDto::from` 直接复用，不额外查库；
- 会话不存在时 `get_conversation` 仍返回 `Ok(None)`，与既有空值口径一致；
- 前端服务落点 `src/services/historyService.ts`：`getMessageRecords(conversationId)`，内部以 `mode: "all"` + `includeRecords: true` 调用 `get_conversation`，只返回 `records`；
- **不新增独立命令**：逐条 IPC 在展开原文面板时会产生 N 次调用，合并到详情查询后一次拉全，且虚拟列表卸载行不会造成重复请求。

### 6.3 前端来源切换

`recordSnapshot.ts` 的来源枚举扩为三值（面板状态表见 §4.5）：

```ts
type RecordSource = "loading" | "raw" | "dto";
```

- `"raw"`（M2）：`getMessageRecords(conversationId)` 的结果（一次拉全会话，按 `id` 取用）；
- `"loading"`（M2）：拉取在途，面板展示快照内容但标签明确标注，复制禁用；
- `"dto"`（M1）：`toRecordSnapshot(message)`；同时承担「拉取成功但该 id 无记录」与「拉取失败」两种降级，失败原因由标签的 `error` 色 + Tooltip 承载；
- 面板头部必须显示来源标签（`history.detail.source_loading` / `source_raw` / `source_dto`），让使用者知道当前看到的是「前端快照」还是「落库原文」；
- 拉取时机是**首次展开任一行**（不是进入某个视图模式），一次拉全并缓存于 `HistoryBranchView` 内的 `Map<id, MessageRecordDto>`（不进全局 store），会话切换时随 `HistoryDetail` 的 `selectedId` effect 一并清空；
- 失败不留「已加载」标记：再次展开时允许重试，避免一次瞬时失败让该会话永久停在降级态。

---

## 7 请求体采集（M3，可选，默认关闭）

现状不存「实际发给 provider 的请求体」，因此任何 UI 都**不能**声称展示的是「原始模型消息」。

### 7.1 文案禁令（立即生效）

历史页所有新增文案必须写「落库记录 / 记录原文」，**禁止**使用「原始模型消息 / raw model request / 请求体」等措辞（`zh-CN` 与 `en-US` 同步）。这条禁令在 M3 落地前不解除。

### 7.2 准入条件（M3 若要立项，必须独立规格覆盖）

1. 新增独立表（建议 `chat_requests`：`id` / `conversation_id` / `message_id` / `attempt` / `provider_id` / `model_id` / `payload_json` / `created_at`），**禁止**复用 `messages` 表加列；
2. 必须有**默认关闭**的开关 + 明确容量上限与保留策略（按条数或按天），落盘前必须过脱敏（密钥、`Authorization` 头、`data:` 内联图截断）；
3. 采集失败不得阻断请求（沿用 I50「审计不阻断业务」的失败语义）；
4. 必须定义删除语义：删除消息 / 删除会话 / 清空会话时，请求记录同步删除（外键或事务内级联）；
5. 必须定义导出语义：M3 的请求体只在原文面板按行展开时展示，禁止批量注入导出文件。

M3 不在本文验收范围内，本文只固定准入条件。

---

## 8 文件落点与改动清单

### 8.1 前端（M1）

| 文件 | 改动 |
|---|---|
| `src/history/detail/historyTree.ts` | `HistoryMessageRow` 加 `showIdentity`；`flattenHistoryRows` 末尾加身份归并 pass（§2.2） |
| `src/history/detail/HistoryBranchRow.tsx` | 显式传 `showIdentity`；挂载 `MessageMetaLine`；行内新增记录原文面板（§4.3） |
| `src/history/detail/MessageMetaLine.tsx` | 新增（§3）；行尾独立原文开关按钮（§4.2） |
| `src/history/detail/formatStamp.ts` | 新增：历史页唯一时间格式化函数（§3.3 / G15），供元数据行与 stub 预览共用 |
| `src/history/detail/recordSnapshot.ts` | 新增 `toRecordSnapshot`（§4.4） |
| `src/history/detail/HistoryBranchView.tsx` | `rawRows` 更名为 `showToolRows`；不再承载视图模式 |
| `src/history/detail/HistoryDetail.tsx` | 工具栏只留「工具行」开关（§4.2）；导出改 Dropdown（§5.1） |
| `src/history/detail/HistoryBranchView.module.css` | 复用现有 `.rawHeader` / `.rawBody`（等宽、10px、`--color-text-tertiary` 已就位）；新增 `.rawPanel` / `.rawId` / `.metaLine` / `.metaTable`；`.metaLine` 改横向（chips 撑满 + 右侧开关） |
| `src/history/detail/HistoryStubRow.tsx` | 预览时间改走 `formatStamp`（去掉 `toLocaleTimeString`，G15）；不加原文面板 |
| `src/chat/message/MessageBubble.tsx` | 改 `showIdentity` 的 JSDoc（§2.2 约束 3）；新增 `showTimestamp?: boolean`（默认 `true`，历史行传 `false`，§4.3）；主会话默认路径 DOM 与行为零变化 |
| `src/i18n/locales/zh-CN.json`、`en-US.json` | 新增 `history.detail.*`；删除 `history.tree.raw_rows` |
| `src/history/detail/__tests__/historyTree.test.ts` | 补身份归并用例（§9.2） |
| `src/history/detail/__tests__/recordSnapshot.test.ts` | 新增（§9.2） |
| `src/history/detail/__tests__/formatStamp.test.ts` | 新增：短 / 完整模式的格式与补零（§9.2 / G15） |

### 8.2 后端（M2）

| 文件 | 改动 |
|---|---|
| `src-tauri/src/protocol/dto/chat.rs` | 新增 `MessageRecordDto` + `From<MessageRow>`；`ConversationDetailDto` 增 `records`、`ConversationQueryOptsDto` 增 `include_records` |
| `src-tauri/tests/protocol_bindings.rs` | `declarations` 注册 `MessageRecordDto::decl(&cfg)`（该测试扫描 `src/protocol/dto/*.rs` 的 `#[derive(..., TS)]` 结构体并与 `declarations` 双向比对，漏注册直接失败） |
| `src-tauri/tests/protocol_contract.rs`、`tests/protocol_dto_roundtrip.rs` | 同步 `ConversationDetailDto` / `ConversationQueryOptsDto` 的结构体字面量 |
| `src-tauri/src/cmd/conversation.rs` | `get_conversation` 按 `include_records` 填充 `records`（不新增命令）；扩展 `export_conversation` 的 `"jsonl"` 分支（JSONL 组装在此处完成）与 markdown 模板 |
| `src-tauri/src/ipc.rs` | 无需改动（无新命令） |
| `src-tauri/crates/assistant-storage/src/conversations/ops.rs` | markdown 导出扩写（§5.2）；JSON 导出加 `schema` 包装（§5.3）。**JSONL 不落此处**：`assistant-storage` 不依赖 `assistant-app`，无法复用 `MessageRecordDto`，禁止重复定义镜像结构 |
| `src-tauri/crates/assistant-storage/src/content_codec.rs` | 新增 `content_markdown_text`（导出专用派生：图片块 → `![image omitted]` 并计数）。**不复用 `content_plain_text`**：后者被全文检索与预览共用，占位口径不能因导出而改 |
| `src/services/historyService.ts` | 新增 `getMessageRecords(conversationId)`（`mode: "all"` + `includeRecords: true`） |
| `src-tauri/src/protocol/mod.rs` | 无需改动（`dto/mod.rs` 已 `pub use chat::*`，`protocol/mod.rs` 已 `pub use dto::*`） |
| `src/types/protocol.contract.ts` | `npm run contract:gen` 产物，禁止手改 |

### 8.3 前端（M2.2 / M2.3）

| 文件 | 改动 |
|---|---|
| `src/history/detail/recordSnapshot.ts` | 新增 `RecordSource` 枚举（`loading` / `raw` / `dto` 三态）与 `toRawRecordJson(record, message)`（有落库原文用原文，无原文退回 `toRecordSnapshot` 快照） |
| `src/history/detail/HistoryBranchView.tsx` | 新增 `conversationId` 入参；**首次展开任一行**时一次性拉全并缓存 `Map<id, MessageRecordDto>`（**必须挂在此常驻组件**，禁止挂行组件）；会话切换清空 |
| `src/history/detail/HistoryBranchRow.tsx` | 新增 `record` / `recordState` 入参；面板头部按 §4.5 输出来源标签与失败提示；`loading` 期间禁用复制 |
| `src/history/detail/HistoryDetail.tsx` | 透传 `conversationId`；导出菜单加 `jsonl` |
| `src/history/sidebar/ConversationListItem.tsx`、`src/history/HistoryManager.tsx` | 右键菜单加 `jsonl`；导出文件后缀 / filter 名按 format 分支 |
| `src/i18n/locales/zh-CN.json`、`en-US.json` | 新增 `history.export_jsonl`、`history.detail.source_loading` / `source_dto` / `source_raw` / `source_raw_failed`；删除 `history.detail.view_read` / `view_raw` |
| `src/history/detail/__tests__/recordSnapshot.test.ts` | 补 `toRawRecordJson` 的「用原文」与「退回快照」两条用例；补 `RecordSource` 三态取值 |

---

## 9 验收门禁

### 9.1 硬门禁

| 编号 | 门禁 | 判定方式 |
|---|---|---|
| G1 | 历史页同一 turn 内仅首个非 `dimmed` 的 assistant 行显示身份行 | 构造同 turn 多 iteration + 兄弟分支的树，断言 `rows` 中 `showIdentity` 为 true 的行数 = turn 数 |
| G2 | `turnIndex === null` 的存量行逐行显示身份行，不推断轮次 | 构造全 `null` 坐标的树，断言所有 assistant 行 `showIdentity === true` |
| G3 | 主会话渲染零回归 | `src/chat/message/__tests__/nodeLayoutGates.test.ts` 与 `groupNodeRows.test.ts` 不改断言即通过；`MessageBubble` 在 `showTimestamp` 缺省（`true`）路径下 DOM 与改动前一致，历史行才传 `false` |
| G4 | 元数据行只在历史页出现 | 全仓 `MessageMetaLine` 的引用点仅 `src/history/detail/` |
| G5 | 记录序列化惰性 | `historyStore` / `historyTree.ts` / `groupNodeRows.ts` 无 `JSON.stringify` 调用；`recordJson` 只在展开行的 `useMemo` 内生成 |
| G6 | 记录快照键恒在 | 单测断言：缺值字段序列化为 `null` 而非键缺失；`toolCalls` 内无 `argsText` 等本地字段 |
| G7 | 详情页导出可选 md / json | 手测 + 单测：两种 format 都调用 `exportConversationToFile` 且后缀正确 |
| G8 | markdown 导出含 reasoning / 工具调用 / 坐标 / 时间；内联图片被占位 | 后端单测（M2） |
| G9 | 复制语义不混 | 面板复制的是记录 JSON；`buildCopyText` 只输出消息正文的可读文本（多模态按块派生，不混入记录 JSON） |
| G10 | 新增文案双语齐全且无硬编码 | 全仓搜索新增中文字面量；两 locale 键集一致 |
| G11 | M1 无后端命令、无契约变更 | `git diff --stat` 不含 `src-tauri/` 与 `protocol.contract.ts`（M1 提交） |
| G12 | 文案不称「原始模型消息」 | 两 locale 全文检索「原始模型消息 / raw model request」零命中 |
| G13 | 单视图，无模式切换 | 全仓零 `viewMode` / `view_read` / `view_raw` 残留；原文面板与气泡同属一个 `data-row-key`（源文件结构断言，沿用 `readRepoFile` 读源文件的既有做法） |
| G14 | 来源四态可区分 | `loading` 与 `dto` 降级态标签颜色不同；`loading` 期间复制禁用；`error` 态标签带 Tooltip 原因（§4.5 表逐行核对） |
| G15 | 时间格式单一口径 | `src/history/` 内 `toLocaleString` / `toLocaleTimeString` 零命中；历史页时间一律走 `formatStamp` |

### 9.2 单测用例（M1 必须覆盖）

`historyTree.test.ts`：

1. 单 turn 单 assistant → 该行 `showIdentity === true`；
2. 单 turn 三 iteration（assistant 链）→ 仅首个为 true；
3. 两 turn 各一个 assistant → 各 true；
4. 同 turn 兄弟分支展开（`expandedKeys` 含 stub key）→ 被展开的 `dimmed` 兄弟行 false，焦点路径行 true；
5. 全 `turnIndex === null` → 所有 assistant 行 true；
6. `role === "tool"` / `compact_summary" / `"user"` → 恒 false。

`recordSnapshot.test.ts`：

1. 键序与 `MessageDto` 一致；
2. `toolCalls` 投影后不含 `argsText` / `approval` / `startedAt`；
3. 缺值字段为 `null`；
4. 同一输入两次调用结果深度相等（纯函数）。

### 9.3 命令

前端：`npm run typecheck`、`npm test`、`npm run lint`（先 `npm run lint:fix`）。
后端（仅 M2）：在 `src-tauri` 目录下 `cargo test -p assistant-storage`、`cargo test --test protocol_contract`，改 DTO 后 `npm run contract:gen` + `npm run contract:check`。

---

## 10 阶段与依赖

| 阶段 | 内容 | 依赖 | 契约变更 | 可独立合并 |
|---|---|---|---|---|
| M1 | 身份归并、元数据行（含原文开关）、行内记录面板（DTO 快照）、导出菜单接线、命名修正 | 无 | 无 | 是 |
| M2 | `MessageRecordDto`（随 `get_conversation` 返回）、markdown/JSON 导出增强、面板来源切换（三态）、可选 JSONL | M1 的 `recordSnapshot` 抽象已就位（§6.3） | 有（`contract:gen`） | 是（在 M1 之后） |
| M3 | 请求体采集 | 独立规格；需新表 + 开关 + 保留策略 | 有 | 否（需独立评审） |

顺序约束：

- M1 内必须按「§2 身份 → §3 元数据 → §4 行内面板 → §5.1 导出」推进，前三步同属渲染链路，拆开会产生中间态（元数据行没有身份行对齐、面板没有行结构支撑）；
- M2 的 §6.3 来源切换必须在 §6.1/§6.2 落地后才可接线，禁止先接 UI 再补命令；
- M3 与 M1/M2 无代码依赖，但文案禁令（§7.1）在 M1 即生效。

---

## 11 与既有规格的关系

| 文档 | 关系 |
|---|---|
| I43-assistant-identity-spec（已归档） | 本文 §2.1 作废其 N7（历史树逐行显示身份），其余条款继续有效；该文档冻结，不再修改，本表即为替代声明 |
| I46 节点布局规格（已归档） | 坐标语义（`turn_index` / `iteration`）与「不猜测未知坐标」的唯一出处，本文只引用不复述；本文 §2.2 是其在历史树上的等价判据，不改变聊天页的 `groupNodeRows` |
| I46 紧凑消息格式设计（已归档） | `compact` 信封仍是工具结果渲染的唯一事实源；本文 §4.3 面板内不重复渲染工具卡片，`compact` 投影仍由气泡承载 |
| I40-history-message-tree-spec（已归档） | 树构建 / 折叠 / 轨道色的基线；本文不改变其语义，只在 `flattenHistoryRows` 输出上追加派生字段 |
| I50 运行时审计规格（已归档） | 相邻但**不通用**：审计记录安全事件（`security_audit_log` / `permission_audit`），本文记录会话内容；禁止把本文的元数据行当作审计来源，也禁止把审计事件塞进历史页 |
| [I38 Agent 阶段总纲](../../store/I38-agent-roadmap.md) | 本文不进阶段门禁；`turn` 坐标的前置条件（I46 S1/S2 已落地）已满足 |
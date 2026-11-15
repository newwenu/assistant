# I48 — 对话视图区域化与滚动意图 实施规格

| 项 | 值 |
|---|---|
| 状态 | 已实施（S1a / S1b / S2 / S3 全部落地；本文不记录进度，实施偏差见 §13） |
| 日期 | 2026-11-13 |
| 关联 | I38d-agent-frontend-modes-spec（已归档）§4.4 / §4.7 / §4.10；I46-agent-node-layout-spec（活跃）§1.3；docs/principle/CODE-SPLIT-PRINCIPLE.md |
| 改动范围 | 前端 `src/chat` 域：布局骨架、工具区域、消息区滚动；无后端改动、无 DTO 变更、无 i18n 新增 |
| 阶段 | S1a 单骨架 · S1b 工具区域 · S2 滚动意图 · S3 位置记忆 |

定位：把「双布局组件 + 视图构造插槽」改为「单骨架 + 条件工具区域」，把 `MessageList` 的隐式滚动改为**显式意图 + 可单测的纯决策函数**。这是规格级变更：取代 I38d §4.10 中「两个布局组件」的形态条款，取代范围见 §2；滚动部分无既有归属，由本文首次建立。

设计的三条主线：

1. **容器身份稳定**——消息区在任何 `link` 下都是同一组件、同一位置，差异只表达为「工具区域挂不挂」。
2. **状态有唯一归属**——工具区域状态由一份 hook 持有；布局骨架无状态；滚动决策由纯函数给出，effect 只负责喂数据和写 DOM。
3. **位置由意图决定**——进入会话、跟随新增、前插补偿三种位移各有唯一触发点，互不叠加；`isStreaming`、`link`、高度变化都不再是位移的判据。

---

## 1 阶段总览

| 阶段 | 目标 | 前置 | 判定 |
|---|---|---|---|
| **S1a 单骨架** | 删除两个转发型布局，`ConversationFrame` 由 `link` 驱动；槽位形态暂不变 | 无 | §9 结构断言 |
| **S1b 工具区域** | 工具区域状态归 `useToolRegionState`，骨架只收数据；视图不再 import 工具组件 | S1a | §9 结构断言 |
| **S2 滚动意图** | 切会话 / 切布局 / 流式跟随收敛为显式意图；阈值分档；前插补偿 | S1a | §9 决策断言 + 运行时核对 |
| **S3 位置记忆** | 每个会话记住 `scrollTop` 与贴底状态，切回时恢复 | S2 | §9 决策断言 + 运行时核对 |

S1a 单独可交付、可回滚：它只是一次渲染结构替换，收益（消息区实例稳定）当场生效，且不依赖任何滚动改动。
S1b 的收益是状态收敛与职责归位，与滚动正确性无关，因此不与 S2 绑定。
S2 的 `entry` 需要 `conversationId` 贯穿到 `MessageList`，这是 S1a 之后就要落位的契约（见 §4）。

S3 依赖 S2：`restore` 是 `planEntry` 的一个分支。

---

## 2 变更性质与取代声明

### 2.1 现行形态的两个后果

I38d §4.10 定义「一个视图 + 两个布局」，意图是**视图不重建**，但实现把差异表达成了**两个组件类型**（`ChatLayout` / `AgentLayout`），于是：

1. **消息区被子树重建波及**。`route.link` 跨 `chat` / `agent` 时，`ConversationView` 的分支换成另一种组件类型，React 卸载旧子树、挂载新子树：滚动容器 `scrollTop` 归零、`lastScrollHeightRef` 重置。消息内容没变，位置却变了。
2. **布局位置知识外泄到视图**。四个区域的位置由视图决定谁塞进哪个插槽，布局组件只做单行转发，不承担任何编排决策；工具区域的开合、目录、提示、默认值进行态也全部由视图持有。

### 2.2 本文取代与保留的条款

| I38d §4.10 条款 | 处置 |
|---|---|
| 「形态：一个视图 + 两个布局」 | **取代**：一个视图 + 一个骨架 + 条件工具区域（§3） |
| 目录树中 `ChatLayout.tsx` / `AgentLayout.tsx` | **取代**：两文件删除，新增 `ToolRegion.tsx`（§10） |
| 「`ChatLayout` / `AgentLayout` 是纯展示布局」 | **保留并加强**：骨架仍无状态，差异由 `link` 枚举驱动，不由组件类型驱动 |
| 「布局选择：只由 `resolveConversationRoute` 决定」 | **保留**：判据不变，消费方式从「选组件」改为「读枚举」 |
| 「模式切换后布局整体替换，`ConversationView` 不重建」 | **改写**：视图不重建（保留）；「布局整体替换」改为「工具区域挂载/卸载」，消息区不参与替换 |
| §4.10 职责边界表中 `ConversationView` 的 `toolConfig` 读写与落库 | **保留**：唯一写入函数与落库时机不变（§5） |
| §4.10 职责边界表中 `ChatLayout` / `AgentLayout` 两行 | **取代**：由 §5 的状态归属表替代 |
| §4.4 / §4.7 工具装配、模式默认值、「单一写入函数」 | **保留**：语义与落库链路不变 |

### 2.3 非目标

- 不改 `resolveConversationRoute` 判据、`route.link` 取值域、模式切换数据流。
- 不改工具装配的业务语义（tier 分组、勾选增删、模式默认值、`patch_conversation`）。
- 不改 `ToolConfigPanel` / `ToolAssemblySection` 两份装配 UI 不复用的既有结论。
- 不改消息树渲染与投影（属 I46 范围）；本文只在 `MessageList` 的滚动治理内改动。
- 不做滚动位置持久化（进程重启不保留），不做程序性平滑跳转动画。
- 不改未读语义、分页接口、`hasMore` 判定来源、`role !== "tool"` 过滤。

---

## 3 目标形态

```
src/chat/
  ConversationView.tsx        会话数据与命令；不 import 任何工具 / 工作区组件；不构造布局 JSX
  layouts/
    ConversationFrame.tsx     无状态骨架：区域位置固定，按 link 条件挂载工具区域
    layoutProps.ts            数据契约（不含 ReactNode 插槽）
    ToolRegion.tsx            工具区域三个挂点的展示适配（无状态）
  tool/
    useToolRegionState.ts     工具区域状态唯一持有者
  message/
    scrollIntent.ts           滚动决策纯函数与常量
    scrollPositionMemory.ts   位置记忆（模块级 Map）
    MessageList.tsx           消费决策结果，只负责测量与写 DOM
```

骨架内的区域位置固定，不随 `link` 改变：

| 区域 | 位置 | `link` 差异 |
|---|---|---|
| 顶部 chrome | 第一行 | `agent` 追加工作区选择器 |
| 工具配置面板 | 输入区工具栏上方 | `agent` 且面板打开时挂载 |
| 消息区 | 中部，flex 撑满 | 无差异（**容器身份必须稳定**） |
| 工具变更提示 | 消息区下方 | `agent` 且有文案时挂载 |
| 输入区工具栏 | 输入区第一行 | `agent` 追加 `ToolToolbar` |
| 输入区 | 下部 | 无差异 |
| 上下文栏 | 底部 | 无差异 |

关键约束：`MessageList` 在骨架中的**父节点与兄弟顺序恒定**。条件挂载只发生在它之外的三个区域，因此 `link` 切换不会改变 `MessageList` 的挂载位置。

---

## 4 props 契约

`layoutProps.ts` 删除 `AgentLayoutSlots`，只保留数据。`ConversationFrame` 改为按 `link` 区分的联合类型，缺失数据在类型层即报错：

```ts
import type { ConversationLink } from "../core/conversationLink";
import type { RefObject } from "react";
import type { ConversationToolConfig } from "../core/chatTypes";
import type { ToolDescriptorDto } from "@/types/protocol.contract";

export interface WorkspaceControlData {
  /** 空串 = 未配置（与 `WorkspaceSelector.workspaceRoot` 语义一致）。 */
  root: string;
  /** turn 进行中禁用。 */
  disabled: boolean;
  /** 菜单展开态回传，供面板统一关闭 / Escape 拦截。 */
  openRef?: RefObject<boolean>;
  /** 返回 `false` 表示失败，保留原值。 */
  onSwitch: (path: string) => Promise<boolean>;
  /** 清除绑定（写回空串）。 */
  onClear: () => Promise<boolean>;
}

/** 视图交给 `useToolRegionState` 的输入（命令侧）。 */
export interface ToolRegionInput {
  conversationId: string | null;
  config: ConversationToolConfig;
  onChange: (next: ConversationToolConfig) => void;
  /** 写回模式默认值；由视图负责落库与提示，hook 只管理进行态。 */
  onSetDefault: () => Promise<void>;
}

/** hook 交给骨架的视图数据（展示侧，无状态）。 */
export interface ToolRegionView {
  open: boolean;
  toolbar: {
    config: ConversationToolConfig;
    total: number;
    onToggle: () => void;
  };
  panel: {
    config: ConversationToolConfig;
    tools: ToolDescriptorDto[];
    loading: boolean;
    canSetDefault: boolean;
    settingDefault: boolean;
    onChange: (next: ConversationToolConfig) => void;
    onSetDefault: () => void;
  };
  notice: string | null;
}

export type ConversationFrameProps = ConversationLayoutProps &
  (
    | { link: "chat" }
    | { link: "agent"; workspace: WorkspaceControlData; toolRegion: ToolRegionView }
  );
```

配套改动：

- `ConversationLayoutProps` 增加 `conversationId: string | null`，由骨架原样传给 `MessageList`（S2 的 `entry` 与 S3 的位置记忆都只认它）。
- `ConversationLink` 直接从 `core/conversationLink.ts` 复用，不重复声明。
- `canSetDefault` 不作为外部输入：`link === "agent"` 时 `mode.kind` 必为 `agent`（`resolveConversationRoute` 的定义即如此），因此该值恒为 `true`，由 hook 直接给出，避免一个永远为真的字段在契约里漂移。
- `settingDefault` 属于 hook 内部进行态，不出现在 `ToolRegionInput`。
- `modeControls` 维持 `ReactNode`：它在两个 `link` 下位置相同，不构成 §2.1 的「位置知识外泄」，本次不动。

`ConversationFrame` 以普通函数组件声明（`function ConversationFrame(props: ConversationFrameProps)`），便于联合类型收窄，不用 `React.FC`。

---

## 5 状态归属

| 状态 / 副作用 | 现状持有者 | 目标持有者 | 理由 |
|---|---|---|---|
| `toolPanelOpen` | `ConversationView` | `useToolRegionState` | 面板开合是工具区域的 UI 状态 |
| 工具目录（`useToolCatalog`） | `ConversationView` | `useToolRegionState` | 目录只被工具区域消费；模块级缓存保证跨布局不重复请求 |
| 外部点击收起（`pointerdown` 捕获） | `ConversationView` | `useToolRegionState` | 面板存在期才有意义 |
| 切会话收起面板 | `ConversationView`（effect） | `useToolRegionState`（`conversationId` 变化重置） | 与面板语义绑定 |
| orb 收起关闭面板 | `ConversationView` | `useToolRegionState` | 同上 |
| 变更提示 `notice` + 6s 定时器 | `ConversationView` | `useToolRegionState` | 文案由 `diffToolConfig` + `tools` + `t` 构造，三者均在区域内可得 |
| `settingDefault` | `ConversationView` | `useToolRegionState` | 面板按钮的进行态 |
| `effectiveToolConfig` | `ConversationView` | **保留** | 来源是 `route.toolConfig`，属路由数据 |
| 配置写入与落库 | `ConversationView` | **保留** | 单一写入函数不变 |
| 模式默认值写回 + 成功/失败提示 | `ConversationView` | **保留** | 跨域写入（settings），不属工具区域 |
| 工作区切换 / 清除 | `ConversationView` | **保留** | 会话级数据操作 |
| 滚动位置、贴底状态、未读 | `MessageList` | **保留**（`MessageList`） | 位移与视口是 DOM 事实，只有它持有测量能力 |
| 滚动决策规则 | `MessageList` 内联 | `scrollIntent.ts`（纯函数） | 决策可单测，effect 变薄 |

`useToolRegionState` 签名：

```ts
export function useToolRegionState(input: ToolRegionInput & { enabled: boolean }): ToolRegionView;
```

行为约定：

- `enabled === false`（`link === "chat"`）：不注册任何副作用、不发起目录请求、`open` 恒为 `false`，`panel` / `toolbar` 为空实现。hook 始终被调用（遵守 hooks 规则），但对外只返回空视图。
- `enabled` 由 `true` 变 `false` 时收起面板；由 `false` 变 `true` 时不自动展开。理由：面板属于 agent 链路，离开即关是它的语义；这一点相对现状是**有意收紧**（现状视图的 `toolPanelOpen` 跨 `link` 保留），在 §12 风险表登记。
- `conversationId` 变化：收起面板、清空提示、清空定时器。
- `applyChange(next)`：`diffToolConfig(config, next)` → 生成文案 → 显示并计时 6s → 调用 `onChange(next)`。空 diff 不提示但仍提交（与现状一致）。
- `applySetDefault()`：置 `settingDefault=true` → `await onSetDefault()` → 置回 `false`（`finally`）。

允许的依赖：`useToolCatalog`、`core/toolConfig`、`orb/orbOverlays`、`react-i18next`。**禁止** import `chatService`、任何 store 的写操作、`settings` 域模块。

`ToolRegion.tsx` 只做三件事：把 `ToolRegionView` 摊平成 `ToolToolbar` / `ToolConfigPanel` / `ToolChangeNotice` 的 props，并在不满足挂载条件时返回 `null`。它无状态、无副作用、不 import 服务。

---

## 6 滚动意图规格

### 6.1 意图定义

滚动位置不是派生值，而是**显式意图**。位移只有三种来源，互斥：

| 意图 | 触发条件 | 行为 | 时机 |
|---|---|---|---|
| `entry` | `conversationId` 变化（含首次挂载） | 写 `scrollTop = plan.top`，`behavior: "auto"` | paint 前 |
| `follow` | 内容高度增加或视口变小，且当前贴底 | `scrollTop = scrollHeight`，`behavior: "auto"` | ResizeObserver 回调内 |
| `compensate` | 向上分页前插导致内容高度增加 | `scrollTop = pending.top + (contentHeight - pending.height)`，`behavior: "auto"` | ResizeObserver 回调内 |
| `none` | 其余一切 | 不写 `scrollTop` | — |

「进入某会话」的唯一判据是 `conversationId` 变化。**不得**以「`scrollHeight` 变了」「`isStreaming` 变了」「`link` 变了」作为进入判据。

`smooth` 只用于用户主动点击「回到最新」。程序性位移一律 `auto`：流式单次增量大于一屏时，`smooth` 会变成"追着跑"的动画，与 §9.3 的「流式期间无追赶动画」直接冲突。

### 6.2 纯函数契约

`src/chat/message/scrollIntent.ts`，不含 React / store / i18n 依赖：

```ts
export const FOLLOW_EPSILON = 2;      // 贴底判定
export const AWAY_THRESHOLD = 100;    // 判定为用户上翻
export const LOAD_MORE_TRIGGER = 100; // 触顶加载更多

export interface ScrollMetrics {
  scrollTop: number;
  contentHeight: number;
  viewportHeight: number;
}

export interface SavedPosition {
  top: number;
  /** 保存时是否贴底；贴底优先于像素值。 */
  pinned: boolean;
}

export function maxTop(m: ScrollMetrics): number;
export function distanceFromBottom(m: ScrollMetrics): number;

/** 迟滞：贴底 → 距底超过 AWAY_THRESHOLD 才脱离；上翻 → 距底回到 FOLLOW_EPSILON 才重新贴底。 */
export function nextPinned(prev: boolean, distance: number): boolean;

export interface EntryPlan {
  top: number;
  pinned: boolean;
}

export function planEntry(m: ScrollMetrics, saved: SavedPosition | null): EntryPlan;

export type ResizePlan =
  | { action: "none" }
  | { action: "follow" }
  | { action: "compensate"; top: number };

export function planResize(input: {
  metrics: ScrollMetrics;
  delta: number;          // 内容高度增量（本次回调）
  viewportDelta: number;  // 视口高度增量（本次回调）
  pinned: boolean;
  pendingPrepend: { height: number; top: number } | null;
}): ResizePlan;
```

`planEntry` 规则：

- `contentHeight <= viewportHeight` → `{ top: 0, pinned: true }`（内容不足一屏，不产生滚动）。
- `saved?.pinned` → `{ top: maxTop, pinned: true }`（内容长度变了也仍然在最新处）。
- `saved` 存在 → `top = clamp(saved.top, 0, maxTop)`，`pinned = top >= maxTop - FOLLOW_EPSILON`。
- `saved` 为空 → `{ top: maxTop, pinned: true }`。

`planResize` 规则（按优先级）：

1. `pendingPrepend` 非空 → `compensate`，`top = pendingPrepend.top + (contentHeight - pendingPrepend.height)`。
2. `!pinned` → `none`。
3. `delta > 0` → `follow`。
4. `viewportDelta < 0`（工具栏挂上、面板打开等压缩了消息区）→ `follow`。

第 4 条是"贴底用户凭空失去底部"的修复：视口变小使 `maxTop` 变大，若不跟随，用户会看到上方内容且按钮不出现。

`pendingPrepend` 必须由调用侧在**发起加载前**记录，不能读 store 的 `isLoadingMoreMessages`：该标志在插入消息的同一次 `set` 中被置回 `false`，渲染时已是 `false`，无法作为"前插未补偿"的判据。

### 6.3 阈值分档

| 常量 | 用途 |
|---|---|
| `FOLLOW_EPSILON` | 贴底判定（重新贴底的门槛） |
| `AWAY_THRESHOLD` | 判定为用户上翻、未读累计门槛、悬浮按钮显示 |
| `LOAD_MORE_TRIGGER` | 触顶加载更多 |

`FOLLOW_EPSILON` 与 `AWAY_THRESHOLD` 之间是**有意设计的迟滞带**：2~100px 之间保持既有状态，不来回抖动。流式期间跟随判定必须用 `FOLLOW_EPSILON`，否则"已上翻 30px"会被误判为贴底并打断阅读。

### 6.4 DOM 结构

`MessageList` 需要一个**内容容器**作为观察对象，滚动容器不能兼任：

```
.wrapper（position: relative, flex: 1, min-height: 0）
  .container（height: 100%, overflow-y: auto, overflow-anchor: none）  ← 视口
    .content（消息与加载更多区的父节点）                              ← ResizeObserver 观察
  .scrollToBottom（悬浮按钮）
```

- `overflow-anchor: none` 加在 `MessageList.module.css` 的 `.container` 上。理由：位置由意图决定，浏览器 scroll anchoring 会与手动写入竞争，产生不可预测漂移。
- 空态分支同样保留 `.content` 包装，避免观察对象在有无消息之间切换。

### 6.5 时序

| 环节 | 手段 |
|---|---|
| `entry` | `useLayoutEffect`，依赖 `conversationId`，paint 前同步写入 |
| `follow` / `compensate` | `ResizeObserver` 回调，回调在 layout 之后、paint 之前，回调内直接写入，不再叠 `requestAnimationFrame` |
| 用户滚动判定、未读、加载更多 | `scroll` 事件 + `rafIdRef` 节流（沿用现状） |
| 位置保存 | 滚动节流回调内（覆盖同一会话内的移动）+ 组件卸载时 |
| 观察对象 | 一个 `ResizeObserver` 同时观察 `.content` 与 `.container`（前者给 `delta`，后者给 `viewportDelta`） |

`entry` 步骤（顺序固定）：

1. 读该会话的 `SavedPosition`。
2. 测量 `contentHeight` / `viewportHeight`（`useLayoutEffect` 保证 DOM 已提交）。
3. 置 `programScrollRef = true`，写 `scrollTop = plan.top`。
4. 重置基线：`baselineRef = contentHeight`、`viewportBaselineRef = viewportHeight`。
5. 按 `plan.pinned` 同步派生状态：`pinnedRef` 写入；`!pinned` 时 `userScrolledUp = true`、`unreadCount = 0`、`seenMessageCountRef = visibleMessages.length`；`pinned` 时 `userScrolledUp = false`、`unreadCount = 0`。

第 4 步不可省略：切会话后内容高度整体变化，首个 RO 回调的 `delta` 会是整页高度，会被判成 `follow`，`entry` 立刻被覆盖。

第 5 步不可省略：`restore` 到非底部时若不置 `userScrolledUp`，`unreadCount` 会按"两个会话消息数差"虚增。

`ResizeObserver` 回调步骤：

1. 取 `contentHeight` / `viewportHeight`，算 `delta`、`viewportDelta`，随即更新基线（**先算后更新**，避免累积误差）。
2. `pendingPrepend` 非空 → `compensate`，写 `scrollTop`（`programScrollRef` 保护），清空 `pendingPrepend`，返回。
3. `planResize` 返回 `follow` → 写 `scrollTop = scrollHeight`（`programScrollRef` 保护）。

降级：`ResizeObserver` 不可用或初始化失败时，退回"渲染后比较 `scrollHeight`"的分支，同样只发一次 `follow`，其余规则不变。

### 6.6 不变量

- **I1** 任何程序性 `scrollTop` 写入都在 `programScrollRef` 保护下进行；滚动处理器看到该标志时跳过**本轮全部判定**（含加载更多触发），不只是贴底判定。
- **I2** `entry` 之后必须重设 RO 基线（§6.5 第 4 步）。
- **I3** `entry` 只在 `conversationId` 变化时发生。
- **I4** `pinned` 是"是否跟随"的唯一判据；`userScrolledUp` 由 `!pinned` 派生，不独立维护第二份真相。
- **I5** `compensate` 与 `follow` 互斥，同一次回调只可能命中一个。

### 6.7 与分页、未读的交互

- **发起加载更多**：条件为 `hasMore && scrollTop < LOAD_MORE_TRIGGER && !pendingPrepend && !programScroll`。发起前记录 `pendingPrepend = { height: contentHeight, top: scrollTop }`，然后调 `onLoadMore()`。沿用现有的 1s 去抖。
- **补偿完成**：由 RO 回调按 §6.5 执行；`pendingPrepend` 在补偿后清空。若 3s 内未观察到高度变化（请求失败或返回空页），清空标志并允许重新触发。
- **加载期间不跟随**：`pendingPrepend` 非空即视为"前插进行中"，`planResize` 优先走补偿分支，不进入 `follow`。
- **未读计数**：仅在 `!pinned` 时累计新增可见消息数，判定口径不变。
- **流式结束归位**：现状「流式结束后若距底 ≤ 阈值则清零上翻标记」改用 `AWAY_THRESHOLD`，语义不变。
- **悬浮按钮**：点击后 `smooth` 到底，并置 `pinned = true`、清零未读。

---

## 7 位置记忆规格

- `src/chat/message/scrollPositionMemory.ts` 持有模块级 `Map<conversationId, SavedPosition>`；不写 store、不持久化。
- **保存时机**：滚动节流回调内（每次 rAF 一次 Map 写入，成本可忽略）；组件卸载时（元素仍在，内容未换）。
- **禁止**在 `conversationId` 变化的 cleanup 中保存：此时新内容已渲染、旧内容已卸载，浏览器可能已按新内容 clamp 过 `scrollTop`，会把旧会话的位置写成 0；且开发环境 `StrictMode` 会让 cleanup 双跑，错误被放大。
- **恢复时机**：`planEntry` 的 `saved` 输入。
- **`pinned` 优先**：保存时贴底则以 `pinned: true` 记录，切回时直接落到底部，不受内容长度变化影响；非贴底才用像素值，并按 `maxTop` 截断。
- **已知取舍**：切回只加载最新一页，若原位置靠上，`saved.top` 往往超出 `maxTop`，会截断到底部——这是"未加载的历史无法定位"的必然结果，不视为异常（锚点恢复已被 §2.3 排除）。
- **清理**：不清理。键与打开过的会话数同级，值是 `{ top, pinned }`，不构成实际泄漏；不做跨域删除联动，避免 store 反向依赖 UI 模块。
- **非目标**：进程重启、窗口关闭不保留；不做消息 id 锚点恢复。

---

## 8 实施步骤

| 步 | 内容 | 可独立验收 |
|---|---|---|
| 1 | 新建 `ToolRegion.tsx` 骨架（先按现有四插槽渲染，不改行为） | 否 |
| 2 | `layoutProps.ts` 增 `conversationId`；`ConversationFrame` 增 `link`，槽位形态暂不变 | 是 |
| 3 | `ConversationView` 删除 `link === "agent"` 的组件分支，改传 `link` + 四个插槽给单一 `ConversationFrame`；删除 `ChatLayout` / `AgentLayout` | 是（S1a 完成，消息区实例稳定） |
| 4 | 新建 `useToolRegionState`，迁移工具区域的状态与副作用；`ToolRegion` 改为消费 `ToolRegionView`；删除四插槽类型 | 是（S1b 完成） |
| 5 | 新建 `scrollIntent.ts` + 用例；`MessageList` 接入 `planEntry` / `planResize` / `nextPinned` / `pendingPrepend`；新增 `.content` 包装与 `overflow-anchor` | 是（S2 完成） |
| 6 | 新建 `scrollPositionMemory.ts`，接入保存与恢复 | 是（S3 完成） |

步骤 3 之后可以停在 S1a 观察一段时间，滚动改动与它无耦合。

---

## 9 验收

### 9.1 结构断言

- `src/` 内不存在 `ChatLayout` / `AgentLayout` 的导出与引用。
- `ConversationFrameProps` 不含 `workspaceControl` / `toolToolbar` / `toolPanel` / `belowMessages` 四个 `ReactNode` 槽位。
- `ConversationView.tsx` 不 import `ToolToolbar` / `ToolConfigPanel` / `ToolChangeNotice` / `WorkspaceSelector`，也不含按 `link` 选组件的分支。
- `ConversationFrame.tsx` 无 `useState` / `useEffect` / 服务调用。
- `scrollIntent.ts`、`scrollPositionMemory.ts` 无 React / store / i18n / 服务依赖。
- `useToolRegionState.ts` 不 import `chatService`、不写 store。
- `npm run typecheck`、`npm run lint`、`npm run test` 通过。

### 9.2 决策断言（`scrollIntent.test.ts`，可自动化）

- 内容超过一屏、无保存位置 → `planEntry` 返回 `top = maxTop`、`pinned = true`。
- 内容不足一屏 → `planEntry` 返回 `top = 0`、`pinned = true`。
- 保存值超过 `maxTop` → 截断为 `maxTop`；截断后 `pinned = true`。
- 保存值为 `pinned: true` → 无论内容多长都返回 `maxTop`。
- `nextPinned`：贴底后距底 50px 仍为 `true`；距底 101px 变 `false`；上翻后距底 1px 变 `true`，距底 50px 仍为 `false`（迟滞带）。
- `planResize`：`pendingPrepend` 非空时优先返回 `compensate`，且 `top = prevTop + (contentHeight - prevHeight)`。
- `planResize`：`!pinned` 且内容增高 → `none`。
- `planResize`：`pinned` 且视口变小 → `follow`。
- `planResize`：`delta <= 0` 且 `viewportDelta >= 0` → `none`。
- 函数签名不含 `isStreaming` / `link`（编译期即可确认）。

### 9.3 运行时核对（手动，开发环境）

仓库测试环境为 node + 无 DOM 仿真（无 jsdom / testing-library），组件级时序无法自动化，以下条目按清单人工核对：

| 条目 | 核对方法 |
|---|---|
| 切模式跨 `chat` / `agent`，消息区实例不重建 | React DevTools 观察组件树不重建；`scrollTop` 前后一致 |
| 布局变化无可观察的滚动动画 | 贴底用户切模式后仍在底部，无"顶部 → 底部"位移 |
| 上翻用户切模式后位置不被拉到顶部，也不跳到底 | 停在原消息附近 |
| 切会话首帧即终态位置 | 打开会话瞬间即到底部（或恢复位置），无补滚动画 |
| 进入会话不误触加载更多 | 恢复位置靠近顶部时不额外拉一页 |
| 流式期间视口无抖动、无追赶动画 | 连续输出时底部稳定 |
| 上翻后流式继续不被拉回，未读正常累计 | 悬浮按钮数字递增 |
| 「回到最新」单次平滑并清零未读 | 点击后一次到位 |
| 切走再切回同一会话恢复原位置 | 贴底会话回到底部；上翻会话回到近似位置 |
| 加载更多后视口内容不位移 | 上滑触发分页，当前阅读行不动 |

---

## 10 文件变更

| 动作 | 文件 | 要点 |
|---|---|---|
| 新建 | `src/chat/tool/useToolRegionState.ts` | 工具区域状态唯一持有者（§5） |
| 新建 | `src/chat/layouts/ToolRegion.tsx` | 三个挂点的展示适配，无状态 |
| 新建 | `src/chat/message/scrollIntent.ts` | 常量、`planEntry`、`planResize`、`nextPinned` |
| 新建 | `src/chat/message/scrollPositionMemory.ts` | `SavedPosition` 的读写 |
| 新建 | `src/chat/message/__tests__/scrollIntent.test.ts` | §9.2 用例 |
| 修改 | `src/chat/layouts/layoutProps.ts` | 删 `AgentLayoutSlots`；增 `conversationId`、`WorkspaceControlData`、`ToolRegionInput`、`ToolRegionView` |
| 修改 | `src/chat/layouts/ConversationFrame.tsx` | 联合类型 props；按 `link` 挂载三处工具区域；区域位置固定 |
| 修改 | `src/chat/message/MessageList.tsx` | 删高度差 effect 与 `lastScrollHeightRef`；接入意图决策、`ResizeObserver`、前插补偿、位置记忆 |
| 修改 | `src/chat/message/MessageList.module.css` | `.container` 加 `overflow-anchor: none` |
| 修改 | `src/chat/ConversationView.tsx` | 删除四插槽构造与工具组件 import；删除工具区域的状态与副作用；改为传 `link` + 数据 |
| 删除 | `src/chat/layouts/ChatLayout.tsx`、`src/chat/layouts/AgentLayout.tsx` | 由 `link` 条件挂载替代 |

无 i18n 新增：变更提示沿用 `chat.tool.notice.*` 既有键。

---

## 11 与既有规格的关系

| 文档 | 关系 |
|---|---|
| I38d-agent-frontend-modes-spec（已归档） | 取代 §4.10 的形态与目录树条款（§2.2 逐条列出）；§4.4 / §4.7 的工具装配语义与单一写入函数不变 |
| I46-agent-node-layout-spec（活跃） | 其 §1.3 明确「不改 `MessageList` 的滚动与未读逻辑」，本文接管滚动治理；本文不改消息树投影、分组、turn 尾，`MessageList` 的数据入口与投影链保持不变 |
| docs/principle/CODE-SPLIT-PRINCIPLE.md | 按「一个文件只对应一个变化原因」：布局位置知识归骨架，工具区域状态归 hook，滚动决策归纯函数，位置记忆独立成模块（生命周期与决策不同） |
| docs/CODE-STRUCTURE.md、docs/ARCHITECTURE.md | 两处 `chat/layouts` 描述在**实施后**同步为「骨架 + 工具区域」，本文不预改活跃结构文档 |

---

## 12 风险与取舍

| # | 风险 / 取舍 | 处置 |
|---|---|---|
| 1 | 规格级变更触及已归档 I38d §4.10 | §2.2 逐条声明取代范围；「一个视图」与工具装配语义保留，变更面收敛到「差异如何表达」 |
| 2 | `ResizeObserver` 在不同平台对内容变化的触发时机差异 | 保留高度差降级分支（§6.5），两条路径都只发一次 `follow` |
| 3 | `overflow-anchor: none` 使上滑加载历史必须手动补偿 | §6.2 给出补偿公式，`pendingPrepend` 作为唯一判据（不依赖瞬时 store 标志） |
| 4 | 位置记忆 Map 长期驻留 | 键与打开过的会话数同级，值是纯数据；不做跨域删除联动 |
| 5 | 行为变化：离开 agent 链路时收起工具面板（现状是保留） | 有意收紧，语义为"面板属于 agent 链路"；在 §5 与本文档一并记录 |
| 6 | `ConversationFrame` 会 import `ToolRegion` 与 `WorkspaceSelector` | 约束是"骨架不持有状态、不发请求"，而非"子树无副作用"：`WorkspaceSelector` 自身仍访问 `workspaceService`，这一点在文档中如实说明，不宣称"布局层零服务调用" |
| 7 | 组件级时序缺少自动化测试 | 决策全部下沉到纯函数（§6.2），时序核对降级为 §9.3 人工清单；不为此引入 jsdom / testing-library 依赖 |
| 8 | 取舍：不做持久化位置记忆、不做程序性平滑跳转 | 前者收益低于复杂度；后者与"纠正应瞬时"冲突，`smooth` 只留给用户主动操作 |
| 9 | 取舍：不做消息 id 锚点恢复 | 属 I46 投影范围，本文不越界 |
| 10 | 消息异步到达时 `entry` 可能先以「内容不足一屏」判定为贴底 | 若切会话首帧 `messages` 仍为空，`entry` 落 `top = 0, pinned = true`，随后消息到达走 `follow` 到底，保存的靠上位置失效。现状依赖「`entry` 时内容已就绪」（`switchConversation` 先 `await` 详情再 `set`），尚未覆盖流式期间切回或分页首帧为空的场景；出现时需在 `entry` 加「内容就绪」判据，不在本次实施范围 |

---

## 13 实施记录（as-built 偏差）

实施结果与 §4 ~ §7 的契约一致，下列是规格文本未写、实施时补上的必要细节：

| # | 偏差 | 原因 |
|---|---|---|
| 1 | 新增 `writeScrollTop`：程序性位移的唯一写入点，先按 `maxTop` 钳制、与当前值比较，**只有真正产生位移才置 `programScrollRef`** | 规格只说「在 `programScrollRef` 保护下写入」。若写入值与现值相同，浏览器不发 `scroll` 事件，标志不会被消费而滞留，会把用户下一次真实滚动的判定吞掉（含加载更多） |
| 2 | 新增 `requestLoadMore`：滚动触顶与「加载更早」按钮**共用**同一入口 | §6.7 只写了滚动触发路径。按钮原本直连 `onLoadMore`，不记录 `pendingPrepend` 基线，前插后无法补偿，视口会跳 |
| 3 | `MessageList` 新增必填 `conversationId`，由 `ConversationFrame` 透传 | §4 已约定 `ConversationLayoutProps.conversationId`，实施时补上消费端 prop 与骨架传递 |
| 4 | `entry` 的 `saved` 来自 `readPosition(conversationId)`（S2 落地时先传 `null`，S3 接上） | §6.2 契约本就包含 `saved`，分阶段实施时先留空再接入，避免 S2 依赖 S3 |
| 5 | 卸载保存位置用 `conversationIdRef` 取最新 id，不依赖闭包快照 | 卸载 effect 的依赖必须为空（否则每次会话变化都重挂），闭包会捕获旧 id |
| 6 | `ToolRegionView.panel.canSetDefault` 由 hook 恒给 `true` | 与 §4「`link === "agent"` 时 `mode.kind` 必为 `agent`」一致，避免一个永远为真的字段进入外部契约 |
| 7 | `enabled === false` 时 hook 直接返回空视图（`open: false`、`toolbar` / `panel` 为空实现、`notice: null`） | §5 已约定；实施时以早返回实现，保证 `chat` 链路不注册任何副作用 |
| 8 | 离开 `agent` 链路与切会话合并为同一个重置 effect（`[enabled, conversationId]`） | 两者都是「收起面板」，分开写会在 `link` 切换时重复触发一次 |
| 9 | 降级分支（无 `ResizeObserver`）只在 `!pinned` 时跳过，且用基线比较 | 与 §6.5「同样只发一次 `follow`」对齐；`entry` 仍走 `useLayoutEffect`，不依赖 RO |
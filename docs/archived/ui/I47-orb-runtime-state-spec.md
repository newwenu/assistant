# I47 — Orb 运行态与可见性 实施规格

| 项 | 值 |
|---|---|
| 状态 | 实施规格（分阶段，未实施；本文不记录进度） |
| 日期 | 2026-11-13 |
| 关联 | I38d §4.7（已归档，会话工具装配）；I38g（审批策略）；[I35-screenshot-qa-spec.md](../../store/I35-screenshot-qa-spec.md)（条件挂起项） |
| 影响面 | 前端 4 文件（含 1 新建）+ 2 i18n；后端 3 文件；测试 2 文件 |
| 阶段 | C1 运行态门控 · C2 档位常驻可见 · C3 窗口属性 · C4 冷启动锚点 |

定位：本规格把 Orb 的运行态门控、档位常驻可见、常驻层级三项增量，与一项冷启动锚点缺陷，建成四个可独立合入的阶段。范围与排除项均以本文 §2 为准，不再依赖任何前置分析文档。

---

## 1 阶段总览

| 阶段 | 目标 | 改动层 | 前置 | 判定条目 |
|---|---|---|---|---|
| **C1 运行态门控** | 流式进行中或存在待处理工具调用时，拦截失焦自动收起 | 前端 | 无 | F1–F5、AC1–AC2 |
| **C2 档位常驻可见** | 会话审批档位在输入区概览条上始终可见，且与发送时生效配置同源 | 前端 + i18n | 无 | AC3–AC4 |
| **C3 窗口属性** | 球跨虚拟桌面/空间可见；启动时窗口属性与配置一致 | 后端 | 无 | AC5–AC6 |
| **C4 冷启动锚点** | 无保存位置时球落在可用区右下角，不覆盖任务栏 | 后端 crate | 无 | B1–B3、AC7 |

四个阶段互不前置：C1/C2 无后端改动，C3/C4 无前端改动，可任意顺序独立合入。每个阶段的「依赖」小节只写本阶段内部的先后约束。

---

## 2 目标与判定口径

### 2.1 可判定目标

| 编号 | 目标 | 判定方式 |
|---|---|---|
| G1 | 流式进行中或存在等待用户处理的工具调用时，**失焦自动收起被拦截**；显式收起不受影响 | C1 的 F2 + F3 + F4 |
| G2 | 会话审批档位在输入区概览条上**始终可见**，且与发送时实际生效的配置同源 | AC3 + AC4 |
| G3 | 球在切换虚拟桌面/空间后仍可见（macOS、Linux），且启动时窗口属性与配置一致 | AC5 + AC6 |
| G4 | 无保存位置的冷启动不再把球放进任务栏区域 | C4 的 B1 + B2 + AC7 |
| G5 | 不新增持久化字段、不改 DTO、不新增设置项 | §9 文件清单复核 |

### 2.2 非目标

- 不做边缘停靠竖条（放弃理由见 §10.1）。
- 不改 `ORB_SNAP_BORDER_X` 吸附、拖拽 clamp、展开方向滞回、位置持久化。
- 不改 `clickThrough` 语义（C1 §4.4 澄清）。
- 不在球上新增状态点、角标、进度环等视觉元素（§10.3）。
- 不把吸附与纠正的边界从整屏 bounds 改为 work_area（C4 §7.4）。
- 不做截图排除与 HID 穿透 guard（§10.2，条件挂起）。

### 2.3 编号空间

| 前缀 | 含义 |
|---|---|
| C | 阶段（可独立合入的实施单元） |
| B | 后端断言（Rust 测试） |
| F | 前端断言（vitest） |
| AC | 验收条目 |

---

## 3 现状基线

以下事实全部经源码逐点核对，是后续阶段的引用依据；本文不复述实现细节，只标注「现状结论」。

| # | 事实 | 位置 | 现状结论 |
|---|---|---|---|
| 1 | `orbStore` 只有展开、悬停、拖拽、可见、固定、位置、方向七类字段 | `src/stores/orbStore.ts`（`OrbState`） | 无任何运行态或等待态字段；门控信号不能来自 orb 侧 |
| 2 | `isStreaming` 是全局流式信号 | `src/stores/chatStreamStore.ts`（`ChatStreamState`） | 可直接作为「正在生成」的信号源 |
| 3 | `hasPendingToolCalls` 判定 `running` 与 `awaiting_user` | `src/chat/core/toolCallView.ts`（同名单函数） | 一个判定覆盖「工具执行中」 |
| 4 | `awaitingApproval` 与 `awaitingAnswer` 均收敛为 `awaiting_user` | `src/chat/core/toolCallView.ts`（`applyToolCallEvent`） | 同一判定同时覆盖「等待审批」与「等待回答」，不需要第二个信号 |
| 5 | 失焦自动收起是唯一的自动收起路径，已有 `keepExpanded`、`suppress`、`isDragging` 三条豁免 | `src/orb/OrbPanel.tsx`（`listen(UiEvent.OrbBlur)` 回调） | 门控只需插在这一条路径上 |
| 6 | Esc 与顶栏收起按钮直接调用 `collapse()` | `src/orb/OrbPanel.tsx`（`handleKeyDown`、顶栏按钮） | 属于显式路径，不应被门控拦截 |
| 7 | 收起完成后按 `clickThrough` 恢复穿透 | `src/orb/OrbExpand.ts`（`collapseOrbPanel`） | 穿透只在「真的收起」时恢复，门控提前返回不会误开穿透 |
| 8 | 展开时按 `clickThrough` 关闭穿透；`collapse` 是无参数强收起 | `src/orb/OrbExpand.ts`（`useOrbExpand`） | 需要新增一个受门控的入口，而不是改 `collapse` 语义 |
| 9 | `useOrbExpand` 有两个调用点：`OrbPanel` 取 `expand`/`collapse`，`OrbDrag` 只取 `expand` | `src/orb/OrbPanel.tsx`、`src/orb/OrbDrag.tsx` | 返回值新增成员是纯增量，不影响 `OrbDrag` |
| 10 | 概览条只展示「已启用数量 + tier 上限」 | `src/chat/tool/ToolToolbar.tsx`（`summary`） | `approvalLevel` 完全不可见 |
| 11 | 概览条入参 `config` 即 `effectiveToolConfig`（`route.toolConfig ?? DEFAULT_TOOL_CONFIG`） | `src/chat/ConversationView.tsx`（`effectiveToolConfig`、`ToolToolbar` 传参） | 档位可从这里直接读，不需要新数据通路 |
| 12 | 每轮发送携带 `toolConfig` | `src/services/stream/turnOrchestrator.ts`（`SendChatMessageArgs`） | 后端按本次发送的配置构策略，「发送前重新应用」已由传输形态满足 |
| 13 | 冷启动锚点 = 主屏整屏右下 − 20px | `src-tauri/src/setup.rs`（`position_orb_window` 无保存位置分支） | 边距小于任务栏高度，球落入任务栏区域 |
| 14 | 屏幕信息取 monitor 的 `position` / `size` | `src-tauri/crates/assistant-orb/src/placement.rs`（`get_available_screens`） | 是整屏 bounds，不含可用区 |
| 15 | `always_on_top` 只在配置变更时应用 | `src-tauri/src/cmd/config.rs`（`ORB_SIDE_EFFECTS`） | 启动时不应用，与配置文件可能不一致 |
| 16 | orb 窗口 `alwaysOnTop` 为 true、`skipTaskbar` 为 true、`visible` 为 false | `src-tauri/tauri.conf.json`（`orb` 窗口） | 启动态固定为置顶，与配置项默认值一致，但用户改为 false 后启动不生效 |
| 17 | 依赖 tauri 2.12.5、tao 0.37.2；`Monitor::work_area()` 与 `set_visible_on_all_workspaces` 均存在 | `src-tauri/Cargo.lock` | 两个 API 都可用；`set_visible_on_all_workspaces` 在 tao 侧被 `cfg(any(macos, linux))` 包住，Windows 上为空实现且不返回错误 |
| 18 | 启动顺序为 `position_orb_window` → `restore_main_window` → `show_orb_window` | `src-tauri/src/setup.rs`（`setup` 主流程） | 属性必须在 `show_orb_window` 之前应用，才不会出现「先按 conf 默认显示、再跳变」 |

---

## 4 阶段 C1 — 运行态门控

### 4.1 终态结构

新建 `src/orb/orbBusy.ts`，只放两个导出：

| 导出 | 签名 | 职责 |
|---|---|---|
| `orbBusyFrom` | `(isStreaming: boolean, messages: Message[]) => boolean` | 纯函数，唯一判定实现 |
| `isOrbBusy` | `() => boolean` | 从两个 store 取值后转调 `orbBusyFrom` |

`OrbExpand.ts` 新增模块级函数，供组件与测试共用：

| 导出 | 签名 | 职责 |
|---|---|---|
| `autoCollapseOrb` | `() => Promise<void>` | 自动收起入口：busy 时直接返回，否则转调 `collapseOrbPanel` |

`useOrbExpand` 的返回值新增 `autoCollapse`（转调 `autoCollapseOrb`），`collapse` 保持无参数强收起：

| 入口 | 语义 | 是否门控 |
|---|---|---|
| `collapse` | 强收起 | 否 |
| `autoCollapse` | 自动收起（失焦触发） | 是 |

`OrbPanel` 的失焦监听（含 `!isHoverCapable()` 分支与坐标判定后的兜底）改调 `autoCollapse`；Esc、顶栏收起按钮、历史菜单动作、`collapseOrbOnInit` 仍走 `collapse` / `collapseOrbPanel`。

**为什么用模块级函数而不是只放在 hook 里**：仓库内无组件测试栈（`src/**/*.test.tsx` 为空，devDependencies 无 `@testing-library/*`），hook 内的判定无法用 vitest 直接覆盖。把门控与编排放在模块级函数后，F2–F4 全部可测，hook 只做转调。

### 4.2 判定规则

```
busy = isStreaming || hasPendingToolCalls(messages.flatMap(m => m.toolCalls ?? []))
```

`hasPendingToolCalls` 的入参是工具调用列表，不是消息列表，取值为 `flatMap` 后传入，不做其他过滤。

三条约束：

- **复用既有判定**：`hasPendingToolCalls` 已覆盖 `running` 与 `awaiting_user`，而审批与提问都收敛为 `awaiting_user`（§3 事实 3、4），因此不引入第二个信号，也不解析审批载荷。
- **不新增 store 字段**：门控是派生值。把它写进 `orbStore` 会形成第二份状态，需要订阅与清理，反而制造漂移。取值走 `getState()`，只在事件回调里调用，不参与渲染。
- **保守优先**：`isStreaming` 是全局信号，切到别的会话仍为真时会多拦一次自动收起。这比「漏拦导致面板消失」代价低，接受该偏差（回退方案见 §11）。

### 4.3 拦截点与豁免顺序

失焦路径的判定顺序固定为：

| 顺序 | 判定 | 命中行为 |
|---|---|---|
| 1 | `keepExpanded` | 返回 |
| 2 | `isOrbBlurCollapseSuppressed()` | 返回 |
| 3 | `isDragging` | 返回 |
| 4 | `isOrbBusy()` | 返回（新增） |
| 5 | 指针是否仍在窗口内 | 不在则收起 |

新增项插在既有豁免之后、坐标判定之前：前面三条是「用户意图」与「操作进行中」，门控是「任务进行中」，优先级低于用户显式意图。

### 4.4 与穿透的关系（事实澄清）

`collapseOrbPanel` 在收起完成后按 `clickThrough` 恢复穿透，这不是缺陷：穿透态下球本就不接收鼠标事件，展开只能靠全局快捷键，这是该配置项的既定语义。门控在「未收起」时提前返回，因此不会触发穿透恢复，也不会出现「面板在、鼠标却穿透」的中间态。本阶段不改变这条逻辑，仅要求门控提前返回时**不改任何 store 状态、不调用任何 `orbService` 方法**（F2 断言）。

### 4.5 测试环境前置

`collapseOrbPanel` 的 `waitForPanelTransition` 直接查 `document.querySelector(".orb-panel")`，而 vitest 未启用 jsdom（`vitest.config.ts` 无 `environment` 配置，`src/testSetup.ts` 只补了空 `window`）。因此 C1 必须顺带一处健壮性改动：

- `waitForPanelTransition` 增加 `typeof document === "undefined"` 早退（返回已 resolve 的 Promise），使该函数在非浏览器环境不抛错。

否则 F3/F4 无法在没有 DOM 的测试里跑到「恢复穿透」分支。

### 4.6 断言与验收

| 编号 | 断言 |
|---|---|
| F1 | `orbBusyFrom` 真值表：`isStreaming` 为真 → busy；`messages` 含 `running` → busy；含 `awaiting_user` → busy；两者皆无 → 不 busy |
| F2 | busy 时调 `autoCollapseOrb`：`isExpanded` 保持为真，且未调用 `orbService.collapseOrbPanel` / `setOrbClickThrough` |
| F3 | 非 busy 时调 `autoCollapseOrb`：正常收起并恢复穿透（按 `clickThrough` 配置） |
| F4 | busy 时调 `collapse`（显式）：正常收起 |
| F5 | 非 busy 且已收起时调 `autoCollapseOrb`：不调用任何 `orbService` 方法（`collapseOrbPanel` 自身早退） |

测试位置：`src/orb/__tests__/orbBusy.test.ts`（F1）、`src/orb/__tests__/orbAutoCollapse.test.ts`（F2–F5）。`orbService` 用 `vi.mock` 替换；`getConfValue("orb")` 在未加载配置时返回 `undefined`，F3 需显式写入 `clickThrough: true` 才能断言恢复穿透。

### 4.7 依赖

F2–F5 依赖 C1.1（`autoCollapseOrb` 落地）与 C1.5（`waitForPanelTransition` 早退）；F1 无依赖。

---

## 5 阶段 C2 — 审批档位常驻可见

### 5.1 终态结构

`ToolToolbar` 的单一 chip 内部改为两段文本，档位在前、摘要在后，中间用 `·` 分隔：

```
[ 手动审批 · 工具 3/8 · 上限 沙箱 ]
```

约束：

- 档位段必须在前。`.chip` 已有 `max-width: 200px` 与 `.label` 的尾部省略号，档位在前才不会被裁掉。
- 两段共用同一个 `Button`，点击行为仍为 `onToggle`，不新增第二个点击目标。
- 不新增颜色语义。项目现有设计体系没有「档位色」的定义，为档位发明颜色会与审批卡片的语义冲突。
- 档位段单独一个 `<span>` 并设 `flex: 0 0 auto`（不进省略号容器），摘要段沿用 `.label`；否则两段同处一个省略容器时，窄宽度下会先吃掉档位。

### 5.2 档位文案

| 档位 | 档位名（zh） | 档位名（en） | 说明（zh） | 说明（en） |
|---|---|---|---|---|
| `none` | 直接运行 | Run directly | 所有工具直接执行，不弹审批卡片 | All tools run without asking |
| `dangerous_only` | 默认 | Default | 读取自动放行，修改和执行需确认 | Reads run freely; edits and commands need approval |
| `all_os_tools` | 手动审批 | Manual approval | 所有工具都需确认，包括读取 | All tools need approval, including reads |

档位名只承担「模式名」，条件与范围交给说明。说明出现在两处：装配面板档位下拉的选项行（`optionRender` 两行渲染），以及审批卡片的「审批级别」理由行；chip 与 tooltip 只显示档位名。

说明措辞必须落到具体动作（读取 / 修改 / 执行），不使用「危险操作」这类需要二次解释的概括词——它无法回答「什么算危险」，这正是 `dangerous_only` 这个档位最容易被误读的地方。

新增 i18n 键 `chat.tool.approval.level.{none,dangerous_only,all_os_tools}`（档位名）与 `chat.tool.approval.level_hint.{...}`（说明）；`settings.agent_mode.field.approval_level.options.*` 的档位名与 `chat.tool.approval.level.*` 保持同字面。

档位下拉不得被面板宽度裁切：`Select` 需 `popupMatchSelectWidth={false}`（弹层按内容定宽，而非跟随 132px 的输入框），并因选项是两行固定高度而关闭虚拟滚动（`virtual={false}`），避免按单行高度估算导致错位。

### 5.3 真源与「发送前重新应用」

| 问题 | 结论 |
|---|---|
| 显示值来自哪里 | `config.approvalLevel`，即 `effectiveToolConfig`（会话级持久值；agent 链路缺失时回退模式初始值，见 §3 事实 11） |
| 发送时用的是什么 | 同一份 `route.toolConfig`，由每轮 send 携带（§3 事实 12），后端按它构策略 |
| 是否需要「发送前重新应用」 | 不需要。本项目审批档位只有会话级一个真源，chip 是它的只读投影，发送时读取的也是同一份值，不存在显示值与生效值分离的路径 |
| 是否需要写回 | 不需要。写回会形成第二份偏好，与工具面板的编辑互相覆盖 |

结论：C2 只做可见性，不做任何写回或同步逻辑。

### 5.4 宽度约束

- `.chat-tool-bar` 是单行布局，420px 面板下不得换行。
- chip 宽度上限保持 200px；溢出时只允许摘要段被省略，档位段必须完整显示。
- 不为此引入 `compact` 属性或容器查询；档位在前 + 既有省略规则已满足约束。

### 5.5 tooltip

`chat.tool.toolbar.hint` 扩展为「档位名 + 说明」的句子（如「会话工具装配 · 审批级别：默认（读取自动放行，修改和执行需确认）」），使档位名被误解时有解释入口。实现上 `hint` 增加 `{{level}}` 与 `{{hint}}` 两个插值，分别取 `chat.tool.approval.level.*` 与 `chat.tool.approval.level_hint.*`。

### 5.6 断言与验收

不设渲染断言：仓库内无组件测试栈，不为单个 chip 引入渲染测试依赖；改由 AC3、AC4 人工验收。

---

## 6 阶段 C3 — 常驻层级与启动属性

### 6.1 终态结构

`src-tauri/src/window.rs` 新增 `apply_orb_window_attrs(app: &AppHandle)`，读取 `OrbConf` 后对 orb 窗口应用两个属性：

| 属性 | 取值 |
|---|---|
| `set_always_on_top` | `conf.always_on_top` |
| `set_visible_on_all_workspaces` | `conf.always_on_top` |

两处失败均只 `log::warn!`，不返回错误：窗口属性是尽力而为，不应阻断启动流程。

调用点两处：

1. `src-tauri/src/setup.rs` 在 `show_orb_window` **之前**调用一次（紧邻 `position_orb_window`，保证「先收敛属性再显示」，不产生启动跳变）。
2. `src-tauri/src/cmd/config.rs` 的 `ORB_SIDE_EFFECTS` 中 `always_on_top` 分支改为调用同一函数（替代现在只设 `set_always_on_top` 的内联写法），保证变更与启动走同一条代码。

### 6.2 平台事实与不加 cfg 的理由

- tauri 2.12.5 提供 `WebviewWindow::set_visible_on_all_workspaces`，文档标注 Windows / iOS / Android 不支持。
- 底层 tao 0.37.2 把该方法用 `#[cfg(any(target_os = "macos", target_os = "linux"))]` 包住，Windows 上编译为空操作，**不返回错误**。
- 因此不写 `#[cfg(target_os)]`：Windows 上调用无害且无需分支；若未来依赖版本改为返回错误，再补平台判断（列为 §11 风险）。
- 语义绑定 `always_on_top` 而不新增设置项：新增设置需要 Rust struct、`*ConfDto`、`contract:gen` 三处同步，成本高于收益；且「不置顶却跨所有桌面」本身没有使用场景。

### 6.3 顺带修复的启动属性缺陷

`OrbConf.always_on_top` 目前只在变更时应用（§3 事实 15），用户设为 false 后重启仍置顶。C1 不涉及，本阶段一并修复：启动应用是 C3 的必然产物，不是额外改动。

### 6.4 断言与验收

后端无单测（窗口属性依赖真实窗口对象）；由 AC5、AC6 人工验收，门禁以 `cargo clippy -D warnings`、`cargo fmt --check` 与编译通过为准。

---

## 7 阶段 C4 — 冷启动锚点

### 7.1 缺陷

无保存位置时锚点为「主屏整屏右下 − `edge_margin`」，`edge_margin = 20`（§3 事实 13），而屏幕边界取整屏 bounds（§3 事实 14）。Windows 任务栏通常 40–48px，因此球首次出现时底边落在任务栏区域内，遮挡任务栏图标并抢占其点击。

### 7.2 修法

| 改动 | 内容 |
|---|---|
| `placement.rs` 新增 | `anchor_bottom_right(area: &ScreenBounds, size: u32, margin: i32) -> (i32, i32)`，纯函数，输入为可用区右下角 |
| `placement.rs` 新增 | `get_primary_work_area(app) -> Option<ScreenBounds>`，用 `primary_monitor()` 的 `work_area()` 构造（`PhysicalRect` 的 `position` / `size` 直接映射到 `ScreenBounds`） |
| `setup.rs` 改 | 无保存位置分支改用 `get_primary_work_area` + `anchor_bottom_right`；取不到可用区时回退现有的 `screens.first()` 逻辑 |

`edge_margin` 保持 20，含义从「距整屏边缘」变为「距可用区边缘」，行为随之修正。

### 7.3 断言

`src-tauri/crates/assistant-orb/tests/placement.rs` 新增用例，只覆盖纯函数（可用区来自操作系统，不做端到端断言）：

| 编号 | 断言 |
|---|---|
| B1 | 常规可用区（整屏高 1080、可用区高 1040）下返回值落在可用区右下角内缩 20px |
| B2 | 可用区高度小于整屏高度时，锚点 y + size 不超过可用区底边 |
| B3 | 既有 placement 与 hover 用例全绿（回归） |

### 7.4 非目标说明

吸附与纠正仍按整屏 bounds（`get_available_screens` 不改）。因此拖拽到任务栏区域仍可能出现重叠，这是既有行为，与冷启动锚点无关；统一到 work_area 会改变贴边吸附的落点，需要独立评估，不在本阶段范围。

---

## 8 验收条目

| 编号 | 验收内容 |
|---|---|
| AC1 | 在 orb 中发起一轮会调用工具的对话，切到别的窗口：面板不自动收起；生成结束后失焦可正常自动收起 |
| AC2 | 等待审批或提问期间失焦：面板不收起，审批卡与提问卡保持可见可点 |
| AC3 | 装配面板把档位改为「手动审批」，概览条 chip 立即显示「手动审批」 |
| AC4 | 新开会话后概览条显示模式初始值；发送后后端策略与 chip 显示一致（对照审批卡片出现的频率） |
| AC5 | macOS：切到另一个 Space 后球仍可见；关闭「置顶」后不再跨 Space（人工，macOS 机器） |
| AC6 | Windows：启动无报错，置顶行为与「置顶」配置一致；把配置改为 false 后重启，球不再置顶 |
| AC7 | 清空保存位置后重启：球出现在可用区右下角，不覆盖任务栏 |
| AC8 | `cargo test -p assistant-orb --test placement`（cwd `src-tauri`）与 `npm run test -- src/orb` 全绿；`npm run typecheck`、`npm run lint` 通过 |

---

## 9 文件改动清单

用于核对 G5（不新增持久化字段 / 不改 DTO / 不新增设置项）。

| 层 | 文件 |
|---|---|
| 前端新建 | `src/orb/orbBusy.ts` |
| 前端改 | `src/orb/OrbExpand.ts`、`src/orb/OrbPanel.tsx`、`src/chat/tool/ToolToolbar.tsx`、`src/chat/tool/ToolToolbar.module.css` |
| i18n | `src/i18n/locales/zh-CN.json`、`src/i18n/locales/en-US.json` |
| 前端测试 | `src/orb/__tests__/orbBusy.test.ts`、`src/orb/__tests__/orbAutoCollapse.test.ts` |
| 后端改 | `src-tauri/src/setup.rs`、`src-tauri/src/window.rs`、`src-tauri/src/cmd/config.rs` |
| 后端 crate 改 | `src-tauri/crates/assistant-orb/src/placement.rs`、`src-tauri/crates/assistant-orb/tests/placement.rs` |

不含：`tauri.conf.json`（C3 只改运行时属性，不动窗口配置）、`assistant-config`（不新增设置项）、`protocol/dto`（不改 DTO）。

---

## 10 非目标与挂起项

### 10.1 边缘停靠（不做）

理由：它需要新的窗口几何、新的停靠态、与 `ORB_SNAP_BORDER_X` 的优先级定义，以及在 Windows 上重定义 `clickThrough` 的命中区语义。代价集中在本规格明确不碰的三处（吸附、穿透、持久化），而收益仅是「减少遮挡」；在 84px 球 + 420px 面板的现有形态下，遮挡尚未构成用户可感知的问题。

### 10.2 截图排除与 HID 穿透 guard（条件挂起）

触发条件：I35 中的「agent 驱动本应用做渲染面自测」真正推进。届时值得带走的只有一条设计：capture 与 input 两个独立计数，只在计数跨零时切换，避免嵌套调用导致 overlay 闪烁。在此之前零收益，不实施。

### 10.3 收起态运行指示（暂不做）

门控只在「面板开着」时保护可见性；用户显式收起后仍看不到流式与等待审批。补齐需要球上状态点或角标，属新增视觉，与本规格「不改球本体视觉」的边界冲突，单独立项。

---

## 11 风险与回退

| 风险 | 表现 | 回退 |
|---|---|---|
| 门控过宽 | `isStreaming` 为全局信号，在别的会话等待时也会拦住当前会话球的自动收起 | 把 `orbBusyFrom` 的 `isStreaming` 项改为「当前会话的 attempt 匹配」，只改一处纯函数 |
| 档位段被裁 | chip 宽度不足时省略号切到档位 | 档位段独立 `span` + `flex: 0 0 auto` 已避免；若仍发生则降低摘要段优先级 |
| 测试依赖 DOM | `waitForPanelTransition` 在无 DOM 环境抛错 | C1 §4.5 已加 `typeof document === "undefined"` 早退 |
| 依赖升级改变平台语义 | 未来 tao/tauri 在 Windows 上返回 Unsupported | 补 `#[cfg(any(target_os = "macos", target_os = "linux"))]` 并保留 Windows 分支为空 |
| 启动应用置顶改变观感 | 用户此前配置 false 却一直置顶，修复后首次启动观感变化 | 属修正而非回归；在更新说明中标注 |
| 冷启动锚点仅修一条路径 | 拖拽到任务栏仍可能重叠 | 已列为 C4 §7.4 非目标，需要时独立评估 |

---

## 12 事实来源

本规格只依据两类可核对的事实，不含任何转述：

| 事实 | 来源 | 核对状态 |
|---|---|---|
| 本项目侧全部现状事实（§3 表 18 项） | §3「位置」列所列文件 | 已逐条源码核对 |
| tauri 2.12.5 / tao 0.37.2 的平台行为 | `src-tauri/Cargo.lock` + 依赖源码（`Monitor::work_area`、`set_visible_on_all_workspaces`） | 已核对 |

四项改造的判定依据全部落在本项目内：门控依据 §3 事实 2–4（`isStreaming` 与 `hasPendingToolCalls` 已覆盖全部等待态），档位可见依据事实 10–12（chip 与发送共用同一份 `effectiveToolConfig`），窗口属性依据事实 15–17（配置项存在、平台 API 可用），锚点依据事实 13–14（整屏 bounds 与任务栏高度之差）。因此不需要任何外部参考实现即可实施与验收。

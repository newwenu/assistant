# Orb 运行态与可见性规格

| 项 | 值 |
|---|---|
| 状态 | 规格（定义终态与判定，不含实施记录） |
| 日期 | 2026-11-13 |
| 关联 | 分析《orb-reference-value-analysis.md》；会话装配 I38d §4.7；审批策略 I38g；渲染面自测 I35-screenshot-qa-spec.md |
| 影响面 | 前端 4 文件（含 1 新建）+ 2 i18n；后端 3 文件；测试 2 文件 |

定位：分析文档已判定「参考项目球本体的 16 项能力中，去掉边缘停靠后只剩运行态门控、档位常驻可见、常驻层级三项有增量，另有一项冷启动锚点缺陷与停靠无关」。本规格只把这三项和这个缺陷建成可判定的终态，不重复分析结论，也不再评估已排除项。

---

## 1 目标与判定口径

### 1.1 可判定目标

| 编号 | 目标 | 判定方式 |
|---|---|---|
| G1 | 流式进行中或存在等待用户处理的工具调用时，**失焦自动收起被拦截**；显式收起不受影响 | F2 + F3 + F4 |
| G2 | 会话审批档位在输入区概览条上**始终可见**，且与发送时实际生效的配置同源 | AC3 + AC4 |
| G3 | 球在切换虚拟桌面/空间后仍可见（macOS、Linux），且启动时窗口属性与配置一致 | AC5 + AC6 |
| G4 | 无保存位置的冷启动不再把球放进任务栏区域 | B1 + B2 + AC7 |
| G5 | 不新增持久化字段、不改 DTO、不新增设置项 | 全量改动文件清单复核（§8 文件表） |

### 1.2 非目标

- 不做边缘停靠竖条（放弃理由见 §9.1）。
- 不改 `ORB_SNAP_BORDER_X` 吸附、拖拽 clamp、展开方向滞回、位置持久化。
- 不改 `clickThrough` 语义（§3.4 澄清）。
- 不在球上新增状态点、角标、进度环等视觉元素（§9.3）。
- 不把吸附与纠正的边界从整屏 bounds 改为 work_area（§6.4）。
- 不做截图排除与 HID 穿透 guard（§9.2，条件挂起）。

### 1.3 编号空间

| 前缀 | 含义 |
|---|---|
| S | 改造步骤（一次性动作） |
| B | 后端断言（Rust 测试） |
| F | 前端断言（vitest） |
| AC | 验收条目 |

---

## 2 现状基线

以下事实全部经源码核对，是后续判定的引用依据；规格不复述实现细节，只标注「现状结论」。

| # | 事实 | 位置 | 现状结论 |
|---|---|---|---|
| 1 | `orbStore` 只有展开、悬停、拖拽、可见、固定、位置、方向七类字段 | `src/stores/orbStore.ts` L5-L23 | 无任何运行态或等待态字段；门控信号不能来自 orb 侧 |
| 2 | `isStreaming` 是全局流式信号 | `src/stores/chatStreamStore.ts` L5 | 可直接作为「正在生成」的信号源 |
| 3 | `hasPendingToolCalls` 判定 `running` 与 `awaiting_user` | `src/chat/core/toolCallView.ts` L155-L159 | 一个判定覆盖「工具执行中」 |
| 4 | `awaitingApproval` 与 `awaitingAnswer` 均收敛为 `awaiting_user` | `src/chat/core/toolCallView.ts` L97-L127 | 同一判定同时覆盖「等待审批」与「等待回答」，不需要第二个信号 |
| 5 | 失焦自动收起是唯一的自动收起路径，已有 `keepExpanded`、`isDragging`、`suppress` 三条豁免 | `src/orb/OrbPanel.tsx` L118-L145 | 门控只需插在这一条路径上 |
| 6 | Esc 与顶栏收起按钮直接调用 `collapse()` | `src/orb/OrbPanel.tsx` L163-L176、L340 | 属于显式路径，不应被门控拦截 |
| 7 | 收起完成后按 `clickThrough` 恢复穿透 | `src/orb/OrbExpand.ts` L62-L86 | 穿透只在「真的收起」时恢复，门控提前返回不会误开穿透 |
| 8 | 展开时按 `clickThrough` 关闭穿透；`collapse` 是无参数强收起 | `src/orb/OrbExpand.ts` L190-L203 | 需要新增一个受门控的入口，而不是改 `collapse` 语义 |
| 9 | 概览条只展示「已启用数量 + tier 上限」 | `src/chat/tool/ToolToolbar.tsx` L31-L37 | `approvalLevel` 完全不可见 |
| 10 | 每轮发送携带 `toolConfig` | `src/services/stream/turnOrchestrator.ts` L372 | 后端按本次发送的配置构策略，「发送前重新应用」已由传输形态满足 |
| 11 | 冷启动锚点 = 主屏整屏右下 − 20px | `src-tauri/src/setup.rs` L210-L243 | 边距小于任务栏高度，球落入任务栏区域 |
| 12 | 屏幕信息取 monitor 的 `position` / `size` | `src-tauri/crates/assistant-orb/src/placement.rs` L13-L51 | 是整屏 bounds，不含可用区 |
| 13 | `always_on_top` 只在配置变更时应用 | `src-tauri/src/cmd/config.rs` L91-L100 | 启动时不应用，与配置文件可能不一致 |
| 14 | orb 窗口 `alwaysOnTop` 为 true、`skipTaskbar` 为 true、`visible` 为 false | `src-tauri/tauri.conf.json` L25-L39 | 启动态固定为置顶，与配置项默认值一致，但用户改为 false 后启动不生效 |
| 15 | 依赖版本 tauri 2.12.5 | `src-tauri/Cargo.lock` L6341-L6342 | `Monitor::work_area()` 与 `set_visible_on_all_workspaces` 均可用 |

---

## 3 S1 运行态门控

### 3.1 终态结构

新建 `src/orb/orbBusy.ts`，只放两个导出：

| 导出 | 签名 | 职责 |
|---|---|---|
| `orbBusyFrom` | `(isStreaming: boolean, messages: Message[]) => boolean` | 纯函数，唯一判定实现 |
| `isOrbBusy` | `() => boolean` | 从两个 store 取值后转调 `orbBusyFrom` |

`useOrbExpand` 的返回值新增 `autoCollapse`，`collapse` 保持无参数强收起：

| 入口 | 语义 | 是否门控 |
|---|---|---|
| `collapse` | 强收起 | 否 |
| `autoCollapse` | 自动收起（失焦触发） | 是 |

`OrbPanel` 的失焦监听改调 `autoCollapse`；Esc、顶栏收起按钮、历史菜单动作、初始化收起仍走 `collapse` / `collapseOrbPanel`。

### 3.2 判定规则

```
busy = isStreaming || hasPendingToolCalls(messages.flatMap(m => m.toolCalls ?? []))
```

`hasPendingToolCalls` 的入参是工具调用列表，不是消息列表，取值为 `flatMap` 后传入，不做其他过滤。

三条约束：

- **复用既有判定**：`hasPendingToolCalls` 已覆盖 `running` 与 `awaiting_user`，而审批与提问都收敛为 `awaiting_user`（§2 事实 3、4），因此不引入第二个信号，也不解析审批载荷。
- **不新增 store 字段**：门控是派生值。把它写进 `orbStore` 会形成第二份状态，需要订阅与清理，反而制造漂移。取值走 `getState()`，只在事件回调里调用，不参与渲染。
- **保守优先**：`isStreaming` 是全局信号，切到别的会话仍为真时会多拦一次自动收起。这比「漏拦导致面板消失」代价低，接受该偏差（回退方案见 §10）。

### 3.3 拦截点与豁免顺序

失焦路径的判定顺序固定为：

| 顺序 | 判定 | 命中行为 |
|---|---|---|
| 1 | `keepExpanded` | 返回 |
| 2 | `isOrbBlurCollapseSuppressed()` | 返回 |
| 3 | `isDragging` | 返回 |
| 4 | `isOrbBusy()` | 返回（新增） |
| 5 | 指针是否仍在窗口内 | 不在则收起 |

新增项插在既有豁免之后、坐标判定之前：前面的三条是「用户意图」与「操作进行中」，门控是「任务进行中」，优先级低于用户显式意图。

### 3.4 与穿透的关系（事实澄清）

`collapseOrbPanel` 在收起完成后按 `clickThrough` 恢复穿透，这不是缺陷：穿透态下球本就不接收鼠标事件，展开只能靠全局快捷键，这是该配置项的既定语义。门控在「未收起」时提前返回，因此不会触发穿透恢复，也不会出现「面板在、鼠标却穿透」的中间态。规格不改变这条逻辑，仅要求门控提前返回时**不改任何 store 状态、不调用任何 `orbService` 方法**（F2 断言）。

---

## 4 S2 审批档位常驻可见

### 4.1 终态结构

`ToolToolbar` 的单一 chip 内部改为两段文本，档位在前、摘要在后，中间用 `·` 分隔：

```
[ 危险确认 · 工具 3/8 · 上限 沙箱 ]
```

约束：

- 档位段必须在前。`.chip` 已有 `max-width: 200px` 与 `.label` 的尾部省略号，档位在前才不会被裁掉。
- 两段共用同一个 `Button`，点击行为仍为 `onToggle`，不新增第二个点击目标。
- 不新增颜色语义。项目现有设计体系没有「档位色」的定义，为档位发明颜色会与审批卡片的语义冲突。

### 4.2 短标签映射

| 档位 | 短标签（zh） | 短标签（en） |
|---|---|---|
| `none` | 自动 | Auto |
| `dangerous_only` | 危险确认 | Risky only |
| `all_os_tools` | 全确认 | All confirm |

新增 i18n 键 `chat.tool.toolbar.level_short.{none,dangerous_only,all_os_tools}`（zh-CN 与 en-US 各一份）。完整档位文案继续复用 `chat.tool.approval.level.*`，只出现在 tooltip 中。

### 4.3 真源与「发送前重新应用」

| 问题 | 结论 |
|---|---|
| 显示值来自哪里 | `route.toolConfig.approvalLevel`，即会话级持久值；agent 链路缺失时回退模式初始值（既有 `resolveEffectiveToolConfig`） |
| 发送时用的是什么 | 同一份 `route.toolConfig`，由每轮 send 携带（§2 事实 10），后端按它构策略 |
| 是否需要「发送前重新应用」 | 不需要。参考项目该语义存在，是因为其权限是独立全局偏好、可与会话漂移；本项目只有一个真源，chip 是只读投影，不存在漂移 |
| 是否需要写回 | 不需要。写回会形成第二份偏好，与工具面板的编辑互相覆盖 |

结论：S2 只做可见性，不做任何写回或同步逻辑。

### 4.4 宽度约束

- `.chat-tool-bar` 是单行布局，420px 面板下不得换行。
- chip 宽度上限保持 200px；溢出时只允许摘要段被省略，档位段必须完整显示。
- 不为此引入 `compact` 属性或容器查询；档位在前 + 既有省略规则已满足约束。

### 4.5 tooltip

`chat.tool.toolbar.hint` 扩展为带档位完整文案的句子（如「会话工具装配：仅危险操作需确认」），使短标签被误解时有解释入口。

---

## 5 S3 常驻层级与启动属性

### 5.1 终态结构

`src-tauri/src/window.rs` 新增 `apply_orb_window_attrs(app)`，读取 `OrbConf` 后对 orb 窗口应用两个属性：

| 属性 | 取值 |
|---|---|
| `set_always_on_top` | `conf.always_on_top` |
| `set_visible_on_all_workspaces` | `conf.always_on_top` |

调用点两处：

1. `src-tauri/src/setup.rs` 在 `show_orb_window` **之前**调用一次（与 `position_orb_window` 相邻，保证「先收敛属性再显示」，不产生启动闪烁）。
2. `src-tauri/src/cmd/config.rs` 的 `ORB_SIDE_EFFECTS` 中 `always_on_top` 分支同时改两个属性。

### 5.2 平台事实与不加 cfg 的理由

- tauri 2.12.5 提供 `set_visible_on_all_workspaces`，文档标注 Windows / iOS / Android 不支持。
- 底层 tao 0.37.2 把该方法用 `#[cfg(any(target_os = "macos", target_os = "linux"))]` 包住，Windows 上编译为空操作，**不返回错误**。
- 因此不写 `#[cfg(target_os)]`：Windows 上调用无害且无需分支；若未来依赖版本改为返回错误，再补平台判断（列为 §10 风险）。
- 语义绑定 `always_on_top` 而不新增设置项：新增设置需要 Rust struct、`*ConfDto`、`contract:gen` 三处同步，成本高于收益；且「不置顶却跨所有桌面」本身没有使用场景。

### 5.3 顺带修复的启动属性缺陷

`OrbConf.always_on_top` 目前只在变更时应用（§2 事实 13），用户设为 false 后重启仍置顶。S1 不涉及，本步骤一并修复：启动应用是 S3 的必然产物，不是额外改动。

---

## 6 S4 冷启动锚点

### 6.1 缺陷

无保存位置时锚点为「主屏整屏右下 − `edge_margin`」，`edge_margin = 20`（§2 事实 11），而屏幕边界取整屏 bounds（§2 事实 12）。Windows 任务栏通常 40-48px，因此球首次出现时底边落在任务栏区域内，遮挡任务栏图标并抢占其点击。

### 6.2 修法

| 改动 | 内容 |
|---|---|
| `placement.rs` 新增 | `anchor_bottom_right(area: &ScreenBounds, size: u32, margin: i32) -> (i32, i32)`，纯函数，输入为可用区右下角 |
| `placement.rs` 新增 | `get_primary_work_area(app) -> Option<ScreenBounds>`，用 `primary_monitor()` 的 `work_area()` 构造 |
| `setup.rs` 改 | 无保存位置分支改用 `get_primary_work_area` + `anchor_bottom_right`；取不到可用区时回退现有的 `screens.first()` 逻辑 |

`edge_margin` 保持 20，含义从「距整屏边缘」变为「距可用区边缘」，行为随之修正。

### 6.3 断言

`src-tauri/crates/assistant-orb/tests/placement.rs` 新增两个用例，覆盖纯函数即可（可用区来自操作系统，不做端到端断言）。

### 6.4 非目标说明

吸附与纠正仍按整屏 bounds（`get_available_screens` 不改）。因此拖拽到任务栏区域仍可能出现重叠，这是既有行为，与冷启动锚点无关；统一到 work_area 会改变贴边吸附的落点，需要独立评估，不在本规格范围。

---

## 7 断言清单

### 7.1 后端

| 编号 | 断言 | 位置 |
|---|---|---|
| B1 | `anchor_bottom_right` 在常规可用区（整屏高 1080、可用区高 1040）下返回值落在可用区右下角内缩 20px | `assistant-orb/tests/placement.rs` |
| B2 | 可用区高度小于整屏高度时，锚点 y + size 不超过可用区底边 | 同上 |
| B3 | 既有 placement 与 hover 用例全绿（回归） | 同上 + `tests/hover.rs` |

### 7.2 前端

| 编号 | 断言 | 位置 |
|---|---|---|
| F1 | `orbBusyFrom` 真值表：`isStreaming` 为真 → busy；`messages` 含 `running` → busy；含 `awaiting_user` → busy；两者皆无 → 不 busy | `src/orb/__tests__/orbBusy.test.ts` |
| F2 | busy 时调 `autoCollapse`：`isExpanded` 保持为真，且未调用 `orbService.collapseOrbPanel` / `setOrbClickThrough` | `src/orb/__tests__/orbAutoCollapse.test.ts` |
| F3 | 非 busy 时调 `autoCollapse`：正常收起并恢复穿透（按 `clickThrough` 配置） | 同上 |
| F4 | busy 时调 `collapse`（显式）：正常收起 | 同上 |

S2 不设渲染断言：仓库内无组件测试（`src/**/*.test.tsx` 为空），不为单个 chip 引入渲染测试栈；改由 AC3、AC4 人工验收。

---

## 8 验收条目

| 编号 | 验收内容 |
|---|---|
| AC1 | 在 orb 中发起一轮会调用工具的对话，切到别的窗口：面板不自动收起；生成结束后失焦可正常自动收起 |
| AC2 | 等待审批或提问期间失焦：面板不收起，审批卡与提问卡保持可见可点 |
| AC3 | 装配面板把档位改为「全确认」，概览条 chip 立即显示「全确认」 |
| AC4 | 新开会话后概览条显示模式初始值；发送后后端策略与 chip 显示一致（对照审批卡片出现的频率） |
| AC5 | macOS：切到另一个 Space 后球仍可见；关闭「置顶」后不再跨 Space（人工，macOS 机器） |
| AC6 | Windows：启动无报错，置顶行为与「置顶」配置一致；把配置改为 false 后重启，球不再置顶 |
| AC7 | 清空保存位置后重启：球出现在可用区右下角，不覆盖任务栏 |
| AC8 | `cargo test -p assistant-orb --test placement`（cwd `src-tauri`）与 `npm run test -- src/orb` 全绿；`npm run typecheck`、`npm run lint` 通过 |

文件改动清单（用于核对 G5）：

| 层 | 文件 |
|---|---|
| 前端新建 | `src/orb/orbBusy.ts` |
| 前端改 | `src/orb/OrbExpand.ts`、`src/orb/OrbPanel.tsx`、`src/chat/tool/ToolToolbar.tsx` |
| i18n | `src/i18n/locales/zh-CN.json`、`src/i18n/locales/en-US.json` |
| 前端测试 | `src/orb/__tests__/orbBusy.test.ts`、`src/orb/__tests__/orbAutoCollapse.test.ts` |
| 后端改 | `src-tauri/src/setup.rs`、`src-tauri/src/window.rs`、`src-tauri/src/cmd/config.rs` |
| 后端 crate 改 | `src-tauri/crates/assistant-orb/src/placement.rs`、`src-tauri/crates/assistant-orb/tests/placement.rs` |

---

## 9 非目标与挂起项

### 9.1 边缘停靠（不做）

理由：它需要新的窗口几何、新的停靠态、与 `ORB_SNAP_BORDER_X` 的优先级定义，以及在 Windows 上重定义 `clickThrough` 的命中区语义。代价集中在本规格明确不碰的三处（吸附、穿透、持久化），而收益仅是「减少遮挡」；在 84px 球 + 420px 面板的现有形态下，遮挡尚未构成用户可感知的问题。

### 9.2 截图排除与 HID 穿透 guard（条件挂起）

触发条件：I35-screenshot-qa-spec.md 中的「agent 驱动本应用做渲染面自测」真正推进。届时值得带走的只有一条设计：capture 与 input 两个独立计数，只在计数跨零时切换，避免嵌套调用导致 overlay 闪烁。在此之前零收益，不实施。

### 9.3 收起态运行指示（暂不做）

门控只在「面板开着」时保护可见性；用户显式收起后仍看不到流式与等待审批。补齐需要球上状态点或角标，属新增视觉，与本规格「不改球本体视觉」的边界冲突，单独立项。

---

## 10 风险与回退

| 风险 | 表现 | 回退 |
|---|---|---|
| 门控过宽 | `isStreaming` 为全局信号，在别的会话等待时也会拦住当前会话球的自动收起 | 把 `orbBusyFrom` 的 `isStreaming` 项改为「当前会话的 attempt 匹配」，只改一处纯函数 |
| 档位段被裁 | chip 宽度不足时省略号切到档位 | 顺序约束已保证不会发生；若发生则降低摘要段优先级 |
| 依赖升级改变平台语义 | 未来 tao/tauri 在 Windows 上返回 Unsupported | 补 `#[cfg(any(target_os = "macos", target_os = "linux"))]` 并保留 Windows 分支为空 |
| 启动应用置顶改变观感 | 用户此前配置 false 却一直置顶，修复后首次启动观感变化 | 属修正而非回归；在更新说明中标注 |
| 冷启动锚点仅修一条路径 | 拖拽到任务栏仍可能重叠 | 已列为 §6.4 非目标，需要时独立评估 |

---

## 11 参考事实来源

| 参考项 | 来源 | 核对状态 |
|---|---|---|
| 运行态门控用单一布尔、显式收起不受门控 | 《orb-reference-value-analysis.md》转述 | 未核对源码（参考项目不在 `references/` 内） |
| 档位 chip 常驻可见 | 同上 | 未核对源码 |
| capture/input 双计数 guard | 同上 | 未核对源码（本规格不实施） |
| 边缘停靠的 5 条决策 | 同上 | 未核对源码（本规格不实施） |
| 本项目侧全部事实 | §2 表内位置 | 已逐条源码核对 |

参考项目 `deepseek-harness` 的 orb 应用文件未包含在本仓库 `references/1-ai-agent/deepseek-harness` 的快照中（现有 codex、goose、OpenBitFun、openhuman、deepseek-harness 五个目录），因此参考侧事实一律标为「转述」。本规格的三项改造都不依赖参考项目的实现细节：门控、档位展示、窗口属性均在项目内可独立定义，故转述不影响落地，无待回源码确认项。
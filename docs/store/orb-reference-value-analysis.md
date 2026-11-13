# Orb 参考价值判定：deepseek-harness-orb 球本体

> 日期：2026-11-12｜性质：**活跃判定文档**（只回答「参考项目里球本体这部分现在还有哪些增量价值」，不含实施步骤、排期与阶段划分）
> 事实基线：仓库当前代码逐点核对（见 §7）；参考项目源码与其 `.agents/notes/implemented/feature/` 决策笔记
> 关联：`docs/ARCHITECTURE.md` §3（Orb 现状）· `./I35-screenshot-qa-spec.md`（抓屏覆盖层，Draft）· I1-orb-touch-compat-spec（已归档）
> 范围界定：**只判定「球本体」**——窗口形态、几何、展开/收起、拖拽/吸边、常驻层级、球上的控件与偏好。**排除**依附球存在但属于外部生态的部分：Computer Use preset、划词工具栏、千分比坐标、TCC 门、`code_agent` 委派、subagent 子会话。

---

## 1 判定口径

参考项目 `references/1-ai-agent/deepseek-harness-orb` 的球本体是 Electron 侧的一整套实现，而本项目是 Tauri 2 + 单 webview 的 Orb。两者在**窗口技术栈**上不可移植（`BrowserWindow` / `type: 'panel'` / `setIgnoreMouseEvents` / `WeakMap` 挂窗口态均无 Tauri 等价物），因此判定分两层：

| 层 | 问题 | 可搬运性 |
|---|---|---|
| **决策层** | 这个交互/状态机该不该有 | 可搬，且这是价值主体 |
| **实现层** | 具体怎么写 | 基本不可搬，只作设计参照 |

本文的判定只有四种，不使用分级打分：

| 判定 | 含义 |
|---|---|
| **已覆盖** | 本项目已有等价能力，参考实现无增量信息 |
| **有增量** | 参考解决了本项目当前缺失的行为，且决策层可直接采纳 |
| **条件性** | 仅当某条尚未确定的方向成立时才有价值，否则无意义 |
| **不适用** | 前提不同（资产体系、Agent 抽象、窗口技术栈），照搬会造成降级 |

---

## 2 参考实现盘点（球本体部分）

| 文件 | 体量 | 职责 | 归属判定 |
|---|---|---|---|
| `apps/desktop/src/floating-window.ts` | 35 KB | 几何常量、展开方向、贴边停靠（dock）、右键菜单、overlay guard、常驻层级 | 球本体（主体） |
| `apps/desktop/renderer/floating.js` | 50 KB | 球与面板渲染页：悬停展开、固定、拖动、历史、停止、提问卡、TCC 门、发送 | 球本体（去掉提问卡/TCC/划词后） |
| `apps/desktop/renderer/floating.css` | 15 KB | 绕球折行输入胶囊、停靠竖条、面板外观 | 球本体（布局参照价值低） |
| `apps/desktop/renderer/floating.html` | 4 KB | 面板 DOM 骨架（历史 / Access chip / 新建 / 对话 / 输入 / 停止） | 球本体（控件清单可参照） |
| `apps/desktop/src/floating-session.ts` | 1.7 KB | 球会话 id 持久化 + `dsh_orb` 目录 | 已覆盖（本项目球面板直接复用 chat store 的会话 id） |
| `apps/desktop/src/floating-agent-menu.ts` | 4 KB | 右键动态重建模型菜单 | 不适用（模型抽象不同） |
| `apps/desktop/src/orb-agent-models.ts` | 3.7 KB | 球模型 vs 后台模型双选择 | 不适用 |
| `apps/desktop/src/orb-avatar.ts` | 8.1 KB | 自定义球头像（GIF/PNG/WebP + magic-byte 校验 + 缓存戳） | 不适用 |
| `apps/desktop/src/orb-permission.ts` | 2.2 KB | Access 三档偏好持久化 | 有增量（可见性层） |
| `packages/client/ui-settings-orb` | — | 设置中的「悬浮球」分页 | 已覆盖（本项目设置项已存在） |
| `packages/client/ui-overlay-chat` | — | overlay iframe 的 Compact ChatView 根 | 不适用（本项目同进程共享组件） |

对应的本项目落点：

| 关注点 | 本项目文件 |
|---|---|
| 窗口配置 | `src-tauri/tauri.conf.json`（`orb` 窗口）、`src-tauri/src/setup.rs` |
| 几何 / 展开 / 收起 | `src-tauri/src/window.rs`、`src-tauri/src/cmd/orb.rs`、`src-tauri/crates/assistant-orb`（`placement.rs` / `geometry.rs`，含 proptest） |
| 前端交互 | `src/orb/OrbBall.tsx`、`OrbDrag.tsx`、`OrbExpand.ts`、`OrbPanel.tsx`、`pointerGesture.ts`、`OrbCoord.ts` |
| 状态 | `src/stores/orbStore.ts` |
| 常量 | `src/config/constants.ts`、`assistant-config::constant` |
| 偏好 | `OrbConf`（`assistant-config/src/orb_conf.rs`）、`src/settings/core/field/fieldDefinitions.ts` 的 `settings.orb` 分组 |

---

## 3 逐项价值判定

| # | 能力 | 参考实现要点 | 本项目现状 | 判定 |
|---|---|---|---|---|
| 1 | 展开方向自适应（四象限 + 滞回） | `expandDirection` 按球心与工作区中心比较；球原点在展开矩形角落固定 | 等价（`OrbExpand.ts` + `window.rs`，另有 `EXPAND_DIRECTION_HYSTERESIS_PX` 滞回） | 已覆盖 |
| 2 | 窗口留白（chrome inset） | 12px，为投影与外圈描边留出空间 | 2px，投影在 webview 内绘制 | 已覆盖（数值差异是渲染方式差异，非缺失） |
| 3 | 拖动跟手 + 松手 clamp + 位置持久化 | 主键抓取、按球上偏移记录、pointer-up 才 clamp；**明确选择不记忆拖动位置**（重启回默认右沿） | 已实现跟手与 clamp，且**记忆位置**（`OrbConf.position_x/y`） | 已覆盖（本项目取舍更完整） |
| 4 | 贴边吸附 | 无「吸附」，只有跨边 1/5 后的停靠竖条 | 有吸附：边缘 24px 带内吸附 + 250ms 缓动（`ORB_SNAP_BORDER_X` / `ORB_SNAP_ANIMATION_MS`） | 已覆盖（吸附本身）；停靠缺口计入 #5 |
| 5 | 边缘停靠竖条（dock tab） | 越边 1/5 松手提交 → 整球滑出屏外 250ms → 缩成 6×72 竖条；悬停 800ms 滑回；上下边永不吸；展开清 dock；仅内存态 | 无 | **有增量（价值最高）** |
| 6 | 悬停展开 / 失焦收起 / 单击固定 | 悬停短延迟展开、单击 pin、取消 pin 后指针离开即折叠、pin 时不折叠 | 等价（`ORB_BLUR_COLLAPSE_DELAY_MS` 200ms + `keepExpanded` + 鼠标在窗口内时放弃折叠） | 已覆盖 |
| 7 | 运行态门控（不收起 / 不吸边） | `running \|\| asking() \|\| gatingTcc()` 统一为 `canDock=false`：拖拽不强制收起、clamp 不吸边、hover 不折叠、blur 不收起 | **无**：`orbStore` 无流式/待审批状态，收起只受 `keepExpanded` 控制 | **有增量** |
| 8 | 点击穿透 | 双计数 refcount（capture / input）+ 仅在计数跨零时切换 + 不转发鼠标事件 | 配置驱动 + Rust 原子量缓存（`ORB_CLICK_THROUGH`），值相同即跳过系统调用 | 已覆盖（本项目更简单，够用） |
| 9 | 球上控件布局 | 历史 / Access chip / 新建 挂在输入胶囊对侧；停止是胶囊内与球相对一端 | 历史 / 固定 / 收起 在面板顶栏，停止在输入区（`ConversationView` 提供） | 已覆盖（布局参照价值低，见 §5） |
| 10 | 权限档位常驻可见 | Access chip 三档常驻面板顶栏，随会话应用，每次发送前重新应用 | 能力存在（`ToolConfigPanel` 会话级装配），但**藏在工具面板里、球收起即不可见** | **有增量（可见性层）** |
| 11 | 常驻层级 / 跨 Space | macOS `type: 'panel'` + `setVisibleOnAllWorkspaces(visibleOnFullScreen)` + 更高 always-on-top level；Windows `screen-saver` 层级 | `tauri.conf.json` 仅布尔 `alwaysOnTop`，未设置跨 Space / 全屏可见 | **有增量** |
| 12 | 截图排除 + HID 点击穿透 guard | capture/input 双计数；Windows `setContentProtection`（`WDA_EXCLUDEFROMCAPTURE`）；macOS ScreenCaptureKit 窗口 id 排除；HID 期间整窗穿透且不转发 | 无；`./I35-screenshot-qa-spec.md` 走的是「抓屏前隐藏 orb 窗口」的另一条路 | **条件性**（见 §5.1） |
| 13 | 球上输入框绕球折行 | `contenteditable` + `shape-outside: circle()` + `--composer-height` 逐行长高，上限 72+3×20 | 输入区在面板内（`ChatLayout` composer），展开时球本身隐藏（`setBallVisible(false)`），无重叠可绕 | 不适用 |
| 14 | 自定义球头像 | GIF/PNG/WebP 安装、magic-byte 校验、2MB 上限、mtime 缓存戳 | Rive mascot + 表情状态机（`src/mascot`） | 不适用（照搬是降级） |
| 15 | 球模型与后台模型分离 | 右键两个子菜单分别选 overlay / background 模型，每次右键重建 | 模式级（`builtin-*`）与会话级模型选择 | 不适用（前提是「球锁死 Computer Use + 另开 code agent」，本项目无此分层） |
| 16 | 独立设置分页 | `settings.section` id `orb`，Linux 列出但全禁用 | `settings.orb` 分组已存在，分布在外观/通用页 | 已覆盖（不值得为参考的分页形态重排设置信息架构） |

---

## 4 有增量的三项（可采纳）

### 4.1 边缘停靠竖条（dock tab）

**解决什么**：常驻球始终占 84px 桌面，是「遮挡」与「不想丢掉球」之间唯一没有折中的地方。参考的做法是让球主动藏进屏幕边，只在边缘留一条可点标记。

参考参数（可直接作为设计基线）：

| 参数 | 值 | 语义 |
|---|---|---|
| `FLOATING_DOCK_OVERLAP` | `72/5 ≈ 14` | 球越过屏幕 **bounds**（非工作区）该像素即判定停靠 |
| `FLOATING_DOCK_OFF_GAP` | 2 | 滑出屏外的间隙 |
| `FLOATING_DOCK_SLIDE_OFF_MS` | 250 | 滑出时长 |
| `FLOATING_DOCK_TAB_WIDTH` / `HEIGHT` | 6 / 72 | 绘制的竖条尺寸 |
| `FLOATING_DOCK_GLOW` | 8 | 竖条光晕内边距 |
| `FLOATING_DOCK_HOVER_MARGIN` | 20 | 绘制区之外的命中带 |
| `FLOATING_DOCK_HIT_WIDTH` | 34 | 实际窗口宽度（6+8+20） |
| `FLOATING_DOCK_DRAG_OFF` | `72/3 = 24` | 把竖条向内拖过该距离即解除停靠 |
| `FLOATING_DOCK_IN_PAD` | 5 | 滑回后距屏边内缩 |
| `FLOATING_DOCK_SLIDE_IN_MS` | 300 | 滑回时长 |
| 悬停解锁延迟 | 800 ms | 刚停靠后需等该时长，避免停靠瞬间被自己的指针弹回 |

关键决策（这些才是价值，参数只是配套）：

1. **在 pointer-up 的 clamp 提交，不在拖动过程中提交**。拖动中立刻变竖条会把球藏到指针下方，且与滑出动画互斥。
2. **上下边永不停靠**，只做左右竖条。
3. **停靠态只存内存、不持久化**。重启仍回默认 72px 球，避免用户对「重启后只剩一条」感到意外。
4. **展开面板清 dock**；固定态不能停靠，须先经常规拖动收起。
5. **运行 / 待回答 / 门控可见时 `canDock=false`**，否则收起的球可能在 pointer-up 跨阈值时突然消失。

与本项目的接入约束：

- 本项目窗口留白只有 2px，参考的 12px 是为投影与描边预留；竖条光晕需重新按 2px 留白计算命中宽度，不能直接搬 34px。
- 停靠判定必须与 clamp 用**同一套屏幕坐标系**。本项目 `assistant-orb::placement::get_available_screens` 取的是 monitor 的 `position` / `size`（整屏 bounds，非 work_area），`ensure_orb_on_screen` 也按整屏判定，因此停靠判定可直接复用同一组 `ScreenBounds`，不需要引入第二套坐标；但 `setup.rs::position_orb_window` 的冷启动锚点是「主屏整屏右下角 − 20px 边距」，与参考的「工作区右沿 + 高度 8%」不同，两处锚点需对齐，否则会出现「首次启动整球压住任务栏」与「停靠判定认为已越界」的分歧。
- `ORB_SNAP_BORDER_X`（24px 吸附带）与停靠是两套行为，同时存在时要明确优先级：吸附负责「贴边停住」，停靠负责「藏起来」。当前 `snap_orb_to_edge` 只做前者。
- 停靠态下 `ORB_CLICK_THROUGH` 的语义要重新定义：竖条本身就是命中区，穿透开启时整窗不接收事件（见 `OrbConf.click_through` 现状）。

### 4.2 运行态门控（不收起 / 不吸边）

**解决什么**：本项目球收起后只剩 mascot 表情，用户看不到流式输出、工具执行进度与待审批卡；而 `orbStore` 目前只有 `isExpanded / isHovering / isDragging / isBallVisible / keepExpanded`，收起路径（`ORB_BLUR` 事件、Esc、顶栏收起按钮）**没有任何流式/审批判断**，完全依赖用户手动 pin。

更具体的一处风险：`collapseOrbPanel` 在收起后若 `OrbConf.clickThrough` 为真会重新开启穿透（`OrbExpand.ts`），于是「运行中收起 + 开启穿透」会让面板彻底不可点，只能靠全局快捷键重新唤起。

参考的门控是一个**单一布尔**：`canDock = !(running || asking() || gatingTcc())`，同一个值同时喂给 move、clamp、hover、blur 四条路径，因此不存在「某条路径忘了判断」的分裂。

与本项目的接入约束：

- 本项目需要两个状态源而不是一个：`stream.isStreaming`（在 `ConversationView` 里已有 `isBusy`）与待审批/待回答（`ApprovalCard` 的 `approval`、提问链路）。这些目前都在 chat store 内，orbStore 不感知，属于跨 store 只读依赖（符合 `docs/analysis/PROJECT-DESIGN-GUIDELINES.md` §2 的跨 slice 读取约定）。
- 门控应当**只阻止自动收起**，不阻止用户显式点收起按钮（参考也是这个语义：`setExpanded(false, force)`）。否则用户无法主动离开运行中的球。

### 4.3 权限档位常驻可见

**解决什么**：本项目能力齐全（`ToolConfigPanel` 的会话级装配 + `ToolAssemblySection` 的模式级初始值），但它是 agent 链路里一个需要主动展开的工具面板；球收起或面板未展开时，「当前是 sandbox 还是 dangerous_only、白名单是否为空」完全不可见。球是 AI 入口，装配档位是发送前的关键前提，做成常驻 chip 比藏在工具面板里更符合入口的定位。

参考的完整语义（不只是显示）：

1. chip 三档固定（`read-only` / `workspace-write` / `danger-full-access`），持久化到 profile（`orb-permission.ts`）。
2. 新会话创建时应用该档位；**每次发送前重新应用**（`/permission` 再下发），防止会话中途被改回。
3. 只对「cwd 匹配该工作区」的会话生效，其他工作区会话保持 Host 默认。
4. 发送时把 chip 上的档位写回该 Agent（双向同步，不是单向投影）。

与本项目的接入约束：

- 本项目已有 `approval_level`（`none` / `dangerous_only` / `all_os_tools`）与 sandbox 概念，档位枚举与参考的 `read-only` / `workspace-write` / `danger-full-access` **不是同一套命名**，chip 应复用本项目既有枚举，不引入参考的字符串。
- 「发送前重新应用」与本项目的会话级装配是否冲突，需要先确认装配的持久化边界（模式级默认 vs 会话级覆盖），否则会出现 chip 覆盖用户刚在工具面板里改过的选择。

---

## 5 条件性与不适用

### 5.1 条件性：截图排除 + HID 点击穿透 guard

参考的 `applyFloatingOverlayGuard` 是 capture / input 两个独立计数：

- **capture 计数**：Windows 用 `setContentProtection(true)`（`WDA_EXCLUDEFROMCAPTURE`），macOS 用 ScreenCaptureKit 的窗口 id 排除。macOS 只排除**可见**窗口——隐藏窗口既不可排除，缺失 id 又会让捕获失败。
- **input 计数**：整窗 `setIgnoreMouseEvents(true)` + `blur()`，且**不转发**鼠标事件；Windows 在 HID 期间同时保持 content protection，因为动作后的复拍不会再发一次 capture begin。
- **只在计数跨零时切换**，嵌套的 capture IPC 不会让 overlay 闪烁。
- 输入开始后固定等 80ms 再 ack，等 WindowServer 命中测试落定。

与 `./I35-screenshot-qa-spec.md` 的关系：I35 是「抓屏前隐藏 orb 宿主窗口」，底图与用户所见一致；参考是「orb 保持可见但从截图里被排除」。**两者不冲突但语义不同**：I35 要的是用户视角底图，参考要的是「agent 自己的截图里不出现自己的球」。

**价值判定：条件性。** 只有当「agent 在开发期驱动本应用做渲染面自测」（见 `docs/analysis/AGENT-OPERABLE-TESTING-ANALYSIS.md`）真正推进时，这套 guard 才不可替代——否则 agent 的每一次截图都会带上球与面板，每一次点击都会被球挡住。若不做渲染面自动化，这块无收益。

### 5.2 不适用：明确不要照搬

| 项 | 不照搬的理由 | 本项目已有替代 |
|---|---|---|
| 自定义球头像 | 参考是静态图片替换；本项目是 Rive 状态机驱动的 mascot | `src/mascot` + `expressionPolicy` |
| 球模型 / 后台模型双选择 | 前提是球锁死单一 preset、后台另起会话；本项目按模式与会话分层 | `builtin-*` 模式 + 会话级模型 |
| 绕球折行输入胶囊 | 本项目展开时球隐藏，输入区在面板内，无重叠 | `ChatLayout` composer |
| 独立 `floating.html` + iframe 复用 ChatView | 那是 Electron 壳的绕行方案；本项目单 webview 内直接复用 React 组件 | `ConversationView` |
| 拖拽位置不持久化 | 参考刻意选择重启回默认位；本项目记忆位置是更强的行为 | `OrbConf.position_x/y` |
| Electron 专属 API | `type: 'panel'`、`WeakMap` 挂窗口态、`setIgnoreMouseEvents` 无 Tauri 等价物 | 只能搬决策，不能搬实现 |
| 提问卡 / TCC 门 / 划词工具栏 / 千分比坐标 | 全部依附 Computer Use 链路，本项目无该链路 | 不在范围内 |

---

## 6 判定汇总

| 判定 | 覆盖项 |
|---|---|
| 有增量 | 边缘停靠竖条（最高）· 运行态门控 · 权限档位常驻可见 |
| 条件性 | 截图排除 + HID 穿透 guard（仅渲染面自动化成立时） |
| 已覆盖 | 展开方向 · 留白 · 拖拽与 clamp · 贴边吸附 · 悬停/固定/失焦 · 点击穿透 · 球上控件与停止 · 设置项 |
| 不适用 | 自定义头像 · 球/后台双模型 · 绕球折行输入 · iframe 复用 · 位置不持久化 · Electron 专属 API · Computer Use 附属物 |

**一句话结论**：参考项目的球本体对本项目**已经不再是一个整体模板**——几何、展开、拖拽、穿透、收起这五条主干已等价或更强，剩下的价值集中在「少遮挡」（停靠竖条）、「任务可见」（运行态门控）、「前提可见」（装配 chip）三个**行为层决策**，以及一个只在渲染面自动化方向成立时才有意义的 guard。照搬实现层（Electron API、绕球布局、头像、双模型）会造成降级。

---

## 7 核对方式

| 结论 | 核对点 |
|---|---|
| 展开方向 / 留白 / 收起 | `src/orb/OrbExpand.ts`（`EXPAND_DIRECTION_HYSTERESIS_PX`、`ORB_BLUR_COLLAPSE_DELAY_MS`）、`src-tauri/src/window.rs` `expand_orb_panel` / `collapse_orb_panel` |
| 拖拽 / 吸附 / 位置持久化 | `src/orb/OrbDrag.tsx`、`src-tauri/src/cmd/orb.rs`（`snap_orb_to_edge`、`save_orb_position_sync`）、`assistant-config::constant`（`ORB_SNAP_BORDER_X = 24`、`ORB_SNAP_ANIMATION_MS = 250`、`WINDOW_EDGE_BUFFER = 4`） |
| 运行态无门控 | `src/stores/orbStore.ts` 字段清单（无流式/审批字段）；`src/orb/OrbPanel.tsx` 的 `ORB_BLUR` 收起分支只判断 `keepExpanded` / `isOrbBlurCollapseSuppressed` / `isDragging` / 指针是否在窗口内；Esc 分支只判断 `isDragging` |
| 收起后重开穿透 | `src/orb/OrbExpand.ts` `collapseOrbPanel` 内 `clickThrough` 分支 |
| 装配能力存在但不可见 | `src/chat/tool/ToolConfigPanel.tsx`、`src/chat/ConversationView.tsx`（仅 `toolPanelOpen` 时渲染） |
| 常驻层级缺失 | `src-tauri/tauri.conf.json` 的 `orb` 窗口仅 `alwaysOnTop: true`，无跨 Space / 全屏设置 |
| 参考停靠参数 | `references/1-ai-agent/deepseek-harness-orb/apps/desktop/src/floating-window.ts` 的 `FLOATING_DOCK_*` 常量组 |
| 参考门控语义 | 同文件 `moveFloatingBall(..., canDock)` 与 `clampFloatingWindow(..., canDock)`；`apps/desktop/renderer/floating.js` 的 `canDock = !(running || asking() || gatingTcc())` |
| 参考 guard 语义 | `floating-window.ts` 的 `applyFloatingOverlayGuard` / `syncOverlayGuard` / `OVERLAY_GUARD_INPUT_APPLY_MS` |
| 参考 Access chip | `apps/desktop/src/orb-permission.ts` + `apps/desktop/renderer/floating.html` 的 `#permission` 节点 |
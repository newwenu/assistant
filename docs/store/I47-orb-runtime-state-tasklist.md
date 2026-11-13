# I47 — Orb 运行态与可见性 任务清单

> 规格：[I47-orb-runtime-state-spec.md](./I47-orb-runtime-state-spec.md)
> 前置：无。四个阶段互不前置，可任意顺序独立合入。

四个批次对应规格 §1：C1 运行态门控（前端）→ C2 档位常驻可见（前端 + i18n）→ C3 窗口属性（后端）→ C4 冷启动锚点（后端 crate）。

> 本清单不管理提交。提交粒度按代码依赖与可编译性现场决定；这里只回答「代码改没改、验收跑没跑」。

---

## 阶段 C1 — 运行态门控

### C1.1 新增 `orbBusy.ts`（纯函数 + 取值器）

对应规格 §4.1 / §4.2 / 断言 F1。

- [x] 新建 `src/orb/orbBusy.ts`，只导出两个符号：`orbBusyFrom(isStreaming, messages)` 与 `isOrbBusy()`
- [x] `orbBusyFrom` 判定：`isStreaming || hasPendingToolCalls(messages.flatMap(m => m.toolCalls ?? []))`，复用 `src/chat/core/toolCallView.ts` 的 `hasPendingToolCalls`，不另写状态判定
- [x] `isOrbBusy` 从 `useChatStreamStore.getState().isStreaming` 与 `useChatStore.getState().messages` 取值后转调纯函数；只在事件回调中调用，不参与渲染
- [x] 不新增 store 字段、不改 `orbStore`、不改 DTO

**依赖**：无

---

### C1.2 自动收起入口

对应规格 §4.1 / §4.3 / 断言 F2–F5。

- [x] `src/orb/OrbExpand.ts` 新增模块级 `autoCollapseOrb()`：`isOrbBusy()` 为真时直接返回（不改 store、不调任何 `orbService`），否则转调 `collapseOrbPanel()`
- [x] `useOrbExpand` 返回值新增 `autoCollapse`（转调 `autoCollapseOrb`），`collapse` 保持无参数强收起，语义不变
- [x] `waitForPanelTransition` 增加 `typeof document === "undefined"` 早退（返回已 resolve 的 Promise），使非浏览器测试环境不抛错
- [x] `src/orb/OrbPanel.tsx` 失焦回调的两处 `void collapse()` 改调 `void autoCollapse()`（`!isHoverCapable()` 分支与坐标判定后的兜底），并把 `autoCollapse` 加入 `useEffect` 依赖数组
- [x] Esc、顶栏收起按钮、历史菜单、`collapseOrbOnInit` 保持走 `collapse` / `collapseOrbPanel`（显式路径不受门控）

**依赖**：C1.1

---

### C1.3 前端断言

对应规格 §4.6 / 验收 AC1、AC2。

- [x] 新建 `src/orb/__tests__/orbBusy.test.ts`：F1 真值表（`isStreaming`、`running`、`awaiting_user`、全空闲四种）
- [x] 新建 `src/orb/__tests__/orbAutoCollapse.test.ts`：`vi.mock` 替换 `orbService` 与 `@/settings/core/useConfGroup`（`getConfValue` 返回 `{ clickThrough: true }`）
- [x] F2：busy 时 `autoCollapseOrb` 不收起、不调用 `collapseOrbPanel` / `setOrbClickThrough`
- [x] F3：非 busy 时正常收起并恢复穿透
- [x] F4：busy 时显式 `collapseOrbPanel` 正常收起
- [x] F5：非 busy 且已收起时 `autoCollapseOrb` 不调用任何 `orbService` 方法

**依赖**：C1.1、C1.2

---

## 阶段 C2 — 档位常驻可见

### C2.1 概览条展示档位

对应规格 §5.1 / §5.4 / 验收 AC3。

- [x] `src/chat/tool/ToolToolbar.tsx`：chip 内改为「档位短标签 · 摘要」两段，档位在前
- [x] 档位段独立 `<span>`（`flex: 0 0 auto`，不进省略容器），摘要段沿用 `css.label`；点击行为仍是 `onToggle`，不新增第二个点击目标
- [x] `src/chat/tool/ToolToolbar.module.css`：新增档位段与分隔符两条规则，不新增颜色语义
- [x] `src/chat/tool/ToolToolbar.tsx` 的 tooltip 改为 `t("chat.tool.toolbar.hint", { level: t(\`chat.tool.approval.level.${config.approvalLevel}\`), hint: t(\`chat.tool.approval.level_hint.${config.approvalLevel}\`) })`

**依赖**：无

---

### C2.2 i18n 键位与档位下拉

对应规格 §5.2 / §5.5。

- [x] `src/i18n/locales/zh-CN.json`：`chat.tool.approval.level.*` 改为档位名（直接运行 / 默认 / 手动审批）；新增 `chat.tool.approval.level_hint.*`（说明：所有工具直接执行，不弹审批卡片 / 读取自动放行，修改和执行需确认 / 所有工具都需确认，包括读取）；`chat.tool.toolbar.hint` 增加 `{{level}}`、`{{hint}}` 两个插值；`settings.agent_mode.field.approval_level.options.*` 档位名同步同字面
- [x] `src/i18n/locales/en-US.json`：同上（Run directly / Default / Manual approval 及对应说明）
- [x] `src/chat/tool/ToolConfigPanel.tsx`：档位 `Select` 加 `popupMatchSelectWidth={false}`、`virtual={false}`，用 `optionRender` 渲染「档位名 + 说明」两行，收起态仍只显示档位名
- [x] `src/chat/tool/ToolConfigPanel.module.css`：新增选项行两行布局与说明行样式
- [x] `src/chat/tool/ApprovalCard.tsx`：`reason.approval_level` 的 `level` 插值改取 `chat.tool.approval.level_hint.*`（档位名单独出现时不解释「为什么问我」）

**依赖**：C2.1（键位与消费点一起落地，否则 `hint` 插值取不到值）

---

## 阶段 C3 — 窗口属性

### C3.1 统一属性应用函数

对应规格 §6.1 / §6.2 / 验收 AC5、AC6。

- [x] `src-tauri/src/window.rs` 新增 `pub fn apply_orb_window_attrs(app: &AppHandle)`：读 `OrbConf`，对 orb 窗口同时应用 `set_always_on_top` 与 `set_visible_on_all_workspaces`（两者都取 `conf.always_on_top`）
- [x] 两处失败只 `log::warn!`，不返回错误、不阻断启动
- [x] 不加 `#[cfg(target_os)]` 分支：tao 0.37.2 在 Windows 上为空实现且不报错
- [x] `window.rs` 导入补 `assistant_config::orb_conf::OrbConf`

**依赖**：无

---

### C3.2 两处调用点

对应规格 §6.1 / §6.3。

- [x] `src-tauri/src/setup.rs`：在 `position_orb_window(handle)` 之后、`show_orb_window(handle)` **之前**调用一次 `crate::window::apply_orb_window_attrs(handle)`
- [x] `src-tauri/src/cmd/config.rs`：`ORB_SIDE_EFFECTS` 的 `always_on_top` 分支改为调用同一函数（删除内联 `set_always_on_top`）

**依赖**：C3.1

---

## 阶段 C4 — 冷启动锚点

### C4.1 可用区与锚点纯函数

对应规格 §7.2 / 断言 B1、B2。

- [x] `src-tauri/crates/assistant-orb/src/placement.rs` 新增 `anchor_bottom_right(area: &ScreenBounds, size: u32, margin: i32) -> (i32, i32)`
- [x] 新增 `get_primary_work_area(app: &AppHandle) -> Option<ScreenBounds>`，由 `primary_monitor()` 的 `work_area()`（`PhysicalRect` 的 `position` / `size`）构造
- [x] `get_available_screens` 保持不变（吸附与纠正仍按整屏 bounds，规格 §7.4）

**依赖**：无

---

### C4.2 冷启动改用可用区

对应规格 §7.2 / 验收 AC7。

- [x] `src-tauri/src/setup.rs::position_orb_window` 无保存位置分支改为 `get_primary_work_area` + `anchor_bottom_right`，`edge_margin` 仍为 20
- [x] 取不到可用区时回退现有的 `screens.first()` 逻辑（不引入启动失败面）

**依赖**：C4.1

---

### C4.3 后端断言

对应规格 §7.3 / 验收 AC8。

- [x] `src-tauri/crates/assistant-orb/tests/placement.rs` 新增 B1（常规可用区锚点落在右下角内缩 20px）
- [x] 新增 B2（可用区高度小于整屏时，锚点 `y + size` 不超过可用区底边）
- [x] 既有 placement 用例保持全绿（回归）

**依赖**：C4.1

---

## 门禁

- [x] `npm run lint:fix`
- [x] `npm run typecheck`
- [x] `npm run test`
- [x] `cargo test -p assistant-orb --test placement`（cwd `src-tauri`）
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo fmt --all --check`
- [x] `npm run contract:check`（本规格不改 DTO，应为幂等）

---

## 执行记录

门禁（2026-11-13）：

- `npm run lint:fix`：通过，无剩余违规
- `npm run typecheck`：通过，`tsc --noEmit` 无报错
- `npm run test`：48 个测试文件全绿，565 passed / 1 skipped
- `cargo test -p assistant-orb --test placement`：12 passed，含 B1 `anchor_bottom_right_insets_margin`、B2 `anchor_bottom_right_respects_work_area_height`
- `cargo clippy --workspace --all-targets -- -D warnings`：通过；输出中的 `error finalizing incremental compilation session directory` 为 target 目录锁告警，非代码 lint
- `cargo fmt --all --check`：clean
- `npm run contract:check`：通过，`protocol.contract.ts` / `vault.contract.ts` 等生成文件无差异（幂等）

人工验收 AC1–AC8：待在运行中的应用内确认（busy 时失焦不自动收起、档位 chip 与下拉两行文案、弹层不受面板宽度限制、窗口置顶与跨空间属性、冷启动右下角可用区锚点）。

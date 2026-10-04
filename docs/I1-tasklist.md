# I1 — Orb 悬浮球 任务清单

> 规格：[I1-spec.md](./I1-spec.md)
> 前置：I0 ✅ 已完成

---

## 阶段 A — Rust 后端：Commands + 快捷键 + 位置约束

### A1. 新增 Tauri Commands

**文件**：`src-tauri/src/cmd.rs`

- [x] `toggle_orb_visibility(app: AppHandle) → Result<()>`
  - 获取 Orb 窗口 → 若可见且聚焦 → hide；若隐藏或失焦 → show + set_focus
  - 参考：WindowPet `utils.rs` → `reopen_main_window` 的存在性检查 + focus 逻辑
- [x] `set_orb_click_through(app: AppHandle, enabled: bool) → Result<()>`
  - 获取 Orb 窗口 → `set_ignore_cursor_events(enabled)`
  - 错误处理：窗口不存在时返回 Err
- [x] `snap_orb_to_screen(app: AppHandle) → Result<()>`
  - 获取 Orb 窗口当前位置 + 尺寸 → `get_available_screens()` → `snap_to_nearest_screen()` → `set_position()`
  - 参考：BongoCat `placement.rs` → `correction_for_screens()`
- [x] `app_quit(app: AppHandle) → Result<()>`
  - 调用 `request_exit()` + 销毁 Orb/Main 窗口 + `app.exit(0)`
  - 与 tray.rs 退出路径一致
- [x] 在 `lib.rs` 的 `invoke_handler` 中注册新 commands

**依赖**：A2（snap_orb_to_screen 需要 get_available_screens）

---

### A2. assistant-orb crate 增强：placement 屏幕枚举

**文件**：`src-tauri/crates/assistant-orb/src/placement.rs`

- [x] `get_available_screens(app: &AppHandle) → Vec<ScreenBounds>`
  - 使用 Tauri 2 `app.available_windows()` + `window.current_monitor()?` 获取显示器几何
  - 或使用 `tauri::Manager` 的 `primary_monitor()` / `available_monitors()` API
  - 转换为 `ScreenBounds { x, y, width, height }`
  - 参考：BongoCat `placement.rs` 用平台 API 枚举显示器
- [x] `check_and_snap_orb(app: &AppHandle)`
  - 获取 Orb 窗口 bounds → `bounds_inside_screens()` → 若不在屏幕内 → `snap_to_nearest_screen()` → `set_position()`
  - 此函数由前端拖拽结束后通过 `snap_orb_to_screen` command 调用

**文件**：`src-tauri/crates/assistant-orb/src/lib.rs`

- [x] 导出新增公共函数

**依赖**：无（placement.rs 已有基础结构）

---

### A3. assistant-orb crate 增强：hover 穿透判断

**文件**：`src-tauri/crates/assistant-orb/src/hover.rs`

- [x] `should_toggle_click_through(state: HoverState, prev_state: HoverState) → Option<bool>`
  - 状态从 Visible → FadingOut → 返回 `Some(true)`（开启穿透）
  - 状态从 Hidden → FadingIn → 返回 `Some(false)`（关闭穿透）
  - 其他 → `None`（无需切换）
  - 参考：BongoCat `hover.rs` → `PointerHoverHide::observe()` 返回 alpha + `hidden()` 返回穿透状态

**依赖**：无（hover.rs 已有 HoverState 状态机）

---

### A4. 全局快捷键注册

**文件**：`src-tauri/Cargo.toml`

- [x] 添加 `tauri-plugin-global-shortcut` 依赖到 `[workspace.dependencies]` 和 `[dependencies]`

**文件**：`src-tauri/src/lib.rs`

- [x] `.plugin(tauri_plugin_global_shortcut::Builder::new().build())`
- [x] 在 `setup::init` 中注册快捷键：
  ```rust
  use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
  let shortcut = Shortcut::new(Some(Modifiers::SUPER), Code::KeyK);
  app.global_shortcut().on_shortcut(shortcut, |app, _shortcut, _event| {
      // 调用 toggle_orb_visibility 逻辑
  })?;
  ```
- [x] 注册 `CmdOrCtrl+K`：macOS 用 SUPER(Meta)，Windows/Linux 用 CONTROL
  - 实际使用 `Modifiers::SUPER` 即可，Tauri 自动映射为 Cmd(macOS) / Ctrl(Windows/Linux)

**文件**：`src-tauri/capabilities/default.json`

- [x] 添加 `"global-shortcut:default"` 权限

**依赖**：A1（toggle_orb_visibility command）

---

### A5. 窗口焦点事件监听

**文件**：`src-tauri/src/setup.rs`

- [x] 在 `init` 中监听窗口焦点变化事件：
  ```rust
  app.on_window_event(|window, event| {
      match event {
          tauri::WindowEvent::Focused(false) => {
              if window.label() == WINDOW_ORB {
                  let _ = window.emit("orb-blur", ());
              }
          }
          _ => {}
      }
  });
  ```
- [x] Orb 失焦时 emit `orb-blur` 事件到前端，由前端决定是否收起

**依赖**：无

---

### A6. 常量对齐

**文件**：`src-tauri/crates/assistant-config/src/constant.rs`

- [x] 新增：
  ```rust
  pub const ORB_HITBOX_SIZE: u32 = 48;
  pub const ORB_BLUR_COLLAPSE_DELAY_MS: u64 = 200;
  pub const ORB_SNAP_DELAY_MS: u64 = 1000;
  pub const HOVER_POLL_INTERVAL_MS: u64 = 120;
  pub const DRAG_POLL_INTERVAL_MS: u64 = 16;
  ```

**依赖**：无

---

## 阶段 B — 前端：组件拆分 + Store + 交互逻辑

### B1. orbStore 状态管理

**文件**：`src/stores/orbStore.ts`（新建）

- [x] Zustand store 定义：
  ```typescript
  interface OrbState {
    isExpanded: boolean;
    isHovering: boolean;
    isDragging: boolean;
    isVisible: boolean;
    position: { x: number; y: number } | null;
    expand: () => Promise<void>;
    collapse: () => Promise<void>;
    toggle: () => Promise<void>;
    setHovering: (v: boolean) => void;
    setDragging: (v: boolean) => void;
    setVisible: (v: boolean) => void;
    setPosition: (pos: { x: number; y: number }) => void;
  }
  ```
- [x] `expand()`：调用 `invoke("expand_orb_panel")` + `set({ isExpanded: true })`
- [x] `collapse()`：调用 `invoke("collapse_orb_panel")` + `set({ isExpanded: false })`
- [x] `toggle()`：调用 `invoke("toggle_orb_visibility")` + 切换 `isVisible`

**依赖**：无

---

### B2. 常量对齐

**文件**：`src/config/constants.ts`

- [x] 新增与 Rust constant.rs 对齐的常量：
  ```typescript
  export const ORB_HITBOX_SIZE = 48;
  export const ORB_BLUR_COLLAPSE_DELAY_MS = 200;
  export const ORB_SNAP_DELAY_MS = 1000;
  export const HOVER_POLL_INTERVAL_MS = 120;
  export const DRAG_POLL_INTERVAL_MS = 16;
  ```

**依赖**：无

---

### B3. OrbDrag 拖拽逻辑组件

**文件**：`src/orb/OrbDrag.tsx`（新建）

- [x] 从 App.tsx `OrbPlaceholder` 中提取拖拽逻辑
- [x] 导出 `useOrbDrag` hook：
  ```typescript
  interface UseOrbDragReturn {
    onPointerDown: (e: React.PointerEvent) => void;
    onPointerMove: (e: React.PointerEvent) => void;
    onPointerUp: (e: React.PointerEvent) => void;
    isDragging: boolean;
  }
  ```
- [x] 拖拽阈值判断：`dx*dx + dy*dy < DRAG_THRESHOLD_PX²` → 点击，否则拖拽
- [x] Windows 拖拽：`cursorPosition()` 轮询 + `setPosition()` IPC
  - 参考：OpenBitFun `AgentCompanionDragService.ts`
  - pointerdown 记录 grab → pointermove 超阈值 → 启动 tick 循环（16ms 间隔）
  - tick 内：读 `cursorPosition()` → 计算 `origin + pointer - grab` → `setPosition()`
  - pointerup → 停止 tick → 延迟 ORB_SNAP_DELAY_MS → `invoke("snap_orb_to_screen")`
- [x] macOS 拖拽：`orbWindow.startDragging()`
  - pointermove 超阈值 → 调用 `startDragging()`
  - pointerup 后延迟 `snap_orb_to_screen`
- [x] 点击（未超阈值）→ 调用 `orbStore.expand()`

**依赖**：B1（orbStore）

---

### B4. OrbBall 悬浮球主体

**文件**：`src/orb/OrbBall.tsx`（新建）

- [x] 收起态悬浮球 UI：
  - 外层容器：64×64，flex 居中
  - hitbox 圆形：48×48，`borderRadius: 50%`，`background: var(--color-primary)`
  - opacity：拖拽 0.7 / 悬停 1.0 / 默认 0.85
  - `transition: opacity 150ms ease-in-out`
- [x] 事件绑定：`useOrbDrag` 的 `onPointerDown/Move/Up`
- [x] hover 检测：
  - Windows：`onPointerEnter/Leave` → `orbStore.setHovering()`
  - macOS：`setInterval(HOVER_POLL_INTERVAL_MS)` 轮询 `cursorPosition()` + 窗口位置 → 判断是否在 hitbox 内
- [x] 穿透切换：
  - hover 变化时调用 `invoke("set_orb_click_through", { enabled: !isHovering })`
  - 收起态：不在 hitbox → 穿透开启；在 hitbox → 穿透关闭
  - 展开态：穿透始终关闭

**依赖**：B1（orbStore）、B3（OrbDrag）

---

### B5. OrbPanel 展开面板

**文件**：`src/orb/OrbPanel.tsx`（新建）

- [x] 展开态对话面板 UI（I1 为占位，I2 填充）：
  - 顶部：Mascot 占位区域（预留，I5 实现）
  - 中部：对话消息区占位（"对话功能即将上线" 提示文字）
  - 底部：输入区占位（文本输入框 + 发送按钮，disabled 状态）
- [x] 失焦收起逻辑：
  - 监听 `listen("orb-blur")` 事件
  - 收到事件 → 延迟 `ORB_BLUR_COLLAPSE_DELAY_MS` → 若仍在展开态且非拖拽中 → `orbStore.collapse()`
- [x] Escape 键收起：
  - `keydown` 监听 Escape → `orbStore.collapse()`

**依赖**：B1（orbStore）

---

### B6. OrbContextMenu 右键菜单

**文件**：`src/orb/OrbContextMenu.tsx`（新建）

- [x] Ant Design `Dropdown` 组件：
  - 菜单项：新建对话（disabled，I2 启用）、打开主窗口、退出
  - "打开主窗口" → `invoke("open_main_window")`
  - "退出" → `invoke("app_quit")`（已注册 command，与 tray 退出路径一致）
- [x] 触发方式：`onContextMenu` 事件绑定到 OrbBall / OrbPanel
- [x] 样式：跟随主题，与 Ant Design 设计语言一致

**依赖**：无

---

### B7. OrbExpand 展开/收起控制

**文件**：`src/orb/OrbExpand.ts`（新建）

- [x] 导出 `useOrbExpand` hook，封装展开/收起逻辑：
  ```typescript
  interface UseOrbExpandReturn {
    isExpanded: boolean;
    expand: () => Promise<void>;
    collapse: () => Promise<void>;
  }
  ```
- [x] `expand()`：
  1. 调用 `orbStore.expand()`
  2. 调用 `invoke("set_orb_click_through", { enabled: false })`（展开态不穿透）
- [x] `collapse()`：
  1. 调用 `orbStore.collapse()`
  2. 恢复穿透状态（由 OrbBall 的 hover 逻辑自动处理）
- [x] 监听 `orb-collapse` 事件（Rust 端或其他窗口触发）

**依赖**：B1（orbStore）

---

### B8. App.tsx 重构：OrbPlaceholder → OrbBall + OrbPanel

**文件**：`src/App.tsx`

- [x] 删除 `OrbPlaceholder` 组件（约 170 行拖拽/hover/展开逻辑）
- [x] 新增 Orb 窗口组件：
  ```tsx
  const OrbWindow: React.FC = () => {
    const isExpanded = useOrbStore(s => s.isExpanded);
    return isExpanded ? <OrbPanel /> : <OrbBall />;
  };
  ```
- [x] `viewMap[WINDOW_ORB]` 映射到 `OrbWindow`
- [x] 保留 `MainLayout`、`SettingsWindow` 不变

**依赖**：B4（OrbBall）、B5（OrbPanel）、B6（OrbContextMenu）、B7（OrbExpand）

---

## 阶段 C — 集成 + 验收

### C1. capabilities 权限补全

**文件**：`src-tauri/capabilities/default.json`

- [x] 确认已有权限：`core:window:allow-set-ignore-cursor-events`、`core:window:allow-set-position`、`core:window:allow-show`、`core:window:allow-hide`
- [x] 新增（如需）：`core:window:allow-outer-position`、`core:window:allow-scale-factor`
- [x] 新增：`global-shortcut:default`

**依赖**：A4

---

### C2. macOS private-api 确认

**文件**：`src-tauri/tauri.conf.json`

- [x] 确认 `app.macOSPrivateApi` 已设置（I0 已处理，验证即可）
- [x] 确认 Cargo.toml tauri features 含 `macos-private-api`（I0 已处理，验证即可）

**依赖**：无

---

### C3. 编译 + 运行验收

- [x] `cargo build` 通过（Rust 后端）
- [ ] `npm run tauri dev` 启动无报错
- [ ] 验收 9 项（见 I1-spec.md 第 6 节）：
  1. 桌面可见透明小球
  2. 鼠标悬停高亮，移开半透明
  3. 拖拽自由移动，松手贴边吸附
  4. 点击展开对话面板
  5. 面板失焦自动收起
  6. Cmd/Ctrl+K 切换显示/隐藏
  7. 右键菜单可用
  8. 穿透正常工作
  9. macOS / Windows 行为一致

**依赖**：所有 A、B 阶段任务

---

### C4. DEVLOG 记录

**文件**：`docs/DEVLOG.md`

- [ ] 新增 `## I1 — Orb 悬浮球` 章节
- [ ] 记录技术决策、踩坑、与参考项目的差异

**依赖**：C3

---

## 任务依赖图

```
A6 (常量) ──────────────────────────────────────┐
A2 (placement 增强) ──┐                        │
A3 (hover 增强) ──────┤                        │
A5 (焦点监听) ────────┤                        │
                       ├→ A1 (commands) ─→ A4 (快捷键)
                       │                        │
B1 (orbStore) ─────────┤                        │
B2 (常量对齐) ─────────┤                        │
                       ├→ B3 (OrbDrag)          │
                       ├→ B4 (OrbBall) ──┐      │
                       ├→ B5 (OrbPanel)  │      │
B6 (ContextMenu) ──────┤               │      │
                       ├→ B7 (OrbExpand)│      │
                       │               ├→ B8 (App 重构)
                       │               │
                       └───────────────┴→ C1 (权限) → C2 (macOS) → C3 (验收) → C4 (DEVLOG)
```

## 建议执行顺序

```
第 1 批（无依赖，可并行）：
  A2, A3, A5, A6, B1, B2, B6

第 2 批（依赖第 1 批）：
  A1, B3, B5, B7

第 3 批（依赖第 2 批）：
  A4, B4

第 4 批（依赖第 3 批）：
  B8

第 5 批（集成验收）：
  C1, C2

第 6 批：
  C3, C4
```

## 文件变更清单

| 操作 | 文件 | 说明 |
|------|------|------|
| 修改 | `src-tauri/src/cmd.rs` | +3 commands |
| 修改 | `src-tauri/src/lib.rs` | 注册 commands + 全局快捷键插件 |
| 修改 | `src-tauri/src/setup.rs` | 焦点事件监听 + 快捷键注册 |
| 修改 | `src-tauri/Cargo.toml` | +tauri-plugin-global-shortcut 依赖 |
| 修改 | `src-tauri/capabilities/default.json` | +global-shortcut 权限 |
| 修改 | `src-tauri/crates/assistant-config/src/constant.rs` | +6 常量 |
| 修改 | `src-tauri/crates/assistant-orb/src/placement.rs` | +get_available_screens, +check_and_snap_orb |
| 修改 | `src-tauri/crates/assistant-orb/src/hover.rs` | +should_toggle_click_through |
| 修改 | `src-tauri/crates/assistant-orb/src/lib.rs` | 导出新函数 |
| 修改 | `src/App.tsx` | OrbPlaceholder → OrbWindow |
| 修改 | `src/config/constants.ts` | +5 常量 |
| 新建 | `src/stores/orbStore.ts` | Orb 状态管理 |
| 新建 | `src/orb/OrbBall.tsx` | 悬浮球主体 |
| 新建 | `src/orb/OrbPanel.tsx` | 展开面板 |
| 新建 | `src/orb/OrbDrag.tsx` | 拖拽逻辑 |
| 新建 | `src/orb/OrbExpand.tsx` | 展开/收起控制 |
| 新建 | `src/orb/OrbContextMenu.tsx` | 右键菜单 |
| 修改 | `docs/DEVLOG.md` | I1 开发日志 |
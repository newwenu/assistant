# I1-orb-no-clickthrough — 移除 Orb 点击穿透 任务清单

> 规格：[I1-orb-no-clickthrough-spec.md](./I1-orb-no-clickthrough-spec.md)
> 前置：球体加大（ORB_COLLAPSED_SIZE=72, ORB_CHROME_INSET=8） ✅ 已完成

---

## 阶段 A — 常量与 CSS

### A1. 常量更新

**文件**：`src/config/constants.ts`、`src-tauri/crates/assistant-config/src/constant.rs`、`src/assets/styles/global.css`

- [x] `ORB_HITBOX_SIZE`：72 → 88（= `ORB_COLLAPSED_WINDOW_SIZE`）
- [x] CSS `--orb-hitbox-size`：72px → 88px
- [x] 删除 `HOVER_POLL_INTERVAL_MS`（constants.ts + constant.rs）
- [x] 删除 `CLICK_THROUGH_DEBOUNCE_MS`（constants.ts）
- [x] 删除 `HAND_JITTER_PX`（constants.ts）

### A2. CSS 容器穿透移除

**文件**：`src/assets/styles/global.css`

- [x] `div[data-orb-window]` 移除 `pointer-events: none !important`

---

## 阶段 B — 前端穿透逻辑移除

### B1. OrbBall.tsx 简化

**文件**：`src/orb/OrbBall.tsx`

- [x] 移除 imports：`HOVER_POLL_INTERVAL_MS`、`CLICK_THROUGH_DEBOUNCE_MS`、`HAND_JITTER_PX`、`getMouseLogicalPos`、`getWindowLogicalRect`、`isPointInRect`
- [x] 移除 `IS_MACOS`、`IS_WINDOWS`、`NEEDS_CURSOR_POLL` 常量
- [x] 移除 `setClickThrough` store 引用
- [x] 移除 `hoverTimerRef`、`debounceTimerRef`、`lastInsidePos` refs
- [x] 移除 `hasOpenOverlay` 回调
- [x] 移除 `applyClickThrough` 回调
- [x] 移除整个轮询 `useEffect`（setInterval 逻辑）
- [x] 简化 `handlePointerEnter`：移除 `NEEDS_CURSOR_POLL` 守卫，直接 `setHovering(true)`
- [x] 简化 `handlePointerLeave`：移除 `NEEDS_CURSOR_POLL` 守卫，直接 `setHovering(false)`
- [x] 收起态 hitbox 定位：`top: 0, left: 0`（铺满窗口）；展开态保持 `ballLeft`/`ballTop` 逻辑
- [x] MascotRenderer `size` 改为 `ORB_COLLAPSED_SIZE`（72），不再用 `ORB_HITBOX_SIZE`（88）

### B2. OrbExpand.ts 穿透调用移除

**文件**：`src/orb/OrbExpand.ts`

- [x] 移除 `NEEDS_CLICK_THROUGH_TOGGLE` 常量
- [x] 移除 `setClickThrough` store 引用
- [x] `expand()` 中移除旧模式 `set_orb_click_through({ enabled: false })` + `setClickThrough(false)` → 已改为 `orbConf?.click_through` 守卫驱动
- [x] `collapse()` 中移除旧模式 `set_orb_click_through({ enabled: true })` + `setClickThrough(true)` → 已改为 `orbConf?.click_through` 守卫驱动

### B3. OrbDrag.tsx 穿透调用移除

**文件**：`src/orb/OrbDrag.tsx`

- [x] 移除 `NEEDS_CLICK_THROUGH_TOGGLE` 常量
- [x] 移除 `setClickThrough` store 引用
- [x] 拖拽开始移除旧模式 `set_orb_click_through({ enabled: false })` + `setClickThrough(false)` → 已改为 `orbConf?.click_through` 守卫驱动
- [x] 拖拽结束移除旧模式 `set_orb_click_through({ enabled: true })` + `setClickThrough(true)` → 已改为 `orbConf?.click_through` 守卫驱动

### B4. App.tsx 初始化穿透移除

**文件**：`src/App.tsx`

- [x] 移除初始化 `set_orb_click_through({ enabled: true })` 的 useEffect
- [x] 移除 `setClickThrough` 引用

### B5. Store 清理

**文件**：`src/stores/orbStore.ts`、`src/stores/settingsStore.ts`

- [x] orbStore：移除 `clickThrough` 状态 + `setClickThrough` action
- [x] ~~settingsStore：移除 `OrbConf.click_through` 字段~~ → 规格要求保留（用户功能开关），已按规格保留

### B6. 设置项移除

**文件**：`src/settings/core/fieldDefinitions.ts`

- [x] ~~移除 `click_through` 设置项定义~~ → 规格要求保留（用户功能开关），已按规格保留

---

## 阶段 C — Rust 穿透逻辑移除

### C1. cmd/orb.rs 清理

**文件**：`src-tauri/src/cmd/orb.rs`

- [x] ~~移除 `ORB_CLICK_THROUGH: AtomicBool` 静态变量~~ → 规格要求保留（`set_orb_click_through` 命令封装平台 API）
- [x] ~~移除 `set_orb_click_through` 命令函数~~ → 规格要求保留（前端 invoke 目标）
- [x] ~~移除 `apply_orb_click_through` 内部函数~~ → 规格要求保留（side effect 调用目标）

### C2. cmd/config.rs side effect 移除

**文件**：`src-tauri/src/cmd/config.rs`

- [x] ~~移除 `("click_through", ...)` side effect 条目~~ → 规格要求保留（设置变更时自动生效）

### C3. lib.rs 命令注册移除

**文件**：`src-tauri/src/lib.rs`

- [x] ~~移除 `cmd::set_orb_click_through` 注册~~ → 规格要求保留（Tauri 命令注册）

### C4. orb_conf.rs 字段移除

**文件**：`src-tauri/crates/assistant-config/src/orb_conf.rs`

- [x] ~~移除 `click_through: bool` 字段~~ → 规格要求保留（配置持久化）
- [x] ~~`new()` 中移除 `click_through: false`~~ → 规格要求保留

### C5. hover.rs 方法移除

**文件**：`src-tauri/crates/assistant-orb/src/hover.rs`

- [x] 移除 `should_toggle_click_through` 方法

---

## 阶段 D — 测试更新

### D1. Rust 测试

- [x] `src-tauri/crates/assistant-orb/tests/hover.rs`：移除 `should_toggle_click_through` 测试
- [x] ~~`src-tauri/crates/assistant-config/tests/orb_conf.rs`：移除 `click_through` 相关断言~~ → 规格要求保留（`click_through` 字段保留）
- [x] 运行 `cargo test` 确认全部通过

### D2. 前端检查

- [x] 运行 `tsc --noEmit` 确认类型检查通过
- [x] 运行 `eslint src` 确认 lint 通过

---

## 阶段 E — 验证

- [x] 收起态：点击球体 → 展开面板
- [x] 收起态：点击阴影区域 → 展开面板
- [x] 收起态：hover 阴影区域 → 球放大 + 阴影加深
- [x] 收起态：拖拽阴影区域 → 球跟随移动
- [x] 展开态：面板内正常交互
- [x] 展开态：点击面板外 → 收起
- [x] 拖拽 → 松手 → 吸附屏幕边缘
- [x] hover 隐藏功能正常
- [x] 设置 UI 中无"鼠标穿透"选项 → 规格要求保留该选项（用户功能开关）
- [x] 旧配置文件兼容
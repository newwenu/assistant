# I6 — 设置系统优化 任务清单

> 规格：[I6-settings-optimize-spec.md](./I6-settings-optimize-spec.md)
> 前置：I4 ✅（设置系统已就绪）

---

## 阶段 A — Phase 1：原子写入（Rust 端）

- [x] A1. 新增 `assistant-config/src/atomic_write.rs`：`atomic_write(path, data)` + `recover_temp(path)` — 临时文件 + sync_all + rename + 崩溃恢复
- [x] A2. `app_conf.rs` `save()` 改用 `atomic_write`
- [x] A3. `orb_conf.rs` `save()` 改用 `atomic_write`
- [x] A4. `app_conf.rs` `load()` 增加崩溃恢复：检测 `.tmp` → 有效则提升 → 无效则删除
- [x] A5. `orb_conf.rs` `load()` 增加崩溃恢复（同 A4）
- [x] A6. `lib.rs` 新增 `pub mod atomic_write`

---

## 阶段 B — Phase 2：消除前端旁路

- [x] B1. `settingsStore.ts` 删除 `saveAppConf` / `saveOrbConf` 方法及对应接口声明
- [x] B2. `settingsStore.ts` `updateKeybinding` / `resetKeybindings` 删除额外的 `invoke("reload_shortcuts")` 调用（已移入 Rust 端）
- [x] B3. 查找所有调用 `saveAppConf` / `saveOrbConf` 的组件 → 无外部调用者

---

## 阶段 C — Phase 3：运行时功能归入 Rust 端（取代前端副作用）

- [x] C1. `cmd.rs` `set_app_conf` 接收 `AppHandle`，加载 prev 配置做 diff，变更字段触发对应运行时功能
- [x] C2. `set_app_conf` 中 `launch_at_login` 变更 → `apply_autostart()` 调用 `tauri_plugin_autostart`
- [x] C3. `set_app_conf` 中 `stay_on_top` 变更 → `window.set_always_on_top()`
- [x] C4. `set_app_conf` 中 `keybindings` 变更 → `register_global_shortcuts()`
- [x] C5. `cmd.rs` `set_orb_conf` 增加 diff + 运行时功能
- [x] C6. `set_orb_conf` 中 `always_on_top` 变更 → `window.set_always_on_top()`
- [x] C7. `set_orb_conf` 中 `click_through` 变更 → `apply_orb_click_through()`
- [x] C8. `settingsDispatcher.ts` 删除所有前端副作用代码，只保留 invoke + loadStore + emitEvent
- [x] C9. `useOrbConfSync.ts` 简化：删除 `setAlwaysOnTop` / `set_orb_click_through`，只保留 CSS 变量应用
- [x] C10. 删除 `sideEffects.ts` / `registerAppConfEffects.ts` / `registerOrbConfEffects.ts`
- [x] C11. `main.tsx` 删除副作用注册调用

---

## 阶段 D — Phase 4：配置校验（Rust 端）

- [x] D1. `orb_conf.rs` 新增 `validate()` 方法：校验 `scale_percent` 10-300、`opacity_percent` 10-100、`corner_radius_percent` 0-100、`hover_delay_ms` 0-5000
- [x] D2. `app_conf.rs` 新增 `validate()` 方法：校验 `theme` ∈ {system,light,dark}、`language` 非空
- [x] D3. `cmd.rs` `set_app_conf` 写入前调用 `validate()`
- [x] D4. `cmd.rs` `set_orb_conf` 写入前调用 `validate()`

---

## 阶段 E — 集成验证

### E1. 编译验证

- [x] `cargo check` 通过
- [x] `tsc --noEmit` 通过
- [x] `eslint --fix` 通过

**依赖**：A1~A6, B1~B3, C1~C11, D1~D4

---

### E2. 功能验证

- [ ] 修改 AppConf 后配置正确持久化到 config.json
- [ ] 修改 OrbConf 后配置正确持久化到 orb-config.json
- [ ] 原子写入：config.json 不存在半写状态
- [ ] 崩溃恢复：手动创建 .tmp 文件后 load() 能正确处理
- [ ] 写入路径统一：无 saveAppConf/saveOrbConf 旁路
- [ ] 运行时功能在 Rust 端执行：修改 launch_at_login 触发 autostart
- [ ] 运行时功能在 Rust 端执行：修改 stay_on_top 触发 setAlwaysOnTop
- [ ] 运行时功能在 Rust 端执行：修改 keybindings 触发 reload_shortcuts
- [ ] 运行时功能在 Rust 端执行：修改 always_on_top 触发窗口 API
- [ ] 运行时功能在 Rust 端执行：修改 click_through 触发 set_ignore_cursor_events
- [ ] useOrbConfSync 只做 CSS 变量应用
- [ ] 配置校验：非法 opacity_percent=0 被拒绝
- [ ] 配置校验：非法 scale_percent=0 被拒绝

**依赖**：E1
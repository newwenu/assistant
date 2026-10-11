# I8 — 声明式设置系统 任务清单

> 规格：[I8-declarative-settings-spec.md](./I8-declarative-settings-spec.md)
> 前置：I6 ✅（原子写入 + 前端旁路消除 + 副作用归入 Rust 端 + 校验）

---

## 阶段 A — Phase 1：Rust amend + patch_conf 命令

- [x] A1. `AppConf` 新增 `amend(patch: Value) -> Result<Self>` 方法
- [x] A2. `OrbConf` 新增 `amend` 方法
- [x] A3. `MascotConf` 新增 `amend` 方法
- [x] A4. `config.rs` 新增 `patch_conf(app, group, patch)` command，内部调用 amend + validate + save + apply_side_effects
- [x] A5. `config.rs` 副作用提取为 `SIDE_EFFECTS` 静态注册表 + `apply_side_effects` 函数
- [x] A6. `lib.rs` 注册 `patch_conf` 到 `invoke_handler`
- [x] A7. `patch_conf` 加 Mutex 保证并发安全
- [x] A8. 旧 `set_app_conf` / `set_orb_conf` / `set_mascot_conf` 已删除，所有调用方已迁移至 `dispatchConfPatch`

---

## 阶段 B — Phase 2：前端注册表 + definitions

- [x] B1. 新建 `src/settings/definitions.ts`：迁移所有现有设置项为 `defineSetting` 声明
- [x] B2. 新建 `src/settings/settingRegistry.ts`：`defineSetting` + `getDefinitionsByGroup` + `registerSettingType` + 内置类型渲染器（boolean/string/number/color/slider/select）
- [x] B3. 新建 `src/settings/sideEffectRegistry.ts`：`registerSideEffect` + 前端副作用注册
- [x] B4. `settingsDispatcher.ts` 新增 `dispatchConfPatch(group, patch)` 函数
- [x] B5. `events.ts` 新增 `ConfPatched` 事件枚举
- [x] B6. `settingEvents.ts` 新增 `ConfPatched` 事件监听

---

## 阶段 C — Phase 3：UI 自动渲染

- [x] C1. 新建 `src/settings/SettingField.tsx`：通用设置字段组件，根据 type 选择渲染器，支持 customRender 逃生舱口
- [x] C2. 新建 `src/settings/SettingsGroup.tsx`：设置分组组件，根据 definitions 自动渲染，支持 visible 联动
- [x] C3. `GeneralSettings.tsx` 改为使用 `SettingsGroup`
- [x] C4. `OrbBallSection.tsx` 改为使用 `SettingsGroup`（滑块防抖保留，乐观更新 + debounce）
- [x] C5. `OrbPanelSection.tsx` 改为使用 `SettingsGroup`
- [x] C6. `MascotColorSection.tsx` 保留自定义 UI（应用/重置按钮），改用 `dispatchConfPatch`（customRender 逃生舱口场景）
- [x] C7. 快捷键设置保留 `customRender` 逃生舱口

---

## 阶段 D — Phase 4：清理

- [x] D1. 删除旧 `set_app_conf` / `set_orb_conf` / `set_mascot_conf` 命令
- [x] D2. 删除 `settingsDispatcher.ts` 中旧 `dispatchAppConfChanged` / `dispatchOrbConfChanged` / `dispatchMascotConfChanged` 函数
- [x] D3. 更新 Rust 侧测试覆盖 `amend` + `patch_conf`
- [x] D4. 更新前端测试覆盖 `settingRegistry` + `definitions`

---

## 阶段 E — 验收

### E1. 编译验证

- [x] `cargo check` 通过
- [x] `tsc --noEmit` 通过
- [x] `eslint --fix` 通过

### E2. 功能验收

- [x] 所有现有设置项功能不变（开关/滑块/颜色/快捷键）
- [x] 副作用正常执行（开机自启/窗口置顶/鼠标穿透/快捷键注册）
- [x] 多窗口设置同步正常
- [x] 设置项 visible 联动正常（如禁用动画→隐藏动画速度）

### E3. 扩展性验收

- [x] 新增一个 bool 设置项只需修改 `definitions.ts` 一个文件
- [x] 新增一个设置类型只需 `registerSettingType` + `definitions.ts` 两个文件
- [x] 第三方可通过 `defineSetting` 注册设置项

**依赖**：A1~A8, B1~B6, C1~C7, D1~D4
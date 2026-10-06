# I4 — 设置系统 + 全局快捷键 + 主窗口 UI 改造 任务清单

> 规格：[I4-spec.md](./I4-spec.md)
> 前置：I2 ✅ + I3 ✅（AppConf/OrbConf CRUD + 主窗口路由已就绪）

---

## 阶段 A — Rust 后端：AppConf 扩展 + 快捷键注册 + Tauri Commands

- [ ] A1. AppConf 扩展：新增 `launch_at_login: bool` / `close_to_tray: bool` / `keybindings: HashMap<String, String>` 字段，`new()` 默认值，`load()` 兼容旧配置（缺失字段补默认值，`hotkey` 迁移到 `keybindings`）
- [ ] A2. `tauri-plugin-autostart` 依赖：Cargo.toml + tauri.conf.json plugins 注册 + `lib.rs` plugin 注册
- [ ] A3. 全局快捷键注册：`setup.rs` `register_global_shortcuts()` — 从 AppConf.keybindings 读取绑定 → `app.global_shortcut().on_shortcut()` 注册 toggle_orb / toggle_main / new_chat，注册失败 emit 警告事件
- [ ] A4. Tauri Commands 新增：`reload_shortcuts(app) → Result<()>` / `get_app_version() → String` / `set_launch_at_login(enable: bool) → Result<()>`，注册到 `lib.rs` invoke_handler
- [ ] A5. 托盘菜单增强：`tray.rs` 新增"新建对话"/"通用设置"/"外观设置"/"快捷键"菜单项 + 事件处理

---

## 阶段 B — 前端：Store + 类型 + 依赖

- [ ] B1. 前端依赖：`@tauri-apps/plugin-autostart`（开机自启前端 API）
- [ ] B2. settingsStore：Zustand store — `appConf` / `orbConf` / `loadAppConf` / `saveAppConf` / `loadOrbConf` / `saveOrbConf` / `updateKeybinding` / `resetKeybindings`，invoke 对应 Tauri Commands
- [ ] B3. 快捷键类型定义：`ShortcutDefinition { id, group, label, defaultBinding, currentBinding }` + `SHORTCUT_GROUPS` 常量 + `DEFAULT_KEYBINDINGS` 默认映射
- [ ] B4. AppConf / OrbConf TypeScript 类型定义（与 Rust 结构体对齐）

---

## 阶段 C — 前端：设置页面 UI

- [ ] C1. GeneralSettings：语言 Select + 开机自启 Switch + 关闭到托盘 Switch + 置顶主窗口 Switch，保存调用 `saveAppConf`
- [ ] C2. AppearanceSettings：主题 RadioGroup（system/light/dark）+ OrbAppearanceCard
- [ ] C3. OrbAppearanceCard：透明度 Slider(10-100) + 缩放 Slider(50-200) + 圆角 Slider(0-100) + 鼠标穿透 Switch + 悬停隐藏 Switch + 悬停延迟 Slider(0-2000ms)，保存调用 `saveOrbConf` + 实时更新 Orb 窗口
- [ ] C4. ShortcutSettings：分组展示（全局/主窗口）+ 搜索过滤 + 每行显示 ID/说明/当前绑定 + 点击进入录制 + 冲突检测高亮 + 重置按钮
- [ ] C5. ShortcutRecorder：录制模式组件 — 监听 keydown 捕获组合键 → 显示绑定 → Esc 取消 / Backspace 清除 / Enter 确认
- [ ] C6. AboutPage：应用名 + 版本号(invoke get_app_version) + 构建日期 + Rust 工具链 + 技术栈 + 许可证 + 检查更新按钮（预留）

---

## 阶段 D — 前端：路由 + 侧边栏改造 + 标题栏增强 + 集成

- [ ] D1. 路由扩展：App.tsx 新增 `/settings/general` / `/settings/appearance` / `/settings/shortcuts` / `/settings/about` 路由，`/settings` 重定向到 `/settings/general`
- [ ] D2. 侧边栏改造：替换 `buildToolCategoryItems()` + `Menu group` 模式，改为三区域布局
  - D2a. 工具箱区：`CollapsibleToolMenu` 组件（收藏置顶 + 分类折叠/展开 + ▶/▼ 箭头 + localStorage 持久化）
  - D2b. 导航区：`消息历史` 单独菜单项
  - D2c. 设置区：设置子菜单（通用/外观/快捷键/Provider/模板/关于）
  - D2d. 底部：版本号文字（invoke `get_app_version`）
- [ ] D3. 侧边栏选中态修复：`selectedKeys={[location.pathname]}`，进入 `/tools/:id` 时高亮当前工具项
- [ ] D4. 标题栏增强：新增 `PinToggle`（置顶切换）+ `SettingsEntry`（设置入口）按钮，插入 ThemeToggle 左侧
- [ ] D5. 删除 SettingsPage 空占位组件，`/settings` 路由改为 `<Navigate to="/settings/general" replace />`
- [ ] D6. 全局快捷键事件监听：前端 listen `shortcut-warning` 事件 → 显示 Notification 警告
- [ ] D7. Orb 外观实时更新：OrbAppearanceCard 保存后 invoke `set_orb_conf` + emit 事件通知 Orb 窗口重新应用配置

---

## 阶段 E — 集成验证

### E1. 编译验证

- [ ] `cargo check` 通过
- [ ] `tsc --noEmit` 通过
- [ ] `npm run lint` 0 errors

**依赖**：A1~A5, B1~B4, C1~C6, D1~D7

---

### E2. 功能验证

- [ ] 通用设置页可修改并保存语言/开机自启/关闭到托盘/置顶
- [ ] 外观设置页主题切换实时生效（system/light/dark）
- [ ] Orb 外观修改保存后 Orb 窗口实时更新（透明度/缩放/圆角）
- [ ] 快捷键页显示所有快捷键，默认绑定正确
- [ ] 快捷键录制：点击→按键→显示绑定→保存→重新注册
- [ ] 快捷键冲突检测：重复绑定时红色高亮警告
- [ ] 快捷键重置恢复默认
- [ ] 全局快捷键 toggle_orb 可切换悬浮球显示/隐藏
- [ ] 全局快捷键 toggle_main 可切换主窗口显示/隐藏
- [ ] 全局快捷键 new_chat 可新建对话
- [ ] 关于页显示版本号和构建信息
- [ ] 托盘菜单包含新建对话和设置入口
- [ ] `/settings` 重定向到 `/settings/general`
- [ ] 侧边栏工具分类可折叠/展开，折叠状态刷新后保留
- [ ] 侧边栏进入工具页后高亮当前工具项
- [ ] 侧边栏底部显示版本号
- [ ] 标题栏置顶切换按钮可切换主窗口置顶
- [ ] 标题栏设置入口按钮跳转 `/settings/general`

**依赖**：E1

---

### E3. 验收标准

- [ ] I4-spec §6 验收标准 1~17 项全部通过

**依赖**：E2
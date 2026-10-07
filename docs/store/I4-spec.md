# I4 — 设置系统 + 全局快捷键 + 主窗口 UI 改造 规格说明

## 1. 交付目标

完整可用的设置系统 + 全局快捷键注册与配置 + 主窗口 UI 改造（侧边栏重组/折叠菜单/标题栏增强/选中态修复）。

> **前置依赖**：I2（AppConf/OrbConf 已有 Rust CRUD）+ I3（主窗口路由与侧边栏已就绪）
> **当前缺口**：
> - SettingsPage 为空占位、AppConf.hotkey 未消费、setup.rs 全局快捷键被注释、OrbConf 无 UI、无关于页
> - 侧边栏工具分类不可折叠（I3 验收标准 §6.8 遗留）、选中态错误、结构混乱
> - 标题栏缺少置顶切换和设置入口

## 2. 功能规格

### 2.1 设置页面架构

参考 OpenBitFun `settingsRegistry.ts` 的 `definePage` 模式，每个设置页独立懒加载。

**设置页清单**：

| 页面 | 路由 | 说明 |
|------|------|------|
| 通用设置 | `/settings/general` | 启动行为 + 语言 + 窗口行为 |
| 外观设置 | `/settings/appearance` | 主题 + Orb 外观（透明度/缩放/圆角） |
| 快捷键 | `/settings/shortcuts` | 全局快捷键查看/录制/重置 |
| Provider 管理 | `/settings/providers` | 已有（I2），保持不变 |
| Prompt 模板 | `/settings/templates` | 已有（I2），保持不变 |
| 关于 | `/settings/about` | 版本 + 构建 + 许可证 + 检查更新 |

**侧边栏菜单更新**：

```
设置
  通用设置
  外观设置
  快捷键
  Provider 管理
  Prompt 模板
  关于
```

### 2.2 通用设置页

| 配置项 | 字段 | 类型 | 默认值 | 说明 |
|--------|------|------|--------|------|
| 开机自启 | `launch_at_login` | bool | false | 注册系统开机启动项 |
| 语言 | `language` | string | "zh-CN" | 当前仅 zh-CN，预留 i18n 扩展 |
| 关闭到托盘 | `close_to_tray` | bool | true | 关闭主窗口时最小化到托盘而非退出 |
| 置顶主窗口 | `stay_on_top` | bool | false | AppConf 已有字段，暴露到 UI |

### 2.3 外观设置页

| 配置项 | 字段 | 类型 | 默认值 | 说明 |
|--------|------|------|--------|------|
| 主题模式 | `theme` | "system"\|"light"\|"dark" | "system" | AppConf 已有，暴露到 UI |
| Orb 透明度 | `opacity_percent` | u8 (10-100) | 100 | OrbConf 已有 |
| Orb 缩放 | `scale_percent` | u16 (50-200) | 100 | OrbConf 已有 |
| Orb 圆角 | `corner_radius_percent` | u8 (0-100) | 50 | OrbConf 已有 |
| 鼠标穿透 | `click_through` | bool | true | OrbConf 已有 |
| 悬停隐藏 | `hide_on_pointer_hover` | bool | false | OrbConf 已有 |
| 悬停延迟 | `hover_delay_ms` | u64 (0-2000) | 0 | OrbConf 已有 |
| 始终置顶 | `always_on_top` | bool | true | OrbConf 已有 |

> 外观设置分两个 Card：**主题**（AppConf 字段）+ **Orb 外观**（OrbConf 字段），保存时分别调用 `set_app_conf` / `set_orb_conf`。

### 2.4 快捷键配置页

参考 OpenBitFun `KeyboardShortcutsTab.tsx`：分组展示 + 搜索 + 点击录制 + 冲突检测 + 重置。

**快捷键分两类，注册机制不同**：

| 类别 | 注册方式 | 生效范围 | 修改后生效方式 |
|------|---------|---------|--------------|
| **全局快捷键** | `tauri-plugin-global-shortcut` 系统级注册 | 任何时刻按都生效，即使窗口不可见 | 调用 `reload_shortcuts` command 重新注册 |
| **应用内快捷键** | 前端 DOM `keydown` 监听 | 仅对应窗口获得焦点时生效 | 组件自行从 `AppConf.keybindings` 读取并重新绑定 |

**快捷键清单**：

| ID | 分组 | 类别 | 默认绑定 | 说明 |
|----|------|------|---------|------|
| `toggle_orb` | 全局 | 全局快捷键 | macOS: `Meta+Shift+O` / 其他: `Control+Shift+O` | 显示/隐藏悬浮球 |
| `toggle_main` | 全局 | 全局快捷键 | macOS: `Meta+Shift+M` / 其他: `Control+Shift+M` | 显示/隐藏主窗口 |
| `new_chat` | 全局 | 全局快捷键 | macOS: `Meta+Shift+N` / 其他: `Control+Shift+N` | 新建对话 |
| `command_palette` | 主窗口 | 应用内快捷键 | macOS: `Meta+K` / 其他: `Control+K` | 命令面板（已有，I3） |

> **跨平台修饰键映射**（参照 BongoCat `ShortcutChord` + `ShortcutModifiers` 设计）：
>
> 配置层使用平台中立标识：`Meta` = 平台主修饰键（macOS ⌘Cmd / Windows ⊞Win），`Control` = Ctrl 键。两者不混淆。
>
> - **默认绑定**：macOS 用 `Meta`（⌘Cmd），Windows/Linux 用 `Control`（Ctrl），避免 Win 键与系统快捷键冲突
> - **Rust `parse_shortcut`**：`"meta"/"cmd"/"win"` → `Modifiers::META`；`"ctrl"/"control"` → `Modifiers::CONTROL`
> - **前端显示**：macOS 用符号（⌘⇧⌥⌃），非 macOS 原样显示 canonical 字符串（如 `Control + Shift + O`）
> - **前端录制**：macOS `metaKey` → 保存 `"Meta"`，Windows `ctrlKey` → 保存 `"Control"`
> - **配置迁移**：旧 `"Super"` 值在 `AppConf::load()` 时自动映射为 `"Meta"`

**快捷键录制**：

- 点击快捷键行 → 进入录制模式 → 按下组合键 → 显示绑定 → 保存
- `Esc` 取消录制，`Backspace` 清除绑定
- 冲突检测策略：
  - **同分组内重复绑定**：禁止保存，显示错误提示
  - **跨分组冲突**（如全局快捷键与应用内快捷键绑定相同）：允许保存但显示黄色警告，提示"此绑定与 [另一快捷键] 冲突，可能无法同时生效"
- 录制全局快捷键时，若按下的是已注册的全局快捷键组合，该事件会被系统拦截，录制组件无法捕获——这是已知限制，可在 UI 提示"录制时请避免使用已注册的全局快捷键"

**存储**：AppConf 新增 `keybindings: HashMap<String, String>` 字段，序列化为 JSON。

### 2.5 关于页

| 内容 | 来源 |
|------|------|
| 应用名称 | package.json `name` |
| 版本号 | Tauri `get_version()` |
| 构建日期 | 编译时注入 `env!("BUILD_DATE")` |
| Rust 工具链 | 编译时注入 `env!("RUSTC_VERSION")` |
| 前端技术栈 | 硬编码 "Tauri 2 + React 19 + Ant Design" |
| 许可证 | MIT |
| 检查更新 | Tauri Updater API（预留，I8 实现） |

### 2.6 托盘菜单增强

当前托盘菜单（tray.rs）：

```
打开主窗口
显示悬浮球
────────
退出
```

增强后：

```
新建对话
打开主窗口
显示悬浮球
────────
通用设置
外观设置
快捷键
────────
退出
```

### 2.7 AppConf 扩展

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConf {
    pub theme: String,
    pub language: String,
    pub hotkey: String,           // 废弃，迁移到 keybindings
    pub stay_on_top: bool,
    pub launch_at_login: bool,    // 新增
    pub close_to_tray: bool,      // 新增
    pub keybindings: HashMap<String, String>,  // 新增
}
```

> `hotkey` 字段保留向后兼容，首次加载时若 `keybindings` 为空且 `hotkey` 非空，自动迁移。

### 2.8 主窗口 UI 改造

#### 2.8.1 侧边栏重组

**当前问题**：工具箱分类/消息历史/设置混在一个 `Menu` 中，工具分类用 `type: "group"` 平铺不可折叠，进入工具页后侧边栏高亮"全部工具"而非当前工具。

**改造方案**：侧边栏分为三个区域，用 `type: "divider"` 分隔：

```
┌─────────────────┐
│  工具箱          │  ← 可折叠分类菜单
│    ★ 收藏        │     ▶/▼ 折叠箭头
│    ▼ 加密 (3)    │     localStorage 持久化
│      Hash 计算   │
│      UUID 生成   │
│      Base64      │
│    ▶ 转换器 (4)  │
│    ▶ 开发 (3)    │
│    ▶ 文本 (2)    │
├─────────────────┤
│  消息历史        │  ← 独立导航项
├─────────────────┤
│  设置            │  ← 设置子菜单
│    通用设置      │
│    外观设置      │
│    快捷键        │
│    Provider 管理 │
│    Prompt 模板   │
│    关于          │
├─────────────────┤
│  v0.x.x         │  ← 底部版本号
└─────────────────┘
```

**实现方式**：替换当前 `buildToolCategoryItems()` + `Menu group` 模式，改为：
- 工具箱区：使用 `CollapsibleToolMenu` 组件（已实现但未使用），收藏置顶 + 分类折叠/展开
- 导航区：`消息历史` 单独菜单项
- 设置区：设置子菜单（含所有设置页入口）
- 底部：版本号文字

#### 2.8.2 侧边栏选中态修复

**当前问题**：`selectedKeys={[isToolRoute ? "/" : location.pathname]}`——进入 `/tools/:id` 后高亮"全部工具"。

**修复**：`selectedKeys` 直接使用 `location.pathname`，当路由为 `/tools/:id` 时对应侧边栏工具项自然高亮。

#### 2.8.3 标题栏增强

**当前问题**：标题栏只有 `Assistant` + ThemeToggle + WindowControls，缺少置顶切换和设置入口。

**改造方案**：标题栏右侧新增两个按钮（参考 FRONTEND.md §3.2）：

```
[Assistant]                    [置顶] [设置] [主题] [─] [□] [×]
```

| 按钮 | 图标 | 行为 |
|------|------|------|
| 置顶切换 | `PushpinOutlined` / `PushpinFilled` | `invoke('set_app_conf', { ...conf, stay_on_top: !conf.stay_on_top })` + `window.setAlwaysOnTop(bool)` |
| 设置入口 | `SettingOutlined` | `navigate('/settings/general')` |

#### 2.8.4 SettingsPage 空占位删除

删除 `SettingsPage` 组件，`/settings` 路由改为 `Navigate` 重定向到 `/settings/general`。

## 3. Rust 后端规格

### 3.1 AppConf 字段扩展

新增 3 个字段：`launch_at_login` / `close_to_tray` / `keybindings`，`new()` 提供默认值，`load()` 兼容旧配置（缺失字段补默认值）。

### 3.2 全局快捷键注册

`setup.rs` 恢复并增强全局快捷键注册：

```rust
fn register_global_shortcuts(app: &AppHandle) {
    let conf = AppConf::load().unwrap_or_default();
    let bindings = &conf.keybindings;

    let toggle_orb_key = bindings.get("toggle_orb")
        .map(|s| s.as_str())
        .unwrap_or("Super+Shift+O");
    // ... parse and register via app.global_shortcut().on_shortcut(...)
}
```

- **仅注册全局快捷键**（`toggle_orb` / `toggle_main` / `new_chat`），`command_palette` 为应用内快捷键，由前端 `CommandPalette` 组件自行监听
- 启动时从 AppConf.keybindings 读取绑定并注册
- 注册失败（被占用）时 emit `shortcut-warning` 事件通知前端显示警告
- 前端修改全局快捷键后调用 `reload_shortcuts` command 重新注册
- 前端修改应用内快捷键后，对应组件需自行从 `AppConf.keybindings` 读取新绑定并重新绑定 DOM 事件

### 3.3 新增 Tauri Commands

| command | 签名 | 说明 |
|---------|------|------|
| `reload_shortcuts` | `(app: AppHandle) → Result<()>` | 重新注册全局快捷键 |
| `get_app_version` | `() → String` | 获取版本号 |
| `set_launch_at_login` | `(enable: bool) → Result<()>` | 设置开机自启 |

### 3.4 开机自启

使用 Tauri 2 `autostart` 插件（`tauri-plugin-autostart`）：

```toml
[dependencies]
tauri-plugin-autostart = "2"
```

### 3.5 关闭到托盘

`lib.rs` 的 `on_window_event(CloseRequested)` 需消费 `AppConf.close_to_tray` 字段：

- `close_to_tray = true`（默认）：拦截主窗口关闭事件，隐藏窗口而非退出
- `close_to_tray = false`：不拦截，允许窗口关闭触发正常退出流程

## 4. 前端规格

### 4.1 新增前端依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| `tauri-plugin-autostart` | ^2 | 开机自启（前端 API） |

### 4.2 新增组件

| 组件 | 路径 | 说明 |
|------|------|------|
| `GeneralSettings` | `src/settings/GeneralSettings.tsx` | 通用设置页 |
| `AppearanceSettings` | `src/settings/AppearanceSettings.tsx` | 外观设置页 |
| `ShortcutSettings` | `src/settings/ShortcutSettings.tsx` | 快捷键配置页 |
| `AboutPage` | `src/settings/AboutPage.tsx` | 关于页 |
| `ShortcutRecorder` | `src/settings/ShortcutRecorder.tsx` | 快捷键录制组件 |
| `OrbAppearanceCard` | `src/settings/OrbAppearanceCard.tsx` | Orb 外观配置卡片 |
| `PinToggle` | `src/components/PinToggle.tsx` | 标题栏置顶切换按钮 |
| `SettingsEntry` | `src/components/SettingsEntry.tsx` | 标题栏设置入口按钮 |

### 4.3 Store 扩展

`settingsStore.ts`（新建）：

```typescript
interface SettingsState {
  appConf: AppConf;
  orbConf: OrbConf;

  loadAppConf: () => Promise<void>;
  saveAppConf: (conf: AppConf) => Promise<void>;
  loadOrbConf: () => Promise<void>;
  saveOrbConf: (conf: OrbConf) => Promise<void>;
  updateKeybinding: (id: string, binding: string) => Promise<void>;
  resetKeybindings: () => Promise<void>;
}
```

### 4.4 路由扩展

App.tsx `MainLayout` 路由新增：

- `/settings/general` → `GeneralSettings`
- `/settings/appearance` → `AppearanceSettings`
- `/settings/shortcuts` → `ShortcutSettings`
- `/settings/about` → `AboutPage`

`/settings` 默认重定向到 `/settings/general`。

### 4.5 侧边栏改造

**替换当前 `Menu group` 模式**，改为三区域布局：

1. **工具箱区**：`CollapsibleToolMenu` 组件
   - 收藏工具置顶（★ 收藏 分类）
   - 各分类可折叠/展开（▶/▼ 箭头），折叠状态 localStorage 持久化
   - 当前工具高亮（`selectedKeys={[location.pathname]}`）
   - 折叠时仅显示分类图标

2. **导航区**：`消息历史` 单独菜单项

3. **设置区**：设置子菜单
   ```
   设置
     通用设置    → /settings/general
     外观设置    → /settings/appearance
     快捷键      → /settings/shortcuts
     Provider 管理
     Prompt 模板
     关于        → /settings/about
   ```

4. **底部**：版本号文字（从 Tauri `get_version()` 获取）

**删除**：`SettingsPage` 空占位组件，`/settings` 路由改为 `<Navigate to="/settings/general" replace />`

### 4.6 应用内快捷键绑定

`CommandPalette` 组件需从 `AppConf.keybindings.command_palette` 读取绑定，而非硬编码 `Cmd/Ctrl+K`：

- 组件挂载时从 `settingsStore.appConf.keybindings` 读取 `command_palette` 绑定
- 解析绑定字符串（如 `Super+K`）为修饰键+键码，用于 DOM `keydown` 监听
- 绑定变更时（store 更新）自动重新绑定监听器

### 4.7 设置配置中间层 — 统一保存→生效链路

当前设置保存→生效存在 3 套并行模式，需统一到 Provider 已验证的标准路径。

#### 标准路径（Provider 已验证）

```
UI → providerStore.saveProvider()
    → dispatchProviderSaved(provider)            // settingsDispatcher 统一入口
        → invoke("save_provider", {provider})    // 写后端
        → useProviderStore.loadProviders()       // 刷新本地缓存
        → emitSettingEvent(SettingEvent.ProviderChanged)  // 跨窗口广播
    消费端: settingEvents.ts listen(SettingEvent.ProviderChanged) → loadProviders()
```

#### 当前问题

| 模式 | 使用方 | 问题 |
|------|--------|------|
| ① settingsDispatcher 标准路径 | Provider / Theme | ✅ 规范 |
| ② 裸 invoke + 裸 emit | OrbConf | ❌ `OrbAppearanceSection` 用裸 `emit("orb-conf-changed")`，未走 `dispatchOrbConfChanged` |
| ③ 组件内联侧效应 | AppConf | ❌ `GeneralSettings` 直接 `invoke("set_app_conf")` + 内联 `autostart.enable()` / `setAlwaysOnTop()`，未走 `dispatchAppConfChanged` |

#### 改造目标

1. **AppConf 收敛**：`GeneralSettings` → `dispatchAppConfChanged(conf)`，侧效应（autostart / setAlwaysOnTop）提取到 dispatcher 内
2. **OrbConf 收敛**：`OrbAppearanceSection` → `dispatchOrbConfChanged(conf)`，`useOrbConfSync` 改用 `listen(SettingEvent.OrbConfChanged)`
3. **settingEvents 补齐**：新增 `AppConfChanged` / `OrbConfChanged` 监听，消费端处理运行时生效
4. **CSS 变量消费链补齐**：`global.css` 中 `.orb-ball` 等使用 `var(--orb-opacity)` / `var(--orb-scale)` / `var(--orb-radius)`
5. **OrbConf 字段消费补齐**：`opacity_percent` / `scale_percent` / `corner_radius_percent` / `click_through` / `hide_on_pointer_hover` / `hover_delay_ms` / `always_on_top` 全部在 `useOrbConfSync` 中生效

> `dispatchAppConfChanged` / `dispatchOrbConfChanged` / `SettingEvent.AppConfChanged` / `SettingEvent.OrbConfChanged` 已在 `settingsDispatcher.ts` + `events.ts` 中定义但未接入，仅需启用使用。

## 5. 不做的事（I4 范围外）

- ❌ i18n 多语言内容（仅预留 language 字段，实际翻译后续迭代）
- ❌ 自动更新下载安装（I8 打包发布）
- ❌ 数据导入/导出/重置（后续迭代）
- ❌ 高级窗口管理（多显示器记忆、工作区等）

## 6. 验收标准

1. 通用设置页可修改语言/开机自启/关闭到托盘/置顶，保存后重启生效
2. 外观设置页可切换主题，实时生效
3. 外观设置页可修改 Orb 透明度/缩放/圆角/穿透/悬停，保存后 Orb 窗口实时更新
4. 快捷键页显示所有快捷键分组，默认绑定正确
5. 快捷键可点击录制新绑定，冲突时显示警告
6. 快捷键重置按钮恢复默认绑定
7. 全局快捷键注册成功，按快捷键可切换 Orb/主窗口/新建对话
8. 快捷键注册失败时前端显示警告提示
9. 关于页显示版本号/构建日期/技术栈
10. 托盘菜单包含新建对话/设置入口
11. `/settings` 重定向到 `/settings/general`
12. 侧边栏工具分类可折叠/展开，折叠状态刷新后保留
13. 侧边栏进入工具页后高亮当前工具项（而非"全部工具"）
14. 侧边栏底部显示版本号
15. 标题栏显示置顶切换按钮，点击可切换主窗口置顶
16. 标题栏显示设置入口按钮，点击跳转 `/settings/general`
17. `cargo check` ✅ / `tsc --noEmit` ✅ / `lint 0 errors` ✅
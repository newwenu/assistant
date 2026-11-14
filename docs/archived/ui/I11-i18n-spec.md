# I11 — 国际化（i18n）规格说明

## 1. 交付目标

将全应用中英文硬编码文案替换为**前后端双轨 i18n 体系**，支持运行时语言切换，为多语言扩展奠定基础。首版交付 `zh-CN` + `en-US` 两种 locale。

> **前置依赖**：I8 ✅（声明式设置系统，`AppConf.language` 字段与 `settingsDispatcher` 统一链路已就绪）
> **参考**：BongoCat `bongocat-i18n` crate（`rust_i18n` + 嵌套 JSON + RFC 4647 回退 + 平台覆盖）、ADR-0028（locale 目录结构）

## 2. 当前问题

| # | 问题 | 影响 |
|---|------|------|
| P1 | 前端 58 个 `.tsx` 文件含中文硬编码，Rust 端 13 个 `.rs` 文件含中文硬编码 | 无法支持非中文用户 |
| P2 | `AppConf.language` 字段仅占位，选项仅 `zh-CN` 一项，切换无实际效果 | 语言设置是空壳 |
| P3 | Rust `assistant-i18n` crate 已有骨架但仅 12 个 key，未被任何消费端引用 | 后端文案无法国际化 |
| P4 | 前端无 i18n 库依赖，无 locale 资源文件，无翻译函数 | 每处文案都是一次性字符串 |
| P5 | Ant Design 组件库 locale、date-fns locale 未与语言设置联动 | 日期选择器、空态文案等仍为中文 |

## 3. 架构设计

### 3.1 双轨模型

前后端各自维护独立的 i18n 体系，通过 `AppConf.language` 同步：

```
┌──────────────────────────────────────────────────┐
│  前端 (React)                                    │
│  i18next + react-i18next                        │
│  useTranslation() → t("domain.key")             │
│  Ant Design ConfigProvider.locale               │
│  date-fns locale                                │
├──────────────────────────────────────────────────┤
│  同步层                                          │
│  AppConf.language ──→ i18next.changeLanguage()  │
│                   ──→ invoke("set_locale")       │
│                   ──→ ConfigProvider.locale      │
│                   ──→ date-fns locale            │
│  settingsDispatcher 统一链路（已就绪）            │
├──────────────────────────────────────────────────┤
│  Rust 后端                                       │
│  assistant-i18n crate（升级为 rust_i18n）        │
│  text(locale, "domain.key") → &str              │
│  tray / window / cmd 消费                       │
└──────────────────────────────────────────────────┘
```

### 3.2 与 I8 声明式设置系统的衔接

| 维度 | I8 设置系统 | I11 i18n |
|------|------------|---------|
| 语言字段 | `AppConf.language` 已存在 | 赋予实际语义，驱动全链路切换 |
| 设置项定义 | `defineSetting({ label, extra, ... })` | label/extra/saveMessage 改为 i18n key 引用 |
| 侧效应 | `sideEffectRegistry` | 新增 `locale_change` 侧效应，触发前端 i18n + Rust locale 同步 |
| 选项 | `options: [{ label, value }]` | label 走 i18n，value 保持 locale code |

### 3.3 Locale 目录结构（参考 BongoCat ADR-0028）

所有叶子 key 使用 `snake_case`，按顶层领域组织：

| 领域 | 覆盖范围 |
|------|---------|
| `navigation` | 设置窗口导航项、页面标题 |
| `settings` | 各设置项 label / description / extra / saveMessage |
| `chat` | 对话区 placeholder / 按钮文案 / 状态提示 |
| `errors` | 按 chat / settings / runtime 归属的错误消息 |
| `tools` | 工具名 / 描述 / 分类名 |
| `actions` | 跨页面且语义稳定的通用动作（保存/取消/编辑/删除/复制…） |
| `time` | 相对时间文案（刚刚/分钟前/昨天…） |
| `about` | 关于页面字段 |
| `tray` | 系统托盘 / 右键菜单项 |
| `command` | 命令面板分类名 |

**命名原则**：
- 页面区域使用 `title` / `description` 等有限且有上下文的字段
- 同一词仅在同一语义下复用；不同语义即使字面相同也分 key
- 需要参数的消息使用 i18next 插值语法
- 所有 locale 的 key 集合和插值占位符集合必须完全一致

### 3.4 平台覆盖（参考 BongoCat）

少数文案在 macOS / Windows 上需要不同表达。处理方式：
- 在 locale JSON 中将 base key 拆为嵌套对象，含 `macos` / `windows` 子 key
- 调用点统一使用平台分发函数，不写平台条件分支
- 查找顺序：`<base_key>.<current_platform>` → `<base_key>`（回退）

## 4. 各层规格

### 4.1 前端 i18n 初始化

- 引入 `i18next` + `react-i18next` + `i18next-browser-languagedetector`
- 创建 `src/i18n/` 目录，含初始化入口与 locale JSON 资源
- 在应用入口 `import` 初始化模块，确保早于任何组件渲染
- `fallbackLng` 设为 `zh-CN`（当前主力语言）

### 4.2 前端翻译使用

采用**混合模式**，按场景选择翻译策略：

#### 4.2.1 模式 C — 声明式定义层 + 集中翻译边界（Presentation 层）

适用于声明式设置系统的数据驱动渲染链路：

```
定义层 (defineSetting/defineSection/definePage)     翻译层 (SettingsGroup)              渲染层 (SettingField)
─────────────────────────────────────────────     ─────────────────────────────     ──────────────────────────
label: "settings.xxx.label"                    →  label: "语言"                   →  <span>{label}</span>
extra: "settings.xxx.extra"                    →  extra: "关闭时最小化到托盘"       →  <span>{extra}</span>
options[0].label: "settings.xxx.opt.key"       →  options[0].label: "跟随系统"     →  <Select options={options} />
saveMessage: "settings.xxx.save_msg"           →  saveMessage: "已保存"            →  messageApi.success(msg)
placeholder: "settings.xxx.placeholder"        →  placeholder: "请输入..."         →  <Input placeholder={ph} />
```

**核心原则**：
- **定义层存 i18n key 全路径字面量**，禁止拼接（与 BongoCat `presentation.rs` 对齐）
- **翻译层（SettingsGroup）统一调用 `t()`**，将 schema 中所有 i18n key 翻译为 display string
- **渲染层（SettingField）纯渲染**，不依赖 `useTranslation`，不调用 `t()`，接收的 props 全部是已翻译的 display string
- key 路径 = JSON 嵌套路径，与 UI 结构同构

**优点**：
1. i18n 职责集中在单一翻译边界，与 BongoCat `presentation.rs` 完全对齐
2. SettingField 变成纯渲染组件，不 import `useTranslation` / `react-i18next`
3. 新增 setting 类型零成本 — 新 Field 组件不需要碰 i18n
4. 与声明式系统的数据驱动架构一致：`defineSetting() → 数据 → SettingsGroup(翻译) → SettingField(渲染)`

#### 4.2.2 模式 A — 组件内直接 `t()`

适用于不在 SettingsGroup 翻译链路上的组件：
- 自定义渲染器（ThemeRadioRender、RendererSelectRender、MascotColorGroupRender、KeybindingField）
- Chat / Provider 对话框 / 命令面板等普通 UI 组件
- 错误码映射（chatErrors.ts）

这些组件直接 `const { t } = useTranslation(); t("key")`，主流做法，简单直觉。

#### 4.2.3 通用规则

- 非组件上下文通过 `i18n.t()` 直接调用
- 禁止在组件中残留任何用户可见的中/英文字面量
- 技术性标识（CSS class、HTML id、路由 path、Tauri event 名）不翻译
- 语言选项的 label 使用**本名（endonym）**，不走 i18n（如 "简体中文" 永远显示为 "简体中文"，不随界面语言变化）

#### 4.2.4 命令面板规则

命令面板（CommandPalette）是 `useMemo` 驱动的数据管线，存在两个特有陷阱：

**陷阱 1：模块级常量提前翻译**

`COMMAND_CATEGORIES` 等模块级数组在 import 时执行，此时 `translateKey()` 可能返回 key 本身（i18n 未初始化）或旧语言文本（语言切换后不重算）。

**做法**：模块级常量存 i18n key，渲染时翻译。

```ts
// ❌ 模块级翻译 — 语言切换后不更新
export const COMMAND_CATEGORIES = [
  { key: "tools", label: translateKey("command.category.tools"), ... },
];

// ✅ 模块级存 key，渲染时翻译
export const COMMAND_CATEGORIES = [
  { key: "tools", label: "command.category.tools", ... },
];
// 渲染处：{meta?.icon} {translateKey(meta.label)}
```

**陷阱 2：可变引用相等性**

`getAllCommands()` 返回模块级可变数组 `commandStore` 的同一引用。`useMemo` 用 `Object.is()` 比较依赖——引用相同就命中缓存，即使内容已变。

**做法**：展开为新数组，打破引用相等性。

```ts
// ❌ 返回同一引用 — useMemo 不重算
const allCommands = useMemo(() => {
  aggregateCommands(navigate);
  return getAllCommands();
}, [navigate, currentI18n.language]);

// ✅ 展开为新数组 — 语言切换时产生新引用
const allCommands = useMemo(() => {
  aggregateCommands(navigate);
  return [...getAllCommands()];
}, [navigate, currentI18n.language]);
```

**陷阱 3：useMemo 依赖缺少语言**

`useMemo` 必须包含 `currentI18n.language`（来自 `useTranslation()`），否则语言切换不触发重算。

### 4.3 第三方库 locale 联动

| 库 | 联动方式 |
|----|---------|
| Ant Design | `ConfigProvider` 的 `locale` prop，随语言切换动态导入对应 locale |
| date-fns | `formatDistanceToNow` 等函数的 `locale` 参数，动态导入对应 locale |

### 4.4 Rust assistant-i18n 升级

- 从手写简易版升级为 `rust_i18n` crate 驱动
- 使用 `rust_i18n::i18n!` 宏声明 catalog owner
- 支持 `fallback = "en-US"`
- 扩展 locale JSON 至与前端同构的领域结构
- `text(locale, key)` 函数供 tray / window / cmd 消费

### 4.5 语言切换链路

1. 用户在通用设置切换语言选项
2. `settingsDispatcher` 发起 `patch_conf` 写入 `AppConf.language`
3. 前端监听 `AppConfChanged` 事件，调用 `i18next.changeLanguage()`
4. 同步调用 Rust `set_locale` 命令更新后端 locale
5. Ant Design / date-fns locale 联动更新
6. 所有打开窗口（Orb / 主窗口）通过跨窗口广播同步生效

### 4.6 语言选项

- 首版支持 `zh-CN`（简体中文）和 `en-US`（English）
- 选项 label 使用**本名（endonym）**直接硬编码，不走 i18n：`"简体中文"` / `"English"` 永远以自身语言显示，不随界面语言变化（与 Chrome / VS Code / macOS 等主流产品对齐）
- 后续扩展只需添加 locale JSON 文件 + 注册到 shipped locales 列表

## 5. 硬编码替换策略

### 5.1 优先级

| 优先级 | 区域 | 理由 |
|--------|------|------|
| P0 | 设置系统定义层 | 最集中，`defineSetting` / `defineSection` / `definePage` 的文案属性可统一替换 |
| P1 | Chat 错误体系 + 历史管理 | 用户直接可见的高频提示 |
| P2 | 工具箱注册表 + 分类 | 工具名/描述/分类名 |
| P3 | 设置共享组件 | 运行时 message / placeholder |
| P4 | Provider 管理对话框 | 表单 label / 验证消息 |
| P5 | 命令面板 + 其他组件 | 分散的零散文案 |
| P6 | Rust 端 tray / window / cmd / storage | 需 assistant-i18n 升级后消费 |

### 5.2 替换原则

- **定义层优先**：`defineSetting` / `defineSection` / `definePage` / `defineTool` / `defineCommand` 等注册表中的 label / description / extra / saveMessage 统一改为 i18n key
- **渲染层跟随**：共享组件从 schema 中读取 key 后调用 `t()` 渲染
- **运行时消息**：`message.success()` / `message.error()` 等提示文案直接调用 `t()`
- **Rust 端**：菜单项文本 / 错误消息 / 内置模式名改为 `assistant_i18n::text()` 调用

### 5.3 特殊处理

| 场景 | 策略 |
|------|------|
| 内置 Agent 模式名/描述 | DB 存稳定 key，前端显示层翻译；避免 seed 数据含自然语言 |
| 工具 `name` / `description` | 注册表存 i18n key，渲染层通过 hook 统一翻译 |
| 相对时间文案 | 改用 date-fns locale 化配 + i18n 插值，替代手写分支 |
| Ant Design 内置文案 | 通过 `ConfigProvider.locale` 整体切换，不逐个覆盖 |

## 6. 校验与质量保障

### 6.1 Key 一致性校验

- 所有 locale JSON 的 key 集合必须完全一致
- 插值占位符名称和数量必须一致
- CI 中增加校验脚本，递归展开 JSON 后比对 key 集合与占位符集合

### 6.2 硬编码检测

- ESLint 规则或 CI 脚本检测组件中残留的中/英文字面量
- 对已知豁免区域（路由 path、CSS class、技术标识）配置白名单

### 6.3 回退行为

- 前端：i18next `fallbackLng` → `zh-CN`
- Rust：`rust_i18n` fallback → `en-US`
- 缺失 key 渲染为 key 字符串本身（开发期可见，生产期不应出现）

## 7. 不做的事

| 范围 | 理由 |
|------|------|
| 第三方 locale 贡献流程 | 首版仅内部维护，社区翻译流程后续规划 |
| 自动翻译 / AI 翻译集成 | 质量不可控，首版手工翻译 |
| RTL 布局适配 | 首版无 RTL 语言，后续按需 |
| 语言包按需加载 / 懒加载 | 桌面应用包体敏感度低，首版全量内嵌 |
| locale 文件热更新 | 桌面应用随版本发布，无需运行时更新 |
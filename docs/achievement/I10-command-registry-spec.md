# I10 — 命令面板注册表化 规格说明

## 1. 交付目标

将命令面板从硬编码过程式实现，重构为**注册表驱动 + 自动聚合 + Spotlight 风格 UI**，与 `definePage()` / `defineTool()` 架构对齐。新增设置页或工具时，命令面板自动出现对应命令，零维护成本。

> **前置依赖**：I8 ✅（声明式设置系统）、I3 ✅（工具箱注册表）

## 2. 当前问题

| 类别 | 问题 | 影响 |
|------|------|------|
| 架构 | 命令项硬编码在 379 行组件内；设置页导航与 `definePage()` 重复声明；工具映射内联；分类字符串无类型约束；`IS_MAC` 第三次重复定义 | 新增命令/设置页必须改大文件，易遗漏（`agents` 页已遗漏） |
| UI | Ant Design Modal 偏重；`<Divider>` 分隔粗糙；选中态仅背景色；结果区 320px 固定；fade 动画迟钝 | 视觉违和，不像 Spotlight/Raycast |
| 键盘 | ↑↓ 无法从输入框跳到结果列表；无 Tab 跨分类；无 Ctrl+N/P；无 Ctrl+Enter 保持面板；无 1-9 快捷；鼠标 hover 改变键盘锚点 | 键盘流断裂，必须用鼠标 |

## 3. 架构设计

### 3.1 注册表对齐

| 维度 | I8 `defineSetting` | I3 `defineTool` | I10 `defineCommand` |
|------|-------------------|----------------|-------------------|
| 能力声明 | `type` | `category` | `category` |
| 自动适配 | UI 选控件 | 工具箱分组 | 命令面板分组 |
| 开放-封闭 | 新设置项不改框架 | 新工具不改框架 | 新命令不改 CommandPalette |

### 3.2 三层架构

```
Layer 2: CommandPalette.tsx          — Spotlight 浮层 UI + 键盘导航
Layer 1: commandRegistry.tsx         — defineCommand() + getAllCommands() + clearCommands()
         commandAggregation.tsx      — aggregateCommands() + mergeKeywords()
         commandPaletteBindings.ts   — useCommandPaletteShortcut()
Layer 0: CommandCategory + CommandDefinition（合入 commandRegistry.tsx）
```

## 4. 各层规格

### 4.1 类型与注册表（`commandRegistry.tsx`）

```typescript
export type CommandCategory = "tools" | "navigation" | "actions" | "external";

export interface CommandDefinition {
  id: string;
  name: string;
  description?: string;
  keywords: string[];       // 必填，mergeKeywords 保证非空
  category: CommandCategory;
  icon?: React.ReactNode;
  action: () => void | Promise<void>;
  closeOnSelect?: boolean;
}

export const COMMAND_CATEGORIES: readonly CommandCategoryMeta[] = [
  { key: "tools",     label: "工具", icon: <ThunderboltOutlined />, order: 0 },
  { key: "navigation", label: "导航", icon: <AppstoreOutlined />,   order: 1 },
  { key: "actions",    label: "操作", icon: <BulbOutlined />,       order: 2 },
  { key: "external",   label: "外部", icon: <GlobalOutlined />,     order: 3 },
];
```

注册函数：`defineCommand(cmd)` / `getAllCommands()` / `clearCommands()` / `getCommandsByCategory()` / `getCommandById()`。

**注册时机**：不在模块导入时执行（`navigate` 需组件内获取）。`CommandPalette.tsx` 通过 `useMemo` 调用 `aggregateCommands(navigate)` 一次。防重：聚合前 `clearCommands()`。

### 4.2 自动聚合（`commandAggregation.tsx`）

#### `mergeKeywords` — 两层合并

`最终 keywords = 自动推导(label 分词 ∪ id 分词 ∪ routePath 分词 ∪ categoryLabel 分词) ∪ 注册者 keywords`

i18n 上线后 `label` 本身就是当前语言，推导自动跟进。

#### 聚合工厂

```typescript
export function aggregateCommands(navigate: NavigateFunction): void {
  clearCommands();
  registerSettingsCommands(navigate);      // 遍历 SETTINGS_PAGES → navigation
  registerSettingFieldCommands(navigate);  // 遍历 getAllDefinitions() → navigation + ?focus=key
  registerToolCommands(navigate);          // 遍历 tools → tools
  registerManualCommands();               // 操作/外部/其他路由
}
```

| 注册函数 | 数据源 | category | action |
|---------|--------|----------|--------|
| `registerSettingsCommands` | `SETTINGS_PAGES` | navigation | `navigate(/settings/${page.id})` |
| `registerSettingFieldCommands` | `getAllDefinitions()` | navigation | `navigate(/settings/${page}?focus=${key})` |
| `registerToolCommands` | `tools` | tools | `navigate(/tools/${id})` |
| `registerManualCommands` | 硬编码 | actions/external/navigation | 见下表 |

icon 映射：设置页按 `categoryId`（general→SettingOutlined, provider→ApiOutlined, about→InfoCircleOutlined）；工具直接用 `tool.icon`。

#### 手动命令

| id | name | category | action 要点 |
|----|------|----------|------------|
| `action:toggle-theme` | 切换主题 | actions | `getState().appConf` 读最新值 → `dispatchConfPatch` + `dispatchThemeChanged`；`closeOnSelect: false` |
| `action:random-tool` | 随机工具 | actions | `tools[random]` → navigate |
| `ext:github` | GitHub 仓库 | external | `window.open` |
| `ext:bug-report` | 报告问题 | external | `window.open` |
| `nav:history` | 消息历史 | navigation | `navigate("/history")` |

> **关键**：操作命令 `action` 在**执行时**读 `useSettingsStore.getState()`，不闭包捕获过期快照。

### 4.3 快捷键绑定（`commandPaletteBindings.ts`）

`useCommandPaletteShortcut(binding, onToggle)` — 从组件抽出，复用 `shortcutTypes.ts` 的 `IS_MAC`，默认 `Meta+K` / `Control+K`。

### 4.4 UI 组件（`CommandPalette.tsx`）

职责：`aggregateCommands` → Fuse.js 搜索 → Spotlight 浮层渲染 → 键盘导航。预计 ~150 行。

#### Spotlight 浮层（替代 Modal）

```
┌──────────────────────────────────────────┐
│  🔍 搜索工具、页面、操作...               │
├──────────────────────────────────────────┤
│  工具                                    │  ← 轻量标题（非 Divider）
│  ▸ Hash 计算              SHA1/SHA256    │  ← 左侧 2px accent 条
│    UUID 生成              v1/v4 批量      │
│  导航                                    │
│    通用设置               语言/自启/托盘   │
│  操作                                    │
│    切换主题               亮→暗→跟随系统  │
└──────────────────────────────────────────┘
```

- `position: fixed; top: 15vh; left: 50%; transform: translateX(-50%); width: 560px`
- `border-radius: 12px; box-shadow: 0 8px 32px var(--color-shadow)`
- 无 overlay / 无动画 / 点击外部关闭 / 暗色用 CSS 变量

#### 选中态

```css
.option { padding: 8px 12px; border-radius: 6px; border-left: 2px solid transparent; transition: 0.1s; }
.optionActive { background: var(--color-bg-hover); border-left-color: var(--color-primary); }
```

#### 搜索高亮

不使用 `<mark>` 标记。Fuse.js 仅用于搜索排序，命令名直接渲染纯文本。

#### 键盘交互

| 按键 | 行为 |
|------|------|
| `↓` / `Ctrl/Cmd+N` | +1；**输入框时 ↓ 跳到第一个结果** |
| `↑` / `Ctrl/Cmd+P` | -1；首项时 ↑ 回输入框 |
| `Tab` / `Shift+Tab` | 跳到下/上一分类首项 |
| `Enter` | 无选中项时先跳到第 0 项；有选中项时执行 |
| `Ctrl+Enter` | 执行但保持面板 |
| `1`-`9` | 仅在 `selectedIndex >= 0` 时直接执行前 9 条（避免与输入框文字输入冲突） |
| `Escape` | 有搜索词时先清空搜索；无搜索词时关闭面板 |

**鼠标/键盘同步**：鼠标 `onMouseEnter` 直接 `setSelectedIndex(flatIdx)`，hover 同步更新键盘锚点，↓/↑ 从 hover 位置继续。无独立 `hoveredIndex` 状态。

空状态：图标 + "没有匹配的命令"。结果区 `max-height: 50vh`。

#### 文件结构

```
src/components/commandPalette/
  ├── commandRegistry.tsx        ← 类型 + defineCommand + COMMAND_CATEGORIES
  ├── commandAggregation.tsx     ← aggregateCommands + mergeKeywords
  ├── commandPaletteBindings.ts  ← useCommandPaletteShortcut
  ├── CommandPalette.tsx         ← 纯 UI
  └── CommandPalette.module.css
```

## 5. 验收标准

### 5.1 架构

| # | 标准 |
|---|------|
| 1 | `defineCommand()` 注册表驱动，无硬编码命令项 |
| 2 | `CommandCategory` 联合类型，编译时检查 |
| 3 | `CommandPalette.tsx` ≤ 150 行 |
| 4 | `IS_MAC` 仅在 `shortcutTypes.ts` 定义并 export |

> 扩展性：新增 `definePage()` / `defineTool()` 后命令自动出现，无需额外实现。

### 5.2 功能

| # | 标准 | 验证 |
|---|------|------|
| 5 | Cmd+K 唤起 | 键盘 |
| 6 | 搜设置页/工具/操作/外部均可达 | "外观"/"theme"/"color"/"github" |
| 7 | 关键词自动推导 | "settings"/"appearance" |
| 8 | action 正确执行 | 点击 |
| 9 | 每分类 ≤ `COMMAND_PALETTE_MAX_PER_CATEGORY` | 泛词 |
| 10 | 设置项直达 + `?focus=` 滚动高亮 | "开机自启"/"launch" |

### 5.3 UI + 键盘

| # | 标准 | 验证 |
|---|------|------|
| 11 | Spotlight 浮层，非 Modal；轻量分类标题；accent 条选中态 | 视觉 |
| 12 | 无 `<mark>` 高亮；空状态提示 | 输入 "hash" / "zzzzz" |
| 13 | ↓ 从输入框跳结果；↑ 首项回输入框；Tab 跨分类 | 键盘 |
| 14 | Ctrl/Cmd+N/P；Ctrl+Enter 保持面板；1-9 仅在选中态时快捷 | 键盘 |
| 15 | 鼠标 hover 同步键盘锚点，↓/↑ 从 hover 位置继续 | 混合操作 |

### 5.4 编译

`tsc --noEmit` + `eslint --fix` + `cargo check` 通过
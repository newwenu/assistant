# I10 — 命令面板注册表化 规格说明

## 1. 交付目标

将命令面板从硬编码过程式实现，重构为**注册表驱动 + 自动聚合**模式，与 `definePage()` / `defineTool()` 架构对齐。新增设置页或工具时，命令面板自动出现对应命令，零维护成本。

> **前置依赖**：I8 ✅（声明式设置系统）、I3 ✅（工具箱注册表）
> **参考**：I8 `defineSetting` 注册表模式、I3 `defineTool` 注册表模式、VS Code Command Palette

## 2. 当前问题

| # | 问题 | 影响 |
|---|------|------|
| P1 | 命令项全部硬编码在 `CommandPalette.tsx` 的 `allOptions` 内联数组中 | 新增命令必须改 379 行大文件 |
| P2 | 设置页面导航命令与 `settingsRegistry.ts` 的 `definePage()` 数据重复声明 | 新增设置页需改两处，易遗漏 |
| P3 | 工具命令的映射逻辑内联在组件中 | 与 `tools` 注册表脱节 |
| P4 | 分类系统使用硬编码字符串 `"Tools"/"Navigation"/"Actions"/"External"`，无类型约束 | 拼写错误无编译时检查 |
| P5 | 快捷键绑定逻辑 30 行内嵌在组件中，且 `IS_MAC` 第三次重复定义 | 逻辑不内聚，去重失败 |
| P6 | 组件 379 行，数据声明 / 搜索 / 键盘绑定 / UI 渲染全耦合 | 难以维护和测试 |
| P7 | 当前手写导航命令遗漏 `agents` 设置页（`SETTINGS_PAGES` 有 6 页，命令面板只注册了 5 个） | 新增设置页后极易遗漏，已有实例 |

## 3. 架构设计

### 3.1 与 I8 / I3 注册表对齐

| 维度 | I8 设置注册表 | I3 工具注册表 | I10 命令注册表 |
|------|--------------|--------------|---------------|
| 注册入口 | `defineSetting(def)` | `defineTool(tool)` | `defineCommand(cmd)` |
| 能力声明 | `SettingDefinition.type` | `ToolDefinition.category` | `CommandDefinition.category` |
| 自动适配 | UI 根据 type 选控件 | 工具箱根据 category 分组 | 命令面板根据 category 分组 |
| 扩展方式 | 第三方调用 defineSetting | 第三方调用 defineTool | 第三方调用 defineCommand |
| 开放-封闭 | 新设置项不改框架 | 新工具不改框架 | 新命令不改 CommandPalette |

### 3.2 三层架构

```
┌─────────────────────────────────────────────────────┐
│  Layer 2: UI（命令面板组件）                         │
│  CommandPalette.tsx — 纯 UI：搜索 + 渲染 + 键盘导航 │
├─────────────────────────────────────────────────────┤
│  Layer 1: Registry（注册表 + 自动聚合）              │
│  commandRegistry.ts  commandAggregation.ts           │
│  commandKeywords.ts  commandPaletteBindings.ts       │
├─────────────────────────────────────────────────────┤
│  Layer 0: Types（类型定义）                          │
│  commandTypes.ts — CommandCategory + CommandDefinition│
└─────────────────────────────────────────────────────┘

依赖方向：Layer N → Layer N-1（严格单向）
```

## 4. 各层详细规格

### 4.1 Layer 0+1: 类型与注册表（`commandRegistry.tsx`）

类型定义与注册函数合并在同一文件中，因为它们总是一起使用，分离无独立价值。

#### `CommandCategory`

```typescript
export type CommandCategory = "tools" | "navigation" | "actions" | "external";
```

替代当前硬编码字符串，提供编译时类型检查。

#### `CommandDefinition`

```typescript
export interface CommandDefinition {
  id: string;
  name: string;
  description?: string;
  keywords: string[];
  category: CommandCategory;
  icon?: React.ReactNode;
  action: () => void | Promise<void>;
  closeOnSelect?: boolean;
}
```

与当前 `PaletteOption` 对齐，但 `keywords` 为必填（自动推导保证非空）。

#### 分类元数据

```typescript
export interface CommandCategoryMeta {
  key: CommandCategory;
  label: string;
  icon: React.ReactNode;
  order: number;
}

export const COMMAND_CATEGORIES: readonly CommandCategoryMeta[] = [
  { key: "tools",      label: "工具",   icon: <ThunderboltOutlined />, order: 0 },
  { key: "navigation",  label: "导航",   icon: <AppstoreOutlined />,   order: 1 },
  { key: "actions",     label: "操作",   icon: <BulbOutlined />,       order: 2 },
  { key: "external",    label: "外部",   icon: <GlobalOutlined />,     order: 3 },
];
```

#### `defineCommand` — 注册函数

```typescript
const commands: CommandDefinition[] = [];

export function defineCommand(cmd: CommandDefinition): CommandDefinition {
  commands.push(cmd);
  return cmd;
}

export function getAllCommands(): readonly CommandDefinition[] {
  return commands;
}

export function getCommandsByCategory(
  category: CommandCategory,
): CommandDefinition[] {
  return commands.filter((c) => c.category === category);
}

export function getCommandById(id: string): CommandDefinition | undefined {
  return commands.find((c) => c.id === id);
}
```

#### 注册时机与防重

命令注册**不在模块导入时执行**（因为 `navigate` 需在 React 组件内获取）。`CommandPalette.tsx` 在组件内通过 `useMemo` 调用 `aggregateCommands(navigate)` 一次。

**防重机制**：`aggregateCommands` 内部先调用 `clearCommands()` 清空已有命令，再重新注册。这保证 `navigate` 引用变化时不会导致命令膨胀。

```typescript
export function aggregateCommands(navigate: NavigateFunction): void {
  clearCommands();
  registerSettingsCommands(navigate);
  registerToolCommands(navigate);
  registerManualCommands();
}
```

`CommandPalette.tsx` 在组件内调用：

```typescript
const navigate = useNavigate();
const allCommands = useMemo(() => {
  aggregateCommands(navigate);
  return getAllCommands();
}, [navigate]);
```

### 4.2 Layer 1: 自动聚合与关键词（`commandAggregation.tsx`）

自动聚合逻辑与关键词合并函数合并在同一文件中，因为 `mergeKeywords` 仅被聚合调用，无外部消费者。

#### `mergeKeywords` — 两层合并

```typescript
interface MergeKeywordsInput {
  registered: string[];
  label: string;
  id: string;
  routePath?: string;
  categoryLabel?: string;
}

export function mergeKeywords(input: MergeKeywordsInput): string[] {
  const derived = new Set<string>();

  // 层 1：自动推导
  for (const segment of input.label.split(/[\s\-_]+/)) {
    if (segment) derived.add(segment.toLowerCase());
  }
  for (const segment of input.id.split(/[_\-]+/)) {
    if (segment) derived.add(segment.toLowerCase());
  }
  if (input.routePath) {
    for (const seg of input.routePath.split("/")) {
      if (seg) derived.add(seg.toLowerCase());
    }
  }
  if (input.categoryLabel) {
    for (const seg of input.categoryLabel.split(/[\s\-_]+/)) {
      if (seg) derived.add(seg.toLowerCase());
    }
  }

  // 层 2：注册者提供的 keywords 合并
  for (const kw of input.registered) {
    derived.add(kw.toLowerCase());
  }

  return [...derived];
}
```

**合并公式**：`最终 keywords = 自动推导 ∪ 注册者 keywords`

去重后交给 Fuse.js 搜索。i18n 上线后 `label` 本身就是当前语言，推导自动跟进。

#### 自动聚合 — 数据源

从已有注册表自动生成命令，替代当前硬编码：

```typescript
import { SETTINGS_PAGES, SETTINGS_CATEGORIES } from "@/settings/settingsRegistry";
import { tools } from "@/tools/registry";
import { defineCommand } from "./commandRegistry";

// --- 设置页面 → 导航命令 ---
// icon 按 categoryId 映射（与 settingsRegistry 的 SETTINGS_CATEGORIES 对齐）
const SETTINGS_CATEGORY_ICONS: Record<string, React.ReactNode> = {
  general: <SettingOutlined />,
  provider: <ApiOutlined />,
  about: <InfoCircleOutlined />,
};

// 分类标签从 SETTINGS_CATEGORIES 获取，无需额外定义
const SETTINGS_CATEGORY_LABELS = Object.fromEntries(
  SETTINGS_CATEGORIES.map((c) => [c.id, c.label]),
);

for (const page of SETTINGS_PAGES) {
  defineCommand({
    id: `nav:settings-${page.id}`,
    name: page.label,
    keywords: mergeKeywords({
      registered: page.keywords,
      label: page.label,
      id: page.id,
      routePath: `/settings/${page.id}`,
      categoryLabel: SETTINGS_CATEGORY_LABELS[page.categoryId],
    }),
    category: "navigation",
    icon: SETTINGS_CATEGORY_ICONS[page.categoryId],
    action: () => navigate(`/settings/${page.id}`),
  });
}

// --- 工具 → 工具命令 ---
for (const tool of tools) {
  const ToolIcon = tool.icon;
  defineCommand({
    id: `tool:${tool.id}`,
    name: tool.name,
    description: tool.description,
    keywords: mergeKeywords({
      registered: tool.keywords,
      label: tool.name,
      id: tool.id,
      routePath: `/tools/${tool.id}`,
    }),
    category: "tools",
    icon: <ToolIcon />,
    action: () => navigate(`/tools/${tool.id}`),
  });
}
```

**关键**：`navigate` 通过工厂函数注入，不在模块顶层调用 `useNavigate()`。

#### 聚合工厂

```typescript
export function aggregateCommands(navigate: NavigateFunction): void {
  clearCommands();
  registerSettingsCommands(navigate);
  registerToolCommands(navigate);
  registerManualCommands();
}
```

`CommandPalette.tsx` 在组件内调用 `aggregateCommands(navigate)` 一次（通过 `useMemo` + `navigate` 依赖），确保路由可用。`registerManualCommands` 不需要 `navigate` 参数，因为操作命令通过 `useSettingsStore.getState()` 读取状态，外部链接通过 `window.open` 打开。

### 4.3 Layer 1: 手动命令注册

操作类、外部链接、非设置/非工具的路由页面需手动注册。

> **关键设计**：操作命令的 `action` 在**执行时**读取 store 状态，而非注册时闭包捕获。这保证 action 始终使用最新状态（如 `appConf`），避免注册时快照过期。

```typescript
import { useSettingsStore } from "@/stores/settingsStore";
import { dispatchConfPatch, dispatchThemeChanged } from "@/settings/settingsDispatcher";

// 操作命令
defineCommand({
  id: "action:toggle-theme",
  name: "切换主题",
  description: "循环切换 亮色 → 暗色 → 跟随系统",
  keywords: mergeKeywords({
    registered: ["dark", "light", "system", "theme", "主题", "暗色", "亮色"],
    label: "切换主题",
    id: "toggle-theme",
  }),
  category: "actions",
  icon: <SkinOutlined />,
  closeOnSelect: false,
  action: async () => {
    const appConf = useSettingsStore.getState().appConf;
    if (!appConf) return;
    const order: Array<"system" | "light" | "dark"> = ["system", "light", "dark"];
    const current = appConf.theme as "system" | "light" | "dark";
    const next = order[(order.indexOf(current) + 1) % order.length];
    await dispatchConfPatch("app", { theme: next });
    await dispatchThemeChanged(next);
  },
});

defineCommand({
  id: "action:random-tool",
  name: "随机工具",
  description: "随机打开一个工具",
  keywords: mergeKeywords({
    registered: ["random", "dice", "随机", "随便"],
    label: "随机工具",
    id: "random-tool",
  }),
  category: "actions",
  icon: <ReloadOutlined />,
  action: () => { /* ... */ },
});

// 外部链接
defineCommand({
  id: "ext:github",
  name: "GitHub 仓库",
  description: "查看源代码",
  keywords: mergeKeywords({
    registered: ["github", "repo", "source", "代码", "源码"],
    label: "GitHub 仓库",
    id: "github",
  }),
  category: "external",
  icon: <GlobalOutlined />,
  action: () => window.open("https://github.com", "_blank"),
});

// 其他路由页面
defineCommand({
  id: "nav:history",
  name: "消息历史",
  description: "查看对话历史记录",
  keywords: mergeKeywords({
    registered: ["history", "chat", "对话", "历史"],
    label: "消息历史",
    id: "history",
    routePath: "/history",
  }),
  category: "navigation",
  icon: <HistoryOutlined />,
  action: () => navigate("/history"),
});
```

### 4.4 Layer 1: 快捷键绑定（`commandPaletteBindings.ts`）

#### `commandPaletteBindings.ts`

从 `CommandPalette.tsx` 抽出快捷键监听逻辑，复用 `shortcutTypes.ts` 的 `IS_MAC`：

```typescript
import { IS_MAC } from "@/settings/shortcutTypes";

export function useCommandPaletteShortcut(
  binding: string | undefined,
  onToggle: () => void,
): void {
  useEffect(() => {
    const effectiveBinding = binding ?? (IS_MAC ? "Meta+K" : "Control+K");

    const requireMeta = /\b(meta|super|cmd)\b/i.test(effectiveBinding);
    const requireCtrl = /\bctrl\b/i.test(effectiveBinding) && !requireMeta;
    const requireShift = /\bshift\b/i.test(effectiveBinding);
    const requireAlt = /\balt\b/i.test(effectiveBinding);
    const keyMatch = effectiveBinding.match(/\b([A-Z0-9])\s*$/i);
    const key = keyMatch ? keyMatch[1].toLowerCase() : "k";

    const handler = (e: KeyboardEvent) => {
      if (
        e.target instanceof HTMLInputElement &&
        (e.target as HTMLInputElement).isContentEditable
      )
        return;
      const metaOk = requireMeta
        ? IS_MAC ? e.metaKey : e.ctrlKey
        : !e.metaKey && !e.ctrlKey;
      const ctrlOk = requireCtrl ? e.ctrlKey : true;
      const shiftOk = requireShift ? e.shiftKey : !e.shiftKey;
      const altOk = requireAlt ? e.altKey : !e.altKey;

      if (metaOk && ctrlOk && shiftOk && altOk && e.key.toLowerCase() === key) {
        e.preventDefault();
        onToggle();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [binding, onToggle]);
}
```

### 4.5 Layer 2: UI 组件（`CommandPalette.tsx`）

#### `CommandPalette.tsx` — 瘦身后职责

组件仅负责：
1. 调用 `aggregateCommands(navigate)` 获取命令列表
2. Fuse.js 搜索
3. 分组渲染
4. 键盘导航（↑↓ + Enter + Escape）

预计 ~120 行，与 `SettingsGroup.tsx`（122 行）体量相当。

#### 文件结构

```
src/components/commandPalette/
  ├── commandRegistry.tsx           ← 类型定义 + defineCommand() + getAllCommands() + clearCommands() + COMMAND_CATEGORIES
  ├── commandAggregation.tsx       ← aggregateCommands() + register* + mergeKeywords()
  ├── commandPaletteBindings.ts    ← useCommandPaletteShortcut() 快捷键绑定
  ├── CommandPalette.tsx           ← 纯 UI 组件
  └── CommandPalette.module.css    ← 样式（从现有迁移）
```

> **合并原则**：`commandTypes` 与 `commandRegistry` 总是一起使用，合为 `commandRegistry.tsx`；`commandKeywords` 仅被聚合调用，合入 `commandAggregation.tsx`。最终 4 个源文件，与 tools 模块（registry + types + categories）体量相当。

## 5. 增强功能（P2）

### 5.1 设置字段命令自动生成

遍历 `getAllDefinitions()`，生成直达具体设置项的命令：

```typescript
import { getAllDefinitions } from "@/settings/settingRegistry";

const GROUP_TO_PAGE: Record<ConfGroup, SettingsPageId> = {
  app: "general",
  orb: "appearance",
  mascot: "appearance",
};

for (const def of getAllDefinitions()) {
  defineCommand({
    id: `setting:${def.group}:${def.key}`,
    name: def.label,
    description: def.description,
    keywords: mergeKeywords({
      registered: [def.key],
      label: def.label,
      id: def.key,
      routePath: `/settings/${GROUP_TO_PAGE[def.group]}`,
    }),
    category: "navigation",
    action: () => navigate(`/settings/${GROUP_TO_PAGE[def.group]}?focus=${def.key}`),
  });
}
```

用户搜索 "开机自启" 或 "launch" 可直达该设置项。

## 6. 验收标准

### 6.1 架构验收

| # | 标准 | 验证方式 |
|---|------|---------|
| 1 | 命令面板使用 `defineCommand()` 注册表，无硬编码命令项 | 代码审查：`CommandPalette.tsx` 不含 `PaletteOption` 内联数组 |
| 2 | `CommandCategory` 为联合类型，分类字符串有编译时检查 | `tsc --noEmit` 通过 |
| 3 | `CommandPalette.tsx` ≤ 150 行 | `wc -l` |
| 4 | `IS_MAC` 无重复定义 | 全局搜索 `IS_MAC` 仅在 `shortcutTypes.ts` 出现 |

> **扩展性**：新增 `definePage()` / `defineTool()` 后命令面板自动出现对应命令，这是聚合循环的自然结果，不需要额外实现或验收。

### 6.2 功能验收

| # | 标准 | 验证方式 |
|---|------|---------|
| 5 | Ctrl/Cmd+K 唤起命令面板 | 键盘操作 |
| 6 | 搜索设置页名称/关键词可找到对应导航命令 | 输入 "外观" / "theme" |
| 7 | 搜索工具名称/关键词可找到对应工具命令 | 输入 "颜色" / "color" |
| 8 | 搜索操作命令可找到切换主题等 | 输入 "主题" / "theme" |
| 9 | 搜索外部链接可找到 GitHub 等 | 输入 "github" |
| 10 | 关键词自动推导生效：label 分词、id、路由路径均可搜索 | 输入 "settings" / "appearance" |
| 11 | 选择命令后正确执行 action（导航/切换/打开链接） | 点击验证 |
| 12 | 每分类最多显示 `COMMAND_PALETTE_MAX_PER_CATEGORY` 条 | 搜索泛词验证 |

### 6.3 编译验收

| # | 标准 |
|---|------|
| 13 | `tsc --noEmit` 通过 |
| 14 | `eslint --fix` 通过 |
| 15 | `cargo check` 通过 |

### 6.4 扩展性验收（P2）

| # | 标准 | 验证方式 |
|---|------|---------|
| 18 | 搜索设置项 label 可直达具体设置项 | 输入 "开机自启" |
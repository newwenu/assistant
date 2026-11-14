# SettingsPageShell 统一壳改造设计（组合方案）

## 1. 背景与问题

当前设置模块存在两个页面壳组件，职责高度重叠：

| 组件 | 文件 | 使用方 |
|---|---|---|
| `SettingsPageShell` | `shared/layout/SettingsPageShell.tsx` | GeneralSettings, AppearanceSettings, ShortcutSettings, AboutSettings |
| `EntityListPageShell` | `shared/layout/EntityListPageShell.tsx` | ProviderManager, AgentModeSettings |

**重叠**：返回按钮 + 标题 + Divider 头部布局完全一致。

**差异**：

| 维度 | SettingsPageShell | EntityListPageShell |
|---|---|---|
| 标题来源 | URL `pageId` → 注册表自动推导 | `title` prop 显式传入 |
| 头部右侧 | `extra` 自由插槽 | 搜索框 + 新增按钮（硬编码） |
| 主体内容 | 纯 `children` | 分组列表 + 空状态 + `children` |
| 泛型 | 无 | `<T>` |
| 容器布局 | `maxWidth: 700` 居中窄栏 | 撑满宽度 |

**核心问题**：

1. 两壳头部逻辑重复，改一处漏一处
2. `EntityListPageShell` 的 `title` 与注册表脱节
3. `variant` 分支方案是伪解：壳根本不需要"知道"自己是表单还是列表 — `extra` 插槽 + `children` 本就足够组合一切，任何内容结构差异都应由独立内容组件通过插槽注入，而非壳内分支

## 2. 设计原则

**组合优于分支（Composition over Branching）**

- `SettingsPageShell` 是**纯壳**，只负责头部（返回+标题+分割线）和容器布局，永不因新页面结构而修改
- 列表、表单、树形等**内容结构**由独立组件负责，通过 `children` 和 `extra` 插槽组合进壳
- 未来任何新页面结构 = `SettingsPageShell` + 新内容组件，零修改壳

## 3. 组件职责划分

### 3.1 `SettingsPageShell` — 纯壳（永不修改）

```
┌─────────────────────────────────────────────┐
│  ← 返回   标题（注册表自动）     [extra]   │  ← 头部（壳负责）
│  ─────────────────────────────────────────── │  ← Divider（壳负责）
│  children                                   │  ← 主体（调用方自由组合）
└─────────────────────────────────────────────┘
```

Props：

```ts
interface SettingsPageShellProps {
  children: React.ReactNode;
  extra?: React.ReactNode;   // 头部右侧插槽（搜索栏、操作按钮等）
  wide?: boolean;            // 默认 false：居中窄栏；true：撑满宽度
}
```

- 标题从 URL `pageId` + `SETTINGS_PAGE_MAP` 注册表自动推导，无需 prop
- `wide` 控制容器布局：`false` → `maxWidth: 700, margin: "0 auto"`；`true` → 撑满
- **不再有 `variant`、`list` 等内容结构相关 prop**

### 3.2 `EntityListContent<T>` — 列表主体组件

```
┌─────────────────────────────┐
│  分组标签 (3)               │  ← section label + count
│  ┌───────────────────────┐  │
│  │ row 1                 │  │
│  │ row 2                 │  │  ← section.renderRow
│  │ row 3                 │  │
│  └───────────────────────┘  │
│                             │
│  分组标签 (2)               │
│  ┌───────────────────────┐  │
│  │ row 4                 │  │
│  │ row 5                 │  │
│  └───────────────────────┘  │
└─────────────────────────────┘
```

Props：

```ts
export interface SectionConfig<T> {
  label: string;
  items: T[];
  renderRow: (item: T) => React.ReactNode;
}

interface EntityListContentProps<T> {
  items: T[];
  sections: SectionConfig<T>[];
  emptyDescription: string;
}
```

- 纯展示组件，只负责分组列表渲染 + 空状态
- 不负责搜索/新增（那些在壳的 `extra` 插槽里）

### 3.3 `ListHeaderActions` — 列表头部操作组件

```
[ 🔍 搜索... ]  [ + 新增 ]
```

Props：

```ts
interface ListHeaderActionsProps {
  searchQuery: string;
  onSearchChange: (query: string) => void;
  addLabel: string;
  onAdd: () => void;
  searchPlaceholder?: string;
}
```

- 便捷组件，封装搜索框 + 新增按钮的常见组合
- 调用方也可以不用它，直接在 `extra` 里放自定义内容

## 4. 组合示例

### 4.1 列表页（ProviderManager / AgentModeSettings）

```tsx
<SettingsPageShell
  wide
  extra={
    <ListHeaderActions
      searchQuery={searchQuery}
      onSearchChange={setSearchQuery}
      addLabel={t("settings.provider.add")}
      onAdd={handleAdd}
    />
  }
>
  <EntityListContent
    items={providerStore.providers}
    sections={sections}
    emptyDescription={t("settings.provider.empty")}
  />
  <AddProviderDialog ... />
  <ConnectDialog ... />
</SettingsPageShell>
```

### 4.2 表单页（GeneralSettings / AppearanceSettings）— 不变

```tsx
<SettingsPageShell>
  <SettingsSection>...</SettingsSection>
</SettingsPageShell>
```

### 4.3 带搜索的表单页（ShortcutSettings）— 不变

```tsx
<SettingsPageShell extra={<Space><Input ... /><Button ... /></Space>}>
  <div>...</div>
</SettingsPageShell>
```

### 4.4 未来：树形页面（无需改壳）

```tsx
<SettingsPageShell wide extra={<TreeHeaderActions ... />}>
  <TreeContent ... />
</SettingsPageShell>
```

### 4.5 未来：网格页面（无需改壳）

```tsx
<SettingsPageShell wide extra={<GridHeaderActions ... />}>
  <GridContent ... />
</SettingsPageShell>
```

## 5. 文件变更清单

### 5.1 修改

| 文件 | 变更内容 |
|---|---|
| `shared/layout/SettingsPageShell.tsx` | 增加 `wide` prop，移除 `maxWidth` 硬编码 |
| `provider/ProviderManager.tsx` | 改用 `SettingsPageShell` + `EntityListContent` + `ListHeaderActions` 组合 |
| `pages/AgentModeSettings.tsx` | 同上 |
| `assets/styles/global.css` | 删除 `.entity-list-page` / `.entity-list-page-header`（已由 `.settings-page-shell` / `.settings-page-header` 替代） |

### 5.2 新增

| 文件 | 内容 |
|---|---|
| `shared/layout/EntityListContent.tsx` | `EntityListContent<T>` + `SectionConfig<T>` + `ListHeaderActions` |

### 5.3 删除

| 文件 | 原因 |
|---|---|
| `shared/layout/EntityListPageShell.tsx` | 功能已拆分为 `SettingsPageShell` + `EntityListContent` + `ListHeaderActions` |

### 5.4 不变

| 文件 | 原因 |
|---|---|
| `pages/GeneralSettings.tsx` | 已用 `SettingsPageShell`，无需改动 |
| `pages/AppearanceSettings.tsx` | 同上 |
| `pages/ShortcutSettings.tsx` | 同上 |
| `pages/AboutSettings.tsx` | 同上 |

## 6. 详细实施步骤

### Step 1: 改造 SettingsPageShell 为纯壳

1. 增加 `wide?: boolean` prop
2. 容器样式：`wide` → `{ padding: 24 }`；否则 → `{ maxWidth: 700, margin: "0 auto", padding: 24 }`
3. 移除之前添加的 `variant`/`list`/`ListConfig`/`SectionConfig` 等（回退到纯壳）
4. 保持 `extra` 插槽不变

### Step 2: 创建 EntityListContent.tsx

1. 导出 `SectionConfig<T>` 接口
2. 导出 `EntityListContent<T>` 组件：接收 `items`/`sections`/`emptyDescription`，渲染分组列表或空状态
3. 导出 `ListHeaderActions` 组件：接收 `searchQuery`/`onSearchChange`/`addLabel`/`onAdd`/`searchPlaceholder?`，渲染搜索框+新增按钮

### Step 3: 迁移 ProviderManager

1. 移除 `EntityListPageShell` / `EntityListSectionConfig` import
2. 改 import `SettingsPageShell` + `EntityListContent` + `ListHeaderActions` + `SectionConfig`
3. `sections` 类型改为 `SectionConfig<Provider>[]`
4. JSX 改为组合模式（见 4.1 示例）

### Step 4: 迁移 AgentModeSettings

同 Step 3，类型改为 `SectionConfig<AgentMode>[]`。

### Step 5: CSS 样式合并

1. 删除 `.entity-list-page` 和 `.entity-list-page-header` 规则
2. 保留 `.entity-list-section-label` / `.entity-list-section-count` / `.entity-list-rows` / `.entity-list-row*` 等行级样式

### Step 6: 删除 EntityListPageShell

确认无引用后删除。

### Step 7: 验证

TypeScript 编译 + 各页面渲染正常。

## 7. OCP 保证

| 未来需求 | 是否需要修改 SettingsPageShell |
|---|---|
| 新增列表页 | ❌ 组合 `SettingsPageShell` + `EntityListContent` |
| 新增树形页面 | ❌ 组合 `SettingsPageShell` + 新 `TreeContent` |
| 新增网格页面 | ❌ 组合 `SettingsPageShell` + 新 `GridContent` |
| 列表页只要搜索不要新增 | ❌ `extra` 里只放 `<Input>`，不用 `ListHeaderActions` |
| 列表页新增+筛选+排序 | ❌ `extra` 里放自定义组合 |
| 表单页需要头部操作 | ❌ 用 `extra` 插槽 |

**SettingsPageShell 永不因内容结构变化而修改。**

## 8. 风险与注意事项

1. **标题翻译 key 一致性**：注册表 `label` 与原手动 `title` prop 的翻译 key 需产出相同文本
2. **泛型**：`EntityListContent<T>` 是泛型函数组件，调用方直接传具体类型即可，无需 `as` 断言
3. **CSS 类名**：`.entity-list-page` → `.settings-page-shell`，无外部依赖
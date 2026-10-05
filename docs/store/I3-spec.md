# I3 — 主窗口工具箱 规格说明

## 1. 交付目标

主窗口内可用的工具箱，分类导航 + 搜索 + 收藏 + 12+ 开发/文本工具，工具使用历史持久化。

## 2. 功能规格

### 2.1 工具注册系统

每个工具通过 `defineTool` 工厂注册，统一管理元数据与懒加载组件。

```typescript
interface ToolDefinition {
  id: string;
  name: string;
  description: string;
  category: ToolCategory;
  icon: React.ComponentType;
  keywords: string[];
  component: React.LazyExoticComponent<React.ComponentType>;
  isNew?: boolean;
  createdAt?: Date;
  widthMode?: 'default' | 'wide' | 'full';
}

type ToolCategory = 'crypto' | 'converter' | 'web' | 'dev' | 'text' | 'math';

interface ToolCategoryMeta {
  key: ToolCategory;
  label: string;
  icon: React.ComponentType;
  order: number;
}
```

**`defineTool` 工厂**（参考 it-tools `tool.ts`）：

```typescript
function defineTool(tool: Omit<ToolDefinition, 'isNew'>): ToolDefinition {
  const isNew = tool.createdAt
    ? isAfter(tool.createdAt, subWeeks(new Date(), 2))
    : false;
  return { ...tool, isNew };
}
```

- `isNew`：createdAt 在 2 周内自动标记
- `widthMode`：控制工具页内容区最大宽度（default=640px / wide=960px / full=100%）
- `component`：`React.lazy(() => import(...))` 懒加载，减少首屏体积

### 2.2 工具箱首页

路由：`/`

**布局**（参考 it-tools Home.page.vue）：

```
┌─────────────────────────────────────────────┐
│  [搜索框]                                     │
├─────────────────────────────────────────────┤
│  收藏工具（可拖拽排序）                         │
│  ┌──────┐ ┌──────┐ ┌──────┐                │
│  │ 工具1 │ │ 工具2 │ │ 工具3 │                │
│  └──────┘ └──────┘ └──────┘                │
├─────────────────────────────────────────────┤
│  最近使用                                     │
│  ┌──────┐ ┌──────┐                          │
│  │ 工具4 │ │ 工具5 │                          │
│  └──────┘ └──────┘                          │
├─────────────────────────────────────────────┤
│  新工具（isNew 标记）                          │
│  ┌──────┐ ┌──────┐                          │
│  │ 工具6 │ │ 工具7 │                          │
│  └──────┘ └──────┘                          │
├─────────────────────────────────────────────┤
│  全部工具（按分类分组）                         │
│  加密  ┌──────┐ ┌──────┐                    │
│        │Hash  │ │UUID  │                    │
│        └──────┘ └──────┘                    │
│  转换器 ┌──────┐ ┌──────┐                    │
│        │Base64│ │颜色  │                    │
│        └──────┘ └──────┘                    │
└─────────────────────────────────────────────┘
```

**ToolCard 组件**（参考 it-tools ToolCard.vue）：

| 区域 | 内容 |
|------|------|
| 左上 | 工具图标（40px，neutral 色） |
| 右上 | isNew 徽章 + 收藏按钮 |
| 中部 | 工具名称（truncate） |
| 下部 | 工具描述（line-clamp-2） |

点击 → 路由跳转 `/tools/:id`

**收藏拖拽排序**：使用 `@dnd-kit/core` + `@dnd-kit/sortable`（React 生态标准），拖拽结束后更新 `toolStore.favoriteTools` 顺序。

### 2.3 工具页统一布局

路由：`/tools/:id`

**ToolPage 组件**（参考 it-tools tool.layout.vue）：

```
┌─────────────────────────────────────┐
│  工具标题 (h1, 28px, font-weight:500) │
│                        [收藏按钮]    │
│  ─── 分隔线 (200px, 2px, 20%透明度) ─│
│  工具描述 (14px, secondary 色)       │
│                                     │
│  ┌─────────────────────────────┐   │
│  │ 工具组件内容（懒加载）        │   │
│  │ max-width: widthMode 决定    │   │
│  └─────────────────────────────┘   │
└─────────────────────────────────────┘
```

| widthMode | max-width | 适用工具 |
|-----------|-----------|---------|
| `default` | 640px | 大多数工具（单栏输入/输出） |
| `wide` | 960px | Diff 对比、JSON 查看器等双栏工具 |
| `full` | 100% | 图片压缩等需要大画布的工具 |

### 2.4 侧边栏工具菜单

**CollapsibleToolMenu**（参考 it-tools CollapsibleToolMenu.vue）：

- 收藏工具作为独立分类置顶（`★ 收藏`）
- 各分类可折叠/展开，折叠状态 localStorage 持久化
- 折叠箭头 `▶`/`▼` 在分类名左侧
- 当前工具高亮（Ant Design Menu `selectedKeys`）
- 分类名右侧显示工具数量

**侧边栏结构**：

```
工具箱
  ★ 收藏
    Hash 计算
    UUID 生成
  加密 (3)
    Hash 计算
    UUID 生成
    Base64 编解码
  转换器 (4)
    颜色转换
    时间戳转换
    进制转换
    URL 编解码
  开发 (3)
    JSON 格式化
    正则测试器
    JWT 解码
  文本 (2)
    Diff 对比
    字数统计
──────────
v0.x.x
```

### 2.5 搜索系统

**模糊搜索**（参考 it-tools fuzzySearch.ts）：

- 库：`fuse.js`
- 搜索字段权重：`name` (权重2) > `keywords` (权重1.5) > `description` (权重1)
- 阈值：0.3
- 搜索范围：工具箱首页搜索框 + 命令面板

**命令面板**（参考 it-tools command-palette.vue）：

| 操作 | 快捷键 |
|------|--------|
| 唤起 | `Ctrl/Cmd + K`（主窗口内） |
| 关闭 | `Esc` |
| 上下选择 | `↑` / `↓` |
| 确认 | `Enter` |

搜索结果混合：
- 工具（category: 'Tools'）
- 操作（category: 'Actions'）：切换主题、随机工具
- 设置入口（category: 'Settings'）：Provider 管理、Prompt 模板

每个分类最多显示 5 条结果。

### 2.6 收藏系统

- 收藏状态：`toolStore.favoriteToolIds: string[]`，localStorage 持久化
- 添加/移除：ToolCard 和 ToolPage 上的收藏按钮
- 排序：工具箱首页拖拽排序
- 侧边栏：收藏工具作为独立分类置顶

### 2.7 工具使用历史

SQLite `tool_history` 表（已存在，I0 创建）：

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PK AUTOINCREMENT | 自增主键 |
| tool_id | TEXT NOT NULL | 工具 ID |
| used_at | INTEGER NOT NULL | 使用时间戳 |

**Rust 端 Tauri Commands**：

| command | 签名 | 说明 |
|---------|------|------|
| `record_tool_usage` | `(tool_id: String) → Result<()>` | 记录工具使用 |
| `get_recent_tools` | `(limit?: i32) → Result<Vec<ToolHistoryRow>>` | 获取最近使用 |
| `get_favorite_tools` | `() → Result<Vec<String>>` | 获取收藏工具 ID 列表 |
| `toggle_tool_favorite` | `(tool_id: String) → Result<bool>` | 切换收藏状态 |

> 收藏数据优先使用 Rust 端 SQLite 持久化（跨窗口共享），localStorage 仅作降级缓存。

### 2.8 I3 工具清单（12+ 工具）

| 分类 | 工具 | id | widthMode | 说明 |
|------|------|----|-----------|------|
| **加密** | Hash 计算 | `hash-text` | default | MD5/SHA1/SHA256/SHA512，输入文本→输出哈希 |
| **加密** | UUID 生成 | `uuid-generator` | default | v1/v4 生成 + 批量 + 复制 |
| **加密** | Base64 编解码 | `base64-string-converter` | default | 字符串↔Base64 互转 |
| **转换器** | 颜色转换 | `color-converter` | default | HEX↔RGB↔HSL↔RGBA 互转 + 色板预览 |
| **转换器** | 时间戳转换 | `date-time-converter` | default | Unix 时间戳↔可读日期互转 |
| **转换器** | 进制转换 | `integer-base-converter` | default | 2/8/10/16 进制互转 |
| **转换器** | URL 编解码 | `url-encoder` | default | URL encode/decode + 组件解析 |
| **开发** | JSON 格式化 | `json-viewer` | wide | JSON 格式化/压缩/验证 + 语法高亮 + 树形查看 |
| **开发** | 正则测试器 | `regex-tester` | default | 正则输入 + 测试文本 + 匹配高亮 + 捕获组 |
| **开发** | JWT 解码 | `jwt-parser` | default | JWT 解码→header+payload JSON 展示 |
| **文本** | Diff 对比 | `text-diff` | wide | 双栏文本输入 + diff 高亮 + 统一/拆分视图 |
| **文本** | 字数统计 | `text-statistics` | default | 字符数/单词数/行数/字节数 |

## 3. Rust 后端规格

### 3.1 新增依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| （无新增 crate） | — | tool_history / tool_favorites 表已在 I0 创建 |

### 3.2 storage CRUD 扩展

`tool_history.rs`（新模块）：

```rust
pub fn record_usage(conn: &Connection, tool_id: &str) -> Result<(), String>
pub fn get_recent_tools(conn: &Connection, limit: i32) -> Result<Vec<ToolHistoryRow>, String>
```

`tool_favorites.rs`（新模块）：

```rust
pub fn get_favorites(conn: &Connection) -> Result<Vec<String>, String>
pub fn toggle_favorite(conn: &Connection, tool_id: &str) -> Result<bool, String>
pub fn set_favorites_order(conn: &Connection, tool_ids: &[String]) -> Result<(), String>
```

### 3.3 Tauri Commands

| command | 签名 | 说明 |
|---------|------|------|
| `record_tool_usage` | `(app: AppHandle, tool_id: String) → Result<()>` | 记录使用 |
| `get_recent_tools` | `(app: AppHandle, limit: Option<i32>) → Result<Vec<Value>>` | 最近使用（默认 10） |
| `get_favorite_tools` | `(app: AppHandle) → Result<Vec<String>>` | 收藏列表 |
| `toggle_tool_favorite` | `(app: AppHandle, tool_id: String) → Result<bool>` | 切换收藏 |
| `set_favorite_tools_order` | `(app: AppHandle, tool_ids: Vec<String>) → Result<()>` | 收藏排序 |

## 4. 前端规格

### 4.1 新增前端依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| `fuse.js` | ^7 | 模糊搜索 |
| `@dnd-kit/core` | ^6 | 拖拽排序核心 |
| `@dnd-kit/sortable` | ^8 | 拖拽排序预设 |
| `@dnd-kit/utilities` | ^3 | 拖拽工具函数 |
| `date-fns` | ^3 | 日期计算（isNew 判断） |

### 4.2 新增组件

| 组件 | 路径 | 说明 |
|------|------|------|
| `ToolBox` | `src/tools/ToolBox.tsx` | 工具箱首页（搜索+收藏+分类+卡片网格） |
| `ToolPage` | `src/tools/ToolPage.tsx` | 工具页统一布局（标题+分隔线+描述+内容） |
| `ToolCard` | `src/tools/ToolCard.tsx` | 工具卡片（图标+名称+描述+收藏+isNew） |
| `FavoriteButton` | `src/tools/FavoriteButton.tsx` | 收藏按钮（心形图标+切换） |
| `CollapsibleToolMenu` | `src/tools/CollapsibleToolMenu.tsx` | 可折叠分类菜单 |
| `CommandPalette` | `src/tools/CommandPalette.tsx` | 命令面板（Ctrl/Cmd+K） |
| `ToolSearchBar` | `src/tools/ToolSearchBar.tsx` | 搜索框（fuse.js 驱动） |
| `registry` | `src/tools/registry.ts` | 工具注册表（defineTool + toolsByCategory） |

### 4.3 工具实现组件

每个工具独立目录：`src/tools/[tool-id]/index.tsx`

| 工具 | 路径 | 参考实现 |
|------|------|---------|
| Hash 计算 | `src/tools/hash-text/index.tsx` | it-tools hash-text.vue |
| UUID 生成 | `src/tools/uuid-generator/index.tsx` | it-tools uuid-generator.vue |
| Base64 编解码 | `src/tools/base64-string-converter/index.tsx` | it-tools base64-string-converter.vue |
| 颜色转换 | `src/tools/color-converter/index.tsx` | it-tools color-converter.vue + color-converter.models.ts |
| 时间戳转换 | `src/tools/date-time-converter/index.tsx` | it-tools date-time-converter.vue + models.ts |
| 进制转换 | `src/tools/integer-base-converter/index.tsx` | it-tools integer-base-converter.vue + model.ts |
| URL 编解码 | `src/tools/url-encoder/index.tsx` | it-tools url-encoder.vue |
| JSON 格式化 | `src/tools/json-viewer/index.tsx` | it-tools json-viewer.vue + json.models.ts |
| 正则测试器 | `src/tools/regex-tester/index.tsx` | it-tools regex-tester.vue |
| JWT 解码 | `src/tools/jwt-parser/index.tsx` | it-tools jwt-parser.vue + service.ts |
| Diff 对比 | `src/tools/text-diff/index.tsx` | it-tools text-diff.vue |
| 字数统计 | `src/tools/text-statistics/index.tsx` | it-tools text-statistics.vue |

### 4.4 Store 扩展

`toolStore.ts`（新建）：

```typescript
interface ToolState {
  tools: ToolDefinition[];
  toolsByCategory: ToolCategoryGroup[];
  favoriteToolIds: string[];
  recentTools: ToolHistoryRow[];
  searchQuery: string;
  searchResults: ToolDefinition[];

  loadFavorites: () => Promise<void>;
  toggleFavorite: (toolId: string) => Promise<void>;
  reorderFavorites: (toolIds: string[]) => Promise<void>;
  loadRecentTools: (limit?: number) => Promise<void>;
  recordUsage: (toolId: string) => Promise<void>;
  search: (query: string) => void;
}
```

- `tools`：从 `registry.ts` 静态导入，无需异步加载
- `toolsByCategory`：`tools` 按 `category` groupBy，按 `ToolCategoryMeta.order` 排序
- `favoriteToolIds`：Rust 端 SQLite 持久化，`loadFavorites()` 从 `get_favorite_tools` command 读取
- `search`：fuse.js 实例搜索，结果写入 `searchResults`

### 4.5 路由扩展

App.tsx `MainLayout` 路由新增：

- `/` → `ToolBox`（工具箱首页）
- `/tools/:id` → `ToolPage`（动态路由，根据 id 懒加载对应工具组件）

侧边栏更新：
- "工具箱" 菜单项改为 `CollapsibleToolMenu`（替换当前占位文字）
- 收藏工具置顶分类

### 4.6 常量新增

| 常量 | 值 | 说明 |
|------|-----|------|
| `TOOL_DEFAULT_MAX_WIDTH` | 640 | 默认工具内容区宽度 |
| `TOOL_WIDE_MAX_WIDTH` | 960 | 宽幅工具内容区宽度 |
| `TOOL_CARD_GAP` | 12 | 工具卡片间距 |
| `TOOL_SEARCH_THRESHOLD` | 0.3 | fuse.js 搜索阈值 |
| `TOOL_NEW_WEEKS` | 2 | 新工具标记周数 |
| `RECENT_TOOLS_LIMIT` | 10 | 最近使用工具数量 |
| `COMMAND_PALETTE_MAX_PER_CATEGORY` | 5 | 命令面板每分类最大结果数 |

## 5. 不做的事（I3 范围外）

- ❌ 日常工具（二维码/计算器/单位换算/密码生成/IP查询）（I4）
- ❌ 多媒体工具（图片压缩/SVG预览）（I4）
- ❌ 对话导出 Markdown/JSON（I2-History）
- ❌ SQL 格式化 / Cron 解析 / 进制转换增强（I4）
- ❌ i18n 多语言（后续全局迭代）

## 6. 验收标准

1. 主窗口工具箱首页可见 12 个工具卡片，按分类分组
2. 搜索框输入关键词，实时过滤工具列表
3. 点击工具卡片 → 跳转工具页 → 懒加载组件渲染
4. 工具页标题→分隔线→描述→内容层级正确
5. 收藏按钮可添加/移除，收藏工具在侧边栏和首页置顶
6. 收藏工具可拖拽排序
7. 最近使用工具在首页「最近使用」区域展示，数据来自 tool_history
8. 侧边栏分类可折叠/展开，折叠状态持久化
9. Ctrl/Cmd+K 唤起命令面板，搜索工具+操作+设置
10. 工具使用后 `tool_history` 表写入记录
11. `cargo check` ✅ / `tsc --noEmit` ✅ / `lint 0 errors` ✅
12. 12 个工具全部功能可用（输入→处理→输出→复制）
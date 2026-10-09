# Assistant - UI 风格文档

> 本文档定义 Assistant 的视觉风格体系，基于 **Ant Design 6 默认色板** 统一。
> 架构约束见 [ARCHITECTURE.md](./ARCHITECTURE.md)，前端组件规范见 [FRONTEND.md](./FRONTEND.md)。

---

## 1. 风格定位

**Ant Design 原生风格，零定制配色。** 不引入外部品牌色板，保持与 Ant Design 6 的视觉一致性，降低维护成本。

核心原则：
- 主色、中性色、语义色全部沿用 Ant Design 6 默认值
- CSS 变量层仅做 Ant Design token 的镜像 + 侧边栏/Orb 等自定义扩展
- 暗色模式由 `theme.darkAlgorithm` 自动派生，仅覆盖 3 个背景色
- 禁止在组件中硬编码色值，一律走 CSS 变量

---

## 2. 色板

### 2.1 主色

| 令牌 | 亮色 | 暗色 | 来源 |
|------|------|------|------|
| `colorPrimary` | `#1677ff` | `#1668dc` | Ant Design 6 默认 |
| `colorPrimaryHover` | `#4096ff` | `#3c8aff` | Ant Design 6 默认 |

### 2.2 背景与表面

| 令牌 | 亮色 | 暗色 | CSS 变量 | 来源 |
|------|------|------|---------|------|
| 页面布局 | `#f5f5f5` | `#0a0a0a` | `--color-bg-layout` | Ant `colorBgLayout` |
| 容器 | `#ffffff` | `#141414` | `--color-bg` | Ant `colorBgContainer` |
| 浮层 | `#ffffff` | `#1f1f1f` | `--color-bg-elevated` | Ant `colorBgElevated` |
| 侧边栏 | `#fafafa` | `#1a1a1a` | `--color-bg-sidebar` | 自定义扩展 |

### 2.3 文字

| 令牌 | 亮色 | 暗色 | CSS 变量 | 来源 |
|------|------|------|---------|------|
| 主文字 | `#1f1f1f` | `#f0f0f0` | `--color-text` | Ant `colorText` |
| 次文字 | `#666666` | `#a0a0a0` | `--color-text-secondary` | Ant `colorTextSecondary` |
| 弱文字 | `#999999` | `#666666` | `--color-text-tertiary` | Ant `colorTextTertiary` |

### 2.4 边框

| 令牌 | 亮色 | 暗色 | CSS 变量 | 来源 |
|------|------|------|---------|------|
| 主边框 | `#e8e8e8` | `#303030` | `--color-border` | Ant `colorBorder` |
| 次边框 | `#f0f0f0` | `#262626` | `--color-border-secondary` | Ant `colorBorderSecondary` |

### 2.5 阴影

| 令牌 | 亮色 | 暗色 | CSS 变量 | 来源 |
|------|------|------|---------|------|
| 基础阴影 | `rgba(0,0,0,0.06)` | `rgba(0,0,0,0.2)` | `--color-shadow` | Ant `boxShadow` |
| 次阴影 | `rgba(0,0,0,0.04)` | `rgba(0,0,0,0.12)` | `--color-shadow-secondary` | Ant `boxShadowSecondary` |
| Orb 球体阴影 | `0 2px 8px rgba(0,0,0,0.3)` | `0 2px 8px rgba(0,0,0,0.5)` | `--shadow-orb` | 自定义扩展 |
| Orb 面板阴影 | `0 4px 24px rgba(0,0,0,0.15)` | `0 4px 24px rgba(0,0,0,0.4)` | `--shadow-panel` | 自定义扩展 |

### 2.6 Orb 专属

| 令牌 | 亮色 | 暗色 | CSS 变量 | 说明 |
|------|------|------|---------|------|
| 渐变起始 | `#6ea8fe` | `#4d8cd8` | `--color-orb-gradient-start` | 球体径向渐变亮端 |
| 渐变终止 | `#3b82f6` | `#2563eb` | `--color-orb-gradient-end` | 球体径向渐变暗端 |

> Orb 渐变色基于 `colorPrimary` 的亮/暗变体手动选取，未来可改为从 Ant Design token 动态计算。

---

## 3. 圆角与间距

### 3.1 圆角

| 令牌 | 值 | CSS 变量 | 用途 |
|------|-----|---------|------|
| 基础圆角 | `6px` | `--radius` | 按钮、输入框、卡片、菜单 |
| 大圆角 | `12px` | `--radius-lg` | Orb 面板、弹窗、大卡片 |
| 圆形 | `50%` | — | Orb 球体、头像 |

### 3.2 间距

| 令牌 | 值 | CSS 变量 | 用途 |
|------|-----|---------|------|
| 侧边栏宽度 | `240px` | `--sidebar-width` | 展开态 |
| 侧边栏折叠宽度 | `64px` | `--sidebar-collapsed-width` | 折叠态（仅图标） |
| 标题栏高度 | `38px` | `--titlebar-height` | 自定义标题栏 |
| Orb 球体尺寸 | `64px` | `--orb-ball-size` | 球体渲染大小 |
| Orb 命中框尺寸 | `48px` | `--orb-hitbox-size` | 可点击区域 |
| Orb 面板内边距 | `12px` | `--orb-chrome-inset` | 面板与窗口边距 |
| Orb 面板头高度 | `36px` | `--orb-header-height` | 面板顶部拖拽栏 |

组件内间距遵循 Ant Design 6 默认 token（padding, margin 等），不额外定义。

---

## 4. 字体

沿用 Ant Design 6 默认字体栈：

```css
font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto,
  "Helvetica Neue", Arial, "Noto Sans", sans-serif;
```

| 属性 | 值 |
|------|-----|
| 基础字号 | `14px` |
| 行高 | `1.5` |
| 渲染优化 | `-webkit-font-smoothing: antialiased` |

> 不引入自定义字体（Geist / Inter / CursorGothic 等），保持系统原生渲染，零字体加载开销。

---

## 5. 双层变量体系

### 5.1 架构

```
Ant Design ConfigProvider (theme.ts)
  ↓ 注入 CSS 变量模式 (--ant-*)
  ↓
global.css (:root / [data-theme="dark"])
  ↓ 定义 --color-* 镜像变量 + 自定义扩展
  ↓
组件 inline style → 引用 --color-* / --shadow-* / --radius-*
```

### 5.2 变量命名规范

| 前缀 | 范围 | 示例 |
|------|------|------|
| `--color-` | 颜色 | `--color-bg`, `--color-primary`, `--color-text-secondary` |
| `--shadow-` | 完整阴影值 | `--shadow-orb`, `--shadow-panel` |
| `--radius` | 圆角 | `--radius`, `--radius-lg` |
| `--sidebar-` | 侧边栏布局 | `--sidebar-width`, `--sidebar-collapsed-width` |
| `--titlebar-` | 标题栏布局 | `--titlebar-height` |
| `--orb-` | Orb 布局 | `--orb-ball-size`, `--orb-hitbox-size`, `--orb-chrome-inset`, `--orb-header-height` |

### 5.3 禁止事项

- ❌ 组件中硬编码 `#xxxxxx` 色值
- ❌ 组件中硬编码 `rgba()` 阴影值
- ❌ 使用 `--ant-*` 变量 + 硬编码 fallback（如 `var(--ant-color-bg, #fff)`）
- ❌ 在 `:root` 外定义主题变量

---

## 6. 主题切换

三态循环：`system → light → dark → system`

| 模式 | 行为 |
|------|------|
| `system` | 读取 Tauri `win.theme()` + 监听 `onThemeChanged` |
| `light` | 强制亮色，`[data-theme="light"]` |
| `dark` | 强制暗色，`[data-theme="dark"]` |

切换时 `invoke('set_theme', { theme })` 同步 Rust 端 `AppConf.theme` 持久化。

---

## 7. 组件样式索引

| 组件 | 样式来源 | 变量引用 |
|------|---------|---------|
| Titlebar | inline style | `--color-bg`, `--color-border`, `--color-text-secondary` |
| WindowControls | Ant `Button` type="text" | Ant token |
| ThemeToggle | Ant `Button` type="text" | Ant token |
| Layout.Sider | inline style | `--color-bg-sidebar`, `--color-border` |
| Layout.Content | inline style | `--color-bg-layout`, `--color-bg` |
| OrbBall | CSS 类 `.orb-hitbox` + 状态类 | `--color-orb-gradient-start/end`, `--filter-orb`, `--orb-hitbox-size` |
| OrbPanel | CSS 类 `.orb-panel` + `.orb-compact` | `--color-bg`, `--radius-lg`, `--shadow-panel`, `--color-border-secondary`, `--orb-chrome-inset`, `--orb-header-height` |
| OrbPanelHeader | CSS 类 `.orb-panel-header` | `--orb-header-height`, `--color-border-secondary` |
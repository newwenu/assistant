# Assistant - 前端设计文档

> 本文档定义前端 UI 设计方向、参考项目分析、组件规范与视觉体系。
> 架构约束与接口规范见 [ARCHITECTURE.md](./ARCHITECTURE.md)。

---

## 1. 设计立场

**不照搬任何单一项目的 UI。** 参考多个成熟项目的审美与交互模式，融合后自行设计。

核心原则：
- 布局骨架和交互模式可借鉴，**配色/圆角/间距/字体/图标全部自行定义**
- 工具箱区域追求信息密度，Mascot 区域允许视觉表现力
- 主题系统由 Ant Design ConfigProvider + CSS 变量双层驱动

---

## 2. 参考项目 UI 分析

### 2.1 it-tools（Vue 3 + Naive UI）— 工具箱 UI 典范

**仓库**：references/it-tools | Stars: 40,750

#### 布局结构

```
┌──────────────────────────────────────────────┐
│  Hero 区域（Logo + 渐变背景 + 副标题）          │
├────────┬─────────────────────────────────────┤
│        │  [菜单折叠] [首页] [命令面板] [语言]    │
│  侧边栏 ├─────────────────────────────────────┤
│        │                                     │
│  收藏   │  工具内容区                           │
│  ────   │  ┌─────────────────────────────┐   │
│  加密   │  │ 工具标题      [收藏按钮]      │   │
│  转换器  │  │ ─── 分隔线 ───              │   │
│  编解码  │  │ 工具描述                      │   │
│  网络   │  │                              │   │
│  文本   │  │ 工具组件内容（max-width:600px） │   │
│  数学   │  └─────────────────────────────┘   │
│  数据   │                                     │
│        │  版本号 © 作者                        │
└────────┴─────────────────────────────────────┘
```

#### 关键 UI 特性

| 特性 | 实现方式 | 参考文件 |
|------|---------|---------|
| **可折叠分类菜单** | `CollapsibleToolMenu`：分类名左侧 `▶` 箭头，点击折叠/展开，折叠状态 localStorage 持久化 | [CollapsibleToolMenu.vue](../../references/it-tools/src/components/CollapsibleToolMenu.vue) |
| **收藏工具置顶** | `favoriteTools` 数组作为独立分类插入 `toolsByCategory` 最前 | [base.layout.vue](../../references/it-tools/src/layouts/base.layout.vue) L29-32 |
| **工具页统一布局** | `tool.layout.vue`：标题(h1, 40px, font-weight:400) + 收藏按钮 + 分隔线(200px宽, 2px高, 20%透明度) + 描述 + 内容(max-width:600px居中) | [tool.layout.vue](../../references/it-tools/src/layouts/tool.layout.vue) |
| **命令面板** | Ctrl/Cmd+K 唤起模态框，工具+操作混合搜索，↑↓+Enter 键盘导航 | [command-palette.vue](../../references/it-tools/src/modules/command-palette/command-palette.vue) |
| **Hero 渐变背景** | SVG 渐变 + 绝对定位文字叠加，侧边栏顶部装饰 | base.layout.vue L56-75 |
| **侧边栏折叠** | `isMenuCollapsed` 状态控制，汉堡菜单切换 | base.layout.vue L93-100 |
| **版本号页脚** | 侧边栏底部居中，链接到 GitHub commit | base.layout.vue L107-121 |

#### 借鉴方向

- ✅ 可折叠分类菜单 + 收藏置顶的交互模式
- ✅ 工具页标题→分隔线→描述→内容的层级结构
- ✅ 命令面板（Ctrl/Cmd+K）全局搜索
- ❌ 不采用 Hero 渐变背景（桌面应用不需要 Web 首屏装饰）
- ❌ 不采用 Naive UI（我们用 Ant Design 6）
- ❌ 工具内容 max-width:600px 对部分工具（如 Diff 对比）过窄，需按工具自适应

---

### 2.2 ChatGPT (lencx)（React + Tauri 2）— 标题栏 + 主题 + 多窗口

**仓库**：references/ChatGPT | Stars: 54,585

#### 多窗口分流

```typescript
// App.tsx — 根据 webview label 渲染不同组件
const viewMap = {
  titlebar: <Titlebar />,
  ask: <Ask />,
  settings: <Settings />,
};
export default function App() {
  const webview = getCurrentWebview();
  return viewMap[webview.label as keyof typeof viewMap] || null;
}
```

**参考文件**：[App.tsx](../../references/ChatGPT/src/App.tsx)

#### 自定义标题栏

| 特性 | 实现 |
|------|------|
| **拖拽区域** | `data-tauri-drag-region` 属性标记整个标题栏 |
| **macOS 间距** | 非全屏时 `pl-[80px]` 避让红绿灯按钮 |
| **主题三态切换** | system → light → dark 循环，`invoke('set_theme', { theme })` 同步 Rust |
| **图标切换** | `ThemeSystem` / `ThemeLight` / `ThemeDark` 三个图标组件 |
| **置顶切换** | Pin/UnPin 图标，`invoke('window_pin', { pin })` |
| **设置入口** | Setting 图标，`invoke('open_settings')` |
| **URL 显示** | hostname 胶囊标签 + 外链图标，点击 `open(url)` |
| **导航控制** | 前进/后退/刷新按钮 |

**参考文件**：[Titlebar.tsx](../../references/ChatGPT/src/view/Titlebar.tsx)

#### 窗口控制按钮

```typescript
// WinTitlebar.tsx — 独立窗口控制组件
const [isMax, setMax] = useState(false);
// Window.getByLabel('core') 获取窗口实例
// win?.toggleMaximize() / win?.minimize() / win?.close()
```

**参考文件**：[WinTitlebar.tsx](../../references/ChatGPT/src/components/WinTitlebar.tsx)

#### 主题系统

```typescript
// useTheme.tsx — Tauri 原生窗口主题 API
const [theme, setTheme] = useState<string | null>('light');
useEffect(() => {
  let unlisten: Function;
  (async () => {
    let win = getCurrentWindow();
    setTheme(await win.theme() || '');
    unlisten = await win.onThemeChanged(({ payload: newTheme }) => {
      setTheme(newTheme);
    });
  })()
  return () => { unlisten?.(); };
}, [])
```

**参考文件**：[useTheme.tsx](../../references/ChatGPT/src/hooks/useTheme.tsx)

#### 借鉴方向

- ✅ 多窗口分流模式（`webview.label` → 组件映射）
- ✅ 自定义标题栏 + `data-tauri-drag-region` + macOS 红绿灯避让
- ✅ 主题三态循环切换 UI + Tauri 原生 `win.theme()` / `onThemeChanged()`
- ✅ 窗口控制按钮组件（最小化/最大化/关闭）
- ✅ `vite-tsconfig-paths` 路径别名方案
- ❌ 不采用其 URL 导航栏（我们是本地应用，不需要浏览器式 URL 栏）

---

### 2.3 OpenBitFun（React + Tauri + 自研 UI）— 设计系统 + 启动流程 + 桌面宠物

**仓库**：references/OpenBitFun | Stars: 2,372

#### 设计系统 Provider

```typescript
// OpenBitFunDesignSystemProvider.tsx
// 产品级 UI 宿主，注入外观、语言、设计令牌
<DesignSystemProvider
  colorScheme={colorScheme}    // 'light' | 'dark'
  contrast={contrast}          // 'standard' | 'high'
  density="compact"            // 固定紧凑密度
  locale={currentLanguage}
  nativeTooltipPolicy="application"
  messages={{ ... }}           // i18n 消息
  portalHost={getAppearanceOverlayHost}
>
```

同时在 `<html>` 上设置数据属性驱动 CSS：

```
data-openbitfun-design-system-root
data-color-scheme="light" | "dark"
data-contrast="standard" | "high"
data-density="compact"
```

**参考文件**：[OpenBitFunDesignSystemProvider.tsx](../../references/OpenBitFun/src/web-ui/src/infrastructure/design-system/OpenBitFunDesignSystemProvider.tsx)

#### 启动流程

```
main.tsx
  ├─ registerGlobalErrorHandlers()     // 全局错误捕获（白屏检测 + React minified error）
  ├─ installInteractionModalityTracking() // 动效偏好追踪
  ├─ initializeBeforeRender()          // 日志 + 外观初始化（阻塞首帧）
  ├─ ReactDOM.createRoot().render(
  │    <AppErrorBoundary>
  │      <I18nProvider>
  │        <OpenBitFunDesignSystemProvider>
  │          <WindowContent />
  │        </OpenBitFunDesignSystemProvider>
  │      </I18nProvider>
  │    </AppErrorBoundary>
  │  )
  └─ scheduleAfterStartupSignal(() =>
       initializeAfterRender()         // 非关键初始化（字体/配置/工具/右键菜单）延迟到首帧后
     )
```

**参考文件**：[main.tsx](../../references/OpenBitFun/src/web-ui/src/main.tsx)

#### 桌面宠物情绪外观

- `data-openbitfun-mood` 属性驱动 CSS：rest / analyzing / waiting / working / hover / dragging
- `AgentCompanionDesktopPet.tsx`：96px 宠物 + 8px 拖拽阈值 + 4px 边缘缓冲 + 指针悬停检测 + 右键菜单 + 气泡 typewriter 输出

#### 借鉴方向

- ✅ 设计系统 Provider 模式（colorScheme + contrast + density + locale）
- ✅ `<html>` 数据属性驱动 CSS 变量
- ✅ 启动流程分层：beforeRender（阻塞）→ render → afterRender（延迟）
- ✅ 全局错误边界 + 白屏检测
- ✅ Tab 键禁用全局焦点遍历（IDE 风格），仅在 Monaco/xterm/Modal 内允许
- ✅ 桌面宠物 mood → CSS 驱动外观
- ❌ 不采用其自研 `@openbitfun/ui` 组件库（我们用 Ant Design 6）
- ❌ 不采用其 CSS Layers 排序（Ant Design 6 有自己的样式注入机制）

---

### 2.4 openhuman（Next.js + Rive）— Mascot 视觉表现力

**仓库**：references/openhuman | Stars: 40,468

#### Rive Mascot 组件

```typescript
// RiveMascot.tsx — 核心渲染组件
const { rive, RiveComponent } = useRive({
  ...riveSource,           // buffer 或 src
  stateMachines: 'MascotSM',
  autoplay: true,
  layout: new Layout({ fit: Fit.Contain }),
});
const viewModel = useViewModel(rive, { useDefault: true });
const vmInstance = useViewModelInstance(viewModel, { useDefault: true, rive });
const { setValue: setPose } = useViewModelInstanceEnum('pose', vmInstance);
const { setValue: setMouthVisemeCode } = useViewModelInstanceEnum('mouthVisemeCode', vmInstance);
const { setValue: setPrimaryColor } = useViewModelInstanceColor('primaryColor', vmInstance);
const { setValue: setSecondaryColor } = useViewModelInstanceColor('secondaryColor', vmInstance);
```

**参考文件**：[RiveMascot.tsx](../../references/openhuman/app/src/features/human/Mascot/RiveMascot.tsx)

#### 表情系统

- **18 种 MascotFace**：sleep / idle / listening / thinking / confused / speaking / happy / concerned / curious / proud / cautious / celebrating / writing / reading / waving / dancing / drinking_coffee / drinking_boba
- **9 种 RivePose**：idle / thinking / celebration / bookreading / coffeedrink / writing / bobbateadrink / hand_wave / dancing
- **映射表 FACE_TO_POSE**：18 → 9，如 `curious→bookreading`、`proud→celebration`
- **口型 8 种**：REST / A / E / I / O / U / M / F，每个 `{openness, width}`
- **Rive viseme 15 种**：Oculus 标准集 + 别名归一化（不同 TTS 大小写不一致）

**参考文件**：
- [Ghosty.tsx](../../references/openhuman/app/src/features/human/Mascot/Ghosty.tsx) — SVG 表情参数化（eyeScaleY/X, browTilt, browDy, blushOpacity）
- [riveMaps.ts](../../references/openhuman/app/src/features/human/Mascot/riveMaps.ts) — 映射表 + 归一化
- [visemes.ts](../../references/openhuman/app/src/features/human/Mascot/visemes.ts) — 口型插值 + SVG 路径生成

#### 空闲动画轮转

```
idle 6~12s → 随机 ambient pose（thinking/bookreading/coffeedrink/...）
  → 保持 2.5~5s → 回到 idle → 重复
```

任何真实活动（非 idle）立即中断轮转，活动结束后恢复。

#### 借鉴方向

- ✅ Rive 运行时集成方式（useRive + useViewModel + useViewModelInstanceEnum/Color）
- ✅ 表情→pose 映射表设计（多对一，表情语义丰富，动画状态精简）
- ✅ 口型系统（openness/width 二维 + lerpViseme 插值）
- ✅ 空闲动画轮转（随机间隔 + 随机 pose + 活动中断）
- ✅ SVG Ghosty 表情参数化（眼/眉/腮独立参数，可脱离 Rive 独立渲染）
- ❌ 不采用 Next.js 路由（我们是 Tauri 桌面应用）

---

### 2.5 WindowPet（React + Mantine + Tauri 2）— 双窗口 + 宠物渲染

**仓库**：references/WindowPet | Stars: 670

#### 双窗口路由

```typescript
// App.tsx — BrowserRouter + React.lazy 懒加载
<Router>
  <Routes>
    <Route path="/" element={<PhaserWrapper />} />        // 主窗口：Phaser 宠物渲染
    <Route path="/setting" element={
      <Suspense fallback={<Loading />}>
        <MantineProvider defaultColorScheme="dark" theme={{...}}>
          <SettingWindow />
        </MantineProvider>
      </Suspense>
    } />
  </Routes>
</Router>
```

**参考文件**：[App.tsx](../../references/WindowPet/src/App.tsx)

#### 入口配置

```typescript
// main.tsx
document.addEventListener('contextmenu', (e) => e.preventDefault()); // 禁用右键
ReactDOM.createRoot(document.getElementById("root")).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <I18nextProvider i18n={i18next}>
        <App />
      </I18nextProvider>
    </QueryClientProvider>
  </React.StrictMode>
);
```

**参考文件**：[main.tsx](../../references/WindowPet/src/main.tsx)

#### 借鉴方向

- ✅ React.lazy + Suspense 懒加载设置窗口
- ✅ MantineProvider 主题配置结构（dark 默认 + 自定义 colors + primaryColor）
- ✅ QueryClientProvider + I18nextProvider 包裹顺序
- ✅ 禁用右键菜单（`contextmenu` preventDefault）
- ❌ 不采用 Phaser 渲染宠物（我们用 Rive）
- ❌ 不采用 Mantine UI（我们用 Ant Design 6）

---

## 3. 视觉体系

### 3.1 主题系统

**三态**：亮 / 暗 / 跟随系统

**双层驱动**：

| 层 | 机制 | 职责 |
|----|------|------|
| **Ant Design 层** | `ConfigProvider` + `theme.algorithm` | 组件级主题令牌（colorPrimary, borderRadius, fontSize 等） |
| **CSS 变量层** | `:root` / `[data-theme="dark"]` | 全局变量（背景、边框、阴影等），供非 Ant Design 组件使用 |

**Tauri 原生主题 API**（参考 ChatGPT useTheme.tsx）：

```typescript
import { getCurrentWindow } from '@tauri-apps/api/window';

const win = getCurrentWindow();
const systemTheme = await win.theme();           // 'light' | 'dark'
const unlisten = await win.onThemeChanged(({ payload }) => {
  // 系统主题变化时实时更新
});
```

**主题切换 UI**（参考 ChatGPT Titlebar.tsx L94-106）：

```
system → light → dark → system（三态循环）
```

每次切换 `invoke('set_theme', { theme })` 同步到 Rust 端 `AppConf.theme` 持久化。

### 3.2 自定义标题栏

**方案**：Tauri `decorations: false` + 自绘标题栏（参考 ChatGPT WinTitlebar.tsx + Titlebar.tsx）

| 区域 | 内容 |
|------|------|
| 左侧 | macOS 红绿灯避让区（80px） / Windows 无避让 |
| 中间 | `data-tauri-drag-region` 拖拽区 |
| 右侧 | 主题切换 + 置顶切换 + 设置入口 + 窗口控制（最小化/最大化/关闭） |

### 3.3 设计令牌

基于 Ant Design 6 的 `theme.token` 体系，在其上定制：

```typescript
const commonToken = {
  borderRadius: 6,
  fontSize: 14,
  colorPrimary: '#1677ff',       // Ant Design 6 默认蓝
};

const lightTheme = {
  algorithm: theme.defaultAlgorithm,
  token: {
    ...commonToken,
    colorBgLayout: '#f5f5f5',     // 布局背景
    colorBgContainer: '#ffffff',   // 容器背景
    colorBgElevated: '#ffffff',    // 浮层背景
  },
  components: {
    Menu: { itemHeight: 36, itemMarginBlock: 2 },
    Button: { controlHeight: 32 },
    Input: { controlHeight: 32 },
  },
};

const darkTheme = {
  algorithm: theme.darkAlgorithm,
  token: {
    ...commonToken,
    colorPrimary: '#1668dc',       // 暗色主色（自动派生）
    colorBgLayout: '#0a0a0a',      // 布局背景
    colorBgContainer: '#141414',    // 容器背景
    colorBgElevated: '#1f1f1f',     // 浮层背景
  },
  components: { ...lightTheme.components },
};
```

> 完整 CSS 变量体系见 [DESIGN.md](./DESIGN.md)

### 3.4 Mascot 外观驱动

**Rive 路径**（参考 openhuman RiveMascot.tsx）：

```
对话事件 → MascotFace → faceToPose() → RivePose → setPose()
TTS 输出 → visemeCode → toRiveVisemeCode() → setMouthVisemeCode()
主题色 → primaryColor / secondaryColor → setPrimaryColor() / setSecondaryColor()
```

**CSS 路径**（参考 OpenBitFun mood facet）：

```
Agent 状态 → mood（rest/analyzing/waiting/working）
  → data-mood 属性 → CSS 选择器驱动外观
```

两条路径共存：Rive 控制动画，CSS 控制非动画元素（光晕/阴影/边框等）。

---

## 4. 布局规范

### 4.1 主窗口布局

```
┌──────────────────────────────────────────────┐
│  自定义标题栏（拖拽区 + 主题 + 置顶 + 设置 + 窗口控制）│
├────────┬─────────────────────────────────────┤
│        │                                     │
│  侧边栏 │           主内容区                   │
│ 240px  │           flex: 1                   │
│        │                                     │
│  ·工具箱 │  ·工具分类列表 / 具体工具界面         │
│  ·设置   │  ·设置页面                          │
│        │                                     │
└────────┴─────────────────────────────────────┘
```

- 侧边栏可折叠（折叠后仅图标，64px）
- 侧边栏内容：Logo 区 + 可折叠分类菜单（参考 it-tools CollapsibleToolMenu）+ 底部版本号

### 4.2 工具页布局

参考 it-tools tool.layout.vue 的层级结构，调整尺寸：

```
┌─────────────────────────────────────┐
│  工具标题 (h1)          [收藏按钮]    │
│  ─── 分隔线 ───                     │
│  工具描述                            │
│                                     │
│  工具组件内容                         │
│  （max-width 按工具类型自适应：        │
│   默认 640px，宽幅工具 960px，         │
│   全幅工具 100%）                     │
└─────────────────────────────────────┘
```

### 4.3 Orb 悬浮球布局

```
 收起态 (64×64)          展开态 (420×600)
 ┌──────┐               ┌──────────────────────┐
 │      │               │  Mascot (Rive, 80px) │
 │  ●   │               ├──────────────────────┤
 │      │               │  消息列表（滚动）      │
 └──────┘               ├──────────────────────┤
                         │  Agent 模式 | 模型选择 │
                         │  [输入框] [发送]       │
                         └──────────────────────┘
```

---

## 5. 交互规范

### 5.1 命令面板

参考 it-tools command-palette.vue：

| 操作 | 快捷键 |
|------|--------|
| 唤起 | `Ctrl/Cmd + K` |
| 关闭 | `Esc` |
| 上下选择 | `↑` / `↓` |
| 确认 | `Enter` |
| 搜索 | 实时模糊搜索（fuse.js），工具名 + 关键词 + 描述 |

搜索结果混合：工具 + 最近操作 + 设置入口

### 5.2 侧边栏菜单

参考 it-tools CollapsibleToolMenu.vue：

- 收藏工具作为独立分类置顶
- 各分类可折叠/展开，折叠状态 localStorage 持久化
- 当前工具高亮
- 折叠箭头 `▶`/`▼` 在分类名左侧

### 5.3 主题切换

参考 ChatGPT Titlebar.tsx：

- 标题栏右侧图标，三态循环点击：system → light → dark → system
- 每次切换 invoke Rust 端持久化
- 跟随系统时，监听 `win.onThemeChanged()` 实时响应

### 5.4 Orb 悬浮球交互

参考 OpenBitFun AgentCompanionDesktopPet.tsx：

| 事件 | 行为 |
|------|------|
| 单击（移动 <8px） | 展开 Orb 对话面板 |
| 拖拽（移动 ≥8px） | 自由移动窗口 |
| 松手 | 边缘吸附（距边缘 <4px 时贴边） |
| 面板失焦 | 自动收起为小球 |
| `Cmd/Ctrl+K` | 全局快捷键唤起/隐藏 |
| 右键 | 菜单：新建对话 / 打开主窗口 / 退出 |

---

## 6. 架构约束

> 前端目录结构、Provider 包裹顺序、多窗口分流等架构规范见 [ARCHITECTURE.md](./ARCHITECTURE.md) Section 7。

---

## 7. 迭代 → 前端参考映射

| 迭代 | 前端任务 | 主要参考 | 关键参考文件 |
|------|---------|---------|------------|
| **I0 脚手架** | 主题系统 + 路由 + ESLint + 入口 | ChatGPT + WindowPet | `useTheme.tsx`（Tauri 原生主题 API）、`App.tsx`（多窗口分流）、`main.tsx`（Provider 包裹）、`tsconfig.json`（路径别名） |
| **I1 悬浮球** | OrbBall + OrbPanel + 拖拽 + 展开/收起 | OpenBitFun + ChatGPT | `AgentCompanionDesktopPet.tsx`（拖拽+阈值+边缘缓冲）、`Titlebar.tsx`（窗口主题监听） |
| **I2 Orb 对话** | ChatView + 流式渲染 + Provider UI | OpenBitFun + ChatGPT + openhuman | `aiService.ts`（Provider 适配）、`Ask.tsx`（对话输入）、`chatDeltaCoalescer.ts`（流式合并） |
| **I3 工具箱** | ToolBox + 注册表 + 搜索 + 收藏 | it-tools | `tools.types.ts`（Tool 接口）、`tools.store.ts`（分类+收藏）、`fuzzySearch.ts`（fuse.js）、`command-palette.vue`（命令面板）、`CollapsibleToolMenu.vue`（折叠菜单） |
| **I4 工具扩展** | 更多工具 | it-tools | 60+ 工具实现逻辑（Vue→React 迁移） |
| **I5 Mascot** | Rive 集成 + 表情状态机 | openhuman | `RiveMascot.tsx`（useRive+ViewModel）、`Ghosty.tsx`（18种表情参数）、`riveMaps.ts`（映射+归一化）、`visemes.ts`（口型插值） |
| **I6 Agent** | 模式切换 UI + token 追踪 | OpenBitFun | `agentCompanionMood.ts`（mood 派生）、`chatInputMode.ts`（模式切换） |
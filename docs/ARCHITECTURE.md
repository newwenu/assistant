# Assistant - 工程设计文档

## 1. 架构总览

**Assistant** 是一款桌面端智能助手应用，以 **Orb 悬浮球** 为 AI 交互入口，以**主窗口**为工具与设置中心。

| 模块 | 职责 | 交互方式 |
|------|------|---------|
| **Orb 悬浮球** | AI 对话、Agent 工具调用、Mascot 形象 | 桌面常驻，点击展开对话面板，失焦收起 |
| **主窗口** | 工具箱、设置管理 | 从托盘/悬浮球菜单打开，标准窗口 |

> **主窗口不提供 AI 对话功能**，所有 AI 交互通过 Orb 完成。主窗口专注工具与配置。

---

## 2. 技术栈

| 层级 | 选型 | 说明 |
|------|------|------|
| 框架 | **Tauri 2** | Rust 后端 + Web 前端，包体小 10x+、内存低 50%+ |
| 后端 | Rust | 窗口管理 / Provider 适配 / Agent 运行时 / 本地存储 |
| 前端 | React 19 + TypeScript 6 | UI 构建 |
| 动画 | Rive | 矢量动画引擎，WebGL2 渲染，内置状态机 |
| UI 库 | Ant Design 6 | 组件库 |
| 状态 | Zustand | 轻量级状态管理 |
| 构建 | Vite | 快速构建 |
| 存储 | SQLite (rusqlite) | 本地持久化 |

---

## 3. Orb 悬浮球

Orb 是 AI 交互核心——桌面常驻透明小球，展开后为 AI 对话面板。

### 3.1 窗口配置

```json
{
  "label": "orb",
  "width": 88,
  "height": 88,
  "transparent": true,
  "alwaysOnTop": true,
  "skipTaskbar": true,
  "decorations": false
}
```

初始窗口 88×88（ORB_COLLAPSED_SIZE 64 + 2×ORB_CHROME_INSET 12）。展开时 Rust 端 reposition + resize 至 444×624（ORB_EXPANDED_WIDTH 420 + 2×12, ORB_EXPANDED_HEIGHT 600 + 2×12），CSS `transform: scale()` + `opacity` + `pointer-events` 做 300ms 视觉过渡动画。

**点击穿透机制**：透明区域通过 Tauri `set_ignore_cursor_events(true)` 实现系统级穿透，让鼠标事件传递到下方窗口。悬浮球可见区域（hitbox 48×48）通过光标位置轮询检测，仅在鼠标进入 hitbox 时关闭穿透。坐标系统一使用双约定：**前端逻辑像素**（`OrbCoord.ts` 的 `physicalToLogical`/`logicalToPhysical` 转换）+ **后端物理像素**（`outer_position()`/`outer_size()`/`available_monitors()` 均为物理像素），`WindowBounds` 全部使用物理像素，`ORB_EXPANDED_WIDTH/HEIGHT` 逻辑常量在传入后端时需乘以 `scale_factor` 转换。`getWindowLogicalRect()` 统一使用 `outerPosition()` + `outerSize()` 避免外框/内容区混用。

| 状态 | click-through | 说明 |
|------|--------------|------|
| 收起 + 鼠标在 hitbox 外 | ON | 事件穿透到下方窗口 |
| 收起 + 鼠标在 hitbox 上 | OFF | hitbox 接收点击/拖拽 |
| 展开 + 鼠标在窗口内 | OFF | 面板接收交互 |
| 展开 + 鼠标在窗口外 | ON | 事件穿透 |
| 拖拽中 | OFF | 窗口接收指针事件 |

**穿透实现（按平台）**：

| 平台 | 机制 | 说明 |
|------|------|------|
| Windows | CSS `pointer-events` 精细控制 + Rust 辅助 | 容器 `pointer-events: none`（穿透），`.orb-hitbox` / `.orb-panel` 设 `auto`（交互）；同时用 Rust `set_ignore_cursor_events` 配合光标轮询 |
| macOS | Rust `set_ignore_cursor_events` + 光标轮询 | CSS `pointer-events` 无法穿透到系统级，需 Rust 端控制 |

**穿透优化**：
- **P0 状态缓存**：Rust `AtomicBool` + 前端 `orbStore.clickThrough`，值相同时跳过系统调用
- **P1 穿透防抖**：`CLICK_THROUGH_DEBOUNCE_MS = 30`，开启穿透延迟 30ms，关闭穿透立即执行
- **P1 手抖容差**：`HAND_JITTER_PX = 8`，鼠标离开 hitbox 后 8px 内仍保持不穿透
- **P2 轮询间隔**：前端 `HOVER_POLL_INTERVAL_MS = 80`，Rust `HOVER_POLL_INTERVAL_MS = 120`

### 3.2 交互行为

| 事件 | 行为 |
|------|------|
| 单击悬浮球 | 展开 Orb 对话面板（方向自适应：根据小球在屏幕位置决定左/右 + 上/下展开） |
| 面板失焦 | 自动收起为小球 |
| 拖拽 | 自由移动，松手边缘吸附 + 位置持久化（`save_orb_position`） |
| `Cmd/Ctrl+K` | 全局快捷键唤起/隐藏 |
| 右键 | 菜单：新建对话 / 打开主窗口 / 退出 |

### 3.3 Orb 对话面板

展开后的面板包含：

- **Mascot 形象**：Rive 动画角色，表情随对话状态联动
- **对话区**：消息列表 + 流式输出渲染
- **输入区**：文本输入 + 模型选择 + 发送
- **Agent 模式切换**：Quick / Standard / Deep

### 3.4 Mascot 表情状态机

| 对话事件 | 表情 |
|---------|------|
| 闲置 | idle |
| 用户输入 | listening |
| 等待首 token | thinking |
| 流式输出中 | speaking |
| 输出完成 | happy → idle |
| 错误 | concerned |

情感引擎（jiwen 5 轴模型）驱动主动行为：bond / stubbornness / mood / anxiety / busyness 阈值触发打招呼、提醒休息等。

### 3.5 Agent 模式

| 模式 | 能力 |
|------|------|
| **默认** | 无 system prompt，标准参数(0.7/4096)——最常用的纯对话模式 |
| **快速** | 无 system prompt，低温度(0.3) + 短输出(2048)——简短确定的回答 |
| **深度** | 无 system prompt，长输出(8192)——需要长篇回答的场景 |
| **翻译助手** | 内置翻译 system prompt，低温度(0.3)——高频且 prompt 稳定 |
| **自定义** | 用户自定义 system_prompt / temperature / max_tokens / model / tools 组合 |

> 原 Prompt 模板（代码解释/文本摘要/续写/头脑风暴）不再内置，用户可自行创建自定义模式。I6 实现。

---

## 4. 主窗口

主窗口专注**工具箱**与**设置**，不包含 AI 对话。

> 前端 UI 设计详见 [FRONTEND.md](./FRONTEND.md)

### 4.1 路由

| 路由 | 页面 |
|------|------|
| `/` | 工具箱首页（分类导航 + 搜索 + 收藏） |
| `/history` | 消息历史管理（对话列表 + 搜索 + 筛选 + 批量操作） |
| `/tools/:id` | 具体工具界面 |
| `/settings` | 设置首页（重定向到 `/settings/general`） |
| `/settings/general` | 通用设置（语言/开机自启/关闭到托盘/置顶） |
| `/settings/appearance` | 外观设置（主题 + Orb 外观） |
| `/settings/shortcuts` | 快捷键配置 |
| `/settings/providers` | AI Provider 配置 |
| `/settings/agents` | Agent 模式管理（合并原 Prompt 模板） |
| `/settings/about` | 关于 |

### 4.2 工具箱

#### 工具接口

```typescript
interface ToolDefinition {
  id: string
  name: string
  category: 'crypto' | 'converter' | 'dev' | 'text' | 'math' | 'web' | 'daily' | 'media'
  icon: string
  keywords: string[]
  component: React.LazyExoticComponent<any>
}
```

#### 工具清单

| 分类 | 工具 |
|------|------|
| 加密 | Hash 计算、UUID 生成、Base64 编解码 |
| 转换器 | 颜色转换、时间戳转换、进制转换、URL 编解码、单位换算 |
| 开发 | JSON 格式化、正则测试器、JWT 解码、SQL 格式化、Cron 解析 |
| 文本 | Diff 对比、字数统计 |
| 数学 | 计算器 |
| 网络 | IP 查询 |
| 日常 | 二维码生成、密码生成 |
| 多媒体 | 图片压缩、SVG 预览 |

### 4.3 设置

- **通用设置**：语言（占位 zh-CN）/ 开机自启 / 关闭到托盘 / 置顶主窗口
- **外观设置**：主题（亮/暗/跟随系统）+ Orb 悬浮球外观（透明度/缩放/圆角/鼠标穿透/悬停隐藏/悬停延迟）+ Orb 悬浮窗（始终置顶）
- **快捷键**：全局快捷键（toggle_orb / toggle_main / new_chat）+ 应用内快捷键（command_palette），分组展示 + 录制 + 冲突检测 + 重置
- **Provider 管理**：添加/编辑/删除 AI Provider（名称 + API Key + Base URL + 模型列表）
- **Agent 模式**：内置（默认/快速/深度/翻译助手）+ 自定义模式，支持 system_prompt / temperature / max_tokens / model / tools 组合
- **关于**：版本信息 + 构建信息 + 检查更新（预留）

> **设置保存统一链路**：所有设置变更通过 `settingsDispatcher` 统一入口 → `invoke()` 写后端 → `store.load()` 刷新缓存 → `emitSettingEvent()` 跨窗口广播 → 消费端 `listen()` 生效。

---

## 5. 数据存储

### 5.1 SQLite 表

```sql
CREATE TABLE providers (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  type TEXT NOT NULL,
  base_url TEXT NOT NULL,
  api_key_encrypted TEXT NOT NULL,
  models TEXT NOT NULL,
  default_model TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE conversations (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  provider_id TEXT,
  model_id TEXT NOT NULL,
  system_prompt TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE messages (
  id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  role TEXT NOT NULL,
  content TEXT NOT NULL,
  tokens INTEGER,
  timestamp INTEGER NOT NULL
);

CREATE TABLE agent_modes (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  kind TEXT NOT NULL DEFAULT 'custom',
  prompt TEXT NOT NULL DEFAULT '',
  model TEXT,
  temperature REAL,
  max_tokens INTEGER,
  tools TEXT NOT NULL DEFAULT '[]',
  presentation_rank INTEGER NOT NULL DEFAULT 99,
  is_builtin INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE tool_favorites (
  tool_id TEXT PRIMARY KEY
);

CREATE TABLE tool_history (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  tool_id TEXT NOT NULL,
  used_at INTEGER NOT NULL
);

CREATE TABLE compression_state (
  conversation_id TEXT PRIMARY KEY REFERENCES conversations(id) ON DELETE CASCADE,
  last_compression_at INTEGER,
  compression_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE app_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
```

> **I2-History Schema v2（规划中）**：conversations 新增 is_pinned / is_starred / color_mark / labels / draft_content / draft_updated_at / message_count / total_tokens / generation / sort_order 共 9 列；messages 新增 parent_id / source / feedback / is_pinned 共 4 列；新增 tool_calls 表（id / message_id / name / arguments / result / status / started_at / finished_at / is_error）+ 2 个索引。

### 5.2 文件结构

```
~/.assistant/
├── data/assistant.db     # SQLite
├── config/app.json       # 应用配置
└── cache/                # 缓存
```

---

## 6. API 通信层

### 6.1 Provider 适配器

```typescript
abstract class BaseProviderAdapter {
  abstract chat(req: ChatRequest): Promise<ChatResponse>
  abstract chatStream(req: ChatRequest): AsyncGenerator<ChatResponse>
  abstract listModels(): Promise<ModelConfig[]>
}
```

实现：`OpenAIAdapter`（兼容 Claude/Gemini/DeepSeek 等 OpenAI 格式）、`AnthropicAdapter`、`GeminiAdapter`、`CustomAdapter`

### 6.2 流式渲染

SSE 逐 token 渲染 + `chatDeltaCoalescer` 增量合并（借鉴 OpenHuman）。

### 6.3 错误处理

前端 `ChatError` 体系（13 种错误码，含 retryable 标记）：

| ChatErrorCode | 说明 | retryable |
|---------------|------|-----------|
| ApiKeyMissing | API Key 未配置 | ❌ |
| AuthFailed | 认证失败 | ❌ |
| RateLimited | 请求限流 | ✅ |
| ModelNotFound | 模型不可用 | ❌ |
| ContextOverflow | 上下文过长 | ❌ |
| NetworkError | 网络错误 | ✅ |
| Timeout | 请求超时 | ✅ |
| ServerError | 服务端错误 | ✅ |
| RequestRejected | 请求被拒绝 | ❌ |
| ParseError | 解析错误 | ❌ |
| Cancelled | 已取消 | ❌ |
| StreamError | 流式错误 | ✅ |
| InternalError | 内部错误 | ❌ |

retryable 错误支持指数退避重试（max 3 次，base 1s，max 30s）。

---

## 7. 项目目录

> Rust 后端结构择优参考 BongoCat 的 workspace 多 crate 架构（编译隔离 + API 边界强制 + 独立测试），按我们的领域划分为 1 个根包 + 6 个 workspace crate，远少于 BongoCat 的 20 个。ChatGPT 的 core 模块组织（cmd/conf/constant/setup/window）融入 `assistant-app` 根包。Tauri 2 CLI 要求二进制包位于 `src-tauri/` 根目录（`Cargo.toml` 同时包含 `[package]` 和 `[workspace]`），因此 `assistant-app` 作为根包而非子 crate。

> **实现状态**：Rust 后端 7 crate 骨架已在 I0 中实现；I1 完成 Orb 悬浮球全部功能；I2 完成 AI 对话全部功能（Provider 管理 + 流式输出 + 对话持久化 + ChatError 错误体系）；I3 完成工具箱全部功能（21 工具 + 命令面板 + 侧边栏）；I4 完成设置系统全部功能（通用/外观/快捷键/关于 + 全局快捷键注册 + 侧边栏重组 + 标题栏增强 + settingsDispatcher 统一链路）；I6 完成 Agent 自定义模式（agent_modes 表 + 内置预设 + 模式切换 + token 追踪 + Prompt 模板合并）。I2-History 进行中（Schema v2 迁移 + 高级 CRUD + 虚拟滚动 + 搜索导出等）。前端 mascot/ 等业务模块仍为规划结构，将在后续迭代中逐步实现。

```
assistant/
├── src-tauri/                     # Rust 后端（Cargo workspace，根包为二进制 crate）
│   ├── src/                       # assistant-app 根包（Tauri 集成层）
│   │   ├── main.rs                # 入口（windows_subsystem → assistant_app::run()）
│   │   ├── lib.rs                 # Builder + plugins + setup + invoke_handler
│   │   ├── cmd.rs                 # Tauri commands（前端可调用，53 个）
│   │   ├── chat_tasks.rs          # 流式聊天任务管理（cancel + retry）
│   │   ├── sse_log.rs             # SSE 调试日志模块
│   │   ├── setup.rs               # 应用初始化（配置 + 数据库 + 窗口 + 托盘）
│   │   ├── tray.rs                # 系统托盘
│   │   └── window.rs              # 窗口管理（创建/展开/收起/设置）
│   ├── crates/
│   │   ├── assistant-config/      # 配置 crate（独立于 Tauri，可纯测试）
│   │   │   ├── src/
│   │   │   │   ├── lib.rs
│   │   │   │   ├── app_conf.rs    # AppConf（theme/language/hotkey/stay_on_top/launch_at_login/close_to_tray/keybindings）
│   │   │   │   ├── orb_conf.rs    # OrbConf（click_through/opacity/hover/placement）
│   │   │   │   └── constant.rs    # 常量（窗口标签/尺寸/阈值）
│   │   │   └── Cargo.toml
│   │   ├── assistant-orb/         # Orb 悬浮球逻辑 crate
│   │   │   ├── src/
│   │   │   │   ├── lib.rs
│   │   │   │   ├── hover.rs       # 指针悬停隐藏（延迟 + 淡出，参考 BongoCat overlay/hover.rs）
│   │   │   │   └── placement.rs   # 位置约束（多显示器 + 边缘吸附，参考 BongoCat overlay/placement.rs）
│   │   │   └── Cargo.toml
│   │   ├── assistant-providers/   # AI Provider 适配器 crate
│   │   │   ├── src/
│   │   │   │   ├── lib.rs
│   │   │   │   ├── base.rs        # BaseProviderAdapter trait
│   │   │   │   ├── openai.rs      # OpenAI 兼容格式
│   │   │   │   └── crypto.rs      # API Key AES-256 加解密
│   │   │   └── Cargo.toml
│   │   ├── assistant-agent/       # Agent 运行时 crate
│   │   │   ├── src/
│   │   │   │   ├── lib.rs
│   │   │   │   ├── modes.rs       # Quick/Standard/Deep
│   │   │   │   └── compaction.rs  # Context Compaction（参考 OpenBitFun）
│   │   │   └── Cargo.toml
│   │   ├── assistant-storage/     # 数据层 crate
│   │   │   ├── src/
│   │   │   │   ├── lib.rs
│   │   │   │   ├── db.rs          # SQLite 初始化 + 迁移
│   │   │   │   ├── conversations.rs # 对话+消息 CRUD（含分页/搜索/导出）
│   │   │   │   ├── providers.rs   # Provider CRUD
│   │   │   │   ├── agent_modes.rs # Agent 模式 CRUD（合并原 Prompt 模板）
│   │   │   └── Cargo.toml
│   │   └── assistant-i18n/        # 国际化 crate（参考 BongoCat bongocat-i18n）
│   │       ├── src/
│   │       │   └── lib.rs
│   │       ├── locales/
│   │       │   ├── zh-CN.json
│   │       │   └── en-US.json
│   │       └── Cargo.toml
│   ├── Cargo.toml                 # [package] 根包 + [workspace] 定义
│   └── tauri.conf.json
│
├── src/                           # React 前端
│   ├── main.tsx                   # 入口（Provider 包裹 + 主题 + StrictMode）
│   ├── App.tsx                    # 多窗口分流（webview.label → 组件映射）
│   ├── config/                    # 配置（对应 Rust assistant-config crate）
│   │   └── constants.ts           # 常量（窗口标签/尺寸/阈值/穿透防抖，与 Rust constant.rs 对齐）
│   ├── orb/                       # Orb 悬浮球模块
│   │   ├── OrbBall.tsx            # 悬浮球主体（hitbox + 视觉样式 + 拖拽/点击事件 + hover 轮询 + 穿透切换）
│   │   ├── OrbPanel.tsx           # 展开后的对话面板（失焦收起 + Escape）
│   │   ├── OrbDrag.tsx            # 拖拽逻辑（8px 阈值 + 平台分发 + 边缘吸附 + 位置保存）
│   │   ├── OrbExpand.ts           # 展开/收起控制（方向自适应 + click-through 切换）
│   │   ├── OrbCoord.ts            # 统一逻辑坐标系工具（物理↔逻辑转换 + 几何判断）
│   │   ├── OrbContextMenu.tsx     # 右键菜单（Ant Design Dropdown）
│   │   └── useOrbConfSync.ts      # Orb 配置实时同步（listen OrbConfChanged → CSS 变量 + 运行时生效）
│   ├── mascot/                    # Mascot 形象
│   │   ├── RiveMascot.tsx         # Rive 渲染（useRive + useViewModel）
│   │   ├── faceStates.ts          # MascotFace 枚举 + faceToPose 映射
│   │   ├── visemes.ts             # 口型系统（openness/width + lerpViseme）
│   │   └── emotionEngine.ts       # jiwen 情感引擎集成
│   ├── chat/                      # 对话组件（Orb 面板内使用）
│   │   ├── ChatView.tsx
│   │   ├── MessageList.tsx
│   │   ├── MessageInput.tsx
│   │   ├── ModelSelector.tsx
│   │   ├── ModeSelector.tsx       # Agent 模式选择器（I6）
│   │   ├── TokenUsageBar.tsx      # token 用量指示条（I6）
│   │   ├── ProviderSelector.tsx   # 对话栏内嵌 Provider 切换
│   │   ├── chatErrors.ts          # ChatError 13 种错误码 + retryable 标记
│   │   └── streamCoalescer.ts     # 流式增量合并（rAF + setTimeout 双保险）
│   ├── tools/                     # 工具箱（主窗口使用）
│   │   ├── ToolBox.tsx            # 工具箱首页（分类 + 搜索 + 收藏）
│   │   ├── ToolPage.tsx           # 工具页统一布局（标题+分隔线+描述+内容）
│   │   ├── registry.ts            # 工具注册表（defineTool 工厂）
│   │   └── [tool-name]/           # 各工具实现
│   ├── components/                # 全局共享组件
│   │   ├── CommandPalette.tsx     # 命令面板（Ctrl/Cmd+K，keywords+External+closeOnSelect）
│   │   └── CommandPalette.module.css
│   ├── settings/                  # 设置页面
│   │   ├── GeneralSettings.tsx    # 通用设置（语言/开机自启/关闭到托盘/置顶）
│   │   ├── AppearanceSettings.tsx # 外观设置（主题 + Orb 外观）
│   │   ├── OrbBallSection.tsx     # Orb 悬浮球外观（透明度/缩放/圆角/穿透/悬停）
│   │   ├── OrbPanelSection.tsx    # Orb 悬浮窗设置（始终置顶）
│   │   ├── useDebouncedOrbSave.ts # Orb 配置防抖保存 hook
│   │   ├── ShortcutSettings.tsx   # 快捷键配置页
│   │   ├── ShortcutRecorder.tsx   # 快捷键录制组件
│   │   ├── AboutPage.tsx          # 关于页
│   │   ├── ProviderManager.tsx
│   │   ├── AddProviderDialog.tsx  # 新增 Provider（预设选择）
│   │   ├── ConnectDialog.tsx      # Provider 连接配置
│   │   ├── ProviderSwatch.tsx     # Provider 格式标签
│   │   ├── AgentModeManager.tsx   # Agent 模式管理
│   │   ├── settingsDispatcher.ts  # 设置统一分发（dispatchAppConfChanged / dispatchOrbConfChanged / dispatchThemeChanged / dispatchProviderSaved）
│   │   └── settingListeners.ts    # 设置变更监听
│   ├── history/                   # 消息历史管理（I2-History）
│   │   └── HistoryManager.tsx     # 对话列表 + 搜索 + 筛选 + 批量操作
│   ├── components/                # 通用组件
│   │   ├── Titlebar.tsx           # 自定义标题栏（含 PinToggle + SettingsEntry）
│   │   ├── Sidebar.tsx            # 侧边栏（三区域布局：工具箱/导航/设置 + 底部版本号）
│   │   ├── CollapsibleToolMenu.tsx # 可折叠分类菜单
│   │   ├── ThemeToggle.tsx        # 主题三态切换
│   │   ├── PinToggle.tsx          # 窗口置顶切换
│   │   ├── SettingsEntry.tsx      # 设置入口按钮
│   │   └── WindowControls.tsx     # 窗口控制按钮
│   ├── stores/                    # Zustand
│   │   ├── themeStore.ts          # 主题状态（Tauri 原生 API）
│   │   ├── orbStore.ts            # Orb 状态（展开/收起/位置/穿透缓存/展开方向）
│   │   ├── chatStore.ts           # 对话状态（含分页加载/搜索/错误处理/模式选择/token用量）
│   │   ├── agentModeStore.ts      # Agent 模式状态
│   │   ├── settingsStore.ts       # 设置状态（appConf / orbConf / load/save/updateKeybinding/resetKeybindings）
│   │   ├── mascotStore.ts         # Mascot 表情状态
│   │   ├── providerStore.ts       # Provider 配置
│   │   └── toolStore.ts           # 工具状态（分类/收藏/搜索）
│   ├── services/
│   │   ├── providers/             # Provider 前端适配
│   │   └── settingEvents.ts       # 设置事件消费端（listen SettingEvent → store.load）
│   ├── types/
│   │   └── events.ts             # SettingEvent / SettingAction / SettingEventPayload 枚举
│   ├── hooks/
│   │   └── useTheme.ts            # Tauri 原生主题 hook
│   ├── utils/
│   └── assets/
│       ├── icons/
│       ├── styles/
│       │   ├── global.css          # CSS 变量（亮/暗两套）
│       │   └── theme.ts            # Ant Design 主题令牌配置
│       │   # 注：各组件使用 CSS Modules（*.module.css）实现样式隔离
│       └── mascot.riv              # Rive 动画资源
│
├── package.json
├── tsconfig.json
└── vite.config.ts
```

---

## 8. 安全设计

- API Key AES-256 加密存储，密钥派生自机器唯一标识
- Tauri 2 安全模型：前端通过 IPC 调用 Rust，所有命令需显式声明权限
- Rust 后端处理所有敏感操作（API 请求、加密、数据库），前端零权限访问系统资源
- 不收集用户数据，不发送遥测

---

## 9. 开发计划

采用迭代交付，每个迭代交付**完整可用的功能增量**。

### I0 — 项目脚手架 ✅

**交付：** 可运行的 Tauri 2 空项目 + Cargo workspace + 开发环境

- [x] Tauri 2 + React + Vite + TS 初始化
- [x] Cargo workspace 根包 + 6 crate 结构（app 根包 + config/orb/providers/agent/storage/i18n）
- [x] Rust 后端基础（workspace Cargo.toml、tauri.conf.json 权限配置、SQLite 启动初始化）
- [x] ESLint + Prettier + TS 严格模式
- [x] Zustand + React Router
- [x] 主题系统（亮/暗/跟随系统）
- [x] SQLite 初始化（rusqlite + tauri-plugin-sql）
- [x] 系统托盘（右键：打开主窗口 / 退出）

**踩坑与决策记录**：详见 [DEVLOG.md](./DEVLOG.md) I0 章节

**参考：**

| 功能点 | 参考项目 | 参考文件 | 借鉴内容 |
|--------|---------|---------|---------|
| Cargo workspace 架构 | BongoCat | Cargo.toml (workspace root) | `workspace.members` + `workspace.package` + `workspace.dependencies` 统一版本管理 |
| workspace 依赖共享 | BongoCat | Cargo.toml L30-60 | `bongocat-config = { path = "crates/bongocat-config" }` 内部 crate 路径引用 |
| Tauri 窗口配置 | WindowPet | src-tauri/tauri.conf.json | transparent/alwaysOnTop/skipTaskbar/bundle/updater 配置结构 |
| 系统托盘 | WindowPet | src-tauri/src/app/tray.rs | `init_system_tray()` 菜单构建 + `handle_tray_event()` 事件分发 |
| 双窗口管理 | WindowPet | src-tauri/src/app/utils/ | main + setting 两个独立窗口的创建与切换 |
| AppConf load/save | ChatGPT | src-tauri/src/core/conf.rs | `AppConf` 结构体 + `load()`/`save()`/`get_conf_path()` + JSON 文件持久化 |
| setup 初始化 | ChatGPT | src-tauri/src/core/setup.rs | `init(app: &mut App)` 中创建窗口 + 加载配置 + 设置主题 |
| 命令集中管理 | ChatGPT | src-tauri/src/core/cmd.rs | `#[command]` 统一导出，`invoke_handler` 集中注册 |

**验收：** `npm run tauri dev` 启动后可见主窗口 + 托盘图标，主题可切换

---

### I1 — Orb 悬浮球 ✅

**交付：** 桌面常驻透明悬浮球，可拖拽、可吸附、可唤起

- [x] 透明悬浮球窗口（transparent + alwaysOnTop + skipTaskbar，88×88 含 chrome inset）
- [x] 鼠标穿透 + Rust 端鼠标位置获取（mouse_position crate）+ Windows CSS pointer-events 精细控制
- [x] 拖拽 + 边缘吸附动画
- [x] 全局快捷键 Cmd/Ctrl+K 唤起/隐藏
- [x] 点击展开对话面板，失焦自动收起
- [x] 右键菜单：新建对话 / 打开主窗口 / 退出

**参考：**

| 功能点 | 参考项目 | 参考文件 | 借鉴内容 |
|--------|---------|---------|---------|
| 透明悬浮窗口 | WindowPet | src-tauri/tauri.conf.json L80-91 | `width:64, height:64, transparent:true, alwaysOnTop:true, skipTaskbar:true` |
| 鼠标穿透 + 位置获取 | WindowPet | src-tauri/src/app/cmd.rs | `get_mouse_position()` 用 `mouse_position::mouse_position::Mouse` crate，返回 `{clientX, clientY}` |
| 拖拽（指针式） | OpenBitFun | AgentCompanionPointerDragService.ts | 指针拖拽：记录 grab 点 → pointermove 合并 → `setPosition` IPC，区分点击/拖拽阈值 8px |
| 拖拽阈值区分 | OpenBitFun | AgentCompanionDesktopPet.tsx L43 | `PET_DRAG_THRESHOLD_PX = 8`：移动 <8px 视为点击，≥8px 开始拖拽 |
| 失焦隐藏 | Onepoint | src/electron/utils/window.ts | `setWindowVisile`：hide+blur 失焦，show+setVisibleOnAllWorkspaces+focus 唤起 |
| 桌面宠物尺寸 | OpenBitFun | AgentCompanionDesktopPet.tsx L28-29 | `DEFAULT_PET_SIZE = 96`，`WINDOW_EDGE_BUFFER = 4`，边缘留白防裁剪 |
| Overlay 配置 schema | BongoCat | crates/bongocat-config/src/config_schema/overlay.rs | `OverlayConfig { click_through, always_on_top, scale_percent, opacity_percent, hide_on_pointer_hover, hover_delay_ms }` |
| 指针悬停隐藏 | BongoCat | crates/bongocat-overlay/src/hover.rs | `PointerHoverObservation` + `HOVER_FADE_DURATION=300ms` + `pointer_inside_window()` 边界检测 |
| 点击穿透切换 | BongoCat | crates/bongocat-overlay/src/windows/window.rs | `set_click_through()`：`WS_EX_TRANSPARENT|WS_EX_LAYERED` 扩展样式切换 |
| 点击穿透状态缓存 | BongoCat | crates/bongocat-overlay/src/windows/session.rs | `applied_click_through` 字段避免冗余 Win32 调用 |
| 像素级碰撞检测 | WindowPet | src/scenes/manager.ts | `InputManager.checkIsMouseInOnPet()`：Phaser `hitTestPointer` + 50ms 防抖 |
| DOM 像素级穿透 | airi | composables/use-chat-floating-click-through.ts | `elementFromPoint` + `isPaintedAt` + `forward:true` + 8px 手抖容差 |
| 位置约束 | BongoCat | crates/bongocat-overlay/src/placement.rs | `PLACEMENT_SETTLE_DELAY=1s` + `bounds_inside_screens()` 切片算法（窗口每一像素都必须在屏幕上，正确处理 L 形多屏缺口）+ `OverlayPlacementConstraint` 延迟纠正状态机（窗口位置变化重置倒计时，避免中断拖拽） |

**验收：** 桌面可见透明小球，拖拽贴边，点击展开面板，失焦收起，快捷键唤起

---

### I2 — Orb AI 对话 ✅

**交付：** Orb 面板内可用的多模型对话，流式输出

- [x] Provider 配置 UI（主窗口设置页内）
- [x] API Key 加密存储（Rust 端 AES-256）
- [x] Rust 端 Provider 适配器（OpenAI 兼容格式为基准）
- [x] 前端 Provider 适配层（统一接口）
- [x] Orb 面板对话 UI：消息列表 + 输入框 + 模型选择
- [x] 流式输出实时渲染（SSE + chatDeltaCoalescer）
- [x] 对话管理（新建 / 列表 / 删除 / 重命名）
- [x] 对话持久化（SQLite conversations + messages）
- [x] Prompt 模板库（内置 5 个）

**参考：**

| 功能点 | 参考项目 | 参考文件 | 借鉴内容 |
|--------|---------|---------|---------|
| 流式增量合并 | OpenHuman | app/src/services/chatDeltaCoalescer.ts | `createChatDeltaCoalescer`：相邻同 key delta 合并，每 animation frame 最多 1 次 dispatch；隐藏窗口用 `setTimeout(50ms)` 兜底防 rAF 节流卡死 |
| 流式调度器 | OpenHuman | chatDeltaCoalescer.ts L56-68 | `frameScheduler`：`requestAnimationFrame` + `setTimeout(50)` 双保险，窗口不可见时 rAF 暂停但 timer 兜底 |
| delta 合并条件 | OpenHuman | chatDeltaCoalescer.ts L97-106 | 仅 `(thread_id, request_id, round, channel)` 完全相同的相邻 delta 才合并，不同 key 间保序 |
| 排序保证 | OpenHuman | chatDeltaCoalescer.ts L30-36 | 非 delta 事件（如 tool_call）必须先 `flush()` 再处理，确保文本在工具调用之前 |
| Provider 管理 UI | ChatGPT | src/components/ProviderModal.tsx | Provider 列表 + 添加/编辑/删除 + API Key 输入 + 模型选择 |
| 对话列表侧栏 | ChatGPT | src/components/Sidebar.tsx | 对话列表 + 新建/删除/重命名 + 搜索过滤 |
| 错误处理 + 重试 | xsai | src/retry.ts | 指数退避重试策略 + 可重试错误分类 |

**验收：** 配置 API Key 后 Orb 面板内可正常对话，流式输出，历史可查看

---

### I2-History — 聊天消息历史管理（进行中）

**交付：** 分页加载、搜索、导出、置顶/收藏、消息编辑/重试、代码高亮

- [x] Schema 扩展（新字段写入 CREATE TABLE，删旧 DB 重建）+ 结构体扩展
- [x] 对话高级操作 CRUD + Tauri Commands（置顶/收藏/复制/批量删除/搜索）
- [x] 消息操作 CRUD + Tauri Commands（编辑/重试/删除）
- [x] chatStore 扩展（分页/搜索/导出/重试）+ UI 组件
- [ ] 代码高亮（shiki）+ 数学公式（KaTeX）

> **前置实现**：分页加载、搜索、导出、ChatError 13 种错误码已实现。ConversationSidebar 已删除（孤立组件），功能迁移至 HistoryManager。规格详见 [I2-history-spec.md](./store/I2-history-spec.md) / [I2-history-tasklist.md](./store/I2-history-tasklist.md)

**验收：** 分页加载流畅，搜索/导出可用，置顶/收藏可用，消息可编辑/重试

---

### I3 — 主窗口工具箱 ✅

**交付：** 完整工具箱，分类导航 + 搜索 + 收藏 + 21 工具

- [x] 工具注册系统（ToolDefinition 接口 + defineTool 工厂）
- [x] 工具箱首页（分类 + 搜索 + 收藏 + 最近使用 + 拖拽排序）
- [x] 加密工具：Hash 计算、UUID 生成、Base64 编解码
- [x] 转换器工具：颜色转换、时间戳转换、进制转换、URL 编解码、单位换算
- [x] 开发工具：JSON 格式化、正则测试器、JWT 解码、SQL 格式化、Cron 解析
- [x] 文本工具：Diff 对比、字数统计
- [x] 数学工具：计算器
- [x] 网络工具：IP 查询
- [x] 日常工具：二维码、密码生成
- [x] 多媒体工具：图片压缩、SVG 预览
- [x] 工具使用历史（SQLite tool_history）+ 收藏持久化（SQLite tool_favorites）
- [x] 命令面板（Ctrl/Cmd+K）+ 模糊搜索（fuse.js）
- [x] 侧边栏可折叠分类菜单（CollapsibleToolMenu）
- [x] 对话导出（Markdown / JSON）

> **原 I4（工具箱扩展）已合并入 I3**。I4 本质是 I3 的量变（仅新增工具组件，框架零改动），拆分两个迭代人为制造交付断层。合并后一次性交付完整工具箱，与参考项目 it-tools（60+ 工具一体交付）一致。

**规格**：详见 [I3-spec.md](./store/I3-spec.md) / [I3-tasklist.md](./store/I3-tasklist.md)

**参考：**

| 功能点 | 参考项目 | 参考文件 | 借鉴内容 |
|--------|---------|---------|---------|
| 工具类型定义 | IT-Tools | src/tools/tools.types.ts | `Tool { name, path, description, keywords[], component, icon, isNew, createdAt }` + `ToolCategory { name, components[] }` |
| 工具注册 | IT-Tools | src/tools/tool.ts | `defineTool()`：自动计算 `isNew`（createdAt 在 2 周内），统一入口 |
| 工具 Store | IT-Tools | src/tools/tools.store.ts | `useToolStore`：`toolsByCategory`（groupBy 分类）、`favoriteTools`（localStorage 持久化）、`newTools`（新工具过滤）、`addToolToFavorites/removeToolFromFavorites` |
| 模糊搜索 | IT-Tools | src/composable/fuzzySearch.ts | `useFuzzySearch`：基于 **fuse.js**，支持 keywords 权重搜索，空查询可配置返回全部或空 |
| 命令面板 | IT-Tools | src/components/CommandPalette.tsx | `Ctrl/Cmd+K` 唤起，Tools+Navigation+Actions+External 四分类混合搜索，keywords 权重(w:1.5)，选项图标，closeOnSelect，键盘导航（↑↓+Enter） |
| 侧边栏布局 | IT-Tools | src/layouts/base.layout.vue | 收藏工具置顶 + 按分类折叠菜单（`CollapsibleToolMenu`）+ 版本号页脚 |
| 工具页布局 | IT-Tools | src/layouts/tool.layout.vue | 标题(h1,40px) + 收藏按钮 + 分隔线(200px,2px,20%透明度) + 描述 + 内容(max-width:600px居中) |
| 工具卡片 | IT-Tools | src/components/ToolCard.vue | 图标(40px) + isNew 徽章 + 收藏按钮 + 名称(truncate) + 描述(line-clamp-2) |
| 首页网格 | IT-Tools | src/pages/Home.page.vue | 收藏拖拽排序(vuedraggable) + 新工具区 + 全部工具区(按分类分组) |
| 折叠菜单 | IT-Tools | src/components/CollapsibleToolMenu.vue | ▶/▼ 箭头 + localStorage 折叠状态持久化 + toggle 分类 |
| 工具数量与分类 | IT-Tools | src/tools/index.ts | 60+ 工具按功能分类导入，每个工具独立目录（`tool-name/tool-name.vue` + `tool-name/index.ts`） |

**验收：** 主窗口工具箱可正常使用 20+ 工具，搜索过滤，收藏置顶

---

### I4 — 设置系统 + 全局快捷键 + 主窗口 UI 改造 ✅

**交付：** 完整设置系统 + 全局快捷键注册与配置 + 侧边栏重组/折叠菜单/标题栏增强

- [x] 通用设置页（语言/开机自启/关闭到托盘/置顶）
- [x] 外观设置页（主题切换 + Orb 透明度/缩放/圆角/穿透/悬停配置）
- [x] 快捷键配置页（分组展示 + 点击录制 + 冲突检测 + 重置）
- [x] 关于页（版本/构建/技术栈/许可证）
- [x] AppConf 扩展（launch_at_login / close_to_tray / keybindings）
- [x] 全局快捷键注册（toggle_orb / toggle_main / new_chat）
- [x] 托盘菜单增强（新建对话 + 设置入口）
- [x] tauri-plugin-autostart 开机自启
- [x] 侧边栏重组（工具箱区/导航区/设置区三区域 + 可折叠分类菜单 + 底部版本号）
- [x] 侧边栏选中态修复（进入工具页高亮当前工具项）
- [x] 标题栏增强（置顶切换 + 设置入口按钮）
- [x] 设置配置中间层（AppConf/OrbConf 收敛到 settingsDispatcher 标准路径 + CSS 变量消费链 + OrbConf 字段生效）
- [x] 快捷键跨平台修复（parse_shortcut 键码大小写 + Meta/Control 平台映射）

**规格**：详见 [I4-spec.md](./store/I4-spec.md) / [I4-tasklist.md](./store/I4-tasklist.md)

**参考：**

| 功能点 | 参考项目 | 参考文件 | 借鉴内容 |
|--------|---------|---------|---------|
| 设置页注册表 | OpenBitFun | web-ui/.../settingsRegistry.ts | `definePage()` 懒加载 + `SettingsPageManifest`（id/categoryId/labelKey/keywords/load），分类导航 |
| 快捷键配置 UI | OpenBitFun | web-ui/.../KeyboardShortcutsTab.tsx | 分组展示 + 搜索过滤 + 点击录制 + 冲突检测 + 重置默认 |
| 快捷键管理器 | OpenBitFun | web-ui/.../ShortcutManager.ts | `ShortcutRegistration` + `parseStoredKeybindings` / `buildStoredKeybindings`，持久化到 configManager |
| AppConf 扩展 | ChatGPT | src-tauri/src/core/conf.rs | `AppConf` 结构体 + `amend()` 向后兼容 + `load()` 缺失字段补默认值 |
| 托盘菜单 | WindowPet | src-tauri/src/tray.rs | 菜单项 + 事件处理 + 左键点击行为 |
| 开机自启 | Tauri 2 | tauri-plugin-autostart | `Manager::enable()` / `Manager::disable()` / `Manager::is_enabled()` |
| 可折叠分类菜单 | it-tools | CollapsibleToolMenu.vue | ▶/▼ 折叠箭头 + localStorage 持久化 + 收藏置顶 |
| 侧边栏布局 | it-tools | base.layout.vue | Logo 区 + CollapsibleToolMenu + 底部版本号 |

**验收：** 设置页全部可用，快捷键可录制/重置/冲突检测，全局快捷键生效，托盘菜单完整

---

### I5 — Mascot 形象 + 表情联动

**交付：** Rive 动画 Mascot，与对话状态联动

- [x] Rive 运行时集成（@rive-app/react-webgl2）
- [x] .riv 动画文件（9 种 RivePose：idle/thinking/celebration/bookreading/coffeedrink/writing/bobbateadrink/hand_wave/dancing）
- [x] 表情状态机（对话事件 → 表情切换）
- [x] Mascot 显示于 Orb 悬浮球内 + 发送按钮
- [ ] MascotFace 扩展至 18 种（对齐 OpenHuman Ghosty.tsx）
- [ ] FACE_TO_POSE 完整映射（18→9）
- [ ] RiveMascot 颜色换肤（primaryColor/secondaryColor）
- [ ] RiveMascot buffer 内存加载
- [ ] 用户输入 listening 状态联动
- [ ] ack 表情增强（proud 判断）

**参考：**

| 功能点 | 参考项目 | 参考文件 | 借鉴内容 |
|--------|---------|---------|---------|
| Rive 运行时 | OpenHuman | app/src/features/human/Mascot/RiveMascot.tsx | `useRive` + `useViewModel` + `useViewModelInstanceEnum` + `useViewModelInstanceColor`，支持换色/换表情/换 .riv 文件 |
| 表情词汇表 | OpenHuman | Ghosty.tsx L20-38 | `MascotFace` 18 种：sleep/idle/listening/thinking/confused/speaking/happy/concerned/curious/proud/cautious/celebrating/writing/reading/waving/dancing/drinking_coffee/drinking_boba |
| 表情→Rive pose 映射 | OpenHuman | riveMaps.ts L48-67 | `FACE_TO_POSE` 表：18 种 MascotFace → 9 种 RivePose（idle/thinking/celebration/bookreading/coffeedrink/writing/bobbateadrink/hand_wave/dancing） |
| 口型同步 | OpenHuman | visemes.ts | 8 种口型 `VisemeId`（REST/A/E/I/O/U/M/F），每个 `{openness, width}`，SVG 路径动态生成；`lerpViseme` 插值平滑过渡 |
| Rive viseme 映射 | OpenHuman | riveMaps.ts L82-119 | 15 种 Oculus viseme set + 别名归一化（不同 TTS 提供商大小写不一致，统一映射到 Rive asset 的 `visme_codes` enum） |
| 空闲动画轮转 | OpenHuman | RiveMascot.tsx L30-35 | `AMBIENT_IDLE_MIN_MS=6000, MAX=12000`：idle 6-12 秒后随机切换 ambient pose，2.5-5 秒后回 idle |
| 宠物情绪映射 | OpenBitFun | flow_chat/utils/agentCompanionMood.ts | `deriveAgentCompanionMood`：4 种 mood（rest/analyzing/waiting/working），从 `SessionStateMachine` 的 `ProcessingPhase` 派生 |
| 情绪外观 facet | OpenBitFun | AgentCompanionPet.appearance.ts | `data-openbitfun-mood` 属性驱动 CSS：rest/analyzing/waiting/working/hover/dragging |

**验收：** 对话时 Mascot 表情实时变化；情感轴可触发主动打招呼

---

### I6 — Agent 自定义模式 + Prompt 模板合并 ✅

**交付：** 内置预设 + 用户自定义 Agent 模式 + Prompt 模板合并为统一体系 + token 追踪

- [x] SQLite `agent_modes` 表 + Rust CRUD（assistant-storage）
- [x] 4 个内置预设（默认/快速/深度 + 翻译助手）自动 seed
- [x] Prompt 模板移除：`prompt_templates` 表 DROP、`templates.rs` 删除、`TemplateManager.tsx` 删除、路由删除
- [x] 前端 AgentModeManager（主窗口设置页：列表 + 创建/编辑/删除 + 排序）
- [x] Orb 面板模式切换 UI（下拉 Select，显示内置 + 自定义）
- [x] chatStore 集成（发送消息时注入选中模式的 system_prompt / temperature / max_tokens）
- [x] token 追踪 + 用量指示条

> **Prompt 模板合并**：原 `prompt_templates` 的核心能力（system prompt + 分类 + 内置预设）合并到 `agent_modes`。变量占位符 `{{variable}}` 交互已在 I2 中移除（DEVLOG 2026-10-05），不再保留。原 5 个内置模板仅保留翻译助手（kind='custom'），其余 4 个不再内置——这些场景的 prompt 因人而异，用户可自行创建自定义模式。默认/快速/深度三个模式均无 system prompt，仅控制 temperature / max_tokens。

> **原 I6（Agent 模式固定三级）升级为用户自定义**。参考 OpenBitFun `CustomAgentDefinition`，用户可创建自己的 Agent 模式（如"翻译助手"、"代码审查"），自定义 system_prompt / temperature / max_tokens / model / tools 组合。内置 Quick/Standard/Deep 作为预设 seed，不可删除。

**参考：**

| 功能点 | 参考项目 | 参考文件 | 借鉴内容 |
|--------|---------|---------|---------|
| 内置 Agent 模式 | OpenBitFun | crates/.../agent-runtime/src/agents.rs | `mode_presentation_rank`：Standard=0, Cowork=1, DeepResearch=2, Ultimate=3, Creative=4，排序展示 |
| 自定义 Agent 定义 | OpenBitFun | crates/.../agent-runtime/src/custom_agent.rs | `CustomAgentDefinition { id, name, description, kind, tools, prompt, readonly, review, level, model }`，用户可创建 Mode / Subagent |
| 前端 Agent 管理 | OpenBitFun | web-ui/src/app/scenes/agents/agentsStore.ts | `AgentFilterLevel`（all/builtin/user/project/external）+ 创建/编辑页面 |
| 模式配置 profile | OpenBitFun | agents.rs L22-27 | `STANDARD_HARNESS_CONFIG`：mode 可组合为 profile，支持子代理 |

> **Function Calling / 内置工具 / MCP 客户端暂不实现**，后续迭代按需加入。参考方案已验证：OpenBitFun `mcp_tool_bridge.rs` 命名规范 + 三维安全标记(read_only/destructive/open_world)。

**验收：** 可切换 Agent 模式（内置 + 自定义），可创建/编辑/删除自定义模式，不同模式 system prompt / 温度不同，token 用量可见，Prompt 模板管理页合并到 Agent 模式页

---

### I7 — Context Compaction

**交付：** 单会话内上下文自动压缩，长对话不溢出

当 token 量接近上下文窗口 70-80% 时，异步启动 LLM 摘要替换早期消息，保留最近 N 条完整消息。

```rust
// 参考: OpenBitFun/compression_prefetch.rs
const PREFETCH_LEAD_TOKENS: usize = 10_000;

fn in_prefetch_window(pressure: usize, input_limit: usize) -> bool {
    input_limit > PREFETCH_LEAD_TOKENS
        && pressure >= input_limit - PREFETCH_LEAD_TOKENS
        && pressure < input_limit
}

struct CompressionState {
    last_compression_at: Option<SystemTime>,
    compression_count: usize,
}
```

- 触发：`pressure >= input_limit - 10_000`
- 动作：LLM 摘要早期消息 → 替换为摘要块 + 保留最近 N 条
- 预取：异步启动，不阻塞当前轮次

> **长期记忆（跨会话 facet 提取 + 持久化 + 注入）暂不实现**，后续迭代按需加入。参考方案已验证：OpenHuman `summary_facets.rs` ParsedFacet + memory/tools。

**参考：**

| 功能点 | 参考项目 | 参考文件 | 借鉴内容 |
|--------|---------|---------|---------|
| 预取阈值 | OpenBitFun | compression_prefetch.rs L10-15 | `PREFETCH_LEAD_TOKENS=10000`，`in_prefetch_window`：pressure 在 `[limit-10k, limit)` 区间时触发 |
| 异步预取 | OpenBitFun | compression_prefetch.rs L17-50 | `CompressionPrefetch::spawn`：tokio 异步任务 + CancellationToken，`claim` 非阻塞检查结果（Ready/Discarded/Waiting） |
| 压缩状态 | OpenBitFun | session.rs L84-93 | `CompressionState { last_compression_at, compression_count }`，`increment_compression_count` 每次压缩后更新 |
| 会话配置 | OpenBitFun | session.rs L179,193 | `auto_compact: bool`（默认 true）+ `enable_context_compression: bool`（默认 true），可按会话开关 |

- [ ] Context Compaction（prefetch 阈值触发 + LLM 摘要替换）
- [ ] CompressionState 持久化（SQLite compression_state 表）
- [ ] 压缩状态 UI（对话中显示压缩次数）
- [ ] 自动更新（Tauri Updater）

**验收：** 长对话 token 接近窗口时自动压缩，对话不中断，压缩后 AI 可继续正常回复

---

### I8 — 打包发布

**交付：** 三平台安装包

- [ ] Windows MSI / NSIS
- [ ] macOS DMG（签名 + 公证）
- [ ] Linux AppImage / deb / rpm
- [ ] GitHub Release CI

**参考：**

| 功能点 | 参考项目 | 参考文件 | 借鉴内容 |
|--------|---------|---------|---------|
| Bundle 配置 | WindowPet | src-tauri/tauri.conf.json L38-51 | `bundle.active/targets/identifier/category/copyright/icon` 完整结构 |
| Updater 配置 | WindowPet | src-tauri/tauri.conf.json L52-63 | `updater.active/pubkey/endpoints/windows.installMode`，GitHub Release 端点格式 |

**验收：** 三平台安装包可正常安装/卸载/升级

---

### 迭代依赖

```
I0 脚手架 ✅
 └→ I1 悬浮球 ✅
     └→ I2 Orb AI 对话 ✅
         ├→ I2-History 消息历史管理（进行中）
         ├→ I3 工具箱 ✅（21 工具，8 分类）
         └→ I4 设置 + 快捷键 + UI改造 ✅
             └→ I5 Mascot 形象
                 └→ I6 Agent 自定义模式 ✅
                     └→ I7 Context Compaction
                         └→ I8 打包发布
```

I2-History 依赖 I2；I3 和 I4 可并行（均依赖 I2）；I5 依赖 I2+I4；I6 依赖 I2+I5；I7 依赖 I6

---

## 10. 参考项目

| 项目 | Stars | 技术栈 | 借鉴点 |
|------|-------|--------|--------|
| **BongoCat** | 23,758 | Tauri 2 + Rust workspace | 桌面宠物 + 透明 overlay + hover 淡出 + placement 约束 + 多 crate 架构 + config schema |
| **ChatGPT (lencx)** | 54,585 | Tauri 1 + Rust | AI 桌面对话 + AppConf load/save + 双窗口管理 + setup 初始化 + 命令结构 |
| **WindowPet** | 670 | Tauri 2 + Rust | 透明悬浮窗口 + 鼠标穿透 + 拖拽 + 托盘 + bundle/updater |
| **Onepoint** | 315 | Electron | 悬浮球失焦隐藏 + 全局快捷键 |
| **yoji** | 663 | Electron | 悬浮组件 + AI 对话 + MCP + 情感感知 |
| **OpenHuman** | 40,468 | Next.js + Rive | Rive Mascot + 表情状态机 + 口型同步 + 记忆 + chatDeltaCoalescer |
| **OpenBitFun** | 2,372 | Tauri + React | Agent 模式分级 + Provider 适配器 + MCP + Context Compaction + 桌面宠物 |
| **xsai** | 658 | TS | 轻量级 Provider SDK + 流式输出 |
| **jiwen** | 162 | TS | 极简情感引擎（~500 行，5 轴模型） |
| **AIRI** | 49,949 | Monorepo | 多动画引擎 + 向量记忆 |
| **IT-Tools** | 40,750 | Vue 3 | 工具分类体系 + 搜索 + 命令面板 + 收藏系统 |
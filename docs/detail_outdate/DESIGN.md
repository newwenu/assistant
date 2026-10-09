# Assistant - 智能助手应用设计文档（完整非当前版本）

> ⛔ **本文档已过时，仅作历史参考。** 当前有效架构文档为 [ARCHITECTURE.md](../ARCHITECTURE.md)，前端设计文档为 [FRONTEND.md](../FRONTEND.md)。
>
> 本文档与当前架构存在根本性冲突，包括但不限于：
> - 架构定位：本文档将 AI 对话放在主窗口，当前架构已改为 Orb 悬浮球交互
> - 布局结构：本文档为三栏布局，当前架构为侧边栏+主内容区两栏布局
> - Rust 后端：本文档为单 crate 平铺结构，当前架构已改为 7 crate workspace
> - 数据模型：本文档 API Key 明文存储，当前架构已改为加密存储
>
> **请勿以本文档为依据进行开发。**

## 1. 项目概述

**Assistant** 是一款桌面端智能助手应用，集成大语言模型 API 问答能力与多种实用工具，旨在为用户提供一站式效率提升体验。

### 1.1 核心定位

- **API 问答窗口**：支持接入多种大模型 API（OpenAI、Claude、Gemini、国产模型等），提供流畅的对话体验
- **内置工具箱**：集成开发者与日常办公常用工具，无需切换应用即可完成各类小任务
- **本地优先**：数据本地存储，API 密钥用户自行配置，不依赖云端服务

### 1.2 技术栈

| 层级 | 技术选型 | 说明 |
|------|---------|------|
| 框架 | **Tauri 2** | 轻量级跨平台桌面应用（Rust 后端 + Web 前端），包体小 10x+、内存低 50%+ |
| 后端 | Rust | Tauri 原生后端，处理窗口管理/Provider 适配/Agent 运行时/本地存储 |
| 前端 | React 18 + TypeScript | UI 构建 |
| 动画形象 | Rive | 轻量级矢量动画引擎，WebGL2 渲染，内置状态机，比 Live2D SDK 更轻 |
| UI 库 | Ant Design 5 | 组件库 |
| 状态管理 | Zustand | 轻量级状态管理 |
| 构建工具 | Vite | 快速构建 |
| 本地存储 | SQLite (via Rust rusqlite) | 对话历史持久化 |
| 代码高亮 | Prism.js / Shiki | 代码块渲染 |
| Markdown | react-markdown + remark-gfm | Markdown 渲染 |

---

## 2. 功能模块设计

### 2.1 API 问答模块

#### 2.1.1 API 配置管理

支持用户配置多个 API Provider，每个 Provider 包含：

```typescript
interface APIProvider {
  id: string
  name: string
  type: 'openai' | 'anthropic' | 'gemini' | 'custom'
  baseUrl: string
  apiKey: string
  models: ModelConfig[]
  defaultModel: string
  proxy?: string
}

interface ModelConfig {
  id: string
  name: string
  maxTokens: number
  supportStream: boolean
  supportVision: boolean
}
```

- 预置主流 Provider 模板（OpenAI、Claude、Gemini、DeepSeek、Qwen 等），用户填入 API Key 即可使用
- 支持自定义 Provider（兼容 OpenAI 格式的第三方服务）
- API Key 加密存储于本地

#### 2.1.2 对话管理

```typescript
interface Conversation {
  id: string
  title: string
  providerId: string
  modelId: string
  systemPrompt?: string
  messages: Message[]
  createdAt: number
  updatedAt: number
}

interface Message {
  id: string
  role: 'system' | 'user' | 'assistant'
  content: string
  tokens?: number
  timestamp: number
  attachments?: Attachment[]
}

interface Attachment {
  type: 'image' | 'file' | 'code'
  name: string
  content: string
  mimeType?: string
}
```

功能点：
- 多会话管理（创建、重命名、删除、搜索）
- 系统提示词预设与自定义
- 流式输出（SSE）实时渲染
- 上下文长度控制（自动截断或摘要）
- 对话导出（Markdown / JSON）
- 消息编辑与重新生成
- 图片上传（Vision 模型）
- 代码块一键复制 + 运行（部分语言）

#### 2.1.3 Prompt 模板库

```typescript
interface PromptTemplate {
  id: string
  name: string
  category: string
  content: string
  variables?: TemplateVariable[]
  isBuiltin: boolean
}

interface TemplateVariable {
  key: string
  label: string
  defaultValue?: string
}
```

- 内置常用 Prompt 模板（翻译、代码审查、文档生成等）
- 支持用户自定义模板，可含变量占位符 `{{variable}}`
- 模板分类管理

---

### 2.2 工具箱模块

#### 2.2.1 工具清单

| 序号 | 工具名称 | 分类 | 功能描述 |
|------|---------|------|---------|
| 1 | JSON 格式化 | 开发 | JSON 格式化 / 压缩 / 校验 / 树形查看 |
| 2 | Base64 编解码 | 开发 | 文本与图片的 Base64 编码/解码 |
| 3 | URL 编解码 | 开发 | URL 编码/解码/参数解析 |
| 4 | 正则测试器 | 开发 | 正则表达式实时匹配测试 |
| 5 | 代码格式化 | 开发 | 多语言代码格式化（Prettier 集成） |
| 6 | 颜色转换 | 开发 | HEX / RGB / HSL / CMYK 互转 + 取色器 |
| 7 | 进制转换 | 开发 | 二/八/十/十六进制互转 |
| 8 | Hash 计算 | 开发 | MD5 / SHA-1 / SHA-256 / SHA-512 |
| 9 | 时间戳转换 | 开发 | Unix 时间戳与日期互转，多时区显示 |
| 10 | UUID 生成 | 开发 | UUID v1/v4/v5 批量生成 |
| 11 | 文本差异对比 | 文本 | 双栏 Diff 对比，高亮差异行 |
| 12 | 字数统计 | 文本 | 字符/词/句/段统计 + 阅读时长 |
| 13 | 文本转换 | 文本 | 大小写/驼峰/下划线/短横线转换 |
| 14 | Markdown 预览 | 文本 | Markdown 实时渲染预览 |
| 15 | 文本去重 | 文本 | 按行去重 + 排序 |
| 16 | 二维码生成 | 日常 | 文本/链接转二维码，支持自定义样式 |
| 17 | 计算器 | 日常 | 科学计算器 + 表达式求值 |
| 18 | 单位换算 | 日常 | 长度/重量/温度/面积/数据存储换算 |
| 19 | 密码生成 | 日常 | 可配置长度/字符集的随机密码生成 |
| 20 | IP 查询 | 日常 | 本机 IP 查看 + IP 归属地查询 |
| 21 | 图片压缩 | 多媒体 | 本地图片压缩 + 格式转换 |
| 22 | SVG 预览 | 多媒体 | SVG 代码编辑 + 实时预览 |
| 23 | 网页截图 | 多媒体 | URL 截图（可选） |

#### 2.2.2 工具接口规范

```typescript
interface ToolDefinition {
  id: string
  name: string
  nameEn: string
  category: 'dev' | 'text' | 'daily' | 'media'
  icon: string
  description: string
  keywords: string[]
  component: React.LazyExoticComponent<any>
  isBuiltin: boolean
}
```

- 所有工具遵循统一接口，按需懒加载
- 工具可通过关键词搜索快速定位
- 支持收藏常用工具，置顶显示
- 工具使用记录（最近使用排序）

---

### 2.3 全局辅助功能

#### 2.3.1 全局快捷键

| 快捷键 | 功能 |
|--------|------|
| `Ctrl+Shift+Space` | 唤起/隐藏主窗口 |
| `Ctrl+N` | 新建对话 |
| `Ctrl+Shift+T` | 打开工具箱 |
| `Ctrl+,` | 打开设置 |
| `Ctrl+K` | 全局搜索（对话+工具） |

#### 2.3.2 剪贴板监听（可选）

- 监听剪贴板变化，自动识别内容类型并推荐对应工具
- 例如：复制 JSON → 推荐 JSON 格式化；复制 URL → 推荐 URL 解码

#### 2.3.3 系统托盘

- 最小化到托盘
- 托盘右键菜单：新建对话 / 打开工具箱 / 退出

---

## 3. 界面设计

### 3.1 整体布局

```
┌──────────────────────────────────────────────────┐
│  标题栏 (拖拽区 + 窗口控制)                        │
├────────┬─────────────────────────┬───────────────┤
│        │                         │               │
│  侧边栏 │       主内容区            │   右侧面板     │
│        │                         │  (可折叠)     │
│  ·对话列表│  ·对话窗口 / 工具界面     │  ·工具参数    │
│  ·工具导航│                         │  ·模型信息    │
│  ·设置入口│                         │               │
│        │                         │               │
│        ├─────────────────────────┤               │
│        │  输入区 (对话模式)         │               │
│        │  [附件] [发送] [模型选择]  │               │
└────────┴─────────────────────────┴───────────────┘
```

### 3.2 页面路由

| 路由 | 页面 | 说明 |
|------|------|------|
| `/` | 主页 | 欢迎页 / 最近对话 |
| `/chat/:id` | 对话页 | API 问答对话 |
| `/tools` | 工具箱 | 工具分类列表 |
| `/tools/:id` | 工具页 | 具体工具界面 |
| `/settings` | 设置 | 全局设置 |
| `/settings/providers` | API 配置 | Provider 管理 |
| `/settings/templates` | 模板管理 | Prompt 模板 |
| `/settings/about` | 关于 | 版本信息 |

### 3.3 主题

- 亮色 / 暗色 / 跟随系统
- CSS 变量驱动，支持自定义主题色

---

## 4. 数据存储设计

### 4.1 SQLite 表结构

```sql
CREATE TABLE providers (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  type TEXT NOT NULL,
  base_url TEXT NOT NULL,
  api_key TEXT NOT NULL,
  models TEXT NOT NULL,
  default_model TEXT NOT NULL,
  proxy TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE conversations (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  provider_id TEXT NOT NULL,
  model_id TEXT NOT NULL,
  system_prompt TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  FOREIGN KEY (provider_id) REFERENCES providers(id)
);

CREATE TABLE messages (
  id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL,
  role TEXT NOT NULL,
  content TEXT NOT NULL,
  tokens INTEGER,
  timestamp INTEGER NOT NULL,
  attachments TEXT,
  FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);

CREATE TABLE prompt_templates (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  category TEXT NOT NULL,
  content TEXT NOT NULL,
  variables TEXT,
  is_builtin INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL
);

CREATE TABLE tool_favorites (
  tool_id TEXT PRIMARY KEY,
  created_at INTEGER NOT NULL
);

CREATE TABLE tool_history (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  tool_id TEXT NOT NULL,
  used_at INTEGER NOT NULL
);

CREATE TABLE app_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
```

### 4.2 文件存储

```
~/.assistant/
├── config/
│   ├── app.json          # 应用配置
│   └── providers.json    # Provider 配置备份
├── data/
│   └── assistant.db      # SQLite 数据库
├── cache/
│   ├── images/           # 图片缓存
│   └── temp/             # 临时文件
└── logs/
    └── app.log           # 应用日志
```

---

## 5. API 通信层设计

### 5.1 统一请求接口

```typescript
interface ChatRequest {
  provider: APIProvider
  model: string
  messages: ChatMessage[]
  stream: boolean
  temperature?: number
  maxTokens?: number
  signal?: AbortSignal
}

interface ChatMessage {
  role: 'system' | 'user' | 'assistant'
  content: string | ContentPart[]
}

interface ContentPart {
  type: 'text' | 'image_url'
  text?: string
  image_url?: { url: string }
}

interface ChatResponse {
  id: string
  content: string
  usage?: { promptTokens: number; completionTokens: number }
  finishReason?: string
}
```

### 5.2 Provider 适配器

```typescript
abstract class BaseProviderAdapter {
  abstract chat(request: ChatRequest): Promise<ChatResponse>
  abstract chatStream(request: ChatRequest): AsyncGenerator<ChatResponse>
  abstract listModels(): Promise<ModelConfig[]>
  abstract validateApiKey(): Promise<boolean>
}

class OpenAIAdapter extends BaseProviderAdapter { /* ... */ }
class AnthropicAdapter extends BaseProviderAdapter { /* ... */ }
class GeminiAdapter extends BaseProviderAdapter { /* ... */ }
class CustomAdapter extends BaseProviderAdapter { /* ... */ }
```

- 每个 Provider 对应一个适配器实现
- 自定义 Provider 默认使用 OpenAI 兼容格式
- 统一错误处理与重试机制

---

## 6. 项目目录结构

```
assistant/
├── src-tauri/                    # Tauri 2 Rust 后端
│   ├── src/
│   │   ├── main.rs               # Tauri 主入口
│   │   ├── lib.rs                # 库入口
│   │   ├── commands/             # Tauri Commands（前端可调用的 Rust 函数）
│   │   │   ├── window.rs         # 窗口管理（透明/悬浮/拖拽/吸附）
│   │   │   ├── mouse.rs          # 鼠标位置获取（穿透窗口必需）
│   │   │   ├── tray.rs           # 系统托盘菜单
│   │   │   └── shortcut.rs       # 全局快捷键注册
│   │   ├── providers/            # AI Provider 适配器（Rust 端）
│   │   │   ├── mod.rs            # Provider trait 定义
│   │   │   ├── openai.rs
│   │   │   ├── anthropic.rs
│   │   │   └── gemini.rs
│   │   ├── agent/                # Agent 运行时
│   │   │   ├── modes.rs          # Agent 模式（Quick/Standard/Deep）
│   │   │   ├── session.rs        # 会话管理
│   │   │   └── tools.rs          # 工具注册与执行
│   │   ├── storage/              # 本地存储
│   │   │   ├── sqlite.rs         # SQLite 操作（rusqlite）
│   │   │   └── kv.rs             # 键值存储
│   │   └── utils/                # 工具函数
│   │       └── crypto.rs         # 加密（API Key 安全存储）
│   ├── Cargo.toml                # Rust 依赖
│   └── tauri.conf.json           # Tauri 配置（窗口/权限/插件）
│
├── src/                          # React 前端
│   ├── main.tsx                  # React 入口
│   ├── App.tsx                   # 根组件
│   ├── routes/                   # 路由配置
│   ├── layouts/                  # 布局组件
│   │   ├── MainLayout.tsx
│   │   ├── Sidebar.tsx
│   │   └── TitleBar.tsx
│   ├── pages/                    # 页面组件
│   │   ├── chat/
│   │   │   ├── ChatPage.tsx
│   │   │   ├── MessageList.tsx
│   │   │   ├── MessageInput.tsx
│   │   │   ├── MessageItem.tsx
│   │   │   └── ModelSelector.tsx
│   │   ├── tools/
│   │   │   ├── ToolBox.tsx
│   │   │   └── tools/            # 各工具组件
│   │   │       ├── JsonFormatter.tsx
│   │   │       ├── Base64Codec.tsx
│   │   │       ├── RegexTester.tsx
│   │   │       ├── ColorPicker.tsx
│   │   │       ├── TimestampConverter.tsx
│   │   │       ├── TextDiff.tsx
│   │   │       ├── QrGenerator.tsx
│   │   │       ├── Calculator.tsx
│   │   │       ├── UnitConverter.tsx
│   │   │       ├── PasswordGenerator.tsx
│   │   │       └── ...
│   │   └── settings/
│   │       ├── SettingsPage.tsx
│   │       ├── ProviderManager.tsx
│   │       └── TemplateManager.tsx
│   ├── orb/                      # Orb 悬浮球模块
│   │   ├── OrbBall.tsx           # 悬浮球主体（Rive 动画）
│   │   ├── OrbDrag.tsx           # 拖拽逻辑
│   │   ├── OrbExpand.tsx         # 展开动画（球→面板）
│   │   └── OrbTray.tsx           # 悬浮球托盘菜单
│   ├── mascot/                   # Mascot 形象系统
│   │   ├── RiveMascot.tsx        # Rive 动画渲染
│   │   ├── faceStates.ts         # 表情状态机
│   │   └── visemes.ts            # 口型同步
│   ├── chat/                     # 对话模块（共享组件）
│   │   ├── ChatPanel.tsx
│   │   ├── ChatInput.tsx
│   │   ├── MessageList.tsx
│   │   └── streamCoalescer.ts    # 流式增量合并
│   ├── tools/                    # 工具箱（共享）
│   │   ├── registry.ts           # 工具注册表
│   │   └── [tool-name]/          # 各工具实现
│   ├── components/               # 通用组件
│   │   ├── MarkdownRenderer.tsx
│   │   ├── CodeBlock.tsx
│   │   ├── SearchBar.tsx
│   │   └── ThemeToggle.tsx
│   ├── stores/                   # Zustand 状态
│   │   ├── chatStore.ts
│   │   ├── orbStore.ts
│   │   ├── mascotStore.ts
│   │   ├── providerStore.ts
│   │   └── toolStore.ts
│   ├── services/                 # 业务服务
│   │   ├── chat/
│   │   │   ├── chatService.ts
│   │   │   └── providers/        # Provider 前端适配
│   │   │       ├── adapter.ts
│   │   │       ├── openaiAdapter.ts
│   │   │       ├── anthropicAdapter.ts
│   │   │       ├── geminiAdapter.ts
│   │   │       └── customAdapter.ts
│   │   └── toolRegistry.ts
│   ├── hooks/                    # 自定义 Hooks
│   ├── utils/                    # 工具函数
│   └── assets/                   # 静态资源
│       ├── icons/
│       ├── styles/
│       └── mascot.riv            # Rive 动画资源
│
├── resources/                    # Tauri 资源
│   └── icon.png
├── package.json
├── tsconfig.json
├── vite.config.ts
└── DESIGN.md                     # 本文档
```

---

## 7. 开发计划

采用迭代交付模式，每个迭代交付一个**完整可用的功能增量**，可独立验证和体验。

### Iteration 0 — 项目脚手架

**交付物：** 可运行的 Tauri 2 空项目 + 开发环境

- [ ] `npm create tauri-app@latest` 初始化 Tauri 2 + React + Vite + TypeScript
- [ ] Rust 后端基础（Cargo.toml、tauri.conf.json 权限配置）
- [ ] ESLint + Prettier + TypeScript 严格模式
- [ ] Zustand 状态管理 + React Router 路由
- [ ] 主题系统骨架（亮/暗/跟随系统，CSS 变量驱动）
- [ ] SQLite 数据库初始化（Rust `rusqlite` + `tauri-plugin-sql`）
- [ ] 系统托盘（Rust `SystemTray` API，右键菜单：打开主窗口/退出）
- [ ] 首次安装引导（Onboarding：语言选择 + 主题选择）

**验收：** `cargo tauri dev` 启动后可见主窗口 + 系统托盘图标，亮暗主题可切换

---

### Iteration 1 — Orb 悬浮球

**交付物：** 桌面常驻透明悬浮球，可拖拽、可吸附、可唤起主窗口

- [ ] 透明悬浮球窗口（Tauri `transparent + alwaysOnTop + skipTaskbar`，64×64）
- [ ] 鼠标穿透 + Rust 端鼠标位置获取（`mouse_position` crate，借鉴 WindowPet）
- [ ] 悬浮球拖拽（前端 touch/mouse 事件 → Rust 窗口移动命令）
- [ ] 边缘吸附动画（松手后自动贴边 + 弹性动画）
- [ ] 全局快捷键唤起/隐藏（`Cmd/Ctrl+K`，借鉴 Onepoint）
- [ ] 悬浮球点击 → 主窗口 show/focus；主窗口失焦 → 自动隐藏（借鉴 Onepoint `blur` 事件）
- [ ] 系统托盘菜单扩展（新建对话 / 打开工具箱 / 显示悬浮球 / 退出）
- [ ] 悬浮球外观：默认圆形图标 + hover 发光效果

**验收：** 桌面可见透明小球，可拖拽到任意位置，松手自动贴边；点击展开主窗口，失焦自动收起；快捷键可唤起

---

### Iteration 2 — AI 对话核心

**交付物：** 可用的多模型 API 对话窗口，支持流式输出

- [ ] Provider 配置管理 UI（添加/编辑/删除 Provider：名称 + API Key + Base URL + 模型列表）
- [ ] API Key AES-256 加密存储（Rust 端加密，密钥派生自机器标识）
- [ ] Rust 端 Provider 适配器（OpenAI 兼容格式为基准，覆盖 Claude/Gemini/DeepSeek 等）
- [ ] 前端 Provider 适配层（统一接口，借鉴 xsai 架构）
- [ ] 对话管理（新建对话 / 对话列表 / 删除对话 / 重命名）
- [ ] 消息渲染（Markdown + 代码高亮 + 代码块复制按钮 + LaTeX 公式）
- [ ] 流式输出实时渲染（SSE 逐 token 渲染 + `chatDeltaCoalescer` 增量合并，借鉴 OpenHuman）
- [ ] 系统提示词编辑 + Prompt 模板库（内置 5 个常用模板）
- [ ] 模型选择器（Provider 内模型切换 + 参数调节：temperature / max_tokens）
- [ ] 对话数据持久化（SQLite：conversations + messages 表）

**验收：** 配置 OpenAI API Key 后可正常对话，流式逐字输出，对话历史可查看，切换模型可正常工作

---

### Iteration 3 — 工具箱基础

**交付物：** 可用的开发者工具箱，分类导航 + 搜索 + 10+ 基础工具

- [ ] 工具注册系统（统一 `ToolDefinition` 接口：id / name / category / icon / component）
- [ ] 工具箱首页（分类导航 + 搜索过滤 + 收藏置顶 + 最近使用）
- [ ] 开发工具（第一批）：
  - JSON 格式化/校验
  - Base64 编解码
  - URL 编解码
  - 正则测试器
  - 颜色转换（HEX/RGB/HSL）
  - 时间戳转换
  - Hash 计算（MD5/SHA1/SHA256）
  - UUID 生成
- [ ] 文本工具（第一批）：
  - Diff 对比
  - 字数/字符统计
  - Markdown 预览
  - 文本去重
- [ ] 工具使用历史记录（SQLite：tool_usage 表）
- [ ] 侧边栏导航：对话 / 工具箱 / 设置

**验收：** 工具箱可从侧边栏进入，所有工具可正常使用，搜索可过滤，收藏可置顶

---

### Iteration 4 — 工具箱扩展 + 剪贴板感知

**交付物：** 完整工具箱（20+ 工具）+ 剪贴板智能推荐

- [ ] 开发工具（第二批）：
  - 进制转换
  - JWT 解码
  - SQL 格式化
  - Cron 表达式解析
- [ ] 日常工具：
  - 二维码生成
  - 计算器
  - 单位换算
  - 密码生成（可配置长度/字符集）
  - IP 查询
- [ ] 多媒体工具：
  - 图片压缩/格式转换
  - SVG 预览/优化
- [ ] 剪贴板监听（Tauri `clipboard-manager` 插件）
- [ ] Smart Detection：剪贴板内容自动识别 → 推荐对应工具（JSON→JSON格式化，URL→URL编解码，颜色值→颜色转换…）
- [ ] 对话导出（Markdown / JSON）

**验收：** 工具箱 20+ 工具全部可用；复制 JSON 后悬浮球/主窗口自动提示"格式化 JSON"；对话可导出

---

### Iteration 5 — Mascot 形象 + 表情联动

**交付物：** Rive 动画 Mascot 形象，与对话状态联动

- [ ] Rive 运行时集成（`@rive-app/react-webgl2`）
- [ ] 设计/导入基础 `.riv` 动画文件（idle / listening / thinking / speaking / happy / concerned / curious / working / waving 共 9 种状态）
- [ ] 表情状态机（`MascotFSM`：对话生命周期事件 → 表情切换）
- [ ] Mascot 与 Agent 联动：
  - 用户输入 → listening
  - 等待首 token → thinking
  - 流式输出中 → speaking
  - 输出完成 → happy/idle
  - 错误 → concerned
- [ ] Mascot 显示位置：悬浮球内 / 主窗口侧边 / 对话输入框旁
- [ ] 形象配色方案（跟随主题色）
- [ ] jiwen 情感引擎集成（5 轴情感模型：bond/stubbornness/mood/anxiety/busyness，阈值触发主动行为）

**验收：** 对话时 Mascot 表情随状态实时变化；闲置时 idle 动画；情感轴可影响 Mascot 主动打招呼/提醒休息

---

### Iteration 6 — Agent 模式 + 工具调用

**交付物：** AI 可调用工具执行任务，Agent 模式分级

- [ ] Agent 模式分级（借鉴 OpenBitFun 7 级模式，简化为 3 级）：
  - **Quick**：单轮对话，无工具调用
  - **Standard**：多轮对话 + 基础工具（搜索/计算/翻译）
  - **Deep**：多轮对话 + 全部工具 + 自主规划
- [ ] Function Calling / Tool Use 协议实现（OpenAI 格式为基准）
- [ ] 内置工具（AI 可调用）：
  - `web_search`：网页搜索
  - `calculator`：数学计算
  - `translator`：翻译
  - `code_runner`：代码执行（Rust 端沙箱，受限执行）
- [ ] 工具调用 UI：消息中显示工具调用过程（工具名 → 参数 → 结果）
- [ ] Agent 会话 token 追踪 + 费用估算
- [ ] MCP 客户端集成（连接外部 MCP 服务器，扩展工具能力）

**验收：** Standard 模式下 AI 可主动调用计算器完成数学题；Deep 模式下可调用搜索+计算组合完成任务；MCP 工具可连接

---

### Iteration 7 — 记忆系统 + 高级功能

**交付物：** 跨会话记忆 + 上下文压缩 + TTS 预留

- [ ] 跨会话记忆系统（SQLite `memories` 表：摘要 + 向量嵌入 + 时间衰减权重）
- [ ] 对话上下文自动压缩（超过 token 阈值 → 自动摘要早期消息，保留近期完整）
- [ ] 记忆检索：新对话开始时，自动检索相关历史记忆注入上下文
- [ ] 记忆管理 UI（查看/删除/搜索记忆）
- [ ] TTS 语音合成预留接口（Provider 配置中增加 TTS 选项，后续迭代实现）
- [ ] 口型同步基础（viseme 映射表，TTS 输出 → Mascot 口型动画，预留）
- [ ] 自动更新（Tauri Updater：检测新版本 → 下载 → 安装）
- [ ] 对话导出扩展（PDF 格式）

**验收：** 多次对话后，AI 可引用之前对话的内容；长对话自动压缩不丢关键信息；自动更新可检测新版本

---

### Iteration 8 — 打包发布

**交付物：** 三平台安装包 + 签名公证

- [ ] Windows MSI / NSIS 安装包（Tauri bundler）
- [ ] macOS DMG / App Bundle（代码签名 + 公证）
- [ ] Linux AppImage / deb / rpm
- [ ] 应用图标 + 安装引导界面
- [ ] GitHub Release 自动发布（CI：tag → build → upload）
- [ ] 首次启动引导完善（Provider 配置向导 + 快捷键说明 + 功能介绍）

**验收：** 三平台安装包可正常安装/卸载/升级；macOS 签名公证通过；GitHub Release 流程跑通

---

### 迭代依赖关系

```
I0 脚手架
 └→ I1 悬浮球
     └→ I2 AI 对话
         ├→ I3 工具箱基础
         │   └→ I4 工具箱扩展
         └→ I5 Mascot 形象
             └→ I6 Agent 模式
                 └→ I7 记忆系统
                     └→ I8 打包发布
```

> I3 和 I5 可并行开发；I4 依赖 I3；I6 依赖 I2+I5；I7 依赖 I6

---

## 8. 安全设计

- API Key 使用 AES-256 加密存储，密钥派生自机器唯一标识
- 不收集任何用户数据，不发送遥测
- 对话数据仅存本地，用户可随时清除
- 工具运算均在本地完成，不依赖外部服务（IP 查询除外）
- Tauri 2 安全模型：前端与 Rust 后端通过 IPC 通信，所有命令需显式声明权限（`tauri.conf.json` `capabilities`）
- Rust 后端处理所有敏感操作（API 请求、加密、数据库），前端零权限访问系统资源
- 前端无法直接访问网络或文件系统，所有操作通过 Tauri Commands 由 Rust 端代理执行

---

## 9. 扩展性设计

- 工具采用插件化架构，未来支持第三方工具扩展
- Provider 适配器可扩展，社区可贡献新适配器
- Prompt 模板支持导入/导出，便于分享
- 预留 MCP (Model Context Protocol) 接口，支持工具调用能力

---

## 10. 开源项目调研与选型

根据本应用两大核心功能（API 问答 + 实用工具箱），对现有开源项目进行分类调研，评估可借鉴与复用的价值。

### 10.1 AI 对话客户端类

此类项目实现了多模型 API 接入、对话管理、流式渲染等核心能力，与本应用的问答模块高度匹配。

#### 10.1.1 LobeChat ⭐ 83.0k

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/lobehub/lobe-chat |
| Stars | **83.0k** ⭐ |
| 技术栈 | Next.js + React + TypeScript + Zustand |
| 协议 | MIT |

**核心能力：**
- 多 Provider 支持（OpenAI / Claude / Gemini / Ollama / DeepSeek 等）
- 插件系统（Function Calling / 工具调用）
- TTS 语音合成 + STT 语音识别
- 助手市场（300+ 预设助手）
- 主题系统 + 本地/服务端双模式
- 文件上传 + 图片识别

**可借鉴：** Provider 适配器架构、助手/模板市场设计、插件系统实现
**局限：** 基于 Next.js 的 Web 应用，非 Electron 桌面端；需自行改造为桌面应用

---

#### 10.1.2 NextChat (ChatGPT-Next-Web) ⭐ 88.8k

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/ChatGPTNextWeb/NextChat |
| Stars | **88.8k** ⭐ 🔥 |
| 技术栈 | Next.js + React + TypeScript + TailwindCSS |
| 协议 | MIT |

**核心能力：**
- 一键部署私有 ChatGPT 网页应用
- 多模型切换（GPT-4 / Claude / Gemini 等）
- Prompt 模板预设
- 对话压缩与上下文管理
- 流式输出 + Markdown 渲染
- 多语言支持

**可借鉴：** Prompt 模板系统、对话压缩策略、简洁的 UI 交互设计
**局限：** Web 应用非桌面端；功能相对精简，无工具集成

---

#### 10.1.3 ChatBox ⭐ 41.9k

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/Bin-Huang/chatbox |
| Stars | **41.9k** ⭐ |
| 技术栈 | Electron + React + TypeScript |
| 协议 | GPL-3.0 |

**核心能力：**
- **原生 Electron 桌面应用**，跨平台（Win/Mac/Linux）
- 多模型支持（OpenAI / Claude / Gemini / Ollama / DALL-E）
- 对话管理 + 数据本地存储
- API 配置管理
- 图片生成（DALL-E）

**可借鉴：** **Electron + React 桌面端架构**（与本应用技术栈最接近）、API 配置 UI、桌面端打包方案
**局限：** GPL-3.0 协议限制商业使用；无工具箱功能

---

#### 10.1.4 Cherry Studio ⭐ 52.3k

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/CherryHQ/cherry-studio |
| Stars | **52.3k** ⭐ |
| 技术栈 | Electron + React + TypeScript |
| 协议 | LGPL-3.0 |

**核心能力：**
- **原生 Electron 桌面应用**
- 300+ 预配置 AI 助手
- 多模型支持（含 DeepSeek-R1）
- MCP 服务集成（AI 自主调用工具）
- WebDAV 数据同步/备份
- 流程图可视化 + 代码高亮

**可借鉴：** **MCP 集成方案**、助手预设体系、桌面端数据同步方案
**局限：** LGPL-3.0 协议；MCP 调用外部工具而非内置工具箱

---

#### 10.1.5 Jan ⭐ 44.8k

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/janhq/jan |
| Stars | **44.8k** ⭐ |
| 技术栈 | Electron + React + TypeScript + Rust (Cortex) |
| 协议 | AGPL-3.0 |

**核心能力：**
- **原生 Electron 桌面应用**
- 本地模型运行（Llama / Mistral 等，基于 Cortex 引擎）
- API 模型接入（OpenAI 兼容格式）
- 完全离线可用
- 插件/扩展系统

**可借鉴：** 本地模型运行架构、离线优先设计、扩展系统
**局限：** AGPL-3.0 协议严格；侧重本地推理而非 API 聚合

---

#### 10.1.6 Open WebUI ⭐ 153.8k

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/open-webui/open-webui |
| Stars | **153.8k** ⭐ 🔥 |
| 技术栈 | Python (FastAPI) + SvelteKit |
| 协议 | MIT |

**核心能力：**
- 企业级 AI 交互平台
- 多后端支持（Ollama / OpenAI / Anthropic）
- 知识库 (RAG) + 联网搜索
- 多用户权限管理
- 文档上传与对话
- 函数调用 / 工具集成

**可借鉴：** RAG 知识库设计、多用户权限体系、工具调用框架
**局限：** Python 后端 + Web 应用，非桌面端；部署较重

---

### 10.2 开发者工具箱类

此类项目集成了大量开发者常用小工具，与本应用的工具箱模块高度匹配。

#### 10.2.1 IT-Tools ⭐ 40.8k

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/CorentinTh/it-tools |
| Stars | **40.8k** ⭐ |
| 技术栈 | Vue 3 + TypeScript + Vite |
| 协议 | GPL-3.0 |

**核心能力：**
- **100+ 实用工具**，覆盖加密、转换器、网络、文本、数学、数据等
- 工具分类导航 + 搜索
- 收藏常用工具
- 响应式 Web 界面
- Docker 一键部署

**工具清单（与本应用设计高度重叠）：**
- 加密：Token 生成、Hash 文本、BIP39、HMAC
- 转换器：日期、数字基数、颜色、ROM 数字
- 编解码：Base64、URL、HTML 实体
- 网络：IPv4 子网计算、IPv4 转换、URL 解析、DNS 查询
- 文本：Lorem Ipsum、统计、Diff、Markdown、正则测试、字符替换
- 数学：计算器、ETA、百分比
- 数据：JSON 格式化、JSON path、SQL 美化、XML 格式化、YAML 格式化、CSV ↔ JSON
- 视频：摄像头测试、SVG 预览

**可借鉴：** **工具分类体系与 UI 布局**（最接近本应用工具箱设计）、工具搜索与收藏交互、大量工具实现逻辑
**局限：** Vue 3 技术栈需适配为 React；GPL-3.0 协议；Web 应用非桌面端

---

#### 10.2.2 DevToys ⭐ 32.1k

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/DevToys-app/DevToys |
| Stars | **32.1k** ⭐ |
| 技术栈 | C# + WinUI 3 (Windows App SDK) |
| 协议 | MIT |

**核心能力：**
- **原生 Windows 桌面应用**（WinUI 3）
- 30+ 开发者工具
- Smart Detection：自动识别剪贴板内容推荐工具
- 侧边栏工具导航 + 搜索

**工具清单：**
- 转换器：JSON ↔ YAML、日期、数字基数
- 编解码：HTML、URL、Base64、GZip、JWT
- 格式化：JSON、SQL、XML、Markdown
- 生成器：UUID、Lorem Ipsum、哈希、密码、二维码
- 文本：正则测试、字符串转义、Diff、Markdown 预览
- 图像：压缩、转换

**可借鉴：** **Smart Detection（剪贴板智能识别）**设计理念、工具导航 UI
**局限：** C# / WinUI 3 技术栈不可直接复用；仅 Windows 平台

---

#### 10.2.3 CyberChef ⭐ 36.0k

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/gchq/CyberChef |
| Stars | **36.0k** ⭐ |
| 技术栈 | JavaScript (Vanilla) + Web |
| 协议 | Apache-2.0 |

**核心能力：**
- "网络瑞士军刀"，300+ 编解码/加密/数据分析操作
- **操作链（Recipe）**：多步骤串行处理，前一步输出作为下一步输入
- 拖拽式操作编排
- 离线可用（单 HTML 文件）

**可借鉴：** 操作链（多步骤串联处理）概念、海量编解码算法实现
**局限：** 纯 Web 应用；UI 风格偏技术向；操作链概念对普通用户偏复杂

---

### 10.3 效率启动器类

此类项目提供全局快捷唤起 + 插件化工具架构，与本应用的全局辅助功能匹配。

#### 10.3.1 Rubick ⭐ 10.0k

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/rubickCenter/rubick |
| Stars | **10.0k** ⭐ |
| 技术栈 | Electron + Vue 3 |
| 协议 | MIT |

**核心能力：**
- 类 uTools 开源替代品
- 全局快捷键唤起（Alt+R）
- 插件系统，兼容 uTools 插件生态
- 系统托盘 + 剪贴板管理
- 内置：编码小助手、JSON 查看、正则编辑器、颜色转换等

**可借鉴：** **全局快捷唤起机制**、插件化架构、uTools 插件兼容方案
**局限：** Vue 3 技术栈；定位为启动器而非完整助手应用

---

#### 10.3.2 ZTools ⭐ 1.2k

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/ZTools/ztools |
| Stars | **1.2k** ⭐（uTools 开源替代） |
| 技术栈 | Electron + React + TypeScript |
| 协议 | MIT |

**核心能力：**
- uTools 的开源实现
- 高性能应用启动器 + 插件平台
- 支持 macOS / Windows

**可借鉴：** Electron + React 技术栈的插件化启动器实现
**局限：** 项目较新，生态尚不成熟

---

### 10.4 选型建议与复用策略

#### 推荐方案：组合借鉴，自主构建

鉴于本应用的独特定位（**AI 问答 + 工具箱一体化桌面应用**），目前没有单一开源项目能完全覆盖，建议采用以下策略：

| 功能模块 | 主要借鉴项目 | 复用方式 |
|---------|------------|---------|
| **Electron 桌面端框架** | ChatBox / Cherry Studio | 参考项目结构与打包配置 |
| **Provider 适配器架构** | LobeChat | 借鉴适配器模式，用 TypeScript 重写 |
| **对话管理 + 流式渲染** | LobeChat / NextChat | 参考对话状态管理与 SSE 处理 |
| **MCP 工具调用** | Cherry Studio | 参考集成方案 |
| **工具箱 UI 与分类** | IT-Tools | 借鉴分类体系 + 搜索 + 收藏交互 |
| **工具实现逻辑** | IT-Tools / DevToys | 移植具体工具的算法与逻辑到 React 组件 |
| **剪贴板智能识别** | DevToys | 实现 Smart Detection 功能 |
| **全局快捷唤起** | Rubick | 参考全局快捷键 + 剪贴板监听实现 |
| **Prompt 模板系统** | NextChat / LobeChat | 参考模板数据结构与变量替换 |
| **本地模型运行** | Jan | 可选，未来扩展 |

#### 协议兼容性评估

| 项目 | 协议 | 可否闭源使用 | 建议 |
|------|------|------------|------|
| LobeChat | MIT | ✅ 可以 | 参考架构与设计，可自由使用 |
| NextChat | MIT | ✅ 可以 | 参考设计，可自由使用 |
| ChatBox | GPL-3.0 | ❌ 需开源 | 仅参考思路，不复制代码 |
| Cherry Studio | LGPL-3.0 | ⚠️ 部分限制 | 可动态链接，但修改需开源 |
| Jan | AGPL-3.0 | ❌ 需开源 | 仅参考思路，不复制代码 |
| IT-Tools | GPL-3.0 | ❌ 需开源 | 参考设计思路，工具逻辑自行实现 |
| DevToys | MIT | ✅ 可以 | 可自由参考设计 |
| CyberChef | Apache-2.0 | ✅ 可以 | 可复用算法实现 |
| Rubick | MIT | ✅ 可以 | 可自由参考 |
| Open WebUI | MIT | ✅ 可以 | 可自由参考 |

> **结论：** 优先借鉴 MIT / Apache-2.0 协议项目（LobeChat、NextChat、DevToys、CyberChef、Rubick）的架构与实现；GPL/AGPL 项目仅参考设计思路，不直接复制代码。

---

## 11. Orb 悬浮球开源项目调研

Orb 悬浮球（Floating Orb / Floating Ball）是一种常驻桌面、可拖拽的圆形快捷入口，点击/双击展开主界面，最小化时收缩为悬浮球。本应用计划支持此交互模式。

### 11.1 直接相关项目

#### 11.1.1 Onepoint ⭐ 315 — 最匹配

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/onepointAI/onepoint |
| Stars | **315** ⭐ |
| Forks | 43 |
| 技术栈 | Electron + Vue 3 |
| 协议 | MIT |

**核心能力：**
- **专为 AI 助手设计的悬浮球**，灵感来自苹果辅助触控（AssistiveTouch）
- 桌面悬浮球 → 点击展开 AI 对话窗口
- 不占用桌面空间，使用时不影响其他操作
- 支持 ChatGPT / Claude 等 API 接入
- 内置 Prompt 模板
- 透明无边框悬浮窗 + 丝滑动画

**可借鉴：** **悬浮球 UI 交互全流程**（球态 → 展开态 → 收缩态）、透明无边框窗口实现、AI 助手悬浮球场景设计
**局限：** Vue 3 技术栈；功能相对简单，无工具箱

---

#### 11.1.2 STranslate ⭐ 8.2k — 悬浮球交互典范

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/ZGGSONG/STranslate |
| Stars | **8.2k** ⭐ |
| 技术栈 | C# + WPF (.NET) |
| 协议 | MIT |

**核心能力：**
- **悬浮球 + 鼠标划词**双模式交互
- 悬浮球可拖拽、可吸附屏幕边缘
- 鼠标悬停展开快捷面板
- 划词翻译 / 截图翻译 / OCR 识别
- 支持多种翻译服务 + OpenAI 接口
- 即开即用、即用即走理念

**可借鉴：** **悬浮球吸附边缘动画**、**鼠标划词触发**机制、悬浮球与主窗口的联动交互
**局限：** C# / WPF 技术栈不可直接复用；仅 Windows

---

#### 11.1.3 Rubick ⭐ 10.0k — 全局快捷唤起

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/rubickCenter/rubick |
| Stars | **10,029** ⭐ |
| 技术栈 | Electron + Vue 3 |
| 协议 | MIT |

**核心能力：**
- 类 uTools 的效率启动器
- **全局快捷键唤起悬浮球/搜索框**（Alt+R）
- 插件化架构，兼容 uTools 插件生态
- 系统托盘 + 剪贴板管理
- 内置开发者小工具（JSON 查看、正则编辑器、颜色转换等）

**可借鉴：** **Electron 全局快捷键唤起**实现、**插件化悬浮球**架构、uTools 插件兼容方案
**局限：** Vue 3 技术栈；定位为启动器而非 AI 助手

---

### 11.2 Electron 悬浮球实现方案（技术参考）

以上项目及社区实践表明，Electron 实现桌面悬浮球的核心技术方案如下：

```typescript
const floatingBall = new BrowserWindow({
  width: 56,
  height: 56,
  frame: false,
  transparent: true,
  resizable: false,
  alwaysOnTop: true,
  skipTaskbar: true,
  hasShadow: false,
  webPreferences: {
    nodeIntegration: false,
    contextIsolation: true,
    preload: path.join(__dirname, 'preload.js'),
  },
})
```

**关键实现要点：**

| 要点 | 实现方式 |
|------|---------|
| 透明无边框 | `frame: false` + `transparent: true` + CSS 透明背景 |
| 始终置顶 | `alwaysOnTop: true` |
| 不占任务栏 | `skipTaskbar: true` |
| 自由拖拽 | `-webkit-app-region: drag` 或 IPC 监听鼠标事件手动移动 |
| 吸附边缘 | 拖拽结束时计算最近边缘，`setPosition` 动画吸附 |
| 点击展开 | 悬浮球窗口 IPC 通知主窗口 `show()` / `focus()` |
| 收缩为球 | 主窗口 `hide()` 后显示悬浮球窗口 |
| 丝滑动画 | CSS `transition` / `requestAnimationFrame` 驱动尺寸与位置变化 |

### 11.3 悬浮球交互设计（本应用规划）

```
┌─────────────────────────────────────────────────┐
│                    桌面                           │
│                                                  │
│         ┌──────┐                                 │
│         │  ●   │  ← 悬浮球（常驻，可拖拽）       │
│         └──┬───┘                                 │
│            │ 单击 / 双击                          │
│            ▼                                      │
│   ┌────────────────────┐                         │
│   │  主窗口（展开态）    │                         │
│   │  ┌──────────────┐  │                         │
│   │  │ AI 对话 / 工具 │  │                         │
│   │  └──────────────┘  │                         │
│   │  [最小化→收缩为球]  │                         │
│   └────────────────────┘                         │
│                                                  │
│   悬浮球右键菜单：                                 │
│   · 新建对话                                      │
│   · 快速工具                                      │
│   · 剪贴板内容识别                                 │
│   · 退出                                          │
└─────────────────────────────────────────────────┘
```

**交互状态机：**

```
[悬浮球态] ──单击──→ [展开主窗口]
[悬浮球态] ──右键──→ [快捷菜单]
[展开主窗口] ──最小化──→ [悬浮球态]
[展开主窗口] ──关闭──→ [悬浮球态]（默认）或 [退出]
[任意态] ──全局快捷键──→ 切换 [悬浮球态] ↔ [展开主窗口]
```

### 11.4 选型建议

| 需求 | 推荐参考项目 | 理由 |
|------|------------|------|
| **AI 助手悬浮球全流程** | Onepoint | 场景最匹配，MIT 协议可自由参考 |
| **悬浮球边缘吸附 + 划词** | STranslate | 交互体验最佳，参考设计思路 |
| **全局快捷键唤起** | Rubick | Electron 实现最成熟，MIT 协议 |
| **Electron 悬浮球底层实现** | 社区教程 + Rubick | 透明窗口 + 拖拽 + 动画方案通用 |

> **结论：** Onepoint 是目前最匹配"AI 助手 + 悬浮球"场景的开源项目，MIT 协议可自由借鉴。STranslate 的边缘吸附与划词交互值得参考设计。底层实现基于 Electron 透明窗口方案，技术成熟度高。

---

## 12. 轻平台轻量级框架调研（非 Electron 方向）

Electron 内嵌完整 Chromium，导致安装包 100MB+、内存占用高。以下调研不依赖重量级浏览器引擎的替代方案。

### 12.1 框架横向对比

| 维度 | Electron | Tauri | Wails | Neutralino.js | Flutter Desktop |
|------|----------|-------|-------|---------------|----------------|
| **后端语言** | Node.js | Rust | Go | C++ (系统原生) | Dart |
| **渲染引擎** | 内嵌 Chromium | 系统 WebView2 | 系统 WebView2 | 系统 WebView | Skia 自绘引擎 |
| **前端技术** | Web (任意) | Web (任意) | Web (任意) | Web (任意) | Dart Widget |
| **安装包体积** | 100~200 MB | **3~8 MB** | **8~15 MB** | **2~5 MB** | 15~30 MB |
| **内存占用** | 200~500 MB | **30~80 MB** | **40~100 MB** | **20~50 MB** | 50~120 MB |
| **启动速度** | 慢 (2~5s) | **快 (<1s)** | **快 (<1s)** | **快 (<1s)** | 快 (<1s) |
| **跨平台** | Win/Mac/Linux | Win/Mac/Linux/Mobile | Win/Mac/Linux | Win/Mac/Linux | Win/Mac/Linux/Mobile/Web |
| **协议** | MIT | MIT / Apache-2.0 | MIT | MIT | BSD-3.0 |
| **GitHub Stars** | 115k+ | **104k+** | 25k+ | 6k+ | 170k+ (SDK) |
| **生态成熟度** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐⭐ |
| **Rust/Go 门槛** | 无 | Rust 学习曲线 | Go 学习曲线 | 无 | Dart 学习曲线 |

> **核心差异：** Electron 打包完整 Chromium（~120MB），Tauri/Wails/Neutralino 复用系统已安装的 WebView2 运行时，Flutter 使用 Skia 自绘引擎——三者均不内嵌浏览器。

---

### 12.2 Tauri（重点推荐）⭐ 104k+

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/tauri-apps/tauri |
| Stars | 104k+ |
| 当前版本 | Tauri 2.0（2024.10 发布，支持桌面+移动端） |
| 协议 | MIT / Apache-2.0 |
| 官网 | https://tauri.app |

**核心优势：**
- **安装包仅 3~8 MB**，相比 Electron 缩小 90%+
- **内存占用降低 50%+**，启动速度 <1s
- Rust 后端：内存安全、零 GC、高性能
- 复用系统 WebView2（Windows/macOS）/ WebKitGTK（Linux），不内嵌浏览器
- Tauri 2.0 支持桌面 + Android + iOS 统一代码库
- 安全沙箱：IPC 命令需显式声明权限
- API Key 可安全存储于 Rust 后端，零 CORS 问题
- 完善的自动更新、系统托盘、原生菜单等桌面 API

**已有 AI 助手实践：**
- 社区已实现 Tauri + React AI Chat 桌面应用，打包仅 **3MB**，支持流式输出
- Chat Nio：基于 Tauri 的 AI 聚合聊天平台
- HuLa：基于 Tauri + Vue3 的即时通讯应用（8.4k stars），验证了 Tauri 复杂 UI 场景

**对本项目的适配性：**
- ✅ 前端仍可用 React + TypeScript，迁移成本低
- ✅ Rust 后端处理 API 请求、加密、数据库，性能远超 Node.js
- ✅ 悬浮球透明窗口：Tauri 2.0 支持 `transparent` + `decorations: false`
- ✅ 系统托盘、全局快捷键、剪贴板等桌面 API 均已内置
- ⚠️ SQLite 需通过 Rust 侧插件（tauri-plugin-sql）操作
- ⚠️ Rust 学习曲线：后端逻辑需用 Rust 编写

---

### 12.3 Wails ⭐ 25k+

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/wailsapp/wails |
| Stars | 25k+ |
| 当前版本 | Wails v3（开发中） |
| 协议 | MIT |
| 官网 | https://wails.io |

**核心优势：**
- Go 后端：语法简洁、并发原生支持、学习曲线平缓
- 复用系统 WebView2，安装包 8~15 MB
- 前端支持 React / Vue / Svelte
- Go 调用系统 API 简单（CGO / syscall）
- Wails 3 新增：原生对话框、系统托盘、菜单、事件系统

**已有实践：**
- DevToolKit：基于 Wails + Vue 的开发者工具集
- 社区已实现 Wails + React AI 智能体应用（Wails3 + Eino ADK）
- 多款聊天、图床等桌面应用

**对本项目的适配性：**
- ✅ Go 后端上手快，适合快速开发
- ✅ 前端仍可用 React + TypeScript
- ✅ Go 的 goroutine 天然适合并发 API 请求
- ⚠️ Wails 3 仍在开发中，API 可能变动
- ⚠️ 生态与 Tauri 相比略小
- ⚠️ 悬浮球透明窗口支持需验证

---

### 12.4 Neutralino.js ⭐ 6k+

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/neutralinojs/neutralinojs |
| Stars | 6k+ |
| 协议 | MIT |
| 官网 | https://neutralino.js.org |

**核心优势：**
- 安装包最小（2~5 MB），无需编译步骤
- 复用系统 WebView，极轻量
- 前端任意 Web 框架
- 无需 Node.js / Rust / Go，纯 JS 开发

**局限：**
- ⚠️ 生态较小，桌面 API 不如 Tauri/Wails 丰富
- ⚠️ 无原生后端语言，复杂逻辑（加密、数据库）需依赖扩展或 WASM
- ⚠️ 性能敏感场景不如 Tauri（Rust）/ Wails（Go）
- ⚠️ 不推荐用于功能丰富的助手类应用

---

### 12.5 Flutter Desktop ⭐ 170k+ (SDK)

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/flutter/flutter |
| Stars | 170k+ |
| 协议 | BSD-3.0 |

**核心优势：**
- Skia 自绘引擎，不依赖系统 WebView，UI 一致性最佳
- 桌面 + 移动 + Web 统一代码库
- 丰富的 Widget 体系，UI 表现力强
- Dart 语言类型安全、支持热重载

**局限：**
- ⚠️ 非 Web 技术栈，无法复用 React 生态
- ⚠️ Markdown 渲染、代码高亮等 Web 生态组件需重新寻找/开发
- ⚠️ 与 HTTP API 交互不如 Web 技术栈自然
- ⚠️ 桌面端成熟度仍不如移动端

---

### 12.6 Pake / PakePlus — Tauri 快速封装工具

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/Sjj1024/PakePlus |
| 协议 | MIT |

**定位：** 将任意网页快速打包为轻量桌面应用（基于 Tauri），安装包仅 5MB 左右

**对本项目的价值：** 如果前端先以 Web 应用开发完成，可用 PakePlus 快速出桌面 MVP，后续再逐步迁移为原生 Tauri 应用

---

### 12.7 选型建议

#### 推荐：Tauri 2.0 + React + TypeScript

| 决策因素 | 分析 |
|---------|------|
| **体积与性能** | Tauri 安装包 3~8MB vs Electron 100~200MB，内存降低 50%+ |
| **前端复用** | React + TypeScript 生态完整保留，UI 组件库（Ant Design）可直接使用 |
| **后端能力** | Rust 处理 API 请求、加密存储、SQLite 操作，性能与安全性远超 Node.js |
| **桌面 API** | 系统托盘、全局快捷键、透明窗口、剪贴板、文件系统等均已内置 |
| **悬浮球** | Tauri 2.0 支持 `transparent` 窗口，可实现悬浮球交互 |
| **跨平台** | Win/Mac/Linux + 移动端（Tauri 2.0），未来可扩展 |
| **协议** | MIT / Apache-2.0，无商业限制 |
| **生态** | 104k+ Stars，社区活跃，插件体系日趋完善 |

#### 技术栈调整（原 Electron → Tauri）

| 层级 | 原方案 | 新方案 |
|------|--------|--------|
| 框架 | Electron 28+ | **Tauri 2.0** |
| 后端 | Node.js (主进程) | **Rust** |
| 前端 | React 18 + TypeScript | React 18 + TypeScript（不变） |
| UI 库 | Ant Design 5 | Ant Design 5（不变） |
| 状态管理 | Zustand | Zustand（不变） |
| 构建工具 | Vite | Vite（不变） |
| 本地存储 | SQLite (better-sqlite3) | **SQLite (tauri-plugin-sql)** |
| IPC 通信 | Electron IPC | **Tauri Commands / Events** |
| 加密 | Node.js crypto | **Rust aes-gcm / keyring** |
| HTTP 请求 | Node.js fetch | **Rust reqwest**（零 CORS） |

#### 渐进迁移策略

```
Phase 1: 前端 Web 应用独立开发（React + Vite）
    ↓ 验证 UI 与交互逻辑
Phase 2: Tauri 壳层集成，前端代码零改动接入
    ↓ 验证桌面 API（托盘、快捷键、悬浮球）
Phase 3: Rust 后端逐步替换 Node.js 逻辑
    ↓ API 请求 → Rust reqwest
    ↓ 加密存储 → Rust keyring
    ↓ SQLite → tauri-plugin-sql
Phase 4: 完整 Tauri 应用打包发布
```

> **结论：** Tauri 2.0 是当前最佳轻量级替代方案，在保留 React 前端生态的同时，以 Rust 后端获得极致的体积与性能优势。Wails (Go) 作为备选，适合 Rust 学习曲线过高时降级使用。Neutralino.js 和 Flutter Desktop 不推荐用于本项目。

---

## 13. 助手类开源项目调研（AI 问答 + 工具一体化）

以下项目与本应用定位最接近——**集 AI 对话能力与实用工具/系统能力于一体的桌面助手**，而非单纯的聊天客户端或工具箱。

### 13.1 OpenHuman ⭐ 40,468 — 最匹配

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/tinyhumansai/openhuman |
| Stars | **40,468** ⭐ 🔥（5天突破 23k，持续增长） |
| 技术栈 | **Rust + Tauri + React + TypeScript** |
| 协议 | GPL-3.0 |

**核心能力：**
- **本地优先 AI 个人助手**，定位 "Personal AI super intelligence"
- 桌面 Mascot（会说话的 AI 形象），非纯聊天框
- **长效 AI 记忆**：跨会话记住用户偏好、习惯、上下文
- **自动联动 118+ 应用**：浏览器、日历、邮件、笔记、代码编辑器等一键互通
- 本地数据优先，隐私保护
- 多模型支持

**与本项目匹配度：** ⭐⭐⭐⭐⭐
- ✅ 技术栈完全一致（Tauri + React + TypeScript）
- ✅ AI 助手 + 工具联动的一体化设计理念
- ✅ 桌面形象（Mascot）可参考悬浮球交互
- ✅ 长效记忆机制值得借鉴
- ⚠️ GPL-3.0 协议限制商业使用
- ⚠️ 侧重"AI 联动应用"而非"内置工具箱"

---

### 13.2 Cherry Studio ⭐ 52.3k — AI 助手 + MCP 工具调用

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/CherryHQ/cherry-studio |
| Stars | **52,332** ⭐ |
| 技术栈 | **Electron + React + TypeScript** |
| 协议 | LGPL-3.0 |

**核心能力：**
- 300+ 预配置 AI 助手
- 多模型支持（含 DeepSeek-R1）
- **MCP 服务集成**：AI 可自主调用外部工具（文件系统、数据库、API 等）
- WebDAV 数据同步/备份
- 流程图可视化 + 代码高亮
- 知识库管理

**与本项目匹配度：** ⭐⭐⭐⭐
- ✅ AI 助手 + 工具调用（MCP）一体化
- ✅ Electron + React 技术栈
- ✅ MCP 集成方案可直接参考
- ⚠️ LGPL-3.0 协议
- ⚠️ 依赖 Electron（重量级）
- ⚠️ MCP 调用外部工具而非内置工具箱

---

### 13.3 ToolKnit Desktop ⭐ 1.1k — 工具箱 + AI 辅助

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/LBBJava/ToolKnit-Desktop |
| Stars | **1.1k** ⭐ |
| 技术栈 | **Tauri + Vue 3** |
| 协议 | 开源 |

**核心能力：**
- **本地桌面工具箱**：PDF 压缩/转换、图片处理、音视频处理、AI 辅助
- 一个 exe 搞定四件套（PDF + 图片 + 音视频 + AI）
- 所有处理本地完成，不上传文件
- 基于 Tauri，安装包轻量

**与本项目匹配度：** ⭐⭐⭐⭐
- ✅ 工具箱 + AI 一体化设计
- ✅ 基于 Tauri（轻量级）
- ✅ 本地处理优先
- ⚠️ Vue 3 技术栈
- ⚠️ 侧重文件处理工具，开发者工具较少
- ⚠️ AI 能力较简单，无多模型对话

---

### 13.4 AiPy (AiPyApp) ⭐ 3.2k — AI + Python 执行器

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/aipyapp/aipy |
| Stars | **3.2k** ⭐ |
| 技术栈 | Python + Qt (PySide6) |
| 协议 | 开源 |

**核心能力：**
- **AI + Python 执行器**：AI 自动编写 Python 代码并执行
- Task → Plan → Code → Execute → Feedback 循环
- 本地数据分析、文件处理、应用控制
- 完全本地化部署，不依赖云服务
- 跨平台（Win/Mac/Linux）

**与本项目匹配度：** ⭐⭐⭐
- ✅ AI + 工具执行一体化理念
- ✅ Python-Use 范式值得借鉴（AI 自动调用工具）
- ⚠️ Python + Qt 技术栈，无法直接复用
- ⚠️ 侧重代码执行，非对话 + 工具箱模式
- ⚠️ 无内置工具箱

---

### 13.5 Open Minis ⭐ 4.3k — 本地 AI Agent

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/nicepkg/openminis |
| Stars | **4.3k** ⭐ |
| 技术栈 | React Native / Flutter |
| 协议 | 开源 |

**核心能力：**
- **On-device AI Agent**：AI 直接运行在用户设备上
- 调用系统能力：网页浏览、日程管理、健康数据、智能家居控制
- 隐私优先，数据不离开设备

**与本项目匹配度：** ⭐⭐⭐
- ✅ 本地 AI Agent + 系统能力调用理念
- ✅ 隐私优先设计
- ⚠️ 侧重移动端
- ⚠️ 非 Web 技术栈桌面应用

---

### 13.6 uTools / Rubick / ZTools — 插件化效率启动器

| 项目 | 类型 | 仓库 | 技术栈 | 协议 |
|------|------|------|--------|------|
| uTools | 商业闭源 | https://u.tools | Electron | — |
| Rubick | 开源替代 | https://github.com/rubickCenter/rubick | Electron + Vue 3 | MIT |
| ZTools | 开源替代 | https://github.com/nicepkg/ZTools | Electron + React | MIT |

**核心能力：**
- **全局快捷键唤起** → 搜索 → 即用即走
- **插件化架构**：2000+ 插件（uTools 生态）
- 内置 AI 对话、编码小助手、JSON 查看、正则编辑器等
- 剪贴板监听 + 智能识别
- ZTools 已内置 **MCP 服务器**，AI 客户端可调用启动器命令

**与本项目匹配度：** ⭐⭐⭐⭐
- ✅ **插件化工具 + AI 对话**的一体化交互最接近本项目
- ✅ 全局唤起 + 剪贴板识别 + 即用即走理念
- ✅ ZTools 的 MCP 集成方案
- ⚠️ 依赖 Electron
- ⚠️ 定位为"启动器"而非"助手"，AI 对话非核心

---

### 13.7 Onepoint ⭐ 315 — AI 悬浮球助手

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/onepointAI/onepoint |
| Stars | **315** ⭐ |
| Forks | 43 |
| 技术栈 | Electron + Vue 3 |
| 协议 | MIT |

**核心能力：**
- 桌面悬浮球 → 点击展开 AI 对话窗口
- 支持 ChatGPT / Claude 等 API
- 内置 Prompt 模板
- 透明无边框悬浮窗 + 丝滑动画

**与本项目匹配度：** ⭐⭐⭐⭐
- ✅ AI 助手 + 悬浮球交互
- ✅ MIT 协议
- ⚠️ 功能较简单，无工具箱
- ⚠️ Electron + Vue 3

---

### 13.8 综合对比与选型建议

| 项目 | Stars | AI 对话 | 内置工具 | 悬浮球 | 轻平台 | 轻框架 | 协议 | 综合匹配度 |
|------|-------|---------|---------|--------|--------|--------|------|-----------|
| **OpenHuman** | 40,468 | ✅ 长效记忆 | ✅ 118+应用联动 | ✅ Mascot | Tauri+React | 轻量 | GPL-3.0 | ⭐⭐⭐⭐⭐ |
| **Cherry Studio** | 52,332 | ✅ 300+助手 | ✅ MCP工具调用 | ❌ | Electron+React | 重 | LGPL-3.0 | ⭐⭐⭐⭐ |
| **ToolKnit** | 1,100 | ⚠️ 简单AI | ✅ PDF/图片/音视频 | ❌ | Tauri+Vue | 轻量 | 开源 | ⭐⭐⭐⭐ |
| **uTools/ZTools** | 10,029 | ⚠️ 插件AI | ✅ 2000+插件 | ✅ 全局唤起 | Electron+React | 重 | MIT | ⭐⭐⭐⭐ |
| **Onepoint** | 315 | ✅ 多模型 | ❌ | ✅ 悬浮球 | Electron+Vue | 重 | MIT | ⭐⭐⭐⭐ |
| **AiPy** | 3,200 | ✅ 代码执行 | ❌ | ❌ | Python+Qt | 中 | 开源 | ⭐⭐⭐ |
| **Open Minis** | 4,300 | ✅ 本地Agent | ✅ 系统能力 | ❌ | RN/Flutter | 中 | 开源 | ⭐⭐⭐ |
| **yoji** | 663 | ✅ 情感感知 | ✅ MCP工具调用 | ✅ 悬浮组件 | TS 跨平台 | 中 | MIT | ⭐⭐⭐⭐⭐ |

#### 核心结论

> **OpenHuman（40,468 Stars）是当前最匹配的助手类开源项目**——技术栈（Tauri + React + TS）与本项目完全一致，且实现了 AI 助手 + 工具联动 + 桌面形象的一体化设计。但 GPL-3.0 协议限制商业使用，应仅参考架构设计思路。
>
> **yoji（663 Stars）是目前发现的最接近目标产品的项目**——悬浮组件 + AI 对话 + MCP 工具调用 + 情感感知，MIT 协议可自由参考。
>
> **组合借鉴策略：**
> - **整体架构 + AI 记忆** → 参考 OpenHuman
> - **悬浮组件 + AI 联动 + 情感感知** → 参考 yoji
> - **MCP 工具调用** → 参考 Cherry Studio / yoji
> - **插件化工具生态** → 参考 uTools / ZTools
> - **悬浮球交互** → 参考 Onepoint
> - **本地文件处理工具** → 参考 ToolKnit
> - **AI 自动执行工具** → 参考 AiPy 的 Python-Use 范式

---

## 14. Orb / 悬浮球 / 桌面宠物类开源项目调研

本应用计划实现 **Orb 悬浮球**——常驻桌面、可拖拽、点击展开主界面的快捷入口。以下项目均实现了类似的"桌面常驻形象"交互。

### 14.1 WindowPet ⭐ 最匹配技术栈

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/SeakMengs/WindowPet |
| Stars | **670** ⭐ |
| 技术栈 | **Tauri + React + TypeScript** |
| 协议 | MIT |
| 最近更新 | 2026-09-27 |

**核心能力：**
- **桌面宠物叠加应用**——宠物/动漫角色常驻桌面，悬浮显示
- 不遮挡窗口、不干扰正常操作
- 基于 Tauri 透明窗口实现
- 支持 Windows / macOS / Linux
- 可自定义宠物形象（GIF / Sprite 动画）

**与本项目匹配度：** ⭐⭐⭐⭐⭐
- ✅ 技术栈完全一致（Tauri + React + TypeScript）
- ✅ **透明悬浮窗口**实现方案可直接复用
- ✅ MIT 协议，可自由借鉴代码
- ✅ 不遮挡窗口、不干扰操作——与 Orb 悬浮球理念一致
- ⚠️ 定位为桌宠，无 AI 对话功能

---

### 14.2 XSUN_DESKTOP_PET — Tauri 2 + Rust 桌宠

| 项目 | 信息 |
|------|------|
| 仓库 | https://gitee.com/sunlinglei/xsun_desktop_pet |
| 技术栈 | **Rust + Tauri 2** |
| 协议 | 开源 |

**核心能力：**
- 基于 Rust + Tauri 2 开发的桌面宠物应用
- 桌面常驻、透明窗口、动画驱动
- 跨平台（Win/Mac/Linux）

**与本项目匹配度：** ⭐⭐⭐⭐
- ✅ Tauri 2 技术栈
- ✅ Rust 后端透明窗口实现
- ⚠️ Gitee 托管，社区较小
- ⚠️ 学习项目，功能较简单

---

### 14.3 Desktop Pixel Pet v2.0 — 桌宠 + AI 实用功能

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/CanFlyhang/Desktop-Pixel-Pet |
| Stars | **145** ⭐ |
| 技术栈 | HTML + JS (Tauri) |
| 协议 | MIT |
| 最近更新 | 2026-09-14 |

**核心能力：**
- 像素动画驱动的桌面陪伴应用
- **v2.0 新增：AI 疲劳监测 + 多感官交互**
- 宠物常驻桌面，悬浮显示不遮挡窗口
- 轻量、可扩展

**与本项目匹配度：** ⭐⭐⭐⭐
- ✅ **桌宠 + AI 功能**一体化——与本项目"悬浮球 + AI 助手"理念最接近
- ✅ Tauri 轻量级框架
- ✅ AI 疲劳监测展示了"桌宠 + 实用 AI 能力"的组合范式
- ⚠️ AI 能力偏向监测，非对话/工具箱

---

### 14.4 AIRI — AI 多模态桌面伴侣

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/moeru-ai/airi |
| Stars | **49,949** ⭐ 🔥 |
| 技术栈 | TypeScript + Vue (Web/Electron 桌面端) |
| 协议 | MIT |
| 最近更新 | 2026-10-03 |

**核心能力：**
- **AI 虚拟角色灵魂项目**——可互动的数字伴侣
- 多模态交互：聊天、语音对话、玩游戏（Minecraft、Factorio）
- 虚拟形象渲染（Live2D / 3D 模型）
- 支持 Web / macOS / Windows
- 自托管，用户完全拥有数据

**与本项目匹配度：** ⭐⭐⭐⭐
- ✅ **AI 伴侣 + 桌面形象**一体化设计
- ✅ 多模态交互（语音、游戏操作）理念值得借鉴
- ✅ 虚拟形象渲染方案（Live2D）可参考
- ⚠️ Vue + Electron 技术栈
- ⚠️ 定位为"伴侣"而非"助手+工具箱"

---

### 14.5 DyberPet — 桌面宠物开发框架

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/ChaozhongLiu/DyberPet |
| Stars | **989** ⭐ |
| 技术栈 | Python + PySide6 (Qt) |
| 协议 | GPL-3.0 |
| 最近更新 | 2026-10-02 |

**核心能力：**
- **桌面宠物开发框架**——提供全套核心工具
- 自定义屏幕上的动画桌面陪伴角色
- 支持多种动画格式、交互事件、状态机
- 插件化扩展

**与本项目匹配度：** ⭐⭐⭐
- ✅ 桌面宠物**状态机 + 动画驱动**架构值得参考
- ✅ 插件化扩展设计
- ⚠️ Python + Qt 技术栈，无法直接复用
- ⚠️ 无 AI 能力

---

### 14.6 Onepoint — AI 悬浮球助手（已调研）

> 详见第 11 章 11.1.1 节。最匹配"AI 助手 + 悬浮球"场景，但基于 Electron。

---

### 14.7 STranslate — 悬浮球交互典范（已调研）

> 详见第 11 章 11.1.2 节。悬浮球边缘吸附 + 划词交互最佳，但 C#/WPF 技术栈。

---

### 14.8 OpenBitFun — Rust Agent 桌面应用

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/GCWing/OpenBitFun |
| Stars | **2,372** ⭐ |
| 技术栈 | **Rust** (Tauri 桌面端) |
| 协议 | MIT |
| 最近更新 | 2026-10-02 |

**核心能力：**
- 高性能 Agent Runtime（Rust 编写）+ 桌面应用
- Code Agent + 通用 Agent 双模式
- Computer Use Agent（操控电脑）
- Agent Teams 多智能体协作
- Vibe Coding 支持

**与本项目匹配度：** ⭐⭐⭐⭐
- ✅ Rust 后端 + Tauri 桌面端，轻量级
- ✅ MIT 协议
- ✅ Agent 多智能体协作架构值得借鉴
- ✅ Computer Use Agent 展示了 AI 操控桌面的范式
- ⚠️ 定位为 Code Agent IDE，非助手+工具箱

---

### 14.9 Awesome AI Companion — AI 伴侣项目大全

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/DasterProkio/awesome-ai-companion |
| Stars | **852** ⭐ |
| 技术栈 | — (Awesome List 索引) |
| 协议 | CC0-1.0 |
| 最近更新 | 2026-10-03 |

**核心能力：**
- **AI 伴侣 / 桌宠开源项目大全**——收录了所有主流 AI Companion 项目
- 涵盖：长期记忆、主动聊天、桌面宠物、Live2D、语音对话
- 按功能分类：记忆系统、情感引擎、语音管道、桌面形象
- 持续更新，2026-10-03 仍活跃

**与本项目匹配度：** ⭐⭐⭐⭐⭐（作为参考索引）
- ✅ **最重要的参考索引**——从中可发现更多细分项目
- ✅ 涵盖了 AI 伴侣的所有技术维度
- ✅ CC0 协议，完全自由

---

### 14.10 OpenHuman Mascot — Rive 动画角色（已分析）

> 详见第 13 章。Mascot 是窗口内嵌的 Rive 动画角色（18 种表情 + 唇形同步），**非独立悬浮窗**，但动画架构（Rive + 状态机 + dock/stage 过渡）值得借鉴。

---

### 14.11 综合对比与选型建议

| 项目 | Stars | 悬浮/常驻 | AI 能力 | 技术栈 | 框架 | 协议 | 综合匹配度 |
|------|-------|----------|---------|--------|------|------|-----------|
| **AIRI** | **49,949** | ✅ 桌面形象 | ✅ 多模态AI | TS+Vue+Electron | 重 | MIT | ⭐⭐⭐⭐⭐ |
| **OpenHuman** | **40,468** | ❌ 窗口内嵌 | ✅ 长效记忆+Mascot | **Tauri+React+TS** | 轻量 | GPL-3.0 | ⭐⭐⭐⭐⭐ |
| **STranslate** | **8,157** | ✅ 边缘吸附 | ⚠️ 翻译 | C#+WPF | 中 | MIT | ⭐⭐⭐⭐ |
| **OpenBitFun** | **2,372** | ❌ 桌面应用 | ✅ Agent+ComputerUse | **Rust+Tauri** | 轻量 | MIT | ⭐⭐⭐⭐ |
| **Awesome AI Companion** | **852** | — (索引) | ✅ 全维度 | — | — | CC0 | ⭐⭐⭐⭐⭐ |
| **WindowPet** | **670** | ✅ 透明悬浮 | ❌ | **Tauri+React+TS** | 轻量 | MIT | ⭐⭐⭐⭐⭐ |
| **yoji** | **663** | ✅ 悬浮组件 | ✅ MCP+情感感知 | TS 跨平台 | 中 | MIT | ⭐⭐⭐⭐⭐ |
| **xsai** | **658** | ❌ | ✅ Provider SDK | TS SDK | 轻量 | MIT | ⭐⭐⭐⭐⭐ |
| **Onepoint** | **315** | ✅ 悬浮球 | ✅ 多模型对话 | Electron+Vue | 重 | MIT | ⭐⭐⭐⭐ |
| **jiwen** | **162** | ❌ | ❌ | JS 零依赖 | 轻量 | MIT | ⭐⭐⭐⭐ |

#### 核心结论

> **AIRI（49,949 Stars）是悬浮球+AI 形象领域最热门的项目**——MIT 协议，多模态 AI 伴侣 + Live2D 形象，但基于 Electron。
>
> **OpenHuman（40,468 Stars）是 Tauri + Rive Mascot 的最佳参考**——Rive 动画 + 18 种表情状态机 + 口型同步 + 长效记忆，GPL-3.0 仅参考架构。
>
> **WindowPet 是实现 Orb 悬浮球的最佳 Tauri 参考项目**——技术栈（Tauri + React + TypeScript）完全一致，MIT 协议可自由复用代码，透明悬浮窗口方案开箱即用。
>
> **yoji（663 Stars）是目前发现的最匹配目标产品**——悬浮组件 + AI 对话 + MCP 工具调用 + 情感感知，几乎就是我们的目标。
>
> **xsai（658 Stars）是前端 Provider 适配的最佳参考**——轻量级 AI SDK，统一接口 + 多实现 + 流式输出。
>
> **OpenBitFun（2,372 Stars）是 Rust + Tauri Agent 桌面应用的最佳参考**——MIT 协议，Computer Use Agent + Agent Teams 架构值得借鉴。
>
> **Awesome AI Companion（852 Stars）是最全的 AI 伴侣项目索引**——CC0 协议，从中发现了 yoji、xsai、jiwen 等高匹配项目。
>
> **组合借鉴策略：**
> - **悬浮球底层实现** → WindowPet（Tauri 透明窗口 + 拖拽 + 吸附边缘）
> - **悬浮球交互设计** → Onepoint（失焦隐藏 + 快捷键）+ yoji（悬浮组件 + AI 联动）
> - **Mascot 动画形象** → OpenHuman（Rive + 表情状态机）+ jiwen（情感引擎）
> - **AI Provider 适配** → OpenBitFun（Rust 端）+ xsai（前端 SDK）
> - **Agent 模式 + MCP** → OpenBitFun（模式分级）+ MCP Launcher（服务器管理）
> - **记忆系统** → OpenHuman（长效记忆）+ AIRI（向量记忆）
> - **Agent 多智能体协作** → OpenBitFun（Rust Agent Runtime + Computer Use）
> - **边缘吸附 + 划词** → STranslate（交互细节参考）
> - **项目发现索引** → Awesome AI Companion（持续更新的 AI 伴侣项目大全）

---

## 15. 参考项目深度分析与架构思路

### 15.1 WindowPet — Tauri 透明悬浮窗口的教科书级实现

**核心架构：**

```
WindowPet
├── src-tauri/           # Rust 后端
│   ├── src/app/
│   │   ├── tray.rs      # 系统托盘（Show/Pause/Setting/Restart/Quit）
│   │   ├── cmd.rs       # Tauri Commands（获取鼠标位置、打开文件夹）
│   │   ├── utils.rs     # 窗口管理（reopen_main_window / open_setting_window）
│   │   ├── conf.rs      # 应用配置
│   │   └── mod.rs       # 模块入口
│   └── tauri.conf.json  # 窗口配置：transparent+alwaysOnTop+skipTaskbar+64x64
├── src/                 # React 前端
│   ├── scenes/
│   │   ├── Pet.ts       # Phaser Scene：精灵表动画（状态机驱动 idle/walk/sit…）
│   │   ├── Pets.ts      # 多宠物管理
│   │   └── manager.ts   # 场景管理器
│   ├── PhaserWrapper.tsx # Phaser 游戏容器（全屏透明、重力物理、30fps）
│   ├── App.tsx          # 路由：/ → PhaserWrapper，/setting → SettingWindow
│   └── config/          # 每个宠物一个 JSON 配置（帧动画定义）
```

**关键技术点：**

1. **透明穿透窗口**：`tauri.conf.json` 设置 `transparent: true` + `alwaysOnTop: true` + `skipTaskbar: true`，窗口 64×64 像素
2. **鼠标穿透**：`appWindow.setIgnoreCursorEvents(true)` — 窗口不拦截鼠标事件，让点击穿透到桌面
3. **鼠标位置获取**：因为窗口穿透后 JS 无法获取鼠标位置，所以用 Rust 端 `mouse_position` crate 通过 `#[tauri::command]` 返回坐标
4. **Phaser 游戏引擎**：用 Phaser 做精灵表动画，全屏透明 Canvas，重力物理模拟宠物下落/行走
5. **双窗口架构**：主窗口（宠物）+ 设置窗口（独立路由 `/setting`），通过托盘菜单切换
6. **系统托盘**：Rust 端 `SystemTray` 实现，支持 Show/Pause(释放内存)/Setting/Restart/Quit

**可借鉴：**
- ✅ Tauri 透明悬浮窗口的完整实现方案
- ✅ 鼠标穿透 + Rust 端获取鼠标位置的技巧
- ✅ 双窗口架构（悬浮球 + 设置面板）
- ✅ 系统托盘菜单设计

### 15.2 Onepoint — Electron 悬浮球 + AI 对话的交互范式

**核心架构：**

```
onepoint
├── src/electron/
│   ├── main.ts          # Electron 主进程：无边框透明窗口 + 失焦自动隐藏
│   ├── os/
│   │   ├── tray.ts      # 系统托盘（Display Window / Settings / Quit）
│   │   └── shortcuts.ts # 全局快捷键 Cmd+K 唤起/隐藏
│   ├── client/
│   │   ├── bridge.ts    # IPC 桥接
│   │   ├── clipboard.ts # 剪贴板监听
│   │   ├── window.ts    # 窗口可见性控制
│   │   └── store.ts     # 持久化存储
│   ├── apis/            # API 调用（account/apply/crawl/prompt）
│   └── utils/
│       └── window.ts    # setWindowVisile() — 窗口显隐+聚焦控制
├── src/app/             # React 前端
│   ├── api.ts           # API 配置
│   ├── store.ts         # 全局状态
│   └── hooks.ts         # 自定义 Hooks
├── src/components/      # UI 组件
│   ├── ChatPanel/       # 对话面板
│   ├── Setting/         # 设置面板（Account/Basic/Prompt）
│   └── Preset/          # 预设管理
└── src/features/        # Redux Slice（chat/clipboard/history/preset/setting）
```

**关键技术点：**

1. **失焦自动隐藏**：`win.on('blur', () => setWindowVisile({win, visible: false}))` — 窗口失焦即隐藏，实现"用完即走"
2. **全局快捷键唤起**：`globalShortcut.register('CommandOrControl+k')` — 随时唤起
3. **剪贴板感知**：唤起时自动读取选中文本/剪贴板内容，作为对话上下文
4. **多模型切换**：通过 Preset 系统管理不同 AI 模型配置
5. **窗口可见性控制**：`setWindowVisile()` 统一管理 show/hide/focus/blur，支持 `setVisibleOnAllWorkspaces`
6. **无边框透明窗口**：`frame: false, transparent: true, backgroundColor: '#00000000', skipTaskbar: true`

**可借鉴：**
- ✅ 失焦自动隐藏的交互模式（悬浮球的核心体验）
- ✅ 全局快捷键唤起 + 剪贴板感知
- ✅ 窗口显隐的统一控制函数
- ✅ 多模型 Preset 切换机制

### 15.3 OpenBitFun — Rust Agent Runtime 的企业级架构

**核心架构（分层设计）：**

```
OpenBitFun/src/crates/
├── execution/                    # 执行层
│   ├── agent-runtime/            # Agent 运行时
│   │   └── src/agents.rs         # Agent 定义：7种模式 + 10+子Agent
│   │       ├── 模式(Mode): Minimal/Standard/Cowork/Creative/Claw/DeepResearch/Ultimate
│   │       └── 子Agent: SwarmPlanner/SwarmWorker/SwarmReviewer/ComputerUse/Explore/
│   │                    GeneralPurpose/ResearchSpecialist/ReviewWorker/ReviewJudge/
│   │                    ReviewFixer/CodeReview/DeepReview/GenerateDoc/OpenBitFun
│   ├── tool-execution/           # 工具执行（shell/search/fs）
│   └── tool-call-jsonrepair/     # JSON 修复解析
├── adapters/                     # 适配器层
│   ├── ai-adapters/              # AI Provider 适配器
│   │   └── src/providers/
│   │       ├── openai/           # OpenAI 适配
│   │       ├── anthropic/        # Anthropic 适配
│   │       └── gemini/           # Gemini 适配
│   └── webdriver/                # 浏览器控制（Computer Use）
├── services/                     # 服务层
│   ├── services-core/            # 核心服务（session/filesystem/diff/diagnostics）
│   ├── services-integrations/    # 集成服务（mcp/git/terminal/speech/browser_control/canvas）
│   └── relay-service/            # 中继服务（realtime/routes）
├── contracts/                    # 契约层
│   └── product-domains/          # 产品域（miniapp/canvas/function_agents）
├── assembly/                     # 装配层
│   └── core/                     # 核心装配
│       ├── service/              # 服务装配（workspace/session/mcp/git/…）
│       ├── agentic/              # Agent 装配
│       │   ├── agents/           # Agent 注册表 + 定义 + Prompt Builder
│       │   ├── tools/            # 工具实现（exec_command/skills/task/web/control_hub）
│       │   ├── session/          # Agent 会话管理
│       │   ├── memories/         # 记忆系统
│       │   ├── subagent_runtime/ # 子Agent 运行时
│       │   └── coordination/     # Agent 协调
│       └── infrastructure/       # 基础设施（storage/events/ai/app_paths）
└── interfaces/                   # 接口层
    ├── app-server/               # HTTP 服务
    ├── app-server-protocol/      # 协议定义
    └── acp/                      # Agent Communication Protocol
```

**关键技术点：**

1. **Agent 模式系统**：7 种模式（Minimal → Ultimate），每种模式有不同的工具集和上下文策略
2. **子 Agent 协作**：SwarmPlanner → SwarmWorker → SwarmReviewer 三级协作；ComputerUse 独立子 Agent
3. **Provider 适配器模式**：统一的 `MessageConverter` trait，OpenAI/Anthropic/Gemini 各自实现
4. **MCP 集成**：完整的 MCP (Model Context Protocol) 服务端 + 配置 + 适配器
5. **工具执行沙箱**：`tool-execution` 独立 crate，shell 分析、文件搜索、文件系统操作
6. **会话持久化**：session + token_usage + session_usage 三层追踪
7. **Computer Use**：webdriver 适配器实现浏览器控制，支持键盘/鼠标/元素操作

**可借鉴：**
- ✅ Agent 模式分级（从轻量到全功能）的设计思路
- ✅ Provider 适配器模式（统一接口 + 多实现）
- ✅ MCP 协议集成方案
- ✅ 工具执行的安全沙箱设计
- ✅ 子 Agent 协作架构（Swarm 模式）

### 15.4 OpenHuman — Tauri + React + Rive Mascot 的完整助手应用

**核心架构：**

```
openhuman
├── crates/openhuman-app/         # Tauri Rust 端
│   └── tauri.conf.json           # 窗口配置：1280×900, visible:false, Overlay标题栏
├── app/src/                      # React 前端
│   ├── features/human/Mascot/    # Mascot 形象系统
│   │   ├── RiveMascot.tsx        # Rive 动画渲染（WebGL2）
│   │   ├── Ghosty.tsx            # SVG 矢量形象（18种表情状态机）
│   │   ├── riveMaps.ts           # 表情→Rive 状态映射
│   │   ├── visemes.ts            # 口型同步（Oculus 15-set viseme）
│   │   ├── mascotPalette.ts      # 形象配色方案
│   │   ├── useMascotClock.ts     # 形象动画时钟
│   │   └── manifest/             # 形象清单系统（自定义形象加载）
│   ├── store/
│   │   ├── mascotSlice.ts        # Mascot 状态（颜色/语音/形象选择）
│   │   └── chatRuntimeSlice.ts   # 对话运行时（消息/工具调用/子Agent）
│   ├── services/
│   │   ├── chatService.ts        # 对话服务（RPC + Socket.IO 流式）
│   │   └── chatDeltaCoalescer.ts # 流式增量合并
│   └── providers/
│       └── chatSurfaceHandlers.ts # 对话 UI 事件处理
```

**关键技术点：**

1. **Rive 动画引擎**：使用 `@rive-app/react-webgl2` 渲染 `.riv` 动画文件，支持状态机驱动
2. **18 种表情状态机**：`MascotFace` 类型定义了 idle/listening/thinking/confused/speaking/happy/concerned/curious/proud/cautious/celebrating/writing/reading/waving/dancing/drinking_coffee/drinking_boba + sleep，与 Agent 生命周期联动
3. **口型同步**：Oculus 15-set viseme 系统，TTS 输出时嘴型与语音同步
4. **双形象系统**：RiveMascot（.riv 动画）+ Ghosty（SVG 矢量），可切换
5. **形象配色**：yellow/burgundy/black/navy/custom 五种配色 + 自定义颜色
6. **对话运行时**：Redux Toolkit 管理完整的对话生命周期（消息/工具调用/子Agent/时间线）
7. **流式合并**：`chatDeltaCoalescer` 处理 SSE 增量更新，合并为完整消息
8. **RPC + Socket.IO**：发送走 RPC，接收走 Socket.IO 流式事件

**可借鉴：**
- ✅ Rive 动画形象系统（比 Live2D 更轻量，WebGL2 渲染）
- ✅ 表情状态机设计（18 种状态覆盖 Agent 完整生命周期）
- ✅ 口型同步方案（viseme 系统）
- ✅ 对话运行时的 Redux 状态管理
- ✅ 流式增量合并策略

### 15.5 AIRI — 多模态 AI 伴侣的 Monorepo 架构

**核心包结构：**

```
airi/packages/
├── core-agent/              # Agent 运行时编排（spark-notify / spark-command）
├── core-character/          # 角色管线编排（分割/情感/延迟/可选TTS）
├── provider-inference/      # Provider 推理层（运行时无关的 Provider 定义）
├── stage-ui-live2d/         # Live2D 场景组件（PixiJS 渲染）
├── stage-ui-spine/          # Spine 动画
├── stage-ui-mmd/            # MMD 动画
├── stage-ui-tachie/         # Tachie 动画
├── stage-ui-three/          # Three.js 3D 场景
├── stage-pages/             # 场景页面
├── stage-layouts/           # 场景布局
├── model-driver-magic-live2d/ # Live2D 模型驱动
├── model-driver-mediapipe/  # MediaPipe 面部追踪
├── model-driver-lipsync/    # 口型同步驱动
├── motion-driver-magic/     # 动作驱动
├── pipelines-audio/         # 音频管线
├── memory-pgvector/         # 向量记忆（pgvector）
├── plugin-sdk/              # 插件 SDK
├── plugin-protocol/         # 插件协议
├── server-runtime/          # 服务端运行时
├── server-sdk/              # 服务端 SDK
└── electron-vueuse/         # Electron VueUse 集成
```

**关键技术点：**

1. **Monorepo 架构**：pnpm workspace，50+ 包，清晰的分层（core → driver → stage → scenario）
2. **多动画引擎支持**：Live2D / Spine / MMD / Tachie / Three.js，统一接口
3. **Provider 推理层**：运行时无关的 Provider 定义，支持 OpenAI/Anthropic/本地模型
4. **角色管线**：`core-character` 编排 分割→情感→延迟→TTS 的完整管线
5. **Agent 运行时**：`core-agent` 提供 spark-notify（通知触发）和 spark-command（命令触发）
6. **向量记忆**：`memory-pgvector` 基于 PostgreSQL pgvector 的语义记忆
7. **面部追踪**：`model-driver-mediapipe` 用 MediaPipe 做面部追踪驱动 Live2D
8. **插件系统**：`plugin-sdk` + `plugin-protocol` 定义插件接口

**可借鉴：**
- ✅ Monorepo 分层架构（core → driver → stage → scenario）
- ✅ 多动画引擎的统一抽象
- ✅ 角色管线编排模式
- ✅ 向量记忆系统设计
- ✅ 插件 SDK + 协议的设计

### 15.6 Awesome AI Companion — AI 伴侣生态全景索引

**分类体系（12 大类）：**

1. **Companion Clients & Workspaces** — 对话客户端和工作空间
2. **Virtual Phones & Companion Spaces** — 虚拟手机和伴侣空间
3. **Background Heartbeats & Proactive Messaging** — 后台心跳和主动消息
4. **Memory, Identity & Emotion State** — 记忆、身份和情感状态
5. **Voice, Visual Presence & Embodiment** — 语音、视觉形象和具身化
6. **Perception** — 感知
7. **Services & Real-World Integrations** — 服务和现实世界集成
8. **Game Worlds & Agent Toys** — 游戏世界和 Agent 玩具
9. **Shared Activities & Media** — 共享活动和媒体
10. **Communities & Forums** — 社区和论坛
11. **Continuity & Data Ownership** — 连续性和数据所有权
12. **Featured Badge** — 精选标记

**关键发现：**
- **yoji** — 情感感知桌面 AI 伴侣：本地语音唤醒、悬浮组件、情绪漂移、MCP 工具调用、办公辅助。MIT。**高度匹配我们的目标！**
- **ackem** — 本地优先 AI 桌面伴侣（Electron）：隐私优先记忆、情感引擎、扩展。AGPLv3。
- **Operit** — Android Agent 应用：工具调用、工作流自动化、记忆、角色卡、语音、本地 MNN/llama.cpp 模型。
- **Aura** — Android AI 伴侣应用：跨会话记忆、情感状态机、深化关系模型、Health Connect 数据、MCP。
- **jiwen (积温)** — 主动意识引擎：5 个漂移轴（连接度/固执/情绪/焦虑/忙碌度）在阈值触发行为。~500 行零依赖。MIT。

---

### 15.7 架构思路综合 — 本项目的借鉴方案

基于以上 6 个项目的深度分析，提炼出以下架构思路：

#### 15.7.1 技术栈决策

| 决策项 | 选型 | 依据 |
|--------|------|------|
| **桌面框架** | **Tauri 2** | WindowPet/OpenHuman/OpenBitFun 均验证了 Tauri 的可行性；比 Electron 包体积小 10x+、内存低 50%+ |
| **前端** | **React + TypeScript** | WindowPet/OpenHuman 同技术栈；生态最丰富 |
| **动画形象** | **Rive** | OpenHuman 验证了 Rive + React WebGL2 方案；比 Live2D SDK 更轻量、Web 原生、状态机内置 |
| **状态管理** | **Zustand** | 比 Redux 轻量；OpenHuman 用 Redux Toolkit 偏重 |
| **AI Provider** | **适配器模式** | OpenBitFun 的 providers/mod.rs 模式；统一接口 + 多实现 |
| **Agent 模式** | **分级模式** | OpenBitFun 的 Minimal→Ultimate 分级；我们可做 Quick/Standard/Deep 三级 |

#### 15.7.2 核心架构蓝图

```
Assistant (Tauri 2 + React + TypeScript)
│
├── src-tauri/                          # Rust 后端
│   ├── commands/                       # Tauri Commands
│   │   ├── window.rs                   # 窗口管理（借鉴 WindowPet utils.rs）
│   │   ├── mouse.rs                    # 鼠标位置（穿透窗口必需）
│   │   ├── tray.rs                     # 系统托盘
│   │   └── shortcut.rs                 # 全局快捷键
│   ├── providers/                      # AI Provider 适配器（借鉴 OpenBitFun）
│   │   ├── mod.rs                      # Provider trait 定义
│   │   ├── openai.rs
│   │   ├── anthropic.rs
│   │   └── gemini.rs
│   ├── agent/                          # Agent 运行时（借鉴 OpenBitFun）
│   │   ├── modes.rs                    # 模式定义（Quick/Standard/Deep）
│   │   ├── session.rs                  # 会话管理
│   │   └── tools.rs                    # 工具注册与执行
│   └── storage/                        # 本地存储
│       ├── sqlite.rs                   # SQLite 操作
│       └── kv.rs                       # 键值存储
│
├── src/                                # React 前端
│   ├── orb/                            # Orb 悬浮球（借鉴 WindowPet + Onepoint）
│   │   ├── OrbBall.tsx                 # 悬浮球主体（Rive 动画）
│   │   ├── OrbDrag.tsx                 # 拖拽逻辑
│   │   └── OrbExpand.tsx              # 展开动画（球→面板）
│   ├── mascot/                         # Mascot 形象系统（借鉴 OpenHuman）
│   │   ├── RiveMascot.tsx             # Rive 渲染
│   │   ├── faceStates.ts              # 表情状态机（18种状态）
│   │   └── visemes.ts                 # 口型同步
│   ├── chat/                           # 对话模块
│   │   ├── ChatPanel.tsx              # 对话面板
│   │   ├── ChatInput.tsx              # 输入区
│   │   ├── MessageList.tsx            # 消息列表
│   │   └── streamCoalescer.ts         # 流式增量合并（借鉴 OpenHuman）
│   ├── tools/                          # 工具箱
│   │   ├── registry.ts                # 工具注册表
│   │   └── [tool-name]/              # 各工具实现
│   ├── store/                          # 状态管理（Zustand）
│   │   ├── chatStore.ts               # 对话状态
│   │   ├── orbStore.ts                # 悬浮球状态
│   │   ├── mascotStore.ts             # 形象状态
│   │   └── providerStore.ts           # Provider 配置
│   └── providers/                      # Provider 前端适配
│       ├── adapter.ts                  # 统一接口
│       └── [provider-name]/           # 各 Provider 实现
│
└── resources/
    └── mascot.riv                      # Rive 动画资源
```

#### 15.7.3 交互流程设计

```
[桌面空闲状态]
    │
    ├── Orb 悬浮球常驻桌面（透明窗口 + alwaysOnTop + skipTaskbar）
    │   ├── 默认：小球态（64×64，Rive idle 动画）
    │   ├── 拖拽：跟随鼠标移动（Rust 端获取鼠标位置）
    │   ├── 吸附：松手后自动吸附到屏幕边缘
    │   └── 右键：托盘菜单（设置/退出）
    │
    ├── 全局快捷键 Cmd/Ctrl+K 唤起
    │   │
    │   ▼
    │  [展开态] 悬浮球展开为主面板（800×600）
    │   ├── 展开动画：球→圆角矩形（Rive 状态机过渡）
    │   ├── Mascot 形象：面板顶部 Rive 动画角色
    │   ├── 对话区：流式对话 + 工具调用展示
    │   ├── 工具箱：侧边栏工具导航
    │   └── 失焦/ESC：自动收起为悬浮球
    │
    └── 剪贴板感知（可选）
        └── 检测到新内容 → 悬浮球显示提示动画
```

#### 15.7.4 表情状态机（借鉴 OpenHuman Ghosty.tsx）

```typescript
type MascotFace =
  | 'sleep'        // 长时间无交互
  | 'idle'         // 待命
  | 'listening'    // 用户输入中
  | 'thinking'     // AI 推理中
  | 'speaking'     // AI 回复中（嘴型同步）
  | 'happy'        // 任务完成
  | 'concerned'    // 出错
  | 'curious'      // 发现有趣内容
  | 'working'      // 工具执行中
  | 'waving'       // 打招呼
```

#### 15.7.5 Agent 模式分级（借鉴 OpenBitFun agents.rs）

| 模式 | 描述 | 工具集 | 上下文策略 |
|------|------|--------|-----------|
| **Quick** | 快速问答 | 无工具 | 最少上下文 |
| **Standard** | 标准助手 | 基础工具（搜索/计算/翻译） | 标准上下文 |
| **Deep** | 深度助手 | 全部工具 + 子Agent | 最大上下文 + 记忆 |

### 15.8 新发现高匹配项目深度分析

通过 Awesome AI Companion 索引和 AIRI 推荐，发现以下与目标高度匹配的新项目：

#### 15.8.1 yoji — 情感感知桌面 AI 伴侣（最匹配！）

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/wangxijie001/yoji |
| Stars | **663** ⭐ |
| Forks | 111 |
| 技术栈 | TypeScript（跨平台） |
| 协议 | MIT |
| 最近更新 | 2026-10-03 |

**核心能力：**
- **情感感知桌面 AI 伴侣**——几乎就是我们的目标产品！
- 本地语音唤醒（Whisper 本地模型）
- **悬浮组件**（Orb！）常驻桌面
- 情绪漂移系统（5 个情感轴动态变化）
- MCP 工具调用（文件系统、浏览器、日历等）
- 办公辅助（日程管理、邮件提醒、笔记整理）
- 跨平台（Win/Mac/Linux）

**可借鉴：**
- ✅ **悬浮组件 + AI 对话 + 工具调用**的完整实现——最接近我们的目标
- ✅ 本地语音唤醒方案（Whisper 本地模型）
- ✅ 情绪漂移系统（5 轴情感模型）
- ✅ MCP 工具调用集成
- ✅ 办公辅助场景设计
- ⚠️ 非 Tauri 技术栈，但架构思路完全可借鉴

#### 15.8.2 xsai — 轻量级 AI SDK（AIRI 的 Provider 层）

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/moeru-ai/xsai |
| Stars | **658** ⭐ |
| Forks | 61 |
| 技术栈 | TypeScript |
| 协议 | MIT |

**核心能力：**
- 轻量级 AI SDK，类似 Vercel AI SDK 但更小
- 统一的 Provider 接口（OpenAI / Anthropic / Gemini / 本地模型）
- 流式输出支持
- 工具调用支持
- AIRI 的 Provider 层就是基于 xsai

**可借鉴：**
- ✅ **前端 Provider 适配器的最佳参考**——统一接口 + 多实现
- ✅ 流式输出处理方案
- ✅ 工具调用抽象
- ✅ MIT 协议，可直接参考或集成

#### 15.8.3 jiwen (积温) — 极简情感引擎

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/ClaraShafiq/jiwen |
| Stars | **162** ⭐ |
| Forks | 24 |
| 技术栈 | JavaScript（~500 行零依赖） |
| 协议 | MIT |

**核心能力：**
- 主动意识引擎：5 个漂移轴（连接度/固执/情绪/焦虑/忙碌度）
- 阈值触发行为：当某个轴达到阈值时，自动触发对应行为
- ~500 行零依赖，极简实现
- 可与任何 AI 对话系统结合

**可借鉴：**
- ✅ **极简情感引擎，可直接嵌入我们的项目**
- ✅ 5 轴情感模型：连接度（bond）、固执（stubbornness）、情绪（mood）、焦虑（anxiety）、忙碌度（busyness）
- ✅ 阈值触发机制：情感轴达到阈值 → 自动行为（主动打招呼、提醒休息、表达关心等）
- ✅ 零依赖，~500 行，易于理解和改造

#### 15.8.4 MCP Launcher — MCP 服务器管理器

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/moeru-ai/mcp-launcher |
| Stars | **46** ⭐ |
| Forks | 6 |
| 技术栈 | Go |
| 协议 | 无明确协议 |

**核心能力：**
- MCP 服务器构建器和启动器
- 像 Ollama 之于模型，MCP Launcher 之于 MCP 服务器
- 简化 MCP 集成：一键启动/停止/配置 MCP 服务器

**可借鉴：**
- ✅ **MCP 服务器管理方案**——简化 MCP 集成复杂度
- ✅ 可作为外部依赖，简化我们的 MCP 实现

#### 15.8.5 tinyflows — 工作流引擎（OpenHuman 底座）

| 项目 | 信息 |
|------|------|
| 仓库 | https://github.com/tinyhumansai/tinyflows |
| Stars | **47** ⭐ |
| Forks | 17 |
| 技术栈 | Rust |
| 协议 | GPL-3.0 |

**核心能力：**
- 工作流引擎，22 种节点类型
- 可视化画布编排
- OpenHuman 的工作流底座

**可借鉴：**
- ✅ **Agent 工具调用的工作流编排**——复杂任务可拆解为工作流节点
- ✅ 可视化画布设计（未来扩展）

#### 15.8.6 新项目综合匹配度

| 项目 | Stars | 悬浮球 | AI 对话 | 工具调用 | 情感引擎 | 技术栈 | 协议 | 匹配度 |
|------|-------|--------|---------|----------|----------|--------|------|--------|
| **yoji** | 663 | ✅ | ✅ | ✅ MCP | ✅ 5轴 | TS 跨平台 | MIT | ⭐⭐⭐⭐⭐ |
| **xsai** | 658 | ❌ | ✅ Provider | ✅ | ❌ | TS SDK | MIT | ⭐⭐⭐⭐⭐ |
| **jiwen** | 162 | ❌ | ❌ | ❌ | ✅ 极简 | JS 零依赖 | MIT | ⭐⭐⭐⭐ |
| **MCP Launcher** | 46 | ❌ | ❌ | ✅ MCP | ❌ | Go | 无 | ⭐⭐⭐⭐ |
| **tinyflows** | 47 | ❌ | ❌ | ✅ 工作流 | ❌ | Rust | GPL-3.0 | ⭐⭐⭐ |

> **结论：yoji 是目前发现的最匹配项目**——悬浮组件 + AI 对话 + MCP 工具调用 + 情感感知，几乎就是我们的目标产品。xsai 是前端 Provider 适配的最佳参考。jiwen 是极简情感引擎，可直接嵌入。

---

### 15.9 全部参考项目综合索引

#### 15.9.1 已拉取到 references 的项目

| 项目 | Stars | 技术栈 | 协议 | 核心借鉴点 |
|------|-------|--------|------|-----------|
| **AIRI** | 49,949 | TS+Vue+Electron | MIT | Monorepo 架构、多动画引擎、角色管线、向量记忆 |
| **OpenHuman** | 40,468 | Rust+Tauri+React+TS | GPL-3.0 | Rive Mascot、表情状态机、口型同步、对话运行时 |
| **OpenBitFun** | 2,372 | Rust+Tauri | MIT | Agent 模式分级、Provider 适配器、MCP 集成、Swarm 协作 |
| **Onepoint** | 315 | Electron+Vue | MIT | 悬浮球交互、失焦隐藏、全局快捷键、剪贴板感知 |
| **WindowPet** | 670 | Tauri+React+TS | MIT | 透明悬浮窗口、鼠标穿透、双窗口架构、系统托盘 |
| **Awesome AI Companion** | 852 | — (索引) | CC0 | 项目发现索引、生态全景 |

#### 15.9.2 新发现待拉取的高价值项目

| 项目 | Stars | 技术栈 | 协议 | 核心借鉴点 | 优先级 |
|------|-------|--------|------|-----------|--------|
| **yoji** | 663 | TS 跨平台 | MIT | 悬浮组件+AI对话+MCP+情感感知 | 🔴 高 |
| **xsai** | 658 | TS SDK | MIT | 轻量级 Provider SDK、流式输出 | 🔴 高 |
| **jiwen** | 162 | JS 零依赖 | MIT | 极简情感引擎（~500行） | 🟡 中 |
| **MCP Launcher** | 46 | Go | 无 | MCP 服务器管理 | 🟡 中 |
| **tinyflows** | 47 | Rust | GPL-3.0 | 工作流引擎 | 🟢 低 |

#### 15.9.3 组合借鉴策略（最终版）

| 功能模块 | 主要借鉴项目 | 复用方式 |
|---------|------------|---------|
| **Tauri 透明悬浮球** | WindowPet | 直接复用代码（MIT） |
| **悬浮球交互（失焦隐藏/快捷键）** | Onepoint + yoji | 参考设计思路 |
| **Rive Mascot 形象** | OpenHuman | 参考架构（GPL-3.0 仅参考） |
| **表情状态机** | OpenHuman + jiwen | OpenHuman 架构 + jiwen 情感模型 |
| **AI Provider 适配** | OpenBitFun + xsai | 参考适配器模式 |
| **Agent 模式分级** | OpenBitFun | 参考设计思路 |
| **MCP 集成** | OpenBitFun + MCP Launcher | 参考集成方案 |
| **流式对话** | OpenHuman + xsai | 参考增量合并 + Provider SDK |
| **口型同步** | OpenHuman + AIRI | 参考 viseme 系统 |
| **记忆系统** | OpenHuman + AIRI | 参考向量记忆设计 |
| **工具箱 UI** | IT-Tools + DevToys | 参考分类体系 + Smart Detection |
| **项目发现** | Awesome AI Companion | 持续跟踪生态 |

---

#### 15.7.6 开发路线图

| 迭代 | 交付物 | 主要参考项目 |
|------|--------|-------------|
| **I0** | 项目脚手架（Tauri 2 + React + SQLite + 托盘） | — |
| **I1** | Orb 悬浮球（透明穿透 + 拖拽 + 吸附 + 快捷键） | WindowPet + Onepoint + yoji |
| **I2** | AI 对话核心（Provider 适配 + 流式 + 对话管理） | OpenBitFun + OpenHuman + xsai |
| **I3** | 工具箱基础（10+ 工具 + 分类 + 搜索） | IT-Tools + DevToys |
| **I4** | 工具箱扩展（20+ 工具 + 剪贴板感知） | IT-Tools + DevToys |
| **I5** | Mascot 形象 + 表情联动 + 情感引擎 | OpenHuman + jiwen |
| **I6** | Agent 模式 + 工具调用 + MCP | OpenBitFun + MCP Launcher |
| **I7** | 记忆系统 + 上下文压缩 + TTS 预留 | OpenHuman + AIRI + jiwen |
| **I8** | 打包发布（三平台 + 签名 + CI） | — |
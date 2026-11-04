# Assistant - 工程设计文档

## 1. 架构总览

**Assistant** 是一款桌面端智能助手应用，以 **Orb 悬浮球** 为 AI 交互入口，以**主窗口**为工具与设置中心。

| 模块 | 职责 | 交互方式 |
|------|------|---------|
| **Orb 悬浮球** | AI 对话、Agent 模式切换、Mascot 形象、上下文压缩提示 | 桌面常驻，点击展开对话面板，失焦收起 |
| **主窗口** | 工具箱、消息历史、密码保险库、设置入口 | 从托盘/悬浮球菜单打开，标准窗口 |
| **设置窗口** | 通用 / 外观 / 快捷键 / Provider / Agent / 存储 / 保险库 / 关于 | 独立窗口 |
| **保险库解锁窗口** | 主密码 / 生物识别解锁 | 独立窗口（`vault-unlock`） |

> **主窗口不提供 AI 对话功能**，所有 AI 交互通过 Orb 完成。主窗口专注工具、历史管理与配置。

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
| Agent 工具调用中 | working |
| 流式输出中 | speaking（→ writing pose） |
| 输出完成 | happy → idle |
| 工具调用成功 | proud → idle |
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
| `/` | 工具箱首页（分类导航 + 搜索 + 收藏 + 最近使用） |
| `/tools/:id` | 具体工具界面 |
| `/history` | 消息历史管理（对话列表 + 搜索 + 筛选 + 批量操作 + 详情/分支） |
| `/vault` | 密码保险库（解锁后进入条目列表/详情） |
| `/vault-unlock` | 独立解锁窗口（`vault-unlock` 窗口专用路由） |
| `/settings` | 设置窗口入口（重定向到 `/settings/overview`） |
| `/settings/overview` | 设置总览 |
| `/settings/general` | 通用设置（语言/开机自启/关闭到托盘/置顶/压缩参数） |
| `/settings/appearance` | 外观设置（主题 + Orb/Mascot 外观） |
| `/settings/shortcuts` | 快捷键配置 |
| `/settings/providers` | AI Provider 配置 |
| `/settings/agents` | Agent 模式管理 |
| `/settings/storage` | 存储管理（附件占用统计 + 孤儿附件清理） |
| `/settings/vault` | 保险库设置（Git 同步/SSH/生物识别/恢复密钥） |
| `/settings/about` | 关于（版本/构建信息；检查更新未实现） |

### 4.2 工具箱

#### 工具接口

```typescript
type ToolCategory = 'dev' | 'converter' | 'security' | 'media'

interface ToolDefinition {
  id: string
  name: string                 // i18n key，如 tools.uuid_generator.name
  description: string          // i18n key
  category: ToolCategory       // 由 keywords 经 classifyKeywords 自动判定
  icon: ComponentType
  keywords: string[]
  component: RecoverableLazyComponent<ComponentType>
  widthMode?: 'default' | 'wide' | 'full'
  capabilities?: { fileDrop?: { accept: string } }
  createdAt?: Date             // 两周内视为 NEW
}
```

#### 工具清单（22 个）

| 分类 | 工具 |
|------|------|
| `dev` | UUID 生成、JSON 格式化、正则测试器、SQL 格式化、Cron 解析、文本 Diff、字数统计、计算器、IP 查询、文件系统链接 |
| `converter` | Base64 编解码、颜色转换、时间戳转换、进制转换、URL 编解码、单位换算 |
| `security` | Hash 计算、JWT 解码、密码生成器 |
| `media` | 二维码生成、图片压缩、SVG 预览 |

### 4.3 设置

- **通用设置**：语言（zh-CN / en-US）/ 开机自启 / 关闭到托盘 / 置顶主窗口 / 上下文压缩阈值与模型
- **外观设置**：主题（亮/暗/跟随系统）+ Orb 悬浮球外观（透明度/缩放/圆角/鼠标穿透/悬停隐藏/悬停延迟）+ Orb 悬浮窗（始终置顶）+ Mascot 外观
- **快捷键**：全局快捷键（toggle_orb / toggle_main / new_chat）+ 应用内快捷键（command_palette），分组展示 + 录制 + 冲突检测 + 重置
- **Provider 管理**：添加/编辑/删除 AI Provider（名称 + API Key + Base URL + 模型列表），密钥经后端加密存储
- **Agent 模式**：内置（默认/快速/深度/翻译助手）+ 自定义模式，支持 system_prompt / temperature / max_tokens / model / tools 组合
- **存储管理**：附件占用统计 + 孤儿附件清理
- **保险库**：Git 同步（远程/分支/Rebase）/ SSH 密钥 / 生物识别解锁 / 恢复密钥导出
- **关于**：版本信息 + 构建信息；检查更新目前为禁用占位（I8 未实施）

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
  title_source TEXT NOT NULL DEFAULT 'placeholder',
  provider_id TEXT,
  model_id TEXT NOT NULL,
  system_prompt TEXT,
  is_pinned INTEGER NOT NULL DEFAULT 0,
  sort_order INTEGER NOT NULL DEFAULT 0,
  generation INTEGER NOT NULL DEFAULT 0,
  mode_id TEXT,
  is_full INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  active_branch_id TEXT
);

CREATE TABLE messages (
  id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  role TEXT NOT NULL,
  content TEXT NOT NULL,
  tokens INTEGER,
  timestamp INTEGER NOT NULL,
  parent_id TEXT,
  sibling_index INTEGER NOT NULL DEFAULT 0,
  is_pinned INTEGER NOT NULL DEFAULT 0,
  reasoning TEXT,
  tool_calls_json TEXT,
  provider_id TEXT,
  model_id TEXT
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
  reasoning_enabled INTEGER,
  reasoning_effort TEXT,
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
  tool_id TEXT PRIMARY KEY,
  used_at INTEGER NOT NULL
);

CREATE TABLE compression_state (
  conversation_id TEXT PRIMARY KEY REFERENCES conversations(id) ON DELETE CASCADE,
  last_compression_at INTEGER,
  compression_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE app_settings (
  key TEXT PRIMARY KEY,     -- 含 schema_version
  value TEXT NOT NULL
);

CREATE TABLE attachment_refs (
  attachment_id   TEXT NOT NULL,
  conversation_id TEXT NOT NULL,
  message_id      TEXT NOT NULL,
  PRIMARY KEY (attachment_id, conversation_id, message_id)
);
-- 索引：idx_messages_conv_ts / idx_messages_parent_id / idx_attachment_refs_conv / idx_attachment_refs_att
```

> **Schema 实况**：I2-History Schema v2 的会话/消息分支相关列已全部落地（conversations 的 `title_source` / `is_pinned` / `sort_order` / `generation` / `mode_id` / `is_full` / `active_branch_id`；messages 的 `parent_id` / `sibling_index` / `is_pinned` / `reasoning` / `tool_calls_json` / `provider_id` / `model_id`）。
> 规划中但**尚未实施**：conversations 的 `is_starred` / `color_mark` / `labels` / `draft_content` / `draft_updated_at` / `message_count` / `total_tokens`；messages 的 `feedback` / `source`；独立 `tool_calls` 表。
> 上下文压缩状态记录在 `compression_state`；压缩摘要以 `role = 'compact_summary'` 的消息写入 messages，前端通过 `CompactBanner` + “继续对话”消费。

### 5.2 文件结构

```
<app_data_dir>/           # Win: %APPDATA%\com.assistant / macOS: ~/Library/Application Support/com.assistant / Linux: ~/.local/share/com.assistant
├── assistant.db          # SQLite（业务数据）
├── attachments/          # 图片附件（asset 协议注册目录）
├── mascot-assets/        # Mascot 资源
└── security/
    ├── security.db       # 安全模块 / 保险库元数据（SQLCipher）
    └── vault/            # 保险库 Git 工作区（本地提交 / 远程同步）

<config_dir>/             # Win: %APPDATA%\com.assistant / macOS: ~/Library/Application Support/com.assistant / Linux: ~/.config/com.assistant
├── config.json           # AppConf
├── orb-config.json       # OrbConf
├── mascot-config.json    # MascotConf
└── vault-config.json     # VaultConf（保险库设置，禁止落在 vault 仓库目录）
```

> 日志由 `tauri-plugin-log` 写入系统日志目录，开发环境同时输出到终端。

---

## 6. API 通信层

### 6.1 Provider 适配器

```rust
pub trait BaseProviderAdapter: Send + Sync {
    fn kind(&self) -> &ProviderKind;
    fn config(&self) -> &ProviderConfig;
    fn chat_completion_stream(
        &self,
        messages: &[ChatMessage],
        model: &str,
        options: &ChatOptions,
        cancel_token: CancellationToken,
    ) -> Result<ChatStream, ProviderError>;
    fn list_models(&self) -> Result<Vec<ModelConfig>, ProviderError>;
}
```

实现（`assistant-providers` crate，按 `ProviderKind` 分派）：`OpenAIAdapter`（OpenAI Chat Completions / OpenAI Responses / 自定义 base_url 兼容端点）、`AnthropicAdapter`、`GeminiAdapter`（含 Gemini Code Assist）。模型元数据来自内嵌的 models.dev 快照。

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

> Rust 后端结构择优参考 BongoCat 的 workspace 多 crate 架构（编译隔离 + API 边界强制 + 独立测试），按我们的领域划分为 1 个根包 + 9 个 workspace crate，远少于 BongoCat 的 20 个。ChatGPT 的 core 模块组织（cmd/conf/constant/setup/window）融入 `assistant-app` 根包。Tauri 2 CLI 要求二进制包位于 `src-tauri/` 根目录（`Cargo.toml` 同时包含 `[package]` 和 `[workspace]`），因此 `assistant-app` 作为根包而非子 crate。

> **实现状态**：Rust 后端为根包 + 9 个 workspace crate；I0–I6 全部完成（Orb / AI 对话 / 23 个工具 / 设置系统 / Mascot / Agent 模式）；I7 上下文压缩已完成（`compression_state` + CompactBanner + 继续对话）；I2-History 已完成（Schema 实况见 §5.1，含分支/搜索/导出/批量操作）；I2e/I2f 图片附件与附件引用清理、I9 保险库、I26 mini 模式与 clock 倒计时已上线。尚未完成项见 §9 末尾“后续迭代”。

```
assistant/
├── src-tauri/                     # Rust 后端（Cargo workspace，根包 assistant-app + 9 个成员 crate）
│   ├── src/                       # 根包（Tauri 集成层）
│   │   ├── main.rs                # 入口（windows_subsystem → assistant_app::run()）
│   │   ├── lib.rs                 # Builder + plugins + setup + invoke_handler
│   │   ├── cmd/                   # Tauri commands（110 个，按域拆分）
│   │   │   ├── mod.rs
│   │   │   ├── app.rs             # 应用级（窗口/配置/快捷键）
│   │   │   ├── chat.rs            # 对话发送与流式任务
│   │   │   ├── chat_stream.rs     # 流式事件消费
│   │   │   ├── chat_utils.rs
│   │   │   ├── compaction.rs      # 上下文压缩命令
│   │   │   ├── conversation.rs    # 会话/消息 CRUD、搜索、导出
│   │   │   ├── conversation_utils.rs
│   │   │   ├── agent_mode.rs
│   │   │   ├── provider.rs
│   │   │   ├── storage.rs         # 附件统计 / 孤儿附件清理
│   │   │   ├── mascot_asset.rs
│   │   │   ├── orb.rs
│   │   │   ├── os_tool_exec.rs
│   │   │   ├── tool.rs
│   │   │   ├── config.rs
│   │   │   └── vault/             # 保险库命令（解锁/CRUD/同步/SSH/恢复密钥）
│   │   ├── setup.rs               # 应用初始化（配置 + 数据库 + 窗口 + 托盘）
│   │   ├── tray.rs                # 系统托盘
│   │   ├── window.rs              # 窗口管理（创建/展开/收起/设置）
│   │   └── crash_dump.rs          # 崩溃转储
│   ├── crates/
│   │   ├── assistant-config/      # AppConf / OrbConf / MascotConf + 原子写
│   │   ├── assistant-orb/         # hover.rs（悬停隐藏）/ placement.rs（多屏约束）
│   │   ├── assistant-providers/   # openai（Chat + Responses）/ anthropic / gemini（含 Code Assist）/ crypto
│   │   ├── assistant-agent/       # chat_runtime / chat_tasks / compaction / prompt_builder / multimodal
│   │   ├── assistant-storage/     # db / conversations / providers / agent_modes / attachments
│   │   ├── assistant-i18n/        # 后端文案（zh-CN / en-US）
│   │   ├── assistant-tools/       # 工具执行上下文与注册（Agent 侧）
│   │   ├── assistant-security/    # vault / crypto / biometric / audit / ssh_agent / recovery_key / screen_lock / clipboard_guard
│   │   └── assistant-sync/        # git_sync / sync_engine / vault_file
│   ├── Cargo.toml                 # [package] 根包 + [workspace] members
│   └── tauri.conf.json
│
├── src/                           # React 前端
│   ├── main.tsx                   # 入口（Provider 包裹 + 主题 + StrictMode）
│   ├── App.tsx                    # 多窗口分流（webview.label → 组件映射）
│   ├── config/constants.ts        # 窗口标签/尺寸/阈值（与 Rust constant.rs 对齐）
│   ├── layouts/                   # 各窗口布局（主窗口 / 设置窗口 / 保险库窗口等）
│   ├── orb/                       # OrbBall / OrbPanel / OrbDrag / OrbExpand / OrbCoord / OrbContextMenu / useOrbConfSync
│   ├── mascot/                    # RiveMascot / faceStates / riveMaps / useMascotChatSync / mascotStore / emotionEngine
│   ├── chat/                      # ChatView / MessageList / MessageInput / 选择器 / compact 横幅 / message 渲染
│   ├── tools/                     # ToolBox / MiniToolBox / ToolPage / registry.ts + 23 个工具实现
│   ├── vault/                     # 保险库页面（列表 / 详情 / 解锁）
│   ├── history/                   # 历史管理（列表 / 详情 / 分支 / 批量操作 / 导出）
│   ├── settings/                  # 设置（core 字段与页面注册 / pages 各设置页 / provider / agentMode / overview）
│   ├── components/                # Titlebar / Sidebar / CommandPalette / 错误边界等共享组件
│   ├── stores/                    # Zustand（theme/orb/chat/agentMode/settings/mascot/provider/tool）
│   ├── services/                  # Provider 前端适配 / 设置事件消费
│   ├── hooks/                     # 主题、附件、快捷键等 hooks
│   ├── i18n/                      # i18next（zh-CN / en-US）
│   ├── types/
│   ├── utils/
│   ├── web-ui/                    # 主窗口 UI 入口
│   └── assets/                    # icons / styles（global.css + theme.ts）/ mascot.riv
│
├── package.json
├── tsconfig.json
└── vite.config.ts
```

---

## 8. 安全设计

- API Key AES-256-GCM 加密存储，密钥派生自机器唯一标识 + HKDF
- 保险库：主密码经 Argon2id 派生主密钥，条目密钥由 HKDF 按 entry_id 派生；条目内容 AES-256-GCM / ChaCha20-Poly1305 加密；元数据库 `security.db` 使用 SQLCipher
- 保险库附加防护：TOTP 二次验证、HIBP 泄露检查、生物识别解锁（平台原生）、应用内锁屏 / 空闲锁定、剪贴板自动清除与防截屏、恢复密钥、SSH Agent
- 同步：保险库以本地 Git 仓库工作区形式提交，支持远程仓库 / 分支 / Rebase（`assistant-sync`）
- Tauri 2 安全模型：前端通过 IPC 调用 Rust，capabilities 中显式声明权限；asset 协议仅放行附件目录
- Rust 后端处理所有敏感操作（API 请求、加密、数据库），前端不直接访问系统资源
- 不收集用户数据，不发送遥测

---

## 9. 开发计划

采用迭代交付，每个迭代交付**完整可用的功能增量**。

### I0 — 项目脚手架 ✅

**交付：** 可运行的 Tauri 2 空项目 + Cargo workspace + 开发环境

- [x] Tauri 2 + React + Vite + TS 初始化
- [x] Cargo workspace 根包 + 6 crate 结构（后扩展为 1 根包 + 9 成员 crate）
- [x] Rust 后端基础（workspace Cargo.toml、tauri.conf.json 权限配置、SQLite 启动初始化）
- [x] ESLint + Prettier + TS 严格模式
- [x] Zustand + React Router
- [x] 主题系统（亮/暗/跟随系统）
- [x] SQLite 初始化（rusqlite；`tauri-plugin-sql` 后续已移除以避免双份 SQLite）
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

### I2-History — 聊天消息历史管理 ✅（代码高亮除外）

**交付：** 分页加载、搜索、导出、置顶、消息编辑/重试、分支切换

- [x] Schema 扩展（落地列见 §5.1）+ 结构体扩展
- [x] 对话高级操作 CRUD + Tauri Commands（置顶/复制/分支/批量删除/搜索/导出 Markdown·JSON）
- [x] 消息操作 CRUD + Tauri Commands（编辑/重试/删除/切换分支）
- [x] chatStore 扩展（分页/搜索/导出/重试）+ UI 组件
- [x] 数学公式（KaTeX）
- [ ] 代码高亮（shiki）
- [ ] 对话收藏（`is_starred`）及标签/草稿等 Schema v2 剩余列

> **现状**：分页、搜索、导出、分支与 ChatError 13 种错误码均已实现；ConversationSidebar 已删除（孤立组件），功能迁移至 `src/history/`。未落地的 Schema 字段见 §5.1 注。

**验收：** 分页加载流畅，搜索/导出可用，置顶/分支可用，消息可编辑/重试

---

### I3 — 主窗口工具箱 ✅

**交付：** 完整工具箱，分类导航 + 搜索 + 收藏 + 23 工具

- [x] 工具注册系统（ToolDefinition 接口 + defineTool 工厂）
- [x] 工具箱首页（分类 + 搜索 + 收藏 + 最近使用 + 拖拽排序）
- [x] 工具分类（dev / converter / security / media / productivity，由 keywords 自动归类）
- [x] dev：UUID 生成、JSON 格式化、正则测试器、SQL 格式化、Cron 解析、文本 Diff、字数统计、计算器、IP 查询、文件系统链接
- [x] converter：Base64 编解码、颜色转换、时间戳转换、进制转换、URL 编解码、单位换算
- [x] security：Hash 计算、JWT 解码、密码生成器
- [x] media：二维码生成、图片压缩、SVG 预览
- [x] productivity：倒计时（clock，支持 mini 紧凑形态）
- [x] 工具使用历史（SQLite tool_history）+ 收藏持久化（SQLite tool_favorites）
- [x] 命令面板（Ctrl/Cmd+K）+ 模糊搜索（fuse.js）
- [x] 侧边栏可折叠分类菜单（CollapsibleToolMenu）
- [x] 对话导出（Markdown / JSON）

> **原 I4（工具箱扩展）已合并入 I3**。I4 本质是 I3 的量变（仅新增工具组件，框架零改动），拆分两个迭代人为制造交付断层。合并后一次性交付完整工具箱，与参考项目 it-tools（60+ 工具一体交付）一致。

> **规格与任务清单**：详见 `docs/store/`；DEVLOG.md 保留全链路踩坑记录。I3 / I4 / I2-history 迭代详情已合并入已完成阶段说明，不再单独引用不存在的旧 spec/tasklist 文件名。

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

> **规格与任务清单**：详见 `docs/store/`；I4 的落地细节见本迭代清单与 DEVLOG.md。

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

### I5 — Mascot 形象 + 表情联动 ✅

**交付：** Rive 动画 Mascot，与对话状态联动

- [x] Rive 运行时集成（@rive-app/react-webgl2）
- [x] .riv 动画文件（9 种 RivePose：idle/thinking/celebration/bookreading/coffeedrink/writing/bobbateadrink/hand_wave/dancing）
- [x] 表情状态机（对话事件 → 表情切换）
- [x] Mascot 显示于 Orb 悬浮球内 + 发送按钮
- [x] MascotFace 扩展至 10 种
- [x] FACE_TO_POSE 完整映射（10→9）
- [x] RiveMascot 颜色换肤（primaryColor/secondaryColor）
- [x] RiveMascot buffer 内存加载
- [x] 用户输入 listening 状态联动
- [x] ack 表情增强（proud 判断）

**参考：**

| 功能点 | 参考项目 | 参考文件 | 借鉴内容 |
|--------|---------|---------|---------|
| Rive 运行时 | OpenHuman | app/src/features/human/Mascot/RiveMascot.tsx | `useRive` + `useViewModel` + `useViewModelInstanceEnum` + `useViewModelInstanceColor`，支持换色/换表情/换 .riv 文件 |
| 表情词汇表 | OpenHuman | Ghosty.tsx L20-38 | `MascotFace` 10 种：idle/listening/thinking/speaking/happy/concerned/curious/working/waving/proud（OpenHuman 18 种精简，不引入语义重复或仅 ambient 用途的表情） |
| 表情→Rive pose 映射 | OpenHuman | riveMaps.ts L48-67 | `FACE_TO_POSE` 表：10 种 MascotFace → 9 种 RivePose（speaking→writing，其余按语义映射） |
| 口型同步 | — | — | 已移除（口型在 writing 姿势下不可见，不再驱动 viseme） |
| Rive viseme 映射 | OpenHuman | riveMaps.ts L82-119 | 15 种 Oculus viseme set + 别名归一化（保留映射表供后续 TTS 场景使用，当前流式输出不驱动口型） |
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

### I7 — Context Compaction ✅

**交付：** 对话满即终止 + 交接摘要 + 继续对话

当 token 量达到上下文窗口 80% 时，异步生成结构化交接摘要，标记对话为已满（只读），前端显示横幅 + [继续对话]（自动新建对话并注入摘要）+ [解锁] 按钮。

```rust
fn should_terminate(prompt_tokens: u32, context_window: u32) -> bool {
    prompt_tokens >= context_window * 80 / 100
}
```

- 触发：`prompt_tokens >= context_window * 80%`（流结束后检查）
- 动作：生成交接摘要（目标/进展/待办/关键信息）→ 标记 `is_full=1` → 前端锁定对话
- 摘要：以 `role = 'compact_summary'` 的消息持久化，继续对话时注入上下文
- 检索：`search_history` 工具仍待 tool-call 基础设施就绪

> **跨对话检索工具依赖 tool-call 执行循环**（当前项目未实现），I7 已交付终止 + 摘要 + 横幅 + 继续对话；检索工具待 tool-call 就绪后接入。

**参考：**

| 功能点 | 参考项目 | 参考文件 | 借鉴内容 |
|--------|---------|---------|---------|
| 阈值判断 | OpenBitFun | compression_prefetch.rs L10-15 | `in_prefetch_window` 阈值公式（简化为百分比阈值） |
| 压缩状态 | OpenBitFun | session.rs L84-93 | `CompressionState { last_compression_at, compression_count }` |
| 会话配置 | OpenBitFun | session.rs L179,193 | `auto_compact: bool` 开关 |

- [x] 对话满终止（阈值触发 + 交接摘要生成 + is_full 标记）
- [x] CompressionState 持久化（SQLite compression_state 表）
- [x] 对话满 UI（横幅 + 继续对话（新建对话 + 摘要注入）+ 解锁 + 输入区禁用 + 侧边栏锁图标）
- [ ] 跨对话检索工具（`search_history`，待 tool-call 就绪）

**验收：** 长对话 token 接近窗口 80% 时自动终止，"继续对话"按钮自动注入摘要到新对话，对话只读

---

### I8 — 打包发布（未开始）

**交付：** 三平台安装包 + 自动更新

- [ ] Windows MSI / NSIS
- [ ] macOS DMG（签名 + 公证）
- [ ] Linux AppImage / deb / rpm
- [ ] GitHub Release CI
- [ ] 自动更新（tauri-plugin-updater + 关于页“检查更新”启用）

> **现状**：`tauri.conf.json` 尚无 updater 配置，关于页“检查更新”为禁用占位；CI 目录不存在。

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
         ├→ I2-History 消息历史管理 ✅
         ├→ I3 工具箱 ✅（22 工具，4 分类）
         └→ I4 设置 + 快捷键 + UI改造 ✅
             └→ I5 Mascot 形象 ✅
                 └→ I6 Agent 自定义模式 ✅
                     └→ I7 Context Compaction ✅
                         └→ I8 打包发布（未开始）
```

I2-History 依赖 I2；I3 和 I4 可并行（均依赖 I2）；I5 依赖 I2+I4；I6 依赖 I2+I5；I7 依赖 I6；I8 依赖全部功能稳定。保险库（I9）与图片附件（I2e/I2f）为并行支线，已完成。

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
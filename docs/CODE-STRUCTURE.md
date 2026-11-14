# 代码结构整理

> 性质：本文只描述仓库**当前代码组织的事实**（目录、分层、模块职责、依赖方向），不包含目标架构与施工计划。
> 相关文档：`ARCHITECTURE.md`（组件职责与交互语义）、`FRONTEND.md`（UI 设计与参考项目）、`docs/store/crate-architecture.md`（后端 crate 规模与编译归属实测）、`docs/principle/CODE-SPLIT-PRINCIPLE.md`（文件拆分判据）、`docs/store/vault-optional-build.md`（vault 可选构建）。

---

## 1. 仓库顶层布局

| 路径 | 作用 |
|---|---|
| `src/` | React 前端（Webview 侧全部代码） |
| `src-tauri/` | Tauri 应用层（`src/`）+ Rust workspace 子 crate（`crates/`） |
| `config/features.json` | 可选功能清单唯一事实源（当前仅 `vault`），前端 Vite define 与后端 Cargo feature 都据此派生 |
| `scripts/` | 构建与维护脚本：`copy-rive-wasm.mjs`（prebuild）、`feature-build.ps1` / `assert-feature-absent.mjs`（可选功能构建校验）、`update-models-dev-snapshot.mjs`（模型目录快照）、`fetch-fonts.mjs`、`generate-icon.cjs` |
| `docs/` | 设计与实现文档；`archived/` 为归档区（按域分目录），`store/` 为活跃设计区，`analysis/` 为预分析区，`principle/` 为规范 |
| `references/` | 参考项目源码（13 个，已分组），仅供检索，不参与构建 |
| `temp-backup/` | 备份与临时脚本产物，不参与构建 |
| `dist/` | Vite 构建产物 |

---

## 2. 三层运行时与三条通道

```
┌─────────────────────────────────────────────┐
│ Webview 前端            src/                 │
│  React 19 + Zustand + antd + i18next         │
└───────┬───────────────────────▲─────────────┘
        │ invoke(命令)           │ emit(事件)
┌───────▼───────────────────────┴─────────────┐
│ Tauri 应用层            src-tauri/src/       │
│  窗口/菜单/托盘、IPC 注册、协议 DTO、宿主适配  │
└───────┬─────────────────────────────────────┘
        │ crate 调用
┌───────▼─────────────────────────────────────┐
│ 业务能力层              src-tauri/crates/     │
│  12 个子 crate（存储、Provider、Agent、工具等）│
└─────────────────────────────────────────────┘
```

| 通道 | 载体 | 说明 |
|---|---|---|
| 命令通道 | `src-tauri/src/ipc.rs` + `cmd/` | 前端 `invoke()` 调用的全部命令在此注册 |
| 事件通道 | `src-tauri/src/protocol/events.rs` | Rust 向前端推送（流式增量、状态变更、跨窗口广播） |
| 契约通道 | `src-tauri/src/protocol/dto/` → `src/types/protocol.contract.ts` | ts-rs 由 Rust DTO 生成 TypeScript 类型，禁止手改 |

---

## 3. 前端结构（`src/`）

### 3.1 启动流程

入口 `src/main.tsx` 的初始化顺序固定：

1. `RuntimeLoader.setWasmUrl()` 指定 Rive 运行时（`/rive.wasm` + fallback）；
2. `initSettingSystem()` 注册内置设置页、字段、Provider、Agent 模式等声明式注册表；
3. `bootstrapLogger()` + `registerGlobalErrorHandlers()`；
4. `bootstrap()`：`loadFeatures()` → 逐个 `getConfGroups()` 从后端 `loadConf()` → `applyLocaleChange()`；
5. 渲染：`AppErrorBoundary` → `ThemedApp`（主题/语言/antd ConfigProvider）→ `App`。

### 3.2 窗口分流

`src/App.tsx` 按 `getCurrentWebview().label` 选择顶层视图：

| webview label | 视图 | 说明 |
|---|---|---|
| `WINDOW_MAIN` | `layouts/MainLayout` | 主窗口：工具箱、历史、设置、保险库 |
| `WINDOW_ORB` | `layouts/OrbWindow` | 悬浮球窗口：小球 + 展开面板 |
| 其它/未知 | 回落 `MainLayout` | — |

`layouts/` 内再分形态：`AppShell`（外壳）、`NormalLayout` / `MiniLayout`（主窗口常规与迷你形态）、`layoutChrome`（窗口装饰控制）。

### 3.3 目录职责总表

| 目录 | 职责 |
|---|---|
| `assets/styles/` | `fonts.css`、`global.css`、`theme.ts`（antd 亮/暗主题令牌） |
| `config/` | `constants.ts`：窗口 label、尺寸、阈值、超时等全局常量 |
| `layouts/` | 窗口级布局与分流（见 3.2） |
| `chat/` | AI 对话 UI 与纯逻辑（见 3.4） |
| `history/` | 消息历史管理（列表 / 搜索 / 详情 / 分支树） |
| `tools/` | 工具箱与 23 个内置工具（见 3.4） |
| `vault/` | 密码保险库全部 UI 与状态（可选功能，见 3.5） |
| `settings/` | 声明式设置系统（注册表 + 运行时 + 页面，见 3.4） |
| `services/` | 后端调用与跨窗口通信（无 UI） |
| `stores/` | Zustand 状态切片（见 3.4） |
| `hooks/` | 跨模块复用 Hook：初始化、主题、导航监听、错误通知、文件拖放 |
| `i18n/` | i18next 初始化、`locales/*.json`、`localeApplier.ts`（语言生效链路） |
| `mascot/` | Mascot 形象：策略表 + 多渲染器 + 状态同步 |
| `orb/` | 悬浮球交互：拖拽、吸附、展开、指针手势、配置同步 |
| `components/` | 通用组件：错误边界、侧栏、标题栏、窗口按钮、命令面板 |
| `features/` | 可选功能前端挂载点（见 3.5） |
| `types/` | `protocol.contract.ts`（生成）、`events.ts`、`shortcutDefaults.generated.ts`（生成） |
| `test-utils/` | 测试夹具、命令桩与 Rust 命令名声明清单（`rustCommands.ts`） |

### 3.4 领域模块内部结构

**`chat/`** —— 对话展示与纯逻辑，按"渲染 / 纯函数 / 输入"分离：

| 子目录 | 内容 |
|---|---|
| `ConversationView.tsx` | 对话区总装 |
| `layouts/` | `ChatLayout` / `AgentLayout` 两种链路形态、`ConversationFrame`、`layoutProps` |
| `core/` | 纯逻辑：消息树、内容块、流合并、错误恢复、推理展示、工具调用视图、工作区显示、类型与 Hook |
| `message/` | 消息气泡、列表、输入框、附件预览、头像、操作条、消息身份 |
| `blocks/` | 分支选择、编辑块、错误块、推理块、重试条、Fork 对话框 |
| `selectors/` | 模式 / 模型 / Provider / 推理强度 / 工作区选择器 |
| `tool/` | 工具调用气泡、审批卡、提问卡、工具配置面板、工具栏、变更提示 |
| `attachment/` | 附件摄入：`intake` / `admit` / `factory` / `paths` / `sources`（拖放、选择、粘贴） |
| `compact/`、`context/` | 压缩提示条、上下文栏 |

**`services/`** —— 与后端交互的唯一边界，按领域拆分：

| 文件/目录 | 职责 |
|---|---|
| `stream/turnOrchestrator.ts` | 一轮对话/Agent 的编排核心：发送、取消、事件分发 |
| `stream/turnStrategy.ts` + `stream/strategies/{chatTurn,agentTurn}.ts` | 按 `kind` 选链路的策略实现 |
| `stream/transport.ts` | 事件订阅与命令调用的传输层 |
| `tauriInvoke.ts` | `invoke()` 统一封装（错误归一 + 日志）；命令名由各领域 service 直接持有 |
| `chatService.ts` / `chatStreamEvents.ts` / `chatLifecycleEvents.ts` | 对话命令、流事件、生命周期事件 |
| `crossWindowEvent.ts` | 跨窗口事件广播与订阅 |
| `settingService.ts` / `settingEvents.ts` | 设置读写与跨窗口生效 |
| `historyService.ts` / `historyEvents.ts` | 历史查询与刷新事件 |
| `providerService.ts` / `agentModeService.ts` / `toolService.ts` / `permissionService.ts` | 对应领域命令封装 |
| `storageService.ts` / `workspaceService.ts` / `orbService.ts` / `windowService.ts` | 存储统计、工作区、悬浮球、窗口控制 |
| `featureIpc.ts` | 可选功能（vault）命令通道 |
| `logger.ts` / `globalErrorHandler.ts` / `reactProductionError.ts` | 日志与错误上报 |

**`stores/`** —— 状态按领域切片，聚合与编排解耦：

| 文件 | 职责 |
|---|---|
| `chatStore.ts` | 对话状态聚合点（组合 conversation / message / compact 切片） |
| `chatConversationSlice.ts` / `chatMessageSlice.ts` / `chatCompactSlice.ts` | 对话、消息、压缩三个切片 |
| `chatStorePorts.ts` | 向编排器暴露 StatePort / EnvPort，隔离 store 实现 |
| `chatStreamStore.ts` | 高频流式状态独立存放，避免整树重渲染 |
| `agent/agentSegments.ts` | Agent 轮次分段与展示折叠 |
| 其余 | `historyStore` / `toolStore` / `providerStore` / `settingsStore` / `agentModeStore` / `attachmentStore` / `orbStore` / `themeStore` / `localeStore` / `notifyStore` / `windowModeStore` |

**`settings/`** —— 声明式设置系统，四层结构：

| 层 | 位置 | 内容 |
|---|---|---|
| 声明 | `core/{field,provider,agentKind}/` | 字段定义、Provider 定义与格式/预设、Agent 模式定义 |
| 注册表 | `core/{confRegistry,confSlot,pageRegistry,builtinRegistry,sideEffectRegistry}.ts` | 设置组、页面、副作用登记 |
| 运行时 | `core/{confRuntime,snapshot,settingsDispatcher,optimisticUpdate,query,insertUnique}.ts` | 快照、统一保存链路、乐观更新 |
| 视图 | `pages/`、`provider/`、`overview/`、`shared/` | 设置页、Provider 管理、总览、通用渲染器与页面外壳 |

**`tools/`** —— 23 个工具采用统一三件套，外层是工具箱框架：

| 部分 | 内容 |
|---|---|
| 单个工具 | `<tool>/index.tsx` + `<tool>/<tool>.service.ts`（纯逻辑）+ `<tool>.module.css` |
| 框架 | `registry.ts`（工具注册）、`types.ts`、`categories.ts`（关键词分类）、`capabilities.tsx`（文件拖放等能力）、`os.ts`（平台判定） |
| 界面 | `ToolBox`（主窗口）、`MiniToolBox` + `miniNavigate`（迷你形态）、`ToolPage`、`ToolCard`、`ToolSearchBar`、`FavoriteButton`、`CollapsibleToolMenu` |
| 工具清单 | base64 / calculator / clock / color / cron / date-time / fs-link / hash / image-compressor / integer-base / ip-info / json-viewer / jwt / password-generator / qrcode / regex / sql-formatter / svg-preview / text-diff / text-statistics / unit-converter / url-encoder / uuid |

**`vault/`** —— 保险库前端按"核心 / 详情 / 表单 / 设置 / 解锁"分层：

| 子目录 | 内容 |
|---|---|
| `core/` | `vaultStore` + 7 个切片（lifecycle / entries / biometric / recovery / reveal / sync / audit）、`vault.service.ts`、`contract/vault.contract.ts`（生成）、`paperKey`、`syncDisplay`、`useVaultAutoLock` / `useVaultEvents` / `useVaultStatusReady` / `useVaultUnlock` |
| `detail/` | 条目详情、历史、TOTP 展示、HIBP 标记、字段揭示按钮 |
| `forms/` | 按条目类型拆分的字段组：Login / Card / Identity / Note / SshKey / Custom |
| `settings/` | Git 同步、SSH Agent、恢复密钥、改主密码、DEK 轮换、导入导出、危险区、冲突解决 |
| `unlock/` | 解锁与灾难恢复：密码表单、生物识别、纸质恢复、DB 修复、仓库重置、版本过新面板 |
| 顶层 | `VaultApp` / `VaultUnlock` / `VaultList` / `VaultEntryDrawer` / 工具栏、状态栏、分类栏、告警、回收站抽屉 |

**`mascot/`** —— 形象与表情：

| 部分 | 内容 |
|---|---|
| `core/` | `expressionPolicy`（表情策略表）、`mascotConfig`、`registry`、`resolve`、`colorUtils`、`types` |
| `renderers/` | `svg/`（内置 Ghosty）、`rive/`（状态机引擎 + 映射）、`gif/`（URL 校验），由 `MascotRenderer.tsx` 按配置选择 |
| 状态 | `mascotStore`、`useMascotChatSync`（订阅对话事件驱动表情） |

**`orb/`** —— 悬浮球交互：

`OrbBall`（小球）、`OrbPanel`（展开面板）、`panelActions`、`OrbDrag`（拖拽与吸附）、`OrbExpand`（展开/收起）、`OrbCoord.ts`（逻辑/物理像素换算）、`pointerGesture`（指针手势判定）、`useOrbConfSync`（配置同步）。

**`history/`** —— 历史管理：

`core/`（分组、搜索、刷新 Hook）、`sidebar/`（会话列表、搜索条、多选条）、`detail/`（详情、分支树、虚拟列表、分支行）、`HistoryManager.tsx` 总装。

### 3.5 可选功能的前端挂载

```
config/features.json
  └─ vite.config.ts featureDefines() → __VAULT_ENABLED__（编译期常量）
       └─ features/registry.ts loadFeatures()
            ├─ 条件 dynamic import("./vault/module")
            ├─ aggregate() 汇总 confs / nav / settings renderers
            ├─ registerConfSlot() 注册设置组
            └─ bindExtensionInput() 绑定设置快照扩展输入
```

`features/vault/` 只是**薄挂载层**：`module.ts` 声明功能元信息，`conf.ts` 声明设置槽位，`settings/` 放功能专属渲染器；真正的保险库 UI 在 `src/vault/`。关闭 feature 时该目录不被引用，`src/vault/` 整块由动态 import 隔离，不进入主包。

---

## 4. 后端结构（`src-tauri/`）

### 4.1 应用层文件（`src-tauri/src/`）

| 文件/目录 | 职责 |
|---|---|
| `main.rs` | 进程入口：解析应用目录 → 安装崩溃处理 → 设置 WebView2 参数 → `assistant_app::run()` |
| `lib.rs` | 模块声明与 `run()`：`plugins::attach` → `setup::init` → `ipc::handler` → 菜单/窗口事件 → `lifecycle::on_run_event` |
| `plugins.rs` | 基础 Tauri 插件链（opener / dialog / notification / log / single-instance / global-shortcut / autostart）+ 遍历 `feature_mounts` 挂载功能插件；日志档位由 `debug_assertions` 决定 |
| `setup.rs` | 初始化：feature `on_setup` → 配置加载 → i18n → `AppDirs` → SQLite 初始化与启动清理（中断流式消息）→ 托盘 → 全局快捷键 |
| `lifecycle.rs` | 退出控制：`ExitRequested` 时按 `SHOULD_EXIT` 决定是否 `prevent_exit`，`Exit` 时执行 feature `on_exit` 钩子 |
| `ipc.rs` | **宿主命令**集中注册（无 `cfg`），feature 命令在各自插件边界注册 |
| `feature_mounts.rs` | 可选功能挂载表 `MOUNTS`：`id` + 插件工厂 + `on_setup` + `on_exit` |
| `vault_plugin.rs` | vault 功能插件边界：`cfg(feature = "vault")` 内注册 60+ 条命令、conf applier、setup 钩子 |
| `window.rs` | 窗口创建/显示/事件、主窗口打开、Orb 展开与收起 |
| `menu.rs` / `tray.rs` | 应用菜单与托盘 |
| `clock.rs` | 世界时钟/计时器后端状态机 |
| `tool_setup.rs` | 工具基础设施生产装配：注册表 → 分组注册 → 授权/审计 → 审批 → 网关 → 流水线 → 执行器端口 |
| `app_dirs/` | 受管目录解析：`roots`（local / config / cache 基根）、`dir_name`、`form` |
| `adapters/` | Agent kernel 端口的宿主实现（见 4.4） |
| `cmd/` | 命令域模块（见 4.2） |
| `protocol/` | DTO 与事件（见 4.3） |

### 4.2 命令层（`cmd/`）

`cmd/mod.rs` 声明全部命令模块并 `pub use` 扁平导出，同时维护 **conf applier 注册表**（`register_conf_applier` / `conf_side_effects`）：配置写入后按 group（`app` / `orb` / `mascot` / `vault`）触发后端副作用，vault 组由功能插件自行注册。

| 模块 | 覆盖命令 |
|---|---|
| `chat/`（`mod.rs` + `insert.rs`） | 发送/取消对话、消息插入与持久化 |
| `agent/` | 发送/取消 Agent、工具调用决议、提问决议、待决提问查询 |
| `vault/`（`audit` `biometric` `conf` `entries` `lifecycle` `recovery` `reveal` `ssh` `sync`） | 保险库全部命令，按子域拆分 |
| `conversation.rs` / `conversation_utils.rs` | 会话 CRUD、分支、置顶、复制、批量删除 |
| `provider.rs` / `agent_mode.rs` / `workspace.rs` | Provider、Agent 模式、工作区 |
| `config.rs` / `app.rs` | 配置读写与 patch、版本信息、退出 |
| `orb.rs` | 鼠标位置、屏幕尺寸、吸附、穿透 |
| `tool.rs` / `os_tool_exec.rs` / `permission.rs` | 工具清单、OS 工具执行、权限授权 |
| `storage.rs` / `attachment.rs` / `token_usage_record.rs` / `compaction.rs` / `mascot_asset.rs` / `clock.rs` | 存储统计与附件、token 用量、手动压缩、Mascot 资源、时钟 |

### 4.3 协议层（`protocol/`）

| 部分 | 内容 |
|---|---|
| `dto/` | 按域拆分的 DTO：`agent` `branch` `chat` `clock` `config` `orb` `os_tool` `settings` `storage` `workspace`，`dto/mod.rs` 汇总导出并承载 ts-rs 导出 |
| `events.rs` | 事件名与 payload DTO、`emit_broadcast` 等广播辅助 |
| 生成物 | `src/types/protocol.contract.ts`（前端契约）、`src/vault/core/contract/vault.contract.ts`（vault 契约，由 vault crate 导出） |

### 4.4 宿主适配层（`adapters/`）

Agent kernel 只定义端口，宿主实现全部集中在 `adapters/`，每个文件对应一组端口：

| 文件 | 实现 |
|---|---|
| `runner.rs` | `build_turn_runner`：装配一轮执行的 runner |
| `event_sink.rs` / `compaction_sink.rs` | 事件与压缩结果输出到 Tauri |
| `message_store.rs` | 会话/消息读写（SQLite） |
| `input_source.rs` | 用户输入来源 |
| `approval.rs` | 审批提示器：Tauri 交互、超时策略、全拒绝策略、待决注册表 |
| `question.rs` | Agent 提问提示器与待决注册表 |
| `permission_policy.rs` / `permission_store.rs` | 权限策略解析、授权存取与审计 |
| `tool_exec.rs` / `tool_resolver.rs` | 工具执行管线与可用工具解析 |
| `system_clock.rs` | 时钟端口 |

### 4.5 Rust workspace（`src-tauri/crates/`，12 个）

**依赖图**（仅 workspace 内部边，共 5 条，无环、无跨层回指）：

```
assistant-app (根包, src-tauri/src)
├── assistant-config        叶子
├── assistant-i18n          叶子
├── assistant-crypto        叶子
├── assistant-orb           叶子（唯一依赖 tauri 的子 crate）
├── assistant-tools         叶子
├── assistant-storage       叶子
├── assistant-crash         叶子
├── assistant-providers     叶子
├── assistant-sync          叶子
├── assistant-security  → assistant-crypto
├── assistant-agent     → assistant-providers, assistant-storage
└── assistant-vault     → assistant-crypto, assistant-sync
```

**分层**：

| 层 | crate |
|---|---|
| 基础设施 | `config` / `i18n` / `crypto` / `storage` / `crash` / `tools` |
| 能力 | `providers` / `orb` / `sync` |
| 领域 | `agent` / `security` / `vault` |
| 应用 | `assistant-app`（只做 IPC、编排、状态装配） |

**逐 crate 职责与模块**：

| crate | 职责 | 关键模块 |
|---|---|---|
| `assistant-config` | 纯 JSON 配置读写（不依赖 tauri / 业务 crate） | `app_conf` `orb_conf` `mascot_conf` `vault_conf` `shortcut` `atomic_write` `json_config` `constant` |
| `assistant-i18n` | 后端文案与语言目录；`build.rs` 生成语言 revision 常量，`locales/*.json` 经 `include_str!` 编译期嵌入 | `lib.rs`（`t` / `t_with_args` / `set_locale`） |
| `assistant-crypto` | 加密原语：Provider API Key 加密、vault 加密与 TOTP | `api_key` `vault_crypto/`（`mod` `totp` `types`）`error` |
| `assistant-storage` | SQLite 持久化（会话、附件、工具历史、Provider、模式、工作区、token 用量） | `db` `migrations` `conversations/`（`crud` `msg_ops` `branch` `tree_ops` `compression` `ops`）`attachment_store` `content_codec` `providers` `agent_modes` `workspaces` `tool_history` `tool_favorites` `token_usage` |
| `assistant-providers` | 三家 LLM 的请求构造、SSE 解析、工具调用累积、视觉与模型目录 | `openai/` `anthropic/` `gemini/`（各含 `request` `stream` `message_converter` `types` `discovery`；openai 另有 `chat` `responses_*`）`client/` `stream/` `types/` `base` `capability` `model_catalog` `vision` `sse_log` `trace` `diagnostics` |
| `assistant-agent` | Agent 运行时与对话编排：kernel 抽象 + 循环执行 + 请求组装 | `kernel/`（`runner` `request` `stream` `persist` `emit` `outcome` `state` `phase` `tasks` `limits` `ids` `ports`）`agent/`（`loop_executor/` `round_executor` `ladder` `middleware` `tool_call` `tool_event` `stop`）`chat_options` `compaction` `normalization` `prompt_builder` `provider_config` `request_projection` `multimodal` `pdf_text` `token_estimate` |
| `assistant-tools` | Agent OS 工具定义、网关与执行 | `gateway` `definition` `exec` `context` `os/`（`agent_read` `agent_write` `agent_edit/` `agent_grep` `agent_glob` `agent_list` `agent_mkdir` `agent_move` `agent_delete` `agent_ask_user`） |
| `assistant-orb` | 悬浮球纯计算：几何、悬停、屏幕放置（仅 `placement` 触达 tauri） | `geometry` `hover` `placement` |
| `assistant-crash` | 崩溃捕获与 minidump 落盘（目录由应用层传入） | `crash_dump` |
| `assistant-security` | 安全能力：剪贴板守卫、揭示守卫、SSH Agent、屏幕锁定、HIBP、原生解锁对话框 | `clipboard_guard` `reveal_guard` `ssh_agent/` `screen_lock/` `unlock_dialog/`（Windows 自绘实现按 `layout` `paint` `wnd_proc` `subclass` 等拆分）`hibp/` `vault_crypto/` |
| `assistant-sync` | Git 同步底层：仓库操作、SSH 凭据、敏感信息脱敏 | `git_sync/`（`git` `ssh`）`types/` `redact` |
| `assistant-vault` | 保险库领域实现：加密库、文件格式、同步引擎、生物识别、恢复密钥、审计 | `security_db/`（`entries/` 与 `biometric_ops` `crypto_ops` `rekey` `schema` `import_export` `meta`）`vault_file/` `sync_engine/`（`export` `history` `lifecycle` `preview` `pull_push` `remote_admin`）`biometric/` `recovery_key/` `audit/` `password_strength/` `plaintext_export` `dto/` `types/` `shared/` `lifecycle` |

### 4.6 可选功能的后端挂载

```
config/features.json (vault)
  ├─ Cargo.toml: default = ["vault"]
  │    └─ vault feature = dep:assistant-vault + dep:assistant-sync + dep:assistant-security
  ├─ lib.rs / plugins.rs: feature_mounts::mounts() 遍历 → 挂载 vault 插件
  ├─ vault_plugin.rs: cfg(feature = "vault") 内注册命令与 conf applier
  └─ capabilities/vault.json: vault 插件命令权限（作用于 main / orb 窗口）
```

关闭 feature 时：可选依赖不编译、vault 命令不注册、`cmd/vault` 不参与编译；前端对应常量 `__VAULT_ENABLED__ = false`，动态 import 不执行。

---

## 5. 契约生成与校验

| 项 | 命令/位置 |
|---|---|
| 生成 | `npm run contract:gen`：`cargo test -p assistant-vault --lib export_bindings` + `cargo test -p assistant-app --test protocol_bindings` |
| 校验 | `npm run contract:check`：重新生成并比对 `src/types/protocol.contract.ts`、`src/vault/core/contract/vault.contract.ts`、`src/types/shortcutDefaults.generated.ts` |
| 规则 | 生成文件禁止手改；新增设置项需 Rust `assistant-config` struct 与 `protocol/dto` `*ConfDto` 同步加字段 |
| 类型安全 | `as unknown as` 仅允许 `settings/core/types/confUtils.ts` 的 `toConfRecord()` 与 `i18n/index.ts` 的 `translateKey()` |

---

## 6. 测试布局

| 侧 | 位置 | 约定 |
|---|---|---|
| 前端 | 就近 `__tests__/`（`chat/core`、`stores`、`services`、`settings`、`vault/core`、`tools/*` 等） | Vitest；夹具在 `src/test-utils/` |
| 命令名一致性 | `src/services/__tests__/commandNames.test.ts` | 扫描前端 `invoke` / `tauriInvoke` 字面量，与 `rustCommands.ts` 声明清单做双向差集校验 |
| 前端配置快照 | `src/settings/__tests__/fixtures/*.json` | golden 快照（full / vaultless / diff） |
| Rust crate | 各 crate `tests/` 集成测试 | 共享夹具放 `tests/common/mod.rs` |
| 应用层 | `src-tauri/tests/`（如 `protocol_bindings`、`assembly_contract`） | 契约与装配校验 |
| 构建校验 | `scripts/assert-feature-absent.mjs` / `feature-build.ps1` | 验证关闭 feature 后产物不含 vault |

---

## 7. 结构约定

| 约定 | 内容 |
|---|---|
| 拆分判据 | 以"变化原因"拆分文件，不设行数目标；细则见 `docs/principle/CODE-SPLIT-PRINCIPLE.md` |
| 前端边界 | 组件只消费 store/service；编排逻辑放 `services/stream`，通过 ports 与 store 解耦 |
| 状态边界 | 高频流式状态独立到 `chatStreamStore`；领域状态按切片聚合到对应 store |
| 后端边界 | 命令层只做参数校验、状态取用与错误翻译；业务规则下沉到 crate；宿主能力通过 `adapters/` 注入 kernel |
| 依赖方向 | 前端：组件 → store/service → `tauriInvoke`；后端：`cmd` → `adapters` / crate，crate 不回指 `assistant-app` |
| 样式 | 组件同目录 `<Name>.module.css`；页面级样式集中在 `settings/styles`、`tools/tool-page.css` |
| 命名 | Rust 文件按单一职责命名（`*_conf`、`*_slice`、`*_ops`）；前端服务以 `xxxService.ts` 结尾，事件以 `xxxEvents.ts` 结尾 |
| 文档 | `docs/` 只用仓库相对路径；归档文档冻结不再修改 |

---

## 8. 定位索引（改什么去哪里）

| 需求 | 位置 |
|---|---|
| 新增 IPC 命令 | `src-tauri/src/cmd/<domain>.rs` + `src-tauri/src/ipc.rs`（宿主）或 `vault_plugin.rs`（feature） |
| 新增前端命令封装 | `src/services/<domain>Service.ts`（命令名字面量写在调用处）+ `src/test-utils/rustCommands.ts` 声明清单 |
| 新增设置项 | `src-tauri/crates/assistant-config/src/*_conf.rs` + `src-tauri/src/protocol/dto/settings.rs` + `src/settings/core/field/fieldDefinitions.ts` → `npm run contract:gen` |
| 新增设置页 | `src/settings/pages/` + `src/settings/core/builtinRegistry.ts` 注册 |
| 新增工具箱工具 | `src/tools/<tool>/`（三件套）+ `src/tools/registry.ts` |
| 新增 Agent 工具 | `src-tauri/crates/assistant-tools/src/os/` + `gateway` 注册 + `src-tauri/src/tool_setup.rs` |
| 调整对话链路 | `src/services/stream/{turnOrchestrator,turnStrategy}.ts` + `src-tauri/crates/assistant-agent/src/kernel/` |
| 调整 Agent 循环 | `src-tauri/crates/assistant-agent/src/agent/{loop_executor,round_executor,ladder}.rs` |
| 新增 Provider | `src-tauri/crates/assistant-providers/src/<vendor>/` + `src/settings/core/provider/` 定义 |
| 新增窗口 | `src-tauri/src/window.rs` + `src/App.tsx` viewMap + `src/layouts/` |
| 调整 Mascot 表情 | `src/mascot/core/expressionPolicy.ts` + `src/mascot/useMascotChatSync.ts` |
| 调整悬浮球行为 | `src/orb/` + `src-tauri/src/cmd/orb.rs` + `src-tauri/crates/assistant-orb/` |
| 新增可选功能 | `config/features.json` + `src/features/registry.ts` + `src-tauri/src/feature_mounts.rs` + `Cargo.toml` feature |
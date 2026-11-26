# 代码结构（语义层）

> 性质：只写**索引生成不了的语义**——分层、边界、依赖方向、改动路由；不写逐文件清单与模块枚举（这类内容易过期）。
> 逐文件事实（路径 / 领域 / 角色 / 关键导出 / 行数）见 `CODE-INDEX.md`：全文由 `node scripts/structure-index.mjs` 生成，勿手改，代码变动后重跑即刷新。
> 更新日期：2026-11-20。
> 读法：找具体文件 → `CODE-INDEX.md`（Ctrl+F 路径或导出名）；找「改什么去哪」→ §8；找层与依赖方向 → §2 / §4.5。
> 相关文档：`ARCHITECTURE.md`（组件职责与交互语义）、`FRONTEND.md`（UI 设计与参考项目）、`docs/store/crate-architecture.md`（后端 crate 规模与编译归属实测）、`docs/principle/CODE-SPLIT-PRINCIPLE.md`（文件拆分判据）、`vault-optional-build.md`（已归档，vault 可选构建）。

---

## 1. 仓库顶层布局

| 路径 | 领域 | 内容与关键字 |
|---|---|---|
| `src/` | 前端 | React Webview 侧全部代码（领域目录 + `main.tsx` / `App.tsx`） |
| `src-tauri/src/` | 应用层 | 窗口、菜单、托盘、IPC 注册、DTO、宿主适配 |
| `src-tauri/crates/` | 业务能力层 | 12 个独立 crate（见 §4.5） |
| `src-tauri/tests/` | 契约与集成测试 | 协议绑定、装配契约、MCP 端到端、审批时序、权限策略、工具解析、应用目录、时钟运行时 |
| `src-tauri/capabilities/` | 权限清单 | 插件与窗口能力声明（vault 命令权限在 `vault.json`） |
| `src-tauri/tauri.conf.json` / `Cargo.toml` | 应用配置 | 窗口定义、打包、feature 声明（`default = ["vault"]`） |
| `config/features.json` | 功能清单 | 可选功能唯一事实源；前端 Vite define 与后端 Cargo feature 都据此派生 |
| `scripts/` | 构建与维护脚本 | 构建前处理、契约校验、统一检查入口、规模探测、结构索引生成（`structure-index.mjs` → `docs/CODE-INDEX.md`）、可选功能构建校验、工具聚类、字体与图标生成 |
| `docs/` | 文档 | `archived/`（按域分目录，冻结）、`store/`（活跃设计）、`analysis/`（预分析）、`principle/`（规范）、`achievement/`（待归档） |
| `references/` | 参考源码 | 参考项目已分组；gitignored、不参与构建；引用写「项目名 + 相对其仓库根的路径」 |
| `temp-backup/` | 临时区 | 备份与临时脚本产物，不参与构建 |
| `dist/` | 构建产物 | Vite 输出 |

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

| 层 | 路径 | 技术栈 / 关键字 | 约束 |
|---|---|---|---|
| Webview 前端 | `src/` | React 19、Zustand、antd、i18next | 只经 `src/services/` 调后端；组件不直接 `invoke` |
| Tauri 应用层 | `src-tauri/src/` | 窗口 / 菜单 / 托盘、IPC、DTO、宿主适配 | 只做参数校验、状态取用与错误翻译；业务规则下沉 crate |
| 业务能力层 | `src-tauri/crates/` | 12 个 crate（见 §4.5） | 不回指 `assistant-app` |

| 通道 | 载体 | 说明 |
|---|---|---|
| 命令通道 | `src-tauri/src/ipc.rs` + `src-tauri/src/cmd/` | 前端 `invoke()` 调用的全部宿主命令在此注册；feature 命令在各自插件边界注册 |
| 事件通道 | `src-tauri/src/protocol/events.rs` | Rust 向前端推送：流式增量、状态变更、跨窗口广播 |
| 契约通道 | `src-tauri/src/protocol/dto/` → `src/types/protocol.contract.ts` | Rust DTO 经 ts-rs 生成 TypeScript 类型，禁止手改生成物 |

---

## 3. 前端（`src/`）

### 3.1 启动顺序与窗口分流

| 路径 | 角色 | 顺序与语义 |
|---|---|---|
| `src/main.tsx` | 启动入口 | Rive wasm URL 设置 → 设置系统初始化 → 日志与全局错误处理 → bootstrap（加载功能 → 逐组取配置并向后端加载 → 应用语言）→ 渲染错误边界 → 应用根 |
| `src/App.tsx` | 窗口分流 | 按当前 Webview label 选布局：主窗口 → 常规布局、悬浮球窗口 → Orb 布局、未知回落常规布局 |
| `src/layouts/` | 窗口布局 | 外壳与两种形态（常规 / 迷你）+ 窗口装饰控制 + Orb 布局 |

### 3.2 领域职责与边界（语义）

| 路径 | 职责 | 内部构成与边界 |
|---|---|---|
| `src/chat/` | 对话展示与纯逻辑 | `core/`（消息树 / 内容块 / 流合并 / 错误恢复）· `message/`（气泡 / 列表 / 输入 / 附件预览）· `blocks/` · `selectors/` · `tool/` · `attachment/`（intake / admit / factory / paths / sources）· `layouts/`（单骨架 + 工具区域挂点）。总装入口不 import 工具 / 工作区组件、不构造布局 JSX |
| `src/history/` | 会话列表 / 搜索 / 详情 / 分支树 / 虚拟列表 | `core/` 与列表、详情两侧视图分离 |
| `src/tools/` | 工具箱框架 + 内置工具 | 框架（注册、分类、UI 外壳）+ 每个工具一套「视图入口 + 纯逻辑服务 + 组件样式」 |
| `src/vault/` | 保险库（可选功能）全部 UI 与状态 | `core/`（store + 切片 + 服务 + 契约 + 展示 + hooks + `audit/`）· `entry/`（详情 / 历史 / 揭示 / Drawer + `forms/` 字段编辑器）· `settings/` · `unlock/` · 顶层页面 |
| `src/settings/` | 声明式设置系统 | 四层：声明（字段 / Provider / Agent 模式）→ 注册表（页注册、内置注册、副作用注册）→ 运行时（配置运行时、快照、分发、乐观更新）→ 视图（各设置页与共享渲染器；权限审计结果词表在 `shared/sections/PermissionAuditSection/permissionOutcomes.ts`） |
| `src/services/` | 与后端交互的唯一边界 | 按领域分 `stream/` `chat/` `tool/` `desktop/` `system/`；无 UI 组件 |
| `src/stores/` | Zustand 状态 | `chat/`（聚合 + 切片 + ports + 独立流式 store）· `agent/`（分段、模式、观测投影）· `system/`（语言 / 通知 / 主题 / 窗口模式）· 根级（历史 / 工具 / Provider / 设置 / MCP / 附件 / 悬浮球 / 截屏）。聚合与编排解耦，高频流式独立存放 |
| `src/mascot/` | 形象与表情 | 策略表 + 多渲染器 + 与对话状态同步 |
| `src/orb/` | 悬浮球 | 拖拽 / 吸附 / 展开 / 指针手势 / 配置同步 / 运行态标记 |
| `src/screenshot/` | 截屏提问 | 选区层 + 截屏会话 |
| `src/components/` | 通用组件 | 错误边界、侧栏、标题栏、窗口按钮、命令面板 |
| `src/hooks/` | 跨模块 Hook | 初始化、主题、导航监听、错误通知、文件拖放 |
| `src/utils/` | 通用工具 | 字节 / 时间格式化、懒加载恢复、模块载入、持久化队列 |
| `src/i18n/` | 国际化 | i18next 初始化、语言包、语言生效链路 |
| `src/features/` | 功能挂载点 | 可选功能（vault）与常驻功能（clock / observation / orchestration）的注册、聚合与导航注入；`canvas/` 为共享画布设施，不注册为 feature |
| `src/types/` | 契约类型 | 生成契约 + 事件类型（生成物禁止手改） |
| `src/test-utils/` | 测试夹具 | 夹具、命令桩、Rust 命令名声明清单 |
| `src/assets/styles/` | 样式与主题 | 全局样式与 antd 令牌 |

### 3.3 逐文件索引

见 `CODE-INDEX.md` §1（脚本生成）。

### 3.4 可选功能的前端挂载

```
config/features.json
  └─ vite.config.ts featureDefines() → __VAULT_ENABLED__（编译期常量）
       └─ features/registry.ts loadFeatures()
            ├─ 条件 dynamic import("./vault/module")
            ├─ aggregate() 汇总 confs / nav / settings renderers
            ├─ registerConfSlot() 注册设置组
            └─ bindExtensionInput() 绑定设置快照扩展输入
```

关闭时编译期常量置 false，动态 import 不执行，功能代码不进主包。

---

## 4. 后端（`src-tauri/`）

### 4.1 应用层装配链

```
main.rs → assistant_app::run()
  ├─ plugins::attach()      固定插件链 + 遍历 feature_mounts 挂载可选功能插件
  ├─ setup::init()          feature on_setup → 配置加载 → i18n → 应用目录 → SQLite 初始化与启动清理 → 托盘 → 全局快捷键
  ├─ ipc::handler()         宿主命令集中注册（无 cfg）
  ├─ 菜单 / 窗口事件
  └─ lifecycle::on_run_event()   退出控制（ExitRequested 按标志决定 prevent_exit；Exit 执行 feature on_exit）
```

应用层只做参数校验、状态取用与错误翻译，业务规则一律下沉 crate；可选功能通过挂载表（`feature_mounts.rs`）以「插件工厂 + setup / exit 钩子」接入，关闭 feature 时整条链不编译。

### 4.2 命令层（`cmd/`）

`cmd/mod.rs` 声明全部命令模块并扁平导出，同时维护 **conf applier 注册表**（`register_conf_applier` / `conf_side_effects`）：配置写入后按 group（`app` / `orb` / `mascot` / `vault`）触发后端副作用，vault 组由功能插件自行注册。命令函数只做参数校验、状态取用与错误翻译；feature 命令在其插件边界注册，不进宿主注册表。

### 4.3 协议层（`protocol/`）

DTO 按域拆成 `dto/` 下的子模块，由 `dto/mod.rs` 汇总并承载 ts-rs 导出（新增域须在此登记才会生成前端类型）；`events.rs` 放事件名常量与跨窗口广播辅助。生成物 `src/types/protocol.contract.ts`、`src/vault/core/contract/vault.contract.ts` 禁止手改。

### 4.4 宿主适配层（`adapters/`）

Agent kernel 只定义端口，宿主实现全部集中在 `adapters/`，一类端口对应一组文件：轮次装配、事件与压缩输出、消息持久化、用户输入、审批、提问、权限策略与授权存取、工具执行与解析、工具层级、系统时钟。

### 4.5 Rust workspace（`src-tauri/crates/`，12 个）

**依赖图**（仅 workspace 内部边，共 6 条，无环、无跨层回指）：

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
└── assistant-vault     → assistant-crypto, assistant-sync, assistant-config
```

**分层**：基础设施 `config` / `i18n` / `crypto` / `storage` / `crash` / `tools`；能力 `providers` / `orb` / `sync`；领域 `agent` / `security` / `vault`；应用 `assistant-app`（只做 IPC、编排、状态装配）。

| crate | 层 | 职责 |
|---|---|---|
| `assistant-config` | 基础设施 | 配置机制库（加载 / 保存 / 校验 / patch）与核心配置组 schema；不依赖 tauri 与业务 crate |
| `assistant-i18n` | 基础设施 | 后端文案与语言目录；构建期生成语言 revision，语言文件编译期嵌入 |
| `assistant-crypto` | 基础设施 | 加密原语：Provider API Key、vault 加密、TOTP |
| `assistant-storage` | 基础设施 | SQLite 持久化：会话、附件、工具历史、Provider、模式、工作区、MCP server、token 用量 |
| `assistant-crash` | 基础设施 | 崩溃捕获与 minidump 落盘（目录由应用层传入） |
| `assistant-tools` | 基础设施 | Agent OS 工具定义、网关、策略、执行管线、MCP 与沙箱契约 |
| `assistant-providers` | 能力 | 各家 LLM 请求构造、SSE 解析、工具调用累积、视觉与模型目录 |
| `assistant-orb` | 能力 | 悬浮球纯计算（几何 / 悬停 / 屏幕放置），唯一触达 tauri 的子 crate |
| `assistant-sync` | 能力 | Git 同步底层：仓库操作、SSH 凭据、敏感信息脱敏 |
| `assistant-agent` | 领域 | Agent 运行时与对话编排：kernel 抽象 + 循环执行 + 请求组装 |
| `assistant-security` | 领域 | 剪贴板 / 揭示守卫、SSH Agent、屏幕锁定、HIBP、原生解锁对话框 |
| `assistant-vault` | 领域 | 保险库领域实现：加密库、文件格式、同步引擎、生物识别、恢复密钥、审计 |

### 4.6 逐文件索引

见 `CODE-INDEX.md` §2（脚本生成）。

### 4.7 可选功能的后端挂载

```
config/features.json (vault)
  ├─ Cargo.toml: default = ["vault"]
  │    └─ vault feature = dep:assistant-vault + dep:assistant-sync + dep:assistant-security
  ├─ lib.rs / plugins.rs: feature_mounts::mounts() 遍历 → 挂载 vault 插件
  ├─ feature_plugins/mod.rs → feature_plugins/vault_plugin.rs: cfg(feature = "vault") 内注册命令与 conf applier
  └─ capabilities/vault.json: vault 插件命令权限（作用于 main / orb 窗口）
```

关闭 feature 时：可选依赖不编译、vault 命令不注册、`cmd/vault` 不参与编译；前端对应常量 `__VAULT_ENABLED__ = false`，动态 import 不执行。

---

## 5. 契约生成与校验

| 路径 / 命令 | 领域 | 内容与关键字 |
|---|---|---|
| `npm run contract:gen` | 生成 | 跑 vault 的 `export_bindings` 与应用层的 `protocol_bindings` 测试，写出前端契约 |
| `npm run contract:check` | 校验 | 重新生成并比对 `src/types/protocol.contract.ts`、`src/vault/core/contract/vault.contract.ts`、`src/types/shortcutDefaults.generated.ts` |
| 核心配置组 `*_conf.rs` + 对应 `*ConfDto` | 新增设置项 | Rust struct 与 DTO 同步加字段（Rust 侧 `#[serde(default)]`）→ `contract:gen` |
| 特性配置组 `conf.rs` + 特性契约域 DTO | 新增特性设置项 | 同上；vault 的 DTO 在特性契约域内 |
| `src/settings/core/types/confUtils.ts` / `src/i18n/index.ts` | 类型安全 | `as unknown as` 仅允许此两处（`toConfRecord()` / `translateKey()`） |

---

## 6. 测试布局

| 路径 | 领域 | 约定 |
|---|---|---|
| `src/**/__tests__/` | 前端 | 就近放置；Vitest，夹具在 `src/test-utils/` |
| `src/services/__tests__/commandNames.test.ts` | 命令一致性 | 扫描前端命令名字面量，与 `rustCommands.ts` 双向差集校验 |
| `src/settings/__tests__/fixtures/*.json` | 配置快照 | golden（full / vaultless / diff） |
| `src-tauri/crates/*/tests/` | crate 集成测试 | 共享夹具放 `tests/common/mod.rs` |
| `src-tauri/tests/` | 应用层 | 契约、装配与跨模块集成；MCP 测试服务在 `tests/fixtures/` |
| `scripts/*.test.mjs` | 脚本自测 | 可选功能构建校验脚本的自测 |
| 可选功能构建校验脚本 | 构建校验 | 关闭 feature 后产物不含 vault：字符串探针、Vite 产物图断言、依赖图断言 |

---

## 7. 结构约定

| 约定 | 内容 |
|---|---|
| 拆分判据 | 以「变化原因」拆分文件，不设行数目标；细则见 `docs/principle/CODE-SPLIT-PRINCIPLE.md` |
| 前端边界 | 组件只消费 store / service；编排逻辑放 `services/stream`，通过 ports 与 store 解耦 |
| 状态边界 | 高频流式状态独立到 `chatStreamStore`；领域状态按切片聚合到对应 store |
| 后端边界 | 命令层只做参数校验、状态取用与错误翻译；业务规则下沉 crate；宿主能力通过 `adapters/` 注入 kernel |
| 依赖方向 | 前端：组件 → store / service → `tauriInvoke`；后端：`cmd` → `adapters` / crate，crate 不回指 `assistant-app` |
| 样式 | 组件同目录 `<Name>.module.css`（索引不列样式）；页面级样式集中到对应域的样式目录 |
| 命名 | Rust 按单一职责命名（`*_conf`、`*_ops`、`*_store`）；前端服务 `xxxService.ts`、事件 `xxxEvents.ts`、切片 `xxxSlice.ts`、Hook `useXxx.ts` |
| 文档 | `docs/` 只用仓库相对路径；归档文档冻结不再修改；脚本生成的文档不手改 |

---

## 8. 定位索引（改什么去哪里）

| 需求 | 位置 |
|---|---|
| 新增 IPC 命令 | `src-tauri/src/cmd/<domain>/` + `src-tauri/src/ipc.rs`（宿主）或功能插件边界（feature） |
| 新增前端命令封装 | `src/services/<domain>/`（命令名字面量写在调用处）+ `src/test-utils/rustCommands.ts` 声明清单 |
| 新增设置项 | 核心 group 在 `assistant-config` 的配置组 struct；特性 group 在特性 crate 的 `conf.rs` + 特性契约域 DTO + 设置字段声明 → `npm run contract:gen` |
| 新增设置页 | `src/settings/pages/` + 内置页注册表登记 |
| 新增工具箱工具 | `src/tools/<tool>/`（三件套）+ 工具注册表登记 |
| 新增 Agent 工具 | `assistant-tools` 的 `os/tools/`（文件系统工具）或 `os/tools/meta/`（元工具）+ 网关注册 + `src-tauri/src/tool_setup.rs` |
| 新增 / 调整 MCP server | `assistant-tools` 的 MCP 模块（协议 / 管理 / 工具桥）+ `src-tauri/src/mcp/`（宿主传输与配置加密）+ 对应命令与 DTO |
| 调整沙箱探测 / 后端 | `assistant-tools` 的沙箱模块 + `src-tauri/src/tool_setup.rs`（当前仅契约与类型，平台后端与 shell 工具尚未接入） |
| 调整对话链路 | `src/services/stream/` + `assistant-agent` 的 kernel |
| 调整 Agent 循环 | `assistant-agent` 的循环执行与阶梯模块 |
| 新增 Provider | `assistant-providers` 新增厂商目录 + 设置侧 Provider 定义 |
| 新增窗口 | `src-tauri/src/window.rs` + `src-tauri/src/protocol/dto/window.rs` + `src/App.tsx` 分流 + `src/layouts/` |
| 调整 Mascot 表情 | `src/mascot/` 的策略表与状态同步；素材 URL 拼接在 `src/mascot/core/assetUrl.ts` |
| 新增审计事件类型 | vault 侧：`assistant-vault` 审计事件枚举 + `src/vault/core/audit/vaultEvents.ts`（事件串 / i18n 键）；permission 侧：`src-tauri/src/adapters/permission_store.rs` + `src/settings/shared/sections/PermissionAuditSection/permissionOutcomes.ts` |
| 调整悬浮球行为 | `src/orb/` + `src-tauri/src/cmd/orb.rs` + `assistant-orb` |
| 调整截屏 | `src/screenshot/` + `src-tauri/src/screenshot/` + 截屏命令 |
| 新增可选功能 | `config/features.json` + 前端功能注册表 + `src-tauri/src/feature_mounts.rs` + `Cargo.toml` feature |
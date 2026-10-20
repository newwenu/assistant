# I19 — 统一错误处理体系规格说明

> **前置依赖**：I17 ✅（Chat 重发/重试恢复体系已完成，`chatErrors.ts` + `chatErrorRecovery.ts` 已实现）
> **核心参考**：OpenBitFun `logger.ts` + `AppErrorBoundary.tsx` + `reactProductionError.ts` + `lazyWithRecovery.tsx` + `moduleLoader.ts` + `main.tsx` 全局错误处理器；airi `ApiError` 工厂方法 + `error-boundary.vue`

## 1. 目标

将当前散落各处的 `console.error` / `.catch(() => {})` / 局部 ErrorBoundary 统一为**分层防御体系**，使错误在**生产环境可回溯**、**用户可感知**、**开发者可诊断**。

**设计原则**：
- **分层防御**：全局兜底 → App 级 ErrorBoundary → Feature 级 ErrorBoundary → 结构化错误 → 静默吞没（逐步消除）
- **生产可回溯**：所有错误通过 `createLogger` 写入 Tauri `webview.log`，用户可导出诊断日志
- **用户可感知**：Store 级错误通过通知系统推送，不再仅 console
- **渐进实施**：每个 Step 独立可测，不破坏现有行为

## 2. 现状分析

### 2.1 当前错误处理全景

| 层级 | 机制 | 位置 | 覆盖范围 | 用户感知 | 生产可回溯 |
|------|------|------|---------|---------|-----------|
| **Chat 结构化错误** | `ChatErrorCode` + `ChatErrorCategory` + `ChatErrorPresentation` + `RecoveryAction` | `chat/core/chatErrors.ts` + `chatErrorRecovery.ts` | 仅 Chat 流式错误 | ✅ ErrorBlock + RetryBanner | ❌ 仅 console |
| **Compact 错误** | `compactError: string \| null` | `chatStore.ts` | 仅对话压缩 | ✅ CompactErrorBanner | ❌ 仅 console |
| **React ErrorBoundary** | `LazyErrorBoundary` (Class Component) | `tools/ToolPage.tsx` | 仅工具页懒加载 | ✅ Result error UI | ❌ 无持久化 |
| **console.error 散落** | `console.error("xxx failed:", e)` | 17 个文件共 56 处（见 §2.4） | 所有非 Chat 错误 | ❌ 用户不可见 | ❌ 生产环境丢失 |
| **静默吞没** | `.catch(() => {})` / `.catch(console.error)` | 8 处（见 §2.3） | cancelChat / incrementGeneration / clipboard 等 | ❌ | ❌ |
| **tauriInvoke 透传** | `console.error` + `throw e` | `services/tauriInvoke.ts` | 所有 Tauri invoke | ❌ 由调用方决定 | ❌ 仅 console |
| **Rust crash_dump** | `panic hook` + `crash_handler` | `src-tauri/crash_dump.rs` | Rust panic / 原生崩溃 | ❌ 无 UI | ✅ 写入 `crashes/` 目录 |

### 2.2 关键缺失

| 缺失项 | 影响 | 严重度 |
|--------|------|--------|
| **无 App 级 ErrorBoundary** | React 渲染异常 → 白屏，用户只能重启 | 🔴 P0 |
| **无 `window.error` / `unhandledrejection` 监听** | Promise 静默失败、脚本加载错误无感知 | 🔴 P0 |
| **无统一日志系统** | 生产环境 `console.error` 丢失，无法回溯用户问题 | 🔴 P0 |
| **`@tauri-apps/plugin-log` 前端包未安装** | Rust 端已注册插件（`lib.rs` 中 `.plugin(tauri_plugin_log::Builder::default().build())`），但 `package.json` 缺少前端 JS 绑定，Step 1 无法工作 | 🔴 P0 |
| **Store 错误用户不可见** | `renameConversation` / `updateModel` / `deleteMessage` 等失败用户无感知 | 🟡 P1 |
| **`compactError` 非结构化** | 无法扩展恢复策略，仅字符串 | 🟡 P1 |
| **`lazyWithRecovery` 缺失** | 代码分割加载失败后重试复用已失败的 chunk，无法恢复 | 🟡 P1 |
| **设置页 `React.lazy` 无 ErrorBoundary** | `pageRegistry.ts` 中所有设置页均用 `React.lazy` 但无错误边界，加载失败冒泡至白屏 | 🟡 P1 |
| **Chat 错误无 Tauri 持久化** | `errorStream()` 仅设 state，不写日志 | 🟢 P2 |
| **无 React 压缩错误解码** | 生产环境 `Minified React error #301` 无意义 | 🟢 P2 |

### 2.3 静默吞没清单

以下 `.catch(() => {})` 和 `.catch(console.error)` 需逐一评估是否应改为日志记录或用户通知：

| 位置 | 代码 | 评估 |
|------|------|------|
| `chatStore.ts:254` | `incrementGeneration(convId).catch(() => {})` | 🟢 可保留：generation 递增是尽力而为，失败无影响 |
| `chatStore.ts:278` | `getConversationTokenUsage(...).catch(() => {})` | 🟢 可保留：token 用量是辅助信息 |
| `chatStore.ts:407` | `cancelChat(prevConvId).catch(() => {})` | 🟡 改为 `log.warn`：cancel 失败可能表示后端状态不一致 |
| `chatStore.ts:462` | `getConversationTokenUsage(...).catch(() => {})` | 🟢 同上 |
| `orchestrator.ts:106` | `cancelChat(convId).catch(() => {})` | 🟡 改为 `log.warn`：ensureIdle 中 cancel 失败值得记录 |
| `orchestrator.ts:142` | `cancelChat(prevConvId).catch(() => {})` | 🟡 改为 `log.warn`：同上 |
| `ChatView.tsx:227` | `updateConversationProvider(...).catch(() => {})` | 🟡 改为 `log.warn`：Provider 更新失败可能导致下次启动时 Provider 不一致 |
| `MessageBubble.tsx:74` | `navigator.clipboard.writeText(text).catch(() => {})` | 🟢 可保留：剪贴板写入失败无需通知用户 |

此外，`OrbDrag.tsx` 中有 5 处 `.catch(console.error)`（`setOrbClickThrough` / `snapOrbToEdge`），迁移后自动受益于 console 劫持持久化，无需额外处理。

### 2.4 console.error 清单

以下 `console.error` / `console.warn` 需迁移到统一日志系统（共 56 处 / 17 个文件）：

| 文件 | 出现次数 | 典型调用 |
|------|---------|---------|
| `chatStore.ts` | 16 | `console.error("Failed to load conversations:", e)` |
| `HistoryManager.tsx` | 10 | `console.error("Failed to delete conversation:", e)` |
| `OrbDrag.tsx` | 5 | `.catch(console.error)` 模式 |
| `OrbExpand.ts` | 3 | `console.error("[OrbExpand] expand invoke failed:", e)` |
| `settingsStore.ts` | 3 | `console.error("[settingsStore] loadAppConf failed:", e)` |
| `ChatView.tsx` | 3 | `console.error("Failed to update provider:", e)` |
| `chatOrchestrator.ts` | 2 | `console.error("[orchestrator] sendChatMessage failed:", e)` |
| `chatDeltaEvents.ts` | 2 | `console.error("[chat-delta] ...")` |
| `SettingsGroup.tsx` | 2 | `console.error("[SettingsGroup] patch failed ...")` |
| `useHistoryRefresh.ts` | 2 | `console.error("Failed to refresh conversations:", e)` |
| `settingsDispatcher.ts` | 2 | `console.error` + `console.warn` |
| `providerStore.ts` | 1 | `console.error("Failed to load providers:", e)` |
| `agentModeStore.ts` | 1 | `console.error("Failed to load agent modes:", e)` |
| `tauriInvoke.ts` | 1 | `console.error('[tauriInvoke] ...')` |
| `themeStore.ts` | 1 | `.catch(console.error)` 模式 |
| `OrbContextMenu.tsx` | 1 | `console.error(err)` |
| `useHistorySearch.ts` | 1 | `console.error("Search failed:", e)` |

## 3. 参考项目做法

### 3.1 OpenBitFun — 四层防御体系

#### Layer 1：统一日志 `createLogger` + Tauri 持久化

OpenBitFun 的 `logger.ts` 采用**单例 Logger + 工厂方法**模式：`Logger.getInstance()` 管理全局日志级别，`createLogger(context)` 返回带上下文标签的轻量代理对象。核心要点：

- **分级日志**：`LogLevel` 枚举（TRACE/DEBUG/INFO/WARN/ERROR/NONE），低于 `currentLevel` 的不输出
- **双通道输出**：Tauri 环境下同时写 console + `@tauri-apps/plugin-log`（持久化到 `webview.log`）；非 Tauri 环境仅 console
- **console 劫持**：`bootstrapLogger()` 在应用启动最早阶段安装，把 `console.error/warn/info/debug/trace/log` 全部转发到 Tauri 日志插件。劫持带有**分级过滤**（`consoleForwardMinLevel`），生产环境默认 WARN 以上才转发，避免 TRACE 级别洪水
- **Error 格式化**：`formatError()` 处理 WebKit（Tauri macOS）的 stack 缺失 message 问题——`error.stack` 只有帧信息没有 `error.message`，需手动拼接
- **多参数格式化**：`formatConsoleArgs(args)` 逐个格式化每个参数后 `join(' ')`，确保 `console.error("msg", errorObj)` 中 Error 对象的 stack 不丢失
- **敏感诊断控制**：`setIncludeSensitiveDiagnostics()` 可动态关闭，避免日志泄露敏感信息
- **启动日志级别注入**：通过 `globalThis.__OPENBITFUN_BOOTSTRAP_LOG_LEVEL__` 在 WebView 初始化脚本中设置，早于前端 bundle 执行

#### Layer 2：全局错误处理器

OpenBitFun 在 `main.tsx` 中 `registerGlobalErrorHandlers()` 注册 `window.error` + `unhandledrejection`。关键设计：

- **延迟白屏检测**：不直接在事件回调中调用 `isRootEmpty()`，而是通过 `queueMicrotask → requestAnimationFrame → requestAnimationFrame` 延迟两帧后检测，避免 React 尚未完成渲染时误判
- **白屏去重**：`WHITE_SCREEN_LOGGED_FLAG` 确保白屏崩溃只记录一次，但 ErrorBoundary 的日志不受此限制——后续不同错误仍会记录
- **React 压缩错误优先持久化**：`isMinifiedReactErrorMessage(msg)` 检测后立即 `log.error`，不等待延迟调度，确保 `react.dev/errors/{code}` 信息不丢失
- **资源加载错误降级**：仅当 `isRootEmpty()` 为 true 时才记录资源加载错误（资源错误很少导致白屏，减少噪音）
- **capture 阶段注册**：`addEventListener('error', ..., true)` 使用捕获阶段，确保最先处理

#### Layer 3：多层 ErrorBoundary

| 层级 | 组件 | 包裹范围 | 恢复方式 |
|------|------|---------|---------|
| App 级 | `AppErrorBoundary` | 整个 `<App/>` | 页面 Reload |
| Feature 级 | `FlowToolCardErrorBoundary` | 每个 ToolCard | 单卡片重试，不影响对话流 |
| 懒加载级 | `lazyWithRecovery` | 动态 import | 重试创建新 `React.lazy()` 实例 + 监听模块恢复事件 |

**`AppErrorBoundary` 关键设计**：使用**纯 HTML/CSS + 自有 Button 组件**渲染兜底 UI，不依赖 antd 等重 UI 库。原因：如果 UI 库本身加载/渲染出错，依赖它的 ErrorBoundary 也会崩溃，反而加剧白屏。开发模式下额外展示 error stack + componentStack 的 `<details>` 折叠区。

**`FlowToolCardErrorBoundary` 关键设计**：每个 ToolCard 独立容错，渲染失败时显示错误卡片（带工具名 + 错误首行 + 重试按钮），不影响对话流中其他卡片。截断过长 payload（4000 字符上限）避免日志爆炸。

**`lazyWithRecovery` 关键设计**：

- `ModuleImportError` 区分 import 错误和 render 错误——`getDerivedStateFromError` 中非 `ModuleImportError` 继续冒泡至上层 ErrorBoundary
- `componentDidCatch` 中若 `Content === this.state.content`（当前 lazy 实例仍是失败的那个），则创建新实例赋给模块级 `Content`，确保后续新挂载不会继承已失败的 payload
- `componentDidMount` 中若已有 error 则 `listenForRecovery()`——处理 React StrictMode 重放 mount/unmount 的场景
- `componentWillUnmount` 清理 recovery 订阅，避免内存泄漏
- `retry()` 重置 state 并使用模块级最新 `Content`，而非重新创建——让用户手动重试时复用已被其他边界成功加载的模块

**`moduleLoader.ts` 辅助模块**：

- `importWithRetry(load)`：对模块加载错误（`Failed to fetch dynamically imported module` 等）自动重试 2 次（间隔 250ms / 750ms），非模块错误或重试耗尽则抛出
- `createModuleLoader(load)`：共享 preload/render 工作，缓存成功模块的 Promise，但**永远不缓存 rejected promise**——失败后 `pending = undefined` 允许后续重试
- `subscribeToModuleRecovery(retry)` / `retryFailedModuleLoads()`：发布-订阅模式，Vite HMR `vite:afterUpdate` 和 `vite:ws:connect` 事件触发所有失败模块重试

#### Layer 4：React 压缩错误解码

`reactProductionError.ts` 解码 `Minified React error #xxx`，返回 `{ code, decoderUrl, hint? }`。`REACT_HINTS` 目前覆盖 300/301/310 三个最常见错误。`buildReactCrashLogPayload()` 合并 error 基本信息 + componentStack + React 诊断信息为单一可序列化对象。

### 3.2 airi — `ApiError` 工厂方法

airi 服务端使用 `ApiError` 类（继承 `Error`），携带 `statusCode` + `errorCode` + `details`。通过语义化工厂方法（`createBadGatewayError` / `createUnauthorizedError` / `createPaymentRequiredError` 等）创建，每个方法固定 statusCode 和 errorCode，调用方只需传 message 和可选 details。

**对我们的启示**：`ChatError` 接口可增加 `details?: Record<string, unknown>` 字段，让 `toChatError()` 携带更多上下文（Tauri command 名、参数摘要等），替代当前的 `message` 字符串拼接。

### 3.3 airi — Vue `error-boundary.vue`

airi 的 Vue ErrorBoundary 通过 `onErrorCaptured` 捕获子组件异常，返回 `false` 阻止冒泡。支持 `retryable` prop + `fallback` slot + `emit('error')` 事件。retry 通过 `renderKey += 1` 重新挂载子树。`defineExpose` 暴露 `retry()` 和 `hasError()` 供外部调用。

**对我们的启示**：React ErrorBoundary 也应支持 `onError` 回调和 `fallback` render props，让调用方可自定义降级 UI 和错误上报逻辑。

## 4. 设计方案

### 4.1 架构总览

```
┌──────────────────────────────────────────────────────────────────────────┐
│  Layer 6: UI 组件                                                        │
│  ErrorBlock → 结构化错误展示 + 恢复动作按钮                               │
│  RetryBanner → 倒计时 + 取消按钮                                         │
│  CompactErrorBanner → 压缩错误 + 重试                                    │
│  GlobalErrorToast → Store 级错误通知（新增）                              │
├──────────────────────────────────────────────────────────────────────────┤
│  Layer 5: ErrorBoundary 层                                               │
│  AppErrorBoundary → 全局兜底，白屏时提供 Reload（纯 HTML，不依赖 UI 库）  │
│  LazyWithRecovery → 懒加载容错，重试时创建新 lazy 实例                     │
│  FeatureErrorBoundary → 功能级容错（ChatView / Settings / History 等）    │
├──────────────────────────────────────────────────────────────────────────┤
│  Layer 4: 全局错误处理器                                                  │
│  window.addEventListener('error', ...) → 日志 + 延迟白屏检测 + 去重      │
│  window.addEventListener('unhandledrejection', ...) → 日志 + 去重        │
├──────────────────────────────────────────────────────────────────────────┤
│  Layer 3: 统一日志系统                                                    │
│  createLogger(context) → 分级日志 + Tauri 持久化 + console 劫持           │
│  reactProductionError.ts → 压缩错误解码                                  │
├──────────────────────────────────────────────────────────────────────────┤
│  Layer 2: 结构化错误定义                                                  │
│  ChatErrorCode + ChatErrorCategory + ChatError (已有，增强 details 字段)  │
│  AppError (新增) → Store/Service 级结构化错误                             │
├──────────────────────────────────────────────────────────────────────────┤
│  Layer 1: Rust crash_dump (已有)                                         │
│  panic hook → crashes/panic_*.log                                       │
│  crash_handler → crashes/crash_*.dmp + crashes/crash_*.log              │
└──────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Step 1：统一日志系统

**目标**：引入 `createLogger` + `@tauri-apps/plugin-log`，劫持 `console.*` 转发到 Tauri 日志插件，生产环境所有错误可回溯。

**前置操作**：安装前端依赖 `npm install @tauri-apps/plugin-log`。Rust 端 `lib.rs` 已注册 `.plugin(tauri_plugin_log::Builder::default().build())`，无需额外操作。

#### 4.2.1 新增 `src/services/logger.ts`

参照 OpenBitFun `logger.ts` 实现，核心接口：

- `LogLevel` 枚举：TRACE=0, DEBUG=1, INFO=2, WARN=3, ERROR=4, NONE=5
- `createLogger(context: string)` → 返回带 `[context]` 前缀的 Logger 实例
- Logger 方法签名：`trace/debug/info/warn/error(msg: string, data?: unknown): void`
- 环境检测：`isTauri`（`'__TAURI__' in window`）、`isDev`（`import.meta.env.DEV`）
- 默认日志级别：开发环境 DEBUG，生产环境 WARN

**双通道输出策略**：所有日志始终输出到 console（保留开发者工具可观测性）；Tauri 环境额外 fire-and-forget 到 `@tauri-apps/plugin-log`（持久化到 `webview.log`），`.catch(() => {})` 防止日志系统自身引发错误。

**Error 格式化**：`formatError(error)` 处理 WebKit stack 缺失 message 的情况——若 `error.stack` 不包含 `error.message`，手动拼接 `${name}: ${message}\n${stack}`。

**data 格式化**：`formatData(data)` 将 Error 对象从普通数据中分离——Error 走 `formatError` 输出 stack，其余走 `JSON.stringify`。避免 `JSON.stringify(errorInstance)` 返回 `{}` 丢失 stack。

#### 4.2.2 console 劫持（Step 1b）

新增 `bootstrapLogger()` 函数，在 `main.tsx` 最早期调用。劫持 `console.error/warn/info/debug/trace/log` 六个方法，每个调用先通过 Tauri 日志插件持久化，再调用原始 console 方法。

**关键实现细节**（对照 OpenBitFun 实测经验）：

- **多参数逐个格式化**：`formatConsoleArgs(args)` = `args.map(formatConsoleArg).join(' ')`，确保 `console.error("msg", errorObj)` 中 Error 的 stack 不被 `JSON.stringify` 吞成 `{}`
- **分级过滤**：`consoleForwardMinLevel` 控制哪些级别的 console 调用才转发到 Tauri。生产环境默认 WARN，避免 TRACE/INFO 洪水
- **幂等安装**：通过 `window.__assistant_console_forward_installed__` 标志防止重复安装
- **非 Tauri 环境跳过**：`if (!isTauri) return`

#### 4.2.3 迁移策略

**Phase A**（Step 1 完成后）：安装 console 劫持，**零改动**即可实现生产日志持久化。

**Phase B**（渐进）：逐个将 `console.error("xxx failed:", e)` 替换为 `log.error("xxx failed", e)`，获得分级控制 + 上下文标签的好处。按文件优先级迁移：
1. `chatStore.ts`（16 处，最高频）
2. `HistoryManager.tsx`（10 处，次高频）
3. `chatOrchestrator.ts`（2 处，关键路径）
4. `settingsStore.ts`（3 处）
5. 其余文件按出现次数降序

**Phase B 批量迁移建议**：有了 console 劫持兜底后，Phase B 可采用更激进的策略——`console.error("[xxx]", e)` → `log.error("xxx", e)` 的模式高度机械化，可用 AST 工具（如 jscodeshift）一次性完成 50+ 处替换。替换后若出问题，console 劫持仍兜底，不会丢失日志。每个文件替换后立即运行该文件的测试，确保无回归。

### 4.3 Step 2：全局错误处理器

**目标**：在 `main.tsx` 注册 `window.error` + `unhandledrejection` 监听，捕获所有未处理异常。

#### 4.3.1 新增 `src/services/reactProductionError.ts`

参照 OpenBitFun 实现，解码 `Minified React error #xxx`。核心接口：

- `parseMinifiedReactError(message)` → `{ code, decoderUrl, hint? }` 或 `null`
- `isMinifiedReactErrorMessage(message)` → boolean
- `buildReactCrashLogPayload(error, errorInfo?)` → 合并 error 基本信息 + componentStack + React 诊断信息的可序列化对象
- `safeReactErrorInfo(info)` → 仅提取 `componentStack` 字符串，忽略非序列化字段

`REACT_HINTS` 覆盖最常见错误码：300（hooks 数量不匹配）、301（无限重渲染）、310（条件 hooks）。

#### 4.3.2 新增 `src/services/globalErrorHandler.ts`

参照 OpenBitFun `main.tsx` 中 `registerGlobalErrorHandlers()` 实现。核心逻辑：

**`window.error` 处理**：

1. **ErrorEvent 分支**：先检测 React 压缩错误（`isMinifiedReactErrorMessage`），匹配则立即 `log.error` 持久化（不等延迟调度）。然后通过 `scheduleCrashLog()` 延迟记录通用错误信息
2. **延迟白屏检测**：`scheduleCrashLog()` 通过 `queueMicrotask → requestAnimationFrame → requestAnimationFrame` 延迟两帧后调用 `isRootEmpty()`，避免 React 尚未完成渲染时误判白屏
3. **白屏去重**：`WHITE_SCREEN_LOGGED_FLAG` 确保白屏崩溃只记录一次，但非白屏错误不受此限制
4. **资源加载错误**：`event` 非 `ErrorEvent` 时为资源加载错误，提取 `target.tagName/src/href`，仅当白屏时才记录（减少噪音）

**`unhandledrejection` 处理**：

1. 同样检测 React 压缩错误并立即持久化
2. `serializeError(reason)` 将 Error 对象转为 `{ name, message, stack }`，非 Error 转为 `{ value: String(reason) }`

**注册方式**：`addEventListener('error', ..., true)` 使用捕获阶段，确保最先处理。通过 `window.__assistant_global_error_handlers_registered__` 防止重复注册。

**与 ErrorBoundary 的去重**：全局处理器和 ErrorBoundary 会各自独立记录同一错误——这是有意为之。全局处理器记录原始错误 + 白屏状态，ErrorBoundary 记录 componentStack，两者信息互补。OpenBitFun 也采用此策略。

#### 4.3.3 集成到 `main.tsx`

在 `bootstrap()` 之前调用 `bootstrapLogger()` + `registerGlobalErrorHandlers()`，确保启动阶段的 console 输出和错误也被捕获。

### 4.4 Step 3：App 级 ErrorBoundary

**目标**：包裹 `<App/>`，React 渲染异常时提供全屏兜底 UI + Reload 按钮。

#### 4.4.1 新增 `src/components/AppErrorBoundary.tsx`

**关键设计决策：兜底 UI 零外部依赖**。不依赖 antd，也不依赖 i18n。原因：若 antd 或 i18n bundle 本身加载/渲染出错（chunk 加载失败、CSS-in-JS 注入失败），`import { Button, Result } from 'antd'` 或 `translateKey()` 会失败，ErrorBoundary 自身崩溃反而加剧白屏。OpenBitFun 的 `AppErrorBoundary` 也使用纯 HTML/CSS + 自有轻量 Button，不依赖重 UI 库。兜底 UI 中的文案（"应用遇到了问题"/"重新加载"/"技术详情"）硬编码中文，与不依赖 antd 同理——这是白屏最后防线，零外部依赖优先于国际化。

兜底 UI 使用纯 HTML 元素 + inline styles 实现：
- 全屏居中布局（`height: 100vh; display: flex; align-items: center; justify-content: center`）
- 标题 + 错误首行（`error.message.split('\n')[0]`）
- Reload 按钮（`window.location.reload()`）
- 开发模式额外展示 `<details>` 折叠区：error.stack + componentStack

`componentDidCatch` 中调用 `log.error('[CRASH] React error boundary caught exception', buildReactCrashLogPayload(error, errorInfo))` 持久化。

**React 19 注意事项**：项目使用 `react: ^19.1.0`，ErrorBoundary 的 Class Component API 在 React 19 中保持兼容。需在 StrictMode 下验证：React 19 StrictMode 会重放 mount/unmount，但 `getDerivedStateFromError` + `componentDidCatch` 的行为与 React 18 一致。验收时需额外测试 StrictMode 场景，具体用例：
- StrictMode 下触发渲染错误，确认 `getDerivedStateFromError` 只调用一次（StrictMode 不重放 error 生命周期）
- `componentDidCatch` 中 `log.error` 不重复记录（StrictMode 下 `componentDidCatch` 可能调用两次，需去重或接受——OpenBitFun 选择接受，因为两次调用的 componentStack 可能不同）
- AppErrorBoundary 的 `setState({ hasError: true })` 在 StrictMode 下不触发无限循环

#### 4.4.2 集成到 `main.tsx`

在 `React.StrictMode` 内、`ThemedApp` 外层包裹 `AppErrorBoundary`。

### 4.5 Step 4：Feature 级 ErrorBoundary

**目标**：关键功能区域独立容错，单个功能崩溃不影响其他区域。

#### 4.5.1 新增 `src/components/FeatureErrorBoundary.tsx`

Props：`children` + `featureName` + 可选 `onError` 回调 + 可选 `fallback` render prop。

**业务错误与渲染错误区分**：FeatureErrorBoundary 只应捕获**渲染错误**（组件 crash），不应拦截业务错误（如 `ChatError`）。在 `getDerivedStateFromError` 中需判断：若 error 是 `ChatError` 实例（通过 `isChatError(error)` 检测），则不设置 `hasError`，让错误继续由 `ErrorBlock` / `RetryBanner` 处理；仅对非 ChatError 的渲染异常设置 `hasError: true`。这避免 FeatureErrorBoundary 吞没 Chat 流式错误的用户感知逻辑。

兜底 UI 使用 antd `Result` 组件（Feature 级可以依赖 antd，因为 App 级 ErrorBoundary 已兜底 antd 加载失败的情况）居中展示，包含：
- 错误标题（`common.errors.feature.crashed`，插值 `feature`）
- 错误描述（`error.message` 首行）
- 重试按钮（`common.errors.feature.retry`）

**i18n 策略**：FeatureErrorBoundary 是 class 组件，无法使用 `useTranslation()` hook，使用 `translateKey()` 直接调用 `i18n.t()`。i18n 失败时错误会冒泡至 AppErrorBoundary 兜底，不会白屏，因此可以安全接入 i18n。

支持 `fullPage` prop 控制布局：默认行内展示（`Alert`），`fullPage=true` 时全屏居中展示（`Result`），适配 ChatView 等全屏区域。

`componentDidCatch` 中调用 `log.error` 持久化，并调用 `onError?.(error, errorInfo)` 通知调用方。

#### 4.5.2 使用位置

- `ChatView.tsx`：包裹 MessageList + MessageInput（`fullPage`）
- `App.tsx` History 区域：包裹 `HistoryManager`
- `App.tsx` Settings 区域：包裹 Settings 路由内容

### 4.6 Step 5：`lazyWithRecovery` 懒加载容错

**目标**：代码分割加载失败时，重试创建新 `React.lazy()` 实例，而非复用已失败的 chunk。

#### 4.6.1 新增 `src/utils/lazyWithRecovery.tsx` + `src/utils/moduleLoader.ts`

参照 OpenBitFun 的 `lazyWithRecovery.tsx` + `moduleLoader.ts` 实现。

**`moduleLoader.ts` 职责**：

- `isModuleLoadError(error)`：识别模块加载错误（`Failed to fetch dynamically imported module` / `importing a module script failed` 等），区分于执行错误和语法错误
- `importWithRetry(load)`：对模块加载错误自动重试 2 次（间隔 250ms / 750ms），非模块错误或重试耗尽则抛出
- `createModuleLoader(load)`：缓存成功模块的 Promise，**永远不缓存 rejected promise**（失败后 `pending = undefined` 允许后续重试）
- `subscribeToModuleRecovery(retry)` / `retryFailedModuleLoads()`：发布-订阅模式，Vite HMR 事件触发所有失败模块重试

**`lazyWithRecovery.tsx` 职责**：

- `ModuleImportError` 类区分 import 错误和 render 错误——`getDerivedStateFromError` 中非 `ModuleImportError` 继续冒泡
- `ModuleBoundary` 组件：捕获 import 错误后展示重试 UI（重试按钮 + Reload 按钮），`componentDidCatch` 中若当前 lazy 实例仍是失败的则创建新实例
- `retry()` 重置 state 并使用模块级最新 `Content`
- 开发模式下额外展示 `<details>` 折叠区显示错误详情
- `componentDidMount` / `componentWillUnmount` 管理 recovery 订阅生命周期

**i18n 策略**：`ModuleBoundary` 使用 `translateKey()` 而非 `useTranslation()` hook。模块加载失败时 i18n 可能尚未初始化，但 `translateKey()` 内部直接读 `i18n.t()`，若 i18n 未初始化则返回 key 本身而非抛异常，可安全使用。文案键：`common.errors.moduleLoad.title`、`common.errors.moduleLoad.retry`，Reload 按钮复用 `common.errors.boundary.reload`，技术详情复用 `common.errors.boundary.technicalDetails`。

返回类型 `RecoverableLazyComponent<T>` = `ForwardRefExoticComponent` + `{ preload }`，保留 ref 转发和预加载能力。

#### 4.6.2 使用位置

替换项目中所有 `React.lazy` 用法：

1. **`tools/registry.ts`**：工具页懒加载（当前已有 `LazyErrorBoundary` 包裹，替换为 `lazyWithRecovery` 后可移除 `LazyErrorBoundary`）
2. **`settings/core/pageRegistry.ts`**：设置页懒加载（当前**无 ErrorBoundary**，这是 §2.2 中识别的 P1 缺失）

### 4.7 Step 6：`ChatError` 增强 + `compactError` 结构化

**目标**：增加 `details` 字段，让 `toChatError()` 携带更多诊断上下文；将 `compactError` 从 `string | null` 升级为结构化 `ChatError`。

#### 4.7.1 `ChatError` 接口变更

在 `chat/core/chatErrors.ts` 的 `ChatError` 接口新增 `details?: Record<string, unknown>` 字段。向后兼容，新字段可选。

`toChatError(value, extra?)` 增强第二个参数：合并 `value.details` 和 `extra` 到 `details` 字段。

#### 4.7.2 `tauriInvoke` 集成

`tauriInvoke.ts` 中 `console.error` 替换为 `log.error`，携带 `{ command, argsKeys: Object.keys(args) }` 作为诊断上下文。

`chatOrchestrator.ts` 的 `send()` catch 中调用 `toChatError(e, { command: 'send_chat_message', conversationId: convId })` 附加命令上下文。

#### 4.7.3 `compactError` 结构化

将 `chatStore.ts` 中 `compactError: string | null` 改为 `compactError: ChatError | null`。`CompactErrorBanner.tsx` 可复用 `ErrorBlock` + `RetryBanner` 组件替代当前手写 UI，获得统一的错误展示 + 恢复动作。`clearCompactError()` 逻辑不变。

**迁移前置**：`compactError` 在 `chatStore.ts` 中被多处读写，实施 Step 6b 前需先 grep 全量读写点（`compactError` / `setCompactError` / `clearCompactError`），确保不遗漏任何消费方。风险从"中"上调至"中高"，因为涉及 Store 接口变更 + UI 组件重写双重改动。

### 4.8 Step 7：Store 级错误通知

**目标**：Store 中 `console.error` 迁移到 `log.error` 后，关键操作失败通过通知系统推送给用户。

#### 4.8.1 错误通知工具

新增 `src/services/errorNotifier.ts`。

**关键设计决策：复用 App.tsx 已有的 `notification.useNotification()` 实例**。当前 `App.tsx` 已使用 `const [api, contextHolder] = notification.useNotification()`，这是 antd v6 推荐方式，可控制 placement、maxCount 等全局配置。若 `errorNotifier` 直接调用 `notification.error()` 静态方法，会绕过 `contextHolder` 的全局配置，且可能无法获得 ConfigProvider 的主题。

**确定方案：Zustand 队列桥接**。选择理由：
- Store 中的错误处理函数是同步调用的，无法直接持有 React hook 返回的 `api` 实例
- 注入方案（`errorNotifier` 接受 `notification` 实例作为参数，由 `App.tsx` 注入）要求在 App mount 后手动调用 `initErrorNotifier(api)`，时序脆弱，若 Store 在 App 渲染前就抛错则通知丢失
- 队列方案天然解耦：Store push 错误 → React 组件消费队列 → 调用 `api.error()`，两端独立

实现方式：`useErrorNotifyStore`（Zustand）维护 `pending: Array<{ titleKey, descriptionKey, options }>` 队列。`notifyError(titleKey, descriptionKey, options?)` 将条目 push 到队列并同步调用 `log.warn` 记录通知事件。`App.tsx` 中在 `contextHolder` 旁新增 `<ErrorNotifyConsumer>` 组件，`useEffect` 订阅队列变化，逐条调用 `api.error()` 后从队列移除。

#### 4.8.2 需要用户通知的关键操作

| Store | 操作 | 通知键 |
|-------|------|--------|
| `chatStore` | `renameConversation` | `chat.errors.renameFailed` |
| `chatStore` | `updateModel` | `chat.errors.updateModelFailed` |
| `chatStore` | `deleteMessage` | `chat.errors.deleteMessageFailed` |
| `chatStore` | `switchBranch` | `chat.errors.switchBranchFailed` |
| `chatStore` | `manualCompact` | `chat.errors.compactFailed` |
| `HistoryManager` | `deleteConversation` | `history.errors.deleteConversationFailed` |
| `HistoryManager` | `renameConversation` | `history.errors.renameFailed` |
| `HistoryManager` | `exportConversation` | `history.errors.exportFailed` |

其余操作（`loadConversations`、`loadMessages` 等数据加载）仅 `log.error`，不弹通知——加载失败用户可通过 UI 空状态感知。

### 4.9 i18n 键值

**分层原则**：`common.errors` 仅放置**跨组件通用**的错误文案（boundary / feature / moduleLoad），与 `common.actions`、`common.theme` 等通用 UI 文案平级。**组件专属错误**放入各自命名空间的 `errors` 子对象（如 `chat.errors`、`history.errors`），保持 `common` 不膨胀。

**AppErrorBoundary 不接入 i18n**（§4.4 零外部依赖原则），其文案硬编码。`common.errors.boundary.*` 键供 `ModuleBoundary` 的"重新加载"/"技术详情"按钮复用。

需在 `zh-CN.json` 和 `en-US.json` 中新增以下键值：

```json
{
  "common": {
    "errors": {
      "boundary": {
        "title": "应用遇到了问题",
        "reload": "重新加载",
        "technicalDetails": "技术详情",
        "unknown": "未知错误"
      },
      "feature": {
        "crashed": "{{feature}} 遇到了问题",
        "retry": "重试"
      },
      "moduleLoad": {
        "title": "模块加载失败",
        "retry": "重试"
      }
    }
  },
  "chat": {
    "errors": {
      "renameFailed": { "title": "重命名失败", "description": "对话重命名未成功，请稍后重试" },
      "updateModelFailed": { "title": "切换模型失败", "description": "模型切换未成功，请稍后重试" },
      "deleteMessageFailed": { "title": "删除消息失败", "description": "消息删除未成功，请稍后重试" },
      "switchBranchFailed": { "title": "切换分支失败", "description": "分支切换未成功，请稍后重试" },
      "compactFailed": { "title": "压缩失败", "description": "对话压缩未成功，请稍后重试" }
    }
  },
  "history": {
    "errors": {
      "deleteConversationFailed": { "title": "删除失败", "description": "对话删除未成功，请稍后重试" },
      "renameFailed": { "title": "重命名失败", "description": "对话重命名未成功，请稍后重试" },
      "exportFailed": { "title": "导出失败", "description": "对话导出未成功，请稍后重试" }
    }
  }
}
```

**各组件 i18n 接入策略**：

| 组件 | 接入 i18n | 调用方式 | 原因 |
|------|----------|---------|------|
| `AppErrorBoundary` | ❌ 不接入 | 硬编码中文 | 零外部依赖原则，白屏最后防线 |
| `FeatureErrorBoundary` | ✅ 接入 | `translateKey()` | class 组件无法用 hook；i18n 失败冒泡至 AppErrorBoundary 兜底 |
| `ModuleBoundary` | ✅ 接入 | `translateKey()` | 懒加载时 i18n 可能未初始化，`translateKey()` 返回 key 本身不抛异常 |

## 5. 实施计划

> **策略**：四步到位，每步内多个子项并行开发、一次提交。不再拆 15 个微 Step，按**基础设施 → 防御层 → 错误增强 → 全量迁移**四阶段推进。

| Step | 内容 | 涉及文件 | 风险 |
|------|------|---------|------|
| **1** ✅ | **基础设施层**：安装 `@tauri-apps/plugin-log` + `logger.ts`（createLogger + bootstrapLogger + console 劫持）+ `reactProductionError.ts` + `globalErrorHandler.ts` + `AppErrorBoundary.tsx` + `main.tsx` 集成 | `package.json`, `src/services/logger.ts` (新), `src/services/reactProductionError.ts` (新), `src/services/globalErrorHandler.ts` (新), `src/components/AppErrorBoundary.tsx` (新), `src/main.tsx` | 低：全部新增文件 + main.tsx 最早期注入，不触碰现有业务逻辑 |
| **2** | **防御层**：`FeatureErrorBoundary.tsx`（区分 ChatError/渲染错误）+ `moduleLoader.ts` + `lazyWithRecovery.tsx` + 替换 `tools/registry.ts` + `settings/core/pageRegistry.ts` 中全部 `React.lazy` | `src/components/FeatureErrorBoundary.tsx` (新), `src/utils/moduleLoader.ts` (新), `src/utils/lazyWithRecovery.tsx` (新), `src/tools/registry.ts`, `src/settings/core/pageRegistry.ts`, `src/chat/ChatView.tsx`, `src/App.tsx` | 中：FeatureErrorBoundary 需区分 ChatError；React.lazy 替换影响 27 处 |
| **3** | **错误增强层**：`ChatError.details` 字段 + `compactError` 结构化 + `errorNotifier.ts`（Zustand 队列）+ i18n 键值 | `src/chat/core/chatErrors.ts`, `src/stores/chatStore.ts`, `src/chat/CompactErrorBanner.tsx`, `src/services/errorNotifier.ts` (新), `src/App.tsx`, `zh-CN.json`, `en-US.json` | 中高：compactError 涉 Store 接口变更 + UI 重写；需先 grep 全量读写点 |
| **4** | **全量迁移层**：56 处 `console.error` → `log.error` 批量替换 + 8 处 `.catch(() => {})` 评估迁移 + chatStore 关键操作接入 `notifyError` | 17 个文件（见 §2.4） | 低：console 劫持已兜底，批量替换可 jscodeshift 自动化 |

## 6. 验收标准

| # | 标准 | 验证方式 |
|---|------|---------|
| V1 | React 渲染异常不再白屏，显示 AppErrorBoundary 兜底 UI（纯 HTML，不依赖 antd） | 手动在 render 中 throw Error |
| V2 | React 19 StrictMode 下 AppErrorBoundary 正常工作，不出现二次捕获或状态丢失 | StrictMode 下触发渲染错误；验证 `getDerivedStateFromError` 仅调用一次；`componentDidCatch` 中 `log.error` 可接受两次调用（componentStack 可能不同）；`setState` 不触发无限循环 |
| V3 | `window.error` 和 `unhandledrejection` 被捕获并记录到 Tauri 日志 | 手动 `setTimeout(() => { throw new Error('test'); }, 0)` |
| V4 | 白屏检测延迟两帧执行，不误判 React 渲染中的状态 | 在 React 渲染周期内触发错误 |
| V5 | 生产环境 `console.error` 输出可在 `webview.log` 中找到 | 打包后触发错误，检查日志文件 |
| V6 | `Minified React error #301` 被解码为 `https://react.dev/errors/301` + hint | 生产构建中触发 React hooks 错误 |
| V7 | 工具页和设置页代码分割加载失败均显示重试 UI，重试后成功加载 | DevTools Network 中 Block 某个 chunk |
| V8 | `renameConversation` / `updateModel` 等关键操作失败时用户看到通知 | Mock 后端返回错误 |
| V9 | 通知使用 `notification.useNotification()` 实例，不使用静态 API | 检查 errorNotifier 实现 |
| V10 | `ChatError.details` 携带 Tauri command 上下文 | 检查 `toChatError(e, { command: '...' })` 输出 |
| V11 | `compactError` 为结构化 `ChatError`，CompactErrorBanner 复用 ErrorBlock | 触发压缩错误 |
| V12 | 所有 `.catch(() => {})` 已评估，cancelChat / updateProvider 失败记录 `log.warn` | 代码审查 |
| V13 | i18n 键值完整，zh-CN / en-US 均有对应翻译 | 构建无 missing key 警告 |
| V14 | 每个 Step 完成后 `console.error` 剩余数量可验证 | `grep -r "console\.error" src/ --include="*.ts" --include="*.tsx" -c` 逐 Step 递减，最终仅剩 logger.ts 内部实现 |
| V15 | FeatureErrorBoundary 不吞没 ChatError 业务错误 | ChatView 内触发 ChatError 实例，确认 ErrorBlock 正常展示，FeatureErrorBoundary 不拦截 |

## 7. 与现有体系的关系

| 现有体系 | I19 变更 | 兼容性 |
|---------|---------|--------|
| `chatErrors.ts` + `chatErrorRecovery.ts` | 增加 `details` 字段 | ✅ 向后兼容，新字段可选 |
| `ErrorBlock.tsx` + `RetryBanner.tsx` | 无变更 | ✅ 不受影响 |
| `CompactErrorBanner.tsx` | `compactError` 从 `string` 升级为 `ChatError`，复用 ErrorBlock | 🔄 行为增强，UI 升级；需先 grep 全量读写点 |
| `LazyErrorBoundary` (ToolPage) | 被 `lazyWithRecovery` 替代 | 🔄 替换，行为增强 |
| `pageRegistry.ts` 中 `React.lazy` | 被 `lazyWithRecovery` 替代，新增错误边界 | 🔄 新增容错，原无 ErrorBoundary |
| `tauriInvoke.ts` | `console.error` → `log.error` | ✅ 行为不变，增加持久化 |
| `App.tsx` notification | errorNotifier 通过 Zustand 队列桥接复用已有 `useNotification()` 实例 | ✅ 共享全局配置；新增 `<ErrorNotifyConsumer>` 消费组件 |
| Rust `crash_dump.rs` | 无变更 | ✅ 不受影响 |
| Rust `ProviderError` + `ProviderErrorPayload` | 无变更（已有 `http_status` + `provider` + `provider_code`）；`toChatError()` 通过 `details` 字段桥接 Rust 错误结构 | ✅ 不受影响 |

### 7.1 Rust → 前端错误桥接

`tauriInvoke` 抛出的错误来自 Rust 序列化的 `ProviderError` / `ProviderErrorPayload`（含 `http_status` + `provider` + `provider_code`）。`toChatError()` 的 fallback 逻辑需处理此结构：

- 若错误对象含 `http_status` 字段，映射到 `ChatError.httpStatus`
- 若含 `provider` / `provider_code`，映射到 `ChatError.provider` / `ChatError.providerCode`
- 其余 Rust 特有字段（如 `message`、错误类型名）放入 `ChatError.details`

这确保 Rust 端结构化错误信息不丢失，同时前端 `ErrorBlock` 可正常展示。
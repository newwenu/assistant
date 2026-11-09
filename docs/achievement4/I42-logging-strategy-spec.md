# I42 — 日志策略规格（`tauri-plugin-log` 配置 + 高频日志治理）

| 项 | 值 |
|---|---|
| 状态 | 现行（2026-11-09 已落地：S1–S6 全部完成） |
| 归属 | 基础设施；横跨 `assistant-app`、`assistant-providers`、前端 `services/logger.ts` |
| 关联 | [ARCHITECTURE.md](../ARCHITECTURE.md) §5 日志说明、[error-swallowing-analysis](../analysis/error-swallowing-analysis.md) §1.2 S2 / §4「日志级别过低」/ §2 G10 |
| 参考调研 | 13 个参考项目全量比对，结论见 §2.3；配置代码可直接对照的仅 `references/1-ai-agent/OpenBitFun/src/apps/desktop/src/logging.rs`（同栈 Tauri v2 + `tauri-plugin-log`） |
| 影响面 | `src-tauri/src/plugins.rs`、`src-tauri/src/adapters/event_sink.rs`、`src-tauri/crates/assistant-providers/src/{trace.rs,gemini/request.rs,openai/chat.rs,anthropic/request.rs}`、`src/services/logger.ts`、`src/services/stream/transport.ts`、`docs/ARCHITECTURE.md` |
| 依赖 | `tauri-plugin-log 2.10.1`（现依赖，无需升级） |

## 0 定位

起因是一条真实日志：

```
[2026-11-09][10:37:54][assistant_app::adapters::event_sink][DEBUG] chat-delta turn=... seq=725 iteration=1
```

它直接打到控制台，且**每个流式增量一行**。本文只解决「日志写到哪里、写多细、怎么轮转、哪些不该写」，不改业务逻辑、不改事件协议、不改崩溃转储机制。

三条约束贯穿全文：

1. **单一配置点**：后端日志行为只在 `src-tauri/src/plugins.rs` 一处定义，其它模块只调用 `log::*`，不各自初始化 logger。
2. **默认档位可排障**：开发档看得到细节，生产档只留可行动信息，两者文件互不覆盖。
3. **高频路径零日志**：任何「每个 delta / 每个 SSE chunk / 每次渲染」触发的日志默认不写；确需时降到 `trace`，由 `log` 全局级别门控（默认档为 `Debug`/`Info`，天然过滤）。

## 1 现状与问题

### 1.1 当前配置

`src-tauri/src/plugins.rs` 第 8 行：

```rust
.plugin(tauri_plugin_log::Builder::default().build())
```

未设置级别、未设置目标、未设置滚动、未设置时区，全部走插件默认值。

### 1.2 默认值实测（`tauri-plugin-log 2.10.1` 源码）

| 配置项 | 默认值 | 出处 |
|---|---|---|
| 全局级别 | `LevelFilter::Trace` | `Builder::new()` 文档 |
| 目标 | `[Stdout, LogDir { file_name: None }]` | `DEFAULT_LOG_TARGETS` |
| 单文件上限 | `40_000` 字节 | `DEFAULT_MAX_FILE_SIZE` |
| 滚动策略 | `RotationStrategy::KeepOne` | `DEFAULT_ROTATION_STRATEGY` |
| 时区 | `TimezoneStrategy::UseUtc` | `DEFAULT_TIMEZONE_STRATEGY` |
| 打开策略 | `FileOpenStrategy::Append` | `DEFAULT_FILE_OPEN_STRATEGY` |

`KeepOne` 的语义是**超限即删掉旧文件**（`RotatingFile::rotate` 的 `KeepOne` 分支直接 `fs::remove_file`），不是保留一份备份。因此默认配置的实际效果是：**日志只有一个 40KB 的环形窗口，写满就整体丢弃**。

### 1.3 噪音来源清单

| 来源 | 位置 | 频次 | 级别 | 状态 |
|---|---|---|---|---|
| 流式增量 | `event_sink.rs` `TurnEvent::Delta` 分支 `chat-delta` | 每 delta 一行 | DEBUG | **已删除** |
| 前端 coalescer 投递 | `services/stream/transport.ts` 第 159-161 行 `[coalescer-deliver]` | 每帧一行 | `console.debug`，开发档被 `bootstrapLogger` 转发到后端**再落一次** | 已处理（S4） |
| SSE 逐 chunk | `assistant-providers/src/trace.rs` `log_response_chunk`（唯一调用点 `gemini/request.rs`） | 每 chunk 一行 | DEBUG | 已处理（S3） |
| SSE 预览 / 非标准事件 | `openai/chat.rs`、`anthropic/request.rs` 各 2 处 | 每 chunk / 每事件 | DEBUG | 已处理（S3） |
| 依赖库 | `hyper_util`、`h2`、`rustls`、`ignore`、`globset`、`grep_searcher`、`tao`、`wry` | 由全局 `Trace` 放行 | TRACE/DEBUG | 已处理（S5） |

`DiagnosticLevel`（`assistant-providers/src/diagnostics.rs`）是**死代码**：三个 `should_log_*` 与 `set()` 在业务代码零调用，仅自身单测引用。故不能用它关噪音，只能靠 `log` 全局级别。

### 1.4 后果量化

`chat-delta` 一行约 90 字节，40KB ≈ 440 行；实测单轮 `seq` 已到 725，即**一轮对话覆盖日志文件约 1.6 次**。后果：关键错误被挤出窗口、控制台双写刷屏、时间戳为 UTC、`KeepOne` 无历史可比。

## 2 目标与非目标

### 2.1 目标

| # | 交付 |
|---|---|
| 1 | 开发 / 生产双档 profile（级别、目标、文件名） |
| 2 | 文件滚动改为 `KeepSome(3)` + 10MB + 本地时区 + 每次启动轮转 |
| 3 | 移除逐 delta 日志，改为 turn 级起止汇总 |
| 4 | 逐 chunk 日志降为 `trace`（由 `log` 全局级别门控，不引入 `DiagnosticLevel`） |
| 5 | 前端转发级别收敛，消除 dev 双写 |
| 6 | 依赖库噪音压制清单（`level_for`） |
| 7 | 敏感信息规则与崩溃转储边界说明 |
| 8 | 日志字段截断规则（含修复 `&data[..200]` 的 UTF-8 边界 panic） |
| 9 | 目录总量第二轴上界与验收 |

### 2.2 非目标

- 不引入会话目录（`logs/20261109T103754/`）与旧会话清理任务。参考项目 OpenBitFun 有该能力，但当前单进程单实例、无 E2E 多会话需求，收益不足。
- 不按业务域拆分多文件（`app.log` / `ai.log` / `webview.log`）。单文件 + `target` 字段已足够过滤，多文件会让「一次排障要看四个文件」。
- 不做运行期 UI 调级面板（§3.10 仅保留能力位，不接线到设置页）。
- 不改 `assistant-crash`（panic / 系统崩溃转储独立于日志体系，见 §3.9）。
- 不改任何 `log::*` 的**语义**（消息内容、字段），只改级别与去留。

### 2.3 参考项目调研结论

13 个项目逐个查过源码。**只有 OpenBitFun 与本项目同栈**（Tauri v2 + `tauri-plugin-log`），配置可直接对照；其余用于校验「哪些决策值得抄」。

| 项目 | 可借鉴点 |
|---|---|
| OpenBitFun | 全局 `Trace` + `level_for` 白名单；`KeepSome(2)` + 10MB + `UseLocal`；4 文件按 target 分流 + 会话目录（不采纳，见 §2.2） |
| BongoCat | 字段截断上限；保留双轴（天数 + 总量），「either alone lets one axis grow without limit」 |
| vaultwarden | 噪音库级别随全局浮动；敏感调试开关开启时先打 `WARN` |
| 其余 10 个 | 无可直接采纳项：无落盘（memos / deepseek-harness-orb）、无集中初始化（Cap 的 `tracing!` 是 no-op）、默认 `WARN` 会丢启动信息（goose）、单备份等于无历史（Kiri）、硬编码级别不便排障（WindowPet） |

三条补充已分别落到 §3.3（噪音联动）、§3.4.1（双轴）、§3.8.1（截断）。

## 3 规格

### 3.1 双档 profile

用 `cfg!(debug_assertions)` 区分，**不读配置文件**（日志配置错误会导致排障链本身失效，必须是编译期常量）。

| 项 | 开发档（`debug_assertions`） | 生产档 |
|---|---|---|
| 全局级别 | `Debug` | `Info` |
| Stdout 目标 | 启用 | 不启用 |
| 文件目标 | `LogDir { file_name: "assistant-dev" }` | `LogDir { file_name: "assistant" }` |
| 前端 `currentLevel` | `DEBUG`（现状保持） | `WARN`（现状保持） |
| 前端 `consoleForwardMinLevel` | `INFO` | `INFO` |

文件名分离的理由：开发期刷屏日志不能占用生产档的保留位。两档各自滚动，互不覆盖。

生产档不启用 Stdout 的理由：`main.rs` 首行 `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`，release 下 stdout 句柄不可见，`Target::Stdout` 只是空写。

### 3.2 目标（targets）

```
生产：Target::new(TargetKind::LogDir { file_name: Some("assistant".into()) })
开发：Target::new(TargetKind::Stdout)
    + Target::new(TargetKind::LogDir { file_name: Some("assistant-dev".into()) })
```

不使用 `Target::new(TargetKind::Webview)`：本项目前端不订阅 `log://log` 事件（`attachConsole` 未被调用），启用该目标只会产生无人消费的事件。前端日志走 `@tauri-apps/plugin-log` 的 `log` 命令，其 target 固定为 `webview::{location}`（`tauri-plugin-log/src/commands.rs` 第 17-22 行），与后端日志写入同一文件——这是刻意的：一次排障只需一个文件，按 `webview` 前缀即可把前端行分离出来。

`LogDir` 解析到系统日志目录（Windows：`%LOCALAPPDATA%\com.assistant\logs`；macOS：`~/Library/Logs/com.assistant`；Linux：`$XDG_DATA_HOME/com.assistant/logs`），最终文件名带 `.log` 扩展。

### 3.3 级别策略

全局级别只解决「默认放行到哪」，依赖库噪音必须用 `level_for` 逐条压制——`level_for` 按「最具体优先」匹配（`hyper::http::h1` → `hyper::http` → `hyper` → 全局），因此只需写库名前缀。

**确定项**（与参考项目 `logging.rs` 的压制清单一致，且均在本项目依赖树中）：

| target | 级别 | 理由 |
|---|---|---|
| `ignore` / `ignore::walk` | `Off` | 目录遍历逐条输出，`grep` / `glob` 工具高频路径 |
| `globset` | `Off` | 匹配器逐条输出 |
| `grep_searcher` | `Warn` | 逐文件输出，保留告警 |
| `hyper_util` / `h2` | `Info` | 连接池 / HTTP/2 帧级 |
| `reqwest` | `Info` | `starting new connection` 等连接日志，URL 已在业务日志中 |
| `tao` / `wry` | `Warn` | 窗口事件循环帧级（`NewEvents emitted without explicit RedrawEventsCleared`），对业务排障无用 |

`ignore` / `globset` / `grep_searcher` 是 `assistant-tools` 的直接依赖，属必需项而非预防性配置。`reqwest` / `tao` / `wry` 由 A7 实测确定：2 轮对话内 `tao::platform_impl` 占 12 条 DEBUG，为 DEBUG 最大来源；`reqwest::connect` 2 条。

**未压制项**（实测无噪音，不为「可能吵」预埋）：`rustls`、`pdf-extract`、`image`、`rusqlite`、`keyring_core`、`windows`、`zxcvbn`。原则是写多了会掩盖真实告警，且库升级改名后静默失效；出现噪音时按 A7 方法（`Group-Object` 统计）追加。

**联动性质**：`level_for` 只在**比全局更严**时生效。`hyper_util` / `h2` / `reqwest` 设 `Info` 在 `Info` 档等于没设、只在 `Debug` 档起作用。`Off` 三条在两档都应为零输出，故不随全局浮动。

**本项目自有 target 不压制**，只改写法（§3.6）：`assistant_app`、`assistant_agent`、`assistant_providers`、`assistant_tools`、`assistant_vault`、`assistant_storage`、`assistant_sync`、`assistant_security`、`assistant_config`、`webview`。

### 3.4 滚动与体积

| 配置 | 值 | 说明 |
|---|---|---|
| `max_file_size` | `10 * 1024 * 1024` | 单文件 10MB |
| `rotation_strategy` | `RotationStrategy::KeepSome(3)` | 1 个活动文件 + 3 个历史归档 |
| `timezone_strategy` | `TimezoneStrategy::UseLocal` | 时间戳与归档文件名用本地时区 |
| `file_open_strategy` | `FileOpenStrategy::Rotate` | 每次启动把上一份归档，便于「本次 vs 上次」对比 |

归档命名为 `{file_name}_{YYYY-MM-DD_HH-MM-SS}.log`（`LOG_DATE_FORMAT`）。`KeepSome` 会在写入前 `remove_old_files(keep_count - 1)` 再归档，保证归档数不超过 3。

### 3.4.1 两条轴上界

| 轴 | 上界 | 来源 |
|---|---|---|
| 单文件 | 10MB | `max_file_size` |
| 文件数 | 4（1 活动 + 3 归档） | `KeepSome(3)` |
| 目录总量 | 40MB / 档，两档合计 80MB | 上两者相乘 |

该上界是**推导值而非配置项**：插件不提供目录总量预算。实测若归档数超 3 或单档超 40MB，说明 `remove_old_files` 前缀匹配失效，应视为缺陷。验收 A10 用实测字节确认。

### 3.4.2 与 `format` 的覆盖关系

`timezone_strategy()` 会**替换**默认 `format()`（插件源码注释明确「Calling this method overrides the format set in `Self::format`」），且它自带的格式是 `[时间][级别][target]`，与默认的 `[时间][target][级别]` 顺序相反。本规格**接受该形态**，不再调 `format()`：自定义格式需直接依赖 `time` crate 才能拼本地时间，为纯顺序差异引入依赖不划算。

### 3.5 日志格式

由 `timezone_strategy(UseLocal)` 决定，即 `[YYYY-MM-DD][HH:MM:SS][LEVEL][target] message`：

```
[2026-11-09][10:37:54][INFO][assistant_app::setup] Assistant started
```

`target` 保留完整模块路径，是定位来源的唯一字段，不做简化。grep 时注意级别在 target 之前。

### 3.6 高频日志治理规则

**规则 R1（逐增量不写）**：任何由单个流式增量、单个 SSE chunk、单帧渲染触发的日志，默认不写。确需保留时降为 `trace`。

**规则 R2（turn 级汇总）**：一轮对话的流式细节用「起止两条」承载，中间过程不落盘。

落点：

| 位置 | 现状 | 目标 |
|---|---|---|
| `event_sink.rs` `TurnEvent::Delta` 分支 `chat-delta` | 每 delta 一行 DEBUG | **已删除** |
| `event_sink.rs` `StreamStart` 分支 | 无日志 | 新增 `debug!("chat-stream-start turn={turn_id} phase={phase:?}")` |
| `event_sink.rs` `Terminated` 分支 | 无日志 | 新增 `info!("chat-stream-end turn={turn_id} status={status:?} last_seq={last_seq}")`，`Failed` 时带 `error` 摘要 |
| `trace.rs` `log_response_chunk` | 每 chunk DEBUG | 降 `trace!` + 修 `&data[..200]` panic（§3.8.1） |
| `trace.rs` `log_request` | 每请求 DEBUG | 保持（每请求一次，不属高频） |
| `openai/chat.rs` `[SSE #{}]` 预览、`skipping non-standard event` | 每 chunk DEBUG | 降 `trace!` |
| `anthropic/request.rs` `Skipping invalid content_block_delta`、`Unknown Anthropic event type` | 每 delta / 每事件 DEBUG | 降 `trace!` |
| 各 provider `stream done after {} chunks`、`stats.rs` `log_summary` | 每流一次 DEBUG | 保持（每轮一次） |

**门控机制只用 `log` 全局级别**：开发档 `Debug` 天然过滤 `trace`，需要时按 §3.10 提级。不引入 `DiagnosticLevel`——它零调用，用它门控等于永久关死且无开启路径，反而掩盖「以为能按需打开」的事实。

S3 因此是**纯级别降级**，只把 `debug!` 改成 `trace!`，并把 `preview` 计算放进同一个 `log_enabled!` 块以避免关闭态白算字符串：

```rust
if log_enabled!(log::Level::Trace) {
    let preview = truncate_utf8(data, 200);
    trace!("[trace] response attempt={} chunk={} data={}", attempt, chunk_index, preview);
}
```

`chat-stream-end` 用 `info` 保证生产档可见（一轮一行，成本可忽略）。`DiagnosticLevel` 死代码的处理不在本规格范围（§7）。

### 3.7 前端日志转发

前端有**两条**转发路径，门槛必须同时覆盖，否则收敛无效：

| 路径 | 触发 | 门槛 |
|---|---|---|
| `bootstrapLogger()` 劫持 `console.*` | 裸 `console.debug/info/...` | `consoleForwardMinLevel` |
| `createLogger(ctx).debug(...)` | 业务模块经 `Logger` 接口 | `currentLevel` + 直接 `writeTauri` |

`createLogger` 在模块加载期捕获 `console.debug` 原始引用（早于 `bootstrapLogger` 安装劫持），因此它**不会**经由劫持层转发，而是自己直调 `writeTauri`——原实现只判 `currentLevel`，绕过了 `consoleForwardMinLevel`。实测开发档 `[streamTransport] session-state` 仍以 `webview::writeTauri` 前缀写入后端，即此路径。

两处改动：

```ts
let consoleForwardMinLevel: LogLevel = LogLevel.INFO;

// createLogMethod 内
consoleFn(fullMessage);
if (level >= consoleForwardMinLevel) {
  void writeTauri(level, fullMessage);
}
```

`currentLevel` 保持 `isDev ? DEBUG : WARN` 不变：开发档 `console.debug` 仍在浏览器控制台可见，只是不再进后端文件；生产档 `WARN` 不变。`transport.ts` 的 `[coalescer-deliver]` 保留（coalescer 的唯一现场观测点），收敛后不再进后端文件。

### 3.8 敏感信息

沿用 `assistant-providers/src/trace.rs` 既有脱敏：

- URL 中的 `key=` 参数：保留前 4 位 + `***`（`redact_api_key`）。
- 请求体 JSON：直接删除 `apiKey` / `api_key` 键（`redact_json_api_key`）。

新增约束（实施时逐条自检，不新增工具函数）：

| 禁止入日志 | 说明 |
|---|---|
| API key / token / 密码 / 主密码 / 助记词 | 任何形式，含部分掩码以外的原文 |
| vault 条目明文（标题、密码、备注） | 只允许记录条目 id 与操作类型 |
| 附件与图片内容 | 只允许记录字节数与 mime |
| 完整请求 / 响应体 | 任何档位都不写（`DiagnosticLevel` 为死代码，当前不存在「已脱敏的完整体」通道） |

允许入日志：模型名、provider 名、token 用量、`turn_id` / `conversation_id` / `message_id` / `call_id`、工具名、错误码。

### 3.8.1 字段截断（脱敏的另一半）

脱敏只解决「不能写什么」，不解决「写得太多」：`error` 内嵌上游响应体或文件内容时会把单行撑到几 KB。上限如下（超限追加 `...[truncated]`）：

| 字段 | 上限 |
|---|---|
| `error` 摘要 | 512 B |
| 单行消息整体 | 1024 B |
| 路径 / URL | 256 B |
| SSE `preview` | 200 B（现有行为） |
| 上下文键值对 | 8 个，超出追加 `...[N more]` |

换行与控制字符必须转义（`\r` / `\n` / `\t` → 字面形式），否则一条日志被拆成多行，破坏「一行一事件」前提。

**必修 bug（S3 一并处理）**：`trace.rs` 的

```rust
let preview = if data.len() > 200 {
    format!("{}...(truncated {} bytes)", &data[..200], data.len() - 200)
} else { data.to_string() };
```

`len()` 是**字节**长度，`&data[..200]` 按**字节**索引。第 200 字节若落在中文/emoji 的多字节字符中间，切片直接 **panic**，且位置在日志函数内——用户侧表现为「流式中断且无错误提示」。

修法（放在 `trace.rs` 内部，`pub(crate)`，不新建模块）：

```rust
fn truncate_utf8(data: &str, max_bytes: usize) -> String {
    if data.len() <= max_bytes { return data.to_string(); }
    let end = data.char_indices()
        .take_while(|(i, _)| *i < max_bytes)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    format!("{}...(truncated {} bytes)", &data[..end], data.len() - end)
}
```

### 3.9 崩溃转储与日志的关系

`assistant-crash` 的 panic hook 与崩溃转储写在 `%APPDATA%\com.assistant\crashes`，与日志目录 `%LOCALAPPDATA%\com.assistant\logs` **不同**。不改动该路径：崩溃转储是独立兜底链，不受日志滚动影响，反而在日志被覆盖时提供旁证。两者位置差异需写入 ARCHITECTURE（S6）。

### 3.10 运行期调级（能力位，不接线）

`tauri-plugin-log` 的 `build()` 内部已调用 `log::set_max_level(max_level)`，运行期调级只需再调一次 `log::set_max_level(level)`，无需重建 logger。

本规格只保留能力，不接设置页、不加命令；出现「生产档需临时开 debug」的真实诉求时再单独立规格。

**提级前必须打敏感警告**（照抄 vaultwarden 的 `SMTP_DEBUG`，其注释为「This can contain sensitive information we do not want in the default debug/trace logging」）：

```rust
log::warn!("Log level raised to Trace: request/response bodies and SSE chunks may be recorded. Disable after troubleshooting.");
```

`Trace` 会放行逐 chunk 数据与请求体长度，脱敏后仍可能含用户数据，必须让操作者知道自己在临时放宽什么。

### 3.11 已满足、无需引入

以下能力调研过，本项目已具备或天然不适用，记录以免重复调研：

- **前端日志与后端同文件**：已满足。前端走 `plugin-log` 的 `log` 命令，target 固定 `webview::{location}`，与后端同文件，无需 `LogTracer` 桥接。
- **`RUST_LOG` 覆盖**：不适用。`tauri-plugin-log` 不读该变量，临时调级走 §3.10。
- **早期启动日志**：不需要。`main.rs` 首行即 `assistant_crash::install()`，启动期崩溃已有独立转储。
- **会话目录 / 高频数据独立文件 / 内存环形缓冲**：不需要。分别由 `Rotate + KeepSome(3)`、前端内存态、直接读文件替代。

## 4 实施清单

| # | 文件 | 改动 | 状态 |
|---|---|---|---|
| S1 | `src-tauri/src/plugins.rs` | 新增私有函数 `log_plugin() -> tauri::plugin::TauriPlugin<Wry>`，按 §3.1–§3.4 构造 `Builder`；`attach` 中替换默认调用 | 已完成 |
| S2 | `src-tauri/src/adapters/event_sink.rs` | 删 `TurnEvent::Delta` 分支的 `chat-delta` 日志；`StreamStart` 增 `debug`；`Terminated` 增 `info`（含 `last_seq` / `status` / `error` 摘要） | 已完成 |
| S3 | `src-tauri/crates/assistant-providers/src/{trace.rs,openai/chat.rs,anthropic/request.rs}` | 逐 chunk / 逐 delta 日志降 `trace`；`preview` 计算用 `log_enabled!(Trace)` 包裹；新增 `truncate_utf8` 替换 `&data[..200]`（同时修掉 `openai/chat.rs` 的 `&data[..min(200)]` 同类隐患） | 已完成 |
| S4 | `src/services/logger.ts` | `consoleForwardMinLevel` 改为常量 `LogLevel.INFO` | 已完成 |
| S5 | `src-tauri/src/plugins.rs` | 按 §3.3 落确定项 6 条（随 S1 一并写入）；A7 实测后再补待实测项 | 已完成 |
| S6 | `docs/ARCHITECTURE.md` | 第 355 行单句扩写为「日志目录 / 双档 / 滚动 / 崩溃目录」四要素，并链到本文 | 已完成 |

S2/S3 是**唯一改变日志内容**的两步，S1/S4/S5/S6 只改配置与文档。S1 已先行落地，滚动与容量立即生效。

S3 的 `trace.rs` 最终形态：

```rust
pub(crate) fn log_response_chunk(attempt: u32, chunk_index: u32, data: &str) {
    if log_enabled!(Level::Trace) {
        let preview = truncate_utf8(data, 200);
        trace!("[trace] response attempt={} chunk={} data={}", attempt, chunk_index, preview);
    }
}
```

注意 `gemini/request.rs` 的 `log_response_chunk` **调用点不改**——函数内部早返回即可，避免改动 provider 主循环逻辑。

S1 最终形态（`plugins.rs` 内，不新建模块）：

```rust
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

fn log_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let is_dev = cfg!(debug_assertions);

    let mut builder = tauri_plugin_log::Builder::new()
        .level(if is_dev { log::LevelFilter::Debug } else { log::LevelFilter::Info })
        .level_for("ignore", log::LevelFilter::Off)
        .level_for("ignore::walk", log::LevelFilter::Off)
        .level_for("globset", log::LevelFilter::Off)
        .level_for("grep_searcher", log::LevelFilter::Warn)
        .level_for("hyper_util", log::LevelFilter::Info)
        .level_for("h2", log::LevelFilter::Info)
        .rotation_strategy(RotationStrategy::KeepSome(3))
        .max_file_size(10 * 1024 * 1024)
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .file_open_strategy(tauri_plugin_log::FileOpenStrategy::Rotate)
        .clear_targets();

    if is_dev {
        builder = builder.target(Target::new(TargetKind::Stdout));
    }
    builder
        .target(Target::new(TargetKind::LogDir {
            file_name: Some(if is_dev { "assistant-dev" } else { "assistant" }.into()),
        }))
        .build()
}
```

`clear_targets()` 必须调用，否则默认的 `Stdout` + `LogDir { None }` 会与新增目标并存，产生重复写入。

## 5 验收

| # | 场景 | 判据 | 结果 |
|---|---|---|---|
| A1 | 开发档跑一轮 ≥500 delta 的对话 | `assistant-dev.log` 中 `chat-delta` 出现 0 次；`chat-stream-start` / `chat-stream-end` 每轮各 1 次，后者含 `last_seq` | 通过：2 轮对话 `chat-delta`=0，`last_seq`=483 / 1375 |
| A2 | 同 A1 | 文件未在单轮内发生滚动；归档数 ≤3 | 通过：2 轮共 80KB 未滚动，归档 0 |
| A3 | 生产构建启动 | 无 stdout 输出；`assistant.log` 正常写入且含启动行 | 未实测（需生产构建） |
| A4 | 生产档触发一次 `log::error!` 后继续跑多轮 | 该 error 行仍在 `assistant.log` 或最近归档中 | 未实测（需生产构建） |
| A5 | 任意日志行 | 时间戳为本地时区，与系统时间一致 | 通过：`[2026-11-09][19:35:15]` 与系统时间一致 |
| A6 | 开发档前端 `console.debug` | 浏览器控制台可见；后端日志中 `webview` 前缀行只有 `INFO` 及以上 | 通过：修复 `createLogMethod` 后新产生 0 条 DEBUG `webview`，WARN/INFO 正常 |
| A7 | 开发档跑一轮对话 + 一次 grep 工具调用 | 无 `ignore` / `globset` 逐条输出；`grep_searcher` 无 DEBUG | 部分通过：`ignore`/`globset` 零出现；`grep_searcher` 因未跑 grep 工具未覆盖 |
| A8 | 调用一次 provider | 日志中不出现 API key 原文 | 通过：`key=` / `apiKey` / `sk-` / `Bearer` 均为 0 行 |
| A9 | 开发档把全局级别临时提到 `Trace` 后跑一轮 | 逐 chunk 行出现且为 `TRACE`；恢复 `Debug` 后不再出现 | 静态确认：`log_enabled!` 由 `log` crate 保证，运行期提级未接线（§3.10） |
| A10 | 构造 40MB 以上日志量 | 单档总字节 ≤40MB，归档数 ≤3，最旧归档被删 | 源码确认 + Rotate 实测：1MB 活动文件重启后被归档为 `assistant-dev_2026-11-09_19-39-00.log`；`flush` 在 `current_size + buf > max_size` 时 rotate，`KeepSome(3)` 先 `remove_old_files(2)` 再归档，故归档恒为 3、总量 ≤4×10MB。40MB 真实量未实测 |
| A11 | 含中文 / emoji 的 SSE 增量 | 无 panic；`preview` 不出现乱码；后缀为 `...(truncated N bytes)` | 通过：`trace::tests` 3 个 `truncate_utf8` 用例 + 4 个脱敏用例全绿 |

A9：开发档 `Debug` 过滤 `trace` 正是 R1 的默认态，不需 `DiagnosticLevel` 参与。

A11：§3.8.1 panic 修复的回归验证，单测覆盖（`assistant-providers/src/trace.rs`）：

```rust
#[test]
fn truncate_utf8_never_splits_multibyte_char() {
    let result = truncate_utf8("中文中文中文", 7);
    assert!(result.starts_with("中文中"));
    assert!(result.ends_with("(truncated 9 bytes)"));
}
```

A1 / A2 / A7 验证命令（PowerShell）：

```powershell
$log = "$env:LOCALAPPDATA\com.assistant\logs\assistant-dev.log"
@(Get-Content $log | Select-String -Pattern 'chat-delta').Count
(Get-Content $log | Select-String -Pattern 'chat-stream-(start|end)').Line
Get-ChildItem "$env:LOCALAPPDATA\com.assistant\logs" -Filter 'assistant-dev*.log' | Measure-Object
```

## 6 风险与边界

| 风险 | 处理 |
|---|---|
| 删掉 `chat-delta` 后流式卡顿难定位 | `chat-stream-end` 的 `last_seq` 给出总量；乱序由前端 R4 自检告警覆盖（`transport.ts` `R4 seq non-monotonic`），后端看总量、前端看乱序，互补 |
| `KeepSome(3)` 被单轮巨量日志占满 | 单轮已无高频日志（S2/S3），10MB ≈ 10 万行，远超单轮量级 |
| 生产档 `Info` 掩盖可恢复异常 | [error-swallowing-analysis](../analysis/error-swallowing-analysis.md) §4 仍生效：用户可见数据的失败用 `warn` 起。本规格只压第三方噪音与高频细节，不降业务错误级别 |
| 每次启动都产生归档 | `KeepSome(3)` 自动清理，开发期频繁重启最多保留 4 个文件 |
| 降 `trace` 后逐 chunk 日志「事实上不可用」 | 有意为之：默认态本就不该有；需要时按 §3.10 提级且带警告。相比用零调用的 `DiagnosticLevel` 门控（永久关死且无开启路径），提级可控可验证 |
| `truncate_utf8` 引入新越界 | 用 `char_indices` + `len_utf8` 计算边界，天然不越界；A11 单测覆盖多字节 |

## 7 不改变的东西

- `assistant-crash` 的 panic hook、minidump、`crash_{ts}.log` 生成逻辑（`main.rs` 首行 `install()` 位置不变）。
- `capabilities/default.json` 的 `log:default` 权限（前端仍需 `plugin-log` 命令）。
- `logger.ts` 的 `LogLevel` 枚举、`createLogger` 上下文前缀、`formatError` / `formatData` 行为。
- 所有业务 `log::*` 的消息文案与字段（除 §3.6 明确列出的删除/降级项）。
- 前端 `globalErrorHandler` 的 `[CRASH]` 白屏检测链路（保持 `log.error`）。
- `diagnostics.rs` 的 `DiagnosticLevel`：**既不接线也不删除**。接线会让 S3 变成「永久关死」，删除会波及 `tests/diagnostics.rs`，作为独立技术债处理。
- `trace.rs` 的 `redact_api_key` / `redact_json_api_key`（只新增 `truncate_utf8`，不改脱敏逻辑）。

## 8 落地顺序

S1 → S2 → S3 → S4 → S5 → S6，按 §4 清单依次执行，每步可独立回滚（单文件还原，无数据迁移）。S1 完成后应**立即**观察到「日志不再被 40KB 覆盖」，这是整条链路的止血点。

每步后跑 `npm run lint`（前端步）与 `cargo clippy -p assistant-app -p assistant-providers`（Rust 步）。验收对应关系：S1→A2/A3/A5/A10，S2→A1，S3→A9/A11 + `cargo test -p assistant-providers`，S4→A6，S5→A7，S6→文档评审。

**旧日志处理**：默认配置产出的文件名是包名 `Assistant.log`，新配置 dev 档用 `assistant-dev.log`、生产档用 `assistant.log`。Windows 文件名大小写不敏感，故生产档首次启动会命中 `Assistant.log` 并 `Rotate` 归档；dev 档不命中，旧文件成为孤儿（实测：`Assistant.log` 32KB 仍留在目录）。手工删除一次即可：`Remove-Item "$env:LOCALAPPDATA\com.assistant\logs\Assistant.log"`。
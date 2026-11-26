# I78 — 串口调试工具任务清单

| 项 | 值 |
|---|---|
| 状态 | 唯一任务级进度表；只记「改哪里、改没改」，语义一律见规格 |
| 日期 | 2026-11-26 |
| 规格 | [I78-serial-debug-spec.md](./I78-serial-debug-spec.md)（唯一设计来源，本文不复述） |

> 每项末尾括号是规格锚点，用于回查，不在本文展开。`§n` → 规格章节，`Vn` → §7.2 验收判据，`OQn` → §11 开放问题。
> 勾选 = 代码已改且对应验收已跑。
> 批次与规格 §9 一一对应：R1 → R2 → R3。R1 内部按 R1a–R1d 顺序推进，**出口条件共用 §9 R1 那一条**，不因内部分段而提前收口。
> 所有 `cargo` 命令在 `src-tauri` 目录执行（不带 `--manifest-path`）；`npm` / `node` 在仓库根执行。
> PowerShell 先执行 `$OutputEncoding = [System.Text.Encoding]::UTF8; [Console]::OutputEncoding = [System.Text.Encoding]::UTF8;`。

---

## R1 — 契约、领域层、命令、装配、最小页面（§9 R1）

### R1a 契约 DTO 与代码生成

- [ ] `src-tauri/crates/assistant-protocol/src/serial_debug.rs`（新增）：12 个类型 = 6 struct（`SerialPortInfoDto` / `SerialOpenInputDto` / `SerialSetSignalsInputDto` / `SerialSessionDto` / `SerialRxBatchDto` / `SerialDebugErrorDto`）+ 6 enum（`SerialPortKindDto` / `SerialDataBitsDto` / `SerialParityDto` / `SerialStopBitsDto` / `SerialFlowControlDto` / `SerialDebugErrorCodeDto`）（§3.1）
  - 字段逐条照 §3.1 表；`#[serde(rename_all = "camelCase")]`；`#[derive(TS)]` 全部带上（`protocol_bindings` 会比对源码根里所有 `#[derive(TS)]` 类型）
  - 输出 `Option` **不加** `skip_serializing_if`（键恒在、值 `null`）；输入 `Option` 加 `#[ts(optional = nullable)]`；两个输入 struct 加 `#[serde(deny_unknown_fields)]`；无下划线开头字段
  - 封闭集合只列 §3.1 列出的成员，无保留值、无未用成员；计数 `rxBytes` / `txBytes` / `seq` 用 `u64`（工作区 `with_large_int("number")`）
- [ ] `src-tauri/crates/assistant-protocol/src/lib.rs`：`pub mod serial_debug;` 与 `pub use serial_debug::*;` **各 1 处**，两组各自字母序，落点都是 `screenshot` 之后、`shortcut` 之前（§3.1）
- [ ] `src-tauri/crates/assistant-protocol/tests/protocol_bindings.rs`：**两处各加一次**——`use assistant_protocol::{...}` 导入清单、`export_protocol_contract` 的 `declarations` 数组（12 条 `::decl(&cfg)`）；漏加该测试直接报「漏注册 / 多注册」（§3.4）
- [ ] 跑 `npm run contract:gen` 并提交 `src/types/protocol.contract.ts` 变更；**禁止手改生成文件**（§3.4、V6）

### R1b 领域 crate `assistant-serial`

- [ ] `src-tauri/crates/assistant-serial/Cargo.toml`（新增）：`[package]` 沿用 `version.workspace = true` 等四项；依赖 = `assistant-protocol = { workspace = true }`、`serialport = { workspace = true }`、`thiserror = { workspace = true }`、`log = { workspace = true }`；**不依赖 tauri**（§4.1）
- [ ] `src-tauri/Cargo.toml`：`[workspace] members` 增 `"crates/assistant-serial"`；`[workspace.dependencies]` 增 `assistant-serial = { path = "crates/assistant-serial" }` 与 `serialport = { version = "4.7", default-features = false }`；`[dependencies]` 增 `assistant-serial = { workspace = true, optional = true }`；`[features]` 增 `serial_debug = ["dep:assistant-serial"]` 且 `default` 改为 `["vault", "timekeeper", "serial_debug"]`（§6.2）
- [ ] `src-tauri/crates/assistant-serial/src/lib.rs`：只做模块声明与 `pub use`（拆分原则 §4）；对外导出 `SerialRuntime`、`build_event_sink` 所需类型与常量（§4.1）
- [ ] `src-tauri/crates/assistant-serial/src/state.rs`：会话状态机唯一集中点——`Closed/Open`、代际 `u64`、`rxBytes`/`txBytes`、`Arc<Mutex<..>>`；状态迁移只在本文件单一函数内完成（拆分原则 §5「状态集中」、§4.2）
- [ ] `src-tauri/crates/assistant-serial/src/error.rs`：领域错误枚举（`NotOpen` / `AlreadyOpen` / `PortNotFound` / `PermissionDenied` / `Busy` / `Io` / `Unsupported` / `InvalidInput`）→ `SerialDebugErrorDto` 的**唯一**映射点（§3.1、§4.1）
- [ ] `src-tauri/crates/assistant-serial/src/backend.rs`：`PortBackend` / `PortHandle` trait（最小面照 §4.8）+ `serialport` 实现 + DTO↔`serialport` 参数映射（`DataBits` / `Parity` / `StopBits` / `FlowControl`，未知组合 → `InvalidInput`）；Windows 打开时补 `\\.\` 前缀、对外仍是裸名（§4.5、§4.6、§4.8）
- [ ] `src-tauri/crates/assistant-serial/src/session.rs`：`open` / `close` / `write` / `set_signals` 编排；`open` 成功后递增代际并起读线程；`close` 幂等、置停止标志后 `join`（最长 50 ms）；异常转 `closed` 保留计数（§4.2、§4.3）
- [ ] `src-tauri/crates/assistant-serial/src/reader.rs`：读循环——端口 `set_timeout(50 ms)`；`本批 ≥ 4096 B` 或 `距批首 ≥ 16 ms` 触发一次 `Rx`；代际不匹配直接丢弃；循环体 `catch_unwind`（§4.3）
- [ ] `src-tauri/crates/assistant-serial/src/event.rs`：`SerialEvent { Rx(SerialRxBatchDto), Status(SerialSessionDto) }`；`EventSink = Arc<dyn Fn(SerialEvent) + Send + Sync>`（§4.1）
- [ ] 常量集中声明、禁止魔法数字：`READ_TIMEOUT_MS = 50`、`BATCH_INTERVAL_MS = 16`、`BATCH_BYTES = 4096`、`BAUD_RATE_MAX = 4_000_000`（§4.3、§3.2）
- [ ] `src-tauri/crates/assistant-serial/tests/session.rs`：`FakeBackend` 驱动——打开成功/已开再开报错、16 ms/4 KB 聚合阈值、代际丢弃旧批、拔线转 `closed`、写失败转 `closed`、信号线 `Unsupported`、`write` 空串 `InvalidInput`、`close` 幂等（§4.8、V7）
- [ ] `cargo test -p assistant-serial` 通过（V7）

### R1c 命令、插件、装配点

- [ ] `src-tauri/src/cmd/serial_debug/mod.rs`（新增）：事件名常量 `SERIAL_DEBUG_RX = "serial_debug:rx"` / `SERIAL_DEBUG_STATUS = "serial_debug:status"` 与 `EventSink` 构造（内部调 `crate::protocol::events::emit_broadcast`）；`mod` 声明 + `pub use` 各子模块命令（§0.10、§3.3、C10）
- [ ] `src-tauri/src/cmd/serial_debug/ports.rs`：`serial_debug_list_ports` 命令（§3.2）
- [ ] `src-tauri/src/cmd/serial_debug/session.rs`：`serial_debug_open` / `serial_debug_close` / `serial_debug_status` / `serial_debug_write` / `serial_debug_set_signals` 五条命令；签名形如 `(app: AppHandle, rt: State<'_, Arc<SerialRuntime>>, ...)`，错误返回 `SerialDebugErrorDto`（§3.2）
- [ ] `src-tauri/src/cmd/serial_debug/reader.rs`：读线程与 `AppHandle` 的接线（把 `EventSink` 注入领域层）；本文件**不得**出现任何 `#[cfg]`（§6.3 B4）
- [ ] **禁止**在 `src-tauri/src/cmd/mod.rs` 加 `pub use serial_debug::*;`：该行会让 core 足迹多一条含 `serial_debug` 的命中（与 timekeeper 同形，命令一律走 `crate::cmd::serial_debug::<name>` 全路径）（C8）
- [ ] `src-tauri/src/serial_debug_plugin.rs`（新增）：照 `timekeeper_plugin.rs` 逐行对齐的一对互斥块
  - 启用块：`tauri::plugin::Builder::<tauri::Wry>::new("serial_debug")` + **恰 1 个** `generate_handler![`，6 条全部写成 `crate::cmd::serial_debug::serial_debug_list_ports` 这类**全路径**（`build.rs` 只认 `crate::cmd::serial_debug::` 前缀，写短路径 ACL 直接空）；`on_setup` 里 `ctx.app.manage(Arc::new(SerialRuntime::new()))`
  - 关闭块：同样 `Builder::new("serial_debug").build()`，**不得**出现 `generate_handler![`（`build.rs` 见 2 个块会 panic）
  - 两侧 `init` / `on_setup` / `on_exit` **签名逐字一致**（B5）；`on_exit` 调 `close()` 且**不发事件**（§4.7）
- [ ] `src-tauri/capabilities/serial_debug.json`（新增）：`$schema` 指 `../gen/schemas/desktop-schema.json`、`identifier: "serial_debug"`、`windows: ["main", "orb"]`、`permissions: ["serial_debug:default"]`；**不新建 `permissions/serial_debug/` 目录**（`default_permission(AllowAllCommands)` 由 `build.rs` 自动派生）（§6.1、§6.3 A4/B8）
- [ ] `src-tauri/src/lib.rs`：增 `mod serial_debug_plugin;`（**不带 cfg**），落点建议 `mod screenshot;` 之后、`mod shortcuts;` 之前（字母序；门禁不校验顺序，但既有 `mod timekeeper_plugin;` 已在非严格字母位，别借机重排）；改完**行数必须 ≤ 40**（当前 39 → 40，顶格）（§6.2、A3）
- [ ] `src-tauri/src/feature_mounts.rs`：`MOUNTS` 增 1 项（`id: "serial_debug"` / `plugin` / `on_setup` / `on_exit`）；表项本身零 cfg；`MOUNTS` 全文仍只出现 2 次（§6.2、B7）
- [ ] `src-tauri/src/cmd/mod.rs`：增 `#[cfg(feature = "serial_debug")] pub(crate) mod serial_debug;`，落点 `mod screenshot;` 之后、`pub mod startup;` 之前；该文件对 `serial_debug` **恰好 1 条** cfg（§6.2、B6）
- [ ] `config/features.json`：增 `"serial_debug": { "default": true }`；只加条目、不动他项（§0.2、§6.2）
- [ ] `src/vite-env.d.ts`：增 `declare const __SERIAL_DEBUG_ENABLED__: boolean;`（§6.2）
- [ ] `src/features/serial_debug/module.ts`（新增）：`{ id: "serial_debug", ipc: { namespace: "serial_debug" } }`；无 conf 槽、无设置页（§6.1）
- [ ] `src/features/registry.ts`：增 `if (__SERIAL_DEBUG_ENABLED__) { const { serialDebugModule } = await import("./serial_debug/module"); modules.push(serialDebugModule); }`；只允许编译期守卫 + 动态 import（§6.2、`feature-bundle-guard`）
- [ ] `src/features/__tests__/coreBoundary.test.ts`：三处登记——`FEATURE_FOOTPRINT_ALLOWLIST.serial_debug`（`src-tauri/src/cmd/mod.rs`，`markers: ['#[cfg(feature = "serial_debug")]', "mod serial_debug;"]`，`reason` 同 timekeeper）、`FEATURE_PROBE_PREFIX` 增 `serial_debug: "tools"`、`FEATURE_ALIASES` 增 `serial_debug: ["串口"]`；豁免粒度是行、必须带 reason（§6.2）
- [ ] `cargo test --test assembly_contract` 与 `cargo test --test assembly_contract --no-default-features --features serial_debug` 全绿（V1、V5）

### R1d 最小可用页面

- [ ] `src/features/serial_debug/` 下**不**新增任何 `.tsx`：工具页统一放 `src/tools/serial_debug/`（§5.1）
- [ ] `src/tools/serial_debug/serialDebug.service.ts`（新增）：`featureInvoke(serialDebugModule)` 封装 6 条命令 + `listen("serial_debug:rx")` / `listen("serial_debug:status")` 订阅封装（`UnlistenFn` 成对返回）（§3.2、§3.3）
- [ ] `src/tools/serial_debug/byteCodec.ts`（新增）：base64↔`Uint8Array`、HEX 解析/格式化、文本解码（UTF-8 主、Latin-1 兜底）、行尾追加（§5.1、§5.3）
- [ ] `src/tools/serial_debug/index.tsx`（新增，本批只做最小闭环）：端口下拉 + 刷新 + 打开/关闭 + 原始文本收发 + 计数；HEX/暂停/循环/缓冲裁剪留到 R2（§9 R1）
- [ ] `src/tools/framework/registry.ts`：增 `const serialDebugTools: ToolDefinition[] = __SERIAL_DEBUG_ENABLED__ ? [defineTool({ id: "serial-debug", name: "tools.serial_debug.name", description: "tools.serial_debug.description", icon, keywords: [...], component: lazyWithRecovery(() => import("../serial_debug")), widthMode: "full" })] : [];` 并在 `tools` 数组里展开；`keywords` 含 `serial` / `uart` / `baud` / `串口`，**不得**含 `hex`（会命中 converter 签名，分类跑偏）（§0.13、§5.1）
- [ ] `src/tools/__tests__/categories.test.ts`：`EXPECTED_CATEGORIES` 增 `"serial-debug": "dev"`（该用例遍历 `tools`，漏加即 `undefined` 断言失败）（§6.2）
- [ ] `src/i18n/locales/zh-CN.json` 与 `src/i18n/locales/en-US.json`：**同批**新增 `tools.serial_debug` 段（本批先给 `name` / `description` / `port` / `refresh` / `noPorts` / `baudRate` / `open` / `close` / `statusClosed` / `statusOpen` / `receive` / `send` / `rxBytes` / `txBytes` + 8 个 `error*` 键；其余键 R2 补齐）；两份键集合必须相等，否则 `localeParity.test.ts` 失败（§5.5）
- [ ] 本批人工冒烟：Windows 上枚举真实端口 → 打开 → 收数据 → 发送 → 关闭 → **应用退出后端口可被其它工具立刻打开**（V9 的 R1 子集、V11）

> 验收：`npm run typecheck`；`npm run test`；`npm run contract:gen; npm run contract:check`；`cargo test -p assistant-serial`；`cargo test --test assembly_contract`（含 `--no-default-features` 与 `--no-default-features --features serial_debug` 两态）；手工 V9（R1 子集）、V11。

---

## R2 — 完整前端页面（§9 R2）

- [ ] `src/tools/serial_debug/useSerialSession.ts`（新增）：会话状态、事件入队、rAF(16 ms) 合并成一次 `setState`、缓冲裁剪、`code` → i18n 键映射（§5.2、§5.4）
- [ ] 行对象 `{ seq, atMs, bytes: Uint8Array }`：HEX/文本切换**只改渲染**，不重编码数据（§5.2）
- [ ] 缓冲裁剪：超 `5000 行` 或 `2 MB` 从头部删、置 `truncated` 并提示；暂停只停渲染、事件仍入队（队列有上限）（§5.2）
- [ ] 自动滚动：仅当处于底部（8 px 内）且未暂停时跟随；用户上滚即关闭跟随（§5.2）
- [ ] `src/tools/serial_debug/PortToolbar.tsx`（新增）：端口下拉 + 刷新 + 波特率 + 高级参数折叠（数据位/校验/停止位/流控）+ 打开/关闭 + DTR/RTS（§5.1）
- [ ] `src/tools/serial_debug/ReceivePane.tsx`（新增）：HEX/文本切换、暂停/继续、自动滚动、清屏、`rxBytes`/`txBytes`、截断提示（§5.1）
- [ ] `src/tools/serial_debug/SendPane.tsx`（新增）：输入框 + HEX 开关 + 行尾（无/CR/LF/CRLF）+ 循环间隔（`10..60000`）+ 发送；HEX 奇数长度或非法字符即时标红并禁用发送；**不做本地回显**（§5.3）
- [ ] 循环发送用前端 `setInterval`；发送失败或会话转 `closed` 自动停（§4.4、§5.3）
- [ ] `SerialSessionDto` 作为唯一真源：打开/关闭/计数/错误全部来自命令返回或 `status` 事件；异常转 `closed` 保留接收历史并切回可打开态（§5.4）
- [ ] 错误条按 `code` 取 i18n 键；`message` 只进 `console.debug`，不进 UI（§5.4）
- [ ] `src/tools/serial_debug/byteCodec.test.ts`（新增）：base64 往返、HEX 奇偶/非法字符、行尾四态、Latin-1 兜底（V8）
- [ ] i18n 补齐 §6.5 全部键（两份同批）：`dataBits` / `parity` / `stopBits` / `flowControl` / `advanced` / `viewText` / `viewHex` / `autoscroll` / `pause` / `resume` / `clear` / `truncated` / `lineEnding` / `lineNone` / `lineCr` / `lineLf` / `lineCrlf` / `repeat` / `repeatInterval` / `sendHint` / `dtr` / `rts` / `history` / `deviceDisconnected`（§6.5、V12）
- [ ] 复核页面内**无**固件烧录、协议解析、日志导出、多端口分栏、参数持久化等推迟项占位 UI（§1.2、§10）

> 验收：`npm run typecheck`；`npm run test`（含 `byteCodec.test.ts` 与 `localeParity.test.ts`）；手工 V9 全流程 + V10 端口独占 + V13 大流量 30 s。

---

## R3 — CI 接线与文档同步（§9 R3）

- [ ] `.github/workflows/ci.yml` 前端 job：slim 构建行改为 `VAULT_ENABLED=0 TIMEKEEPER_ENABLED=0 SERIAL_DEBUG_ENABLED=0 npm run build`；新增 `node scripts/assert-feature-absent.mjs --feature serial_debug --prefix tools --dist dist`（§6.4、V4）
- [ ] `.github/workflows/ci.yml` rust job 增 `--features serial_debug` 两条：`cargo check -p assistant-app --no-default-features --features serial_debug`、`cargo test -p assistant-app --no-default-features --features serial_debug --test assembly_contract`、`node ../scripts/assert-feature-shed.mjs --features serial_debug`（与既有 vault / timekeeper 三件套同形）（§6.4、V3）
- [ ] `docs/ARCHITECTURE.md` §4.2：工具清单 `23 个` → `24 个`，`dev` 行追加「串口调试」；只改计数与条目（§6.2）
- [ ] 全量回归：`cargo test --workspace --all-targets --no-fail-fast`（开发机）+ `npm run typecheck` + `npm run test` + `npm run contract:check` + `npx eslint . --max-warnings 0`（§7.1）
- [ ] 复核关闭态：`cargo tree --no-default-features -e normal` 无 `serialport` / `assistant-serial`；slim 构建产物无 `tools.serial_debug.` 字面量（V2、V4）

> 验收：7.1 全部命令通过；V1–V8、V12 全绿。

---

## 收尾

- [ ] 复核 cfg 预算：`serial_debug` 全后端仍**恰好 3 行** `#[cfg]`（插件启用块 1 + 插件关闭块 1 + `cmd/mod.rs` 1）；`cmd/serial_debug/*.rs` 与 `assistant-serial` 内零 `#[cfg]`（V5、§8 R1）
- [ ] 复核 core 足迹：`src-tauri/src`、`src/features`、`src/settings` 内除 `ASSEMBLY_POINTS` 与特性本体外，只有 `cmd/mod.rs` 一条豁免命中（§6.2 C8）
- [ ] 复核 `lib.rs` 行数 = 40（≤ 上限）：已顶格，**下一个特性开工前**必须先提「插件声明收敛」议题（§8 R2）
- [ ] 复核回滚路径：删 `config/features.json` 条目 + 撤 §6.2 全部改动 + 移出 workspace 成员，无残留状态与数据迁移（§8）

---

## 未决项（不阻塞开工，按 §11 倾向推进）

| 项 | 未定案时的行为 |
|---|---|
| OQ1 默认启用 | 按 `"default": true` 落地；slim 仍可 `SERIAL_DEBUG_ENABLED=0` 整体裁掉 |
| OQ2 Linux 是否启用 `serialport/libudev` | 不启用（`default-features = false`）；端口元数据取不到输出 `null`，前端只显示端口名 |
| OQ3 文本视图编码集合 | 只 UTF-8 + Latin-1 兜底；GBK / UTF-16 推迟 |
| OQ4 工具 id 形态 | `serial-debug`（kebab），URL `/tools/serial-debug` |
| OQ5 macOS 是否列 `tty.*` | 都列出，打开优先 `cu` |
| OQ6 打开后是否默认置 DTR | 默认 `dtr = true`，`rts` 不动，可手动改 |
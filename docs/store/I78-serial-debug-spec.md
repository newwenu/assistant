# I78 — 串口调试工具规格（serial_debug）

| 项 | 内容 |
|---|---|
| 状态 | 待评审 |
| 日期 | 2026-11-26 |
| 类型 | 特性规格（新增可选特性 + 新增工具页） |
| 特性 id | `serial_debug` |
| 工具 id | `serial-debug` |
| 关联 | 装配规范：`docs/store/feature-optional-binary-spec.md`；拆分判据：`docs/principle/CODE-SPLIT-PRINCIPLE.md`；参考实现：`references/10-serial-debug/espflash`（只取 monitor 的读循环与信号线思路，不取烧录） |
| 平台 | Windows / Linux / macOS，同一实现，无平台分叉文件 |
| 影响范围 | 新增 1 个可选特性、1 个后端 crate、1 个前端工具页；后端 core 3 个装配点 + 1 条测试豁免 |
| 一句话 | 在项目内提供枚举端口、打开会话、实时收发原始字节的串口调试工具，按既有可选特性范式装配，关闭态零产物。 |

> 自包含：本文档单独交给实施者即可完成开发与验收。只引用《可选特性二进制装配规范》与《代码拆分原则》，不复述其条款。
> 边界：**不做固件烧录、不做协议解析、不做日志落盘**（见 §1.2）。

---

## 0 判定

| # | 判定 | 结论 | 理由 |
|---|---|---|---|
| 0.1 | 形态 | 可选特性 `serial_debug` + 工具页 | 独占依赖第三方 crate `serialport`，必须能被整体裁掉 |
| 0.2 | 默认启用 | `config/features.json` → `"default": true` | 与 vault/timekeeper 一致；slim 仍可整体裁掉（OQ1） |
| 0.3 | 特性 id | `serial_debug`，禁用 `serial` | `serial` 是 `serialize`/`serialization`/`serial_number` 的子串，会让 core 边界扫描大面积误报 |
| 0.4 | 工具 id | `serial-debug`（kebab） | 与既有工具 URL 形态一致：`/tools/serial-debug` |
| 0.5 | 后端 crate | `src-tauri/crates/assistant-serial` | 串口状态机 + 读线程是领域逻辑，不得进主 app |
| 0.6 | 契约位置 | `src-tauri/crates/assistant-protocol/src/serial_debug.rs` | 与 timekeeper 的 `clock.rs` 同范式：DTO 恒在编译，命令面随特性消失 |
| 0.7 | 编解码位置 | 后端只传 base64 原始字节，解码全在前端 | 后端不引入字符集判断依赖；前端可自由切 HEX/文本/编码 |
| 0.8 | 会话数 | 全局单会话，同时只开 1 个端口 | 多端口并存需要 UI 分栏与独立事件通道，v1 不做（§10） |
| 0.9 | 发送队列 | 后端无发送队列，`write` 同步直写 + `flush` | 调试语义是"发了就是发了"，隐藏队列会掩盖时序问题 |
| 0.10 | 事件通道 | `serial_debug:status` 用 `protocol::events::emit_broadcast`；`serial_debug:rx` 用 `app.emit_to(assistant_config::constant::WINDOW_MAIN, ..)` 定向主窗口 | `emit_broadcast` 是 `app.emit`（全窗口），115200 持续灌入时 orb 小窗会跟着逐批序列化，白烧 CPU；工具页只在主窗口。事件名常量只放特性目录；**不写入** `src-tauri/src/protocol/events.rs`（避免再增豁免） |
| 0.11 | 读侧节流 | 端口读超时 **8 ms**；批边界为 **16 ms 或 4096 B** 先到者 | 16 ms 只是批边界上限，实际粒度受读超时约束：读超时取 8 ms 才能让 16 ms 边界真正生效（若取 50 ms，空闲时最坏 50 ms 才发一批，且 `close` 的 `join` 也要等 50 ms）。避免逐字节 IPC 洪泛 |
| 0.12 | 错误形态 | 命令失败返回 `SerialDebugErrorDto`（`code` 封闭枚举） | 对齐 vault 的 `VaultErrorDto` 范式；timekeeper 的 `Result<_, String>` 无法支撑 i18n 错误文案 |
| 0.13 | 工具分类 | `dev`；`keywords` 钉死为 `["serial","uart","baud","com-port","串口"]` | `classifyKeywords` 是「keywords 与各类签名词表**精确匹配**、取最高分，全零则落首类 dev」——dev 签名表内并无上述四个词，分类结论靠兜底成立。**禁止加入 `hex`/`encode`/`decode`/`bit`/`binary`/`time`/`timestamp`**：这些是 converter 签名，一加就使 converter 得分 > 0，工具被分到 converter，`src/tools/__tests__/categories.test.ts` 立即断言失败 |
| 0.14 | 页面形态 | `widthMode: "full"`，上下分栏（上接收、下发送） | 收发需同时可见 |
| 0.15 | 打开后初始信号线 | 打开成功默认置 `dtr = true`、`rts` 不动 | 部分 USB 转串口设备不拉 DTR 不出数据，避免"打开了没反应"的首因误解（OQ6） |
| 0.16 | 持久化 | v1 无后端配置组；端口/波特率/编码/行尾/时间戳开关由前端 `localStorage` 记住上次值 | 后端零 conf 组即零 DTO 同步成本；前端本地记忆不产生 wire 契约，也不违反「无后端配置组」。跨设备/跨安装同步见 §10 |
| 0.17 | 插件 ACL 名 | 插件名与 capability 权限前缀用 `serial-debug`；特性 id、模块路径、文件名仍用 `serial_debug` | `tauri-utils` 的 ACL `Identifier` 只允许小写 ASCII 与连字符，含下划线直接报 `invalid plugin or permission identifier 'serial_debug:default'`。`build.rs` 把插件文件名里的 `_` 统一转 `-` 后交给 `tauri_build::Attributes::plugin()`，运行时 `plugin::Builder::new()` 与 `capabilities/*.json` 必须用同一个连字符名，三者错一个就启动即失败 |

---

## 1 范围与交付

### 1.1 交付功能

| 编号 | 功能 |
|---|---|
| F1 | 端口枚举：端口名、类型、VID/PID、序列号、厂商、产品；可刷新 |
| F2 | 打开会话：端口 + 波特率 + 数据位 + 校验 + 停止位 + 流控 |
| F3 | 关闭会话：释放端口、停读线程、计数归零；幂等 |
| F4 | 实时接收：原始字节，16 ms/4 KB 聚合为批量事件，不丢字节不重排 |
| F5 | 发送：HEX 或文本输入，行尾可选（无/CR/LF/CRLF），循环发送可配间隔；发送历史（最近 20 条，点击回填） |
| F6 | 显示：HEX/文本双视图、文本编码可选（UTF-8/GBK/Latin-1）、可选时间戳列、暂停、自动滚动、清屏、截断提示 |
| F7 | 计数：`rxBytes`/`txBytes` 会话内累计，状态事件同步 |
| F8 | 信号线：DTR/RTS 手动置位/清零；平台不支持返回 `unsupported` |
| F9 | 异常收口：读写失败或设备拔出 → 转 `closed` 并携带错误码，前端不崩 |
| F10 | 缓冲上限：前端环形缓冲 5000 行 / 2 MB，超出丢头部并提示 |
| F11 | 端口占用：被占用/无权限时打开失败，错误码可区分（`busy`/`permissionDenied` 仅 Linux/macOS 可达，Windows 归 `io`，见 §3.1 平台说明） |
| F12 | 关闭态零产物：无工具入口、无 i18n 文案、无 crate 与依赖 |

### 1.2 不交付

- 固件烧录、bin 下载、擦写（参考项目 espflash 的 `flash`/`erase` 逻辑不取）。
- 协议解析、AT 指令集、自定义帧解析。
- 日志落盘、导出文件、会话回放、自动重连。
- 多端口并存、端口热插拔事件订阅。
- 后端 conf 组持久化（无 conf 组；前端 `localStorage` 记住上次参数与发送历史，属本地 UI 状态，不落 wire 契约）。

---

## 2 术语与硬约束

| # | 术语/约束 | 定义 |
|---|---|---|
| C1 | 会话 | 一次 `open` 到 `close`（或异常转 `closed`）之间的后端状态：端口句柄、读线程、代际号、计数 |
| C2 | 代际 | 每次成功 `open` 递增的 `u64`；读线程携带代际，代际不匹配的事件一律丢弃（防重开后旧线程串话，与 `cmd/timekeeper/ticker.rs` 同手法） |
| C3 | 批 | 一次 `serial_debug:rx` 事件承载的字节集合，带 `sessionId`（会话代际）与 `seq`（会话内从 1 单调递增，跨会话不保证单调） |
| C4 | 后端 | 抽象 `PortBackend` trait；生产实现用 `serialport`，测试用内存假端口 |
| C5 | 硬约束·字节不解码 | 后端只做 `read → base64`、`write(base64 → bytes)`；任何字符集判断都在前端 |
| C6 | 硬约束·单会话 | 已有会话时 `open` 返回 `alreadyOpen`，不隐式关闭旧会话 |
| C7 | 硬约束·cfg 预算 | `serial_debug` 在全后端**恰好 3 行** `#[cfg]`：插件启用块 1 + 插件关闭块 1 + `cmd/mod.rs` 1；不得再增（`assembly_contract` B4/B6 会红） |
| C8 | 硬约束·core 足迹 | 扫描根 `src-tauri/src`、`src/features`、`src/settings` 内，足迹扫描会跳过 `ASSEMBLY_POINTS`（`config/features.json`、`src/vite-env.d.ts`、`src/features/registry.ts`、`src-tauri/Cargo.toml`、`src-tauri/src/feature_mounts.rs`、`src-tauri/src/lib.rs`、`src-tauri/src/feature_plugins/mod.rs`）与特性本体目录；因此只有 `src-tauri/src/cmd/mod.rs` 需要新增 1 条测试豁免。`src/tools/**` 不在扫描根内 |
| C9 | 硬约束·读线程不阻塞退出 | 读循环用 8 ms 读超时 + 停止标志，退出最多延迟 8 ms；不依赖"关句柄打断读"的平台语义 |
| C10 | 硬约束·事件只从特性目录发 | 事件名常量、事件发射点（`emit_to` / `emit_broadcast`）都在 `src-tauri/src/cmd/serial_debug/`，core 的事件表零改动 |

---

## 3 数据契约

### 3.1 DTO（`assistant-protocol/src/serial_debug.rs`）

封闭集合一律 `enum`；输出 `Option` 不加 `skip_serializing_if`（键恒在、值 `null`，前端声明 `T | null` 必填）；输入 `Option` 加 `#[ts(optional = nullable)]`，入参 struct 加 `deny_unknown_fields`；禁止下划线开头字段。

```
SerialPortKindDto        = "usb" | "bluetooth" | "pci" | "unknown"
SerialDataBitsDto        = "five" | "six" | "seven" | "eight"
SerialParityDto          = "none" | "odd" | "even" | "mark" | "space"
SerialStopBitsDto        = "one" | "two"
SerialFlowControlDto     = "none" | "software" | "hardware"
SerialDebugErrorCodeDto  = "notOpen" | "alreadyOpen" | "portNotFound" | "permissionDenied"
                         | "busy" | "io" | "unsupported" | "invalidInput"
```

> 平台可达性：`permissionDenied` 与 `busy` 只有 Linux（`dialout` 组）/ macOS 才可能返回。Windows 的 COM 口**通常不做进程级独占**，同一端口被两个进程同时打开多半会成功，失败一律归 `io`。因此「端口独占」用例（V10）只在 Linux/macOS 断言，Windows 侧不断言 `busy`（见 §7.2）。

| DTO | 方向 | 字段 |
|---|---|---|
| `SerialPortInfoDto` | 出 | `portName: string`、`kind`、`vid: number \| null`、`pid: number \| null`、`serialNumber: string \| null`、`manufacturer: string \| null`、`product: string \| null` |
| `SerialOpenInputDto` | 入 | `portName: string`、`baudRate: number`、`dataBits`、`parity`、`stopBits`、`flowControl` |
| `SerialConfigDto` | 出 | 与 `SerialOpenInputDto` 同字段，但**全部必填**（无 `#[ts(optional = nullable)]`） |
| `SerialSetSignalsInputDto` | 入 | `dtr: boolean \| null`（可选）、`rts: boolean \| null`（可选） |
| `SerialSessionDto` | 出 | `sessionId: number`、`open: boolean`、`portName: string \| null`、`config: SerialConfigDto \| null`、`rxBytes: number`、`txBytes: number`、`error: SerialDebugErrorDto \| null` |
| `SerialRxBatchDto` | 出（事件） | `sessionId: number`、`seq: number`、`receivedAtMs: number`（epoch 毫秒，`u64`）、`dataBase64: string` |
| `SerialDebugErrorDto` | 出（失败） | `code: SerialDebugErrorCodeDto`、`message: string` |

> 出参 `config` **不得**复用入参 `SerialOpenInputDto`：入参要求 `#[ts(optional = nullable)]` + `deny_unknown_fields`，复用会让生成的 `config` 字段全部变可选，与「输出 `Option` 键恒在、值 `null`、前端声明 `T | null` 必填」的规则冲突，也丢掉「`config` 非空即参数齐全」的语义。故单独出 `SerialConfigDto`。
> `sessionId` 即 C2 的代际号，`SerialSessionDto` 与 `SerialRxBatchDto` 都带，前端据此做 `(sessionId, seq)` 双重过滤（见 §3.3）。关闭态保留最后一次成功 `open` 的代际号（从未开过则为 `0`），前端据此判断"这批是不是我当前会话的"。
> `receivedAtMs` 是**批首字节到达时刻的 epoch 毫秒**（`SystemTime::now()` 距 `UNIX_EPOCH` 的 ms），不是会话内相对毫秒：前端可直接 `new Date(atMs)` 格式化出绝对时间，与外部日志/示波器截图对齐，重开端口后语义也不变。量级约 1.7e12，远小于 2^53，`u64 → number` 安全。

计数用 `u64`（沿用工作区 `TS_RS_LARGE_INT` 映射为 `number`；会话内计数不可能逼近 2^53，安全）。

### 3.2 命令（6 条，统一 `serial_debug_` 前缀，注册在插件 `generate_handler!`）

| 命令 | 入参 | 返回 | 语义 |
|---|---|---|---|
| `serial_debug_list_ports` | 无 | `Vec<SerialPortInfoDto>` | 枚举；不改会话状态 |
| `serial_debug_open` | `input: SerialOpenInputDto` | `SerialSessionDto` | 已开 → `alreadyOpen`；端口不存在 → `portNotFound`；占用/无权限 → `busy`/`permissionDenied`（仅 Linux/macOS 可达，Windows 归 `io`） |
| `serial_debug_close` | 无 | `SerialSessionDto` | 幂等：未开时返回 `open=false` 快照，不报错 |
| `serial_debug_status` | 无 | `SerialSessionDto` | 前端挂载/窗口聚焦时对齐真实状态 |
| `serial_debug_write` | `dataBase64: string` | `SerialSessionDto` | 未开 → `notOpen`；空串 → `invalidInput`；同步直写，返回写后计数 |
| `serial_debug_set_signals` | `input: SerialSetSignalsInputDto` | `SerialSessionDto` | 未开 → `notOpen`；平台不支持 → `unsupported` |

`baudRate` 校验区间 `1..=4_000_000`，越界 `invalidInput`；不做"常用波特率白名单"（自定义波特率是调试刚需）。

### 3.3 事件

| 事件名 | 载荷 | 触发时机 |
|---|---|---|
| `serial_debug:rx` | `SerialRxBatchDto` | 读线程聚合到阈值；**定向 `main` 窗口**（`emit_to`），不广播 |
| `serial_debug:status` | `SerialSessionDto` | 打开成功、关闭、异常转 `closed`（写成功不逐次发，计数随命令返回）；`emit_broadcast` |

前端用 `listen` 订阅，工具页挂载时建立、卸载时解除。过滤规则是 `(sessionId, seq)` 双重判定：`sessionId` 必须等于当前会话代际号（由 `open`/`status` 返回的 `SerialSessionDto.sessionId` 建立基线），`seq` 只需在同一 `sessionId` 内递增。**只比 `seq` 不够**：旧会话残留批的 `seq` 往往远大于新会话首值，会被误判成新数据，表现为重开端口后接收区串入上次的尾巴。

### 3.4 生成与同步

- 新增/修改 DTO 后必须 `npm run contract:gen`，产物 `src/types/protocol.contract.ts`；禁止手改生成文件。
- `npm run contract:check` 作为门禁，CI 校验无漂移。

---

## 4 后端设计

### 4.1 分层（`src-tauri/crates/assistant-serial`）

| 文件 | 职责 |
|---|---|
| `Cargo.toml` | 依赖：`assistant-protocol`（workspace）、`serialport`（workspace，`default-features = false`）、`thiserror`、`log` |
| `src/lib.rs` | 只做模块声明与 `pub use`（拆分原则 §4） |
| `src/state.rs` | 会话状态机：`Closed/Open`、代际、计数、`Arc<Mutex<..>>` 集中点（拆分原则 §5"状态集中"） |
| `src/backend.rs` | `PortBackend` trait + `serialport` 实现 + DTO↔`serialport` 参数映射 |
| `src/session.rs` | 打开/关闭/写/信号线编排，读线程启动与代际校验 |
| `src/reader.rs` | 读循环：8 ms 超时、16 ms/4 KB 聚合、`EventSink` 回调 |
| `src/event.rs` | `SerialEvent { Rx(SerialRxBatch), Status(SerialSession) }` 领域事件 |
| `src/error.rs` | 领域错误 → `SerialDebugErrorDto` 的唯一映射点 |
| `tests/session.rs` | `FakeBackend` 驱动：打开、聚合、拔线、写失败、信号线不支持 |

`assistant-serial` **不依赖 tauri**。事件外发通过注入 `EventSink = Arc<dyn Fn(SerialEvent) + Send + Sync>`，由 cmd 层用 `AppHandle` 构造：`Rx` 走 `emit_to(WINDOW_MAIN)`，`Status` 走 `emit_broadcast`（见 0.10）。领域层因此可单测、可移植。

### 4.2 状态机

```
Closed --open ok--> Open --close--> Closed
Open --close--> Closed（计数归零）
Open --读写 io 错误--> Closed(error) + 发 status（计数保留）
Open --open--> Err(alreadyOpen)（状态不变）
```

- 状态迁移只发生在 `state.rs` 的单一函数内；每次迁移必发一次 `serial_debug:status`。
- 异常转 `closed` 保留计数（用户能看到断开前收了多少），下次 `open` 成功后归零。

### 4.3 读线程

- `open` 成功后 `std::thread::spawn`；端口 `set_timeout(8 ms)`，循环 `read`：
  追加字节到本批 → 当 `本批字节 ≥ 4096` 或 `距批首字节 ≥ 16 ms` → 发 `Rx` → 清批。
- 读超时必须小于批边界（8 ms < 16 ms），否则 `read` 超时成为实际节拍：空闲时最坏要等一个完整读超时才发批，`close` 的 `join` 也跟着变慢。
- 每批带 `sessionId`（本会话代际）与 `seq`（本会话内从 1 递增）；重开端口后 `seq` 从头计，靠 `sessionId` 区分新旧（C3）。
- 每批的 `receivedAtMs` 取**批首字节到达时刻**的 epoch 毫秒（`SystemTime::now()` 距 `UNIX_EPOCH`）；同批内多行共享该值（§3.1、§5.2）。
- 停止：`AtomicBool` 停止标志 + 代际比对；`close` 置标志后 `join`，最长阻塞 8 ms。
- 读错误（`NoDevice`/`BrokenPipe`/`PermissionDenied` 等）统一按 §4.2 转 `closed`，`code` 取 `io`；不细分平台错误码（平台差异由前端文案兜底）。
- 循环体包 `catch_unwind`：panic 也转 `closed`，避免会话永久假死。

### 4.4 写入与信号线

- `write`：base64 解码 → `write_all` → `flush`（调试场景必须显式 `flush`，否则"发了没反应"）→ `txBytes += n` → 返回快照。
- 信号线：`write_data_terminal_ready` / `write_request_to_send`；平台返回 `Unsupported` 时映射为 `unsupported`。
- 循环发送不在后端实现（后端藏定时器无法观察）；由前端定时器驱动。

### 4.5 端口枚举与名称规范化

- 枚举用 `serialport::available_ports()`。
- Windows：`COM10` 及以上打开必须走 `\\.\COM10`；`open` 时统一补前缀，`portName` 对外仍是裸名 `COM10`（前端展示与回传都用裸名）。
- Linux：`default-features = false`（不带 `libudev`），枚举退化为 `/sys/class/tty` 扫描，USB 元数据取不到时输出 `null`（OQ2）。
- macOS：`/dev/cu.*` 与 `/dev/tty.*` 都列出，打开优先 `cu`（`tty` 会阻塞等载波）（OQ5）。
- 排序：`portName` 自然序（`COM2` 排在 `COM10` 前）。

### 4.6 参数映射

`SerialDataBitsDto`→`serialport::DataBits`、`SerialParityDto`→`Parity`、`SerialStopBitsDto`→`StopBits`、`SerialFlowControlDto`→`FlowControl`（`software`=XON/XOFF，`hardware`=RTS/CTS）。映射表只放 `backend.rs`；未知组合返回 `invalidInput`。

### 4.7 退出清理

`on_exit`（`ExitHook`）调 `close()`：停线程、关端口；未开时返回 `Ok(())`（幂等）。**`on_exit` 内不发事件**（窗口已销毁）。

### 4.8 可测性（`PortBackend` 最小面）

```
trait PortBackend {
    fn list(&self) -> Result<Vec<SerialPortInfo>, SerialError>;
    fn open(&self, cfg: &SerialConfig) -> Result<Box<dyn PortHandle>, SerialError>;
}
trait PortHandle {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize>;
    fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()>;
    fn flush(&mut self) -> std::io::Result<()>;
    fn set_dtr(&mut self, on: bool) -> Result<(), SerialError>;
    fn set_rts(&mut self, on: bool) -> Result<(), SerialError>;
}
```

`FakeBackend` 支持：注入可读字节流、模拟读错误、模拟写错误、记录写内容、记录信号线调用。

---

## 5 前端设计

### 5.1 文件结构（`src/tools/serial_debug/`）

| 文件 | 职责 |
|---|---|
| `index.tsx` | 工具页组合入口：工具栏 + 接收区 + 发送区 |
| `serialDebug.service.ts` | `featureInvoke(serialDebugModule)` 的 6 命令封装 + 2 事件订阅封装 |
| `useSerialSession.ts` | 会话状态、事件合并、rAF 刷屏、缓冲裁剪、错误映射 |
| `byteCodec.ts` | base64↔`Uint8Array`、HEX 解析/格式化、文本解码（`TextDecoder` 原生编码表：UTF-8/GBK/Latin-1）、行尾追加 |
| `localPrefs.ts` | `localStorage` 读写上次端口/波特率/编码/行尾/时间戳开关与发送历史（最近 20 条）；键名 `serial_debug.prefs` |
| `PortToolbar.tsx` | 端口下拉 + 刷新 + 波特率 + 高级参数（折叠）+ 打开/关闭 + DTR/RTS |
| `ReceivePane.tsx` | HEX/文本切换、编码选择、时间戳列开关、复制（跟随时间戳开关）、暂停、自动滚动、清屏、计数、截断提示 |
| `SendPane.tsx` | 输入框 + HEX 开关 + 行尾 + 循环间隔 + 发送 + 发送历史下拉回填 |
| `byteCodec.test.ts` | 编解码往返与边界（含 GBK 解码） |

工具注册：`src/tools/framework/registry.ts` 增 `serialDebugTools`（`__SERIAL_DEBUG_ENABLED__ ? [defineTool({...})] : []`），`widthMode: "full"`，`lazy` 页面组件，`keywords` **只能是** `["serial","uart","baud","com-port","串口"]`（理由见 0.13：多写一个 `hex` 就会被分到 converter）。

### 5.2 数据流与限流

```
listen("serial_debug:rx") → 入 ref 队列（不 setState）
rAF(16 ms) → 合并队列 → 生成行对象 → setState 一次
```

- 行对象：`{ sessionId, seq, atMs, bytes: Uint8Array }`；HEX/文本切换只改渲染，不重编码数据。
- 入队前先按 `(sessionId, seq)` 过滤（§3.3），`sessionId` 不符的批直接丢弃，不进队列。
- 时间戳列渲染 `atMs`（批首字节的 epoch 毫秒），开关状态由 `localPrefs` 持久化。
- 时间戳是**批级**而非行级：一批可含多行，同批内所有行共享同一个 `atMs`。不按行造不同毫秒值——那超出后端实际精度（批边界 16 ms），属伪造。精度上限即 16 ms，测时延/看心跳间隔够用。
- 复制接收区内容时**与时间戳开关一致**：开关开则每行带 `HH:MM:SS.mmm ` 前缀，关则只复制原始数据（文本按当前编码解出，HEX 视图复制 HEX 串）。所见即所得，不另设"复制含时间戳"开关。
- 缓冲裁剪：超 `5000 行` 或 `2 MB` 从头部删，置 `truncated = true` 并提示"已丢弃早期数据"。
- 自动滚动：仅当用户处于底部（8 px 内）且未暂停时滚到底；用户上滚即关闭跟随。
- 暂停只停渲染，事件仍入队（队列有上限，超限丢弃并计入 `truncated`）。

### 5.3 发送

- 文本模式：输入原样 + 行尾（无/CR/LF/CRLF）→ UTF-8 编码 → base64（发送侧固定 UTF-8，GBK 只影响显示侧解码）。
- HEX 模式：允许空格/逗号/`0x` 前缀；奇数长度或非法字符即时标红并禁用发送。
- 循环发送：`setInterval(intervalMs)`，区间 `10..60000`；发送失败或会话关闭自动停。
- 发送历史：每次成功发送把「模式 + 原文 + 行尾」压入 `localPrefs`，保留最近 20 条，下拉选择即回填输入框；失败不入历史。
- 不做本地回显（回显由设备真实返回驱动），避免与接收区混淆。
- 打开会话前，工具栏默认值取 `localPrefs` 的上次端口/波特率/编码/行尾；端口列表里找不到上次端口时回落到第一个可用端口，不报错。

### 5.4 状态与错误

- `SerialSessionDto` 是唯一真源：打开/关闭/计数/错误全部来自命令返回或 `status` 事件。
- 错误展示：顶部内联条，文案按 `code` 取 i18n 键；`message` 只进 `console.debug`。
- 会话异常转 `closed`：保留接收历史、工具栏切回可打开态、循环发送停止。

### 5.5 i18n

- 全部文案键 `tools.serial_debug.*`，写进 `src/i18n/locales/zh-CN.json` 与 `src/i18n/locales/en-US.json` 的 `tools.serial_debug` 段；**两份必须同批改**，否则 `node scripts/check-i18n-parity.mjs` 的「zh / en 键集合完全相等」断言失败。
- 注意：该检查由 `npm run check`（`scripts/check.mjs`）调起，**不在 `npm run test`（vitest）里**。只跑 `npm run test` 会漏检、假绿；验收必须跑 `npm run check`（见 §7.1）。
- 不得出现裸中文/英文文案。
- 键集合见 §6.5；关闭态探针依赖这些字面量存在于 `src/tools/serial_debug`。

---

## 6 装配点

### 6.1 新增文件

| 文件 | 说明 |
|---|---|
| `src-tauri/crates/assistant-serial/**` | §4 领域 crate |
| `src-tauri/crates/assistant-protocol/src/serial_debug.rs` + `lib.rs` 增 `pub mod serial_debug;` | DTO 契约 |
| `src-tauri/src/feature_plugins/serial_debug_plugin.rs` | 一对互斥块 + `init/on_setup/on_exit`（照 `timekeeper_plugin.rs` 逐行对齐） |
| `src-tauri/src/cmd/serial_debug/{mod.rs,ports.rs,session.rs,reader.rs}` | 6 命令 + 事件常量 + 读线程 sink |
| `src-tauri/capabilities/serial_debug.json` | 仅授 `serial-debug:default`，`windows: ["main"]`（工具页只在主窗口，`rx` 事件也定向主窗口，见 0.10 / 0.17） |
| `src/features/serial_debug/module.ts` | `FeatureModule`：仅 `id` + `ipc` 命名空间；无 conf 槽、无设置页 |
| `src/tools/serial_debug/**` | §5 |
| `src/i18n/locales/zh-CN.json` + `en-US.json` 的 `tools.serial_debug` 段 | 文案（两份同批） |

### 6.2 改动清单（core 只 3 处 + 测试数据）

| 文件 | 改动 | 约束 |
|---|---|---|
| `config/features.json` | 增 `"serial_debug": { "default": true }` | 只加条目，不动他项 |
| `src-tauri/Cargo.toml` | `[workspace.members]` 增 `"crates/assistant-serial"`；`[workspace.dependencies]` 增 `assistant-serial = { path = "crates/assistant-serial" }` 与 `serialport = { version = "4.7", default-features = false }`；`[dependencies]` 增 `assistant-serial = { workspace = true, optional = true }`；`[features]` 增 `serial_debug = ["dep:assistant-serial"]`，`default` 改为 `["vault", "timekeeper", "serial_debug"]` | 不改无关依赖 |
| `src-tauri/src/feature_plugins/mod.rs` | 增 `pub(crate) mod serial_debug_plugin;`（**不带 cfg**，与既有两个插件同形） | `lib.rs` 不新增行（保持 38 行，上限 40）；插件声明已收敛到本文件（§8 R2 已消解） |
| `src-tauri/src/feature_mounts.rs` | `MOUNTS` 增 1 项：`id: "serial_debug"`、`plugin: crate::feature_plugins::serial_debug_plugin::init`、`on_setup`、`on_exit` | 表项本身零 cfg；不新建表 |
| `src-tauri/src/cmd/mod.rs` | 增 `#[cfg(feature = "serial_debug")] pub(crate) mod serial_debug;` | 恰 1 条 cfg |
| `src/vite-env.d.ts` | 增 `declare const __SERIAL_DEBUG_ENABLED__: boolean;` | — |
| `src/features/registry.ts` | `if (__SERIAL_DEBUG_ENABLED__) { const { serialDebugModule } = await import("./serial_debug/module"); modules.push(serialDebugModule); }` | 只允许编译期守卫 + 动态 import |
| `src/tools/framework/registry.ts` | 增 `serialDebugTools`（`__SERIAL_DEBUG_ENABLED__ ? [...] : []`）并在 `tools` 数组里展开 | 关闭态该数组为空；`defineTool` 自动分类；`keywords` 只允许 §0.13 白名单（含 `hex` 会被分到 converter 并使 `categories.test.ts` 红） |
| `src/tools/__tests__/categories.test.ts` | `EXPECTED_CATEGORIES` 增 `"serial-debug": "dev"` | 该用例遍历 `tools`，漏加即 `undefined` 断言失败 |
| `src/features/__tests__/coreBoundary.test.ts` | `FEATURE_FOOTPRINT_ALLOWLIST` 增 `serial_debug` 1 条（`src-tauri/src/cmd/mod.rs`，`markers: ['#[cfg(feature = "serial_debug")]', "mod serial_debug;"]`，reason 同 timekeeper）；`FEATURE_PROBE_PREFIX` 增 `serial_debug: "tools"`；`FEATURE_ALIASES` 增 `serial_debug: ["串口"]` | 豁免必须带 reason，粒度是行 |
| `src-tauri/build.rs` | `acl_plugin_name()`：把插件文件名里的 `_` 转 `-` 后再交给 `tauri_build::Attributes::plugin()` | ACL 标识符禁止下划线（0.17）；只影响派生出的插件名，命令前缀仍用下划线模块路径 |
| `.github/workflows/ci.yml` | §6.4 | — |
| `docs/ARCHITECTURE.md` | §4.2 工具清单 23 → 24（dev 类 + 串口调试） | 只改计数与条目 |

**零改动**：`src-tauri/src/plugins.rs`（`mounts()` 循环已通用）、`src-tauri/src/lifecycle.rs`、`src-tauri/src/boot/**`、`src-tauri/src/ipc.rs`、`src-tauri/src/protocol/events.rs`、`src-tauri/src/tray.rs`、`src-tauri/src/window/**`、`src-tauri/src/cmd/config.rs`、`src-tauri/tauri.conf.json`、`src-tauri/capabilities/default.json`、`src/App.tsx`、`src/layouts/AppShell.tsx`。

### 6.3 门禁对应

| 门禁 | 覆盖点 | 本特性如何满足 |
|---|---|---|
| `assembly_contract` A1–A3 | `ipc.rs` 无 cfg、`lib.rs` 仅装配、cfg 位置合法 | cfg 只出现在 `cmd/mod.rs` 与 `feature_plugins/serial_debug_plugin.rs` |
| A4 / B8 | ACL ↔ capabilities 双向一致 | `capabilities/serial_debug.json` 仅授 `serial-debug:default`（连字符名，见 0.17）；`default.json` 不含该权限 |
| B4 | 单特性 cfg ≤ 3 且插件为一对互斥块 | 恰好 3；插件文件含启用/关闭两块 |
| B5 | 关闭块与启用块三函数同签名 | `init/on_setup/on_exit` 两侧逐字一致 |
| B6 | `cmd/mod.rs` 每特性恰 1 条 cfg | 1 条 |
| B7 | 单挂载表、`MOUNTS` 只出现 2 次 | 只加表项 |
| `coreBoundary.test.ts` | 声明↔装配点齐备、core 足迹冻结、动态 import | §6.2 全覆盖 |
| `assert-feature-shed.mjs` | 关闭态 `dep:assistant-serial` 不在 normal-edge 图 | `assistant-serial` 为 optional 且仅被特性启用 |
| `feature-bundle-guard.mjs` | 关闭态模块不入 chunk | 编译期守卫 + 动态 import |
| `assert-feature-absent.mjs` | 关闭态无 `tools.serial_debug.` 字面量 | 文案键全部在 `src/tools/serial_debug` |

### 6.4 CI 三态 lane（改为矩阵，避免特性数线性增长）

- 矩阵键 `none` / `vault` / `timekeeper` / `serial_debug`（`none` 即 slim）。
- 每 lane：`cargo check --no-default-features [--features <id>]` → `cargo test --test assembly_contract ...` → `node ../scripts/assert-feature-shed.mjs [--features <id>]`。
- 前端 slim lane 额外：`$env:SERIAL_DEBUG_ENABLED="0"` 构建 + `node scripts/assert-feature-absent.mjs --feature serial_debug --prefix tools --dist dist`。
- 契约 lane 恒跑一次 `npm run contract:gen` 后 `npm run contract:check`。

### 6.5 i18n 键清单（必须全部出现）

`name, description, port, refresh, noPorts, baudRate, dataBits, parity, stopBits, flowControl, advanced, open, close, statusClosed, statusOpen, receive, send, viewText, viewHex, encoding, encodingUtf8, encodingGbk, encodingLatin1, timestamp, autoscroll, pause, resume, clear, copy, copied, truncated, lineEnding, lineNone, lineCr, lineLf, lineCrlf, repeat, repeatInterval, sendHint, dtr, rts, rxBytes, txBytes, history, historyEmpty, errorNotOpen, errorAlreadyOpen, errorPortNotFound, errorPermissionDenied, errorBusy, errorIo, errorUnsupported, errorInvalidInput, deviceDisconnected`

---

## 7 验收

### 7.1 命令

仓库根：

```
npm run lint
npm run typecheck
npm run test
npm run check
npm run contract:gen; npm run contract:check
$env:SERIAL_DEBUG_ENABLED="0"; npm run build
node scripts/assert-feature-absent.mjs --feature serial_debug --prefix tools --dist dist
```

`npm run check` 不可省：i18n 键集一致性由它调起 `scripts/check-i18n-parity.mjs`，不在 `npm run test` 内（见 §5.5）。

`src-tauri` 目录（不带 `--manifest-path`）：

```
cargo check --no-default-features -p assistant-app
cargo test -p assistant-app --test assembly_contract
cargo test -p assistant-app --test assembly_contract --no-default-features
cargo test -p assistant-app --test assembly_contract --no-default-features --features serial_debug
cargo test -p assistant-serial
node ../scripts/assert-feature-shed.mjs
node ../scripts/assert-feature-shed.mjs --features serial_debug
```

显式带 `-p assistant-app` 是为了与 CI lane 同源：裸 `cargo test --test assembly_contract` 在 workspace 下会按当前目录解析包，行为虽等价但命令行不同源，容易在 CI 上才暴露差异。

### 7.2 判据

| # | 判据 | 方法 |
|---|---|---|
| V1 | 三态装配契约全绿 | 7.1 中三条 `assembly_contract` |
| V2 | slim 关闭态：`serialport`、`assistant-serial` 均不在依赖图 | `cargo tree --no-default-features -e normal` 无 `serialport`；shed 脚本通过 |
| V3 | 单特性独立：`--features serial_debug` 时 `assistant-vault`/`assistant-clock` 不在图 | `cargo tree --no-default-features --features serial_debug -e normal` |
| V4 | 关闭态前端零产物 | 7.1 末两条（探针 + bundle guard） |
| V5 | cfg 预算恰 3 | `assembly_contract` B4 通过；再增 1 条立即红 |
| V6 | 契约无漂移 | `contract:check` 通过 |
| V7 | 领域行为：聚合阈值、代际丢弃、拔线转 closed、写失败转 closed、信号线不支持 | `cargo test -p assistant-serial` |
| V8 | 编解码：base64 往返、HEX 奇偶/非法字符、行尾、UTF-8/GBK/Latin-1 解码、时间戳前缀拼接 | `byteCodec.test.ts` |
| V9 | 人工冒烟（Windows 必做，实物或虚拟串口对）：枚举→打开→收发→HEX/文本切换→编码切换→时间戳列（同批多行时间相同）→复制（内容随开关含/不含时间戳）→循环发送→发送历史回填→关闭→再打开（验证不串上次残留数据） | 手工，记录端口/波特率 |
| V10 | 端口独占：被其它程序占用时返回 `busy`/`permissionDenied`，不崩（**仅 Linux/macOS 断言**；Windows 的 COM 口一般不进程级独占，双开多半成功，故 Windows 只验"不崩"） | 手工 |
| V11 | 退出清理：应用退出后端口立即可被其它工具打开 | 手工 |
| V12 | i18n：无硬编码文案，`zh-CN`/`en-US` 键集一致 | `npm run check`（`scripts/check-i18n-parity.mjs`）——**不能用 `npm run test`**，vitest 里没有这项 |
| V13 | 大流量：115200 持续灌入 30 s，UI 可交互、内存不涨、缓冲裁剪生效 | 手工 |

---

## 8 风险与回滚

| # | 风险 | 处置 |
|---|---|---|
| R1 | cfg 预算顶格（3/3） | 禁止在 `cmd/serial_debug/*.rs` 内写 `#[cfg]`；领域能力靠 `dep:` 自然消失 |
| R2 | ~~`lib.rs` 行数顶格（39 → 40）~~ 已消解（2026-11-26） | 插件声明已收敛：插件文件统一放 `src/feature_plugins/`，`lib.rs` 只留 `mod feature_plugins;`（现 38 行），新增特性只在 `feature_plugins/mod.rs` 加 1 行；`lib.rs` 不再随特性数增长 |
| R3 | Linux 无 libudev → 端口元数据为 `null` | 前端对 `null` 字段降级（只显示端口名）；是否启用见 OQ2 |
| R4 | USB 拔线错误码平台差异 | 统一收敛为 `io`，前端文案写"设备可能已断开" |
| R5 | IPC 洪泛 | 读超时 8 ms + 16 ms/4 KB 聚合 + 前端 rAF 合并；`rx` 事件 `emit_to("main")` 定向主窗口，避免 orb 小窗重复序列化 |
| R6 | 前端渲染卡顿 | 环形缓冲上限 + 暂停只停渲染 |
| R7 | 串口被占用被误判为 bug | 错误码区分 `busy`/`permissionDenied`/`portNotFound`，文案提示"被其它程序占用"；注意 Windows 一般不进程级独占（双开常成功），该文案只在 Linux/macOS 有意义，Windows 失败统一走 `io` 文案"设备可能已断开或被占用" |
| R8 | 照搬参考项目带入烧录依赖 | 只取 monitor 的读循环/无缓冲/信号线思路，不引入 espflash 任何 crate |

回滚：删除 `config/features.json` 条目与 §6.2 全部改动、移除 `assistant-serial` workspace 成员；无后端持久化状态、无数据迁移，回滚无残留（前端 `localStorage` 的 `serial_debug.prefs` 键随工具移除即失效，不影响其它模块）。

---

## 9 实施批次

| 批 | 内容 | 出口条件 |
|---|---|---|
| R1 | 清单 + DTO 契约 + `assistant-serial` 领域层 + 6 命令 + 2 事件 + 全部装配点 + 前端最小可用页（端口列表/打开/关闭/原始文本收发） | `cargo test -p assistant-serial` 与三条 `assembly_contract` 通过；能枚举并打开真实端口 |
| R2 | 前端完整页（HEX/行尾/循环/暂停/缓冲裁剪/编码切换/时间戳列/发送历史/localPrefs/i18n 全键） | V8/V12 通过；V9 人工冒烟通过 |
| R3 | CI 矩阵 lane + 探针接线 + `docs/ARCHITECTURE.md` 工具清单同步 | 7.1 全部通过 |

---

## 10 推迟项

- 多端口并存会话、端口热插拔事件订阅。
- 后端 conf 组持久化（把前端 `localStorage` 的偏好提升为跨设备同步的配置项；届时 Rust struct 与 `*ConfDto` 同步加字段 + `npm run contract:gen`）。
- 日志落盘/导出/回放、自动重连。
- 编码扩展（UTF-16）、协议帧解析、宏指令序列。
- 固件烧录（明确不在本议题范围，也不作为后续延伸）。

---

## 11 开放问题

| # | 问题 | 倾向 |
|---|---|---|
| OQ1 | 默认启用还是默认关闭 | 默认启用（与既有两特性一致，slim 仍可裁） |
| OQ2 | Linux 是否启用 `serialport/libudev`（换取 VID/PID 元数据，代价是 CI 装 `libudev-dev`） | v1 不启用，元数据 `null` 降级 |
| OQ3 | 文本视图编码集合（UTF-8 + Latin-1 是否够） | **已定**：v1 即支持 UTF-8/GBK/Latin-1。三者都在 `TextDecoder` 的原生编码表内，零新增依赖；GBK 对国内教材/单片机中文打印场景收益很大。UTF-16 推迟 |
| OQ4 | 工具 id 用 `serial-debug` 还是 `serial_debug` | `serial-debug`（与既有工具 URL 一致） |
| OQ5 | macOS 是否同时列出 `tty.*` 与 `cu.*` | 都列，打开优先 `cu` |
| OQ6 | 打开后是否默认置 DTR | 默认 `dtr=true`（见 0.15），可手动改 |
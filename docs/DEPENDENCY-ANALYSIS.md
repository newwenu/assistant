# 后端依赖分析与优化建议

> 生成日期：2026-10-24 | Rust 版本：1.93.0 | 项目：Assistant (Tauri 2 Workspace)

---

## 1. 依赖总览

项目采用 Tauri 2 Workspace 架构，主 bin + 9 个 crate，依赖统一在 `[workspace.dependencies]` 声明。

### 1.1 Workspace 依赖分类

#### 序列化 / 数据格式

| 依赖 | 版本 | 用途 | 使用范围 |
|------|------|------|---------|
| serde | 1 (derive) | 序列化/反序列化框架，derive 自动生成实现 | 全部 crate |
| serde_json | 1 | JSON 序列化，Tauri 命令/事件通信格式 | config, storage, providers, tools, agent, sync, app |

#### 日志 / 错误处理

| 依赖 | 版本 | 用途 | 使用范围 |
|------|------|------|---------|
| log | 0.4 | 标准日志门面，配合 tauri-plugin-log 输出 | 全部 crate |
| thiserror | 2 | 派生宏简化自定义错误类型 | providers, agent, storage, security, sync, tools |

#### Tauri 核心 + 插件

| 依赖 | 版本 | 特性 | 用途 |
|------|------|------|------|
| tauri | 2 | tray-icon, macos-private-api, protocol-asset | 桌面应用框架核心 |
| tauri-plugin-opener | 2 | — | 系统默认程序打开文件/URL |
| tauri-plugin-log | 2 | — | 日志写入文件并暴露给前端 |
| tauri-plugin-single-instance | 2 | — | 单实例限制 |
| tauri-plugin-global-shortcut | 2 | — | 全局快捷键注册 |
| tauri-plugin-autostart | 2 | — | 开机自启动 |
| tauri-plugin-dialog | 2 | — | 原生文件选择/消息对话框 |
| tauri-plugin-notification | 2 | — | 系统通知（clock 到点提醒，Rust 侧发起） |

> `tauri-plugin-sql` 已移除（原前端未使用 JS 绑定，Rust 端统一用 rusqlite，避免双份 SQLite 编译）。

#### 网络 / HTTP

| 依赖 | 版本 | 特性 | 用途 | 使用范围 |
|------|------|------|------|---------|
| reqwest | 0.12 | stream, json | HTTP 客户端，调用 LLM API | providers, security, app |
| eventsource-stream | 0.2 | — | SSE 流解析器 | providers（直接依赖，非 workspace） |

#### 异步运行时

| 依赖 | 版本 | 特性 | 用途 | 使用范围 |
|------|------|------|------|---------|
| tokio | 1 | full | 异步运行时 | providers, agent, storage, security, sync, app |
| tokio-util | 0.7 | rt | CancellationToken 等工具 | providers, agent |
| futures | 0.3 | — | Future/Stream 组合子 | providers, agent, app |
| async-trait | 0.1 | — | trait async fn 支持（dyn Trait 兼容） | tools |
| async-stream | 0.3 | — | stream!{} 宏创建 Stream | providers |

> `tokio-stream` 已移除（原仅用 Stream trait re-export，已替换为 `futures::Stream`，同一 trait）。

#### 加密 / 安全

| 依赖 | 版本 | 用途 | 使用范围 |
|------|------|------|---------|
| aes-gcm | 0.10 | AES-GCM 认证加密 | security |
| chacha20poly1305 | 0.10 | ChaCha20-Poly1305 AEAD 加密 | security |
| argon2 | 0.5 | Argon2 密码哈希，密钥派生 | security |
| hkdf | 0.12 | HKDF 密钥派生函数 | security |
| sha2 | 0.10 | SHA-2 哈希，密钥派生/校验/附件指纹 | security, storage, sync |
| sha1 | 0.10 | SHA-1 哈希，TOTP/HIBP k-匿名 | security |
| hmac | 0.12 | HMAC 消息认证码 | security, sync |
| zeroize | 1 (derive, alloc) | 安全内存清零，密钥 Drop 后擦除 | security, sync |
| subtle | 2 | 常量时间比较，防止时序攻击 | security |
| base64 | 0.23 | Base64 编解码 | security, agent, storage, sync |
| data-encoding | 2 | Base32 等编解码（TOTP/恢复密钥） | security |
| hex | 0.4 | 十六进制编解码（SQLCipher 密钥/HIBP） | security |
| rand | 0.8 | 随机数生成，AES-GCM nonce / 密钥生成 | security |
| machine-uid | 0.2 | 机器唯一标识，设备绑定加密密钥 | security |
| zxcvbn | 3 | 密码强度评估 | security |
| keyring | 4 (v1) | OS 密钥链集成（生物识别后备存储） | security (Windows) |

#### 数据库 / 存储

| 依赖 | 版本 | 特性 | 用途 | 使用范围 |
|------|------|------|------|---------|
| rusqlite | 0.31 | bundled-sqlcipher | SQLite 绑定 + SQLCipher 加密，聊天记录/配置/安全库持久化 | storage, security |
| dirs | 7 | — | 系统标准目录路径 | config, app |

#### 时间 / ID

| 依赖 | 版本 | 特性 | 用途 | 使用范围 |
|------|------|------|------|---------|
| chrono | 0.4 | serde | 日期时间，聊天记录时间戳 | storage, security, sync, app |
| uuid | 1 | v4 | 随机 UUID，消息/会话 ID | storage, security, sync, app |

#### 编码检测

| 依赖 | 版本 | 用途 | 使用范围 |
|------|------|------|---------|
| chardetng | 0.1 | 字符编码检测（WHATWG 标准） | tools |
| encoding_rs | 0.8 | 编码转换（Mozilla 开发） | tools |

#### 并发 / 工具

| 依赖 | 版本 | 用途 | 使用范围 |
|------|------|------|---------|
| dashmap | 6 | 并发安全 HashMap，工具注册表 | tools |
| bytes | 1 | 字节缓冲区，reqwest 流式数据单元 | providers |
| urlencoding | 2 | URL 编解码，Gemini API 路径编码 | providers |

#### 剪贴板 / 导入导出

| 依赖 | 版本 | 用途 | 使用范围 |
|------|------|------|---------|
| arboard | 3 | 跨平台剪贴板读写 | security, app |
| csv | 1 | CSV 导入/导出（Vault 条目） | app |

#### 国际化

| 依赖 | 版本 | 用途 | 使用范围 |
|------|------|------|---------|
| rust-i18n | 4 | i18n 宏 + 运行时翻译 | i18n |

#### 系统 / 平台

| 依赖 | 版本 | 用途 | 使用范围 |
|------|------|------|---------|
| mouse_position | 0.1 | 鼠标位置获取，窗口拖拽/定位 | app |
| rustc_version_runtime | 0.3 | 运行时 Rust 编译器版本，诊断日志 | app |
| windows | 0.62 | Win32/WinRT API（仅 Windows） | security (workspace), tools (workspace) |
| windows-future | 0.3 | WinRT async 类型桥接（IAsyncAction/IAsyncOperation → .join()） | security (Windows only) |

> `windows` crate 统一使用 workspace 0.62 版本，与 Tauri 2.12.x 生态对齐（tao/wry/webview2-com 均依赖 windows 0.62），消除双份 `windows-sys`/`windows-core` 编译。`windows-future` 0.3 提供 0.62 中从 `windows::Foundation` 移出的 async 类型。

#### 崩溃处理

| 依赖 | 版本 | 用途 |
|------|------|------|
| crash-handler | 0.6 | 跨平台崩溃信号捕获 |
| minidump-writer | 0.10 | 写入 minidump 文件供离线分析 |

#### 测试

| 依赖 | 版本 | 用途 |
|------|------|------|
| proptest | 1 | 属性测试框架 |
| tempfile | 3 | 临时文件/目录（storage, config, security, sync 测试） |
| minidump | 0.27 | 解析 minidump（dev-only） |
| pdb | 0.8 | 解析 PDB 调试符号（dev-only） |
| rustc-demangle | 0.1 | Rust 符号反混淆（dev-only） |

### 1.2 各 Crate 依赖分布

| Crate | 职责 | 特有依赖 |
|-------|------|---------|
| assistant-config | 配置文件读写 | dirs |
| assistant-storage | 数据持久化层 | rusqlite, uuid, chrono, sha2, base64, thiserror, tokio |
| assistant-providers | LLM API 接入 | reqwest, eventsource-stream, async-stream, rand, bytes, urlencoding, tokio-util, futures, thiserror + assistant-security |
| assistant-agent | Agent 编排逻辑 | tokio-util, futures, thiserror, base64 + providers + storage + i18n |
| assistant-tools | 工具系统 | dashmap, chardetng, encoding_rs, async-trait, windows (workspace 0.62) |
| assistant-orb | Orb 悬浮球功能 | tauri |
| assistant-i18n | 国际化 | rust-i18n |
| assistant-security | 安全子系统（加密/密钥链/生物识别/TOTP/HIBP/审计） | rusqlite, uuid, chrono, aes-gcm, chacha20poly1305, argon2, hkdf, sha2, sha1, hmac, zeroize, subtle, base64, data-encoding, hex, rand, machine-uid, zxcvbn, keyring, arboard, reqwest, thiserror, windows (workspace 0.62), windows-future (0.3) + platform-specific (core-foundation, security-framework, libsecret) |
| assistant-sync | 数据同步（Git/SSH/Vault 文件） | sha2, base64, hmac, zeroize, thiserror, chrono, uuid + assistant-security |
| assistant-app | 应用入口 | 全部 crate + 全部 tauri 插件 + mouse_position + crash-handler + minidump-writer + arboard + csv + dirs + uuid + chrono + reqwest + rusqlite + futures + tokio |

---

## 2. 功能重叠分析

### 2.1 `urlencoding` ↔ `reqwest` — 🟢 场景不同

`urlencoding` 仅在 `gemini/request.rs` 用于编码路径片段中的模型名（`models/gemini-1.5-pro` → `models%2Fgemini-1.5-pro`），reqwest 不会自动编码路径中的 `/`，两者不重叠。

### 2.2 `async-stream` ↔ `futures` — 🟢 互补

`async-stream` 提供 `stream!{}` 声明式创建宏（5 处使用：openai/chat, openai/responses_stream, gemini/request, gemini/code_assist, anthropic/request），`futures` 提供 `StreamExt` 消费组合子（providers 6 处 + agent 2 处 + app 1 处），职责互补。

### 2.3 `rand` ↔ `uuid` — 🟢 底层不同

`uuid` v4 内部用 `getrandom` crate，`rand` 在 `security` 用于 AES-GCM nonce / 密钥生成，底层依赖不同。

### 2.4 `aes-gcm` ↔ `chacha20poly1305` — 🟢 场景不同

`aes-gcm` 用于 API Key 加密（`crypto/api_key.rs`），`chacha20poly1305` 用于 Vault 数据加密（`vault_crypto/mod.rs`），两种 AEAD 算法服务于不同安全层级。

### 2.5 `sha2` ↔ `sha1` — 🟢 场景不同

`sha2`（SHA-256/512）用于密钥派生、附件指纹、HMAC 签名；`sha1`（SHA-1）仅用于 TOTP（`vault_crypto/totp.rs`）和 HIBP k-匿名查询（`vault_crypto/hibp.rs`），两者不可互替。

### 2.6 `base64` ↔ `data-encoding` ↔ `hex` — 🟢 场景不同

| 依赖 | 编码格式 | 使用场景 |
|------|---------|---------|
| base64 | Base64 | API Key 加密存储、多模态图片、SSH 密钥、恢复密钥 |
| data-encoding | Base32 (NOPAD) | TOTP 共享密钥编码、恢复密钥编码 |
| hex | Hex | SQLCipher 密钥传递、HIBP 哈希前缀 |

### 2.7 `hmac` ↔ `sha2` ↔ `sha1` — 🟢 互补

`hmac` 是通用 MAC 框架（`Hmac<D>` 泛型），需要具体哈希算法作为类型参数：

| 组合 | 使用位置 | 用途 |
|------|---------|------|
| `Hmac<Sha256>` | sync/vault_file/mod.rs | Vault 文件 HMAC-SHA256 签名 |
| `Hmac<Sha1>` | security/vault_crypto/totp.rs | TOTP HMAC-SHA1（RFC 6238） |

`hmac` 不自带哈希实现，必须搭配 `sha2` 或 `sha1`，三者互补不可拆。

### 2.8 `zeroize` ↔ `subtle` — 🟢 互补

两者同属密码学安全防护，但职责完全不同：

| 依赖 | 防护目标 | 使用位置 | 机制 |
|------|---------|---------|------|
| zeroize | 内存泄露 | vault_crypto/mod.rs, hibp.rs, security_db/{mod,rekey,biometric_ops}.rs, recovery_key/mod.rs | 密钥/敏感数据 Drop 后内存清零（`Zeroize` trait + derive） |
| subtle | 时序攻击 | vault_crypto/mod.rs | 密文比较使用常量时间算法（`ConstantTimeEq`），防止通过耗时差异推断密文 |

一个防静态泄露（内存残留），一个防动态泄露（时序侧信道），互补不可替代。

### 2.9 `log` ↔ `tauri-plugin-log` — 🟢 门面/后端

`log` 是日志门面 crate（`info!`/`warn!`/`error!` 宏），不写入任何目标；`tauri-plugin-log` 是 `log` 的后端实现，将日志写入文件并暴露给前端 JS。关系等同 `log` ↔ `env_logger`，门面与后端不可缺一。

使用统计：`log::` 引用 32 处（跨全部 crate），`tauri-plugin-log` 仅在 `lib.rs` 注册 1 处。

### 2.10 `tokio` ↔ `futures` — 🟢 互补

两者有少量功能交集（`tokio::select!` vs `futures::select!`），但实际使用完全分离：

| 依赖 | 使用内容 | 使用位置 |
|------|---------|---------|
| tokio | `tokio::select!`（20 处）、`tokio::fs`、`tokio::time`、`tokio::spawn` | providers（SSE 取消+重试）、storage（异步文件 I/O）、app（超时） |
| futures | `futures::Stream`/`StreamExt`（22 处）、`futures::stream` | providers（流消费）、agent（流转发）、app（并发审计） |

`tokio::select!` 用于多分支异步取消（配合 `CancellationToken`），`futures::StreamExt` 用于流组合子链。两者不可互替——`tokio` 不提供 `StreamExt`，`futures` 不提供 `select!`。

### 2.11 `reqwest` ↔ `eventsource-stream` — 🟢 互补

`reqwest` 负责 HTTP 请求/响应（连接、Header、Body），`eventsource-stream` 负责在 `reqwest` 返回的 byte stream 上解析 SSE 协议（`data:`/`event:`/`id:` 行格式）。

调用链：`reqwest::Response → .bytes_stream() → eventsource_stream::Eventsource → futures::Stream<Item=SSEEvent>`

5 处使用完全一致：openai/chat, openai/responses_stream, gemini/request, gemini/code_assist, anthropic/request。两者是上下游关系，非重叠。

### 2.12 `windows` ↔ `tauri` — 🟢 场景不同

两者都能访问 Win32 API，但使用场景不重叠：

| 依赖 | 访问的 API | 使用位置 | 用途 |
|------|-----------|---------|------|
| windows (0.62, workspace) | WinRT (`Security::Credentials::KeyCredentialManager`)、Win32 (`System::WinRT`, `Storage::FileSystem`) | security/biometric/windows.rs, screen_lock/mod.rs, tools/os/fs_link.rs | Windows Hello 生物识别、屏幕锁定检测、符号链接解析 |
| tauri | 窗口管理、事件系统、资源协议、系统托盘 | app 全局 | 桌面应用框架 |

`tauri` 封装的 Win32 API（窗口/托盘/对话框）与 `windows` crate 直接调用的 API（生物识别/文件系统底层）完全不重叠。

> **0.58 → 0.62 升级已完成**：与 Tauri 2.12.x 生态对齐，消除双份 `windows-sys`/`windows-core` 编译。API 迁移要点：
> - `IAsyncAction`/`IAsyncOperation` 从 `windows::Foundation` 移至 `windows_future` crate，`.get()` → `.join()`
> - `CreateWindowExW` 的 `hinstance` 参数改为 `Option<HINSTANCE>`
> - `RegisterPowerSettingNotification` 的 `hrecipient` 参数从 `HWND` 改为 `HANDLE`（`.into()` 转换）

### 2.13 `argon2` ↔ `hkdf` — 🟢 场景不同

两者都做密钥派生，但输入假设和用途不同：

| 依赖 | 输入 | 用途 | 使用位置 |
|------|------|------|---------|
| argon2 | 低熵密码 → 高熵密钥 | 从用户主密码派生加密主密钥（慢哈希，抗暴力） | vault_crypto/mod.rs, recovery_key/mod.rs |
| hkdf | 高熵密钥材料 → 派生密钥 | 从已有密钥派生子密钥（快哈希，确定性） | vault_crypto/mod.rs |

Argon2 是"密码 → 密钥"的慢速桥接，HKDF 是"密钥 → 子密钥"的快速扩展，安全模型不可互换。

---

## 3. 非重叠依赖确认

以下依赖经分析确认无重叠，保留（§2 已详细分析的不再重复）：

| 依赖对 | 关系 | 说明 |
|--------|------|------|
| `tokio` ↔ `tokio-util` | 父子 | 运行时 + 工具（CancellationToken，9 处） |
| `serde` ↔ `serde_json` | 父子 | 框架 + JSON 实现（serde_json 19 文件 100+ 处） |
| `chardetng` ↔ `encoding_rs` | 互补 | 前者检测编码（1 处），后者转换编码（3 处），同文件 process_output.rs |
| `crash-handler` ↔ `minidump-writer` | 互补 | 前者捕获信号，后者写入 dump，同文件 crash_dump.rs |
| `arboard` ↔ `csv` | 场景不同 | 剪贴板读写（security 8 处） vs CSV 导入导出（app 2 处） |
| `keyring` ↔ `machine-uid` | 场景不同 | OS 密钥链存储（biometric/windows.rs 3 处，keyring 4 v1 feature） vs 设备指纹绑定（crypto/api_key.rs 1 处） |
| `zxcvbn` ↔ `argon2` | 互补 | 前者评估密码强度（password_strength/mod.rs），后者执行密码哈希（vault_crypto/mod.rs） |
| `chrono` ↔ `uuid` | 场景不同 | 时间戳（app 4 处 + storage/security/sync） vs 随机 ID（app 5 处 + storage/security/sync），uuid v4 不依赖时间 |
| `lazy_static` ↔ `std::sync::LazyLock` | 已统一 | 项目已全面使用 `LazyLock`/`OnceLock`，`lazy_static` 已移除 |
| `serde_json` ↔ `base64`/`data-encoding`/`hex` | 层级不同 | serde_json 是结构化序列化（JSON 对象），base64/data-encoding/hex 是字节↔文本编码，不同抽象层 |

---

## 4. 附录：依赖使用追踪

### rusqlite（21 文件）

```
assistant-storage (15 文件):
  conversations/{mod,crud,msg_ops,ops,tree_ops,compression,branch}.rs
  attachment_store.rs, agent_modes.rs, content_codec.rs, db.rs, migrations.rs
  providers.rs, tool_favorites.rs, tool_history.rs

assistant-security (6 文件):
  security_db/{mod,crud,crypto_ops,meta,biometric_ops,rekey}.rs
  audit/mod.rs
```

### reqwest（3 文件）

```
assistant-providers/src/{openai,gemini,anthropic}/mod.rs — LLM API HTTP 调用
assistant-security — HIBP 密码泄露检查（via reqwest workspace）
```

### futures（9 文件，22 处）

```
assistant-providers (7 处): base.rs, {openai,gemini,anthropic}/request.rs, openai/{chat,responses_stream}.rs, gemini/code_assist.rs
assistant-agent (2 处): chat_runtime.rs, tests/chat_runtime_spec.rs (10 处 stream::)
app (1 处): cmd/vault/audit.rs
```

### tokio::select!（5 文件，20 处）

```
assistant-providers (20 处): openai/{chat,responses_stream}.rs, gemini/{request,code_assist}.rs, anthropic/request.rs
  每文件 4 处：cancel 分支 + timeout 分支 + 内层 cancel + 内层 timeout
```

### hmac（2 文件）

```
assistant-security/src/vault_crypto/totp.rs    — Hmac<Sha1> (TOTP)
assistant-sync/src/vault_file/mod.rs            — Hmac<Sha256> (Vault 文件签名)
```

### zeroize（7 文件，9 处）

```
assistant-security/src/vault_crypto/{mod,types}.rs, hibp.rs
assistant-security/src/security_db/{mod,rekey,biometric_ops}.rs
assistant-security/src/recovery_key/mod.rs
assistant-security/tests/vault_crypto.rs
```

### subtle（1 文件）

```
assistant-security/src/vault_crypto/mod.rs — ConstantTimeEq (密文比较)
```

### log（32 处）

```
assistant-app (5 处): window.rs, cmd/{orb,config,chat_utils,chat_stream}.rs
assistant-storage (1 处): db.rs
assistant-security (14 处): vault_crypto/{mod,hibp}.rs, ssh_agent/{validate,windows,unix}.rs,
  security_db/{mod,meta}.rs, reveal_guard.rs, recovery_key/mod.rs, crypto/api_key.rs,
  clipboard_guard.rs, biometric/{windows,macos,linux}.rs, audit/mod.rs
assistant-providers (6 处): trace.rs, tool_call_accumulator.rs, stream/stats.rs, sse_log.rs,
  openai/{chat,responses_stream}.rs, gemini/{request,code_assist}.rs, anthropic/request.rs
assistant-orb (1 处): placement.rs
assistant-config (1 处): json_config.rs
```

### serde_json（19 文件，100+ 处）

```
assistant-app: cmd/{os_tool_exec,chat}.rs, protocol/dto/{mod,settings}.rs, cmd/vault/{recovery,entries}.rs
assistant-storage: content_codec.rs, conversations/ops.rs
assistant-security: security_db/{crud,crypto_ops,rekey}.rs
assistant-providers: types/{message,config}.rs, trace.rs, tool_call_accumulator.rs,
  openai/{types,responses_types,message_converter,discovery,chat,responses_stream}.rs,
  gemini/{types,stream,message_converter,discovery,request,code_assist}.rs,
  anthropic/message_converter.rs, client/error.rs
assistant-sync: vault_file/mod.rs, sync_engine/mod.rs
```

### base64（18 处）

```
assistant-security (14 处): vault_crypto/mod.rs, crypto/api_key.rs, security_db/{meta,biometric_ops}.rs,
  recovery_key/mod.rs, ssh_agent/validate.rs, biometric/windows.rs, clipboard_guard.rs
assistant-security/tests/vault_crypto.rs (2 处)
assistant-agent (2 处): multimodal.rs
assistant-sync (2 处): vault_file/mod.rs
```

### windows（2 crate，16 处，统一 workspace 0.62）

```
assistant-security (11 处):
  biometric/windows.rs — WinRT: KeyCredentialManager, CryptographicBuffer, IAsyncAction
  screen_lock/mod.rs — Win32: SystemParametersInfo, SetWindowPos

assistant-tools (5 处):
  os/fs_link.rs — Win32: CreateFileW, GetFinalPathNameByHandleW, ReparseDataBuffer
```

---

## 5. 优化结论

| 依赖 | 原问题 | 结论 | 要点 |
|------|--------|------|------|
| tauri-plugin-sql | 与 rusqlite 双份 SQLite 编译 | ✅ 已移除 | 前端无 JS 绑定，Rust 端统一用 rusqlite（bundled-sqlcipher） |
| tokio-stream | 与 futures 重复 Stream trait | ✅ 已移除 | 唯一使用点改为 `use futures::Stream`，同一 trait（futures-core） |
| async-trait | Rust 1.75+ 原生 async fn in trait | 🔒 保留 | `Tool` trait 经 `Arc<dyn Tool>` 作 trait object，原生 RPITIT 返回 `impl Future` 无法构建 vtable；待 `#[dyn_compat]` 稳定后可移除 |
| lazy_static | 项目已全面使用 `std::sync::LazyLock`/`OnceLock` | ✅ 已移除 | 仅 sync/git_sync/ssh.rs 1 处，改写为 `LazyLock`；消除 1 个外部依赖 |
| windows 双版本 | workspace 0.58 + tools 直依赖 0.61 | ✅ 已统一 | 升级至 0.62 与 Tauri 生态对齐，消除双份 windows-sys 编译；API 迁移：IAsyncAction→windows_future .join()、CreateWindowExW Option<HINSTANCE>、RegisterPowerSettingNotification HWND→HANDLE |
| eventsource-stream | 非 workspace 声明 | ✅ 已规范化 | 移入 `[workspace.dependencies]`，providers 改为 `{ workspace = true }` |
| keyring 3→4 | security-framework 双版本编译 | ✅ 已升级 | `keyring = { version = "4", features = ["v1"] }`，v1 API 100% 兼容 3.x，代码零修改；消除 macOS security-framework + core-foundation 双份编译 |
| base64 0.22→0.23 | 版本对齐 | ✅ 已升级 | 已用 Engine trait API，代码零修改 |
| dirs 6→7 | 版本对齐 | ✅ 已升级 | API 兼容，代码零修改 |
| tokio 未使用依赖 | 3 个 crate 声明但未使用 | ✅ 已移除 | assistant-tools/security/sync 移除直接 tokio 依赖 |
| Workspace 卫生 | 9 处未通过 workspace 引用 | ✅ 已修复 | dashmap/tempfile/平台依赖等改为 `{ workspace = true }` |

### 5.1 `lazy_static` → `std::sync::LazyLock` — ✅ 已完成

**已执行修改**：

| # | 文件 | 修改 |
|---|------|------|
| 1 | `crates/assistant-sync/Cargo.toml` | 删除 `lazy_static = "1"` |
| 2 | `crates/assistant-sync/src/git_sync/ssh.rs` | `use std::sync::Mutex` → `use std::sync::{LazyLock, Mutex}`；`lazy_static!{}` → `LazyLock::new()` |

### 5.2 `windows` 双版本统一 — ✅ 已完成

**已执行修改**：

| # | 文件 | 修改 |
|---|------|------|
| 1 | `src-tauri/Cargo.toml` | workspace `windows` 版本 0.58 → 0.62，特性列表追加 `Win32_Security`、`Win32_Storage_FileSystem` |
| 2 | `crates/assistant-tools/Cargo.toml` | `windows = { version = "0.61", features = [...] }` → `windows = { workspace = true }` |
| 3 | `crates/assistant-security/Cargo.toml` | `[target.'cfg(target_os = "windows")'.dependencies]` 新增 `windows-future = "0.3"` |
| 4 | `crates/assistant-security/src/biometric/windows.rs` | `windows::Foundation::{IAsyncAction, IAsyncOperation}` → `windows_future::{IAsyncAction, IAsyncOperation}`；`.get()` → `.join()`；移除 `pump_messages()` 死代码 |
| 5 | `crates/assistant-security/src/screen_lock/mod.rs` | `CreateWindowExW` 的 `hinstance` → `Some(hinstance)`；`RegisterPowerSettingNotification` 的 `hwnd` → `hwnd.into()` |

### 5.3 `eventsource-stream` 规范化 — ✅ 已完成

**已执行修改**：

| # | 文件 | 修改 |
|---|------|------|
| 1 | `src-tauri/Cargo.toml` | `[workspace.dependencies]` 新增 `eventsource-stream = "0.2"` |
| 2 | `crates/assistant-providers/Cargo.toml` | `eventsource-stream = "0.2"` → `eventsource-stream = { workspace = true }` |
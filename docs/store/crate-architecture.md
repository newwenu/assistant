# 后端 Crate 架构现状

> 更新日期：2026-10-30
> 数据口径：`src-tauri` workspace 实测（`Cargo.toml` + 源码行数 + `cargo tree`）
> 关联文档：`docs/store/compile-time-attribution.md`（编译耗时归属实测）
> 历史决策与施工记录：crate-architecture-analysis（已归档，不再维护）

---

## 0. 数据口径

| 项 | 口径 |
|---|---|
| 行数 | 该 crate 内 `*.rs` 的有效行数（含空行与注释，含 crate 根目录的 `build.rs`，不含 `tests/`、不含 `target/` 产物） |
| 直接依赖数 | 该 crate `Cargo.toml` 中 `[dependencies]` 段的条目数（不含 dev / build / target 平台专属段） |
| 编译耗时 | 三次 `cargo clean + cargo build --timings` 的**中位数**，详见 `docs/store/compile-time-attribution.md` |
| 依赖边 | `Cargo.toml` 中显式声明的 workspace 内部 crate 依赖；不推断传递依赖 |

本文只描述**当前事实**，不包含"目标架构""执行顺序"等规划内容。规划类内容一律写入 `docs/store/` 下的专项文档。

---

## 1. 总览

### 1.1 规模

| 组成部分 | 文件数 | 行数 | 说明 |
|---|---|---|---|
| 主 app（`src/`） | 39 | 6,702 | Tauri 命令层、协议层、初始化 |
| 12 个子 crate | 198 | 31,201 | 业务与基础设施（含 3 个 `build.rs`） |
| **合计** | **237** | **37,903** | |

### 1.2 workspace 成员（12 个）

```
crates/assistant-config
crates/assistant-orb
crates/assistant-providers
crates/assistant-agent
crates/assistant-storage
crates/assistant-i18n
crates/assistant-tools
crates/assistant-crypto
crates/assistant-security
crates/assistant-sync
crates/assistant-vault
crates/assistant-crash
```

主 app 自身为 workspace 根包 `assistant-app`，依赖全部 12 个成员。

### 1.3 依赖图

```
assistant-app  (根包, src/)
├── assistant-config        无内部依赖
├── assistant-i18n          无内部依赖
├── assistant-crypto        无内部依赖
├── assistant-orb           无内部依赖（唯一依赖 tauri 的子 crate）
├── assistant-tools         无内部依赖
├── assistant-storage       无内部依赖
├── assistant-crash         无内部依赖
├── assistant-providers     无内部依赖
├── assistant-sync          无内部依赖
├── assistant-security  ──→ assistant-crypto
├── assistant-agent     ──→ assistant-providers, assistant-storage
└── assistant-vault     ──→ assistant-crypto, assistant-sync
```

内部依赖边共 **5 条**：

| 边 | 性质 |
|---|---|
| `assistant-security → assistant-crypto` | security 复用 crypto 的加密原语 |
| `assistant-vault → assistant-crypto` | vault 复用 crypto 的加密原语 |
| `assistant-vault → assistant-sync` | vault 用 sync 做 git 同步 |
| `assistant-agent → assistant-providers` | agent 调用 provider 发起请求 |
| `assistant-agent → assistant-storage` | agent 读写会话与工具历史 |

其余 9 个 crate（config / i18n / crypto / orb / tools / storage / crash / providers / sync）均为叶子节点，无内部依赖。**没有环，也没有跨层回指。**

### 1.4 分层视图

| 层 | crate | 特征 |
|---|---|---|
| 基础设施层 | config / i18n / crypto / storage / crash / tools | 无业务语义，可被任意上层复用 |
| 能力层 | providers / orb / sync | 单一外部能力封装 |
| 领域层 | agent / security / vault | 含业务规则，内部可再分模块 |
| 应用层 | assistant-app | 仅做 IPC、编排、状态装配 |

---

## 2. 逐 crate 现状

### 2.1 assistant-config

| 项 | 值 |
|---|---|
| 行数 / 文件 | 1,354 / 14 |
| 直接依赖 | 4（serde、serde_json、log、dirs） |
| 内部依赖 | 无 |
| 模块 | `app_conf` `orb_conf` `mascot_conf` `vault_conf` `atomic_write` `json_config` `constant` |

纯 JSON 配置读写，不依赖 tauri，也不依赖任何业务 crate。`vault_conf` 模块只负责读写 vault 配置结构，**不引用 vault 的任何代码**，因此配置层与 vault 功能本身是零耦合的。

### 2.2 assistant-i18n

| 项 | 值 |
|---|---|
| 行数 / 文件 | 202 / 3 |
| 直接依赖 | 2（serde、serde_json） |
| 内部依赖 | 无 |
| build.rs | 有（14 行，生成语言目录 revision 常量） |
| 导出 | 无 `pub mod`，纯函数 + 常量（`DEFAULT_LOCALE`、`SHIPPED_LOCALES`、`text`、`t`、`t_with_args`、`format_text`、`set_locale`、`current_locale`、`locale_code`、`init_global`） |

`zh-CN.json` / `en-US.json` 通过 `include_str!` 在编译期嵌入，运行时零 IO。

### 2.3 assistant-crypto

| 项 | 值 |
|---|---|
| 行数 / 文件 | 956 / 9 |
| 直接依赖 | 16 |
| 内部依赖 | 无 |
| 模块 | `api_key` `error` `vault_crypto` |

依赖构成：`serde` `log` `thiserror` `aes-gcm` `chacha20poly1305` `argon2` `hkdf` `zeroize` `subtle` `base64` `data-encoding` `rand` `sha2` `machine-uid` `hmac` `sha1`。

**这是全库唯一被两个业务方向共用的加密 crate**，但两个模块的依赖需求并不相同：

| 模块 | 消费者 | 需要的依赖 |
|---|---|---|
| `api_key` | 主 app（provider / compaction / chat） | `aes-gcm` `base64` `rand` `sha2` `machine-uid` |
| `vault_crypto` | `assistant-vault` | `argon2` `hkdf` `chacha20poly1305` `subtle` `data-encoding` `hmac` `sha1` `zeroize` |

当前两者混在同一个 crate 的默认 feature 里，因此主 app 即使完全不碰 vault，也会被迫编译 `argon2` / `hkdf` / `chacha20poly1305` 等约 10s 的依赖。这是**当前唯一一处已识别的、可通过 crate 内 feature 切分解决的浪费**（详见 `docs/store/compile-time-attribution.md` §7）。

### 2.4 assistant-storage

| 项 | 值 |
|---|---|
| 行数 / 文件 | 3,046 / 24 |
| 直接依赖 | 10（含 `rusqlite`、`tokio`） |
| 内部依赖 | 无 |
| build.rs | 有（8 行，Windows 链接 crypt32/advapi32/ws2_32/user32） |
| 模块 | `db` `conversations` `migrations` `providers` `attachment_store` `content_codec` `tool_history` `tool_favorites` `agent_modes` `error` |

会话、附件、工具历史、agent 模式的 SQLite 持久化。`rusqlite` 走 `bundled-sqlcipher`（`workspace.dependencies` 中声明 `features = ["bundled-sqlcipher"]`）。

**重要事实**：主 app 自身也直接依赖 `rusqlite`（`src-tauri/src/setup.rs` 的 `DbState` 持有 `rusqlite::Connection`，`src-tauri/src/cmd/chat/conversation.rs` / `storage.rs` / `conversation_utils.rs` 直接使用 `rusqlite::params!`）。因此 `rusqlite` + `libsqlite3-sys`（合计约 10.3s）是**主功能固定成本，排除 vault 也无法省掉**。

### 2.5 assistant-providers

| 项 | 值 |
|---|---|
| 行数 / 文件 | 8,573 / 44 |
| 直接依赖 | 12（`reqwest` `tokio` `futures` `async-stream` `bytes` `eventsource-stream` 等） |
| 内部依赖 | 无 |
| 模块 | `openai` `anthropic` `gemini` `base` `client` `stream` `sse_log` `types` `vision` `trace` `diagnostics` `model_catalog` `tool_call_accumulator` |

行数第二、文件数第二的 crate（8.5k 行、44 文件，仅次于 `assistant-vault`）。承载三家 LLM 的请求构造、SSE 流解析、工具调用累积、视觉输入。无内部依赖，是纯能力层。

### 2.6 assistant-agent

| 项 | 值 |
|---|---|
| 行数 / 文件 | 2,192 / 13 |
| 直接依赖 | 10（含 `image`） |
| 内部依赖 | `assistant-providers`、`assistant-storage` |
| 模块 | `chat_request` `chat_runtime` `chat_tasks` `compaction` `normalization` `prompt_builder` `provider_config` `request_projection` `multimodal` `error` |

会话编排层：把用户输入 + 历史 + 工具结果组装成 provider 请求，并把结果落回 storage。`image`（9.8s）与 `moxcms` / `pfxm` / `png` / `tiff` 等图像栈由该 crate 的 `multimodal` 引入。

### 2.7 assistant-tools

| 项 | 值 |
|---|---|
| 行数 / 文件 | 702 / 8 |
| 直接依赖 | 6（`async-trait` `dashmap` `chardetng` `encoding_rs` 等） |
| 内部依赖 | 无 |
| 模块 | `registry` `context` `result` `os` `r#trait` |
| 平台依赖 | `windows`（`Win32::Storage::FileSystem` 用于 `fs_link.rs` 的路径与句柄校验） |

工具注册表与平台适配（文件链接、编码探测）。**这是唯一使用 `windows` crate 且属于主功能的子 crate**，其所需 feature 仅 `Win32_Foundation` + `Win32_Storage_FileSystem`。

### 2.8 assistant-orb

| 项 | 值 |
|---|---|
| 行数 / 文件 | 759 / 6 |
| 直接依赖 | 2（`log`、`tauri`） |
| 内部依赖 | 无 |
| 模块 | `geometry` `hover` `placement` |

**唯一直接依赖 `tauri` 的子 crate**。精确耦合点只有 `src-tauri/crates/assistant-orb/src/placement.rs` 的 `get_available_screens(app: &AppHandle)` 一个函数（取可用屏幕几何）；`geometry.rs`、`hover.rs` 均为纯 Rust，且已有 `tests/placement.rs`、`tests/hover.rs` 覆盖这些纯计算路径。因此"依赖 tauri 导致无法测试"不成立，要去掉 tauri 依赖只需把该函数上移到主 app 的 `cmd/orb.rs`。

### 2.9 assistant-crash

| 项 | 值 |
|---|---|
| 行数 / 文件 | 89 / 2 |
| 直接依赖 | 4（`crash-handler` `minidump-writer` `dirs` `chrono`） |
| 内部依赖 | 无 |
| 模块 | 无 `pub mod`，私有 `mod crash_dump` + `pub use crash_dump::install` |

最小 crate（89 行）。`minidump-common`（7.6s）由 `minidump-writer` 引入。

### 2.10 assistant-sync

| 项 | 值 |
|---|---|
| 行数 / 文件 | 1,432 / 8 |
| 直接依赖 | 3（`serde` `log` `thiserror`） |
| 内部依赖 | 无 |
| 模块 | `git_sync` `types` |

vault 的 git 同步实现。**唯一消费者是 `assistant-vault`**，主 app 对它的直接依赖（`Cargo.toml` 中声明）仅用于在 `cmd/vault/*` 里访问，因此它与 vault 属同一排除单元。

### 2.11 assistant-security

| 项 | 值 |
|---|---|
| 行数 / 文件 | 2,753 / 20 |
| 直接依赖 | 10（`assistant-crypto` `arboard` `ssh-key` `rsa` `reqwest` `sha1` `hex` `zeroize` `serde` `log`） |
| 内部依赖 | `assistant-crypto` |
| 模块 | `clipboard_guard` `reveal_guard` `screen_lock` `ssh_agent` `unlock_dialog` `hibp` `vault_crypto` |
| 平台依赖 | `windows`（GDI 自绘、DWM、Registry、RemoteDesktop、Power、WinRT 初始化） |

vault 的安全外围：剪贴板守卫、防截屏、锁屏/休眠监听、SSH agent 桥接、自绘解锁弹窗、HIBP 泄露检测、`vault_crypto` 再导出桩。

**关键事实（2026-10-30 已修正）**：`assistant-security` **不被 `assistant-vault` 依赖**，它是主 app 直接依赖的并列 crate。二者的耦合发生在代码层——`cmd/vault/*` 调用 security 的模块。经 L1 解耦后，主功能（provider / chat / compaction）已完全改为直接使用 `assistant_crypto::api_key`，**不再有任何主功能代码引用 `assistant-security`**。

因此：security 的依赖声明仍留在主 app，但实际消费者只剩 `cmd/vault/*`。这是"声明与使用分离"的典型样本，也是后续可排除编译时必须以 `vault + sync + security` 三件套为单位排除的原因。

### 2.12 assistant-vault

| 项 | 值 |
|---|---|
| 行数 / 文件 | 9,143 / 47 |
| 直接依赖 | 21（20 个 workspace + `ts-rs = "12"`） |
| 内部依赖 | `assistant-crypto`、`assistant-sync` |
| build.rs | 有（9 行，Windows 链接 crypt32/advapi32/ws2_32/user32） |
| 模块 | `security_db` `vault_file` `audit` `biometric` `password_strength` `recovery_key` `dto` `types` `shared` `sync_engine` |
| feature | `slow-tests`、`git-tests`（仅测试用） |
| 平台依赖 | Windows：`windows` + `windows-future`；macOS：`core-foundation` + `security-framework`；Linux：`libsecret` |

全库第二大 crate（9.1k 行、47 文件）。重依赖集中地：`zxcvbn`（5.0s）、`argon2`（2.1s）、`keyring`（0.3s）、`rusqlite`（共享）、`ts-rs` + `ts-rs-macros`（6.1s，用于 DTO 类型生成）。

三个平台专属依赖段是 vault 独有的：主 app 不声明 `core-foundation` / `security-framework` / `libsecret`。

---

## 3. 横向事实汇总

### 3.1 规模排序

| 排名 | crate | 行数 | 文件数 |
|---|---|---|---|
| 1 | assistant-vault | 9,143 | 47 |
| 2 | assistant-providers | 8,573 | 44 |
| 3 | assistant-storage | 3,046 | 24 |
| 4 | assistant-security | 2,753 | 20 |
| 5 | assistant-agent | 2,192 | 13 |
| 6 | assistant-sync | 1,432 | 8 |
| 7 | assistant-config | 1,354 | 14 |
| 8 | assistant-crypto | 956 | 9 |
| 9 | assistant-orb | 759 | 6 |
| 10 | assistant-tools | 702 | 8 |
| 11 | assistant-i18n | 202 | 3 |
| 12 | assistant-crash | 89 | 2 |

### 3.2 编译耗时（中位数）

| crate | 自身耗时 | 占 Σ 1088.3s |
|---|---|---|
| assistant-app | 20.7s | 1.9% |
| assistant-providers | 12.0s | 1.1% |
| assistant-storage | 7.8s | 0.7% |
| assistant-crypto | 6.3s | 0.6% |
| assistant-vault | 6.1s | 0.6% |
| assistant-config | 3.6s | 0.3% |
| assistant-i18n | 3.4s | 0.3% |
| assistant-sync | 3.3s | 0.3% |
| assistant-agent | 2.9s | 0.3% |
| assistant-crash | 2.0s | 0.2% |
| assistant-security | 1.2s | 0.1% |
| assistant-orb | 1.1s | 0.1% |
| assistant-tools | 0.8s | 0.1% |
| **合计** | **71.2s** | **6.5%** |

**workspace 自身只占 6.5%，93.5% 的编译成本来自第三方依赖。** 这是判断"拆 crate / 排除 crate"是否值得的核心前提：动 crate 结构本身收益很小，收益必须来自依赖面的增减。

### 3.3 build.rs 分布（4 个）

| 位置 | 行数 | 作用 |
|---|---|---|
| `build.rs`（app） | 3 | `tauri_build::build()` |
| `crates/assistant-i18n/build.rs` | 14 | 生成语言目录 revision |
| `crates/assistant-storage/build.rs` | 8 | Windows 链接 `crypt32` `advapi32` `ws2_32` `user32` |
| `crates/assistant-vault/build.rs` | 9 | 同上 |

storage 与 vault 的 build.rs 内容一致，都服务于 `rusqlite`（SQLCipher）在 Windows 上的链接需求。**规则：谁依赖 `rusqlite`（SQLCipher），谁必须自带这份 `rustc-link-lib` 声明**——因为 `rustc-link-lib` 只沿依赖链单向传播，不能指望兄弟 crate 代为链接。

排除 vault 时，`crates/assistant-vault/build.rs` 随之消失；`crates/assistant-storage/build.rs` 必须保留（storage 仍在）。

### 3.4 平台专属依赖归属

| 平台 | 依赖 | 声明方 |
|---|---|---|
| Windows | `windows` | workspace 共享声明；被 `assistant-vault`、`assistant-security`、`assistant-tools` 三处 `workspace = true` 消费 |
| Windows | `windows-future` | 仅 `assistant-vault` |
| macOS | `core-foundation`、`security-framework` | 仅 `assistant-vault` |
| Linux | `libsecret` | 仅 `assistant-vault` |

`windows` 共声明 18 个 feature。按源码实际引用归口：

| 服务对象 | feature 数 | feature 列表 |
|---|---|---|
| 主功能（`assistant-tools` 的 `fs_link`） | 2 | `Win32_Foundation`、`Win32_Storage_FileSystem` |
| vault（Windows Hello 生物识别） | 4 | `Security_Credentials`、`Security_Cryptography`、`Storage_Streams`、`Win32_System_WinRT` |
| security（解锁弹窗 / 锁屏 / SSH agent） | 11 | `Win32_Security`、`Win32_System_LibraryLoader`、`Win32_System_Registry`、`Win32_System_RemoteDesktop`、`Win32_System_Power`、`Win32_Graphics_Dwm`、`Win32_Graphics_Gdi`、`Win32_UI_WindowsAndMessaging`、`Win32_UI_Input_KeyboardAndMouse`、`Win32_UI_Controls`、`Win32_UI_HiDpi` |
| 源码中未检索到直接引用 | 1 | `Foundation`（无 `Win32_` 前缀的顶层命名空间） |

即：**18 个 feature 里只有 2 个是主功能必需的，其余 15 个服务于 vault + security，另有 1 个疑似冗余。**

机制上，`features` 写在 `[workspace.dependencies]` 里，成员只能用 `workspace = true` 取**并集**，无法从成员侧削减。因此 `assistant-tools` 被迫接受 vault / security 的全部 feature。可选的整理方式是把 feature 从 workspace 段移除、由各成员自行声明（旧文档 §5.3 已论证：该改动只对单 crate 构建有效，完整构建因 feature 统一取并集而零提速）。

但 `windows` crate 本身无法从依赖图中移除——Tauri 运行时栈（`tauri-runtime-wry` / `wry` / `webview2-com-sys` / `tao` / `muda` / `tray-icon`）独立声明了自己的 windows feature 集合，仍需完整编译。排除 vault 能减少的是**本次构建启用的 feature 面**，不是整个 `windows` 包。

### 3.5 依赖闭包与可省范围

| 集合 | 包数 | 说明 |
|---|---|---|
| `assistant-vault` 依赖闭包 | 126 | 含 crypto、sync 及共享依赖 |
| `vault + sync + security` 联合闭包 | 246 | 潜在排除集合 |
| 其中主功能同样需要（共享） | 198 | 无法省 |
| 真正可省（专属） | **48** | 串行 78.2s，占 Σ 的 **7.2%** |

结论：**vault 闭包看着大（126 包），实际可省的只有 48 包**。因为 `windows`（54.3s）、`tauri-utils`（50.9s）、`windows-sys`（37.7s）、`tokio`（15.5s）、`rusqlite`+`libsqlite3-sys`（10.3s）这些大头同时被主功能需要。完整清单见 `docs/store/compile-time-attribution.md` §6.3。

---

## 4. 当前架构的已知问题

按影响排序，均为**事实描述**，不含解决方案。

### 4.1 `assistant-security` 声明与使用分离

主 app 在 `[dependencies]` 中声明 `assistant-security`，但主功能代码已零引用（L1 解耦后仅 `cmd/vault/*` 使用）。这导致：

- 无法通过"删除依赖声明"判断该 crate 是否还需要；
- 任何排除 vault 的方案必须显式处理它，否则会留下零消费者却仍在编译的死重。

### 4.2 `assistant-crypto` 两个方向的依赖混装

`api_key`（主功能）与 `vault_crypto`（vault）共用默认 feature，主功能被迫编译 `argon2` / `hkdf` / `chacha20poly1305` / `data-encoding` 等约 10s 的依赖。当前 crate 内没有 `[features]` 切分。

### 4.3 `assistant-vault` 与 `assistant-security` 是代码层耦合而非 crate 依赖

二者的调用关系只存在于 `cmd/vault/*`，crate 依赖图上 security 是主 app 的并列下游。这意味着：

- 依赖图无法反映真实耦合强度；
- 排除决策必须同时参考**代码层引用**（`cargo tree` 不够）；
- 任何"只排 vault 不排 security"的做法都会得到错误结论。

### 4.4 `assistant-sync` 的消费者单一

`assistant-sync` 仅被 `assistant-vault` 依赖，且主 app 对它的直接声明也只为 `cmd/vault/*` 服务。它本质上属于 vault 的组成部分，当前作为独立 crate 存在只是因为历史上先拆出来了。

### 4.5 主 app 直接持有 `rusqlite`

`src-tauri/src/setup.rs` 的 `DbState` 与 `src-tauri/src/cmd/` 下的 `conversation.rs`、`storage.rs`、`conversation_utils.rs` 直接使用 `rusqlite`，而 `assistant-storage` 也依赖 `rusqlite`。这意味着 SQLCipher 的链接成本（`rusqlite` 3.4s + `libsqlite3-sys` 6.9s + 两份 build.rs）由两个位置共同承担，且无法通过排除任一业务 crate 消除。

### 4.6 `assistant-orb` 依赖 `tauri`

12 个子 crate 中只有 orb 直接依赖 `tauri`，但耦合面仅 `placement.rs` 的一个取屏幕函数；纯计算部分已有测试覆盖，因此"可脱 tauri 测试"这一判据实际是**满足**的，不属于阻塞项。

---

## 5. 判据：何时该拆出新 crate

沿用存档文档 crate-architecture-analysis（已归档）§4.3 的决策检查表，口径不变：**命中任一条件才值得独立成 crate；全部不命中则留在主 app**。示例按 2026-10-30 实测更新。

| # | 条件 | 判据 | 当前实测示例 |
|---|---|---|---|
| ① | 多消费者 | 被 ≥2 个上游 crate 引用 | `assistant-crypto`（3：app / security / vault）、`assistant-storage`（2）、`assistant-providers`（2）、`assistant-sync`（2：app / vault） |
| ② | 可脱 tauri 独立测试 | 核心逻辑不依赖 `AppHandle` / 窗口系统，且可测逻辑 >100 行 | `assistant-crash`（纯文件 IO）、`assistant-orb`（纯几何部分已有 `tests/`）、`assistant-config`、`assistant-i18n` |
| ③ | 编译隔离 | 拉入重依赖（单项编译 >3s），需 feature gate 让开发时跳过 | `assistant-crash`（`minidump-common` 7.6s）、`assistant-vault`（`zxcvbn` 5.0s、`ts-rs` 6.1s） |
| ④ | 独立领域 + 体量 | 自有类型体系与错误处理，且 >300 行 | `vault`(9.1k) `providers`(8.6k) `storage`(3.0k) `security`(2.8k) `agent`(2.2k) `sync`(1.4k) `config`(1.4k) `crypto`(1.0k) `orb`(0.8k) `tools`(0.7k) |
| ⑤ | 以上均不命中 | 留在主 app：tauri 胶水 / 单消费者 / 轻依赖 / 体量小 | `src-tauri/src/cmd/mascot_asset.rs`（138 行，两个 `#[command]` 均持有 `AppHandle`） |

> **对旧文档示例的两处修正**：① 旧文把 `assistant-config` 列为"多消费者"示例，实测 `cargo tree -i assistant-config` 只有 `assistant-app` 一个上游，其独立依据应记为 ②+④；② 旧文 `security` 的 ③ 示例写"zxcvbn 9.7s"，但 zxcvbn 已随拆分迁入 vault，security 现自身编译 1.2s，其 ③ 依据应改记为"独占 20+ 外部依赖（arboard / ssh-key / rsa / reqwest 等），无第二消费者"。

**关键区分（沿用旧结论）**：「不能放进某个 crate」≠「必须新建 crate」。判断前先问：**该模块是否本质上是 tauri 框架胶水？** 若是，主 app 就是它的正确归属；独立只会把 tauri 重依赖搬进新 crate。

### 5.1 主 app 职责下沉：旧结论与当前状态

旧文档 §3 给出的下沉清单，按当前代码逐项核对（行数为实测）：

| 项 | 旧结论 | 当前状态 |
|---|---|---|
| 崩溃转储 `crash_dump.rs`（87 行） | 下放为 `assistant-crash` | ✅ 已落地，见 §2.9 |
| 存储分析逻辑 `cmd/storage.rs`（159 行） | → `assistant-storage` | ⬜ 未做。仍需先剥离 `AppHandle` 胶水，核心为 DB + `attachment_store` 调用 |
| 会话工具函数 `cmd/conversation_utils.rs`（38 行） | → `assistant-storage` | ⬜ 未做。纯 DB 查询（2 处 `rusqlite` 引用），返回 storage 类型 |
| Chat 消息插入 `cmd/chat/insert.rs`（260 行） | → `assistant-agent` | ⬜ 未做。前置条件仍在：5 处 `assistant_i18n::t()` 需先改为结构化错误 |
| DTO / 协议类型 `protocol/dto/*.rs`（7 文件 1,085 行） | 有条件抽 `assistant-protocol` | ⬜ 未做。前提是补齐非 vault 的直出类型 DTO 并引入 `ts-rs` / `typeshare`，否则只是行数搬家、零复用 |
| API Key 掩码 `cmd/provider.rs`（188 行） | ❌ 留主 app | ❌ 结论不变，但**依据已失效**：L1 解耦后主 app 不再引用 `assistant-security`，改为直接调用 `assistant_crypto::api_key`。搬运仍无收益（单消费者，且 crypto 已是叶节点） |
| Vault 编排辅助 `cmd/vault/mod.rs`（272 行） | ❌ 不搬运 | ❌ 不变。`AppConf` 读取 + `SshAgentState` 状态 + 领域调用的编排胶水，搬入会让 security 反向依赖 config 与 tauri |
| `mascot_asset`（138 行） | ❌ 不抽 crate（命中 ⑤） | ❌ 不变 |

> DTO 的现状补充：I23 已完成 **vault 侧** IPC 契约收口（`ts-rs` 自动生成 + DTO 集中转换 + 门禁），因此 vault 的 wire 类型不再直出。但 sync / security / crypto 侧的直出类型仍未补 DTO，"抽 `assistant-protocol`"的前提条件依然不成立。

---

## 6. 复现命令

```powershell
# workspace 成员
Select-String -Path 'Cargo.toml' -Pattern 'crates/'

# 各 crate 行数 / 文件数
Get-ChildItem 'crates' -Directory | ForEach-Object {
  $rs = Get-ChildItem $_.FullName -Recurse -Include *.rs -File | Where-Object { $_.FullName -notmatch '\\target\\' }
  "{0}|files={1}|lines={2}" -f $_.Name, $rs.Count, (($rs | ForEach-Object { (Get-Content $_.FullName | Measure-Object -Line).Lines } | Measure-Object -Sum).Sum)
}

# 内部依赖边
Get-ChildItem 'crates' -Directory | ForEach-Object {
  $c = Get-Content (Join-Path $_.FullName 'Cargo.toml') -Raw
  "{0}|->{1}" -f $_.Name, (([regex]::Matches($c, '(?m)^(assistant-[a-z0-9]+)\s*=') | ForEach-Object { $_.Groups[1].Value }) -join ',')
}

# build.rs 分布
Get-ChildItem . -Recurse -Include build.rs -File | Where-Object { $_.FullName -notmatch '\\target\\' }

# 依赖闭包
cargo tree -p assistant-vault --prefix none --no-dedupe
cargo tree -i assistant-security --prefix none
```
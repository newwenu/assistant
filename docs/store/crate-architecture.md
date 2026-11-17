# 后端 Crate 架构现状

| 项 | 值 |
|---|---|
| 更新日期 | 2026-11-17 |
| 数据来源 | 规模、文件数、模块清单由 `scripts/probe-structure.mjs` 生成；依赖数与依赖边由 `cargo metadata` 生成；编译耗时与依赖闭包取自 `docs/store/compile-time-attribution.md` |
| 关联文档 | `docs/store/compile-time-attribution.md`（编译耗时归属实测）、`docs/CODE-STRUCTURE.md`（前后端目录现状）、`docs/ARCHITECTURE.md`（组件职责） |
| 历史决策 | crate-architecture-analysis（已归档，施工记录，不再维护） |

本文只描述**当前事实**，不含目标架构与执行顺序。规划类内容写入 `docs/store/` 下的专项文档。

---

## 0. 数据口径

| 项 | 口径 |
|---|---|
| 行数 | **非空行**（剔除纯空白行，含注释与内联测试）。统计各 crate `src/**/*.rs`，**不含** `build.rs`、`tests/`、`target/` 产物 |
| 文件数 | 各 crate `src/**/*.rs` 文件数（含 `lib.rs` / `mod.rs`，不含 `tests/` 与 `build.rs`） |
| 直接依赖数 | `cargo metadata` 中该 crate `kind = normal` 且**非 path**（第三方）的依赖条目数，**含 target 平台专属段**；不含 dev / build 依赖。workspace 内部 crate 依赖单列在「内部依赖」行 |
| 编译耗时 | 三次 `cargo clean + cargo build --timings` 的**中位数**，详见 `docs/store/compile-time-attribution.md` |
| 依赖边 | `Cargo.toml` 中显式声明的 workspace 内部 crate 依赖；不推断传递依赖 |

---

## 1. 总览

### 1.1 规模

| 组成部分 | 文件数 | 非空行 | 说明 |
|---|---|---|---|
| 主 app（`src/`） | 101 | 16,303 | Tauri 命令层、协议层、初始化 |
| 12 个子 crate | 338 | 55,327 | 业务与基础设施 |
| **合计** | **439** | **71,630** | |

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

主 app 自身为 workspace 根包 `assistant-app`。

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
├── assistant-sync      ──→ （无内部依赖，被 vault 依赖）
├── assistant-security  ──→ assistant-crypto
├── assistant-agent     ──→ assistant-providers, assistant-storage
└── assistant-vault     ──→ assistant-crypto, assistant-sync, assistant-config
```

内部依赖边共 **6 条**（不含主 app）：

| 边 | 性质 |
|---|---|
| `assistant-security → assistant-crypto` | security 复用 crypto 的加密原语 |
| `assistant-vault → assistant-crypto` | vault 复用 crypto 的加密原语 |
| `assistant-vault → assistant-sync` | vault 用 sync 做 git 同步 |
| `assistant-vault → assistant-config` | vault 配置组复用 `ConfGroup` 机制层 |
| `assistant-agent → assistant-providers` | agent 调用 provider 发起请求 |
| `assistant-agent → assistant-storage` | agent 读写会话与工具历史 |

其余 6 个 crate（config / i18n / crypto / orb / tools / storage / crash / sync 中的 8 个，除 config 与 sync 被 vault 依赖外）为叶子节点。**没有环，也没有跨层回指。**

#### 可选依赖与 feature 门控

主 app 对 12 个子 crate 全部声明，但其中 **3 个是 `optional = true`**，由单一 feature 激活：

```toml
[features]
default = ["vault"]
vault = ["dep:assistant-vault", "dep:assistant-sync", "dep:assistant-security"]
```

代码侧对应门控：`src/vault_plugin.rs` 有 `#[cfg(feature = "vault")]` / `#[cfg(not(feature = "vault"))]` 两个分支，`src/cmd/mod.rs` 的 `mod vault` 声明带 `#[cfg(feature = "vault")]`。

因此 **vault / sync / security 已构成一个可整体关闭的编译单元**：`--no-default-features` 一次移除三者，不需要额外处理声明与使用的分离问题（旧文档 §4.1 所述缺口已由该 feature 消解）。

#### 消费者分布（含主 app）

| crate | 上游消费者 | 数量 |
|---|---|---|
| `assistant-crypto` | app / security / vault | 3 |
| `assistant-config` | app / vault | 2 |
| `assistant-providers` | app / agent | 2 |
| `assistant-storage` | app / agent | 2 |
| `assistant-sync` | app / vault | 2 |
| `assistant-agent` | app | 1 |
| `assistant-security` | app（仅 `cmd/vault/*`） | 1 |
| `assistant-vault` | app（仅 `cmd/vault/*`） | 1 |
| `assistant-tools` / `orb` / `crash` / `i18n` | app | 1 |

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
| 行数 / 文件 | 753 / 9 |
| 直接依赖 | 5（`serde` `serde_json` `serde_ignored` `log` `dirs`） |
| 内部依赖 | 无 |
| feature | `slow-tests`（仅测试用） |
| 模块 | `app_conf` `atomic_write` `conf_group` `constant` `json_config` `mascot_conf` `orb_conf` `shortcut` |

配置机制库：提供 `ConfGroup` 机制层（加载策略、原子写入、`apply_patch`）与三个核心配置组的 schema，纯 JSON 配置读写，不依赖 tauri，也不依赖任何业务 crate。只提供配置机制，vault 配置 schema 位于 `assistant-vault/src/conf.rs`，因此机制层与各特性仍是零耦合的。

### 2.2 assistant-i18n

| 项 | 值 |
|---|---|
| 行数 / 文件 | 124 / 1 |
| 直接依赖 | 1（`serde_json`） |
| 内部依赖 | 无 |
| build.rs | 有（16 行，生成语言目录 revision 常量） |
| 导出 | 无 `pub mod`，纯函数 + 常量（`DEFAULT_LOCALE`、`SHIPPED_LOCALES`、`text`、`t`、`t_with_args`、`format_text`、`set_locale`、`current_locale`、`locale_code`、`init_global`） |

单文件 crate（`lib.rs`）。`zh-CN.json` / `en-US.json` 通过 `include_str!` 在编译期嵌入，运行时零 IO。

### 2.3 assistant-crypto

| 项 | 值 |
|---|---|
| 行数 / 文件 | 1,071 / 6 |
| 直接依赖 | 16（`serde` `log` `thiserror` `aes-gcm` `chacha20poly1305` `argon2` `hkdf` `zeroize` `subtle` `base64` `data-encoding` `rand` `sha2` `machine-uid` `hmac` `sha1`） |
| 内部依赖 | 无 |
| feature | `slow-tests`（仅测试用）；**无依赖面 feature 切分** |
| 模块 | `api_key` `error` `vault_crypto`（含 `totp` `types`） |

**这是全库唯一被两个业务方向共用的加密 crate**，但两个模块的依赖需求并不相同：

| 模块 | 消费者 | 需要的依赖 |
|---|---|---|
| `api_key` | 主 app（provider / compaction / chat） | `aes-gcm` `base64` `rand` `sha2` `machine-uid` |
| `vault_crypto` | `assistant-vault`（+ `assistant-security` 的再导出桩） | `argon2` `hkdf` `chacha20poly1305` `subtle` `data-encoding` `hmac` `sha1` `zeroize` |

`assistant-crypto` 是主 app 的**非可选**依赖，而 crate 内没有按依赖面切分的 feature，因此即使 `--no-default-features`（vault 三件套全部关闭），主 app 仍会编译 `argon2` / `hkdf` / `chacha20poly1305` 等约 10s 的依赖。这是**当前唯一一处已识别的、可通过 crate 内 feature 切分解决的浪费**（详见 `docs/store/compile-time-attribution.md` §7）。

### 2.4 assistant-storage

| 项 | 值 |
|---|---|
| 行数 / 文件 | 4,036 / 21 |
| 直接依赖 | 11（含 `rusqlite`、`tokio`） |
| 内部依赖 | 无 |
| build.rs | 有（8 行，Windows 链接 crypt32/advapi32/ws2_32/user32） |
| feature | `slow-tests`（仅测试用） |
| 模块 | `agent_modes` `attachment_store` `content_codec` `db` `error` `migrations` `providers` `token_usage` `workspaces`；`conversations`（`branch` `compression` `crud` `msg_ops` `ops` `tree_ops`）；`tools`（私有聚合：`mcp_servers` `tool_favorites` `tool_history`） |

会话、附件、工具历史、agent 模式的 SQLite 持久化。`rusqlite` 走 `bundled-sqlcipher`（`workspace.dependencies` 中声明 `features = ["bundled-sqlcipher"]`）。`conversations/` 与 `tools/` 均为分组产物，`lib.rs` 以 `pub use` 保持原公开路径。

**重要事实**：主 app 自身也直接依赖 `rusqlite`。实测直接使用点为 `src/setup.rs`（`DbState(pub std::sync::Mutex<rusqlite::Connection>)`）、`src/adapters/{input_source,message_store,permission_store}.rs`、`src/cmd/{mod,storage,conversation_utils}.rs`。因此 `rusqlite` + `libsqlite3-sys`（合计约 10.3s）是**主功能固定成本，排除 vault 也无法省掉**。

### 2.5 assistant-providers

| 项 | 值 |
|---|---|
| 行数 / 文件 | 9,300 / 47 |
| 直接依赖 | 12（`reqwest` `tokio` `tokio-util` `futures` `async-stream` `bytes` `urlencoding` `eventsource-stream` 等） |
| 内部依赖 | 无 |
| feature | `internal-tests`、`slow-tests`（仅测试用） |
| 模块 | `base` `capability` `diagnostics` `model_catalog` `sse_log` `trace` `vision`；`client`（`error` `quirks`）；`stream`（`inline_think` `stats` `timeout`）；`types`（`chat_delta` `config` `message` `tool` `unified`）；`openai`（`chat` `discovery` `message_converter` `responses_stream` `responses_types` `stream` `types`）；`anthropic`（`discovery` `request` `stream` `types` + `message_converter/{messages,tools}`）；`gemini`（`code_assist` `discovery` `request` `stream` `types` + `message_converter/{messages,schema,tools}`） |

行数第三、文件数第三的 crate（9.3k 行、47 文件）。承载三家 LLM 的请求构造、SSE 流解析、工具调用累积、视觉输入。无内部依赖，是纯能力层。

### 2.6 assistant-agent

| 项 | 值 |
|---|---|
| 行数 / 文件 | 7,948 / 45 |
| 直接依赖 | 14（含 `image`、`pdf-extract`） |
| 内部依赖 | `assistant-providers`、`assistant-storage` |
| feature | `slow-tests`、`test-doubles`（仅测试用） |
| 模块 | `chat_options` `compaction` `error` `multimodal` `normalization` `pdf_text` `prompt_builder` `provider_config` `request_projection` `token_estimate`；`agent`（`ladder` `middleware` `round_executor` `stop` `tool_call` `tool_event` + `loop_executor/{budget,call,execute,persist,terminal}`）；`kernel`（`ids` `limits` `request` `tasks` + `ports/{test_doubles}` + `turn/{emit,outcome,persist,phase,runner,state,stream}`） |

会话编排层：把用户输入 + 历史 + 工具结果组装成 provider 请求，并把结果落回 storage。`image`（9.8s）与 `moxcms` / `pfxm` / `png` / `tiff` 等图像栈由该 crate 的 `multimodal` 引入；`pdf-extract` 服务于 `pdf_text`。`kernel/turn/` 与 `agent/loop_executor/` 均为分组产物，`kernel/mod.rs` 以 `pub use turn::{...}` 保持原路径。

### 2.7 assistant-tools

| 项 | 值 |
|---|---|
| 行数 / 文件 | 17,499 / 97 |
| 直接依赖 | 22（`async-trait` `dashmap` `chardetng` `encoding_rs` `ignore` `globset` `grep-searcher` `grep-regex` `tempfile` `trash` `windows` `libc` `landlock` 等） |
| 内部依赖 | 无 |
| 模块 | `contract`（`context` `definition` `result` `trait`）、`runtime`（`exec` `pipeline` `policy` `question` `registry` `storage` + `compact/{access,anchor,envelope,error_kind,head,preview,text,truncation,types}` + `gateway/{approval_key,context,decision,stores,testing}`）、`mcp`（`bridge/{call,defs,naming}` `manager/{lifecycle,types}` `stdio/{client,codec,stderr,transport}`）、`os`（`shared/{fs_util,output,process_output,scope,test_support,text_encoding,walk,path/{args,membership,protected,resolve}}` `tools/{agent_ask_user,agent_glob,agent_list,agent_mkdir,agent_move,agent_delete/{attrs,exec,scan,target,trash},agent_edit/{apply,locate,normalize,report,text},agent_grep/{outcome,probe,render,request,search,sink},agent_read/{window},agent_write/{atomic,encoding,lines,preview},fs_link/{check,create,validate}}`）、`sandbox`（`linux` `macos` + `windows/{acl}`） |
| 平台依赖 | Windows：`windows`（声明于 `[target.'cfg(windows)'.dependencies]`） |

工具契约、执行运行时与平台适配（文件读写/编辑/搜索、链接、编码探测、MCP 桥接、沙箱）。全库最大 crate。`windows` crate 的实际使用点集中在 `sandbox/windows/`（`mod.rs` + `acl.rs`），feature 归属见 §3.4。

### 2.8 assistant-orb

| 项 | 值 |
|---|---|
| 行数 / 文件 | 382 / 4 |
| 直接依赖 | 2（`log` `tauri`） |
| 内部依赖 | 无 |
| 模块 | `geometry` `hover` `placement` |

**唯一直接依赖 `tauri` 的子 crate**。精确耦合点是 `src-tauri/crates/assistant-orb/src/placement.rs` 的 `get_available_screens(app: &AppHandle)`（取可用屏幕几何）；`geometry.rs`、`hover.rs` 均为纯 Rust，且已有 `tests/placement.rs`、`tests/hover.rs` 覆盖纯计算路径。因此"依赖 tauri 导致无法测试"不成立，要去掉 tauri 依赖只需把该函数上移到主 app 的 `cmd/orb.rs`。

### 2.9 assistant-crash

| 项 | 值 |
|---|---|
| 行数 / 文件 | 146 / 2 |
| 直接依赖 | 3（`crash-handler` `minidump-writer` `chrono`） |
| 内部依赖 | 无 |
| 模块 | 无 `pub mod`，私有 `mod crash_dump` + `pub use crash_dump::install` |

最小 crate（146 行）。`minidump-common`（7.6s）由 `minidump-writer` 引入。

### 2.10 assistant-sync

| 项 | 值 |
|---|---|
| 行数 / 文件 | 1,489 / 17 |
| 直接依赖 | 3（`serde` `log` `thiserror`） |
| 内部依赖 | 无 |
| feature | `git-tests`（仅测试用） |
| 模块 | `redact` `types`；`git_sync`（`commit` `conflict` `helpers` `lifecycle` `remote` `state` `ssh`） |

vault 的 git 同步实现。**唯一 crate 级消费者是 `assistant-vault`**；主 app 对它的直接声明（optional，由 feature `vault` 激活）仅用于 `cmd/vault/*`，因此它与 vault 属同一排除单元。

### 2.11 assistant-security

| 项 | 值 |
|---|---|
| 行数 / 文件 | 2,771 / 27 |
| 直接依赖 | 10（`arboard` `hex` `log` `reqwest` `rsa` `serde` `sha1` `ssh-key` `windows` `zeroize`） |
| 内部依赖 | `assistant-crypto` |
| feature | `slow-tests`（仅测试用） |
| 模块 | `clipboard_guard` `reveal_guard` `vault_crypto`；`hibp`（`mod`）；`screen_lock`（`linux` `macos` `windows`）；`ssh_agent`（`unix` `validate` `windows`）；`unlock_dialog`（`linux` `macos` + `windows/{ffi,layout,localization,paint,state,subclass,text,theme,wm_handlers,wnd_proc}`） |
| 平台依赖 | Windows：`windows`（源码实际引用 `Win32::Foundation`、`Win32::System::Registry`） |

vault 的安全外围：剪贴板守卫、防截屏、锁屏/休眠监听、SSH agent 桥接、自绘解锁弹窗、HIBP 泄露检测、`vault_crypto` 再导出桩。

**关键事实**：`assistant-security` **不被 `assistant-vault` 依赖**，它与 vault 是主 app 的并列下游，二者耦合发生在代码层——`cmd/vault/{ssh,mod,lifecycle,audit}.rs` 调用 security 的模块。经 L1 解耦后，主功能（provider / chat / compaction）已完全改为直接使用 `assistant_crypto::api_key`，**不再有任何主功能代码引用 `assistant-security`**。它的可选依赖声明与 `cmd/vault` 同受 feature `vault` 门控，因此"排除 vault 却不排除 security"不会发生。

### 2.12 assistant-vault

| 项 | 值 |
|---|---|
| 行数 / 文件 | 9,808 / 62 |
| 直接依赖 | 25（全部为第三方，含平台专属 `core-foundation` `security-framework` `libsecret` `windows` `windows-future` 与类型生成 `ts-rs`） |
| 内部依赖 | `assistant-crypto`、`assistant-sync`、`assistant-config` |
| build.rs | 有（9 行，Windows 链接 crypt32/advapi32/ws2_32/user32） |
| feature | `git-tests`、`slow-tests`（仅测试用） |
| 模块 | `audit`（`jsonl`）`biometric`（`linux` `macos` `windows`）`conf` `lifecycle` `password_strength` `plaintext_export` `types`；`dto`（`audit` `conf` `error` `hibp` `integrity` `lifecycle` `recovery` `ssh` `sync` `totp` + `convert/{entry,error,misc,sync}`）；`recovery_key`（`paper` `session`）；`security_db`（`anchor` `biometric_ops` `crypto_ops` `import_export` `meta` `rekey` `schema` `session` `state` + `entries/{deleted,field_crypto,import,query,snapshot,tx,write}`）；`shared`（`totp`）；`sync_engine`（`export` `history` `lifecycle` `preview` `pull_push` `remote_admin`）；`vault_file`（`entry` `meta` `types`） |
| 平台依赖 | Windows：`windows` + `windows-future`；macOS：`core-foundation` + `security-framework`；Linux：`libsecret` |

全库第二大 crate（9.8k 行、62 文件，仅次于 `assistant-tools`）。重依赖集中地：`zxcvbn`（5.0s）、`argon2`（2.1s）、`keyring`（0.3s）、`rusqlite`（共享）、`ts-rs` + `ts-rs-macros`（6.1s，用于 DTO 类型生成）。

三个平台专属依赖段是 vault 独有的：主 app 不声明 `core-foundation` / `security-framework` / `libsecret`。

---

## 3. 横向事实汇总

### 3.1 规模排序

| 排名 | crate | 非空行 | 文件数 |
|---|---|---|---|
| 1 | assistant-tools | 17,499 | 97 |
| 2 | assistant-vault | 9,808 | 62 |
| 3 | assistant-providers | 9,300 | 47 |
| 4 | assistant-agent | 7,948 | 45 |
| 5 | assistant-storage | 4,036 | 21 |
| 6 | assistant-security | 2,771 | 27 |
| 7 | assistant-sync | 1,489 | 17 |
| 8 | assistant-crypto | 1,071 | 6 |
| 9 | assistant-config | 753 | 9 |
| 10 | assistant-orb | 382 | 4 |
| 11 | assistant-crash | 146 | 2 |
| 12 | assistant-i18n | 124 | 1 |

### 3.2 编译耗时（快照）

> 下表取自 `docs/store/compile-time-attribution.md` 的三次 `cargo build --timings` 中位数。**该快照测量于 crate 内部分组调整之前**，分组会改变单 crate 自身耗时（代码总量不变），因此逐 crate 数值只作量级参考，不用于比较或验收；需要最新值请按 §6 重测。

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

**workspace 自身只占约 6.5%，93% 以上的编译成本来自第三方依赖。** 这是判断"拆 crate / 排除 crate"是否值得的核心前提：动 crate 结构本身收益很小，收益必须来自依赖面的增减。

### 3.3 build.rs 分布（4 个）

| 位置 | 行数 | 作用 |
|---|---|---|
| `build.rs`（app） | 100 | 扫描 `src/*_plugin.rs` 派生命令清单并注册内联插件，再调 `tauri_build::try_build` |
| `crates/assistant-i18n/build.rs` | 16 | 生成语言目录 revision |
| `crates/assistant-storage/build.rs` | 8 | Windows 链接 `crypt32` `advapi32` `ws2_32` `user32` |
| `crates/assistant-vault/build.rs` | 9 | 同上 |

storage 与 vault 的 build.rs 内容一致，都服务于 `rusqlite`（SQLCipher）在 Windows 上的链接需求。**规则：谁依赖 `rusqlite`（SQLCipher），谁必须自带这份 `rustc-link-lib` 声明**——因为 `rustc-link-lib` 只沿依赖链单向传播，不能指望兄弟 crate 代为链接。

排除 vault 时，`crates/assistant-vault/build.rs` 随之消失；`crates/assistant-storage/build.rs` 必须保留（storage 仍在）。

### 3.4 平台专属依赖归属

| 平台 | 依赖 | 声明与消费 |
|---|---|---|
| Windows | `windows` | 声明于 `[workspace.dependencies]`；由 app、`assistant-tools`、`assistant-security`、`assistant-vault` 四处以 `workspace = true` 消费 |
| Windows | `windows-future` | 仅 `assistant-vault` |
| macOS | `core-foundation`、`security-framework` | 仅 `assistant-vault` |
| Linux | `libsecret` | 仅 `assistant-vault` |
| Linux | `landlock`、`libc` | 仅 `assistant-tools` |

`windows` 在 workspace 段共声明 **22 个 feature**。按源码实际引用归口：

| 服务对象 | feature 数 | feature 列表 |
|---|---|---|
| 主功能：app `orb_overlay.rs` | 3 | `Win32_Foundation`、`Win32_Graphics_Dwm`、`Win32_UI_WindowsAndMessaging` |
| 主功能：tools `sandbox/windows/` | 6 | `Win32_Foundation`（共用）、`Win32_Security`、`Win32_Security_Authorization`、`Win32_System_JobObjects`、`Win32_System_Pipes`、`Win32_System_Threading` |
| vault：`biometric/windows.rs` | 4 | `Security_Credentials`、`Security_Cryptography`、`Storage_Streams`、`Win32_System_WinRT` |
| security：`unlock_dialog/windows/` | 1 | `Win32_System_Registry` |
| 源码中无引用 | 9 | `Foundation`、`Win32_Storage_FileSystem`、`Win32_System_LibraryLoader`、`Win32_System_RemoteDesktop`、`Win32_System_Power`、`Win32_Graphics_Gdi`、`Win32_UI_Input_KeyboardAndMouse`、`Win32_UI_Controls`、`Win32_UI_HiDpi` |

去重后：**13 个 feature 有实际引用**（主功能 8、vault 4、security 1），**9 个无源码引用**。主功能所需的 8 个中，`Win32_Foundation` 与 `Win32_UI_WindowsAndMessaging` 同时被 vault 使用。

机制上，`features` 写在 `[workspace.dependencies]` 里，成员只能用 `workspace = true` 取**并集**，无法从成员侧削减。因此 `assistant-tools` 被迫接受 vault / security 的全部 feature。可选的整理方式是把 feature 从 workspace 段移除、由各成员自行声明（已论证：该改动只对单 crate 构建有效，完整构建因 feature 统一取并集而零提速）。

但 `windows` crate 本身无法从依赖图中移除——Tauri 运行时栈（`tauri-runtime-wry` / `wry` / `webview2-com-sys` / `tao` / `muda` / `tray-icon`）独立声明了自己的 windows feature 集合，仍需完整编译。排除 vault 能减少的是**本次构建启用的 feature 面**，不是整个 `windows` 包。

### 3.5 依赖闭包与可省范围

| 集合 | 包数 | 说明 |
|---|---|---|
| `assistant-vault` 依赖闭包 | 126 | 含 crypto、sync 及共享依赖 |
| `vault + sync + security` 联合闭包 | 246 | 潜在排除集合 |
| 其中主功能同样需要（共享） | 198 | 无法省 |
| 真正可省（专属） | **48** | 串行 78.2s，占 Σ 的 **7.2%** |

结论：**vault 闭包看着大（126 包），实际可省的只有 48 包**。因为 `windows`（54.3s）、`tauri-utils`（50.9s）、`windows-sys`（37.7s）、`tokio`（15.5s）、`rusqlite`+`libsqlite3-sys`（10.3s）这些大头同时被主功能需要。完整清单与重测方法见 `docs/store/compile-time-attribution.md` §6.3。

---

## 4. 当前架构的已知问题

按影响排序，均为**事实描述**，不含解决方案。

### 4.1 `assistant-crypto` 两个方向的依赖混装

`api_key`（主功能）与 `vault_crypto`（vault）共用同一套依赖，crate 内没有依赖面 feature 切分（仅有 `slow-tests` 测试 feature）。`assistant-crypto` 是主 app 的非可选依赖，因此**即使关闭 feature `vault`**，主功能仍被迫编译 `argon2` / `hkdf` / `chacha20poly1305` / `data-encoding` 等约 10s 的依赖。这是当前唯一可通过 crate 内 feature 切分直接消解的浪费。

### 4.2 `assistant-vault` 与 `assistant-security` 是代码层耦合而非 crate 依赖

二者的调用关系只存在于 `cmd/vault/{ssh,mod,lifecycle,audit}.rs`，crate 依赖图上 security 是主 app 的并列下游。这意味着：

- 依赖图无法反映真实耦合强度；
- 排除决策必须同时参考**代码层引用**（`cargo tree` 不够）；
- 该耦合已由 feature `vault` 统一封装（三者同为 optional），因此不会出现"只排 vault 不排 security"的错误组合。

### 4.3 `assistant-sync` 的消费者单一

`assistant-sync` 仅被 `assistant-vault` 依赖，主 app 对它的直接声明也只服务于 `cmd/vault/*`。它本质上属于 vault 的组成部分，当前作为独立 crate 存在只是因为历史上先拆出来了。

### 4.4 主 app 直接持有 `rusqlite`

`src/setup.rs` 的 `DbState` 与 `src/adapters/*`、`src/cmd/{storage,conversation_utils,mod}.rs` 直接使用 `rusqlite`，而 `assistant-storage` 也依赖 `rusqlite`。这意味着 SQLCipher 的链接成本（`rusqlite` 3.4s + `libsqlite3-sys` 6.9s + 两份 build.rs）由两个位置共同承担，且无法通过排除任一业务 crate 消除。

### 4.5 `assistant-orb` 依赖 `tauri`

12 个子 crate 中只有 orb 直接依赖 `tauri`，但耦合面仅 `placement.rs` 的一个取屏幕函数；纯计算部分已有测试覆盖，因此"可脱 tauri 测试"这一判据实际是**满足**的，不属于阻塞项。

### 4.6 `windows` feature 面存在 9 个无引用项

workspace 段声明 22 个 `windows` feature，其中 9 个在四个消费方的源码中均无引用（清单见 §3.4）。由于成员只能取并集，这 9 个 feature 对所有消费方都是纯增量编译面。

---

## 5. 判据：何时该拆出新 crate

沿用存档文档 crate-architecture-analysis（已归档）§4.3 的决策检查表，口径不变：**命中任一条件才值得独立成 crate；全部不命中则留在主 app**。示例按 2026-11-16 实测。

| # | 条件 | 判据 | 当前实测示例 |
|---|---|---|---|
| ① | 多消费者 | 被 ≥2 个上游 crate 引用 | `assistant-crypto`（3：app / security / vault）、`assistant-config`（2：app / vault）、`assistant-storage`（2：app / agent）、`assistant-providers`（2：app / agent）、`assistant-sync`（2：app / vault） |
| ② | 可脱 tauri 独立测试 | 核心逻辑不依赖 `AppHandle` / 窗口系统，且可测逻辑 >100 行 | `assistant-crash`（纯文件 IO）、`assistant-orb`（纯几何部分已有 `tests/`）、`assistant-config`、`assistant-i18n` |
| ③ | 编译隔离 | 拉入重依赖（单项编译 >3s），需 feature gate 让开发时跳过 | `assistant-crash`（`minidump-common` 7.6s）、`assistant-vault`（`zxcvbn` 5.0s、`ts-rs` 6.1s）、`assistant-agent`（`image` 9.8s） |
| ④ | 独立领域 + 体量 | 自有类型体系与错误处理，且 >300 行 | `tools`(17.5k) `vault`(9.8k) `providers`(9.3k) `agent`(7.9k) `storage`(4.0k) `security`(2.8k) `sync`(1.5k) `crypto`(1.1k) `config`(0.7k) `orb`(0.4k) |
| ⑤ | 以上均不命中 | 留在主 app：tauri 胶水 / 单消费者 / 轻依赖 / 体量小 | `src-tauri/src/cmd/mascot_asset.rs`（182 非空行，两个 `#[command]` 均持有 `AppHandle`） |

> **对旧文档示例的修正**：① 旧文把 `assistant-config` 判为"只有 app 一个上游、命中 ②+④"，但 vault 已依赖 config 复用 `ConfGroup`，实测上游为 app + vault 两个，**① 亦命中**；② 旧文 `security` 的 ③ 示例写"zxcvbn 9.7s"，但 zxcvbn 已随拆分迁入 vault，security 现自身编译 1.2s，其 ③ 依据应改记为"独占 20+ 外部依赖（arboard / ssh-key / rsa / reqwest 等），无第二消费者"。

**关键区分（沿用旧结论）**：「不能放进某个 crate」≠「必须新建 crate」。判断前先问：**该模块是否本质上是 tauri 框架胶水？** 若是，主 app 就是它的正确归属；独立只会把 tauri 重依赖搬进新 crate。

### 5.1 主 app 职责下沉：旧结论与当前状态

旧文档 §3 给出的下沉清单，按当前代码逐项核对（行数为非空行实测）：

| 项 | 旧结论 | 当前状态 |
|---|---|---|
| 崩溃转储 `crash_dump.rs` | 下放为 `assistant-crash` | ✅ 已落地，见 §2.9 |
| 存储分析逻辑 `cmd/storage.rs`（191） | → `assistant-storage` | ⬜ 未做。仍需先剥离 `AppHandle` 胶水，核心为 DB + `attachment_store` 调用 |
| 会话工具函数 `cmd/conversation_utils.rs`（196） | → `assistant-storage` | ⬜ 未做。纯 DB 查询（2 处 `rusqlite` 引用），返回 storage 类型 |
| Chat 消息插入 `cmd/chat/insert.rs`（569） | → `assistant-agent` | ⬜ 未做。前置条件仍在：3 处 `assistant_i18n::t()` 需先改为结构化错误 |
| DTO / 协议类型 `protocol/dto/`（18 文件 2,112） | 有条件抽 `assistant-protocol` | ⬜ 结论：**维持不抽**。① 不命中（单消费者 app）；④ 形式命中（2.1k 行、自有 wire 类型体系），但抽出的收益只有边界清晰，成本是新增 crate 边界 + 该 crate 需再引入 `ts-rs`，零复用 |
| API Key 掩码 `cmd/provider.rs`（246） | ❌ 留主 app | ❌ 结论不变。L1 解耦后主 app 直接调用 `assistant_crypto::api_key`，搬运无收益（单消费者，且 crypto 已是叶节点） |
| Vault 编排辅助 `cmd/vault/mod.rs`（410） | ❌ 不搬运 | ❌ 不变。`AppConf` 读取 + `SshAgentState` 状态 + 领域调用的编排胶水，搬入会让 security 反向依赖 config 与 tauri |
| `mascot_asset`（182） | ❌ 不抽 crate（命中 ⑤） | ❌ 不变 |

> **DTO 现状补充（旧文此处已过期）**：`ts-rs` **已引入**——主 app `Cargo.toml` 声明 `ts-rs = { version = "12", features = ["serde-json-impl", "no-serde-warnings"] }`（注释即"协议 DTO 的 TS 契约生成"），`src/types/protocol.contract.ts` 由其生成并有 `src-tauri/tests/protocol_bindings.rs` 作为一致性门禁；vault 侧另有独立 `dto/` 体系（`ts-rs` 在 vault 自身声明）。因此"引入 ts-rs / typeshare"不再是前提条件，抽 `assistant-protocol` 的取舍已回到纯粹的 ①/④ 判断，见上表。

---

## 6. 复现命令

规模、模块、依赖、深度与扁平度的数字一律由脚本生成，不在文档内维护独立统计逻辑：

```powershell
# 全量（Rust workspace + 前端），输出 temp-backup/structure-probe.md
node scripts/probe-structure.mjs

# 仅 Rust workspace
node scripts/probe-structure.mjs rust
```

依赖数与依赖边：

```powershell
# 工作目录 src-tauri
cargo metadata --format-version 1 --no-deps
```

依赖闭包与编译耗时：

```powershell
cargo tree -p assistant-vault --prefix none --no-dedupe
cargo tree -i assistant-security --prefix none
cargo clean; cargo build --timings
```

feature 门控验证：

```powershell
# 关闭 vault 三件套后的依赖面
cargo tree --no-default-features -e normal --prefix none
```
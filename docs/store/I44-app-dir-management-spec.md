# I44 — 应用目录管理规格（受管目录的解析、归属与边界）

| 项 | 值 |
|---|---|
| 状态 | Draft |
| 日期 | 2026-11-10 |
| 关联 | CODE-REVIEW-PRINCIPLE（§7.6 文件与路径链路；D3 错误处理与可观测性、D4 架构与职责边界、D11 安全与隐私、D15 平台兼容）；I38h 工作区规格（已归档；工作区根解析沿用其结论）；路径处理不统一分析（已归档；仅作现状来源，不作为规格依据）；I31 工具层路径规格（已归档；外部路径校验实现归属） |
| 影响范围 | 新增目录模块 `src-tauri/src/app_dirs/`；`src-tauri/src` 启动与命令层；`assistant-storage` / `assistant-tools` / `assistant-crash` / `assistant-agent`；前端附件与资产 URL 模块；`tauri.conf.json`；`src/types/protocol.contract.ts` |

> 本文只管理**受管目录**：应用自有、由应用解析、只被应用自己读写的目录。
> 工具调用的输入路径与工作区边界由 `assistant-tools/src/os/path.rs` 承担，本文不重复规划；本文只处理两类边界——「外部文件进入/离开受管目录」与「受管目录内 id → 文件名」。
> 现状分析（已归档）已复核：「新建 `os/path.rs`」一项**已完成**；其余条目按第 4 节的事实重新取值。
> 文件名 `I44-app-dir-management-spec.md` 与标题一致；不再使用旧名。

---

## 1 概述

受管目录是应用自己拼出来的路径：数据根、本地根、配置根及其下的第一层目录。它们不来自 webview、用户选择或模型输出。

当前有三类问题：

1. **基根由谁解析**：后端存在 4 套解析器（`dirs::config_dir()`、Tauri `app_data_dir()`、`dirs::data_dir()`、`tauri-plugin-log` 的 `LogDir`），前端还有第 5 套 `appDataDir()`。Windows 上其中三套恰好落到同一物理目录，掩盖了差异。
2. **目录名由谁持有**：第一层目录名与标识符字面量分散在 6 个 Rust 文件、1 个配置文件和 1 个前端文件，各调用点自行 `join`。
3. **失败怎么办**：同一类「取不到应用目录」有四种处理——panic、回退相对路径、回退 `.`、记日志后继续启动。

交付四块：

1. `src-tauri/src/app_dirs/`：无条件目录名常量 + 归属表 + `AppDirs`，**基根与无条件目录的唯一拼串点**，只在应用层；可选能力（vault）自持目录名，只从 `AppDirs::data()` 取基根（§6.2）。
2. 消费点收口：9 处目录解析/拼串点统一到 `AppDirs`，1 处登记为既有例外，另声明模块入口（第 9 节 B1–B11）。
3. 受管目录边界：附件 id 判据、外部文件入库、asset 协议作用域、前端资产 URL 单一拼接（第 7–8 节）。
4. 形态与编码：verbatim 三态分离、跨语言测试向量、非 UTF-8 一律失败、长路径按支持处理（第 10 节）。

## 2 目标

- 每个第一层目录名在**全仓库只有一个字面量**，且字面量的位置等于它的所有者：无条件目录在 `app_dirs::dir_name`，可选能力的目录在该能力模块内（vault 在 `cmd/vault`）。前端、`tauri.conf.json` 从常量派生或与之比对。
- **目录布局只在应用层持有**：`crates/` 下的库不解析基根、不调用 `dirs::`、不持有第一层目录名；需要目录的库一律由调用方传入根路径。
- 基根解析失败**没有任何**回退到相对路径、当前目录或默认值的路径；启动期失败中止启动。
- 受管目录的跨层访问只经三个入口：附件按 id 访问、外部文件经入库命令、前端资产 URL 经单一模块拼接。
- 资产协议作用域、写入目录、读取目录**同源**：由同一组常量生成，并由漂移测试守护。
- 展示形态与 IO 形态只在单一转换点互转；展示值不参与比较、持久化、日志键。
- **长路径可用**：不设长度上限、不截断、不因长度拒绝；超过平台 260 限制的路径能正常读写（§10.4）。
- 上述每条都有可执行的防回归测试，不依赖人工审查。

## 3 非目标

- **不**新建 crate，**不**把目录布局放进 `crates/` 下任何库：理由见 §5.2。
- **不**改动 `assistant-config` 现有的配置目录解析（5 个 `get_conf_dir` 计算点、4 个配置文件）：既有例外，登记见 §6.4。
- **不**做目录迁移：崩溃目录改根后，旧目录文件不搬迁、不删除，仅记录一条日志。
- **不**为长路径改注册表（`LongPathsEnabled`）、也不改应用清单（`longPathAware`）：支持只靠进程内的路径形态（§10.4）。
- **不**引入 `normpath` / `path-clean` 等新依赖；规范化只用 `std::path` 与既有 `canonicalize_lenient`。
- **不**把 DTO 的路径字段改成 `PathBuf`：wire 仍为 `String`，且强制 UTF-8（§10.3）。
- **不**改变 `tool-results` 的失效语义、`<persisted-output path>` 的跨进程重放约定。
- **不**处理网络盘/UNC 的性能与可靠性差异（允许使用，不做介质探测）。
- **不**重复规划工具调用的输入路径与工作区边界：由 `assistant-tools/src/os/path.rs` 与 I31 工具层路径规格（已归档）承担。

## 4 既有事实

| 事实 | 位置 |
|---|---|
| 后端存在 4 套基址解析：`dirs::config_dir()`、Tauri `app_data_dir()`、`dirs::data_dir()`、`tauri-plugin-log` 的 `LogDir`；前端另有 `appDataDir()` | `crates/assistant-config/src/json_config.rs`、`src-tauri/src/setup.rs`、`src-tauri/src/cmd/vault/mod.rs`、`src-tauri/src/cmd/vault/recovery.rs`、`src-tauri/src/cmd/mascot_asset.rs`、`crates/assistant-crash/src/crash_dump.rs`、`src-tauri/src/plugins.rs`、`src/chat/attachment/paths.ts` |
| 外部路径校验实现已存在：`os/path.rs` 有 10 个 `pub fn`（`resolve`、`canonicalize_lenient`、`reject_system_dirs`、`ensure_within_root` 等）；`canonicalize_lenient` 支持叶子不存在 | `crates/assistant-tools/src/os/path.rs` |
| 第一层目录名与标识符字面量分散在 6 个 Rust 文件 + 1 个配置文件 + 1 个前端文件 | `src-tauri/src/setup.rs`、`tool_setup.rs`、`cmd/vault/mod.rs`、`cmd/vault/recovery.rs`、`cmd/mascot_asset.rs`、`crates/assistant-crash/src/crash_dump.rs`、`src-tauri/tauri.conf.json`、`src/chat/attachment/paths.ts` |
| `APP_IDENTIFIER` 已在 `assistant-config::constant`；应用 crate 已依赖 `assistant-config`，但**未**声明 `dirs`（workspace 已固定 `dirs = "7"`，现有使用者是 `assistant-config` 与 `assistant-crash`） | `crates/assistant-config/src/constant.rs`、`src-tauri/Cargo.toml`、`src-tauri/crates/assistant-crash/Cargo.toml` |
| 目录注入模式已存在：`AttachmentStore::new(dir)`、`FileToolResultStorage::new(results_dir)` | `crates/assistant-storage/src/attachment_store.rs`、`src-tauri/src/tool_setup.rs` |
| 崩溃 crate 例外：自解析目录、依赖 `dirs`、在 `assistant_app::run()` 之前安装；解析失败回退 `.` | `crates/assistant-crash/src/crash_dump.rs`、`src-tauri/src/main.rs` |
| 崩溃安装时日志插件尚未初始化，只能 `eprintln!`，不能用 `log::warn` | `src-tauri/src/main.rs`、`src-tauri/src/plugins.rs` |
| vault 安全库/vault 目录解析失败分别回退相对路径；安全库路径非 UTF-8 时用 `unwrap_or` 换值 | `src-tauri/src/cmd/vault/mod.rs` |
| 附件根创建失败用 `expect` panic；主库解析/初始化失败只记日志后继续启动 | `src-tauri/src/setup.rs` |
| 库内子布局已存在：`attachments/derived`（附件侧车）、`security/backups`（安全库备份）、`mascot-assets/{rive,gif,svg}`、`tool-results` 文件名 | `crates/assistant-storage/src/attachment_store.rs`、`crates/assistant-vault/src/security_db/mod.rs`、`src-tauri/src/cmd/mascot_asset.rs`、`crates/assistant-tools/src/storage.rs` |
| 安全库 rekey 的备份目录由 `db_path.parent().unwrap_or(".")` 推导；父目录缺失时落 cwd | `crates/assistant-vault/src/security_db/mod.rs`、`crates/assistant-vault/src/security_db/rekey.rs` |
| 单段文件名校验只覆盖 `open_attachment`；`AttachmentStore` 的读写与侧车方法直接 `join(id)` | `src-tauri/src/cmd/storage.rs`、`crates/assistant-storage/src/attachment_store.rs` |
| vault 导入/导出、桌宠素材安装的源路径零校验 | `src-tauri/src/cmd/vault/entries.rs`、`src-tauri/src/cmd/mascot_asset.rs` |
| 消息内容仍接受 `file_path`；可信通道 `allow_absolute_paths: true` 使 `validate_path` 成为空操作 | `crates/assistant-agent/src/multimodal.rs` |
| 前端已存在「路径三态分离」范式：`name`（展示名）/ `path`（绑定值原样）/ `displayPath`（剥离 verbatim 前缀） | `src/chat/core/workspaceDisplay.ts` |
| 前端资产 URL 未收口：`ImageThumbnail` 本地 `join` + `convertFileSrc`；`factory` 对拖入路径直接 `convertFileSrc`；`AssetPickerField` 对 `installedPath` 直接 `convertFileSrc` | `src/chat/message/ImageThumbnail.tsx`、`src/chat/attachment/factory.ts`、`src/settings/shared/renderers/AssetPickerField.tsx` |
| asset 作用域以字面量列出两个目录，其中 `attachments/**` 连带覆盖 `attachments/derived/` 侧车 | `src-tauri/tauri.conf.json` |
| 配置目录由 5 个 `get_conf_dir` 计算点持有，产出 4 个配置文件 | `crates/assistant-config/src/{json_config,app_conf,orb_conf,mascot_conf,vault_conf}.rs` |
| `ARCHITECTURE.md` §5.2 的文件结构漏记 `tool-results/`、`security/recovery/`、`security/backups/`、`attachments/derived/`，并把崩溃目录记在 `%APPDATA%` 下 | `docs/ARCHITECTURE.md` |

## 5 落点与依赖

### 5.1 受管目录与受管边界的分工

| 类别 | 来源 | 落点 | 处理 |
|---|---|---|---|
| 受管目录 | 代码常量 + 系统目录 API | `src-tauri/src/app_dirs/`（应用层） | 只拼接；解析失败即错误；库只接收调用方传入的根 |
| 受管目录边界 | webview、用户选择、外部文件 | `assistant-tools/src/os/path.rs` + `assistant-storage` + 单一前端模块 | 每次使用前校验；入库后按 id 访问；资产 URL 单一拼接 |

两者不共用同一个函数，理由是**要做的事不同**：受管目录的边界就是它自己，套 canonicalize + 边界检查没有意义；边界路径的允许范围必须由调用方逐次声明（工作区根、系统目录、凭据目录）。这不是互相设防，也不构成信任分级。

### 5.2 为什么目录布局只能在应用层

目录布局是**组合根**的知识：哪个数据放哪个根、哪些进漫游、哪些与日志同根，取决于应用怎么部署，而不是某个库怎么实现。库一旦自己算目录，会带来两个具体问题：

1. **依赖反向**。任何需要该目录的 crate 都要依赖持有它的 crate。若把目录常量放进 `assistant-config`，`assistant-crash` 就必须依赖 `assistant-config`（配置加载、serde、迁移逻辑）才能拿到崩溃目录——为了一个字符串引入一整条依赖链，还让 panic hook 的可用性受配置层约束。反过来，让 `assistant-crash` 自己算目录（现状），它就不得不依赖 `dirs` 并自带一份 `com.assistant` 字面量。两种做法都在把目录责任塞进库，只是塞的位置不同。
2. **无法注入**。库自算目录后，测试只能读写真实用户目录，或改环境变量；崩溃转储、附件库这类要写盘的东西会变成只能在真机上验证。

本项目已有正确的注入模式（`AttachmentStore::new(dir)`、`FileToolResultStorage::new(results_dir)`），本规格只把需要收口的例外拉齐（崩溃目录改为传入），不发明新结构。

选 `src-tauri/src/app_dirs/` 的具体理由：

- workspace 已固定 `dirs = "7"`，应用 crate 补一行 `dirs = { workspace = true }` 即可，不引入新的外部 crate；
- 应用 crate 已依赖 `assistant-config`，可直接引用 `constant::APP_IDENTIFIER`，字面量仍只有一处；
- 模块在应用 crate 内，`crates/` 下的库一个都不需要改依赖声明；
- 以目录模块组织，按职责拆成 `dir_name.rs`（常量）与 `roots.rs`（基根与 `AppDirs`），后续新增目录类别不再扩大单文件。

库内部的子布局（`attachments/derived/`、`security/backups/`、`mascot-assets/rive/` 等）不在此列：它们属于库的存储实现，见 §6.5。

### 5.3 依赖方向的净变化

- 新增 crate 边：**0 条**（这里的 crate 指 workspace 内成员）。
- 依赖声明变化：应用 crate 新增 `dirs = { workspace = true }`（第三方 crate，已在编译图中，不新增 crate 节点）；`assistant-crash` 去掉 `dirs`（改为接收目录参数）。
- 约束：`crates/` 下任何库**不得**新增 `dirs` 依赖或 `dirs::` 调用；唯一例外是 `assistant-config` 的既有配置目录解析（§6.4）。

## 6 受管目录的解析与归属

### 6.1 基根与归属表

两个基根：**data**（`dirs::data_dir()/APP_IDENTIFIER`）与 **local**（`dirs::local_data_dir()/APP_IDENTIFIER`）。

| 条目 | 基根 | 持有者 | 进持久引用 | 说明 |
|---|---|---|---|---|
| `assistant.db` | data | 应用层 | 是 | 主库；业务数据 |
| `attachments/` | data | 应用层 | 是 | 附件根；消息以 id 引用 |
| `attachments/derived/` | data | `AttachmentStore` | 否 | PDF 文本侧车；库内子布局，不进 asset 作用域 |
| `tool-results/` | data | 应用层 | 是 | `<persisted-output path>` 跨进程重放依赖稳定绝对路径 |
| `mascot-assets/` | data | 应用层 | 是 | 下分 `rive/`、`gif/`、`svg/`；版本号被持久引用 |
| `security/security.db` | data | `cmd/vault`（vault 特性） | 是 | 安全域主库 |
| `security/vault/` | data | `cmd/vault`（vault 特性） | 是 | vault 条目 Git 工作区 |
| `security/recovery/` | data | `cmd/vault`（vault 特性） | 是 | 灾备临时仓库 |
| `security/backups/` | data | `SecurityDbState` | 否 | 安全库备份；库内子布局 |
| `logs/` | local | 应用层 | 否 | 诊断产物，不进漫游、不进持久引用 |
| `crashes/` | local | 应用层 | 否 | **变更**：由 data 迁到 local，与 `logs/` 同根 |
| `config.json` 等 4 个配置文件 | config | `assistant-config` | 是 | 既有例外（§6.4） |

变更与取舍：

- `crashes/` 由 data 迁到 local。现状崩溃与日志分属两个根，同一份诊断数据落在两处；旧目录不迁移（§3），启动后记录一条日志说明新位置。
- `attachments` 的 asset 作用域由 `$APPDATA/attachments/**` 收窄为 `$APPDATA/attachments/*`。附件原文件都在第一层，`derived/` 侧车只供后端读取，不应经 asset 协议暴露。Tauri 的 scope 匹配启用 `require_literal_separator`，`*` 不会跨目录匹配。
- `tool-results/`、`security/recovery/`、`security/backups/`、`attachments/derived/` 纳入归属表，用于修正 `ARCHITECTURE.md` 的文件结构漂移（§15）。
- `security/` 下的四条目录名由 `cmd/vault` 自持，**不**放进 `app_dirs`。两个理由：装配契约的 a2 门禁规定 `#[cfg(feature = ...)]` 只允许出现在 `cmd/mod.rs` 与同名 `*_plugin.rs`，把 vault 常量放进无条件模块会直接越界；且关闭特性后，这些路径知识不应继续留在编译产物里。vault 侧只从 `AppDirs::data()` 取基根（§6.2）。

### 6.2 API

```rust
// src-tauri/src/app_dirs/mod.rs
pub mod dir_name;
mod roots;

pub use roots::{AppDirs, DirError};

// src-tauri/src/app_dirs/dir_name.rs —— 仅无条件能力的目录名
pub const DATABASE: &str = "assistant.db";
pub const ATTACHMENTS: &str = "attachments";
pub const TOOL_RESULTS: &str = "tool-results";
pub const MASCOT_ASSETS: &str = "mascot-assets";
pub const LOGS: &str = "logs";
pub const CRASHES: &str = "crashes";

// src-tauri/src/app_dirs/roots.rs
#[derive(Debug)]
pub enum DirError {
    DataDirUnavailable,
    LocalDataDirUnavailable,
}

pub struct AppDirs { /* data, local */ }

impl AppDirs {
    pub fn resolve() -> Result<Self, DirError>;               // dirs::data_dir()/data_local_dir() + APP_IDENTIFIER
    pub fn with_bases(data: PathBuf, local: PathBuf) -> Self; // 测试注入
    pub fn data(&self) -> &Path;
    pub fn local(&self) -> &Path;
    pub fn database(&self) -> PathBuf;
    pub fn attachments(&self) -> PathBuf;
    pub fn tool_results(&self) -> PathBuf;
    pub fn mascot_assets(&self) -> PathBuf;
    pub fn logs(&self) -> PathBuf;
    pub fn crashes(&self) -> PathBuf;

    /// `["$APPDATA/mascot-assets/**", "$APPDATA/attachments/*"]`
    /// 唯一用途：与 tauri.conf.json 的 assetProtocol.scope.allow 做相等断言。
    pub fn asset_scopes() -> Vec<String>;
}

// src-tauri/src/cmd/vault/mod.rs —— 可选能力自持目录名，随 feature = "vault" 编译
const SECURITY_DIR: &str = "security";
const SECURITY_DB_FILE: &str = "security.db";
const VAULT_DIR: &str = "vault";
const RECOVERY_DIR: &str = "recovery";

pub(crate) fn security_db_path(dirs: &AppDirs) -> PathBuf; // data/security/security.db
pub(crate) fn repo_dir(dirs: &AppDirs) -> PathBuf;         // data/security/vault
pub(crate) fn recovery_dir(dirs: &AppDirs) -> PathBuf;     // data/security/recovery
```

vault 侧的函数刻意不加 `vault_` 前缀：`crates/assistant-vault/tests/conf_keys.rs` 会把 `pub vault_*` 声明当作配置键比对，加前缀会误命中。

`app_dirs` 模块内**不出现任何 `#[cfg(feature = ...)]`**：这是 a2 门禁的直接要求，也是「可选性边界与代码边界重合」的体现——vault 关掉后，`cmd/vault` 整个模块不编译，其目录名随之消失。

`DirError` 需实现 `Display` 与 `std::error::Error`，让启动层可以直接 `?`。`asset_scopes()` 不依赖实例：作用域用 `$APPDATA` 令牌表达，不写死本机路径。

`app_dirs` 模块在 `lib.rs` 声明为 `pub mod app_dirs`：`main.rs` 需要在 `assistant_app::run()` 之前算出崩溃目录，传给 `assistant_crash::install`。

### 6.3 解析时机：每个调用点自行 `resolve()`，不设全局状态

不采用 `OnceLock` 全局、也不采用 `manage(AppDirs)` 状态：

- 日志插件的 target 在 `plugins::attach` 构建 Builder 时就要确定，`AppHandle` 尚不存在；
- 崩溃安装早于 `assistant_app::run()`，同样没有 `AppHandle`；
- 插件 setup（`vault_plugin`）早于应用 setup 执行，托管状态有顺序依赖；
- `resolve()` 是两次系统目录查询 + 字符串拼接，幂等、无副作用、可在测试中注入基根。

用 `dirs` 而不是 `AppHandle::path()` 的原因：上述三个消费点都发生在 Tauri 初始化之前或插件构建期。`dirs` 在任意时刻可用；Windows/macOS/Linux 上 `dirs::data_dir()/APP_IDENTIFIER` 与 Tauri `app_data_dir()` 同源，T2 通过作用域断言守护这条同源关系。

**唯一性由所有者保证**：基根只在 `app_dirs` 拼接；无条件目录名只在 `app_dirs` 拼接；可选能力的目录名只在其自身模块拼接（vault → `cmd/vault`，以 `AppDirs::data()` 为根）。不由调用次数保证。调用点必须写 `AppDirs::resolve()?`，禁止自己 `dirs::*` + `join` 拼串。

### 6.4 跨技术栈同步与既有例外

- Rust：`APP_IDENTIFIER` 留在 `assistant-config::constant`，`app_dirs` 引用它；不迁址，既有调用点不变，字面量仍只有一处。
- **既有例外（登记）**：`assistant-config` 的 5 个 `get_conf_dir` 计算点（`json_config`、`app_conf`、`orb_conf`、`mascot_conf`、`vault_conf`）自己算配置目录，产出 `config.json`、`orb-config.json`、`mascot-config.json`、`vault-config.json`。本期不改，理由：`dirs` 已是其依赖、不新增任何边，且 `load()` / `save()` 有 30+ 调用点，改成 `load_from(dir)` 属独立重构。约束：`app_dirs` 不提供 `config()`，避免出现第 6 个计算点。
- 配置：`tauri.conf.json` 的 `identifier` 与 `assetProtocol.scope.allow` 由测试比对（§12 T2）；该文件是手写配置，按 §15 对齐。
- 前端：不持有任何第一层目录名字面量；asset URL 由后端返回的目录 + 单一模块拼接（§8.4）。

### 6.5 库内子布局的边界

本规格约束的是**基根与第一层目录名**。库内部为自身存储需要而创建的子目录（`attachments/derived/`、`security/backups/`、`mascot-assets/{rive,gif,svg}/`、`tool-results` 内的文件名）由库自己持有，条件是：

- 只在调用方传入的根下创建，不得反推或重算基根；
- 不得调用 `dirs::`，不得读取环境变量决定落点；
- 子目录名不得在库外被当作应用目录名拼串；跨层只传根路径与逻辑 id；
- 子布局变化不改变 §6.1 的归属表。

这条边界的目的是避免把「附件侧车怎么放」「备份怎么放」这类存储细节抬到应用层，同时保住「基根只有一处解析」这一约束。

## 7 受管目录的边界

本层不做「信任」判定。校验是**当前时刻的一次检查**：返回一个已规范化的 `PathBuf`，供紧接着的 IO 使用。校验与 IO 之间的状态变化由 IO 自身承担（`create_new` 排他创建、打开后复核类型），不承诺「校验过的路径此后一直指向同一目标」。

### 7.1 id → 文件名判据

`assistant_storage::attachment_store::is_plain_file_name(&str) -> bool`，在该模块 `pub` 导出。

放这里的理由：它不是目录管理，而是 **id → 文件名映射的不变量**——`AttachmentStore` 自己就是「id 就是文件名」的那个组件，判据必须和构造路径的代码在一起，否则任何调用方都能绕过。依赖上无新增边：`assistant-storage` 无新依赖，`src-tauri` 与 `assistant-agent` 已依赖它。

判据：

- 非空；仅一个 `Normal` 组件；不含 `/` 与 `\`；
- 非 `.`、非 `..`；不以 `.` 开头（隐藏名）；
- 不含 `:`（盘符/ADS）；不含 NUL 与控制字符；
- 不以 `.tmp` 结尾（保留给原子写临时文件）。

落地要求：

- 删除 `src-tauri/src/cmd/storage.rs` 的私有实现，改为调用该函数；
- `AttachmentStore` 的 `read` / `file_path` / `derived_path` / `write_derived_text` / `delete_derived_text` / `delete` **入口即校验**，非法 id 返回新增的 `AttachmentError::InvalidId`；`read_derived_text` 保持 `Option` 返回契约，非法 id 直接返回 `None`、不发起 IO；
- `file_path` 改为返回 `Result<PathBuf, AttachmentError>`，同步 `cmd/storage.rs::open_attachment` 与 `assistant-agent::pdf_text` 两个调用点；
- `derived_path` 可降为私有，由上述侧车方法内部调用，避免外部再拿到一条未校验的路径构造入口；
- `delete_attachments` 先**全量**校验 id 列表，任一非法 → 整个请求返回错误，**不删除任何文件、不删除任何附件引用**（不做部分成功）。

### 7.2 外部文件进入/离开受管目录

`assistant-tools/src/os/path.rs` 新增：

```rust
pub enum ExternalInputMode { Read, Write }

pub fn resolve_external_input(input: &str, mode: ExternalInputMode) -> Result<PathBuf, String>;
```

语义：绝对路径、`canonicalize_lenient`、`reject_system_dirs`（含凭据目录）、拒绝 `..` 段；`Read` 要求 `is_file()`，`Write` 要求目标父目录存在且非系统目录。与 `resolve` 的区别是**不套工作区边界**——这些路径本来就不在工作区内。返回 `String` 失败原因，与 `os/path.rs` 现有函数保持一致。

适用点：

- vault 导入源、vault 导出目标；
- 桌宠素材安装源；
- 附件导入源（§8.1）。

### 7.3 校验即使用（D11）

调用方必须使用 `resolve_external_input` 返回的 `PathBuf`，禁止校验完再用原字符串 `open`/`create`。需要展示原始用户输入时，展示用原字符串，IO 用返回值，两者不混用。

## 8 附件归档与前端资产 URL

### 8.1 新命令 `import_attachment`

```
import_attachment(path: String) -> AttachmentImportDto { attachmentId, mediaType, bytes, fileName }
```

流程：`resolve_external_input(path)` → 扩展名/MIME 白名单 → 大小上限 → 读取 → `AttachmentStore::save_attachment`（内容寻址，`create_new` 排他）→ 返回 id 与后端判定结果。上限与 `src/chat/attachment/types.ts` 的常量保持一致，但以后端判定为准；前端预筛只用于快速失败。

### 8.2 前端改为入库后使用

- `factory.ts` 的 `createImageAttachmentFromPath` / `createFileAttachmentFromPath` 合并为 `createPathAttachment(filePath): Promise<AttachmentResult>`：先做前端扩展名预筛，再调 `import_attachment`，用返回 id 与 `mediaType` 构造附件；
- 构造结果保留本地 `id` 作为 UI key，`attachmentId` 使用后端返回 id；`file` 仍用空 `File` 占位（与现有 path 分支一致），仅承载名称与类型，真实字节在受管目录中；
- `Attachment` 删除 `filePath` 字段；路径附件与粘贴附件统一走 `attachmentId`；`previewUrl` 由 §8.4 的 `attachmentUrl(id)` 生成（此时文件已在 asset 作用域内）；
- `intake.ts` 的 `ingestPath` 改为异步，文件选择与拖放两个调用点 `await` / `void` 适配；
- `contentBlockUtils.ts` 删除 `file_path` 分支，`buildContentBlocks` 与 `buildContentBlocksAsync` 统一只输出 `attachmentId`。

### 8.3 `multimodal` 拒绝 `file_path`

`ContentBlock::Image/File` 的 `file_path` 分支改为返回错误（「附件需重新添加」）。理由：入库后前端不再产出该字段；保留分支等于保留「webview 指定任意绝对路径 → 后端读取并上传」的通道，而该通道没有任何调用方可声明的边界。历史消息在写入前已经过 `admit_attachments`，附件已落盘并写回 `attachment_id`，重放不命中该分支（`attachment_id` 非空即早退）。

同时删除 `MultimodalConfig::allow_absolute_paths` 与 `validate_path`：可信通道不再需要绝对路径分支，不可信通道由附件配额（`max_images = 0`、`max_files = 0`）直接拒绝。

### 8.4 前端唯一拼接点

新增命令 `get_asset_dirs() -> AssetDirsDto { attachmentsDir, mascotAssetsDir }`（展示形态的绝对路径，UTF-8，非 UTF-8 返回错误）。

`src/chat/attachment/paths.ts` 作为**唯一**拼接点：

- 缓存 `get_asset_dirs()` 结果（首次调用拉取）；
- 导出 `attachmentUrl(id)`、`mascotAssetUrl(relPath, version?)`；
- 内部只保留一处 `convertFileSrc` 调用点；
- 删除 `getAttachmentsDir()`，不保留任何前端目录拼串 API。

约束：`ImageThumbnail.tsx`、`FileCard.tsx`、`AssetPickerField.tsx` 以及任何新组件都不得自建 `join` + `convertFileSrc`；前端不出现 `"attachments"` / `"mascot-assets"` 字面量。

## 9 消费点收口清单

| # | 位置 | 现状 | 目标 |
|---|---|---|---|
| B1 | `src-tauri/src/setup.rs` 主库 | `app_data_dir()` + 手拼 `assistant.db`；初始化失败只记日志后继续 | `AppDirs::resolve()?.database()`；解析或初始化失败**中止启动** |
| B2 | `src-tauri/src/setup.rs` 附件目录 | `expect` panic | `attachments()`；`create_dir_all` 失败返回 `Err`，中止启动 |
| B3 | `src-tauri/src/tool_setup.rs` | `app_data_dir()` + `join("tool-results")` | `tool_results()` |
| B4 | `src-tauri/src/cmd/vault/mod.rs` 安全库 | 非 UTF-8 `unwrap_or` 换值；失败回退相对路径 | `security_db_path(&dirs)`（目录名自持于本模块，根取 `AppDirs::data()`）；`to_str().ok_or` 失败；`vault_setup` 返回 `Result`，插件 setup 中止启动 |
| B5 | `src-tauri/src/cmd/vault/mod.rs` vault 目录 | 失败回退相对路径 `security/vault` | `repo_dir(&dirs)`（同上）；失败返回 `Err`，中止启动 |
| B6 | `src-tauri/src/cmd/vault/recovery.rs` | `data_dir()` + `join("security").join("recovery")` | `recovery_dir(&dirs)`（同上） |
| B7 | `src-tauri/src/cmd/mascot_asset.rs` | `data_dir()` + `join("mascot-assets")`；返回绝对 `installedPath` | `mascot_assets()`；DTO 改返回相对 `relPath`（相对素材根） |
| B8 | `crates/assistant-crash/src/crash_dump.rs` | 自算目录、`unwrap_or(".")`、依赖 `dirs` | `install(crash_dir: Option<PathBuf>)`：目录由 `main.rs` 传入；`None` 时只装 stderr panic hook、不装 minidump handler；`create_dir_all` 失败 `eprintln!` 后按 `None` 处理；删除 `dirs` 依赖 |
| B9 | `crates/assistant-config/src/json_config.rs` 等 | 5 个 `get_conf_dir` 自算配置目录 | **保持现状**（§6.4 既有例外） |
| B10 | `src-tauri/src/plugins.rs` 日志 | `TargetKind::LogDir`（默认落在 local） | `TargetKind::Folder { path: logs(), .. }`，与崩溃同根；解析失败时不注册文件 target（开发构建仅 stdout），不回落默认目录 |
| B11 | `src-tauri/src/lib.rs` | — | 声明 `pub mod app_dirs`，供 `main.rs` 与 `plugins.rs` 在 Tauri 初始化前使用 |
| C1 | `src-tauri/src/cmd/vault/entries.rs` 导入/导出 | 源/目标路径零校验 | `resolve_external_input` + 校验即使用 |
| C2 | `src-tauri/src/cmd/mascot_asset.rs` 源路径 | 仅扩展名/MIME/大小 | 追加 `resolve_external_input` |
| C3 | `src-tauri/src/cmd/storage.rs` `delete_attachments` | 非法 id 静默跳过；先删引用再删文件 | 全量校验，非法即整体失败，引用与文件都不动 |
| C4 | `crates/assistant-storage/src/attachment_store.rs` | `join(id)` 直用 | `is_plain_file_name` 入口校验；新增 `InvalidId`；`file_path` 改返回 `Result` |
| C5 | `crates/assistant-vault/src/security_db/mod.rs` 备份目录 | `parent().unwrap_or(".")`，父目录缺失时落 cwd | 要求安全库路径为绝对路径且父目录非空，否则返回错误；仍以调用方传入的路径为根，不新增 `AppDirs` 依赖 |
| A4 | `crates/assistant-agent/src/multimodal.rs` `file_path` 分支 | 绝对路径直读 | 拒绝（§8.3）；删除 `allow_absolute_paths` 与 `validate_path` |
| E1 | `src/chat/message/ImageThumbnail.tsx` | 本地 `join` + `convertFileSrc` | 调 `attachmentUrl(id)` |
| E2 | `src/chat/attachment/factory.ts` | `convertFileSrc(filePath)` 预览 | 改为 `createPathAttachment` 入库后按 id 预览（§8.2） |
| E3 | `src/settings/shared/renderers/AssetPickerField.tsx` | `convertFileSrc(result.installedPath)` | 调 `mascotAssetUrl(relPath, version)` |
| E4 | `src/chat/core/contentBlockUtils.ts` | 无 id 时输出 `file_path` | 删除 `file_path` 分支；附件一律带 `attachmentId` |

## 10 路径形态与编码

### 10.1 verbatim 三态

IO 形态 = `canonicalize` 原样返回（Windows 上带 `\\?\` 前缀）；展示形态 = 剥离前缀后的路径；绑定形态 = 原样字符串，仅用于比较与持久化。约束：

- 展示形态**不得**用于比较、持久化、DB 存储、日志键、缓存键；
- wire 与 UI 用展示形态；IO 用 verbatim 形态；两者只在单一转换点互转；
- 展示形态进 prompt / UI / 日志文本时，必须剥离前缀（现状 `src/chat/core/workspaceDisplay.ts` 已做，Rust 侧需对齐）；
- 剥离前缀只做**前缀**处理，不做大小写归一、不做分隔符替换（后者会破坏比较语义）。

### 10.2 跨语言受控重复（D4 例外登记）

Rust 与 TS 各实现一份 `strip_verbatim_prefix`，属**受控重复**，条件是共用同一组测试向量：

- 向量文件：`src-tauri/tests/fixtures/verbatim-form-vectors.json`；
- Rust 测试与前端 vitest 均读该文件，逐条断言同一输入 → 同一输出；
- 任一侧新增用例必须改该文件，另一侧无需改代码即可覆盖。

### 10.3 非 UTF-8

跨 wire 的路径字段（DTO）必须是 UTF-8：

- 不可转换时返回错误，**禁止** `to_string_lossy()` 写入 wire、DB、比较键；
- `Path::display()` 只允许出现在日志与 prompt 文本，且先经 §10.1 剥离；
- 现状待修：`cmd/vault/mod.rs` 的 `to_str().unwrap_or(...)`、`cmd/mascot_asset.rs` 的 `to_string_lossy()`（B7 改为相对路径后消除）。

### 10.4 长路径（D15 平台兼容）

目标：不设长度上限、不截断、不因长度拒绝。超过 260 的路径正常读写。

手段：IO 侧一律使用 verbatim 形态。Windows 上 Rust std 的文件操作在调用 Win32 API 前会把路径转成 `\\?\` 形态，因此经 std 的读写不受 260 限制，**不需要**注册表 `LongPathsEnabled`，**不需要**应用清单 `longPathAware`。`canonicalize` 返回的 verbatim 路径直接用于 IO，展示与 wire 走 §10.1 的剥离形态。

禁止：

- 按长度截断或按长度拒绝；
- 为长度换目录、降级到短路径；
- 把 `\\?\` 写进 wire 字段或 DB。

限制登记：不经 Rust std 的组件（webview asset 协议、系统打开器）可能仍受 260 限制。此时按 §11「需用户决策」报错并提示路径过长，不降级、不换路径。

## 11 失败策略映射（D3）

| 场景 | 分类 | 处置 |
|---|---|---|
| 启动期解析 data/local 基根失败 | 应上报 | `eprintln!`（日志插件尚未初始化）后中止启动；不回退相对路径 |
| 启动期创建 `attachments/` / `tool-results/` 失败 | 应上报 | 中止启动（当前 `attachments` 为 `expect`，改为 `Err`） |
| 主库解析或初始化失败 | 应上报 | 中止启动（当前只记日志后继续，后续命令全部失配） |
| vault 安全库/vault 目录解析失败 | 应上报 | vault 插件 setup 返回 `Err`，中止启动；不回落相对路径 |
| 运行期 `import_attachment` 源不可读/超限/类型不允许 | 需用户决策 | 返回错误码，前端提示后由用户换文件 |
| vault 导入源 / 导出目标不合法 | 需用户决策 | 返回错误，不落盘 |
| `delete_attachments` 任一 id 非法 | 应上报 | 整体失败，不删文件、不删引用 |
| 单个附件文件删除失败（IO） | 可恢复 | 继续其余，回传统计（可感知，不静默） |
| 路径过长且组件不支持 | 需用户决策 | 报错提示路径过长，不截断、不换目录 |
| 日志 / 崩溃转储写入失败 | 可降级 | 仅 stderr（`eprintln!`），不写 cwd、不改目录 |
| 路径非 UTF-8 | 需用户决策 | 返回错误，由用户另存为可表示路径 |

## 12 测试与防回归

Rust：

- T1 `app_dirs` 单测：归属表逐项（`with_bases` 注入后断言 `database`/`attachments`/`tool_results`/`mascot_assets` 四个 data 根 getter 与 `logs`/`crashes` 两个 local 根 getter）；`asset_scopes()` 字符串；`resolve()` 幂等。模块内无 `#[cfg(feature = ...)]` 由 a2 门禁守护，不重复断言。
- T2 漂移测试：读 `tauri.conf.json`，断言 `identifier == APP_IDENTIFIER`，`assetProtocol.scope.allow` 与 `asset_scopes()` 集合相等（顺序无关）。
- T3 `is_plain_file_name` 向量：`""`、`..`、`a/b`、`a\b`、`.hidden`、`x.tmp`、`C:x`、`a:b`、含 NUL、正常 `abc.png`。
- T4 `resolve_external_input`：`Read` 下相对路径、`..`、系统目录、凭据目录、非文件均拒绝；`Write` 下父目录不存在拒绝；正常绝对路径通过。
- T5 `AttachmentStore` 非法 id：`read` / `file_path` / `write_derived_text` / `delete_derived_text` / `delete` 返回 `InvalidId`；`read_derived_text` 返回 `None` 且不发起 IO（`derived_path` 若保留公开则一并覆盖）。
- T6 `delete_attachments` 含非法 id：tempdir 集成测试，断言返回错误、附件文件全在、`attachment_refs` 未被删除。
- T7 vault 导入/导出非法路径、安全库备份父目录缺失：断言错误且目标未创建/未被覆盖，不在 cwd 创建 `backups/`。
- T8 `install_mascot_asset` 源为系统目录：拒绝。
- T9 `import_attachment`：超限、非白名单、源不存在各返回对应错误。
- T10 `multimodal` 带 `file_path` 的 Image/File 块：返回错误，且不读文件。
- T11 崩溃安装：`install(None)` 不创建任何目录、不写文件；`install(Some(dir))` 以该目录为根写入；两者都不使用进程 cwd。
- T12 长路径：tempdir 下构造总长超过 260 的目录链，`save_attachment` / `read` / `delete` 全部成功，断言路径未被截断。
- T13 基根与编码：`with_bases` 注入后断言 `logs()` 与 `crashes()` 同属 local 根且不等于 data 根；注入非 UTF-8 基根（Unix）时 `get_asset_dirs` 返回错误，不输出 lossy 字符串。

前端（vitest）：

- T14 `paths.ts` 只导出既定 API，且内部 `convertFileSrc` 调用唯一（结构测试）。
- T15 `attachmentUrl` / `mascotAssetUrl` 使用 `get_asset_dirs` 返回值（mock 命令）。
- T16 共用向量（§10.2）逐条断言 `stripVerbatimPrefix`。
- T17 `createPathAttachment`：`import_attachment` 失败时返回 `AdmitError`，不构造附件。
- T18 `contentBlockUtils` 在无 id 时不产出 `file_path`。

## 13 验收清单

- A1 目录字面量检索（`join("attachments"|"tool-results"|"mascot-assets"|"logs"|"crashes")` 与 `"assistant.db"`）在 `src-tauri/src`、`src/` 只命中 `app_dirs::dir_name` 常量定义与测试向量；`"security"`、`"security.db"`、`"vault"`、`"recovery"` 只命中 `cmd/vault` 内的局部常量定义。
- A2 全仓库无 `unwrap_or(".")` / `unwrap_or_else(|| PathBuf::from("."))` 形式的路径回退。
- A3 `dirs::` 只出现在 `src-tauri/src/app_dirs/` 与 `assistant-config`（§6.4 既有例外）；`crates/` 下其余库无 `dirs` 依赖。
- A4 asset 作用域与 `asset_scopes()` 一致，`attachments` 为 `*` 不覆盖 `derived/`；附件与桌宠预览均来自 asset 协议，前端无本地拼接点。
- A5 `multimodal` 不再读取任何 `file_path`，`allow_absolute_paths` / `validate_path` 已删除。
- A6 `delete_attachments` 非法 id 不产生部分删除，也不删除引用。
- A7 vault 导入/导出、素材安装的源路径均过 `resolve_external_input`。
- A8 `tauri.conf.json` 与常量漂移测试通过（T2）。
- A9 崩溃与日志同根；`assistant-crash` 的 `Cargo.toml` 已无 `dirs`。
- A10 非 UTF-8 路径在所有 wire 出口返回错误，无 `to_string_lossy` 写入。
- A11 T1–T18 全部通过。
- A12 长路径用例（T12）通过，且代码中无长度判断分支。
- A13 未新增任何 crate；workspace 内 crate 间依赖边为 0；依赖声明变化仅两处：应用 crate 补 `dirs`（第三方、已在编译图），`assistant-crash` 去掉 `dirs`。
- A14 `npm run contract:gen` 已执行并提交生成物（新增/修改的 DTO：`AssetDirsDto`、`AttachmentImportDto`、`InstallMascotAssetResultDto` 的 `relPath`）。
- A15 `ARCHITECTURE.md` §5.2 的文件结构已补齐 `tool-results/`、`security/recovery/`、`security/backups/`、`attachments/derived/`，崩溃目录已改为 local 根。

## 14 交付段（按依赖方向排序）

按**依赖方向**分五段，而非按问题性质分三批。三条硬约束：每个文件在整条链上只改一次；破坏性签名与全部调用点同段（每段结束 `cargo check --all-targets` 必须干净）；唯一性先行（`app_dirs` 先落地）。

```
S1 app_dirs 骨架（无消费者，纯新增）
 ├──> S2 启动链收口
 ├──> S3 库内不变量 ──> S4 边界+入库+前端 ──> S5 形态
 └──(S2 与 S3 可并行；S4 依赖 S1+S3；S5 依赖 S4)
```

**S1 基座**（§6.2、B11、T1）

- 新增 `src-tauri/src/app_dirs/`（`mod.rs` + `dir_name.rs` + `roots.rs`），`lib.rs` 声明 `pub mod app_dirs`，应用 crate 补 `dirs = { workspace = true }`；
- 不接任何消费点，`asset_scopes()` 暂不与 `tauri.conf.json` 一致（T2 在 S2 落地，避免出现不可通过的中间态）。

**S2 启动链收口**（B1–B6、B8、B10 + 作用域收窄 + T2、T11）

- `setup.rs`（B1/B2）、`tool_setup.rs`（B3）、`cmd/vault`（B4/B5，目录名自持）、`cmd/vault/recovery.rs`（B6）、`assistant-crash` + `main.rs`（B8，同段删 `dirs` 依赖）、`plugins.rs`（B10）；
- `tauri.conf.json` 的 `attachments/**` → `attachments/*`；T2 落新增 `src-tauri/tests/app_dirs_contract.rs`；T11 落 `crash_dump.rs` 的 `#[cfg(test)]`（用 `std::env::temp_dir()`，不给崩溃 crate 加 dev-dependency）。

**S3 库内不变量**（C3、C4、C5、A4 + `resolve_external_input` + T3–T5、T7、T10）

- C4 `attachment_store.rs`（判据下沉、`InvalidId`、`file_path → Result`）与两个调用点（`open_attachment`、`pdf_text`）同段；
- C3 并入本段：`file_path` 改签名会让 `cmd/storage.rs` 立刻不可编译，`delete_attachments` 必须同时修好，该文件只在 S3 被改一次；
- C5 `security_db/mod.rs`；A4 `multimodal.rs`；新增 `assistant-tools/src/os/path.rs::resolve_external_input`。

**S4 边界 + 入库 + 前端**（C1、C2、B7、E1–E4 + 新命令 + DTO + `contract:gen`）

- C1 `cmd/vault/entries.rs`、C2+B7 `cmd/mascot_asset.rs` 同段（都改该文件，`contract:gen` 只跑一次）；
- 新命令 `import_attachment`（新模块 `cmd/attachment.rs`，不回头改 `cmd/storage.rs`）与 `get_asset_dirs`；DTO `AssetDirsDto`、`AttachmentImportDto`、`InstallMascotAssetResultDto::relPath`；新 DTO 必须登记进 `tests/protocol_bindings.rs` 的 import 列表；
- 前端 `paths.ts` 成为唯一拼接点，`factory.ts` 合并为 `createPathAttachment`，`intake.ts` 异步，`contentBlockUtils.ts` 删 `file_path` 分支，`ImageThumbnail.tsx` / `AssetPickerField.tsx` 改调 URL 函数；T9、T14–T18。

**S5 形态**（§10 + T12、T16）

- 新增 `app_dirs/form.rs::strip_verbatim_prefix`；共用向量 `src-tauri/tests/fixtures/verbatim-form-vectors.json`；Rust 与 vitest 同读一份；
- 非 UTF-8 全 wire 出口排查；T12 长路径用例落 `assistant-storage` tests（该 crate 已有 `tempfile` dev-dependency），清理长度分支。

与原三批的差异及理由：

| # | 原批次 | 现位置 | 理由 |
|---|---|---|---|
| 1 | B8 批 1 用 `dirs` 临时算目录、批 2 换 `AppDirs` | 直接进 S2 | `AppDirs` 在 S1 已可用，临时实现只多一轮验证成本 |
| 2 | B7 批 2、C2 批 1 | 同进 S4 | 两者都改 `mascot_asset.rs`，合并后该文件只改一次 |
| 3 | C3 批 1 | 并入 S3 | 与 C4 同文件同依赖，拆开会让段内不可编译 |
| 4 | 作用域收窄在批 2 靠前 | 与 B1–B6 同段（S2） | 作用域由 `asset_scopes()` 生成，先收窄再定义常量会留下 T2 不可通过的窗口 |
| 5 | §10 形态批 3 | S5 | 展示形态第一次跨 wire 在 S4 的 `AssetDirsDto`；剥离前缀须在该出口存在之后对齐 |

## 15 落地时须同步的文档

- `docs/ARCHITECTURE.md` §5.2：补齐 `tool-results/`、`security/recovery/`、`security/backups/`、`attachments/derived/`；崩溃目录由 data 根改为 local 根（与日志同根）；不新增 crate。
- `src-tauri/tauri.conf.json`：`assetProtocol.scope.allow` 与常量对齐（由 T2 守护），`attachments` 使用 `*`。
- `src/types/protocol.contract.ts` / `src/vault/core/contract/vault.contract.ts`：`npm run contract:gen` 生成，禁止手改。

## 16 取舍记录

| 取舍 | 结论 | 理由 |
|---|---|---|
| `dirs` 还是 `AppHandle::path()` | 用 `dirs` | 日志 target、崩溃安装发生在 Tauri 初始化前；`dirs` 任意时刻可用，落点与 `app_data_dir()` 一致 |
| 崩溃目录放 data 还是 local | local | 与日志同根，诊断数据集中；旧目录不迁移 |
| `attachments` 作用域用 `**` 还是 `*` | `*` | 附件原文件在第一层；`derived/` 侧车不应经 asset 协议暴露 |
| 配置目录是否纳入 `app_dirs` | 不纳入 | 已有 5 个 `get_conf_dir` 计算点；纳入会造成双实现或第 6 个计算点 |
| 库内子布局是否算应用目录 | 不算 | 存储细节留在库内；本规格只约束基根与第一层目录名 |
| vault 目录名放 `app_dirs` 还是 `cmd/vault` | `cmd/vault` | a2 门禁禁止 `app_dirs` 出现 `#[cfg(feature = ...)]`；放进去会让关闭特性后路径知识仍留在二进制里。唯一性改为「字面量位置 = 所有者」 |
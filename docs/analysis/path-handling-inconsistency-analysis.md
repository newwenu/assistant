# 路径处理不统一分析（前端 / Rust）

> 分析对象：当前工作区代码（前端 `src/`，后端 `src-tauri/`，配置 `src-tauri/tauri.conf.json`、`src-tauri/capabilities/`）
> 分析方式：静态检索 + 调用链人工追溯
> 关联文档：`docs/store/I31-phase1-tool-framework-spec.md` §5（共享路径安全 `os/path.rs`）、`docs/store/I18-agent-tool-architecture-spec.md` Step 2（文件系统工具）、`docs/analysis/PROJECT-DESIGN-GUIDELINES.md` §1.3（Rust cmd 薄壳）
> 性质：**分析**，不含实施步骤；整改建议见 §9

## 目录

- [路径处理不统一分析（前端 / Rust）](#路径处理不统一分析前端--rust)
  - [目录](#目录)
  - [0 结论摘要](#0-结论摘要)
  - [1 分析口径](#1-分析口径)
    - [1.1 检索矩阵](#11-检索矩阵)
    - [1.2 判定维度](#12-判定维度)
    - [1.3 风险等级](#13-风险等级)
    - [1.4 方法局限](#14-方法局限)
  - [2 路径基址全景](#2-路径基址全景)
  - [3 受信路径清单（应用自算目录）](#3-受信路径清单应用自算目录)
  - [4 不可信路径清单（外部输入）](#4-不可信路径清单外部输入)
  - [5 六个不统一维度](#5-六个不统一维度)
    - [5.1 基址解析分散](#51-基址解析分散)
    - [5.2 目录名字面量散落](#52-目录名字面量散落)
    - [5.3 类型三态](#53-类型三态)
    - [5.4 失败策略五种](#54-失败策略五种)
    - [5.5 编码转换四处](#55-编码转换四处)
    - [5.6 校验覆盖缺口](#56-校验覆盖缺口)
  - [6 前端侧现状](#6-前端侧现状)
  - [7 与既有规格的关系](#7-与既有规格的关系)
  - [8 缺陷清单（分级）](#8-缺陷清单分级)
  - [9 整改建议：分两层，不要合并](#9-整改建议分两层不要合并)
    - [A. 受信目录层（`assistant-config` 内新增，或独立 crate）](#a-受信目录层assistant-config-内新增或独立-crate)
    - [B. 不可信路径层（`assistant-tools/src/os/path.rs`，I31 T3）](#b-不可信路径层assistant-toolssrcospathrsi31-t3)
    - [C. 边界类型约定](#c-边界类型约定)
    - [D. 不做什么](#d-不做什么)
  - [10 防回归与验收](#10-防回归与验收)
    - [10.1 机制性防回归](#101-机制性防回归)
    - [10.2 验收标准](#102-验收标准)
    - [10.3 建议推进顺序](#103-建议推进顺序)

---

## 0 结论摘要

1. **只有一个环节真正统一**：JSON 配置文件的路径计算与落盘（`assistant-config` 的 `get_conf_dir()` + `load_json_config()` + `atomic_write()`）。它覆盖 `config.json` / `orb-config.json` / `mascot-config.json` / `vault-config.json` 四类文件，是项目里唯一「路径只在一个地方算」的实现。
2. **应用数据目录没有统一层**。`assistant.db`、`attachments`、`security/security.db`、`security/vault`、`security/recovery`、`mascot-assets`、`crashes`、`logs` 各自在调用点 `基址.join("字面量")`，基址来自 **4 套互不相同的解析器**，失败策略有 **5 种**，类型有 **3 种**。
3. **同一个目录名在 4 个地方各写一遍**：`attachments` 同时出现在 Rust 初始化、前端 `paths.ts`、`tauri.conf.json` 的 asset 协议作用域、`capabilities/default.json` 的 opener 作用域。改一处必然漏三处。
4. **用户/模型/前端输入的路径基本没有统一校验**。唯一的校验函数是 `fs_link.rs` 的私有 `validate_path`（存在规范化降级缺陷），而 `vault_export_to_file` / `vault_import` / `delete_attachments` / `install_mascot_asset` 都没有等价校验。其中 `delete_attachments` 的 `id` 直接 `join` 进目录，存在路径穿越删除能力。
5. **展示形态与 IO 形态未分离**。同一个 `String` 既进日志、又进 UI、又喂给 `std::fs` / `openPath` / `git` / `mklink`。这既是长路径无法支持的结构原因（项目当前无任何长路径设计），也是 asset 协议作用域失效的成因。
6. 结论：**不是「缺一个 path_utils」，而是缺两个层次**——「受信目录解析层」与「不可信路径校验层」。两者合并会同时破坏安全边界与可维护性，见 §9。

---

## 1 分析口径

### 1.1 检索矩阵

| 语言 | 检索模式 | 命中 | 说明 |
|------|---------|------|------|
| Rust | `app_data_dir\|app_config_dir\|dirs::` | 16 | 全部路径基址入口 |
| Rust | `to_string_lossy\|to_str()\|display()\|PathBuf::from\|.join(`（`src-tauri/src`） | 21 | 转换与拼接点 |
| Rust | `MAX_PATH\|longPathAware\|LongPathsEnabled\|\\\\?\\` | 0 | 无长路径设计（见 §5.6 附注） |
| Rust | `resource_dir\|current_exe\|temp_dir`（业务代码） | 0 | 业务代码不读资源目录与 cwd（测试除外） |
| 前端 | `appDataDir\|appConfigDir\|@tauri-apps/api/path` | 3 文件 | 仅 `paths.ts`、`ImageThumbnail.tsx` |
| 前端 | `convertFileSrc\|openPath\|revealItemInDir` | 4 文件 | 三类跨界调用 |
| 配置 | `assetProtocol\|scope\|opener:allow-open-path` | 2 文件 | 路径作用域第二处定义 |

说明：命中数只用于定位，不等于缺陷数。例如 `to_string_lossy` 在日志上下文（`display()`）中是合理的，在跨 wire 传输上下文（`installed_path`）中则是缺陷。

### 1.2 判定维度

| 维度 | 判定问题 |
|------|---------|
| D1 基址 | 同一个「应用目录」由谁解析？是否只有一个解析器？ |
| D2 常量 | 目录名 / 文件名是否集中定义？ |
| D3 类型 | 边界上传递 `PathBuf`、`String`、`&str` 哪一种？是否可逆？ |
| D4 失败 | 解析失败、创建失败、读写失败各自怎么处理？是否留痕？ |
| D5 编码 | 非 UTF-8 路径怎么处理？lossy、丢弃、报错？ |
| D6 校验 | 外部输入路径是否校验？校验点是否可复用？ |
| D7 形态 | 展示形态与 IO 形态是否区分？ |

### 1.3 风险等级

| 等级 | 定义 |
|------|------|
| P0 | 安全边界失效（路径穿越、越权删除/写入）或数据静默分叉 |
| P1 | 用户可感知的功能失败但无反馈；目录名不一致导致「写在这里、读在那里」 |
| P2 | 可维护性缺陷（重复字面量、类型往返、失败策略不一致），当前行为正确 |
| P3 | 合理设计，无需整改（如 `display()` 用于日志、测试用 `temp_dir()`） |

### 1.4 方法局限

- 静态分析，未做运行时路径注入验证；P0 判定基于「命令是否可从 Webview 调用 + 参数是否直达 `join`」的代码可达性推断。
- 未覆盖 `references/` 参考项目与 `temp-backup/`。
- 未验证 Windows 上 `dirs::config_dir()` / `app_data_dir()` / 插件 LogDir 的实际落点差异在跨平台构建中的表现（仅按 crate 文档与 API 语义推断，见 §2 注）。
- 未逐条确认每个 Tauri command 的权限声明，仅以「命令已注册 + 前端可 invoke」为可达性依据。

---

## 2 路径基址全景

项目实际存在 **4 套基址解析器**，Windows 上其中 3 套恰好收敛到同一物理目录，掩盖了不一致：

| # | 基址解析器 | 调用方式 | Windows 落点（推断） | 使用方 |
|---|-----------|---------|---------------------|--------|
| B1 | `dirs::config_dir()` | `assistant-config/src/json_config.rs:8-13` | `%APPDATA%\com.assistant` | 全部 JSON 配置 |
| B2 | Tauri `app_data_dir()` | `src/setup.rs:48`、`src/setup.rs:66`、`src/cmd/vault/mod.rs:196`、`src/cmd/vault/mod.rs:213`、`src/cmd/vault/recovery.rs:69`、`src/cmd/mascot_asset.rs:38` | `%APPDATA%\com.assistant` | 主库、附件、安全库、vault、灾备、桌宠资产 |
| B3 | `dirs::data_dir()` | `crates/assistant-crash/src/crash_dump.rs:6` | `%APPDATA%\com.assistant` | 崩溃转储 |
| B4 | `tauri-plugin-log` 默认 `TargetKind::LogDir` | `src/plugins.rs:7`（未显式配置） | `%LOCALAPPDATA%\com.assistant\logs` | 运行日志 |

关键事实：

- **B1/B2/B3 在 Windows 上重合，在 Linux 上不重合**（`config_dir` = `~/.config`，`data_dir` = `~/.local/share`）。这不是「统一」，是巧合。
- **B4 与 B1~B3 在 Windows 上就不重合**（Local 与 Roaming 不同）。日志与崩溃转储落在两个不同根目录，用户排查问题时容易找错位置。
- `crash_dump.rs:6` 硬编码字符串 `"com.assistant"`，未引用 `assistant-config` 的 `APP_IDENTIFIER`（`crates/assistant-config/src/constant.rs:1`）。原因是 crash crate 若依赖 config crate 会引入循环/多余依赖，但代价是标识符双写。
- 前端还有第 5 套：`appDataDir()`（`src/chat/attachment/paths.ts:8`），由 Tauri 前端 API 独立解析，与 B2 语义相同但实现分离。

---

## 3 受信路径清单（应用自算目录）

这类路径由程序自己算出，输入可信，**不需要 `..` 校验**，但需要「唯一来源 + 失败策略一致」。

| 用途 | 基址 | 拼接结果 | 位置 |
|------|------|---------|------|
| 应用配置 | B1 | `config.json` | `assistant-config/src/app_conf.rs:78-80` |
| 悬浮球配置 | B1 | `orb-config.json` | `assistant-config/src/orb_conf.rs:46` |
| 桌宠配置 | B1 | `mascot-config.json` | `assistant-config/src/mascot_conf.rs:57` |
| 密码库配置 | B1 | `vault-config.json` | `assistant-config/src/vault_conf.rs:73-75` |
| 主库 | B2 | `assistant.db` | `src/setup.rs:50` |
| 附件库 | B2 | `attachments/` | `src/setup.rs:67` |
| 附件文件 | 附件库 | `<sha256>.<ext>` | `assistant-storage/src/attachment_store.rs:97-98` |
| 附件派生文本 | 附件库 | `derived/<stem>.extract-v1.txt` | `assistant-storage/src/attachment_store.rs:139-144` |
| 安全库 | B2 | `security/security.db` | `src/cmd/vault/mod.rs:197` |
| Vault 目录 | B2 | `security/vault/` | `src/cmd/vault/mod.rs:214` |
| Vault 条目 | Vault 目录 | `entries/<id>.enc` | `assistant-vault/src/vault_file/mod.rs:49-59` |
| Vault 元数据 | Vault 目录 | `.vault-meta` | `assistant-vault/src/vault_file/mod.rs:219` |
| 灾备仓库 | B2 | `security/recovery/` | `src/cmd/vault/recovery.rs:71` |
| 桌宠资产 | B2 | `mascot-assets/<dir>/<file>` | `src/cmd/mascot_asset.rs:39`、`src/cmd/mascot_asset.rs:111-113` |
| 桌宠清单 | 桌宠资产 | `rive/custom-manifest.json` | `src/cmd/mascot_asset.rs:143` |
| 崩溃转储 | B3 | `crashes/panic_*.log` | `crates/assistant-crash/src/crash_dump.rs:6` |
| 运行日志 | B4 | `logs/` | `src/plugins.rs:7` |

观察：受信路径本身**没有安全风险**，问题在于「谁算的」「算失败怎么办」「目录名写在哪」三者都不统一。

---

## 4 不可信路径清单（外部输入）

这类路径来自用户对话框、拖拽、前端参数或模型输出，**必须校验**。

| 入口 | 输入形态 | 现有校验 | 位置 |
|------|---------|---------|------|
| `fs_link` 的 `source` / `target` | 任意字符串（绝对或相对） | `validate_path`：`..` 拒绝 + canonicalize + 系统目录前缀 | `assistant-tools/src/os/fs_link.rs:157-175`、`assistant-tools/src/os/fs_link.rs:244-287` |
| `vault_export_to_file` | 对话框返回的绝对路径 | **无** | `src/cmd/vault/entries.rs:303-347` |
| `vault_import` | 对话框返回或手填的绝对路径 | **无** | `src/cmd/vault/entries.rs:354-361` |
| `delete_attachments` | `Vec<String>` 附件 id | **无** | `src/cmd/storage.rs:128-131`、`src/cmd/storage.rs:159` |
| `install_mascot_asset` | 前端传入的源文件路径 | 仅扩展名 + MIME + 大小 | `src/cmd/mascot_asset.rs:74-108` |
| 拖拽图片 | Tauri 拖放事件的 `paths` | **无**（前端直接 `convertFileSrc`） | `src/chat/attachment/sources/dragDrop.ts:15`、`src/chat/attachment/factory.ts:67-79` |
| 前端打开附件 | 附件 id 字符串 | **无**（前端 `join` 后 `openPath`） | `src/chat/attachment/paths.ts:14-18` |

`fs_link` 的校验虽然存在，但两处已知缺陷（I31 已记录）：

- `assistant-tools/src/os/fs_link.rs:255` 用 `canonicalize().unwrap_or_else(|_| path.to_path_buf())`：目标不存在时静默回退原路径，校验对象与实际使用对象不一致。
- `assistant-tools/src/os/fs_link.rs:270-283` 用 `to_string_lossy().starts_with("C:\\Windows")` 做保护目录判断：大小写敏感、无分隔符边界、写死盘符。
- `assistant-tools/src/os/fs_link.rs:330-370` 的 junction 走 `cmd /c mklink /J`，路径以原始形态进外部进程。

---

## 5 六个不统一维度

> 「展示形态 vs IO 形态」（§1.2 的 D7）不单列小节：它同时体现在 §5.3 的类型往返与 §5.5 的编码转换中，是这两者的共同根因。

### 5.1 基址解析分散

见 §2。核心问题不是「有 4 个解析器」，而是**没有任何代码层约束阻止第 5 个出现**。业务代码可以直接写 `app.path().app_data_dir()`，编译器不会提示「应走统一入口」。

### 5.2 目录名字面量散落

`attachments` 一个名字出现 4 次，分布在 4 个技术栈：

| 位置 | 形态 |
|------|------|
| `src/setup.rs:67` | Rust 字符串 |
| `src/chat/attachment/paths.ts:8` | 前端字符串 |
| `src-tauri/tauri.conf.json:41-44` | asset 协议作用域 `$APPDATA/attachments/**` |
| `src-tauri/capabilities/default.json:33-35` | opener 作用域 `$APPDATA/attachments/*` |

`mascot-assets` 同样出现 3 次（`src/cmd/mascot_asset.rs:39`、`tauri.conf.json:41-44`、前端经 `installedPath` 间接依赖）。

后果：

- 改名必须同时改 4 处，漏一处 → 图片能读不能显示（asset 作用域不匹配），或能显示不能打开（opener 作用域不匹配）。
- 作用域是「白名单字符串」，不是编译期检查，漏改不会报错，只会在运行期表现为「403 / 打开失败」。

### 5.3 类型三态

| 类型 | 使用位置 |
|------|---------|
| `PathBuf` | `AttachmentStoreConfig.dir`（`assistant-storage/src/attachment_store.rs:8-10`）、`GitSync::repo_path`（`assistant-sync/src/git_sync/mod.rs:16`）、`VaultFileManager.vault_dir`（`assistant-vault/src/vault_file/mod.rs:45`） |
| `String` | `vault_export_to_file(path: String)`（`src/cmd/vault/entries.rs:305`）、`vault_import(path: String)`（`src/cmd/vault/entries.rs:356`）、`install_mascot_asset(file_path: String)`（`src/cmd/mascot_asset.rs:77`）、`InstalledMascotAssetDto.installed_path` |
| `&str` | `SecurityDbState::new(&str)`（`src/cmd/vault/mod.rs:203-205`） |

类型往返造成的具体问题：

- `src/cmd/vault/mod.rs:204`：`security_db_path.to_str().unwrap_or("security/security.db")` —— 非 UTF-8 时**静默换成另一个路径**，不是报错，是数据分叉。
- `src/cmd/mascot_asset.rs:126`：`installed_path: target_path.to_string_lossy().to_string()` —— 非 UTF-8 被替换字符，前端拿到的路径无法再用于 `convertFileSrc`。
- `src/cmd/vault/entries.rs:361`：`PathBuf::from(&path)` —— 字符串重新解析，Windows 上前导/尾随空格、正斜杠、`\\?\` 都会被重新解释。
- `src/cmd/mascot_asset.rs:80`：同上。

### 5.4 失败策略五种

同一个「取不到应用目录」，五处行为不同：

| 策略 | 位置 | 后果 |
|------|------|------|
| 记日志，继续启动 | `src/setup.rs:48-57`（主库） | 应用起来了但没有数据库，后续所有 DB 操作失败 |
| 记日志，中止启动 | `src/setup.rs:66-72`（附件） | 用户看到启动失败，但不知道是目录问题 |
| `expect()` 直接 panic | `src/setup.rs:73` | 崩溃，且崩溃转储本身可能也写不进去 |
| 回退相对路径 | `src/cmd/vault/mod.rs:200`、`src/cmd/vault/mod.rs:215` | 在**进程 cwd** 下建库，用户下次启动可能换目录 → 数据分叉 |
| 回退 `.` | `crates/assistant-crash/src/crash_dump.rs:6` | 崩溃日志写进安装目录或 cwd，可能无写权限 |

其中「回退相对路径」是最高危的一类：它把「解析失败」翻译成「换一个地方存」，且不产生任何用户可见提示。`security/vault` 落在 cwd 意味着密码库数据可能出现在任意启动目录下。

### 5.5 编码转换四处

| 转换 | 位置 | 是否可逆 |
|------|------|---------|
| `to_string_lossy()` 跨 wire | `src/cmd/mascot_asset.rs:126` | 否（替换为 U+FFFD） |
| `to_str().unwrap_or(默认值)` | `src/cmd/vault/mod.rs:204` | 否（换值） |
| `display()` 进日志 | `src/cmd/vault/recovery.rs:77`、`src/cmd/vault/recovery.rs:171` | 不适用（只读） |
| 前端 `join(dir, id)` | `src/chat/attachment/paths.ts:16`、`src/chat/message/ImageThumbnail.tsx:52-56` | 是（但依赖前端解析规则） |

在 Windows 上非 UTF-8 路径罕见，所以这些缺陷长期不暴露；一旦用户目录含特殊字符（或未来支持 Linux/macOS 的编码差异），行为会不可预测。

### 5.6 校验覆盖缺口

- **校验函数不可复用**：`validate_path` 是 `fs_link.rs` 的私有函数（`assistant-tools/src/os/fs_link.rs:244`），第二个文件系统工具无法调用。I31 §5 已规划抽出 `os/path.rs`，但**尚未创建**（`assistant-tools/src/os/` 当前只有 `fs_link.rs` 与 `process_output.rs`）。
- **校验策略不统一**：`fs_link` 校验「系统目录 + `..`」，vault 导入导出不校验，附件 id 不校验，桌宠只校验扩展名。同属「外部输入」，保护等级不同。
- **校验时机不统一**：`fs_link` 在校验后仍使用**未规范化**的 `path`（`assistant-tools/src/os/fs_link.rs:160-166` 返回 `path` 而非 `canonical`），校验对象与使用对象分离。

**附注：长路径**。全仓库无 `MAX_PATH` / `longPathAware` / `LongPathsEnabled` / `\\?\` 相关代码，`src-tauri/build.rs` 使用默认 manifest（未声明 `longPathAware`），`git` 调用未设置 `core.longpaths`（`assistant-sync/src/git_sync/git.rs:587-596`）。当前所有路径默认受 260 字符约束。而 I31 §5.2 要求「`canonicalize()` 返回的 `\\?\` 前缀必须剥离」，这与「未来支持长路径」方向相反。若要在未来支持，必须采用**双形态模型**（IO 形态 `\\?\` / 展示形态普通路径），见 §9。

---

## 6 前端侧现状

前端只有一个真正的路径模块：`src/chat/attachment/paths.ts:1-18`，负责 `appDataDir() + join("attachments")` 与 `openPath`。其余为散点：

| 位置 | 行为 | 问题 |
|------|------|------|
| `src/chat/attachment/factory.ts:67-79` | 对拖拽路径直接 `convertFileSrc(filePath)` | 该路径不在 asset 作用域内（`tauri.conf.json:41-44` 只允许 `$APPDATA/attachments/**`），预览必然失败 |
| `src/chat/message/ImageThumbnail.tsx:52-56` | 自己再拼 `join(getAttachmentsDir(), id)` | 与 `paths.ts` 的拼接规则重复 |
| `src/settings/shared/renderers/AssetPickerField.tsx:32` | `convertFileSrc(installedPath) + "?v="` | 依赖 Rust 侧 `to_string_lossy` 的字符串，编码缺陷传导 |
| `src/chat/attachment/paths.ts:14-18` | `openPath(join(dir, id))` | id 未校验，依赖后端不生成异常 id |

结构性问题：**Rust 的 `app_data_dir()` 与前端 `appDataDir()` 是两个独立解析器对同一语义的各自实现**，只靠字符串约定（`attachments`）对齐，无共享常量、无契约测试。

---

## 7 与既有规格的关系

| 规格 | 已覆盖 | 未覆盖 |
|------|--------|--------|
| `docs/store/I31-phase1-tool-framework-spec.md` §5 | `canonicalize_lenient`、`reject_system_dirs`、`ensure_within_root`、Windows 路径形态（盘符相对、UNC、符号链接 root） | 应用目录统一（§2/§3 的 B1~B4）；`\\?\` 双形态；外部进程（`git` / `mklink`）路径形态 |
| `docs/store/I18-agent-tool-architecture-spec.md` Step 2 | 「路径校验必须落在 `os/path.rs`，不得内联」 | 受信目录层；`workspace_root` 的规范化时机 |
| `docs/store/I27-tool-data-store-spec.md` | `tool_data_dir` 的绝对路径与 `..` 校验 | 长度约束；`tool_data_dir` 与 `files/<tool_id>` 的拼接校验 |
| 无 | — | 附件 id / vault 导入导出 / 桌宠源路径的校验 |

也就是说：现有规格只规划了「工具层」（不可信路径），**完全没有规划「应用目录层」**（受信路径）。本分析的 §3 与 §9-A 是这部分空白。

---

## 8 缺陷清单（分级）

| 编号 | 等级 | 缺陷 | 位置 | 说明 |
|------|------|------|------|------|
| P0-1 | P0 | `delete_attachments` 的 `id` 直接 `join` 后 `remove_file`，无 `..` 校验 | `src/cmd/storage.rs:128-131`、`src/cmd/storage.rs:159`、`assistant-storage/src/attachment_store.rs:203-207` | 命令可从 Webview 调用；`id = "..\\..\\x"` 可删除附件库之外的文件 |
| P0-2 | P0 | vault 目录解析失败时回退相对路径 `security/vault` | `src/cmd/vault/mod.rs:215` | 密码库数据可能落到进程 cwd，用户无感知 |
| P0-3 | P0 | `security.db` 路径 `to_str().unwrap_or(默认)` | `src/cmd/vault/mod.rs:204` | 非 UTF-8 时静默换库，数据分叉 |
| P1-1 | P1 | `validate_path` 规范化失败静默回退原路径 | `assistant-tools/src/os/fs_link.rs:255` | 校验对象 ≠ 使用对象（I31 F5） |
| P1-2 | P1 | 系统目录用字符串前缀比较 | `assistant-tools/src/os/fs_link.rs:270-283` | 大小写/边界/盘符三处绕过（I31 F6） |
| P1-3 | P1 | `vault_export_to_file` / `vault_import` 零校验 | `src/cmd/vault/entries.rs:303-347`、`src/cmd/vault/entries.rs:354-361` | 任意路径读写 |
| P1-4 | P1 | 附件库 `read` / `file_path` 的 `id` 无校验 | `assistant-storage/src/attachment_store.rs:124-131` | 数据库内 id 若被污染可越界读 |
| P1-5 | P1 | `attachments` 目录名 4 处重复定义 | `src/setup.rs:67`、`src/chat/attachment/paths.ts:8`、`tauri.conf.json:41-44`、`capabilities/default.json:33-35` | 改名漏改 → 运行期静默失败 |
| P1-6 | P1 | 拖拽外部图片直接 `convertFileSrc`，越出 asset 作用域 | `src/chat/attachment/factory.ts:67-79` | 预览必然失败 |
| P1-7 | P1 | 目录解析失败策略 5 种不一致 | `src/setup.rs:48-73`、`src/cmd/vault/mod.rs:196-216`、`crates/assistant-crash/src/crash_dump.rs:6` | 同一故障不同后果 |
| P2-1 | P2 | `crash_dump` 硬编码 `"com.assistant"` | `crates/assistant-crash/src/crash_dump.rs:6` | 未引用 `APP_IDENTIFIER` |
| P2-2 | P2 | 日志落 LocalAppData，其余落 Roaming | `src/plugins.rs:7` | 排查时找错目录 |
| P2-3 | P2 | `installed_path` 经 `to_string_lossy` 跨 wire | `src/cmd/mascot_asset.rs:126` | 编码缺陷传导至前端 |
| P2-4 | P2 | 前端重复拼接附件路径 | `src/chat/message/ImageThumbnail.tsx:52-56` | 与 `paths.ts` 规则重复 |
| P2-5 | P2 | `SecurityDbState::new(&str)` 边界用 `&str` | `src/cmd/vault/mod.rs:203-205` | 类型三态之一 |
| P3-1 | P3 | `display()` 用于日志 | `src/cmd/vault/recovery.rs:77` 等 | 合理 |
| P3-2 | P3 | 测试用 `temp_dir()` | `crates/assistant-vault/tests/*` | 合理 |

---

## 9 整改建议：分两层，不要合并

把「应用自己的目录」和「外部输入的路径」塞进同一个 `path_utils` 是常见错误：前者会被套上不必要的 `..` 校验与越界检查，后者会因「同模块可信」而被放松校验。

### A. 受信目录层（`assistant-config` 内新增，或独立 crate）

职责：唯一基址解析 + 目录常量表 + 统一失败策略。

- 单一入口：`app_dir()` / `conf_dir()` / `log_dir()` / `crash_dir()`，内部封装 `app_data_dir` / `dirs::*` / 插件默认值。
- 常量表：`DIR_ATTACHMENTS`、`DIR_SECURITY`、`DIR_VAULT`、`DIR_RECOVERY`、`DIR_MASCOT_ASSETS`、`DIR_DERIVED`、`DIR_CRASHES`、`FILE_MAIN_DB`、`FILE_SECURITY_DB`、`FILE_VAULT_META`。
- 失败策略：**统一为 `Err` 上抛**，禁止回退相对路径、禁止 `expect`。启动期由调用方决定「记日志继续」还是「中止」，但解析函数本身只返回 `Result`。
- 硬约束：业务代码禁止直接出现 `app_data_dir()` / `dirs::config_dir()` / `"com.assistant"` 字面量。
- 跨技术栈同步：目录名同时出现在 Rust、前端、`tauri.conf.json`、`capabilities`。建议以 Rust 常量为源，前端通过一个 `paths` 模块集中导出，配置侧用契约测试校验字符串一致（类似 `src/settings/__tests__/confSlotContract.test.ts` 的读文件断言方式）。

### B. 不可信路径层（`assistant-tools/src/os/path.rs`，I31 T3）

职责：解析、规范化、越界校验、系统目录拒绝、形态转换。

- 沿用 I31 §5 的 `canonicalize_lenient` / `reject_system_dirs` / `ensure_within_root`。
- 新增：`validate_segment(id)` —— 单段文件名（附件 id、vault entry id）拒绝分隔符与 `..`，供 `AttachmentStore` 使用。
- 新增：`to_io_path()` / `to_display_path()` 双形态转换。IO 形态允许 `\\?\`，展示形态统一剥离；`ensure_within_root` 的比较必须在**同一形态**下进行。
- 外部进程单独处理：`git` 需实测 `-C` 传参在长路径下的行为（必要时设 `core.longpaths=true`）；`mklink` 不接受 `\\?\`，junction 应改为 Win32 直接创建 reparse point；`openPath` 不接受 `\\?\`，只能传展示形态。

### C. 边界类型约定

- Rust 内部一律 `PathBuf`；仅 Tauri command 入参/出参与 DTO 使用 `String`，且转换必须显式（禁止 `to_string_lossy` 跨 wire；非 UTF-8 应返回错误而非替换）。
- 前端一律通过 `paths` 模块拼接，禁止散点 `join`。

### D. 不做什么

- 不把 A 与 B 合并成单模块。
- 不在业务代码里临时 `format!("\\\\?\\{}", p)`。
- 不为了「看起来统一」把 `fs_link` 的系统目录白名单搬进受信目录层（二者保护对象不同）。

---

## 10 防回归与验收

### 10.1 机制性防回归

| 机制 | 目标 | 实现思路 |
|------|------|---------|
| 禁止裸基址调用 | 防止出现第 5 个解析器 | 在 `assistant-config` 之外禁止 `app_data_dir` / `dirs::`，用 CI 静态检查（`rg` 断言，类似 `src/services/__tests__/commandNames.test.ts` 的「stray 调用」断言） |
| 目录名契约测试 | 防止 4 处字面量漂移 | 读 `tauri.conf.json` 与 `capabilities/default.json`，断言作用域中的目录名等于 Rust 常量 |
| 路径穿越单测 | 防止 P0-1 复发 | `delete` / `read` / `file_path` 对 `../`、`..\\`、`/`、`\\`、`C:` 的用例 |
| 失败策略单测 | 防止回退相对路径复发 | 断言解析失败时返回 `Err` 而非 `PathBuf::from("security/vault")` |
| 双形态单测 | 长路径支持前置 | `to_io_path` / `to_display_path` 往返；`ensure_within_root` 在两种形态下的结果一致 |

### 10.2 验收标准

1. 业务代码中 `app_data_dir()` 出现次数 = 1（仅受信层内部）。
2. 业务代码中 `"attachments"` / `"security"` / `"mascot-assets"` 字面量出现次数 = 0（仅常量表）。
3. `PathBuf::from("security/vault")`、`expect("Failed to create attachments directory")` 归零。
4. `delete_attachments` / `read` / `file_path` 对穿越用例全部返回错误。
5. `tauri.conf.json` 与 `capabilities` 中的作用域目录名与常量表一致（契约测试通过）。
6. 非 UTF-8 路径在命令边界返回明确错误，不发生静默换值。

### 10.3 建议推进顺序

1. **P0 止血**：`delete_attachments` 的 id 校验；`vault` 目录解析失败改为 `Err`；`security.db` 去掉 `to_str().unwrap_or`。
2. **A 层落地**：受信目录常量表 + 单一入口 + 契约测试，替换 `src/setup.rs` / `src/cmd/vault/` / `src/cmd/mascot_asset.rs` / `crates/assistant-crash` 的散点。
3. **B 层落地**：`os/path.rs`（I31 T3），迁移 `fs_link`，补穿越单测。
4. **跨界收口**：前端 `paths` 模块 + `convertFileSrc` 作用域校验；`git` / `mklink` / `openPath` 形态适配。
5. **长路径评估**：若确认需要，先改 I31 §5.2 的「剥离 `\\?\`」结论为双形态模型，再实施；否则明确记录「不支持 >260 路径」为已知限制。

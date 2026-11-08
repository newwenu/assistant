# 后端依赖树审查与优化分析

> **审查日期**：2026-10-26（初版）、2026-10-27（升级验证）
> **审查范围**：`src-tauri/` Cargo workspace（10 个成员：根 crate `assistant-app` + 9 个 `crates/*`）
> **审查环境**：cargo 1.93.0 / rustc 1.93.0 / `x86_64-pc-windows-msvc`
> **审查方法**：`cargo metadata --filter-platform`、`cargo tree --duplicates`、逐 crate 源码引用检索、可疑项人工复核；升级验证在临时 workspace 副本中完成，未改动项目文件
> **限制**：`cargo-machete` 因本机 registry 缓存目录权限限制无法安装，未使用工具自动扫描；未使用依赖结论来自“源码标识符检索 + 人工阅读”，删除后仍需 `cargo check` 验证

---

## 1. 结论摘要

| # | 优先级 | 事项 | 实测收益 | 风险/成本 |
|---|--------|------|----------|-----------|
| 1 | P0 | 删除 10 个未使用的普通依赖、7 个未使用的开发依赖 | 清单准确性；测试构建少量收益 | 低，删除后编译验证即可 |
| 2 | P1 | 执行 `cargo update` + `machine-uid` 升级到 0.6 | 487→488→487；重复组 22→21，`png 0.17`、`winreg 0.6` 消失（新增 `rand_core` 分裂） | 低，`machine-uid` 需改 1 行 API |
| 3 | P1（可选） | 用 `include_str!` + `serde_json` 替换 `rust-i18n` | 487→459（−28 包），消除 5 组重复 | 中，需要重写翻译查找并验证回退行为 |
| 4 | P2 | 精简 `windows` crate 特性 | 可能减少生成代码 | 中，需确认上游是否已启用 |
| 5 | P2 | 删除 `keyring` 冗余的 `features = ["v1"]` | 可读性 | 无（v1 就是默认特性） |
| — | 不建议 | `base64` 降到 0.22、`rand` 升到 0.10、`rusqlite` 升到 0.41、`minidump-writer` 升到 0.13、裁剪 `tokio full`、强行统一上游重复版本 | 实测无收益、净增包或引入风险 | — |

---

## 2. 基线数据

对象为 Windows 桌面目标的 **普通依赖 + 构建依赖**（不含 dev）：

| 指标 | 数值 |
|------|------|
| 工作区成员 | 10 |
| Windows 构建参与包（去重后 name+version，含 10 个工作区成员） | **487** |
| 补充：临时副本执行 `cargo update` 后 | **488** |
| 补充：再升级 `machine-uid` 0.6 后 | **487** |
| 补充：再移除 `rust-i18n` 后 | **459** |
| 完成建议项后的重复版本组 | **16**（当前 22） |
| 加上 dev-dependencies 后的包数 | 509 |
| `Cargo.lock` 全平台包数 | 758 |
| Windows 构建中的重复版本组（当前锁文件） | **22** |

> 说明：`cargo tree` 输出的是依赖树中的出现次数，不是唯一包数；上表以 `cargo metadata --filter-platform x86_64-pc-windows-msvc` 的去重结果为准。

---

## 3. 未使用的直接依赖

### 3.1 普通依赖

| crate | 依赖 | 声明位置 | 核查结果 | 建议 |
|-------|------|----------|----------|------|
| `assistant-app`（根） | `reqwest` | `src-tauri/Cargo.toml:137` | 根 crate 源码 0 引用；真实使用者是 providers/security | 删除 |
| `assistant-app`（根） | `arboard` | `src-tauri/Cargo.toml:142` | 根 crate 源码 0 引用；真实使用者是 security | 删除 |
| `assistant-orb` | `serde` | `crates/assistant-orb/Cargo.toml:12` | 全 crate 无 `serde` / `Serialize` / `Deserialize` | 删除 |
| `assistant-orb` | `serde_json` | `crates/assistant-orb/Cargo.toml:13` | 同上 | 删除 |
| `assistant-agent` | `serde` | `crates/assistant-agent/Cargo.toml:12` | 仅使用 `serde_json`，无 serde 标识符 | 删除 |
| `assistant-sync` | `uuid` | `crates/assistant-sync/Cargo.toml:14` | 全 crate 无 `uuid` / `Uuid` | 删除 |
| `assistant-sync` | `zeroize` | `crates/assistant-sync/Cargo.toml:18` | 无 `zeroize` / `Zeroize` / `Zeroizing` | 删除 |
| `assistant-tools` | `log` | `crates/assistant-tools/Cargo.toml:10` | 无 `log` 及 `debug!/info!/warn!/error!` | 删除 |
| `assistant-tools` | `thiserror` | `crates/assistant-tools/Cargo.toml:11` | 无 `thiserror` / `Error` 派生 | 删除 |
| `assistant-providers` | `rand` | `crates/assistant-providers/Cargo.toml:24` | 无 `rand` / `OsRng` / `thread_rng` | 删除 |

### 3.2 开发依赖

| crate | 依赖 | 声明位置 | 核查结果 | 建议 |
|-------|------|----------|----------|------|
| `assistant-agent` | `proptest` | `crates/assistant-agent/Cargo.toml:25-26` | 全 crate 0 引用 | 删除 |
| `assistant-agent` | `tokio` 的 `test-util` | `crates/assistant-agent/Cargo.toml:27` | 无 `start_paused` / `time::pause` / `time::advance`；`timeout/sleep` 只需 `time`（normal 的 `full` 已包含） | 删除整条 dev 依赖 |
| `assistant-security` | `proptest` | `crates/assistant-security/Cargo.toml:51-52` | 全 crate 0 引用 | 删除 |
| `assistant-security` | `tempfile` | `crates/assistant-security/Cargo.toml:53` | 全 crate 0 引用 | 删除 |
| `assistant-sync` | `tempfile` | `crates/assistant-sync/Cargo.toml:21-22` | 全 crate 0 引用 | 删除 |
| `assistant-i18n` | `serde_json` | `crates/assistant-i18n/Cargo.toml:12-13` | 全 crate 0 引用 | 删除 |
| `assistant-app`（根） | `pdb` | `src-tauri/Cargo.toml:171` | 根 crate（含 tests/examples）0 引用 | 删除 |
| `assistant-app`（根） | `rustc-demangle` | `src-tauri/Cargo.toml:172` | 同上 | 删除 |

> 根 crate 的 `minidump` 在崩溃测试中有 6 处引用，保留。

### 3.3 预期收益说明

Cargo 的 feature 与依赖在 workspace 内统一解析，上述依赖大多同时被其他成员使用，删除后**发布/常规构建的包数几乎不变**（例如根 crate 删除 `reqwest`，providers 仍会编译它）。真正的收益是：

- 清单与实际使用一致，避免后续给错误 crate 增加特性；
- 根 crate 的 `pdb`、`rustc-demangle` 删除后，测试构建可少编译两个包；
- 减少“看起来能用、实际未接通”的误导。

---

## 4. 重复依赖

### 4.1 `base64`：无法通过版本对齐消除

| 版本 | 使用方（执行 `cargo update` 后） |
|------|--------|
| `0.22.1` | `reqwest 0.12`、`wry 0.57` |
| `0.23.1` | 本项目 5 个 crate（`Cargo.toml:51` 声明）、`tauri-codegen 2.7.4`、`hyper-util 0.1.21`、`plist 1.10.3`、`serde_with 3.24`、`serde-saphyr 1.3`（仅 i18n） |

初版结论“降到 0.22 可少一个包”经实测**不成立**：把工作区声明改为 `0.22` 后，`0.23.1` 仍被上述 6 个上游包使用，重复组保持不变；改为 `0.23` 也同样覆盖不到 `reqwest`/`wry`。只有等 `reqwest`/`wry` 升级到 `base64 0.23`，才会自然收敛。

源码只使用 `base64::engine::general_purpose::STANDARD` 与 `base64::Engine`，0.22/0.23 都可编译，**保持 0.23 即可，无需动作**。

### 4.2 Windows 构建的重复版本与处理结论

| 包 | 版本 | 来源 | 升级/替换后 |
|----|------|------|-------------|
| `base64` | 0.22 / 0.23 | `reqwest`、`wry` / 本项目 + `tauri-codegen`、`hyper-util`、`plist`、`serde_with` | **不可消除**；上游分裂 |
| `bitflags` | 1.3 / 2.13 | `globwalk`（i18n）、`png 0.17`（`ico 0.5`）、`jiff`（`defmt 1.1`） / Tauri 生态 | 移除 i18n + 升级 `ico` 后，仍由 `defmt` 保留 1.3 |
| `darling` / `darling_core` / `darling_macro` | 0.20 / 0.24 | `derive_builder`（`zxcvbn`）/ `serde_with` | 上游 proc-macro 链，不处理 |
| `getrandom` | 0.2 / 0.3 / 0.4 | `rand 0.8 → rand_core 0.6` / `tauri` / `uuid → getrandom 0.4` | 3 个版本并存，不处理 |
| `hashbrown` | 0.12 / 0.14 / 0.17 | `schemars 0.8 → indexmap 1.9`（经 `tauri-build`）/ `dashmap 6` + `hashlink` / `indexmap 2.14` | `rusqlite 0.41` 只把 `hashlink 0.9→0.12`，0.14 仍由 `dashmap` 保留 |
| `indexmap` | 1.9 / 2.14 | `schemars 0.8` + `serde_with` / 现代生态 | 上游，不处理 |
| `itertools` | 0.11 / 0.14 | `rust-i18n-support` / 其他 | **移除 i18n 后消除** |
| `keyboard-types` | 0.7 / 0.8 | `global-hotkey` / `muda` | Tauri 内部，不处理 |
| `miniz_oxide` | 0.8 / 0.9 | `image 0.25 → png 0.18` / `tauri-codegen` 等 | 上游，不处理 |
| `png` | 0.17 / 0.18 | `tauri-codegen → ico 0.5`（构建） / `image`、`muda`、`tray-icon`、`tauri-codegen` | **`cargo update` 后 `ico 0.6` 已消除 0.17** |
| `schemars` | 0.8 / 0.9 / 1.2 | `tauri-build`（`schemars 0.8`）/ `serde_with` / 上游 | 上游，不处理 |
| `serde_spanned` / `toml` / `toml_datetime` / `winnow` | 0.6 / 1.x、0.8 / 1.1 | `rust-i18n-support`（Windows）/ Tauri 工具链 | **移除 i18n 后消除 0.6/0.8/0.7 这一代** |
| `syn` | 2 / 3 | 两代 proc-macro 生态 | 不处理 |
| `windows-sys` | 0.59 / 0.60 / 0.61 | `tauri-plugin-global-shortcut → global-hotkey` / 构建工具 / Tauri 生态 | 上游，不处理 |
| `windows_x86_64_msvc` / `windows-targets` | 0.52 / 0.53 | 上述 `windows-sys` 代际 | 不处理 |
| `winreg` | 0.55 / 0.6 | `tauri-build → tauri-winres → embed-resource`（构建） / `machine-uid 0.2` | **`machine-uid 0.6` 后消除 0.6.2** |

结论：执行 `cargo update` + `machine-uid 0.6` + 移除 `rust-i18n` 后，重复组从 22 降到 16；剩下 16 组全部来自 Tauri 生态、proc-macro 代际或不可避免的随机数/平台 API 版本分裂，强行 `cargo update -p` 锁版本收益极低且容易破坏上游兼容性，不建议处理。

### 4.3 平台专属重复（不影响 Windows）

| 平台 | 重复项 | 来源 |
|------|--------|------|
| Linux | `aes 0.8/0.9`、`sha2 0.10/0.11`、`hmac/hkdf 0.12/0.13`、`cbc/cipher` | `keyring 4` 的 `secret-service 5.2` 后端 |
| Linux | `dirs 6/7` | `auto-launch`（Tauri 自启）与本项目 |
| Android / iOS | `reqwest 0.12/0.13` | Tauri 仅在 `android` 或非 macOS 的 Apple 目标启用 0.13 |
| macOS | `core-foundation 0.9/0.10`、`core-graphics` 系列 | Tauri 生态代际 |

如果后续需要正式支持 Linux，可再评估将 `keyring` 的 Linux 后端换成更轻的 `linux-keyutils-keyring-store`；这会改变密钥存储行为，当前不建议仅为体积调整。

---

## 5. 结构性优化候选：移除 `rust-i18n`

### 5.1 现状

`crates/assistant-i18n/src/lib.rs:6-9` 只用到两件事：

```rust
#[macro_use]
extern crate rust_i18n;
rust_i18n::i18n!("locales", fallback = "zh-CN");
```

以及 `crates/assistant-i18n/src/lib.rs:39` 的 `t!(key, locale = ...)`。`format_text`、缓存、全局 locale 都是项目自己实现的。

**版本状态（2026-10-27 核查）**：`rust-i18n` / `rust-i18n-macro` / `rust-i18n-support` 的最新稳定版均为 **4.2.4**（上游无 5.x，仅 `4.0.0-preview0/1` 两个预发布），而项目 `Cargo.lock` 已锁定 **4.2.4**，即已是最新版本，不存在"等待上游升级以精简依赖"的空间。核查最新版 4.2.4 的清单确认其依赖结构未变：`rust-i18n-support` 仍以 `codegen` feature 可选依赖 `globwalk 0.8`、`itertools ^0.11`、`toml ^0.8`、`serde-saphyr`、`normpath`，`rust-i18n` 仍把 `globwalk ^0.8.1`、`regex ^1` 作为构建依赖。因此第 5.2 节的 28 包链只能通过自实现替换来消除，升级路线无效。

### 5.2 独占依赖链

按 Windows 目标的普通+构建依赖计算，**只通过 `assistant-i18n` 才能到达的包共 28 个**（含 `rust-i18n` 本身）。临时副本中移除该依赖后，Windows 构建包数从 **487 降到 459**，没有新增包：

```
annotate-snippets 0.12.17
anstyle 1.0.14
arc-swap 1.9.3
arraydeque 0.5.1
base62 2.2.6
bstr 1.13.1
crossbeam-deque 0.8.8
crossbeam-epoch 0.9.21
encoding_rs_io 0.1.8
globset 0.4.20
globwalk 0.8.1
granit-parser 1.3.0
ignore 0.4.33
itertools 0.11.0
nohash-hasher 0.2.0
normpath 1.5.2
rust-i18n 4.2.4
rust-i18n-macro 4.2.4
rust-i18n-support 4.2.4
serde-saphyr 1.3.0
serde_spanned 0.6.9
toml 0.8.23
toml_datetime 0.6.11
toml_edit 0.22.27
toml_write 0.1.2
triomphe 0.1.17
unicode-width 0.2.2
winnow 0.7.15
```

其中 5 组会直接退出重复版本表：`itertools`、`serde_spanned`、`toml`、`toml_datetime`、`winnow`。

### 5.3 替代方案

- `include_str!` 编译期嵌入 `locales/zh-CN.json`、`locales/en-US.json`；
- 启动时用 `serde_json` 解析为 `HashMap<String, String>`（扁平化点分 key），或保留自定义嵌套查找；
- `text(locale, key)` 先查目标 locale，缺失时回退 `zh-CN`；
- 保留 `format_text` 的 `%{name}` 占位符替换与现有 `CATALOG_REVISION` 构建指纹；
- 保持 `text(locale, key) -> &'static str` 签名（可继续用 `Box::leak` 或一次性静态表），避免改动所有调用方；
- 移除 `rust_i18n::i18n!` 宏及 `#[macro_use] extern crate`。

### 5.4 风险与验证

| 风险 | 验证方式 |
|------|----------|
| locale key 查找/回退语义变化 | 对比两个 JSON 的 key 集合；对 `text()` 做全 key 遍历测试 |
| 占位符格式差异 | 保留 `format_text` 现有实现，覆盖带参文案测试 |
| 编译期指纹失效 | 保留 `build.rs` 的 `rerun-if-changed` 与 `ASSISTANT_I18N_CATALOG_REVISION` |
| 调用方影响 | `cargo test --workspace` + 启动后切换中英文检查托盘、设置页文案 |

这是本次审查中唯一能成规模缩减依赖树的方向，但属于代码重构，建议独立提交。

---

## 6. Feature 分析

### 6.1 `tokio = ["full"]`

当前声明位于 `src-tauri/Cargo.toml:46`。源码实际使用：

- 使用：`time`（`timeout`/`sleep`）、`fs`、`io`、`sync`、`macros`、`rt-multi-thread`（Tauri 运行时）
- 未使用：`tokio::process`（0 处）、`tokio::signal`（0 处）、`tokio::net`（0 处）、`io-std`

但**裁剪后包数几乎不会下降**：

- `parking_lot` 同时被 `arboard`、`crash-handler`、`tao` 使用；
- `signal-hook-registry` 同时被 `async-signal` 使用；
- `net` 仍会被 `reqwest`/Tauri 启用。

因此 `full` 的优化价值主要是“显式表达真实需求”，不建议作为编译提速项单独推进。

### 6.2 `windows` crate 特性

`src-tauri/Cargo.toml:93-114` 声明了 20 个特性。源码有实际使用的包括：

| 特性 | 使用位置 |
|------|----------|
| `Win32_Foundation`、`Win32_Storage_FileSystem` | `assistant-tools/src/os/fs_link.rs` |
| `Security_Credentials`、`Security_Cryptography`、`Storage_Streams` | `assistant-security/src/biometric/windows.rs` |
| `Win32_System_WinRT`、`Win32_UI_WindowsAndMessaging` | `assistant-security/src/biometric/windows.rs` |
| `Win32_System_Registry` | `unlock_dialog/windows/theme.rs` |
| `Win32_Graphics_Dwm`、`Win32_Graphics_Gdi`、`Win32_UI_Controls`、`Win32_UI_HiDpi`、`Win32_UI_Input_KeyboardAndMouse` | `unlock_dialog/windows/*` |
| `Win32_System_RemoteDesktop`、`Win32_System_Power` | `screen_lock/mod.rs:73-91`（`System::{Power, RemoteDesktop}` 分组导入） |
| `Win32_System_LibraryLoader` | 动态加载相关代码 |

源码中零引用、可优先尝试删除的特性：

- `Security_Cryptography_DataProtection`
- `Win32_System_StationsAndDesktops`

`Win32_Security`、`Foundation` 没有直接导入，但可能被其他已启用特性隐式依赖；建议一次只删一个并编译验证，不要批量删除。

注意：Tauri/wry 可能也启用了这些特性，删除本项目声明后如果包数不变，则说明收益为零；删除后必须执行 `cargo check --workspace --all-targets`。

### 6.3 其他特性

| 项 | 结论 |
|----|------|
| `keyring = { features = ["v1"] }` | `v1` 是 `keyring 4` 的默认特性，显式声明冗余，可简化为 `keyring = "4"` |
| `image` 的 `png/jpeg/bmp/tiff` | 全部被 `assistant-agent/src/normalization.rs` 与 `multimodal.rs` 使用，保留 |
| `reqwest` 的 `stream/json` + 默认特性 | 默认的 `http2`、`system-proxy`、TLS 对 LLM 流式请求有意义，不建议关闭 |
| `rusqlite` 的 `bundled-sqlcipher` | 代码中 `PRAGMA key` 出现 14 次，SQLCipher 是功能必需，保留 |

---

## 7. 其他发现

- `src-tauri/crates/assistant-app/` 是空目录（只有空的 `gen/schemas`、`src`），且不在 `workspace.members` 中，属于遗留目录，可删除。
- `assistant-orb` 对 `tauri` 的唯一引用是 `placement.rs` 中的 `tauri::AppHandle`，依赖本身不会增加包数（Tauri 已由根 crate 引入），无需处理。
- dev 依赖中 `minidump 0.27` 与 `minidump-writer` 的 `minidump-common 0.24` 存在跨版本重复，仅在测试构建出现，对齐收益低，暂不处理。
- 前端 `npm ls --depth=0` 无缺失/多余包；`graphology`、`graphology-communities-louvain` 仅被 `scripts/cluster-tools.mjs` 使用，放在 devDependencies 合理。

---

## 8. 较新版本的重合验证（实测）

本节回答“升级到较新版本能否减少重复依赖”。方法：把 `src-tauri/` 复制到临时目录（排除 `target`），在副本中逐项修改 `Cargo.toml`/`Cargo.lock`，用 `cargo metadata --filter-platform x86_64-pc-windows-msvc` 对比包数与重复组；项目文件未改动。

### 8.1 实验结果总览

| 实验 | 变更 | 包数 | 重复组 | 结论 |
|------|------|------|--------|------|
| 基线 | — | 487 | 22 | — |
| E1 | `cargo update`（保持所有版本约束） | 488 | 22 | `ico 0.6` 消除 `png 0.17`，但 `uuid 1.29 → getrandom 0.4.3` 链上出现 `rand_core 0.10`，组数抵消；净增 1 包 |
| E2 | E1 + `machine-uid 0.2 → 0.6` | 487 | 21 | **真正有效**：消除 `winreg 0.6` 与 `windows-registry`，净减 1 包 |
| E3 | E2 + `base64 0.23 → 0.22` | 487 | 21 | **无效**：0.23 仍被 `tauri-codegen`、`hyper-util`、`plist`、`serde_with`、`serde-saphyr` 使用 |
| E4 | E2 + `rusqlite 0.31 → 0.41` | 487 | 21 | 包数与重复组均不变；`hashlink 0.9→0.12`，`hashbrown 0.14` 仍由 `dashmap` 保留 |
| E5 | E2 + `rand 0.8 → 0.10` | 490 | 24 | **净增 3 包、重复组 +3**：新增 `rand`、`chacha20`、`cpufeatures` 三组分裂 |
| E6 | E2 + `minidump-writer 0.10 → 0.13` | 488 | 22 | **净增 1 包、重复组 +1**：新增 `crash-context 0.6/0.8` |
| E7 | E2 + 移除 `rust-i18n` 依赖 | **459** | **16** | 最大收益：−28 包、消除 5 组重复 |

### 8.2 逐项说明

**`ico`（Tauri 构建依赖）**：`cargo update` 会把 `ico 0.5` 升到 `0.6`，后者改用 `png 0.18`，从而消除 `png 0.17` 重复。但同一次更新把 `uuid` 升到 1.29，其 `getrandom 0.4.3` 依赖上出现了 `rand_core 0.10`（与 `rand 0.8`、`rsa` 所用的 `rand_core 0.6` 并存），重复组数量不变；同时 `ico 0.6` 自身新增 `windows-sys 0.60` 等构建依赖，整体包数 +1。属于“换一组重复、不减少包数”的收益，可随常规 `cargo update` 顺带获得。

> 依赖路径（普通/构建边，实测）：`assistant-providers → assistant-security → uuid 1.29 → getrandom 0.4.3 → rand_core 0.10.1`。基线中同一路径为 `uuid 1.27 → getrandom 0.4.3`，但 `getrandom` 的 `rand_core` feature 未启用，故无该节点；具体由哪个包开启该 feature 需用 `cargo tree -e features -i getrandom@0.4.3` 进一步确认（推测与 dev 依赖 `proptest 1.11→1.12` 引入的 `rand 0.10` 有关，Cargo 的 feature 在 workspace 内全局统一）。

**`machine-uid 0.2 → 0.6`**：0.2 依赖 `winreg 0.6` + `windows-registry`；0.6 改用 `windows-sys 0.61`（Tauri 生态已有）。实测包数 488→487、重复组 22→21，是**唯一值得主动升级的直接依赖**。

API 变化：0.2 的 `machine_uid::get() -> Result<String, Box<dyn Error>>`，0.6 的 `get()` 返回 `Result<String, MachineUidError>`；项目在 [machine_id.rs](../../src-tauri/crates/assistant-security/src/machine_id.rs) 中只做“取字符串 + 错误映射”，改动约 1 行。**注意该值参与密钥派生，升级后必须验证同一台机器仍返回相同 ID**（尤其 Windows 注册表路径变化），否则会导致已存的加密数据无法解密。

**`base64 0.23 → 0.22`**：本项目改到 0.22 后，0.23 仍被 `tauri-codegen`、`hyper-util`、`plist`、`serde_with`、`serde-saphyr` 等 6 个上游包使用；反之保持 0.23 也覆盖不到 `reqwest`/`wry`。结论：**不动**。这是对初版文档的修正。

**`rusqlite 0.31 → 0.41`**：实测包数与重复组均不变（487/21）；主要变化只是 `hashlink 0.9 → 0.12`，而 `hashbrown 0.14` 仍被 `dashmap 6` 引用。升级本身还有 SQLCipher 链接脚本与 API 迁移成本，**不建议**。

**`rand 0.8 → 0.10`**：项目 `rand` 只被 `assistant-security` 使用（`OsRng`/`CryptoRng`），而 `rsa` 仍强制 `rand_core 0.6`。升到 0.10 后实测包数 487→490、重复组 21→24，新增 `rand 0.8/0.10`、`chacha20 0.9/0.10`、`cpufeatures 0.2/0.3` 三组分裂，**明显负收益**。

**`minidump-writer 0.10 → 0.13`**：实测包数 487→488、重复组 21→22，新增 `crash-context 0.6/0.8` 分裂。虽然 `defmt` 早已由 `jiff` 引入，不是本次升级的增量，但升级本身仍带来 +1 包 +1 组重复，**不建议**。

### 8.3 是否需要执行 `cargo update`

`cargo update` 本身会带来 Tauri 2.12.1→2.12.5 等补丁升级，属于常规维护，建议配合 `machine-uid` 升级一起做，并在升级后执行完整 `cargo test`。单独看依赖体积，`cargo update` 是 +1 包、0 组变化，不要指望它减包；真正的收益来自紧随其后的 `machine-uid 0.6`（−1 包、−1 组）。

---

## 9. 建议实施顺序

### Step 1（P0，低风险）

删除第 3 节的未使用依赖，然后执行：

```powershell
cargo check --workspace --all-targets
cargo test --workspace
```

### Step 2（P1，低风险）

执行常规 `cargo update` 并把 `machine-uid` 升到 0.6：

```powershell
cargo update
# 修改 src-tauri/Cargo.toml：machine-uid = "0.6"
cargo check --workspace --all-targets
cargo tree --duplicates
```

预期：包数 487→488（`cargo update`）→487（`machine-uid 0.6`），重复组 22→22→21；`png 0.17`、`winreg 0.6` 消失，但新增 `rand_core 0.6/0.10` 分裂。

**前置验证**（必须）：升级前后在同一台机器上分别打印 `machine_uid::get()`，确认值一致；该值参与密钥派生，变化会导致历史加密数据无法解密。

### Step 3（P1，可选重构）

按第 5 节替换 `rust-i18n`，重点验证翻译回退、带参文案与语言切换。实测预期：

- Windows 构建包数 **487 → 459**（−28）；
- 重复版本组 21 → 16（消除 `itertools`、`serde_spanned`、`toml`、`toml_datetime`、`winnow`）；
- `assistant-i18n` 的 `serde_json` 由 dev 依赖转为普通依赖。

### Step 4（P2，可选）

尝试删除 `windows` 的 2 个零引用特性（`Security_Cryptography_DataProtection`、`Win32_System_StationsAndDesktops`），必要时同时把 `keyring` 的 `v1` 显式声明去掉，执行全量 `cargo check`。

---

## 附录 A：审查命令

```powershell
# 当前平台普通 + 构建依赖去重
cargo metadata --format-version 1 --locked --filter-platform x86_64-pc-windows-msvc

# 重复版本
cargo tree --duplicates

# 反查某版本来源
cargo tree -i base64@0.22.1 --depth 2

# 特性使用
cargo tree -e features -i tokio --depth 1
cargo tree -e features -i windows@0.62.2 --depth 1
```

## 附录 B：`assistant-i18n` 独占依赖（28 个，`cargo update` 后实测）

```
annotate-snippets 0.12.17
anstyle 1.0.14
arc-swap 1.9.3
arraydeque 0.5.1
base62 2.2.6
bstr 1.13.1
crossbeam-deque 0.8.8
crossbeam-epoch 0.9.21
encoding_rs_io 0.1.8
globset 0.4.20
globwalk 0.8.1
granit-parser 1.3.0
ignore 0.4.33
itertools 0.11.0
nohash-hasher 0.2.0
normpath 1.5.2
rust-i18n 4.2.4
rust-i18n-macro 4.2.4
rust-i18n-support 4.2.4
serde-saphyr 1.3.0
serde_spanned 0.6.9
toml 0.8.23
toml_datetime 0.6.11
toml_edit 0.22.27
toml_write 0.1.2
triomphe 0.1.17
unicode-width 0.2.2
winnow 0.7.15
```

## 附录 C：Windows 构建重复版本组（当前锁文件，22 组）

```
base64 0.22.1, 0.23.1
bitflags 1.3.2, 2.13.2
darling 0.20.11, 0.24.1
darling_core 0.20.11, 0.24.1
darling_macro 0.20.11, 0.24.1
getrandom 0.2.17, 0.3.4, 0.4.3
hashbrown 0.12.3, 0.14.5, 0.17.1
indexmap 1.9.3, 2.14.2
itertools 0.11.0, 0.14.0
keyboard-types 0.7.0, 0.8.3
miniz_oxide 0.8.9, 0.9.1
png 0.17.16, 0.18.1
schemars 0.8.22, 0.9.0, 1.2.2
serde_spanned 0.6.9, 1.1.1
syn 2.0.119, 3.0.6
toml 0.8.23, 1.1.6+spec-1.1.0
toml_datetime 0.6.11, 1.1.1+spec-1.1.0
windows_x86_64_msvc 0.52.6, 0.53.1
windows-sys 0.59.0, 0.60.2, 0.61.2
windows-targets 0.52.6, 0.53.5
winnow 0.7.15, 1.0.4
winreg 0.55.0, 0.6.2
```

## 附录 D：执行建议项后的重复版本组（16 组）

条件：`cargo update` + `machine-uid 0.6` + 移除 `rust-i18n`（未升级 `rand`/`rusqlite`/`minidump-writer`）。

```
base64 0.22.1, 0.23.1
bitflags 1.3.2, 2.13.2
darling 0.20.11, 0.24.1
darling_core 0.20.11, 0.24.1
darling_macro 0.20.11, 0.24.1
getrandom 0.2.17, 0.3.4, 0.4.3
hashbrown 0.12.3, 0.14.5, 0.17.1
indexmap 1.9.3, 2.14.2
keyboard-types 0.7.0, 0.8.3
miniz_oxide 0.8.9, 0.9.1
rand_core 0.10.1, 0.6.4
schemars 0.8.22, 0.9.0, 1.2.3
syn 2.0.119, 3.0.9
windows_x86_64_msvc 0.52.6, 0.53.1
windows-sys 0.59.0, 0.60.2, 0.61.2
windows-targets 0.52.6, 0.53.5
```
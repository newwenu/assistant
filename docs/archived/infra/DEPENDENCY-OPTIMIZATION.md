# 后端依赖优化补充分析（cargo tree --duplicates）

> 生成日期：2026-10-24 | 基于 `cargo tree --duplicates` 实际编译数据
> 更新日期：2026-10-24 | P0+P1+P2+P3 全部已执行，见 §6 修复记录

---

## 1. 重复版本编译分析

原 DEPENDENCY-ANALYSIS.md 侧重"功能重叠"分析（§2），本节补充 **实际编译中的版本重复** 问题。
版本重复会增加编译时间、二进制体积和内存占用。`cargo tree --duplicates` 输出完整数据见 `_tree_duplicates.txt`。

### 1.1 🔴 可操作的重复（项目可直接修复）

| 依赖 | 版本 A | 版本 B | 原因 | 影响 | 建议 |
|------|--------|--------|------|------|------|
| `base64` | 0.22.1（reqwest/tauri-codegen 传递依赖） | 0.23.1（项目直接依赖 + hyper-util/plist/serde-saphyr） | ~~项目锁定 0.22~~ 已升级 0.23，0.22 来自上游 | 0.22 由 reqwest/tauri-codegen 锁定，项目侧已统一 | ✅ 已升级 workspace `base64 = "0.23"`，代码已用 Engine trait API，零修改 |
| `dirs` | ~~6.0.0（项目直接依赖）~~ | 7.0.0（项目 + tauri/tauri-build） | ~~项目用 v6，Tauri 2.12 生态用 v7~~ | ~~双份编译~~ | ✅ 已升级 workspace `dirs = "7"`，双份编译已消除 |
| `security-framework` | ~~2.11.1（keyring 3.x 传递）~~ | 3.7.0（项目直接依赖） | ~~keyring 3.6.3 同时依赖两个版本~~ | ~~macOS 双份编译~~ | ✅ 已升级 keyring 3→4，security-framework 双版本已消除 |
| `core-foundation` | 0.9.4（core-graphics/system-configuration 传递） | 0.10.1（项目直接依赖 + security-framework 3.x） | core-graphics/system-configuration 锁定 0.9，与项目 0.10 并存 | macOS 额外编译（上游传递，无法控制） | ⚠️ keyring 升级后 security-framework 双版本已消除，但 core-graphics/system-configuration 仍拉入 0.9.4 |

### 1.2 🟡 传递依赖重复（项目无法直接控制）

| 依赖 | 版本列表 | 来源 | 评估 |
|------|---------|------|------|
| `windows-sys` | 0.59.0, 0.60.2, 0.61.2 | global-hotkey(0.59), keyring/rfd(0.60), arboard/tokio/mio/tauri(0.61) | Windows 生态碎片化，三份 windows-sys 编译。项目已统一 `windows` crate 至 0.62，但 `windows-sys` 是不同 crate，由各库自行依赖 |
| `getrandom` | 0.2.17, 0.3.4, 0.4.3 | crypto 生态(0.2), tauri(0.3), tempfile/uuid(0.4) | 三个版本，但均为轻量 crate，影响小 |
| `hashbrown` | 0.12.3, 0.14.5, 0.17.1 | schemars(0.12), dashmap/hashlink(0.14), indexmap(0.17) | 不同 epoch 的 indexmap 生态，无法统一 |
| `indexmap` | 1.9.3, 2.14.2 | schemars(v1), hyper/toml/plist(v2) | v1→v2 生态迁移未完成 |
| `itertools` | 0.11.0, 0.14.0 | rust-i18n-support(0.11), prost/zxcvbn(0.14) | 轻量 crate，影响小 |
| `winreg` | 0.6.2, 0.55.0 | machine-uid(0.6), embed-resource(0.55) | 仅 Windows，build-dep 占一个 |
| `toml` | 0.8.23, 1.1.6 | rust-i18n(0.8), cargo_toml/embed-resource(1.1) | build-dependency，不影响运行时 |
| `syn` | 2.0.119, 3.0.6 | 多数 proc-macro(2.0), async-trait/cssparser/futures-macro(3.0) | proc-macro 仅编译期，不影响二进制 |
| `png` | 0.17.16, 0.18.1 | ico/tauri-codegen(0.17), image/arboard(0.18) | 图标编译 + 剪贴板，影响小 |
| `scroll` | 0.11.0, 0.12.0, 0.13.0 | pdb(0.11), minidump-writer(0.12), minidump(0.13) | **仅 dev-dependency**，不影响 release |

### 1.3 🟢 仅 dev-build 的重复（不影响 release）

| 依赖 | 版本 | 说明 |
|------|------|------|
| `minidump-common` | 0.24.1, 0.27.0 | minidump-writer(0.24) vs minidump dev-dep(0.27) |
| `fallible-iterator` | 0.2.0, 0.3.0 | pdb dev-dep(0.2) vs rusqlite(0.3) |
| `scroll` 系列 | 0.11/0.12/0.13 | 全部来自 dev-dep 链 |

---

## 2. 优化建议

### 2.1 🔴 keyring 双 security-framework 问题（macOS）

**问题**：`keyring 3.6.3` 的 `Cargo.toml` 同时依赖 `security-framework 2.x` 和 `3.x`：

```
keyring v3.6.3
├── security-framework 2.11.1  ← 旧版（Apple 通用密钥链）
├── security-framework 3.7.0   ← 新版（项目直接依赖）
└── ...
```

这导致 macOS 编译产出：
- `security-framework` 2.11.1 + 3.7.0（双份）
- `core-foundation` 0.9.4 + 0.10.1（双份）
- `security-framework-sys` 2.17.0（共享）

**根因**：keyring 3.x 为兼容旧 macOS 版本，保留 security-framework 2.x 路径用于 `SecItemAdd/Delete/CopyMatching`（通用密钥链），3.x 路径用于新 API。

**建议**：

| 方案 | 操作 | 风险 | 收益 |
|------|------|------|------|
| A. 升级 keyring 3→4 | ✅ 已执行：`keyring = { version = "4", features = ["v1"] }`，`v1` feature API 100% 兼容 3.x（源码验证 `Entry::new`/`Error::NoEntry`/`set_password`/`get_password`/`delete_credential` 签名不变），代码零修改 | ~~中~~ → 低（edition 2024 + rust-version 1.88，当前 Rust 1.93.0 满足；API 兼容性已验证） | ✅ security-framework 双版本已消除 |
| B. 评估是否可移除 keyring | 若生物识别仅用 Windows Hello + macOS LocalAuthentication，keyring 可选 | 中（Linux 无 keyring 则生物识别不可用） | 消除双份 security-framework + keyring 本身 |
| C. 接受现状 | macOS 双份编译约增加 30s | 无 | 无 |

**推荐**：✅ 方案 A 已执行。验证结果：
1. ✅ `keyring = { version = "4", features = ["v1"] }` 等价于原 `features = ["apple-native", "windows-native", "linux-native"]`（v1 自动包含三平台后端）
2. ✅ `keyring::Entry::new` / `keyring::Error::NoEntry` API 签名完全不变（源码级对比 v1.rs vs 3.x lib.rs）
3. ✅ `edition = "2024"` + `rust-version = "1.88"` 编译通过（Rust 1.93.0）
4. ✅ `cargo tree --duplicates --target all -p security-framework` 无重复输出

### 2.2 🔴 Workspace 卫生问题

以下依赖在 crate 中直接写版本号，未通过 `{ workspace = true }` 引用，违反 workspace 统一管理原则：

| Crate | 依赖 | 当前写法 | 应改为 | 状态 |
|-------|------|---------|--------|------|
| assistant-tools | `dashmap` | ~~`dashmap = "6"`~~ | `dashmap = { workspace = true }` | ✅ 已修复 |
| assistant-security | `windows-future` | ~~`windows-future = "0.3"`~~ | `windows-future = { workspace = true }` | ✅ 已修复 |
| assistant-security | `core-foundation` | ~~`core-foundation = "0.10"`~~ | `core-foundation = { workspace = true }` | ✅ 已修复 |
| assistant-security | `security-framework` | ~~`security-framework = "3.0"`~~ | `security-framework = { workspace = true }` | ✅ 已修复 |
| assistant-security | `libsecret` | ~~`libsecret = "0.1"`~~ | `libsecret = { workspace = true }` | ✅ 已修复 |
| assistant-security | `tempfile` (dev) | ~~`tempfile = "3"`~~ | `tempfile = { workspace = true }` | ✅ 已修复 |
| assistant-storage | `tempfile` (dev) | ~~`tempfile = "3"`~~ | `tempfile = { workspace = true }` | ✅ 已修复 |
| assistant-sync | `tempfile` (dev) | ~~`tempfile = "3"`~~ | `tempfile = { workspace = true }` | ✅ 已修复 |
| assistant-config | `tempfile` (dev) | ~~`tempfile = "3"`~~ | `tempfile = { workspace = true }` | ✅ 已修复 |

**已执行**：`tempfile`、`windows-future`、`core-foundation`、`security-framework`、`libsecret` 已加入 `[workspace.dependencies]`，所有 crate 统一使用 `{ workspace = true }`。

### 2.3 🔴 未使用的直接依赖

通过 `grep -r "use tokio\|tokio::" crates/*/src/` 验证，以下 crate 声明了依赖但代码未使用：

| Crate | 未使用依赖 | 声明位置 | 分析 | 建议 |
|-------|-----------|---------|------|------|
| **assistant-tools** | `tokio` | ~~`Cargo.toml` line 7~~ | 全 crate 无 `use tokio` / `tokio::` 引用 | ✅ 已移除 |
| **assistant-tools** | `tokio-util` | ~~`Cargo.toml` line 8~~ | 全 crate 无 `use tokio_util` / `CancellationToken` 引用 | ✅ 已移除 |
| **assistant-security** | `tokio` | ~~`Cargo.toml` line 26~~ | 全 crate 无 `use tokio` / `tokio::` 引用；reqwest 已传递依赖 tokio | ✅ 已移除 |
| **assistant-sync** | `tokio` | ~~`Cargo.toml` line 7~~ | 全 crate 无 `use tokio` / `tokio::` 引用；通过 assistant-security 传递 | ✅ 已移除 |

**注意**：移除 `tokio` 直接依赖后，这些 crate 仍可通过传递依赖使用 tokio 运行时（由 assistant-app 的 `tokio = { workspace = true }` 提供）。但若这些 crate 的测试需要 `#[tokio::test]`，需在 `[dev-dependencies]` 中保留 `tokio`。

**预估收益**：减少 3 个 crate 的直接 tokio 依赖声明，简化依赖图，减少 feature 传播（tokio `full` feature 不再传播到这些 crate）。

### 2.4 🟡 版本对齐机会

| 依赖 | 当前版本 | 建议版本 | 原因 | 风险 |
|------|---------|---------|------|------|
| `base64` | ~~0.22~~ → 0.23 | 0.23 ✅ | ~~消除与 hyper-util/plist 的双份编译~~ 项目侧已统一；0.22 仍由 reqwest/tauri-codegen 传递依赖 | ~~API 不兼容~~ 代码已用 Engine trait，零修改 |
| `dirs` | ~~6~~ → 7 | 7 ✅ | 与 Tauri 2.12 生态对齐（tauri/tauri-build 用 dirs 7） | API 兼容（dirs 7 仅新增函数），低风险 ✅ |

### 2.5 🟡 tokio feature 优化

当前 workspace 声明 `tokio = { version = "1", features = ["full"] }`。`full` 启用所有 feature，包括项目不需要的 `io-uring`（Linux）、`tracing` 等。

**实际使用的 tokio feature**：

| Feature | 使用位置 | 说明 |
|---------|---------|------|
| `rt-multi-thread` | app 启动 | 多线程运行时（full 包含） |
| `macros` | `#[tokio::test]`、`#[tokio::main]` | 测试 + main |
| `fs` | storage/attachment_store.rs | 异步文件 I/O |
| `time` | providers 超时、app 超时 | 定时器 |
| `sync` | providers CancellationToken | Mutex/Barrier 等 |
| `signal` | — | 未使用 |
| `io-uring` | — | 未使用（仅 Linux，且需内核 5.1+） |
| `process` | — | 未使用 |

**建议**：保留 `full`。原因：
1. `full` 是 tokio 官方推荐的桌面应用配置
2. 精确 feature 列表维护成本高，容易遗漏
3. `full` 排除的 feature（`io-uring`、`tracing`）本身不影响编译大小（feature gate 零成本）

---

## 3. 原文档缺失内容补充

### 3.1 原文档未覆盖的分析维度

| 维度 | 原文档 | 本补充 | 重要性 |
|------|--------|--------|--------|
| 功能重叠 | ✅ §2 详细分析 | — | 正确性 |
| **版本重复编译** | ❌ 未分析 | ✅ §1 基于 cargo tree | 编译时间/二进制体积 |
| **Workspace 卫生** | ❌ 仅记录已修复项 | ✅ §2.2 发现 9 处未规范化 | 版本一致性 |
| **未使用依赖** | ❌ 未分析 | ✅ §2.3 发现 4 处 | 依赖图简化 |
| **传递依赖成本** | ❌ 未量化 | ✅ §1.2 分类评估 | 编译时间 |
| **版本对齐** | ❌ 仅记录已修复项 | ✅ §2.4 发现 2 处对齐机会 | 生态一致性 |
| **Feature 优化** | ❌ 未分析 | ✅ §2.5 分析 tokio full | 编译大小 |

### 3.2 建议补充到原文档的章节

1. **§6 重复版本编译分析** — 将本文件 §1 内容整合
2. **§7 Workspace 卫生检查** — 将本文件 §2.2 内容整合
3. **§8 依赖清理建议** — 将本文件 §2.3 + §2.4 内容整合

---

## 4. 优化优先级排序

| 优先级 | 项目 | 工作量 | 收益 | 状态 |
|--------|------|--------|------|------|
| P0 | 移除 assistant-tools 的 tokio/tokio-util 未使用依赖 | 小（删 2 行） | 依赖图简化，减少 feature 传播 | ✅ 已完成 |
| P0 | 移除 assistant-security/assistant-sync 的 tokio 未使用依赖 | 小（删 2 行，检查测试） | 同上 | ✅ 已完成 |
| P1 | Workspace 卫生：dashmap 改 workspace 引用 | 小（改 1 行） | 版本一致性 | ✅ 已完成 |
| P1 | Workspace 卫生：tempfile/windows-future/平台依赖加入 workspace | 中（改 ~10 行） | 版本一致性，集中管理 | ✅ 已完成 |
| P2 | 升级 dirs 6→7 | 小（改 1 行 + 验证） | 消除双份 dirs 编译 | ✅ 已完成 |
| P3 | 升级 base64 0.22→0.23 | ~~中（改 18 处调用 API）~~ 小（零修改） | 项目侧统一（0.22 仍由上游传递） | ✅ 已完成 |
| P3 | keyring security-framework 双版本 | ~~中~~ 小（升级 keyring 3→4，v1 API 零修改） | macOS 编译时间 -30s，消除双份 security-framework | ✅ 已完成 |

---

## 5. cargo tree --duplicates 摘要

以下为 `cargo tree --duplicates --prefix indent` 输出中项目直接依赖相关的重复条目摘要（完整输出见 `_tree_duplicates.txt`）：

### base64 双版本（修复后）

```
base64 v0.22.1                          ← reqwest/tauri-codegen 传递依赖（上游锁定）
├── reqwest → assistant-app, assistant-providers, assistant-security
└── tauri-codegen

base64 v0.23.1                          ← 项目 workspace 依赖 ✅ + 传递依赖
├── assistant-agent
├── assistant-security
├── assistant-storage
├── assistant-sync
├── hyper-util → hyper-tls → reqwest
├── plist → tauri-utils
└── serde-saphyr → rust-i18n-support
```

### dirs 双版本（已消除 ✅）

项目已升级至 dirs 7，与 Tauri 生态统一，不再有双版本编译。

### security-framework 双版本（已消除 ✅）

升级 keyring 3→4 后，`apple-native-keyring-store` 仅依赖 `security-framework 3.7`，双版本已消除。

### core-foundation 双版本（上游传递，无法控制）

```
core-foundation v0.9.4                  ← core-graphics (mouse_position) + system-configuration (hyper-util/reqwest)
core-foundation v0.10.1                 ← 项目直接依赖 + security-framework 3.x
```

keyring 升级已消除 security-framework 导致的 core-foundation 双版本路径，但 core-graphics/system-configuration 仍拉入 0.9.4。

### windows-sys 三版本（Windows only）

```
windows-sys v0.59.0                     ← global-hotkey, winreg
windows-sys v0.60.2                     ← keyring, rfd
windows-sys v0.61.2                     ← arboard, tokio, mio, tauri-plugin-single-instance, ...
```

---

## 6. 修复执行记录

> 执行日期：2026-10-24 | `cargo check` 通过

### 6.1 P0：移除未使用的 tokio/tokio-util 依赖

| 文件 | 变更 |
|------|------|
| `crates/assistant-tools/Cargo.toml` | 移除 `tokio = { workspace = true }`、`tokio-util = { workspace = true }` |
| `crates/assistant-security/Cargo.toml` | 移除 `tokio = { workspace = true }` |
| `crates/assistant-sync/Cargo.toml` | 移除 `tokio = { workspace = true }` |

验证：全 crate 无 `use tokio`/`tokio::`/`#[tokio::test]` 引用，移除安全。

### 6.2 P1：Workspace 卫生修复

**workspace Cargo.toml 新增声明**：
```toml
tempfile = "3"
windows-future = "0.3"
core-foundation = "0.10"
security-framework = "3.0"
libsecret = "0.1"
```

**各 crate Cargo.toml 变更**：
- `assistant-tools`: `dashmap = "6"` → `dashmap = { workspace = true }`
- `assistant-security`: `windows-future`/`core-foundation`/`security-framework`/`libsecret`/`tempfile` 全部改为 `{ workspace = true }`
- `assistant-sync`: `tempfile = "3"` → `tempfile = { workspace = true }`
- `assistant-storage`: `tempfile = "3"` → `tempfile = { workspace = true }`
- `assistant-config`: `tempfile = "3"` → `tempfile = { workspace = true }`

### 6.3 P2：dirs 升级 6→7

`Cargo.toml` workspace: `dirs = "6"` → `dirs = "7"`

效果：`cargo tree --duplicates` 中 dirs 双版本条目消失。

### 6.4 P3：base64 升级 0.22→0.23

`Cargo.toml` workspace: `base64 = "0.22"` → `base64 = "0.23"`

代码零修改：项目已使用 `base64::Engine` trait + `base64::engine::general_purpose::STANDARD` API（0.22 即支持）。

残留：`base64 v0.22.1` 仍由 `reqwest`/`tauri-codegen` 传递依赖，需等上游升级。

### 6.5 P3：keyring 升级 3→4

`Cargo.toml` workspace: `keyring = { version = "3", features = ["apple-native", "windows-native", "linux-native"] }` → `keyring = { version = "4", features = ["v1"] }`

代码零修改：`v1` feature 保留与 3.x 完全相同的 API（`Entry::new`/`Error::NoEntry`/`set_password`/`get_password`/`delete_credential`），源码级验证通过。

效果：`cargo tree --duplicates --target all -p security-framework` 无重复输出，security-framework 双版本已消除。

注意：`core-foundation` 仍有 0.9.4 + 0.10.1 双版本，但来源为 `core-graphics`（mouse_position）和 `system-configuration`（hyper-util/reqwest），非 keyring 导致，项目无法控制。

### 6.6 修复后重复依赖统计

| 指标 | 修复前 | 修复后 |
|------|--------|--------|
| `cargo tree --duplicates` 条目数 | 91 | 88 |
| `dirs` 双版本 | ✗ 存在 | ✅ 消除 |
| `base64` 项目侧版本 | 0.22 | ✅ 0.23（上游 0.22 残留） |
| `security-framework` 双版本 | ✗ 存在（keyring 3.x 导致） | ✅ 消除 |
| `core-foundation` 双版本 | ✗ 存在（security-framework + core-graphics） | ⚠️ 部分消除（security-framework 路径已消除，core-graphics/system-configuration 路径残留） |
| 未使用 tokio 依赖 | 4 处 | ✅ 0 处 |
| Workspace 卫生违规 | 9 处 | ✅ 0 处 |
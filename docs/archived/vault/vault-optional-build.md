# Vault 可选编译（编译拆分）方案

> 状态：L1 已实施并验证通过；L2~L5 未实施
> 2026-11-11 更新：`get_vault_conf` 已按 feature-mount-traversal-spec.md（已归档） §4 S7 更名为 `vault_get_conf` 并迁入 vault 插件；下文 L1 记录中「命令保留在 core / `main.tsx` 无条件调用」等描述按历史口径阅读，最新形态以该规格为准。
> 事实基线：`docs/store/crate-architecture.md`、`docs/store/compile-time-attribution.md`
> 覆盖范围：`src-tauri/`（Cargo workspace + Tauri 集成层）、`src/`（前端）

---

## 0. 结论速览

| # | 结论 | 依据 |
|---|---|---|
| 1 | 排除单元是**三件套** `assistant-vault` + `assistant-sync` + `assistant-security`，不是单个 vault | `assistant-security` 在主功能侧零消费者，单独排 vault 会让它变成死重 |
| 2 | 后端切口只有 4 个文件需要改，`src-tauri/src/cmd/vault/` 9 个文件整目录随 feature 消失 | 59 处外部 crate 引用全部落在 `src-tauri/src/cmd/vault/` 内，零跨模块泄漏 |
| 3 | 前端切口 17 个文件，但只改 **1 处开关**：vault 声明为**一个特性模块**，设置 / 路由 / 侧栏全部由同一份清单派生 | 内置 33 条设置零改动；vault 的 10 条字段 + 2 section + 1 page + 1 路由 + 1 菜单项收敛到一个 `FeatureModule`，见第 6 章 |
| 4 | 编译收益有上限：48 个包可省、串行 78.2s、占比 7.2%、wall 5~15s | `compile-time-attribution.md` 实测；`rusqlite`/`libsqlite3-sys` 10.3s 属主功能固定成本，排除 vault 也省不掉 |
| 5 | 现有两个扫描型门禁测试**不需要改造**，但应新增 1 条断言 | 它们做的是源码文本扫描，不参与编译；cfg 属性不影响文本匹配 |
| 6 | 真正需要前置验证的只有一件事：`generate_handler!` 是否支持条目级 `#[cfg]` | 已核 `tauri-macros` 源码，**支持**，见 4.4 |

---

## 1. 目标与非目标

### 1.1 目标

构建一个**不含 vault 能力**的二进制：`cargo build --no-default-features`（或显式 `--features ""`）后，主功能（chat / provider / tool / storage / orb / 设置）的行为与全量构建完全一致，只是没有密码保险库。

### 1.2 非目标（明确不做）

- 不做运行时开关：feature 只在编译期生效，二进制内没有「关掉 vault」的运行时分支
- 不做插件化：不把 vault 抽成动态库或外部进程
- 不为「方便排除」而改业务逻辑：vault 代码本身不改写，只做编译裁剪
- 不支持「同一二进制内热切换」

### 1.3 验收口径（5 条硬指标）

1. `cargo check --no-default-features` 零错误零警告（新增警告同样视为不通过）
2. `cargo check`（默认 feature）与 `cargo test -p assistant-vault` 全绿，行为与改造前一致
3. 前端 `VAULT_ENABLED=0 npm run build` 成功，入口 chunk 与所有被引用 chunk 中不含 vault 代码；`npm run typecheck`、`npm run lint` 通过
4. 排除构建下，前端不出现任何 vault 入口（侧边栏、设置分类、命令面板、启动期 invoke）
5. L2.5（6.8 P1）完成后，`MainLayout.tsx` / `AppSidebar.tsx` 源码中不出现任何 feature id 字面量（`vault` 等），路由与侧栏完全派生自 `getFeatures()`

---

## 2. 排除单元：为什么是三件套

| crate | 主功能侧消费者 | 排除后处置 | 理由 |
|---|---|---|---|
| `assistant-vault` | 无（仅 `src-tauri/src/cmd/vault/*`） | 随 vault 消失 | vault 主体 |
| `assistant-sync` | 无（仅 `assistant-vault`） | 随 vault 消失 | crate-architecture 已核：唯一消费者是 vault |
| `assistant-security` | 无（仅 `src-tauri/src/cmd/vault/*`） | 随 vault 消失 | 59 处引用全部在 `cmd/vault` 内；不排除就是零消费者死重 |
| `assistant-crypto` | 有（api_key 等主功能） | **保留** | 内部 `vault_crypto` 与主功能模块混装，属 L5 独立议题 |
| `assistant-config` | 有 | **保留** | 配置机制库（`ConfGroup` / `apply_patch`）；`VaultConf` 已迁入 `assistant-vault/src/conf.rs`，该 crate 不依赖任何 vault crate |
| `rusqlite` / `libsqlite3-sys` | 有 | **保留** | 主功能 DB 固定成本，10.3s |

关键判断：`assistant-vault` 不依赖 `assistant-security`，二者是主 app 直接依赖的**并列** crate，耦合只发生在 `src-tauri/src/cmd/vault/*` 的代码层。因此排除必须在同一层同时切断三个依赖边，否则会出现「security 还在图里但没人用」的状态。

---

## 3. 现状耦合清单（实测）

### 3.1 后端代码层

`src-tauri/src/cmd/vault/` 9 个文件、1,928 行：

| 文件 | 行数 |
|---|---|
| `audit.rs` | 224 |
| `biometric.rs` | 101 |
| `entries.rs` | 320 |
| `lifecycle.rs` | 104 |
| `mod.rs` | 272 |
| `recovery.rs` | 188 |
| `reveal.rs` | 101 |
| `ssh.rs` | 246 |
| `sync.rs` | 372 |

59 处 `assistant_vault::` / `assistant_security::` / `assistant_sync::` 引用**全部**落在上述 9 个文件内。跨模块边界有 3 个符号，全部经 `pub use vault::*` 通配重导出泄漏到 `cmd` 命名空间：

| 符号 | 引用点 | 性质 |
|---|---|---|
| `cmd::setup` | `src-tauri/src/setup.rs:90` | 实际解析到 `cmd::vault::setup`，应用初始化钩子 |
| `cmd::cleanup_on_exit` | `src-tauri/src/lib.rs:267` | 实际解析到 `cmd::vault::cleanup_on_exit`，退出时清空剪贴板 |
| `cmd::vault::on_vault_conf_changed` | `src-tauri/src/cmd/config.rs:98` | 配置变更副作用回调 |

### 3.2 命令层

- `src-tauri/src/lib.rs:98-162`：`generate_handler!` 内 65 条 `cmd::vault_*`
- `src-tauri/src/lib.rs:42`：`cmd::get_vault_conf`（配置类命令，依赖仅 `assistant-config`）

### 3.3 状态层

`src-tauri/src/cmd/vault/mod.rs:195` 的 `setup()` 注册 6 个 managed state 并启动 1 个后台监听：

| 注册项 | 类型来源 |
|---|---|
| `SecurityDbState` | `assistant-vault` |
| `RevealGuard` | `assistant-security` |
| `ClipboardGuard` | `assistant-security` |
| `SshAgentState` | 本地定义（字段用 `assistant-security` 语义） |
| `SyncEngine` | `assistant-vault`（配置来自 `assistant-sync`） |
| `ScreenLockMonitor` | `assistant-security`，`spawn_screen_lock_watcher` 内部还会回调 `vault_show_unlock_window` |

### 3.4 配置层

| 项 | 位置 | 是否依赖 vault 代码 |
|---|---|---|
| `VaultConf`（13 字段） | `src-tauri/crates/assistant-vault/src/conf.rs` | 否（仅依赖 `assistant-config` 机制层） |
| `VaultConfDto` | `src-tauri/crates/assistant-vault/src/dto/conf.rs`（vault 契约域） | 否 |
| `get_vault_conf` | `src-tauri/src/cmd/config.rs:147` | 否 |
| `patch_conf` 的 `"vault"` 分支 | `src-tauri/src/cmd/config.rs:167-173` | 否（写 JSON） |
| `apply_vault_side_effects` | `src-tauri/src/cmd/config.rs:96-98` | **是** |
| 启动期 `VaultConf::load()` | `src-tauri/src/setup.rs:31` | 否 |

### 3.5 事件层

`src-tauri/src/protocol/events.rs` 3 个常量：`vault:locked`、`vault:unlocked`、`vault:biometric-enroll-step`。仅为字符串常量，消费方全在 `src/vault/` 内。

### 3.6 前端接入点

`src/vault/` 之外共 17 个文件引用 vault。按「是否需要在注册表里真正断开」分成两类：

**A 类：必须改（7 处）**

| # | 位置 | 引用内容 | 收口方式 |
|---|---|---|---|
| F0 | 新增 `src/features/vault.ts` | 无 | 唯一读取 `__VAULT_ENABLED__` 的位置，导出 `VAULT_ENABLED` |
| F1 | `src/settings/core/pageRegistry.ts:125-142,153,164,181,188` | vault 页面定义、`SETTINGS_PAGES`/`SETTINGS_DETAIL_PAGES` 成员、`vault` 分类 | 页面定义改三元 + 数组展开 + 空分类过滤，见 5.3 |
| F2 | `src/settings/core/field/fieldRegistry.ts:95-120` | 4 个查询函数 | 查询函数统一过滤 `group === "vault"` 与 `pageId === "vault"`，见 5.3 |
| F3 | `src/layouts/MainLayout.tsx:18-19,52-55` | `lazyWithRecovery` 懒加载 `@/vault` + `/vault` 路由 | `VaultApp` 改三元 + 路由项条件展开，见 5.3 |
| F4 | `src/components/AppSidebar.tsx:135-136` | 侧边栏 `/vault` 菜单项（硬编码） | 菜单项数组条件展开，见 5.3 |
| F5 | `src/main.tsx:44` | 启动期 `store.loadVaultConf()` | 早返回，见 5.3 |
| F6 | `src/services/settingEvents.ts:72` | `group === "vault"` 时 `loadVaultConf()` | 早返回，见 5.3 |

> 上表是**最小改动基线（方案 A）**。6.8 将该基线升级为「特性模块派生」：F1–F4 不再各自打条件，而是统一由 `features/aggregate.ts` 产出，`MainLayout` / `AppSidebar` 内不再出现 `VAULT_ENABLED`。两种收口方式都满足 1.3 的四条验收口径，差别在扩展性与可维护性。

**B 类：明确不动（12 处）**

| 文件 | 引用处数 | 为什么不动 |
|---|---|---|
| `settings/core/field/fieldDefinitions.ts` | 10 条 `group: "vault"` 字段 + 2 个 section | 纯数据声明，无副作用；F2 在查询侧过滤后永不对外暴露，见 5.2 的设计取舍 |
| `settings/core/field/fieldRegistry.ts` 的 `CONF_KEY_MAP` / `CONF_LOAD_KEY_MAP` | 2 | **类型源**：`ConfGroup = keyof typeof CONF_KEY_MAP`。删 `vault` 键会让 `ConfGroup` 塌陷、`VaultConf` 相关类型全部失效 |
| `components/commandPalette/commandAggregation.tsx` | 1（`GROUP_TO_PAGE` 的 `vault`） | 遍历源是 `getAllDefinitions()` / `getAllSections()`，F2 过滤后自然为空 |
| `settings/overview/SettingsOverview.tsx` | 6 | `allGroups` 由 `getSectionsByPage` 推导，vault section 被过滤后 `has("vault")` 恒为 false，两个 `useEffect` 分支自动跳过 |
| `settings/core/types/confTypes.ts` | 14 | `VaultConf` 接口仍在用：`get_vault_conf` 命令保留，类型不会悬空 |
| `settings/core/types/confUtils.ts` | 2 | 同上 |
| `settings/core/types/pageTypes.ts` | 2 | `SettingsPageId` / `SettingsCategoryId` 保留 `"vault"` 字面量，避免联合类型收窄引发连锁改动 |
| `settings/core/settingsDispatcher.ts` | 3 | 仅类型引用 |
| `settings/core/settingsStore.ts` | 9 | `vaultConf` 槽位与 `loadVaultConf` 保留，F5/F6 只决定是否调用 |
| `settings/core/settingService.ts` | 2 | 仅类型引用 |
| `settings/pages/VaultSettingsPage.tsx` | 21 | 仅被 `pageRegistry.ts` 懒加载引用，F1 不注册即不进被加载的 chunk |
| `settings/shared/renderers/BiometricSwitchRender.tsx` | 13 | 仅被 `fieldDefinitions.ts:16-20` 的 lazy 常量引用，残留成本见 5.7 |

> 表内「不动」不等于「零成本」：`BiometricSwitchRender` 仍会产出一个不被请求的异步 chunk，详见 5.7 的残留说明。

### 3.7 已核实「无耦合」的项

| 项 | 核查结果 |
|---|---|
| `src-tauri/capabilities/default.json` | 无任何 vault 条目，排除后不触发 ACL 校验问题 |
| `src-tauri/tauri.conf.json` | 无 vault 窗口定义，解锁窗口是运行时动态创建 |
| 后端事件常量 | 仅字符串，无编译依赖 |
| i18n 词条 | `vault.*` 命名空间可保留，未使用词条不产生成本 |

---

## 4. 后端方案

### 4.1 feature 拓扑

```
assistant-app (features: default = ["vault"])
└── vault
    ├── dep:assistant-vault
    ├── dep:assistant-sync
    ├── dep:assistant-security
    └── (L5) assistant-crypto/vault-crypto
```

单一 feature 名为 `vault`，默认开启。不拆 `vault-sync` / `vault-ssh` 等子开关：sync 与 ssh 的实现深度嵌在 vault 的条目模型里，拆子开关会让 `cmd/vault/*` 内部再长一层 cfg，收益远小于复杂度。

### 4.2 Cargo 改动

```toml
# src-tauri/Cargo.toml
[features]
default = ["vault"]
vault = [
    "dep:assistant-vault",
    "dep:assistant-sync",
    "dep:assistant-security",
]

[dependencies]
assistant-vault = { workspace = true, optional = true }
assistant-sync = { workspace = true, optional = true }
assistant-security = { workspace = true, optional = true }
```

`[workspace.dependencies]` 中的 path 声明保持不变，`optional = true` 加在消费点即可。

### 4.3 门控点清单

| 文件 | 现状 | 改法 |
|---|---|---|
| `src-tauri/src/cmd/mod.rs:14` | `mod vault;` | 前置 `#[cfg(feature = "vault")]` |
| `src-tauri/src/cmd/mod.rs:29` | `pub use vault::*;` | 同上 |
| `src-tauri/src/cmd/vault/mod.rs:195` | `pub fn setup` | 改名 `pub fn vault_setup`（见 4.5） |
| `src-tauri/src/cmd/vault/mod.rs:290` | `pub fn cleanup_on_exit` | 改名 `pub fn vault_cleanup_on_exit`（同 4.5 陷阱） |
| `src-tauri/src/setup.rs:90` | `crate::cmd::setup(handle);` | `#[cfg(feature = "vault")] crate::cmd::vault_setup(handle);` |
| `src-tauri/src/lib.rs:267` | `cmd::cleanup_on_exit(app_handle);` | `#[cfg]` 门控 + `#[cfg(not(...))] let _ = app_handle;` |
| `src-tauri/src/cmd/config.rs:96-102` | `apply_vault_side_effects` 调 vault | 双函数 + cfg，见下 |
| `src-tauri/src/lib.rs:98-162` | 65 条 `cmd::vault_*` | 每条前置 `#[cfg(feature = "vault")]` |
| `src-tauri/src/lib.rs:42` | `cmd::get_vault_conf` | **保留不动** |

`apply_vault_side_effects` 的推荐写法（**双函数**，保留调用点、彻底避开 unused 警告）：

```rust
#[cfg(feature = "vault")]
fn apply_vault_side_effects(prev: &VaultConf, conf: &VaultConf) {
    crate::cmd::vault::on_vault_conf_changed(prev, conf);
}

#[cfg(not(feature = "vault"))]
fn apply_vault_side_effects(_prev: &VaultConf, _conf: &VaultConf) {}
```

不推荐在单函数体内写 `#[cfg(not(...))] let _ = (prev, conf);`：调用点虽保留，但函数签名参数名不带下划线时，`unused_variables` 警告只被 `let _` 掩盖，语义上是「空实现占位」，不如显式空函数清晰。

`lib.rs` 的 handler 列表（每条约目各带一个属性，vault 段集中在末尾）：

```rust
.invoke_handler(tauri::generate_handler![
    // ... 主功能命令 ...
    cmd::get_vault_conf,
    #[cfg(feature = "vault")]
    cmd::vault_init,
    #[cfg(feature = "vault")]
    cmd::vault_unlock,
    // ... 其余 63 条 ...
])
```

### 4.4 `generate_handler!` 支持条目级 `#[cfg]`（可行性验证）

这是整个方案唯一「可能不成立」的前提，已通过读 `tauri-macros` 源码确认：

1. `generate_handler!` 是 proc macro，入口 `parse_macro_input!(item as command::Handler)`
2. `CommandDef::parse` 先调用 `Attribute::parse_outer()` 收集条目属性，再解析路径：

```rust
impl Parse for CommandDef {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let attrs = input.call(Attribute::parse_outer)?;
    let path = input.parse()?;
    Ok(CommandDef { path, attrs })
  }
}
```

3. `From<Handler> for TokenStream` 把属性原样拼到 match 分支前：

```rust
quote::quote!(move |#invoke| {
  let #cmd = #invoke.message.command();
  match #cmd {
    #(#(#attrs)* #command_name_macros => #wrappers!(#paths, #invoke),)*
    _ => { return false; },
  }
})
```

结论：`#[cfg(feature = "vault")]` 会落在 match 分支上，未启用时该分支连同 `cmd::vault_init` 路径与 wrapper 宏一起被裁剪，**不会产生未解析路径**。

### 4.5 通配重导出重名陷阱（必改）

`src-tauri/src/cmd/mod.rs` 用 `pub use vault::*;` 通配重导出，两个 vault 内部函数因此泄漏成 `cmd` 命名空间里的「通用名」：

| 泄漏符号 | 实际定义 | 调用点 | 名字看起来像 |
|---|---|---|---|
| `cmd::setup` | `cmd::vault::setup` | `src-tauri/src/setup.rs:90` | 命令层通用初始化 |
| `cmd::cleanup_on_exit` | `cmd::vault::cleanup_on_exit` | `src-tauri/src/lib.rs:267` | 应用级退出清理 |

一旦 `mod vault` 被 cfg 掉，这两处调用直接编译失败。改法：分别改名为 `vault_setup` / `vault_cleanup_on_exit`，调用点显式写全名并加 `#[cfg]`。这样既修掉隐式依赖，也避免未来其它命令模块引入同名函数时发生歧义。

`cleanup_on_exit` 的调用点在闭包内，门控后需补一句 `#[cfg(not(feature = "vault"))] let _ = app_handle;`，否则排除构建下闭包参数 `app_handle` 变成未使用变量，触发 `unused_variables` 警告。

注：这类「通配泄漏」不会报错，也不会被现有门禁测试发现——`command_registry.rs` 只比对 `cmd::vault_*` 命令，`cleanup_on_exit` 不是 `#[command]`。只有真正跑一次 `cargo check --no-default-features` 才暴露。

### 4.6 配置层取舍

结论：**配置读接口保留，副作用回调门控**。

| 项 | 决策 | 原因 |
|---|---|---|
| `assistant-config::vault_conf` | 保留（**已被 `feature-config-ownership-spec.md`（已归档） 替代**：`VaultConf` 现位于 `assistant-vault/src/conf.rs`） | 纯 JSON，无 vault 代码依赖；`src-tauri/crates/assistant-config` 本身是主功能依赖 |
| `VaultConfDto` | 保留（**已被 `feature-config-ownership-spec.md`（已归档） 替代**：现位于 vault 契约域 `assistant-vault/src/dto/conf.rs`） | 同上 |
| `get_vault_conf` 命令 | 保留 | `src/main.tsx:44` 启动期无条件调用，删了会让排除构建启动即 invoke 失败 |
| `patch_conf` 的 `"vault"` 分支 | 保留 | 只是写 JSON 文件，无副作用即可 |
| `apply_vault_side_effects` | 门控 | 唯一真实 vault 调用点 |

---

## 5. 前端方案

### 5.1 编译期常量与门控模块

沿用 `vite.config.ts` 已有的 `define` 机制（现用于 `__BUILD_DATE__`）：

```ts
// vite.config.ts
define: {
  __BUILD_DATE__: JSON.stringify(new Date().toISOString().slice(0, 10)),
  ...featureDefines(),
},
```

> **v2.5 修订（I24 E4）**：`__VAULT_ENABLED__` 不再手写在 `define` 里，改由手写清单 `config/features.json` 派生——`featureDefines()` 遍历清单，按每项的 `env` 生成 `__<ENV>__`；`process.env[env]` 有值时以环境变量为准（`-Without vault` 走这条），无值时取清单的 `default`。原写法 `JSON.stringify(process.env.VAULT_ENABLED !== "0")` 的问题是「环境变量缺省即开启」，默认值散落在配置里、与后端 `Cargo.toml [features] default` 无任何约束关系；改为清单后，默认值只有一处，并由 `scripts/feature-build.ps1` 启动断言与 cargo 双向对齐。

```ts
// src/vite-env.d.ts
declare const __VAULT_ENABLED__: boolean;
```

注意：**不要用 `import.meta.env`**。Vite 只向 `import.meta.env` 注入 `VITE_` 前缀变量，`VAULT_ENABLED` 不会被注入；`define` 是唯一能拿到任意环境变量的方式。常量在构建期被替换为字面量 `true`/`false`，三元分支经 Rollup 常量折叠后可被完全 DCE。

全局常量**只在一个文件里读取一次**，其余模块从门控模块导入：

```ts
// src/features/vault.ts（新增，仅此一个文件读 __VAULT_ENABLED__）
export const VAULT_ENABLED: boolean = __VAULT_ENABLED__;
```

这样全仓搜索 `__VAULT_ENABLED__` 只有一个命中，开关语义集中可审计；各消费点依赖的是普通导出常量 `VAULT_ENABLED`，Rollup 会做跨模块常量传播与折叠，`VAULT_ENABLED ? [x] : []` 在关闭时折叠为空数组。

### 5.2 设计取舍：为什么不在注册点就地 `if`

`defineSetting` / `defineSection` / `definePage` 三个注册函数都是**模块加载即执行**的副作用式注册。最直观的改法是把 vault 定义包进 `if (VAULT_ENABLED)` 或写成 `VAULT_ENABLED ? [...] : []`，但有三个实际问题：

| 问题 | 后果 |
|---|---|
| 侵入数据定义文件 | `fieldDefinitions.ts` 的 10 条字段 + 2 个 section 整块缩进漂移，diff 达 87 行，且把 feature 判断混进「数据声明」职责 |
| 语义变成「注册但不生效」 | `? [...] : []` 让字段仍进入 `definitions` 数组，只是汇总时被丢弃，等于两次判断做一件事 |
| lazy import 重复内联 | `BiometricSwitchRender` 的 `lazyWithRecovery` 随数组条件化时，同一段加载逻辑要在多处维护 |

因此改为**「定义侧零改动 + 查询侧收口」**：

| 层 | 职责 | 感知 vault |
|---|---|---|
| 定义层 `fieldDefinitions.ts` / `pageTypes.ts` | 声明全部字段与类型，与 feature 无关 | 否，零改动 |
| 注册表层 `fieldRegistry` / `pageRegistry` | 维护注册表，提供查询 API | **是，唯一过滤点** |
| 消费层 `MainLayout` / `AppSidebar` / `SettingsOverview` / 命令面板 | 只通过注册表 API 取数据 | 否，过滤后自然看不到 vault |

关键前提：**vault 字段是纯数据对象**。`fieldDefinitions.ts` 中 10 条 vault 字段的 `customRender` 只在渲染时触发 `import()`，注册本身无副作用。因此「照常注册、查询时过滤」在语义上安全。

代价与补偿：字段定义照常进入内存数组（10 条对象，可忽略），换来定义文件零改动、过滤点收敛到 4 个函数，以及「关闭 vault 不会漏掉任何一条引用」的强保证——过滤发生在唯一出口。

**但本方案有两个结构性缺陷**：过滤点是「需要人工维护」的补丁（新增消费方要重新扫描），且顶层 `defineSetting` 副作用使 vault 数据无法被 DCE（残留见 5.4）。第 6 章给出更彻底的「不注册」方案（内置 + 扩展双通道），**推荐优先采纳**；本节保留为不重构架构时的降级路径。

### 5.3 7 处改动（方案 A：最小改动备选）

> 本节是**不重构注册架构**时的最小改动方案。若采纳第 6 章的「内置 + 扩展」方案，本节 F1 / F2 被扩展清单取代，只需保留 F0、F3~F6。

#### F0 新增门控模块

```ts
// src/features/vault.ts
export const VAULT_ENABLED: boolean = __VAULT_ENABLED__;
```

#### F1 `pageRegistry.ts`：页面定义改三元 + 数组展开 + 空分类过滤

vault 页面定义由无条件 `const vault = definePage({...})` 改为可空常量，`definePage` 内是动态 `import()`，折叠为 `null` 后 Rollup 会把整个调用连同 `import()` 一起删除：

```ts
const vaultPage = VAULT_ENABLED
  ? definePage({
      id: "vault",
      categoryId: "vault",
      label: "common.navigation.vault_settings.title",
      keywords: ["vault", "password", "ssh", "biometric", "cipher", "密码", "保险库", "加密"],
      icon: SafetyCertificateOutlined,
      load: () =>
        import("../pages/VaultSettingsPage").then((m) => ({
          default: m.VaultSettingsPage,
        })),
    })
  : null;
```

`SETTINGS_PAGES` 与 `SETTINGS_DETAIL_PAGES` 用条件展开（TS 在 truthy 分支自动把 `vaultPage` 收窄为 `SettingsPageManifest`，无需非空断言）：

```ts
export const SETTINGS_PAGES: readonly SettingsPageManifest[] = [
  general, appearance, shortcuts, providers, agents, storage,
  ...(vaultPage ? [vaultPage] : []),
  about,
];

export const SETTINGS_DETAIL_PAGES: readonly SettingsPageManifest[] = [
  general, appearance, shortcuts, providers, agents, storage,
  ...(vaultPage ? [vaultPage] : []),
  about,
];
```

`SETTINGS_CATEGORIES` 由 `CATEGORY_ORDER.map` 生成，vault 分类会退化成 `pages: []` 的空分类，需过滤：

```ts
export const SETTINGS_CATEGORIES: readonly SettingsCategory[] =
  CATEGORY_ORDER.map((id) => ({
    id,
    label: CATEGORY_LABELS[id],
    pages: SETTINGS_PAGES.filter((p) => p.categoryId === id),
  })).filter((c) => c.pages.length > 0);
```

这一步是**单点收益**：`SETTINGS_DETAIL_PAGES` 同时被 `AppSidebar` 的 settings 子菜单和命令面板消费，过滤后两处自动收敛，无需各自判断。

#### F2 `fieldRegistry.ts`：4 个查询函数统一过滤

定义侧不动，只在出口加过滤。vault 字段的判定用 `group === "vault"`，vault section 用 `pageId === "vault"`：

```ts
const isVaultGroup = (g: ConfGroup) => g === "vault";

export function getDefinitionsByGroup(group: ConfGroup): SettingDefinition[] {
  if (!VAULT_ENABLED && isVaultGroup(group)) return [];
  return definitions.filter((d) => d.group === group);
}

export function getDefinitionsBySection(sectionId: string): SettingDefinition[] {
  return definitions.filter((d) => d.section === sectionId);
}

export function getAllDefinitions(): readonly SettingDefinition[] {
  return VAULT_ENABLED ? definitions : definitions.filter((d) => !isVaultGroup(d.group));
}

export function getSectionsByPage(pageId: string): SettingSectionDef[] {
  if (!VAULT_ENABLED && pageId === "vault") return [];
  return sections.filter((s) => s.pageId === pageId);
}

export function getAllSections(): readonly SettingSectionDef[] {
  return VAULT_ENABLED ? sections : sections.filter((s) => s.pageId !== "vault");
}
```

`getSectionById` 无需改：vault section 的 `id` 只可能来自 `getSectionsByPage` 的返回值，上游已过滤。

**`CONF_KEY_MAP` / `CONF_LOAD_KEY_MAP` 不可删**。它们同时是类型源（`ConfGroup = keyof typeof CONF_KEY_MAP`），删掉 `vault` 键会让 `ConfGroup` 从 4 元联合塌成 3 元，`ConfKey` / `ConfLoadKey` 及所有 `Record<ConfGroup, ...>` 使用点连锁报错。

#### F3 `MainLayout.tsx`：主路由条件化

```tsx
const VaultApp = VAULT_ENABLED
  ? lazyWithRecovery(() => import("@/vault").then((m) => ({ default: m.VaultApp })))
  : null;
```

```tsx
{VAULT_ENABLED && VaultApp && (
  <Route
    path="/vault"
    element={
      <FeatureErrorBoundary featureName="密码保险库">
        <VaultApp />
      </FeatureErrorBoundary>
    }
  />
)}
```

双条件 `VAULT_ENABLED && VaultApp` 是给 TS 的类型收窄用的（`VaultApp` 为 `RecoverableLazyComponent | null`，JSX 不接受 null）；运行时第二个条件恒真，折叠后不产生额外成本。

#### F4 `AppSidebar.tsx`：`/vault` 菜单项条件展开

`/vault` 是硬编码菜单项，不经过 `SETTINGS_DETAIL_PAGES`，必须单独处理：

```tsx
items={[
  ...toolMenuItems,
  { type: "divider" as const },
  ...(VAULT_ENABLED
    ? [{
        key: "/vault",
        label: t("common.navigation.vault"),
        icon: <SafetyCertificateOutlined />,
      }]
    : []),
  {
    key: "/history",
    label: t("common.navigation.chat_history"),
    icon: <HistoryOutlined />,
  },
  // ... settings-sub 保持不变 ...
]}
```

settings 子菜单里的 vault 项**不用改**，它消费 `SETTINGS_DETAIL_PAGES`，F1 已过滤。

#### F5 / F6 启动期与事件回调

```ts
// src/main.tsx —— bootstrap() 内部
if (VAULT_ENABLED) await store.loadVaultConf();

const lang = useSettingsStore.getState().appConf?.language ?? "zh-CN";
await applyLocaleChange(lang);
```

```ts
// src/services/settingEvents.ts
if (group === "vault") {
  if (VAULT_ENABLED) void loadVaultConf();
}
```

**`main.tsx` 不能写 `if (!VAULT_ENABLED) return;`**：该处 `return` 退出的是整个 `bootstrap()`，会把紧随其后的 `applyLocaleChange(lang)` 一起跳过，导致排除构建下界面语言不生效。这是启动主路径，必须用 `if (VAULT_ENABLED)` 正条件包裹单条语句。

`get_vault_conf` 命令本身保留（后端 4.6 已定），因此 `store.loadVaultConf` 的实现无需改动，只是排除构建下不再被调用。

### 5.4 残留成本

`BiometricSwitchRender` 的 `lazyWithRecovery` 定义仍在 `fieldDefinitions.ts` 顶层（第 16-20 行），按 5.2 原则该文件零改动，因此这个 `import()` 会在构建产物中留下一个**独立异步 chunk**。

| 项 | 影响 |
|---|---|
| 是否进入入口 chunk | 否，它是异步 chunk |
| 是否会被请求 | 否，唯一引用它的 vault 字段被 F2 过滤后永不渲染 |
| 体积 | 单个小组件（生物识别开关），无第三方依赖 |
| 消除方式 | 把 `customRender` 的 lazy 定义移入 `VAULT_ENABLED` 三元。代价是 `fieldDefinitions.ts` 出现一处条件表达式，与「数据定义零改动」冲突 |

结论：列为**可选优化项**，不纳入 L2 验收。验收口径相应表述为「入口 chunk 与所有被引用 chunk 中不含 vault 代码」，而不是「产物中不存在任何 vault 相关 chunk」。

### 5.5 不做的事

**不删除 `src/vault/` 目录**。理由：

- 它是 `tsc` 的输入，删除会让 `npm run typecheck` 少覆盖 7,497 行，且切回默认构建时要还原
- `ts-rs` 生成的契约文件位于该目录，删除后 `contract:gen` 与源码树不一致
- 关闭时它只是「不被任何启用路径引用」，不进入入口 chunk，也不产生运行时成本

---

## 6. 设置前端注册架构重构（Manifest 化）

> 本章回答一个前置问题：**为什么当前的注册式架构连「取消注册一个功能」都做不到**，以及如何重构。

### 6.1 现状诊断：注册式架构的三个硬约束

设置系统目前是「模块加载即注册」的可变单例模型：

| 注册表 | 位置 | 容器 | 注册函数 |
|---|---|---|---|
| 字段 | `settings/core/field/fieldRegistry.ts:71` | `const definitions: SettingDefinition[] = []` | `defineSetting()` push |
| 分区 | `settings/core/field/fieldRegistry.ts:74` | `const sections: SettingSectionDef[] = []` | `defineSection()` push |
| 字段类型渲染器 | `settings/core/field/fieldRegistry.ts:76` | `const typeRenderers = new Map()` | `registerSettingType()` |
| 副作用 | `settings/core/sideEffectRegistry.ts:9` | `const sideEffectHandlers = new Map()` | `registerSideEffect()` |
| Agent 类型 | `settings/core/agentKind/agentModeRegistry.ts:6` | `const kindMap = new Map()` | `defineAgentKind()` |
| Provider 预设 | `settings/core/provider/providerPresetRegistry.ts:8` | `const presetMap = new Map()` | `defineProviderPreset()` |

六个注册表模式完全一致：**模块级可变容器 + 只有 add 没有 remove + 注册靠 import 副作用触发**。由此产生三个硬约束：

| # | 根因 | 具体表现 |
|---|---|---|
| R1 | 注册表是**可变单例**，无 unregister / 无作用域 | 想移除 vault 的 10 条字段，只能从 `definitions` 数组里逐条删，没有「撤销 vault 全部注册」的原子操作 |
| R2 | 注册是**导入副作用** | `import "…/fieldDefinitions"` 一执行就完成注册。任何路径 import 一次即污染全局，无法按 feature 撤回；`main.tsx:36` 的 `initSettingSystem()` 同样是裸副作用调用 |
| R3 | 定义与注册**混在同一文件** | `fieldDefinitions.ts` 既声明数据又 push 注册表，导致「只要数据、不要注册」不可表达 |

R2 还带来一个对本次任务更致命的后果：**顶层副作用使 Rollup 无法删除 vault 定义**。`defineSetting({...})` 是模块求值时的函数调用，Rollup 无法证明它无副作用，只能保守保留。这就是 5.2 只能选择「照常注册、查询时过滤」的根本原因——不是不想在编译期删，是删不掉。

R1 的代价已经在实际工作中体现：vault 的排除要改 7 处接入点（5.3），且必须逐一确认 `commandPalette`、`SettingsOverview` 这类「间接消费方」不会泄漏。每加一个消费方，就要重新扫一遍全部注册表的读取路径。

### 6.2 候选方案对比

| 维度 | A：查询侧过滤（5.3 现状方案） | B：全量 Manifest 聚合 | C：Scoped 注册 | D：内置 + 扩展双通道（本章推荐） |
|---|---|---|---|---|
| 取消注册的改动点 | 7 处 | 1 处 | 1 处 | **1 处**（扩展清单三元） |
| 迁移代码量 | ~50 行改动，0 搬迁 | **~450 行搬迁**（core 33 条 + vault 10 条全搬） | ~50 行 | **~90 行搬迁**（只搬 vault） |
| 能否编译期删除 vault 数据 | 否 | 是 | 否 | **是** |
| core 代码是否要动 | 否 | 是（33 条全搬） | 否 | **否** |
| 新增 feature 的改动点 | 改 N 个消费点 | 加一个 manifest | 加一个 register 调用 | 加一个扩展项 |
| 残留产物 | `BiometricSwitchRender` chunk | 无 | 同 A | 无 |
| 主要风险 | 漏改一处即静默泄漏 | 大范围搬迁引入回归 | 全局单例仍在 | 双通道查询合并、去重 |

B 的致命弱点是**搬迁量**：为了统一架构把 core 的 33 条内置设置也改成 manifest，改动面覆盖整个设置系统，与「为排除 vault 而重构」的投入产出比不成立。D 只搬 vault 的 10 条字段 + 2 个 section + 1 个 page，core 一行不动。

C 的典型形态：

```ts
withFeature("vault", (r) => {
  r.setting({ key: "vaultAutoLockMinutes", ... });
});
```

它有归属、能批量跳过，但 `withFeature` 仍是运行时函数调用，顶层副作用依旧存在，Rollup 照样删不掉，且注册表依然是可变单例。**只解决了 R1 的一半，没解决 R2 / R3**。

因此选 **D**。

### 6.3 方案 D：内置注册 + 扩展注册

保留现有 `defineSetting` / `defineSection` 作为**内置通道**（core 的 33 条字段继续用，零改动），新增**扩展通道**供可排除的功能模块使用。

| 通道 | 使用者 | 注册方式 | 容器 | 可排除 |
|---|---|---|---|---|
| 内置 | core（general / appearance / shortcuts / about / storage） | `defineSetting()` push 到模块私有数组 | 模块私有 `definitions` | 否，恒存在 |
| 扩展 | vault（未来其他可选功能） | 声明 manifest 字面量，加入扩展清单 | `EXTENSIONS` 数组 | **是，清单项可条件展开** |

#### 6.3.1 扩展清单

```ts
// src/settings/core/extensions.ts —— 唯一需要按 feature 改动的地方
import type { SettingsExtension } from "./types/extension";
import { vaultExtension } from "@/features/vault/settingsExtension";
import { VAULT_ENABLED } from "@/features/vault";

export const EXTENSIONS: readonly SettingsExtension[] = [
  ...(VAULT_ENABLED ? [vaultExtension] : []),
];
```

「取消注册 vault 的全部设置」= 这一个三元。新增可选功能 = 往这个数组加一项，无需改任何消费点。

#### 6.3.2 扩展类型：只有数据，没有构造

```ts
// src/settings/core/types/extension.ts —— 只有类型，无容器、无运行时代码
import type { FC } from "react";
import type { FieldRenderProps } from "./fieldSchema";
import type { SettingsPageDefinition } from "./pageTypes";
import type {
  SettingDefinition,
  SettingSectionDef,
} from "../field/fieldRegistry";

export type ExtensionSettingDefinition = Omit<
  SettingDefinition,
  "customRender"
> & {
  customRenderLoad?: () => Promise<{ default: FC<FieldRenderProps> }>;
};

export interface SettingsExtension {
  id: string;
  pages?: readonly SettingsPageDefinition[];
  sections?: readonly SettingSectionDef[];
  fields?: readonly ExtensionSettingDefinition[];
}
```

三条约束：

1. 扩展文件只导出**数据字面量**，模块顶层**不得出现任何函数调用**（`definePage`、`lazyWithRecovery`、`defineSetting` 全部禁止）。组件构造推迟到聚合层（6.3.3），因此**不需要 `/*#__PURE__*/` 注解**。
2. 需要懒加载组件的地方一律写成 `customRenderLoad: () => import(...)` 这种**纯箭头函数值**，字段本身仍是字面量。
3. 本文件**不得 import 运行时值**（`pageRegistry.ts` / `fieldRegistry.ts` / `extensions.ts`），只能 `import type`（编译期擦除，不构成环）。否则形成 `pageRegistry → extensions → settingsExtension → pageRegistry` 的环。因此 `SettingsPageDefinition` 必须先下沉（见 6.3.4）。

不设 `renderers` 通道：vault 用的是**字段级** `customRender`（`BiometricSwitchRender` 挂在字段上），不是 `registerSettingType` 的类型渲染器。等真有第二个可选功能需要类型渲染器时再加，避免提前设计出无消费者的 API。

#### 6.3.3 查询合并：内置优先，扩展追加

**必须改的是 6 个函数，不是 4 个。** 漏改 `getDefinitionsBySection` / `getSectionById` 会在**默认构建**（日常构建）下回归，而不只是排除构建：

| 函数 | 消费方 | 漏改后果 |
|---|---|---|
| `getAllDefinitions` | 命令面板 | 命令面板仍出 vault 字段命令 |
| `getAllSections` | 命令面板 | 仍出 vault section 命令 |
| `getDefinitionsByGroup` | `SettingsGroup`、`OrbBallSection` | 设置页表单为空 |
| `getSectionsByPage` | `SettingsOverview` | 总览仍显示 vault 卡片 |
| `getDefinitionsBySection` | `SettingsOverview:118/151` | **默认构建**下 vault 总览卡片变空卡，且不再触发 `loadVaultConf` |
| `getSectionById` | `SettingsSection:19` | **默认构建**下 vault 设置页两个分区标题丢失，只剩表单 |

```ts
// fieldRegistry.ts —— 六个查询函数全部改为基于两个聚合器
import { lazyWithRecovery } from "@/utils/lazyWithRecovery";

function toSettingDefinition(def: ExtensionSettingDefinition): SettingDefinition {
  const { customRenderLoad, ...rest } = def;
  if (!customRenderLoad) return rest;
  return { ...rest, customRender: lazyWithRecovery(customRenderLoad) };
}

const extensionFieldCache = new Map<string, SettingDefinition[]>();

function extensionFields(): SettingDefinition[] {
  return EXTENSIONS.flatMap((ext) => {
    let cached = extensionFieldCache.get(ext.id);
    if (!cached) {
      cached = (ext.fields ?? []).map(toSettingDefinition);
      extensionFieldCache.set(ext.id, cached);
    }
    return cached;
  });
}

export function getAllDefinitions(): readonly SettingDefinition[] {
  return [...definitions, ...extensionFields()];
}

export function getAllSections(): readonly SettingSectionDef[] {
  return [...sections, ...EXTENSIONS.flatMap((e) => e.sections ?? [])];
}

export function getDefinitionsByGroup(group: ConfGroup): SettingDefinition[] {
  return getAllDefinitions().filter((d) => d.group === group);
}

export function getDefinitionsBySection(sectionId: string): SettingDefinition[] {
  return getAllDefinitions().filter((d) => d.section === sectionId);
}

export function getSectionsByPage(pageId: string): SettingSectionDef[] {
  return getAllSections().filter((s) => s.pageId === pageId);
}

export function getSectionById(id: string): SettingSectionDef | undefined {
  return getAllSections().find((s) => s.id === id);
}
```

六个函数**不含任何 feature 判断**，vault 是否在清单里由 `EXTENSIONS` 决定。对比 5.3 的 F2 补丁式过滤，这里过滤是结构性的。

`extensionFieldCache` 只缓存**扩展字段的构造结果**，不缓存 `getAllDefinitions()` 的结果，因此不会吃掉 mascot 的运行时注册（M8）。这个缓存是必要的：`customRender` 由 `lazyWithRecovery` 生成，若每次查询都重新构造，`React.lazy` 的组件标识每帧变化，会导致 `BiometricSwitchRender` 反复重挂载、生物识别状态丢失。扩展清单是编译期常量，缓存键 `ext.id` 天然稳定，安全。

`pageRegistry.ts` 的页面聚合同理（`SettingsPageDefinition` 的 `definePage` 下沉见 6.3.4）：

```ts
const EXTENSION_PAGES: readonly SettingsPageManifest[] = EXTENSIONS.flatMap(
  (e) => e.pages ?? [],
).map((page) => definePage(page));

export const SETTINGS_PAGES: readonly SettingsPageManifest[] = [
  ...CORE_PAGES,
  ...EXTENSION_PAGES,
].sort((a, b) => a.order - b.order);
```

扩展只写 `{ id, categoryId, label, keywords, icon, order, load }` 纯字面量，`definePage`（内部调 `lazyWithRecovery`）**只在聚合层调用一次**。内置页同样走 `definePage`，两条通道最终产出同一种 `SettingsPageManifest`。

**不要加惰性缓存。** `mascot/core/registry.ts:54` 在 `registerRenderer()` 时**运行时**调用 `defineSetting()` 注册渲染器设置项；一旦 `getAllDefinitions()` 的缓存先被填充，后注册的 mascot 设置项将永久查不到。原实现的每次 `filter` 重建虽多一次分配，但与 `definitions` 的增长同步，语义正确。若确有高频调用，应在 `defineSetting` 内把缓存置空，而那等于让 `fieldRegistry` 重新引入可变状态，与方案 D 的方向相反——不做。

#### 6.3.4 `definePage` 与 `SettingsPageDefinition` 下沉（必做，非可选）

当前 `definePage` 与 `SettingsPageDefinition` 定义在 `pageRegistry.ts:20-29`。若扩展模块直接 import 它，会形成：

```
pageRegistry → extensions → settingsExtension → pageRegistry
```

`SETTINGS_PAGES` 是模块顶层 `const`，在环中求值顺序不确定，有 TDZ 风险（`Cannot access before initialization`）。因此这一步是**必做项**，不是 6.7 M4 里写的"或"：

1. 把 `SettingsPageDefinition` 移入 `types/pageTypes.ts`
2. 把 `definePage` 移入 `core/definePage.ts`，只依赖 `lazyWithRecovery` 与 `types/pageTypes`
3. `pageRegistry.ts` 从 `definePage.ts` 导入 `definePage`，扩展模块同样从 `definePage.ts` 导入

依赖方向变为单向：`types → definePage → 扩展 → extensions → pageRegistry`，无环。

#### 6.3.5 与 `ConfGroup` 的关系（不可动的地方）

`ConfGroup` 是类型源，定义在 `fieldRegistry.ts:32`：


```ts
export const CONF_KEY_MAP = { app: "appConf", orb: "orbConf", mascot: "mascotConf", vault: "vaultConf" } as const;
export type ConfGroup = keyof typeof CONF_KEY_MAP;
```

它**不能跟着扩展走**，原因与 5.3 的结论一致：

- 删掉 `vault` 键 → `ConfGroup` 从 4 元联合塌成 3 元 → `ConfKey` / `ConfLoadKey` / 所有 `Record<ConfGroup, …>` 连锁报错
- `settingsStore` 的 `vaultConf` 槽位与 `loadVaultConf` 仍需存在（后端 `get_vault_conf` 命令保留，见 4.6）
- `commandAggregation.tsx:61` 的 `GROUP_TO_PAGE: Record<ConfGroup, string>` 依赖完整 4 元

结论：扩展只接管「数据与注册」，**类型层维持现状**。扩展可声明 `confGroups?: ConfGroup[]` 用于文档与校验，但类型不由此推导。

### 6.4 为什么方案 D 能真正做编译期删除

这是选 D 而不选 A 的关键论证。

| 环节 | 方案 A（现状） | 方案 D（扩展） |
|---|---|---|
| vault 字段的写法 | `defineSetting({...})` 顶层调用 | `fields: [{...}]` 纯字面量 |
| Rollup 判定 | 函数调用，可能有副作用 → **保留** | 字面量，无副作用 → **可删** |
| `VAULT_ENABLED` 折叠后 | `definitions` 数组里仍有 10 条 vault 对象 | `vaultExtension` 的 import 成为死引用 |
| 最终产物 | vault 字段对象 + `BiometricSwitchRender` 的 `import()` 都在包里 | 整个扩展模块连同 `lazyWithRecovery` 与 `import()` 一起消失 |

扩展模块里**不允许出现任何函数调用表达式**，因此完全不依赖 `/*#__PURE__*/`：

| 位置 | 带注解的写法（已废弃） | 扩展通道的写法 |
|---|---|---|
| 字段级自定义渲染器 | `customRender: /*#__PURE__*/ lazyWithRecovery(() => import(...))` | `customRenderLoad: () => import("../BiometricSwitchRender")` |
| 页面 | `pages: [/*#__PURE__*/ definePage({ ... })]` | `pages: [{ id, categoryId, label, keywords, icon, order, load: () => import(...) }]` |
| section / 普通字段 | 字面量 | 字面量（不变） |

`/*#__PURE__*/` 的四个问题，正是本轮设计要消掉的：

1. **脆弱**：它是对打包器的提示，不是语义保证。不同 Rollup/Vite/esbuild 版本、不同 minifier 配置下行为不一致，注解写错位置（未紧贴调用、`const x = f(); export { x }`）就静默失效。
2. **易漏**：每个新增的构造调用都要记得加，靠 code review 兜底，没有编译期保障。
3. **可读性差**：业务代码里散落打包器指令，读者要先懂 DCE 才知道它为什么存在。
4. **治标**：注解只是在说「这个调用没副作用」，本质问题仍是**扩展文件里存在构造逻辑**。

新设计的做法是把构造**整体推迟到聚合层**：扩展文件退化为纯字面量，`lazyWithRecovery` / `definePage` 只在 `fieldRegistry` / `pageRegistry` 里被调用一次。于是 DCE 判定退回到 Rollup 最基础的规则——**未被引用的字面量对象直接丢弃**，不需要任何注解：

```ts
// src/features/vault/settingsExtension.ts —— 顶层零调用
export const vaultExtension: SettingsExtension = {
  id: "vault",
  fields: [
    {
      key: "vaultBiometricUnlock",
      // ...
      customRenderLoad: () => import("@/settings/shared/renderers/BiometricSwitchRender"),
    },
  ],
  pages: [
    {
      id: "vault",
      categoryId: "vault",
      order: 65,
      load: () => import("@/settings/pages/VaultSettingsPage"),
    },
  ],
};
```

`VAULT_ENABLED` 折叠为 `false` 时，清单不再引用 `vaultModule`，该模块的顶层数据被 tree-shaking 丢弃。**但这只保证「代码删除」，不保证「chunk 不产出」**：静态 import 在折叠后会退化为副作用 import，`vaultModule` 里的 `import()` 箭头函数仍被保留，打包器照常物化异步 chunk。要做到产物零残留，清单必须用**动态 import + 编译期守卫**（见 6.8.2 实测对照）。这也正是 A 方案做不到的：A 里 `defineSetting({...})` 是顶层调用且结果被 push 进全局数组，属于「被使用」，连代码都删不掉。

代码规范相应加一条：**扩展文件只导出数据字面量，模块顶层禁止任何函数调用（含 `definePage` / `lazyWithRecovery` / `defineSetting`）；组件构造一律在聚合层完成，不得使用 `/*#__PURE__*/` 注解**。

### 6.5 迁移路径（只搬 vault）

方案 D 的迁移量是 7 项，core 的 33 条内置设置一行不动：

| 步 | 内容 | 改动量 | 验证 |
|---|---|---|---|
| S0 | 下沉 `SettingsPageDefinition` → `types/pageTypes.ts`，`definePage` → `core/definePage.ts`（解除 6.3.4 的环） | ~10 行移动 | `typecheck` 通过 |
| S1 | 新增 `src/features/vault.ts`，导出 `VAULT_ENABLED` | 1 行 | `typecheck` 通过 |
| S2 | 新增 `src/settings/core/types/extension.ts`（只有类型：`SettingsExtension` + `ExtensionSettingDefinition`） | ~30 行 | — |
| S3 | 新增 `src/features/vault/settingsExtension.ts`：搬 `VAULT_GENERAL_SETTINGS`(6 条) + `VAULT_SSH_SETTINGS`(4 条) + 2 个 section + 1 个 page，`customRender` 改为 `customRenderLoad` | ~90 行搬迁 | 默认构建 UI 无变化 |
| S4 | 新增 `src/settings/core/extensions.ts`，`EXTENSIONS` 数组 + vault 条件展开 | ~8 行 | — |
| S5 | 改 `fieldRegistry.ts` 的 **6 个**查询函数 + `toSettingDefinition` 物化 + 扩展字段缓存（6.3.3）+ `pageRegistry.ts` 页面合并与排序 | ~40 行 | 默认构建 UI 无变化 |
| S6 | 删 `fieldDefinitions.ts` 的 vault 段（2 个 section + 10 条字段 + `BiometricSwitchRender` lazy 常量） | -87 行 | 排除构建产物无 vault 探针 |

S0–S6 是方案 D 的最小落地，完成后即可通过 1.3 的全部验收。若继续做 6.8 的 P1，追加四步：

| 步 | 内容 | 改动量 | 验证 |
|---|---|---|---|
| S7 | 新增 `src/features/types.ts`（`FeatureModule` / `FeatureRoute` / `FeatureSidebarEntry`）与 `aggregate.ts`（物化、排序、唯一性断言、缓存） | ~120 行新增 | `typecheck` 通过 |
| S8 | 新增 `src/features/registry.ts`（`__VAULT_ENABLED__` 守卫 + `await import()` 动态装载，唯一读取编译期常量的清单点，对外只导出 `getFeatures()` / `loadFeatures()`），把 `settingsExtension.ts` 的 `settings` 段并入 `features/vault/module.ts`，补 `routes` 与 `sidebar`；`main.tsx` 的 `bootstrap()` 首行 `await loadFeatures()` | ~40 行搬迁 | 默认构建 UI 无变化 |
| S9 | 改 `MainLayout.tsx`（删 `VaultApp` 常量与双条件，路由改 `getFeatures().routes.map`）、`AppSidebar.tsx`（删 `vaultMenuItems`，条目改派生 + 4 处 memo）、`pageRegistry.ts`（页面来源改为 `[...内置, ...getFeatures().pages]`） | ~60 行 | 侧栏顺序、图标、选中态与改造前一致 |
| S10 | `pageRegistry.ts` 的 `CATEGORY_LABELS` 改 i18n key；`pageTypes.ts` 的 `SettingsPageId` / `SettingsCategoryId` 放宽为 `string & {}`（6.9） | ~20 行 | `typecheck` / `lint` 通过；排除构建无 `"安全"` 残留 |

净代码量：**新增 ~145 行、删除 ~92 行**（S0–S6），再加 **新增 ~180 行、删除 ~60 行**（S7–S10），远低于方案 B 的 450 行搬迁。

页面顺序（M5 的处置）：给 `definePage` 增加 `order: number` 字段，内置页写 `0 / 10 / … / 70`，vault 扩展写 `order: 65`（介于 `storage`(60) 与 `about`(70) 之间，与改造前 `[…, storage, vault, about]` 一致），聚合后按 `order` 稳定排序。这样 UI 顺序与改造前逐项一致，满足 L2 验收的「默认构建 UI 与改造前一致」。

`VaultSettingsPage` 不再直接 import 扩展的原始数组（扩展字段是 `ExtensionSettingDefinition`，未经物化不能交给 `SettingsGroup`），改为按 section 走查询：

```ts
// src/settings/pages/VaultSettingsPage.tsx
import { getDefinitionsBySection } from "../core/field/fieldRegistry";

<SettingsGroup group="vault" definitions={getDefinitionsBySection("vault-general")} ... />
<SettingsGroup group="vault" definitions={getDefinitionsBySection("vault-ssh")} ... />
```

这样页面本身也**不依赖 vault 扩展模块**，它只是「消费注册表」的普通页面，扩展在不在清单里由 `EXTENSIONS` 决定。

### 6.6 迁移后的净收益

| 项 | 方案 A（5.3） | 方案 D（S0–S6） | 方案 D + P1（S7–S10） |
|---|---|---|---|
| vault 排除改动点 | 7 处 | 1 处（`extensions.ts` 的三元） | **1 处**（`features/registry.ts` 的编译期守卫） |
| `fieldRegistry` 查询函数 | 4 个函数各加 feature 判断 | 0 判断 | 0 判断 |
| core 33 条内置字段 | 不动 | **不动** | 不动 |
| `BiometricSwitchRender` 残留 chunk | 存在（5.4） | 残留（静态清单仍拉入子图，见 6.8.2 实测） | **消失**（动态装载，不进入 chunk 图） |
| 路由 / 侧栏是否感知特性 | 是（各打条件） | 是（`VAULT_ENABLED` 仍在 2 处） | **否**（派生自 `getFeatures()`） |
| 新增可选功能的改动点 | 4 个查询函数 + N 个消费点 | 加一个扩展项 | **加一个 `module.ts`** |
| 移除可选功能 | 需扫全仓找消费点 | 删清单一项 | 删清单一项 |
| 侧栏每次 render 的重复分配 | 存在 | 存在 | 消除（S1–S4） |
| 迁移风险面 | 分散在 7 个接入点 | 集中在 1 个新增目录 | 集中 + 1 个聚合器单点 |

方案 D 与 A 的本质区别：A 是「注册后过滤」，D 是「不注册」。前者要求每个查询点都记得加判断，后者由 `EXTENSIONS` 的组成决定结果——**漏改一处不会静默泄漏，因为不存在需要改的第二处**。

P1 相对 D 的增量价值在**扩展性**而非可删除性：D 之后「新增一个可选功能」仍需在 `MainLayout` 与 `AppSidebar` 各写一个条件；P1 之后只需写一份 `module.ts`，两个消费点零改动。

### 6.7 重构引入的新风险

| # | 风险 | 说明 | 处理 |
|---|---|---|---|
| M1 | 去重保护丢失 | `defineSetting` 现有 `definitionKeys` 去重（`fieldRegistry.ts:79-85`），扩展字面量绕过了它 | `getAllDefinitions` 合并后做一次 `import.meta.env.DEV` 断言：字段 key、section id 全局唯一 |
| M2 | 扩展与内置同名冲突 | 若扩展字段 key 与 core 重复，后者会覆盖前者（`SettingsGroup` 按 key 渲染） | 冲突检测在 M1 的断言里一起做，命中即抛错 |
| M3 | 循环依赖 | 扩展 import 类型；`extensions.ts` import 扩展；`fieldRegistry` import `extensions.ts` | 保持单向：扩展不 import `fieldRegistry` / `extensions.ts`；类型放独立 `types/extension.ts` |
| M4 | `definePage` 的归属 | `definePage` 在 `pageRegistry.ts`，扩展 import 它会造成 扩展 → pageRegistry 的环 | **必做**：下沉到 `core/definePage.ts`，见 6.3.4 / S0 |
| M5 | 排序变化 | `[...内置, ...扩展]` 会让 vault 页面排到 `about` 之后，改变 UI 顺序 | 给 `definePage` 加 `order`，内置 `0..70`、vault `65`，聚合后稳定排序，见 6.5 |
| M6 | 扩展文件混入顶层调用 | 有人在扩展里直接写 `customRender: lazyWithRecovery(...)` 或 `pages: [definePage({...})]`，DCE 立刻失效，且无编译期报错 | 类型层不给 `customRender` 通道、只给 `customRenderLoad`；`pages` 类型是 `SettingsPageDefinition`（无 `component` 字段），写 `definePage` 会类型不匹配；CI 用**纯字面量 key** 探针（`vaultAutoLockMinutes`、`vaultSshAutoAdd`）检查产物 |
| M7 | 前后端开关不同源 | 后端 cargo feature `vault`、前端 env `VAULT_ENABLED` 独立配置。若前端开、后端关：`get_vault_conf` 不存在，`loadVaultConf` 自身 catch 掉错误只打日志（`settingsStore.ts:72-78`），表现为静默失败 | 两个开关必须由同一构建入口同时导出；CI 只跑「全开」与「全关」两种组合，不跑混合组合 |
| M8 | 缓存吃掉动态注册 | 若给 `getAllDefinitions` 加惰性缓存，`mascot/core/registry.ts:54` 运行时注册的设置项将永久查不到 | 只缓存**扩展字段的物化结果**（`ext.id` 为键，扩展是编译期常量），不缓存 `getAllDefinitions()`，见 6.3.3 |
| M9 | 特性声明分散 | 一个 vault 被拆成 4 处互不相识的声明（开关、设置扩展、路由、菜单），每加一个可选功能就要在 3 处补条件 | 引入 `FeatureModule` 单一声明 + `aggregate()` 派生，见 6.8 |
| M10 | 路由 nullable 类型补丁 | `VaultApp` 是 `RecoverableLazyComponent \| null`，JSX 不接受 `null`，被迫写 `VAULT_ENABLED && VaultApp && (...)` 双条件 | 聚合器产出 `ComponentType`（非 nullable），双条件消失，见 6.8.4 |
| M11 | 类型联合写死特性 id | `SettingsPageId` / `SettingsCategoryId` 含 `"vault"` 字面量，新增扩展页必须回头改 core 类型 | 放宽为 `string`，合法性交由聚合器 DEV 断言，见 6.9.2 |

### 6.8 终局设计：FeatureModule（特性模块）

> 6.3–6.7 的方案 D 解决了「能不能删」，但没有解决「一个特性被拆成几处」。本节把抽象层级从「设置扩展」抬到「特性模块」。

#### 6.8.1 方案 D 落地后的残留别扭点

| # | 现象 | 代码证据 | 本质 |
|---|---|---|---|
| N1 | 一个特性分散 4 处 | `features/vault.ts`（开关）、`features/vault/settingsExtension.ts`（设置）、`MainLayout.tsx`（路由）、`AppSidebar.tsx`（菜单） | 特性未被建模成整体，而是「一个开关 + 三处补丁」 |
| N2 | 同一概念两套形状 | 内置 `customRender: FC`；扩展 `customRenderLoad: () => Promise` | 扩展类型由 `Omit<SettingDefinition, "customRender">` **做减法**派生，不是独立契约 |
| N3 | 同一组件两种取数 | `GeneralSettings` 直传 `APP_SETTINGS`；`VaultSettingsPage` 走 `getDefinitionsBySection` | 消费者必须知道「自己是内置还是扩展」 |
| N4 | core 里出现 feature 的物化逻辑 | `toSettingDefinition` + `extensionFieldCache` 写在 `fieldRegistry.ts` | 扩展通道的实现细节泄漏进 core |
| N5 | 路由 nullable 补丁 | `VaultApp = VAULT_ENABLED ? lazyWithRecovery(...) : null`，再 `VAULT_ENABLED && VaultApp &&` | 开关 + 类型收窄双机制叠加，`null` 是给类型系统打的补丁 |
| N6 | 仓库内两套范式 | 工具走 `defineTool` 声明 → `useToolMenuItems` 派生；路由/侧栏是手写数组 | 同一仓库两种心智模型 |
| N7 | 顺序靠魔法数字 | `order: 0/10/…/70`，vault `65` | 新增页面要手工挑空位 |

核心判断：`EXTENSIONS` 容器本身没错，错的是它**只承载「设置」这一个维度**。路由和侧栏各自持有独立开关，于是每个可选功能都要在 3 处补条件——这才是扩展性差的根因。

#### 6.8.2 契约：一个特性 = 一个模块声明

目标目录：

```
src/features/
  registry.ts       唯一条件展开点（编译期常量守卫 + 动态 import 装载）
  aggregate.ts      唯一构造点（lazy 包装 / 排序 / 冲突断言 / 快照）
  vault/
    module.ts       纯数据：routes + sidebar + settings
src/settings/core/
  fieldRegistry.ts  只剩内置可变注册表 + 读快照
  pageRegistry.ts   只剩内置页 + 读快照
```

契约（**全部纯数据，顶层零函数调用**，延续 6.3 已确立的 DCE 规则）：

```ts
// src/features/types.ts
import type { ComponentType } from "react";
import type { SettingsPageDefinition } from "@/settings/core/types/pageTypes";
import type { ExtensionSettingDefinition } from "@/settings/core/types/extension";
import type { SettingSectionDef } from "@/settings/core/types/fieldTypes";

export interface FeatureRoute {
  path: string;
  load: () => Promise<{ default: ComponentType }>;
  boundary: string;
  order?: number;
}

export interface FeatureSidebarEntry {
  label: string;
  icon: ComponentType;
  slot: "after-tools" | "before-history" | "before-settings";
  key?: string;
  order?: number;
}

export interface FeatureModule {
  id: string;
  order?: number;
  routes?: readonly FeatureRoute[];
  sidebar?: readonly FeatureSidebarEntry[];
  settings?: {
    pages?: readonly SettingsPageDefinition[];
    sections?: readonly SettingSectionDef[];
    fields?: readonly ExtensionSettingDefinition[];
  };
}
```

`registry.ts` 是唯一读取编译期常量的地方，采用**运行时动态装载**：起步为空聚合，启动期由 `loadFeatures()` 一次性覆盖，对外只暴露访问函数（不导出 `FEATURES` 常量）：

```ts
// src/features/registry.ts
import { aggregate } from "./aggregate";
import type { AggregatedFeatures } from "./aggregate";

let features: AggregatedFeatures = aggregate([]);

export function getFeatures(): AggregatedFeatures {
  return features;
}

export async function loadFeatures(): Promise<void> {
  if (!__VAULT_ENABLED__) return;
  const { vaultModule } = await import("./vault/module");
  features = aggregate([vaultModule]);
}
```

`loadFeatures()` 由 bootstrap 首行 `await`，早于任何路由 / 侧栏 / 设置读取：

```ts
// src/main.tsx
async function bootstrap(): Promise<void> {
  await loadFeatures();
  const { useSettingsStore } = await import("@/stores/settingsStore");
  // ...
}
```

四点约束：

- **唯一条件展开点**：`__VAULT_ENABLED__` 只允许出现在本文件；`VAULT_ENABLED` 的其余消费点仅限启动期 `loadVaultConf()` 与事件回调，不参与路由 / 侧栏派生。
- **导出函数而非常量**：消费方一律 `getFeatures().routes` / `getFeatures().sidebar` 取值，注册入口收敛为一个，增删特性只改本文件，消费方零改动。
- **动态 import 而非静态 import**：关闭 vault 时 `if (!__VAULT_ENABLED__) return;` 折叠为 `if (true) return;`，其后 `await import("./vault/module")` 成为不可达代码被整段删除，`vault/module` 连同其子图根本不进入 chunk 图。
- **缓存以引用相等失效**：`getFeatures()` 返回的聚合结果在装载前后是两个不同对象，所有派生缓存（如 `pageRegistry` 的 `derive()`）以 `derived.features === features` 判断失效，装载后自动重算一次，无需显式清缓存。

**为什么必须是动态 import（实测对照）**

静态 import + 编译期三元折叠只能删掉「引用」，删不掉「模块产物」。`VAULT_ENABLED=0` 构建实测：

| 写法 | 产物文件数 | vault 相关 chunk | 入口是否含 vault 字段 |
|---|---|---|---|
| 静态 import + 三元（`aggregate(__VAULT_ENABLED__ ? [vaultModule] : [])`） | 172 | 7 个 | 是（`vaultAutoLockMinutes` 等字面量仍在） |
| 静态 import + 无条件展开（`aggregate([vaultModule])`） | 172 | 7 个 | 是 |
| 静态 import + 完全不引用（`aggregate([])`，import 语句残留） | 158 | 0 | 否 |
| **动态 import + 守卫早退（本方案）** | **158** | **0** | **否** |

对照第 1、2 行可见：编译期常量折叠后，`import { vaultModule } from "./vault/module"` 退化为副作用 import，`vault/module` 的顶层数组（含 `load: () => import("@/vault")` 等箭头函数）被保留，打包器据此照常物化 `module` / `vault` / `vaultStore` / `VaultSettingsPage` / `BiometricSwitchRender` / `useVaultStatusReady` 等异步 chunk。只有第 4 行的动态 import 能让「不可达的 `import()` 调用」本身被删除，从而不产生 chunk。

`VAULT_ENABLED=1` 实测：175 个文件、7 个 vault chunk、入口含完整特性数据，功能与开启态一致。

vault 的声明退化为一份数据：

```ts
// src/features/vault/module.ts
import { SafetyCertificateOutlined } from "@ant-design/icons";
import type { FeatureModule } from "../types";

export const vaultModule: FeatureModule = {
  id: "vault",
  order: 65,
  routes: [
    {
      path: "/vault",
      load: () => import("@/vault").then((m) => ({ default: m.VaultApp })),
      boundary: "密码保险库",
    },
  ],
  sidebar: [
    { label: "common.navigation.vault", icon: SafetyCertificateOutlined, slot: "after-tools" },
  ],
  settings: { /* 同 6.3.2 的 pages / sections / fields */ },
};
```

`sidebar` 的 `key` 缺省取 `routes[0].path`、`label` 缺省取 route 的同名字段；只有需要自定义时才显式写。这样「一个 `/vault` 路由」只声明一次。

#### 6.8.3 聚合器：唯一的构造点

```ts
// src/features/aggregate.ts
export interface AggregatedFeatures {
  routes: readonly PreparedRoute[];        // element 已 lazyWithRecovery
  sidebar: readonly PreparedSidebarEntry[]; // 已按 slot + order 排好
  pages: readonly SettingsPageManifest[];
  sections: readonly SettingSectionDef[];
  fields: readonly SettingDefinition[];
}

export function aggregate(modules: readonly FeatureModule[]): AggregatedFeatures;
```

`aggregate()` 集中承担今天散落三处的四件事：

| 职责 | 今天在哪 | 迁移后 |
|---|---|---|
| `lazyWithRecovery` 包装组件 | `MainLayout.tsx` 三元 + `toSettingDefinition` | 聚合器内一次完成 |
| 页面 / 路由排序 | `pageRegistry.ts` 排序 + 手工 `order` | 聚合器稳定排序，`order` 相同按 `id` 字典序 |
| 字段 key / section id 全局唯一断言 | **尚未实现**（M1/M2 只在文档） | 聚合器内 `import.meta.env.DEV` 断言，命中即抛错 |
| 扩展字段物化缓存 | `extensionFieldCache`（N4） | 聚合器内部细节，core 不再感知 |

聚合结果在模块加载时**构造一次**，之后全部 O(1) 读。

#### 6.8.4 路由：从条件分支变数据遍历

`MainLayout` 不再出现 `VAULT_ENABLED`，`VaultApp` 这个 nullable 常量整体删除：

```tsx
<Routes>
  <Route path="/" element={<ToolBox />} />
  <Route path="/tools/:id" element={<ToolPage />} />
  {getFeatures().routes.map((r) => (
    <Route
      key={r.path}
      path={r.path}
      element={
        <FeatureErrorBoundary featureName={r.boundary}>{r.element}</FeatureErrorBoundary>
      }
    />
  ))}
  <Route path="/history" element={...} />
  <Route path="/settings" element={<Navigate to={`/settings/${DEFAULT_SETTINGS_PAGE_ID}`} replace />} />
  <Route path="/settings/:pageId" element={...} />
</Routes>
```

`r.element` 类型是 `ComponentType` 而非 `... | null`，N5 的双条件与 M10 的类型补丁自然消失。**新增可选功能时 `MainLayout` 零改动。**

#### 6.8.5 侧栏：派生 + 渲染优化

条目派生：`items` 由「手写三段 + 一个条件数组」变为按 `slot` 拼接 `getFeatures().sidebar`，新增功能无需再决定插在哪。

同时修掉 4 处真实渲染浪费：

| # | 问题 | 位置 | 修法 |
|---|---|---|---|
| S1 | `items` 数组每次 render 重建，内部 `translateKey` 重复执行 | `AppSidebar.tsx` 的 `items` 内联字面量 | 整体 `useMemo`，依赖 `[toolMenuItems, t, navigate]`（`getFeatures()` 结果引用稳定，不进依赖） |
| S2 | `SETTINGS_DETAIL_PAGES.map` 每次 render 生成 N 个元素 | 同上，`settings-sub.children` | 并入 S1 的 memo |
| S3 | `selectedKeys` 每次 render 新建数组 | 同上，`selectedKeys={[...]}` | `useMemo` 按 `location.pathname` |
| S4 | `setOpenKeys` 无变化也返回新数组，触发重渲染 | `AppSidebar.tsx` 的 `useEffect` | 抽纯函数 `deriveOpenKeys(pathname)` + 相等短路（无新增键时 `return prev`） |

四项均为低风险局部改动，不依赖 6.8 其余部分，可独立先落地。

#### 6.8.6 分期

| 期 | 内容 | 消除 | 风险 |
|---|---|---|---|
| **P1** | 新增 `features/types.ts` + `aggregate.ts` + `registry.ts`；vault 改 `module.ts`；`MainLayout` / `AppSidebar` 改派生；侧栏 4 处 memo | N1、N5、N6、N7、S1–S4 | 低：新增文件 + 改 3 个消费点，core 查询函数不动 |
| **P2** | mascot 运行时注册改声明式 → 内置 33 条字段进快照，删 `defineSetting` 与 6 个查询函数 | N2、N3、N4 | 中：触及 mascot 渲染器体系 |

P2 的唯一难点是 `mascot/core/registry.ts` 在 `registerRenderer()` 时**运行时** `defineSetting()` 动态生成设置项——这是「必须每次重建、不能缓存」的唯一真实来源（M8）。它需独立改造为「渲染器清单在聚合期声明式展开」，不与本任务耦合。

#### 6.8.7 与方案 D 的关系

方案 D 是 P1 的**中间态**：先把 vault 变成扩展（能删），再把扩展抬成特性模块（好扩）。P1 完成后：

- `VAULT_ENABLED` 的消费点从 4 个降到 2 个（`features/registry.ts` 的编译期守卫、`settingEvents.ts` 的启动加载判断）
- 排除构建的改动面从「4 个文件」收缩到「1 个文件」（`features/registry.ts`）
- 验收口径 1.3 四条不变，且新增两条：**默认构建下 `MainLayout` / `AppSidebar` 源码不含任何 feature id 字面量**；**`VAULT_ENABLED=0` 构建产物中 vault 相关 chunk 数量为 0**（判定方式见 6.8.2 实测对照，`dist/assets` 中不得存在 `vault*` / `Vault*` / `module-*` 命名的 chunk 与 css）

### 6.9 两处泄漏（应随 P1 一并处理）

#### 6.9.1 `CATEGORY_LABELS` 硬编码中文分类名

`pageRegistry.ts` 的 `CATEGORY_LABELS` 是对象字面量，即便 `SETTINGS_CATEGORIES` 会过滤掉空分类，该字面量本身被引用，**不会被 DCE**，排除构建产物里仍残留 `"安全"` 字符串。虽非功能代码，但违反 1.3 的「无 vault 残留」口径。

处理：分类 label 改为 i18n key，且分类由 `FeatureModule` 声明（vault 分类随模块消失），core 只保留 `CATEGORY_ORDER` 的稳定排序职责。

#### 6.9.2 `SettingsPageId` / `SettingsCategoryId` 写死特性 id

`types/pageTypes.ts` 中 `SettingsCategoryId` 含 `"vault"`、`SettingsPageId` 含 `"vault"`。后果是**扩展新增一个设置页时必须回头改 core 类型**——扩展性被类型系统反向卡住（M11）。

处理：两者放宽为 `string & {}`（保留字面量补全提示但不锁死取值），合法性校验由 `isSettingsPageId()` 与聚合器的 DEV 断言承担。注意 `SETTINGS_PAGE_MAP` 的 `get()` 返回类型需同步放宽，避免 `SettingsPageId` 收窄后出现 `undefined` 分支。

---

## 7. 门禁测试影响

### 7.1 现状：两个扫描型测试不受影响

`src-tauri/crates/assistant-vault/tests/` 下两个测试都做**源码文本扫描**，不参与编译：

| 测试 | 机制 | 受 cfg 影响 |
|---|---|---|
| `command_registry.rs` | 扫 `src-tauri/src/cmd/` 找 `#[command]`，扫 `src-tauri/src/lib.rs` 的 `generate_handler!` 块比对两向一致 | 无：`#[cfg]` 只加行，不改变 `cmd::vault_*` 文本与 `pub fn` 定义 |
| `command_gate.rs` | 扫 `src-tauri/src/cmd/vault/` 的 `// precondition:` 声明与门控调用 | 无：文件内容不变 |

且两个测试都在 `assistant-vault` crate 内，feature 关闭时整个 crate 不参与构建，测试自然不运行。

### 7.2 需要新增的断言

在 `command_registry.rs` 增加一条：`src-tauri/src/lib.rs` 中每一条 `cmd::vault_*` 注册项前一行必须是 `#[cfg(feature = "vault")]`。否则后续新增命令漏加属性时，默认构建仍然正常，只有 `--no-default-features` 才炸，问题会滞后暴露。

### 7.3 需要新增的 CI 断言

```powershell
cargo tree --no-default-features -i assistant-vault
```

期望输出 `package ID specification 'assistant-vault' did not match any package`。这条断言把「feature 声明正确但依赖没真正断开」这类静默失败挡在构建期。

---

## 8. 实施分层

| 层 | 内容 | 涉及文件 | 验收 |
|---|---|---|---|
| **L1** | Cargo feature + 后端门控（4.2 / 4.3 / 4.5） | `src-tauri/Cargo.toml`、`src-tauri/src/cmd/mod.rs`、`src-tauri/src/cmd/vault/mod.rs`、`src-tauri/src/cmd/config.rs`、`src-tauri/src/setup.rs`、`src-tauri/src/lib.rs` | `cargo check --no-default-features` 零错误零警告；`cargo check` 与 `cargo test -p assistant-vault` 全绿 |
| **L2** | 前端门控：设置注册架构改「内置 + 扩展」双通道（6.3–6.7）+ 其余接入点（5.3 的 F0/F3~F6） | `vite.config.ts`、`src/vite-env.d.ts`、新增 `src/features/vault.ts`、新增 `src/settings/core/types/extension.ts`、新增 `src/settings/core/extensions.ts`、新增 `src/features/vault/settingsExtension.ts`、`fieldRegistry.ts`、`pageRegistry.ts`、`fieldDefinitions.ts`（删 vault 段）、`VaultSettingsPage.tsx`、`MainLayout.tsx`、`AppSidebar.tsx`、`main.tsx`、`settingEvents.ts` | `VAULT_ENABLED=0 npm run build` 成功，产物 JS 无 `vaultAutoLockMinutes` / `vaultSshAutoAdd` 等 vault 字段字面量；`typecheck` / `lint` 通过；默认构建 UI 与改造前一致。**注意：本层用静态清单，关闭态仍会残留 vault 异步 chunk，chunk 归零由 L2.5 达成** |
| **L2.5** | 特性模块化 P1（6.8）：`FeatureModule` 契约 + `aggregate()` 派生 + 侧栏渲染优化 | 新增 `src/features/types.ts`、`src/features/aggregate.ts`、`src/features/registry.ts`、`src/features/vault/module.ts`；改 `MainLayout.tsx`、`AppSidebar.tsx`、`pageRegistry.ts`、`main.tsx` | 默认构建 UI 逐项一致；`MainLayout` / `AppSidebar` 源码零 feature id；**`VAULT_ENABLED=0` 构建产物 vault 相关 chunk 数为 0**（实测 158 文件 / 0 chunk，见 6.8.2） |
| **L3** | 门禁补强（7.2 / 7.3） | `src-tauri/crates/assistant-vault/tests/command_registry.rs` + CI | 漏加 cfg 属性时测试变红；依赖未断时 cargo tree 断言变红 |
| **L4** | `windows` feature 下放 | `src-tauri/Cargo.toml`、各 member 的 `Cargo.toml` | 排除构建下 `windows` 实际启用 feature 从 18 项降到主功能所需 2 项 |
| **L5** | `assistant-crypto` 内部分层 | `src-tauri/crates/assistant-crypto/Cargo.toml` + `vault_crypto` 模块 | 主功能构建不再编译 `vault_crypto` 及其专属依赖 |

L1 + L2 完成后功能已可用；L4 / L5 是收益增强，可独立排期。

### 8.1 L4 的真实机制（易误解点）

Cargo 的 feature 是**按包求并集**。把 18 项写在 `[workspace.dependencies]` 里，只要 `assistant-tools` 仍用 `workspace = true` 引用 `windows`，18 项就全生效——排除 vault 不会减少任何一个 feature。

因此 L4 的正确做法是：

1. `[workspace.dependencies] windows` 只保留主功能真正需要的 2 项
2. `assistant-vault` / `assistant-security` 在自己的 `Cargo.toml` 里**额外**声明所需 feature
3. 排除 vault/security 后，并集自然回落到 2 项

这一步与「功能下沉判据」一致：feature 声明应与实际消费者对齐，而不是挂在 workspace 上兜底。

---

## 9. 陷阱清单

| # | 陷阱 | 后果 | 处理 |
|---|---|---|---|
| T1 | `cmd::setup` / `cmd::cleanup_on_exit` 经通配重导出实际指向 vault 内部函数 | 排除构建在 `setup.rs:90`、`lib.rs:267` 两处编译失败 | 改名 `vault_setup` / `vault_cleanup_on_exit`，见 4.5 |
| T2 | 把 `#[cfg]` 加在 `generate_handler!` 外层 | 整段 handler 被裁剪，主功能命令全丢 | 属性只能写在**条目上**，见 4.4 |
| T3 | 函数定义上 `#[cfg]` 写在 `#[tauri::command]` 之后 | proc macro 会连同 cfg 一起展开，生成的 wrapper 宏不受控 | `#[cfg]` 必须在 `#[tauri::command]` **之上** |
| T4 | `apply_vault_side_effects` 只门控函数体 | `prev`/`conf` 变未使用参数，触发 `unused_variables` | 用双函数 + `_prev`/`_conf`，见 4.3 |
| T5 | 误删 `get_vault_conf` 命令 | `src/main.tsx:44` 启动期 invoke 失败，设置系统加载中断 | 保留该命令，见 4.6 |
| T6 | 用 `import.meta.env.VAULT_ENABLED` | 变量不被注入，恒为 `undefined`，开关永远等于「开启」 | 用 `define`，见 5.1 |
| T7 | 为省事删除 `src/vault/` | 类型检查覆盖下降、`contract:gen` 产物与源码树失配、回切成本高 | 保留目录，只断引用，见 5.3 |
| T8 | 新增 vault 命令时漏加 `#[cfg]` | 默认构建正常，只在排除构建爆炸，问题滞后 | L3 的门禁断言兜住，见 7.2 |
| T9 | `npm run contract:gen` 在排除构建下执行 | 脚本固定 `cargo test -p assistant-vault`，crate 不在图中会报错 | 不改脚本；该脚本只在默认 feature 下使用 |
| T10 | 前端用 `if (!VAULT_ENABLED) return;` 包 `loadVaultConf()` | 退出整个 `bootstrap()`，连带跳过 `applyLocaleChange`，语言不生效 | 用 `if (VAULT_ENABLED)` 正条件包裹单条语句，见 5.3 F5 |
| T11 | 只改 4 个查询函数 | `getDefinitionsBySection` / `getSectionById` 仍读内置数组，**默认构建**下 vault 总览变空卡、设置页丢分区标题 | 六个函数全改，见 6.3.3 |
| T12 | 扩展模块 import `pageRegistry` 的 `definePage` | 形成 `pageRegistry → extensions → 扩展 → pageRegistry` 环，顶层 `const` 触发 TDZ | 下沉 `definePage` 到 `core/definePage.ts`，见 6.3.4 |
| T13 | 路由 / 侧栏各打一个 `VAULT_ENABLED` 条件 | 每加一个可选功能要在 3 处补条件，漏一处即静默泄漏（M9） | 统一由 `FeatureModule` + `aggregate()` 派生，见 6.8 |
| T14 | 聚合器产出 nullable 组件（`... \| null`） | JSX 不接受 `null`，被迫写 `VAULT_ENABLED && VaultApp && (...)` 双条件（M10） | 聚合器只产出已物化的 `ComponentType`，`null` 分支不存在 |
| T15 | 侧栏 `items` / `selectedKeys` 内联字面量 | 每次 render 重建数组并重复执行 `translateKey`，菜单越大越明显 | `useMemo` + `setOpenKeys` 相等短路，见 6.8.5 |
| T16 | 给 `getAllDefinitions()` 加惰性缓存 | `mascot/core/registry.ts` 运行时注册的设置项永久查不到（M8） | 只缓存扩展字段物化结果，见 6.3.3 |

---

## 10. 验证命令

```powershell
# 后端：排除构建
cd src-tauri
cargo check --no-default-features

# 依赖确实断开（期望 "did not match any package"）
cargo tree --no-default-features -i assistant-vault

# 默认构建不受影响
cargo check
cargo test -p assistant-vault

# 耗时对比（同一机器、清缓存后各测三次取中位数）
cargo build --timings
```

```powershell
# 前端：排除构建（PowerShell 语法，勿写 bash 的 VAULT_ENABLED=0 npm run build）
$env:VAULT_ENABLED = "0"
npm run build
npm run typecheck
npm run lint

# 产物探针：用纯字面量 key 而非组件名（组件名可能残留在 sourcemap/注释里）
Get-ChildItem -Recurse dist -Include *.js | Select-String -Pattern "vaultAutoLockMinutes|vaultSshAutoAdd"

# chunk 探针：关闭态必须为 0（文件名匹配，静态 import 方案在此会失败）
(Get-ChildItem dist/assets -File | Where-Object { $_.Name -match "vault|Vault|Biometric|module-" }).Count

# 默认构建
$env:VAULT_ENABLED = "1"
npm run build
```

---

## 11. 收益与不确定性

### 11.1 确定收益

| 维度 | 收益 |
|---|---|
| 可省包数 | 48 个（vault 闭包 126 包中，其余 78 包被主功能共享） |
| 串行耗时 | 78.2s，占全量串行 7.2% |
| wall 时间 | 5~15s（并行度受限，收益区间见 `compile-time-attribution.md`） |
| 依赖面 | 移除 `keyring`、`zxcvbn`、`ssh-key`、`rsa`、`arboard`、`ts-rs`、`git2` 等专属依赖 |
| 供应链/审计面 | 需要审计的第三方 crate 显著减少 |

### 11.2 上限说明

排除 vault **不会**让主功能少编 `rusqlite` / `libsqlite3-sys`（10.3s），这部分是主功能 DB 的固定成本。因此 vault 拆分的价值不应只用「省几秒」衡量，更主要的是依赖面收敛。

### 11.3 未实测项

| 项 | 状态 |
|---|---|
| L4 `windows` 收敛的实际收益 | 未实测，需在 L4 落地时用 `cargo build --timings` 复核 |
| L5 `vault_crypto` 分层的收益 | `compile-time-attribution.md` 记录约 10s 量级，需按该文档口径复测 |
| 排除构建下前端产物体积变化 | 未实测，预期为删除一个懒加载 chunk |
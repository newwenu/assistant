# 可选特性挂载遍历化规格

| 项 | 值 |
|---|---|
| 状态 | 规格（定义终态与判定，不含实施记录） |
| 日期 | 2026-11-11 |
| 关联 | 分析《FEATURE-ZERO-COST-MOUNT-ANALYSIS.md》；前置分析《FEATURE-OPTIONAL-BINARY-ANALYSIS.md》；已实施 L1《vault-optional-build.md》；拆分判据 `docs/principle/CODE-SPLIT-PRINCIPLE.md` |
| 影响面 | 后端 12 文件（含 2 新建）、前端 1 文件、capabilities 2 文件（含 1 新建）、门禁 2 文件、脚本 1 文件 |

定位：分析文档回答「哪些成本能归零、哪些不能」；本规格回答「归零的那部分具体建成什么形状、用什么断言证明它真的归零」。成本口径（M=挂载成本、D=声明成本、B=特性本体成本）与不可达项一律引用分析文档，本文不复述，只定义终态结构、改造步骤与断言编号。

---

## 1 目标与判定口径

### 1.1 可判定目标

| 编号 | 目标 | 判定方式 |
|---|---|---|
| G1 | 新增一个可选特性时，core 侧改动 = 装配点 6 文件 9 行 + 足迹 1 文件 2 行，其中**调用型 0 行**（两处均存在进一步压缩形态，见 §3.2） | B1 + B6 断言；按 §7 清单人工复核 |
| G2 | 关闭某特性后，其后端依赖闭包断开 | `cargo tree --no-default-features -i <crate>` 无输出 |
| G3 | 关闭某特性后，前端产物不含该特性代码 | 关闭态构建后跑产物探针 |
| G4 | 现有 vault 行为零变化 | 命令集合、配置读写、退出清理、启动播种四类行为对照 |
| G5 | 门禁本身随特性数增长不膨胀 | 门禁只读清单与结构，不枚举特性名 |

### 1.2 非目标

- 不做全量生成器（M=0、调用型足迹全归零）。理由见分析文档：N=1 下生成器成本高于收益，且会引入「生成物入库 vs 真删除」的第三态维护负担。条件化升级判据见 §10。
- 不改任何业务逻辑、命令签名、DTO 结构、wire 字段。
- 不动 `src-tauri/tauri.conf.json`、`src-tauri/.cargo/config.toml`、根 `.cargo/config.toml`。
- 不迁移 3 项本期保留足迹（`cmd/mod.rs`、`protocol/dto/config.rs`、`protocol/events.rs`，见 §6.1）；其中配置 DTO 一项已由 `feature-config-ownership-spec.md` 迁出（该规格收口后此项归零，保留足迹为 2 项）。其余 4 项（`lifecycle.rs`、`setup.rs`、`ipc.rs`、`cmd/config.rs`）由 S4–S7 迁出。其中 `cmd/mod.rs` 属「本期保留」而非「原理不可归零」，零化形态与未采纳理由见 §3.2。

### 1.3 编号空间

| 前缀 | 含义 |
|---|---|
| G | 目标 |
| R | 挂载表契约条目 |
| S | 改造步骤（一次性动作） |
| B | 后端门禁断言（Rust 测试） |
| F | 前端门禁断言（vitest） |
| AC | 验收条目 |

---

## 2 终态结构

### 2.1 后端

```
src-tauri/src/
├── feature_mounts.rs        ← 新增：唯一挂载表（唯一建立「特性 → 挂载函数」映射的地方）
├── vault_plugin.rs          ← 特性本体：一对 cfg 包住 init/on_setup/on_exit 三函数
├── plugins.rs               ← 遍历表挂插件（不再出现特性名）
├── lifecycle.rs             ← 遍历表跑退出钩子（不再出现特性名）
├── setup.rs                 ← 遍历表跑启动钩子（不再出现特性名）
├── lib.rs                   ← 仅作装配声明：mod feature_mounts; + mod <id>_plugin;（已有）
└── cmd/mod.rs               ← 仅保留 `#[cfg(feature = "vault")]` + `pub(crate) mod vault;` 两行
```

数据流：`feature_mounts::mounts()` 是唯一读入口；`vault_plugin` 用一对互斥 cfg 提供同名同签名的两套实现，调用方永远只写 `(mount.plugin)()`、`(mount.on_setup)(app)`、`(mount.on_exit)(app)`，不写 `if`、不写 `cfg`。

### 2.2 前端

```
config/features.json         ← 唯一真源（已有，形态不变）
src/features/registry.ts     ← 遍历清单 + 动态 import 守卫（已有，形态不变；新增特性仍 +3 行）
src/features/vault/conf.ts    ← 配置槽 invoke 改名（特性自有文件，非 core）
```

说明：`registry.ts` 的「形态不变」指已是遍历 + 动态 import 守卫，不再需要结构改造；但它仍是每特性 3 行的 M 项（见 §7 步骤 6），不可归零。

### 2.3 capabilities

```
src-tauri/capabilities/default.json   ← 只留 core 与常驻插件权限
src-tauri/capabilities/vault.json     ← 新增：vault:default + windows 列表
```

### 2.4 构建脚本

```
src-tauri/build.rs           ← 遍历 src/*_plugin.rs 生成 InlinedPlugin 与 ACL
                               （每文件恰 1 个 generate_handler! 块，否则 panic，见 B9）
```

---

## 3 挂载表契约

`src-tauri/src/feature_mounts.rs` 是全部规则的落点，必须满足以下 6 条：

| 编号 | 契约 | 理由 |
|---|---|---|
| R1 | 条目只含 `id` + 三个函数指针，不含 `bool`、不含分支、不含闭包 | 表是数据不是逻辑，逻辑在特性本体里 |
| R2 | 本文件禁止出现 `#[cfg]` | 关闭态若删条目，`plugins.rs` 等调用方就得跟着写 `cfg`，调用型足迹重新出现 |
| R3 | 条目对清单全量声明，含关闭态特性 | 同 R2：表结构恒定，开关只影响函数体 |
| R4 | 三函数签名固定：`fn() -> TauriPlugin<Wry>` / `fn(&AppHandle) -> Result<(), String>` / `fn(&AppHandle)` | 签名漂移会让调用方需要泛型或 trait 对象，破坏「调用方不感知特性」 |
| R5 | `mounts()` 返回 `&'static [FeatureMount]`，无锁、无运行时初始化 | 启动期调用，不能引入初始化顺序依赖 |
| R6 | 不注册 conf applier、不 emit 事件、不做 IO | 副作用留在特性本体，避免关闭态表项有行为 |

参考实现：

```rust
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Wry};

pub(crate) type PluginFactory = fn() -> TauriPlugin<Wry>;
pub(crate) type SetupHook = fn(&AppHandle) -> Result<(), String>;
pub(crate) type ExitHook = fn(&AppHandle);

pub(crate) struct FeatureMount {
    pub id: &'static str,
    pub plugin: PluginFactory,
    pub on_setup: SetupHook,
    pub on_exit: ExitHook,
}

const MOUNTS: &[FeatureMount] = &[FeatureMount {
    id: "vault",
    plugin: crate::vault_plugin::init,
    on_setup: crate::vault_plugin::on_setup,
    on_exit: crate::vault_plugin::on_exit,
}];

pub(crate) fn mounts() -> &'static [FeatureMount] {
    MOUNTS
}
```

### 3.1 契约的框架依据

R4 的三函数形态不是风格偏好，而是由 Tauri 2 插件生命周期决定的事实：

| 事实 | 依据 | 对本规格的含义 |
|---|---|---|
| `Builder::plugin()` 内部即 `PluginStore::initialize()` → `TauriPlugin::initialize()`，当场执行插件 `.setup` 钩子 | tauri `src/app.rs` `plugin_boxed()`；`src/plugin.rs` `PluginStore::initialize` / `TauriPlugin::initialize` | 插件 `.setup` 早于 `Builder::setup(setup::init)`，故「特性配置先于 core 配置播种」不依赖表里的 `on_setup` |
| `PluginStore::on_event` 对每个 `RunEvent` 遍历所有已注册插件 | tauri `src/app.rs` `on_event_loop_event()` 末尾 | `RunEvent::Exit` 清理可直接写在插件 `.on_event` 里，不依赖表里的 `on_exit` |
| `Plugin` trait 无 `on_exit` 钩子；`on_drop` 是插件对象析构，不是进程退出 | tauri `src/plugin.rs` `Plugin` trait | 退出清理只能走 `on_event` 的 `Exit` 分支 |

即：表里的 `on_setup` / `on_exit` 是**显式化**（把顺序与清理写在一张可见的表里），不是**必需**。两者是同一终态的两种等价写法。

### 3.2 可选压缩形态（已评估，未采纳）

以下三项都能进一步降低 M，但都引入新的约定负担；本期不采纳，记录在此以免重复论证。任一项被采纳时，须同步修订 §6.2 的 B5/B6、§7 的行数口径与 §8 的 AC10。

| 形态 | 做法 | 收益 | 未采纳理由 |
|---|---|---|---|
| 表项降为 2 字段 | 删 `on_setup` / `on_exit`：播种移入插件 `.setup`，清理移入插件 `.on_event` 的 `Exit` 分支（依据见 §3.1）；`lifecycle.rs` / `setup.rs` 的遍历循环随之删除 | 新增特性 M 由 4 行降到 2 行（9 行 → 7 行）；B5 从「三函数签名一致」降到「`init` 签名一致」 | 顺序与清理变成框架隐式语义，表不再自解释；且 `.on_event` 是「所有事件」入口，退出清理藏在事件分支里可读性差 |
| `cmd/mod.rs` 足迹归零 | 把 `src/cmd/<id>/` 移到 `src/<id>/`，模块声明搬进特性边界：`#[cfg(feature = "<id>")] #[path = "<id>/mod.rs"] pub(crate) mod <id>;` | 足迹从 1 文件 2 行降到 0；B6 改为「`cmd/mod.rs` 无特性 cfg」 | 命令路径前缀从 `crate::cmd::<id>::` 变为 `crate::<id>_plugin::<id>::`，`build.rs` 的 `COMMAND_PREFIX_TEMPLATE`、前端 `isFeatureBody` 的目录识别、A6/A7 解析器都要跟着改；为 2 行收益改动 4 处解析口径不划算 |
| 分布式静态注册 | 每个 `*_plugin.rs` 用 `inventory` / `linkme` 自行 `submit!` 一条 `FeatureMount`，core 侧不再有表 | 新增特性 M 再省 2~4 行，且天然「加文件即挂载」 | 新增第三方依赖 + 链接段机制；注册顺序不受控，需额外排序兜底；N≥3 时 §10 的生成器是更好的答案，本项处于两者之间的尴尬位置 |

**注意**：`cmd/mod.rs` 的「不可归零」只在「不移动命令目录」的前提下成立。分析文档中「模块树必须静态声明」的结论仍然正确，但静态声明可以落在特性边界文件里，不必落在 core 文件里。

---

## 4 后端改造步骤

### S1 新增挂载表

新增 `src-tauri/src/feature_mounts.rs`（内容见 §3）；`lib.rs` 增加 `mod feature_mounts;`（与既有 `mod vault_plugin;` 同级，lib.rs 仍只做装配声明）。

### S2 特性本体收成三函数

`src-tauri/src/vault_plugin.rs` 改为「一对 cfg 包住同名实现」形态。要点：

- 互斥块内提供 `init` / `on_setup` / `on_exit` 三函数，块外用 `pub(crate) use mounted::{init, on_exit, on_setup};` 统一导出，使调用方看到的符号恒定。
- `init()` 内部保留现有的 `register_conf_applier("vault", ...)` 与插件 `setup`/`invoke_handler` 装配；`assistant_vault` 等特性 crate 的引用只允许出现在启用块内。
- `on_setup` 承接原 `setup.rs` 的 `VaultConf::load()`，返回 `Result<(), String>`，错误文案与原文一致（`Failed to init vault config: {}`）。
- `on_exit` 承接原 `lifecycle.rs` 的 `vault_cleanup_on_exit(app)`。
- 关闭块只提供 `tauri::plugin::Builder::<Wry>::new("vault").build()` 与两个空实现，且**不得引用任何 `assistant_vault` 符号**。

注意：`#[cfg(feature = "vault")]` 与 `#[cfg(not(feature = "vault"))]` 各 1 处，本文件特性 cfg 计数为 2，呈 B4 要求的「一对互斥块」形态；加上 `cmd/mod.rs` 的 1 行，全后端共 3 行，恰为 B4 收紧后的上限。

### S3 `plugins.rs` 改遍历

链式 `.plugin(...)` 中删掉 `.plugin(crate::vault_plugin::init())`，改为在链式常量插件之后统一遍历：

```rust
pub fn attach(mut builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder = builder
        .plugin(tauri_plugin_opener::init())
        // …既有常量插件保持链式…
        ;
    for mount in crate::feature_mounts::mounts() {
        builder = builder.plugin((mount.plugin)());
    }
    builder
}
```

顺序约束：特性插件必须排在链式常量插件之后（当前 `.plugin(crate::vault_plugin::init())` 是链尾，即 `autostart` 之后），改为遍历后仍保持链尾位置，`single_instance` 与 `global_shortcut` 的相对次序不变。

### S4 `lifecycle.rs` 改遍历

```rust
if let tauri::RunEvent::Exit = event {
    for mount in crate::feature_mounts::mounts() {
        (mount.on_exit)(app_handle);
    }
}
```

### S5 `setup.rs` 改遍历

删除 `VaultConf::load()` 块，并从分组导入 `use assistant_config::{...}` 中删掉 `vault_conf::VaultConf` 一项（该导入与 `app_conf::AppConf`、`orb_conf::OrbConf` 同组，不是独立 `use` 行）；在原位置插入：

```rust
for mount in crate::feature_mounts::mounts() {
    if let Err(e) = (mount.on_setup)(handle) {
        log::error!("{}", e);
    }
}
```

位置约束：必须排在 `AppConf::load()` 之前，保持原启动顺序（vault 配置先于 app 配置播种）。

### S6 `cmd/mod.rs` 收敛到 1 行 cfg

保留 `#[cfg(feature = "vault")] pub(crate) mod vault;`；删除 `#[cfg(feature = "vault")] pub use vault::vault_cleanup_on_exit;` 与 `#[cfg(not(feature = "vault"))] pub fn vault_cleanup_on_exit(_app: &tauri::AppHandle) {}` 空桩（空桩职责已上移到 S2 的关闭块）。

### S7 配置读取命令迁入特性边界

把 `get_vault_conf` 从 core 配置面迁到 vault 命令目录，消除「配置面命令 core 保留」这一特殊例外：

- `src-tauri/src/cmd/config.rs`：删除 `get_vault_conf` 命令本体，并清理随之无用的 `VaultConfDto`、`vault_conf::VaultConf` 导入。
- `src-tauri/src/cmd/vault/conf.rs`（新增）：`#[command] pub fn vault_get_conf() -> Result<VaultConfDto, String>`，沿用 `VaultConf::load()` + `VaultConfDto::from` + `log::error!` 的原有语义。
- `src-tauri/src/cmd/vault/mod.rs`：按既有约定加 `mod conf; pub use conf::*;`。
- 新命令必须在上方声明 `// precondition: None`，否则 `assistant-vault/tests/command_gate.rs` 的 `vault_commands_declare_precondition_in_place` 失败。
- `src-tauri/src/ipc.rs`：删除 `cmd::get_vault_conf,` 一行；新命令由 `vault_plugin.rs` 的 `generate_handler!` 注册。
- 前端 `src/features/vault/conf.ts`：`invoke: "get_vault_conf"` → `invoke: "vault_get_conf"`。

命名约束：新命令名必须带 `vault_` 前缀且不得与 `assistant-config/src/vault_conf.rs` 的 `pub vault_*` 字段重名——`assistant-vault/tests/conf_keys.rs` 会把 `src/cmd/vault/*.rs` 里的 `.vault_<字段>` 与 `vault_conf.rs` 的 `pub vault_*` 声明做集合比对，新增文件不得引入未声明的 `.vault_` 访问。

`src/settings/__tests__/snapshotInvariants.test.ts` 里的 `get_vault_conf` 是通用槽位夹具（合成数据），不随本改造调整。

### S8 `build.rs` 泛化

现状：`PLUGIN_SOURCE`、`COMMAND_PREFIX`、`.plugin("vault", ...)` 三处硬编码。改为按文件系统遍历：

1. 扫 `src/` 下文件名匹配 `*_plugin.rs` 的文件，去 `_plugin` 后缀得到插件 id（`vault_plugin.rs` → `vault`），排序保证稳定。
2. 对每个 id，沿用现有解析器（前缀 `crate::cmd::<id>::`、`default_permission(AllowAllCommands)`），命令名保持 snake_case 原样输出——`InlinedPlugin::commands` 要求 snake_case，权限名 `allow-<snake>` 由 tauri-build 自行生成，不得在此转 camelCase。
3. 对每个源文件发 `cargo:rerun-if-changed=src/<id>_plugin.rs`，另发 `cargo:rerun-if-changed=src` 以感知新增/删除文件。
4. 用 `Attributes::plugins(iter)` 一次性注入：`plugin(name: &'static str, ...)` 与 `InlinedPlugin::commands(&'static [&'static str])` 都要求 `'static`，故 id 与命令名数组按现有做法 `Box::leak`（build.rs 单次执行，泄漏无害）。
5. 某文件解析不到命令时仍 panic，保留「插件忘了注册命令」这一失败模式。
6. 同一文件出现第 2 个 `generate_handler![` 块时 panic：解析器只读首个块，多块会让后续命令被静默丢弃，且失败现场指向 ACL 而非解析器（测试期由 B9 复验同一判据）。

兼容约束：泛化后 `vault` 的插件 id、命令列表、默认权限必须与当前产物逐字节一致，否则 ACL 与前端 `invoke` 全部失配。

---

## 5 capabilities 改造（S9）

- 新增 `src-tauri/capabilities/vault.json`：`identifier: "vault"`，`windows: ["main", "orb"]`（与 `default.json` 当前窗口列表一致），`permissions: ["vault:default"]`。
- `src-tauri/capabilities/default.json`：删除 `"vault:default"` 项，其余保持。

约束：capability 文件名与 `identifier` 取特性名。`<id>:default` 必须始终可解析——这由「`build.rs` 无条件声明插件（不按 `CARGO_FEATURE_*` 门控）」保证：关闭态仍从 `*_plugin.rs` 文本派生 ACL 与 `allow-*` 权限，权限存在但无命令，属惰性声明。因此**不得**把 S8 的插件声明改成按开关门控，否则关闭态构建会因「权限不存在」失败。

代价记账：关闭态残余 3 项（空插件注册、ACL 惰性声明、capability 文件静态存在），判据仍是 `cargo tree --no-default-features`，不依赖这三项。

---

## 6 门禁改造

### 6.1 退役项

| 退役 | 原因 |
|---|---|
| 前端 `FEATURE_FOOTPRINT_ALLOWLIST` 的 vault 7 文件白名单 | 遍历化后 `lifecycle.rs`、`setup.rs`、`ipc.rs`、`cmd/config.rs` 不再被特性触碰（前两者由遍历钩子取代，后两者由 S7 迁出），白名单收缩为 3 项，见 F2 |
| 后端 A3「lib.rs 仅作装配层」放宽为「lib.rs 只允许 `mod`/`pub use` 声明」 | `mod feature_mounts;` 属于装配声明，不构成足迹 |
| A5 解析器下限 | 泛化后解析器按文件遍历，下限断言改为「每个 `*_plugin.rs` 至少 1 条命令」 |
| A6 的「桩 vs 真实现签名比对」 | S6 删除 `cmd/mod.rs` 的空桩后该比对失去一侧对象，`normalized_signature` 会直接 panic；改由 B5 承接为「同一 `*_plugin.rs` 内启用块与关闭块三函数签名一致」，A6 只保留「宿主与插件注册不重叠」 |
| A7 的「build.rs 引用真源」子断言 | S8 泛化后 `build.rs` 不再含逐特性的 `"src/<id>_plugin.rs"` 字面量（只剩 `format!` 模板），`build_rs_plugin_sources()` 会解析出模板串并误判「未引用真源」；改为「`build.rs` 含 `*_plugin.rs` 遍历逻辑且不含清单内特性 id 字面量」，A7 的「注册表 ↔ ACL 双向一致」保留 |

收缩后的 2 项本期保留足迹（由现有 `FEATURE_FOOTPRINT_ALLOWLIST` 的 7 项扣除 S4/S5/S7 迁出的 4 项与 `feature-config-ownership-spec.md` 迁出的配置 DTO 一项后确定）：

| 文件 | 残留内容 | 保留理由 |
|---|---|---|
| `src-tauri/src/cmd/mod.rs` | `#[cfg(feature = "vault")]` + `pub(crate) mod vault;` | 本期不移动命令目录，故模块声明留在 core；零化形态见 §3.2，代价是改动 4 处路径解析口径 |
| `src-tauri/src/protocol/events.rs` | 3 个 vault 事件名常量 | 跨端协议名集中登记，拆散会让事件名失去唯一来源 |

> `src-tauri/src/protocol/dto/config.rs` 的 `VaultConfDto` 一项已由 `feature-config-ownership-spec.md` 迁入 vault 契约域（`assistant-vault/src/dto/conf.rs`），上表因此由 3 项收缩为 2 项。

### 6.2 后端新增断言

`src-tauri/tests/assembly_contract.rs` 追加：

| 编号 | 断言 | 失败含义 |
|---|---|---|
| B1 | `feature_mounts.rs` 内 `#[cfg(` 出现次数 = 0 | 有人把开关写回表里，调用型足迹回潮 |
| B2 | 三集合全等：`config/features.json` 键集合 == `MOUNTS` 的 `id` 集合 == 磁盘 `src/*_plugin.rs` 的 stem 集合；另加一条**子集**断言：清单各条目的 `cargoFeature` 取值集合 ⊆ `Cargo.toml` `[features]` 键集合（排除 `default`） | 声明态、挂载表、磁盘文件三方漂移；或清单声明了 Cargo 里不存在的开关 |
| B3 | `plugins.rs` / `lifecycle.rs` / `setup.rs` 文本中不出现任何清单内特性 id 字面量（如 `vault`） | 调用方又直接点名特性，遍历化被绕过 |
| B4 | 全后端某特性 `#[cfg]` 行数 ≤ 3（`VAULT_CFG_LIMIT` 由 6 收紧到 3），且 `*_plugin.rs` 内为「一对互斥块」 | 特性本体内部二次门控，或空桩回流使 cfg 反弹 |
| B5 | 每个 `src/*_plugin.rs` 都存在 `#[cfg(not(feature = "<id>"))]` 关闭块，且关闭块与启用块的 `init` / `on_setup` / `on_exit` 三函数签名一致 | 关闭态没有空实现，core 无法编译；或两套实现签名漂移，挂载表的函数指针类型对不上 |
| B6 | `cmd/mod.rs` 中特性 cfg 行数 = 特性数（每特性 1 行 `mod` 声明） | 空桩或 `pub use` 回流，调用型足迹增加 |
| B7 | `feature_mounts.rs` 内 `&[FeatureMount]` 常量恰为 1 个，且只被 `mounts()` 返回 | 出现第二张表，遍历点分叉 |
| B8 | 每个特性 id 存在 `capabilities/<id>.json` 且 `permissions` 含 `<id>:default`，`default.json` 不含 | 权限随特性关闭仍常驻，或关闭态构建失败 |
| B9 | 每个 `src/*_plugin.rs` 恰有 1 个 `generate_handler![` 块 | `build.rs` 与 ACL 派生只读首个块，多块会让后续命令被静默丢弃（ACL 少声明、运行期 invoke 报权限不足） |

B2 的 Cargo 侧必须是**子集**而不是全等：Cargo 特性可以承载与插件无关的用途（如 tauri 模板的 `custom-protocol`），全等会在这类特性出现时误报。清单键集合与挂载表、磁盘文件之间才是全等关系——这三方任一方多出条目都意味着「有开关没挂载」或「有文件没声明」。

B2 的「清单 ↔ Cargo 特性」部分与 `scripts/feature-build.ps1` 的启动校验同源（脚本负责构建期拦截，测试负责仓库期拦截，判据必须是同一份清单）；B2 的「挂载表 ↔ 磁盘」与 B8 的 capabilities 拆分是脚本当前不覆盖的，只由测试守。

B9 与 S8 的解析器是同一条风险的两种拦截：`build.rs` 里同一判据同时 panic（构建期），B9 在测试期复验。之所以单列，是因为「命令被静默丢弃」不产生编译错误，只有 A7 的「注册表 ↔ ACL」才会报出，且报错信息指向 ACL 而非解析器。

实现口径（避免断言各自造解析器）：

| 断言 | 复用/新增解析 | 说明 |
|---|---|---|
| B2 | 复用 `plugin_feature_names()`（已按 `*_plugin.rs` 遍历磁盘）；新增 `mounts_ids()`，按 `id: "<literal>"` 提取 `feature_mounts.rs` 的字符串字面量；Cargo 侧复用现有 `[features]` 段解析，只取清单条目的 `cargoFeature` 做子集判定 | `MOUNTS` 表是数据，字面量提取足够，无需语法分析 |
| B5 | 把现有 `normalized_signature(text, file)` 从「固定找 `fn vault_cleanup_on_exit`」泛化为「按函数名查找」，逐个比对 `init` / `on_setup` / `on_exit` | 现有 `CLEANUP_FN` 常量退役，改为传入函数名 |
| B6 | 复用 `cfg_feature_names()` 按文件计数，只统计 `cmd/mod.rs` | 与 B4 同一解析器，两处口径不分离 |
| B8 | 新增：读 `capabilities/*.json`，按 `identifier` 与 `permissions` 断言 | capability 是静态 JSON，直接解析 |
| B9 | 复用 `HANDLER_OPEN` 常量对每个 `*_plugin.rs` 计数，`!= 1` 即失败；`build.rs` 的解析器用同一常量做同样的 panic | 构建期与测试期共用一条判据，避免两侧口径分叉 |

A7 保留部分：`plugin_feature_names()` 遍历出的每个插件，其 `generate_handler!` 注册集合与 `gen/schemas/acl-manifests.json` 对应条目必须双向相等。A7 退役的只是「build.rs 逐特性引用真源」这一子断言，改为「`build.rs` 文本含目录遍历（`read_dir`）与 `"_plugin.rs"` 后缀常量」——这条与 F6 的「`build.rs` 无特性 id 字面量」互补，一起证明泛化真的发生。

### 6.3 前端新增断言

`src/features/__tests__/coreBoundary.test.ts`：

| 编号 | 断言 | 失败含义 |
|---|---|---|
| F1 | `ASSEMBLY_POINTS` 收缩为 6 项：移除 `src-tauri/build.rs`、`src-tauri/capabilities/default.json`、`src-tauri/src/plugins.rs`，新增 `src-tauri/src/feature_mounts.rs`；保留 `config/features.json`、`src/vite-env.d.ts`、`src/features/registry.ts`、`src-tauri/Cargo.toml`、`src-tauri/src/lib.rs`。同时「装配点齐备」测试里三条**按特性点名调用型文件**的检查必须改写：删掉 `build.rs.includes("\"<name>\"")`（由 F6 + A7 的泛化断言覆盖）、删掉 `plugins.rs.includes("<name>_plugin::init()")`（由 B2/B3 覆盖）、`capabilities/default.json` 的 `"<name>:default"` 检查改为「存在 `capabilities/<name>.json` 且其 `permissions` 含 `<name>:default`」 | 已遍历化的文件仍按特性逐处断言（存在性断言残留），或挂载表未登记导致其 id 被误判为足迹；或泛化后仍留着「build.rs 必须写特性名」这类与遍历化直接矛盾的检查 |
| F2 | `FEATURE_FOOTPRINT_ALLOWLIST` 每特性条目从 7 项收缩为 2 项（`cmd/mod.rs`、`protocol/events.rs`；`protocol/dto/config.rs` 一项已由 `feature-config-ownership-spec.md` 归零），且只减不增 | 已消除的足迹被重新引入，或豁免清单被当成新增足迹的通行证 |
| F3 | `registry.ts` 的 import 形态为「动态 import + 编译期守卫」，静态 import 一律失败 | 退回静态 import 后 chunk 无法裁剪 |
| F4 | `config/features.json` 每个条目的 `env` 字段都能在 `src/vite-env.d.ts` 找到 `declare const __<env>__: boolean`（如 `env: "VAULT_ENABLED"` ↔ `__VAULT_ENABLED__`） | define 注入缺失，`registry.ts` 的守卫恒为 `undefined`，特性永不加载 |
| F5 | 关闭态产物探针：`node scripts/assert-feature-absent.mjs --feature vault --dist dist` 断言 `dist/assets` 内无特性专有字符串（取该特性 i18n 键或路由字面量）。vitest 包装层的语义必须是「无关闭态声明则显式 `ctx.skip()`，声明了关闭态（`env=0`）却缺 `dist/assets` 则失败」——静默 `return` 会让该断言在新鲜检出下永远为绿，等于不存在 | chunk 泄漏 |
| F6 | 反向断言：`src-tauri/build.rs`、`src-tauri/capabilities/default.json`、`src-tauri/src/plugins.rs`、`src-tauri/src/lifecycle.rs`、`src-tauri/src/setup.rs` 文本中不出现任何清单内特性 id 字面量 | 遍历化后调用方又直接点名特性，与 B3 互补（前端侧覆盖同一批文件） |

F1 与 F6 的关系：F1 管「谁还需要按特性改」，F6 管「谁已经不该再出现特性名」。两者对同一批文件方向相反，同时成立才算遍历化落地；只做 F1 会把断言删空，只做 F6 会失去装配点齐备性检查。

F5 新增脚本 `scripts/assert-feature-absent.mjs`：探针字面量**不写进清单**（`config/features.json` 形态不变），而是由脚本从 `src/features/<id>/` 源码里提取引号字符串中匹配 `<prefix>.<id>.` 的部分（现有 vault 的 `settings.vault.*` 即符合该前缀约定），逐个扫描 `dist/assets/*.js`，命中即非零退出。选取依据是「会被压缩保留的字符串」——i18n key 与路由 path 满足，标识符会被 mangle，不可用。

边界：脚本只对 `src/features/<id>/` 内的字面量负责；若特性把用户可见文案搬到 `src/locales/` 而不在特性目录留 key，则探针无字面量可扫，此时脚本须报「未提取到探针字面量」并非零退出，避免空扫描静默通过。

---

## 7 新增特性操作序列（M 口径复核）

实施后新增特性 `x` 的标准动作，用来验证 G1。计数规则：只统计**已存 core 文件的修改行**；新建文件单独列出，不计入行数。表按分析文档的两类口径分组：**装配点**（`coreBoundary.test.ts` 的 `ASSEMBLY_POINTS`）与**足迹**（同一测试的 `FEATURE_FOOTPRINT_ALLOWLIST`）。

### 7.1 装配点（6 文件）

| 步骤 | 文件 | 性质 | 改动 |
|---|---|---|---|
| 1 | `config/features.json` | D | +1 条目 |
| 2 | `src-tauri/Cargo.toml` | D | +1 行（`[features]` 声明） |
| 3 | `src-tauri/src/lib.rs` | M | +1 行（`mod x_plugin;`） |
| 4 | `src-tauri/src/feature_mounts.rs` | M | +4 行（表项：`id` / `plugin` / `on_setup` / `on_exit`；可选降到 2 行，见 §3.2） |
| 5 | `src/features/registry.ts` | M | +3 行（守卫） |
| 6 | `src/vite-env.d.ts` | M | +1 行（declare） |

装配点合计：**6 文件，M = 4 文件 9 行（后端 2 文件 5 行 + 前端 2 文件 4 行）**，D = 2 文件 2 条。

与《FEATURE-ZERO-COST-MOUNT-ANALYSIS.md》§3 的「6 文件 8 行」差异 1 行，来源是表项字段：分析文档按 `plugin` + `on_exit` 两项计（启动播种隐含在插件 `setup` 钩子内），本规格显式保留 `on_setup`，并保留 `id` 供 B2 做漂移比对，故为 4 行。显式化的理由是「表自解释」——启动顺序与退出清理集中在一张可见的表里，审查新增特性时无需回读框架语义；**不是**因为顺序需要框架之外的保证，§3.1 已核实插件 `.setup` 在 `Builder::plugin()` 阶段执行、`on_event` 会广播 `Exit`。若采纳 §3.2 的 2 字段形态，可退回 8 行。

### 7.2 足迹（1 文件）

| 步骤 | 文件 | 性质 | 改动 |
|---|---|---|---|
| 7 | `src-tauri/src/cmd/mod.rs` | M（足迹，非装配点） | +2 行（`#[cfg(feature = "x")]` + `mod x;`） |

足迹合计：**1 文件 2 行**，即 §6.1 所列 2 项保留足迹中唯一可被 S6 压到「每特性恒定 1 条 cfg 属性 + 1 行 `mod`」的一项；若要归零（把 `mod` 声明搬进特性边界），见 §3.2 的目录迁移方案与代价。另一项（`protocol/events.rs` 的事件名常量）只在特性自带事件时才 +1 条目，属特性本体（B），不是装配成本；无事件的特性不触碰它。（原「`protocol/dto/config.rs` 的 `*ConfDto`」一项已由 `feature-config-ownership-spec.md` 迁入 vault 契约域，不再计入。）

口径区分：B6 数的是 `#[cfg(...)]` **属性行数**（每特性 1 条），§7.2 数的是**新增总行数**（属性行 + `mod` 行 = 2）。两者不冲突，`cmd/mod.rs` 的当前形态正是「属性行 + `mod` 行」两行（vault 在 19–20 行）。

口径提示：§7.1 的「9 行」只统计装配点，不含足迹。带 DTO + 事件的特性（如 vault）总改动为 **7 文件 11 行**；纯前端/无协议面的特性为 **6 文件 9 行 + 足迹 1 文件 2 行**。

### 7.3 新建文件（不计行，但需列出以便审查）

`src-tauri/src/x_plugin.rs`（含一对 cfg 的关闭块）、`src-tauri/capabilities/x.json`、`src/features/x/`（模块与配置槽）。

### 7.4 合计与调用型归零

**7 文件 11 行**（装配点 6 文件 9 行 + 足迹 1 文件 2 行）。调用型（`plugins.rs` / `lifecycle.rs` / `setup.rs`）**0 行**——这三处只写一次遍历循环，此后永不再改，满足 G1 的核心判定。

可再压缩的项：步骤 6 的 `vite-env.d.ts` declare 可由 `feature-build.ps1` 按清单生成（省 1 行），属 §10 的升级触发点。

不可压缩的项：步骤 7 的 `mod` 行在本期**不压缩**。模块树是 Rust 静态结构，声明本身无法免除；但声明位置可以选——放进 core 的 `cmd/mod.rs` 是「每特性 2 行」，放进特性边界文件（`#[path]`）是「core 侧 0 行、特性侧 2 行」，代价是命令路径前缀与 3 处解析口径随之改动（详见 §3.2）。本期取前者，因为路径稳定比 2 行更值钱；`feature_mounts.rs` 不能承载该声明——那会违反 R2。

---

## 8 验收

### 8.1 命令（按序执行，全部在仓库根）

```powershell
.\scripts\feature-build.ps1 -Task check
.\scripts\feature-build.ps1 -Task check -Without vault
.\scripts\feature-build.ps1 -Task test
.\scripts\feature-build.ps1 -Task test -Without vault
npm run typecheck
npm run lint
npm run test
npm run contract:check
```

补充专项命令（`cwd = src-tauri`）：

```powershell
cargo test --test assembly_contract
cargo tree --no-default-features -i assistant-vault
cargo tree --no-default-features | Select-String assistant-vault
```

第二条在关闭态会以 exit 1 报 `did not match any packages`（optional 依赖未进图），属 AC3 的通过形态之一；第三条无命中即等价通过。

产物探针（先关闭态打包，再回到仓库根扫描）：

```powershell
.\scripts\feature-build.ps1 -Task build -Without vault
node scripts/assert-feature-absent.mjs --feature vault --dist dist
```

说明：`-Task test` 只跑后端 cargo 测试，前端测试走 `npm run test`；`-Without` 是关闭特性的唯一可靠写法（`-Features -` 在 PowerShell 下与参数前缀歧义）。

### 8.2 判定表

| 编号 | 判定 | 通过条件 |
|---|---|---|
| AC1 | 开启态编译 | `-Task check` 退出码 0 |
| AC2 | 关闭态编译 | `-Task check -Without vault` 退出码 0 |
| AC3 | 依赖断开 | `cargo tree --no-default-features -i assistant-vault` 满足任一形态：① 输出「nothing depends on」；② 无输出；③ exit 1 且报 `did not match any packages`（关闭态该 optional 依赖未进入依赖图，断开更强）。判定用 `cargo tree --no-default-features \| Select-String assistant-vault` 无命中作为等价判据 |
| AC4 | 后端门禁 | `assembly_contract` 全绿，含 B1–B9 |
| AC5 | 前端门禁 | `npm run test` 中 `coreBoundary` 全绿，含 F1–F6 |
| AC6 | 契约同步 | `npm run contract:check` 退出码 0（S7 只换命令名、不改 DTO，应零 diff） |
| AC7 | 产物探针 | 关闭态 `-Task build` 后跑 `assert-feature-absent.mjs`，退出码 0 |
| AC8 | 行为等价 | 开启态下：总命令数不变（S7 使 core −1、插件 +1，前端 invoke 名同步为 `vault_get_conf`）、配置读写路径不变、退出清理仍触发、启动仍播种 `VaultConf` |
| AC9 | 权限等价 | 开启态下 `vault:default` 仍授予 `main`/`orb`；关闭态构建不因缺权限报错 |
| AC10 | 足迹计数 | B6 断言 `cmd/mod.rs` cfg 行数 = 1；F2 断言 allowlist 恰为 3 项；F6 断言调用方无特性名 |

---

## 9 风险与回滚

| 风险 | 触发点 | 处置 |
|---|---|---|
| `feature_mounts.rs` 出现 cfg | 有人为省一次空调用加 `#[cfg]` | B1 直接失败；表项恒在是硬约束 |
| `plugins.rs` 遍历顺序变化 | 特性插件挪到 `single_instance` 之前 | 以 AC8 行为对照发现；顺序写死在 S3 注释 |
| `build.rs` 泛化破坏 ACL | 插件 id 或命令名派生规则写错 | S8 要求逐字节等价；`npm run tauri build` 与 AC9 覆盖 |
| 关闭态 capability 解析失败 | `vault.json` 在关闭态仍引用 `vault:default` | 由「`build.rs` 无条件声明插件」保证可解析；若有人给插件声明加 `CARGO_FEATURE_*` 门控即破坏该保证，AC2 会立即失败 |
| 足迹豁免清单被当成通道 | 新人把新逻辑写进 allowlist 的 3 项文件 | F2 要求该清单只减不增，且新增特性禁止触碰这 3 个文件 |
| 命令被静默丢弃 | 插件文件出现第 2 个 `generate_handler!` 块，`build.rs` 只读首个 | 构建期 panic（S8 步骤 6）+ B9 测试期复验；处置是合并为单块，不扩解析器 |
| 回滚 | 任一步失败 | 按 S1→S8 逆序回退，每步自包含，无跨步依赖 |

---

## 10 条件化升级判据（生成器）

本规格以「遍历化 + 行为门禁」为终态。满足下列任一条件时再评估生成器（把 `feature_mounts.rs` 表项、`cmd/mod.rs` 的 `mod` 行、`vite-env.d.ts` 的 declare 改为生成物）：

| 判据 | 来源 | 说明 |
|---|---|---|
| 挂载表条目数 ≥ 3 | 分析文档 §5.5（主判据） | 手写 M 线性增长，生成器边际成本不变 |
| `build.rs` 需要第二套派生规则 | 分析文档 §5.5（主判据） | 说明「按文件遍历」已不足以覆盖，派生逻辑本身开始分叉 |
| `cmd/mod.rs` 的 cfg 行数 ≥ 6 | 本规格（辅判据） | 单文件 cfg 密度触发人工审查疲劳，B6 会持续逼近 |
| 出现「同一特性在 core 侧需要 2 处以上声明」 | 本规格（辅判据） | 声明型足迹开始复利，单表模型失效 |

主判据来自分析文档，辅判据是把「可判定」落到本规格的门禁编号上；任一条满足即可，无需全部满足。

在此之前，生成器属于过度设计：它会引入生成物与手写文件的第三态，并使「真删除」这一前端三角约束在 Rust 侧失效。

### 10.1 已评估并否决的替代方案

| 方案 | 否决理由 | 详见 |
|---|---|---|
| `build.rs` 按 `CARGO_FEATURE_*` 门控插件声明 | 关闭态 `capability` 引用的 `<id>:default` 会解析失败，构建直接报错 | §5 |
| 挂载表用 `inventory` / `linkme` 分布式注册 | 新依赖 + 链接段机制 + 注册顺序不可控；N≥3 时生成器更优 | §3.2 |
| 表项降为 2 字段（`plugin` + `id`） | 顺序与清理退化为框架隐式语义，表不再自解释 | §3.2 |
| `cmd/<id>/` 迁到 `src/<id>/` 以归零 `cmd/mod.rs` | 为 2 行收益改动 4 处路径解析口径（`build.rs` 前缀、`isFeatureBody`、A6/A7） | §3.2 |
| 后端整体改为「一个 crate 一特性」的 workspace 拆分 | 编译单元与链接期成本随特性数增长，且与「单二进制内可选装配」目标冲突 | 分析文档 §5 |

---

## 11 实施记录（S1–S9 落地结果）

### 11.1 落点

| 步骤 | 落点文件 | 结果 |
|---|---|---|
| S1 | `src-tauri/src/feature_mounts.rs`（新增） | `FeatureMount` + `MOUNTS` 常量 + `mounts()`，无 cfg、无分支 |
| S2 | `src-tauri/src/lib.rs` | 新增 `mod feature_mounts;`，与 `mod vault_plugin;` 同级 |
| S3 | `src-tauri/src/plugins.rs` | 删除硬编码 `.plugin(vault_plugin::init())`，改为遍历 `feature_mounts::mounts()`，链尾位置不变 |
| S4 | `src-tauri/src/lifecycle.rs` | 删除硬编码 `vault_cleanup_on_exit`，改为遍历 `mount.on_exit` |
| S5 | `src-tauri/src/setup.rs` | 删除 `VaultConf::load()` 与导入，改为遍历 `mount.on_setup`，仍位于 `AppConf::load()` 之前 |
| S6 | `src-tauri/src/vault_plugin.rs`、`src-tauri/src/cmd/mod.rs` | 插件文件重写为「一对互斥 cfg 块 + 三函数同签名 + 块外 `pub(crate) use`」；`cmd/mod.rs` 只剩 1 行 `#[cfg(feature = "vault")] pub(crate) mod vault;` |
| S7 | `src-tauri/src/cmd/vault/conf.rs`（新增）、`src-tauri/src/cmd/config.rs`、`src-tauri/src/ipc.rs`、`src/features/vault/conf.ts` | `vault_get_conf` 迁入特性命令目录，core 侧删注册行与命令本体，前端 invoke 名同步 |
| S8 | `src-tauri/build.rs` | 改为 `read_dir("src")` 遍历 `*_plugin.rs`，`Box::leak` 满足 `'static`，同文件 `generate_handler!` 块数 ≠ 1 即 panic |
| S9 | `src-tauri/capabilities/vault.json`（新增）、`src-tauri/capabilities/default.json` | 特性权限拆分；`default.json` 删除 `"vault:default"` |

配套改动：`src-tauri/tests/assembly_contract.rs` 追加 B1–B9 并把 A5/A6/A7 按 §6.1 退役口径改写；`src/features/__tests__/coreBoundary.test.ts` 按 F1–F6 改写并新增 4 条断言；`scripts/assert-feature-absent.mjs`（新增）实现 F5 探针；`src/services/__tests__/commandNames.test.ts` 的字面量扫描补 `invoke:` 字段形态（conf 槽位用 `invoke: "vault_get_conf"`，非函数调用形态）。

### 11.2 实测

| 判定 | 命令 | 实测 |
|---|---|---|
| AC1 | `.\scripts\feature-build.ps1 -Task check` | 退出码 0，`--no-default-features --features vault` |
| AC2 | `.\scripts\feature-build.ps1 -Task check -Without vault` | 退出码 0，`VAULT_ENABLED=0` + `--no-default-features` |
| AC3 | `cargo tree --no-default-features -i assistant-vault` | `package ID specification assistant-vault did not match any packages`（包不在依赖图内，等价「nothing depends on」） |
| AC4 | `cargo test --test assembly_contract`（两种特性组合各一次） | 17 passed / 0 failed（A1–A8 + B1–B9） |
| AC5 | `npm run test` | 40 个测试文件全绿，`coreBoundary` 7 条断言全绿（含 F1–F6） |
| AC6 | `npm run contract:check` | 退出码 0，`git diff` 零输出 |
| AC7 | `.\scripts\feature-build.ps1 -Task build -Without vault -NoBundle` 后跑 `node scripts/assert-feature-absent.mjs --feature vault --dist dist` | release 构建 `Finished in 4m 49s`、退出码 0；探针扫 74 个产物、18 条探针、无泄漏，退出码 0 |
| AC8 | 后端全量测试两种组合 | 开启态 57/2/3/17/14/6/1/13/26/10 各套件全绿；关闭态同数全绿；`assistant-vault` 套件（含 `command_gate` / `command_registry` / `conf_keys`）全绿 |
| AC9 | 开启态 `vault:default` 授予窗口 / 关闭态构建 | `vault.json` 覆盖 `main`+`orb`；关闭态 AC2 通过，权限惰性声明可解析 |
| AC10 | B6 / F2 / F6 | `cmd/mod.rs` 特性 cfg = 1；allowlist 恰 3 项；调用方 5 文件无特性名 |

足迹实测：全后端 `vault` 特性 `#[cfg]` 共 3 行（`vault_plugin.rs` 2 行互斥块 + `cmd/mod.rs` 1 行 `mod`），恰好等于 B4 收紧后的上限 3。

### 11.3 关闭态残余（与 §5 记账一致）

空插件注册（`Builder::new("vault").build()` 无命令）、ACL 惰性声明（`allow-vault-*` 存在但无命令）、`capabilities/vault.json` 静态存在。三项均不产生运行时行为，也不进入依赖图——AC3 已实测确认。
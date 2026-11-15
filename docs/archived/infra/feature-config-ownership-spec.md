# 特性配置归属规格

| 项 | 值 |
|---|---|
| 状态 | 规格（定义终态与判定，不含实施记录） |
| 日期 | 2026-11-15 |
| 关联 | `./feature-config-ownership-tasklist.md`（实施清单）；`../../store/feature-mount-traversal-spec.md`（本规格收口其 §6.1 保留足迹中的「配置 DTO」一项）；`../../store/vault-optional-build.md`（本规格替代其 §4.6 配置层取舍）；`../../store/crate-architecture.md`（依赖图）；拆分判据 `../../principle/CODE-SPLIT-PRINCIPLE.md` §1 与 §6 |
| 影响面 | 后端 29 文件（修改 23 / 新建 4 / 删除 2）、前端 3 文件、契约生成物 2 文件、文档 7 处 |

定位：挂载链路已按 `feature-mount-traversal-spec.md` 遍历化，**配置链路没有**。本规格只定义「一个特性如何拥有自己的配置」的终态结构、契约条目与门禁断言；不改任何业务逻辑、wire 字段、磁盘文件名与前端设置页形态。

---

## 1 目标与判定口径

### 1.1 可判定目标

| 编号 | 目标 | 判定方式 |
|---|---|---|
| G1 | 新增一个可选特性的配置组，主应用侧改动 **0 行**（新建文件不计入） | 按 §5 清单人工复核；B2 断言 |
| G2 | 关闭某特性后，该特性的配置 schema、默认值、校验规则、旧键接管逻辑全部不参与编译 | `cargo tree --no-default-features -i <crate>` 无输出 + B1 断言 |
| G3 | 关闭某特性后，主契约 `src/types/protocol.contract.ts` 不含该特性的任何配置类型 | B3 断言 |
| G4 | 现有行为零变化：磁盘文件名、字段名、错误文案、启动顺序、`CONF_LOCK` 语义、前端 `patch_conf` 调用形态 | AC1–AC6 |
| G5 | 配置生命周期实现只有一份，不再按 group 复制 | §2.2 行数对照；B1 断言 |
| G6 | 门禁不枚举特性名，随特性数增长不膨胀 | §6.3 |

### 1.2 非目标

- 不合并为单个 `config.json`（理由见 §9）。
- 不提供通用 `get_conf(group: String) -> Value` 命令（理由见 §9）。
- 不在 `assistant-config` 内部加 `#[cfg(feature = "vault")]` 门控（理由见 §9）。
- 不改前端字段定义、section、page、路由与快照夹具（`src/features/vault/module.ts`、`src/settings/__tests__/fixtures/`）。
- 不改 `patch_conf` 命令签名、ACL、`confRuntime` / `settingsStore` 的加载机制。
- 不改 `VaultConf` 的 12 个字段名与 `vault-config.json` 的文件名（`VaultConf` 无 `rename_all`，磁盘上是 snake_case）。
- 不做 `assistant-config` 的跨组依赖注入框架（`ConfGroup` 之间不得互相引用，见 R9）。

### 1.3 编号空间

| 前缀 | 含义 |
|---|---|
| G | 目标 |
| R | `ConfGroup` 契约条目 |
| S | 改造步骤（一次性动作） |
| B | 后端门禁断言（Rust 测试） |
| F | 前端门禁断言（vitest） |
| AC | 验收条目 |

---

## 2 现状定性

### 2.1 配置链路的七个环节

| # | 环节 | 当前位置 | 归属正确性 |
|---|---|---|---|
| 1 | schema（字段定义） | `src-tauri/crates/assistant-config/src/vault_conf.rs` | 错误：特性策略落在无门控的基础设施 crate |
| 2 | 默认值 / `validate` / 旧键接管 | 同上 | 错误：同上，含 auth_sock 绝对路径、scp 形态 URL、bare host 等 vault 专属规则 |
| 3 | 文件名 `vault-config.json` | 同上（`get_conf_path`） | 错误：同上 |
| 4 | 读取命令 | `src-tauri/src/cmd/vault/conf.rs` | 正确 |
| 5 | 写入 applier 注册 | `src-tauri/src/vault_plugin.rs`（`init()` 内） | 位置正确，时机可议（见 §5.9 O1） |
| 6 | DTO + 契约生成 | `src-tauri/src/protocol/dto/config.rs`（主契约，无门控） | 错误：关闭态仍导出零消费者类型 |
| 7 | 前端槽位与字段 | `src/features/vault/conf.ts`、`src/features/vault/module.ts` | 正确 |

写入分发（`src-tauri/src/cmd/mod.rs` 的 `CONF_APPLIERS` 注册表 + `register_conf_applier`）与前端装配（`src/main.tsx` 遍历 `getConfGroups()`、`src/settings/core/confRuntime.ts`、`ConfTypeRegistry` 模块增强）**已经泛化**，不构成足迹。

### 2.2 同构样板实测

四个 group 的公开方法集合完全相同：

| 文件 | 行数 | 同构方法块 | 测试文件行数 |
|---|---|---|---|
| `src-tauri/crates/assistant-config/src/app_conf.rs` | 208 | `get_conf_dir` / `get_conf_path` / `load` / `load_from` / `save` / `save_to` / `validate` / `amend` + `impl Default` | `tests/app_conf.rs` 303 |
| `src-tauri/crates/assistant-config/src/orb_conf.rs` | 102 | 同上 | `tests/orb_conf.rs` 233 |
| `src-tauri/crates/assistant-config/src/mascot_conf.rs` | 122 | 同上 | `tests/mascot_conf.rs` 145 |
| `src-tauri/crates/assistant-config/src/vault_conf.rs` | 273 | 同上 + `migrate_from_app_conf` | `tests/vault_conf.rs` 284 |

写入侧同样有 4 份副本：`src-tauri/src/cmd/config.rs` 的 `apply_app_conf` / `apply_orb_conf` / `apply_mascot_conf`，以及 `src-tauri/src/cmd/vault/mod.rs` 的 `apply_conf`，四者都是 `load → amend → validate → save → side effects` 的 5 行体。

### 2.3 三个可观测后果

| 后果 | 现状证据 |
|---|---|
| 关闭态编译残留 | `vault_conf.rs` 273 行与其 284 行测试在 `--no-default-features` 下照样编译运行 |
| 关闭态契约残留 | `src/types/protocol.contract.ts` 第 323–324 行导出 `VaultCipherAlgorithmDto` / `VaultConfDto`，而 `vault_get_conf` 命令在关闭态不存在，零消费者 |
| 注册副作用在工厂函数里 | `vault_plugin::init()` 在构造插件对象时就写全局注册表，而 `feature_mounts` 已提供 `on_setup` 生命周期钩子 |

### 2.4 与既有规格的边界

`feature-mount-traversal-spec.md` §6.1 把 `src-tauri/src/protocol/dto/config.rs` 列为「本期保留足迹」，理由是该规格不处理配置归属。本规格替代该判断：该项在实施后归零。`vault-optional-build.md` §4.6「配置层取舍」的四条结论（保留 `assistant-config::vault_conf`、保留 `VaultConfDto`、保留 `get_vault_conf`、保留 `patch_conf` 的 `"vault"` 分支）中，前两条被本规格推翻，后两条继续保持。

---

## 3 终态结构

### 3.1 目录

```
src-tauri/crates/assistant-config/src/
├── conf_group.rs        ← 新增：ConfGroup trait + LoadPolicy + apply_patch（纯机制，无特性名）
├── json_config.rs       ← 改：新增 load_conf（唯一加载生命周期），删除两个宽松加载函数
├── atomic_write.rs      ← 不变
├── constant.rs          ← 不变：仅保留 core 常量
├── shortcut.rs          ← 不变
├── app_conf.rs          ← 收窄：schema + impl ConfGroup + 一行 shims 宏（删 7 个样板方法）
├── orb_conf.rs          ← 同上
├── mascot_conf.rs       ← 同上
└── (vault_conf.rs 删除)

src-tauri/crates/assistant-vault/src/
├── conf.rs              ← 新增：VaultConf + VaultCipherAlgorithm + impl ConfGroup
├── dto/conf.rs          ← 新增：VaultConfDto + VaultCipherAlgorithmDto（export_to = "vault.contract.ts"）
└── dto/convert.rs       ← 追加：From<VaultConf> / From<VaultCipherAlgorithm>（同 crate，满足孤儿规则）
```

### 3.2 机制与策略边界

| 层 | 拥有什么 | 禁止什么 |
|---|---|---|
| `assistant-config`（机制） | 加载策略、原子写、临时文件恢复、`amend` 严格性、生命周期顺序 | 出现任何特性名、特性字段、特性校验规则 |
| 特性 crate（策略） | schema、默认值、`validate`、旧键接管、文件名、wire 键名 | 直接调用 `atomic_write` / `serde_json` 读写（必须走 trait 的 provided 方法） |
| 主应用（装配） | `CONF_APPLIERS` 注册表、`patch_conf` 分发、`CONF_LOCK` | 出现任何 group 的 schema 或字段名 |

### 3.3 依赖边

新增 1 条：`assistant-vault → assistant-config`（基础设施叶子 ← 领域层，方向向下，无环）。`[workspace.dependencies]` 已有 `assistant-config = { path = "crates/assistant-config" }`，只需在 `assistant-vault/Cargo.toml` 加一行 `assistant-config = { workspace = true }`，根 `Cargo.toml` 不改。

逐项影响：

| 维度 | 影响 | 说明 |
|---|---|---|
| 环 | 无 | `assistant-config` 的 `[dependencies]` 只有 `serde` / `serde_json` / `serde_ignored` / `log` / `dirs`，内部依赖 0 条，不指向任何 workspace crate |
| 现有消费者 | 从 1 个变 2 个 | 现状全仓只有根包 `assistant-app` 依赖 `assistant-config`（且非 optional）；本改造使 `assistant-vault` 成为**第一个子 crate 消费者** |
| 编译成本 | 近似 0 | `assistant-config` 已在依赖图中（根包非 optional 依赖），新增边不引入任何第三方 crate，不增加编译单元。增量编译上，改 config 只连带 vault，而 config 仅 8 文件 |
| 关闭态 | 不变 | `assistant-vault` 是 `optional`（`dep:assistant-vault`），关闭 vault 时该边随 crate 一起从图中消失；`assistant-config` 仍由根包编译，与现状一致 |
| API 稳定性 | 升高（约束变硬） | `ConfGroup` / `apply_patch` / `load_conf` 从「主应用内部实现」升级为跨 crate 公共契约，§4 的 R1–R11 从文档约定变成编译期与门禁约束 |
| 能力泄露 | 存在，但只有软约束 | vault 技术上可 `use assistant_config::app_conf::AppConf`。R9 禁止跨组引用，但没有门禁；建议在 B2 同族断言中追加：`assistant-vault/src/` 不出现 `app_conf` / `orb_conf` / `mascot_conf` |
| 门禁 | a8 必挂，需同批改 | `assembly_contract::a8_conf_applier_groups_match_config_structs` 的 expected 来自 `assistant-config/src/lib.rs` 的 `pub mod *_conf;`；`vault_conf` 删除后 expected 少 `vault`，而 `vault_plugin.rs` 仍注册 `"vault"`，`unknown_group` 非空 → 断言失败。详见 §6.1 |
| 测试路径 | 变简单 | `assistant-vault/tests/conf_keys.rs` 现跨 crate 读 `../assistant-config/src/vault_conf.rs`，搬家后为同 crate 的 `../assistant-vault/src/conf.rs`；`tests/conf.rs` 用普通 dependency 即可，不需要额外 dev-dependency |
| 文档 | 2 处计数需手工改 | `../../CODE-STRUCTURE.md` 的「共 5 条」→ 6、依赖图 `assistant-vault → assistant-crypto, assistant-sync` 补 `assistant-config`；`../../store/crate-architecture.md` 的「内部依赖边共 5 条」→ 6、「其余 9 个 crate 均为叶子」→ 8。无门禁，见 §7 |

禁止新增的边：`assistant-config → assistant-vault`（会形成环）；`assistant-app` 为获取 `VaultConf` 而新增的任何间接依赖（保持现状即可，`assistant-app` 本就同时依赖两者）。

不变式（守住即可保持无环）：`assistant-config` 的 `[dependencies]` 永不出现任何 workspace crate。

### 3.4 DTO 生成链路变化

`VaultConfDto` / `VaultCipherAlgorithmDto` 在迁移中**跨过两条独立的契约流水线**，这是本改造唯一会让生成物发生变化的部分：

| 维度 | 迁移前 | 迁移后 |
|---|---|---|
| 类型定义位置 | `src-tauri/src/protocol/dto/config.rs`（`assistant-app` crate，与三个核心 conf 的 DTO 同文件） | `src-tauri/crates/assistant-vault/src/dto/conf.rs`（`assistant-vault` crate，与其余 vault 业务 DTO 同目录） |
| 生成触发方式 | `tests/protocol_bindings.rs::export_protocol_contract` 内手工 `decl(&cfg)` 注册，拼接后 `fs::write` | `#[ts(export, export_to = "vault.contract.ts")]`，由 ts-rs 自动生成 `export_bindings_*` 测试 |
| 生成命令 | `cargo test -p assistant-app --test protocol_bindings`（`contract:gen` 第二条） | `cargo test -p assistant-vault --lib export_bindings`（`contract:gen` 第一条，改造前已在执行） |
| 输出文件 | `src/types/protocol.contract.ts` | `src/vault/core/contract/vault.contract.ts` |
| 注册一致性保障 | 双向断言：`ts_derived_dto_names()`（扫描 `src/protocol/dto/*.rs` 的 `#[derive(TS)]`）必须与 `declarations` 逐名相等，漏注册与多注册均失败 | 无注册清单；漏写 `#[ts(export)]` 时生成物不出现该类型，缺失在生成物 diff 中直接可见 |
| Rust 侧消费 | `crate::protocol::VaultConfDto` | `assistant_vault::dto::VaultConfDto` |
| 前端消费 | `@/types/protocol.contract` | `@/vault/core/vault.types`（barrel 的 `export type { VaultConfDto as VaultConf }`） |
| 字段覆盖保障 | `tests/protocol_dto_roundtrip.rs::config_dtos_cover_every_config_field` | `assistant-vault/tests/dto_contract.rs`（B4），主契约分支删除 |
| 类型渲染配置 | `Config::from_env().with_large_int("number")` | ts-rs 默认 `Config`，从 `.cargo/config.toml` 的 `[env]` 读 `TS_RS_EXPORT_DIR = src/vault/core/contract` 与 `TS_RS_LARGE_INT = number`，两侧对 `i64` 均输出 `number` |

约束：

- 迁移后输出必须与迁移前**逐字节等价**（除文件头注释与相邻类型顺序）。`VaultConfDto` 的 `Option` 字段在输出侧键恒在、值 `null`，故不加 `skip_serializing_if`，`VaultCipherAlgorithmDto` 保持 `"aes256Gcm" | "chacha20Poly1305"` 字面量联合。
- `contract:gen` 的两条命令都直接 `-p` 选包，不经过 `assistant-app` 的 feature 开关，因此 vault 关闭态下 `vault.contract.ts` 仍会被重新生成。关闭态的收益是**主契约不再出现 vault 类型**（G3）与**后端 schema 不参与编译**（G2），不是「生成物消失」——该文件是入库生成物，`contract:check` 的 TARGETS 要求它始终存在。
- 迁移必须在同一批内完成「删主契约注册 + 加 vault 侧 export + 重生成」，三者缺一即 `contract:check` 失败（`protocol_bindings.rs` 的漏注册/多注册断言与生成物哈希会同时报错）。

---

## 4 `ConfGroup` 契约

### 4.1 契约条目

| 编号 | 契约 | 理由 |
|---|---|---|
| R1 | 关联常量 `GROUP: &'static str`（wire 键，值 `app` / `orb` / `mascot` / `vault`），必须与所属文件名对应（核心组为 `*_conf.rs` 的模块名，特性组为 `conf.rs` 的所在 crate）；`register_conf_applier` 的调用点**继续写字面量**，由 a8 断言字面量集合 ⊆ `GROUP` 常量集合 | `assembly_contract::a8` 的解析器只识别 `register_conf_applier(` 后紧跟的引号字面量，改传常量会让解析落空、断言全量误报；`GROUP` 提供机器可读的组名来源，供 a8 发现特性组（见 §6.1） |
| R2 | 关联常量 `LOG_TAG: &'static str`，值必须与现状日志前缀逐字相同（`AppConf` / `OrbConf` / `MascotConf` / `VaultConf`） | 不设默认值，强制显式声明，避免日志前缀被静默改写 |
| R3 | 关联常量 `FILE_NAME: &'static str`（`config.json` / `orb-config.json` / `mascot-config.json` / `vault-config.json`） | 文件名属于特性策略 |
| R4 | 关联常量 `LOAD_POLICY: LoadPolicy`，三态**必须**保留：`Strict` / `Lenient` / `LenientWithMigrate` | 三者语义相反，见 §4.3；合并任何一个都会改变行为 |
| R5 | `fn adopt_legacy(path: &Path) -> Option<Self>`，默认 `None`；仅在目标文件不存在时调用 | `VaultConf::migrate_from_app_conf` 读的是**另一个 group 的文件**（`config.json`），无法用 `migrate(self)` 表达 |
| R6 | `fn migrate(self) -> Self`，默认恒等；仅 `LenientWithMigrate` 使用 | `AppConf` 的迁移窗口机制 |
| R7 | `fn validate(&self) -> Result<(), String>`，默认 `Ok(())`；调用时机由 provided 方法决定，实现方不得自行调用 | 见 §4.3 的时机表 |
| R8 | `amend` 必须走 `amend_json_config`，禁止换成 `serde_json` 浅合并 | 前者用 `serde_ignored` 拒绝未知键，是「前端传错键立即失败」的唯一护栏 |
| R9 | 一个 group 不得引用另一个 group 的类型或常量；跨组一次性接管只能经 `adopt_legacy` 读文件，不得构造对方类型 | 避免 `ConfGroup` 之间形成依赖环 |
| R10 | 每个 group 保留同名的 inherent 方法（`get_conf_dir` / `get_conf_path` / `load` / `load_from` / `save` / `save_to` / `validate` / `amend`），由 `conf_group_shims!` 宏一次性生成并委托到 trait 方法 | 全仓 40+ 调用点以 `X::load()`、`conf.amend(patch)` 形式调用；不做 inherent 委托就必须给约 12 个文件补 `use assistant_config::ConfGroup;`，G4 的「调用形态不变」也随之失守 |
| R11 | 实现体（`validate` / `migrate` / `adopt_legacy` 内）不得调用任何同名方法，必须直接写逻辑 | inherent 方法与 trait 方法同名，`self.validate()` 与 `Self::validate(self)` 都会命中 inherent 委托并回到 trait 实现，形成无限递归 |

### 4.2 参考实现

```rust
// src-tauri/crates/assistant-config/src/conf_group.rs
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

use super::json_config::{amend_json_config, get_conf_dir as conf_dir_root, load_conf, save_json_config};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadPolicy {
    /// 损坏即报错，且不写回用户文件。
    Strict,
    /// 损坏回落默认值并写回。
    Lenient,
    /// 同 `Lenient`，加载后跑 `migrate`。
    LenientWithMigrate,
}

pub trait ConfGroup: Serialize + DeserializeOwned + Clone + PartialEq + Default + Sized {
    const GROUP: &'static str;
    const LOG_TAG: &'static str;
    const FILE_NAME: &'static str;
    const LOAD_POLICY: LoadPolicy;

    fn adopt_legacy(_path: &Path) -> Option<Self> {
        None
    }

    fn migrate(self) -> Self {
        self
    }

    fn validate(&self) -> Result<(), String> {
        Ok(())
    }

    fn get_conf_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
        conf_dir_root()
    }

    fn get_conf_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        Ok(Self::get_conf_dir()?.join(Self::FILE_NAME))
    }

    fn load() -> Result<Self, Box<dyn std::error::Error>> {
        Self::load_from(&Self::get_conf_path()?)
    }

    fn load_from(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        load_conf(
            path,
            Self::LOAD_POLICY,
            Self::default,
            Self::migrate,
            Self::adopt_legacy,
            Self::validate,
            Self::LOG_TAG,
        )
    }

    fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.save_to(&Self::get_conf_path()?)
    }

    fn save_to(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        save_json_config(path, self)
    }

    fn amend(self, patch: Value) -> Result<Self, String> {
        amend_json_config(&self, patch)
    }
}

/// 四个 group 共用的写入序列，顺序固定：load → amend → validate → save。
pub fn apply_patch<C: ConfGroup>(patch: Value) -> Result<(C, C), String> {
    let prev = C::load().map_err(|e| e.to_string())?;
    let next = prev.clone().amend(patch)?;
    next.validate()?;
    next.save().map_err(|e| e.to_string())?;
    Ok((prev, next))
}

/// 为 group 生成与 trait 方法同名的 inherent 委托方法，保持既有调用点不变（R10）。
#[macro_export]
macro_rules! conf_group_shims {
    ($t:ty) => {
        impl $t {
            pub fn get_conf_dir() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
                <$t as $crate::ConfGroup>::get_conf_dir()
            }
            pub fn get_conf_path() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
                <$t as $crate::ConfGroup>::get_conf_path()
            }
            pub fn load() -> Result<$t, Box<dyn std::error::Error>> {
                <$t as $crate::ConfGroup>::load()
            }
            pub fn load_from(path: &std::path::Path) -> Result<$t, Box<dyn std::error::Error>> {
                <$t as $crate::ConfGroup>::load_from(path)
            }
            pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
                <$t as $crate::ConfGroup>::save(self)
            }
            pub fn save_to(&self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
                <$t as $crate::ConfGroup>::save_to(self, path)
            }
            pub fn validate(&self) -> Result<(), String> {
                <$t as $crate::ConfGroup>::validate(self)
            }
            pub fn amend(self, patch: serde_json::Value) -> Result<$t, String> {
                <$t as $crate::ConfGroup>::amend(self, patch)
            }
        }
    };
}
```

每个 group 的写法（以 `VaultConf` 为例，`AppConf` / `OrbConf` / `MascotConf` 同构）：

```rust
impl ConfGroup for VaultConf {
    const GROUP: &'static str = "vault";
    const LOG_TAG: &'static str = "VaultConf";
    const FILE_NAME: &'static str = "vault-config.json";
    const LOAD_POLICY: LoadPolicy = LoadPolicy::Strict;

    fn adopt_legacy(path: &Path) -> Option<Self> {
        let legacy_path = path.parent()?.join("config.json");
        let contents = std::fs::read_to_string(legacy_path).ok()?;
        let value: serde_json::Value = serde_json::from_str(&contents).ok()?;
        serde_json::from_value(value).ok()
    }

    fn validate(&self) -> Result<(), String> {
        // 现状 validate 的四个子校验原样搬入，不得改动规则与文案
    }
}

assistant_config::conf_group_shims!(VaultConf);
```

```rust
// src-tauri/crates/assistant-config/src/json_config.rs —— 新增的唯一加载生命周期实现
pub fn load_conf<T, D, M, A, V>(
    path: &Path,
    policy: LoadPolicy,
    default: D,
    migrate: M,
    adopt: A,
    validate: V,
    tag: &str,
) -> Result<T, Box<dyn std::error::Error>>
where
    T: DeserializeOwned + Serialize,
    D: FnOnce() -> T,
    M: Fn(T) -> T,
    A: FnOnce(&Path) -> Option<T>,
    V: Fn(&T) -> Result<(), String>,
{
    if let Some(tmp_bytes) = recover_temp(path) {
        if let Ok(config) = serde_json::from_slice::<T>(&tmp_bytes) {
            let config = if policy == LoadPolicy::LenientWithMigrate { migrate(config) } else { config };
            if policy == LoadPolicy::Strict { validate(&config)?; }
            save_json_config(path, &config)?;
            return Ok(config);
        }
        error!("[{}] temp file invalid, discarding", tag);
    }

    if !path.exists() {
        let config = match adopt(path) {
            Some(legacy) => { if policy == LoadPolicy::Strict { validate(&legacy)?; } legacy }
            None => default(),
        };
        save_json_config(path, &config)?;
        return Ok(config);
    }

    if policy == LoadPolicy::Strict {
        let config: T = load_json_config_strict(path, tag)?;
        validate(&config)?;
        return Ok(config);
    }

    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    match serde_json::from_str::<T>(&contents) {
        Ok(config) if policy == LoadPolicy::LenientWithMigrate => {
            let migrated = migrate(config);
            if contents != serde_json::to_string_pretty(&migrated)? {
                save_json_config(path, &migrated)?;
            }
            Ok(migrated)
        }
        Ok(config) => Ok(config),
        Err(e) => {
            error!("[{}] parse error: {}, falling back to defaults", tag, e);
            let fallback = default();
            save_json_config(path, &fallback)?;
            Ok(fallback)
        }
    }
}
```

`load_json_config_strict`（只解析、不写回）与 `amend_json_config`、`save_json_config`、`get_conf_dir` 保留不变；`load_json_config` 与 `load_json_config_with_migrate` 被 `load_conf` 完全吸收后删除（全仓仅四个 group 调用，无其它消费者）。

### 4.3 加载分支必须逐字对齐

`load_conf` 必须复现下表行为，任何一项偏差都属违约（表内顺序即代码顺序）：

| # | 分支 | 现状行为 | 策略差异 |
|---|---|---|---|
| 1 | 临时文件 | `recover_temp(path)` 命中且能反序列化 → 写回 → 返回；反序列化失败 → `log::error!("[<LOG_TAG>] temp file invalid, discarding")` 后继续 | `Strict` 额外先 `validate`；`LenientWithMigrate` 额外先 `migrate` |
| 2 | 文件不存在 | 先试 `adopt_legacy(path)`：命中则写回 → 返回；未命中则 `default()` → 写回 → 返回 | `Strict` 在接管命中时额外先 `validate` |
| 3a | `Strict` 且文件存在 | `load_json_config_strict(path, LOG_TAG)` → `validate` → 返回；**损坏时报错、不写回** | — |
| 3b | `Lenient` 且文件存在 | 读文件 → 解析成功直接返回（不写回、不校验）；解析失败 → `log::error!("[<LOG_TAG>] parse error: {}, falling back to defaults")` → 写回默认值 → 返回 | — |
| 3c | `LenientWithMigrate` 且文件存在 | 读文件 → 解析成功后 `migrate`，仅当 `contents != to_string_pretty(migrated)` 时写回；解析失败同上 | — |

约束说明：

- 校验只在 `Strict` 下发生（含分支 1、2 的 `Strict` 情形），`Lenient` / `LenientWithMigrate` 加载期不校验。这是现状事实，不得"顺手补上"：`AppConf` 的 `approval_timeout_secs == Some(0)` 之类若在加载期强制，会让已存在的旧文件变成启动失败。写入路径（§4.2 `apply_patch`）对所有策略都校验。
- 分支 1 的 `Strict` 校验顺序（校验 → 写回）必须保持：临时文件若带越界值，现状是返回错误而非丢弃并回退。
- `adopt_legacy` 排在分支 1 之后、默认值写入之前，保证「有临时文件时优先恢复临时文件」不被接管逻辑抢占。
- 现状 `VaultConf::load_from` 的接管分支用 `legacy.save_to(path)` 而非 `save_json_config`，语义等价，统一走 `save_json_config`。

### 4.4 已评估不采纳的契约变体

| 变体 | 收益 | 未采纳理由 |
|---|---|---|
| 把 `FILE_NAME` 改为由 `GROUP` 派生（`format!("{}-config.json")`） | 少一个常量 | `app` 的文件名是 `config.json`（无前缀），派生规则会出现特例，反而更绕 |
| 用 `const fn default() -> Self` 替代 `Default` trait | 避免 `Default` 与既有 `new()` 并存 | `load_conf` 的 `default` 参数是 `FnOnce() -> T`，函数指针需要 `Self::default`；引入 `new` 与 `default` 双入口正是当前样板重复的来源之一 |
| 让 `apply_patch` 直接接收 `AppHandle` 并回调 side effects | 进一步收敛 4 份 5 行体 | `assistant-config` 是纯基础设施叶子，不得依赖 `tauri`；side effects 必须留在主应用侧 |
| 用泛型 `apply_patch::<C>` 取代 4 个 `apply_*_conf` 函数 | 少 4 个函数 | 4 个函数各自还要跑不同的 side effects（主题广播、窗口属性、托盘重建等），保留它们作为命名入口更可读；其函数体统一收缩为「`apply_patch` + 自己的 side effects」 |
| 让 `validate` 在所有策略的加载期都调用 | 语义更统一 | 会把「已存在的越界旧文件」从「可加载」变成「启动失败」，属行为变更，违反 G4 |

---

## 5 改造步骤

按 §10 的四批执行；每批独立可验证，不必一次做完。

### S1 新增机制层

新增 `src-tauri/crates/assistant-config/src/conf_group.rs`（内容见 §4.2），并在 `json_config.rs` 新增 `load_conf`（内容见 §4.2），删除 `load_json_config` 与 `load_json_config_with_migrate`（其逻辑已被 `load_conf` 完全吸收，全仓无其它消费者）。`src-tauri/crates/assistant-config/src/lib.rs` 增加 `pub mod conf_group;` 与 `pub use conf_group::{apply_patch, ConfGroup, LoadPolicy};`。

### S2 四个 group 原地 impl（行为零变化）

对 `app_conf.rs` / `orb_conf.rs` / `mascot_conf.rs` / `vault_conf.rs` 各自：

- 删除各自的 `get_conf_dir` / `get_conf_path` / `load` / `load_from` / `save` / `save_to` / `amend` 方法体，改为一行宏调用（生成同名 inherent 委托方法，方法名与调用形态不变，因为既有测试与主应用以 `X::get_conf_dir()`、`X::load_from(&path)`、`conf.amend(patch)` 等形式直接调用，且不需要 `use ConfGroup`）。宏在 `assistant-config` 内部调用须写全路径 `crate::conf_group_shims!(X);`（`#[macro_export]` 只把宏导出到 crate 根，跨模块裸名不可见）；特性 crate 内调用写 `assistant_config::conf_group_shims!(X);`；
- 改为 `impl ConfGroup for X`，只写四个关联常量与 `validate`（`AppConf` 另写 `migrate`，`VaultConf` 另写 `adopt_legacy`）；实现体内不得调用同名方法（R11）；
- 保留 `pub fn new() -> Self` 与 `impl Default`（`Default::default()` 内部调 `new()`）。实测 `new()` 只被 `assistant-config/tests/*` 与 `src-tauri/tests/protocol_dto_roundtrip.rs` 调用，主应用侧（`src-tauri/src/`）无调用点；改造后默认值来源从 `Self::new` 改为 `Self::default`，两入口语义恒等。

约束：本批**不得**改动 `validate` 内部逻辑、错误文案、字段顺序、serde 属性。`assistant-config/tests/` 下的既有测试必须全部原样通过（它们只通过 `load_from` / `save_to` / `amend` 等公开方法观察行为，方法名不变）。

### S3 收敛写入序列

`src-tauri/src/cmd/config.rs`：

```rust
pub(crate) fn apply_app_conf(app: &AppHandle, patch: serde_json::Value) -> Result<(), String> {
    let (prev, next) = assistant_config::apply_patch::<AppConf>(patch)?;
    apply_app_side_effects(app, &prev, &next);
    Ok(())
}
```

`apply_orb_conf` / `apply_mascot_conf` 同理（`apply_mascot_conf` 当前无 side effects，改造后函数体为两行，保留函数以维持 `conf_side_effects()` 注册表结构）。

`src-tauri/src/cmd/vault/mod.rs` 的 `apply_conf` 同样收敛：

```rust
pub fn apply_conf(_app: &tauri::AppHandle, patch: serde_json::Value) -> Result<(), String> {
    let (prev, next) = assistant_config::apply_patch::<VaultConf>(patch)?;
    apply_vault_side_effects(&prev, &next);
    Ok(())
}
```

`CONF_LOCK` 的持锁范围不变：仍由 `patch_conf` 与 `update_app_conf` 在外层持锁。side effects 一律在 `apply_patch` 返回后执行，与现状「先 save 再 side effects」的顺序一致。

`register_conf_applier` 的 group 字面量**保持不变**（见 R1）：`assembly_contract::a8` 按字面量解析调用点，改成关联常量会让该断言解析为空、全量误报。`GROUP` 常量只用于 a8 发现特性组与自校验，不进入调用点。

### S4 vault schema 迁入 vault crate

- 新增 `src-tauri/crates/assistant-vault/src/conf.rs`：搬 `VaultConf` / `VaultCipherAlgorithm` / `MAX_AUTO_LOCK_MINUTES` / `MAX_CLIPBOARD_CLEAR_SECONDS` / 全部 `default_*` 函数 / `migrate_from_app_conf` / `is_absolute_local_path` / `is_scp_style_remote` / `is_supported_remote_url`，改为 `impl ConfGroup`：

| 关联常量 | 值 |
|---|---|
| `GROUP` | `"vault"` |
| `LOG_TAG` | `"VaultConf"` |
| `FILE_NAME` | `"vault-config.json"` |
| `LOAD_POLICY` | `LoadPolicy::Strict` |

- 在 `conf.rs` 末尾加一行 `assistant_config::conf_group_shims!(VaultConf);`（生成 R10 要求的 inherent 委托方法）。
- `src-tauri/crates/assistant-vault/src/lib.rs` 增加 `pub mod conf;`。
- `src-tauri/crates/assistant-vault/Cargo.toml` 增加 `assistant-config = { workspace = true }`（`[workspace.dependencies]` 已有该 path，无需改 `src-tauri/Cargo.toml`）。
- 删除 `src-tauri/crates/assistant-config/src/vault_conf.rs` 与 `src-tauri/crates/assistant-config/src/lib.rs` 的 `pub mod vault_conf;`。

约束：`vault_conf.rs` 里出现的 `MAX_AUTO_LOCK_MINUTES` / `MAX_CLIPBOARD_CLEAR_SECONDS` 仅被 `validate` 使用，随 schema 一起搬；`constant.rs` 不动。

### S5 引用改路径

8 个文件里的 `assistant_config::vault_conf::` 改为 `assistant_vault::conf::`（另有 1 个 `assistant-config/tests/vault_conf.rs` 随 S6 整体搬走）：

| 文件 | 形态 |
|---|---|
| `src-tauri/src/cmd/vault/mod.rs` | `use assistant_config::vault_conf::VaultConf;` → `use assistant_vault::conf::VaultConf;` |
| `src-tauri/src/cmd/vault/conf.rs` | 同上 |
| `src-tauri/src/cmd/vault/recovery.rs` | 同上 |
| `src-tauri/src/cmd/vault/sync.rs` | 内联全路径 `assistant_config::vault_conf::VaultConf::load()` |
| `src-tauri/src/cmd/vault/ssh.rs` | 同上 |
| `src-tauri/src/vault_plugin.rs` | 启用块内 `on_setup` 的 `VaultConf::load()` |
| `src-tauri/src/protocol/dto/config.rs` | 两个 `impl From` 的源类型路径。**本步骤只改路径**（`assistant_config::vault_conf::` → `assistant_vault::conf::`）；类型删除留到 S7 |
| `src-tauri/tests/protocol_dto_roundtrip.rs` | `config_dtos_cover_every_config_field` 的 vault 分支构造路径（`assistant_config::vault_conf::VaultConf::new()` → `assistant_vault::conf::VaultConf::new()`）。**本步骤只改路径**；分支删除留到 S7 |

上表 8 个文件必须在 S4 删除 `assistant-config::vault_conf` 的同一步完成路径替换：删模块与改引用之间没有可编译的中间态，S4 与 S5 不可拆分提交。

`src-tauri/src/cmd/vault/mod.rs` 的 `c.vault_cipher_algorithm.as_str()` 是方法调用，类型搬家后无需新增导入。

### S6 测试搬家

- `src-tauri/crates/assistant-config/tests/vault_conf.rs`（284 行）整体搬到 `src-tauri/crates/assistant-vault/tests/conf.rs`，`use assistant_config::vault_conf::{...}` 改为 `use assistant_vault::conf::{...}`，断言内容不变。
- `src-tauri/crates/assistant-vault/tests/conf_keys.rs` 的 `vault_conf_source()` 路径从 `../assistant-config/src/vault_conf.rs` 改为 `../assistant-vault/src/conf.rs`（`src_root()` 与命令扫描目录 `../../src/cmd/vault` 不变）。
- `src-tauri/crates/assistant-vault/tests/dto_contract.rs` 追加 vault 配置的 wire 键冻结断言（见 B4）。

约束：`conf_keys.rs` 比对的是 `src/cmd/vault/*.rs` 里的 `.vault_<name>` 访问与 `conf.rs` 里的 `pub vault_<name>` 声明，搬家后两侧仍在同一 crate 可访问的相对路径上，不得改成绝对路径。

### S7 DTO 迁入 vault 契约域

- 新增 `src-tauri/crates/assistant-vault/src/dto/conf.rs`：搬 `VaultCipherAlgorithmDto` / `VaultConfDto`，两者都加 `#[ts(export, export_to = "vault.contract.ts")]`；serde 属性与字段顺序原样保留（`VaultConfDto` 现状带 `deny_unknown_fields`，保持不变）。
- `src-tauri/crates/assistant-vault/src/dto/mod.rs` 增加 `pub mod conf;` 与 `pub use conf::{VaultCipherAlgorithmDto, VaultConfDto};`。
- `src-tauri/crates/assistant-vault/src/dto/convert.rs` 增加两个 `From` 实现（源类型与目标类型同 crate，孤儿规则满足）。
- `src-tauri/src/protocol/dto/config.rs` 删除该枚举、该结构体与两个 `impl From`，只留 `AppConfDto` / `OrbConfDto` / `MascotConfDto` 及其 `From`。
- `src-tauri/src/cmd/vault/conf.rs` 的 `use crate::protocol::VaultConfDto;` 改为 `use assistant_vault::dto::VaultConfDto;`。
- `src-tauri/tests/protocol_bindings.rs` 删除 import 中的两个名字与两行 `decl`（否则 `ts_derived_dto_names()` 的「注册清单无冗余」断言必挂）。
- `src-tauri/tests/protocol_dto_roundtrip.rs` 删除 vault 分支与对应 import；`assert_dto_covers_config` helper 保留，继续服务三个核心 conf。
- 前端：`src/vault/core/vault.types.ts` 的别名清单（现第 10–32 行，按 DTO 名升序）追加两行，位置在 `UnreadableEntriesDto` 之后：
  ```ts
  export type { VaultCipherAlgorithmDto as VaultCipherAlgorithm } from "./contract/vault.contract";
  export type { VaultConfDto as VaultConf } from "./contract/vault.contract";
  ```
  该文件第 8 行已有 `export * from "./contract/vault.contract";`，新增两行只提供别名，与其余 23 行同构。
- 前端：`src/features/vault/conf.ts` 的 `import type { VaultConfDto } from "@/types/protocol.contract";` 改为 `import type { VaultConf } from "@/vault/core/vault.types";`，并删除 `export type VaultConf = VaultConfDto;`（别名已由 barrel 提供，保留会造成同名二次声明）。`ConfTypeRegistry` 的 `vault: VaultConf` 与 `vaultConfSlot` 的 `satisfies ConfSlot<"vault">` 均不变。
- 跑 `npm run contract:gen` 重生成两份契约，一并提交。

约束：`VaultConfDto` 是**输出 DTO**（`vault_get_conf` 的返回值），保持 `Option` 字段键恒在、值 `null`，不加 `skip_serializing_if`；前端声明 `string | null` 必填，与现状一致。

### S8 前端类型源收口

已在 S7 内完成。此步骤单列仅为便于验收：改完后 `src/features/vault/**` 对 `@/types/protocol.contract` 的引用必须为 **0**。

### S9 文档同步

见 §7。

### 5.9 可选步骤（默认不执行）

| 编号 | 内容 | 收益 | 不默认执行的理由 |
|---|---|---|---|
| O1 | 把 `register_conf_applier("vault", ...)` 从 `vault_plugin::init()` 移到 `vault_plugin::on_setup()` | 插件工厂不再写全局注册表；注册点与配置加载同处 | 与 `feature-mount-traversal-spec.md` §4 S2「`init()` 内部保留 `register_conf_applier`」冲突，采纳需同步修订该规格；且注册早于首次 `patch_conf` 即可，`conf_side_effects()` 的 `call_once` 惰性播种已保证安全，收益是纯度而非正确性 |
| O2 | 为 `ConfGroup` 写一份 conformance 测试套件（每个 group 一行 opt-in，复用 `Default → save_to(temp) → load_from(temp) → 相等`、`amend` 未知键必错、`validate` 拒绝非法值三条） | `assistant-config/tests/` 约 1150 行重复断言可收缩 | 现有测试全部通过即满足 G4；收缩测试会削弱按 group 定制断言的自由度，收益是维护成本而非正确性 |

---

## 6 门禁改造

### 6.1 既有断言改动

| 文件 | 现状 | 改动 |
|---|---|---|
| `src-tauri/tests/protocol_bindings.rs` | 第 33 行 import 含 `VaultCipherAlgorithmDto, VaultConfDto`；第 210–211 行两行 `decl` | 删 import 两名字与两行 `decl`。`ts_derived_dto_names()` 扫描 `src-tauri/src/protocol/dto/*.rs`，删除后两侧同时收缩，断言口径不变 |
| `src-tauri/tests/protocol_dto_roundtrip.rs` | `config_dtos_cover_every_config_field` 覆盖 4 个 conf | 删除 vault 分支与 import；helper `assert_dto_covers_config` 保留 |
| `src-tauri/crates/assistant-vault/tests/conf_keys.rs` | `vault_conf_source()` 指向 `../assistant-config/src/vault_conf.rs` | 指向 `../assistant-vault/src/conf.rs` |
| `src/features/__tests__/coreBoundary.test.ts` | `FEATURE_FOOTPRINT_ALLOWLIST.vault` 为 3 项，含 `src-tauri/src/protocol/dto/config.rs` | 收缩为 2 项（`src-tauri/src/cmd/mod.rs`、`src-tauri/src/protocol/events.rs`）；只减不增 |
| `scripts/contract-check.mjs` | 比对 `src/types/protocol.contract.ts` 与 `src/vault/core/contract` | 不改；S7 后必须重生成一次，否则 `contract:check` 失败 |
| `src-tauri/tests/assembly_contract.rs` | `a8_conf_applier_groups_match_config_structs` 的 expected 集合来自 `assistant-config/src/lib.rs` 的 `pub mod *_conf;`；`conf_applier_groups()` 从 `cmd/mod.rs` 与 `*_plugin.rs` 提取 `register_conf_applier("X"` 的字面量 | expected 扩为两个来源的并集：核心组取 `assistant-config/src/lib.rs` 的 `*_conf` 模块名，特性组取 `crates/*/src/conf.rs` 的 `GROUP` 常量值（按文件名发现，不枚举特性名）。新增断言：每个 `*_conf.rs` 的 `GROUP` 与模块名逐字相等。`conf_applier_groups()` 不改（调用点仍写字面量，见 R1）。`[features]` 相关断言（b2）不变 |

### 6.2 新增断言

| 编号 | 断言 | 落点 |
|---|---|---|
| B1 | `assistant-config/src/` 下不存在 `vault_conf.rs`；`src/*.rs` 内容不含 `config/features.json` 中任一特性 id（大小写不敏感） | `src-tauri/crates/assistant-config/tests/` 新增结构断言 |
| B2 | 配置机制层不得按特性分叉：`conf_group.rs` / `json_config.rs` 不含 `cfg!(feature = ...)`、`#[cfg(feature = ...)]` 与特性 id 判断 | 同上；与 B1 同文件实现，共用特性 id 读取逻辑 |
| B3 | `src/types/protocol.contract.ts` 不含 `VaultConfDto` / `VaultCipherAlgorithmDto`；`src/vault/core/contract/vault.contract.ts` 含两者 | 新增 vitest 或复用 `contract:check` 的 TARGETS 清单 |
| B4 | `assistant-vault/tests/dto_contract.rs` 冻结 `VaultConfDto` 的 12 个 wire 键（camelCase）与 `VaultConf` 的 12 个磁盘键（snake_case），并断言两侧字段集合一致（helper 从 `protocol_dto_roundtrip.rs` 复制一份到 vault 测试） | 该断言是 S7 删除主契约分支后的**唯一**字段覆盖保障，不可省 |
| B5 | `conf_keys.rs` 追加断言：`conf.rs` 的 `pub vault_*` 声明数 ≥ 12，防止搬家时漏字段 | 现有测试内加一行 |
| B6 | `assistant-vault/src/` 不出现 `app_conf` / `orb_conf` / `mascot_conf` 三个核心组标识（R9 的唯一门禁） | `assistant-vault/tests/conf.rs` 内源码扫描 |
| F1 | `src/features/vault/**` 不出现 `@/types/protocol.contract` | 复用 `coreBoundary.test.ts` 的 `CORE_SCAN_ROOTS` 机制 |
| F2 | `FEATURE_FOOTPRINT_ALLOWLIST.vault` 恰为 2 项，且与实际扫描结果逐字相等 | 现有断言，只改清单 |

### 6.3 门禁口径说明

- B1/B2 取代「靠 `cargo tree` 判断残留」的人工复核：`assistant-config` 是叶子 crate，`cargo tree -i` 在关闭态无输出，无法区分「关闭态仍编译但无消费者」与「真正归零」。B1 覆盖「特性 schema 是否已迁出」，B2 覆盖「机制层是否被特性分支污染」。
- B3 与 B4 是互补的两侧：B3 保证前端契约没有悬空类型，B4 保证后端磁盘格式与 wire 格式仍逐字段对齐。只做 B3 会漏掉「DTO 少一个字段」这类静默错误。
- 所有断言只读文件内容与清单，特性 id 集合来自 `config/features.json`（新增特性时自动纳入扫描），符合 G6。

---

## 7 文档同步

| 文档 | 需改内容 |
|---|---|
| `../../store/vault-optional-build.md` | §2 依赖表第 54 行「`assistant-config` 保留 / `VaultConf` 只是 JSON 读写」改写为「`VaultConf` 已迁入 `assistant-vault/src/conf.rs`」；§3.4 第 109–110 行两行归属改为 vault crate 与 vault 契约域；§4.6 配置层取舍表前两条结论标记为「已被本规格替代」 |
| `../../store/feature-mount-traversal-spec.md` | §1.2 非目标中「不迁移 `protocol/dto/config.rs`」加注「其中配置 DTO 一项已由 `feature-config-ownership-spec.md` 迁出」；§6.1 保留足迹表删除该行，表由 3 项变 2 项；若采纳 O1，§4 S2 的注册点描述同步修订 |
| `../agent/I50-runtime-audit-tasklist.md` | P1.1 落地说明中「`protocol/dto/config.rs` 只留配置 DTO（`VaultConfDto` / `VaultCipherAlgorithmDto`）」改为「配置 DTO 亦在 vault 契约域」 |
| `../../store/crate-architecture.md` | `assistant-config` 职责段去掉 `vault_conf`，补「配置机制库」定位；§1.3 依赖图补 `assistant-vault → assistant-config`，「内部依赖边共 5 条」改 6，「其余 9 个 crate 均为叶子」改 8；§2.1 的「`vault_conf` 只负责读写 vault 配置结构，不引用 vault 的任何代码」整句替换为「只提供配置机制，vault 配置 schema 位于 `assistant-vault/src/conf.rs`」 |
| `../../CODE-STRUCTURE.md` | §4.5 依赖图「共 5 条」改 6，`assistant-vault` 行的依赖补 `assistant-config`；§4.5 职责表 `assistant-config` 的「关键模块」去掉 `vault_conf`；§5「新增设置项」一行改为「核心 group 在 `assistant-config`，特性 group 在特性 crate 的 `conf.rs`」 |
| `../../analysis/FEATURE-OPTIONAL-BINARY-ANALYSIS.md` | 「`FEATURE_FOOTPRINT_ALLOWLIST` 现为 3 个」改为 2 个 |
| `../../analysis/FEATURE-ZERO-COST-MOUNT-ANALYSIS.md` | 足迹 7 → 3 的结论保持，补注「3 项中配置 DTO 一项后续归零」 |
| `.trae/rules/project_rules.md` | 「新增设置项：Rust `assistant-config` struct 与 `protocol/dto` `*ConfDto` 同步加字段」需补一句：特性自有配置组的 struct 位于特性 crate，DTO 位于该特性契约域 |

约束：`docs/` 下只写仓库相对路径，不出现本机绝对路径；`docs/archived/` 不再修改。

---

## 8 验收

| 编号 | 验收内容 | 命令 |
|---|---|---|
| AC1 | 配置机制与核心 group 行为零变化 | `cargo test -p assistant-config` |
| AC2 | vault 配置行为零变化（含 `Strict` 损坏不写回、旧键接管） | `cargo test -p assistant-vault --test conf` |
| AC3 | 字段覆盖与 wire 键冻结 | `cargo test -p assistant-vault --test dto_contract` |
| AC4 | 主契约无 vault 类型且注册清单无冗余 | `cargo test -p assistant-app --test protocol_bindings` |
| AC5 | 特性边界与足迹清单 | `cargo test -p assistant-app --test assembly_contract`；`npx vitest run src/features/__tests__/coreBoundary.test.ts` |
| AC6 | 契约生成物同步 | `npm run contract:gen && npm run contract:check` |
| AC7 | 前端类型与字段定义不变 | `npm run typecheck` |
| AC8 | 关闭态依赖闭包断开 | `cargo tree --no-default-features -i assistant-vault`（cwd = `src-tauri`，无输出） |

人工验收补充：

- `git --no-pager diff src/types/protocol.contract.ts` 只应出现**删除**（两条 vault 类型）。
- `git --no-pager diff src/vault/core/contract/vault.contract.ts` 只应出现**新增**（两条 vault 配置类型）。
- `git --no-pager diff src/settings/__tests__/fixtures/` 应为空（字段定义未变，快照不需要 `vitest -u`）。
- 手动跑一次应用：改一项 vault 设置 → `patch_conf` 成功 → `vault-config.json` 内容与改造前逐字节相同（除时间戳字段）。

---

## 9 已评估不采纳的整体方案

| 方案 | 表面收益 | 未采纳理由 |
|---|---|---|
| 在 `assistant-config` 内加 `#[cfg(feature = "vault")] pub mod vault_conf;` | 关闭态不编译 vault schema，改动最小 | 造出第三个开关：主 `Cargo.toml` 的 `vault` feature 必须额外转发 `assistant-config/vault`，并保证两者永远同开同关。`vault-optional-build.md` 已把「前后端开关不同源」列为风险，`assembly_contract` 的「同一份清单」断言按 `*_plugin.rs` 遍历，看不见子 crate 的 feature，门禁会失效。且它只解决编译残留，不解决 DTO 归属 |
| 合并为单个 `config.json` | 少 3 个文件，`get_conf_dir` 只需一处 | `Strict` 策略失效（vault 损坏会污染主配置加载）；`amend_json_config` 的未知键拒绝会跨组误报；`CONF_LOCK` 退化为全文件锁，一次主题修改会阻塞 vault 写入 |
| 通用 `get_conf(group: String) -> Value` 命令 | 命令数不随 group 增长 | wire 形状与 DTO 之间失去静态校验（B4 无法成立）；`contract:gen` 的生成类型失去唯一来源；ACL 从按命令授权退化为按字符串分发。当前「每 group 一个类型化命令」是正确形态，保持 |
| 用代码生成器产出 `*_conf.rs` 与 DTO | 新特性配置组 0 行样板 | N=1 下生成器成本高于收益，且引入「生成物入库 vs 真删除」的第三态维护负担；`ConfGroup` trait 已经把样板压到「4 个常量 + 可选 3 个钩子」，生成器收益仅剩常量声明 |

---

## 10 迁移批次

| 批次 | 内容 | 回滚成本 | 验证 |
|---|---|---|---|
| B1 | S1 + S2 + S3（抽 trait、四个 group 原地 impl、收敛写入序列） | 低：纯重构，零行为变化 | AC1、AC7 |
| B2 | S4 + S5 + S6 + a8 expected 集合改造（vault schema 与测试搬家） | 中：8 处路径 + 1 条测试路径 + 1 条门禁 | AC2、AC5、AC8 |
| B3 | S7 + S8（DTO 迁入 vault 契约域、前端类型源收口） | 中：两份契约重生成 | AC3、AC4、AC6、AC7 |
| B4 | 门禁改造（§6）与文档同步（§7） | 低 | AC5 |

B1 与 B2 之间无契约生成物变化，可分别独立提交；B3 必须与两份生成物同批提交，否则 `contract:check` 会失败。

B2 必须与 a8 改造同批：删除 `assistant-config/src/lib.rs` 的 `pub mod vault_conf;` 后，a8 的 expected 集合立刻少 `vault`，而 `vault_plugin.rs` 仍注册 `"vault"`，`unknown_group` 非空，`assembly_contract` 立即失败。其余门禁（§6.2 的 B1–B6、F1–F2）可按批次增量落地，但 B3 落地前 F2 必须已收缩，否则 `protocol/dto/config.rs` 的足迹仍会被算入。
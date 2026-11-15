# 特性配置归属 任务清单

> 规格：[feature-config-ownership-spec.md](./feature-config-ownership-spec.md)
> 事实基线：规格 §2 的现状实测（`assistant-config/src/vault_conf.rs` 的 12 个 `pub vault_*` 字段、`cmd/vault/` 5 个文件共 19 处 `VaultConf` 引用、`protocol/dto/config.rs` 的 2 个类型与 2 个 `From`、`vault_plugin.rs` 1 处、`protocol_dto_roundtrip.rs` 1 处）
> 前置：无外部依赖。B1 是纯重构（零行为变化）；B2 / B3 各自必须与门禁改动同批，否则单批提交即红。

批次对应规格章节：B1 S1–S3（§4 / §5 S1–S3）→ B2 S4–S6 + a8（§5 S4–S6 / §6.1）→ B3 S7–S8（§3.4 / §5 S7–S8）→ B4 §6.2 / §7 / §5 S9。

依赖：B2 依赖 B1（`ConfGroup` trait 与 `conf_group_shims!` 必须先存在）；B3 依赖 B2（`VaultConf` 必须先落在 `assistant-vault`，`From` 的源类型才在 DTO 所属 crate 内，R10 才成立）；B4 的 B1 / B2 / B6 断言依赖 B1 落地，B3 / B4 / B5 / F1 / F2 依赖各自批次落地。

> 本清单不管理提交。提交粒度按代码依赖与可编译性现场决定；这里只回答「代码改没改、验收跑没跑」。
> 全部任务初始未实施；勾选表示代码已改且对应验收已跑。

---

## 阶段 B1 — 机制层与四组原地 impl

目标：把 4 份同构的 `get_conf_dir` / `get_conf_path` / `load` / `load_from` / `save` / `save_to` / `validate` / `amend` 收敛到一份 trait 默认实现，**磁盘文件名、wire 键、日志前缀、加载策略、`validate` 规则、40+ 调用点形态全部不变**。

### B1.1 新增 `conf_group.rs`

对应规格 §4.2；验收走 AC1。

- [x] 新建 `src-tauri/crates/assistant-config/src/conf_group.rs`
- [x] `pub enum LoadPolicy { Strict, Lenient, LenientWithMigrate }`（三态齐全，禁止合并，R4）
- [x] `pub trait ConfGroup: Sized + Serialize + DeserializeOwned + Clone`：4 个关联常量 `GROUP` / `LOG_TAG` / `FILE_NAME` / `LOAD_POLICY`（R1–R4），默认钩子 `migrate` / `adopt_legacy` / `validate`（R5–R7）
- [x] 8 个 provided 方法 `get_conf_dir` / `get_conf_path` / `load` / `load_from` / `save` / `save_to` / `validate` / `amend`，实现体只做参数转发，不写分支
- [x] `pub fn apply_patch<C: ConfGroup>(patch: serde_json::Value) -> Result<(C, C), String>`：内部 `load → amend → validate → save`，返回 `(prev, next)`；`next.validate()` 通过才返回（口径见规格 §4.2）
- [x] `macro_rules! conf_group_shims`：为 8 个方法生成同名 inherent 委托（`impl $t { pub fn load() -> ... { <$t as ConfGroup>::load() } ... }`），保证既有调用点无需 `use ConfGroup`
- [x] `src-tauri/crates/assistant-config/src/lib.rs`：加 `pub mod conf_group;` 与 `pub use conf_group::{apply_patch, ConfGroup, LoadPolicy};`

### B1.2 `json_config.rs` 收敛

对应规格 §4.2 末段；验收走 AC1（`atomic_write` / 迁移 / 严格校验测试全绿）。

- [x] `src-tauri/crates/assistant-config/src/json_config.rs` 新增 `pub fn load_conf<C: ConfGroup>(path: &Path, policy: LoadPolicy) -> Result<C, String>`
- [x] `load_conf` 内三分支：`Strict` → 走原 `load_json_config_strict` 逻辑（损坏即报错，绝不覆盖）；`Lenient` → 走原 `load_json_config` 逻辑（损坏回落默认并写回）；`LenientWithMigrate` → 走原 `load_json_config_with_migrate` 逻辑（先 `migrate` 再回落）
- [x] 三分支都保持「文件不存在 → `new()` 并写盘」「`adopt_legacy` 仅在文件不存在时调用」两条现状语义
- [x] 删除 `pub fn load_json_config` 与 `pub fn load_json_config_with_migrate`（实测调用点仅 `mascot_conf.rs:65`、`orb_conf.rs:54`、`app_conf.rs:163` 三处，B1.3 后全部消失）
- [x] 保留 `load_json_config_strict` / `amend_json_config` / `save_json_config` / `get_conf_dir` / `recover_temp` 不动
- [x] 确认 `cargo test -p assistant-config --test atomic_write` 与 `--test app_conf` / `--test orb_conf` / `--test mascot_conf` 无回归

### B1.3 四个配置组原地 impl

对应规格 §5 S2；验收走 AC1。每个文件都是「删 8 个方法体 → 加 `impl ConfGroup` → 加 `conf_group_shims!`」。

- [x] `src-tauri/crates/assistant-config/src/app_conf.rs`：`GROUP = "app"`、`LOG_TAG = "AppConf"`、`FILE_NAME = "config.json"`、`LOAD_POLICY = LenientWithMigrate`；`migrate` 保留现有实现
- [x] `src-tauri/crates/assistant-config/src/orb_conf.rs`：`GROUP = "orb"`、`LOG_TAG = "OrbConf"`、`FILE_NAME = "orb-config.json"`、`LOAD_POLICY = Lenient`
- [x] `src-tauri/crates/assistant-config/src/mascot_conf.rs`：`GROUP = "mascot"`、`LOG_TAG = "MascotConf"`、`FILE_NAME = "mascot-config.json"`、`LOAD_POLICY = Lenient`
- [x] `src-tauri/crates/assistant-config/src/vault_conf.rs`：`GROUP = "vault"`、`LOG_TAG = "VaultConf"`、`FILE_NAME = "vault-config.json"`、`LOAD_POLICY = Strict`；`adopt_legacy` 承接现 `migrate_from_app_conf`（读的是 `config.json`，跨 group，不能用 `migrate`）（该文件随 B2.1 / B2.3 整体迁入 `assistant-vault/src/conf.rs` 后删除）
- [x] 四个文件各加 `crate::conf_group_shims!(AppConf);` / `(OrbConf);` / `(MascotConf);` / `(VaultConf);`（crate 内调用写 `crate::`，R8）
- [x] 四个文件保留 `pub fn new() -> Self` 与 `impl Default`（`new` 是既有公开 API，调用点遍布全仓）
- [x] `validate` 实现体内禁止调用 `self.validate()` / `self.load()` / `self.save()` 等同名方法（R11；否则与 inherent 委托互相递归）
- [x] `assistant-config/src/lib.rs` 的 `pub mod app_conf;` / `orb_conf;` / `mascot_conf;` / `vault_conf;` 四行不变（a8 的 expected 集合依赖这四行，B1 不动；B2.3 后 `vault_conf` 一行删除，a8 改由 `feature_conf_groups()` 补回）

### B1.4 写入序列收敛

对应规格 §5 S3；验收走 AC1。

- [x] `src-tauri/src/cmd/config.rs` 的 `apply_app_conf` / `apply_orb_conf` / `apply_mascot_conf` 改为 `let (prev, merged) = assistant_config::apply_patch::<AppConf>(patch)?; apply_*_side_effects(&prev, &merged);`（`apply_patch` 内部已完成 save，调用方不再 `save()`）
- [x] `src-tauri/src/cmd/vault/mod.rs` 的 `apply_conf` 同样改为 `apply_patch::<VaultConf>`
- [x] `CONF_LOCK` 的持锁范围不变（仍由 `patch_conf`（`cmd/config.rs:196`）与 `update_app_conf`（`:134`）在外层持锁）
- [x] side effects 一律在 `apply_patch` 返回后执行，与现状「先 save 再 side effects」顺序一致
- [x] `register_conf_applier("app" / "orb" / "mascot")` 三处调用点**继续写字面量**（R1；改传关联常量会让 a8 的字面量解析落空）
- [x] `src-tauri/src/cmd/mod.rs` 的 `conf_side_effects()` 函数体不改

### B1 验收

- [x] `cargo test -p assistant-config`（cwd = `src-tauri`）全绿
- [x] `cargo test -p assistant-app --test assembly_contract` 全绿（B1 未动 `lib.rs` 的 `pub mod`，a8 不受影响）
- [x] `cargo clippy -p assistant-config --all-targets` 无新增告警
- [x] 本批次零前端改动，不跑 `npm run typecheck` / `contract:check`（留到 B3）

---

## 阶段 B2 — vault schema 迁入 vault crate

目标：`VaultConf` 的 schema、默认值、校验、旧版迁移全部归 `assistant-vault`，`assistant-config` 只留机制。**磁盘文件名、wire 键、加载策略、校验规则不变**。

> B2 与 B3 之间无契约生成物变化，但与 a8 改造必须同批：删掉 `assistant-config/src/lib.rs` 的 `pub mod vault_conf;` 后 a8 的 expected 立即少 `vault`，而 `vault_plugin.rs` 仍注册 `"vault"`，`assembly_contract` 立刻报 `unknown_group`。

### B2.1 新增 `assistant-vault/src/conf.rs`

对应规格 §5 S4；验收走 AC2。

- [x] 新建 `src-tauri/crates/assistant-vault/src/conf.rs`
- [x] 搬 `MAX_AUTO_LOCK_MINUTES` / `MAX_CLIPBOARD_CLEAR_SECONDS` 两个常量（`pub` 可见性不变）
- [x] 搬 `VaultCipherAlgorithm` 枚举与 `as_str` / 反序列化实现（`pub` 不变）
- [x] 搬 `VaultConf` 结构体 12 个 `pub vault_*` 字段与 `#[serde(default = "...")]` 属性，逐字不变（B5 断言依赖 `pub vault_` 前缀）
- [x] 搬全部 `default_*` 函数，函数名与返回值逐字不变
- [x] 搬 `new()` 与 `impl Default`
- [x] 搬 `migrate_from_app_conf` → 改名为 `adopt_legacy(path: &Path) -> Option<Self>`，内部逻辑与读取的 `config.json` 键名逐字不变
- [x] 搬 `validate` 内四段规则（auto_lock 上限、clipboard 上限、SSH 主机、远端 URL），错误文案逐字不变
- [x] 搬 `is_absolute_local_path` / `is_scp_style_remote` / `is_supported_remote_url` 三个自由函数（私有，不导出）
- [x] 删除对 `assistant_config::json_config::{load_json_config_strict, save_json_config, amend_json_config, get_conf_dir}` 与 `super::atomic_write::recover_temp` 的直接 import（全部由 trait provided 方法接管）
- [x] `impl ConfGroup for VaultConf` 只写 4 个关联常量 + `adopt_legacy` + `validate`，其余 8 个方法走默认
- [x] `assistant_config::conf_group_shims!(VaultConf);`（跨 crate 调用写 `assistant_config::`，R8）
- [x] `src-tauri/crates/assistant-vault/src/lib.rs` 加 `pub mod conf;`

### B2.2 依赖边

对应规格 §3.3；验收走 AC8。

- [x] `src-tauri/crates/assistant-vault/Cargo.toml` 的 `[dependencies]` 加 `assistant-config = { workspace = true }`
- [x] 根 `src-tauri/Cargo.toml` 不改（`[workspace.dependencies]` 已有该 path 条目）
- [x] 确认 `assistant-config/Cargo.toml` 的 `[dependencies]` 仍无任何 workspace crate（无环不变式）
- [x] `cargo tree --no-default-features -p assistant-app -i assistant-vault` 无输出（关闭态 vault 不参与编译）

### B2.3 删除旧模块

对应规格 §5 S4；验收走 AC2。

- [x] 删除 `src-tauri/crates/assistant-config/src/vault_conf.rs`
- [x] `src-tauri/crates/assistant-config/src/lib.rs` 删 `pub mod vault_conf;` 一行（其余三个 `pub mod *_conf;` 保留）
- [x] `assistant-config` 的 `Cargo.toml` 与 `[dev-dependencies]` 不动（`tempfile` 仍被 `atomic_write` 等测试使用）

### B2.4 引用改路径（8 文件，与 B2.3 同批）

对应规格 §5 S5。删模块与改引用之间没有可编译的中间态，8 个文件必须在同一批完成。

- [x] `src-tauri/src/cmd/vault/mod.rs:6` `use assistant_config::vault_conf::VaultConf;` → `use assistant_vault::conf::VaultConf;`
- [x] `src-tauri/src/cmd/vault/conf.rs:4` 的 `use assistant_config::vault_conf::VaultConf;` → `use assistant_vault::conf::VaultConf;`；`:6` 的 `use crate::protocol::VaultConfDto;` → `use assistant_vault::dto::VaultConfDto;`（后者随 B3.2 一起改）
- [x] `src-tauri/src/cmd/vault/recovery.rs` 同上
- [x] `src-tauri/src/cmd/vault/sync.rs` 内联全路径 `assistant_config::vault_conf::VaultConf::load()`
- [x] `src-tauri/src/cmd/vault/ssh.rs` 同上
- [x] `src-tauri/src/vault_plugin.rs` 启用块内 `on_setup` 的 `VaultConf::load()`
- [x] `src-tauri/src/protocol/dto/config.rs` 两个 `impl From` 的源类型路径改为 `assistant_vault::conf::{VaultCipherAlgorithm, VaultConf}`（**本批只改路径**，类型删除留 B3）
- [x] `src-tauri/tests/protocol_dto_roundtrip.rs` 的 vault 分支构造改 `assistant_vault::conf::VaultConf::new()`（**本批只改路径**，分支删除留 B3）
- [x] 确认 `cmd/vault/mod.rs` 的 `c.vault_cipher_algorithm.as_str()` 是方法调用，无需新增导入

### B2.5 测试搬家

对应规格 §5 S6；验收走 AC2。

- [x] `src-tauri/crates/assistant-config/tests/vault_conf.rs` 整体搬到 `src-tauri/crates/assistant-vault/tests/conf.rs`，`use` 改为 `assistant_vault::conf::{VaultCipherAlgorithm, VaultConf}`
- [x] `src-tauri/crates/assistant-vault/tests/common/mod.rs` 追加 `pub fn conf_path(name: &str) -> PathBuf`，签名与实现与 `assistant-config/tests/common/mod.rs` 逐字相同（该文件已有 `#![allow(dead_code)]`，不会触发告警）
- [x] `tests/conf.rs` 顶部加 `mod common;`（复用 vault 已有 helper，不新建第二份）
- [x] 删除 `src-tauri/crates/assistant-config/tests/vault_conf.rs`
- [x] `src-tauri/crates/assistant-vault/tests/conf_keys.rs` 的 `vault_conf_source()` 从 `../assistant-config/src/vault_conf.rs` 改为 `../assistant-vault/src/conf.rs`
- [x] `conf_keys.rs` 追加断言：`declared_keys().len() >= 12`（B5，防搬家漏字段）
- [x] `src-tauri/crates/assistant-config/tests/common/mod.rs` 的 `conf_path` 保留（`app_conf` / `orb_conf` / `mascot_conf` 测试仍在用）

### B2.6 a8 门禁改造（与 B2.3 同批）

对应规格 §6.1；验收走 AC5。不改则 `cargo test -p assistant-app --test assembly_contract` 立即失败。

- [x] `src-tauri/tests/assembly_contract.rs` 的 `config_conf_groups()` 保留（继续扫 `assistant-config/src/lib.rs` 的 `pub mod *_conf;`，B1 后剩 3 个）
- [x] 新增 `feature_conf_groups()`：扫 `src-tauri/crates/*/src/conf.rs`，按文件名发现，提取每个文件 `GROUP: &'static str = "X"` 的字面量；**禁止枚举特性名、禁止硬编码 `vault`**
- [x] `a8_conf_applier_groups_match_config_structs` 的 expected 改为 `config_conf_groups() ∪ feature_conf_groups()`
- [x] 新增断言：每个 `*_conf.rs` 的 `GROUP` 常量值与模块名逐字相等（`app_conf.rs` ↔ `"app"`）
- [x] `conf_applier_groups()` **不改**（`register_conf_applier` 调用点仍写字面量，R1）
- [x] `[features]` 相关断言（b2）不改

### B2 验收

- [x] `cargo test -p assistant-vault --test conf`（cwd = `src-tauri`）全绿，用例数 ≥ 搬家前
- [x] `cargo test -p assistant-vault --test conf_keys` 全绿
- [x] `cargo test -p assistant-config` 全绿（不再含 vault 用例）
- [x] `cargo test -p assistant-app --test assembly_contract` 全绿
- [x] `cargo build -p assistant-app --no-default-features` 全绿（关闭态仍可编译）
- [x] `cargo clippy -p assistant-vault --all-targets` 无新增告警

---

## 阶段 B3 — DTO 迁入 vault 契约域

目标：`VaultConfDto` / `VaultCipherAlgorithmDto` 与 vault 其余 23 个 DTO 同域，前端类型源从 `protocol.contract` 收口到 `@/vault/core`。

> 必须与两份生成物同批提交，否则 `npm run contract:check` 失败。

### B3.1 vault 侧新增

对应规格 §5 S7；验收走 AC3。

- [x] 新建 `src-tauri/crates/assistant-vault/src/dto/conf.rs`
- [x] 搬 `VaultCipherAlgorithmDto` 枚举（`#[serde(rename_all = "camelCase")]`，封闭集合保持 `enum`）
- [x] 搬 `VaultConfDto` 结构体 12 个字段（`#[serde(rename_all = "camelCase", deny_unknown_fields)]`）
- [x] 两类型各加 `#[ts(export, export_to = "vault.contract.ts")]`（与 vault 其余 DTO 同款属性）
- [x] 输出 DTO 的 `Option` 字段**不加** `skip_serializing_if`（键恒在、值 `null`）
- [x] 两个 `impl From` 写在 `src-tauri/crates/assistant-vault/src/dto/convert.rs`（类型所属 crate 内，规避孤儿规则，R10）
- [x] `src-tauri/crates/assistant-vault/src/dto/mod.rs` 加 `pub mod conf;` 与 `pub use conf::{VaultCipherAlgorithmDto, VaultConfDto};`

### B3.2 主应用删除

对应规格 §5 S7；验收走 AC4。

- [x] `src-tauri/src/protocol/dto/config.rs` 删 `VaultCipherAlgorithmDto` 枚举与其 `impl From`
- [x] 同文件删 `VaultConfDto` 结构体与其 `impl From<VaultConf>`
- [x] `src-tauri/src/cmd/vault/conf.rs:6` 的 `use crate::protocol::VaultConfDto;` → `use assistant_vault::dto::VaultConfDto;`
- [x] 确认 `src-tauri/src/protocol/dto/mod.rs` 的 `pub use config::*;` 仍保留（另外 3 个核心 `*ConfDto` 还在该文件）

### B3.3 既有门禁改动

对应规格 §6.1；验收走 AC3、AC4。

- [x] `src-tauri/tests/protocol_bindings.rs` 删 `VaultCipherAlgorithmDto` / `VaultConfDto` 两个 import 名
- [x] 同文件删两行 `register::<...>()` 注册（主契约不再含这两个类型）
- [x] `src-tauri/tests/protocol_dto_roundtrip.rs` 删 `config_dtos_cover_every_config_field` 的 vault 分支与相关 import；helper `assert_dto_covers_config` 保留（另 3 个 conf 仍用）
- [x] `src-tauri/crates/assistant-vault/tests/dto_contract.rs` 新增断言（B4）：冻结 `VaultConfDto` 的 12 个 wire 键（camelCase）与 `VaultConf` 的 12 个磁盘键（snake_case），并断言两侧字段集合一致；helper 从 `protocol_dto_roundtrip.rs` 复制一份到 vault 测试（该断言是删主契约分支后的唯一字段覆盖保障）

### B3.4 前端类型源收口

对应规格 §5 S8；验收走 AC7。

- [x] `src/vault/core/vault.types.ts` 在 23 个现有别名之后（`UnreadableEntries` 行后、`RevealedField` 接口前）追加两行，风格与现有别名逐字同构：`export type { VaultCipherAlgorithmDto as VaultCipherAlgorithm } from "./contract/vault.contract";` 与 `export type { VaultConfDto as VaultConf } from "./contract/vault.contract";`
- [x] `src/features/vault/conf.ts` 第 2 行 `import type { VaultConfDto } from "@/types/protocol.contract";` → `import type { VaultConf } from "@/vault/core/vault.types";`（不带 `.ts` 后缀，与仓库既有导入风格一致）
- [x] 同文件第 4 行 `export type VaultConf = VaultConfDto;` 删除（别名上移到 vault 域 barrel）
- [x] 确认 `ConfSlot` 的泛型实参与 `getConf` / `setConf` 调用形态不变（只换类型来源，不改结构）

### B3.5 契约重生成

对应规格 §3.4；验收走 AC6。

- [x] `npm run contract:gen`（cwd = 仓库根）
- [x] 提交 `src/types/protocol.contract.ts`（少两个类型）与 `src/vault/core/contract/vault.contract.ts`（多两个类型）
- [x] 禁止手改两份生成物（类型安全规范）
- [x] `npm run contract:check` 全绿
- [x] `git --no-pager diff --stat` 核对：只有这两份生成物变动，无其它 `.ts` 被顺带改写（本次核对结论：`vault.contract.ts` 仅新增两条类型、`protocol.contract.ts` 的 vault 差异仅删除两条类型；该文件其余差异来自同工作区并行进行的 I51 历史详情改动 `protocol/dto/chat.rs` 的既有漂移，非本规格引入）

### B3 验收

- [x] AC3：`cargo test -p assistant-vault --test dto_contract` 全绿（12 个 wire 键与 12 个磁盘键冻结且字段集合一致）
- [x] AC4：`cargo test -p assistant-app --test protocol_bindings` 全绿（主契约注册清单无 vault 类型、无冗余）
- [x] AC6：`npm run contract:gen && npm run contract:check` 全绿；`protocol.contract.ts` 不含 `VaultConfDto` / `VaultCipherAlgorithmDto`，`vault.contract.ts` 含两者
- [x] AC7：`npm run typecheck` + `npm run lint` 全绿
- [x] `cargo test -p assistant-app --test protocol_dto_roundtrip` 全绿
- [x] 手测：设置页 vault 分组的字段名、默认值、校验提示与改造前逐字一致（G4）

---

## 阶段 B4 — 门禁与文档

### B4.1 新增结构断言

对应规格 §6.2；验收走 AC5。

- [x] 新建 `src-tauri/crates/assistant-config/tests/structure.rs`
- [x] B1：`assistant-config/src/` 下不存在 `vault_conf.rs`
- [x] B1 续：`assistant-config/src/*.rs` 内容不含 `config/features.json`（仓库根）中任一特性 id（大小写不敏感）；特性 id 从该文件读取，禁止硬编码清单
- [x] B2：`conf_group.rs` / `json_config.rs` 不含 `cfg!(feature = ...)`、`#[cfg(feature = ...)]` 与特性 id 判断（与 B1 同文件实现，共用特性 id 读取逻辑）
- [x] B6：在 `src-tauri/crates/assistant-vault/tests/conf.rs` 内加一个 `#[test]`，扫 `assistant-vault/src/` 全量 `.rs`，断言不出现 `app_conf` / `orb_conf` / `mascot_conf` 三个标识（R9 的唯一门禁；不新建测试文件，落点与规格 §6.2 一致）
- [x] F1：`src/features/vault/**` 不出现 `@/types/protocol.contract`（复用 `coreBoundary.test.ts` 的 `CORE_SCAN_ROOTS` 机制）

### B4.2 既有门禁改动

对应规格 §6.1；验收走 AC5。

- [x] `src/features/__tests__/coreBoundary.test.ts`：`FEATURE_FOOTPRINT_ALLOWLIST.vault` 从 3 项收缩为 2 项（`src-tauri/src/cmd/mod.rs`、`src-tauri/src/protocol/events.rs`），删除 `src-tauri/src/protocol/dto/config.rs`
- [x] F2：同文件「allowlist 与实际扫描结果逐字相等」的现有断言保持，只改清单，**只减不增**
- [x] 确认 `scripts/contract-check.mjs` 的 TARGETS 清单不改（两份生成物路径未变）

### B4.3 文档同步

对应规格 §7 与 §5 S9。文档归档后不再用路径指向 `docs/archived/`，一律写「文档名 +（已归档）」。

- [x] `../../store/vault-optional-build.md`：§2 依赖表第 54 行改写为「`VaultConf` 已迁入 `assistant-vault/src/conf.rs`」；§3.4 第 109–110 行两行归属改为 vault crate 与 vault 契约域；§4.6 配置层取舍表前两条结论标记「已被本规格替代」
- [x] `../../store/feature-mount-traversal-spec.md`：§1.2 非目标中「不迁移 `protocol/dto/config.rs`」加注「配置 DTO 一项已由本规格迁出」；§6.1 保留足迹表删除该行（表由 3 项变 2 项）
- [x] `../agent/I50-runtime-audit-tasklist.md`：P1.1 落地说明中「`protocol/dto/config.rs` 只留配置 DTO（`VaultConfDto` / `VaultCipherAlgorithmDto`）」改为「配置 DTO 亦在 vault 契约域」
- [x] `../../store/crate-architecture.md`：`assistant-config` 职责段去掉 `vault_conf`、补「配置机制库」定位；§1.3 依赖图（第 70 行「内部依赖边共 **5 条**」）补 `assistant-vault → assistant-config` 后改 6，第 80 行「其余 9 个 crate（config / …）均为叶子」改为「其余 8 个 crate（i18n / crypto / orb / tools / storage / crash / providers / sync）均为叶子节点」（`config` 从括号内移除，因为它不再是叶子）；§2.1 的「`vault_conf` 只负责读写 vault 配置结构，不引用 vault 的任何代码」整句替换为「只提供配置机制，vault 配置 schema 位于 `assistant-vault/src/conf.rs`」
- [x] `../../CODE-STRUCTURE.md`：§4.5 依赖图「共 5 条」改 6、`assistant-vault` 行依赖补 `assistant-config`；§4.5 职责表 `assistant-config` 的「关键模块」去掉 `vault_conf`；§5「新增设置项」一行改为「核心 group 在 `assistant-config`，特性 group 在特性 crate 的 `conf.rs`」
- [x] `../../analysis/FEATURE-OPTIONAL-BINARY-ANALYSIS.md`：「`FEATURE_FOOTPRINT_ALLOWLIST` 现为 3 个」改 2 个
- [x] `../../analysis/FEATURE-ZERO-COST-MOUNT-ANALYSIS.md`：足迹 7 → 3 的结论保持，补注「3 项中配置 DTO 一项后续归零」
- [x] `.trae/rules/project_rules.md`：类型安全规范的「新增设置项」段补一句：特性自有配置组的 struct 位于特性 crate，DTO 位于该特性契约域，仍需 `npm run contract:gen`

### B4 验收

- [x] AC5：`cargo test -p assistant-app --test assembly_contract` 与 `npx vitest run src/features/__tests__/coreBoundary.test.ts` 全绿（含 §6.2 新增的 B1–B6、F1–F2）
- [x] `cargo test -p assistant-config` 全绿
- [x] `cargo test -p assistant-vault --test conf` 全绿（含 B6 的源码扫描断言）
- [x] `npm run lint` + `npm run typecheck` 全绿

---

## 终态验收（全部批次完成后一次性核对，口径同规格 §8）

- [x] AC1 配置机制与核心 group 行为零变化 → `cargo test -p assistant-config`
- [x] AC2 vault 配置行为零变化（含 `Strict` 损坏不写回、旧键接管）→ `cargo test -p assistant-vault --test conf`
- [x] AC3 字段覆盖与 wire 键冻结 → `cargo test -p assistant-vault --test dto_contract`
- [x] AC4 主契约无 vault 类型且注册清单无冗余 → `cargo test -p assistant-app --test protocol_bindings`
- [x] AC5 特性边界与足迹清单 → `cargo test -p assistant-app --test assembly_contract` + `npx vitest run src/features/__tests__/coreBoundary.test.ts`
- [x] AC6 契约生成物同步 → `npm run contract:gen && npm run contract:check`
- [x] AC7 前端类型与字段定义不变 → `npm run typecheck`
- [x] AC8 关闭态依赖闭包断开 → `cargo tree --no-default-features -i assistant-vault`（cwd = `src-tauri`，无输出）
- [x] 附加：`cargo build --no-default-features` 与 `cargo build`（默认特性）均全绿；`cargo clippy -p assistant-config -p assistant-vault --all-targets` 无新增告警
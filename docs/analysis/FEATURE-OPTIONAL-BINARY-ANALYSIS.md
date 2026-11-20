# 可选功能二进制编译：引入方式的合理性分析

> 日期：2026-11-11（v2）｜2026-11-20（结论表改为命题核对、事实改时点快照、移除分级建议）｜性质：**预先分析**（只核对「某种引入方式与现有机制是否相容」；**不含取舍决议**——采用哪种引入方式、何时做，属规格职责）
> 判据来源：`docs/principle/CODE-REVIEW-PRINCIPLE.md`、`docs/principle/CODE-SPLIT-PRINCIPLE.md`
> 事实快照：截至 2026-11-20 的仓库代码（**时点记录**，不随代码演进追改；需要最新事实时以代码为准）
> 修订说明：v1（2026-11-10）的事实基线是「8 装配点 + 7 足迹 + 5 行 cfg + 单插件写死的 build.rs」。该形态已于同日被 《feature-mount-traversal-spec.md》（已归档） 的 S1–S8 改造取代并落地。**本文 D1/D2/D3 三条判定不变，§1、§3、§5、§8、§9 的证据全部按改造后代码重算**。
> 本文所有结论以**当前代码**为准；与 I24-feature-extension-spec（已归档）冲突处已单独标注（归档设计稿允许滞后）。
> 时效与终止：本文是**一次性决策输入**。可选编译规格定案后整体归档冻结、不再修改；有效内容由规格承接，事实条目作为历史快照保留。

---

## 0 命题核对

命题按三个正交自由度拆开核对，混在一起谈必然误判：

| # | 命题 | 判定 | 一句话依据 |
|---|---|---|---|
| **D1** | 「某个功能在不在」用特性 id 表达 | **成立** | 声明层 / 派生层 / 门禁层已闭环；新增特性成本被压到 7 文件 11 行，其中调用型 0 行 |
| **D2** | 「同一功能以什么入口呈现」（如 vault 独立窗口）用第二个特性 id 表达 | **不成立** | 形态开关的落点在 `window.rs`，而该文件被 A2 断言禁止出现 cfg；且 env 命名断言拒绝连字符 id |
| **D2'** | 形态改为**运行期**决策（窗口注册表 + 运行期配置） | **成立** | `WindowSpec` 注册表与 `show_or_create` 本就是运行期机制，与"是否编译进去"无关 |
| **D3** | 「出几个可执行文件」用特性 id 表达 | **不成立** | `generate_context!` 路径固定、`build.rs` 每 package 一份、ACL 是 package 级产物，feature 无法安全切换 |
| **D3'** | 多产物升级为**独立 package** | **成立** | 唯一能隔离 conf / build.rs / capabilities 的层级 |
| — | 前端 loader 代码生成 | **不成立** | I24（已归档）§3.7 已因"生成器 ↔ 生成物漂移"否决；当前每特性仅 4 行（`registry.ts` 3 + `vite-env.d.ts` 1），收益为负 |
| — | preset 枚举受支持组合 | **部分成立** | 只允许列 feature 集合时成立；一旦写入产物名 / cargo 参数即成为第二真源 |

**核对结果**：与编译期标定相容的只有 **D1（功能集合）**。D2 是运行期问题，D3 是 package 级问题——把它们塞进特性体系，代价不是"多一个开关"，而是**与现有门禁直接冲突**（见 §3、§4 的反例推演）。

---

## 1 代码事实快照（2026-11-20）

### 1.1 声明层

| 事实 | 证据 | 数值 |
|---|---|---|
| 特性清单只有 1 条，三键结构 | `config/features.json` | `vault` / `default:true` / `cargoFeature:"vault"` / `env:"VAULT_ENABLED"` |
| cargo 侧镜像 | `src-tauri/Cargo.toml` 第 136–141 行 | `default = ["vault"]`、`vault = ["dep:assistant-vault", "dep:assistant-sync", "dep:assistant-security"]` |
| 前端常量自动派生 | `vite.config.ts` `featureDefines()` | 遍历清单，产出 `__${spec.env}__`，未设置 env 时回落到 `spec.default` |
| 构建期注入 | `scripts/feature-build.ps1` `Set-FeatureEnvironment()` | 遍历清单，写 `1` / `0` |
| 清单 ↔ cargo 双向断言 | `scripts/feature-build.ps1` `Assert-FeatureConsistency()` | 正向 + 反向 + 默认集合相等，任一不符即中止 |
| 清单 ↔ 挂载表 ↔ 磁盘三方断言 | `assembly_contract.rs` `b2_feature_drift()` | 三方全等，Cargo 侧只做子集（允许与插件无关的 cargo 特性） |

这一层是**单源 + 镜像 + 断言**结构，新增一条特性只需在清单加一段、cargo 加一行，脚本主流程零改动（全部遍历清单）。

### 1.2 装配层

| 事实 | 证据 | 数值 |
|---|---|---|
| 装配点被硬编码为 **6** 条 | `src/features/__tests__/coreBoundary.test.ts` `ASSEMBLY_POINTS` | `features.json` / `vite-env.d.ts` / `registry.ts` / `Cargo.toml` / `feature_mounts.rs` / `lib.rs` |
| 已遍历化的调用方单列 **5** 条 | 同上 `TRAVERSAL_FILES` | `build.rs` / `capabilities/default.json` / `plugins.rs` / `lifecycle.rs` / `setup.rs`（断言：文本内不得出现任何清单内特性 id） |
| 后端 cfg 现状 | `src-tauri/src/**` 中 `#[cfg(...feature = "...")]` | **3 行**：`vault_plugin.rs` 第 5、108 行，`cmd/mod.rs` 第 19 行 |
| 装配层已零 cfg | `lib.rs` / `plugins.rs` / `lifecycle.rs` / `ipc.rs` / `setup.rs` / `window.rs` | 全部 0 行 |
| 可选性落点为「挂载表 + 互斥块」 | `feature_mounts.rs` 的 `MOUNTS`；`vault_plugin.rs` 的 `#[cfg(feature="vault")] mod mounted` / `#[cfg(not(feature="vault"))] mod mounted` | 2 处互斥块，块外 `pub(crate) use mounted::{init, on_exit, on_setup}` |
| 挂载表是唯一「特性 → 函数」映射 | `feature_mounts.rs` `const MOUNTS: &[FeatureMount]` + `fn mounts()` | 1 张表、1 个读入口（B7 断言）；表内 0 处 cfg（B1 断言） |
| 命令清单从插件真源派生 | `build.rs` `plugin_ids()` 扫 `src/*_plugin.rs` + `COMMAND_PREFIX_TEMPLATE = "crate::cmd::{}::"` | 已泛化为多插件；单文件必须恰有 1 个 `generate_handler![` 块，否则 panic |
| vault 命令条数 | `vault_plugin.rs` 的 `generate_handler![` 块 | **69 条**（`cmd/vault/**` 共 71 个 `pub fn vault_*`，其中 `vault_setup` / `vault_cleanup_on_exit` 是插件生命周期钩子，不注册） |

### 1.3 门禁层

**后端 `assembly_contract.rs`（A 组：装配契约；B 组：遍历化新增）**

| 断言 | 约束内容 |
|---|---|
| A1 | `ipc.rs` 内不得出现任何 `#[cfg]` |
| A2 | 特性 cfg **只允许**出现在 `cmd/mod.rs` 或与特性同名的 `<name>_plugin.rs`（按文件名匹配，连字符先转下划线） |
| A3 | `lib.rs` ≤ 40 行且不得内联 `generate_handler!` |
| A4 | 插件注册表 ↔ ACL 双向求差，且不同插件命令不得重叠 |
| A5 | 每个 `*_plugin.rs` 至少注册 1 条命令（解析器下限探针） |
| A6 | 宿主与插件命令不得重叠（注册分区） |
| A7 | 命令名 ↔ 权限标识符派生规则（`allow-` + 蛇形转短横线）；`build.rs` 必须含目录遍历标记 |
| A8 | `register_conf_applier` 分组集合 == `assistant-config` 的 `*_conf` 模块集合 |
| B1 | `feature_mounts.rs` 内 `#[cfg(` 出现次数 = 0 |
| B2 | 清单键集合 == `MOUNTS` 的 `id` 集合 == 磁盘 `src/*_plugin.rs` stem 集合；清单 `cargoFeature` ⊆ Cargo `[features]` |
| B3 | `plugins.rs` / `lifecycle.rs` / `setup.rs` 不出现特性 id 字面量 |
| B4 | 单特性全后端 cfg 行数 ≤ **3**，且本体为「一对互斥块」 |
| B5 | 关闭块与启用块的 `init` / `on_setup` / `on_exit` 三函数签名逐一相等 |
| B6 | `cmd/mod.rs` 中每特性恰 1 条 cfg 属性 |
| B7 | `feature_mounts.rs` 恰 1 张表、1 个读入口，`MOUNTS` 只出现 2 次 |
| B8 | 每特性存在 `capabilities/<id>.json` 且 `identifier` 与 `permissions` 含 `<id>:default`；`default.json` 不得再授予 |
| B9 | 每个 `*_plugin.rs` 恰 1 个 `generate_handler![` 块；`build.rs` 用同一判据 panic |

**前端 `coreBoundary.test.ts`（F 组）**

| 断言 | 约束内容 |
|---|---|
| F-装配点 | 每个声明特性必须齐备 6 个装配点 + 2 处本体（前端 `module.ts` / 后端 `*_plugin.rs`）；`env` 必须等于 `${name.toUpperCase()}_ENABLED`；插件文件必须含 `feature = "<name>"` |
| F-双向一致 | `commandGroups()` 分组名、`src/*_plugin.rs` 文件名、清单键三者集合相等；`permissions/*` 目录分支仍保留但已随目录删除而恒空（见 §1.4） |
| F-足迹冻结 | 特性名在 core 文件中出现的集合必须与 `FEATURE_FOOTPRINT_ALLOWLIST` 逐字相等；vault 现为 **2** 个文件，且明确注明"非新特性可复制先例" |
| F-动态 import | `registry.ts` 只允许 `await import("./<id>/module")` + 编译期守卫，静态 import 直接失败 |
| F-env 声明 | `vite-env.d.ts` 必须含 `declare const __<ENV>__: boolean` |
| F-遍历化反向 | `TRAVERSAL_FILES` 五个文件内不得出现任何特性 id |
| F-产物探针 | 声明了关闭态（`env=0`）时必须跑 `scripts/assert-feature-absent.mjs`；无关闭态声明则显式 `ctx.skip()` |

### 1.4 已实现与文档的偏差（以代码为准）

| 项 | 旧说法 | 代码事实 | 影响 |
|---|---|---|---|
| 装配点计数 | v1 记为 **8 条**（含 `build.rs` / `capabilities/default.json` / `plugins.rs`） | `ASSEMBLY_POINTS` 现为 **6 条**，上述三项与 `lifecycle.rs` / `setup.rs` 移入 `TRAVERSAL_FILES`（"不得出现特性名"） | **无功能影响**；v1 的"每特性成本"按 8 条计已作废，现按 6 + 5 两套口径计（§7） |
| 足迹计数 | v1 记为 vault 保留 **7 个** core 文件 | `FEATURE_FOOTPRINT_ALLOWLIST` 现为 **2 个**：`cmd/mod.rs`、`protocol/events.rs` | `cmd/config.rs` 已退出足迹（`get_vault_conf` 迁为 `vault_get_conf`，见 `cmd/vault/conf.rs`）；`protocol/dto/config.rs` 已由 `feature-config-ownership-spec.md`（已归档） 迁入 vault 契约域（`assistant-vault/src/dto/conf.rs`） |
| ACL 覆盖窗口 | I24 §11.1 称 `["main","orb","settings"]`（三窗口） | 代码只有两个窗口：`src/config/constants.ts` 仅定义 `WINDOW_MAIN` / `WINDOW_ORB`，`window.rs` 的 `WINDOW_SPECS` 只注册 `main`；`capabilities/default.json` 与 `capabilities/vault.json` 均为 `["main","orb"]` | 该行确为归档遗留（`settings` 是主窗口内的页面而非独立窗口）；本文不采信"三窗口"表述 |
| `permissions/<name>/default.toml` | v1 称"该目录对运行期与门禁均无作用，不应再作为装配点" | `src-tauri/permissions/` 根目录仍为空且已删除；ACL 授权改由 `build.rs` 的 `AllowAllCommands` 烘焙 + **每特性一份 `capabilities/<id>.json`** | v1 的结论需修正：**capability 文件拆分是新的必建装配物**（B8 + F-装配点双重断言），但它落在 `capabilities/` 而非 `permissions/` |
| vault cfg 行数 | v1 称 5 行，上限 6 | 实际 **3 行**，上限 `FEATURE_CFG_LIMIT = 3` | 从"接近上限"变为**顶格 3/3，零余量** |
| `build.rs` 插件支持数 | v1 称"结构上只支持一个插件，第二个必 panic" | 已泛化为 `plugin_ids()` 目录遍历 + 模板前缀 | v1 的 E5 整段作废，见 §3 新 E5 |
| `ipc.rs` 命令条数 | v1 称 **84 条** | 实测 **85 条** | 数值漂移，不影响结论 |

---

## 2 判定一：D1「功能集合用特性 id」——成立

**理由不是"已有先例"，而是四条机制已经互相咬合**：

1. **单源可派生**：`vite.config.ts` 与 `feature-build.ps1` 都遍历 `config/features.json`，没有任何手写开关名。新增特性不需要改这两个文件。
2. **装配点可枚举且已收敛到 6 条**：`ASSEMBLY_POINTS` 固化了"必须触碰的 core 文件"清单，其余 5 个调用方已遍历化并被反向断言禁止出现特性名。
3. **可选性只体现在一处**：`vault_plugin.rs` 用一对互斥 cfg 包住自身并提供同签名空实现，`feature_mounts.rs` 恒含该条目，因此装配层零 cfg——这正是 B1/B3/B5 在守的东西。
4. **三方一致性有断言**：清单键 ↔ `MOUNTS` 的 `id` ↔ 磁盘 `*_plugin.rs` 全等（B2），插件命令 ↔ ACL 双向相等（A7），特性权限 ↔ capability 文件（B8）。

**已实测支撑**：L1 阶段 `cargo check --no-default-features` 零错误通过，且 `cargo tree --no-default-features -i assistant-vault` 确认依赖图中不存在该 crate（记录见 `docs/store/vault-optional-build.md`）。这说明"关闭 = 真的不编"，不是"关闭 = 编了但不用"。遍历化改造（S1–S8）未触碰依赖门控，该结论继续成立，但**尚未在 CI 中固化为断言**（见 §8 G-1）。

**边界**：D1 的"自由"在现有机制下的实际边界是「受支持组合可枚举 + 机制统一」，而不是「2^N 组合全可编译」。后者会把门禁成本推高到不可维护。

---

## 3 判定二：D2「形态用第二个特性 id」——不成立

这是本文最重要的否定判定。证据全部来自现有门禁，不是风格偏好。

### E1 env 命名断言拒绝连字符 id

`coreBoundary.test.ts` 断言：

```
entry.env !== `${name.toUpperCase()}_ENABLED`  →  problems.push(...)
```

若 id 为 `vault-window`，期望 env 是 `VAULT-WINDOW_ENABLED`——**不是合法环境变量名**。`feature-build.ps1` 的 `Get-FeatureTable` 另有 `^[A-Z][A-Z0-9_]*$` 校验，必然拒绝。于是只能把 env 写成 `VAULT_WINDOW_ENABLED`，而这条又违反上述断言。**两条规则互相否定，无解**，除非改门禁契约。

### E2 每个特性必须有自己的插件文件、至少 1 条命令、1 份 capability

- `commandGroups()` 以 `src-tauri/src/*_plugin.rs` 文件名派生分组；F-装配点要求 `(groups[name] ?? []).length > 0`，A5 另有"每插件至少 1 条命令"的下限探针。
- B2 要求 `MOUNTS` 的 `id` 集合与磁盘插件文件全等，因此形态开关必须在挂载表登记、必须有 `lib.rs` 的 `mod <id>_plugin`、`cmd/mod.rs` 的 `#[cfg] mod <id>`（B6）。
- B8 要求每特性一份 `capabilities/<id>.json` 且 `default.json` 不得再授予。

结论：一个"只控制窗口开不开"的形态开关，被迫要**新建后端插件文件、注册至少一条命令、登记挂载表、加 `lib.rs` mod 行、加 `cmd/mod.rs` 两行、新建 capability 文件**。语义上也不成立——"开窗口"不是一个功能域。

### E3 形态开关的落点被 A2 挡死

窗口注册表在 `src-tauri/src/window.rs`：

```
static WINDOW_SPECS: LazyLock<HashMap<&'static str, WindowSpec>> = LazyLock::new(|| { ... });
```

`WindowSpec` 是静态表，加 vault 条目必然要写 `#[cfg(...)]`。而 A2 的 `cfg_location_allowed` 只认两个位置：`cmd/mod.rs` 与同名 `<name>_plugin.rs`。**在 `window.rs` 写 cfg 会直接让 `a2_feature_cfg_only_in_boundary_files` 失败**。

这不是缺陷，而是既有契约在说：**形态若需要条件编译，就必须把"条件"下沉到本体（插件文件），装配层只做遍历**。当前 `window.rs` 尚未提供者化，所以任何"给窗口加 cfg"的引入方式都会撞墙。

### E4 cfg 配额已顶格，零余量

`FEATURE_CFG_LIMIT = 3`，现状恰好 3 行（`vault_plugin.rs` 2 + `cmd/mod.rs` 1）。B4 与 B6 双向夹住这个数字：`vault_plugin.rs` 必须是一对互斥块（2 行），`cmd/mod.rs` 每特性必须恰 1 条。**任何新形态一旦再引入 cfg，立刻越界**，且不存在"再压一行"的余地。这从配额角度再次说明：形态不应消耗 cfg 配额。

### E5 新增第二个插件不再被 build.rs 阻塞，但成本并未下降

v1 的 E5（"`build.rs` 单真源、单前缀、单插件，第二个插件必 panic"）**已作废**：`build.rs` 现按 `plugin_ids()` 目录遍历，`COMMAND_PREFIX_TEMPLATE` 按 id 生成前缀，新增 `*_plugin.rs` 无需改脚本。

但这不构成 D2 的翻案。形态开关的真实成本清单（全部由既有断言强制）：

| 装配点 / 足迹 | 改动 | 断言来源 |
|---|---|---|
| `config/features.json` | +1 条目 | F-装配点、B2 |
| `src-tauri/Cargo.toml` | +1 行 `[features]` | B2 子集断言 |
| `src-tauri/src/lib.rs` | +1 行 `mod x_plugin` | F-装配点 |
| `src-tauri/src/feature_mounts.rs` | +4 行表项 | B2、B7 |
| `src-tauri/src/cmd/mod.rs` | +2 行（cfg + mod） | B6 |
| `src/features/registry.ts` | +3 行守卫 | F-装配点、F-动态 import |
| `src/vite-env.d.ts` | +1 行 declare | F-env 声明 |
| `capabilities/x.json`（新建） | 一份授权文件 | B8 |
| `src-tauri/src/x_plugin.rs`（新建） | 至少 1 条命令 + 一对互斥块 | A5、B4、B5 |

合计 **7 文件 11 行 + 2 个新文件**，只为表达"窗口开不开"。而 E1 早已把这条路封死（连字符 id 无法通过 env 命名断言），E3 又禁止在 `window.rs` 落 cfg。三重约束叠加，**形态与特性体系不相容**。

### 推论：形态属于运行期，不属于编译期

窗口是否可打开，本质是 `show_or_create(app, label)` 查表成功与否——一个**运行期**决策。要"某构建里没有 vault 窗口"，只需 vault 特性关闭（此时插件不注册、命令不存在、前端模块不加载）。要"vault 在但窗口不可开"，那是**运行期配置**（如 `VaultConf` 的偏好项）或入口是否渲染的问题，与二进制裁剪无关。

因此与现有门禁相容的落点只有两层：
- **编译期**：只保留 `vault` 一个特性，窗口 spec 由 `vault_plugin.rs` 内的提供者函数返回（关闭态返回空切片）——消耗 cfg 在插件文件内，符合 A2；
- **运行期**：是否允许/默认打开独立窗口，用配置表达。

**当前产品的补充事实**：vault 的"解锁窗口"实际是 `assistant_security::unlock_dialog` 的原生对话框（`cmd/vault/lifecycle.rs` 的 `vault_show_unlock_window`），不是 webview 窗口，也不进 `WINDOW_SPECS`。也就是说 `window.rs` 提供者化在当前**没有消费方**——这条改造只在"vault 真要独立 webview 窗口"时才需要启动，属于 D2' 的预留能力，不是待办缺口。

---

## 4 判定三：D3「多产物用特性 id」——不成立

三条硬约束都在 **package 级**，feature 无法穿透：

| 约束 | 证据 | 为什么 feature 无效 |
|---|---|---|
| 前端上下文路径固定 | `lib.rs` `generate_context!("tauri.conf.json")` | 宏参数是字面路径，不能按 feature 切换；两份 conf 需要两个 `generate_context!` 调用点 |
| `build.rs` 每 package 仅一份 | `src-tauri/build.rs` | cargo 不支持按 feature 提供不同 build script；只能在同一脚本内分支，而 `tauri_build::try_build()` 的产物（`gen/schemas/acl-manifests.json`）写到固定目录，会被不同 feature 的构建互相覆盖 |
| capabilities 是 package 级 | `capabilities/default.json` + `capabilities/<id>.json` | `windows` 数组是静态声明，无法 cfg；按特性拆文件只是**文件级隔离**，仍在同一 package 的 ACL 产物里 |

补充事实：`build.rs` 已泛化为遍历 `src/*_plugin.rs`，但它**依然完全不读 `CARGO_FEATURE_*`**，无条件为每个插件文件声明 `InlinedPlugin`。所以 ACL schema 与特性开关是**解耦**的——ACL 永远描述全部插件，特性只决定命令是否真的注册。这个解耦是有意的（否则 A7 读到的 `gen/schemas` 会随构建历史漂移），但也意味着**ACL 不是裁剪机制**，不能指望它承担产物隔离。

结论：D3 一旦需要，只能升级为独立 package（各自 `Cargo.toml` + `build.rs` + conf + capabilities）。放在同一 package 内的多 bin 方案会在 conf / ACL / 契约生成三处产生"同一目录两种语义"，属于不可维护的复杂度。

---

## 5 判定四：装配层零 cfg + 本体互斥块——成立且已被强制

现状满足度：

| 检查项 | 现状 | 判定 |
|---|---|---|
| 装配层（`lib.rs` / `plugins.rs` / `lifecycle.rs` / `ipc.rs` / `setup.rs` / `window.rs`）cfg 数 | 0 | 达标（A1 守 `ipc.rs`，B3 + F-遍历化反向守其余） |
| 挂载表 cfg 数 | 0（`feature_mounts.rs` 全文无 `#[cfg(`） | 达标（B1） |
| 本体 cfg 数 | 3（== 上限 3） | 达标但**零余量**（B4） |
| 互斥块签名一致性 | `init` / `on_setup` / `on_exit` 三函数两侧同签名 | 达标（B5） |
| 命令表隔离 | 69 条全在 `vault_plugin.rs`，`ipc.rs` 85 条宿主命令零 cfg | 达标（A1、A6） |
| 特性权限隔离 | `capabilities/vault.json` 只含 `vault:default`；`default.json` 不含 | 达标（B8） |
| **`window.rs` 提供者化** | **未做**，`WINDOW_SPECS` 是单条目静态表 | **未达标**，但当前无消费方（见 §3 推论末段） |

前六项说明这套手法在 vault 上已经跑通；最后一项是引入独立 webview 窗口时**唯一**会撞门禁的位置。判定：手法本身成立，未覆盖的只有 `window.rs`，且该项当前无消费方（真出现独立窗口需求才相关）。

需要注意 A2 的一个副作用：`cfg_location_allowed` 用 `relative.rsplit('/').next()` 比对**文件名**，因此 `src-tauri/src/cmd/vault/mod.rs`（本体子文件）**不允许**出现特性 cfg。这使"所有 cfg 集中在 `_plugin.rs`"成为硬约束——与"本体可自由组织"存在张力，新增特性时的事实约束之一。

---

## 6 判定五：preset 枚举受支持组合——部分成立

**成立的一面**：把"自由"从 2^N 收敛为"显式列举 + 静态验证"，且不引入新真源——preset 只写 feature 集合，仍然由 `features.json` 决定语义。

**不成立的一面**：若 preset 里写入 `onlyWindow` / 产物名 / cargo 参数 / 图标等，就变成与 `features.json` 并列的第二真源，两者必然漂移。这正是现有机制一直在避免的失败模式（`feature-build.ps1` 的 `Assert-FeatureConsistency()` 与 B2 的三方全等断言都是为了消除双真源）。

**附带约束**：脚本现有 `-Target` 已被 rust target triple 占用，新参数只能换名；且 `-Preset` 与 `-Features` / `-Without` 需互斥，否则同一份语义有两个输入。

**依赖关系**：preset 只有在 D1 有 ≥2 个特性时才有意义。当前只有 `vault`，此时引入 preset 属于为空集建索引。

---

## 7 判定六：前端 loader 代码生成——不成立

- I24（已归档）§3.7 明确否决过代码生成（原 R5：生成器 ↔ 生成物漂移），并已选择"清单手写 + 每特性手改 `registry.ts`"。
- 当前每特性前端成本是 **4 行**：`registry.ts` 的 `if (__X__) { const {...} = await import(...); modules.push(...); }` 3 行 + `vite-env.d.ts` 的 `declare const __X__: boolean` 1 行。
- 若改为生成物，前提是**不入库**（否则漂移必然发生），但 `ASSEMBLY_POINTS` 断言读的是文件内容，生成物不入库会让断言在干净检出时失效。

结论：收益是"省 4 行/特性"，代价是"新增漂移面 + 断言复杂化"，两者不对称。《feature-mount-traversal-spec.md》（已归档） §10 已把它列为条件化升级项（N≥3 时才重新评估）。

---

## 8 门禁覆盖缺口（影响"自由"的可信度）

这些缺口与引入方式无关，但决定"可选编译是否真的可信"。状态按当前代码标注：

| # | 缺口 | 证据 | 状态 |
|---|---|---|---|
| G-1 | 无"依赖真的断开"断言 | 仓库内无 `--no-default-features` 的自动断言（该串只出现在构建脚本文档与参数构造处），只能人工跑 `cargo tree` | **仍开**（L1 已人工实测，未固化为 CI） |
| G-2 | capability 与**窗口**无关联断言 | B8 只断言"存在 + 含 `<id>:default` + `default.json` 不含"，不校验 `windows` 数组与特性是否该开 | **部分关闭**：拆分与存在已断言，"某特性专属窗口是否被正确授权"仍无守护 |
| G-3 | A6 / A7 硬编码 vault 语义 | 已泛化：A5/A7/b2 全部由 `manifest_feature_ids()` / `plugin_feature_names()` 驱动 | **已关闭** |
| G-4 | A2 按文件名匹配 | `cfg_location_allowed` 只认 `cmd/mod.rs` 与 `<name>_plugin.rs`（连字符先转下划线） | **仍开**（本体子目录不得有 cfg；该点已在本文 §5 声明） |
| G-5 | preset 闭包无断言 | 不存在 `presets.json` | **仍开**（引入 preset 时需同时具备"preset ⊆ 声明特性 + requires 闭包"断言，当前两者皆无） |
| G-6 | 产物探针缺失 | 已补 `scripts/assert-feature-absent.mjs` + F-产物探针用例 | **已关闭** |
| G-7 | 产物探针的作用域有限 | 脚本只扫 `src/features/<id>/` 内的引号字面量；用户文案若搬到 `src/locales/` 则无字面量可扫 | **仍开**（脚本已对"未提取到探针字面量"做非零退出，但未覆盖文案外移场景） |

其中 **G-2 与独立窗口直接相关**：`capabilities/vault.json` 的 `windows` 是 `["main","orb"]`，vault 若出现专属窗口，加进这份 capability 会连带获得 `global-shortcut` / `autostart` / `always-on-top` 等它并不需要的权限（这些权限写在 `default.json` 里，`vault.json` 只授 `vault:default`，但窗口级授权与权限集在 Tauri 中按 capability 文件整体生效）；若新建独立 capability，又没有任何断言守护它。这一取舍的定案时点在引入之前（属规格职责），引入后再补没有对应机制。

---

## 9 反例推演（用现有断言验证结论）

| 反例 | 会失败的断言 | 说明 |
|---|---|---|
| 在 `window.rs` 给 vault 窗口加 `#[cfg(feature = "vault")]` | **A2** | cfg 越界，白名单只有 `cmd/mod.rs` 与 `_plugin.rs` |
| 在 `config/features.json` 加 `"vault-window"` 条目 | **F-装配点**（env 命名） | `${name.toUpperCase()}_ENABLED` 生成含连字符的非法 env 名 |
| 为形态新建 `vault_window_plugin.rs` 但不注册命令 | **A5** + **F-装配点**（分组非空） | 每插件至少 1 条命令，且分组集合必须等于声明特性 |
| 为形态新建插件却漏登记挂载表 | **B2** | 清单键、`MOUNTS` 的 `id`、磁盘插件三方必须全等 |
| 为形态新建插件却漏建 capability | **B8** | 每特性必须有 `capabilities/<id>.json` |
| 在 `feature_mounts.rs` 里按开关删表项 | **B1** | 表内任何 `#[cfg(` 即失败（表内容必须与开关无关） |
| 在 `plugins.rs` / `lifecycle.rs` / `setup.rs` 直接点名 `vault_plugin::init()` | **B3** + **F-遍历化反向** | 调用方只允许遍历 `mounts()` |
| 把 vault 窗口加进 `capabilities/default.json` 的 `windows` | 无断言拦截（但 `vault:default` 会被 B8 拦） | 权限过宽且无守护——属于"能过但不对" |
| 新增第二份 `tauri.conf.json` 供第二 bin | 编译期即失败 | `generate_context!` 字面路径；`build.rs` 每 package 一份；ACL 产物目录冲突 |
| 新增第二个 `*_plugin.rs` 且不改 `build.rs` | **不再失败** | 遍历化后 `build.rs` 自动识别；成本改由 B2/B6/B8/F-装配点 强制（见 §3 E5 表） |
| 新增第二个 `*_plugin.rs` 但 `lib.rs` 漏 `mod` | **F-装配点** + 编译失败 | 装配点齐备性断言与模块树同时报错 |

前三条是"引入方式不成立"的直接证明；后三条是"产物 / 多特性"的边界证明；中间四条是遍历化新增的门禁护栏。

---

## 10 功能可裁剪性核对（按判据逐项对照）

### 10.1 判据：可裁剪的五个必要条件

| 判据 | 含义 | 违反后的后果 |
|---|---|---|
| **C1 领域自包含** | 命令 / 状态 / 存储 / 前端可整体收进一个插件边界，core 只经挂载表或空实现接触它 | 裁剪时必须在 core 里散布 cfg，撞 A2 |
| **C2 无静态声明依赖** | 不依赖 `tauri.conf.json` 的 `windows` 数组、托盘菜单硬编码条目、`capabilities` 的静态窗口列表、`assetProtocol` scope | 这些位置无法 cfg，关闭态仍会创建窗口 / 显示菜单项 |
| **C3 缺失可优雅降级** | 设置快照可无该分类、导航可为空、关闭态的调用方不可达 | 运行期找不到页面 / 设置页报错 |
| **C4 依赖闭包可整体消失** | 可用 `dep:` 门控，且不被任何 core crate 反向依赖 | 关闭后 crate 仍被编进二进制，裁剪只是"不注册" |
| **C5 契约可裁剪** | DTO 与命令不进必填契约集，或关闭态不产出 | 契约生成出孤儿类型 / 前端引用悬空 |

五条全部满足才是"低成本可选"；缺 C2 的可选化成本会陡增（要先把静态声明外移）；缺 C4 的只能做到"不注册"，不是"不编译"。

### 10.2 不可裁剪（裁剪后产品不成立）

| # | 组 | 代码位置 | 为什么不可裁剪 |
|---|---|---|---|
| N1 | 窗口宿主与外壳 | `tauri.conf.json` 的 `main` 窗口、`src-tauri/src/window.rs`、`src/layouts/AppShell.tsx`、`src/components/Titlebar.tsx` | `main` 窗口是静态声明（C2 反向），且 `show_or_create` / 几何持久化 / 关闭到托盘是全部交互的地基 |
| N2 | 设置系统内核 | `src/settings/core/**`（`snapshot.ts` / `confRuntime.ts` / `fieldRegistry.ts` / `pageRegistry.ts` / `definePage.ts`） | 所有特性的设置页、字段、conf 槽位都挂在这套注册表上；移除它等于移除扩展面本身 |
| N3 | 配置内核 | `src-tauri/crates/assistant-config/**` | `AppConf` / `shortcut` 是启动必需；且 A8 断言 `register_conf_applier` 分组集合 == `*_conf` 模块集合，config 侧一旦按特性裁剪，A8 立刻失败 |
| N4 | IPC 契约层 | `src-tauri/src/protocol/**`、`src/types/protocol.contract.ts`、`src/services/{tauriInvoke,featureIpc}.ts`、`contract:gen` | 全部命令的 DTO 与 TS 契约由同一份生成链产出，裁剪会破坏"契约 ↔ 实现"的对应关系 |
| N5 | 存储基座 | `src-tauri/crates/assistant-storage/**`、`DbState` / 迁移 | 会话、提供商、工具历史、附件共用一套库与迁移序列，拆开等于拆数据库 |
| N6 | 主对话面 | `cmd/{chat,agent,provider,conversation,workspace}.rs`、`src/chat/**`、`src/stores/chat*` | 产品主体；且 `ipc.rs` 的 **85 条**核心命令里绝大多数属于这一组 |

补充一条**结构性事实**：`ipc.rs` 被 A1 要求"零 cfg"，而 `ipc.rs` 是核心命令的唯一注册表。这意味着任何被 feature 化的功能，其命令**只能搬离 `ipc.rs`**（搬到 `<name>_plugin.rs`）。因此"留在 `ipc.rs` 里"本身就是"不可裁剪"的等价标记——85 条核心命令所在的域，与可选化不相容。

### 10.3 可选且判据已满足

| # | 功能 | 满足的判据 | 现有证据 | 裁剪后代价 |
|---|---|---|---|---|
| O1 | **vault** | C1–C5 全满足 | `vault_plugin.rs` 唯一 cfg 边界（3 行 cfg 全在此与 `cmd/mod.rs`）；3 个 crate 由 `dep:` 门控；`cargo check --no-default-features` 实测通过且依赖图中无 `assistant-vault`；设置快照已有 `vaultless` 基线；capability 已拆分 | 已完成，无需再改 |
| O2 | **clock** | C1 满足、C3 满足、C4 满足（无独立 crate）；C2 部分违反（`lib.rs` 的 `pub mod clock`、`setup.rs` 的 `ClockRuntime::manage`） | `cmd/clock.rs` 自包含；前端在 `src/tools/clock/**` 内自成一体（10 个文件）；`ipc.rs` 中 6 条命令 | 需把 6 条命令从 `ipc.rs` 搬进插件、把 `manage` 下沉进插件 `setup`、前端调用改为 `plugin:clock\|...`（`featureIpc` 已支持该 wire 形式） |
| O3 | **纯前端小工具** | 无 Rust 依赖，C4 天然满足 | `src/tools/registry.ts` 共 **23** 个工具，仅 `clock`（6 命令）与 `fs-link`（走 `os_tool_exec`）需要 Rust；其余 **21** 个是纯前端 + 懒加载 | **不需要 cargo feature**，只需前端裁剪；但 `feature-build.ps1` 的反向断言要求"清单条目必须有 cargo feature"，走清单体系就要配一个空 feature（纯净成本） |

O3 是本文最主要的发现：**大多数"可选功能"不在二进制裁剪范畴内**。它们是前端 chunk，`lazyWithRecovery` 已经懒加载，关闭时只需不注册；做成 cargo feature 只会额外付出 §3 E5 表里的 7 文件 11 行。

### 10.4 可选但受 C2 静态声明约束

| # | 功能 | 受谁约束 | 约束证据 | 现状代价 |
|---|---|---|---|---|
| B1 | **orb** | `tauri.conf.json` 静态窗口 + 托盘硬编码 + `window.rs` 直接引用 | `tauri.conf.json` 的 `windows` 数组含 `orb`（无法 cfg）；`tray.rs` 两处菜单构造（第 32、155 行附近）都硬编码 `show_orb` 项；`window.rs` 第 24 行 `use assistant_orb::placement`；`setup.rs` 第 91、95 行无条件 `position_orb_window` / `show_orb_window` | 9 条核心命令（`get_orb_conf`、`toggle_orb_visibility`、`set_orb_click_through`、`snap_orb_to_screen`、`snap_orb_to_edge`、`cancel_orb_snap`、`expand_orb_panel`、`collapse_orb_panel`、`show_orb_context_menu`）+ 1 个 crate + 前端 `src/orb/**` + `OrbWindow` + `App.tsx` 的 viewMap |
| B2 | **mascot** | `assetProtocol` scope 静态 + conf applier 无条件 | `tauri.conf.json` 的 `assetProtocol.scope.allow` 含 `$APPDATA/mascot-assets/**`；`cmd/mod.rs` 的 `conf_side_effects()` 无条件播种 `mascot` 分组 | 3 条核心命令（`get_mascot_conf`、`install_mascot_asset`、`uninstall_mascot_asset`）+ `cmd/mascot_asset.rs` + 前端 `src/mascot/**` |

两条额外事实让 B1/B2 的成本被低估：

1. **mascot 在功能上隶属于 orb**：`src/orb/OrbBall.tsx` 是 `MascotRenderer` 的唯一渲染宿主，`AppearanceSettings` 里的 `OrbBallSection` 是唯一设置入口。二者在产品语义上是一个簇（单独裁 mascot 无产品意义）。
2. **A8 + `conf_side_effects` 构成硬锁**：`app` / `orb` / `mascot` 三组 applier 在 `cmd/mod.rs` 的 `call_once` 里无条件播种，而 A8 要求"已注册分组集合 == `assistant-config` 的 `*_conf` 模块集合"。要把 `orb` / `mascot` 变成可选，就同时要在 `cmd/mod.rs` 和 `cmd/config.rs` 加 cfg——但 **A2 的 cfg 白名单只有 `cmd/mod.rs` 与 `<name>_plugin.rs`，不含 `cmd/config.rs`**；且 F-足迹冻结对**新特性期望空集**，`cmd/config.rs` 出现在 vault 的历史 allowlist 里已被明确标注"不是新特性可复制的先例"（该文件现已退出 allowlist，判据不变）。结论：B1/B2 在当前门禁下**无法低成本落地**；前置条件是"静态声明外移"与"config 分组按特性归属"，两者当前都不存在。

### 10.5 裁剪后语义不成立（不构成可裁剪域）

| # | 功能 | 理由 |
|---|---|---|
| X1 | OS 工具（`assistant-tools`、`os_tool_exec`、`src/tools/os.ts`、`fs-link`） | 与 agent 工具调用同源，是 agent 主体能力的一部分；裁剪后 agent 能力残缺，产品语义不成立 |
| X2 | vault 的子能力（sync / HIBP / SSH agent / screen lock / biometric） | 拆二级特性会撞 env 命名断言（`${name.toUpperCase()}_ENABLED` 要求 id 为单段小写），且每个子特性都要付出 §3 E5 表的固定成本，收益仅为几 MB 依赖 |
| X3 | 通用 Tauri 插件（notification / opener / dialog / log / single-instance / global-shortcut / autostart） | 体积小、被 core 交互直接依赖（托盘、快捷键、外部打开），裁剪收益低于回归风险 |
| X4 | 主题 / 语言 / 导航 / 命令面板 / 历史 | 属 N1–N6 的外围，裁剪后主窗口无法自洽 |

### 10.6 三条扩展通道（现状事实）

仓库里已有三条互不重叠的扩展通道；功能落在哪条通道，决定它是否触碰 cargo 特性：

| 通道 | 载体 | 已用者 | 是否消耗 cargo feature | 事实判据 |
|---|---|---|---|---|
| **A 特性模块** | `FeatureModule`（`routes` / `sidebar` / `categories` / `settings` / `confs` / `ipc`） | vault | 是 | 有 Rust 命令或独立 crate（C1/C4 成立） |
| **B 工具注册表** | `src/tools/registry.ts` 的 `defineTool` | 23 个工具 | **否**（21 个零 Rust） | 纯前端、单页工具 |
| **C 设置注册表** | `definePage` / `fieldRegistry` / `section` | 6 个内置设置页 | 否 | 只是设置项，不构成独立域 |

对应关系：有 Rust 命令且能整体收进插件边界（C1）的功能落在 A；只有前端的功能落在 B，不触碰 cargo 特性。这条对应关系是机制的现状，不是取舍。

### 10.7 门禁约束点清单

| # | 约束点 | 影响的候选 | 证据 |
|---|---|---|---|
| P1 | `ipc.rs` 零 cfg（A1） | 全部 | 命令必须搬离核心注册表才能可选化 |
| P2 | cfg 白名单仅 `cmd/mod.rs` + `<name>_plugin.rs`（A2） | orb / mascot / clock | `cmd/config.rs` 与 `lib.rs` / `setup.rs` / `window.rs` 均不可写特性 cfg |
| P3 | A8 要求 applier 分组 == `*_conf` 模块 | orb / mascot | 二者 applier 与 conf 结构体都是无条件编译 |
| P4 | env 命名断言 `${name.toUpperCase()}_ENABLED`（F-装配点） | 任何多段 id | `vault-window` 类 id 不可用（详见 §3 E1） |
| P5 | `tauri.conf.json` 静态 `windows` / `assetProtocol` | orb / mascot | 静态声明无法 cfg |
| P6 | 托盘菜单硬编码 | orb | `tray.rs` 两处菜单构造 |
| P7 | 足迹冻结对新特性期望空集（F-足迹冻结） | 全部新特性 | 不得复制 vault 的历史 core 文件豁免 |
| P8 | 插件命令 wire 形式为 `plugin:<ns>\|` | clock 等从 core 迁出的功能 | 前端 service 必须改用 `featureInvoke` |
| P9 | cfg 配额顶格（B4，上限 3） | 任何给现有特性加 cfg 的方案 | vault 已占满 3/3，无余量可借 |

### 10.8 分级核对结果

| 分类 | 成员 | 一句话依据 |
|---|---|---|
| **不可裁剪** | 窗口宿主、设置内核、配置内核、IPC 契约、存储基座、主对话面 | 移除任一，产品不成立；且它们占据 `ipc.rs` 的绝大部分命令，而该文件被 A1 禁止 cfg |
| **可选·判据全满足** | vault | 唯一同时满足 C1–C5 的域，已有实测证据与完整门禁 |
| **可选·判据基本满足** | clock | 自包含，但需把 6 条命令搬出 `ipc.rs`、`manage` 下沉、前端改 wire 形式 |
| **可选·不需要 feature** | 21 个纯前端工具 | 只有 chunk，没有二进制；做成 cargo feature 是净成本（7 文件 11 行） |
| **可选·受静态声明约束** | orb + mascot（在产品语义上是一个簇） | 撞 C2（静态窗口 / scope）与 P2/P3/P9（cfg 白名单、applier 断言、配额） |
| **裁剪后语义不成立** | OS 工具、vault 子能力、通用 Tauri 插件、外壳外围 | 裁剪破坏产品语义，或收益与回归风险不对称 |

## 11 总结

| 自由度 | 与现有机制相容的表达层级 | 理由 |
|---|---|---|
| **D1 功能集合** | 特性 id（`config/features.json` + `Cargo.toml [features]`） | 单源可派生、装配点可枚举（6 条）、可选性只落一处（挂载表 + 互斥块） |
| **D2 形态** | **运行期**（窗口注册表提供者 + 运行期配置） | 窗口可开与否是运行期查表；编译期只需"特性关闭则整体消失"，无需第二个 id |
| **D3 产物** | **package 级**（独立 crate / 独立 conf + build.rs + capabilities） | 三处约束均为 package 级，feature 无法穿透 |

**一句话判定**：现有机制对 D1 是充分且必要的；对 D2、D3 则不是"不适合"，而是**层级错配**——把运行期决策和 package 级决策硬塞进编译期特性，会让 D1 那套已经咬合好的门禁（A2 白名单、B4 cfg 配额 3/3 顶格、env 命名、B2 三方全等、B6 每特性 1 条 cfg）全部失效或被迫放宽。

由此得到的相容形态：**一个 `vault` 特性不变，窗口注册改由本体提供者模式，形态差异交给运行期配置**；D3 的独立 package 层级只在"vault 作为独立产品单独发安装包"这一事实出现时才相关。这两条是层级事实，采用与否属规格职责。
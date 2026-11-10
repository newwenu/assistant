# 可选功能二进制编译：引入方式的合理性分析

> 日期：2026-11-10｜性质：**预先分析**（只判定是否合理，不含执行计划）
> 判据来源：`docs/principle/CODE-REVIEW-PRINCIPLE.md`、`docs/principle/CODE-SPLIT-PRINCIPLE.md`
> 本文所有结论以**当前代码**为准；与 I24-feature-extension-spec（已归档）冲突处已单独标注（归档设计稿允许滞后）。

---

## 0 结论速览

命题按三个正交自由度拆开判定，混在一起谈必然误判：

| # | 命题 | 判定 | 一句话依据 |
|---|---|---|---|
| **D1** | 「某个功能在不在」用特性 id 表达 | **合理** | 声明层 / 派生层 / 门禁层已闭环，边际成本被断言固化 |
| **D2** | 「同一功能以什么入口呈现」（如 vault 独立窗口）用第二个特性 id 表达 | **不合理** | 形态开关的落点在 `window.rs`，而该文件被 A2 断言禁止出现 cfg；且 env 命名断言拒绝连字符 id |
| **D2'** | 形态改为**运行期**决策（窗口注册表 + 运行期配置） | **合理** | `WindowSpec` 注册表与 `show_or_create` 本就是运行期机制，与"是否编译进去"无关 |
| **D3** | 「出几个可执行文件」用特性 id 表达 | **不合理** | `generate_context!` 路径固定、`build.rs` 每 package 一份、ACL 是 package 级产物，feature 无法安全切换 |
| **D3'** | 多产物升级为**独立 package** | **合理** | 唯一能隔离 conf / build.rs / capabilities 的层级 |
| — | 前端 loader 代码生成 | **不合理** | I24 §3.7 已因"生成器 ↔ 生成物漂移"否决；当前每特性仅 4 行，收益为负 |
| — | preset 枚举受支持组合 | **有条件合理** | 只允许列 feature 集合；一旦写入产物名 / cargo 参数即成为第二真源 |

**核心判断**：真正需要编译期标定的只有 **D1（功能集合）**。D2 是运行期问题，D3 是 package 级问题——把它们塞进特性体系，代价不是"多一个开关"，而是**与现有三道门禁直接冲突**（见 §3、§4 的反例推演）。

---

## 1 代码事实基线

### 1.1 声明层

| 事实 | 证据 | 数值 |
|---|---|---|
| 特性清单只有 1 条，三键结构 | `config/features.json` | `vault` / `default:true` / `cargoFeature:"vault"` / `env:"VAULT_ENABLED"` |
| cargo 侧镜像 | `src-tauri/Cargo.toml` 第 136–141 行 | `default = ["vault"]`、`vault = [...]` |
| 前端常量自动派生 | `vite.config.ts` `featureDefines()` | 遍历清单，产出 `__${spec.env}__` |
| 构建期注入 | `scripts/feature-build.ps1` `Set-FeatureEnvironment()` | 遍历清单，写 `1` / `0` |
| 清单 ↔ cargo 双向断言 | `scripts/feature-build.ps1` `Assert-FeatureConsistency()` | 正向 + 反向 + 默认集合相等，任一不符即中止 |

这一层是**单源 + 镜像 + 断言**结构，新增一条特性只需在清单加一段、cargo 加一行，脚本主流程零改动（全部遍历清单）。

### 1.2 装配层

| 事实 | 证据 | 数值 |
|---|---|---|
| 装配点被硬编码为 8 条 | `src/features/__tests__/coreBoundary.test.ts` `ASSEMBLY_POINTS` | `features.json` / `vite-env.d.ts` / `registry.ts` / `Cargo.toml` / `build.rs` / `capabilities/default.json` / `lib.rs` / `plugins.rs` |
| 后端 cfg 现状 | `src-tauri/src/**` 中 `#[cfg(...feature = "...")]` | **5 行**：`vault_plugin.rs` 2 行、`cmd/mod.rs` 3 行 |
| 装配层已零 cfg | `lib.rs` / `plugins.rs` / `lifecycle.rs` / `ipc.rs` / `setup.rs` / `window.rs` | 全部 0 行 |
| 空桩手法已落地 | `vault_plugin.rs` 第 93 行 `#[cfg(not(feature="vault"))] init()`；`cmd/mod.rs` 第 42 行 `vault_cleanup_on_exit` 空函数 | 2 处 |
| 命令清单从插件真源派生 | `build.rs` `plugin_commands("src/vault_plugin.rs")` | 单文件、单前缀 `crate::cmd::vault::` |

### 1.3 门禁层

| 断言 | 位置 | 约束内容 |
|---|---|---|
| A1 | `assembly_contract.rs` | `ipc.rs` 内不得出现任何 `#[cfg]` |
| A2 | 同上 | 特性 cfg **只允许**出现在 `cmd/mod.rs` 或与特性同名的 `<name>_plugin.rs`；vault 总行数 ≤ `VAULT_CFG_LIMIT = 6` |
| A3 | 同上 | `lib.rs` ≤ 40 行且不得内联 `generate_handler!` |
| A4 | 同上 | 插件注册表 ↔ ACL 双向求差，且不同插件命令不得重叠 |
| A6 | 同上 | 宿主与插件命令不得重叠；`vault_cleanup_on_exit` 桩与真实现签名归一化后必须一致 |
| A7 | 同上 | 命令名 ↔ 权限标识符派生规则；`build.rs` 必须引用每个 `<name>_plugin.rs` 真源 |
| A8 | 同上 | `register_conf_applier` 分组集合 == `assistant-config` 的 `*_conf` 模块集合 |
| F-装配点 | `coreBoundary.test.ts` | 每个声明特性必须齐备 8 个装配点 + 2 处本体，且 `env` 必须等于 `${name.toUpperCase()}_ENABLED` |
| F-双向一致 | 同上 | `commandGroups()` 分组名与 `*_plugin.rs` 文件名，集合必须等于声明特性集合；`permissions/*` 目录分支仍保留但已随目录删除而恒空（见 §1.4） |
| F-足迹冻结 | 同上 | 特性名在 core 文件中出现的集合必须与 `FEATURE_FOOTPRINT_ALLOWLIST` 逐字相等（vault 保留 7 个历史文件，且明确注明"非新特性可复制先例"） |

### 1.4 已实现与文档的偏差（以代码为准）

| 项 | 文档说法 | 代码事实 | 影响 |
|---|---|---|---|
| 装配点计数 | I24 v2.8 §10.1 已记为 **8 个装配点 + 2 处本体** | `ASSEMBLY_POINTS` 恰为 8 条，与文档一致 | **无偏差**；§2 与 §10 的"每特性成本"均按 8 条计 |
| ACL 覆盖窗口 | I24 §11.1 称 `["main","orb","settings"]`（三窗口） | 代码只有两个窗口：`src/config/constants.ts` 仅定义 `WINDOW_MAIN` / `WINDOW_ORB`，`window.rs` 的 `WINDOW_SPECS` 只注册 `main`；`capabilities/default.json` 为 `["main","orb"]` | 该行确为归档遗留（`settings` 是主窗口内的页面而非独立窗口）；本文不采信"三窗口"表述 |
| `permissions/<name>/default.toml` | I24 v2.8 §10.1 已标注"不再是必建文件"，由 `build.rs` 的 `AllowAllCommands` 烘焙 | `src-tauri/permissions/` 根目录为空且**已被删除**；`assembly_contract.rs` 的 ACL 断言读的是 `gen/schemas/acl-manifests.json`，`coreBoundary.test.ts` 的目录遍历以 `exists()` 守卫 | 无功能影响：删除后该分支退化为空集，双向一致断言实际只剩"分组名 + 插件文件名"两路；**结论：该目录对运行期与门禁均无作用，不应再作为装配点** |
| vault cfg 行数 | G1 目标 ≤6（三白名单 ≤8） | 实际 5 行，两白名单 | 已达进阶目标 |

---

## 2 判定一：D1「功能集合用特性 id」——合理

**理由不是"已有先例"，而是三条机制已经互相咬合**：

1. **单源可派生**：`vite.config.ts` 与 `feature-build.ps1` 都遍历 `config/features.json`，没有任何手写开关名。新增特性不需要改这两个文件。
2. **装配点可枚举**：8 条装配点被 `ASSEMBLY_POINTS` 固化，每特性成本 = 1 行 / 1 段，与命令条数无关（A7 使命令清单由 `build.rs` 派生）。
3. **可选性只体现在一处**：`vault_plugin.rs` 用一对 cfg 包住自身，`cmd/mod.rs` 用空桩提供永远存在的入口，因此装配层零 cfg——这正是 A2/A1 在守的东西。

**已实测支撑**：`cargo check --no-default-features` 零错误通过，且 `cargo tree --no-default-features -i assistant-vault` 确认依赖图中不存在该 crate。这说明"关闭 = 真的不编"，不是"关闭 = 编了但不用"。

**边界判断**：D1 的"自由"应定义为「受支持组合可枚举 + 机制统一」，而不是「2^N 组合全可编译」。后者会把门禁成本推高到不可维护。

---

## 3 判定二：D2「形态用第二个特性 id」——不合理

这是本轮最重要的否定结论。五条证据都来自现有门禁，不是风格偏好：

### E1 env 命名断言拒绝连字符 id

`coreBoundary.test.ts` 断言：

```
entry.env !== `${name.toUpperCase()}_ENABLED`  →  problems.push(...)
```

若 id 为 `vault-window`，期望 env 是 `VAULT-WINDOW_ENABLED`——**不是合法环境变量名**。`feature-build.ps1` 的 `Get-FeatureTable` 另有 `^[A-Z][A-Z0-9_]*$` 校验，必然拒绝。于是只能把 env 写成 `VAULT_WINDOW_ENABLED`，而这条又违反上述断言。**两条规则互相否定，无解**，除非改门禁契约（改契约的代价高于收益）。

### E2 每个特性必须有自己的插件文件且至少注册 1 条命令

`commandGroups()` 以 `src-tauri/src/*_plugin.rs` 文件名派生分组；`coreBoundary.test.ts` 要求分组名集合 == 声明特性集合，且 `(groups[name] ?? []).length === 0` 即报错。A4 又要求该插件命令必须获得 ACL 授权。

结论：一个"只控制窗口开不开"的形态开关，被迫要**新建一个后端插件文件、注册至少一条命令、在 capabilities 里加一行授权**。为运行期开关付出 4 个装配点，语义上也不成立——"开窗口"不是一个功能域。

### E3 形态开关的落点被 A2 挡死

窗口注册表在 `src-tauri/src/window.rs`：

```
static WINDOW_SPECS: LazyLock<HashMap<&'static str, WindowSpec>> = LazyLock::new(|| { ... });
```

`WindowSpec` 是静态表，加 vault 条目必然要写 `#[cfg(...)]`。而 A2 的 `cfg_location_allowed` 只认两个位置：`cmd/mod.rs` 与同名 `<name>_plugin.rs`。**在 `window.rs` 写 cfg 会直接让 `a2_feature_cfg_only_in_boundary_files` 失败**。

这不是缺陷，而是既有契约在说：**形态若需要条件编译，就必须把"条件"下沉到本体（插件文件），装配层只做遍历**。当前 `window.rs` 尚未提供者化，所以任何"给窗口加 cfg"的引入方式都会撞墙。

### E4 cfg 配额已接近上限

`VAULT_CFG_LIMIT = 6`，现状 5 行（`vault_plugin.rs` 2 + `cmd/mod.rs` 3）。若形态再引入一对 cfg，立刻达到 6，任何后续调整都没有余量。这从配额角度再次说明：**形态不应消耗 cfg 配额**。

### E5 `build.rs` 是单真源、单前缀、单插件

```
const PLUGIN_SOURCE: &str = "src/vault_plugin.rs";
const COMMAND_PREFIX: &str = "crate::cmd::vault::";
```

`plugin_commands()` 若解析不到以该前缀开头的条目会 **panic**。也就是说 `build.rs` 目前**结构上只支持一个插件**；A7 又要求它必须引用每个 `<name>_plugin.rs`。新增第二个插件必然要把它改成"按特性参数化 + 循环"，这是**既有装配点的改造**，不是"加一行"。

### 推论：形态属于运行期，不属于编译期

窗口是否可打开，本质是 `show_or_create(app, label)` 查表成功与否——一个**运行期**决策。要"某构建里没有 vault 窗口"，只需 vault 特性关闭（此时插件不注册、命令不存在、前端模块不加载）。要"vault 在但窗口不可开"，那是**运行期配置**（如 `VaultConf` 的偏好项）或入口是否渲染的问题，与二进制裁剪无关。

因此 D2 的正确落点是：
- **编译期**：只保留 `vault` 一个特性，窗口 spec 由 `vault_plugin.rs` 内的提供者函数返回（关闭态返回空切片）——消耗 cfg 在插件文件内，符合 A2；
- **运行期**：是否允许/默认打开独立窗口，用配置表达。

这样既满足"独立窗口形态"，又不新增任何特性 id、不消耗 cfg 配额、不动 `build.rs` 的插件数量。

---

## 4 判定三：D3「多产物用特性 id」——不合理

三条硬约束都在 **package 级**，feature 无法穿透：

| 约束 | 证据 | 为什么 feature 无效 |
|---|---|---|
| 前端上下文路径固定 | `lib.rs` `generate_context!("tauri.conf.json")` | 宏参数是字面路径，不能按 feature 切换；两份 conf 需要两个 `generate_context!` 调用点 |
| `build.rs` 每 package 仅一份 | `src-tauri/build.rs` | cargo 不支持按 feature 提供不同 build script；只能在同一脚本内分支，而 `tauri_build::try_build()` 的产物（`gen/schemas/acl-manifests.json`）写到固定目录，会被不同 feature 的构建互相覆盖 |
| capabilities 是 package 级 | `capabilities/default.json` | `windows` 数组是静态声明，无法 cfg；给独立窗口单独授权只能靠**新增 capability 文件**（这是文件级隔离，不是 feature 级） |

补充事实：`build.rs` 当前**完全不读 `CARGO_FEATURE_*`**，它无条件声明 vault 内联插件。所以 ACL schema 与特性开关是**解耦**的——ACL 永远描述全部插件，特性只决定命令是否真的注册。这个解耦是有意的（否则 A4 读到的 `gen/schemas` 会随构建历史漂移），但也意味着**ACL 不是裁剪机制**，不能指望它承担产物隔离。

结论：D3 一旦需要，只能升级为独立 package（各自 `Cargo.toml` + `build.rs` + conf + capabilities）。放在同一 package 内的多 bin 方案会在 conf / ACL / 契约生成三处产生"同一目录两种语义"，属于不可维护的复杂度。

---

## 5 判定四：装配层零 cfg + 本体空桩——合理且已被强制

现状满足度：

| 检查项 | 现状 | 判定 |
|---|---|---|
| 装配层（`lib.rs` / `plugins.rs` / `lifecycle.rs` / `ipc.rs` / `setup.rs`）cfg 数 | 0 | 达标 |
| 本体 cfg 数 | 5（≤6） | 达标 |
| 空桩签名一致性 | A6 断言通过 | 达标 |
| 命令表隔离 | 66 条全在 `vault_plugin.rs`，`ipc.rs` 零 cfg | 达标（A1） |
| **`window.rs` 提供者化** | **未做**，`WINDOW_SPECS` 是单条目静态表 | **未达标** |

前三项说明这套手法在 vault 上已经跑通；最后一项是引入独立窗口时**唯一**会撞门禁的位置。判定：手法本身合理，缺的是把它应用到 `window.rs`。

需要注意 A2 的一个副作用：`cfg_location_allowed` 用 `relative.rsplit('/').next()` 比对**文件名**，因此 `src-tauri/src/cmd/vault/mod.rs`（本体子文件）**不允许**出现特性 cfg。这会把"所有 cfg 集中在 `_plugin.rs`"变成硬约束——合理，但与"本体可以自由组织"存在张力，新增特性时要意识到这一点。

---

## 6 判定五：preset 枚举受支持组合——有条件合理

**合理的一面**：把"自由"从 2^N 收敛为"显式列举 + 静态验证"，且不引入新真源——preset 只写 feature 集合，仍然由 `features.json` 决定语义。

**不合理的一面**：若 preset 里写入 `onlyWindow` / 产物名 / cargo 参数 / 图标等，就变成与 `features.json` 并列的第二真源，两者必然漂移。这正是现有机制一直在避免的失败模式（`feature-build.ps1` 的断言就是为了消除"清单 ↔ cargo"的双真源）。

**附带约束**：脚本现有 `-Target` 已被 rust target triple 占用，新参数必须换名；且 `-Preset` 与 `-Features` / `-Without` 必须互斥，否则同一份语义有两个输入。

**依赖关系**：preset 只有在 D1 有 ≥2 个特性时才有意义。当前只有 `vault`，此时引入 preset 属于为空集建索引。

---

## 7 判定六：前端 loader 代码生成——不合理

- I24 §3.7 明确否决过代码生成（原 R5：生成器 ↔ 生成物漂移），并已选择"清单手写 + 每特性 4 行手改 `registry.ts`"。
- 当前 `registry.ts` 的每特性成本是 3 行（`if (__X__) { import; push; }`），且该文件已在 `ASSEMBLY_POINTS` 白名单内，改动被门禁允许。
- 若改为生成物，必须保证**不入库**（否则漂移必然发生），但 `ASSEMBLY_POINTS` 断言读的是文件内容，生成物不入库会让断言在干净检出时失效。

结论：生成方案在"收益 = 省 3 行/特性"与"新增漂移面 + 断言复杂化"之间明显不划算。

---

## 8 门禁覆盖缺口（影响"自由"的可信度）

这些缺口与引入方式无关，但决定"可选编译是否真的可信"：

| # | 缺口 | 证据 | 后果 |
|---|---|---|---|
| G-1 | 无"依赖真的断开"断言 | 目前只靠人工执行 `cargo tree --no-default-features -i assistant-vault` | 关闭态若引入隐式依赖，CI 不会发现 |
| G-2 | capability 与窗口 / 特性无关联断言 | `coreBoundary.test.ts` 只检查 `default.json` 含 `<name>:default`；独立窗口的专属 capability 完全在断言覆盖之外 | 新增 capability 可静默漂移（漏授权 → 运行期 `Command not allowed`） |
| G-3 | A6 / A7 硬编码 vault 语义 | `CLEANUP_FN = "fn vault_cleanup_on_exit"`、`build.rs` 的 `PLUGIN_SOURCE` / `COMMAND_PREFIX` 均写死 vault | 第二个特性可跳过"桩签名一致"检查 |
| G-4 | A2 按文件名匹配 | `cfg_location_allowed` 只认 `cmd/mod.rs` 与 `<name>_plugin.rs` | 本体子目录不得有 cfg；这一点未在文档中声明，易被误犯 |
| G-5 | preset 闭包无断言 | 不存在 `presets.json` | 若引入，必须同时加"preset ⊆ 声明特性 + requires 闭包"的静态断言，否则 preset 自身可写错 |
| G-6 | 产物探针缺失 | 无"关闭态 chunk 数 = 0"的自动断言 | 常量折叠失效只能靠人工核对 `dist/assets` |

其中 **G-2 与独立窗口直接相关**：`capabilities/default.json` 的 `windows` 是 `["main","orb"]`，vault 窗口若被加进这个 capability，会连带获得 `global-shortcut` / `autostart` / `always-on-top` 等它并不需要的权限；若新建独立 capability，又没有任何断言守护它。这个取舍必须在引入前定下来，而不是引入后再补。

---

## 9 反例推演（用现有断言验证结论）

| 反例 | 会失败的断言 | 说明 |
|---|---|---|
| 在 `window.rs` 给 vault 窗口加 `#[cfg(feature = "vault")]` | **A2** | cfg 越界，白名单只有 `cmd/mod.rs` 与 `_plugin.rs` |
| 在 `config/features.json` 加 `"vault-window"` 条目 | **F-装配点**（env 命名） | `${name.toUpperCase()}_ENABLED` 生成含连字符的非法 env 名 |
| 为形态新建 `vault_window_plugin.rs` 但不注册命令 | **F-双向一致**（分组非空）、**A4** | 分组集合必须等于声明特性，且每插件须有 ACL 授权命令 |
| 把 vault 窗口加进 `capabilities/default.json` 的 `windows` | 无断言拦截 | 权限过宽且无守护——属于"能过但不对" |
| 新增第二份 `tauri.conf.json` 供第二 bin | 编译期即失败 | `generate_context!` 字面路径；`build.rs` 每 package 一份；ACL 产物目录冲突 |
| 新增第二个 `*_plugin.rs` 而不改 `build.rs` | **A7** + `build.rs` panic | A7 要求引用真源；`COMMAND_PREFIX` 写死 vault 前缀，解析为空即 panic |

前三条是"引入方式不合理"的直接证明；后三条是"产物 / 多特性"的边界证明。

---

## 10 功能必要性分级（哪些能 feature 化）

### 10.1 判据：可裁剪的五个必要条件

| 判据 | 含义 | 违反后的后果 |
|---|---|---|
| **C1 领域自包含** | 命令 / 状态 / 存储 / 前端可整体收进一个插件边界，core 只经槽位或空桩接触它 | 裁剪时必须在 core 里散布 cfg，撞 A2 |
| **C2 无静态声明依赖** | 不依赖 `tauri.conf.json` 的 `windows` 数组、托盘菜单硬编码条目、`capabilities` 的静态窗口列表、`assetProtocol` scope | 这些位置无法 cfg，关闭态仍会创建窗口 / 显示菜单项 |
| **C3 缺失可优雅降级** | 设置快照可无该分类、导航可为空、关闭态的调用方不可达 | 运行期找不到页面 / 设置页报错 |
| **C4 依赖闭包可整体消失** | 可用 `dep:` 门控，且不被任何 core crate 反向依赖 | 关闭后 crate 仍被编进二进制，裁剪只是"不注册" |
| **C5 契约可裁剪** | DTO 与命令不进必填契约集，或关闭态不产出 | 契约生成出孤儿类型 / 前端引用悬空 |

五条全部满足才是"低成本可选"；缺 C2 的可选化成本会陡增（要先把静态声明外移）；缺 C4 的只能做到"不注册"，不是"不编译"。

### 10.2 必要（core）：裁剪后产品不成立

| # | 组 | 代码位置 | 为什么不可裁剪 |
|---|---|---|---|
| N1 | 窗口宿主与外壳 | `tauri.conf.json` 的 `main` 窗口、`src-tauri/src/window.rs`、`src/layouts/AppShell.tsx`、`src/components/Titlebar.tsx` | `main` 窗口是静态声明（C2 反向），且 `show_or_create` / 几何持久化 / 关闭到托盘是全部交互的地基 |
| N2 | 设置系统内核 | `src/settings/core/**`（`snapshot.ts` / `confRuntime.ts` / `fieldRegistry.ts` / `pageRegistry.ts` / `definePage.ts`） | 所有特性的设置页、字段、conf 槽位都挂在这套注册表上；移除它等于移除扩展面本身 |
| N3 | 配置内核 | `src-tauri/crates/assistant-config/**` | `AppConf` / `shortcut` 是启动必需；且 A8 断言 `register_conf_applier` 分组集合 == `*_conf` 模块集合，config 侧一旦按特性裁剪，A8 立刻失败 |
| N4 | IPC 契约层 | `src-tauri/src/protocol/**`、`src/types/protocol.contract.ts`、`src/services/{tauriInvoke,featureIpc}.ts`、`contract:gen` | 全部命令的 DTO 与 TS 契约由同一份生成链产出，裁剪会破坏"契约 ↔ 实现"的对应关系 |
| N5 | 存储基座 | `src-tauri/crates/assistant-storage/**`、`DbState` / 迁移 | 会话、提供商、工具历史、附件共用一套库与迁移序列，拆开等于拆数据库 |
| N6 | 主对话面 | `cmd/{chat,agent,provider,conversation,workspace}.rs`、`src/chat/**`、`src/stores/chat*` | 产品主体；且 `ipc.rs` 84 条核心命令里绝大多数属于这一组 |

补充一条**结构性理由**：`ipc.rs` 被 A1 要求"零 cfg"，而 `ipc.rs` 是核心命令的唯一注册表。这意味着任何被 feature 化的功能，其命令**必须搬离 `ipc.rs`**（搬到 `<name>_plugin.rs`）。因此"留在 `ipc.rs` 里"本身就是"必要"的等价标记——84 条核心命令所在的域，默认都不该可选化。

### 10.3 可选且已具备条件

| # | 功能 | 满足的判据 | 现有证据 | 裁剪后代价 |
|---|---|---|---|---|
| O1 | **vault** | C1–C5 全满足 | `vault_plugin.rs` 唯一 cfg 边界；3 个 crate 由 `dep:` 门控；`cargo check --no-default-features` 实测通过且依赖图中无 `assistant-vault`；设置快照已有 `vaultless` 基线 | 已完成，无需再改 |
| O2 | **clock** | C1 满足、C3 满足、C4 满足（无独立 crate）；C2 部分违反（`lib.rs` 的 `pub mod clock`、`setup.rs` 的 `ClockRuntime::manage`） | `cmd/clock.rs` 自包含；前端在 `src/tools/clock/**` 内自成一体；仅 6 条命令 | 需把 6 条命令从 `ipc.rs` 搬进插件、把 `manage` 下沉进插件 `setup`、前端调用改为 `plugin:clock\|...`（`featureIpc` 已支持该 wire 形式） |
| O3 | **纯前端小工具** | 无 Rust 依赖，C4 天然满足 | `src/tools/registry.ts` 共 23 个工具，仅 `clock`（6 命令）与 `fs-link`（走 `os_tool_exec`）需要 Rust；其余 21 个是纯前端 + 懒加载 | **不需要 cargo feature**，只需前端裁剪；但要注意构建脚本的反向断言要求"清单条目必须有 cargo feature"，若要走清单体系必须配一个空 feature |

O3 是本轮最重要的发现：**大多数"可选功能"根本不属于二进制裁剪范畴**。它们是前端 chunk，`lazyWithRecovery` 已经懒加载，关闭时只需不注册；强行做成 cargo feature 会白白增加装配点。

### 10.4 可选但被 C2 阻塞（静态声明依赖）

| # | 功能 | 被谁阻塞 | 阻塞证据 | 现状代价 |
|---|---|---|---|---|
| B1 | **orb** | `tauri.conf.json` 静态窗口 + 托盘硬编码 + `window.rs` 直接引用 | `tauri.conf.json` 的 `windows` 数组含 `orb`（无法 cfg）；`tray.rs` 两处菜单构造都硬编码 `show_orb` 项；`window.rs` 第 24 行 `use assistant_orb::placement`；`setup.rs` 无条件 `position_orb_window` / `show_orb_window` | 9 条核心命令（`get_orb_conf`、`toggle_orb_visibility`、`set_orb_click_through`、`snap_orb_to_screen`、`snap_orb_to_edge`、`cancel_orb_snap`、`expand_orb_panel`、`collapse_orb_panel`、`show_orb_context_menu`）+ 1 个 crate + 前端 `src/orb/**` + `OrbWindow` + `App.tsx` 的 viewMap |
| B2 | **mascot** | `assetProtocol` scope 静态 + conf applier 无条件 | `tauri.conf.json` 的 `assetProtocol.scope` 含 `$APPDATA/mascot-assets/**`；`cmd/mod.rs` 的 `conf_side_effects()` 无条件播种 `mascot` 分组 | 3 条核心命令（`get_mascot_conf`、`install_mascot_asset`、`uninstall_mascot_asset`）+ `cmd/mascot_asset.rs` + 前端 `src/mascot/**` |

两条额外事实让 B1/B2 的成本被低估：

1. **mascot 在功能上隶属于 orb**：`src/orb/OrbBall.tsx` 是 `MascotRenderer` 的唯一渲染宿主，`AppearanceSettings` 里的 `OrbBallSection` 是唯一设置入口。二者应作为一个簇一起裁剪，单独裁 mascot 没有产品意义。
2. **A8 + `conf_side_effects` 构成硬锁**：`app` / `orb` / `mascot` 三组 applier 在 `cmd/mod.rs` 的 `call_once` 里无条件播种，而 A8 要求"已注册分组集合 == `assistant-config` 的 `*_conf` 模块集合"。要把 `orb` / `mascot` 变成可选，就必须同时给 `cmd/mod.rs` 和 `cmd/config.rs` 加 cfg——但 **A2 的 cfg 白名单只有 `cmd/mod.rs` 与 `<name>_plugin.rs`，不含 `cmd/config.rs`**；且 `coreBoundary.test.ts` 的足迹冻结对**新特性期望空集**，`cmd/config.rs` 出现在 vault 的历史 allowlist 里已被明确标注"不是新特性可复制的先例"。结论：B1/B2 在当前门禁下**无法低成本落地**，需要先做"静态声明外移"与"config 分组按特性归属"两项前置。

### 10.5 不建议 feature 化

| # | 功能 | 理由 |
|---|---|---|
| X1 | OS 工具（`assistant-tools`、`os_tool_exec`、`src/tools/os.ts`、`fs-link`） | 与 agent 工具调用同源，是 agent 主体能力的一部分；裁剪后 agent 能力残缺，产品语义不成立 |
| X2 | vault 的子能力（sync / HIBP / SSH agent / screen lock / biometric） | 拆二级特性会撞 env 命名断言（`${name.toUpperCase()}_ENABLED` 要求 id 为单段小写），且每个子特性都要付出"插件文件 + 命令搬迁 + ACL"的固定成本，收益仅为几 MB 依赖 |
| X3 | 通用 Tauri 插件（notification / opener / dialog / log / single-instance / global-shortcut / autostart） | 体积小、被 core 交互直接依赖（托盘、快捷键、外部打开），裁剪收益低于回归风险 |
| X4 | 主题 / 语言 / 导航 / 命令面板 / 历史 | 属 N1–N6 的外围，裁剪后主窗口无法自洽 |

### 10.6 三条扩展通道的分工

引入"自由编译"前必须先判断功能属于哪条通道，否则会把前端问题做成 cargo 问题：

| 通道 | 载体 | 已用者 | 是否消耗 cargo feature | 适用判据 |
|---|---|---|---|---|
| **A 特性模块** | `FeatureModule`（`routes` / `sidebar` / `categories` / `settings` / `confs` / `ipc`） | vault | 是 | 有 Rust 命令或独立 crate（C1/C4 成立） |
| **B 工具注册表** | `src/tools/registry.ts` 的 `defineTool` | 23 个工具 | **否**（21 个零 Rust） | 纯前端、单页工具 |
| **C 设置注册表** | `definePage` / `fieldRegistry` / `section` | 6 个内置设置页 | 否 | 只是设置项，不构成独立域 |

判据链：**有 Rust 命令 → 看能否整体收进插件边界（C1）→ 能则走 A；只有前端 → 走 B，不要做成 feature。**

### 10.7 阻塞点清单（按门禁编号）

| # | 阻塞点 | 影响的候选 | 证据 |
|---|---|---|---|
| P1 | `ipc.rs` 零 cfg（A1） | 全部 | 命令必须搬离核心注册表才能可选化 |
| P2 | cfg 白名单仅 `cmd/mod.rs` + `<name>_plugin.rs`（A2） | orb / mascot / clock | `cmd/config.rs` 与 `lib.rs` / `setup.rs` / `window.rs` 均不可写特性 cfg |
| P3 | A8 要求 applier 分组 == `*_conf` 模块 | orb / mascot | 二者 applier 与 conf 结构体都是无条件编译 |
| P4 | env 命名断言 `${name.toUpperCase()}_ENABLED` | 任何多段 id | `vault-window` 类 id 不可用（详见 §3 E1） |
| P5 | `tauri.conf.json` 静态 `windows` / `assetProtocol` | orb / mascot | 静态声明无法 cfg |
| P6 | 托盘菜单硬编码 | orb | `tray.rs` 两处菜单构造 |
| P7 | 足迹冻结对新特性期望空集 | 全部新特性 | 不得复制 vault 的 7 个历史 core 文件 |
| P8 | 插件命令 wire 形式为 `plugin:<ns>\|<cmd>` | clock 等从 core 迁出的功能 | 前端 service 必须改用 `featureInvoke` |

### 10.8 分级结论

| 分类 | 成员 | 一句话依据 |
|---|---|---|
| **必要（core）** | 窗口宿主、设置内核、配置内核、IPC 契约、存储基座、主对话面 | 移除任一，产品不成立；且它们占据 `ipc.rs` 的绝大部分命令，而该文件被 A1 禁止 cfg |
| **可选·已就绪** | vault | 唯一同时满足 C1–C5 的域，已有实测证据 |
| **可选·低成本** | clock | 自包含但需把 6 条命令搬出 `ipc.rs`、`manage` 下沉、前端改 wire 形式 |
| **可选·无需 feature** | 21 个纯前端工具 | 只有 chunk，没有二进制；做成 cargo feature 是净成本 |
| **可选·高成本（阻塞）** | orb + mascot（应作为一个簇） | 撞 C2（静态窗口 / scope）与 P2/P3（cfg 白名单、applier 断言） |
| **不建议** | OS 工具、vault 子能力、通用 Tauri 插件、外壳外围 | 裁剪破坏产品语义或收益低于回归风险 |

## 11 总结

| 自由度 | 正确的表达层级 | 理由 |
|---|---|---|
| **D1 功能集合** | 特性 id（`config/features.json` + `Cargo.toml [features]`） | 单源可派生、装配点可枚举、可选性只落一处 |
| **D2 形态** | **运行期**（窗口注册表提供者 + 运行期配置） | 窗口可开与否是运行期查表；编译期只需"特性关闭则整体消失"，无需第二个 id |
| **D3 产物** | **package 级**（独立 crate / 独立 conf + build.rs + capabilities） | 三处约束均为 package 级，feature 无法穿透 |

**一句话判定**：现有机制对 D1 是充分且必要的；对 D2、D3 则不是"不适合"，而是**层级错配**——把运行期决策和 package 级决策硬塞进编译期特性，会让 D1 那套已经咬合好的门禁（A2 白名单、cfg 配额、env 命名、装配点枚举）全部失效或被迫放宽。

因此"引入独立窗口"的合理形态是：**保持一个 `vault` 特性不变，把窗口注册改为本体提供者模式，形态差异交给运行期配置**；只有当 vault 需要作为独立产品单独发安装包时，才启动 D3 的独立 package 方案。
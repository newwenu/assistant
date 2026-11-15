# 可选特性零成本挂载：设计思路合理性分析

> 性质：分析。只判定"零成本挂载"这一目标本身能否成立、成立需要什么样的设计前提、哪些部分注定不可归零。不含实施步骤（实施规格见 `docs/store/feature-mount-traversal-spec.md`）。
> 前置分析：`docs/analysis/FEATURE-OPTIONAL-BINARY-ANALYSIS.md`（引入方式的合理性，D1/D2/D3 三自由度）
> 事实基线：仓库当前代码（逐点核对，更正记录见 §10）；`docs/store/vault-optional-build.md`（L1 已实施，L2~L5 未实施）
> 拆分判据一律引用 `docs/principle/CODE-SPLIT-PRINCIPLE.md`，本文不复述。

---

## 0 结论速览

| # | 命题 | 判定 | 依据 |
|---|---|---|---|
| 1 | "零成本"以"装配点个数"计量是伪命题 | **不成立**，必须先拆分成本口径 | 8 个装配点里混着"声明存在"与"接入运行期"两类语义（§1.1） |
| 2 | 声明成本可归零 | **不成立** | `[features]`、`dep:` 门控、`optional = true` 是 cargo 语义要求；清单条目是特性存在的定义（§1.2） |
| 3 | 挂载成本可归零（M=0） | **手写阶段不成立，生成阶段成立** | 手写可压到 4 文件 8 行；M=0 需要"清单 → 挂载表"生成器，N=1 时收益为负（§3、§5.5） |
| 4 | 前端"零手写 + 真删除 + 入库"三者可同时满足 | **成立，但只有一种形态** | 生成物内容只依赖清单、开关仍以字面量参与条件判断（§5） |
| 5 | 零成本的核心前提是"core 只遍历、不调名" | **成立，且是必要条件** | 空桩手法的唯一存在理由就是 core 直接调名（§4） |
| 6 | 零成本的代价是门禁从"存在性断言"退化为"派生一致性断言" | **成立** | 存在性断言正是"每特性手改 8 处"的固化形式（§7） |
| 7 | 全部装配点都能归零 | **不成立** | `tauri.conf.json` 的 `windows` 数组、capabilities 的静态文件语义无法从清单派生（§6） |
| 8 | core 足迹可归零（7 → 0） | **不成立**，下界 3 | `cmd/mod.rs` 的模块声明、`protocol/dto/config.rs` 的 DTO 契约、`protocol/events.rs` 的事件名（§4.3） |
| 9 | 关闭态"遍历 0 次" | **不成立**，只能做到"遍历命中空实现" | Rust 的表若 cfg-free，则表内容与开关无关（§4.2） |

一句话：**"零成本挂载"的可达形态是"调用型足迹归零 + 声明型足迹冻结"，不是"改动消失"**。手写阶段可把新增特性的 core 改动从 8 文件 13 行压到 6 文件 8 行，把调用型边从 5 条压到 0 条（见 §4.1、§10.1），把 `cfg` 从 5 行压到 3 行，把足迹从 7 文件压到 3 文件；**只有引入"清单 → 挂载表"生成器才能把 M 真正压到 0**，而那一步在 N=1 时是负收益（§5.5）。

---

## 1 成本计量模型

### 1.1 为什么现有 8 个装配点不是正确的计量单位

`src/features/__tests__/coreBoundary.test.ts` 的 `ASSEMBLY_POINTS` 列了 8 条，语义是"新增一个特性必须触碰的 core 文件"。但这 8 条不是同一种成本：

- `config/features.json` 的一条条目、`src-tauri/Cargo.toml` 的 `[features]` 声明——这是**让系统知道"存在这个特性"**，任何编译期特性机制都无法免除；
- `registry.ts` 的 `if` 分支、`plugins.rs` 的 `.plugin(...)`、`build.rs` 的 `plugin("vault", ...)`、`capabilities/default.json` 的一行授权——这是**让这个特性在运行期生效**，是真正可以压缩的部分。

两类混在一起计，只能得出"零成本不可能"；分开计，才能给出可达目标。

### 1.2 三个成本项

| 记号 | 定义 | 是否可归零 |
|---|---|---|
| **D 声明成本** | 让系统"知道存在这个特性"的最小改动：清单条目 + `[features]` 声明 + 每个可选依赖的 `dep:` / `optional = true` | 不可归零。最小 **2 项**，但**随依赖闭包线性增长**：当前 vault 一项即含 3 个可选 crate（`assistant-vault` / `assistant-sync` / `assistant-security`），共 2 + 6 行 |
| **M 挂载成本** | 让特性"在运行期真正生效"的 core 改动 | 可压缩：手写下界 4 文件；生成后 0 |
| **B 本体成本** | 特性自身边界内的文件：`src/features/<id>/**`、`src-tauri/src/cmd/<id>/**`、`src-tauri/src/<id>_plugin.rs`、`capabilities/<id>.json` | 不设上限，计入特性自身 |

"零成本挂载"的准确含义是 **M 的调用型部分 = 0**：新增特性不再需要在 core 里新增"调这个名字"的代码。M 的声明型部分（挂载表条目、`lib.rs` 的 `mod` 行）在无生成器时无法免除。

### 1.3 现状计数

| 装配点 | 归类 | 现状形态 | 证据 |
|---|---|---|---|
| `config/features.json` | D | 1 条目 | 当前仅 `vault` 一项 |
| `src-tauri/Cargo.toml` `[features]` + `default` | D | 1 行特性 + 3 行 `optional = true` | `vault = ["dep:assistant-vault", ...]` |
| `src/vite-env.d.ts` | M | 每特性 1 行 `declare const __X__` | 断言要求逐条存在 |
| `src/features/registry.ts` | M | 每特性 3 行 `if / import / push` | `loadFeatures()` 内 |
| `src-tauri/build.rs` | M | 写死 `PLUGIN_SOURCE` / `COMMAND_PREFIX` / 插件名 `"vault"` | 三个常量均为字面量，**当前结构上无法支持第二个插件** |
| `src-tauri/src/lib.rs` | M | 每特性 1 行 `mod <id>_plugin;` | 模块树静态声明 |
| `src-tauri/src/plugins.rs` | M | 每特性 1 行 `.plugin(<id>_plugin::init())` | `attach()` 链尾 |
| `src-tauri/capabilities/default.json` | M | 每特性 1 行 `"<id>:default"` | ACL 授权 |

**M = 6**（前端 4 + 后端 2），另有 `build.rs` 的"单插件写死"使 M 在 N≥2 时直接失效。

此外还有 **7 个足迹文件**（在 `FEATURE_FOOTPRINT_ALLOWLIST` 内、不在 `ASSEMBLY_POINTS` 内），它们是"core 调名"的另一半成本：

| 足迹文件 | 形态 | 类型 |
|---|---|---|
| `src-tauri/src/cmd/mod.rs` | `#[cfg(feature = "vault")] mod vault;` + `pub use` + `#[cfg(not(...))]` 空桩（3 行 cfg） | 调用型 |
| `src-tauri/src/lifecycle.rs` | `crate::cmd::vault_cleanup_on_exit(app_handle)` 直调 | 调用型 |
| `src-tauri/src/setup.rs` | 启动期 `VaultConf::load()` 播种 | 调用型 |
| `src-tauri/src/ipc.rs` | `cmd::get_vault_conf` 注册行 | 调用型（声明式写法，但是命令名） |
| `src-tauri/src/cmd/config.rs` | `get_vault_conf` 命令本体 | 调用型（同上，与上一行是同一条边的两半） |
| `src-tauri/src/protocol/dto/config.rs` | `VaultConfDto` | 声明型（契约）；**已由 `feature-config-ownership-spec.md` 迁入 vault 契约域，此项归零** |
| `src-tauri/src/protocol/events.rs` | 3 个 vault 事件名常量 | 声明型（跨端协议名） |

**已不存在的旧边**：`cmd/config.rs` 的 `apply_vault_side_effects` 直调已被 `register_conf_applier("vault", ...)` 取代——注册动作发生在 `vault_plugin::init()` 内，方向是"特性调 core"，不是"core 调特性"。这是遍历化思路**已被采纳过的先例**，也是本文判定它可行的直接依据。

---

## 2 可归零的四条必要条件

任何"派生式挂载"方案必须同时满足以下四条，缺一条就不成立。

| 判据 | 含义 | 违反后果 |
|---|---|---|
| **R1 构建期可枚举** | 派生源能在构建期被读取，且是唯一真源 | 派生源与清单不一致，产生第二个真源 |
| **R2 派生结果 cfg-free** | 挂载表不得引入特性 `cfg` | `cfg` 出现在 A2 白名单之外，装配契约直接失败 |
| **R3 不破坏真删除** | 前端开关仍须以字面量参与条件判断 | 关闭态仍产出 chunk，产物探针失败 |
| **R4 表内容与开关无关** | 同一份挂载表源码在开 / 关两种构建下逐字节相同 | 表内容随开关变化 → 表本身成了 cfg 的藏身处 |

R2 与 R4 是一对：**表要 cfg-free，就必须与开关无关**。这在 Rust 里引出一个硬约束（§4.2）：表引用的模块必须**无条件存在**，关闭态的差异只能落在特性边界文件**内部**（空实现），不能落在表上。前端不受此约束——`if (__X__)` 是常量折叠，不是源码分支。

R2 最容易忽略：派生代码若写成 `#[cfg(feature = "vault")] fn mount() {...}`，即便位于生成文件里，也违反"特性 cfg 只允许出现在 `cmd/mod.rs` 与 `<name>_plugin.rs`"的约束——`cfg` 的**位置**是契约，与文件是否生成无关。

---

## 3 逐点判定

| 装配点 | 目标形态 | 可归零 | 代价与风险 |
|---|---|---|---|
| `vite-env.d.ts` | 由清单生成 `declare` 块 | 可（需生成器） | 生成物入库 + 漂移断言；当前手写 1 行更便宜 |
| `registry.ts` | 由清单生成 mount 表，条目内保留 `if (__<ENV>__)` 字面量开关 | 可（需生成器） | 见 §5，形态唯一；当前手写 3 行更便宜 |
| `build.rs` | 扫描 `src/*_plugin.rs`，插件 id 取文件名、命令名取注册条目最后一段 | **可** | 顺带修复"单插件写死"（前置分析 E5）；**必须无条件生成 ACL**（§6.2） |
| `plugins.rs` | 遍历 `feature_mounts::mounts()` | **可** | 需表结构（§4.4）；该文件随即退出装配点集合 |
| `lib.rs` | 一次性加 `mod feature_mounts;`；每特性仍需 1 行 `mod <id>_plugin;` | **不可** | Rust 模块树静态声明，`mod` 必须由父模块写出 |
| `feature_mounts.rs`（新增） | 每特性 1 条目（`plugin` + `on_exit` 函数指针） | **不可**（手写） / 可（生成） | 表是唯一"core 知道特性名字"的数据处；生成后才归零 |
| `capabilities/default.json` | 拆为 `capabilities/<id>.json` | **不可**（tauri 读静态文件） | 降级为 B，同时解决窗口作用域问题（§6.1） |

结论（手写阶段）：**新增特性的 core 改动 8 文件 → 6 文件**（`features.json`、`Cargo.toml`、`vite-env.d.ts`、`registry.ts`、`lib.rs`、`feature_mounts.rs`），其中 D 占 2、M 占 4；M 行数 13 → 8（后端 4 行、前端 4 行）。调用型边 5 → 0。

---

## 4 核心手法：从"core 调名"到"core 遍历"

### 4.1 现状的五条"调名边"（已核对）

| 边 | 位置 | 形态 |
|---|---|---|
| 插件注册 | `src-tauri/src/plugins.rs` | `.plugin(crate::vault_plugin::init())` |
| 退出清理 | `src-tauri/src/lifecycle.rs` | `crate::cmd::vault_cleanup_on_exit(app_handle)` |
| 模块与空桩 | `src-tauri/src/cmd/mod.rs` | `#[cfg(feature = "vault")] mod vault;` + `pub use` + `#[cfg(not(...))]` 空桩 |
| 配置面命令 | `src-tauri/src/ipc.rs` + `src-tauri/src/cmd/config.rs` | `cmd::get_vault_conf` 注册行与命令本体 |
| 启动播种 | `src-tauri/src/setup.rs` | `VaultConf::load()` |

共同点：**core 知道特性叫什么名字**。这就是 `cmd/mod.rs` 里 3 行 `cfg` 与空桩存在的全部理由——core 要调用一个名字固定的函数，关闭态下这个名字必须仍然存在，于是只能造一个空桩。

### 4.2 遍历化之后：空桩退役，但"空表"不可达

把四条边都改成"遍历一个由特性提供的表"。这里必须更正一个常见的想当然：

| 设想 | 是否可达 | 原因 |
|---|---|---|
| 关闭态的表是空切片，core 遍历 0 次 | **不可达** | 表若 cfg-free（R2），表内容就与开关无关；空切片本身就是开关相关的源码分支 |
| 关闭态表仍含条目，但条目方法体是空实现 | **可达** | 差异落在特性边界文件内部（`<id>_plugin.rs` 的 `#[cfg(not(...))]` 分支），core 侧无感知 |

因此准确表述是：**core 只遍历、不调名；关闭态遍历到的是"空实现"，而不是"空表"**。行为等价于"这个特性不存在"（无命令、无启动副作用、无退出清理），但会留下两处残余：一个空插件注册、一份 ACL 条目（§6.2）。

**空桩是"core 调名"的伴生品**——这条推论仍然成立：只要 core 还在调名，关闭态就必须有同名空函数兜底。遍历化把调名边清零后，空桩才失去存在理由，`cmd/mod.rs` 的 3 行 cfg 才能降到 1 行。

### 4.3 连带收益与不可达目标

| 项 | 现状 | 遍历化后 |
|---|---|---|
| `cfg` 配额（`VAULT_CFG_LIMIT = 6`） | 5 行（`cmd/mod.rs` 3 + 插件文件 2） | **3 行**（`cmd/mod.rs` 1 + 插件文件 2）；钩子增加不再增加 cfg 行（§4.4） |
| 调用型足迹 | 5 个文件（`cmd/mod.rs`、`lifecycle.rs`、`setup.rs`、`ipc.rs`、`cmd/config.rs`） | **0** |
| 声明型足迹 | `protocol/dto/config.rs`、`protocol/events.rs` | **`protocol/dto/config.rs` 已归零**（配置 DTO 迁入 vault 契约域，见 `feature-config-ownership-spec.md`），`protocol/events.rs` 不变（协议名不是挂载边） |
| 足迹 allowlist | 7 文件 | **2 文件**（`cmd/mod.rs`、`protocol/events.rs`） |

**"足迹 7 → 0"不可达**，原因是剩余 2 个文件的"特性名字"不是挂载成本：

1. `cmd/mod.rs` 的 `#[cfg(feature = "vault")] mod vault;`——Rust 要求 `mod` 由父模块声明。把它挪到别处只会换文件（`lib.rs` 会因此进入足迹，更糟），或用 `#[path]` 下沉到挂载表（`cmd/vault` 内部的 `super::` 解析会改变，连带改动全部引用）。
2. `protocol/events.rs` 的 3 个事件名——跨端协议命名空间，集中声明是既有惯例；迁出仅省 1 个足迹文件，收益低于一致性损失。

> 原第 2 项「`protocol/dto/config.rs` 的 `VaultConfDto`」已由 `feature-config-ownership-spec.md` 收口：配置 schema 归特性 crate、配置 DTO 归特性契约域，因此该足迹归零，足迹目标由「3」进一步收缩为「2」。

判定：**足迹目标应从"0"改为"2"，并把足迹断言从"允许列表"改成"允许列表 + 类型说明"**——只允许声明型足迹，调用型足迹一律为 0。

### 4.4 契约形状的判定

需要的不是"每特性写一套装配代码"，而是**一个统一的挂载表**，条目在两种构建下都存在、行为由特性边界文件决定：

- 表本身零 `cfg`、与开关无关（R2 + R4）；
- 条目只含函数指针（`plugin` / `on_exit`）与 id 字符串，不含任何逻辑；
- 关闭态的空实现由 `<id>_plugin.rs` 内部的一对 `cfg` 提供，core 侧无分支。

判定：**合理**。它与前端 `FeatureModule`（一个特性 = 一份声明，聚合器是唯一构造点）在语义上对称，形成"前端一个模块、后端一张挂载表"的对应关系，符合 `CODE-SPLIT-PRINCIPLE` 的"一个文件只对应一个变化原因"。

附带收益：`cfg` 行数不再随钩子数量增长——把"开/关"两套实现各自包进一个 `mod mounted` 块，`cfg` 恒为 2 行，新增钩子只增加块内函数。这是本文对原方案的一处形态改进（原方案按"每钩子一对 cfg"计，钩子一多就顶破配额）。

---

## 5 前端三角：三个目标不能靠同一手法满足

### 5.1 三个目标

| 目标 | 含义 |
|---|---|
| **T1 零手写** | 新增特性不改 `registry.ts` / `vite-env.d.ts` |
| **T2 真删除** | 关闭态产物中不含该特性的 chunk |
| **T3 生成物入库** | 可 code review、可断言、IDE 可导航 |

### 5.2 三种候选形态的判定

| 形态 | T1 | T2 | T3 | 判定 |
|---|---|---|---|---|
| 现状：手写 `if (__VAULT_ENABLED__) { await import(...) }` | 否 | **是** | 是 | T1 不满足，这就是前端 M 的来源 |
| `import.meta.glob("./*/module.ts")` 枚举 | **是** | **否** | 是 | glob 会为所有匹配产出 chunk，关闭态 chunk 仍在产物里，违反产物探针 |
| 生成器按当前 env 决定"是否写入 import 语句" | **是** | **是** | 否 | 不同特性组合产出不同文件内容，无法入库；每次构建都需重跑生成 |

三者各缺一个角，没有任何一种能单独满足 T1+T2+T3。

### 5.3 唯一可行的折中

**生成物内容只依赖清单，不依赖 env**：每个条目仍写成

```
if (__<ENV>__) { ... await import("./<id>/module") ... }
```

- 文件内容对给定清单是确定的 → **T3 成立**（可入库、可 review、可漂移断言）；
- 构建时 `define` 把 `__<ENV>__` 替换成字面量 `false` → `if (false)` → Rollup 折叠掉动态 import → **T2 成立**；
- 手写成本为 0 → **T1 成立**。

判定：**合理且是唯一形态**。其成立的关键是"开关必须是字面量"——`define` 的替换结果必须是 `true` / `false` 这种常量，Rollup 才能做死代码消除。任何把开关做成运行期查表（`flags[id]`、`Record<string, boolean>`）的写法都会退化成不可折叠，T2 随即失效（见 §8 反例）。

### 5.4 与既有生成物范式的关系

仓库已有"生成 + 入库 + 断言"的先例：`npm run contract:gen` 产出 `src/types/protocol.contract.ts`，项目规则要求提交该文件、禁止手改。前端挂载表复用同一范式，属于既有工程习惯的延伸，不引入新范式。

### 5.5 盈亏平衡：为什么当前不启用生成器

前置分析（§7）已从**性价比**角度否决前端 loader 生成：每特性仅省 3 行，却新增"生成器 ↔ 生成物"漂移面，并让 `ASSEMBLY_POINTS` 断言在干净检出时失效。

本文补充**可达性**角度的结论，两者不冲突：

| 阶段 | N=1（当前） | N≥3~4 |
|---|---|---|
| 手写挂载 | M = 4 文件 8 行，可接受 | M 线性增长，且 `build.rs` 若不同步派生会先失效 |
| 生成挂载 | 新增 1 个生成器 + 1 条漂移断言，换来省 8 行 → **负收益** | 漂移断言边际成本不变，省下的行数与文件数线性增长 → **正收益** |

判定：**当前阶段选手写遍历化，把生成器作为 N≥3 时的条件化升级**。触发条件写成可判定形式：当"挂载表条目数 ≥ 3"或"`build.rs` 需要第二套派生规则"时启用生成。

---

## 6 不可归零项

| 项 | 不可归零的原因 | 处置 |
|---|---|---|
| `Cargo.toml` 的 `[features]` 与 `default` | cargo 要求特性静态声明；`dep:` 门控与 `optional = true` 只能写在这里，无任何派生机制 | 计入 **D**：2 行 + 每个可选依赖 2 行 |
| `tauri.conf.json` 的 `windows` 数组 | tauri 在编译期读取静态配置；它不是代码，无法参与派生；`main` 窗口必须静态存在 | 计入 **D/B**；独立窗口形态只能走"运行期创建"（见前置分析 §3 推论） |
| `capabilities` 的 `windows` 列表 | tauri 读取静态 capability 文件；`build.rs` 若去写该目录等于自修改源码树，构建顺序脆弱 | 降级为 **B**：`capabilities/<id>.json` |
| conf 分组播种（`app` / `orb` / `mascot` 的 applier） | A8 要求"已注册分组集合 == `*_conf` 模块集合"，这三组属 core 特性，不是挂载成本 | 与本文无关；只有把这三组变成可选特性时才谈得上 |

### 6.1 capabilities 按特性拆文件的两重收益

把 `"<id>:default"` 从 `capabilities/default.json` 挪到 `capabilities/<id>.json`，同时解决两件事：

1. **成本归属**：从 core 装配点转为 B，M 计数中消失；
2. **窗口作用域**：前置分析 G-2 指出"capability 与窗口/特性之间没有断言守护"。`default.json` 的 `windows` 是 `["main","orb"]`，若把独立特性窗口并进去，会连带获得 `global-shortcut` / `autostart` / `always-on-top` 等与之无关的权限。按特性拆文件后，每个特性文件可以只声明自己的 `windows` 列表。

代价是"新增一个文件"，但该文件属于特性本体，不构成挂载成本。

### 6.2 关闭态的三项残余（必须显式记账）

| 残余 | 形态 | 影响 |
|---|---|---|
| 空插件注册 | 关闭态 `mounts()` 仍遍历到条目，`plugin()` 返回只含名字的空插件 | 运行期多 1 个插件名，无命令、无权限、无行为 |
| ACL 条目 | `build.rs` 文本解析插件文件，关闭态仍生成该插件的 ACL manifest 与 `allow-*` 权限 | 权限存在但无对应命令，属惰性声明 |
| capability 文件 | `capabilities/<id>.json` 静态存在，其引用的 `<id>:default` 必须可解析 | **硬约束**：`build.rs` 的插件声明因此不能按 `CARGO_FEATURE_*` 门控，否则关闭态构建会因"权限不存在"失败 |

第三项是最容易踩的坑：capabilities 是静态文件、ACL 是构建产物，两者必须**同生共死**。选择"ACL 无条件生成"就把关闭态残余固定为惰性声明；选择"ACL 按开关门控"就必须同时放弃 capabilities 拆文件（回到 core 装配点）。

判定：**接受"ACL 无条件生成 + capabilities 拆文件"**，残余是惰性声明，不构成"特性仍被编译进去"的证据——真删除的判据始终是 `cargo tree --no-default-features`（§7.2）。

---

## 7 门禁改造：零成本的代价

### 7.1 现有断言正是成本的固化形式

`coreBoundary.test.ts` 的三条断言与 `assembly_contract.rs` 的 A1-A8，绝大部分是**存在性断言**：某文件里是否出现了某个字符串。

| 现有断言 | 实际证明的东西 |
|---|---|
| `lib.rs` 含 `mod vault_plugin` | 有人手写过了 |
| `plugins.rs` 含 `vault_plugin::init()` | 有人手写过了 |
| `capabilities` 含 `vault:default` | 有人手写过了 |
| `registry.ts` 含 `__VAULT_ENABLED__` | 有人手写过了 |
| A6 桩与真实现签名一致 | 有人写过空桩 |

这些断言**只能证明"装配动作发生过"**，不能证明"特性真的被排除了"。它们与"每特性手改 8 处"互为因果：因为要求逐处手写，所以逐处断言；因为逐处断言，所以不能改成派生。

### 7.2 换挡后必须替换的四类断言

| 类别 | 内容 | 替代的旧断言 |
|---|---|---|
| **反向足迹** | `plugins.rs` / `build.rs` / `capabilities/default.json` 退出装配点集合后，断言其**不含**任何特性 id；足迹 allowlist 从 7 降为 3 并标注类型 | 取代 `plugins.rs 含 init()` / `build.rs 含 "vault"` / `capabilities 含 "vault:default"` 三条 |
| **表-清单-磁盘三方一致** | 挂载表条目 id 集合 == 磁盘 `src/*_plugin.rs` 的 stem 集合 ⊆ `config/features.json` 的键集合 | 取代 A7 的"build.rs 引用了真源"字符串检查 |
| **结构约束** | 挂载表 cfg-free；关闭态实现只允许出现在 `<id>_plugin.rs`；插件命令名必须以 `<stem>_` 开头 | 取代 A6 的"桩与真实现签名一致" |
| **行为断言** | `cargo tree --no-default-features -i <crate>` 依赖断开；关闭态 chunk 数 = 0 | 补上前置分析 G-1 / G-6 两处缺口 |

判定：**净收益**。存在性断言与行为断言的证明力不在同一层级：前者证明"源码里写了什么"，后者证明"产物里真的没有什么"。换挡把断言从前者整体换成后者，**门禁强度上升而不是下降**——前提是三方一致断言必须落地。若只删旧断言、不加新断言，门禁会退化为空转，这比现状更差。

---

## 8 会破坏现有契约的"零成本"写法（反例）

| 反例 | 破坏点 |
|---|---|
| `import.meta.glob("./*/module.ts")` 枚举前端模块 | 关闭态仍产出 chunk，产物探针失败（T2） |
| 把 `cfg` 写进挂载表 / 派生出的装配代码 | 违反 R2；"位置"是契约，与是否生成无关 |
| 开关做成 `Record<string, boolean>` / `flags[id]` 运行期查表 | 失去字面量常量折叠，动态 import 无法被消除（T2）；且违反类型安全规范 |
| 在挂载表条目里放 `enabled: bool` 运行期字段 | 把编译期决策降级为运行期查表：关闭态插件仍被注册并参与分发，真删除失效 |
| `build.rs` 按 `CARGO_FEATURE_*` 门控插件声明 | 与 `capabilities/<id>.json` 的静态存在冲突，关闭态构建因"权限不存在"失败（§6.2） |
| 在 `build.rs` 里写 `src-tauri/capabilities/*.json` | 自修改源码树，构建顺序脆弱，且让"生成物"脱离 review |
| 用 `OUT_DIR` + `include!` 生成 `mod` 块 | A2 的源码扫描覆盖不到，`cfg` 逃逸门禁；IDE 不可导航 |
| 用 `linkme` / `inventory` 做链接期自注册 | 新增依赖；且 Rust 侧的"是否编译"是静态决策，链接期注册改变不了这一点 |
| 把 `mod <id>` 用 `#[path]` 下沉到挂载表以清零 `cmd/mod.rs` | `cmd/<id>/**` 内部的 `super::` 解析基准改变，连带改动全部引用；足迹只是换文件 |
| 让 core 保留调名、只把调用点包成 `if feature_enabled()` | 空桩仍在、`cfg` 配额仍紧、足迹仍非零——只换了调用形式，没消除成本 |

倒数第二条与最后一条最值得注意：它们都是"看起来零成本、实际成本不变或只是搬家"的典型。判据是 §4.2 那句话——**只要 core 还在调名，成本就没有消失**。

---

## 9 判定结论

| 命题 | 判定 |
|---|---|
| "零成本挂载"作为目标是可达的 | **部分成立**：调用型足迹可达 0，声明型足迹下界 3，M 手写下界 4 文件、生成后 0 |
| 前提一：core 只遍历、不调名 | **必要**，否则空桩与 `cfg` 配额无法退役 |
| 前提二：表 cfg-free 且与开关无关 | **必要**，否则 `cfg` 换个位置继续违约 |
| 前提三：关闭态差异落在特性边界文件内部 | **必要**（Rust 特有约束），"空表"不可达 |
| 前提四：前端开关保持字面量 | **必要**，否则真删除失效 |
| 前提五：生成物入库 + 漂移断言 | **仅在启用生成器时必要**；手写阶段不适用 |
| "零声明成本" | **不成立**，cargo 与清单语义要求静态声明，且随依赖闭包增长 |
| "零生成物" | **不成立**，模块树与 `vite-env.d.ts` 无法在无生成层时派生 |
| 现有三道门禁 | **需替换而非叠加**：存在性断言退役，反向足迹 / 三方一致 / 结构约束 / 行为断言上位 |
| 对 vault 的意义 | 把已完成的 L1 从"一个特例"提升为"一类机制"：调用型边 5 → 0、`cfg` 5 → 3、足迹 7 → 3（配置 DTO 迁出后进一步收缩为 7 → 2） |

最终判定：**设计思路合理，但必须成套落地**——"成本口径拆分 + 遍历化 + 门禁换挡"三件事缺一不可。**"M=0 + 足迹 0"是原方案的两个不可达目标**：前者在手写阶段不可达、在 N=1 时引入生成器是负收益；后者与 Rust 模块树、DTO 契约、协议命名空间冲突。可达目标应改写为 **"调用型足迹 0、声明型足迹 2、M 4 文件（N≥3 时转生成器归零）"**（声明型原为 3，配置 DTO 一项已由 `feature-config-ownership-spec.md` 归零）。

---

## 10 更正记录（对照当前代码基线）

| # | 原文档说法 | 代码事实 | 更正 |
|---|---|---|---|
| 1 | `cmd/config.rs` 的 `apply_vault_side_effects` 是第四条调名边 | 该函数已不存在；conf 副作用改由 `vault_plugin::init()` 调 `register_conf_applier("vault", ...)` 注册，方向是"特性调 core" | 删除该边，改列为**遍历化已落地的先例** |
| 2 | 调名边共 4 条 | 实为 5 条：`plugins.rs`、`lifecycle.rs`、`cmd/mod.rs`（cfg + 空桩）、`ipc.rs` + `cmd/config.rs`（`get_vault_conf`）、`setup.rs`（`VaultConf::load()`） | 补齐 `ipc.rs` / `cmd/config.rs` / `setup.rs` 三条 |
| 3 | 足迹目标 7 → 0 | 剩余 3 个文件的特性名字是声明型（模块树、DTO 契约、协议事件名），迁出会破坏既有规则或只是搬家 | 目标改为 **7 → 2**（配置 DTO 一项已由 `feature-config-ownership-spec.md` 迁入 vault 契约域而归零），并区分调用型 / 声明型 |
| 4 | "core 代码里不再出现任何特性名字"，同时要求生成含特性名的 `feature_mounts.rs` | 自相矛盾：表必然含特性名 | 改为"core 不再出现**调用型**特性名；唯一允许的是挂载表的数据条目" |
| 5 | "关闭态的表是空切片，core 遍历 0 次" | Rust 中 cfg-free 的表与开关无关，无法在关闭态变空 | 改为"表恒定，关闭态命中空实现"，并记账空插件 / ACL 残余 |
| 6 | "D 恒为 2 项" | 每个可选依赖还需 `dep:` + `optional = true` 两行；vault 一项即 3 个 crate | 改为"D = 2 项 + 依赖闭包线性增长" |
| 7 | "M 由 6 降到 0" | `lib.rs` 的 `mod <id>_plugin;` 与挂载表条目在手写阶段无法免除 | 改为"手写 4 文件 8 行；生成后 0" |
| 8 | `cfg` 5 → 2 行 | 保留 `cmd/mod.rs` 的 1 行 `mod` 后为 3 行；若按"每钩子一对 cfg"写，钩子增多会顶破配额 | 改为 **5 → 3**，并规定"双实现包进单个 `mod mounted`"形态 |
| 9 | 前端生成器为"零成本"的必要条件 | 前置分析已从性价比否决（每特性 3 行）；当前 N=1 | 改为**条件化升级**（N≥3 触发），手写阶段不启用 |

### 10.1 落地后口径复核

本分析给出的目标值已在规格 `docs/store/feature-mount-traversal-spec.md` 中实施完毕，实测与上文口径的对应关系如下（口径不变，数值均取实测）：

| 口径 | 分析预期 | 落地实测 | 复核 |
|---|---|---|---|
| 调名边 | 5 → 0 | 5 → 0（`plugins.rs` / `lifecycle.rs` / `setup.rs` 改为遍历；`ipc.rs` + `cmd/config.rs` 的 `get_vault_conf` 迁入 `cmd/vault/conf.rs` 并由插件注册；`cmd/mod.rs` 只剩 `mod` 声明） | 一致 |
| `cfg` 配额 | 5 → 3 | 3 行（`vault_plugin.rs` 互斥块 2 行 + `cmd/mod.rs` 1 行），常量由 `VAULT_CFG_LIMIT = 6` 收紧为 `FEATURE_CFG_LIMIT = 3` | 一致，且配额随之下调 |
| 调用型足迹 | 5 文件 → 0 | 0（B3 / F6 断言调用方不含特性名字面量） | 一致 |
| 声明型足迹 | 3 文件 | 2 文件（`cmd/mod.rs`、`protocol/events.rs`），`FEATURE_FOOTPRINT_ALLOWLIST` 由 7 项收缩为 2 项；原第 3 项 `protocol/dto/config.rs` 已由 `feature-config-ownership-spec.md` 迁入 vault 契约域而归零 | 一致（且优于分析预期） |
| 装配点 | 8 → 6 | 6（`plugins.rs`、`build.rs`、`capabilities/default.json` 退出逐特性集合：前者改遍历，后两者按文件 / 按特性名泛化） | 一致 |
| 手写 M | 4 文件 8 行 | 4 文件（`config/features.json`、`src-tauri/Cargo.toml`、`src/vite-env.d.ts`、`src-tauri/src/feature_mounts.rs` 条目）+ `lib.rs` 的 1 行 `mod <id>_plugin;` | 一致 |
| 关闭态残余 | 空插件 + ACL + capability 文件 | 三项均在；`cargo tree --no-default-features -i assistant-vault` 报包不在依赖图内，证明残余不引入运行时成本 | 一致 |
| 前端真删除 | 关闭态无特性 chunk | `VAULT_ENABLED=0` 构建后探针扫描 74 个产物 / 18 条 i18n 探针，零泄漏 | 一致 |

**唯一需要强调的口径细化**：`build.rs` 泛化后不再"每特性一行插件声明"，而是按 `*_plugin.rs` 文件名派生，因此它从"每特性必改"降为"仅解析规则变化时才改"——这是本分析未单列的一条收益，落地时由 A7 的改写断言与 B9 的块数判据共同守住。
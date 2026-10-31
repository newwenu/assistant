# I24 — 特性扩展通道通用化（Feature Extension v2）

> 阶段号：I24（I23 之后的下一个活跃阶段；子阶段 E0–E8，按第 5 章依赖顺序执行）
> 状态：设计稿 v2.4，E0–E8 未实施（v2.1–v2.4 只修订契约与口径，不扩大实施范围）
> 前置文档：`docs/store/vault-optional-build.md`（后端 L1 与前端 L2.5 已在代码中落地）
> 基线口径：本文所有结论以**当前仓库代码**为准，不引用文档中的旧描述
> 工作流说明：本文含两条**可独立评审、独立落地**的工作流——前端扩展面（3.1–3.7 + E0–E5 + E8）与后端装配（3.8 + E6–E7，含 E6a 的 ACL spike）。若要拆成两份文档，以 3.8 为界；两者之间只有 3.2 的 IPC 命名空间一处耦合（见 3.2.1）
> v2.1 修订：①补 IPC 命名空间契约，让前端配置槽位与后端插件边界对齐；②配置面「类型承诺 vs 运行时事实」对齐（`Partial` + 可用性判定 + lint 约束）；③`confPageMap` 从推断改为声明 + 双向校验；④校验策略改为「构建/测试期抛错，生产降级丢弃」；⑤新增 1.7「未覆盖面」并收紧「扩展面完整」的口径；⑥快照补相位（phase）契约；⑦口径修正（vault 命令 66→65、注册表条目 128→127、全后端 vault cfg 68→73、G1 上限按算术可达性重算、`lib.rs` 职责数 5→6）；⑧补 B7（`setup.rs` 条目级 cfg）并把它纳入 3.8.3 的插件 `.setup()` 下沉，使 G1 的 ≤6 有明确前提；⑨3.8.5 门禁扩为四条断言（含边际成本不回归）
> v2.2 修订（参考项目复核）：①**3.8.3 修正插件命令名**——`plugin:<ns>|<函数名>` 不剥 `vault_` 前缀（原文裁剪前缀会在运行期 command not found）；②**新增 3.8.3a ACL spike**——Tauri v2 插件命令受 capability 约束，G4/G5 的成本口径必须补上权限面；③**3.8.5 门禁从计数改为集合相等**——双向差集 + 解析器自检下限 + 三态（core / plugin / host-only）；④**3.8.4 补两条结构性断言**——「注册集合 == 遍历集合」与「桩签名与真实现同形」；⑤**3.5 校验改批量收集**——一次抛出全部违规而非 fail-fast；⑥**3.6 侧栏模型解耦**——`sidebar.key` 自证 + `page.navParent` 归属，`navOnly` 更名 `emptyPage` 并新增 `visibility`（v2.2 原定三值，v2.3 收缩为 `sidebar` / `deepLinkOnly` 两值）；⑦新增 3.9 记录参考项目的采纳项与反例
> v2.3 修订（合规与约束取舍）：①修正 `confs` 类型——`Partial<Record<ConfGroup, ConfOf<ConfGroup>>>` 会让每个键退化为联合类型，改为逐键收窄的映射类型 `ConfStore`（3.2）；②按项目规则修正 `as unknown as` 口径——允许 2 处而非「唯一」，且 `translateKey` 实际同时使用 `as never` 与 `as unknown as`（3.2 / 1.7）；③补「新增 conf group 必须同步 Rust struct + `#[serde(default)]` + TS 接口」的规则对齐（3.2）；④`toConfRecord` 现状是闭合 `ConfUnion`（已含 `VaultConf`），泛化与 `VaultConf` 迁出补进迁移表（第 4 章）；⑤文档内 2 处指向本机路径的绝对链接改回仓库相对路径（项目规则禁止文档写本机绝对路径）；⑥新增 3.10「约束的取舍：必要约束 vs 防御性约束」，把过度防御项降级或收缩，并说明本文为何出现大量「禁止 / 必须」句式
> v2.4 修订（2026-10-31 按当前仓库代码复核，仅校正事实与口径，不改设计）：①vault 命令实为 **66** 条（`cmd/vault/**` 66 个 `#[command]` 与注册表逐字相等，v2.1 的 66→65 修正有误，正文 65 一律按 66 读）；②注册表条目实为 **123** 条（core 57 + vault 66，`cmd::` 字面量逐行计数），cfg + 条目 **132** 行（注册表块 L37–L232 共 196 行，cfg 噪声占 67%）；③`lib.rs` 实为 **280** 行，迁移区间校正为 handler L37–L232 / menu L233–L244 / window L245–L264 / lifecycle L16–L21 + L265–L280；④G1 现状 **73** 行成立，构成为 `lib.rs` 68（注册表 66 + `Exit` 的 1 对）+ `setup.rs` 1 + `cmd/mod.rs` 2 + `cmd/config.rs` 2（正文一处「现状 70」按 73 读）；⑤当前 `SettingsPageDefinition` **没有 `navOnly` 字段**（全仓 0 处），3.1 规则 5 与 3.6 的「替代 navOnly」应理解为「新增 `visibility` / `emptyPage`」，6.1 的「旧 fixture 迁移」不成立（仓库尚无既有快照 fixture）；⑥**E4 与已落地的 `scripts/feature-build.ps1` 存在单源冲突**（该脚本已把 `src-tauri/Cargo.toml` 的 `[features]` 当唯一真源并注入 `<FEATURE>_ENABLED`），执行前须先择一，见 3.7 的 v2.4 注记

---

## 0. 结论速览

| # | 结论 | 依据 |
|---|---|---|
| 1 | 现有 `FeatureModule` 只覆盖「UI 挂载」三个面（route / sidebar / settings），**不覆盖配置面、IPC 面**（文案面见 1.7），因此「新增一个可选特性只加一个 `module.ts`」对 vault 之外的特性不成立 | `ConfGroup` 是闭合联合类型，`conf` 槽位、加载函数、命令名、类型映射全部硬编码在 core 的 9 处（C1–C9） |
| 2 | core 与 features 存在**双向运行时依赖**，当前不报错只因为其中一跳是 `import type` | `fieldRegistry.ts` / `pageRegistry.ts` 运行时 import `@/features/registry`；`aggregate.ts` 运行时 import `settings/core/definePage` |
| 3 | 「唯一条件展开点」名不副实：`features/registry.ts` 全文 14 行，vault 以字面量写死在 `if` 与 `aggregate([...])` 里 | 加第二个特性必须改 registry，等价于「把 N 处补丁搬进同一个文件」 |
| 4 | 类型层为了可扩展性放宽成 `string & {}`，但运行时**没有补上引用完整性校验**，扩展写错的表现是「UI 静默少一块」 | `pageTypes.ts` 放宽；`pageRegistry.ts` 用 `filter` 生成分类；`assertUnique` 只查重复，不查引用存在性 |
| 5 | 查询层每次调用重建数组，聚合器的收益在出口处被丢光 | `getAllDefinitions()` 每次 `[...definitions, ...features.fields]` + `assertUniqueDefinitions()` |
| 6 | 本文的改造目标不是「让 vault 能删」，而是「让**扩展面完整到 UI + 配置 + IPC 三面**」：新增特性应当只动 `features/<id>/` 与一份清单；文案面（i18n）本阶段不覆盖，归属见 1.7 | 见第 3 章契约与 6.3 演练 |
| 7 | 后端 `lib.rs` 的条目级 cfg 要**归零**，不是「压缩」：vault 走插件边界后，注册表内 `#[cfg]` = 0，全后端 cfg ≤ 8 行且只落在三个白名单文件（桩与 setup 下沉后可收紧到 ≤ 6） | 见 1.6 与 3.8（G1–G6 可计数）：现状 66 条命令各 1 行 cfg（连同条目共 132 行），全后端共 73 行 cfg |
| 8 | 前后端扩展契约**不对称但方向不同**：配置面命令（`get_vault_conf` / `patch_conf`）属于 core、不随特性开关消失，因此 conf 槽位用裸命令名是对的；缺的是「特性自有命令」的命名空间概念，6.3 探针因此无法零改动接入 | `cmd/config.rs` L151/L159 未加 cfg；`vault.service.ts` L1 只是 `import { tauriInvoke as invoke }`，70 处调用全部是裸命令名、文件内无 `plugin:` 前缀转发 |
| 9 | 用推断代替声明（`confPageMap` 由「第一个含该 group 字段的 section」推导）与本文「不变式靠结构而非约定」的主张自相矛盾 | 见 3.5（改为 `confSlot.pageId` 显式声明 + 双向校验） |

一句话：**可排除已经做到，可扩展还停在表面。本阶段把扩展面从「UI 三件套」补齐到「配置 + IPC + 构建 + 校验」，把依赖方向拉直，并把后端 `lib.rs` 从「六职责内联 + 66 条命令各带 cfg（共 132 行）」改成「40 行装配 + 插件边界，注册表内 cfg 归零」。**

---

## 1. 现状事实（代码证据）

### 1.1 配置面闭合（扩展通道只有一条腿）

| # | 事实 | 位置 |
|---|---|---|
| C1 | `ConfGroup = keyof typeof CONF_KEY_MAP`，`vault` 是字面量键 | `src/settings/core/field/fieldRegistry.ts` 的 `CONF_KEY_MAP` / `CONF_LOAD_KEY_MAP` |
| C2 | `ConfOf<G>` 是四层嵌套条件类型，新增 group 必须改类型 | `src/settings/core/settingsDispatcher.ts` 的 `ConfOf` |
| C3 | 四个具名槽位 + 四个加载函数写死在 store 接口里 | `src/stores/settingsStore.ts` 的 `SettingsState` |
| C4 | 命令名硬编码：`getVaultConf` 直接 invoke `get_vault_conf` | `src/services/settingService.ts` |
| C5 | 总览页手写 `confMap: Record<ConfGroup, ...>` 四个 group | `src/settings/overview/SettingsOverview.tsx` 的 `FieldDrivenSummary` |
| C6 | 命令面板手写 `GROUP_TO_PAGE: Record<ConfGroup, string>` | `src/components/commandPalette/commandAggregation.tsx` |
| C7 | 乐观更新通过 `CONF_KEY_MAP[group]` 找槽位 | `src/settings/core/optimisticUpdate.ts` |
| C8 | 跨窗口同步按 group 硬编码分支，并直接引用前端开关 `VAULT_ENABLED` | `src/services/settingEvents.ts` 的 `if (group === "app") ... else if (group === "vault" && VAULT_ENABLED)` |
| C9 | 「按 group 取 conf」的 hook 写了两份（总览页与 `SettingsGroup` 各一个 `useConfByGroup`） | `src/settings/overview/SettingsOverview.tsx`、`src/settings/shared/sections/SettingsGroup.tsx` |

C1–C9 构成一个事实：**「带配置项的可选特性」当前无法只靠 `FeatureModule` 接入**。vault 之所以只加一个 `module.ts` 就行，是因为它早已被预埋进 core 的九个位置。

### 1.2 依赖方向倒置

| # | 事实 | 位置 |
|---|---|---|
| D1 | core 运行时 import features | `src/settings/core/field/fieldRegistry.ts`、`src/settings/core/pageRegistry.ts` 的 `import { getFeatures } from "@/features/registry"` |
| D2 | features 运行时 import core | `src/features/aggregate.ts` 的 `import { definePage } from "@/settings/core/definePage"` |
| D3 | 不炸的原因：`aggregate.ts` 对 `fieldRegistry` 只做 `import type`（编译期擦除） | `src/features/aggregate.ts` 的 `SettingDefinition` / `SettingSectionDef` 引用 |

真实依赖图是 `settings/core ⇄ features`，与设计意图（单向）相反。一旦把类型校验或去重逻辑从 core 挪进 `aggregate`（很自然的下一步），`import type` 会变成运行时 import，形成真环 + 顶层 `const` 的 TDZ 风险。

### 1.3 不变式靠约定而非结构

| # | 事实 | 位置 | 后果 |
|---|---|---|---|
| I1 | `SettingsPageId` / `SettingsCategoryId` 放宽为 `string & {}` | `src/settings/core/types/pageTypes.ts` | 编译期不再校验 id 合法性 |
| I2 | 分类由 `pages.filter(p => p.categoryId === id)` 生成 | `src/settings/core/pageRegistry.ts` 的 `derive()` | 扩展声明 page 忘了声明 category → **页面静默消失** |
| I3 | `getSettingsPageManifest` 找不到时回落到 overview | 同上 | `/settings/vault` 在排除构建下渲染成总览而非未找到提示 |
| I4 | `getSectionById` / `getDefinitionsBySection` 只 filter | `src/settings/core/field/fieldRegistry.ts` | section 的 `pageId`、field 的 `section` 指向不存在的东西 → 静默为空 |
| I5 | 去重两套语义：内置 `defineSetting` **静默丢弃**重复；扩展 `aggregate` **直接 push**，靠 DEV 断言拦 | `fieldRegistry.ts` 的 `defineSetting` vs `aggregate.ts` | 同一个错误（key 重复）在两条通道表现不同（丢弃 vs 覆盖） |
| I6 | `assertUnique` 只在 `import.meta.env.DEV` 跑 | `src/features/aggregate.ts` | release 构建零校验 |
| I7 | 内置去重的缓存键是**长度签名** | `fieldRegistry.ts` 的 `assertUniqueDefinitions` | 同数量的不同扩展 → 签名不变 → 校验被跳过 |

### 1.4 单一事实源缺失

| # | 事实 | 位置 |
|---|---|---|
| S1 | 侧栏 `key` 缺省取 `routes[0].path`，选中态与跳转都挂在这条隐式规则上 | `src/features/aggregate.ts`；消费方 `src/components/AppSidebar.tsx` 的 `selectedKeys` |
| S2 | `SidebarSlot` 只有 `"after-tools"` / `"before-settings"` 两个值，无法表达「history 之后」 | `src/features/types.ts` |
| S3 | `order` 一个数值同时作用于 module / route / sidebar / category / page，模块级 `base + i` 会跨模块相撞 | `src/features/aggregate.ts` 的 `aggregate()` |
| S4 | 前端开关读 `process.env.VAULT_ENABLED`，后端读 cargo feature，`tauri.conf.json` 的 `beforeBuildCommand` 是裸 `npm run build` | `vite.config.ts`、`src-tauri/tauri.conf.json` |
| S5 | `features/registry.ts` 把 vault 写成字面量，无生成物、无清单 | `src/features/registry.ts` |

### 1.5 资产未 colocate（E8 已解决）

| # | 事实 | 位置 |
|---|---|---|
| A1 | `VaultSettingsPage` 在 `src/settings/pages/`，却 import `@/vault/core/*` | `src/settings/pages/VaultSettingsPage.tsx` |
| A2 | `BiometricSwitchRender` 在 `src/settings/shared/renderers/`，同样 import `@/vault/core/*` | `src/settings/shared/renderers/BiometricSwitchRender.tsx` |

后果：`src/settings/**` 不依赖 vault 这条不变式在文件系统层面已不成立，任何「按目录审计排除面」的手段失效，只能 grep。

E8 落地后：两个文件迁入 `src/features/vault/settings/`，`src/settings/**` 对 `@/vault` 的引用归零，不变式恢复为文件系统事实；`src/settings/**` 仅剩 `@/features/vault/module` 一处，位于快照黄金测试（聚合模块是测试输入，非生产依赖）。

### 1.6 后端（L1 已实施，遗留七项结构缺陷）

| # | 事实 | 位置 |
|---|---|---|
| B1 | 通配重导出仍在：`pub use vault::*`，vault 内任何新增 `pub fn` 自动进入 `cmd` 命名空间 | `src-tauri/src/cmd/mod.rs` |
| B2 | 66 条 vault 命令逐条挂 `#[cfg(feature = "vault")]`（66 行 cfg + 66 行条目 = 132 行），漏加靠扫描测试兜 | `src-tauri/src/lib.rs` L37–L232 |
| B3 | 副作用两套范式并存：vault 用 `#[cfg]` 双函数桩，orb 用注册表 | `src-tauri/src/cmd/config.rs` |
| B4 | `lib.rs` 280 行同时承担六种职责：模块声明、插件注册、命令注册表、菜单回调、窗口回调、生命周期；注册表 196 行里 132 行是 cfg 噪声（67%） | `src-tauri/src/lib.rs` L1–L280 |
| B5 | 扫描测试把 `src/lib.rs` 与「第一个 `generate_handler![` 块」写死，遇 `])` 即停。任何拆分（换文件、多块）都会让它误报 | `src-tauri/crates/assistant-vault/tests/command_registry.rs` |
| B6 | 参考项目的注册块内 cfg 恒为 0，可选性由模块边界（no-op stub / 插件）承担，而不是条目级 cfg | 见 3.8 对照表 |
| B7 | setup 钩子同样是条目级 cfg：`#[cfg(feature = "vault")] vault_setup(handle)` 写在 `setup.rs`，使 v2.0「全后端 cfg ≤ 6 行」的算术上限不成立（即使注册表归零，setup 仍是第 7 处） | `src-tauri/src/setup.rs` L90 |

B4 + B6 的含义：**当前做法把「可选性」编码在每一条命令条目上，边际成本是「每加一个可选特性 +132 行、每加一条命令 2 行」**。这不是「列表长」的问题——OpenBitFun 的注册块有 684 行却保持整洁——而是**可选性表达位置错误**的问题。

### 1.7 本阶段不覆盖的扩展面（口径限定）

「扩展面完整」是本文最容易说过头的一句，v2.1 把口径钉死：**本阶段只覆盖 UI + 配置 + IPC 三面**，其余面显式登记为不做。

| # | 未覆盖面 | 证据 | 后果 | 归属 |
|---|---|---|---|---|
| N1 | 文案面（i18n） | `src/i18n/index.ts` 把 `zh-CN.json` / `en-US.json` 作为**单一 resource** 静态注入；`src/i18n/i18next.d.ts` 的 `CustomTypeOptions.resources` 又以 `typeof zhCN` 闭合键空间 | 新特性的 label / section 文案必须改 2 个核心 JSON，且 `t()` 的类型键集不含新键（`translateKey` L56 用 `i18n.t(key, options as never) as unknown as string` 绕过类型——这是项目规则允许的 2 处 `as unknown as` 之一，只留下运行时 `missingKeyHandler` 警告） | 后续独立阶段；本阶段 6.3 探针使用字面量 label，因此验收不受影响 |
| N2 | 后端 setup 钩子 | `setup.rs` L90（见 B7） | 若不下沉，`setup.rs` 会长期占用一个白名单位，G1 无法从 8 收紧到 6 | E7（下沉到插件 `.setup()`） |
| N3 | 跨窗口事件通道 | `settingEvents.ts`（见 C8） | 新特性同步需改 core 分支 | E2（改槽位驱动） |
| N4 | 总览摘要与 `SettingsGroup` 的 conf hook 重复实现 | 见 C9 | 两处各改一遍 | E2（收敛为单一 `useConfGroup`） |
| N5 | core 保留了以特性命名的配置命令 | `cmd::get_vault_conf` 在 `ipc.rs`（core 注册块）内，无 cfg；vaultless 构建下它是「注册但返回默认值」的死命令 | 不违反 G1/G2，但与「core 不认识特性」的直觉冲突 | 不做（移入插件会让配置面同时存在裸名与命名空间两种 invoke 形态，收益小于一致性损失） |

N1 是最值得强调的一条：**它不影响本阶段验收，但会让「带文案的新特性零改动接入」不成立**，因此不能把本文的「完整」理解为「任何特性都能只加一个 `module.ts`」。

---

## 2. 根因归因

上面 33 条事实收敛成 5 个根因：

| 根因 | 含义 | 对应事实 |
|---|---|---|
| R1 | **扩展面不完整**：`FeatureModule` 只建模了「UI 挂载」，配置 / 加载 / IPC / 校验四个面仍闭合在 core；文案面另见 1.7/N1 | C1–C9、S5 |
| R2 | **依赖方向倒置**：core 反向依赖 features，靠类型擦除维持分层 | D1–D3 |
| R3 | **不变式靠断言而非结构**：放宽类型后未补运行时校验，错误表现为静默缺失 | I1–I7 |
| R4 | **单一事实源缺失**：开关、排序、key 各有一份隐式约定 | S1–S4 |
| R5 | **装配职责未分层**：`lib.rs` 单文件承担六种职责，可选性用条目级 cfg 表达，导致成本随「命令数 × 特性数」线性增长 | B2、B4–B7 |

本阶段处理这 5 个根因。**不处理**「产物是否为零残留」（已由动态 import + 守卫早退达成）、「编译收益」（见 `docs/store/compile-time-attribution.md`）、「cargo feature 下放」（见前置文档 L4/L5）。

---

## 3. 目标契约

### 3.1 FeatureModule v2

```ts
// src/features/types.ts
export interface FeatureModule {
  id: string;
  order?: number;                       // 仅作模块级排序基线，不再下传给子项
  routes?: readonly FeatureRoute[];
  sidebar?: readonly FeatureSidebarEntry[];
  categories?: readonly FeatureCategory[];
  settings?: FeatureSettings;
  conf?: readonly FeatureConfSlot[];    // 新增：配置面
  ipc?: FeatureIpc;                     // 新增：特性自有命令的命名空间
}

export interface FeatureIpc {
  namespace: string;                    // 后端插件名，例："vault"；特性无自有命令时省略整个 ipc 字段
}
```

五条规则：

1. **模块 `order` 不下传**。子项的 `order` 必须自声明，缺省按声明顺序；跨模块不共享数值空间。
2. **`conf` 与 `settings.fields[].group` 必须闭合**：字段声明的 group 必须能在 `conf` 里找到槽位，否则构建期抛错（见 3.5）。
3. **`sidebar.key` 必填、全局唯一，且侧栏与页面解耦**（v2.2 改动）：侧栏条目自带稳定标识 `key`，页面用 `navParent` 反向声明归属，多页可共享一个侧栏条目；不再要求 `key` 等于某条 route path（见 3.6）。
4. **`ipc.namespace` 只描述「特性自有命令」，不描述配置面命令**。配置面（`get_*_conf` / `patch_conf`）属于 core 职责，永远用裸命令名；若特性后端命令由插件提供（如 6.3 的 `probe_plugin`），前端必须通过 `ipc.namespace` 组合 invoke 名，而不是在各调用点手写 `plugin:xxx|`（见 3.2.1）。
5. **可见性显式声明**（v2.2 改动，v2.3 收敛为两值）：`visibility` 两值（`"sidebar"` / `"deepLinkOnly"`，缺省 `"sidebar"`），与「无 section 是否合法」的 `emptyPage` 正交。当前 `SettingsPageDefinition` 没有任何可见性字段（v2.4 复核：全仓无 `navOnly`），两者均为新增，必须按正交语义拆分（见 3.5 / 3.6）。v2.2 曾定义三值并含 `"nav"`（进命令面板但不进侧栏），但当前仓库没有任何页面需要它，按 3.10 标准 3 不进枚举。

对应的类型增补（`src/features/types.ts` 与 `SettingsPageDefinition` 同步）：

```ts
export interface FeatureSidebarEntry {
  key: string;                                     // 原 key?: string → 必填
  slot: SidebarSlot;
  label: string;
  icon: ComponentType;
  order?: number;
}

export interface FeatureRoute {
  path: string;
  load: () => Promise<{ default: ComponentType }>;
  boundary: string;
  order?: number;
  navParent?: string;                              // 归属的 sidebar.key；缺省时该 route 独立成条
  visibility?: "sidebar" | "deepLinkOnly";  // 缺省 "sidebar"
  keywords?: readonly string[];                    // 命令面板匹配词（settings page 已有同名字段）
}
```

`SettingsPageDefinition` 同步新增 `navParent?: string` / `visibility?` / `emptyPage?: boolean`（v2.4 复核：`navOnly` 在当前代码中不存在，三者均为新增字段）。侧栏选中态由「当前 route → 对应 page → `navParent`」推导，推导失败时回落到 page id，**不再依赖 `routes[0]` 这类隐式规则**。反向同理：点击侧栏条目时由 `key` 解析落地路径——优先命中 `navParent === key` 的 route，其次命中同条件的 page（落到 `/settings/<id>`），两者都没有时才把 `key` 本身当路径（兼容旧写法）。

### 3.2 配置面：类型层用声明合并，运行时用槽位

**类型层**改为开放接口，core 不再枚举 group：

```ts
// src/settings/core/types/confRegistry.ts
import type { AppConf, OrbConf, MascotConf } from "./confTypes";

export interface ConfTypeRegistry {
  app: AppConf;
  orb: OrbConf;
  mascot: MascotConf;
}

export type ConfGroup = keyof ConfTypeRegistry & string;
export type ConfOf<G extends ConfGroup> = ConfTypeRegistry[G];
```

扩展在自己的目录里补一条：

```ts
// src/features/vault/conf.ts
import type { VaultConf } from "./types";

declare module "@/settings/core/types/confRegistry" {
  interface ConfTypeRegistry {
    vault: VaultConf;
  }
}
```

这样 `ConfOf<G>` 保持完全类型安全（不是 `Record<string, unknown>`），而新增 group **不需要改 core**。代价是 `tsc` 会编译全树，`ConfGroup` 在 typecheck 期始终是完整集合——这正是我们要的：**类型层与构建开关解耦，运行时层由清单决定**。

**运行时层**用槽位描述符：

```ts
// src/settings/core/types/confSlot.ts
// G 默认取 ConfGroup（槽位注册表按 group 索引，值类型退化为联合是合理的）；
// 具体槽位必须写成 ConfSlot<"vault">，group 与值类型因此编译期绑定，不再有 `T = unknown` 的擦除。
export interface ConfSlot<G extends ConfGroup = ConfGroup> {
  group: G;
  invoke: string;                                     // 例："get_vault_conf"：core 命令，不带命名空间
  pageId?: SettingsPageId;                            // 该 conf 的编辑页（显式声明，见 3.5）
}
```

> **v2.5 偏离说明（实现收敛，主目的优先）**
> 1. **删除 `storeKey`**：`confs` 以 `group` 为键，一个 group 对应且仅对应一份配置，`storeKey`（如 `"vaultConf"`）只是「group → store 字段名」的冗余映射层，每新增一个 group 就要多维护一条字符串，且它与 `group` 一旦不同名就会产生第二种真源。改为 `confs[group]` 直接寻址，`CONF_KEY_MAP` / `CONF_LOAD_KEY_MAP` 随之整体消失（E2.12/E2.13）。
> 2. **删除 `toRecord` 成员**：它唯一的作用是把配置转成 `Record<string, unknown>` 交给字段渲染器，而这件事已经由 `confUtils.toConfRecord()` 在访问边界统一完成。放在槽位上意味着**每个特性都要重复实现一遍**，等于把唯一的信任边界拆成 N 份。改为槽位只描述「怎么取」（`invoke`）与「在哪编辑」（`pageId`），转换统一走 `toConfRecord()`。
> 3. 类型名统一为 `ConfSlot`（不再有 `FeatureConfSlot` 别名），内置槽位与扩展槽位共用同一类型、同一注册通道（见 3.2 的 `builtinRegistry` 说明）。

```ts
// src/features/vault/conf.ts（续）
export const VAULT_CONF_SLOT: ConfSlot<"vault"> = {
  group: "vault",
  invoke: "get_vault_conf",
  pageId: "vault",
};
```

`toConfRecord` 现状是**闭合联合** `AppConf | OrbConf | MascotConf | VaultConf | null`（`confUtils.ts` L3），泛化为 `<T extends object>` 后仍是 `confUtils.ts` 这一处允许的 `as unknown as`——按项目类型安全规范，全局共允许 2 处，另一处是 `i18n/index.ts` 的 `translateKey`（见 1.7/N1），本文不新增第 3 处。

**新增 conf group 的同步义务**（项目规则）：新增一个 group 必须在同一提交内完成三件事——① TS 接口（`AppConf` 类或特性自有类型）；② Rust 对应 struct 加同名字段 + `#[serde(default)]`；③ `config/features.json` 的清单项。三者缺一，类型层与运行时会分叉，且只在前端读配置时报错。3.5 校验只覆盖 ①↔③ 与 ②↔③ 的集合关系，①↔② 的字段级一致性依赖 3.8.4 的 `conf_side_effects()` 自证（group 存在即 struct 存在）。

#### 3.2.1 IPC 命名空间：配置面归 core，特性自有命令才需要命名空间

这一条是 v2.1 新增的，用来消掉「前后端扩展契约不对称」这个隐性坑。先钉住事实：

- 后端 `cmd/config.rs` 的 `get_vault_conf`（L151）与 `patch_conf`（L159）**没有 cfg**，属于 core 注册块。vaultless 构建下它们仍然存在，只是返回默认值（见 N5）。
- vault 自己的 66 条命令全部在 `cmd/vault/**` 内、全部带 cfg，插件化后变成 `plugin:vault|<函数名>`——**命令名就是函数名 `vault_xxx`，不做前缀裁剪**（v2.2 修正：`#[tauri::command]` 注册名与函数名同名，剥掉 `vault_` 会让 `plugin:vault|init` 找不到后端命令，只在运行期暴露）。

因此**配置面不需要命名空间**，`FeatureConfSlot.invoke` 保持裸命令名；需要命名空间的是「特性自有命令」。vault 用 `ipc.namespace = "vault"` 描述，而 `vault.service.ts` 第 1 行的 `invoke` 别名正是这条机制的**第一个落点**（现状仍是裸名调用，E7 时改为 `featureInvoke(vaultModule)`，见 3.8.3）：

```ts
// src/services/featureIpc.ts
export function featureInvoke(module: Pick<FeatureModule, "ipc">) {
  return <T = void>(command: string, args?: Record<string, unknown>) =>
    tauriInvoke<T>(
      module.ipc ? `plugin:${module.ipc.namespace}|${command}` : command,
      args,
    );
}
```

约束：**除 `featureIpc.ts` 与各特性自己的 service 外，任何地方不得出现 `plugin:` 字面量**（加一条 lint 规则）。这条约束让 6.3 探针的 mock 命令不需要手拼前缀，也让「特性从 core 命令迁移到插件命令」只改 `module.ts` 一处。

反向不变量（写进 3.5 校验）：`ipc.namespace` 若声明，后端必须存在同名插件；`config/features.json` 的 `cargoFeature` 与 `namespace` 必须同名。二者不一致时，前端能拼出 invoke 名但后端没有该插件，只会在运行时报 command not found——这是必须用构建期断言堵住的失败模式。

**store 改为单字段映射**，槽位数量由 `ConfGroup` 决定，不再是四个具名属性：

```ts
// src/stores/settingsStore.ts
// 逐键可选的映射类型：每个键保留 ConfOf<G> 的按键收窄
export type ConfStore = { [G in ConfGroup]?: ConfOf<G> | null };

export interface SettingsState {
  // 无该特性的构建里键不存在（undefined），与「已注册但未加载」的 null 区分
  confs: ConfStore;
  loadConf: <G extends ConfGroup>(group: G) => Promise<void>;
  updateKeybinding: (id: string, binding: string) => Promise<void>;
  resetKeybindings: () => Promise<void>;
}
```

**为什么不是 `Partial<Record<ConfGroup, ConfOf<ConfGroup> | null>>`**：`Record` 的值类型不随键变化，索引签名会让每个键都退化成 `AppConf | OrbConf | MascotConf | VaultConf` 的联合类型——于是 `getConfValue("vault")` 声明的 `ConfOf<"vault">` 返回类型与实现不匹配，只能靠断言糊过去。这正是本文要消灭的「类型说谎」，而映射类型（`{ [G in ConfGroup]?: ConfOf<G> | null }`）在零运行时成本下解决它：`confs[group]`（`group: G`）自然收窄为 `ConfOf<G> | null | undefined`。

**类型承诺与运行时事实对齐**（v2.1 新增，修 R1 的副作用）：

`ConfGroup` 在 typecheck 期恒为全集（声明合并的结果），但运行时键集合由构建清单决定。若把 `confs` 声明为全量必填，类型会说「一定有」，运行时是 `undefined`——这是本文最典型的「类型说谎」位置。三条约束补上：

1. `confs` 用可选映射类型 `ConfStore`（逐键可选），`undefined`（该构建不含此特性）与 `null`（已注册、尚未加载）语义分离。
2. 新增 `isConfGroupAvailable(group): boolean`（= `getConfSlot(group) !== undefined`），UI 据此决定是否渲染配置区。
3. **统一访问出口**：读 `confs` 只走 `getConfValue(group)` / `useConfGroup(group)`。用 ESLint `no-restricted-syntax` 拦截 `confs` 上的**直接属性访问**（`confs.vaultConf`），仅 `useConfGroup.ts` / `confRuntime.ts` 放行、且只允许变量下标 `confs[group]`。**注意这条 lint 的目的不是类型安全**——`ConfStore` 已经保证下标访问的类型正确；它的目的是把「Phase 变化时统一处理失效键」这件事收在一处（3.3 的相位契约），属于可维护性约束，因此只对新增代码生效，不要求一次性迁移存量。

两个访问器的定义（`getConfValue` 是命令式取值的唯一出口，`useConfGroup` 是渲染层唯一出口）：

```ts
// src/settings/core/useConfGroup.ts
export function getConfValue<G extends ConfGroup>(group: G): ConfOf<G> | null | undefined {
  return useSettingsStore.getState().confs[group];
}
export function useConfGroup<G extends ConfGroup>(group: G): ConfOf<G> | null {
  return useSettingsStore((s) => s.confs[group]) ?? null;
}
```

`loadConf` 由 core 的槽位注册表驱动，扩展在装载时注册自己的槽位：

```ts
// src/settings/core/confRuntime.ts
const slots = new Map<string, FeatureConfSlot>();

export function registerConfSlot<G extends ConfGroup>(slot: FeatureConfSlot<G>): void {
  slots.set(slot.group, slot);      // 方法签名双变，无需断言
}
export function getConfSlot(group: string): FeatureConfSlot | undefined { return slots.get(group); }
export function isConfGroupAvailable(group: string): boolean { return slots.has(group); }
export function getConfGroups(): readonly string[] { return [...slots.keys()]; }

// 线上格式的唯一信任边界：后端 struct 带 #[serde(default)]，字段恒存在，
// 因此这里用 unknown → T 的断言（不是 `as unknown as`，不占用类型安全规范的 2 个名额），
// 且全仓库只有这一处把「后端返回」当类型化 conf 用。
export async function fetchConf<G extends ConfGroup>(group: G): Promise<ConfOf<G> | null> {
  const slot = getConfSlot(group);
  if (!slot) return null;
  const raw = await tauriInvoke(slot.invoke);   // core 配置命令，裸名（见 3.2.1）
  return (raw as ConfOf<G>) ?? null;
}
```

core 的三个槽位（app / orb / mascot）在 `builtinRegistry.ts` 以数据形式声明（`defineConfSlot`），由 `initSettingSystem()` 在启动时注册进运行时表。**声明是唯一真源，快照与运行时都从它派生**——这是 core 唯一保留的硬编码点，且它属于内置通道，本就不该被排除。v2.5 修正：内置槽位不再散落在 `initSettingSystem()` 的注册代码里，否则快照（`normalize()`）在不跑启动流程的测试环境下拿不到内置槽位，会与运行时表分叉；这也是「vault 槽位只从扩展通道收集、内置槽位只从注册代码产生」这一双通道问题的根因。

配置面改造后，各闭合点各自变成一次查表（C1–C9 全覆盖，漏掉 C8 会让跨窗口同步继续按 group 硬编码）：

| 闭合点 | 改造后 |
|---|---|
| C1 `CONF_KEY_MAP` / `CONF_LOAD_KEY_MAP` | 删除，改由 `confs[group]`（以 group 为键）与 `loadConf(group)` |
| C2 `ConfOf<G>` 条件类型 | 改为 `ConfTypeRegistry[G]` 索引 |
| C3 四个具名槽 | 改为 `confs: ConfStore` 映射（逐键可选 + 可用性判定，见上） |
| C4 `settingService.getVaultConf` | 删除，改由 `fetchConf("vault")` |
| C5 `SettingsOverview` 的 `confMap` | 改为 `getConfGroups()` 动态构造 |
| C6 `GROUP_TO_PAGE` | 删除，改为 `snapshot.confPageMap`（由 `confSlot.pageId` 显式声明，见 3.5） |
| C7 `optimisticUpdate` | 改为 `confs[group]`（以 group 为键，无需 storeKey 映射） |
| C8 `settingEvents` 的 group 分支 | 改为 `if (isConfGroupAvailable(group)) store.loadConf(group)`；`VAULT_ENABLED` 引用随之消失 |
| C9 两份 `useConfByGroup` | 合并为 `settings/core/useConfGroup.ts`，内部走 `getConfValue` |

### 3.3 快照与索引：core 只读，不建

core 定义**纯类型**的快照，不引用 features：

```ts
// src/settings/core/types/snapshot.ts
export interface SettingsSnapshot {
  pages: readonly SettingsPageManifest[];
  pageMap: ReadonlyMap<string, SettingsPageManifest>;
  categories: readonly SettingsCategory[];
  categoryMap: ReadonlyMap<string, SettingsCategory>;
  sections: readonly SettingSectionDef[];
  sectionMap: ReadonlyMap<string, SettingSectionDef>;
  sectionsByPage: ReadonlyMap<string, readonly SettingSectionDef[]>;
  fields: readonly SettingDefinition[];
  fieldsByGroup: ReadonlyMap<string, readonly SettingDefinition[]>;
  fieldsBySection: ReadonlyMap<string, readonly SettingDefinition[]>;
  confPageMap: ReadonlyMap<string, string>;   // conf group -> settings pageId
}
```

快照**惰性构建 + 双键失效**，同时解决「每次重建」与「mascot 运行时注册查不到」：

```ts
// src/settings/core/snapshot.ts
let extensionInput: SettingsExtensionInput = EMPTY_INPUT;
let builtinVersion = 0;
let cache: { key: string; value: SettingsSnapshot } | null = null;

export function bumpBuiltinVersion(): void { builtinVersion += 1; }   // defineSetting / defineSection 调用
export function bindExtensionInput(input: SettingsExtensionInput): void { extensionInput = input; }

export function getSettingsSnapshot(): SettingsSnapshot {
  const key = `${builtinVersion}:${extensionInput.id}`;
  if (cache && cache.key === key) return cache.value;
  const value = buildSnapshot(builtinVersion, extensionInput);   // 内含 validate
  cache = { key, value };
  return value;
}
```

要点：

- **失效键不是长度**（修 I7）。内置注册用单调递增版本号，扩展用装载标识（`"empty"` 或 `"vault"`），任何一次真实变化都会使键改变。
- **内置可变容器保留**：`defineSetting` / `defineSection` 仍 push，只在末尾 `bumpBuiltinVersion()`。mascot 的运行时注册因此每次都能被下一次查询看到（沿用 `M8` 的结论，不引入「不许动态注册」的约束）。
- **查询函数只读快照**：`getDefinitionsByGroup` 等六个函数全部退化为 `getSettingsSnapshot().fieldsByGroup.get(group) ?? []`，返回冻结数组，**引用稳定**。

**相位契约**（v2.1 新增）。「全局可变容器 + 惰性构建 + 单调版本号」这套机制本身不定义生命周期，于是「首次 `getSettingsSnapshot()` 之前必须完成 `defineSetting` 与 `bindExtensionInput()`」就是一条隐式时序不变式——这与 `derive()` 当年靠 `features` 引用比较踩的坑是同一类问题，只是换了个形式重现。显式化为三段：

| 相位 | 允许的动作 | 约束 / 强制手段 |
|---|---|---|
| 静态注册期 | 内置通道 `defineSetting` / `defineSection` / `registerRenderer` 可任意时刻调用（含 mascot 渲染器懒加载） | 每次末尾 `bumpBuiltinVersion()`，下一次查询自然重建；**不引入「禁止动态注册」** |
| 绑定期 | `bindExtensionInput()` | 只在 `loadFeatures()` 内调用；一旦已发生查询，再绑定必须换新的 `id`（不允许原地 mutate `aggregated`，否则缓存键不变、读到旧快照） |
| 查询期 | `getSettingsSnapshot()` 及其派生查询 | 只读；快照与派生数组全部 `Object.freeze` |

`main.tsx` 的启动顺序因此成为契约的一部分，顺序错误必须**在开发期立刻报错**而不是表现为「设置页少几项」：

```ts
// src/main.tsx
initSettingSystem();
await loadFeatures();     // 内部 bindExtensionInput()
render(<App />);          // 首次查询发生在 render 之后
```

并在 `getSettingsSnapshot()` 内加 `assertBound()`：DEV 下 `extensionInput` 仍为初始值时抛错（「先查询后绑定」= 编程错误）。生产不抛，走 3.5 的降级策略。

### 3.4 依赖方向拉直

```
settings/core/types/*      （纯类型，零依赖）
settings/core/snapshot.ts  （只 import types + 内置容器）
settings/core/query.ts     （只 import snapshot）
        ↑
features/aggregate.ts      （import types + lazyWithRecovery + definePage）
features/registry.ts       （构建 aggregate → bindExtensionInput）
        ↑
main.tsx                   （await loadFeatures()）
```

core **不再 import `@/features`**；`features` 单向依赖 core。绑定动作发生在 `loadFeatures()` 内：

```ts
// src/features/registry.ts
export async function loadFeatures(): Promise<void> {
  const modules: FeatureModule[] = [];
  for (const entry of ENABLED_FEATURES) {           // 生成物，见 3.6
    modules.push(await entry.load());
  }
  const aggregated = aggregate(modules);
  bindExtensionInput(aggregated.settingsInput);      // 交给 core，方向：features → core
  current = aggregated;
}
```

这条改造顺带消掉 `fieldRegistry` / `pageRegistry` 里两处 `getFeatures()` 调用，`settings/core` 的 import 清单里再也不会出现 `@/features`。

### 3.5 校验：结构不变式 + 两档处理

`buildSnapshot` 内部**无条件**执行 `validate()`（校验本身总是执行；只有「违规后怎么处理」分档——DEV / 测试抛错，生产丢弃该条目并 `log.error`，见下文策略 3。这些都是代码缺陷而非数据问题，n≈50，O(n) 一次，成本可忽略）：

| 不变式 | 违反时（DEV / 测试） | 违反时（生产） |
|---|---|---|
| field key 全局唯一、section id 全局唯一、page id 全局唯一、route path 唯一、sidebar key 唯一 | 抛错 | 丢弃后出现的那一条 |
| `page.categoryId` 必须存在于 categories | 抛错 | 丢弃该 page |
| `section.pageId` 必须存在于 pages | 抛错 | 丢弃该 section |
| `field.section`（若声明）必须存在于 sections | 抛错 | 丢弃该 field |
| `field.group` 必须在已注册的 conf 槽位里 | 抛错 | 丢弃该 field |
| 每个 page 的 sections 非空，或显式声明 `emptyPage: true` | 抛错 | 保留页面但标空（记 error） |
| `page.navParent`（若声明）必须命中某个 `sidebar.key`；且 `visibility !== "deepLinkOnly"` 的 page 必须能推导出归属（`navParent` 或自身 route） | 抛错 | 丢弃该 page |
| 每个 `sidebar.key` 至少被一个 page / route 引用 | 抛错 | 丢弃该 sidebar 条目 |
| `visibility: "deepLinkOnly"` 的 page 不得声明 `navParent` | 抛错 | 忽略 `navParent` |
| `confSlot.pageId`（若声明）必须存在于 pages，且该 page 内至少有一个该 group 的 section / field | 抛错 | 从 `confPageMap` 剔除 |
| `confSlot` 有字段但未声明 `pageId` | 抛错 | 不建映射（命令面板跳过该 group） |
| `ipc.namespace`（若声明）必须与 `config/features.json` 的 `cargoFeature` 同名，且后端存在同名插件 | 抛错 / 构建期断言 | 构建期断言已拦，生产不会到达 |

三条策略变更：

1. **去重统一为「插入即查重，命中即抛」**。`defineSetting` 不再静默丢弃（修 I5），两条通道共用同一个 `insertUnique()`（落 `core/insertUnique.ts`：DEV 抛错，生产 `log.error` + 跳过该条目）。
2. **页面缺失改「未找到」面板，且要改两条回落路径**。`SettingsContent.tsx` 现有两处回落：L12 的 `isSettingsPageId(pageId)` 守卫失败即在 L13 直接 Navigate 到 `/settings/${DEFAULT_SETTINGS_PAGE_ID}`，L18 再调 `getSettingsPageManifest`，而它内部（`pageRegistry.ts` L210）又 `?? map.get(DEFAULT_SETTINGS_PAGE_ID)!`。**只改后者无效**（守卫先跳转，后者根本走不到）。改法：守卫内渲染未找到面板、删除 `getSettingsPageManifest` 的 `??` 回落并让返回类型变 `| null`，使回落点唯一（修 I3）。面板用 antd `Empty` 空态（与 `VaultList` / `VaultApp` 一致），**不渲染 HTTP 404 数字插画**——桌面端是前端内存路由，无 HTTP 语义（v2.5 改动）。
3. **校验分两档：构建 / 测试期抛错，生产降级**（v2.1 改动）。`buildSnapshot` 在非生产环境 throw；生产环境 `log.error` + **丢弃该条目**（页面级 / 字段级隔离），不让整页白屏。理由：错误声明是静态代码缺陷，6.2 的 5 类不变式测试与 6.1 的三份黄金快照已能在 CI 覆盖，生产再抛错换来的不是更早发现，而是可用性下降（原 R3「刻意白屏」的论证不成立）。

**校验批量收集，不 fail-fast**（v2.2 改动）。`validate()` 的返回类型改为 `Violation[]`（`{ rule, target, message }`），DEV / 测试期一次性抛出全部违规（`AggregateError` 或逐行拼接的消息），生产期逐条 `log.error` 并丢弃对应条目。理由：扩展作者一次写错 5 处时，fail-fast 需要 5 轮才能全绿；批量收集的成本与 fail-fast 相同（同为 O(n) 单次遍历），而参考项目（OpenBitFun 的 `ConfigValidationResult` 收集 errors + warnings，warning 还带 `field` / `code`）已验证这一策略在扩展场景下更好用。`Violation.rule` 直接复用上表的「不变式」列，便于测试按规则名断言。

`confPageMap` 改为**显式声明**（v2.1 改动）。原方案是「取第一个含该 group 字段的 section 的 pageId」——这是用推断代替声明，与 I2 / I4 同一类问题（顺序敏感 + 纯数据型 conf 静默缺席），也违背本文「不变式靠结构而非约定」的主张。改为槽位自带 `pageId`（见 3.2），快照只做校验与建表：

```ts
// buildSnapshot 内
const confPageMap = new Map<string, string>();
for (const slot of slots) {
  if (!slot.pageId) continue;                  // 无字段的纯数据 conf 不建映射
  confPageMap.set(slot.group, slot.pageId);
}
```

| 方向 | 断言 |
|---|---|
| 声明了 `pageId` → 页面必须存在 | 抛错 |
| 声明了 `pageId` → 该页必须至少有一个该 group 的 section 或 field | 抛错（防「声明了配置页但 UI 没接」） |
| 该 group 有字段 → 必须声明 `pageId` | 抛错（防「字段存在但命令面板跳不过去」） |

代价是扩展多写一个 `pageId`；收益是映射从「顺序敏感」变成「顺序无关」——调整 `order` 或 section 声明顺序不再改变命令面板的跳转目标。

### 3.6 排序、插槽与 key 的显式契约

| 面 | 现状 | 目标 |
|---|---|---|
| 排序 | 单一 `order` + 模块 `base + i` 下传 | 每个集合独立命名空间（`page.order` / `route.order` / `entry.order`），模块 `order` 不下传；page 支持 `after: SettingsPageId` 锚点，构建期解析并检测环 |
| 插槽 | 2 值 | 4 值：`"after-tools"` / `"before-history"` / `"after-history"` / `"before-settings"`；每个插槽内部按 `order` → `key` 字典序稳定排序 |
| 侧栏 key 与归属 | 缺省取 `routes[0].path`，侧栏与页面被隐式锁成 1:1 | `sidebar.key` **必填且全局唯一**（自证标识，与 route path 解耦）；页面用 `navParent` 反向声明归属，多页可共享一个侧栏条目；`selectedKeys` 由「当前 route → page → `navParent`」推导，无隐式规则 |
| 可见性 | 当前无可见性字段（`SettingsPageDefinition` 只有 `id` / `categoryId` / `label` / `keywords` / `icon` / `order` / `load`；v2.4 复核无 `navOnly`） | 新增两个正交字段：`emptyPage`（无 section 是否合法）+ `visibility: "sidebar" \| "deepLinkOnly"`（缺省 `sidebar`，v2.3 从三值收敛为两值）；`deepLinkOnly` 页面仍参与路由与未找到判定，只是不进侧栏 / 命令面板 |
| 命令面板匹配词 | 由 label / id / path / category 拼接推导 | `page.keywords` / `route.keywords` 声明式参与（`SettingsPageManifest.keywords` 已存在，`FeatureRoute` 补齐同名字段）；不再从 title 反推 |
| 页面分类 | 分类存在性靠 filter 结果 | 分类必须被声明；声明了但无页面 → 构建期抛错（比静默剪枝更早暴露） |

**为什么放弃「`sidebar.key` 必须等于某条 route path」**（v2.2 改动）：这条约束把「侧栏条目 ↔ 页面」锁成 1:1，一旦某个特性需要「一个侧栏入口 + 多个子页面（tab / 深链）」，就只能新增侧栏条目或伪造 route，两者都会污染导航。参考项目（openhuman）的做法是侧栏条目持有独立 `id`、页面用 `navParent` 指回条目，导航顺序与路由路径因此完全解耦——这与本文「单一事实源」的目标一致，且校验规则更少（只需两条：`navParent` 命中、`key` 被引用）。

### 3.7 构建开关单源

新建仓库级清单 `config/features.json`（**手写**，不生成任何中间产物）：

```json
{
  "vault": {
    "default": true,
    "cargoFeature": "vault",
    "env": "VAULT_ENABLED"
  }
}
```

> **v2.5 决策（D1 已定）**：清单手写，**取消代码生成**。曾计划 `gen-features.mjs` 输出 `src/features/generated/manifest.ts`，但那会新增「生成器 ↔ 生成物」漂移这一失败模式（原 R5），而本阶段只有 1 个特性、2 个消费点，生成的收益为负。代价是新增特性时要在 `features/registry.ts` 补一段编译期守卫 + 动态 import（每特性约 4 行），这比维护一个生成器便宜。
> 同时保留 `src-tauri/Cargo.toml` 的 `[features]` 段——cargo 强制要求特性在此声明，无法省略；两处因此是「一份清单 + 一份 cargo 声明」，用启动期断言消除漂移，而不是靠人工记忆。

三个消费/校验环节：

1. **前端开关**（`vite.config.ts`）：直接读清单，按每项的 `env` 生成 `__<ENV>__` 常量（`vault` → `__VAULT_ENABLED__`），不再手写。`process.env[env]` 有值时以环境变量为准（`-Without vault` 走这条），无值时取清单的 `default`——「清单描述常态，命令行描述本次构建」。
2. **构建脚本**（`scripts/feature-build.ps1`）：读**同一份**清单，按 `env` 注入 `1` / `0`，按 `cargoFeature` 拼 `--features`。`tauri.conf.json` 的 `beforeBuildCommand`（`npm run build`）继承该环境变量，前端与后端在**同一次进程调用**里拿到同一份开关，混合态不可能出现。
3. **一致性断言**（构建脚本启动即跑）：`config/features.json` ↔ `Cargo.toml [features]` 双向——`cargoFeature` 必须已声明、两侧无孤儿条目、默认启用集合必须相等；不一致直接中止。已实测：把 `default` 改成 `false` 后脚本在跑 npm 之前以退出码 1 拦下。`cargoFeature` ↔ `FeatureModule.ipc.namespace` 的断言推迟至 E7（`FeatureModule` 当前无 `ipc` 成员）。

关闭特性的 chunk 归零仍由「编译期常量守卫 + 动态 import」保证（`features/registry.ts` 的 `if (!__VAULT_ENABLED__) return;` 折叠后整段不可达），与清单是否手写无关。实测：`VAULT_ENABLED=0` 时产物 vault chunk = 0、`vaultAutoLockMinutes` 命中 = 0；默认（清单 `default: true`）时 vault chunk = 5。

### 3.8 后端收敛：从「条目级 cfg」到「模块边界」

#### 3.8.1 参考项目对照（实测）

| 项目 | lib/main 行数 | 职责组织 | 命令数 | 注册块行数 | 块内 `#[cfg]` |
|---|---|---|---|---|---|
| ChatGPT | 28 | 命令集中 `core/cmd.rs`，注册表只写 `cmd::xxx` | 11 | 13 | 0 |
| WindowPet | 76 | `mod app; use app::{cmd, conf, tray, utils};` | 5 | 7 | 0 |
| OpenBitFun data-migrator | 62 | `mod commands` + 内联 `generate_handler!` | 11 | 13 | 0 |
| OpenBitFun desktop | 数千 | 扁平列表 + 解析 `generate_handler!` 的双向断言脚本 | 400+ | 684 | **0**（可选能力用 no-op stub） |
| openhuman | 数千 | 重命令面走进程内 RPC，IPC 只留宿主级；可选能力做成 Tauri 插件 | ~70 | 78 | 少量（gateways 一组） |
| **本仓库** | **280** | 六种职责内联 | **123**（core 57 + vault 66） | 196 | **66 行 cfg + 66 行条目 = 132 行** |

三条可直接借用的结论：

1. **列表长度不是问题，列表内的 cfg 才是**。OpenBitFun 的注册块比我们长 3.5 倍仍放在 lib.rs，因为它禁止列表内出现 cfg——可选性由模块边界（no-op stub / 插件）承担，不由条目级 cfg 承担。
2. **`generate_handler!` 的入参必须是字面路径列表**，所以「把列表拆到多个文件」在任何参考项目里都不存在；能拆的是**列表之外的职责**，以及**列表之间的组合方式**。
3. **OpenBitFun 的断言脚本用 `^[A-Za-z0-9_:]+$` 匹配条目**，该正则本身就不允许 cfg 条目存在——它把「注册表内不得出现 cfg」变成 CI 约束，而不是靠 review。

因此 3.8 的目标是**确定的、可计数的**，不是「缓解」：

| # | 判定标准 | 现状 | 目标 |
|---|---|---|---|
| G1 | `src-tauri/src/**` 中 `#[cfg(feature = "vault")]` / `#[cfg(not(...))]` 行数 | **73**（`lib.rs` 68 + `setup.rs` 1 + `cmd/mod.rs` 2 + `cmd/config.rs` 2） | **≤ 8**，且只允许位于 `vault_plugin.rs` / `cmd/mod.rs` / `setup.rs` 三个白名单文件；setup 钩子下沉到插件后 `setup.rs` 归零，上限收紧到 **≤ 6** |
| G2 | 注册表文件内 `#[cfg]` 出现次数 | **66** | **0** |
| G3 | `lib.rs` 行数 | 280 | **≤ 40** |
| G4 | 新增一个可选特性的后端边际成本 | +132 行 | **+1 个插件文件 + 1 对 cfg + 1 个 `permissions/<plugin>/default.toml` + capability 1 行**（3.8.3a spike 已实测确认） |
| G5 | 新增一条命令的注册成本 | 2 行（cfg + 条目） | **1 行条目**；ACL 摊到特性级（`default` 权限集自动覆盖全部命令），不随命令数增长 |
| G6 | 装配层（`lib.rs` / `plugins.rs` / `lifecycle.rs`）内 `#[cfg]` | 2 处（`Exit` 双分支） | **0** |

G1 的上限为什么是 8 而不是 6：v2.0 的 ≤6 在算术上不可达。即便 `setup.rs` 归零，`cmd/mod.rs` 仍需 `mod vault`(1) + 显式重导出(1) + `conf_side_effects` 注册(1) + `vault_cleanup_on_exit` 桩(2)，加上 `vault_plugin.rs` 的 1 对(2) 共 7 行。若把 3.8.4 做到「彻底不重导出、`vault_plugin` 直接引用 `crate::cmd::vault::*`」，可降到 6——因此 ≤6 保留为**进阶目标**，≤8 是本阶段验收线。

**`setup.rs` 是 ≤8 能否成立的硬前提**（v2.1 补充）：现状 `setup.rs` L90 那 1 行是 G1 现状 73 的组成部分，但它在 3.8.3 之前**无法**被白名单化——它既不是插件文件也不是 `cmd/mod.rs`。因此 3.8.3 必须把 `vault_setup`（`cmd/vault/mod.rs` 的 `pub fn vault_setup`，负责 `manage` 五个 State + `spawn_screen_lock_watcher`）整体下沉到插件 `.setup()`，把 `setup.rs` 的 cfg 归零，G1 才从「三白名单 ≤8」收紧到「两白名单 ≤6」。只做注册表拆分而不下沉 setup，上限永远是 8。

路径固定为三步：职责拆分（3.8.2）→ 插件化归零（3.8.3）→ 重导出与副作用收敛（3.8.4）；3.8.5 把 G1–G6 写成 CI 断言，防止回退。

#### 3.8.2 职责拆分：lib.rs 只剩装配

目标 `src-tauri/src/` 结构：

| 文件 | 职责 | 从 `lib.rs` 迁出 |
|---|---|---|
| `lib.rs` | 模块声明 + `run()` 装配（≤ 40 行） | — |
| `plugins.rs` | 6 个 `.plugin(...)` | L25–L35 |
| `ipc.rs` | `handler()`，core 57 条注册表（vault 66 条在 3.8.3 迁出） | L37–L232 |
| `menu.rs` | `on_menu_event` 回调 | L233–L244 |
| `window.rs`（已存在） | `on_window_event` 回调 | L245–L264 |
| `lifecycle.rs` | `SHOULD_EXIT` / `request_exit` / `should_exit` / run 事件 | L16–L21、L265–L280 |
| `setup.rs`（已存在） | 初始化钩子；vault 的 `vault_setup` 调用在 3.8.3 下沉到插件 `.setup()`，使本文件 cfg 归零（G1 收紧到 ≤6 的前提） | L90 |

装配后 `lib.rs`：

```rust
mod cmd;
mod ipc;
mod lifecycle;
mod menu;
mod plugins;
pub mod protocol;
mod setup;
mod tray;
mod window;

pub use lifecycle::request_exit;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    plugins::register(tauri::Builder::default())
        .setup(setup::init)
        .invoke_handler(ipc::handler())
        .on_menu_event(menu::on_menu_event)
        .on_window_event(window::on_window_event)
        .build(tauri::generate_context!("tauri.conf.json"))
        .expect("error while building tauri application")
        .run(lifecycle::on_run_event);
}
```

（3.8.3 再追加 `mod vault_plugin;` 一行与 `plugins.rs` 里的一次 `.plugin(...)`，`run()` 本身不再变化。）

各文件的签名（与参考项目一致，均为「回调函数化」）：

```rust
// ipc.rs
pub fn handler<R: tauri::Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static

// plugins.rs
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry>

// menu.rs
pub fn on_menu_event(app: &tauri::AppHandle, event: tauri::menu::MenuEvent)

// window.rs
pub fn on_window_event(window: &tauri::Window, event: &tauri::WindowEvent)

// lifecycle.rs
pub fn should_exit() -> bool
pub fn on_run_event(app_handle: &tauri::AppHandle, event: tauri::RunEvent)
```

`handler<R: Runtime>() -> impl Fn(Invoke<R>) -> bool` 是合法形态：`generate_handler!` 生成的闭包对任意 `R` 都满足 `Fn(Invoke<R>) -> bool`，openhuman 的 `tauri-plugin-ptt` 就在 `init<R: Runtime>()` 内以同一写法返回 handler。

**装配层零 cfg**：`Exit` 事件原本要写「vault 清理 / 非 vault 占位」双分支，改为在 `cmd/mod.rs` 提供**双实现桩**（OpenBitFun 的 no-op stub 手法），`lifecycle.rs` 无条件调用：

```rust
// cmd/mod.rs
#[cfg(feature = "vault")]
pub use vault::vault_cleanup_on_exit;
#[cfg(not(feature = "vault"))]
pub fn vault_cleanup_on_exit(_app: &tauri::AppHandle) {}
```

约束：这一步**不拆命令列表、注册表内的 66 对 cfg 原样保留**，只做文件搬迁 + 把 `Exit` 的装配期双分支下沉为模块内双实现桩；`git diff` 应只体现「行移动 + 可见性调整」。`setup.rs` 与 `cmd/app.rs` 对 `crate::SHOULD_EXIT` / `crate::request_exit` 的引用改为 `crate::lifecycle::should_exit()` / `crate::lifecycle::request_exit()`，`lib.rs` 的 `pub use` 只作过渡，最终保留单一入口。

#### 3.8.3a 前置 spike：插件命令的 ACL 面（v2.2 新增）

v2.1 把插件化的成本写成「后端 +1 个插件文件 + 1 对 cfg」，漏掉了一个 Tauri v2 的硬约束：**插件命令受 capability 约束**，与 app 命令不同。参考项目的实测证据：

- `tauri-plugin-ptt` 的 `build.rs` 调 `tauri_plugin::Builder::new(COMMANDS).try_build()`，生成 `permissions/autogenerated/commands/*.toml`（5 条命令 → 5 个文件）；
- 宿主 `capabilities/ios.json` 必须逐条声明 `ptt:allow-start-listening` 等（**kebab-case**，与 snake_case 命令名不同形）；
- 本仓库现状相反：`src-tauri/build.rs` 只有 `tauri_build::build()`，`capabilities/default.json` 只授了官方插件，`gen/schemas/acl-manifests.json` 也不含任何自定义插件。

因此 3.8.3 在动手前必须先做一次 spike，验证三件事：

| # | 待验证 | 两条候选路线 |
|---|---|---|
| S1 | 插件命令 `plugin:vault\|vault_init` 能否在同一窗口内被调用 | (a) 独立 crate + `tauri_plugin::Builder`（ptt 路线）；(b) `build.rs` 的 `tauri_build::Attributes::plugin("vault", InlinedPlugin::new().commands(&[...]))` 内联插件（免新 crate） |
| S2 | capability 的最小声明形态 | (a) 插件内 `permissions/default.toml` 定义 `default` 权限集（capability 写 1 条 `vault:default`）；(b) 逐条 `vault:allow-*`（66 条） |
| S3 | 关闭特性时的行为 | 插件 `init()` 返回空 builder（命令不存在）→ 前端调用报错；确认该错误在构建期（capability 引用不存在的 identifier）还是运行期（command not found）暴露 |

spike 用 **1 条命令 + 1 个窗口**即可，半天量级。它决定 G4/G5 的最终口径：走 (a)+(a) 时新增特性 ≈ 1 文件 + 1 对 cfg + 1 个 `default.toml` + capability 1 行；走 (a)+(b) 时 capability 有 66 行，G5 的「1 行条目」不再成立。

**spike 实测结论（已完成）**：采用路线 **(b) 内联插件 + (a) 权限集**，ACL 边际成本降为 1 个 `permissions/<plugin>/default.toml` + capability 1 行。

| # | 结论 | 证据 |
|---|---|---|
| S1 | 成立：`build.rs` 的 `Attributes::plugin("spike", InlinedPlugin::new().commands(&["echo"]))` 免新 crate，前端 `plugin:spike\|echo` 同窗口调用成功 | 运行期返回 `S1-OK`，后端日志 `SPIKE_ECHO_INVOKED` |
| S2 | 最小声明形态 = `permissions/<plugin>/default.toml` 的 `[default]` 表 + capability 1 条 `<plugin>:default`，逐条 `allow-*` 不需要 | `acl-manifests.json` 中 spike 与 `autostart`/`dialog`/`log`/`opener` 形态一致（`default_permission=default`、`permission_sets=[]`） |
| S3 | 失败面分两段：ACL **声明**错误在构建期暴露；ACL **授权**缺失在运行期暴露 | 三条实测见下 |

S3 实测细节：

1. capability 引用不存在的权限 → 构建期失败：`failed to run tauri-build: Permission spike:does-not-exist not found expected one of ...`
2. `build.rs` 未声明插件但 capability 仍引用其权限 → 构建期失败：`Permission spike:default not found ...`
3. 插件与权限均存在、capability 未授 → 构建通过、运行期被拒：`log.log not allowed. Permissions associated with this command: log:allow-log, log:default`

两条由此固化的实现约束：

- **`[default]` 而非 `[[set]] identifier = "default"`**。后者只进 manifest 的 `permission_sets`，`default_permission` 仍为 `null`，capability 写 `<plugin>:default` 会被运行期拒绝，而报错信息仍把该标识列为可用权限，极难排查。
- **ACL 声明不受 feature 影响**。`build.rs` 的 `plugin(...)` 与 `permissions/<plugin>/default.toml` 恒存在，可选性只体现在「运行期命令是否存在」；否则关闭特性时 capability 的 `<plugin>:default` 会直接让构建期失败。

**在 spike 结论落地前，G4/G5 只作为方向性指标，不作为验收线。** 若 spike 显示 ACL 成本不可接受，备选路线是「桩化」——命令恒注册、主体按 cfg 切换，用 no-op stub 表达可选性（OpenBitFun 的做法：400+ 命令的注册块 0 处 cfg）。桩化的代价是禁用构建仍暴露 66 条惰性命令，收益是**完全不碰 IPC 契约与 ACL**；它不满足「编译期不存在」的强约束，因此只作为 Plan B，不替代 3.8.3。

spike 结论落地后，ACL 成本已降到 1 行，**Plan B 不触发**，3.8.3 按插件边界主路径执行。

#### 3.8.3 cfg 归零：vault 走插件边界（需先完成 3.8.3a）

`assistant-vault` 已经是 optional dep，feature 关闭时整个 crate 不参与编译。既然如此，**把 66 条命令的注册整体搬进插件边界**，而不是把「可选性」重复编码 66 遍。

新增 `src-tauri/src/vault_plugin.rs`，全文只有 1 对 cfg：

```rust
use tauri::{plugin::TauriPlugin, Runtime};

#[cfg(feature = "vault")]
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::<R>::new("vault")
        .setup(|app, _api| {                      // setup 钩子随插件走，setup.rs 因此 cfg 归零
            crate::cmd::vault_setup(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            crate::cmd::vault_init,
            crate::cmd::vault_unlock,
            // ... 66 条，逐条**无 cfg**
            crate::cmd::vault_disaster_recovery_export,
        ])
        .build()
}

#[cfg(not(feature = "vault"))]
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::<R>::new("vault").build()
}
```

`plugins.rs` 无条件注册（**装配层零 cfg**）：

```rust
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder
        .plugin(tauri_plugin_opener::init())
        // ...
        .plugin(crate::vault_plugin::init())
}
```

`ipc.rs` 只剩 core 57 条，**无任何 cfg**。同时 `Exit` 事件也不再需要双分支（见 3.8.2 的双实现桩）。

前端成本已实测为「一行」：vault 的全部 70 处 `invoke` 都集中在 `src/vault/core/vault.service.ts` 一个文件，且第 1 行是**别名导入**（现状直接用裸命令名，文件内没有 `plugin:` 字面量）：

```ts
// 现状 L1
import { tauriInvoke as invoke } from "@/services/tauriInvoke";

// 目标 L1（唯一改动）
import { tauriInvoke } from "@/services/tauriInvoke";

const invoke = <T = void>(cmd: string, args?: Record<string, unknown>) =>
  tauriInvoke<T>(`plugin:vault|${cmd}`, args);
```

**注意不做前缀裁剪**（v2.2 修正）：后端注册名是函数名 `vault_init`，所以 `plugin:vault|vault_init`；原文的 `${cmd.replace(/^vault_/, "")}` 会拼出 `plugin:vault|init`，运行期 command not found。不裁剪还有一个附带好处——`cmd` 参数与后端注册名保持**逐字一致**，3.8.5 的「FE 字面量 ↔ 后端注册名」交叉校验可以做成纯字符串集合比对，不需要归一化规则。

70 个调用点一个字都不用改（它们都不带显式泛型，返回类型由上下文推断，泛型 `<T = void>` 保证推断仍然成立），命名空间转换只发生在这一层。这是插件化唯一的外部成本，且已被这一层吸收。3.2.1 的 `featureInvoke(vaultModule)` 落地后，这两行只是换成 `const invoke = featureInvoke(vaultModule);`，调用点仍为零改动，`plugin:` 字面量也收敛到 `featureIpc.ts` 一处。

**插件化还有一处后端成本：capability**（见 3.8.3a）。前端这一行不覆盖它，因此 3.8.3 的验收必须包含「spike 选定的 ACL 形态已落地且禁用构建下 capability 校验通过」。

为什么不选「两个 handler + 前缀分派」：它能压到 2 行 cfg，但把「哪些命令属于 vault」变成运行期字符串判断——前缀写错不会编译失败，只会在运行时报 command not found；而插件边界让「不属于 vault 的命令」在**编译期**就不存在。两者成本几乎相同（都是 1 对 cfg），选编译期约束的那个。

#### 3.8.4 重导出与副作用收敛

| 项 | 现状 | 目标 |
|---|---|---|
| 重导出 | `pub use vault::*` | 显式 `pub use vault::{vault_cleanup_on_exit, ...}`；新增 `pub fn` 不再自动泄漏进 `cmd` |
| 副作用 | vault 双函数桩 + orb 注册表两套 | 统一为 `CONF_SIDE_EFFECTS` 注册表，按 group 字符串索引；vault 在自己的 cfg 模块里注册 `"vault"` 项，`config.rs` 只查表 |
| 注册表自证（v2.2 新增） | 「注册」与「遍历」是两份手写集合：`register()` 建表，消费处却用字面量数组 | **注册集合 == 遍历集合**，由测试断言双向差集为空；否则新注册项永远不会被消费，且编译不报错（参考项目 OpenBitFun 的 `validate_config` 正是这样退化的：`register()` 建 HashMap，遍历却写 `["ai", "appearance", ...]`） |
| 桩签名一致（v2.2 新增） | `vault_cleanup_on_exit` 的桩与真实现是两个独立函数，签名漂移时编译通过、运行期才崩 | 桩签名必须与真实现同形（仅允许参数名前缀 `_`），由门禁做归一化比对（见 3.8.5） |

`CONF_SIDE_EFFECTS` 的装配放在 `cmd/mod.rs`（后端 cfg 白名单两个文件之一，见 3.8.5；这里只允许「注册一项」，不允许「逐条 cfg」）：

```rust
type SideEffect = fn(&tauri::AppHandle, &serde_json::Value, &serde_json::Value);

pub(crate) fn conf_side_effects() -> Vec<(&'static str, SideEffect)> {
    let mut effects = vec![
        ("orb", orb_side_effects as SideEffect),
        ("app", app_side_effects as SideEffect),
    ];
    #[cfg(feature = "vault")]
    effects.push(("vault", vault_side_effects as SideEffect));
    effects
}
```

消费侧必须从同一个函数取集合，而不是另写一份字面量：

```rust
// 反例（禁止）：for group in ["app", "orb", "vault"] { ... }
for (group, effect) in conf_side_effects() { ... }
```

`assert_registry_closure()` 校验三件事：① 每个注册的 group 在 assistant-config 里存在对应 struct（`#[serde(default)]`）；② group 键无重复；③ 遍历点只能消费 `conf_side_effects()` 的返回值——用 `grep` 门禁拦截「同一 group 名字面量在 `config.rs` 里出现第二次」。

**桩签名一致性**为什么必须是门禁而不是约定：`#[cfg(not(feature = "vault"))] pub fn vault_cleanup_on_exit(_app: &AppHandle) {}` 与真实现 `pub fn vault_cleanup_on_exit(app: &AppHandle)` 是两个**互不相关的函数**，真实现加一个参数时桩不会报错（桩自己编译得过），只有禁用构建下调用点才炸。参考项目把这套规则做成脚本（`normalizeConditionalSignature` 把 `_app:` 归一化为 `app:` 后要求两种实现签名集合大小为 1），本文照抄该做法，但放在 Rust 侧（`assembly_contract.rs` 解析两个 `#[cfg]` 分支的函数签名并比对）。

**落地偏差（v2.3）**：上面的 `SideEffect` 草图只承载「副作用」，落地时把注册表项升级为「整段 apply」，类型命名为 `ConfApplier`：

```rust
pub(crate) type ConfApplier = fn(&tauri::AppHandle, serde_json::Value) -> Result<(), String>;

pub(crate) fn conf_side_effects() -> Vec<(&'static str, ConfApplier)> {
    let effects: Vec<(&'static str, ConfApplier)> = vec![
        ("app", config::apply_app_conf),
        ("orb", config::apply_orb_conf),
        ("mascot", config::apply_mascot_conf),
    ];
    #[cfg(feature = "vault")]
    let effects = effects
        .into_iter()
        .chain(std::iter::once(("vault", vault::apply_conf as ConfApplier)))
        .collect();
    effects
}
```

两条理由：

1. **`patch_conf` 必须零 group 分支**。若注册表只给副作用，`patch_conf` 仍需 `match group` 做 load/amend/validate/save，group 字面量就会在 `config.rs` 里出现两次，直接违反上面那条 grep 门禁。把整段 apply 收进注册表后，`patch_conf` 退化为「查表 → 调用」，`config.rs` 内 group 字面量计数为 **0**（实测 `grep '"app"|"orb"|"mascot"|"vault"'` 无命中）。
2. **弱签名是刻意的**。四个 group 的配置 struct 类型互不相同，强签名（`&AppConf` / `&VaultConf` / ...）无法放进同一个 `Vec`，只能给每个 group 再套一层 `Value → T` 的适配器，`cmd/mod.rs` 就从「注册一项」退化为「逐 group 代码」；A6 的签名归一化也会失去「天然同形」这个前提（归一化后集合大小不再是 1）。弱签名把「类型差异」下沉到各 group 自己的 applier 内部。

因此分层是：`apply_app_conf` / `apply_orb_conf` / `apply_mascot_conf` 留在 core（`cmd/config.rs`），内部仍是强类型链路（`AppConf::load → amend → validate → save → 既有副作用表`），编译期约束没有丢；`vault::apply_conf` 留在 `cmd/vault/mod.rs`，`cmd/mod.rs` 只认识一个函数路径，不知道 `VaultConf` 的存在——这才符合「注册一项」。

同时，`pub use vault::*` 通配重导出已取消：`vault_plugin.rs` 的命令表直接用 `crate::cmd::vault::<cmd>` 路径引用，`cmd/mod.rs` 的重导出只剩 `vault_cleanup_on_exit` 一项（与桩配对）。全后端 vault 相关 `#[cfg]` 实测共 6 行（`cmd/mod.rs` 4 + `vault_plugin.rs` 2），落在 3.8.5 的 A2 上限内。

#### 3.8.5 门禁：把可计数约束变成集合断言

新增 `src-tauri/tests/assembly_contract.rs`，六条断言，任何回退直接红：

| # | 断言 | 实现 |
|---|---|---|
| A1 | 注册表文件内无 cfg | 读 `src/ipc.rs`，`count("#[cfg") == 0` |
| A2 | 全后端 cfg 只出现在边界 | 递归扫描 `src/`，`#[cfg(feature = "vault")]` 只允许出现在 `vault_plugin.rs` 与 `cmd/mod.rs` 两个白名单文件（`setup.rs` 在 3.8.3 后必须为 0，因此不在白名单——这是 G1 从 8 收紧到 6 的验收条件）。**E7 未完成前**该断言按 G1 的宽松口径（三文件白名单 + ≤8 行），E7 完成后改为两文件 + ≤6 行，避免门禁先于改造落地而长期红灯 |
| A3 | lib.rs 只做装配 | `lib.rs` 行数 ≤ 40，且不含 `generate_handler!` 字面量 |
| A4 | 注册集合 == 声明集合（v2.2 改写） | **双向差集，不是计数**：`registered.difference(&declared)` 报「注册了但未声明」，`declared.difference(&registered)` 报「声明了但未注册」。计数断言只能防「少一条」，防不住「错一条 + 多一条」。声明清单新增 `src-tauri/commands.json`（core 57 + vault 66，与注册表变更同一提交更新；前端 `featureIpc` 的命令字面量也从此清单派生） |
| A5 | 解析器自检下限（v2.2 新增） | 断言 `registered.len() >= 120`（拆分后若解析器只抓到 3 条，A4 的两个差集仍可能通过，必须用下限拦住「解析器坏了」这种假绿）。参考项目的阈值是 `> 400`，本文按 123 条给 110 下限（留 13 条余量，防止「漏解析一整块」仍假绿） |
| A6 | 桩签名同形 + 三态分类（v2.2 新增） | 解析所有 `generate_handler![` 块的条目，先 `strip_rust_comments` 再按逗号切分；每条目按 `^[A-Za-z0-9_:]+$` 校验（**cfg 条目直接报错**）；再对每个条目归类为 `core` / `plugin` / `hostOnly` 三态，任一命令必须恰好命中一态（参考项目 `HostInvokeOnly` 的同类设计）。桩与真实现的签名归一化后必须一致 |

A4/A5/A6 的解析器细节（照抄参考项目，避免重复踩坑）：**先剥 Rust 注释再切分**——否则注册表里一行 `// vault_xxx 待删` 会被当成条目，多块解析时尤其容易命中；条目必须匹配 `^[A-Za-z0-9_:]+$`，这条正则本身就是「注册表内不得出现 cfg」的 CI 化表达，而不是靠 review。

现有 `src-tauri/crates/assistant-vault/tests/command_registry.rs` 同步改造：扫描路径从 `src/lib.rs` 改为 `src/vault_plugin.rs`，解析**所有** `generate_handler![` 块而非「遇 `])` 即停」，并新增反向断言「core 注册块内不得出现 `vault_` 前缀」。这一步是 3.8.2 的硬前置——不改测试，拆分当天就会误报。

**前端 ↔ 后端交叉校验**（v2.2 新增）：`featureIpc.ts` 的 `featureInvoke` 不做前缀裁剪（见 3.8.3），因此前端传给 `invoke` 的字符串与后端注册名**逐字一致**。门禁因此可以是一条纯集合比对：抽取 `src/vault/core/vault.service.ts` 内所有 `invoke("...")` 字面量，与 `vault_plugin.rs` 的注册集合做双向差集。这条断言能拦住「前端拼错命令名」这类只在运行期暴露的缺陷（参考项目靠 guest-js 的单测断言 invoke 字符串，我们靠生成清单 + 集合比对，成本更低）。

### 3.9 参考项目复核：采纳与拒绝（v2.2 新增）

本节记录「为什么上面某些条款长这样」的外部依据，避免后续评审重复讨论。样本来自 `references/1-ai-agent/` 的 13 个项目，重点三个：OpenBitFun（配置 provider 注册表 + 400+ 命令注册块）、openhuman（Tauri 插件标准实现 + 设置路由注册表）、ChatGPT（极简硬编码配置）。

| 采纳项 | 参考实现 | 落到本文 |
|---|---|---|
| 集合相等断言（双向差集） | OpenBitFun 解析 `generate_handler!` 后 `registered.difference(&declared)` / `declared.difference(&registered)`，并带 `> 400` 的解析器自检 | 3.8.5 的 A4 + A5 |
| 解析器工程细节 | 同一脚本：先剥注释再切分、条目正则 `^[A-Za-z0-9_:]+$` 拒绝 cfg | 3.8.5 A6 的实现约束 |
| 桩签名一致性归一化 | `normalizeConditionalSignature` 把 `_app:` 归一化为 `app:`，签名集合大小必须为 1 | 3.8.4 桩签名 + 3.8.5 A6 |
| 命令恒注册 / 可选性用桩 | OpenBitFun AGENTS.md：debug-only 命令「provide no-op stubs so commands can always be registered」 | 3.8.2 的 `Exit` 双实现桩；3.8.3a 记录为 Plan B |
| 配置校验批量收集 | OpenBitFun `ConfigValidationResult` 收集 errors + warnings，warning 带 `field` / `code` | 3.5「批量收集，不 fail-fast」 |
| 插件命令名零转换 | openhuman ptt 注册 `commands::start_listening`，前端 invoke `plugin:ptt\|start_listening` | 3.8.3 前缀修正（不剥 `vault_`） |
| 侧栏条目与路由解耦 | openhuman `SettingsRegistryEntry` 独立 `id` + `navParent`，`devOnly` / `hiddenDeepLink` 正交 | 3.6 的 `sidebar.key` + `navParent` + `visibility` |
| 条目自带搜索词 | openhuman `searchKeywords`；本仓库 `SettingsPageManifest.keywords` 已存在 | 3.6「命令面板匹配词」行 |

| 拒绝项（参考项目里的反例） | 问题 | 本文对应处置 |
|---|---|---|
| 注册表被字面量数组绕过 | OpenBitFun `validate_config` 用 `["ai", "appearance", ...]` 遍历，新 provider 注册后永不生效且编译不报错 | 3.8.4「注册集合 == 遍历集合」+ `assert_registry_closure()` |
| 元数据表与渲染表双源 | openhuman 的 `settingsRouteRegistry.ts` 与 `settingsRouteElements.tsx` 各写一份、靠人肉对齐 | 坚持单源 + 黄金快照，不引入 elements 表 |
| 可见性字段堆叠 | openhuman 有 `devOnly` / `hiddenDeepLink` / `navGroup` / `navParent` / `highlight` 五个相关字段，扩展作者难以选择 | 收敛为 `visibility`（两值）+ `navParent` 一个可选字段，其余交给业务侧 |
| 配置面下沉到插件 | 若把 `get_vault_conf` / `patch_conf` 也移进插件，同一配置面会同时存在裸名与命名空间两种 invoke 形态 | 1.7/N5：明确不做 |

**一条没有采纳的候选**：OpenBitFun 把 `default_config` / `validate` / `on_changed` 三件事绑在同一个 `ConfigProvider` trait 上。本文只采纳「副作用注册表」这一层，默认值仍留在 `assistant-config` 的 `#[serde(default)]`——因为默认值已有单一事实源（Rust struct），在前端槽位再声明一份 `default` 会立刻产生两份需要人工同步的默认值。**配置的四个事实（默认值 / 校验 / 副作用 / 编辑页）分别落在最合适的一侧，而不是为了对称硬塞进一个对象**，这正是 3.2.1「前后端契约不对称」的处置原则。

### 3.10 约束的取舍：必要约束 vs 防御性约束（v2.3 新增）

本文有大量「禁止 / 必须 / 抛错 / 断言」句式，读起来像不信任扩展作者。先说清成因：**每一条约束都对应一个已经发生过的静默失败**——1.3 的 I1–I7（新注册项永不生效、页面空白无报错）、1.4 的 S1–S4（注册表被 cfg 撑爆后无法判断某命令属于谁）、1.5 的 A1–A2（类型说「一定有」、运行时是 `undefined`）。这些失败的共同特征是**不报错**，所以对策只能是「提前断言」。

但「有失败史」不等于「该加约束」。判定一条约束该不该留，用三条标准：

1. **能用类型表达的，不要用 lint 或文档约定**——类型错误编译期必红，约定会随人和时间腐化。
2. **能用一次性断言表达的，不要用逐处规则**——断言的维护成本是 O(1)，逐处规则的维护成本随代码量增长。
3. **没有真实用例的取值，不进枚举**——枚举每加一个值，所有消费方都要处理它，这是纯预付成本。

按这三条逐项复核，本文的约束分成三档：

| 约束 | 回应的失败模式 | 能否结构替代 | 处置（v2.3） |
|---|---|---|---|
| 快照 10 条不变式（DEV 抛错、生产丢弃） | I1–I5：声明写错 → 空页面且无报错 | 部分：字面量能靠类型，**集合关系只能运行时断言** | 保留 |
| 校验批量收集 `Violation[]` | 扩展作者改一处跑一次、返工多轮 | 不适用 | 保留（成本与 fail-fast 相同，收益明确） |
| 类型收窄（`ConfStore` 映射类型 + `FeatureConfSlot<G>` 泛型绑定） | A2：类型说「一定有」；`T = unknown` 让槽位值类型被擦除 | **可以**（纯类型层，零运行时成本） | 保留，本轮已修正 3.2 |
| ESLint 禁止 `confs.<group>` 直接访问 | 访问出口分散，Phase 变更时漏改 | 部分（访问器不足以强制） | **降级**：只拦直接属性访问，放行变量下标，不要求迁移存量（3.2 约束 3） |
| ESLint 禁止 `plugin:` 字面量 | 命名空间漂移 | **可以**：`featureInvoke` 已是唯一出口 + 6.2 命令名集合比对已覆盖 | **降级为建议**：只约束 `src/features/**` 与新增 service，不设全局规则 |
| `visibility` 三值（`sidebar` / `nav` / `deepLinkOnly`） | 单一可见性开关混用「无 section 合法」与「不进导航」两种语义（当前代码尚无该字段） | 不适用 | **收缩为两值**：只留 `sidebar` / `deepLinkOnly`；`nav`（进命令面板但不进侧栏）目前**无任何真实用例**，等出现第二个特性再加，且加时只需扩一个联合成员 |
| A1–A6 六条装配断言 | 注册表回退、桩签名漂移、前端拼错命令名 | 部分 | 保留，但**分两步落**：E6 先落 A2/A3/A4（注册表闭合 + 双向差集），E7 再补 A5/A6 |
| G1 ≤8 / ≤6 | 承诺不可达（v2.1 曾把上限写错） | 不适用 | 保留，但标注「E6a 后复核」——它是**验收线而非设计约束** |
| Plan B（桩化） | ACL 成本不确定 | 不适用 | **移出主路径**：只作为 3.8.3a 的结论分支，不参与日常决策 |
| 相位顺序 E0→E1→E3→E2 | 顺序错乱导致返工 | 不适用 | 保留（依赖真实存在：E3 依赖 E0 的快照构建点，E2 依赖 E1 的绑定方向） |
| 三份黄金快照 + 6.3 探针演练 | 回归、以及「扩展面」被口头宣称 | 不适用 | 保留 |

两处**表述**问题也一并修掉，它们让文档显得比设计更僵硬：

- 「不再讨论」（R3）、「明确不做」（第 8 章）这类治理语句，作用是**冻结已决结论**、避免评审反复；但它们不该覆盖推理过程。因此本文的证据链（1.2–1.7、3.9、本节）应视为**附录**（评审时读），主线 3.1–3.8 只留契约（实施时读）。
- 「口径」一词在本文出现约 20 次，多数场合其实是「验收阈值」。建议实施时以第 5 章的验收列 + 3.8.5 的断言为准，正文里的「口径」不必逐条记忆——**能被执行的就是断言，不能被执行的口径只是叙述**。

---

## 4. 迁移映射（旧 → 新）

| 旧位置 | 动作 | 新位置 |
|---|---|---|
| `settings/core/field/fieldRegistry.ts` 的 `CONF_KEY_MAP` / `CONF_LOAD_KEY_MAP` | 删除 | `settings/core/types/confRegistry.ts` + `confRuntime.ts` |
| `settings/core/field/fieldRegistry.ts` 的 6 个查询函数 | 改为读快照 | `settings/core/query.ts` |
| `settings/core/field/fieldRegistry.ts` 的 `definitions` / `sections` 容器 | 保留（内置通道），末尾 `bumpBuiltinVersion()` | 同文件（更名 `builtinRegistry.ts`） |
| `settings/core/pageRegistry.ts` 的 `derive()` | 改为读快照 | `settings/core/snapshot.ts` |
| `settings/core/pageRegistry.ts` 的 `getSettingsPages` / `getSettingsDetailPages` | 合并为一个 `getSettingsPages()`（两者当前返回同一数组） | `query.ts` |
| `settingsDispatcher.ts` 的 `ConfOf` 条件类型 | 删除 | `confRegistry.ts` |
| `settingService.ts` 的 `getVaultConf` | 删除 | `confRuntime.fetchConf()` |
| `settings/core/types/confUtils.ts` 的 `toConfRecord`（入参为闭合联合 `ConfUnion`，含 `VaultConf`） | 泛化为 `<T extends object>(conf: T \| null)`；`as unknown as` 仍只此一处 | 同文件 |
| `settings/core/types/confTypes.ts` 的 `VaultConf` | 迁出 core，改为 vault 特性自有类型 + `declare module` 声明合并 | `features/vault/conf.ts`（或 `features/vault/types.ts`） |
| `optimisticUpdate.ts` 的 `CONF_KEY_MAP[group]` | 改查槽位 | `confRuntime.getConfSlot()` |
| `SettingsOverview.tsx` 的 `confMap` | 动态构造 | `getConfGroups()` |
| `commandAggregation.tsx` 的 `GROUP_TO_PAGE` | 删除 | `snapshot.confPageMap`（来源改为 `confSlot.pageId`，见 3.2） |
| `settings/overview/SettingsOverview.tsx` 与 `settings/shared/sections/SettingsGroup.tsx` 两份 `useConfByGroup` | 合并为一份 | `settings/core/useConfGroup.ts` |
| `services/settingEvents.ts` 的 group 硬编码分支 | 改为 `isConfGroupAvailable(group)` 判定 | 同文件 |
| 各特性 service 内的 `plugin:xxx\|` 字面量 | 收敛到一处 | `services/featureIpc.ts` 的 `featureInvoke()` |
| `features/registry.ts` | 每特性一段编译期守卫 + 动态 import（清单手写，不生成 manifest） | 同文件 |
| `settings/pages/VaultSettingsPage.tsx` | 迁移 | `features/vault/settings/VaultSettingsPage.tsx` |
| `settings/shared/renderers/BiometricSwitchRender.tsx` | 迁移 | `features/vault/settings/BiometricSwitchRender.tsx` |
| `src-tauri/src/cmd/config.rs` 的 `apply_vault_side_effects` 双函数 | 删除 | `cmd/mod.rs` 的 `conf_side_effects()` |
| `src-tauri/src/lib.rs` 的 `.plugin(...)` 六连 | 迁移 | `src-tauri/src/plugins.rs` 的 `register()` |
| `src-tauri/src/lib.rs` 的 `generate_handler!` 注册表 | 迁移 + 拆分：core 57 条 → `ipc.rs`，vault 66 条 → `vault_plugin.rs`（均无逐条 cfg） | `src-tauri/src/ipc.rs` 的 `handler()`、`src-tauri/src/vault_plugin.rs` 的 `init()` |
| `src-tauri/src/lib.rs` 的 `on_menu_event` | 迁移 | `src-tauri/src/menu.rs` |
| `src-tauri/src/lib.rs` 的 `on_window_event` | 迁移 | `src-tauri/src/window.rs` |
| `src-tauri/src/lib.rs` 的 `SHOULD_EXIT` / `request_exit` / `.run(...)` 闭包 | 迁移 | `src-tauri/src/lifecycle.rs` |
| `src-tauri/src/lib.rs` 的 `Exit` 事件双分支 | 删除，改无条件调用 + `cmd/mod.rs` 双实现桩 | `src-tauri/src/lifecycle.rs` |
| `src-tauri/crates/assistant-vault/tests/command_registry.rs` 的 `src/lib.rs` 硬编码与单块解析 | 改扫描 `src/vault_plugin.rs` + 多块收集 + core 前缀反向断言 | 同文件 |
| `src/vault/core/vault.service.ts` 的 70 处 `invoke("vault_xxx")` | 调用点不改；导入改 `tauriInvoke`，本文件内 `invoke` 改为 `plugin:vault\|` 转发 | 同文件 L1 + 新增 3 行 |
| `src-tauri/src/setup.rs` L90 的 `#[cfg(feature = "vault")] vault_setup(...)` | 删除，钩子移入 `vault_plugin::init()` 的 `.setup()` | `src-tauri/src/vault_plugin.rs` |
| 无（新增，v2.2） | 新增命令声明清单（A4 的 `declared` 来源） | `src-tauri/commands.json` |
| 无（新增，v2.2） | 按 E6a spike 结论新增插件权限面 | `src-tauri/permissions/**` + `src-tauri/capabilities/default.json` 一行 |
| `src/i18n/locales/zh-CN.json` / `en-US.json` 与 `i18next.d.ts` | **本阶段不改**（见 1.7/N1）：新特性文案仍需改这两个核心文件 | 后续独立阶段 |

---

## 5. 实施分层

| 期 | 内容 | 涉及文件 | 验收 |
|---|---|---|---|
| **E0** 快照与索引 | 新增 `types/snapshot.ts`、`snapshot.ts`、`query.ts`；core 查询改读快照；内置容器加版本号 | `settings/core/**` | 行为零变化：黄金快照测试 diff 为空；`getAllDefinitions()` 连续两次调用返回同一引用 |
| **E1** 依赖方向拉直 | `fieldRegistry` / `pageRegistry` 删除 `@/features/registry` import；`loadFeatures()` 内 `bindExtensionInput()` | `settings/core/**`、`features/registry.ts`、`main.tsx` | `settings/**` 下 grep 不到 `@/features`；应用启动正常、设置页可打开 |
| **E2** 配置面通用化 | `confRegistry.ts` + `confSlot.ts` + `confRuntime.ts`；store 改 `confs: ConfStore`（逐键可选 + `isConfGroupAvailable`）；删除 C1–C9 九处闭合点；两份 `useConfByGroup` 合并为 `useConfGroup`；`settingEvents` 改槽位判定；`featureIpc.ts` 收敛 `plugin:` 字面量 | 见第 4 章前 10 行 | vault 配置读写、乐观更新、跨窗口同步、命令面板跳转全部行为不变；`ConfGroup` 仍覆盖 app/orb/mascot/vault；`getConfValue("vault")` 的返回类型收窄为 `VaultConf \| null \| undefined`（`tsc` 即可验证 `ConfStore` 生效，不依赖 lint） |
| **E3** 校验与引用完整性 | `validate()` 进快照构建（DEV 抛错 / 生产丢弃）；`validate()` 返回 `Violation[]` 批量收集；`insertUnique()` 统一两条通道（落 `core/insertUnique.ts`，内置容器与 `aggregate` 共用）；页面缺失改「未找到」面板（守卫与 `getSettingsPageManifest` 两处都要改） | `snapshot.ts`、`builtinRegistry.ts`、`insertUnique.ts`、`aggregate.ts`、`SettingsContent.tsx` | 构造错误声明（漏 category / 漏 section / 重复 key 等**当前可表达**的规则）**一次性全部抛出**；`/settings/vault` 在无 vault 构建下显示未找到面板而非跳总览。**两条规则随字段落地推迟**：`field.group` 槽位校验 → E2（槽位动态化后才可校验），`navParent` / `sidebar.key` 悬空 → E5（字段在 E5.4 新增） |
| **E4** 构建开关单源 | 手写 `config/features.json` 为单源（**不生成 manifest**）；`vite.config.ts` 直接读清单派生 define；`feature-build.ps1` 读同一清单并断言与 `Cargo.toml [features]` 一致 | `config/`、`scripts/`、`vite.config.ts` | 一条命令产出无 vault 构建；vault chunk 数为 0、产物无 `vaultAutoLockMinutes`；清单与 cargo 不一致时脚本启动即中止 |
| **E5** 排序 / 插槽 / key 契约 | `order` 分命名空间；slot 扩到 4 值；`sidebar.key` 必填 + 全局唯一；新增 `page.navParent` / `visibility` / `emptyPage`（均为新增字段，当前无 `navOnly`）；`FeatureRoute` 补 `keywords` | `features/types.ts`、`aggregate.ts`、`AppSidebar.tsx`、`pageTypes.ts` | 打乱 vault `routes` 数组顺序后侧栏 key 与高亮不变；四个插槽各放一个探针条目位置正确；「两页共享一个侧栏条目」的用例高亮正确；`deepLinkOnly` 页面不出现在侧栏与命令面板但深链可达 |
| **E6** 后端装配拆分（零行为变化） | `lib.rs` 只留装配；`plugins.rs` / `ipc.rs` / `menu.rs` / `lifecycle.rs` 抽出；`window::on_window_event` 入 `window.rs`；`SHOULD_EXIT` 改 `lifecycle::should_exit()`；`cmd/mod.rs` 加 `vault_cleanup_on_exit` 双实现桩；扫描测试改多块解析 | `src-tauri/src/**`、`crates/assistant-vault/tests/command_registry.rs` | `lib.rs` ≤ 40 行；`cargo check`（开/关 feature 两种）通过；扫描测试双向断言通过；前端行为零变化 |
| **E6a** ACL spike（v2.2 新增，半天量级） | 1 条命令 + 1 个窗口验证插件命令可调用；对比「独立 crate + `permissions/default.toml`」与「`build.rs` 内联插件 + 逐条 `allow-*`」两条路线的 capability 成本 | `src-tauri/build.rs`、`capabilities/default.json`、临时 `spike_plugin.rs` | 三条结论落表（S1/S2/S3，见 3.8.3a）：插件命令可用 / 最小 capability 形态 / 关闭特性时的报错阶段；据此确认或下调 G4/G5 口径。**spike 未通过时 E7 改走 Plan B（桩化）** |
| **E7** 后端 cfg 归零（插件化） | 新增 `vault_plugin.rs`（含 `.setup()` 钩子，接住 `cmd::vault::vault_setup`）；`plugins.rs` 无条件注册；`ipc.rs` 只留 core 57 条；`setup.rs` cfg 归零；按 E6a 结论落地 capability / `permissions/default.toml`；前端 `vault.service.ts` 改 `featureInvoke(vaultModule)`；新增 `assembly_contract.rs` 门禁；`conf_side_effects()` 注册表 | `src-tauri/src/**`、`src-tauri/capabilities/**`、`src/vault/core/vault.service.ts`、`src/services/featureIpc.ts` | G1 ≤ 8（本阶段验收）/ ≤ 6（进阶，取决于重导出收敛；`setup.rs` 归零是收紧前提）、G2 = 0、G4 = 1 对 cfg + 1 组 ACL（按 E6a 结论）；`assembly_contract.rs` **六条**断言通过（含 A4 双向差集 / A5 解析器下限 / A6 三态）；前端 ↔ 后端命令名集合比对通过；vault 全部 70 个调用点无改动即可工作（密码读写 / TOTP / 同步 / SSH / 生物识别全链路回归） |
| **E8** 资产 colocate | vault 设置页与渲染器迁入 `features/vault/settings/` | `features/vault/**`、`src/settings/**` | `src/settings/**` 下 grep 不到 `@/vault`。**落地**：两文件迁入 `features/vault/settings/`，跨目录引用统一改 `@/settings/**` 别名，`module.ts` 两处动态导入改相对路径 `./settings/*`；实测 `src/settings/**` 命中 0 |

依赖顺序：**E0 → E1 → E3 → E2 → E4 → E5 → E6 → E6a → E7 → E8**。E0/E1 是纯结构重构，先做且必须行为零变化；E3 依赖 E0 的快照构建点；E2 依赖 E1 的绑定方向；E4/E5/E8 相互独立，可并行；E7 必须在 E6 之后（E6 是它的文件前提），且必须在 E6a 之后（ACL 口径决定 E7 的验收线）。

---

## 6. 测试与门禁

### 6.1 黄金快照测试（本阶段最关键的一道护栏）

新增 `src/settings/__tests__/settingsSnapshot.golden.test.ts`：

```
序列化对象 = {
  pages: [id, categoryId, path, visibility, emptyPage, navParent, order, keywords],
  categories: [id, label, order],
  sections: [id, pageId, title, order],
  fields: [key, group, section, type, order],
  routes: [path, visibility, navParent, order, keywords],
  sidebar: [key, slot, order],
  confPageMap: [[group, pageId]]
}
```

快照序列化字段包含 `visibility` / `emptyPage` / `navParent`（三者均为新增，当前代码无 `navOnly`）——仓库当前尚无既有快照 fixture，E0 直接建立首版黄金快照；之后任何字段增删都必须显式更新并 review。

三份 fixture，任何一份变化都必须显式更新并 review：

| fixture | 输入 | 用途 |
|---|---|---|
| `snapshot.full.json` | `aggregate([vaultModule])` | 全量构建未漂移 |
| `snapshot.vaultless.json` | `aggregate([])` | 排除构建未漂移 |
| `snapshot.diff.json` | 两份的差集 | 排除面**恰好**等于 vault 的全部贡献，多一个少一个都失败 |

`snapshot.diff.json` 是这套测试的核心：它把「无 vault 构建到底删掉了什么」变成可 review 的清单，也顺带替代了靠 grep 字面量的验收方式。

### 6.2 结构不变式测试

`src/settings/__tests__/snapshotInvariants.test.ts`，对 5 类故意错误的声明断言抛错（见 E3 验收）。这类测试的价值在于：**扩展作者写错时，失败发生在构建期而不是用户看到的空页面**。

v2.2 追加两类「注册表被绕过」的回归测试（这类缺陷比声明写错更隐蔽——代码能编译、能运行，只是新注册项永不生效）：

| 类 | 断言 | 失败形态 |
|---|---|---|
| 注册表闭合 | 前端 `registerConfSlot` 的键集合 == `getConfGroups()` 返回的集合；后端 `conf_side_effects()` 的 group 集合 == `assistant-config` 中存在的 struct 集合 | 新 group 注册后不被遍历 / 配置页不出现，但无报错 |
| 命令名一致 | `vault.service.ts` 内 `invoke("...")` 字面量集合 == `vault_plugin.rs` 注册集合（双向差集） | 前端拼错命令名，只在用户操作时报 command not found |

### 6.3 扩展面演练（E8 收尾的硬门槛）

在 `src/features/_probe/` 放一个最小探针特性：一条 route、一个 sidebar 条目、一个 category、一个 page、一个 section、两条 field、一个 conf 槽位（声明 `pageId`）、`ipc.namespace = "probe"`、一个后端 mock 命令（以 `probe_plugin.rs` 注册）。要求：

- 除 `features/_probe/**`、`src-tauri/src/probe_plugin.rs`、`plugins.rs` 的一行、`config/features.json`、以及 **capability 里的一行 `probe:default`（或 `probe:allow-probe_echo`）** 外**零改动**——v2.2 把 ACL 列入白名单：插件命令没有 capability 就调不通，若仍宣称「只改 4 处」，探针会在运行期失败且原因不在探针代码里；
- 探针的 `visibility` 用 `"sidebar"`、`emptyPage: false`、`navParent` 缺省，确保主链路（侧栏 + 导航 + 深链）三面都被覆盖；
- 探针文案用**字面量**，不碰 `zh-CN.json` / `en-US.json`（i18n 面不在本阶段范围，见 1.7/N1）；
- `tsc --noEmit`、lint、全部测试通过；
- 开启该特性后设置页出现探针页并可读写配置；关闭后产物中无该特性代码；
- 探针的 `conf` 命令走 core 裸名（`get_probe_conf`），后端 mock 命令走 `featureInvoke(probeModule)` 拼 `plugin:probe|`——这条同时验证 3.2.1 的两种 invoke 形态。

演练通过后删除 `_probe`，并把「新增特性不触碰 core」写成 CI 断言：对比改动路径白名单（白名单含 `config/features.json`、`features/registry.ts`、`plugins.rs`、`capabilities/*.json`、`permissions/**` 五类装配点，每类都是「每特性一行 / 一段」，而非「每命令一行」）。**`settings/**` 与 `src-tauri/src/{ipc,setup,menu,window,lifecycle}.rs` 必须零改动**——这是本阶段真正要守住的口径；`features/registry.ts` 因清单手写、无生成物，需要每特性约 4 行守卫 + 动态 import，它属于「特性装配点」而非 core。

### 6.4 现有门禁保持不变

`npm run lint`、`tsc --noEmit`、`cargo check --no-default-features`、`cargo check`、前端扫描测试（L3）继续作为准入条件；新增 `assembly_contract.rs`（3.8.5 的 A1–A6 六条断言）与 `command_registry.rs` 的双向断言作为后端装配准入条件。

**门禁设计原则**（v2.2）：门禁只断言「集合关系」，不断言「计数」。计数会随实现漂移（拆分、合并、增删注释都会变），而集合关系是设计意图本身（注册集合 == 声明集合、桩签名 == 真实现签名、前端命令名 == 后端注册名）。唯一保留的计数是**解析器自检下限**（A5）——它的目的不是约束设计，而是防止解析器静默失效导致「假绿」。

---

## 7. 风险与对策

| # | 风险 | 影响 | 对策 |
|---|---|---|---|
| R1 | 声明合并让 `ConfGroup` 在 typecheck 期恒为完整集合 | 类型层不反映「无 vault 构建」，`confs` 若声明为全量必填则「类型说谎」 | **有意设计**，但必须配套三条约束（3.2）：`confs` 用逐键可选的映射类型 `ConfStore`、提供 `isConfGroupAvailable`、读值只走访问器。运行时正确性由 E4 的清单与一致性断言保证，两条口径各管一段 |
| R2 | 快照缓存引入状态，调试时可能看到「旧快照」 | 排查困难 | 快照带 `version` 与 `extensionId`，DEV 下暴露 `__settingsSnapshotMeta()`；单测覆盖双键失效；相位契约（3.3）禁止「先查询后绑定」 |
| R3 | 校验策略影响可用性 | 抛错太严导致白屏，太松导致静默缺失 | 已定档：DEV / 测试抛错，生产 `log.error` + 丢弃条目（3.5 策略 3）。**不再讨论**「生产也白屏」——静态声明错误由 6.1/6.2 在 CI 兜住 |
| R4 | store 从四个具名槽改为 `confs` 映射，触及约 10 个消费方 | 迁移面较宽、易漏 | 分两步：先加 `confs` 与旧槽并存（旧槽改为 getter 代理），全部消费方切换后删除旧槽；用 lint 规则禁止新增旧槽访问 |
| R5 | 清单手写，`config/features.json` 与 `Cargo.toml [features]` 是两处独立手写的事实 | 默认集合 / 特性名漂移，前端开而后端关 | 已消除「生成物被手改」这一类；剩下的漂移由 `feature-build.ps1` 启动断言双向拦截（cargoFeature 已声明、无孤儿条目、默认集合相等），不一致即中止。绕过脚本直接 `npm run tauri build` 时前端按清单默认、后端按 cargo 默认，两者本就同值，断言失配才需要人介入 |
| R6 | 拆分或插件化后，有人图省事把 cfg 条目又加回注册表 | 132 行噪声回归 | 不是「接受风险」，而是**禁止**：`assembly_contract.rs` 断言注册表内 `#[cfg]` 计数为 0、全后端 cfg 只出现在白名单文件（`vault_plugin.rs` / `cmd/mod.rs`，`setup.rs` 必须为 0），违反即 CI 红（3.8.5） |
| R7 | 页面未找到行为变化影响现有深链 | 用户书签失效 | 仅影响「访问不存在页面」，现有页面均存在；发布说明中记录 |
| R8 | 插件命令的 ACL 成本未经验证（v2.2 新增） | 若需逐条 66 项 capability，G4/G5 的「边际成本」口径失效，插件化收益被摊薄 | **先做 E6a spike 再定验收线**；spike 不过或成本过高则改走 Plan B（桩化：命令恒注册 + no-op stub），它不碰 IPC 契约与 ACL，但放弃「编译期不存在」的强约束 |
| R9 | 前端命令名与后端注册名漂移（v2.2 新增） | 拼错前缀/名字只在用户操作时报 command not found，测试环境若无覆盖则漏到线上 | 3.8.3 不做前缀裁剪 + 6.2「命令名一致」双向差集；`plugin:` 字面量收敛到 `featureIpc.ts` 一处并由 lint 限制 |
| R10 | `sidebar.key` / `navParent` 模型改动波及侧栏与路由选中态 | E5 触及 `AppSidebar.tsx`、`aggregate.ts`，高亮回归风险 | E5 验收含「两页共享一个侧栏条目」与「打乱 routes 顺序」两个用例；`selectedKeys` 推导函数单测覆盖「navParent 命中 / 回落到 page id」两条分支 |

---

## 8. 明确不做

| 项 | 原因 |
|---|---|
| 再讨论「产物零残留」 | 已由动态 import + 守卫早退达成，本阶段不重新论证 |
| cargo feature 下放 / `windows` / `assistant-crypto` 拆分 | 属前置文档 L4/L5，与本阶段正交 |
| 把 `generate_handler!` 改成运行时注册 | 不需要：Tauri v2 只接受单个 handler 且无公开组合 API，而插件边界已经解决同一问题，且约束在编译期 |
| 用 `macro_rules!` 包裹 `generate_handler!` 以消除逐条 cfg | proc macro 入参内嵌套宏不会先展开，此路不通；插件化后不再需要 |
| 引入第三方 DI / 注册框架 | 现有 `defineTool` 范式已够用，引入框架会抬高认知成本 |
| 用 `Record<string, unknown>` 代替类型化 conf | 违反项目类型安全规范；声明合并已能在保持类型安全的前提下开放集合 |
| 特性文案的 i18n 接入 | 本阶段只覆盖 UI + 配置 + IPC 三面；i18n 资源仍是两个核心 JSON（见 1.7/N1），留作独立阶段 |

---

## 9. 验收总表

| 口径 | 判定方式 |
|---|---|
| 扩展面完整（UI / 配置 / IPC 三面） | 探针特性零 core 改动即可接入（6.3）；**文案面不在本阶段**（1.7/N1） |
| 依赖单向 | `settings/**` 无 `@/features` import；`features/**` 可有 core import |
| 引用完整性 | 5 类错误声明全部构建期抛错（6.2） |
| 去重一致 | 两条通道重复 key 均抛错，无静默丢弃/覆盖 |
| 类型不说谎 | `confs` 为逐键可选的 `ConfStore`（`getConfValue("vault")` 返回 `VaultConf \| null \| undefined`）；`isConfGroupAvailable` 可用；读值走访问器（3.2） |
| 查询稳定 | 同版本快照下 `getAllDefinitions()` 引用恒定；「先查询后绑定」在 DEV 直接报错（3.3） |
| 构建单源 | 一条命令产出无 vault 构建；前后端开关不一致时构建失败；`cargoFeature` 与 `ipc.namespace` 不一致时失败 |
| 无回归 | 三份黄金快照 fixture 全部匹配 |
| 资产内聚 | `src/settings/**` 无 `@/vault` 引用 |
| 装配可计数 | `lib.rs` ≤ 40 行；注册表内 `#[cfg]` = 0；全后端 `#[cfg(feature = "vault")]` ≤ 8（本阶段）/ ≤ 6（进阶），且仅在 `vault_plugin.rs` / `cmd/mod.rs`（3.8.5 断言） |
| 可选性表达 | 新增可选特性的后端边际成本 = 1 个插件文件 + 1 对 cfg + 1 组 ACL（E6a spike 结论为准；6.3 探针演练） |
| 注册表自证 | `assembly_contract.rs` A4 双向差集为空、A5 解析器下限成立、A6 三态分类恰好命中；前端槽位键集合 == `getConfGroups()`（6.2） |
| 命令名一致 | 前端 `invoke` 字面量集合 == 后端注册集合（双向差集，6.2） |
| 侧栏模型 | 无「`sidebar.key` 必须等于 route path」约束；`navParent` 命中或回落可推导；`deepLinkOnly` 页面不进侧栏与命令面板（E5） |

---

## 10. 探针演练实测（v2.3 修订）

6.3 的白名单是在探针落地前写下的；P.1 实测后需补齐三类此前漏记的装配点，并记下演练暴露的设计缺口。逐项改动与校验结果见 `docs/store/I24-tasklist.md`「探针演练执行记录」。

### 10.1 白名单修订

6.3 原文白名单为「`features/_probe/**`、`probe_plugin.rs`、`plugins.rs` 一行、`config/features.json`、capability 一行、`permissions/**`、`features/registry.ts`」。实测还必然触及以下三处，且各自都是「每特性一行 / 一段」，与命令条数无关：

| 文件 | 为何躲不开 | 形态 |
|---|---|---|
| `src-tauri/build.rs` | Tauri v2 内联插件必须在构建期声明命令清单，否则运行期 ACL 查不到该命令 | 每特性 1 段 `InlinedPlugin::new().commands([...])` |
| `src-tauri/commands.json` | A4 双向差集的 `declared` 侧唯一事实源 | 每特性 1 个 `groups.<name>` 数组 |
| `src-tauri/Cargo.toml` | cargo 特性名与默认集合 | 每特性 1 行 `[features]` 条目 + default 集合增 1 项 |

结论：**「新增特性改动 core 文件数 = 0」的正确口径是「core 逻辑零改动」，不是「文件零改动」**。装配点随特性数线性增长，实测每特性需触碰 **11 个装配点文件**，另有 2 处特性本体新建：

| 组 | 文件 | 每特性改动量 |
|---|---|---|
| 后端（6） | `lib.rs` / `plugins.rs` / `cmd/mod.rs` / `build.rs` / `commands.json` / `Cargo.toml` | 各 1 行或 1 段 |
| ACL（2） | `capabilities/default.json` / `permissions/<name>/default.toml` | 1 行 / 新建 |
| 前端（3） | `config/features.json` / `features/registry.ts` / `vite-env.d.ts` | 1 条 / 1 段 / 1 行 |
| 本体（2） | `features/_probe/**` / `probe_plugin.rs` | 新建 |

P.4 的 CI 断言应断言「除上述 13 条路径外无改动」，并把 `settings/**`、`src-tauri/src/{ipc,setup,menu,window,lifecycle}.rs`、`features/{types,aggregate,nav}.ts` 列为禁区。`cmd/mod.rs` 的 conf applier 段已由 F2 移入各插件 `init()` 自注册，装配点降为 10（后端 5 + ACL 2 + 前端 3）。

P.4 已落地为 `src/features/__tests__/coreBoundary.test.ts`，口径修正为「足迹冻结」：`src/settings` + `src/features` + `src-tauri/src` 中（排除测试产物、特性本体、10 条装配点）出现特性名的 core 文件集合必须与 `FEATURE_FOOTPRINT_ALLOWLIST` 逐字相等。**新增特性无白名单条目 → 期望空集**，这才是「改动 core 文件数 = 0」的可执行形式；vault 作为历史存量保留 7 个白名单文件（`cmd/config.rs`、`cmd/mod.rs`、`ipc.rs`、`lifecycle.rs`、`protocol/dto/config.rs`、`protocol/events.rs`、`setup.rs`），它们不是新特性可复制的先例。同时补两条集合断言：装配点齐备（每特性 10 条 + 本体 2 处）与双向一致（`commands.json` 分组 / `*_plugin.rs` / `permissions/*` 三集合 = 声明特性集合，保留组 `core` 与 `hostOnly` 除外）。

### 10.2 演练暴露的缺口

| # | 缺口 | 现状 | 归属 |
|---|---|---|---|
| F1 | 前端 id 引用完整性无断言 | `navParent`→`sidebar.key`、`fields.group`→`confs.group`、`section.pageId`→`pages.id` 三组引用写错只在运行期静默失效；`assembly_contract.rs` 只覆盖后端 | **已完成**：`src/features/__tests__/featureModuleRefs.test.ts` 覆盖 `navParent`/`categoryId`/`pageId`/`section`/`group` 6 类跨槽位引用，含 7 类反例自检，3 例全绿（P.4 删探针后：vault 模块遍历 + 合成八槽位模块 + 反例） |
| F2 | `conf_side_effects()` 仍是 core 内唯一按特性增长的 cfg 点 | `cmd/mod.rs` 每特性 1 段 cfg 重绑定；多特性叠加时链式 `collect()` 触发 `E0282`，须显式标注 `Vec<(&'static str, ConfApplier)>` | **已完成**：`cmd/mod.rs` 改为 `CONF_APPLIERS` 注册表（core 三组首次消费时惰性播种）+ `register_conf_applier()`；`vault_plugin.rs` 在自身 `#[cfg(feature)] init()` 内自注册（原 `probe_plugin.rs` 同款实现，已于 P.4 删除），core 侧特性 cfg 归零 |
| F3 | 读写前缀不对称 | 读走 `plugin:probe\|get_probe_conf`，写走 core `patch_conf`，同一 group 两条前缀 | 记录项：`patch_conf` 是否插件化留待后续阶段决定 |
| F4 | 前端重复字面量 | `conf.ts` 与 `ipc.ts` 各写一份 `{ ipc: { namespace: "probe" } }` | 小清理：由 `ipc.ts` 导出 `probeIpc` 常量，其余引用它 |
| F5 | 运行时 ACL 无静态门禁 | A1–A6 只读源码文本，不校验 `capabilities/*.json` / `permissions/**` / `build.rs` 三方命令名求交；错位时静态全绿、运行期报 `Command ... not allowed`。**一次性冒烟已通过**（P.2），但下次改 ACL 仍无回归保护 | 门禁补强：P.4 已加「装配点存在 + 三集合相等」断言（`coreBoundary.test.ts`），**逐命令求交仍待补**（`build.rs` 的 `InlinedPlugin::commands` vs `commands.json` 分组 vs `permissions/<name>/default.toml` 的 `allow-<name>-<cmd>`）；冒烟只作一次性验证 |
| F6 | A2 的 cfg 断言写死了 vault | `cfg_line_count()` 只统计含 `vault` 的 `#[cfg]` 行、白名单也只为 vault 定义；probe 在 `cmd/mod.rs` 的 1 行 cfg 对 A2 不可见，任何新特性往 core 塞 cfg 都能静默通过 | **已完成**：A2 改 `cfg_feature_names()` + `cfg_location_allowed()`，按特性名分组计数（vault 行数上限 6 保留） |

F1 / F2 / F6 属本阶段应补齐项，F3 属记录项，F4 属小清理，F5 属门禁补强。
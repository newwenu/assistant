# I24 — 任务清单

> 规格：[I24-feature-extension-spec.md](./I24-feature-extension-spec.md)
> 顺序：E0 → E1 → E3 → E2 → E4 → E5 → E6 → E6a → E7 → E8 → 探针演练
> 勾选条件：代码已改 **且** 对应验收已跑

## E0 快照与索引

`src/settings/core/{types/snapshot.ts,snapshot.ts,query.ts}`、`fieldRegistry.ts`、`pageRegistry.ts`、`__tests__/settingsSnapshot.golden.test.ts`

- [x] E0.1 `SettingsSnapshot` 类型 + `version` / `phase`
- [x] E0.2 `buildSnapshot()` 惰性构建 + 双键失效
- [x] E0.3 `query.ts` 收敛 6 个查询函数，`getAllDefinitions()` 两次同引用
- [x] E0.4 内置容器 `bumpBuiltinVersion()`，三个 define 写入时递增
- [x] E0.5 黄金快照三 fixture：`full` / `vaultless` / `diff`
- [x] E0.6 `diff` 断言「排除面恰好等于 vault 贡献」
- [x] E0.7 `npm run test` 通过，行为零变化

## E1 依赖方向拉直

`fieldRegistry.ts`、`pageRegistry.ts`、`features/aggregate.ts`、`features/registry.ts`、`main.tsx`

- [x] E1.1 `fieldRegistry` 删除 `@/features/registry` 运行时 import
- [x] E1.2 `pageRegistry` 同上，`derive()` 改读快照
- [x] E1.3 `aggregate.ts` 仍 import `core/definePage`（features → core，方向合法）
- [x] E1.4 `loadFeatures()` 末尾 `bindExtensionInput()`
- [x] E1.5 `main.tsx` 顺序：`loadFeatures` → 绑定 → 渲染
- [x] E1.6 `src/settings/**`（不含 `__tests__`）grep 不到 `@/features`
- [x] E1.7 手测：启动 / 设置页 / vault 页
  - 双特性构建下 `feature-build.ps1 -Task dev` 启动通过：主窗口正常渲染、侧栏 vault / 探针条目就位、设置页可开、vault 页可进；全程无控制台报错

## E3 校验与引用完整性

`snapshot.ts`、`builtinRegistry.ts`、`insertUnique.ts`、`pageRegistry.ts`、`SettingsContent.tsx`、`aggregate.ts`、`__tests__/snapshotInvariants.test.ts`

- [x] E3.1 `validateSnapshot()` 返回 `Violation[]` 批量收集
- [x] E3.2 DEV 抛错 / 生产 log + 丢弃违规条目
- [x] E3.3 `insertUnique()` 统一两条通道（`core/insertUnique.ts`，内置容器与 `aggregate` 共用）
- [x] E3.4 页面缺失改未找到面板（守卫处去掉 `Navigate`，用 `Empty` 空态）
- [x] E3.5 `getSettingsPageManifest()` 缺失返回 undefined，不回落总览
- [x] E3.6 现可表达的不变式一次性全抛（12 条规则，`snapshotInvariants.test.ts`）
  - 已由 E2 完成：`field.group` 不在已注册槽位 → `unknown-conf-group`（同时补 `unknown-conf-page` / `conf-group-without-page` / `conf-page-without-fields`）
  - 推迟：`navParent` 未命中 / `sidebar.key` 悬空 → E5（这两个字段在 E5.4 才新增）
- [x] E3.7 手测：无 vault 构建下 `/settings/vault` 显示未找到面板
  - `feature-build.ps1 -Task dev -Without vault` 启动通过，经探针页「调试导航」直接跳 `/settings/vault`，命中 `SettingsContent` 的 `Empty` 兜底（`settings.not_found.title`），未出现白屏或报错

## E2 配置面通用化

新建 `types/confRegistry.ts`、`confSlot.ts`、`confRuntime.ts`、`useConfGroup.ts`、`features/vault/conf.ts`（`services/featureIpc.ts` 推迟至 E7，见 3.8.3）

- [x] E2.1 `ConfStore` 逐键收窄映射类型（`{ [G in ConfGroup]?: ConfOf<G> | null }`；`undefined` = 无此特性，`null` = 已注册未加载）
- [x] E2.2 `settingsStore` 加 `confs`；`isConfGroupAvailable()` 落在 `confRuntime`
  - 偏离：可用性由注册表回答，store 只存数据，不在 store 上再挂一份判定
  - 偏离：旧四槽一次性删除、不留代理（代理会让 E2.13 变成第二遍改动）
- [x] E2.3 `registerConfSlot()` / `getConfSlot()` / `getConfGroups()`
- [x] E2.4 `confRuntime.fetchConf()` / `getConfValue()`，删 `getVaultConf`
  - 语义：`fetchConf` 未注册返回 `null` 而不抛错，调用方以 `null` 守卫降级
- [x] E2.5 `VaultConf` 迁出 core，`declare module` 合并（Rust `VaultConf` struct 已带 `#[serde(default = ...)]`）
- [x] E2.6 `toConfRecord` 收敛为唯一转换边界
  - 偏离：签名为 `(value: unknown)` 而非 `<T extends object>`——参数类型不影响返回类型，`unknown` 直接覆盖 `null`/`undefined`
- [x] E2.7 两份 `useConfByGroup` 合并为 `useConfGroup`（`SettingsGroup` 内适配器已内联为 `toConfRecord(useConfGroup(group))`）
- [x] E2.8 `settingEvents` 改 `isConfGroupAvailable` 判定
- [x] E2.9 `commandAggregation.tsx` 删 `GROUP_TO_PAGE`，读 `confPageMap`
- [x] E2.10 `optimisticUpdate` 改查 `confs[group]`
- [x] E2.12 删 `CONF_KEY_MAP` / `CONF_LOAD_KEY_MAP`
  - 偏离：保留 `ConfOf<G>` 别名（它已是 `ConfTypeRegistry[G]` 索引而非条件类型，删除只是把同一表达式抄到 5 处）
- [x] E2.13 删旧四槽，`confs` 唯一入口
- [x] E2.15 快照补 3 条 conf 不变式：`unknown-conf-page` / `conf-group-without-page` / `conf-page-without-fields`
  - 生产降级为丢弃 `pageId`，`confPageMap` 因此不会收录悬空跳转目标
- [x] E2.11 `featureInvoke(module)` 收敛 `plugin:` 字面量 —— **已由 E7.8 交付**（规格 3.8.3 把它归入「特性自有命令命名空间」）
  - `src/services/featureIpc.ts` 的 `featureCommand()` 是生产侧唯一拼接点；`src/` 生产代码实测 1 处（`featureIpc.ts:8`），其余 5 处均在 `__tests__` 内（`vaultStore.test.ts` 的 mock 键、`vaultService.ssh.test.ts` 的命令清单、`probeIpc.test.ts` 的断言），属测试侧合法用途
- [x] E2.14 `npm run typecheck` + vault 配置读写/乐观更新/跨窗口同步/命令面板回归
  - 自动校验（typecheck / lint / test）全绿：`tsc --noEmit` 0 错、`eslint src --max-warnings=0` 0 错、`vitest run` 17 文件 238 例通过

## E4 构建开关单源

`config/features.json`（手写）、`vite.config.ts`、`scripts/feature-build.ps1`

- [x] E4.0 定单源：**手写 `config/features.json`**（D1 已定，规格 3.7 v2.5 决策）
- [x] ~~E4.1 生成 `src/features/generated/manifest.ts`~~ **取消**：清单手写、不生成中间产物，新增特性在 `features/registry.ts` 补守卫段
- [x] E4.2 `vite.config.ts` 的 `__VAULT_ENABLED__` 由清单派生（`define` 遍历清单，`process.env[env]` 优先于 `default`）
- [x] E4.3 一致性断言：清单 ↔ `Cargo.toml [features]` 双向（cargoFeature 已声明 / 无孤儿条目 / 默认集合相等）；`ipc.namespace` 一项推迟至 E7（`FeatureModule` 暂无 `ipc`）
- [x] ~~E4.4 `gen-features --check` 漂移即红~~ **取消**：无生成物即无漂移；改为 `feature-build.ps1` 启动即断言（已实测：`default` 改 `false` → 退出码 1，且在跑 npm 之前中止）
- [x] E4.5 单一构建入口：只有 `scripts/feature-build.ps1`，无 `.mjs` 并存
- [x] E4.6 无 vault 产物验收：`VAULT_ENABLED=0` 构建 → vault chunk = 0、`vaultAutoLockMinutes` 命中 = 0；默认构建 → vault chunk = 5（对照）
  - 未跑 `-Task build`（需完整 Rust 打包）；前端半边已实测，后端半边由既有 `cargo check --no-default-features` 覆盖

## E5 排序 / 插槽 / key 契约

`features/types.ts`、`features/aggregate.ts`、`settings/core/types/pageTypes.ts`、`components/AppSidebar.tsx`、`features/vault/module.ts`

- [x] E5.1 `order` 分命名空间，模块 `order` 不下传
  - `aggregate()` 移除 `mod.order` 基准；子项缺省取声明序号 `i`，仅显式 `order` 才覆盖
- [x] E5.2 插槽扩到 4 值
  - `SidebarSlot` = `after-tools` / `before-history` / `after-history` / `before-settings`；`AppSidebar` 按插槽分组拼装，`/history` 固定在第二、三插槽之间
- [x] E5.3 `sidebar.key` 必填 + 全局唯一
  - `FeatureSidebarEntry.key` 必填；`aggregate()` 用 `insertUnique` 拦重复；`navParent` 不再回落到 route path
  - 连带修复：菜单点击改为 `resolveSidebarTarget()`（key → 命中 `navParent` 的 route / settings page），不再把 key 当 path 直接 `navigate`
- [x] E5.4 新增 `navParent` / `visibility` / `emptyPage`
  - 三字段落在 `SettingsPageManifest`（必填）/ `SettingsPageDefinition`（可选，`definePage` 补缺省）
- [x] E5.5 `FeatureRoute` 补 `keywords`
  - `keywords` 经 `aggregate()` 透传到 `PreparedRoute`，命令面板参与匹配
- [x] E5.6 侧栏选中态按 `navParent` 推导
  - 新增 `features/nav.ts` `resolveSidebarKeys()`：route → `navParent`，settings page → `navParent`，失败回落 path
- [x] E5.7 `deepLinkOnly` 不进侧栏/命令面板，深链可达
  - 侧栏用 `getSettingsDetailPages()` 过滤；命令面板三处（页面/分区/字段）+ 特性路由统一跳过；`MainLayout` 仍注册全部路由
- [x] E5.8 打乱 routes 顺序后侧栏不变
  - `features/__tests__/nav.test.ts` 覆盖
- [x] E5.9 四插槽位置正确
  - `AppSidebar` 菜单装配顺序：工具 → `after-tools` → `before-history` → 历史 → `after-history` → `before-settings` → 设置
- [x] E5.10 两页共享一侧栏条目高亮正确
  - `nav.test.ts` 覆盖同 `navParent` 双页 → 同一 key
- [x] E5.11 快照 fixture 显式加入三个新字段
  - 三份 fixture 已重新生成：`pages` 带 `navParent`/`visibility`/`emptyPage`，`routes` 带 `navParent`/`visibility`/`keywords`
- 校验：`tsc --noEmit` 0 错误、`eslint src` 0 错误、`vitest` 230 通过（含 4 条新规则用例 + 7 条 nav 推导用例）

## E6 后端装配拆分（零行为变化）

新建 `src-tauri/src/{plugins.rs,ipc.rs,menu.rs,lifecycle.rs}`

- [x] E6.1 `plugins.rs` 接 6 个 `.plugin(...)`
  - `plugins::attach(builder)` 返回同一 `Builder`，`lib.rs` 只做链式调用
- [x] E6.2 `ipc.rs` 接 `handler()`（66 对 cfg 原样保留）
  - `handler()` 返回 `impl Fn(Invoke<Wry>) -> bool`，123 条注册条目整块搬迁，vault 条目 cfg 未动
  - 偏差：规格示例写 `handler<R: Runtime>()`，实测不成立——命令参数含 `AppHandle`（默认泛型为 `Wry`），泛型化报 112 处 `AppHandle: CommandArg<R>` 不满足；固定 `Wry` 后通过
- [x] E6.3 `menu.rs` 接 `on_menu_event`
  - `menu::on_menu_event(app, id)` 承接 `orb_new_chat` / `orb_open_main` / `orb_quit` 三分支
- [x] E6.4 `window.rs` 接 `on_window_event`
  - `window::on_window_event(window, event)` 承接主窗关窗进托盘、orb 失焦广播
- [x] E6.5 `lifecycle.rs` 接 `SHOULD_EXIT` / `.run(...)`
  - `lifecycle` 持静态原子量，对外暴露 `request_exit()` / `should_exit()` / `on_run_event()`；`lib.rs` 收口为 `.run(lifecycle::on_run_event)`
- [x] E6.6 `cmd/mod.rs` 加 `vault_cleanup_on_exit` 桩，`Exit` 双分支改无条件
  - 无 vault 时桩为空实现，`Exit` 分支不再带 cfg
- [x] E6.7 `setup.rs` / `cmd/app.rs` 改 `lifecycle::should_exit()` / `request_exit()`
  - 两处调用点已切换，`lib.rs` 不再导出退出标志
- [x] E6.8 `command_registry.rs` 改多块解析（剥注释 + 正则校验）
  - 扫描 `src/**/*.rs` 全部 `generate_handler![` 块，按方括号深度配对结束位置；先剥行/块注释，再剥 `#[...]` 属性，按逗号切分后以标识符正则校验条目
- [x] E6.9 `lib.rs` ≤ 40 行、无 `generate_handler!`
  - 33 行，注册块已整体外迁
- [x] E6.10 `-Task check` 开/关 vault 两种通过
  - 开 vault 6.16s / 无特性 3.33s，均 `Finished`
- [x] E6.11 `cargo test -p assistant-vault --test command_registry` 通过
  - 4 条用例通过（双向差集 + 唯一性 + 两条负向样例）
- [x] E6.12 `git diff` 只有行移动 + 可见性调整
  - E6 增量：`lib.rs` -265/+33，新增 `plugins.rs`/`ipc.rs`/`menu.rs`/`lifecycle.rs`，`window.rs` +25，`cmd/mod.rs` +3，`cmd/app.rs` 1 行替换，`setup.rs` 1 行替换
- 校验：`cargo fmt --check` 通过；`cargo check --all-targets` 与 `--no-default-features` 均通过；扫描测试 4/4

## E6a ACL spike

- [x] E6a.1 临时插件 1 条命令，前端 `plugin:spike|echo` 调通
  - 后端日志 `assistant_app::spike_plugin SPIKE_ECHO_INVOKED value=S1-OK`，前端收到 `S1-OK`，两处窗口（main/orb）均触发
- [x] E6a.2 路线 (a) 独立 crate + `permissions/default.toml` 实测
  - 未走：路线 (b) 已证明免新 crate 即可完整跑通 ACL，无需为单插件引入独立 crate
- [x] E6a.3 路线 (b) `build.rs` 内联 + 逐条 `allow-*` 实测
  - `tauri_build::try_build(Attributes::new().plugin("spike", InlinedPlugin::new().commands(&["echo"])))` 自动生成 `allow-echo`/`deny-echo` 到 `gen/schemas/acl-manifests.json`
  - 逐条 66 项不需要：`permissions/spike/default.toml` + capability 1 行 `spike:default` 即可
- [x] E6a.4 S1/S2/S3 结论落表并回写规格 3.8.3a
  - 见规格 3.8.3a「spike 实测结论」表
- [x] E6a.5 按结论确认/下调 G4、G5
  - G4 确认：后端边际成本 = 1 个插件文件 + 1 对 cfg + 1 个 `permissions/<plugin>/default.toml` + capability 1 行
  - G5 确认：ACL 面恒为 **1 行**（不随命令数增长），逐条 66 项路线作废
- [x] E6a.6 成本不可接受 → E7 走 Plan B 并在规格标注
  - 不触发：ACL 边际成本已降到 1 行，E7 按主路径（插件边界）执行
- [x] E6a.7 删临时文件，`cargo check` 恢复绿
  - 已删 `src/plugins.rs` 中的 `spike_plugin::init()` 调用、`src/spike_plugin.rs`、`permissions/spike/`、`build.rs` 的 `plugin(...)`、capability 的 `spike:default`、`main.tsx` 探针

### spike 实测结论（S1/S2/S3）

| # | 结论 | 证据 |
|---|---|---|
| S1 | **成立**：内联插件命令 `plugin:spike\|echo` 可被同窗口前端调用 | 运行期返回 `S1-OK`，后端 `SPIKE_ECHO_INVOKED` |
| S2 | **最小形态 = 2 个文件 1 行**：`permissions/<plugin>/default.toml` + capability 1 条 `<plugin>:default` | `acl-manifests.json` 中 `spike` 与 `autostart`/`dialog`/`log`/`opener` 形态一致 |
| S3 | 失败面分两段：**权限标识写错/插件未在 build.rs 声明 → 构建期**；**权限存在但 capability 未授 → 运行期** | 见下方三条实测 |

S3 三条实测：

1. capability 写 `spike:does-not-exist` → `cargo check` 失败（exit 101）：`failed to run tauri-build: Permission spike:does-not-exist not found expected one of ...`（**构建期**）
2. `build.rs` 不再声明 `spike` 插件、capability 仍写 `spike:default` → `cargo check` 失败（exit 101）：`Permission spike:default not found ...`（**构建期**）
3. 插件与权限都在、capability 未授（`log:default` 未声明）→ 构建通过，前端调用被拒：`log.log not allowed. Permissions associated with this command: log:allow-log, log:default`（**运行期**）

**关键坑（已踩）**：`permissions/<plugin>/default.toml` 必须用 `[default]` 表，不能写 `[[set]] identifier = "default"`。后者只落进 manifest 的 `permission_sets`，`default_permission` 为 `null`，capability 写 `<plugin>:default` 会**运行期被拒**（错误信息却仍把 `spike:default` 列为可用权限，极易误导）。官方插件全部是 `default_permission=default` + `permission_sets=[]` 的形态。

**对 E7 的硬约束**：`build.rs` 的 `plugin("vault", ...)` 声明与 `permissions/vault/default.toml` 必须**不受 feature 影响恒存在**，否则关闭 vault 特性时 capability 的 `vault:default` 会直接让构建期失败。可选性只体现在「运行时命令是否存在」，不体现在 ACL 声明。


## E7 后端 cfg 归零

新建 `src-tauri/src/vault_plugin.rs`、`src-tauri/commands.json`、`src-tauri/tests/assembly_contract.rs`

- [x] E7.1 `vault_plugin.rs` 全文 1 对 cfg，66 条命令无 cfg
- [x] E7.2 `.setup()` 接住 `vault_setup`
- [x] E7.3 `plugins.rs` 无条件注册
- [x] E7.4 `ipc.rs` 只留 core 57 条，cfg = 0
  - 实测：注册表 62 条（非 57），`#[cfg` 计数 0；与 vault 66 条合计 128 条，A5 下限 120 仍成立
- [x] E7.5 `setup.rs` 删 vault 调用，cfg 归零
- [x] E7.6 `cmd/mod.rs` 收敛重导出 + `conf_side_effects()`，删 `apply_vault_side_effects` 双桩
  - 注册表项类型 `ConfApplier = fn(&AppHandle, serde_json::Value) -> Result<(), String>`，承载「load → amend → validate → save → 副作用」整段；`patch_conf` 只做查表，`cmd/config.rs` 内 group 字面量 0 次
  - 弱签名刻意保留（强签名无法放进同一 `Vec`，会让注册表退化为逐 group wrapper）；`apply_app_conf`/`apply_orb_conf`/`apply_mascot_conf` 留 core 且内部仍强类型，`vault::apply_conf` 留 `cmd/vault/mod.rs`
  - `apply_vault_side_effects` 双桩已删；`pub use vault::*` 已取消，`vault_plugin.rs` 改 `crate::cmd::vault::<cmd>` 直接引用
  - 全后端 vault 相关 `#[cfg]` 实测 6 行（`cmd/mod.rs` 4 + `vault_plugin.rs` 2），双特性构建均 `cargo check` 通过、无新增 warning
- [x] E7.7 落 capability / `permissions/`
  - `permissions/vault/default.toml` 用 `[default]` 表（66 条 `allow-*`），`capabilities/default.json` 加 `vault:default`；`acl-manifests.json` 实测 `default_permission=default`、`permissions_count=66`、`permission_sets=[]`
- [x] E7.8 `vault.service.ts` 首行改 `featureInvoke(vaultModule)`，70 处调用点零改动
  - `src/services/featureIpc.ts` 落地：`featureInvoke(module)` 按 `ipc.namespace` 拼 `plugin:${ns}|${cmd}`，无 `ipc` 时退回裸命令名
  - `FeatureModule.ipc?: FeatureIpc`（`namespace: string`）已加；`vaultModule` 声明 `ipc: { namespace: "vault" }`
  - `vault.service.ts` 改为 `const invoke = featureInvoke(vaultModule)`，70 处调用点零改动；`src/` 内 `plugin:` 字面量实测仅剩 `featureIpc.ts` 1 处
  - 测试侧 `vaultStore.test.ts` 的 `mockCmd` 键改为 `plugin:vault|<cmd>`（mock 挂在 `tauriInvoke` 层，仍覆盖前缀拼接）；`tsc --noEmit`、`eslint`、`vitest run`（230/230）全绿
- [x] E7.9 `commands.json` 落盘（57 + 66）
  - 实测落盘 core 62 + vault 66 = 128 条（core 按 `ipc.rs` 实际条数，非文档草图的 57）；`groups.hostOnly` 保留空数组，供 A6 三态分类使用
  - 清单是 A4 的 `declared` 侧唯一事实源，与注册表变更同提交更新
- [x] E7.10 `assembly_contract.rs` 六条断言（A1–A6）
  - A1 `ipc.rs` 无 `#[cfg]`；A2 全后端 vault cfg 仅 `cmd/mod.rs`(4) + `vault_plugin.rs`(2)，合计 6 ≤ 6；A3 `lib.rs` 25 行 ≤ 40 且不含 `generate_handler!`；A4 双向差集为空；A5 解析器解析 128 条 ≥ 120；A6 三态恰好命中 + `vault_cleanup_on_exit` 桩签名归一化一致
- [x] E7.11 G1 ≤ 8 / G2 = 0 / G3 ≤ 40 / G4 = 1 对 cfg + 1 组 ACL
  - 实测 G1 = 6（已达进阶目标 ≤6，`cmd/mod.rs` 4 + `vault_plugin.rs` 2）；G2 = 0；G3 = 25 行；G4 = `vault_plugin.rs` 1 对 cfg + `permissions/vault/default.toml` 1 组 ACL（66 条 `allow-*`，capability 仅 1 行 `vault:default`）
- [x] E7.12 前后端命令名双向差集为空
  - 新增 `src/services/__tests__/commandNames.test.ts`：扫描 `src/**` 的 `invoke("...")` 字面量，与 `commands.json` 做双向差集；另断言 vault 前端命令与 `groups.vault` 双向一致、清单无重复
- [x] E7.13 全链路回归：密码 / TOTP / 同步 / SSH / 生物识别
  - 密码读写 / TOTP / 同步 / 生物识别由既有 `src/vault/core/__tests__/vaultStore.test.ts`（40 例）覆盖（原与源同目录，已按 `__tests__/` 约定归位）；SSH 无 store 层用例，补 `src/vault/core/__tests__/vaultService.ssh.test.ts`（3 例）验证 7 个 SSH 命令全部走 `plugin:vault|` 且参数按 camelCase 透传
  - 全量：`tsc --noEmit` 0 错、`eslint` 0 错、`vitest run` 236/236、`cargo test`（`assembly_contract` 6 + `protocol_contract` 9）全绿

## E8 资产 colocate

- [x] E8.1 `settings/pages/VaultSettingsPage.tsx` 迁入 `features/vault/settings/`
  - 相对导入全部改 `@/settings/**` 绝对别名（`SettingsGroup` / `SettingsSection` / `SettingsPageShell` / `fieldRegistry` / `useConfPatchHandlers`）；`module.ts` 的 `load` 改 `import("./settings/VaultSettingsPage")`
- [x] E8.2 `settings/shared/renderers/BiometricSwitchRender.tsx` 同上
  - `FieldRenderProps` 改 `@/settings/core/types/fieldSchema`、`useSettingMessage` 改 `@/settings/shared/useSettingMessage`；`module.ts` 的 `customRenderLoad` 改 `import("./settings/BiometricSwitchRender")`
- [x] E8.3 `src/settings/**` grep 不到 `@/vault`
  - 实测 0 命中；`src/settings/**` 现仅剩 `@/features/vault/module`（快照黄金测试需要聚合模块，属测试侧合法依赖，不属 `@/vault`）
- [x] E8.4 vault 设置页功能回归
  - `tsc --noEmit` 0 错、`eslint src` 0 错（迁移后 3 处 prettier 尾行已 `--fix`）、`vitest run` 236/236；`features/vault/settings/` 下两文件与源文件内容等价，仅导入路径变化，行为零改动

## 探针演练（收尾硬门槛）

- [x] P.1 `features/_probe/**`：1 route + 1 sidebar + 1 category + 1 page + 1 section + 2 field + 1 conf 槽位 + `ipc.namespace = "probe"`
  - 实测落地 6 文件：`module.ts` / `conf.ts` / `ipc.ts` / `ProbePage.tsx` / `settings/ProbeSettingsPage.tsx` / `__tests__/probeIpc.test.ts`；八类槽位全覆盖，`probeConfSlot` 与 `probeEcho` 均由 `featureCommand` / `featureInvoke` 拼前缀，探针内无 `plugin:` 字面量
- [x] P.2 `probe_plugin.rs` 1 条 mock 命令，经 `featureInvoke` 调通
  - 实测注册 2 条（`probe_echo` / `get_probe_conf`，后者供 conf 槽位复用）；静态链路由 `probeIpc.test.ts` 断言全部命中 `plugin:probe|`
  - **运行时 ACL 冒烟通过**：侧栏「探针」条目出现、设置「探针设置」页可开、两字段正常渲染（打开页面即触发 `plugin:probe|get_probe_conf`，等于一次 ACL 校验）、ProbePage 点按钮返回 `probe:ping`；全程无 `not allowed` 报错
- [x] P.3 白名单核对：除 11 个装配点文件 + 2 处本体新建（后端 6：`lib.rs` / `plugins.rs` / `cmd/mod.rs` / `build.rs` / `commands.json` / `Cargo.toml`；ACL 2：`capabilities/default.json` / `permissions/probe/default.toml`；前端 3：`config/features.json` / `features/registry.ts` / `vite-env.d.ts`；本体 2：`_probe/**` / `probe_plugin.rs`）外零改动；`settings/**` 与后端 `{ipc,setup,menu,window,lifecycle}.rs` 必须零改动（见 spec §10.1）
  - 禁区实测零命中：`src/settings/**`、`src/features/{types,aggregate,nav}.ts`、`src-tauri/src/{ipc,setup,menu,window,lifecycle}.rs` 全部 CLEAN
  - 后端 `#[cfg(feature)]` 分布实测（F2 后）：`probe_plugin.rs` 10（含 1 行 `not` 分支）、`vault_plugin.rs` 2、`cmd/mod.rs` 3（vault 3 + probe 0），其余文件 0
- [x] P.4 删 `_probe`，「新增特性改动 core 文件数 = 0」写成 CI 断言
  - 口径已修订为「core 逻辑零改动」而非「文件零改动」：断言校验「除 10 条装配点 + 2 处本体外无改动」，并显式声明禁区目录；F2 已完成（`cmd/mod.rs` 的 conf applier cfg 已归零），装配点降为 10（后端 5 + ACL 2 + 前端 3）
  - 删除清单已执行（删除前备份至 `temp-backup/probe-removed/`）：`src/features/_probe/**`（含 `ProbePage.tsx` 的「调试导航」面板，该面板为 E3.7 手测临时提供，用于绕过侧栏直接跳 `/settings/vault` 等未注册路由）、`src-tauri/src/probe_plugin.rs`、`src-tauri/permissions/probe/`（含删文件后残留的空目录）、`commands.json` 的 `groups.probe`、`capabilities/default.json` 的 `probe:default`、`Cargo.toml` 的 `probe` 特性与 default 集合条目、`config/features.json` 与 `vite-env.d.ts` 的 `probe` 条目、`registry.ts` 的 `__PROBE_ENABLED__` 分支、`lib.rs` / `plugins.rs` / `build.rs` 的探针装配行
  - 前端测试收敛：`featureModuleRefs.test.ts` 改为仅遍历 `vaultModule`；原「探针覆盖八类槽位」改为合成模块 `FULL_SLOT_MODULE`（八类槽位齐全且自身引用自洽，同样过 `checkModule()`），「故意写错的引用会被逐条报出」反例用例保留
  - 新增 CI 断言 `src/features/__tests__/coreBoundary.test.ts`（3 条，全绿）：
    - **装配点齐备**：每个声明特性必须在前端 3 处（`config/features.json` 的 `<NAME>_ENABLED`、`vite-env.d.ts` 声明、`registry.ts` 消费）+ 后端 5 处（`Cargo.toml [features]` 与 default 集合、`build.rs` 内联插件、`commands.json groups.<name>`、`lib.rs` 的 `mod <name>_plugin`、`plugins.rs` 的 `<name>_plugin::init()`）+ ACL 2 处（`capabilities/default.json` 的 `<name>:default`、`permissions/<name>/default.toml`）齐备；本体侧校验 `src/features/<name>/module.ts` 存在、`src-tauri/src/<name>_plugin.rs` 用同名 `#[cfg]` 包住自身
    - **双向一致**：`commands.json` 分组（排除保留组 `core` / `hostOnly`）、`src-tauri/src/*_plugin.rs`、`src-tauri/permissions/*` 三个集合与 `config/features.json` 声明集合完全相等，覆盖「删了本体忘拆装配点」与「残留空目录」两类回归
    - **足迹冻结**：扫描 `src/settings` + `src/features` + `src-tauri/src`（排除测试产物、特性本体、10 条装配点），特性名出现的 core 文件集合须与白名单逐字相等；新增特性无白名单条目 → 期望 `[]`，即「改动 core 文件数 = 0」
  - vault 白名单实测 7 个 core 文件（历史存量，新特性不可触碰）：`cmd/config.rs`（`get_vault_conf` 核心命令）、`cmd/mod.rs`（`vault_cleanup_on_exit` 三态桩）、`ipc.rs`、`lifecycle.rs`、`protocol/dto/config.rs`、`protocol/events.rs`、`setup.rs`；新特性白名单为空
  - 全量校验：`npm run typecheck` / `npm run lint` / `npx vitest run`（18 files / 242 tests）/ `cargo test`（lib 15 + assembly_contract 6 + protocol_contract 9）全绿
  - 遗留：§10.2 的 F5（`build.rs` / `commands.json` / `permissions` 三方命令名求交）仍待补，本次断言只覆盖「装配点存在 + 集合相等」，不覆盖逐命令 ACL 对齐

## 待决项

| # | 待决 | 影响 |
|---|---|---|
| D1 | 单源归属：Cargo `[features]` vs `config/features.json` | **已定：手写 `config/features.json`，不生成 manifest**（E4 已完成） |
| D2 | 插件命令 ACL：`default` 权限集 vs 逐条 66 项 | E6a → E7 |
| D3 | i18n 文案面扩展 | 本阶段不做 |
| D4 | `visibility` 是否加第三值 `nav` | E5，当前无用例，不加 |

## 提交切分

1. `refactor(I24): E0 快照与索引`
2. `refactor(I24): E1 依赖方向拉直`
3. `feat(I24): E3 校验与引用完整性`
4. `refactor(I24): E2 配置面通用化`
5. `build(I24): E4 构建开关单源`
6. `feat(I24): E5 排序/插槽/key 契约`
7. `refactor(I24): E6 后端装配拆分`
8. `chore(I24): E6a ACL spike`
9. `refactor(I24): E7 后端 cfg 归零`
10. `refactor(I24): E8 资产 colocate`
11. `test(I24): P 探针演练（P.1 骨架 + 契约断言泛化）`
12. `chore(I24): P.4 删除探针 + core 足迹冻结断言`

---

## 探针演练执行记录（P.1）

### 槽位覆盖

| 槽位 | 落点 | 关键字段 |
|---|---|---|
| route | `src/features/_probe/module.ts` `routes[0]` | `path: "/probe"`、`navParent: "probe"`、`boundary`、`keywords`、`load` 动态 import |
| sidebar | 同文件 `sidebar[0]` | `key: "probe"`、`slot: "after-tools"`、`icon` |
| category | 同文件 `categories[0]` | `id: "probe"`、`order: 21` |
| page | 同文件 `settings.pages[0]` | `id: "probe"`、`categoryId: "probe"`、`load` 动态 import |
| section | 同文件 `PROBE_SECTIONS` | `id: "probe-general"`、`pageId: "probe"` |
| field | 同文件 `PROBE_FIELDS` | `probeFlag`（boolean）/ `probeText`（string），`group: "probe"`、`section: "probe-general"` |
| conf | `src/features/_probe/conf.ts` | `probeConfSlot`：`group` / `invoke` / `pageId`；`ProbeConf` 类型 + 声明合并注册 `ConfTypeRegistry` |
| ipc | `src/features/_probe/ipc.ts` | `featureInvoke({ ipc: { namespace: "probe" } })`，命令名由 `featureCommand` 自动拼 `plugin:probe\|` |

### 装配点改动清单（实测）

| 文件 | 改动量 | 性质 |
|---|---|---|
| `src-tauri/src/probe_plugin.rs` | 新建（约 100 行） | 特性边界文件，`#[cfg(feature = "probe")]` 全部收敛于此 |
| `src-tauri/src/lib.rs` | +1 行 `mod probe_plugin;` | 模块声明 |
| `src-tauri/src/plugins.rs` | +1 行 `.plugin(crate::probe_plugin::init())` | 插件注册 |
| `src-tauri/src/cmd/mod.rs` | 0（F2 后无需改动） | conf applier 改由插件 `init()` 自注册 |
| `src-tauri/build.rs` | +1 段 `InlinedPlugin` | 构建期命令声明 |
| `src-tauri/commands.json` | +1 组 `groups.probe` | A4 `declared` 侧事实源 |
| `src-tauri/Cargo.toml` | +1 行特性 + default 集合 | cargo 开关 |
| `src-tauri/capabilities/default.json` | +1 行 `"probe:default"` | ACL 授权 |
| `src-tauri/permissions/probe/default.toml` | 新建 | 内联插件权限集 |
| `config/features.json` | +1 条（`default` / `cargoFeature` / `env`） | 前后端开关单源 |
| `src/features/registry.ts` | +4 行守卫 + 动态 import | 特性装配点 |
| `src/vite-env.d.ts` | +1 行全局声明 | vite define 类型 |
| `src/services/featureIpc.ts` | +`featureCommand` 函数 | 通用工具，`plugin:` 字面量仍仅此一处 |
| `src/features/_probe/**` | 新建 6 文件 | 探针本体 |

`src/settings/**`、`src/features/{types,aggregate,nav}.ts`、`src-tauri/src/{ipc,setup,menu,window,lifecycle}.rs` 均零改动——P.3 核对时按此表逐项比对。

### 校验结果

| 检查 | 结果 |
|---|---|
| `tsc --noEmit` | 0 错 |
| `eslint src --max-warnings=0` | 0 错（`ProbePage` 的 `Space direction` 已按弃用提示改 `orientation`） |
| `vitest run` | 17 文件 / 238 例全通过（含 `probeIpc.test.ts`） |
| `cargo test` | 全通过；`assembly_contract` A1–A6 六条全绿 |
| `feature-build.ps1 -Task check -Without probe` | 通过，`#[cfg(not(feature = "probe"))] init()` 空插件分支生效 |

### 演练暴露的缺口

F1–F5 见 `docs/store/I24-feature-extension-spec.md` §10.2。其中 F2 已在本次演练中实际命中：`cmd/mod.rs` 两段 cfg 链式 `collect()` 在多特性叠加时触发 `E0282`（类型推断失效），补显式 `Vec<(&'static str, ConfApplier)>` 标注后通过；A4/A5/A6 的注册表扫描也已从硬编码 `vault_plugin.rs` 泛化为「扫描全部 `*_plugin.rs`」，probe 的两条命令自动纳入断言——这一步是探针的核心收益：**它逼着断言从「为 vault 写死」变成「对任意特性成立」**。

---

## 阶段收尾：文档善后（v2.5，仅文档）

本轮不改代码，只做规格文档与审查记录的口径对齐，依据为 `docs/store/I24-execution-code-review.md` §6。执行结果登记在该文档 **§8「善后清单」**：文档项 13 条全部 `[x]`，代码项 C1–C7 保持 `[ ]` 待排期。

主要修正：

- 规格文档首部补「实施状态」表，消除「E0–E8 未实施」与 §10「P.4 已完成」的时间错位；
- 可计数口径回写为 core 62 / vault 66 / 合计 128，A5 下限 120（余量 8），A2 改按特性名分组，A3 注明实测 25 行与 15 行松弛；
- §10.1 装配点数量三处互斥（4–5 / 11 / 10）统一为 **10 个装配点 + 2 处本体**；
- 新增 §11「安全取舍登记」（ACL 全量 × 三窗口、4 份命令副本、后端注册表无重复检测、`plugin:` 无 lint）与 §12「已知残留与技术债」（T1–T6）；
- 6 处「文档有、代码无」逐条标 `TODO(未实现)`；§6.3 标注为历史施工图，避免重新引入已删除的 `_probe`。

---

## 阶段收尾：配置收敛策略（v2.6，仅文档）

本轮仍不改代码，只把「4 份命令清单副本」的处置从「补三方求交断言」改为**配置收敛**。设计依据见 `docs/store/I24-feature-extension-spec.md` §11.2，执行步骤见 `docs/store/I24-execution-code-review.md` §9。

主要变更：

- 判定 4 处副本各自的必然性：`vault_plugin.rs`（宏硬约束，不可消除）、`build.rs`（构建期可派生）、`permissions/<name>/default.toml`（内容纯冗余，可生成）、`commands.json`（本项目自建测试夹具，可删）；
- 策略改为两步：**S1 止血**（逐命令求交断言 + 派生规则常量化 + 解析器下限自检，0.5–1 天）、**S2 治本**（收敛为「1 份手写真源 + 2 份生成物 + 0 份冗余」，1–2 天，须在 S1 之后）；
- 审查记录 C1 拆为 `C1a`（S1）与 `C1b`（S2），两者状态均保持 `[ ]`；
- 规格 §10.2 F5、§12 T4 的处置口径同步改为「配置收敛」；
- 明确 S2 开工前必须确认的两点：生成物落盘位置（`OUT_DIR` 还是入库）、派生逻辑是否按多特性通用化。
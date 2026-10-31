# I24 — 任务清单

> 规格：[I24-feature-extension-spec.md](./I24-feature-extension-spec.md)
> 顺序：E0 → E1 → E3 → E2 → E4 → E5 → E6 → E6a → E7 → E8 → 探针演练
> 勾选条件：代码已改 **且** 对应验收已跑

## E0 快照与索引

`src/settings/core/{types/snapshot.ts,snapshot.ts,query.ts}`、`fieldRegistry.ts`、`pageRegistry.ts`、`__tests__/settingsSnapshot.golden.test.ts`

- [ ] E0.1 `SettingsSnapshot` 类型 + `version` / `phase`
- [ ] E0.2 `buildSnapshot()` 惰性构建 + 双键失效
- [ ] E0.3 `query.ts` 收敛 6 个查询函数，`getAllDefinitions()` 两次同引用
- [ ] E0.4 内置容器 `bumpBuiltinVersion()`，三个 define 写入时递增
- [ ] E0.5 黄金快照三 fixture：`full` / `vaultless` / `diff`
- [ ] E0.6 `diff` 断言「排除面恰好等于 vault 贡献」
- [ ] E0.7 `npm run test` 通过，行为零变化

## E1 依赖方向拉直

`fieldRegistry.ts`、`pageRegistry.ts`、`features/aggregate.ts`、`features/registry.ts`、`main.tsx`

- [ ] E1.1 `fieldRegistry` 删除 `@/features/registry` 运行时 import
- [ ] E1.2 `pageRegistry` 同上，`derive()` 改读快照
- [ ] E1.3 `aggregate.ts` 不再 import `settings/core/definePage`
- [ ] E1.4 `loadFeatures()` 末尾 `bindExtensionInput()`
- [ ] E1.5 `main.tsx` 顺序：`loadFeatures` → 绑定 → 渲染
- [ ] E1.6 `src/settings/**` grep 不到 `@/features`
- [ ] E1.7 手测：启动 / 设置页 / vault 页

## E3 校验与引用完整性

`snapshot.ts`、`builtinRegistry.ts`、`pageRegistry.ts`、`SettingsContent.tsx`、`__tests__/snapshotInvariants.test.ts`

- [ ] E3.1 `validate()` 返回 `Violation[]` 批量收集
- [ ] E3.2 DEV 抛错 / 生产 log + 丢弃
- [ ] E3.3 `insertUnique()` 统一两条通道
- [ ] E3.4 页面缺失改 404（守卫处去掉 `Navigate`）
- [ ] E3.5 `getSettingsPageManifest()` 缺失返回 undefined，不回落总览
- [ ] E3.6 5 类错误声明一次性全抛
- [ ] E3.7 手测：无 vault 构建下 `/settings/vault` 显示 404

## E2 配置面通用化

新建 `types/confRegistry.ts`、`confSlot.ts`、`confRuntime.ts`、`useConfGroup.ts`、`services/featureIpc.ts`、`features/vault/conf.ts`

- [ ] E2.1 `ConfStore` 逐键收窄映射类型
- [ ] E2.2 `settingsStore` 加 `confs` + `isConfGroupAvailable()`，旧四槽先留代理
- [ ] E2.3 `registerConfSlot()` / `getConfSlot()` / `getConfGroups()`
- [ ] E2.4 `confRuntime.fetchConf()` / `getConfValue()`，删 `getVaultConf`
- [ ] E2.5 `VaultConf` 迁出 core，`declare module` 合并（同步 Rust struct + `#[serde(default)]`）
- [ ] E2.6 `toConfRecord` 泛化为 `<T extends object>`
- [ ] E2.7 两份 `useConfByGroup` 合并为 `useConfGroup`
- [ ] E2.8 `settingEvents` 改 `isConfGroupAvailable` 判定
- [ ] E2.9 `components/commandPalette/commandAggregation.tsx` 删 `GROUP_TO_PAGE`，读 `confPageMap`
- [ ] E2.10 `optimisticUpdate` 改查槽位
- [ ] E2.11 `featureInvoke(module)` 收敛 `plugin:` 字面量
- [ ] E2.12 删 `CONF_KEY_MAP` / `CONF_LOAD_KEY_MAP` / `ConfOf`
- [ ] E2.13 删旧四槽，`confs` 唯一入口
- [ ] E2.14 `npm run typecheck` + vault 配置读写/乐观更新/跨窗口同步/命令面板回归

## E4 构建开关单源

- [ ] E4.0 **阻塞**：定单源归属（Cargo `[features]` 或 `config/features.json`），见待决 D1
- [ ] E4.1 生成 `src/features/generated/manifest.ts`
- [ ] E4.2 `vite.config.ts` 的 `__VAULT_ENABLED__` 改由清单生成
- [ ] E4.3 一致性断言：清单 ↔ cargo ↔ `ipc.namespace` ↔ env
- [ ] E4.4 `gen-features --check` 漂移即红
- [ ] E4.5 单一构建入口（不留 `.ps1` + `.mjs` 双入口）
- [ ] E4.6 `scripts/feature-build.ps1 -Task build -Without vault`：vault chunk 0、产物无 `vaultAutoLockMinutes`

## E5 排序 / 插槽 / key 契约

`features/types.ts`、`features/aggregate.ts`、`settings/core/types/pageTypes.ts`、`components/AppSidebar.tsx`、`features/vault/module.ts`

- [ ] E5.1 `order` 分命名空间，模块 `order` 不下传
- [ ] E5.2 插槽扩到 4 值
- [ ] E5.3 `sidebar.key` 必填 + 全局唯一
- [ ] E5.4 新增 `navParent` / `visibility` / `emptyPage`
- [ ] E5.5 `FeatureRoute` 补 `keywords`
- [ ] E5.6 侧栏选中态按 `navParent` 推导
- [ ] E5.7 `deepLinkOnly` 不进侧栏/命令面板，深链可达
- [ ] E5.8 打乱 routes 顺序后侧栏不变
- [ ] E5.9 四插槽位置正确
- [ ] E5.10 两页共享一侧栏条目高亮正确
- [ ] E5.11 快照 fixture 显式加入三个新字段

## E6 后端装配拆分（零行为变化）

新建 `src-tauri/src/{plugins.rs,ipc.rs,menu.rs,lifecycle.rs}`

- [ ] E6.1 `plugins.rs` 接 6 个 `.plugin(...)`
- [ ] E6.2 `ipc.rs` 接 `handler()`（66 对 cfg 原样保留）
- [ ] E6.3 `menu.rs` 接 `on_menu_event`
- [ ] E6.4 `window.rs` 接 `on_window_event`
- [ ] E6.5 `lifecycle.rs` 接 `SHOULD_EXIT` / `.run(...)`
- [ ] E6.6 `cmd/mod.rs` 加 `vault_cleanup_on_exit` 桩，`Exit` 双分支改无条件
- [ ] E6.7 `setup.rs` / `cmd/app.rs` 改 `lifecycle::should_exit()` / `request_exit()`
- [ ] E6.8 `command_registry.rs` 改多块解析（剥注释 + 正则校验）
- [ ] E6.9 `lib.rs` ≤ 40 行、无 `generate_handler!`
- [ ] E6.10 `-Task check` 开/关 vault 两种通过
- [ ] E6.11 `cargo test -p assistant-vault --test command_registry` 通过
- [ ] E6.12 `git diff` 只有行移动 + 可见性调整

## E6a ACL spike

- [ ] E6a.1 临时插件 1 条命令，前端 `plugin:spike|echo` 调通
- [ ] E6a.2 路线 (a) 独立 crate + `permissions/default.toml` 实测
- [ ] E6a.3 路线 (b) `build.rs` 内联 + 逐条 `allow-*` 实测
- [ ] E6a.4 S1/S2/S3 结论落表并回写规格 3.8.3a
- [ ] E6a.5 按结论确认/下调 G4、G5
- [ ] E6a.6 成本不可接受 → E7 走 Plan B 并在规格标注
- [ ] E6a.7 删临时文件，`cargo check` 恢复绿

## E7 后端 cfg 归零

新建 `src-tauri/src/vault_plugin.rs`、`src-tauri/commands.json`、`src-tauri/tests/assembly_contract.rs`

- [ ] E7.1 `vault_plugin.rs` 全文 1 对 cfg，66 条命令无 cfg
- [ ] E7.2 `.setup()` 接住 `vault_setup`
- [ ] E7.3 `plugins.rs` 无条件注册
- [ ] E7.4 `ipc.rs` 只留 core 57 条，cfg = 0
- [ ] E7.5 `setup.rs` 删 vault 调用，cfg 归零
- [ ] E7.6 `cmd/mod.rs` 收敛重导出 + `conf_side_effects()`，删 `apply_vault_side_effects` 双桩
- [ ] E7.7 落 capability / `permissions/`
- [ ] E7.8 `vault.service.ts` 首行改 `featureInvoke(vaultModule)`，70 处调用点零改动
- [ ] E7.9 `commands.json` 落盘（57 + 66）
- [ ] E7.10 `assembly_contract.rs` 六条断言（A1–A6）
- [ ] E7.11 G1 ≤ 8 / G2 = 0 / G3 ≤ 40 / G4 = 1 对 cfg + 1 组 ACL
- [ ] E7.12 前后端命令名双向差集为空
- [ ] E7.13 全链路回归：密码 / TOTP / 同步 / SSH / 生物识别

## E8 资产 colocate

- [ ] E8.1 `settings/pages/VaultSettingsPage.tsx` 迁入 `features/vault/settings/`
- [ ] E8.2 `settings/shared/renderers/BiometricSwitchRender.tsx` 同上
- [ ] E8.3 `src/settings/**` grep 不到 `@/vault`
- [ ] E8.4 vault 设置页功能回归

## 探针演练（收尾硬门槛）

- [ ] P.1 `features/_probe/**`：1 route + 1 sidebar + 1 category + 1 page + 1 section + 2 field + 1 conf 槽位 + `ipc.namespace = "probe"`
- [ ] P.2 `probe_plugin.rs` 1 条 mock 命令，经 `featureInvoke` 调通
- [ ] P.3 白名单核对：除 `_probe/**`、`probe_plugin.rs`、`plugins.rs` 一行、清单、capability 一行外零改动
- [ ] P.4 删 `_probe`，「新增特性改动 core 文件数 = 0」写成 CI 断言

## 待决项

| # | 待决 | 影响 |
|---|---|---|
| D1 | 单源归属：Cargo `[features]` vs `config/features.json` | 阻塞 E4 |
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
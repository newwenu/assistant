# I75 — 启动链路分层任务清单

| 项 | 值 |
|---|---|
| 状态 | 唯一任务级进度表；只记「改哪里、改没改」，语义一律见规格 |
| 日期 | 2026-11-25 |
| 规格 | [I75-startup-layering-spec.md](./I75-startup-layering-spec.md)（唯一设计来源，本文不复述） |

> 每项末尾括号是规格锚点，用于回查，不在本文展开。`N*` → §2，`J*` → §0，`S*` → §4.1，`C*` → §4.4，`F*` → §6.1，`M*` → §7.2，`G*` → §8，`I*` → §11，`A*` → §12，`R*` → §14，`P*` → §12.4。
> 勾选 = 代码已改且对应验收已跑。
> 阶段门禁子集（§12.4）：P0 只需 G2；P1 需 G1 / G3 / G7；P2 需 G4 / G6；P3 需 G5。
> 所有 `cargo` 命令在 `src-tauri` 目录执行（不带 `--manifest-path`）；`npm` / `npx` 在仓库根执行。

---

## P0 — 度量脚手架（§7 / §12.4）

- [x] `src-tauri/src/main.rs`：`let t0 = Instant::now()` 与 `t0_epoch_ms`（`SystemTime` → epoch 毫秒）在同一行采集，生成 `startup_id`（§4.2 / §7.1 / M1）
- [x] `src-tauri/src/boot/ctx.rs`：`BootClock`（`startup_id` / `t0` / `t0_epoch_ms` / `last`）、`StartupReport`（`phases` 去重集合 + `wake_starts`）；`BootClock` 与 `StartupReport` 各 `manage` 一次（§4.2）
- [x] 阶段日志：单阶段行 `startup stage=<id> lane=<…> startup_id=<uuid> ms=<n> outcome=<ok|degraded|fatal|ignored>`（`debug`，dev 档）；失败原因由 `Registry::build` 记一次，阶段行不重复（§7.1 / §7.1.1）
- [x] `src-tauri/src/cmd/startup.rs`：`report_first_frame`（写首帧记录）；idle pass 的 `OnceLock` 触发随 P1 的 `run_idle` 接入（P0 无 idle 泳道可触发）
- [x] `src-tauri/src/ipc.rs` 的 `generate_handler!` 增该命令；`assistant-protocol/src/lib.rs` 增 `pub mod startup;`；`startup.rs` 定义 `CapStateDto` / `CapabilityStateDto`（N16 / §10.1）
- [x] 跑 `npm run contract:gen` 并提交 `protocol.contract.ts` 变更（§4.4 / I10）
- [x] `initialization_script` 注入**不在 P0**：P0 只产出 `startup_id` 与阶段日志；`window.__STARTUP_ID__` 注入 + 前端 `requestAnimationFrame` 回传随 P2 建窗路径改造同批交付（§6.4 / J9）
- [x] `src-tauri/tests/startup_contract.rs` 骨架：读 `plugins.rs` 源码断言 `single_instance` 早于 `log_plugin()`（G2 / N3）
- [x] 先单独跑 `cargo test --test assembly_contract` 记录基线，确认目录化前的绿色状态（R5）

> 验收：`cargo test --test startup_contract`（G2）；`npm run typecheck`。阶段级耗时只在 dev 档 `debug` 可见，不设预算列、基线文件与预算门禁（J5）。

---

## P1 — 目录化 + 单一来源 + 前端减载（§5 / §4.5 / §6.1 / §8.1）

### P1a 目录化与资源表

- [x] 新建 `src-tauri/src/boot/` 16 个文件（`mod` / `resources` / `registry` / `runner` / `ctx` / `conf` / `access` + `build/{dirs,storage,attachments,state,tools,mcp,windows,shell,maintenance}`）（§5）
- [x] `boot/mod.rs` 只做导出，≤60 行、无业务分支（§5 / A2）
- [x] `boot/resources.rs`：`Lane`、`Res`（`id` / `lane` / `needs` / `build`）、`RESOURCES` 按 §9 表序复刻现状；四份集合 + `severity()`（§4.1 / §4.3 / §9）
- [x] `boot/registry.rs`：single-flight 槽、`Pending` / `Ready` / `Failed` 状态机、`retryable` 重试、`manage` 一次性（C1 / C2 / §4.4）
- [x] `boot/runner.rs`：`run_boot`（表序 + 计时 + `severity` 分派）、`run_idle`（逐项 `ensure`，项间让出）（§3.2 / S1 / R4）
- [x] `boot/access.rs`：每资源一行类型化访问器；未就绪返回 `NotReady`、不 panic、不隐式装配（C6 / §4.4）
- [x] `src-tauri/src/lib.rs`：`pub fn run(dirs: AppDirs)`；`.setup(move |app| boot::run_boot(app, &dirs))`；删 `mod setup`（§10.2）
- [x] `src-tauri/src/window.rs`（617 行）目录化为 `window/` 7 文件：`mod` / `spec` / `create` / `geometry` / `mode` / `orb_panel` / `placement`；`mod.rs` 用 `pub use` 转发，`generate_handler!` 路径不变（§5 / §10.1）
- [x] `src-tauri/src/db_state.rs`：`DbState` + `Clone` + `db_file_stats` 从 `setup.rs` 迁出；`crate::setup::DbState` 的 29 文件 / 71 处引用同批改路径（§5 / §10.1）
- [x] `src-tauri/src/shortcuts.rs`：`register_global_shortcuts` / `SHORTCUT_SPECS` / 快捷键动作 / 警告广播从 `setup.rs` 迁出（`cmd/config.rs` 运行期复用）（§5 / §10.1）
- [x] `src-tauri/tests/support/scan.rs`：从 `assembly_contract.rs` 抽 `collect_rs_files` / `read` / `strip_rust_comments` / `declared_type_names` / `window_labels` 供两套门禁共用；`assembly_contract.rs` 的 a1–a8 / b1–b12 / g5–g6 断言本次不动（§5 / §10.1）
- [x] 删除 `src-tauri/src/setup.rs`（§10.3）

### P1b 单一来源（目录 / 配置）

- [x] `main.rs`：`AppDirs::resolve()` 唯一一次，结果经 `run(dirs)` 下传并注入 `dirs` 表项（§4.5 / N15）
- [x] `plugins.rs::log_plugin(&dirs)`、`tool_setup.rs`、`window/create.rs::show_or_create`、`vault_plugin.rs` / `clock_plugin.rs` 的 `on_setup` 删除内部 `AppDirs::resolve()`（§4.5 / N15）
- [x] `boot/conf.rs`：`BootConf::load_all()` **不收 `&dirs`**，内部 `assistant_config::get_conf_dir()` 一次定位后逐组 `load_from(path)`，启动期唯一读盘点；失败回退默认值 + `report.degrade`，**永不返回 `Err`**（S5 / S6 / I1）
- [x] `window/` 侧启动期读盘点收 `&BootConf`：`geometry.rs::restore_main_window` 与 `create.rs::apply_orb_window_attrs` 改签名，否则 P1 的 I1 只是纸面承诺（G3 只扫 `boot/`，抓不到 `window/`）（§8 补充约束 / I1）
- [x] `i18n` 语言取自 `BootConf`，不再二次 `load`（§9 #3）
- [x] `feature_on_setup` 挪到 `dirs` / `boot_conf` 之后；`SetupHook` 改为 `fn(&StartupCtx) -> Result<(), String>`；`clock` 若依赖「配置尚未加载」的时序，改为在 `on_setup` 内自行延迟首次 tick（§9 / R8）
- [x] `storage` 只留 DB 打开 / 迁移 / 种子 / `manage(DbState)`；审计裁剪移出（§9 #5）
- [x] `attachment_store` 只留目录 + `AttachmentStore`；`clean_tmp` 移出（§9 #6）
- [x] 新增 `storage_maintenance`（`lane: Idle`，`needs: [dirs, storage, attachment_store, boot_conf]`）：审计裁剪 + 工具结果清理 + `clean_tmp`（§9 #14）
- [x] `orb_shell` 合并为一项：定位 + 属性 + `show` + E14 漂移守卫；属性在构建期一次给出（§9 #10）
- [x] `run_pending_purge` 改限量（标记文件记录剩余子目录，逐次推进）（§9 进程级 / R2）

### P1c 能力门禁迁移（不含 G5 清单扩张）

- [x] `cmd/agent/mod.rs`（`ToolRegistry` ×2、`PipelineToolExecution` ×1）、`adapters/runner.rs`（`PipelineToolExecution` ×1）、`cmd/storage/mod.rs`（`try_state::<Arc<PipelineToolExecution>>` ×1）改 `access::*`；`access.rs` 本阶段只登记 `tools` / `tool_exec`（C3 / N10）
- [x] `cmd/mcp.rs`（`McpServerManager`）、`cmd/storage/mod.rs` 的 `SectionRegistry` 不在 P1 迁移，随 P3 并入 `access.rs`；G5 类型清单 P1 只含两项（§10.2 / §12.4）
- [x] 上述调用点替换与对应 `manage` 移除**同一提交**（R6 / I11）
- [x] `tool_setup.rs`（568 行）拆两段：`init` 主体 + 沙箱探测（`sandbox_host_paths` / `shell_registration_allowed`）迁入 `boot/build/tools.rs`（装配函数化，不直接 `manage`，交由 registry）；结果保留策略 / 超时现读 / `resolve_workspace` / `validate_workspace_root` 留在原文件（约 250 行）（§9 #8 / §5 / §10.2）

### P1d 前端减载（不依赖生成物的部分）

- [x] F1：删 `bootstrap()` 末尾单独的 `applyLocaleChange`（`loadConf("app")` 已应用语言）（N11 / §6.1）
- [x] F2：`themeStore.init` 改为从 `settingsStore.confs.app` 读 `theme`，删 `fetchConf("app")`（N12 / §6.1）
- [x] F3：拆 `initLocal()`（读 `confs.app.theme` + 一次 `getCurrentWindow().theme()`，render 前 `await`）与 `subscribe()`（两个监听，render 后）；`useTheme` 只留订阅（§6.1）
- [x] F4：删 render 前的 `await loadFeatures()`，改渲染后触发（N13 / §6.1 / X8）
- [x] F5：删 `bootstrap` 的 `getConfGroups()` 全量循环，改为按当前窗口显式加载首帧核心组（`app` + Orb 的 `orb` / `mascot`）；该临时清单在 P2 换成生成物时删除（N11 / §6.1 / §6.2）
- [x] `main.tsx` 时序重排：render 前只 `await loadConf("app")`（Orb 窗口加 `orb` / `mascot`）+ `await themeStore.initLocal()`；`loadFeatures()`、其余配置组、`subscribeSettingEvents()`、`themeStore.subscribe()` 全部移到 render 之后（N13 / §6.3）
- [x] `src/App.tsx`：`MainLayout` / `OrbWindow` 改动态 `import`（N13 / §6.3）
- [x] 前端 G7 断言：新建 `src/__tests__/startupBoundary.test.ts`（沿用 `@/test-utils/paths` 的 `readRepoFile` 源码扫描范式，不引入 jsdom 渲染测试）：`App.tsx` 无 `MainLayout` / `OrbWindow` 静态 `import`；`main.tsx` 的 render 前 await 链不含 `loadFeatures`（G7 / A3）

### P1e 既有测试跟随

- [x] `assembly_contract.rs`：`CALLER_FILES` 的 `"setup.rs"` / `"window.rs"` 改为实际 `boot/**`、`window/**` 路径；`LIB_RS_LINE_LIMIT` 按新行数调整；确认 `boot/` / `window/` 子目录不被插件发现误判（§8.1 / R5）
- [x] `a2_feature_cfg_only_in_boundary_files` / `b4_feature_cfg_budget_and_mutual_blocks`：递归扫到新目录，确认 `boot/**`、`window/**` 内无 `#[cfg(feature = "...")]`（`#[cfg(target_os)]` 不受限）（§8.1）
- [x] `src/features/__tests__/coreBoundary.test.ts`：`TRAVERSAL_FILES` 含 `src-tauri/src/setup.rs` / `src-tauri/src/window.rs`，`readRepoFile` 会直接抛错——删文件的同一提交内改为 `boot/**` + `window/**` + `main.rs` / `lib.rs`，且只列真正遍历挂载表的文件（§8.1 / R5）
- [x] `startup_contract.rs`：G1（id 唯一 / `needs` 在前 / 四集合互斥全划分 / `ACTIONABLE ⊆ OBSERVABLE` / `PRIMARY_ENTRY ⊆ Boot` / `BOOT_REQUIRED ⊆ Boot` / Idle 无 Fatal / `ctx.boot()` 文件 `needs` 含 `boot_conf`）（G1）
- [x] `startup_contract.rs`：G3（`boot/` 除 `conf.rs` 外无 `AppConf::load()` / `OrbConf::load()`；`AppDirs::resolve()` 在启动路径白名单内只允许出现在 `main.rs`；白名单作常量集中声明）（G3 / R9）

> 验收：`cargo test --test startup_contract`；`cargo test --test assembly_contract`；`cargo test -p assistant-app --lib`；`npx vitest run src/__tests__/startupBoundary.test.ts src/features/__tests__/coreBoundary.test.ts`；`npm run typecheck`。

### P1 遗留（不在本阶段收口，随后续阶段处理）

- [x] `coreBoundary.test.ts` 的「特性名在 core 逻辑中的足迹被冻结」仍对 `clock` 特性误报：`BootClock` / `ctx.clock` 等启动计时标识符与特性名同词不同义，共 6 个文件、42 行（`boot/ctx.rs`、`boot/mod.rs`、`boot/registry.rs`、`boot/runner.rs`、`cmd/startup.rs`、`main.rs`）。根因是特性 id 选了 core 必然使用的通用词，故**保留 `BootClock` 作为启动时钟**、把特性 `clock`（倒计时 / 正计时计时器）改名 `timekeeper`（实测该词在扫描根 `src/settings` / `src/features` / `src-tauri/src` 内 0 命中；`timer` / `alarm` / `focus` / `chrono` 分别撞 core 局部变量或依赖名，不可用）。同批改名（不改即门禁红）：`config/features.json`、`src-tauri/Cargo.toml` 的 feature 键与 `default` 集合、`src/vite-env.d.ts` 的 `__TIMEKEEPER_ENABLED__`、`src/features/registry.ts`、`src/features/clock` → `src/features/timekeeper`（`id` / `ipc.namespace` / 导出名）、`src/tools/clock` → `src/tools/timekeeper`（tool id、`tools.timekeeper.*` 文案键与两套 locale）、`src-tauri/src/clock_plugin.rs` → `timekeeper_plugin.rs`（`#[cfg(feature = "timekeeper")]` / `Builder::new("timekeeper")`）、`src-tauri/src/cmd/clock` → `cmd/timekeeper`（命令 `timekeeper_*`）、`src-tauri/capabilities/clock.json` → `timekeeper.json`（`timekeeper:default`）、`feature_mounts.rs`、`cmd/mod.rs`、`lib.rs`、`src/types/events.ts` 的 `timekeeper:finished`（事件名一并改名，避免在 core `protocol/events.rs` 落第二份足迹豁免）、`assistant-i18n` 两套 locale 的 `timekeeper.notification_*` 命名空间、`coreBoundary.test.ts`（`clock` 豁免整块删除 → 只留 `cmd/mod.rs` 的 cfg 装配点，并更新 `FEATURE_PROBE_PREFIX` / `FEATURE_ALIASES`）、`startup_contract.rs` 的 `STARTUP_PATH_FILES`、`.github/workflows/ci.yml`（`--features clock` / `CLOCK_ENABLED=0` / 探针 `--feature`）、`src/tools/__tests__/categories.test.ts` 与 `miniNavigate.test.ts`；改完重跑 `node scripts/structure-index.mjs` 刷新 CODE-INDEX（§8.1 / R5）
  **不改**：`BootClock` / `ctx.clock`（启动时钟）、`assistant-clock` crate 与其 `ClockMode` / `ClockSnapshot` 领域类型、`ClockSnapshotDto` 等 wire DTO（改名要连带 `contract:gen` 且对门禁零收益）、`ClockCircleOutlined` 等 antd 图标名、`BudgetRemainingDto.remainingWallClockMs`（wire 字段）。
  **验收**：`npx vitest run src/features/__tests__/coreBoundary.test.ts src/tools/__tests__`；`cargo test --test startup_contract`；`cargo test --test assembly_contract`；`npm run typecheck`。

---

## P2 — 窗口单一来源 + 生成物 + 主窗口懒建（§6.2 / §9 #13 / G4 / G6）

- [x] `src-tauri/crates/assistant-config/src/window_spec.rs`：`WindowSpec` 全字段 + `WINDOW_SPECS: &'static [WindowSpec]`（有序切片，不用 `HashMap`）（§6.2 / I12）
- [x] 迁移前逐字段对照 `tauri.conf.json` 的 main / orb 项，程序化建窗默认值差异单独列出（`visible` 默认 true，漏填即启动弹窗）（R3）
- [x] `assistant-config/src/lib.rs` 增 `pub mod window_spec;`（§10.2）
- [x] `assistant-config/tests/window_requirements_bindings.rs`：生成 `WINDOW_LABELS` / `WindowLabel` / `WINDOW_CONF_REQUIREMENTS`（与 `shortcut_defaults_bindings` 同形）（§6.2 / §10.1）
- [x] `package.json`：新增 `contract:gen:windows` 并接入 `contract:gen`；`contract:verify` 文件清单与 `git ls-files --error-unmatch` 增补生成物；生成物先 `git add`（§6.2 / R7）
- [x] `window/`：`spec.rs` 改为从 `assistant_config::window_spec` 读取；`create.rs` 的 `show_or_create(app, label, &dirs)` 收 dirs、`show()` 前套几何（几何从 `geometry.rs` 迁入建窗路径）；删 `geometry.rs::restore_main_window`（§4.5 / §10.2 / §10.3）
- [x] `create.rs` 的 `WebviewWindowBuilder` 注入 `initialization_script`：`window.__STARTUP_ID__ = "<uuid>"`；主窗口懒建时由 `show_or_create` 生成 `wake-<uuid>` 前缀的新 id（§6.4 / J9）
- [x] 前端首帧回传：渲染后 `requestAnimationFrame` 调 `report_first_frame(startupId, Math.round(performance.timeOrigin + performance.now()))`，Rust 直接与 `t0_epoch_ms` 相减；A13 在 P2 验收（§6.4 / A13）
- [x] `tauri.conf.json` 的 `app.windows` 置空数组（§10.2）
- [x] `assembly_contract.rs::window_labels()` 改读 `WINDOW_SPECS`，**与上一条同批提交**（§8.1 / R5）
- [x] `window_main` 的 `lane` 由 `Boot` 改 `Idle`；托盘 / 快捷键 / 单实例改经 `ensure(window_main)` 触发（§9 #13 / J8）
- [x] 前端：`bootstrap` 按 `WINDOW_CONF_REQUIREMENTS[label]` 加载首帧依赖，删 P1d 的临时清单（F5 收口）（§6.1 / §6.2）
- [x] `OrbWindow.tsx` 删除自带的 `loadConf("orb")` / `loadConf("mascot")`，由需求表驱动（§6.2）
- [x] 前端 G6 断言：核心三组静态槽齐全；`WINDOW_CONF_REQUIREMENTS` 每组可被 `getConfSlot` 解析（特性组按页面注册豁免）；未注册时 `loadConf` 返回而非抛错（G6）
- [x] `startup_contract.rs`：G4（解析 `tauri.conf.json` 断言 `app.windows` 为空数组 + `contract:verify` 的 `git diff --exit-code`）（G4）

> 验收：`cargo test -p assistant-config --test window_requirements_bindings`；`npm run contract:verify`；`cargo test --test startup_contract`；`cargo test --test assembly_contract`；`npx vitest run src/__tests__/startupBoundary.test.ts`；`npm run typecheck`。
> 回滚点：若 `wake_first_frame` 的 `wake_ms` 明显劣化，把 `window_main` 的 `lane` 改回 `Boot`（一行 diff），无需恢复 `tauri.conf.json`（§9 / M2）。

---

## P3 — 能力门禁全覆盖 + 降档 + 降级提示（§4.3 / §4.4 / G5）

- [x] `tool_runtime` 降档：`lane` `Boot`→`Idle`、`BOOT_REQUIRED`→`OBSERVABLE`、加入 `ACTIONABLE`，**三处同批**（§4.3 / I11 / R1）
- [x] `tool_runtime` 的 `build` 改为可重试的 `Degraded` 装配（失败返回可重试 `Err`，由 `registry` 记一次 warn + 广播，不升级 `Fatal`）（S5 / §4.3）
- [x] `mcp_start` 依赖 `tool_runtime`，经 `ensure` 调用；单 server 失败只记日志（§9 #15）
- [x] 剩余未登记类型（`McpServerManager` 等）并入 `access.rs`，G5 类型清单随源码派生自动跟随（C3 / §4.4）
- [x] `src/stores/system/startupStore.ts`：订阅 `startup:capability-changed`，`Pending` 置「正在准备」、`Failed` 显示可读原因 + 重试入口（C4 / C5）
- [x] 命令层统一 `NotReady` 返回，不 panic（C6 / I7）
- [x] `startup_contract.rs`：G5（`cmd/` / `adapters/` 不出现 `boot/access.rs` 已登记类型的 `state::<>` / `try_state::<>`，不出现 `boot::registry::` 内部符号；类型清单由访问器返回类型源码扫描派生）（G5）
- [ ] 行为验收：A9（工具结果目录 I/O 失败仍启动 + 「工具暂不可用」+ 重试）、A12（第二实例经 `ensure(window_main)` 唤起）、A13（首帧锚点与阶段行同一 `startup_id`）（§12.2 / §12.3）

> 验收：`cargo test --test startup_contract`；`cargo test -p assistant-app --lib`；`npx vitest run`；`npm run typecheck`；`npm run contract:verify`。
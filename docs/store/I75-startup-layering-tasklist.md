# I75 — 启动链路分层任务清单

| 项 | 值 |
|---|---|
| 状态 | 唯一任务级进度表；只记「改哪里、改没改」，语义一律见规格 |
| 日期 | 2026-11-25 |
| 规格 | [I75-startup-layering-spec.md](./I75-startup-layering-spec.md)（唯一设计来源，本文不复述） |

> 每项末尾括号是规格锚点，用于回查，不在本文展开。`N*` → §2，`J*` → §0，`S*` → §4.1，`C*` → §4.4，`F*` → §6.1，`M*` → §7.3，`G*` → §8，`I*` → §11，`A*` → §12，`R*` → §14，`P*` → §12.4。
> 勾选 = 代码已改且对应验收已跑。
> 阶段门禁子集（§12.4）：P0 只需 G2；P1 需 G1 / G3 / G7；P2 需 G4 / G6；P3 需 G5 / G8。
> 所有 `cargo` 命令在 `src-tauri` 目录执行（不带 `--manifest-path`）；`npm` / `npx` 在仓库根执行。

---

## P0 — 度量脚手架与预算基线（§7 / §12.4）

- [ ] `src-tauri/src/main.rs`：`let t0 = Instant::now()` 与 `t0_epoch_ms`（`SystemTime` → epoch 毫秒）在同一行采集，生成 `startup_id`（§4.2 / §7.1 / M4）
- [ ] `src-tauri/src/boot/ctx.rs`：`BootClock`（`startup_id` / `t0` / `t0_epoch_ms` / `last`）、`StartupReport`、`PhaseRecord`、`CapIssue`；`BootClock` 与 `StartupReport` 各 `manage` 一次（§4.2）
- [ ] 阶段日志：`startup stage=<id> lane=<…> startup_id=<uuid> ms=<n> budget_ms=<n> outcome=<ok|degraded|fatal|ignored>`；仅当 `budget_ms > 0` 且超预算时额外 warn（§7.1 / J5）
- [ ] `src-tauri/src/cmd/startup.rs`：`report_first_frame`（写首帧记录 + 首次触发 idle pass 的 `OnceLock`）、`get_startup_report`（§3.2 / §4.2）
- [ ] `src-tauri/src/ipc.rs` 的 `generate_handler!` 增两个命令；`assistant-protocol/src/lib.rs` 增 `pub mod startup;`；`startup.rs` 定义 `StartupReportDto` / `CapIssueDto` / `CapStateDto`（N16 / §10.1）
- [ ] 跑 `npm run contract:gen` 并提交 `protocol.contract.ts` 变更（§4.4 / I10）
- [ ] `initialization_script` 注入 `window.__STARTUP_ID__`；前端 `requestAnimationFrame` 回传 `Math.round(performance.timeOrigin + performance.now())`（§6.4 / J9）
- [ ] `src-tauri/tests/startup_contract.rs` 骨架：读 `plugins.rs` 源码断言 `single_instance` 早于 `log_plugin()`（G2 / N3）
- [ ] release 包取数：每组 ≥10 次、固定缩放与库规模、报 p50 / p95，T_process / T_visible / T_wake 分列（M1–M5 / §7.3）
- [ ] 新建 `docs/analysis/startup-budget-baseline.json`（§7.4 格式，`phases` 键与 `RESOURCES` id 一一对应）（§7.4）
- [ ] 回填 `RESOURCES` 的 `budget_ms`（中位数 ×2，未测保持 0）（§7.2 / J5）
- [ ] 先单独跑 `cargo test --test assembly_contract` 记录基线，确认目录化前的绿色状态（R5）

> 验收：`cargo test --test startup_contract`（G2）；`npm run typecheck`；人工取数结论写入 `docs/analysis/startup-budget-baseline.json`。

---

## P1 — 目录化 + 单一来源 + 前端减载（§5 / §4.5 / §6.1 / §8.1）

### P1a 目录化与资源表

- [ ] 新建 `src-tauri/src/boot/` 16 个文件（`mod` / `resources` / `registry` / `runner` / `ctx` / `conf` / `access` + `build/{dirs,storage,attachments,state,tools,mcp,windows,shell,maintenance}`）（§5）
- [ ] `boot/mod.rs` 只做导出，≤60 行、无业务分支（§5 / A2）
- [ ] `boot/resources.rs`：`Lane`、`Res`（`id` / `lane` / `needs` / `budget_ms` / `build`）、`RESOURCES` 按 §9 表序复刻现状；四份集合 + `severity()`（§4.1 / §4.3 / §9）
- [ ] `boot/registry.rs`：single-flight 槽、`Pending` / `Ready` / `Failed` 状态机、`retryable` 重试、`manage` 一次性（C1 / C2 / §4.4）
- [ ] `boot/runner.rs`：`run_boot`（表序 + 计时 + `severity` 分派）、`run_idle`（逐项 `ensure`，项间让出）（§3.2 / S1 / R4）
- [ ] `boot/access.rs`：每资源一行类型化访问器；未就绪返回 `NotReady`、不 panic、不隐式装配（C6 / §4.4）
- [ ] `src-tauri/src/lib.rs`：`pub fn run(dirs: AppDirs)`；`.setup(move |app| boot::run_boot(app, &dirs))`；删 `mod setup`（§10.2）
- [ ] 删除 `src-tauri/src/setup.rs`（§10.3）

### P1b 单一来源（目录 / 配置）

- [ ] `main.rs`：`AppDirs::resolve()` 唯一一次，结果经 `run(dirs)` 下传并注入 `dirs` 表项（§4.5 / N15）
- [ ] `plugins.rs::log_plugin(&dirs)`、`tool_setup.rs`、`window.rs::show_or_create`、`vault_plugin.rs` / `clock_plugin.rs` 的 `on_setup` 删除内部 `AppDirs::resolve()`（§4.5 / N15）
- [ ] `boot/conf.rs`：`BootConf::load_all(&dirs)` 经 `load_from` 显式传路径，启动期唯一读盘点；失败回退默认值 + `report.degrade`，**永不返回 `Err`**（S5 / S6 / I1）
- [ ] `i18n` 语言取自 `BootConf`，不再二次 `load`（§9 #3）
- [ ] `feature_on_setup` 挪到 `dirs` / `boot_conf` 之后；`SetupHook` 改为 `fn(&StartupCtx) -> Result<(), String>`；`clock` 若依赖「配置尚未加载」的时序，改为在 `on_setup` 内自行延迟首次 tick（§9 / R8）
- [ ] `storage` 只留 DB 打开 / 迁移 / 种子 / `manage(DbState)`；审计裁剪移出（§9 #5）
- [ ] `attachment_store` 只留目录 + `AttachmentStore`；`clean_tmp` 移出（§9 #6）
- [ ] 新增 `storage_maintenance`（`lane: Idle`，`needs: [storage, attachment_store]`）：审计裁剪 + 工具结果清理 + `clean_tmp`（§9 #14）
- [ ] `orb_shell` 合并为一项：定位 + 属性 + `show` + E14 漂移守卫；属性在构建期一次给出（§9 #10）
- [ ] `run_pending_purge` 改限量（标记文件记录剩余子目录，逐次推进）（§9 进程级 / R2）

### P1c 能力门禁迁移（不含 G5 清单扩张）

- [ ] `cmd/agent/mod.rs`（3 处）、`cmd/mcp.rs`（1 处）、`adapters/runner.rs`（1 处）的 `state::<Arc<T>>()` 改 `access::*`；`cmd/storage/mod.rs` 的 2 处 `try_state` 同改（C3 / N10）
- [ ] 上述调用点替换与对应 `manage` 移除**同一提交**（R6 / I11）
- [ ] `tool_setup.rs` 装配函数化（不直接 `manage`，交由 registry），由 `tool_runtime` 表项的 `build` 调用（§9 #8 / §10.2）

### P1d 前端减载（不依赖生成物的部分）

- [ ] F1：删 `bootstrap()` 末尾单独的 `applyLocaleChange`（`loadConf("app")` 已应用语言）（N11 / §6.1）
- [ ] F2：`themeStore.init` 改为从 `settingsStore.confs.app` 读 `theme`，删 `fetchConf("app")`（N12 / §6.1）
- [ ] F3：拆 `initLocal()`（读 `confs.app.theme` + 一次 `getCurrentWindow().theme()`，render 前 `await`）与 `subscribe()`（两个监听，render 后）；`useTheme` 只留订阅（§6.1）
- [ ] F4：删 render 前的 `await loadFeatures()`，改渲染后触发（N13 / §6.1 / X8）
- [ ] F5：删 `bootstrap` 的 `getConfGroups()` 全量循环，改为按当前窗口显式加载首帧核心组（`app` + Orb 的 `orb` / `mascot`）；该临时清单在 P2 换成生成物时删除（N11 / §6.1 / §6.2）
- [ ] `main.tsx` 时序重排：render 前只 `await loadConf("app")`（Orb 窗口加 `orb` / `mascot`）+ `await themeStore.initLocal()`；`loadFeatures()`、其余配置组、`subscribeSettingEvents()`、`themeStore.subscribe()` 全部移到 render 之后（N13 / §6.3）
- [ ] `src/App.tsx`：`MainLayout` / `OrbWindow` 改动态 `import`（N13 / §6.3）
- [ ] 前端 G7 断言：新建 `src/__tests__/startupBoundary.test.ts`（沿用 `@/test-utils/paths` 的 `readRepoFile` 源码扫描范式，不引入 jsdom 渲染测试）：`App.tsx` 无 `MainLayout` / `OrbWindow` 静态 `import`；`main.tsx` 的 render 前 await 链不含 `loadFeatures`（G7 / A3）

### P1e 既有测试跟随

- [ ] `assembly_contract.rs`：`CALLER_FILES` 的 `"setup.rs"` 改为实际 `boot/*` 路径；`LIB_RS_LINE_LIMIT` 按新行数调整；确认 `boot/` 子目录不被插件发现误判（§8.1 / R5）
- [ ] `src/features/__tests__/coreBoundary.test.ts`：`TRAVERSAL_FILES` 含 `src-tauri/src/setup.rs`，`readRepoFile` 会直接抛错——删 `setup.rs` 的同一提交内改为新的调用方文件清单（`boot/*` + `main.rs` / `lib.rs`）（§8.1 / R5）
- [ ] `startup_contract.rs`：G1（id 唯一 / `needs` 在前 / 四集合互斥全划分 / `ACTIONABLE ⊆ OBSERVABLE` / `PRIMARY_ENTRY ⊆ Boot` / `BOOT_REQUIRED ⊆ Boot` / Idle 无 Fatal / `ctx.boot()` 文件 `needs` 含 `boot_conf`）（G1）
- [ ] `startup_contract.rs`：G3（`boot/` 除 `conf.rs` 外无 `AppConf::load()` / `OrbConf::load()`；`AppDirs::resolve()` 在启动路径白名单内只允许出现在 `main.rs`；白名单作常量集中声明）（G3 / R9）

> 验收：`cargo test --test startup_contract`；`cargo test --test assembly_contract`；`cargo test -p assistant-app --lib`；`npx vitest run src/__tests__/startupBoundary.test.ts src/features/__tests__/coreBoundary.test.ts`；`npm run typecheck`。

---

## P2 — 窗口单一来源 + 生成物 + 主窗口懒建（§6.2 / §9 #13 / G4 / G6）

- [ ] `src-tauri/crates/assistant-config/src/window_spec.rs`：`WindowSpec` 全字段 + `WINDOW_SPECS: &'static [WindowSpec]`（有序切片，不用 `HashMap`）（§6.2 / I12）
- [ ] 迁移前逐字段对照 `tauri.conf.json` 的 main / orb 项，程序化建窗默认值差异单独列出（`visible` 默认 true，漏填即启动弹窗）（R3）
- [ ] `assistant-config/src/lib.rs` 增 `pub mod window_spec;`（§10.2）
- [ ] `assistant-config/tests/window_requirements_bindings.rs`：生成 `WINDOW_LABELS` / `WindowLabel` / `WINDOW_CONF_REQUIREMENTS`（与 `shortcut_defaults_bindings` 同形）（§6.2 / §10.1）
- [ ] `package.json`：新增 `contract:gen:windows` 并接入 `contract:gen`；`contract:verify` 文件清单与 `git ls-files --error-unmatch` 增补生成物；生成物先 `git add`（§6.2 / R7）
- [ ] `window.rs`：改为从 `assistant_config::window_spec` 读取；`show_or_create(app, label, &dirs)` 收 dirs、`show()` 前套几何、注入 `initialization_script`；删 `restore_main_window`（§4.5 / §10.2 / §10.3）
- [ ] `tauri.conf.json` 的 `app.windows` 置空数组（§10.2）
- [ ] `assembly_contract.rs::window_labels()` 改读 `WINDOW_SPECS`，**与上一条同批提交**（§8.1 / R5）
- [ ] `window_main` 的 `lane` 由 `Boot` 改 `Idle`；托盘 / 快捷键 / 单实例改经 `ensure(window_main)` 触发（§9 #13 / J8）
- [ ] 前端：`bootstrap` 按 `WINDOW_CONF_REQUIREMENTS[label]` 加载首帧依赖，删 P1d 的临时清单（F5 收口）（§6.1 / §6.2）
- [ ] `OrbWindow.tsx` 删除自带的 `loadConf("orb")` / `loadConf("mascot")`，由需求表驱动（§6.2）
- [ ] 前端 G6 断言：核心三组静态槽齐全；`WINDOW_CONF_REQUIREMENTS` 每组可被 `getConfSlot` 解析（特性组按页面注册豁免）；未注册时 `loadConf` 返回而非抛错（G6）
- [ ] `startup_contract.rs`：G4（解析 `tauri.conf.json` 断言 `app.windows` 为空数组 + `contract:verify` 的 `git diff --exit-code`）（G4）

> 验收：`cargo test -p assistant-config --test window_requirements_bindings`；`npm run contract:verify`；`cargo test --test startup_contract`；`cargo test --test assembly_contract`；`npx vitest run src/settings/core/runtime/__tests__`；`npm run typecheck`。
> 回滚点：若 T_wake 劣化超 ±5%，把 `window_main` 的 `lane` 改回 `Boot`（一行 diff），无需恢复 `tauri.conf.json`（§9 / A15）。

---

## P3 — 能力门禁全覆盖 + 降档 + 降级提示（§4.3 / §4.4 / G5 / G8）

- [ ] `tool_runtime` 降档：`lane` `Boot`→`Idle`、`BOOT_REQUIRED`→`OBSERVABLE`、加入 `ACTIONABLE`，**三处同批**（§4.3 / I11 / R1）
- [ ] `tool_runtime` 的 `build` 改为可重试的 `Degraded` 装配（失败不返回 `Err`，记 `CapIssue`）（S6 / §4.3）
- [ ] `mcp_start` 依赖 `tool_runtime`，经 `ensure` 调用；单 server 失败只记日志（§9 #15）
- [ ] 剩余未登记类型（`McpServerManager` 等）并入 `access.rs`，G5 类型清单随源码派生自动跟随（C3 / §4.4）
- [ ] `src/stores/system/startupStore.ts`：订阅 `startup:capability-changed`，`Pending` 置「正在准备」、`Failed` 显示可读原因 + 重试入口（C4 / C5）
- [ ] 命令层统一 `NotReady` 返回，不 panic（C6 / I7）
- [ ] `startup_contract.rs`：G5（`cmd/` / `adapters/` 不出现 `boot/access.rs` 已登记类型的 `state::<>` / `try_state::<>`，不出现 `boot::registry::` 内部符号；类型清单由访问器返回类型源码扫描派生）（G5）
- [ ] `startup_contract.rs`：G8（只对 `budget_ms > 0` 断言 p95 ≤ `budget_ms`；`docs/analysis/startup-budget-baseline.json` 缺失即失败；`phases` 多余键报错）（G8 / §7.4）
- [ ] P3 复测并覆盖 `docs/analysis/startup-budget-baseline.json`（`date` 即历史对比点）（§7.4 / A14）
- [ ] 行为验收：A9（工具结果目录 I/O 失败仍启动 + 「工具暂不可用」+ 重试）、A12（第二实例经 `ensure(window_main)` 唤起）、A15（T_visible 下降、T_wake 不劣化 ±5%）（§12.2 / §12.3）

> 验收：`cargo test --test startup_contract`；`cargo test -p assistant-app --lib`；`npx vitest run`；`npm run typecheck`；`npm run contract:verify`。

---

## 不做（§13）

> 有意排除项，不带勾选框（勾选只表达「已改且已验收」）。

- X1 带失效机制的全局配置缓存
- X2 通用 DI 容器 / 插件框架
- X3 把 DB 打开移出 `Boot`
- X4 trait 对象 / 动态注册实现资源表
- X5 定时器式预热（`sleep` 后装配）
- X6 独立的预热调度器（「Orb 无交互才预热」启发式）
- X7 `after` 字段
- X8 把 `loadFeatures` 拆成按窗口的模块清单
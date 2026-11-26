# I76 — 进程资源监视任务清单

| 项 | 值 |
|---|---|
| 状态 | 唯一任务级进度表；只记「改哪里、改没改」，语义一律见规格 |
| 日期 | 2026-11-26 |
| 规格 | [I76-process-resource-monitor-spec.md](./I76-process-resource-monitor-spec.md)（唯一设计来源，本文不复述） |

> 每项末尾括号是规格锚点，用于回查，不在本文展开。`§n` → 规格章节，`Vn` → §10.2 验收判据，`OQn` → §12 开放问题。
> 勾选 = 代码已改且对应验收已跑。
> 批次顺序：R0 → R1a →（R1b / R1c 与 R2 可并行）→ R2。**R0 / R2 是原子批次**（§0.13、§10.1），不允许拆开合入。
> 所有 `cargo` 命令在 `src-tauri` 目录执行（不带 `--manifest-path`）；`npm` / `npx` 在仓库根执行。

---

## R0 — 容器与契约（原子，§10.1）

### R0a 契约与命令骨架（后端）

- [x] `src-tauri/crates/assistant-protocol/src/resource.rs`（新增）：11 个类型（5 struct + 6 enum），字段与枚举成员照 §6.2（§6.2、§6.3、V16）
- [x] `src-tauri/crates/assistant-protocol/src/lib.rs`：`pub mod resource;` + `pub use resource::*;`，落点 `provider` 之后、`screenshot` 之前（§7.1 第 2 处）
- [x] `src-tauri/crates/assistant-protocol/tests/protocol_bindings.rs`：导入清单与 `declarations` 数组各加一次（§7.1 第 3 处）
- [x] `src-tauri/src/cmd/resource.rs`（新增）：两条命令骨架，均返回 `notImplemented` + `reason`（§6.1、V5、V6）
- [x] `src-tauri/src/cmd/mod.rs`：`mod resource;` + `pub use resource::*;`，落点 `provider` 之后、`screenshot` 之前（§7.1 第 4 处）
- [x] `src-tauri/src/ipc.rs`：`generate_handler!` 加 `cmd::get_resource_capabilities, cmd::get_process_snapshot`（§7.1 第 5 处、V21）
- [x] `npm run contract:gen` 并提交 `src/types/protocol.contract.ts`（§6.3 第 2 条、V18）

### R0b 页面容器与 i18n（前端）

- [x] `src/settings/core/registry/builtinRegistry.ts`：`id` → `resource`、`label` → `common.navigation.resource.title`、`keywords` 追加，其余字段不动（§3.1、V1）
- [x] `src/i18n/locales/zh-CN.json` 与 `en-US.json`：`common.navigation.storage.title` 改名为 `...resource.title`，并新增 `settings.resource.*` 最小键表，两份同批（§7.2 键表）
- [x] `src/settings/pages/StorageSettings.tsx`：改为容器（单次 `SettingsPageShell wide` + 受控 `Tabs`）+ 按 `activeKey` 记忆滚动（§3.3、V2、V24）
- [x] `src/settings/pages/storage/StorageContent.tsx`：删掉两处自带的 `SettingsPageShell wide`，只渲染 body（§3.3 第 1 条）
- [x] `src/settings/styles/settings.css`：tab 栏 `sticky top: 0` 且高度取 `--storage-sticky-top`；进程表头沿用该变量，不新增偏移常量（§3.3 第 2 条、V3）

### R0c 进程树 tab 占位与能力声明（前端）

- [x] `src/services/desktop/resourceService.ts`（新增）：两条 invoke 封装，与 `storageService.ts` 互不引用（§6.4、§7.2 第 6 条）
- [x] `src/settings/pages/resource/ProcessTreeTab.tsx`（新增）：本批只做能力声明读取与 `notImplemented` 分支（§5.5、V20、V21）
- [x] 读取失败只置本地 `loadFailed` + 横幅，不改 wire、不新增枚举取值（§2.7 约束 3、V19）
- [x] `Tabs` 第二个 `item` 挂载 `ProcessTreeTab`（§3.2）

### R0d 既有测试跟随

- [x] `src/settings/__tests__/fixtures/snapshot.full.json`、`snapshot.vaultless.json` 用 `npx vitest run -u` 重生成，只允许 `pages[].id` 一行变化（V4）
- [x] 复核无其它位置把 `"storage"` 当页面 id 断言（§3.1）
- [x] 复核路由与类型层无需改动（§3.1）

> 验收：`cargo test -p assistant-protocol --test protocol_bindings`；`npm run contract:verify`；`npm run typecheck`；`npx vitest run src/settings/__tests__/settingsSnapshot.golden.test.ts src/settings/pages/storage/__tests__/localeParity.test.ts`；手工 V1 / V2 / V3 / V17 / V21 / V24。

---

## R1a — 采样 crate 与 Windows 后端（§10.1）

> 前置：平台后端选型与取数口径已由探针实测定案，结论落在规格 §0.16 / §2.3 / §2.4 / §5.3 / §12 OQ1；本批不重复探针。

### R1a-1 crate 骨架与纯逻辑

- [ ] `src-tauri/crates/assistant-resource-probe/Cargo.toml`（新增）：依赖只有 `sysinfo`（`features = ["system"]`，关其余 feature）、`libc`（macOS 段）、`assistant-protocol`；Windows 复用工作区 `windows` 并显式声明 V27 所需 feature（§6.4、§7.1 第 7、8 处）
- [ ] `src-tauri/Cargo.toml`：`members` / `[workspace.dependencies]` / `[dependencies]` 三处加 crate（§7.1 第 1 处）
- [ ] `src/…/source.rs`：`SnapshotSource` trait + `RawProcess`，「每拍一次快照」写成契约注释（§6.4、§0.15）
- [ ] `src/…/mock.rs`：可注入任意根与进程表的假快照，供 V8 / V10 / V11 / V12 使用（§0.15）
- [ ] `src/…/tree.rs`：先过滤线程条目，再建表 + 反向 DFS + PID 复用校验 + 深度 / 条数截断（§2.2「线程条目」、§2.2 约束 4、§4.1、§8）
- [ ] `src/…/classify.rs`：三层判定与 `webviewUnknown` 计入自身（§2.3、§8）
- [ ] `src/…/cpu.rs`：`CpuDiffCache`，键 `pid + startTime`，首拍 `null`，消失键淘汰（§4.2、§2.5、§2.6）
- [ ] `src/…/lib.rs`：导出 `build_snapshot(...)` 与 `capabilities()`；不读宿主身份与配置路径（§6.4、§2.4、§2.7）
- [ ] 常量集中声明（`SAMPLE_INTERVAL_MS` / `CPU_CACHE_IDLE_TIMEOUT_MS` / `MIN_CPU_DELTA_MS` / `MAX_ENTRIES` / `MAX_DEPTH`），禁魔法数字（§2.6）

### R1a-2 Windows 后端

- [ ] `src/…/win.rs`：`sysinfo` 一轮全量枚举 + `with_cmd(UpdateKind::Always)` + `without_tasks()`（§2.2 约束 1、§2.3、§12 OQ1）
- [ ] Windows 口径：`privateWorkingSet` / `true` / `false` / `webviewDetached = false`；逐行内存取 `virtual_memory()`；`hostThreadCount` 落 `null`（§2.4 取数约束 1、§5.3、V26）
- [ ] 根 manifest 的 `windows` feature 清单不改；仅在采样 crate 内为 V27 显式声明 feature；新增依赖 / feature 单独过审（§7.1 第 8 处）

### R1a-3 命令接线与单测

- [ ] `src-tauri/src/cmd/resource.rs`：换真实调用，缓存用本文件内 `OnceLock<Mutex<CpuDiffCache>>`，禁 `AppHandle::state` / `boot::access.rs` / `boot::RESOURCES`（§2.6、§6.4）
- [ ] `get_resource_capabilities` 的 `processTree` 改由 crate 平台判定（Windows 批内 `available`，Linux / macOS 仍 `notImplemented`）（§2.7）
- [ ] 单测：V5 / V6 / V7 / V8 / V9 / V10 / V11 / V12（mock 注入）（§10.2）
- [x] V27 实机断言：`virtual_memory()` 与 `PrivateUsage` 逐字节相等，断言对象取静止子进程（自身取数会漂移，见 §10.2 V27）；开发机探针 5/5 通过，结论已回写 §10.2 V27（§10.2 V27）
- [ ] 开发机跑 `cargo check` + `cargo test`（§10.1「平台验证分工」）

> 验收：`cargo test -p assistant-resource-probe`；`cargo test --workspace --all-targets`（开发机）；`npm run contract:verify`；V5–V12、V27。

---

## R1b — Linux 后端（§10.1）

- [ ] `src/…/linux.rs`：`sysinfo` 一轮枚举；**保留默认 `with_tasks`**（宿主线程数依赖它）；线程条目在 `tree.rs` 过滤（§2.2「线程条目」、§5.3、§12 OQ1、OQ2）
- [ ] Linux 口径：`workingSet` / `false` / `false` / `webviewDetached = false`；家族清单照 §2.3（§2.4、V26）
- [ ] Linux 段依赖复用现有 `libc`，不新增依赖（§6.4）
- [ ] `capabilities()` Linux 分支改 `available`（§2.7）
- [ ] 单测：`memoryAdditive == false` 路径（合计位无数字 + 逐行照常）（V14）
- [ ] CI `ubuntu-latest` 真机验证（V22、V23 Linux 部分）

> 验收：CI 绿（`ubuntu-latest`）；V14、V22、V23（Linux 部分）。

---

## R1c — macOS 后端（§10.1）

- [ ] `src/…/macos.rs`：`sysinfo` 枚举进程与命令行；**逐行内存改 `libc::proc_pid_rusage` 的 `ri_phys_footprint`**；宿主线程数取 `proc_pidinfo(PROC_PIDTASKINFO).pti_threadnum`（§2.4 取数约束 2、§5.3、§12 OQ1、OQ2）
- [ ] macOS 口径：`physFootprint` / `true` / `true` / `webviewDetached = true`；家族清单照 §2.3（§2.4、V26）
- [ ] macOS 段依赖（§6.4）
- [ ] `capabilities()` macOS 分支改 `available`（§2.7）
- [ ] 单测：`phys_footprint` 口径与 `webviewDetached` 不入 `entries`（V26）
- [ ] CI `macos-latest` 真机验证（V22、V23 macOS 部分）

> 验收：CI 绿（`macos-latest`）；V22、V23（macOS 部分）、V26。

---

## R2 — 实时树 UI（原子，§10.1）

- [ ] `src/settings/pages/resource/ProcessTreeTab.tsx`：一次性补全表 + 两个合计 + 宿主线程数 + 口径分支 + §5.5 全部状态（§10.1 R2）
- [ ] 表格：四列，行直接渲染后端 `entries`，前端不重建树，不设推迟项列（§5.1、§5.2 约束 2）
- [ ] 两个合计同时显示并标注包含关系，禁止相加；CPU 合计注明单核百分比可超 100（§5.2、§2.5）
- [ ] 宿主线程数进合计头，不进表列；`null` 不填 `0`（§5.3、V16）
- [ ] 口径分支由 `memoryAdditive` 字段驱动；内存列名由 `memoryBasis` 驱动（§5.4、V14、V15）
- [ ] 状态展示齐备（`notImplemented` / `unreadable` / `memoryUnavailable` / `truncated` / CPU `null` / `coverageSubtreeOnly` / `webviewDetached` / 能力声明失败横幅）（§5.5、V13、V17、V19、V20、V26）
- [ ] 轮询 `SAMPLE_INTERVAL_MS`，卸载即停，无开始 / 停止按钮（§2.6、V17）
- [ ] 表头吸顶复用 `--storage-sticky-top`（§3.3 第 2 条、V3）
- [ ] 复核页面内无任何推迟项占位 UI（§1.2、§5.6、V17）

> 验收：`npm run typecheck`；`npx vitest run`；手工 V2 / V13 / V15 / V17 / V20 / V26；实机 V14 / V23。

---

## 收尾

- [ ] 全量回归：`cargo test --workspace --all-targets --no-fail-fast`（开发机）+ `npm run contract:verify` + `npm run typecheck` + `npm run lint`
- [ ] 复核 V25（三平台齐备后 `notImplemented` 不出现在 `processTree` 能力上）
- [ ] 复核 V22（Linux / macOS 由 CI 覆盖、Windows 由开发机覆盖，未新增 job）

---

## 未决项（不阻塞开工）

| 项 | 结论 |
|---|---|
| OQ1 平台后端选型 | 已定案 `sysinfo 0.40`（§12 OQ1）；仍须单独过审，不过审则回退手写 |
| OQ2 角色分类数据源 | 已定案：`sysinfo` 的 `cmd`（§12 OQ2、§2.3） |
| OQ4 平台单测是否进 CI | 纯逻辑进既有矩阵，OS 层用 mock；不另加 job（§12 OQ4） |
| OQ6 WebView profile 目录 | 只作辅助判据（§12 OQ6） |
| OQ7 能力声明是否缓存 | 不缓存（§12 OQ7） |
| OQ8 页面 icon | 保持 `DatabaseOutlined`（§12 OQ8） |
| OQ9 macOS XPC 归属辅助 | 本期不引入（§12 OQ9） |
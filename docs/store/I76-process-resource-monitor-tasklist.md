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

- [ ] `src-tauri/crates/assistant-protocol/src/resource.rs`（新增）：11 个类型 = 5 struct（`ResourceCapabilitiesDto` / `ResourceTabCapabilityDto` / `ProcessSnapshotDto` / `ProcessGroupDto` / `ProcessEntryDto`）+ 6 enum（`ResourceTabDto` / `ResourceAvailabilityDto` / `ProcessRoleDto` / `MemoryBasisDto` / `CpuBasisDto` / `CoverageDto`）；`#[serde(rename_all = "camelCase")]`；输出 `Option` 一律不加 `skip_serializing_if`；`ProcessEntryDto` **无** `threadCount` 字段（§6.2、§6.3 第 1 条、V16）
  - 字段逐条照 §6.2 表：`ProcessSnapshotDto` 含 `availability` / `reason` / `sampledAt` / `rootPid` / `memoryBasis` / `memoryAdditive` / `memoryEqualsFootprint` / `cpuBasis` / `coverage` / `webviewDetached` / `truncated` / `hostThreadCount` / `selfGroup` / `subtreeGroup` / `entries`
  - 封闭枚举成员只列 §6.2「封闭枚举清单」，无保留值、无未用成员
- [ ] `src-tauri/crates/assistant-protocol/src/lib.rs`：加 `pub mod resource;` 与 `pub use resource::*;`——两处各一次；该文件 `pub mod` 组与 `pub use` 组分开且均为字母序，两处落点都是 `provider` 之后、`screenshot` 之前（§7.1 第 2 处）
- [ ] `src-tauri/crates/assistant-protocol/tests/protocol_bindings.rs`：**两处各加一次**——`use assistant_protocol::{...}` 导入清单、`export_protocol_contract` 的 `declarations` 数组（11 个 `::decl(&cfg)`）；漏加该测试直接失败（§7.1 第 3 处）
- [ ] `src-tauri/src/cmd/resource.rs`（新增）：两条命令骨架（§6.1、§10.1 R0）
  - `get_resource_capabilities`：`tabs` 恒两项，`storage` 恒 `available`、`processTree` 为 `notImplemented` + `reason`；**不含任何数字字段**（V5、V6）
  - `get_process_snapshot`：顶层 `notImplemented` + `reason`，`entries` 为空、两个组合计 `null`、`sampledAt` 为 `null`（**不填 `0`**）；`rootPid` 取 `std::process::id()`
- [ ] `src-tauri/src/cmd/mod.rs`：加 `pub mod resource;` 与 `pub use resource::*;`——两处各一次；该文件 `mod` / `pub mod` 混排一组、`pub use` 一组，均为字母序，两处落点都是 `provider` 之后、`screenshot` 之前（§7.1 第 4 处）
- [ ] `src-tauri/src/ipc.rs`：`generate_handler!` 内加 `cmd::get_resource_capabilities, cmd::get_process_snapshot`（§7.1 第 5 处、V21）
- [ ] 跑 `npm run contract:gen` 并提交 `src/types/protocol.contract.ts` 变更（§6.3 第 2 条、V18）

### R0b 页面容器与 i18n（前端）

- [ ] `src/settings/core/registry/builtinRegistry.ts`：`id` 由 `storage` 改为 `resource`；`label` 改为 `common.navigation.resource.title`；`keywords` 追加 `resource` / `process` / `cpu` / `memory` / `资源` / `进程`，**保留**原 7 个词；`categoryId: "general"` / `order: 60` / `icon: DatabaseOutlined` / `emptyPage: true` / `load` **全部不动**（§3.1、§7.2 第 1 条、V1）
- [ ] `src/i18n/locales/zh-CN.json` 与 `src/i18n/locales/en-US.json`：`common.navigation.storage.title` **键名改为** `common.navigation.resource.title`（值「资源监视器」/`Resource Monitor`）；**两份同批改**，否则 `src/settings/pages/storage/__tests__/localeParity.test.ts` 的「zh / en 键集合相等」断言失败（§3.1、§7.2 第 2 条）
- [ ] 两份 locale 新增 `settings.resource.*` 最小键表（逐条照 §7.2 键表）：`tab.storage` / `tab.processTree`；`column.process` / `column.role` / `column.cpu` / `column.memory`；`summary.self` / `summary.subtree` / `summary.hostThreads`；`status.notImplemented` / `status.unreadable` / `status.memoryUnavailable` / `status.truncated` / `status.cpuFirstSample` / `status.coverageSubtreeOnly` / `status.coveragePlatformManaged` / `status.capabilitiesLoadFailed`；`caliber.winPrivate` / `caliber.linuxWorkingSet` / `caliber.macFootprint`；`settings.storage.*` 内容键**一律保留**（§7.2 键表）
- [ ] `src/settings/pages/StorageSettings.tsx`：改为容器——唯一一次 `SettingsPageShell wide` + antd `Tabs`（受控：`activeKey` + `onChange` + 两个 `items`，切 tab 只换 `children`）；tab key 用 `storage` / `processTree`（**不跟 id 改名**）；页面标题由壳读 manifest `label` 自动变（**不新增标题文案**）（§3.3 第 1、3 条、§3.1 末行）
- [ ] 同一文件内实现滚动记忆：页面根节点 `ref` → 向上取第一个可滚动祖先（`src/layouts/AppShell.tsx` 的 `Layout.Content`，`overflow: auto`）→ 按 `activeKey` 保存 / 恢复 `scrollTop`；**禁止**新建内层滚动容器、**禁止**改 `AppShell`（§3.3 第 3 条、§7.2 第 3 条、V24）
- [ ] `src/settings/pages/storage/StorageContent.tsx`：删掉两处自带的 `SettingsPageShell wide`（「加载 / 失败」分支与「正常」分支各一次），改为只渲染 body（§3.3 第 1 条、§7.2 第 4 条）
- [ ] 不新增第二个 sticky 偏移常量：tab 栏 `position: sticky; top: 0`，进程表表头 `top: var(--storage-sticky-top)`（该变量定义在 `.settings-page-shell` 上，当前 40px，既有表头也用同一变量）；新样式追加进 `src/settings/styles/settings.css`，**不改** 既有 `.storage-*` 规则（§3.3 第 2 条、V3）

### R0c 进程树 tab 占位与能力声明读取（前端）

- [ ] `src/services/desktop/resourceService.ts`（新增）：`getResourceCapabilities()` / `getProcessSnapshot()`，沿用 `src/services/system/tauriInvoke` 的 `tauriInvoke`；与 `storageService.ts` **分开文件、互不引用**（§6.4、§7.2 第 6 条）
- [ ] `src/settings/pages/resource/ProcessTreeTab.tsx`（新增）：本批只实现能力声明读取与 §5.5 的 `notImplemented` 分支（文案 + `reason`，**非空占位**）；**不含**表、合计、线程数（§10.1 R0、V20、V21）
- [ ] 本地状态类型 `CapabilitiesView = ResourceCapabilitiesDto & { loadFailed?: boolean }`；读取失败只置 `loadFailed` + 本地横幅 + 保留上次渲染，**不改 wire、不新增枚举取值、不默认判为可用**（§7.2 末段、§2.7 约束 3、V19）
- [ ] R0b 建好的 `Tabs` 第二个 `item` 挂载 `ProcessTreeTab`（`key: "processTree"`，`label` 用 `settings.resource.tab.processTree`）（§3.2）

### R0d 既有测试跟随

- [ ] `src/settings/__tests__/fixtures/snapshot.full.json`、`snapshot.vaultless.json`：用 `npx vitest run -u src/settings/__tests__/settingsSnapshot.golden.test.ts` 重生成；**只允许 `pages[].id` 由 `storage` → `resource` 一行变化**，`snapshot.diff.json` 无变化；diff 需人工核对，**禁止**手改 fixture（§10.2 V4）
- [ ] 复核仓库内无其它位置以字符串 `"storage"` 作为**页面 id** 断言（现状仅 `builtinRegistry.ts` 与两份 fixture 出现；CSS 类 `.storage-*`、`createLogger("storage")`、`settings.storage.*` 内容键、`chatErrors.ts` 的 `Storage = "storage"` 错误码均**不改**）（§3.1）
- [ ] 复核路由与类型层**无需**改动：`SettingsPageId = string & {}`（不是字面量联合），`MainLayout.tsx` 只有参数化路由 `/settings/:pageId`，URL 随 id 自动变为 `/settings/resource`；**禁止**新增 redirect / 别名路由（§3.1、§9）

> 验收：`cargo test -p assistant-protocol --test protocol_bindings`；`npm run contract:verify`；`npm run typecheck`；`npx vitest run src/settings/__tests__/settingsSnapshot.golden.test.ts src/settings/pages/storage/__tests__/localeParity.test.ts`；手工 V1 / V2 / V3 / V17 / V21 / V24。

---

## R1a — 采样 crate 与 Windows 后端（§10.1）

### R1a-1 crate 骨架与纯逻辑

- [ ] `src-tauri/crates/assistant-resource-probe/Cargo.toml`（新增）：包名 `assistant-resource-probe`；`[target.'cfg(target_os = "linux")'.dependencies]` 与 `[target.'cfg(target_os = "macos")'.dependencies]` 两个依赖段；Unix 侧共用依赖用 `cfg(any(target_os = "linux", target_os = "macos"))` 段；Windows 侧复用工作区 `windows` 依赖；依赖只有 OS API 与 `assistant-protocol`（§6.4、§7.1 第 7 处）
- [ ] `src-tauri/Cargo.toml`：`[workspace] members` 加 `"crates/assistant-resource-probe"`（当前 14 个成员）；`[workspace.dependencies]` 加 path 条目；`[dependencies]` 加 `assistant-resource-probe = { workspace = true }`（§7.1 第 1 处）
- [ ] `src-tauri/crates/assistant-resource-probe/src/source.rs`：`SnapshotSource` trait + `RawProcess`（`pid` / `parentPid` / `startTime` / `memoryBytes` / `cpuTime` / `threadCount` / `exeName` / `cmdline`）；「每拍一次快照」写成 trait 契约注释（§6.4、§2.2 约束 2、§0.15）
- [ ] `src-tauri/crates/assistant-resource-probe/src/mock.rs`：假快照实现，可注入任意根与任意进程表；必须能构造 V8 / V10 / V11 / V12 的用例（孙进程、同名镜像不在子树、`startTime` 倒挂、枚举次数计数）（§0.15、§7.1 第 6 处）
- [ ] `src-tauri/crates/assistant-resource-probe/src/tree.rs`：建 `pid → 条目` 表；从 `rootPid` 沿 `parentPid` **反向**遍历、DFS 先序输出；PID 复用校验（`child.startTime >= parent.startTime` 且父 PID 在快照内，不通过则排除并标注）；`MAX_DEPTH = 16` / `MAX_ENTRIES = 512` 截断置 `truncated`（§2.2 约束 4、§4.1、§8）
- [ ] `src-tauri/crates/assistant-resource-probe/src/classify.rs`：三层判定（① 归属 → ② 家族 → ③ 命令行 `--type=`）；家族清单三平台照 §2.3；命令行读不到落 `webviewUnknown` 且**仍计入自身**（§2.3、§8）
- [ ] `src-tauri/crates/assistant-resource-probe/src/cpu.rs`：`CpuDiffCache`——键 `pid + startTime`；`(t2 - t1) / (wall2 - wall1) / 核数 × 100`；首拍 `null`；`MIN_CPU_DELTA_MS = 500` 不足则沿用上一拍；快照中消失的键淘汰（§4.2、§2.5、§2.6）
- [ ] `src-tauri/crates/assistant-resource-probe/src/lib.rs`：导出面 = `build_snapshot(source, root_pid, &mut cache, now)` 组装 `ProcessSnapshotDto`（顶层 `memoryBasis` / `memoryAdditive` / `memoryEqualsFootprint` / `cpuBasis` / `coverage` / `webviewDetached` 按平台常量给出）+ `capabilities()` 按 `cfg` 给出 `processTree` 的 availability（后端未接线的平台返回 `notImplemented` + `reason`）；**不读宿主身份、不读配置路径**（§6.4、§2.4、§2.7）
- [ ] 常量集中声明：`SAMPLE_INTERVAL_MS = 1000`（前端轮询）、`CPU_CACHE_IDLE_TIMEOUT_MS = 3000`、`MIN_CPU_DELTA_MS = 500`、`MAX_ENTRIES = 512`、`MAX_DEPTH = 16`；禁止魔法数字；`CPU_CACHE_IDLE_TIMEOUT_MS` 必须落在 `SAMPLE_INTERVAL_MS` 的 2–3 倍区间（§2.6 常量行）

### R1a-2 Windows 后端

- [ ] `src-tauri/crates/assistant-resource-probe/src/win.rs`：`#[cfg(target_os = "windows")]` 后端，**一轮**全量枚举（`NtQuerySystemInformation(SystemProcessInformation)`；若 OQ1 定案 `sysinfo`，则确认其单次遍历同时给出六项字段）（§2.2 约束 1、§12 OQ1）
- [ ] Windows 口径常量：`memoryBasis = privateWorkingSet`、`memoryAdditive = true`、`memoryEqualsFootprint = false`、`webviewDetached = false`（§2.4、§5.4、V26）
- [ ] 若走手写：`src-tauri/Cargo.toml` 的 `windows` feature 清单追加 `Win32_System_ProcessStatus`、`Win32_System_Diagnostics_ToolHelp`（现有清单无进程枚举项）；新增 feature / 新增依赖**单独过审**（§7.1 第 8 处、§12 OQ1）
- [ ] 命令行读取只对「已确认在子树内且家族命中」的进程执行，读不到不重试、不降级归属（§2.3 唯一例外）

### R1a-3 命令接线与单测

- [ ] `src-tauri/src/cmd/resource.rs`：骨架换真实调用——注入 `std::process::id()` 作 `rootPid`；缓存为**本文件内** `OnceLock<Mutex<CpuDiffCache>>`，首次命令调用才初始化；**禁止**用 `AppHandle::state` 持有、**禁止**挂到 `boot::access.rs` 访问器、**禁止**登记进 `boot::RESOURCES`（`Lane::Boot` 与 `Lane::Idle` 都算，后者由 `boot::runner::run_idle` 惰性 `ensure`）；本文件在 I75 的 G5 扫描范围内（`cmd/` 不得出现 `boot/access.rs` 已登记类型的 `state::<>` / `try_state::<>`），`OnceLock` 写法天然合规（§2.6、§6.1、§6.4）
- [ ] `get_resource_capabilities` 的 `processTree` 改由 crate 的平台支持判定（Windows 批内为 `available`；Linux / macOS 仍 `notImplemented`，直至 R1b / R1c）（§2.7、§10.1）
- [ ] 单测（mock 注入）：V6 `tabs` 两项且 `tab` 不重复、V7 同一响应内 `entries` 与两个组同源 `sampledAt`、V8 孙进程角色正确、V9 `pid` 不重复、V10 同名镜像但不在子树必被排除、V11 PID 复用排除并标注、V12 快照枚举计数为 1 且不随进程数增长；另加一条断言 `get_resource_capabilities` 响应结构无数字字段（对齐 V5 契约审查）（§10.2）
- [ ] 开发机跑 `cargo check` + `cargo test`（Windows 不在 CI 矩阵内，见 §10.1「平台验证分工」）

> 验收：`cargo test -p assistant-resource-probe`；`cargo test --workspace --all-targets`（开发机）；`npm run contract:verify`；V5–V12。

---

## R1b — Linux 后端（§10.1）

- [ ] `src-tauri/crates/assistant-resource-probe/src/linux.rs`：`#[cfg(target_os = "linux")]` 后端，一轮内枚举 `/proc` 并每进程读 `stat` / `statm`（或 `status`）；命令行走 `/proc/<pid>/cmdline`（§2.2 约束 1、§12 OQ1、§12 OQ2）
- [ ] Linux 口径常量：`memoryBasis = workingSet`、`memoryAdditive = **false**`、`memoryEqualsFootprint = false`、`webviewDetached = false`；家族清单 `WebKitWebProcess` / `WebKitNetworkProcess`（§2.4、§2.3、V26）
- [ ] Linux 段依赖：复用现有 `libc`（`cfg(any(target_os = "linux", target_os = "macos"))` 段），不新增依赖（§6.4）
- [ ] 能力声明：`capabilities()` 的 Linux 分支由 `notImplemented` 改为 `available`（`reason` 置 `null`）（§2.7）
- [ ] 单测：`memoryAdditive == false` 路径——合计位**无数字**、`memoryAvailability == unavailable`、`memoryReason` 非空、逐行数字照常显示（V14）
- [ ] 真机验证：`ubuntu-latest` 随既有 CI 矩阵跑 `cargo test --workspace --all-targets`；取一次真机快照核对 `memoryBasis` / `memoryAdditive`（V22、V23 的 Linux 分支）

> 验收：CI 绿（`ubuntu-latest`）；V14、V22、V23（Linux 部分）。

---

## R1c — macOS 后端（§10.1）

- [ ] `src-tauri/crates/assistant-resource-probe/src/macos.rs`：`#[cfg(target_os = "macos")]` 后端，`proc_listpids` 一轮列 PID 后 `proc_pidinfo` 每进程取一次；命令行走 `sysctl`（§2.2 约束 1、§12 OQ1、§12 OQ2）
- [ ] macOS 口径常量：`memoryBasis = physFootprint`、`memoryAdditive = true`、`memoryEqualsFootprint = **true**`、`webviewDetached = **true**`；家族清单 `com.apple.WebKit.WebContent` / `com.apple.WebKit.Networking` / `com.apple.WebKit.GPU`（§2.4、§2.3、V26）
- [ ] macOS 段依赖：`[target.'cfg(target_os = "macos")'.dependencies]`（§6.4）
- [ ] 能力声明：`capabilities()` 的 macOS 分支由 `notImplemented` 改为 `available`（`reason` 置 `null`）（§2.7）
- [ ] 单测：`phys_footprint` 口径；`webviewDetached == true` 时 WebView 家族进程**不**因镜像名被收进 `entries`，`selfGroup` 只含宿主（V26）
- [ ] 真机验证：`macos-latest` 随既有 CI 矩阵跑测试；取一次真机快照，确认 WebView 进程不在子树内时如实展示平台托管说明（V22、V23 的 macOS 分支）

> 验收：CI 绿（`macos-latest`）；V22、V23（macOS 部分）、V26。

---

## R2 — 实时树 UI（原子，§10.1）

- [ ] `src/settings/pages/resource/ProcessTreeTab.tsx`：补全为**一次性齐备**的完整 tab——表 + 两个合计 + 宿主线程数 + 口径分支 + §5.5 全部状态（§10.1 R2）
- [ ] 表格：列 = 进程（`exeName` + `pid`）/ 角色 / CPU / 内存（列名带口径）；**行 = 后端树序 `entries` 直接渲染，前端不重建树**；**不设**「存在」/「投影」/「线程数」/「任务名」/「停止」列（§5.1、§5.2 约束 2）
- [ ] 进程列按 `exeName (pid 1234)` 展示：同一角色的多个进程**按 PID 区分**，**不做**「渲染进程 #N」这类编号，也不引入任何任务名归因（§5.1「无归因边界」）
- [ ] 两个合计：`selfGroup` 与 `subtreeGroup` **同时显示**并标注包含关系；**禁止**相加；CPU 合计注明「单核百分比之和，可超过 100」（§5.2、§2.5）
- [ ] 宿主线程数进合计头（`host · pid · N 线程`），**不进**表列；`null` 时不填 `0`（§5.3、V16）
- [ ] 口径分支由 `memoryAdditive` **字段驱动**：`true` → 显示数字（`memoryEqualsFootprint` 时标「≈ 物理占用」，否则标「不含共享页重复计数」）；`false` → 合计位显示 `unavailable` + `memoryReason`，逐行照常显示（§5.4、V14、V15）
- [ ] 内存列名由 `memoryBasis` 驱动（`workingSet` / `privateWorkingSet` / `physFootprint` 三套文案）（§5.4、V15）
- [ ] 状态展示齐备：`notImplemented`（整 tab 原因）、`unreadable`（失败 + 保留上次快照 + `sampledAt`）、组 `memoryUnavailable`、`truncated` 横幅、CPU `null` 显示「本拍无基准」（**禁止**错误态）、`coverageSubtreeOnly` 常驻行、`webviewDetached` 常驻行、能力声明读取失败本地横幅（§5.5、V13、V17、V19、V20、V26）
- [ ] 轮询：按 `SAMPLE_INTERVAL_MS = 1000` 拉取；组件卸载即停止（**无**常驻线程、**无**开始 / 停止按钮）；loading 与上次快照时刻可见（§2.6、§8、V17）
- [ ] 表头吸顶复用 `--storage-sticky-top`，与 tab 栏共用同一偏移（§3.3 第 2 条、V3）
- [ ] 复核页面内**无**峰值区块、无停止按钮、无任务名列、无任何推迟项占位 UI（§1.2、§5.6、V17）

> 验收：`npm run typecheck`；`npx vitest run`；手工 V2 / V13 / V15 / V17 / V20 / V26；实机 V14 / V23。

---

## 收尾

- [ ] 全量回归：`cargo test --workspace --all-targets --no-fail-fast`（开发机）+ `npm run contract:verify` + `npm run typecheck` + `npm run lint`
- [ ] 复核 V25：三平台齐备后，正常构建中 `notImplemented` **不**出现在 `processTree` 能力上（§10.2 V25）
- [ ] 复核 V22：Linux / macOS 由 CI 覆盖、Windows 由开发机覆盖，**未新增**交叉编译 job（§10.2 V22）

---

## 未决项（不阻塞开工，按 §12 的确定行为推进）

| 项 | 未定案时的行为 |
|---|---|
| OQ1 各平台后端选型（三平台各自独立定案） | 优先 `sysinfo`（仓库当前没有；`Cargo.lock` 的 `procfs-core` 是 `minidump-writer` 的传递依赖，不可直接当采样后端）；不满足「单次遍历全字段」则该平台改手写；未定案平台报 `notImplemented` |
| OQ2 角色分类数据源细节 | 判据不变（§2.3 三层），来源随 OQ1：Windows 读命令行 / Linux 读 `/proc/<pid>/cmdline` / macOS `sysctl`；读不到一律落 `webviewUnknown` 且仍计入自身 |
| OQ4 平台相关单测是否进 CI | 跨平台单测只覆盖纯逻辑（`tree` / `classify` / `cpu`），OS 调用层由 `SnapshotSource` trait + mock 隔离；新增 crate 自动纳入既有 CI 矩阵，**不另加 job** |
| OQ6 WebView profile 目录作辅助判据 | 可作辅助，**不得**作主判据；角色分类自足于命令行特征 |
| OQ7 能力声明是否缓存 | 不缓存；只返回状态，成本可忽略，缓存反而引入状态陈旧 |
| OQ8 页面 icon | 保持 `DatabaseOutlined` 不改 |
| OQ9 macOS XPC 归属辅助 | 本期不引入，按 `webviewDetached` 如实展示平台托管说明 |
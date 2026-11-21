# 时钟工具可选化（clock feature）规格

| 项 | 值 |
|---|---|
| 状态 | 已实施（S1–S8 全部落地；门禁、编译、测试、关闭态探针全绿） |
| 日期 | 2026-11-21 |
| 关联 | [可选功能二进制编译规格](./feature-optional-binary-spec.md)（契约唯一来源：R1–R28、B1–B12、F1–F7、CI-1/CI-2、§8.1 新增特性序列）、[crate-architecture](./crate-architecture.md) §5（后端 crate 拆分判据唯一来源）、[vault-optional-build](./vault-optional-build.md)（L1 参考实现）、[CODE-SPLIT-PRINCIPLE](../principle/CODE-SPLIT-PRINCIPLE.md)（拆分原则，只引用不复述） |
| 影响范围 | 后端 12 处装配点 + 新建 1 个 crate + 新建 1 个插件文件；前端 6 处装配点 + 新建 1 个声明模块；门禁/脚本 4 处扩展；CI 增加 2 条单特性 lane；`config/presets.json` 首次落地（N 由 1 变 2） |

> 一句话：clock 是本仓库**第二个**可选特性，也是第一个「工具型」特性。本文的价值不在时钟本身，而在把「一个工具如何从默认二进制里整块消失」的**装配点模板**固化下来——后端一个薄 crate + 一个插件边界，前端一个空声明模块 + 工具注册表里的一处编译期守卫，门禁把 `src/tools/<id>/` 纳入产物证据。便签等后续工具照此模板执行，只需替换 id。

编号约定：`N*` 现状事实；`P*` 装配点（**实施动作**）；`G*` 门禁/脚本扩展；`S*` 实施步骤；`R*` 残余与风险。契约条款一律引用 [可选功能二进制编译规格](./feature-optional-binary-spec.md) 的 R/B/F 编号（如 `R8`、`B6`、`F7`），本文不复述其内容。

---

## 0 结论速览

| # | 结论 | 依据 |
|---|---|---|
| 1 | clock 走**通道 A**（Cargo feature），不走 `src/tools/registry.ts` 的纯前端注册表 | 它有 6 条 Rust 命令 + 后端计时状态 + 到点通知 + 一个前端 chunk；R19 的「纯前端工具」判据不成立 |
| 2 | 新建**一个薄 crate** `assistant-clock`，不并入 `assistant-tools`，也不建「工具大类」聚合 crate | 命中 crate-architecture §5 判据②（状态机可脱 tauri 独立测试）；聚合会让 CI-2 的依赖断开证据互相污染 |
| 3 | 后端装配点 **12 处**，其中 11 处是「删一行 / 移一行」，只有 `cmd/mod.rs` 需要新增一条 cfg | 时钟当前所有 tauri 胶水集中在 `clock.rs` 尾部 ~52 行与 `cmd/clock.rs`，域逻辑本身零 tauri 依赖 |
| 4 | 后端 cfg 预算**恰好用满 3/3**（`cmd/mod.rs` 1 条 + `clock_plugin.rs` 2 条） | `FEATURE_CFG_LIMIT = 3`；因此 **`lib.rs`、`ticker.rs` 内一律不得再出现 `#[cfg(feature = "clock")]`** |
| 5 | 前端必须新建 `src/features/clock/module.ts`，但它只是 `{ id: "clock" }` 的**声明桩** | 门禁硬要求（`coreBoundary.test.ts` 校验 `src/features/<id>/module.ts` 存在 + `registry.ts` 有 `await import("./<id>/module")`）；时钟的 UI 仍留在 `src/tools/clock/` |
| 6 | `src/tools/framework/registry.ts` 里 clock 条目**必须**包在 `__CLOCK_ENABLED__` 守卫内 | 该文件不在 `CORE_SCAN_ROOTS`，源码门禁抓不到它；唯一证据是 F7 产物图断言（关闭态不得产出 `src/tools/clock/` chunk） |
| 7 | 现有 3 个关闭态脚本必须各扩一处，否则「关闭」没有证据 | F7 的根目前只有 `src/features/`；探针根只有 `src/features/<id>/`；而时钟的 UI 与文案都在 `src/tools/<id>/` |
| 8 | `vitest.config.ts` 必须补 `__CLOCK_ENABLED__` 的 define | 守卫在 `tools/framework/registry.ts` 的**模块顶层**求值；`src/tools/__tests__/categories.test.ts` 会 import 它，缺 define 直接 ReferenceError |
| 9 | `protocol/events.rs` 的 `CLOCK_FINISHED` 常量迁入特性边界，**不申请足迹豁免** | 契约 §8.1 明确「新特性不得复制 vault 的历史豁免」；该常量是自由字符串，`emit_broadcast` 接受 `&str`，迁移零成本 |
| 10 | 唯一的足迹豁免是 `cmd/mod.rs` 的 `#[cfg(feature = "clock")]` + `pub(crate) mod clock;` | cfg 谓词必须点名特性名，无法用无名词表达（与 vault 同形，属装配点豁免而非历史债务） |

---

## 1 判定与结论

### 1.1 通道判定（契约 §5.1）

| 通道 | 判据 | clock |
|---|---|---|
| A：Cargo feature | 有 Rust 命令 / 独立 crate / 后端状态 | **命中**：6 条命令、`ClockRuntime` 状态、到点通知、事件广播 |
| B：工具注册表 | 仅前端 chunk，无二进制 | 否：`clock_start` 等命令在关闭态必须不可调用 |
| C：设置注册表 | 仅设置项 | 否：时钟没有配置组（无 `clock_conf`） |

### 1.2 crate 判定（crate-architecture §5，逐条核对）

| # | 判据 | clock 实测 | 命中 |
|---|---|---|---|
| ① | 多消费者（≥2 上游 crate） | 只有 `assistant-app` | 否 |
| ② | 可脱 tauri 独立测试，且可测逻辑 >100 行 | `ClockRuntime` 纯状态机（`Mutex` + 原子 generation），无 `AppHandle`；现有集成测试 156 非空行全部只测状态机 | **是** |
| ③ | 编译隔离（重依赖 >3s） | 无重依赖（`chrono` 已在 workspace，`assistant-protocol` 是叶子） | 否 |
| ④ | 独立领域 + 体量 >300 行 | 域逻辑约 290 非空行，**低于阈值** | 否（临界） |
| ⑤ | 以上均不命中 | 被 ② 推翻 | 否 |

**结论**：新建 `src-tauri/crates/assistant-clock/`（一特性一薄 crate）。拆分的**首要依据是判据②**，不是④——所以本文不把「时钟以后会变大」当作理由。

### 1.3 为什么是「一特性一薄 crate」，不是「工具大类 crate」

| 方案 | 否决理由 |
|---|---|
| 并入 `assistant-tools` | 该 crate 是**前端工具契约**（工具 id / 分类 / 关键词）的 Rust 侧投影，不含 tauri 胶水；把需要 `AppHandle` 的计时器塞进去会让它反向依赖 tauri |
| 新建 `assistant-clock-tools` 聚合 crate（未来便签等同住） | ① 判据①在聚合后失真：N 个工具互不依赖却同 crate，关一个仍编译全部；② CI-2 的「依赖断开」要求**每个特性拥有自己的 `dep:` crate**，聚合后关 clock 时便签 crate 仍在 normal-edge 图里，断言必须按 crate 细分，脚本不支持；③ `assert-feature-shed.mjs` 在「禁用特性未解析出任何独占 `dep:` crate」时 `exit 2`（GG-3），聚合会让每个特性的可检查对象变成空集 |
| 不拆 crate，全部留在主 app（`clock = []`） | 判据② 已命中；且 `clock = []` 时单特性 lane 的依赖断开断言无对象 → `exit 2`，关闭态失去硬证据 |

---

## 2 现状事实（2026-11-21 核对）

| # | 事实 | 落点 |
|---|---|---|
| N1 | 域逻辑与 tauri 胶水**同文件**：`src-tauri/src/clock.rs` 344 行，1–293 行零 tauri 依赖（仅顶部 `use tauri::{AppHandle, Manager}` 供尾部使用），295–344 行是 `spawn_ticker` / `on_finished` / `send_notification` | `src-tauri/src/clock.rs` |
| N2 | 6 条命令：`clock_set_mode` / `clock_start` / `clock_pause` / `clock_resume` / `clock_reset` / `clock_snapshot`，全部 `State<'_, Arc<ClockRuntime>>`，无 `AppHandle`（`AppHandle` 只在 `clock_start` 里转发给 `spawn_ticker`） | `src-tauri/src/cmd/clock.rs`（54 行） |
| N3 | 命令在 **core 的 `ipc.rs`** 里注册（`cmd::clock_*` 6 行），不经任何插件边界 | `src-tauri/src/ipc.rs` |
| N4 | 状态在 **core 的 `setup.rs`** 里 `handle.manage(...)` | `src-tauri/src/setup.rs` |
| N5 | `lib.rs` 无条件 `#[doc(hidden)] pub mod clock;` | `src-tauri/src/lib.rs` |
| N6 | `cmd/mod.rs` 无条件 `mod clock;` + `pub use clock::*;` | `src-tauri/src/cmd/mod.rs` |
| N7 | 事件名 `CLOCK_FINISHED = "clock:finished"` 在 core 的 `event_name` 模块；`emit_broadcast(app, event: &str, payload)` 接受自由字符串 | `src-tauri/src/protocol/events.rs` |
| N8 | DTO 已在公共 crate（`ClockModeDto` / `ClockStatusDto` / `ClockSnapshotDto`），`From` 映射实现在 `clock.rs` 内 | `src-tauri/crates/assistant-protocol/src/clock.rs` |
| N9 | `build.rs` 按 `src/*_plugin.rs` **文件遍历**生成 `InlinedPlugin`（`DefaultPermissionRule::AllowAllCommands`），自动派生 `<id>:default` 与 `allow-<cmd>` 权限；`generate_handler![` 每文件恰好 1 处，命令前缀 `crate::cmd::<id>::`。仓库**没有** `src-tauri/permissions/` 目录，clock 同样不需要新建 | `src-tauri/build.rs` |
| N10 | 前端工具注册表硬编码 `defineTool({ id: "clock", ... component: lazyWithRecovery(() => import("../clock")) })`，并列入 `tools` 数组 | `src/tools/framework/registry.ts` |
| N11 | 前端时钟实现 9 文件 814 行，全在 `src/tools/clock/`，零跨 core 硬引用；mini 布局供 orb 窗口使用 | `src/tools/clock/` |
| N12 | `capabilities/default.json` **不含**任何 clock 权限（core 命令不经 ACL）；`capabilities/vault.json` 是唯一的特性 capability，`windows` 为 `["main", "orb"]` | `src-tauri/capabilities/` |
| N13 | 关闭态三件套现状：F7 只匹配 `src/features/<id>/`；字符串探针**已支持** `--prefix`，但源根硬编码为 `src/features/<id>/`、前缀默认 `settings`；`coreBoundary` 的关闭态用例调脚本时**未传** `--prefix`；依赖断开断言按清单驱动 | `scripts/feature-bundle-guard.mjs`、`scripts/assert-feature-absent.mjs`、`scripts/assert-feature-shed.mjs`、`src/features/__tests__/coreBoundary.test.ts` |
| N14 | 全仓对时钟的跨文件引用只有 3 处：`setup.rs:114`、`cmd/clock.rs:5`、`tests/clock_runtime.rs:1` | 全仓检索 |
| N15 | `coreBoundary` 的 `isFeatureBody()` 把 `src-tauri/src/cmd/<id>/`（**目录**）视为特性体，`src-tauri/src/cmd/<id>.rs` 不在白名单 → 单文件形态会被足迹扫描判红 | `coreBoundary.test.ts` |
| N16 | 文案键在 `src/i18n/locales/*.json` 里是**嵌套对象**，产物中不会出现 `tools.clock.` 这样的连续字符串；该连续串只出现在 `translateKey("tools.clock.*")` 的调用点 → 探针天然只命中时钟自身代码 | `src/i18n/index.ts`、`src/tools/clock/` |
| N17 | `vitest.config.ts` 只有 alias + `test` 段，**没有** `define`；`src/tools/__tests__/categories.test.ts` 直接 `import { tools } from "@/tools/framework/registry"` | `vitest.config.ts` |
| N18 | `feature-build.ps1` 只有 `-Task/-Features/-Without/-NoBundle/-Release/-Target/-ExtraArgs/-CargoArgs`，**无** `-Preset`；`config/presets.json` 不存在 | `scripts/feature-build.ps1`、`config/` |

---

## 3 装配点

### 3.1 后端 P1–P12

| # | 文件 | 动作 | 约束 |
|---|---|---|---|
| P1 | `src-tauri/Cargo.toml` | `[workspace] members` +`"crates/assistant-clock"`；`[workspace.dependencies]` +`assistant-clock = { path = "crates/assistant-clock" }`；`[dependencies]` +`assistant-clock = { workspace = true, optional = true }`；`[features]` +`clock = ["dep:assistant-clock"]`，`default = ["vault", "clock"]` | `dep:` 引用 + `optional = true` 必须成对（B2 的「无隐式 optional feature」）；`default` 是**默认构建集合**（R5），变体构建一律显式 `--features`（R25） |
| P2 | `src-tauri/crates/assistant-clock/Cargo.toml`（新建） | 依赖仅 `assistant-protocol` + `chrono`（`now_ms` 用 `chrono::Utc`）；`[features]` 段为空 | 不得依赖 `tauri` / `assistant-config` / `assistant-i18n` / `tauri-plugin-notification`，否则判据② 失效 |
| P3 | `src-tauri/crates/assistant-clock/src/lib.rs`（新建） | 迁入 `clock.rs` 的 1–293 行：`ClockMode` / `ClockStatus` / `ClockSnapshot` / `ClockInner` / `ClockRuntime` / `impl Default` / 三个时长常量 / `now_ms`（`chrono::Utc::now().timestamp_millis()`）/ DTO `From` 映射 | `use crate::protocol::` → `use assistant_protocol::`；删除顶部 `use tauri::{AppHandle, Manager}` 与 `use crate::protocol::events::{emit_broadcast, event_name}`（它们只服务 295–344 行）；保留 `use std::time::Duration`（仅状态机用） |
| P4 | `src-tauri/crates/assistant-clock/tests/clock_runtime.rs`（新建） | 由 `src-tauri/tests/clock_runtime.rs` 整体迁入，仅改首行 `use assistant_app::clock::{...}` → `use assistant_clock::{...}` | 这是判据② 的**可验证证据**：`cargo test -p assistant-clock` 不编译 tauri |
| P5 | `src-tauri/src/cmd/clock/mod.rs`（由 `cmd/clock.rs` 迁移） | 6 条命令体不变；`use crate::clock::{now_ms, spawn_ticker, ClockMode, ClockRuntime}` → `use assistant_clock::{now_ms, ClockMode, ClockRuntime}; use crate::cmd::clock::ticker::spawn_ticker;`；`use crate::protocol::{...}` → `use assistant_protocol::{...}` | 命令名与签名**逐字不变**（前端 `clock.service.ts` 与 ACL 派生都按名匹配） |
| P6 | `src-tauri/src/cmd/clock/ticker.rs`（新建） | 迁入 `clock.rs` 的 295–344 行（`spawn_ticker` / `on_finished` / `send_notification`）+ 常量 `pub(crate) const CLOCK_FINISHED: &str = "clock:finished";`（由 `protocol/events.rs` 迁入） | 本文件是**特性体内**，`#[cfg]` 一律不得出现（cfg 预算已用满，A2）；`assistant_i18n::t("clock.notification_*")`、`crate::window::show_or_create_main`、`assistant_config::constant::WINDOW_MAIN`、`emit_broadcast` 都在主 app 内，可直接用 |
| P7 | `src-tauri/src/clock_plugin.rs`（新建） | 一对互斥块（见 §3.1 末尾骨架）：启用块 = `Builder::new("clock")` + 6 条 `crate::cmd::clock::*` + `on_setup` 里 `manage(Arc::new(ClockRuntime::new()))`；关闭块 = 空插件 + 同签名空钩子 | `generate_handler![` 全文**恰好 1 处**（`build.rs` 硬校验）；命令前缀必须 `crate::cmd::clock::`（R7 / B12）；关闭块不得引用任何 `assistant_clock` 符号（R9） |
| P8 | `src-tauri/src/lib.rs` | 删 `#[doc(hidden)] pub mod clock;`，+1 行 `mod clock_plugin;`（**无 cfg**） | lib.rs 是装配点豁免文件；行数 34 → 35，仍 ≤ `LIB_RS_LINE_LIMIT = 40` |
| P9 | `src-tauri/src/cmd/mod.rs` | `mod clock;` → `#[cfg(feature = "clock")]` + `pub(crate) mod clock;`；删 `pub use clock::*;` | 逐字照抄 vault 的形态（`#[cfg(feature = "vault")] pub(crate) mod vault;` 且**没有** `pub use vault::*;`）；必须 `pub(crate)`，因为 `clock_plugin.rs` 在 `cmd` 之外要写 `crate::cmd::clock::*`；B6 要求该文件每特性**恰 1 条** cfg |
| P10 | `src-tauri/src/feature_mounts.rs` | `MOUNTS` +1 表项（`id: "clock"` / `plugin: crate::clock_plugin::init` / `on_setup` / `on_exit`） | 文件保持 cfg-free、单一 `MOUNTS`、单一 `mounts()`（R12 / B7） |
| P11 | `src-tauri/src/setup.rs` | 删 `handle.manage(std::sync::Arc::new(crate::clock::ClockRuntime::new()));` | `setup.rs` 在 `TRAVERSAL_FILES` 内，删净后不得再出现 `clock` 子串（B3 / F6）；manage 改由 `on_setup` 钩子承担，时序安全（`setup.rs` 的 `on_setup` 循环在 IPC 开放之前） |
| P12 | `src-tauri/src/ipc.rs`、`src-tauri/src/protocol/events.rs`、`src-tauri/capabilities/clock.json` | `ipc.rs` 删 6 行 `cmd::clock_*`（第 26–31 行）；`events.rs` 删 `pub const CLOCK_FINISHED`（迁 P6）；新建 `clock.json`（`identifier: "clock"`、`windows: ["main", "orb"]`、`permissions: ["clock:default"]`） | `ipc.rs` / `events.rs` 的 `clock` 子串必须清零（F6 子串比对）；`ipc.rs` 与插件注册表**不得重叠**（A6：同一命令不能既在宿主又在插件注册）；`clock.json` 的 `permissions` 只能 `<id>:*`（B10），`windows` ⊆ `tauri.conf.json` 已声明标签（B10）；**关闭态保留该文件与 ACL 惰性声明**（R24） |

**P7 骨架**（与 `vault_plugin.rs` 同形，只替换 id 与钩子体）：

```rust
// 参考: feature-mount-traversal-spec.md（已归档）§3 — 可选性只体现一次

use tauri::plugin::TauriPlugin;

#[cfg(feature = "clock")]
mod mounted {
    use super::TauriPlugin;
    use std::sync::Arc;

    use tauri::Manager;

    pub(crate) fn init() -> TauriPlugin<tauri::Wry> {
        tauri::plugin::Builder::<tauri::Wry>::new("clock")
            .invoke_handler(tauri::generate_handler![
                crate::cmd::clock::clock_set_mode,
                crate::cmd::clock::clock_start,
                crate::cmd::clock::clock_pause,
                crate::cmd::clock::clock_resume,
                crate::cmd::clock::clock_reset,
                crate::cmd::clock::clock_snapshot,
            ])
            .build()
    }

    pub(crate) fn on_setup(app: &tauri::AppHandle) -> Result<(), String> {
        app.manage(Arc::new(assistant_clock::ClockRuntime::new()));
        Ok(())
    }

    pub(crate) fn on_exit(_app: &tauri::AppHandle) {}
}

#[cfg(not(feature = "clock"))]
mod mounted {
    use super::TauriPlugin;

    pub(crate) fn init() -> TauriPlugin<tauri::Wry> {
        tauri::plugin::Builder::<tauri::Wry>::new("clock").build()
    }

    pub(crate) fn on_setup(_app: &tauri::AppHandle) -> Result<(), String> {
        Ok(())
    }

    pub(crate) fn on_exit(_app: &tauri::AppHandle) {}
}

pub(crate) use mounted::{init, on_exit, on_setup};
```

> 注意：顶层只保留 `use tauri::plugin::TauriPlugin;`（两态都用），其余 `use` 全部下沉到模块内——否则关闭态出现 unused import 警告，而 AC 把「新增警告」也判为不通过。

### 3.2 前端 P13–P18

| # | 文件 | 动作 | 约束 |
|---|---|---|---|
| P13 | `config/features.json` | +1 条目 `"clock": { "default": true }`（只写 `default`） | 单一真源：`vite.config.ts`、`feature-build.ps1`、`coreBoundary.test.ts`、`assembly_contract.rs` 四处自动派生（R1–R6），不得手写 `cargoFeature` / `env` |
| P14 | `src/vite-env.d.ts` | +1 行 `declare const __CLOCK_ENABLED__: boolean;` | 门禁正则要求 `declare const __CLOCK_ENABLED__\s*:\s*boolean` |
| P15 | `src/features/clock/module.ts`（新建） | `import type { FeatureModule } from "../types";` + `export const clockModule: FeatureModule = { id: "clock" };` | 声明桩：时钟不带设置页/路由/侧栏/配置槽，`FeatureModule` 全部字段可选，`{ id }` 即合法；**不得** import `@/types/protocol.contract`（F 断言：特性自有契约不回流主契约） |
| P16 | `src/features/registry.ts` | 在 `if (__VAULT_ENABLED__)` 块后插入：`if (__CLOCK_ENABLED__) { const { clockModule } = await import("./clock/module"); modules.push(clockModule); }` | 只允许「编译期字面量守卫 + 动态 import」（R14）；必须逐字包含 `await import("./clock/module")` |
| P17 | `src/tools/framework/registry.ts` | 把 `const clock = defineTool({...})` 改为 `const clockTools: ToolDefinition[] = __CLOCK_ENABLED__ ? [defineTool({...})] : [];`；`tools` 数组里 `clock,` → `...clockTools` | 守卫必须在**模块顶层表达式**里，Rollup 才能把 `false ? [...] : []` 的死分支连同 `() => import("../clock")` 一起折叠；否则关闭态仍产出 `src/tools/clock/` chunk → F7 失败。`toolsMap` 由 `tools` 派生，无需改 |
| P18 | `vitest.config.ts`（+ 建议抽 `scripts/feature-manifest.mjs`） | 增 `define: { __CLOCK_ENABLED__: ... }`，取值与 `vite.config.ts` 同一派生规则（读 `config/features.json`） | 硬前置：守卫在模块顶层求值，`src/tools/__tests__/categories.test.ts` import `tools` → 缺 define 时 `npm run test` 直接 ReferenceError。建议把 `vite.config.ts` 现有的「清单 → defines / disabledIds」解析抽成 `scripts/feature-manifest.mjs`，两处 config 共用，避免第二份派生规则（R3/R4 的同类风险） |

> P15 与 P17 的分工：`src/features/clock/module.ts` 满足**特性边界契约**（每个特性一个前端模块），`src/tools/clock/**` 是**工具实现**。二者不合并——合并会把 814 行 UI 搬进特性目录，违背 R19 的「工具实现归工具注册表」边界，也让后续便签工具无法复用同一模板。

### 3.3 门禁与脚本扩展 G1–G4

| # | 落点 | 动作 | 失败含义 |
|---|---|---|---|
| G1 | `scripts/feature-bundle-guard.mjs` + `vite.config.ts` + `scripts/feature-bundle-guard.test.mjs` | `findFeatureModules(moduleIds, featureIds, roots = ["src/features/"])` 增加第三参（默认值保持现有 2 参调用不变）；`featureBundleGuard(featureIds, roots = ["src/features/"])`；`vite.config.ts` 传 `["src/features/", "src/tools/"]`；测试补一条「产物含 `src/tools/alpha/ClockLayout.tsx` 时报告违规」 | 关闭态仍产出 `src/tools/<id>/` 模块（P17 守卫写错或被人改回静态引用） |
| G2 | `scripts/assert-feature-absent.mjs` | `probeLiterals()` 的源根改为**候选集** `[src/features/<id>, src/tools/<id>]`：只扫存在的目录并合并结果；两处都空才 `exit 2`；输出打印命中的源根。`--prefix` 参数**已存在**，无需新增 | 探针只能从 `src/features/<id>/` 提取 → 时钟 0 条探针 → 关闭态无字符串证据（GG-3 的空扫描规则保持不变） |
| G3 | `src/features/__tests__/coreBoundary.test.ts` | ① `FEATURE_FOOTPRINT_ALLOWLIST` +1 条：`cmd/mod.rs`，`reason` = 「装配点：cfg 谓词必须点名特性名」，`markers` = `['#[cfg(feature = "clock")]', "mod clock;"]`；② 新增 `FEATURE_PROBE_PREFIX = { clock: "tools" }`（默认 `"settings"`），关闭态 `spawnSync` 的参数数组补 `"--prefix", 该值` | ① 时钟在 core 出现未豁免引用（尤其 `setup.rs` / `ipc.rs` / `events.rs` 残留）；② 关闭态探针用错前缀 → `settings.clock.` 提取 0 条 → 探针步骤失败。另注意豁免是**行级**且会校验 markers 存活（死豁免必须删） |
| G4 | `.github/workflows/ci.yml` | 前端 lane：把现有 slim 步改为 `VAULT_ENABLED=0 CLOCK_ENABLED=0 npm run build`，紧随其后加一条 `node scripts/assert-feature-absent.mjs --feature clock --prefix tools --dist dist`（与既有 vault 探针同一 dist，不新增构建）；后端 Linux lane：在既有 `-p assistant-app --no-default-features`（两特性全关）之后追加 `--features vault` / `--features clock` 两组 `cargo check` + `assembly_contract` + `assert-feature-shed.mjs --features <id>` | 单特性独立性只在本地可用（CI-1）；「关 clock」变成只跑默认态的错误假设；slim 步若漏掉 `CLOCK_ENABLED=0`，产物里仍有时钟 chunk → F7 与探针都会红，但红的原因会被误读为「守卫失效」 |

> G4 的两种 lane 是**互补证据**：`--features vault` 证明关 clock 时 `assistant-clock` 不在 normal-edge 图（`disabledDepCrates` 的差集恰好是 `assistant-clock`）；`--features clock` 证明关 vault 时三个 vault crate 消失。无参调用（两特性全关）继续保留为 slim 态。

### 3.4 N=2 的连带义务 P19

`config/features.json` 从 1 条变 2 条即触发契约 `R27`（「N≥2 时引入 `config/presets.json`」），这是本规格**唯一超出「时钟迁移」本身的动作**：

| # | 文件 | 动作 | 约束 |
|---|---|---|---|
| P19 | `config/presets.json`（新建）+ `scripts/feature-build.ps1` | 新建四条 preset：`full = ["vault","clock"]`、`slim = []`、`vault-only = ["vault"]`、`clock-only = ["clock"]`；`feature-build.ps1` 增 `-Preset`（与 `-Features` / `-Without` 互斥），解析后走现有 `Resolve-Features` | 每条 `features` ⊆ 清单键集合、`full` = 清单全集、`slim` = 空、字段只允许 `features`（§7.2）；校验落在脚本内（GG-1 式：解析 0 条 → 失败） |

> 若审查决定本次不落 P19，必须在本文登记偏差：改用 `-Without vault` / `-Without clock` 穷举单特性变体（N=2 时可穷举），并把「`config/presets.json` 延后至 N≥3」写入 [可选功能二进制编译规格](./feature-optional-binary-spec.md) §7.2。**默认按 R27 落地**，不擅自偏离契约。

---

## 4 实施顺序 S1–S8

| 步骤 | 动作 | 自检 |
|---|---|---|
| S1 | 建 crate（P1–P4）：迁域逻辑与测试，`cargo test -p assistant-clock` 先绿 | 判据② 可验证；此时主 app 仍用旧 `clock.rs`，两套并存但互不引用 |
| S2 | 后端切边界（P5–P7）：`cmd/clock/` 两文件 + `clock_plugin.rs` | `cargo check` 两态均过；`build.rs` 已自动派生出 `clock` 插件与 6 条命令 |
| S3 | 后端装配点收口（P8–P12）：`lib.rs` / `cmd/mod.rs` / `feature_mounts.rs` / `setup.rs` / `ipc.rs` / `events.rs` / `capabilities/clock.json`，删旧 `src-tauri/src/clock.rs` 与旧 `src-tauri/tests/clock_runtime.rs` | 全仓 `clock` 引用只剩 `cmd/mod.rs`（豁免行）与特性体内 |
| S4 | 清单与前端（P13–P17） | `npm run typecheck`；此时先不跑 `npm run test`（P18 未做会红） |
| S5 | 测试 define（P18） | `npm run test` 全绿；`categories.test.ts` 的 23 个工具断言不变（默认态 `clock` 仍在） |
| S6 | 门禁扩展（G1–G3） | `npm run test`（含 `coreBoundary` 与 `feature-bundle-guard` 单测） |
| S7 | N=2 连带（P19） | `.\scripts\feature-build.ps1 -Task check -Preset clock-only` |
| S8 | CI 扩展（G4） | 本地按 CI 命令逐条手跑一遍（见 §5.2） |

每步可独立回滚；S1–S3 不改变时钟运行期行为（命令名、DTO、事件名逐字不变）。

---

## 5 门禁映射与验收

### 5.1 门禁映射

| 门禁 | 覆盖本规格的哪些点 | 说明 |
|---|---|---|
| `A1` `ipc.rs` 无任何 cfg | P12 | 删 6 行后仍保持纯核心注册表 |
| `A2` cfg 只能落在 `cmd/mod.rs` 或 `<id>_plugin.rs` | P9、P6 | `cmd/clock/**`、`ticker.rs`、`lib.rs` 一律不得出现 `#[cfg(feature = "clock")]` |
| `A3` `lib.rs` 装配层 | P8 | 行数 ≤ 40 且不得内联 `generate_handler!` |
| `A4` / `A7` ACL ↔ 注册双向一致 | P7、P12 | build.rs 派生的 `allow-clock-*` 必须与 6 条注册命令逐条相等；`ACL_PARSER_FLOOR = 60` 为全仓下限，加 6 条只会抬高 |
| `A5` 解析下限 | P7 | clock 插件必须解析出 ≥1 条命令 |
| `A6` 宿主 / 插件分区不重叠 | P12 | `ipc.rs` 与插件注册表命令名交集必须为空 |
| `A8` conf applier 与配置组一致 | — | 时钟无配置组，不受影响（这也是「通道 C 不成立」的旁证） |
| `B2` 清单扩展 | P1、P13 | 清单特性拥有的 optional 依赖必须 `dep:` 引用；`cargoFeature` / `env` 省略即派生 |
| `B3` 边界文件不点名特性 | P8–P12 | `setup.rs` / `ipc.rs` / `plugins.rs` / `lifecycle.rs` / `window.rs` / `tray.rs` / `cmd/config.rs` 的字符串字面量不得出现 `clock` |
| `B4` / `B5` 互斥块与同签名 | P7 | 两态 `init` / `on_setup` / `on_exit` 签名逐字一致 |
| `B6` 单文件 cfg 预算 | P9 | `cmd/mod.rs` 每特性恰 1 条；**总量 3/3**（`cmd/mod.rs` 1 + `clock_plugin.rs` 2） |
| `B7` 挂载表恒定 | P10 | cfg-free、单一 `MOUNTS`、单一 `mounts()` |
| `B8` / `B10` capability | P12 | `permissions` 只 `clock:default`；`windows` ⊆ `tauri.conf.json`；`default.json` 不含 `clock:*` |
| `B11` 单 `tauri.conf.json` | P2 | 新 crate 是 library，无 conf、无 `generate_context!` |
| `B12` 命令前缀 | P7 | 6 条全部 `crate::cmd::clock::` |
| `F1` 前端清单/装配 | P13–P16 | `module.ts` 存在、`vite-env.d.ts` 声明、`registry.ts` 消费 `__CLOCK_ENABLED__`、Cargo `default` 含 `clock`、`lib.rs` 有 `mod clock_plugin`、挂载表有 `id: "clock"`、`capabilities/clock.json` 有 `clock:default` |
| `F6` 足迹冻结 | P11、P12、G3 | `setup.rs` / `ipc.rs` / `events.rs` 的 `clock` 子串清零；`cmd/mod.rs` 走唯一豁免 |
| `F7` 产物图断言 | P17、G1 | 关闭态不得出现 `src/features/clock/` 与 `src/tools/clock/` 模块 |
| 字符串探针 | G2、G3 | `tools.clock.*` 文案探针；源根 = `src/features/clock` + `src/tools/clock`，前缀 `tools`。文案本体在 `src/i18n/locales/*.json` 是嵌套对象（N16），不产生连续串，故不会误报 |
| `CI-2` 依赖断开 | P1、P2、G4 | `--features vault` 下 `assistant-clock` 不在 normal-edge 图 |
| `GG-1` / `GG-3` | G2、P19 | 清单/preset 解析 0 条、探针 0 条、独占 crate 0 个 → 一律 `exit 2` |

### 5.2 验收命令（全部必须绿）

**顺序有讲究**：`assert-feature-absent` 只能在**该特性关闭态**的产物上跑（`coreBoundary` 关闭态用例的报错原文就是「先跑 `feature-build.ps1 -Task build -Without <id>` 再验」）。`dist/` 只能承载一个变体，因此「构建 → 断言」必须成对，不能先 full 构建再断言 clock 缺席。

仓库根，第 1 组（默认态 = full，产物含 clock）：

```powershell
npm run typecheck
npm run lint
npm run test
npm run contract:check
npm run build
.\scripts\feature-build.ps1 -Task check
.\scripts\feature-build.ps1 -Task test
```

第 2 组（clock 关闭态 = `vault-only`，构建后立即断言）：

```powershell
.\scripts\feature-build.ps1 -Task build -Preset vault-only
node scripts/assert-feature-absent.mjs --feature clock --prefix tools --dist dist
```

第 3 组（clock 开启态 = `clock-only`，同时是 vault 关闭态）：

```powershell
.\scripts\feature-build.ps1 -Task build -Preset clock-only
node scripts/assert-feature-absent.mjs --feature vault --prefix settings --dist dist
```

`cwd = src-tauri`（与前端产物无关，可在任意时刻跑）：

```powershell
cargo test -p assistant-clock
cargo check -p assistant-app --no-default-features --features vault
cargo check -p assistant-app --no-default-features --features clock
cargo test -p assistant-app --no-default-features --features vault --test assembly_contract
cargo test -p assistant-app --no-default-features --features clock --test assembly_contract
node ../scripts/assert-feature-shed.mjs --features vault
node ../scripts/assert-feature-shed.mjs --features clock
```

判定口径：`clock` 关闭态二进制内**没有**时钟命令、没有 ticker 线程、没有 `clock:finished` 事件、没有通知文案；`clock` 开启态行为与改造前逐字一致（`npm run test` 的时钟用例、`cargo test -p assistant-clock` 全绿即视为一致）。

---

## 6 残余与风险

| # | 项 | 处置 |
|---|---|---|
| R1 | 关闭态仍留 `capabilities/clock.json` 与 ACL 惰性声明 | 契约 R24 已记账：静态文件与 ACL 同生共死，不构成「特性仍被编译」 |
| R2 | 后端常量迁入特性体后，前端 `src/types/events.ts` 的 `ClockFinished = "clock:finished"` 仍在 core | 该文件不在 `CORE_SCAN_ROOTS`；事件名是 wire 数据不是代码知识，且此前就是前后端各持一份。若将来把 `src/types` 纳入扫描根，需要第二条豁免（不推荐，成本大于收益） |
| R3 | `src/tools/framework/registry.ts` 不在 `CORE_SCAN_ROOTS`，其守卫只能靠 F7 产物证据兜底 | 这是**有意**的边界：工具注册表是工具域的 core，把 21 个工具 id 纳入特性名扫描会立刻误报。证据链：F7（chunk 必须消失）+ 探针（文案必须消失） |
| R4 | 时钟域逻辑约 290 非空行，略低于判据④ 的 300 | 拆 crate 的依据是判据②（可脱 tauri 独立测试），不是④；本文不把「将来会长大」当理由 |
| R5 | `vitest` 缺 `__CLOCK_ENABLED__` define 会在模块加载期 ReferenceError | P18 是**硬前置**，与 P17 同一提交落地；单测不得用 `vi.stubGlobal` 代替（守卫是编译期标识符，不是运行时全局） |
| R6 | 关闭态下若用户上次停留在时钟工具页 | 已核：工具选择**无持久化**（全仓无 `activeToolId` / `selectedTool` 存储），关闭态工具列表天然不含该 id，无需回落逻辑 |
| R7 | cfg 预算已用满 3/3 | 后续任何针对 clock 的改动**不得**在 `lib.rs` / `ticker.rs` / `cmd/clock/**` 新增 `#[cfg]`；需要新 cfg 时必须先说明为何能收进 P7 / P9 的既有两条 |
| R8 | `cmd/clock.rs` 单文件形态会被足迹扫描判红（N15） | P5 必须改成目录 `cmd/clock/mod.rs` + `cmd/clock/ticker.rs`；`clock_plugin.rs` 必须放在 `src/` 顶层（`isFeatureBody` 只认 `src/<id>_plugin.rs`） |
| R9 | `FEATURE_ALIASES` 是否需要登记「时钟」 | 已核：`src/settings`、`src/features`（除新模块本体）、`src-tauri/src`（除特性体与豁免行）均无「时钟」字样 → 不登记。若将来 core 出现中文产品名，必须补 `clock: ["时钟"]`，否则中文泄漏绕过扫描 |

---

## 7 范围外

- **不改时钟行为**：命令名、DTO 字段、时长上下限、通知文案、mini 布局均逐字保留（`contract:gen` 无需重跑）。
- **不做运行时开关**：feature 只在编译期生效，二进制内没有「关掉时钟」的运行时分支。
- **不做多产物打包**：变体产物共存安装属契约 D3，本文不涉。
- **不动 `assistant-tools`**：它是前端工具契约 crate，与本次无关。
- **不为便签等后续工具预建目录**：模板已固化在 §3，第二个工具照做即可，不预留空壳。
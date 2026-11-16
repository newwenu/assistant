# 可选功能二进制编译规格

| 项 | 值 |
|---|---|
| 状态 | 规格（定义终态与判定，不含实施记录） |
| 日期 | 2026-11-16 |
| 关联 | 分析《FEATURE-OPTIONAL-BINARY-ANALYSIS.md》；前置规格《feature-mount-traversal-spec.md》；L1 基线《vault-optional-build.md》；成本模型分析《FEATURE-ZERO-COST-MOUNT-ANALYSIS.md》；拆分判据 `docs/principle/CODE-SPLIT-PRINCIPLE.md` |
| 影响面 | 配置 1、后端 3、前端 3、capabilities 2、门禁 2、脚本 1、CI 1；不新增第三方依赖 |

定位：本规格把「可选功能在编译期整体移出二进制」的终态固定下来。分析文档回答「哪种引入方式合理」，前置规格回答「挂载如何遍历化」，本规格回答**「什么算真的关掉、由什么证据证明、边界在哪里」**。

三条自由度先划清，后文不再混谈：

| 自由度 | 含义 | 表达层级 | 本规格处置 |
|---|---|---|---|
| **D1 功能集合** | 某个功能在不在这个二进制里 | 编译期 Cargo feature + `config/features.json` | 保留现有机制，补关闭态证据与门禁 |
| **D2 形态** | 同一功能以什么入口呈现（独立窗口、面板、布局） | 运行期窗口/入口注册 + 运行期配置 | 只定边界门禁；本规格不实现窗口提供者 |
| **D3 产物** | 出几个可执行文件、几个安装包 | package 级（独立 crate / conf / build.rs / capabilities） | 只定边界门禁与触发条件；本规格不实施多产物 |

**一句话**：D1 已经有一套咬合良好的机制，缺的不是新框架，而是**把「关闭」从源码约定升级为 CI 常驻证据**；D2、D3 若塞进 feature 体系会直接撞现有门禁，应分别留在运行期与 package 级。

---

## 0 结论速览

| # | 结论 | 判定 | 处置 |
|---|---|---|---|
| 1 | 现有 D1 机制（清单 + 插件边界 + 挂载表 + A/B/F 门禁）方向正确 | **保留** | 不换框架，只补证据 |
| 2 | 「关闭态」目前只在本地人工跑过，没有 CI 常驻 | **最大缺口** | 新增 CI 关闭态 lane |
| 3 | `cargo tree -i <crate>` 的退出码 + dev 依赖语义不适合做断开证明 | **不可靠** | 改为 normal-edge 依赖图成员判定 |
| 4 | 前端关闭态只做字符串探针，证明力弱于产物图 | **需加强** | 加构建期 bundle graph 断言，字符串探针降为补充 |
| 5 | 清单里的 `env` / `cargoFeature` 是 `id` 的派生值 | **可简化** | 改为可省略 + 派生，旧值保留必须逐字相等 |
| 6 | 多个门禁在清单为空或解析器失效时会空转通过 | **需加固** | 统一 guard-the-guard：空输入即失败 |
| 7 | capability 只断言「存在且含 `<id>:default`」，不限制权限面 | **G-2 仍开** | 加最小权限 + 窗口标签断言 |
| 8 | D2/D3 的判定没有反向门禁，可能被后人用新 feature 绕过 | **需固化** | core 边界文件不得出现特性名；单 package 单产物 |
| 9 | 前端 loader 代码生成、`import.meta.glob`、运行期 flag 表、`inventory`/`linkme` | **维持否决** | 生成器仅在 N≥3 时重评 |
| 10 | 预置组合（preset） | **N≥2 才有意义** | 先以隐式 `full` / `slim` 两态跑 CI |

---

## 1 目标与判定口径

### 1.1 可判定目标

| 编号 | 目标 | 判定方式 |
|---|---|---|
| G1 | 单一真源：功能集合只由 `config/features.json` 定义，派生值不手写 | B2 扩展 + F1 扩展；清单条目不得有未知键 |
| G2 | 关闭 = 依赖闭包真的不在图里，不是「编了但不注册」 | `scripts/assert-feature-shed.mjs` 扫 normal-edge 图 |
| G3 | 关闭 = 前端产物图里没有任何该特性模块 | Vite `generateBundle` 断言（F7） |
| G4 | 开启态行为零变化：命令集合、配置读写、退出清理、启动播种不因本规格改变 | 现有 AC8 口径 + 默认态全量测试 |
| G5 | 门禁随特性数 N 增长不膨胀：只遍历清单/磁盘，不逐特性写死断言 | 新增门禁全部按集合运算实现 |
| G6 | 受支持组合可枚举且被 CI 实跑；不做 2^N | 隐式 `full` / `slim`；N≥2 时 `config/presets.json` |
| G7 | D2 形态、D3 产物不能借 D1 的 feature 体系偷渡 | 边界文件反向断言 + 单 package 单产物断言 |

### 1.2 非目标

- 不做运行期功能开关、灰度、按用户授权裁剪。编译期裁剪与运行期开关是两种东西，不得互相替代。
- 不做前端 loader 生成器、`vite-env.d.ts` 生成器。N=1 下收益为负，触发判据见 §7.3。
- 不把 `tauri.conf.json` 的静态 `windows` / 托盘菜单 / `assetProtocol` 改造成运行期注册。它们是 D2/D3 的前置迁移，不在本规格实施。
- 不改任何业务逻辑、命令签名、DTO 结构、wire 字段、配置文件格式。
- 不新增第三方依赖（Cargo 与 npm 均不新增）。

### 1.3 编号空间

| 前缀 | 含义 |
|---|---|
| G | 目标 |
| R | 终态规则 |
| S | 改造步骤（一次性动作） |
| B | 后端门禁（继续既有 B1–B9 编号） |
| F | 前端门禁（继续既有 F1–F6 编号） |
| CI | CI 常驻检查 |
| GG | guard-the-guard（空输入/解析失效即失败） |
| AC | 验收条目 |

既有 A1–A8、B1–B9、F1–F6 的定义不重写；本规格只声明哪些保持、哪些扩展。

---

## 2 事实基线（2026-11-16 核对）

### 2.1 声明与装配

| 项 | 当前事实 | 证据 |
|---|---|---|
| 特性清单 | 仅 `vault` 一条，`default: true`，`cargoFeature: "vault"`，`env: "VAULT_ENABLED"` | `config/features.json` |
| Cargo 镜像 | `default = ["vault"]`；`vault = ["dep:assistant-vault", "dep:assistant-sync", "dep:assistant-security"]` | `src-tauri/Cargo.toml` `[features]` |
| 挂载表 | 1 张 `MOUNTS`、1 个 `mounts()` 读入口；表项含 `id` + `plugin` + `on_setup` + `on_exit` | `src-tauri/src/feature_mounts.rs` |
| 插件边界 | `vault_plugin.rs` 一对互斥 cfg 块；启用块含 1 个 `generate_handler!` 注册块，关闭块为无命令空插件 | `src-tauri/src/vault_plugin.rs` |
| 后端 cfg | 全后端 3 行：`vault_plugin.rs` 2 + `cmd/mod.rs` 1，恰为 `FEATURE_CFG_LIMIT = 3` | `src-tauri/tests/assembly_contract.rs` |
| 装配点 | 6 条：清单、`vite-env.d.ts`、`registry.ts`、`Cargo.toml`、`feature_mounts.rs`、`lib.rs` | `src/features/__tests__/coreBoundary.test.ts` |
| 遍历化调用方 | 5 条：`build.rs`、`capabilities/default.json`、`plugins.rs`、`lifecycle.rs`、`setup.rs`；断言文本内不出现特性名 | 同上 `TRAVERSAL_FILES` |
| 足迹豁免 | 2 条：`src-tauri/src/cmd/mod.rs`、`src-tauri/src/protocol/events.rs` | 同上 `FEATURE_FOOTPRINT_ALLOWLIST` |
| 前端 loader | `registry.ts` 3 行守卫 + `vite-env.d.ts` 1 行 declare | `src/features/registry.ts`、`src/vite-env.d.ts` |

> 命令条数是时点快照，不是契约。本规格与门禁**不得**把具体命令条数写成断言；只允许「集合相等」「至少 1 条」「解析总数不低于下限」这类结构判据。

### 2.2 静态声明与 D2/D3 阻塞点

| 项 | 当前事实 | 对可选化的含义 |
|---|---|---|
| 静态窗口 | `tauri.conf.json` 的 `windows` 含 `main`、`orb` | 静态数组无法按 feature 裁剪；orb 不能低成本 feature 化 |
| assetProtocol | `security.assetProtocol.scope.allow` 含 `$APPDATA/mascot-assets/**` | mascot 不能低成本 feature 化 |
| 托盘菜单 | `tray.rs` 两处硬编码 `show_orb` 等条目 | orb 的形态入口在 core，不能随 feature 消失 |
| orb 启动路径 | `setup.rs` 无条件调用 `position_orb_window` / `show_orb_window`；`window.rs` 引用 `assistant_orb::placement`；`App.tsx` 的 `viewMap` 含 `OrbWindow` | orb 与 mascot 应作为一个簇，先迁运行期提供者，再谈 feature |
| capability | `default.json` 授 `main`/`orb` 的 core 与常驻插件权限；`vault.json` 只授 `vault:default` | 拆分已做；权限面与窗口标签仍无断言 |
| 单产物 | `lib.rs` 只有一处 `generate_context!("tauri.conf.json")`；`src-tauri` 只有一份 `tauri.conf.json` | 当前无多产物；应加断言防止就地加第二份 |

### 2.3 门禁与 CI

| 项 | 当前事实 | 状态 |
|---|---|---|
| 后端门禁 | A1–A8 + B1–B9 | 已落地 |
| 前端门禁 | F1–F6（含产物探针 F-产物探针） | 已落地 |
| 关闭态编译 | 本地 `.\scripts\feature-build.ps1 -Task check -Without vault` 通过 | 人工证据 |
| 关闭态单测 | 本地 `assembly_contract` 两种组合通过 | 人工证据 |
| 关闭态产物 | 本地 74 个产物 / 18 条探针无泄漏 | 人工证据 |
| CI | 只跑默认特性：`cargo test --workspace --all-targets`、前端 typecheck/lint/test；**无 `--no-default-features` lane，无 `npm run build`** | **缺口** |
| 依赖断开 | 无脚本；只有人工 `cargo tree --no-default-features -i assistant-vault` | **缺口** |
| capability 权限面 | B8 只断言存在与授予，不限制可授权限、不校验窗口标签 | **缺口** |
| 门禁空转 | A5/A7 有下限；F 组与部分解析器无空输入失败保护 | **缺口** |

> 关闭态编译/单测/产物的「人工证据」来自《vault-optional-build.md》与《feature-mount-traversal-spec.md》的实施记录；本次只核对代码结构，未重跑关闭态构建。

### 2.4 与分析文档的偏差

- 分析文档 §1.4 已记录「装配点 8 → 6、足迹 7 → 2、cfg 5 → 3」；本次核对与代码一致。
- 分析文档中的命令条数已被后续改动推动；这正说明**条数不能作为门禁常量**，本规格只保留结构判据。
- 分析文档 §8 的 G-3、G-6 已关闭；G-1、G-2、G-4、G-5、G-7 仍开。本规格逐条给出处置：G-1 → CI-1/CI-2，G-2 → B10，G-4 → 维持并记录，G-5 → §7.2，G-7 → F7 + 保留字符串探针。

---

## 3 设计评审：保留什么、优化什么、否决什么

### 3.1 保留项（不因「优化」而改动）

| 项 | 保留理由 |
|---|---|
| 清单 + Cargo feature 双声明 | Cargo 语义要求静态声明；清单是跨前端/脚本/门禁的唯一真源，二者由 B2 做子集断言 |
| 一对互斥 cfg 块 + 块外 `pub(crate) use` | 调用方看到的符号恒定，挂载表 cfg-free；这是遍历化能成立的前提 |
| 挂载表 4 字段（`id` + 3 函数） | `on_setup` / `on_exit` 显式化启动顺序与退出清理；降为 2 字段会把语义藏进框架隐式钩子，读性变差 |
| `cmd/mod.rs` 保留 1 行 `mod` 声明 | 命令路径前缀 `crate::cmd::<id>::` 稳定；下沉到插件边界要改 `build.rs`、A6/A7、前端目录识别，收益仅 2 行 |
| `build.rs` 无条件声明所有 `*_plugin.rs` | 关闭态 `capabilities/<id>.json` 仍引用 `<id>:default`，若按 `CARGO_FEATURE_*` 门控会构建失败；ACL 惰性声明是刻意取舍 |
| 前端字面量守卫 `if (__X__)` + 动态 import | 常量折叠才能让关闭态 chunk 不产出；运行期 flag 表会破坏真删除 |
| capability 按特性拆文件 | 成本从 core 装配点降为特性本体，且窗口作用域可按特性声明 |
| 生成器/`import.meta.glob`/`inventory`/`linkme` 否决 | 已在成本模型分析中逐条论证；N=1 下收益为负或破坏真删除 |

### 3.2 采纳的优化

| # | 优化 | 现状问题 | 终态 | 依据 |
|---|---|---|---|---|
| O1 | 关闭态 CI lane | 关闭态只有本地人工证据，机制可静默腐烂 | CI 常驻跑 `full` 与 `slim` 两态（§7.1） | 参考项目 openhuman 把「产物特性集合」与「贡献者默认集合」分开，并用 CI lane 实跑产品集合 |
| O2 | 依赖断开改为图成员判定 | `cargo tree -i` 在 crate 不存在时退出非零，且 `-e` 只过滤显示边、不限制 `-i` 查找集，dev 依赖会造成假阳性 | `scripts/assert-feature-shed.mjs` 解析 `cargo tree -p assistant-app -e normal --prefix none` 的包名集合，断言禁用特性的 `dep:` crate 不在其中 | 参考项目 openhuman 的 `assert-shed.sh` 明确记录了这两个失败模式 |
| O3 | 前端产物图断言 | 字符串探针依赖「特性目录里恰好有可扫描字面量」；文案外移就失效 | Vite `generateBundle` 遍历输出 chunk 的 module id，禁用特性目录下任何模块出现即失败 | 构建期图断言比字符串扫描更接近「真的没有编译进去」 |
| O4 | 清单派生值 | `env` / `cargoFeature` 与 `id` 重复，靠门禁防止漂移 | 二者可省略；省略时按 `id` 派生；显式写出时必须逐字相等 | 单一真源原则；减少手写面 |
| O5 | 特性 id 语法 | 连字符 id 会派生非法环境变量名，只能在门禁报错时才发现 | 明确 `^[a-z][a-z0-9_]*$`；环境变量为 `ID.toUpperCase() + "_ENABLED"` | 分析文档 E1 的直接结论 |
| O6 | capability 最小权限 | 特性 capability 可被加进任意 core 权限，且 `windows` 可写不存在的标签 | 特性 capability 的 `permissions` 只能是 `<id>:*`（白名单为空）；`windows` ⊆ `tauri.conf.json` 的窗口标签；`default.json` 不得出现 `<id>:*` | 参考项目按窗口/能力拆 capability 的做法；Tauri 2 capability 按文件整体生效 |
| O7 | 边界文件反向断言 | D2/D3 只写在分析里，后人可用新 feature 绕过 | `ipc.rs`、`window.rs`、`tray.rs`、`cmd/config.rs`、`tauri.conf.json`、`src/App.tsx`、`src/layouts/AppShell.tsx` 不得出现任何清单内特性名字面量 | 把「形态/产物不进 feature 体系」变成可失败断言 |
| O8 | 单 package 单产物 | 有人可能在 `src-tauri` 内加第二份 `tauri.conf.json` 或第二个 `generate_context!` | 每个 package 恰一份 `tauri.conf.json`、恰一处 `generate_context!`；多产物必须新建 package | 参考项目 OpenBitFun 的 `src/apps/desktop` 与 `src/apps/data-migrator` 各自持有 `tauri.conf.json`、`build.rs`、`capabilities` |
| O9 | guard-the-guard | 清单为空或解析器失效时，遍历型测试会全绿 | 所有清单/插件/命令/产物解析器在「解析结果为空」时以非零退出，而不是返回空集合 | 参考项目 openhuman 的 forwarding 检查把「解析到 0 个 gate」视为检查本身失败（exit 2） |
| O10 | 显式无隐式可选特性 | `optional = true` 的依赖若未写成 `dep:`，会生成隐式 feature，使清单与 Cargo 集合出现未声明项 | 清单特性所拥有的 optional 依赖必须用 `dep:` 引用；B2 只做子集，不把隐式 feature 当合法特性 | Cargo 现代做法：用 `dep:` 抑制隐式 feature |

### 3.3 条件化或否决的优化

| 项 | 判定 | 理由 / 触发条件 |
|---|---|---|
| 前端 loader 生成器（清单 → `registry.ts` 条目 + `vite-env.d.ts`） | **否决（N=1）** | 每特性仅省 4 行，却引入生成物与手写文件的第三态；触发 N≥3 时重评 |
| `import.meta.glob("./*/module.ts")` | **否决** | 会把所有特性模块编进产物图，关闭态仍有 chunk，违背 G3 |
| 运行期 `flags[id]` 查表替代字面量守卫 | **否决** | 失去常量折叠，动态 import 无法被消除；且违反类型安全规范 |
| `inventory` / `linkme` 分布式注册 | **否决** | 新增依赖、链接段机制、注册顺序不可控；N≥3 时生成器更优 |
| 把插件装配整体搬进特性 crate | **否决（当前）** | cfg 只是换文件，core 仍需一行 `mod` 与表项；N≥2 或插件文件超过单文件职责时再评估 |
| 挂载表降为 2 字段 | **否决** | 启动顺序与退出清理退化为框架隐式语义，表不再自解释 |
| `cmd/<id>/` 迁到 `src/<id>/` 以归零 `cmd/mod.rs` | **否决（当前）** | 为 2 行收益改 4 处路径解析口径；路径稳定优先 |
| A2 允许 `src/<id>_plugin/` 目录内 cfg | **暂缓** | 当前插件是单文件；若插件拆成目录，cfg 仍应集中在根插件文件，子模块只被启用块引用 |
| build.rs 按 `CARGO_FEATURE_*` 门控插件声明 | **否决** | 关闭态 capability 引用的 `<id>:default` 会解析失败 |
| 在 `build.rs` 生成 `capabilities/*.json` | **否决** | 自修改源码树，构建顺序脆弱，生成物脱离 review |
| `OUT_DIR` + `include!` 生成 `mod` 块 | **否决** | A2 源码扫描覆盖不到，cfg 逃逸门禁；IDE 不可导航 |
| 多 bin 同 package 承载不同产物 | **否决** | 同一目录两套 conf / ACL / 契约语义；多产物必须独立 package |
| 预置组合写入产物名 / cargo 参数 / 图标 | **否决** | 与 `features.json` 并列成第二真源 |

### 3.4 外部参考（仅作实践对照，不引入代码依赖）

参考项目按「本机检出、不入仓」使用，此处只记项目名与项目内相对位置：

| 项目 | 可对照做法 |
|---|---|
| **OpenBitFun** | 多 app workspace：`src/apps/desktop` 与 `src/apps/data-migrator` 各自持有 `tauri.conf.json`、`build.rs`、`capabilities/`、`Cargo.toml`；`devtools` feature 用启用/关闭两套实现保证命令始终可注册；`capabilities:generate` / `capabilities:check` 是生成物 + 漂移检查的组合 |
| **openhuman** | `scripts/ci/product-features.txt` 把「产品要发布的 feature 集合」与 `[features] default`（贡献者集合）分开；`check-feature-forwarding.mjs` 双向断言 + 空解析即失败；`assert-shed.sh` 用 normal-edge 图成员证明依赖被移除；`required-features` 的测试目标用 build.rs 警告让跳过「可听见」 |
| **BongoCat** | workspace 按 crate 拆分；`required-features` 用于只读/生成型 bin；`just` 作为唯一构建入口 |

对照结论：**D1 的「清单 + 插件边界 + 遍历挂载」与现代做法一致；需要补的是「关闭态 CI 证据」和「边界反向门禁」**。D2/D3 的现代做法分别是运行期注册与独立 package，不是把 feature 用得更花。

---

## 4 终态设计

### 4.1 单一真源：`config/features.json`

目标形态（N=1）：

```json
{
  "vault": {
    "default": true
  }
}
```

规则：

| 编号 | 规则 | 失败后果 |
|---|---|---|
| R1 | 键 `id` 必须匹配 `^[a-z][a-z0-9_]*$`（单段蛇形，不含连字符） | 环境变量名非法或派生规则歧义 |
| R2 | 每项只允许 `default`（必填 bool）、`cargoFeature`（可选 string）、`env`（可选 string）；未知键即失败 | 拼写错误（如 `defualt`）会静默改变默认集合 |
| R3 | `cargoFeature` 省略时等于 `id`；显式写出时必须与 `id` 逐字相等 | 清单与 Cargo 出现第二条映射 |
| R4 | `env` 省略时等于 `${id.toUpperCase()}_ENABLED`；显式写出时必须逐字相等 | 前端 define 与后端开关可能指向不同变量 |
| R5 | `default` 表示**默认构建集合**（贡献者与本产品当前一致）；变体构建必须显式传 feature 集合，不依赖 Cargo default | 「默认」同时承担产品与贡献者语义，后续拆分时产生歧义 |
| R6 | 不承载 `requires`、`windows`、`product`、`icons`、`cargoArgs` 等字段；`requires` 仅在 N≥2 且出现真实依赖关系时引入 | 清单变成第二真源，覆盖 Cargo / tauri.conf / 打包脚本 |

消费方（四处，必须同规则）：

| 消费方 | 读取内容 | 备注 |
|---|---|---|
| `vite.config.ts` | `id`、`default`、`env`（派生） | 注入 `__<ENV>__` |
| `scripts/feature-build.ps1` | 同上 + `cargoFeature`（派生） | 注入 `<ENV>=1/0` 与 `--features` |
| `src/features/__tests__/coreBoundary.test.ts` | 同上 | F1 扩展校验 |
| `src-tauri/tests/assembly_contract.rs` | `id`、`cargoFeature`（派生） | B2 扩展校验 |

### 4.2 后端终态

沿用 `feature-mount-traversal-spec.md` §2、§3 的结构，不重画；本规格追加以下规则：

| 编号 | 规则 | 理由 |
|---|---|---|
| R7 | `<id>_plugin.rs` 的每条注册命令必须以 `crate::cmd::<id>::` 开头 | `build.rs` 只按该前缀派生 ACL；前缀不符会「运行期注册了、ACL 没声明」 |
| R8 | 特性 cfg 只允许出现在 `cmd/mod.rs` 与 `src/` 根下的 `<id>_plugin.rs`；`cmd/<id>/**`、`ipc.rs`、`window.rs`、`tray.rs`、`setup.rs`、`lifecycle.rs`、`plugins.rs`、`cmd/config.rs` 一律不得出现清单内特性 cfg | 保持 A2 白名单的可读性；关闭态差异只落在插件边界；插件文件固定在 `src/` 根，避免同名文件藏进子目录 |
| R9 | 关闭块不得引用任何 `assistant_<id>` / 特性 crate 符号 | 否则关闭态仍编译该 crate，G2 失效 |
| R10 | `register_conf_applier("<id>", ...)` 只能出现在启用块内 | A8 要求已注册分组 == 配置模块集合；core 不得知道特性分组 |
| R11 | `build.rs` 不读 `CARGO_FEATURE_*` 决定是否声明 `InlinedPlugin` | 关闭态 capability 仍引用 `<id>:default`，门控会导致构建失败 |
| R12 | `feature_mounts.rs` 保持 cfg-free、单一 `MOUNTS`、单一 `mounts()` | 表结构恒定，开关只影响函数体（B1/B7） |
| R13 | 每个 package 恰一份 `tauri.conf.json`、恰一处 `generate_context!` | 第二份 conf 需要第二个 package，不得就地叠加 |

### 4.3 前端终态

| 编号 | 规则 | 理由 |
|---|---|---|
| R14 | `registry.ts` 只允许「编译期字面量守卫 + 动态 import」；静态 import 一律失败 | 常量折叠才能让关闭态 chunk 不产出 |
| R15 | 每特性有 `src/features/<id>/module.ts`，导出 `FeatureModule`；core 只经 `registry.ts` 接触它 | 特性边界自包含 |
| R16 | 特性 DTO / 契约类型留在特性契约域；`src/types/protocol.contract.ts` 不得包含特性专有类型 | 既有 F 断言已守；关闭态不产出孤儿契约 |
| R17 | 关闭态构建的 bundle graph 不得含 `src/features/<id>/` 下任何模块 | F7；比字符串探针更接近真删除 |
| R18 | 特性代码不得被 `src/App.tsx`、`src/layouts/**` 等 core 文件静态引用 | 形态入口应走运行期注册或路由，不进 core 静态依赖 |
| R19 | 纯前端工具走 `src/tools/registry.ts`，不走 cargo feature | 21 个纯前端工具只有 chunk，没有二进制 |
| R20 | 纯设置项走 `definePage` / `fieldRegistry` / `section`，不走 cargo feature | 设置项不构成独立功能域 |

### 4.4 capabilities 终态

| 编号 | 规则 | 理由 |
|---|---|---|
| R21 | `capabilities/default.json` 只授 core 与常驻插件权限，不得出现任何 `<id>:*` | 特性关闭后不应残留授权 |
| R22 | `capabilities/<id>.json` 的 `permissions` 只能是 `<id>:*`（当前即 `<id>:default`）；白名单为空，新增非自身权限必须显式修订本规格 | 防止特性 capability 借窗口拿到 `global-shortcut` / `autostart` / `always-on-top` 等无关权限 |
| R23 | `<id>.json` 的 `windows` 必须是 `tauri.conf.json` 已声明窗口标签的子集 | 防止拼写错误或历史窗口名残留 |
| R24 | 关闭态保留 `<id>.json` 与 ACL 惰性声明 | 静态文件 + ACL 必须同生共死；这是已记账的残余，不构成「特性仍被编译」 |

### 4.5 构建变体终态

| 编号 | 规则 | 理由 |
|---|---|---|
| R25 | 变体构建一律 `--no-default-features` + 显式 `--features`；不得依赖 Cargo `default` | `default` 是默认构建集合，不是变体输入 |
| R26 | 两个隐式变体：`full` = 清单 `default` 集合；`slim` = 空集合 | N=1 时可枚举且 CI 可实跑，不需要 `presets.json` |
| R27 | N≥2 时引入 `config/presets.json`，只允许 `{ "<name>": { "features": [...] } }` | 把「自由」收敛为显式列举；不引入产物名/参数第二真源 |
| R28 | 并行构建不同变体时使用独立 `CARGO_TARGET_DIR`；同一 `target/` 会被 Cargo 锁串行化，且 bundle 产物会互相覆盖 | 操作正确性；变体若要共存安装，属于 D3 |

---

## 5 三条自由度的边界

### 5.1 D1：功能集合（本规格主体）

D1 只回答「这个功能在这个二进制里在不在」。判定链：

1. 有 Rust 命令或独立 crate → 走 feature（通道 A）；
2. 只有前端 chunk → 走工具注册表（通道 B）；
3. 只是设置项 → 走设置注册表（通道 C）。

只有通道 A 消耗 Cargo feature；通道 B/C 若强行按通道 A 做成 feature，是净成本（6 文件 9 行 + 足迹 2 行）。

### 5.2 D2：形态（本规格只定边界）

形态 = 窗口是否可开、面板展开/收起、布局、主题、快捷键绑定等。它属于**运行期**：

- 编译期只保留「功能整体在不在」；形态差异由运行期配置或入口渲染决定。
- 若某功能将来真的需要独立 webview 窗口：窗口 spec 由插件边界的提供者函数返回（关闭态返回空切片），`window.rs` 遍历挂载表收集；挂载表增加 `window_specs` 字段，B5 从三函数扩展到四函数。**本规格不实施**，因为没有消费方（vault 的解锁窗口是 `assistant_security::unlock_dialog` 原生对话框，不进 `WINDOW_SPECS`）。
- 静态 `tauri.conf.json` 窗口、托盘菜单条目、`assetProtocol` scope 是 C2 阻塞点：**先迁运行期注册，再谈 feature 化**。在迁移完成前，orb / mascot 不得进入 `config/features.json`。
- 反向门禁：`ipc.rs`、`window.rs`、`tray.rs`、`cmd/config.rs`、`tauri.conf.json`、`src/App.tsx`、`src/layouts/AppShell.tsx` 不得出现清单内特性名。若 `orb` 被声明为特性，这些文件会立即失败——这是要求先做形态提供者重构的信号，不是加 allowlist 的理由。

### 5.3 D3：产物（本规格只定边界）

D3 = 产物身份/安装包不同，例如 `productName`、`identifier`、图标、bundle targets、独立发布节奏、可与主产品并存安装。

- 必须在 package 级解决：`apps/<product>/` 作为 workspace member，各自持有 `Cargo.toml`、`build.rs`、`tauri.conf.json`、`capabilities/`、`icons/`、`src/main.rs`；共享 core crates 与前端源码。
- 同一 package 内多 `[[bin]]` 若共享同一份 conf/ACL，只能作为工具 bin（如 `dev_mcp_server`）；若需要不同 conf/ACL，必须新建 package。
- 反向门禁：`src-tauri` 恰一份 `tauri.conf.json`、恰一处 `generate_context!`；出现第二份即失败，并提示「多产物走独立 package」。
- 触发条件（任一满足才启动 D3 规格）：产物名/identifier/图标/安装包不同；需要并存安装；需要独立发布通道；需要不同静态窗口集合。
- 本规格不实施 D3；当前所有变体都是同一产品身份的构建变体，不应并存安装。

---

## 6 门禁

### 6.1 既有门禁的处置

| 组 | 处置 |
|---|---|
| A1–A8 | 保持；A2 的白名单与 R8 一致；A7 的 ACL 派生保持 |
| B1–B9 | 保持；B2 扩展清单 schema/派生值/无隐式 optional feature；B3 扩展边界文件集合；B8 保持存在性断言 |
| F1–F6 | 保持；F1 扩展清单 schema/派生值；F6 扩展边界文件集合 |
| 门禁总数 | 只随「规则类别」增长，不随 N 增长；所有循环按清单/磁盘集合驱动 |

### 6.2 新增/扩展门禁

| 编号 | 断言 | 实现位置 | 失败含义 |
|---|---|---|---|
| **B2 扩展** | 清单非空；`id` 匹配 `^[a-z][a-z0-9_]*$`；只允许已知键；`cargoFeature`/`env` 省略时派生、显式时逐字相等；清单特性所拥有的 optional 依赖必须用 `dep:` 引用，不得靠隐式 feature 启用 | `assembly_contract.rs` | 清单拼写错误、隐式 feature、派生值漂移 |
| **B3 扩展** | `plugins.rs`、`lifecycle.rs`、`setup.rs`、`ipc.rs`、`window.rs`、`tray.rs`、`cmd/config.rs` 的字符串字面量中不出现任何清单内特性名 | 同上 | core 又直接点名特性，遍历化/形态边界被绕过 |
| **B10** | 特性 capability：`permissions` 只允许字符串 `<id>:*`（白名单为空，不接受内联权限对象）；`windows` ⊆ `tauri.conf.json` 窗口标签；`default.json` 不含 `<id>:*` | 同上 | 特性借 capability 拿到无关权限，或窗口标签拼错 |
| **B11** | 每个 package 的根目录恰一份 `tauri.conf.json`；其 `src/` 内（去注释后）恰一处 `generate_context!` | 同上 | 有人就地加第二产物，导致 conf/ACL 语义分裂 |
| **B12** | `<id>_plugin.rs` 的 `generate_handler!` 每条命令以 `crate::cmd::<id>::` 开头 | 同上 | 命令注册与 ACL 派生前缀不一致，运行期权限失败 |
| **F1 扩展** | 与 B2 相同的清单规则；清单非空 | `coreBoundary.test.ts` | 前端与脚本读到不同的派生值 |
| **F6 扩展** | 边界文件集合加入 `src/App.tsx`、`src/layouts/AppShell.tsx`、`src-tauri/src/ipc.rs`、`src-tauri/src/window.rs`、`src-tauri/src/tray.rs`、`src-tauri/src/cmd/config.rs`、`src-tauri/tauri.conf.json` | 同上 | 形态/静态声明把特性名带进 core |
| **F7** | Vite `generateBundle`：关闭态产物中 `chunk.modules` 的键（规范化路径）不得落在 `src/features/<id>/` 下 | `vite.config.ts` 插件 | 关闭态仍产出特性 chunk |
| **CI-1** | CI 常驻跑 `full` 与 `slim` 两态：编译、`assembly_contract`、前端 build、产物探针 | `.github/workflows/ci.yml` | 关闭态只在本地可用，迟早腐烂 |
| **CI-2** | `scripts/assert-feature-shed.mjs`：禁用特性的 `dep:` crate 不在 normal-edge 依赖图 | 新脚本 | 「关闭」只是不注册，依赖仍被编进二进制 |

> B3 用「精确字符串字面量」比对，F6 用「去注释后的子串」比对；两者互补：前者避免普通文案误报，后者能抓住 `WINDOW_ORB` / `show_orb` 这类标识符里的特性名片段。

### 6.3 guard-the-guard

| 编号 | 规则 | 落点 |
|---|---|---|
| GG-1 | 清单解析结果为 0 条 → 失败（exit 2），不是通过 | `feature-build.ps1`、`coreBoundary.test.ts`、`assembly_contract.rs` |
| GG-2 | 插件文件/命令/产物/chunk 解析结果为 0 → 失败 | A5/A7 已有下限；F7 与 CI-2 新增 |
| GG-3 | 脚本类检查（产物探针、依赖断开、bundle graph）在「没有可检查对象」时失败 | `assert-feature-absent.mjs` 已有；新增脚本同规则 |
| GG-4 | 门禁不得把命令条数写成常量；只允许集合相等、至少 1 条、解析总数下限 | 全部门禁 |

---

## 7 构建、变体与 CI

### 7.1 两个隐式变体

| 变体 | 特性集合 | 本地命令 | 用途 |
|---|---|---|---|
| `full` | 清单 `default`（当前 `vault`） | `.\scripts\feature-build.ps1 -Task check` | 默认产品行为 |
| `slim` | 空 | `.\scripts\feature-build.ps1 -Task check -Without vault` | 证明可裁剪 |

CI 必须两种都跑；不能只跑默认态。

### 7.2 预置组合（N≥2 才启用）

当清单出现第二条特性时，新增 `config/presets.json`：

```json
{
  "full": { "features": ["vault", "clock"] },
  "slim": { "features": [] },
  "vault-only": { "features": ["vault"] }
}
```

规则：

- 只允许 `features` 字段；出现产物名、cargo 参数、图标、窗口、`requires` 之外的任何字段即失败。
- 每个 preset 的 `features` ⊆ 清单键集合；`full` 必须等于清单全集；`slim` 必须为空。
- 若引入 `requires`，preset 必须包含其闭包；CI 对每个 preset 跑一遍编译/测试/产物断言。
- `feature-build.ps1 -Preset <name>` 与 `-Features` / `-Without` 互斥；`-Target` 继续表示 rust target triple，不复用。
- 不做 2^N 全组合；`cargo hack --feature-powerset` 不进入 CI。

### 7.3 生成器触发判据（维持条件化）

满足任一条件再评估「清单 → 挂载表/前端 loader/vite-env 声明」生成器：

| 判据 | 说明 |
|---|---|
| 挂载表条目 ≥ 3 | 手写 M 线性增长，生成器边际成本不变 |
| `build.rs` 需要第二套派生规则 | 按文件遍历已不足以覆盖 |
| `cmd/mod.rs` cfg 行数 ≥ 6 | 单文件 cfg 密度触发审查疲劳 |
| 同一特性在 core 侧需要 2 处以上声明 | 声明型足迹开始复利 |

在此之前，生成器引入「生成物入库 vs 真删除」的第三态，收益为负。

### 7.4 CI 落点

| Lane | 步骤 | 说明 |
|---|---|---|
| 前端 | `npm run build`（full） | 确认 F7 不误报 |
| 前端 | `VAULT_ENABLED=0 npm run build`（slim） | F7 必须失败于任何特性模块 |
| 前端 | `node scripts/assert-feature-absent.mjs --feature vault --dist dist` | 字符串探针作为补充 |
| 后端 Linux | `cargo check -p assistant-app --no-default-features` | 关闭态编译 |
| 后端 Linux | `cargo test -p assistant-app --no-default-features --test assembly_contract` | 关闭态门禁 |
| 后端 Linux | `node ../scripts/assert-feature-shed.mjs` | normal-edge 依赖图断言；脚本内部 `cwd = src-tauri` |
| 后端 Windows | 同两项 `cargo` 命令（要求） | vault 含 Windows 专属代码，关闭态应在 Windows 也编译一次 |

注意：`cargo test --workspace --no-default-features` **不能**替代上述命令——workspace 成员的测试目标仍会编译 `assistant-vault` 自身；关闭态要针对 `assistant-app` 这个 package。

---

## 8 新增/下线特性操作序列

### 8.1 新增特性

| 步骤 | 动作 | 门禁 |
|---|---|---|
| 1 | `config/features.json` +1 条目（只写 `default`） | B2/F1 |
| 2 | `src-tauri/Cargo.toml` +1 feature + `dep:` / `optional = true` | B2 |
| 3 | 新建 `src-tauri/src/<id>_plugin.rs`：启用/关闭两套实现，命令前缀 `crate::cmd::<id>::` | A2/B4/B5/B12 |
| 4 | `cmd/<id>/` 命令目录；`cmd/mod.rs` +1 行 cfg `mod` | B6 |
| 5 | `lib.rs` +1 行 `mod <id>_plugin;`；`feature_mounts.rs` +4 行表项 | F1/B2/B7 |
| 6 | 新建 `capabilities/<id>.json`（只授 `<id>:default`，`windows` ⊆ 已声明窗口） | B8/B10 |
| 7 | 前端 `src/features/<id>/module.ts`；`registry.ts` +3 行守卫；`vite-env.d.ts` +1 行 | F1/F4 |
| 8 | 跑 `full` 与 `slim` 两态、`assembly_contract`、`npm run test`、`contract:check` | AC |

新增特性的 core 足迹预期为空；若 `FEATURE_FOOTPRINT_ALLOWLIST` 需要新增条目，必须在审查中说明为何不能收进特性边界（新特性不得复制 vault 的历史豁免）。

### 8.2 下线特性

| 步骤 | 动作 | 检查 |
|---|---|---|
| 1 | 删除 `src/features/<id>/**`、`src-tauri/src/<id>_plugin.rs`、`cmd/<id>/**`、`capabilities/<id>.json`、特性契约类型 | 无残留引用 |
| 2 | 删除清单条目、Cargo feature、`dep:` / `optional = true`、挂载表表项、`lib.rs` mod、`cmd/mod.rs` cfg mod、前端守卫/declare | B2/B6/F1 |
| 3 | 清理特性事件名/配置分组/前端 i18n 命名空间 | 足迹 allowlist 只减不增 |
| 4 | 跑 `full` 与 `slim` 两态 + 依赖断开 | AC |

下线是「真删除」，不是「关闭」；不要用注释或 `#[allow]` 掩盖残留。

---

## 9 验收

### 9.1 命令

仓库根：

```powershell
.\scripts\feature-build.ps1 -Task check
.\scripts\feature-build.ps1 -Task check -Without vault
.\scripts\feature-build.ps1 -Task test
.\scripts\feature-build.ps1 -Task test -Without vault
npm run typecheck
npm run lint
npm run test
npm run contract:check
```

`cwd = src-tauri`：

```powershell
cargo test --test assembly_contract
cargo test --test assembly_contract --no-default-features
```

`cwd = src-tauri`（依赖断开，仅 `slim` 变体）：

```powershell
node ..\scripts\assert-feature-shed.mjs
```

关闭态前端产物：

```powershell
$env:VAULT_ENABLED = "0"
npm run build
node scripts/assert-feature-absent.mjs --feature vault --dist dist
```

### 9.2 判定表

| 编号 | 判定 | 通过条件 |
|---|---|---|
| AC1 | 默认态编译 | `-Task check` 退出码 0 |
| AC2 | 关闭态编译 | `-Task check -Without vault` 退出码 0 |
| AC3 | 依赖断开 | `assert-feature-shed.mjs` 退出码 0，且输出被禁用特性的 `dep:` crate 均 absent |
| AC4 | 后端门禁 | 默认态与关闭态 `assembly_contract` 全绿，含 A1–A8、B1–B12 |
| AC5 | 前端门禁 | `npm run test` 中 `coreBoundary` 全绿（F1–F6）；F7 在关闭态 `npm run build` 中生效 |
| AC6 | 契约同步 | `npm run contract:check` 退出码 0 |
| AC7 | 产物图 | 关闭态 `npm run build` 成功且 F7 不报特性 module；字符串探针退出码 0 |
| AC8 | 行为等价 | 默认态命令集合、配置读写、退出清理、启动播种与改造前一致 |
| AC9 | capability 最小权限 | 特性 capability 只含 `<id>:*`，窗口标签均存在 |
| AC10 | 单产物边界 | `src-tauri` 恰一份 `tauri.conf.json`、恰一处 `generate_context!` |
| AC11 | CI 常驻 | CI 有 full/slim 两态 lane，且失败会阻断合并 |
| AC12 | guard-the-guard | 人为清空清单/插件目录时，门禁以非零退出，而不是全绿 |

---

## 10 风险与回滚

| 风险 | 触发点 | 处置 |
|---|---|---|
| 关闭态 CI 变慢 | 每平台都跑两态 cargo | 后端关闭态只跑 Linux + Windows；macOS 依赖默认态覆盖 |
| F7 误报 | 模块 id 含绝对路径/虚拟模块 | 只匹配规范化后的 `/src/features/<id>/` 子串；虚拟模块不参与 |
| F7 漏报 | 特性代码被复制进 core chunk | 保留字符串探针；代码复制本身应按「core 引用特性」审查 |
| `assert-feature-shed` 假阳性 | `-e normal` 仍显示 dev/build 边 | 只解析 normal 边；脚本输出中同时打印图命令，便于复核 |
| capability 残余 | 关闭态仍存在 `<id>.json` 与 ACL | 已记账的惰性声明；B10 保证权限面最小；若将来需要，另立「变体 capability 生成」规格 |
| 清单 schema v2 改动 | 四处消费方不同步 | S 步骤按「先加派生兼容、再删手写字段」顺序；B2/F1 同时守 |
| 边界文件误伤 | 新特性名与现有 `orb` 常量冲突 | 这是预期失败：先做 D2 形态迁移，不加 allowlist |
| 多产物偷渡 | 第二份 `tauri.conf.json` | B11 直接失败；按 §5.3 新建 package |
| 回滚 | 任一步失败 | 门禁与脚本可独立回退；不涉及数据/配置格式，无迁移回滚 |

---

## 11 改造步骤（建议顺序）

| 步骤 | 动作 | 说明 |
|---|---|---|
| S1 | 先加「现状可通过」的边界/结构门禁：B3/F6 边界扩展、B10/B11/B12 | 先固化边界，再改机制；失败即说明现状已有越界 |
| S2 | 加 CI-1 full/slim lane | 让关闭态从人工证据变为常驻证据 |
| S3 | 清单 schema v2：`vite.config.ts` / `feature-build.ps1` / `coreBoundary.test.ts` / `assembly_contract.rs` 改为派生 | 先兼容显式 `env`/`cargoFeature`，再删字段 |
| S4 | B2 扩展：id 正则、闭键、无隐式 optional feature | 与 S3 同一批改 |
| S5 | 加 F7 构建期 bundle graph 断言 | 与字符串探针并存 |
| S6 | 加 `scripts/assert-feature-shed.mjs` | normal-edge 图成员判定 |
| S7 | 更新 CI 步骤与文档交叉引用 | 本文与前置规格互相指向 |
| S8 | 预置组合 | **不在此次实施**；N≥2 时按 §7.2 新增 |

每步自包含、可单独回滚；S1 不改变任何运行期行为。

---

## 12 与既有文档的关系

| 文档 | 关系 |
|---|---|
| 《FEATURE-OPTIONAL-BINARY-ANALYSIS.md》 | 本规格保留其 D1/D2/D3 判定，并把 §8 的 G-1/G-2/G-5/G-7 落成具体门禁；G-4 维持「按文件名匹配」并记录为暂缓项 |
| 《feature-mount-traversal-spec.md》 | 本规格沿用其 R1–R6、S1–S9 终态与 B1–B9/F1–F6；只追加不推翻 |
| 《vault-optional-build.md》 | L1 基线；本规格不重做 L1，只补关闭态证据 |
| 《FEATURE-ZERO-COST-MOUNT-ANALYSIS.md》 | 成本模型与「不可归零项」的唯一来源；本规格不重复计量，只引用其 R1–R4 约束 |
| `docs/principle/CODE-SPLIT-PRINCIPLE.md` | 拆分判据唯一来源；本规格不另立阈值 |

冲突时的优先级：**当前代码 + 本规格 > 前置规格 > 分析文档**。实施时若发现本规格与代码事实不符，先修规格再改代码，不把偏差留成「文档滞后」。

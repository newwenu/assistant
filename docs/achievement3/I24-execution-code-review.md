# I24 阶段执行后代码审查记录

- 审查对象：`docs/store/I24-feature-extension-spec.md`、`docs/store/I24-tasklist.md` 与实际落地的代码/配置/断言
- 审查方式：纯静态阅读（不运行构建、不运行测试、不执行任何命令）
- 审查重点：按委托要求，只看**文档善后**——文档与实现的口径落差、未显式登记的安全取舍、实施完成后未回收的文档内容；**不评价实施正确性**，不把「probe 未删除」「死代码未清理」这类实施层遗留算作本审查的问题（仅在文档缺对应记录时登记）
- 审查日期：2026-10-31

## 0. 结论摘要

一句话结论：**未发现会造成安全回归或运行期破坏的问题；真正的问题是文档层面的「承诺大于实现」与「安全取舍未登记」，且集中在规格文档自身内部口径互相矛盾。**

按严重度归并：

| 级别 | 类别 | 条目数 | 说明 |
| --- | --- | --- | --- |
| P0 | 安全取舍未登记 | 3 | ACL 粗粒度授权的安全含义、命令清单 4 份手写副本的结构性成本、`plugin:` 约束降级为建议但验收仍按强不变量表述 |
| P0 | 文档状态自相矛盾 | 2 | 规格首部仍写「未实施」；§6.3 探针施工图与 §10.1「探针已删除」并存 |
| P1 | 可计数声明与实际不符 | 5 | 命令条数、cfg 行数上限、`lib.rs` 行数门禁、A5 解析器下限、A4 声明集合 |
| P1 | 文档声称存在、代码中不存在 | 6 | `__settingsSnapshotMeta`、`after` 锚点、空分类抛错、page 归属推导、§6.2 注册表闭合断言、§3.8.4 样例签名 |
| P2 | 代码残留无对应文档记录 | 2 | `src/features/vault.ts` 死代码、同名文件与目录共存 |

最高风险三条（建议优先处理文档，不涉及代码改动）：

1. `capabilities/default.json` 的 1 行 `vault:default` 等价于「三个窗口各拿到全部 66 条 vault 命令（含导出、复制密码、灾难恢复导出、改主密码）」，文档只记录了「边际成本 1 行」，没有登记这一取舍的安全含义；
2. 同一份 66 条命令清单存在 4 份手写副本，任一漂移只在运行期暴露，文档 §10.1 只描述了其中 2 份；
3. 文档 §6.2 声称存在的「注册表闭合」后端断言在代码中不存在，前端那一条是同义反复（同一 `Map` 的两个读法），验收表却按「已有测试」计为通过。

## 1. 文档状态与元数据（P0）

### 1.1 同一文档并存三种实施状态

- 首部状态行仍为「设计稿 v2.4，E0–E8 未实施」（`I24-feature-extension-spec.md` L4）；
- 中段 §3.8.3a 标注「spike 实测结论（已完成）」；
- §10.1 标注「P.4 已落地」，且 `I24-tasklist.md` 的 P.4 已勾选。

结果：读者无法从文档首部判断阶段是否完成，必须翻到 §10 才能拼出真实状态。规格文档没有「实施结果」章节，§1「现状事实」仍是实施前的代码快照（C1–C9 硬编码点），与 §10 的「已消除」形成时间错位。

### 1.2 §6.3 探针施工图未回收

§6.3 整节仍是可执行的施工图（「在 `src/features/_probe/` 放一个最小探针特性……在前端 `features/registry.ts` 加一段 `if (__PROBE_ENABLED__)`」），而 §10.1/§10.2 与 `I24-tasklist.md` 的 P.4 已记录探针演练完成并删除探针、改为 CI 断言。

同一文档内两节指令互斥，且 §6.3 未标注「历史施工图，最终形态见 §10.1」。这是本阶段最容易被后续读者误执行的文档缺口——照 §6.3 操作会重新引入已被删除的 `_probe`。

### 1.3 装配点数量三处口径互斥

- §6.3：「4–5 处」（预估阶段）；
- §10.1：「实测每特性需触碰 **11** 个装配点文件」，随后 F2 修正为 **10**；
- §9 验收总表仍写「新增可选特性的后端边际成本 = 1 个插件文件 + 1 对 cfg + 1 组 ACL」。

§9 的表述是「收益承诺」而不是「实测值」，但没有任何一句说明实测是 10 个装配点、且其中 5 个在后端。F2 的修正只落在 §10.1 一处，没有回写到 §9 与 §6.3，导致「零 core 改动新增特性」的对外口径与 §10.1 自己的实测结论相反。

## 2. 可计数声明与实际不符（P1）

| 声明位置 | 文档值 | 实测值 | 影响 |
| --- | --- | --- | --- |
| §3.8.3 / §3.8.5 / §5 / §9 | core 57 条命令 | `src-tauri/src/ipc.rs` L9–L70 = **62** 条 | A4 的 `declared` 侧口径整体偏移 5 条 |
| 全文命令总数 | 123 条 | **128** 条（core 62 + vault 66，`commands.json` L6–L136） | 见下行 |
| §3.8.5 A5 | 下限 110（「留 13 余量」） | 实际 `PARSER_FLOOR = 120`，余量 **8** | 余量从约 10% 收窄到约 6%，对「少量条目漏解析」的拦截力下降；只对「整块丢失」有效 |
| §3.8.5 A2 | 「全后端 cfg ≤ 6，仅两文件」 | 实际按特性名分组，仅对 `vault` 计数 `VAULT_CFG_LIMIT = 6`；非 vault 特性**无逐特性上限** | 断言弱于文档：未来新特性可在自己的 `*_plugin.rs` 里堆任意多 `cfg` 而不被拦 |
| §3.8.5 A3 | `lib.rs` ≤ 40 行 | 实际 25 行 | 门禁留 15 行松弛，「装配归位」约束可被逐步侵蚀而不报警 |

补充：`vault_plugin.rs` 实测 2 行 `cfg`（1 对）、`cmd/mod.rs` 3 行 `cfg`（1 对 + 1 个带条件的 `use`），与 §3.8.3a 的「2 行 cfg」描述差 1 行（`use` 分支未计入）。差异很小，但文档中的「全后端共 73 行 cfg」是迁移前基线，未标注「历史基线」，容易被当作当前值引用。

## 3. 未登记的安全取舍（P0）

### 3.1 ACL 粗粒度授权：只登记了成本，没登记安全含义

事实：

- `src-tauri/capabilities/default.json` L5 `"windows": ["main", "orb", "settings"]`，L43 `"vault:default"`——**1 行授予全部 66 条命令**；
- `src-tauri/permissions/vault/default.toml` 66 条 `allow-vault-*`，其中包含 `vault_reveal_password`、`vault_copy_password`、`vault_export_to_file`、`vault_disaster_recovery_export`、`vault_change_master_password` 等高敏命令；
- 三个窗口同等授权，`orb` 窗口对 vault 无业务需求，也拿到全量。

文档现状：§3.8.3a 只写了「ACL 边际成本 1 行/特性」，把它当作**收益**来记录；没有登记这一形态的**代价**——capability 是 Tauri v2 里唯一能做「按窗口 / 按命令」最小权限的位置，当前形态等于把该位置一次性用尽（1 行 = 全量 × 3 窗口），后续若要引入「低信任窗口」（例如第三方内容预览、外部链接窗口），必须回头重构 capability 与权限集。

同时，文档缺少一句显式的**无回归结论**：迁移前 app 命令本就不经 ACL，三个窗口本就能调用全部命令，因此「插件化 + 1 行 ACL」并没有扩大暴露面，也没有收窄。这句话不写，读者会误以为插件边界提升了安全边界；写清了，才能把这个取舍作为「已知并接受」而不是「没想过」。

建议登记三条：

1. 当前 ACL = 全量命令 × 三窗口，无按窗口/按命令细分；
2. 与迁移前暴露面等价，无新增暴露；
3. 该形态是后续「窗口分级」的唯一阻塞点，若引入低信任窗口需同时改 capability 与 `permissions/vault/default.toml`。

### 3.2 命令清单 4 份手写副本，文档只描述了 2 份

66 条 vault 命令在四处各自手写：

| 位置 | 形态 | 作用 |
| --- | --- | --- |
| `src-tauri/build.rs` | `"vault_xxx"` 蛇形字面量 ×66 | `InlinedPlugin::commands()` 生成 |
| `src-tauri/permissions/vault/default.toml` | `allow-vault-xxx` 短横线派生 ×66 | ACL 权限集 |
| `src-tauri/commands.json` | `groups.vault` ×66 | A4 断言 `declared` 侧唯一事实源 |
| `src-tauri/src/vault_plugin.rs` | `crate::cmd::vault::vault_xxx` ×66（另 1 处 `vault_setup`） | `generate_handler!` 实际注册 |

其中 `permissions` 的短横线名是蛇形名的第 5 种派生形态，靠构建期约定而非断言保证一致。§10.1 只描述了 `build.rs` 与 `commands.json` 两段（每特性 2 段），没有把「4 份副本 + 1 种派生」这个结构性成本写进「边际成本」条目；F5 只登记了「逐命令求交待补」，未量化「漏改一处 = 运行期 `Command not allowed` 或 `command not found`，且现有断言全部通过」。

**v2.6 补注**：本节的处置已从「补断言」升级为「配置收敛」，逐处必然性判定与两步方案见 §9；规格文档对应改写为 §11.2。

### 3.3 `plugin:` 字面量约束：文档降级为建议，验收仍按强不变量表述

- §3.10 标准 6 已明确「ESLint 全局规则降为建议，只约束 `src/features/**` 与新增 service」；
- 但 §6.3 与 §9 仍写「除 `featureIpc.ts` 与各特性 service 外不得出现 `plugin:`」这种强不变量语气。

实测：生产代码中 `plugin:` 字面量唯一出口确实是 `src/services/featureIpc.ts` L8；其余出现点都在 `src/vault/core/__tests__/**` 的 mock 与断言里（测试产物，可接受）。因此**当前实现是干净的**，但文档两处语气不一致会让后续读者以为有 lint 兜底；实际上没有对应的 lint 规则落地，这条约束只能靠人工审查维持。

### 3.4 后端 conf applier 注册表无重复检测

- `src-tauri/src/cmd/mod.rs` L14 静态 `CONF_APPLIERS`，L38–L56 `register_conf_applier()` 只做 `push`，L30 `conf_side_effects()` 只做键投影，**没有重复 group 检测**；
- 前端 `src/settings/core/confRuntime.ts` 的 `registerConfSlot()` 在 DEV 下重复注册即抛 `duplicate-conf-group`。

两侧一致性策略不对称：前端硬失败、后端静默接受。重复注册时后端 `find` 命中先注册者，后者永不执行，且无任何告警。F3 只登记了「读写前缀不对称」（`get_vault_conf` 带前缀 / `patch_conf` 裸名 + 任意 group 字符串），没有登记「后端注册表无重复检测」这一条。

## 4. 文档声称存在、代码中不存在（P1）

| 文档条款 | 文档承诺 | 代码实际 | 后果 |
| --- | --- | --- | --- |
| §3.3 | 「DEV 下暴露 `__settingsSnapshotMeta()`」 | 全仓无此符号 | 文档给出的调试入口不存在 |
| §3.6 | 「`page` 支持 `after: SettingsPageId` 锚点，构建期解析并检测环」 | `pageTypes` 无 `after` 字段，`snapshot.ts` 无环检测 | 排序能力被文档高估 |
| §3.6 | 「分类必须被声明；声明了但无页面 → 构建期抛错」 | `snapshot.ts` L417 `.filter((category) => category.pages.length > 0)` 静默剪掉空分类 | 与文档「不做静默剪枝」的自我承诺相反 |
| §3.5 | 「`visibility !== deepLinkOnly` 的 page 必须能推导出归属（`navParent` 或自身 route）」 | `ViolationRule` 18 条（`snapshot.ts` L47–L65）中无对应规则 | 该不变式无任何检查 |
| §6.2 | 「注册表闭合：后端 `conf_side_effects()` 的 group 集合 == `assistant-config` 结构体集合」 | 无该测试 | 验收表按「已有」计为通过 |
| §6.2 | 前端「`registerConfSlot` 键集合 == `getConfGroups()`」 | 二者读同一 `Map`，断言恒真 | 同义反复，无鉴别力 |

另外 §6.2 的「命令名一致」确有 `commandNames.test.ts`，但只比对字面量 invoke，**不覆盖经 `featureCommand()` 拼接后的动态名**，因此对 `plugin:` 前缀错拼无拦截力。

§3.8.4 的代码样例也已过期：文档给的是 `CONF_SIDE_EFFECTS: &[(&str, SideEffect)]` 与 `SideEffect = fn(&AppHandle, &Value, &Value)`（两参、无返回值）；实现是 `CONF_APPLIERS` + `ConfApplier = fn(&AppHandle, serde_json::Value) -> Result<(), String>`（单参、有返回值）。§3.1/§3.2 同样有命名漂移：`conf?: FeatureConfSlot[]` → 实现 `confs?: ConfSlot[]`；`VAULT_CONF_SLOT` → 实现 `vaultConfSlot`。

## 5. 代码残留与文档对应缺口（P2）

1. `src/features/vault.ts` 仅一行 `export const VAULT_ENABLED = __VAULT_ENABLED__`，全仓无 import；`src/features/registry.ts` 已直接 `import("./vault/module")`。即「`VAULT_ENABLED` 间接层」已被放弃，但 §10.1 迁移表仍写「`features/registry.ts`：每特性一段编译期守卫 + 动态 import」——旧守卫变量已不再被该文件消费。
2. `src/features/vault.ts` 与目录 `src/features/vault/` 同名共存，模块解析靠后缀优先级区分，属于易误读的目录结构，文档未记录这一过渡形态的去留。
3. `I24-tasklist.md` 的 P.4 一项同时承载「删除探针」「新增 `coreBoundary` 断言」「测试收敛」三件事，而该文件自身的「提交切分」按 11 项组织；P.4 与切分表无对应关系，追溯时无法定位「哪次提交做了哪件」。

## 6. 建议的文档动作（不涉及代码改动）

P0（下次编辑规格文档时必须做）：

1. 在规格首部加「实施状态」表：E0–E8 + P.1–P.4 全部完成，标注日期与「探针已删除、改为 CI 断言」。
2. 给 §6.3 加顶部提示：**历史施工图，最终形态见 §10.1；探针已删除，勿再创建**。
3. 新增「安全取舍登记」小节，至少包含 §3.1（ACL 全量×三窗口 + 无回归结论 + 窗口分级的阻塞点）、§3.2（4 份命令副本）、§3.4（后端注册表无重复检测）。

P1：

4. 回写命令条数：core 62 / vault 66 / 合计 128；同步 A5 下限 120 与「余量 8、仅拦整块丢失」的说明。
5. 把 A2 措辞改为「vault 相关 `cfg` ≤ 6 且按特性名分组；非 vault 特性暂无上限」，避免把「按特性」误读为「全后端」。
6. 把 §9 的「边际成本 1 插件文件 + 1 对 cfg + 1 组 ACL」改为「实测 10 个装配点（后端 5 / 前端 3 / ACL 2）」，与 §10.1 对齐。
7. 对 §4 表中 6 条「文档有、代码无」的条款逐条二选一：**删条款**或**标 `TODO(未实现)`**，不允许继续以「已具备」语气存在。优先处理「空分类静默剪枝」——它与文档自我承诺正面冲突。
8. 统一命名：`conf`/`confs`、`ConfSlot`/`FeatureConfSlot`、`CONF_APPLIERS`/`CONF_SIDE_EFFECTS`。

P2：

9. 登记 `src/features/vault.ts` 为待清理死代码，并说明 `registry.ts` 不再消费 `VAULT_ENABLED`。
10. 把 F3/F5 从「记录项」升为带 owner 与触发条件的待办（F5 的逐命令求交是 4 份副本唯一的收敛手段）。
    - **v2.6 补注**：本条已被改写。处置不再是「补求交断言即止」，而是「配置收敛」（S1 断言止血 + S2 收敛为生成物），见 §9。原表述中的「唯一收敛手段」应读作「唯一止血手段」。

## 7. 附：审查证据（仓库相对路径）

- `docs/store/I24-feature-extension-spec.md` L4（状态）、L25（基线行数）、L29（一句话承诺）、L142–L196（§3.1）、L456（§3.5 归属规则）、L502（§3.6 锚点）、L988（§6.2）、L999（§6.3 探针施工图）、L1051（§9）、L1072+（§10.1）
- `docs/store/I24-tasklist.md` P.4 条目
- `src-tauri/src/ipc.rs` L9–L70（core 62 条，无 `cfg`）
- `src-tauri/commands.json` L6–L136（core 62 + vault 66）
- `src-tauri/src/vault_plugin.rs` L5 / L87（全文件仅 1 对 `#[cfg]`），L60–L82（66 条命令 + `vault_setup`）
- `src-tauri/build.rs`（66 条蛇形字面量）
- `src-tauri/permissions/vault/default.toml`（66 条 `allow-vault-*`）
- `src-tauri/capabilities/default.json` L5 / L43
- `src-tauri/src/cmd/mod.rs` L14 / L30 / L38–L56
- `src-tauri/src/cmd/config.rs` L171 / L179（`get_vault_conf` / `patch_conf`）
- `src-tauri/tests/assembly_contract.rs` A1–A6
- `src/settings/core/types/snapshot.ts` L47–L65（`ViolationRule` 定义）、`src/settings/core/snapshot.ts` L417（空分类过滤）
- `src/settings/core/confRuntime.ts`（`registerConfSlot` / `isConfGroupAvailable`）
- `src/features/registry.ts`（直接 `import("./vault/module")`）
- `src/features/vault.ts`（1 行死代码）
- `src/services/featureIpc.ts` L8（唯一 `plugin:` 生产出口）
- `src/services/settingEvents.ts`（`isConfGroupAvailable` 判定）

## 8. 善后清单（简易执行版）

图例：`[x]` 已完成（文档落地或代码/断言落地，括号内注明轮次）；`[ ]` 仍开放（技术债，需要时再排期）。

### 8.1 文档部分（本轮已完成）

| # | 动作 | 落点 | 状态 |
|---|---|---|---|
| 1 | 首部加「实施状态」表（E0–E8 + P.1–P.4 全部完成，探针已删） | 规格 §首部 | [x] |
| 2 | §6.3 顶部标注「历史施工图，探针已删，勿再创建」 | 规格 §6.3 | [x] |
| 3 | 新增「安全取舍登记」：ACL 全量×三窗口 + 无回归结论 + 窗口分级阻塞点、4 份命令副本、后端注册表无重复检测、`plugin:` 无 lint | 规格 §11 | [x] |
| 4 | 回写命令条数：core 62 / vault 66 / 合计 128 | 规格 §2 表、§3.8.2、§3.8.3、§3.8.5 A4、§5 表、E7 | [x] |
| 5 | A5 下限改 120，并说明「余量 8、仅拦整块丢失」 | 规格 §3.8.5 A5 | [x] |
| 6 | A2 措辞改「按特性名分组，vault ≤ 6，非 vault 暂无上限」 | 规格 §3.8.5 A2 | [x] |
| 7 | A3 补实测 25 行与 15 行松弛的缺口说明 | 规格 §3.8.5 A3 | [x] |
| 8 | §9 边际成本改「实测 10 个装配点（后端 5 / 前端 3 / ACL 2）」 | 规格 §9 | [x] |
| 9 | 6 条「文档有、代码无」逐条标 `TODO(未实现)`：`__settingsSnapshotMeta`、`after` 锚点、空分类抛错、归属推导、后端注册表闭合、前端同义反复 | 规格 §3.3/R2、§3.5、§3.6、§6.2 | [x] |
| 10 | 统一命名：`conf`→`confs`、`FeatureConfSlot`→`ConfSlot`、`CONF_SIDE_EFFECTS`→`CONF_APPLIERS`、`VAULT_CONF_SLOT`→`vaultConfSlot` | 规格 §3.1 / §3.2 / §3.10 / §3.8.4 | [x] |
| 11 | 装配点数量三处口径互斥（4–5 / 11 / 10）统一为 10 + 本体 2 | 规格 §10.1 | [x] |
| 12 | 新增「已知残留与技术债」表（T1–T6） | 规格 §12 | [x] |
| 13 | 修正证据路径笔误（`ViolationRule` 在 `types/snapshot.ts`） | 本文 §7 | [x] |

### 8.2 代码部分（按排期落地）

| # | 动作 | 触发条件 | 状态 |
|---|---|---|---|
| C1a | 逐命令求交断言（S1 止血）：`build.rs` / `commands.json` / `permissions/<name>/default.toml` 三方 ↔ `vault_plugin.rs` 双向求差，并把「蛇形 → 短横线」派生写成显式常量 | 0.5–1 天；必须先落地，S2 期间作护栏 | [x] 已由 A7 承接（`command_to_permission` / `permission_to_command` 正反向用例 + 注册表 ↔ 构建期 ACL 双向求差 + 解析器下限 `ACL_PARSER_FLOOR = 60`）；S2 落地后 `commands.json` 与 `default.toml` 两侧消失，A7 改为「注册表 ↔ `gen/schemas/acl-manifests.json`」单点求交 |
| C1b | 配置收敛（S2 治本）：`build.rs` 从 `vault_plugin.rs` 派生、`default.toml` 改为生成物、删除 `commands.json` | 1–2 天；须在 C1a 之后；方案见 §9 | [x] 已落地（偏差见 §9.6）：③ 未生成 toml，改用 `DefaultPermissionRule::AllowAllCommands` 让框架直接烘焙 `default` 权限集，`permissions/vault/default.toml` 与整个 `permissions/` 目录删除；④ `commands.json` 删除 |
| C2 | 后端 `conf_side_effects()` group 集合 == `assistant-config` struct 集合 | 当前完全缺失，验收表曾虚报 | [x] 已由 A8 承接：`config_conf_groups()` 解析 `assistant-config/src/lib.rs` 的 `pub mod *_conf`，与 `register_conf_applier("...")` 字面量双向求差，两侧为空集才算通过 |
| C3 | 删除 `src/features/vault.ts` 死代码（顺带消除同名文件/目录共存） | 无依赖，随时可删 | [x] 已删除；全仓无 `VAULT_ENABLED` 消费点，同名文件/目录共存的过渡形态一并消除 |
| C4 | 空分类静默剪枝：改代码抛错 或 改文档承认剪枝 | **需决策**，两者互斥 | [x] 已定档（v2.8）：选择「改文档承认剪枝」——空分类不进快照是有意行为，原「无页面 → 抛错」目标撤销；理由（分步开发的中间态必然违规、后果仅为设置页少一个分组标题）见规格 §3.6「页面分类」行 |
| C5 | 后端 `register_conf_applier()` 重复 group 检测 | 出现同 group 双注册场景前可暂缓 | [x] 已由 A8 承接：同一 group 注册多次即报 `conf applier 分组重复注册` |
| C6 | `after` 锚点 / page 归属推导 / `__settingsSnapshotMeta` | 当前无用例，按需排期 | [ ] |
| C7 | 前端「槽位键集合 == `getConfGroups()`」断言替换为有鉴别力的形式 | 当前同义反复，可与 C2 同批处理 | [x] 已由 `src/settings/__tests__/confSlotContract.test.ts` 承接：内置槽位 / 特性槽位 / 全局唯一性三条断言均解析真实 Rust 注册表（`src/test-utils/rustCommands.ts`），不再是同一 `Map` 的两个读法；全仓已无 `getConfGroups()` 同义反复用例 |

---

## 9. 配置收敛执行方案（v2.6 决策）

决策：C1 由「补三方求交断言」改为**配置收敛**。断言只是过渡期护栏，目标是让命令清单回到「1 份手写真源 + 2 份生成物 + 0 份冗余」。设计依据见 `docs/store/I24-feature-extension-spec.md` §11.2；本节只写执行步骤。

### 9.1 现状与必然性判定

| # | 位置 | 消费者 | 阶段 | 必然性 | 处置 |
|---|---|---|---|---|---|
| ① | `src-tauri/src/vault_plugin.rs` | `generate_handler!` 宏展开 | 编译期 | 宏只接受函数路径，不接受运行时数据 | **保留为唯一真源** |
| ② | `src-tauri/build.rs` | `InlinedPlugin::commands()` 生成 ACL schema | 构建期（编译前） | 宏尚未展开，拿不到 ① | S2 改为从 ① 派生 |
| ③ | `src-tauri/permissions/vault/default.toml` | Tauri ACL 授权链 | 构建期烘焙 | 内容与 `gen/schemas/acl-manifests.json` 的 66 条逐字相同，纯冗余 | S2 改为生成 |
| ④ | `src-tauri/commands.json` | 仅 A4 断言的 `declared` 侧 | 测试期 | 不参与编译/运行/ACL，为本项目自建夹具 | S2 删除 |

### 9.2 S1 止血（必须先落地）

1. `src-tauri/tests/assembly_contract.rs` 新增一条逐命令求交断言（沿用 A1–A6 命名风格）：
   - ① 解析 `src/vault_plugin.rs`，正则提取 `crate::cmd::vault::<name>` 并去重；
   - ② 解析 `build.rs` 的 `InlinedPlugin::new().commands(&[...])` 字面量；
   - ③ 解析 `commands.json` 的 `groups.vault`；
   - ④ 解析 `permissions/vault/default.toml` 的 `allow-vault-*`，按「去 `allow-` 前缀 + `-` → `_`」逆变换回蛇形；
   - 四集合两两双向差集，失败时输出具体差异项而非只报数量。
2. **派生规则常量化**：把「命令名 → 权限标识符」写成显式函数（`allow-` + `name.replace('_', "-")`）并配一条正向用例，避免求交逻辑本身成为第 5 份隐式约定。
3. **解析器自检下限**：沿用 A5 的做法，断言四个集合条目数各 ≥ 66。否则正则失效会产出空集合，「恰好相等」假绿。
4. 验收方式：手工删掉 `default.toml` 中任意一条 `allow-vault-*`，断言必须失败；恢复后必须通过。

S1 完成后副本数仍为 4，但漂移必在 CI 暴露。

### 9.3 S2 治本（须在 S1 之后）

1. **② 变生成物**：`build.rs` 读取 `src/vault_plugin.rs` 文本提取命令名，传给 `InlinedPlugin::commands()`。必须补 `println!("cargo:rerun-if-changed=src/vault_plugin.rs")`，否则改插件文件不触发 build.rs 重跑。
2. **③ 变生成物**：
   - 首选：`InlinedPlugin::default_permission(DefaultPermission::new().allow(...))`（`tauri_build` 是否暴露该 API 需在开工时先验证）；
   - 退化：`build.rs` 自行写出 toml 文本。
3. **④ 删除**：删 `commands.json`，A4 的 `declared` 侧改为解析 `vault_plugin.rs`。
4. S2 完成后，S1 断言降级为「生成物与真源一致」的单点断言，或直接删除。

### 9.4 风险与开工前必须确认的两点

- **生成物落盘位置**：若把 `permissions/vault/default.toml` 生成为**源码树文件**，每次构建都会改写工作区（git 脏、可能引发格式化/审查噪声）。开工前必须先确认是「写 `OUT_DIR`」还是「入库生成物」，再决定是否加 `.gitignore`。
- **多特性泛化**：S2 的派生逻辑应按「每个 `plugins/*.rs`」通用化，否则下一个特性又要手写一遍。当前只有 vault，可先单点实现，但接口按多特性设计。
- 其余风险（正则脆弱性、`DefaultPermission` 可用性、回退路径）见规格 §11.2.2。

### 9.5 与其他条目的关系

- **与 C2 无关**：C2 是后端 conf applier 注册表闭合，不涉及命令清单。
- **与规格 §11.1 窗口分级无关**：窗口分级改的是 `capabilities/default.json` 里「哪些窗口拿到 `vault:default`」，不改 `permissions/vault/default.toml` 的命令集合。
- **与 DTO 层无关**：DTO 只定义参数/返回值形状，不参与 ACL 授权链。

> 本轮（v2.6）仅更新文档，C1a / C1b 均保持 `[ ]`，未做任何代码改动。

### 9.6 实施结果（v2.7）

S1 + S2 已全部落地，最终形态为 **1 份手写真源 + 0 份手写副本**（比原计划「1 + 2 份生成物」更少一层）：

| # | 计划动作 | 实际落地 | 偏差说明 |
|---|---|---|---|
| ② | `build.rs` 从 `vault_plugin.rs` 派生 | `plugin_commands()` 按 `generate_handler![` 块 + `crate::cmd::vault::` 前缀提取命令名并去重；补 `cargo:rerun-if-changed=src/vault_plugin.rs`；空集合直接 panic | 与计划一致；未用正则，改为「先定位注册块、再按前缀过滤」，避免注释/别名干扰 |
| ③ | `permissions/<name>/default.toml` 改为生成物 | 改为 `InlinedPlugin::default_permission(DefaultPermissionRule::AllowAllCommands)`，由 `tauri-build` 直接烘焙 `default` 权限集；`permissions/vault/default.toml` 与整个 `permissions/` 目录删除 | 优于计划：不再需要「生成 toml 文本」这条退化路径，也不必决定生成物落盘位置（`OUT_DIR` / 入库）——**没有生成物文件** |
| ④ | 删除 `commands.json` | 已删除；A4 的 `declared` 侧改为直接读插件注册表 | 与计划一致 |

配套改动：

- **A4 重写**为 `a4_plugin_registry_and_acl_grants_match_both_ways()`：插件注册表 ↔ 构建期 ACL（`gen/schemas/acl-manifests.json`）双向求差 + 跨特性重复命令检测；
- **A6 重写**为 `a6_registration_partition_and_stub_signature()`：宿主（`ipc.rs`）/ 插件注册分区互斥 + `vault_cleanup_on_exit` 桩签名归一化；
- **A7 新增**为「注册表 ↔ 构建期 ACL」逐命令求交 + 派生规则正反向用例 + `ACL_PARSER_FLOOR = 60` 下限；
- **A8 新增**为「conf applier 分组 ↔ `assistant-config` 结构体」双向求差 + 重复注册检测（同时关闭 C2 / C5）；
- **前端解析器收敛**为 `src/test-utils/rustCommands.ts`（`coreCommands()` / `pluginFeatures()` / `pluginCommands()` / `commandGroups()`），`commandNames.test.ts`、`confSlotContract.test.ts`、`coreBoundary.test.ts` 共用同一份实现，不再各自解析 Rust 源码；
- `src/features/__tests__/coreBoundary.test.ts` 的装配点从 10 条降为 **8 条**（`commands.json` 消失、`permissions/<name>/default.toml` 不再是必填项），并允许「`build.rs` 声明 `.default_permission(`」替代手写权限集。

验收与反证：

- 后端 `assembly_contract` A1–A8 全绿；前端 9 条命令/边界/配置槽位用例全绿；
- 反证：临时删除 `vault_plugin.rs` 中的 `vault_init` 条目，前端 2 条断言立即报出缺失命令，确认断言非空转；恢复后复绿；
- `tsc --noEmit` / `eslint` / `prettier` / `cargo fmt` 全部通过。
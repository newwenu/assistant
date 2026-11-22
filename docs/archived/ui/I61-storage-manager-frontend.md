# I61 存储管理器 · 前端设计（T4）

> 性质：**前端实现设计（T4 唯一实现基线）**｜上位契约：[I61-storage-manager-spec.md](./I61-storage-manager-spec.md)（§1.1 / §3 / §5 / §6.1 / §6.2 / §7 / §10 / §11）
> 拆分判据：[CODE-SPLIT-PRINCIPLE.md](../../principle/CODE-SPLIT-PRINCIPLE.md)（只引用，不复述）
> 任务清单：[I61-storage-manager-tasklist.md](./I61-storage-manager-tasklist.md)（T4-a / T4-b）

本文只回答「前端怎么做」：组件边界、视图模型、交互状态机、视觉与文案、测试方式。

**口径变更（2026-11-21）**：规格 §6.3「前端」已**整体划出作废**（原文划掉保留，仅供追溯），前端口径不再以该节为准——**本文即前端唯一实现基线**，页面结构 / 列与分组 / 清理流程 / 余量段 / 文案键树 / 样式全部由本文定义。规格中仍然有效的上位约束只剩后端侧：条目与观测（§3）、回收语义（§5）、命令与 DTO（§6.1 / §6.2）、失败降级（§7）、验收判据（§10）。**判据、字段语义、资格与档位来源仍以规格为准**，这些内容与规格冲突时以规格为准并回改本文。

---

## 0 边界声明

| 事项 | 归属 | 本文的态度 |
|---|---|---|
| 条目集合与条目观测值、`availability`、`reclaim` 视图 | 规格 §3 / §6.1 | **只渲染后端返回值**，不推算、不兜底成「可用」；**不假设条目数量与名称**（§5.2 / §5.5） |
| 页面壳、三 tab 容器、页标题、导航 | I54 §3（R0 建容器） | **不设计**。本文只设计 `storage` tab 的内容区 |
| 资格（`eligibility`）与确认档位（`confirmation`） | 规格 §3.3 / §3.6 | **只信后端**，禁止按 `expectedBytes` 自行升降档（档位由后端 `confirmation` 给定，见本文 §6.2）；资格不单独成列，唯一可见载体是清理按钮的可用态（§5.7） |
| 可清理字节 / 是否安全 | 后端计划 | 前端**只展示** `reclaim.reclaimableBytes`，不做任何加减比较 |
| 删除附件（`A2`） | 既有附件明细区 | 保留原入口与原命令（并入段一的区块 A），条目表**不**开第二套入口（本文 §3.4；后端依据是 §1.4 的「`delete_attachments` 是既有例外」） |
| 清理（`A1`/`A3`/`A4`/`A5`） | 规格 §3.3 / §6.1 | **本期提供行内「清理」按钮**（区块 B 第 4 列），固定走 `get_storage_reclaim_plan` → 确认 → `reclaim_storage`（§6）；可用性不单独成列，收敛到占用列角标与详情（§5.4） |
| 未登记占用（规格称「对账余量」） | 规格 §3.11 | 只读展示，无删除入口、不进任何合计（§3.11 理念 5 / 本文 §7.4） |
| 数据来源 | `get_storage_breakdown` 一次拉全 | 不新建 store、不新建 service、不新增轮询（§1.1 对外唯一入口 / §3.11.4 通知复用；本文 §8） |

### 0.1 口径沿革（原「偏差登记」，§6.3 划出后即基线）

本节原为「与规格的偏差登记」。规格 §6.3 已于 2026-11-21 整体划出作废，D1 / D2 不再需要规格侧同步登记，**直接作为前端基线**；保留此表只为记录沿革，防止后续读者把 D1 / D2 当成缺陷重新引入。

| # | 原登记项 | 原规格原文 | 现行口径 | 理由 |
|---|---|---|---|---|
| D1 | 可用性与资格不单独成列 | §6.3 第 3 条（已划出） | 列结构收敛为 4 列（名称 / 占用·条数 / 策略 / 清理）：`availability` 以占用列角标 + 详情「诊断」行表达，`eligibility` 以清理按钮的可用态表达（§5.4 / §5.6 / §5.7） | 一列一信息：可用性只有异常值才需要解释，资格的唯一用途就是「按钮能不能点」，两者各设一列等于把同一结论说两遍。信息不减少、命令与档位不变 |
| D2 | 不提供全局全选 | 无（原拟追加） | 附件区**只有**组全选（既有能力），**不**新增跨组全选与「已选 N / 共 M」选择条（§4.2） | 全局全选把「删全部附件」压缩成一次点击，与 `A2` 的 `confirm` 档风险不匹配；逐组勾选保留了人为停顿 |

**本期不降级任何能力**：计划命令、`reclaim_storage`、`confirmation` 档位、`ReclaimViewDto` 字段全部被消费（§6），`src/services/desktop/storageService.ts` 的三个封装（`getStorageReclaimPlan` / `reclaimStorage` / `refreshStorageInventory`）全部在用（§1 末段）。

---

## 1 旧设计作废说明

被作废的是「在 `StorageSettings.tsx` 里线性追加两张表」的做法（含上一轮草稿的 CSS 追加与扁平 i18n 键）。逐条列出问题与本文的替换方案：

| # | 旧做法 | 问题 | 本文替换 |
|---|---|---|---|
| P1 | 290 行单文件继续线性增长，状态、IO、渲染、文案全在一处 | 一个文件对应四种变化原因（数据拉取 / 表格布局 / 清理流程 / 文案），违反拆分主判据 | 拆成 17 个文件，见 §9；页面文件退化为薄壳 |
| P2 | i18n 键扁平混放（`entry_names.*` 与 `inventory.*` 同级） | 无法表达「列 / 徽标 / 详情 / 弹窗 / 结果」层级；且缺键无法被类型检查发现（`I18nPath` 只由 zh-CN 推导） | 命名空间键树 + 枚举全覆盖测试 + zh/en 键路径对等测试，见 §11 |
| P3 | 6 列 `minmax` 等宽 grid，无表头、无分组、无详情 | 13 条条目平铺，`reason` / `note` / `effectiveSince` 无处可放；安全域五条夹在中间难以收拢 | 自绘表格：默认扁平行 + 仅安全域一个可折叠组 + 4 列固定语义 + 单行手风琴详情，见 §5 |
| P4 | 徽标、策略、原因文案散在 JSX 三元里 | 枚举增删时静默显示空白；无法测试 | `lexicon.ts` 纯映射 + `entryView.ts` 纯视图模型，见 §4 / §5.6 |
| P5 | 余量区与条目表混在一个滚动区，无状态层级 | 扫描失败 / 部分扫描 / 超登记三类异常没有位置，容易静默 | 独立段二 + 警示栈，见 §7 |
| P6 | 清理流程没有状态机，错误码直接 `catch` 后 toast 原文 | `planExpired` 被当成通用失败；可能重复提交 | `cleanupFlow.ts` 纯 reducer + 错误码单点映射（本文 §6.1 / §6.3）；`planning` / `executing` 期间禁用其余行的清理按钮，避免并发 `busy` |
| P7 | 事件监听的去重与清理未定义 | 事件与手动刷新叠加时重复请求 | `useStorageSnapshot.ts` 单飞 + 卸载清理，见 §8 |
| P8 | CSS 只有布局，没有状态修饰符 | 禁用 / 警示 / 超登记三种状态视觉无差别 | 状态修饰符 + 既有令牌，见 §10 |
| P9 | 首载失败与刷新失败都 `setBreakdown(null)` | 刷新一次抖动就把整页清空，用户丢失上下文 | 两态分离：首载失败空态重试；刷新失败保留上次数据 + 顶部提示，见 §8.3 |
| P10 | `bytes = null` 与 `missing`（`0`）未区分 | `null` 直接进 `formatBytes` 会得到 `NaN`；`missing` 显示成未知 | 占位符规则表，见 §5.3 |
| P11 | 与 R0 容器关系未定义 | T4 交付后可能二次搬迁 | `StorageSettings.tsx` 只做薄壳，内容组件与容器解耦，见 §9.1 |
| P12 | 条目表 `max-height: 400px` + `overflow: auto`，余量表同样 400px（规格从未要求） | 形成三层滚动（页面 / 条目表 / 余量表），滚轮在表内被 `overscroll-behavior: contain` 吞掉，页面不动；`sticky` 表头只对 400px 容器生效，与页面滚动位置错位 | 单流滚动：页面唯一滚动容器，表内零滚动，见 §3.6 / §5.8 |
| P13 | 附件区只剩「一条裸账本行 + 组列表」 | 无区块头、无孤儿标识、无已选反馈；与条目表的列表观感雷同，读者分不清哪块是「干活」哪块是「看数」 | 区块 A 升级为「清理附件」面板（区块头 + 账本摘要 + 孤儿徽标 + 已选计数），区块 B 定位为条目清理（行内清理），见 §3.4 / §4.2 |
| P14 | 条目表每行恒为两行（`key` + `reason` 常驻） | 13 行约 750px，加上 400px 内嵌滚动，一屏只能看到半张表；`key` 常驻主行挤占空间，而它只在强确认时才需要 | 行高按需（无第二行文案时 32px），`key` 下沉详情，见 §5.4 / §5.9 |
| P15 | `E1` 行的「清理孤儿」（A1）与附件区的「删除选中」（A2）看起来是同一件事 | 两条命令语义不同（A1 按判据删全部孤儿、`strongConfirm`；A2 删用户勾选的文件、`confirm`），若两处都叫「删除」，用户不知道点哪个 | **一条命令 = 一个入口 = 一种措辞**：A2 只在区块 A，叫「删除选中」；A1 只在区块 B `attachments` 行的「清理」按钮，弹窗正文写明「按判据删除全部无关联附件」；`E1` 行不再提供第二种删除措辞（§3.4 / §3.5 / §5.7） |
| P16 | 策略列对 `policy.source = none` 显示「无策略」 | 「无策略」+ 右侧可点「清理」= 语义矛盾（用户读成「没策略所以不该删」，而按钮又摆在那里） | 文案改「无保留策略」；`failClosed` 是后端字段，**不翻译、不渲染**（不进策略 chip、不进详情）；清理按钮的可点性**只由 `eligibility` 决定**，`policyDisabled` 等策略类原因写进禁用 tooltip，矛盾两端各有出处，见 §5.6 / §5.7 |
| P17 | 段一、段二各自一个卡片 + 段头 + hint，两套视觉层级 | 两段职责同属「本机存储占用」，并列等于让用户先做一次无意义的分类判断；两段都像列表，重复观感明显 | **合并为一段两区块**：一个段头（「存储占用」）+ 区块 A（附件明细，勾选列表）+ 区块 B（条目清理），见 §3.3 |
| P18 | 条目集合在前端被写死：`ENTRY_KEYS`（13 项）、`key → E1..E13` 映射、静态域分组顺序表 | 后端登记表新增 / 更名条目时，前端会把它归进「其他」组、名称显示成 `key` 原文并打 `console.warn`——**新增条目被当成异常**，必须改前端才能正常显示 | 条目集合完全由 `entries` 数组驱动：默认扁平行、仅安全域按 `COLLAPSIBLE_GROUPS` 置末折叠、名称走「可选覆盖 + 原文兜底」、规格编号 `E*` 不参与渲染，见 §5.2 / §5.5 / §5.7 |

**草稿处置**：上一轮已落地的 `settings.css` 追加（`storage-section-*` / `storage-entry-*` / `storage-inventory-*`）与 `zh-CN` / `en-US` 中的扁平草稿键（`entries_title` / `col_*` / `entry_names` / `reclaim.*` / `inventory.*`）**同属被作废的旧设计**：键按 §11.1 重排（`reclaim.*` 本轮更名为 `cleanup.*`）、样式按 §10 重写（`storage-inventory-*` 更名为 `storage-residual-*`），随 §15 第 1 / 9 步一次性替换，替换前不得被任何组件引用。`src/services/desktop/storageService.ts` 的三个命令封装保留（它们是契约镜像，与设计无关）。

---

## 2 设计目标

### 2.1 用户任务（按发生频率排序）

| 任务 | 用户此刻关心 | 对应位置 |
|---|---|---|
| 清附件腾空间 | 哪些附件没用、删了释放多少 | 段一 · 区块 A「附件明细」（勾选删除，`A2`） |
| 看清各目录谁在占、能不能清 | 这条是什么、占多少、为什么不能清 | 段一 · 区块 B「条目清理」（条目数由后端决定，行内「清理」按钮执行 `A1`/`A3`/`A4`/`A5`） |
| 排查「磁盘比条目加起来还大」 | 有没有没被登记的东西、扫描可信吗 | 段二「未登记占用（只读）」 |

顺序即上表顺序：**最常用的操作在最上面**，诊断信息在最下面。原「段一 / 段二」合并为**一段两区块**（§3.3）：附件明细是段一里唯一执行 `delete_attachments` 的区块；区块 B **不执行** `delete_attachments`（本文 §3.4），它唯一的动作是行内「清理」按钮（`A1`/`A3`/`A4`/`A5`，§6）；段二只做未登记占用的观测，不提供任何清理（§7）。

### 2.2 三条不可让步的原则

1. **数字只有一个来源**：页面所有数字来自同一次 `get_storage_breakdown` 快照（含 `inventory`），不拼接第二次请求，不缓存第二份。
2. **分支不进组件**：所有「该显示什么 / 该用什么措辞」的判断由纯函数算出视图模型，组件只做「视图模型 → JSX」映射（理由见 §2.3：本仓库测试环境无 DOM 渲染器）。
3. **异常必须可见**：`unreadable` / `notImplemented` / `overRegistered` / 扫描失败，每一类都有确定的展示位置，不允许只打日志或静默取 0。

### 2.3 为什么必须「纯函数出视图模型」

仓库测试栈是 `vitest` + node 环境（`src/testSetup.ts` 只补 `window` 空对象），**没有** `jsdom` 与 `@testing-library/react`。因此：

- 组件层不可测，纯函数层可测；
- 若把分支写在 JSX 里，§13 的全部断言都只能靠人眼回归；
- 所以 `entryView.ts` / `residualView.ts` 承担全部判断，测试直接吃 DTO 假数据（无需渲染）。

---

## 3 信息架构

### 3.1 结构（内容区）：两段

```
┌ 快照工具条（吸顶 top:0，高 40px，横跨内容区）──────────────────────┐
│ 采样于 HH:mm · 口径：文件长度                              [刷新] │
└────────────────────────────────────────────────────────────────┘
╭ 段一 存储占用（卡片）───────────────────────────────────────────╮
│ 段头：存储占用 · hint              共 N 个附件 · X · M 条条目           │
│ ┌ 区块 A 附件明细（勾选删除，A2）─────────────────────────────┐ │
│ │ 区块头：附件明细 · hint       共 N · X · 共享 Y   [删除选中(N)] │
│ │ 警告条（> 500 MiB 才出现）                                    │ │
│ │ 组头：▾ 孤儿组 [无关联对话]      N 项 · X · 更新时间           │ │
│ │   文件行：勾选 · 文件名 · 大小 · 类型 · 时间                   │ │
│ │ 组头：▾ 会话组 A …  组头：▾ 会话组 B …                        │ │
│ └────────────────────────────────────────────────────────────┘ │
│ ┌ 区块 B 已知条目清理（行内清理，A1/A3/A4/A5）──────────────────┐ │
│ │ 区块头：已知条目清理 · hint                                   │ │
│ │ 表头（吸顶 top:40）：条目 · 占用/条数 · 策略 · 清理            │ │
│ │ 组：attachments ▾ (2)                                         │ │
│ │   attachments         无关联对话 N 项 · X   …        [清理]    │ │
│ │   attachments/derived                       …        [清理]    │ │
│ │ 组：security ▾ (5)  ← 仅当 entries 含 security/* 条目；未挂载则整组不出现 │ │
│ └────────────────────────────────────────────────────────────┘ │
╰────────────────────────────────────────────────────────────────╯
╭ 段二 未登记占用（卡片，只读）──────────────────────────────────╮
│ 段头：未登记占用 [只读] · 状态文案                        [扫描]  │
│ 警示栈：扫描失败 / 部分扫描 / 超登记                              │
│ 表头（吸顶 top:40）：名称 · 类别 · 范围 · 扫描 · 登记 · 余量      │
│ 节点行 / children 行                                             │
│ 合计行：未登记合计 N 项 · X（不计入任何页面合计）                 │
╰────────────────────────────────────────────────────────────────╯
```

唯一滚动容器是 `AppShell` 的 `Layout.Content`（`overflow: auto`）。本页**不产生任何内嵌滚动**：两段随页面自然展开，两张表不设 `max-height` / `overflow` / `overscroll-behavior`（§3.6）。

**合并的依据与边界**：原规格 §6.3 曾要求「同页两段」（附件明细 + 条目表）并追加第三段「对账余量区（只读）」；该节已于 2026-11-21 整体划出（§0.1），页面结构改由本文定义。本文在界面文案上称**未登记占用**（`residual` 键树不变），并把原前两段合并为**一张卡片、两个区块**：

- 区块 A = 原附件明细区原样延续（G14 / V16 的九项能力，§4.1），只从「独立段」降为「段内区块」，且**唯一**执行 `delete_attachments`；
- 区块 B = **已知条目清理**：4 列（名称 / 占用·条数 / 策略 / 清理），行内「清理」按钮执行 `A1`/`A3`/`A4`/`A5`（§6）。资格只由按钮可用态表达、可用性只由占用列角标表达，**都不占列**（§0.1 D1）；
- 段二 = **未登记占用**：只读，除段头扫描按钮外无任何动作，不参与清理（§7.4）；
- 原 §6.3 第 7 条「不得把附件明细区的删除按钮搬进条目表」的意图继续成立（本文 §3.4）：区块 B 的按钮是「清理」（plan → `reclaim_storage`），区块 A 的按钮是「删除选中」（`delete_attachments`），两条命令、两种措辞（§3.4）；
- 合并只动布局，不改任何命令、DTO 或能力契约。

### 3.2 段头的统一形状

每段段头都是「标题 + 一句 hint（说明这段是什么、数字口径）+ 右侧段级信息 / 操作」，不重复页标题、不重复页级刷新（刷新在工具条）。区块头（区块 A / 区块 B）是段头之下的一级，形状为「区块标题 + 一句 hint + 右侧区块级操作」，字号比段头小一档。

### 3.3 一段两区块的分工（视觉与职责）

同一张卡片内用**内容形态**而不是「并列两段」来区分职责：

| 区块 | 内容形态 | 数据性质 | 主操作 | 选择态 |
|---|---|---|---|---|
| A 附件明细 | **可勾选列表**（组头 / 文件行），无列头、无数字列 | 文件级账本（`conversations` / `orphans`） | 区块头右侧危险按钮「删除选中（N）」 | 有（`selectedIds`，只服务 `delete_attachments`） |
| B 已知条目清理 | **四列对齐表**（扁平行 + 安全域折叠组 + 详情手风琴） | 条目级观测 + 清理（`entries`） | 行内「清理」按钮（`A1`/`A3`/`A4`/`A5`，§6） | 无 |

区块之间的分隔只有一条 1px 细线（`--color-border-secondary`）+ 12px 内边距，不另起卡片、不另起段头：读者读到的层级是「存储占用 → 附件明细 / 已知条目清理」而不是「两个不相干的功能」。

### 3.4 职责边界（去重矩阵）

「两段功能重合」的根因是同一批附件文件有两条看起来一样的删除路径。按「**一条命令 = 一个入口 = 一种措辞**」重排：

| 对象 | 语义 | 命令 | 唯一入口 | 出现在另一处时 |
|---|---|---|---|---|
| 附件文件（`A2`） | 用户勾选后删文件，`confirm` 档 | `delete_attachments` | **区块 A** 的「删除选中（N）」 | 区块 B `attachments` 行只给孤儿数量与账本数字，**没有**删除按钮 |
| 孤儿附件（`A1`） | 按判据清**全部**无引用附件，`strongConfirm` 档 | `get_storage_reclaim_plan` → `reclaim_storage` | **区块 B** `attachments` 行的「清理」 | 区块 A 的孤儿组头只提示「可用清理按钮一次清空」，不在组头放按钮 |
| 目录级清理（`A3`/`A4`/`A5`） | owner 按判据清理 | plan → execute | **区块 B** 行内「清理」按钮 | 区块 A 不出现这些动作 |
| 未登记余量 | 只读对账 | 无 | **段二** | 区块 A / B 都不出现余量数字 |

配套规则：

- 区块 A 与区块 B **不共享选择态**：区块 A 的 `selectedIds` 只服务 `delete_attachments`，区块 B 没有任何多选，只有行内「清理」按钮；
- 孤儿的两处数字（区块 A 孤儿组头 / 区块 B `attachments` 行）来自同一次快照，必然一致；**区块 B 那处必须点名区块 A**（§3.5），避免读者以为两处都能删；
- 区块 B 的区块头 hint 必须写清「本表按条目清理；附件文件的逐个删除在上方区块 A」；
- 段二是**未登记占用**：`unregistered` 不是「可清理」，它连 `reclaim` 字段都没有；段二**没有**任何行内按钮（§3.11 理念 5；本文 §7.4）。

### 3.5 两条删除路径的措辞（写进文案，不靠用户猜）

本期页面有**两条**会删除文件的路径，措辞必须让读者一眼分清「哪个按钮删什么」：区块 A 删**用户勾选的附件文件**（`delete_attachments`），区块 B 清**某条目按判据登记的产物**（`reclaim_storage`）。两者互不覆盖，同一行不允许出现两个删除入口。

| 位置 | 文案键 | zh-CN |
|---|---|---|
| 区块 A hint | `attachments.hint` | 按对话分组；删除只移除你勾选的文件 |
| 区块 A 按钮 | `attachments.delete_selected` | 删除选中（{{count}}） |
| 区块 B `attachments` 行次行 | `entries.attachment_orphans` | 无关联对话 {{count}} 项 · {{size}}（逐个删除请在上方区块 A 勾选） |
| 区块 B 清理按钮 | `entries.cleanup_button` | 清理 |
| 区块 B 区块头 hint | `entries.hint` | 条目由各自 owner 提供观测；行内「清理」只清该条目登记的产物，附件文件请在上方区块 A 勾选 |

`attachments` 行次行的括号说明**只在 `orphans.length > 0` 时出现**；`orphans = 0` 时整行没有次行，也不需要指向区块 A（没有可删的孤儿，指引反而制造疑问）。

### 3.6 滚动模型：单流 + 两处吸顶

| 层 | 旧行为（问题） | 新行为 |
|---|---|---|
| 页面（`Layout.Content`） | 唯一滚动容器，但同时被两张表的内嵌滚动打断 | **仍是唯一滚动容器**，且是唯一的滚动来源 |
| 区块 B 表格容器 | `max-height: 400px` + `overflow: auto` + `overscroll-behavior: contain` | **删掉这三条**；表格随内容自然展开，不裁剪、不内嵌滚动 |
| 段二表格容器 | `max-height: 400px` + `overflow: auto`（规格从未要求） | 同上 |
| 区块 A 组列表 | 无内嵌滚动（既有行为） | 保持不变 |
| 横向 | 表格 `min-width: 960px` → 窄窗时表内横向滚动（第二个滚动容器） | **取消 `min-width`**；列宽用 `minmax(0, …)` 流式分配，窄窗时名称 / 策略列省略号 + tooltip（全文在详情），**永不出现横向滚动条** |

吸顶只保留两处，且都相对**页面滚动容器**生效：

1. 快照工具条：`position: sticky; top: 0`，固定高 40px，背景不透明（`--color-bg`）+ 下边框；吸顶时横向铺满内容区（`margin-inline: calc(var(--settings-page-pad) * -1)`、`padding-inline: var(--settings-page-pad)`）。
2. 区块 B 与段二的**表头**：`position: sticky; top: var(--storage-sticky-top)`（= 工具条高 40px），`z-index` 低于工具条；`sticky` 的包含块是该表的容器，因此表头只在所属卡片可见期间吸顶，卡片结束后自然让位，**不会**出现两条表头叠在页面顶部。

明确**不做**吸顶的元素与理由：

| 元素 | 旧行为 | 新行为 | 理由 |
|---|---|---|---|
| 安全域折叠组头 | `sticky top: 32px`（为 400px 容器设计） | 普通行，不吸顶 | 组头吸顶需要第二层偏移（工具条 40 + 表头 32），三层吸顶会吃掉 72px 视口；条目表规模小（core 条目 + 已挂载分区条目），不需要 |
| 段头 / 区块头 | 无 | 不吸顶 | 卡片本身提供了视觉边界，段头随卡片滚动即可 |

代价与取舍：页面总高变长（条目表约 640px、余量表按节点数增长），换来「滚轮永远作用于页面」与「表头与页面滚动同步」。规格从未规定表格高度，原 §6.3 已划出，滚动模型由本文本节决定，不改任何能力契约。

---

## 4 段一 · 区块 A：附件明细（勾选删除，`A2`）

### 4.1 保留的能力（V16 逐项回归基线）

| 能力 | 现状 | 本文处置 |
|---|---|---|
| 会话分组（按 `totalBytes` 降序） | 有 | 保留，逻辑原样搬入 `AttachmentBlock.tsx` |
| 孤儿分组（`orphans.length > 0` 才出现） | 有 | 保留 |
| 组折叠（`RightOutlined` / `DownOutlined`） | 有 | 保留 |
| 组全选（`checked` / `indeterminate`） | 有 | 保留 |
| 单选删除（`selectedIds` + 危险按钮） | 有 | 保留 |
| 删除确认弹窗（不可逆文案） | 有 | 保留（`modal.confirm`，`okButtonProps.danger`） |
| 删除后重拉 + 清空勾选 | 有 | 保留 |
| 占用 > 500 MiB 警告 | 有 | 保留（阈值提为常量 `SIZE_WARNING_THRESHOLD_BYTES`，值不变） |
| 首载失败空态 | 有 | 保留，文案键迁到 `common.load_failed` |

G14 / V16 要求「不得删除附件明细区的任何既有能力」：本区块的**能力集合与既有实现完全一致**，唯一的位移是「从独立段降为段一的区块 A」，不新增、不删除任何命令入口。

### 4.2 允许的「追加」（不删任何既有能力）

1. 区块头形态统一（标题 + hint + 右侧账本摘要 + 危险按钮），摘要从「无标题的裸行」升级为区块头右侧信息；
2. 孤儿组头追加**徽标**（`attachments.orphan_badge`：无关联对话），让「这组没人引用」在组头一眼可见；
3. 区块头追加「跨会话共享 X」（`sharedBytes > 0` 才显示，`OQ1` 允许单列）；
4. 删除成功追加 `message.success`（`removedCount` / `removedBytes` 来自 `AttachmentDeleteResultDto`，此前返回值未被使用）；
5. 删除失败追加 `message.error`（此前 `onOk` 抛错只让弹窗停住，无提示）；
6. 首载失败空态追加「重试」按钮（此前只能靠页头刷新，且空态下没有指向性）。

以上 6 条均为纯追加，`selectedIds` 语义、勾选行为、分组排序、确认文案、命令与 DTO 全部不变。

**明确不追加**（§0.1 D2）：跨组**全局全选**、独立**选择条**（「已选 N / 共 M」）、「全选孤儿」按钮。已选数量只体现在按钮文案「删除选中（{{count}}）」——数量非 0 时按钮本身就是选择反馈，再加一条选择条等于把同一信息说两遍，同时把「全删附件」压缩成一次点击。

### 4.3 账本的三个数字（不在前端校验）

规格 §10 V5 要求 `totalBytes = referencedOnceBytes + sharedBytes + orphanBytes`，但**该恒等式由后端保证**，前端不重算、不显示等式、不显示 `referencedOnceBytes`（`OQ1`：本期不单列）。前端只显示：

- `ledger.total`：共 `totalCount` 个附件，占用 `totalBytes`；
- `ledger.shared`：其中跨会话共享 `sharedBytes`（>0 时）；
- 孤儿组头的 `orphan` 数字：`orphans.length` 项 · `orphans` 字节和（与区块 B `attachments` 行次行同源，见 §3.5）。

口径与采样时刻（`byteBasis` / `sampledAt`）留在页级工具条（§8.1），不重复出现在区块头。

### 4.4 组件拆分

| 文件 | 职责（唯一变化原因） |
|---|---|
| `AttachmentBlock.tsx` | 区块头 + 账本摘要 + 警告 + 分组容器（迁移既有逻辑，原 `AttachmentSection.tsx`） |
| 组头 / 行渲染 | 留在 `AttachmentBlock.tsx` 内部（同一变化原因：附件列表的呈现），不额外拆文件；若迁移后仍超 300 行，再按「组头 / 行」拆分 |

---

## 5 段一 · 区块 B：已知条目清理

### 5.1 数据形状（只用这些字段）

`breakdown.entries` 是**唯一**的条目集合来源：条数、顺序、名称、分组全部由它决定，前端**不维护**任何条目白名单（P18 / §0.1）。

- 行集合 = `entries` 原样，**不增不删**（`availability` 不可用的条目也必须在场）；
- 行顺序 = `entries` 顺序，**不重排、不排序**（后端按登记表顺序返回，这就是稳定顺序）；
- 后端新增 / 更名条目 → 本表自动多一行 / 换一行，**前端零改动**即可显示（名称可能退化为 `key` 原文，见 §5.2；首段首次出现时按单成员组平铺，见 §5.5）。

行内只消费：

| 字段 | 用途 |
|---|---|
| `key` | 稳定身份（分组首段、名称覆盖查表、详情展示） |
| `layer` | 详情展示（`data` / `local`） |
| `owner` | 详情展示（`由 {{owner}} 提供`） |
| `bytes` / `count` | 占用 / 条数列 |
| `oldestAt` / `newestAt` | 详情 |
| `availability` | 占用列异常角标 + 详情「诊断」行（**不占列**，§5.3 / §5.6） |
| `includesDerived` / `countsSelfLogs` | 详情（口径提示 chip） |
| `reclaim.eligibility` / `.reason` / `.note` | 清理按钮可用态与禁用 tooltip（§5.7） |
| `reclaim.action` | 清理按钮文案来源（`actionText(action)`）；`action = none` 时不渲染按钮 |
| `reclaim.confirmation` | 弹窗强度（`confirm` / `strongConfirm`，§6.2） |
| `reclaim.autoActive` | 策略列 chip + 详情 |
| `reclaim.reclaimableBytes` / `.reclaimableCount` | 详情（后端给出的可清理量，只展示，不推算） |
| `reclaim.policy.{source, retentionDays, keepCount, maxBytes, effectiveSince, mutable}` | 策略 chip + 详情（`failClosed` 不消费，§5.6） |

**不消费**：`breakdown.complete`（本期不展示任何跨条目合计，故无守卫对象）、`totalReclaimableBytes`（一次只清一个条目，不展示计划合计，§6.1）。

### 5.2 名称：可选覆盖表 + `key` 原文兜底

条目 `key` 含 `/` 与 `.`（`attachments/derived`、`assistant.db`、`security/vault`），而 i18next 默认以 `.` 与 `/` 作路径分隔，**不能**直接作为 i18n 路径段。因此：

- 名称的 i18n 路径用 `key` 的**规范化形式**（`/`、`.`、`-` → `_`）：`attachments` → `entries.names.attachments`、`attachments/derived` → `entries.names.attachments_derived`、`assistant.db` → `entries.names.assistant_db`、`security/security.db` → `entries.names.security_security_db`；
- `lexicon.ts` 维护一张 `ENTRY_NAME_KEYS: Record<key, I18nPath>`，**按 `key` 索引**（不按规格条目编号 `E*`）；
- `entryNameText(key)`：命中 → 本地化名称；未命中 → **返回 `key` 原文，不 `console.warn`、不视为异常**。名称表是**可选覆盖层**，覆盖集合不要求穷举：后端新增条目缺名称键时页面照常可用，补名称只需加一条 i18n 键 + 一行映射，**不改组件、不改分组逻辑**；
- `E1`–`E18` 是**规格文档的编号**（core 自有 13 条 + 可选功能分区自述 5 条），`StorageEntryDto` 没有 id 字段，所以前端**不渲染 `E*`**：详情只显示 `key`，分组只用 `key` 首段（§5.5）。行集合与行数**完全由 `entries` 决定**：可选功能未挂载时 `security/*` 五条**根本不存在**于 `entries`，前端不补占位行、不显示「未启用」假行。

**不允许动态拼路径**：`translateKey` 的类型参数是静态 `I18nPath` 联合，写成 `translateKey(\`settings.storage.entries.names.${key}\`)` 会绕过类型检查（且违反 §12 的类型安全约束）。所以名称查表必须是静态映射表，而不是字符串拼接。

**`lexicon.ts` 的写法照抄 `src/audit/lexicon.ts`**，不另创一套：

- 每个**封闭枚举**（`availability` / `eligibility` / `reason` / `policy.source` / `action` / `residual.class` / `layer`）一张 `as const satisfies Record<Enum, I18nPath>` 表 —— **编译期**保证「新增枚举值必然有键」，这比运行期测试更强；
- **开放集合**（`key`、`key` 首段）用 `Record<string, I18nPath>` 的**可选覆盖表**，未命中不报错；
- `lookupKey(keys, value)` 用 `Object.prototype.hasOwnProperty.call` 判存在（不写成 `keys[value] ?? …`，避免 `constructor` / `toString` 之类的原型键误命中）；
- 导出函数返回**已翻译的字符串**（`translateKey(key)`），封闭枚举未命中时返回原始值并 `console.warn`；开放集合未命中时返回原始值且**静默**。`translateKey` 是 `src/i18n/index.ts` 里唯一的动态键入口（已登记的类型安全例外），本设计不新增第二处；
- 因此**组件不直接对枚举调 `t()`**，只渲染 lexicon 返回的字符串；`t()` 只用于带参数的静态文案（列头、按钮、区块头 hint）。

### 5.3 占位符规则（`null` 与 `0` 的区别）

| 情况 | 语义 | 占用列主行 | 次行（条数） |
|---|---|---|---|
| `bytes === null` | 取不到数 | `—`（`common.bytes_unknown`），**不调用** `formatBytes` | 隐藏 |
| `bytes === 0` 且 `availability = missing` | 目录确实不存在，计 0 | `0 B` | 隐藏（`count` 为 0） |
| `bytes > 0` | 正常 | `formatBytes(bytes)` | `{{count}} 项`（`count` 非 null 时） |
| `count === null` 且 `bytes !== null` | 条数不可得 | 正常 | 隐藏 |

规则以 `bytesOrDash()` 单点实现（`format.ts`），组件内**禁止**再出现 `formatBytes(x ?? 0)` 或 `x ? … : …` 的数值兜底。

**可用性不占列**：`missing` 只通过 `0 B` 表达，不加角标；只有 `unreadable` / `locked` / `unavailable` / `notImplemented` / `noReportYet` / `staleReport` 这类「数可能取不到或有前提」的异常态，才在占用列末尾追加 `InfoCircleOutlined` 角标，tooltip 显示 `availabilityText(availability)` 词表文案，详情「诊断」行同时给出该文案（§5.6 / §5.9）。

### 5.4 列定义（四列，清理为第 4 列）

| # | 列 | 宽度 | 内容 |
|---|---|---|---|
| 1 | 名称 | `minmax(0, 1.8fr)` | 第一行：`entryNameText(key)`（未命中覆盖表时就是 `key` 原文）+ 展开箭头（行末）；第二行：`key`（等宽 10px 次要色，单行省略 + tooltip）——**仅当第一行是本地化名称时显示**（第一行已是 `key` 时不重复同一串） |
| 2 | 占用 / 条数 | `112px` | 主行 `bytesOrDash` + 异常可用性角标（§5.3 / §5.6）；次行条数 |
| 3 | 策略 | `minmax(0, 1fr)` | chip 组（最多 2 枚 + `+N`，见 §5.6） |
| 4 | 清理 | `88px` | 行内「清理」按钮 / owner 自动管理的静态文案 / `—`（§5.7） |

两列用 `minmax(0, …)` 允许收缩到 0，两列固定像素合计仅 200px，**任何窗口宽度都不会产生横向滚动**（§3.6）；窄窗时名称 / 策略列以省略号 + tooltip 截断，全文在详情。

**没有资格列、没有可用性列**（§0.1 D1）：资格的唯一载体是清理按钮可用态（§5.7），可用性的唯一载体是占用列角标与详情「诊断」行（§5.6 / §5.9）。**清理列是本期唯一的动作列**，由原「动作列」并入资格列的位置，不再有独立的第 5 / 第 6 列。

列宽常量与分组常量放在 `entryColumns.ts`（纯数据，无 React 导入），grid 模板由该常量生成 inline style —— 与既有 `PermissionAuditSection/AuditTable.tsx` 同法，避免 CSS 与 TS 两处列宽漂移。

**不做列宽拖拽**：行数由后端决定但规模小、列宽由内容决定，拖拽收益低，且会把审计表的 `useColumnResize` 机制复制第二份（共享叶子未下沉时不复制，见 §9.3）。

### 5.5 分组：扁平优先，安全域置末且唯一可折叠

**默认全部扁平**：条目按 `entries` 顺序逐行平铺，不设组头、不做分类。唯一例外是安全域：

- 唯一可折叠组 = `key` 首段为 `security` 的条目（`security/security.db` / `security/backups` / `security/vault` / `security/recovery` / `security/audit`），合并为一个可折叠组，组标签取 `entries.groups.security`（安全域）；
- **组的存废由 `entries` 决定，不由常量决定**：`entries` 里没有任何 `security/*` 条目时（可选功能未挂载），**整组不渲染**——不出现空组头、不出现 `(0)` 计数、不出现「可选功能未启用」占位行；此时区块 B 只渲染 core 自有条目（8 条）；
- 安全域**固定在表格末尾**：所有平铺行按 `entries` 顺序先排，安全域组整体移到最后一屏位置；组内顺序仍为 `entries` 顺序，**不重排组内行**；
- 其余条目（`attachments`、`attachments/derived`、`tool-results`、`tool-index`、`mascot-assets`、`assistant.db`、`logs`、`crashes`）一律平铺成行，不产生组头；
- 可折叠组集合是**显式常量**（`COLLAPSIBLE_GROUPS = ["security"]`），置末顺序由该常量顺序决定；该常量只声明「哪些首段**可**折叠」，**不保证该组一定存在**（存在性看 `entries`，见上一条）；新增可折叠组是产品决策，不是后端数据自动派生；后端新增条目若不在集合内，就以平铺行出现，**永远不会掉进「其他」组**；
- 组可折叠，默认展开；折叠状态存组件内存，不持久化。

硬规则（沿用）：

1. 组头**不带复选框、不带清理按钮**（否则等于「一键清理该域」，违反 X4 / G10 精神；清理只能逐行发起）；
2. 组头**不显示字节合计**（跨条目相加只有附件账本与 `totalReclaimableBytes` 两处允许，G10；后者本期不展示，§6.1）；
3. 组内顺序 = `entries` 顺序，**不提供排序**（不按大小排序，避免诱导「先清最大的」）；
4. 组头显示「组名 + 条目数」，条目数 = 组内行数（不是后端字段，也不是字节合计）；
5. **不假设条目总数**：行数 = `entries.length`（core 条目 + 已挂载分区自述条目），组件与文案**不得**写死任何条目总数（如「共 18 项」）；空 `entries`（理论上不会发生）显示空态而不是伪造行。

### 5.6 徽标、chip 与按钮态（`lexicon.ts` + `entryView.ts` 单点映射）

| 列 | 第一行 / 主内容 | 第二行 / 附加 |
|---|---|---|
| 名称 | `entryNameText(key)` + 展开箭头 | `key`（仅第一行为本地化名称时） |
| 占用 | `bytesOrDash(bytes)`；异常可用性追加 `InfoCircleOutlined` 角标（tooltip = `availabilityText(availability)`） | 条数次行（`countText`） |
| 策略 | chip 组，最多 2 枚 + `+N`：第 1 枚恒为 `policySourceText(policy.source)`；其余按 `retentionDays` → `keepCount` → `maxBytes` → `autoActive` 顺序追加 | 不显示（其余 chip 与 `effectiveSince` / `mutable` 全在详情） |
| 清理 | 见 §5.7（按钮 / 静态文案 / `—`） | 不显示 |

**可用性只在异常时出现**：

| `availability` | 占用列 | 详情「诊断」行 |
|---|---|---|
| `available` / `missing` | 无角标（`missing` 由 `0 B` 表达） | 不出现 |
| `locked` / `unreadable` / `unavailable` / `notImplemented` / `noReportYet` / `staleReport` | `InfoCircleOutlined` 角标（`locked` / `staleReport` 橙、`unreadable` / `unavailable` 红、其余次要色） | 同上 |

角标只影响图标颜色，不额外加 Tag —— Tag 会把「异常」说成一种并列的分类，角标才是「这条数字有前提」的正确语气。

**`notImplemented` 只剩一种来源**：core 自有、组件没有回收入口的条目（`E13` `crashes`），其结构性缺口说明由后端 `reclaim.note` 承载（可用性本身只由 `availability` 表达，wire 上没有原因文案字段）。`E12`（`logs`）的占用由应用侧只读遍历得到，是**正常可用行**（`available` / `missing` / `unreadable`），只有策略 chip 与「清理」列的静态文案，不再是异常行。可选功能分区的条目在未挂载时**根本不在 `entries` 里**，因此前端**不会**收到「可选功能未启用」的 `notImplemented` 行，也**不得**为它补一行——这是后端分区自管理语义（规格 §3.1.1 / §3.10）在前端的直接体现；把未挂载画成占位行会让用户看到一个永远点不动、也永远不会消失的条目。

**`policy.source = none` 时策略列只剩一枚「无保留策略」**：矛盾的两端分别落在「策略列说明没有保留策略」与「清理按钮的可用态说明现在能不能清」上，不再用 `failClosed` 的第二枚 chip 去补语义——`failClosed` 是后端判据字段，**前端不翻译、不渲染**（不进 chip、不进详情）；能否手动清理由第 4 列按钮的可用态决定（P16 / §5.7）。

硬规则：

1. **`note` 是后端自由文本，原样展示**（含语言），不映射成前端枚举文案、不翻译、不截断成固定词；否则等于前端维护第二套判据词表（X7 的变体）；可用性文案一律走 `availabilityText(availability)` 词表，不再有后端原因原文；
2. **封闭枚举**值未命中词表（后端新增值）→ 显示**原始枚举值**并 `console.warn`，**禁止**兜底成「可用」或空白（静默会把未知说成安全）；
3. 占用列角标**不重复**详情已给的原文，只做「有前提」的提示；全文在详情「诊断」行。

### 5.7 清理列：资格的唯一载体

区块 B **只有一种动作：行内「清理」按钮**。没有多选、没有组级按钮、没有开关。资格（`eligibility`）不出现在任何列里，只决定这个按钮的可用态：

| `reclaim` 情况 | 清理列渲染 | 可点 | tooltip |
|---|---|---|---|
| `action = none` | `—`（`bytesOrDash(null)`，无按钮） | — | — |
| `eligibility = eligible` | 「清理」按钮；`confirmation = strongConfirm` 时带 `SafetyOutlined` | 是 | 无（弹窗里给强确认，§6.2） |
| `eligibility = ownerManaged` | 静态文案 `entries.eligibility.auto_managed`（由 owner 自动管理），**不给按钮** | — | 无 |
| 其他非 `eligible`（`notEligible` / `locked` / `notImplemented`） | 「清理」按钮 + `disabled` | 否 | `actionText(action) · <原因>`；`notImplemented` 且有 `note` → **`note` 原文**；否则 `reasonText(reason)`，`reason = null` 时用 `eligibilityText(eligibility)` |

`planning` / `executing` 期间：**其余行的清理按钮一律禁用**（同一时刻只允许一个清理流程，避免并发 `busy`）；正在执行的那一行由弹窗独占。

由此**保留并落地**的整块实现：`cleanupFlow.ts`（状态机 / 错误码映射 / 结果文案）、`useCleanup.ts`（副作用壳）、`CleanupDialog.tsx`（弹窗）、`cleanup.*` 与 `cleanup.error_code.*` 键树、`storageService` 的 `getStorageReclaimPlan` / `reclaimStorage` 调用点（§6）。命令名与错误码保持后端契约不变，只有前端命名与文案统一为「清理」。

一致性保障：按钮可点 ⟺ 后端 `reclaim_storage` 会接受（规格 §3.3 的 `eligible` 定义）；「为什么不能清」的答案**只在按钮 tooltip 与详情**里，不另设资格列。

### 5.8 表格容器

- **零内嵌滚动**：不设 `max-height` / `overflow` / `overscroll-behavior`，表格随内容自然展开，滚动只由页面容器承担（§3.6）；
- 表头 `position: sticky; top: var(--storage-sticky-top)`（= 工具条高 40px），`z-index` 低于工具条；`sticky` 的包含块是本表容器，卡片滚出视口后表头自然让位；
- 组头**不吸顶**（§3.6 明确取舍：避免三层吸顶吃掉视口）；
- 行高：主行 `min-height: 40px`（两行单元格），详情展开区不占主行高度；
- 不设 `min-width`：列宽全部由 §5.4 的 `minmax(0, …)` 流式分配，**永不出现横向滚动条**。

### 5.9 详情展开（单行手风琴）

- 名称列末尾一个 `RightOutlined` / `DownOutlined`（旋转 90°）切换按钮，`aria-expanded` + `aria-controls`；
- 同一时刻最多展开一行（`expandedKey: string | null`），再次点击收起；
- 详情区 `grid-column: 1 / -1`，两列 key-value，12px 字号，不参与排序/筛选；
- 详情内容固定为 8 项：

| 项 | 值 | 空值 |
|---|---|---|
| 条目 key | `attachments`（等宽） | 恒有 |
| 层 | `entries.facts.layer` + `data` / `local` 原文 | 恒有 |
| 提供方 | `entries.provided_by`（`{{owner}}`） | 恒有 |
| 时间范围 | 最早 `oldestAt` · 最近 `newestAt` | `—` |
| 可清理 | `reclaim.reclaimableCount` / `reclaim.reclaimableBytes`（后端值） | `—` |
| 策略 | `source` / `effectiveSince` / `mutable` | `—` |
| 口径标记 | `includesDerived` / `countsSelfLogs` 两个 chip（真值才出现） | 都不出现 |
| 诊断 | `availabilityText(availability)`（仅异常态） / `note`（原文） | 都不出现 |

---

## 6 清理流程（本期落地）

> **本期状态：整章落地。** 区块 B 第 4 列的「清理」按钮固定走 `get_storage_reclaim_plan` → 确认 → `reclaim_storage`（规格 §6.1），只对 `eligible` 的条目开放（§5.7）。前端命名与文案统一为 `cleanup` 语义：`cleanupFlow.ts` / `useCleanup.ts` / `CleanupDialog.tsx`、`cleanup.*` 键树；命令名与错误码仍是后端契约的 `get_storage_reclaim_plan` / `reclaim_storage` / `planExpired` 等，不改动。

### 6.1 流程与状态机

```
idle ──点击清理──▶ planning ──计划到达──▶ confirming ──确认──▶ executing ──▶ done / failed
                     │                        │                    │
                     └──失败──────────────────┴────────────────────┴──▶ idle（+ toast）
```

- **一次只提交一个条目**（`entries: [{ key, action }]`）：串行、可解释、失败隔离清晰；
- 因此**不展示计划合计**（`totalReclaimableBytes` 与 `complete` 不参与渲染）：单条目下「合计」等于该条目本身，展示只会让用户以为可以一次清多条；
- `planId` 不透明：前端只回传，不解析、不比较、不持久化（计划只在内存活到本次流程结束）；
- 计划**不做本地缓存**（`planExpired` 是常态而非异常，缓存只会制造过期错误）；
- 弹窗内的条数/字节来自计划条目（`expectedCount` / `expectedBytes`），`null` → `—`，不填 0。

### 6.2 两级确认

| `confirmation` | 弹窗 | 确认门槛 |
|---|---|---|
| `none` | **不弹窗**，直接进入 `executing`（仅 `eligible` 才可能走到这里，`none` 表示后端判定无风险） | — |
| `confirm` | `Modal`（受控，非 `Modal.confirm`）：标题「清理「{{name}}」？」、正文三件套（动作 / 预计条数 / 预计字节）+ 不可撤销警示 | 点「执行清理」 |
| `strongConfirm` | 同上 + 输入框：要求输入**条目 key**（等宽字体展示待输入值） | 输入与 `key` 完全相等才启用「执行清理」；不等时按钮禁用并显示 `cleanup.strong.mismatch` |

为什么强确认输入 `key` 而不是本地化名称：名称随语言变化、含空格与全角字符，用户无法准确复现；`key` 稳定、可复制、与后端身份一致。弹窗同时显示「名称（key）」，满足「二次输入条目名」的意图而不引入歧义。

交互细节：

- 输入框 `autoFocus`；回车 = 提交（仅在匹配时生效）；Esc = 取消；
- `executing` 期间：确认按钮 loading、取消按钮禁用（后端已开始删除，取消无意义）；
- 弹窗关闭后焦点回到触发的行按钮（`ref` 保存）；
- 同一时刻只允许一个流程（`planning` / `executing` 期间其余行按钮禁用，避免并发触发 `busy`）。

### 6.3 错误码映射（`cleanupFlow.ts` 单点）

命令错误以字符串码返回（`reclaim.rs`：`planNotFound` / `planExpired` / `confirmationRequired` / `busy` / `entryNotAllowed`），前端映射到 `cleanup.error.*`：

| 错误码 | 提示 | 弹窗 | 是否自动重放 |
|---|---|---|---|
| `busy` | `cleanup.error.busy`（另一项清理正在进行，请稍后重试） | 关闭 | **否** |
| `planExpired` | `cleanup.error.plan_expired`（计划已过期，请重试） | 关闭 | **否** |
| `planNotFound` | `cleanup.error.plan_not_found`（计划不存在，请重试） | 关闭 | **否** |
| `entryNotAllowed` | `cleanup.error.not_allowed`（该条目当前不允许清理） | 关闭 | 否 |
| `confirmationRequired` | `cleanup.error.confirmation_required` | 保持打开 | 否（这是前端漏传 `confirmed` 的信号，属缺陷，不重试） |
| 计划阶段其他错误 | `cleanup.error.plan_failed` | 关闭 | 否 |
| 执行阶段其他错误 | `cleanup.error.execute_failed`（`{{error}}` = 原文） | 关闭 | 否 |

「提示重试，不自动重放」是本节硬规则（原 §6.3 第 8 条已随该节划出，口径由本文承接）：**只提示，用户自己再点一次**。任何自动重试都可能导致「用户没看到就删了」。

### 6.4 结果呈现

执行成功后先关闭弹窗，再按 `result.entries[0].status` 分支（`toast` 用 `message`，时长 4s）：

| `status` | 文案 | 追加 |
|---|---|---|
| `done` | `cleanup.result.done`（已清理 N 项，释放 X） | — |
| `partial` | `cleanup.result.partial`（删除 N 项，释放 X，失败 M 项） | 失败项原因取 `errors[0].code` → `cleanup.error_code.<code>` 词表（`permissionDenied` / `io` / `ownerUnavailable` / `policyDisabled` / `locked` / `unknown`） |
| `failed` | `cleanup.result.failed`（清理失败） | 同上 |
| `skipped` | `cleanup.result.skipped`（没有可清理的内容，可能刚被引用） | — |
| `notEligible` | 理论不可达（计划已过滤），出现即 `console.warn` + `cleanup.error.not_allowed` | — |

无论哪种状态，**都重拉一次** `get_storage_breakdown`（`execute` 之后目录已变，旧快照必然失真）。

### 6.5 组件边界

| 文件 | 职责 |
|---|---|
| `cleanupFlow.ts` | 纯 reducer（`idle / planning / confirming / executing`）+ 错误码 → 文案键映射 + 结果 → 提示描述；无 IO、无 React |
| `useCleanup.ts` | 副作用壳：调 `getStorageReclaimPlan` / `reclaimStorage`，把结果喂给 reducer，成功后调快照刷新回调 |
| `CleanupDialog.tsx` | 只吃 props（`open` / `entry` / `plan` / `phase` / `onConfirm` / `onCancel`），不调 service |

---

## 7 段二：未登记占用（只读）

本段回答「盘上还有多少没被条目覆盖」，包含两类节点：①**未登记**（`class ∈ {unregistered, optionalNotMounted, companion, ignored}`，没有任何条目认领）；②**已登记但未统计完**（`class = declared`，`scanBytes − declaredBytes > 0`，即已登记目录里 owner 未计入的差额，多为 `-wal` / `-shm` / `.tmp` 伴生文件，属规格 §3.11.2 第 4 条的预期噪声）。因此「已登记」行出现在本段**不是矛盾**：它答的是「登记之外的那部分」，不是「这个目录未登记」。

段头 hint 必须同时覆盖两类（文案：`受管目录中未被任何条目统计的部分，含已登记目录内 owner 未计入的差额；不计入上方合计`），**不得**写成「不属于任何条目的部分」——后者会把 `declared` 行读成「未登记」，与列表口径冲突。

本段不提供清理：条目清理全部在段一区块 B（§5.7 / §6），本段**没有任何行内按钮**，唯一动作是段头的扫描。

### 7.1 数据来源

`breakdown.inventory`（与条目、账本同一次快照）。因此：

- 打开页面**不发**第二个请求；
- `storage:inventory-updated` 事件到达后重拉 `get_storage_breakdown`（一次），段一（区块 A / B）与段二一起更新，不会出现「条目是新数字、余量是旧数字」的错帧；
- `refresh_storage_inventory()` 只返回 `StorageInventorySnapshotStatusDto`（`status` / `scannedAt` / `failedAt`），**数字只从重拉的快照来**。

### 7.2 五态与按钮

| `status` | 状态文案 | 按钮 | 按钮行为 |
|---|---|---|---|
| `neverRun` | `residual.status.neverRun`（尚未扫描） | 「扫描」（默认按钮） | 调 `refresh_storage_inventory()` |
| `running` | `residual.status.running`（扫描中） | 禁用 + `loading` | — |
| `fresh` | `residual.status.fresh`（上次扫描：{{time}}） | 「重新扫描」 | 同上 |
| `stale` | `residual.status.stale`（上次扫描：{{time}}，可能过期）+ 橙字 | 「重新扫描」 | 同上 |
| `failed` | `residual.status.failed`（扫描失败：{{time}}）+ 红字 | 「重试扫描」 | 同上 |

点击扫描后：**立刻**把按钮切到 `running` 态（乐观的按钮态，只影响按钮，不碰任何数字），再用返回的 `status` 校正；若调用本身失败，恢复原状态 + `residual.scan_failed`（`{{error}}`）提示。数字永远等事件 + 重拉。

### 7.3 警示栈（自上而下，各自独立）

| 条件 | 展示 | 层级 |
|---|---|---|
| `status = failed` 且存在上次数字 | `Alert type="warning"`：`residual.notice.failed_keep_last`（扫描失败，以下为上次成功扫描的数字） | 1 |
| `partial = true` | `Alert type="warning"`：`residual.notice.partial` + `truncatedNodes`（最多列 3 个，余下 `residual.notice.truncated_more` `{{count}}`） | 2 |
| 任一节点 `overRegistered` | `Alert type="error"`：`residual.notice.over_registered`（登记值大于盘上实际值，可能是统计口径不一致） | 3 |
| 无任何节点 | `Empty` + `residual.notice.empty` | — |

硬规则：`failed` **不清空**数字（保留上次成功的 `nodes` / `unregisteredBytes`）；`partial` 必须说明「以下数字是下界」，不能只显示数字。

### 7.4 节点列表

**显示范围**：`class ∈ {unregistered, optionalNotMounted, companion, ignored}` 全显示；`class = declared` **仅在 `residualBytes > 0` 时**以折叠行出现（它的总量已在条目表里，出现只为暴露「已登记目录里未被 owner 统计的部分」）。

**排序**（`residualView.ts` 纯函数）：

1. `unregistered` 优先（主信号：有东西没人认领）；
2. 其余按类别序：`optionalNotMounted` → `companion` → `ignored` → `declared`；
3. 同类别内按 `residualBytes` 降序，`null` 置末；
4. 稳定兜底按 `name` 升序（避免 `null` 值顺序抖动）。

**行结构**（6 列）：

| 列 | 内容 |
|---|---|
| 名称 | 节点名（等宽 12px）+ `children.length > 0` 时展开箭头 |
| 类别 | `classText(node.class)`（`residual.class.*`；`unregistered` 橙、`optionalNotMounted` 蓝、`companion` / `ignored` 中性、`declared` 绿） |
| 范围 | `layerText(node.scope)`（与条目详情共用同一份 `entries.layer.*` 词表，不另立 `scope` 词表） |
| 扫描 | `bytesOrDash(scanBytes)` |
| 登记 | `class = declared` 时 `bytesOrDash(declaredBytes)`，否则 `—`（非 declared 的 0 是无意义噪声） |
| 余量 | `bytesOrDash(residualBytes)`；`overRegistered` 时追加 `ExclamationCircleOutlined`（红色） |

`overRegistered = true` 的行：整行红底（`storage-residual-row is-over`）+ 余量列图标 + 该节点 Tooltip 显示 `residual.notice.over_registered`。本节硬规则：**必须显著标红并给固定文案，不得静默**（沿革自原 §6.3 第 9 条，该节已划出，口径由本文承接）。

**children 展开**：点击名称列箭头展开该节点的 `children`（`name` + 扫描字节/条数），最多 20 项，超出显示 `residual.children.more`（`{{count}}`）；`declared` 节点默认折叠（出现即已提示，展开是可选深挖）。

**合计行**：`residual.totals.unregistered`（未登记合计：`unregisteredCount` 项，`unregisteredBytes`）+ 固定 hint `residual.totals.hint`（不计入页面任何合计）。`unregisteredBytes` / `unregisteredCount` 为 `null` → `—`（`null` 传染，不填 0）。

**硬禁令**：本段**没有任何按钮**，除段头的扫描按钮。不提供删除、清理、忽略、标记。

### 7.5 组件边界

| 文件 | 职责 |
|---|---|
| `ResidualSection.tsx` | 段头 + 五态 + 扫描按钮（局部 `useState` 管理按钮态，无复用价值故不单独成 hook） |
| `ResidualTable.tsx` | 警示栈 + 节点表 + 合计行（吃 `ResidualViewModel`） |
| `residualView.ts` | 纯函数：排序、`declared` 过滤、警示栈生成、合计格式化 |

---

## 8 数据层

### 8.1 `useStorageSnapshot.ts`

| 项 | 设计 |
|---|---|
| 状态 | `snapshot: StorageBreakdownDto \| null`、`phase: "initial" \| "ready" \| "refreshing"`、`error: string \| null` |
| 拉取 | `load(mode)`：`initial` 只在首载与手动重试用；`refresh` 保留旧 `snapshot` |
| 单飞 | 用 `inFlightRef` 去重：已有请求在飞时直接返回同一个 promise，**不叠加**请求（事件与按钮可能同时触发） |
| 事件 | `listen("storage:inventory-updated")` → `load("refresh")`；`unlisten` 在卸载时调用（写法同 `src/vault/core/hooks/useVaultEvents.ts`）；订阅失败 `log.error` 并**不**影响页面 |
| 轮询 | **无**（规格禁止新增轮询） |
| 刷新后选择 | 保留既有行为：重拉后清空 `selectedIds`（刷新可能移除已删文件，保留选择会产生幽灵勾选） |

### 8.2 事件名常量

`STORAGE_INVENTORY_UPDATED = "storage:inventory-updated"` 定义在 `useStorageSnapshot.ts`（唯一消费者），不新建全局事件模块（避免为一行常量开文件）。

### 8.3 失败两态（相对旧实现的唯一行为改进）

| 阶段 | 旧行为 | 新行为 | 理由 |
|---|---|---|---|
| 首载失败 | `snapshot = null` + 空态「加载失败」 | 同（空态 + 「重试」按钮） | 无数据可留 |
| 刷新失败 | `snapshot = null` + 空态 | **保留** `snapshot` + 工具条下方 `Alert type="warning"`（`toolbar.refresh_failed`）+ 刷新按钮可再点 | 一次网络抖动不应清空用户正在看的页面；规格 §7.1 只要求单条目失败不整页报错，未要求刷新失败清空 |

`error` 只承载「本次请求失败」的提示，不参与任何数字判断。

---

## 9 组件与文件拆分

### 9.1 目录与职责

```
src/settings/pages/StorageSettings.tsx       薄壳：渲染 <StorageContent/>（保持既有文件路径，路由/注册不动）
src/settings/pages/storage/
  StorageContent.tsx      组合入口：工具条 + 段一（区块 A / B）+ 段二；只做顺序与传参，无业务分支
  SnapshotToolbar.tsx     采样时刻 / 口径 / 刷新失败提示 / 刷新按钮
  AttachmentBlock.tsx     段一 · 区块 A：区块头 + 账本摘要 + 警告 + 分组容器（迁移既有逻辑，原 AttachmentSection.tsx）
  EntryBlock.tsx          段一 · 区块 B：区块头 + EntryTable + 清理流程接线（原 EntrySection.tsx）
  EntryTable.tsx          表头 / 扁平行 + 安全域置末折叠组 / 行 / 详情切换（吃 EntryRowView[]）
  EntryDetail.tsx         详情展开区（key-value 列表）
  CleanupDialog.tsx       清理确认弹窗（confirm / strongConfirm），只吃 props
  cleanupFlow.ts          清理状态机 + 错误码 → 文案键 + 结果 → 提示描述（纯函数）
  useCleanup.ts           清理副作用壳：调 getStorageReclaimPlan / reclaimStorage，成功后触发快照刷新
  ResidualSection.tsx     段二：段头 + 五态 + 扫描按钮
  ResidualTable.tsx       警示栈 + 节点表 + 合计行（吃 ResidualViewModel）
  entryColumns.ts         列键 / 列宽 / 表格常量（4 列；无 React 导入；扁平行 + `COLLAPSIBLE_GROUPS` 置末折叠组）
  entryView.ts            条目视图模型纯函数（占位符、占用角标、策略 chip、清理按钮态；无 IO、无 React）
  residualView.ts         余量视图模型纯函数（排序、过滤、警示栈、合计）
  lexicon.ts              枚举 → 已翻译文案（availability / eligibility / reason / action / policy / class / layer / 条目名与组标签的可选覆盖），写法同 src/audit/lexicon.ts
  format.ts               bytesOrDash / 时间 / 条数（`formatTimestamp` 从旧文件机械搬入）
  useStorageSnapshot.ts   拉取 / 刷新 / 事件重拉 / 单飞
  __tests__/              lexicon / entryView / cleanupFlow / residualView / localeParity
```

### 9.2 依赖方向（单向，无环）

```
StorageSettings.tsx → StorageContent.tsx
StorageContent.tsx  → SnapshotToolbar / AttachmentBlock / EntryBlock / ResidualSection / useStorageSnapshot
EntryBlock.tsx      → EntryTable / useCleanup / CleanupDialog / entryView
EntryTable.tsx      → entryColumns / EntryDetail / format
EntryDetail.tsx     → lexicon / format
ResidualSection.tsx → ResidualTable / residualView
ResidualTable.tsx   → residualView / format
entryView.ts        → entryColumns / lexicon / format
cleanupFlow.ts      → lexicon（错误码 → 文案键）
residualView.ts     → lexicon / format
```

规则：纯函数模块（`entryView` / `residualView` / `cleanupFlow` / `lexicon` / `format`）**不导入 React、不导入 service**；组件**不导入 service**（只经 hook）。区块 A 不依赖 `entryView` / `entryColumns`（它没有列定义，只有组与文件行）；区块 B 不直接调用 `storageService`，清理命令只经 `useCleanup`。

### 9.3 与既有模块的关系

| 既有物 | 处置 | 理由 |
|---|---|---|
| `pages/StorageSettings.tsx` 路径 | 保留文件名，内容改为薄壳 | 路由与页面注册引用该路径，改路径等于动注册表 |
| `SettingsPageShell` | T4 期间继续包裹薄壳；R0 交付后由 tab 容器替换 | 内容组件不依赖壳，二次搬迁成本为 0 |
| `PermissionAuditSection/useColumnResize` | **不复用** | 跨 section 目录引用会形成耦合；且本表不需要拖拽（§5.4）。若将来第二个表格需要拖拽，再把 hook 下沉到 `settings/shared/hooks/`（共享叶子下沉），不在 T4 顺手搬 |
| `utils/formatBytes` | 不改签名 | 占位符由 `format.ts` 的 `bytesOrDash` 承担（任务清单明确禁止改签名） |
| `settings.css` | 追加类，不重命名既有类 | 既有 `storage-*` 类继续服务区块 A |

---

## 10 样式与视觉规范

### 10.1 类名

| 前缀 | 归属 | 例 |
|---|---|---|
| `storage-`（既有） | 区块 A | `storage-summary-row` / `storage-group-header` / `storage-attachment-row` |
| `storage-toolbar-` | 工具条 | `storage-toolbar` / `storage-toolbar-meta` |
| `storage-section-` | 段头与区块头 | `storage-section-head` / `storage-section-title` / `storage-section-hint`；区块头追加 `is-block` 降一档字号 |
| `storage-entry-` | 区块 B | `storage-entry-table` / `storage-entry-header` / `storage-entry-group` / `storage-entry-row` / `storage-entry-detail` |
| `storage-residual-` | 段二 | `storage-residual-table` / `storage-residual-row` / `storage-residual-total` |

状态修饰符（一律 `is-` 前缀，与既有 CSS 习惯一致）：`is-block` / `is-warning` / `is-over` / `is-collapsed` / `is-running`。不需要 `is-eligible` / `is-blocked` / `is-owner`：资格只由清理按钮的 `disabled` 态表达，策略只由 chip 表达（§5.7）。

### 10.2 视觉规则

| 项 | 取值 |
|---|---|
| 颜色 | 只用既有 CSS 变量（`--color-border-secondary` / `--color-bg-elevated` / 语义色）；不新增色值常量、不写死 `#hex` |
| 字体 | 名称 12px 常规；`key` / 节点名 12px 等宽；第二行文案 10px；表头 11px 次要色 |
| 行高 | 表头 32px；条目行 `min-height: 40px`；余量行 32px |
| 分隔 | 列间 `1px solid var(--color-border-secondary)`（与审计表一致），域组头下边框加粗 |
| 危险色 | 只用于区块 A 的「删除选中」按钮、区块 B 的「清理」按钮（可点时 `danger`）、`unreadable` / `unavailable` 角标、`overRegistered` 行与图标；行底用 `--color-error-bg`，图标/文字用 `--color-error`；警示文案用 `--color-warning` |
| 数字对齐 | 字节/条数列右对齐 + `font-variant-numeric: tabular-nums` |
| 悬停 | 条目行 / 文件行 `hover` 提亮背景；禁用按钮不显示 hover 变化 |

### 10.3 密度与响应式

- 不设内容区最小宽度：表格列用 `minmax(0, …)` 流式分配，窄窗时以省略号 + tooltip 截断，**永不横向滚动、不折叠列、不隐藏信息**（§3.6 / §5.4）；
- 段一与段二之间间距 24px（与 `SettingsSection` 的 `marginTop` 一致）；
- 段内区块 A 与区块 B 之间只用 1px 细线 + 12px 内边距分隔，不另起卡片、不另起段头（§3.3）。

---

## 11 i18n 键树（zh-CN 为类型源）

### 11.1 结构

```
settings.storage
├─ title                        既有（I54 导航用，不动）
├─ toolbar.{refresh, sampled_at, byte_basis, refresh_failed}
├─ common.{load_failed, retry, bytes_unknown, count_value, time_unknown}
├─ attachments.{title, hint, delete_selected, confirm_delete.{title,content},
│               deleted, delete_failed, empty, size_warning,
│               ledger.{total, shared}, orphan_group, orphan_badge}
├─ entries.{title, hint, provided_by, details, hide_details, attachment_orphans, cleanup_button,
│           columns.{name,size,policy,cleanup},
│           groups.{attachments, security},          ← 可选覆盖，未命中回退首段原文
│           names.{attachments, attachments_derived, tool_results, tool_index,
│                  mascot_assets, assistant_db, security_security_db,
│                  security_backups, security_vault, security_recovery,
│                  security_audit, logs, crashes},   ← 可选覆盖，未命中回退 key 原文
│           availability.{available,missing,locked,unreadable,unavailable,notImplemented,noReportYet,staleReport},
│           eligibility.{eligible,notEligible,locked,ownerManaged,auto_managed,notImplemented},
│           reason.{orphan,expired,overLimit,derivedCache,transient,referenced,userData,
│                   businessData,replayRisk,locked,noStats,noReclaimApi,externalOwner,
│                   policyDisabled,unknown},
│           action.{none,deleteOrphans,deleteSelected,sweepExpired,pruneToLimit,rebuildCache,rotate},
│           layer.{data,local},
│           policy.{source.{config,constant,owner,none}, retention, retention_disabled,
│                   keep_count, max_bytes, auto_active},
│           facts.{layer,oldest,newest,reclaimable,effective_since,mutable,
│                  includes_derived,counts_self_logs,note}}
├─ cleanup.{planning, executing, ok, cancel,
│           dialog.{title, body, key_hint, irreversible},
│           strong.{title, body, placeholder, mismatch},
│           error.{busy, plan_expired, plan_not_found, not_allowed, confirmation_required,
│                  plan_failed, execute_failed},
│           error_code.{permissionDenied, io, ownerUnavailable, policyDisabled, locked, unknown},
│           result.{done, partial, failed, skipped}}
└─ residual.{title, read_only, hint, scan, scanning, rescan, retry_scan,
             status.{neverRun,running,fresh,stale,failed},
             notice.{failed_keep_last,partial,truncated_more,over_registered,empty,scan_failed},
             columns.{name,class,scope,scan,declared,residual},
             class.{declared,optionalNotMounted,companion,ignored,unregistered},
             scope.{data,local},
             totals.{unregistered,hint},
             children.{more}}
```

**`cleanup.*` 是本期落地的键树**（§6），文案一律「清理」；后端命令与错误码仍是 `get_storage_reclaim_plan` / `reclaim_storage` / `planExpired` 等，不随前端命名改变。`groups.*` / `names.*` 是**可选覆盖层**，未命中的 `key` 或首段一律回退原文并静默（§5.2 / §5.5），因此后端新增条目或新分组不需要前端改键树。**不得残留 `reclaim.*` / `entries.reclaim_button` / `entries.columns.availability` / `entries.columns.eligibility` / `entries.columns.action`**（§13 localeParity 断言）。

### 11.2 文案取值（zh-CN / en-US 一一对应）

//移除

### 11.3 既有键迁移（代码正在使用的键）

//移除

### 11.4 草稿键处置（上一轮已写入、当前无代码引用）

//移除

---

## 12 可访问性与文案规范

| 项 | 规则 |
|---|---|
| 展开 | 详情切换按钮 `aria-expanded` + `aria-controls`；组头折叠按钮同理 |
| 禁用 | 扫描按钮在 `running` 态禁用 + `loading`；区块 A 的「删除选中」在 `selectedIds` 为空时禁用；区块 B 的「清理」在非 `eligible` 或另一清理流程进行中禁用，禁用原因写进 tooltip |
| 键盘 | 行展开、组折叠支持 Enter / Space（用 `Button type="text"` 而非裸 `div` 点击） |
| 数字 | 字节用 `formatBytes`；`null` 用 `—`；时间统一「月 日 时:分」（沿用既有 `formatTimestamp`） |
| 语言 | `note` / `error.message` 为后端原文，**不翻译、不改写**；枚举文案（含 `availability`）一律走词表 |

---

## 13 测试矩阵（无 DOM，全部吃纯函数）

| 文件 | 断言 |
|---|---|
| `__tests__/lexicon.test.ts` | 「枚举值有键」由 `satisfies Record<Enum, I18nPath>` 在**编译期**保证，测试只覆盖运行期语义：枚举未知值（`"futureValue"`）返回原文并 `warn`；`constructor` / `toString` 等原型键不误命中；**任意未登记 `key`**（如 `"future/entry"`）`entryNameText` 回退 `key` 原文且**不告警**；未登记首段 `groupNameText` 回退首段原文；`entryNameText` 与 `availabilityText` 对同一输入稳定 |
| `__tests__/entryView.test.ts` | `bytes = null` → `—` 且**不调用** `formatBytes`；`missing` + `0` → `0 B` 且**无角标**；`unreadable` / `locked` → 占用列带角标 + tooltip 取 `availabilityText(availability)` 词表文案；视图模型**不含**可用性列、资格列字段，只含 4 列（`name` / `size` / `policyChips` / `cleanup`）；清理按钮四态：`eligible` → 可点、`strongConfirm` 标记、`ownerManaged` → 静态文案无按钮、其余非 eligible → 禁用 + tooltip（`notImplemented` 用 `note` 原文，否则 `reasonText`）；`action = none` → 清理列为 `—`；`policy.source = none` → 策略列只有一枚「无保留策略」chip（`failClosed` 不参与渲染）；分组派生：`attachments/derived` 并入 `attachments`、单成员组不生成组头、**未登记的新 `key` 自动成组 / 平铺**而不落「其他」；**条目行数 = `entries.length`**（8 条 core → 8 行；8 条 + 5 条 `security/*` → 13 行，两组用例都要覆盖）；**未挂载用例**：`entries` 不含任何 `security/*` 时**不产出安全域组**（无组头、无 `(0)` 计数、无占位行），且**不得**出现 `notImplemented` 的「可选功能未启用」行 |
| `__tests__/cleanupFlow.test.ts` | `idle → planning → confirming → executing → done/failed` 状态迁移；`confirmation = none` 跳过 `confirming`；`busy` / `planExpired` / `planNotFound` / `entryNotAllowed` → 对应 `cleanup.error.*` 且**不自动重放**；`result.status` 四态 → `cleanup.result.*`；`errors[0].code` → `cleanup.error_code.*`，未知码 → `unknown` |
| `__tests__/residualView.test.ts` | 排序（`unregistered` 优先、同类别按余量降序、`null` 置末、同值按名称稳定）；`declared` 仅在 `residualBytes > 0` 时出现；`partial` → 警示栈含 `partial` 且 `truncatedNodes` 最多列 3 个；任一 `overRegistered` → 警示栈含 `over_registered` 且该行带 `is-over`；`failed` 且保留数字 → 警示栈含 `failed_keep_last` 且 `nodes` 非空；全空 → `empty`；合计 `null` → `—` |
| `__tests__/localeParity.test.ts` | `zh-CN` 与 `en-US` 的键路径集合完全相等；`entries.names.*`（规范化 key）、`entries.groups.*`、`entries.layer.*`、`entries.availability.*`、`entries.eligibility.*`、`cleanup.*`、`residual.class.*`、`residual.status.*` 全部存在；**不得残留 `entries.names.e1..e13`、`entries.domains.*`、`entries.facts.fail_closed`、`reclaim.*`、`entries.reclaim_button`、`entries.columns.availability` / `eligibility` / `action`** |

不写的测试：组件渲染（无 DOM 环境）、CSS 断言、事件监听（`listen` 需 Tauri 运行时）。事件路径由 T4-b 手动回归覆盖（任务清单已列）。

---

## 14 门禁与自查（本设计自身的约束清单）

| # | 检查点 | 自查方式 |
|---|---|---|
| D1 | 前端不新建 store / service / 轮询 | `grep` 新增文件中的 `useState` 之外的持久化与 `setInterval` / `setTimeout` 轮询；service 只经 `storageService` |
| D2 | 不推算可清理量；清理只走 plan → execute | 全量检索 `expectedBytes` 的比较运算（只允许格式化展示）；`getStorageReclaimPlan` / `reclaimStorage` 的调用点**只在** `useCleanup.ts`；`confirmation` 只在 `cleanupFlow` / `CleanupDialog` 消费 |
| D3 | 不做跨条目合计 | 检索条目表与余量段的求和/`reduce`；组头与合计行不含跨条目字节 |
| D4 | `null` 不直传 `formatBytes` | `formatBytes(` 的调用点必须包在 `bytesOrDash` 之后；检索 `formatBytes(` 出现位置 |
| D5 | 余量区无删除入口 | 检索 `ResidualSection` / `ResidualTable` 中 `Button` 仅 1 处（扫描） |
| D6 | 区块 B 只有「清理」一个动作 | 检索 `EntryBlock` / `EntryTable` / `EntryDetail` 子树：`Button` 仅出现在清理列与详情展开切换；不出现 `deleteAttachments` / `AttachmentDeleteResultDto`；不直接调 `storageService`（命令只经 `useCleanup`）；无 `checkbox` / 多选 |
| D7 | 枚举全覆盖 | `lexicon.test.ts` + `localeParity.test.ts` 通过 |
| D8 | 事件只重拉一次 | `useStorageSnapshot` 单飞；`listen` 回调内只有一次 `load("refresh")` |
| D9 | 既有附件能力零删除 | 区块 A diff 只含 §4.2 的 6 条追加 |
| D10 | 文案无本机绝对路径、无硬编码语言 | 检索 `C:/`、`/Users/`、中文字面量出现在 `.tsx`（应全部来自 `t()`） |
| D11 | 拆分判据 | 每个文件对应单一变化原因；组件不导入 service；纯函数模块不导入 React |
| D12 | 页面路径不变 | `StorageSettings.tsx` 仍存在且导出默认组件（路由/注册未改） |
| D13 | 条目集合零硬编码 | 全量检索 `ENTRY_KEYS` / `ENTRY_IDS` / `E1`..`E13` / 静态域分组表在渲染路径中**不出现**；行集合、名称、组标签全部由 `entries` 驱动，可折叠组仅 `COLLAPSIBLE_GROUPS`（§5.1 / §5.2 / §5.5） |
| D14 | 资格与可用性不占列 | `entryColumns.ts` 的 `ENTRY_COLUMN_ORDER` 恰为 `name` / `size` / `policy` / `cleanup`；检索 `entries.columns.availability` / `.eligibility` / `.action` 为 0；占用列角标仅异常态出现（§5.3 / §5.6） |
| D15 | 未挂载不留占位 | 检索渲染路径中「可选功能未启用」类字面量与按 `key` 补行的逻辑；`entries` 无 `security/*` 时组头 / 组计数 / 行都不产出；行数与 `entries.length` 严格相等（§5.2 / §5.5 / §5.6） |

---

## 15 落地顺序（对应 T4-a）

| 步 | 产物 | 完成判据 |
|---|---|---|
| 1 | `lexicon.ts` + `format.ts` + 两份 locale 键树（§11） | `localeParity` / `lexicon` 测试绿 |
| 2 | `entryColumns.ts`（4 列）+ `entryView.ts` + `entryView.test.ts` | 占位符、角标与清理按钮四态断言全绿 |
| 3 | `residualView.ts` + `residualView.test.ts` | 排序 / 警示栈 / 过滤断言全绿 |
| 4 | `cleanupFlow.ts` + `cleanupFlow.test.ts` | 状态机与错误码断言全绿 |
| 5 | `useStorageSnapshot.ts`（含事件与单飞） | 页面仍只显示段一，行为与现状一致 |
| 6 | `AttachmentSection.tsx` 迁入 + 薄壳改造 | §4.1 九项能力逐项回归 |
| 7 | `EntryBlock` / `EntryTable` / `EntryDetail` + `useCleanup` + `CleanupDialog` | 4 列全部在场、行顺序与 `entries` 一致、行数 = `entries.length`（8 条 / 13 条两种构建都要走通）、清理流程可走通（confirm / strongConfirm） |
| 8 | `ResidualSection` / `ResidualTable` | 五态可手动触发（`neverRun` → `running` → `fresh`；失败态靠断网或注入错误） |
| 9 | `settings.css` 追加（§10） | 视觉无回归、窄宽横向滚动 |
| 10 | 全量门禁 | `npm run typecheck` / `npm run lint` / `npm run test` + §14 逐条 |

---

## 16 与规格的对应索引

| 规格条目 | 本文落点 |
|---|---|
| §3.1 条目表（core 13 条 + 分区自述 5 条） | §5.1 / §5.5 / §5.2（key 映射与行数不假设） |
| §3.1.1 可选功能分区条目 | §5.2 / §5.5 / §5.6（未挂载整组不出现、不补占位行） |
| §3.3 资格 | §5.7（清理按钮可用态）/ §5.6 |
| §3.5 动作 | §5.7 / §6 |
| §3.6 确认档位 | §6.2（档位只读、不改强度） |
| §3.7 策略 | §5.6 / §5.9（chip + 详情） |
| §3.11 对账余量 | §7 全段（未登记占用，只读） |
| §6.1 命令契约 | §6.1 / §7.1（计划与扫描均不本地缓存） |
| §6.2 DTO | §5.1（只消费列出的字段）/ §11.5（类型源） |
| ~~§6.3 第 1–10 条~~ **（已整体划出作废）** | 不再作为判据；意图由本文承接：不新建 store/service → §0 / §8.1；真实观测与 `null` 占位 → §5.3；保留附件能力 → §4.1；不推算 / 不降档 → §5.7 / §6.2；无第二套删除入口 → §3.3；失败提示重试不重放 → §6.3；余量只读 / 五态 / 折叠 / 警示 → §7 全段；余量不进合计 → §7.4 |
| §7 失败降级 | §6.3 / §8.3 / §7.3 |
| §10 V15–V20 | §12（文案与留痕）/ §6 / §7 |
| §11 切片 | §15 |
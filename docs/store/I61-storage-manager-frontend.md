# I61 存储管理器 · 前端设计（T4）

> 性质：**前端实现设计（T4 唯一实现基线）**｜上位契约：[I61-storage-manager-spec.md](./I61-storage-manager-spec.md)（§6.3 / §7 / §10 / §11）
> 拆分判据：[CODE-SPLIT-PRINCIPLE.md](../principle/CODE-SPLIT-PRINCIPLE.md)（只引用，不复述）
> 任务清单：[I61-storage-manager-tasklist.md](./I61-storage-manager-tasklist.md)（T4-a / T4-b）

本文只回答「前端怎么做」：组件边界、视图模型、交互状态机、视觉与文案、测试方式。**判据、字段语义、资格与档位来源一律以规格为准**，本文任何段落与规格冲突时以规格为准，并回改本文。

---

## 0 边界声明

| 事项 | 归属 | 本文的态度 |
|---|---|---|
| 13 条条目的观测值、`availability`、`reclaim` 视图 | 规格 §3 / §6.1 | **只渲染后端返回值**，不推算、不兜底成「可用」 |
| 页面壳、三 tab 容器、页标题、导航 | I54 §3（R0 建容器） | **不设计**。本文只设计 `storage` tab 的内容区 |
| 资格（`eligibility`）与确认档位（`confirmation`） | 规格 §3.3 / §3.6 | **只信后端**，禁止按 `expectedBytes` 自行升降档（§6.3 第 6 条） |
| 可回收字节 / 是否安全 | 后端计划 | 前端**只展示** `reclaim.reclaimableBytes` 与计划值，不做任何加减比较 |
| 删除附件（`A2`） | 既有附件明细区 | 保留原入口与原命令，条目表**不**开第二套入口（§6.3 第 7 条） |
| 对账余量 | 规格 §3.11 | 只读展示，无删除入口、不进任何合计（§6.3 第 10 条） |
| 数据来源 | `get_storage_breakdown` 一次拉全 | 不新建 store、不新建 service、不新增轮询（§6.3 第 1 条） |

---

## 1 旧设计作废说明

被作废的是「在 `StorageSettings.tsx` 里线性追加两张表」的做法（含上一轮草稿的 CSS 追加与扁平 i18n 键）。逐条列出问题与本文的替换方案：

| # | 旧做法 | 问题 | 本文替换 |
|---|---|---|---|
| P1 | 290 行单文件继续线性增长，状态、IO、渲染、文案全在一处 | 一个文件对应四种变化原因（数据拉取 / 表格布局 / 回收流程 / 文案），违反拆分主判据 | 拆成 17 个文件，见 §9；页面文件退化为薄壳 |
| P2 | i18n 键扁平混放（`entry_names.*` 与 `inventory.*` 同级） | 无法表达「列 / 徽标 / 详情 / 弹窗 / 结果」层级；且缺键无法被类型检查发现（`I18nPath` 只由 zh-CN 推导） | 命名空间键树 + 枚举全覆盖测试 + zh/en 键路径对等测试，见 §11 |
| P3 | 6 列 `minmax` 等宽 grid，无表头、无域分组、无详情 | 13 条条目平铺，`reason` / `note` / `effectiveSince` 无处可放；用户无法建立「附件 / 工具 / 安全域」的心智分组 | 自绘表格：域分组 + 6 列固定语义 + 单行手风琴详情，见 §5 |
| P4 | 徽标、策略、原因文案散在 JSX 三元里 | 枚举增删时静默显示空白；无法测试 | `lexicon.ts` 纯映射 + `entryView.ts` 纯视图模型，见 §4 / §5.6 |
| P5 | 余量区与条目表混在一个滚动区，无状态层级 | 扫描失败 / 部分扫描 / 超登记三类异常没有位置，容易静默 | 独立第三段 + 警示栈，见 §7 |
| P6 | 回收流程没有状态机，错误码直接 `catch` 后 toast 原文 | `planExpired` 被当成通用失败；可能重复提交 | `reclaimFlow.ts` 纯 reducer + 错误码映射表，见 §6 |
| P7 | 事件监听的去重与清理未定义 | 事件与手动刷新叠加时重复请求 | `useStorageSnapshot.ts` 单飞 + 卸载清理，见 §8 |
| P8 | CSS 只有布局，没有状态修饰符 | 禁用 / 警示 / 超登记三种状态视觉无差别 | 状态修饰符 + 既有令牌，见 §10 |
| P9 | 首载失败与刷新失败都 `setBreakdown(null)` | 刷新一次抖动就把整页清空，用户丢失上下文 | 两态分离：首载失败空态重试；刷新失败保留上次数据 + 顶部提示，见 §8.3 |
| P10 | `bytes = null` 与 `missing`（`0`）未区分 | `null` 直接进 `formatBytes` 会得到 `NaN`；`missing` 显示成未知 | 占位符规则表，见 §5.3 |
| P11 | 与 R0 容器关系未定义 | T4 交付后可能二次搬迁 | `StorageSettings.tsx` 只做薄壳，内容组件与容器解耦，见 §9.1 |

**草稿处置**：上一轮已落地的 `settings.css` 追加（`storage-section-*` / `storage-entry-*` / `storage-inventory-*`）与 `zh-CN` / `en-US` 中的扁平草稿键（`entries_title` / `col_*` / `entry_names` / `reclaim.*` / `inventory.*`）**同属被作废的旧设计**：键按 §11.4 重排、样式按 §10 重写（`storage-inventory-*` 更名为 `storage-residual-*`），随 §15 第 1 / 9 步一次性替换，替换前不得被任何组件引用。`src/services/desktop/storageService.ts` 的三个命令封装保留（它们是契约镜像，与设计无关）。

---

## 2 设计目标

### 2.1 用户任务（按发生频率排序）

| 任务 | 用户此刻关心 | 对应段落 |
|---|---|---|
| 清附件腾空间 | 哪些附件没用、删了释放多少 | 段一（附件明细） |
| 看清各目录谁在占、能不能回收 | 这条是什么、占多少、为什么不能删 | 段二（条目表） |
| 排查「磁盘比条目加起来还大」 | 有没有没被登记的东西、扫描可信吗 | 段三（对账余量） |

三段顺序即上表顺序：**最常用的操作在最上面**，诊断信息在最下面。

### 2.2 三条不可让步的原则

1. **数字只有一个来源**：页面所有数字来自同一次 `get_storage_breakdown` 快照（含 `inventory`），不拼接第二次请求，不缓存第二份。
2. **分支不进组件**：所有「该显示什么 / 按钮能不能点 / 点什么」的判断由纯函数算出视图模型，组件只做「视图模型 → JSX」映射（理由见 §3.2：本仓库测试环境无 DOM 渲染器）。
3. **异常必须可见**：`unreadable` / `notImplemented` / `planExpired` / `partial` / `overRegistered` / 扫描失败，每一类都有确定的展示位置，不允许只打日志或静默取 0。

### 2.3 为什么必须「纯函数出视图模型」

仓库测试栈是 `vitest` + node 环境（`src/testSetup.ts` 只补 `window` 空对象），**没有** `jsdom` 与 `@testing-library/react`。因此：

- 组件层不可测，纯函数层可测；
- 若把分支写在 JSX 里，§13 的全部断言都只能靠人眼回归；
- 所以 `entryView.ts` / `residualView.ts` / `reclaimFlow.ts` 承担全部判断，测试直接吃 DTO 假数据（无需渲染）。

---

## 3 信息架构

### 3.1 结构（内容区）

```
┌─ 快照工具条 ────────────────────────────────────────────────┐
│ 采样于 HH:mm · 口径：文件长度 · [刷新失败提示（仅失败时）]    [刷新] │
├─ 段一 附件明细 ─────────────────────────────────────────────┤
│ 段头：标题 + hint                          共 N 个 · X  [删除选中(N)] │
│ 账本行：合计 / 跨会话共享                                     │
│ [占用过大警告]                                                │
│ 孤儿组 ▸ 会话组 ▸ 会话组 …                                    │
├─ 段二 资源监视条目 ─────────────────────────────────────────┤
│ 段头：标题 + hint                                            │
│ 表头（吸顶）                                                 │
│ 域组：附件      ▾  E1 / E2                                   │
│ 域组：工具      ▾  E3 / E4                                   │
│ 域组：素材      ▾  E5                                        │
│ 域组：数据库    ▾  E6                                        │
│ 域组：安全域    ▾  E7…E11                                    │
│ 域组：日志与诊断 ▾  E12 / E13                                │
├─ 段三 对账余量（只读）──────────────────────────────────────┤
│ 段头：标题 + [只读] + 状态文案                     [扫描 / 重新扫描] │
│ 警示栈：扫描失败 / 部分扫描 / 超登记                          │
│ 节点表：名称 · 类别 · 范围 · 扫描 · 登记 · 余量               │
│ 合计行：未登记合计 N 项 · X（不计入任何页面合计）             │
└────────────────────────────────────────────────────────────┘
```

工具条与三段都是**同一垂直流**，不嵌套滚动：条目表自带定高滚动区（§5.8），其余随页面滚动。

**「两段」与「三段」的关系**：规格 §6.3 第 2 条写「同页两段」（附件明细 + 条目表），第 9 条又追加「对账余量区（第三段，只读）」。两者不矛盾——第 2 条管的是「保留段一 + 追加条目表」，第 9 条是条目表下方的只读追加栏。本文按第 9 条称其为段三，不另立第四段，也不把余量并回条目表。

### 3.2 段头的统一形状

每段段头都是「标题 + 一句 hint（说明这段是什么、数字口径）+ 右侧段级操作」，不重复页标题、不重复页级刷新（刷新在工具条）。

### 3.3 三段之间的互斥关系（写进 UI 文案，不靠用户猜）

- 段一的删除只作用于「选中的附件文件」，走 `delete_attachments`；
- 段二的回收由各 owner 执行，走 plan + execute；
- 两者语义不同、命令不同，**段二段头 hint 必须点明这一点**（`entries.hint`），避免用户把 E1 的「回收」当成段一删除的批量版；
- 段三与钱无关：`unregistered` 不是「可回收」，它连 `reclaim` 字段都没有。

---

## 4 段一：附件明细

### 4.1 保留的能力（V16 逐项回归基线）

| 能力 | 现状 | 本文处置 |
|---|---|---|
| 会话分组（按 `totalBytes` 降序） | 有 | 保留，逻辑原样搬入 `AttachmentSection.tsx` |
| 孤儿分组（`orphans.length > 0` 才出现） | 有 | 保留 |
| 组折叠（`RightOutlined` / `DownOutlined`） | 有 | 保留 |
| 组全选（`checked` / `indeterminate`） | 有 | 保留 |
| 单选删除（`selectedIds` + 危险按钮） | 有 | 保留 |
| 删除确认弹窗（不可逆文案） | 有 | 保留（`modal.confirm`，`okButtonProps.danger`） |
| 删除后重拉 + 清空勾选 | 有 | 保留 |
| 占用 > 500 MiB 警告 | 有 | 保留（阈值提为常量 `SIZE_WARNING_THRESHOLD_BYTES`，值不变） |
| 首载失败空态 | 有 | 保留，文案键迁到 `common.load_failed` |

### 4.2 允许的「追加」（不删任何既有能力）

1. 段头形态统一（标题 + hint + 操作），摘要行从「无标题的裸行」升级为段头右侧信息；
2. 账本行追加「跨会话共享 X」（`sharedBytes > 0` 才显示，`OQ1` 允许单列）；
3. 账本行追加口径与采样时刻（`byteBasis` / `sampledAt` 是 DTO 字段，规格要求口径进 DTO，UI 应可见）；
4. 删除成功追加 `message.success`（`removedCount` / `removedBytes` 来自 `AttachmentDeleteResultDto`，此前返回值未被使用）；
5. 删除失败追加 `message.error`（此前 `onOk` 抛错只让弹窗停住，无提示）；
6. 首载失败空态追加「重试」按钮（此前只能靠页头刷新，且空态下没有指向性）。

以上 6 条均为纯追加，`selectedIds` 语义、勾选行为、分组排序、确认文案全部不变。

### 4.3 账本行的三数关系（不在前端校验）

规格 §10 V5 要求 `totalBytes = referencedOnceBytes + sharedBytes + orphanBytes`，但**该恒等式由后端保证**，前端不重算、不显示等式、不显示 `referencedOnceBytes`（`OQ1`：本期不单列）。前端只显示：

- `ledger.total`：共 `totalCount` 个附件，占用 `totalBytes`；
- `ledger.shared`：其中跨会话共享 `sharedBytes`（>0 时）。

### 4.4 组件拆分

| 文件 | 职责（唯一变化原因） |
|---|---|
| `AttachmentSection.tsx` | 段头 + 账本行 + 警告 + 分组容器（迁移既有逻辑） |
| 组头 / 行渲染 | 留在 `AttachmentSection.tsx` 内部（同一变化原因：附件列表的呈现），不额外拆文件；若迁移后仍超 300 行，再按「组头 / 行」拆分 |

---

## 5 段二：资源监视条目表

### 5.1 数据形状（只用这些字段）

`breakdown.entries` 顺序固定（`E1`→`E13`），不可用条目也必须在场。行内只消费：

| 字段 | 用途 |
|---|---|
| `key` | 稳定身份（名称映射、强确认输入、详情展示） |
| `layer` | 详情展示（`data` / `local`） |
| `owner` | 详情展示（`由 {{owner}} 提供`） |
| `bytes` / `count` | 占用列 |
| `oldestAt` / `newestAt` | 详情 |
| `availability` / `availabilityReason` | 可用性列 |
| `includesDerived` / `countsSelfLogs` | 详情（口径提示 chip） |
| `reclaim.action` / `.eligibility` / `.reason` / `.confirmation` / `.note` / `.autoActive` / `.reclaimableBytes` / `.reclaimableCount` | 资格列、策略列、动作列、详情 |
| `reclaim.policy.{source, retentionDays, keepCount, maxBytes, effectiveSince, mutable, failClosed}` | 策略 chip + 详情 |

**不消费**：`breakdown.complete`（本期不展示任何跨条目合计，故无守卫对象）、`totalReclaimableBytes`（回收流程按条目串行，见 §6.1）。

### 5.2 名称映射：按条目 id，不按 key

条目 `key` 含 `/` 与 `.`（`attachments/derived`、`assistant.db`、`security/vault`），而 i18next 默认以 `.` 与 `/` 作路径分隔，**不能**直接作为 i18n 路径段。因此：

- 名称映射用条目 id（`E1`..`E13` → `entries.names.e1`..`e13`），id 是登记表声明的对外契约（`registry.rs` 注释：「条目 id 是对外契约」）；
- `key` 只在需要精确身份的地方出现（强确认输入、详情、tooltip）；
- 名称表由 `lexicon.ts` 提供 `entryNameText(key)`，内部维护 `key → id` 的静态映射；`key` 不在映射内（后端新增条目但前端未更新）时，**回退显示 `key` 原文**并记一条 `console.warn`，不显示空白、不猜名称。

**`lexicon.ts` 的写法照抄 `src/audit/lexicon.ts`**，不另创一套：

- 每个枚举一张 `as const satisfies Record<Enum, I18nPath>` 表 —— **编译期**保证「新增枚举值必然有键」，这比运行期测试更强；
- `lookupKey(keys, value)` 用 `Object.prototype.hasOwnProperty.call` 判存在（不写成 `keys[value] ?? …`，避免 `constructor` / `toString` 之类的原型键误命中）；
- 导出函数返回**已翻译的字符串**（`translateKey(key)`），未命中时返回原始值。`translateKey` 是 `src/i18n/index.ts` 里唯一的动态键入口（已登记的类型安全例外），本设计不新增第二处；
- 因此**组件不直接对枚举调 `t()`**，只渲染 lexicon 返回的字符串；`t()` 只用于带参数的静态文案（列头、按钮、弹窗正文）。

### 5.3 占位符规则（`null` 与 `0` 的区别）

| 情况 | 语义 | 占用列主行 | 次行（条数） |
|---|---|---|---|
| `bytes === null` | 取不到数 | `—`（`common.bytes_unknown`），**不调用** `formatBytes` | 隐藏 |
| `bytes === 0` 且 `availability = missing` | 目录确实不存在，计 0 | `0 B` | 隐藏（`count` 为 0） |
| `bytes > 0` | 正常 | `formatBytes(bytes)` | `{{count}} 项`（`count` 非 null 时） |
| `count === null` 且 `bytes !== null` | 条数不可得 | 正常 | 隐藏 |

规则以 `bytesOrDash()` 单点实现（`format.ts`），组件内**禁止**再出现 `formatBytes(x ?? 0)` 或 `x ? … : …` 的数值兜底。

### 5.4 列定义

| # | 列 | 宽度 | 内容 |
|---|---|---|---|
| 1 | 名称 | `minmax(220px, 1.4fr)` | 第一行：本地化名称 + 展开箭头（行末）；第二行：`key`（等宽 10px 次要色，单行省略 + tooltip） |
| 2 | 占用 / 条数 | `120px` | 见 §5.3 |
| 3 | 可用性 | `120px` | Tag + （`availabilityReason` 非空时）`InfoCircleOutlined` 角标，tooltip 显示 reason 原文 |
| 4 | 回收资格 | `176px` | Tag + 第二行文案（见 §5.6） |
| 5 | 策略 | `152px` | chip 组（最多 2 枚 + `+N`） |
| 6 | 动作 | `88px` | 按钮 / 文本 / `—`（见 §5.7） |

列宽常量与域分组常量放在 `entryColumns.ts`（纯数据，无 React 导入），grid 模板由该常量生成 inline style —— 与既有 `PermissionAuditSection/AuditTable.tsx` 同法，避免 CSS 与 TS 两处列宽漂移。

**不做列宽拖拽**：13 行固定、列宽由内容决定，拖拽收益低，且会把审计表的 `useColumnResize` 机制复制第二份（共享叶子未下沉时不复制，见 §9.3）。

### 5.5 域分组

| 组 | 条目 | 组头显示 |
|---|---|---|
| 附件 | E1 `attachments`、E2 `attachments/derived` | 组名 + 条目数 |
| 工具 | E3 `tool-results`、E4 `tool-index` | 同上 |
| 素材 | E5 `mascot-assets` | 同上 |
| 数据库 | E6 `assistant.db` | 同上 |
| 安全域 | E7 `security/security.db`、E8 `security/backups`、E9 `security/vault`、E10 `security/recovery`、E11 `security/audit` | 同上 |
| 日志与诊断 | E12 `logs`、E13 `crashes` | 同上 |

硬规则：

1. 组头**不带复选框**（否则等于「一键清理该域」，违反 X4 / G10 精神）；
2. 组头**不显示字节合计**（跨条目相加只有附件账本与 `totalReclaimableBytes` 两处允许，G10）；
3. 组内顺序 = `entries` 数组顺序，**不提供排序**（不按大小排序，避免诱导「先清最大的」）；
4. 组可折叠，默认全展开；折叠状态只存组件内存，不持久化；
5. 分组常量与 `entries` 中出现的 `key` 必须一一对应；出现未分组 `key` 时归入「其他」组（兜底可见，不隐藏）。

### 5.6 徽标与第二行文案（`lexicon.ts` 单点映射）

| 列 | 第一行 | 第二行 |
|---|---|---|
| 可用性 | `availabilityText(entry.availability)` Tag；`available` 用中性色、`missing` 中性、`locked` 橙、`unreadable` / `unavailable` 红、`notImplemented` 蓝灰虚边 | 不显示（reason 走 tooltip + 详情） |
| 回收资格 | `eligibilityText(entry.reclaim.eligibility)` Tag | `reason` 非空 → `reasonText(reason)`（本地化枚举文案）；`reason` 为空 → 空；`notImplemented` 且有 `note` → **note 原文**（覆盖 reason 文案，详情区两者都给） |
| 策略 | chip：`policySourceText(policy.source)` + 非空的 `retentionDays` / `keepCount` / `maxBytes` 中取前 2 枚（字段名见 `ReclaimPolicyDto`）；`reclaim.autoActive` 为真时追加 `entries.policy.auto_active` chip | 不显示（`effectiveSince` / `mutable` / `failClosed` 在详情） |

硬规则：

1. **`availabilityReason` 与 `note` 是后端自由文本，原样展示**（含语言），不映射成前端枚举文案、不翻译、不截断成固定词；否则等于前端维护第二套判据词表（X7 的变体）；
2. 枚举值未命中词表（后端新增值）→ 显示**原始枚举值**并 `console.warn`，**禁止**兜底成「可用」或空白（静默会把未知说成安全）；
3. `eligibility = ownerManaged` 时不显示「回收」按钮（§5.7），因此第二行文案只在 `notEligible` / `locked` / `notImplemented` 有意义。

### 5.7 动作列判定（唯一分支点，实现在 `entryView.ts`）

| `reclaim.eligibility` | `reclaim.action` | 动作列 |
|---|---|---|
| `eligible` | ≠ `none` | 危险文字按钮「回收」；`confirmation = strongConfirm` 追加 `SafetyOutlined` 图标 |
| `eligible` | `none` | `—`（数据自相矛盾时降级为不可点，并 `console.warn`） |
| `notEligible` / `locked` / `notImplemented` | ≠ `none` | 禁用按钮「回收」+ Tooltip（`reason` 文案，`notImplemented` 追加 note） |
| `ownerManaged` | ≠ `none` | 纯文本「自动管理」（11px 次要色），无按钮、无 Tooltip |
| 任意 | `none` | `—`，无按钮（Tooltip 无宿主） |

实现细节：

- 禁用按钮的 Tooltip 必须包裹 `span`（antd 对 disabled 元素不触发 hover）；
- 禁用原因不能只放 Tooltip：资格列第二行已显示 `reason`，Tooltip 是冗余提示而非唯一出口；
- `confirmation` 只决定「点击后弹哪种弹窗」，**不参与**是否可点（是否可点只看 `eligibility`）。

### 5.8 表格容器

- 定高滚动：`max-height: 400px`，`overflow: auto`，`overscroll-behavior: contain`；
- 表头 `position: sticky; top: 0`；
- 域组头 `position: sticky; top: 32px`（表头行高 32）；若二级吸顶在实现中与滚动容器冲突，**退化为仅表头吸顶**并在代码注释写明取舍，不得为此引入第三方表格库；
- 行高：主行 `min-height: 40px`（两行单元格），详情展开区不占主行高度；
- 容器 `min-width: 856px`（列宽之和），窄于此横向滚动，**不折叠列**（折叠会造成信息缺失）。

### 5.9 详情展开（单行手风琴）

- 名称列末尾一个 `RightOutlined` / `DownOutlined`（旋转 90°）切换按钮，`aria-expanded` + `aria-controls`；
- 同一时刻最多展开一行（`expandedKey: string | null`），再次点击收起；
- 详情区 `grid-column: 1 / -1`，两列 key-value，12px 字号，不参与排序/筛选；
- 详情内容固定为 8 项：

| 项 | 值 | 空值 |
|---|---|---|
| 条目 id / key | `E1` / `attachments`（等宽） | 恒有 |
| 层 | `entries.facts.layer` + `data` / `local` 原文 | 恒有 |
| 提供方 | `entries.provided_by`（`{{owner}}`） | 恒有 |
| 时间范围 | 最早 `oldestAt` · 最近 `newestAt` | `—` |
| 可回收 | `reclaim.reclaimableCount` / `reclaim.reclaimableBytes`（后端值） | `—` |
| 策略 | `source` / `effectiveSince` / `mutable` / `failClosed` | `—` |
| 口径标记 | `includesDerived` / `countsSelfLogs` 两个 chip（真值才出现） | 都不出现 |
| 诊断 | `availabilityReason` / `note`（原文） | 都不出现 |

---

## 6 回收流程（段二的动作）

### 6.1 流程与状态机

```
idle ──点击回收──▶ planning ──计划到达──▶ confirming ──确认──▶ executing ──▶ done / failed
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
| `confirm` | `Modal`（受控，非 `Modal.confirm`）：标题「回收「{{name}}」？」、正文三件套（动作 / 预计条数 / 预计字节）+ 不可撤销警示 | 点「执行回收」 |
| `strongConfirm` | 同上 + 输入框：要求输入**条目 key**（等宽字体展示待输入值） | 输入与 `key` 完全相等才启用「执行回收」；不等时按钮禁用并显示 `strong.mismatch` |

为什么强确认输入 `key` 而不是本地化名称：名称随语言变化、含空格与全角字符，用户无法准确复现；`key` 稳定、可复制、与后端身份一致。弹窗同时显示「名称（key）」，满足「二次输入条目名」的意图而不引入歧义。

交互细节：

- 输入框 `autoFocus`；回车 = 提交（仅在匹配时生效）；Esc = 取消；
- `executing` 期间：确认按钮 loading、取消按钮禁用（后端已开始删除，取消无意义）；
- 弹窗关闭后焦点回到触发的行按钮（`ref` 保存）；
- 同一时刻只允许一个流程（`planning` / `executing` 期间其余行按钮禁用，避免并发触发 `busy`）。

### 6.3 错误码映射（`reclaimFlow.ts` 单点）

命令错误以字符串码返回（`reclaim.rs`：`planNotFound` / `planExpired` / `confirmationRequired` / `busy` / `entryNotAllowed`），前端映射：

| 错误码 | 提示 | 弹窗 | 是否自动重放 |
|---|---|---|---|
| `busy` | `reclaim.error.busy`（另一项回收正在进行，请稍后重试） | 关闭 | **否** |
| `planExpired` | `reclaim.error.plan_expired`（计划已过期，请重试） | 关闭 | **否** |
| `planNotFound` | `reclaim.error.plan_not_found`（计划不存在，请重试） | 关闭 | **否** |
| `entryNotAllowed` | `reclaim.error.not_allowed`（该条目当前不允许回收） | 关闭 | 否 |
| `confirmationRequired` | `reclaim.error.confirmation_required` | 保持打开 | 否（这是前端漏传 `confirmed` 的信号，属缺陷，不重试） |
| 计划阶段其他错误 | `reclaim.error.plan_failed` | 关闭 | 否 |
| 执行阶段其他错误 | `reclaim.error.execute_failed`（`{{error}}` = 原文） | 关闭 | 否 |

规格 §6.3 第 8 条「提示重试，不自动重放」在此落为：**只提示，用户自己再点一次**。任何自动重试都可能导致「用户没看到就删了」。

### 6.4 结果呈现

执行成功后先关闭弹窗，再按 `result.entries[0].status` 分支（`toast` 用 `message`，时长 4s）：

| `status` | 文案 | 追加 |
|---|---|---|
| `done` | `reclaim.result.done`（已回收 N 项，释放 X） | — |
| `partial` | `reclaim.result.partial`（删除 N 项，释放 X，失败 M 项） | 失败项原因取 `errors[0].code` → `reclaim.error_code.<code>` 词表（`permissionDenied` / `io` / `ownerUnavailable` / `policyDisabled` / `locked` / `unknown`） |
| `failed` | `reclaim.result.failed`（回收失败） | 同上 |
| `skipped` | `reclaim.result.skipped`（没有可回收的内容，可能刚被引用） | — |
| `notEligible` | 理论不可达（计划已过滤），出现即 `console.warn` + `reclaim.error.not_allowed` | — |

无论哪种状态，**都重拉一次** `get_storage_breakdown`（`execute` 之后目录已变，旧快照必然失真）。

### 6.5 组件边界

| 文件 | 职责 |
|---|---|
| `reclaimFlow.ts` | 纯 reducer（`idle / planning / confirming / executing`）+ 错误码 → 文案键映射 + 结果 → 提示描述；无 IO、无 React |
| `useReclaim.ts` | 副作用壳：调 `getStorageReclaimPlan` / `reclaimStorage`，把结果喂给 reducer，成功后调快照刷新回调 |
| `ReclaimDialog.tsx` | 只吃 props（`open` / `entry` / `plan` / `phase` / `onConfirm` / `onCancel`），不调 service |

---

## 7 段三：对账余量（只读）

### 7.1 数据来源

`breakdown.inventory`（与条目、账本同一次快照）。因此：

- 打开页面**不发**第二个请求；
- `storage:inventory-updated` 事件到达后重拉 `get_storage_breakdown`（一次），三段一起更新，不会出现「条目是新数字、余量是旧数字」的错帧；
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

`overRegistered = true` 的行：整行红底（`storage-residual-row is-over`）+ 余量列图标 + 该节点 Tooltip 显示 `residual.notice.over_registered`。规格 §6.3 第 9 条要求「必须显著标红并给固定文案，不得静默」。

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
  StorageContent.tsx      组合入口：工具条 + 三段；只做顺序与传参，无业务分支
  SnapshotToolbar.tsx     采样时刻 / 口径 / 刷新失败提示 / 刷新按钮
  AttachmentSection.tsx   段一（迁移既有逻辑，见 §4）
  EntrySection.tsx        段二：段头 + EntryTable 容器 + 回收流程接线
  EntryTable.tsx          表头 / 域组 / 行 / 详情切换（吃 EntryRowViewModel[]）
  EntryDetail.tsx         详情展开区（key-value 列表）
  ResidualSection.tsx     段三：段头 + 五态 + 扫描按钮
  ResidualTable.tsx       警示栈 + 节点表 + 合计行（吃 ResidualViewModel）
  ReclaimDialog.tsx       两级确认弹窗（纯 props）
  entryColumns.ts         列键 / 列宽 / 域分组 / 表格常量（无 React 导入）
  entryView.ts            条目视图模型纯函数（占位符、徽标、按钮态、第二行文案）
  residualView.ts         余量视图模型纯函数（排序、过滤、警示栈、合计）
  reclaimFlow.ts          回收 reducer + 错误码映射 + 结果文案键
  lexicon.ts              枚举 → 已翻译文案（availability / eligibility / reason / action / policy / class / scope / 条目名 / 错误码），写法同 src/audit/lexicon.ts
  format.ts               bytesOrDash / 时间 / 条数（`formatTimestamp` 从旧文件机械搬入）
  useStorageSnapshot.ts   拉取 / 刷新 / 事件重拉 / 单飞
  useReclaim.ts           reducer ↔ service 的副作用壳
  __tests__/              lexicon / entryView / residualView / reclaimFlow / localeParity
```

### 9.2 依赖方向（单向，无环）

```
StorageSettings.tsx → StorageContent.tsx
StorageContent.tsx  → SnapshotToolbar / AttachmentSection / EntrySection / ResidualSection / useStorageSnapshot
EntrySection.tsx    → EntryTable / ReclaimDialog / useReclaim / entryView
EntryTable.tsx      → entryColumns / EntryDetail / format
EntryDetail.tsx     → lexicon / format
ResidualSection.tsx → ResidualTable / residualView
ResidualTable.tsx   → residualView / format
entryView.ts        → entryColumns / lexicon / format
residualView.ts     → lexicon / format
reclaimFlow.ts      → lexicon（错误码词表）/ format
useReclaim.ts       → storageService / reclaimFlow
```

规则：纯函数模块（`entryView` / `residualView` / `reclaimFlow` / `lexicon` / `format`）**不导入 React、不导入 service**；组件**不导入 service**（只经 hook）。

### 9.3 与既有模块的关系

| 既有物 | 处置 | 理由 |
|---|---|---|
| `pages/StorageSettings.tsx` 路径 | 保留文件名，内容改为薄壳 | 路由与页面注册引用该路径，改路径等于动注册表 |
| `SettingsPageShell` | T4 期间继续包裹薄壳；R0 交付后由 tab 容器替换 | 内容组件不依赖壳，二次搬迁成本为 0 |
| `PermissionAuditSection/useColumnResize` | **不复用** | 跨 section 目录引用会形成耦合；且本表不需要拖拽（§5.4）。若将来第二个表格需要拖拽，再把 hook 下沉到 `settings/shared/hooks/`（共享叶子下沉），不在 T4 顺手搬 |
| `utils/formatBytes` | 不改签名 | 占位符由 `format.ts` 的 `bytesOrDash` 承担（任务清单明确禁止改签名） |
| `settings.css` | 追加类，不重命名既有类 | 既有 `storage-*` 类继续服务段一 |

---

## 10 样式与视觉规范

### 10.1 类名

| 前缀 | 归属 | 例 |
|---|---|---|
| `storage-`（既有） | 段一 | `storage-summary-row` / `storage-group-header` / `storage-attachment-row` |
| `storage-toolbar-` | 工具条 | `storage-toolbar` / `storage-toolbar-meta` |
| `storage-section-` | 段头 | `storage-section-head` / `storage-section-title` / `storage-section-hint` |
| `storage-entry-` | 段二 | `storage-entry-table` / `storage-entry-header` / `storage-entry-group` / `storage-entry-row` / `storage-entry-detail` |
| `storage-residual-` | 段三 | `storage-residual-table` / `storage-residual-row` / `storage-residual-total` |

状态修饰符（一律 `is-` 前缀，与既有 CSS 习惯一致）：`is-eligible` / `is-blocked` / `is-owner` / `is-warning` / `is-over` / `is-collapsed` / `is-running`。

### 10.2 视觉规则

| 项 | 取值 |
|---|---|
| 颜色 | 只用既有 CSS 变量（`--color-border-secondary` / `--color-bg-elevated` / 语义色）；不新增色值常量、不写死 `#hex` |
| 字体 | 名称 12px 常规；`key` / 节点名 12px 等宽；第二行文案 10px；表头 11px 次要色 |
| 行高 | 表头 32px；条目行 `min-height: 40px`；余量行 32px |
| 分隔 | 列间 `1px solid var(--color-border-secondary)`（与审计表一致），域组头下边框加粗 |
| 危险色 | 只用于「回收」按钮、`unreadable` / `unavailable` 徽标、`overRegistered` 行与图标；行底用 `--color-error-bg`，图标/文字用 `--color-error`；警示文案用 `--color-warning` |
| 数字对齐 | 字节/条数列右对齐 + `font-variant-numeric: tabular-nums` |
| 悬停 | 条目行 `hover` 提亮背景；禁用按钮不显示 hover 变化 |

### 10.3 密度与响应式

- 内容区最小宽度 856px（表格列宽和），窄于此横向滚动，不折叠列、不隐藏信息；
- 三段之间间距 24px（与 `SettingsSection` 的 `marginTop` 一致）；
- 段三与段二之间不加大分隔装饰（两段都是「信息区」，靠段头标题区分）。

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
│               ledger.{total, shared}, orphan_group}
├─ entries.{title, hint, provided_by, details, hide_details,
│           columns.{name,size,availability,eligibility,policy,action},
│           domains.{attachments,tools,mascot,database,security,diagnostics,other},
│           names.{e1..e13},
│           availability.{available,missing,locked,unreadable,unavailable,notImplemented,noReportYet,staleReport},
│           eligibility.{eligible,notEligible,locked,ownerManaged,notImplemented},
│           reason.{orphan,expired,overLimit,derivedCache,transient,referenced,userData,
│                   businessData,replayRisk,locked,noStats,noReclaimApi,externalOwner,
│                   policyDisabled,unknown},
│           action.{none,deleteOrphans,deleteSelected,sweepExpired,pruneToLimit,rebuildCache,rotate},
│           policy.{source.{config,constant,owner,none}, retention, retention_disabled,
│                   keep_count, max_bytes, auto_active},
│           facts.{layer,oldest,newest,reclaimable,effective_since,mutable,fail_closed,
│                  includes_derived,counts_self_logs,availability_reason,note},
│           reclaim_button}
├─ reclaim.{planning, executing, ok, cancel,
│           dialog.{title,body,key_hint,irreversible},
│           strong.{title,body,placeholder,mismatch},
│           error.{busy,plan_expired,plan_not_found,not_allowed,confirmation_required,
│                  plan_failed,execute_failed},
│           error_code.{permissionDenied,io,ownerUnavailable,policyDisabled,locked,unknown},
│           result.{done,partial,failed,skipped}}
└─ residual.{title, read_only, hint, scan, scanning, rescan, retry_scan,
             status.{neverRun,running,fresh,stale,failed},
             notice.{failed_keep_last,partial,truncated_more,over_registered,empty,scan_failed},
             columns.{name,class,scope,scan,declared,residual},
             class.{declared,optionalNotMounted,companion,ignored,unregistered},
             scope.{data,local},
             totals.{unregistered,hint},
             children.{more}}
```

### 11.2 文案取值（zh-CN / en-US 一一对应）

| 键 | zh-CN | en-US |
|---|---|---|
| `toolbar.refresh` | 刷新 | Refresh |
| `toolbar.sampled_at` | 采样于 {{time}} | Sampled at {{time}} |
| `toolbar.byte_basis` | 口径：文件长度 | Basis: file length |
| `toolbar.refresh_failed` | 刷新失败，以下为上次数据 | Refresh failed; showing previous data |
| `common.load_failed` | 加载存储数据失败 | Failed to load storage data |
| `common.retry` | 重试 | Retry |
| `common.bytes_unknown` | — | — |
| `common.count_value` | {{count}} 项 | {{count}} items |
| `common.time_unknown` | 未知 | Unknown |
| `attachments.title` | 附件明细 | Attachments |
| `attachments.hint` | 按对话分组；删除只移除选中的附件文件 | Grouped by conversation; deletion removes only the selected files |
| `attachments.delete_selected` | 删除选中（{{count}}） | Delete selected ({{count}}) |
| `attachments.confirm_delete.title` | 删除选中的附件？ | Delete the selected attachments? |
| `attachments.confirm_delete.content` | 此操作不可逆，文件将被永久删除。 | This cannot be undone; the files are deleted permanently. |
| `attachments.deleted` | 已删除 {{count}} 个附件，释放 {{size}} | Removed {{count}} attachments, freed {{size}} |
| `attachments.delete_failed` | 删除失败，请重试 | Deletion failed; please retry |
| `attachments.empty` | 暂无附件 | No attachments |
| `attachments.size_warning` | 附件占用较大，建议清理以释放磁盘空间 | Attachments take a lot of space; consider cleaning up |
| `attachments.ledger.total` | 共 {{count}} 个附件，占用 {{size}} | {{count}} attachments, {{size}} |
| `attachments.ledger.shared` | 其中跨会话共享 {{size}} | {{size}} shared across conversations |
| `attachments.orphan_group` | 无关联对话的附件 | Attachments without a conversation |
| `entries.title` | 资源监视条目 | Monitored entries |
| `entries.hint` | 条目由各自的 owner 提供观测；可回收量以计划为准，页面不做推算；此处的回收与上方附件删除是两条独立入口 | Entries are observed by their owners; reclaimable amounts come from the plan. Reclaim here is separate from attachment deletion above |
| `entries.provided_by` | 由 {{owner}} 提供 | Provided by {{owner}} |
| `entries.details` | 详情 | Details |
| `entries.hide_details` | 收起 | Hide |
| `entries.columns.name` | 条目 | Entry |
| `entries.columns.size` | 占用 / 条数 | Size / count |
| `entries.columns.availability` | 可用性 | Availability |
| `entries.columns.eligibility` | 回收资格 | Eligibility |
| `entries.columns.policy` | 策略 | Policy |
| `entries.columns.action` | 动作 | Action |
| `entries.domains.attachments` | 附件 | Attachments |
| `entries.domains.tools` | 工具 | Tools |
| `entries.domains.mascot` | 素材 | Mascot |
| `entries.domains.database` | 数据库 | Database |
| `entries.domains.security` | 安全域 | Security |
| `entries.domains.diagnostics` | 日志与诊断 | Logs & diagnostics |
| `entries.domains.other` | 其他 | Other |
| `entries.names.e1` | 附件 | Attachments |
| `entries.names.e2` | 附件派生缓存 | Attachment derived cache |
| `entries.names.e3` | 工具结果 | Tool results |
| `entries.names.e4` | 工具索引缓存 | Tool index cache |
| `entries.names.e5` | 形象素材 | Mascot assets |
| `entries.names.e6` | 主数据库 | Main database |
| `entries.names.e7` | 安全库 | Security database |
| `entries.names.e8` | 安全库备份 | Security backups |
| `entries.names.e9` | 保险库 | Vault |
| `entries.names.e10` | 恢复临时区 | Recovery temp |
| `entries.names.e11` | 审计日志 | Audit log |
| `entries.names.e12` | 运行日志 | Runtime logs |
| `entries.names.e13` | 崩溃报告 | Crash reports |
| `entries.availability.available` | 可用 | Available |
| `entries.availability.missing` | 不存在 | Not present |
| `entries.availability.locked` | 已锁定 | Locked |
| `entries.availability.unreadable` | 不可读 | Unreadable |
| `entries.availability.unavailable` | 不可用 | Unavailable |
| `entries.availability.notImplemented` | 未实现 | Not implemented |
| `entries.availability.noReportYet` | 暂无数据 | No data yet |
| `entries.availability.staleReport` | 数据过期 | Stale data |
| `entries.eligibility.eligible` | 可回收 | Reclaimable |
| `entries.eligibility.notEligible` | 当前不可回收 | Not reclaimable |
| `entries.eligibility.locked` | 已锁定 | Locked |
| `entries.eligibility.ownerManaged` | 由 owner 管理 | Owner managed |
| `entries.eligibility.notImplemented` | 未实现 | Not implemented |
| `entries.reason.orphan` | 引用集合之外 | Outside the reference set |
| `entries.reason.expired` | 早于保留期 | Older than retention |
| `entries.reason.overLimit` | 超过数量上限 | Over the count limit |
| `entries.reason.derivedCache` | 可重建缓存 | Rebuildable cache |
| `entries.reason.transient` | 瞬态产物 | Transient artifact |
| `entries.reason.referenced` | 仍被引用 | Still referenced |
| `entries.reason.userData` | 用户数据 | User data |
| `entries.reason.businessData` | 业务数据 | Business data |
| `entries.reason.replayRisk` | 删除会让历史回放定位失效 | Deleting breaks historical replay |
| `entries.reason.locked` | 受保护 | Protected |
| `entries.reason.noStats` | 缺统计入口 | No stats entry point |
| `entries.reason.noReclaimApi` | 缺回收入口 | No reclaim API |
| `entries.reason.externalOwner` | 由外部 owner 管理 | Managed by an external owner |
| `entries.reason.policyDisabled` | 保留策略已禁用 | Retention policy disabled |
| `entries.reason.unknown` | 判据不可得 | Criteria unavailable |
| `entries.action.none` | 不可回收 | No reclaim action |
| `entries.action.deleteOrphans` | 删除孤儿 | Delete orphans |
| `entries.action.deleteSelected` | 删除选中 | Delete selected |
| `entries.action.sweepExpired` | 清理过期 | Sweep expired |
| `entries.action.pruneToLimit` | 裁剪到上限 | Prune to limit |
| `entries.action.rebuildCache` | 重建缓存 | Rebuild cache |
| `entries.action.rotate` | 自动轮转 | Auto rotate |
| `entries.policy.source.config` | 配置策略 | Config policy |
| `entries.policy.source.constant` | 常量策略 | Constant policy |
| `entries.policy.source.owner` | 由 owner 管理 | Owner managed |
| `entries.policy.source.none` | 无策略 | No policy |
| `entries.policy.retention` | 保留 {{days}} 天 | Retain {{days}} days |
| `entries.policy.retention_disabled` | 保留策略已禁用 | Retention disabled |
| `entries.policy.keep_count` | 保留 {{count}} 份 | Keep {{count}} |
| `entries.policy.max_bytes` | 条目上限 {{size}} | Entry cap {{size}} |
| `entries.policy.auto_active` | 已自动执行 | Auto-run active |
| `entries.facts.layer` | 层 | Layer |
| `entries.facts.oldest` | 最早 {{time}} | Oldest {{time}} |
| `entries.facts.newest` | 最近 {{time}} | Newest {{time}} |
| `entries.facts.reclaimable` | 可回收 {{count}} 项 / {{size}} | Reclaimable {{count}} items / {{size}} |
| `entries.facts.effective_since` | 生效自 {{time}} | Effective since {{time}} |
| `entries.facts.mutable` | 可修改 | Mutable |
| `entries.facts.fail_closed` | 读不到即不删 | Fail-closed |
| `entries.facts.includes_derived` | 含派生缓存 | Includes derived cache |
| `entries.facts.counts_self_logs` | 统计本进程日志 | Counts own-process logs |
| `entries.facts.availability_reason` | 可用性说明 | Availability reason |
| `entries.facts.note` | 备注 | Note |
| `entries.reclaim_button` | 回收 | Reclaim |
| `reclaim.planning` | 正在生成计划… | Preparing plan… |
| `reclaim.executing` | 正在回收… | Reclaiming… |
| `reclaim.ok` | 执行回收 | Reclaim |
| `reclaim.cancel` | 取消 | Cancel |
| `reclaim.dialog.title` | 回收「{{entry}}」？ | Reclaim "{{entry}}"? |
| `reclaim.dialog.body` | 动作：{{action}}；预计 {{count}} 项，{{size}}。 | Action: {{action}}; about {{count}} items, {{size}}. |
| `reclaim.dialog.key_hint` | 条目 key：{{key}} | Entry key: {{key}} |
| `reclaim.dialog.irreversible` | 此操作不可撤销。 | This cannot be undone. |
| `reclaim.strong.title` | 回收高风险条目「{{entry}}」？ | Reclaim high-risk entry "{{entry}}"? |
| `reclaim.strong.body` | 动作：{{action}}；预计 {{count}} 项，{{size}}。请输入条目 key「{{key}}」以确认。 | Action: {{action}}; about {{count}} items, {{size}}. Type the entry key "{{key}}" to confirm. |
| `reclaim.strong.placeholder` | 输入 {{key}} | Type {{key}} |
| `reclaim.strong.mismatch` | 输入不匹配，无法执行回收 | Input does not match; reclaim is blocked |
| `reclaim.error.busy` | 另一项回收正在进行，请稍后重试 | Another reclaim is running; please retry later |
| `reclaim.error.plan_expired` | 回收计划已过期，请重试 | The plan expired; please retry |
| `reclaim.error.plan_not_found` | 回收计划不存在，请重试 | The plan was not found; please retry |
| `reclaim.error.not_allowed` | 该条目当前不允许回收 | This entry cannot be reclaimed now |
| `reclaim.error.confirmation_required` | 需要确认后才能执行 | Confirmation is required |
| `reclaim.error.plan_failed` | 获取回收计划失败，请重试 | Failed to prepare the plan; please retry |
| `reclaim.error.execute_failed` | 回收执行失败：{{error}} | Reclaim failed: {{error}} |
| `reclaim.error_code.permissionDenied` | 权限不足 | Permission denied |
| `reclaim.error_code.io` | 读写失败 | I/O error |
| `reclaim.error_code.ownerUnavailable` | owner 不可用 | Owner unavailable |
| `reclaim.error_code.policyDisabled` | 保留策略已禁用 | Retention policy disabled |
| `reclaim.error_code.locked` | 被占用或锁定 | Busy or locked |
| `reclaim.error_code.unknown` | 未知原因 | Unknown reason |
| `reclaim.result.done` | 已回收 {{count}} 项，释放 {{size}} | Reclaimed {{count}} items, freed {{size}} |
| `reclaim.result.partial` | 部分完成：删除 {{count}} 项，释放 {{size}}，失败 {{failed}} 项 | Partially done: removed {{count}} items, freed {{size}}, {{failed}} failed |
| `reclaim.result.failed` | 回收失败 | Reclaim failed |
| `reclaim.result.skipped` | 没有可回收的内容（可能刚被引用） | Nothing to reclaim (possibly just referenced) |
| `residual.title` | 对账余量 | Reconciliation residual |
| `residual.read_only` | 只读 | Read-only |
| `residual.hint` | 列出受管根目录中未被条目登记的部分，不参与任何合计 | Parts of managed roots not registered by any entry; excluded from all totals |
| `residual.scan` | 扫描 | Scan |
| `residual.scanning` | 扫描中 | Scanning |
| `residual.rescan` | 重新扫描 | Rescan |
| `residual.retry_scan` | 重试扫描 | Retry scan |
| `residual.status.neverRun` | 尚未扫描 | Not scanned yet |
| `residual.status.running` | 扫描中 | Scanning |
| `residual.status.fresh` | 上次扫描：{{time}} | Last scanned: {{time}} |
| `residual.status.stale` | 上次扫描：{{time}}（可能过期） | Last scanned: {{time}} (may be stale) |
| `residual.status.failed` | 扫描失败：{{time}} | Scan failed: {{time}} |
| `residual.notice.failed_keep_last` | 扫描失败，以下为上次成功扫描的数字 | Scan failed; numbers below are from the last successful scan |
| `residual.notice.partial` | 有界扫描未覆盖完，以下数字是下界 | The bounded scan is incomplete; numbers below are lower bounds |
| `residual.notice.truncated_more` | 另有 {{count}} 个未覆盖节点 | {{count}} more uncovered nodes |
| `residual.notice.over_registered` | 登记值大于盘上实际值，可能是统计口径不一致 | Registered bytes exceed on-disk bytes; the two measurements disagree |
| `residual.notice.empty` | 没有需要展示的余量节点 | No residual nodes to show |
| `residual.notice.scan_failed` | 触发扫描失败：{{error}} | Failed to start the scan: {{error}} |
| `residual.columns.name` | 名称 | Name |
| `residual.columns.class` | 类别 | Class |
| `residual.columns.scope` | 范围 | Scope |
| `residual.columns.scan` | 扫描 | Scanned |
| `residual.columns.declared` | 登记 | Registered |
| `residual.columns.residual` | 余量 | Residual |
| `residual.class.declared` | 已登记 | Registered |
| `residual.class.optionalNotMounted` | 可选能力未装配 | Optional capability not mounted |
| `residual.class.companion` | 伴生文件 | Companion files |
| `residual.class.ignored` | 已知忽略 | Known ignored |
| `residual.class.unregistered` | 未登记 | Unregistered |
| `residual.scope.data` | 数据目录 | Data |
| `residual.scope.local` | 本地目录 | Local |
| `residual.totals.unregistered` | 未登记合计：{{count}} 项，{{size}} | Unregistered total: {{count}} items, {{size}} |
| `residual.totals.hint` | 不计入页面任何合计 | Not included in any page total |
| `residual.children.more` | 另有 {{count}} 项未展开 | {{count}} more not shown |

### 11.3 既有键迁移（代码正在使用的键）

| 旧键（现被 `StorageSettings.tsx` 引用） | 新键 |
|---|---|
| `settings.storage.summary` | `attachments.ledger.total` |
| `settings.storage.orphan_group` | `attachments.orphan_group` |
| `settings.storage.delete_selected` | `attachments.delete_selected` |
| `settings.storage.confirm_delete.title` / `.content` | `attachments.confirm_delete.title` / `.content` |
| `settings.storage.deleted` | `attachments.deleted` |
| `settings.storage.empty` | `attachments.empty` |
| `settings.storage.load_failed` | `common.load_failed` |
| `settings.storage.refresh` | `toolbar.refresh` |
| `settings.storage.size_warning` | `attachments.size_warning` |

`settings.storage.title` 保留原位（I54 导航键，不在本次迁移范围）。迁移与组件改造**同一次提交**完成，不留「旧键没人用、新键还没接」的中间态。

### 11.4 草稿键处置（上一轮已写入、当前无代码引用）

这些键是旧设计的产物，按 §1 / §11.1 重排；**值可原样复用**，路径必须改到新树，改完后仓库内不得残留旧路径（含 en-US）。

| 草稿键 | 新位置 | 处置 |
|---|---|---|
| `bytes_unknown` / `count_value` | `common.bytes_unknown` / `common.count_value` | 迁 |
| `entries_title` / `entries_hint` / `entry_provided_by` | `entries.title` / `entries.hint` / `entries.provided_by` | 迁（hint 文案按 §11.2 补全「两条独立入口」一句） |
| `col_name` / `col_size` / `col_availability` / `col_eligibility` / `col_policy` / `col_action` | `entries.columns.*` | 迁 |
| `entry_names.{attachments, attachments_derived, tool_results, tool_index, mascot_assets, assistant_db, security_db, security_backups, security_vault, security_recovery, security_audit, logs, crashes}` | `entries.names.e1`..`e13` | 迁并改键名（原键名由 `key` 清洗而来，`key` 含 `/` 与 `.`，作为 i18n 路径段有歧义；改用条目 id） |
| `availability.*` / `eligibility.*` / `reason.*` / `action.*` | `entries.availability.*` / `.eligibility.*` / `.reason.*` / `.action.*` | 迁（值不变） |
| `policy.{source.*, retention, retention_disabled, keep_count, max_bytes, auto_active}` | `entries.policy.*` | 迁 |
| `reclaim.{busy, plan_failed, plan_expired, plan_not_found, not_allowed, confirmation_required, execute_failed}` | `reclaim.error.*` | 迁 |
| `reclaim.unknown` | — | **删**（与 `reclaim.error_code.unknown` 重复，错误码文案统一在 `error_code`） |
| `reclaim.result` / `result_partial` / `result_skipped` | `reclaim.result.done` / `.partial` / `.skipped` | 迁（补 `reclaim.result.failed`） |
| `reclaim.confirm_title` / `confirm_content` / `confirm_ok` / `confirm_cancel` | `reclaim.dialog.title` / `.body` / `reclaim.ok` / `reclaim.cancel` | 迁 |
| `reclaim.strong_title` / `strong_content` / `strong_placeholder` / `strong_mismatch` | `reclaim.strong.*` | 迁 |
| `reclaim.reclaimable` | `entries.facts.reclaimable` | 迁（补条数参数） |
| `inventory.title` / `hint` / `scan` / `scanning` / `scan_failed` | `residual.title` / `hint` / `scan` / `scanning` / `notice.scan_failed` | 迁 |
| `inventory.status.*` | `residual.status.*` | 迁（`stale` / `failed` 文案按 §11.2 调整括号位置） |
| `inventory.failed_keep_last` / `partial` / `truncated` / `over_registered` / `empty` | `residual.notice.*` | 迁（`truncated` → `notice.truncated_more`，参数由「节点列表」改为「数量」） |
| `inventory.unregistered_total` | `residual.totals.unregistered` | 迁（参数由拼接串改为 `{{count}}` + `{{size}}`） |
| `inventory.class.*` | `residual.class.*` | 迁（值不变） |
| `inventory.unknown_time` | `common.time_unknown` | 迁 |
| `inventory.scan_bytes` / `scan_count` / `declared_bytes` / `residual` / `declared_toggle` / `children_hint` | — | **删**（新设计用列头 + 单元格表达同一语义，不再拼行内标签） |

### 11.5 两份 locale 的对等性必须被测试

`I18nPath` 只由 `zh-CN` 推导，因此 **en-US 缺键不会被 `typecheck` 发现**（只会在运行时由 `missingKeyHandler` 打 warn）。故新增 `__tests__/localeParity.test.ts`：把两份 JSON 摊平成键路径集合，断言完全相等（含新增与迁移后无孤儿键）。

---

## 12 可访问性与文案规范

| 项 | 规则 |
|---|---|
| 展开 | 详情切换按钮 `aria-expanded` + `aria-controls`；组头折叠按钮同理 |
| 禁用 | 禁用按钮外层 `span` 承载 Tooltip；禁用原因同时在资格列可见 |
| 弹窗 | 强确认输入 `autoFocus` + `aria-describedby` 指向 `strong.body`；Esc 取消；回车仅在匹配时提交 |
| 焦点 | 弹窗关闭后焦点回到触发行按钮 |
| 键盘 | 行展开、组折叠支持 Enter / Space（用 `Button type="text"` 而非裸 `div` 点击） |
| 数字 | 字节用 `formatBytes`；`null` 用 `—`；时间统一「月 日 时:分」（沿用既有 `formatTimestamp`） |
| 语言 | `availabilityReason` / `note` / `error.message` 为后端原文，**不翻译、不改写**；枚举文案一律走词表 |

---

## 13 测试矩阵（无 DOM，全部吃纯函数）

| 文件 | 断言 |
|---|---|
| `__tests__/lexicon.test.ts` | 「枚举值有键」由 `satisfies Record<Enum, I18nPath>` 在**编译期**保证，测试只覆盖运行期语义：未知值（`"futureValue"`）返回原文而非空串；`constructor` / `toString` 等原型键不误命中；13 个条目 `key` 全部有名称、未知 `key` 回退原文；`entryNameText` 与 `availabilityText` 等对同一输入稳定 |
| `__tests__/entryView.test.ts` | `bytes = null` → `—` 且**不调用** `formatBytes`；`missing` + `0` → `0 B`；`count = null` → 无次行；四类不可回收 → 按钮禁用 + reason 文案；`notImplemented` → note 覆盖第二行；`ownerManaged` → 无按钮；`action = none` → `—`；`strongConfirm` → 标记强确认；`eligibility = eligible` 但 `action = none` → 不可点 |
| `__tests__/residualView.test.ts` | 排序（`unregistered` 优先、同类别按余量降序、`null` 置末、同值按名称稳定）；`declared` 仅在 `residualBytes > 0` 时出现；`partial` → 警示栈含 `partial` 且 `truncatedNodes` 最多列 3 个；任一 `overRegistered` → 警示栈含 `over_registered` 且该行带 `is-over`；`failed` 且保留数字 → 警示栈含 `failed_keep_last` 且 `nodes` 非空；全空 → `empty`；合计 `null` → `—` |
| `__tests__/reclaimFlow.test.ts` | `none` → 不经 `confirming` 直接 `executing`；`confirm` / `strongConfirm` → 进入 `confirming` 且强确认未匹配时 `canExecute = false`；`busy` / `planExpired` / `planNotFound` → 回到 `idle` 且**不**产生重试动作；`executing` 期间重复提交被忽略；`partial` 结果带 `failed` 计数与首个错误码文案 |
| `__tests__/localeParity.test.ts` | `zh-CN` 与 `en-US` 的键路径集合完全相等；`entries.names.e1..e13`、`entries.availability.*`、`entries.eligibility.*`、`residual.class.*`、`residual.status.*`、`reclaim.error.*`、`reclaim.result.*` 全部存在 |

不写的测试：组件渲染（无 DOM 环境）、CSS 断言、事件监听（`listen` 需 Tauri 运行时）。事件路径由 T4-b 手动回归覆盖（任务清单已列）。

---

## 14 门禁与自查（本设计自身的约束清单）

| # | 检查点 | 自查方式 |
|---|---|---|
| D1 | 前端不新建 store / service / 轮询 | `grep` 新增文件中的 `useState` 之外的持久化与 `setInterval` / `setTimeout` 轮询；service 只经 `storageService` |
| D2 | 不推算可回收量、不自行升降档 | 全量检索 `expectedBytes` 的比较运算（只允许格式化展示）；`confirmation` 只用于选弹窗 |
| D3 | 不做跨条目合计 | 检索条目表与余量段的求和/`reduce`；组头与合计行不含跨条目字节 |
| D4 | `null` 不直传 `formatBytes` | `formatBytes(` 的调用点必须包在 `bytesOrDash` 之后；检索 `formatBytes(` 出现位置 |
| D5 | 余量区无删除入口 | 检索 `ResidualSection` / `ResidualTable` 中 `Button` 仅 1 处（扫描） |
| D6 | 条目表无附件删除入口 | 检索 `EntrySection` 子树中不出现 `deleteAttachments` / `AttachmentDeleteResultDto` |
| D7 | 枚举全覆盖 | `lexicon.test.ts` + `localeParity.test.ts` 通过 |
| D8 | 事件只重拉一次 | `useStorageSnapshot` 单飞；`listen` 回调内只有一次 `load("refresh")` |
| D9 | 既有附件能力零删除 | 段一 diff 只含 §4.2 的 5 条追加 |
| D10 | 文案无本机绝对路径、无硬编码语言 | 检索 `C:/`、`/Users/`、中文字面量出现在 `.tsx`（应全部来自 `t()`） |
| D11 | 拆分判据 | 每个文件对应单一变化原因；组件不导入 service；纯函数模块不导入 React |
| D12 | 页面路径不变 | `StorageSettings.tsx` 仍存在且导出默认组件（路由/注册未改） |

---

## 15 落地顺序（对应 T4-a）

| 步 | 产物 | 完成判据 |
|---|---|---|
| 1 | `lexicon.ts` + `format.ts` + 两份 locale 键树（§11） | `localeParity` / `lexicon` 测试绿 |
| 2 | `entryColumns.ts` + `entryView.ts` + `entryView.test.ts` | 占位符与按钮态断言全绿 |
| 3 | `residualView.ts` + `residualView.test.ts` | 排序 / 警示栈 / 过滤断言全绿 |
| 4 | `reclaimFlow.ts` + `reclaimFlow.test.ts` | 状态机与错误码断言全绿 |
| 5 | `useStorageSnapshot.ts`（含事件与单飞） | 页面仍只显示段一，行为与现状一致 |
| 6 | `AttachmentSection.tsx` 迁入 + 薄壳改造 | §4.1 九项能力逐项回归 |
| 7 | `EntrySection` / `EntryTable` / `EntryDetail` + `useReclaim` + `ReclaimDialog` | 13 行全部在场、顺序与登记表一致 |
| 8 | `ResidualSection` / `ResidualTable` | 五态可手动触发（`neverRun` → `running` → `fresh`；失败态靠断网或注入错误） |
| 9 | `settings.css` 追加（§10） | 视觉无回归、窄宽横向滚动 |
| 10 | 全量门禁 | `npm run typecheck` / `npm run lint` / `npm run test` + §14 逐条 |

---

## 16 与规格的对应索引

| 规格条目 | 本文落点 |
|---|---|
| §3.1 13 条条目 | §5.1 / §5.5 / §5.2（id 映射） |
| §3.3 资格 | §5.6 / §5.7 |
| §3.5 动作 | §5.7 |
| §3.6 确认档位 | §6.2（档位只读、不改强度） |
| §3.7 策略 | §5.6 / §5.9（chip + 详情） |
| §3.11 对账余量 | §7 全段 |
| §6.1 命令契约 | §6.1 / §7.1（计划与扫描均不本地缓存） |
| §6.2 DTO | §5.1（只消费列出的字段）/ §11.5（类型源） |
| §6.3 第 1 条 不新建 store/service | §0 / §8.1 / D1 |
| §6.3 第 2–3 条 展示真实观测、`null` 占位 | §5.3 / D4 |
| §6.3 第 4 条 保留附件能力 | §4.1 / D9 |
| §6.3 第 5–6 条 不推算、不降档 | §0 / §5.7 / §6.2 / D2 |
| §6.3 第 7 条 无第二套删除入口 | §3.3 / D6 |
| §6.3 第 8 条 失败提示重试不重放 | §6.3 |
| §6.3 第 9 条 余量区只读 / 五态 / 折叠 / 警示 | §7 全段 / D5 |
| §6.3 第 10 条 余量不进合计 | §7.4 合计行 hint / D3 |
| §7 失败降级 | §6.3 / §8.3 / §7.3 |
| §10 V15–V20 | §12（文案与留痕）/ §6 / §7 |
| §11 切片 | §15 |
# I33 保险库首页 UI 审计与设置入口语义整合

| 项 | 内容 |
| --- | --- |
| 文档状态 | 已实施（阶段一至六代码完成，待人工验收；问题与决策已合并为单表） |
| 审计基线 | 主干 `d728ed0`（2026-11-02） |
| 适用范围 | 保险库首页（`/vault`）及承载它的主窗口外壳（`MainLayout` / `Titlebar` / `AppSidebar` / `Layout.Content`）布局、页头按钮、列表与状态栏、筛选与窄屏结构、设置入口语义 |
| 依据 | 对当前主干前端与相关 Rust 命令实现的逐文件阅读，非历史存档 |
| 上游规格 | `docs/store/I32-vault-git-sync-spec.md`（同步状态语义与文案，已实施） |
| 前端入口 | 外壳：`src/layouts/MainLayout.tsx`、`src/components/Titlebar.tsx`、`src/components/AppSidebar.tsx`；页面：`src/vault/VaultApp.tsx`、`src/vault/VaultList.tsx`、`src/vault/VaultStatusBar.tsx`、`src/vault/detail/VaultEntryDetail.tsx`、`src/vault/settings/VaultSettings.tsx`、`src/features/vault/module.ts` |
| 状态层入口 | `src/vault/core/slices/*`、`src/vault/core/syncDisplay.ts`、`src/vault/core/useVaultAutoLock.ts`、`src/vault/core/useVaultEvents.ts` |
| 后端证据 | `src-tauri/src/cmd/vault/entries.rs`、`src-tauri/src/cmd/vault/audit.rs`、`src-tauri/crates/assistant-vault/src/security_db/entries/query.rs` |
| 样式入口 | `src/vault/vault.module.css`、`src/vault/core/vaultColors.ts` |
| 不在本文件范围 | 后端同步引擎语义（见 I32）、加密格式、条目表单字段、冲突解决算法、自动 HIBP 审计的产品设计（见 F12） |

---

## 1. 审计范围与方法

本文件回答两个问题：

1. 保险库首页当前存在哪些**用户可见**的问题（功能误导、交互死路、性能、视觉、可访问性）。
2. 页内「工具」抽屉与全局「密码保险库设置」页**语义重复**到什么程度、应当如何划分。

方法：只读代码，逐条核对前端组件、状态层与相关 Rust 命令；每个问题给出「代码证据」与「用户影响」，并直接给出决策与可验证的验收断言。视觉类结论以 CSS 与组件结构为准，标注为「需实测」的条目必须在实施时用真实窗口尺寸复核。

证据书写约定：优先写文件 + 符号名，行号仅作定位参考，代码变动后以符号为准。

分级约定：

| 级别 | 含义 |
| --- | --- |
| P0 | 功能误导或交互死路，用户会做错事或无法退出某状态 |
| P1 | 高频体验缺陷，首屏即可感知 |
| P2 | 结构与视觉问题，影响观感、空间利用率或反馈真实性 |
| P3 | 可访问性、文案、可维护性 |

编号约定：沿用 F（功能/状态）、V（视觉/布局）、A（可访问性/可维护性）、H（页头专项）；本轮复核新增的问题按系列追加，不重排既有编号。

---

## 2. 当前结构事实

### 2.1 页面骨架

```
App（按 webview label 选择视图；主窗口为 MainLayout）
└── MainLayout
    ├── Layout（height:100vh，overflow:hidden）
    │   ├── Titlebar（高 38px；右侧控件 paddingRight:4）
    │   └── Layout（横向，flex:1，minHeight:0）
    │       ├── AppSidebar（宽 240px，可折叠为 64px）
    │       └── Layout.Content（overflow:auto）
    │           └── FeatureErrorBoundary
    │               └── /vault（features/vault/module.ts 懒加载）
    │                   └── VaultApp
    │                       ├── appHeader：标题 + 「工具」按钮 + 锁定图标按钮
    │                       └── appBody（lg 以上双栏 / 以下单栏）
    │                           ├── listPane
    │                           │   ├── VaultList（横幅 ×2 + 搜索行 + 分类行 + 条目列表）
    │                           │   └── VaultStatusBar（条目数 / 泄露数 / 同步态 / 上次同步）
    │                           └── detailPane（VaultEntryDetail 或空态）
    └── CommandPalette（全局浮层）
```

关键尺寸与断点：`Titlebar` 高 38px；`AppSidebar` 宽 240px、折叠 64px；`Layout.Content` 为 `overflow:auto`，其宽度 = 窗口宽度 − 侧栏宽度，高度 = `100vh − 38px`。`VaultApp` 内部 `appBody[data-wide]` 为 `360px + 1fr`，带 `padding:16px` 与 `gap:16px`；`isWide = screens.lg`（窗口宽度 ≥992px，基于 `Grid.useBreakpoint` 的视口宽度，**不感知侧栏宽度**）。`listPane[data-wide]` 自带 `padding-right:16px` 与右边框，`detailPane[data-wide]` 自带 `padding-left:16px`。

由此可得：侧栏展开时，992px 窗口的内容区约 752px；双栏扣除 `padding` 32、列表 360、`gap` 16 与详情左内边距 16 后，详情可用宽度约 328px。这是 V2 / F9 判定「窄屏」时必须纳入的约束。

### 2.2 设置面矩阵（三处入口）

| 入口 | 载体 | 内容 | 未解锁时可用 | 数据通道 |
| --- | --- | --- | --- | --- |
| 标题栏齿轮 | 主窗口 `Layout.Content` 内 `/settings/overview` → `/settings/vault`（`SettingsContent`） | `vault-general`：自动锁定、锁屏锁定、剪贴板清除、加密算法、生物识别、HIBP 自动审计；`vault-ssh`：自动添加、Pageant、OpenSSH Agent、AUTH_SOCK | 是（未初始化时显示「去创建保险库」引导） | `settingsStore` 的 `VaultConf` |
| 页头「工具」按钮 | 页内 `Drawer`（`VaultSettings`，默认页签 `sync`） | 同步与远程、SSH Agent 密钥列表、恢复密钥、修改主密码、导入导出、危险操作 | 否（需已解锁） | 混合：`syncConfig`（后端）、`VaultConf`（`vaultSyncSshHosts`、`vaultSshAutoAdd`）、纯动作 |
| 侧栏「密码保险库」 | `/vault` 路由 | 首页本体 | 否（先解锁） | — |

注意：`vaultHibpAutoAudit` 目前没有任何消费点（见 F12），在上述矩阵中不能视为「有效的持久化偏好」。

补充：`src/layouts/SettingsWindow.tsx`（webview label `settings`）另有独立设置窗口实现，Rust 侧注册了 `open_settings` 命令，但当前前端没有调用入口；用户可见的设置页面均在主窗口内，因此本文件按主窗口内的设置路由分析。

结论：第二、三处不是「重复的设置界面」，而是**同一个抽屉同时承担了「配置」与「操作」两种职责**，并且在 SSH 主题上与第一处真实重叠。详见 §4。

---

## 3. 问题与决策

### 3.1 P0：功能误导与交互死路

| 编号 | 问题 | 代码证据 | 用户影响 | 决策 | 验收断言 |
| --- | --- | --- | --- | --- | --- |
| F1 | 条目增删改后不刷新同步状态 | `vaultEntriesSlice.ts` 的 `createEntry` / `updateEntry` / `setFavorite` / `deleteEntry` / `restoreEntry` 成功后只调 `loadEntries`；对应 Rust 命令均调用 `finalize_sync`（`src-tauri/src/cmd/vault/entries.rs`） | 本地 `ahead` 已变化，状态栏仍显示旧待推送数或「已与远程一致」，被判断为同步坏了 | 变更成功后追加 `loadSyncReadyness()` 与 `loadRemoteStatus(false)`，不阻塞条目操作的主流程 | 新增 / 编辑 / 删除 / 恢复 / 收藏后 `syncReadyness` 与 `remoteStatus` 至少各刷新一次；`ahead` 与后端 `sync_readyness` 返回值一致 |
| F2 | 条目计数被筛选污染，存在三层口径 | `VaultApp.tsx` 向 `VaultStatusBar` 传 `entries.length`；`entries` 是 `listEntries(category, search, tag)` 的服务端结果；`VaultList` 再按 `onlyLeaked` 客户端过滤为 `visibleEntries`；`list_entries` 只返回 `Vec`，无总数（`src-tauri/crates/assistant-vault/src/security_db/entries/query.rs`） | 搜索时底部数字被当作库存总数；开启「仅泄露」后状态栏数字与列表行数不一致 | 定义三级口径：总数（后端 COUNT）、服务端筛选数、客户端可见数；状态栏未筛选时只显示总数，筛选时显示「可见 / 总数」 | 空筛选时「N 个条目」等于数据库未删除条目总数；搜索 / 分类 / 标签 / 泄露筛选时为「M / N」；`onlyLeaked` 开启时 M 与列表行数一致 |
| F3 | 泄露映射不随删除清理，且跨锁定残留 | `deleteEntry` 只过滤 `entries` 与 `selectedEntry`；`hibpLeaks` 仅在 `hibpCheck` / `hibpAudit` 写入；`vault_hibp_audit` 只返回 `match_count > 0` 的条目并整体替换 map（`src-tauri/src/cmd/vault/audit.rs`）；`lock()` 与 `vault:locked` 均不清空 `hibpLeaks`（`vaultLifecycleSlice.ts`、`useVaultEvents.ts`） | 删除泄露条目后状态栏与筛选 chip 仍统计旧泄露；重新解锁后仍显示上次会话的泄露标记 | `deleteEntry` 成功后就地移除该 id；`purgeDeleted` 后按现存条目重建 map；`lock()` 与 `vault:locked` 清空 `hibpLeaks` | 删除唯一泄露条目后泄露数立即为 0、chip 消失；锁定再解锁后 `hibpLeaks` 为空，直到重新审计或单条检查 |
| F4 | 「仅泄露」筛选可进入无法退出状态 | chip 仅在 `leakCount > 0` 时渲染（`VaultList.tsx`）；`onlyLeaked` 为组件本地 state，除 chip 外无复位路径；审计无泄露时后端返回空数组，前端把 `hibpLeaks` 置为 `{}` | 列表空白且没有任何入口关闭筛选 | 阶段二先加保护：`leakCount` 变为 0 时自动复位 `onlyLeaked`；阶段五再补「当前筛选」条，使泄露条件不再依赖 chip 是否渲染即可见、可清除 | 审计返回 0 条时列表不空白；任意非默认筛选都能在筛选条中看到并一键清除 |
| F5 | 标签筛选没有显式清除，且锁定后残留 | 唯一取消路径是再次点击条目内同名 Tag（`VaultList.tsx`）；`tagFilter` 位于 store，`lock()` 不重置；分类栏无标签态展示 | 用户不知道列表为什么变短；重新解锁后仍然变短且无提示 | 与 F4 共用「当前筛选」区，展示搜索 / 分类 / 标签 / 泄露条件并带清除；`lock()` 时重置 `searchQuery`、`categoryFilter`、`tagFilter` | 标签筛选时筛选条显示「标签: xxx」且可清除；`lock()` 后三个筛选字段为空 |
| F6 | 状态优先级让「冲突」被「失败」覆盖，且 `syncError` 混用两类失败 | `deriveSyncDisplay` 先判 `syncError`（`src/vault/core/syncDisplay.ts`）；`failSync` 与 `resolveConflict` 失败都会写入 `syncError`（`vaultSyncSlice.ts`）；`syncError` 只在下次同步开始 / 成功或手动 `clearSyncError` 时清除；`syncDisplay.test.ts` 目前明确断言 `syncError` 优先 | 一次失败后，真实 `repoState`（冲突）被渲染成「同步失败」，分类错误 | 定义新优先级：`repoState`（对象）优先于 `syncError`，`syncError` 优先于其余 `readyness`；只让「同步失败」进入状态栏，「操作失败」走独立提示；同步修改单测与文案 | `readyness = { repoState: "merge" }` 且 `syncError` 非空时显示冲突态；`syncError` 仅在无 `repoState` 时显示失败；单测覆盖两种组合 |
| F7 | 「待确认远端删除」与同步按钮不联动 | 同步按钮 `danger={repoState !== null}`，未读取 `pendingRemovals`；`onClick` 直接调用 `syncNow()`（`VaultList.tsx`） | 有未确认删除时仍可直接同步，UI 无「先确认删除」的引导 | `pendingRemovals !== null` 时同步按钮改为引导至确认弹窗（或禁用并用 Tooltip 说明），确认后再同步；验收前先核对后端在待确认状态下的实际同步行为 | 存在待确认删除时点击同步不直接发起推送，而是打开确认弹窗；确认后 `pendingRemovals` 清空 |
| F12 | `vaultHibpAutoAudit` 是无效开关 | 该键只出现在 `src/features/vault/module.ts`、`src/features/vault/conf.ts`、Rust `assistant-config` 的 `vault_conf` 与 settings golden fixture；`src/` 下无消费点，`vault:unlocked` 事件也不触发审计（`useVaultEvents.ts`） | 用户开启「HIBP 密码泄露审计」后什么也不会发生，属功能误导 | 本轮移除该设置项及对应 i18n；`VaultConf` 字段暂留以兼容旧配置；自动审计另立需求（需进度、取消与频率控制，参考 F16） | 设置页、i18n 与 golden 快照中不再出现该开关；`vault-general` 其余项不受影响 |

### 3.2 P1：高频体验缺陷

| 编号 | 问题 | 代码证据 | 用户影响 | 决策 | 验收断言 |
| --- | --- | --- | --- | --- | --- |
| F8 | 鼠标移动触发整页重渲染 | `useVaultAutoLock` 对 `mousemove` / `keydown` 无节流调用 `resetAutoLockTimer`；该函数每次 `clearTimeout` + `setTimeout` + `set({ autoLockTimerId })`（`vaultLifecycleSlice.ts`）；全仓 14 处 `useVaultStore()` 整仓订阅：`VaultApp` / `VaultList` / `VaultStatusBar` / `VaultEntryDetail` / `VaultEntryDrawer` / `TrashDrawer` / `VaultSettings` / `VaultSyncSettings` / `VaultConflictResolve` / `DangerZoneTab` / `ChangePasswordTab` / `VaultEntryHistory` / `useVaultUnlock` / `BiometricSwitchRender` | 鼠标移动即重渲染列表内全部条目与已挂载的抽屉内容 | 定时器移出 React store（模块级变量），只暴露 `autoLockDeadline` 供 UI 使用；事件按 300–500ms 节流；组件改逐字段 selector，取多字段时用 `useShallow`，避免对象字面量导致无限重渲染 | 连续移动鼠标 5 秒，`VaultList` 与 `VaultEntryDetail` 不重渲染（Profiler 或临时渲染计数验证）；自动锁定仍按配置生效 |
| F9 | 窄屏进详情会卸载整个列表面板 | 渲染条件为 `isWide \|\| !isDetail`，`isWide` 按窗口宽度而非内容区宽度判定（`VaultApp.tsx`，见 V2）；`initialLoadDone` ref、`sortBy`、`onlyLeaked`、`trashOpen` 均为 `VaultList` 本地状态；重挂载后 effect 重跑 `loadEntries`、`loadSyncReadyness`（可能连带 `loadSyncConflicts`）、`loadPendingRemovals`、`loadSyncLastTime`，至少 4 条 IPC | 状态栏在窄屏详情页完全消失；返回时列表闪 Spin、排序 / 筛选 / 回收站状态丢失 | 「窄屏」判定随 V2 改为内容区宽度；窄屏改为「列表常驻 + 详情覆盖层」，覆盖层关闭仅隐藏，不卸载 `VaultList` 与 `VaultStatusBar` | 窄屏进入 / 返回详情时列表无 Spin、排序与筛选保持、状态栏始终可见；返回时不新增同步类 IPC；侧栏折叠/展开不影响该判定 |
| F10 | 选中条目重复解密 | `handleSelect` 已调 `selectEntry(id)`（`VaultApp.tsx`），`VaultEntryDetail` 在 `entryReady` 为 false 时再次调用；`selectSeq` 使前一次结果被丢弃（`vaultEntriesSlice.ts`、`VaultEntryDetail.tsx`） | 每次点开条目多一次解密 IPC，且首次结果无效 | `selectEntry` 增加 in-flight 标记（同 id 进行中则跳过）；详情仅在 store 无该 id 且无在途请求时发起 | 单次点击详情只产生 1 次 `get_entry` IPC；快速切换条目时最终详情与选中项一致 |
| F11 | 页头「工具 / 锁定」语义与权重错误 | 见 §3.5（H1–H5） | 锁定是高频终止动作却最弱；入口命名与实际内容不符 | 见 §3.5 各行 | 见 §3.5 各行验收断言；阶段四实施 |
| F13 | 删除失败静默，详情仍关闭 | `deleteEntry` 的 catch 只 `set(error)`，不抛出（`vaultEntriesSlice.ts`）；`VaultList.handleDelete` 用 `.then(onDeleted)`，`VaultEntryDetail.handleDelete` 在调用后立即 `onClose()` | 删除失败时无提示，详情已关闭，用户以为已删除 | `deleteEntry` 失败时 reject（或返回成功标志）；调用方只在成功回调里关闭详情 / 清选中，并提示错误 | 注入删除失败后详情保持打开、列表条目仍在、出现错误提示；成功路径行为不变 |
| F17 | 初始化 / 解锁在主线程执行，阻塞 UI | `vault_init` / `vault_unlock` / `vault_lock` 为同步 `fn`（`src-tauri/src/cmd/vault/lifecycle.rs`）；Tauri 同步命令在主线程执行，KDF 与数据库打开期间界面无法响应 | 解锁时窗口卡住、输入框与按钮无响应，用户以为程序假死 | 改为 `async fn` + `tauri::async_runtime::spawn_blocking`，把 KDF、数据库打开与 SSH 自动添加放到阻塞线程；前端保持按钮 loading | 解锁 / 初始化期间窗口可拖动、输入可响应；命令返回前 loading 保持，失败后恢复 |

### 3.3 P2：布局、视觉与反馈真实性

| 编号 | 问题 | 代码证据 | 影响 | 决策 | 验收断言 |
| --- | --- | --- | --- | --- | --- |
| V1 | 双栏缝宽约 48px 且分割线不居中 | `appBody[data-wide]` 用 `gap:16px`，`listPane[data-wide]` 再补 `padding-right:16px` + 右边框，`detailPane[data-wide]` 再补 `padding-left:16px`（`vault.module.css`） | 两栏内容各自缩进，分割线偏左，像「假双栏」 | 去掉 `listPane` 的 `padding-right` 与 `detailPane` 的 `padding-left`；分割线只由 `gap` + 单一边框承担 | 两栏内容到分割线的距离相等；`gap` 与边框的归属在 CSS 中只有一处定义 |
| V2 | 断点按窗口宽度而非内容区宽度 | `isWide = screens.lg`（窗口宽度 ≥992px）；`Grid.useBreakpoint` 基于 matchMedia 视口宽度，不感知 `AppSidebar` 的 240px / 64px；侧栏展开时 992px 窗口的内容区仅约 752px（`vault.module.css`、`VaultApp.tsx`） | 双栏在内容区不足时提前触发，详情可用宽度约 328px；900–990px 又被判为单栏，列表与详情需往返切换 | 改为按内容区宽度判定（阈值 784px）：在 `VaultApp` 上用 `ResizeObserver` 监听容器宽度（本轮实现）；或给路由内容外层加 `container-type: inline-size` 用容器查询，或把侧栏折叠状态上移到共享 store 后按「窗口宽度 − 侧栏宽度」计算。不再简单把断点从 `lg` 降到 `md` | 侧栏展开 1024px 窗口双栏且详情内容宽 ≥360px；侧栏折叠 848px 窗口仍可双栏；侧栏展开 768px 窗口保持单栏；折叠/展开切换后布局即时正确，无需缩放窗口 |
| V3 | 竖向空间被五层占用 | 可用高度为 `100vh − 38px`（`Titlebar`）；`appHeader` + `appBody padding:16px` + 搜索行 + 分类行 + `syncBar` 占五层，`listPane` 仍为 `grid-template-rows: 1fr auto` | 窄窗可视条目不足 3 行（需实测） | 压缩搜索行与操作行间距、收紧分类栏（见 V4）；在保持 32px 点击区的前提下减少纵向留白 | 1366×768 与 1280×720 下列表可视条目行数 ≥3；高度计算含 `Titlebar` 38px |
| V4 | 分类标签在窄列下易换行 | 6 分类 + 「全部」+ 泄露 chip 共 8 个 `Tag.CheckableTag`；`categoryOptions` 为 `flex-wrap: wrap`；列表列固定 360px | 分类栏折行进一步压缩列表（临界宽度需实测） | 分类栏改单行横向滚动（不换行）；实测确认泄露 chip 可见 | 360px 列宽下分类栏始终单行，可横向滚动到泄露 chip |
| V5 | 横幅插在列表流顶部 | 两条 `Alert` 带 `marginBottom:12` 置于 `listContainer` 顶部 | 挤压正文，且与状态栏同义信息分散在两处 | 抽出 `VaultAlerts`：按严重度排序、默认最多展示 1 条 + 「还有 N 条」折叠；与状态栏告警合并 | 同时存在待删除与冲突告警时只展示优先级最高的一条；展开后可见其余条目 |
| V6 | 条目行操作区不可收缩 | `entryActions` 为 `min-width:118px`，子元素 `flex-shrink:0`，实际宽度约 137px；`entryTags` 也为 `flex-shrink:0` | 窄列时 username / url / tags 集体截断 | 操作改为 hover 浮层或「复制密码 + 更多」；`entryTags` 允许收缩或省略 | 360px 列宽下标题与 username 至少保留可辨识长度；操作在 hover / focus 时可用键盘触达 |
| V7 | 同步态区分度不足 | 11 种展示态实际使用 5 个色彩 token（error 复用 2 处），有效色值 4 个：`ahead / behind / dirty` 同色 `syncDirty`，`noSync / noRemote` 同色 `syncOff` | 颜色不承担区分职责，只能读 12px 灰字 | 为 `ahead` / `behind` 引入浅色变体，`noSync` 与 `noRemote` 分开；颜色之外补图标或形状，避免仅靠颜色区分 | 浅色与深色主题下，`ahead`、`behind`、`dirty`、`noSync`、`noRemote`、`failed` 六态可区分；状态不只依赖颜色 |
| V8 | 页头与 titlebar 右边缘错位 | `Titlebar` 为 `paddingRight:4`，`appHeader` 为 `padding:8px 16px` | 同一竖线上两排按钮右边缘不对齐 | 页头右侧内边距按 titlebar 控件区实际宽度校准；左右两侧统一使用同一间距变量 | 页头按钮右边缘与 titlebar 按钮右边缘对齐（多窗口宽度下复核） |
| V9 | 标题基线与按钮不一致 | 页头标题用 `Typography.Text strong`（15px），按钮 32px 高，靠 `align-items:center` 居中 | 标题视觉偏上，与按钮文字基线不齐 | 标题改 `Typography.Title level={5}` + `flex:1`，与 `HistoryDetail` 页头模式一致 | 标题文字基线与按钮文字基线视觉齐平 |
| V10 | 空态不分场景 | `VaultList` 空态只有「暂无条目」一种文案，但已带「新建」按钮；真空库与筛选无结果共用同一分支 | 新用户缺少导入 / 克隆引导；搜索无结果时误导为「库里没东西」 | 区分真空库（导入 / 克隆远程 / 新建引导）与筛选无结果（只显示「清除筛选」）；引导动作跳转管理页对应区域 | 空库显示三种引导；搜索无结果不显示导入 / 克隆，只显示清除筛选 |
| V11 | 设置抽屉体量过大 | antd `Drawer size="large"` 固定 736px，仅在窗口小于约 800px 时接近全宽 | 次要入口的视觉体量超过主界面 | 改 `size="default"` 或 `width: min(480px, 100vw)` | 1440px 窗口下抽屉宽度 ≤480px；窄窗口下不超出视口 |
| F14 | 已锁定状态仍在布防自动锁定 | `useVaultAutoLock` 不判断 `isLocked`；`VaultApp` 锁定时渲染 `VaultUnlock` 但 Hook 仍挂载；`resetAutoLockTimer` 会重新 `setTimeout`，到点再次调用 `lock()` | 锁定页移动鼠标会反复布置定时器，长时间停留会重复调用 `lock` IPC | 事件处理内先读 `isLocked`，为 true 直接返回；`lock()` 与 `vault:locked` 清空 deadline | 锁定状态移动 / 按键不产生新定时器；解锁后按配置生效 |
| F15 | 状态栏泄露提示是假可点击 | `.leakWarning` 定义 `cursor:pointer` 与 hover 下划线，但 JSX 为纯 `span` 无 `onClick`（`VaultStatusBar.tsx`、`vault.module.css`） | 看起来可点击却没有反应 | 点击进入「仅泄露」筛选（与 F4 / F5 状态联动）；若不做交互则移除 pointer 与 hover 样式 | 点击泄露提示能切换筛选；不可点击时样式不再暗示可点 |
| F16 | 全量 HIBP 审计时长不可感知 | `vault_hibp_audit` 每条至少 1500ms 间隔且并发信号量为 1（`src-tauri/src/cmd/vault/audit.rs`）；前端只有按钮 loading（`VaultList.tsx`） | 条目多时审计耗时分钟级，无进度 / 取消，用户以为卡死 | 后端逐条发出 `vault:hibp-progress` 事件并新增 `vault_hibp_audit_cancel`；前端显示「已完成 / 总数」与取消按钮；取消后未开始的任务不再发起请求 | 审计 20 条时显示进度；取消后不再发起新请求，`hibpAuditing` 正确复位 |

### 3.4 P3：可访问性与可维护性

| 编号 | 问题 | 代码证据 | 影响 | 决策 | 验收断言 |
| --- | --- | --- | --- | --- | --- |
| A1 | 条目行不可键盘操作 | `entryItem` 为可点击 `div`，无 `role` / `tabIndex` / 键盘处理（`VaultList.tsx`） | 键盘与读屏用户无法选中条目 | 加 `role="button"`、`tabIndex=0`、Enter / Space 处理；若维持单选语义可用 `listbox` / `option` | 键盘可选中条目并有可见 focus 样式；读屏能读出条目名与分类 |
| A2 | 纯图标按钮无 `aria-label` | 页头锁定、HIBP 审计、同步、回收站按钮只有 `Tooltip` | 读屏用户拿不到可访问名称 | 补 `aria-label`，文案复用对应 Tooltip 的 i18n key | 无障碍树中按钮有名称，且不重复朗读 tooltip |
| A3 | 同步态按钮重复实现样式 | `syncFailed` 与 `noRemote` 均为裸 `<button>`；`noRemote` 内联重置 border / background / padding / font / cursor 共 5 项（`VaultStatusBar.tsx`） | 与 `.syncState` 重复，维护成本高 | 统一 `syncStateButton` 类，删除内联 reset | 两个按钮共用同一类，无内联样式 reset |
| A4 | `VaultList.tsx` 职责过多 | 单文件约 470 行，含列表、两条横幅、搜索 / 分类 / 排序、冲突选择 Modal、待删除确认 Modal、回收站入口、泄露筛选 | 后续改样式与逻辑会在同一文件反复冲突 | 拆分为 `VaultToolbar` / `VaultActiveFilters` / `VaultCategoryBar` / `VaultEntryRow` / `VaultAlerts`（横幅与折叠提示）；`VaultList` 只保留数据接线与 Modal | 各子组件单文件职责单一；`VaultList` 只负责组合与数据接线 |
| A5 | 页头复用疑问句 key | `t("vault.confirm_lock")`（「确定锁定保险库？」）同时用于旧 Tooltip 与确认框标题（`VaultApp.tsx`） | hover 弹出问句，语义错误 | 新增 `vault.lock`（「锁定保险库」）专供锁定按钮文字与 `aria-label`；`vault.confirm_lock` 仅用于确认框 | 按钮文字为陈述句，确认框仍为疑问句；两种语言文件同时更新 |
| A6 | 自动锁定无倒计时提示 | store 只保存 `autoLockTimerId`；`resetAutoLockTimer` 每次重建 timer；页头无剩余时间 | 密码管理器缺失一类重要的安全感反馈 | 新增 `autoLockDeadline` 时间戳，`resetAutoLockTimer` 同时写入，`lock()` 与 `vault:locked` 清空；页头按相对时间渲染，配置变更时重算 | 页头显示距锁定剩余时间并随时间递减；锁定后清除，重新解锁后重新计时 |

### 3.5 页头专项（「工具」与「锁定」）

| 编号 | 问题 | 代码证据 | 说明 | 决策 | 验收断言 |
| --- | --- | --- | --- | --- | --- |
| H1 | 权重与语义倒置 | 原「工具」为带文字按钮；「锁定」为纯图标按钮（`VaultApp.tsx` 页头 `Space`） | 纯图标锁定不显眼，用户无法一眼识别 | 「管理」保持默认按钮（`SettingOutlined` + 文字）；「锁定」也使用默认按钮（`LockOutlined` + 「锁定」文字），依靠图标与文字双重识别，不用降低权重来区分 | 锁定按钮同时显示锁图标与「锁定」文字；页头两个按钮均可一眼识别用途 |
| H2 | 命名错位 | 按钮文案与抽屉标题均为 `vault.tab.tools`（「工具」），打开的是 `VaultSettings` | 内容实际是「保险库管理」，不是「工具」 | 新增 `vault.tab.manage`（「管理」/ "Manage"），按钮与抽屉标题共用；删除 `vault.tab.tools` 引用 | 页头与抽屉标题均显示「管理」；全仓检索 `vault.tab.tools` 无结果 |
| H3 | 锁定二次确认且标记 danger | `handleLock` 使用 `modal.confirm` + `okType:"danger"` | 锁定可逆、低风险，红色确认按钮与删除类操作混淆 | 保留一次确认但去掉 `okType:"danger"`，确认按钮使用普通主按钮 | 锁定确认框无红色按钮，文案与删除类确认框可区分 |
| H4 | 锁定无 loading / 防重复 | `handleLock` 中 `onOk` 返回 void，立即关闭确认框；`lock()` 异步且失败只记录日志 | 确认框在锁定完成前关闭，失败无反馈 | `onOk` 返回 `lock()` 的 Promise，使确认按钮显示 loading 并防重入；失败时提示错误 | 锁定期间确认按钮 loading；重复确认不产生多次 `lock` IPC；失败有提示 |
| H5 | 窄屏页头变三层标题 | 窄屏详情时页头标题为「查看条目」，内容区 `detailHeader` 又有返回箭头 + 分类图标 + 条目名，上方还有 titlebar | 用户此刻意图是「返回」，页头提供的却是「锁定」 | 窄屏详情页头标题保持「保险库」，条目标题只由 `detailHeader` 承担 | 窄屏详情页头不出现「查看条目」；返回入口只在 `detailHeader` |

---

## 4. 设置入口语义重复：专项分析与处理方案

### 4.1 重复的精确边界

先区分三类内容，避免把「动作」误判为「设置」：

| 内容类型 | 例子 | 天然归属 |
| --- | --- | --- |
| 持久化偏好（conf） | 自动锁定分钟、加密算法、SSH 自动添加 | 全局设置页 |
| 仓库绑定配置（需要解锁 + 需要展示同步态） | 远程地址、初始化 / 克隆、SSH 复用主机 | 页内面板（与同步态同屏） |
| 动作（一次性或运行时） | 拉取、推送、强制推送、冲突解决、恢复密钥生成、改主密码、导入导出、清空回收站、SSH Agent 密钥列表 | 页内面板 |

注意：`vaultHibpAutoAudit` 目前无消费点（F12），在移除前不能作为「有效偏好」计入本轮归属调整。

据此，**真实重复项只有两条**：

| 重复项 | 全局设置页位置 | 页内抽屉位置 | 重复性质 |
| --- | --- | --- | --- |
| `vaultSshAutoAdd` | `src/features/vault/module.ts` 的 `VAULT_SSH_SETTINGS` 定义开关 | `src/vault/settings/SshAgentTab.tsx` 手写 `Switch` + `dispatchConfPatch` | **同一 conf 键两套 UI、两个入口**，且两处样式与保存反馈不一致 |
| 「SSH Agent」这一主题 | 分组标题 `settings.vault.ssh.title` =「SSH Agent」，内容为 3–4 个开关 | 页签名 `vault.tab.ssh_agent` =「SSH Agent」，内容为密钥列表与运行时状态 | **同名不同内容**，用户无法判断该去哪一处 |

次要不一致（不是重复，但应记录）：

| 项 | 事实 | 风险 |
| --- | --- | --- |
| 同步配置双通道 | `remoteUrl` 走后端 `syncConfig`；`vaultSyncSshHosts` 走 `VaultConf`（`dispatchConfPatch`），但两者在同一张表单内相邻（`VaultSyncSettings.tsx`） | 保存时机、失败反馈、离线可用性不一致；后续若搬移配置需分别处理 |
| 抽屉 `sync` 页签混合职责 | 同一页签内既有远程绑定配置，又有拉取 / 推送 / 克隆 / 冲突入口 | 属于「配置 + 动作」混合，是本轮语义混淆的结构根源 |

**本轮结论**：双通道保留现状，登记为技术债。原因：`syncConfig` 由后端维护远程地址与 Git 状态，迁入 `VaultConf` 会改变持久化时机与离线可用性，超出本轮范围。只要求在同一表单内注明通道差异，后续统一另立需求。

### 4.2 候选方案对比

| 方案 | 做法 | 优点 | 代价与风险 | 结论 |
| --- | --- | --- | --- | --- |
| S1 全部并入全局设置页 | 删除页内抽屉，所有页签搬入 `/settings/vault` | 单一入口，语义最干净 | 设置窗口是独立 Tauri window，未解锁时无法执行保险库动作；需 Drawer / Modal / 条目上下文；改造量大且与「未解锁只显示引导」冲突 | 否决 |
| S2 全部并入页内面板 | 删除 `/settings/vault` 页，conf 开关搬入抽屉 | 单一入口，改动集中在 vault 目录 | 违反项目既有设置架构（`fieldRegistry` / `confSlot` 统一在设置窗口）；未解锁时无法调整自动锁定等偏好 | 否决 |
| S3 只改名，不迁移 | 抽屉改名「管理」，内容不动 | 成本最低 | 不解决 `vaultSshAutoAdd` 双入口与同名分组问题 | 不足 |
| S4 按「配置 / 动作」二分 + 交叉引用 | 全局设置页收全部 conf；页内面板改名「保险库管理」，只留动作与仓库绑定；删掉抽屉内重复开关，两处互相指引 | 职责单一、命名自洽、成本可控、不破坏既有设置架构 | 需迁移 1 个开关的 UI 入口、修改 2 处文案与 2 处 i18n 键 | **推荐** |

### 4.3 推荐方案 S4 的实施清单

#### 4.3.1 命名

| 位置 | 现状 | 改为 |
| --- | --- | --- |
| 页头按钮文案 | `vault.tab.tools`（「工具」/ "Tools"） | 新增 `vault.tab.manage`（「管理」/ "Manage"） |
| 抽屉标题 | `vault.tab.tools` | 同上，改用 `vault.tab.manage` |
| 页签 `sync` | `vault.tab.sync`（「同步」） | `vault.tab.sync`（「同步与远程」/ "Sync & Remote"） |
| 全局设置页 SSH 分组 | `settings.vault.ssh.title`（「SSH Agent」） | 「SSH Agent 偏好」/ "SSH Agent Preferences" |
| 抽屉 SSH 页签 | `vault.tab.ssh_agent`（「SSH Agent」） | 保持「SSH Agent」（内容为运行时密钥管理） |

命名理由：「管理」覆盖同步、SSH、恢复、密码、导入导出、危险操作，且与「设置」明确区分；页签改为「同步与远程」后，其「配置 + 动作」的混合属性在名称上即可自洽，不必强行拆分。

#### 4.3.2 内容归属调整

| 项 | 现状 | 调整 |
| --- | --- | --- |
| `vaultSshAutoAdd` | 全局设置页开关 + 抽屉 `SshAgentTab` 手写 `Switch` | 删除抽屉内的 `Switch` 与 `handleToggleAutoAdd`；改为只读展示当前值 + 「在设置中修改」链接（跳 `/settings/vault`） |
| `vaultSyncSshHosts` | 抽屉 `sync` 页签（`VaultConf` 通道） | 保留在抽屉，与远程地址同屏（属同步行为调优）；在文档与代码注释层面标注其通道差异，后续再统一 |
| 远程绑定、初始化 / 克隆、拉取 / 推送 / 强制推送、冲突、待确认删除 | 抽屉 `sync` 页签 | 保留 |
| SSH Agent 密钥列表、加 / 移除、运行时状态 | 抽屉 `ssh_agent` 页签 | 保留 |
| 恢复密钥、修改主密码、导入导出、危险操作 | 抽屉对应页签 | 保留 |
| `vault-general` 全部偏好 | 全局设置页 | 保留，作为 conf 的唯一入口；其中 `vaultHibpAutoAudit` 按 F12 移除 |

#### 4.3.3 交叉引用文案

- 全局设置页 `vault-ssh` 分组底部增加一行说明：「密钥列表与运行时状态在保险库内『管理 → SSH Agent』查看」。新增键 `settings.vault.ssh.agent_location_hint`。
- 抽屉 `SshAgentTab` 顶部说明后增加一行：「自动添加开关属于全局偏好，在设置 → 密码保险库 → SSH Agent 偏好中修改」。新增键 `vault.ssh.auto_add_in_settings`。

#### 4.3.4 验收标准

1. 全仓仅剩一处 `vaultSshAutoAdd` 的写入 UI（全局设置页），抽屉内为只读。
2. 页头按钮、抽屉标题均显示「管理」，并从 `zh-CN.json` / `en-US.json` 中删除 `vault.tab.tools` key（`command.category.tools` 是不同 key，不受影响）。
3. 全局设置页与抽屉中不再出现两个同名「SSH Agent」标题。
4. 未解锁状态下，全局设置页仍可查看并修改 `vault-general` 全部偏好（`vaultHibpAutoAudit` 除外）。
5. `vault-general` / `vault-ssh` 的设置字段定义（`module.ts` 的 `ExtensionSettingDefinition`，经 `getDefinitionsBySection` 渲染）与 `VaultConf` 字段保持一致，无新增 `Record<string, unknown>` 传递。
6. 移除 `vaultHibpAutoAudit` 设置项后，同步更新 `src/settings/__tests__/fixtures/snapshot.full.json` 与 `snapshot.diff.json`。
7. 检索检查：
   - `rg "vault.tab.tools|handleToggleAutoAdd" src` 无结果；
   - `rg "vaultHibpAutoAudit" src` 只允许命中 `src/features/vault/conf.ts`（兼容字段）；
   - `rg "hibp_auto_audit" src/i18n` 无结果。
8. `zh-CN.json` 与 `en-US.json` 同步更新；`vault.confirm_lock` 只用于确认框，`vault.lock` 只用于 Tooltip。

---

## 5. 实施顺序

排序原则：按 P0 / P1 优先；F4 / F5 的 UI 部分与 F9 同批，因为它们共用分类栏与窄屏结构，拆开会导致同一区域两次重排；但 F4 / F5 的状态复位（泄露筛选自动关闭、锁定重置筛选）提前到阶段二，确保 P0 死路尽早消失。

| 阶段 | 内容 | 依赖与理由 | 风险 | 完成判据 |
| --- | --- | --- | --- | --- |
| 阶段一 | F8（定时器移出 store + selector）、F10（去重复解密）、F14（锁定态不布防） | 无 | 低；纯状态层与订阅调整 | 移动鼠标不触发列表重渲染；单次选中只 1 次解密；锁定态不新增定时器 |
| 阶段二 | F1 / F2 / F3 / F6 / F7 / F12 / F13 / F17；F4 / F5 的状态复位 | F2 需后端新增 COUNT 查询；F6 需同步修改 `syncDisplay.test.ts`；F17 涉及 Rust 命令签名 | 低；追加刷新与派生顺序调整 | 条目操作后 `ahead` 立即更新；冲突态优先于 `syncError`；泄露与筛选在删除 / 锁定后正确复位；无效开关移除；解锁期间 UI 不冻结 |
| 阶段三 | §4 设置语义整合（命名 + `vaultSshAutoAdd` 单入口 + 交叉引用） | 阶段二（状态栏文案稳定，同批 i18n 改动） | 中；涉及 i18n 双语言文件与两处组件 | §4.3.4 全部满足 |
| 阶段四 | §3.5 页头重构（H1–H5、A2 / A5 / A6） | 阶段三（命名已定） | 低 | 管理 / 锁定均可一眼识别，对齐、倒计时与无障碍名称达标 |
| 阶段五 | F4 / F5 筛选条、F9 窄屏结构、V2 内容区宽度判定、F15、F16 | V2 与 F9 共用同一「窄屏」判定，必须同批 | 中；布局结构调整 | 任意筛选可一键清除；窄屏列表不卸载；侧栏展开/折叠下双栏判定正确；审计有进度与取消 |
| 阶段六 | V1、V3–V11、A1、A3、A4 | 阶段五 | 中；需逐屏核对 | 1080p 与 768p 各断点无假双栏；分类栏单行（临界宽度实测）；组件拆分完成 |

验收口径：阶段一至四完成后，首页不应再出现「状态显示与实际不一致」「同一配置两处可改」「纯图标按钮无名称」「无效开关」四类问题；阶段五完成后，窄屏进出详情不再卸载列表；阶段六完成后，全断点分类栏保持单行、无假双栏。

### 5.1 验证方式

- 单测：`src/vault/core/__tests__/syncDisplay.test.ts` 必须覆盖 F6 的新优先级（`repoState` 优先、`syncError` 仅在无冲突时生效）；`src/vault/core/__tests__/vaultStore.test.ts` 增加 F1 / F3 / F13 的 mock 用例（刷新同步态、清理泄露映射、删除失败 reject）。
- 检索：阶段二 / 三完成后按 §4.3.4 第 7 条执行三条检索；`vaultSshAutoAdd` 的写入 UI（`dispatchConfPatch` / `Switch`）只应保留在全局设置页路径，抽屉内为只读。
- golden 快照：修改设置字段后需重新生成 `src/settings/__tests__/fixtures/snapshot.full.json` 与 `snapshot.diff.json`。
- 布局：V2 验收需在侧栏展开（1024px、848px、768px）与折叠（848px、768px）两组状态下各验证一次；其余视觉项在 1366×768、1280×720、992px、900px、768px 各截一图留档（仓库内相对路径，不写本机绝对路径）。

---

## 6. 附：本文件涉及的关键代码位置

| 关注点 | 文件 |
| --- | --- |
| 主窗口外壳（视图分发、Titlebar、侧栏、Content） | `src/App.tsx`、`src/layouts/MainLayout.tsx`、`src/components/Titlebar.tsx`、`src/components/AppSidebar.tsx`、`src/config/constants.ts` |
| 页面骨架、页头、窄屏分支、抽屉、内容区断点 | `src/vault/VaultApp.tsx` |
| 列表编排、两个 Modal | `src/vault/VaultList.tsx` |
| 列表子组件 | `src/vault/VaultToolbar.tsx`、`VaultActiveFilters.tsx`、`VaultCategoryBar.tsx`、`VaultEntryRow.tsx`、`VaultAlerts.tsx` |
| 同步态渲染、泄露提示 | `src/vault/VaultStatusBar.tsx`、`src/vault/core/syncDisplay.ts` |
| 条目 / 审计 / 同步状态层 | `src/vault/core/slices/vaultEntriesSlice.ts`、`vaultAuditSlice.ts`、`vaultSyncSlice.ts` |
| 自动锁定与锁定事件 | `src/vault/core/useVaultAutoLock.ts`、`src/vault/core/slices/vaultLifecycleSlice.ts`、`src/vault/core/useVaultEvents.ts` |
| 设置抽屉与页签 | `src/vault/settings/VaultSettings.tsx`、`VaultSyncSettings.tsx`、`SshAgentTab.tsx` |
| 全局设置页与字段定义 | `src/features/vault/settings/VaultSettingsPage.tsx`、`src/features/vault/module.ts`、`src/features/vault/conf.ts` |
| 样式与色调 | `src/vault/vault.module.css`、`src/vault/core/vaultColors.ts` |
| 文案 | `src/i18n/locales/zh-CN.json`、`src/i18n/locales/en-US.json` |
| 后端条目变更与同步提交 | `src-tauri/src/cmd/vault/entries.rs` |
| HIBP 审计实现与节流 | `src-tauri/src/cmd/vault/audit.rs` |
| 条目查询（无总数） | `src-tauri/crates/assistant-vault/src/security_db/entries/query.rs` |
| 设置字段 golden 快照 | `src/settings/__tests__/fixtures/snapshot.full.json`、`snapshot.diff.json` |

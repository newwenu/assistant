# 时钟与便签工具参考项目分析

> 日期：2026-10-31｜性质：**选型分析**｜结论：两者均按现有工具箱机制落地；悬浮形态统一走 **mini 模式**，不新增窗口
> 关联：[MAIN-WINDOW-MINI-MODE.md](./MAIN-WINDOW-MINI-MODE.md)（mini 形态定义与窗口层改造）· `docs/ARCHITECTURE.md` · `docs/FRONTEND.md`

---

## 1 结论速览

| 项 | 时钟工具 | 便签工具 |
|---|---|---|
| 是否需改框架 | 否 | 否，但需新增存储层 |
| 建议分类 | 新增 `productivity` | 新增 `productivity` |
| 数据存储 | localStorage（配置 + 运行态快照） | SQLite（业务数据） |
| 完整形态 | 工具页（`default` 宽） | 工具页（`full` 宽，列表 + 编辑双栏） |
| 悬浮形态 | **mini 模式**（复用 main 窗口） | **mini 模式**（复用 main 窗口） |
| 主要参考 | it-tools `chronometer`、Pomatez | airi notebook、notes / memos |
| 落地成本 | 低（纯前端） | 中（前端 + Rust + 表结构 + 测试） |

一句话：**时钟是"纯前端工具页"，便签是"第一个带业务数据的工具"**；两者的"悬浮"都由 [MAIN-WINDOW-MINI-MODE.md](./MAIN-WINDOW-MINI-MODE.md) 提供的 mini 形态承载，因此**不需要新增窗口、不需要跨窗口同步**。

---

## 2 项目现状（与本需求相关的机制）

| 机制 | 现状 | 位置 |
|---|---|---|
| 工具注册 | `defineTool()` 按 keywords 自动归类，`createdAt` 决定"新"标记 | `src/tools/registry.ts` |
| 工具契约 | `id/name/description/icon/keywords/component/widthMode/capabilities` | `src/tools/types.ts` |
| 分类 | 固定 4 类：`dev`、`converter`、`security`、`media` | `src/tools/categories.ts` |
| 工具页容器 | 统一标题、收藏、宽度模式；以 `<tool.component key={id} />` 渲染，**不传 props** | `src/tools/ToolPage.tsx` |
| 工具状态 | 收藏、最近使用、Fuse 模糊搜索 | `src/stores/toolStore.ts` |
| 存储 | SQLite 已有 `tool_favorites`、`tool_history`，尚无业务数据表 | `src-tauri/crates/assistant-storage/src/db.rs` |
| 主窗口 | `decorations: false`，最小 300×200，运行时仅一种呈现 | `src-tauri/tauri.conf.json`、`src-tauri/crates/assistant-config/src/constant.rs` |
| 多窗口 | 按 webview label 固定映射；窗口由 `WindowSpec + show_or_create` 创建 | `src/App.tsx`、`src-tauri/src/window.rs` |
| 悬浮能力 | Orb 已有位置持久化、多屏校正、跨窗口广播 | `src-tauri/crates/assistant-orb/src/placement.rs`、`src-tauri/src/protocol/events.rs` |
| mini 模式 | **分析完成，尚未实现**（`normal / mini` 运行时状态、外壳分叉、几何持久化） | [MAIN-WINDOW-MINI-MODE.md](./MAIN-WINDOW-MINI-MODE.md) |
| 快捷键 | 已有全局快捷键插件与默认绑定，配置为 `Record` | `src-tauri/src/setup.rs`、`src/settings/core/types/shortcutTypes.ts` |
| 通知 | 未接入 `tauri-plugin-notification` | `src-tauri/Cargo.toml` |
| i18n | 中英双语、类型化 key，新增工具须同步两个 JSON | `src/i18n/locales/*.json` |

两个约束需要提前注意：

1. **分类由关键词打分决定**：分数相同时取数组靠前的分类。现有 `dev` 已含 `timer`、`cron`，`converter` 已含 `date`、`time`、`timezone`。新增 `productivity` 时，其签名关键词不能与这两类重叠，否则工具会被错误归类。
2. **mini 是唯一悬浮形态**：mini 文档把"不新增窗口"定为硬前提，且 `D8` 明确不允许"完整窗口 + 小窗并存"。因此时钟/便签**不做独立悬浮窗口**，也不存在"两个 WebView 同时编辑便签"的一致性问题。

---

## 3 需求拆解

### 3.1 时钟

| 功能 | 实现要点 | 风险 |
|---|---|---|
| 世界时钟 | `Intl.DateTimeFormat` + IANA 时区，夏令时由浏览器处理 | 不要手算时区偏移 |
| 秒表 / 倒计时 | 保存 `startedAt`/`endAt` 绝对时间，tick 时用 `Date.now()` 重算 | 递减计数在窗口隐藏时被节流，累积漂移 |
| 番茄钟 | 阶段状态机：专注 → 短休 → 长休，按轮次自动流转 | 状态机需可快照，崩溃/隐藏后可恢复 |
| 提醒 | 应用内提示 + 声音；后台通知需通知插件 | 插件是新增依赖，需评估权限与体积 |
| 悬浮时钟 | **mini 模式**：恒置顶 + 大字号计时 + 极简控制 | 不要做点击穿透，否则无法拖动/操作 |

**隐藏态的特殊处理**：`close_to_tray` 生效时窗口 `hide()`，WebView 的 `setInterval` 会被系统节流。因此时钟必须做到：

1. 运行态（`startedAt`/`endAt`/阶段/轮次）落 localStorage 快照，不只存在内存；
2. 每次 tick 与每次窗口 `show`/`focus` 都用 `Date.now()` 重算剩余时间；
3. 首版"倒计时结束"只在窗口可见时弹应用内提示；需要"隐藏时也提醒"再加 Rust 侧定时器 + 通知插件。

### 3.2 便签

| 功能 | 实现要点 | 风险 |
|---|---|---|
| 列表 / 编辑 | 完整形态：左列表 + 右编辑（`widthMode: "full"`） | 无 |
| Markdown | 依赖中已有 `react-markdown` + `remark-gfm`，做"编辑/预览"切换即可 | 无需引入富文本编辑器 |
| 自动保存 | 防抖 + 串行写入（参考 `src/utils/persistQueue.ts` 思路） | 并发写覆盖 |
| 悬浮便签 | **mini 模式**：单便签编辑器 + 顶部便签切换 | 双栏布局在 380 宽下无法压缩 |
| 提醒 | 可选，参考任务模型的 `dueAt`/`nextNotifyAt` | 依赖通知能力 |
| 回收站 | `deleted_at` 软删除 | 避免物理删除 |

**存储判断**：便签首版就用 SQLite。理由不是"多窗口同步"（mini 下只有一个 WebView），而是业务数据需要持久、可导出、可被 AI 上下文复用，且与 `tool_favorites` / `tool_history` 的既有分层一致。若只做一次性草稿，localStorage 也能跑，但后续迁移成本更高。

**布局分支判断**：便签是同一个工具在两种宽度下呈现两种结构（双栏 / 单栏），不是两个工具。用 `useWindowMode()` 在组件内分支即可，**不要把双栏和单栏拆成两个组件再各存一份草稿**。

---

## 4 参考项目选型

### 4.1 现有 `references/` 内可直接借鉴

`references/` 当前 5 个分组：`1-ai-agent`、`2-desktop-orb`、`3-vault-sync`、`4-vault-security`、`5-design-style`。

| 项目 | 借鉴点 | 位置 | 用途 |
|---|---|---|---|
| airi | `NotebookEntry`（kind/text/tags/metadata/createdAt）、`ScheduledTask`（priority/status/dueAt/lastNotifiedAt/nextNotifyAt） | `references/2-desktop-orb/airi/packages/stage-ui/src/stores/character/notebook.ts` | 便签字段与可选提醒字段设计 |
| onepoint | 全局唤起、即用即走、本地存储与导出、历史列表 | `references/2-desktop-orb/onepoint/README-CN.md` | 便签"快速记录"的产品定位 |
| BongoCat | 悬浮窗位置防抖写入、关闭前 flush、坐标范围校验 | `references/2-desktop-orb/BongoCat/crates/bongocat-app/src/overlay_placement.rs`、`crates/bongocat-config/src/window_state.rs` | **供 mini 模式几何持久化参考**；工具本身不涉及窗口 |
| WindowPet | 透明、置顶、跳过任务栏、多窗口创建参数组合 | `references/2-desktop-orb/WindowPet/src-tauri/src/app/utils.rs` | 仅供 mini 的置顶/跳过任务栏行为参考（旧 Tauri API） |
| awesome-design-md | 卡片、悬浮层、间距与配色规范 | `references/5-design-style/awesome-design-md/` | 时钟卡片与便签视觉 |
| OpenBitFun | 内置 miniapp 静态嵌入、安装标记、内容哈希 | `references/1-ai-agent/OpenBitFun/src/crates/contracts/product-domains/src/miniapp/builtin.rs` | 仅当便签要做成可安装迷你应用时参考，不建议 |

**明确不采纳**：WindowPet 的全局点击穿透（时钟/便签需可拖动可点击）、透明窗口（mini 文档 §8.1 已定 `transparent` 不可运行期切换且建议保持不透明）、OpenBitFun 的 miniapp 运行时（对一个工具页过重）。

### 4.2 建议补充的外部参考

`references/` 目录体量已较大，建议只补 3 个，统一放 `references/6-productivity/`，克隆时用 `--depth 1`：

| 项目 | 技术栈 | 借鉴内容 | 优先级 |
|---|---|---|---|
| `roldanjr/pomatez` | Electron + React | 番茄钟状态机、阶段流转、托盘倒计时、通知、主题 | 高（时钟最贴合） |
| `nuttyartist/notes` | Qt / C++ | 桌面便签的文件夹/标签/回收站/Markdown、贴纸交互 | 高（便签产品形态最贴合） |
| `usememos/memos` | Go + React | 快速记录、标签、极简列表的信息架构 | 中（便签结构参考） |

暂不引入：`notesnook` / `joplin`（加密与同步需求未定，体量大且 copyleft）、`TrafficMonitor`（桌面悬浮已由 mini 覆盖，不再需要）。外部项目一律以各自 `LICENSE` 为准，GPL/AGPL 只借鉴设计不复制代码。

### 4.3 推荐组合

- 时钟完整形态 = it-tools `chronometer` 交互 + Pomatez 状态机 + 现有 AntD / i18n
- 时钟 mini 形态 = Pomatez 的"单一计时焦点 + 极简控制" + mini 外壳（无侧栏、恒置顶）
- 便签完整形态 = airi 字段模型 + notes/memos 的列表-编辑结构 + SQLite
- 便签 mini 形态 = onepoint 的"快速记录"交互 + 单便签编辑器

---

## 5 与 mini 模式的对接（核心）

### 5.1 mini 模式已经提供了什么

| 能力 | 对两个工具的意义 |
|---|---|
| 复用 main 窗口、不新建 webview | 工具输入状态天然保留，切换形态不丢内容 |
| `normal / mini` 运行时状态（Rust `Mutex<Mode>` + 前端 `windowModeStore`） | 工具可以读模式做布局分支 |
| 恒置顶（`effective_on_top = mode == mini \|\| stay_on_top`） | 时钟"悬浮时钟"、便签"桌面便签"开箱可用 |
| 两套几何 + 工作区收敛 | 小窗位置由窗口层负责，工具不参与 |
| `ToolPage` 密度降级（padding / 标题 / 描述 / 分割线） | 工具内部不用重复处理外层间距 |
| 小窗首页"启动器"（搜索 + 收藏/最近 chip + 记住上次工具） | 时钟/便签可被快速唤出，无需额外入口 |

### 5.2 两个工具需要 mini 提供的三件事

| # | 需求 | 建议实现 | 备注 |
|---|---|---|---|
| N1 | **`miniFriendly` 元数据** | `ToolDefinition` 增加 `miniFriendly?: boolean`；`true` 表示"可在小窗中使用，工具自带紧凑布局" | mini 文档 §8.4 已提出，目前列为 P2；**建议提前到 P1**，两个新工具成为首批使用者 |
| N2 | **工具侧读取模式** | `useWindowMode()`（读 `windowModeStore`，只读），工具组件内使用 | `ToolPage` 以 `<tool.component key={id} />` 渲染且不传 props，因此只能走 store hook |
| N3 | **直达某工具的 mini 入口** | 托盘 / Orb 菜单 / 快捷键 → `set_main_window_mode("mini")` + `navigate("/tools/<id>")` | 依赖 mini 文档 P2；工具侧只需保证 id 与路由稳定 |

### 5.3 `widthMode` 与 `miniFriendly` 的优先级（必须定死）

mini 文档 `D5` 的默认策略是"`wide`/`full` 工具自动展开 + 可撤销提示"。若直接按此执行，**便签（`widthMode: "full"`）永远无法在 mini 中打开**，与本次需求冲突。

建议把判定顺序改为：

```
1. tool.miniFriendly === true  → 留在 mini，工具渲染紧凑布局
2. widthMode === "wide"/"full" → 自动展开（D5 原策略）+ 可撤销提示
3. 其余                        → 留在 mini
```

对应结论：

| 工具 | `widthMode` | `miniFriendly` | mini 行为 |
|---|---|---|---|
| 时钟 | `default` | `true` | 留在 mini，大字号计时布局 |
| 便签 | `full` | `true` | 留在 mini，单栏编辑布局 |

这条优先级规则需要回写到 mini 文档 `D5` 与 `§5.5`，否则实现阶段会按 `widthMode` 直接自动展开，便签的 mini 形态直接失效。

### 5.4 工具内的模式分支：运行态必须外置

`ToolPage` 用 `<tool.component key={id} />` 渲染，`key` 只在工具 id 变化时改变，因此 **normal ↔ mini 切换不会重挂载工具组件**。但工具内部若按模式渲染不同子树，子树会卸载重建。由此得出硬性要求：

| 工具 | 运行态存放位置 | 原因 |
|---|---|---|
| 时钟 | `clock.service.ts` 模块级单例 + `useSyncExternalStore`（或轻量 store） | 若把 `setInterval`/剩余时间放 `useState`，布局分支切换会重置计时 |
| 便签 | `noteStore`（Zustand，草稿与当前便签 id 都在 store） | 双栏↔单栏分支切换不能丢未保存草稿 |

这条与项目既有的 `Service → Orchestrator → Store` 分层一致：计时状态机属于 service，草稿与列表属于 store，组件只做渲染。

### 5.5 两种形态的逐项对齐

| 关注点 | 时钟 normal | 时钟 mini | 便签 normal | 便签 mini |
|---|---|---|---|---|
| 主结构 | 世界时钟网格 + 秒表/倒计时/番茄钟分区 | 单一大字号计时 + 底部控制条 | 左列表 + 右编辑器 | 单便签编辑器 + 顶部便签切换 |
| 次要操作 | 城市增删、主题、音效 | 折叠为图标按钮 | 标签、颜色、置顶、回收站 | 折叠为紧凑工具条 |
| 外层间距 | `24px 32px`（由 `ToolPage` 提供） | `10px 14px`（由 `ToolPage` 提供） | 同左 | 同左 |
| 宽度约束 | `maxWidth: 640` | 跳过 `maxWidth`，占满窗口 | `maxWidth: 0`（`full`） | 跳过 `maxWidth` |
| 数据 | localStorage（配置 + 快照） | 同左（同一份） | SQLite | 同左（同一份） |
| 持久化触发 | 配置变更、计时状态变更 | 同左 | 防抖自动保存 + 失焦/切换工具时 flush | 同左 |

### 5.6 需要回写 mini 文档的三处调整

| 位置 | 现状 | 建议调整 |
|---|---|---|
| `D5` / `§5.5` | `wide`/`full` 一律自动展开 | 改为"`miniFriendly` 优先，`widthMode` 兜底"（见 5.3） |
| `§8.4` | `miniFriendly` 列为 P2 中期项 | 提前到 P1，与两个新工具同批落地 |
| `§5.3` | `ToolPage` 统一密度降级 | 补充"工具可自带紧凑布局，`ToolPage` 只负责外层密度与 `maxWidth` 跳过" |

### 5.7 mini 层的既有决策对工具的影响

| mini 决策 | 对时钟 | 对便签 |
|---|---|---|
| `transparent` 保持 `false`（§8.1） | 不能做半透明时钟；用背景色 + 系统圆角 | 便签"贴纸感"靠背景色/阴影，不靠透明 |
| `skip_taskbar` 先不启用（D6） | 悬浮时钟会占一个任务栏图标，后续实测再定 | 无影响 |
| `X` = 隐藏到托盘（D2） | 隐藏后计时靠快照 + 重算恢复 | 关闭前需 flush 草稿 |
| 进入 `/settings` `/history` `/vault` 强制展开（D4） | 无影响 | 无影响 |
| 不允许"完整窗口 + 小窗并存"（D8） | 无影响 | 单 WebView，无跨窗口一致性问题 |
| Orb 层级避让（§7.5） | 悬浮时钟与 Orb 会争 z-order，进入 mini 时应收起 Orb 面板 | 同左 |

---

## 6 落地路径

### Phase 1：两个工具页

时钟（纯前端）：

```
src/tools/clock/
├── index.tsx               工具组件（默认导出，无 props）
├── clock.service.ts        计时状态机 / 时区格式化 / 快照
└── clock.module.css
```

便签（前端 + 后端）：

```
src/tools/sticky-notes/
├── index.tsx
├── sticky-notes.service.ts
└── sticky-notes.module.css

src/services/noteService.ts        invoke 封装
src/stores/noteStore.ts            状态 + 防抖自动保存
src-tauri/crates/assistant-storage/src/notes.rs
src-tauri/src/cmd/notes.rs
```

公共改动：

- `src/tools/registry.ts` 注册两条，带 `createdAt` 获得"新"标记，`miniFriendly: true`
- `src/tools/categories.ts` 新增 `productivity` 分类、图标、`order`、签名关键词
- `src/i18n/locales/zh-CN.json`、`en-US.json` 同步新增 key（mini 标题栏直接展示 `tool.name`，缺 key 会显示原始 key）
- `src-tauri/crates/assistant-storage/src/db.rs` 新增 `notes` 表（`CREATE TABLE IF NOT EXISTS`，老库可安全升级）
- `src-tauri/src/cmd/mod.rs`、`src-tauri/src/lib.rs` 注册 handler
- `src-tauri/crates/assistant-storage/tests/notes.rs` 参照 `tool_favorites.rs` 的内存库测试

建议的 `notes` 表最小字段：

```sql
id TEXT PRIMARY KEY,
title TEXT NOT NULL DEFAULT '',
content TEXT NOT NULL DEFAULT '',
color TEXT NOT NULL DEFAULT 'default',
pinned INTEGER NOT NULL DEFAULT 0,
tags TEXT NOT NULL DEFAULT '[]',
sort_order INTEGER NOT NULL DEFAULT 0,
created_at INTEGER NOT NULL,
updated_at INTEGER NOT NULL,
deleted_at INTEGER
```

### Phase 2：mini 对接（依赖 mini 文档 P0/P1）

前置：`windowModeStore` + `set_main_window_mode` + 外壳分叉 + `ToolPage` 密度降级已落地。

1. `ToolDefinition` 增加 `miniFriendly?: boolean`，两个工具置 `true`
2. `ToolPage` 的 `widthMode` 判定改为"`miniFriendly` 优先"
3. 两个工具组件接入 `useWindowMode()`，实现 5.5 的紧凑布局
4. 时钟运行态外置到 `clock.service.ts`，便签草稿外置到 `noteStore`
5. 便签在 `mode` 变化与工具切换时 flush 草稿
6. 进入 mini 时若 Orb 展开，先 `collapse_orb_panel`

### Phase 3：联动

- `quick_note` 全局快捷键：`shortcutTypes.ts` + Rust 配置默认值 + `setup.rs` 注册 + i18n；行为 = 进入 mini + 新建便签
- 可选 `toggle_clock_mini` 或复用 `toggle_main_mini`，在 mini 中直达时钟
- 番茄钟完成通过 Orb / Mascot 提醒，而非立即引入通知插件
- 便签内容接入 AI 上下文（可选）

---

## 7 待确认决策

1. **时钟范围**：世界时钟 / 秒表 / 倒计时 / 番茄钟选哪几个？（全部纳入会让 mini 布局拥挤，建议 mini 只保留"当前计时 + 控制"）
2. **便签内容**：纯文本还是 Markdown？是否需要提醒字段？
3. **存储**：便签直接上 SQLite（建议是），还是首版 localStorage。
4. **分类**：新增 `productivity`（建议），还是分别塞进 `converter` 与 `dev`。
5. **`miniFriendly` 是否提前**：建议提前到 P1 并由两个新工具首批使用；否则便签无法在 mini 中使用。
6. **通知**：先用应用内提示，还是直接接入系统通知插件（影响"隐藏态番茄钟提醒"）。
7. **外部参考**：是否将 `pomatez` / `notes` / `memos` 克隆到 `references/6-productivity/`。

---

## 8 主要风险清单

| 风险 | 说明 | 对策 |
|---|---|---|
| 分类误判 | `timer`/`date`/`time` 已被现有分类占用 | 新分类签名词避开重叠，并补分类单测 |
| 计时漂移 | 递减计数在窗口隐藏时被节流 | 绝对时间戳重算 + `visibilitychange`/`focus` 重算 + localStorage 快照 |
| 布局分支重置状态 | normal ↔ mini 切换会卸载工具内子树 | 计时状态放 service、草稿放 store，组件不持有运行态 |
| 便签无法进 mini | `D5` 按 `widthMode` 自动展开，便签是 `full` | 引入 `miniFriendly` 并设为最高优先级（5.3） |
| 便签双栏压不进小窗 | 380 宽下列表 + 编辑器不可用 | mini 走单栏布局，列表收进顶部切换器 |
| 隐藏时无提醒 | 未接入通知插件 | 首版仅可见时提示，后续 Rust 定时器 + 插件 |
| 半透明不可行 | `transparent` 仅创建期可设 | 用背景色 + 系统圆角，不做透明 |
| 依赖膨胀 | 富文本编辑器 / 通知插件 | 复用已有 `react-markdown`，通知按需引入 |
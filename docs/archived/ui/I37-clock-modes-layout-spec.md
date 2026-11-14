# I37 — clock 双模式与双布局优化规格

| 项 | 值 |
|---|---|
| 状态 | Implemented（2026-11-04，自动验收通过；手工回归待补） |
| 上游规格 | `docs/archived/ui/I26-mini-mode-clock-spec.md` |
| 上游分析 | `docs/analysis/CLOCK-NOTES-REFERENCE-ANALYSIS.md`、`docs/archived/ui/MAIN-WINDOW-MINI-MODE.md` |
| 参考实现 | `references/6-productivity/pomatez`、`references/6-productivity/stretchly` |
| 影响范围 | clock 的 Rust 运行态与 DTO、clock 工具前端目录、i18n、协议契约 |
| 不变部分 | mini 窗口形态、`ToolPage` 密度、`clock` 的 `miniFriendly` 与 `productivity` 分类 |

本文定义 **普通计时（Stopwatch）与倒计时（Countdown）两种模式**，并在现有 `ClockLayout` / `ClockMiniLayout` 基础上优化两套布局。I26 的窗口形态、几何、通知与分类结论继续有效；本规格只取代 I26 §9.2–§9.4 中「只有倒计时」的状态机定义。

---

## 1 概述

### 1.1 现状

| 层 | 现状 |
|---|---|
| Rust 运行态 | `src-tauri/src/clock.rs` 只有倒计时：`duration_ms` + `elapsed_before_ms` + `started_at_ms`，快照派生 `remaining_ms`，到点 `Finished` 并通知 |
| DTO | `ClockSnapshotDto` 没有 `mode` / `elapsed_ms`，`remaining_ms` 是必填数字 |
| 命令 | `clock_start(duration_ms)` / `pause` / `resume` / `reset` / `snapshot` |
| 前端 | `src/tools/clock/` 已拆为 `index.tsx` + `ClockLayout` + `ClockMiniLayout` + `ClockRing` + `clockView` + 一个 CSS module |
| 布局选择 | 容器宽度 ≤ `MINI_DENSITY_BREAKPOINT` 时渲染 `ClockMiniLayout`，不读取窗口 `mode` |
| 布局内容 | 两套布局目前结构基本相同，仅尺寸与间距不同；倒计时运行中仍显示预设与自定义行 |

### 1.2 本阶段

1. **新增计时模式**：同一时钟实例可在「倒计时」与「普通计时」之间切换，后者从 0 正计时。
2. **优化两套布局**：普通布局承载完整控制与时长选择；紧凑布局只保留模式、时间、状态和主操作，减少纵向占用。
3. **统一视图模型**：Rust 快照同时给出 `mode`、`remaining_ms`、`elapsed_ms`，前端根据模式选择显示值，不在组件内做模式相关的计时运算。

---

## 2 目标

- 倒计时与普通计时共用一套运行态，切换模式不新增计时器实例。
- 普通计时可暂停、继续、重置；不自动结束、不触发完成通知。
- 倒计时行为保持 I26：1–180 分钟、到点通知、完成态保留重置入口。
- 两套布局在相同状态下表达相同语义，只改变信息密度与控件形态。
- 紧凑布局在 `MINI_WINDOW_WIDTH` 对应内容宽度下不横向溢出，且不因首帧测量产生可见的布局跳变。
- 时间显示 ≥ 1 小时时使用 `h:mm:ss`，避免 `180:00` 这类分钟数无限增长。

---

## 3 非目标

- 番茄钟阶段（专注/短休/长休）、轮次、自动进入下一阶段。
- 同时运行多个计时器，或同一时钟内保留多轮历史。
- 计时状态跨进程重启恢复；进程退出后回到「倒计时 25 分钟 Idle」。
- 托盘倒计时图标、常驻提醒、严格模式、阻止系统休眠。
- 手动「紧凑模式」开关：布局由容器宽度自动选择，不引入第二个密度开关。
- 全局快捷键切换模式或开始/暂停。
- 修改 `widthMode` / `miniFriendly` / 分类关键词。

---

## 4 参考项目结论

### 4.1 可采纳的部分

| 来源 | 做法 | 本规格怎么用 |
|---|---|---|
| `references/6-productivity/pomatez/app/renderer/src/routes/Timer/Counter/Counter.tsx` | `compactMode` 下只渲染时间（非全屏时连进度环都去掉），普通模式渲染进度环、类型与标签 | 紧凑布局只保留时间、状态与主控制；进度环缩小而非去掉，保持时钟的视觉识别 |
| `references/6-productivity/pomatez/app/renderer/src/routes/Timer/Control/Control.tsx` | 紧凑控制只保留重置/播放/跳过，隐藏音量、严格模式等次级操作 | 紧凑布局隐藏时长选择、完成提示与自定义说明，只保留主操作 + 重置 |
| `references/6-productivity/pomatez/app/renderer/src/routes/Timer/Counter/CounterTimer.tsx` | 小时数 > 0 时才显示小时段 | 时间 ≥ 1 小时时改用 `h:mm:ss`，两种布局一致 |
| `references/6-productivity/pomatez/app/renderer/src/store/timer/types.ts` | 用封闭集合表达计时类型，运行态与 UI 分离 | 新增 `ClockModeDto { Countdown, Stopwatch }`，Rust 持真值 |
| `references/6-productivity/stretchly/app/utils/scheduler.js` | `timeLeft = now + delay - Date.now()`，每次校正而不是累减 | 继续沿用「起点 + 当前墙钟」重算，正计时同样不累加 |
| `references/6-productivity/stretchly/app/utils/utils.js` | 区分剩余时长与已用时长的人类可读格式化 | 提取 `formatClockTime`，按模式选择取整方向 |

### 4.2 明确不采纳的部分

| 来源 | 做法 | 不采纳理由 |
|---|---|---|
| pomatez `CounterContext.tsx` | 专注/短休/长休/特殊休息的阶段流转、轮次、自动开始下一阶段 | 会让 clock 变成番茄钟编排器，超出「单计时器」范围 |
| pomatez `CounterContext.tsx` | `count -= timePassed / 1000` 的增量累减 | 隐藏窗口或休眠后会累积偏差；本项目继续用墙钟重算 |
| pomatez `Control.tsx` | 严格模式、跳过阶段、音量、阻止休眠 | 需要新的状态与系统能力，与本次目标无关 |
| pomatez `CompactModeButton.tsx` | 用户手动切换紧凑模式 | 我们的紧凑布局由容器宽度决定；手动开关会与 `ToolPage` 外壳密度形成第二真值 |
| stretchly `app/utils/scheduler.js` | 安排休息周期、托盘图标计时 | 属于休息提醒产品，不是单次计时工具 |
| stretchly 的 30/60 秒预警 | 多段提前通知 | 首版只保留到点通知，避免通知噪音与设置面扩大 |

**结论：可以参考项目优化布局与状态建模，但不引入番茄钟阶段机。** 收益集中在三处：紧凑布局的信息取舍、时间格式化、模式作为封闭集合。

---

## 5 现状与改造边界

### 5.1 已有事实

| 事实 | 位置 |
|---|---|
| 计时运行态只在 Rust 内存，不落盘 | `src-tauri/src/clock.rs` |
| 前端只做 1 秒本地续算，快照来自 `clock_snapshot` | `src/tools/clock/index.tsx` |
| 布局按容器宽度切换，不读窗口 `mode` | `src/tools/clock/index.tsx`、`src/config/constants.ts` |
| 两套布局共享 `ClockRing` 与 `clockView` | `src/tools/clock/ClockRing.tsx`、`src/tools/clock/clockView.ts` |
| 到点通知只依赖倒计时 | `src-tauri/src/clock.rs` 的 `on_finished` |
| DTO 契约由 ts-rs 生成 | `src-tauri/tests/protocol_bindings.rs`、`npm run contract:gen` |

### 5.2 改造边界

- 不改 `ToolPage` 的 mini 密度分支、标题栏与路由。
- 不改 `registry.ts` 的 `clock` 注册项，只更新 `tools.clock.description` 文案。
- 不改 `clock` 的 `miniFriendly: true`、`widthMode: "default"`。
- 不改 `productivity` 分类与关键词；`stopwatch` 已在既有 keywords 中。
- 通知插件与 Rust i18n 的 `clock.notification_*` 不变。

---

## 6 不变式

违反任一条即为实现错误：

| # | 不变式 |
|---|---|
| 1 | 同一时刻只有一个计时器实例；两种模式互斥，切换模式即放弃当前轮次 |
| 2 | Rust 仍是运行态唯一真值；前端只从快照做本地续算，不自行维护计时起点 |
| 3 | 计时值只能由「起点 + 当前墙钟」重算，禁止在 Rust 或前端做增量累减 |
| 4 | 普通计时永不进入 `Finished`，不 spawn ticker，不发 `clock:finished` |
| 5 | 模式**变化**且处于 `Running` 时返回错误；前端同时禁用模式开关（双保险） |
| 6 | `clock_set_mode` 目标模式与当前一致时幂等（即使 `Running` 也不报错）；模式变化时回到目标模式的 `Idle` |
| 7 | 倒计时时长约束仍为 1–180 分钟；普通计时没有时长上限 |
| 8 | `remaining_ms` 仅在倒计时模式有值；普通计时模式下为 `null`，且 wire 上键恒在 |
| 9 | `elapsed_ms` 两种模式恒有值；倒计时下等于 `duration_ms - remaining_ms` |
| 10 | 两套布局都只读取 `ClockViewProps`，不直接读 `windowModeStore`、不判断容器宽度 |
| 11 | 布局选择只看容器宽度；阈值统一使用 `MINI_DENSITY_BREAKPOINT`，不得复制魔法数字 |
| 12 | 计时状态不写 SQLite、不写 `AppConf`、不写 localStorage |
| 13 | 输出 DTO 的 `Option` 不添加 `skip_serializing_if`；前端按生成类型声明 `T \| null` |
| 14 | `Running` 时 `started_at_ms` 必有值；非 `Running` 时其为 `null` |

---

## 7 数据模型

### 7.1 Rust 内部状态

`src-tauri/src/clock.rs`：

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClockMode {
    Countdown,
    Stopwatch,
}

struct ClockInner {
    mode: ClockMode,
    status: ClockStatus,          // Idle | Running | Paused | Finished
    duration_ms: Millis,          // 仅倒计时配置；计时模式保留上次值
    started_at_ms: Option<Millis>,
    elapsed_before_ms: Millis,    // 已结束运行段的累计
}
```

- 默认值：`Countdown`、`Idle`、25 分钟、`started_at_ms = None`、`elapsed_before_ms = 0`。
- `ClockMode` 是内部枚举；wire 上的 `ClockModeDto` 独立定义并显式转换，内部类型不直接解析前端契约。
- `duration_ms` 在计时模式下不参与展示，但保留值，以便切回倒计时时恢复用户上次选择的时长。

### 7.2 派生规则

唯一的累计量是 `elapsed_before_ms`；运行段用当前墙钟计算：

```text
running_elapsed = elapsed_before_ms + (status == Running ? now_ms - started_at_ms : 0)

Countdown:
  remaining_ms = clamp(duration_ms - running_elapsed, 0, duration_ms)
  elapsed_ms   = duration_ms - remaining_ms

Stopwatch:
  remaining_ms = None
  elapsed_ms   = running_elapsed
```

- 倒计时 `Finished`：`elapsed_before_ms = duration_ms`，`remaining_ms = 0`，`elapsed_ms = duration_ms`。
- 倒计时 `Idle`：`remaining_ms = duration_ms`，`elapsed_ms = 0`。
- 普通计时 `Idle`：`elapsed_ms = 0`。
- 普通计时不裁剪到 `duration_ms`，允许超过 1 小时继续增长。

### 7.3 DTO

`src-tauri/src/protocol/dto/clock.rs`：

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum ClockModeDto {
    Countdown,
    Stopwatch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClockSnapshotDto {
    pub mode: ClockModeDto,
    pub status: ClockStatusDto,
    pub duration_ms: Millis,
    pub remaining_ms: Option<Millis>,   // 仅倒计时；计时模式为 null
    pub elapsed_ms: Millis,
    pub started_at_ms: Option<Millis>,  // 仅 Running 有值
    pub elapsed_before_ms: Millis,
}
```

- `remaining_ms` 改为 `Option<Millis>` 后，前端生成类型为 `number | null`；输出侧不写 `skip_serializing_if`。
- `ClockModeDto` 加入 `src-tauri/tests/protocol_bindings.rs` 的导出列表，随后 `npm run contract:gen`。
- 前端 `ClockSnapshotDto["mode"]` 由契约生成，禁止手写同名联合类型。

---

## 8 命令与状态机

### 8.1 命令

`src-tauri/src/cmd/clock.rs`：

| 命令 | 参数 | 行为 |
|---|---|---|
| `clock_set_mode` | `mode: ClockModeDto` | 相同模式直接返回快照（幂等）；模式变化时重置当前轮次；模式变化且 `Running` 时返回错误 |
| `clock_start` | `duration_ms: Option<Millis>` | 按当前模式开始新一轮；倒计时使用传入时长或当前时长；普通计时忽略该参数 |
| `clock_pause` | 无 | `Running → Paused`；其它状态空操作 |
| `clock_resume` | 无 | `Paused → Running`；其它状态空操作 |
| `clock_reset` | 无 | 回到当前模式的 `Idle`；保留模式与倒计时时长 |
| `clock_snapshot` | 无 | 返回当前快照 |

- `clock_start` 在 `Running` 或 `Paused` 时表示「重开一轮」，从 0 开始，不复用旧进度；与 I26 的 `start` 语义一致。
- 倒计时 `duration_ms` 越界（< 60_000 或 > 10_800_000）返回错误；普通计时传入 `duration_ms` 时忽略并保持 `duration_ms` 原值。
- `clock_set_mode` 与 `clock_start` 成功后都递增 `generation`，使旧 ticker 退出。
- `clock_resume` 仅在倒计时模式下 spawn ticker；普通计时不创建后台线程。

### 8.2 状态迁移

| 当前模式 | 当前状态 | 动作 | 结果 |
|---|---|---|---|
| Countdown | Idle | `start(d)` | Running，剩余 = `d` |
| Countdown | Running | `pause` | Paused，剩余冻结 |
| Countdown | Paused | `resume` | Running，从剩余继续 |
| Countdown | Running | `finish` | Finished，剩余 0，通知 |
| Countdown | Finished | `start(d)` | Running，新一轮 |
| Stopwatch | Idle | `start` | Running，elapsed 0 |
| Stopwatch | Running | `pause` | Paused，elapsed 冻结 |
| Stopwatch | Paused | `resume` | Running，从 elapsed 继续 |
| Stopwatch | Running | `finish` | **不可达**；`mark_finished` 返回 `None` |
| 任意 | Idle / Paused / Finished | `set_mode(不同)` | 目标模式 Idle，elapsed 0 |
| 任意 | Running | `set_mode(不同)` | 错误，状态不变 |
| 任意 | Running | `set_mode(相同)` | 快照返回，状态不变 |

### 8.3 ticker 与通知

- `spawn_ticker` 只在倒计时 `start` / `resume` 后调用。
- ticker 继续每最多 1 秒按墙钟重算剩余；`remaining <= 0` 时 `mark_finished` 并通知。
- `mark_finished` 增加模式判断：仅 `Countdown + Running` 才完成；普通计时返回 `None`。
- 通知文案与触发条件沿用 I26 §9.5：窗口不可见或未聚焦时发系统通知，发送失败回退显示主窗口。
- `clock:finished` 事件载荷仍为 `ClockSnapshotDto`，新增字段由契约自动包含。

---

## 9 前端视图模型

### 9.1 纯函数

新增 `src/tools/clock/clockTime.ts`，集中放可测试的纯逻辑：

```ts
export interface ClockView {
  elapsedMs: number;
  remainingMs: number | null;
  progress: number | null;
}

export function computeClockView(
  snapshot: ClockSnapshotDto,
  nowMs: number,
): ClockView;

export function formatClockTime(
  ms: number,
  mode: ClockSnapshotDto["mode"],
): string;
```

规则：

| 函数 | 规则 |
|---|---|
| `computeClockView` | 运行中 `elapsed = elapsedBeforeMs + max(0, nowMs - startedAtMs)`；倒计时 `remaining = clamp(durationMs - elapsed, 0, durationMs)`，`progress = remaining / durationMs`；普通计时 `remaining = null`、`progress = null` |
| `formatClockTime` | 倒计时对剩余毫秒向上取整到秒，避免提前显示 `00:00`；普通计时向下取整到秒；总秒数 ≥ 3600 时输出 `h:mm:ss`，否则 `mm:ss` |
| 进度环 | 倒计时使用 `progress`；普通计时不渲染进度描边，只保留轨道 |

`remaining_ms` / `elapsed_ms` 是**快照时刻**的服务端派生值，供首帧展示与非 React 消费者使用；前端本地续算必须以 `elapsed_before_ms + started_at_ms` 为准，不能拿快照里的 `remaining_ms` 再减运行时间，否则会二次扣除。两处派生公式必须一致。

### 9.2 本地续算

`src/tools/clock/index.tsx`：

- 挂载时取 `clock_snapshot()`，之后运行中每秒 `setNowMs(Date.now())`。
- 页面隐藏时停止本地定时器，`visibilitychange` 回可见时重新取快照。
- 到点事件 `UiEvent.ClockFinished` 继续更新快照；普通计时不会收到该事件。
- 本地定时器只更新 `nowMs`，所有派生值由 `computeClockView` 计算，不保存「剩余秒数」。

### 9.3 布局选择

- 继续用 `ResizeObserver` 观察 clock 根节点宽度。
- 首帧测量改用 `useLayoutEffect`（或等价的同步测量），避免 mini 窗口下先渲染普通布局、下一帧再切换的闪烁。
- 阈值使用 `MINI_DENSITY_BREAKPOINT`；不在 CSS 里再写第二份 `320px`。
- 布局选择状态只保留一个布尔值 `compact`，不进入 store，也不与窗口 `mode` 耦合。

### 9.4 视图 props

`src/tools/clock/clockView.ts` 扩展为：

```ts
export interface ClockViewProps {
  mode: ClockSnapshotDto["mode"];
  status: ClockSnapshotDto["status"];
  timeLabel: string;
  statusLabel: string;
  progress: number | null;
  isFinished: boolean;
  error: string | null;
  busy: boolean;
  minutes: number;
  primaryLabel: string | null;
  onModeChange: (mode: ClockSnapshotDto["mode"]) => void;
  onMinutesChange: (value: number | null) => void;
  onPreset: (minutes: number) => void;
  onStart: () => void;
  onPrimary: () => void;
  onReset: () => void;
}
```

- `onPreset` 只设置 `minutes`，不再直接开始；开始由 `onStart` 统一触发。
- `onModeChange` 在 `status === "running"` 时不会触发，因为开关被禁用；`index.tsx` 仍对返回值做错误兜底。
- 布局文件不计算时间、不判断模式合法性，只决定渲染区域。

---

## 10 布局规格

### 10.1 共享组件

| 组件 | 职责 |
|---|---|
| `ClockModeSwitch.tsx` | 模式切换；`Segmented`；`Running` 时禁用并显示 `tools.clock.mode_switch_running` 提示 |
| `ClockRing.tsx` | 进度环 + 时间标签；新增 `progress: number \| null` 与 `compact`；`null` 时只画轨道 |
| `clockView.ts` | 视图 props、预设与时长常量 |
| `clockTime.ts` | 纯计算与格式化 |

### 10.2 普通布局（`ClockLayout.tsx`）

自上而下：

| 区域 | 显示条件 | 内容 |
|---|---|---|
| 模式切换 | 始终 | `Segmented`：倒计时 / 计时；`Running` 禁用；普通布局使用 large 尺寸、最大宽 320 |
| 进度环 | 始终 | 200×200；时间 48px；倒计时显示进度描边，计时只显示轨道 |
| 状态 | 始终 | `准备开始` / `计时中` / `已暂停` / `已完成`；完成态使用成功色 |
| 错误 | 有错误 | `Alert`，宽度 100% |
| 时长选择 | `mode === "countdown"` | 4 个预设按钮（15/25/45/60）选中态高亮；自定义 `InputNumber` 1–180；`idle` / `finished` 可编辑并显示 `开始`，`running` / `paused` 保留展示但整体禁用 |
| 主控制 | `running` / `paused` | `暂停` / `继续` + `重置` |
| 计时空闲 | `stopwatch && idle` | `开始` + `重置`（禁用）+ `stopwatch_hint` |
| 完成提示 | `countdown && finished` | `finished_hint`；`开始` 可再次开始当前时长 |

交互规则：

- 预设按钮只改时长，不启动；`开始` 是唯一启动入口。
- 选中时长使用边框/文字强调，不使用第二个 `type="primary"`，避免与 `开始` 抢主操作层级。
- **普通布局的时长选择常驻**：`running` / `paused` 时仍显示当前时长，但预设、输入框与 `开始` 全部禁用；改变时长只能在 `idle` / `finished` 进行。
- `finished` 时时长选择重新可编辑，允许换时长后再次开始。
- `重置` 在 `idle` 时禁用；在 `finished`、`paused`、`running` 时可用。
- 普通布局使用更大的环（200）、字号（48）、按钮尺寸（default）、间距（16px），与紧凑布局形成密度差，而不是等比放大。

### 10.3 紧凑布局（`ClockMiniLayout.tsx`）

自上而下：

| 区域 | 显示条件 | 内容 |
|---|---|---|
| 模式切换 | 始终 | 紧凑 `Segmented`，宽度 100%；`Running` 禁用 |
| 进度环 | 始终 | 120×120；时间 28px；只有轨道或倒计时进度 |
| 状态 | 始终 | 13px 单行，不额外显示模式名 |
| 错误 | 有错误 | `Alert`，紧凑间距 |
| 时长选择 | `countdown && (idle \|\| finished)` | 一行 `Segmented` 数字预设 15/25/45/60（当前分钟数不在预设内时不高亮任何项）；下一行 `InputNumber`（宽 72）+ `开始` |
| 主控制 | `running` / `paused` | `暂停` / `继续` + `重置`，单行居中 |
| 计时空闲 | `stopwatch && idle` | `开始` + `重置`（禁用） |
| 完成提示 | 不显示 | 状态区已显示 `已完成` |

紧凑布局的取舍：

- 不显示 `finished_hint`、模式说明与自定义说明。
- 预设从 4 个带「分钟」的按钮改为无单位 `Segmented`，单位只在自定义输入行体现。
- 所有区域间距 8px，顶部内边距 4px；不使用 `wrap` 造成两行以上的按钮组。
- 保留返回按钮与工具标题：它们属于 `ToolPage` 外壳，不由 clock 布局承担。

### 10.4 两套布局的差异汇总

| 维度 | 普通布局 | 紧凑布局 |
|---|---|---|
| 模式切换 | 100% 宽，最大 320px，large 尺寸 | 100% 宽，small 尺寸 |
| 进度环 | 200 | 120 |
| 时间字号 | 48 | 28 |
| 预设 | 4 个带单位按钮（default 尺寸） | 一行数字 `Segmented`（small） |
| 自定义输入 | 宽 110 | 宽 72 |
| 区域间距 | 16px | 8px |
| 完成提示 | 显示 | 不显示 |
| 运行中时长选择 | 常驻但禁用 | 隐藏 |
| 布局判定 | 容器宽度 > 320 | 容器宽度 ≤ 320 |

### 10.5 样式

- 继续使用 `src/tools/clock/clock.module.css`，新增 `.modeSwitch`、`.modeSwitchCompact`、`.durationPicker`、`.presetSegmented`、`.presetActive`、`.unit` 等类。
- 颜色只用主题变量（`--color-primary`、`--color-text-secondary`、`--color-success` 等），不写十六进制。
- clock 的密度完全由组件选择控制，CSS 中不保留 `@container (max-width: 320px)` 规则；`tool-page.css` 的 `container-type` 保持不变，供其他工具使用。

---

## 11 i18n

`src/i18n/locales/zh-CN.json` 与 `en-US.json` 同步新增/修改：

| 键 | 中文 | 英文 |
|---|---|---|
| `tools.clock.name` | 计时器 | Timer |
| `tools.clock.description` | 倒计时与正计时，支持预设与自定义时长 | Countdown and stopwatch with presets and custom durations |
| `tools.clock.mode_countdown` | 倒计时 | Countdown |
| `tools.clock.mode_stopwatch` | 计时 | Stopwatch |
| `tools.clock.mode_switch_running` | 计时进行中，暂停后可切换模式 | Pause the timer before switching modes |
| `tools.clock.stopwatch_hint` | 正计时不会自动结束 | The stopwatch does not stop automatically |

- `start` / `pause` / `resume` / `reset` / `finished` / `status_*` / `minutes` / `custom_minutes` / `error` / `finished_hint` 沿用 I26 键。
- Rust i18n 的 `clock.notification_title` / `clock.notification_body` 不变，因为普通计时不通知。
- `i18n.d.ts` 的类型从 `zh-CN` 推导，两个 locale 的键集必须一致。

---

## 12 文件清单

新增：

| 文件 | 内容 |
|---|---|
| `src/tools/clock/ClockModeSwitch.tsx` | 两套布局共用的模式切换 |
| `src/tools/clock/clockTime.ts` | `computeClockView` / `formatClockTime` |
| `src/tools/clock/__tests__/clockTime.test.ts` | 纯函数单测 |

修改：

| 文件 | 改动 |
|---|---|
| `src-tauri/src/clock.rs` | `ClockMode`、`elapsed_ms`、模式切换、普通计时运行态、条件 ticker |
| `src-tauri/src/cmd/clock.rs` | `clock_set_mode`；`clock_start` 改为 `Option<Millis>`；仅在倒计时时 spawn ticker |
| `src-tauri/src/protocol/dto/clock.rs` | `ClockModeDto`、`remaining_ms: Option`、`elapsed_ms` |
| `src-tauri/src/ipc.rs` | 注册 `clock_set_mode` |
| `src-tauri/tests/clock_runtime.rs` | 普通计时、模式切换、DTO 派生测试 |
| `src-tauri/tests/protocol_bindings.rs` | 导出 `ClockModeDto` |
| `src/types/protocol.contract.ts` | `contract:gen` 生成，不手改 |
| `src/tools/clock/clock.service.ts` | `setMode`、`start(durationMs?)` |
| `src/tools/clock/index.tsx` | 模式状态、`useLayoutEffect` 首帧测量、`computeClockView`、命令接线 |
| `src/tools/clock/clockView.ts` | 新 props、预设常量 |
| `src/tools/clock/ClockRing.tsx` | `progress: number \| null`、紧凑尺寸 |
| `src/tools/clock/ClockLayout.tsx` | 普通布局重组 |
| `src/tools/clock/ClockMiniLayout.tsx` | 紧凑布局重组 |
| `src/tools/clock/clock.module.css` | 新模式、时长选择、紧凑类 |
| `src/i18n/locales/zh-CN.json` | 新键 |
| `src/i18n/locales/en-US.json` | 新键 |

不修改：

- `src/tools/registry.ts` 的 `clock` 注册项（除 `tools.clock.description` 文案）。
- `src/tools/ToolPage.tsx`、`src/tools/ToolBox.tsx`、`src/tools/miniNavigate.ts`。
- `src-tauri/src/protocol/events.rs`、`src-tauri/src/setup.rs` 中的 `ClockRuntime` manage。

---

## 13 验收

### 13.1 静态与构建

- `npm run typecheck` 通过。
- `npm run lint` 通过。
- `npm run test` 通过。
- `npm run contract:gen` 后 `src/types/protocol.contract.ts` 含 `ClockModeDto` 与 `remainingMs: number | null`。
- 在 `src-tauri` 下 `cargo fmt` 后运行：
  `cargo test -p assistant-app --test clock_runtime --test protocol_bindings --test protocol_contract --test assembly_contract`
- `cargo clippy --all-targets` 0 warning / 0 error。

### 13.2 Rust 单元/集成测试

- 倒计时：现有用例在新字段下继续通过；`remaining_ms` 为 `Some`。
- 普通计时：`start → 前进 now → elapsed` 单调递增；暂停冻结；继续后增长。
- 普通计时：`mark_finished` 返回 `None`，状态不会变成 `Finished`，无 ticker。
- 模式切换：切到**不同**模式且 `Running` 时 `set_mode` 返回错误且状态不变。
- 模式切换：切到**相同**模式时幂等，即使 `Running` 也不报错、不重置。
- 模式切换：`Idle` 切到另一模式后回到 `Idle`、elapsed 0、`duration_ms` 保留。
- 快照：普通计时 `remaining_ms == None`，`elapsed_ms` 为累计值；倒计时 `remaining_ms == Some(...)`。

### 13.3 前端单测

- `formatClockTime`：倒计时 `59_500 → "01:00"`，普通计时 `59_500 → "00:59"`；`3_600_000 → "1:00:00"`；`0 → "00:00"`。
- `computeClockView`：倒计时 Running/Paused/Finished 的 `remaining` 与 `progress`；普通计时 Running/Paused 的 `elapsed` 与 `remaining === null`。
- 普通计时超过 `duration_ms` 后不被裁剪。

### 13.4 手工回归

- normal ↔ mini 往返，模式与计时状态不丢、不重挂载。
- mini 窗口下两个模式均无横向溢出，预设行保持一行。
- 倒计时运行中模式开关不可用；暂停后可切换并回到目标模式 Idle。
- 普通布局倒计时运行/暂停时，时长选择仍可见但整体禁用；紧凑布局仍隐藏。
- 普通计时超过 1 小时显示 `h:mm:ss`。
- 普通计时到点不通知；倒计时隐藏窗口到点仍通知。
- 从 mini 切到普通布局（窗口拉宽或退出 mini）时首帧不出现明显的普通布局闪烁。
- 停止后重新开始、暂停后继续、重置后剩余/已用值符合预期。
- 计时模式不写任何配置文件或数据库。

---

## 14 未验证前提

| 前提 | 验证方式 | 失败处置 |
|---|---|---|
| 280px 内容宽度下 `Segmented` 4 个预设 + `InputNumber` 72 + 开始按钮可在一行/两行内放下 | 实际启动 mini 窗口截图 | 缩小预设字号或把自定义输入移到第二行 |
| `useLayoutEffect` 首帧测量可消除布局闪烁 | 录屏逐帧检查 | 回退到 CSS 容器查询兜底，或接受一帧切换 |
| `h:mm:ss` 在 120px 环内仍可读 | 普通计时运行超过 1 小时截图 | 缩小字号或改用 `hh:mm:ss` 换行策略 |
| Rust 命令的 `Option<Millis>` 参数在 Tauri invoke 中可省略 | 前端调用 `clock_start` 不传 `durationMs` 联调 | 改为 `clock_start_countdown` / `clock_start_stopwatch` 两条命令 |
| 用户接受「运行中不可切换模式」 | 手工回归与反馈 | 改为运行中切换前弹确认并自动重置 |

---

## 15 延期项

| # | 项 | 处置 |
|---|---|---|
| D1 | 番茄钟阶段与轮次 | 不在本期；需要时在 `ClockMode` 之上增加阶段机，不修改现有两模式接口 |
| D2 | 计时状态跨重启恢复 | 不做；进程退出即清空 |
| D3 | 托盘倒计时图标与常驻提醒 | 不做；通知仍只覆盖倒计时到点 |
| D4 | 手动紧凑模式开关 | 不做；布局由容器宽度决定 |
| D5 | 多计时器并行 | 不做；保持单实例 |

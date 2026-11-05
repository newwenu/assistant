# I37 — clock 双模式与双布局优化任务清单

> 规格：[I37-clock-modes-layout-spec.md](./I37-clock-modes-layout-spec.md)
> 顺序：T0 → T1 → T2 → T3 → T4 → T5 → T6 → T7 → T8
> 勾选条件：代码已改 **且** 对应验收已跑
> 范围：只交付 clock 的倒计时/普通计时与两套布局优化；番茄钟阶段、跨重启恢复、托盘图标不在本期。

---

## T0 口径与前置确认

- [x] T0.1 确认本规格取代 I26 §9.2–§9.4 的时钟状态机定义，I26 其余结论不变
- [x] T0.2 确认两种模式共用单实例，不引入番茄钟阶段/轮次
- [x] T0.3 确认普通计时不进入 `Finished`、不通知、不 spawn ticker
- [x] T0.4 确认布局选择继续按容器宽度，不读窗口 `mode`
- [x] T0.5 确认 `duration_ms` 在普通计时下保留上次倒计时值，仅用于切回恢复
- [x] T0.6 确认 `remaining_ms` 改为 `Option<Millis>`，输出侧不加 `skip_serializing_if`

---

## T1 Rust 运行态与 DTO

`src-tauri/src/clock.rs`、`src-tauri/src/protocol/dto/clock.rs`

- [x] T1.1 `ClockMode { Countdown, Stopwatch }` 内部枚举，默认 `Countdown`
- [x] T1.2 `ClockInner` 增加 `mode`，保留 `duration_ms` / `elapsed_before_ms` / `started_at_ms`
- [x] T1.3 唯一派生：`running_elapsed = elapsed_before_ms + (Running ? now - started_at : 0)`
- [x] T1.4 倒计时派生 `remaining = clamp(duration - elapsed, 0, duration)`、`elapsed = duration - remaining`
- [x] T1.5 普通计时派生 `remaining = None`、`elapsed = running_elapsed`，不裁剪到 `duration_ms`
- [x] T1.6 `set_mode`：相同模式幂等；模式变化且 `Running` 返回错误；否则回到目标模式 `Idle`
- [x] T1.7 `start` 支持可选时长：倒计时用传入值或当前值并校验 1–180 分钟；普通计时忽略时长
- [x] T1.8 `pause` / `resume` / `reset` 对两种模式语义一致，保留模式与倒计时时长
- [x] T1.9 `mark_finished` 仅 `Countdown + Running` 生效；普通计时返回 `None`
- [x] T1.10 状态命令递增 `generation`；旧 ticker 自然退出
- [x] T1.11 `ClockModeDto` 落地，`ClockSnapshotDto` 增加 `mode` / `elapsed_ms`，`remaining_ms` 改 `Option`
- [x] T1.12 `From<ClockSnapshot> for ClockSnapshotDto` 同步新字段
- [x] T1.13 普通计时不 spawn ticker；倒计时 `start` / `resume` 才 spawn

---

## T2 命令、IPC 与契约

`src-tauri/src/cmd/clock.rs`、`src-tauri/src/ipc.rs`、`src-tauri/tests/protocol_bindings.rs`、`src/types/protocol.contract.ts`

- [x] T2.1 `clock_set_mode(mode: ClockModeDto)` 命令落地
- [x] T2.2 `clock_start(duration_ms: Option<Millis>)` 改签名，倒计时/普通计时分派
- [x] T2.3 `clock_resume` 仅倒计时 spawn ticker；普通计时只更新状态
- [x] T2.4 `ipc.rs` 注册 `clock_set_mode`，`clock_start` 名称不变
- [x] T2.5 `protocol_bindings.rs` 导出 `ClockModeDto`
- [x] T2.6 `npm run contract:gen` 生成 `src/types/protocol.contract.ts`，确认含 `mode`、`remainingMs: number | null`、`elapsedMs`
- [x] T2.7 前端 `clock.service.ts` 增加 `setMode`，`start` 支持可选 `durationMs`
- [ ] T2.8 联调确认 Tauri invoke 可省略 `durationMs`；若不可省略，按规格 §14 拆为 `clock_start_countdown` / `clock_start_stopwatch`
  - 代码已按 `durationMs === undefined ? {} : { durationMs }` 调用，等待桌面应用联调确认

---

## T3 前端视图模型与布局选择

`src/tools/clock/clockTime.ts`、`src/tools/clock/clockView.ts`、`src/tools/clock/index.tsx`

- [x] T3.1 新增 `clockTime.ts`：`computeClockView(snapshot, nowMs)` / `formatClockTime(ms, mode)`
- [x] T3.2 本地续算以 `elapsedBeforeMs + startedAtMs` 为准，不用快照 `remaining_ms` 二次扣减
- [x] T3.3 倒计时剩余秒向上取整；普通计时已用秒向下取整
- [x] T3.4 总秒数 ≥ 3600 输出 `h:mm:ss`，否则 `mm:ss`
- [x] T3.5 `clockView.ts` 扩展 `ClockViewProps`，类型引用 `ClockSnapshotDto["mode"]` / `["status"]`
- [x] T3.6 `index.tsx` 管理 `mode` / `compact` / `busy` / `error`，命令失败后重新拉取快照
- [x] T3.7 首帧改用 `useLayoutEffect` 测量容器宽度，阈值使用 `MINI_DENSITY_BREAKPOINT`
- [x] T3.8 `onPreset` 只改 `minutes`，不再直接开始；`onStart` 统一启动
- [x] T3.9 `clock:finished` 事件只更新倒计时完成态
- [x] T3.10 普通计时不启动前端到点逻辑，只按本地 1 秒更新 `nowMs`

---

## T4 普通布局

`src/tools/clock/ClockLayout.tsx`、`src/tools/clock/ClockModeSwitch.tsx`、`src/tools/clock/ClockRing.tsx`

- [x] T4.1 新增 `ClockModeSwitch`：`Segmented`，普通布局 large 尺寸、最大宽 320；`Running` 禁用，禁用时显示 `mode_switch_running`
- [x] T4.2 `ClockRing` 支持 `progress: number | null`；`null` 只画轨道
- [x] T4.3 普通布局顺序：模式切换 → 进度环 → 状态 → 错误 → 时长选择/主控制
- [x] T4.4 倒计时所有状态显示时长选择；`idle` / `finished` 可编辑并显示 `开始`，`running` / `paused` 保留展示但整体禁用
- [x] T4.5 预设只改时长，选中态用边框/文字强调，不与 `开始` 抢 `primary`
- [x] T4.6 `running` / `paused` 时长选择保留但禁用，主控制仅显示 `暂停/继续` + `重置`
- [x] T4.7 普通计时 `idle` 显示 `开始` + 禁用 `重置` + `stopwatch_hint`
- [x] T4.8 `countdown && finished` 显示 `finished_hint`，允许再次开始
- [x] T4.9 `重置` 在 `idle` 禁用，在 `running` / `paused` / `finished` 可用
- [x] T4.10 普通计时进度环不显示进度描边
- [x] T4.11 普通布局使用 200 环、48 时间字号、default 按钮与 16px 间距，避免与紧凑布局同密度

---

## T5 紧凑布局

`src/tools/clock/ClockMiniLayout.tsx`、`src/tools/clock/clock.module.css`

- [x] T5.1 紧凑布局顺序：模式切换 → 小环 → 状态 → 错误 → 时长选择/主控制
- [x] T5.2 进度环 120×120、时间 28px
- [x] T5.3 倒计时 `idle` / `finished`：一行数字 `Segmented` 15/25/45/60 + `InputNumber`（宽 72）+ `开始`
- [x] T5.4 当前分钟数不在预设内时 `Segmented` 不高亮任何项
- [x] T5.5 `running` / `paused` 显示单行 `暂停/继续` + `重置`
- [x] T5.6 普通计时 `idle` 显示 `开始` + 禁用 `重置`
- [x] T5.7 不显示 `finished_hint`、模式说明、自定义说明
- [x] T5.8 间距 8px、顶部内边距 4px；不产生两行以上按钮组
- [x] T5.9 `clock.module.css` 补充 `.modeSwitch` / `.modeSwitchCompact` / `.durationPicker` / `.presetSegmented` / `.presetActive` / `.unit`
- [x] T5.10 CSS 中不保留 `@container (max-width: 320px)`；密度由组件选择控制
- [x] T5.11 颜色只使用主题变量，不写十六进制

---

## T6 i18n

`src/i18n/locales/zh-CN.json`、`src/i18n/locales/en-US.json`

- [x] T6.1 更新 `tools.clock.name` 与 `tools.clock.description`
- [x] T6.2 新增 `tools.clock.mode_countdown` / `mode_stopwatch`
- [x] T6.3 新增 `tools.clock.mode_switch_running`
- [x] T6.4 新增 `tools.clock.stopwatch_hint`
- [x] T6.5 确认两个 locale 键集一致，`i18n.d.ts` 类型检查通过
- [x] T6.6 Rust 通知文案保持不变

---

## T7 测试与静态检查

`src-tauri/tests/clock_runtime.rs`、`src/tools/clock/__tests__/clockTime.test.ts`

- [x] T7.1 Rust：倒计时现有用例适配 `remaining_ms: Some`
- [x] T7.2 Rust：普通计时 Running 单调递增；Paused 冻结；Resume 继续
- [x] T7.3 Rust：普通计时 `mark_finished` 返回 `None`，状态不变
- [x] T7.4 Rust：模式变化且 Running 返回错误；相同模式幂等
- [x] T7.5 Rust：模式切换后 `duration_ms` 保留、elapsed 归零
- [x] T7.6 Rust：普通计时快照 `remaining_ms == None`
- [x] T7.7 前端：`formatClockTime` 覆盖 `59_500`、`3_600_000`、`0`
- [x] T7.8 前端：`computeClockView` 覆盖倒计时 Running/Paused/Finished 与普通计时 Running/Paused
- [x] T7.9 前端：普通计时超过 `duration_ms` 不被裁剪
- [x] T7.10 `npm run format` → `npm run typecheck`
- [x] T7.11 `npm run lint`
- [x] T7.12 `npm run test`
- [x] T7.13 `npm run contract:gen`
- [x] T7.14 在 `src-tauri` 下 `cargo fmt` 后运行：
  `cargo test -p assistant-app --test clock_runtime --test protocol_bindings --test protocol_contract --test assembly_contract`
- [ ] T7.15 `cargo clippy --all-targets` 0 warning / 0 error
  - 本次改动无新增 clippy warning；仓库存在既有 `assistant-config/src/vault_conf.rs` 的 `derivable_impls` warning，`-D warnings` 会失败

---

## T8 手工回归与文档回填

- [ ] T8.1 normal ↔ mini 往返，模式与计时状态不丢、不重挂载
- [ ] T8.2 mini 下两个模式均无横向溢出，预设行保持一行
- [ ] T8.3 倒计时运行中模式开关不可用；暂停后可切换并回到目标模式 Idle
- [ ] T8.4 普通计时超过 1 小时显示 `h:mm:ss`
- [ ] T8.5 普通计时到点不通知；倒计时隐藏窗口到点仍通知
- [ ] T8.6 从 mini 切回普通布局首帧无明显闪烁
- [ ] T8.7 重新开始、暂停继续、重置后的剩余/已用值符合预期
- [ ] T8.8 计时模式不写 SQLite / `AppConf` / localStorage
- [x] T8.9 回填本清单执行记录
- [ ] T8.10 回填规格 §14 未验证前提的实测结果

---

## 执行记录

| 检查 | 结果 |
|---|---|
| `npm run typecheck` | 0 错 |
| `npm run lint` | 0 错 0 warning |
| `npm run test` | 28 文件 / 327 例通过（含 clockTime 8 例） |
| `npm run contract:gen` | 成功，契约含 62 个 DTO，`ClockModeDto` 与 `remainingMs: number \| null` 已生成 |
| Rust 集成测试 | clock_runtime 14 / protocol_bindings 1 / protocol_contract 10 / assembly_contract 8，全部通过 |
| `cargo check --all-targets` | 通过 |
| `cargo clippy --all-targets` | 本次改动无新增 warning；既有 `assistant-config` derivable_impls warning 未处理 |
| `cargo clippy --all-targets -- -D warnings` | 失败于上述既有 warning |
| 手工回归 | 未执行（需要启动桌面应用） |
| `npm run contract:check` | 待提交后补跑（生成文件未提交时 diff 必然非空） |

---

## 提交切分建议

1. `docs(I37): 新增 clock 双模式与双布局优化规格及任务清单`
2. `feat(I37): clock 运行态支持倒计时与普通计时`
3. `feat(I37): clock 双模式命令与协议契约更新`
4. `feat(I37): 优化 clock 普通与紧凑布局`
5. `chore(I37): 回填 clock 双模式验收记录`

拆分时保证每个提交单独可编译；契约生成文件与 DTO 改动放在同一提交，避免中间提交类型不一致。

---

## 延期项

| # | 项 | 处置 |
|---|---|---|
| D1 | 番茄钟阶段与轮次 | 不在本期；需要时在 `ClockMode` 之上增加阶段机 |
| D2 | 跨重启恢复计时 | 不做；进程退出即清空 |
| D3 | 托盘倒计时图标与常驻提醒 | 不做；通知仍只覆盖倒计时到点 |
| D4 | 手动紧凑模式开关 | 不做；布局由容器宽度决定 |
| D5 | 多计时器并行 | 不做；保持单实例 |

# I26 — mini 模式与时钟工具任务清单

> 规格：[I26-mini-mode-clock-spec.md](./I26-mini-mode-clock-spec.md)
> 顺序：M0 → M1 → M2 → M3 → M4 → M5 → M6 → M7 → M8
> 勾选条件：代码已改 **且** 对应验收已跑
> 范围：只交付 mini 模式与 `clock`；便签（notes）与本清单无关，见规格 §16。

## M0 口径与文档

- [x] M0.1 规格仅保留 mini + clock，notes 移到延期项
- [x] M0.2 删除 `main_window_mode` 落盘设计，统一为「形态不记忆、几何记忆」
- [x] M0.3 几何单位统一：位置物理像素、尺寸逻辑像素
- [x] M0.4 「启动落点」改为「进入 mini 的落点」
- [x] M0.5 `docs/store/I27-tool-data-store-spec.md` 的 I26 引用改到新文件名，并注明便签延期

## M1 配置与窗口几何

`src-tauri/crates/assistant-config/src/{app_conf.rs,constant.rs}`、`src-tauri/src/protocol/dto/config.rs`、`src/types/protocol.contract.ts`

- [x] M1.1 `AppConf` 增加 6 个几何字段（`main_window_x/y/width/height`、`mini_window_x/y`）
- [x] M1.2 `AppConf::new()` 六字段为 `None`，`validate()` 不新增形态校验
- [x] M1.3 `AppConfDto` 同步 6 字段并透出
- [x] M1.4 `constant.rs` 增加 4 个 mini 逻辑像素常量
- [x] M1.5 `src/config/constants.ts` 增加 `MINI_WINDOW_WIDTH/HEIGHT`、`MINI_TITLEBAR_HEIGHT`、`MINI_DENSITY_BREAKPOINT`
- [x] M1.6 `npm run contract:gen` 生成契约，`src/types/protocol.contract.ts` 含新字段

## M2 窗口模式命令与启动恢复

`src-tauri/src/window.rs`、`src-tauri/src/setup.rs`、`src-tauri/src/cmd/{app.rs,config.rs}`、`src-tauri/src/ipc.rs`、`src-tauri/src/protocol/{events.rs,dto/clock.rs}`

- [x] M2.1 内部 `MainWindowMode { Normal, Mini }` + `MAIN_WINDOW_MODE` 静态量
- [x] M2.2 `effective_on_top()`：`mode == Mini || stay_on_top`
- [x] M2.3 `set_main_window_mode`：校验 → persist 当前几何 → 施加尺寸/位置/置顶 → 更新状态 → 广播 `ui:window-mode-changed`
- [x] M2.4 进入 mini：`resizable(false)` → `min_size(240,320)` → `size(280,420)` → position 收敛 → 置顶
- [x] M2.5 退出 mini：恢复 normal 逻辑尺寸 → `min_size(300,200)` → `resizable(true)` → position 收敛 → 按 `stay_on_top` 置顶
- [x] M2.6 `persist_main_geometry`：normal 写 x/y + 逻辑宽高；mini 只写 x/y
- [x] M2.7 几何写入复用 `apply_app_conf` 的配置锁，避免与其它 `patch_conf` 互相覆盖
- [x] M2.8 `restore_main_window`：启动只恢复 normal 尺寸与位置，不进入 mini
- [x] M2.9 `setup::init` 在 `position_orb_window` 后调用恢复
- [x] M2.10 `on_window_event` 的 `CloseRequested` 隐藏前写几何
- [x] M2.11 `perform_exit` 前写几何
- [x] M2.12 `stay_on_top` 副作用改走 `effective_on_top`
- [x] M2.13 `ipc.rs` 注册 `set_main_window_mode`
- [x] M2.14 `event_name` 增加 `WINDOW_MODE_CHANGED`
- [x] M2.15 `WindowModeDto` / `WindowModeChangedPayloadDto` 落地并加入契约导出

## M3 前端模式 store 与外壳

`src/stores/windowModeStore.ts`、`src/services/windowService.ts`、`src/layouts/MainLayout.tsx`、`src/components/Titlebar.tsx`、`src/types/events.ts`、`src/tools/{ToolBox.tsx,ToolPage.tsx,MiniToolBox.tsx,miniNavigate.ts}`、`src/tools/tool-page.css`

- [x] M3.1 `windowModeStore`：`mode` 默认 `normal`，`setMode` / `toggleMode` / `applyExternalMode`
- [x] M3.2 `windowService.setMainWindowMode` 代理命令
- [x] M3.3 `UiEvent.WindowModeChanged` 监听并同步 store
- [x] M3.4 `MainLayout` 按 mode 卸载侧栏、挂 `data-window-mode`
- [x] M3.5 `Titlebar` 支持 `mode` / `onToggleMode`：mini 高 32，隐藏置顶/设置/主题
- [x] M3.6 `ToolBox` 在 mini 下渲染 `MiniToolBox`
- [x] M3.7 `MiniToolBox`：搜索框 + 单列列表，顺序 recent → favorites → 其余并去重
- [x] M3.8 `ToolPage` mini 密度分支：padding / 标题 / 描述 / 收藏 / maxWidth
- [x] M3.9 `ToolPage` 内容容器加 `container-type: inline-size`
- [x] M3.10 `miniNavigate`：`resolveNavigateMode` / `navigateInCurrentMode`（切换按钮不再调用落点函数）
- [x] M3.11 切换按钮只改形态、不导航；不兼容页面提示 `mini.page_unsupported` 并保持 normal
- [x] M3.12 `useNavigationListener` 与 `commandAggregation` 的路由动作走形态判断
- [x] M3.13 `ToolDefinition.miniFriendly`

## M4 分类与工具注册

`src/tools/categories.ts`、`src/tools/registry.ts`

- [x] M4.1 `CATEGORY_KEYS` 末尾追加 `productivity`
- [x] M4.2 `productivity` 签名与 `TOOL_CATEGORIES` meta 追加
- [x] M4.3 `clock` 注册，`miniFriendly: true`，`widthMode: "default"`
- [x] M4.4 `classifyKeywords` 回归：既有工具归类不变

## M5 clock 后端

`src-tauri/src/clock.rs`、`src-tauri/src/cmd/clock.rs`、`src-tauri/src/protocol/dto/clock.rs`、`src-tauri/tests/clock_runtime.rs`

- [x] M5.1 `ClockStatus` / `ClockInner` / `ClockRuntime` + `generation`
- [x] M5.2 剩余时间按墙钟重算，不做递减计数
- [x] M5.3 `start/pause/resume/reset/snapshot` 状态转换与空操作语义
- [x] M5.4 时长约束 1–180 分钟，越界返回错误
- [x] M5.5 ticker：最多 1s 重算、`generation` 退出、到点 `mark_finished` 幂等
- [x] M5.6 `ClockSnapshotDto` / `ClockStatusDto` + `Millis`
- [x] M5.7 5 个命令注册到 `ipc.rs`，`ClockRuntime` manage 到 setup
- [x] M5.8 单元测试：剩余时间、暂停冻结、恢复、幂等完成、generation

## M6 clock 前端

`src/tools/clock/{index.tsx,clock.service.ts,clock.module.css}`

- [x] M6.1 `clock.service.ts` 五个命令代理
- [x] M6.2 首帧快照 + 本地 1s 续算，隐藏时停表、可见时重取快照
- [x] M6.3 剩余 `mm:ss`、进度环、开始/暂停/继续/重置
- [x] M6.4 预设 15/25/45/60 + 自定义 1–180
- [x] M6.5 `clock:finished` 事件驱动完成态
- [x] M6.6 窄容器样式（`@container max-width: 320px`）
- [x] M6.7 i18n 键集（中英一致）

## M7 通知

`src-tauri/Cargo.toml`、`src-tauri/src/plugins.rs`、`src-tauri/capabilities/default.json`、`assistant-i18n` 资源

- [x] M7.1 引入 `tauri-plugin-notification = "2"` 并注册插件
- [x] M7.2 capability 增加 notification 权限
- [x] M7.3 到点：窗口不可见/未聚焦时发通知，发送失败回退显示主窗口
- [x] M7.4 Rust i18n 增加 `clock.notification_title/body`

## M8 验证

- [x] M8.1 `npm run format` → `npm run typecheck`
- [x] M8.2 `npm run lint`
- [x] M8.3 `npm run test`（27 文件 / 320 例通过，含分类与 miniNavigate 新增用例）
- [ ] M8.4 `npm run contract:check`
  - `npm run contract:gen` 已通过；`contract:check` 会在生成后检查 git diff，未提交时必然非空，提交后补跑
- [x] M8.5 `cargo fmt` → 集成测试
  - `cargo test -p assistant-app --test clock_runtime --test protocol_bindings --test protocol_contract --test assembly_contract`
  - 结果：clock 6 + bindings 1 + protocol 10 + assembly 8，全部通过
  - 本机限制：lib 测试二进制缺 Common Controls v6 清单，启动即 `STATUS_ENTRYPOINT_NOT_FOUND`；因此时钟状态机单测落在 `tests/clock_runtime.rs`
- [x] M8.6 `cargo clippy --all-targets`（0 warning / 0 error）
- [ ] M8.7 手工回归：normal ↔ mini 往返、重启回 normal、mini 位置记忆、clock 窄态、托盘进设置先展开、隐藏到点提醒、`productivity` 分组
  - 需要启动桌面应用；本次未执行

## 执行记录

| 检查 | 结果 |
|---|---|
| `npm run typecheck` | 0 错 |
| `npm run lint` | 0 错 0 warning |
| `npm run test` | 27 文件 / 320 例通过 |
| `npm run typecheck`（切换按钮修正后） | 0 错 |
| `npm run test`（切换按钮修正后） | 27 文件 / 319 例通过 |
| `npm run contract:gen` | 成功，契约含 61 个 DTO |
| `cargo check --all-targets` | 通过 |
| `cargo clippy --all-targets` | 0 warning / 0 error |
| Rust 集成测试 | clock 6 / bindings 1 / protocol 10 / assembly 8 全通过 |

## 延期项

| # | 项 | 处置 |
|---|---|---|
| D1 | notes 便签工具 | 不在本阶段；规格 §16 保留结论，后续单独排期 |
| D2 | I27 工具数据存储 | 无消费者，保持 Draft |
| D3 | mini 形态跨重启记忆 | **不做**，启动永远 normal |
| D4 | 形态切换全局快捷键 | 不复用 `toggle_main`；需要时新增独立 id 并登记设置页 |
| D5 | 时钟跨重启续算 | 不做；进程退出即清空 |

## 提交切分

1. `docs(I26): 收敛 mini 与时钟规格，拆分便签延期项`
2. `feat(I26): 主窗口几何持久化与 mini 模式命令`
3. `feat(I26): 前端窗口模式外壳与 mini 工具列表`
4. `feat(I26): productivity 分类与 clock 工具`
5. `feat(I26): 时钟到点系统通知`
6. `chore(I26): 契约生成与验收记录`
